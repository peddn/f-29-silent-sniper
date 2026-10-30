//! Waffen (M-014, Wirkung M-010 bis M-012).
//!
//! Vier interne Schächte mit je einer Waffenart. Der Schacht wird von Hand
//! geöffnet; ausgelöst wird nur bei offenem Schacht und mäßiger Querlage.
//!
//! Alle Waffen fliegen **physikalisch** – wie im Original entscheidet der
//! Flug, nicht ein Würfel beim Abschuss (Umbau v0.2.9):
//! - **Bomben**: Massenpunkt mit Schwerkraft und Luftwiderstand; Schaden nach
//!   dem Abstand des Einschlags vom Ziel.
//! - **Raketen**: übernehmen das aufgeschaltete Ziel, wenn es im
//!   **Abschusskreis** um die Nase liegt und ihr Suchkopf es sieht, sonst
//!   fliegen sie geradeaus und schalten auf, was ihnen vor den Suchkopf kommt.
//!   Sie verfolgen das Ziel (reine Verfolgung, begrenzte Drehrate), solange es
//!   im Suchkegel bleibt; liegt es weit hinter ihnen, geben sie auf. Ein
//!   Annäherungszünder löst aus, wenn sie dem Ziel näher kommen als ihr
//!   Wirkradius. **Die Wirkung wird gewürfelt** wie im Original: Die
//!   Wirkungsmatrix ist die Chance, das Ziel zu zerstören, sonst
//!   „wirkungslos“.
//! - **Laserbombe** (GBU-12): fällt wie eine Bombe und lenkt auf das Ziel,
//!   solange das Flugzeug es optisch aufgeschaltet hält (Laser).
//! - Ob ein Schuss treffen würde, rechnet [`launch_zone`] mit genau diesem Flug
//!   voraus („SHOOT“).
//! - **Feindliche Boden-Luft-Raketen** (M-021) fliegen in derselben Liste und
//!   mit demselben Flug, Ziel ist das Flugzeug: Startet eine Radarstellung,
//!   lenkt die Rakete halbaktiv – weit weg nur, solange die Stellung das
//!   Flugzeug beim letzten Umlauf erfasst hat, nah mit eigenem Suchkopf. Ein
//!   Treffer beschädigt zufällige Bordsysteme (keine Trefferpunkte).
//! - **Täuschkörper** (M-021, wie im Original): Düppel und Fackeln bleiben am
//!   Abwurfort stehen und brennen einige Sekunden. Liegt einer im Suchkegel
//!   einer Rakete der passenden Lenkart (Düppel: Radar, Fackel: Infrarot /
//!   optisch), lenkt sie auf ihn statt auf ihr Ziel – kein Würfel.
//! - **Vorhalt** für alle Lenkwaffen: Sie fliegen auf den Treffpunkt eines
//!   bewegten Ziels (Abweichung vom Original, damit Hineindrehen nicht
//!   rettet); bei stehenden Zielen ist das die reine Verfolgung des Originals.
//!   Der Vorhaltfaktor je Waffe stellt die Gefährlichkeit ein.
//!
//! Werte: eigene Größenordnungen aus öffentlichen Daten realer Waffen und
//! Spielbalance, siehe M-014.

use crate::atmosphere::{self, G0};
use crate::detmath;
use crate::math::{DQuat, DVec3};
use crate::rng::Pcg32;
use crate::sensors::line_of_sight;
use crate::systems::System;
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
/// Wahrscheinlichkeit eines kritischen Bombentreffers.
pub const CRITICAL_CHANCE: f64 = 0.01;
/// Längste Flugzeit einer ungelenkten Waffe (s), Schutz gegen Endlosflug.
const MAX_FALL_S: f64 = 180.0;
/// Sicherheitsabstand über dem Boden, ab dem die Vorhersage das Gelände nur
/// noch selten abfragt (m).
const PREDICT_MARGIN_M: f64 = 300.0;
/// Nur so viele Ereignisse merken (für Meldungen).
const EVENT_LOG: usize = 16;
/// Lenkwaffen fallen erst frei aus dem Schacht, dann setzt die Lenkung ein (s).
const GUIDANCE_DELAY_S: f64 = 0.4;
/// Suchkopf und Laser alle so viele Ticks prüfen (Sichtlinie ist teuer).
const SEEKER_TICKS: u64 = 12;
/// Liegt das Ziel mehr als so weit hinter der Flugrichtung, gibt die Rakete
/// auf (wie im Original, rad, 135°).
const GIVE_UP_RAD: f64 = 3.0 * std::f64::consts::FRAC_PI_4;
/// Vorausberechnung: so viele Simulationsschritte auf einmal.
const PREDICT_STEPS: f64 = 4.0;
/// Feindliche Raketen höchstens so viele gleichzeitig in der Luft (je
/// Stellung eine, wie im Original).
pub const MAX_HOSTILE: usize = 4;
/// Näher als so (m) lenkt eine Boden-Luft-Rakete mit eigenem Suchkopf, weiter
/// weg nur mit der Beleuchtung ihrer Stellung (halbaktiv).
pub const SEMI_ACTIVE_M: f64 = 5_000.0;
/// Boden-Luft-Raketen starten mindestens so steil über der Linie zum
/// Flugzeug (rad, 10° – innerhalb des Suchkegels).
const SAM_DIRECT_RAD: f64 = 10.0 * std::f64::consts::PI / 180.0;
/// Vorhalt höchstens für so viele Sekunden Restflugzeit.
const MAX_LEAD_S: f64 = 10.0;
/// Startgeschwindigkeit der Boden-Luft-Raketen (m/s).
const SAM_LAUNCH_MPS: f64 = 60.0;
/// Täuschkörper: Vorrat je Art, höchstens so viele gleichzeitig, Brenndauer (s).
pub const DECOY_STOCK: u8 = 20;
pub const MAX_DECOYS: usize = 3;
pub const DECOY_LIFE_S: f64 = 5.0;
/// So viele Bordsysteme beschädigt ein Raketentreffer (Original: Konflikt-
/// stufe plus eins; Schwierigkeitsgrade folgen).
pub const HIT_SYSTEMS: usize = 2;

/// Abschusskreis um die Flugzeugnase (halber Öffnungswinkel, rad, 8°): Nur ein
/// Ziel darin übernimmt eine Rakete (Spielerentscheidung v0.2.7).
pub const LAUNCH_CIRCLE_RAD: f64 = 8.0 * std::f64::consts::PI / 180.0;

/// Waffenarten (M-014), benannt nach den realen Vorbildern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponKind {
    /// Freifallbombe.
    FreeFall,
    /// Fallschirmbombe: bremst kurz nach dem Abwurf stark ab, fällt steil und
    /// lässt dem Flugzeug Zeit, sich aus dem Splitterbereich zu entfernen.
    Retarded,
    /// Lasergelenkte Bombe: das Flugzeug beleuchtet das Ziel bis zum Einschlag.
    GuidedBomb,
    /// Optisch gelenkte Luft-Boden-Rakete.
    AirToGround,
    /// Radarjäger: fliegt nur Flugabwehrstellungen (Radar) an.
    AntiRadar,
    /// Seezielrakete: fliegt nur Schiffe an.
    AntiShip,
    /// Boden-Luft-Rakete mittlerer Reichweite (SAM-Stellungen).
    MediumRangeSam,
    /// Langstrecken-Boden-Luft-Rakete (Frühwarnradare).
    LongRangeSam,
    /// Kurzstrecken-Boden-Luft-Rakete (Kurzstrecken-Flugabwehr).
    ShortRangeSam,
}

/// Suchkopf einer gelenkten Waffe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seeker {
    /// Ungelenkt.
    None,
    /// Optisch: braucht freie Sicht, alle Bodenziele.
    Optical,
    /// Laser: das Flugzeug muss das Ziel optisch aufgeschaltet halten.
    Laser,
    /// Passiv auf Radarstrahlung: nur Flugabwehrstellungen, ohne Sichtlinie.
    Radiation,
    /// Aktives Radar gegen Schiffe, ohne Sichtlinie.
    Naval,
    /// Halbaktives Radar gegen das Flugzeug (Boden-Luft-Raketen).
    SemiActive,
}

