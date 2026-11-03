//! Auftrag einer Mission (M-032): Start- und Landeplatz, Primär- und
//! Sekundärziel aus den Objekten des Einsatzgebiets, die Route dazwischen.
//!
//! Der Seed wählt zwei militärische Ziele im Band
//! [`MIN_RANGE_M`]..[`MAX_RANGE_M`] um die Startbasis,
//! [`MIN_APART_M`]..[`MAX_APART_M`] auseinander; das nähere ist das
//! Primärziel (Spielerentscheidung). Je Ziel eine Auftragsart – Angriff oder
//! Aufklärung –, die Mischung nach Regelset (wie im Original nach
//! Konfliktstufe). Route: Start → Primärziel → Sekundärziel → Landeplatz.
//! Dazu eine vorgeschlagene Beladung (Kamera für Aufklärung).

use crate::nav::Steerpoint;
use crate::rng::Pcg32;
use crate::rules::Conflict;
use crate::targets::{Target, TargetClass};
use crate::weapons::{Loadout, WeaponKind};
use f29_world::{AirfieldKind, Terrain};

/// RNG-Strom des Auftrags.
pub const STREAM_MISSION: u64 = 0x4d53_4e00;
/// Ziele so weit von der Startbasis (m).
pub const MIN_RANGE_M: f64 = 80_000.0;
pub const MAX_RANGE_M: f64 = 220_000.0;
/// Primär- und Sekundärziel so weit auseinander (m).
pub const MIN_APART_M: f64 = 20_000.0;
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

/// Klassen, die sich fotografieren lassen (Radarstellungen nicht: zu
/// gefährlich nah).
const RECON_CLASSES: [TargetClass; 4] =
    [TargetClass::Building, TargetClass::Hardened, TargetClass::Bridge, TargetClass::ParkedAircraft];

/// Was am Ziel zu tun ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    /// Zerstören.
    Attack,
    /// Fotografieren (Kamera, M-032).
    Recon,
}

impl Task {
    pub fn label(self) -> &'static str {
        match self {
            Task::Attack => "ATTACK",
            Task::Recon => "RECON",
        }
    }
}

/// Anteil der Aufklärungsaufträge (Prozent) je Regelset (Spielerentscheidung
/// v0.3): im Kalten Krieg nur Aufklärung.
fn recon_percent(c: Conflict) -> u32 {
    match c {
        Conflict::ColdWar => 100,
        Conflict::Limited => 50,
        Conflict::War => 20,
    }
}

/// Ein Auftrag: Indizes in `Terrain::runways` bzw. `Sim::targets`; je Ziel
/// (Primär, Sekundär) die Auftragsart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mission {
    pub start: usize,
    pub landing: usize,
    pub primary: Option<usize>,
    pub secondary: Option<usize>,
    pub tasks: [Task; 2],
}

impl Mission {
    /// Primär- und Sekundärziel mit Auftragsart.
    pub fn objectives(&self) -> impl Iterator<Item = (usize, Task)> + '_ {
        [self.primary, self.secondary].into_iter().zip(self.tasks).filter_map(|(i, t)| Some((i?, t)))
    }

    pub fn has_recon(&self) -> bool {
        self.objectives().any(|(_, t)| t == Task::Recon)
    }

    /// Vorgeschlagene Beladung (wie im Original): Sidewinder und HARM, dazu je
    /// Ziel die Kamera bzw. eine Waffe gegen seine Klasse; die Kamera nur
    /// einmal. Reicht der Haupttank nicht (`fuel_short`), ersetzt ein
    /// Zusatztank die HARM.
    pub fn loadout(&self, targets: &[Target], fuel_short: bool) -> Loadout {
        let second = if fuel_short { WeaponKind::DropTank } else { WeaponKind::AntiRadar };
        let mut bays = [Some(WeaponKind::Sidewinder), Some(second), None, None];
        for (k, (i, task)) in self.objectives().enumerate() {
            bays[2 + k] = Some(match task {
                Task::Recon if !bays.contains(&Some(WeaponKind::Camera)) => WeaponKind::Camera,
                _ => match targets[i].class {
                    TargetClass::Building | TargetClass::Hardened | TargetClass::Bridge => {
                        WeaponKind::GuidedBomb
                    }
                    _ => WeaponKind::AirToGround,
                },
            });
        }
        Loadout(bays)
    }
}

fn dist(a: [f64; 2], t: &Target) -> f64 {
    (t.north - a[0]).hypot(t.east - a[1])
}

/// Wählt den Auftrag für `seed` unter dem Regelset `conflict`.
pub fn plan(terrain: &Terrain, targets: &[Target], seed: u64, conflict: Conflict) -> Mission {
    let start = terrain.runways().iter().position(|r| r.kind == AirfieldKind::Home).unwrap_or(0);
    let home = terrain.runways().get(start).map_or([0.0, 0.0], |r| r.center);
    let band: Vec<usize> = (0..targets.len())
        .filter(|&i| {
            let t = &targets[i];
            !t.civilian
                && MISSION_CLASSES.contains(&t.class)
                && (MIN_RANGE_M..=MAX_RANGE_M).contains(&dist(home, t))
        })
        .collect();
    let mut rng = Pcg32::new(seed, STREAM_MISSION);
    // Je Ziel zuerst die Auftragsart, dann ein passendes Ziel (Aufklärung nur
    // an fotografierbaren Klassen); ohne passendes Ziel: Angriff.
    let mut choose = |near: &dyn Fn(usize) -> bool| {
        let recon = rng.next_u32() % 100 < recon_percent(conflict);
        let of = |task: Task| -> Vec<usize> {
            band.iter()
                .copied()
                .filter(|&i| near(i) && (task == Task::Attack || RECON_CLASSES.contains(&targets[i].class)))
                .collect()
        };
        let (task, c) = match of(Task::Recon) {
            c if recon && !c.is_empty() => (Task::Recon, c),
            _ => (Task::Attack, of(Task::Attack)),
        };
        (!c.is_empty()).then(|| (c[rng.next_u32() as usize % c.len()], task))
    };
    // Neu würfeln, bis beide Ziele passen (im Kalten Krieg beide
    // fotografierbar).
    let mut pair = (None, None);
    for _ in 0..50 {
        let first = choose(&|_| true);
        let second = first.and_then(|(a, _)| {
            let at = [targets[a].north, targets[a].east];
            choose(&|b| (MIN_APART_M..=MAX_APART_M).contains(&dist(at, &targets[b])))
        });
        let allowed =
            |x: Option<(usize, Task)>| x.is_some_and(|x| recon_percent(conflict) < 100 || x.1 == Task::Recon);
        let ok = allowed(first) && allowed(second);
        if pair.0.is_none() || ok {
            pair = (first, second);
        }
        if ok {
            break;
        }
    }
    let (first, second) = pair;
    let (first, second) = match (first, second) {
        (Some(a), Some(b)) if dist(home, &targets[b.0]) < dist(home, &targets[a.0]) => (Some(b), Some(a)),
        other => other,
    };
    let task = |x: Option<(usize, Task)>| x.map_or(Task::Attack, |x| x.1);
    Mission {
        start,
        landing: start,
        primary: first.map(|x| x.0),
        secondary: second.map(|x| x.0),
        tasks: [task(first), task(second)],
    }
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
