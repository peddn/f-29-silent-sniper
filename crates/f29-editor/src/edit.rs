//! Bearbeiten eines Gebiets ohne Oberfläche: Pinsel, Füllen, Wassermaske und
//! Verlauf zum Rückgängigmachen. Alles schreibt über `f29_world::Theater`.

use f29_world::theater::GRID_N;
use f29_world::{Ground, Side, Theater};

/// Was der Pinsel setzt: eine Geländeart oder eine Seite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    Ground(Ground),
    Side(Side),
}

impl Paint {
    /// Wert der Zelle in derselben Ebene wie `self`.
    pub fn at(self, t: &Theater, row: usize, col: usize) -> Paint {
        match self {
            Paint::Ground(_) => Paint::Ground(t.ground(row, col)),
            Paint::Side(_) => Paint::Side(t.side(row, col)),
        }
    }

    /// Setzt die Zelle; `true`, wenn sie sich änderte.
    fn set(self, t: &mut Theater, row: usize, col: usize) -> bool {
        if self.at(t, row, col) == self {
            return false;
        }
        match self {
            Paint::Ground(g) => t.set_ground(row, col, g),
            Paint::Side(s) => t.set_side(row, col, s),
        }
        true
    }
}

/// Malt einen runden Pinsel mit `size` Zellen Durchmesser um die Zelle
/// `(row, col)` (Kommazahlen: Lage im Raster); `true`, wenn sich etwas änderte.
pub fn dab(t: &mut Theater, p: Paint, row: f32, col: f32, size: usize) -> bool {
    let r = (size.max(1) as i32 - 1) / 2;
    let (r0, c0) = (row.floor() as i32, col.floor() as i32);
    let mut changed = false;
    for dr in -r..=r {
        for dc in -r..=r {
            let (rr, cc) = (r0 + dr, c0 + dc);
            let inside = (0..GRID_N as i32).contains(&rr) && (0..GRID_N as i32).contains(&cc);
            if inside && dr * dr + dc * dc <= r * r + r {
                changed |= p.set(t, rr as usize, cc as usize);
            }
        }
    }
    changed
}

/// Malt einen Strich von `from` nach `to` (Rasterlage Zeile, Spalte) ohne Lücken.
pub fn stroke(t: &mut Theater, p: Paint, from: [f32; 2], to: [f32; 2], size: usize) -> bool {
    let steps = ((to[0] - from[0]).abs().max((to[1] - from[1]).abs()) * 2.0).ceil().max(1.0) as usize;
    let mut changed = false;
    for k in 0..=steps {
        let f = k as f32 / steps as f32;
        changed |= dab(t, p, from[0] + (to[0] - from[0]) * f, from[1] + (to[1] - from[1]) * f, size);
    }
    changed
}

/// Füllt die zusammenhängende Fläche gleichen Werts (über Kanten verbunden)
/// ab `(row, col)`; gibt die Zahl der geänderten Zellen zurück.
pub fn fill(t: &mut Theater, p: Paint, row: usize, col: usize) -> usize {
    let old = p.at(t, row, col);
    if old == p {
        return 0;
    }
    let (mut todo, mut n) = (vec![(row, col)], 0);
    while let Some((r, c)) = todo.pop() {
        if p.at(t, r, c) != old {
            continue;
        }
        p.set(t, r, c);
        n += 1;
        if r > 0 {
            todo.push((r - 1, c));
        }
        if r + 1 < GRID_N {
            todo.push((r + 1, c));
        }
        if c > 0 {
            todo.push((r, c - 1));
        }
        if c + 1 < GRID_N {
            todo.push((r, c + 1));
        }
    }
    n
}

/// Übernimmt Wasser und Land aus einer Maske (Graustufen, Zeile 0 im Norden,
/// beliebige Größe über das ganze Gebiet): dunkle Zellen werden Wasser, helle
/// Wasserzellen Flachland, andere Landarten bleiben. Gibt die Zahl der
/// geänderten Zellen zurück.
pub fn water_from_mask(t: &mut Theater, size: [usize; 2], gray: &[u8]) -> usize {
    let [w, h] = size;
    let mut n = 0;
    for row in 0..GRID_N {
        for col in 0..GRID_N {
            let ys = row * h / GRID_N..((row + 1) * h / GRID_N).max(row * h / GRID_N + 1);
            let xs = col * w / GRID_N..((col + 1) * w / GRID_N).max(col * w / GRID_N + 1);
            let (sum, cnt) = ys
                .flat_map(|y| xs.clone().map(move |x| u32::from(gray[y * w + x])))
                .fold((0, 0), |(s, c), v| (s + v, c + 1));
            let water = sum < 128 * cnt;
            let is_water = t.ground(row, col) == Ground::Water;
            if water != is_water {
                t.set_ground(row, col, if water { Ground::Water } else { Ground::Plain });
                n += 1;
            }
        }
    }
    n
}

