//! Landschaftsvorschau: dasselbe Satellitenbild wie Karte und `theater-png`,
//! aus demselben Code (`Terrain::from_theater`, `MapRaster`), in einem
//! Hintergrund-Thread gerechnet.

use f29_render::map_raster::{MapRaster, MapStyle};
use f29_world::theater::SIZE_M;
use f29_world::{Terrain, Theater};
use std::sync::mpsc::{Receiver, Sender, channel};

/// Kantenlänge des Vorschaubilds (Pixel), knapp 0,5 km je Pixel.
pub const SIZE: usize = 1024;

/// Satellitenbild des ganzen Gebiets als RGB, `n × n`, Zeile 0 im Norden.
pub fn landscape(t: &Theater, n: usize) -> Vec<u8> {
    let map = MapRaster::build(&Terrain::from_theater(t), [0.0, 0.0], SIZE_M * 0.5, n, MapStyle::Satellite);
    // MapRaster zählt die Zeilen von Süden.
    (0..n).rev().flat_map(|i| map.pixels[i * n..(i + 1) * n].iter().flatten().copied()).collect()
}

/// Rechnet Vorschauen nacheinander; liegen mehrere Aufträge an, nur den neuesten.
#[derive(Debug)]
pub struct Worker {
    tx: Sender<Theater>,
    rx: Receiver<Vec<u8>>,
    /// Gerade in Arbeit.
    pub busy: bool,
}

impl Worker {
    /// Startet den Thread für Bilder von `n × n` Pixeln; `done` wird nach
    /// jedem fertigen Bild gerufen (Neuzeichnen anstoßen).
    pub fn new(n: usize, done: impl Fn() + Send + 'static) -> Self {
        let (tx, jobs) = channel::<Theater>();
        let (results, rx) = channel();
        std::thread::spawn(move || {
            while let Ok(mut t) = jobs.recv() {
                while let Ok(newer) = jobs.try_recv() {
                    t = newer;
                }
                if results.send(landscape(&t, n)).is_err() {
                    break;
                }
                done();
            }
        });
        Self { tx, rx, busy: false }
    }

    pub fn request(&mut self, t: &Theater) {
        self.busy = self.tx.send(t.clone()).is_ok();
    }

    /// Fertiges Bild, falls eines vorliegt.
    pub fn poll(&mut self) -> Option<Vec<u8>> {
        let img = self.rx.try_recv().ok()?;
        self.busy = false;
        Some(img)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use f29_world::Ground;
    use f29_world::theater::GRID_N;

    /// Norden oben: Ist die Nordhälfte Wasser, ist das Bild oben blau und unten nicht.
    #[test]
    fn landscape_has_north_up() {
        let mut t = Theater::empty("T", 3);
        for row in 0..GRID_N / 2 {
            for col in 0..GRID_N {
                t.set_ground(row, col, Ground::Water);
            }
        }
        t.airfields[0].north = -150_000.0;
        let n = 64;
        let img = landscape(&t, n);
        assert_eq!(img.len(), n * n * 3);
        let blue = |row: usize| {
            let p = &img[(row * n + n / 4) * 3..][..3];
            p[2] > p[0] && p[2] > p[1]
        };
        assert!(blue(5) && !blue(n - 5));
    }

    #[test]
    fn worker_delivers() {
        let mut w = Worker::new(32, || {});
        w.request(&Theater::empty("T", 1));
        assert!(w.busy);
        let img = loop {
            if let Some(img) = w.poll() {
                break img;
            }
            std::thread::yield_now();
        };
        assert_eq!(img.len(), 32 * 32 * 3);
        assert!(!w.busy);
    }
}
