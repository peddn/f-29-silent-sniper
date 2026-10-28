//! Deterministische transzendente Funktionen.
//!
//! IEEE 754 garantiert bitgenaue Ergebnisse nur für `+ - * /`, `sqrt`,
//! `floor`/`round` und Vergleiche. `f64::sin`, `atan2`, `exp`, `powf` usw.
//! rufen dagegen die Mathe-Bibliothek der Plattform auf und dürfen zwischen
//! Betriebssystemen, Compilern und CPUs um einige ULP abweichen. Für eine
//! Simulation mit bitgleichen Replays ist das nicht akzeptabel.
//!
//! Deshalb implementiert dieses Modul die benötigten Funktionen selbst, nur
//! aus den exakt spezifizierten Grundoperationen. Rust kontrahiert
//! Gleitkomma-Ausdrücke nicht eigenmächtig zu FMA, daher ist das Ergebnis auf
//! allen 64-Bit-Zielen mit IEEE-754-Hardware identisch.
//!
//! **Regel für den Simulationskern:** Kein `f64::sin/cos/tan/atan2/exp/ln/powf`
//! – immer die Funktionen aus diesem Modul. `sqrt` ist erlaubt.
//!
//! Genauigkeit: wenige ULP, mehr als genug für die Simulation.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

// Cody-Waite-Aufspaltung von π/2 bzw. ln 2 (Konstanten wie in fdlibm).
const PIO2_HI: f64 = 1.570_796_326_734_125_6e0;
const PIO2_LO: f64 = 6.077_100_506_506_192e-11;
const LN2_HI: f64 = 6.931_471_803_691_238e-1;
const LN2_LO: f64 = 1.908_214_929_270_587_7e-10;
const TAN_PI_8: f64 = 0.414_213_562_373_095_03;

/// Zerlegt `x` in Quadrant `k` und Rest `r` mit |r| <= π/4.
#[inline]
fn reduce_pio2(x: f64) -> (i64, f64) {
    let k = (x * std::f64::consts::FRAC_2_PI).round();
    let r = (x - k * PIO2_HI) - k * PIO2_LO;
    (k as i64, r)
}

/// Taylor-Polynom für sin auf [-π/4, π/4] (Restglied < 1e-16).
#[inline]
fn sin_kernel(r: f64) -> f64 {
    const C3: f64 = -1.0 / 6.0;
    const C5: f64 = 1.0 / 120.0;
    const C7: f64 = -1.0 / 5_040.0;
    const C9: f64 = 1.0 / 362_880.0;
    const C11: f64 = -1.0 / 39_916_800.0;
    const C13: f64 = 1.0 / 6_227_020_800.0;
    const C15: f64 = -1.0 / 1_307_674_368_000.0;
    let z = r * r;
    r + r * z * (C3 + z * (C5 + z * (C7 + z * (C9 + z * (C11 + z * (C13 + z * C15))))))
}

/// Taylor-Polynom für cos auf [-π/4, π/4] (Restglied < 1e-17).
#[inline]
fn cos_kernel(r: f64) -> f64 {
    const C2: f64 = -1.0 / 2.0;
    const C4: f64 = 1.0 / 24.0;
    const C6: f64 = -1.0 / 720.0;
    const C8: f64 = 1.0 / 40_320.0;
    const C10: f64 = -1.0 / 3_628_800.0;
    const C12: f64 = 1.0 / 479_001_600.0;
    const C14: f64 = -1.0 / 87_178_291_200.0;
    const C16: f64 = 1.0 / 20_922_789_888_000.0;
    let z = r * r;
    1.0 + z * (C2 + z * (C4 + z * (C6 + z * (C8 + z * (C10 + z * (C12 + z * (C14 + z * C16)))))))
}

