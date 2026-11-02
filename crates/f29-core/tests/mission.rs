//! Auftrag (M-032, M-033): Auftragsarten nach Regelset, Kamera und Foto,
//! vorgeschlagene Beladung, Einsatzregeln im Kalten Krieg.

mod common;
use common::*;

use f29_core::controls::{flags, weapon_flags};
use f29_core::mission::{MissionEvent, Task};
use f29_core::rules::Conflict;
use f29_core::sensors::LookDir;
use f29_core::sim::Spawn;
use f29_core::weapons::{Loadout, WeaponKind};
use f29_core::{ControlFrame, Sim, SimConfig};

fn config(seed: u64, conflict: Conflict) -> SimConfig {
    SimConfig::with(seed, Spawn { conflict, loadout: None, ..Spawn::default() })
}

/// Kalter Krieg: nur Aufklärung; begrenzter Konflikt: beides; die
/// vorgeschlagene Beladung hat die Kamera genau dann, wenn fotografiert wird.
#[test]
fn tasks_follow_the_rules_and_the_loadout_follows_the_tasks() {
    let mut limited = Vec::new();
    for seed in 1..=12 {
        let cold = Sim::new(config(seed, Conflict::ColdWar));
        assert!(cold.mission().objectives().all(|(_, t)| t == Task::Recon), "Seed {seed}");
        let sim = Sim::new(config(seed, Conflict::Limited));
        limited.extend(sim.mission().objectives().map(|(_, t)| t));
        for s in [&cold, &sim] {
            let camera = s.stores().bays.iter().any(|b| b.kind == Some(WeaponKind::Camera));
            assert_eq!(camera, s.mission().has_recon());
        }
    }
    assert!(limited.contains(&Task::Recon) && limited.contains(&Task::Attack), "{limited:?}");
}

/// Aufklärung fliegt bei Tag, auch wenn Nacht verlangt ist.
#[test]
fn recon_flies_by_day() {
    let mut c = config(1, Conflict::ColdWar);
    c.spawn.night = true;
    assert!(!Sim::new(c).night());
    let mut c = config(1, Conflict::War);
    c.spawn.night = true;
    let sim = Sim::new(c);
    assert_eq!(sim.night(), !sim.mission().has_recon());
}

/// Anflug 300 m über dem Aufklärungsziel (Primärziel im Kalten Krieg), `back` m
/// davor, Kamera in Schacht 1 gewählt, Ziel gewählt, Schacht offen.
fn recon_run(back: f64) -> (Sim, usize) {
    let probe = Sim::new(config(1, Conflict::ColdWar));
    let i = probe.mission().primary.expect("Primärziel");
    let t = probe.targets()[i].clone();
    let rest = Spawn {
        min_clearance: 150.0,
        conflict: Conflict::ColdWar,
        loadout: Some(Loadout([Some(WeaponKind::Camera), None, None, None])),
        ..Spawn::default()
    };
    let (mut sim, i) = approach_target(1, i, &t, back, 0.0, 300.0, rest);
    no_air_defense(&mut sim);
    step(&mut sim, flags::RADAR_TOGGLE, weapon_flags::NEXT_BAY | weapon_flags::BAY_DOORS);
    for _ in 0..40 {
        if sim.picks().ground.index == Some(i) {
            break;
        }
        step(&mut sim, ControlFrame::look_flags(LookDir::Forward.code()), 0);
    }
    assert_eq!(sim.picks().ground.index, Some(i), "Ziel gewählt");
    fly(&mut sim, 2.0, 0, 0);
    (sim, i)
}

fn photographed(sim: &Sim) -> bool {
    sim.mission_events().iter().any(|(_, e)| *e == MissionEvent::Photographed(0))
}

/// Foto aus der Nähe mit dem Ziel in der Sichtlinie der Kamera: erledigt;
/// aus zu großer Entfernung zählt es nicht.
#[test]
fn close_photo_of_the_recon_target_counts() {
    let (mut far, _) = recon_run(4_500.0);
    step(&mut far, 0, weapon_flags::RELEASE);
    assert_eq!(far.stores().photos, 1);
    assert!(!photographed(&far) && !far.objective_done(0), "aus 4 km gezählt");

    let (mut sim, i) = recon_run(3_000.0);
    let mut done = false;
    for _ in 0..ticks(8.0) {
        // Auslösen, sobald das Ziel nah genug ist (alle halbe Sekunde ein Foto).
        let t = &sim.targets()[i];
        let a = sim.aircraft().position;
        let near = (t.north - a.x).hypot(t.east - a.y) < 1_400.0;
        let w = if near && sim.tick().is_multiple_of(60) { weapon_flags::RELEASE } else { 0 };
        let framed = sim.flight_data().camera.expect("Kamera gewählt").in_frame;
        step(&mut sim, 0, w);
        // Die Anzeige „IN FRAME“ sagt das gültige Foto voraus.
        if w != 0 {
            assert_eq!(photographed(&sim) && !done, framed && !done, "Anzeige und Foto uneins");
        }
        done |= photographed(&sim);
    }
    assert!(done && sim.objective_done(0), "nicht fotografiert ({} Fotos)", sim.stores().photos);
    assert!(sim.flight_data().objectives[0] == Some((Task::Recon, true)));
    assert!(sim.targets()[i].is_intact(), "die Kamera ist keine Waffe");
}

