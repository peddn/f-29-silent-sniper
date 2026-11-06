//! Feindliche Jäger (M-022), noch ohne eigene Waffen.
//!
//! Wie im Original stehen die Jäger auf den feindlichen Fliegerhorsten bereit
//! und starten erst bei Alarm – wenn ein Radar am Boden das Flugzeug bis zur
//! Stufe „verfolgt“ erfasst hat oder ein Jäger es mit seinem eigenen Radar
//! sieht. Der Kontaktort muss vom Platz aus in Reichweite liegen. Höchstens
//! [`Fighters::max_airborne`] fliegen gleichzeitig (Regelset), je Platz
//! startet einer nach dem anderen.
//!
//! **Bewegungsmodell** wie im Original statt eines Flugmodells: Die Querlage
//! folgt einem Sollwert mit begrenzter Rollrate, die Kurve folgt der
//! Querlage (koordiniert, `g·tan(Querlage)/Fahrt`), die Steigrate einem
//! Sollwert; Fahrt je Typ. Unter einer Mindesthöhe über Grund steigen sie.
//!
//! **Zustände:** bereit → Start (Rollen, Abheben) → Abfangen (zum
//! Kontaktort, in dessen Höhe) → Verfolgen (solange das eigene Radar das
//! Flugzeug sieht, direkt auf es zu) → Patrouille (Rennbahn über den
//! Kontaktort: drüber, geradeaus weiter, wenden – auf dem Rückweg schaut das
//! Radar ihn an) →
//! Rückflug und Landung, wenn die Flugdauer um ist; danach wieder bereit.
//!
//! **Eigenes Radar:** pingt wie eine Radarstellung (gleiches Stealth-Level,
//! M-020), aber nur in einem Kegel um die Nase.
//!
//! **Abschuss** (wie im Original): Eine Rakete zerstört, die Kanone braucht
//! je Typ einige Treffer – die Trefferstärken summieren sich bis
//! [`FighterSpec::toughness`]. Der getroffene Jäger stürzt ab und bleibt als
//! Wrack aus dem Spiel. Verfolgt ihn eine Rakete und ist sie nah, weicht er
//! aus – je Typ mit eigenen Manövern ([`Evasion`]) – und wirft dabei
//! passende Täuschkörper ([`DECOYS`] Stück); ob sie ihnen folgt, entscheidet
//! ihre Resistenz.
//!
//! **Pilotenkönnen** ([`Skill`], eigene Ergänzung – im Original wählt es nur
//! das Manöver): je Jäger aus dem Seed gewürfelt, von der Gegnerqualität
//! verschoben.
//! Es bestimmt die Länge der Feuerstöße, wie früh er Täuschkörper wirft und
//! aus welcher Entfernung er seine Rakete startet.

use crate::atmosphere::G0;
use crate::detmath;
use crate::events::EventLog;
use crate::gun::{GSH_23L, GSH_30_1, Gun, GunSpec};
use crate::math::{DVec3, deg};
use crate::radar::{Exposure, RadarKind, RadarSpec};
use crate::rng::Pcg32;
use crate::weapons::{DecoyKind, WeaponKind};
use f29_world::{AirfieldKind, Terrain};

/// Ein Radarkontakt eines Jägers gilt so lange als frisch (s).
const CONTACT_S: f64 = 15.0;
/// Mindesthöhe über Grund im Flug (m).
const MIN_AGL_M: f64 = 250.0;
/// Am Kontaktort: so nah (m) gilt er als erreicht, dann Patrouille.
const ARRIVE_M: f64 = 3_000.0;
/// Patrouille: so weit (m) fliegt er nach dem Überflug geradeaus weiter.
const PATROL_LEG_M: f64 = 12_000.0;
/// Rakete nur starten, wenn das Flugzeug so nah an der Nase liegt (rad).
pub const LAUNCH_CONE_RAD: f64 = deg(30.0);
/// Kanone: feuert, wenn das Flugzeug näher als so (m) und so nah an der
/// Nase (rad) liegt (wie im Original: sehr nah, fast genau voraus).
pub const GUN_RANGE_M: f64 = 1_200.0;
pub const GUN_CONE_RAD: f64 = deg(3.0);
/// Eigener Zufallsstrom je Kanone (plus Nummer des Jägers) und fürs
/// Pilotenkönnen.
const STREAM_GUNS: u64 = 0x4A47_554E;
const STREAM_SKILL: u64 = 0x534B_494C;
/// Pause zwischen zwei Feuerstößen (s).
pub const BURST_PAUSE_S: f64 = 1.5;
/// Täuschkörper je Jäger; beim Ausweichen wirft er alle so viele Sekunden
/// einen passenden, sobald die Rakete näher als [`SkillSpec::decoy_range_m`]
/// ist.
pub const DECOYS: u8 = 4;
pub const DECOY_INTERVAL_S: f64 = 1.0;

