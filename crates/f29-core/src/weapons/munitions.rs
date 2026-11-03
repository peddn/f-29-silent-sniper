//! Was fliegt: eigene und feindliche Waffen und Täuschkörper, mit dem
//! Zufall der Waffen und ihren Ereignissen.

use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Munitions {
    pub in_flight: Vec<Weapon>,
    /// Täuschkörper in der Luft.
    pub decoys: Vec<Decoy>,
    next_id: u32,
    rng: Pcg32,
    /// Ereignisse mit Tick, neueste zuletzt.
    pub events: EventLog<WeaponEvent>,
}

impl Munitions {
    pub fn new(seed: u64) -> Self {
        Self {
            in_flight: Vec::new(),
            decoys: Vec::new(),
            next_id: 0,
            rng: Pcg32::new(seed, STREAM_WEAPONS),
            events: EventLog::default(),
        }
    }

    /// Nummer für Waffe oder Täuschkörper.
    pub(super) fn next_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Startet eine Boden-Luft-Rakete der Radarstellung `site` (Index in
    /// `Sim::targets`, Startpunkt `from`) auf das Flugzeug bei `aircraft` –
    /// wenn es in Reichweite ist, die Stellung keine Rakete mehr in der Luft
    /// hat und nicht schon `max` feindliche fliegen (wie im Original).
    pub fn launch_sam(
        &mut self,
        tick: u64,
        kind: WeaponKind,
        site: usize,
        from: DVec3,
        aircraft: DVec3,
        max: usize,
    ) -> bool {
        let to = aircraft - from;
        if to.length() > kind.spec().max_range_m || !self.hostile_slot_free(Launcher::Site(site), max) {
            return false;
        }
        // Im Startwinkel des Typs, mindestens knapp über der Linie zum
        // Flugzeug abheben.
        let flat = DVec3::new(to.x, to.y, 0.0).normalize_or_zero();
        let direct = detmath::atan2(-to.z, to.x.hypot(to.y)) + SAM_DIRECT_RAD;
        let elev = direct.max(kind.spec().launch_rad).min(std::f64::consts::FRAC_PI_2);
        let dir = flat * elev.cos() + DVec3::new(0.0, 0.0, -elev.sin());
        let id = self.next_id();
        let mut w = Weapon::new(id, kind, from, dir * SAM_LAUNCH_MPS);
        w.owner = Some(Launcher::Site(site));
        w.target = Some(Track::Aircraft);
        self.in_flight.push(w);
        self.events.push(tick, WeaponEvent::SamLaunch { kind, id, site });
        true
    }

    /// Je Stellung bzw. Jäger eine Rakete in der Luft, insgesamt höchstens
    /// `max` (wie im Original; Regelset).
    fn hostile_slot_free(&self, by: Launcher, max: usize) -> bool {
        let hostile = self.in_flight.iter().filter(|w| w.owner.is_some());
        !hostile.clone().any(|w| w.owner == Some(by)) && hostile.count() < max
    }

    /// Startet die Luft-Luft-Rakete `kind` des Jägers `fighter` (Ort und
    /// Geschwindigkeit `from`) auf das Flugzeug bei `aircraft` – wenn es in ihrer
    /// Reichweite, aber nicht näher als die Mindestentfernung des Regelsets
    /// ist und ein Platz frei ist (wie im Original).
    pub fn launch_air(
        &mut self,
        tick: u64,
        kind: WeaponKind,
        fighter: usize,
        (from, velocity): (DVec3, DVec3),
        aircraft: DVec3,
        rules: &Rules,
    ) -> bool {
        let range = (aircraft - from).length();
        if !(rules.fighter_min_range_m..=kind.spec().max_range_m).contains(&range)
            || !self.hostile_slot_free(Launcher::Fighter(fighter), rules.max_hostile_missiles)
        {
            return false;
        }
        let id = self.next_id();
        let mut w = Weapon::new(id, kind, from, velocity);
        w.owner = Some(Launcher::Fighter(fighter));
        w.target = Some(Track::Aircraft);
        self.in_flight.push(w);
        self.events.push(tick, WeaponEvent::AirLaunch { kind, id, fighter });
        true
    }

    /// Täuschkörper bei `position` in die Luft; `owner` wie
    /// [`Decoy::owner`] (feindlich: der Jäger).
    pub fn push_decoy(&mut self, tick: u64, kind: DecoyKind, position: DVec3, owner: Option<Launcher>) {
        let id = self.next_id();
        // Zufall und Abwurfzeitpunkt: Der k-te Täuschkörper einer Mission
        // würfelt nicht immer gleich.
        let salt = self.rng.next_u32() ^ tick as u32;
        self.decoys.push(Decoy { id, kind, owner, position, age_s: 0.0, salt });
    }

    /// Alle Waffen im Flug einen Zeitschritt weiterbewegen; Zünder und
    /// Einschläge auswerten.
    pub fn step(&mut self, world: &mut World<'_>) {
        for d in &mut self.decoys {
            d.age_s += world.dt;
        }
        self.decoys.retain(|d| d.age_s < DECOY_LIFE_S);
        let decoys = self.decoys.clone();
        let mut i = 0;
        while i < self.in_flight.len() {
            let mut w = self.in_flight[i];
            let view = View {
                targets: world.targets,
                terrain: world.terrain,
                designated: world.designated,
                aircraft: world.flying.then_some((world.aircraft, world.aircraft_velocity)),
                fighters: world.fighters,
                decoys: &decoys,
                painted: w.owner.is_some_and(|o| world.painting.contains(&o)),
                jammed: world.jammed,
                los: true,
            };
            if w.kind.is_guided() && (world.tick + u64::from(w.id)).is_multiple_of(SEEKER_TICKS) {
                let had = w.tracking;
                if let Some(e) = seek(&mut w, &view) {
                    self.events.push(world.tick, e);
                }
                if had && !w.tracking && w.kind.spec().seeker == Seeker::Laser {
                    self.events.push(world.tick, WeaponEvent::LaserLost { kind: w.kind, id: w.id });
                }
            }
            let aim = w.target.filter(|_| w.tracking).and_then(|t| view.locate(t));
            match advance(&mut w, aim, world.terrain, world.dt) {
                Fate::Flying => {
                    self.in_flight[i] = w;
                    i += 1;
                }
                Fate::Fuze(_) if matches!(w.target, Some(Track::Fighter(_))) => {
                    self.in_flight.remove(i);
                    let Some(Track::Fighter(fighter)) = w.target else { unreachable!() };
                    let on = Struck::Fighter(fighter);
                    self.events.push(world.tick, WeaponEvent::Hit { kind: w.kind, id: w.id, on });
                }
                Fate::Fuze(_) if w.target == Some(Track::Aircraft) => {
                    self.in_flight.remove(i);
                    let on = Struck::Aircraft;
                    self.events.push(world.tick, WeaponEvent::Hit { kind: w.kind, id: w.id, on });
                }
                // Feindlich im Gelände: wirkungslos.
                Fate::Fuze(p) | Fate::Ground(p) if w.owner.is_some() => {
                    self.in_flight.remove(i);
                    let result = ImpactResult::Ground;
                    self.events.push(
                        world.tick,
                        WeaponEvent::Impact { kind: w.kind, id: w.id, position: p, result },
                    );
                }
                Fate::Fuze(p) | Fate::Ground(p) => {
                    self.in_flight.remove(i);
                    self.impact(world, &w, p);
                }
                Fate::Expired => {
                    self.in_flight.remove(i);
                    self.events.push(world.tick, WeaponEvent::Expired { kind: w.kind, id: w.id });
                }
            }
        }
    }

    /// Bordsysteme, die ein Treffer der Stärke `strength` verschlechtert:
    /// verschiedene, zufällig, im Mittel [`HIT_SYSTEMS`] × Stärke (eine
    /// Rakete genau [`HIT_SYSTEMS`]).
    pub fn roll_systems(&mut self, strength: f64) -> Vec<System> {
        let n = (strength * HIT_SYSTEMS as f64 + self.rng.next_f64()).floor() as usize;
        let mut pool = System::ALL.to_vec();
        (0..n.min(pool.len())).map(|_| pool.remove(self.rng.next_u32() as usize % pool.len())).collect()
    }

    /// Schaden ohne kritischen Treffer (Bordkanone).
    pub(crate) fn apply_damage(target: &mut Target, i: usize, amount: f64) -> ImpactResult {
        Self::apply(target, i, amount, false)
    }

    /// Schaden an einem Ziel anrichten; liefert das Ergebnis für dieses Ziel.
    pub(super) fn apply(target: &mut Target, i: usize, amount: f64, critical: bool) -> ImpactResult {
        if critical {
            target.damage = 1.0;
            target.state = TargetState::Destroyed;
            return ImpactResult::Destroyed { target: i, critical: true };
        }
        target.damage = (target.damage + amount).min(1.0);
        // Rundungsreste (drei Drittel) zählen als ganz.
        if target.damage >= 1.0 - 1e-6 {
            target.damage = 1.0;
            target.state = TargetState::Destroyed;
            ImpactResult::Destroyed { target: i, critical: false }
        } else {
            target.state = TargetState::Damaged;
            ImpactResult::Damaged(i)
        }
    }

    /// Detonation bei `p`: wirkt auf alle intakten Ziele im Wirkradius.
    fn impact(&mut self, world: &mut World<'_>, w: &Weapon, p: DVec3) {
        let v = w.velocity;
        let impact_rad = detmath::atan2(v.z, (v.x * v.x + v.y * v.y).sqrt());
        let result = self.blast(world, w, p, impact_rad);
        self.events.push(world.tick, WeaponEvent::Impact { kind: w.kind, id: w.id, position: p, result });
        if (world.aircraft - p).length() < w.kind.spec().frag_radius_m {
            self.events.push(world.tick, WeaponEvent::FragWarning);
        }
    }

    /// Wirkung im Radius, nach Abstand vom Zielrand sortiert. Raketen würfeln
    /// wie im Original: Die Wirksamkeit ist die Chance, das Ziel zu zerstören,
    /// sonst wirkungslos. Bomben: Schaden fällt mit dem Abstand; ein voller
    /// Treffer würfelt einen kritischen.
    fn blast(&mut self, world: &mut World<'_>, w: &Weapon, p: DVec3, impact_rad: f64) -> ImpactResult {
        let reach = w.kind.spec().lethal_radius_m;
        let mut near: Vec<(f64, usize)> = world
            .targets
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_intact())
            .filter_map(|(i, t)| {
                let d = t.center() - p;
                let miss = ((d.x * d.x + d.y * d.y).sqrt() - t.class.radius())
                    .max(0.0)
                    .max(d.z.abs() - t.class.height());
                (miss <= reach).then_some((miss, i))
            })
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let missile = w.kind.spec().cruise_mps > 0.0;
        let mut result = ImpactResult::Ground;
        for (miss, i) in near {
            let e = w.kind.effectiveness(world.targets[i].class, impact_rad);
            let x = (miss / reach - FULL_EFFECT_FRACTION) / (1.0 - FULL_EFFECT_FRACTION);
            let falloff = (1.0 - x).clamp(0.0, 1.0);
            let this = if e <= 0.0 {
                ImpactResult::Ineffective(i)
            } else if missile {
                if self.rng.next_f64() < e {
                    Self::apply(&mut world.targets[i], i, 1.0, false)
                } else {
                    ImpactResult::Ineffective(i)
                }
            } else if e * falloff < MIN_DAMAGE {
                ImpactResult::Ground
            } else {
                let critical = falloff >= 1.0 && self.rng.next_f64() < CRITICAL_CHANCE;
                Self::apply(&mut world.targets[i], i, e * falloff, critical)
            };
            if this.rank() > result.rank() {
                result = this;
            }
        }
        result
    }
}
