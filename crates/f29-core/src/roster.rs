//! Dienstplan (M-036): zehn Plätze mit den Akten der Piloten. Ein Einsatz
//! kommt erst beim Übergang von der Nachbesprechung zum Dienstplan in die
//! Akte ([`Roster::record`]) – wer vorher beendet, verwirft ihn, wie im
//! Original. Gespeichert wird als RON; Datei und Ort bestimmt das Frontend.

use crate::rules::{Conflict, Landings, Opponents};
use crate::sim::Status as Outcome;
use serde::{Deserialize, Serialize};

/// Plätze im Dienstplan.
pub const SLOTS: usize = 10;
/// Höchstlänge eines Namens (Zeichen).
pub const NAME_LEN: usize = 16;
/// Ränge, niedrigster zuerst; befördert wird erst mit der Laufbahn (M-030).
pub const RANKS: [&str; 6] = ["2LT", "1LT", "CAPT", "MAJ", "LTCOL", "COL"];

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
}

impl Pilot {
    /// Neuer Pilot: niedrigster Rang, ohne Punkte und Einsätze.
    pub fn new(name: &str, settings: Settings) -> Self {
        Pilot { name: name.into(), rank: 0, points: 0, missions: 0, status: Status::Active, settings }
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

    /// Ein geflogener Einsatz des Piloten in `slot`: ein Einsatz mehr, die
    /// Punkte der Wertung, die Einstellungen; tot ist er nach Absturz oder
    /// misslungenem Ausstieg (eine Gefangennahme beendet die Laufbahn nicht).
    pub fn record(&mut self, slot: usize, points: i32, outcome: Outcome, settings: Settings) {
        let Some(p) = self.slots.get_mut(slot).and_then(Option::as_mut) else { return };
        p.missions += 1;
        p.points += i64::from(points);
        p.settings = settings;
        if matches!(outcome, Outcome::Crashed { .. } | Outcome::Ejected { survived: false, .. }) {
            p.status = Status::Killed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::CrashCause;
    use f29_world::Side;

    /// Ein Einsatz zählt Punkte und Einsatz; nach einem Absturz ist der
    /// Pilot gefallen, nach Gefangennahme bleibt er aktiv; RON hin und
    /// zurück ändert nichts.
    #[test]
    fn record_counts_and_survives_ron() {
        let mut r = Roster::default();
        r.slots[3] = Some(Pilot::new("MAVERICK", Settings::default()));
        let captured = Outcome::Ejected { tick: 1, side: Side::Hostile, survived: true };
        r.record(3, 450, captured, Settings { conflict: Conflict::War, ..Settings::default() });
        let p = r.slots[3].as_ref().unwrap();
        assert_eq!(
            (p.missions, p.points, p.status, p.settings.conflict),
            (1, 450, Status::Active, Conflict::War)
        );
        r.record(3, 0, Outcome::Crashed { tick: 2, cause: CrashCause::Terrain }, Settings::default());
        assert_eq!(r.slots[3].as_ref().unwrap().status, Status::Killed);
        r.record(5, 100, captured, Settings::default());
        assert_eq!(r.slots[5], None, "leerer Platz bleibt leer");
        r.current = Some(3);
        assert_eq!(Roster::from_ron(&r.to_ron()).unwrap(), r);
    }
}
