//! Einsatzgebiet als Daten (M-031): Grobraster, Flugplätze und alle Objekte.
//!
//! Eine Gebietsdatei (RON, lesbar und von Hand bearbeitbar) beschreibt ein
//! quadratisches Gebiet um den Ursprung:
//! - **Grobraster** `n × n`: je Zelle Gelände (`~` Wasser, `.` Flachland,
//!   `h` Hügel, `M` Gebirge) und Seite (`F` eigen, `N` neutral, `H`
//!   feindlich). Erste Zeile = Nordrand, erste Spalte = Westrand. Aus dem
//!   Gelände folgt die Großform der Landschaft, das Rauschen liefert die
//!   Feinheiten ([`crate::Terrain::from_theater`]).
//! - **Flugplätze** mit Art, Lage, Richtung und den Jägern feindlicher Horste.
//! - **Objekte**: alle Ziele des Gebiets mit Klasse, Lage und Ausrichtung; der
//!   Missionsgenerator wählt aus ihnen die Missionsziele.
//!
//! Koordinaten in Metern, Nord und Ost, Ursprung in der Gebietsmitte.

use crate::AirfieldKind;
use crate::class::TargetClass;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

/// Seite einer Rasterzelle bzw. eines Flugplatzes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Friendly,
    Neutral,
    Hostile,
}

impl Side {
    pub fn label(self) -> &'static str {
        match self {
            Side::Friendly => "FRIENDLY",
            Side::Neutral => "NEUTRAL",
            Side::Hostile => "HOSTILE",
        }
    }

    fn from_char(c: char) -> Option<Self> {
        match c {
            'F' => Some(Side::Friendly),
            'N' => Some(Side::Neutral),
            'H' => Some(Side::Hostile),
            _ => None,
        }
    }

    pub fn to_char(self) -> char {
        match self {
            Side::Friendly => 'F',
            Side::Neutral => 'N',
            Side::Hostile => 'H',
        }
    }
}

/// Geländeart einer Rasterzelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ground {
    Water,
    Plain,
    Hills,
    Mountains,
}

impl Ground {
    fn from_char(c: char) -> Option<Self> {
        match c {
            '~' => Some(Ground::Water),
            '.' => Some(Ground::Plain),
            'h' => Some(Ground::Hills),
            'M' => Some(Ground::Mountains),
            _ => None,
        }
    }

    pub fn to_char(self) -> char {
        match self {
            Ground::Water => '~',
            Ground::Plain => '.',
            Ground::Hills => 'h',
            Ground::Mountains => 'M',
        }
    }
}

/// Grobraster als Textzeilen (eine Zeile je Rasterzeile, Nord zuerst).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    pub ground: Vec<String>,
    pub side: Vec<String>,
}

/// Ein Flugplatz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AirfieldDef {
    pub name: String,
    pub kind: AirfieldKind,
    pub north: f64,
    pub east: f64,
    /// Bahnrichtung (Grad, rechtweisend); gerundet auf 22,5°-Schritte.
    pub heading_deg: f64,
    /// Feindlicher Horst: Jäger MiG-21 und MiG-29 (M-022).
    #[serde(default)]
    pub mig21: u8,
    #[serde(default)]
    pub mig29: u8,
}

/// Ein Objekt (Ziel) des Gebiets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectDef {
    pub name: String,
    pub class: TargetClass,
    pub north: f64,
    pub east: f64,
    /// Ausrichtung (Grad, rechtweisend).
    pub heading_deg: f64,
    /// Zivil: nicht angreifen.
    #[serde(default)]
    pub civilian: bool,
}

/// Ein Einsatzgebiet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Theater {
    pub name: String,
    /// Seed des Detailrauschens.
    pub seed: u64,
    /// Kantenlänge (m).
    pub size_m: f64,
    pub grid: Grid,
    pub airfields: Vec<AirfieldDef>,
    pub objects: Vec<ObjectDef>,
}

/// Fehler beim Lesen eines Gebiets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheaterError(pub String);

impl std::fmt::Display for TheaterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Einsatzgebiet: {}", self.0)
    }
}

impl std::error::Error for TheaterError {}

impl Theater {
    /// Liest ein Gebiet aus RON und prüft das Raster.
    pub fn parse(text: &str) -> Result<Self, TheaterError> {
        let t: Theater = ron::from_str(text).map_err(|e| TheaterError(e.to_string()))?;
        let n = t.grid.ground.len();
        let rows_ok = |rows: &[String], ok: fn(char) -> bool| {
            rows.len() == n && rows.iter().all(|r| r.chars().count() == n && r.chars().all(ok))
        };
        if n < 2
            || !rows_ok(&t.grid.ground, |c| Ground::from_char(c).is_some())
            || !rows_ok(&t.grid.side, |c| Side::from_char(c).is_some())
        {
            return Err(TheaterError("Raster muss n × n Zeichen je Ebene haben (~ . h M bzw. F N H)".into()));
        }
        if !t.airfields.iter().any(|a| a.kind == AirfieldKind::Home) {
            return Err(TheaterError("keine eigene Basis".into()));
        }
        Ok(t)
    }