impl Seeker {
    /// Lässt sich dieser Suchkopf von dieser Art Täuschkörper ablenken?
    pub fn lured_by(self, decoy: DecoyKind) -> bool {
        matches!((self, decoy), (Seeker::SemiActive, DecoyKind::Chaff) | (Seeker::Optical, DecoyKind::Flare))
    }

    /// Kann dieser Suchkopf ein Ziel der Klasse überhaupt erfassen?
    pub fn sees(self, class: TargetClass) -> bool {
        match self {
            Seeker::None => false,
            Seeker::Optical | Seeker::Laser => true,
            Seeker::Radiation => class.is_radar(),
            Seeker::Naval => class.is_naval(),
            Seeker::SemiActive => false,
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
    /// Marschfahrt der Rakete (m/s), die sie bis zum Ende hält (0 = Bombe).
    pub cruise_mps: f64,
    /// Beschleunigung auf Marschfahrt (m/s²).
    pub accel_mps2: f64,
    /// Größte Drehrate der Flugrichtung (rad/s).
    pub turn_rate_rad: f64,
    /// Startwinkel einer Boden-Luft-Rakete über der Waagerechten (rad; die
    /// Langstreckenrakete fast senkrecht wie im Original), mindestens knapp
    /// über der Linie zum Flugzeug.
    pub launch_rad: f64,
    /// Boost (s): So lange steigt sie nach dem Start ungelenkt.
    pub boost_s: f64,
    /// Bogen: Die Rakete zielt um diesen Anteil der Entfernung über das Ziel
    /// (wie im Original; schrumpft beim Annähern, 0 = direkt).
    pub loft: f64,
    /// Vorhaltfaktor 0..1 gegen bewegte Ziele: 0 = reine Verfolgung (wie im
    /// Original, leicht auszukurven), 1 = voller Vorhalt (Kollisionskurs).
    pub lead: f64,
    /// Reichweite des Suchkopfs (m) bei Übergabe und Selbstsuche (Laserbombe:
    /// ohne Belang, sie folgt dem Laserfleck).
    pub max_range_m: f64,
    /// Halber Öffnungswinkel des Suchkopfs bei der Selbstsuche (rad).
    pub seeker_cone_rad: f64,
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
    label: "MK-82",
    name: "MK-82 BOMB",
    mass_kg: 241.0,
    drag_area_m2: 0.020,
    deployed_drag_area_m2: 0.020,
    deploy_s: 0.0,
    per_bay: 2,
    seeker: Seeker::None,
    cruise_mps: 0.0,
    accel_mps2: 0.0,
    turn_rate_rad: 0.0,
    launch_rad: 0.0,
    boost_s: 0.0,
    loft: 0.0,
    // Alle Lenkwaffen mit voller Gefährlichkeit (Spielerwunsch v0.2.9).
    lead: 1.0,
    max_range_m: 0.0,
    seeker_cone_rad: 0.0,
    lethal_radius_m: 30.0,
    frag_radius_m: 180.0,
    life_s: MAX_FALL_S,
};

/// Gemeinsame Grundwerte der Lenkwaffen.
const MISSILE: WeaponSpec = WeaponSpec {
    kind: WeaponKind::AirToGround,
    label: "AGM-65",
    name: "AGM-65 MAVERICK",
    mass_kg: 210.0,
    drag_area_m2: 0.035,
    deployed_drag_area_m2: 0.035,
    deploy_s: 0.0,
    per_bay: 2,
    seeker: Seeker::Optical,
    // Deutlich schneller als das Flugzeug, beschleunigt aber gemächlich und
    // bleibt so länger sichtbar (Spielerwunsch v0.2.7).
    cruise_mps: 700.0,
    accel_mps2: 70.0,
    turn_rate_rad: deg(25.0),
    launch_rad: 0.0,
    boost_s: 0.0,
    loft: 0.0,
    lead: 1.0,
    max_range_m: 16_000.0,
    seeker_cone_rad: deg(25.0),
    lethal_radius_m: 8.0,
    frag_radius_m: 90.0,
    life_s: 70.0,
};

/// Boden-Luft-Rakete mittlerer Reichweite (Vorbild SA-6): mittelschnell,
/// mittlerer Boost und Bogen.
const SAM: WeaponSpec = WeaponSpec {
    kind: WeaponKind::MediumRangeSam,
    label: "SA-6",
    name: "SA-6 GAINFUL",
    mass_kg: 600.0,
    per_bay: 0,
    seeker: Seeker::SemiActive,
    cruise_mps: 650.0,
    accel_mps2: 250.0,
    turn_rate_rad: deg(22.0),
    launch_rad: deg(45.0),
    boost_s: 1.5,
    loft: 0.1,
    max_range_m: 25_000.0,
    // Suchkegel wie im Original
    seeker_cone_rad: deg(22.0),
    lethal_radius_m: 20.0,
    frag_radius_m: 0.0,
    life_s: 50.0,
    ..MISSILE
};

const SPECS: [WeaponSpec; 9] = [
    BOMB,
    WeaponSpec {
        kind: WeaponKind::Retarded,
        label: "MK-82R",
        name: "MK-82 SNAKEYE",
        deployed_drag_area_m2: 0.9,
        deploy_s: 0.3,
        ..BOMB
    },
    WeaponSpec {
        kind: WeaponKind::GuidedBomb,
        label: "GBU-12",
        name: "GBU-12 PAVEWAY II",
        mass_kg: 227.0,
        drag_area_m2: 0.022,
        deployed_drag_area_m2: 0.022,
        seeker: Seeker::Laser,
        // Steuerflächen: lenkt die Flugbahn mit bis zu 6°/s auf den Laserfleck.
        turn_rate_rad: deg(6.0),
        ..BOMB
    },
    MISSILE,
    WeaponSpec {
        kind: WeaponKind::AntiRadar,
        label: "AGM-88",
        name: "AGM-88 HARM",
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
        label: "AGM-84",
        name: "AGM-84 HARPOON",
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
    SAM,
    // Langstreckenrakete der Frühwarnradare (Vorbild SA-5): langsam, langer
    // fast senkrechter Boost und ausgeprägter Bogen – kommt über jeden Berg
    // und auch aus großer Entfernung, ist aber lange unterwegs.
    WeaponSpec {
        kind: WeaponKind::LongRangeSam,
        label: "SA-5",
        name: "SA-5 GAMMON",
        mass_kg: 7_000.0,
        cruise_mps: 450.0,
        accel_mps2: 100.0,
        turn_rate_rad: deg(10.0),
        launch_rad: deg(85.0),
        boost_s: 4.0,
        loft: 0.25,
        max_range_m: 60_000.0,
        lethal_radius_m: 30.0,
        life_s: 150.0,
        ..SAM
    },
    // Kurzstreckenrakete mit Infrarotsuchkopf (Vorbild SA-13): sehr schnell
    // und wendig, startet ohne Boost direkt auf das Ziel. Sucht selbst (keine
    // Beleuchtung durch die Stellung), braucht aber freie Sicht; Fackeln
    // lenken sie ab.
    WeaponSpec {
        kind: WeaponKind::ShortRangeSam,
        label: "SA-13",
        name: "SA-13 GOPHER",
        seeker: Seeker::Optical,
        mass_kg: 130.0,
        cruise_mps: 900.0,
        accel_mps2: 400.0,
        turn_rate_rad: deg(30.0),
        launch_rad: 0.0,
        boost_s: 0.0,
        loft: 0.0,
        max_range_m: 10_000.0,
        lethal_radius_m: 12.0,
        life_s: 20.0,
        ..SAM
    },
];

impl WeaponKind {
    /// Die Waffen für die Schächte (ohne die feindlichen Raketen).
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

    /// Fernsehsuchkopf: Die Waffe sendet ihr Bild (WPN-CAM-Seite).
    pub fn has_camera(self) -> bool {
        self.spec().seeker == Seeker::Optical
    }

    /// Kennwort für `--loadout` und Replays.
    pub fn code(self) -> &'static str {
        match self {
            WeaponKind::FreeFall => "fall",
            WeaponKind::Retarded => "chute",
            WeaponKind::GuidedBomb => "lgb",
            WeaponKind::AirToGround => "agm",
            WeaponKind::AntiRadar => "arm",
            WeaponKind::AntiShip => "ship",
            WeaponKind::MediumRangeSam => "sa6",
            WeaponKind::LongRangeSam => "sa5",
            WeaponKind::ShortRangeSam => "sa13",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            // Ältere Replays (bis v0.2.6: Gleitbombe)
            "glide" => Some(WeaponKind::GuidedBomb),
            _ => Self::ALL.into_iter().find(|k| k.code() == code),
        }
    }

    /// Boden-Luft-Rakete, die eine Radarstellung der Klasse startet.
    pub fn sam_for(class: TargetClass) -> Option<Self> {
        match class {
            TargetClass::AirDefense => Some(WeaponKind::MediumRangeSam),
            TargetClass::EarlyWarning => Some(WeaponKind::LongRangeSam),
            TargetClass::ShortRangeSam => Some(WeaponKind::ShortRangeSam),
            _ => None,
        }
    }

    /// Rakete mit Abschusskreis (alle Lenkwaffen außer der Laserbombe).
    pub fn uses_launch_circle(self) -> bool {
        self.is_guided() && self.spec().seeker != Seeker::Laser
    }

    /// Schaden pro Treffer gegen eine Zielklasse (M-010): 1 = ein Treffer
    /// zerstört, 0,5 = zwei Treffer, 0 = wirkungslos. `impact_rad` ist der
    /// Eintreffwinkel unter der Waagerechten (gehärtete Ziele, M-012).
    pub fn effectiveness(self, class: TargetClass, impact_rad: f64) -> f64 {
        use TargetClass as C;
        use WeaponKind as W;
        let steep = impact_rad >= HARDENED_MIN_IMPACT_RAD;
        match (self, class) {
            (W::MediumRangeSam | W::LongRangeSam | W::ShortRangeSam, _) => 0.0,
            (W::AntiRadar, C::AirDefense | C::EarlyWarning | C::ShortRangeSam) => 1.0,
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
            (
                _,
                C::Soft
                | C::ParkedAircraft
                | C::AirDefense
                | C::EarlyWarning
                | C::ShortRangeSam
                | C::ShipSmall,
            ) => 1.0,
            (W::GuidedBomb | W::AirToGround, C::Armored) => 1.0,
            (W::AirToGround, C::Building) => 0.34,
            (W::AirToGround, C::Bridge | C::ShipLarge) => 0.25,
            (W::GuidedBomb, C::ShipLarge) => 0.5,
            (_, C::ShipLarge) => 0.34,
            (_, C::Armored | C::Building | C::Bridge) => 0.5,
        }
    }
}

/// Zone einer Lenkwaffe gegen das aufgeschaltete Ziel (Zielrahmen und
/// Feuersignal im HUD).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchZone {
    /// Waffe wirkt nicht gegen dieses Ziel oder ihr Suchkopf sieht es nicht.
    NoEffect,
    /// Die Waffe würde das Ziel nicht übernehmen (Reichweite, Abschusskreis,
    /// keine optische Aufschaltung).
    NoLock,
    /// Sie würde es übernehmen, aber laut Vorausberechnung verfehlen.
    Locked,
    /// Die Vorausberechnung trifft: gute Abschussbedingungen („SHOOT“).
    Good,
}

/// Zone der Waffe `kind` gegen Ziel `i`, wenn sie jetzt unter `l` ausgelöst
/// würde: Übergabe wie beim echten Auslösen, dann derselbe Flug in groben
/// Schritten vorausgerechnet (Sichtlinie und Laser als gehalten angenommen).
pub fn launch_zone(kind: WeaponKind, l: &Launch<'_>, i: usize) -> LaunchZone {
    let t = &l.targets[i];
    if !kind.spec().seeker.sees(t.class) || kind.effectiveness(t.class, std::f64::consts::FRAC_PI_2) <= 0.0 {
        return LaunchZone::NoEffect;
    }
    let mut w = Weapon::launched(0, kind, l);
    if w.ground_target() != Some(i) {
        return LaunchZone::NoLock;
    }
    let view = View {
        targets: l.targets,
        terrain: l.terrain,
        designated: Some(i),
        aircraft: None,
        decoys: &[],
        painted: false,
        los: false,
    };
    let dt = l.dt * PREDICT_STEPS;
    let mut step = 0_u32;
    loop {
        if step.is_multiple_of(3) {
            seek(&mut w, &view);
        }
        step += 1;
        let aim = w.target.filter(|_| w.tracking).and_then(|t| view.locate(t));
        match advance(&mut w, aim, l.terrain, dt) {
            Fate::Flying => {}
            Fate::Fuze(_) => return LaunchZone::Good,
            Fate::Ground(p) => {
                let (dn, de) = (t.north - p.x, t.east - p.y);
                let near = (dn * dn + de * de).sqrt() - t.class.radius() < kind.spec().lethal_radius_m;
                return if near { LaunchZone::Good } else { LaunchZone::Locked };
            }
            Fate::Expired => return LaunchZone::Locked,
        }
    }
}

/// Beladung der vier Schächte (eine Waffenart je Schacht oder leer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loadout(pub [Option<WeaponKind>; BAYS]);

impl Default for Loadout {
    /// Standardbeladung: Mk 82, Snakeye, GBU-12, Maverick.
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

