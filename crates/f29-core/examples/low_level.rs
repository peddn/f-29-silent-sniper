//! Tiefflug mit losgelassenem Knüppel: wie stark rüttelt die Turbulenz?
//! `cargo run -p f29-core --example low_level -- <höhe_über_grund_ft> <kt> [klappen 0/1]`
use f29_core::controls::flags;
use f29_core::sim::Spawn;
use f29_core::*;
fn main() {
    let a: Vec<f64> = std::env::args().skip(1).map(|s| s.parse().unwrap()).collect();
    let (agl_ft, kt) = (a[0], a[1]);
    let flap = if a.get(2).copied().unwrap_or(0.0) > 0.5 { flags::FLAPS } else { 0 };
    let mut sim = Sim::new(SimConfig {
        seed: 0x0F29,
        spawn: Spawn {
            altitude: 0.0,
            min_clearance: units::ft_to_m(agl_ft),
            speed: units::kt_to_mps(kt),
            throttle: 0.6,
            ..Spawn::default()
        },
    });
    let (mut max_roll, mut max_pitch_dev) = (0.0_f64, 0.0_f64);
    let p0 = sim.flight_data().pitch;
    for _ in 0..(10 * 120) {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.6, flap));
        let fd = sim.flight_data();
        max_roll = max_roll.max(fd.roll.abs());
        max_pitch_dev = max_pitch_dev.max((fd.pitch - p0).abs());
        if sim.status() != Status::Flying {
            println!("Absturz nach {:.1} s", fd.time_s);
            break;
        }
    }
    let fd = sim.flight_data();
    println!(
        "{agl_ft} ft / {kt} kt: max. Querlage {:.1}°, max. Nickabweichung {:.1}°, Radarhöhe jetzt {:.0} ft",
        max_roll.to_degrees(),
        max_pitch_dev.to_degrees(),
        units::m_to_ft(fd.radar_altitude_m)
    );
}
