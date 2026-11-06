//! Lagebesprechung (M-036): nordausgerichtete Karte des Einsatzraums im
//! Bildfeld `area` des Frontends (Einheiten des 16:10-Bildschirms: Höhe = 1,
//! Mitte 0,0, y nach oben). Immer zu sehen: Basis, Route und die beiden
//! Ziele; je nach Thema hervorgehoben die Ziele, die Bedrohungen
//! (Radarkreise, gepunktet Impuls, durchgezogen Doppler; feindliche
//! Flugplätze) oder der Flugplan. Darunter die Planquadrate (M-031): das
//! 100-km-Gitter mit den Buchstaben, eingezoomt auch die 10-km-Felder. Den
//! Ausschnitt ([`MapView`]) verschiebt
//! und zoomt das Frontend mit der Maus; das Gelände legt es als Rasterbild
//! darunter, die Beschriftung setzt es in seiner Schrift ([`labels`]).

use crate::canvas::{Align, DisplayList, Ink, Rect};
use crate::map::{MapData, MapProjection};
use crate::mfd::target_symbol;
use f29_core::RadarSiteInfo;
use f29_core::nav::grid_square;
use f29_core::radar::RadarKind;
use f29_core::targets::TargetRole;
use f29_core::terrain::AirfieldKind;

/// Themen der Lagebesprechung, in der Reihenfolge der Auswahl.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    Targets,
    Threats,
    FlightPlan,
    Leave,
}

impl Topic {
    pub const ALL: [Topic; 4] = [Topic::Targets, Topic::Threats, Topic::FlightPlan, Topic::Leave];

    pub fn label(self) -> &'static str {
        match self {
            Topic::Targets => "TARGETS",
            Topic::Threats => "THREATS",
            Topic::FlightPlan => "FLIGHT PLAN",
            Topic::Leave => "LEAVE BRIEFING",
        }
    }
}

/// Eine Beschriftung der Karte: Ecke unten am Text (Bildschirmeinheiten),
/// links bzw. rechts – in der rechten Kartenhälfte steht der Text links vom
/// Symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub pos: [f32; 2],
    pub align: Align,
    pub text: String,
    pub ink: Ink,
}

/// Engster Ausschnitt (halbe Kantenlänge, m): darunter würde das Höhenbild
/// (rund 120 m je Bildpunkt) nur noch gröber.
pub const MIN_HALF_M: f64 = 12_500.0;
/// Ab diesem Ausschnitt (halbe Kantenlänge, m) zeigt die Karte auch die
/// 10-km-Felder der Planquadrate.
const FINE_GRID_HALF_M: f64 = 60_000.0;

/// Kartenausschnitt: Mitte (Nord, Ost) und halbe Kantenlänge (m), immer im
/// Gebiet mit der halben Kantenlänge `bound` um den Ursprung.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapView {
    pub center: [f64; 2],
    pub half: f64,
}

impl MapView {
    /// Die Route mit Rand.
    pub fn fit(map: &MapData, bound: f64) -> Self {
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for p in &map.route {
            for k in 0..2 {
                lo[k] = lo[k].min(p.pos[k]);
                hi[k] = hi[k].max(p.pos[k]);
            }
        }
        let half = (hi[0] - lo[0]).max(hi[1] - lo[1]) * 0.6 + 20_000.0;
        MapView { center: [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5], half }.within(bound)
    }

    /// Meter je Bildschirmeinheit im Bildfeld.
    fn m_per_unit(&self, area: Rect) -> f64 {
        self.half / f64::from(size(area) * 0.5)
    }

    /// Weltpunkt (Nord, Ost) unter dem Punkt `p` des Bildfelds.
    pub fn world(&self, area: Rect, p: [f32; 2]) -> [f64; 2] {
        let (c, m) = (area.center(), self.m_per_unit(area));
        [self.center[0] + f64::from(p[1] - c[1]) * m, self.center[1] + f64::from(p[0] - c[0]) * m]
    }

    /// Um `factor` zoomen (kleiner 1: hinein); der Weltpunkt unter `at`
    /// bleibt stehen.
    pub fn zoom(self, area: Rect, at: [f32; 2], factor: f64, bound: f64) -> Self {
        let w = self.world(area, at);
        let half = (self.half * factor).clamp(MIN_HALF_M, bound);
        let k = half / self.half;
        let center = [w[0] + (self.center[0] - w[0]) * k, w[1] + (self.center[1] - w[1]) * k];
        MapView { center, half }.within(bound)
    }

    /// Um `delta` (Bildschirmeinheiten) verschieben: Die Karte folgt dem Zeiger.
    pub fn drag(self, area: Rect, delta: [f32; 2], bound: f64) -> Self {
        let m = self.m_per_unit(area);
        let center = [self.center[0] - f64::from(delta[1]) * m, self.center[1] - f64::from(delta[0]) * m];
        MapView { center, ..self }.within(bound)
    }

    /// Ganz im Gebiet.
    fn within(self, bound: f64) -> Self {
        let half = self.half.clamp(MIN_HALF_M, bound);
        let lim = bound - half;
        MapView { center: self.center.map(|c| c.clamp(-lim, lim)), half }
    }

    /// Nordausgerichtete Abbildung auf das Bildfeld.
    fn projection(&self, area: Rect) -> MapProjection {
        MapProjection {
            center_world: self.center,
            heading: 0.0,
            scale: 1.0 / self.m_per_unit(area),
            center_screen: area.center(),
        }
    }
}

