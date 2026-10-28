//! Engine-neutrale Dreiecksliste.

/// Nicht indizierte Dreiecksliste mit flachen Normalen und Vertexfarben.
///
/// Farben sind **linear** (nicht sRGB) mit Alpha, so wie sie die meisten
/// Engines als Vertexattribut erwarten. Die Beleuchtung ist bereits
/// eingebacken – das Frontend rendert unbeleuchtet. Das hält den Retro-Look
/// unabhängig von Belichtung und Tonemapping der Engine.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
}

/// Richtung *zur* Sonne in Y-up-Koordinaten (normiert, von Südwesten, hoch).
pub const SUN_DIR: [f32; 3] = [-0.3665, 0.8552, 0.3665];
/// Grundhelligkeit unbeleuchteter Flächen.
pub const AMBIENT: f32 = 0.42;

impl MeshData {
    pub fn with_capacity(triangles: usize) -> Self {
        Self {
            positions: Vec::with_capacity(triangles * 3),
            normals: Vec::with_capacity(triangles * 3),
            colors: Vec::with_capacity(triangles * 3),
        }
    }

    pub fn triangle_count(&self) -> usize {
        self.positions.len() / 3
    }

    /// Fügt ein Dreieck (gegen den Uhrzeigersinn von außen gesehen) mit
    /// flacher Normale und eingebackener Lambert-Beleuchtung hinzu.
    pub fn push_lit(&mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], srgb: [f32; 3]) {
        let n = face_normal(a, b, c);
        let lit = AMBIENT + (1.0 - AMBIENT) * dot(n, SUN_DIR).max(0.0);
        let rgb = [srgb[0] * lit, srgb[1] * lit, srgb[2] * lit];
        self.push_raw(a, b, c, n, srgb_to_linear(rgb));
    }

    pub fn push_raw(&mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], n: [f32; 3], linear: [f32; 4]) {
        self.positions.extend_from_slice(&[a, b, c]);
        self.normals.extend_from_slice(&[n, n, n]);
        self.colors.extend_from_slice(&[linear, linear, linear]);
    }
}

pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = dot(v, v).sqrt();
    if l > 0.0 { [v[0] / l, v[1] / l, v[2] / l] } else { [0.0, 1.0, 0.0] }
}

/// Normale eines Dreiecks mit Umlaufsinn gegen den Uhrzeigersinn.
pub fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    normalize(cross(sub(b, a), sub(c, a)))
}

/// sRGB (0..1) → linear, Alpha = 1.
pub fn srgb_to_linear(c: [f32; 3]) -> [f32; 4] {
    let f = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    [f(c[0]), f(c[1]), f(c[2]), 1.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sun_is_normalized() {
        assert!((dot(SUN_DIR, SUN_DIR) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn ccw_triangle_faces_up() {
        // Von oben gesehen (Blick entlang −y): x rechts, −z oben.
        let n = face_normal([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, -1.0]);
        assert!(n[1] > 0.99);
    }

    #[test]
    fn srgb_conversion_endpoints() {
        assert_eq!(srgb_to_linear([0.0, 1.0, 0.5])[0], 0.0);
        assert!((srgb_to_linear([0.0, 1.0, 0.5])[1] - 1.0).abs() < 1e-6);
        assert!((srgb_to_linear([0.0, 1.0, 0.5])[2] - 0.214).abs() < 1e-3);
    }
}
