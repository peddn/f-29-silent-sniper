//! Kennwerte des Flugverhaltens: feste, deterministische Standardmanöver, die
//! jeweils eine Zahl liefern (Abrissfahrt, Höhenverlust im Abriss, Fahrtverlust
//! in der Kurve, Start, Landung …).
//!
//! Dient zum Abstimmen (`cargo run -p f29-core --example flight_report`) und
//! als Grundlage der Verhaltenstests (`tests/envelope.rs`), die die Kennwerte
//! gegen Toleranzbänder prüfen statt einzelne Flugbahnen. So sieht man bei
//! jeder Änderung auf einen Blick, was sie bewirkt, und Tests kippen nicht
//! wegen kleiner Verschiebungen chaotischer Flugbahnen.

use crate::atmosphere::{self, G0};
use crate::autopilot::{AIM_POINT_M, Approach, GLIDE_SLOPE};
use crate::controls::{ControlFrame, flags};
use crate::flight::{self, AircraftState, STALL_NOTICEABLE};
use crate::math::{DQuat, DVec3};
use crate::sim::{Spawn, SpawnKind};
use crate::{DT, Sim, SimConfig, Status, units};
use f29_data::{AircraftSpec, F29_SPEC};
use f29_terrain::AirfieldKind;

/// Startwert für alle Manöver auf Gelände.
const SEED: u64 = 0x0F29;
/// Treibstoff in den Manövern ohne Gelände (Anteil).
const FUEL: f64 = 0.6;

/// Ein Kennwert.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metric {
    /// Kurzname (stabil, für Tests).
    pub id: &'static str,
    /// Beschreibung des Manövers.
    pub label: &'static str,
    pub value: f64,
    pub unit: &'static str,
}

/// Alle Kennwerte in fester Reihenfolge.
pub fn report() -> Vec<Metric> {
    let spec = &F29_SPEC;
    let m = |id, label, value, unit| Metric { id, label, value, unit };
    let mass = spec.full_mass_kg();
    let mut out = vec![
        m(
            "vs_clean",
            "Abrissfahrt, volle Tanks, Meereshöhe",
            kt(spec.stall_speed_1g(mass, 1.225, 0.0)),
            "kt",
        ),
        m("vs_flaps", "… mit Klappen", kt(spec.stall_speed_1g(mass, 1.225, 1.0)), "kt"),
    ];
    for (id, label, flaps) in [
        ("sink_clean", "Durchsacken 30 s bei 1,2·Vs, 50 % Schub, Nase gehalten", 0.0),
        ("sink_flaps", "… mit Klappen", 1.0),
    ] {
        out.push(m(id, label, units::m_to_ft(slow_flight_height_change(1.2, flaps)), "ft"));
    }
    out.push(m(
        "sink_fast",
        "… bei 1,8·Vs ohne Klappen",
        units::m_to_ft(slow_flight_height_change(1.8, 0.0)),
        "ft",
    ));
    for (id, label, kind, flaps) in [
        ("stall_level_clean", "Höhenverlust: horizontal bis zum Abriss verzögert", StallEntry::Level, false),
        ("stall_level_flaps", "… mit Klappen", StallEntry::Level, true),
        ("stall_zoom_clean", "Höhenverlust: Abriss aus 40°-Steigflug", StallEntry::Zoom, false),
        ("stall_zoom_flaps", "… mit Klappen", StallEntry::Zoom, true),
        ("stall_turn_clean", "Höhenverlust: 60°-Kurve bei 250 kt voll gezogen", StallEntry::Turn, false),
    ] {
        out.push(m(id, label, units::m_to_ft(stall_height_loss(kind, flaps)), "ft"));
    }
    out.push(m(
        "stall_time_turn",
        "Zeit bis Abriss: 70°-Kurve, 185 kt, voll gezogen",
        time_to_stall(1.22, 0.0),
        "s",
    ));
    out.push(m("stall_time_level", "… waagerecht hochgezogen", time_to_stall(0.0, 0.0), "s"));
    out.push(m("stall_time_turn_flaps", "… 70°-Kurve mit Klappen", time_to_stall(1.22, 1.0), "s"));
    out.push(m("turn_speed_clean", "Fahrt nach 8 s am g-Begrenzer ab 450 kt", kt(hard_turn_speed(0)), "kt"));
    out.push(m("turn_speed_flaps", "… mit Klappen", kt(hard_turn_speed(flags::FLAPS)), "kt"));
    let release = takeoff(false, 0);
    out.push(m(
        "takeoff_release_stall",
        "Start voll gezogen, bei 15° losgelassen: Abriss (0/1)",
        release.stalled,
        "",
    ));
    out.push(m("takeoff_release_agl", "… Höhe nach 60 s", units::m_to_ft(release.final_agl), "ft"));
    let hold = takeoff(true, 0);
    out.push(m(
        "takeoff_hold_stall_agl",
        "Start voll gezogen gehalten: Höhe beim Abriss",
        units::m_to_ft(hold.stall_agl),
        "ft",
    ));
    let hold_flaps = takeoff(true, flags::FLAPS);
    out.push(m(
        "takeoff_hold_flaps_max_agl",
        "… mit Klappen: größte Höhe",
        units::m_to_ft(hold_flaps.max_agl),
        "ft",
    ));
    let (landed, sink) = autopilot_landing();
    out.push(m("landing_ok", "Lande-Autopilot auf der Heimatbasis (0/1)", landed, ""));
    out.push(m("landing_sink", "… Sinkrate beim Aufsetzen", units::mps_to_fpm(sink), "ft/min"));
    out
}

