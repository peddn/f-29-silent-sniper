//! Das Low-Poly-Modell des F-29 – eigener Entwurf.
//!
//! Facettierter Nurflügler mit Pfeilflügeln, Sägezahn-Hinterkante und
//! zwei nach außen geneigten Leitwerksflossen. Länge 16,5 m, Spannweite 13 m.
//! Modellachsen (Y-up): Nase in −z, rechte Fläche in +x, oben +y.

use crate::mesh::{MeshData, dot, face_normal, sub};
use crate::palette;

/// Länge des Modells in Metern (für Kameraabstände).
pub const LENGTH_M: f32 = 16.5;

pub fn aircraft() -> MeshData {
    // Grundriss (x, z) im Uhrzeigersinn von oben, beginnend an der Nase.
    let nose = [0.0, 0.0, -9.0];
    let r_tip = [6.5, 0.0, 4.0];
    let r_notch = [3.0, 0.0, 5.0];
    let r_tail = [2.2, 0.0, 7.5];
    let rear = [0.0, 0.1, 6.2];
    let mirror = |p: [f32; 3]| [-p[0], p[1], p[2]];
    let (l_tip, l_notch, l_tail) = (mirror(r_tip), mirror(r_notch), mirror(r_tail));

    let canopy = [0.0, 1.15, -1.5];
    let ridge = [0.0, 0.85, 3.8];
    let belly = [0.0, -0.65, 1.0];
    let center = [0.0, 0.1, 1.5];

    let hull = palette::mix(13, 0, 0.55); // dunkles Tarngrau
    let hull_dark = palette::mix(13, 0, 0.7);
    let glass = palette::mix(10, 0, 0.6); // goldbedampfte Haube

    let mut m = MeshData::with_capacity(48);
    // Dreieck nach außen ausrichten (Normale weg vom Modellzentrum).
    let mut outward = |a: [f32; 3], b: [f32; 3], c: [f32; 3], col: [f32; 3]| {
        let n = face_normal(a, b, c);
        let centroid = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0];
        if dot(n, sub(centroid, center)) >= 0.0 {
            m.push_lit(a, b, c, col);
        } else {
            m.push_lit(a, c, b, col);
        }
    };

    for (tip, notch, tail) in [(r_tip, r_notch, r_tail), (l_tip, l_notch, l_tail)] {
        // Oberseite
        outward(nose, tip, canopy, hull);
        outward(canopy, tip, ridge, hull);
        outward(ridge, tip, notch, hull);
        outward(ridge, notch, tail, hull);
        outward(ridge, tail, rear, hull);
        // Unterseite
        outward(nose, tip, belly, hull_dark);
        outward(tip, notch, belly, hull_dark);
        outward(notch, tail, belly, hull_dark);
        outward(tail, rear, belly, hull_dark);
    }
    // Kanzel als kleines eigenes Dreieck auf der Oberseite
    let cf = [0.0, 0.62, -5.0];
    let cl = [-0.7, 0.75, -1.8];
    let cr = [0.7, 0.75, -1.8];
    outward(cf, cr, canopy, glass);
    outward(cf, canopy, cl, glass);

    // Leitwerk: zwei nach außen geneigte Flossen, beidseitig sichtbar.
    for s in [1.0_f32, -1.0] {
        let a = [1.3 * s, 0.6, 3.4];
        let b = [1.6 * s, 0.45, 6.6];
        let tip = [2.9 * s, 2.5, 6.8];
        m.push_lit(a, b, tip, hull);
        m.push_lit(a, tip, b, hull);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_dimensions() {
        let m = aircraft();
        let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &m.positions {
            for i in 0..3 {
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i]);
            }
        }
        let length = max[2] - min[2];
        let span = max[0] - min[0];
        assert!((length - LENGTH_M).abs() < 1.0, "Länge {length}");
        assert!((12.0..14.0).contains(&span), "Spannweite {span}");
        // Nase zeigt nach −z
        assert_eq!(min[2], -9.0);
    }

    #[test]
    fn top_faces_point_up() {
        let m = aircraft();
        // Die ersten 5 Dreiecke je Seite sind Oberseite
        for t in (0..5).chain(9..14) {
            assert!(m.normals[t * 3][1] > 0.0, "Oberseiten-Dreieck {t} zeigt nach unten");
        }
    }
}
