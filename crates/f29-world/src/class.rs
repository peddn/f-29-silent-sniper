//! Objektklassen der Einsatzgebiete (M-010): eigene Klassen mit einfachen
//! Formen und Namen.
//!
//! Radar und Flugabwehr sind – wie im Original – **ein** Objekt: feste
//! Stellungen mit eigenem Radar und eigenen Raketenwerfern.
//!
//! Formen sind Listen achsenparalleler Quader in Zielachsen (x = vorn,
//! y = rechts, z = oben, Ursprung am Boden). Daraus zeichnet das HUD das
//! optische Zielbild und baut das Frontend die 3D-Modelle.

use serde::{Deserialize, Serialize};

/// Zielklasse (eigener Entwurf).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetClass {
    /// Ungeschützt: Lastwagen, Lager, Depots.
    Soft,
    /// Panzer, Schützenpanzer.
    Armored,
    /// Hallen, Kommandostände, Kraftwerke.
    Building,
    /// Bunker, Flugzeugschutzbauten.
    Hardened,
    /// Flugabwehrstellung: Radar mit eigenen Raketenwerfern.
    AirDefense,
    /// Brücken und Dämme.
    Bridge,
    /// Abgestellte Flugzeuge.
    ParkedAircraft,
    /// Kleine Schiffe (Patrouillenboote).
    ShipSmall,
    /// Große Schiffe (Fregatten, Frachter).
    ShipLarge,
    /// Frühwarnradar: große Drehantenne mit Langstreckenraketen (M-020).
    EarlyWarning,
    /// Kurzstrecken-Flugabwehr: Fahrzeug mit Radar und Werfer, bewacht Ziele.
    ShortRangeSam,
}

/// Achsenparalleler Quader einer Zielform (Meter, Zielachsen).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeBox {
    pub center: [f32; 3],
    pub half: [f32; 3],
}

const fn b(center: [f32; 3], half: [f32; 3]) -> ShapeBox {
    ShapeBox { center, half }
}

const SHAPE_SOFT: [ShapeBox; 2] = [b([-1.0, 0.0, 1.3], [2.6, 1.2, 1.3]), b([2.6, 0.0, 1.0], [1.0, 1.2, 1.0])];
const SHAPE_ARMORED: [ShapeBox; 3] = [
    b([0.0, 0.0, 0.9], [3.5, 1.7, 0.9]),
    b([-0.3, 0.0, 2.2], [1.6, 1.3, 0.5]),
    b([3.4, 0.0, 2.2], [2.1, 0.15, 0.15]),
];
const SHAPE_BUILDING: [ShapeBox; 3] = [
    b([0.0, 0.0, 6.0], [20.0, 12.0, 6.0]),
    b([-8.0, 5.0, 14.0], [2.0, 2.0, 2.0]),
    b([10.0, -6.0, 13.0], [3.0, 3.0, 1.0]),
];
const SHAPE_HARDENED: [ShapeBox; 2] =
    [b([0.0, 0.0, 3.5], [14.0, 10.0, 3.5]), b([0.0, 0.0, 7.5], [9.0, 7.0, 0.5])];
const SHAPE_AIR_DEFENSE: [ShapeBox; 5] = [
    b([0.0, 0.0, 1.5], [2.5, 1.6, 1.5]),
    b([0.0, 0.0, 5.0], [0.3, 0.3, 2.0]),
    b([0.0, 0.0, 7.2], [0.4, 3.0, 0.8]),
    b([12.0, 8.0, 1.3], [3.2, 1.1, 0.9]),
    b([12.0, -8.0, 1.3], [3.2, 1.1, 0.9]),
];
const SHAPE_EARLY_WARNING: [ShapeBox; 4] = [
    b([0.0, 0.0, 2.0], [5.0, 4.0, 2.0]),
    b([0.0, 0.0, 9.0], [0.8, 0.8, 5.0]),
    b([0.0, 0.0, 15.0], [1.0, 7.0, 1.5]),
    b([14.0, 0.0, 1.2], [4.0, 1.4, 1.2]),
];
const SHAPE_SHORT_RANGE_SAM: [ShapeBox; 4] = [
    b([0.0, 0.0, 1.1], [4.5, 1.4, 1.1]),
    b([-1.5, 0.0, 2.8], [1.2, 1.2, 0.6]),
    b([-1.5, 0.0, 3.9], [0.2, 1.0, 0.5]),
    b([1.8, 0.0, 2.6], [1.6, 0.8, 0.4]),
];
const SHAPE_BRIDGE: [ShapeBox; 4] = [
    b([0.0, 0.0, 8.0], [60.0, 5.0, 0.8]),
    b([-30.0, 0.0, 3.6], [1.5, 4.0, 3.6]),
    b([0.0, 0.0, 3.6], [1.5, 4.0, 3.6]),
    b([30.0, 0.0, 3.6], [1.5, 4.0, 3.6]),
];
const SHAPE_AIRCRAFT: [ShapeBox; 3] = [
    b([0.0, 0.0, 2.0], [8.0, 1.0, 1.0]),
    b([-1.0, 0.0, 2.0], [2.5, 6.5, 0.25]),
    b([-7.0, 0.0, 4.0], [1.0, 0.2, 2.0]),
];
const SHAPE_SHIP_SMALL: [ShapeBox; 2] =
    [b([0.0, 0.0, 1.5], [12.0, 3.0, 1.5]), b([-2.0, 0.0, 4.5], [3.0, 2.0, 1.5])];
