//! Erstbefüllung eines Einsatzgebiets aus einem Seed – ein Werkzeug für das
//! Anlegen von Gebieten (später „automatisch bevölkern“ im Editor), nicht
//! Teil des Spiels. Das Ergebnis wird als Datei gespeichert; das Spiel lädt
//! nur die Datei.
//!
//! Ablauf: Rauschgelände ins Grobraster abtasten (Nordrand Meer: eine
//! Küste), Flugplätze auf dem daraus erzeugten Gelände suchen (eigene Basis
//! im Südwesten, feindliche Horste im Nordosten), Seiten nach der Nähe zu
//! eigenen bzw. feindlichen Plätzen, dann Objekte verteilen: Stellungen um
//! die Horste, Frühwarnradare an der Grenze, militärische Anlagen im
//! feindlichen Gebiet, Schiffe auf See, zivile Gebäude und Frachter.
//!
//! Winkel und Zufall hier stammen aus `std` bzw. einem einfachen Generator:
//! Bitgleichheit ist nicht nötig, denn das Spiel liest nur die Datei.

use crate::airfield::find_site;
use crate::class::TargetClass;
use crate::noise::{fbm, mix64};
use crate::theater::{AirfieldDef, Grid, ObjectDef, Side, Theater};
use crate::{AirfieldKind, Ground, Surface, Terrain};

/// Mindestabstand zwischen zwei Objekten (m).
pub const MIN_SPACING_M: f64 = 120.0;
/// Feindliche Radare mindestens so weit von eigenen Basen (m).
pub const HOME_RADAR_CLEARANCE_M: f64 = 35_000.0;
/// Mindestabstand zu Flugplätzen (m).
const RUNWAY_CLEARANCE_M: f64 = 2_500.0;
/// Abstand der Anker, um die Anlagen verteilt werden (m).
const ANCHOR_M: f64 = 22_000.0;

/// Einfacher Zufallsgenerator (SplitMix64).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix64(self.0)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1_u64 << 53) as f64
    }

    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.unit()
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.next() as usize % items.len()]
    }
}

/// Legt ein Gebiet `name` mit Kantenlänge `size_m` und `n × n` Raster an.
pub fn generate(name: &str, seed: u64, size_m: f64, n: usize) -> Theater {
    let mut theater = Theater {
        name: name.into(),
        seed,
        size_m,
        grid: Grid { ground: sample_ground(seed, size_m, n), side: vec!["N".repeat(n); n] },
        airfields: Vec::new(),
        objects: Vec::new(),
    };
    let terrain = Terrain::from_theater(&theater);
    let q = size_m * 0.25;
    let plan = [
        ("HOME", AirfieldKind::Home, [-q * 1.2, -q * 1.0]),
        ("RED 1", AirfieldKind::Enemy, [q * 0.4, q * 0.8]),
        ("RED 2", AirfieldKind::Enemy, [q * 0.9, q * 0.1]),
        ("RED 3", AirfieldKind::Enemy, [-q * 0.1, q * 1.5]),
    ];
    for (name, kind, around) in plan {
        if let Some((c, heading, _)) = find_site(|x, y| terrain.raw_height(x, y), seed, kind, around, q * 0.4)
        {
            let enemy = kind == AirfieldKind::Enemy;
            theater.airfields.push(AirfieldDef {
                name: name.into(),
                kind,
                north: c[0].round(),
                east: c[1].round(),
                heading_deg: heading,
                mig21: if enemy { 3 } else { 0 },
                mig29: if enemy { 2 } else { 0 },
            });
        }
    }
    theater.grid.side = sides(&theater);
    let terrain = Terrain::from_theater(&theater);
    theater.objects = Placer::new(&theater, &terrain, seed).run();
    theater
}

