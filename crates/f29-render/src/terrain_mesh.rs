//! Terrain-Kacheln als Meshes.
//!
//! Die Welt ist in quadratische Kacheln (`CHUNK_SIZE`) eingeteilt. Jede
//! Kachel wird je nach Entfernung mit 32, 16 oder 8 Zellen pro Kante
//! trianguliert. Zwischen Kacheln unterschiedlicher Auflösung verdecken
//! „Schürzen“ (senkrechte Streifen am Rand) die entstehenden Spalten.
//!
//! Lokale Koordinaten einer Kachel (Y-up): Ursprung an der Süd-West-Ecke
//! auf Meereshöhe; x = Ost, y = Höhe, z = −Nord.

use crate::mesh::MeshData;
use crate::palette;
use f29_core::DVec3;
use f29_terrain::noise::fbm;
use f29_terrain::{Surface, Terrain};

/// Kantenlänge einer Kachel in Metern.
pub const CHUNK_SIZE: f64 = 2_048.0;

/// Index einer Kachel: `north` und `east` zählen in Kachelgrößen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChunkKey {
    pub north: i32,
    pub east: i32,
}

impl ChunkKey {
    /// Kachel, die den Punkt (Nord, Ost) enthält.
    pub fn containing(north: f64, east: f64) -> Self {
        Self { north: (north / CHUNK_SIZE).floor() as i32, east: (east / CHUNK_SIZE).floor() as i32 }
    }

    /// Süd-West-Ecke in NED auf Meereshöhe.
    pub fn origin_ned(&self) -> DVec3 {
        DVec3::new(f64::from(self.north) * CHUNK_SIZE, f64::from(self.east) * CHUNK_SIZE, 0.0)
    }

    /// Horizontaler Abstand vom Punkt zur nächstgelegenen Stelle der Kachel.
    pub fn distance_to(&self, north: f64, east: f64) -> f64 {
        let o = self.origin_ned();
        let dn = (o.x - north).max(0.0).max(north - (o.x + CHUNK_SIZE));
        let de = (o.y - east).max(0.0).max(east - (o.y + CHUNK_SIZE));
        (dn * dn + de * de).sqrt()
    }
}

/// Detailstufe einer Kachel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Lod {
    High,
    Medium,
    Low,
}

impl Lod {
    pub fn cells(self) -> usize {
        match self {
            Lod::High => 32,
            Lod::Medium => 16,
            Lod::Low => 8,
        }
    }

    /// Detailstufe für eine Entfernung; `None` = außerhalb der Sichtweite.
    pub fn for_distance(d: f64, view_distance: f64) -> Option<Self> {
        if d > view_distance {
            None
        } else if d < 5_000.0 {
            Some(Lod::High)
        } else if d < 12_000.0 {
            Some(Lod::Medium)
        } else {
            Some(Lod::Low)
        }
    }

    fn skirt_depth(self) -> f32 {
        match self {
            Lod::High => 40.0,
            Lod::Medium => 120.0,
            Lod::Low => 300.0,
        }
    }
}

