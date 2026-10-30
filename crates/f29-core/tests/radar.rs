//! Entdeckung durch feindliche Radare (M-020): Pings je Antennenumlauf,
//! Alarmstufen 1–3 mit Abklingen, Doppler gegen Querflug, Zuschläge.

use f29_core::controls::{flags, weapon_flags};
use f29_core::radar::{self, RadarEvent};
use f29_core::sim::Spawn;
use f29_core::targets::TargetClass;
use f29_core::{ControlFrame, Sim, SimConfig, TICK_HZ};

const SEED: u64 = 0x0F29;

/// Luftstart `dist` m südlich (Kurs Nord) bzw. westlich (Kurs Nord, quer)
/// der ersten Stellung der Klasse, `height` m darüber.
fn near_site(class: TargetClass, north_off: f64, east_off: f64, height: f64, heading: f64) -> (Sim, usize) {
    let probe = Sim::new(SimConfig::new(SEED));
    let (i, t) =
        probe.targets().iter().enumerate().find(|(_, t)| t.class == class).expect("Radarstellung vorhanden");
    let site = probe.radar().sites.iter().position(|s| s.target == i).expect("als Radar geführt");
    let sim = Sim::new(SimConfig {
        seed: SEED,
        spawn: Spawn {
            north: t.north + north_off,
            east: t.east + east_off,
            altitude: t.elevation + height,
            min_clearance: 300.0,
            heading,
            speed: 200.0,
            throttle: 0.7,
            ..Spawn::default()
        },
    });
    (sim, site)
}

fn fly(sim: &mut Sim, seconds: f64, w: u16, f: u8) {
    for _ in 0..(seconds * f64::from(TICK_HZ)) as usize {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.7, f).with_weapons(w));
    }
}

fn alert(sim: &Sim, site: usize) -> u8 {
    sim.radar().sites[site].alert
}

#[test]
fn every_mission_has_sam_and_early_warning_radars() {
    for seed in [SEED, 1, 77] {
        let sim = Sim::new(SimConfig::new(seed));
        let kinds: Vec<_> = sim.radar().sites.iter().map(|s| sim.targets()[s.target].class).collect();
        assert!(kinds.contains(&TargetClass::AirDefense), "Seed {seed}: keine SAM-Stellung");
        assert!(kinds.contains(&TargetClass::EarlyWarning), "Seed {seed}: kein Frühwarnradar");
    }
}

/// Auf eine SAM-Stellung zu: drei Treffer in drei Umläufen bis „RADAR ID“;
/// ab Stufe 2 ist der Ping-Ort gemerkt.
#[test]
fn three_hits_climb_to_radar_id() {
    // 6 km südlich, Kurs Nord, mit offenem Schacht
    let (mut sim, site) = near_site(TargetClass::AirDefense, -6_000.0, 0.0, 800.0, 0.0);
    fly(&mut sim, 1.0 / f64::from(TICK_HZ), weapon_flags::NEXT_BAY | weapon_flags::BAY_DOORS, 0);
    let sweep = radar::SAM.sweep_s;
    let mut ticks = 0;
    while alert(&sim, site) < 3 && ticks < (sweep * 3.0 * f64::from(TICK_HZ)) as usize + 2 {
        fly(&mut sim, 1.0 / f64::from(TICK_HZ), 0, 0);
        ticks += 1;
    }
    assert_eq!(alert(&sim, site), 3, "nach drei Umläufen nicht bei Stufe 3");
    let msgs = sim.flight_data().weapon_messages;
    assert!(msgs.iter().any(|m| m.text.starts_with("RADAR ID")), "{msgs:?}");
    assert!(sim.radar().sites[site].ping_point.is_some());
    let levels: Vec<u8> = sim
        .radar()
        .events
        .iter()
        .filter_map(|(_, e)| match e {
            RadarEvent::Alert { site: s, level } if *s == site => Some(*level),
            _ => None,
        })
        .collect();
    assert_eq!(levels, [1, 2, 3]);
}

/// Dopplerradar: quer zur Stellung bleibt unentdeckt, was auf sie zu
/// entdeckt würde.
#[test]
fn flying_across_a_doppler_radar_stays_hidden() {
    let seconds = radar::SAM.sweep_s * 3.0 + 0.5;
    // 3 km südlich, Kurs Nord (auf die Stellung zu)
    let (mut toward, site) = near_site(TargetClass::AirDefense, -3_000.0, 0.0, 800.0, 0.0);
    fly(&mut toward, seconds, 0, 0);
    // 3 km westlich, Kurs Nord (quer)
    let (mut across, _) = near_site(TargetClass::AirDefense, 0.0, -3_000.0, 800.0, 0.0);
    fly(&mut across, seconds, 0, 0);
    assert!(alert(&toward, site) >= 1, "auf die Stellung zu nicht entdeckt");
    assert_eq!(alert(&across, site), 0, "im Querflug entdeckt");
}