/// Sucht einen Kennwert.
pub fn get(report: &[Metric], id: &str) -> f64 {
    report.iter().find(|m| m.id == id).unwrap_or_else(|| panic!("Kennwert {id} fehlt")).value
}

fn kt(mps: f64) -> f64 {
    units::mps_to_kt(mps)
}

/// Getrimmter Horizontalflug ohne Gelände (Höhe 3000 m).
fn trimmed(spec: &AircraftSpec, speed: f64, flaps: f64, engine: f64) -> AircraftState {
    let mut st = AircraftState::level(0.0, 0.0, 3_000.0, 0.0, speed);
    st.fuel_kg = spec.fuel_capacity_kg * FUEL;
    st.flaps = flaps;
    st.engine = engine;
    let rho = atmosphere::isa(3_000.0).density;
    let qs = 0.5 * rho * speed * speed * spec.wing_area_m2;
    let cl = st.mass(spec) * G0 / qs - spec.flap_cl * flaps;
    let alpha = ((cl - spec.cl0) / spec.cl_alpha).clamp(0.0, spec.alpha_warning_rad);
    st.attitude = DQuat::from_euler(0.0, alpha, 0.0);
    st.velocity = DVec3::new(speed, 0.0, 0.0);
    st
}

fn vs_at_3000(spec: &AircraftSpec, flaps: f64) -> f64 {
    spec.stall_speed_1g(
        spec.empty_mass_kg + spec.fuel_capacity_kg * FUEL,
        atmosphere::isa(3_000.0).density,
        flaps,
    )
}

fn flap_flag(flaps: f64) -> u8 {
    if flaps > 0.0 { flags::FLAPS } else { 0 }
}

