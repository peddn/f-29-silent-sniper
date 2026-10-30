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
use crate::weapons::{self, Bay, ImpactResult, LaunchZone, Loadout, Stores, Weapon, WeaponEvent, WeaponKind};
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
    /// Eine feindliche Rakete verfolgt das Flugzeug (Raketenwarnung, M-021).
    pub missile: bool,
}

/// Das aufgeschaltete Ziel (M-013).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetInfo {
    /// Index in [`Sim::targets`].
    pub index: usize,
    pub name: &'static str,
    pub class: TargetClass,
    /// Zivil oder befreundet: nicht angreifen.
    pub civilian: bool,
    pub role: TargetRole,
    /// Optisch erfasst: Zielbild verfügbar.
    pub optical: bool,
    /// Vom Radar erfasst.
    pub radar: bool,
    /// Schrägentfernung zur Zielmitte (m).
    pub range: f64,
    /// Peilung (rechtweisend, rad, 0..2π).
    pub bearing: f64,
    /// Richtung zur Zielmitte in Körperachsen, normiert.
    pub direction_body: DVec3,
    /// Vektor vom Flugzeug zum Zielfußpunkt (NED, m) – für das Zielbild.
    pub offset_ned: DVec3,
    /// Ausrichtung des Ziels (rad, 0 = Nord).
    pub heading: f64,
    /// Reichweitenzone der gewählten Lenkwaffe gegen dieses Ziel (Zielrahmen);
    /// `None`, wenn keine Lenkwaffe gewählt ist.
    pub zone: Option<LaunchZone>,
    /// Angesammelter Schaden 0..1.
    pub damage: f64,
    /// Zerstört: Die Aufschaltung bleibt auf dem Wrack, bis ein neues Ziel
    /// gewählt wird.
    pub destroyed: bool,
}

/// Waffenbestand für die Anzeigen (M-014).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StoresInfo {
    pub bays: [Bay; weapons::BAYS],
    /// Gewählter Schacht (0-basiert); gilt nur, wenn nicht die Kanone gewählt ist.
    pub selected: usize,
    /// Die Bordkanone ist gewählt.
    pub gun_selected: bool,
    /// Schachtklappen 0 (zu) .. 1 (offen).
    pub door: f64,
    /// Eigene Waffen in der Luft.
    pub in_flight: usize,
    /// Masse der Waffen an Bord (kg).
    pub mass_kg: f64,
}

/// Zielhilfe für ungelenkte Bomben (M-014): vorausberechneter Aufschlag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BombAim {
    /// Aufschlagpunkt (NED, auf der Oberfläche).
    pub impact: DVec3,
    /// Richtung zum Aufschlagpunkt in Körperachsen, normiert.
    pub direction_body: DVec3,
    /// Fallzeit (s).
    pub fall_time_s: f64,
    /// Der Einschlag läge im Splitterbereich des weiterfliegenden Flugzeugs.
    pub too_low: bool,
    /// Ablage des aufgeschalteten (intakten) Ziels vom Aufschlagpunkt (m):
    /// längs der Flugrichtung (+ = Ziel liegt noch voraus, Bombe fiele zu kurz)
    /// und quer (+ = Ziel liegt rechts).
    pub target_offset: Option<(f64, f64)>,
    /// Wirkradius der gewählten Bombe (m), für die Auslöseanzeige.
    pub lethal_radius_m: f64,
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

/// Meldung zu einem Waffenereignis, z. B. „MK-82 AWAY“.
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponMessage {
    pub text: String,
    /// Als Warnung zeigen (rot).
    pub warning: bool,
    /// Alter der Meldung (s).
    pub age_s: f64,
}

