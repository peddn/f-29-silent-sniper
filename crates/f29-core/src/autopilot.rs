//! Einfacher Autopilot (Höhe, Kurs, Geschwindigkeit).
//!
//! Dient als Testpilot für Flugtests und Golden Replays, als Demo-Modus im
//! Headless-Runner und später als Grundlage für KI-Flugzeuge. Er erzeugt
//! ganz normale [`ControlFrame`]s – genau wie ein menschlicher Pilot.

use crate::controls::ControlFrame;
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
        let gamma_cmd = (alt_err * 0.003).clamp(-0.2, 0.2);
        // Nachdrücken nur sanft (negative g sind unangenehm und unnötig).
        let pitch = ((gamma_cmd - gamma) * 1.2 / spec.max_pitch_rate).clamp(-0.35, 1.0);

        // Kurs → Querneigung → Rollknüppel.
        let hdg_err = detmath::wrap_pi(self.target_heading - fd.heading);
        let bank_cmd = (hdg_err * 2.0).clamp(-self.max_bank, self.max_bank);
        let roll = ((bank_cmd - fd.roll) * 2.5 / spec.max_roll_rate).clamp(-1.0, 1.0);

        ControlFrame::from_analog(pitch, roll, 0.0, throttle, 0)
    }
}
