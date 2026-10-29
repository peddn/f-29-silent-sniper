//! Flugplätze, Fahrwerk, Landungen und Treibstoff im Zusammenspiel.

use f29_core::autopilot::{AIM_POINT_M, Approach, GLIDE_SLOPE};
use f29_core::controls::flags;
use f29_core::sim::{CrashCause, Spawn, SpawnKind};
use f29_core::terrain::{AirfieldKind, Runway};
use f29_core::{ControlFrame, DT, Sim, SimConfig, Status};

const SEED: u64 = 0x0F29;

fn ticks(seconds: f64) -> usize {
    (seconds / DT).round() as usize
}

fn runway(seed: u64, kind: AirfieldKind) -> Runway {
    let sim = Sim::new(SimConfig::new(seed));
    sim.terrain().runways().iter().find(|r| r.kind == kind).expect("Bahn").clone()
}

/// Luftstart im Endanflug: `dist` Meter vor dem Aufsetzpunkt auf dem Gleitpfad,
/// seitlich um `offset` versetzt.
fn on_final(seed: u64, rw: &Runway, dist: f64, offset: f64, fuel: f64) -> Sim {
    let u = -rw.length_m * 0.5 + AIM_POINT_M - dist;
    let p = rw.world(u, offset);
    let spawn = Spawn {
        kind: SpawnKind::Air,
        fuel,
        north: p[0],
        east: p[1],
        altitude: rw.elevation_m + 2.2 + dist * GLIDE_SLOPE,
        min_clearance: 0.0,
        heading: rw.heading_deg.to_radians(),
        speed: 95.0,
        throttle: 0.3,
    };
    Sim::new(SimConfig { seed, spawn })
}

/// Fliegt den Anflug, bis das Flugzeug steht oder abstürzt.
fn fly_approach(sim: &mut Sim, ap: &mut Approach, seconds: f64) {
    for _ in 0..ticks(seconds) {
        let fd = sim.flight_data();
        if fd.stopped_on_runway || sim.status() != Status::Flying {
            return;
        }
        let c = ap.control(sim.spec(), &fd, DT);
        sim.step(c);
    }
}

fn crash_cause(sim: &Sim) -> Option<CrashCause> {
    match sim.status() {
        Status::Crashed { cause, .. } => Some(cause),
        Status::Flying => None,
    }
}

#[test]
fn runway_start_stands_still_with_gear_down() {
    let mut sim = Sim::new(SimConfig::runway(SEED));
    let fd = sim.flight_data();
    assert!(fd.on_ground && fd.stopped_on_runway);
    assert_eq!(fd.runway_name, Some("HOME"));
    assert!(fd.radar_altitude_m.abs() < 1e-9);
    assert_eq!(fd.gear, 1.0);
    // Leerlauf, Bremse: steht nach 10 s noch genauso da
    let p = sim.aircraft().position;
    for _ in 0..ticks(10.0) {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.0, flags::GEAR | flags::AIRBRAKE));
    }
    assert_eq!(sim.status(), Status::Flying);
    assert!((sim.aircraft().position - p).length() < 1e-6);
    assert!(!sim.flight_data().warnings.stall, "keine Abrisswarnung am Boden");
}

#[test]
fn takeoff_from_home_and_climb_out() {
    let mut sim = Sim::new(SimConfig::runway(SEED));
    let mut airborne_at = None;
    for i in 0..ticks(90.0) {
        let fd = sim.flight_data();
        // Pilot: rotieren und dann etwa 12° Nicklage halten.
        let pull = if fd.tas_mps > 1.1 * fd.stall_speed_mps {
            ((0.21 - fd.pitch) * 4.0).clamp(-0.5, 0.6)
        } else {
            0.0
        };
        let gear = if fd.radar_altitude_m > 30.0 { 0 } else { flags::GEAR };
        sim.step(ControlFrame::from_analog(pull, 0.0, 0.0, 1.0, gear | flags::FLAPS));
        if airborne_at.is_none() && !sim.aircraft().on_ground {
            airborne_at = Some(i);
            // Abheben noch auf der Bahn
            let p = sim.aircraft().position;
            assert!(sim.terrain().runway_at(p.x, p.y).is_some(), "erst hinter der Bahn abgehoben");
        }
        assert_eq!(sim.status(), Status::Flying, "Absturz nach {} s", i as f64 * DT);
    }
    let t = airborne_at.expect("nicht abgehoben") as f64 * DT;
    assert!((10.0..45.0).contains(&t), "Abheben nach {t} s");
    let fd = sim.flight_data();
    assert!(fd.radar_altitude_m > 150.0, "kein Steigflug: {} m", fd.radar_altitude_m);
    assert!(fd.gear < 0.05, "Fahrwerk nicht eingefahren");
}

