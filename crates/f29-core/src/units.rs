//! Umrechnungen für Anzeigen. Der Kern rechnet ausschließlich in SI; die
//! Anzeigen sind metrisch (km, km/h, m, m/s, kg – Spielerentscheidung v0.2.7).
//! Die angloamerikanischen Einheiten bleiben für Werkzeuge und Tests.

pub const M_PER_FT: f64 = 0.304_8;
pub const MPS_PER_KT: f64 = 1_852.0 / 3_600.0;
pub const M_PER_NM: f64 = 1_852.0;
pub const KG_PER_LB: f64 = 0.453_592_37;

/// Geschwindigkeit in km/h.
pub fn mps_to_kmh(v: f64) -> f64 {
    v * 3.6
}

/// Strecke in km.
pub fn m_to_km(m: f64) -> f64 {
    m / 1_000.0
}

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

pub fn kg_to_lb(kg: f64) -> f64 {
    kg / KG_PER_LB
}
