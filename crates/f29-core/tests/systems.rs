//! Bordsysteme (Gerüst für das Schadensmodell): Jeder Systemzustand wirkt an
//! seiner Stelle; intakt ändert sich nichts.

use f29_core::controls::{flags, weapon_flags};
use f29_core::sensors::LookDir;
use f29_core::systems::{Condition, System};
use f29_core::weapons::{self, WeaponEvent};
use f29_core::{ControlFrame, Sim, SimConfig, TICK_HZ};

const SEED: u64 = 0x0F29;

fn ticks(s: f64) -> usize {
    (s * f64::from(TICK_HZ)) as usize
}

fn fly(sim: &mut Sim, seconds: f64, frame: ControlFrame) {
    for _ in 0..ticks(seconds) {
        sim.step(frame);
    }
}

fn level(throttle: f64, flags: u8) -> ControlFrame {
    ControlFrame::from_analog(0.0, 0.0, 0.0, throttle, flags)
}

fn with(system: System, condition: Condition) -> Sim {
    let mut sim = Sim::new(SimConfig::new(SEED));
    sim.set_condition(system, condition);
    sim
}

#[test]
fn intact_by_default_and_damage_changes_the_hash() {
    let sim = Sim::new(SimConfig::new(SEED));
    assert!(!sim.systems().any_damage());
    assert!(!sim.flight_data().systems.any_damage());
    let damaged = with(System::Optics, Condition::Degraded(0.5));
    assert_ne!(sim.state_hash(), damaged.state_hash());
}

#[test]
fn engine_damage_cuts_thrust() {
    let mut ok = Sim::new(SimConfig::new(SEED));
    let mut weak = with(System::Engine, Condition::Degraded(0.5));
    let mut dead = with(System::Engine, Condition::Failed);
    for s in [&mut ok, &mut weak, &mut dead] {
        fly(s, 20.0, level(1.0, 0));
    }
    let (a, b, c) = (ok.flight_data(), weak.flight_data(), dead.flight_data());
    assert!(b.thrust_n < a.thrust_n * 0.6, "Schub {:.0} vs {:.0}", b.thrust_n, a.thrust_n);
    // Wie im Original: Die Triebwerksstellung selbst fällt auf den
    // verfügbaren Höchstschub, auch bei Vollgas.
    assert!((b.engine - 0.5).abs() < 0.01, "Triebwerk {:.2}", b.engine);
    assert!(c.thrust_n <= 0.0, "ausgefallen noch {:.0} N", c.thrust_n);
    assert!(b.tas_mps < a.tas_mps && c.tas_mps < b.tas_mps);
}

#[test]
fn fuel_leak_drains_the_tanks() {
    let mut ok = Sim::new(SimConfig::new(SEED));
    let mut leak = with(System::Fuel, Condition::Failed);
    fly(&mut ok, 10.0, level(0.6, 0));
    fly(&mut leak, 10.0, level(0.6, 0));
    let extra = ok.flight_data().fuel_kg - leak.flight_data().fuel_kg;
    let expected = f29_core::systems::FUEL_LEAK_MAX_KGPS * 10.0;
    assert!((extra - expected).abs() < expected * 0.05, "Leck {extra:.1} kg statt {expected:.1}");
}

#[test]
fn failed_hydraulics_block_gear_and_flaps() {
    let mut sim = with(System::Hydraulics, Condition::Failed);
    fly(&mut sim, 5.0, level(0.6, flags::GEAR | flags::FLAPS));
    let fd = sim.flight_data();
    assert_eq!(fd.gear, 0.0);
    assert_eq!(fd.flaps, 0.0);
    let mut slow = with(System::Hydraulics, Condition::Degraded(0.5));
    let mut ok = Sim::new(SimConfig::new(SEED));
    fly(&mut slow, 2.0, level(0.6, flags::GEAR));
    fly(&mut ok, 2.0, level(0.6, flags::GEAR));
    let (g_slow, g_ok) = (slow.flight_data().gear, ok.flight_data().gear);
    assert!(g_slow > 0.0 && g_slow < g_ok * 0.6, "Fahrwerk {g_slow:.2} vs {g_ok:.2}");
}

#[test]
fn flight_control_damage_slows_the_roll_but_keeps_a_backup() {
    let roll_after = |sim: &mut Sim| {
        fly(sim, 1.0, ControlFrame::from_analog(0.0, 1.0, 0.0, 0.6, 0));
        sim.flight_data().roll.abs()
    };
    let ok = roll_after(&mut Sim::new(SimConfig::new(SEED)));
    let failed = roll_after(&mut with(System::FlightControls, Condition::Failed));
    assert!(failed < ok * 0.5, "Rollwinkel {failed:.2} vs {ok:.2}");
    assert!(failed > ok * 0.1, "Notsteuerung fehlt: {failed:.3}");
}

#[test]
fn failed_radar_stays_off() {
    let mut sim = Sim::new(SimConfig::new(SEED));
    sim.step(level(0.6, flags::RADAR_TOGGLE));
    assert!(sim.sensors().radar_on);
    sim.set_condition(System::Radar, Condition::Failed);
    assert!(!sim.sensors().radar_on, "Ausfall schaltet das Radar ab");
    sim.step(level(0.6, flags::RADAR_TOGGLE));
    assert!(!sim.sensors().radar_on, "lässt sich nicht wieder einschalten");
}

#[test]
fn sensor_damage_shortens_the_range() {
    let mut sim = Sim::new(SimConfig::new(SEED));
    sim.set_condition(System::Radar, Condition::Degraded(0.25));
    sim.set_condition(System::Optics, Condition::Failed);
    sim.step(level(0.6, flags::RADAR_TOGGLE));
    for look in [LookDir::Right, LookDir::Aft, LookDir::Left, LookDir::Forward] {
        sim.step(level(0.6, ControlFrame::look_flags(look.code())));
        if let Some(t) = sim.flight_data().target {
            assert!(!t.optical, "Optik ausgefallen, trotzdem optisch erfasst");
            assert!(t.range <= f29_core::sensors::RADAR_RANGE_M * 0.25 + 1.0, "{:.0} m", t.range);
        }
    }
}

#[test]
fn failed_bay_jams_the_doors_and_blocks_release() {
    let mut sim = with(System::WeaponBay, Condition::Failed);
    sim.step(level(0.6, 0).with_weapons(weapon_flags::BAY_DOORS | weapon_flags::NEXT_BAY));
    fly(&mut sim, weapons::DOOR_TIME_S + 0.5, level(0.6, 0));
    assert_eq!(sim.stores().door, 0.0, "Klappen klemmen");
    sim.step(level(0.6, 0).with_weapons(weapon_flags::RELEASE));
    assert!(sim.weapon_events().iter().any(|(_, e)| *e == WeaponEvent::BayFault));
    assert!(sim.weapons_in_flight().is_empty());
}
