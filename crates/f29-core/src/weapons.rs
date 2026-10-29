//! Waffen (M-014, Wirkung M-010 bis M-012).
//!
//! Vier interne Schächte mit je einer Waffenart. Der Schacht wird von Hand
//! geöffnet; ausgelöst wird nur bei offenem Schacht und mäßiger Querlage.
//!
//! - **Bomben** sind rein physikalisch: Massenpunkt mit Schwerkraft und
//!   Luftwiderstand. Der Schaden folgt aus dem Abstand des Einschlags zum Ziel.
//! - **Lenkwaffen** (Fire and Forget, M-013) fliegen mit konstanter Marschfahrt
//!   und begrenzter Drehrate auf ihr Ziel, wie im Original. Erfasst ihr
//!   Suchkopf ein Ziel, wird der Endanflug **gewürfelt** (Miss, Near Miss, Hit),
//!   abhängig von der Startgüte (Entfernung, Ablage, Zustand der Optik). Die
//!   Waffe fliegt dann sichtbar auf ihren Zielpunkt – bei Miss daneben.
//!   Verliert der Suchkopf das Ziel (Kegel, Sichtlinie), fliegt sie blind weiter.
//!   Ohne Aufschaltung abgefeuert, schaltet sie auf, was ihr vor den Suchkopf
//!   kommt – auch zivile Ziele.
//! - **Schaden** sammelt sich am Ziel: Die Wirkungsmatrix gibt den Schaden pro
//!   Treffer (1 = ein Treffer zerstört, 0,5 = zwei Treffer). Jeder Treffer
//!   würfelt unabhängig einen kritischen Treffer (1 %), der sicher zerstört.
//!
//! Werte: eigene Größenordnungen aus öffentlichen Daten realer Waffen und
//! Spielbalance, siehe M-014.

use crate::atmosphere::{self, G0};
use crate::detmath;
use crate::math::{DQuat, DVec3};
use crate::rng::Pcg32;
use crate::sensors::line_of_sight;
use crate::targets::{Target, TargetClass, TargetState};
use f29_terrain::Terrain;

/// RNG-Strom der Waffenwirkung.
pub const STREAM_WEAPONS: u64 = 0x5745_4150;
/// Anzahl der Schächte.
pub const BAYS: usize = 4;
/// Höchstens so viele eigene Waffen gleichzeitig in der Luft.
pub const MAX_IN_FLIGHT: usize = 8;
/// Zeit zum Öffnen bzw. Schließen der Schachtklappen (s).
pub const DOOR_TIME_S: f64 = 1.2;
/// Auslösen nur bis zu dieser Querlage (rad, 60°).
pub const MAX_RELEASE_BANK_RAD: f64 = std::f64::consts::FRAC_PI_3;
/// Ausstoßgeschwindigkeit nach unten (Körperachse z, m/s).
const EJECT_SPEED_MPS: f64 = 3.0;
/// Startpunkt unter dem Rumpf (Körperachse z, m).
const EJECT_OFFSET_M: f64 = 1.5;
/// Ab diesem Eintreffwinkel (unter der Waagerechten) wirkt eine Bombe gegen
/// gehärtete Ziele (M-012, rad, 60°).
pub const HARDENED_MIN_IMPACT_RAD: f64 = std::f64::consts::FRAC_PI_3;
/// Bis zu diesem Anteil des Wirkradius ist die Wirkung voll (Treffer), darüber
/// fällt sie linear auf null (M-011).
const FULL_EFFECT_FRACTION: f64 = 0.4;
/// Schaden darunter zählt nicht.
const MIN_DAMAGE: f64 = 0.05;
/// Wahrscheinlichkeit eines kritischen Treffers je Treffer.
pub const CRITICAL_CHANCE: f64 = 0.01;
/// Längste Flugzeit einer ungelenkten Waffe (s), Schutz gegen Endlosflug.
const MAX_FALL_S: f64 = 180.0;
/// Nur so viele Ereignisse merken (für Meldungen).
const EVENT_LOG: usize = 16;
/// Lenkwaffen fallen erst frei aus dem Schacht, dann setzt die Lenkung ein (s).
const GUIDANCE_DELAY_S: f64 = 0.4;
/// Sichtlinie des optischen Suchkopfs alle so viele Ticks prüfen.
const SEEKER_LOS_TICKS: u64 = 12;
/// Ohne Ziel senkt eine Lenkwaffe die Nase so schnell (rad/s, 2°/s).
const BLIND_NOSE_DROP: f64 = 0.035;
/// Sweet Spot: volle Startgüte zwischen diesen Anteilen der Reichweite. Von
/// der Reichweitengrenze steigt die Güte bis dorthin an und fällt bei weiterer
/// Annäherung wieder ab (Spielerentscheidung v0.2.5).
pub const SWEET_SPOT_FAR: f64 = 0.5;
pub const SWEET_SPOT_NEAR: f64 = 0.3;
/// Startgüte an der Reichweitengrenze.
const QUALITY_AT_MAX_RANGE: f64 = 0.25;
/// Startgüte direkt am Ziel (zu nah: kaum Zeit zum Eindrehen).
const QUALITY_AT_ZERO_RANGE: f64 = 0.3;
/// Startgüte am Rand des Suchkopfkegels.
const QUALITY_AT_CONE_EDGE: f64 = 0.6;
/// Ablage, bis zu der die volle Güte gilt (rad, 5°).
const BORESIGHT_FREE_RAD: f64 = 0.087;
/// Ausgang-Wahrscheinlichkeiten bei Startgüte 0 bzw. 1: (Miss, Near Miss);
/// der Rest ist Hit.
const ODDS_POOR: (f64, f64) = (0.55, 0.30);
const ODDS_IDEAL: (f64, f64) = (0.05, 0.15);
/// Near Miss richtet diesen Anteil des Treffer-Schadens an.
const NEAR_MISS_DAMAGE: f64 = 0.5;

/// Waffenarten (M-014). Namen sind eigene, fiktive Bezeichnungen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponKind {
    /// Freifallbombe.
    FreeFall,
    /// Fallschirmbombe: bremst kurz nach dem Abwurf stark ab, fällt steil und
    /// lässt dem Flugzeug Zeit, sich aus dem Splitterbereich zu entfernen.
    Retarded,
    /// Gelenkte Gleitbombe mit optischem Suchkopf.
    GuidedBomb,
    /// Optisch gelenkte Luft-Boden-Rakete.
    AirToGround,
    /// Radarjäger: fliegt nur Flugabwehrstellungen (Radar) an.
    AntiRadar,
    /// Seezielrakete: fliegt nur Schiffe an.
    AntiShip,
}

/// Suchkopf einer gelenkten Waffe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seeker {
    /// Ungelenkt.
    None,
    /// Optisch: braucht freie Sicht, alle Bodenziele.
    Optical,
    /// Passiv auf Radarstrahlung: nur Flugabwehrstellungen, ohne Sichtlinie.
    Radiation,
    /// Aktives Radar gegen Schiffe, ohne Sichtlinie.
    Naval,
}

