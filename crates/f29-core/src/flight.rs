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
//! **Treibstoff** ist echte Masse: Mit vollen Tanks ist das Flugzeug träge,
//! gegen Ende leichter. Der Verbrauch wächst mit dem Quadrat der
//! Triebwerksstellung; bei leeren Tanks geht das Triebwerk aus.
//!
//! **Am Boden** gibt es bewusst keine Federbeine, sondern einen einfachen
//! Rollmodus: Höhe auf die Bahn geklemmt, keine Querlage, Lenken über Quer-
//! und Seitenruder (auch auf der Stelle), Radbremse, Rotieren erst ab der
//! Abhebefahrt. Sobald der Auftrieb das Gewicht trägt, hebt das Flugzeug ab.
//! Beim Rotieren und in der Startphase danach steuert der Knüppel die
//! Nicklage (voll gezogen = [`TAKEOFF_PITCH`]) – so lässt sich mit gezogenem
//! Knüppel starten, ohne dass die Strömung direkt nach dem Abheben abreißt.
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
    /// Treibstoff in den Tanks, kg.
    pub fuel_kg: f64,
    /// Fahrwerk 0 = eingefahren … 1 = ausgefahren und verriegelt.
    pub gear: f64,
    /// Steht bzw. rollt auf dem Boden (Rollmodus statt Flugmodell).
    pub on_ground: bool,
    /// Startphase nach dem Abheben: Der Knüppel steuert die Nicklage statt
    /// der Nickrate (siehe [`TAKEOFF_PITCH`]). Endet, sobald der Knüppel
    /// losgelassen wird – danach gilt wieder die normale Ratensteuerung.
    pub takeoff_phase: bool,
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
            fuel_kg: f29_data::F29_SPEC.fuel_capacity_kg,
            gear: 0.0,
            on_ground: false,
            takeoff_phase: false,
        }
    }

    /// Aktuelle Masse (Leermasse + Treibstoff).
    pub fn mass(&self, spec: &AircraftSpec) -> f64 {
        spec.empty_mass_kg + self.fuel_kg.max(0.0)
    }

    /// Ob das Fahrwerk ganz ausgefahren und verriegelt ist.
    pub fn gear_down(&self) -> bool {
        self.gear >= GEAR_LOCKED
    }

    /// Ob das Triebwerk mangels Treibstoff ausgegangen ist.
    pub fn flamed_out(&self) -> bool {
        self.fuel_kg <= 0.0
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
            && self.fuel_kg.is_finite()
            && self.gear.is_finite()
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
/// Ab dieser Stellung gilt das Fahrwerk als ausgefahren und verriegelt.
pub const GEAR_LOCKED: f64 = 0.999;
/// Abhebefahrt als Vielfaches der 1-g-Abrissfahrt: erst darüber hebt
/// Ziehen am Boden die Nase.
pub const ROTATE_FACTOR: f64 = 1.1;
/// Nicklage bei voll gezogenem Knüppel beim Rotieren und in der Startphase (rad, 12°).
pub const TAKEOFF_PITCH: f64 = 0.209;
/// Größte Nickrate beim Rotieren und in der Startphase (rad/s).
const TAKEOFF_PITCH_RATE: f64 = 0.12;
/// Ab diesem Knüppelausschlag gilt der Knüppel als gezogen (Startphase).
const TAKEOFF_STICK: f64 = 0.05;
/// Rate, mit der die Nase am Boden ohne Zug wieder absinkt (rad/s).
const NOSE_SETTLE_RATE: f64 = 0.15;
/// Fahrt, bei der die Lenkrate am Boden auf die Hälfte gefallen ist (m/s).
const STEER_HALF_SPEED: f64 = 15.0;

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
    let gross = if s.flamed_out() {
        0.0
    } else {
        spec.thrust_max_n * s.engine * detmath::powf(air.density / RHO0, spec.thrust_density_exponent)
    };
    let thrust = gross - spec.idle_ram_drag * s.velocity.length() * (1.0 - s.engine);
    if tas < MIN_AERO_SPEED {
        return AeroData { tas, density: air.density, thrust, load_factor: 0.0, ..AeroData::default() };
    }
    let alpha = detmath::atan2(vb.z, vb.x);
    let beta = detmath::asin(vb.y / tas);
    let qbar = 0.5 * air.density * tas * tas;
    let mut cl = lift_coefficient(spec, alpha) + spec.flap_cl * s.flaps;
    let mut cd =
        drag_coefficient(spec, alpha, cl, s.airbrake) + spec.flap_cd * s.flaps + spec.gear_cd * s.gear;
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
        load_factor: -fz / (s.mass(spec) * G0),
        density: air.density,
        cl,
        cd,
        thrust,
        stall: s.stall,
    }
}

