//! Die Simulation: fester Zeitschritt, deterministisch, ohne Engine.

use crate::controls::{ControlFrame, flags, weapon_flags};
use crate::detmath;
use crate::events::EventLog;
use crate::fighters::{self, FighterEvent, FighterKind, Fighters};
use crate::flight::{self, AeroData, AircraftState};
use crate::flightlog::{Detector, LogEntry};
use crate::gun::{self, Gun, GunEvent};
use crate::math::{DQuat, DVec3};
use crate::mission::{self, Mission, Task};
use crate::nav::Steerpoint;
use crate::radar::{Exposure, RadarEvent, RadarKind, RadarNet};
use crate::rng::Pcg32;
use crate::rules::{Conflict, Landings, Opponents, Rules};
use crate::sensors::{self, LookDir, Pick, Seen, Sensors};
use crate::systems::{Condition, System, Systems};
use crate::targets::{self, Target, TargetClass, TargetRole};
use crate::weapons::{
    self, Bay, DecoyKind, ImpactResult, LaunchZone, Launcher, Loadout, Munitions, Seeker, Stores, Struck,
    Track, WeaponEvent, WeaponKind,
};
use f29_data::{AircraftSpec, F29_SPEC};
use f29_world::{AirfieldKind, Side, Terrain, Theater};
use std::sync::Arc;

mod briefing;
mod hash;
mod info;
mod score;

pub use score::{Score, ScoreLine};

pub use info::*;

/// Abrisswarnung unterhalb dieses Vielfachen der Abrissgeschwindigkeit.
const STALL_WARNING_MARGIN: f64 = 1.1;

/// RNG-Strom der Turbulenz.
pub const STREAM_TURBULENCE: u64 = 0x5455_5242;
/// Unterhalb dieser Höhe über Grund setzt Turbulenz ein (1000 ft).
pub const TURBULENCE_CEILING_M: f64 = 304.8;
/// Störrate (rad/s) um die Längsachse bei 200 m/s direkt über dem Boden,
/// vor der Glättung. Um die Querachse halb so viel.
const TURBULENCE_ROLL: f64 = 1.05;
/// Glättung der Turbulenz (s) – verhindert hochfrequentes Zittern.
const TURBULENCE_TIME_CONSTANT_S: f64 = 0.15;
/// Rütteln (rad/s) bei ausgefahrenem Fahrwerk, voll erreicht 25 m/s über
/// dessen Höchstfahrt.
const GEAR_BUFFET: f64 = 0.5;
/// Gleichbleibendes leichtes Rütteln (rad/s) mit ausgefahrenem Fahrwerk in
/// der Luft, unabhängig von Höhe und Fahrt.
const GEAR_SHAKE: f64 = 0.22;
/// Rütteln (rad/s) je Raketentreffer, höchstens für so viele Treffer: Ein
/// beschädigtes Flugzeug schüttelt (Spielerwunsch v0.2.9).
const DAMAGE_SHAKE: f64 = 0.07;
const DAMAGE_SHAKE_HITS: u32 = 6;
/// Schneller als das darf man die befestigte Bahn nicht verlassen (m/s).
const OFF_RUNWAY_MAX_SPEED: f64 = 12.0;
/// Startpunkt auf der Bahn: Abstand vom Bahnanfang (m).
const RUNWAY_START_OFFSET_M: f64 = 150.0;
/// Treibstoffreserve, die bei der Heimkehr übrig bleiben soll (kg).
pub const FUEL_RESERVE_KG: f64 = 450.0;
/// Verbrauch im Reiseflug für die Treibstoffschätzung (kg/km; gemessen mit
/// dem Autopiloten bei 250 m/s in 3000 m).
pub const CRUISE_FUEL_KG_PER_KM: f64 = 11.0;

/// Treibstoffbedarf der Route samt Reserve (kg) – wie das „Fuel Estimate“
/// des Originals allein aus der Streckenlänge (M-036).
pub fn fuel_required_kg(route: &[Steerpoint]) -> f64 {
    crate::nav::route_length_m(route) / 1_000.0 * CRUISE_FUEL_KG_PER_KM + FUEL_RESERVE_KG
}
/// „FUEL LOW“ unter diesem Anteil der Tankfüllung.
const FUEL_LOW_FRACTION: f64 = 0.15;
/// Stillstand: langsamer als das (m/s).
const STOPPED_SPEED: f64 = 0.5;

/// Simulationsfrequenz.
pub const TICK_HZ: u32 = 120;
/// Zeitschritt in Sekunden.
pub const DT: f64 = 1.0 / TICK_HZ as f64;

/// Wo das Flugzeug startet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SpawnKind {
    /// In der Luft, mit den Werten aus [`Spawn`].
    #[default]
    Air,
    /// Am Anfang der Bahn der Heimatbasis, stehend, Fahrwerk unten,
    /// Triebwerk im Leerlauf. Position, Höhe, Kurs und Fahrt aus [`Spawn`]
    /// werden ignoriert.
    Runway,
}

/// Startbedingungen.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Spawn {
    pub kind: SpawnKind,
    /// Tankfüllung 0..1.
    pub fuel: f64,
    pub north: f64,
    pub east: f64,
    /// Gewünschte Höhe MSL; wird angehoben, falls das Gelände zu hoch ist.
    pub altitude: f64,
    /// Mindestabstand zum Gelände beim Start.
    pub min_clearance: f64,
    /// Kurs in rad (0 = Nord, im Uhrzeigersinn).
    pub heading: f64,
    /// Wahre Fluggeschwindigkeit, m/s.
    pub speed: f64,
    /// Schubhebel 0..1, auf den auch das Triebwerk initialisiert wird.
    pub throttle: f64,
    /// Beladung der Waffenschächte (M-014); `None`: Vorschlag des Auftrags.
    pub loadout: Option<Loadout>,
    /// Nachtflug: Sensorreichweiten stark verringert (M-013).
    pub night: bool,
    /// Ein freundliches AWACS ist in der Luft: Datalink-Daten zu Zielen
    /// außerhalb der eigenen Erfassung (pseudosimuliert, M-013).
    pub awacs: bool,
    /// Testoption: Stealth-Level immer 0 – jeder Ping in Reichweite trifft
    /// (`--no-stealth`).
    pub visible: bool,
    /// Testoption: Raketentreffer werden gemeldet, richten aber keinen
    /// Schaden an (`--no-damage`); ein Absturz bleibt tödlich.
    pub invulnerable: bool,
    /// Konfliktlage und damit das Regelset (`--rules`).
    pub conflict: Conflict,
    /// Gegnerqualität und Flugverhalten (Menü „Auftrag“, M-036).
    pub opponents: Opponents,
    pub landings: Landings,
}

impl Default for Spawn {
    fn default() -> Self {
        Self {
            kind: SpawnKind::Air,
            fuel: 1.0,
            north: 0.0,
            east: 0.0,
            altitude: 2_500.0,
            min_clearance: 800.0,
            heading: std::f64::consts::FRAC_PI_4,
            speed: 210.0,
            throttle: 0.55,
            loadout: Some(Loadout::default()),
            night: false,
            awacs: true,
            visible: false,
            invulnerable: false,
            conflict: Conflict::War,
            opponents: Opponents::default(),
            landings: Landings::default(),
        }
    }
}

/// Alles, was eine Simulation eindeutig festlegt (zusammen mit den Eingaben).
#[derive(Debug, Clone, PartialEq)]
pub struct SimConfig {
    pub seed: u64,
    pub spawn: Spawn,
    /// Einsatzgebiet (M-031).
    pub theater: Arc<Theater>,
}

impl SimConfig {
    /// Luftstart im eingebauten Gebiet (für Tests, Demo und Replays).
    pub fn new(seed: u64) -> Self {
        Self::with(seed, Spawn::default())
    }

    /// Mit eigenen Startbedingungen im eingebauten Gebiet.
    pub fn with(seed: u64, spawn: Spawn) -> Self {
        Self { seed, spawn, theater: Arc::new(Theater::builtin().clone()) }
    }

    /// Start auf der Bahn der Startbasis.
    pub fn runway(seed: u64) -> Self {
        Self::with(seed, Spawn { kind: SpawnKind::Runway, throttle: 0.0, ..Spawn::default() })
    }
}

/// Warum das Flugzeug verloren ging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashCause {
    /// In Gelände oder außerhalb einer Bahn aufgeschlagen.
    Terrain,
    /// Ins Wasser gestürzt.
    Water,
    /// Mit eingefahrenem Fahrwerk aufgesetzt.
    GearUp,
    /// Zu hart (oder mit der Nase zuerst) aufgesetzt – das Fahrwerk bricht.
    GearCollapse,
    /// Mit zu viel Querlage aufgesetzt – eine Fläche schlägt auf.
    WingStrike,
    /// Zu schnell von der Bahn abgekommen.
    OffRunway,
}

