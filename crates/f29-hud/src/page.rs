//! Inhalt der Bildschirme außerhalb des Cockpits (Menüs M-036,
//! Nachbesprechung M-032): Überschrift, ein Textfeld fester Größe und
//! darunter die Auswahl, immer an derselben Stelle. Gezeichnet wird im
//! Frontend; welche Bildschirme es gibt und was eine Wahl bewirkt, bestimmt
//! es ebenfalls.

use crate::canvas::Ink;

/// Höchstens so viele Zeilen hat eine Auswahl.
pub const MAX_OPTIONS: usize = 4;

/// Inhalt eines Bildschirms.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Page {
    pub title: String,
    pub rows: Vec<Row>,
    pub options: Vec<String>,
    /// Gewählte Zeile der Auswahl.
    pub cursor: usize,
    /// Zeile, die gedämpft invers bleibt, solange der Cursor anderswo steht
    /// (z. B. das gezeigte Thema der Lagebesprechung).
    pub marked: Option<usize>,
}

/// Eine Zeile des Textfelds: Text links, Wert rechtsbündig.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub text: String,
    pub value: String,
    pub ink: Ink,
    /// Doppelt groß und mittig (Ausgang, Gesamtwertung).
    pub big: bool,
    /// Invers hinterlegt (gewählte Zeile einer Tabelle, z. B. im Dienstplan).
    pub inverse: bool,
}

impl Row {
    pub fn new(text: impl Into<String>, ink: Ink) -> Self {
        Self { text: text.into(), value: String::new(), ink, big: false, inverse: false }
    }

    pub fn value(self, value: impl Into<String>) -> Self {
        Self { value: value.into(), ..self }
    }

    pub fn big(self) -> Self {
        Self { big: true, ..self }
    }

    pub fn inverse(self, on: bool) -> Self {
        Self { inverse: on, ..self }
    }
}

impl From<String> for Row {
    fn from(text: String) -> Self {
        Row::new(text, Ink::Hud)
    }
}
