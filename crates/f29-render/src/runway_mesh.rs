//! Start- und Landebahnen als Meshes, dazu die PAPI-Anflughilfe.
//!
//! Lokale Koordinaten (Y-up): Ursprung in der Bahnmitte auf Bahnhöhe;
//! x = Ost, y = oben, z = −Nord – wie die Terrain-Kacheln.
//!
//! Die Heimatbasis bekommt eigene, vereinfachte Markierungen: Randlinien,
//! Mittellinie, Schwellenbalken, Zielpunktmarken und Bahnnummern in einer
//! 3×5-Blockschrift. Geheime Pisten sind eine unbefestigte Schneise mit nur
//! wenigen weißen Eckmarken.

use crate::mesh::{MeshData, face_normal};
use crate::palette;
use f29_terrain::{AirfieldKind, Runway};

/// Höhe der Bahnoberfläche über dem (eingeebneten) Gelände, gegen Z-Fighting.
const SURFACE_Y: f32 = 0.35;
/// Markierungen liegen knapp über der Bahn.
const MARK_Y: f32 = 0.45;

/// Abstand des Zielpunkts (und der PAPI) von der Schwelle in Metern.
pub const AIM_POINT_M: f64 = 300.0;
/// Gleitwinkel, auf den die PAPI eingestellt ist (rad).
pub const PAPI_GLIDE: f64 = 0.052_36;

/// 3×5-Blockziffern, Zeilen von oben nach unten, Bit 2 = links.
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b001, 0b001, 0b001],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

/// Hilfsobjekt: zeichnet Rechtecke in Bahnkoordinaten (u entlang, v quer).
struct Painter<'a> {
    rw: &'a Runway,
    mesh: MeshData,
}

impl Painter<'_> {
    /// Bahnkoordinaten → lokale Y-up-Koordinaten.
    fn point(&self, u: f64, v: f64, y: f32) -> [f32; 3] {
        let d = self.rw.dir;
        let north = u * d[0] - v * d[1];
        let east = u * d[1] + v * d[0];
        [east as f32, y, -north as f32]
    }

    /// Waagerechtes Rechteck, Normale nach oben.
    fn rect(&mut self, u0: f64, u1: f64, v0: f64, v1: f64, y: f32, srgb: [f32; 3]) {
        let a = self.point(u0, v0, y);
        let b = self.point(u1, v0, y);
        let c = self.point(u1, v1, y);
        let e = self.point(u0, v1, y);
        for (p, q, r) in [(a, b, c), (a, c, e)] {
            if face_normal(p, q, r)[1] >= 0.0 {
                self.mesh.push_lit(p, q, r, srgb);
            } else {
                self.mesh.push_lit(p, r, q, srgb);
            }
        }
    }

    /// Ziffern der Bahnnummer für die Landerichtung `forward` (+u oder −u),
    /// beginnend `start` Meter hinter der Schwelle, lesbar im Anflug.
    fn number(&mut self, number: u32, forward: f64, start: f64) {
        let white = palette::srgb(6);
        let (cell_v, cell_u) = (3.0, 4.0);
        let text = format!("{number:02}");
        let width = text.len() as f64 * 4.0 * cell_v - cell_v;
        let threshold = -forward * self.rw.length_m * 0.5;
        for (k, ch) in text.bytes().enumerate() {
            let glyph = DIGITS[usize::from(ch - b'0')];
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..3 {
                    if bits & (0b100 >> col) == 0 {
                        continue;
                    }
                    // Zeile 0 liegt am weitesten von der Schwelle entfernt.
                    let along = start + (4 - row) as f64 * cell_u;
                    let across = -width * 0.5 + (k as f64 * 4.0 + col as f64) * cell_v;
                    let u0 = threshold + forward * along;
                    let u1 = u0 + forward * cell_u;
                    // „Links“ im Anflug ist −v bei forward = +1, sonst +v.
                    let v0 = forward * across;
                    let v1 = v0 + forward * cell_v;
                    self.rect(u0.min(u1), u0.max(u1), v0.min(v1), v0.max(v1), MARK_Y, white);
                }
            }
        }
    }
}