impl CrashCause {
    /// Kurztext für HUD und Nachbesprechung.
    pub fn label(self) -> &'static str {
        match self {
            Self::Terrain => "GROUND IMPACT",
            Self::Water => "WATER IMPACT",
            Self::GearUp => "GEAR UP LANDING",
            Self::GearCollapse => "GEAR COLLAPSED",
            Self::WingStrike => "WING STRIKE",
            Self::OffRunway => "OFF RUNWAY",
        }
    }
}

/// Bewertung des letzten Aufsetzens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touchdown {
    pub tick: u64,
    /// Index in [`Terrain::runways`].
    pub runway: usize,
    /// Sinkrate beim Aufsetzen (m/s, positiv = sinken).
    pub sink_mps: f64,
    /// Harte Landung (über der Grenze für „gut“, unter der Bruchgrenze).
    pub hard: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Flying,
    Crashed {
        tick: u64,
        cause: CrashCause,
    },
    /// Gelandet und auf einer Bahn zum Stehen gekommen: Missionsende.
    Landed {
        tick: u64,
        side: Side,
    },
    /// Mit dem Schleudersitz ausgestiegen; `survived`: Fahrt und Lage ließen
    /// es zu.
    Ejected {
        tick: u64,
        side: Side,
        survived: bool,
    },
}

impl Status {
    /// Tick des Missionsendes (`None` im Flug).
    pub fn tick(self) -> Option<u64> {
        match self {
            Status::Flying => None,
            Status::Crashed { tick, .. } | Status::Landed { tick, .. } | Status::Ejected { tick, .. } => {
                Some(tick)
            }
        }
    }

    /// Ausgang der Mission für die Ergebnistafel (M-032).
    pub fn outcome(self) -> String {
        let fate = |side: Side| match side {
            Side::Friendly => "RESCUED",
            Side::Neutral => "INTERNATIONAL INCIDENT",
            Side::Hostile => "CAPTURED",
        };
        match self {
            Status::Flying => String::new(),
            Status::Crashed { cause, .. } => format!("CRASHED - {}", cause.label()),
            Status::Landed { side: Side::Friendly, .. } => "MISSION COMPLETE - LANDED".into(),
            Status::Landed { side, .. } => format!("LANDED - {}", fate(side)),
            Status::Ejected { survived: false, .. } => "EJECTION FAILED - KILLED".into(),
            Status::Ejected { side, .. } => format!("EJECTED - {}", fate(side)),
        }
    }
}

/// Der Schleudersitz rettet bis zu dieser Fahrt (m/s, 450 kt) …
pub const EJECT_MAX_TAS_MPS: f64 = 231.5;
/// … und bis zu dieser Quer- bzw. Längsneigung in jede Richtung (rad, 60°).
pub const EJECT_MAX_ANGLE_RAD: f64 = std::f64::consts::FRAC_PI_3;
/// Der Sitz stößt den Piloten mit dieser Geschwindigkeit (m/s) nach oben
/// aus dem Flugzeug. Danach fällt er wie eine Fallschirmbombe (derselbe
/// Flug mit Widerstand, [`weapons::fall_step`]): Masse mit Sitz (kg),
/// Widerstandsfläche Cw·A ohne und mit Schirm (m²; mit Schirm sinkt er in
/// Bodennähe mit gut 6 m/s).
const SEAT_PUSH_MPS: f64 = 30.0;
const PILOT_MASS_KG: f64 = 120.0;
const SEAT_DRAG_AREA_M2: f64 = 0.5;
const CHUTE_DRAG_AREA_M2: f64 = 50.0;
/// Der Schirm beginnt sich nach so vielen Sekunden zu öffnen und braucht
/// dafür so lange (s).
pub const CHUTE_DELAY_S: f64 = 2.0;
const CHUTE_OPENING_S: f64 = 0.6;
/// Nach einem misslungenen Ausstieg endet die Mission nach so vielen
/// Sekunden (man sieht noch den Ausschuss).
const FAILED_EJECTION_S: f64 = 1.5;

/// Der ausgestiegene Pilot (M-032): Sitz, dann Fallschirm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pilot {
    pub position: DVec3,
    pub velocity: DVec3,
    /// Kurs beim Ausstieg (rad).
    pub heading: f64,
    pub ejected_tick: u64,
    pub landed_tick: Option<u64>,
    /// Der Ausstieg ist gelungen: Der Schirm öffnet sich.
    pub survived: bool,
}

impl Pilot {
    /// Sekunden seit dem Ausstieg.
    pub fn seconds(&self, tick: u64) -> f64 {
        (tick - self.ejected_tick) as f64 * DT
    }

    /// Schirm offen 0..1.
    pub fn chute(&self, tick: u64) -> f64 {
        if !self.survived {
            return 0.0;
        }
        ((self.seconds(tick) - CHUTE_DELAY_S) / CHUTE_OPENING_S).clamp(0.0, 1.0)
    }
}

/// Ein Ereignis im einen Ereignisstrom der Simulation: von einem
/// Teilsystem oder ein Treffer mit seinen Folgen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Weapon(WeaponEvent),
    Radar(RadarEvent),
    Gun(GunEvent),
    Fighter(FighterEvent),
    /// Ein Treffer der Stärke `strength` (Rakete 1) – die eine
    /// Schadensregel (M-015), gebucht in [`Sim::hit`].
    Hit {
        on: Struck,
        by: HitBy,
        strength: f64,
    },
}

/// Womit getroffen wurde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitBy {
    Missile(WeaponKind),
    /// Eigene Kanone (`None`) bzw. die eines Jägers.
    Gun(Option<FighterKind>),
}

/// So viele Ereignisse hält der Strom (reicht für Meldungen und Effekte).
const EVENTS_KEPT: usize = 256;

/// Ein Wegpunkt gilt in diesem Umkreis als erreicht (m).
const WAYPOINT_REACHED_M: f64 = 3_000.0;

/// So viele Ziele prüft die eigene Erfassung je Erfassungstakt (reihum).
const CONTACTS_PER_UPDATE: usize = 4;

/// Die Zielhilfe wird alle so viele Ticks neu berechnet (20 Hz).
const BOMB_AIM_TICKS: u64 = 6;

/// Momentane Abschusslage für das Auslösen und die Vorausberechnung (ein
/// Makro, damit `self.stores` und `self.munitions` daneben veränderlich
/// geliehen werden können).
macro_rules! launch {
    ($sim:expr) => {
        weapons::Launch {
            tick: $sim.tick,
            position: $sim.aircraft.position,
            velocity: $sim.aircraft.velocity,
            attitude: $sim.aircraft.attitude,
            lock: $sim.picks.ground.index,
            lock_optical: $sim.picks.ground.seen.optical,
            air: $sim
                .picks
                .air
                .index
                .and_then(|i| $sim.fighters.list.get(i))
                .filter(|f| f.airborne())
                .map(|f| (f.id as usize, f.position, $sim.picks.air.seen.optical)),
            targets: &$sim.targets,
            terrain: &$sim.terrain,
            bay_ok: !$sim.systems.get(System::WeaponBay).is_failed(),
            dt: DT,
        }
    };
}

/// Die Simulation.
#[derive(Debug, Clone)]
pub struct Sim {
    config: SimConfig,
    spec: AircraftSpec,
    terrain: Terrain,
    tick: u64,
    aircraft: AircraftState,
    aero: AeroData,
    status: Status,
    route: Vec<Steerpoint>,
    active_steerpoint: usize,
    last_controls: ControlFrame,
    turbulence_rng: Pcg32,
    /// Geglättete Turbulenz-Störraten (p, q, r) in rad/s.
    gust: DVec3,
    touchdown: Option<Touchdown>,
    targets: Vec<Target>,
    sensors: Sensors,
    stores: Stores,
    munitions: Munitions,
    gun: Gun,
    /// Feindliche Radarstellungen mit Alarmstufen (M-020).
    radar: RadarNet,
    fighters: Fighters,
    /// Gewähltes Ziel je Zielart (M-013, M-022).
    picks: Picks,
    systems: Systems,
    /// Aufschlag (Punkt, Flugzeit) einer jetzt abgefeuerten Patrone; abgeleitet.
    gun_prediction: Option<(DVec3, f64)>,
    /// Vorausberechneter Aufschlag (Punkt, Fallzeit) der gewählten Bombe;
    /// abgeleitet, nicht Teil des Zustandshashes.
    bomb_prediction: Option<(DVec3, f64)>,
    /// Vorausberechnete Abschusslage der gewählten Lenkwaffe: (Ziel, Zone).
    launch_zone: Option<((usize, Seen), weapons::LaunchZone)>,
    /// Bodenziele und Jäger (nach Nummer), die die eigenen Sensoren
    /// entdecken (sonst nur AWACS-Datalink); Bodenziele reihum aktualisiert.
    /// Abgeleitet, nicht Teil des Zustandshashes.
    ground_seen: Vec<Seen>,
    fighter_seen: Vec<Seen>,
    contact_cursor: usize,
    /// Auftrag (M-032) und erledigte Aufklärung je Ziel.
    mission: Mission,
    /// Der eine Ereignisstrom (die letzten Ereignisse aller Teilsysteme).
    events: EventLog<Event, EVENTS_KEPT>,
    /// Flugprotokoll (M-032).
    log: Vec<(u64, LogEntry)>,
    /// Schon in der Luft gewesen (Landung beendet dann die Mission).
    has_flown: bool,
    /// Nach dem Ausstieg: der Pilot; und wann das verlassene Flugzeug
    /// aufschlug.
    pilot: Option<Pilot>,
}