#[test]
fn autopilot_lands_on_home_runway() {
    let rw = runway(SEED, AirfieldKind::Home);
    let mut sim = on_final(SEED, &rw, 8_000.0, 0.0, 0.5);
    let mut ap = Approach::new(rw.clone());
    fly_approach(&mut sim, &mut ap, 240.0);
    assert_eq!(sim.status(), Status::Flying, "Absturz: {:?}", crash_cause(&sim));
    let fd = sim.flight_data();
    assert!(fd.stopped_on_runway, "steht nicht auf der Bahn");
    let td = fd.touchdown.expect("Aufsetzen nicht erfasst");
    assert!(!td.hard, "harte Landung: {} m/s", td.sink_mps);
    assert!(td.sink_mps > 0.0 && td.sink_mps < 2.5, "Sinkrate {}", td.sink_mps);
    assert_eq!(fd.runway_name, Some("HOME"));
}

#[test]
fn land_on_secret_strip_turn_around_and_take_off_again() {
    let rw = runway(SEED, AirfieldKind::Secret);
    let mut sim = on_final(SEED, &rw, 6_000.0, 0.0, 0.45);
    let mut ap = Approach::new(rw.clone());
    fly_approach(&mut sim, &mut ap, 240.0);
    assert!(sim.flight_data().stopped_on_runway, "Landung auf der Piste: {:?}", sim.status());
    let landed_u = rw.local(sim.aircraft().position.x, sim.aircraft().position.y).0;
    assert!(landed_u < rw.length_m * 0.5, "über das Ende hinaus");

    // Zum Ende rollen, auf der Stelle wenden …
    let to_end = |sim: &Sim| {
        rw.length_m * 0.5 - 40.0 - rw.local(sim.aircraft().position.x, sim.aircraft().position.y).0
    };
    for _ in 0..ticks(120.0) {
        if to_end(&sim) < 0.0 {
            break;
        }
        let v = sim.aircraft().velocity.length();
        let thr = if v < 10.0 { 0.25 } else { 0.0 };
        let brake = if v > 12.0 { flags::AIRBRAKE } else { 0 };
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, thr, flags::GEAR | brake));
    }
    for _ in 0..ticks(15.0) {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.0, flags::GEAR | flags::AIRBRAKE));
    }
    let start_hdg = sim.flight_data().heading;
    for _ in 0..ticks(12.0) {
        let fd = sim.flight_data();
        let back = f29_core::detmath::wrap_pi(start_hdg + std::f64::consts::PI - fd.heading);
        let steer = (back * 2.0).clamp(-1.0, 1.0);
        sim.step(ControlFrame::from_analog(0.0, steer, 0.0, 0.0, flags::GEAR | flags::AIRBRAKE));
    }
    assert_eq!(sim.status(), Status::Flying, "beim Wenden: {:?}", crash_cause(&sim));
    let p = sim.aircraft().position;
    assert!(sim.terrain().runway_at(p.x, p.y).is_some(), "beim Wenden von der Piste");

    // … und in Gegenrichtung wieder starten.
    for _ in 0..ticks(60.0) {
        let fd = sim.flight_data();
        let pull = if fd.tas_mps > 1.08 * fd.stall_speed_mps { 0.7 } else { 0.0 };
        sim.step(ControlFrame::from_analog(pull, 0.0, 0.0, 1.0, flags::GEAR | flags::FLAPS));
        if sim.status() != Status::Flying {
            break;
        }
    }
    assert_eq!(sim.status(), Status::Flying, "Start von der Piste: {:?}", crash_cause(&sim));
    assert!(sim.flight_data().radar_altitude_m > 50.0, "nicht abgehoben");
}

