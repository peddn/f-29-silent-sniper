//! Head-up-Display in der 3D-Ansicht.

use crate::canvas::{Align, DisplayList, Ink, Rect};
use crate::{ScreenLayout, heading_deg};
use f29_core::FlightData;
use f29_core::units;
use f29_core::weapons::LaunchZone;

/// Zeichenhöhe der HUD-Schrift (Bildschirmeinheiten).
const TXT: f32 = 0.020;
const TXT_SMALL: f32 = 0.016;
/// Luft-Luft-Kreis um die Flugzeugnase (halber Öffnungswinkel), einheitlich
/// für alle Luft-Luft-Waffen (Spielerwunsch v0.2.8).
pub(crate) const AA_CIRCLE_DEG: f32 = 10.0;

/// HUD-Modus (Taste H blättert): Was das HUD neben den Flugdaten zeigt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HudMode {
    /// Navigation: nur, was für den Anflug von Wegpunkten und Landeplätzen
    /// zählt (Navigationspunkt, Konfiguration, im Landemodus Gleitlinie und
    /// Sinkratenskala).
    #[default]
    Nav,
    /// Luft-Boden: Zielrahmen und Zielhilfe je nach gewählter Waffe
    /// (Kanonenkreuz, Bombenkreis, Abschusskreis, Feuersignal).
    AirToGround,
    /// Luft-Luft: großer Boresight-Kreis für alle Luft-Luft-Waffen, immer
    /// sichtbar.
    AirToAir,
}

impl HudMode {
    pub fn next(self) -> Self {
        match self {
            HudMode::Nav => HudMode::AirToGround,
            HudMode::AirToGround => HudMode::AirToAir,
            HudMode::AirToAir => HudMode::Nav,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HudMode::Nav => "NAV",
            HudMode::AirToGround => "AG",
            HudMode::AirToAir => "AA",
        }
    }
}