/// Geländeart je Zelle aus dem reinen Rauschgelände; der Nordrand wird Meer.
fn sample_ground(seed: u64, size_m: f64, n: usize) -> Vec<String> {
    let noise = Terrain::new(seed);
    let cell = size_m / n as f64;
    (0..n)
        .map(|r| {
            (0..n)
                .map(|c| {
                    let x = size_m * 0.5 - (r as f64 + 0.5) * cell;
                    let y = -size_m * 0.5 + (c as f64 + 0.5) * cell;
                    let coast = (0.18 + 0.06 * fbm(seed ^ 0xC0A5, y / 60_000.0, 0.0, 3)) * n as f64;
                    let hs: Vec<f64> = [(-0.3, -0.3), (-0.3, 0.3), (0.3, -0.3), (0.3, 0.3), (0.0, 0.0)]
                        .iter()
                        .map(|(a, b)| noise.ground_height(x + a * cell, y + b * cell))
                        .collect();
                    let mean = hs.iter().sum::<f64>() / hs.len() as f64;
                    let max = hs.iter().copied().fold(f64::MIN, f64::max);
                    let g = if (r as f64) < coast || mean < -20.0 {
                        Ground::Water
                    } else if max > 1_500.0 || mean > 900.0 {
                        Ground::Mountains
                    } else if mean > 350.0 {
                        Ground::Hills
                    } else {
                        Ground::Plain
                    };
                    g.to_char()
                })
                .collect()
        })
        .collect()
}

/// Seiten nach der Nähe: nah an eigenen Basen eigen, nah an feindlichen
/// Horsten feindlich, dazwischen ein neutraler Streifen.
fn sides(t: &Theater) -> Vec<String> {
    let n = t.grid_n();
    let cell = t.size_m / n as f64;
    let nearest = |kind: AirfieldKind, x: f64, y: f64| {
        t.airfields
            .iter()
            .filter(|a| a.kind == kind)
            .map(|a| (a.north - x).hypot(a.east - y))
            .fold(f64::MAX, f64::min)
    };
    (0..n)
        .map(|r| {
            (0..n)
                .map(|c| {
                    let x = t.size_m * 0.5 - (r as f64 + 0.5) * cell;
                    let y = -t.size_m * 0.5 + (c as f64 + 0.5) * cell;
                    let (own, foe) = (nearest(AirfieldKind::Home, x, y), nearest(AirfieldKind::Enemy, x, y));
                    let share = own / (own + foe);
                    let side = if share < 0.43 {
                        Side::Friendly
                    } else if share < 0.53 {
                        Side::Neutral
                    } else {
                        Side::Hostile
                    };
                    side.to_char()
                })
                .collect()
        })
        .collect()
}

struct Placer<'a> {
    theater: &'a Theater,
    terrain: &'a Terrain,
    rng: Rng,
    objects: Vec<ObjectDef>,
}

impl<'a> Placer<'a> {
    fn new(theater: &'a Theater, terrain: &'a Terrain, seed: u64) -> Self {
        Self { theater, terrain, rng: Rng(mix64(seed ^ 0x5047_4c43)), objects: Vec::new() }
    }

