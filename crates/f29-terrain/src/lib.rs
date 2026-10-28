//! Deterministisches, prozedurales Terrain.
//!
//! Das Terrain ist eine reine Funktion `(seed, x, y) -> Höhe`. Simulation
//! (Kollision, Radarhöhe, später Sichtlinien) und Darstellung (Meshes)
//! werten exakt dieselbe Funktion aus, daher gibt es keine Abweichung
//! zwischen dem, was man sieht, und dem, womit man kollidiert.
//!
//! Koordinaten: `x` = Nord, `y` = Ost in Metern (wie das NED-System im
//! Simulationskern); Höhen in Metern über dem Meeresspiegel.

pub mod airfield;
pub mod noise;

pub use airfield::{AirfieldKind, Runway};

use noise::{fbm, mix64, ridged, smoothstep};

/// Höhe des Meeresspiegels in Metern.
pub const SEA_LEVEL_M: f64 = 0.0;

/// Oberflächenklasse für Darstellung und (später) Bodeneffekte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface {
    Water,
    Beach,
    Grass,
    Forest,
    Rock,
    Snow,
}

/// Parameter der Höhenfunktion. Alle Längen in Metern.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainParams {
    /// Wellenlänge der Kontinent-/Tieflandstruktur.
    pub continent_scale: f64,
    /// Höhenamplitude der Kontinentstruktur.
    pub continent_amplitude: f64,
    /// Grundhöhe (verschiebt Land-/Wasser-Anteil).
    pub base_height: f64,
    /// Wellenlänge der Gebirgsmaske.
    pub mountain_mask_scale: f64,
    /// Wellenlänge der Gebirgsgrate.
    pub ridge_scale: f64,
    /// Maximale Gebirgshöhe über Grundniveau.
    pub mountain_height: f64,
    /// Wellenlänge der Hügel.
    pub hill_scale: f64,
    /// Amplitude der Hügel.
    pub hill_amplitude: f64,
}

impl Default for TerrainParams {
    fn default() -> Self {
        Self {
            continent_scale: 45_000.0,
            continent_amplitude: 1_100.0,
            base_height: 120.0,
            mountain_mask_scale: 22_000.0,
            ridge_scale: 9_000.0,
            mountain_height: 2_600.0,
            hill_scale: 2_200.0,
            hill_amplitude: 90.0,
        }
    }
}

/// Ein Terrain, vollständig bestimmt durch Seed und Parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Terrain {
    seed: u64,
    params: TerrainParams,
    s_continent: u64,
    s_mask: u64,
    s_ridge: u64,
    s_hill: u64,
    s_forest: u64,
    runways: Vec<Runway>,
}

impl Terrain {
    /// Terrain mit Standardparametern.
    pub fn new(seed: u64) -> Self {
        Self::with_params(seed, TerrainParams::default())
    }

    /// Terrain mit eigenen Parametern.
    pub fn with_params(seed: u64, params: TerrainParams) -> Self {
        // Jede Schicht bekommt einen eigenen, aus dem Seed abgeleiteten Strom.
        let sub = |k: u64| mix64(seed ^ mix64(k));
        let mut t = Self {
            seed,
            params,
            s_continent: sub(1),
            s_mask: sub(2),
            s_ridge: sub(3),
            s_hill: sub(4),
            s_forest: sub(5),
            runways: Vec::new(),
        };
        t.runways = airfield::place_airfields(seed, |x, y| t.raw_height(x, y));
        t
    }

    /// Alle Start- und Landebahnen; die erste ist die Heimatbasis.
    pub fn runways(&self) -> &[Runway] {
        &self.runways
    }

