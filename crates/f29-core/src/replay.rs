//! Replays: Startkonfiguration + Eingabestrom + erwarteter End-Hash.
//!
//! Textformat (zeilenbasiert; die Startkonfiguration als eine RON-Zeile,
//! deren Fließkommazahlen exakt zurückgelesen werden):
//!
//! ```text
//! F29REPLAY 13
//! config (seed:3881,spawn:(kind:Air,fuel:1.0,…,landings:Realistic))
//! theater <prüfsumme> <name>
//! frames <anzahl Zeilen>
//! <wiederholungen> <pitch> <roll> <yaw> <throttle> <flags> <waffen>
//! ...
//! hash <hex>
//! ```
//!
//! Ältere Versionen werden nicht gelesen: Seit dem Wechsel hat sich das
//! Modell geändert, dieselben Eingaben ergäben einen anderen Flug.

use crate::controls::ControlFrame;
use crate::sim::{Sim, SimConfig, Spawn};
use f29_world::Theater;
use std::fmt::Write as _;
use std::sync::Arc;

pub const MAGIC: &str = "F29REPLAY 13";

/// Was das Replay von der Konfiguration speichert (das Gebiet nur als
/// Prüfsumme).
#[derive(serde::Serialize, serde::Deserialize)]
struct Header {
    seed: u64,
    spawn: Spawn,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Replay {
    pub config: SimConfig,
    /// Lauflängenkodiert: (Anzahl Ticks, Eingabe).
    pub frames: Vec<(u32, ControlFrame)>,
    /// Hash nach dem letzten Tick, falls bekannt.
    pub final_hash: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Replay Zeile {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

impl Replay {
    pub fn new(config: SimConfig) -> Self {
        Self { config, frames: Vec::new(), final_hash: None }
    }

    /// Hängt einen Tick an (fasst gleiche Eingaben zusammen).
    pub fn push(&mut self, frame: ControlFrame) {
        match self.frames.last_mut() {
            Some((n, f)) if *f == frame && *n < u32::MAX => *n += 1,
            _ => self.frames.push((1, frame)),
        }
    }

    pub fn tick_count(&self) -> u64 {
        self.frames.iter().map(|(n, _)| u64::from(*n)).sum()
    }

    /// Spielt das Replay ab und liefert die Simulation im Endzustand.
    pub fn run(&self) -> Sim {
        let mut sim = Sim::new(self.config.clone());
        for (n, f) in &self.frames {
            for _ in 0..*n {
                sim.step(*f);
            }
        }
        sim
    }

    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "{MAGIC}");
        let header = Header { seed: self.config.seed, spawn: self.config.spawn.clone() };
        let _ = writeln!(s, "config {}", ron::to_string(&header).expect("Konfiguration als RON"));
        let th = &self.config.theater;
        let _ = writeln!(s, "theater {:016x} {}", th.checksum(), th.name);
        let _ = writeln!(s, "frames {}", self.frames.len());
        for (n, f) in &self.frames {
            let _ =
                writeln!(s, "{n} {} {} {} {} {} {}", f.pitch, f.roll, f.yaw, f.throttle, f.flags, f.weapons);
        }
        if let Some(h) = self.final_hash {
            let _ = writeln!(s, "hash {h:016x}");
        }
        s
    }

    /// Liest ein Replay im eingebauten Gebiet.
    pub fn from_text(text: &str) -> Result<Self, ParseError> {
        Self::from_text_in(text, &[Arc::new(Theater::builtin().clone())])
    }

    /// Liest ein Replay; sein Gebiet muss unter `theaters` sein, mit
    /// derselben Prüfsumme wie bei der Aufnahme.
    pub fn from_text_in(text: &str, theaters: &[Arc<Theater>]) -> Result<Self, ParseError> {
        let mut lines = text.lines().enumerate().map(|(i, l)| (i + 1, l.trim()));
        let mut next = |what: &str| {
            lines.by_ref().find(|(_, l)| !l.is_empty()).ok_or_else(|| ParseError {
                line: 0,
                message: format!("unerwartetes Dateiende, erwartet: {what}"),
            })
        };
        let err = |line: usize, message: String| ParseError { line, message };

        let (ln, magic) = next("Kopfzeile")?;
        if magic != MAGIC {
            return Err(err(ln, format!("erwartet `{MAGIC}`")));
        }

        let (ln, l) = next("config")?;
        let header: Header = l
            .strip_prefix("config ")
            .ok_or_else(|| err(ln, "erwartet `config (…)`".into()))
            .and_then(|t| ron::from_str(t).map_err(|e| err(ln, format!("Konfiguration: {e}"))))?;
        let (ln, l) = next("theater")?;
        let theater = theaters
            .iter()
            .find(|t| l == format!("theater {:016x} {}", t.checksum(), t.name))
            .ok_or_else(|| err(ln, format!("Gebiet unbekannt oder verändert: `{l}`")))?
            .clone();

        let (ln, frames_line) = next("frames")?;
        let count = frames_line
            .strip_prefix("frames ")
            .and_then(|v| v.parse::<usize>().ok())
            .ok_or_else(|| err(ln, "erwartet `frames <anzahl>`".into()))?;

        let mut frames = Vec::with_capacity(count);
        for _ in 0..count {
            let (ln, l) = next("Eingabezeile")?;
            let t: Vec<&str> = l.split_whitespace().collect();
            let parse = || -> Option<(u32, ControlFrame)> {
                if t.len() != 7 {
                    return None;
                }
                Some((
                    t[0].parse().ok()?,
                    ControlFrame {
                        pitch: t[1].parse().ok()?,
                        roll: t[2].parse().ok()?,
                        yaw: t[3].parse().ok()?,
                        throttle: t[4].parse().ok()?,
                        flags: t[5].parse().ok()?,
                        weapons: t[6].parse().ok()?,
                    },
                ))
            };
            frames.push(parse().ok_or_else(|| err(ln, format!("ungültige Eingabezeile `{l}`")))?);
        }

        let final_hash = match next("hash") {
            Ok((ln, l)) => Some(
                l.strip_prefix("hash ")
                    .and_then(|v| u64::from_str_radix(v, 16).ok())
                    .ok_or_else(|| err(ln, "erwartet `hash <hex>`".into()))?,
            ),
            Err(_) => None,
        };

        let config = SimConfig { theater, ..SimConfig::with(header.seed, header.spawn) };
        Ok(Self { config, frames, final_hash })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapons::Loadout;

    #[test]
    fn text_roundtrip() {
        let mut r = Replay::new(SimConfig::new(77));
        r.config.spawn.heading = 1.234_567_890_123;
        r.config.spawn.altitude = 0.1 + 0.2;
        r.config.spawn.north = -1.0 / 3.0 * 1e5;
        for i in 0..300 {
            r.push(ControlFrame::from_analog(if i < 100 { 0.5 } else { 0.0 }, -0.25, 0.0, 0.8, 0));
        }
        r.final_hash = Some(0xdead_beef_0123_4567);
        assert_eq!(r.frames.len(), 2);
        assert_eq!(r.tick_count(), 300);
        let parsed = Replay::from_text(&r.to_text()).expect("parse");
        assert_eq!(parsed, r);
        let mut rw = Replay::new(SimConfig::runway(5));
        rw.config.spawn.fuel = 0.4;
        rw.config.spawn.loadout = Loadout::parse("arm,-,ship,cam");
        rw.config.spawn.night = true;
        assert_eq!(Replay::from_text(&rw.to_text()).expect("parse"), rw);
        rw.config.spawn.loadout = None;
        rw.config.spawn.night = true;
        rw.config.spawn.awacs = false;
        rw.config.spawn.visible = true;
        rw.config.spawn.invulnerable = true;
        rw.push(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.5, 0).with_weapons(3));
        assert_eq!(Replay::from_text(&rw.to_text()).expect("parse"), rw);
    }

    #[test]
    fn rejects_garbage() {
        assert!(Replay::from_text("hallo").is_err());
        assert!(Replay::from_text("F29REPLAY 5\nseed 9\n").is_err(), "alte Version");
        assert!(Replay::from_text(&format!("{MAGIC}\nconfig (seed:x)\n")).is_err());
    }

    /// Ein Replay aus einem anderen Gebiet findet sein Gebiet unter den
    /// bekannten wieder; im eingebauten allein ist es unbekannt.
    #[test]
    fn finds_its_theater_by_name_and_checksum() {
        let other = Arc::new(Theater { name: "OTHER".into(), ..Theater::builtin().clone() });
        let r = Replay::new(SimConfig { theater: other.clone(), ..SimConfig::new(3) });
        let text = r.to_text();
        assert!(Replay::from_text(&text).is_err());
        let builtin = Arc::new(Theater::builtin().clone());
        assert_eq!(
            Replay::from_text_in(&text, &[builtin, other]).expect("gefunden").config.theater.name,
            "OTHER"
        );
    }
}
