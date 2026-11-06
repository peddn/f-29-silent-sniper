//! Bodenziele (M-010, M-013): die Objekte des Einsatzgebiets mit Zustand
//! und Rolle in der Mission. Klassen und Formen stehen in `f29_world`; ein
//! Kennzeichen markiert zivile Ziele, die nicht angegriffen werden sollen.

use crate::math::DVec3;
use crate::radar::{self, RadarSpec};
use crate::weapons::WeaponKind;
use f29_world::theater::intern;
use f29_world::{Terrain, Theater};

pub use f29_world::class::{ShapeBox, TargetClass};

/// Flugabwehr der Klasse: ihr Radar (M-020) und die Boden-Luft-Rakete, die
/// sie startet (M-021). Eine Tabelle für alle Stellungstypen.
pub fn air_defense(class: TargetClass) -> Option<(RadarSpec, WeaponKind)> {
    match class {
        TargetClass::ShortRangeSam => Some((radar::SHORAD, WeaponKind::ShortRangeSam)),
        TargetClass::AirDefense => Some((radar::SAM, WeaponKind::MediumRangeSam)),
        TargetClass::EarlyWarning => Some((radar::EARLY_WARNING, WeaponKind::LongRangeSam)),
        _ => None,
    }
}

/// Rolle eines Ziels in der Mission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetRole {
    #[default]
    None,
    Primary,
    Secondary,
}

/// Zustand eines Ziels (folgt aus dem angesammelten Schaden, M-010).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetState {
    #[default]
    Intact,
    Damaged,
    Destroyed,
}

/// Ein Bodenziel.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    /// Anzeigename, mit Ort „NAME AT ORT“ (M-031).
    pub name: &'static str,
    /// Ort allein, leer ohne Ort.
    pub place: &'static str,
    pub class: TargetClass,
    /// Zivil oder befreundet: nicht angreifen.
    pub civilian: bool,
    pub role: TargetRole,
    pub north: f64,
    pub east: f64,
    /// Höhe des Zielfußpunkts (Boden bzw. Wasseroberfläche), m.
    pub elevation: f64,
    /// Ausrichtung (rad, 0 = Nord).
    pub heading: f64,
    pub state: TargetState,
    /// Angesammelter Schaden 0..1; ab 1 zerstört (M-010).
    pub damage: f64,
}

impl Target {
    /// Mittelpunkt der Form in NED (für Erfassung und Anzeige).
    pub fn center(&self) -> DVec3 {
        DVec3::new(self.north, self.east, -(self.elevation + self.class.height() * 0.5))
    }

    /// Name ohne Ort, für knappe Anzeigen (MSN-Seite).
    pub fn short_name(&self) -> &'static str {
        self.name.strip_suffix(self.place).and_then(|n| n.strip_suffix(" AT ")).unwrap_or(self.name)
    }

    pub fn is_intact(&self) -> bool {
        self.state != TargetState::Destroyed
    }
}

/// Rakete der Flugabwehrstellung `i` (M-021): jede dritte SAM-Stellung in
/// der Reihenfolge der Ziele hat die moderne SA-11 statt der SA-6.
pub fn sam_of(targets: &[Target], i: usize) -> Option<WeaponKind> {
    let (_, kind) = air_defense(targets[i].class)?;
    let nth = targets[..i].iter().filter(|t| t.class == TargetClass::AirDefense).count();
    Some(if kind == WeaponKind::MediumRangeSam && nth % 3 == 2 { WeaponKind::ModernSam } else { kind })
}

/// Ziele aus den Objekten eines Einsatzgebiets (Höhe aus dem Gelände).
pub fn from_theater(theater: &Theater, terrain: &Terrain) -> Vec<Target> {
    theater
        .objects
        .iter()
        .map(|o| Target {
            name: intern(&o.label()),
            place: intern(&o.place),
            class: o.class,
            civilian: o.civilian,
            role: TargetRole::None,
            north: o.north,
            east: o.east,
            elevation: terrain.elevation(o.north, o.east),
            heading: o.heading_deg.to_radians(),
            state: TargetState::Intact,
            damage: 0.0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Jedes Objekt des Gebiets wird ein Ziel am selben Ort, auf dem Boden
    /// bzw. auf dem Wasser.
    #[test]
    fn targets_come_from_the_theater() {
        let th = Theater::builtin();
        let terrain = Terrain::from_theater(th);
        let t = from_theater(th, &terrain);
        assert_eq!(t.len(), th.objects.len());
        for (t, o) in t.iter().zip(&th.objects) {
            assert_eq!((t.north, t.east, t.class), (o.north, o.east, o.class));
            assert_eq!(t.elevation, terrain.elevation(o.north, o.east));
            assert_eq!(
                (t.short_name(), t.place),
                (o.name.as_str(), o.place.as_str()),
                "Name und Ort getrennt"
            );
        }
    }
}