impl Seeker {
    /// Kann dieser Suchkopf ein Ziel der Klasse überhaupt erfassen?
    pub fn sees(self, class: TargetClass) -> bool {
        match self {
            Seeker::None => false,
            Seeker::Optical => true,
            Seeker::Radiation => class == TargetClass::AirDefense,
            Seeker::Naval => class.is_naval(),
        }
    }
}

/// Kenngrößen einer Waffenart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponSpec {
    pub kind: WeaponKind,
    /// Kurzname für HUD und MFD.
    pub label: &'static str,
    /// Langname für Meldungen.
    pub name: &'static str,
    pub mass_kg: f64,
    /// Widerstandsfläche Cw·A (m², Bomben).
    pub drag_area_m2: f64,
    /// Widerstandsfläche mit geöffnetem Bremsschirm (m²), sonst wie oben.
    pub deployed_drag_area_m2: f64,
    /// Zeit bis zum Öffnen des Bremsschirms (s).
    pub deploy_s: f64,
    /// Stück je Schacht.
    pub per_bay: u8,
    pub seeker: Seeker,
    /// Marschfahrt der Lenkwaffe (m/s), die sie bis zum Ende hält.
    pub cruise_mps: f64,
    /// Beschleunigung auf Marschfahrt (m/s²).
    pub accel_mps2: f64,
    /// Größte Drehrate der Flugrichtung (rad/s).
    pub turn_rate_rad: f64,
    /// Aufschaltreichweite (m): größte Reichweite für Abschuss und Suchkopf.
    pub max_range_m: f64,
    /// Halber Öffnungswinkel des Suchkopfs (rad).
    pub seeker_cone_rad: f64,
    /// Kleinster Sinkwinkel (rad): Die Gleitbombe kann nicht steigen und nicht
    /// flacher gleiten – aus zu großer Entfernung fällt sie zu kurz.
    pub min_glide_rad: f64,
    /// Wirkradius der Ladung (m).
    pub lethal_radius_m: f64,
    /// Splitterbereich (m): Einschlag näher am Flugzeug löst die Warnung aus.
    pub frag_radius_m: f64,
    /// Längste Flugzeit (s).
    pub life_s: f64,
}

const fn deg(d: f64) -> f64 {
    d * std::f64::consts::PI / 180.0
}

/// Gemeinsame Grundwerte einer 250-kg-Bombe.
const BOMB: WeaponSpec = WeaponSpec {
    kind: WeaponKind::FreeFall,
    label: "B-25",
    name: "B-25 BOMB",
    mass_kg: 250.0,
    drag_area_m2: 0.020,
    deployed_drag_area_m2: 0.020,
    deploy_s: 0.0,
    per_bay: 2,
    seeker: Seeker::None,
    cruise_mps: 0.0,
    accel_mps2: 0.0,
    turn_rate_rad: 0.0,
    max_range_m: 0.0,
    seeker_cone_rad: 0.0,
    min_glide_rad: 0.0,
    lethal_radius_m: 30.0,
    frag_radius_m: 180.0,
    life_s: MAX_FALL_S,
};

/// Gemeinsame Grundwerte der Lenkwaffen.
const MISSILE: WeaponSpec = WeaponSpec {
    kind: WeaponKind::AirToGround,
    label: "AGR-7",
    name: "AGR-7 TV MISSILE",
    mass_kg: 210.0,
    drag_area_m2: 0.035,
    deployed_drag_area_m2: 0.035,
    deploy_s: 0.0,
    per_bay: 2,
    seeker: Seeker::Optical,
    cruise_mps: 320.0,
    accel_mps2: 120.0,
    turn_rate_rad: deg(25.0),
    max_range_m: 12_000.0,
    seeker_cone_rad: deg(25.0),
    min_glide_rad: 0.0,
    lethal_radius_m: 8.0,
    frag_radius_m: 90.0,
    life_s: 55.0,
};

const SPECS: [WeaponSpec; 6] = [
    BOMB,
    WeaponSpec {
        kind: WeaponKind::Retarded,
        label: "B-25R",
        name: "B-25R RETARDED BOMB",
        deployed_drag_area_m2: 0.9,
        deploy_s: 0.3,
        ..BOMB
    },
    WeaponSpec {
        kind: WeaponKind::GuidedBomb,
        label: "GB-25",
        name: "GB-25 GLIDE BOMB",
        mass_kg: 270.0,
        drag_area_m2: 0.025,
        deployed_drag_area_m2: 0.025,
        seeker: Seeker::Optical,
        cruise_mps: 220.0,
        accel_mps2: 15.0,
        turn_rate_rad: deg(12.0),
        max_range_m: 10_000.0,
        seeker_cone_rad: deg(30.0),
        // Gleitzahl 6: je 6 m Strecke mindestens 1 m Höhe.
        min_glide_rad: 0.165,
        lethal_radius_m: 28.0,
        life_s: 120.0,
        ..BOMB
    },
    MISSILE,
    WeaponSpec {
        kind: WeaponKind::AntiRadar,
        label: "ARM-9",
        name: "ARM-9 ANTI-RADAR MISSILE",
        mass_kg: 360.0,
        seeker: Seeker::Radiation,
        cruise_mps: 650.0,
        accel_mps2: 200.0,
        turn_rate_rad: deg(20.0),
        max_range_m: 30_000.0,
        seeker_cone_rad: deg(35.0),
        lethal_radius_m: 10.0,
        frag_radius_m: 100.0,
        life_s: 70.0,
        ..MISSILE
    },
    WeaponSpec {
        kind: WeaponKind::AntiShip,
        label: "ASR-4",
        name: "ASR-4 ANTI-SHIP MISSILE",
        mass_kg: 520.0,
        per_bay: 1,
        seeker: Seeker::Naval,
        cruise_mps: 270.0,
        accel_mps2: 60.0,
        turn_rate_rad: deg(10.0),
        max_range_m: 30_000.0,
        seeker_cone_rad: deg(40.0),
        lethal_radius_m: 15.0,
        frag_radius_m: 120.0,
        life_s: 160.0,
        ..MISSILE
    },
];

impl WeaponKind {
    pub const ALL: [WeaponKind; 6] = [
        WeaponKind::FreeFall,
        WeaponKind::Retarded,
        WeaponKind::GuidedBomb,
        WeaponKind::AirToGround,
        WeaponKind::AntiRadar,
        WeaponKind::AntiShip,
    ];

