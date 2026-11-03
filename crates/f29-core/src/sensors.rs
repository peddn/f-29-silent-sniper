//! Eigene Sensoren und Zielwahl (M-013).
//!
//! Stufen eines Ziels: **gemeldet** (nur AWACS), **entdeckt** (eigenes Radar
//! oder optisches System, rundum, für Boden- wie Luftziele gleich),
//! **gewählt** (je Zielart eines, [`Pick`]) und **erfasst** (von der Waffe).
//!
//! Gewählt wird automatisch das **nächste** entdeckte Ziel in einem Kegel
//! um die Blickrichtung (voraus, rechts, hinten, links). Die Wahl bleibt,
//! solange das Ziel entdeckt ist – auch wenn es den Kegel verlässt: Die
//! Blickrichtung folgt ihm. Erneutes Wählen der aktuellen Richtung schaltet
//! zum nächstweiteren Ziel weiter.
//! - **Radar:** große Reichweite, ohne Sichtlinie – es „sieht“ durch Berge.
//!   Abschaltbar; eingeschaltet erhöht es die eigene Sichtbarkeit (M-020).
//! - **Optisch:** kürzere Reichweite und nur mit freier Sichtlinie, verrät
//!   das Flugzeug aber nicht. Nur ein optisch erfasstes Ziel liefert ein Bild.
//!
//! Die Wahl wird mit [`UPDATE_TICKS`] geprüft (nicht jeden Tick), damit die
//! Sichtlinienprüfung gegen das Gelände wenig kostet; ein Tastendruck oder
//! das Schalten des Radars wirkt sofort.

use crate::detmath;
use crate::math::{DVec3, deg};
use f29_world::Terrain;

/// Reichweite des Radars (m, Spielerentscheidung v0.2.7), Tag wie Nacht.
pub const RADAR_RANGE_M: f64 = 50_000.0;
/// Reichweite des optischen Systems bei Tag (m, Spielerentscheidung v0.2.7).
pub const OPTICAL_RANGE_M: f64 = 50_000.0;
/// Nachts schrumpft die optische Reichweite auf diesen Anteil (20 km) …
pub const NIGHT_OPTICAL_FACTOR: f64 = 0.4;
/// … das Radar verliert nichts.
pub const NIGHT_RADAR_FACTOR: f64 = 1.0;
/// Halber Öffnungswinkel des Erfassungskegels um die Blickrichtung (rad, 45°).
pub const CONE_HALF_RAD: f64 = std::f64::consts::FRAC_PI_4;
/// Erfassungskreis um die Nase (halber Öffnungswinkel, rad): Liegt das
/// gewählte Ziel darin, ist es erfasst und wird an eine Rakete übergeben –
/// eine Regel, die Größe je Zielart (Spielerentscheidung v0.2.9).
pub const AIR_CIRCLE_RAD: f64 = deg(10.0);
pub const GROUND_CIRCLE_RAD: f64 = deg(6.0);
/// Erfassung alle so viele Ticks (4 Hz).
pub const UPDATE_TICKS: u64 = 30;
/// Abstand der Prüfpunkte entlang der Sichtlinie (m).
const LOS_STEP_M: f64 = 60.0;
/// Die letzten Meter vor dem Ziel zählen nicht (Ziel steht selbst am Boden).
const LOS_TARGET_CLEARANCE_M: f64 = 60.0;

/// Blickrichtung der Erfassung relativ zum Kurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum LookDir {
    #[default]
    Forward,
    Right,
    Aft,
    Left,
}

impl LookDir {
    pub const ALL: [LookDir; 4] = [LookDir::Forward, LookDir::Right, LookDir::Aft, LookDir::Left];

    /// Winkel zum Kurs (rad, im Uhrzeigersinn).
    pub fn offset(self) -> f64 {
        use std::f64::consts::{FRAC_PI_2, PI};
        match self {
            LookDir::Forward => 0.0,
            LookDir::Right => FRAC_PI_2,
            LookDir::Aft => PI,
            LookDir::Left => -FRAC_PI_2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LookDir::Forward => "FWD",
            LookDir::Right => "RIGHT",
            LookDir::Aft => "AFT",
            LookDir::Left => "LEFT",
        }
    }

    /// Kennung im Steuerkanal (1..4; 0 = keine Änderung).
    pub fn code(self) -> u8 {
        match self {
            LookDir::Forward => 1,
            LookDir::Right => 2,
            LookDir::Aft => 3,
            LookDir::Left => 4,
        }
    }

    /// Richtung, deren Kegel die relative Peilung `rel` (rad, zum Kurs) enthält.
    pub fn from_relative_bearing(rel: f64) -> Self {
        let deg = detmath::wrap_pi(rel).to_degrees();
        if deg.abs() <= 45.0 {
            LookDir::Forward
        } else if deg.abs() >= 135.0 {
            LookDir::Aft
        } else if deg > 0.0 {
            LookDir::Right
        } else {
            LookDir::Left
        }
    }

    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(LookDir::Forward),
            2 => Some(LookDir::Right),
            3 => Some(LookDir::Aft),
            4 => Some(LookDir::Left),
            _ => None,
        }
    }
}

