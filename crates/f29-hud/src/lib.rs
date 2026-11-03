//! HUD und Cockpit-Instrumente von F-29 Silent Sniper.
//!
//! Eingabe: [`FlightData`] aus dem Simulationskern und das Bildschirm-Layout.
//! Ausgabe: eine [`DisplayList`] aus Liniensegmenten in Bildschirmeinheiten
//! (Fensterhöhe = 1). Das Crate kennt keine Engine; das Frontend zeichnet nur
//! noch Linien. Dadurch ist das HUD ohne GPU testbar und in jeder Auflösung
//! gleich proportioniert.

pub mod arming;
pub mod canvas;
pub mod debrief;
pub mod font;
mod hud;
pub mod intel;
pub mod map;
pub mod mfd;
mod page;
mod pages;
pub mod svg;
mod throttle;

pub use canvas::{Align, DisplayList, Ink, Line, Rect};
use f29_core::FlightData;
use f29_core::systems::{Condition, System};
pub use hud::HudMode;
pub use map::MapData;
pub use mfd::{MapMode, MfdLayout, MfdState, RightPage};
pub use page::{MAX_OPTIONS, Page, Row};
pub use pages::target_image_rect;

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
    /// Standard-Höhe des Instrumentenbretts (mit den beiden MFDs).
    pub const DEFAULT_PANEL: f32 = 0.40;

    pub fn cockpit(aspect: f32) -> Self {
        Self { aspect, panel_fraction: Self::DEFAULT_PANEL, vertical_fov: Self::DEFAULT_FOV }
    }

    /// Weitwinkelsicht für den Luftkampf (M-050): ohne Brett, die 3D-Ansicht
    /// füllt das Fenster im selben Maßstab – man sieht nach unten weiter,
    /// HUD-Symbole behalten ihre Größe.
    pub fn wide(aspect: f32) -> Self {
        let tan = (Self::DEFAULT_FOV * 0.5).tan() / (1.0 - Self::DEFAULT_PANEL);
        Self { aspect, panel_fraction: 0.0, vertical_fov: 2.0 * tan.atan() }
    }

    /// 3D-Ansicht in Bildschirmeinheiten.
    pub fn view_rect(&self) -> Rect {
        Rect { min: [-self.aspect * 0.5, -0.5 + self.panel_fraction], max: [self.aspect * 0.5, 0.5] }
    }

    /// Instrumentenbrett in Bildschirmeinheiten.
    pub fn panel_rect(&self) -> Rect {
        Rect { min: [-self.aspect * 0.5, -0.5], max: [self.aspect * 0.5, -0.5 + self.panel_fraction] }
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
    /// HUD-Modus: Navigation, Luft-Boden, Luft-Luft.
    pub hud_mode: HudMode,
    /// Instrumentenbrett (MFDs) zeichnen.
    pub show_panel: bool,
    /// Bedienzustand der MFDs.
    pub mfd: MfdState,
    /// Das Frontend legt ein gerendertes Kamerabild in das Bildfeld der
    /// TGT-Seite ([`target_image_rect`]); das Drahtgitter entfällt dann.
    pub camera_feed: bool,
}

impl Default for HudOptions {
    fn default() -> Self {
        Self {
            show_hud: true,
            hud_mode: HudMode::default(),
            show_panel: true,
            mfd: MfdState::default(),
            camera_feed: false,
        }
    }
}

/// Erzeugt die komplette Display-Liste für einen Frame. `map` sind die
/// Kartendaten für das linke MFD (`None`, solange sie noch berechnet werden).
pub fn build(
    fd: &FlightData,
    layout: &ScreenLayout,
    opts: &HudOptions,
    map: Option<&MapData>,
) -> DisplayList {
    let mut dl = DisplayList::new();
    // Nach dem Missionsende zeigt das Frontend die Nachbesprechung ([`debrief::page`]).
    if fd.mission_over {
        return dl;
    }
    let on = displays_on(fd);
    // Am Schirm erscheint das HUD (leer) immer.
    if opts.show_hud && (on || fd.pilot.is_some()) {
        hud::draw(&mut dl, fd, layout, opts.hud_mode);
    }
    if opts.show_panel && layout.panel_fraction > 0.0 {
        dl.set_clip(None);
        mfd::draw(&mut dl, fd, layout, opts, map, on);
    }
    dl.set_clip(None);
    dl
}

/// Leuchten HUD und MFDs gerade? Beschädigte Anzeigen flackern unregelmäßig
/// (je weniger Leistung, desto öfter dunkel), ausgefallene bleiben dunkel.
/// Frontends blenden damit auch ihre eigenen Bildflächen (Satellitenbild,
/// Zielkamera) aus.
pub fn displays_on(fd: &FlightData) -> bool {
    match fd.systems.get(System::Displays) {
        Condition::Ok => true,
        Condition::Failed => false,
        Condition::Degraded(c) => {
            // Je Zwanzigstelsekunde ein fester Zufallswert (SplitMix64).
            let mut x = ((fd.time_s * 20.0) as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            let u = ((x ^ (x >> 31)) >> 11) as f64 / (1_u64 << 53) as f64;
            u < 0.35 + 0.65 * c.clamp(0.0, 1.0)
        }
    }
}

/// Formatiert einen Kurs als dreistellige Gradzahl („045“).
pub(crate) fn heading_deg(rad: f64) -> u32 {
    let d = rad.to_degrees().rem_euclid(360.0).round() as u32;
    if d == 360 { 0 } else { d }
}

#[cfg(test)]
mod tests;
