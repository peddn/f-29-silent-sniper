//! Verlauf harter Manöver (Entwicklungshilfe):
//! `cargo run -p f29-core --example stall_trace -- <kt> <höhe_ft> <querlage°> <zieh-s> <schub> [abfangen 0..1]`
//!
//! Nach Ende eines Abrisses wird mit der Stärke `abfangen` gezogen.
use f29_core::sim::Spawn;
use f29_core::*;
fn main() {
    let a: Vec<f64> = std::env::args().skip(1).map(|s| s.parse().unwrap()).collect();
    let (kt, ft, bank, pull_s, thr) = (a[0], a[1], a[2], a[3], a[4]);
    let recover_pull = a.get(5).copied().unwrap_or(0.0);
    let mut was_stalled = false;
    let mut sim = Sim::new(SimConfig {
        seed: 0x0F29,
        spawn: Spawn {
            altitude: units::ft_to_m(ft),
            min_clearance: 0.0,
            speed: units::kt_to_mps(kt),
            throttle: thr,
            ..Spawn::default()
        },
    });
    let ground = sim.terrain().clone();
    let _ = ground;
    for t in 0..(25 * 120) {
        let fd = sim.flight_data();
        let roll_err = bank.to_radians() - fd.roll;
        let roll = (roll_err * 3.0).clamp(-1.0, 1.0);
        was_stalled |= sim.aircraft().stalled;
        let pull = if (t as f64) < pull_s * 120.0 && t > 60 {
            1.0
        } else if was_stalled && !sim.aircraft().stalled {
            recover_pull
        } else {
            0.0
        };
        sim.step(ControlFrame::from_analog(pull, roll, 0.0, thr, 0));
        if t % 60 == 0 {
            let f = sim.flight_data();
            println!(
                "t={:4.1} kt={:4.0} alt={:5.0} pitch={:6.1} bank={:5.0} aoa={:5.1} g={:4.1} vs={:6.0}fpm {:?} {}",
                f.time_s,
                units::mps_to_kt(f.tas_mps),
                units::m_to_ft(f.altitude_m),
                f.pitch.to_degrees(),
                f.roll.to_degrees(),
                f.alpha.to_degrees(),
                f.load_factor,
                units::mps_to_fpm(f.vertical_speed_mps),
                f.status,
                if f.warnings.stall { "STALL" } else { "" }
            );
        }
        if sim.status() != Status::Flying {
            break;
        }
    }
}
