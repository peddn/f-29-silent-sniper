//! Gradientenrauschen und fraktale Summen, nur aus Integer-Hashing und
//! IEEE-754-Grundrechenarten aufgebaut – damit auf allen Plattformen bitgleich.

/// SplitMix64-Finalisierer: verteilt Eingabebits gleichmäßig.
#[inline]
pub(crate) fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[inline]
fn lattice_hash(seed: u64, ix: i64, iy: i64) -> u64 {
    mix64(
        seed ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (iy as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F),
    )
}

const D: f64 = std::f64::consts::FRAC_1_SQRT_2;
const GRADIENTS: [(f64, f64); 8] =
    [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (D, D), (-D, D), (D, -D), (-D, -D)];

/// Quintische Glättung 6t⁵ − 15t⁴ + 10t³ (stetige zweite Ableitung).
#[inline]
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[inline]
fn grad_dot(seed: u64, ix: i64, iy: i64, dx: f64, dy: f64) -> f64 {
    let (gx, gy) = GRADIENTS[(lattice_hash(seed, ix, iy) >> 61) as usize];
    gx * dx + gy * dy
}

/// 2D-Gradientenrauschen, Wertebereich etwa [-1, 1], 0 auf Gitterpunkten.
pub fn gradient_noise(seed: u64, x: f64, y: f64) -> f64 {
    let fx = x.floor();
    let fy = y.floor();
    let ix = fx as i64;
    let iy = fy as i64;
    let dx = x - fx;
    let dy = y - fy;

    let n00 = grad_dot(seed, ix, iy, dx, dy);
    let n10 = grad_dot(seed, ix + 1, iy, dx - 1.0, dy);
    let n01 = grad_dot(seed, ix, iy + 1, dx, dy - 1.0);
    let n11 = grad_dot(seed, ix + 1, iy + 1, dx - 1.0, dy - 1.0);

    let u = fade(dx);
    let v = fade(dy);
    // Maximalbetrag eines 2D-Gradientenrauschens ist sqrt(1/2); auf ~[-1, 1] skalieren.
    lerp(lerp(n00, n10, u), lerp(n01, n11, u), v) * std::f64::consts::SQRT_2
}

/// Fraktale Brownsche Bewegung: Summe mehrerer Oktaven, normiert auf ~[-1, 1].
pub fn fbm(seed: u64, x: f64, y: f64, octaves: u32) -> f64 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut norm = 0.0;
    let mut freq = 1.0;
    for o in 0..octaves {
        let s = mix64(seed.wrapping_add(u64::from(o)));
        // Versatz pro Oktave verhindert, dass alle Oktaven am Ursprung 0 sind.
        let off = f64::from(o) * 17.13;
        sum += amp * gradient_noise(s, x * freq + off, y * freq - off);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

/// Grat-Rauschen für Gebirgsketten, Wertebereich [0, 1].
pub fn ridged(seed: u64, x: f64, y: f64, octaves: u32) -> f64 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut norm = 0.0;
    let mut freq = 1.0;
    let mut weight = 1.0;
    for o in 0..octaves {
        let s = mix64(seed.wrapping_add(0x5151 + u64::from(o)));
        let off = f64::from(o) * 31.7;
        let n = gradient_noise(s, x * freq + off, y * freq + off);
        let mut r = 1.0 - n.abs();
        r *= r;
        r *= weight;
        weight = (r * 2.0).clamp(0.0, 1.0);
        sum += amp * r;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    (sum / norm).clamp(0.0, 1.0)
}

/// Hermite-Übergang zwischen `e0` und `e1`.
pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_zero_on_lattice_points() {
        for i in -5..5 {
            for j in -5..5 {
                assert_eq!(gradient_noise(7, f64::from(i), f64::from(j)), 0.0);
            }
        }
    }

    #[test]
    fn noise_stays_in_range() {
        let mut max: f64 = 0.0;
        for i in 0..20_000 {
            let x = f64::from(i) * 0.137 - 900.0;
            let y = f64::from(i) * 0.071 + 33.3;
            let n = gradient_noise(99, x, y);
            max = max.max(n.abs());
            let f = fbm(99, x, y, 6);
            assert!(f.abs() <= 1.0 + 1e-9, "fbm außerhalb: {f}");
            let r = ridged(99, x, y, 5);
            assert!((0.0..=1.0).contains(&r));
        }
        assert!(max <= 1.0 + 1e-9, "Rauschen außerhalb: {max}");
        assert!(max > 0.5, "Rauschen zu flach: {max}");
    }

    #[test]
    fn different_seeds_differ() {
        let a = fbm(1, 12.34, 56.78, 5);
        let b = fbm(2, 12.34, 56.78, 5);
        assert_ne!(a, b);
    }
}
