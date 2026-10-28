//! Langsamflug: `slow_trace <kt> <klappen 0|1> <zug> [roll]` – 4 s Zug/Rollen auf 3000 ft,
//! Schub 80 %. Gibt größte Nick- und Rollrate, größte Abrissstärke und
//! tiefste Nicklage der folgenden 6 s aus.
use f29_core::controls::flags;
use f29_core::sim::Spawn;
use f29_core::{ControlFrame, DT, Sim, SimConfig, units};

fn main() {
    let a: Vec<f64> = std::env::args().skip(1).filter_map(|v| v.parse().ok()).collect();
    let (kt, flaps, pull) = (a[0], a[1] > 0.5, a[2]);
    let roll = a.get(3).copied().unwrap_or(0.0);
    let spawn = Spawn {
        altitude: 900.0,
        min_clearance: 600.0,
        speed: units::kt_to_mps(kt),
        throttle: 0.8,
        ..Spawn::default()
    };
    let mut sim = Sim::new(SimConfig { seed: 0x0F29, spawn });
    let f = if flaps { flags::FLAPS } else { 0 };
    // Klappen vorab ausfahren, Fahrt halten
    for _ in 0..(3.0 / DT) as usize {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.8, f));
    }
    let h0 = sim.flight_data().altitude_m;
    let (mut q, mut p, mut st, mut minp) = (0.0_f64, 0.0_f64, 0.0_f64, 90.0_f64);
    for i in 0..(10.0 / DT) as usize {
        let c = if i < (4.0 / DT) as usize {
            ControlFrame::from_analog(pull, roll, 0.0, 0.8, f)
        } else {
            ControlFrame::from_analog(0.0, 0.0, 0.0, 0.8, f)
        };
        sim.step(c);
        let r = sim.aircraft().body_rates;
        q = q.max(r.y);
        p = p.max(r.x.abs());
        st = st.max(sim.aircraft().stall);
        minp = minp.min(sim.flight_data().pitch.to_degrees());
    }
    let fd = sim.flight_data();
    println!(
        "{kt:.0} kt klappen {flaps} zug {pull}: Nickrate max {:.1}°/s  Rollrate {:.0}°/s  Abriss max {st:.2}  tiefste Nase {minp:.0}°  Höhe {:+.0} ft  Fahrt jetzt {:.0} kt",
        q.to_degrees(),
        p.to_degrees(),
        units::m_to_ft(fd.altitude_m - h0),
        units::mps_to_kt(fd.tas_mps)
    );
}
