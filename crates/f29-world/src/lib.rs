//! Einsatzgebiete (M-031): Datenformat ([`theater`]), Objektklassen
//! ([`class`]), Flugplätze und das daraus erzeugte Gelände.
//!
//! Das Terrain ist eine reine Funktion `(Gebiet, x, y) -> Höhe`: Das
//! Grobraster des Gebiets gibt die Großform vor (Meer, Ebene, Hügel,
//! Gebirge), deterministisches Rauschen die Feinheiten. Ohne Gebiet
//! ([`Terrain::new`]) ist es reines Rauschen. Simulation
//! (Kollision, Radarhöhe, Sichtlinien) und Darstellung (Meshes)
//! werten exakt dieselbe Funktion aus, daher gibt es keine Abweichung
//! zwischen dem, was man sieht, und dem, womit man kollidiert.
//!
//! Koordinaten: `x` = Nord, `y` = Ost in Metern (wie das NED-System im
//! Simulationskern); Höhen in Metern über dem Meeresspiegel.

pub mod airfield;
pub mod class;
pub mod noise;
pub mod theater;

pub use airfield::{AirfieldKind, Runway};
pub use class::{ShapeBox, TargetClass};
pub use theater::{Ground, Side, Theater};

use std::sync::Arc;
use theater::{CELL_M, GRID_N, SIZE_M};

use noise::{fbm, mix64, ridged, smoothstep};

/// Höhe des Meeresspiegels in Metern.
pub const SEA_LEVEL_M: f64 = 0.0;

/// Oberflächenklasse für Darstellung und Zielplatzierung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface {
    Water,
    Beach,
    Grass,
    Forest,
    Rock,
    Snow,
    Sand,
    City,
    Village,
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

/// Bodenhöhen auf einem regelmäßigen Raster, Zeile `i` entlang Nord,
/// Spalte `j` entlang Ost: Index `i * n + j`.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightGrid {
    /// Südwest-Ecke (Nord, Ost).
    pub origin: [f64; 2],
    pub spacing: f64,
    pub n: usize,
    pub heights: Vec<f64>,
}

impl HeightGrid {
    pub fn at(&self, i: usize, j: usize) -> f64 {
        self.heights[i * self.n + j]
    }

    /// Weltposition (Nord, Ost) eines Rasterpunkts.
    pub fn position(&self, i: usize, j: usize) -> [f64; 2] {
        [self.origin[0] + i as f64 * self.spacing, self.origin[1] + j as f64 * self.spacing]
    }

    /// Steilheit (1 − Normale nach oben) aus zentralen Differenzen.
    pub fn steepness(&self, i: usize, j: usize) -> f64 {
        let n = self.n;
        let h = |a: usize, b: usize| self.at(a.min(n - 1), b.min(n - 1)).max(SEA_LEVEL_M);
        let dn = (h(i + 1, j) - h(i.saturating_sub(1), j)) / (2.0 * self.spacing);
        let de = (h(i, j + 1) - h(i, j.saturating_sub(1))) / (2.0 * self.spacing);
        1.0 - 1.0 / (dn * dn + de * de + 1.0).sqrt()
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
    s_warp: u64,
    runways: Vec<Runway>,
    /// Großform aus dem Grobraster eines Gebiets.
    grid: Option<Arc<GridField>>,
}

/// Je Rasterzelle die Geländeart und ihre Höhenvorgaben: Grundhöhe (m),
/// Gebirgs- und Hügelanteil.
#[derive(Debug, Clone, PartialEq)]
struct GridField {
    ground: Vec<Ground>,
    cells: Vec<[f64; 3]>,
}

/// Vorgaben je Geländeart: Grundhöhe (m), Gebirgsanteil, Hügelanteil.
fn profile(g: Ground) -> [f64; 3] {
    match g {
        Ground::Water => [-150.0, 0.0, 0.3],
        Ground::Plain => [60.0, 0.0, 0.5],
        Ground::Hills => [300.0, 0.12, 1.8],
        Ground::Mountains => [700.0, 1.0, 1.3],
        Ground::Desert => [120.0, 0.0, 0.6],
        Ground::Forest => [90.0, 0.0, 0.9],
        Ground::City => [60.0, 0.0, 0.1],
        Ground::Village => [70.0, 0.0, 0.35],
    }
}

/// Feinrauschen über der Grundhöhe eines Gebiets (m) und seine Wellenlänge.
const GRID_DETAIL_M: f64 = 90.0;
const GRID_DETAIL_SCALE: f64 = 12_000.0;
/// Verzerrung der Rasterabfrage (Anteil einer Zelle): natürliche Küsten.
const GRID_WARP: f64 = 0.4;

impl GridField {
    /// Werte je Zelle (`value(Index)`) weich zwischen den Zellmitten gemischt
    /// (außen: Randzellen).
    fn mix<const N: usize>(&self, x: f64, y: f64, value: impl Fn(usize) -> [f64; N]) -> [f64; N] {
        let last = (GRID_N - 1) as f64;
        let v = ((SIZE_M * 0.5 - x) / CELL_M - 0.5).clamp(0.0, last);
        let u = ((y + SIZE_M * 0.5) / CELL_M - 0.5).clamp(0.0, last);
        let (r0, c0) = (v.floor().min(last - 1.0), u.floor().min(last - 1.0));
        let (fv, fu) = (smoothstep(0.0, 1.0, v - r0), smoothstep(0.0, 1.0, u - c0));
        let idx = |r: f64, c: f64| value(r as usize * GRID_N + c as usize);
        let (a, b, c, d) = (idx(r0, c0), idx(r0, c0 + 1.0), idx(r0 + 1.0, c0), idx(r0 + 1.0, c0 + 1.0));
        std::array::from_fn(|k| {
            let top = a[k] + (b[k] - a[k]) * fu;
            let bottom = c[k] + (d[k] - c[k]) * fu;
            top + (bottom - top) * fv
        })
    }
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
        Self {
            seed,
            params,
            s_continent: sub(1),
            s_mask: sub(2),
            s_ridge: sub(3),
            s_hill: sub(4),
            s_forest: sub(5),
            s_warp: sub(6),
            runways: Vec::new(),
            grid: None,
        }
    }

