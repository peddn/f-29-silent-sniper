//! Bordkanone (M-016): eigenes Waffensystem neben den Waffenschächten.
//!
//! Jede Patrone fliegt als einfacher Massenpunkt (Mündungsgeschwindigkeit plus
//! Flugzeuggeschwindigkeit, Schwerkraft, kein Luftwiderstand) mit kleiner,
//! deterministisch gewürfelter Streuung. Ein Treffer richtet bei jedem Ziel
//! denselben Schaden an; kritische Treffer gibt es nicht
//! (Spielerentscheidungen v0.2.8).

use crate::atmosphere::G0;
use crate::detmath;
use crate::math::{DQuat, DVec3};
use crate::rng::Pcg32;
use crate::targets::Target;
use crate::weapons::{ImpactResult, Stores};
use f29_terrain::Terrain;

/// Eigener Zufallsstrom der Kanone (Streuung).
pub const STREAM_GUN: u64 = 0x4755_4E53;
/// Munition beim Start.
pub const ROUNDS: u16 = 600;
/// Feuerrate (Schuss/s): 600 Schuss reichen für 12 s Dauerfeuer.
pub const RATE_HZ: f64 = 50.0;
/// Mündungsgeschwindigkeit (m/s).
pub const MUZZLE_MPS: f64 = 1_000.0;
/// Flugzeit einer Patrone (s); danach zählt sie als verbraucht.
pub const LIFE_S: f64 = 3.0;
/// Schaden je Treffer (Anteil, bei allen Zielen gleich).
pub const DAMAGE_PER_HIT: f64 = 0.02;
/// Streuung (halber Kegelwinkel, rad, 3 mrad).
pub const DISPERSION_RAD: f64 = 0.003;
/// Mündung so weit vor dem Flugzeugzentrum (m).
const MUZZLE_AHEAD_M: f64 = 8.0;
/// Ziele weiter als das vom Flugzeug prüft die Kanone gar nicht erst (m).
const CHECK_RANGE_M: f64 = 4_000.0;
/// Einschläge, die für die Darstellung aufgehoben werden.
const IMPACT_LOG: usize = 128;
/// Ereignisse (Meldungen), die aufgehoben werden.
const EVENT_LOG: usize = 16;
/// Schrittweite der Aufschlagvorhersage (s).
const PREDICT_STEP_S: f64 = 0.05;

/// Eine Patrone im Flug.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Round {
    /// NED (m).
    pub position: DVec3,
    /// NED (m/s).
    pub velocity: DVec3,
    pub age_s: f64,
}

/// Einschlag einer Patrone (für Darstellung).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GunImpact {
    pub tick: u64,
    /// NED (m).
    pub position: DVec3,
    /// Ein Ziel getroffen (sonst Boden oder Wasser).
    pub hit: bool,
}

/// Meldungswürdige Ereignisse der Kanone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GunEvent {
    /// Ziel durch Kanonentreffer zerstört.
    Destroyed(usize),
    /// Abzug bei leerer Kanone.
    Empty,
}

/// Zustand der Kanone.
#[derive(Debug, Clone, PartialEq)]
pub struct Gun {
    pub rounds: u16,
    pub in_flight: Vec<Round>,
    /// Bruchteil des nächsten Schusses.
    pending: f64,
    /// Abzug im letzten Tick gedrückt (für die einmalige Leer-Meldung).
    pub trigger: bool,
    rng: Pcg32,
    pub impacts: Vec<GunImpact>,
    pub events: Vec<(u64, GunEvent)>,
}

