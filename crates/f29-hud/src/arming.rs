//! Bewaffnungsbildschirm (M-036): oben Treibstoffbedarf und -vorrat, in der
//! Mitte groß die Unterseite der F-29 mit den vier Schächten, Nase nach
//! links (der Umriss folgt dem Grundriss des 3D-Modells und darf über den
//! Rand ragen), unten die Liste in zwei Spalten. Hier entstehen Umriss und
//! Schächte als Linien in Einheiten des 16:10-Bildschirms (Höhe = 1, Mitte
//! 0,0, y nach oben); die Texte setzt das Frontend in [`bay_rect`] und unter
//! [`PICTURE`]. Welche Einträge es gibt und was eine Wahl bewirkt, bestimmt
//! das Frontend.

use crate::canvas::{DisplayList, Ink, Rect};

/// Einträge je Spalte der Liste.
pub const ROWS: usize = 6;
/// Bildbereich (unten, oben) zwischen Kopfzeilen und Liste; die Schächte
/// füllen 40 % der Höhe.
pub const PICTURE: [f32; 2] = [-0.23, 0.285];
const SCALE: f32 = 0.4 / (2.0 * (BAY_CENTER[0] + BAY_HALF[0]));
/// Halbe Breite des Bildschirms.
const HALF_WIDTH: f32 = 0.8;
/// Platz für die Beschriftung oben und unten im Schacht.
const LABEL: f32 = 0.045;
/// Grundriss der F-29 (rechte Hälfte, x quer, z längs, Nase in −z, Meter)
/// wie in `f29_render::model::aircraft`.
const OUTLINE: [[f32; 2]; 5] = [[0.0, -9.0], [6.5, 4.0], [3.0, 5.0], [2.2, 7.5], [0.0, 6.2]];
/// Ein Schacht (halbe Breite, halbe Länge) und die Mitte des vorderen
/// rechten (x, z; Meter).
const BAY_HALF: [f32; 2] = [1.15, 1.45];
const BAY_CENTER: [f32; 2] = [1.3, -1.9];
/// Abstand der Schachtreihen (m).
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
    /// Takt des Blinkens.
    pub blink: bool,
    /// Treibstoff: benötigt, mitgeführt (kg).
    pub fuel: (f64, f64),
    /// Je Schacht: Das Frontend zeigt eine Grafik ([`art_rect`]) statt des
    /// Kurznamens.
    pub art: [bool; 4],
}

/// Ursprung des Modells auf dem Bildschirm: Die Schächte liegen mittig im
/// Bildbereich.
fn origin() -> [f32; 2] {
    let middle = BAY_CENTER[1] + BAY_HALF[1] + BAY_GAP * 0.5;
    [-middle * SCALE, (PICTURE[0] + PICTURE[1]) * 0.5]
}

/// Umriss des Schachts `k` (Bildschirmeinheiten).
pub fn bay_rect(k: usize) -> Rect {
    Rect::from_center(place(bay_center(k), origin()), [BAY_HALF[1] * SCALE, BAY_HALF[0] * SCALE])
}

/// Bildfeld für die Grafik der Ausrüstung im Schacht `k`: zwischen der
/// Nummer oben und der Stückzahl unten.
pub fn art_rect(k: usize) -> Rect {
    let r = bay_rect(k);
    Rect { min: [r.min[0] + 0.012, r.min[1] + LABEL], max: [r.max[0] - 0.012, r.max[1] - LABEL] }
}

/// Modellpunkt (x quer, z längs) → Bildschirm: um 90° gegen den
/// Uhrzeigersinn gedreht, die Nase zeigt nach links.
fn place([x, z]: [f32; 2], origin: [f32; 2]) -> [f32; 2] {
    [origin[0] + z * SCALE, origin[1] + x * SCALE]
}

/// Mitte des Schachts `k` im Modell: 1 vorn oben, 2 vorn unten, 3 hinten
/// oben, 4 hinten unten (so liest man sie im Bild).
fn bay_center(k: usize) -> [f32; 2] {
    let x = if k.is_multiple_of(2) { BAY_CENTER[0] } else { -BAY_CENTER[0] };
    [x, BAY_CENTER[1] + (k / 2) as f32 * (2.0 * BAY_HALF[1] + BAY_GAP)]
}

/// Umriss und Schächte; der Schacht im Fokus blinkt zwischen zwei Farben.
pub fn draw(a: &Arming) -> DisplayList {
    let mut dl = DisplayList::new();
    let blink = if a.blink { Ink::HudWarning } else { Ink::Hud };
    // Unterseite, auf den Bildbereich beschnitten
    dl.set_clip(Some(Rect { min: [-HALF_WIDTH, PICTURE[0]], max: [HALF_WIDTH, PICTURE[1]] }));
    let pt = |p: [f32; 2]| place(p, origin());
    let side: Vec<[f32; 2]> = OUTLINE.iter().map(|&p| pt(p)).collect();
    let other: Vec<[f32; 2]> = OUTLINE.iter().rev().map(|&[x, z]| pt([-x, z])).collect();
    dl.polyline(&[side, other].concat(), true, Ink::HudDim);
    for k in 0..4 {
        let r = bay_rect(k);
        let focus = a.bay == Some(k);
        dl.rect(r, if focus { blink } else { Ink::Hud });
        if focus {
            dl.rect(
                Rect::from_center(r.center(), [BAY_HALF[1] * SCALE + 0.008, BAY_HALF[0] * SCALE + 0.008]),
                blink,
            );
        }
    }
    dl.set_clip(None);
    dl
}