/// Sinus und Kosinus gleichzeitig.
pub fn sin_cos(x: f64) -> (f64, f64) {
    if !x.is_finite() {
        return (f64::NAN, f64::NAN);
    }
    let (k, r) = reduce_pio2(x);
    let s = sin_kernel(r);
    let c = cos_kernel(r);
    match k.rem_euclid(4) {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

pub fn sin(x: f64) -> f64 {
    sin_cos(x).0
}

pub fn cos(x: f64) -> f64 {
    sin_cos(x).1
}

pub fn tan(x: f64) -> f64 {
    let (s, c) = sin_cos(x);
    s / c
}

/// atan-Reihe für |u| <= tan(π/8).
#[inline]
fn atan_series(u: f64) -> f64 {
    // atan(u) = u · Σ (-1)^n u^(2n) / (2n+1); 24 Terme reichen für |u| <= 0.4143.
    let z = u * u;
    let mut acc = 0.0;
    let mut n = 23_i32;
    while n >= 0 {
        let term = 1.0 / f64::from(2 * n + 1);
        acc = if n % 2 == 0 { term } else { -term } + z * acc;
        n -= 1;
    }
    u * acc
}

/// atan für 0 <= t <= 1.
#[inline]
fn atan_unit(t: f64) -> f64 {
    if t > TAN_PI_8 { FRAC_PI_4 + atan_series((t - 1.0) / (t + 1.0)) } else { atan_series(t) }
}

pub fn atan(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let a = x.abs();
    let r = if a > 1.0 { FRAC_PI_2 - atan_unit(1.0 / a) } else { atan_unit(a) };
    if x < 0.0 { -r } else { r }
}

pub fn atan2(y: f64, x: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if x == 0.0 {
        return if y > 0.0 {
            FRAC_PI_2
        } else if y < 0.0 {
            -FRAC_PI_2
        } else {
            0.0
        };
    }
    let base = atan(y / x);
    if x > 0.0 {
        base
    } else if y >= 0.0 {
        base + PI
    } else {
        base - PI
    }
}

/// asin, Eingabe wird auf [-1, 1] begrenzt.
pub fn asin(x: f64) -> f64 {
    let x = x.clamp(-1.0, 1.0);
    atan2(x, ((1.0 - x) * (1.0 + x)).sqrt())
}

pub fn acos(x: f64) -> f64 {
    let x = x.clamp(-1.0, 1.0);
    atan2(((1.0 - x) * (1.0 + x)).sqrt(), x)
}

/// 2^k als f64 (k im normalen Exponentenbereich).
#[inline]
fn pow2i(k: i64) -> f64 {
    debug_assert!((-1022..=1023).contains(&k));
    f64::from_bits(((k + 1023) as u64) << 52)
}

pub fn exp(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x > 709.7 {
        return f64::INFINITY;
    }
    if x < -745.0 {
        return 0.0;
    }
    let k = (x / std::f64::consts::LN_2).round();
    let r = (x - k * LN2_HI) - k * LN2_LO;
    // Taylor-Reihe bis r^18, |r| <= ln2/2.
    let mut acc = 1.0;
    let mut n = 18;
    while n >= 1 {
        acc = 1.0 + r * acc / f64::from(n);
        n -= 1;
    }
    let k = k as i64;
    // Skalierung in zwei Schritten, damit auch Randbereiche keine Überläufe haben.
    let k1 = k / 2;
    let k2 = k - k1;
    acc * pow2i(k1) * pow2i(k2)
}

/// Natürlicher Logarithmus.
pub fn ln(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x.is_infinite() {
        return f64::INFINITY;
    }
    let mut x = x;
    let mut e: i64 = 0;
    if x < f64::MIN_POSITIVE {
        // subnormal: hochskalieren
        x *= pow2i(54);
        e -= 54;
    }
    let bits = x.to_bits();
    e += ((bits >> 52) & 0x7ff) as i64 - 1023;
    let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | (1023_u64 << 52));
    if m > std::f64::consts::SQRT_2 {
        m *= 0.5;
        e += 1;
    }
    // ln(m) = 2·atanh(s), s = (m-1)/(m+1), |s| <= 0.1716
    let s = (m - 1.0) / (m + 1.0);
    let z = s * s;
    let mut acc = 0.0;
    let mut n = 14_i32;
    while n >= 0 {
        acc = 1.0 / f64::from(2 * n + 1) + z * acc;
        n -= 1;
    }
    let ln_m = 2.0 * s * acc;
    let ef = e as f64;
    ef * LN2_HI + (ef * LN2_LO + ln_m)
}

/// x^y für x > 0.
pub fn powf(x: f64, y: f64) -> f64 {
    if y == 0.0 {
        return 1.0;
    }
    if x == 1.0 {
        return 1.0;
    }
    if x <= 0.0 {
        return if x == 0.0 && y > 0.0 { 0.0 } else { f64::NAN };
    }
    exp(y * ln(x))
}

/// Winkel auf [0, 2π) normieren.
pub fn wrap_two_pi(a: f64) -> f64 {
    let tau = 2.0 * PI;
    let r = a - (a / tau).floor() * tau;
    if r >= tau { 0.0 } else { r }
}

/// Winkel auf (-π, π] normieren.
pub fn wrap_pi(a: f64) -> f64 {
    let r = wrap_two_pi(a + PI) - PI;
    if r <= -PI { r + 2.0 * PI } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, rel: f64) -> bool {
        let diff = (a - b).abs();
        diff <= rel * b.abs().max(1.0)
    }

    #[test]
    fn sin_cos_match_std() {
        for i in -20_000..20_000 {
            let x = f64::from(i) * 0.000_731 * 7.0;
            let (s, c) = sin_cos(x);
            assert!(close(s, x.sin(), 1e-15), "sin({x}) = {s} vs {}", x.sin());
            assert!(close(c, x.cos(), 1e-15), "cos({x}) = {c} vs {}", x.cos());
        }
    }

    #[test]
    fn atan2_matches_std() {
        for i in -300..300 {
            for j in -300..300 {
                let y = f64::from(i) * 0.37;
                let x = f64::from(j) * 0.29;
                let a = atan2(y, x);
                let b = y.atan2(x);
                if x == 0.0 && y == 0.0 {
                    continue;
                }
                assert!(close(a, b, 1e-15), "atan2({y}, {x}) = {a} vs {b}");
            }
        }
    }

    #[test]
    fn asin_acos_match_std() {
        for i in -1000..=1000 {
            let x = f64::from(i) / 1000.0;
            assert!(close(asin(x), x.asin(), 1e-14), "asin({x})");
            assert!(close(acos(x), x.acos(), 1e-14), "acos({x})");
        }
    }

    #[test]
    fn exp_ln_pow_match_std() {
        for i in -2000..2000 {
            let x = f64::from(i) * 0.173;
            assert!(close(exp(x), x.exp(), 1e-14), "exp({x}) = {} vs {}", exp(x), x.exp());
        }
        for i in 1..5000 {
            let x = f64::from(i) * 0.0137 + 1e-6;
            assert!(close(ln(x), x.ln(), 1e-15), "ln({x})");
            assert!(close(powf(x, 4.255_88), x.powf(4.255_88), 1e-13), "pow({x})");
        }
        assert!(close(ln(1e-310), (1e-310_f64).ln(), 1e-14));
        assert_eq!(ln(0.0), f64::NEG_INFINITY);
        assert!(ln(-1.0).is_nan());
    }

    #[test]
    fn wrapping() {
        assert!((wrap_two_pi(-0.1) - (2.0 * PI - 0.1)).abs() < 1e-12);
        assert!((wrap_pi(3.0 * PI / 2.0) + PI / 2.0).abs() < 1e-12);
        assert_eq!(wrap_pi(PI), PI);
        for i in -100..100 {
            let a = f64::from(i) * 0.77;
            let w = wrap_pi(a);
            assert!(w > -PI && w <= PI);
            let t = wrap_two_pi(a);
            assert!((0.0..2.0 * PI).contains(&t));
        }
    }
}
