//! Feindliche Jäger (M-022) im ganzen Spiel: Alarm durch eine Radarstellung
//! lässt sie starten, mit Meldung; Anzeigedaten.

mod common;
use common::*;

use f29_core::sim::Spawn;
use f29_core::targets::TargetClass;

/// Voll sichtbar über einer SAM-Stellung: Sobald sie das Flugzeug verfolgt,
/// starten Jäger, die Karte sieht sie, und es gibt eine Startmeldung.
#[test]
fn alert_scrambles_fighters_with_a_message() {
    let (mut sim, _) = approach(
        SEED,
        TargetClass::AirDefense,
        15_000.0,
        0.0,
        3_000.0,
        Spawn { visible: true, ..Spawn::default() },
    )
    .expect("Stellung");
    assert!(sim.flight_data().fighters.is_empty());
    let mut message = false;
    for _ in 0..ticks(90.0) {
        step(&mut sim, 0, 0);
        message |= sim.flight_data().weapon_messages.iter().any(|m| m.text.contains("TAKEOFF"));
        if message {
            break;
        }
    }
    assert!(message, "keine Startmeldung");
    fly(&mut sim, 30.0, 0, 0);
    let fd = sim.flight_data();
    assert!(!fd.fighters.is_empty(), "keine Jäger in der Luft");
    assert!(fd.fighters.iter().all(|f| f.label.starts_with("MIG")));
}

/// Das eigene Radar schaltet einen Jäger voraus auf und hält ihn; ohne Radar
/// keine Aufschaltung.
#[test]
fn radar_locks_a_fighter_ahead() {
    let (mut sim, _) = approach(
        SEED,
        TargetClass::AirDefense,
        15_000.0,
        0.0,
        3_000.0,
        Spawn { visible: true, ..Spawn::default() },
    )
    .expect("Stellung");
    let mut locked = false;
    step(&mut sim, f29_core::controls::flags::RADAR_TOGGLE, 0);
    let mut ap = f29_core::autopilot::Autopilot::new(-sim.aircraft().position.z, 0.0, 250.0);
    for _ in 0..ticks(240.0) {
        // Auf den nächsten Jäger zudrehen.
        let fd = sim.flight_data();
        if let Some(f) = fd.fighters.iter().min_by(|a, b| {
            (a.position - fd.position).length().total_cmp(&(b.position - fd.position).length())
        }) {
            let to = f.position - fd.position;
            ap.target_heading = to.y.atan2(to.x);
        }
        let c = ap.control(sim.spec(), &fd, f29_core::DT);
        sim.step(c);
        if let Some(t) = sim.flight_data().air_target {
            locked = true;
            assert!(t.label.starts_with("MIG") && t.range <= 50_000.0 + 1.0);
            break;
        }
    }
    assert!(locked, "kein Jäger aufgeschaltet");
    step(&mut sim, f29_core::controls::flags::RADAR_TOGGLE, 0);
    fly(&mut sim, 1.0, 0, 0);
    assert!(sim.flight_data().air_target.is_none(), "Radar aus, trotzdem aufgeschaltet");
}
