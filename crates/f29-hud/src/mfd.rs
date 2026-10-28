//! Multifunktionsanzeigen (MFDs) im Instrumentenbrett.
//!
//! Links: Karte (Gitter oder Satellitenbild, Flugrichtung oben, stufenweise
//! zoombar). Rechts: Seiten mit Systeminformationen, zuerst „Ressourcen“
//! (Treibstoff, Waffenstationen). Dazwischen ein senkrechter Balken, der für
//! die Stealth- und Radarimpuls-Anzeige reserviert ist.

use crate::canvas::{Align, DisplayList, Ink, Rect};
use crate::map::{MapData, MapProjection};
use crate::{ScreenLayout, heading_deg};
use f29_core::FlightData;
use f29_core::terrain::AirfieldKind;
use f29_core::units;

/// Kartenbereiche: Entfernung von der Mitte bis zum Rand in Seemeilen.
pub const MAP_RANGES_NM: [f64; 5] = [2.0, 5.0, 10.0, 20.0, 40.0];
/// Anzahl Waffenstationen auf der Ressourcen-Seite.
pub const WEAPON_STATIONS: usize = 4;

/// Darstellungsart der Karte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapMode {
    /// Gitter mit Küstenlinie und Höhenlinien.
    #[default]
    Grid,
    /// Satellitenbild (Rasterbild, das das Frontend unter die Linien legt).
    Satellite,
}

/// Seiten des rechten MFDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightPage {
    /// Treibstoff und Waffenstationen.
    #[default]
    Resources,
}

/// Bedienzustand der MFDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MfdState {
    pub map_mode: MapMode,
    /// Index in [`MAP_RANGES_NM`].
    pub map_zoom: usize,
    pub right_page: RightPage,
}

impl Default for MfdState {
    fn default() -> Self {
        Self { map_mode: MapMode::Grid, map_zoom: 2, right_page: RightPage::Resources }
    }
}

impl MfdState {
    pub fn zoom_in(&mut self) {
        self.map_zoom = self.map_zoom.saturating_sub(1);
    }

    pub fn zoom_out(&mut self) {
        self.map_zoom = (self.map_zoom + 1).min(MAP_RANGES_NM.len() - 1);
    }

    pub fn toggle_map_mode(&mut self) {
        self.map_mode = match self.map_mode {
            MapMode::Grid => MapMode::Satellite,
            MapMode::Satellite => MapMode::Grid,
        };
    }

    /// Nächste Seite des rechten MFDs (bisher nur eine).
    pub fn next_page(&mut self) {
        self.right_page = match self.right_page {
            RightPage::Resources => RightPage::Resources,
        };
    }

    /// Kartenbereich (Mitte bis Rand) in Metern.
    pub fn map_range_m(&self) -> f64 {
        units::M_PER_NM * MAP_RANGES_NM[self.map_zoom.min(MAP_RANGES_NM.len() - 1)]
    }
}

/// Lage der MFDs im Instrumentenbrett (Bildschirmeinheiten).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MfdLayout {
    /// Gehäuse des linken MFDs.
    pub left: Rect,
    /// Bildfläche des linken MFDs (hier liegt das Satellitenbild).
    pub left_screen: Rect,
    /// Gehäuse des rechten MFDs.
    pub right: Rect,
    pub right_screen: Rect,
    /// Platz für die Stealth-/Radarimpuls-Anzeige zwischen den MFDs.
    pub center: Rect,
}

impl ScreenLayout {
    /// MFD-Anordnung: zwei quadratische MFDs, dazwischen ein schmaler Balken.
    /// `None`, wenn kein Instrumentenbrett angezeigt wird (z. B. Weitwinkel).
    pub fn mfd_layout(&self) -> Option<MfdLayout> {
        if self.panel_fraction <= 0.05 {
            return None;
        }
        let panel = self.panel_rect();
        let ph = panel.max[1] - panel.min[1];
        let cy = panel.center()[1];
        let width = self.aspect;
        let mut side = ph * 0.92;
        let mut strip = ph * 0.14;
        let mut gap = ph * 0.05;
        let total = 2.0 * side + strip + 2.0 * gap;
        if total > width * 0.96 {
            let k = width * 0.96 / total;
            side *= k;
            strip *= k;
            gap *= k;
        }
        let half = side * 0.5;
        let cx_left = -(strip * 0.5 + gap + half);
        let cx_right = -cx_left;
        let left = Rect::from_center([cx_left, cy], [half, half]);
        let right = Rect::from_center([cx_right, cy], [half, half]);
        let bezel = side * 0.05;
        let inset = |r: Rect| Rect {
            min: [r.min[0] + bezel, r.min[1] + bezel],
            max: [r.max[0] - bezel, r.max[1] - bezel],
        };
        let center = Rect::from_center([0.0, cy], [strip * 0.5, half - bezel * 0.5]);
        Some(MfdLayout { left, left_screen: inset(left), right, right_screen: inset(right), center })
    }
}

