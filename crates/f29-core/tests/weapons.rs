//! Waffen (M-014): Schacht, Auslösen, Ballistik, Wirkung beim Einschlag.

use f29_core::controls::weapon_flags;
use f29_core::sim::Spawn;
use f29_core::targets::{TargetClass, TargetState};
use f29_core::weapons::{self, ImpactResult, Loadout, WeaponEvent, WeaponKind};
use f29_core::{ControlFrame, DT, Sim, SimConfig, TICK_HZ};

const SEED: u64 = 0x0F29;

fn ticks(s: f64) -> usize {
    (s * f64::from(TICK_HZ)) as usize
}

/// Geradeausflug mit Schub 0,7; `w` sind die Waffenbits.
fn step(sim: &mut Sim, w: u8) {
    sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.7, 0).with_weapons(w));
}

fn events_since(sim: &Sim, tick: u64) -> Vec<WeaponEvent> {
    sim.weapon_events().iter().filter(|(t, _)| *t > tick).map(|(_, e)| *e).collect()
}

/// Luftstart `height` m über einem Ziel der Klasse `class`, `back` m davor:
/// positiv = südlich mit Kurs Nord, negativ = nördlich mit Kurs Süd.
/// `east_shift` versetzt den Startpunkt quer.
fn approach_at(
    class: TargetClass,
    height: f64,
    back: f64,
    east_shift: f64,
    loadout: Loadout,
) -> (Sim, usize) {
    let probe = Sim::new(SimConfig::new(SEED));
    let (i, t) = probe
        .targets()
        .iter()
        .enumerate()
        .find(|(_, t)| t.class == class)
        .map(|(i, t)| (i, t.clone()))
        .expect("Zielklasse vorhanden");
    let sim = Sim::new(SimConfig {
        seed: SEED,
        spawn: Spawn {
            north: t.north - back,
            east: t.east + east_shift,
            altitude: t.elevation + height,
            min_clearance: 0.0,
            heading: if back >= 0.0 { 0.0 } else { std::f64::consts::PI },
            speed: 200.0,
            throttle: 0.7,
            loadout,
            ..Spawn::default()
        },
    });
    (sim, i)
}

#[test]
fn release_needs_open_bay_and_drops_mass() {
    let mut sim = Sim::new(SimConfig::new(SEED));
    let bay0 = sim.stores().bays[0];
    assert_eq!(bay0.kind, Some(WeaponKind::FreeFall));
    let mass0 = sim.flight_data().mass_kg;

    step(&mut sim, weapon_flags::RELEASE);
    assert!(events_since(&sim, 0).contains(&WeaponEvent::BayClosed));
    assert_eq!(sim.stores().bays[0].count, bay0.count);

    step(&mut sim, weapon_flags::BAY_DOORS);
    for _ in 0..ticks(weapons::DOOR_TIME_S + 0.1) {
        step(&mut sim, 0);
    }
    assert!(sim.stores().door_open());
    let t0 = sim.flight_data().tick;
    step(&mut sim, weapon_flags::RELEASE);
    assert!(matches!(
        events_since(&sim, t0)[..],
        [WeaponEvent::Released { kind: WeaponKind::FreeFall, bay: 0, .. }]
    ));
    assert_eq!(sim.stores().bays[0].count, bay0.count - 1);
    assert_eq!(sim.weapons_in_flight().len(), 1);
    let dropped = mass0 - sim.flight_data().mass_kg;
    assert!(dropped > WeaponKind::FreeFall.spec().mass_kg - 5.0, "Masse {dropped:.0} kg");

    // Schacht wechseln und schließen.
    step(&mut sim, weapon_flags::NEXT_BAY | weapon_flags::BAY_DOORS);
    assert_eq!(sim.stores().selected, 1);
    for _ in 0..ticks(weapons::DOOR_TIME_S + 0.1) {
        step(&mut sim, 0);
    }
    assert_eq!(sim.stores().door, 0.0);
}

/// Ergebnis eines Bombenanflugs.
struct Run {
    sim: Sim,
    target: usize,
    predicted: f29_core::DVec3,
    impact: f29_core::DVec3,
    result: ImpactResult,
}

