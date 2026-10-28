//! Einfacher Autopilot (Höhe, Kurs, Geschwindigkeit).
//!
//! Dient als Testpilot für Flugtests und Golden Replays, als Demo-Modus im
//! Headless-Runner und später als Grundlage für KI-Flugzeuge. Er erzeugt
//! ganz normale [`ControlFrame`]s – genau wie ein menschlicher Pilot.

use crate::controls::{ControlFrame, flags};
use crate::detmath;
use crate::sim::FlightData;
use f29_data::AircraftSpec;

#[derive(Debug, Clone, PartialEq)]
pub struct Autopilot {
    pub target_altitude_m: f64,
    pub target_heading: f64,
    pub target_speed_mps: f64,
    /// Maximaler Querneigungswinkel, rad.
    pub max_bank: f64,
    throttle_integral: f64,
}

impl Autopilot {
    pub fn new(altitude_m: f64, heading: f64, speed_mps: f64) -> Self {
        Self {
            target_altitude_m: altitude_m,
            target_heading: heading,
            target_speed_mps: speed_mps,
            max_bank: 0.7,
            throttle_integral: 0.5,
        }
    }

    /// Berechnet die Steuereingabe für den nächsten Tick.
    pub fn control(&mut self, spec: &AircraftSpec, fd: &FlightData, dt: f64) -> ControlFrame {
        // Schub: PI-Regler auf die Geschwindigkeit.
        let speed_err = self.target_speed_mps - fd.tas_mps;
        self.throttle_integral = (self.throttle_integral + 0.002 * speed_err * dt).clamp(0.0, 1.0);
        let throttle = (self.throttle_integral + 0.03 * speed_err).clamp(0.0, 1.0);

        // Höhe → gewünschter Bahnwinkel → Nickknüppel.
        let gamma = if fd.tas_mps > 1.0 { detmath::asin(fd.vertical_speed_mps / fd.tas_mps) } else { 0.0 };
        let alt_err = self.target_altitude_m - fd.altitude_m;
        let mut gamma_cmd = (alt_err * 0.003).clamp(-0.2, 0.2);
        // Energieschutz: Steigen nur, solange genug Fahrt da ist. Das Flugzeug
        // hat keinen Abrissschutz – der Autopilot muss selbst aufpassen.
        if gamma_cmd > 0.0 {
            let energy = ((fd.tas_mps - 0.75 * self.target_speed_mps) / (0.25 * self.target_speed_mps))
                .clamp(0.0, 1.0);
            gamma_cmd *= energy;
        }
        // Nachdrücken nur sanft (negative g sind unangenehm und unnötig).
        let mut pitch = ((gamma_cmd - gamma) * 1.2 / spec.max_pitch_rate).clamp(-0.35, 1.0);
        // Abrissschutz: nie über den Warnwinkel ziehen.
        if fd.alpha > 0.85 * spec.alpha_warning_rad {
            pitch = pitch.min(-0.2);
        }

        // Kurs → Querneigung → Rollknüppel.
        let hdg_err = detmath::wrap_pi(self.target_heading - fd.heading);
        let bank_cmd = (hdg_err * 2.0).clamp(-self.max_bank, self.max_bank);
        let roll = ((bank_cmd - fd.roll) * 2.5 / spec.max_roll_rate).clamp(-1.0, 1.0);

        ControlFrame::from_analog(pitch, roll, 0.0, throttle, 0)
    }
}

/// Gleitwinkel des Anflugs (rad), 3°.
pub const GLIDE_SLOPE: f64 = 0.052_36;
/// Aufsetzpunkt hinter der Schwelle (m).
pub const AIM_POINT_M: f64 = 300.0;
/// Ab dieser Radhöhe über der Bahn fängt der Anflug-Autopilot ab (m).
const FLARE_HEIGHT_M: f64 = 12.0;

/// Anflug- und Lande-Autopilot auf eine Bahn: Kurs auf die verlängerte
/// Mittellinie, 3°-Gleitpfad, Fahrt 1,3 × Abrissfahrt, Abfangen, Aufsetzen,
/// Bremsen bis zum Stillstand. Testpilot für Landungen und Golden Replays.
#[derive(Debug, Clone, PartialEq)]
pub struct Approach {
    pub runway: f29_terrain::Runway,
    /// Fahrwerk ausfahren (für Tests der Bauchlandung abschaltbar).
    pub gear: bool,
    throttle_integral: f64,
}

