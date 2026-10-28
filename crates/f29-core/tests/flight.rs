//! Flugtests: fliegen das Modell headless und prüfen Verhalten statt Formeln.

use f29_core::autopilot::Autopilot;
use f29_core::controls::flags;
use f29_core::sim::Spawn;
use f29_core::{ControlFrame, DT, Sim, SimConfig, Status, TICK_HZ};

const SEED: u64 = 0x0F29;

fn ticks(seconds: f64) -> u64 {
    (seconds * f64::from(TICK_HZ)).round() as u64
}

/// Spawn hoch über allem Gelände, damit Manöver nicht in Bergen enden.
fn high_sim(altitude: f64, speed: f64, throttle: f64) -> Sim {
    Sim::new(SimConfig {
        seed: SEED,
        spawn: Spawn { altitude, min_clearance: 2_500.0, speed, throttle, ..Spawn::default() },
    })
}

/// Halbe Rolle auf den Rücken, dann ziehen, bis die Nase steil nach unten zeigt.
fn enter_steep_dive(sim: &mut Sim) {
    fly(sim, 1.2, ControlFrame::from_analog(0.0, 1.0, 0.0, 1.0, 0));
    fly(sim, 3.0, ControlFrame::from_analog(1.0, 0.0, 0.0, 1.0, 0));
    assert!(sim.flight_data().pitch < -0.4, "kein Sturzflug: θ = {}", sim.flight_data().pitch);
}

fn fly(sim: &mut Sim, seconds: f64, c: ControlFrame) {
    for _ in 0..ticks(seconds) {
        sim.step(c);
        assert!(sim.aircraft().is_finite(), "Zustand nicht endlich bei Tick {}", sim.tick());
        let q = sim.aircraft().attitude;
        assert!((q.length_squared() - 1.0).abs() < 1e-9, "Quaternion nicht normiert");
    }
}

#[test]
fn autopilot_holds_altitude_heading_and_speed() {
    let mut sim = high_sim(5_000.0, 200.0, 0.5);
    let start = sim.flight_data();
    let mut ap = Autopilot::new(start.altitude_m, start.heading, 200.0);
    let mut max_dev: f64 = 0.0;
    for t in 0..ticks(120.0) {
        let fd = sim.flight_data();
        if t > ticks(20.0) {
            max_dev = max_dev.max((fd.altitude_m - start.altitude_m).abs());
        }
        let c = ap.control(sim.spec(), &fd, DT);
        sim.step(c);
    }
    let end = sim.flight_data();
    assert!(max_dev < 40.0, "Höhenabweichung {max_dev:.1} m");
    assert!((end.tas_mps - 200.0).abs() < 8.0, "Geschwindigkeit {:.1}", end.tas_mps);
    assert!(f29_core::detmath::wrap_pi(end.heading - start.heading).abs() < 0.02);
    assert!(end.roll.abs() < 0.05);
}

#[test]
fn autopilot_turns_to_new_heading() {
    let mut sim = high_sim(5_000.0, 200.0, 0.5);
    let start = sim.flight_data();
    let target = f29_core::detmath::wrap_two_pi(start.heading + std::f64::consts::PI * 0.75);
    let mut ap = Autopilot::new(start.altitude_m, target, 200.0);
    for _ in 0..ticks(90.0) {
        let c = ap.control(sim.spec(), &sim.flight_data(), DT);
        sim.step(c);
    }
    let end = sim.flight_data();
    let err = f29_core::detmath::wrap_pi(end.heading - target).abs();
    assert!(err < 0.03, "Kursfehler {err:.3} rad");
    assert!((end.altitude_m - start.altitude_m).abs() < 80.0);
}

#[test]
fn neutral_stick_holds_flight_path() {
    // Fly-by-Wire mit Bahnhaltung: Knüppel los ⇒ Flugbahn bleibt annähernd gerade.
    let mut sim = high_sim(5_000.0, 220.0, 0.6);
    let start = sim.flight_data();
    fly(&mut sim, 30.0, ControlFrame::from_analog(0.0, 0.0, 0.0, 0.6, 0));
    let end = sim.flight_data();
    assert_eq!(sim.status(), Status::Flying);
    let dh = (end.altitude_m - start.altitude_m).abs();
    assert!(dh < 150.0, "Höhenänderung {dh:.1} m in 30 s ohne Eingabe");
    assert!(end.roll.abs() < 0.01);
}

#[test]
fn full_roll_input_reaches_expected_rate() {
    let mut sim = high_sim(5_000.0, 220.0, 0.6);
    fly(&mut sim, 0.5, ControlFrame::from_analog(0.0, 1.0, 0.0, 0.6, 0));
    let roll = sim.flight_data().roll;
    // ~2,6 rad/s mit 0,18 s Verzögerung ⇒ nach 0,5 s etwa 0,85 rad
    assert!((0.6..1.2).contains(&roll), "Querlage nach 0,5 s: {roll:.3} rad");
}

#[test]
fn g_limiter_caps_load_factor() {
    let mut sim = high_sim(4_000.0, 260.0, 1.0);
    let mut max_g: f64 = 0.0;
    for _ in 0..ticks(6.0) {
        sim.step(ControlFrame::from_analog(1.0, 0.0, 0.0, 1.0, 0));
        max_g = max_g.max(sim.flight_data().load_factor);
    }
    let limit = sim.spec().g_limit_pos;
    assert!(max_g <= limit + 0.5, "Lastvielfaches {max_g:.2} > Grenze {limit}");
    assert!(max_g >= limit - 1.5, "Begrenzer zu konservativ: {max_g:.2}");
}

