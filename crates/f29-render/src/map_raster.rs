//! Rasterbilder für die Karte im linken MFD: Satellitenbild und Höhenbild.
//!
//! Einmal pro Seed entsteht ein nordausgerichtetes Farbraster des
//! Kartengebiets – als Satellitenbild (Oberflächenfarben der Palette mit
//! eingebackener Hangschattierung) oder als Höhenbild in Graustufen (Wasser
//! schwarz, je höher, desto heller; gedämpft, damit Gitter und Symbole
//! darüber gut lesbar bleiben). Für jedes Bild wird daraus ein kursbezogener Ausschnitt
//! um das Flugzeug gezogen – mit Nächster-Nachbar-Abtastung, damit es pixelig
//! im Retro-Stil bleibt.

use crate::palette;
use f29_core::terrain::noise::fbm;
use f29_core::terrain::{Surface, Terrain};

/// Nordausgerichtetes Farbraster (sRGB, 8 Bit).
#[derive(Debug, Clone, PartialEq)]
pub struct MapRaster {
    /// Südwest-Ecke (Nord, Ost).
    pub origin: [f64; 2],
    /// Meter pro Pixel.
    pub spacing: f64,
    /// Pixel je Kante.
    pub n: usize,
    /// Zeile `i` entlang Nord, Spalte `j` entlang Ost.
    pub pixels: Vec<[u8; 3]>,
}

/// Art des Rasterbilds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapStyle {
    Satellite,
    Height,
}

/// Grauwerte des Höhenbilds: Küste und höchster Punkt des Kartengebiets
/// (dazwischen nach der Wurzel der Höhe, damit das Flachland Relief zeigt).
const HEIGHT_GRAY: [f32; 2] = [0.16, 0.42];
/// Wasser im Höhenbild.
const HEIGHT_WATER: [u8; 3] = [0, 0, 0];

/// Farbe außerhalb des Rasters.
const OUTSIDE: [u8; 3] = [11, 13, 23];