    pub fn spec(self) -> &'static WeaponSpec {
        &SPECS[self as usize]
    }

    pub fn label(self) -> &'static str {
        self.spec().label
    }

    pub fn is_guided(self) -> bool {
        self.spec().seeker != Seeker::None
    }

    /// Kennwort für `--loadout` und Replays.
    pub fn code(self) -> &'static str {
        match self {
            WeaponKind::FreeFall => "fall",
            WeaponKind::Retarded => "chute",
            WeaponKind::GuidedBomb => "glide",
            WeaponKind::AirToGround => "agm",
            WeaponKind::AntiRadar => "arm",
            WeaponKind::AntiShip => "ship",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.code() == code)
    }

    /// Schaden pro Treffer gegen eine Zielklasse (M-010): 1 = ein Treffer
    /// zerstört, 0,5 = zwei Treffer, 0 = wirkungslos. `impact_rad` ist der
    /// Eintreffwinkel unter der Waagerechten (gehärtete Ziele, M-012).
    pub fn effectiveness(self, class: TargetClass, impact_rad: f64) -> f64 {
        use TargetClass as C;
        use WeaponKind as W;
        let steep = impact_rad >= HARDENED_MIN_IMPACT_RAD;
        match (self, class) {
            (W::AntiRadar, C::AirDefense) => 1.0,
            (W::AntiRadar, _) => 0.0,
            (W::AntiShip, C::ShipSmall) => 1.0,
            (W::AntiShip, C::ShipLarge) => 0.5,
            (W::AntiShip, _) => 0.0,
            // Gehärtete Ziele nur mit steil einfallender Bombe (Toss-Bombing,
            // Sturzflug), und auch dann zwei Treffer; die Fallschirmbombe ist
            // dafür zu leicht gebaut.
            (W::FreeFall | W::GuidedBomb, C::Hardened) => {
                if steep {
                    0.5
                } else {
                    0.0
                }
            }
            (_, C::Hardened) => 0.0,
            (_, C::Soft | C::ParkedAircraft | C::AirDefense | C::ShipSmall) => 1.0,
            (W::GuidedBomb | W::AirToGround, C::Armored) => 1.0,
            (W::AirToGround, C::Building) => 0.34,
            (W::AirToGround, C::Bridge | C::ShipLarge) => 0.25,
            (W::GuidedBomb, C::ShipLarge) => 0.5,
            (_, C::ShipLarge) => 0.34,
            (_, C::Armored | C::Building | C::Bridge) => 0.5,
        }
    }

    /// Größte Reichweite für Abschuss bzw. Aufschalten (m). Die Gleitbombe
    /// kommt außerdem nur so weit, wie ihre Höhe über dem Ziel trägt.
    pub fn max_range(self, height_above_target_m: f64) -> f64 {
        let s = self.spec();
        if s.min_glide_rad > 0.0 {
            let glide = height_above_target_m.max(0.0) / detmath::tan(s.min_glide_rad);
            s.max_range_m.min(glide)
        } else {
            s.max_range_m
        }
    }
}

/// Reichweitenzone einer Lenkwaffe gegen das aufgeschaltete Ziel (Zielrahmen
/// im HUD, wie im Original: erfasst, näher dran Farbwechsel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchZone {
    /// Waffe wirkt nicht gegen dieses Ziel oder ihr Suchkopf sieht es nicht.
    NoEffect,
    /// Außer Reichweite (oder optisch nicht sichtbar).
    OutOfRange,
    /// In Reichweite.
    InRange,
    /// Gute Zone: höchste Trefferchance.
    Good,
}

/// Zone für die Waffe `kind` gegen `target` von `position` (NED) aus.
/// `visible` = das Ziel ist optisch erfasst (für optische Suchköpfe).
pub fn launch_zone(kind: WeaponKind, position: DVec3, target: &Target, visible: bool) -> LaunchZone {
    let s = kind.spec();
    if !s.seeker.sees(target.class) || kind.effectiveness(target.class, std::f64::consts::FRAC_PI_2) <= 0.0 {
        return LaunchZone::NoEffect;
    }
    let c = target.center_ned();
    let d = DVec3::new(c[0], c[1], c[2]) - position;
    let range = d.length();
    let max = kind.max_range(-position.z - target.elevation);
    if (s.seeker == Seeker::Optical && !visible) || range > max {
        LaunchZone::OutOfRange
    } else if (SWEET_SPOT_NEAR..=SWEET_SPOT_FAR).contains(&(range / max.max(1.0))) {
        LaunchZone::Good
    } else {
        LaunchZone::InRange
    }
}

/// Startgüte 0..1 aus Entfernung, Ablage vom Suchkopfkegel und `sensor`
/// (Leistung der Bordoptik bei optischen Suchköpfen, sonst 1).
pub fn launch_quality(kind: WeaponKind, range: f64, off_angle: f64, sensor: f64, max_range: f64) -> f64 {
    let s = kind.spec();
    let f_range = range_quality(range / max_range.max(1.0));
    let span = (s.seeker_cone_rad - BORESIGHT_FREE_RAD).max(1e-6);
    let a = ((off_angle - BORESIGHT_FREE_RAD) / span).clamp(0.0, 1.0);
    let f_angle = 1.0 - (1.0 - QUALITY_AT_CONE_EDGE) * a;
    (f_range * f_angle * sensor.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

/// Entfernungsanteil der Startgüte über `r` = Entfernung / Reichweite: niedrig
/// an der Reichweitengrenze, ansteigend bis zum Sweet Spot, dort 1, bei
/// weiterer Annäherung wieder abfallend.
pub fn range_quality(r: f64) -> f64 {
    let r = r.max(0.0);
    if r >= 1.0 {
        QUALITY_AT_MAX_RANGE
    } else if r > SWEET_SPOT_FAR {
        1.0 - (1.0 - QUALITY_AT_MAX_RANGE) * (r - SWEET_SPOT_FAR) / (1.0 - SWEET_SPOT_FAR)
    } else if r >= SWEET_SPOT_NEAR {
        1.0
    } else {
        QUALITY_AT_ZERO_RANGE + (1.0 - QUALITY_AT_ZERO_RANGE) * r / SWEET_SPOT_NEAR
    }
}

/// Wahrscheinlichkeiten (Miss, Near Miss, Hit) bei Startgüte `q`.
pub fn outcome_odds(q: f64) -> (f64, f64, f64) {
    let q = q.clamp(0.0, 1.0);
    let miss = ODDS_POOR.0 + (ODDS_IDEAL.0 - ODDS_POOR.0) * q;
    let near = ODDS_POOR.1 + (ODDS_IDEAL.1 - ODDS_POOR.1) * q;
    (miss, near, 1.0 - miss - near)
}

/// Gewürfelter Endanflug einer Lenkwaffe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Miss,
    NearMiss,
    Hit,
}

/// Beladung der vier Schächte (eine Waffenart je Schacht oder leer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loadout(pub [Option<WeaponKind>; BAYS]);

impl Default for Loadout {
    /// Standardbeladung: Freifall, Fallschirm, Gleitbombe, Luft-Boden-Rakete.
    fn default() -> Self {
        Self([
            Some(WeaponKind::FreeFall),
            Some(WeaponKind::Retarded),
            Some(WeaponKind::GuidedBomb),
            Some(WeaponKind::AirToGround),
        ])
    }
}

impl Loadout {
    /// Leere Schächte (z. B. für Landeanflüge nach der Mission).
    pub const EMPTY: Loadout = Loadout([None; BAYS]);

    /// `fall,chute,glide,agm` – `-` für einen leeren Schacht.
    pub fn parse(text: &str) -> Option<Self> {
        let parts: Vec<&str> = text.split(',').map(str::trim).collect();
        if parts.len() != BAYS {
            return None;
        }
        let mut bays = [None; BAYS];
        for (bay, p) in bays.iter_mut().zip(parts) {
            *bay = match p {
                "-" | "" => None,
                code => Some(WeaponKind::from_code(code)?),
            };
        }
        Some(Self(bays))
    }

    pub fn to_text(&self) -> String {
        self.0.iter().map(|b| b.map_or("-", WeaponKind::code)).collect::<Vec<_>>().join(",")
    }

    /// Masse der vollen Beladung (kg).
    pub fn mass_kg(&self) -> f64 {
        self.0.iter().flatten().map(|k| k.spec().mass_kg * f64::from(k.spec().per_bay)).sum()
    }
}

/// Ein Schacht.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bay {
    pub kind: Option<WeaponKind>,
    pub count: u8,
}

