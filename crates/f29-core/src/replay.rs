//! Replays: Startkonfiguration + Eingabestrom + erwarteter End-Hash.
//!
//! Textformat (zeilenbasiert, Fließkommazahlen als Bitmuster in Hex, damit
//! die Startbedingungen exakt erhalten bleiben):
//!
//! ```text
//! F29REPLAY 9
//! seed 3881
//! spawn <air|runway> <north> <east> <altitude> <min_clearance> <heading> <speed> <throttle> <fuel>
//! loadout fall,chute,lgb,agm
//! time <day|night>
//! awacs <on|off>
//! stealth <on|off>
//! damage <on|off>
//! rules <cold|limited|war>
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
use crate::rules::Conflict;
use crate::sim::{Sim, SimConfig, Spawn, SpawnKind};
use crate::weapons::Loadout;
use f29_world::Theater;
use std::fmt::Write as _;

pub const MAGIC: &str = "F29REPLAY 9";

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
        let _ = writeln!(s, "loadout {}", sp.loadout.to_text());
        let _ = writeln!(s, "time {}", if sp.night { "night" } else { "day" });
        let _ = writeln!(s, "awacs {}", if sp.awacs { "on" } else { "off" });
        let _ = writeln!(s, "stealth {}", if sp.visible { "off" } else { "on" });
        let _ = writeln!(s, "damage {}", if sp.invulnerable { "off" } else { "on" });
        let _ = writeln!(s, "rules {}", sp.conflict.code());
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
        if magic != MAGIC {
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
        let kind = match tokens.next() {
            Some("air") => SpawnKind::Air,
            Some("runway") => SpawnKind::Runway,
            other => return Err(err(ln, format!("unbekannte Startart {other:?}"))),
        };
        let vals: Vec<f64> = tokens
            .map(|t| u64::from_str_radix(t, 16).map(f64::from_bits))
            .collect::<Result<_, _>>()
            .map_err(|e| err(ln, format!("ungültiger Hexwert: {e}")))?;
        let (north, east, altitude, min_clearance, heading, speed, throttle, fuel) = match vals[..] {
            [n, e, a, m, h, s, t, f] => (n, e, a, m, h, s, t, f),
            _ => return Err(err(ln, format!("spawn hat {} Werte", vals.len()))),
        };

        let (ln, l) = next("loadout")?;
        let loadout = l
            .strip_prefix("loadout ")
            .and_then(Loadout::parse)
            .ok_or_else(|| err(ln, "erwartet `loadout a,b,c,d`".into()))?;
        // Schalterzeile `<name> <wahr>|<falsch>`.
        let mut switch = |name: &str, yes: &str, no: &str| {
            let (ln, l) = next(name)?;
            match l.strip_prefix(name).and_then(|v| v.strip_prefix(' ')) {
                Some(v) if v == yes => Ok(true),
                Some(v) if v == no => Ok(false),
                _ => Err(err(ln, format!("erwartet `{name} {yes}|{no}`"))),
            }
        };
        let night = switch("time", "night", "day")?;
        let awacs = switch("awacs", "on", "off")?;
        let visible = switch("stealth", "off", "on")?;
        let invulnerable = switch("damage", "off", "on")?;
        let (ln, l) = next("rules")?;
        let conflict = l
            .strip_prefix("rules ")
            .and_then(Conflict::from_code)
            .ok_or_else(|| err(ln, "erwartet `rules cold|limited|war`".into()))?;
        // Bisher nur das eingebaute Gebiet; die Prüfsumme sichert, dass es
        // dasselbe ist wie bei der Aufnahme.
        let (ln, l) = next("theater")?;
        let builtin = Theater::builtin();
        let expected = format!("theater {:016x} {}", builtin.checksum(), builtin.name);
        if l != expected {
            return Err(err(ln, format!("Gebiet unbekannt oder verändert: `{l}`")));
        }

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

        Ok(Self {
            config: SimConfig::with(
                seed,
                Spawn {
                    kind,
                    fuel,
                    north,
                    east,
                    altitude,
                    min_clearance,
                    heading,
                    speed,
                    throttle,
                    loadout,
                    night,
                    awacs,
                    visible,
                    invulnerable,
                    conflict,
                },
            ),
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
        rw.config.spawn.loadout = Loadout::parse("arm,-,ship,fall").unwrap();
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
        assert!(Replay::from_text(&format!("{MAGIC}\nseed x\n")).is_err());
    }
}
