//! Feindliche Boden-Luft-Raketen (M-021): Start nach „Radar ID“, Lenkung auf
//! das Flugzeug, Treffer mit Folgen nur über die Bordsysteme.

mod common;
use common::*;
#[path = "support/sam_report.rs"]
#[allow(dead_code)]
mod sam_report;

use f29_core::controls::weapon_flags;
use f29_core::sim::Spawn;
use f29_core::systems::System;
use f29_core::targets::TargetClass;
use f29_core::weapons::{Struck, WeaponEvent, WeaponKind};
use f29_core::{ControlFrame, Sim, SimConfig, TICK_HZ};

/// Luftstart `dist` m südlich der ersten Stellung der Klasse, Kurs Nord auf
/// sie zu, `height` m darüber, Schacht offen (gut sichtbar).
fn toward_site(class: TargetClass, dist: f64, height: f64) -> (Sim, usize) {
    toward_site_with(class, dist, height, false)
}

/// Wie [`toward_site`]; `visible` schaltet das Stealth-Level ab (jeder Ping
/// trifft).
fn toward_site_with(class: TargetClass, dist: f64, height: f64, visible: bool) -> (Sim, usize) {
    let (mut sim, i) =
        approach(SEED, class, dist, 0.0, height, Spawn { min_clearance: 300.0, visible, ..Spawn::default() })
            .expect("Stellung");
    step(&mut sim, 0, weapon_flags::NEXT_BAY | weapon_flags::BAY_DOORS);
    (sim, i)
}

/// Fliegt geradeaus, bis `done` oder `seconds` vorbei sind.
fn fly_until(sim: &mut Sim, seconds: f64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..ticks(seconds) {
        step(sim, 0, 0);
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
    let hostile: Vec<_> = sim.munitions().in_flight.iter().filter(|w| w.owner.is_some()).collect();
    assert_eq!(hostile.len(), 1, "eine Rakete je Stellung");
    fly(&mut sim, 1.0, 0, 0);
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
        s.weapon_events().iter().any(|(_, e)| matches!(e, WeaponEvent::Hit { on: Struck::Aircraft, .. }))
    };
    assert!(fly_until(&mut sim, 60.0, hit), "kein Treffer");
    // Klang: Treffer am eigenen Flugzeug
    let blast = sim.flight_data().blast.expect("Detonation");
    assert_eq!(blast.kind, f29_core::BlastKind::AircraftHit);
    let damaged = System::ALL.iter().filter(|&&s| !sim.systems().get(s).is_ok()).count();
    assert_eq!(damaged, 2);
    assert_eq!(sim.systems().damage, 1.0);
    assert_eq!(sim.status(), f29_core::Status::Flying);
    // Das beschädigte Flugzeug rüttelt (hier über der Turbulenzgrenze).
    let mut shake: f64 = 0.0;
    for _ in 0..ticks(1.0) {
        step(&mut sim, 0, 0);
        shake = shake.max(sim.turbulence().length());
    }
    assert!(shake > 0.01, "rüttelt nicht: {shake}");
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
        fly(&mut sim, 40.0, 0, 0);
        sim.state_hash()
    };
    assert_eq!(run(), run());
}

