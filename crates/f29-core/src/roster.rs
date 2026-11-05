//! Dienstplan und Laufbahn (M-036, M-030): zehn Plätze mit den Akten der
//! Piloten. Ein Einsatz kommt erst beim Übergang von der Nachbesprechung zum
//! Dienstplan in die Akte ([`Roster::record`]) – wer vorher beendet, verwirft
//! ihn, wie im Original; was er bewirkt, zeigt die Ehrung vorher an einer
//! Kopie ([`Pilot::fly`]). Laufbahn wie im Original mit dessen Rang- und
//! Ordensnamen, die Schwellen sind eigene. Gespeichert wird als RON; Datei
//! und Ort bestimmt das Frontend.

use crate::rules::{Conflict, Landings, Opponents};
use crate::sim::Status as Outcome;
use f29_world::Side;
use serde::{Deserialize, Serialize};

/// Plätze im Dienstplan.
pub const SLOTS: usize = 10;
/// Höchstlänge eines Namens (Zeichen).
pub const NAME_LEN: usize = 16;
/// Ränge, niedrigster zuerst.
pub const RANKS: [&str; 7] =
    ["2ND LT", "1ST LT", "CAPTAIN", "MAJOR", "LT COLONEL", "COLONEL", "BRIG GENERAL"];
/// Nach so vielen Einsätzen geht ein Pilot in den Ruhestand.
pub const RETIREMENT: u32 = 60;
/// Schwellen der Beförderung in Rang k + 1: Einsätze, Gesamtpunkte, Schnitt
/// je Einsatz. Den höchsten Rang gibt es nur mit dem letzten Einsatz.
pub const PROMOTION: [(u32, i64, i64); 6] = [
    (3, 1_500, 300),
    (8, 5_000, 450),
    (15, 12_000, 600),
    (25, 22_000, 700),
    (40, 36_000, 800),
    (RETIREMENT, 45_000, 700),
];
/// Orden, niedrigster zuerst: Name und Punkte eines Einsatzes für den ersten,
/// der n-te verlangt das n-Fache. Den höchsten gibt es nur einmal und erst
/// über der Schwelle, den niedrigsten höchstens [`LOWEST_MAX`]-mal.
pub const MEDALS: [(&str, i32); 5] = [
    ("AIRMAN'S MEDAL", 250),
    ("DISTINGUISHED FLYING CROSS", 600),
    ("SILVER STAR", 1_000),
    ("AIR FORCE CROSS", 1_500),
    ("CONGRESSIONAL MEDAL OF HONOR", 2_000),
];
pub const LOWEST_MAX: u32 = 9;
/// Einmalig für einen Treffer am eigenen Flugzeug, auch neben einem Orden.
pub const PURPLE_HEART: &str = "PURPLE HEART";
/// Einsatzbänder ab so vielen Einsätzen; paarweise, das zweite ersetzt das
/// erste (Namen eigene, die des Originals sind nicht überliefert).
pub const RIBBONS: [(u32, &str); 4] = [
    (5, "LONGEVITY SERVICE RIBBON"),
    (15, "LONGEVITY SERVICE RIBBON WITH OAK LEAF CLUSTER"),
    (30, "EXPEDITIONARY SERVICE RIBBON"),
    (50, "EXPEDITIONARY SERVICE RIBBON WITH OAK LEAF CLUSTER"),
];

/// Status eines Piloten; nur aktive lassen sich wählen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Active,
    Retired,
    Killed,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Active => "ACTIVE",
            Status::Retired => "RETIRED",
            Status::Killed => "KILLED",
        }
    }
}

/// Einstellungen des Auftrags, die ein Pilot mitnimmt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Settings {
    pub conflict: Conflict,
    pub opponents: Opponents,
    pub landings: Landings,
}

impl Settings {
    pub fn of(spawn: &crate::sim::Spawn) -> Self {
        Settings { conflict: spawn.conflict, opponents: spawn.opponents, landings: spawn.landings }
    }
}

/// Was ein Einsatz der Akte liefert.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mission {
    /// Gesamtwertung (M-032).
    pub points: i32,
    pub outcome: Outcome,
    /// Das eigene Flugzeug wurde getroffen.
    pub hit: bool,
    pub settings: Settings,
}

impl Mission {
    /// Der Einsatz, den `sim` geflogen hat.
    pub fn of(sim: &crate::Sim) -> Self {
        Mission {
            points: sim.score().total,
            outcome: sim.status(),
            hit: sim.logged(crate::flightlog::LogEntry::Hit).is_some(),
            settings: Settings::of(&sim.config().spawn),
        }
    }
}

