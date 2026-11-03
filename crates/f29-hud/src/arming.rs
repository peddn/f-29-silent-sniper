//! Bewaffnungsbildschirm (M-036): die Unterseite der F-29 mit den vier
//! Schächten, Nase nach links, ganz im Bildfeld `area` des Frontends
//! (Einheiten des 16:10-Bildschirms: Höhe = 1, Mitte 0,0, y nach oben). Der
//! Umriss folgt dem Grundriss des 3D-Modells; die Schächte liegen im breiten
//! hinteren Teil. Texte und Auswahl setzt das Frontend; welche Einträge es
//! gibt und was eine Wahl bewirkt, bestimmt es ebenfalls.

use crate::canvas::{DisplayList, Ink, Rect};

/// Einträge je Spalte der Liste (so viele Zeilen wie jede Auswahl).
pub const ROWS: usize = 4;
/// Grundriss der F-29 (rechte Hälfte, x quer, z längs, Nase in −z, Meter)
/// wie in `f29_render::model::aircraft`: Spannweite 13 m, Länge 16,5 m.
const OUTLINE: [[f32; 2]; 5] = [[0.0, -9.0], [6.5, 4.0], [3.0, 5.0], [2.2, 7.5], [0.0, 6.2]];
const SPAN: f32 = 13.0;
const NOSE: f32 = -9.0;
const TAIL: f32 = 7.5;
/// Ein Schacht (halbe Breite quer, halbe Länge) und die Mitte des vorderen
/// rechten (x, z; Meter); Abstand der vorderen und hinteren Schächte.
const BAY_HALF: [f32; 2] = [1.0, 1.9];
const BAY_CENTER: [f32; 2] = [1.2, -0.5];
const BAY_GAP: f32 = 0.4;

/// Inhalt des Bildschirms.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Arming {
    /// Je Schacht Kurzname und Anzahl.
    pub bays: [Option<(String, u8)>; 4],
    pub items: Vec<String>,
    /// Gewählter Listeneintrag.
    pub cursor: usize,
    /// Fokus auf einem Schacht (die Waffe am Cursor wird dort abgelegt).
    pub bay: Option<usize>,
    /// Treibstoff: benötigt, mitgeführt (kg).
    pub fuel: (f64, f64),
}

/// Maßstab (Einheiten je Meter) und Ursprung: der ganze Grundriss mittig im
/// Bildfeld, mit etwas Rand.
fn frame(area: Rect) -> (f32, [f32; 2]) {
    let size = [area.max[0] - area.min[0], area.max[1] - area.min[1]];
    let scale = (size[1] / SPAN).min(size[0] / (TAIL - NOSE)) * 0.92;
    let c = area.center();
    (scale, [c[0] - (NOSE + TAIL) * 0.5 * scale, c[1]])
}

/// Modellpunkt (x quer, z längs) → Bildschirm: um 90° gegen den
/// Uhrzeigersinn gedreht, die Nase zeigt nach links.
fn place([x, z]: [f32; 2], (scale, origin): (f32, [f32; 2])) -> [f32; 2] {
    [origin[0] + z * scale, origin[1] + x * scale]
}

/// Umriss des Schachts `k`: 1 vorn oben, 2 vorn unten, 3 hinten oben,
/// 4 hinten unten (so liest man sie im Bild).
pub fn bay_rect(k: usize, area: Rect) -> Rect {
    let x = if k.is_multiple_of(2) { BAY_CENTER[0] } else { -BAY_CENTER[0] };
    let z = BAY_CENTER[1] + (k / 2) as f32 * (2.0 * BAY_HALF[1] + BAY_GAP);
    let f = frame(area);
    Rect::from_center(place([x, z], f), [BAY_HALF[1] * f.0, BAY_HALF[0] * f.0])
}

/// Bildfeld für die Grafik der Ausrüstung im Schacht `k`.
pub fn art_rect(k: usize, area: Rect) -> Rect {
    let r = bay_rect(k, area);
    let m = 0.008;
    Rect { min: [r.min[0] + m, r.min[1] + m], max: [r.max[0] - m, r.max[1] - m] }
}

/// Umriss und Schächte; der Schacht im Fokus doppelt gerahmt.
pub fn draw(a: &Arming, area: Rect) -> DisplayList {
    let mut dl = DisplayList::new();
    let f = frame(area);
    let side: Vec<[f32; 2]> = OUTLINE.iter().map(|&p| place(p, f)).collect();
    let other: Vec<[f32; 2]> = OUTLINE.iter().rev().map(|&[x, z]| place([-x, z], f)).collect();
    dl.polyline(&[side, other].concat(), true, Ink::HudDim);
    for k in 0..4 {
        let r = bay_rect(k, area);
        dl.rect(r, Ink::Hud);
        if a.bay == Some(k) {
            let half = [(r.max[0] - r.min[0]) * 0.5 + 0.008, (r.max[1] - r.min[1]) * 0.5 + 0.008];
            dl.rect(Rect::from_center(r.center(), half), Ink::Hud);
        }
    }
    dl
}
