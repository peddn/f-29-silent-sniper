//! Feindliche Jäger (M-022), Etappe 1: ohne Waffen.
//!
//! Wie im Original stehen die Jäger auf den feindlichen Fliegerhorsten bereit
//! und starten erst bei Alarm – wenn ein Radar am Boden das Flugzeug bis zur
//! Stufe „verfolgt“ erfasst hat oder ein Jäger es mit seinem eigenen Radar
//! sieht. Der Kontaktort muss vom Platz aus in Reichweite liegen. Höchstens
//! [`MAX_AIRBORNE`] fliegen gleichzeitig, je Platz startet einer nach dem
//! anderen.
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

use crate::atmosphere::G0;
use crate::detmath;
use crate::events::EventLog;
use crate::math::DVec3;
use crate::radar::{Exposure, RadarKind, RadarSpec};
use f29_terrain::{AirfieldKind, Terrain};

/// Jäger je feindlichem Platz.
pub const PER_BASE: usize = 5;
/// Höchstens so viele Jäger gleichzeitig in der Luft.
pub const MAX_AIRBORNE: usize = 6;
/// Ein Radarkontakt eines Jägers gilt so lange als frisch (s).
const CONTACT_S: f64 = 15.0;
/// Mindesthöhe über Grund im Flug (m).
const MIN_AGL_M: f64 = 250.0;
/// Am Kontaktort: so nah (m) gilt er als erreicht, dann Patrouille.
const ARRIVE_M: f64 = 3_000.0;
/// Patrouille: so weit (m) fliegt er nach dem Überflug geradeaus weiter.
const PATROL_LEG_M: f64 = 12_000.0;
/// Abhebefahrt (m/s) und Beschleunigung beim Rollen (m/s²).
const ROTATE_MPS: f64 = 90.0;
const TAKEOFF_ACCEL: f64 = 12.0;

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
}

const fn deg(d: f64) -> f64 {
    d * std::f64::consts::PI / 180.0
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
}

/// Ein feindlicher Jäger.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fighter {
    pub id: u32,
    pub kind: FighterKind,
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
}

impl Fighter {
    /// Bereit am Anfang der Bahn seines Platzes.
    fn parked(id: u32, kind: FighterKind, base: usize, terrain: &Terrain) -> Self {
        let rw = &terrain.runways()[base];
        let start = rw.world(-rw.length_m * 0.5 + 150.0, 0.0);
        let position = DVec3::new(start[0], start[1], -rw.elevation_m);
        Self {
            id,
            kind,
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
        }
    }

    pub fn spec(&self) -> &'static FighterSpec {
        self.kind.spec()
    }

    pub fn airborne(&self) -> bool {
        self.mode != Mode::Ready
    }

    /// Geschwindigkeit (NED, m/s).
    pub fn velocity(&self) -> DVec3 {
        let (sh, ch) = detmath::sin_cos(self.heading);
        let (sp, cp) = detmath::sin_cos(self.pitch);
        DVec3::new(ch * cp, sh * cp, -sp) * self.speed
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
        let err = detmath::wrap_pi(detmath::atan2(to.y, to.x) - self.heading);
        let outbound = self.mode == Mode::Patrol
            && err.abs() > std::f64::consts::FRAC_PI_2
            && DVec3::new(to.x, to.y, 0.0).length() < PATROL_LEG_M;
        let want = if outbound { 0.0 } else { (err * 2.0).clamp(-s.max_bank_rad, s.max_bank_rad) };
        self.bank += (want - self.bank).clamp(-s.roll_rate * dt, s.roll_rate * dt);
        self.heading = detmath::wrap_pi(self.heading + G0 * detmath::tan(self.bank) / self.speed * dt);
        let ground = terrain.elevation(self.position.x, self.position.y);
        let alt = -self.position.z;
        let want_alt = (-self.goal.z).max(ground + MIN_AGL_M);
        let vs = ((want_alt - alt) * 0.2).clamp(-s.climb_mps, s.climb_mps);
        self.pitch = detmath::asin((vs / self.speed).clamp(-0.5, 0.5));
        self.speed += (s.cruise_mps - self.speed).clamp(-10.0 * dt, 10.0 * dt);
        self.position += self.velocity() * dt;
        self.flight_s += dt;
    }
}

/// Ereignisse der Jäger (Meldungen).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FighterEvent {
    Takeoff {
        kind: FighterKind,
        base: usize,
    },
    /// Das eigene Radar eines Jägers hat das Flugzeug erfasst.
    Contact {
        kind: FighterKind,
    },
    Landed {
        kind: FighterKind,
        base: usize,
    },
}

