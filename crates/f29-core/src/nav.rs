//! Navigationspunkte (Steerpoints).
//!
//! In v0.1 erzeugt der Seed eine kurze Route über Land. Ab v0.2 kommen
//! Missionsziele aus den Szenariodaten hinzu.

use crate::rng::Pcg32;
use f29_terrain::Terrain;

/// RNG-Strom der Navigation.
pub const STREAM_NAV: u64 = 0x4e41_5600;

#[derive(Debug, Clone, PartialEq)]
pub struct Steerpoint {
    pub name: String,
    pub north: f64,
    pub east: f64,
    /// Geländehöhe am Punkt.
    pub elevation: f64,
}

/// Erzeugt eine Route aus `count` Punkten, beginnend nahe `(north, east)`.
pub fn generate_route(terrain: &Terrain, seed: u64, north: f64, east: f64, count: usize) -> Vec<Steerpoint> {
    let mut rng = Pcg32::new(seed, STREAM_NAV);
    let mut points = Vec::with_capacity(count);
    let (mut n, mut e) = (north, east);
    let mut bearing = rng.range_f64(0.0, std::f64::consts::TAU);
    for i in 0..count {
        // Bis zu 16 Versuche, einen Punkt über Land zu finden.
        let mut chosen = None;
        for _ in 0..16 {
            let b = bearing + rng.range_f64(-1.0, 1.0);
            let d = rng.range_f64(12_000.0, 30_000.0);
            let (sb, cb) = crate::detmath::sin_cos(b);
            let cand = (n + cb * d, e + sb * d);
            if terrain.ground_height(cand.0, cand.1) > 5.0 {
                chosen = Some((cand, b));
                break;
            }
            chosen.get_or_insert((cand, b));
        }
        let ((pn, pe), b) = chosen.expect("mindestens ein Kandidat");
        bearing = b;
        n = pn;
        e = pe;
        points.push(Steerpoint {
            name: format!("WP{}", i + 1),
            north: pn,
            east: pe,
            elevation: terrain.elevation(pn, pe),
        });
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_is_deterministic() {
        let t = Terrain::new(5);
        let a = generate_route(&t, 5, 0.0, 0.0, 4);
        let b = generate_route(&t, 5, 0.0, 0.0, 4);
        assert_eq!(a, b);
        assert_eq!(a.len(), 4);
        assert_eq!(a[3].name, "WP4");
    }
}