/// Zeichnet beide MFDs und den Mittelbalken.
pub(crate) fn draw(
    dl: &mut DisplayList,
    fd: &FlightData,
    layout: &ScreenLayout,
    state: &MfdState,
    map: Option<&MapData>,
) {
    let Some(m) = layout.mfd_layout() else {
        return;
    };
    // Trennlinie zwischen Ansicht und Brett
    let panel = layout.panel_rect();
    dl.line([panel.min[0], panel.max[1]], [panel.max[0], panel.max[1]], Ink::Panel);

    bezel(dl, m.left, m.left_screen);
    bezel(dl, m.right, m.right_screen);
    dl.set_clip(Some(m.left_screen));
    draw_map(dl, fd, m.left_screen, state, map);
    dl.set_clip(Some(m.right_screen));
    match state.right_page {
        RightPage::Resources => draw_resources(dl, fd, m.right_screen),
    }
    dl.set_clip(None);
    signature_placeholder(dl, m.center);
}

/// Gehäuse mit Bildschirmrahmen und je fünf Tasten oben und unten.
fn bezel(dl: &mut DisplayList, outer: Rect, screen: Rect) {
    dl.rect(outer, Ink::PanelDim);
    dl.rect(screen, Ink::Panel);
    let w = screen.max[0] - screen.min[0];
    let gap = outer.max[1] - screen.max[1];
    for k in 0..5 {
        let x = screen.min[0] + w * (k as f32 + 0.5) / 5.0;
        let bw = w * 0.07;
        for y in [outer.max[1] - gap * 0.5, outer.min[1] + gap * 0.5] {
            dl.line([x - bw, y], [x + bw, y], Ink::PanelDim);
        }
    }
}

/// Gute Gitterweite (m) für einen Kartenbereich.
fn grid_spacing(range_m: f64) -> f64 {
    let target = range_m / 2.5;
    [500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0, 20_000.0, 50_000.0]
        .into_iter()
        .find(|&s| s >= target)
        .unwrap_or(50_000.0)
}

