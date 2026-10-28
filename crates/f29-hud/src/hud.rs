//! Head-up-Display in der 3D-Ansicht.

use crate::canvas::{Align, DisplayList, Ink, Rect};
use crate::{ScreenLayout, heading_deg};
use f29_core::FlightData;
use f29_core::units;

/// Zeichenhöhe der HUD-Schrift (Bildschirmeinheiten).
const TXT: f32 = 0.020;
const TXT_SMALL: f32 = 0.016;

pub(crate) fn draw(dl: &mut DisplayList, fd: &FlightData, layout: &ScreenLayout) {
    let view = layout.view_rect();
    let vh = view.max[1] - view.min[1];
    let c = view.center();
    // Das HUD-Glas: etwa 60 % der Ansichtshöhe, leicht breiter als hoch.
    let glass = Rect::from_center(c, [vh * 0.36, vh * 0.34]);

    if fd.warnings.crashed {
        dl.text_mid(c, TXT * 2.2, Align::Center, "CRASHED", Ink::Warning);
        dl.text_mid([c[0], c[1] - TXT * 3.0], TXT, Align::Center, "R - RESTART", Ink::Warning);
        return;
    }

    pitch_ladder(dl, fd, layout, &glass);
    boresight(dl, c, vh);
    flight_path_marker(dl, fd, layout, &glass, vh);
    steerpoint_marker(dl, fd, layout, &glass, vh);
    heading_tape(dl, fd, &glass);
    bank_scale(dl, fd, &glass);
    left_data(dl, fd, &glass);
    right_data(dl, fd, &glass);
    warnings(dl, fd, &glass);
}

/// Nicklage-Leiter: Sprossen alle 5°, konform zur Außenwelt.
fn pitch_ladder(dl: &mut DisplayList, fd: &FlightData, layout: &ScreenLayout, glass: &Rect) {
    let f = layout.focal();
    let c = glass.center();
    let (sr, cr) = (fd.roll as f32).sin_cos();
    let along = [cr, sr]; // Richtung des Horizonts auf dem Schirm
    let up = [-sr, cr];
    let pitch = fd.pitch as f32;

    // Die Leiter bleibt im inneren Bereich, damit sie die Datenspalten links
    // und rechts sowie das Kursband nicht überdeckt.
    let half = [(glass.max[0] - glass.min[0]) * 0.5, (glass.max[1] - glass.min[1]) * 0.5];
    dl.set_clip(Some(Rect::from_center(c, [half[0] * 0.56, half[1] * 0.70])));
    for deg in (-90_i32..=90).step_by(5) {
        let rung = (deg as f32).to_radians();
        let delta = rung - pitch;
        // Nur Sprossen in Sichtweite (Tangens bleibt beschränkt).
        if delta.abs() > 0.7 {
            continue;
        }
        let off = f * delta.tan();
        let mid = [c[0] + up[0] * off, c[1] + up[1] * off];
        let p = |s: f32| [mid[0] + along[0] * s, mid[1] + along[1] * s];
        let tick =
            |s: f32, len: f32| [mid[0] + along[0] * s + up[0] * len, mid[1] + along[1] * s + up[1] * len];

        if deg == 0 {
            // Horizontlinie, breit, mit Lücke für das Boresight-Symbol
            dl.line(p(-0.40), p(-0.04), Ink::Hud);
            dl.line(p(0.04), p(0.40), Ink::Hud);
            continue;
        }
        let (gap, len) = (0.035, 0.06);
        let tick_len = if deg > 0 { -0.012 } else { 0.012 };
        if deg > 0 {
            dl.line(p(-gap - len), p(-gap), Ink::Hud);
            dl.line(p(gap), p(gap + len), Ink::Hud);
        } else {
            dl.dashed(p(-gap - len), p(-gap), 3, Ink::Hud);
            dl.dashed(p(gap), p(gap + len), 3, Ink::Hud);
        }
        dl.line(p(-gap - len), tick(-gap - len, tick_len), Ink::Hud);
        dl.line(p(gap + len), tick(gap + len, tick_len), Ink::Hud);
        let label = format!("{}", deg.abs());
        let lp = p(-gap - len - 0.012);
        dl.text_mid(lp, TXT_SMALL, Align::Right, &label, Ink::Hud);
        let rp = p(gap + len + 0.012);
        dl.text_mid(rp, TXT_SMALL, Align::Left, &label, Ink::Hud);
    }
    dl.set_clip(None);
}

/// Boresight („Wasserlinie“): zeigt die Längsachse des Flugzeugs.
fn boresight(dl: &mut DisplayList, c: [f32; 2], vh: f32) {
    let s = vh * 0.018;
    dl.polyline(
        &[
            [c[0] - 2.2 * s, c[1]],
            [c[0] - 1.1 * s, c[1]],
            [c[0] - 0.55 * s, c[1] - 0.6 * s],
            [c[0], c[1]],
            [c[0] + 0.55 * s, c[1] - 0.6 * s],
            [c[0] + 1.1 * s, c[1]],
            [c[0] + 2.2 * s, c[1]],
        ],
        false,
        Ink::Hud,
    );
}

