//! Flugmodell.
//!
//! **Translation:** volle Kräftebilanz (Auftrieb, Widerstand, Seitenkraft,
//! Schub, Gewicht) in 3D, integriert mit semi-implizitem Euler.
//!
//! **Rotation:** Fly-by-Wire mit Ratenkommando. Statt Momente, Trägheiten und
//! Ruderderivative zu modellieren, folgen die Drehraten einem kommandierten
//! Wert mit einer Zeitkonstante erster Ordnung. Die Flugsteuerung enthält
//! Ratenhaltung (Knüppel neutral = Lage bleibt), Turn-Koordination,
//! Gierdämpfer, g- und Anstellwinkelbegrenzer.
//!
//! Damit entstehen Anstellwinkel, Schiebewinkel, Lastvielfaches,
//! Energieverlust im Kurvenflug und Strömungsabriss physikalisch aus der
//! Translation, während die Steuerung gutmütig und gut testbar bleibt.
//!
//! Koordinatensysteme:
//! - Welt: NED (x = Nord, y = Ost, z = unten), Ursprung auf Meereshöhe.
//! - Körper: x = Nase, y = rechte Fläche, z = unten.

use crate::atmosphere::{self, G0, RHO0};
use crate::controls::{ControlFrame, flags};
use crate::detmath;
use crate::math::{DQuat, DVec3};
use f29_data::AircraftSpec;

/// Zustand eines Flugzeugs. Nur Primärgrößen – alles andere wird abgeleitet.
#[derive(Debug, Clone, PartialEq)]
pub struct AircraftState {
    /// Position in NED, Meter.
    pub position: DVec3,
    /// Geschwindigkeit in NED, m/s.
    pub velocity: DVec3,
    /// Lage: dreht Körper- in Weltachsen.
    pub attitude: DQuat,
    /// Drehraten (p, q, r) in Körperachsen, rad/s.
    pub body_rates: DVec3,
    /// Triebwerkszustand 0..1 (folgt dem Schubhebel verzögert).
    pub engine: f64,
    /// Luftbremse 0..1.
    pub airbrake: f64,
}

impl AircraftState {
    /// Horizontalflug in gegebener Höhe, Kurs und Geschwindigkeit.
    pub fn level(north: f64, east: f64, altitude: f64, heading: f64, speed: f64) -> Self {
        let attitude = DQuat::from_euler(heading, 0.0, 0.0);
        Self {
            position: DVec3::new(north, east, -altitude),
            velocity: attitude.rotate(DVec3::new(speed, 0.0, 0.0)),
            attitude,
            body_rates: DVec3::ZERO,
            engine: 0.6,
            airbrake: 0.0,
        }
    }

    pub fn altitude(&self) -> f64 {
        -self.position.z
    }

    pub fn is_finite(&self) -> bool {
        self.position.is_finite()
            && self.velocity.is_finite()
            && self.attitude.is_finite()
            && self.body_rates.is_finite()
            && self.engine.is_finite()
            && self.airbrake.is_finite()
    }
}

/// Aus dem Zustand abgeleitete aerodynamische Größen.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AeroData {
    pub tas: f64,
    pub mach: f64,
    pub qbar: f64,
    pub alpha: f64,
    pub beta: f64,
    /// Lastvielfaches in Körper-z (positiv = in den Sitz gedrückt).
    pub load_factor: f64,
    pub density: f64,
    pub cl: f64,
    pub cd: f64,
    pub thrust: f64,
    /// Ob der Anstellwinkel jenseits des Abrisswinkels liegt.
    pub stalled: bool,
}

/// Geschwindigkeit, unter der keine Aerodynamik gerechnet wird (Singularität).
const MIN_AERO_SPEED: f64 = 1.0;
/// Unterhalb dieser Geschwindigkeit sind Bahnhaltung und Koordination aus.
const MIN_FBW_SPEED: f64 = 40.0;
/// Verstärkung des Anstellwinkelbegrenzers, 1/s.
const ALPHA_LIMITER_GAIN: f64 = 3.0;
/// Ausfahrgeschwindigkeit der Luftbremse, 1/s.
const AIRBRAKE_RATE: f64 = 1.25;

