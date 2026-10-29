//! Lenkwaffen (M-013, M-014): Übergabe der Aufschaltung, gewürfelter
//! Endanflug abhängig von der Startgüte, Selbstsuche, Reichweitenzonen.

use f29_core::controls::{flags, weapon_flags};
use f29_core::sensors::LookDir;
use f29_core::sim::Spawn;
use f29_core::targets::TargetClass;
use f29_core::weapons::{self, ImpactResult, LaunchZone, Loadout, WeaponEvent, WeaponKind};
use f29_core::{ControlFrame, Sim, SimConfig, TICK_HZ};

fn ticks(s: f64) -> usize {
    (s * f64::from(TICK_HZ)) as usize
}

fn step(sim: &mut Sim, f: u8, w: u8) {
    sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.7, f).with_weapons(w));
}

/// Flug auf das erste Ziel der Klasse zu, `dist` m davor, 1500 m darüber;
/// Radar an und auf dieses Ziel weitergeschaltet. `None`, wenn das Ziel nicht
/// aufschaltbar ist (z. B. Gelände im Weg).
fn lined_up(seed: u64, class: TargetClass, dist: f64, loadout: Loadout) -> Option<(Sim, usize)> {
    lined_up_at(seed, class, dist, 1_500.0, loadout)
}

/// Wie [`lined_up`], aber `height` m über dem Ziel.
fn lined_up_at(
    seed: u64,
    class: TargetClass,
    dist: f64,
    height: f64,
    loadout: Loadout,
) -> Option<(Sim, usize)> {
    let probe = Sim::new(SimConfig::new(seed));
    let (i, t) = probe.targets().iter().enumerate().find(|(_, t)| t.class == class)?;
    let mut sim = Sim::new(SimConfig {
        seed,
        spawn: Spawn {
            north: t.north - dist,
            east: t.east,
            altitude: t.elevation + height,
            min_clearance: 150.0,
            heading: 0.0,
            speed: 200.0,
            throttle: 0.7,
            loadout,
            ..Spawn::default()
        },
    });
    step(&mut sim, flags::RADAR_TOGGLE, 0);
    step(&mut sim, ControlFrame::look_flags(LookDir::Right.code()), 0);
    step(&mut sim, ControlFrame::look_flags(LookDir::Forward.code()), 0);
    for _ in 0..40 {
        if sim.sensors().lock.map(|l| l.target) == Some(i) {
            return Some((sim, i));
        }
        step(&mut sim, ControlFrame::look_flags(LookDir::Forward.code()), 0);
    }
    None
}

/// Schacht öffnen, auslösen, bis zum Ende des Flugs weiterfliegen. Liefert
/// die Ereignisse seit dem Auslösen.
fn shoot(sim: &mut Sim) -> Vec<WeaponEvent> {
    step(sim, 0, weapon_flags::BAY_DOORS);
    for _ in 0..ticks(weapons::DOOR_TIME_S + 0.1) {
        step(sim, 0, 0);
    }
    let t0 = sim.flight_data().tick;
    step(sim, 0, weapon_flags::RELEASE);
    for _ in 0..ticks(90.0) {
        step(sim, 0, 0);
        if sim.weapons_in_flight().is_empty() {
            break;
        }
    }
    sim.weapon_events().iter().filter(|(t, _)| *t > t0).map(|(_, e)| *e).collect()
}

fn impact(events: &[WeaponEvent]) -> Option<ImpactResult> {
    events.iter().find_map(|e| match e {
        WeaponEvent::Impact { result, .. } => Some(*result),
        _ => None,
    })
}

/// Nur Luft-Boden-Raketen an Bord (Schacht 1).
fn agm() -> Loadout {
    Loadout([Some(WeaponKind::AirToGround), None, None, None])
}

#[derive(Debug, Default)]
struct Tally {
    shots: u32,
    destroyed: u32,
    damaged: u32,
    missed: u32,
}

fn tally(dist: f64, height: f64) -> Tally {
    let mut t = Tally::default();
    for seed in 1..=80_u64 {
        let Some((mut sim, target)) = lined_up_at(seed, TargetClass::Soft, dist, height, agm()) else {
            continue;
        };
        let zone = sim.flight_data().target.and_then(|t| t.zone);
        if !matches!(zone, Some(LaunchZone::InRange | LaunchZone::Good)) {
            continue;
        }
        let events = shoot(&mut sim);
        // Nur Schüsse mit Übergabe der Aufschaltung zählen (sonst sucht die
        // Waffe selbst und fliegt womöglich ein anderes Ziel an).
        if !events.iter().any(|e| matches!(e, WeaponEvent::Acquired { target: x, .. } if *x == target)) {
            continue;
        }
        t.shots += 1;
        match impact(&events) {
            Some(ImpactResult::Destroyed { .. }) => t.destroyed += 1,
            Some(ImpactResult::Damaged(_)) => t.damaged += 1,
            _ => t.missed += 1,
        }
    }
    t
}

