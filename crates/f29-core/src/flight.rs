//! Flugmodell.
//!
//! **Translation:** volle Kräftebilanz (Auftrieb, Widerstand, Seitenkraft,
//! Schub, Gewicht) in 3D, integriert mit semi-implizitem Euler.
//!
//! **Rotation:** Fly-by-Wire mit Ratenkommando. Statt Momente, Trägheiten und
//! Ruderderivative zu modellieren, folgen die Drehraten einem kommandierten
//! Wert mit einer Zeitkonstante erster Ordnung. Die Flugsteuerung enthält
//! Ratenhaltung (Knüppel neutral = Lage bleibt), Turn-Koordination,
//! Gierdämpfer und einen g-Begrenzer – aber bewusst **keinen**
//! Anstellwinkelbegrenzer.
//!
//! **Strömungsabriss** ist eine stetige Größe 0..1, kein Schaltzustand. Sie
//! folgt dem Fahrtdefizit gegenüber der Abrissgeschwindigkeit
//! `v_s = v_s1g(ρ) · √n_angefordert` – je höher und je härter gezogen, desto
//! früher. Mit der Abrissstärke wachsen Auftriebs- und Ruderverlust und ein
//! Abkippen der Nase Richtung senkrecht nach unten. Im Sturz kommt die Fahrt
//! zurück, die Abrissstärke sinkt stetig, und das Flugzeug lässt sich langsam
//! abfangen; die Nickwirkung wächst mit der Fahrt.
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
    /// Abrissstärke 0..1 (geglättet aus dem Fahrtdefizit, siehe Moduldoku).
    pub stall: f64,
    /// Landeklappen 0..1.
    pub flaps: f64,
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
            stall: 0.0,
            flaps: 0.0,
        }
    }

    /// Ob ein merklicher Strömungsabriss vorliegt.
    pub fn is_stalled(&self) -> bool {
        self.stall > STALL_NOTICEABLE
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
            && self.stall.is_finite()
            && self.flaps.is_finite()
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
    /// Abrissstärke 0..1 (aus [`AircraftState::stall`]).
    pub stall: f64,
}

/// Geschwindigkeit, unter der keine Aerodynamik gerechnet wird (Singularität).
const MIN_AERO_SPEED: f64 = 1.0;
/// Unterhalb dieser Geschwindigkeit sind Bahnhaltung und Koordination aus.
const MIN_FBW_SPEED: f64 = 40.0;
/// Wie schnell sich die Flugbahn bei vollem Abriss zur Nase dreht (1/s).
/// Das Flugzeug fällt dadurch der Nase nach, statt flach durchzusacken.
const STALL_PATH_ALIGN: f64 = 1.6;
/// Anstellwinkel jenseits des Abrisswinkels, bei dem der Abriss voll ist (rad).
const STALL_ALPHA_FULL: f64 = 0.15;
/// Ab diesem Winkel zur Senkrechten (rad) kippt die Nase im Abriss nicht weiter.
const STALL_DIVE_CONE: f64 = 0.2;
/// Zusätzlicher Widerstandsbeiwert bei vollem Abriss.
const STALL_EXTRA_DRAG: f64 = 0.08;
/// Abrissstärke, ab der ein Abriss als „merklich“ gilt (Anzeige, Tests).
pub const STALL_NOTICEABLE: f64 = 0.1;
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
    let plate = 0.5 * detmath::sin(2.0 * alpha);
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
    // Nettoschub: Bruttoschub minus Staudruckwiderstand des Triebwerks, der bei
    // niedriger Drehzahl am größten ist – Gas wegnehmen bremst dadurch spürbar.
    let gross =
        spec.thrust_max_n * s.engine * detmath::powf(air.density / RHO0, spec.thrust_density_exponent);
    let thrust = gross - spec.idle_ram_drag * s.velocity.length() * (1.0 - s.engine);
    if tas < MIN_AERO_SPEED {
        return AeroData { tas, density: air.density, thrust, load_factor: 0.0, ..AeroData::default() };
    }
    let alpha = detmath::atan2(vb.z, vb.x);
    let beta = detmath::asin(vb.y / tas);
    let qbar = 0.5 * air.density * tas * tas;
    let mut cl = lift_coefficient(spec, alpha) + spec.flap_cl * s.flaps;
    let mut cd = drag_coefficient(spec, alpha, cl, s.airbrake) + spec.flap_cd * s.flaps;
    if s.stall > 0.0 {
        // Abgerissene Strömung: Auftrieb bricht ein, Widerstand steigt.
        cl *= 1.0 + (spec.stall_lift_factor - 1.0) * s.stall;
        cd += STALL_EXTRA_DRAG * s.stall;
    }
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
        stall: s.stall,
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
    // Im Abriss verlieren die Ruder ihre Wirkung (stetig mit der Abrissstärke).
    let stall_eff = 1.0 + (spec.stall_control_factor - 1.0) * aero.stall;
    let mut q_cmd = c.pitch_f() * spec.max_pitch_rate * eff * stall_eff;
    if v > MIN_FBW_SPEED {
        // g-Begrenzer über die Bahnkrümmung n ≈ V·q/g + cosθ·cosφ.
        let q_max = (spec.g_limit_pos - ctheta * cphi) * G0 / v;
        let q_min = (-spec.g_limit_neg - ctheta * cphi) * G0 / v;
        q_cmd = q_cmd.clamp(q_min, q_max);
    }
    // --- Rollen ---
    let p_cmd = c.roll_f() * spec.max_roll_rate * eff * stall_eff;

    // --- Gieren ---
    let mut r_cmd = c.yaw_f() * spec.max_yaw_rate * eff + spec.yaw_damper_gain * aero.beta;
    if v > MIN_FBW_SPEED && aero.stall < 0.5 {
        r_cmd += (G0 / v * sphi * ctheta).clamp(-0.3, 0.3);
    }

    DVec3::new(p_cmd, q_cmd, r_cmd) + stall_departure_rates(spec, s)
}

