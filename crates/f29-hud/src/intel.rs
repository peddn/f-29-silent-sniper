//! Lagebesprechung (M-036): nordausgerichtete Karte des Einsatzraums im
//! Bildfeld `area` des Frontends (Einheiten des 16:10-Bildschirms: Höhe = 1,
//! Mitte 0,0, y nach oben). Immer zu sehen: Basis, Route und die beiden
//! Ziele; je nach Thema hervorgehoben die Ziele, die Bedrohungen
//! (Radarkreise, gepunktet Impuls, durchgezogen Doppler; feindliche
//! Flugplätze) oder der Flugplan. Das Gelände legt das Frontend als
//! Rasterbild darunter ([`view`]), die Beschriftung setzt es in seiner Schrift
//! ([`labels`]).

use crate::canvas::{Align, DisplayList, Ink, Rect};
use crate::map::{MapData, MapProjection};
use crate::mfd::target_symbol;
use f29_core::RadarSiteInfo;
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

/// Kartenausschnitt: Mitte (Nord, Ost) und halbe Kantenlänge (m) – die Route
/// mit Rand.
pub fn view(map: &MapData) -> ([f64; 2], f64) {
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in &map.route {
        for k in 0..2 {
            lo[k] = lo[k].min(p.pos[k]);
            hi[k] = hi[k].max(p.pos[k]);
        }
    }
    let half = (hi[0] - lo[0]).max(hi[1] - lo[1]) * 0.6 + 20_000.0;
    ([(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5], half)
}

/// Nordausgerichtete Abbildung des Ausschnitts auf das Bildfeld.
fn projection(map: &MapData, area: Rect) -> MapProjection {
    let (center, half) = view(map);
    let size = (area.max[0] - area.min[0]).min(area.max[1] - area.min[1]);
    MapProjection {
        center_world: center,
        heading: 0.0,
        scale: f64::from(size * 0.5) / half,
        center_screen: area.center(),
    }
}

/// Größe der Symbole (Bildschirmeinheiten).
fn symbol(area: Rect) -> f32 {
    (area.max[1] - area.min[1]) * 0.03
}

pub fn draw(map: &MapData, sites: &[RadarSiteInfo], topic: Topic, area: Rect) -> DisplayList {
    let mut dl = DisplayList::new();
    let proj = projection(map, area);
    let s = symbol(area);
    dl.set_clip(Some(area));
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
pub fn labels(map: &MapData, topic: Topic, area: Rect) -> Vec<Label> {
    let proj = projection(map, area);
    let off = symbol(area) * 2.0;
    let mut out = Vec::new();
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