#[test]
fn gear_up_landing_crashes() {
    let rw = runway(SEED, AirfieldKind::Home);
    let mut sim = on_final(SEED, &rw, 5_000.0, 0.0, 0.5);
    let mut ap = Approach::new(rw);
    ap.gear = false;
    fly_approach(&mut sim, &mut ap, 200.0);
    assert_eq!(crash_cause(&sim), Some(CrashCause::GearUp));
}

#[test]
fn hard_and_off_runway_touchdowns_crash() {
    // Steil auf die Bahn gedrückt: Fahrwerk bricht
    let rw = runway(SEED, AirfieldKind::Home);
    let mut sim = on_final(SEED, &rw, 1_500.0, 0.0, 0.5);
    for _ in 0..ticks(60.0) {
        sim.step(ControlFrame::from_analog(-0.4, 0.0, 0.0, 0.3, flags::GEAR | flags::FLAPS));
        if sim.status() != Status::Flying {
            break;
        }
    }
    assert!(
        matches!(crash_cause(&sim), Some(CrashCause::GearCollapse | CrashCause::Terrain)),
        "{:?}",
        sim.status()
    );

    // Sauberer Anflug, aber 400 m neben der Bahn: Landung im Gelände
    let mut sim = on_final(SEED, &rw, 5_000.0, 400.0, 0.5);
    let mut ap = Approach::new(rw.clone());
    for _ in 0..ticks(200.0) {
        let fd = sim.flight_data();
        let mut c = ap.control(sim.spec(), &fd, DT);
        c.roll = 0; // kein Korrigieren auf die Mittellinie
        sim.step(c);
        if sim.status() != Status::Flying {
            break;
        }
    }
    assert!(matches!(crash_cause(&sim), Some(CrashCause::Terrain | CrashCause::Water)), "{:?}", sim.status());
}

#[test]
fn route_leads_over_the_secret_strip_back_home() {
    for seed in [SEED, 1, 77] {
        let sim = Sim::new(SimConfig::new(seed));
        let route = sim.route();
        let n = route.len();
        assert!(n >= 3);
        let strip = &route[n - 2];
        let home = &route[n - 1];
        assert_eq!(home.name, "HOME");
        let si = strip.runway.expect("Piste als Navigationspunkt");
        assert_eq!(sim.terrain().runways()[si].kind, AirfieldKind::Secret);
        let rw = &sim.terrain().runways()[si];
        assert_eq!((strip.north, strip.east), (rw.center[0], rw.center[1]));
    }
}

#[test]
fn fuel_burns_and_weight_matters() {
    let high = |fuel: f64| {
        Sim::new(SimConfig {
            seed: SEED,
            spawn: Spawn {
                altitude: 6_000.0,
                min_clearance: 2_500.0,
                speed: 200.0,
                throttle: 1.0,
                fuel,
                ..Spawn::default()
            },
        })
    };
    // Vollgas eine Minute: rund 4 kg/s (Höhe senkt den Verbrauch)
    let mut sim = high(1.0);
    let f0 = sim.flight_data().fuel_kg;
    for _ in 0..ticks(60.0) {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 1.0, 0));
    }
    let burned = f0 - sim.flight_data().fuel_kg;
    assert!((100.0..260.0).contains(&burned), "Verbrauch {burned} kg/min");
    let fd = sim.flight_data();
    assert!(fd.endurance_s > 15.0 * 60.0 && fd.endurance_s < 60.0 * 60.0, "Restzeit {}", fd.endurance_s);

    // Leichtes Flugzeug steigt besser
    let climb = |fuel: f64| {
        let mut sim = high(fuel);
        let h0 = sim.flight_data().altitude_m;
        for _ in 0..ticks(30.0) {
            sim.step(ControlFrame::from_analog(0.25, 0.0, 0.0, 1.0, 0));
        }
        sim.flight_data().altitude_m - h0
    };
    let (heavy, light) = (climb(1.0), climb(0.1));
    assert!(light > heavy + 40.0, "leicht {light} m, schwer {heavy} m");
}