/// Kantenlänge des quadratischen Bildfelds.
fn size(area: Rect) -> f32 {
    (area.max[0] - area.min[0]).min(area.max[1] - area.min[1])
}

/// Größe der Symbole (Bildschirmeinheiten).
fn symbol(area: Rect) -> f32 {
    (area.max[1] - area.min[1]) * 0.03
}

pub fn draw(map: &MapData, sites: &[RadarSiteInfo], topic: Topic, view: MapView, area: Rect) -> DisplayList {
    let mut dl = DisplayList::new();
    let proj = view.projection(area);
    let s = symbol(area);
    dl.set_clip(Some(area));
    let h = map.half_m;
    let fine = view.half < FINE_GRID_HALF_M;
    for (step, ink) in
        [(10_000.0, Ink::MfdGrid), (100_000.0, Ink::MfdDim)].into_iter().filter(|g| fine || g.0 > 10_000.0)
    {
        for k in (-h / step).ceil() as i64..=(h / step).floor() as i64 {
            let x = k as f64 * step;
            dl.line(proj.to_screen([x, -h]), proj.to_screen([x, h]), ink);
            dl.line(proj.to_screen([-h, x]), proj.to_screen([h, x]), ink);
        }
    }
    for rw in &map.runways {
        let threat = rw.kind == AirfieldKind::Enemy;
        if !threat || topic == Topic::Threats {
            dl.polyline(
                &rw.corners.map(|p| proj.to_screen(p)),
                true,
                if threat { Ink::Warning } else { Ink::Panel },
            );
        }
    }
    if topic == Topic::Threats {
        for site in sites {
            let (c, r) = (proj.to_screen(site.pos), (site.range_m * proj.scale) as f32);
            match site.kind {
                RadarKind::Doppler => dl.circle(c, r, 64, Ink::Warning),
                RadarKind::Pulse => (0..64).step_by(2).for_each(|k| {
                    let at = |k: u32| {
                        let a = k as f32 / 64.0 * std::f32::consts::TAU;
                        [c[0] + r * a.cos(), c[1] + r * a.sin()]
                    };
                    dl.line(at(k), at(k + 1), Ink::Warning);
                }),
            }
            if let Some(t) = map.targets.get(site.target) {
                target_symbol(&mut dl, c, s, t.class, Ink::Warning);
            }
        }
    }
    let plan = topic == Topic::FlightPlan;
    for w in map.route.windows(2) {
        let (a, b) = (proj.to_screen(w[0].pos), proj.to_screen(w[1].pos));
        if plan { dl.line(a, b, Ink::Needle) } else { dl.dashed(a, b, 12, Ink::MfdDim) }
    }
    if plan {
        for p in &map.route {
            dl.circle(proj.to_screen(p.pos), s * 0.5, 8, Ink::Needle);
        }
    }
    let ink = if topic == Topic::Targets { Ink::Warning } else { Ink::MfdDim };
    for t in map.targets.iter().filter(|t| t.role != TargetRole::None) {
        let c = proj.to_screen(t.pos);
        dl.circle(c, s * 1.6, 12, ink);
        if topic == Topic::Targets {
            target_symbol(&mut dl, c, s, t.class, ink);
        }
    }
    dl.set_clip(None);
    dl
}

/// Beschriftungen zum Thema, nur innerhalb des Bildfelds.
pub fn labels(map: &MapData, topic: Topic, view: MapView, area: Rect) -> Vec<Label> {
    let proj = view.projection(area);
    let off = symbol(area) * 2.0;
    let mut out = Vec::new();
    // Buchstaben der 100-km-Quadrate an ihrer Südwestecke (im Gebiet)
    let h = map.half_m;
    let corners =
        || ((-h / 100_000.0).floor() as i64..(h / 100_000.0).ceil() as i64).map(|k| (k as f64 * 1e5).max(-h));
    for n in corners() {
        for e in corners() {
            let [x, y] = proj.to_screen([n, e]);
            let pos = [x + off * 0.3, y + off * 0.3];
            if area.contains(pos) {
                let text = grid_square(n + 1.0, e + 1.0, h)[..2].to_string();
                out.push(Label { pos, align: Align::Left, text, ink: Ink::MfdDim });
            }
        }
    }
    let mut add = |p: [f64; 2], text: String, ink: Ink| {
        let [x, y] = proj.to_screen(p);
        let right = x > area.center()[0];
        let (dx, align) = if right { (-off, Align::Right) } else { (off, Align::Left) };
        if area.contains([x, y]) {
            out.push(Label { pos: [x + dx, y - off * 0.5], align, text, ink });
        }
    };
    for rw in &map.runways {
        let threat = rw.kind == AirfieldKind::Enemy;
        if !threat || topic == Topic::Threats {
            add(rw.center, rw.name.to_string(), if threat { Ink::Warning } else { Ink::Panel });
        }
    }
    match topic {
        Topic::Targets => {
            for t in map.targets.iter() {
                let tag = match t.role {
                    TargetRole::Primary => "P",
                    TargetRole::Secondary => "S",
                    TargetRole::None => continue,
                };
                add(t.pos, format!("{tag} {}", t.name), Ink::Warning);
            }
        }
        Topic::FlightPlan => {
            let last = map.route.len().saturating_sub(1);
            for (k, p) in map.route.iter().enumerate().filter(|&(k, _)| k > 0 && k < last) {
                add(p.pos, (k + 1).to_string(), Ink::Needle);
            }
        }
        Topic::Threats | Topic::Leave => {}
    }
    out
}