/// Hineindrehen rettet nicht: Die Rakete fliegt mit Vorhalt auf den
/// Treffpunkt. Querflug 6 km südlich einer SAM-Stellung (ohne Stealth), nach
/// dem Start harte Kurve auf die Rakete zu.
#[test]
fn turning_into_the_missile_does_not_help() {
    let (_, t) = first_of(SEED, TargetClass::AirDefense).expect("Stellung");
    let mut sim = Sim::new(SimConfig::with(
        SEED,
        Spawn {
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
    ));
    assert!(fly_until(&mut sim, 40.0, |s| launched(s).is_some()), "kein Start");
    let hit = |s: &Sim| {
        s.weapon_events().iter().any(|(_, e)| matches!(e, WeaponEvent::Hit { on: Struck::Aircraft, .. }))
    };
    let mut was_hit = false;
    for _ in 0..(30 * TICK_HZ) {
        let fd = sim.flight_data();
        let Some(m) = sim.munitions().in_flight.iter().find(|w| w.owner.is_some()) else { break };
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

/// Die Kurzstrecken-Flugabwehr bei den Zielgruppen startet die SA-13 direkt
/// auf das Flugzeug, ohne Boost.
#[test]
fn short_range_sam_fires_directly() {
    let (mut sim, site) = toward_site_with(TargetClass::ShortRangeSam, 8_000.0, 1_500.0, true);
    assert!(fly_until(&mut sim, 30.0, |s| launched(s).is_some()), "kein Start");
    assert_eq!(launched(&sim), Some((WeaponKind::ShortRangeSam, site)));
    let w = sim.munitions().in_flight.iter().find(|w| w.owner.is_some()).unwrap();
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
    let start = sim.munitions().in_flight.iter().find(|w| w.owner.is_some()).unwrap().position;
    let mut top: f64 = 0.0;
    fly_until(&mut sim, 30.0, |s| match s.munitions().in_flight.iter().find(|w| w.owner.is_some()) {
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

/// Wartet auf den Start, dann fliegt geradeaus und wirft `decoy`, sobald die
/// Rakete näher als `at_m` ist. Liefert die Ereignisse danach.
fn decoy_run(class: TargetClass, dist: f64, height: f64, decoy: u16, at_m: f64) -> Vec<WeaponEvent> {
    let (mut sim, site) = toward_site_with(class, dist, height, true);
    // Nur diese Stellung schießt.
    for k in 0..sim.targets().len() {
        if k != site && sim.targets()[k].class.is_radar() {
            sim.destroy_target(k);
        }
    }
    assert!(fly_until(&mut sim, 40.0, |s| launched(s).is_some()), "kein Start");
    let t0 = sim.flight_data().tick;
    let mut thrown = false;
    for _ in 0..(20 * TICK_HZ) {
        let near = sim
            .munitions()
            .in_flight
            .iter()
            .filter(|w| w.owner.is_some())
            .any(|w| (w.position - sim.aircraft().position).length() < at_m);
        let w = if near && !thrown { decoy } else { 0 };
        thrown |= w != 0;
        // Werfen, dann hart abdrehen (75° Querlage, ziehen): Die geblendete
        // Rakete findet das Flugzeug danach nicht wieder.
        let fd = sim.flight_data();
        let (roll, pull) =
            if thrown { (((75f64.to_radians() - fd.roll) * 2.0).clamp(-1.0, 1.0), 0.8) } else { (0.0, 0.0) };
        sim.step(ControlFrame::from_analog(pull, roll, 0.0, 0.7, 0).with_weapons(w));
        if thrown && sim.munitions().in_flight.iter().all(|w| w.owner.is_none()) {
            break;
        }
    }
    assert!(thrown, "Rakete kam nicht nah genug");
    events_since(&sim, t0)
}

/// Ein Düppel kurz vor dem Einschlag blendet die Radarrakete (SA-6); wer
/// dann abdreht, ist sie los.
#[test]
fn chaff_decoys_the_radar_missile() {
    let ev = decoy_run(TargetClass::AirDefense, 12_000.0, 1_500.0, weapon_flags::CHAFF, 2_000.0);
    assert!(
        ev.iter().any(|e| matches!(e, WeaponEvent::Decoyed { kind: WeaponKind::MediumRangeSam, .. })),
        "{ev:?}"
    );
    assert!(!ev.iter().any(|e| matches!(e, WeaponEvent::Hit { on: Struck::Aircraft, .. })), "{ev:?}");
}

/// Die Infrarotrakete (SA-13) lässt sich von Düppeln nicht beirren, wohl
/// aber von einer Fackel.
#[test]
fn flare_decoys_the_infrared_missile_but_chaff_does_not() {
    // Hoch genug für freie Sicht (Infrarot braucht sie).
    let chaff = decoy_run(TargetClass::ShortRangeSam, 8_000.0, 3_000.0, weapon_flags::CHAFF, 1_500.0);
    // (Andere Stellungen schießen mit; es zählt nur die SA-13.)
    let decoyed = |ev: &[WeaponEvent]| {
        ev.iter().find_map(|e| match e {
            WeaponEvent::Decoyed { kind: WeaponKind::ShortRangeSam, id } => Some(*id),
            _ => None,
        })
    };
    assert_eq!(decoyed(&chaff), None, "{chaff:?}");
    let flare = decoy_run(TargetClass::ShortRangeSam, 8_000.0, 3_000.0, weapon_flags::FLARE, 1_500.0);
    let id = decoyed(&flare).expect("SA-13 abgelenkt");
    assert!(
        !flare.iter().any(|e| matches!(e, WeaponEvent::Hit { id: x, on: Struck::Aircraft, .. } if *x == id)),
        "{flare:?}"
    );
}

/// Vorrat 20 je Art, höchstens drei gleichzeitig; leer meldet sich.
#[test]
fn decoy_stock_is_limited() {
    let (mut sim, _) = toward_site(TargetClass::AirDefense, 30_000.0, 4_000.0);
    // Jeder Tastendruck wirft einen aus, ohne Grenze in der Luft.
    for _ in 0..5 {
        step(&mut sim, 0, weapon_flags::FLARE);
    }
    assert_eq!(sim.flight_data().decoys.len(), 5);
    assert_eq!(sim.flight_data().stores.flares, 15);
    for _ in 0..40 {
        step(&mut sim, 0, weapon_flags::FLARE);
        fly(&mut sim, 2.0, 0, 0);
    }
    assert_eq!(sim.flight_data().stores.flares, 0);
    assert_eq!(sim.flight_data().stores.chaff, 20);
    assert!(
        sim.weapon_events()
            .iter()
            .any(|(_, e)| *e == WeaponEvent::DecoyEmpty(f29_core::weapons::DecoyKind::Flare))
    );
}

/// ECM: ein/aus, macht sichtbarer; Infrarotstörer: 30 s, ohne Zuschlag.
#[test]
fn jammers_switch_and_cost_stealth() {
    let (mut sim, _) = toward_site(TargetClass::AirDefense, 30_000.0, 4_000.0);
    let stealth = |s: &Sim| s.flight_data().threat.stealth;
    let before = stealth(&sim);
    step(&mut sim, 0, weapon_flags::ECM);
    assert!(sim.flight_data().ecm);
    assert!(stealth(&sim) < before - 0.1, "ECM kostet Stealth");
    step(&mut sim, 0, weapon_flags::ECM);
    assert!(!sim.flight_data().ecm);
    let off = stealth(&sim);
    step(&mut sim, 0, weapon_flags::IR_JAMMER);
    assert!(sim.flight_data().ir_jammer_s > 29.0);
    assert!((stealth(&sim) - off).abs() < 0.01, "Infrarotstörer ohne Zuschlag");
    fly(&mut sim, 30.5, 0, 0);
    assert_eq!(sim.flight_data().ir_jammer_s, 0.0);
    // Abklingzeit: erneutes Einschalten wirkt erst nach 60 s
    assert!(sim.flight_data().ir_cooldown_s > 59.0);
    step(&mut sim, 0, weapon_flags::IR_JAMMER);
    assert_eq!(sim.flight_data().ir_jammer_s, 0.0, "noch in der Abklingzeit");
    fly(&mut sim, 60.0, 0, 0);
    step(&mut sim, 0, weapon_flags::IR_JAMMER);
    assert!(sim.flight_data().ir_jammer_s > 29.0, "wieder bereit");
}

/// Gefährlichkeit (Prüfstand `sam_report`): Von vorn treffen alle; Querflug
/// schüttelt SA-5 und SA-13 ab, die moderne SA-11 nicht (Spielerwunsch
/// v0.2.9: „mit Querflug das meiste abschütteln“).
#[test]
fn beaming_shakes_off_all_but_the_modern_sam() {
    use sam_report::{Maneuver, engagement};
    let hit = |kind, m| engagement(SEED, kind, 0.5, 0.6, m).expect("Start ohne Absturz");
    for kind in [
        WeaponKind::ShortRangeSam,
        WeaponKind::LongRangeSam,
        WeaponKind::MediumRangeSam,
        WeaponKind::ModernSam,
    ] {
        assert!(hit(kind, Maneuver::Toward), "{kind:?} trifft von vorn nicht");
    }
    assert!(!hit(WeaponKind::LongRangeSam, Maneuver::Beam), "SA-5 querab nicht abgeschüttelt");
    assert!(!hit(WeaponKind::ShortRangeSam, Maneuver::Beam), "SA-13 querab nicht abgeschüttelt");
    assert!(hit(WeaponKind::ModernSam, Maneuver::Beam), "SA-11 querab abgeschüttelt");
}

/// `--no-damage`: Der Treffer wird gemeldet, die Systeme bleiben heil.
#[test]
fn no_damage_reports_hits_without_consequences() {
    let (_, t) = first_of(SEED, TargetClass::AirDefense).expect("Stellung");
    let mut sim = Sim::new(SimConfig::with(
        SEED,
        Spawn {
            north: t.north - 7_000.0,
            east: t.east,
            altitude: t.elevation + 800.0,
            min_clearance: 300.0,
            heading: 0.0,
            speed: 200.0,
            throttle: 0.7,
            invulnerable: true,
            ..Spawn::default()
        },
    ));
    step(&mut sim, 0, weapon_flags::NEXT_BAY | weapon_flags::BAY_DOORS);
    let hit = |s: &Sim| {
        s.weapon_events().iter().any(|(_, e)| matches!(e, WeaponEvent::Hit { on: Struck::Aircraft, .. }))
    };
    assert!(fly_until(&mut sim, 60.0, hit), "kein Treffer");
    assert!(System::ALL.iter().all(|s| sim.systems().get(*s).is_ok()), "Schaden trotz --no-damage");
    assert_eq!(sim.systems().hits, 0);
}

/// Zeitraffer (M-034): Im ruhigen Flug kein Abbruch; der Start einer SAM
/// beendet ihn; mit ausgefahrenem Fahrwerk gibt es keinen.
#[test]
fn time_warp_ends_on_launch_and_gear() {
    let (mut sim, _) = toward_site(TargetClass::AirDefense, 7_000.0, 800.0);
    assert!(!sim.interrupts_time_warp());
    let mut stopped = false;
    assert!(fly_until(&mut sim, 30.0, |s| {
        stopped |= s.interrupts_time_warp();
        launched(s).is_some()
    }));
    assert!(stopped, "Start ohne Abbruch");
    // Auf der Bahn ist das Fahrwerk draußen: kein Zeitraffer.
    let quiet = Sim::new(SimConfig::runway(SEED));
    assert!(quiet.interrupts_time_warp(), "Fahrwerk");
}
