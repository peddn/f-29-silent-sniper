//! Bodenziele (M-010, M-013): eigene Zielklassen, einfache Formen und die
//! deterministische Platzierung pro Seed.
//!
//! Die Klassen sind eigener Entwurf. Radar und Flugabwehr sind – wie im
//! Original – **ein** Objekt: feste Stellungen mit eigenem Radar und eigenen
//! Raketenwerfern. Ein Kennzeichen markiert zivile oder befreundete Ziele, die
//! nicht angegriffen werden sollen.
//!
//! Formen sind Listen achsenparalleler Quader in Zielachsen (x = vorn,
//! y = rechts, z = oben, Ursprung am Boden). Daraus zeichnet das HUD das
//! optische Zielbild und baut das Frontend die 3D-Modelle.

use crate::detmath;
use crate::nav::Steerpoint;
use crate::rng::Pcg32;
use f29_terrain::{Surface, Terrain};

/// RNG-Strom der Zielplatzierung.
pub const STREAM_TARGETS: u64 = 0x5447_5453;

/// Zielklasse (eigener Entwurf).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub const ALL: [TargetClass; 9] = [
        TargetClass::Soft,
        TargetClass::Armored,
        TargetClass::Building,
        TargetClass::Hardened,
        TargetClass::AirDefense,
        TargetClass::Bridge,
        TargetClass::ParkedAircraft,
        TargetClass::ShipSmall,
        TargetClass::ShipLarge,
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
        }
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
    fn names(self) -> (&'static [&'static str], &'static [&'static str]) {
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
        }
    }

    /// Anteil ziviler Objekte dieser Klasse.
    fn civilian_share(self) -> f64 {
        match self {
            TargetClass::Building => 0.3,
            TargetClass::ShipSmall => 0.3,
            TargetClass::ShipLarge => 0.4,
            _ => 0.0,
        }
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
    pub name: &'static str,
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
    pub fn center_ned(&self) -> [f64; 3] {
        [self.north, self.east, -(self.elevation + self.class.height() * 0.5)]
    }

    pub fn is_intact(&self) -> bool {
        self.state != TargetState::Destroyed
    }
}

/// Mindestabstand zwischen zwei Zielen (m).
const MIN_SPACING_M: f64 = 120.0;
/// Mindestabstand zu Flugplätzen (m).
const RUNWAY_CLEARANCE_M: f64 = 2_500.0;

struct Placer<'a> {
    terrain: &'a Terrain,
    rng: Pcg32,
    targets: Vec<Target>,
}

