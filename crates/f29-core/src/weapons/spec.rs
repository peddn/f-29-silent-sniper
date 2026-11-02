//! Waffenarten, Suchköpfe und ihre Kennwerte.

use super::*;

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
    /// Moderne Boden-Luft-Rakete mittlerer Reichweite (jede dritte
    /// SAM-Stellung): wendig und schwer zu täuschen.
    ModernSam,
    /// Luft-Luft-Rakete kurzer Reichweite mit Infrarotsuchkopf (spiel-
    /// mechanisch das optische System).
    Sidewinder,
    /// Luft-Luft-Rakete großer Reichweite mit eigenem Radar.
    Amraam,
    /// Feindliche Luft-Luft-Rakete kurzer Reichweite mit Infrarotsuchkopf
    /// (MiG-21).
    Aphid,
    /// Feindliche radargelenkte Luft-Luft-Rakete mittlerer Reichweite
    /// (MiG-29), weit weg halbaktiv.
    Alamo,
    /// Aufklärungskamera (M-032): kein Geschoss; der Auslöser macht ein Foto.
    Camera,
}

/// Suchkopf einer gelenkten Waffe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seeker {
    /// Ungelenkt.
    None,
    /// Optisch: braucht freie Sicht, alle Bodenziele.
    Optical,
    /// Laser: das Flugzeug muss das Ziel gewählt und optisch entdeckt halten.
    Laser,
    /// Passiv auf Radarstrahlung: nur Flugabwehrstellungen, ohne Sichtlinie.
    Radiation,
    /// Radar, ohne Sichtlinie: gegen Bodenziele nur Schiffe, gegen
    /// Flugzeuge mit eigenem Radar – weiter als
    /// [`WeaponSpec::illuminated_beyond_m`] nur mit Beleuchtung durch die
    /// Stellung (halbaktiv, Boden-Luft-Raketen).
    Radar,
}

impl Seeker {
    /// Lässt sich dieser Suchkopf von dieser Art Täuschkörper ablenken?
    pub fn lured_by(self, decoy: DecoyKind) -> bool {
        matches!((self, decoy), (Seeker::Radar, DecoyKind::Chaff) | (Seeker::Optical, DecoyKind::Flare))
    }