/// Zuschläge: dieselbe Lage ohne Zuschlag unentdeckt, mit offenem Schacht
/// bzw. eingeschaltetem Radar entdeckt.
#[test]
fn open_bay_and_own_radar_give_us_away() {
    let seconds = radar::EARLY_WARNING.sweep_s * 2.0 + 0.5;
    let run = |w: u16, f: u8| {
        // 20 km südlich des Frühwarnradars, Kurs Ost (Flanke zum Radar)
        let (mut sim, site) =
            near_site(TargetClass::EarlyWarning, -20_000.0, 0.0, 1_500.0, std::f64::consts::FRAC_PI_2);
        fly(&mut sim, 1.0 / f64::from(TICK_HZ), w, f);
        fly(&mut sim, seconds, 0, 0);
        alert(&sim, site)
    };
    let clean = run(0, 0);
    let bay = run(weapon_flags::NEXT_BAY | weapon_flags::BAY_DOORS, 0);
    let radar = run(0, flags::RADAR_TOGGLE);
    assert!(bay > clean, "Schacht: {bay} vs {clean}");
    assert!(radar > clean, "Radar: {radar} vs {clean}");
}

/// Ohne Treffer sinkt die Stufe alle 30 s um eins bis „LOST CONTACT“.
#[test]
fn alert_decays_without_hits() {
    let (mut sim, site) = near_site(TargetClass::AirDefense, -3_000.0, 0.0, 800.0, 0.0);
    fly(&mut sim, radar::SAM.sweep_s + 0.5, 0, 0);
    let reached = alert(&sim, site);
    assert!(reached >= 1);
    // Zeit ohne Treffer: Das Flugzeug stürzt ab (keine Pings mehr), die
    // Stufen klingen trotzdem ab.
    let mut crashed = sim.clone();
    for _ in 0..(120.0 * f64::from(TICK_HZ)) as usize {
        crashed.step(ControlFrame::from_analog(-1.0, 0.0, 0.0, 0.0, 0));
        if crashed.status() != f29_core::Status::Flying && alert(&crashed, site) == 0 {
            break;
        }
    }
    assert_eq!(alert(&crashed, site), 0);
    assert!(crashed.radar().events.iter().any(|(_, e)| *e == RadarEvent::Lost { site }));
}

/// Eine zerstörte Stellung pingt nicht mehr und vergisst.
#[test]
fn destroyed_radar_goes_quiet() {
    let (mut sim, site) = near_site(TargetClass::AirDefense, -3_000.0, 0.0, 800.0, 0.0);
    fly(&mut sim, radar::SAM.sweep_s + 0.5, 0, 0);
    assert!(alert(&sim, site) >= 1);
    let target = sim.radar().sites[site].target;
    sim.destroy_target(target);
    fly(&mut sim, radar::SAM.sweep_s * 2.0, 0, 0);
    assert_eq!(alert(&sim, site), 0);
    assert!(sim.flight_data().threat.sites.iter().all(|s| s.target != target));
}

#[test]
fn radar_state_is_deterministic() {
    let run = || {
        let (mut sim, _) = near_site(TargetClass::AirDefense, -6_000.0, 0.0, 800.0, 0.0);
        fly(&mut sim, 20.0, 0, 0);
        sim.state_hash()
    };
    assert_eq!(run(), run());
}

/// Start auf der Heimatbasis: Der Boden ist kein Sonderfall, aber keine
/// feindliche Radarstellung steht näher als 35 km an der Basis – beim Anrollen
/// reicht kein Impuls über das Stealth-Level.
#[test]
fn takeoff_roll_stays_undetected() {
    for seed in [SEED, 1, 77, 1234] {
        let probe = Sim::new(SimConfig::runway(seed));
        let home = probe.terrain().runways()[0].center;
        for s in &probe.radar().sites {
            let t = &probe.targets()[s.target];
            let d = (t.north - home[0]).hypot(t.east - home[1]);
            assert!(
                d > f29_core::targets::HOME_RADAR_CLEARANCE_M,
                "Seed {seed}: Radar {d:.0} m von der Basis"
            );
        }
    }
    let mut sim = Sim::new(SimConfig::runway(SEED));
    // Vollgas mit Fahrwerk und Klappen, Rollen ohne abzuheben
    for _ in 0..(20 * TICK_HZ) {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 1.0, flags::GEAR | flags::FLAPS));
        if !sim.aircraft().on_ground {
            break;
        }
    }
    assert_eq!(sim.radar().max_alert(), 0);
}