/// Gewähltes Ziel je Zielart; der HUD-Modus bestimmt, welche Auswahl die
/// Richtungstasten bedienen.
#[derive(Debug, Clone, Copy, Default)]
pub struct Picks {
    pub ground: Pick,
    pub air: Pick,
    pub runway: Pick,
}

impl Sim {
    pub fn new(config: SimConfig) -> Self {
        let seed = config.seed;
        let terrain = Terrain::from_theater(&config.theater);
        let spec = F29_SPEC;
        let sp = &config.spawn;
        let home = terrain.runways().first().cloned();
        let mut aircraft = match (sp.kind, &home) {
            (SpawnKind::Runway, Some(rw)) => {
                let p = rw.world(-rw.length_m * 0.5 + RUNWAY_START_OFFSET_M, 0.0);
                let heading = rw.heading_rad();
                let mut a =
                    AircraftState::level(p[0], p[1], rw.elevation_m + spec.gear_height_m, heading, 0.0);
                a.on_ground = true;
                a.gear = 1.0;
                a
            }
            _ => {
                let ground = terrain.elevation(sp.north, sp.east);
                let altitude = sp.altitude.max(ground + sp.min_clearance);
                AircraftState::level(sp.north, sp.east, altitude, sp.heading, sp.speed)
            }
        };
        aircraft.engine = sp.throttle.clamp(0.0, 1.0);
        aircraft.fuel_kg = spec.fuel_capacity_kg * sp.fuel.clamp(0.0, 1.0);
        let mut targets = targets::from_theater(&config.theater, &terrain);
        let plan = mission::plan(&terrain, &targets, seed, sp.conflict);
        for (i, role) in [(plan.primary, TargetRole::Primary), (plan.secondary, TargetRole::Secondary)] {
            if let Some(i) = i {
                targets[i].role = role;
            }
        }
        let route = mission::route(&plan, &terrain, &targets);
        let suggested = || plan.loadout(&targets, fuel_required_kg(&route) > aircraft.fuel_kg);
        let stores = Stores::new(&sp.loadout.unwrap_or_else(suggested));
        aircraft.stores_kg = stores.mass_kg();
        if !aircraft.on_ground {
            // Getrimmt starten: Nase um den Anstellwinkel anheben, bei dem der
            // Auftrieb das Gewicht trägt – sonst sackt das Flugzeug zuerst durch.
            let air = crate::atmosphere::isa(aircraft.altitude());
            let qbar = 0.5 * air.density * sp.speed * sp.speed;
            if qbar > 0.0 {
                let cl_req = aircraft.mass(&spec) * crate::atmosphere::G0 / (qbar * spec.wing_area_m2);
                // Plus Durchsackwinkel, damit die Bahn anfangs waagerecht liegt.
                let vs = spec.stall_speed_1g(aircraft.mass(&spec), air.density, aircraft.flaps);
                let alpha_trim = ((cl_req - spec.cl0) / spec.cl_alpha).clamp(0.0, spec.alpha_warning_rad)
                    + flight::sink_angle(&spec, sp.speed, vs, aircraft.flaps);
                aircraft.attitude = crate::math::DQuat::from_euler(sp.heading, alpha_trim, 0.0);
            }
        }
        aircraft.height_agl_m = (aircraft.altitude()
            - terrain.elevation(aircraft.position.x, aircraft.position.y)
            - spec.gear_height_m)
            .max(0.0);
        let fighters =
            Fighters::new(&terrain, seed, sp.conflict.rules().max_airborne, sp.opponents.skill_shift());
        // Aufklärung fliegt immer bei Tag.
        let sensors = Sensors { night: sp.night && !plan.has_recon(), ..Sensors::default() };
        let aero = flight::aero_data(&spec, &aircraft);
        let last_controls = ControlFrame::from_analog(0.0, 0.0, 0.0, sp.throttle, 0);
        let aircraft_on_ground = aircraft.on_ground;
        let mut sim = Self {
            config,
            spec,
            terrain,
            tick: 0,
            aircraft,
            aero,
            status: Status::Flying,
            // Erster Wegpunkt ist die Startbasis: gleich zum nächsten.
            active_steerpoint: usize::from(route.len() > 1),
            route,
            last_controls,
            turbulence_rng: Pcg32::new(seed, STREAM_TURBULENCE),
            gust: DVec3::ZERO,
            touchdown: None,
            targets,
            sensors,
            stores,
            munitions: Munitions::new(seed),
            gun: Gun::new(seed),
            radar: RadarNet::default(),
            fighters,
            picks: Picks::default(),
            systems: Systems::default(),
            gun_prediction: None,
            bomb_prediction: None,
            launch_zone: None,
            ground_seen: Vec::new(),
            fighter_seen: Vec::new(),
            contact_cursor: 0,
            events: EventLog::default(),
            mission: plan,
            log: Vec::new(),
            has_flown: !aircraft_on_ground,
            pilot: None,
        };
        sim.ground_seen = vec![Seen::default(); sim.targets.len()];
        sim.fighter_seen = vec![Seen::default(); sim.fighters.list.len()];
        sim.pick(MarkKind::Ground, None);
        sim.radar = RadarNet::new(&sim.targets);
        for _ in 0..sim.targets.len().div_ceil(CONTACTS_PER_UPDATE) {
            sim.update_contacts();
        }
        sim.update_bomb_prediction();
        sim
    }

    /// Setzt den Zustand eines Bordsystems – Zugang für das Schadensmodell
    /// (Etappe 4) und für Tests. Die Wirkung gilt ab sofort.
    pub fn set_condition(&mut self, system: System, condition: Condition) {
        self.systems.set(system, condition);
        self.apply_systems();
    }

    /// Zerstört ein Ziel sofort – Zugang für Tests (und spätere
    /// Missionsereignisse).
    pub fn destroy_target(&mut self, index: usize) {
        if let Some(t) = self.targets.get_mut(index) {
            t.damage = 1.0;
            t.state = targets::TargetState::Destroyed;
        }
    }

    /// Bordsysteme (Schadensmodell).
    pub fn systems(&self) -> &Systems {
        &self.systems
    }

    /// Systemzustände auf Flugmodell und Sensoren übertragen.
    fn apply_systems(&mut self) {
        self.aircraft.health = self.systems.health();
        self.sensors.radar_capability = self.systems.capability(System::Radar);
        self.sensors.optics_capability = self.systems.capability(System::Optics);
        if self.systems.get(System::Radar).is_failed() {
            self.sensors.radar_on = false;
        }
    }

    /// Einen Tick (1/120 s) simulieren.
    /// Ein Takt; danach in jedem Fall die Ereignisse abholen und Auftrag und
    /// Protokoll fortführen.
    pub fn step(&mut self, controls: ControlFrame) {
        self.advance(controls);
        self.collect_events();
        self.update_mission();
    }

    fn advance(&mut self, controls: ControlFrame) {
        self.tick += 1;
        self.last_controls = controls;
        if controls.has(flags::NEXT_STEERPOINT) && !self.route.is_empty() {
            self.active_steerpoint = (self.active_steerpoint + 1) % self.route.len();
        }
        // Waffen im Flug fliegen auch nach einem Absturz weiter.
        self.step_weapons();
        self.mark_threats();
        self.fighter_decoys();
        self.step_gun();
        if self.status != Status::Flying {
            self.gun.trigger(self.tick, false, DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY, DT);
            self.radar.step(self.tick, &self.targets, None);
            self.fighters.step(self.tick, DT, &self.terrain, self.radar.latest_ping_point(), None);
            self.step_unmanned();
            self.step_pilot();
            return;
        }
        if controls.has_weapon(weapon_flags::EJECT) {
            self.eject();
            return;
        }
        self.update_weapon_controls(&controls);
        self.update_sensors(&controls);
        self.update_turbulence();
        self.update_height_agl();
        if self.aircraft.on_ground {
            self.step_ground(&controls);
        } else {
            flight::step(&self.spec, &mut self.aircraft, &controls, self.gust, DT);
            self.check_contact();
        }
        self.update_height_agl();
        self.aero = flight::aero_data(&self.spec, &self.aircraft);
        self.check_landed();
        let exposure = self.exposure();
        self.radar.step(self.tick, &self.targets, Some(&exposure));
        self.fighters.step(self.tick, DT, &self.terrain, self.radar.latest_ping_point(), Some(&exposure));
        self.launch_sams();
        self.launch_air();
        self.fighter_guns();
        // Neu gewählt: Abschusslage sofort, sonst im Takt der Bombenanzeige.
        let guided = self.stores.selected_weapon().is_some_and(|k| k.is_guided());
        let lock = self.ground_pick();
        let new_lock = guided && lock.is_some() && lock != self.launch_zone.map(|(l, _)| l);
        if self.tick.is_multiple_of(BOMB_AIM_TICKS) || new_lock {
            self.update_bomb_prediction();
            let a = &self.aircraft;
            self.gun_prediction = if a.on_ground || self.gun.rounds == Some(0) {
                None
            } else {
                gun::predict_impact(a.position, a.velocity, a.attitude, &self.terrain)
            };
        }
    }

