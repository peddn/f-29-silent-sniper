//! Instrumentenbrett unter der 3D-Ansicht.

use crate::canvas::{Align, DisplayList, Ink, Rect};
use crate::{ScreenLayout, heading_deg};
use f29_core::FlightData;
use f29_core::units;
use std::f32::consts::FRAC_PI_2;

const TXT: f32 = 0.017;
const TXT_SMALL: f32 = 0.013;

pub(crate) fn draw(dl: &mut DisplayList, fd: &FlightData, layout: &ScreenLayout) {
    let panel = layout.panel_rect();
    let ph = panel.max[1] - panel.min[1];
    let cy = panel.center()[1] + ph * 0.04;
    let r = ph * 0.36;

    // Rahmen
    dl.line([panel.min[0], panel.max[1]], [panel.max[0], panel.max[1]], Ink::Panel);

    // Instrumentenabstand passt sich an schmale Fenster an.
    let half_w = layout.aspect * 0.5;
    let spacing = (r * 2.0 + 0.05).max((half_w * 0.5).min(r * 3.2));
    engine_gauge(dl, fd, [-spacing, cy], r);
    hsi(dl, fd, [0.0, cy], r);
    attitude_indicator(dl, fd, [spacing, cy], r);

    let left_x = -spacing - r - 0.035;
    let right_x = spacing + r + 0.035;
    if left_x - 0.16 > -half_w {
        left_column(dl, fd, [left_x, panel.max[1]], ph);
    }
    if right_x + 0.16 < half_w {
        right_column(dl, fd, [right_x, panel.max[1]], ph);
    }
}

/// Rundes Instrument: Skalenbogen 270° von unten links nach unten rechts.
fn dial_angle(t: f32) -> f32 {
    // t = 0 → 225°, t = 1 → −45°
    (225.0 - 270.0 * t.clamp(0.0, 1.0)).to_radians()
}

fn engine_gauge(dl: &mut DisplayList, fd: &FlightData, c: [f32; 2], r: f32) {
    dl.circle(c, r, 40, Ink::PanelDim);
    for i in 0..=10 {
        let a = dial_angle(i as f32 / 10.0);
        let len = if i % 2 == 0 { r * 0.18 } else { r * 0.09 };
        dl.line(
            [c[0] + (r - len) * a.cos(), c[1] + (r - len) * a.sin()],
            [c[0] + r * a.cos(), c[1] + r * a.sin()],
            Ink::Panel,
        );
        if i % 2 == 0 {
            let lr = r * 0.66;
            dl.text_mid(
                [c[0] + lr * a.cos(), c[1] + lr * a.sin()],
                TXT_SMALL * 0.8,
                Align::Center,
                &format!("{}", i * 10),
                Ink::PanelDim,
            );
        }
    }
    // Drehzahl in % (Leerlauf 55 %, voll 100 %)
    let rpm = 0.55 + 0.45 * fd.engine as f32;
    let a = dial_angle(rpm);
    dl.line(c, [c[0] + r * 0.85 * a.cos(), c[1] + r * 0.85 * a.sin()], Ink::Needle);
    // Schubhebelmarke
    let ta = dial_angle(0.55 + 0.45 * fd.throttle as f32);
    dl.line(
        [c[0] + r * 1.02 * ta.cos(), c[1] + r * 1.02 * ta.sin()],
        [c[0] + r * 1.14 * ta.cos(), c[1] + r * 1.14 * ta.sin()],
        Ink::Needle,
    );
    dl.text_mid([c[0], c[1] - r * 0.45], TXT_SMALL, Align::Center, "RPM", Ink::Panel);
    dl.text_mid(
        [c[0], c[1] - r * 0.75],
        TXT_SMALL,
        Align::Center,
        &format!("{:.0}", rpm * 100.0),
        Ink::Panel,
    );
}

