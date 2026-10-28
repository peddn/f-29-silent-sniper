//! Display-Liste und Zeichenprimitive.

use crate::font;

/// Logische Farbe; das Frontend ordnet sie einer Palettenfarbe zu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ink {
    /// HUD-Symbolik (klassisch grün).
    Hud,
    /// Abgeschwächte HUD-Symbolik.
    HudDim,
    /// Warnungen.
    Warning,
    /// Beschriftung und Skalen der Instrumente.
    Panel,
    /// Hilfslinien der Instrumente.
    PanelDim,
    /// Zeiger.
    Needle,
    /// MFD-Schrift und Kartenlinien (Küste).
    Mfd,
    /// Abgeschwächte MFD-Linien (Höhenlinien, Platzhalter).
    MfdDim,
    /// Kartengitter.
    MfdGrid,
}

/// Ein Liniensegment in Bildschirmeinheiten.
///
/// Bildschirmeinheiten: Fensterhöhe = 1, Ursprung in der Fenstermitte,
/// x nach rechts, y nach oben. x reicht von −aspect/2 bis +aspect/2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub ink: Ink,
}

/// Horizontale Textausrichtung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Achsenparalleles Rechteck (min/max).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

impl Rect {
    pub fn from_center(c: [f32; 2], half: [f32; 2]) -> Self {
        Self { min: [c[0] - half[0], c[1] - half[1]], max: [c[0] + half[0], c[1] + half[1]] }
    }

    pub fn contains(&self, p: [f32; 2]) -> bool {
        p[0] >= self.min[0] && p[0] <= self.max[0] && p[1] >= self.min[1] && p[1] <= self.max[1]
    }

    pub fn center(&self) -> [f32; 2] {
        [(self.min[0] + self.max[0]) * 0.5, (self.min[1] + self.max[1]) * 0.5]
    }
}