/// Höchstens so viele Schritte lassen sich zurücknehmen.
const HISTORY: usize = 200;

/// Verlauf: frühere und zurückgenommene Stände des ganzen Gebiets.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Theater>,
    redo: Vec<Theater>,
}

impl History {
    /// Merkt sich den Stand vor einer Änderung.
    pub fn push(&mut self, before: Theater) {
        if self.undo.len() == HISTORY {
            self.undo.remove(0);
        }
        self.undo.push(before);
        self.redo.clear();
    }

    pub fn undo(&mut self, now: &mut Theater) -> bool {
        self.undo.pop().map(|t| self.redo.push(std::mem::replace(now, t))).is_some()
    }

    pub fn redo(&mut self, now: &mut Theater) -> bool {
        self.redo.pop().map(|t| self.undo.push(std::mem::replace(now, t))).is_some()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(t: &Theater, g: Ground) -> usize {
        (0..GRID_N * GRID_N).filter(|k| t.ground(k / GRID_N, k % GRID_N) == g).count()
    }

    #[test]
    fn brush_sizes_are_discs_and_clip_at_the_edge() {
        let water = Paint::Ground(Ground::Water);
        for (size, cells) in [(1, 1), (3, 9), (5, 21), (9, 69)] {
            let mut t = Theater::empty("T", 1);
            assert!(dab(&mut t, water, 60.5, 60.5, size));
            assert_eq!(count(&t, Ground::Water), cells, "Pinsel {size}");
            assert!(!dab(&mut t, water, 60.5, 60.5, size), "nichts mehr zu ändern");
        }
        let mut t = Theater::empty("T", 1);
        dab(&mut t, water, 0.2, 0.2, 3);
        assert_eq!(count(&t, Ground::Water), 4, "am Rand abgeschnitten");
    }

    #[test]
    fn strokes_have_no_gaps_and_sides_are_their_own_layer() {
        let mut t = Theater::empty("T", 1);
        stroke(&mut t, Paint::Side(Side::Hostile), [10.5, 0.5], [10.5, 127.5], 1);
        assert!((0..GRID_N).all(|c| t.side(10, c) == Side::Hostile));
        assert_eq!(count(&t, Ground::Plain), GRID_N * GRID_N, "Gelände unberührt");
        stroke(&mut t, Paint::Ground(Ground::Forest), [0.5, 0.5], [127.5, 127.5], 1);
        assert!((0..GRID_N).all(|k| t.ground(k, k) == Ground::Forest));
    }

    #[test]
    fn fill_stops_at_other_values() {
        let mut t = Theater::empty("T", 1);
        // senkrechte Wasserlinie teilt das Gebiet
        stroke(&mut t, Paint::Ground(Ground::Water), [0.5, 50.5], [127.5, 50.5], 1);
        let n = fill(&mut t, Paint::Ground(Ground::Desert), 5, 5);
        assert_eq!(n, 50 * GRID_N);
        assert_eq!(t.ground(5, 60), Ground::Plain);
        assert_eq!(fill(&mut t, Paint::Ground(Ground::Desert), 5, 5), 0);
    }

    #[test]
    fn mask_sets_water_and_land() {
        let mut t = Theater::empty("T", 1);
        t.set_ground(0, 0, Ground::Mountains);
        t.set_ground(127, 127, Ground::Water);
        // 256 × 256: obere Hälfte dunkel (Wasser), untere hell (Land)
        let gray: Vec<u8> = (0..256 * 256).map(|k| if k / 256 < 128 { 10 } else { 240 }).collect();
        let n = water_from_mask(&mut t, [256, 256], &gray);
        assert_eq!(n, 64 * GRID_N + 1);
        assert_eq!(t.ground(0, 0), Ground::Water);
        assert_eq!(t.ground(63, 5), Ground::Water);
        assert_eq!(t.ground(64, 5), Ground::Plain);
        assert_eq!(t.ground(127, 127), Ground::Plain, "Wasser im Land wird Flachland");
        // kleinere Maske als das Raster
        let mut u = Theater::empty("T", 1);
        assert_eq!(water_from_mask(&mut u, [2, 2], &[0, 255, 255, 255]), 64 * 64);
    }

    #[test]
    fn history_undoes_and_redoes() {
        let mut t = Theater::empty("T", 1);
        let mut h = History::default();
        let before = t.clone();
        dab(&mut t, Paint::Ground(Ground::City), 3.5, 3.5, 1);
        h.push(before.clone());
        let after = t.clone();
        assert!(h.undo(&mut t) && t == before);
        assert!(!h.undo(&mut t));
        assert!(h.redo(&mut t) && t == after);
        assert!(!h.can_redo());
        for _ in 0..HISTORY + 5 {
            h.push(before.clone());
        }
        assert_eq!(h.undo.len(), HISTORY);
    }
}
