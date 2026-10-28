//! SVG-Export einer Display-Liste – zum Debuggen und für Dokumentation,
//! ganz ohne GPU (siehe `f29-headless hud-svg`).

use crate::{DisplayList, Ink, ScreenLayout};
use std::fmt::Write as _;

fn ink_color(ink: Ink) -> &'static str {
    match ink {
        Ink::Hud => "#55ff55",
        Ink::HudDim => "#22aa22",
        Ink::Warning => "#ff5555",
        Ink::Panel => "#cccccc",
        Ink::PanelDim => "#666677",
        Ink::Needle => "#ffff55",
    }
}

/// Rendert die Display-Liste als SVG mit `height` Pixeln Höhe.
pub fn to_svg(dl: &DisplayList, layout: &ScreenLayout, height: u32) -> String {
    let h = height as f32;
    let w = (layout.aspect * h).round();
    let to_px = |p: [f32; 2]| [(p[0] + layout.aspect * 0.5) * h, (0.5 - p[1]) * h];
    let mut s = String::new();
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#
    );
    let view_h = (1.0 - layout.panel_fraction) * h;
    let _ = writeln!(s, r##"<rect width="{w}" height="{view_h}" fill="#3a5f8f"/>"##);
    let _ = writeln!(s, r##"<rect y="{view_h}" width="{w}" height="{}" fill="#202024"/>"##, h - view_h);
    let stroke = (h / 450.0).max(1.0);
    let _ = writeln!(s, r#"<g stroke-width="{stroke:.2}" stroke-linecap="round" fill="none">"#);
    for l in &dl.lines {
        let a = to_px(l.a);
        let b = to_px(l.b);
        let _ = writeln!(
            s,
            r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}"/>"#,
            a[0],
            a[1],
            b[0],
            b[1],
            ink_color(l.ink)
        );
    }
    let _ = writeln!(s, "</g></svg>");
    s
}