/// Horizontal Situation Indicator: Kompassrose (Kurs oben) und Peilzeiger.
fn hsi(dl: &mut DisplayList, fd: &FlightData, c: [f32; 2], r: f32) {
    dl.circle(c, r, 48, Ink::PanelDim);
    let hdg = fd.heading as f32;
    // Bildschirmwinkel einer rechtweisenden Richtung (Kurs zeigt nach oben).
    let screen = |bearing: f32| FRAC_PI_2 - (bearing - hdg);
    for deg in (0..360).step_by(10) {
        let a = screen((deg as f32).to_radians());
        let len = if deg % 30 == 0 { r * 0.15 } else { r * 0.07 };
        dl.line(
            [c[0] + (r - len) * a.cos(), c[1] + (r - len) * a.sin()],
            [c[0] + r * a.cos(), c[1] + r * a.sin()],
            Ink::Panel,
        );
        if deg % 30 == 0 {
            let label = match deg {
                0 => "N".to_string(),
                90 => "E".to_string(),
                180 => "S".to_string(),
                270 => "W".to_string(),
                d => format!("{}", d / 10),
            };
            let lr = r * 0.70;
            dl.text_mid(
                [c[0] + lr * a.cos(), c[1] + lr * a.sin()],
                TXT_SMALL * 0.85,
                Align::Center,
                &label,
                Ink::Panel,
            );
        }
    }
    // Steuerstrich oben
    dl.line([c[0], c[1] + r * 1.02], [c[0], c[1] + r * 1.16], Ink::Needle);
    // Flugzeugsymbol
    let s = r * 0.12;
    dl.line([c[0] - s, c[1]], [c[0] + s, c[1]], Ink::Needle);
    dl.line([c[0], c[1] + s * 0.8], [c[0], c[1] - s * 1.2], Ink::Needle);
    dl.line([c[0] - s * 0.5, c[1] - s], [c[0] + s * 0.5, c[1] - s], Ink::Needle);

    if let Some(sp) = &fd.steerpoint {
        let a = screen(sp.bearing as f32);
        let (ca, sa) = (a.cos(), a.sin());
        let tip = [c[0] + r * 0.9 * ca, c[1] + r * 0.9 * sa];
        let tail = [c[0] - r * 0.9 * ca, c[1] - r * 0.9 * sa];
        dl.line([c[0] + r * 0.25 * ca, c[1] + r * 0.25 * sa], tip, Ink::Hud);
        dl.line(tail, [c[0] - r * 0.25 * ca, c[1] - r * 0.25 * sa], Ink::Hud);
        let n = [-sa, ca];
        let back = [tip[0] - ca * r * 0.15, tip[1] - sa * r * 0.15];
        dl.polyline(
            &[
                [back[0] + n[0] * r * 0.08, back[1] + n[1] * r * 0.08],
                tip,
                [back[0] - n[0] * r * 0.08, back[1] - n[1] * r * 0.08],
            ],
            true,
            Ink::Hud,
        );
        let txt = format!("{} {:.1}NM", sp.name, units::m_to_nm(sp.range));
        dl.text_mid([c[0], c[1] - r - TXT * 1.0], TXT_SMALL, Align::Center, &txt, Ink::Hud);
    }
    dl.text_mid(
        [c[0], c[1] + r + TXT * 1.3],
        TXT_SMALL,
        Align::Center,
        &format!("{:03}", heading_deg(fd.heading)),
        Ink::Panel,
    );
}