/// Pilotenkönnen eines Jägers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Skill {
    Rookie,
    Regular,
    Ace,
}

/// Was das Können bewirkt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillSpec {
    pub label: &'static str,
    /// Länge eines Feuerstoßes (s).
    pub burst_s: f64,
    /// Täuschkörper ab dieser Entfernung der Rakete (m).
    pub decoy_range_m: f64,
    /// Rakete erst innerhalb dieses Anteils ihrer Reichweite.
    pub launch_share: f64,
}

const SKILLS: [SkillSpec; 3] = [
    SkillSpec { label: "ROOKIE", burst_s: 0.3, decoy_range_m: 1_000.0, launch_share: 1.0 },
    SkillSpec { label: "REGULAR", burst_s: 0.6, decoy_range_m: 2_000.0, launch_share: 0.7 },
    SkillSpec { label: "ACE", burst_s: 1.0, decoy_range_m: 3_500.0, launch_share: 0.5 },
];

impl Skill {
    pub fn spec(self) -> &'static SkillSpec {
        &SKILLS[self as usize]
    }

    /// Gewürfelt (gleich verteilt), um `shift` Stufen verschoben.
    fn roll(rng: &mut Pcg32, shift: i8) -> Self {
        match (rng.next_u32() % 3) as i8 + shift {
            ..=0 => Skill::Rookie,
            1 => Skill::Regular,
            _ => Skill::Ace,
        }
    }
}
/// Abhebefahrt (m/s) und Beschleunigung beim Rollen (m/s²).
const ROTATE_MPS: f64 = 90.0;
const TAKEOFF_ACCEL: f64 = 12.0;

/// Näher als so (m) weicht ein Jäger einer Rakete aus, die ihn verfolgt.
pub const EVADE_RANGE_M: f64 = 6_000.0;

/// Ausweichmanöver vor einer Rakete (Spielerentscheidung v0.2.9: ein, zwei
/// je Typ).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evasion {
    /// Harte Kurve querab zur Rakete: Ihr Vorhaltwinkel wächst.
    Beam,
    /// Querab und dabei in den Tiefflug (Abschwung): Gelände kann den
    /// optischen Suchkopf abschatten.
    Dive,
}

/// Jägertypen (reale Vorbilder, eigene Werte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FighterKind {
    /// Alt, schnell, wenig wendig, Radar nur knapp nach vorn.
    MiG21,
    /// Modern, wendig, weit reichendes Radar mit breitem Kegel.
    MiG29,
}

/// Kenngrößen eines Jägertyps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FighterSpec {
    pub label: &'static str,
    /// Marschfahrt (m/s).
    pub cruise_mps: f64,
    /// Größte Querlage (rad) und Rollrate (rad/s).
    pub max_bank_rad: f64,
    pub roll_rate: f64,
    /// Größte Steig- bzw. Sinkrate (m/s).
    pub climb_mps: f64,
    /// Flugdauer (s), danach Rückflug; die halbe Strecke ist die Reichweite
    /// für den Start.
    pub endurance_s: f64,
    pub radar: RadarSpec,
    /// Halber Öffnungswinkel des Radars um die Nase (rad).
    pub radar_cone_rad: f64,
    /// Summe der Trefferstärken bis zum Abschuss (eine Rakete = 1; mit der
    /// M61A1: MiG-21 5, MiG-29 8 Treffer, Spielerentscheidung v0.2.9).
    pub toughness: f64,
    pub gun: &'static GunSpec,
    /// Luft-Luft-Rakete (Vorrat unbegrenzt, eine in der Luft).
    pub missile: WeaponKind,
    /// Ausweichmanöver, je Rakete reihum.
    pub evasions: &'static [Evasion],
}

