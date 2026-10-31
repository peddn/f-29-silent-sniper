//! Spieldaten von F-29 Silent Sniper.
//!
//! Alle Zahlen in diesem Crate sind **eigene Werte** dieses Projekts
//! (siehe `docs/MECHANICS.md`, Clean-Room-Protokoll). Sie sind nicht aus dem
//! Original übernommen, sondern aus öffentlich bekannten Größenordnungen
//! vergleichbarer Flugzeuge abgeleitet und auf Spielbarkeit abgestimmt. Sie
//! liegen als Rust-Konstanten vor; die Typen hier sind die gemeinsame Sprache
//! zwischen Daten und Kern.

mod aircraft;

pub use aircraft::{AircraftSpec, F29_SPEC};
