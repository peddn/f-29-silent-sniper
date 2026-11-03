//! Trefferquoten der Boden-Luft-Raketen je Flugweise: feste,
//! deterministische Anflüge auf eine Stellung, je drei Startpunkte in drei
//! Missionen (verschiedene Zufallsströme, z. B. für Täuschkörper).
//!
//! Dient wie [`crate::envelope`] zum Abstimmen
//! (`cargo run --release -p f29-core --example sam_report`) und als Grundlage
//! der Gefährlichkeits-Tests (`tests/sam.rs`): Von vorn und hinten treffen
//! alle; Querflug schüttelt langsame Raketen mit engem Suchkegel ab, die
//! moderne kaum; Täuschkörper wirken nach der Resistenz der Rakete.

use crate::DT;
use crate::autopilot::Autopilot;
use crate::controls::weapon_flags;
use crate::sim::{Sim, SimConfig, Spawn, Status};
use crate::targets;
use crate::weapons::{Seeker, WeaponEvent, WeaponKind};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

const SEEDS: [u64; 3] = [0x0F29, 0x0F2A, 0x0F2B];
/// Flughöhe über der Stellung (m), mindestens aber so viel über dem
/// höchsten Gelände.
const HEIGHT_M: f64 = 2_000.0;
const CLEARANCE_M: f64 = 400.0;

/// Was der Pilot tut, sobald eine Rakete fliegt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Maneuver {
    /// Auf die Rakete zu (sie kommt von vorn).
    Toward,
    /// Von ihr weg (sie kommt von hinten).
    Away,
    /// Querab halten.
    Beam,
    /// Auf sie zu und bei 1,5 km so viele passende Täuschkörper auf einmal
    /// werfen, dann geradeaus weiter.
    Decoys(u8),
    /// Wie [`Maneuver::Decoys`], nach dem Wurf aber querab abdrehen.
    DecoysBeam(u8),
}

impl Maneuver {
    pub const ALL: [Maneuver; 6] = [
        Maneuver::Toward,
        Maneuver::Away,
        Maneuver::Beam,
        Maneuver::Decoys(1),
        Maneuver::Decoys(3),
        Maneuver::DecoysBeam(1),
    ];

    pub fn label(self) -> String {
        match self {
            Maneuver::Toward => "vorn".into(),
            Maneuver::Away => "hinten".into(),
            Maneuver::Beam => "querab".into(),
            Maneuver::Decoys(n) => format!("{n} Täuschk."),
            Maneuver::DecoysBeam(n) => format!("{n} T.+querab"),
        }
    }
}

/// Die vier feindlichen Raketen.
pub const SAMS: [WeaponKind; 4] =
    [WeaponKind::ShortRangeSam, WeaponKind::LongRangeSam, WeaponKind::MediumRangeSam, WeaponKind::ModernSam];

fn wrap(a: f64) -> f64 {
    (a + PI).rem_euclid(TAU) - PI
}

/// Ein Anflug: Start `south`/`west` (Anteile der Reichweite) südlich bzw.
/// westlich der Stellung, Kurs Ost. `Some(Treffer)`, `None`, wenn keine
/// Rakete startete oder das Flugzeug abstürzte.
pub fn engagement(seed: u64, kind: WeaponKind, south: f64, west: f64, m: Maneuver) -> Option<bool> {
    let probe = Sim::new(SimConfig::new(seed));
    // Je Seed eine andere Stellung dieser Art.
    let sites: Vec<usize> =
        (0..probe.targets().len()).filter(|&i| targets::sam_of(probe.targets(), i) == Some(kind)).collect();
    let i = *sites.get(seed as usize % sites.len().max(1))?;
    let t = &probe.targets()[i];
    let range = kind.spec().max_range_m;
    // Über dem höchsten Gelände im Umkreis der Reichweite.
    let terrain = probe.terrain();
    let top = (-10..=10)
        .flat_map(|a| (-10..=10).map(move |b| (f64::from(a) * 0.1, f64::from(b) * 0.1)))
        .map(|(a, b)| terrain.elevation(t.north + a * range, t.east + b * range))
        .fold(t.elevation, f64::max);
    let alt = (t.elevation + HEIGHT_M).max(top + CLEARANCE_M);
    let mut sim = Sim::new(SimConfig::with(
        seed,
        Spawn {
            north: t.north - south * range,
            east: t.east - west * range,
            altitude: alt,
            min_clearance: 300.0,
            heading: FRAC_PI_2,
            speed: 230.0,
            throttle: 1.0,
            visible: true,
            ..Spawn::default()
        },
    ));
    // Nur diese Stellung schießt.
    for k in 0..sim.targets().len() {
        if k != i && sim.targets()[k].class.is_radar() {
            sim.destroy_target(k);
        }
    }
    let decoy = if kind.spec().seeker == Seeker::Optical { weapon_flags::FLARE } else { weapon_flags::CHAFF };
    let mut ap = Autopilot::new(alt, FRAC_PI_2, 300.0);
    ap.max_bank = 1.25;
    let (mut launched, mut thrown) = (false, 0);
    for _ in 0..(200.0 / DT) as usize {
        let fd = sim.flight_data();
        let missile = sim.munitions().in_flight.iter().find(|w| w.owner.is_some()).copied();
        let mut weapons = 0;
        if let Some(w) = missile {
            launched = true;
            let to = w.position - fd.position;
            let b = to.y.atan2(to.x);
            let beam = b - FRAC_PI_2 * wrap(b - fd.heading).signum();
            ap.target_heading = match m {
                Maneuver::Toward | Maneuver::Decoys(_) => b,
                Maneuver::DecoysBeam(_) if thrown > 0 => beam,
                Maneuver::DecoysBeam(_) => b,
                Maneuver::Away => b + PI,
                Maneuver::Beam => beam,
            };
            if let Maneuver::Decoys(n) | Maneuver::DecoysBeam(n) = m
                && to.length() < 1_500.0
                && thrown < n
            {
                thrown += 1;
                weapons = decoy;
            }
        } else if launched {
            return Some(false);
        }
        sim.step(ap.control(sim.spec(), &fd, DT).with_weapons(weapons));
        if sim
            .weapon_events()
            .iter()
            .any(|(_, e)| matches!(e, WeaponEvent::Hit { on: crate::weapons::Struck::Aircraft, .. }))
        {
            return Some(true);
        }
        if sim.status() != Status::Flying {
            return None;
        }
    }
    launched.then_some(false)
}

/// Treffer und gültige Anflüge (gestartet, nicht abgestürzt) einer Rakete
/// bei einem Manöver über drei Missionen mit je drei Startpunkten.
pub fn hit_rate(kind: WeaponKind, m: Maneuver) -> (usize, usize) {
    let mut hits = (0, 0);
    for seed in SEEDS {
        for (south, west) in [(0.25, 0.3), (0.5, 0.6), (0.75, 0.9)] {
            if let Some(hit) = engagement(seed, kind, south, west, m) {
                hits.0 += usize::from(hit);
                hits.1 += 1;
            }
        }
    }
    hits
}
