//! Startlauf mit Tastatur-Verhalten: `takeoff_trace [klappen 0|1] [zug 0..1] [zug_ab_kt]`.
//! Der Zug wird wie bei der Tastatur mit 3/s aufgebaut, sobald die Fahrt erreicht ist.
use f29_core::controls::flags;
use f29_core::{ControlFrame, DT, Sim, SimConfig, Status, units};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let flaps = a.get(1).is_some_and(|v| v == "1");
    let pull: f64 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let from_kt: f64 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let mut sim = Sim::new(SimConfig::runway(0x0F29));
    let mut stick = 0.0_f64;
    let f = flags::GEAR | if flaps { flags::FLAPS } else { 0 };
    for i in 0..(60.0 / DT) as usize {
        let fd = sim.flight_data();
        let target = if units::mps_to_kt(fd.tas_mps) >= from_kt { pull } else { 0.0 };
        stick += (target - stick).clamp(-3.0 * DT, 3.0 * DT);
        if i % 30 == 0 {
            println!(
                "t={:4.1} {:4.0} kt vs1g {:4.0} kt  R {:5.0} ft  pitch {:5.1}  aoa {:5.1}  stall {:.2}  stick {:.2} ground {} vs {:6.0} fpm",
                fd.time_s,
                units::mps_to_kt(fd.tas_mps),
                units::mps_to_kt(fd.stall_speed_mps),
                units::m_to_ft(fd.radar_altitude_m),
                fd.pitch.to_degrees(),
                fd.alpha.to_degrees(),
                sim.aircraft().stall,
                stick,
                fd.on_ground,
                units::mps_to_fpm(fd.vertical_speed_mps)
            );
        }
        sim.step(ControlFrame::from_analog(stick, 0.0, 0.0, 1.0, f));
        if sim.status() != Status::Flying {
            println!("{:?}", sim.status());
            break;
        }
    }
}