/// Was ein Einsatz bewirkt hat – Stoff der Ehrung.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Report {
    pub promoted: bool,
    /// Beförderung wegen Gefangennahme verzögert.
    pub delayed: bool,
    /// Stufe des Ordens ([`MEDALS`]).
    pub medal: Option<usize>,
    pub purple_heart: bool,
    /// Neues Einsatzband ([`RIBBONS`]).
    pub ribbon: Option<usize>,
}

/// Akte eines Piloten.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pilot {
    pub name: String,
    /// Index in [`RANKS`].
    pub rank: usize,
    pub points: i64,
    pub missions: u32,
    pub status: Status,
    pub settings: Settings,
    /// Verliehene Orden je Stufe ([`MEDALS`]).
    #[serde(default)]
    pub medals: [u32; 5],
    #[serde(default)]
    pub purple_heart: bool,
}

impl Pilot {
    /// Neuer Pilot: niedrigster Rang, ohne Punkte, Einsätze und Orden.
    pub fn new(name: &str, settings: Settings) -> Self {
        Pilot {
            name: name.into(),
            rank: 0,
            points: 0,
            missions: 0,
            status: Status::Active,
            settings,
            medals: [0; 5],
            purple_heart: false,
        }
    }

    pub fn rank_name(&self) -> &'static str {
        RANKS[self.rank.min(RANKS.len() - 1)]
    }

    /// Einsatzbänder, die er trägt.
    pub fn ribbons(&self) -> impl Iterator<Item = &'static str> + '_ {
        RIBBONS.chunks(2).filter_map(|pair| pair.iter().rev().find(|r| self.missions >= r.0)).map(|r| r.1)
    }

    /// Ein geflogener Einsatz: ein Einsatz mehr, seine Punkte und
    /// Einstellungen; dann wie im Original Beförderung (höchstens ein Rang,
    /// nach Gefangennahme verzögert), Orden, Einsatzband und Status – tot
    /// nach Absturz oder misslungenem Ausstieg, sonst nach dem letzten
    /// Einsatz im Ruhestand.
    pub fn fly(&mut self, m: &Mission) -> Report {
        self.missions += 1;
        self.points += i64::from(m.points);
        self.settings = m.settings;
        let mut r = Report::default();
        let mean = self.points / i64::from(self.missions);
        let meets =
            |&(n, total, avg): &(u32, i64, i64)| self.missions >= n && self.points >= total && mean >= avg;
        if PROMOTION.get(self.rank..).unwrap_or_default().iter().any(meets) {
            let captured = matches!(
                m.outcome,
                Outcome::Landed { side: Side::Hostile, .. }
                    | Outcome::Ejected { side: Side::Hostile, survived: true, .. }
            );
            r.delayed = captured;
            r.promoted = !captured;
            self.rank += usize::from(!captured);
        }
        let earned = |k: usize| {
            let (n, threshold) = (self.medals[k], MEDALS[k].1);
            match k {
                0 => n < LOWEST_MAX && m.points >= threshold * (n as i32 + 1),
                4 => n == 0 && m.points > threshold,
                _ => m.points >= threshold * (n as i32 + 1),
            }
        };
        r.medal = (0..MEDALS.len()).rev().find(|&k| earned(k));
        if let Some(k) = r.medal {
            self.medals[k] += 1;
        }
        r.purple_heart = m.hit && !self.purple_heart;
        self.purple_heart |= m.hit;
        r.ribbon = RIBBONS.iter().position(|ribbon| ribbon.0 == self.missions);
        if matches!(m.outcome, Outcome::Crashed { .. } | Outcome::Ejected { survived: false, .. }) {
            self.status = Status::Killed;
        } else if self.missions >= RETIREMENT {
            self.status = Status::Retired;
        }
        r
    }
}

/// Der Dienstplan.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Roster {
    pub slots: [Option<Pilot>; SLOTS],
    /// Zuletzt gewählter Pilot.
    pub current: Option<usize>,
}