impl Placer<'_> {
    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        let i = (self.rng.next_u32() as usize) % items.len();
        items[i]
    }

    fn free(&self, n: f64, e: f64, spacing: f64) -> bool {
        self.targets.iter().all(|t| (t.north - n).powi(2) + (t.east - e).powi(2) > spacing * spacing)
            && self.terrain.runways().iter().all(|r| {
                (r.center[0] - n).powi(2) + (r.center[1] - e).powi(2)
                    > RUNWAY_CLEARANCE_M * RUNWAY_CLEARANCE_M
            })
    }

    /// Geeigneter Standort für eine Klasse: Schiffe auf Wasser, alles andere
    /// auf flachem Land unterhalb der Schneegrenze.
    fn suitable(&self, class: TargetClass, n: f64, e: f64) -> bool {
        let surface = self.terrain.surface(n, e);
        if class.is_naval() {
            // Genug Wasser rundherum, damit das Schiff nicht im Strand steckt.
            let r = class.radius();
            return surface == Surface::Water
                && [(r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)]
                    .iter()
                    .all(|(dn, de)| self.terrain.ground_height(n + dn, e + de) < -2.0);
        }
        let ok_surface = matches!(surface, Surface::Grass | Surface::Forest | Surface::Beach);
        let flat = 1.0 - self.terrain.normal(n, e)[2] < 0.12;
        ok_surface && flat
    }

    fn name(&mut self, class: TargetClass) -> (&'static str, bool) {
        let (military, civil) = class.names();
        if !civil.is_empty() && self.rng.next_f64() < class.civilian_share() {
            (self.pick(civil), true)
        } else {
            (self.pick(military), false)
        }
    }

    fn push(&mut self, class: TargetClass, n: f64, e: f64, role: TargetRole) {
        // Missionsziele sind immer militärisch.
        let (name, civilian) =
            if role == TargetRole::None { self.name(class) } else { (self.pick(class.names().0), false) };
        let heading = self.rng.range_f64(0.0, std::f64::consts::TAU);
        let elevation = self.terrain.elevation(n, e);
        self.targets.push(Target {
            name,
            class,
            civilian,
            role,
            north: n,
            east: e,
            elevation,
            heading,
            state: TargetState::Intact,
            damage: 0.0,
        });
    }

    /// Sucht in einem Ring um `(n, e)` einen Standort für `class`.
    fn find_spot(
        &mut self,
        class: TargetClass,
        n: f64,
        e: f64,
        r_min: f64,
        r_max: f64,
    ) -> Option<(f64, f64)> {
        for _ in 0..60 {
            let b = self.rng.range_f64(0.0, std::f64::consts::TAU);
            let d = self.rng.range_f64(r_min, r_max);
            let (sb, cb) = detmath::sin_cos(b);
            let (pn, pe) = (n + cb * d, e + sb * d);
            if self.suitable(class, pn, pe) && self.free(pn, pe, MIN_SPACING_M + class.radius()) {
                return Some((pn, pe));
            }
        }
        None
    }

    /// Eine Stellung: Hauptobjekt plus `escorts` Begleiter in der Nähe.
    fn site(
        &mut self,
        class: TargetClass,
        n: f64,
        e: f64,
        role: TargetRole,
        escorts: &[TargetClass],
    ) -> bool {
        self.push(class, n, e, role);
        for &esc in escorts {
            if let Some((pn, pe)) = self.find_spot(esc, n, e, 150.0, 600.0) {
                self.push(esc, pn, pe, TargetRole::None);
            }
        }
        true
    }
}

