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
