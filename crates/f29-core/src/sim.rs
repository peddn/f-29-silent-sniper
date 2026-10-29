//! Die Simulation: fester Zeitschritt, deterministisch, ohne Engine.

use crate::controls::{ControlFrame, flags};
use crate::detmath;
use crate::flight::{self, AeroData, AircraftState};
use crate::math::DVec3;
use crate::nav::{self, Steerpoint};
use crate::rng::Pcg32;
use f29_data::{AircraftSpec, F29_SPEC};
use f29_terrain::{AirfieldKind, Terrain};

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

/// Warnungen für HUD und Ton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Warnings {
    pub stall: bool,
    pub pull_up: bool,
    pub over_g: bool,
    pub crashed: bool,
    /// Weniger als 15 % Treibstoff.
    pub fuel_low: bool,
    /// Der Rest reicht nicht mehr bis zur Heimatbasis plus Reserve.
    pub bingo: bool,
    /// Tanks leer, Triebwerk aus.
    pub flameout: bool,
    /// Fahrwerk draußen und zu schnell.
    pub gear_overspeed: bool,
    /// Im Landeanflug, aber Fahrwerk nicht ausgefahren.
    pub gear_up: bool,
}

/// Informationen zum aktiven Navigationspunkt.
#[derive(Debug, Clone, PartialEq)]
pub struct SteerpointInfo {
    pub index: usize,
    pub name: String,
    /// Peilung (rechtweisend) zum Punkt, rad, 0..2π.
    pub bearing: f64,
    /// Horizontale Entfernung, m.
    pub range: f64,
    /// Richtung zum Punkt (am Boden) in Körperachsen, normiert.
    pub direction_body: DVec3,
    /// Bahnrichtung (rad, 0..π), falls der Punkt ein Flugplatz ist.
    pub runway_heading: Option<f64>,
}