    fn run(mut self) -> Vec<ObjectDef> {
        let half = self.theater.size_m * 0.5;
        // Stellungen und Anlagen um die feindlichen Horste.
        for a in self.theater.airfields.iter().filter(|a| a.kind == AirfieldKind::Enemy) {
            for (class, r) in [
                (TargetClass::AirDefense, 6_000.0),
                (TargetClass::AirDefense, 9_000.0),
                (TargetClass::ShortRangeSam, 3_500.0),
                (TargetClass::Hardened, 3_000.0),
                (TargetClass::ParkedAircraft, 3_000.0),
            ] {
                self.site_near(class, [a.north, a.east], r * 0.6, r, &[TargetClass::Soft]);
            }
        }
        // Anker im Gitter über das ganze Gebiet.
        let steps = (self.theater.size_m / ANCHOR_M) as i32;
        let mut ew: Vec<[f64; 2]> = Vec::new();
        for i in 0..steps {
            for j in 0..steps {
                let p = [
                    half - (f64::from(i) + 0.5) * ANCHOR_M + self.rng.range(-0.3, 0.3) * ANCHOR_M,
                    -half + (f64::from(j) + 0.5) * ANCHOR_M + self.rng.range(-0.3, 0.3) * ANCHOR_M,
                ];
                let side = self.theater.side_at(p[0], p[1]);
                let water = self.terrain.surface(p[0], p[1]) == Surface::Water;
                match (side, water) {
                    (Side::Hostile, false) => {
                        // Frühwarnradar an der Grenze, mindestens 60 km auseinander.
                        let border =
                            [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)].iter().any(|(a, b)| {
                                self.theater.side_at(p[0] + a * ANCHOR_M, p[1] + b * ANCHOR_M)
                                    != Side::Hostile
                            });
                        if border && ew.iter().all(|q| (q[0] - p[0]).hypot(q[1] - p[1]) > 60_000.0) {
                            if self.site_near(
                                TargetClass::EarlyWarning,
                                p,
                                0.0,
                                6_000.0,
                                &[TargetClass::Soft],
                            ) {
                                ew.push(p);
                            }
                            continue;
                        }
                        self.hostile_site(p);
                    }
                    (Side::Hostile, true) if self.rng.unit() < 0.35 => {
                        let class = self.rng.pick(&[TargetClass::ShipSmall, TargetClass::ShipLarge]);
                        self.site_near(class, p, 0.0, 8_000.0, &[TargetClass::ShipSmall]);
                    }
                    (Side::Neutral, true) if self.rng.unit() < 0.25 => {
                        self.civilian_near(TargetClass::ShipLarge, p);
                    }
                    (Side::Neutral, false) if self.rng.unit() < 0.3 => {
                        self.civilian_near(TargetClass::Building, p);
                    }
                    _ => {}
                }
            }
        }
        self.objects
    }

    /// Eine militärische Anlage im feindlichen Gebiet.
    fn hostile_site(&mut self, p: [f64; 2]) {
        let roll = self.rng.unit();
        let (class, escorts): (TargetClass, &[TargetClass]) = if roll < 0.22 {
            (TargetClass::AirDefense, &[TargetClass::Soft])
        } else if roll < 0.3 {
            (TargetClass::ShortRangeSam, &[TargetClass::Armored])
        } else if roll < 0.45 {
            (TargetClass::Building, &[TargetClass::Soft, TargetClass::ShortRangeSam])
        } else if roll < 0.55 {
            (TargetClass::Hardened, &[TargetClass::Soft])
        } else if roll < 0.62 {
            (TargetClass::Bridge, &[])
        } else if roll < 0.75 {
            (TargetClass::Armored, &[TargetClass::Armored, TargetClass::Soft])
        } else if roll < 0.85 {
            (TargetClass::Soft, &[TargetClass::Soft, TargetClass::Soft])
        } else {
            return self.civilian_near(TargetClass::Building, p);
        };
        self.site_near(class, p, 0.0, 6_000.0, escorts);
    }

    fn civilian_near(&mut self, class: TargetClass, p: [f64; 2]) {
        if let Some(q) = self.spot(class, p, 0.0, 6_000.0) {
            let name = self.rng.pick(class.names().1);
            self.push(class, q, name, true);
        }
    }

    /// Hauptobjekt nahe `p` plus Begleiter dicht daneben.
    fn site_near(
        &mut self,
        class: TargetClass,
        p: [f64; 2],
        r_min: f64,
        r_max: f64,
        escorts: &[TargetClass],
    ) -> bool {
        let Some(q) = self.spot(class, p, r_min, r_max) else {
            return false;
        };
        let name = self.rng.pick(class.names().0);
        self.push(class, q, name, false);
        for &e in escorts {
            if let Some(s) = self.spot(e, q, 150.0, 600.0) {
                let name = self.rng.pick(e.names().0);
                self.push(e, s, name, false);
            }
        }
        true
    }

    fn push(&mut self, class: TargetClass, p: [f64; 2], name: &str, civilian: bool) {
        let heading_deg = (self.rng.range(0.0, 360.0) * 10.0).round() / 10.0;
        self.objects.push(ObjectDef {
            name: name.into(),
            class,
            north: p[0].round(),
            east: p[1].round(),
            heading_deg,
            civilian,
        });
    }

    /// Ein freier, geeigneter Ort für `class` im Ring um `p`.
    fn spot(&mut self, class: TargetClass, p: [f64; 2], r_min: f64, r_max: f64) -> Option<[f64; 2]> {
        for _ in 0..60 {
            let (s, c) = self.rng.range(0.0, std::f64::consts::TAU).sin_cos();
            let d = self.rng.range(r_min, r_max);
            let q = [(p[0] + c * d).round(), (p[1] + s * d).round()];
            if self.suitable(class, q) && self.free(q, MIN_SPACING_M + class.radius()) {
                return Some(q);
            }
        }
        None
    }

    fn free(&self, p: [f64; 2], spacing: f64) -> bool {
        self.objects.iter().all(|o| (o.north - p[0]).hypot(o.east - p[1]) > spacing)
            && self
                .terrain
                .runways()
                .iter()
                .all(|r| (r.center[0] - p[0]).hypot(r.center[1] - p[1]) > RUNWAY_CLEARANCE_M)
    }

    /// Schiffe auf Wasser mit Abstand zum Ufer, alles andere auf flachem
    /// Land; feindliche Radare nicht nah an eigenen Basen.
    fn suitable(&self, class: TargetClass, p: [f64; 2]) -> bool {
        let half = self.theater.size_m * 0.5 - 2_000.0;
        if p[0].abs() > half || p[1].abs() > half {
            return false;
        }
        let surface = self.terrain.surface(p[0], p[1]);
        if class.is_naval() {
            let r = class.radius();
            return surface == Surface::Water
                && [(r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)]
                    .iter()
                    .all(|(a, b)| self.terrain.ground_height(p[0] + a, p[1] + b) < -2.0);
        }
        let ok = matches!(surface, Surface::Grass | Surface::Forest | Surface::Beach);
        let flat = 1.0 - self.terrain.normal(p[0], p[1])[2] < 0.12;
        let away = !class.is_radar()
            || self
                .terrain
                .runways()
                .iter()
                .filter(|r| r.kind == AirfieldKind::Home)
                .all(|r| (r.center[0] - p[0]).hypot(r.center[1] - p[1]) > HOME_RADAR_CLEARANCE_M);
        ok && flat && away
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kleines Gebiet: deterministisch, Plätze gefunden, Schiffe nur auf
    /// Wasser, Abstände eingehalten, Datei wieder lesbar.
    #[test]
    fn generated_theater_is_plausible() {
        let t = generate("TEST", 7, 200_000.0, 32);
        assert_eq!(t, generate("TEST", 7, 200_000.0, 32));
        assert_eq!(Theater::parse(&t.to_text()).as_ref(), Ok(&t));
        assert!(t.airfields.iter().any(|a| a.kind == AirfieldKind::Home));
        assert!(t.airfields.iter().any(|a| a.kind == AirfieldKind::Enemy));
        assert!(t.objects.len() > 20, "nur {} Objekte", t.objects.len());
        let terrain = Terrain::from_theater(&t);
        for o in &t.objects {
            let water = terrain.surface(o.north, o.east) == Surface::Water;
            assert_eq!(water, o.class.is_naval(), "{} am falschen Ort", o.name);
        }
        for (i, a) in t.objects.iter().enumerate() {
            for b in &t.objects[i + 1..] {
                assert!(
                    (a.north - b.north).hypot(a.east - b.east) > MIN_SPACING_M,
                    "{} / {}",
                    a.name,
                    b.name
                );
            }
        }
    }
}