/// So lange bleibt eine Waffenmeldung stehen (s).
pub const WEAPON_MESSAGE_S: f64 = 4.0;

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
    /// Nettoschub (N).
    pub thrust_n: f64,
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
    /// Radar eingeschaltet.
    pub radar_on: bool,
    /// Blickrichtung der Zielerfassung.
    pub look: LookDir,
    /// Aufgeschaltetes Ziel.
    pub target: Option<TargetInfo>,
    /// Waffenbestand, Schacht, Waffen in der Luft.
    pub stores: StoresInfo,
    /// Aktuelle Waffenmeldungen, neueste zuletzt (höchstens drei).
    pub weapon_messages: Vec<WeaponMessage>,
    /// Zustand der Bordsysteme (Schadensmodell).
    pub systems: Systems,
    /// Indizes der zerstörten Ziele, aufsteigend (die Karte blendet sie aus).
    pub destroyed_targets: Vec<usize>,
    /// Indizes der Ziele, die die eigenen Sensoren erfassen, aufsteigend. Alle
    /// anderen kennt das Flugzeug nur über den AWACS-Datalink (verminderte
    /// Information: Lage und Zieltyp, aber keine Kennung „zivil“) – und nur,
    /// wenn ein AWACS in der Luft ist.
    pub own_contacts: Vec<usize>,
    /// Ein freundliches AWACS ist in der Luft (Datalink).
    pub awacs: bool,
    /// Zielhilfe, wenn eine ungelenkte Bombe gewählt ist und das Flugzeug fliegt.
    pub bomb_aim: Option<BombAim>,
    /// Waffen in der Luft für die Karte: eigene und (ab Etappe 3) feindliche.
    pub munitions: Vec<MunitionInfo>,
    /// Bild der jüngsten eigenen Waffe mit Fernsehsuchkopf in der Luft.
    pub weapon_cam: Option<WeaponCam>,
    /// Bordkanone: Munition, Abzug und Aufschlagpunkt.
    pub gun: GunInfo,
    /// Lage gegenüber den feindlichen Radaren (M-020).
    pub threat: ThreatInfo,
    pub warnings: Warnings,
}

/// Lage gegenüber den feindlichen Radaren (M-020).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThreatInfo {
    /// Eigenes Stealth-Level 0..1 (ein Wert für alle Radare).
    pub stealth: f64,
    /// Die Stellung mit dem stärksten Impuls am Flugzeug.
    pub worst: Option<ThreatSite>,
    /// Höchste Alarmstufe aller Stellungen.
    pub max_alert: u8,
    /// Letzter Ping, der das Flugzeug erreicht hat.
    pub last_ping: Option<PingInfo>,
    /// Jüngster gemerkter Ping-Ort (NED).
    pub ping_point: Option<DVec3>,
    /// Alle intakten Radarstellungen (für die Karte).
    pub sites: Vec<RadarSiteInfo>,
}

/// Eine Radarstellung in Reichweite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThreatSite {
    /// Index in [`RadarNet::sites`].
    pub site: usize,
    /// Index in [`Sim::targets`].
    pub target: usize,
    pub name: &'static str,
    pub kind: RadarKind,
    /// Stärke ihres Impulses am Flugzeug 0..1.
    pub signal: f64,
    pub alert: u8,
    pub range_m: f64,
}

/// Letzter Ping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PingInfo {
    pub tick: u64,
    pub age_s: f64,
    pub signal: f64,
    pub stealth: f64,
    pub hit: bool,
    /// Typ des pingenden Radars.
    pub kind: RadarKind,
}

/// Radarstellung für die Karte.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadarSiteInfo {
    pub target: usize,
    /// Nord, Ost (m).
    pub pos: [f64; 2],
    pub range_m: f64,
    pub kind: RadarKind,
    pub alert: u8,
}

/// Bordkanone für die Anzeigen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GunInfo {
    pub rounds: u16,
    /// Abzug gedrückt.
    pub firing: bool,
    /// Wo eine jetzt abgefeuerte Patrone auf dem Gelände einschlüge – nur
    /// innerhalb ihrer Flugzeit.
    pub aim: Option<GunAim>,
}

/// Aufschlagpunkt der Kanone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GunAim {
    /// NED (m).
    pub impact: DVec3,
    /// Richtung zum Aufschlagpunkt in Körperachsen, normiert.
    pub direction_body: DVec3,
    /// Schrägentfernung (m).
    pub range_m: f64,
}

