//! Auftrag einer Mission (M-032): Start- und Landeplatz, Primär- und
//! Sekundärziel aus den Objekten des Einsatzgebiets, die Route dazwischen.
//!
//! Übergangsregel bis zum Missionsgenerator: Der Seed wählt zwei
//! militärische Ziele im Band [`MIN_RANGE_M`]..[`MAX_RANGE_M`] um die
//! Startbasis, nicht weiter als [`MAX_APART_M`] auseinander; das nähere ist
//! das Primärziel (Spielerentscheidung). Route: Start → Primärziel →
//! Sekundärziel → Landeplatz (wie im Original; gewertet wird später jede
//! Landung auf eigenem Gebiet).

use crate::nav::Steerpoint;
use crate::rng::Pcg32;
use crate::targets::{Target, TargetClass};
use f29_world::{AirfieldKind, Terrain};

/// RNG-Strom des Auftrags.
pub const STREAM_MISSION: u64 = 0x4d53_4e00;
/// Ziele so weit von der Startbasis (m).
pub const MIN_RANGE_M: f64 = 100_000.0;
pub const MAX_RANGE_M: f64 = 260_000.0;
/// Primär- und Sekundärziel höchstens so weit auseinander (m).
pub const MAX_APART_M: f64 = 80_000.0;
/// Klassen, die Missionsziele sein können.
const MISSION_CLASSES: [TargetClass; 6] = [
    TargetClass::Building,
    TargetClass::Hardened,
    TargetClass::Bridge,
    TargetClass::ParkedAircraft,
    TargetClass::Armored,
    TargetClass::Soft,
];

/// Ein Auftrag: Indizes in `Terrain::runways` bzw. `Sim::targets`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mission {
    pub start: usize,
    pub landing: usize,
    pub primary: Option<usize>,
    pub secondary: Option<usize>,
}

fn dist(a: [f64; 2], t: &Target) -> f64 {
    (t.north - a[0]).hypot(t.east - a[1])
}

/// Wählt den Auftrag für `seed`.
pub fn plan(terrain: &Terrain, targets: &[Target], seed: u64) -> Mission {
    let start = terrain.runways().iter().position(|r| r.kind == AirfieldKind::Home).unwrap_or(0);
    let home = terrain.runways().get(start).map_or([0.0, 0.0], |r| r.center);
    let fit = |t: &Target| !t.civilian && MISSION_CLASSES.contains(&t.class);
    let band: Vec<usize> = (0..targets.len())
        .filter(|&i| fit(&targets[i]) && (MIN_RANGE_M..=MAX_RANGE_M).contains(&dist(home, &targets[i])))
        .collect();
    let mut rng = Pcg32::new(seed, STREAM_MISSION);
    let mut pick = |c: &[usize]| (!c.is_empty()).then(|| c[rng.next_u32() as usize % c.len()]);
    let first = pick(&band);
    let second = first.and_then(|a| {
        let at = [targets[a].north, targets[a].east];
        let near: Vec<usize> =
            band.iter().copied().filter(|&b| b != a && dist(at, &targets[b]) <= MAX_APART_M).collect();
        pick(&near)
    });
    let (primary, secondary) = match (first, second) {
        (Some(a), Some(b)) if dist(home, &targets[b]) < dist(home, &targets[a]) => (Some(b), Some(a)),
        other => other,
    };
    Mission { start, landing: start, primary, secondary }
}

/// Route des Auftrags: Start → Primärziel → Sekundärziel → Landeplatz.
pub fn route(m: &Mission, terrain: &Terrain, targets: &[Target]) -> Vec<Steerpoint> {
    let runway = |i: usize| terrain.runways().get(i).map(|r| Steerpoint::runway(r, i));
    let target = |i: Option<usize>, name: &str| {
        i.map(|i| {
            let t = &targets[i];
            Steerpoint {
                name: name.into(),
                north: t.north,
                east: t.east,
                elevation: t.elevation,
                runway: None,
            }
        })
    };
    [runway(m.start), target(m.primary, "PRIMARY"), target(m.secondary, "SECONDARY"), runway(m.landing)]
        .into_iter()
        .flatten()
        .collect()
}