/// Vom Piloten angeforderte g-Last (aus dem Knüppel, nicht aus dem tatsächlich
/// erreichten Auftrieb – sonst würde der Auftriebsverlust im Abriss die
/// Abrissgeschwindigkeit sofort wieder senken und es entstünde ein Pendeln).
pub fn demanded_load_factor(spec: &AircraftSpec, c: &ControlFrame) -> f64 {
    1.0 + c.pitch_f().max(0.0) * spec.stall_stick_g
}

/// Abrissgeschwindigkeit bei Luftdichte `density` und g-Last `n` (m/s).
pub fn stall_speed(spec: &AircraftSpec, density: f64, n: f64, flaps: f64) -> f64 {
    spec.stall_speed_1g(density, flaps) * n.max(1.0).sqrt()
}

/// Flugbahnwinkel γ (rad, positiv = steigen).
pub fn flight_path_angle(s: &AircraftState) -> f64 {
    let v = s.velocity.length();
    if v < MIN_AERO_SPEED { 0.0 } else { detmath::asin(-s.velocity.z / v) }
}

/// Ziel-Abrissstärke 0..1 aus dem relativen Fahrtdefizit.
fn stall_target(spec: &AircraftSpec, v_stall: f64, tas: f64) -> f64 {
    ((v_stall - tas) / (spec.stall_deficit_full * v_stall)).clamp(0.0, 1.0)
}

/// Abkippen im Abriss: Drehung, die die Nase aus jeder Lage Richtung
/// senkrecht nach unten bringt (über Nicken und Gieren, Rollen bleibt frei),
/// proportional zur Abrissstärke.
fn stall_departure_rates(spec: &AircraftSpec, s: &AircraftState) -> DVec3 {
    if s.stall <= 0.0 {
        return DVec3::ZERO;
    }
    // Schwerkraftrichtung in Körperachsen
    let g = s.attitude.inverse_rotate(DVec3::Z);
    // Winkel zwischen Nase (Körper-x) und der Senkrechten nach unten
    let angle = detmath::acos(g.x);
    if angle <= STALL_DIVE_CONE {
        return DVec3::ZERO;
    }
    // Drehachse Nase × unten = (0, −g.z, g.y); bei senkrecht hochstehender
    // Nase ist sie unbestimmt – dann einfach über die Nase abkippen.
    let axis = DVec3::new(0.0, -g.z, g.y);
    let len = axis.length();
    let dir = if len > 0.05 { axis / len } else { DVec3::new(0.0, -1.0, 0.0) };
    // Weich auslaufen, wenn die Nase fast senkrecht steht
    let fade = ((angle - STALL_DIVE_CONE) / 0.4).min(1.0);
    dir * (spec.stall_pitch_break * fade * s.stall)
}

