//! Internationale Standardatmosphäre (ISA) bis 20 km.

use crate::detmath;

pub const G0: f64 = 9.806_65;
pub const RHO0: f64 = 1.225;
const T0: f64 = 288.15;
const P0: f64 = 101_325.0;
const LAPSE: f64 = 0.006_5;
const R_AIR: f64 = 287.052_87;
const GAMMA: f64 = 1.4;
const TROPOPAUSE_M: f64 = 11_000.0;
const T_TROPOPAUSE: f64 = T0 - LAPSE * TROPOPAUSE_M;

/// Luftzustand in einer bestimmten Höhe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Air {
    pub temperature_k: f64,
    pub pressure_pa: f64,
    pub density: f64,
    pub speed_of_sound: f64,
}

/// ISA-Zustand für eine geometrische Höhe in Metern (begrenzt auf -500..20 000 m).
pub fn isa(altitude_m: f64) -> Air {
    let h = altitude_m.clamp(-500.0, 20_000.0);
    let exponent = G0 / (LAPSE * R_AIR);
    let (t, p) = if h <= TROPOPAUSE_M {
        let t = T0 - LAPSE * h;
        (t, P0 * detmath::powf(t / T0, exponent))
    } else {
        let p11 = P0 * detmath::powf(T_TROPOPAUSE / T0, exponent);
        let p = p11 * detmath::exp(-G0 / (R_AIR * T_TROPOPAUSE) * (h - TROPOPAUSE_M));
        (T_TROPOPAUSE, p)
    };
    Air {
        temperature_k: t,
        pressure_pa: p,
        density: p / (R_AIR * t),
        speed_of_sound: (GAMMA * R_AIR * t).sqrt(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sea_level() {
        let a = isa(0.0);
        assert!((a.density - RHO0).abs() < 1e-3);
        assert!((a.speed_of_sound - 340.29).abs() < 0.1);
    }

    #[test]
    fn reference_points() {
        // Tabellenwerte der US-Standardatmosphäre 1976
        assert!((isa(5_000.0).density - 0.7364).abs() < 1e-3);
        assert!((isa(11_000.0).density - 0.3639).abs() < 1e-3);
        let d15 = isa(15_000.0).density;
        assert!((d15 - 0.193_67).abs() < 1e-4, "rho(15 km) = {d15}");
    }

    #[test]
    fn density_decreases_monotonically() {
        let mut last = f64::MAX;
        for i in 0..200 {
            let rho = isa(f64::from(i) * 100.0).density;
            assert!(rho < last);
            last = rho;
        }
    }
}
