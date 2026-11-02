//! Bewaffnungsbildschirm (M-036): oben Treibstoffbedarf und -vorrat, in der
//! Mitte groß die Unterseite der F-29 mit den vier Schächten, Nase nach
//! links (der Umriss folgt dem Grundriss des 3D-Modells und darf über den
//! Rand ragen), unten
//! die Liste in zwei Spalten. Welche Einträge es gibt und was eine Wahl
//! bewirkt, bestimmt das Frontend.

use crate::ScreenLayout;
use crate::canvas::{Align, DisplayList, Ink, Rect};
use crate::debrief::{TXT, footer};

/// Einträge je Spalte der Liste.
pub const ROWS: usize = 6;
const ROW: f32 = 0.04;
/// Bildbereich (Fensterhöhe = 1) und Maßstab (Bildschirm je Meter): Die
/// Schächte füllen 40 % der Höhe, darunter Platz für die Liste.
const PICTURE: [f32; 2] = [-0.13, 0.43];
const SCALE: f32 = 0.4 / (2.0 * (BAY_CENTER[0] + BAY_HALF[0]));
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
    Rect { min: [r.min[0] + 0.012, r.min[1] + TXT * 1.7], max: [r.max[0] - 0.012, r.max[1] - TXT * 1.7] }
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

pub fn draw(layout: &ScreenLayout, a: &Arming) -> DisplayList {
    let mut dl = DisplayList::new();
    let blink = if a.blink { Ink::HudWarning } else { Ink::Hud };
    dl.text_mid([0.0, 0.475], TXT, Align::Center, "ARMING", Ink::Hud);
    let (need, have) = a.fuel;
    let ink = if have < need { Ink::HudWarning } else { Ink::HudDim };
    let fuel = format!("FUEL REQUIRED {need:.0} KG    CARRIED {have:.0} KG");
    dl.text_mid([0.0, 0.447], TXT * 0.85, Align::Center, &fuel, ink);

    // Unterseite, auf den Bildbereich beschnitten
    let half = layout.aspect * 0.5;
    let area = Rect { min: [-half, PICTURE[0]], max: [half, PICTURE[1]] };
    let pt = |p: [f32; 2]| place(p, origin());
    dl.set_clip(Some(area));
    let side: Vec<[f32; 2]> = OUTLINE.iter().map(|&p| pt(p)).collect();
    let other: Vec<[f32; 2]> = OUTLINE.iter().rev().map(|&[x, z]| pt([-x, z])).collect();
    dl.polyline(&[side, other].concat(), true, Ink::HudDim);
    for k in 0..4 {
        let r = bay_rect(k);
        let c = r.center();
        let focus = a.bay == Some(k);
        let ink = if focus { blink } else { Ink::Hud };
        dl.rect(r, ink);
        if focus {
            dl.rect(Rect::from_center(c, [BAY_HALF[1] * SCALE + 0.008, BAY_HALF[0] * SCALE + 0.008]), ink);
        }
        dl.text_mid([c[0], r.max[1] - TXT], TXT * 0.8, Align::Center, &format!("BAY {}", k + 1), Ink::HudDim);
        let (name, count) = match &a.bays[k] {
            Some((n, c)) => (n.as_str(), format!("X{c}")),
            None => ("EMPTY", String::new()),
        };
        if !a.art[k] {
            dl.text_mid(c, TXT, Align::Center, name, ink);
        }
        dl.text_mid([c[0], r.min[1] + TXT], TXT, Align::Center, &count, ink);
    }
    dl.set_clip(None);

    // Liste in zwei Spalten
    for (k, item) in a.items.iter().enumerate() {
        let x = if k < ROWS { -0.62 } else { 0.08 };
        let y = PICTURE[0] - 0.04 - (k % ROWS) as f32 * ROW;
        let ink = match (k == a.cursor, a.bay) {
            (true, None) => blink,
            (true, Some(_)) => Ink::Hud,
            _ => Ink::HudDim,
        };
        dl.text_mid([x, y], TXT, Align::Left, item, ink);
    }
    let hint =
        if a.bay.is_some() { "ARROWS - BAY    SPACE - LOAD" } else { "ARROWS - SELECT    SPACE - TAKE" };
    footer(&mut dl, hint);
    dl
}