    /// Gelände eines Einsatzgebiets: Großform aus dem Grobraster,
    /// Flugplätze aus der Datei.
    pub fn from_theater(theater: &Theater) -> Self {
        let ground: Vec<Ground> =
            (0..GRID_N * GRID_N).map(|k| theater.ground(k / GRID_N, k % GRID_N)).collect();
        let cells = ground.iter().map(|&g| profile(g)).collect();
        let mut t = Self::new(theater.seed);
        t.grid = Some(Arc::new(GridField { ground, cells }));
        t.runways = theater
            .airfields
            .iter()
            .map(|a| {
                let c = [a.north, a.east];
                let elevation = t.raw_height(c[0], c[1]).max(airfield::MIN_SITE_M);
                let mut r = Runway::new(a.kind, theater::intern(&a.name), c, a.heading_deg, elevation);
                r.fighters = [a.mig21, a.mig29];
                r
            })
            .collect();
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
    pub fn raw_height(&self, x: f64, y: f64) -> f64 {
        if let Some(g) = &self.grid {
            return self.grid_height(g, x, y);
        }
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

    /// Abfragepunkt im Grobraster, verzerrt für natürliche Grenzen.
    fn warped(&self, x: f64, y: f64) -> (f64, f64) {
        let ws = CELL_M * 1.5;
        let wx = fbm(self.s_warp, x / ws, y / ws, 3) * CELL_M * GRID_WARP;
        let wy = fbm(self.s_warp ^ 0x5741_5250, x / ws, y / ws, 3) * CELL_M * GRID_WARP;
        (x + wx, y + wy)
    }

    /// Geländeart der (verzerrt abgefragten) Rasterzelle, ohne Mischen – für
    /// die Oberfläche; ohne Gebiet `None`.
    pub fn ground_at(&self, x: f64, y: f64) -> Option<Ground> {
        let g = self.grid.as_ref()?;
        let (x, y) = self.warped(x, y);
        let at = |v: f64| (v / CELL_M).floor().clamp(0.0, (GRID_N - 1) as f64) as usize;
        Some(g.ground[at(SIZE_M * 0.5 - x) * GRID_N + at(y + SIZE_M * 0.5)])
    }

    /// Eine Eigenschaft `f` der Geländeart, so weich gemischt und verzerrt
    /// wie die Landschaft selbst; ohne Gebiet `None`.
    pub fn ground_mix(&self, x: f64, y: f64, f: impl Fn(Ground) -> f64) -> Option<f64> {
        let g = self.grid.as_ref()?;
        let (x, y) = self.warped(x, y);
        Some(g.mix(x, y, |k| [f(g.ground[k])])[0])
    }

    /// Höhe aus dem Grobraster: Vorgaben der verzerrt abgefragten Zelle, dazu
    /// Feinrauschen, Gebirgsgrate und Hügel nach ihrem Anteil.
    fn grid_height(&self, g: &GridField, x: f64, y: f64) -> f64 {
        let p = &self.params;
        let (wx, wy) = self.warped(x, y);
        let [base, mountain, hill] = g.mix(wx, wy, |k| g.cells[k]);
        let detail = fbm(self.s_continent, x / GRID_DETAIL_SCALE, y / GRID_DETAIL_SCALE, 4) * GRID_DETAIL_M;
        let rs = p.ridge_scale;
        let r = ridged(self.s_ridge, x / rs, y / rs, 5);
        let hs = p.hill_scale;
        let hills = fbm(self.s_hill, x / hs, y / hs, 4) * p.hill_amplitude * hill;
        base + detail + mountain * r * r * p.mountain_height + hills
    }

    /// Obere Schranke der Geländehöhe (m): Rauschfunktionen liegen in −1..1.
    /// Darüber muss niemand das Gelände abfragen (z. B. Bombenvorhersage).
    pub fn max_elevation(&self) -> f64 {
        let p = &self.params;
        if self.grid.is_some() {
            let [base, mountain, hill] = profile(Ground::Mountains);
            return base
                + GRID_DETAIL_M
                + mountain * p.mountain_height
                + hill.max(1.8) * p.hill_amplitude
                + 50.0;
        }
        p.continent_amplitude.abs() + p.base_height + p.mountain_height.abs() + p.hill_amplitude.abs() + 50.0
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
        self.classify(x, y, h, 1.0 - self.normal(x, y)[2])
    }

    /// Oberflächenklasse aus bereits bekannter Bodenhöhe `h` und Steilheit
    /// (1 − Normale nach oben) – für Raster, die Höhen schon vorliegen haben.
    pub fn classify(&self, x: f64, y: f64, h: f64, steepness: f64) -> Surface {
        if h < SEA_LEVEL_M {
            return Surface::Water;
        }
        if h < 6.0 {
            return Surface::Beach;
        }
        if self.runways.iter().any(|r| r.in_flat_area(x, y)) {
            return Surface::Grass;
        }
        if steepness > 0.30 {
            return Surface::Rock;
        }
        if h > 1_900.0 {
            return Surface::Snow;
        }
        if h > 1_350.0 {
            return Surface::Rock;
        }
        match self.ground_at(x, y) {
            Some(Ground::Desert) => Surface::Sand,
            Some(Ground::Forest) => Surface::Forest,
            Some(Ground::City) => Surface::City,
            // verstreute Siedlungen zwischen Wiesen
            Some(Ground::Village) => {
                let houses = fbm(self.s_forest ^ 0x5649_4C4C, x / 900.0, y / 900.0, 2);
                if houses > -0.1 { Surface::Village } else { Surface::Grass }
            }
            _ => {
                let forest = fbm(self.s_forest, x / 7_000.0, y / 7_000.0, 4);
                if forest > 0.18 && h < 1_100.0 { Surface::Forest } else { Surface::Grass }
            }
        }
    }

    /// Bodenhöhen (inkl. Meeresgrund) auf einem quadratischen Raster mit
    /// `n × n` Punkten um `center` (Nord, Ost), Kantenlänge `2 · half_extent`.
    pub fn height_grid(&self, center: [f64; 2], half_extent: f64, n: usize) -> HeightGrid {
        let spacing = 2.0 * half_extent / (n.max(2) - 1) as f64;
        let origin = [center[0] - half_extent, center[1] - half_extent];
        let heights = par_rows(n, |i| {
            (0..n)
                .map(|j| self.ground_height(origin[0] + i as f64 * spacing, origin[1] + j as f64 * spacing))
                .collect()
        });
        HeightGrid { origin, spacing, n, heights }
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

/// Berechnet `n` Zeilen parallel auf allen Kernen und hängt sie aneinander.
pub fn par_rows<T: Send>(n: usize, row: impl Fn(usize) -> Vec<T> + Sync) -> Vec<T> {
    let threads = std::thread::available_parallelism().map_or(1, |k| k.get());
    let chunk = n.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        let parts: Vec<_> = (0..n)
            .step_by(chunk)
            .map(|a| {
                let row = &row;
                s.spawn(move || (a..(a + chunk).min(n)).flat_map(row).collect::<Vec<T>>())
            })
            .collect();
        parts.into_iter().flat_map(|h| h.join().expect("Rasterzeile")).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_elevation_bounds_the_terrain() {
        for seed in [1, 42, 0x0F29, 9_999] {
            let t = Terrain::new(seed);
            let bound = t.max_elevation();
            let mut highest = f64::MIN;
            for i in -150..=150 {
                for j in -150..=150 {
                    highest = highest.max(t.elevation(f64::from(i) * 700.0, f64::from(j) * 700.0));
                }
            }
            assert!(highest < bound, "Seed {seed}: {highest:.0} m über Schranke {bound:.0} m");
        }
    }

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

    fn theater() -> Terrain {
        Terrain::from_theater(Theater::builtin())
    }

    #[test]
    fn airfields_come_from_the_theater() {
        let t = theater();
        let r = t.runways();
        assert_eq!(r.len(), Theater::builtin().airfields.len());
        assert_eq!(r[0].kind, AirfieldKind::Home);
        assert!(r.iter().any(|x| x.kind == AirfieldKind::Enemy && x.fighters != [0, 0]));
        assert_eq!(theater().runways(), r);
        assert!(Terrain::new(1).runways().is_empty(), "ohne Gebiet keine Plätze");
    }

    /// Neue Geländearten (seit Format 2): Eine Fläche ganz aus einer Art hat
    /// deren Oberfläche.
    #[test]
    fn new_ground_types_have_their_surface() {
        use crate::theater::{AirfieldDef, FORMAT, Grid};
        for (c, surface) in
            [('d', Surface::Sand), ('f', Surface::Forest), ('C', Surface::City), ('v', Surface::Village)]
        {
            let home = AirfieldDef {
                name: "HOME".into(),
                kind: AirfieldKind::Home,
                north: -30_000.0,
                east: -30_000.0,
                heading_deg: 0.0,
                mig21: 0,
                mig29: 0,
            };
            let th = Theater {
                format: FORMAT,
                name: format!("UNI {c}"),
                seed: 7,
                grid: Grid {
                    ground: vec![c.to_string().repeat(GRID_N); GRID_N],
                    side: vec!["N".repeat(GRID_N); GRID_N],
                },
                airfields: vec![home],
                objects: vec![],
            };
            let t = Terrain::from_theater(&Theater::parse(&th.to_text()).expect("lesbar"));
            let at = [(15_000.0, 15_000.0), (5_000.0, 20_000.0), (20_000.0, -5_000.0)];
            assert!(at.iter().all(|&(n, e)| t.ground_at(n, e).map(Ground::to_char) == Some(c)));
            // Dörfer: Siedlungen zwischen Wiesen
            let ok = |s: Surface| s == surface || (c == 'v' && s == Surface::Grass);
            assert!(at.iter().all(|&(n, e)| ok(t.surface(n, e))), "{c}");
            let k = (0..400)
                .filter(|k| t.surface(f64::from(k % 20) * 1e3, f64::from(k / 20) * 1e3) == surface)
                .count();
            assert!(k > 100, "{c}: nur {k} von 400 Punkten");
        }
    }

    /// Das Grobraster gibt die Großform vor: In der Mitte von Wasserzellen
    /// ist Meer, in Gebirgszellen ist es hoch; die Schranke hält.
    #[test]
    fn grid_shapes_the_land() {
        let (th, t) = (Theater::builtin(), theater());
        let (mut wet, mut water, mut high, mut mountains) = (0, 0, 0, 0);
        for k in 0..GRID_N * GRID_N {
            let (r, c) = (k / GRID_N, k % GRID_N);
            let (x, y) = theater::cell_center(r, c);
            let h = t.ground_height(x, y);
            assert!(h < t.max_elevation());
            match th.ground(r, c) {
                Ground::Water => {
                    water += 1;
                    wet += usize::from(h < SEA_LEVEL_M);
                }
                Ground::Mountains => {
                    mountains += 1;
                    high += usize::from(h > 500.0);
                }
                _ => {}
            }
        }
        assert!(wet * 10 >= water * 8, "nur {wet} von {water} Wasserzellen nass");
        assert!(high * 10 >= mountains * 7, "nur {high} von {mountains} Gebirgszellen hoch");
    }

    #[test]
    fn runways_are_flat_on_land_and_blend_smoothly() {
        let t = theater();
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
        let t = theater();
        for rw in t.runways() {
            let (a, b) = rw.numbers();
            assert!((1..=36).contains(&a) && (1..=36).contains(&b));
            assert_eq!((a + 18 - 1) % 36 + 1, b);
        }
    }

    #[test]
    fn height_grid_matches_point_queries() {
        let t = Terrain::new(4);
        let g = t.height_grid([1_000.0, -2_000.0], 10_000.0, 21);
        assert_eq!(g.heights.len(), 21 * 21);
        assert!((g.spacing - 1_000.0).abs() < 1e-9);
        for (i, j) in [(0, 0), (20, 20), (7, 13)] {
            let p = g.position(i, j);
            assert_eq!(g.at(i, j).to_bits(), t.ground_height(p[0], p[1]).to_bits());
            let class = t.classify(p[0], p[1], g.at(i, j), 1.0 - t.normal(p[0], p[1])[2]);
            assert_eq!(class, t.surface(p[0], p[1]));
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