pub(crate) fn draw(dl: &mut DisplayList, fd: &FlightData, layout: &ScreenLayout, mode: HudMode) {
    let view = layout.view_rect();
    let vh = view.max[1] - view.min[1];
    let c = view.center();
    // Das HUD-Glas: gut drei Viertel der Ansichtshöhe und deutlich breiter als
    // hoch, damit Daten und Symbole nach außen rücken (v0.2.8).
    let glass = Rect::from_center(c, [vh * 0.56, vh * 0.38]);

    if fd.warnings.crashed {
        dl.text_mid(c, TXT * 2.2, Align::Center, "CRASHED", Ink::HudWarning);
        if let f29_core::Status::Crashed { cause, .. } = fd.status {
            dl.text_mid([c[0], c[1] - TXT * 2.6], TXT, Align::Center, cause.label(), Ink::HudWarning);
        }
        dl.text_mid([c[0], c[1] - TXT * 5.0], TXT, Align::Center, "R - RESTART", Ink::HudWarning);
        return;
    }

    pitch_ladder(dl, fd, layout, &glass);
    if mode == HudMode::Nav && landing_mode(fd) {
        glide_line(dl, fd, layout, &glass);
        sink_scale(dl, fd, &glass);
    }
    ground_messages(dl, fd, &glass);
    boresight(dl, c, vh);
    flight_path_marker(dl, fd, layout, &glass, vh);
    steerpoint_marker(dl, fd, layout, &glass, vh);
    match mode {
        HudMode::Nav => {}
        HudMode::AirToGround => {
            target_marker(dl, fd, layout, &glass, vh);
            bomb_aim(dl, fd, layout, &glass, vh);
            gun_cross(dl, fd, layout, &glass, vh);
            launch_circle(dl, fd, layout, c);
            shoot_cue(dl, fd, &glass);
        }
        HudMode::AirToAir => {
            let r = layout.focal() * AA_CIRCLE_DEG.to_radians().tan();
            dl.circle(c, r, 48, Ink::Hud);
        }
    }
    heading_tape(dl, fd, &glass);
    bank_scale(dl, fd, &glass);
    left_data(dl, fd, &glass, mode);
    right_data(dl, fd, &glass, mode);
    warnings(dl, fd, &glass, mode);
    weapon_messages(dl, fd, &glass);
    // Modus links unter dem Kursband
    let at = [glass.min[0] + 0.018, glass.max[1] - 0.075];
    dl.boxed_text(at, TXT_SMALL, Align::Left, mode.label(), Ink::Hud);
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
    dl.set_clip(Some(Rect::from_center(c, [half[0] * 0.66, half[1] * 0.70])));
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

        // Am Boden nur der Horizont – Platz für die Landemeldungen.
        if fd.on_ground && deg != 0 {
            continue;
        }
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

/// Zielhilfe für ungelenkte Bomben (M-014): Aufschlagpunkt mit Fall-Linie
/// vom Flugbahnsymbol; liegt der Kreis auf dem Ziel, auslösen. Rot, wenn der
/// Einschlag im eigenen Splitterbereich läge. (Die Auslöseklammer des
/// Originals ist für später vorgemerkt, M-014.)
fn bomb_aim(dl: &mut DisplayList, fd: &FlightData, layout: &ScreenLayout, glass: &Rect, vh: f32) {
    let Some(aim) = &fd.bomb_aim else {
        return;
    };
    if fd.roll.abs() > f29_core::weapons::MAX_RELEASE_BANK_RAD {
        return;
    }
    let ink = if aim.too_low { Ink::HudWarning } else { Ink::Hud };
    let r = vh * 0.011;
    // Aufschlagpunkt und Fall-Linie
    if let (Some(fpm), Some(p)) = (
        layout.project_body(fd.velocity_dir_body.to_array()),
        layout.project_body(aim.direction_body.to_array()),
    ) {
        dl.set_clip(Some(*glass));
        let d = [p[0] - fpm[0], p[1] - fpm[1]];
        let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
        if len > r * 2.0 {
            let k = (len - r) / len;
            dl.line(fpm, [fpm[0] + d[0] * k, fpm[1] + d[1] * k], ink);
        }
        dl.circle(p, r, 12, ink);
        dl.line([p[0] - r * 0.2, p[1]], [p[0] + r * 0.2, p[1]], ink);
        dl.set_clip(None);
    }
}

/// Kanonenkreuz (M-016): wo eine jetzt abgefeuerte Patrone auf dem Gelände
/// einschlüge – nur innerhalb ihrer Flugzeit. Ein offenes Kreuz, damit es
/// sich vom Bombenkreis unterscheidet.
fn gun_cross(dl: &mut DisplayList, fd: &FlightData, layout: &ScreenLayout, glass: &Rect, vh: f32) {
    let Some(aim) = &fd.gun.aim else {
        return;
    };
    if fd.gun.rounds == 0 || !fd.stores.gun_selected {
        return;
    }
    let Some(p) = layout.project_body(aim.direction_body.to_array()) else {
        return;
    };
    if !glass.contains(p) {
        return;
    }
    let (gap, arm) = (vh * 0.004, vh * 0.014);
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        dl.line([p[0] + dx * gap, p[1] + dy * gap], [p[0] + dx * arm, p[1] + dy * arm], Ink::Hud);
    }
}

/// Abschusskreis der Raketen um die Flugzeugnase (M-014): Liegt das
/// aufgeschaltete Ziel darin, übernimmt die Rakete es beim Abschuss.
fn launch_circle(dl: &mut DisplayList, fd: &FlightData, layout: &ScreenLayout, center: [f32; 2]) {
    let st = &fd.stores;
    let Some(kind) = st.bays[st.selected].kind.filter(|_| st.bays[st.selected].count > 0 && !st.gun_selected)
    else {
        return;
    };
    if !kind.uses_launch_circle() {
        return;
    }
    let r = layout.focal() * (f29_core::weapons::LAUNCH_CIRCLE_RAD as f32).tan();
    // Heller als früher, fast wie die übrige Symbolik (Spielerwunsch v0.2.8).
    dl.circle(center, r, 40, Ink::Hud);
}