impl Gun {
    pub fn new(seed: u64) -> Self {
        Self {
            rounds: ROUNDS,
            in_flight: Vec::new(),
            pending: 0.0,
            trigger: false,
            rng: Pcg32::new(seed, STREAM_GUN),
            impacts: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn rng_state(&self) -> (u64, u64) {
        self.rng.raw_state()
    }

    pub fn pending(&self) -> f64 {
        self.pending
    }

    /// Abzug für einen Tick: `fire` = gedrückt und Schießen erlaubt.
    pub fn trigger(
        &mut self,
        tick: u64,
        fire: bool,
        position: DVec3,
        velocity: DVec3,
        attitude: DQuat,
        dt: f64,
    ) {
        let pressed = fire && !self.trigger;
        self.trigger = fire;
        if !fire {
            self.pending = 0.0;
            return;
        }
        if self.rounds == 0 {
            if pressed {
                self.log(tick, GunEvent::Empty);
            }
            return;
        }
        // Der erste Schuss fällt sofort.
        if pressed {
            self.pending = 1.0;
        } else {
            self.pending += RATE_HZ * dt;
        }
        let bore = attitude.rotate(DVec3::X);
        let right = attitude.rotate(DVec3::Y);
        let down = attitude.rotate(DVec3::Z);
        while self.pending >= 1.0 && self.rounds > 0 {
            self.pending -= 1.0;
            self.rounds -= 1;
            // Gleichverteilt im Streukegel (Fläche): Radius ~ √u.
            let r = DISPERSION_RAD * self.rng.next_f64().sqrt();
            let (s, c) = detmath::sin_cos(self.rng.range_f64(0.0, std::f64::consts::TAU));
            let dir = (bore + right * (r * c) + down * (r * s)).normalize_or_zero();
            self.in_flight.push(Round {
                position: position + bore * MUZZLE_AHEAD_M,
                velocity: velocity + dir * MUZZLE_MPS,
                age_s: 0.0,
            });
        }
    }

    /// Patronen bewegen; Treffer an Zielen und Gelände auswerten.
    pub fn step(&mut self, tick: u64, terrain: &Terrain, targets: &mut [Target], aircraft: DVec3, dt: f64) {
        if self.in_flight.is_empty() {
            return;
        }
        // Nur intakte Ziele in der Nähe prüfen.
        let near: Vec<usize> = (0..targets.len())
            .filter(|&i| {
                let t = &targets[i];
                t.is_intact()
                    && (t.north - aircraft.x).abs() < CHECK_RANGE_M
                    && (t.east - aircraft.y).abs() < CHECK_RANGE_M
            })
            .collect();
        let mut k = 0;
        while k < self.in_flight.len() {
            let r = self.in_flight[k];
            let v = r.velocity + DVec3::new(0.0, 0.0, G0 * dt);
            let p = r.position + v * dt;
            let hit =
                near.iter().copied().find_map(|i| segment_hits(r.position, p, &targets[i]).map(|at| (i, at)));
            let outcome = if let Some((i, at)) = hit {
                let result = Stores::apply_damage(&mut targets[i], i, DAMAGE_PER_HIT);
                if matches!(result, ImpactResult::Destroyed { .. }) {
                    self.log(tick, GunEvent::Destroyed(i));
                }
                Some((at, true))
            } else {
                let ground = terrain.elevation(p.x, p.y);
                if -p.z <= ground {
                    // Zwischen beiden Punkten auf die Oberfläche legen.
                    let h0 = -r.position.z - terrain.elevation(r.position.x, r.position.y);
                    let h1 = -p.z - ground;
                    let f = if h0 > h1 { (h0 / (h0 - h1)).clamp(0.0, 1.0) } else { 1.0 };
                    let mut at = r.position + (p - r.position) * f;
                    at.z = -terrain.elevation(at.x, at.y);
                    Some((at, false))
                } else {
                    None
                }
            };
            if let Some((at, hit)) = outcome {
                if self.impacts.len() >= IMPACT_LOG {
                    self.impacts.remove(0);
                }
                self.impacts.push(GunImpact { tick, position: at, hit });
                self.in_flight.swap_remove(k);
                continue;
            }
            let age_s = r.age_s + dt;
            if age_s >= LIFE_S {
                self.in_flight.swap_remove(k);
                continue;
            }
            self.in_flight[k] = Round { position: p, velocity: v, age_s };
            k += 1;
        }
    }

    fn log(&mut self, tick: u64, e: GunEvent) {
        if self.events.len() >= EVENT_LOG {
            self.events.remove(0);
        }
        self.events.push((tick, e));
    }
}

/// Trifft die Strecke `a`→`b` die Form des Ziels (senkrechter Zylinder mit
/// Klassenradius und -höhe)? Liefert den Treffpunkt.
fn segment_hits(a: DVec3, b: DVec3, t: &Target) -> Option<DVec3> {
    let r = t.class.radius();
    let d = b - a;
    // Nächster Punkt der Strecke zur Zylinderachse (waagerecht).
    let (ax, ay) = (a.x - t.north, a.y - t.east);
    let dd = d.x * d.x + d.y * d.y;
    let s = if dd > 1e-9 { (-(ax * d.x + ay * d.y) / dd).clamp(0.0, 1.0) } else { 0.0 };
    let (px, py) = (ax + d.x * s, ay + d.y * s);
    if px * px + py * py > r * r {
        return None;
    }
    let p = a + d * s;
    let h = -p.z - t.elevation;
    (-1.0..=t.class.height()).contains(&h).then_some(p)
}

/// Wo eine jetzt abgefeuerte Patrone (ohne Streuung) auf das Gelände träfe,
/// und nach welcher Flugzeit – `None`, wenn sie vorher verbraucht ist.
pub fn predict_impact(
    position: DVec3,
    velocity: DVec3,
    attitude: DQuat,
    terrain: &Terrain,
) -> Option<(DVec3, f64)> {
    let bore = attitude.rotate(DVec3::X);
    let mut p = position + bore * MUZZLE_AHEAD_M;
    let mut v = velocity + bore * MUZZLE_MPS;
    let ceiling = terrain.max_elevation();
    let mut t = 0.0;
    while t < LIFE_S {
        let v1 = v + DVec3::new(0.0, 0.0, G0 * PREDICT_STEP_S);
        let p1 = p + (v + v1) * (0.5 * PREDICT_STEP_S);
        if -p1.z <= ceiling {
            let ground = terrain.elevation(p1.x, p1.y);
            if -p1.z <= ground {
                let h0 = -p.z - terrain.elevation(p.x, p.y);
                let h1 = -p1.z - ground;
                let f = if h0 > h1 { (h0 / (h0 - h1)).clamp(0.0, 1.0) } else { 1.0 };
                let mut at = p + (p1 - p) * f;
                at.z = -terrain.elevation(at.x, at.y);
                return Some((at, t + PREDICT_STEP_S * f));
            }
        }
        p = p1;
        v = v1;
        t += PREDICT_STEP_S;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::targets::{TargetClass, TargetRole, TargetState};

    const DT: f64 = 1.0 / 120.0;

    fn target(class: TargetClass, north: f64, elevation: f64) -> Target {
        Target {
            name: "TGT",
            class,
            civilian: false,
            role: TargetRole::None,
            north,
            east: 0.0,
            elevation,
            heading: 0.0,
            state: TargetState::Intact,
            damage: 0.0,
        }
    }

    /// Flugzeug `dist` m vor dem Ziel und `height` m darüber, Nase genau auf
    /// die Zielmitte.
    fn aimed(t: &Target, dist: f64, height: f64) -> (DVec3, DVec3, DQuat) {
        let mid = t.elevation + t.class.height() * 0.5;
        let pos = DVec3::new(t.north - dist, 0.0, -(mid + height));
        let pitch = -detmath::atan2(height, dist);
        let att = DQuat::from_euler(0.0, pitch, 0.0);
        (pos, att.rotate(DVec3::new(200.0, 0.0, 0.0)), att)
    }

    #[test]
    fn rate_of_fire_and_empty() {
        let mut gun = Gun::new(1);
        let (p, v, a) = (DVec3::new(0.0, 0.0, -3_000.0), DVec3::new(200.0, 0.0, 0.0), DQuat::IDENTITY);
        for tick in 0..120 {
            gun.trigger(tick, true, p, v, a, DT);
        }
        let fired = ROUNDS - gun.rounds;
        assert!((49..=51).contains(&fired), "{fired} Schuss in 1 s");
        // Kurzer Druck: genau ein Schuss sofort.
        gun.trigger(200, false, p, v, a, DT);
        let before = gun.rounds;
        gun.trigger(201, true, p, v, a, DT);
        assert_eq!(gun.rounds, before - 1);
        // Leer schießen; ein neuer Druck meldet „leer“.
        for tick in 300..(300 + 120 * 13) {
            gun.trigger(tick, true, p, v, a, DT);
        }
        assert_eq!(gun.rounds, 0);
        assert!(gun.events.iter().all(|e| e.1 != GunEvent::Empty), "nur beim neuen Druck");
        gun.trigger(5_000, false, p, v, a, DT);
        gun.trigger(5_001, true, p, v, a, DT);
        assert_eq!(gun.events.last().map(|e| e.1), Some(GunEvent::Empty));
    }

    /// Jedes Ziel nimmt 2 % je Treffer: nach 50 Treffern zerstört, auch das
    /// gehärtete.
    #[test]
    fn strafing_destroys_any_class_after_fifty_hits() {
        let terrain = Terrain::new(1);
        for class in [TargetClass::Soft, TargetClass::Hardened] {
            let elevation = terrain.elevation(5_000.0, 0.0);
            let mut targets = vec![target(class, 5_000.0, elevation)];
            let (p, v, a) = aimed(&targets[0], 800.0, 150.0);
            let mut gun = Gun::new(7);
            let mut tick = 0;
            let mut hits = 0;
            while targets[0].is_intact() && tick < 120 * 20 {
                tick += 1;
                gun.trigger(tick, gun.rounds > 0, p, v, a, DT);
                gun.step(tick, &terrain, &mut targets, p, DT);
                hits = gun.impacts.iter().filter(|i| i.hit).count().max(hits);
            }
            assert!(
                !targets[0].is_intact(),
                "{class:?} nicht zerstört ({hits} Treffer, {} Schuss)",
                ROUNDS - gun.rounds
            );
            assert!(gun.events.iter().any(|e| e.1 == GunEvent::Destroyed(0)));
            // Mindestens 50 Treffer waren nötig.
            assert!(ROUNDS - gun.rounds >= 50);
        }
    }

    #[test]
    fn impact_prediction_matches_the_rounds() {
        let terrain = Terrain::new(1);
        let elevation = terrain.elevation(5_000.0, 0.0);
        let t = target(TargetClass::Soft, 5_000.0, elevation);
        let (p, v, a) = aimed(&t, 1_200.0, 400.0);
        let (impact, tof) = predict_impact(p, v, a, &terrain).expect("Boden in Reichweite");
        assert!(tof > 0.5 && tof < LIFE_S, "{tof}");
        // Eine Patrone ohne Streuung landet dort.
        let mut gun = Gun::new(3);
        gun.in_flight.push(Round {
            position: p + a.rotate(DVec3::X) * MUZZLE_AHEAD_M,
            velocity: v + a.rotate(DVec3::X) * MUZZLE_MPS,
            age_s: 0.0,
        });
        let mut none: Vec<Target> = Vec::new();
        for tick in 0..(120 * 3) {
            gun.step(tick, &terrain, &mut none, p, DT);
        }
        let hit = gun.impacts.first().expect("Einschlag").position;
        assert!((hit - impact).length() < 15.0, "Abweichung {:.1} m", (hit - impact).length());
        // Waagerecht in großer Höhe: kein Aufschlag in Reichweite.
        let high = DVec3::new(0.0, 0.0, -(terrain.max_elevation() + 3_000.0));
        assert!(predict_impact(high, DVec3::new(200.0, 0.0, 0.0), DQuat::IDENTITY, &terrain).is_none());
    }
}
