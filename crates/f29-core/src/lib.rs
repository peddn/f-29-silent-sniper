//! Simulationskern von F-29 Silent Sniper.
//!
//! Dieses Crate kennt weder Fenster noch GPU noch Engine. Es ist eine reine,
//! deterministische Funktion `(Konfiguration, Eingabestrom) → Zustand` mit
//! festem Zeitschritt ([`sim::TICK_HZ`]). Frontends lesen den Zustand und
//! stellen ihn dar; sie schreiben ihn nie direkt.
//!
//! Regeln für Code in diesem Crate (siehe `docs/ARCHITECTURE.md`):
//! - SI-Einheiten, Welt in NED, Körperachsen x vorn / y rechts / z unten.
//! - Transzendente Funktionen nur aus [`detmath`].
//! - Kein `HashMap`-Iterieren, keine Systemzeit, kein globaler Zufall –
//!   Zufall nur über [`rng::Pcg32`] mit eigenem Strom pro Subsystem.

pub mod atmosphere;
pub mod autopilot;
pub mod controls;
pub mod detmath;
pub mod flight;
pub mod math;
pub mod nav;
pub mod replay;
pub mod rng;
pub mod sim;
pub mod units;

pub use controls::ControlFrame;
pub use math::{DQuat, DVec3};
pub use sim::{CrashCause, DT, FlightData, Sim, SimConfig, SpawnKind, Status, TICK_HZ, Touchdown};

pub use f29_data as data;
pub use f29_terrain as terrain;
