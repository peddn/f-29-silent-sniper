//! Die Simulation: fester Zeitschritt, deterministisch, ohne Engine.

use crate::controls::{ControlFrame, flags, weapon_flags};
use crate::detmath;
use crate::flight::{self, AeroData, AircraftState};
use crate::gun::{self, Gun, GunEvent};
use crate::math::{DQuat, DVec3};
use crate::nav::{self, Steerpoint};
use crate::radar::{Exposure, RadarEvent, RadarKind, RadarNet};
use crate::rng::Pcg32;
use crate::sensors::{self, LookDir, Sensors};
use crate::systems::{Condition, System, Systems};
use crate::targets::{self, Target, TargetClass, TargetRole};
use crate::weapons::{
    self, Bay, DecoyKind, ImpactResult, LaunchZone, Loadout, Stores, Track, Weapon, WeaponEvent, WeaponKind,
};
use f29_data::{AircraftSpec, F29_SPEC};
use f29_terrain::{AirfieldKind, Terrain};

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
    /// Beladung der Waffenschächte (M-014).
    pub loadout: Loadout,
    /// Nachtflug: Sensorreichweiten stark verringert (M-013).
    pub night: bool,
    /// Ein freundliches AWACS ist in der Luft: Datalink-Daten zu Zielen
    /// außerhalb der eigenen Erfassung (pseudosimuliert, M-013).
    pub awacs: bool,
    /// Testoption: Stealth-Level immer 0 – jeder Ping in Reichweite trifft
    /// (`--no-stealth`).
    pub visible: bool,
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
            loadout: Loadout::default(),
            night: false,
            awacs: true,
            visible: false,
        }
    }
}

/// Alles, was eine Simulation eindeutig festlegt (zusammen mit den Eingaben).
#[derive(Debug, Clone, PartialEq)]
pub struct SimConfig {
    pub seed: u64,
    pub spawn: Spawn,
}

impl SimConfig {
    /// Luftstart (für Tests, Demo und Replays).
    pub fn new(seed: u64) -> Self {
        Self { seed, spawn: Spawn::default() }
    }

    /// Start auf der Bahn der Heimatbasis.
    pub fn runway(seed: u64) -> Self {
        Self { seed, spawn: Spawn { kind: SpawnKind::Runway, throttle: 0.0, ..Spawn::default() } }
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
    Crashed { tick: u64, cause: CrashCause },
}

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
            lock: $sim.sensors.lock.map(|l| l.target),
            lock_optical: $sim.sensors.lock.is_some_and(|l| l.optical),
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
    systems: Systems,
    /// Aufschlag (Punkt, Flugzeit) einer jetzt abgefeuerten Patrone; abgeleitet.
    gun_prediction: Option<(DVec3, f64)>,
    /// Vorausberechneter Aufschlag (Punkt, Fallzeit) der gewählten Bombe;
    /// abgeleitet, nicht Teil des Zustandshashes.
    bomb_prediction: Option<(DVec3, f64)>,
    /// Vorausberechnete Abschusslage der gewählten Lenkwaffe: (Ziel, Zone).
    launch_zone: Option<(sensors::Lock, weapons::LaunchZone)>,
    /// Ziele, die die eigenen Sensoren erfassen (sonst nur AWACS-Datalink);
    /// reihum aktualisiert, abgeleitet, nicht Teil des Zustandshashes.
    own_contacts: Vec<bool>,
    contact_cursor: usize,
}