const SPECS: [FighterSpec; 2] = [
    FighterSpec {
        label: "MIG-21",
        cruise_mps: 290.0,
        max_bank_rad: deg(60.0),
        roll_rate: deg(70.0),
        climb_mps: 110.0,
        endurance_s: 900.0,
        radar: RadarSpec { kind: RadarKind::Pulse, range_m: 15_000.0, sweep_s: 2.0 },
        radar_cone_rad: deg(30.0),
        toughness: 0.625,
        gun: &GSH_23L,
        missile: WeaponKind::Aphid,
        evasions: &[Evasion::Beam],
    },
    FighterSpec {
        label: "MIG-29",
        cruise_mps: 300.0,
        max_bank_rad: deg(75.0),
        roll_rate: deg(120.0),
        climb_mps: 150.0,
        endurance_s: 1_200.0,
        radar: RadarSpec { kind: RadarKind::Doppler, range_m: 35_000.0, sweep_s: 2.0 },
        radar_cone_rad: deg(60.0),
        toughness: 1.0,
        gun: &GSH_30_1,
        missile: WeaponKind::Alamo,
        evasions: &[Evasion::Beam, Evasion::Dive],
    },
];

impl FighterKind {
    pub fn spec(self) -> &'static FighterSpec {
        &SPECS[self as usize]
    }
}

/// Was ein Jäger gerade tut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Ready,
    Takeoff,
    Intercept,
    Pursue,
    Patrol,
    Return,
    /// Abgeschossen: stürzt ab.
    Falling,
    /// Am Boden zerschellt: aus dem Spiel.
    Wreck,
}

/// Ein feindlicher Jäger.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fighter {
    pub id: u32,
    pub kind: FighterKind,
    pub skill: Skill,
    /// Heimatplatz (Index in `Terrain::runways`).
    pub base: usize,
    pub mode: Mode,
    /// NED (m).
    pub position: DVec3,
    /// Kurs (rad, rechtweisend), Nicklage der Flugbahn und Querlage (rad).
    pub heading: f64,
    pub pitch: f64,
    pub bank: f64,
    pub speed: f64,
    /// Zeit in der Luft (s).
    pub flight_s: f64,
    /// Ziel des Flugs (NED).
    pub goal: DVec3,
    /// Tick des letzten eigenen Radartreffers.
    pub contact_tick: Option<u64>,
    /// Summe der erhaltenen Trefferstärken.
    pub damage: f64,
    /// Beginn des letzten Feuerstoßes (Tick).
    pub burst_tick: Option<u64>,
    /// Restliche Täuschkörper und Tick des letzten Wurfs.
    pub decoys: u8,
    pub decoy_tick: Option<u64>,
    /// Nächste Rakete, die ihn verfolgt und nah ist (Ort, Nummer, passender
    /// Täuschkörper); vom Sim zu Beginn jedes Ticks gesetzt.
    pub threat: Option<(DVec3, u32, DecoyKind)>,
}

impl Fighter {
    /// Bereit am Anfang der Bahn seines Platzes.
    fn parked(id: u32, kind: FighterKind, skill: Skill, base: usize, terrain: &Terrain) -> Self {
        let rw = &terrain.runways()[base];
        let start = rw.world(-rw.length_m * 0.5 + 150.0, 0.0);
        let position = DVec3::new(start[0], start[1], -rw.elevation_m);
        Self {
            id,
            kind,
            skill,
            base,
            mode: Mode::Ready,
            position,
            heading: detmath::atan2(rw.dir[1], rw.dir[0]),
            pitch: 0.0,
            bank: 0.0,
            speed: 0.0,
            flight_s: 0.0,
            goal: position,
            contact_tick: None,
            damage: 0.0,
            burst_tick: None,
            decoys: DECOYS,
            decoy_tick: None,
            threat: None,
        }
    }

