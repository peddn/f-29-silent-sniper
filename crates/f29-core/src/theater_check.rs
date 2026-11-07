//! Prüfung eines Einsatzgebiets (M-031): Befunde und Kennzahlen für den
//! Karteneditor und die Werkzeuge. Rein und deterministisch; sie nutzt die
//! Regeln des Spiels selbst – Missionsgenerator, Radare, Gelände –, damit
//! Editor und Spiel dieselben Zahlen sehen. Was eine Datei unlesbar macht,
//! prüft schon `Theater::parse`.

use crate::mission;
use crate::radar;
use crate::rules::Conflict;
use crate::targets::{self, TargetClass};
use f29_world::theater::SIZE_M;
use f29_world::{AirfieldKind, Runway, SEA_LEVEL_M, Side, Terrain, Theater};
use std::collections::BTreeSet;

/// Feindliche Radare mindestens so weit von der eigenen Basis (m): Schon das
/// Anrollen bleibt unentdeckt.
pub const RADAR_MIN_HOME_M: f64 = 35_000.0;
/// Objekte mindestens so weit voneinander (m).
pub const OBJECT_MIN_APART_M: f64 = 120.0;
/// Objekte mindestens so weit von der Bahn der eigenen Basis (m): Start und
/// Anflug bleiben frei.
pub const OBJECT_MIN_HOME_RUNWAY_M: f64 = 2_500.0;
/// Objekte mindestens so weit von feindlichen Bahnen (m): Schutzbauten und
/// abgestellte Flugzeuge dürfen daneben stehen, nicht darauf.
pub const OBJECT_MIN_ENEMY_RUNWAY_M: f64 = 500.0;
/// Flugplätze mindestens so weit voneinander (m), sonst überlappen ihre
/// Einebnungen.
pub const AIRFIELD_MIN_APART_M: f64 = 5_000.0;
/// So viele Seeds probiert die Missionsprüfung je Konfliktstufe.
pub const MISSION_SEEDS: u64 = 200;
/// Weniger verschiedene Zielpaare sind eine Warnung.
pub const FEW_PAIRS: usize = 10;
/// Gitterweite für die Radarabdeckung (m).
const COVERAGE_STEP_M: f64 = 2_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Spielt sich falsch oder gar nicht.
    Warning,
    /// Ungewöhnlich, aber möglich.
    Hint,
}

/// Worauf sich ein Befund bezieht: Index in `theater.airfields` bzw.
/// `theater.objects`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Airfield(usize),
    Object(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub level: Level,
    /// Kurz, auf Deutsch, ohne den Namen des eigenen Eintrags.
    pub text: String,
    pub item: Option<Item>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stats {
    /// Verschiedene gültige Zielpaare aus [`MISSION_SEEDS`] Seeds je
    /// Konfliktstufe.
    pub pairs: [(Conflict, usize); 3],
    pub early_warning: usize,
    pub sam: usize,
    pub shorad: usize,
    pub enemy_airfields: usize,
    pub mig21: u32,
    pub mig29: u32,
    /// Mögliche Missionsziele im Band um die eigene Basis.
    pub band_targets: usize,
    /// Anteil (%) des feindlichen Teils des Missionsbands in Reichweite
    /// eines Frühwarnradars.
    pub ew_coverage_pct: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub stats: Stats,
}

impl Report {
    pub fn warnings(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|f| f.level == Level::Warning)
    }
}

fn km(m: f64) -> String {
    format!("{:.1} km", m / 1_000.0).replace('.', ",")
}

fn inside(p: [f64; 2]) -> bool {
    p[0].abs() <= SIZE_M * 0.5 && p[1].abs() <= SIZE_M * 0.5
}

/// Abstand eines Punkts von der befestigten Fläche einer Bahn (m).
fn runway_distance(r: &Runway, p: [f64; 2]) -> f64 {
    let (u, v) = r.local(p[0], p[1]);
    (u.abs() - r.length_m * 0.5).max(0.0).hypot((v.abs() - r.width_m * 0.5).max(0.0))
}

