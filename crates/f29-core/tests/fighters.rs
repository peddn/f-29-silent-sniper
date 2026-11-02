//! Feindliche Jäger (M-022) im ganzen Spiel: Alarm durch eine Radarstellung
//! lässt sie starten, mit Meldung; Anzeigedaten.

mod common;
use common::*;

use f29_core::sim::Spawn;
use f29_core::targets::{Target, TargetClass};
use f29_core::terrain::AirfieldKind;
use f29_core::{ControlFrame, Sim, SimConfig};

/// 15 km südlich der SAM-Stellung, die einem feindlichen Horst am nächsten
/// liegt, 3 km darüber.
fn near_base(rest: Spawn) -> (Sim, usize) {
    let probe = Sim::new(SimConfig::new(SEED));
    let bases: Vec<[f64; 2]> = probe
        .terrain()
        .runways()
        .iter()
        .filter(|r| r.kind == AirfieldKind::Enemy)
        .map(|r| r.center)
        .collect();
    let to_base =
        |t: &Target| bases.iter().map(|c| (c[0] - t.north).hypot(c[1] - t.east)).fold(f64::MAX, f64::min);
    let (i, t) = probe
        .targets()
        .iter()
        .enumerate()
        .filter(|(_, t)| t.class == TargetClass::AirDefense)
        .min_by(|a, b| to_base(a.1).total_cmp(&to_base(b.1)))
        .expect("Stellung");
    approach_target(SEED, i, t, 15_000.0, 0.0, 3_000.0, rest)
}

/// Voll sichtbar über einer SAM-Stellung: Sobald sie das Flugzeug verfolgt,
/// starten Jäger, die Karte sieht sie, und es gibt eine Startmeldung.
#[test]
fn alert_scrambles_fighters_with_a_message() {
    let (mut sim, _) = near_base(Spawn { visible: true, ..Spawn::default() });
    assert!(sim.flight_data().fighters.is_empty());
    let mut message = false;
    for _ in 0..ticks(90.0) {
        step(&mut sim, 0, 0);
        message |= sim.flight_data().weapon_messages.iter().any(|m| m.text.contains("TAKEOFF"));
        if message {
            break;
        }
    }
    assert!(message, "keine Startmeldung");
    fly(&mut sim, 30.0, 0, 0);
    let fd = sim.flight_data();
    assert!(!fd.fighters.is_empty(), "keine Jäger in der Luft");
    assert!(fd.fighters.iter().all(|f| f.label.starts_with("MIG")));
}

/// Mit eingeschaltetem Radar wählt die Erfassung automatisch den nächsten
/// Jäger voraus; eine Richtungstaste mit Zielart Jäger wählt neu, ohne die
/// Bodenzielwahl anzutasten.
#[test]
fn fighters_are_picked_separately_from_ground_targets() {
    use f29_core::controls::{flags, weapon_flags};
    use f29_core::sensors::LookDir;
    let (mut sim, _) = near_base(Spawn { visible: true, ..Spawn::default() });
    step(&mut sim, flags::RADAR_TOGGLE, 0);
    let mut ap = f29_core::autopilot::Autopilot::new(-sim.aircraft().position.z, 0.0, 250.0);
    let mut picked = false;
    for _ in 0..ticks(240.0) {
        // Auf den nächsten Jäger zudrehen.
        let fd = sim.flight_data();
        if let Some(f) = fd.fighters.iter().min_by(|a, b| {
            (a.position - fd.position).length().total_cmp(&(b.position - fd.position).length())
        }) {
            let to = f.position - fd.position;
            ap.target_heading = to.y.atan2(to.x);
        }
        let c = ap.control(sim.spec(), &fd, f29_core::DT);
        sim.step(c);
        if sim.flight_data().air_target.is_some() {
            picked = true;
            break;
        }
    }
    assert!(picked, "kein Jäger gewählt");
    let ground = sim.flight_data().target.map(|t| t.index);
    let look = ControlFrame::look_flags(LookDir::Forward.code());
    step(&mut sim, look, weapon_flags::PICK_AIR);
    let fd = sim.flight_data();
    assert!(fd.air_target.is_some_and(|t| t.label.starts_with("MIG")));
    assert_eq!(fd.target.map(|t| t.index), ground, "Bodenzielwahl verändert");
    assert!(fd.marks.iter().any(|m| m.kind == f29_core::MarkKind::Air), "kein Viereck um Jäger");
}

