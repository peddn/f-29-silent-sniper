//! Menübildschirme vor dem Flug (M-036): Überschrift, Textzeilen und eine
//! Auswahlliste mit Cursor, übers ganze Fenster wie die Nachbesprechung.
//! Welche Bildschirme es gibt und was eine Wahl bewirkt, bestimmt das
//! Frontend.

use crate::ScreenLayout;
use crate::canvas::{Align, DisplayList, Ink};
use crate::debrief::{ROW, TXT, footer, half_width, title};

/// Inhalt eines Menübildschirms.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Page {
    pub title: String,
    pub lines: Vec<String>,
    pub options: Vec<String>,
    /// Gewählte Zeile der Auswahlliste.
    pub cursor: usize,
}

/// Zeichnet eine Seite: Text links in der Spalte, darunter die Auswahl
/// zentriert, die gewählte Zeile hell und mit Pfeilen markiert.
pub fn draw(layout: &ScreenLayout, page: &Page) -> DisplayList {
    let mut dl = DisplayList::new();
    let left = -half_width(layout);
    let mut y = title(&mut dl, &page.title);
    for line in &page.lines {
        dl.text_mid([left, y], TXT, Align::Left, line, Ink::Hud);
        y -= ROW;
    }
    if !page.lines.is_empty() {
        y -= ROW;
    }
    for (k, option) in page.options.iter().enumerate() {
        let (text, ink) = if k == page.cursor {
            (format!("> {option} <"), Ink::Hud)
        } else {
            (option.clone(), Ink::HudDim)
        };
        dl.text_mid([0.0, y], TXT, Align::Center, &text, ink);
        y -= ROW;
    }
    let hint = if page.options.len() > 1 { "UP/DOWN - SELECT    SPACE - OK" } else { "SPACE - CONTINUE" };
    footer(&mut dl, hint);
    dl
}