/// Treibstoffverbrauch (kg/s) bei Triebwerksstellung `engine` und Luftdichte
/// `density`: Grundverbrauch plus ein Anteil, der mit dem Quadrat der
/// Stellung wächst und mit der Luftdichte fällt (wie der Schub).
pub fn fuel_flow(spec: &AircraftSpec, engine: f64, density: f64) -> f64 {
    let e = engine.clamp(0.0, 1.0);
    let lapse = detmath::powf(density / RHO0, spec.thrust_density_exponent);
    spec.fuel_flow_idle_kgps + (spec.fuel_flow_max_kgps - spec.fuel_flow_idle_kgps) * e * e * lapse
}

/// Triebwerk, Luftbremse, Klappen, Fahrwerk und Treibstoff – im Flug wie am Boden.
fn update_systems(spec: &AircraftSpec, s: &mut AircraftState, c: &ControlFrame, aero: &AeroData, dt: f64) {
    // Triebwerk: ohne Treibstoff läuft es aus.
    let spool = (dt / spec.engine_spool_time_s).min(1.0);
    let target = if s.flamed_out() { 0.0 } else { c.throttle_f() };
    s.engine += (target - s.engine) * spool;
    if !s.flamed_out() {
        s.fuel_kg = (s.fuel_kg - fuel_flow(spec, s.engine, aero.density) * dt).max(0.0);
    }

    let brake_target = if c.has(flags::AIRBRAKE) { 1.0 } else { 0.0 };
    let max_move = AIRBRAKE_RATE * dt;
    s.airbrake += (brake_target - s.airbrake).clamp(-max_move, max_move);
    // Klappen; oberhalb der Höchstfahrt fahren sie selbsttätig ein.
    let flap_target = if c.has(flags::FLAPS) && aero.tas < spec.flap_max_speed_mps { 1.0 } else { 0.0 };
    let flap_move = spec.flap_rate * dt;
    s.flaps += (flap_target - s.flaps).clamp(-flap_move, flap_move);
    // Fahrwerk; am Boden lässt es sich nicht einfahren.
    let gear_target = if c.has(flags::GEAR) || s.on_ground { 1.0 } else { 0.0 };
    let gear_move = dt / spec.gear_time_s;
    s.gear += (gear_target - s.gear).clamp(-gear_move, gear_move);
}

/// Abhebefahrt: unterhalb hebt Ziehen am Boden die Nase nicht.
pub fn rotate_speed(spec: &AircraftSpec, s: &AircraftState, density: f64) -> f64 {
    spec.stall_speed_1g(s.mass(spec), density, s.flaps) * ROTATE_FACTOR
}

