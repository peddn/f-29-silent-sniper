//! Navigationspunkte (Steerpoints) einer Route.

use f29_world::Runway;

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