    pub fn spec(&self) -> &'static FighterSpec {
        self.kind.spec()
    }

    /// In der Luft und im Kampf (Ziel, Radar, Karte).
    pub fn airborne(&self) -> bool {
        !matches!(self.mode, Mode::Ready | Mode::Falling | Mode::Wreck)
    }

    /// In der Luft, auch im Absturz (Darstellung).
    pub fn in_air(&self) -> bool {
        self.airborne() || self.mode == Mode::Falling
    }

    /// Geschwindigkeit (NED, m/s).
    pub fn velocity(&self) -> DVec3 {
        let (sh, ch) = detmath::sin_cos(self.heading);
        let (sp, cp) = detmath::sin_cos(self.pitch);
        DVec3::new(ch * cp, sh * cp, -sp) * self.speed
    }

    /// Lage (Kurs, Nicklage, Querlage).
    pub fn attitude(&self) -> crate::math::DQuat {
        crate::math::DQuat::from_euler(self.heading, self.pitch, self.bank)
    }

    /// Sein Radar hat das Flugzeug beim letzten Umlauf erfasst: Er
    /// beleuchtet es für seine Rakete.
    pub fn painting(&self, tick: u64, dt: f64) -> bool {
        let sweep = (self.spec().radar.sweep_s / dt) as u64;
        self.airborne() && self.contact_tick.is_some_and(|t| tick.saturating_sub(t) <= sweep)
    }

    fn sees(&self, at: DVec3) -> bool {
        let to = at - self.position;
        let bearing = detmath::atan2(to.y, to.x);
        detmath::wrap_pi(bearing - self.heading).abs() <= self.spec().radar_cone_rad
    }

    /// Fliegt einen Zeitschritt auf `goal` zu (Höhe `goal.z`, mindestens
    /// [`MIN_AGL_M`] über Grund). Auf Patrouille nach dem Überflug erst ein
    /// Stück geradeaus, dann zurück.
    fn fly(&mut self, terrain: &Terrain, dt: f64) {
        let s = *self.spec();
        let to = self.goal - self.position;
        let mut err = detmath::wrap_pi(detmath::atan2(to.y, to.x) - self.heading);
        let ground = terrain.elevation(self.position.x, self.position.y);
        let mut want_alt = (-self.goal.z).max(ground + MIN_AGL_M);
        let mut outbound = self.mode == Mode::Patrol
            && err.abs() > std::f64::consts::FRAC_PI_2
            && DVec3::new(to.x, to.y, 0.0).length() < PATROL_LEG_M;
        // Ausweichen: auf den näheren der beiden Kurse querab zur Rakete.
        if let Some((m, id, _)) = self.threat {
            let away = self.position - m;
            let b = detmath::atan2(away.y, away.x);
            let side = |h: f64| detmath::wrap_pi(h - self.heading);
            let (l, r) = (side(b + std::f64::consts::FRAC_PI_2), side(b - std::f64::consts::FRAC_PI_2));
            err = if l.abs() < r.abs() { l } else { r };
            outbound = false;
            if s.evasions[id as usize % s.evasions.len()] == Evasion::Dive {
                want_alt = ground + MIN_AGL_M;
            }
        }
        let want = if outbound { 0.0 } else { (err * 2.0).clamp(-s.max_bank_rad, s.max_bank_rad) };
        self.bank += (want - self.bank).clamp(-s.roll_rate * dt, s.roll_rate * dt);
        self.heading = detmath::wrap_pi(self.heading + G0 * detmath::tan(self.bank) / self.speed * dt);
        let alt = -self.position.z;
        let vs = ((want_alt - alt) * 0.2).clamp(-s.climb_mps, s.climb_mps);
        self.pitch = detmath::asin((vs / self.speed).clamp(-0.5, 0.5));
        self.speed += (s.cruise_mps - self.speed).clamp(-10.0 * dt, 10.0 * dt);
        self.position += self.velocity() * dt;
        self.flight_s += dt;
    }

    /// Absturz: rollt ab und senkt die Nase; am Boden zerschellt er.
    fn fall(&mut self, terrain: &Terrain, dt: f64) {
        self.bank = (self.bank + dt).min(1.2);
        self.pitch = (self.pitch - 0.3 * dt).max(-1.0);
        self.heading = detmath::wrap_pi(self.heading + G0 * detmath::tan(self.bank) / self.speed * dt);
        self.position += self.velocity() * dt;
        if -self.position.z <= terrain.elevation(self.position.x, self.position.y) {
            self.mode = Mode::Wreck;
        }
    }
}

/// Ereignisse der Jäger (Meldungen).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FighterEvent {
    Takeoff {
        kind: FighterKind,
        base: usize,
    },
    /// Das eigene Radar eines Jägers (Nummer `id`) hat das Flugzeug erfasst.
    Contact {
        kind: FighterKind,
        id: u32,
    },
    Landed {
        kind: FighterKind,
        base: usize,
    },
    /// Abgeschossen (stürzt ab).
    Destroyed {
        kind: FighterKind,
        id: u32,
    },
}