    /// `fall,chute,lgb,agm` – `-` für einen leeren Schacht.
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

/// Was eine Lenkwaffe verfolgt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Track {
    /// Bodenziel (Index in `Sim::targets`).
    Ground(usize),
    /// Das eigene Flugzeug (feindliche Raketen).
    Aircraft,
    /// Ein Täuschkörper (Nummer).
    Decoy(u32),
}

/// Art eines Täuschkörpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoyKind {
    /// Düppel gegen Radarlenkung.
    Chaff,
    /// Fackel gegen Infrarot- bzw. optische Lenkung.
    Flare,
}

impl DecoyKind {
    pub fn label(self) -> &'static str {
        match self {
            DecoyKind::Chaff => "CHAFF",
            DecoyKind::Flare => "FLARES",
        }
    }
}

/// Ein Täuschkörper: steht am Abwurfort, bis er ausgebrannt ist.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decoy {
    pub id: u32,
    pub kind: DecoyKind,
    /// NED (m).
    pub position: DVec3,
    pub age_s: f64,
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
    /// Verfolgtes Ziel; `None`, wenn es keines (mehr) hat.
    pub target: Option<Track>,
    /// Der Suchkopf (bzw. der Laser) hatte das Ziel bei der letzten Prüfung;
    /// sonst fliegt die Waffe ungelenkt weiter.
    pub tracking: bool,
    /// Ohne Aufschaltung abgefeuert: sucht selbst.
    pub searching: bool,
    /// Richtung, in der die Rakete ohne Ziel weiterfliegt: die Abschusslage
    /// (Flugzeugnase).
    pub boresight: DVec3,
    /// Feindliche Rakete: gestartet von der Radarstellung mit diesem Index in
    /// `Sim::targets`; `None` = eigene Waffe.
    pub owner: Option<usize>,
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
            tracking: false,
            searching: false,
            boresight: velocity.normalize_or_zero(),
            owner: None,
        }
    }

    /// Frisch ausgelöst unter `l`, mit Übergabe des aufgeschalteten Ziels:
    /// Laserbombe bei optischer Aufschaltung; Rakete, wenn das Ziel im
    /// Abschusskreis liegt und ihr Suchkopf es sieht (optisch nur bei optischer
    /// Aufschaltung) – sonst sucht sie selbst.
    fn launched(id: u32, kind: WeaponKind, l: &Launch<'_>) -> Self {
        let down = l.attitude.rotate(DVec3::new(0.0, 0.0, 1.0));
        let mut w =
            Weapon::new(id, kind, l.position + down * EJECT_OFFSET_M, l.velocity + down * EJECT_SPEED_MPS);
        let lock = l.lock.and_then(|i| l.targets.get(i).map(|t| (i, t)));
        match kind.spec().seeker {
            Seeker::None => {}
            Seeker::Laser => {
                w.target =
                    lock.filter(|(_, t)| l.lock_optical && t.is_intact()).map(|(i, _)| Track::Ground(i));
            }
            seeker => {
                let nose = l.attitude.rotate(DVec3::new(1.0, 0.0, 0.0));
                w.boresight = nose;
                let optical_ok = seeker != Seeker::Optical || l.lock_optical;
                w.target = lock.filter(|_| optical_ok).and_then(|(i, t)| {
                    seeker_sees(kind, l.position, nose, LAUNCH_CIRCLE_RAD, t, l.terrain, true)
                        .map(|_| Track::Ground(i))
                });
                w.searching = w.target.is_none();
            }
        }
        w.tracking = w.target.is_some();
        w
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

    /// Verfolgtes Bodenziel (Index in `Sim::targets`).
    pub fn ground_target(&self) -> Option<usize> {
        match self.target? {
            Track::Ground(i) => Some(i),
            _ => None,
        }
    }

    /// Punkt, den eine Waffe gegen Bodenziele gerade anfliegt: die Zielmitte,
    /// solange der Suchkopf sie hat.
    pub fn aim_point(&self, targets: &[Target]) -> Option<DVec3> {
        let t = targets.get(self.ground_target()?).filter(|_| self.tracking)?;
        let c = t.center_ned();
        Some(DVec3::new(c[0], c[1], c[2]))
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
    /// Laserbombe: Das Flugzeug beleuchtet das Ziel nicht mehr (keine
    /// optische Aufschaltung mehr) – sie fällt ungelenkt weiter.
    LaserLost {
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
    /// Eine Radarstellung (Index in `Sim::targets`) hat eine Rakete gestartet.
    SamLaunch {
        kind: WeaponKind,
        id: u32,
        site: usize,
    },
    /// Eine Rakete hat sich von einem Täuschkörper ablenken lassen.
    Decoyed {
        kind: WeaponKind,
        id: u32,
    },
    /// Täuschkörper dieser Art sind aufgebraucht.
    DecoyEmpty(DecoyKind),
    /// Eine feindliche Rakete hat das Flugzeug getroffen und diese Systeme
    /// beschädigt.
    AircraftHit {
        kind: WeaponKind,
        id: u32,
        systems: [System; HIT_SYSTEMS],
    },
}

/// Bestand und Zustand der Waffen.
#[derive(Debug, Clone, PartialEq)]
pub struct Stores {
    pub bays: [Bay; BAYS],
    /// Gewählter Schacht (0-basiert).
    pub selected: usize,
    /// Die Bordkanone ist gewählt (vor Schacht 1 in der Reihe GUN, BAY 1–4;
    /// Startwahl, Spielerwunsch v0.2.8).
    pub gun_selected: bool,
    /// Klappen 0 (zu) .. 1 (offen).
    pub door: f64,
    /// Klappen sollen offen sein.
    pub door_cmd: bool,
    pub in_flight: Vec<Weapon>,
    /// Vorrat an Düppeln und Fackeln.
    pub chaff: u8,
    pub flares: u8,
    /// Täuschkörper in der Luft.
    pub decoys: Vec<Decoy>,
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
    /// Position und Geschwindigkeit des eigenen Flugzeugs (NED).
    pub aircraft: DVec3,
    pub aircraft_velocity: DVec3,
    /// Optisch aufgeschaltetes Ziel des Flugzeugs (Laserbeleuchtung).
    pub designated: Option<usize>,
    /// Das Flugzeug fliegt (Ziel der feindlichen Raketen).
    pub flying: bool,
    /// Radarstellungen (Index in `Sim::targets`), die das Flugzeug beim letzten
    /// Umlauf erfasst haben: Sie beleuchten es für ihre Raketen.
    pub painting: &'a [usize],
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
    /// Die Aufschaltung des Flugzeugs ist optisch (optische Suchköpfe
    /// übernehmen nur optische Aufschaltungen).
    pub lock_optical: bool,
    /// Waffenschacht funktioniert (Schadensmodell).
    pub bay_ok: bool,
    /// Zeitschritt der Simulation (s), für die Vorausberechnung.
    pub dt: f64,
}

/// Winkel zwischen zwei Richtungen (rad).
fn angle_between(a: DVec3, b: DVec3) -> f64 {
    let (la, lb) = (a.length(), b.length());
    if la <= 0.0 || lb <= 0.0 {
        return std::f64::consts::PI;
    }
    detmath::acos((a.dot(b) / (la * lb)).clamp(-1.0, 1.0))
}

/// Sieht der Suchkopf in `pos` mit Blickrichtung `dir` das Ziel innerhalb des
/// Kegels `cone`? Prüft Klasse, Kegel, Reichweite und – optisch, wenn `los` –
/// die Sichtlinie. Liefert die Entfernung.
fn seeker_sees(
    kind: WeaponKind,
    pos: DVec3,
    dir: DVec3,
    cone: f64,
    t: &Target,
    terrain: &Terrain,
    los: bool,
) -> Option<f64> {
    let s = kind.spec();
    if !t.is_intact() || !s.seeker.sees(t.class) {
        return None;
    }
    let c = t.center_ned();
    let to = DVec3::new(c[0], c[1], c[2]) - pos;
    let range = to.length();
    if angle_between(dir, to) > cone || range > s.max_range_m {
        return None;
    }
    if los
        && s.seeker == Seeker::Optical
        && !line_of_sight(|n, e| terrain.elevation(n, e), [pos.x, pos.y, -pos.z], [c[0], c[1], -c[2]])
    {
        return None;
    }
    Some(range)
}

impl Stores {
    pub fn new(loadout: &Loadout, seed: u64) -> Self {
        let bays = loadout.0.map(|kind| Bay { kind, count: kind.map_or(0, |k| k.spec().per_bay) });
        let selected = bays.iter().position(|b| b.count > 0).unwrap_or(0);
        Self {
            bays,
            selected,
            gun_selected: true,
            door: 0.0,
            door_cmd: false,
            in_flight: Vec::new(),
            chaff: DECOY_STOCK,
            flares: DECOY_STOCK,
            decoys: Vec::new(),
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
        if self.gun_selected {
            return None;
        }
        let b = self.bays[self.selected];
        b.kind.filter(|_| b.count > 0)
    }

    /// Nächste Waffe in der Reihe GUN → BAY 1 → … → BAY 4 → GUN.
    pub fn next_bay(&mut self) {
        if self.gun_selected {
            self.gun_selected = false;
            self.selected = 0;
        } else if self.selected + 1 >= BAYS {
            self.gun_selected = true;
        } else {
            self.selected += 1;
        }
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

    /// Löst die Waffe des gewählten Schachts aus, wenn alle Bedingungen
    /// erfüllt sind (Übergabe siehe [`Weapon::launched`]). Liefert die
    /// abgeworfene Masse (kg).
    pub fn release(&mut self, l: &Launch<'_>) -> f64 {
        let bay = self.bays[self.selected];
        let (_, _, roll) = l.attitude.to_euler();
        let event = match bay.kind {
            _ if !l.bay_ok => Some(WeaponEvent::BayFault),
            _ if !self.door_open() => Some(WeaponEvent::BayClosed),
            Some(_) if bay.count == 0 => Some(WeaponEvent::BayEmpty),
            None => Some(WeaponEvent::BayEmpty),
            _ if roll.abs() > MAX_RELEASE_BANK_RAD => Some(WeaponEvent::BankLimit),
            _ if self.in_flight.iter().filter(|w| w.owner.is_none()).count() >= MAX_IN_FLIGHT => {
                Some(WeaponEvent::Busy)
            }
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
        let w = Weapon::launched(id, kind, l);
        self.log(l.tick, WeaponEvent::Released { kind, bay: self.selected, id });
        if let Some(target) = w.ground_target() {
            self.log(l.tick, WeaponEvent::Acquired { kind, id, target });
        }
        self.in_flight.push(w);
        kind.spec().mass_kg
    }

    /// Startet eine Boden-Luft-Rakete der Radarstellung `site` (Index in
    /// `Sim::targets`, Startpunkt `from`) auf das Flugzeug bei `aircraft` –
    /// wenn es in Reichweite ist, die Stellung keine Rakete mehr in der Luft
    /// hat und nicht schon [`MAX_HOSTILE`] fliegen (wie im Original).
    pub fn launch_sam(
        &mut self,
        tick: u64,
        kind: WeaponKind,
        site: usize,
        from: DVec3,
        aircraft: DVec3,
    ) -> bool {
        let to = aircraft - from;
        let hostile = self.in_flight.iter().filter(|w| w.owner.is_some());
        if to.length() > kind.spec().max_range_m
            || hostile.clone().any(|w| w.owner == Some(site))
            || hostile.count() >= MAX_HOSTILE
        {
            return false;
        }
        // Im Startwinkel des Typs, mindestens knapp über der Linie zum
        // Flugzeug abheben.
        let flat = DVec3::new(to.x, to.y, 0.0).normalize_or_zero();
        let direct = detmath::atan2(-to.z, to.x.hypot(to.y)) + SAM_DIRECT_RAD;
        let elev = direct.max(kind.spec().launch_rad).min(std::f64::consts::FRAC_PI_2);
        let dir = flat * elev.cos() + DVec3::new(0.0, 0.0, -elev.sin());
        let id = self.next_id;
        self.next_id += 1;
        let mut w = Weapon::new(id, kind, from, dir * SAM_LAUNCH_MPS);
        w.owner = Some(site);
        w.target = Some(Track::Aircraft);
        self.in_flight.push(w);
        self.log(tick, WeaponEvent::SamLaunch { kind, id, site });
        true
    }

    /// Wirft einen Täuschkörper bei `position` ab (höchstens
    /// [`MAX_DECOYS`] gleichzeitig; leerer Vorrat meldet sich).
    pub fn drop_decoy(&mut self, tick: u64, kind: DecoyKind, position: DVec3) {
        let stock = match kind {
            DecoyKind::Chaff => &mut self.chaff,
            DecoyKind::Flare => &mut self.flares,
        };
        if *stock == 0 {
            self.log(tick, WeaponEvent::DecoyEmpty(kind));
            return;
        }
        if self.decoys.len() >= MAX_DECOYS {
            return;
        }
        *stock -= 1;
        let id = self.next_id;
        self.next_id += 1;
        self.decoys.push(Decoy { id, kind, position, age_s: 0.0 });
    }

    /// Alle Waffen im Flug einen Zeitschritt weiterbewegen; Zünder und
    /// Einschläge auswerten.
    pub fn step(&mut self, world: &mut World<'_>) {
        for d in &mut self.decoys {
            d.age_s += world.dt;
        }
        self.decoys.retain(|d| d.age_s < DECOY_LIFE_S);
        let decoys = self.decoys.clone();
        let mut i = 0;
        while i < self.in_flight.len() {
            let mut w = self.in_flight[i];
            let view = View {
                targets: world.targets,
                terrain: world.terrain,
                designated: world.designated,
                aircraft: world.flying.then_some((world.aircraft, world.aircraft_velocity)),
                decoys: &decoys,
                painted: w.owner.is_some_and(|o| world.painting.contains(&o)),
                los: true,
            };
            if w.kind.is_guided() && (world.tick + u64::from(w.id)).is_multiple_of(SEEKER_TICKS) {
                let had = w.tracking;
                if let Some(e) = seek(&mut w, &view) {
                    self.log(world.tick, e);
                }
                if had && !w.tracking && w.kind.spec().seeker == Seeker::Laser {
                    self.log(world.tick, WeaponEvent::LaserLost { kind: w.kind, id: w.id });
                }
            }
            let aim = w.target.filter(|_| w.tracking).and_then(|t| view.locate(t));
            match advance(&mut w, aim, world.terrain, world.dt) {
                Fate::Flying => {
                    self.in_flight[i] = w;
                    i += 1;
                }
                Fate::Fuze(_) if w.target == Some(Track::Aircraft) => {
                    self.in_flight.remove(i);
                    let systems = self.pick_systems();
                    self.log(world.tick, WeaponEvent::AircraftHit { kind: w.kind, id: w.id, systems });
                }
                // Am Täuschkörper oder (feindlich) im Gelände: wirkungslos.
                Fate::Fuze(p) | Fate::Ground(p)
                    if w.owner.is_some() || matches!(w.target, Some(Track::Decoy(_))) =>
                {
                    self.in_flight.remove(i);
                    let result = ImpactResult::Ground;
                    self.log(world.tick, WeaponEvent::Impact { kind: w.kind, id: w.id, position: p, result });
                }
                Fate::Fuze(p) | Fate::Ground(p) => {
                    self.in_flight.remove(i);
                    self.impact(world, &w, p);
                }
                Fate::Expired => {
                    self.in_flight.remove(i);
                    self.log(world.tick, WeaponEvent::Expired { kind: w.kind, id: w.id });
                }
            }
        }
    }

    /// Zufällige, verschiedene Bordsysteme für einen Raketentreffer.
    fn pick_systems(&mut self) -> [System; HIT_SYSTEMS] {
        let mut pool = System::ALL.to_vec();
        std::array::from_fn(|_| pool.remove(self.rng.next_u32() as usize % pool.len()))
    }

    /// Schaden ohne kritischen Treffer (Bordkanone).
    pub(crate) fn apply_damage(target: &mut Target, i: usize, amount: f64) -> ImpactResult {
        Self::apply(target, i, amount, false)
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

    /// Detonation bei `p`: wirkt auf alle intakten Ziele im Wirkradius.
    fn impact(&mut self, world: &mut World<'_>, w: &Weapon, p: DVec3) {
        let v = w.velocity;
        let impact_rad = detmath::atan2(v.z, (v.x * v.x + v.y * v.y).sqrt());
        let result = self.blast(world, w, p, impact_rad);
        self.log(world.tick, WeaponEvent::Impact { kind: w.kind, id: w.id, position: p, result });
        if (world.aircraft - p).length() < w.kind.spec().frag_radius_m {
            self.log(world.tick, WeaponEvent::FragWarning);
        }
    }

    /// Wirkung im Radius, nach Abstand vom Zielrand sortiert. Raketen würfeln
    /// wie im Original: Die Wirksamkeit ist die Chance, das Ziel zu zerstören,
    /// sonst wirkungslos. Bomben: Schaden fällt mit dem Abstand; ein voller
    /// Treffer würfelt einen kritischen.
    fn blast(&mut self, world: &mut World<'_>, w: &Weapon, p: DVec3, impact_rad: f64) -> ImpactResult {
        let reach = w.kind.spec().lethal_radius_m;
        let mut near: Vec<(f64, usize)> = world
            .targets
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_intact())
            .filter_map(|(i, t)| {
                let c = t.center_ned();
                let d = DVec3::new(c[0], c[1], c[2]) - p;
                let miss = ((d.x * d.x + d.y * d.y).sqrt() - t.class.radius())
                    .max(0.0)
                    .max(d.z.abs() - t.class.height());
                (miss <= reach).then_some((miss, i))
            })
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let missile = w.kind.spec().cruise_mps > 0.0;
        let mut result = ImpactResult::Ground;
        for (miss, i) in near {
            let e = w.kind.effectiveness(world.targets[i].class, impact_rad);
            let x = (miss / reach - FULL_EFFECT_FRACTION) / (1.0 - FULL_EFFECT_FRACTION);
            let falloff = (1.0 - x).clamp(0.0, 1.0);
            let this = if e <= 0.0 {
                ImpactResult::Ineffective(i)
            } else if missile {
                if self.rng.next_f64() < e {
                    Self::apply(&mut world.targets[i], i, 1.0, false)
                } else {
                    ImpactResult::Ineffective(i)
                }
            } else if e * falloff < MIN_DAMAGE {
                ImpactResult::Ground
            } else {
                let critical = falloff >= 1.0 && self.rng.next_f64() < CRITICAL_CHANCE;
                Self::apply(&mut world.targets[i], i, e * falloff, critical)
            };
            if this.rank() > result.rank() {
                result = this;
            }
        }
        result
    }
}

/// Wie ein Flugschritt einer Waffe endet.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Fate {
    Flying,
    /// Annäherungszünder am verfolgten Ziel (Detonationspunkt).
    Fuze(DVec3),
    /// Aufschlag auf Gelände oder Wasser.
    Ground(DVec3),
    /// Flugzeit überschritten.
    Expired,
}

/// Was ein Suchkopf von der Welt sieht.
struct View<'a> {
    targets: &'a [Target],
    terrain: &'a Terrain,
    /// Optisch aufgeschaltetes Ziel des Flugzeugs (Laserbeleuchtung).
    designated: Option<usize>,
    /// Ort und Geschwindigkeit des eigenen Flugzeugs, solange es fliegt.
    aircraft: Option<(DVec3, DVec3)>,
    decoys: &'a [Decoy],
    /// Die Stellung dieser (feindlichen) Rakete hat das Flugzeug beim letzten
    /// Umlauf erfasst (halbaktive Beleuchtung).
    painted: bool,
    /// Sichtlinie prüfen (die Vorausberechnung spart sie sich).
    los: bool,
}

impl View<'_> {
    /// Ort und Geschwindigkeit eines verfolgten Ziels (Bodenziele stehen);
    /// `None`, wenn es nicht mehr da ist.
    fn locate(&self, track: Track) -> Option<(DVec3, DVec3)> {
        match track {
            Track::Ground(i) => {
                let c = self.targets.get(i)?.center_ned();
                Some((DVec3::new(c[0], c[1], c[2]), DVec3::ZERO))
            }
            Track::Aircraft => self.aircraft,
            Track::Decoy(id) => self.decoys.iter().find(|d| d.id == id).map(|d| (d.position, DVec3::ZERO)),
        }
    }
}

/// Peilungsabweichung (waagerecht, rad) der Richtung `to` von der
/// Flugrichtung – wie im Original prüfen Raketen gegen Luftziele nur die
/// Peilung, so findet auch eine steil gestartete ihr Ziel.
fn bearing_off(velocity: DVec3, to: DVec3) -> f64 {
    let flat = |v: DVec3| DVec3::new(v.x, v.y, 0.0);
    if flat(velocity).length() < 1.0 { 0.0 } else { angle_between(flat(velocity), flat(to)) }
}

/// Suchkopf prüfen (gemeinsam für den echten Flug und die Vorausberechnung).
/// Ein Täuschkörper der passenden Art im Suchkegel lenkt ab (wie im
/// Original). Dann je Ziel:
/// - **Bodenziel:** Laserbombe – wird es noch beleuchtet? Rakete – liegt es
///   noch in Suchkegel und Reichweite (optisch und mit `los` auch in Sicht)?
///   Weit hinter ihr gibt sie auf.
/// - **Flugzeug / Täuschkörper:** Peilung im Suchkegel; halbaktiv weiter als
///   [`SEMI_ACTIVE_M`] nur mit Beleuchtung durch die Stellung, optisch
///   (Infrarot) nur in Reichweite und mit freier Sicht. Weit hinter ihr gibt
///   sie auf.
/// - **Keines:** sucht selbst das nächste passende Bodenziel vor dem Suchkopf
///   (auch zivile), wenn sie ohne Übergabe abgefeuert wurde.
///
/// Liefert ein neues Ziel oder die Ablenkung als Ereignis.
fn seek(w: &mut Weapon, v: &View<'_>) -> Option<WeaponEvent> {
    let s = w.kind.spec();
    // Erst fallen (eigene) bzw. im Boost steigen (feindliche).
    if w.age_s < s.boost_s.max(GUIDANCE_DELAY_S) {
        return None;
    }
    if w.target.is_some_and(|t| !matches!(t, Track::Decoy(_)))
        && let Some(d) = v.decoys.iter().find(|d| {
            s.seeker.lured_by(d.kind) && {
                let to = d.position - w.position;
                to.length() <= s.max_range_m && bearing_off(w.velocity, to) <= s.seeker_cone_rad
            }
        })
    {
        w.target = Some(Track::Decoy(d.id));
        w.tracking = true;
        return Some(WeaponEvent::Decoyed { kind: w.kind, id: w.id });
    }
    match w.target {
        Some(Track::Ground(i)) => {
            let t = &v.targets[i];
            if s.seeker == Seeker::Laser {
                w.tracking = v.designated == Some(i) && t.is_intact();
                return None;
            }
            let c = t.center_ned();
            if angle_between(w.velocity, DVec3::new(c[0], c[1], c[2]) - w.position) > GIVE_UP_RAD {
                w.target = None;
                w.tracking = false;
                return None;
            }
            w.tracking =
                seeker_sees(w.kind, w.position, w.velocity, s.seeker_cone_rad, t, v.terrain, v.los).is_some();
            None
        }
        Some(track) => {
            let Some((p, _)) = v.locate(track) else {
                w.target = None;
                w.tracking = false;
                return None;
            };
            let to = p - w.position;
            let off = bearing_off(w.velocity, to);
            if off > GIVE_UP_RAD {
                w.target = None;
                w.tracking = false;
                return None;
            }
            let decoy = matches!(track, Track::Decoy(_));
            let sees = match s.seeker {
                Seeker::SemiActive => decoy || v.painted || to.length() <= SEMI_ACTIVE_M,
                _ => {
                    to.length() <= s.max_range_m
                        && (!v.los
                            || line_of_sight(
                                |n, e| v.terrain.elevation(n, e),
                                [w.position.x, w.position.y, -w.position.z],
                                [p.x, p.y, -p.z],
                            ))
                }
            };
            w.tracking = off <= s.seeker_cone_rad && sees;
            None
        }
        None if w.searching => {
            let (_, i) = v
                .targets
                .iter()
                .enumerate()
                .filter_map(|(i, t)| {
                    seeker_sees(w.kind, w.position, w.velocity, s.seeker_cone_rad, t, v.terrain, v.los)
                        .map(|r| (r, i))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))?;
            w.target = Some(Track::Ground(i));
            w.tracking = true;
            w.searching = false;
            Some(WeaponEvent::Acquired { kind: w.kind, id: w.id, target: i })
        }
        None => None,
    }
}

/// Ein Zeitschritt einer Waffe (gemeinsam für den echten Flug und die
/// Vorausberechnung): bewegen, dann Annäherungszünder (Raketen: kürzester
/// Abstand der Strecke zum Zielpunkt `aim` unter dem Wirkradius),
/// Bodenaufschlag und Flugzeit prüfen. `aim` ist der Punkt, den der Suchkopf
/// gerade hat (Zielmitte bzw. Flugzeug) mit seiner Geschwindigkeit, sonst
/// `None`; gelenkt wird auf den Treffpunkt mit Vorhalt.
fn advance(w: &mut Weapon, aim: Option<(DVec3, DVec3)>, terrain: &Terrain, dt: f64) -> Fate {
    let before = w.position;
    let s = w.kind.spec();
    let steer = aim.map(|(p, v)| intercept(w, p, v));
    match s.seeker {
        Seeker::None => fly(w, dt),
        Seeker::Laser => steer_bomb(w, steer, dt),
        _ => guide(w, steer, dt),
    }
    if s.cruise_mps > 0.0
        && let Some((aim, _)) = aim
    {
        let d = w.position - before;
        let k = if d.length() > 1e-9 { ((aim - before).dot(d) / d.dot(d)).clamp(0.0, 1.0) } else { 0.0 };
        let closest = before + d * k;
        if (aim - closest).length() < s.lethal_radius_m {
            return Fate::Fuze(closest);
        }
    }
    if let Some(p) = ground_crossing(terrain, before, w.position) {
        return Fate::Ground(p);
    }
    if w.age_s > s.life_s {
        return Fate::Expired;
    }
    Fate::Flying
}

/// Schneidet die Strecke `a`→`b` das Gelände (bzw. die Wasseroberfläche)?
/// Liefert den Schnittpunkt auf der Oberfläche.
pub(crate) fn ground_crossing(terrain: &Terrain, a: DVec3, b: DVec3) -> Option<DVec3> {
    let ground = terrain.elevation(b.x, b.y);
    if -b.z > ground {
        return None;
    }
    let h0 = -a.z - terrain.elevation(a.x, a.y);
    let h1 = -b.z - ground;
    let f = if h0 > h1 { (h0 / (h0 - h1)).clamp(0.0, 1.0) } else { 1.0 };
    let mut p = a + (b - a) * f;
    p.z = -terrain.elevation(p.x, p.y);
    Some(p)
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
    // Grobe Schritte reichen für die Anzeige: 8 Simulationsschritte auf einmal,
    // mit gemittelter Geschwindigkeit (exakt für die Schwerkraft).
    let dt = dt * 8.0;
    // Oberhalb der höchsten möglichen Geländehöhe ist keine Abfrage nötig;
    // hoch über dem zuletzt gemessenen Boden reicht jeder vierte Schritt (in
    // vier groben Schritten kommt die Bombe kaum 200 m weit).
    let ceiling = terrain.max_elevation();
    let mut last_ground = f64::MIN;
    let mut since_check = 0;
    let mut density = 0.0;
    let mut steps = 0_u32;
    while w.age_s < MAX_FALL_S {
        let before = w;
        // Luftdichte ändert sich über wenige hundert Meter kaum: nur jeden
        // vierten Schritt neu berechnen.
        if steps.is_multiple_of(4) {
            density = atmosphere::isa(-w.position.z).density;
        }
        steps += 1;
        let v0 = w.velocity;
        fly_with_density(&mut w, dt, density);
        w.position = before.position + (v0 + w.velocity) * (0.5 * dt);
        let alt = -w.position.z;
        since_check += 1;
        if alt > ceiling || (alt - last_ground > PREDICT_MARGIN_M && since_check < 4) {
            continue;
        }
        since_check = 0;
        last_ground = terrain.elevation(w.position.x, w.position.y);
        if let Some(p) = ground_crossing(terrain, before.position, w.position) {
            let f = (p - before.position).length() / (w.position - before.position).length().max(1e-9);
            return Some((p, before.age_s + (w.age_s - before.age_s) * f));
        }
    }
    None
}

/// Ein Zeitschritt Bombenflugbahn: Schwerkraft und Widerstand (semi-implizit,
/// stabil bei 120 Hz).
fn fly(w: &mut Weapon, dt: f64) {
    let density = atmosphere::isa(-w.position.z).density;
    fly_with_density(w, dt, density);
}

/// Wie [`fly`] mit vorgegebener Luftdichte.
fn fly_with_density(w: &mut Weapon, dt: f64, density: f64) {
    let s = w.kind.spec();
    let speed = w.velocity.length();
    let drag_area = if w.chute_open() { s.deployed_drag_area_m2 } else { s.drag_area_m2 };
    let mut accel = DVec3::new(0.0, 0.0, G0);
    if speed > 0.0 {
        accel -= w.velocity / speed * (0.5 * density * speed * speed * drag_area / s.mass_kg);
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

/// Laserbombe: fällt physikalisch (Schwerkraft, Widerstand) und dreht mit
/// ihren Steuerflächen die Flugbahn mit begrenzter Rate zum Ziel, solange es
/// beleuchtet ist; sonst fällt sie wie eine normale Bombe.
fn steer_bomb(w: &mut Weapon, aim: Option<DVec3>, dt: f64) {
    fly(w, dt);
    if w.age_s < GUIDANCE_DELAY_S {
        return;
    }
    if let Some(aim) = aim {
        let speed = w.velocity.length();
        let dir = w.velocity / speed.max(1.0);
        let wanted = (aim - w.position).normalize_or_zero();
        w.velocity = turn_towards(dir, wanted, w.kind.spec().turn_rate_rad * dt) * speed;
    }
}

/// Treffpunkt mit Vorhalt: wo das Ziel (bei `aim`, Geschwindigkeit `vel`)
/// nach der Restflugzeit sein wird, gewichtet mit dem Vorhaltfaktor der
/// Waffe. Für stehende Ziele die Zielmitte (reine Verfolgung wie im Original).
fn intercept(w: &Weapon, aim: DVec3, vel: DVec3) -> DVec3 {
    let s = w.kind.spec();
    let to = aim - w.position;
    let range = to.length();
    // Bogen: um einen Anteil der Entfernung über das Ziel zielen.
    let aim = aim + DVec3::new(0.0, 0.0, -s.loft * range);
    if vel == DVec3::ZERO {
        return aim;
    }
    let closing = (w.velocity - vel).dot(to / range.max(1.0)).max(1.0);
    aim + vel * (s.lead * (range / closing).min(MAX_LEAD_S))
}

/// Ein Zeitschritt Rakete: erst frei aus dem Schacht fallen, dann mit
/// begrenzter Drehrate auf das Ziel drehen (reine Verfolgung wie im Original)
/// und auf Marschfahrt gehen. Ohne Ziel fliegt sie geradeaus in ihrer
/// Abschusslage (`boresight`).
fn guide(w: &mut Weapon, aim: Option<DVec3>, dt: f64) {
    // Eigene fallen aus dem Schacht; Boden-Luft-Raketen starten mit Schub
    // (im Boost ohne Ziel geradeaus).
    if w.owner.is_none() && w.age_s < GUIDANCE_DELAY_S {
        fly(w, dt);
        return;
    }
    let s = w.kind.spec();
    let speed = w.velocity.length().max(1.0);
    let dir = w.velocity / speed;
    let wanted = match aim {
        Some(aim) => (aim - w.position).normalize_or_zero(),
        None if w.target.is_some() || w.owner.is_some() => dir,
        None => w.boresight,
    };
    let new_dir = turn_towards(dir, wanted, s.turn_rate_rad * dt);
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
        assert_eq!(l.to_text(), "fall,chute,lgb,agm");
        assert_eq!(Loadout::parse("fall,chute,glide,agm"), Some(l), "alte Kennung");
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

    /// Auslösen auf ein Ziel 5 km voraus, 3 km darunter, Nase genau aufs Ziel.
    fn launch_at_target(lock_optical: bool) -> (Weapon, Vec<Target>, Terrain) {
        let terrain = Terrain::new(1);
        let elevation = terrain.elevation(5_000.0, 0.0);
        let mut t = bridge();
        t.class = TargetClass::Soft;
        t.north = 5_000.0;
        t.elevation = elevation;
        let targets = vec![t];
        let alt = elevation + 3_000.0 + TargetClass::Soft.height() * 0.5;
        let pitch = -detmath::atan2(3_000.0, 5_000.0);
        let attitude = DQuat::from_euler(0.0, pitch, 0.0);
        let launch = Launch {
            tick: 1,
            position: DVec3::new(0.0, 0.0, -alt),
            velocity: attitude.rotate(DVec3::new(200.0, 0.0, 0.0)),
            attitude,
            lock: Some(0),
            lock_optical,
            targets: &targets,
            terrain: &terrain,
            bay_ok: true,
            dt: 1.0 / 120.0,
        };
        let w = Weapon::launched(1, WeaponKind::AirToGround, &launch);
        (w, targets, terrain)
    }

    /// Sicht auf Bodenziele für die Tests (ohne Flugzeug, ohne Täuschkörper).
    fn ground_view<'a>(targets: &'a [Target], terrain: &'a Terrain) -> View<'a> {
        View { targets, terrain, designated: None, aircraft: None, decoys: &[], painted: false, los: true }
    }

    #[test]
    fn optical_seeker_takes_only_an_optical_lock() {
        let (w, _, _) = launch_at_target(true);
        assert_eq!(w.target, Some(Track::Ground(0)));
        assert!(!w.searching && w.tracking);
        let (w, _, _) = launch_at_target(false);
        assert_eq!(w.target, None, "reine Radaraufschaltung wird nicht übernommen");
        assert!(w.searching);
    }

    /// Mit Übergabe fliegt die Rakete das Ziel an und zündet daran.
    #[test]
    fn missile_pursues_and_fuzes_at_the_target() {
        let (mut w, targets, terrain) = launch_at_target(true);
        let mut fate = Fate::Flying;
        for k in 0..(120 * 30) {
            if k % 12 == 0 {
                seek(&mut w, &ground_view(&targets, &terrain));
            }
            let aim = w.aim_point(&targets).map(|p| (p, DVec3::ZERO));
            fate = advance(&mut w, aim, &terrain, 1.0 / 120.0);
            if fate != Fate::Flying {
                break;
            }
        }
        let Fate::Fuze(p) = fate else { panic!("kein Zünder: {fate:?}") };
        let c = targets[0].center_ned();
        assert!((p - DVec3::new(c[0], c[1], c[2])).length() < WeaponKind::AirToGround.spec().lethal_radius_m);
    }

    #[test]
    fn unlocked_missile_flies_on_in_launch_attitude() {
        let (mut w, _, terrain) = launch_at_target(false);
        // Ohne Ziele in der Welt: nichts zu finden, nur geradeaus.
        let none: Vec<Target> = Vec::new();
        let nose = w.boresight;
        // Lange genug, um auf Marschfahrt zu kommen (gemächliche Beschleunigung).
        for _ in 0..(120 * 12) {
            seek(&mut w, &ground_view(&none, &terrain));
            guide(&mut w, None, 1.0 / 120.0);
        }
        let dir = w.velocity.normalize_or_zero();
        assert!(angle_between(dir, nose) < 0.01, "weicht {:.3} rad ab", angle_between(dir, nose));
        assert!((w.velocity.length() - w.kind.spec().cruise_mps).abs() < 1.0);
    }

    /// Feindliche Rakete auf das Flugzeug bei `at`, nach dem Start.
    fn sam(kind: WeaponKind) -> Weapon {
        let mut w = Weapon::new(0, kind, DVec3::ZERO, DVec3::new(700.0, 0.0, -1.0));
        w.owner = Some(0);
        w.target = Some(Track::Aircraft);
        w.age_s = 10.0;
        w
    }

    fn air_view<'a>(terrain: &'a Terrain, at: DVec3, painted: bool, decoys: &'a [Decoy]) -> View<'a> {
        View {
            targets: &[],
            terrain,
            designated: None,
            aircraft: Some((at, DVec3::ZERO)),
            decoys,
            painted,
            los: true,
        }
    }

    /// Halbaktiv: Weit weg lenkt die Boden-Luft-Rakete nur, solange ihre
    /// Stellung das Flugzeug erfasst; nah mit eigenem Suchkopf. Außerhalb des
    /// Suchkegels lenkt sie nicht, weit hinter ihr gibt sie auf.
    #[test]
    fn sam_guidance_is_semi_active() {
        let terrain = Terrain::new(1);
        let mut w = sam(WeaponKind::MediumRangeSam);
        let far = DVec3::new(10_000.0, 0.0, 0.0);
        seek(&mut w, &air_view(&terrain, far, true, &[]));
        assert!(w.tracking);
        seek(&mut w, &air_view(&terrain, far, false, &[]));
        assert!(!w.tracking && w.target.is_some(), "ohne Beleuchtung blind, aber noch dran");
        seek(&mut w, &air_view(&terrain, DVec3::new(3_000.0, 0.0, 0.0), false, &[]));
        assert!(w.tracking, "nah: eigener Suchkopf");
        seek(&mut w, &air_view(&terrain, DVec3::new(3_000.0, 3_000.0, 0.0), true, &[]));
        assert!(!w.tracking && w.target.is_some(), "45° neben der Flugrichtung: außerhalb des Kegels");
        seek(&mut w, &air_view(&terrain, DVec3::new(-3_000.0, 0.0, 0.0), true, &[]));
        assert!(w.target.is_none(), "hinter ihr: aufgegeben");
        seek(&mut w, &air_view(&terrain, far, true, &[]));
        assert!(!w.tracking, "aufgegeben bleibt aufgegeben");
    }

    /// Täuschkörper wie im Original: Ein Düppel im Suchkegel lenkt die
    /// Radarrakete ab, nicht die Infrarotrakete – bei der Fackel umgekehrt.
    #[test]
    fn decoys_lure_only_their_kind_of_seeker() {
        let terrain = Terrain::new(1);
        // Hoch über allem Gelände: freie Sicht
        let at = DVec3::new(8_000.0, 0.0, -9_000.0);
        let place = DVec3::new(4_000.0, 200.0, -9_000.0);
        let chaff = [Decoy { id: 7, kind: DecoyKind::Chaff, position: place, age_s: 0.0 }];
        let flare = [Decoy { id: 8, kind: DecoyKind::Flare, position: place, age_s: 0.0 }];
        let start = |kind| {
            let mut w = sam(kind);
            w.position = DVec3::new(0.0, 0.0, -9_000.0);
            w
        };
        let mut radar = start(WeaponKind::MediumRangeSam);
        assert!(seek(&mut radar, &air_view(&terrain, at, true, &flare)).is_none());
        assert_eq!(radar.target, Some(Track::Aircraft));
        assert!(matches!(
            seek(&mut radar, &air_view(&terrain, at, true, &chaff)),
            Some(WeaponEvent::Decoyed { .. })
        ));
        assert_eq!(radar.target, Some(Track::Decoy(7)));
        let mut ir = start(WeaponKind::ShortRangeSam);
        seek(&mut ir, &air_view(&terrain, DVec3::new(5_000.0, 0.0, -9_000.0), false, &chaff));
        assert_eq!(ir.target, Some(Track::Aircraft));
        assert!(ir.tracking, "Infrarot braucht keine Beleuchtung");
        seek(&mut ir, &air_view(&terrain, at, false, &flare));
        assert_eq!(ir.target, Some(Track::Decoy(8)));
        // Ausgebrannt: Die Rakete hat kein Ziel mehr.
        seek(&mut ir, &air_view(&terrain, at, false, &[]));
        assert!(ir.target.is_none() && !ir.tracking);
    }

    /// Vorhalt: stehendes Ziel → Zielmitte; bewegtes Ziel → Treffpunkt
    /// voraus, voll bei Faktor 1.
    #[test]
    fn intercept_leads_moving_targets_only() {
        let w = Weapon::new(0, WeaponKind::ShortRangeSam, DVec3::ZERO, DVec3::new(700.0, 0.0, 0.0));
        let aim = DVec3::new(7_000.0, 0.0, 0.0);
        assert_eq!(intercept(&w, aim, DVec3::ZERO), aim);
        // Ziel quer mit 200 m/s, Annäherung 700 m/s ⇒ 10 s ⇒ 2 km voraus
        let p = intercept(&w, aim, DVec3::new(0.0, 200.0, 0.0));
        assert!((p - DVec3::new(7_000.0, 2_000.0, 0.0)).length() < 1e-6, "{p:?}");
        for k in WeaponKind::ALL {
            assert_eq!(k.spec().lead, 1.0, "{k:?}: volle Gefährlichkeit");
        }
        // Bogen: Die SA-5 zielt ein Viertel der Entfernung über das Ziel.
        let w = Weapon { kind: WeaponKind::LongRangeSam, ..w };
        assert_eq!(intercept(&w, aim, DVec3::ZERO), aim + DVec3::new(0.0, 0.0, -1_750.0));
    }

    /// Wie im Original: Ein Ziel weit hinter der Rakete gibt sie auf.
    #[test]
    fn missile_gives_up_a_target_behind_it() {
        let (mut w, mut targets, terrain) = launch_at_target(true);
        w.age_s = 1.0;
        targets[0].north = -20_000.0;
        seek(&mut w, &ground_view(&targets, &terrain));
        assert_eq!(w.target, None);
        assert!(!w.tracking);
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
        let start = |kind| Weapon::new(0, kind, DVec3::new(0.0, 0.0, -3_000.0), DVec3::new(250.0, 0.0, 0.0));
        let (mut a, mut b) = (start(WeaponKind::FreeFall), start(WeaponKind::Retarded));
        for _ in 0..(120 * 5) {
            fly(&mut a, 1.0 / 120.0);
            fly(&mut b, 1.0 / 120.0);
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