/// Auftriebsbeiwert über dem Anstellwinkel.
pub fn lift_coefficient(spec: &AircraftSpec, alpha: f64) -> f64 {
    let stall = spec.alpha_stall_rad;
    if alpha.abs() <= stall {
        return spec.cl0 + spec.cl_alpha * alpha;
    }
    let sign = if alpha > 0.0 { 1.0 } else { -1.0 };
    let cl_peak = spec.cl0 + spec.cl_alpha * stall * sign;
    let excess = alpha.abs() - stall;
    let degraded = cl_peak * (1.0 - spec.post_stall_lift_drop * excess).max(0.0);
    // Jenseits des Abrisses bleibt eine Plattenströmung übrig (~ sin 2α).
    let plate = 0.8 * detmath::sin(2.0 * alpha);
    if sign > 0.0 { degraded.max(plate) } else { degraded.min(plate) }
}

/// Widerstandsbeiwert.
pub fn drag_coefficient(spec: &AircraftSpec, alpha: f64, cl: f64, airbrake: f64) -> f64 {
    let base = spec.cd0 + spec.induced_drag_k * cl * cl + spec.airbrake_cd * airbrake;
    let excess = alpha.abs() - spec.alpha_stall_rad;
    if excess > 0.0 {
        let s = detmath::sin(alpha);
        base + 1.2 * s * s * (excess / 0.1).min(1.0)
    } else {
        base
    }
}

/// Berechnet die aerodynamischen Größen für einen Zustand.
pub fn aero_data(spec: &AircraftSpec, s: &AircraftState) -> AeroData {
    let air = atmosphere::isa(s.altitude());
    let vb = s.attitude.inverse_rotate(s.velocity);
    let tas = vb.length();
    let thrust =
        spec.thrust_max_n * s.engine * detmath::powf(air.density / RHO0, spec.thrust_density_exponent);
    if tas < MIN_AERO_SPEED {
        return AeroData { tas, density: air.density, thrust, load_factor: 0.0, ..AeroData::default() };
    }
    let alpha = detmath::atan2(vb.z, vb.x);
    let beta = detmath::asin(vb.y / tas);
    let qbar = 0.5 * air.density * tas * tas;
    let cl = lift_coefficient(spec, alpha);
    let cd = drag_coefficient(spec, alpha, cl, s.airbrake);
    let ca = detmath::cos(alpha);
    // Normalkraft in Körper-z (Auftrieb und Widerstand projiziert).
    let fz = qbar * spec.wing_area_m2 * (-cl * ca - cd * (vb.z / tas));
    AeroData {
        tas,
        mach: tas / air.speed_of_sound,
        qbar,
        alpha,
        beta,
        load_factor: -fz / (spec.mass_kg * G0),
        density: air.density,
        cl,
        cd,
        thrust,
        stalled: alpha.abs() > spec.alpha_stall_rad,
    }
}

/// Kommandierte Drehraten der Flugsteuerung.
fn fbw_rate_command(spec: &AircraftSpec, s: &AircraftState, aero: &AeroData, c: &ControlFrame) -> DVec3 {
    let (_yaw, pitch, roll) = s.attitude.to_euler();
    let (sphi, cphi) = detmath::sin_cos(roll);
    let ctheta = detmath::cos(pitch);
    let v = aero.tas;

    // Ruderwirksamkeit sinkt mit dem Staudruck.
    let eff = (aero.qbar / spec.control_q_ref_pa).clamp(0.1, 1.0);

    // --- Nicken ---
    // Ratenkommando: Knüppel neutral ⇒ q = 0. Der Geradeausflug ist damit
    // ein stabiles Gleichgewicht (zu wenig Auftrieb ⇒ Bahn krümmt sich nach
    // unten ⇒ α wächst ⇒ Auftrieb wächst), es braucht keine Kompensation.
    let mut q_cmd = c.pitch_f() * spec.max_pitch_rate * eff;
    if v > MIN_FBW_SPEED {
        // g-Begrenzer über die Bahnkrümmung n ≈ V·q/g + cosθ·cosφ.
        let q_max = (spec.g_limit_pos - ctheta * cphi) * G0 / v;
        let q_min = (-spec.g_limit_neg - ctheta * cphi) * G0 / v;
        q_cmd = q_cmd.clamp(q_min, q_max);
    }
    // Anstellwinkelbegrenzer (positiv und – mit 60 % – negativ).
    // Vorausschauend: α̇ = q − γ̇, also darf q höchstens γ̇ plus eine
    // Rückführung auf den Grenzwert betragen. γ̇ wird aus dem aktuellen
    // Lastvielfachen geschätzt.
    if v > MIN_AERO_SPEED {
        let path_rate = (aero.load_factor - ctheta * cphi) * G0 / v;
        let a_hi = spec.alpha_limit_rad;
        let a_lo = -0.6 * spec.alpha_limit_rad;
        if aero.alpha > a_hi - 0.05 {
            q_cmd = q_cmd.min(path_rate + ALPHA_LIMITER_GAIN * (a_hi - aero.alpha));
        }
        if aero.alpha < a_lo + 0.05 {
            q_cmd = q_cmd.max(path_rate + ALPHA_LIMITER_GAIN * (a_lo - aero.alpha));
        }
    }

    // --- Rollen ---
    let p_cmd = c.roll_f() * spec.max_roll_rate * eff;

    // --- Gieren ---
    let mut r_cmd = c.yaw_f() * spec.max_yaw_rate * eff + spec.yaw_damper_gain * aero.beta;
    if v > MIN_FBW_SPEED {
        r_cmd += (G0 / v * sphi * ctheta).clamp(-0.3, 0.3);
    }

    DVec3::new(p_cmd, q_cmd, r_cmd)
}

