//! Zustandshash für Determinismus und Referenz-Replays.

use super::*;
use std::fmt::Write as _;

impl Sim {
    /// Hash über den vollständigen veränderlichen Zustand (FNV-1a, 64 Bit),
    /// gebildet aus seiner Debug-Ausgabe – so fehlt kein Feld. Neue Felder
    /// erzwingt die vollständige Zerlegung: Sie müssen hier eingetragen oder
    /// ausdrücklich ausgelassen werden.
    ///
    /// Gleicher Seed + gleiche Eingaben ⇒ gleicher Hash, auf jeder Plattform
    /// (mit derselben Rust-Version: ändert eine neue das Debug-Format, sind
    /// die Golden-Replays neu zu schreiben).
    pub fn state_hash(&self) -> u64 {
        let Sim {
            config,
            tick,
            aircraft,
            status,
            active_steerpoint,
            last_controls,
            turbulence_rng,
            gust,
            touchdown,
            targets,
            sensors,
            stores,
            gun,
            radar,
            fighters,
            picks,
            systems,
            contact_cursor,
            events,
            log,
            has_flown,
            pilot,
            // fest bzw. aus Seed und Zustand abgeleitet
            spec: _,
            terrain: _,
            aero: _,
            route: _,
            mission: _,
            gun_prediction: _,
            bomb_prediction: _,
            launch_zone: _,
            ground_seen: _,
            fighter_seen: _,
        } = self;
        let mut text = String::new();
        let _ = write!(
            text,
            "{:?}{tick}{aircraft:?}{status:?}{active_steerpoint}{last_controls:?}{turbulence_rng:?}{gust:?}",
            (config.seed, &config.spawn)
        );
        let _ = write!(text, "{touchdown:?}{targets:?}{sensors:?}{stores:?}{gun:?}{radar:?}{fighters:?}");
        let _ = write!(text, "{picks:?}{systems:?}{contact_cursor}{events:?}{log:?}{has_flown}{pilot:?}");
        text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
    }
}
