//! Abriss in Querlage: `bank_stall <kt> <querlage°>` – Querlage halten und voll ziehen bis zum Abriss, dann 4 s weiter ziehen.
use f29_core::sim::Spawn;
use f29_core::{ControlFrame, DT, Sim, SimConfig, units};
fn main() {
    let a: Vec<f64> = std::env::args().skip(1).filter_map(|v| v.parse().ok()).collect();
    let spawn = Spawn {
        altitude: 2_500.0,
        min_clearance: 2_500.0,
        speed: units::kt_to_mps(a[0]),
        throttle: 0.7,
        ..Spawn::default()
    };
    let mut sim = Sim::new(SimConfig { seed: 0x0F29, spawn });
    let mut stalled_at = None;
    for i in 0..(10.0 / DT) as usize {
        let fd = sim.flight_data();
        let c = if stalled_at.is_none() {
            let roll = ((a[1].to_radians() - fd.roll) * 3.0).clamp(-1.0, 1.0);
            ControlFrame::from_analog(1.0, roll, 0.0, 0.7, 0)
        } else {
            ControlFrame::from_analog(1.0, 0.0, 0.0, 0.7, 0)
        };
        if stalled_at.is_none() && sim.aircraft().is_stalled() {
            stalled_at = Some(i);
        }
        if i % 30 == 0 {
            println!(
                "t={:4.1} {:4.0} kt pitch {:6.1} roll {:6.1} stall {:.2} g {:4.1}",
                fd.time_s,
                units::mps_to_kt(fd.tas_mps),
                fd.pitch.to_degrees(),
                fd.roll.to_degrees(),
                sim.aircraft().stall,
                fd.load_factor
            );
        }
        sim.step(c);
    }
}