    /// Index der Bahn, auf deren befestigter Fläche der Punkt liegt.
    pub fn runway_at(&self, x: f64, y: f64) -> Option<usize> {
        self.runways.iter().position(|r| r.contains(x, y))
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn params(&self) -> &TerrainParams {
        &self.params
    }

    /// Bodenhöhe inkl. Meeresgrund (kann negativ sein), mit eingeebneten
    /// Flugplätzen.
    pub fn ground_height(&self, x: f64, y: f64) -> f64 {
        airfield::flatten(&self.runways, x, y, self.raw_height(x, y))
    }

    /// Geländehöhe ohne Flugplätze.
    fn raw_height(&self, x: f64, y: f64) -> f64 {
        let p = &self.params;
        let cs = p.continent_scale;
        let continent = fbm(self.s_continent, x / cs, y / cs, 5);
        let base = continent * p.continent_amplitude + p.base_height;

        let ms = p.mountain_mask_scale;
        let mask = smoothstep(0.0, 0.30, fbm(self.s_mask, x / ms, y / ms, 3));
        let rs = p.ridge_scale;
        let r = ridged(self.s_ridge, x / rs, y / rs, 5);
        let mountains = mask * r * r * p.mountain_height;

        let hs = p.hill_scale;
        let hills = fbm(self.s_hill, x / hs, y / hs, 4) * p.hill_amplitude * (0.35 + 0.65 * mask);

        base + mountains + hills
    }

    /// Kollisionshöhe: Boden oder Wasseroberfläche, je nachdem was höher ist.
    pub fn elevation(&self, x: f64, y: f64) -> f64 {
        self.ground_height(x, y).max(SEA_LEVEL_M)
    }

    /// Oberflächennormale der Kollisionsfläche als `[x, y, up]`, normiert.
    pub fn normal(&self, x: f64, y: f64) -> [f64; 3] {
        const EPS: f64 = 8.0;
        let dhdx = (self.elevation(x + EPS, y) - self.elevation(x - EPS, y)) / (2.0 * EPS);
        let dhdy = (self.elevation(x, y + EPS) - self.elevation(x, y - EPS)) / (2.0 * EPS);
        let len = (dhdx * dhdx + dhdy * dhdy + 1.0).sqrt();
        [-dhdx / len, -dhdy / len, 1.0 / len]
    }

    /// Oberflächenklasse an einem Punkt.
    pub fn surface(&self, x: f64, y: f64) -> Surface {
        let h = self.ground_height(x, y);
        if h < SEA_LEVEL_M {
            return Surface::Water;
        }
        if h < 6.0 {
            return Surface::Beach;
        }
        if self.runways.iter().any(|r| r.in_flat_area(x, y)) {
            return Surface::Grass;
        }
        let steepness = 1.0 - self.normal(x, y)[2];
        if steepness > 0.30 {
            return Surface::Rock;
        }
        if h > 1_900.0 {
            return Surface::Snow;
        }
        if h > 1_350.0 {
            return Surface::Rock;
        }
        let forest = fbm(self.s_forest, x / 7_000.0, y / 7_000.0, 4);
        if forest > 0.18 && h < 1_100.0 { Surface::Forest } else { Surface::Grass }
    }

    /// Tastet ein Gitter aus `(n+1)²` Punkten ab, beginnend bei Gitterindex
    /// `(ix0, iy0)` mit Abstand `spacing`.
    ///
    /// Koordinaten werden aus *globalen* Gitterindizes berechnet. Dadurch sind
    /// Randpunkte benachbarter Kacheln bitgleich und es entstehen keine Risse.
    /// Ergebnis ist zeilenweise: Index `j * (n + 1) + i` mit `i` entlang x.
    pub fn sample_grid(&self, ix0: i64, iy0: i64, n: usize, spacing: f64) -> Vec<f64> {
        let mut out = Vec::with_capacity((n + 1) * (n + 1));
        for j in 0..=n as i64 {
            for i in 0..=n as i64 {
                let x = (ix0 + i) as f64 * spacing;
                let y = (iy0 + j) as f64 * spacing;
                out.push(self.elevation(x, y));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_terrain() {
        let a = Terrain::new(42);
        let b = Terrain::new(42);
        for i in 0..500 {
            let x = f64::from(i) * 977.3 - 120_000.0;
            let y = f64::from(i) * -611.9 + 40_000.0;
            assert_eq!(a.ground_height(x, y).to_bits(), b.ground_height(x, y).to_bits());
        }
    }

    #[test]
    fn different_seed_different_terrain() {
        let a = Terrain::new(1);
        let b = Terrain::new(2);
        let differs = (0..50).any(|i| {
            let x = f64::from(i) * 1234.5;
            a.ground_height(x, 0.0) != b.ground_height(x, 0.0)
        });
        assert!(differs);
    }

    #[test]
    fn heights_are_plausible() {
        let t = Terrain::new(7);
        let mut min = f64::MAX;
        let mut max = f64::MIN;
        let mut water = 0;
        let mut land = 0;
        for i in -100..100 {
            for j in -100..100 {
                let h = t.ground_height(f64::from(i) * 1000.0, f64::from(j) * 1000.0);
                assert!(h.is_finite());
                min = min.min(h);
                max = max.max(h);
                if h < SEA_LEVEL_M {
                    water += 1;
                } else {
                    land += 1;
                }
            }
        }
        assert!(min > -1_500.0 && max < 4_000.0, "min {min} max {max}");
        assert!(max > 800.0, "keine Berge: max {max}");
        assert!(land > water, "zu viel Wasser: {water} vs {land}");
    }

    #[test]
    fn chunk_edges_match_exactly() {
        let t = Terrain::new(3);
        let n = 16;
        let spacing = 64.0;
        let a = t.sample_grid(0, 0, n, spacing);
        let b = t.sample_grid(n as i64, 0, n, spacing);
        let c = t.sample_grid(0, n as i64, n, spacing);
        for k in 0..=n {
            // rechter Rand von a == linker Rand von b
            assert_eq!(a[k * (n + 1) + n].to_bits(), b[k * (n + 1)].to_bits());
            // oberer Rand von a == unterer Rand von c
            assert_eq!(a[n * (n + 1) + k].to_bits(), c[k].to_bits());
        }
    }

    #[test]
    fn terrain_is_continuous() {
        // Benachbarte Punkte im Abstand 1 m unterscheiden sich nicht sprunghaft.
        let t = Terrain::new(11);
        for i in 0..2_000 {
            let x = f64::from(i) * 37.1 - 30_000.0;
            let y = f64::from(i) * 13.7;
            let d = (t.ground_height(x + 1.0, y) - t.ground_height(x, y)).abs();
            assert!(d < 5.0, "Sprung von {d} m bei ({x}, {y})");
        }
    }

    #[test]
    fn normal_is_unit_and_points_up() {
        let t = Terrain::new(5);
        for i in 0..200 {
            let n = t.normal(f64::from(i) * 311.0, f64::from(i) * -97.0);
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-12);
            assert!(n[2] > 0.0);
        }
    }

    #[test]
    fn airfields_exist_and_are_deterministic() {
        for seed in [0x0F29_u64, 1, 7, 1234, 99_999] {
            let t = Terrain::new(seed);
            let r = t.runways();
            assert!(r.len() >= 2, "Seed {seed}: nur {} Bahnen", r.len());
            assert_eq!(r[0].kind, AirfieldKind::Home);
            assert!(r[1..].iter().all(|x| x.kind == AirfieldKind::Secret));
            assert_eq!(Terrain::new(seed).runways(), r);
        }
    }

    #[test]
    fn runways_are_flat_on_land_and_blend_smoothly() {
        let t = Terrain::new(0x0F29);
        for rw in t.runways() {
            assert!(rw.elevation_m > 10.0);
            for i in 0..=20 {
                // knapp innerhalb der Kanten (Rundung bei der Rücktransformation)
                let u = (-0.5 + f64::from(i) / 20.0) * rw.length_m * 0.999;
                for v in [-rw.width_m * 0.499, 0.0, rw.width_m * 0.499] {
                    let p = rw.world(u, v);
                    assert_eq!(t.elevation(p[0], p[1]), rw.elevation_m, "{} nicht eben", rw.name);
                    assert!(t.runway_at(p[0], p[1]).is_some());
                }
            }
            // Übergang ohne Sprung
            for i in 0..400 {
                let p = rw.world(0.0, rw.width_m * 0.5 + f64::from(i) * 2.5);
                let q = rw.world(0.0, rw.width_m * 0.5 + f64::from(i) * 2.5 + 1.0);
                let d = (t.elevation(p[0], p[1]) - t.elevation(q[0], q[1])).abs();
                assert!(d < 2.0, "Sprung {d} m neben {}", rw.name);
            }
        }
    }

    #[test]
    fn runway_numbers_match_heading() {
        let t = Terrain::new(3);
        for rw in t.runways() {
            let (a, b) = rw.numbers();
            assert!((1..=36).contains(&a) && (1..=36).contains(&b));
            assert_eq!((a + 18 - 1) % 36 + 1, b);
        }
    }

    #[test]
    fn water_has_flat_elevation() {
        let t = Terrain::new(9);
        for i in -60..60 {
            for j in -60..60 {
                let (x, y) = (f64::from(i) * 2_000.0, f64::from(j) * 2_000.0);
                if t.surface(x, y) == Surface::Water {
                    assert_eq!(t.elevation(x, y), SEA_LEVEL_M);
                }
            }
        }
    }
}