/// Waagerechter Anflug in 900 m mit der Freifallbombe; ausgelöst, sobald der
/// vorhergesagte Aufschlagpunkt das Ziel erreicht.
fn bomb_run(class: TargetClass, east_shift: f64) -> Run {
    let (mut sim, target) = approach_at(class, 900.0, 5_000.0, east_shift, Loadout::default());
    step(&mut sim, weapon_flags::BAY_DOORS);
    let goal_north = sim.targets()[target].north;
    let mut released: Option<(u64, f29_core::DVec3)> = None;
    for _ in 0..ticks(60.0) {
        let a = sim.aircraft();
        let mut w = 0;
        if released.is_none() && sim.stores().door_open() {
            let (p, _) = weapons::predict_release(
                WeaponKind::FreeFall,
                a.position,
                a.velocity,
                a.attitude,
                sim.terrain(),
                DT,
            )
            .expect("schlägt auf");
            if p.x >= goal_north {
                w = weapon_flags::RELEASE;
                released = Some((sim.flight_data().tick, p));
            }
        }
        step(&mut sim, w);
        if released.is_some() && sim.weapons_in_flight().is_empty() {
            break;
        }
    }
    let (t0, predicted) = released.expect("ausgelöst");
    let (impact, result) = events_since(&sim, t0)
        .into_iter()
        .find_map(|e| match e {
            WeaponEvent::Impact { position, result, .. } => Some((position, result)),
            _ => None,
        })
        .expect("Einschlag");
    Run { sim, target, predicted, impact, result }
}

/// Die Vorhersage trifft den echten Aufschlag; mit ausgeglichener Querdrift
/// des Anflugs zerstört die Bombe das weiche Ziel.
#[test]
fn free_fall_bomb_hits_the_predicted_point_and_destroys_a_soft_target() {
    let first = bomb_run(TargetClass::Soft, 0.0);
    let err = (first.impact - first.predicted).length();
    assert!(err < 5.0, "Vorhersage {err:.1} m daneben");
    // Querversatz des ersten Anflugs ausgleichen (Drift beim Trimmen).
    let goal = first.sim.targets()[first.target].clone();
    let drift = first.impact.y - goal.east;
    let run = bomb_run(TargetClass::Soft, -drift);
    let miss = ((run.impact.x - goal.north).powi(2) + (run.impact.y - goal.east).powi(2)).sqrt();
    assert!(miss < 8.0, "Einschlag {miss:.1} m neben dem Ziel");
    assert!(matches!(run.result, ImpactResult::Destroyed { .. }), "{:?}", run.result);
    assert_eq!(run.sim.targets()[run.target].state, TargetState::Destroyed);
}

/// Gehärtetes Ziel: Die waagerecht abgeworfene Bombe fällt zu flach ein und
/// bleibt wirkungslos (M-012).
#[test]
fn level_bomb_is_ineffective_against_a_bunker() {
    let first = bomb_run(TargetClass::Hardened, 0.0);
    let east = first.sim.targets()[first.target].east;
    let run = bomb_run(TargetClass::Hardened, east - first.impact.y);
    assert_eq!(run.result, ImpactResult::Ineffective(run.target));
    assert!(run.sim.targets()[run.target].is_intact());
}

/// Tiefflug über Wasser: Die Freifallbombe schlägt im Splitterbereich ein,
/// die Fallschirmbombe weit genug hinter dem Flugzeug.
#[test]
fn retarded_bomb_avoids_the_frag_warning_at_low_level() {
    // Startpunkt über offenem Wasser mit 6 km freier Strecke nach Norden.
    let probe = Sim::new(SimConfig::new(SEED));
    let water =
        |n: f64, e: f64| (0..=24).all(|k| probe.terrain().elevation(n + f64::from(k) * 250.0, e) <= 1.0);
    let start = (-40..=40)
        .flat_map(|i| (-40..=40).map(move |j| (f64::from(i) * 2_000.0, f64::from(j) * 2_000.0)))
        .find(|&(n, e)| water(n, e))
        .expect("6 km offene See nach Norden");
    let frag = |bay: usize| {
        let mut sim = Sim::new(SimConfig {
            seed: SEED,
            spawn: Spawn {
                north: start.0,
                east: start.1,
                altitude: 70.0,
                min_clearance: 0.0,
                heading: 0.0,
                speed: 200.0,
                throttle: 0.7,
                ..Spawn::default()
            },
        });
        // Höhe halten (einfacher Regler auf Höhe und Steigrate).
        let hold = |sim: &mut Sim, w: u8| {
            let fd = sim.flight_data();
            let pull = (0.02 * (70.0 - fd.altitude_m) - 0.08 * fd.vertical_speed_mps).clamp(-0.4, 0.4);
            sim.step(ControlFrame::from_analog(pull, -0.5 * fd.roll, 0.0, 0.8, 0).with_weapons(w));
        };
        let shift = if bay == 1 { weapon_flags::NEXT_BAY } else { 0 };
        hold(&mut sim, weapon_flags::BAY_DOORS | shift);
        for _ in 0..ticks(weapons::DOOR_TIME_S + 0.1) {
            hold(&mut sim, 0);
        }
        hold(&mut sim, weapon_flags::RELEASE);
        for _ in 0..ticks(12.0) {
            hold(&mut sim, 0);
        }
        assert!(sim.weapons_in_flight().is_empty(), "noch im Flug");
        assert_eq!(sim.status(), f29_core::Status::Flying);
        sim.weapon_events().iter().any(|(_, e)| *e == WeaponEvent::FragWarning)
    };
    assert!(frag(0), "Freifall im Tiefflug ohne Splitterwarnung");
    assert!(!frag(1), "Fallschirmbombe mit Splitterwarnung");
}