/// Alle feindlichen Jäger.
#[derive(Debug, Clone, PartialEq)]
pub struct Fighters {
    pub list: Vec<Fighter>,
    /// Letzter Radarkontakt eines Jägers (Ort, Tick).
    pub contact: Option<(DVec3, u64)>,
    pub events: EventLog<FighterEvent>,
}

impl Fighters {
    /// [`PER_BASE`] Jäger auf jedem feindlichen Platz bereit: drei MiG-21,
    /// zwei MiG-29.
    pub fn new(terrain: &Terrain) -> Self {
        let mut list = Vec::new();
        for (base, _) in terrain.runways().iter().enumerate().filter(|(_, r)| r.kind == AirfieldKind::Enemy) {
            for k in 0..PER_BASE {
                let kind = if k < 3 { FighterKind::MiG21 } else { FighterKind::MiG29 };
                list.push(Fighter::parked(list.len() as u32, kind, base, terrain));
            }
        }
        Self { list, contact: None, events: EventLog::default() }
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
                        *f = Fighter::parked(f.id, f.kind, f.base, terrain);
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
                    self.events.push(tick, FighterEvent::Contact { kind: f.kind });
                }
                f.contact_tick = Some(tick);
                self.contact = Some((x.position, tick));
            }
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
            if airborne >= MAX_AIRBORNE
                || f.airborne()
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
        let terrain = Terrain::new(0x0F29);
        let f = Fighters::new(&terrain);
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
    fn two_bases_with_five_fighters_each_wait_for_an_alert() {
        let (terrain, mut f) = setup();
        assert_eq!(f.list.len(), 2 * PER_BASE);
        for base in f.list.iter().map(|x| x.base) {
            assert_eq!(terrain.runways()[base].kind, AirfieldKind::Enemy);
        }
        assert_eq!(f.list.iter().filter(|x| x.kind == FighterKind::MiG29).count(), 4);
        run(&mut f, &terrain, 30.0, None, None);
        assert!(f.list.iter().all(|x| x.mode == Mode::Ready), "ohne Alarm bleibt alles am Boden");
    }

    /// Bei Alarm startet je Platz einer nach dem anderen, höchstens sechs
    /// fliegen gleichzeitig.
    #[test]
    fn alert_scrambles_one_after_another_up_to_the_limit() {
        let (terrain, mut f) = setup();
        let rw = &terrain.runways()[f.list[0].base];
        let alert = Some(DVec3::new(rw.center[0] + 20_000.0, rw.center[1], -3_000.0));
        let mut max_rolling = 0;
        for t in 1..(300.0 / TICK) as u64 {
            f.step(t, TICK, &terrain, alert, None);
            for base in [f.list[0].base, f.list[PER_BASE].base] {
                let rolling = f.list.iter().filter(|x| x.base == base && x.mode == Mode::Takeoff).count();
                max_rolling = max_rolling.max(rolling);
            }
            assert!(f.list.iter().filter(|x| x.airborne()).count() <= MAX_AIRBORNE);
        }
        assert_eq!(max_rolling, 1, "je Platz rollt nur einer");
        assert_eq!(f.list.iter().filter(|x| x.airborne()).count(), MAX_AIRBORNE);
        assert!(f.list.iter().all(|x| -x.position.z > terrain.elevation(x.position.x, x.position.y) - 1.0));
    }

    /// Abfangen zum Kontaktort, dort kreisen; sieht das Radar das Flugzeug,
    /// verfolgen und Kontakt melden.
    #[test]
    fn fighters_intercept_find_and_pursue() {
        let (terrain, mut f) = setup();
        let rw = &terrain.runways()[f.list[0].base];
        let at = DVec3::new(rw.center[0] + 20_000.0, rw.center[1] + 5_000.0, -3_000.0);
        run(&mut f, &terrain, 150.0, Some(at), None);
        let near = |f: &Fighters| {
            f.list
                .iter()
                .filter(|x| x.airborne())
                .map(|x| (x.position - at).length())
                .fold(f64::MAX, f64::min)
        };
        assert!(near(&f) < 8_000.0, "nicht am Kontaktort: {:.0} m", near(&f));
        assert!(f.list.iter().any(|x| x.mode == Mode::Patrol), "patrouilliert nicht");
        let x = visible(at);
        let t0 = (150.0 / TICK) as u64;
        // Eine Rennbahn lang (wenden außerhalb der Radarreichweite).
        for t in t0..t0 + (120.0 / TICK) as u64 {
            f.step(t, TICK, &terrain, Some(at), Some(&x));
        }
        assert!(f.list.iter().any(|x| x.mode == Mode::Pursue), "verfolgt nicht");
        let near_now = near(&f);
        assert!(near_now < 10_000.0, "verfolgt, aber nicht dran: {near_now:.0} m");
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
