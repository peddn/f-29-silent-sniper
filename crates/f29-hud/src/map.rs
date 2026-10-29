//! Kartendaten für das linke MFD: Höhenlinien, Route, Flugplätze, Ziele.
//!
//! Die Daten liegen in Weltkoordinaten (Nord, Ost) und werden einmal pro
//! Seed erzeugt. Beim Zeichnen werden sie kursbezogen („Flugrichtung oben“)
//! um das Flugzeug gedreht und auf den Bildschirm des MFDs abgebildet.

use f29_core::nav::Steerpoint;
use f29_core::targets::{Target, TargetClass, TargetRole};
use f29_core::terrain::{AirfieldKind, HeightGrid, Terrain};

/// Halbe Kantenlänge des Kartengebiets um die Heimatbasis (m).
pub const MAP_HALF_EXTENT_M: f64 = 100_000.0;
/// Rasterpunkte je Kante für die Höhenlinien (500 m Abstand).
pub const CONTOUR_GRID_N: usize = 401;

/// Mittelpunkt des Kartengebiets: die Heimatbasis (oder der Ursprung).
pub fn map_center(terrain: &Terrain) -> [f64; 2] {
    terrain.runways().first().map_or([0.0, 0.0], |rw| rw.center)
}

/// Höhenlinien in Metern; die erste ist die Küstenlinie.
pub const CONTOUR_LEVELS: [f64; 4] = [0.0, 400.0, 900.0, 1_500.0];

/// Ein Höhenlinien-Segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContourSegment {
    /// Index in [`CONTOUR_LEVELS`] (0 = Küste).
    pub level: u8,
    pub a: [f64; 2],
    pub b: [f64; 2],
}

/// Ein Flugplatz auf der Karte.
#[derive(Debug, Clone, PartialEq)]
pub struct MapRunway {
    pub name: &'static str,
    pub kind: AirfieldKind,
    /// Ecken der Bahn (Nord, Ost), umlaufend.
    pub corners: [[f64; 2]; 4],
    pub center: [f64; 2],
}

/// Ein Navigationspunkt auf der Karte.
#[derive(Debug, Clone, PartialEq)]
pub struct MapPoint {
    pub name: String,
    pub pos: [f64; 2],
}

/// Ein Bodenziel auf der Karte (Index wie in `Sim::targets`).
#[derive(Debug, Clone, PartialEq)]
pub struct MapTarget {
    pub name: &'static str,
    pub class: TargetClass,
    pub role: TargetRole,
    pub civilian: bool,
    pub pos: [f64; 2],
}

/// Alles, was die Karte zeichnet, außer dem Flugzeug selbst.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MapData {
    pub contours: Vec<ContourSegment>,
    pub runways: Vec<MapRunway>,
    pub route: Vec<MapPoint>,
    pub targets: Vec<MapTarget>,
}

impl MapData {
    /// Kartendaten für das Standard-Kartengebiet (dauert einige Zehntelsekunden).
    pub fn for_terrain(terrain: &Terrain, route: &[Steerpoint], targets: &[Target]) -> Self {
        let grid = terrain.height_grid(map_center(terrain), MAP_HALF_EXTENT_M, CONTOUR_GRID_N);
        Self::build(terrain, &grid, route, targets)
    }

    /// Baut die Kartendaten aus einem Höhenraster (für die Höhenlinien) sowie
    /// den Flugplätzen des Geländes, der Route und den Zielen.
    pub fn build(terrain: &Terrain, grid: &HeightGrid, route: &[Steerpoint], targets: &[Target]) -> Self {
        let runways = terrain
            .runways()
            .iter()
            .map(|rw| {
                let (hl, hw) = (rw.length_m * 0.5, rw.width_m.max(150.0) * 0.5);
                MapRunway {
                    name: rw.name,
                    kind: rw.kind,
                    corners: [rw.world(-hl, -hw), rw.world(hl, -hw), rw.world(hl, hw), rw.world(-hl, hw)],
                    center: rw.center,
                }
            })
            .collect();
        let route =
            route.iter().map(|sp| MapPoint { name: sp.name.clone(), pos: [sp.north, sp.east] }).collect();
        let targets = targets
            .iter()
            .map(|t| MapTarget {
                name: t.name,
                class: t.class,
                role: t.role,
                civilian: t.civilian,
                pos: [t.north, t.east],
            })
            .collect();
        Self { contours: contours(grid), runways, route, targets }
    }
}

