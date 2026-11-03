//! Flugplätze: eigene Basen und feindliche Fliegerhorste (Jäger, M-022),
//! aus der Gebietsdatei.
//!
//! Das Gelände wird unter der Bahn auf eine Höhe eingeebnet und weich in die
//! Umgebung übergeblendet. Weil die Einebnung Teil der Höhenfunktion ist,
//! gelten Kollision, Radarhöhe und Darstellung automatisch auch für die Bahn.
//!
//! Bahnrichtungen sind auf 22,5°-Schritte beschränkt. Damit kommen die
//! Richtungsvektoren aus einer Tabelle exakter Konstanten, und das Crate
//! braucht keine Winkelfunktionen (bitgenauer Determinismus).

use crate::noise::{mix64, smoothstep};
use serde::{Deserialize, Serialize};

/// Art des Flugplatzes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AirfieldKind {
    /// Eigene Basis: lange, voll markierte Bahn mit Anflughilfe.
    Home,
    /// Feindlicher Fliegerhorst: Heimat der Jäger.
    Enemy,
}

/// Eine Start- und Landebahn.
#[derive(Debug, Clone, PartialEq)]
pub struct Runway {
    pub kind: AirfieldKind,
    pub name: &'static str,
    /// Mittelpunkt (Nord, Ost) in Metern.
    pub center: [f64; 2],
    /// Einheitsvektor entlang der Bahn (Nord, Ost).
    pub dir: [f64; 2],
    /// Richtung von `dir` in Grad, rechtweisend (0..180).
    pub heading_deg: f64,
    pub length_m: f64,
    pub width_m: f64,
    /// Höhe der eingeebneten Bahn über dem Meer.
    pub elevation_m: f64,
    /// Feindlicher Horst: Jäger MiG-21 und MiG-29 (M-022).
    pub fighters: [u8; 2],
}

/// Eben bleibender Rand seitlich der Bahn (m).
pub const FLAT_SIDE_M: f64 = 120.0;
/// Eben bleibender Rand vor und hinter den Bahnenden (m).
pub const FLAT_END_M: f64 = 200.0;
/// Breite des Übergangs vom ebenen Rand ins Gelände (m).
pub const BLEND_M: f64 = 600.0;

/// Bahnlänge und -breite je Art (m).
pub fn dimensions(kind: AirfieldKind) -> (f64, f64) {
    match kind {
        AirfieldKind::Home => (3_000.0, 45.0),
        AirfieldKind::Enemy => (2_500.0, 40.0),
    }
}
/// Bahnen nicht tiefer (Strand, Wasser) und nicht höher als das.
pub(crate) const MIN_SITE_M: f64 = 12.0;
const MAX_SITE_M: f64 = 900.0;

/// Richtungen in 22,5°-Schritten als (Nord, Ost).
const DIRECTIONS: [[f64; 2]; 8] = [
    [1.0, 0.0],
    [0.923_879_532_511_286_7, 0.382_683_432_365_089_8],
    [std::f64::consts::FRAC_1_SQRT_2, std::f64::consts::FRAC_1_SQRT_2],
    [0.382_683_432_365_089_8, 0.923_879_532_511_286_7],
    [0.0, 1.0],
    [-0.382_683_432_365_089_8, 0.923_879_532_511_286_7],
    [-std::f64::consts::FRAC_1_SQRT_2, std::f64::consts::FRAC_1_SQRT_2],
    [-0.923_879_532_511_286_7, 0.382_683_432_365_089_8],
];

impl Runway {
    /// Bahnrichtung im Bogenmaß.
    pub fn heading_rad(&self) -> f64 {
        self.heading_deg * (std::f64::consts::PI / 180.0)
    }

    /// Bahnkoordinaten: `u` entlang `dir`, `v` nach rechts quer dazu.
    pub fn local(&self, x: f64, y: f64) -> (f64, f64) {
        let dx = x - self.center[0];
        let dy = y - self.center[1];
        (dx * self.dir[0] + dy * self.dir[1], -dx * self.dir[1] + dy * self.dir[0])
    }

    /// Weltpunkt aus Bahnkoordinaten.
    pub fn world(&self, u: f64, v: f64) -> [f64; 2] {
        [
            self.center[0] + u * self.dir[0] - v * self.dir[1],
            self.center[1] + u * self.dir[1] + v * self.dir[0],
        ]
    }

    /// Ob der Punkt auf der befestigten Bahn liegt.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        let (u, v) = self.local(x, y);
        u.abs() <= self.length_m * 0.5 && v.abs() <= self.width_m * 0.5
    }

    /// Bahnnummern der beiden Enden (Richtung / 10°, 1…36).
    /// Das erste Ende ist das, von dem man in Richtung `dir` startet.
    pub fn numbers(&self) -> (u32, u32) {
        let num = |deg: f64| {
            let n = (deg / 10.0).round() as u32 % 36;
            if n == 0 { 36 } else { n }
        };
        (num(self.heading_deg), num(self.heading_deg + 180.0))
    }

    /// Gewicht der Einebnung (1 = ganz eben, 0 = unverändertes Gelände).
    fn flatten_weight(&self, x: f64, y: f64) -> Option<f64> {
        let reach = self.length_m * 0.5 + FLAT_END_M + BLEND_M;
        if (x - self.center[0]).abs() > reach || (y - self.center[1]).abs() > reach {
            return None;
        }
        let (u, v) = self.local(x, y);
        let du = (u.abs() - (self.length_m * 0.5 + FLAT_END_M)).max(0.0);
        let dv = (v.abs() - (self.width_m * 0.5 + FLAT_SIDE_M)).max(0.0);
        let d = (du * du + dv * dv).sqrt();
        if d >= BLEND_M { None } else { Some(1.0 - smoothstep(0.0, BLEND_M, d)) }
    }

    /// Ob der Punkt im voll eingeebneten Bereich um die Bahn liegt.
    pub fn in_flat_area(&self, x: f64, y: f64) -> bool {
        self.flatten_weight(x, y).is_some_and(|w| w >= 1.0)
    }
}

