//! Feindliche Boden-Luft-Raketen (M-021): Start nach „Radar ID“, Lenkung auf
//! das Flugzeug, Treffer mit Folgen nur über die Bordsysteme.

use f29_core::controls::weapon_flags;
use f29_core::sim::Spawn;
use f29_core::systems::System;
use f29_core::targets::TargetClass;
use f29_core::weapons::{WeaponEvent, WeaponKind};
use f29_core::{ControlFrame, Sim, SimConfig, TICK_HZ};

const SEED: u64 = 0x0F29;

/// Luftstart `dist` m südlich der ersten Stellung der Klasse, Kurs Nord auf
/// sie zu, `height` m darüber, Schacht offen (gut sichtbar).
fn toward_site(class: TargetClass, dist: f64, height: f64) -> (Sim, usize) {
    toward_site_with(class, dist, height, false)
}

/// Wie [`toward_site`]; `visible` schaltet das Stealth-Level ab (jeder Ping
/// trifft).
fn toward_site_with(class: TargetClass, dist: f64, height: f64, visible: bool) -> (Sim, usize) {
    let probe = Sim::new(SimConfig::new(SEED));
    let (i, t) = probe.targets().iter().enumerate().find(|(_, t)| t.class == class).expect("Stellung");
    let mut sim = Sim::new(SimConfig {
        seed: SEED,
        spawn: Spawn {
            north: t.north - dist,
            east: t.east,
            altitude: t.elevation + height,
            min_clearance: 300.0,
            heading: 0.0,
            speed: 200.0,
            throttle: 0.7,
            visible,
            ..Spawn::default()
        },
    });
    sim.step(
        ControlFrame::from_analog(0.0, 0.0, 0.0, 0.7, 0)
            .with_weapons(weapon_flags::NEXT_BAY | weapon_flags::BAY_DOORS),
    );
    (sim, i)
}

/// Fliegt geradeaus, bis `done` oder `seconds` vorbei sind.
fn fly_until(sim: &mut Sim, seconds: f64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..(seconds * f64::from(TICK_HZ)) as usize {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.7, 0));
        if done(sim) {
            return true;
        }
    }
    false
}

fn launched(sim: &Sim) -> Option<(WeaponKind, usize)> {
    sim.weapon_events().iter().find_map(|(_, e)| match e {
        WeaponEvent::SamLaunch { kind, site, .. } => Some((*kind, *site)),
        _ => None,
    })
}

/// Auf eine SAM-Stellung zu: Nach „Radar ID“ startet sie beim nächsten
/// Treffer eine SA-6; die Raketenwarnung geht an.
#[test]
fn sam_site_fires_after_radar_id() {
    let (mut sim, site) = toward_site(TargetClass::AirDefense, 7_000.0, 800.0);
    assert!(fly_until(&mut sim, 30.0, |s| launched(s).is_some()), "kein Start");
    assert_eq!(launched(&sim), Some((WeaponKind::MediumRangeSam, site)));
    assert_eq!(sim.radar().sites.iter().find(|s| s.target == site).unwrap().alert, 3);
    let hostile: Vec<_> = sim.weapons_in_flight().iter().filter(|w| w.owner.is_some()).collect();
    assert_eq!(hostile.len(), 1, "eine Rakete je Stellung");
    fly_until(&mut sim, 1.0, |_| false);
    let fd = sim.flight_data();
    assert!(fd.warnings.missile);
    assert!(fd.munitions.iter().any(|m| m.hostile && m.missile));
    assert!(
        fd.weapon_messages.iter().any(|m| m.text.starts_with("MISSILE LAUNCH SA-6")),
        "{:?}",
        fd.weapon_messages
    );
}

/// Wer geradeaus weiterfliegt, wird getroffen: Zwei Bordsysteme sind
/// beschädigt, sonst geschieht nichts (keine Trefferpunkte).
#[test]
fn missile_hit_damages_two_systems() {
    let (mut sim, _) = toward_site(TargetClass::AirDefense, 7_000.0, 800.0);
    let hit = |s: &Sim| {
        s.weapon_events().iter().find_map(|(_, e)| match e {
            WeaponEvent::AircraftHit { systems, .. } => Some(*systems),
            _ => None,
        })
    };
    assert!(fly_until(&mut sim, 60.0, |s| hit(s).is_some()), "kein Treffer");
    let systems = hit(&sim).unwrap();
    assert_ne!(systems[0], systems[1]);
    for s in System::ALL {
        let damaged = !sim.systems().get(s).is_ok();
        assert_eq!(damaged, systems.contains(&s), "{s:?}");
    }
    assert_eq!(sim.status(), f29_core::Status::Flying);
}

