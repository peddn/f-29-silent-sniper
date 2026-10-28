//! Harte Kurve: `turn_trace <kt> <höhe_ft> <querlage°> <zug 0..1> <klappen 0|1> [sekunden]`.
//! Vollschub; gibt je Sekunde Fahrt, g, Abrissstärke und Energiehöhe aus.
use f29_core::controls::flags;
use f29_core::sim::Spawn;
use f29_core::{ControlFrame, DT, Sim, SimConfig, units};

fn main() {
    let a: Vec<f64> = std::env::args().skip(1).filter_map(|v| v.parse().ok()).collect();
    let (kt, ft, bank, pull, flaps) = (a[0], a[1], a[2], a[3], a[4] > 0.5);
    let secs = a.get(5).copied().unwrap_or(10.0);
    let spawn = Spawn {
        altitude: units::ft_to_m(ft),
        min_clearance: 2_500.0,
        speed: units::kt_to_mps(kt),
        throttle: 1.0,
        ..Spawn::default()
    };
    let mut sim = Sim::new(SimConfig { seed: 0x0F29, spawn });
    let f = if flaps { flags::FLAPS } else { 0 };
    let energy = |fd: &f29_core::FlightData| fd.altitude_m + fd.tas_mps * fd.tas_mps / (2.0 * 9.81);
    let e0 = energy(&sim.flight_data());
    let mut max_stall = 0.0_f64;
    for i in 0..=(secs / DT) as usize {
        let fd = sim.flight_data();
        if i % 120 == 0 {
            println!(
                "t={:4.1} {:4.0} kt  g {:4.1}  aoa {:5.1}  stall {:.2}  ΔE {:6.0} m",
                fd.time_s,
                units::mps_to_kt(fd.tas_mps),
                fd.load_factor,
                fd.alpha.to_degrees(),
                sim.aircraft().stall,
                energy(&fd) - e0
            );
        }
        max_stall = max_stall.max(sim.aircraft().stall);
        let roll = ((bank.to_radians() - fd.roll) * 3.0).clamp(-1.0, 1.0);
        sim.step(ControlFrame::from_analog(pull, roll, 0.0, 1.0, f));
    }
    println!("max Abrissstärke {max_stall:.2}");
}