/// Eine Waffe im Flug.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weapon {
    /// Fortlaufende Nummer (für die Darstellung).
    pub id: u32,
    pub kind: WeaponKind,
    /// NED (m).
    pub position: DVec3,
    /// NED (m/s).
    pub velocity: DVec3,
    pub age_s: f64,
    /// Ziel des Suchkopfs (Index in `Sim::targets`).
    pub target: Option<usize>,
    /// Gewürfelter Endanflug gegen `target`.
    pub outcome: Option<Outcome>,
    /// Kritischer Treffer (nur mit `Outcome::Hit`).
    pub critical: bool,
    /// Zielpunkt relativ zum Zielfußpunkt (NED, m): bei Hit die Zielmitte,
    /// sonst ein Punkt daneben am Boden.
    pub aim_offset: DVec3,
    /// Ohne Aufschaltung abgefeuert: sucht selbst.
    pub searching: bool,
}

impl Weapon {
    fn new(id: u32, kind: WeaponKind, position: DVec3, velocity: DVec3) -> Self {
        Self {
            id,
            kind,
            position,
            velocity,
            age_s: 0.0,
            target: None,
            outcome: None,
            critical: false,
            aim_offset: DVec3::ZERO,
            searching: false,
        }
    }

    /// Lage für die Darstellung: Längsachse entlang der Flugrichtung.
    pub fn attitude(&self) -> DQuat {
        let v = self.velocity;
        let yaw = detmath::atan2(v.y, v.x);
        let horiz = (v.x * v.x + v.y * v.y).sqrt();
        let pitch = detmath::atan2(-v.z, horiz);
        DQuat::from_euler(yaw, pitch, 0.0)
    }

    /// Bremsschirm offen (Fallschirmbombe).
    pub fn chute_open(&self) -> bool {
        let s = self.kind.spec();
        s.deployed_drag_area_m2 != s.drag_area_m2 && self.age_s >= s.deploy_s
    }

    /// Weltpunkt, den die Lenkwaffe anfliegt.
    fn aim_point(&self, targets: &[Target]) -> Option<DVec3> {
        let t = targets.get(self.target?)?;
        Some(DVec3::new(t.north, t.east, -t.elevation) + self.aim_offset)
    }
}

/// Was beim Einschlag geschah (bezogen auf das wichtigste getroffene Ziel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactResult {
    /// Ziel zerstört; `critical` = durch kritischen Treffer.
    Destroyed { target: usize, critical: bool },
    /// Ziel beschädigt, aber noch nicht zerstört.
    Damaged(usize),
    /// Ziel im Wirkbereich, aber die Waffe wirkt nicht gegen seine Klasse.
    Ineffective(usize),
    /// Kein Ziel getroffen.
    Ground,
}

impl ImpactResult {
    fn rank(self) -> u8 {
        match self {
            ImpactResult::Destroyed { .. } => 3,
            ImpactResult::Damaged(_) => 2,
            ImpactResult::Ineffective(_) => 1,
            ImpactResult::Ground => 0,
        }
    }
}

/// Ereignisse für Meldungen, Klang und Darstellung.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeaponEvent {
    Released {
        kind: WeaponKind,
        bay: usize,
        id: u32,
    },
    /// Auslösen bei geschlossenem Schacht.
    BayClosed,
    /// Waffenschacht defekt (Schadensmodell): kein Auslösen.
    BayFault,
    /// Gewählter Schacht leer.
    BayEmpty,
    /// Querlage zu groß zum Auslösen.
    BankLimit,
    /// Zu viele eigene Waffen in der Luft.
    Busy,
    /// Suchkopf hat ein Ziel erfasst (beim Abschuss oder selbst gesucht).
    Acquired {
        kind: WeaponKind,
        id: u32,
        target: usize,
    },
    /// Suchkopf hat sein Ziel verloren (Kegel, Sichtlinie).
    LockLost {
        kind: WeaponKind,
        id: u32,
    },
    Impact {
        kind: WeaponKind,
        id: u32,
        position: DVec3,
        result: ImpactResult,
    },
    /// Einschlag im Splitterbereich des eigenen Flugzeugs.
    FragWarning,
    /// Flugkörper hat seine Flugzeit überschritten und ist abgestürzt.
    Expired {
        kind: WeaponKind,
        id: u32,
    },
}

/// Bestand und Zustand der Waffen.
#[derive(Debug, Clone, PartialEq)]
pub struct Stores {
    pub bays: [Bay; BAYS],
    /// Gewählter Schacht (0-basiert).
    pub selected: usize,
    /// Klappen 0 (zu) .. 1 (offen).
    pub door: f64,
    /// Klappen sollen offen sein.
    pub door_cmd: bool,
    pub in_flight: Vec<Weapon>,
    next_id: u32,
    rng: Pcg32,
    /// Letzte Ereignisse mit Tick, neueste zuletzt.
    pub events: Vec<(u64, WeaponEvent)>,
}

/// Umgebung eines Waffen-Ticks.
#[derive(Debug)]
pub struct World<'a> {
    pub terrain: &'a Terrain,
    pub targets: &'a mut [Target],
    pub tick: u64,
    pub dt: f64,
    /// Position des eigenen Flugzeugs (NED).
    pub aircraft: DVec3,
}