/// Welches eigene System ein Ziel entdeckt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Seen {
    pub radar: bool,
    /// Optisch (Reichweite und freie Sicht): Zielbild verfügbar.
    pub optical: bool,
}

impl Seen {
    pub fn any(self) -> bool {
        self.radar || self.optical
    }

    pub fn bits(self) -> u64 {
        u64::from(self.optical) | u64::from(self.radar) << 1
    }
}

/// Auswahl einer Zielart (Bodenziele, Jäger, Landebahnen) mit eigener
/// Blickrichtung: das gewählte Ziel und wie es entdeckt ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pick {
    pub look: LookDir,
    pub index: Option<usize>,
    pub seen: Seen,
}

impl Pick {
    /// Richtungstaste `look` über die Ziele `points` (Index, Lage): Dieselbe
    /// Richtung, in der das gewählte Ziel gerade liegt, schaltet zum
    /// nächstweiteren entdeckten, eine neue wählt das nächste dort.
    pub fn press(
        &mut self,
        look: LookDir,
        points: &[(usize, DVec3)],
        pos: DVec3,
        heading: f64,
        sees: impl Fn(DVec3) -> Seen,
    ) {
        self.follow(points, pos, heading);
        let c = in_cone(points, pos, heading, look);
        let start = self.index.filter(|_| look == self.look).and_then(|i| c.iter().position(|x| x.1 == i));
        self.look = look;
        self.choose(&c, start.map_or(0, |k| k + 1), sees);
    }

    /// Hält die Wahl, solange sie entdeckt ist, und dreht die Blickrichtung
    /// mit ihr mit (etwa beim Überflug von voraus nach hinten); sonst das
    /// nächste entdeckte Ziel der Blickrichtung.
    pub fn update(
        &mut self,
        points: &[(usize, DVec3)],
        pos: DVec3,
        heading: f64,
        sees: impl Fn(DVec3) -> Seen,
    ) {
        if let Some(&(_, p)) = self.index.and_then(|i| points.iter().find(|q| q.0 == i)) {
            self.seen = sees(p);
            if self.seen.any() {
                self.follow(points, pos, heading);
                return;
            }
        }
        self.choose(&in_cone(points, pos, heading, self.look), 0, sees);
    }

    fn follow(&mut self, points: &[(usize, DVec3)], pos: DVec3, heading: f64) {
        if let Some(&(_, p)) = self.index.and_then(|i| points.iter().find(|q| q.0 == i)) {
            let to = p - pos;
            self.look = LookDir::from_relative_bearing(detmath::atan2(to.y, to.x) - heading);
        }
    }

    /// Das erste entdeckte der Kandidaten `c` ab `start` (reihum).
    fn choose(&mut self, c: &[(f64, usize, DVec3)], start: usize, sees: impl Fn(DVec3) -> Seen) {
        let n = c.len();
        let found =
            (0..n).map(|k| c[(start + k) % n]).find_map(|(_, i, p)| Some((i, sees(p))).filter(|x| x.1.any()));
        (self.index, self.seen) = found.map_or((None, Seen::default()), |(i, seen)| (Some(i), seen));
    }
}

/// Punkte im Kegel der Blickrichtung `look` (Kurs `heading`), nach
/// Entfernung sortiert: (Entfernung, Index, Lage).
pub fn in_cone(
    points: &[(usize, DVec3)],
    pos: DVec3,
    heading: f64,
    look: LookDir,
) -> Vec<(f64, usize, DVec3)> {
    let look = heading + look.offset();
    let mut c: Vec<_> = points
        .iter()
        .filter_map(|&(i, p)| {
            let to = p - pos;
            let off = detmath::wrap_pi(detmath::atan2(to.y, to.x) - look).abs();
            (off <= CONE_HALF_RAD).then_some((to.length(), i, p))
        })
        .collect();
    c.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    c
}

/// Zustand der eigenen Sensoren.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sensors {
    pub radar_on: bool,
    /// Leistung des Radars 0..1 (Schadensmodell, [`crate::systems`]):
    /// Faktor auf die Reichweite, 0 = ausgefallen.
    pub radar_capability: f64,
    /// Leistung des optischen Systems 0..1, wie oben.
    pub optics_capability: f64,
    /// Nachtflug: optische Reichweite verringert.
    pub night: bool,
}

impl Default for Sensors {
    fn default() -> Self {
        Self { radar_on: false, radar_capability: 1.0, optics_capability: 1.0, night: false }
    }
}

/// Freie Sicht von `from` nach `to` (Nord, Ost, Höhe über NN)? `height`
/// liefert die Gelände- bzw. Wasseroberfläche.
pub fn line_of_sight(height: impl Fn(f64, f64) -> f64, from: [f64; 3], to: [f64; 3]) -> bool {
    let (dn, de) = (to[0] - from[0], to[1] - from[1]);
    let dist = (dn * dn + de * de).sqrt();
    let usable = dist - LOS_TARGET_CLEARANCE_M;
    if usable <= 0.0 {
        return true;
    }
    let steps = (usable / LOS_STEP_M).ceil().clamp(1.0, 1_000.0) as usize;
    for i in 1..=steps {
        let d = usable * i as f64 / steps as f64;
        let t = d / dist;
        let line = from[2] + (to[2] - from[2]) * t;
        if height(from[0] + dn * t, from[1] + de * t) > line {
            return false;
        }
    }
    true
}