/// Flight Path Marker: wohin das Flugzeug tatsächlich fliegt.
fn flight_path_marker(dl: &mut DisplayList, fd: &FlightData, layout: &ScreenLayout, glass: &Rect, vh: f32) {
    let v = fd.velocity_dir_body.to_array();
    let Some(mut p) = layout.project_body(v) else {
        return;
    };
    let caged = !glass.contains(p);
    if caged {
        p[0] = p[0].clamp(glass.min[0], glass.max[0]);
        p[1] = p[1].clamp(glass.min[1], glass.max[1]);
    }
    let r = vh * 0.012;
    let ink = if caged { Ink::HudDim } else { Ink::Hud };
    dl.circle(p, r, 12, ink);
    dl.line([p[0] - r, p[1]], [p[0] - 2.6 * r, p[1]], ink);
    dl.line([p[0] + r, p[1]], [p[0] + 2.6 * r, p[1]], ink);
    dl.line([p[0], p[1] + r], [p[0], p[1] + 2.0 * r], ink);
}

/// Raute über dem aktiven Navigationspunkt bzw. Pfeil am Glasrand.
fn steerpoint_marker(dl: &mut DisplayList, fd: &FlightData, layout: &ScreenLayout, glass: &Rect, vh: f32) {
    let Some(sp) = &fd.steerpoint else {
        return;
    };
    let s = vh * 0.014;
    let c = glass.center();
    match layout.project_body(sp.direction_body.to_array()) {
        Some(p) if glass.contains(p) => {
            dl.polyline(
                &[[p[0], p[1] + s], [p[0] + s, p[1]], [p[0], p[1] - s], [p[0] - s, p[1]]],
                true,
                Ink::Hud,
            );
        }
        _ => {
            // Pfeil am Rand in Richtung des Punktes (Projektion auf die Bildebene).
            let d = sp.direction_body;
            let dir = [d.y as f32, -d.z as f32];
            let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
            if len < 1e-6 {
                return;
            }
            let u = [dir[0] / len, dir[1] / len];
            let half = [
                (glass.max[0] - glass.min[0]) * 0.5 - s * 2.0,
                (glass.max[1] - glass.min[1]) * 0.5 - s * 2.0,
            ];
            let k = (half[0] / u[0].abs().max(1e-6)).min(half[1] / u[1].abs().max(1e-6));
            let tip = [c[0] + u[0] * k, c[1] + u[1] * k];
            let n = [-u[1], u[0]];
            let back = [tip[0] - u[0] * s * 1.6, tip[1] - u[1] * s * 1.6];
            dl.polyline(
                &[
                    [back[0] + n[0] * s * 0.8, back[1] + n[1] * s * 0.8],
                    tip,
                    [back[0] - n[0] * s * 0.8, back[1] - n[1] * s * 0.8],
                ],
                false,
                Ink::Hud,
            );
        }
    }
}

/// Kursband oben im HUD.
fn heading_tape(dl: &mut DisplayList, fd: &FlightData, glass: &Rect) {
    let width = glass.max[0] - glass.min[0];
    let cx = glass.center()[0];
    let y = glass.max[1] - 0.012;
    let span_deg = 30.0_f32; // sichtbarer Bereich ±30°
    let px_per_deg = width * 0.42 / span_deg;
    let hdg = (fd.heading as f32).to_degrees();
    let band =
        Rect { min: [cx - span_deg * px_per_deg, y - 0.06], max: [cx + span_deg * px_per_deg, y + 0.03] };
    dl.set_clip(Some(band));
    let first = ((hdg - span_deg) / 5.0).floor() as i32 * 5;
    let mut d = first;
    while (d as f32) <= hdg + span_deg {
        let x = cx + (d as f32 - hdg) * px_per_deg;
        let major = d.rem_euclid(10) == 0;
        dl.line([x, y], [x, y - if major { 0.016 } else { 0.008 }], Ink::Hud);
        let label = format!("{:03}", d.rem_euclid(360));
        let half_label = crate::font::text_width(&label, TXT_SMALL) * 0.5;
        if major && x - half_label >= band.min[0] && x + half_label <= band.max[0] {
            dl.text([x, y - 0.040], TXT_SMALL, Align::Center, &label, Ink::Hud);
        }
        d += 5;
    }
    // Peilmarke zum Steerpoint
    if let Some(sp) = &fd.steerpoint {
        let rel = f29_core::detmath::wrap_pi(sp.bearing - fd.heading) as f32;
        let x = cx + rel.to_degrees().clamp(-span_deg + 1.5, span_deg - 1.5) * px_per_deg;
        dl.polyline(&[[x - 0.007, y + 0.012], [x, y + 0.002], [x + 0.007, y + 0.012]], true, Ink::Hud);
    }
    dl.set_clip(None);
    // Zeiger und Digitalanzeige
    dl.line([cx, y - 0.046], [cx - 0.008, y - 0.058], Ink::Hud);
    dl.line([cx, y - 0.046], [cx + 0.008, y - 0.058], Ink::Hud);
    dl.boxed_text(
        [cx, y - 0.075],
        TXT_SMALL,
        Align::Center,
        &format!("{:03}", heading_deg(fd.heading)),
        Ink::Hud,
    );
}

