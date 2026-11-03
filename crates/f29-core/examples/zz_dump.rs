use f29_core::autopilot::Autopilot;
use f29_core::controls::weapon_flags as w;
use f29_core::replay::Replay;
use f29_core::rules::Conflict;
use f29_core::{DT, Sim, SimConfig, TICK_HZ};

fn main() {
    for f in ["landing", "patrol"] {
        let text = std::fs::read_to_string(format!("crates/f29-core/tests/golden/{f}.replay")).unwrap();
        let sim = Replay::from_text(&text).unwrap().run();
        println!("{f}: {:?}", sim.flight_data());
    }
    for seed in 1..=4 {
        let mut c = SimConfig::new(seed);
        c.spawn.conflict = Conflict::War;
        c.spawn.visible = true;
        let mut sim = Sim::new(c);
        let fd = sim.flight_data();
        let mut ap = Autopilot::new(fd.altitude_m.max(300.0), fd.heading, 250.0);
        for t in 0..(240 * TICK_HZ) {
            let fd = sim.flight_data();
            if let Some(sp) = &fd.steerpoint {
                ap.target_heading = sp.bearing;
            }
            let c = ap.control(sim.spec(), &fd, DT);
            let flags = match t % 600 {
                0 => w::NEXT_BAY,
                10 => w::BAY_DOORS,
                100 | 110 | 120 => w::RELEASE,
                200 => w::FLARE,
                300 => w::CHAFF,
                400 => w::ECM,
                500 => w::IR_JAMMER,
                590 => w::BAY_DOORS,
                _ => 0,
            };
            sim.step(c.with_weapons(flags));
            if t % (30 * TICK_HZ) == 0 {
                println!("{seed}/{t}: {:?}", sim.flight_data());
            }
        }
        println!("{seed}/end: {:?}", sim.flight_data());
    }
}