    /// Auftrag (M-032): Fotos der Aufklärungsziele abhaken; Einsatzregeln
    /// prüfen – im Kalten Krieg darf nur getroffen werden, was uns entdeckt
    /// hat (Radarstellung mit Alarm, Jäger mit Kontakt).
    fn update_mission(&mut self) {
        let now = self.tick;
        let events: Vec<Event> = self.events.recent(now, 0).map(|&(_, e)| e).collect();
        // Entdeckungen zuerst (je Quelle einmal): Ein Treffer im selben Tick
        // auf den Entdecker ist dann schon erlaubt.
        for e in &events {
            let by = match *e {
                Event::Radar(RadarEvent::Alert { site, .. }) => {
                    Detector::Radar(self.radar.sites[site].target)
                }
                Event::Fighter(FighterEvent::Contact { kind, id }) => Detector::Fighter(id, kind),
                _ => continue,
            };
            if self.logged(LogEntry::Detected(by)).is_none() {
                self.log.push((now, LogEntry::Detected(by)));
            }
        }
        let mut ground = Vec::new();
        let mut air = Vec::new();
        let mut log = Vec::new();
        for e in events {
            match e {
                Event::Weapon(WeaponEvent::Photo { target: Some(i), .. }) => {
                    let objectives =
                        [self.mission.primary, self.mission.secondary].into_iter().zip(self.mission.tasks);
                    for (k, (o, task)) in objectives.enumerate() {
                        if o == Some(i)
                            && task == Task::Recon
                            && self.logged(LogEntry::Photographed(k)).is_none()
                        {
                            log.push(LogEntry::Photographed(k));
                        }
                    }
                }
                Event::Weapon(WeaponEvent::Impact {
                    result: ImpactResult::Destroyed { target: i, .. } | ImpactResult::Damaged(i),
                    ..
                }) => {
                    ground.push(i);
                    if !self.targets[i].is_intact() {
                        log.push(LogEntry::Destroyed(i));
                    }
                }
                Event::Gun(GunEvent::Destroyed(i)) => {
                    ground.push(i);
                    log.push(LogEntry::Destroyed(i));
                }
                Event::Hit { on: Struck::Fighter(f), .. } => air.push(f),
                Event::Hit { on: Struck::Aircraft, .. } => log.push(LogEntry::Hit),
                Event::Radar(RadarEvent::Alert { site, level: 3 }) => {
                    log.push(LogEntry::RadarId(self.radar.sites[site].target))
                }
                Event::Fighter(FighterEvent::Destroyed { kind, id }) => {
                    log.push(LogEntry::FighterKill(id, kind))
                }
                _ => {}
            }
        }
        let by_fighter = |f: usize| {
            self.log
                .iter()
                .any(|(_, e)| matches!(e, LogEntry::Detected(Detector::Fighter(id, _)) if *id as usize == f))
        };
        // Zivile Ziele zählen in der Wertung für sich (ein Abzug je Tat).
        if self.rules().attack_needs_detection
            && (ground.iter().any(|&i| {
                !self.targets[i].civilian && self.logged(LogEntry::Detected(Detector::Radar(i))).is_none()
            }) || air.iter().any(|&f| !by_fighter(f)))
        {
            log.push(LogEntry::RoeViolation);
        }
        // Wegpunkte der Route (ohne Start- und Landeplatz) beim ersten Mal.
        if self.status == Status::Flying {
            let p = self.aircraft.position;
            for k in 1..self.route.len().saturating_sub(1) {
                let sp = &self.route[k];
                if self.logged(LogEntry::Waypoint(k)).is_none()
                    && (sp.north - p.x).hypot(sp.east - p.y) < WAYPOINT_REACHED_M
                {
                    log.push(LogEntry::Waypoint(k));
                }
            }
        }
        self.log.extend(log.into_iter().map(|e| (now, e)));
    }

    /// Übernimmt die Ereignisse der Teilsysteme aus diesem Tick in den einen
    /// Ereignisstrom.
    fn collect_events(&mut self) {
        let weapons = self.munitions.events.take().into_iter().map(|(t, e)| (t, Event::Weapon(e)));
        let radar = self.radar.events.take().into_iter().map(|(t, e)| (t, Event::Radar(e)));
        let gun = self.gun.events.take().into_iter().map(|(t, e)| (t, Event::Gun(e)));
        let fighters = self.fighters.events.take().into_iter().map(|(t, e)| (t, Event::Fighter(e)));
        for (t, e) in weapons.chain(radar).chain(gun).chain(fighters).collect::<Vec<_>>() {
            self.events.push(t, e);
        }
    }

    /// Alle Ereignisse der letzten Zeit (Meldungen, Klang, Effekte), älteste
    /// zuerst.
    pub fn events(&self) -> &[(u64, Event)] {
        &self.events
    }

    /// Nur die Waffenereignisse.
    pub fn weapon_events(&self) -> Vec<(u64, WeaponEvent)> {
        self.events
            .iter()
            .filter_map(|&(t, e)| if let Event::Weapon(w) = e { Some((t, w)) } else { None })
            .collect()
    }

    /// Nur die Radarereignisse.
    pub fn radar_events(&self) -> Vec<(u64, RadarEvent)> {
        self.events
            .iter()
            .filter_map(|&(t, e)| if let Event::Radar(r) = e { Some((t, r)) } else { None })
            .collect()
    }

    /// Flugprotokoll (M-032).
    pub fn log(&self) -> &[(u64, LogEntry)] {
        &self.log
    }

    /// Ein Protokolleintrag als Text (Ergebnistafel, Nachbesprechung).
    pub fn log_text(&self, e: LogEntry) -> String {
        let name = |i: usize| self.targets[i].name;
        let role = |i: usize| match self.targets[i].role {
            TargetRole::Primary => "PRIMARY ",
            TargetRole::Secondary => "SECONDARY ",
            TargetRole::None if self.targets[i].civilian => "CIVILIAN ",
            TargetRole::None => "",
        };
        match e {
            LogEntry::Takeoff => "TAKEOFF".into(),
            LogEntry::Waypoint(k) => format!("WAYPOINT {} {}", k + 1, self.route[k].name),
            LogEntry::RadarId(i) => format!("RADAR ID - {}", name(i)),
            LogEntry::Detected(Detector::Radar(i)) => format!("DETECTED - {}", name(i)),
            LogEntry::Detected(Detector::Fighter(_, kind)) => format!("{} RADAR CONTACT", kind.spec().label),
            LogEntry::Destroyed(i) => format!("{}{} DESTROYED", role(i), name(i)),
            LogEntry::Photographed(0) => "PRIMARY PHOTOGRAPHED".into(),
            LogEntry::Photographed(_) => "SECONDARY PHOTOGRAPHED".into(),
            LogEntry::FighterKill(_, kind) => format!("{} SHOT DOWN", kind.spec().label),
            LogEntry::Hit => "AIRCRAFT HIT".into(),
            LogEntry::RoeViolation => "ROE VIOLATION".into(),
            LogEntry::Landed(side) => format!("LANDED - {}", side.label()),
            LogEntry::Ejected { side, survived: true } => format!("EJECTED - {}", side.label()),
            LogEntry::Ejected { .. } => "EJECTION FAILED".into(),
            LogEntry::PilotDown => "PILOT ON THE GROUND".into(),
            LogEntry::AircraftLost => "AIRCRAFT LOST".into(),
            LogEntry::Crashed(cause) => format!("CRASHED - {}", cause.label()),
        }
    }

    /// Auftrag der Mission.
    pub fn mission(&self) -> &Mission {
        &self.mission
    }

    /// Ist das Ziel des Auftrags `k` (0 = Primär, 1 = Sekundär) erledigt –
    /// zerstört bzw. fotografiert?
    pub fn objective_done(&self, k: usize) -> bool {
        let target = [self.mission.primary, self.mission.secondary][k];
        match self.mission.tasks[k] {
            Task::Recon => self.logged(LogEntry::Photographed(k)).is_some(),
            Task::Attack => target.is_some_and(|i| !self.targets[i].is_intact()),
        }
    }

