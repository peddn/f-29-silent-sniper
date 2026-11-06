//! Navigationspunkte (Steerpoints) einer Route und Planquadrate.

use f29_world::{Runway, Terrain};

#[derive(Debug, Clone, PartialEq)]
pub struct Steerpoint {
    pub name: String,
    pub north: f64,
    pub east: f64,
    /// Geländehöhe am Punkt.
    pub elevation: f64,
    /// Index der Bahn, falls der Punkt ein Flugplatz ist.
    pub runway: Option<usize>,
}

impl Steerpoint {
    /// Navigationspunkt in der Mitte einer Bahn.
    pub fn runway(rw: &Runway, index: usize) -> Self {
        Self {
            name: rw.name.to_string(),
            north: rw.center[0],
            east: rw.center[1],
            elevation: rw.elevation_m,
            runway: Some(index),
        }
    }
}

/// Länge der Route über alle Steuerpunkte (m, waagerecht).
pub fn route_length_m(route: &[Steerpoint]) -> f64 {
    route.windows(2).map(|w| (w[1].north - w[0].north).hypot(w[1].east - w[0].east)).sum()
}

/// Halbe Kantenlänge des Gebiets (m); ohne Gebiet 100 km.
pub fn half_extent_m(terrain: &Terrain) -> f64 {
    terrain.size_m().unwrap_or(200_000.0) * 0.5
}

/// Planquadrat (M-031): zwei Buchstaben für das 100-km-Quadrat (Spalte von
/// West, Zeile von Süd), zwei Ziffern für das 10-km-Feld darin (Ost, Nord),
/// z. B. „CE12“. Das Gitter liegt auf vollen 10 bzw. 100 km um den Ursprung
/// wie das Kartengitter; „A“ ist das Quadrat an der West- bzw. Südkante des
/// Gebiets mit der halben Kantenlänge `half_m`.
pub fn grid_square(north: f64, east: f64, half_m: f64) -> String {
    let edge = (half_m / 100_000.0).ceil() * 100_000.0;
    let cell = |x: f64| ((x + edge) / 10_000.0).floor().clamp(0.0, 259.0) as u8;
    let (e, n) = (cell(east), cell(north));
    format!("{}{}{}{}", (b'A' + e / 10) as char, (b'A' + n / 10) as char, e % 10, n % 10)
}

#[cfg(test)]
mod tests {
    use super::grid_square;

    /// Buchstaben von West bzw. Süd, Ziffern Ost vor Nord; die Kanten eines
    /// 500-km-Gebiets liegen mitten in den äußeren Quadraten.
    #[test]
    fn grid_squares_read_east_then_north() {
        let sq = |n: f64, e: f64| grid_square(n * 1e3, e * 1e3, 250e3);
        assert_eq!(sq(-250.0, -250.0), "AA55");
        assert_eq!(sq(0.0, 0.0), "DD00");
        assert_eq!(sq(123.0, -87.0), "CE12");
        assert_eq!(sq(249.9, 249.9), "FF44");
    }
}