/// Höhenlinien per Marching Squares über das Raster.
pub fn contours(grid: &HeightGrid) -> Vec<ContourSegment> {
    let n = grid.n;
    let mut out = Vec::new();
    for (level, &h) in CONTOUR_LEVELS.iter().enumerate() {
        for i in 0..n.saturating_sub(1) {
            for j in 0..n.saturating_sub(1) {
                // Ecken gegen den Uhrzeigersinn: (i,j) (i,j+1) (i+1,j+1) (i+1,j)
                let c = [(i, j), (i, j + 1), (i + 1, j + 1), (i + 1, j)];
                let v = c.map(|(a, b)| grid.at(a, b) - h);
                let mut pts: Vec<[f64; 2]> = Vec::with_capacity(4);
                for k in 0..4 {
                    let (va, vb) = (v[k], v[(k + 1) % 4]);
                    if (va < 0.0) != (vb < 0.0) {
                        let t = va / (va - vb);
                        let pa = grid.position(c[k].0, c[k].1);
                        let pb = grid.position(c[(k + 1) % 4].0, c[(k + 1) % 4].1);
                        pts.push([pa[0] + (pb[0] - pa[0]) * t, pa[1] + (pb[1] - pa[1]) * t]);
                    }
                }
                // 2 Schnittpunkte: ein Segment; 4 (Sattel): zwei Segmente
                let (pairs, _) = pts.as_chunks::<2>();
                for [a, b] in pairs {
                    out.push(ContourSegment { level: level as u8, a: *a, b: *b });
                }
            }
        }
    }
    out
}

/// Kursbezogene Abbildung Welt → MFD-Bildschirm.
#[derive(Debug, Clone, Copy)]
pub struct MapProjection {
    pub center_world: [f64; 2],
    pub heading: f64,
    /// Bildschirmeinheiten pro Meter.
    pub scale: f64,
    pub center_screen: [f32; 2],
}

impl MapProjection {
    pub fn to_screen(&self, p: [f64; 2]) -> [f32; 2] {
        let (dn, de) = (p[0] - self.center_world[0], p[1] - self.center_world[1]);
        let (s, c) = self.heading.sin_cos();
        let fwd = dn * c + de * s;
        let right = -dn * s + de * c;
        [
            self.center_screen[0] + (right * self.scale) as f32,
            self.center_screen[1] + (fwd * self.scale) as f32,
        ]
    }

    /// Bildschirmrichtung (Einheitsvektor) einer rechtweisenden Richtung.
    pub fn direction(&self, bearing: f64) -> [f32; 2] {
        let rel = bearing - self.heading;
        [rel.sin() as f32, rel.cos() as f32]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contour_of_a_cone_is_closed_ring() {
        // Kegel: Höhe 1000 − r; die 400-m-Linie ist ein Kreis mit r = 600.
        let n = 41;
        let spacing = 50.0;
        let origin = [-1_000.0, -1_000.0];
        let mut heights = Vec::new();
        for i in 0..n {
            for j in 0..n {
                let (x, y) = (origin[0] + i as f64 * spacing, origin[1] + j as f64 * spacing);
                heights.push(1_000.0 - (x * x + y * y).sqrt());
            }
        }
        let grid = HeightGrid { origin, spacing, n, heights };
        let segs: Vec<_> = contours(&grid).into_iter().filter(|s| s.level == 1).collect();
        assert!(segs.len() > 40);
        for s in &segs {
            for p in [s.a, s.b] {
                let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
                assert!((r - 600.0).abs() < 15.0, "Punkt auf r = {r}");
            }
        }
    }

    #[test]
    fn projection_puts_heading_up() {
        let p =
            MapProjection { center_world: [0.0, 0.0], heading: 1.0, scale: 0.001, center_screen: [0.0, 0.0] };
        // 1 km voraus in Flugrichtung ⇒ 1 Einheit nach oben
        let ahead = p.to_screen([1.0_f64.cos() * 1_000.0, 1.0_f64.sin() * 1_000.0]);
        assert!(ahead[0].abs() < 1e-5 && (ahead[1] - 1.0).abs() < 1e-5);
        let d = p.direction(1.0 + std::f64::consts::FRAC_PI_2);
        assert!((d[0] - 1.0).abs() < 1e-6 && d[1].abs() < 1e-6, "90° rechts ⇒ nach rechts");
    }
}
