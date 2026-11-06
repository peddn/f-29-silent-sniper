//! Einsatzgebiet als Daten (M-031): Grobraster, Flugplätze und alle Objekte.
//! Das Format beschreibt `docs/GEBIETSFORMAT.md` (Version [`FORMAT`]); die
//! Gebiete entstehen von Hand bzw. im Editor, das Spiel liest sie aus
//! [`THEATER_DIR`] ([`Theater::load_dir`]).
//!
//! Eine Gebietsdatei (RON, lesbar und von Hand bearbeitbar) beschreibt ein
//! Quadrat von [`SIZE_M`] um den Ursprung – wie im Original alle Gebiete
//! gleich groß:
//! - **Grobraster** [`GRID_N`] × [`GRID_N`]: je Zelle Gelände (`~` Wasser, `.` Flachland,
//!   `h` Hügel, `M` Gebirge, `d` Wüste, `f` Wald, `C` Stadt, `v` Dorf) und
//!   Seite (`F` eigen, `N` neutral, `H`
//!   feindlich). Erste Zeile = Nordrand, erste Spalte = Westrand. Aus dem
//!   Gelände folgt die Großform der Landschaft, das Rauschen liefert die
//!   Feinheiten ([`crate::Terrain::from_theater`]).
//! - **Flugplätze** mit Art, Lage, Richtung und den Jägern feindlicher Horste.
//! - **Objekte**: alle Ziele des Gebiets mit Klasse, Lage, Ausrichtung und
//!   Ort (angezeigt „NAME AT ORT“ wie im Original); der Missionsgenerator
//!   wählt aus ihnen die Missionsziele.
//!
//! Koordinaten in Metern, Nord und Ost, Ursprung in der Gebietsmitte.

use crate::AirfieldKind;
use crate::class::TargetClass;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// Version des Dateiformats, die dieses Spiel liest (3: feste Größe und
/// festes Raster).
pub const FORMAT: u32 = 3;
/// Kantenlänge jedes Gebiets (m).
pub const SIZE_M: f64 = 500_000.0;
/// Zellen je Zeile und Spalte des Rasters.
pub const GRID_N: usize = 128;
/// Kantenlänge einer Rasterzelle (m), knapp 4 km.
pub const CELL_M: f64 = SIZE_M / GRID_N as f64;
/// Ordner der Gebietsdateien, vom Arbeitsverzeichnis aus.
pub const THEATER_DIR: &str = "data/theaters";

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
    Desert,
    Forest,
    City,
    Village,
}

impl Ground {
    fn from_char(c: char) -> Option<Self> {
        match c {
            '~' => Some(Ground::Water),
            '.' => Some(Ground::Plain),
            'h' => Some(Ground::Hills),
            'M' => Some(Ground::Mountains),
            'd' => Some(Ground::Desert),
            'f' => Some(Ground::Forest),
            'C' => Some(Ground::City),
            'v' => Some(Ground::Village),
            _ => None,
        }
    }

    pub fn to_char(self) -> char {
        match self {
            Ground::Water => '~',
            Ground::Plain => '.',
            Ground::Hills => 'h',
            Ground::Mountains => 'M',
            Ground::Desert => 'd',
            Ground::Forest => 'f',
            Ground::City => 'C',
            Ground::Village => 'v',
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
    /// Ort (echter Name); angezeigt als „NAME AT ORT“, leer nur der Name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub place: String,
}

impl ObjectDef {
    /// Angezeigter Name: Name, mit Ort „NAME AT ORT“.
    pub fn label(&self) -> String {
        if self.place.is_empty() { self.name.clone() } else { format!("{} AT {}", self.name, self.place) }
    }
}

/// Ein Einsatzgebiet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Theater {
    /// Version des Dateiformats ([`FORMAT`]).
    pub format: u32,
    pub name: String,
    /// Seed des Detailrauschens.
    pub seed: u64,
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
        if t.format != FORMAT {
            return Err(TheaterError(format!("Formatversion {} statt {FORMAT}", t.format)));
        }
        let rows_ok = |rows: &[String], ok: fn(char) -> bool| {
            rows.len() == GRID_N && rows.iter().all(|r| r.chars().count() == GRID_N && r.chars().all(ok))
        };
        if !rows_ok(&t.grid.ground, |c| Ground::from_char(c).is_some())
            || !rows_ok(&t.grid.side, |c| Side::from_char(c).is_some())
        {
            return Err(TheaterError(format!(
                "Raster muss {GRID_N} × {GRID_N} Zeichen je Ebene haben (~ . h M d f C v bzw. F N H)"
            )));
        }
        if t.airfields.first().is_none_or(|a| a.kind != AirfieldKind::Home) {
            return Err(TheaterError("der erste Flugplatz muss die eigene Basis sein".into()));
        }
        Ok(t)
    }

