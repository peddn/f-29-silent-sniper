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

use f29_core::autopilot::Autopilot;
use f29_core::controls::flags;
use f29_core::replay::Replay;
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
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/patrol.replay");
    let fresh = scripted_patrol(0x0F29);
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

    // 1. Gespeicherte Eingaben abspielen ⇒ gespeicherter Hash (Plattform-Determinismus).
    let actual = golden.run().state_hash();
    assert_eq!(
        actual, expected,
        "Golden-Replay weicht ab: {actual:016x} statt {expected:016x}.\n\
         Wenn die Verhaltensänderung beabsichtigt ist: F29_BLESS=1 cargo test -p f29-core --test determinism"
    );
    // 2. Skript erzeugt noch dieselben Eingaben (Autopilot unverändert).
    assert_eq!(fresh.frames, golden.frames, "Autopilot/Skript erzeugt andere Eingaben");
}
