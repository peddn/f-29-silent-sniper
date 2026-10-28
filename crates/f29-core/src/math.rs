//! Minimale f64-Vektor- und Quaternion-Typen für den Simulationskern.
//!
//! Bewusst eigene Typen statt einer Mathe-Bibliothek: volle Kontrolle über
//! die Rechenreihenfolge (Determinismus) und keine Abhängigkeit, deren
//! Implementierungsdetails sich zwischen Versionen ändern.

use crate::detmath;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// 3D-Vektor in doppelter Genauigkeit.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl DVec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const X: Self = Self::new(1.0, 0.0, 0.0);
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);
    pub const Z: Self = Self::new(0.0, 0.0, 1.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, o: Self) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Self) -> Self {
        Self::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }

    pub fn length_squared(self) -> f64 {
        self.dot(self)
    }

    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }

    /// Normierter Vektor; Nullvektor bleibt Nullvektor.
    pub fn normalize_or_zero(self) -> Self {
        let l = self.length();
        if l > 0.0 { self / l } else { Self::ZERO }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    pub fn to_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
}

impl Add for DVec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl AddAssign for DVec3 {
    fn add_assign(&mut self, o: Self) {
        *self = *self + o;
    }
}

impl Sub for DVec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl SubAssign for DVec3 {
    fn sub_assign(&mut self, o: Self) {
        *self = *self - o;
    }
}

impl Mul<f64> for DVec3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Mul<DVec3> for f64 {
    type Output = DVec3;
    fn mul(self, v: DVec3) -> DVec3 {
        v * self
    }
}

impl Div<f64> for DVec3 {
    type Output = Self;
    fn div(self, s: f64) -> Self {
        Self::new(self.x / s, self.y / s, self.z / s)
    }
}

impl Neg for DVec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

