//! Determinismus- und Golden-Replay-Tests.
//!
//! Der Golden-Hash in `tests/golden/patrol.replay` ist absichtlich empfindlich:
//! Jede Änderung am Flugmodell, an Daten oder am Terrain ändert ihn. Das ist
//! gewollt – so fällt jede unbeabsichtigte Verhaltensänderung sofort auf.
//!
//! Nach einer *beabsichtigten* Änderung den Hash neu schreiben:
//!
//! ```sh
//! F29_BLESS=1 cargo test -p f29-core --test determinism
//! ```

use f29_core::autopilot::{AIM_POINT_M, Approach, Autopilot, GLIDE_SLOPE};
use f29_core::controls::flags;
use f29_core::replay::Replay;
use f29_core::sim::{Spawn, SpawnKind};
use f29_core::{ControlFrame, DT, Sim, SimConfig};
use std::path::PathBuf;

/// Erzeugt einen abwechslungsreichen, aber reproduzierbaren Flug.
fn scripted_patrol(seed: u64) -> Replay {
    let mut sim = Sim::new(SimConfig::new(seed));
    let mut replay = Replay::new(sim.config().clone());
    let start = sim.flight_data();
    let mut ap = Autopilot::new(start.altitude_m + 500.0, start.heading, 230.0);
    for t in 0..(90 * f29_core::TICK_HZ) {
        // Phasen: Steigflug, Kurve, Rollen von Hand, Luftbremse, Steerpoint-Wechsel
        match t {
            1_800 => ap.target_heading = f29_core::detmath::wrap_two_pi(start.heading + 2.0),
            6_000 => ap.target_altitude_m -= 900.0,
            _ => {}
        }
        let mut c = if (4_200..4_500).contains(&t) {
            ControlFrame::from_analog(0.1, 1.0, 0.2, 0.9, 0)
        } else {
            ap.control(sim.spec(), &sim.flight_data(), DT)
        };
        if (7_000..8_000).contains(&t) {
            c.flags |= flags::AIRBRAKE;
        }
        if t == 5_000 || t == 9_000 {
            c.flags |= flags::NEXT_STEERPOINT;
        }
        sim.step(c);
        replay.push(c);
    }
    replay.final_hash = Some(sim.state_hash());
    replay
}

/// Endanflug, Landung, Ausrollen, Lenken im Stand und erneuter Start: deckt Fahrwerk,
/// Klappen, Bodenmodus, Aufsetzbewertung und Treibstoff ab.
fn scripted_landing(seed: u64) -> Replay {
    let rw = Sim::new(SimConfig::new(seed)).terrain().runways()[0].clone();
    let dist = 6_000.0;
    let p = rw.world(-rw.length_m * 0.5 + AIM_POINT_M - dist, 0.0);
    let spawn = Spawn {
        kind: SpawnKind::Air,
        fuel: 0.4,
        north: p[0],
        east: p[1],
        altitude: rw.elevation_m + 2.2 + dist * GLIDE_SLOPE,
        min_clearance: 0.0,
        heading: rw.heading_deg * (std::f64::consts::PI / 180.0),
        speed: 95.0,
        throttle: 0.3,
        // Nach der Mission: Schächte leer.
        loadout: f29_core::weapons::Loadout::EMPTY,
        night: false,
        awacs: true,
        visible: false,
    };
    let mut sim = Sim::new(SimConfig { seed, spawn });
    let mut replay = Replay::new(sim.config().clone());
    let mut ap = Approach::new(rw);
    let mut phase_tick = None;
    for t in 0..(150 * f29_core::TICK_HZ) {
        let fd = sim.flight_data();
        if phase_tick.is_none() && fd.stopped_on_runway {
            phase_tick = Some(t);
        }
        let c = match phase_tick {
            None => ap.control(sim.spec(), &fd, DT),
            // Auf der Stelle 3 s nach links und 3 s zurück lenken, dann
            // geradeaus Vollgas und abheben.
            Some(t0) if t - t0 < 6 * f29_core::TICK_HZ => {
                let steer = if t - t0 < 3 * f29_core::TICK_HZ { -1.0 } else { 1.0 };
                ControlFrame::from_analog(0.0, steer, 0.0, 0.0, flags::GEAR | flags::AIRBRAKE)
            }
            Some(_) => {
                // rotieren und etwa 12° Nicklage halten
                let pull = if fd.tas_mps > 1.1 * fd.stall_speed_mps {
                    ((0.21 - fd.pitch) * 4.0).clamp(-0.5, 0.6)
                } else {
                    0.0
                };
                ControlFrame::from_analog(pull, 0.0, 0.0, 1.0, flags::GEAR | flags::FLAPS)
            }
        };
        sim.step(c);
        replay.push(c);
    }
    assert!(phase_tick.is_some(), "Golden-Landung: nicht zum Stehen gekommen");
    assert_eq!(sim.status(), f29_core::Status::Flying);
    replay.final_hash = Some(sim.state_hash());
    replay
}

fn check_golden(name: &str, fresh: Replay) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(name);
    if std::env::var_os("F29_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, fresh.to_text()).unwrap();
        eprintln!("Golden-Replay neu geschrieben: {}", path.display());
        return;
    }
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} fehlt ({e}). Mit F29_BLESS=1 erzeugen.", path.display()));
    let golden = Replay::from_text(&text).expect("Golden-Replay lesbar");
    let expected = golden.final_hash.expect("Golden-Replay ohne Hash");
    let actual = golden.run().state_hash();
    assert_eq!(
        actual, expected,
        "{name} weicht ab: {actual:016x} statt {expected:016x}.\n\
         Wenn die Verhaltensänderung beabsichtigt ist: F29_BLESS=1 cargo test -p f29-core --test determinism"
    );
    assert_eq!(fresh, golden, "{name}: Szenario erzeugt andere Eingaben als gespeichert");
}

#[test]
fn golden_landing_replay() {
    check_golden("landing.replay", scripted_landing(0x0F29));
}

#[test]
fn identical_inputs_give_identical_state() {
    let a = scripted_patrol(1);
    let b = scripted_patrol(1);
    assert_eq!(a, b);
    assert_eq!(a.run().state_hash(), a.final_hash.unwrap());
}

#[test]
fn different_seed_gives_different_state() {
    assert_ne!(scripted_patrol(1).final_hash, scripted_patrol(2).final_hash);
}

#[test]
fn tiny_input_change_changes_state() {
    let base = scripted_patrol(1);
    let mut changed = base.clone();
    // Eine einzige Eingabe um einen Quantisierungsschritt ändern
    let (_, f) = &mut changed.frames[10];
    f.roll = f.roll.saturating_add(1);
    assert_ne!(base.run().state_hash(), changed.run().state_hash());
}

#[test]
fn replay_survives_text_roundtrip() {
    let r = scripted_patrol(3);
    let parsed = Replay::from_text(&r.to_text()).unwrap();
    assert_eq!(parsed.run().state_hash(), r.final_hash.unwrap());
}

#[test]
fn golden_patrol_replay() {
    check_golden("patrol.replay", scripted_patrol(0x0F29));
}