    /// Kann dieser Suchkopf ein Ziel der Klasse überhaupt erfassen?
    pub fn sees(self, class: TargetClass) -> bool {
        match self {
            Seeker::None => false,
            Seeker::Optical | Seeker::Laser => true,
            Seeker::Radiation => class.is_radar(),
            Seeker::Radar => class.is_naval(),
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
    /// Chance (0..1), einen passenden Täuschkörper im Suchkegel zu
    /// übergehen; je Täuschkörper einmal gewürfelt.
    pub decoy_resistance: f64,
    /// Luft-Luft-Rakete: verfolgt Jäger statt Bodenziele.
    pub anti_air: bool,
    /// Radarsuchkopf gegen Flugzeuge: weiter als so (m) lenkt er nur mit
    /// Beleuchtung durch die Stellung; 0 = immer mit eigenem Radar (Fire and
    /// Forget).
    pub illuminated_beyond_m: f64,
}

pub(super) const fn deg(d: f64) -> f64 {
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
    max_range_m: 0.0,
    seeker_cone_rad: 0.0,
    lethal_radius_m: 30.0,
    frag_radius_m: 180.0,
    life_s: MAX_FALL_S,
    decoy_resistance: 0.0,
    anti_air: false,
    illuminated_beyond_m: 0.0,
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
    max_range_m: 16_000.0,
    seeker_cone_rad: deg(25.0),
    lethal_radius_m: 8.0,
    frag_radius_m: 90.0,
    life_s: 70.0,
    decoy_resistance: 0.0,
    anti_air: false,
    illuminated_beyond_m: 0.0,
};

/// Boden-Luft-Rakete mittlerer Reichweite (Vorbild SA-6): mittelschnell,
/// mittlerer Boost und Bogen.
const SAM: WeaponSpec = WeaponSpec {
    kind: WeaponKind::MediumRangeSam,
    label: "SA-6",
    name: "SA-6 GAINFUL",
    mass_kg: 600.0,
    per_bay: 0,
    seeker: Seeker::Radar,
    // Halbaktiv wie im Original: weit weg nur, solange die Stellung das
    // Flugzeug erfasst.
    illuminated_beyond_m: 5_000.0,
    cruise_mps: 650.0,
    accel_mps2: 250.0,
    turn_rate_rad: deg(45.0),
    launch_rad: deg(45.0),
    boost_s: 1.5,
    loft: 0.1,
    max_range_m: 25_000.0,
    // Etwas breiter als im Original (22°): Querflug schüttelt sie meist,
    // aber nicht immer ab.
    seeker_cone_rad: deg(26.0),
    lethal_radius_m: 30.0,
    frag_radius_m: 0.0,
    life_s: 50.0,
    decoy_resistance: 0.15,
    ..MISSILE
};

const SPECS: [WeaponSpec; 15] = [
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
        seeker: Seeker::Radar,
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
        turn_rate_rad: deg(30.0),
        launch_rad: deg(85.0),
        boost_s: 4.0,
        loft: 0.25,
        max_range_m: 60_000.0,
        lethal_radius_m: 45.0,
        life_s: 150.0,
        decoy_resistance: 0.05,
        ..SAM
    },
    // Kurzstreckenrakete mit Infrarotsuchkopf (Vorbild SA-13): wendig,
    // startet ohne Boost direkt auf das Ziel. Sucht selbst (keine Beleuchtung
    // durch die Stellung), braucht aber freie Sicht; Fackeln lenken sie ab,
    // Querflug schüttelt sie ab.
    WeaponSpec {
        kind: WeaponKind::ShortRangeSam,
        label: "SA-13",
        name: "SA-13 GOPHER",
        seeker: Seeker::Optical,
        mass_kg: 130.0,
        cruise_mps: 650.0,
        accel_mps2: 400.0,
        turn_rate_rad: deg(60.0),
        launch_rad: 0.0,
        boost_s: 0.0,
        loft: 0.0,
        max_range_m: 10_000.0,
        seeker_cone_rad: deg(22.0),
        lethal_radius_m: 18.0,
        life_s: 20.0,
        decoy_resistance: 0.05,
        ..SAM
    },
    // Moderne Rakete mittlerer Reichweite (Vorbild SA-11): schneller und
    // wendiger als die SA-6, übergeht die Hälfte der Täuschkörper und lässt
    // sich durch Querflug selten abschütteln – nur viele Düppel helfen.
    WeaponSpec {
        kind: WeaponKind::ModernSam,
        label: "SA-11",
        name: "SA-11 GADFLY",
        mass_kg: 690.0,
        cruise_mps: 800.0,
        accel_mps2: 300.0,
        turn_rate_rad: deg(50.0),
        // Breiter Suchkegel: Querflug schüttelt sie selten ab.
        seeker_cone_rad: deg(35.0),
        boost_s: 1.0,
        loft: 0.05,
        life_s: 40.0,
        decoy_resistance: 0.5,
        ..SAM
    },
    // AIM-9 Sidewinder: schnell und wendig, kurze Reichweite; sieht nur, was
    // das optische System sieht (freie Sicht), Fackeln lenken sie ab.
    WeaponSpec {
        kind: WeaponKind::Sidewinder,
        label: "AIM-9",
        name: "AIM-9 SIDEWINDER",
        mass_kg: 86.0,
        per_bay: 4,
        seeker: Seeker::Optical,
        cruise_mps: 850.0,
        accel_mps2: 500.0,
        turn_rate_rad: deg(40.0),
        max_range_m: 8_000.0,
        seeker_cone_rad: deg(25.0),
        lethal_radius_m: 9.0,
        frag_radius_m: 0.0,
        life_s: 25.0,
        // Jäger werfen gegen jede Rakete eine Fackel: etwa jede zweite
        // bis dritte geht vorbei.
        decoy_resistance: 0.6,
        anti_air: true,
        ..MISSILE
    },
    // AIM-120 AMRAAM: Fire and Forget mit eigenem Radar (Spielerentscheidung
    // v0.2.9), große Reichweite, weniger wendig; Düppel lenken sie ab.
    WeaponSpec {
        kind: WeaponKind::Amraam,
        label: "AIM-120",
        name: "AIM-120 AMRAAM",
        mass_kg: 152.0,
        per_bay: 3,
        seeker: Seeker::Radar,
        cruise_mps: 1_000.0,
        accel_mps2: 300.0,
        turn_rate_rad: deg(30.0),
        max_range_m: 30_000.0,
        seeker_cone_rad: deg(30.0),
        lethal_radius_m: 12.0,
        frag_radius_m: 0.0,
        life_s: 60.0,
        decoy_resistance: 0.7,
        anti_air: true,
        ..MISSILE
    },
    // AA-8 Aphid der MiG-21: wendig, kurze Reichweite, Fackeln helfen.
    WeaponSpec {
        kind: WeaponKind::Aphid,
        label: "AA-8",
        name: "AA-8 APHID",
        mass_kg: 65.0,
        per_bay: 0,
        seeker: Seeker::Optical,
        cruise_mps: 750.0,
        accel_mps2: 400.0,
        turn_rate_rad: deg(40.0),
        max_range_m: 7_000.0,
        seeker_cone_rad: deg(22.0),
        lethal_radius_m: 12.0,
        frag_radius_m: 0.0,
        life_s: 20.0,
        decoy_resistance: 0.1,
        ..MISSILE
    },
    // AA-10 Alamo der MiG-29: halbaktiv wie die SAMs – weit weg nur, solange
    // ihr Jäger das Flugzeug mit seinem Radar hält; Düppel helfen.
    WeaponSpec {
        kind: WeaponKind::Alamo,
        label: "AA-10",
        name: "AA-10 ALAMO",
        mass_kg: 250.0,
        per_bay: 0,
        seeker: Seeker::Radar,
        illuminated_beyond_m: 5_000.0,
        cruise_mps: 850.0,
        accel_mps2: 300.0,
        turn_rate_rad: deg(30.0),
        max_range_m: 25_000.0,
        seeker_cone_rad: deg(25.0),
        lethal_radius_m: 15.0,
        frag_radius_m: 0.0,
        life_s: 45.0,
        decoy_resistance: 0.15,
        ..MISSILE
    },
    // Aufklärungskamera: bleibt im Schacht, macht unbegrenzt Fotos.
    WeaponSpec {
        kind: WeaponKind::Camera,
        label: "CAM",
        name: "KS-87 RECON CAMERA",
        mass_kg: 150.0,
        per_bay: 1,
        lethal_radius_m: 0.0,
        frag_radius_m: 0.0,
        ..BOMB
    },
];

impl WeaponKind {
    /// Die Waffen für die Schächte (ohne die feindlichen Raketen).
    pub const ALL: [WeaponKind; 9] = [
        WeaponKind::FreeFall,
        WeaponKind::Retarded,
        WeaponKind::GuidedBomb,
        WeaponKind::AirToGround,
        WeaponKind::AntiRadar,
        WeaponKind::AntiShip,
        WeaponKind::Sidewinder,
        WeaponKind::Amraam,
        WeaponKind::Camera,
    ];

