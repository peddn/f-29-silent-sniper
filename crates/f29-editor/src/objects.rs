//! Flugplätze und Objekte bearbeiten, ohne Oberfläche: setzen mit Namen aus
//! dem Spiel und dem nächsten Ort aus der Ortsliste, verschieben, löschen,
//! duplizieren, finden.

use f29_world::theater::{AirfieldDef, ObjectDef, SIZE_M};
use f29_world::{AirfieldKind, TargetClass, Theater};
use std::path::Path;

/// Ein Flugplatz oder Objekt, als Index in `airfields` bzw. `objects`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Airfield(usize),
    Object(usize),
}

/// Was „Setzen“ anlegt: einen feindlichen Horst oder ein Objekt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Airfield,
    Object(TargetClass),
}

impl Kind {
    pub fn all() -> impl Iterator<Item = Kind> {
        std::iter::once(Kind::Airfield).chain(TargetClass::ALL.map(Kind::Object))
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Airfield => "AIRBASE (Horst)",
            Kind::Object(c) => c.label(),
        }
    }
}

/// Ein Ort aus der Ortsliste einer Unterlage (Meter).
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub name: String,
    pub north: f64,
    pub east: f64,
}

/// Liest die Orte (ohne Flugplätze) aus `<Unterlage>_orte.txt` neben der
/// Unterlage; fehlt die Datei, keine.
pub fn places_for(underlay: &Path) -> Vec<Place> {
    let Some(stem) = underlay.file_stem() else { return Vec::new() };
    let list = underlay.with_file_name(format!("{}_orte.txt", stem.to_string_lossy()));
    parse_places(&std::fs::read_to_string(list).unwrap_or_default())
}

/// Zeilen `ORT<Tab>NAME<Tab>Nord km<Tab>Ost km`; andere Zeilen (auch
/// `FLUGPLATZ`) zählen nicht.
pub fn parse_places(text: &str) -> Vec<Place> {
    let km = |s: &str| s.trim().parse::<f64>().ok().map(|v| v * 1e3);
    text.lines()
        .filter_map(|l| match l.split('\t').collect::<Vec<_>>()[..] {
            ["ORT", name, n, e] => Some(Place { name: name.trim().into(), north: km(n)?, east: km(e)? }),
            _ => None,
        })
        .collect()
}

/// Bis zu dieser Entfernung liegt ein Objekt „bei“ einem Ort.
const NEAR_M: f64 = 40_000.0;

/// Name des nächsten Orts bis 40 km, sonst leer.
pub fn nearest_place(places: &[Place], north: f64, east: f64) -> String {
    let d = |p: &&Place| (p.north - north).hypot(p.east - east);
    places
        .iter()
        .filter(|p| d(p) <= NEAR_M)
        .min_by(|a, b| d(a).total_cmp(&d(b)))
        .map_or_else(String::new, |p| p.name.clone())
}

/// Erster Name aus den Listen des Spiels (zivil, sofern es zivile gibt).
pub fn default_name(class: TargetClass, civilian: bool) -> &'static str {
    let (mil, civ) = class.names();
    let list = if civilian && !civ.is_empty() { civ } else { mil };
    list.first().copied().unwrap_or(class.label())
}

/// Ändert Klasse und zivil; trug das Objekt den Standardnamen der alten
/// Klasse, bekommt es den der neuen.
pub fn retype(o: &mut ObjectDef, class: TargetClass, civilian: bool) {
    let (mil, civ) = o.class.names();
    if mil.iter().chain(civ).any(|n| *n == o.name) || o.name == o.class.label() {
        o.name = default_name(class, civilian).into();
    }
    o.class = class;
    o.civilian = civilian;
}