    /// Alle Gebiete eines Ordners (`*.ron`), nach Name sortiert, dazu je
    /// unlesbarer Datei ein Fehler mit ihrem Pfad.
    pub fn load_dir(dir: &Path) -> (Vec<Theater>, Vec<TheaterError>) {
        let (mut found, mut errors) = (Vec::new(), Vec::new());
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "ron"))
            .collect();
        paths.sort();
        for p in paths {
            let text = std::fs::read_to_string(&p).map_err(|e| TheaterError(e.to_string()));
            match text.and_then(|t| Theater::parse(&t)) {
                Ok(t) => found.push(t),
                Err(e) => errors.push(TheaterError(format!("{}: {}", p.display(), e.0))),
            }
        }
        found.sort_by(|a, b| a.name.cmp(&b.name));
        (found, errors)
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
        cell(north, east).map_or(Side::Neutral, |(r, c)| self.side(r, c))
    }

    /// Das eingebaute Testgebiet TEST RANGE (alle Geländearten, Seiten und
    /// Objektklassen): Grundlage der Tests und Ersatz, wenn kein Gebiet
    /// geladen ist.
    pub fn builtin() -> &'static Theater {
        static T: OnceLock<Theater> = OnceLock::new();
        T.get_or_init(|| {
            Theater::parse(include_str!("../../../data/test/test_range.ron"))
                .expect("eingebautes Gebiet lesbar")
        })
    }
}

/// Rasterzelle (Zeile von Nord, Spalte von West) an einem Ort; außerhalb `None`.
pub fn cell(north: f64, east: f64) -> Option<(usize, usize)> {
    let row = ((SIZE_M * 0.5 - north) / CELL_M).floor();
    let col = ((east + SIZE_M * 0.5) / CELL_M).floor();
    let n = GRID_N as f64;
    ((0.0..n).contains(&row) && (0.0..n).contains(&col)).then_some((row as usize, col as usize))
}