impl Approach {
    pub fn new(runway: f29_terrain::Runway) -> Self {
        Self { runway, gear: true, throttle_integral: 0.3 }
    }

    pub fn control(&mut self, spec: &AircraftSpec, fd: &FlightData, dt: f64) -> ControlFrame {
        let rw = &self.runway;
        let (u, v) = rw.local(fd.position.x, fd.position.y);
        let gear_flag = if self.gear { flags::GEAR } else { 0 };
        if fd.on_ground {
            // Ausrollen: Leerlauf, Radbremse, geradeaus.
            let steer = (-v * 0.05).clamp(-0.3, 0.3);
            return ControlFrame::from_analog(
                0.0,
                steer,
                0.0,
                0.0,
                flags::AIRBRAKE | flags::FLAPS | gear_flag,
            );
        }
        let aim = -rw.length_m * 0.5 + AIM_POINT_M;
        let dist = aim - u;
        let wheels = fd.altitude_m - spec.gear_height_m;
        let height = wheels - rw.elevation_m;
        let flare = height < FLARE_HEIGHT_M;

        // Fahrt: 1,3-fache Abrissfahrt, kurz vor dem Aufsetzen Gas weg.
        let target_speed = 1.3 * fd.stall_speed_mps;
        let speed_err = target_speed - fd.tas_mps;
        self.throttle_integral = (self.throttle_integral + 0.01 * speed_err * dt).clamp(0.0, 1.0);
        let throttle =
            if height < 3.0 { 0.0 } else { (self.throttle_integral + 0.05 * speed_err).clamp(0.0, 1.0) };

        // Gleitpfad → Bahnwinkel → Nickknüppel.
        let gamma = if fd.tas_mps > 1.0 { detmath::asin(fd.vertical_speed_mps / fd.tas_mps) } else { 0.0 };
        let gamma_cmd = if flare {
            // Sinkrate mit der Höhe abbauen: bei 0 m noch etwa 0,5 m/s.
            -(0.5 + height * 0.1) / fd.tas_mps.max(30.0)
        } else {
            let target_h = dist.max(0.0) * GLIDE_SLOPE;
            (-GLIDE_SLOPE + (target_h - height) * 0.004).clamp(-0.12, 0.03)
        };
        // Bei Landefahrt wirken die Ruder schwach: kräftig regeln.
        let gain = if flare { 10.0 } else { 4.0 };
        let mut pitch = ((gamma_cmd - gamma) * gain / spec.max_pitch_rate).clamp(-0.4, 1.0);
        // Nie so hart ziehen, dass die Abrissfahrt über 90 % der Fahrt steigt.
        pitch = pitch.min(max_pull(spec, fd, 0.9));
        if fd.alpha > 0.9 * spec.alpha_warning_rad {
            pitch = pitch.min(0.0);
        }

        // Seitlich: Kurs auf die Mittellinie.
        let hdg = rw.heading_deg * (std::f64::consts::PI / 180.0);
        let hdg_cmd = hdg - (v * 0.004).clamp(-0.35, 0.35);
        let hdg_err = detmath::wrap_pi(hdg_cmd - fd.heading);
        let bank_cmd = if flare { 0.0 } else { (hdg_err * 2.0).clamp(-0.4, 0.4) };
        let roll = ((bank_cmd - fd.roll) * 2.5 / spec.max_roll_rate).clamp(-1.0, 1.0);

        ControlFrame::from_analog(pitch, roll, 0.0, throttle, flags::FLAPS | gear_flag)
    }
}

/// Größter Zug am Knüppel, bei dem die (g-abhängige) Abrissfahrt höchstens
/// `ratio` × aktuelle Fahrt erreicht.
pub fn max_pull(spec: &AircraftSpec, fd: &FlightData, ratio: f64) -> f64 {
    if fd.stall_speed_mps <= 0.0 {
        return 0.0;
    }
    let n_max = (ratio * fd.tas_mps / fd.stall_speed_mps).powi(2);
    let per_stick = crate::flight::stick_g_per_unit(spec, fd.tas_mps, fd.qbar_pa).max(1e-6);
    ((n_max - 1.0) / per_stick).clamp(0.0, 1.0)
}