#[test]
fn flameout_glides_and_warns() {
    let mut sim = Sim::new(SimConfig {
        seed: SEED,
        spawn: Spawn {
            altitude: 6_000.0,
            min_clearance: 2_500.0,
            speed: 200.0,
            throttle: 1.0,
            fuel: 0.002,
            ..Spawn::default()
        },
    });
    for _ in 0..ticks(20.0) {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 1.0, 0));
    }
    let fd = sim.flight_data();
    assert!(fd.warnings.flameout && fd.warnings.fuel_low);
    assert_eq!(fd.fuel_kg, 0.0);
    assert!(fd.engine < 0.01, "Triebwerk dreht noch: {}", fd.engine);
    assert_eq!(sim.status(), Status::Flying, "segelt weiter");
}

#[test]
fn bingo_when_far_from_home_with_little_fuel() {
    let far = |fuel: f64| {
        let mut sim = Sim::new(SimConfig {
            seed: SEED,
            spawn: Spawn {
                north: 120_000.0,
                altitude: 6_000.0,
                min_clearance: 2_500.0,
                speed: 200.0,
                fuel,
                ..Spawn::default()
            },
        });
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.6, 0));
        sim.flight_data().warnings.bingo
    };
    assert!(!far(1.0));
    assert!(far(0.15));
}

#[test]
fn gear_overspeed_buffets_and_warns() {
    let mut sim = Sim::new(SimConfig {
        seed: SEED,
        spawn: Spawn {
            altitude: 3_000.0,
            min_clearance: 2_500.0,
            speed: 200.0,
            throttle: 0.8,
            ..Spawn::default()
        },
    });
    for _ in 0..ticks(6.0) {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.8, flags::GEAR));
    }
    let fd = sim.flight_data();
    assert!(fd.warnings.gear_overspeed);
    assert!(fd.turbulence > 0.05, "kein Rütteln: {}", fd.turbulence);
}

/// Spielertest: Vollgas und von Anfang an voll gezogen (Tastatur). Direkt
/// nach dem Abheben darf die Strömung nicht abreißen; wer bei 15° Nicklage
/// loslässt, steigt normal weiter. Wer dagegen voll gezogen hält, steigt
/// immer steiler und reißt erst ab, wenn die Fahrt verbraucht ist – hoch über
/// der Bahn.
#[test]
fn full_pull_takeoff_does_not_stall_near_the_ground() {
    for flap in [0, flags::FLAPS] {
        for hold in [false, true] {
            let mut sim = Sim::new(SimConfig::runway(SEED));
            let mut stick = 0.0_f64;
            let mut released = false;
            let mut first_stall_agl = None;
            let mut max_agl = 0.0_f64;
            for _ in 0..ticks(60.0) {
                let fd = sim.flight_data();
                released |= !hold && !fd.on_ground && fd.pitch > 0.26;
                let target = if released { 0.0 } else { 1.0 };
                stick += (target - stick).clamp(-3.0 * DT, 3.0 * DT);
                sim.step(ControlFrame::from_analog(stick, 0.0, 0.0, 1.0, flags::GEAR | flap));
                max_agl = max_agl.max(sim.flight_data().radar_altitude_m);
                if first_stall_agl.is_none() && sim.aircraft().is_stalled() {
                    first_stall_agl = Some(sim.flight_data().radar_altitude_m);
                }
                if sim.status() != Status::Flying {
                    break;
                }
            }
            if flap != 0 {
                // Mit Klappen ist ein Abriss mild: kein Absturz, kräftiger Steigflug
                // (bei Dauerzug pendelnd, daher die größte Höhe).
                assert_eq!(sim.status(), Status::Flying, "Absturz mit Klappen (Dauerzug {hold})");
                assert!(max_agl > 150.0, "kein Steigflug mit Klappen: höchstens {max_agl:.0} m");
            } else if hold {
                let agl = first_stall_agl.expect("Dauerzug endet nicht im Abriss");
                assert!(agl > 60.0, "Abriss schon {agl:.0} m über Grund");
            } else {
                assert_eq!(sim.status(), Status::Flying, "Absturz nach Loslassen");
                assert!(first_stall_agl.is_none(), "Abriss trotz Loslassen bei {first_stall_agl:?} m");
            }
        }
    }
}

