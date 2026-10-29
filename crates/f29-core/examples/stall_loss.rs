//! Höhenverlust durch Abrisse in drei Szenarien, je ohne/mit Klappen.
use f29_core::controls::flags;
use f29_core::sim::Spawn;
use f29_core::{ControlFrame, DT, Sim, SimConfig, units};
fn scenario(kind: u8, flaps: bool) -> String {
    let f = if flaps { flags::FLAPS } else { 0 };
    let (kt, thr) = match kind {
        0 => (180.0, 0.2),
        1 => (250.0, 0.7),
        _ => (300.0, 0.7),
    };
    let mut sim = Sim::new(SimConfig {
        seed: 0x0F29,
        spawn: Spawn {
            altitude: 3000.0,
            min_clearance: 2500.0,
            speed: units::kt_to_mps(kt),
            throttle: thr,
            fuel: 0.6,
            ..Spawn::default()
        },
    });
    let (mut stalled_at, mut phase, mut min_alt, mut max_stall, mut t_stall) =
        (None::<f64>, 0, f64::MAX, 0.0_f64, 0.0);
    let mut out_t = 0.0;
    for i in 0..(120.0 / DT) as usize {
        let fd = sim.flight_data();
        let st = sim.aircraft().stall;
        if phase == 0 && st > 0.1 {
            phase = 1;
            stalled_at = Some(fd.altitude_m);
        }
        let (pitch, roll) = match phase {
            0 => match kind {
                0 => (
                    (-fd.vertical_speed_mps * 0.15 + 0.1).clamp(-0.3, 1.0),
                    (-fd.roll * 2.0).clamp(-1.0, 1.0),
                ),
                1 => {
                    let r = ((1.05 - fd.roll) * 3.0).clamp(-1.0, 1.0);
                    (if fd.roll > 0.9 { 1.0 } else { 0.0 }, r)
                }
                _ => (
                    if fd.pitch < 0.7 { 0.7 } else { ((0.7 - fd.pitch) * 3.0).clamp(-0.3, 0.3) },
                    (-fd.roll * 2.0).clamp(-1.0, 1.0),
                ),
            },
            1 => {
                if st < 0.05 {
                    phase = 2;
                }
                (0.0, (-fd.roll * 2.0).clamp(-1.0, 1.0))
            }
            _ => {
                if fd.vertical_speed_mps > 0.0 {
                    phase = 3;
                    out_t = fd.time_s;
                }
                (0.5, (-fd.roll * 2.0).clamp(-1.0, 1.0))
            }
        };
        if phase >= 1 {
            min_alt = min_alt.min(fd.altitude_m);
            max_stall = max_stall.max(st);
            if st > 0.1 {
                t_stall += DT;
            }
        }
        if phase == 3 {
            break;
        }
        sim.step(ControlFrame::from_analog(pitch, roll, 0.0, if phase >= 1 { 1.0 } else { thr }, f));
        let _ = i;
    }
    match stalled_at {
        Some(h) => format!(
            "Verlust {:5.0} ft  max Abriss {:.2}  Abrisszeit {:4.1} s  Erholt t={:4.1}",
            units::m_to_ft(h - min_alt),
            max_stall,
            t_stall,
            out_t
        ),
        None => "kein Abriss".into(),
    }
}
fn main() {
    for (k, name) in [(0u8, "Horizontal verzögern"), (1, "Kurve 60° voll"), (2, "Steigflug 40°")] {
        for fl in [false, true] {
            println!("{name:22} Klappen {}: {}", u8::from(fl), scenario(k, fl));
        }
    }
}
