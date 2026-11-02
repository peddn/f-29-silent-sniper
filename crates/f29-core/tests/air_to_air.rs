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
    duel_with(kind, 0, ahead, false)
}

/// Wie [`duel`] mit Jäger `jet` (0 MiG-21, 3 MiG-29); `visible`: für sein
/// Radar voll sichtbar.
fn duel_with(kind: WeaponKind, jet: usize, ahead: f64, visible: bool) -> Sim {
    duel_in(kind, jet, ahead, Spawn { visible, invulnerable: !visible, ..Spawn::default() })
}

/// Wie [`duel_with`] mit Startort, Regelset usw. aus `base`.
fn duel_in(kind: WeaponKind, jet: usize, ahead: f64, base: Spawn) -> Sim {
    let probe = Sim::new(SimConfig::new(SEED));
    let alt = probe.terrain().max_elevation() + 4_000.0;
    let spawn = Spawn {
        altitude: alt,
        heading: 0.0,
        speed: 250.0,
        throttle: 0.8,
        loadout: Some(Loadout([Some(kind), None, None, None])),
        ..base
    };
    let mut sim = Sim::new(SimConfig::with(SEED, spawn));
    no_air_defense(&mut sim);
    let a = sim.aircraft().position;
    sim.fighters_mut().put_in_air(jet, a + DVec3::new(ahead, 0.0, 0.0), std::f64::consts::PI);
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

/// Erster Raketenstart eines Jägers nach `t0`: (Rakete, Entfernung).
fn first_air_launch(sim: &mut Sim, seconds: f64) -> Option<(WeaponKind, f64)> {
    let t0 = sim.tick();
    for _ in 0..ticks(seconds) {
        step(sim, 0, 0);
        let launch = events_since(sim, t0).iter().find_map(|e| match e {
            WeaponEvent::AirLaunch { kind, fighter, .. } => Some((*kind, *fighter)),
            _ => None,
        });
        if let Some((kind, f)) = launch {
            return Some((kind, (sim.fighters().list[f].position - sim.aircraft().position).length()));
        }
    }
    None
}

/// Sieht sein Radar das Flugzeug vor der Nase, startet der Jäger seine
/// Rakete in deren Reichweite: die MiG-21 eine AA-8, die MiG-29 eine AA-10
/// auf größere Entfernung; mit Meldung.
#[test]
fn fighters_fire_their_missiles_in_range() {
    let mut sim = duel_with(WeaponKind::Sidewinder, 0, 14_000.0, true);
    let (kind, range) = first_air_launch(&mut sim, 30.0).expect("MiG-21 schießt nicht");
    assert_eq!(kind, WeaponKind::Aphid);
    assert!((1_000.0..=7_000.0).contains(&range), "AA-8 auf {range:.0} m");
    assert!(sim.flight_data().weapon_messages.iter().any(|m| m.text == "MISSILE LAUNCH AA-8 - MIG-21"));
    let mut sim = duel_with(WeaponKind::Sidewinder, 3, 34_000.0, true);
    let (kind, range) = first_air_launch(&mut sim, 30.0).expect("MiG-29 schießt nicht");
    assert_eq!(kind, WeaponKind::Alamo);
    assert!((7_000.0..=25_000.0).contains(&range), "AA-10 auf {range:.0} m");
}

/// Ohne Radarkontakt (Stealth) startet kein Jäger.
#[test]
fn stealthy_aircraft_draws_no_fire() {
    let mut sim = duel_with(WeaponKind::Sidewinder, 3, 34_000.0, false);
    assert!(first_air_launch(&mut sim, 20.0).is_none());
}

/// Ein Jäger dicht hinter dem Flugzeug schießt mit der Kanone: Jeder Treffer
/// zählt mit der Stärke seiner Kanone, mit Meldung.
#[test]
fn fighter_gun_hits_with_its_strength() {
    let mut sim = duel_with(WeaponKind::Sidewinder, 0, 1.0, true);
    let a = sim.aircraft().position;
    sim.fighters_mut().put_in_air(0, a - DVec3::new(900.0, 0.0, 0.0), 0.0);
    fly(&mut sim, 15.0, 0, 0);
    let s = sim.systems();
    assert!(s.hits > 0, "keine Kanonentreffer");
    let strength = sim.fighters().list[0].spec().gun.strength;
    assert!((s.damage - strength * f64::from(s.hits)).abs() < 1e-9);
    let hit = f29_core::sim::Event::Fighter(f29_core::fighters::FighterEvent::GunHit {
        kind: f29_core::fighters::FighterKind::MiG21,
    });
    assert!(sim.events().iter().any(|(_, e)| *e == hit));
}

/// Kommt eine Rakete nah, dreht der Jäger querab zu ihr.
#[test]
fn fighters_turn_away_from_missiles() {
    let course = |jet: usize, kind: WeaponKind| {
        let mut sim = duel_with(kind, jet, 7_000.0, false);
        sim.fighters_mut().list[jet].decoys = 0;
        let h0 = sim.fighters().list[jet].heading;
        step(&mut sim, 0, weapon_flags::RELEASE);
        fly(&mut sim, 3.5, 0, 0);
        (sim.fighters().list[jet].heading - h0).abs()
    };
    for jet in [0, 3] {
        let turn = course(jet, WeaponKind::Sidewinder);
        assert!(turn > 0.05, "Jäger {jet} dreht nicht: {turn:.3} rad");
    }
}

/// Kalter Krieg: Über eigenem Gebiet feuert der Jäger nicht, über
/// feindlichem sehr wohl; im offenen Krieg überall.
#[test]
fn cold_war_fighters_fire_only_over_hostile_territory() {
    use f29_core::rules::Conflict;
    let probe = Sim::new(SimConfig::new(SEED));
    let home = probe.terrain().runways()[0].center;
    let foe = probe
        .terrain()
        .runways()
        .iter()
        .find(|r| r.kind == f29_core::terrain::AirfieldKind::Enemy)
        .unwrap()
        .center;
    let fires = |conflict: Conflict, at: [f64; 2]| {
        let base = Spawn { north: at[0], east: at[1], visible: true, conflict, ..Spawn::default() };
        let mut sim = duel_in(WeaponKind::Sidewinder, 3, 30_000.0, base);
        // Anfänger schießen auf größte Entfernung: Hier zählt nur das Regelset.
        sim.fighters_mut().list[3].skill = f29_core::fighters::Skill::Rookie;
        let hostile = sim.flight_data().territory == f29_core::Side::Hostile;
        (first_air_launch(&mut sim, 20.0).is_some(), hostile)
    };
    assert_eq!(fires(Conflict::ColdWar, home), (false, false), "über eigenem Gebiet geschossen");
    assert_eq!(fires(Conflict::War, home), (true, false), "offener Krieg: schießt überall");
    let far = [foe[0] - 20_000.0, foe[1]];
    assert_eq!(fires(Conflict::ColdWar, far), (true, true), "über feindlichem Gebiet nicht geschossen");
}