/// Querneigungsskala unten im HUD.
fn bank_scale(dl: &mut DisplayList, fd: &FlightData, glass: &Rect) {
    let c = glass.center();
    let r = (glass.max[1] - glass.min[1]) * 0.42;
    for deg in [-60.0_f32, -45.0, -30.0, -20.0, -10.0, 0.0, 10.0, 20.0, 30.0, 45.0, 60.0] {
        let a = (-90.0 + deg).to_radians();
        let len = if deg.abs() == 30.0 || deg == 0.0 || deg.abs() == 60.0 { 0.016 } else { 0.009 };
        let p0 = [c[0] + r * a.cos(), c[1] + r * a.sin()];
        let p1 = [c[0] + (r + len) * a.cos(), c[1] + (r + len) * a.sin()];
        dl.line(p0, p1, Ink::HudDim);
    }
    let roll = (fd.roll as f32).clamp(-1.4, 1.4);
    let a = -std::f32::consts::FRAC_PI_2 + roll;
    let tip = [c[0] + r * a.cos(), c[1] + r * a.sin()];
    let n = [-a.sin(), a.cos()];
    let inner = r - 0.016;
    let b = [c[0] + inner * a.cos(), c[1] + inner * a.sin()];
    dl.polyline(
        &[[b[0] + n[0] * 0.007, b[1] + n[1] * 0.007], tip, [b[0] - n[0] * 0.007, b[1] - n[1] * 0.007]],
        true,
        Ink::Hud,
    );
}

/// Links: Geschwindigkeit, Mach, g, Anstellwinkel.
fn left_data(dl: &mut DisplayList, fd: &FlightData, glass: &Rect) {
    let x = glass.min[0] + 0.01;
    let cy = glass.center()[1];
    let kts = units::mps_to_kt(fd.tas_mps).round() as i64;
    dl.text_mid([x, cy + 0.045], TXT_SMALL, Align::Left, &format!("G {:.1}", fd.load_factor), Ink::Hud);
    dl.boxed_text([x + 0.006, cy], TXT, Align::Left, &format!("{kts:>4}"), Ink::Hud);
    dl.text_mid([x, cy - 0.045], TXT_SMALL, Align::Left, &format!("M {:.2}", fd.mach), Ink::Hud);
    dl.text_mid(
        [x, cy - 0.075],
        TXT_SMALL,
        Align::Left,
        &format!("A {:.1}", fd.alpha.to_degrees()),
        Ink::HudDim,
    );
}

/// Rechts: Höhe, Radarhöhe, Steigrate, Steerpoint.
fn right_data(dl: &mut DisplayList, fd: &FlightData, glass: &Rect) {
    let x = glass.max[0] - 0.01;
    let cy = glass.center()[1];
    let ft = units::m_to_ft(fd.altitude_m).round() as i64;
    let vs = (units::mps_to_fpm(fd.vertical_speed_mps) / 100.0).round() as i64 * 100;
    dl.text_mid([x, cy + 0.045], TXT_SMALL, Align::Right, &format!("{vs:+}"), Ink::Hud);
    dl.boxed_text([x - 0.006, cy], TXT, Align::Right, &format!("{ft:>5}"), Ink::Hud);
    // Höhe über Grund (Radarhöhe) immer anzeigen: Das Gelände hat echte Berge,
    // die Höhe über dem Meer allein reicht für den Tiefflug nicht.
    let rft = units::m_to_ft(fd.radar_altitude_m).max(0.0);
    let step = if rft < 1_000.0 { 10.0 } else { 50.0 };
    let r = (rft / step).round() as i64 * step as i64;
    let ink = if rft < 500.0 { Ink::Warning } else { Ink::Hud };
    dl.text_mid([x, cy - 0.045], TXT_SMALL, Align::Right, &format!("R {r:>5}"), ink);
    if let Some(sp) = &fd.steerpoint {
        let nm = units::m_to_nm(sp.range);
        let txt = format!("{} {:.1}", sp.name, nm);
        dl.text_mid([x, glass.min[1] + 0.03], TXT_SMALL, Align::Right, &txt, Ink::Hud);
    }
}

/// Warnungen in der unteren Glasmitte; blinken mit 2 Hz.
fn warnings(dl: &mut DisplayList, fd: &FlightData, glass: &Rect) {
    let blink = (fd.time_s * 2.0).fract() < 0.6;
    let c = glass.center();
    let mut y = c[1] - (glass.max[1] - glass.min[1]) * 0.22;
    let mut show = |dl: &mut DisplayList, txt: &str| {
        if blink {
            dl.boxed_text([c[0], y], TXT, Align::Center, txt, Ink::Warning);
        }
        y -= TXT * 2.2;
    };
    if fd.warnings.pull_up {
        show(dl, "PULL UP");
    }
    if fd.warnings.stall {
        show(dl, "STALL");
    }
    if fd.warnings.over_g {
        show(dl, "OVER G");
    }
}
