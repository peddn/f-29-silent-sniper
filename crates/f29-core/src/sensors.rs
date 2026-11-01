//! Zielerfassung (M-013): optisches System und Radar.
//!
//! Beide schalten automatisch auf das **nächste** intakte Ziel in einem
//! Kegel um die gewählte Blickrichtung auf (voraus, rechts, hinten, links).
//! Eine Aufschaltung bleibt bestehen, solange irgendein System das Ziel
//! erfasst – auch wenn es den Kegel verlässt: Die Blickrichtung folgt dem
//! Ziel (etwa beim Überflug von voraus nach hinten). Erneutes Wählen der
//! aktuellen Richtung schaltet zum nächstweiteren Ziel weiter.
//! - **Radar:** große Reichweite, ohne Sichtlinie – es „sieht“ durch Berge.
//!   Abschaltbar; eingeschaltet erhöht es die eigene Sichtbarkeit (M-020).
//! - **Optisch:** kürzere Reichweite und nur mit freier Sichtlinie, verrät
//!   das Flugzeug aber nicht. Nur ein optisch erfasstes Ziel liefert ein Bild.
//!
//! Die Erfassung läuft mit [`UPDATE_TICKS`] (nicht jeden Tick), damit die
//! Sichtlinienprüfung gegen das Gelände wenig kostet; ein Wechsel der
//! Blickrichtung oder des Radars wirkt sofort.

use crate::detmath;
use crate::math::DVec3;
use crate::targets::Target;
use f29_terrain::Terrain;

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
pub const AIR_CIRCLE_RAD: f64 = 10.0 * std::f64::consts::PI / 180.0;
pub const GROUND_CIRCLE_RAD: f64 = 6.0 * std::f64::consts::PI / 180.0;
/// Optische Sichtweite gegen Jäger bei Tag bzw. Nacht (m).
pub const AIR_SIGHT_M: f64 = 15_000.0;
pub const AIR_SIGHT_NIGHT_M: f64 = 5_000.0;
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

/// Aufgeschaltetes Ziel und welches System es hält.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lock {
    /// Index in `Sim::targets`.
    pub target: usize,
    /// Optisch erfasst (Reichweite und freie Sicht): Zielbild verfügbar.
    pub optical: bool,
    /// Vom Radar erfasst.
    pub radar: bool,
}

/// Auswahl einer weiteren Zielart (Jäger, Landebahnen) mit eigener
/// Blickrichtung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pick {
    pub look: LookDir,
    pub index: Option<usize>,
}

impl Pick {
    /// Richtungstaste `look`: Dieselbe Richtung schaltet zum nächstweiteren
    /// Kandidaten, eine neue wählt den nächsten dort. `candidates` sind
    /// (Entfernung, Index), nach Entfernung sortiert ([`in_cone`]).
    pub fn press(&mut self, look: LookDir, candidates: &[(f64, usize)]) {
        let next =
            self.index.filter(|_| look == self.look).and_then(|i| candidates.iter().position(|c| c.1 == i));
        let start = next.map_or(0, |k| k + 1);
        self.look = look;
        self.index = (!candidates.is_empty()).then(|| candidates[start % candidates.len()].1);
    }
}

/// Punkte im Erfassungskegel der Blickrichtung `look` (Kurs `heading`) bis
/// `max_range`, nach Entfernung sortiert: (Entfernung, Index).
pub fn in_cone(
    points: impl IntoIterator<Item = (usize, DVec3)>,
    pos: DVec3,
    heading: f64,
    look: LookDir,
    max_range: f64,
) -> Vec<(f64, usize)> {
    let look = heading + look.offset();
    let mut c: Vec<(f64, usize)> = points
        .into_iter()
        .filter_map(|(i, p)| {
            let to = p - pos;
            let off = detmath::wrap_pi(detmath::atan2(to.y, to.x) - look).abs();
            (to.length() <= max_range && off <= CONE_HALF_RAD).then_some((to.length(), i))
        })
        .collect();
    c.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    c
}

