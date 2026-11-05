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
pub mod events;
pub mod fighters;
pub mod flight;
pub mod flightlog;
pub mod gun;
pub mod math;
pub mod mission;
pub mod nav;
pub mod radar;
pub mod replay;
pub mod rng;
pub mod roster;
pub mod rules;
pub mod sensors;
pub mod sim;
pub mod systems;
pub mod targets;
pub mod units;
pub mod weapons;

pub use controls::ControlFrame;
pub use math::{DQuat, DVec3};
pub use sim::{
    BlastInfo, BlastKind, BombAim, CameraInfo, CrashCause, DT, DecoyInfo, FighterInfo, FlightData, GunAim,
    GunInfo, Mark, MarkKind, MunitionInfo, PickedInfo, PilotInfo, PingInfo, RadarSiteInfo, Score, ScoreLine,
    Sim, SimConfig, SpawnKind, Stage, Status, StoresInfo, TICK_HZ, TargetInfo, ThreatInfo, ThreatSite,
    Touchdown, WeaponCam, WeaponMessage,
};

pub use f29_data as data;
pub use f29_world as terrain;
pub use f29_world::Side;