fn to_u8(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

impl MapRaster {
    /// Berechnet das Raster (rechenintensiv, gedacht für einen Hintergrund-Thread).
    pub fn build(terrain: &Terrain, center: [f64; 2], half_extent: f64, n: usize, style: MapStyle) -> Self {
        let grid = terrain.height_grid(center, half_extent, n);
        if style == MapStyle::Height {
            let top = grid.heights.iter().copied().fold(1.0_f64, f64::max);
            let pixels = grid
                .heights
                .iter()
                .map(|&h| {
                    if h <= 0.0 {
                        return HEIGHT_WATER;
                    }
                    let g = HEIGHT_GRAY[0] + (HEIGHT_GRAY[1] - HEIGHT_GRAY[0]) * (h / top).sqrt() as f32;
                    to_u8([g; 3])
                })
                .collect();
            return Self { origin: grid.origin, spacing: grid.spacing, n, pixels };
        }
        let sun = [0.45_f64, -0.35, 0.82]; // Licht von Südwesten (Nord, Ost, oben)
        let mut pixels = Vec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                let p = grid.position(i, j);
                let h = grid.at(i, j);
                let steep = grid.steepness(i, j);
                let surface = terrain.classify(p[0], p[1], h, steep);
                let v = (fbm(terrain.seed() ^ 0xC010, p[0] / 900.0, p[1] / 900.0, 2) * 0.5 + 0.5) as f32;
                let base = match surface {
                    Surface::Water => palette::mix(2, 1, ((-h).clamp(0.0, 300.0) / 300.0) as f32 * 0.7),
                    Surface::Beach => palette::srgb(10),
                    Surface::Grass => palette::mix(8, 9, v * 0.8),
                    Surface::Forest => palette::mix(7, 8, v * 0.3),
                    Surface::Rock => palette::mix(13, 12, v * 0.5),
                    Surface::Snow => palette::srgb(6),
                };
                // Hangschattierung aus dem Raster
                let lit = if surface == Surface::Water {
                    1.0
                } else {
                    let hn = |a: usize, b: usize| grid.at(a.min(n - 1), b.min(n - 1)).max(0.0);
                    let dn = (hn(i + 1, j) - hn(i.saturating_sub(1), j)) / (2.0 * grid.spacing);
                    let de = (hn(i, j + 1) - hn(i, j.saturating_sub(1))) / (2.0 * grid.spacing);
                    let len = (dn * dn + de * de + 1.0).sqrt();
                    let d = (-dn * sun[0] - de * sun[1] + sun[2]) / len;
                    (0.55 + 0.55 * d).clamp(0.35, 1.15) as f32
                };
                pixels.push(to_u8(base.map(|c| c * lit)));
            }
        }
        let mut map = Self { origin: grid.origin, spacing: grid.spacing, n, pixels };
        // Bahnen als dunkle bzw. erdfarbene Striche einzeichnen
        for rw in terrain.runways() {
            let color = match rw.kind {
                f29_core::terrain::AirfieldKind::Home => to_u8(palette::mix(0, 13, 0.3)),
                f29_core::terrain::AirfieldKind::Secret => to_u8(palette::srgb(11)),
            };
            let steps = (rw.length_m / (map.spacing * 0.5)).ceil() as usize;
            for k in 0..=steps {
                let u = -rw.length_m * 0.5 + rw.length_m * k as f64 / steps as f64;
                let p = rw.world(u, 0.0);
                if let Some(idx) = map.index(p) {
                    map.pixels[idx] = color;
                }
            }
        }
        map
    }

    fn index(&self, p: [f64; 2]) -> Option<usize> {
        let i = ((p[0] - self.origin[0]) / self.spacing).round();
        let j = ((p[1] - self.origin[1]) / self.spacing).round();
        let n = self.n as f64;
        (i >= 0.0 && j >= 0.0 && i < n && j < n).then(|| i as usize * self.n + j as usize)
    }

    /// Farbe an einem Weltpunkt (nächster Rasterpunkt).
    pub fn sample(&self, p: [f64; 2]) -> [u8; 3] {
        self.index(p).map_or(OUTSIDE, |k| self.pixels[k])
    }

    /// Kursbezogener Ausschnitt als RGBA-Bild (`size × size`, Zeile 0 oben):
    /// Flugzeug in der Mitte, Flugrichtung oben, `range_m` von der Mitte bis
    /// zum Rand.
    pub fn render_view(&self, center: [f64; 2], heading: f64, range_m: f64, size: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(size * size * 4);
        let (s, c) = heading.sin_cos();
        let m_per_px = 2.0 * range_m / size as f64;
        for row in 0..size {
            let fwd = (size as f64 * 0.5 - row as f64 - 0.5) * m_per_px;
            for col in 0..size {
                let right = (col as f64 + 0.5 - size as f64 * 0.5) * m_per_px;
                let p = [center[0] + fwd * c - right * s, center[1] + fwd * s + right * c];
                let [r, g, b] = self.sample(p);
                out.extend_from_slice(&[r, g, b, 255]);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_is_heading_up_and_centered() {
        // Künstliches Raster: Nordhälfte weiß, Südhälfte schwarz
        let n = 64;
        let mut pixels = Vec::new();
        for i in 0..n {
            for _ in 0..n {
                pixels.push(if i >= n / 2 { [255, 255, 255] } else { [0, 0, 0] });
            }
        }
        let map = MapRaster { origin: [-640.0, -640.0], spacing: 20.0, n, pixels };
        let px = |img: &[u8], size: usize, r: usize, c: usize| img[(r * size + c) * 4];
        // Kurs Nord: oben weiß, unten schwarz
        let img = map.render_view([0.0, 0.0], 0.0, 300.0, 32);
        assert_eq!(px(&img, 32, 2, 16), 255);
        assert_eq!(px(&img, 32, 29, 16), 0);
        // Kurs Ost: Norden liegt links
        let img = map.render_view([0.0, 0.0], std::f64::consts::FRAC_PI_2, 300.0, 32);
        assert_eq!(px(&img, 32, 16, 2), 255);
        assert_eq!(px(&img, 32, 16, 29), 0);
        // Weit außerhalb: Rahmenfarbe
        let img = map.render_view([5_000.0, 0.0], 0.0, 300.0, 8);
        assert_eq!(&img[0..3], &OUTSIDE);
    }

    #[test]
    fn real_terrain_has_water_and_land() {
        let t = Terrain::new(0x0F29);
        let map = MapRaster::build(&t, [0.0, 0.0], 60_000.0, 96, MapStyle::Satellite);
        let water = to_u8(palette::mix(2, 1, 0.0));
        let blueish = map.pixels.iter().filter(|p| p[2] > p[1] && p[2] > p[0]).count();
        assert!(blueish > 0 && blueish < map.pixels.len(), "Wasser {blueish} von {}", map.pixels.len());
        assert_ne!(water, OUTSIDE);
    }

    /// Höhenbild: Wasser schwarz, an Land grau und je höher, desto heller.
    #[test]
    fn height_map_is_gray_and_brighter_uphill() {
        let t = Terrain::new(0x0F29);
        let (c, half, n) = ([0.0, 0.0], 60_000.0, 96);
        let map = MapRaster::build(&t, c, half, n, MapStyle::Height);
        let grid = t.height_grid(c, half, n);
        assert!(map.pixels.iter().all(|p| p[0] == p[1] && p[1] == p[2]));
        let mut land: Vec<(f64, u8)> = Vec::new();
        for (k, &h) in grid.heights.iter().enumerate() {
            if h <= 0.0 {
                assert_eq!(map.pixels[k], HEIGHT_WATER);
            } else {
                land.push((h, map.pixels[k][0]));
            }
        }
        land.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert!(land.len() > 10);
        assert!(land.first().unwrap().1 < land.last().unwrap().1);
    }
}