/// Mitte der Rasterzelle `(Zeile, Spalte)` als (Nord, Ost).
pub fn cell_center(row: usize, col: usize) -> (f64, f64) {
    (SIZE_M * 0.5 - (row as f64 + 0.5) * CELL_M, (col as f64 + 0.5) * CELL_M - SIZE_M * 0.5)
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

    /// Ein Raster ganz aus `c`.
    fn layer(c: char) -> Vec<String> {
        vec![c.to_string().repeat(GRID_N); GRID_N]
    }

    /// Flachland, die Nordwestzelle Wasser, die Südreihe feindlich.
    fn tiny() -> Theater {
        let mut grid = Grid { ground: layer('.'), side: layer('N') };
        grid.ground[0].replace_range(..1, "~");
        grid.side[GRID_N - 1] = "H".repeat(GRID_N);
        Theater {
            format: FORMAT,
            name: "TINY".into(),
            seed: 1,
            grid,
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
                place: "TOBRUK".into(),
            }],
        }
    }

    #[test]
    fn text_roundtrip_and_cells() {
        let t = tiny();
        let text = t.to_text();
        let back = Theater::parse(&text).expect("lesbar");
        assert_eq!(back, t);
        assert_eq!(back.checksum(), t.checksum());
        let half = SIZE_M * 0.5;
        assert_eq!(cell(half - 1.0, 1.0 - half), Some((0, 0)));
        assert_eq!(cell_center(0, 0), (half - CELL_M * 0.5, CELL_M * 0.5 - half));
        assert_eq!(t.ground(0, 0), Ground::Water);
        assert_eq!(t.side_at(1.0 - half, 0.0), Side::Hostile);
        assert_eq!(t.side_at(half + 1.0, 0.0), Side::Neutral, "außerhalb");
        assert!(Theater::parse(&text.replacen("\"~", "\"X", 1)).is_err(), "unbekanntes Zeichen");
        let small = Theater {
            grid: Grid { ground: vec![".".repeat(64); 64], side: vec!["N".repeat(64); 64] },
            ..t.clone()
        };
        assert!(Theater::parse(&small.to_text()).is_err(), "falsches Raster");
        for other in ["format: 2", "format: 99"] {
            assert!(Theater::parse(&text.replace(&format!("format: {FORMAT}"), other)).is_err(), "{other}");
        }
        assert_eq!(t.objects[0].label(), "BUNKER AT TOBRUK");
        let enemy_first = Theater {
            airfields: vec![AirfieldDef { kind: AirfieldKind::Enemy, ..t.airfields[0].clone() }],
            ..t.clone()
        };
        assert!(Theater::parse(&enemy_first.to_text()).is_err(), "erst die eigene Basis");
        assert_eq!(intern("AB"), intern(&String::from("AB")));
    }

    /// Ein Ordner liefert seine lesbaren Gebiete nach Name und meldet die
    /// übrigen mit Pfad.
    #[test]
    fn load_dir_reads_all_and_reports_broken_files() {
        let dir = std::env::temp_dir().join(format!("f29-theaters-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut b = tiny();
        b.name = "ALPHA".into();
        std::fs::write(dir.join("z.ron"), b.to_text()).unwrap();
        std::fs::write(dir.join("a.ron"), tiny().to_text()).unwrap();
        std::fs::write(dir.join("kaputt.ron"), "(").unwrap();
        std::fs::write(dir.join("notiz.txt"), "kein Gebiet").unwrap();
        let (found, errors) = Theater::load_dir(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(found.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["ALPHA", "TINY"]);
        assert!(errors.len() == 1 && errors[0].0.contains("kaputt.ron"), "{errors:?}");
        assert!(Theater::load_dir(Path::new("gibt/es/nicht")).0.is_empty());
    }

    /// Das Testgebiet enthält alles, was die Tests brauchen: jede
    /// Geländeart, jede Seite, Horste mit Jägern, jede Objektklasse, Ziviles
    /// und Orte.
    #[test]
    fn test_range_has_everything() {
        let t = Theater::builtin();
        let cells = || (0..GRID_N * GRID_N).map(|k| (k / GRID_N, k % GRID_N));
        for c in "~.hMdfCv".chars() {
            assert!(cells().any(|(r, col)| t.ground(r, col).to_char() == c), "Gelände {c}");
        }
        for c in "FNH".chars() {
            assert!(cells().any(|(r, col)| t.side(r, col).to_char() == c), "Seite {c}");
        }
        assert!(t.airfields.iter().any(|a| a.kind == AirfieldKind::Enemy && a.mig21 > 0 && a.mig29 > 0));
        for class in TargetClass::ALL {
            assert!(t.objects.iter().any(|o| o.class == class && !o.civilian), "{class:?}");
        }
        assert!(t.objects.iter().any(|o| o.civilian));
        let ship = |c| matches!(c, TargetClass::ShipSmall | TargetClass::ShipLarge);
        assert!(t.objects.iter().all(|o| o.place.is_empty() == ship(o.class)), "Orte an Land");
    }
}
