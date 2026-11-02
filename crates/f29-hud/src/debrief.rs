//! Nachbesprechung (M-032): Nach dem Missionsende folgen wie im Original
//! eigene Bildschirme aufeinander – Ausgang, Ereignisse mit Punkten
//! (seitenweise), Wertung. Gezeichnet über das ganze Fenster; das Frontend
//! legt eine dunkle Fläche dahinter (später ein Hintergrundbild je
//! Bildschirm, [`Screen`]).

use crate::ScreenLayout;
use crate::canvas::{Align, DisplayList, Ink};
use f29_core::{FlightData, Score, Side, Status};

pub(crate) const TITLE: f32 = 0.045;
pub(crate) const TXT: f32 = 0.028;
/// Zeilenabstand der Listen (Anteil der Fensterhöhe).
pub(crate) const ROW: f32 = 0.042;
/// So viele Ereignisse je Bildschirm.
pub const EVENTS_PER_SCREEN: usize = 14;

/// Ein Bildschirm der Nachbesprechung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Outcome,
    /// Ereignisse ab diesem Index.
    Events(usize),
    Rating,
}

/// Die Bildschirme dieser Nachbesprechung, der Reihe nach.
pub fn screens(score: &Score) -> Vec<Screen> {
    let events = (0..score.lines.len()).step_by(EVENTS_PER_SCREEN).map(Screen::Events);
    std::iter::once(Screen::Outcome).chain(events).chain([Screen::Rating]).collect()
}

/// Zeichnet Bildschirm Nummer `index` (über das Ende hinaus: der letzte).
pub(crate) fn draw(dl: &mut DisplayList, fd: &FlightData, layout: &ScreenLayout, index: usize) {
    let Some(score) = &fd.debrief else { return };
    let all = screens(score);
    let screen = all[index.min(all.len() - 1)];
    let half = half_width(layout);
    let (left, right) = (-half, half);
    match screen {
        Screen::Outcome => {
            let good = matches!(fd.status, Status::Landed { side: Side::Friendly, .. });
            let mut y = title(dl, "MISSION DEBRIEF");
            let ink = if good { Ink::Hud } else { Ink::HudWarning };
            dl.text_mid([0.0, y], TITLE, Align::Center, &fd.status.outcome(), ink);
            y -= TITLE * 2.5;
            for (k, label) in ["PRIMARY", "SECONDARY"].into_iter().enumerate() {
                if let Some((task, done)) = fd.objectives[k] {
                    let state = if done { "DONE" } else { "NOT DONE" };
                    let ink = if done { Ink::Hud } else { Ink::HudDim };
                    dl.text_mid([left, y], TXT, Align::Left, &format!("{label} {}", task.label()), ink);
                    dl.text_mid([right, y], TXT, Align::Right, state, ink);
                    y -= ROW;
                }
            }
        }
        Screen::Events(from) => {
            let pages = score.lines.len().div_ceil(EVENTS_PER_SCREEN);
            let page = from / EVENTS_PER_SCREEN + 1;
            let mut y = title(dl, &format!("MISSION EVENTS {page}/{pages}"));
            for l in score.lines.iter().skip(from).take(EVENTS_PER_SCREEN) {
                let time = format!("{:02}:{:02}", l.time_s / 60, l.time_s % 60);
                dl.text_mid([left, y], TXT, Align::Left, &format!("{time} {}", l.text), Ink::Hud);
                if l.points != 0 {
                    let ink = if l.points < 0 { Ink::HudWarning } else { Ink::Hud };
                    dl.text_mid([right, y], TXT, Align::Right, &format!("{:+}", l.points), ink);
                }
                y -= ROW;
            }
        }
        Screen::Rating => {
            let mut y = title(dl, "MISSION RATING");
            let mut row = |dl: &mut DisplayList, label: &str, value: String, ink: Ink| {
                dl.text_mid([left, y], TXT, Align::Left, label, ink);
                dl.text_mid([right, y], TXT, Align::Right, &value, ink);
                y -= ROW;
            };
            row(dl, "EVENTS", score.subtotal.to_string(), Ink::Hud);
            for (label, f) in &score.factors {
                row(dl, label, format!("X {f:.2}"), Ink::HudDim);
            }
            y -= ROW * 0.5;
            dl.line([left, y + ROW * 0.6], [right, y + ROW * 0.6], Ink::HudDim);
            dl.text_mid(
                [0.0, y - ROW * 0.5],
                TITLE,
                Align::Center,
                &format!("TOTAL {}", score.total),
                Ink::Hud,
            );
        }
    }
    let last = index + 1 >= all.len();
    let hint = if last { "SPACE - CONTINUE    R - RESTART" } else { "SPACE - NEXT    R - RESTART" };
    footer(dl, hint);
}

/// Halbe Breite der Textspalte in der Fenstermitte (höchstens so breit wie
/// hoch).
pub(crate) fn half_width(layout: &ScreenLayout) -> f32 {
    (layout.aspect * 0.5).min(0.65) * 0.9
}

/// Bedienhinweis unten.
pub(crate) fn footer(dl: &mut DisplayList, hint: &str) {
    dl.text_mid([0.0, -0.44], TXT, Align::Center, hint, Ink::HudDim);
}

/// Überschrift oben; liefert die Höhe der ersten Zeile darunter.
pub(crate) fn title(dl: &mut DisplayList, text: &str) -> f32 {
    let y = 0.40;
    dl.text_mid([0.0, y], TITLE, Align::Center, text, Ink::Hud);
    y - TITLE * 2.2
}
