//! Flugprotokoll (M-032): was während des Flugs geschah, mit Tick – die
//! Grundlage für Nachbesprechung und Wertung. Entsteht aus den Eingaben und
//! ist damit so deterministisch wie die Simulation.

use crate::fighters::FighterKind;
use crate::sim::CrashCause;
use f29_world::Side;

/// Ein Eintrag im Flugprotokoll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogEntry {
    /// Abgehoben (nur beim Start von der Bahn).
    Takeoff,
    /// Wegpunkt der Route erreicht (Index in `Sim::route`).
    Waypoint(usize),
    /// Eine Radarstellung hat das Flugzeug identifiziert (Ziel-Index).
    RadarId(usize),
    /// Ein Jäger hat das Flugzeug mit seinem Radar erfasst.
    FighterContact(FighterKind),
    /// Bodenziel zerstört (Ziel-Index).
    Destroyed(usize),
    /// Aufklärungsziel fotografiert (0 = Primär-, 1 = Sekundärziel).
    Photographed(usize),
    /// Jäger abgeschossen.
    FighterKill(FighterKind),
    /// Das Flugzeug wurde getroffen.
    Hit,
    /// Einsatzregeln verletzt.
    RoeViolation,
    /// Gelandet und zum Stehen gekommen.
    Landed(Side),
    /// Ausgestiegen; `survived`: Fahrt und Lage ließen es zu.
    Ejected {
        side: Side,
        survived: bool,
    },
    /// Der Pilot ist am Schirm gelandet.
    PilotDown,
    /// Das verlassene Flugzeug ist aufgeschlagen.
    AircraftLost,
    Crashed(CrashCause),
}
