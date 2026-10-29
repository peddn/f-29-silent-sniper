//! Zielerfassung (M-013): Radar ohne Sichtlinie, optisch mit Sichtlinie und
//! kürzerer Reichweite, automatisch auf das nächste Ziel der Blickrichtung.

use f29_core::controls::flags;
use f29_core::detmath;
use f29_core::sensors::{self, CONE_HALF_RAD, LookDir, OPTICAL_RANGE_M, RADAR_RANGE_M};
use f29_core::sim::Spawn;
use f29_core::{ControlFrame, Sim, SimConfig};

const SEED: u64 = 0x0F29;

/// Luftstart 12 km südlich des Primärziels, Kurs Nord, 3000 m.
fn near_primary() -> Sim {
    let probe = Sim::new(SimConfig::new(SEED));
    let t =
        probe.targets().iter().find(|t| t.role == f29_core::targets::TargetRole::Primary).unwrap().clone();
    Sim::new(SimConfig {
        seed: SEED,
        spawn: Spawn {
            north: t.north - 12_000.0,
            east: t.east,
            altitude: 3_000.0,
            min_clearance: 600.0,
            heading: 0.0,
            speed: 180.0,
            throttle: 0.7,
            ..Spawn::default()
        },
    })
}

/// Nächstes intaktes Ziel im Kegel der Blickrichtung bis `range` (ohne Sichtlinie).
fn nearest_in_cone(sim: &Sim, look: LookDir, range: f64) -> Option<usize> {
    let a = sim.aircraft();
    let heading = a.attitude.to_euler().0 + look.offset();
    sim.targets()
        .iter()
        .enumerate()
        .filter(|(_, t)| t.is_intact())
        .filter_map(|(i, t)| {
            let c = t.center_ned();
            let d = [c[0] - a.position.x, c[1] - a.position.y, c[2] - a.position.z];
            let r = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            let off = detmath::wrap_pi(detmath::atan2(d[1], d[0]) - heading).abs();
            (r <= range && off <= CONE_HALF_RAD).then_some((r, i))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, i)| i)
}

fn step(sim: &mut Sim, flags: u8) {
    sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.7, flags));
}

#[test]
fn radar_locks_the_nearest_target_ahead_and_can_be_switched_off() {
    let mut sim = near_primary();
    assert!(!sim.sensors().radar_on, "Radar startet ausgeschaltet");
    step(&mut sim, flags::RADAR_TOGGLE);
    assert!(sim.sensors().radar_on);
    // Eine bestehende Aufschaltung bleibt; Richtung neu wählen setzt auf das nächste Ziel.
    step(&mut sim, ControlFrame::look_flags(LookDir::Right.code()));
    step(&mut sim, ControlFrame::look_flags(LookDir::Forward.code()));
    let lock = sim.sensors().lock.expect("Radar schaltet auf");
    assert!(lock.radar);
    assert_eq!(Some(lock.target), nearest_in_cone(&sim, LookDir::Forward, RADAR_RANGE_M));
    let fd = sim.flight_data();
    let t = fd.target.expect("Zieldaten");
    assert!(t.range <= RADAR_RANGE_M && fd.radar_on);

    // Radar aus: nur noch optisch – in optischer Reichweite und mit freier Sicht.
    step(&mut sim, flags::RADAR_TOGGLE);
    assert!(!sim.sensors().radar_on);
    if let Some(l) = sim.sensors().lock {
        assert!(l.optical && !l.radar);
        assert!(sim.flight_data().target.unwrap().range <= OPTICAL_RANGE_M);
    }
}

#[test]
fn look_direction_selects_the_cone() {
    let mut sim = near_primary();
    step(&mut sim, flags::RADAR_TOGGLE);
    // Mit Rechts beginnen: Vorn ist schon gewählt, dort würde der Tastendruck weiterschalten.
    for look in [LookDir::Right, LookDir::Aft, LookDir::Left, LookDir::Forward] {
        step(&mut sim, ControlFrame::look_flags(look.code()));
        assert_eq!(sim.sensors().look, look);
        assert_eq!(
            sim.sensors().lock.map(|l| l.target),
            nearest_in_cone(&sim, look, RADAR_RANGE_M),
            "Blickrichtung {look:?}"
        );
    }
}

