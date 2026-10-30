//! Bordkanone (M-016) in der Simulation: eigene Feuertaste unabhängig von
//! Schacht und Klappen, kein Feuer am Boden, Anzeigedaten, Determinismus.

use f29_core::controls::{flags, weapon_flags};
use f29_core::gun;
use f29_core::sim::Spawn;
use f29_core::{ControlFrame, Sim, SimConfig, TICK_HZ};

fn air() -> Sim {
    Sim::new(SimConfig { seed: 0x0F29, spawn: Spawn::default() })
}

fn hold(sim: &mut Sim, seconds: f64, w: u16) {
    for _ in 0..(seconds * f64::from(TICK_HZ)) as usize {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.7, 0).with_weapons(w));
    }
}

#[test]
fn gun_fires_without_bay_doors() {
    let mut sim = air();
    assert!(sim.stores().door < 1e-9, "Schacht zu");
    assert_eq!(sim.flight_data().gun.rounds, gun::ROUNDS);
    hold(&mut sim, 1.0, weapon_flags::GUN);
    let fd = sim.flight_data();
    assert!(fd.gun.firing);
    let fired = gun::ROUNDS - fd.gun.rounds;
    assert!((49..=51).contains(&fired), "{fired}");
    assert!(!sim.gun().in_flight.is_empty(), "Patronen im Flug");
    // Loslassen: Feuer hört auf.
    hold(&mut sim, 0.5, 0);
    assert!(!sim.flight_data().gun.firing);
    assert_eq!(sim.flight_data().gun.rounds, gun::ROUNDS - fired);
}

#[test]
fn no_gun_on_the_ground() {
    let mut sim = Sim::new(SimConfig::runway(0x0F29));
    for _ in 0..60 {
        let c = ControlFrame::from_analog(0.0, 0.0, 0.0, 0.0, flags::GEAR).with_weapons(weapon_flags::GUN);
        sim.step(c);
    }
    assert!(sim.aircraft().on_ground);
    assert_eq!(sim.flight_data().gun.rounds, gun::ROUNDS);
}

#[test]
fn empty_gun_reports_on_new_trigger_pull() {
    let mut sim = air();
    hold(&mut sim, 13.0, weapon_flags::GUN);
    assert_eq!(sim.flight_data().gun.rounds, 0);
    hold(&mut sim, 0.1, 0);
    hold(&mut sim, 0.1, weapon_flags::GUN);
    let msgs = sim.flight_data().weapon_messages;
    assert!(msgs.iter().any(|m| m.text == "GUN EMPTY" && m.warning), "{msgs:?}");
}

#[test]
fn gun_is_deterministic() {
    let run = || {
        let mut sim = air();
        hold(&mut sim, 0.7, weapon_flags::GUN);
        hold(&mut sim, 0.3, 0);
        hold(&mut sim, 1.2, weapon_flags::GUN);
        hold(&mut sim, 2.0, 0);
        sim.state_hash()
    };
    assert_eq!(run(), run());
    // Und die Kanone gehört zum Zustand.
    let mut quiet = air();
    hold(&mut quiet, 4.2, 0);
    assert_ne!(run(), quiet.state_hash());
}

/// Die Kanone ist die Startwahl: Der gehaltene Abzug der gewählten Waffe
/// feuert sie; nach dem Weiterschalten zu einem Schacht nicht mehr.
#[test]
fn trigger_fires_the_selected_gun() {
    let mut sim = air();
    assert!(sim.stores().gun_selected);
    hold(&mut sim, 0.5, weapon_flags::TRIGGER | weapon_flags::RELEASE);
    let after = sim.flight_data().gun.rounds;
    assert!(after < gun::ROUNDS - 20, "{after}");
    assert!(sim.weapons_in_flight().is_empty(), "keine Schachtwaffe");
    hold(&mut sim, 0.1, 0);
    hold(&mut sim, 1.0 / f64::from(TICK_HZ), weapon_flags::NEXT_BAY);
    hold(&mut sim, 0.5, weapon_flags::TRIGGER);
    assert_eq!(sim.flight_data().gun.rounds, after, "Schacht gewählt: Abzug feuert die Kanone nicht");
    // Rücktaste feuert die Kanone immer.
    hold(&mut sim, 0.2, weapon_flags::GUN);
    assert!(sim.flight_data().gun.rounds < after);
}
