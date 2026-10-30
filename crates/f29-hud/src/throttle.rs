//! Schubhebel ganz links im Instrumentenbrett.
//!
//! Angelehnt an das Original (ein Hebel, dessen Schaft von einem Drehpunkt in
//! der unteren Ecke schräg nach oben zum Griff läuft), eigene Gestaltung: Der
//! Hebel schwenkt in einer Bogenkulisse von IDLE (flach) bis MIL (steil). Eine
//! Marke an der Kulisse zeigt die tatsächliche Triebwerksstellung, die dem
//! Hebel verzögert folgt. Ist das Triebwerk beschädigt, sperrt eine rote
//! Marke den nicht mehr verfügbaren Bereich der Kulisse (Höchstschub).

use crate::canvas::{Align, DisplayList, Ink, Rect};
use f29_core::FlightData;
use f29_core::systems::System;

/// Hebelwinkel (rad, gegen die Waagerechte) bei Leerlauf und Vollschub.
const ANGLE_IDLE: f32 = 0.30;
const ANGLE_MIL: f32 = 1.40;

fn angle(t: f32) -> f32 {
    ANGLE_IDLE + (ANGLE_MIL - ANGLE_IDLE) * t.clamp(0.0, 1.0)
}

pub(crate) fn draw(dl: &mut DisplayList, fd: &FlightData, r: Rect) {
    let w = r.max[0] - r.min[0];
    let h = r.max[1] - r.min[1];
    let txt = (w * 0.11).min(h * 0.06);
    // Drehpunkt unten links, Hebellänge passt in Breite und Höhe
    let pivot = [r.min[0] + w * 0.12, r.min[1] + h * 0.14];
    let len = (w * 0.68).min(h * 0.66);
    let at = |a: f32, l: f32| [pivot[0] + a.cos() * l, pivot[1] + a.sin() * l];

    // Kulisse: zwei Bögen mit Skala alle 10 %
    let (r_in, r_out) = (len * 0.80, len * 0.92);
    dl.arc(pivot, r_in, ANGLE_IDLE - 0.06, ANGLE_MIL + 0.06, 20, Ink::PanelDim);
    dl.arc(pivot, r_out, ANGLE_IDLE - 0.06, ANGLE_MIL + 0.06, 20, Ink::PanelDim);
    for k in 0..=10 {
        let a = angle(k as f32 / 10.0);
        let outer = if k % 5 == 0 { r_out + len * 0.08 } else { r_out + len * 0.04 };
        dl.line(at(a, r_out), at(a, outer), Ink::Panel);
    }
    let idle = at(ANGLE_IDLE, r_out);
    dl.text_mid([idle[0], idle[1] - txt * 1.3], txt * 0.8, Align::Center, "IDLE", Ink::PanelDim);
    let mil = at(ANGLE_MIL, len * 1.18);
    dl.text_mid(mil, txt * 0.8, Align::Center, "MIL", Ink::PanelDim);

    // Triebwerksschaden: Bereich über dem verfügbaren Höchstschub rot
    // gesperrt, dicke Sperrmarke quer über die Kulisse.
    let cap = fd.systems.capability(System::Engine) as f32;
    if cap < 1.0 {
        let ac = angle(cap);
        for rr in [r_in, r_out, (r_in + r_out) * 0.5] {
            dl.arc(pivot, rr, ac, ANGLE_MIL + 0.06, 12, Ink::Warning);
        }
        for k in -1..=1 {
            let a = ac + k as f32 * 0.008;
            dl.line(at(a, r_in - len * 0.06), at(a, r_out + len * 0.1), Ink::Warning);
        }
    }

    // Tatsächliche Triebwerksstellung als Dreieck an der inneren Kulisse
    let ae = angle(fd.engine as f32);
    let tip = at(ae, r_in);
    let n = [-ae.sin(), ae.cos()];
    let base = at(ae, r_in - len * 0.09);
    let s = len * 0.035;
    dl.polyline(
        &[[base[0] + n[0] * s, base[1] + n[1] * s], tip, [base[0] - n[0] * s, base[1] - n[1] * s]],
        true,
        Ink::Hud,
    );

    // Hebel: dreifacher Schaft und T-Griff
    let a = angle(fd.throttle as f32);
    let (dir, nrm) = ([a.cos(), a.sin()], [-a.sin(), a.cos()]);
    let grip = at(a, len);
    for k in -1..=1 {
        let o = k as f32 * len * 0.018;
        dl.line(
            [pivot[0] + nrm[0] * o, pivot[1] + nrm[1] * o],
            [grip[0] + nrm[0] * o, grip[1] + nrm[1] * o],
            Ink::Needle,
        );
    }
    let gw = len * 0.13;
    let gd = len * 0.05;
    let corner = |u: f32, v: f32| [grip[0] + nrm[0] * u + dir[0] * v, grip[1] + nrm[1] * u + dir[1] * v];
    dl.polyline(&[corner(-gw, 0.0), corner(gw, 0.0), corner(gw, gd), corner(-gw, gd)], true, Ink::Needle);
    dl.circle(pivot, len * 0.05, 10, Ink::Panel);

    // Beschriftung
    let pct = (fd.throttle * 100.0).round() as i64;
    dl.text_mid(
        [r.min[0] + w * 0.5, r.min[1] + txt * 0.9],
        txt,
        Align::Center,
        &format!("THR {pct}"),
        Ink::Panel,
    );
    if cap < 1.0 {
        let max = (cap * 100.0).round() as i64;
        dl.text_mid(
            [r.max[0] - w * 0.02, r.max[1] - txt * 0.9],
            txt,
            Align::Right,
            &format!("MAX {max}"),
            Ink::Warning,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use f29_core::{ControlFrame, Sim, SimConfig};

    /// Höchster Punkt des Hebelgriffs (Needle-Linien).
    fn grip_top(throttle: f64) -> f32 {
        let mut sim = Sim::new(SimConfig::new(1));
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, throttle, 0));
        let mut fd = sim.flight_data();
        fd.throttle = throttle;
        let mut dl = DisplayList::new();
        let r = Rect { min: [0.0, 0.0], max: [0.2, 0.35] };
        draw(&mut dl, &fd, r);
        for l in &dl.lines {
            for p in [l.a, l.b] {
                assert!(p[0] >= r.min[0] - 0.02 && p[0] <= r.max[0] + 0.02, "x außerhalb: {p:?}");
                assert!(p[1] >= r.min[1] - 0.02 && p[1] <= r.max[1] + 0.02, "y außerhalb: {p:?}");
            }
        }
        dl.lines
            .iter()
            .filter(|l| l.ink == Ink::Needle)
            .flat_map(|l| [l.a[1], l.b[1]])
            .fold(f32::MIN, f32::max)
    }

    /// Beschädigtes Triebwerk: rote Sperre an der Kulisse und „MAX“.
    #[test]
    fn engine_damage_shows_a_red_stop() {
        use f29_core::systems::Condition;
        let reds = |cap: f64| {
            let mut sim = Sim::new(SimConfig::new(1));
            if cap < 1.0 {
                sim.set_condition(System::Engine, Condition::Degraded(cap));
            }
            let mut dl = DisplayList::new();
            draw(&mut dl, &sim.flight_data(), Rect { min: [0.0, 0.0], max: [0.2, 0.35] });
            dl.lines.iter().filter(|l| l.ink == Ink::Warning).count()
        };
        assert_eq!(reds(1.0), 0);
        assert!(reds(0.8) > 20);
    }

    #[test]
    fn lever_rises_with_throttle_and_stays_in_its_box() {
        let idle = grip_top(0.0);
        let half = grip_top(0.5);
        let full = grip_top(1.0);
        assert!(idle < half && half < full, "{idle} {half} {full}");
    }
}