/// Ein Tick im Rollmodus. `elevation` ist die Bodenhöhe unter dem Flugzeug.
/// Liefert `true`, wenn das Flugzeug in diesem Tick abgehoben hat.
pub fn ground_step(
    spec: &AircraftSpec,
    s: &mut AircraftState,
    c: &ControlFrame,
    elevation: f64,
    dt: f64,
) -> bool {
    let aero = aero_data(spec, s);
    update_systems(spec, s, c, &aero, dt);
    s.stall = 0.0;

    let (yaw, pitch, _) = s.attitude.to_euler();
    let (sy, cy) = detmath::sin_cos(yaw);
    let fwd = DVec3::new(cy, sy, 0.0);
    let v = s.velocity.dot(fwd).max(0.0);
    let mass = s.mass(spec);
    let weight = mass * G0;
    let qs = aero.qbar * spec.wing_area_m2;
    let lift = qs * aero.cl;
    let wheel_load = (weight - lift).max(0.0);

    // Längs: Schub gegen Luftwiderstand, Rollreibung und Radbremse.
    let mut resist = qs * aero.cd + spec.rolling_friction * wheel_load;
    if c.has(flags::AIRBRAKE) {
        resist += spec.wheel_brake_decel * mass * (wheel_load / weight);
    }
    let push = aero.thrust;
    let accel = (push - resist) / mass;
    // Reibung hält ein stehendes Flugzeug fest, schiebt es aber nie rückwärts.
    let v_new = if v <= 0.0 && push <= resist { 0.0 } else { (v + accel * dt).max(0.0) };

    // Lenken über Quer- und Seitenruder, auch auf der Stelle (wie im Original).
    let steer = (c.roll_f() + c.yaw_f()).clamp(-1.0, 1.0);
    let r = steer * spec.ground_steer_rate / (1.0 + v_new / STEER_HALF_SPEED);

    // Nicken: erst ab der Abhebefahrt; der Knüppel wählt die Nicklage
    // (voll gezogen = TAKEOFF_PITCH). Darunter oder ohne Zug senkt sich die Nase.
    let v_rot = rotate_speed(spec, s, aero.density);
    let q = if v_new >= v_rot && c.pitch_f() > TAKEOFF_STICK {
        takeoff_pitch_rate(c, pitch)
    } else if pitch > 0.0 {
        -NOSE_SETTLE_RATE
    } else {
        0.0
    };
    let new_pitch = (pitch + q * dt).clamp(0.0, TAKEOFF_PITCH);
    let new_yaw = yaw + r * dt;

    s.attitude = DQuat::from_euler(new_yaw, new_pitch, 0.0);
    s.body_rates = DVec3::new(0.0, q, r);
    let (sy, cy) = detmath::sin_cos(new_yaw);
    s.velocity = DVec3::new(cy, sy, 0.0) * v_new;
    s.position += s.velocity * dt;
    s.position.z = -(elevation + spec.gear_height_m);

    // Abheben: Der Auftrieb trägt das Gewicht und die Nase ist oben.
    if lift > weight && new_pitch > 0.02 {
        s.on_ground = false;
        s.takeoff_phase = c.pitch_f() > TAKEOFF_STICK;
        return true;
    }
    false
}

/// Nickrate, die die Nicklage zum Knüppel-Sollwert führt (Startphase).
fn takeoff_pitch_rate(c: &ControlFrame, pitch: f64) -> f64 {
    let target = c.pitch_f().max(0.0) * TAKEOFF_PITCH;
    ((target - pitch) * 1.5).clamp(-TAKEOFF_PITCH_RATE, TAKEOFF_PITCH_RATE)
}