/// Zielhilfe: Mit aufgeschaltetem Ziel auslösen, wenn die Längsablage null
/// wird – die Bombe schlägt längs der Flugrichtung am Ziel ein.
#[test]
fn release_cue_puts_the_bomb_on_the_target_along_track() {
    let (mut sim, target) = approach_at(TargetClass::Soft, 900.0, 5_000.0, 0.0, Loadout::default());
    sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.7, f29_core::controls::flags::RADAR_TOGGLE));
    // Bis zum gewünschten Ziel weiterschalten (Richtung voraus).
    for _ in 0..40 {
        if sim.sensors().lock.map(|l| l.target) == Some(target) {
            break;
        }
        let look = ControlFrame::look_flags(f29_core::sensors::LookDir::Forward.code());
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.7, look));
    }
    assert_eq!(sim.sensors().lock.map(|l| l.target), Some(target));
    step(&mut sim, weapon_flags::BAY_DOORS);
    let mut released = None;
    for _ in 0..ticks(40.0) {
        let aim = sim.flight_data().bomb_aim.expect("Zielhilfe bei gewählter Mk 82");
        assert!(!aim.too_low, "900 m: kein Splitterrisiko");
        let (along, _) = aim.target_offset.expect("Ablage zum aufgeschalteten Ziel");
        let w = if released.is_none() && sim.stores().door_open() && along <= 0.0 {
            released = Some((sim.flight_data().tick, sim.aircraft().velocity));
            weapon_flags::RELEASE
        } else {
            0
        };
        step(&mut sim, w);
        if released.is_some() && sim.weapons_in_flight().is_empty() {
            break;
        }
    }
    let (t0, v) = released.expect("Auslöseklammer geschlossen");
    let impact = events_since(&sim, t0)
        .into_iter()
        .find_map(|e| match e {
            WeaponEvent::Impact { position, .. } => Some(position),
            _ => None,
        })
        .expect("Einschlag");
    let goal = &sim.targets()[target];
    let h = f29_core::DVec3::new(v.x, v.y, 0.0).normalize_or_zero();
    let along_miss = f29_core::DVec3::new(goal.north - impact.x, goal.east - impact.y, 0.0).dot(h);
    assert!(along_miss.abs() < 20.0, "längs {along_miss:.1} m daneben");
}

/// Zielhilfe warnt im Tiefflug vor dem eigenen Splitterbereich – mit der
/// Snakeye nicht.
#[test]
fn bomb_aim_warns_too_low_except_with_the_snakeye() {
    let probe = Sim::new(SimConfig::new(SEED));
    let water =
        |n: f64, e: f64| (0..=12).all(|k| probe.terrain().elevation(n + f64::from(k) * 250.0, e) <= 1.0);
    let start = (-40..=40)
        .flat_map(|i| (-40..=40).map(move |j| (f64::from(i) * 2_000.0, f64::from(j) * 2_000.0)))
        .find(|&(n, e)| water(n, e))
        .expect("offene See");
    let too_low = |bay_shift: bool| {
        let mut sim = Sim::new(SimConfig {
            seed: SEED,
            spawn: Spawn {
                north: start.0,
                east: start.1,
                altitude: 70.0,
                min_clearance: 0.0,
                heading: 0.0,
                speed: 200.0,
                throttle: 0.8,
                ..Spawn::default()
            },
        });
        step(&mut sim, if bay_shift { weapon_flags::NEXT_BAY } else { 0 });
        for _ in 0..12 {
            step(&mut sim, 0);
        }
        sim.flight_data().bomb_aim.expect("Zielhilfe").too_low
    };
    assert!(too_low(false), "Mk 82 im Tiefflug ohne Warnung");
    assert!(!too_low(true), "Snakeye mit Warnung");
}
