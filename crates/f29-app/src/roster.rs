//! Dienstplan (M-036): die Datei im Benutzerordner (oder `--roster DATEI`),
//! der Bildschirm und seine Bedienung. Die Tabelle der zehn Plätze ist die
//! Auswahl: Bestätigen wählt einen aktiven Piloten oder legt auf einem leeren
//! Platz einen neuen an; Entf bzw. rechte Maustaste belegt einen Platz nach
//! Rückfrage neu. Einen Einsatz schreibt [`Duty::record`] erst beim Übergang
//! von der Nachbesprechung zum Dienstplan in die Akte – wer vorher beendet
//! (Escape, Alt+Q), verwirft ihn, wie im Original.

use crate::ui::Hit;
use bevy::prelude::*;
use f29_core::SimConfig;
use f29_core::roster::{Mission, NAME_LEN, Pilot, RANKS, Roster, SLOTS, Settings, Status};
use f29_hud::{Ink, Page, Row};
use std::path::PathBuf;

/// Was der Dienstplan gerade tut.
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    List,
    /// Meldung zur gewählten Zeile.
    Notice(String),
    /// Rückfrage vor dem Überschreiben; Wahl in der Auswahl (0: ersetzen).
    Replace(usize),
    /// Namenseingabe für den gewählten Platz.
    Name(String),
}

/// Eingaben eines Frames für den Dienstplan.
pub struct Keys {
    pub up: bool,
    pub down: bool,
    pub ok: bool,
    pub enter: bool,
    pub escape: bool,
    pub backspace: bool,
    /// Entf oder rechte Maustaste: Platz neu belegen.
    pub replace: bool,
    pub text: String,
    pub hit: Option<Hit>,
    pub click: bool,
}

#[derive(Resource)]
pub struct Duty {
    pub roster: Roster,
    /// Datei; `None`: nicht speichern (kein Ort oder unlesbare Datei).
    path: Option<PathBuf>,
    /// Pilot des laufenden Einsatzes.
    pub pilot: Option<usize>,
    /// Gewählte Zeile der Tabelle.
    pub cursor: usize,
    pub mode: Mode,
}

/// Üblicher Ort für Spieldaten des Benutzers.
fn default_path() -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let base = if cfg!(windows) {
        var("APPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local/share")))
    };
    Some(base?.join("f29-silent-sniper").join("roster.ron"))
}

impl Duty {
    /// Lädt den Dienstplan aus `path` bzw. vom üblichen Ort; fehlt die Datei,
    /// ist er leer. Eine unlesbare Datei bleibt unangetastet.
    pub fn load(path: Option<PathBuf>) -> Self {
        let mut path = path.or_else(default_path);
        let text = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
        let roster = match text.map(|t| Roster::from_ron(&t)) {
            Some(Ok(r)) => r,
            Some(Err(e)) => {
                eprintln!("Dienstplan unlesbar, wird nicht gespeichert: {e}");
                path = None;
                Roster::default()
            }
            None => Roster::default(),
        };
        let cursor = roster.current.unwrap_or(0);
        Duty { roster, path, pilot: None, cursor, mode: Mode::List }
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        let written = path.parent().map_or(Ok(()), std::fs::create_dir_all);
        if let Err(e) = written.and_then(|()| std::fs::write(path, self.roster.to_ron())) {
            eprintln!("Dienstplan nicht gespeichert ({}): {e}", path.display());
        }
    }

    /// Schreibt den Einsatz des laufenden Piloten in seine Akte und speichert.
    pub fn record(&mut self, m: &Mission) {
        if let Some(slot) = self.pilot.take() {
            self.roster.record(slot, m);
            self.cursor = slot;
            self.save();
        }
    }

    /// Ehrung des laufenden Piloten nach diesem Einsatz – an einer Kopie,
    /// die Akte bleibt bis [`Duty::record`] unberührt.
    pub fn honors(&self, m: &Mission) -> Option<Page> {
        let mut p = self.roster.slots[self.pilot?].clone()?;
        let r = p.fly(m);
        Some(f29_hud::debrief::honors(&p, &r))
    }

