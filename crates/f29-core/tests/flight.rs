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
fn hard_pull_stalls_sinks_and_recovers_when_released() {
    // 250 kt in 5000 m: voll ziehen überschreitet den Abrisswinkel schnell.
    let mut sim = high_sim(5_000.0, 130.0, 0.7);
    let mut stalled_at = None;
    for t in 0..ticks(6.0) {
        sim.step(ControlFrame::from_analog(1.0, 0.0, 0.0, 0.7, 0));
        if sim.aircraft().is_stalled() && stalled_at.is_none() {
            stalled_at = Some(t);
        }
    }
    let stalled_at = stalled_at.expect("kein Strömungsabriss trotz vollem Ziehen");
    assert!(stalled_at < ticks(3.0), "Abriss erst nach {stalled_at} Ticks");

    // Im Abriss bricht der Auftrieb ein: kein Lastvielfaches nahe der Grenze möglich.
    assert!(sim.flight_data().load_factor < 3.5);

    // Loslassen: Nase fällt, Strömung liegt wieder an.
    let mut recovered = false;
    for _ in 0..ticks(10.0) {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.7, 0));
        if !sim.aircraft().is_stalled() {
            recovered = true;
            break;
        }
    }
    assert!(recovered, "keine Erholung nach dem Loslassen");
}

#[test]
fn slow_stall_drops_nose_steeply_then_recovers_with_speed() {
    // 160 kt: hart ziehen bis zum Abriss, dann loslassen.
    let mut sim = high_sim(2_500.0, 82.0, 0.7);
    let h0 = sim.flight_data().altitude_m;
    let mut t = 0;
    while !sim.aircraft().is_stalled() {
        sim.step(ControlFrame::from_analog(1.0, 0.0, 0.0, 0.7, 0));
        t += 1;
        assert!(t < ticks(5.0), "kein Abriss");
    }
    // Die Nase kippt steil nach unten – Ziehen hilft dabei kaum.
    let mut min_pitch: f64 = 0.0;
    for _ in 0..ticks(4.0) {
        sim.step(ControlFrame::from_analog(1.0, 0.0, 0.0, 0.7, 0));
        min_pitch = min_pitch.min(sim.flight_data().pitch);
    }
    assert!(min_pitch < -1.0, "Nase nur auf {:.0}° gefallen", min_pitch.to_degrees());
    // Loslassen: im Sturz Fahrt aufnehmen, Abriss endet.
    let mut ended = false;
    for _ in 0..ticks(15.0) {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.7, 0));
        if !sim.aircraft().is_stalled() {
            ended = true;
            break;
        }
    }
    assert!(ended, "Abriss endet nicht");
    let fd = sim.flight_data();
    assert!(fd.tas_mps > 90.0, "Abriss endet ohne ausreichende Fahrt: {:.0} m/s", fd.tas_mps);
    assert!(h0 - fd.altitude_m > 150.0, "kaum Höhenverlust: {:.0} m", h0 - fd.altitude_m);
}

#[test]
fn stall_strength_grows_with_speed_deficit() {
    // Gleich hartes Ziehen: bei hoher Fahrt nur ein leichtes Schütteln an der
    // Grenze, bei niedriger Fahrt ein voller Abriss mit abkippender Nase.
    let pull = |speed: f64| {
        let mut sim = high_sim(1_500.0, speed, 1.0);
        let (mut max_stall, mut min_pitch) = (0.0_f64, 0.0_f64);
        for _ in 0..ticks(4.0) {
            sim.step(ControlFrame::from_analog(1.0, 0.0, 0.0, 1.0, 0));
            max_stall = max_stall.max(sim.aircraft().stall);
            min_pitch = min_pitch.min(sim.flight_data().pitch);
        }
        (max_stall, min_pitch)
    };
    let (fast_stall, fast_pitch) = pull(160.0);
    let (slow_stall, slow_pitch) = pull(90.0);
    assert!(fast_stall < 0.5, "schnell: Abrissstärke {fast_stall:.2}");
    assert!(fast_pitch > -0.35, "schnell: Nase fällt auf {:.0}°", fast_pitch.to_degrees());
    assert!(slow_stall > 0.8, "langsam: Abrissstärke {slow_stall:.2}");
    assert!(slow_pitch < -0.8, "langsam: Nase nur auf {:.0}°", slow_pitch.to_degrees());
}

#[test]
fn stall_strength_changes_smoothly() {
    // Kein Schaltzustand: zwischen zwei Ticks ändert sich die Abrissstärke nur wenig.
    let mut sim = high_sim(2_500.0, 82.0, 0.7);
    let mut last = sim.aircraft().stall;
    for t in 0..ticks(20.0) {
        let pull = if t < ticks(3.0) { 1.0 } else { 0.4 };
        sim.step(ControlFrame::from_analog(pull, 0.0, 0.0, 0.7, 0));
        let now = sim.aircraft().stall;
        assert!((now - last).abs() < 0.05, "Sprung der Abrissstärke {last:.2} → {now:.2} bei Tick {t}");
        last = now;
    }
}

#[test]
fn stall_warning_precedes_stall() {
    let mut sim = high_sim(5_000.0, 130.0, 0.7);
    let mut warned_before = false;
    for _ in 0..ticks(5.0) {
        let fd = sim.flight_data();
        if fd.warnings.stall && !sim.aircraft().is_stalled() {
            warned_before = true;
        }
        if sim.aircraft().is_stalled() {
            break;
        }
        sim.step(ControlFrame::from_analog(0.6, 0.0, 0.0, 0.7, 0));
    }
    assert!(warned_before, "keine Warnung vor dem Abriss");
}

