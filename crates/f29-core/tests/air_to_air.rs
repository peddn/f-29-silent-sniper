//! Luftkampf (M-022, Etappe 2): Luft-Luft-Raketen gegen Jäger.

mod common;
use common::*;

use f29_core::controls::weapon_flags;
use f29_core::fighters::Mode;
use f29_core::sim::Spawn;
use f29_core::weapons::{Loadout, WeaponEvent, WeaponKind};
use f29_core::{DVec3, Sim, SimConfig};

/// Hoch über der Karte, Schacht 1 mit `kind`, ohne Flugabwehr; ein Jäger
/// `ahead` m voraus kommt entgegen. Schacht gewählt und offen.
fn duel(kind: WeaponKind, ahead: f64) -> Sim {
    let probe = Sim::new(SimConfig::new(SEED));
    let alt = probe.terrain().max_elevation() + 4_000.0;
    let spawn = Spawn {
        altitude: alt,
        heading: 0.0,
        speed: 250.0,
        throttle: 0.8,
        loadout: Loadout([Some(kind), None, None, None]),
        ..Spawn::default()
    };
    let mut sim = Sim::new(SimConfig { seed: SEED, spawn });
    no_air_defense(&mut sim);
    let a = sim.aircraft().position;
    sim.fighters_mut().put_in_air(0, a + DVec3::new(ahead, 0.0, 0.0), std::f64::consts::PI);
    step(&mut sim, 0, weapon_flags::NEXT_BAY | weapon_flags::BAY_DOORS);
    fly(&mut sim, 1.5, 0, 0);
    sim
}

/// Abschuss auf den erfassten Jäger: Die Rakete holt ihn herunter.
fn shoot(sim: &mut Sim) -> bool {
    let t0 = sim.tick();
    step(sim, 0, weapon_flags::RELEASE);
    for _ in 0..ticks(25.0) {
        step(sim, 0, 0);
        if events_since(sim, t0).iter().any(|e| matches!(e, WeaponEvent::FighterHit { fighter: 0, .. })) {
            return true;
        }
    }
    false
}

#[test]
fn sidewinder_shoots_down_an_oncoming_fighter() {
    let mut sim = duel(WeaponKind::Sidewinder, 7_000.0);
    sim.fighters_mut().list[0].decoys = 0;
    let t = sim.flight_data().air_target.expect("Jäger gewählt");
    assert!(t.locked, "nicht erfasst: {:.0} m", t.range);
    assert!(shoot(&mut sim), "kein Treffer");
    fly(&mut sim, 0.5, 0, 0);
    assert_eq!(sim.fighters().list[0].mode, Mode::Falling);
    assert!(sim.flight_data().weapon_messages.iter().any(|m| m.text == "MIG-21 DESTROYED"));
    assert!(sim.flight_data().air_target.is_none_or(|t| t.index != 0), "Abgeschossener bleibt gewählt");
}

/// Außer Reichweite des Suchkopfs gilt der Jäger nicht als erfasst.
#[test]
fn sidewinder_does_not_lock_beyond_its_range() {
    let sim = duel(WeaponKind::Sidewinder, 14_000.0);
    let t = sim.flight_data().air_target.expect("Jäger gewählt");
    assert!(!t.locked, "auf {:.0} m erfasst", t.range);
}

/// Kill rate über 30 Abstände: ohne Täuschkörper trifft jede Rakete, mit
/// ihnen (einer je Rakete) etwa jede zweite.
fn kill_rate(kind: WeaponKind, decoys: bool, from: f64, step_m: f64) -> usize {
    (0..30)
        .filter(|&k| {
            let mut sim = duel(kind, from + step_m * k as f64);
            if !decoys {
                sim.fighters_mut().list[0].decoys = 0;
            }
            sim.flight_data().air_target.is_some_and(|t| t.locked) && shoot(&mut sim)
        })
        .count()
}

#[test]
fn fighter_flares_save_about_every_second_jet() {
    assert_eq!(kill_rate(WeaponKind::Sidewinder, false, 4_000.0, 120.0), 30);
    let kills = kill_rate(WeaponKind::Sidewinder, true, 4_000.0, 120.0);
    println!("AIM-9 gegen Fackeln: {kills}/30");
    assert!((9..=21).contains(&kills), "{kills}/30 Abschüsse");
}

#[test]
fn amraam_shoots_down_a_fighter_far_away() {
    let mut sim = duel(WeaponKind::Amraam, 20_000.0);
    sim.fighters_mut().list[0].decoys = 0;
    let t = sim.flight_data().air_target.expect("Jäger gewählt");
    assert!(t.locked, "nicht erfasst: {:.0} m", t.range);
    assert!(shoot(&mut sim), "kein Treffer");
    assert!(!duel(WeaponKind::Amraam, 38_000.0).flight_data().air_target.is_some_and(|t| t.locked));
}

#[test]
fn fighter_chaff_saves_some_jets_from_the_amraam() {
    let kills = kill_rate(WeaponKind::Amraam, true, 5_000.0, 150.0);
    println!("AIM-120 gegen Düppel: {kills}/30");
    assert!((12..=27).contains(&kills), "{kills}/30 Abschüsse");
}