/// Mesh einer Bahn in lokalen Koordinaten (Ursprung Bahnmitte, Bahnhöhe 0).
pub fn runway(rw: &Runway) -> MeshData {
    let mut p = Painter { rw, mesh: MeshData::with_capacity(400) };
    let (hl, hw) = (rw.length_m * 0.5, rw.width_m * 0.5);
    let white = palette::srgb(6);
    match rw.kind {
        AirfieldKind::Home => {
            p.rect(-hl, hl, -hw, hw, SURFACE_Y, palette::mix(0, 13, 0.45));
            // Randlinien
            for s in [-1.0, 1.0] {
                p.rect(-hl, hl, s * hw - 1.5, s * hw - 0.5, MARK_Y, white);
            }
            // Mittellinie: 30 m Strich, 20 m Lücke
            let mut u = -hl + 120.0;
            while u + 30.0 < hl - 120.0 {
                p.rect(u, u + 30.0, -0.45, 0.45, MARK_Y, white);
                u += 50.0;
            }
            let (num_a, num_b) = rw.numbers();
            for (forward, number) in [(1.0, num_a), (-1.0, num_b)] {
                let t = -forward * hl;
                // Schwellenbalken: 4 je Seite
                for i in 0..4 {
                    let v = 3.0 + f64::from(i) * 4.5;
                    for s in [-1.0, 1.0] {
                        let (u0, u1) = (t + forward * 6.0, t + forward * 36.0);
                        p.rect(u0.min(u1), u0.max(u1), s * v - 0.9, s * v + 0.9, MARK_Y, white);
                    }
                }
                p.number(number, forward, 45.0);
                // Zielpunktmarken
                let (u0, u1) = (t + forward * (AIM_POINT_M - 22.0), t + forward * (AIM_POINT_M + 22.0));
                for s in [-1.0, 1.0] {
                    p.rect(u0.min(u1), u0.max(u1), s * 6.0 - 2.5, s * 6.0 + 2.5, MARK_Y, white);
                }
            }
        }
        AirfieldKind::Secret => {
            // Unbefestigte Schneise in Erdfarben, nur Eck- und wenige Randmarken.
            p.rect(-hl, hl, -hw, hw, SURFACE_Y, palette::mix(11, 12, 0.5));
            for su in [-1.0, 1.0] {
                for sv in [-1.0, 1.0] {
                    let u = su * (hl - 3.0);
                    let v = sv * (hw - 1.5);
                    p.rect(u - 2.0, u + 2.0, v - 1.0, v + 1.0, MARK_Y, white);
                }
            }
            let mut u = -hl + 250.0;
            while u < hl - 200.0 {
                for sv in [-1.0, 1.0] {
                    p.rect(u - 1.0, u + 1.0, sv * hw - 1.0, sv * hw, MARK_Y, palette::mix(6, 11, 0.4));
                }
                u += 250.0;
            }
        }
    }
    p.mesh
}

/// Ein PAPI-Licht: Position (Nord, Ost) und Grenzwinkel. Wer höher als der
/// Grenzwinkel steht, sieht es weiß, sonst rot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PapiLight {
    pub north: f64,
    pub east: f64,
    pub elevation_m: f64,
    pub angle_rad: f64,
    /// Landerichtung, für die das Licht gilt (+1 = `dir`, −1 = Gegenrichtung).
    pub forward: f64,
}

impl PapiLight {
    /// Ob ein Beobachter an `eye` (Nord, Ost, Höhe) das Licht weiß sieht.
    /// Von hinten (hinter der Schwelle in Landerichtung) ist es unsichtbar.
    pub fn is_white(&self, eye_north: f64, eye_east: f64, eye_alt: f64) -> bool {
        let (dn, de) = (eye_north - self.north, eye_east - self.east);
        let dist = (dn * dn + de * de).sqrt().max(1.0);
        (eye_alt - self.elevation_m).atan2(dist) > self.angle_rad
    }

    /// Ob das Licht für einen Beobachter dieser Bahnrichtung gilt.
    pub fn faces(&self, rw: &Runway, eye_north: f64, eye_east: f64) -> bool {
        let (u, _) = rw.local(eye_north, eye_east);
        let (lu, _) = rw.local(self.north, self.east);
        (lu - u) * self.forward > 0.0
    }
}