/// Karte: Flugrichtung oben, Flugzeug in der Mitte.
fn draw_map(dl: &mut DisplayList, fd: &FlightData, screen: Rect, state: &MfdState, map: Option<&MapData>) {
    let half = (screen.max[0] - screen.min[0]) * 0.5;
    let c = screen.center();
    let txt = half * 0.075;
    let range = state.map_range_m();
    let proj = MapProjection {
        center_world: [fd.position.x, fd.position.y],
        heading: fd.heading,
        scale: f64::from(half) / range,
        center_screen: c,
    };
    // Weltpunkte bis zum Eckradius sind sichtbar.
    let reach = range * 1.45;
    let near = |p: [f64; 2]| (p[0] - fd.position.x).abs() < reach && (p[1] - fd.position.y).abs() < reach;

    match map {
        None => {
            dl.text_mid(c, txt * 1.3, Align::Center, "MAP LOADING", Ink::MfdDim);
        }
        Some(map) => {
            if state.map_mode == MapMode::Grid {
                // Gitter (nordausgerichtet in der Welt, dreht sich mit dem Kurs)
                let g = grid_spacing(range);
                let n0 = ((fd.position.x - reach) / g).floor() as i64;
                let n1 = ((fd.position.x + reach) / g).ceil() as i64;
                let e0 = ((fd.position.y - reach) / g).floor() as i64;
                let e1 = ((fd.position.y + reach) / g).ceil() as i64;
                let (lo_n, hi_n) = (n0 as f64 * g, n1 as f64 * g);
                let (lo_e, hi_e) = (e0 as f64 * g, e1 as f64 * g);
                for k in n0..=n1 {
                    let x = k as f64 * g;
                    dl.line(proj.to_screen([x, lo_e]), proj.to_screen([x, hi_e]), Ink::MfdGrid);
                }
                for k in e0..=e1 {
                    let y = k as f64 * g;
                    dl.line(proj.to_screen([lo_n, y]), proj.to_screen([hi_n, y]), Ink::MfdGrid);
                }
                for s in &map.contours {
                    if near(s.a) {
                        let ink = if s.level == 0 { Ink::Mfd } else { Ink::MfdDim };
                        dl.line(proj.to_screen(s.a), proj.to_screen(s.b), ink);
                    }
                }
            }
            // Flugplätze
            for rw in &map.runways {
                if !near(rw.center) {
                    continue;
                }
                let pts = rw.corners.map(|p| proj.to_screen(p));
                let ink = if rw.kind == AirfieldKind::Home { Ink::Panel } else { Ink::Needle };
                dl.polyline(&pts, true, ink);
                let pc = proj.to_screen(rw.center);
                dl.text_mid([pc[0] + txt * 0.8, pc[1] - txt * 1.4], txt * 0.8, Align::Left, rw.name, ink);
            }
            // Route
            let active = fd.steerpoint.as_ref().map(|s| s.index);
            let mut prev: Option<[f32; 2]> = None;
            for (k, p) in map.route.iter().enumerate() {
                let s = proj.to_screen(p.pos);
                if let Some(a) = prev {
                    dl.dashed(a, s, 6, Ink::Needle);
                }
                prev = Some(s);
                let r = txt * if Some(k) == active { 0.9 } else { 0.5 };
                if Some(k) == active {
                    dl.polyline(
                        &[[s[0], s[1] + r], [s[0] + r, s[1]], [s[0], s[1] - r], [s[0] - r, s[1]]],
                        true,
                        Ink::Needle,
                    );
                } else {
                    dl.circle(s, r, 8, Ink::Needle);
                }
                // Flugplätze sind schon beschriftet
                if !map.runways.iter().any(|rw| rw.name == p.name) {
                    dl.text_mid([s[0] + r + txt * 0.3, s[1]], txt * 0.75, Align::Left, &p.name, Ink::Needle);
                }
            }
        }
    }

    // Eigenes Flugzeug
    let s = txt * 0.9;
    dl.polyline(
        &[
            [c[0], c[1] + s * 1.3],
            [c[0] + s, c[1] - s * 0.8],
            [c[0], c[1] - s * 0.3],
            [c[0] - s, c[1] - s * 0.8],
        ],
        true,
        Ink::Hud,
    );
    // Nordpfeil am Rand
    let nd = proj.direction(0.0);
    let np = [c[0] + nd[0] * half * 0.86, c[1] + nd[1] * half * 0.86];
    dl.text_mid(np, txt, Align::Center, "N", Ink::Mfd);
    // Beschriftung
    let mode = match state.map_mode {
        MapMode::Grid => "GRID",
        MapMode::Satellite => "SAT",
    };
    let top = screen.max[1] - txt * 2.6;
    dl.text_mid([screen.min[0] + txt * 0.6, top], txt, Align::Left, mode, Ink::Mfd);
    let rng = format!("{}NM", MAP_RANGES_NM[state.map_zoom.min(MAP_RANGES_NM.len() - 1)]);
    dl.text_mid([screen.max[0] - txt * 0.6, top], txt, Align::Right, &rng, Ink::Mfd);
    dl.text_mid([c[0], top], txt, Align::Center, &format!("{:03}", heading_deg(fd.heading)), Ink::Mfd);
}

