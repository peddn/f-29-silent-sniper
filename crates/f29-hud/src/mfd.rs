//! Multifunktionsanzeigen (MFDs) im Instrumentenbrett.
//!
//! Links: Karte (Gitter oder Satellitenbild, Flugrichtung oben, stufenweise
//! zoombar). Rechts: Seiten mit Systeminformationen, umschaltbar: Mission,
//! Ziel (optisches Zielbild), Waffen, Triebwerk mit Treibstoff, Reserve-Horizont und Systeme (siehe
//! `pages`). Dazwischen ein senkrechter Balken, der für die
//! Stealth- und Radarimpuls-Anzeige reserviert ist.

use crate::canvas::{Align, DisplayList, Ink, Rect};
use crate::map::{MapData, MapProjection};
use crate::{ScreenLayout, heading_deg};
use f29_core::FlightData;
use f29_core::targets::{TargetClass, TargetRole};
use f29_core::terrain::AirfieldKind;

/// Kartenbereiche: Entfernung von der Mitte bis zum Rand in Seemeilen.
/// Kartenbereiche (Mitte bis Rand) in km.
pub const MAP_RANGES_KM: [f64; 5] = [5.0, 10.0, 20.0, 40.0, 80.0];
/// Anzahl Waffenschächte.
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
    /// Mission: Ziele, Route, Heimweg.
    #[default]
    Mission,
    /// Aufgeschaltetes Ziel mit optischem Zielbild.
    Target,
    /// Waffenschächte und gewählte Waffe.
    Weapons,
    /// Drehzahl, Schub, Verbrauch, Treibstoff.
    Engine,
    /// Künstlicher Horizont (Reserveinstrument).
    Attitude,
    /// Konfiguration und Warnungen.
    Systems,
}

impl RightPage {
    /// Alle Seiten in Umschaltreihenfolge.
    pub const ALL: [RightPage; 6] = [
        RightPage::Mission,
        RightPage::Target,
        RightPage::Weapons,
        RightPage::Engine,
        RightPage::Attitude,
        RightPage::Systems,
    ];

    /// Kurzname in der Seitenleiste.
    pub fn label(self) -> &'static str {
        match self {
            RightPage::Mission => "MSN",
            RightPage::Target => "TGT",
            RightPage::Weapons => "WPN",
            RightPage::Engine => "ENG",
            RightPage::Attitude => "ADI",
            RightPage::Systems => "SYS",
        }
    }
}

/// Bedienzustand der MFDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MfdState {
    pub map_mode: MapMode,
    /// Index in [`MAP_RANGES_KM`].
    pub map_zoom: usize,
    pub right_page: RightPage,
}

impl Default for MfdState {
    fn default() -> Self {
        Self { map_mode: MapMode::Grid, map_zoom: 2, right_page: RightPage::Mission }
    }
}

impl MfdState {
    pub fn zoom_in(&mut self) {
        self.map_zoom = self.map_zoom.saturating_sub(1);
    }

    pub fn zoom_out(&mut self) {
        self.map_zoom = (self.map_zoom + 1).min(MAP_RANGES_KM.len() - 1);
    }

    /// Kartenmaßstab reihum (eine Taste): weiter, nach dem weitesten wieder
    /// der nächste.
    pub fn cycle_zoom(&mut self) {
        self.map_zoom = (self.map_zoom + 1) % MAP_RANGES_KM.len();
    }

    pub fn toggle_map_mode(&mut self) {
        self.map_mode = match self.map_mode {
            MapMode::Grid => MapMode::Satellite,
            MapMode::Satellite => MapMode::Grid,
        };
    }

    /// Nächste Seite des rechten MFDs (nach der letzten wieder die erste).
    pub fn next_page(&mut self) {
        let all = RightPage::ALL;
        let i = all.iter().position(|p| *p == self.right_page).unwrap_or(0);
        self.right_page = all[(i + 1) % all.len()];
    }