/// Zustand der Zielerfassung.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sensors {
    pub radar_on: bool,
    pub look: LookDir,
    pub lock: Option<Lock>,
    /// Leistung des Radars 0..1 (Schadensmodell, [`crate::systems`]):
    /// Faktor auf die Reichweite, 0 = ausgefallen.
    pub radar_capability: f64,
    /// Leistung des optischen Systems 0..1, wie oben.
    pub optics_capability: f64,
    /// Nachtflug: beide Reichweiten stark verringert.
    pub night: bool,
}

impl Default for Sensors {
    fn default() -> Self {
        Self {
            radar_on: false,
            look: LookDir::Forward,
            lock: None,
            radar_capability: 1.0,
            optics_capability: 1.0,
            night: false,
        }
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
    /// Erfassen die eigenen Sensoren ein Ziel (ohne Kegel)? Radar (falls an)
    /// in Reichweite, oder optisch in Reichweite mit freier Sicht. Ziele, die
    /// das nicht erfüllen, kennt das Flugzeug nur über den AWACS-Datalink.
    pub fn sees(&self, t: &Target, terrain: &Terrain, pos: DVec3) -> bool {
        let c = t.center_ned();
        let range = (DVec3::new(c[0], c[1], c[2]) - pos).length();
        (self.radar_on && range <= self.radar_range())
            || (range <= self.optical_range()
                && line_of_sight(|n, e| terrain.elevation(n, e), [pos.x, pos.y, -pos.z], [c[0], c[1], -c[2]]))
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

    /// Kandidaten im Kegel der Blickrichtung und in Reichweite des größeren
    /// Systems, nach Entfernung sortiert: `(Entfernung, Index)`.
    fn candidates(&self, targets: &[Target], pos: DVec3, heading: f64) -> Vec<(f64, usize)> {
        let max_range =
            if self.radar_on { self.radar_range().max(self.optical_range()) } else { self.optical_range() };
        let intact = targets.iter().enumerate().filter(|(_, t)| t.is_intact()).map(|(i, t)| {
            let c = t.center_ned();
            (i, DVec3::new(c[0], c[1], c[2]))
        });
        in_cone(intact, pos, heading, self.look, max_range)
    }

    /// Sieht ein eigenes System einen Jäger bei `at`? Radar (falls an) in
    /// Reichweite, oder optisch bis zur Sichtweite mit freier Sicht.
    pub fn sees_aircraft(&self, terrain: &Terrain, pos: DVec3, at: DVec3) -> bool {
        let range = (at - pos).length();
        let sight =
            if self.night { AIR_SIGHT_NIGHT_M } else { AIR_SIGHT_M } * self.optics_capability.clamp(0.0, 1.0);
        (self.radar_on && range <= self.radar_range())
            || (range <= sight
                && line_of_sight(|n, e| terrain.elevation(n, e), [pos.x, pos.y, -pos.z], [at.x, at.y, -at.z]))
    }

    /// Erfasst ein Kandidat eines der Systeme? Liefert die Aufschaltung.
    fn detect(
        &self,
        targets: &[Target],
        terrain: &Terrain,
        pos: DVec3,
        range: f64,
        i: usize,
    ) -> Option<Lock> {
        let c = targets[i].center_ned();
        let optical = range <= self.optical_range()
            && line_of_sight(|n, e| terrain.elevation(n, e), [pos.x, pos.y, -pos.z], [c[0], c[1], -c[2]]);
        let radar = self.radar_on && range <= self.radar_range();
        (optical || radar).then_some(Lock { target: i, optical, radar })
    }

    /// Hält die Aufschaltung, solange ein System das Ziel erfasst, und dreht
    /// die Blickrichtung mit dem Ziel mit; sonst automatisch auf das nächste
    /// Ziel der Blickrichtung. `pos` ist die eigene Position (NED), `heading`
    /// der Kurs.
    pub fn update(&mut self, targets: &[Target], terrain: &Terrain, pos: DVec3, heading: f64) {
        // Auf einem zerstörten Ziel bleibt die Aufschaltung stehen, bis der
        // Spieler mit einer Richtungstaste ein neues wählt (M-013).
        if let Some(cur) = self.lock
            && targets.get(cur.target).is_some_and(|t| !t.is_intact())
        {
            return;
        }
        if let Some(cur) = self.lock
            && targets.get(cur.target).is_some_and(Target::is_intact)
        {
            let c = targets[cur.target].center_ned();
            let (dn, de, dd) = (c[0] - pos.x, c[1] - pos.y, c[2] - pos.z);
            let range = (dn * dn + de * de + dd * dd).sqrt();
            if let Some(lock) = self.detect(targets, terrain, pos, range, cur.target) {
                self.look = LookDir::from_relative_bearing(detmath::atan2(de, dn) - heading);
                self.lock = Some(lock);
                return;
            }
        }
        let candidates = self.candidates(targets, pos, heading);
        self.lock = candidates.iter().find_map(|&(range, i)| self.detect(targets, terrain, pos, range, i));
    }

    /// Wählt die Blickrichtung neu und schaltet auf das nächste Ziel dort.
    pub fn look_at(
        &mut self,
        look: LookDir,
        targets: &[Target],
        terrain: &Terrain,
        pos: DVec3,
        heading: f64,
    ) {
        self.look = look;
        self.lock = None;
        self.update(targets, terrain, pos, heading);
    }

    /// Schaltet in der gewählten Blickrichtung zum nächstweiteren erfassten
    /// Ziel weiter; nach dem weitesten wieder zum nächsten.
    pub fn next_target(&mut self, targets: &[Target], terrain: &Terrain, pos: DVec3, heading: f64) {
        let candidates = self.candidates(targets, pos, heading);
        let start =
            self.lock.and_then(|l| candidates.iter().position(|(_, i)| *i == l.target)).map_or(0, |k| k + 1);
        let n = candidates.len();
        self.lock = (0..n)
            .map(|k| candidates[(start + k) % n])
            .find_map(|(range, i)| self.detect(targets, terrain, pos, range, i));
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

    /// Jäger: Radar (an) weit, sonst nur in Sichtweite mit freier Sicht,
    /// nachts kürzer.
    #[test]
    fn aircraft_are_seen_by_radar_or_by_eye() {
        let terrain = Terrain::new(1);
        let pos = DVec3::new(0.0, 0.0, -9_000.0);
        let at = |km: f64| DVec3::new(km * 1_000.0, 0.0, -9_000.0);
        let mut s = Sensors::default();
        assert!(s.sees_aircraft(&terrain, pos, at(12.0)));
        assert!(!s.sees_aircraft(&terrain, pos, at(20.0)), "ohne Radar zu weit");
        s.radar_on = true;
        assert!(s.sees_aircraft(&terrain, pos, at(40.0)));
        s.radar_on = false;
        s.night = true;
        assert!(!s.sees_aircraft(&terrain, pos, at(8.0)), "nachts zu weit");
    }

    /// Dieselbe Richtung schaltet zum nächstweiteren, eine neue Richtung
    /// beginnt beim nächsten.
    #[test]
    fn pick_cycles_by_range() {
        let mut p = Pick::default();
        let c = [(1.0, 7), (2.0, 3), (3.0, 9)];
        p.press(LookDir::Forward, &c);
        assert_eq!(p.index, Some(7));
        p.press(LookDir::Forward, &c);
        assert_eq!(p.index, Some(3));
        p.press(LookDir::Right, &c);
        assert_eq!(p.index, Some(7));
        p.press(LookDir::Right, &[]);
        assert_eq!(p.index, None);
    }

    #[test]
    fn look_codes_roundtrip() {
        for d in LookDir::ALL {
            assert_eq!(LookDir::from_code(d.code()), Some(d));
        }
        assert_eq!(LookDir::from_code(0), None);
    }
}