/// Feuersignal der Lenkwaffen unten mittig im Glas: „LOCKED“, solange die
/// Waffe das Ziel übernähme, darüber „SHOOT“ (umrahmt), wenn die
/// vorausberechnete Flugbahn trifft.
fn shoot_cue(dl: &mut DisplayList, fd: &FlightData, glass: &Rect) {
    let Some(t) = &fd.target else {
        return;
    };
    let st = &fd.stores;
    if st.gun_selected
        || !st.bays[st.selected].kind.is_some_and(|k| k.is_guided())
        || st.bays[st.selected].count == 0
    {
        return;
    }
    let pos = [glass.center()[0], glass.min[1] + TXT * 1.4];
    let zone = t.zone;
    if matches!(zone, Some(LaunchZone::Locked | LaunchZone::Good)) {
        dl.text_mid(pos, TXT_SMALL, Align::Center, "LOCKED", Ink::Hud);
    }
    if zone == Some(LaunchZone::Good) {
        dl.boxed_text([pos[0], pos[1] + TXT * 1.7], TXT, Align::Center, "SHOOT", Ink::HudWarning);
    }
}

/// Raute über dem aktiven Navigationspunkt bzw. Pfeil am Glasrand.
fn steerpoint_marker(dl: &mut DisplayList, fd: &FlightData, layout: &ScreenLayout, glass: &Rect, vh: f32) {
    let Some(sp) = &fd.steerpoint else {
        return;
    };
    let s = vh * 0.014;
    match layout.project_body(sp.direction_body.to_array()) {
        Some(p) if glass.contains(p) => {
            dl.polyline(
                &[[p[0], p[1] + s], [p[0] + s, p[1]], [p[0], p[1] - s], [p[0] - s, p[1]]],
                true,
                Ink::Hud,
            );
        }
        _ => {
            edge_arrow(dl, sp.direction_body, glass, s, Ink::Hud);
        }
    }
}

/// Pfeil am Glasrand in Richtung `d` (Körperachsen, Projektion auf die
/// Bildebene); liefert die Pfeilspitze.
fn edge_arrow(dl: &mut DisplayList, d: f29_core::DVec3, glass: &Rect, s: f32, ink: Ink) -> Option<[f32; 2]> {
    let c = glass.center();
    let dir = [d.y as f32, -d.z as f32];
    let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
    if len < 1e-6 {
        return None;
    }
    let u = [dir[0] / len, dir[1] / len];
    let half = [(glass.max[0] - glass.min[0]) * 0.5 - s * 2.0, (glass.max[1] - glass.min[1]) * 0.5 - s * 2.0];
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
        ink,
    );
    Some(tip)
}