impl Roster {
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).unwrap_or_default()
    }

    /// Ein geflogener Einsatz des Piloten in `slot` ([`Pilot::fly`]).
    pub fn record(&mut self, slot: usize, m: &Mission) -> Option<Report> {
        Some(self.slots.get_mut(slot)?.as_mut()?.fly(m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::CrashCause;

    const HOME: Outcome = Outcome::Landed { tick: 1, side: Side::Friendly };
    const CAPTURED: Outcome = Outcome::Ejected { tick: 1, side: Side::Hostile, survived: true };

    fn mission(points: i32, outcome: Outcome) -> Mission {
        Mission { points, outcome, hit: false, settings: Settings::default() }
    }

    /// Ein Einsatz zählt Punkte und Einsatz; nach Gefangennahme bleibt der
    /// Pilot aktiv, nach einem Absturz ist er gefallen; RON hin und zurück
    /// ändert nichts.
    #[test]
    fn record_counts_and_survives_ron() {
        let mut r = Roster::default();
        r.slots[3] = Some(Pilot::new("MAVERICK", Settings::default()));
        let war = Settings { conflict: Conflict::War, ..Settings::default() };
        r.record(3, &Mission { settings: war, ..mission(450, CAPTURED) });
        let p = r.slots[3].as_ref().unwrap();
        assert_eq!((p.missions, p.points, p.status, p.settings), (1, 450, Status::Active, war));
        r.record(3, &mission(0, Outcome::Crashed { tick: 2, cause: CrashCause::Terrain }));
        assert_eq!(r.slots[3].as_ref().unwrap().status, Status::Killed);
        assert_eq!(r.record(5, &mission(100, HOME)), None, "leerer Platz bleibt leer");
        r.current = Some(3);
        assert_eq!(Roster::from_ron(&r.to_ron()).unwrap(), r);
    }

    /// Orden nach der Wertung des Einsatzes: der höchste erreichte, beim
    /// n-ten Mal das n-Fache der Schwelle; der höchste nur einmal, danach
    /// zählt die nächste Stufe; dazu einmal das Purple Heart.
    #[test]
    fn medals_per_mission_and_repeated() {
        let mut p = Pilot::new("ICEMAN", Settings::default());
        let top = MEDALS[4].1 + 1;
        let fly = |p: &mut Pilot, points, hit| p.fly(&Mission { hit, ..mission(points, HOME) });
        assert_eq!(fly(&mut p, MEDALS[4].1, false).medal, Some(3), "die höchste erst über der Schwelle");
        let r = fly(&mut p, top, true);
        assert_eq!((r.medal, r.purple_heart), (Some(4), true));
        let r = fly(&mut p, top, true);
        assert_eq!((r.medal, r.purple_heart), (Some(2), false), "zweites Air Force Cross erst ab doppelt");
        assert_eq!(fly(&mut p, 2 * MEDALS[3].1, false).medal, Some(3));
        assert_eq!(fly(&mut p, MEDALS[0].1 - 1, false).medal, None);
        assert_eq!(p.medals, [0, 0, 1, 2, 1]);
        p.medals[0] = LOWEST_MAX;
        assert_eq!(fly(&mut p, MEDALS[1].1 - 1, false).medal, None, "die niedrigste höchstens neunmal");
    }

    /// Beförderung um höchstens einen Rang, nach Gefangennahme verzögert;
    /// Bänder bei den Einsatzzahlen, das zweite eines Paars ersetzt das
    /// erste; Ruhestand nach dem letzten Einsatz, Tod geht vor.
    #[test]
    fn promotion_ribbons_retirement() {
        let mut p = Pilot::new("GOOSE", Settings::default());
        let rich = 10_000;
        for _ in 1..PROMOTION[0].0 {
            assert!(!p.fly(&mission(rich, HOME)).promoted);
        }
        let r = p.fly(&mission(0, CAPTURED));
        assert!(r.delayed && !r.promoted && p.rank == 0);
        assert!(p.fly(&mission(rich, HOME)).promoted);
        p.missions = PROMOTION[4].0 - 1;
        assert!(p.fly(&mission(rich, HOME)).promoted);
        assert_eq!(p.rank, 2, "ein Rang je Einsatz, auch wenn mehrere Schwellen erfüllt sind");
        p.missions = RIBBONS[1].0 - 1;
        assert_eq!(p.fly(&mission(0, HOME)).ribbon, Some(1));
        assert_eq!(p.ribbons().collect::<Vec<_>>(), [RIBBONS[1].1]);
        p.missions = RETIREMENT - 1;
        p.fly(&mission(rich, HOME));
        assert_eq!(p.status, Status::Retired);
        let mut q = Pilot { missions: RETIREMENT - 1, ..Pilot::new("HOLLYWOOD", Settings::default()) };
        q.fly(&mission(0, Outcome::Ejected { tick: 1, side: Side::Friendly, survived: false }));
        assert_eq!(q.status, Status::Killed);
    }
}