impl Sim {
    pub fn new(config: SimConfig) -> Self {
        let seed = config.seed;
        let terrain = Terrain::new(seed);
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
        let stores = Stores::new(&sp.loadout, seed);
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
        let route = build_route(&terrain, config.seed, sp);
        let targets = targets::generate(&terrain, seed, &route);
        let mut sensors = Sensors { night: sp.night, ..Sensors::default() };
        sensors.update(&targets, &terrain, aircraft.position, aircraft.attitude.to_euler().0);
        let aero = flight::aero_data(&spec, &aircraft);
        let last_controls = ControlFrame::from_analog(0.0, 0.0, 0.0, sp.throttle, 0);
        let mut sim = Self {
            config,
            spec,
            terrain,
            tick: 0,
            aircraft,
            aero,
            status: Status::Flying,
            route,
            active_steerpoint: 0,
            last_controls,
            turbulence_rng: Pcg32::new(seed, STREAM_TURBULENCE),
            gust: DVec3::ZERO,
            touchdown: None,
            targets,
            sensors,
            stores,
            gun: Gun::new(seed),
            radar: RadarNet::default(),
            systems: Systems::default(),
            gun_prediction: None,
            bomb_prediction: None,
            launch_zone: None,
            own_contacts: Vec::new(),
            contact_cursor: 0,
        };
        sim.own_contacts = vec![false; sim.targets.len()];
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
        self.gun.step(self.tick, &self.terrain, &mut self.targets, self.aircraft.position, DT);
        if self.status != Status::Flying {
            self.gun.trigger(self.tick, false, DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY, DT);
            self.radar.step(self.tick, &self.targets, None);
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
        let exposure = self.exposure();
        self.radar.step(self.tick, &self.targets, Some(&exposure));
        self.launch_sams();
        // Neu aufgeschaltet: Abschusslage sofort, sonst im Takt der Bombenanzeige.
        let guided = self.stores.selected_weapon().is_some_and(|k| k.is_guided());
        let lock = self.sensors.lock.filter(|l| self.targets[l.target].is_intact());
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
    }

    /// Radarstellungen, die in diesem Tick feuern wollen, starten ihre
    /// Boden-Luft-Rakete (M-021).
    fn launch_sams(&mut self) {
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
            self.stores.launch_sam(self.tick, kind, i, from, self.aircraft.position);
        }
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

    /// Aufschlag der gewählten ungelenkten Bombe bzw. Abschusslage der
    /// gewählten Lenkwaffe vorausberechnen (M-014).
    fn update_bomb_prediction(&mut self) {
        let kind = self.stores.selected_weapon().filter(|_| !self.aircraft.on_ground);
        let a = &self.aircraft;
        self.bomb_prediction = match kind {
            Some(k) if !k.is_guided() => {
                weapons::predict_release(k, a.position, a.velocity, a.attitude, &self.terrain, DT)
            }
            _ => None,
        };
        let lock = self.sensors.lock.filter(|l| self.targets[l.target].is_intact());
        self.launch_zone = match (kind, lock) {
            (Some(k), Some(l)) if k.is_guided() => {
                Some((l, weapons::launch_zone(k, &launch!(self), l.target)))
            }
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

    fn step_weapons(&mut self) {
        if self.stores.in_flight.is_empty() && self.stores.countermeasures.decoys.is_empty() {
            return;
        }
        let painting: Vec<usize> = self.radar.sites.iter().filter(|s| s.painted).map(|s| s.target).collect();
        let mut world = weapons::World {
            terrain: &self.terrain,
            targets: &mut self.targets,
            tick: self.tick,
            dt: DT,
            aircraft: self.aircraft.position,
            aircraft_velocity: self.aircraft.velocity,
            designated: self.sensors.lock.filter(|l| l.optical).map(|l| l.target),
            flying: self.status == Status::Flying,
            painting: &painting,
        };
        // Die Aufschaltung bleibt auch auf einem zerstörten Ziel, bis der
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
        if !hits.is_empty() {
            for systems in hits {
                self.systems.missile_hit(&systems);
            }
            self.apply_systems();
        }
    }

    /// Bedienung der Zielerfassung und Aufschalten (M-013): Radar umschalten,
    /// Blickrichtung wählen (dieselbe Richtung noch einmal: nächstes Ziel).
    fn update_sensors(&mut self, controls: &ControlFrame) {
        let (pos, heading) = (self.aircraft.position, self.aircraft.attitude.to_euler().0);
        let mut done = false;
        if controls.has(flags::RADAR_TOGGLE) {
            // Ein ausgefallenes Radar lässt sich nicht einschalten.
            self.sensors.radar_on = !self.sensors.radar_on && !self.systems.get(System::Radar).is_failed();
            self.sensors.update(&self.targets, &self.terrain, pos, heading);
            done = true;
        }
        if let Some(look) = LookDir::from_code(controls.look_code()) {
            // Erst die mitgeführte Blickrichtung auf den Stand bringen: Die
            // Taste der Richtung, in der das Ziel gerade liegt, schaltet weiter.
            if !done {
                self.sensors.update(&self.targets, &self.terrain, pos, heading);
            }
            if look == self.sensors.look {
                self.sensors.next_target(&self.targets, &self.terrain, pos, heading);
            } else {
                self.sensors.look_at(look, &self.targets, &self.terrain, pos, heading);
            }
            done = true;
        }
        if !done && self.tick.is_multiple_of(sensors::UPDATE_TICKS) {
            self.sensors.update(&self.targets, &self.terrain, pos, heading);
        }
        if self.tick.is_multiple_of(sensors::UPDATE_TICKS) {
            self.update_contacts();
        }
    }

    /// Eigene Erfassung reihum für einige Ziele neu bestimmen (die Sichtlinie
    /// ist teuer); das aufgeschaltete Ziel gilt immer als erfasst.
    fn update_contacts(&mut self) {
        let n = self.targets.len();
        if n == 0 {
            return;
        }
        let pos = self.aircraft.position;
        for _ in 0..CONTACTS_PER_UPDATE.min(n) {
            let i = self.contact_cursor % n;
            self.contact_cursor = (i + 1) % n;
            self.own_contacts[i] = self.sensors.sees(&self.targets[i], &self.terrain, pos);
        }
        if let Some(l) = self.sensors.lock {
            self.own_contacts[l.target] = true;
        }
    }

    fn update_height_agl(&mut self) {
        let a = &mut self.aircraft;
        let ground = self.terrain.elevation(a.position.x, a.position.y);
        a.height_agl_m = (a.altitude() - ground - self.spec.gear_height_m).max(0.0);
    }

    fn crash(&mut self, cause: CrashCause) {
        let p = self.aircraft.position;
        let ground = self.terrain.elevation(p.x, p.y);
        self.status = Status::Crashed { tick: self.tick, cause };
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
        if self.terrain.ground_height(p.x, p.y) < f29_terrain::SEA_LEVEL_M {
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
        if self.terrain.ground_height(p.x, p.y) < f29_terrain::SEA_LEVEL_M {
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
    pub fn sensors(&self) -> &Sensors {
        &self.sensors
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

/// Route: zwei Wegpunkte, dann die nächstgelegene geheime Piste, dann nach Hause.
fn build_route(terrain: &Terrain, seed: u64, sp: &Spawn) -> Vec<Steerpoint> {
    let runways = terrain.runways();
    let Some(home) = runways.first() else {
        return nav::generate_route(terrain, seed, sp.north, sp.east, 4);
    };
    let mut route = nav::generate_route(terrain, seed, home.center[0], home.center[1], 2);
    let last = route.last().map_or(home.center, |p| [p.north, p.east]);
    let dist = |c: [f64; 2]| (c[0] - last[0]) * (c[0] - last[0]) + (c[1] - last[1]) * (c[1] - last[1]);
    let strip = runways
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == AirfieldKind::Secret)
        .min_by(|a, b| dist(a.1.center).total_cmp(&dist(b.1.center)));
    if let Some((i, rw)) = strip {
        route.push(Steerpoint::runway(rw, i));
    }
    route.push(Steerpoint::runway(home, 0));
    route
}