/// Künstlicher Horizont.
fn attitude_indicator(dl: &mut DisplayList, fd: &FlightData, c: [f32; 2], r: f32) {
    dl.circle(c, r, 40, Ink::PanelDim);
    let clip = Rect::from_center(c, [r * 0.72, r * 0.72]);
    let (sr, cr) = (fd.roll as f32).sin_cos();
    let along = [cr, sr];
    let up = [-sr, cr];
    // 90° Nicklage entsprechen dem Radius
    let scale = r / FRAC_PI_2 * 1.6;
    dl.set_clip(Some(clip));
    for deg in [-30, -20, -10, 0, 10, 20, 30] {
        let off = ((deg as f32).to_radians() - fd.pitch as f32) * scale;
        let m = [c[0] + up[0] * off, c[1] + up[1] * off];
        let half = if deg == 0 {
            r
        } else if deg % 20 == 0 {
            r * 0.35
        } else {
            r * 0.2
        };
        let ink = if deg == 0 { Ink::Panel } else { Ink::PanelDim };
        dl.line(
            [m[0] - along[0] * half, m[1] - along[1] * half],
            [m[0] + along[0] * half, m[1] + along[1] * half],
            ink,
        );
    }
    dl.set_clip(None);
    // feste Flugzeugreferenz
    let s = r * 0.3;
    dl.polyline(
        &[
            [c[0] - s, c[1]],
            [c[0] - s * 0.35, c[1]],
            [c[0], c[1] - s * 0.25],
            [c[0] + s * 0.35, c[1]],
            [c[0] + s, c[1]],
        ],
        false,
        Ink::Needle,
    );
    // Rollzeiger am Rand
    let a = FRAC_PI_2 + fd.roll as f32;
    let tip = [c[0] + r * 0.98 * a.cos(), c[1] + r * 0.98 * a.sin()];
    let b = [c[0] + r * 0.82 * a.cos(), c[1] + r * 0.82 * a.sin()];
    let n = [-a.sin(), a.cos()];
    dl.polyline(
        &[
            [b[0] + n[0] * r * 0.07, b[1] + n[1] * r * 0.07],
            tip,
            [b[0] - n[0] * r * 0.07, b[1] - n[1] * r * 0.07],
        ],
        true,
        Ink::Needle,
    );
    for deg in [-60.0_f32, -30.0, 0.0, 30.0, 60.0] {
        let a = (90.0 - deg).to_radians();
        dl.line(
            [c[0] + r * a.cos(), c[1] + r * a.sin()],
            [c[0] + r * 1.1 * a.cos(), c[1] + r * 1.1 * a.sin()],
            Ink::Panel,
        );
    }
    dl.text_mid([c[0], c[1] - r - TXT], TXT_SMALL, Align::Center, "ADI", Ink::Panel);
}

/// Linke Spalte: Schub, Luftbremse, g-Messer.
fn left_column(dl: &mut DisplayList, fd: &FlightData, top: [f32; 2], ph: f32) {
    let x = top[0];
    let mut y = top[1] - ph * 0.2;
    let step = ph * 0.17;
    let mut row = |dl: &mut DisplayList, txt: &str, ink: Ink| {
        dl.text_mid([x, y], TXT, Align::Right, txt, ink);
        y -= step;
    };
    row(dl, &format!("THR {:>3}%", (fd.throttle * 100.0).round() as i64), Ink::Panel);
    row(dl, &format!("G {:>4.1}", fd.load_factor), Ink::Panel);
    row(dl, &format!("AOA {:>4.1}", fd.alpha.to_degrees()), Ink::Panel);
    let brake_ink = if fd.airbrake > 0.05 { Ink::Warning } else { Ink::PanelDim };
    row(dl, "BRAKE", brake_ink);
}

/// Rechte Spalte: Missionszeit und Warnleuchten.
fn right_column(dl: &mut DisplayList, fd: &FlightData, top: [f32; 2], ph: f32) {
    let x = top[0];
    let mut y = top[1] - ph * 0.2;
    let step = ph * 0.17;
    let t = fd.time_s.max(0.0) as u64;
    dl.text_mid([x, y], TXT, Align::Left, &format!("T {:02}:{:02}", t / 60, t % 60), Ink::Panel);
    y -= step;
    let lamps =
        [("STALL", fd.warnings.stall), ("PULL UP", fd.warnings.pull_up), ("OVER G", fd.warnings.over_g)];
    for (txt, on) in lamps {
        let ink = if on { Ink::Warning } else { Ink::PanelDim };
        dl.text_mid([x, y], TXT, Align::Left, txt, ink);
        y -= step;
    }
}
