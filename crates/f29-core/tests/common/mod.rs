//! Gemeinsame Helfer der Integrationstests.
#![allow(dead_code)]

use f29_core::sim::Spawn;
use f29_core::targets::{Target, TargetClass};
use f29_core::weapons::WeaponEvent;
use f29_core::{ControlFrame, Sim, SimConfig, TICK_HZ};

pub const SEED: u64 = 0x0F29;

/// Sekunden in Ticks (abgeschnitten).
pub fn ticks(s: f64) -> usize {
    (s * f64::from(TICK_HZ)) as usize
}

/// Ein Tick Geradeausflug mit Schub 0,7; `f` Flugbits, `w` Waffenbits.
pub fn step(sim: &mut Sim, f: u8, w: u16) {
    sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.7, f).with_weapons(w));
}

/// Wie [`step`], `seconds` lang.
pub fn fly(sim: &mut Sim, seconds: f64, f: u8, w: u16) {
    for _ in 0..ticks(seconds) {
        step(sim, f, w);
    }
}

/// Waffenereignisse nach `tick`.
pub fn events_since(sim: &Sim, tick: u64) -> Vec<WeaponEvent> {
    sim.weapon_events().iter().filter(|(t, _)| *t > tick).map(|(_, e)| *e).collect()
}

/// Ziel der Klasse im Gebiet, das am weitesten von anderen Radarstellungen
/// entfernt ist – damit Tests eine Stellung allein betrachten.
pub fn first_of(seed: u64, class: TargetClass) -> Option<(usize, Target)> {
    let probe = Sim::new(SimConfig::new(seed));
    let ts = probe.targets();
    let alone = |i: usize| {
        let t = &ts[i];
        ts.iter()
            .enumerate()
            .filter(|&(j, o)| j != i && o.class.is_radar())
            .map(|(_, o)| (o.north - t.north).hypot(o.east - t.east))
            .fold(f64::MAX, f64::min)
    };
    (0..ts.len())
        .filter(|&i| ts[i].class == class && !ts[i].civilian)
        .max_by(|&a, &b| alone(a).total_cmp(&alone(b)))
        .map(|i| (i, ts[i].clone()))
}

/// Luftstart `height` m über dem ersten Ziel der Klasse, `back` m davor
/// (positiv südlich mit Kurs Nord, negativ nördlich mit Kurs Süd), `side` m
/// quer versetzt, mit 200 m/s und Schub 0,7. Die übrigen Felder kommen aus
/// `rest`. Liefert Simulation und Zielindex.
pub fn approach(
    seed: u64,
    class: TargetClass,
    back: f64,
    side: f64,
    height: f64,
    rest: Spawn,
) -> Option<(Sim, usize)> {
    let (i, t) = first_of(seed, class)?;
    Some(approach_target(seed, i, &t, back, side, height, rest))
}

/// Ziel Nummer `seed` (umlaufend) unter den militärischen der Klasse – für
/// Tests über viele Seeds mit wechselnden Zielen.
pub fn nth_of(seed: u64, class: TargetClass) -> Option<(usize, Target)> {
    let probe = Sim::new(SimConfig::new(seed));
    let all: Vec<_> =
        probe.targets().iter().enumerate().filter(|(_, t)| t.class == class && !t.civilian).collect();
    all.get(seed as usize % all.len().max(1)).map(|(i, t)| (*i, (*t).clone()))
}

/// Wie [`approach`], auf das Ziel `i` (`t`).
pub fn approach_target(
    seed: u64,
    i: usize,
    t: &Target,
    back: f64,
    side: f64,
    height: f64,
    rest: Spawn,
) -> (Sim, usize) {
    let spawn = Spawn {
        north: t.north - back,
        east: t.east + side,
        altitude: t.elevation + height,
        heading: if back >= 0.0 { 0.0 } else { std::f64::consts::PI },
        speed: 200.0,
        throttle: 0.7,
        ..rest
    };
    (Sim::new(SimConfig::with(seed, spawn)), i)
}

/// Alle Flugabwehrstellungen zerstören.
pub fn no_air_defense(sim: &mut Sim) {
    for i in 0..sim.targets().len() {
        if sim.targets()[i].class.is_radar() {
            sim.destroy_target(i);
        }
    }
}