#[test]
fn hard_turn_bleeds_energy() {
    // 70° Querlage, voll ziehen, 10 s: deutlicher Energieverlust (Höhe + Fahrt),
    // selbst mit Vollschub. Wer zu hart zieht, spiralt nach unten.
    let mut sim = high_sim(2_000.0, 180.0, 1.0);
    let energy = |fd: &f29_core::FlightData| fd.altitude_m + fd.tas_mps * fd.tas_mps / (2.0 * 9.81);
    let e0 = energy(&sim.flight_data());
    for _ in 0..ticks(10.0) {
        let fd = sim.flight_data();
        let roll = ((70.0_f64.to_radians() - fd.roll) * 3.0).clamp(-1.0, 1.0);
        sim.step(ControlFrame::from_analog(1.0, roll, 0.0, 1.0, 0));
    }
    let lost = e0 - energy(&sim.flight_data());
    assert!(lost > 300.0, "nur {lost:.0} m Energiehöhe verloren in 10 s harter Kurve");
}

#[test]
fn steep_zoom_climb_ends_in_stall() {
    // Steil hochziehen und halten: die Fahrt reicht nicht, die Strömung reißt ab.
    let mut sim = high_sim(3_000.0, 200.0, 1.0);
    let mut stalled = false;
    for _ in 0..ticks(60.0) {
        let fd = sim.flight_data();
        // 45° Nicklage anstreben und halten
        let pitch = ((45.0_f64.to_radians() - fd.pitch) * 2.0).clamp(-1.0, 1.0);
        sim.step(ControlFrame::from_analog(pitch, 0.0, 0.0, 1.0, 0));
        stalled |= sim.aircraft().is_stalled();
    }
    assert!(stalled, "45°-Steigflug mit Vollschub endet nicht im Abriss");
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
    // Begrenztes Steigvermögen: spürbar, aber deutlich unter einem Jäger.
    assert!(gained > 600.0, "Steigleistung zu gering: {gained:.0} m/min");
    assert!(gained < 3_000.0, "Steigleistung zu hoch: {gained:.0} m/min");

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

#[test]
fn neutral_stick_holds_altitude_even_when_speed_changes() {
    // Waagerecht, Knüppel los, Gas weg und wieder auf: die Höhe bleibt.
    let mut sim = high_sim(4_000.0, 200.0, 0.6);
    let h0 = sim.flight_data().altitude_m;
    let mut max_dev: f64 = 0.0;
    for t in 0..ticks(60.0) {
        let thr = if t < ticks(20.0) { 0.2 } else { 0.9 };
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, thr, 0));
        max_dev = max_dev.max((sim.flight_data().altitude_m - h0).abs());
    }
    assert!(max_dev < 25.0, "Höhenabweichung {max_dev:.1} m im Geradeausflug");
}

#[test]
fn throttle_changes_speed_noticeably() {
    // 10 s Vollgas gegen 10 s Leerlauf aus demselben Reiseflug.
    let run = |thr: f64| {
        let mut sim = high_sim(3_000.0, 200.0, 0.55);
        fly(&mut sim, 10.0, ControlFrame::from_analog(0.0, 0.0, 0.0, thr, 0));
        sim.flight_data().tas_mps
    };
    let (full, idle) = (run(1.0), run(0.0));
    assert!(full - 200.0 > 8.0, "Vollgas: nur +{:.1} m/s in 10 s", full - 200.0);
    assert!(200.0 - idle > 25.0, "Leerlauf: nur −{:.1} m/s in 10 s", 200.0 - idle);
}

#[test]
fn turbulence_only_near_the_ground() {
    let roll_activity = |agl_m: f64| {
        let mut sim = Sim::new(SimConfig {
            seed: SEED,
            spawn: Spawn {
                altitude: 0.0,
                min_clearance: agl_m,
                speed: 220.0,
                throttle: 0.6,
                ..Spawn::default()
            },
        });
        let mut sum = 0.0;
        for _ in 0..ticks(3.0) {
            sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.6, 0));
            sum += sim.turbulence().x.abs();
        }
        sum / ticks(3.0) as f64
    };
    let low = roll_activity(40.0);
    let high = roll_activity(600.0);
    assert!(high < 1e-12, "Turbulenz in 600 m: {high}");
    assert!(low > 0.05, "kaum Turbulenz in 40 m über Grund: {low:.3} rad/s");
}

#[test]
fn flaps_lower_stall_speed_add_drag_and_retract_when_fast() {
    use f29_core::controls::flags;
    // Langsam mit Klappen: weniger Anstellwinkel nötig als ohne.
    let alpha_at = |with_flaps: bool| {
        let mut sim = high_sim(2_000.0, 95.0, 0.8);
        let f = if with_flaps { flags::FLAPS } else { 0 };
        fly(&mut sim, 6.0, ControlFrame::from_analog(0.0, 0.0, 0.0, 0.8, f));
        (sim.flight_data().alpha, sim.flight_data().flaps)
    };
    let (alpha_clean, _) = alpha_at(false);
    let (alpha_flaps, flaps) = alpha_at(true);
    assert!(flaps > 0.99, "Klappen nicht ausgefahren: {flaps}");
    assert!(alpha_flaps < alpha_clean - 0.03, "Klappen senken den Anstellwinkel kaum");

    // Zu schnell: Klappen fahren selbsttätig ein.
    let mut fast = high_sim(2_000.0, 200.0, 0.8);
    fly(&mut fast, 4.0, ControlFrame::from_analog(0.0, 0.0, 0.0, 0.8, flags::FLAPS));
    assert!(fast.flight_data().flaps < 1e-9, "Klappen bei 200 m/s ausgefahren");
}