    /// Kartenbereich (Mitte bis Rand) in Metern.
    pub fn map_range_m(&self) -> f64 {
        1_000.0 * MAP_RANGES_KM[self.map_zoom.min(MAP_RANGES_KM.len() - 1)]
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
    /// Schubhebel ganz links im Brett.
    pub throttle: Rect,
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
        // Schubhebel: ganz links, so breit wie der Platz es zulässt (höchstens
        // 0,8 × so breit wie hoch).
        let margin = ph * 0.04;
        let free = (left.min[0] - margin) - (panel.min[0] + margin);
        let tw = free.min((half - bezel) * 1.6).max(0.0);
        let throttle = Rect {
            min: [panel.min[0] + margin, cy - half + bezel * 0.5],
            max: [panel.min[0] + margin + tw, cy + half - bezel * 0.5],
        };
        Some(MfdLayout {
            left,
            left_screen: inset(left),
            right,
            right_screen: inset(right),
            center,
            throttle,
        })
    }
}

/// Zeichnet beide MFDs und den Mittelbalken.
pub(crate) fn draw(
    dl: &mut DisplayList,
    fd: &FlightData,
    layout: &ScreenLayout,
    state: &MfdState,
    map: Option<&MapData>,
    camera_feed: bool,
    screens_on: bool,
) {
    let Some(m) = layout.mfd_layout() else {
        return;
    };
    // Trennlinie zwischen Ansicht und Brett
    let panel = layout.panel_rect();
    dl.line([panel.min[0], panel.max[1]], [panel.max[0], panel.max[1]], Ink::Panel);

    bezel(dl, m.left, m.left_screen);
    bezel(dl, m.right, m.right_screen);
    // Der Schubhebel ist mechanisch und bleibt auch bei dunklen Anzeigen sichtbar.
    if m.throttle.max[0] - m.throttle.min[0] > 0.03 {
        crate::throttle::draw(dl, fd, m.throttle);
    }
    if !screens_on {
        return;
    }
    dl.set_clip(Some(m.left_screen));
    draw_map(dl, fd, m.left_screen, state, map);
    dl.set_clip(Some(m.right_screen));
    let rs = m.right_screen;
    match state.right_page {
        RightPage::Mission => crate::pages::mission(dl, fd, rs, map),
        RightPage::Target => crate::pages::target(dl, fd, rs, camera_feed),
        RightPage::Weapons => crate::pages::weapons(dl, fd, rs),
        RightPage::Engine => crate::pages::engine(dl, fd, rs),
        RightPage::Attitude => crate::pages::attitude(dl, fd, rs),
        RightPage::Systems => crate::pages::systems(dl, fd, rs),
    }
    crate::pages::page_bar(dl, rs, state.right_page);
    dl.set_clip(None);
    signature(dl, fd, m.center);
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
            // Ziele
            let locked = fd.target.as_ref().map(|t| t.index);
            for (k, t) in map.targets.iter().enumerate() {
                // Zerstörte Ziele verschwinden von der Karte.
                if !near(t.pos) || fd.destroyed_targets.binary_search(&k).is_ok() {
                    continue;
                }
                let p = proj.to_screen(t.pos);
                let s = txt * 0.55;
                if fd.own_contacts.binary_search(&k).is_ok() {
                    // Eigene Erfassung: Zieltyp und Kennung „zivil“ bekannt.
                    let ink = if t.civilian { Ink::MfdDim } else { Ink::Warning };
                    target_symbol(dl, p, s, t.class, ink);
                } else if fd.awacs {
                    // AWACS-Datalink: Lage und Zieltyp, aber nicht „zivil“ –
                    // einheitlich gedämpft.
                    target_symbol(dl, p, s, t.class, Ink::MfdDim);
                } else {
                    // Weder eigene Erfassung noch AWACS: unbekannt (nur
                    // Primär-/Sekundärziel aus dem Briefing bleiben markiert).
                    if t.role == TargetRole::None {
                        continue;
                    }
                }
                let tag = match t.role {
                    TargetRole::Primary => Some("P"),
                    TargetRole::Secondary => Some("S"),
                    TargetRole::None => None,
                };
                if let Some(tag) = tag {
                    dl.circle(p, s * 2.2, 10, Ink::Warning);
                    dl.text_mid([p[0] + s * 2.6, p[1] + s * 1.6], txt * 0.75, Align::Left, tag, Ink::Warning);
                }
                if Some(k) == locked {
                    let b = s * 1.8;
                    let c = b * 0.45;
                    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                        let corner = [p[0] + sx * b, p[1] + sy * b];
                        dl.line(corner, [corner[0] - sx * c, corner[1]], Ink::Hud);
                        dl.line(corner, [corner[0], corner[1] - sy * c], Ink::Hud);
                    }
                }
            }
            // Radarstellungen, die das Flugzeug entdeckt haben (M-020):
            // Reichweitenkreis und Symbol, auch ohne eigene Erfassung oder
            // AWACS; Farbe nach Alarmstufe. Dazu der gemerkte Ping-Ort.
            for site in fd.threat.sites.iter().filter(|s| s.alert > 0) {
                let p = proj.to_screen(site.pos);
                let r = (site.range_m * proj.scale) as f32;
                let ink = match site.alert {
                    1 => Ink::MfdDim,
                    2 => Ink::Mfd,
                    _ => Ink::Warning,
                };
                dl.circle(p, r, 72, ink);
                if near(site.pos)
                    && let Some(t) = map.targets.get(site.target)
                {
                    target_symbol(dl, p, txt * 0.55, t.class, Ink::Warning);
                }
            }
            if let Some(pp) = fd.threat.ping_point {
                let p = proj.to_screen([pp.x, pp.y]);
                let k = txt * 0.45;
                dl.line([p[0] - k, p[1] - k], [p[0] + k, p[1] + k], Ink::Warning);
                dl.line([p[0] - k, p[1] + k], [p[0] + k, p[1] - k], Ink::Warning);
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

    // Waffen in der Luft: Raketen als kurzer Strich in Flugrichtung (Spitze
    // vorn), Bomben als Punkt; eigene in der Farbe des Flugzeugsymbols,
    // feindliche rot.
    for w in &fd.munitions {
        let pos = [w.position.x, w.position.y];
        if !near(pos) {
            continue;
        }
        let p = proj.to_screen(pos);
        if !screen.contains(p) {
            continue;
        }
        let ink = if w.hostile { Ink::Warning } else { Ink::Hud };
        let v = [w.velocity.x, w.velocity.y];
        let speed = v[0].hypot(v[1]);
        if w.missile && speed > 1.0 {
            let ahead =
                proj.to_screen([pos[0] + v[0] / speed * range * 0.1, pos[1] + v[1] / speed * range * 0.1]);
            let d = [ahead[0] - p[0], ahead[1] - p[1]];
            let len = d[0].hypot(d[1]).max(1e-6);
            let tail = [p[0] - d[0] / len * txt * 1.1, p[1] - d[1] / len * txt * 1.1];
            dl.line(tail, p, ink);
        } else {
            dl.circle(p, txt * 0.18, 6, ink);
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
    let rng = format!("{}KM", MAP_RANGES_KM[state.map_zoom.min(MAP_RANGES_KM.len() - 1)]);
    dl.text_mid([screen.max[0] - txt * 0.6, top], txt, Align::Right, &rng, Ink::Mfd);
    dl.text_mid([c[0], top], txt, Align::Center, &format!("{:03}", heading_deg(fd.heading)), Ink::Mfd);
}

/// Treibstoff als waagerechter Balken mit Viertelmarken und BINGO-Marke
/// (Bedarf bis nach Hause plus Reserve), darüber Menge und Restflugzeit.
/// `txt` ist die Schriftgröße der Beschriftung.
pub(crate) fn fuel_bar(dl: &mut DisplayList, fd: &FlightData, bar: Rect, txt: f32) {
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
    let bingo = (fd.bingo_fuel_kg / (fd.fuel_kg / fd.fuel_fraction.max(1e-9))).clamp(0.0, 1.0) as f32;
    if fd.fuel_fraction > 1e-9 && bingo > 0.0 {
        let x = bar.min[0] + (bar.max[0] - bar.min[0]) * bingo;
        dl.line([x, bar.min[1] - txt * 0.4], [x, bar.max[1] + txt * 0.4], Ink::Warning);
        dl.text_mid([x, bar.min[1] - txt * 1.0], txt * 0.7, Align::Center, "B", Ink::Warning);
    }
    let kg = (fd.fuel_kg / 10.0).round() as i64 * 10;
    let label_y = bar.max[1] + txt * 1.1;
    let label_ink = if low { Ink::Warning } else { Ink::Mfd };
    dl.text_mid([bar.min[0], label_y], txt, Align::Left, &format!("FUEL {kg} KG"), label_ink);
    let min = (fd.endurance_s / 60.0).floor() as i64;
    dl.text_mid([bar.max[0], label_y], txt, Align::Right, &format!("{min} MIN"), label_ink);
}

/// Kartensymbol einer Zielklasse um `p` mit Größe `s`.
pub(crate) fn target_symbol(dl: &mut DisplayList, p: [f32; 2], s: f32, class: TargetClass, ink: Ink) {
    let [x, y] = p;
    match class {
        TargetClass::Soft => dl.circle(p, s * 0.6, 6, ink),
        TargetClass::Armored => dl.rect(Rect::from_center(p, [s * 0.7, s * 0.5]), ink),
        TargetClass::Building => dl.polyline(
            &[
                [x - s * 0.7, y - s * 0.6],
                [x + s * 0.7, y - s * 0.6],
                [x + s * 0.7, y + s * 0.2],
                [x, y + s * 0.8],
                [x - s * 0.7, y + s * 0.2],
            ],
            true,
            ink,
        ),
        TargetClass::Hardened => {
            dl.arc([x, y - s * 0.5], s * 0.8, 0.0, std::f32::consts::PI, 6, ink);
            dl.line([x - s * 0.8, y - s * 0.5], [x + s * 0.8, y - s * 0.5], ink);
        }
        TargetClass::AirDefense => {
            dl.polyline(
                &[[x - s * 0.8, y - s * 0.6], [x + s * 0.8, y - s * 0.6], [x, y + s * 0.8]],
                true,
                ink,
            );
            dl.line([x, y - s * 0.6], [x, y + s * 1.3], ink);
        }
        TargetClass::EarlyWarning => {
            // Radarschirm auf einem Mast
            dl.arc([x, y + s * 0.2], s * 0.9, std::f32::consts::PI, std::f32::consts::TAU, 6, ink);
            dl.line([x - s * 0.9, y + s * 0.2], [x + s * 0.9, y + s * 0.2], ink);
            dl.line([x, y + s * 0.2], [x, y - s * 0.8], ink);
        }
        TargetClass::Bridge => {
            dl.line([x - s, y - s * 0.3], [x + s, y - s * 0.3], ink);
            dl.line([x - s, y + s * 0.3], [x + s, y + s * 0.3], ink);
        }
        TargetClass::ParkedAircraft => {
            dl.line([x - s * 0.9, y + s * 0.1], [x + s * 0.9, y + s * 0.1], ink);
            dl.line([x, y - s * 0.8], [x, y + s * 0.8], ink);
            dl.line([x - s * 0.35, y - s * 0.7], [x + s * 0.35, y - s * 0.7], ink);
        }
        TargetClass::ShipSmall | TargetClass::ShipLarge => {
            let w = if class == TargetClass::ShipLarge { s * 1.1 } else { s * 0.7 };
            dl.polyline(
                &[
                    [x - w, y + s * 0.2],
                    [x + w, y + s * 0.2],
                    [x + w * 0.6, y - s * 0.4],
                    [x - w * 0.6, y - s * 0.4],
                ],
                true,
                ink,
            );
            if class == TargetClass::ShipLarge {
                dl.line([x - w * 0.2, y + s * 0.2], [x - w * 0.2, y + s * 0.7], ink);
            }
        }
    }
}

/// Stealth-Anzeige zwischen den MFDs (M-020): links das eigene Stealth-Level,
/// rechts die Stärke des Impulses der gefährlichsten Radarstellung in
/// Reichweite. Ragt der Impuls über das Stealth-Level, wird der nächste Ping
/// dieser Stellung ein Treffer. Darüber drei Lampen für die höchste
/// Alarmstufe, unten der Radartyp (PLS Impuls, DOP Doppler).
fn signature(dl: &mut DisplayList, fd: &FlightData, r: Rect) {
    let w = r.max[0] - r.min[0];
    let txt = w * 0.32;
    let th = &fd.threat;
    let lamps_y = r.max[1] - txt * 2.6;
    let bar = Rect {
        min: [r.min[0] + w * 0.18, r.min[1] + txt * 2.2],
        max: [r.max[0] - w * 0.18, lamps_y - txt * 0.9],
    };
    dl.rect(r, Ink::PanelDim);
    dl.rect(bar, Ink::MfdDim);
    let cx = r.center()[0];
    let h = bar.max[1] - bar.min[1];
    let y_of = |v: f64| bar.min[1] + h * v.clamp(0.0, 1.0) as f32;
    // Füllung als waagerechte Linien (wie der Treibstoffbalken)
    let fill = |dl: &mut DisplayList, x0: f32, x1: f32, v: f64, ink: Ink| {
        let top = y_of(v);
        let mut y = bar.min[1] + h * 0.01;
        while y <= top {
            dl.line([x0, y], [x1, y], ink);
            y += h * 0.035;
        }
    };
    let gap = (bar.max[0] - bar.min[0]) * 0.1;
    let mid = cx;
    let signal = th.worst.map_or(0.0, |t| t.signal);
    let exposed = signal > th.stealth;
    fill(dl, bar.min[0] + gap * 0.5, mid - gap * 0.5, th.stealth, Ink::Mfd);
    fill(
        dl,
        mid + gap * 0.5,
        bar.max[0] - gap * 0.5,
        signal,
        if exposed { Ink::Warning } else { Ink::Needle },
    );
    // Letzter Ping: kurz eine helle Marke in seiner Stärke
    if let Some(p) = th.last_ping.filter(|p| p.age_s < 0.4) {
        let y = y_of(p.signal);
        dl.line(
            [bar.min[0] - w * 0.08, y],
            [bar.max[0] + w * 0.08, y],
            if p.hit { Ink::Warning } else { Ink::Mfd },
        );
    }
    // Alarmstufe
    let lamp = w * 0.22;
    for k in 0..3_u8 {
        let x = cx + (f32::from(k) - 1.0) * lamp * 1.35;
        let c = [x, lamps_y];
        let on = th.max_alert > k;
        let rr = Rect::from_center(c, [lamp * 0.5, lamp * 0.5]);
        dl.rect(rr, if on { Ink::Warning } else { Ink::MfdDim });
        if on {
            // gefüllt
            for f in 1..4 {
                let y = rr.min[1] + (rr.max[1] - rr.min[1]) * f as f32 / 4.0;
                dl.line([rr.min[0], y], [rr.max[0], y], Ink::Warning);
            }
        }
    }
    dl.text_mid([cx, r.max[1] - txt * 1.1], txt, Align::Center, "SIG", Ink::Mfd);
    let (kind, ink) = match th.worst.map(|t| t.kind) {
        Some(f29_core::radar::RadarKind::Pulse) => ("PLS", Ink::Mfd),
        Some(f29_core::radar::RadarKind::Doppler) => ("DOP", Ink::Mfd),
        None => ("RDR", Ink::MfdDim),
    };
    dl.text_mid([cx, r.min[1] + txt * 1.1], txt, Align::Center, kind, ink);
}