/// PAPI-Lichter (je vier pro Landerichtung, links der Bahn) der Heimatbasis.
pub fn papi_lights(rw: &Runway) -> Vec<PapiLight> {
    if rw.kind != AirfieldKind::Home {
        return Vec::new();
    }
    let mut out = Vec::new();
    // Grenzwinkel von innen (nahe der Bahn, höchster) nach außen.
    let offsets = [0.5, 0.17, -0.17, -0.5_f64];
    for forward in [1.0, -1.0] {
        let u = -forward * (rw.length_m * 0.5 - AIM_POINT_M);
        for (i, off) in offsets.iter().enumerate() {
            // links der Landerichtung
            let v = -forward * (rw.width_m * 0.5 + 15.0 + i as f64 * 9.0);
            let p = rw.world(u, v);
            out.push(PapiLight {
                north: p[0],
                east: p[1],
                elevation_m: rw.elevation_m,
                angle_rad: PAPI_GLIDE + off.to_radians(),
                forward,
            });
        }
    }
    out
}

/// Ein Lichtkasten (Würfel 6 × 6 × 2 m) in lokalen Koordinaten um den Ursprung.
pub fn light_box(srgb: [f32; 3]) -> MeshData {
    let mut m = MeshData::with_capacity(10);
    let (h, top) = (3.0_f32, 2.0_f32);
    let c = [[-h, -h], [h, -h], [h, h], [-h, h]];
    // Deckel – unbeleuchtet hell (Lichter leuchten selbst)
    let lin = crate::mesh::srgb_to_linear(srgb);
    let up = [0.0, 1.0, 0.0];
    let p = |i: usize, y: f32| [c[i][0], y, c[i][1]];
    m.push_raw(p(0, top), p(3, top), p(2, top), up, lin);
    m.push_raw(p(0, top), p(2, top), p(1, top), up, lin);
    // Seiten
    for i in 0..4 {
        let j = (i + 1) % 4;
        let (a, b, cc, d) = (p(i, 0.0), p(j, 0.0), p(j, top), p(i, top));
        let n = face_normal(a, b, cc);
        let (a, b, cc, d) =
            if n[0] * (a[0] + b[0]) + n[2] * (a[2] + b[2]) >= 0.0 { (a, b, cc, d) } else { (b, a, d, cc) };
        let n = face_normal(a, b, cc);
        m.push_raw(a, b, cc, n, lin);
        m.push_raw(a, cc, d, n, lin);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use f29_terrain::Terrain;

    #[test]
    fn runway_meshes_cover_the_runway_and_face_up() {
        let t = Terrain::new(0x0F29);
        for rw in t.runways() {
            let m = runway(rw);
            assert!(m.triangle_count() >= 2);
            assert!(m.normals.iter().all(|n| n[1] > 0.99), "{}: Fläche zeigt nicht nach oben", rw.name);
            let max_r =
                m.positions.iter().map(|p| (p[0] * p[0] + p[2] * p[2]).sqrt()).fold(0.0_f32, f32::max);
            let half_diag = ((rw.length_m * 0.5).powi(2) + (rw.width_m * 0.5).powi(2)).sqrt() as f32;
            assert!((max_r - half_diag).abs() < 1.0, "{}: {max_r} vs {half_diag}", rw.name);
        }
        let home = &t.runways()[0];
        assert!(runway(home).triangle_count() > 100, "Heimatbasis ohne Markierungen");
    }

    #[test]
    fn papi_shows_two_white_two_red_on_glide_path() {
        let t = Terrain::new(0x0F29);
        let rw = &t.runways()[0];
        let lights = papi_lights(rw);
        assert_eq!(lights.len(), 8);
        let fwd: Vec<_> = lights.iter().filter(|l| l.forward > 0.0).collect();
        let at = |dist: f64, glide: f64| {
            let u = -rw.length_m * 0.5 + AIM_POINT_M - dist;
            let p = rw.world(u, 0.0);
            let alt = rw.elevation_m + dist * glide.tan();
            fwd.iter().filter(|l| l.is_white(p[0], p[1], alt)).count()
        };
        assert_eq!(at(4_000.0, PAPI_GLIDE), 2, "auf dem Gleitpfad");
        assert_eq!(at(4_000.0, 0.075), 4, "zu hoch");
        assert_eq!(at(4_000.0, 0.035), 0, "zu tief");
        let eye = rw.world(-rw.length_m, 0.0);
        assert!(fwd.iter().all(|l| l.faces(rw, eye[0], eye[1])));
    }

    #[test]
    fn digits_are_drawn() {
        let t = Terrain::new(3);
        let rw = &t.runways()[0];
        let mut p = Painter { rw, mesh: MeshData::default() };
        p.number(8, 1.0, 45.0);
        // „08“: 0 hat 12 Zellen, 8 hat 13 Zellen, je zwei Dreiecke
        assert_eq!(p.mesh.triangle_count(), (12 + 13) * 2);
    }
}