/// Einheitsquaternion (w, x, y, z) für Rotationen.
///
/// Konvention im Kern: `attitude.rotate(v_body) = v_world`, d. h. das
/// Quaternion dreht Körperachsen in Weltachsen (NED).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DQuat {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for DQuat {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl DQuat {
    pub const IDENTITY: Self = Self { w: 1.0, x: 0.0, y: 0.0, z: 0.0 };

    pub const fn from_wxyz(w: f64, x: f64, y: f64, z: f64) -> Self {
        Self { w, x, y, z }
    }

    /// Aus Euler-Winkeln (Luftfahrt-Reihenfolge Z-Y-X: Gier ψ, Nick θ, Roll φ).
    pub fn from_euler(yaw: f64, pitch: f64, roll: f64) -> Self {
        let (sy, cy) = detmath::sin_cos(yaw * 0.5);
        let (sp, cp) = detmath::sin_cos(pitch * 0.5);
        let (sr, cr) = detmath::sin_cos(roll * 0.5);
        Self {
            w: cr * cp * cy + sr * sp * sy,
            x: sr * cp * cy - cr * sp * sy,
            y: cr * sp * cy + sr * cp * sy,
            z: cr * cp * sy - sr * sp * cy,
        }
    }

    /// Euler-Winkel `(yaw, pitch, roll)` in Z-Y-X-Reihenfolge.
    pub fn to_euler(self) -> (f64, f64, f64) {
        let Self { w, x, y, z } = self;
        let roll = detmath::atan2(2.0 * (w * x + y * z), 1.0 - 2.0 * (x * x + y * y));
        let pitch = detmath::asin(2.0 * (w * y - z * x));
        let yaw = detmath::atan2(2.0 * (w * z + x * y), 1.0 - 2.0 * (y * y + z * z));
        (yaw, pitch, roll)
    }

    /// Rotation um eine Achse (muss normiert sein).
    pub fn from_axis_angle(axis: DVec3, angle: f64) -> Self {
        let (s, c) = detmath::sin_cos(angle * 0.5);
        Self { w: c, x: axis.x * s, y: axis.y * s, z: axis.z * s }
    }

    /// Rotation um den Vektor `v`, Drehwinkel = |v| (Rotationsvektor).
    pub fn from_rotation_vector(v: DVec3) -> Self {
        let angle = v.length();
        if angle < 1e-12 {
            // Taylor-Näherung erster Ordnung, danach normieren.
            return Self { w: 1.0, x: v.x * 0.5, y: v.y * 0.5, z: v.z * 0.5 }.normalize();
        }
        Self::from_axis_angle(v / angle, angle)
    }

    pub fn conjugate(self) -> Self {
        Self { w: self.w, x: -self.x, y: -self.y, z: -self.z }
    }

    pub fn length_squared(self) -> f64 {
        self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn normalize(self) -> Self {
        let l = self.length_squared().sqrt();
        Self { w: self.w / l, x: self.x / l, y: self.y / l, z: self.z / l }
    }

    /// Dreht einen Vektor: q · v · q*.
    pub fn rotate(self, v: DVec3) -> DVec3 {
        // Optimierte Form: t = 2·(q_v × v); v' = v + w·t + q_v × t
        let qv = DVec3::new(self.x, self.y, self.z);
        let t = qv.cross(v) * 2.0;
        v + t * self.w + qv.cross(t)
    }

    /// Dreht einen Vektor mit der inversen Rotation (Welt → Körper).
    pub fn inverse_rotate(self, v: DVec3) -> DVec3 {
        self.conjugate().rotate(v)
    }

    pub fn is_finite(self) -> bool {
        self.w.is_finite() && self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl Mul for DQuat {
    type Output = Self;
    /// Hamilton-Produkt: `(a * b).rotate(v) == a.rotate(b.rotate(v))`.
    fn mul(self, b: Self) -> Self {
        let a = self;
        Self {
            w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
            x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
            y: a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
            z: a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    fn approx(a: DVec3, b: DVec3) -> bool {
        (a - b).length() < 1e-12
    }

    #[test]
    fn yaw_90_turns_north_into_east() {
        let q = DQuat::from_euler(FRAC_PI_2, 0.0, 0.0);
        assert!(approx(q.rotate(DVec3::X), DVec3::Y));
    }

    #[test]
    fn pitch_up_raises_nose() {
        // NED: z zeigt nach unten, also bedeutet Nase hoch negatives z.
        let q = DQuat::from_euler(0.0, 0.3, 0.0);
        let nose = q.rotate(DVec3::X);
        assert!(nose.z < 0.0);
    }

    #[test]
    fn roll_right_lowers_right_wing() {
        let q = DQuat::from_euler(0.0, 0.0, 0.5);
        let right_wing = q.rotate(DVec3::Y);
        assert!(right_wing.z > 0.0);
    }

    #[test]
    fn euler_roundtrip() {
        for &(y, p, r) in &[(0.1, 0.2, 0.3), (-2.5, -1.0, 2.9), (3.0, 1.2, -0.4)] {
            let (y2, p2, r2) = DQuat::from_euler(y, p, r).to_euler();
            assert!((y - y2).abs() < 1e-12 && (p - p2).abs() < 1e-12 && (r - r2).abs() < 1e-12);
        }
    }

    #[test]
    fn product_composes_rotations() {
        let a = DQuat::from_euler(0.4, -0.2, 1.1);
        let b = DQuat::from_euler(-1.3, 0.7, 0.2);
        let v = DVec3::new(0.3, -2.0, 5.0);
        assert!(approx((a * b).rotate(v), a.rotate(b.rotate(v))));
        assert!(approx(a.inverse_rotate(a.rotate(v)), v));
    }

    #[test]
    fn rotation_vector_matches_axis_angle() {
        let q = DQuat::from_rotation_vector(DVec3::new(0.0, 0.0, 0.5));
        let r = DQuat::from_axis_angle(DVec3::Z, 0.5);
        assert!((q.w - r.w).abs() < 1e-15 && (q.z - r.z).abs() < 1e-15);
        let tiny = DQuat::from_rotation_vector(DVec3::new(1e-14, 0.0, 0.0));
        assert!((tiny.length_squared() - 1.0).abs() < 1e-15);
    }
}