impl Sensors {
    /// Entdecken die eigenen Sensoren einen Punkt (rundum, für jede
    /// Zielart)? Radar (falls an) in Reichweite; optisch in Reichweite mit
    /// freier Sicht. Ziele, die das nicht erfüllen, kennt das Flugzeug nur
    /// über den AWACS-Datalink.
    pub fn sees(&self, terrain: &Terrain, pos: DVec3, at: DVec3) -> Seen {
        let range = (at - pos).length();
        Seen {
            radar: self.radar_on && range <= self.radar_range(),
            optical: range <= self.optical_range()
                && line_of_sight(|n, e| terrain.elevation(n, e), [pos.x, pos.y, -pos.z], [at.x, at.y, -at.z]),
        }
    }

    /// Radarreichweite unter Berücksichtigung von Schäden (m).
    pub fn radar_range(&self) -> f64 {
        let night = if self.night { NIGHT_RADAR_FACTOR } else { 1.0 };
        RADAR_RANGE_M * night * self.radar_capability.clamp(0.0, 1.0)
    }

    /// Optische Reichweite unter Berücksichtigung von Schäden (m).
    pub fn optical_range(&self) -> f64 {
        let night = if self.night { NIGHT_OPTICAL_FACTOR } else { 1.0 };
        OPTICAL_RANGE_M * night * self.optics_capability.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_of_sight_is_blocked_by_a_ridge() {
        // Grat von 500 m Höhe bei Nord = 1000 m.
        let ridge = |n: f64, _e: f64| if (n - 1_000.0).abs() < 50.0 { 500.0 } else { 0.0 };
        assert!(!line_of_sight(ridge, [0.0, 0.0, 100.0], [2_000.0, 0.0, 5.0]));
        assert!(line_of_sight(ridge, [0.0, 0.0, 1_200.0], [2_000.0, 0.0, 5.0]));
        // Ziel am Boden, flaches Gelände: frei.
        assert!(line_of_sight(|_, _| 0.0, [0.0, 0.0, 50.0], [5_000.0, 0.0, 2.0]));
    }

    /// Radar (an) rundum weit, optisch nur mit freier Sicht, nachts kürzer.
    #[test]
    fn radar_and_optics_see_any_point() {
        let terrain = Terrain::new(1);
        let pos = DVec3::new(0.0, 0.0, -9_000.0);
        let at = |km: f64| DVec3::new(-km * 1_000.0, 0.0, -9_000.0);
        let mut s = Sensors::default();
        assert_eq!(s.sees(&terrain, pos, at(40.0)), Seen { radar: false, optical: true });
        assert!(!s.sees(&terrain, pos, at(60.0)).any(), "ohne Radar zu weit");
        s.radar_on = true;
        assert_eq!(s.sees(&terrain, pos, at(45.0)), Seen { radar: true, optical: true });
        s.radar_on = false;
        s.night = true;
        assert!(!s.sees(&terrain, pos, at(30.0)).any(), "nachts zu weit");
    }

    /// Dieselbe Richtung schaltet zum nächstweiteren, eine neue Richtung
    /// beginnt beim nächsten.
    #[test]
    fn pick_cycles_by_range() {
        let ahead = |i: usize, km: f64| (i, DVec3::new(km * 1_000.0, 0.0, 0.0));
        let points = [ahead(9, 3.0), ahead(7, 1.0), ahead(3, 2.0), ahead(5, 4.0)];
        let sees = |p: DVec3| Seen { radar: p.x < 3_500.0, optical: false };
        let mut p = Pick::default();
        let press = |p: &mut Pick, look| p.press(look, &points, DVec3::ZERO, 0.0, sees);
        press(&mut p, LookDir::Forward);
        assert_eq!(p.index, Some(7));
        press(&mut p, LookDir::Forward);
        assert_eq!(p.index, Some(3));
        press(&mut p, LookDir::Forward);
        assert_eq!(p.index, Some(9));
        press(&mut p, LookDir::Forward);
        assert_eq!(p.index, Some(7), "nicht entdeckte übersprungen, dann von vorn");
        press(&mut p, LookDir::Right);
        assert_eq!(p.index, None, "rechts nichts");
        p.look = LookDir::Forward;
        p.update(&points, DVec3::ZERO, 0.0, sees);
        assert_eq!(p.index, Some(7), "ohne Wahl automatisch das nächste");
    }

    #[test]
    fn look_codes_roundtrip() {
        for d in LookDir::ALL {
            assert_eq!(LookDir::from_code(d.code()), Some(d));
        }
        assert_eq!(LookDir::from_code(0), None);
    }
}
