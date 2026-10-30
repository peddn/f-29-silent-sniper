//! Kleine DSP-Bausteine: Rauschen, Oszillatorphasen, einfache Filter.
//!
//! Bewusst schlicht und ohne Abhängigkeiten. Alles rechnet in `f32` mit fester
//! Abtastrate; Determinismus ist hier nicht nötig (der Klang beeinflusst die
//! Simulation nicht), aber praktisch für Tests – es gibt keinen globalen Zufall.

use std::f32::consts::TAU;

/// Weißes Rauschen −1..1 (Xorshift32).
#[derive(Debug, Clone)]
pub struct Noise(u32);

impl Noise {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    pub fn sample(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Tiefpass erster Ordnung; auch zum Glätten von Parametern.
#[derive(Debug, Clone, Default)]
pub struct OnePole {
    pub y: f32,
}

impl OnePole {
    /// Koeffizient für eine Grenzfrequenz `fc` bei Abtastrate `sr`.
    pub fn coeff(fc: f32, sr: f32) -> f32 {
        1.0 - (-TAU * fc / sr).exp()
    }

    /// Koeffizient für eine Zeitkonstante `tau` (s).
    pub fn coeff_time(tau: f32, sr: f32) -> f32 {
        1.0 - (-1.0 / (tau * sr).max(1e-6)).exp()
    }

    pub fn process(&mut self, x: f32, a: f32) -> f32 {
        self.y += (x - self.y) * a;
        self.y
    }
}

/// Phase eines Oszillators 0..1.
#[derive(Debug, Clone, Default)]
pub struct Phase(pub f32);

impl Phase {
    pub fn advance(&mut self, freq: f32, sr: f32) -> f32 {
        self.0 += freq / sr;
        self.0 -= self.0.floor();
        self.0
    }
}

pub fn sine(p: f32) -> f32 {
    (TAU * p).sin()
}

/// Rechteck mit Tastverhältnis `duty`.
pub fn pulse(p: f32, duty: f32) -> f32 {
    if p < duty { 1.0 } else { -1.0 }
}

pub fn triangle(p: f32) -> f32 {
    4.0 * (p - (p + 0.5).floor()).abs() - 1.0
}

pub fn saw(p: f32) -> f32 {
    2.0 * p - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_bounded_and_centered() {
        let mut n = Noise::new(7);
        let (mut sum, mut max) = (0.0_f64, 0.0_f32);
        for _ in 0..100_000 {
            let v = n.sample();
            sum += f64::from(v);
            max = max.max(v.abs());
        }
        assert!(max <= 1.0 && max > 0.99);
        assert!((sum / 100_000.0).abs() < 0.02);
    }

    #[test]
    fn waveforms_are_bounded() {
        for k in 0..1000 {
            let p = k as f32 / 1000.0;
            for v in [sine(p), pulse(p, 0.3), triangle(p), saw(p)] {
                assert!((-1.0..=1.0).contains(&v));
            }
        }
        assert!((triangle(0.0) + 1.0).abs() < 1e-6 && (triangle(0.5) - 1.0).abs() < 1e-6);
    }
}