/// Ein Integrationsschritt. `gust` sind Störungen der Drehraten (rad/s, z. B.
/// Turbulenz). Gibt die Aerodynamik des *Ausgangszustands* zurück.
pub fn step(spec: &AircraftSpec, s: &mut AircraftState, c: &ControlFrame, gust: DVec3, dt: f64) -> AeroData {
    let aero = aero_data(spec, s);

    // Triebwerk und Luftbremse
    let spool = (dt / spec.engine_spool_time_s).min(1.0);
    s.engine += (c.throttle_f() - s.engine) * spool;
    let brake_target = if c.has(flags::AIRBRAKE) { 1.0 } else { 0.0 };
    let max_move = AIRBRAKE_RATE * dt;
    s.airbrake += (brake_target - s.airbrake).clamp(-max_move, max_move);
    // Klappen; oberhalb der Höchstfahrt fahren sie selbsttätig ein.
    let flap_target = if c.has(flags::FLAPS) && aero.tas < spec.flap_max_speed_mps { 1.0 } else { 0.0 };
    let flap_move = spec.flap_rate * dt;
    s.flaps += (flap_target - s.flaps).clamp(-flap_move, flap_move);

    // Flugsteuerung: Raten folgen dem Kommando mit Verzögerung erster Ordnung.
    let cmd = fbw_rate_command(spec, s, &aero, c) + gust;
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
        if s.stall > 0.0 {
            // Flugbahn zur Nase drehen: Kraft senkrecht zur Geschwindigkeit,
            // ändert also nur die Richtung, nicht den Betrag der Fahrt.
            let to_nose = DVec3::X - v_hat * v_hat.x;
            force_body += to_nose * (spec.mass_kg * aero.tas * STALL_PATH_ALIGN * s.stall);
        }
    }
    let force_world = s.attitude.rotate(force_body) + DVec3::new(0.0, 0.0, spec.mass_kg * G0);
    let accel = force_world / spec.mass_kg;

    // Semi-impliziter Euler: erst Geschwindigkeit, dann Position mit neuer Geschwindigkeit.
    s.velocity += accel * dt;
    s.position += s.velocity * dt;
    s.attitude = (s.attitude * DQuat::from_rotation_vector(s.body_rates * dt)).normalize();

    // Abrissstärke folgt stetig dem Fahrtdefizit gegenüber der (g-abhängigen)
    // Abrissgeschwindigkeit – kein Schaltzustand, keine Hysterese. Zusätzlich
    // wächst sie jenseits des Abrisswinkels mit dem Anstellwinkel, damit auch
    // knapp über der Abrissgeschwindigkeit kein absurder Anstellwinkel entsteht.
    let target = if aero.tas >= MIN_AERO_SPEED {
        let n = demanded_load_factor(spec, c);
        let by_speed = stall_target(spec, stall_speed(spec, aero.density, n, s.flaps), aero.tas);
        let by_alpha = ((aero.alpha.abs() - spec.alpha_stall_rad) / STALL_ALPHA_FULL).clamp(0.0, 1.0);
        by_speed.max(by_alpha)
    } else {
        1.0
    };
    s.stall += (target - s.stall) * (dt / spec.stall_time_constant_s).min(1.0);

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

    /// Simuliert `seconds` mit fester Eingabe und liefert den Zustand.
    fn run(st: &mut AircraftState, c: ControlFrame, seconds: f64) {
        for _ in 0..(seconds * 120.0) as usize {
            step(&F29_SPEC, st, &c, DVec3::ZERO, 1.0 / 120.0);
        }
    }

    #[test]
    fn stall_is_continuous_and_follows_speed_deficit() {
        let spec = &F29_SPEC;
        let rho = atmosphere::isa(3000.0).density;
        let vs = spec.stall_speed_1g(rho, 0.0);
        // Deutlich über der Abrissgeschwindigkeit: kein Abriss
        let mut fast = AircraftState::level(0.0, 0.0, 3000.0, 0.0, vs * 1.6);
        run(&mut fast, ControlFrame::default(), 0.5);
        assert!(fast.stall < 1e-9);
        // Darunter: Abriss baut sich stetig auf (keine Sprünge zwischen Ticks)
        let mut slow = AircraftState::level(0.0, 0.0, 3000.0, 0.0, vs * 0.8);
        let mut last = slow.stall;
        for _ in 0..60 {
            step(spec, &mut slow, &ControlFrame::default(), DVec3::ZERO, 1.0 / 120.0);
            assert!((slow.stall - last).abs() < 0.05, "Sprung in der Abrissstärke");
            last = slow.stall;
        }
        assert!(slow.stall > 0.3);
    }

    #[test]
    fn pulling_raises_stall_speed() {
        let spec = &F29_SPEC;
        let rho = atmosphere::isa(3000.0).density;
        let v = spec.stall_speed_1g(rho, 0.0) * 1.4;
        // Ohne Ziehen kein Abriss, mit vollem Ziehen schon (beschleunigter Abriss)
        let mut neutral = AircraftState::level(0.0, 0.0, 3000.0, 0.0, v);
        run(&mut neutral, ControlFrame::default(), 0.5);
        let mut pulled = AircraftState::level(0.0, 0.0, 3000.0, 0.0, v);
        run(&mut pulled, ControlFrame::from_analog(1.0, 0.0, 0.0, 0.6, 0), 0.5);
        assert!(neutral.stall < 0.05, "neutral: {}", neutral.stall);
        assert!(pulled.stall > neutral.stall + 0.1, "gezogen: {}", pulled.stall);
    }

    #[test]
    fn stalled_lift_collapses() {
        let spec = &F29_SPEC;
        let mut st = AircraftState::level(0.0, 0.0, 5000.0, 0.0, 150.0);
        st.attitude = DQuat::from_euler(0.0, 0.2, 0.0);
        let clean = aero_data(spec, &st).cl;
        st.stall = 1.0;
        let stalled = aero_data(spec, &st).cl;
        assert!((stalled - clean * spec.stall_lift_factor).abs() < 1e-9);
        st.stall = 0.5;
        let half = aero_data(spec, &st).cl;
        assert!(half < clean && half > stalled, "Auftrieb muss stetig mit der Abrissstärke fallen");
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