/// Kleine, deterministische Helligkeitsstreuung pro Zelle (Retro-„Textur“).
fn jitter(n: i64, e: i64) -> f32 {
    let mut z =
        (n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (e as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    z = (z ^ (z >> 29)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^= z >> 32;
    (z & 0xffff) as f32 / 65_535.0 * 0.08 - 0.04
}

/// Farbe (sRGB) einer Zelle.
fn cell_color(terrain: &Terrain, north: f64, east: f64, gn: i64, ge: i64) -> [f32; 3] {
    let surface = terrain.surface(north, east);
    let v = (fbm(terrain.seed() ^ 0xC010, north / 900.0, east / 900.0, 2) * 0.5 + 0.5) as f32;
    let base = match surface {
        Surface::Water => {
            let depth = (-terrain.ground_height(north, east)).clamp(0.0, 300.0) as f32 / 300.0;
            palette::mix(2, 1, depth * 0.7)
        }
        Surface::Beach => palette::srgb(10),
        Surface::Grass => palette::mix(8, 9, v * 0.8),
        Surface::Forest => palette::mix(7, 8, v * 0.3),
        Surface::Rock => palette::mix(13, 12, v * 0.5),
        Surface::Snow => palette::srgb(6),
    };
    let j = if surface == Surface::Water { 0.0 } else { jitter(gn, ge) };
    [base[0] * (1.0 + j), base[1] * (1.0 + j), base[2] * (1.0 + j)]
}

/// Farbe einer ebenen Fläche unter der Standardbeleuchtung.
fn lit_flat(c: [f32; 3]) -> [f32; 3] {
    use crate::mesh::{AMBIENT, SUN_DIR};
    let lit = AMBIENT + (1.0 - AMBIENT) * SUN_DIR[1];
    [c[0] * lit, c[1] * lit, c[2] * lit]
}

/// Baut das Mesh einer Kachel.
pub fn build_chunk(terrain: &Terrain, key: ChunkKey, lod: Lod) -> MeshData {
    let n = lod.cells();
    let spacing = CHUNK_SIZE / n as f64;
    let ix0 = i64::from(key.north) * n as i64;
    let iy0 = i64::from(key.east) * n as i64;
    let heights = terrain.sample_grid(ix0, iy0, n, spacing);
    let o = key.origin_ned();
    let s = spacing as f32;

    // Vertex an Gitterindex (i = Nord, j = Ost) in lokalen Y-up-Koordinaten.
    let vtx =
        |i: usize, j: usize| -> [f32; 3] { [j as f32 * s, heights[j * (n + 1) + i] as f32, -(i as f32 * s)] };

    let mut mesh = MeshData::with_capacity(2 * n * n + 16 * n);
    let mut colors = vec![[0.0_f32; 3]; n * n];
    for i in 0..n {
        for j in 0..n {
            let cn = o.x + (i as f64 + 0.5) * spacing;
            let ce = o.y + (j as f64 + 0.5) * spacing;
            let c = cell_color(terrain, cn, ce, ix0 + i as i64, iy0 + j as i64);
            colors[i * n + j] = c;
            let p00 = vtx(i, j);
            let p01 = vtx(i, j + 1);
            let p10 = vtx(i + 1, j);
            let p11 = vtx(i + 1, j + 1);
            // Gegen den Uhrzeigersinn von oben: SW → SO → NO, SW → NO → NW
            mesh.push_lit(p00, p01, p11, c);
            mesh.push_lit(p00, p11, p10, c);
        }
    }

    // Schürzen an allen vier Rändern (beidseitig, damit die Orientierung egal ist).
    let depth = lod.skirt_depth();
    // Die Schürzen tragen die Farbe der Randzelle. So bleiben auch winzige
    // Rundungsspalten zwischen Kacheln unsichtbar.
    let mut skirt = |a: [f32; 3], b: [f32; 3], c: [f32; 3]| {
        let a2 = [a[0], a[1] - depth, a[2]];
        let b2 = [b[0], b[1] - depth, b[2]];
        let up = [0.0, 1.0, 0.0];
        let lin = crate::mesh::srgb_to_linear(lit_flat(c));
        mesh.push_raw(a, b, b2, up, lin);
        mesh.push_raw(a, b2, a2, up, lin);
        mesh.push_raw(b, a, a2, up, lin);
        mesh.push_raw(b, a2, b2, up, lin);
    };
    for k in 0..n {
        skirt(vtx(0, k), vtx(0, k + 1), colors[k]); // Süd
        skirt(vtx(n, k), vtx(n, k + 1), colors[(n - 1) * n + k]); // Nord
        skirt(vtx(k, 0), vtx(k + 1, 0), colors[k * n]); // West
        skirt(vtx(k, n), vtx(k + 1, n), colors[k * n + n - 1]); // Ost
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_lookup() {
        assert_eq!(ChunkKey::containing(10.0, -10.0), ChunkKey { north: 0, east: -1 });
        let k = ChunkKey { north: 2, east: 3 };
        assert_eq!(k.distance_to(2.5 * CHUNK_SIZE, 3.5 * CHUNK_SIZE), 0.0);
        assert!((k.distance_to(0.0, 3.5 * CHUNK_SIZE) - 2.0 * CHUNK_SIZE).abs() < 1e-9);
    }

    #[test]
    fn lod_selection() {
        assert_eq!(Lod::for_distance(0.0, 25_000.0), Some(Lod::High));
        assert_eq!(Lod::for_distance(8_000.0, 25_000.0), Some(Lod::Medium));
        assert_eq!(Lod::for_distance(20_000.0, 25_000.0), Some(Lod::Low));
        assert_eq!(Lod::for_distance(30_000.0, 25_000.0), None);
    }

    #[test]
    fn triangle_count_and_finite_data() {
        let t = Terrain::new(4);
        for lod in [Lod::High, Lod::Medium, Lod::Low] {
            let m = build_chunk(&t, ChunkKey { north: 1, east: -2 }, lod);
            let n = lod.cells();
            assert_eq!(m.triangle_count(), 2 * n * n + 16 * n);
            assert_eq!(m.positions.len(), m.colors.len());
            assert!(m.positions.iter().flatten().all(|v| v.is_finite()));
            assert!(m.colors.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
        }
    }

    #[test]
    fn surface_faces_up() {
        let m = build_chunk(&Terrain::new(4), ChunkKey { north: 0, east: 0 }, Lod::Medium);
        let n = Lod::Medium.cells();
        for t in 0..2 * n * n {
            assert!(m.normals[t * 3][1] > 0.0, "Dreieck {t} zeigt nach unten");
        }
    }

    #[test]
    fn neighbouring_chunks_share_edge_vertices() {
        let t = Terrain::new(8);
        let a_key = ChunkKey { north: 0, east: 0 };
        let b_key = ChunkKey { north: 0, east: 1 };
        let a = build_chunk(&t, a_key, Lod::High);
        let b = build_chunk(&t, b_key, Lod::High);
        // Weltposition (Ost, Höhe, Süd) aller Oberflächen-Vertices sammeln
        let world = |m: &MeshData, k: ChunkKey| -> Vec<[i64; 3]> {
            let o = k.origin_ned();
            let n = Lod::High.cells();
            m.positions[..2 * n * n * 3]
                .iter()
                .map(|p| {
                    [
                        ((p[0] as f64 + o.y) * 16.0).round() as i64,
                        (p[1] as f64 * 16.0).round() as i64,
                        ((p[2] as f64 - o.x) * 16.0).round() as i64,
                    ]
                })
                .collect()
        };
        let edge_x = (CHUNK_SIZE * 16.0) as i64;
        let a_edge: std::collections::BTreeSet<_> =
            world(&a, a_key).into_iter().filter(|p| p[0] == edge_x).collect();
        let b_edge: std::collections::BTreeSet<_> =
            world(&b, b_key).into_iter().filter(|p| p[0] == edge_x).collect();
        assert_eq!(a_edge.len(), Lod::High.cells() + 1);
        assert_eq!(a_edge, b_edge);
    }
}