    /// Inhalt des Bildschirms.
    pub fn page(&self) -> Page {
        // Spalten: Platz, Rang, Name, Punkte, Einsätze; Status rechtsbündig.
        let head = format!("    {:<30}{:>12}{:>12}", "PILOT", "POINTS", "MISSIONS");
        let mut rows = vec![Row::new(head, Ink::HudDim).value("STATUS ")];
        for (k, slot) in self.roster.slots.iter().enumerate() {
            let (text, value) = match (slot, &self.mode) {
                (_, Mode::Name(name)) if k == self.cursor => {
                    (format!("{:>2}  {:<13}{name}_", k + 1, RANKS[0]), "")
                }
                (Some(p), _) => (
                    format!(
                        "{:>2}  {:<13}{:<17}{:>12}{:>12}",
                        k + 1,
                        p.rank_name(),
                        p.name,
                        p.points,
                        p.missions
                    ),
                    p.status.label(),
                ),
                (None, _) => (format!("{:>2}  -", k + 1), ""),
            };
            let ink = if slot.as_ref().is_some_and(|p| p.status == Status::Active) {
                Ink::Hud
            } else {
                Ink::HudDim
            };
            rows.push(Row::new(text, ink).value(format!("{value:<7}")).inverse(k == self.cursor));
        }
        rows.push(Row::new("", Ink::Hud));
        let name = self.roster.slots[self.cursor].as_ref().map_or("", |p| p.name.as_str());
        let (status, options, cursor) = match &self.mode {
            Mode::Notice(text) => (text.clone(), vec![], 0),
            Mode::Replace(choice) => {
                (format!("REPLACE {name}?"), vec!["REPLACE PILOT".into(), "CANCEL".into()], *choice)
            }
            Mode::List | Mode::Name(_) => (String::new(), vec![], 0),
        };
        rows.push(Row::new(status, Ink::HudWarning));
        Page { title: "DUTY ROSTER".into(), rows, options, cursor, marked: None }
    }

    /// Bedienung; liefert die Einstellungen des gewählten Piloten, sobald
    /// einer gewählt ist.
    pub fn input(&mut self, k: &Keys, config: &SimConfig) -> Option<Settings> {
        match &mut self.mode {
            Mode::Name(name) => {
                for c in k.text.chars().filter(|c| c.is_ascii_alphanumeric() || " -.".contains(*c)) {
                    if name.len() < NAME_LEN {
                        name.push(c.to_ascii_uppercase());
                    }
                }
                if k.backspace {
                    name.pop();
                }
                let name = name.trim().to_string();
                if k.escape {
                    self.mode = Mode::List;
                } else if k.enter && !name.is_empty() {
                    self.roster.slots[self.cursor] = Some(Pilot::new(&name, Settings::of(config)));
                    self.mode = Mode::List;
                    self.save();
                }
                None
            }
            Mode::Replace(choice) => {
                if let Some(Hit::Option(i)) = k.hit {
                    *choice = i;
                }
                if k.up || k.down {
                    *choice ^= 1;
                }
                if k.ok || (k.click && matches!(k.hit, Some(Hit::Option(_)))) {
                    self.mode = if *choice == 0 { Mode::Name(String::new()) } else { Mode::List };
                }
                None
            }
            Mode::List | Mode::Notice(_) => {
                let before = self.cursor;
                if let Some(Hit::Row(r)) = k.hit {
                    self.cursor = r;
                }
                if k.up {
                    self.cursor = (self.cursor + SLOTS - 1) % SLOTS;
                }
                if k.down {
                    self.cursor = (self.cursor + 1) % SLOTS;
                }
                if self.cursor != before {
                    self.mode = Mode::List;
                }
                let slot = &self.roster.slots[self.cursor];
                if k.replace {
                    self.mode = if slot.is_some() { Mode::Replace(1) } else { Mode::Name(String::new()) };
                    return None;
                }
                if !(k.ok || (k.click && matches!(k.hit, Some(Hit::Row(_))))) {
                    return None;
                }
                match slot {
                    None => self.mode = Mode::Name(String::new()),
                    Some(p) if p.status == Status::Active => {
                        let s = p.settings.clone();
                        (self.pilot, self.roster.current) = (Some(self.cursor), Some(self.cursor));
                        self.save();
                        return Some(s);
                    }
                    Some(p) => {
                        let why =
                            if p.status == Status::Killed { "WAS KILLED IN ACTION" } else { "HAS RETIRED" };
                        self.mode = Mode::Notice(format!("{} {why}.", p.name));
                    }
                }
                None
            }
        }
    }
}