/// Prüft ein gelesenes Gebiet.
pub fn check(theater: &Theater) -> Report {
    let terrain = Terrain::from_theater(theater);
    let targets = targets::from_theater(theater, &terrain);
    let runways = terrain.runways();
    let home = runways.first().map_or([0.0, 0.0], |r| r.center);
    let dist = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    let mut findings = Vec::new();
    let mut note = |level, item, text: String| findings.push(Finding { level, text, item });
    let (warn, hint) = (Level::Warning, Level::Hint);

    // Flugplätze: im Gebiet, an Land (Höhe ohne Einebnung, die jede Bahn an
    // Land höbe), die eigene Basis auf eigener Seite, Horste mit Jägern.
    for (i, (a, r)) in theater.airfields.iter().zip(runways).enumerate() {
        let item = Some(Item::Airfield(i));
        let half = r.length_m * 0.5;
        let points = [r.center, r.world(-half, 0.0), r.world(half, 0.0)];
        if !points.iter().all(|&p| inside(p)) {
            note(warn, item, "Bahn liegt (teilweise) außerhalb des Gebiets".into());
        }
        if points.iter().any(|p| terrain.raw_height(p[0], p[1]) < SEA_LEVEL_M) {
            note(warn, item, "Bahn liegt (teilweise) im Wasser".into());
        }
        let side = |p: &[f64; 2]| theater.side_at(p[0], p[1]);
        match a.kind {
            AirfieldKind::Home if points.iter().any(|p| side(p) != Side::Friendly) => note(
                warn,
                item,
                "eigene Basis nicht ganz auf eigener Seite: eine Landung dort zählt nicht als Heimkehr"
                    .into(),
            ),
            AirfieldKind::Enemy => {
                if a.mig21 + a.mig29 == 0 {
                    note(hint, item, "feindlicher Horst ohne Jäger".into());
                }
                if side(&r.center) == Side::Friendly {
                    note(hint, item, "feindlicher Horst auf eigener Seite".into());
                }
            }
            _ => {}
        }
        for (other, o) in theater.airfields.iter().zip(runways).take(i) {
            let d = dist(r.center, o.center);
            if d < AIRFIELD_MIN_APART_M {
                note(hint, item, format!("nur {} von {}: die Einebnungen überlappen", km(d), other.name));
            }
        }
    }

    // Objekte: im Gebiet, Schiffe auf Wasser und alles andere an Land (nach
    // der Geländehöhe am Punkt), Radare weit genug von der Basis, Abstände.
    for (i, o) in theater.objects.iter().enumerate() {
        let item = Some(Item::Object(i));
        let at = [o.north, o.east];
        if !inside(at) {
            note(warn, item, "liegt außerhalb des Gebiets".into());
        }
        let ship = matches!(o.class, TargetClass::ShipSmall | TargetClass::ShipLarge);
        let wet = terrain.ground_height(o.north, o.east) < SEA_LEVEL_M;
        if ship && !wet {
            note(warn, item, "Schiff an Land".into());
        } else if !ship && wet {
            note(warn, item, "steht im Wasser".into());
        }
        if targets::air_defense(o.class).is_some() && dist(at, home) < RADAR_MIN_HOME_M {
            note(
                warn,
                item,
                format!(
                    "Radar nur {} von der eigenen Basis (mindestens {})",
                    km(dist(at, home)),
                    km(RADAR_MIN_HOME_M)
                ),
            );
        }
        for (a, r) in theater.airfields.iter().zip(runways) {
            let min = if r.kind == AirfieldKind::Home {
                OBJECT_MIN_HOME_RUNWAY_M
            } else {
                OBJECT_MIN_ENEMY_RUNWAY_M
            };
            let d = runway_distance(r, at);
            if d < min {
                note(warn, item, format!("nur {d:.0} m von der Bahn {} (mindestens {min:.0} m)", a.name));
            }
        }
        if !o.civilian && theater.side_at(o.north, o.east) != Side::Hostile {
            note(hint, item, "militärisches Objekt nicht auf feindlicher Seite".into());
        }
        for (j, other) in targets[..i].iter().enumerate() {
            let d = dist(at, [other.north, other.east]);
            if d < OBJECT_MIN_APART_M {
                note(warn, item, format!("nur {d:.0} m neben Objekt {j} ({})", other.name));
            }
        }
    }

    // Missionen mit dem Generator des Spiels: verschiedene gültige
    // Zielpaare je Konfliktstufe.
    let mut stats = Stats::default();
    for (k, conflict) in Conflict::ALL.into_iter().enumerate() {
        let pairs: BTreeSet<(usize, usize)> = (0..MISSION_SEEDS)
            .map(|seed| mission::plan(&terrain, &targets, seed, conflict))
            .filter(|m| m.is_complete(conflict))
            .filter_map(|m| Some((m.primary?, m.secondary?)))
            .map(|(a, b)| (a.min(b), a.max(b)))
            .collect();
        let label = conflict.rules().label;
        match pairs.len() {
            0 => note(warn, None, format!("{label}: keine Mission möglich")),
            n if n < FEW_PAIRS => note(warn, None, format!("{label}: nur {n} verschiedene Zielpaare")),
            _ => {}
        }
        stats.pairs[k] = (conflict, pairs.len());
    }

    // Kennzahlen.
    let count = |class| theater.objects.iter().filter(|o| o.class == class).count();
    stats.early_warning = count(TargetClass::EarlyWarning);
    stats.sam = count(TargetClass::AirDefense);
    stats.shorad = count(TargetClass::ShortRangeSam);
    let enemy = || theater.airfields.iter().filter(|a| a.kind == AirfieldKind::Enemy);
    stats.enemy_airfields = enemy().count();
    stats.mig21 = enemy().map(|a| u32::from(a.mig21)).sum();
    stats.mig29 = enemy().map(|a| u32::from(a.mig29)).sum();
    stats.band_targets = mission::candidates(&terrain, &targets).len();
    stats.ew_coverage_pct = ew_coverage(theater, home);
    Report { findings, stats }
}