/// Setzt ein neues Element an `(north, east)` und gibt es zurück.
pub fn add(t: &mut Theater, kind: Kind, civilian: bool, north: f64, east: f64, places: &[Place]) -> Item {
    let near = nearest_place(places, north, east);
    let it = match kind {
        Kind::Airfield => {
            t.airfields.push(AirfieldDef {
                name: if near.is_empty() { "AIRBASE".into() } else { near },
                kind: AirfieldKind::Enemy,
                north,
                east,
                heading_deg: 0.0,
                mig21: 2,
                mig29: 0,
            });
            Item::Airfield(t.airfields.len() - 1)
        }
        Kind::Object(class) => {
            t.objects.push(ObjectDef {
                name: default_name(class, civilian).into(),
                class,
                north,
                east,
                heading_deg: 0.0,
                civilian,
                place: near,
            });
            Item::Object(t.objects.len() - 1)
        }
    };
    set_pos(t, it, [north, east]);
    it
}

pub fn exists(t: &Theater, it: Item) -> bool {
    match it {
        Item::Airfield(i) => i < t.airfields.len(),
        Item::Object(i) => i < t.objects.len(),
    }
}

/// Alle Elemente: erst Flugplätze, dann Objekte.
pub fn all(t: &Theater) -> impl Iterator<Item = Item> {
    (0..t.airfields.len()).map(Item::Airfield).chain((0..t.objects.len()).map(Item::Object))
}

/// Lage (Nord, Ost).
pub fn pos(t: &Theater, it: Item) -> [f64; 2] {
    match it {
        Item::Airfield(i) => [t.airfields[i].north, t.airfields[i].east],
        Item::Object(i) => [t.objects[i].north, t.objects[i].east],
    }
}

/// Verschiebt auf ganze Meter, aber nicht aus dem Gebiet hinaus.
pub fn set_pos(t: &mut Theater, it: Item, [north, east]: [f64; 2]) {
    let c = |v: f64| v.round().clamp(-SIZE_M * 0.5, SIZE_M * 0.5);
    match it {
        Item::Airfield(i) => (t.airfields[i].north, t.airfields[i].east) = (c(north), c(east)),
        Item::Object(i) => (t.objects[i].north, t.objects[i].east) = (c(north), c(east)),
    }
}

/// Angezeigter Name.
pub fn label(t: &Theater, it: Item) -> String {
    match it {
        Item::Airfield(i) => t.airfields[i].name.clone(),
        Item::Object(i) => t.objects[i].label(),
    }
}

/// Löscht; die eigene Basis (erster Flugplatz) bleibt.
pub fn remove(t: &mut Theater, it: Item) -> bool {
    match it {
        Item::Airfield(0) => return false,
        Item::Airfield(i) => drop(t.airfields.remove(i)),
        Item::Object(i) => drop(t.objects.remove(i)),
    }
    true
}

/// Kopie 1 km östlich; die eigene Basis gibt es nur einmal.
pub fn duplicate(t: &mut Theater, it: Item) -> Option<Item> {
    let copy = match it {
        Item::Airfield(0) => return None,
        Item::Airfield(i) => {
            t.airfields.push(t.airfields[i].clone());
            Item::Airfield(t.airfields.len() - 1)
        }
        Item::Object(i) => {
            t.objects.push(t.objects[i].clone());
            Item::Object(t.objects.len() - 1)
        }
    };
    let [n, e] = pos(t, copy);
    set_pos(t, copy, [n, e + 1_000.0]);
    Some(copy)
}