    /// Tick des ersten Protokolleintrags `e`.
    pub fn logged(&self, e: LogEntry) -> Option<u64> {
        self.log.iter().find(|(_, x)| *x == e).map(|(t, _)| *t)
    }

    /// Verletzungen der Einsatzregeln bisher.
    pub fn roe_violations(&self) -> u32 {
        self.log.iter().filter(|(_, e)| *e == LogEntry::RoeViolation).count() as u32
    }

    /// Radarstellungen, die in diesem Tick feuern wollen, starten ihre
    /// Boden-Luft-Rakete (M-021).
    fn launch_sams(&mut self) {
        if !self.hostile_fire() {
            return;
        }
        let fire: Vec<usize> = self
            .radar
            .events
            .iter()
            .filter_map(|(_, e)| match e {
                RadarEvent::Fire { site } => Some(self.radar.sites[*site].target),
                _ => None,
            })
            .collect();
        for i in fire {
            let t = &self.targets[i];
            let Some(kind) = targets::sam_of(&self.targets, i) else { continue };
            let from = DVec3::new(t.north, t.east, -t.elevation - t.class.height());
            let max = self.rules().max_hostile_missiles;
            self.munitions.launch_sam(self.tick, kind, i, from, self.aircraft.position, max);
        }
    }

    /// Jäger, deren Radar das Flugzeug gerade erfasst hat und es vor der Nase
    /// haben, starten ihre Luft-Luft-Rakete (M-022, wie im Original) – je
    /// nach Können erst ab einem Anteil ihrer Reichweite.
    fn launch_air(&mut self) {
        if !self.hostile_fire() {
            return;
        }
        let (a, rules) = (self.aircraft.position, self.rules());
        for f in &self.fighters.list {
            let to = a - f.position;
            let ahead = f.velocity().normalize_or_zero().dot(to.normalize_or_zero());
            let kind = f.spec().missile;
            if f.airborne()
                && f.contact_tick == Some(self.tick)
                && ahead >= fighters::LAUNCH_CONE_RAD.cos()
                && to.length() <= kind.spec().max_range_m * f.skill.spec().launch_share
            {
                self.munitions.launch_air(
                    self.tick,
                    kind,
                    f.id as usize,
                    (f.position, f.velocity()),
                    a,
                    rules,
                );
            }
        }
    }

    /// Kanonen der Jäger: Feuerstöße (Länge nach Können, dazwischen
    /// [`fighters::BURST_PAUSE_S`]), solange das Flugzeug nah und fast genau
    /// vor der Nase eines verfolgenden Jägers liegt (M-015).
    fn fighter_guns(&mut self) {
        let (a, allowed) = (self.aircraft.position, self.hostile_fire());
        for i in 0..self.fighters.list.len() {
            let f = &mut self.fighters.list[i];
            let to = a - f.position;
            let nose = f.velocity().normalize_or_zero();
            let aimed = allowed
                && f.mode == fighters::Mode::Pursue
                && to.length() < fighters::GUN_RANGE_M
                && nose.dot(to.normalize_or_zero()) >= fighters::GUN_CONE_RAD.cos();
            let burst = (f.skill.spec().burst_s / DT) as u64;
            let cycle = burst + (fighters::BURST_PAUSE_S / DT) as u64;
            if aimed && f.burst_tick.is_none_or(|t| self.tick >= t + cycle) {
                f.burst_tick = Some(self.tick);
            }
            let fire = aimed && f.burst_tick.is_some_and(|t| self.tick < t + burst);
            let f = *f;
            let gun = &mut self.fighters.guns[i];
            gun.trigger(self.tick, fire, f.position, f.velocity(), f.attitude(), DT);
            gun.step(self.tick, &self.terrain, &mut [], &[(0, a)], f.position, DT);
            let hits = gun.events.take().iter().filter(|(_, e)| *e == GunEvent::AirHit(0)).count();
            let strength = gun.spec.strength;
            for _ in 0..hits {
                self.hit(Struck::Aircraft, HitBy::Gun(Some(f.kind)), strength);
            }
        }
    }

    /// Ein Treffer der Stärke `strength` (Rakete 1): die eine Schadensregel
    /// für Raketen und Kanonen (M-015) – am Jäger bis zum Abschuss, am
    /// eigenen Flugzeug über die Bordsysteme. Steht danach als
    /// [`Event::Hit`] im Ereignisstrom.
    fn hit(&mut self, on: Struck, by: HitBy, strength: f64) {
        match on {
            Struck::Fighter(i) => self.fighters.hit(self.tick, i, strength),
            Struck::Aircraft if self.config.spawn.invulnerable => {}
            Struck::Aircraft => {
                let systems = self.munitions.roll_systems(strength);
                self.systems.hit(strength, &systems);
                self.apply_systems();
            }
        }
        self.events.push(self.tick, Event::Hit { on, by, strength });
    }