/// Höhenänderung (m) nach 30 s mit gehaltener Nase bei `factor` × Vs.
fn slow_flight_height_change(factor: f64, flaps: f64) -> f64 {
    let spec = &F29_SPEC;
    let mut st = trimmed(spec, factor * vs_at_3000(spec, flaps), flaps, 0.5);
    let c = ControlFrame::from_analog(0.0, 0.0, 0.0, 0.5, flap_flag(flaps));
    for _ in 0..ticks(30.0) {
        flight::step(spec, &mut st, &c, DVec3::ZERO, DT);
    }
    st.altitude() - 3_000.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StallEntry {
    /// Horizontal mit wenig Schub verzögern, Höhe halten.
    Level,
    /// Aus 300 kt auf 40° hochziehen und halten.
    Zoom,
    /// 60°-Kurve bei 250 kt, voll ziehen.
    Turn,
}

/// Höhenverlust (m) von Abrissbeginn bis zum Wieder-Steigen. Pilot: bei
/// merklichem Abriss loslassen, nach dem Abriss mit halbem Zug abfangen,
/// Vollgas.
fn stall_height_loss(entry: StallEntry, with_flaps: bool) -> f64 {
    let spec = &F29_SPEC;
    let flaps = if with_flaps { 1.0 } else { 0.0 };
    let f = flap_flag(flaps);
    let (speed, throttle) = match entry {
        StallEntry::Level => (units::kt_to_mps(180.0), 0.2),
        StallEntry::Zoom => (units::kt_to_mps(300.0), 0.7),
        StallEntry::Turn => (units::kt_to_mps(250.0), 0.7),
    };
    let mut st = trimmed(spec, speed, flaps, throttle);
    let (mut phase, mut h_stall, mut h_min) = (0, 0.0, f64::MAX);
    for _ in 0..ticks(120.0) {
        let (_, pitch, roll) = st.attitude.to_euler();
        let vs = -st.velocity.z;
        let level_wings = (-roll * 2.0).clamp(-1.0, 1.0);
        if phase == 0 && st.stall > STALL_NOTICEABLE {
            phase = 1;
            h_stall = st.altitude();
        }
        let (stick, aileron) = match phase {
            0 => match entry {
                StallEntry::Level => ((-vs * 0.15 + 0.1).clamp(-0.3, 1.0), level_wings),
                StallEntry::Zoom => {
                    let p = if pitch < 0.7 { 0.7 } else { ((0.7 - pitch) * 3.0).clamp(-0.3, 0.3) };
                    (p, level_wings)
                }
                StallEntry::Turn => {
                    let a = ((1.05 - roll) * 3.0).clamp(-1.0, 1.0);
                    (if roll > 0.9 { 1.0 } else { 0.0 }, a)
                }
            },
            1 => {
                if st.stall < 0.05 {
                    phase = 2;
                }
                (0.0, level_wings)
            }
            _ => {
                if vs > 0.0 {
                    return h_stall - h_min;
                }
                (0.5, level_wings)
            }
        };
        if phase >= 1 {
            h_min = h_min.min(st.altitude());
        }
        let thr = if phase >= 1 { 1.0 } else { throttle };
        flight::step(spec, &mut st, &ControlFrame::from_analog(stick, aileron, 0.0, thr, f), DVec3::ZERO, DT);
    }
    if phase == 0 { 0.0 } else { h_stall - h_min }
}

/// Zeit (s) bis zum merklichen Abriss bei vollem Zug in Querlage `bank` (rad),
/// ab 95 m/s, Vollgas. Unendlich, wenn innerhalb von 20 s kein Abriss.
fn time_to_stall(bank: f64, flaps: f64) -> f64 {
    let spec = &F29_SPEC;
    let f = flap_flag(flaps);
    let mut st = trimmed(spec, 95.0, flaps, 1.0);
    for i in 0..ticks(20.0) {
        let (_, _, roll) = st.attitude.to_euler();
        let aileron = ((bank - roll) * 3.0).clamp(-1.0, 1.0);
        let stick = if (bank - roll).abs() < 0.15 { 1.0 } else { 0.0 };
        flight::step(spec, &mut st, &ControlFrame::from_analog(stick, aileron, 0.0, 1.0, f), DVec3::ZERO, DT);
        if st.stall > STALL_NOTICEABLE {
            return i as f64 * DT;
        }
    }
    f64::INFINITY
}

/// Fahrt (m/s) nach 8 s voll gezogen in 80° Querlage ab 230 m/s, Vollgas.
fn hard_turn_speed(f: u8) -> f64 {
    let spec = &F29_SPEC;
    let mut st = trimmed(spec, 230.0, if f != 0 { 1.0 } else { 0.0 }, 1.0);
    for _ in 0..ticks(8.0) {
        let (_, _, roll) = st.attitude.to_euler();
        let aileron = ((80.0_f64.to_radians() - roll) * 3.0).clamp(-1.0, 1.0);
        flight::step(spec, &mut st, &ControlFrame::from_analog(1.0, aileron, 0.0, 1.0, f), DVec3::ZERO, DT);
    }
    st.velocity.length()
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TakeoffResult {
    /// 1, wenn die Strömung merklich abriss.
    stalled: f64,
    /// Radhöhe beim ersten Abriss (m), sonst unendlich.
    stall_agl: f64,
    max_agl: f64,
    final_agl: f64,
}

/// Start von der Heimatbasis mit Vollgas, Knüppel wie bei der Tastatur mit 3/s
/// auf voll gezogen. `hold`: gezogen halten, sonst bei 15° Nicklage loslassen.
fn takeoff(hold: bool, f: u8) -> TakeoffResult {
    let mut sim = Sim::new(SimConfig::runway(SEED));
    let (mut stick, mut released) = (0.0_f64, false);
    let mut r = TakeoffResult { stalled: 0.0, stall_agl: f64::INFINITY, max_agl: 0.0, final_agl: 0.0 };
    for _ in 0..ticks(60.0) {
        let fd = sim.flight_data();
        released |= !hold && !fd.on_ground && fd.pitch > 0.26;
        let target = if released { 0.0 } else { 1.0 };
        stick += (target - stick).clamp(-3.0 * DT, 3.0 * DT);
        sim.step(ControlFrame::from_analog(stick, 0.0, 0.0, 1.0, flags::GEAR | f));
        let agl = sim.flight_data().radar_altitude_m;
        r.max_agl = r.max_agl.max(agl);
        if r.stalled == 0.0 && sim.aircraft().is_stalled() {
            r.stalled = 1.0;
            r.stall_agl = agl;
        }
        if sim.status() != Status::Flying {
            break;
        }
    }
    r.final_agl = if sim.status() == Status::Flying { sim.flight_data().radar_altitude_m } else { 0.0 };
    r
}

/// Lande-Autopilot aus 8 km Endanflug: (gelandet und gestanden 0/1, Sinkrate
/// beim Aufsetzen m/s).
fn autopilot_landing() -> (f64, f64) {
    let base = Sim::new(SimConfig::new(SEED));
    let Some(rw) = base.terrain().runways().iter().find(|r| r.kind == AirfieldKind::Home).cloned() else {
        return (0.0, 0.0);
    };
    let dist = 8_000.0;
    let p = rw.world(-rw.length_m * 0.5 + AIM_POINT_M - dist, 0.0);
    let spawn = Spawn {
        kind: SpawnKind::Air,
        fuel: 0.5,
        north: p[0],
        east: p[1],
        altitude: rw.elevation_m + F29_SPEC.gear_height_m + dist * GLIDE_SLOPE,
        min_clearance: 0.0,
        heading: rw.heading_deg.to_radians(),
        speed: 95.0,
        throttle: 0.3,
        // Nach der Mission: Schächte leer.
        loadout: crate::weapons::Loadout::EMPTY,
        night: false,
        awacs: true,
    };
    let mut sim = Sim::new(SimConfig { seed: SEED, spawn });
    let mut ap = Approach::new(rw);
    let mut sink = 0.0;
    for _ in 0..ticks(300.0) {
        let fd = sim.flight_data();
        if let Some(td) = fd.touchdown {
            sink = td.sink_mps;
        }
        if fd.stopped_on_runway {
            return (1.0, sink);
        }
        if sim.status() != Status::Flying {
            return (0.0, sink);
        }
        let c = ap.control(sim.spec(), &fd, DT);
        sim.step(c);
    }
    (0.0, sink)
}

fn ticks(seconds: f64) -> usize {
    (seconds / DT).round() as usize
}