#[test]
fn optical_lock_needs_line_of_sight_and_is_shorter_ranged() {
    // Viele Positionen abfliegen: Jede optische Erfassung liegt in Reichweite
    // und hat freie Sicht; jede reine Radarerfassung ohne optische hat keine
    // (oder liegt außerhalb der optischen Reichweite).
    let mut sim = near_primary();
    step(&mut sim, flags::RADAR_TOGGLE);
    let (mut optical, mut radar_only) = (0, 0);
    for i in 0..(120 * 120) {
        // Die Aufschaltung hält ein Ziel; ab und zu weiterschalten, damit auch
        // fernere (reine Radar-)Ziele drankommen.
        let cycle = i % (sensors::UPDATE_TICKS as usize * 4) == 1;
        step(&mut sim, if cycle { ControlFrame::look_flags(LookDir::Forward.code()) } else { 0 });
        if i % sensors::UPDATE_TICKS as usize != 0 {
            continue;
        }
        let Some(t) = sim.flight_data().target else { continue };
        let a = sim.aircraft().position;
        let tgt = &sim.targets()[t.index];
        let c = tgt.center_ned();
        let los = sensors::line_of_sight(
            |n, e| sim.terrain().elevation(n, e),
            [a.x, a.y, -a.z],
            [c[0], c[1], -c[2]],
        );
        if t.optical {
            optical += 1;
            assert!(t.range <= OPTICAL_RANGE_M && los, "optisch ohne Sicht oder zu weit");
        } else {
            radar_only += 1;
            assert!(t.range > OPTICAL_RANGE_M || !los, "Sicht frei, aber nicht optisch erfasst");
        }
    }
    assert!(optical > 0 && radar_only > 0, "optisch {optical}, nur Radar {radar_only}");
}

/// Dieselbe Richtung noch einmal wählen: zum nächstweiteren Ziel, nach dem
/// weitesten wieder zum nächsten; die Aufschaltung bleibt zwischen den Tasten.
#[test]
fn same_direction_cycles_through_targets_by_range() {
    let mut sim = near_primary();
    step(&mut sim, flags::RADAR_TOGGLE);
    step(&mut sim, ControlFrame::look_flags(LookDir::Right.code()));
    step(&mut sim, ControlFrame::look_flags(LookDir::Forward.code()));
    let first = sim.sensors().lock.expect("Aufschaltung").target;
    let range = |sim: &Sim, i: usize| {
        let c = sim.targets()[i].center_ned();
        let p = sim.aircraft().position;
        ((c[0] - p.x).powi(2) + (c[1] - p.y).powi(2) + (c[2] - p.z).powi(2)).sqrt()
    };
    let mut seen = vec![first];
    for _ in 0..40 {
        let before = sim.sensors().lock.unwrap().target;
        step(&mut sim, ControlFrame::look_flags(LookDir::Forward.code()));
        let after = sim.sensors().lock.expect("bleibt aufgeschaltet").target;
        if after == first {
            break;
        }
        assert!(range(&sim, after) >= range(&sim, before) - 50.0, "Weiterschalten springt näher");
        seen.push(after);
        // Zwischen den Tastendrücken hält die Erfassung das gewählte Ziel.
        for _ in 0..(f29_core::sensors::UPDATE_TICKS * 2) {
            step(&mut sim, 0);
        }
        assert_eq!(sim.sensors().lock.unwrap().target, after, "Aufschaltung springt zurück");
    }
    assert!(seen.len() >= 2, "nur {} Ziel(e) im Kegel", seen.len());
    let mut unique = seen.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), seen.len(), "ein Ziel doppelt vor dem Umlauf");
}
