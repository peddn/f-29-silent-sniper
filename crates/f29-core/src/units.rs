//! Umrechnungen für Anzeigen. Der Kern rechnet ausschließlich in SI.

pub const M_PER_FT: f64 = 0.304_8;
pub const MPS_PER_KT: f64 = 1_852.0 / 3_600.0;
pub const M_PER_NM: f64 = 1_852.0;

pub fn m_to_ft(m: f64) -> f64 {
    m / M_PER_FT
}

pub fn mps_to_kt(v: f64) -> f64 {
    v / MPS_PER_KT
}

pub fn mps_to_fpm(v: f64) -> f64 {
    v / M_PER_FT * 60.0
}

pub fn m_to_nm(m: f64) -> f64 {
    m / M_PER_NM
}

pub fn kt_to_mps(kt: f64) -> f64 {
    kt * MPS_PER_KT
}

pub fn ft_to_m(ft: f64) -> f64 {
    ft * M_PER_FT
}
