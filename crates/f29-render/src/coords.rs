//! NED (Simulation) ↔ Y-up (Darstellung).
//!
//! Abbildung `M`: (Nord, Ost, Unten) → (x = Ost, y = −Unten, z = −Nord).
//! `M` ist eine echte Drehung (det = +1), daher bleibt die Händigkeit
//! erhalten. Dieselbe Abbildung bringt Körperachsen (vorn, rechts, unten)
//! auf Modellachsen (−z, +x, −y) – ein Modell mit Nase in −z ist also
//! ohne weitere Korrektur richtig ausgerichtet.

use f29_core::{DQuat, DVec3};

/// Position/Vektor von NED nach Y-up.
pub fn ned_to_yup(v: DVec3) -> [f64; 3] {
    [v.y, -v.z, -v.x]
}

/// Lage von NED nach Y-up als Quaternion `[x, y, z, w]`.
///
/// Für eine Drehung R gilt R' = M·R·Mᵀ; beim Quaternion bleibt w gleich und
/// der Vektoranteil wird wie ein Vektor mit M abgebildet.
pub fn quat_ned_to_yup(q: DQuat) -> [f64; 4] {
    let [x, y, z] = ned_to_yup(DVec3::new(q.x, q.y, q.z));
    [x, y, z, q.w]
}

/// Position relativ zu einem Ursprung (Floating Origin) als `f32`.
///
/// Die Differenz wird in `f64` gebildet und erst dann gerundet, damit auch
/// hunderte Kilometer vom Weltursprung keine Zitterei entsteht.
pub fn relative_yup(pos: DVec3, origin: DVec3) -> [f32; 3] {
    let d = ned_to_yup(pos - origin);
    [d[0] as f32, d[1] as f32, d[2] as f32]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    /// Dreht v mit einem `[x, y, z, w]`-Quaternion.
    fn rotate(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
        let quat = DQuat::from_wxyz(q[3], q[0], q[1], q[2]);
        quat.rotate(DVec3::new(v[0], v[1], v[2])).to_array()
    }

    fn approx(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-12)
    }

    #[test]
    fn axes_map_correctly() {
        assert_eq!(ned_to_yup(DVec3::X), [0.0, 0.0, -1.0]); // Nord → −z
        assert_eq!(ned_to_yup(DVec3::Y), [1.0, 0.0, 0.0]); // Ost → +x
        let down = ned_to_yup(DVec3::Z);
        assert_eq!(down, [0.0, -1.0, 0.0]); // Unten → −y
    }

    #[test]
    fn model_nose_follows_heading() {
        // Kurs Ost: Modellnase (−z) muss nach +x (Ost) zeigen.
        let q = quat_ned_to_yup(DQuat::from_euler(FRAC_PI_2, 0.0, 0.0));
        assert!(approx(rotate(q, [0.0, 0.0, -1.0]), [1.0, 0.0, 0.0]));
    }

    #[test]
    fn conversion_commutes_with_rotation() {
        let q = DQuat::from_euler(0.7, -0.3, 1.9);
        let v = DVec3::new(1.5, -2.0, 0.25);
        let lhs = ned_to_yup(q.rotate(v));
        let rhs = rotate(quat_ned_to_yup(q), ned_to_yup(v));
        assert!(approx(lhs, rhs));
    }

    #[test]
    fn relative_position_keeps_precision_far_away() {
        let origin = DVec3::new(400_000.123, -300_000.456, -2_000.0);
        let pos = origin + DVec3::new(0.01, 0.02, 0.0);
        let r = relative_yup(pos, origin);
        assert!((r[0] - 0.02).abs() < 1e-6 && (r[2] + 0.01).abs() < 1e-6);
    }
}
