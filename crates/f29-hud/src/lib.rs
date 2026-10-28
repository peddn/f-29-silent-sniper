//! HUD und Cockpit-Instrumente von F-29 Silent Sniper.
//!
//! Eingabe: [`FlightData`] aus dem Simulationskern und das Bildschirm-Layout.
//! Ausgabe: eine [`DisplayList`] aus Liniensegmenten in Bildschirmeinheiten
//! (Fensterhöhe = 1). Das Crate kennt keine Engine; das Frontend zeichnet nur
//! noch Linien. Dadurch ist das HUD ohne GPU testbar und in jeder Auflösung
//! gleich proportioniert.

pub mod canvas;
pub mod font;
mod hud;
mod panel;
pub mod svg;

pub use canvas::{Align, DisplayList, Ink, Line, Rect};
use f29_core::FlightData;

/// Aufteilung des Fensters in 3D-Ansicht (oben) und Instrumentenbrett (unten).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenLayout {
    /// Fensterbreite / Fensterhöhe.
    pub aspect: f32,
    /// Anteil der Fensterhöhe, den das Instrumentenbrett einnimmt (0 = kein Brett).
    pub panel_fraction: f32,
    /// Vertikaler Sichtwinkel der 3D-Ansicht in rad.
    pub vertical_fov: f32,
}

impl ScreenLayout {
    /// Standard-Sichtwinkel der Cockpitansicht.
    pub const DEFAULT_FOV: f32 = 0.87; // ≈ 50°
    /// Standard-Höhe des Instrumentenbretts.
    pub const DEFAULT_PANEL: f32 = 0.27;

    pub fn cockpit(aspect: f32) -> Self {
        Self { aspect, panel_fraction: Self::DEFAULT_PANEL, vertical_fov: Self::DEFAULT_FOV }
    }

    /// 3D-Ansicht in Bildschirmeinheiten.
    pub fn view_rect(&self) -> Rect {
        Rect { min: [-self.aspect * 0.5, -0.5 + self.panel_fraction], max: [self.aspect * 0.5, 0.5] }
    }

    /// Instrumentenbrett in Bildschirmeinheiten.
    pub fn panel_rect(&self) -> Rect {
        Rect { min: [-self.aspect * 0.5, -0.5], max: [self.aspect * 0.5, -0.5 + self.panel_fraction] }
    }

    /// 3D-Ansicht als Anteil des Fensters, Ursprung oben links (für Viewports).
    /// Rückgabe: `(x, y, breite, höhe)` jeweils 0..1.
    pub fn view_viewport_fraction(&self) -> (f32, f32, f32, f32) {
        (0.0, 0.0, 1.0, 1.0 - self.panel_fraction)
    }

    /// Brennweite der Ansicht in Bildschirmeinheiten.
    pub fn focal(&self) -> f32 {
        let vh = 1.0 - self.panel_fraction;
        vh * 0.5 / (self.vertical_fov * 0.5).tan()
    }

    /// Projiziert eine Richtung in Körperachsen (x vorn, y rechts, z unten)
    /// auf den Bildschirm. `None`, wenn die Richtung nach hinten zeigt.
    pub fn project_body(&self, d: [f64; 3]) -> Option<[f32; 2]> {
        if d[0] <= 1e-6 {
            return None;
        }
        let f = self.focal();
        let c = self.view_rect().center();
        Some([c[0] + f * (d[1] / d[0]) as f32, c[1] - f * (d[2] / d[0]) as f32])
    }
}

/// Darstellungsoptionen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudOptions {
    /// HUD-Symbolik in der 3D-Ansicht zeichnen.
    pub show_hud: bool,
    /// Instrumentenbrett zeichnen.
    pub show_panel: bool,
}

impl Default for HudOptions {
    fn default() -> Self {
        Self { show_hud: true, show_panel: true }
    }
}

/// Erzeugt die komplette Display-Liste für einen Frame.
pub fn build(fd: &FlightData, layout: &ScreenLayout, opts: &HudOptions) -> DisplayList {
    let mut dl = DisplayList::new();
    if opts.show_hud {
        hud::draw(&mut dl, fd, layout);
    }
    if opts.show_panel && layout.panel_fraction > 0.0 {
        dl.set_clip(None);
        panel::draw(&mut dl, fd, layout);
    }
    dl.set_clip(None);
    dl
}

/// Formatiert einen Kurs als dreistellige Gradzahl („045“).
pub(crate) fn heading_deg(rad: f64) -> u32 {
    let d = rad.to_degrees().rem_euclid(360.0).round() as u32;
    if d == 360 { 0 } else { d }
}

#[cfg(test)]
mod tests;