const SHAPE_SHIP_LARGE: [ShapeBox; 3] = [
    b([0.0, 0.0, 4.0], [60.0, 8.0, 4.0]),
    b([-15.0, 0.0, 11.0], [12.0, 6.0, 3.0]),
    b([-15.0, 0.0, 17.0], [0.5, 0.5, 3.0]),
];

impl TargetClass {
    pub const ALL: [TargetClass; 11] = [
        TargetClass::Soft,
        TargetClass::Armored,
        TargetClass::Building,
        TargetClass::Hardened,
        TargetClass::AirDefense,
        TargetClass::Bridge,
        TargetClass::ParkedAircraft,
        TargetClass::ShipSmall,
        TargetClass::ShipLarge,
        TargetClass::EarlyWarning,
        TargetClass::ShortRangeSam,
    ];

    /// Kurzname für Anzeigen.
    pub fn label(self) -> &'static str {
        match self {
            TargetClass::Soft => "SOFT",
            TargetClass::Armored => "ARMOR",
            TargetClass::Building => "BUILDING",
            TargetClass::Hardened => "HARDENED",
            TargetClass::AirDefense => "AIR DEFENSE",
            TargetClass::Bridge => "BRIDGE",
            TargetClass::ParkedAircraft => "AIRCRAFT",
            TargetClass::ShipSmall => "BOAT",
            TargetClass::ShipLarge => "SHIP",
            TargetClass::EarlyWarning => "EW RADAR",
            TargetClass::ShortRangeSam => "SHORAD",
        }
    }

    /// Strahlt ein Radar ab (Radarstellung, M-020).
    pub fn is_radar(self) -> bool {
        matches!(self, TargetClass::AirDefense | TargetClass::EarlyWarning | TargetClass::ShortRangeSam)
    }

    pub fn is_naval(self) -> bool {
        matches!(self, TargetClass::ShipSmall | TargetClass::ShipLarge)
    }

    /// Form aus Quadern (Zielachsen, Meter).
    pub fn shape(self) -> &'static [ShapeBox] {
        match self {
            TargetClass::Soft => &SHAPE_SOFT,
            TargetClass::Armored => &SHAPE_ARMORED,
            TargetClass::Building => &SHAPE_BUILDING,
            TargetClass::Hardened => &SHAPE_HARDENED,
            TargetClass::AirDefense => &SHAPE_AIR_DEFENSE,
            TargetClass::Bridge => &SHAPE_BRIDGE,
            TargetClass::ParkedAircraft => &SHAPE_AIRCRAFT,
            TargetClass::ShipSmall => &SHAPE_SHIP_SMALL,
            TargetClass::ShipLarge => &SHAPE_SHIP_LARGE,
            TargetClass::EarlyWarning => &SHAPE_EARLY_WARNING,
            TargetClass::ShortRangeSam => &SHAPE_SHORT_RANGE_SAM,
        }
    }

    /// Höhe der Form über dem Boden (m).
    pub fn height(self) -> f64 {
        self.shape().iter().map(|s| f64::from(s.center[2] + s.half[2])).fold(0.0, f64::max)
    }

    /// Radius der Form in der Ebene (m).
    pub fn radius(self) -> f64 {
        self.shape()
            .iter()
            .map(|s| {
                let x = f64::from(s.center[0].abs() + s.half[0]);
                let y = f64::from(s.center[1].abs() + s.half[1]);
                (x * x + y * y).sqrt()
            })
            .fold(0.0, f64::max)
    }

    /// Namen militärischer und ziviler Ziele dieser Klasse.
    pub fn names(self) -> (&'static [&'static str], &'static [&'static str]) {
        match self {
            TargetClass::Soft => (&["TRUCK", "FUEL TRUCK", "SUPPLY DUMP"], &[]),
            TargetClass::Armored => (&["TANK", "APC"], &[]),
            TargetClass::Building => {
                (&["COMMAND POST", "BARRACKS", "POWER PLANT"], &["FACTORY", "WAREHOUSE"])
            }
            TargetClass::Hardened => (&["BUNKER", "SHELTER"], &[]),
            TargetClass::AirDefense => (&["SAM SITE"], &[]),
            TargetClass::Bridge => (&["BRIDGE"], &[]),
            TargetClass::ParkedAircraft => (&["FIGHTER", "TRANSPORT"], &[]),
            TargetClass::ShipSmall => (&["PATROL BOAT"], &["FISHING BOAT"]),
            TargetClass::ShipLarge => (&["FRIGATE", "DESTROYER"], &["FREIGHTER", "TANKER"]),
            TargetClass::EarlyWarning => (&["EW RADAR"], &[]),
            TargetClass::ShortRangeSam => (&["SHORT RANGE SAM"], &[]),
        }
    }

    /// Anteil ziviler Objekte dieser Klasse.
    pub fn civilian_share(self) -> f64 {
        match self {
            TargetClass::Building => 0.3,
            TargetClass::ShipSmall => 0.3,
            TargetClass::ShipLarge => 0.4,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_has_a_shape_and_names() {
        for c in TargetClass::ALL {
            assert!(!c.shape().is_empty() && c.height() > 1.0 && c.radius() > 1.0, "{c:?}");
            assert!(!c.names().0.is_empty(), "{c:?}");
        }
    }
}