/// Sweet Spot: Von der Reichweitengrenze steigt die Trefferchance bis zum
/// Sweet Spot an und fällt bei weiterer Annäherung wieder ab. Flache Anflüge,
/// damit das Ziel nahe der Flugrichtung liegt (Ablage spielt kaum mit).
#[test]
fn sweet_spot_hits_more_often_than_too_far_or_too_close() {
    let max = WeaponKind::AirToGround.spec().max_range_m;
    let sweet = tally(max * 0.4, 500.0);
    let far = tally(max * 0.92, 800.0);
    let close = tally(1_500.0, 250.0);
    for t in [&sweet, &far, &close] {
        assert!(t.shots >= 30, "zu wenige Schüsse: {sweet:?} {far:?} {close:?}");
    }
    let rate = |n: u32, t: &Tally| f64::from(n) / f64::from(t.shots);
    let report = format!("Sweet Spot {sweet:?}, Grenze {far:?}, zu nah {close:?}");
    assert!(rate(sweet.destroyed, &sweet) > 0.65, "{report}");
    assert!(rate(far.destroyed, &far) < rate(sweet.destroyed, &sweet) - 0.2, "{report}");
    assert!(rate(close.destroyed, &close) < rate(sweet.destroyed, &sweet) - 0.1, "{report}");
    assert!(rate(far.missed, &far) > rate(sweet.missed, &sweet), "{report}");
    assert!(rate(close.missed, &close) > rate(sweet.missed, &sweet), "{report}");
    eprintln!("{report}");
}

/// Zielrahmen-Zonen: außer Reichweite, in Reichweite, gute Zone; der
/// Radarjäger wirkt nicht gegen weiche Ziele.
#[test]
fn launch_zones_follow_the_range() {
    let max = WeaponKind::AirToGround.spec().max_range_m;
    // Erster Anflug mit (bei `optical`) optischer Erfassung.
    let zone = |dist: f64, loadout, optical: bool| {
        (1..=30_u64)
            .filter_map(|seed| lined_up(seed, TargetClass::Soft, dist, loadout))
            .find_map(|(sim, _)| sim.flight_data().target.filter(|t| t.optical || !optical).map(|t| t.zone))
            .flatten()
    };
    assert_eq!(zone(max * 0.35, agm(), true), Some(LaunchZone::Good));
    // Näher als der Sweet Spot: wieder nur „in Reichweite“.
    assert_eq!(zone(max * 0.1, agm(), true), Some(LaunchZone::InRange));
    assert_eq!(zone(max * 0.8, agm(), true), Some(LaunchZone::InRange));
    assert_eq!(zone(max * 1.3, agm(), false), Some(LaunchZone::OutOfRange));
    let arm = Loadout([Some(WeaponKind::AntiRadar), None, None, None]);
    assert_eq!(zone(max * 0.3, arm, false), Some(LaunchZone::NoEffect));
    // Ungelenkte Waffe gewählt: keine Zone.
    let bombs = Loadout([Some(WeaponKind::FreeFall), None, None, None]);
    assert_eq!(zone(max * 0.3, bombs, false), None);
}

/// Ohne passende Aufschaltung abgefeuert, sucht der Radarjäger selbst – und
/// schaltet nur Flugabwehrstellungen auf.
#[test]
fn unlocked_weapon_searches_for_its_own_kind() {
    let arm = Loadout([Some(WeaponKind::AntiRadar), None, None, None]);
    let mut checked = 0;
    for seed in 1..=10_u64 {
        let Some((mut sim, _)) = lined_up(seed, TargetClass::Soft, 4_000.0, arm) else { continue };
        let events = shoot(&mut sim);
        for e in &events {
            if let WeaponEvent::Acquired { target, .. } = e {
                assert_eq!(sim.targets()[*target].class, TargetClass::AirDefense);
            }
        }
        checked += 1;
    }
    assert!(checked > 3);
}

/// Die Gleitbombe kommt nur so weit, wie ihre Höhe trägt: aus 1500 m über dem
/// Ziel und 12 km Entfernung ist sie außer Reichweite und fällt zu kurz.
#[test]
fn glide_bomb_falls_short_from_too_far() {
    let glide = Loadout([Some(WeaponKind::GuidedBomb), None, None, None]);
    let (mut sim, target) =
        (1..=20_u64).find_map(|s| lined_up(s, TargetClass::Soft, 12_000.0, glide)).expect("Anflug");
    assert_eq!(sim.flight_data().target.and_then(|t| t.zone), Some(LaunchZone::OutOfRange));
    let events = shoot(&mut sim);
    assert!(sim.targets()[target].is_intact());
    assert!(!matches!(impact(&events), Some(ImpactResult::Destroyed { target: t, .. }) if t == target));
}

/// Nach der Zerstörung bleibt die Aufschaltung auf dem Wrack; das Wrack
/// verschwindet von der Karte (Liste der zerstörten Ziele) und ist nach einem
/// Zielwechsel nicht mehr anwählbar.
#[test]
fn lock_stays_on_the_wreck_until_a_new_target_is_chosen() {
    let max = WeaponKind::AirToGround.spec().max_range_m;
    let (mut sim, target) = (1..=40_u64)
        .find_map(|seed| {
            let (mut sim, target) = lined_up_at(seed, TargetClass::Soft, max * 0.4, 500.0, agm())?;
            let events = shoot(&mut sim);
            matches!(impact(&events), Some(ImpactResult::Destroyed { target: t, .. }) if t == target)
                .then_some((sim, target))
        })
        .expect("ein Treffer");
    for _ in 0..ticks(2.0) {
        step(&mut sim, 0, 0);
    }
    let fd = sim.flight_data();
    let t = fd.target.expect("Aufschaltung bleibt");
    assert_eq!(t.index, target);
    assert!(t.destroyed);
    assert_eq!(t.zone, None);
    assert!(fd.destroyed_targets.contains(&target));
    // Neues Ziel wählen: das Wrack ist nicht mehr dabei.
    for look in [LookDir::Forward, LookDir::Right, LookDir::Aft, LookDir::Left, LookDir::Forward] {
        step(&mut sim, ControlFrame::look_flags(look.code()), 0);
        assert_ne!(sim.sensors().lock.map(|l| l.target), Some(target), "Wrack wieder gewählt");
    }
}