/// Zielrahmen um das aufgeschaltete Ziel (M-013) bzw. Pfeil am Glasrand.
/// Zivile Ziele sind durchgekreuzt.
fn target_marker(dl: &mut DisplayList, fd: &FlightData, layout: &ScreenLayout, glass: &Rect, vh: f32) {
    let Some(t) = &fd.target else {
        return;
    };
    let s = vh * 0.02;
    let zone = t.zone;
    let ink = if t.civilian { Ink::HudDim } else { Ink::Hud };
    match layout.project_body(t.direction_body.to_array()) {
        Some(p) if glass.contains(p) => {
            // Form nach Reichweitenzone der gewählten Lenkwaffe (wie im
            // Original: in Reichweite erfasst, näher dran Farbwechsel). Blau,
            // wenn die vorausberechnete Flugbahn trifft.
            let r = Rect::from_center(p, [s, s]);
            let ink = match zone {
                Some(LaunchZone::Good) => Ink::HudWarning,
                Some(LaunchZone::NoEffect) => Ink::HudDim,
                _ if t.destroyed => Ink::HudDim,
                _ => ink,
            };
            if zone == Some(LaunchZone::NoEffect) {
                dashed_rect(dl, r, ink);
            } else {
                dl.rect(r, ink);
            }
            if matches!(zone, Some(LaunchZone::Locked | LaunchZone::Good)) {
                // Kräftige Ecken außen um den Rahmen
                let o = Rect::from_center(p, [s * 1.45, s * 1.45]);
                let k = s * 0.55;
                for (cx, cy, dx, dy) in [
                    (o.min[0], o.min[1], 1.0, 1.0),
                    (o.max[0], o.min[1], -1.0, 1.0),
                    (o.min[0], o.max[1], 1.0, -1.0),
                    (o.max[0], o.max[1], -1.0, -1.0),
                ] {
                    dl.polyline(&[[cx + dx * k, cy], [cx, cy], [cx, cy + dy * k]], false, ink);
                }
            }
            if t.civilian {
                dl.line(r.min, r.max, ink);
                dl.line([r.min[0], r.max[1]], [r.max[0], r.min[1]], ink);
            }
            let sys = match zone {
                _ if t.destroyed => "DESTROYED",
                Some(LaunchZone::NoEffect) => "NO EFF",
                _ if t.optical => "OPT",
                _ => "RDR",
            };
            dl.text_mid([p[0], p[1] - s * 2.2], TXT_SMALL * 0.8, Align::Center, sys, ink);
            if t.damage > 0.0 && !t.destroyed {
                dl.text_mid([p[0], p[1] + s * 2.2], TXT_SMALL * 0.8, Align::Center, "DMG", ink);
            }
        }
        _ => {
            if let Some(tip) = edge_arrow(dl, t.direction_body, glass, s * 0.8, ink) {
                let c = glass.center();
                let back = [
                    tip[0] + (c[0] - tip[0]).signum() * s * 2.4,
                    tip[1] + (c[1] - tip[1]).signum() * s * 1.6,
                ];
                dl.text_mid(back, TXT_SMALL * 0.8, Align::Center, "TGT", ink);
            }
        }
    }
}