/// NAV-Modus: Richtungstasten wählen Landebahnen; die Heimatbasis ist
/// freundlich, die Fliegerhorste feindlich.
#[test]
fn runways_are_picked_with_their_side() {
    use f29_core::controls::weapon_flags;
    use f29_core::sensors::LookDir;
    let mut sim = Sim::new(SimConfig::new(SEED));
    let pos = sim.aircraft().position;
    let heading = sim.aircraft().attitude.to_euler().0;
    let side_of = |sim: &mut Sim, kind: AirfieldKind| {
        let rw = sim.terrain().runways().iter().find(|r| r.kind == kind).expect("Bahn").clone();
        let bearing = (rw.center[1] - pos.y).atan2(rw.center[0] - pos.x);
        let look = LookDir::from_relative_bearing(bearing - heading);
        // Zweimal dieselbe Richtung schaltet weiter; bis zu fünfmal drücken.
        for _ in 0..5 {
            sim.step(
                ControlFrame::from_analog(0.0, 0.0, 0.0, 0.55, ControlFrame::look_flags(look.code()))
                    .with_weapons(weapon_flags::PICK_RUNWAY),
            );
            if let Some(t) = sim.flight_data().runway_target.filter(|t| t.label == rw.name) {
                return t.side;
            }
        }
        panic!("{} nicht wählbar", rw.name);
    };
    assert_eq!(side_of(&mut sim, AirfieldKind::Home), f29_core::Side::Friendly);
    assert_eq!(side_of(&mut sim, AirfieldKind::Enemy), f29_core::Side::Hostile);
}

/// Stufen: Mit AWACS sind alle Jäger bekannt, entdeckt nur die in Reichweite
/// der eigenen Sensoren (nachts, Radar aus: optisch 20 km); ohne AWACS
/// kennen HUD und Karte nur die entdeckten.
#[test]
fn awacs_reports_what_own_sensors_do_not_detect() {
    use f29_core::{MarkKind, Stage};
    let start = |awacs: bool| {
        let spawn = Spawn { visible: true, night: true, awacs, invulnerable: true, ..Spawn::default() };
        near_base(spawn).0
    };
    let (mut with, mut without) = (start(true), start(false));
    let range = f29_core::sensors::OPTICAL_RANGE_M * f29_core::sensors::NIGHT_OPTICAL_FACTOR;
    let mut stages = [false; 2];
    for _ in 0..ticks(300.0) {
        step(&mut with, 0, 0);
        step(&mut without, 0, 0);
        let fd = with.flight_data();
        let air: Vec<_> = fd.marks.iter().filter(|m| m.kind == MarkKind::Air).collect();
        assert_eq!(air.len(), fd.fighters.len(), "AWACS meldet alle Jäger");
        for m in &air {
            let far = (m.position - fd.position).length() > range;
            assert!(!far || m.stage == Stage::Reported, "zu weit für die Optik und doch entdeckt");
            stages[usize::from(m.stage == Stage::Detected)] = true;
        }
        let detected = air.iter().filter(|m| m.stage == Stage::Detected).count();
        let fd = without.flight_data();
        assert_eq!(fd.fighters.len(), detected, "Karte ohne AWACS: nur entdeckte");
        assert!(fd.marks.iter().filter(|m| m.kind == MarkKind::Air).all(|m| m.stage == Stage::Detected));
    }
    assert_eq!(stages, [true; 2], "gemeldete und entdeckte Jäger kamen nicht beide vor");
}
