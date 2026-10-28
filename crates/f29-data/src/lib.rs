//! Spieldaten von F-29 Silent Sniper.
//!
//! Alle Zahlen in diesem Crate sind **eigene Werte** dieses Projekts
//! (siehe `docs/MECHANICS.md`, Clean-Room-Protokoll). Sie sind nicht aus dem
//! Original übernommen, sondern aus öffentlich bekannten Größenordnungen
//! vergleichbarer Flugzeuge abgeleitet und auf Spielbarkeit abgestimmt.
//!
//! In v0.1 liegen die Daten als Rust-Konstanten vor. Mit v0.2 (Waffen,
//! Bedrohungen) kommt das Laden aus RON-Dateien unter `data/` hinzu; die
//! Typen hier bleiben dabei die gemeinsame Sprache zwischen Daten und Kern.

mod aircraft;

pub use aircraft::{AircraftSpec, F29_SPEC};

/// Fehler bei der Plausibilitätsprüfung von Spieldaten.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    /// Name des Datensatzes, z. B. `"F-29"`.
    pub record: String,
    /// Feld, das die Prüfung nicht besteht.
    pub field: &'static str,
    /// Menschlich lesbare Begründung.
    pub reason: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: Feld `{}`: {}", self.record, self.field, self.reason)
    }
}

impl std::error::Error for ValidationError {}