/// Kommandierte Drehraten der Flugsteuerung.
fn fbw_rate_command(spec: &AircraftSpec, s: &AircraftState, aero: &AeroData, c: &ControlFrame) -> DVec3 {
    let (_yaw, pitch, roll) = s.attitude.to_euler();
    let (sphi, cphi) = detmath::sin_cos(roll);
    let ctheta = detmath::cos(pitch);
    let v = aero.tas;

    // Ruderwirksamkeit sinkt mit dem Staudruck.
    let eff = control_effectiveness(spec, aero.qbar);

    // --- Nicken ---
    // Ratenkommando: Knüppel neutral ⇒ q = 0. Der Geradeausflug ist damit
    // ein stabiles Gleichgewicht (zu wenig Auftrieb ⇒ Bahn krümmt sich nach
    // unten ⇒ α wächst ⇒ Auftrieb wächst), es braucht keine Kompensation.
    // Im Abriss verlieren die Ruder ihre Wirkung (stetig mit der Abrissstärke).
    let stall_eff = 1.0 + (spec.stall_control_factor - 1.0) * aero.stall;
    let mut q_cmd = if s.takeoff_phase {
        takeoff_pitch_rate(c, pitch)
    } else {
        c.pitch_f() * spec.max_pitch_rate * eff * stall_eff
    };
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
///
/// Voller Zug fordert höchstens `stall_stick_g` zusätzliche g an – oder
/// weniger, wenn die Ruder bei geringem Staudruck gar nicht mehr Nickrate
/// erzeugen können (n = 1 + V·q/g). So lässt sich bei Landefahrt behutsam
/// abfangen, während harter Zug bei Kampffahrt unverändert früh abreißt.
pub fn demanded_load_factor(spec: &AircraftSpec, c: &ControlFrame, tas: f64, qbar: f64) -> f64 {
    1.0 + c.pitch_f().max(0.0) * stick_g_per_unit(spec, tas, qbar)
}

/// g-Last, gegen die die Abrissfahrt bestimmt wird: die Knüppelanforderung,
/// in der Startphase dagegen 1 g – dort hält die Steuerung nur sanft eine
/// Nicklage von höchstens 12°, der Knüppel fordert keine g-Last an.
pub fn stall_load_factor(spec: &AircraftSpec, s: &AircraftState, c: &ControlFrame, aero: &AeroData) -> f64 {
    if s.takeoff_phase { 1.0 } else { demanded_load_factor(spec, c, aero.tas, aero.qbar) }
}

/// Zusätzliche g-Anforderung pro voller Knüppelauslenkung.
pub fn stick_g_per_unit(spec: &AircraftSpec, tas: f64, qbar: f64) -> f64 {
    let rate_g = tas * spec.max_pitch_rate * control_effectiveness(spec, qbar) / G0;
    rate_g.min(spec.stall_stick_g)
}

/// Ruderwirksamkeit 0,1..1, steigt mit dem Staudruck.
pub fn control_effectiveness(spec: &AircraftSpec, qbar: f64) -> f64 {
    (qbar / spec.control_q_ref_pa).clamp(0.1, 1.0)
}

/// Abrissgeschwindigkeit bei Luftdichte `density` und g-Last `n` (m/s).
pub fn stall_speed(spec: &AircraftSpec, mass_kg: f64, density: f64, n: f64, flaps: f64) -> f64 {
    spec.stall_speed_1g(mass_kg, density, flaps) * n.max(1.0).sqrt()
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
    let mass = s.mass(spec);
    update_systems(spec, s, c, &aero, dt);
    // Startphase endet beim Loslassen des Knüppels (kein Sprung in der
    // Abrissanforderung, solange er gezogen bleibt).
    if s.takeoff_phase && c.pitch_f() <= TAKEOFF_STICK {
        s.takeoff_phase = false;
    }

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
            force_body += to_nose * (mass * aero.tas * STALL_PATH_ALIGN * s.stall);
        }
    }
    let force_world = s.attitude.rotate(force_body) + DVec3::new(0.0, 0.0, mass * G0);
    let accel = force_world / mass;

    // Semi-impliziter Euler: erst Geschwindigkeit, dann Position mit neuer Geschwindigkeit.
    s.velocity += accel * dt;
    s.position += s.velocity * dt;
    s.attitude = (s.attitude * DQuat::from_rotation_vector(s.body_rates * dt)).normalize();

    // Abrissstärke folgt stetig dem Fahrtdefizit gegenüber der (g-abhängigen)
    // Abrissgeschwindigkeit – kein Schaltzustand, keine Hysterese. Zusätzlich
    // wächst sie jenseits des Abrisswinkels mit dem Anstellwinkel, damit auch
    // knapp über der Abrissgeschwindigkeit kein absurder Anstellwinkel entsteht.
    let target = if aero.tas >= MIN_AERO_SPEED {
        let n = stall_load_factor(spec, s, c, &aero);
        let by_speed = stall_target(spec, stall_speed(spec, mass, aero.density, n, s.flaps), aero.tas);
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
        let vs = spec.stall_speed_1g(spec.full_mass_kg(), rho, 0.0);
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
        let v = spec.stall_speed_1g(spec.full_mass_kg(), rho, 0.0) * 1.4;
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
    fn fuel_burn_grows_quadratically_and_flames_out() {
        let spec = &F29_SPEC;
        let idle = fuel_flow(spec, 0.0, RHO0);
        let half = fuel_flow(spec, 0.5, RHO0);
        let full = fuel_flow(spec, 1.0, RHO0);
        assert!((idle - spec.fuel_flow_idle_kgps).abs() < 1e-12);
        assert!((full - spec.fuel_flow_max_kgps).abs() < 1e-9);
        // quadratisch: halbe Stellung = Grundverbrauch + ein Viertel des Zuwachses
        assert!((half - (idle + 0.25 * (full - idle))).abs() < 1e-9);
        // in der Höhe weniger
        assert!(fuel_flow(spec, 1.0, 0.4) < full * 0.6);

        let mut st = AircraftState::level(0.0, 0.0, 5000.0, 0.0, 200.0);
        st.fuel_kg = 2.0;
        run(&mut st, ControlFrame::from_analog(0.0, 0.0, 0.0, 1.0, 0), 3.0);
        assert!(st.flamed_out());
        assert_eq!(aero_data(spec, &st).thrust.max(0.0), 0.0, "ohne Treibstoff kein Schub");
    }

    #[test]
    fn lighter_aircraft_stalls_slower() {
        let spec = &F29_SPEC;
        let full = stall_speed(spec, spec.full_mass_kg(), RHO0, 1.0, 0.0);
        let empty = stall_speed(spec, spec.empty_mass_kg, RHO0, 1.0, 0.0);
        assert!(empty < 0.9 * full, "{empty} vs {full}");
    }

    fn on_runway() -> AircraftState {
        let mut st = AircraftState::level(0.0, 0.0, 100.0 + F29_SPEC.gear_height_m, 0.0, 0.0);
        st.on_ground = true;
        st.gear = 1.0;
        st.engine = 0.0;
        st
    }

    fn roll(st: &mut AircraftState, c: ControlFrame, seconds: f64) -> bool {
        for _ in 0..(seconds * 120.0) as usize {
            if ground_step(&F29_SPEC, st, &c, 100.0, 1.0 / 120.0) {
                return true;
            }
        }
        false
    }

    #[test]
    fn takeoff_roll_needs_rotation_speed() {
        let mut st = on_runway();
        let full = ControlFrame::from_analog(1.0, 0.0, 0.0, 1.0, flags::GEAR | flags::FLAPS);
        // Volles Ziehen von Anfang an: Die Nase hebt sich erst ab der Abhebefahrt.
        let mut lifted = false;
        let mut dist = 0.0;
        for _ in 0..(60.0 * 120.0) as usize {
            let before = st.position;
            let (_, pitch, _) = st.attitude.to_euler();
            let v = st.velocity.length();
            if v < 0.9 * rotate_speed(&F29_SPEC, &st, RHO0) {
                assert!(pitch < 1e-9, "Nase hebt sich bei {v} m/s");
            }
            lifted = ground_step(&F29_SPEC, &mut st, &full, 100.0, 1.0 / 120.0);
            dist += (st.position - before).length();
            if lifted {
                break;
            }
        }
        assert!(lifted, "kein Abheben");
        assert!((500.0..1_400.0).contains(&dist), "Startrollstrecke {dist} m");
        assert!(st.attitude.to_euler().2.abs() < 1e-12, "am Boden keine Querlage");
    }

    #[test]
    fn brakes_stop_and_steering_turns_on_the_spot() {
        let mut st = on_runway();
        st.velocity = DVec3::new(70.0, 0.0, 0.0);
        let brake = ControlFrame::from_analog(0.0, 0.0, 0.0, 0.0, flags::GEAR | flags::AIRBRAKE);
        let x0 = st.position.x;
        roll(&mut st, brake, 30.0);
        let dist = st.position.x - x0;
        assert!(st.velocity.length() < 1e-9, "steht nicht");
        assert!((300.0..900.0).contains(&dist), "Bremsweg {dist} m");
        // Auf der Stelle wenden: 180° in wenigen Sekunden, ohne sich zu bewegen
        let turn = ControlFrame::from_analog(0.0, 1.0, 0.0, 0.0, flags::GEAR | flags::AIRBRAKE);
        let p0 = st.position;
        roll(&mut st, turn, 6.0);
        let hdg = st.attitude.to_euler().0;
        assert!(detmath::wrap_pi(hdg).abs() > 2.5, "Kurs nach Wenden {hdg}");
        assert!((st.position - p0).length() < 1e-6);
        // Im Leerlauf bleibt es stehen, mit etwas Schub rollt es an.
        roll(&mut st, ControlFrame::from_analog(0.0, 0.0, 0.0, 0.0, flags::GEAR), 3.0);
        assert!(st.velocity.length() < 1e-9);
        roll(&mut st, ControlFrame::from_analog(0.0, 0.0, 0.0, 0.25, flags::GEAR), 10.0);
        let v = st.velocity.length();
        assert!(v > 3.0 && v < 40.0, "Rollen mit 25 % Schub: {v} m/s");
    }

    #[test]
    fn gear_cannot_retract_on_ground_and_takes_time() {
        let mut st = on_runway();
        roll(&mut st, ControlFrame::from_analog(0.0, 0.0, 0.0, 0.0, 0), 2.0);
        assert_eq!(st.gear, 1.0);
        let mut air = AircraftState::level(0.0, 0.0, 3000.0, 0.0, 150.0);
        run(&mut air, ControlFrame::from_analog(0.0, 0.0, 0.0, 0.6, flags::GEAR), 2.5);
        assert!(air.gear > 0.3 && air.gear < 0.7, "{}", air.gear);
        run(&mut air, ControlFrame::from_analog(0.0, 0.0, 0.0, 0.6, flags::GEAR), 3.0);
        assert!(air.gear_down());
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
