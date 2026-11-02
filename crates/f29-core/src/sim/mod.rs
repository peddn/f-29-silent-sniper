//! Die Simulation: fester Zeitschritt, deterministisch, ohne Engine.

use crate::controls::{ControlFrame, flags, weapon_flags};
use crate::detmath;
use crate::events::EventLog;
use crate::fighters::{self, FighterEvent, Fighters};
use crate::flight::{self, AeroData, AircraftState};
use crate::flightlog::LogEntry;
use crate::gun::{self, Gun, GunEvent};
use crate::math::{DQuat, DVec3};
use crate::mission::{self, Mission, MissionEvent, Task};
use crate::nav::Steerpoint;
use crate::radar::{Exposure, RadarEvent, RadarKind, RadarNet};
use crate::rng::Pcg32;
use crate::rules::{Conflict, Rules};
use crate::sensors::{self, LookDir, Pick, Seen, Sensors};
use crate::systems::{Condition, System, Systems};
use crate::targets::{self, Target, TargetClass, TargetRole};
use crate::weapons::{
    self, Bay, DecoyKind, ImpactResult, LaunchZone, Launcher, Loadout, Seeker, Stores, Track, Weapon,
    WeaponEvent, WeaponKind,
};
use f29_data::{AircraftSpec, F29_SPEC};
use f29_world::{AirfieldKind, Side, Terrain, Theater};
use std::sync::Arc;

mod hash;
mod info;

pub use hash::Fnv1a;
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
/// „FUEL LOW“ unter diesem Anteil der Tankfüllung.
const FUEL_LOW_FRACTION: f64 = 0.15;
/// Stillstand: langsamer als das (m/s).
const STOPPED_SPEED: f64 = 0.5;

/// Simulationsfrequenz.
pub const TICK_HZ: u32 = 120;
/// Zeitschritt in Sekunden.
pub const DT: f64 = 1.0 / TICK_HZ as f64;

/// Wo das Flugzeug startet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, PartialEq)]
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
    fn code(self) -> u64 {
        match self {
            Self::Terrain => 0,
            Self::Water => 1,
            Self::GearUp => 2,
            Self::GearCollapse => 3,
            Self::WingStrike => 4,
            Self::OffRunway => 5,
        }
    }

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
/// Ein Wegpunkt gilt in diesem Umkreis als erreicht (m).
const WAYPOINT_REACHED_M: f64 = 3_000.0;

/// So viele Ziele prüft die eigene Erfassung je Erfassungstakt (reihum).
const CONTACTS_PER_UPDATE: usize = 4;

/// Die Zielhilfe wird alle so viele Ticks neu berechnet (20 Hz).
const BOMB_AIM_TICKS: u64 = 6;

