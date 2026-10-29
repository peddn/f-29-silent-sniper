//! Waffen (M-014, Wirkung M-010 bis M-012).
//!
//! Vier interne Schächte mit je einer Waffenart. Der Schacht wird von Hand
//! geöffnet; ausgelöst wird nur bei offenem Schacht und mäßiger Querlage.
//! Jede Waffe ist im Flug ein Massenpunkt mit Schwerkraft, Luftwiderstand
//! und – bei Flugkörpern – Triebwerk; gelenkte Waffen lenken mit eigenem
//! Suchkopf (Fire and Forget, M-013). Beim Einschlag entscheidet die
//! Wirkungsmatrix Waffe × Zielklasse mit stetig fallender Wahrscheinlichkeit
//! über den Abstand, ob ein Ziel zerstört wird.
//!
//! Werte: eigene Größenordnungen aus öffentlichen Daten realer Waffen
//! (Masse, Widerstandsfläche, Triebwerk) und Spielbalance, siehe M-014.

use crate::atmosphere::{self, G0};
use crate::detmath;
use crate::math::{DQuat, DVec3};
use crate::rng::Pcg32;
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
/// Bis zu diesem Anteil des Wirkradius ist die Wirkung voll, darüber fällt sie
/// linear auf null (M-011).
const FULL_EFFECT_FRACTION: f64 = 0.4;
/// Längste Flugzeit einer ungelenkten Waffe (s), Schutz gegen Endlosflug.
const MAX_FALL_S: f64 = 180.0;
/// Nur so viele Ereignisse merken (für Meldungen).
const EVENT_LOG: usize = 16;

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