    pub fn spec(self) -> &'static WeaponSpec {
        &SPECS[self as usize]
    }

    pub fn label(self) -> &'static str {
        self.spec().label
    }

    /// Würde diese Luft-Luft-Rakete einen Jäger in `range` übernehmen
    /// (optische Suchköpfe nur, wenn er `optical` entdeckt ist)?
    pub fn takes_aircraft(self, range: f64, optical: bool) -> bool {
        let s = self.spec();
        s.anti_air && range <= s.max_range_m && (s.seeker != Seeker::Optical || optical)
    }

    pub fn is_guided(self) -> bool {
        self.spec().seeker != Seeker::None
    }

    /// Ungelenkte Bombe (Aufschlagvorhersage, Zielhilfe).
    pub fn is_bomb(self) -> bool {
        !self.is_guided() && self != WeaponKind::Camera
    }

    /// Fernsehsuchkopf: Die Waffe sendet ihr Bild (WPN-CAM-Seite).
    pub fn has_camera(self) -> bool {
        self.spec().seeker == Seeker::Optical && !self.spec().anti_air
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
            WeaponKind::ModernSam => "sa11",
            WeaponKind::Sidewinder => "aim9",
            WeaponKind::Amraam => "aim120",
            WeaponKind::Aphid => "aa8",
            WeaponKind::Alamo => "aa10",
            WeaponKind::Camera => "cam",
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
            (
                W::MediumRangeSam
                | W::LongRangeSam
                | W::ShortRangeSam
                | W::Sidewinder
                | W::Amraam
                | W::Aphid
                | W::Alamo
                | W::Camera,
                _,
            ) => 0.0,
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