/// Momentane Abschusslage für das Auslösen und die Vorausberechnung (ein
/// Makro, damit `self.stores` daneben veränderlich geliehen werden kann).
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
    photographed: [bool; 2],
    /// Bodenziele, die das Flugzeug entdeckt haben (Einsatzregeln).
    detected_us: Vec<bool>,
    roe_violations: u32,
    mission_events: EventLog<MissionEvent>,
    /// Flugprotokoll (M-032).
    log: Vec<(u64, LogEntry)>,
    /// Schon in der Luft gewesen (Landung beendet dann die Mission).
    has_flown: bool,
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
                let heading = rw.heading_deg * (std::f64::consts::PI / 180.0);
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
        let stores = Stores::new(&sp.loadout.unwrap_or_else(|| plan.loadout(&targets)), seed);
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
        let route = mission::route(&plan, &terrain, &targets);
        let fighters = Fighters::new(&terrain, seed, sp.conflict.rules().max_airborne);
        // Aufklärung fliegt immer bei Tag.
        let sensors = Sensors { night: sp.night && !plan.has_recon(), ..Sensors::default() };
        let aero = flight::aero_data(&spec, &aircraft);
        let last_controls = ControlFrame::from_analog(0.0, 0.0, 0.0, sp.throttle, 0);
        let targets_len = targets.len();
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
            detected_us: vec![false; targets_len],
            mission: plan,
            photographed: [false; 2],
            roe_violations: 0,
            log: Vec::new(),
            has_flown: !aircraft_on_ground,
            mission_events: EventLog::default(),
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
    pub fn step(&mut self, controls: ControlFrame) {
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
            self.update_mission();
            return;
        }
        if controls.has_weapon(weapon_flags::EJECT) {
            self.eject();
            self.update_mission();
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
            self.gun_prediction = if a.on_ground || self.gun.rounds == 0 {
                None
            } else {
                gun::predict_impact(a.position, a.velocity, a.attitude, &self.terrain)
            };
        }
        self.update_mission();
    }

    /// Auftrag (M-032): Fotos der Aufklärungsziele abhaken; Einsatzregeln
    /// prüfen – im Kalten Krieg darf nur getroffen werden, was uns entdeckt
    /// hat (Radarstellung mit Alarm, Jäger mit Kontakt).
    fn update_mission(&mut self) {
        let now = self.tick;
        for s in &self.radar.sites {
            if s.alert > 0 {
                self.detected_us[s.target] = true;
            }
        }
        let mut ground = Vec::new();
        let mut air = Vec::new();
        let mut log = Vec::new();
        for &(_, e) in self.stores.events.recent(now, 0) {
            match e {
                WeaponEvent::Photo { target: Some(i), .. } => {
                    let objectives =
                        [self.mission.primary, self.mission.secondary].into_iter().zip(self.mission.tasks);
                    for (k, (o, task)) in objectives.enumerate() {
                        if o == Some(i) && task == Task::Recon && !self.photographed[k] {
                            self.photographed[k] = true;
                            self.mission_events.push(now, MissionEvent::Photographed(k));
                            log.push(LogEntry::Photographed(k));
                        }
                    }
                }
                WeaponEvent::Impact {
                    result: ImpactResult::Destroyed { target: i, .. } | ImpactResult::Damaged(i),
                    ..
                } => {
                    ground.push(i);
                    if !self.targets[i].is_intact() {
                        log.push(LogEntry::Destroyed(i));
                    }
                }
                WeaponEvent::FighterHit { fighter, .. } => air.push(fighter),
                WeaponEvent::AircraftHit { .. } => log.push(LogEntry::Hit),
                _ => {}
            }
        }
        for &(_, e) in self.gun.events.recent(now, 0) {
            match e {
                GunEvent::Destroyed(i) => {
                    ground.push(i);
                    log.push(LogEntry::Destroyed(i));
                }
                GunEvent::AirHit(f) => air.push(f),
                GunEvent::Empty => {}
            }
        }
        if self.rules().attack_needs_detection
            && (ground.iter().any(|&i| !self.detected_us[i])
                || air.iter().any(|&f| self.fighters.list[f].contact_tick.is_none()))
        {
            self.roe_violations += 1;
            self.mission_events.push(now, MissionEvent::RoeViolation);
            log.push(LogEntry::RoeViolation);
        }
        for &(_, e) in self.radar.events.recent(now, 0) {
            if let RadarEvent::Alert { site, level: 3 } = e {
                log.push(LogEntry::RadarId(self.radar.sites[site].target));
            }
        }
        for &(_, e) in self.fighters.events.recent(now, 0) {
            match e {
                FighterEvent::Contact { kind } => log.push(LogEntry::FighterContact(kind)),
                FighterEvent::Destroyed { kind } => log.push(LogEntry::FighterKill(kind)),
                FighterEvent::GunHit { .. } => log.push(LogEntry::Hit),
                _ => {}
            }
        }
        // Wegpunkte der Route (ohne Start- und Landeplatz) beim ersten Mal.
        if self.status == Status::Flying {
            let p = self.aircraft.position;
            for k in 1..self.route.len().saturating_sub(1) {
                let sp = &self.route[k];
                let reached = self.log.iter().any(|(_, e)| *e == LogEntry::Waypoint(k));
                if !reached && (sp.north - p.x).hypot(sp.east - p.y) < WAYPOINT_REACHED_M {
                    log.push(LogEntry::Waypoint(k));
                }
            }
        }
        self.log.extend(log.into_iter().map(|e| (now, e)));
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
            LogEntry::FighterContact(kind) => format!("{} RADAR CONTACT", kind.spec().label),
            LogEntry::Destroyed(i) => format!("{}{} DESTROYED", role(i), name(i)),
            LogEntry::Photographed(0) => "PRIMARY PHOTOGRAPHED".into(),
            LogEntry::Photographed(_) => "SECONDARY PHOTOGRAPHED".into(),
            LogEntry::FighterKill(kind) => format!("{} SHOT DOWN", kind.spec().label),
            LogEntry::Hit => "AIRCRAFT HIT".into(),
            LogEntry::RoeViolation => "ROE VIOLATION".into(),
            LogEntry::Landed(side) => format!("LANDED - {}", side.label()),
            LogEntry::Ejected { side, survived: true } => format!("EJECTED - {}", side.label()),
            LogEntry::Ejected { .. } => "EJECTION FAILED".into(),
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
            Task::Recon => self.photographed[k],
            Task::Attack => target.is_some_and(|i| !self.targets[i].is_intact()),
        }
    }

    pub fn mission_events(&self) -> &[(u64, MissionEvent)] {
        &self.mission_events
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
            .filter(|(t, _)| *t == self.tick)
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
            self.stores.launch_sam(self.tick, kind, i, from, self.aircraft.position, max);
        }
    }

    /// Jäger, deren Radar das Flugzeug gerade erfasst hat und es vor der Nase
    /// haben, starten ihre Luft-Luft-Rakete (M-022, wie im Original).
    fn launch_air(&mut self) {
        if !self.hostile_fire() {
            return;
        }
        let (a, rules) = (self.aircraft.position, self.rules());
        for f in &self.fighters.list {
            let to = a - f.position;
            let ahead = f.velocity().normalize_or_zero().dot(to.normalize_or_zero());
            if f.airborne() && f.contact_tick == Some(self.tick) && ahead >= fighters::LAUNCH_CONE_RAD.cos() {
                let kind = f.spec().missile;
                self.stores.launch_air(self.tick, kind, f.id as usize, (f.position, f.velocity()), a, rules);
            }
        }
    }

    /// Kanonen der Jäger: Feuer, solange das Flugzeug nah und fast genau vor
    /// der Nase eines verfolgenden Jägers liegt. Jeder Treffer kostet Schub
    /// und verschlechtert mit 15 % ein zufälliges System (M-015).
    fn fighter_guns(&mut self) {
        let (a, allowed) = (self.aircraft.position, self.hostile_fire());
        for i in 0..self.fighters.list.len() {
            let f = self.fighters.list[i];
            let to = a - f.position;
            let nose = f.velocity().normalize_or_zero();
            let fire = allowed
                && f.mode == fighters::Mode::Pursue
                && to.length() < fighters::GUN_RANGE_M
                && nose.dot(to.normalize_or_zero()) >= fighters::GUN_CONE_RAD.cos();
            let gun = &mut self.fighters.guns[i];
            gun.trigger(self.tick, fire, f.position, f.velocity(), f.attitude(), DT);
            gun.step(self.tick, &self.terrain, &mut [], &[(0, a)], f.position, DT);
            let hits =
                gun.events.iter().filter(|&&(t, e)| t == self.tick && e == GunEvent::AirHit(0)).count();
            for _ in 0..hits {
                self.fighters.events.push(self.tick, FighterEvent::GunHit { kind: f.kind });
                if !self.config.spawn.invulnerable {
                    let system = self.stores.roll_gun_system();
                    self.systems.gun_hit(system);
                    self.apply_systems();
                }
            }
        }
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
        self.aircraft.gear > 0.0
            || self.status != Status::Flying
            || self.stores.events.iter().any(|(t, e)| {
                *t == now
                    && matches!(
                        e,
                        WeaponEvent::SamLaunch { .. }
                            | WeaponEvent::AirLaunch { .. }
                            | WeaponEvent::AircraftHit { .. }
                    )
            })
            || self.radar.events.recent(now, 0).any(|(_, e)| matches!(e, RadarEvent::Alert { .. }))
            || self.fighters.events.recent(now, 0).any(|(_, e)| matches!(e, FighterEvent::GunHit { .. }))
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
                self.stores.drop_decoy(self.tick, kind, self.aircraft.position);
            }
        }
        // Bei gewählter Kanone löst die Auslösetaste keine Schachtwaffe aus.
        if controls.has_weapon(weapon_flags::RELEASE) && !self.stores.gun_selected {
            let launch = launch!(self);
            let dropped = if self.aircraft.on_ground { 0.0 } else { self.stores.release(&launch) };
            self.aircraft.stores_kg = (self.aircraft.stores_kg - dropped).max(0.0);
        }
    }

    /// Jedem Jäger die nächste eigene Rakete, die ihn verfolgt und näher als
    /// [`fighters::EVADE_RANGE_M`] ist: Er weicht ihr aus.
    fn mark_threats(&mut self) {
        for f in &mut self.fighters.list {
            f.threat = None;
        }
        for w in self.stores.in_flight.iter().filter(|w| w.owner.is_none() && w.tracking) {
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
            if (m - f.position).length() < fighters::DECOY_RANGE_M
                && f.decoys > 0
                && f.decoy_tick.is_none_or(|t| self.tick >= t + every)
            {
                f.decoys -= 1;
                f.decoy_tick = Some(self.tick);
                self.stores.push_decoy(self.tick, kind, f.position, Some(Launcher::Fighter(i)));
            }
        }
    }

    /// Patronen im Flug; Treffer an Jägern zählen dort (M-022).
    fn step_gun(&mut self) {
        let jets: Vec<(usize, DVec3)> =
            self.fighters.list.iter().filter(|f| f.airborne()).map(|f| (f.id as usize, f.position)).collect();
        self.gun.step(self.tick, &self.terrain, &mut self.targets, &jets, self.aircraft.position, DT);
        for (_, e) in self.gun.events.iter().filter(|(t, _)| *t == self.tick) {
            if let GunEvent::AirHit(i) = *e {
                self.fighters.hit(self.tick, i, false);
            }
        }
    }

    fn step_weapons(&mut self) {
        if self.stores.in_flight.is_empty() && self.stores.countermeasures.decoys.is_empty() {
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
        };
        // Die Wahl bleibt auch auf einem zerstörten Ziel, bis der
        // Spieler ein neues wählt (Wracks sind dann nicht mehr anwählbar).
        self.stores.step(&mut world);
        // Raketentreffer: Folgen nur über die Bordsysteme (M-015).
        let hits: Vec<_> = self
            .stores
            .events
            .iter()
            .filter(|(t, _)| *t == self.tick)
            .filter_map(|(_, e)| match e {
                WeaponEvent::AircraftHit { systems, .. } => Some(*systems),
                _ => None,
            })
            .collect();
        let downed: Vec<usize> = self
            .stores
            .events
            .iter()
            .filter(|(t, _)| *t == self.tick)
            .filter_map(|(_, e)| match e {
                WeaponEvent::FighterHit { fighter, .. } => Some(*fighter),
                _ => None,
            })
            .collect();
        for i in downed {
            self.fighters.hit(self.tick, i, true);
        }
        if !hits.is_empty() && !self.config.spawn.invulnerable {
            for systems in hits {
                self.systems.missile_hit(&systems);
            }
            self.apply_systems();
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
                .map(|(i, t)| (i, center(t)))
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
            self.ground_seen[i] = self.sensors.sees(&self.terrain, pos, center(&self.targets[i]));
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
        let side = self.territory(self.aircraft.position);
        self.status = Status::Ejected { tick: self.tick, side, survived };
        self.log.push((self.tick, LogEntry::Ejected { side, survived }));
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
        if !a.gear_down() {
            return Err(CrashCause::GearUp);
        }
        let sink = a.velocity.z;
        let (_, pitch, roll) = a.attitude.to_euler();
        if sink > self.spec.max_landing_sink_mps || pitch < NOSE_FIRST_PITCH {
            return Err(CrashCause::GearCollapse);
        }
        if roll.abs() > self.spec.max_landing_bank_rad {
            return Err(CrashCause::WingStrike);
        }
        Ok(Touchdown {
            tick: self.tick,
            runway,
            sink_mps: sink,
            hard: sink > self.spec.hard_landing_sink_mps,
        })
    }

    /// Turbulenz in Bodennähe: unter 500 ft über Grund, stärker je tiefer und
    /// je schneller; Landeklappen halbieren sie. Zufall aus eigenem RNG-Strom,
    /// geglättet, damit es rüttelt statt zittert.
    fn update_turbulence(&mut self) {
        let a = &self.aircraft;
        let ground = self.terrain.elevation(a.position.x, a.position.y);
        let agl = a.altitude() - ground;
        let depth = (1.0 - agl / TURBULENCE_CEILING_M).clamp(0.0, 1.0);
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

    /// Waffenbestand, Schacht und Waffen im Flug (M-014).
    pub fn stores(&self) -> &Stores {
        &self.stores
    }

    /// Bordkanone (Patronen im Flug, Einschläge für die Darstellung).
    pub fn gun(&self) -> &Gun {
        &self.gun
    }

    /// Waffen im Flug (für die Darstellung).
    pub fn weapons_in_flight(&self) -> &[Weapon] {
        &self.stores.in_flight
    }

    /// Waffenereignisse mit Tick, neueste zuletzt.
    pub fn weapon_events(&self) -> &[(u64, WeaponEvent)] {
        &self.stores.events
    }
}

/// Nicklage beim Aufsetzen, unter der das Bugrad zuerst aufschlägt (rad).
const NOSE_FIRST_PITCH: f64 = -0.087;

/// Mitte eines Bodenziels (NED).
fn center(t: &Target) -> DVec3 {
    let c = t.center_ned();
    DVec3::new(c[0], c[1], c[2])
}

/// Bahnmitte (NED).
fn runway_point(r: &f29_world::Runway) -> DVec3 {
    DVec3::new(r.center[0], r.center[1], -r.elevation_m)
}