/// Kalter Krieg: Treffer auf ein Ziel, das uns nicht entdeckt hat, verletzt
/// die Einsatzregeln; im offenen Krieg nicht.
#[test]
fn cold_war_attack_on_an_undetecting_target_breaks_the_rules() {
    let hit = |conflict: Conflict| {
        (1..=20_u64)
            .find_map(|seed| {
                let (i, t) = nth_of(seed, f29_core::targets::TargetClass::Soft)?;
                let rest = Spawn {
                    conflict,
                    min_clearance: 150.0,
                    loadout: Some(Loadout([Some(WeaponKind::AirToGround), None, None, None])),
                    ..Spawn::default()
                };
                let (mut sim, i) = approach_target(seed, i, &t, 8_000.0, 0.0, 300.0, rest);
                no_air_defense(&mut sim);
                step(&mut sim, flags::RADAR_TOGGLE, weapon_flags::NEXT_BAY | weapon_flags::BAY_DOORS);
                for _ in 0..40 {
                    if sim.picks().ground.index == Some(i) {
                        break;
                    }
                    step(&mut sim, ControlFrame::look_flags(LookDir::Forward.code()), 0);
                }
                fly(&mut sim, 2.0, 0, 0);
                step(&mut sim, 0, weapon_flags::RELEASE);
                fly(&mut sim, 30.0, 0, 0);
                (!sim.targets()[i].is_intact()).then(|| sim.flight_data().roe_violations)
            })
            .expect("ein Treffer")
    };
    assert!(hit(Conflict::ColdWar) >= 1);
    assert_eq!(hit(Conflict::War), 0);
}

/// Kamerabild nur bei offenem Schacht und in der Luft.
#[test]
fn camera_needs_open_bay_and_flight() {
    let cam = Some(Loadout([Some(WeaponKind::Camera), None, None, None]));
    let mut on_runway = SimConfig::runway(1);
    on_runway.spawn.loadout = cam;
    let mut ground = Sim::new(on_runway);
    let mut air = Sim::new(SimConfig::with(1, Spawn { loadout: cam, ..Spawn::default() }));
    for sim in [&mut ground, &mut air] {
        step(sim, 0, weapon_flags::NEXT_BAY);
        assert!(!sim.flight_data().camera.expect("gewählt").active, "Schacht zu");
        step(sim, 0, weapon_flags::BAY_DOORS);
        fly(sim, 2.0, 0, 0);
    }
    assert!(air.flight_data().camera.unwrap().active);
    assert!(!ground.flight_data().camera.unwrap().active, "am Boden");
    // Verschluss: Ein zweiter Druck kurz danach macht kein Foto.
    step(&mut air, 0, weapon_flags::RELEASE);
    step(&mut air, 0, weapon_flags::RELEASE);
    assert_eq!(air.stores().photos, 1);
    fly(&mut air, f29_core::weapons::PHOTO_INTERVAL_S, 0, 0);
    step(&mut air, 0, weapon_flags::RELEASE);
    assert_eq!(air.stores().photos, 2);
}

/// Schleudersitz (Strg+E): bis 450 kt gerettet – das Gebiet entscheidet –,
/// schneller tödlich; das Protokoll hält es fest.
#[test]
fn ejection_depends_on_speed_and_territory() {
    let eject = |speed: f64| {
        let mut sim = Sim::new(SimConfig::with(1, Spawn { speed, ..Spawn::default() }));
        step(&mut sim, 0, weapon_flags::EJECT);
        sim
    };
    let slow = eject(200.0);
    let side = slow.territory(slow.aircraft().position);
    assert_eq!(slow.status(), f29_core::Status::Ejected { tick: 1, side, survived: true });
    assert!(slow.flight_data().debrief.iter().any(|l| l.contains("EJECTED")));
    assert!(matches!(eject(260.0).status(), f29_core::Status::Ejected { survived: false, .. }));
}