/// Was der Abschuss über Flugzeug und Ziel wissen muss.
#[derive(Debug, Clone, Copy)]
pub struct Launch<'a> {
    pub tick: u64,
    pub position: DVec3,
    pub velocity: DVec3,
    pub attitude: DQuat,
    /// Aufgeschaltetes Ziel des Flugzeugs.
    pub lock: Option<usize>,
    pub targets: &'a [Target],
    pub terrain: &'a Terrain,
    /// Waffenschacht funktioniert (Schadensmodell).
    pub bay_ok: bool,
    /// Leistung der Bordoptik 0..1 (Schadensmodell).
    pub optics: f64,
}

/// Winkel zwischen zwei Richtungen (rad).
fn angle_between(a: DVec3, b: DVec3) -> f64 {
    let (la, lb) = (a.length(), b.length());
    if la <= 0.0 || lb <= 0.0 {
        return std::f64::consts::PI;
    }
    detmath::acos((a.dot(b) / (la * lb)).clamp(-1.0, 1.0))
}

/// Sieht der Suchkopf in `pos` mit Flugrichtung `dir` das Ziel? Prüft Klasse,
/// Kegel, Reichweite und – optisch – die Sichtlinie.
fn seeker_sees(
    kind: WeaponKind,
    pos: DVec3,
    dir: DVec3,
    t: &Target,
    terrain: &Terrain,
) -> Option<(f64, f64)> {
    let s = kind.spec();
    if !t.is_intact() || !s.seeker.sees(t.class) {
        return None;
    }
    let c = t.center_ned();
    let to = DVec3::new(c[0], c[1], c[2]) - pos;
    let range = to.length();
    let off = angle_between(dir, to);
    if off > s.seeker_cone_rad || range > kind.max_range(-pos.z - t.elevation) {
        return None;
    }
    if s.seeker == Seeker::Optical
        && !line_of_sight(|n, e| terrain.elevation(n, e), [pos.x, pos.y, -pos.z], [c[0], c[1], -c[2]])
    {
        return None;
    }
    Some((range, off))
}

impl Stores {
    pub fn new(loadout: &Loadout, seed: u64) -> Self {
        let bays = loadout.0.map(|kind| Bay { kind, count: kind.map_or(0, |k| k.spec().per_bay) });
        let selected = bays.iter().position(|b| b.count > 0).unwrap_or(0);
        Self {
            bays,
            selected,
            door: 0.0,
            door_cmd: false,
            in_flight: Vec::new(),
            next_id: 0,
            rng: Pcg32::new(seed, STREAM_WEAPONS),
            events: Vec::new(),
        }
    }

    /// Zustand des Zufallsstroms (für den Zustandshash).
    pub fn rng_state(&self) -> (u64, u64) {
        self.rng.raw_state()
    }

    /// Masse der Waffen an Bord (kg).
    pub fn mass_kg(&self) -> f64 {
        self.bays.iter().filter_map(|b| b.kind.map(|k| k.spec().mass_kg * f64::from(b.count))).sum()
    }

    pub fn door_open(&self) -> bool {
        self.door >= 1.0
    }

    /// Waffe des gewählten Schachts, falls noch eine da ist.
    pub fn selected_weapon(&self) -> Option<WeaponKind> {
        let b = self.bays[self.selected];
        b.kind.filter(|_| b.count > 0)
    }

    /// Nächster Schacht (mit Umlauf).
    pub fn next_bay(&mut self) {
        self.selected = (self.selected + 1) % BAYS;
    }

    fn log(&mut self, tick: u64, e: WeaponEvent) {
        if self.events.len() >= EVENT_LOG {
            self.events.remove(0);
        }
        self.events.push((tick, e));
    }

    /// Klappen bewegen; `speed` 0..1 aus dem Systemzustand (0 = klemmen).
    pub fn update_door(&mut self, dt: f64, speed: f64) {
        let step = speed * dt / DOOR_TIME_S;
        self.door = if self.door_cmd { (self.door + step).min(1.0) } else { (self.door - step).max(0.0) };
    }

    /// Endanflug würfeln und Zielpunkt festlegen.
    fn roll_outcome(&mut self, w: &mut Weapon, target: usize, targets: &[Target], quality: f64) {
        let (p_miss, p_near, _) = outcome_odds(quality);
        let roll = self.rng.next_f64();
        let crit_roll = self.rng.next_f64();
        let dir_roll = self.rng.next_f64();
        let dist_roll = self.rng.next_f64();
        let t = &targets[target];
        let around = |r: f64| {
            let a = dir_roll * std::f64::consts::TAU;
            DVec3::new(detmath::cos(a) * r, detmath::sin(a) * r, 0.0)
        };
        let (outcome, offset) = if roll < p_miss {
            (Outcome::Miss, around(t.class.radius() + 30.0 + 50.0 * dist_roll))
        } else if roll < p_miss + p_near {
            let r = t.class.radius() + w.kind.spec().lethal_radius_m * (0.5 + 0.5 * dist_roll);
            (Outcome::NearMiss, around(r))
        } else {
            (Outcome::Hit, DVec3::new(0.0, 0.0, -t.class.height() * 0.5))
        };
        w.target = Some(target);
        w.outcome = Some(outcome);
        w.critical = outcome == Outcome::Hit && crit_roll < CRITICAL_CHANCE;
        w.aim_offset = offset;
    }

    /// Löst die Waffe des gewählten Schachts aus, wenn alle Bedingungen
    /// erfüllt sind. Lenkwaffen übernehmen das aufgeschaltete Ziel, wenn ihr
    /// Suchkopf es sieht, sonst suchen sie selbst. Liefert die abgeworfene
    /// Masse (kg).
    pub fn release(&mut self, l: &Launch<'_>) -> f64 {
        let bay = self.bays[self.selected];
        let (_, _, roll) = l.attitude.to_euler();
        let event = match bay.kind {
            _ if !l.bay_ok => Some(WeaponEvent::BayFault),
            _ if !self.door_open() => Some(WeaponEvent::BayClosed),
            Some(_) if bay.count == 0 => Some(WeaponEvent::BayEmpty),
            None => Some(WeaponEvent::BayEmpty),
            _ if roll.abs() > MAX_RELEASE_BANK_RAD => Some(WeaponEvent::BankLimit),
            _ if self.in_flight.len() >= MAX_IN_FLIGHT => Some(WeaponEvent::Busy),
            _ => None,
        };
        if let Some(e) = event {
            self.log(l.tick, e);
            return 0.0;
        }
        let kind = bay.kind.expect("geprüft");
        self.bays[self.selected].count -= 1;
        let id = self.next_id;
        self.next_id += 1;
        let down = l.attitude.rotate(DVec3::new(0.0, 0.0, 1.0));
        let mut w =
            Weapon::new(id, kind, l.position + down * EJECT_OFFSET_M, l.velocity + down * EJECT_SPEED_MPS);
        self.log(l.tick, WeaponEvent::Released { kind, bay: self.selected, id });
        if kind.is_guided() {
            // Übergabe: Der Suchkopf muss das Ziel selbst sehen (Kegel um die
            // Flugrichtung, Reichweite, optisch mit Sichtlinie).
            let seen = l.lock.and_then(|i| {
                let t = l.targets.get(i)?;
                seeker_sees(kind, l.position, l.velocity, t, l.terrain).map(|(r, off)| (i, r, off, t))
            });
            match seen {
                Some((i, range, off, t)) => {
                    let sensor = if kind.spec().seeker == Seeker::Optical { l.optics } else { 1.0 };
                    let max = kind.max_range(-l.position.z - t.elevation);
                    let q = launch_quality(kind, range, off, sensor, max);
                    self.roll_outcome(&mut w, i, l.targets, q);
                    self.log(l.tick, WeaponEvent::Acquired { kind, id, target: i });
                }
                None => w.searching = true,
            }
        }
        self.in_flight.push(w);
        kind.spec().mass_kg
    }