/// Platziert die Ziele für eine Route. Deterministisch in Seed und Route:
/// Primärziel am zweiten, Sekundärziel am ersten Wegpunkt, Flugabwehr
/// entlang der Route, Schiffe vor der Küste, weitere Ziele verstreut.
pub fn generate(terrain: &Terrain, seed: u64, route: &[Steerpoint]) -> Vec<Target> {
    let mut p = Placer { terrain, rng: Pcg32::new(seed, STREAM_TARGETS), targets: Vec::new() };
    let wps: Vec<[f64; 2]> = route.iter().filter(|s| s.runway.is_none()).map(|s| [s.north, s.east]).collect();
    let (primary_wp, secondary_wp) = match wps.as_slice() {
        [] => return p.targets,
        [only] => (*only, *only),
        [first, second, ..] => (*second, *first),
    };

    // Primärziel: ein lohnendes festes Ziel mit Bewachung.
    let primary_class = p.pick(&[
        TargetClass::Building,
        TargetClass::Hardened,
        TargetClass::Bridge,
        TargetClass::ParkedAircraft,
    ]);
    if let Some((n, e)) = p.find_spot(primary_class, primary_wp[0], primary_wp[1], 0.0, 2_500.0) {
        p.site(
            primary_class,
            n,
            e,
            TargetRole::Primary,
            &[TargetClass::Soft, TargetClass::Armored, TargetClass::AirDefense],
        );
    }
    // Sekundärziel
    let secondary_class = p.pick(&[TargetClass::Soft, TargetClass::Armored, TargetClass::AirDefense]);
    if let Some((n, e)) = p.find_spot(secondary_class, secondary_wp[0], secondary_wp[1], 0.0, 2_500.0) {
        p.site(secondary_class, n, e, TargetRole::Secondary, &[TargetClass::Soft, TargetClass::Soft]);
    }

    // Anker entlang der Route (Wegpunkte und Mitte jeder Teilstrecke)
    let all: Vec<[f64; 2]> = route.iter().map(|s| [s.north, s.east]).collect();
    let mut anchors = all.clone();
    for w in all.windows(2) {
        anchors.push([(w[0][0] + w[1][0]) * 0.5, (w[0][1] + w[1][1]) * 0.5]);
    }

    // Flugabwehrstellungen entlang der Route
    for k in 0..5 {
        let a = anchors[k % anchors.len()];
        if let Some((n, e)) = p.find_spot(TargetClass::AirDefense, a[0], a[1], 4_000.0, 14_000.0) {
            p.site(TargetClass::AirDefense, n, e, TargetRole::None, &[TargetClass::Soft]);
        }
    }
    // Schiffe vor der Küste
    for k in 0..4 {
        let a = anchors[(k * 3 + 1) % anchors.len()];
        let class = if k % 2 == 0 { TargetClass::ShipLarge } else { TargetClass::ShipSmall };
        if let Some((n, e)) = p.find_spot(class, a[0], a[1], 3_000.0, 25_000.0) {
            p.site(class, n, e, TargetRole::None, &[TargetClass::ShipSmall]);
        }
    }
    // Weitere Ziele verstreut
    let others = [
        (TargetClass::Building, &[TargetClass::Soft][..]),
        (TargetClass::Soft, &[TargetClass::Soft, TargetClass::Soft][..]),
        (TargetClass::Armored, &[TargetClass::Armored, TargetClass::Armored][..]),
        (TargetClass::ParkedAircraft, &[TargetClass::ParkedAircraft, TargetClass::Hardened][..]),
        (TargetClass::Bridge, &[][..]),
        (TargetClass::Building, &[TargetClass::Building][..]),
        (TargetClass::Hardened, &[TargetClass::Soft][..]),
    ];
    for (k, (class, escorts)) in others.iter().enumerate() {
        let a = anchors[(k * 2 + 1) % anchors.len()];
        if let Some((n, e)) = p.find_spot(*class, a[0], a[1], 2_000.0, 15_000.0) {
            p.site(*class, n, e, TargetRole::None, escorts);
        }
    }
    p.targets
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(seed: u64) -> crate::Sim {
        crate::Sim::new(crate::SimConfig::new(seed))
    }

    #[test]
    fn placement_is_deterministic_and_plausible() {
        for seed in [0x0F29, 1, 1234, 77] {
            let sim = sample(seed);
            let (terrain, a) = (sim.terrain(), sim.targets());
            assert_eq!(a, sample(seed).targets(), "Seed {seed}: nicht deterministisch");
            assert!(a.len() >= 15, "Seed {seed}: nur {} Ziele", a.len());
            let primaries = a.iter().filter(|t| t.role == TargetRole::Primary).count();
            let secondaries = a.iter().filter(|t| t.role == TargetRole::Secondary).count();
            assert_eq!((primaries, secondaries), (1, 1), "Seed {seed}");
            for t in a {
                let water = terrain.surface(t.north, t.east) == Surface::Water;
                assert_eq!(water, t.class.is_naval(), "Seed {seed}: {} am falschen Ort", t.name);
                assert!(!(t.civilian && t.role != TargetRole::None), "ziviles Missionsziel");
            }
            for (i, x) in a.iter().enumerate() {
                for y in &a[i + 1..] {
                    let d = ((x.north - y.north).powi(2) + (x.east - y.east).powi(2)).sqrt();
                    assert!(
                        d > MIN_SPACING_M,
                        "Seed {seed}: {} und {} nur {d:.0} m entfernt",
                        x.name,
                        y.name
                    );
                }
            }
        }
    }

    #[test]
    fn every_class_has_a_shape_and_names() {
        for c in TargetClass::ALL {
            assert!(!c.shape().is_empty() && c.height() > 1.0 && c.radius() > 1.0, "{c:?}");
            assert!(!c.names().0.is_empty(), "{c:?}");
        }
    }
}