/// Das Frühwarnradar startet die Langstreckenrakete.
#[test]
fn early_warning_radar_fires_long_range_missile() {
    let (mut sim, site) = toward_site(TargetClass::EarlyWarning, 20_000.0, 3_000.0);
    assert!(fly_until(&mut sim, 60.0, |s| launched(s).is_some()), "kein Start");
    assert_eq!(launched(&sim), Some((WeaponKind::LongRangeSam, site)));
}

#[test]
fn sam_engagement_is_deterministic() {
    let run = || {
        let (mut sim, _) = toward_site(TargetClass::AirDefense, 7_000.0, 800.0);
        fly_until(&mut sim, 40.0, |_| false);
        sim.state_hash()
    };
    assert_eq!(run(), run());
}

/// Hineindrehen rettet nicht: Die Rakete fliegt mit Vorhalt auf den
/// Treffpunkt. Querflug 6 km südlich einer SAM-Stellung (ohne Stealth), nach
/// dem Start harte Kurve auf die Rakete zu.
#[test]
fn turning_into_the_missile_does_not_help() {
    let probe = Sim::new(SimConfig::new(SEED));
    let t = probe.targets().iter().find(|t| t.class == TargetClass::AirDefense).expect("Stellung").clone();
    let mut sim = Sim::new(SimConfig {
        seed: SEED,
        spawn: Spawn {
            north: t.north - 6_000.0,
            east: t.east - 3_000.0,
            altitude: t.elevation + 1_000.0,
            min_clearance: 300.0,
            heading: std::f64::consts::FRAC_PI_2,
            speed: 230.0,
            throttle: 1.0,
            visible: true,
            ..Spawn::default()
        },
    });
    assert!(fly_until(&mut sim, 40.0, |s| launched(s).is_some()), "kein Start");
    let hit = |s: &Sim| s.weapon_events().iter().any(|(_, e)| matches!(e, WeaponEvent::AircraftHit { .. }));
    let mut was_hit = false;
    for _ in 0..(30 * TICK_HZ) {
        let fd = sim.flight_data();
        let Some(m) = sim.weapons_in_flight().iter().find(|w| w.owner.is_some()) else { break };
        // Peilung zur Rakete relativ zum Kurs: Vorzeichen = Kurvenrichtung.
        let to = m.position - fd.position;
        let rel = (to.y.atan2(to.x) - fd.heading + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        let bank = 75f64.to_radians() * rel.signum();
        let roll = ((bank - fd.roll) * 2.0).clamp(-1.0, 1.0);
        let pull = if (bank - fd.roll).abs() < 0.3 { 1.0 } else { 0.2 };
        sim.step(ControlFrame::from_analog(pull, roll, 0.0, 1.0, 0));
        if hit(&sim) {
            was_hit = true;
            break;
        }
    }
    assert!(was_hit, "Rakete verfehlt beim Hineindrehen");
}

/// Die Kurzstrecken-Flugabwehr bei den Zielgruppen startet die SA-8 direkt
/// auf das Flugzeug, ohne Boost.
#[test]
fn short_range_sam_fires_directly() {
    let (mut sim, site) = toward_site_with(TargetClass::ShortRangeSam, 8_000.0, 1_500.0, true);
    assert!(fly_until(&mut sim, 30.0, |s| launched(s).is_some()), "kein Start");
    assert_eq!(launched(&sim), Some((WeaponKind::ShortRangeSam, site)));
    let w = sim.weapons_in_flight().iter().find(|w| w.owner.is_some()).unwrap();
    let to = sim.aircraft().position - w.position;
    let off = (w.velocity.normalize_or_zero().dot(to.normalize_or_zero())).acos().to_degrees();
    assert!(off < 25.0, "startet {off:.0}° neben dem Flugzeug");
}

/// Die SA-5 steigt nach dem fast senkrechten Start hoch und fliegt im Bogen
/// – über jeden Berg.
#[test]
fn long_range_sam_climbs_and_lofts() {
    let (mut sim, _) = toward_site(TargetClass::EarlyWarning, 20_000.0, 3_000.0);
    assert!(fly_until(&mut sim, 60.0, |s| launched(s).is_some()), "kein Start");
    let start = sim.weapons_in_flight().iter().find(|w| w.owner.is_some()).unwrap().position;
    let mut top: f64 = 0.0;
    fly_until(&mut sim, 30.0, |s| match s.weapons_in_flight().iter().find(|w| w.owner.is_some()) {
        Some(w) => {
            top = top.max(start.z - w.position.z);
            false
        }
        None => true,
    });
    let above_aircraft = top - (start.z - sim.aircraft().position.z);
    assert!(
        top > 1_500.0 && above_aircraft > 0.0,
        "Gipfel {top:.0} m, über dem Flugzeug {above_aircraft:.0} m"
    );
}