/// Alle feindlichen Jäger.
#[derive(Debug, Clone, PartialEq)]
pub struct Fighters {
    pub list: Vec<Fighter>,
    /// Kanone je Jäger (gleicher Index).
    pub guns: Vec<Gun>,
    /// Höchstens so viele gleichzeitig in der Luft (Regelset).
    pub max_airborne: usize,
    /// Letzter Radarkontakt eines Jägers (Ort, Tick).
    pub contact: Option<(DVec3, u64)>,
    pub events: EventLog<FighterEvent>,
}

impl Fighters {
    /// Die Jäger der feindlichen Plätze laut Gebiet ([`Runway::fighters`]:
    /// MiG-21, MiG-29) bereit, höchstens `max_airborne` gleichzeitig in der
    /// Luft; das gewürfelte Können um `skill_shift` Stufen verschoben
    /// (Gegnerqualität).
    pub fn new(terrain: &Terrain, seed: u64, max_airborne: usize, skill_shift: i8) -> Self {
        let mut rng = Pcg32::new(seed, STREAM_SKILL);
        let mut list = Vec::new();
        for (base, rw) in terrain.runways().iter().enumerate().filter(|(_, r)| r.kind == AirfieldKind::Enemy)
        {
            let [mig21, mig29] = rw.fighters;
            let kinds = std::iter::repeat_n(FighterKind::MiG21, mig21.into())
                .chain(std::iter::repeat_n(FighterKind::MiG29, mig29.into()));
            for kind in kinds {
                let skill = Skill::roll(&mut rng, skill_shift);
                list.push(Fighter::parked(list.len() as u32, kind, skill, base, terrain));
            }
        }
        let guns = (0..list.len() as u64)
            .map(|k| Gun::unlimited(list[k as usize].spec().gun, seed, STREAM_GUNS + k))
            .collect();
        Self { list, guns, max_airborne, contact: None, events: EventLog::default() }
    }

    /// Ein Tick. `alert` ist der jüngste Ping-Ort der Radarstellungen,
    /// `exposure` `None`, wenn das Flugzeug nicht mehr fliegt.
    pub fn step(
        &mut self,
        tick: u64,
        dt: f64,
        terrain: &Terrain,
        alert: Option<DVec3>,
        x: Option<&Exposure>,
    ) {
        let fresh = (CONTACT_S / dt) as u64;
        let own = self.contact.filter(|(_, t)| tick.saturating_sub(*t) <= fresh).map(|(p, _)| p);
        let target = own.or(alert);
        self.scramble(tick, target);
        for k in 0..self.list.len() {
            let f = &mut self.list[k];
            if f.mode == Mode::Falling {
                f.fall(terrain, dt);
                continue;
            }
            if !f.airborne() {
                continue;
            }
            let rw = &terrain.runways()[f.base];
            let home = DVec3::new(rw.center[0], rw.center[1], -rw.elevation_m - 1_000.0);
            let seen = f.contact_tick.is_some_and(|t| tick.saturating_sub(t) <= fresh);
            if f.mode == Mode::Takeoff {
                f.speed += TAKEOFF_ACCEL * dt;
                if f.speed >= ROTATE_MPS {
                    f.mode = Mode::Intercept;
                    f.goal = f.position - DVec3::new(0.0, 0.0, 1_000.0);
                } else {
                    f.position += f.velocity() * dt;
                    continue;
                }
            }
            if f.flight_s > f.spec().endurance_s {
                f.mode = Mode::Return;
            }
            match f.mode {
                Mode::Return => {
                    f.goal = home;
                    let d = home - f.position;
                    if DVec3::new(d.x, d.y, 0.0).length() < ARRIVE_M {
                        *f = Fighter::parked(f.id, f.kind, f.skill, f.base, terrain);
                        self.events.push(tick, FighterEvent::Landed { kind: f.kind, base: f.base });
                        continue;
                    }
                }
                _ if seen => {
                    f.mode = Mode::Pursue;
                    f.goal = x.map_or(f.goal, |x| x.position);
                }
                _ => {
                    // Kontakt verloren oder neuer Kontaktort: abfangen;
                    // am Ziel angekommen: kreisen.
                    let fresh_point =
                        target.filter(|p| f.mode == Mode::Pursue || (*p - f.goal).length() > ARRIVE_M * 2.0);
                    if let Some(p) = fresh_point {
                        f.goal = p;
                    }
                    if fresh_point.is_some() || f.mode == Mode::Pursue {
                        f.mode = Mode::Intercept;
                    }
                    let d = f.goal - f.position;
                    if f.mode == Mode::Intercept && DVec3::new(d.x, d.y, 0.0).length() < ARRIVE_M {
                        f.mode = Mode::Patrol;
                    }
                }
            }
            f.fly(terrain, dt);
            // Eigenes Radar, Umlauf je Jäger versetzt
            let sweep = (f.spec().radar.sweep_s / dt) as u64;
            if let Some(x) = x
                && (tick + u64::from(f.id) * 37).is_multiple_of(sweep)
                && f.sees(x.position)
                && x.signal(&f.spec().radar, f.position) > x.stealth()
            {
                if !seen {
                    self.events.push(tick, FighterEvent::Contact { kind: f.kind, id: f.id });
                }
                f.contact_tick = Some(tick);
                self.contact = Some((x.position, tick));
            }
        }
    }