/// Nächstes Element bis `max_m` um `(north, east)`.
pub fn nearest(t: &Theater, north: f64, east: f64, max_m: f64) -> Option<Item> {
    let d = |it: &Item| {
        let [n, e] = pos(t, *it);
        (n - north).hypot(e - east)
    };
    all(t).filter(|it| d(it) <= max_m).min_by(|a, b| d(a).total_cmp(&d(b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = "ORT\tBANGHAZI\t80.0\t-50.4\nFLUGPLATZ\tBENINA INT'L (BEN)\t76.5\t-31.3\nORT\tDARNAH\t153.6\t190.7\nkaputt\n";

    #[test]
    fn place_list_and_nearest_town() {
        let p = parse_places(LIST);
        assert_eq!(p.len(), 2, "nur Orte, keine Flugplätze");
        assert_eq!(p[0], Place { name: "BANGHAZI".into(), north: 80_000.0, east: -50_400.0 });
        assert_eq!(nearest_place(&p, 70_000.0, -40_000.0), "BANGHAZI");
        assert_eq!(nearest_place(&p, 0.0, 0.0), "", "weiter als 40 km");
        assert_eq!(nearest_place(&[], 0.0, 0.0), "");
    }

    #[test]
    fn new_items_get_game_names_and_places() {
        let places = parse_places(LIST);
        let mut t = Theater::empty("T", 1);
        let radar = add(&mut t, Kind::Object(TargetClass::EarlyWarning), false, 75_000.0, -45_000.0, &places);
        assert_eq!(radar, Item::Object(0));
        assert_eq!(label(&t, radar), "EW RADAR AT BANGHAZI");
        let ship = add(&mut t, Kind::Object(TargetClass::ShipLarge), true, 0.0, 0.0, &places);
        assert_eq!(label(&t, ship), "FREIGHTER");
        let base = add(&mut t, Kind::Airfield, false, 150_000.0, 180_000.0, &places);
        assert_eq!((label(&t, base), t.airfields[1].kind), ("DARNAH".into(), AirfieldKind::Enemy));
        assert_eq!(Theater::parse(&t.to_text()).expect("lesbar"), t);
    }

    #[test]
    fn retype_renames_only_default_names() {
        let mut t = Theater::empty("T", 1);
        add(&mut t, Kind::Object(TargetClass::Building), false, 0.0, 0.0, &[]);
        retype(&mut t.objects[0], TargetClass::Building, true);
        assert_eq!(t.objects[0].name, "FACTORY");
        retype(&mut t.objects[0], TargetClass::Hardened, false);
        assert_eq!(t.objects[0].name, "BUNKER");
        t.objects[0].name = "GADDAFI BUNKER".into();
        retype(&mut t.objects[0], TargetClass::Building, false);
        assert_eq!(t.objects[0].name, "GADDAFI BUNKER", "eigener Name bleibt");
    }

    #[test]
    fn home_base_stays_unique_and_positions_are_clamped() {
        let mut t = Theater::empty("T", 1);
        assert!(!remove(&mut t, Item::Airfield(0)));
        assert_eq!(duplicate(&mut t, Item::Airfield(0)), None);
        let o = add(&mut t, Kind::Object(TargetClass::Bridge), false, 10.0, 20.0, &[]);
        let copy = duplicate(&mut t, o).expect("Kopie");
        assert_eq!(pos(&t, copy), [10.0, 1_020.0]);
        set_pos(&mut t, copy, [66_860.465, -39_244.5]);
        assert_eq!(pos(&t, copy), [66_860.0, -39_245.0], "ganze Meter");
        set_pos(&mut t, copy, [1e9, -1e9]);
        assert_eq!(pos(&t, copy), [SIZE_M * 0.5, -SIZE_M * 0.5]);
        assert!(remove(&mut t, o));
        assert_eq!(t.objects.len(), 1);
        assert!(exists(&t, Item::Object(0)) && !exists(&t, Item::Object(1)));
    }

    #[test]
    fn nearest_finds_the_closest_in_range() {
        let mut t = Theater::empty("T", 1);
        add(&mut t, Kind::Object(TargetClass::Soft), false, 1_000.0, 0.0, &[]);
        add(&mut t, Kind::Object(TargetClass::Soft), false, 3_000.0, 0.0, &[]);
        assert_eq!(nearest(&t, 2_600.0, 0.0, 500.0), Some(Item::Object(1)));
        assert_eq!(nearest(&t, 200.0, 0.0, 500.0), Some(Item::Airfield(0)));
        assert_eq!(nearest(&t, 2_000.0, 0.0, 500.0), None);
        assert_eq!(all(&t).count(), 3);
    }
}
