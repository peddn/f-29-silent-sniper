//! Hörproben: rendert die Klänge als WAV-Dateien, ganz ohne Soundkarte.

use f29_audio::{AudioInput, Mix, SAMPLE_RATE, Synth, TouchdownCue, voice, wav};
use f29_core::autopilot::{AIM_POINT_M, Approach, GLIDE_SLOPE};
use f29_core::controls::flags;
use f29_core::sim::{Spawn, SpawnKind};
use f29_core::{ControlFrame, DT, Sim, SimConfig, Status, TICK_HZ};
use std::path::Path;

/// Bildrate, mit der die Hörproben den Synthesizer füttern (wie das Spiel).
const FPS: f32 = 60.0;

/// Nur das Triebwerk.
const ENGINE_ONLY: Mix = Mix { engine: 1.0, systems: 0.0, events: 0.0, voice: 0.0 };

/// Rendert eine Zeitleiste: `input(t)` liefert die Momentaufnahme zur Zeit t.
fn render(seconds: f32, mix: Mix, mut input: impl FnMut(f32) -> AudioInput) -> Vec<f32> {
    let mut synth = Synth::with_default_voice();
    synth.set_mix(mix);
    let per_frame = (SAMPLE_RATE as f32 / FPS) as usize;
    let frames = (seconds * FPS) as usize;
    let mut out = vec![0.0; frames * per_frame];
    for (k, chunk) in out.chunks_mut(per_frame).enumerate() {
        synth.set_input(input(k as f32 / FPS));
        synth.render(chunk);
    }
    out
}

fn ramp(t: f32, t0: f32, t1: f32) -> f32 {
    ((t - t0) / (t1 - t0)).clamp(0.0, 1.0)
}

fn cruise(engine: f32, tas: f32) -> AudioInput {
    AudioInput { engine, running: true, tas, ..AudioInput::default() }
}

/// Ein Flug aus der echten Simulation: Start mit Klappen, Fahrwerk ein,
/// Luftbremse, Steigflug, Abriss durch hartes Ziehen in der Kurve, Erholung.
fn flight_demo() -> Vec<f32> {
    let mut sim = Sim::new(SimConfig::runway(0x0F29));
    let ticks_per_frame = (TICK_HZ as f32 / FPS) as usize;
    render(85.0, Mix::default(), |t| {
        let fd = sim.flight_data();
        let mut f = flags::FLAPS;
        if fd.radar_altitude_m < 60.0 || t < 25.0 {
            f |= flags::GEAR;
        }
        if t > 30.0 {
            f &= !flags::FLAPS;
        }
        let pitch = if fd.on_ground {
            if fd.tas_mps > 1.15 * fd.stall_speed_mps { 0.7 } else { 0.0 }
        } else if (68.0..71.0).contains(&t) {
            1.0
        } else {
            ((0.22 - fd.pitch) * 3.0).clamp(-0.5, 0.5)
        };
        let roll = if (68.0..71.0).contains(&t) {
            ((1.2 - fd.roll) * 3.0).clamp(-1.0, 1.0)
        } else {
            (-fd.roll * 2.0).clamp(-1.0, 1.0)
        };
        let throttle = if (33.0..37.0).contains(&t) { 0.2 } else { 1.0 };
        if (33.0..37.0).contains(&t) {
            f |= flags::AIRBRAKE;
        }
        for _ in 0..ticks_per_frame {
            sim.step(ControlFrame::from_analog(pitch, roll, 0.0, throttle, f));
        }
        AudioInput::from_flight(&sim.flight_data(), false)
    })
}

