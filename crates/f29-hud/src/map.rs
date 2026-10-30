//! Kartendaten für das linke MFD: Route, Flugplätze, Ziele (das Gelände
//! zeigt das Frontend als Rasterbild darunter).
//!
//! Die Daten liegen in Weltkoordinaten (Nord, Ost) und werden einmal pro
//! Seed erzeugt. Beim Zeichnen werden sie kursbezogen („Flugrichtung oben“)
//! um das Flugzeug gedreht und auf den Bildschirm des MFDs abgebildet.

use f29_core::nav::Steerpoint;
use f29_core::targets::{Target, TargetClass, TargetRole};
use f29_core::terrain::{AirfieldKind, Terrain};

/// Halbe Kantenlänge des Kartengebiets um die Heimatbasis (m).
pub const MAP_HALF_EXTENT_M: f64 = 100_000.0;
/// Mittelpunkt des Kartengebiets: die Heimatbasis (oder der Ursprung).
pub fn map_center(terrain: &Terrain) -> [f64; 2] {
    terrain.runways().first().map_or([0.0, 0.0], |rw| rw.center)
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
    pub runways: Vec<MapRunway>,
    pub route: Vec<MapPoint>,
    pub targets: Vec<MapTarget>,
}

impl MapData {
    /// Kartendaten aus den Flugplätzen des Geländes, der Route und den Zielen.
    pub fn for_terrain(terrain: &Terrain, route: &[Steerpoint], targets: &[Target]) -> Self {
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
        Self { runways, route, targets }
    }
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