/// Ein Integrationsschritt. Gibt die Aerodynamik des *Ausgangszustands* zurück.
pub fn step(spec: &AircraftSpec, s: &mut AircraftState, c: &ControlFrame, dt: f64) -> AeroData {
    let aero = aero_data(spec, s);

    // Triebwerk und Luftbremse
    let spool = (dt / spec.engine_spool_time_s).min(1.0);
    s.engine += (c.throttle_f() - s.engine) * spool;
    let brake_target = if c.has(flags::AIRBRAKE) { 1.0 } else { 0.0 };
    let max_move = AIRBRAKE_RATE * dt;
    s.airbrake += (brake_target - s.airbrake).clamp(-max_move, max_move);

    // Flugsteuerung: Raten folgen dem Kommando mit Verzögerung erster Ordnung.
    let cmd = fbw_rate_command(spec, s, &aero, c);
    let k = (dt / spec.rate_time_constant_s).min(1.0);
    s.body_rates += (cmd - s.body_rates) * k;

    // Kräfte in Körperachsen
    let mut force_body = DVec3::new(aero.thrust, 0.0, 0.0);
    if aero.tas >= MIN_AERO_SPEED {
        let vb = s.attitude.inverse_rotate(s.velocity);
        let v_hat = vb / aero.tas;
        let (sa, ca) = detmath::sin_cos(aero.alpha);
        let lift_dir = DVec3::new(sa, 0.0, -ca);
        let qs = aero.qbar * spec.wing_area_m2;
        force_body += lift_dir * (qs * aero.cl);
        force_body -= v_hat * (qs * aero.cd);
        force_body += DVec3::Y * (-qs * spec.cy_beta * aero.beta);
    }
    let force_world = s.attitude.rotate(force_body) + DVec3::new(0.0, 0.0, spec.mass_kg * G0);
    let accel = force_world / spec.mass_kg;

    // Semi-impliziter Euler: erst Geschwindigkeit, dann Position mit neuer Geschwindigkeit.
    s.velocity += accel * dt;
    s.position += s.velocity * dt;
    s.attitude = (s.attitude * DQuat::from_rotation_vector(s.body_rates * dt)).normalize();

    aero
}

#[cfg(test)]
mod tests {
    use super::*;
    use f29_data::F29_SPEC;

    #[test]
    fn lift_curve_shape() {
        let s = &F29_SPEC;
        let peak = lift_coefficient(s, s.alpha_stall_rad);
        assert!(lift_coefficient(s, 0.0) < lift_coefficient(s, 0.1));
        assert!(lift_coefficient(s, s.alpha_stall_rad + 0.1) < peak);
        assert!(lift_coefficient(s, -0.1) < 0.0);
        // stetig am Abrisspunkt
        let just_after = lift_coefficient(s, s.alpha_stall_rad + 1e-9);
        assert!((just_after - peak).abs() < 1e-6);
    }

    #[test]
    fn drag_grows_with_lift_and_airbrake() {
        let s = &F29_SPEC;
        assert!(drag_coefficient(s, 0.1, 0.5, 0.0) > drag_coefficient(s, 0.0, 0.0, 0.0));
        assert!(drag_coefficient(s, 0.0, 0.0, 1.0) > drag_coefficient(s, 0.0, 0.0, 0.0));
    }

    #[test]
    fn level_state_has_no_alpha_initially() {
        let st = AircraftState::level(0.0, 0.0, 3000.0, 1.0, 200.0);
        let a = aero_data(&F29_SPEC, &st);
        assert!(a.alpha.abs() < 1e-12);
        assert!(a.beta.abs() < 1e-12);
        assert!((a.tas - 200.0).abs() < 1e-9);
    }
}