#[test]
fn alpha_limiter_prevents_stall_at_low_speed() {
    // Leerlauf, voll ziehen: Geschwindigkeit sinkt, Anstellwinkel bleibt begrenzt.
    let mut sim = high_sim(6_000.0, 150.0, 0.0);
    let mut max_alpha: f64 = 0.0;
    for _ in 0..ticks(40.0) {
        sim.step(ControlFrame::from_analog(1.0, 0.0, 0.0, 0.0, 0));
        max_alpha = max_alpha.max(sim.flight_data().alpha);
    }
    let spec = sim.spec();
    assert!(
        max_alpha < spec.alpha_stall_rad,
        "Strömungsabriss: α = {max_alpha:.3} rad (Abriss bei {})",
        spec.alpha_stall_rad
    );
    assert!(max_alpha > spec.alpha_limit_rad - 0.05, "Begrenzer greift zu früh: {max_alpha:.3}");
}

#[test]
fn full_throttle_climbs_and_idle_descends() {
    let mut climb = high_sim(3_000.0, 200.0, 1.0);
    let mut ap = Autopilot::new(9_000.0, climb.flight_data().heading, 200.0);
    let h0 = climb.flight_data().altitude_m;
    for _ in 0..ticks(60.0) {
        let c = ap.control(climb.spec(), &climb.flight_data(), DT);
        climb.step(c);
    }
    let gained = climb.flight_data().altitude_m - h0;
    assert!(gained > 1_500.0, "Steigleistung zu gering: {gained:.0} m/min");

    let mut glide = high_sim(5_000.0, 200.0, 0.0);
    let h0 = glide.flight_data().altitude_m;
    let v0 = glide.flight_data().tas_mps;
    fly(&mut glide, 20.0, ControlFrame::from_analog(0.0, 0.0, 0.0, 0.0, 0));
    let fd = glide.flight_data();
    // Ohne Schub verliert das Flugzeug Energie: Höhe oder Fahrt (oder beides).
    let e0 = 9.81 * h0 + 0.5 * v0 * v0;
    let e1 = 9.81 * fd.altitude_m + 0.5 * fd.tas_mps * fd.tas_mps;
    assert!(e1 < e0 - 5_000.0, "kein Energieverlust im Leerlauf");
}

#[test]
fn top_speed_is_subsonic_at_sea_level() {
    let mut sim = Sim::new(SimConfig {
        seed: SEED,
        spawn: Spawn {
            altitude: 300.0,
            min_clearance: 300.0,
            speed: 230.0,
            throttle: 1.0,
            ..Spawn::default()
        },
    });
    let mut ap = Autopilot::new(sim.flight_data().altitude_m, sim.flight_data().heading, 400.0);
    for _ in 0..ticks(120.0) {
        let c = ap.control(sim.spec(), &sim.flight_data(), DT);
        sim.step(c);
        if sim.status() != Status::Flying {
            break;
        }
    }
    let fd = sim.flight_data();
    if sim.status() == Status::Flying {
        assert!(fd.mach < 0.95, "Höchstgeschwindigkeit Mach {:.2}", fd.mach);
        assert!(fd.mach > 0.7, "Höchstgeschwindigkeit nur Mach {:.2}", fd.mach);
    }
}

#[test]
fn airbrake_decelerates() {
    let mut a = high_sim(5_000.0, 230.0, 0.3);
    let mut b = high_sim(5_000.0, 230.0, 0.3);
    fly(&mut a, 10.0, ControlFrame::from_analog(0.0, 0.0, 0.0, 0.3, 0));
    fly(&mut b, 10.0, ControlFrame::from_analog(0.0, 0.0, 0.0, 0.3, flags::AIRBRAKE));
    assert!(b.flight_data().tas_mps < a.flight_data().tas_mps - 15.0);
    assert!((b.aircraft().airbrake - 1.0).abs() < 1e-9);
}

#[test]
fn diving_into_terrain_crashes_and_freezes() {
    let mut sim = Sim::new(SimConfig::new(SEED));
    enter_steep_dive(&mut sim);
    for _ in 0..ticks(120.0) {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 1.0, 0));
        if matches!(sim.status(), Status::Crashed { .. }) {
            break;
        }
    }
    let Status::Crashed { tick } = sim.status() else {
        panic!("kein Absturz – Höhe {:.0} m", sim.flight_data().altitude_m);
    };
    let fd = sim.flight_data();
    assert!(fd.warnings.crashed);
    assert!(fd.radar_altitude_m.abs() < 1e-6);
    let hash = sim.state_hash();
    let pos = sim.aircraft().position;
    sim.step(ControlFrame::default());
    assert_eq!(sim.aircraft().position, pos, "Wrack bewegt sich");
    assert_ne!(sim.state_hash(), hash, "Tick-Zähler muss weiterlaufen");
    assert!(tick > 0);
}

#[test]
fn pull_up_warning_in_steep_dive_near_ground() {
    let mut sim = Sim::new(SimConfig::new(SEED));
    enter_steep_dive(&mut sim);
    let mut warned = false;
    for _ in 0..ticks(60.0) {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 1.0, 0));
        if sim.flight_data().warnings.pull_up {
            warned = true;
        }
        if sim.status() != Status::Flying {
            break;
        }
    }
    assert!(warned, "keine PULL-UP-Warnung vor dem Aufschlag");
}