/// Ressourcen: Statuszeile, Waffenstationen (Platzhalter), Treibstoffbalken.
fn draw_resources(dl: &mut DisplayList, fd: &FlightData, screen: Rect) {
    let w = screen.max[0] - screen.min[0];
    let h = screen.max[1] - screen.min[1];
    // Zeichenvorschub = Zeichenhöhe: Größen so, dass alles in die Breite passt.
    let txt = w * 0.042;
    let small = w * 0.031;
    let x0 = screen.min[0] + w * 0.05;
    let x1 = screen.max[0] - w * 0.05;
    dl.text_mid([x0, screen.max[1] - txt * 1.2], txt, Align::Left, "RESOURCES", Ink::Mfd);

    // Statuszeile: Schub, Klappen, Fahrwerk, Bremse
    let y = screen.max[1] - h * 0.17;
    let blink = (fd.time_s * 2.0).fract() < 0.5;
    let gear = if fd.gear >= 0.999 {
        ("GEAR", if fd.warnings.gear_overspeed && blink { Ink::Warning } else { Ink::Needle })
    } else if fd.gear > 0.001 {
        ("GEAR", if blink { Ink::Warning } else { Ink::MfdDim })
    } else {
        ("GEAR", Ink::MfdDim)
    };
    let items = [
        (format!("THR{:>3}", (fd.throttle * 100.0).round() as i64), Ink::Mfd),
        ("FLAP".to_string(), if fd.flaps > 0.05 { Ink::Needle } else { Ink::MfdDim }),
        (gear.0.to_string(), gear.1),
        ("BRK".to_string(), if fd.airbrake > 0.05 { Ink::Warning } else { Ink::MfdDim }),
    ];
    for (k, (label, ink)) in items.iter().enumerate() {
        let x = x0 + (x1 - x0) * (k as f32 + 0.5) / items.len() as f32;
        dl.text_mid([x, y], small, Align::Center, label, *ink);
    }

    // Waffenstationen: leere Rahmen als Platzhalter
    let slot_top = screen.max[1] - h * 0.29;
    let slot_bottom = screen.min[1] + h * 0.33;
    let slot_w = (x1 - x0) / WEAPON_STATIONS as f32;
    for k in 0..WEAPON_STATIONS {
        let r = Rect {
            min: [x0 + slot_w * k as f32 + slot_w * 0.08, slot_bottom],
            max: [x0 + slot_w * (k + 1) as f32 - slot_w * 0.08, slot_top],
        };
        dl.rect(r, Ink::MfdDim);
        let cx = r.center()[0];
        dl.text_mid([cx, slot_top - txt * 1.1], txt, Align::Center, &format!("{}", k + 1), Ink::Mfd);
        dl.text_mid([cx, r.center()[1]], small, Align::Center, "EMPTY", Ink::MfdDim);
    }

    // Treibstoffbalken am unteren Rand
    let bar = Rect { min: [x0, screen.min[1] + h * 0.08], max: [x1, screen.min[1] + h * 0.15] };
    let low = fd.warnings.fuel_low || fd.warnings.bingo || fd.warnings.flameout;
    let fill_ink = if low { Ink::Warning } else { Ink::Needle };
    let frac = fd.fuel_fraction.clamp(0.0, 1.0) as f32;
    let fill_x = bar.min[0] + (bar.max[0] - bar.min[0]) * frac;
    let rows = 7;
    for r in 0..=rows {
        let yy = bar.min[1] + (bar.max[1] - bar.min[1]) * r as f32 / rows as f32;
        if frac > 0.0 {
            dl.line([bar.min[0], yy], [fill_x, yy], fill_ink);
        }
    }
    dl.rect(bar, Ink::Mfd);
    for q in 1..4 {
        let x = bar.min[0] + (bar.max[0] - bar.min[0]) * q as f32 / 4.0;
        dl.line([x, bar.min[1]], [x, bar.min[1] + (bar.max[1] - bar.min[1]) * 0.35], Ink::Mfd);
    }
    // BINGO-Marke: Bedarf bis nach Hause plus Reserve
    let bingo = (fd.bingo_fuel_kg / (fd.fuel_kg / fd.fuel_fraction.max(1e-9))).clamp(0.0, 1.0) as f32;
    if fd.fuel_fraction > 1e-9 && bingo > 0.0 {
        let x = bar.min[0] + (bar.max[0] - bar.min[0]) * bingo;
        dl.line([x, bar.min[1] - txt * 0.4], [x, bar.max[1] + txt * 0.4], Ink::Warning);
        dl.text_mid([x, bar.min[1] - txt * 1.0], txt * 0.7, Align::Center, "B", Ink::Warning);
    }
    let lb = (units::kg_to_lb(fd.fuel_kg) / 10.0).round() as i64 * 10;
    let label_y = bar.max[1] + txt * 1.1;
    let label_ink = if low { Ink::Warning } else { Ink::Mfd };
    dl.text_mid([x0, label_y], txt, Align::Left, &format!("FUEL {lb} LB"), label_ink);
    let min = (fd.endurance_s / 60.0).floor() as i64;
    dl.text_mid([x1, label_y], txt, Align::Right, &format!("{min} MIN"), label_ink);
}

/// Platzhalter für die Stealth- und Radarimpuls-Anzeige (kommt mit v0.2).
fn signature_placeholder(dl: &mut DisplayList, r: Rect) {
    let w = r.max[0] - r.min[0];
    let txt = w * 0.32;
    let bar = Rect {
        min: [r.min[0] + w * 0.25, r.min[1] + txt * 2.2],
        max: [r.max[0] - w * 0.25, r.max[1] - txt * 2.2],
    };
    dl.rect(r, Ink::PanelDim);
    dl.rect(bar, Ink::MfdDim);
    let cx = r.center()[0];
    for k in 1..10 {
        let y = bar.min[1] + (bar.max[1] - bar.min[1]) * k as f32 / 10.0;
        dl.line([bar.min[0], y], [bar.min[0] + (bar.max[0] - bar.min[0]) * 0.3, y], Ink::MfdDim);
    }
    dl.text_mid([cx, r.max[1] - txt * 1.1], txt, Align::Center, "SIG", Ink::Mfd);
    dl.text_mid([cx, r.min[1] + txt * 1.1], txt, Align::Center, "RDR", Ink::Mfd);
}