    /// Alle Waffen im Flug einen Zeitschritt weiterbewegen; Einschläge auswerten.
    pub fn step(&mut self, world: &mut World<'_>) {
        let mut i = 0;
        while i < self.in_flight.len() {
            let before = self.in_flight[i];
            let mut w = before;
            if w.kind.is_guided() {
                self.seek(&mut w, world);
                guide(&mut w, world);
                // Lenkwaffe am Zielpunkt angekommen?
                if let Some(aim) = w.aim_point(world.targets)
                    && (w.position - aim).length() < (w.velocity.length() * world.dt * 1.5).max(4.0)
                {
                    self.in_flight.remove(i);
                    self.impact(world, &w, aim, true);
                    continue;
                }
            } else {
                fly(&mut w, world);
            }
            let ground = world.terrain.elevation(w.position.x, w.position.y);
            let alt = -w.position.z;
            if alt <= ground {
                // Einschlagpunkt zwischen den Ticks auf die Oberfläche legen.
                let prev_ground = world.terrain.elevation(before.position.x, before.position.y);
                let h0 = -before.position.z - prev_ground;
                let h1 = alt - ground;
                let t = if h0 > h1 { (h0 / (h0 - h1)).clamp(0.0, 1.0) } else { 1.0 };
                let mut p = before.position + (w.position - before.position) * t;
                p.z = -world.terrain.elevation(p.x, p.y);
                self.in_flight.remove(i);
                self.impact(world, &w, p, false);
                continue;
            }
            if w.age_s > w.kind.spec().life_s {
                self.in_flight.remove(i);
                self.log(world.tick, WeaponEvent::Expired { kind: w.kind, id: w.id });
                continue;
            }
            self.in_flight[i] = w;
            i += 1;
        }
    }

    /// Suchkopf: Ziel halten oder (ohne Aufschaltung abgefeuert) selbst suchen.
    fn seek(&mut self, w: &mut Weapon, world: &World<'_>) {
        if w.age_s < GUIDANCE_DELAY_S {
            return;
        }
        let los_tick = (world.tick + u64::from(w.id)).is_multiple_of(SEEKER_LOS_TICKS);
        if let Some(i) = w.target {
            let t = &world.targets[i];
            let c = t.center_ned();
            let to = DVec3::new(c[0], c[1], c[2]) - w.position;
            // Kegel jeden Tick, Sichtlinie seltener (teuer).
            let in_cone = t.is_intact()
                && (angle_between(w.velocity, to) <= w.kind.spec().seeker_cone_rad
                    || to.length() < w.velocity.length() * 0.5);
            let visible = !los_tick
                || w.kind.spec().seeker != Seeker::Optical
                || to.length() < 200.0
                || seeker_sees(w.kind, w.position, w.velocity, t, world.terrain).is_some();
            if !(in_cone && visible) {
                w.target = None;
                w.outcome = None;
                w.critical = false;
                self.log(world.tick, WeaponEvent::LockLost { kind: w.kind, id: w.id });
            }
            return;
        }
        if !w.searching || !los_tick {
            return;
        }
        // Selbstsuche: das nächste passende Ziel vor dem Suchkopf, auch zivile.
        let best = world
            .targets
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                seeker_sees(w.kind, w.position, w.velocity, t, world.terrain).map(|(r, off)| (r, off, i))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
        if let Some((range, off, i)) = best {
            let t = &world.targets[i];
            let max = w.kind.max_range(-w.position.z - t.elevation);
            let q = launch_quality(w.kind, range, off, 1.0, max);
            self.roll_outcome(w, i, world.targets, q);
            self.log(world.tick, WeaponEvent::Acquired { kind: w.kind, id: w.id, target: i });
        }
    }

    /// Schaden an einem Ziel anrichten; liefert das Ergebnis für dieses Ziel.
    fn apply(target: &mut Target, i: usize, amount: f64, critical: bool) -> ImpactResult {
        if critical {
            target.damage = 1.0;
            target.state = TargetState::Destroyed;
            return ImpactResult::Destroyed { target: i, critical: true };
        }
        target.damage = (target.damage + amount).min(1.0);
        // Rundungsreste (drei Drittel) zählen als ganz.
        if target.damage >= 1.0 - 1e-6 {
            target.damage = 1.0;
            target.state = TargetState::Destroyed;
            ImpactResult::Destroyed { target: i, critical: false }
        } else {
            target.state = TargetState::Damaged;
            ImpactResult::Damaged(i)
        }
    }

    /// `arrived` = Lenkwaffe hat ihren Zielpunkt erreicht (sonst Gelände).
    fn impact(&mut self, world: &mut World<'_>, w: &Weapon, p: DVec3, arrived: bool) {
        let v = w.velocity;
        let impact_rad = detmath::atan2(v.z, (v.x * v.x + v.y * v.y).sqrt());
        let result = match (w.target, w.outcome) {
            // Lenkwaffe am Zielpunkt mit gewürfeltem Endanflug: nur ihr Ziel zählt.
            (Some(i), Some(outcome)) if arrived && world.targets[i].is_intact() => {
                let e = w.kind.effectiveness(world.targets[i].class, impact_rad);
                match outcome {
                    Outcome::Miss => ImpactResult::Ground,
                    _ if e <= 0.0 => ImpactResult::Ineffective(i),
                    Outcome::NearMiss => Self::apply(&mut world.targets[i], i, e * NEAR_MISS_DAMAGE, false),
                    Outcome::Hit => Self::apply(&mut world.targets[i], i, e, w.critical),
                }
            }
            // Bomben (und blind fliegende Lenkwaffen): Schaden nach Abstand.
            _ => self.blast(world, w, p, impact_rad),
        };
        self.log(world.tick, WeaponEvent::Impact { kind: w.kind, id: w.id, position: p, result });
        if (world.aircraft - p).length() < w.kind.spec().frag_radius_m {
            self.log(world.tick, WeaponEvent::FragWarning);
        }
    }