    /// Regelset der Mission.
    pub fn rules(&self) -> &'static Rules {
        self.config.spawn.conflict.rules()
    }

    /// Gebiet unter `pos` (Raster des Einsatzgebiets, M-031).
    pub fn territory(&self, pos: DVec3) -> Side {
        self.config.theater.side_at(pos.x, pos.y)
    }

    /// Liegt `pos` über feindlichem Gebiet?
    pub fn over_hostile(&self, pos: DVec3) -> bool {
        self.territory(pos) == Side::Hostile
    }

    /// Darf der Gegner gerade feuern (Regelset; im Kalten Krieg nur über
    /// feindlichem Gebiet)?
    fn hostile_fire(&self) -> bool {
        !self.rules().fire_only_over_hostile || self.over_hostile(self.aircraft.position)
    }

    /// Was die feindlichen Radare vom Flugzeug sehen (M-020).
    fn exposure(&self) -> Exposure {
        let a = &self.aircraft;
        Exposure {
            position: a.position,
            velocity: a.velocity,
            roll: a.attitude.to_euler().2,
            height_agl_m: a.height_agl_m,
            door: self.stores.door,
            radar_on: self.sensors.radar_on,
            ground: self
                .terrain
                .ground_mix(a.position.x, a.position.y, crate::radar::ground_factor)
                .unwrap_or(1.0),
            ecm: self.stores.countermeasures.ecm,
            visible: self.config.spawn.visible,
        }
    }

    /// Feindliche Radarstellungen (M-020).
    pub fn radar(&self) -> &RadarNet {
        &self.radar
    }

    /// Feindliche Jäger (M-022).
    pub fn fighters(&self) -> &Fighters {
        &self.fighters
    }

    /// Beendet der letzte Tick den Zeitraffer (M-034)? Wie im Original:
    /// Fahrwerk draußen, Raketenstart gegen das Flugzeug, steigende
    /// Alarmstufe; dazu Treffer und Ende des Flugs.
    pub fn interrupts_time_warp(&self) -> bool {
        let now = self.tick;
        if self.status != Status::Flying {
            // Am Schirm darf gerafft werden; das Ende unterbricht.
            return self.mission_over();
        }
        self.aircraft.gear > 0.0
            || self.events.recent(now, 0).any(|(_, e)| {
                matches!(
                    e,
                    Event::Weapon(WeaponEvent::SamLaunch { .. } | WeaponEvent::AirLaunch { .. })
                        | Event::Radar(RadarEvent::Alert { .. })
                        | Event::Hit { on: Struck::Aircraft, .. }
                )
            })
    }

    /// Feindliche Jäger zum Verändern – Zugang für Tests.
    pub fn fighters_mut(&mut self) -> &mut Fighters {
        &mut self.fighters
    }

    /// Aufschlag der gewählten ungelenkten Bombe bzw. Abschusslage der
    /// gewählten Lenkwaffe vorausberechnen (M-014).
    fn update_bomb_prediction(&mut self) {
        let kind = self.stores.selected_weapon().filter(|_| !self.aircraft.on_ground);
        let a = &self.aircraft;
        self.bomb_prediction = match kind {
            Some(k) if k.is_bomb() => {
                weapons::predict_release(k, a.position, a.velocity, a.attitude, &self.terrain, DT)
            }
            _ => None,
        };
        self.launch_zone = match (kind, self.ground_pick()) {
            (Some(k), Some(l)) if k.is_guided() => Some((l, weapons::launch_zone(k, &launch!(self), l.0))),
            _ => None,
        };
    }

    /// Schacht öffnen/schließen, Schacht wählen, auslösen (M-014).
    fn update_weapon_controls(&mut self, controls: &ControlFrame) {
        if controls.has_weapon(weapon_flags::BAY_DOORS) {
            self.stores.door_cmd = !self.stores.door_cmd;
        }
        if controls.has_weapon(weapon_flags::NEXT_BAY) {
            self.stores.next_bay();
            self.update_bomb_prediction();
        }
        self.stores.update_door(DT, self.systems.bay_door_speed());
        // Bordkanone: eigenes System, unabhängig von Schacht und Klappen.
        let a = &self.aircraft;
        let gun_trigger = controls.has_weapon(weapon_flags::TRIGGER) && self.stores.gun_selected;
        let fire = (controls.has_weapon(weapon_flags::GUN) || gun_trigger) && !a.on_ground;
        self.gun.trigger(self.tick, fire, a.position, a.velocity, a.attitude, DT);
        // Störer: ECM ein/aus, Infrarotstörer für seine Wirkdauer
        self.stores.countermeasures.jammers(
            controls.has_weapon(weapon_flags::ECM),
            controls.has_weapon(weapon_flags::IR_JAMMER),
            DT,
        );
        // Täuschkörper (nicht am Boden)
        for (flag, kind) in [(weapon_flags::CHAFF, DecoyKind::Chaff), (weapon_flags::FLARE, DecoyKind::Flare)]
        {
            if controls.has_weapon(flag) && !self.aircraft.on_ground {
                self.stores.drop_decoy(&mut self.munitions, self.tick, kind, self.aircraft.position);
            }
        }
        // Bei gewählter Kanone löst die Auslösetaste keine Schachtwaffe aus.
        if controls.has_weapon(weapon_flags::RELEASE) && !self.stores.gun_selected {
            let launch = launch!(self);
            let tank = self.stores.selected_weapon() == Some(WeaponKind::DropTank);
            let dropped =
                if self.aircraft.on_ground { 0.0 } else { self.stores.release(&mut self.munitions, &launch) };
            self.aircraft.stores_kg = (self.aircraft.stores_kg - dropped).max(0.0);
            // Umgepumpt wird, was in die Haupttanks passt; der Rest geht verloren.
            if tank && dropped > 0.0 {
                let a = &mut self.aircraft;
                a.fuel_kg = (a.fuel_kg + weapons::TANK_FUEL_KG).min(self.spec.fuel_capacity_kg);
            }
        }
    }

    /// Jedem Jäger die nächste eigene Rakete, die ihn verfolgt und näher als
    /// [`fighters::EVADE_RANGE_M`] ist: Er weicht ihr aus.
    fn mark_threats(&mut self) {
        for f in &mut self.fighters.list {
            f.threat = None;
        }
        for w in self.munitions.in_flight.iter().filter(|w| w.owner.is_none() && w.tracking) {
            if let Some(Track::Fighter(i)) = w.target {
                let f = &mut self.fighters.list[i];
                let d = (f.position - w.position).length();
                let optical = w.kind.spec().seeker == Seeker::Optical;
                let kind = if optical { DecoyKind::Flare } else { DecoyKind::Chaff };
                if d < fighters::EVADE_RANGE_M && f.threat.is_none_or(|(p, ..)| d < (f.position - p).length())
                {
                    f.threat = Some((w.position, w.id, kind));
                }
            }
        }
    }

    /// Weicht ein Jäger aus, wirft er alle [`fighters::DECOY_INTERVAL_S`]
    /// einen passenden Täuschkörper, solange sein Vorrat reicht (M-022).
    fn fighter_decoys(&mut self) {
        let every = (fighters::DECOY_INTERVAL_S / DT) as u64;
        for (i, f) in self.fighters.list.iter_mut().enumerate() {
            let Some((m, _, kind)) = f.threat else { continue };
            if (m - f.position).length() < f.skill.spec().decoy_range_m
                && f.decoys > 0
                && f.decoy_tick.is_none_or(|t| self.tick >= t + every)
            {
                f.decoys -= 1;
                f.decoy_tick = Some(self.tick);
                self.munitions.push_decoy(self.tick, kind, f.position, Some(Launcher::Fighter(i)));
            }
        }
    }

    /// Patronen im Flug; Treffer an Jägern zählen dort (M-022).
    fn step_gun(&mut self) {
        let jets: Vec<(usize, DVec3)> =
            self.fighters.list.iter().filter(|f| f.airborne()).map(|f| (f.id as usize, f.position)).collect();
        self.gun.step(self.tick, &self.terrain, &mut self.targets, &jets, self.aircraft.position, DT);
        let now: Vec<_> = self.gun.events.iter().map(|&(_, e)| e).collect();
        for e in now {
            if let GunEvent::AirHit(i) = e {
                self.hit(Struck::Fighter(i), HitBy::Gun(None), self.gun.spec.strength);
            }
        }
    }

    fn step_weapons(&mut self) {
        if self.munitions.in_flight.is_empty() && self.munitions.decoys.is_empty() {
            return;
        }
        let sites = self.radar.sites.iter().filter(|s| s.painted).map(|s| Launcher::Site(s.target));
        let jets = self.fighters.list.iter().filter(|f| f.painting(self.tick, DT));
        let painting: Vec<Launcher> = sites.chain(jets.map(|f| Launcher::Fighter(f.id as usize))).collect();
        let fighters: Vec<_> =
            self.fighters.list.iter().map(|f| f.airborne().then(|| (f.position, f.velocity()))).collect();
        let mut world = weapons::World {
            terrain: &self.terrain,
            targets: &mut self.targets,
            tick: self.tick,
            dt: DT,
            aircraft: self.aircraft.position,
            aircraft_velocity: self.aircraft.velocity,
            designated: self.picks.ground.index.filter(|_| self.picks.ground.seen.optical),
            flying: self.status == Status::Flying,
            painting: &painting,
            fighters: &fighters,
            jammed: (self.stores.countermeasures.ecm, self.stores.countermeasures.ir_active()),
        };
        // Die Wahl bleibt auch auf einem zerstörten Ziel, bis der
        // Spieler ein neues wählt (Wracks sind dann nicht mehr anwählbar).
        self.munitions.step(&mut world);
        let now: Vec<_> = self.munitions.events.iter().map(|&(_, e)| e).collect();
        for e in now {
            if let WeaponEvent::Hit { kind, on, .. } = e {
                self.hit(on, HitBy::Missile(kind), 1.0);
            }
        }
    }

    /// Bedienung der Zielerfassung und Aufschalten (M-013): Radar umschalten,
    /// Blickrichtung wählen (dieselbe Richtung noch einmal: nächstes Ziel).
    fn update_sensors(&mut self, controls: &ControlFrame) {
        let mut refresh = self.tick.is_multiple_of(sensors::UPDATE_TICKS);
        if controls.has(flags::RADAR_TOGGLE) {
            // Ein ausgefallenes Radar lässt sich nicht einschalten.
            self.sensors.radar_on = !self.sensors.radar_on && !self.systems.get(System::Radar).is_failed();
            refresh = true;
        }
        let look = LookDir::from_code(controls.look_code());
        let pressed = if controls.has_weapon(weapon_flags::PICK_AIR) {
            MarkKind::Air
        } else if controls.has_weapon(weapon_flags::PICK_RUNWAY) {
            MarkKind::Runway
        } else {
            MarkKind::Ground
        };
        if self.tick.is_multiple_of(sensors::UPDATE_TICKS) {
            self.update_contacts();
        }
        for kind in [MarkKind::Ground, MarkKind::Air, MarkKind::Runway] {
            match look.filter(|_| kind == pressed) {
                Some(l) => self.pick(kind, Some(l)),
                None if refresh => self.pick(kind, None),
                None => {}
            }
        }
    }

    /// Richtungstaste `look` für die Zielart `kind`, ohne Taste die Prüfung
    /// der Wahl ([`Pick`]). Ein zerstörtes Bodenziel bleibt gewählt, bis der
    /// Spieler ein neues wählt (Wracks sind nicht anwählbar).
    fn pick(&mut self, kind: MarkKind, look: Option<LookDir>) {
        if look.is_none()
            && kind == MarkKind::Ground
            && self.ground_pick().is_none()
            && self.picks.ground.index.is_some()
        {
            return;
        }
        let (pos, heading) = (self.aircraft.position, self.aircraft.attitude.to_euler().0);
        let points: Vec<(usize, DVec3)> = match kind {
            MarkKind::Ground => self
                .targets
                .iter()
                .enumerate()
                .filter(|(_, t)| t.is_intact())
                .map(|(i, t)| (i, t.center()))
                .collect(),
            MarkKind::Air => self
                .fighters
                .list
                .iter()
                .filter(|f| f.airborne())
                .map(|f| (f.id as usize, f.position))
                .collect(),
            MarkKind::Runway => self.terrain.runways().iter().map(runway_point).enumerate().collect(),
        };
        let (sensors, terrain) = (&self.sensors, &self.terrain);
        // Landebahnen sind von der Karte bekannt.
        let sees = |p| {
            if kind == MarkKind::Runway {
                Seen { radar: false, optical: true }
            } else {
                sensors.sees(terrain, pos, p)
            }
        };
        let pick = match kind {
            MarkKind::Ground => &mut self.picks.ground,
            MarkKind::Air => &mut self.picks.air,
            MarkKind::Runway => &mut self.picks.runway,
        };
        match look {
            Some(l) => pick.press(l, &points, pos, heading, sees),
            None => pick.update(&points, pos, heading, sees),
        }
    }

    /// Gewähltes, intaktes Bodenziel und wie es entdeckt ist.
    fn ground_pick(&self) -> Option<(usize, Seen)> {
        let p = self.picks.ground;
        p.index.filter(|&i| self.targets[i].is_intact()).map(|i| (i, p.seen))
    }

    /// Eigene Entdeckung: Jäger jedes Mal, Bodenziele reihum für einige (die
    /// Sichtlinie ist teuer); das gewählte Bodenziel mit seinem Stand.
    fn update_contacts(&mut self) {
        let pos = self.aircraft.position;
        for f in &self.fighters.list {
            self.fighter_seen[f.id as usize] = if f.airborne() {
                self.sensors.sees(&self.terrain, pos, f.position)
            } else {
                Seen::default()
            };
        }
        let n = self.targets.len();
        for _ in 0..CONTACTS_PER_UPDATE.min(n) {
            let i = self.contact_cursor % n;
            self.contact_cursor = (i + 1) % n;
            self.ground_seen[i] = self.sensors.sees(&self.terrain, pos, self.targets[i].center());
        }
        if let Some(i) = self.picks.ground.index {
            self.ground_seen[i] = self.picks.ground.seen;
        }
    }

    fn update_height_agl(&mut self) {
        let a = &mut self.aircraft;
        let ground = self.terrain.elevation(a.position.x, a.position.y);
        a.height_agl_m = (a.altitude() - ground - self.spec.gear_height_m).max(0.0);
    }

    /// Schleudersitz: gelingt bis [`EJECT_MAX_TAS_MPS`] und
    /// [`EJECT_MAX_ANGLE_RAD`] Quer- und Längsneigung; das Gebiet entscheidet
    /// über Rettung, Zwischenfall oder Gefangenschaft.
    fn eject(&mut self) {
        let (_, pitch, roll) = self.aircraft.attitude.to_euler();
        let survived = self.aero.tas <= EJECT_MAX_TAS_MPS
            && roll.abs() <= EJECT_MAX_ANGLE_RAD
            && pitch.abs() <= EJECT_MAX_ANGLE_RAD;
        let a = &self.aircraft;
        let side = self.territory(a.position);
        self.status = Status::Ejected { tick: self.tick, side, survived };
        self.log.push((self.tick, LogEntry::Ejected { side, survived }));
        let up = a.attitude.rotate(DVec3::new(0.0, 0.0, -1.0));
        self.pilot = Some(Pilot {
            position: a.position + up * 2.0,
            velocity: a.velocity + up * SEAT_PUSH_MPS,
            heading: a.attitude.to_euler().0,
            ejected_tick: self.tick,
            landed_tick: None,
            survived,
        });
    }

    /// Das verlassene Flugzeug kippt ab und stürzt in einer steilen
    /// Spirale ab; am Boden bleibt das Wrack liegen.
    fn step_unmanned(&mut self) {
        if self.pilot.is_none() || self.wreck_tick().is_some() || self.aircraft.on_ground {
            return;
        }
        let controls = ControlFrame::from_analog(-0.4, 0.7, 0.0, self.aircraft.engine, 0);
        flight::step(&self.spec, &mut self.aircraft, &controls, DVec3::ZERO, DT);
        let a = &mut self.aircraft;
        let ground = self.terrain.elevation(a.position.x, a.position.y);
        if !a.is_finite() || a.altitude() - ground <= self.spec.belly_height_m {
            a.position.z = -ground;
            a.velocity = DVec3::ZERO;
            a.body_rates = DVec3::ZERO;
            self.log.push((self.tick, LogEntry::AircraftLost));
        }
    }

    /// Sitz: Ausschuss nach oben, die Luft bremst; dann der Schirm mit
    /// gleichmäßigem Sinken, bis der Pilot den Boden erreicht.
    fn step_pilot(&mut self) {
        let Some(p) = self.pilot.as_mut().filter(|p| p.landed_tick.is_none()) else {
            return;
        };
        let t = p.seconds(self.tick);
        let area = SEAT_DRAG_AREA_M2 + (CHUTE_DRAG_AREA_M2 - SEAT_DRAG_AREA_M2) * p.chute(self.tick);
        let density = crate::atmosphere::isa(-p.position.z).density;
        weapons::fall_step(&mut p.position, &mut p.velocity, PILOT_MASS_KG, area, density, DT);
        let ground = self.terrain.elevation(p.position.x, p.position.y);
        if t > 0.5 && -p.position.z <= ground {
            p.position.z = -ground;
            p.velocity = DVec3::ZERO;
            p.landed_tick = Some(self.tick);
            self.log.push((self.tick, LogEntry::PilotDown));
        }
    }

    /// Der ausgestiegene Pilot, falls es einen gibt.
    pub fn pilot(&self) -> Option<&Pilot> {
        self.pilot.as_ref()
    }

    /// Ist die Mission vorbei (Ergebnistafel)? Nach einem Ausstieg erst, wenn
    /// der Pilot unten ist – nach einem misslungenen kurz nach dem Ausschuss.
    pub fn mission_over(&self) -> bool {
        match (self.status, self.pilot) {
            (Status::Flying, _) => false,
            (Status::Ejected { survived: true, .. }, Some(p)) => p.landed_tick.is_some(),
            (Status::Ejected { .. }, Some(p)) => {
                (self.tick - p.ejected_tick) as f64 * DT >= FAILED_EJECTION_S
            }
            _ => true,
        }
    }

    /// Wann das verlassene Flugzeug aufschlug (Explosion, Klang).
    pub fn wreck_tick(&self) -> Option<u64> {
        self.logged(LogEntry::AircraftLost)
    }

    /// Nach einem Flug auf einer Bahn zum Stehen gekommen: Missionsende.
    fn check_landed(&mut self) {
        let a = &self.aircraft;
        if self.status != Status::Flying {
            return;
        }
        if !a.on_ground {
            if !self.has_flown {
                self.log.push((self.tick, LogEntry::Takeoff));
            }
            self.has_flown = true;
        } else if self.has_flown
            && a.velocity.length() < STOPPED_SPEED
            && self.terrain.runway_at(a.position.x, a.position.y).is_some()
        {
            let side = self.territory(a.position);
            self.status = Status::Landed { tick: self.tick, side };
            self.log.push((self.tick, LogEntry::Landed(side)));
        }
    }

    fn crash(&mut self, cause: CrashCause) {
        let p = self.aircraft.position;
        let ground = self.terrain.elevation(p.x, p.y);
        self.status = Status::Crashed { tick: self.tick, cause };
        self.log.push((self.tick, LogEntry::Crashed(cause)));
        self.aircraft.position.z = -ground;
        self.aircraft.velocity = DVec3::ZERO;
        self.aircraft.body_rates = DVec3::ZERO;
        self.aircraft.on_ground = false;
    }

    /// Rollmodus: Abheben erkennen, Abkommen von der Bahn bestrafen.
    fn step_ground(&mut self, controls: &ControlFrame) {
        let p = self.aircraft.position;
        let elevation = self.terrain.elevation(p.x, p.y);
        if flight::ground_step(&self.spec, &mut self.aircraft, controls, elevation, DT) {
            self.touchdown = None;
            return;
        }
        let p = self.aircraft.position;
        if self.terrain.ground_height(p.x, p.y) < f29_world::SEA_LEVEL_M {
            self.crash(CrashCause::Water);
        } else if self.terrain.runway_at(p.x, p.y).is_none()
            && self.aircraft.velocity.length() > OFF_RUNWAY_MAX_SPEED
        {
            self.crash(CrashCause::OffRunway);
        }
    }

    /// Im Flug: Bodenberührung erkennen und das Aufsetzen bewerten.
    fn check_contact(&mut self) {
        if !self.aircraft.is_finite() {
            self.crash(CrashCause::Terrain);
            return;
        }
        let a = &self.aircraft;
        let p = a.position;
        let ground = self.terrain.elevation(p.x, p.y);
        let agl = a.altitude() - ground;
        let contact = if a.gear_down() { self.spec.gear_height_m } else { self.spec.belly_height_m };
        let sinking = a.velocity.z > 0.0;
        if agl > contact || (!sinking && agl > 0.0) {
            return;
        }
        match self.evaluate_touchdown() {
            Ok(td) => {
                let a = &mut self.aircraft;
                let (yaw, pitch, _) = a.attitude.to_euler();
                a.on_ground = true;
                a.position.z = -(ground + self.spec.gear_height_m);
                a.velocity.z = 0.0;
                a.attitude = crate::math::DQuat::from_euler(yaw, pitch.max(0.0), 0.0);
                a.body_rates = DVec3::ZERO;
                a.stall = 0.0;
                self.touchdown = Some(td);
            }
            Err(cause) => self.crash(cause),
        }
    }

    /// Sinkrate für eine harte Landung und für Bruch (m/s) nach dem
    /// Flugverhalten; `None`: Aufsetzen auf der Bahn zerstört nie.
    pub fn landing_limits(&self) -> Option<(f64, f64)> {
        let k = self.config.spawn.landings.tolerance()?;
        Some((self.spec.hard_landing_sink_mps * k, self.spec.max_landing_sink_mps * k))
    }

    /// Bewertet die Bodenberührung: Bahn, Fahrwerk, Sinkrate, Querlage.
    fn evaluate_touchdown(&self) -> Result<Touchdown, CrashCause> {
        let a = &self.aircraft;
        let p = a.position;
        if self.terrain.ground_height(p.x, p.y) < f29_world::SEA_LEVEL_M {
            return Err(CrashCause::Water);
        }
        let Some(runway) = self.terrain.runway_at(p.x, p.y) else {
            return Err(CrashCause::Terrain);
        };
        let sink = a.velocity.z;
        let hard = self.landing_limits().map_or(self.spec.hard_landing_sink_mps, |l| l.0);
        let td = Ok(Touchdown { tick: self.tick, runway, sink_mps: sink, hard: sink > hard });
        // Flugverhalten „kein Absturz“: auf der Bahn geht alles gut.
        let Some(k) = self.config.spawn.landings.tolerance() else { return td };
        let (_, pitch, roll) = a.attitude.to_euler();
        if !a.gear_down() {
            Err(CrashCause::GearUp)
        } else if sink > self.spec.max_landing_sink_mps * k || pitch < NOSE_FIRST_PITCH * k {
            Err(CrashCause::GearCollapse)
        } else if roll.abs() > self.spec.max_landing_bank_rad * k {
            Err(CrashCause::WingStrike)
        } else {
            td
        }
    }

    /// Turbulenz in Bodennähe: unter 300 m über Grund, stärker je tiefer und
    /// je schneller; Landeklappen halbieren sie. Zufall aus eigenem RNG-Strom,
    /// geglättet, damit es rüttelt statt zittert.
    fn update_turbulence(&mut self) {
        let a = &self.aircraft;
        let ground = self.terrain.elevation(a.position.x, a.position.y);
        let agl = a.altitude() - ground;
        // Flugverhalten „kein Absturz“: kein Wackeln in Bodennähe.
        let calm = self.config.spawn.landings == Landings::NoCrash;
        let depth = if calm { 0.0 } else { (1.0 - agl / TURBULENCE_CEILING_M).clamp(0.0, 1.0) };
        let v = a.velocity.length();
        let speed = v / 200.0;
        let buffet = GEAR_SHAKE * a.gear
            + GEAR_BUFFET * a.gear * ((v - self.spec.gear_max_speed_mps) / 25.0).clamp(0.0, 1.0)
            + DAMAGE_SHAKE * f64::from(self.systems.hits.min(DAMAGE_SHAKE_HITS));
        let amplitude =
            if a.on_ground { 0.0 } else { TURBULENCE_ROLL * depth * speed * (1.0 - 0.5 * a.flaps) + buffet };
        // Zufallszahlen immer ziehen, damit der Strom nicht von der Höhe abhängt.
        let wp = self.turbulence_rng.range_f64(-1.0, 1.0);
        let wq = self.turbulence_rng.range_f64(-1.0, 1.0);
        let target = DVec3::new(wp * amplitude, wq * amplitude * 0.5, 0.0);
        let k = (DT / TURBULENCE_TIME_CONSTANT_S).min(1.0);
        self.gust += (target - self.gust) * k;
    }

    /// Aktuelle Turbulenz-Störraten (rad/s).
    pub fn turbulence(&self) -> DVec3 {
        self.gust
    }

    pub fn config(&self) -> &SimConfig {
        &self.config
    }

    pub fn spec(&self) -> &AircraftSpec {
        &self.spec
    }

    pub fn terrain(&self) -> &Terrain {
        &self.terrain
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn time_s(&self) -> f64 {
        self.tick as f64 * DT
    }

    pub fn aircraft(&self) -> &AircraftState {
        &self.aircraft
    }

    pub fn aero(&self) -> &AeroData {
        &self.aero
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn route(&self) -> &[Steerpoint] {
        &self.route
    }

    pub fn active_steerpoint(&self) -> usize {
        self.active_steerpoint
    }

    /// Bodenziele (M-010).
    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    /// Zielerfassung (M-013).
    /// Nachtflug (Aufklärungsaufträge fliegen immer bei Tag).
    pub fn night(&self) -> bool {
        self.sensors.night
    }

    pub fn sensors(&self) -> &Sensors {
        &self.sensors
    }

    pub fn picks(&self) -> &Picks {
        &self.picks
    }

    /// Waffenbestand, Schacht, Klappen, Selbstschutz (M-014).
    pub fn stores(&self) -> &Stores {
        &self.stores
    }

    /// Bordkanone (Patronen im Flug, Einschläge für die Darstellung).
    pub fn gun(&self) -> &Gun {
        &self.gun
    }

    /// Waffen und Täuschkörper im Flug.
    pub fn munitions(&self) -> &Munitions {
        &self.munitions
    }
}

/// Nicklage beim Aufsetzen, unter der das Bugrad zuerst aufschlägt (rad).
const NOSE_FIRST_PITCH: f64 = -0.087;

/// Bahnmitte (NED).
fn runway_point(r: &f29_world::Runway) -> DVec3 {
    DVec3::new(r.center[0], r.center[1], -r.elevation_m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use f29_world::Ground;

    /// Flugverhalten (M-036): Leicht verträgt das Anderthalbfache, „kein
    /// Absturz“ auf der Bahn alles – auch ohne Fahrwerk.
    #[test]
    fn landings_set_the_touchdown_limits() {
        let touch = |landings: Landings, sink: f64, gear: bool| {
            let mut sim = Sim::new(SimConfig::with(1, Spawn { landings, ..SimConfig::runway(1).spawn }));
            sim.aircraft.velocity.z = sink;
            sim.aircraft.gear = if gear { 1.0 } else { 0.0 };
            sim.evaluate_touchdown().map(|t| t.hard)
        };
        let max = F29_SPEC.max_landing_sink_mps;
        assert_eq!(touch(Landings::Realistic, max * 1.2, true), Err(CrashCause::GearCollapse));
        assert_eq!(touch(Landings::Easy, max * 1.2, true), Ok(true));
        assert_eq!(touch(Landings::Easy, max * 1.6, true), Err(CrashCause::GearCollapse));
        assert_eq!(touch(Landings::Realistic, 1.0, false), Err(CrashCause::GearUp));
        assert!(touch(Landings::NoCrash, max * 3.0, false).is_ok());
    }

    /// Radar (M-020): Mitten im Hügelland verkürzt das Gelände die Reichweite
    /// ganz, mitten im Flachland nicht, über Gebirgszellen im Schnitt stärker
    /// als über Hügeln (die Ketten sind schmal, nur ihr Kamm hat den vollen
    /// Faktor); dazwischen ändert es sich stetig wie die Landschaft, ohne
    /// Sprung an den Zellgrenzen.
    #[test]
    fn exposure_carries_the_ground_under_the_aircraft() {
        use crate::radar::ground_factor;
        let mut sim = Sim::new(SimConfig::new(1));
        let th = sim.config.theater.clone();
        let n = f29_world::theater::GRID_N;
        let cells = || (1..n - 1).flat_map(|r| (1..n - 1).map(move |c| (r, c)));
        let center = |(r, c)| f29_world::theater::cell_center(r, c);
        let mut at = |(north, east): (f64, f64)| {
            (sim.aircraft.position.x, sim.aircraft.position.y) = (north, east);
            sim.exposure().ground
        };
        let factor = |r: usize, c: usize| ground_factor(th.ground(r, c));
        for f in [ground_factor(Ground::Hills), 1.0] {
            let inner = cells().find(|&(r, c)| (0..9).all(|k| factor(r + k / 3 - 1, c + k % 3 - 1) == f));
            assert_eq!(at(center(inner.unwrap())), f);
        }
        let peaks: Vec<f64> =
            cells().filter(|&(r, c)| th.ground(r, c) == Ground::Mountains).map(|rc| at(center(rc))).collect();
        let mean = peaks.iter().sum::<f64>() / peaks.len() as f64;
        assert!(mean < ground_factor(Ground::Hills), "über Gebirge im Schnitt {mean}");
        let along: Vec<f64> = (0..5_000).map(|k| at((0.0, k as f64 * 100.0 - 250_000.0))).collect();
        let jump = along.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f64::max);
        assert!(jump < 0.02, "Sprung {jump} auf 100 m");
    }
}