/// Eine Waffe in der Luft, wie die Karte sie zeigt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MunitionInfo {
    /// NED (m).
    pub position: DVec3,
    /// NED (m/s).
    pub velocity: DVec3,
    /// Rakete (mit Antrieb) statt Bombe: Die Karte zeigt sie als Strich.
    pub missile: bool,
    /// Feindliche Waffe (Boden-Luft-Rakete).
    pub hostile: bool,
}

/// Suchkopfbild einer Waffe im Flug (WPN-CAM-Seite).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponCam {
    pub id: u32,
    pub kind: WeaponKind,
    /// NED (m).
    pub position: DVec3,
    /// Blickrichtung (NED, Einheitsvektor): zur Zielmitte, solange der
    /// Suchkopf das Ziel verfolgt, sonst in Flugrichtung.
    pub look: DVec3,
    /// Verfolgtes Ziel und Entfernung zu seiner Mitte (m).
    pub tracking: Option<(usize, f64)>,
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
            let Some(kind) = WeaponKind::sam_for(t.class) else { continue };
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
            visible: self.config.spawn.visible,
        }
    }

    /// Feindliche Radarstellungen (M-020).
    pub fn radar(&self) -> &RadarNet {
        &self.radar
    }

    /// Lage gegenüber den feindlichen Radaren für die Anzeigen.
    fn threat_info(&self) -> ThreatInfo {
        let x = self.exposure();
        let mut worst: Option<ThreatSite> = None;
        for (k, s) in self.radar.sites.iter().enumerate() {
            let t = &self.targets[s.target];
            if !t.is_intact() {
                continue;
            }
            let c = t.center_ned();
            let at = DVec3::new(c[0], c[1], c[2]);
            let range_m = (at - x.position).length();
            let signal = x.signal(&s.spec, at);
            if signal <= 0.0 {
                continue;
            }
            let site = ThreatSite {
                site: k,
                target: s.target,
                name: t.name,
                kind: s.spec.kind,
                signal,
                alert: s.alert,
                range_m,
            };
            if worst.is_none_or(|w| site.signal > w.signal) {
                worst = Some(site);
            }
        }
        ThreatInfo {
            stealth: x.stealth(),
            worst,
            max_alert: self.radar.max_alert(),
            last_ping: self.radar.last_ping.map(|p| PingInfo {
                tick: p.tick,
                age_s: self.tick.saturating_sub(p.tick) as f64 * DT,
                signal: p.signal,
                stealth: p.stealth,
                hit: p.hit,
                kind: self.radar.sites[p.site].spec.kind,
            }),
            ping_point: self.radar.latest_ping_point(),
            sites: self
                .radar
                .sites
                .iter()
                .filter(|s| self.targets[s.target].is_intact())
                .map(|s| {
                    let t = &self.targets[s.target];
                    RadarSiteInfo {
                        target: s.target,
                        pos: [t.north, t.east],
                        range_m: s.spec.range_m,
                        kind: s.spec.kind,
                        alert: s.alert,
                    }
                })
                .collect(),
        }
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
        // Bei gewählter Kanone löst die Auslösetaste keine Schachtwaffe aus.
        if controls.has_weapon(weapon_flags::RELEASE) && !self.stores.gun_selected {
            let launch = launch!(self);
            let dropped = if self.aircraft.on_ground { 0.0 } else { self.stores.release(&launch) };
            self.aircraft.stores_kg = (self.aircraft.stores_kg - dropped).max(0.0);
        }
    }

    fn step_weapons(&mut self) {
        if self.stores.in_flight.is_empty() {
            return;
        }
        let painting: Vec<usize> = self.radar.sites.iter().filter(|s| s.painted).map(|s| s.target).collect();
        let mut world = weapons::World {
            terrain: &self.terrain,
            targets: &mut self.targets,
            tick: self.tick,
            dt: DT,
            aircraft: self.aircraft.position,
            designated: self.sensors.lock.filter(|l| l.optical).map(|l| l.target),
            flying: self.status == Status::Flying,
            painting: &painting,
        };
        // Die Aufschaltung bleibt auch auf einem zerstörten Ziel, bis der
        // Spieler ein neues wählt (Wracks sind dann nicht mehr anwählbar).
        self.stores.step(&mut world);
        // Raketentreffer: Folgen nur über die Bordsysteme (M-015).
        let hits: Vec<System> = self
            .stores
            .events
            .iter()
            .filter(|(t, _)| *t == self.tick)
            .filter_map(|(_, e)| match e {
                WeaponEvent::AircraftHit { systems, .. } => Some(*systems),
                _ => None,
            })
            .flatten()
            .collect();
        if !hits.is_empty() {
            for s in hits {
                self.systems.hit(s);
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

        let target = self.sensors.lock.and_then(|lock| {
            let t = self.targets.get(lock.target)?;
            let c = t.center_ned();
            let to_center = DVec3::new(c[0] - a.position.x, c[1] - a.position.y, c[2] - a.position.z);
            Some(TargetInfo {
                index: lock.target,
                name: t.name,
                class: t.class,
                civilian: t.civilian,
                role: t.role,
                optical: lock.optical,
                radar: lock.radar,
                range: to_center.length(),
                bearing: detmath::wrap_two_pi(detmath::atan2(to_center.y, to_center.x)),
                direction_body: a.attitude.inverse_rotate(to_center).normalize_or_zero(),
                offset_ned: DVec3::new(
                    t.north - a.position.x,
                    t.east - a.position.y,
                    -t.elevation - a.position.z,
                ),
                heading: t.heading,
                zone: self.launch_zone.filter(|(l, _)| *l == lock).map(|(_, z)| z),
                damage: t.damage,
                destroyed: !t.is_intact(),
            })
        });

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
            missile: airborne && self.stores.in_flight.iter().any(|w| w.owner.is_some() && w.chasing),
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
            thrust_n: aero.thrust,
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
            radar_on: self.sensors.radar_on,
            look: self.sensors.look,
            target,
            stores: StoresInfo {
                bays: self.stores.bays,
                selected: self.stores.selected,
                gun_selected: self.stores.gun_selected,
                door: self.stores.door,
                in_flight: self.stores.in_flight.len(),
                mass_kg: self.stores.mass_kg(),
            },
            weapon_messages: self.weapon_messages(),
            systems: self.systems,
            awacs: self.config.spawn.awacs,
            own_contacts: (0..self.targets.len())
                .filter(|&i| {
                    self.own_contacts.get(i).copied().unwrap_or(false) && self.targets[i].is_intact()
                })
                .collect(),
            bomb_aim: self.bomb_aim(),
            gun: GunInfo {
                rounds: self.gun.rounds,
                firing: self.gun.trigger,
                aim: self.gun_prediction.map(|(impact, _)| {
                    let to = impact - self.aircraft.position;
                    GunAim {
                        impact,
                        direction_body: self.aircraft.attitude.inverse_rotate(to).normalize_or_zero(),
                        range_m: to.length(),
                    }
                }),
            },
            threat: self.threat_info(),
            munitions: self
                .stores
                .in_flight
                .iter()
                .map(|w| MunitionInfo {
                    position: w.position,
                    velocity: w.velocity,
                    missile: w.kind.spec().cruise_mps > 0.0,
                    hostile: w.owner.is_some(),
                })
                .collect(),
            weapon_cam: self.stores.in_flight.iter().rev().find(|w| w.kind.has_camera()).map(|w| {
                let aim = w.aim_point(&self.targets).map(|p| p - w.position);
                WeaponCam {
                    id: w.id,
                    kind: w.kind,
                    position: w.position,
                    look: aim.unwrap_or(w.velocity).normalize_or_zero(),
                    tracking: w.target.zip(aim).map(|(i, d)| (i, d.length())),
                }
            }),
            destroyed_targets: (0..self.targets.len()).filter(|&i| !self.targets[i].is_intact()).collect(),
            warnings,
        }
    }

    /// Zielhilfe aus der zuletzt berechneten Aufschlagvorhersage.
    fn bomb_aim(&self) -> Option<BombAim> {
        let kind = self.stores.selected_weapon().filter(|k| !k.is_guided())?;
        let (impact, fall_time_s) = self.bomb_prediction?;
        let a = &self.aircraft;
        let to = impact - a.position;
        // Wo das Flugzeug beim Einschlag wäre (geradeaus weiter).
        let later = a.position + a.velocity * fall_time_s;
        let too_low = (later - impact).length() < kind.spec().frag_radius_m;
        let h = DVec3::new(a.velocity.x, a.velocity.y, 0.0).normalize_or_zero();
        let right = DVec3::new(-h.y, h.x, 0.0);
        let target_offset = self.sensors.lock.and_then(|l| {
            let t = self.targets.get(l.target).filter(|t| t.is_intact())?;
            let d = DVec3::new(t.north - impact.x, t.east - impact.y, 0.0);
            Some((d.dot(h), d.dot(right)))
        });
        Some(BombAim {
            impact,
            direction_body: a.attitude.inverse_rotate(to).normalize_or_zero(),
            fall_time_s,
            too_low,
            target_offset,
            lethal_radius_m: kind.spec().lethal_radius_m,
        })
    }

    /// Meldungen der letzten [`WEAPON_MESSAGE_S`] Sekunden, neueste zuletzt.
    fn weapon_messages(&self) -> Vec<WeaponMessage> {
        let name = |i: usize| self.targets[i].name;
        let mut all: Vec<(u64, WeaponMessage)> = self
            .stores
            .events
            .iter()
            .filter_map(|&(tick, e)| {
                let age_s = self.tick.saturating_sub(tick) as f64 * DT;
                if age_s > WEAPON_MESSAGE_S {
                    return None;
                }
                let (text, warning) = match e {
                    WeaponEvent::Released { kind, .. } => (format!("{} AWAY", kind.label()), false),
                    WeaponEvent::BayClosed => ("BAY CLOSED".to_string(), true),
                    WeaponEvent::BayFault => ("BAY FAULT".to_string(), true),
                    WeaponEvent::BayEmpty => ("BAY EMPTY".to_string(), true),
                    WeaponEvent::BankLimit => ("BANK LIMIT".to_string(), true),
                    WeaponEvent::Busy => ("WEAPONS BUSY".to_string(), true),
                    WeaponEvent::FragWarning => ("FRAG WARNING".to_string(), true),
                    WeaponEvent::Expired { kind, .. } => (format!("{} LOST", kind.label()), false),
                    WeaponEvent::SamLaunch { kind, site, .. } => {
                        (format!("MISSILE LAUNCH {} - {}", kind.label(), name(site)), true)
                    }
                    WeaponEvent::AircraftHit { kind, .. } => (format!("{} HIT", kind.label()), true),
                    WeaponEvent::Acquired { kind, target, .. } => {
                        (format!("{} LOCK {}", kind.label(), name(target)), self.targets[target].civilian)
                    }
                    WeaponEvent::LaserLost { kind, .. } => (format!("{} LASER LOST", kind.label()), true),
                    WeaponEvent::Impact { kind, result, .. } => match result {
                        ImpactResult::Destroyed { target: i, .. } if self.targets[i].civilian => {
                            (format!("CIVILIAN {} DESTROYED", name(i)), true)
                        }
                        ImpactResult::Destroyed { target: i, critical: true } => {
                            (format!("CRITICAL HIT {} DESTROYED", name(i)), false)
                        }
                        ImpactResult::Destroyed { target: i, .. } => {
                            (format!("{} DESTROYED", name(i)), false)
                        }
                        ImpactResult::Damaged(i) => (format!("{} DAMAGED", name(i)), false),
                        ImpactResult::Ineffective(i) => (format!("INEFFECTIVE ON {}", name(i)), true),
                        ImpactResult::Ground => (format!("{} MISSED", kind.label()), false),
                    },
                };
                Some((tick, WeaponMessage { text, warning, age_s }))
            })
            .chain(self.radar.events.iter().filter_map(|&(tick, e)| {
                let age_s = self.tick.saturating_sub(tick) as f64 * DT;
                if age_s > WEAPON_MESSAGE_S {
                    return None;
                }
                let site_name = |k: usize| self.targets[self.radar.sites[k].target].name;
                let (text, warning) = match e {
                    RadarEvent::Alert { site, level: 1 } => {
                        (format!("RADAR DETECTED - {}", site_name(site)), true)
                    }
                    RadarEvent::Alert { site, level: 2 } => (format!("TRACKED - {}", site_name(site)), true),
                    RadarEvent::Alert { site, .. } => (format!("RADAR ID - {}", site_name(site)), true),
                    RadarEvent::Lost { site } => (format!("{} LOST CONTACT", site_name(site)), false),
                    RadarEvent::Fire { .. } => return None,
                };
                Some((tick, WeaponMessage { text, warning, age_s }))
            }))
            .chain(self.gun.events.iter().filter_map(|&(tick, e)| {
                let age_s = self.tick.saturating_sub(tick) as f64 * DT;
                if age_s > WEAPON_MESSAGE_S {
                    return None;
                }
                let (text, warning) = match e {
                    GunEvent::Destroyed(i) if self.targets[i].civilian => {
                        (format!("CIVILIAN {} DESTROYED", name(i)), true)
                    }
                    GunEvent::Destroyed(i) => (format!("GUN {} DESTROYED", name(i)), false),
                    GunEvent::Empty => ("GUN EMPTY".to_string(), true),
                };
                Some((tick, WeaponMessage { text, warning, age_s }))
            }))
            .collect();
        // Waffen und Kanone zeitlich mischen (stabil: gleiche Ticks in Reihenfolge).
        all.sort_by_key(|(tick, _)| *tick);
        let mut out: Vec<WeaponMessage> = all.into_iter().map(|(_, m)| m).collect();
        let skip = out.len().saturating_sub(3);
        out.drain(..skip);
        out
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
        h.f64(a.stores_kg);
        let st = &self.stores;
        for b in &st.bays {
            h.u64(b.kind.map_or(99, |k| k as u64));
            h.u64(u64::from(b.count));
        }
        h.u64(st.selected as u64 | u64::from(st.gun_selected) << 8);
        for (kind, value) in self.systems.hash_parts() {
            h.u64(kind);
            h.f64(value);
        }
        let (ws, wi) = st.rng_state();
        h.u64(ws);
        h.u64(wi);
        h.f64(st.door);
        h.u64(u64::from(st.door_cmd));
        for w in &st.in_flight {
            h.u64(u64::from(w.id));
            for v in [w.position, w.velocity] {
                h.f64(v.x);
                h.f64(v.y);
                h.f64(v.z);
            }
            h.f64(w.age_s);
            h.u64(w.target.map_or(u64::MAX, |t| t as u64));
            h.u64(u64::from(w.tracking) | u64::from(w.searching) << 1 | u64::from(w.chasing) << 2);
            h.u64(w.owner.map_or(u64::MAX, |o| o as u64));
        }
        for s in &self.radar.sites {
            h.u64(u64::from(s.alert));
            h.u64(s.last_change_tick | u64::from(s.painted) << 63);
            if let Some(p) = s.ping_point {
                h.f64(p.x);
                h.f64(p.y);
                h.f64(p.z);
            }
        }
        let g = &self.gun;
        h.u64(u64::from(g.rounds) | u64::from(g.trigger) << 16);
        h.f64(g.pending());
        let (gs, gi) = g.rng_state();
        h.u64(gs);
        h.u64(gi);
        for r in &g.in_flight {
            for v in [r.position, r.velocity] {
                h.f64(v.x);
                h.f64(v.y);
                h.f64(v.z);
            }
            h.f64(r.age_s);
        }
        h.u64(u64::from(self.sensors.radar_on) | u64::from(self.sensors.night) << 1);
        h.u64(u64::from(self.sensors.look.code()));
        if let Some(l) = self.sensors.lock {
            h.u64(l.target as u64);
            h.u64(u64::from(l.optical) | u64::from(l.radar) << 1);
        }
        for t in &self.targets {
            h.u64(t.state as u64);
            h.f64(t.damage);
        }
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