    /// Physikalischer Einschlag: alle Ziele im Wirkradius, Schaden fällt mit
    /// dem Abstand vom Zielrand; voller Treffer würfelt einen kritischen.
    fn blast(&mut self, world: &mut World<'_>, w: &Weapon, p: DVec3, impact_rad: f64) -> ImpactResult {
        let reach = w.kind.spec().lethal_radius_m;
        let mut near: Vec<(f64, usize)> = world
            .targets
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_intact())
            .filter_map(|(i, t)| {
                let (dn, de) = (t.north - p.x, t.east - p.y);
                let miss = ((dn * dn + de * de).sqrt() - t.class.radius()).max(0.0);
                (miss <= reach).then_some((miss, i))
            })
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut result = ImpactResult::Ground;
        for (miss, i) in near {
            let e = w.kind.effectiveness(world.targets[i].class, impact_rad);
            let x = (miss / reach - FULL_EFFECT_FRACTION) / (1.0 - FULL_EFFECT_FRACTION);
            let falloff = (1.0 - x).clamp(0.0, 1.0);
            let this = if e <= 0.0 {
                ImpactResult::Ineffective(i)
            } else if e * falloff < MIN_DAMAGE {
                ImpactResult::Ground
            } else {
                let hit = falloff >= 1.0;
                let critical = hit && self.rng.next_f64() < CRITICAL_CHANCE;
                Self::apply(&mut world.targets[i], i, e * falloff, critical)
            };
            if this.rank() > result.rank() {
                result = this;
            }
        }
        result
    }
}

/// Vorausberechneter Aufschlag einer jetzt ausgelösten ungelenkten Waffe:
/// gleiche Startwerte und Flugbahn wie beim echten Abwurf. Liefert Punkt
/// (NED, auf der Oberfläche) und Fallzeit (s); `None`, wenn die Waffe vor
/// [`MAX_FALL_S`] nicht aufschlägt.
pub fn predict_release(
    kind: WeaponKind,
    position: DVec3,
    velocity: DVec3,
    attitude: DQuat,
    terrain: &Terrain,
    dt: f64,
) -> Option<(DVec3, f64)> {
    let down = attitude.rotate(DVec3::new(0.0, 0.0, 1.0));
    let mut w = Weapon::new(0, kind, position + down * EJECT_OFFSET_M, velocity + down * EJECT_SPEED_MPS);
    let mut targets: [Target; 0] = [];
    let world = World { terrain, targets: &mut targets, tick: 0, dt, aircraft: position };
    // Grobe Schritte reichen für die Anzeige: 4 Simulationsschritte auf einmal.
    let coarse = World { dt: dt * 4.0, ..world };
    while w.age_s < MAX_FALL_S {
        let before = w;
        fly(&mut w, &coarse);
        let ground = terrain.elevation(w.position.x, w.position.y);
        if -w.position.z <= ground {
            let h0 = -before.position.z - terrain.elevation(before.position.x, before.position.y);
            let h1 = -w.position.z - ground;
            let t = if h0 > h1 { (h0 / (h0 - h1)).clamp(0.0, 1.0) } else { 1.0 };
            let mut p = before.position + (w.position - before.position) * t;
            p.z = -terrain.elevation(p.x, p.y);
            return Some((p, before.age_s + (w.age_s - before.age_s) * t));
        }
    }
    None
}

/// Ein Zeitschritt Bombenflugbahn: Schwerkraft und Widerstand (semi-implizit,
/// stabil bei 120 Hz).
fn fly(w: &mut Weapon, world: &World<'_>) {
    let s = w.kind.spec();
    let dt = world.dt;
    let air = atmosphere::isa(-w.position.z);
    let speed = w.velocity.length();
    let drag_area = if w.chute_open() { s.deployed_drag_area_m2 } else { s.drag_area_m2 };
    let mut accel = DVec3::new(0.0, 0.0, G0);
    if speed > 0.0 {
        accel -= w.velocity / speed * (0.5 * air.density * speed * speed * drag_area / s.mass_kg);
    }
    w.velocity += accel * dt;
    w.position += w.velocity * dt;
    w.age_s += dt;
}

/// Dreht die Einheitsrichtung `from` um höchstens `max_angle` zu `to`.
fn turn_towards(from: DVec3, to: DVec3, max_angle: f64) -> DVec3 {
    let angle = angle_between(from, to);
    if angle <= max_angle || angle < 1e-9 {
        return to;
    }
    let axis = from.cross(to);
    let axis = if axis.length() < 1e-9 { DVec3::new(0.0, 0.0, 1.0).cross(from) } else { axis };
    DQuat::from_axis_angle(axis.normalize_or_zero(), max_angle).rotate(from).normalize_or_zero()
}