/// Wendet die Einebnung aller Bahnen auf eine Rohhöhe an.
pub(crate) fn flatten(runways: &[Runway], x: f64, y: f64, raw: f64) -> f64 {
    let mut h = raw;
    for rw in runways {
        if let Some(w) = rw.flatten_weight(x, y) {
            h += (rw.elevation_m - h) * w;
        }
    }
    h
}

/// Gleichverteilte Zahl 0..1 aus einem Hash.
fn unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1_u64 << 53) as f64
}

/// Bewertet einen Bauplatz: (Höhenspanne, mittlere Höhe) oder `None`, wenn
/// der Platz ungeeignet ist (Wasser, Strand, Hochgebirge).
fn evaluate(
    raw: &impl Fn(f64, f64) -> f64,
    c: [f64; 2],
    dir: [f64; 2],
    len: f64,
    width: f64,
) -> Option<(f64, f64)> {
    let half_u = len * 0.5 + FLAT_END_M;
    let half_v = width * 0.5 + FLAT_SIDE_M;
    let (mut min, mut max, mut sum, mut n) = (f64::MAX, f64::MIN, 0.0, 0.0);
    for i in 0..=10 {
        let u = -half_u + 2.0 * half_u * f64::from(i) / 10.0;
        for v in [-half_v, 0.0, half_v] {
            let x = c[0] + u * dir[0] - v * dir[1];
            let y = c[1] + u * dir[1] + v * dir[0];
            let h = raw(x, y);
            if !(MIN_SITE_M..=MAX_SITE_M).contains(&h) {
                return None;
            }
            min = min.min(h);
            max = max.max(h);
            sum += h;
            n += 1.0;
        }
    }
    Some((max - min, sum / n))
}

struct Site {
    center: [f64; 2],
    dir_index: usize,
    elevation: f64,
    cost: f64,
}

/// Sucht den günstigsten Bauplatz unter `count` Kandidaten.
fn best_site(
    raw: &impl Fn(f64, f64) -> f64,
    seed: u64,
    salt: u64,
    count: u64,
    candidate: impl Fn(u64, f64, f64) -> Option<([f64; 2], f64)>,
    len: f64,
    width: f64,
) -> Option<Site> {
    let mut best: Option<Site> = None;
    for k in 0..count {
        let h = mix64(seed ^ mix64(salt.wrapping_add(k)));
        let Some((center, extra_cost)) = candidate(k, unit(h), unit(mix64(h))) else {
            continue;
        };
        let ch = raw(center[0], center[1]);
        if !(MIN_SITE_M..=MAX_SITE_M).contains(&ch) {
            continue;
        }
        for (dir_index, dir) in DIRECTIONS.iter().enumerate() {
            if let Some((range, mean)) = evaluate(raw, center, *dir, len, width) {
                let cost = range + extra_cost;
                if best.as_ref().is_none_or(|b| cost < b.cost) {
                    best = Some(Site { center, dir_index, elevation: mean, cost });
                }
            }
        }
    }
    best
}

impl Runway {
    /// Bahn der Art `kind` bei `center`, Richtung `heading_deg` (gerundet auf
    /// 22,5°-Schritte), auf Höhe `elevation_m`.
    pub fn new(
        kind: AirfieldKind,
        name: &'static str,
        center: [f64; 2],
        heading_deg: f64,
        elevation_m: f64,
    ) -> Self {
        let dir_index = ((heading_deg / 22.5).round() as i64).rem_euclid(8) as usize;
        let (length_m, width_m) = dimensions(kind);
        Runway {
            kind,
            name,
            center,
            dir: DIRECTIONS[dir_index],
            heading_deg: dir_index as f64 * 22.5,
            length_m,
            width_m,
            elevation_m,
            fighters: [0, 0],
        }
    }
}

/// Sucht einen flachen Bauplatz für eine Bahn der Art `kind` im Umkreis
/// `radius` um `around` (`raw`: Geländehöhe ohne Einebnung). Liefert Mitte,
/// Richtung (Grad) und Höhe – für die Erstbefüllung von Gebieten.
pub fn find_site(
    raw: impl Fn(f64, f64) -> f64,
    seed: u64,
    kind: AirfieldKind,
    around: [f64; 2],
    radius: f64,
) -> Option<([f64; 2], f64, f64)> {
    let (len, width) = dimensions(kind);
    let site = best_site(
        &raw,
        seed,
        0x5349_5445,
        64,
        |k, a, b| {
            let c = if k == 0 {
                around
            } else {
                [around[0] + (a * 2.0 - 1.0) * radius, around[1] + (b * 2.0 - 1.0) * radius]
            };
            let (dx, dy) = (c[0] - around[0], c[1] - around[1]);
            Some((c, (dx * dx + dy * dy).sqrt() / 1_000.0))
        },
        len,
        width,
    )?;
    Some((site.center, site.dir_index as f64 * 22.5, site.elevation))
}