/// Rechteck aus Strichen (drei je Seite).
fn dashed_rect(dl: &mut DisplayList, r: Rect, ink: Ink) {
    let corners = [r.min, [r.max[0], r.min[1]], r.max, [r.min[0], r.max[1]]];
    for k in 0..4 {
        let (a, b) = (corners[k], corners[(k + 1) % 4]);
        for d in 0..3 {
            let t0 = (d as f32 * 2.0) / 5.0;
            let t1 = t0 + 1.0 / 5.0;
            let at = |t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            dl.line(at(t0), at(t1), ink);
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

/// Links: Geschwindigkeit, Mach, g, Anstellwinkel; Konfiguration; in den
/// Waffenmodi die gewählte Waffe.
fn left_data(dl: &mut DisplayList, fd: &FlightData, glass: &Rect, mode: HudMode) {
    let x = glass.min[0] + 0.012;
    let cy = glass.center()[1];
    let kmh = units::mps_to_kmh(fd.tas_mps).round() as i64;
    dl.text_mid([x, cy + 0.055], TXT_SMALL, Align::Left, &format!("G {:.1}", fd.load_factor), Ink::Hud);
    dl.boxed_text([x + 0.006, cy], TXT, Align::Left, &format!("{kmh:>4}"), Ink::Hud);
    dl.text_mid([x, cy - 0.055], TXT_SMALL, Align::Left, &format!("M {:.2}", fd.mach), Ink::Hud);
    dl.text_mid(
        [x, cy - 0.085],
        TXT_SMALL,
        Align::Left,
        &format!("A {:.1}", fd.alpha.to_degrees()),
        Ink::HudDim,
    );
    configuration(dl, fd, [x, cy - 0.13]);
    if mode != HudMode::Nav {
        selected_weapon(dl, fd, glass);
    }
}

/// Gewählte Waffe unten links: Name und Anzahl (bzw. Kanone mit Munition),
/// darunter Schacht und Klappenzustand.
fn selected_weapon(dl: &mut DisplayList, fd: &FlightData, glass: &Rect) {
    let st = &fd.stores;
    let bay = st.bays[st.selected];
    let x = glass.min[0] + 0.012;
    let y = glass.min[1] + TXT_SMALL * 1.2;
    let (text, ink) = match bay.kind {
        _ if st.gun_selected && fd.gun.rounds > 0 => (format!("GUN {}", fd.gun.rounds), Ink::Hud),
        _ if st.gun_selected => ("GUN EMPTY".to_string(), Ink::HudDim),
        Some(kind) if bay.count > 0 => (format!("{} x{}", kind.label(), bay.count), Ink::Hud),
        Some(kind) => (format!("{} EMPTY", kind.label()), Ink::HudDim),
        None => ("NO STORE".to_string(), Ink::HudDim),
    };
    dl.text_mid([x, y + TXT_SMALL * 1.7], TXT_SMALL, Align::Left, &text, ink);
    let door = if st.door >= 1.0 {
        "OPEN"
    } else if st.door > 0.0 {
        "DOORS"
    } else {
        "SAFE"
    };
    let ink = if st.door > 0.0 { Ink::HudWarning } else { Ink::HudDim };
    let bay_txt =
        if st.gun_selected { format!("BAYS {door}") } else { format!("BAY {} {door}", st.selected + 1) };
    dl.text_mid([x, y], TXT_SMALL, Align::Left, &bay_txt, ink);
}

/// Konfiguration unten links: Fahrwerk, Klappen, Bremse – nur, wenn
/// ausgefahren; das Fahrwerk blinkt, solange es fährt oder zu schnell ist.
/// Jede Anzeige hat eine feste Zeile, damit Ausblenden oder Blinken die
/// anderen nicht verschiebt.
fn configuration(dl: &mut DisplayList, fd: &FlightData, top_left: [f32; 2]) {
    let blink = (fd.time_s * 2.0).fract() < 0.5;
    let at = |slot: usize| [top_left[0], top_left[1] - slot as f32 * TXT_SMALL * 1.7];
    let gear_moving = fd.gear > 0.001 && fd.gear < 0.999;
    let gear_down = fd.gear >= 0.999;
    if (gear_down && (!fd.warnings.gear_overspeed || blink)) || (gear_moving && blink) {
        let ink = if gear_down && fd.warnings.gear_overspeed { Ink::HudWarning } else { Ink::Hud };
        dl.text_mid(at(CONFIG_GEAR), TXT_SMALL, Align::Left, "GEAR", ink);
    }
    if fd.flaps > 0.05 {
        let txt = if fd.flaps < 0.999 { "FLAPS ." } else { "FLAPS" };
        dl.text_mid(at(CONFIG_FLAPS), TXT_SMALL, Align::Left, txt, Ink::Hud);
    }
    if fd.airbrake > 0.05 {
        let txt = if fd.on_ground { "BRAKES" } else { "AIRBRAKE" };
        dl.text_mid(at(CONFIG_BRAKE), TXT_SMALL, Align::Left, txt, Ink::Hud);
    }
}

/// Feste Zeilen der Konfigurationsanzeige.
const CONFIG_GEAR: usize = 0;
const CONFIG_FLAPS: usize = 1;
const CONFIG_BRAKE: usize = 2;

/// Rechts: Höhe (m), Radarhöhe (m), Steigrate (m/s), Steerpoint (km); in
/// den Waffenmodi die Zielerfassung.
fn right_data(dl: &mut DisplayList, fd: &FlightData, glass: &Rect, mode: HudMode) {
    let x = glass.max[0] - 0.012;
    let cy = glass.center()[1];
    let alt = fd.altitude_m.round() as i64;
    let vs = fd.vertical_speed_mps.round() as i64;
    dl.text_mid([x, cy + 0.055], TXT_SMALL, Align::Right, &format!("{vs:+}"), Ink::Hud);
    dl.boxed_text([x - 0.006, cy], TXT, Align::Right, &format!("{alt:>5}"), Ink::Hud);
    // Höhe über Grund (Radarhöhe) immer anzeigen: Das Gelände hat echte Berge,
    // die Höhe über dem Meer allein reicht für den Tiefflug nicht.
    let ragl = fd.radar_altitude_m.max(0.0);
    let step = if ragl < 300.0 { 5.0 } else { 10.0 };
    let r = (ragl / step).round() as i64 * step as i64;
    let ink = if ragl < 150.0 { Ink::HudWarning } else { Ink::Hud };
    dl.text_mid([x, cy - 0.055], TXT_SMALL, Align::Right, &format!("R {r:>5}"), ink);
    if let Some(sp) = &fd.steerpoint {
        let txt = format!("{} {:.1}", sp.name, units::m_to_km(sp.range));
        dl.text_mid([x, glass.min[1] + 0.03], TXT_SMALL, Align::Right, &txt, Ink::Hud);
    }
    if mode == HudMode::Nav {
        return;
    }
    // Zielerfassung: Radar an, Blickrichtung, aufgeschaltetes Ziel
    let mut sensor = String::new();
    if fd.radar_on {
        sensor.push_str("RDR ");
    }
    sensor.push_str(fd.look.label());
    if let Some(t) = &fd.target {
        sensor = format!("{sensor} {:.1}", units::m_to_km(t.range));
    }
    dl.text_mid([x, glass.min[1] + 0.055], TXT_SMALL, Align::Right, &sensor, Ink::Hud);
}

/// Warnungen in der unteren Glasmitte; blinken mit 2 Hz.
/// Waffenmeldungen unter dem HUD-Glas in der Außensicht, neueste oben.
fn weapon_messages(dl: &mut DisplayList, fd: &FlightData, glass: &Rect) {
    let c = glass.center();
    let mut y = glass.min[1] - TXT_SMALL * 1.3;
    for m in fd.weapon_messages.iter().rev() {
        let ink = if m.warning { Ink::HudWarning } else { Ink::Hud };
        dl.text_mid([c[0], y], TXT_SMALL, Align::Center, &m.text, ink);
        y -= TXT_SMALL * 1.45;
    }
}

fn warnings(dl: &mut DisplayList, fd: &FlightData, glass: &Rect, mode: HudMode) {
    let blink = (fd.time_s * 2.0).fract() < 0.6;
    let c = glass.center();
    let mut y = c[1] - (glass.max[1] - glass.min[1]) * 0.22;
    let mut show = |dl: &mut DisplayList, txt: &str| {
        if blink {
            dl.boxed_text([c[0], y], TXT, Align::Center, txt, Ink::HudWarning);
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
    if fd.warnings.gear_up {
        show(dl, "GEAR");
    }
    if fd.warnings.gear_overspeed {
        show(dl, "GEAR SPEED");
    }
    // Bombe fiele in den eigenen Splitterbereich (M-014).
    if mode == HudMode::AirToGround && fd.bomb_aim.is_some_and(|a| a.too_low) {
        show(dl, "TOO LOW");
    }
    if fd.warnings.flameout {
        show(dl, "FLAMEOUT");
    } else if fd.warnings.bingo {
        show(dl, "BINGO");
    } else if fd.warnings.fuel_low {
        show(dl, "FUEL LOW");
    }
}

/// Landemodus: Fahrwerk ausgefahren und in der Luft.
fn landing_mode(fd: &FlightData) -> bool {
    fd.gear > 0.99 && !fd.on_ground
}

/// Gestrichelte −3°-Linie in der Pitch-Leiter: Liegt der Flight Path Marker
/// darauf und die Linie auf der Schwelle, stimmt der Gleitpfad.
fn glide_line(dl: &mut DisplayList, fd: &FlightData, layout: &ScreenLayout, glass: &Rect) {
    let f = layout.focal();
    let c = glass.center();
    let (sr, cr) = (fd.roll as f32).sin_cos();
    let along = [cr, sr];
    let up = [-sr, cr];
    let delta = -GLIDE_DEG.to_radians() - fd.pitch as f32;
    if delta.abs() > 0.7 {
        return;
    }
    let off = f * delta.tan();
    let mid = [c[0] + up[0] * off, c[1] + up[1] * off];
    let p = |s: f32| [mid[0] + along[0] * s, mid[1] + along[1] * s];
    dl.dashed(p(-0.16), p(-0.05), 4, Ink::HudDim);
    dl.dashed(p(0.05), p(0.16), 4, Ink::HudDim);
}

/// Gleitwinkel des Anflugs in Grad.
const GLIDE_DEG: f32 = 3.0;
/// Sinkrate, ab der eine Landung hart wird bzw. das Fahrwerk bricht (m/s),
/// direkt aus den Flugzeugdaten.
pub(crate) const HARD_SINK_MPS: f32 = f29_core::data::F29_SPEC.hard_landing_sink_mps as f32;
pub(crate) const MAX_SINK_MPS: f32 = f29_core::data::F29_SPEC.max_landing_sink_mps as f32;

/// Sinkratenskala rechts im Glas (Landemodus): Balken für die Steig-/Sinkrate,
/// Marken für „hart“ und „Bruch“ – wie die Landehilfe im Original, eigene Form.
fn sink_scale(dl: &mut DisplayList, fd: &FlightData, glass: &Rect) {
    // rechts neben dem Glas, damit Höhe und Pitch-Leiter frei bleiben
    let x = glass.max[0] + 0.03;
    let top = glass.center()[1] + 0.03;
    let full = 10.0_f32; // m/s über die ganze Skala
    let len = 0.17_f32;
    let y_of = |mps: f32| top - (mps.clamp(0.0, full) / full) * len;
    dl.line([x, top], [x, top - len], Ink::HudDim);
    for (mps, ink) in [(HARD_SINK_MPS, Ink::Hud), (MAX_SINK_MPS, Ink::HudWarning)] {
        let y = y_of(mps);
        dl.line([x - 0.010, y], [x + 0.010, y], ink);
    }
    let sink = (-fd.vertical_speed_mps) as f32;
    let y = y_of(sink);
    let ink = if sink > MAX_SINK_MPS { Ink::HudWarning } else { Ink::Hud };
    dl.line([x - 0.004, top], [x - 0.004, y], ink);
    dl.line([x + 0.004, top], [x + 0.004, y], ink);
    dl.line([x - 0.006, y], [x + 0.006, y], ink);
    dl.text_mid([x, top + 0.018], TXT_SMALL * 0.8, Align::Center, "SINK", Ink::HudDim);
}

/// Meldungen am Boden: Aufsetzbewertung, Stillstand auf der Bahn.
fn ground_messages(dl: &mut DisplayList, fd: &FlightData, glass: &Rect) {
    if !fd.on_ground {
        return;
    }
    let c = glass.center();
    let y = c[1] + (glass.max[1] - glass.min[1]) * 0.18;
    if let Some(td) = &fd.touchdown {
        let sink = td.sink_mps;
        let (word, ink) =
            if td.hard { ("HARD LANDING", Ink::HudWarning) } else { ("GOOD LANDING", Ink::Hud) };
        dl.text_mid([c[0], y], TXT, Align::Center, word, ink);
        dl.text_mid([c[0], y - TXT * 1.6], TXT_SMALL, Align::Center, &format!("{sink:.1} M/S"), Ink::Hud);
    }
    if fd.stopped_on_runway
        && let Some(name) = fd.runway_name
    {
        dl.text_mid([c[0], y - TXT * 3.2], TXT_SMALL, Align::Center, &format!("STOPPED - {name}"), Ink::Hud);
    }
}
