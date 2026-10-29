//! 3D-Modelle der Bodenziele aus den Quaderformen des Simulationskerns
//! (`f29_core::targets::TargetClass::shape`).
//!
//! Modellachsen (Y-up): Zielachse „vorn“ → −z, „rechts“ → +x, „oben“ → +y.
//! Das Frontend dreht das Modell um +y mit −Kurs.

use crate::mesh::MeshData;
use crate::palette;
use f29_core::targets::TargetClass;

/// Grundfarbe einer Klasse (sRGB); zivile Gebäude und Schiffe heller.
fn base_color(class: TargetClass, civilian: bool) -> [f32; 3] {
    match class {
        TargetClass::Soft => palette::mix(8, 12, 0.6),
        TargetClass::Armored => palette::mix(7, 13, 0.45),
        TargetClass::Building if civilian => palette::mix(14, 10, 0.4),
        TargetClass::Building => palette::mix(13, 11, 0.4),
        TargetClass::Hardened => palette::mix(13, 12, 0.35),
        TargetClass::AirDefense => palette::mix(7, 13, 0.55),
        TargetClass::EarlyWarning => palette::mix(13, 7, 0.4),
        TargetClass::Bridge => palette::srgb(13),
        TargetClass::ParkedAircraft => palette::mix(14, 13, 0.4),
        TargetClass::ShipSmall | TargetClass::ShipLarge if civilian => palette::mix(15, 12, 0.3),
        TargetClass::ShipSmall | TargetClass::ShipLarge => palette::mix(13, 1, 0.25),
    }
}

/// Modell eines Ziels.
pub fn target(class: TargetClass, civilian: bool) -> MeshData {
    let shape = class.shape();
    let mut m = MeshData::with_capacity(shape.len() * 12);
    let base = base_color(class, civilian);
    for (k, b) in shape.iter().enumerate() {
        // Aufbauten etwas heller als der Rumpf
        let t = if k == 0 { 0.0 } else { 0.15 };
        let col =
            [base[0] + (1.0 - base[0]) * t, base[1] + (1.0 - base[1]) * t, base[2] + (1.0 - base[2]) * t];
        let c = [b.center[1], b.center[2], -b.center[0]];
        let h = [b.half[1], b.half[2], b.half[0]];
        push_box(&mut m, c, h, col);
    }
    m
}

/// Quader mit nach außen zeigenden Flächen (Y-up, achsenparallel).
fn push_box(m: &mut MeshData, c: [f32; 3], h: [f32; 3], col: [f32; 3]) {
    let v = |sx: f32, sy: f32, sz: f32| [c[0] + sx * h[0], c[1] + sy * h[1], c[2] + sz * h[2]];
    // Jede Fläche: vier Ecken gegen den Uhrzeigersinn von außen gesehen.
    let faces = [
        [v(1., -1., -1.), v(1., 1., -1.), v(1., 1., 1.), v(1., -1., 1.)], // +x
        [v(-1., -1., 1.), v(-1., 1., 1.), v(-1., 1., -1.), v(-1., -1., -1.)], // −x
        [v(-1., 1., -1.), v(-1., 1., 1.), v(1., 1., 1.), v(1., 1., -1.)], // +y
        [v(-1., -1., 1.), v(-1., -1., -1.), v(1., -1., -1.), v(1., -1., 1.)], // −y
        [v(-1., -1., 1.), v(1., -1., 1.), v(1., 1., 1.), v(-1., 1., 1.)], // +z
        [v(1., -1., -1.), v(-1., -1., -1.), v(-1., 1., -1.), v(1., 1., -1.)], // −z
    ];
    for [a, b, c2, d] in faces {
        m.push_lit(a, b, c2, col);
        m.push_lit(a, c2, d, col);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{dot, face_normal, sub};

    #[test]
    fn every_class_builds_and_faces_point_outward() {
        for class in TargetClass::ALL {
            let m = target(class, false);
            assert_eq!(m.triangle_count(), class.shape().len() * 12);
            // Für jeden Quader zeigen die Flächennormalen weg von seiner Mitte.
            for (k, b) in class.shape().iter().enumerate() {
                let c = [b.center[1], b.center[2], -b.center[0]];
                for t in 0..12 {
                    let i = (k * 12 + t) * 3;
                    let (p0, p1, p2) = (m.positions[i], m.positions[i + 1], m.positions[i + 2]);
                    let n = face_normal(p0, p1, p2);
                    let centroid = [
                        (p0[0] + p1[0] + p2[0]) / 3.0,
                        (p0[1] + p1[1] + p2[1]) / 3.0,
                        (p0[2] + p1[2] + p2[2]) / 3.0,
                    ];
                    assert!(dot(n, sub(centroid, c)) > 0.0, "{class:?}: Fläche nach innen");
                }
            }
        }
    }
}
