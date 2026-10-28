//! Die Simulation: fester Zeitschritt, deterministisch, ohne Engine.

use crate::controls::{ControlFrame, flags};
use crate::detmath;
use crate::flight::{self, AeroData, AircraftState};
use crate::math::DVec3;
use crate::nav::{self, Steerpoint};
use crate::rng::Pcg32;
use f29_data::{AircraftSpec, F29_SPEC};
use f29_terrain::Terrain;

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

/// Simulationsfrequenz.
pub const TICK_HZ: u32 = 120;
/// Zeitschritt in Sekunden.
pub const DT: f64 = 1.0 / TICK_HZ as f64;

/// Startbedingungen.
#[derive(Debug, Clone, PartialEq)]
pub struct Spawn {
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
    pub fn new(seed: u64) -> Self {
        Self { seed, spawn: Spawn::default() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Flying,
    Crashed { tick: u64 },
}

/// Warnungen für HUD und Ton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Warnings {
    pub stall: bool,
    pub pull_up: bool,
    pub over_g: bool,
    pub crashed: bool,
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
}

impl Sim {
    pub fn new(config: SimConfig) -> Self {
        let seed = config.seed;
        let terrain = Terrain::new(seed);
        let spec = F29_SPEC;
        let sp = &config.spawn;
        let ground = terrain.elevation(sp.north, sp.east);
        let altitude = sp.altitude.max(ground + sp.min_clearance);
        let mut aircraft = AircraftState::level(sp.north, sp.east, altitude, sp.heading, sp.speed);
        aircraft.engine = sp.throttle.clamp(0.0, 1.0);
        // Getrimmt starten: Nase um den Anstellwinkel anheben, bei dem der
        // Auftrieb das Gewicht trägt – sonst sackt das Flugzeug zuerst durch.
        let air = crate::atmosphere::isa(altitude);
        let qbar = 0.5 * air.density * sp.speed * sp.speed;
        if qbar > 0.0 {
            let cl_req = spec.mass_kg * crate::atmosphere::G0 / (qbar * spec.wing_area_m2);
            let alpha_trim = ((cl_req - spec.cl0) / spec.cl_alpha).clamp(0.0, spec.alpha_warning_rad);
            aircraft.attitude = crate::math::DQuat::from_euler(sp.heading, alpha_trim, 0.0);
        }
        let route = nav::generate_route(&terrain, config.seed, sp.north, sp.east, 4);
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
        flight::step(&self.spec, &mut self.aircraft, &controls, self.gust, DT);
        self.aero = flight::aero_data(&self.spec, &self.aircraft);

        let p = self.aircraft.position;
        let ground = self.terrain.elevation(p.x, p.y);
        if self.aircraft.altitude() <= ground || !self.aircraft.is_finite() {
            self.status = Status::Crashed { tick: self.tick };
            self.aircraft.position.z = -ground;
            self.aircraft.velocity = DVec3::ZERO;
            self.aircraft.body_rates = DVec3::ZERO;
        }
    }

    /// Turbulenz in Bodennähe: unter 500 ft über Grund, stärker je tiefer und
    /// je schneller; Landeklappen halbieren sie. Zufall aus eigenem RNG-Strom,
    /// geglättet, damit es rüttelt statt zittert.
    fn update_turbulence(&mut self) {
        let a = &self.aircraft;
        let ground = self.terrain.elevation(a.position.x, a.position.y);
        let agl = a.altitude() - ground;
        let depth = (1.0 - agl / TURBULENCE_CEILING_M).clamp(0.0, 1.0);
        let speed = a.velocity.length() / 200.0;
        let amplitude = TURBULENCE_ROLL * depth * speed * (1.0 - 0.5 * a.flaps);
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
        let radar_alt = a.altitude() - ground;
        let vs = -a.velocity.z;
        let vel_body = a.attitude.inverse_rotate(a.velocity).normalize_or_zero();
        let velocity_dir_body = if vel_body == DVec3::ZERO { DVec3::X } else { vel_body };
        let crashed = matches!(self.status, Status::Crashed { .. });

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
            }
        });

        let time_to_impact = if vs < -0.5 { radar_alt / -vs } else { f64::INFINITY };
        // Abrisswarnung: knapp über der (g-abhängigen) Abrissgeschwindigkeit,
        // bei hohem Anstellwinkel oder wenn der Abriss schon einsetzt.
        let n_demand = flight::demanded_load_factor(&self.spec, &self.last_controls);
        let v_stall = flight::stall_speed(&self.spec, aero.density, n_demand, a.flaps);
        let warnings = Warnings {
            stall: !crashed
                && (a.stall > 0.02
                    || aero.alpha.abs() >= self.spec.alpha_warning_rad
                    || aero.tas < STALL_WARNING_MARGIN * v_stall)
                && aero.tas > 1.0,
            pull_up: !crashed && radar_alt < 900.0 && time_to_impact < 6.0,
            over_g: !crashed && aero.load_factor > self.spec.g_limit_pos * 0.95,
            crashed,
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
            alpha: aero.alpha,
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
            Status::Crashed { tick } => {
                h.u64(1);
                h.u64(tick);
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
        h.f64(self.gust.x);
        h.f64(self.gust.y);
        let (rs, ri) = self.turbulence_rng.raw_state();
        h.u64(rs);
        h.u64(ri);
        h.u64(self.active_steerpoint as u64);
        h.finish()
    }
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