/// Abgeleitete Flugdaten – alles, was Anzeigen brauchen.
#[derive(Debug, Clone, PartialEq)]
pub struct FlightData {
    pub tick: u64,
    pub time_s: f64,
    pub status: Status,
    pub position: DVec3,
    pub altitude_m: f64,
    pub ground_elevation_m: f64,
    pub radar_altitude_m: f64,
    pub heading: f64,
    pub pitch: f64,
    pub roll: f64,
    /// Anstellwinkel des Flügels (ohne Durchsackwinkel, siehe
    /// [`flight::sink_angle`]).
    pub alpha: f64,
    pub beta: f64,
    pub tas_mps: f64,
    pub mach: f64,
    pub vertical_speed_mps: f64,
    pub load_factor: f64,
    /// Flugbahnrichtung in Körperachsen (für den Flight Path Marker).
    pub velocity_dir_body: DVec3,
    pub throttle: f64,
    pub engine: f64,
    pub airbrake: f64,
    /// Landeklappen 0..1.
    pub flaps: f64,
    /// Stärke der Turbulenz (Betrag der Störraten, rad/s).
    pub turbulence: f64,
    /// Treibstoff, kg.
    pub fuel_kg: f64,
    /// Tankinhalt 0..1.
    pub fuel_fraction: f64,
    /// Aktueller Verbrauch, kg/s.
    pub fuel_flow_kgps: f64,
    /// Restflugzeit beim aktuellen Verbrauch, s.
    pub endurance_s: f64,
    /// Fahrwerk 0..1.
    pub gear: f64,
    pub on_ground: bool,
    /// Steht still auf einer Bahn (gelandet bzw. startbereit).
    pub stopped_on_runway: bool,
    /// Letztes Aufsetzen, solange das Flugzeug am Boden ist.
    pub touchdown: Option<Touchdown>,
    /// Name der Bahn unter dem Flugzeug (am Boden).
    pub runway_name: Option<&'static str>,
    /// Entfernung zur Heimatbasis, m.
    pub home_range_m: f64,
    /// Treibstoff, der bis zur Heimatbasis plus Reserve gebraucht wird (BINGO-Grenze), kg.
    pub bingo_fuel_kg: f64,
    /// Aktuelle Masse, kg.
    pub mass_kg: f64,
    /// Staudruck, Pa.
    pub qbar_pa: f64,
    /// 1-g-Abrissfahrt bei aktueller Masse, Höhe und Klappenstellung, m/s.
    pub stall_speed_mps: f64,
    pub steerpoint: Option<SteerpointInfo>,
    pub warnings: Warnings,
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
        let aero = flight::aero_data(&spec, &aircraft);
        let last_controls = ControlFrame::from_analog(0.0, 0.0, 0.0, sp.throttle, 0);
        Self {
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
        }
    }

    /// Einen Tick (1/120 s) simulieren.
    pub fn step(&mut self, controls: ControlFrame) {
        self.tick += 1;
        self.last_controls = controls;
        if controls.has(flags::NEXT_STEERPOINT) && !self.route.is_empty() {
            self.active_steerpoint = (self.active_steerpoint + 1) % self.route.len();
        }
        if self.status != Status::Flying {
            return;
        }
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
            + GEAR_BUFFET * a.gear * ((v - self.spec.gear_max_speed_mps) / 25.0).clamp(0.0, 1.0);
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

    /// Anzeige- und Autopilot-Daten für den aktuellen Zustand.
    pub fn flight_data(&self) -> FlightData {
        let a = &self.aircraft;
        let aero = &self.aero;
        let (yaw, pitch, roll) = a.attitude.to_euler();
        let ground = self.terrain.elevation(a.position.x, a.position.y);
        // Radarhöhe bezogen auf die Räder: am Boden 0.
        let radar_alt = (a.altitude() - ground - self.spec.gear_height_m).max(0.0);
        let vs = -a.velocity.z;
        let vel_body = a.attitude.inverse_rotate(a.velocity).normalize_or_zero();
        let velocity_dir_body = if vel_body == DVec3::ZERO { DVec3::X } else { vel_body };
        let crashed = matches!(self.status, Status::Crashed { .. });
        let airborne = !crashed && !a.on_ground;
        let mass = a.mass(&self.spec);

        let steerpoint = self.route.get(self.active_steerpoint).map(|sp| {
            let dn = sp.north - a.position.x;
            let de = sp.east - a.position.y;
            let to_point = DVec3::new(dn, de, -sp.elevation - a.position.z);
            SteerpointInfo {
                index: self.active_steerpoint,
                name: sp.name.clone(),
                bearing: detmath::wrap_two_pi(detmath::atan2(de, dn)),
                range: (dn * dn + de * de).sqrt(),
                direction_body: a.attitude.inverse_rotate(to_point).normalize_or_zero(),
                runway_heading: sp
                    .runway
                    .and_then(|i| self.terrain.runways().get(i))
                    .map(|rw| rw.heading_deg * (std::f64::consts::PI / 180.0)),
            }
        });

        let time_to_impact = if vs < -0.5 { radar_alt / -vs } else { f64::INFINITY };
        // Abrisswarnung: knapp über der (g-abhängigen) Abrissgeschwindigkeit,
        // bei hohem Anstellwinkel oder wenn der Abriss schon einsetzt.
        let n_demand = flight::demanded_load_factor(&self.spec, a, &self.last_controls, aero.tas, aero.qbar);
        let v_stall = flight::stall_speed(&self.spec, mass, aero.density, n_demand, a.flaps);

        // Treibstoff: Restzeit beim aktuellen Verbrauch und Bedarf bis nach Hause.
        let flow = if a.flamed_out() { 0.0 } else { flight::fuel_flow(&self.spec, a.engine, aero.density) };
        let endurance_s = if flow > 0.0 { a.fuel_kg / flow } else { 0.0 };
        let home_range_m = self.terrain.runways().first().map_or(0.0, |h| {
            let (dn, de) = (h.center[0] - a.position.x, h.center[1] - a.position.y);
            (dn * dn + de * de).sqrt()
        });
        // Rückweg mit mindestens Reisefahrt und mindestens halbem Schub gerechnet.
        let cruise_flow = flight::fuel_flow(&self.spec, a.engine.max(0.5), aero.density);
        let fuel_needed = home_range_m / aero.tas.max(150.0) * cruise_flow + FUEL_RESERVE_KG;
        let runway_here = self.terrain.runway_at(a.position.x, a.position.y);
        let stopped_on_runway = a.on_ground && runway_here.is_some() && a.velocity.length() < STOPPED_SPEED;
        // Landeanflug: tief, sinkend, langsam.
        let approach = radar_alt < 300.0
            && vs < -1.0
            && aero.tas < 1.6 * self.spec.stall_speed_1g(mass, aero.density, 0.0);

        let warnings = Warnings {
            stall: airborne
                && (a.stall > 0.02
                    || aero.wing_alpha.abs() >= self.spec.alpha_warning_rad
                    || aero.tas < STALL_WARNING_MARGIN * v_stall)
                && aero.tas > 1.0,
            // Mit ausgefahrenem Fahrwerk nur bei gefährlicher Sinkrate (Landeanflug).
            pull_up: airborne
                && radar_alt < 900.0
                && time_to_impact < 6.0
                && (!a.gear_down() || -vs > 2.0 * self.spec.max_landing_sink_mps),
            over_g: airborne && aero.load_factor > self.spec.g_limit_pos * 0.95,
            crashed,
            fuel_low: !crashed && a.fuel_kg < FUEL_LOW_FRACTION * self.spec.fuel_capacity_kg,
            bingo: airborne && a.fuel_kg < fuel_needed,
            flameout: !crashed && a.flamed_out(),
            gear_overspeed: airborne && a.gear > 0.05 && aero.tas > self.spec.gear_max_speed_mps,
            gear_up: airborne && !a.gear_down() && approach,
        };

        FlightData {
            tick: self.tick,
            time_s: self.time_s(),
            status: self.status,
            position: a.position,
            altitude_m: a.altitude(),
            ground_elevation_m: ground,
            radar_altitude_m: radar_alt,
            heading: detmath::wrap_two_pi(yaw),
            pitch,
            roll,
            alpha: aero.wing_alpha,
            beta: aero.beta,
            tas_mps: aero.tas,
            mach: aero.mach,
            vertical_speed_mps: vs,
            load_factor: aero.load_factor,
            velocity_dir_body,
            throttle: self.last_controls.throttle_f(),
            engine: a.engine,
            airbrake: a.airbrake,
            flaps: a.flaps,
            turbulence: self.gust.length(),
            fuel_kg: a.fuel_kg,
            fuel_fraction: a.fuel_kg / self.spec.fuel_capacity_kg,
            fuel_flow_kgps: flow,
            endurance_s,
            gear: a.gear,
            on_ground: a.on_ground,
            stopped_on_runway,
            touchdown: if a.on_ground { self.touchdown } else { None },
            runway_name: if a.on_ground { runway_here.map(|i| self.terrain.runways()[i].name) } else { None },
            home_range_m,
            bingo_fuel_kg: fuel_needed,
            mass_kg: mass,
            qbar_pa: aero.qbar,
            stall_speed_mps: self.spec.stall_speed_1g(mass, aero.density, a.flaps),
            steerpoint,
            warnings,
        }
    }

    /// Hash über den vollständigen Simulationszustand (FNV-1a, 64 Bit).
    ///
    /// Gleicher Seed + gleiche Eingaben ⇒ gleicher Hash, auf jeder Plattform.
    pub fn state_hash(&self) -> u64 {
        let mut h = Fnv1a::new();
        h.u64(self.config.seed);
        h.u64(self.tick);
        match self.status {
            Status::Flying => h.u64(0),
            Status::Crashed { tick, cause } => {
                h.u64(1);
                h.u64(tick);
                h.u64(cause.code());
            }
        }
        let a = &self.aircraft;
        for v in [a.position, a.velocity, a.body_rates] {
            h.f64(v.x);
            h.f64(v.y);
            h.f64(v.z);
        }
        h.f64(a.attitude.w);
        h.f64(a.attitude.x);
        h.f64(a.attitude.y);
        h.f64(a.attitude.z);
        h.f64(a.engine);
        h.f64(a.airbrake);
        h.f64(a.stall);
        h.f64(a.flaps);
        h.f64(a.fuel_kg);
        h.f64(a.gear);
        h.u64(u64::from(a.on_ground));
        if let Some(td) = &self.touchdown {
            h.u64(td.tick);
            h.f64(td.sink_mps);
        }
        h.f64(self.gust.x);
        h.f64(self.gust.y);
        let (rs, ri) = self.turbulence_rng.raw_state();
        h.u64(rs);
        h.u64(ri);
        h.u64(self.active_steerpoint as u64);
        h.finish()
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

/// FNV-1a über 64-Bit-Wörter.
#[derive(Debug, Clone)]
pub struct Fnv1a(u64);

impl Fnv1a {
    pub fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    pub fn u64(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    /// Hasht das Bitmuster; -0.0 und 0.0 werden gleich behandelt.
    pub fn f64(&mut self, v: f64) {
        let v = if v == 0.0 { 0.0 } else { v };
        self.u64(v.to_bits());
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}

impl Default for Fnv1a {
    fn default() -> Self {
        Self::new()
    }
}
