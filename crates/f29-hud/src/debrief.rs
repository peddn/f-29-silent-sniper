//! Nachbesprechung (M-032): Nach dem Missionsende folgen wie im Original
//! eigene Bildschirme aufeinander – Ausgang, Ereignisse mit Punkten
//! (seitenweise), Wertung; fliegt ein Pilot aus dem Dienstplan, zuletzt die
//! Ehrung ([`honors`], M-030).

use crate::canvas::Ink;
use crate::page::{Page, Row};
use f29_core::roster::{self, MEDALS, PURPLE_HEART, Pilot, RANKS, RIBBONS, Report};
use f29_core::{FlightData, Score, Side, Status};

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

/// Bildschirm Nummer `index` (über das Ende hinaus: der letzte).
pub fn page(fd: &FlightData, index: usize) -> Page {
    let Some(score) = &fd.debrief else { return Page::default() };
    let all = screens(score);
    let (title, rows) = match all[index.min(all.len() - 1)] {
        Screen::Outcome => {
            let good = matches!(fd.status, Status::Landed { side: Side::Friendly, .. });
            let ink = if good { Ink::Hud } else { Ink::HudWarning };
            let mut rows = vec![Row::new(fd.status.outcome(), ink).big(), Row::new("", ink)];
            for (k, label) in ["PRIMARY", "SECONDARY"].into_iter().enumerate() {
                if let Some((task, done)) = fd.objectives[k] {
                    let ink = if done { Ink::Hud } else { Ink::HudDim };
                    let row = Row::new(format!("{label} {}", task.label()), ink);
                    rows.push(row.value(if done { "DONE" } else { "NOT DONE" }));
                }
            }
            ("MISSION DEBRIEF".to_string(), rows)
        }
        Screen::Events(from) => {
            let pages = score.lines.len().div_ceil(EVENTS_PER_SCREEN);
            let rows = score.lines.iter().skip(from).take(EVENTS_PER_SCREEN).map(|l| {
                let ink = if l.points < 0 { Ink::HudWarning } else { Ink::Hud };
                let text = format!("{:02}:{:02} {}", l.time_s / 60, l.time_s % 60, l.text);
                let points = if l.points == 0 { String::new() } else { format!("{:+}", l.points) };
                Row::new(text, ink).value(points)
            });
            (format!("MISSION EVENTS {}/{pages}", from / EVENTS_PER_SCREEN + 1), rows.collect())
        }
        Screen::Rating => {
            let mut rows = vec![Row::new("EVENTS", Ink::Hud).value(score.subtotal.to_string())];
            rows.extend(
                score
                    .factors
                    .iter()
                    .map(|(l, f)| Row::new(l.clone(), Ink::HudDim).value(format!("X {f:.2}"))),
            );
            rows.push(Row::new("", Ink::Hud));
            rows.push(Row::new(format!("TOTAL {}", score.total), Ink::Hud).big());
            ("MISSION RATING".to_string(), rows)
        }
    };
    Page { title, rows, options: vec!["CONTINUE".into()], cursor: 0, marked: None }
}

/// Ehrung (vorerst nur Text): was der Einsatz bewirkt hat, darunter alle
/// Auszeichnungen des Piloten, höchste zuerst. `p` ist die Akte danach.
pub fn honors(p: &Pilot, r: &Report) -> Page {
    let posthumous = if p.status == roster::Status::Killed { " POSTHUMOUSLY" } else { "" };
    let mut rows = Vec::new();
    if r.promoted {
        rows.push(Row::new(format!("PROMOTED{posthumous} TO"), Ink::Hud).value(p.rank_name()));
    }
    if r.delayed {
        let next = RANKS.get(p.rank + 1).copied().unwrap_or_default();
        rows.push(Row::new("PROMOTION DELAYED BY CAPTURE", Ink::HudWarning).value(next));
    }
    let awards: Vec<&str> =
        r.medal.map(|k| MEDALS[k].0).into_iter().chain(r.purple_heart.then_some(PURPLE_HEART)).collect();
    if !awards.is_empty() {
        rows.push(Row::new(format!("AWARDED{posthumous}"), Ink::Hud).value(awards.join(", ")));
    }
    if let Some(k) = r.ribbon {
        rows.push(Row::new(format!("{} MISSIONS", p.missions), Ink::Hud).value(RIBBONS[k].1));
    }
    if rows.is_empty() {
        rows.push(Row::new("NO PROMOTION OR DECORATION", Ink::HudDim));
    }
    if p.status == roster::Status::Retired {
        rows.push(Row::new("RETIRED FROM ACTIVE DUTY", Ink::Hud).value(format!("{} MISSIONS", p.missions)));
    }
    rows.extend([Row::new("", Ink::Hud), Row::new("DECORATIONS", Ink::HudDim)]);
    let held = rows.len();
    let medals = (0..MEDALS.len()).rev().filter(|&k| p.medals[k] > 0);
    rows.extend(medals.map(|k| Row::new(MEDALS[k].0, Ink::Hud).value(p.medals[k].to_string())));
    rows.extend(p.purple_heart.then(|| Row::new(PURPLE_HEART, Ink::Hud)));
    rows.extend(p.ribbons().map(|name| Row::new(name, Ink::Hud)));
    if rows.len() == held {
        rows.push(Row::new("NONE", Ink::HudDim));
    }
    let title = format!("{} {}", p.rank_name(), p.name);
    Page { title, rows, options: vec!["CONTINUE".into()], cursor: 0, marked: None }
}