/// Liang-Barsky-Clipping eines Segments an einem Rechteck.
pub fn clip_segment(a: [f32; 2], b: [f32; 2], r: &Rect) -> Option<([f32; 2], [f32; 2])> {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let mut t0: f32 = 0.0;
    let mut t1: f32 = 1.0;
    let checks =
        [(-dx, a[0] - r.min[0]), (dx, r.max[0] - a[0]), (-dy, a[1] - r.min[1]), (dy, r.max[1] - a[1])];
    for (p, q) in checks {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some(([a[0] + t0 * dx, a[1] + t0 * dy], [a[0] + t1 * dx, a[1] + t1 * dy]))
}

/// Sammelt Linien, optional mit Clipping.
#[derive(Debug, Default, Clone)]
pub struct DisplayList {
    pub lines: Vec<Line>,
    clip: Option<Rect>,
}

impl DisplayList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Setzt (oder entfernt) das Clip-Rechteck für folgende Zeichenbefehle.
    pub fn set_clip(&mut self, clip: Option<Rect>) {
        self.clip = clip;
    }

    pub fn line(&mut self, a: [f32; 2], b: [f32; 2], ink: Ink) {
        if !(a[0].is_finite() && a[1].is_finite() && b[0].is_finite() && b[1].is_finite()) {
            return;
        }
        match self.clip {
            None => self.lines.push(Line { a, b, ink }),
            Some(r) => {
                if let Some((a, b)) = clip_segment(a, b, &r) {
                    self.lines.push(Line { a, b, ink });
                }
            }
        }
    }

    /// Gestrichelte Linie mit `dashes` Strichen.
    pub fn dashed(&mut self, a: [f32; 2], b: [f32; 2], dashes: u32, ink: Ink) {
        let n = dashes.max(1) * 2 - 1;
        for i in (0..n).step_by(2) {
            let t0 = i as f32 / n as f32;
            let t1 = (i + 1) as f32 / n as f32;
            self.line(lerp2(a, b, t0), lerp2(a, b, t1), ink);
        }
    }

    pub fn polyline(&mut self, pts: &[[f32; 2]], closed: bool, ink: Ink) {
        for w in pts.windows(2) {
            self.line(w[0], w[1], ink);
        }
        if closed && pts.len() > 2 {
            self.line(pts[pts.len() - 1], pts[0], ink);
        }
    }

    pub fn rect(&mut self, r: Rect, ink: Ink) {
        let [x0, y0] = r.min;
        let [x1, y1] = r.max;
        self.polyline(&[[x0, y0], [x1, y0], [x1, y1], [x0, y1]], true, ink);
    }

    /// Kreis(bogen) als Polygonzug. Winkel in rad, mathematisch positiv, 0 = rechts.
    pub fn arc(&mut self, c: [f32; 2], r: f32, from: f32, to: f32, segments: u32, ink: Ink) {
        let n = segments.max(1);
        let pts: Vec<[f32; 2]> = (0..=n)
            .map(|i| {
                let t = from + (to - from) * i as f32 / n as f32;
                [c[0] + r * t.cos(), c[1] + r * t.sin()]
            })
            .collect();
        self.polyline(&pts, false, ink);
    }

    pub fn circle(&mut self, c: [f32; 2], r: f32, segments: u32, ink: Ink) {
        self.arc(c, r, 0.0, std::f32::consts::TAU, segments, ink);
    }

    /// Text mit Zeichenhöhe `h`; `pos` ist die Grundlinie an der Ausrichtungskante.
    pub fn text(&mut self, pos: [f32; 2], h: f32, align: Align, text: &str, ink: Ink) {
        let w = font::text_width(text, h);
        let x0 = match align {
            Align::Left => pos[0],
            Align::Center => pos[0] - w * 0.5,
            Align::Right => pos[0] - w,
        };
        let s = h / font::GLYPH_H;
        for (i, c) in text.chars().enumerate() {
            let ox = x0 + i as f32 * font::ADVANCE * s;
            let segs = font::glyph(c).or_else(|| font::glyph('?')).unwrap_or(&[]);
            for &(ax, ay, bx, by) in segs {
                self.line([ox + ax * s, pos[1] + ay * s], [ox + bx * s, pos[1] + by * s], ink);
            }
        }
    }

    /// Text, vertikal um `pos[1]` zentriert.
    pub fn text_mid(&mut self, pos: [f32; 2], h: f32, align: Align, text: &str, ink: Ink) {
        self.text([pos[0], pos[1] - h * 0.5], h, align, text, ink);
    }

    /// Gerahmter Text (Anzeigebox).
    pub fn boxed_text(&mut self, pos: [f32; 2], h: f32, align: Align, text: &str, ink: Ink) {
        let w = font::text_width(text, h);
        let pad = h * 0.4;
        let x0 = match align {
            Align::Left => pos[0],
            Align::Center => pos[0] - w * 0.5,
            Align::Right => pos[0] - w,
        };
        self.text_mid([x0, pos[1]], h, Align::Left, text, ink);
        self.rect(
            Rect { min: [x0 - pad, pos[1] - h * 0.5 - pad], max: [x0 + w + pad, pos[1] + h * 0.5 + pad] },
            ink,
        );
    }
}

pub fn lerp2(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipping() {
        let r = Rect { min: [-1.0, -1.0], max: [1.0, 1.0] };
        assert_eq!(clip_segment([-2.0, 0.0], [2.0, 0.0], &r), Some(([-1.0, 0.0], [1.0, 0.0])));
        assert_eq!(clip_segment([-2.0, 2.0], [2.0, 2.0], &r), None);
        assert_eq!(clip_segment([0.0, 0.0], [0.5, 0.5], &r), Some(([0.0, 0.0], [0.5, 0.5])));
    }

    #[test]
    fn text_alignment() {
        let mut dl = DisplayList::new();
        dl.text([0.0, 0.0], 0.06, Align::Right, "10", Ink::Hud);
        let max_x = dl.lines.iter().flat_map(|l| [l.a[0], l.b[0]]).fold(f32::MIN, f32::max);
        assert!(max_x.abs() < 1e-6, "rechtsbündiger Text endet bei {max_x}");
    }
}