    /// Als RON, gut lesbar.
    pub fn to_text(&self) -> String {
        let cfg = ron::ser::PrettyConfig::new().depth_limit(3);
        ron::ser::to_string_pretty(self, cfg).expect("Gebiet serialisierbar")
    }

    /// Prüfsumme (FNV-1a über den Text) – für Replays.
    pub fn checksum(&self) -> u64 {
        self.to_text()
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
    }

    /// Kantenlänge des Rasters in Zellen.
    pub fn grid_n(&self) -> usize {
        self.grid.ground.len()
    }

    /// Zelle (Zeile von Nord, Spalte von West) mit Anteilen 0..1 darin; außerhalb `None`.
    pub fn cell(&self, north: f64, east: f64) -> Option<(usize, usize)> {
        let n = self.grid_n() as f64;
        let half = self.size_m * 0.5;
        let row = ((half - north) / self.size_m * n).floor();
        let col = ((east + half) / self.size_m * n).floor();
        ((0.0..n).contains(&row) && (0.0..n).contains(&col)).then_some((row as usize, col as usize))
    }

    /// Gelände der Zelle `(Zeile, Spalte)`.
    pub fn ground(&self, row: usize, col: usize) -> Ground {
        Ground::from_char(self.grid.ground[row].as_bytes()[col] as char).expect("geprüft")
    }

    /// Seite der Zelle `(Zeile, Spalte)`.
    pub fn side(&self, row: usize, col: usize) -> Side {
        Side::from_char(self.grid.side[row].as_bytes()[col] as char).expect("geprüft")
    }

    /// Seite an einem Ort; außerhalb des Gebiets neutral.
    pub fn side_at(&self, north: f64, east: f64) -> Side {
        self.cell(north, east).map_or(Side::Neutral, |(r, c)| self.side(r, c))
    }

    /// Das eingebaute Standardgebiet.
    pub fn builtin() -> &'static Theater {
        static T: OnceLock<Theater> = OnceLock::new();
        T.get_or_init(|| {
            Theater::parse(include_str!("../../../data/theaters/north_coast.ron"))
                .expect("eingebautes Gebiet lesbar")
        })
    }
}

/// Text mit statischer Lebensdauer (Namen aus Gebietsdateien), je Inhalt
/// nur einmal angelegt.
pub fn intern(s: &str) -> &'static str {
    static SET: OnceLock<Mutex<std::collections::BTreeSet<&'static str>>> = OnceLock::new();
    let mut set = SET.get_or_init(Default::default).lock().expect("intern");
    if let Some(&x) = set.get(s) {
        return x;
    }
    let x: &'static str = Box::leak(s.to_owned().into_boxed_str());
    set.insert(x);
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> Theater {
        Theater {
            name: "TINY".into(),
            seed: 1,
            size_m: 40_000.0,
            grid: Grid { ground: vec!["~.".into(), "hM".into()], side: vec!["FN".into(), "NH".into()] },
            airfields: vec![AirfieldDef {
                name: "HOME".into(),
                kind: AirfieldKind::Home,
                north: 0.0,
                east: 0.0,
                heading_deg: 90.0,
                mig21: 0,
                mig29: 0,
            }],
            objects: vec![ObjectDef {
                name: "BUNKER".into(),
                class: TargetClass::Hardened,
                north: 5.0,
                east: -5.0,
                heading_deg: 0.0,
                civilian: false,
            }],
        }
    }

    #[test]
    fn text_roundtrip_and_cells() {
        let t = tiny();
        let back = Theater::parse(&t.to_text()).expect("lesbar");
        assert_eq!(back, t);
        assert_eq!(back.checksum(), t.checksum());
        // Nordwesten Wasser/eigen, Südosten Gebirge/feindlich
        assert_eq!(t.cell(10_000.0, -10_000.0), Some((0, 0)));
        assert_eq!(t.ground(0, 0), Ground::Water);
        assert_eq!(t.side_at(-10_000.0, 10_000.0), Side::Hostile);
        assert_eq!(t.side_at(90_000.0, 0.0), Side::Neutral, "außerhalb");
        assert!(Theater::parse(&t.to_text().replace("\"hM\"", "\"hX\"")).is_err());
        assert_eq!(intern("AB"), intern(&String::from("AB")));
    }

    #[test]
    fn builtin_theater_is_valid() {
        let t = Theater::builtin();
        assert!(t.grid_n() >= 16);
        assert!(!t.objects.is_empty());
    }
}