/// Ausgefahrenes Fahrwerk rüttelt in jeder Höhe leicht und bremst.
#[test]
fn gear_shakes_at_any_altitude_and_brakes() {
    let run = |gear: u8| {
        let mut sim = Sim::new(SimConfig {
            seed: SEED,
            spawn: Spawn {
                altitude: 6_000.0,
                min_clearance: 2_500.0,
                speed: 110.0,
                throttle: 0.6,
                ..Spawn::default()
            },
        });
        let mut max_bank = 0.0_f64;
        for i in 0..ticks(20.0) {
            sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.6, gear));
            if i > ticks(6.0) {
                max_bank = max_bank.max(sim.flight_data().roll.abs());
            }
        }
        (sim.flight_data().turbulence, max_bank, sim.flight_data().tas_mps)
    };
    let (t_up, bank_up, v_up) = run(0);
    let (t_dn, bank_dn, v_dn) = run(flags::GEAR);
    assert!(t_up < 1e-9 && bank_up < 1e-6, "ohne Fahrwerk kein Rütteln in der Höhe");
    assert!(t_dn > 0.02, "kein Rütteln: {t_dn}");
    assert!(
        bank_dn.to_degrees() > 0.3 && bank_dn.to_degrees() < 4.0,
        "Querlage {:.1}°",
        bank_dn.to_degrees()
    );
    assert!(v_dn < v_up - 1.5, "Fahrwerk bremst kaum: {v_dn:.1} vs {v_up:.1}");
}

/// Schnelle, flache Landung (Spielertest): rund 190 kt, steiler als der
/// Gleitpfad (−3,8°), ohne Abfangen. Das ist hart, muss aber gelingen.
#[test]
fn fast_landing_without_flare_survives() {
    let rw = runway(SEED, AirfieldKind::Home);
    let mut sim = on_final(SEED, &rw, 2_500.0, 0.0, 0.4);
    let v = f29_core::units::kt_to_mps(190.0);
    let mut touchdown_speed = 0.0;
    for _ in 0..ticks(60.0) {
        let fd = sim.flight_data();
        if fd.on_ground || sim.status() != Status::Flying {
            break;
        }
        touchdown_speed = fd.tas_mps;
        let gamma = (fd.vertical_speed_mps / fd.tas_mps.max(1.0)).asin();
        // Gleitpfad folgen, in den letzten 30 m steiler (−3,8°) durchsinken
        let height = fd.altitude_m - 2.2 - rw.elevation_m;
        let (u, _) = rw.local(fd.position.x, fd.position.y);
        let to_aim = -rw.length_m * 0.5 + AIM_POINT_M - u;
        let target = if height > 30.0 {
            (-GLIDE_SLOPE + (to_aim.max(0.0) * GLIDE_SLOPE - height) * 0.004).clamp(-0.12, 0.03)
        } else {
            -0.066
        };
        let pitch = ((target - gamma) * 6.0).clamp(-0.5, 0.5);
        let thr = (0.4 + (v - fd.tas_mps) * 0.05).clamp(0.0, 1.0);
        let (_, vv) = rw.local(fd.position.x, fd.position.y);
        let roll = (-vv * 0.01 - fd.roll).clamp(-0.3, 0.3);
        sim.step(ControlFrame::from_analog(pitch, roll, 0.0, thr, flags::GEAR));
    }
    let p = sim.aircraft().position;
    assert_eq!(sim.status(), Status::Flying, "Absturz: {:?} bei {:?}", crash_cause(&sim), rw.local(p.x, p.y));
    let td = sim.flight_data().touchdown.expect("kein Aufsetzen");
    assert!(f29_core::units::mps_to_kt(touchdown_speed) > 175.0, "zu langsam: {touchdown_speed} m/s");
    assert!(td.sink_mps > 5.0, "Sinkrate {}", td.sink_mps);
    assert!(td.hard, "ohne Abfangen ist das eine harte Landung");
}