/// Endanflug und Landung aus der Simulation (Autopilot), mit Ausrollen.
fn landing_demo() -> Vec<f32> {
    let rw = Sim::new(SimConfig::new(0x0F29)).terrain().runways()[0].clone();
    let dist = 3_000.0;
    let p = rw.world(-rw.length_m * 0.5 + AIM_POINT_M - dist, 0.0);
    let spawn = Spawn {
        kind: SpawnKind::Air,
        fuel: 0.4,
        north: p[0],
        east: p[1],
        altitude: rw.elevation_m + 2.2 + dist * GLIDE_SLOPE,
        min_clearance: 0.0,
        heading: rw.heading_deg.to_radians(),
        speed: 85.0,
        throttle: 0.4,
    };
    let mut sim = Sim::new(SimConfig { seed: 0x0F29, spawn });
    let mut ap = Approach::new(rw);
    let ticks_per_frame = (TICK_HZ as f32 / FPS) as usize;
    render(60.0, Mix::default(), |_| {
        for _ in 0..ticks_per_frame {
            if sim.status() == Status::Flying {
                let c = ap.control(sim.spec(), &sim.flight_data(), DT);
                sim.step(c);
            }
        }
        AudioInput::from_flight(&sim.flight_data(), false)
    })
}

pub fn render_all(dir: &Path) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let td = |id: u64, hard: bool| Some(TouchdownCue { id, hard });
    let clips: Vec<(&str, Vec<f32>)> = vec![
        (
            "01_triebwerk",
            render(12.0, ENGINE_ONLY, |t| {
                let up = ramp(t, 2.0, 5.0) - ramp(t, 8.0, 11.0);
                cruise(up, 40.0 + 180.0 * ramp(t, 2.0, 9.0))
            }),
        ),
        (
            "02_fahrwerk",
            render(13.0, Mix::WITHOUT_ENGINE, |t| AudioInput {
                gear: ramp(t, 1.0, 6.0) - ramp(t, 7.5, 12.5),
                ..cruise(0.55, 120.0)
            }),
        ),
        (
            "03_klappen",
            render(6.0, Mix::WITHOUT_ENGINE, |t| AudioInput {
                flaps: ramp(t, 0.5, 2.5) - ramp(t, 3.5, 5.5),
                ..cruise(0.55, 120.0)
            }),
        ),
        (
            "04_luftbremse",
            render(6.0, Mix::WITHOUT_ENGINE, |t| AudioInput {
                airbrake: ramp(t, 1.0, 1.8) - ramp(t, 4.0, 4.8),
                ..cruise(0.6, 200.0)
            }),
        ),
        (
            "05_aufsetzen",
            render(8.0, Mix::WITHOUT_ENGINE, |t| {
                let (on_ground, touchdown) = if t >= 4.5 {
                    (true, td(2, true))
                } else if t >= 1.0 {
                    (t < 3.5, td(1, false))
                } else {
                    (false, None)
                };
                AudioInput { gear: 1.0, flaps: 1.0, on_ground, touchdown, ..cruise(0.05, 70.0 - 6.0 * t) }
            }),
        ),
        (
            "06b_bodenwarnung_pull_up",
            render(6.0, Mix::WITHOUT_ENGINE, |t| AudioInput {
                pull_up_warning: (1.0..4.5).contains(&t),
                ..cruise(0.8, 200.0)
            }),
        ),
        (
            "06_abrisswarnung",
            render(7.0, Mix::WITHOUT_ENGINE, |t| AudioInput {
                stall_warning: (1.0..5.5).contains(&t),
                ..cruise(0.8, 75.0)
            }),
        ),
        (
            "07_absturz",
            render(4.0, Mix::WITHOUT_ENGINE, |t| AudioInput { crashed: t > 1.0, ..cruise(0.9, 200.0) }),
        ),
        ("08_flug_start_abriss", flight_demo()),
        ("09_flug_landung", landing_demo()),
    ];
    let mut written = Vec::new();
    for (name, samples) in clips {
        let path = dir.join(format!("{name}.wav"));
        std::fs::write(&path, wav::write_mono16(&samples, SAMPLE_RATE))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        written.push(path.display().to_string());
    }
    for (name, word) in [("10_ansage_pull_up", &voice::PULL_UP[..]), ("11_ansage_stall", &voice::STALL[..])] {
        let path = dir.join(format!("{name}.wav"));
        std::fs::write(&path, wav::write_mono16(&voice::speak(word, SAMPLE_RATE), SAMPLE_RATE))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        written.push(path.display().to_string());
    }
    Ok(written)
}