/// Kenngrößen einer Waffenart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponSpec {
    pub kind: WeaponKind,
    /// Kurzname für HUD und MFD.
    pub label: &'static str,
    /// Langname für Meldungen.
    pub name: &'static str,
    pub mass_kg: f64,
    /// Widerstandsfläche Cw·A (m²).
    pub drag_area_m2: f64,
    /// Widerstandsfläche mit geöffnetem Bremsschirm (m²), sonst wie oben.
    pub deployed_drag_area_m2: f64,
    /// Zeit bis zum Öffnen des Bremsschirms (s).
    pub deploy_s: f64,
    /// Stück je Schacht.
    pub per_bay: u8,
    pub seeker: Seeker,
    /// Schub des Triebwerks (N), 0 = keins.
    pub thrust_n: f64,
    /// Brenndauer (s).
    pub burn_s: f64,
    /// Größte Lenk-Querbeschleunigung (g).
    pub max_g: f64,
    /// Reichweite des Suchkopfs (m).
    pub seeker_range_m: f64,
    /// Halber Öffnungswinkel des Suchkopfs (rad).
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
    label: "B-25",
    name: "B-25 BOMB",
    mass_kg: 250.0,
    drag_area_m2: 0.020,
    deployed_drag_area_m2: 0.020,
    deploy_s: 0.0,
    per_bay: 2,
    seeker: Seeker::None,
    thrust_n: 0.0,
    burn_s: 0.0,
    max_g: 0.0,
    seeker_range_m: 0.0,
    seeker_cone_rad: 0.0,
    lethal_radius_m: 30.0,
    frag_radius_m: 180.0,
    life_s: MAX_FALL_S,
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
        per_bay: 2,
        seeker: Seeker::Optical,
        max_g: 3.0,
        seeker_range_m: 10_000.0,
        seeker_cone_rad: deg(30.0),
        lethal_radius_m: 28.0,
        ..BOMB
    },
    WeaponSpec {
        kind: WeaponKind::AirToGround,
        label: "AGR-7",
        name: "AGR-7 TV MISSILE",
        mass_kg: 210.0,
        drag_area_m2: 0.035,
        deployed_drag_area_m2: 0.035,
        deploy_s: 0.0,
        per_bay: 2,
        seeker: Seeker::Optical,
        thrust_n: 9_000.0,
        burn_s: 6.0,
        max_g: 15.0,
        seeker_range_m: 12_000.0,
        seeker_cone_rad: deg(25.0),
        lethal_radius_m: 8.0,
        frag_radius_m: 90.0,
        life_s: 60.0,
    },
    WeaponSpec {
        kind: WeaponKind::AntiRadar,
        label: "ARM-9",
        name: "ARM-9 ANTI-RADAR MISSILE",
        mass_kg: 360.0,
        drag_area_m2: 0.040,
        deployed_drag_area_m2: 0.040,
        deploy_s: 0.0,
        per_bay: 2,
        seeker: Seeker::Radiation,
        thrust_n: 20_000.0,
        burn_s: 8.0,
        max_g: 20.0,
        seeker_range_m: 40_000.0,
        seeker_cone_rad: deg(35.0),
        lethal_radius_m: 10.0,
        frag_radius_m: 100.0,
        life_s: 90.0,
    },
    WeaponSpec {
        kind: WeaponKind::AntiShip,
        label: "ASR-4",
        name: "ASR-4 ANTI-SHIP MISSILE",
        mass_kg: 520.0,
        drag_area_m2: 0.080,
        deployed_drag_area_m2: 0.080,
        deploy_s: 0.0,
        per_bay: 1,
        seeker: Seeker::Naval,
        thrust_n: 3_500.0,
        burn_s: 150.0,
        max_g: 8.0,
        seeker_range_m: 30_000.0,
        seeker_cone_rad: deg(40.0),
        lethal_radius_m: 15.0,
        frag_radius_m: 120.0,
        life_s: 160.0,
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

    pub fn is_bomb(self) -> bool {
        self.spec().thrust_n == 0.0
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

    /// Wirkung gegen eine Zielklasse (0 = wirkungslos, 1 = volle Wirkung) bei
    /// einem Eintreffwinkel `impact_rad` unter der Waagerechten (M-010, M-012).
    pub fn effectiveness(self, class: TargetClass, impact_rad: f64) -> f64 {
        use TargetClass as C;
        use WeaponKind as W;
        let steep = impact_rad >= HARDENED_MIN_IMPACT_RAD;
        match (self, class) {
            (W::AntiRadar, C::AirDefense) => 0.9,
            (W::AntiRadar, _) => 0.0,
            (W::AntiShip, C::ShipSmall) => 1.0,
            (W::AntiShip, C::ShipLarge) => 0.9,
            (W::AntiShip, _) => 0.0,
            // Gehärtete Ziele nur mit steil einfallender Bombe (Toss-Bombing,
            // Sturzflug); die Fallschirmbombe ist dafür zu leicht gebaut.
            (W::FreeFall | W::GuidedBomb, C::Hardened) => {
                if steep {
                    0.9
                } else {
                    0.0
                }
            }
            (_, C::Hardened) => 0.0,
            (W::AirToGround, C::Soft | C::ParkedAircraft) => 1.0,
            (W::AirToGround, C::Armored) => 0.9,
            (W::AirToGround, C::AirDefense | C::ShipSmall) => 0.8,
            (W::AirToGround, C::Building) => 0.5,
            (W::AirToGround, C::Bridge | C::ShipLarge) => 0.3,
            (_, C::Soft | C::ParkedAircraft) => 1.0,
            (_, C::AirDefense) => 0.9,
            (W::GuidedBomb, C::Building) => 0.9,
            (_, C::Building) => 0.8,
            (W::GuidedBomb, C::Armored | C::Bridge) => 0.8,
            (_, C::Armored | C::ShipSmall) => 0.7,
            (W::Retarded, C::Bridge) => 0.5,
            (_, C::Bridge) => 0.6,
            (W::GuidedBomb, C::ShipLarge) => 0.5,
            (_, C::ShipLarge) => 0.4,
        }
    }
}

/// Zerstörungswahrscheinlichkeit: Wirkung mal Abstandsabfall. `miss_m` ist
/// der waagerechte Abstand des Einschlags zum Zielrand (M-011).
pub fn kill_probability(kind: WeaponKind, class: TargetClass, miss_m: f64, impact_rad: f64) -> f64 {
    let r = kind.spec().lethal_radius_m;
    let x = (miss_m.max(0.0) / r - FULL_EFFECT_FRACTION) / (1.0 - FULL_EFFECT_FRACTION);
    kind.effectiveness(class, impact_rad) * (1.0 - x).clamp(0.0, 1.0)
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
}

impl Weapon {
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
}

/// Was beim Einschlag geschah.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactResult {
    /// Mindestens ein Ziel zerstört (das erste hier).
    Destroyed(usize),
    /// Ziel im Wirkbereich, aber die Waffe wirkt nicht gegen seine Klasse.
    Ineffective(usize),
    /// Ziel im Wirkbereich, aber nicht zerstört.
    Survived(usize),
    /// Kein Ziel in der Nähe.
    Ground,
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

    /// Löst die Waffe des gewählten Schachts aus, wenn alle Bedingungen
    /// erfüllt sind. `attitude`, `position`, `velocity` beschreiben das
    /// Flugzeug, `lock` ist das aufgeschaltete Ziel. Liefert die abgeworfene
    /// Masse (kg).
    pub fn release(
        &mut self,
        tick: u64,
        position: DVec3,
        velocity: DVec3,
        attitude: DQuat,
        lock: Option<usize>,
        bay_ok: bool,
    ) -> f64 {
        let bay = self.bays[self.selected];
        let (_, _, roll) = attitude.to_euler();
        let event = match bay.kind {
            _ if !bay_ok => Some(WeaponEvent::BayFault),
            _ if !self.door_open() => Some(WeaponEvent::BayClosed),
            Some(_) if bay.count == 0 => Some(WeaponEvent::BayEmpty),
            None => Some(WeaponEvent::BayEmpty),
            _ if roll.abs() > MAX_RELEASE_BANK_RAD => Some(WeaponEvent::BankLimit),
            _ if self.in_flight.len() >= MAX_IN_FLIGHT => Some(WeaponEvent::Busy),
            _ => None,
        };
        if let Some(e) = event {
            self.log(tick, e);
            return 0.0;
        }
        let kind = bay.kind.expect("geprüft");
        self.bays[self.selected].count -= 1;
        let id = self.next_id;
        self.next_id += 1;
        let down = attitude.rotate(DVec3::new(0.0, 0.0, 1.0));
        let target = if kind.is_guided() { lock } else { None };
        self.in_flight.push(Weapon {
            id,
            kind,
            position: position + down * EJECT_OFFSET_M,
            velocity: velocity + down * EJECT_SPEED_MPS,
            age_s: 0.0,
            target,
        });
        self.log(tick, WeaponEvent::Released { kind, bay: self.selected, id });
        kind.spec().mass_kg
    }

    /// Alle Waffen im Flug einen Zeitschritt weiterbewegen; Einschläge auswerten.
    pub fn step(&mut self, world: &mut World<'_>) {
        let mut i = 0;
        while i < self.in_flight.len() {
            let before = self.in_flight[i];
            let mut w = before;
            fly(&mut w, world);
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
                self.impact(world, &w, p);
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

    fn impact(&mut self, world: &mut World<'_>, w: &Weapon, p: DVec3) {
        let v = w.velocity;
        let impact_rad = detmath::atan2(v.z, (v.x * v.x + v.y * v.y).sqrt());
        let reach = w.kind.spec().lethal_radius_m;
        let mut result = ImpactResult::Ground;
        // Ziele nach Abstand, damit das Ergebnis vom nächsten Ziel ausgeht.
        let mut near: Vec<(f64, usize)> = world
            .targets
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_intact())
            .filter_map(|(i, t)| {
                let (dn, de) = (t.north - p.x, t.east - p.y);
                let miss = (dn * dn + de * de).sqrt() - t.class.radius();
                (miss <= reach).then_some((miss, i))
            })
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (miss, i) in near {
            let class = world.targets[i].class;
            let p_kill = kill_probability(w.kind, class, miss, impact_rad);
            // Immer würfeln, damit der Zufallsstrom nicht von der Wirkung abhängt.
            let roll = self.rng.next_f64();
            let this = if p_kill <= 0.0 && w.kind.effectiveness(class, impact_rad) == 0.0 {
                ImpactResult::Ineffective(i)
            } else if roll < p_kill {
                world.targets[i].state = TargetState::Destroyed;
                ImpactResult::Destroyed(i)
            } else {
                ImpactResult::Survived(i)
            };
            result = match (result, this) {
                (ImpactResult::Destroyed(_), _) => result,
                (_, ImpactResult::Destroyed(_)) | (ImpactResult::Ground, _) => this,
                _ => result,
            };
        }
        self.log(world.tick, WeaponEvent::Impact { kind: w.kind, id: w.id, position: p, result });
        if (world.aircraft - p).length() < w.kind.spec().frag_radius_m {
            self.log(world.tick, WeaponEvent::FragWarning);
        }
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
    let mut w = Weapon {
        id: 0,
        kind,
        position: position + down * EJECT_OFFSET_M,
        velocity: velocity + down * EJECT_SPEED_MPS,
        age_s: 0.0,
        target: None,
    };
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

/// Ein Zeitschritt Flugbahn: Schwerkraft, Widerstand, Triebwerk (explizit
/// über die Geschwindigkeit am Schrittanfang, dann Position mit der neuen
/// Geschwindigkeit – semi-implizit, stabil bei 120 Hz).
fn fly(w: &mut Weapon, world: &World<'_>) {
    let s = w.kind.spec();
    let dt = world.dt;
    let air = atmosphere::isa(-w.position.z);
    let speed = w.velocity.length();
    let drag_area = if w.chute_open() { s.deployed_drag_area_m2 } else { s.drag_area_m2 };
    let mut accel = DVec3::new(0.0, 0.0, G0);
    if speed > 0.0 {
        let dir = w.velocity / speed;
        accel -= dir * (0.5 * air.density * speed * speed * drag_area / s.mass_kg);
        if w.age_s < s.burn_s {
            accel += dir * (s.thrust_n / s.mass_kg);
        }
    }
    w.velocity += accel * dt;
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
    fn effect_falls_off_with_distance_and_needs_steep_impact_on_bunkers() {
        let k = WeaponKind::FreeFall;
        assert_eq!(kill_probability(k, TargetClass::Soft, 0.0, 0.5), 1.0);
        let mid = kill_probability(k, TargetClass::Soft, 20.0, 0.5);
        assert!(mid > 0.0 && mid < 1.0);
        assert_eq!(kill_probability(k, TargetClass::Soft, 31.0, 0.5), 0.0);
        assert_eq!(k.effectiveness(TargetClass::Hardened, deg(40.0)), 0.0);
        assert!(k.effectiveness(TargetClass::Hardened, deg(70.0)) > 0.5);
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
        let start = |kind| Weapon {
            id: 0,
            kind,
            position: DVec3::new(0.0, 0.0, -3_000.0),
            velocity: DVec3::new(250.0, 0.0, 0.0),
            age_s: 0.0,
            target: None,
        };
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
