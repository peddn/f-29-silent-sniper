//! Replays: Startkonfiguration + Eingabestrom + erwarteter End-Hash.
//!
//! Textformat (zeilenbasiert, Fließkommazahlen als Bitmuster in Hex, damit
//! die Startbedingungen exakt erhalten bleiben):
//!
//! ```text
//! F29REPLAY 2
//! seed 3881
//! spawn <air|runway> <north> <east> <altitude> <min_clearance> <heading> <speed> <throttle> <fuel>
//! frames <anzahl Zeilen>
//! <wiederholungen> <pitch> <roll> <yaw> <throttle> <flags>
//! ...
//! hash <hex>
//! ```
//!
//! Version 1 (ohne Startart und Tankfüllung) wird weiter gelesen: Luftstart,
//! volle Tanks. Ihre Hashes passen aber nicht mehr, weil das Flugmodell seitdem
//! Treibstoff verbraucht.

use crate::controls::ControlFrame;
use crate::sim::{Sim, SimConfig, Spawn, SpawnKind};
use std::fmt::Write as _;

pub const MAGIC: &str = "F29REPLAY 2";
const MAGIC_V1: &str = "F29REPLAY 1";

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
        let sp = &self.config.spawn;
        let hex = |v: f64| format!("{:016x}", v.to_bits());
        let _ = writeln!(s, "{MAGIC}");
        let _ = writeln!(s, "seed {}", self.config.seed);
        let _ = writeln!(
            s,
            "spawn {} {} {} {} {} {} {} {} {}",
            match sp.kind {
                SpawnKind::Air => "air",
                SpawnKind::Runway => "runway",
            },
            hex(sp.north),
            hex(sp.east),
            hex(sp.altitude),
            hex(sp.min_clearance),
            hex(sp.heading),
            hex(sp.speed),
            hex(sp.throttle),
            hex(sp.fuel)
        );
        let _ = writeln!(s, "frames {}", self.frames.len());
        for (n, f) in &self.frames {
            let _ = writeln!(s, "{n} {} {} {} {} {}", f.pitch, f.roll, f.yaw, f.throttle, f.flags);
        }
        if let Some(h) = self.final_hash {
            let _ = writeln!(s, "hash {h:016x}");
        }
        s
    }

    pub fn from_text(text: &str) -> Result<Self, ParseError> {
        let mut lines = text.lines().enumerate().map(|(i, l)| (i + 1, l.trim()));
        let mut next = |what: &str| {
            lines.by_ref().find(|(_, l)| !l.is_empty()).ok_or_else(|| ParseError {
                line: 0,
                message: format!("unerwartetes Dateiende, erwartet: {what}"),
            })
        };
        let err = |line: usize, message: String| ParseError { line, message };

        let (ln, magic) = next("Kopfzeile")?;
        let v1 = magic == MAGIC_V1;
        if magic != MAGIC && !v1 {
            return Err(err(ln, format!("erwartet `{MAGIC}`")));
        }

        let (ln, seed_line) = next("seed")?;
        let seed = seed_line
            .strip_prefix("seed ")
            .and_then(|v| v.parse::<u64>().ok())
            .ok_or_else(|| err(ln, "erwartet `seed <zahl>`".into()))?;

        let (ln, spawn_line) = next("spawn")?;
        let mut tokens = spawn_line
            .strip_prefix("spawn ")
            .ok_or_else(|| err(ln, "erwartet `spawn ...`".into()))?
            .split_whitespace()
            .peekable();
        let kind = if v1 {
            SpawnKind::Air
        } else {
            match tokens.next() {
                Some("air") => SpawnKind::Air,
                Some("runway") => SpawnKind::Runway,
                other => return Err(err(ln, format!("unbekannte Startart {other:?}"))),
            }
        };
        let vals: Vec<f64> = tokens
            .map(|t| u64::from_str_radix(t, 16).map(f64::from_bits))
            .collect::<Result<_, _>>()
            .map_err(|e| err(ln, format!("ungültiger Hexwert: {e}")))?;
        let (north, east, altitude, min_clearance, heading, speed, throttle, fuel) = match vals[..] {
            [n, e, a, m, h, s, t] if v1 => (n, e, a, m, h, s, t, 1.0),
            [n, e, a, m, h, s, t, f] if !v1 => (n, e, a, m, h, s, t, f),
            _ => return Err(err(ln, format!("spawn hat {} Werte", vals.len()))),
        };

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
                if t.len() != 6 {
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

        Ok(Self {
            config: SimConfig {
                seed,
                spawn: Spawn { kind, fuel, north, east, altitude, min_clearance, heading, speed, throttle },
            },
            frames,
            final_hash,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_roundtrip() {
        let mut r = Replay::new(SimConfig::new(77));
        r.config.spawn.heading = 1.234_567_890_123;
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
        assert_eq!(Replay::from_text(&rw.to_text()).expect("parse"), rw);
    }

    #[test]
    fn reads_version_1() {
        let v1 =
            "F29REPLAY 1\nseed 9\nspawn 0 0 40a3880000000000 0 0 406a400000000000 0\nframes 1\n5 0 0 0 0 0\n";
        let r = Replay::from_text(v1).expect("v1");
        assert_eq!(r.config.spawn.kind, SpawnKind::Air);
        assert_eq!(r.config.spawn.fuel, 1.0);
        assert_eq!(r.tick_count(), 5);
    }

    #[test]
    fn rejects_garbage() {
        assert!(Replay::from_text("hallo").is_err());
        assert!(Replay::from_text("F29REPLAY 1\nseed x\n").is_err());
    }
}
