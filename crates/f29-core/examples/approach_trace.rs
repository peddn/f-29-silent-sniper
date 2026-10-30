//! Verfolgt einen Autopilot-Anflug: `approach_trace [seed] [bahn-index] [entfernung_m]`.
use f29_core::autopilot::{AIM_POINT_M, Approach, GLIDE_SLOPE};
use f29_core::sim::{Spawn, SpawnKind};
use f29_core::{DT, Sim, SimConfig, Status};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(0x0F29);
    let idx: usize = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(0);
    let dist: f64 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(8_000.0);
    let rw = Sim::new(SimConfig::new(seed)).terrain().runways()[idx].clone();
    println!("{} Richtung {}° Länge {} m Höhe {:.1} m", rw.name, rw.heading_deg, rw.length_m, rw.elevation_m);
    let u = -rw.length_m * 0.5 + AIM_POINT_M - dist;
    let p = rw.world(u, 0.0);
    let spawn = Spawn {
        kind: SpawnKind::Air,
        fuel: 0.5,
        north: p[0],
        east: p[1],
        altitude: rw.elevation_m + 2.2 + dist * GLIDE_SLOPE,
        min_clearance: 0.0,
        heading: rw.heading_deg.to_radians(),
        speed: 95.0,
        throttle: 0.3,
        // Nach der Mission: Schächte leer.
        loadout: f29_core::weapons::Loadout::EMPTY,
        night: false,
        awacs: true,
        visible: false,
    };
    let mut sim = Sim::new(SimConfig { seed, spawn });
    let mut ap = Approach::new(rw.clone());
    for i in 0..(300.0 / DT) as usize {
        let fd = sim.flight_data();
        if i % 60 == 0 || fd.radar_altitude_m < 15.0 && i % 12 == 0 {
            let (u, v) = rw.local(fd.position.x, fd.position.y);
            println!(
                "t={:5.1} u={:7.0} v={:5.1} h={:6.1} R={:6.1} gnd={:6.1} vs={:5.1} v={:5.1} vs1g={:5.1} pitch={:5.1} roll={:5.1} gear={:.2} flaps={:.2} thr={:.2} ground={}",
                fd.time_s,
                u,
                v,
                fd.altitude_m - rw.elevation_m - 2.2,
                fd.radar_altitude_m,
                fd.ground_elevation_m - rw.elevation_m,
                fd.vertical_speed_mps,
                fd.tas_mps,
                fd.stall_speed_mps,
                fd.pitch.to_degrees(),
                fd.roll.to_degrees(),
                fd.gear,
                fd.flaps,
                fd.throttle,
                fd.on_ground
            );
        }
        if fd.stopped_on_runway || sim.status() != Status::Flying {
            println!("Ende: {:?} touchdown {:?}", sim.status(), fd.touchdown);
            break;
        }
        let c = ap.control(sim.spec(), &fd, DT);
        sim.step(c);
    }
}
