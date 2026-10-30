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
    assert_eq!(launched(&sim), Some((WeaponKind::SurfaceToAir, site)));
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