    /// Setzt Jäger `i` bei `position` mit Kurs `heading` in die Luft, im
    /// Abfangen geradeaus – Zugang für Tests.
    pub fn put_in_air(&mut self, i: usize, position: DVec3, heading: f64) {
        let f = &mut self.list[i];
        let (s, c) = detmath::sin_cos(heading);
        f.mode = Mode::Intercept;
        f.position = position;
        f.heading = heading;
        f.speed = f.spec().cruise_mps;
        f.goal = position + DVec3::new(c, s, 0.0) * 100_000.0;
    }

    /// Treffer der Stärke `strength` am Jäger `i` (Rakete 1); ab
    /// [`FighterSpec::toughness`] stürzt er ab.
    pub fn hit(&mut self, tick: u64, i: usize, strength: f64) {
        let Some(f) = self.list.get_mut(i).filter(|f| f.airborne()) else {
            return;
        };
        f.damage += strength;
        if f.damage >= f.spec().toughness {
            f.mode = Mode::Falling;
            self.events.push(tick, FighterEvent::Destroyed { kind: f.kind, id: f.id });
        }
    }

    /// Startet bereitstehende Jäger, deren Platz den Kontaktort erreicht –
    /// je Platz einer, solange dort keiner rollt.
    fn scramble(&mut self, tick: u64, target: Option<DVec3>) {
        let Some(p) = target else {
            return;
        };
        let mut airborne = self.list.iter().filter(|f| f.airborne()).count();
        for k in 0..self.list.len() {
            let f = self.list[k];
            let rolling = self.list.iter().any(|o| o.base == f.base && o.mode == Mode::Takeoff);
            let reach = f.spec().cruise_mps * f.spec().endurance_s * 0.5;
            let d = p - f.position;
            if airborne >= self.max_airborne
                || f.mode != Mode::Ready
                || rolling
                || DVec3::new(d.x, d.y, 0.0).length() > reach
            {
                continue;
            }
            self.list[k].mode = Mode::Takeoff;
            self.events.push(tick, FighterEvent::Takeoff { kind: f.kind, base: f.base });
            airborne += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DT;

    const TICK: f64 = DT;

    fn setup() -> (Terrain, Fighters) {
        let terrain = Terrain::from_theater(f29_world::Theater::builtin());
        let f = Fighters::new(&terrain, 1, 6, 0);
        (terrain, f)
    }

    /// Voll sichtbares Flugzeug bei `at`, langsam nach Norden.
    fn visible(at: DVec3) -> Exposure {
        Exposure {
            position: at,
            velocity: DVec3::new(200.0, 0.0, 0.0),
            roll: 0.0,
            height_agl_m: 3_000.0,
            door: 0.0,
            radar_on: false,
            ground: 1.0,
            ecm: false,
            visible: true,
        }
    }

    fn run(
        f: &mut Fighters,
        terrain: &Terrain,
        seconds: f64,
        alert: Option<DVec3>,
        x: Option<&Exposure>,
    ) -> u64 {
        let ticks = (seconds / TICK) as u64;
        for t in 1..=ticks {
            f.step(t, TICK, terrain, alert, x);
        }
        ticks
    }

    #[test]
    fn fighters_of_the_theater_wait_for_an_alert() {
        let (terrain, mut f) = setup();
        let per_kind = |k: usize| terrain.runways().iter().map(|r| usize::from(r.fighters[k])).sum::<usize>();
        assert_eq!(f.list.len(), per_kind(0) + per_kind(1));
        for base in f.list.iter().map(|x| x.base) {
            assert_eq!(terrain.runways()[base].kind, AirfieldKind::Enemy);
        }
        assert_eq!(f.list.iter().filter(|x| x.kind == FighterKind::MiG29).count(), per_kind(1));
        run(&mut f, &terrain, 30.0, None, None);
        assert!(f.list.iter().all(|x| x.mode == Mode::Ready), "ohne Alarm bleibt alles am Boden");
    }

    /// Bei Alarm startet je Platz einer nach dem anderen, höchstens sechs
    /// fliegen gleichzeitig.
    #[test]
    fn alert_scrambles_one_after_another_up_to_the_limit() {
        let (terrain, mut f) = setup();
        let rw = &terrain.runways()[f.list[0].base];
        let alert = DVec3::new(rw.center[0] + 20_000.0, rw.center[1], -3_000.0);
        let mut max_rolling = 0;
        for t in 1..(300.0 / TICK) as u64 {
            f.step(t, TICK, &terrain, Some(alert), None);
            for base in f.list.iter().map(|x| x.base) {
                let rolling = f.list.iter().filter(|x| x.base == base && x.mode == Mode::Takeoff).count();
                max_rolling = max_rolling.max(rolling);
            }
            assert!(f.list.iter().filter(|x| x.airborne()).count() <= f.max_airborne);
        }
        assert_eq!(max_rolling, 1, "je Platz rollt nur einer");
        let in_reach = f
            .list
            .iter()
            .filter(|x| {
                let (a, c) = (alert, terrain.runways()[x.base].center);
                (a.x - c[0]).hypot(a.y - c[1]) <= x.spec().cruise_mps * x.spec().endurance_s * 0.5
            })
            .count();
        assert_eq!(f.list.iter().filter(|x| x.airborne()).count(), f.max_airborne.min(in_reach));
        assert!(f.list.iter().all(|x| -x.position.z > terrain.elevation(x.position.x, x.position.y) - 1.0));
    }

    /// Abfangen zum Kontaktort, dort kreisen; sieht das Radar das Flugzeug,
    /// verfolgen und Kontakt melden.
    #[test]
    fn fighters_intercept_find_and_pursue() {
        let (terrain, mut f) = setup();
        let rw = &terrain.runways()[f.list[0].base];
        let (n, e) = (rw.center[0] + 20_000.0, rw.center[1] + 5_000.0);
        let at = DVec3::new(n, e, -terrain.elevation(n, e) - 3_000.0);
        let near = |f: &Fighters| {
            f.list
                .iter()
                .filter(|x| x.airborne())
                .map(|x| (x.position - at).length())
                .fold(f64::MAX, f64::min)
        };
        // Am Kontaktort gewesen (danach Rennbahn mit langen Schenkeln).
        let mut closest = f64::MAX;
        for t in 1..=(150.0 / TICK) as u64 {
            f.step(t, TICK, &terrain, Some(at), None);
            closest = closest.min(near(&f));
        }
        assert!(closest < ARRIVE_M, "nicht am Kontaktort: {closest:.0} m");
        assert!(f.list.iter().any(|x| x.mode == Mode::Patrol), "patrouilliert nicht");
        let x = visible(at);
        let t0 = (150.0 / TICK) as u64;
        // Eine Rennbahn lang (wenden außerhalb der Radarreichweite): Die
        // Jäger verfolgen und kommen heran.
        let mut pursued = f64::MAX;
        for t in t0..t0 + (120.0 / TICK) as u64 {
            f.step(t, TICK, &terrain, Some(at), Some(&x));
            if f.list.iter().any(|x| x.mode == Mode::Pursue) {
                pursued = pursued.min(near(&f));
            }
        }
        assert!(pursued < 3_000.0, "verfolgt nicht oder nicht dran: {pursued:.0} m");
        assert!(f.events.iter().any(|(_, e)| matches!(e, FighterEvent::Contact { .. })));
        assert!(f.contact.is_some());
    }

    /// Ist die Flugdauer um, fliegt der Jäger heim, landet und steht wieder
    /// bereit.
    #[test]
    fn fighters_return_and_land_when_their_time_is_up() {
        let (terrain, mut f) = setup();
        let rw = &terrain.runways()[f.list[0].base];
        let at = Some(DVec3::new(rw.center[0] + 15_000.0, rw.center[1], -3_000.0));
        run(&mut f, &terrain, 60.0, at, None);
        let k = f.list.iter().position(|x| x.airborne() && x.mode != Mode::Takeoff).expect("einer fliegt");
        f.list[k].flight_s = f.list[k].spec().endurance_s + 1.0;
        let mut t = (60.0 / TICK) as u64;
        while f.list[k].airborne() && t < (400.0 / TICK) as u64 {
            t += 1;
            f.step(t, TICK, &terrain, None, None);
        }
        assert_eq!(f.list[k].mode, Mode::Ready, "nicht gelandet");
        assert!(f.events.iter().any(|(_, e)| matches!(e, FighterEvent::Landed { .. })));
    }

    /// Die Kanone braucht je Typ ihre Treffer, eine Rakete einen; der
    /// Abgeschossene stürzt ab, bleibt als Wrack liegen und startet nie wieder.
    #[test]
    fn hits_shoot_fighters_down_for_good() {
        let (terrain, mut f) = setup();
        let rw = &terrain.runways()[f.list[0].base];
        let alert = Some(DVec3::new(rw.center[0] + 15_000.0, rw.center[1], -3_000.0));
        run(&mut f, &terrain, 120.0, alert, None);
        let up: Vec<usize> = (0..f.list.len()).filter(|&k| f.list[k].airborne()).collect();
        let (gun, missile) = (up[0], up[1]);
        let need = match f.list[gun].kind {
            FighterKind::MiG21 => 5,
            FighterKind::MiG29 => 8,
        };
        for k in 0..need {
            assert!(f.list[gun].airborne(), "nach {k} Treffern schon unten");
            f.hit(1, gun, crate::gun::M61A1.strength);
        }
        assert_eq!(f.list[gun].mode, Mode::Falling);
        f.hit(1, missile, 1.0);
        assert_eq!(f.list[missile].mode, Mode::Falling, "Rakete zerstört sofort");
        assert!(f.events.iter().any(|(_, e)| matches!(e, FighterEvent::Destroyed { .. })));
        let t0 = (120.0 / TICK) as u64;
        for t in t0..t0 + (120.0 / TICK) as u64 {
            f.step(t, TICK, &terrain, alert, None);
        }
        assert_eq!(f.list[gun].mode, Mode::Wreck, "nicht aufgeschlagen");
        assert_eq!(f.list[missile].mode, Mode::Wreck);
        assert!(f.list.iter().filter(|x| x.airborne()).count() <= f.max_airborne);
    }

    /// Das Können ist gewürfelt und folgt der Gegnerqualität: Green mehr
    /// Anfänger, Veteran mehr Asse, Elite nur Asse.
    #[test]
    fn pilot_skill_follows_the_opponents() {
        use crate::rules::Opponents::{Elite, Green, Veteran};
        let terrain = Terrain::from_theater(f29_world::Theater::builtin());
        let count = |o: crate::rules::Opponents, s: Skill| {
            (0..20u64)
                .map(|seed| {
                    Fighters::new(&terrain, seed, 6, o.skill_shift())
                        .list
                        .iter()
                        .filter(|f| f.skill == s)
                        .count()
                })
                .sum::<usize>()
        };
        assert!(count(Veteran, Skill::Ace) > count(Green, Skill::Ace));
        assert!(count(Green, Skill::Rookie) > count(Veteran, Skill::Rookie));
        assert!(count(Green, Skill::Regular) > 0 && count(Veteran, Skill::Regular) > 0);
        assert_eq!(count(Elite, Skill::Rookie) + count(Elite, Skill::Regular), 0);
    }

    /// Tief und waagerecht bleibt das Flugzeug für das Jägerradar unsichtbar
    /// (gleiches Stealth-Level wie gegen Radarstellungen); hinter dem
    /// Jäger sieht ihn sein Radar ohnehin nicht.
    #[test]
    fn fighter_radar_respects_stealth_and_its_cone() {
        let (terrain, f) = setup();
        let mut j = f.list[3];
        j.mode = Mode::Patrol;
        j.position = DVec3::new(0.0, 0.0, -3_000.0);
        j.heading = 0.0;
        let ahead = DVec3::new(10_000.0, 0.0, -3_000.0);
        let x = visible(ahead);
        assert!(j.sees(ahead) && x.signal(&j.spec().radar, j.position) > x.stealth());
        assert!(!j.sees(DVec3::new(-10_000.0, 0.0, -3_000.0)), "hinten blind");
        let low = Exposure { visible: false, height_agl_m: 50.0, ..x };
        assert!(x.signal(&j.spec().radar, j.position) <= low.stealth() + 1.0);
        assert!(low.signal(&j.spec().radar, j.position) < low.stealth(), "tief gesehen");
        let _ = terrain;
    }
}
