//! Was der Editor sich zwischen zwei Starts merkt: die zuletzt bearbeitete
//! Gebietsdatei und zu jeder Gebietsdatei ihre Unterlage. eframe legt es in
//! seinem Speicherordner ab (Linux `~/.local/share/f29-editor/app.ron`),
//! zusammen mit Fenstergröße und Panelbreiten.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FILE: &str = "datei";
const UNDERLAYS: &str = "unterlagen";

/// Pfade werden absolut gemerkt, damit der Start aus jedem Ordner passt.
pub fn absolute(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

#[derive(Debug, Default)]
pub struct Recent {
    /// Zuletzt bearbeitete Gebietsdatei.
    pub file: Option<PathBuf>,
    /// Gebietsdatei → Unterlage.
    underlays: BTreeMap<PathBuf, PathBuf>,
}

impl Recent {
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        let get = |key| storage.and_then(|s| s.get_string(key)).unwrap_or_default();
        let underlays = get(UNDERLAYS)
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(t, u)| (t.into(), u.into()))
            .collect();
        Self { file: Some(PathBuf::from(get(FILE))).filter(|f| !f.as_os_str().is_empty()), underlays }
    }

    /// Schreibt alles; Einträge zu verschwundenen Dateien fallen weg.
    pub fn store(&mut self, storage: &mut dyn eframe::Storage) {
        self.underlays.retain(|t, u| t.exists() && u.exists());
        let lines =
            self.underlays.iter().map(|(t, u)| format!("{}\t{}\n", t.display(), u.display())).collect();
        storage.set_string(FILE, self.file.as_ref().map_or_else(String::new, |f| f.display().to_string()));
        storage.set_string(UNDERLAYS, lines);
    }

    /// Die gemerkte Unterlage einer Gebietsdatei, falls sie noch existiert.
    pub fn underlay(&self, theater: &Path) -> Option<PathBuf> {
        self.underlays.get(&absolute(theater)).filter(|u| u.exists()).cloned()
    }

    pub fn set_underlay(&mut self, theater: &Path, underlay: &Path) {
        self.underlays.insert(absolute(theater), absolute(underlay));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Mem(HashMap<String, String>);

    impl eframe::Storage for Mem {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }

    #[test]
    fn remembers_file_and_underlays_and_drops_missing_ones() {
        let dir = std::env::temp_dir().join(format!("f29-editor-recent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("Ordner");
        let [a, b, png] = ["a.ron", "b.ron", "a.png"].map(|n| dir.join(n));
        for f in [&a, &png] {
            std::fs::write(f, "").expect("Datei");
        }

        let mut r = Recent::load(None);
        assert!(r.file.is_none() && r.underlay(&a).is_none());
        r.file = Some(a.clone());
        r.set_underlay(&a, &png);
        r.set_underlay(&b, &png); // b.ron gibt es nicht
        let mut mem = Mem::default();
        r.store(&mut mem);

        let back = Recent::load(Some(&mem));
        assert_eq!(back.file, Some(a.clone()));
        assert_eq!(back.underlay(&a), Some(png.clone()));
        assert_eq!(back.underlays.len(), 1, "Eintrag zu b.ron entfernt");
        std::fs::remove_file(&png).expect("löschen");
        assert_eq!(back.underlay(&a), None, "Unterlage verschwunden");
        std::fs::remove_dir_all(&dir).ok();
    }
}