/// Anteil (%) der feindlichen Gitterpunkte im Missionsband, die ein
/// Frühwarnradar erreicht.
fn ew_coverage(theater: &Theater, home: [f64; 2]) -> f64 {
    let ew: Vec<[f64; 2]> = theater
        .objects
        .iter()
        .filter(|o| o.class == TargetClass::EarlyWarning)
        .map(|o| [o.north, o.east])
        .collect();
    let steps = (mission::MAX_RANGE_M / COVERAGE_STEP_M) as i64;
    let (mut band, mut covered) = (0_u32, 0_u32);
    for i in -steps..=steps {
        for j in -steps..=steps {
            let p = [home[0] + i as f64 * COVERAGE_STEP_M, home[1] + j as f64 * COVERAGE_STEP_M];
            let d = (p[0] - home[0]).hypot(p[1] - home[1]);
            if !(mission::MIN_RANGE_M..=mission::MAX_RANGE_M).contains(&d)
                || theater.side_at(p[0], p[1]) != Side::Hostile
            {
                continue;
            }
            band += 1;
            covered +=
                u32::from(ew.iter().any(|e| (p[0] - e[0]).hypot(p[1] - e[1]) < radar::EARLY_WARNING.range_m));
        }
    }
    if band == 0 { 0.0 } else { 100.0 * f64::from(covered) / f64::from(band) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Das Testgebiet spielt sich ohne Warnung; Hinweise gibt es keine.
    #[test]
    fn test_range_is_clean() {
        let r = check(Theater::builtin());
        assert!(r.findings.is_empty(), "{:#?}", r.findings);
        let s = &r.stats;
        assert!(s.pairs.iter().all(|&(_, n)| n >= FEW_PAIRS), "{:?}", s.pairs);
        assert_eq!((s.early_warning, s.enemy_airfields, s.mig21, s.mig29), (4, 3, 9, 6));
        assert!(s.sam > 0 && s.shorad > 0 && s.band_targets > 0);
        assert!((0.0..=100.0).contains(&s.ew_coverage_pct) && s.ew_coverage_pct > 0.0);
    }

    /// Jede Prüfung schlägt an, wo sie soll, und nennt den Eintrag.
    #[test]
    fn flaws_are_found() {
        let mut t = Theater::builtin().clone();
        let home = [t.airfields[0].north, t.airfields[0].east];
        let ship = t.objects.iter().position(|o| o.class == TargetClass::ShipSmall).unwrap();
        let radar = t.objects.iter().position(|o| o.class == TargetClass::EarlyWarning).unwrap();
        let bunker = t.objects.iter().position(|o| o.class == TargetClass::Hardened).unwrap();
        [t.objects[ship].north, t.objects[ship].east] = home; // Schiff an Land, neben der Bahn
        [t.objects[radar].north, t.objects[radar].east] = [home[0] + 20_000.0, home[1]];
        t.objects[bunker].north = SIZE_M; // außerhalb
        t.airfields[1].mig21 = 0;
        t.airfields[1].mig29 = 0;
        let r = check(&t);
        let has =
            |item: Item, part: &str| r.findings.iter().any(|f| f.item == Some(item) && f.text.contains(part));
        assert!(has(Item::Object(ship), "Schiff an Land"));
        assert!(has(Item::Object(ship), "von der Bahn HOME"));
        assert!(has(Item::Object(radar), "Radar nur 20,0 km"));
        assert!(has(Item::Object(bunker), "außerhalb"));
        assert!(has(Item::Airfield(1), "ohne Jäger"));
        assert!(r.findings.iter().any(|f| f.level == Level::Hint), "Horst ohne Jäger ist ein Hinweis");
    }

    /// Ohne Missionsziele gibt es in keiner Konfliktstufe einen Auftrag.
    #[test]
    fn no_targets_no_missions() {
        let mut t = Theater::builtin().clone();
        t.objects.clear();
        let r = check(&t);
        assert_eq!(r.warnings().filter(|f| f.text.contains("keine Mission")).count(), 3);
        assert_eq!(r.stats.ew_coverage_pct, 0.0);
    }

    /// Schnell genug für den Editor, der nach jeder Änderung prüft.
    #[test]
    fn check_is_fast() {
        let start = std::time::Instant::now();
        check(Theater::builtin());
        // Großzügig, auch für Debug-Builds; im Release-Build sind es Millisekunden.
        assert!(start.elapsed().as_secs_f64() < 5.0, "{:?}", start.elapsed());
    }
}