/// Ein Zeitschritt Lenkwaffe: erst frei aus dem Schacht fallen, dann mit
/// begrenzter Drehrate auf den Zielpunkt drehen und auf Marschfahrt gehen.
/// Ohne Ziel fliegt sie geradeaus und senkt langsam die Nase.
fn guide(w: &mut Weapon, world: &World<'_>) {
    if w.age_s < GUIDANCE_DELAY_S {
        fly(w, world);
        return;
    }
    let s = w.kind.spec();
    let dt = world.dt;
    let speed = w.velocity.length().max(1.0);
    let dir = w.velocity / speed;
    let wanted = match w.aim_point(world.targets) {
        Some(aim) => (aim - w.position).normalize_or_zero(),
        None => {
            let horiz = DVec3::new(dir.x, dir.y, 0.0).normalize_or_zero();
            // Nase sinkt: Zielrichtung leicht unter der aktuellen.
            let p = detmath::asin((-dir.z).clamp(-1.0, 1.0)) - BLIND_NOSE_DROP * dt;
            let (sp, cp) = detmath::sin_cos(p);
            DVec3::new(horiz.x * cp, horiz.y * cp, -sp)
        }
    };
    let mut new_dir = turn_towards(dir, wanted, s.turn_rate_rad * dt);
    if s.min_glide_rad > 0.0 {
        // Gleitbombe: mindestens den Gleitwinkel sinken, nie steigen.
        let min_down = detmath::sin(s.min_glide_rad);
        if new_dir.z < min_down {
            let horiz = DVec3::new(new_dir.x, new_dir.y, 0.0).normalize_or_zero();
            let c = detmath::cos(s.min_glide_rad);
            new_dir = DVec3::new(horiz.x * c, horiz.y * c, min_down);
        }
    }
    let new_speed = speed + (s.cruise_mps - speed).clamp(-s.accel_mps2 * dt, s.accel_mps2 * dt);
    w.velocity = new_dir * new_speed;
    w.position += w.velocity * dt;
    w.age_s += dt;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loadout_text_roundtrip() {
        let l = Loadout::default();
        assert_eq!(l.to_text(), "fall,chute,glide,agm");
        assert_eq!(Loadout::parse(&l.to_text()), Some(l));
        let e = Loadout::parse("arm,-,ship,fall").unwrap();
        assert_eq!(e.0[1], None);
        assert_eq!(Loadout::parse("arm,ship"), None);
        assert_eq!(Loadout::parse("arm,ship,x,fall"), None);
    }

    #[test]
    fn damage_per_hit_matrix() {
        let k = WeaponKind::FreeFall;
        assert_eq!(k.effectiveness(TargetClass::Soft, 0.5), 1.0);
        assert_eq!(k.effectiveness(TargetClass::Bridge, 0.5), 0.5, "Brücke: zwei Bomben");
        assert_eq!(k.effectiveness(TargetClass::Hardened, deg(40.0)), 0.0);
        assert_eq!(k.effectiveness(TargetClass::Hardened, deg(70.0)), 0.5);
        assert_eq!(WeaponKind::AntiRadar.effectiveness(TargetClass::Soft, 1.0), 0.0);
        assert_eq!(WeaponKind::AntiShip.effectiveness(TargetClass::Building, 1.0), 0.0);
        for w in WeaponKind::ALL {
            for c in TargetClass::ALL {
                let e = w.effectiveness(c, 1.2);
                assert!((0.0..=1.0).contains(&e), "{w:?} gegen {c:?}");
            }
        }
    }

    #[test]
    fn quality_and_odds() {
        let k = WeaponKind::AirToGround;
        let max = k.spec().max_range_m;
        assert_eq!(launch_quality(k, max * 0.4, 0.0, 1.0, max), 1.0);
        let far = launch_quality(k, max, 0.0, 1.0, max);
        assert!((far - QUALITY_AT_MAX_RANGE).abs() < 1e-9);
        let edge = launch_quality(k, max * 0.4, k.spec().seeker_cone_rad, 1.0, max);
        assert!((edge - QUALITY_AT_CONE_EDGE).abs() < 1e-9);
        assert_eq!(launch_quality(k, max * 0.4, 0.0, 0.5, max), 0.5);
        // Sweet Spot: von der Reichweitengrenze ansteigend, danach wieder fallend.
        let curve: Vec<f64> = (0..=20).map(|i| range_quality(f64::from(i) / 20.0)).collect();
        let peak = curve.iter().cloned().fold(0.0, f64::max);
        assert_eq!(peak, 1.0);
        assert!(range_quality(0.95) < range_quality(0.75) && range_quality(0.75) < range_quality(0.5));
        assert!(range_quality(0.05) < range_quality(0.2) && range_quality(0.2) < range_quality(0.3));
        assert!(range_quality(0.0) < 0.5 && range_quality(1.0) < 0.5);
        for q in [0.0, 0.4, 1.0] {
            let (m, n, h) = outcome_odds(q);
            assert!((m + n + h - 1.0).abs() < 1e-12 && m >= 0.0 && n >= 0.0 && h >= 0.0);
        }
        assert!(outcome_odds(1.0).2 > 0.75 && outcome_odds(0.0).0 > 0.5);
    }

    fn bridge() -> Target {
        Target {
            name: "BRIDGE",
            class: TargetClass::Bridge,
            civilian: false,
            role: crate::targets::TargetRole::None,
            north: 0.0,
            east: 0.0,
            elevation: 0.0,
            heading: 0.0,
            state: TargetState::Intact,
            damage: 0.0,
        }
    }

    #[test]
    fn damage_accumulates_and_critical_destroys_at_once() {
        let e = WeaponKind::FreeFall.effectiveness(TargetClass::Bridge, 0.5);
        let mut t = bridge();
        assert_eq!(Stores::apply(&mut t, 7, e, false), ImpactResult::Damaged(7));
        assert_eq!(t.state, TargetState::Damaged);
        assert!(t.is_intact());
        assert_eq!(
            Stores::apply(&mut t, 7, e, false),
            ImpactResult::Destroyed { target: 7, critical: false }
        );
        assert!(!t.is_intact());
        let mut t = bridge();
        assert_eq!(Stores::apply(&mut t, 7, e, true), ImpactResult::Destroyed { target: 7, critical: true });
        // Drei Drittel zählen als ganz.
        let mut t = bridge();
        for _ in 0..3 {
            Stores::apply(&mut t, 7, 0.34 - 0.0067, false);
        }
        assert_eq!(t.state, TargetState::Damaged, "0,99 ist noch nicht zerstört");
        let mut t = bridge();
        for _ in 0..3 {
            Stores::apply(&mut t, 7, 1.0 / 3.0, false);
        }
        assert_eq!(t.state, TargetState::Destroyed);
    }

    #[test]
    fn glide_bomb_range_depends_on_height() {
        let k = WeaponKind::GuidedBomb;
        assert!(k.max_range(300.0) < 2_000.0);
        assert_eq!(k.max_range(5_000.0), k.spec().max_range_m);
        assert_eq!(WeaponKind::AirToGround.max_range(10.0), MISSILE.max_range_m);
    }

    #[test]
    fn turning_is_rate_limited() {
        let a = DVec3::new(1.0, 0.0, 0.0);
        let b = DVec3::new(0.0, 1.0, 0.0);
        let c = turn_towards(a, b, 0.1);
        assert!((angle_between(a, c) - 0.1).abs() < 1e-9);
        assert_eq!(turn_towards(a, b, 2.0), b);
    }

    #[test]
    fn retarded_bomb_falls_slower_than_free_fall() {
        let terrain = Terrain::new(1);
        let mut targets: Vec<Target> = Vec::new();
        let world = World {
            terrain: &terrain,
            targets: &mut targets,
            tick: 0,
            dt: 1.0 / 120.0,
            aircraft: DVec3::ZERO,
        };
        let start = |kind| Weapon::new(0, kind, DVec3::new(0.0, 0.0, -3_000.0), DVec3::new(250.0, 0.0, 0.0));
        let (mut a, mut b) = (start(WeaponKind::FreeFall), start(WeaponKind::Retarded));
        for _ in 0..(120 * 5) {
            fly(&mut a, &world);
            fly(&mut b, &world);
        }
        // Nach 5 s: Freifall fast ungebremst vorwärts, Fallschirm weit zurück.
        assert!(a.position.x > 1_150.0, "Freifall {:.0} m", a.position.x);
        // Das Flugzeug (250 m/s) ist nach 5 s bei 1250 m: Die Fallschirmbombe
        // liegt außerhalb des Splitterbereichs zurück, die Freifallbombe nicht.
        let spec = WeaponKind::Retarded.spec();
        assert!(1_250.0 - b.position.x > spec.frag_radius_m, "Fallschirm {:.0} m", b.position.x);
        assert!(1_250.0 - a.position.x < spec.frag_radius_m, "Freifall {:.0} m", a.position.x);
        assert!(b.velocity.length() < a.velocity.length() * 0.6);
        // Freifall ohne Widerstand wäre 0,5·g·t² ≈ 123 m gefallen.
        let fell = a.position.z + 3_000.0;
        assert!(fell > 110.0 && fell < 125.0, "gefallen {fell:.1} m");
    }
}
