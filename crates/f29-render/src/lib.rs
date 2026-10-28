//! Engine-neutrale Rendering-Geometrie für F-29 Silent Sniper.
//!
//! Alles, was das Frontend zum Zeichnen braucht, aber keine Engine kennen
//! muss: Umrechnung vom Simulations- ins Darstellungskoordinatensystem,
//! Farbpalette, Terrain-Kachel-Meshes und das Flugzeugmodell. Dadurch bleibt
//! das Bevy-Crate eine dünne Schicht, und die Geometrie ist ohne GPU testbar.
//!
//! **Darstellungskoordinaten (Y-up)**, wie Bevy sie erwartet:
//! x = Ost, y = oben, z = Süd (rechtshändig, „vorn“ eines Modells = −z).

pub mod coords;
pub mod mesh;
pub mod model;
pub mod palette;
pub mod runway_mesh;
pub mod terrain_mesh;

pub use mesh::MeshData;
