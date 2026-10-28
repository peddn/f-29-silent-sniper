use crate::ValidationError;

/// Parameter eines Flugzeugtyps für das Flugmodell in `f29-core`.
///
/// Einheiten: SI (kg, m, N, s, rad). Winkelgeschwindigkeiten in rad/s.
#[derive(Debug, Clone, PartialEq)]
pub struct AircraftSpec {
    /// Anzeigename.
    pub name: &'static str,

    // --- Masse und Geometrie ---
    /// Flugmasse inkl. Treibstoff und interner Zuladung.
    pub mass_kg: f64,
    /// Flügelreferenzfläche.
    pub wing_area_m2: f64,

    // --- Antrieb ---
    /// Maximaler Gesamtschub auf Meereshöhe (ohne Nachbrenner – Tarnkappe).
    pub thrust_max_n: f64,
    /// Schub skaliert mit (rho / rho0)^exponent.
    pub thrust_density_exponent: f64,
    /// Zeitkonstante des Triebwerkshochlaufs.
    pub engine_spool_time_s: f64,

    // --- Aerodynamik ---
    /// Auftriebsbeiwert bei Anstellwinkel 0.
    pub cl0: f64,
    /// Auftriebsanstieg dCL/dα pro rad.
    pub cl_alpha: f64,
    /// Anstellwinkel, ab dem die Strömung abreißt.
    pub alpha_stall_rad: f64,
    /// Relativer Auftriebsverlust pro rad jenseits von `alpha_stall_rad`.
    pub post_stall_lift_drop: f64,
    /// Nullwiderstandsbeiwert.
    pub cd0: f64,
    /// Faktor des induzierten Widerstands: CD = cd0 + k·CL².
    pub induced_drag_k: f64,
    /// Zusätzlicher Widerstandsbeiwert bei ausgefahrener Luftbremse.
    pub airbrake_cd: f64,
    /// Seitenkraftbeiwert pro rad Schiebewinkel.
    pub cy_beta: f64,

    // --- Flugsteuerung (Fly-by-Wire, ratenkommandiert) ---
    /// Maximale Rollrate bei voller Ruderwirksamkeit.
    pub max_roll_rate: f64,
    /// Maximale Nickrate bei voller Ruderwirksamkeit.
    pub max_pitch_rate: f64,
    /// Maximale Gierrate bei voller Ruderwirksamkeit.
    pub max_yaw_rate: f64,
    /// Zeitkonstante, mit der die Drehraten dem Kommando folgen.
    pub rate_time_constant_s: f64,
    /// Staudruck, ab dem die Ruder voll wirken.
    pub control_q_ref_pa: f64,
    /// Positives Lastvielfaches, das die Steuerung zulässt.
    pub g_limit_pos: f64,
    /// Negatives Lastvielfaches (Betrag), das die Steuerung zulässt.
    pub g_limit_neg: f64,
    /// Anstellwinkelbegrenzer (muss unter `alpha_stall_rad` liegen).
    pub alpha_limit_rad: f64,
    /// Verstärkung der Schiebewinkel-Kompensation (Gierdämpfer).
    pub yaw_damper_gain: f64,
}

/// Der F-29 „Silent Sniper“: einsitziges Tarnkappen-Angriffsflugzeug mit
/// zwei Triebwerken ohne Nachbrenner.
///
/// Die Werte sind bewusst eigenständig gewählt: ein schweres, eher träges
/// Angriffsflugzeug mit hohem Gleitwiderstand der facettierten Zelle,
/// aber gutmütigem Abreißverhalten.
pub const F29_SPEC: AircraftSpec = AircraftSpec {
    name: "F-29",
    mass_kg: 21_500.0,
    wing_area_m2: 84.0,

    thrust_max_n: 98_000.0,
    thrust_density_exponent: 0.75,
    engine_spool_time_s: 2.5,

    cl0: 0.04,
    cl_alpha: 3.6,
    alpha_stall_rad: 0.30,
    post_stall_lift_drop: 1.8,
    cd0: 0.024,
    induced_drag_k: 0.13,
    airbrake_cd: 0.07,
    cy_beta: 0.65,

    max_roll_rate: 2.6,
    max_pitch_rate: 0.45,
    max_yaw_rate: 0.20,
    rate_time_constant_s: 0.18,
    control_q_ref_pa: 9_000.0,
    g_limit_pos: 7.0,
    g_limit_neg: 2.5,
    alpha_limit_rad: 0.26,
    yaw_damper_gain: 2.5,
};

impl AircraftSpec {
    /// Prüft die Parameter auf physikalische Plausibilität.
    pub fn validate(&self) -> Result<(), ValidationError> {
        let err = |field: &'static str, reason: String| ValidationError {
            record: self.name.to_string(),
            field,
            reason,
        };
        let positive: [(&'static str, f64); 19] = [
            ("mass_kg", self.mass_kg),
            ("wing_area_m2", self.wing_area_m2),
            ("thrust_max_n", self.thrust_max_n),
            ("thrust_density_exponent", self.thrust_density_exponent),
            ("engine_spool_time_s", self.engine_spool_time_s),
            ("cl_alpha", self.cl_alpha),
            ("alpha_stall_rad", self.alpha_stall_rad),
            ("post_stall_lift_drop", self.post_stall_lift_drop),
            ("cd0", self.cd0),
            ("induced_drag_k", self.induced_drag_k),
            ("cy_beta", self.cy_beta),
            ("max_roll_rate", self.max_roll_rate),
            ("max_pitch_rate", self.max_pitch_rate),
            ("max_yaw_rate", self.max_yaw_rate),
            ("rate_time_constant_s", self.rate_time_constant_s),
            ("control_q_ref_pa", self.control_q_ref_pa),
            ("g_limit_pos", self.g_limit_pos),
            ("g_limit_neg", self.g_limit_neg),
            ("alpha_limit_rad", self.alpha_limit_rad),
        ];
        for (field, value) in positive {
            if !(value.is_finite() && value > 0.0) {
                return Err(err(field, format!("muss endlich und > 0 sein, ist {value}")));
            }
        }
        if !(self.airbrake_cd.is_finite() && self.airbrake_cd >= 0.0) {
            return Err(err("airbrake_cd", "muss >= 0 sein".into()));
        }
        if !(self.yaw_damper_gain.is_finite() && self.yaw_damper_gain >= 0.0) {
            return Err(err("yaw_damper_gain", "muss >= 0 sein".into()));
        }
        if self.alpha_limit_rad >= self.alpha_stall_rad {
            return Err(err(
                "alpha_limit_rad",
                "muss kleiner als alpha_stall_rad sein, sonst schützt der Begrenzer nicht".into(),
            ));
        }
        if self.alpha_stall_rad > 0.6 {
            return Err(err("alpha_stall_rad", "unplausibel groß (> 0.6 rad)".into()));
        }
        if self.g_limit_pos < 1.0 {
            return Err(err("g_limit_pos", "muss mindestens 1 g erlauben".into()));
        }
        Ok(())
    }

    /// Maximaler Auftriebsbeiwert (am Abreißpunkt).
    pub fn cl_max(&self) -> f64 {
        self.cl0 + self.cl_alpha * self.alpha_stall_rad
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f29_spec_is_valid() {
        F29_SPEC.validate().expect("F-29-Daten müssen plausibel sein");
    }

    #[test]
    fn validation_rejects_limiter_above_stall() {
        let spec = AircraftSpec { alpha_limit_rad: F29_SPEC.alpha_stall_rad + 0.01, ..F29_SPEC };
        let e = spec.validate().unwrap_err();
        assert_eq!(e.field, "alpha_limit_rad");
    }

    #[test]
    fn validation_rejects_non_finite_values() {
        let spec = AircraftSpec { mass_kg: f64::NAN, ..F29_SPEC };
        assert_eq!(spec.validate().unwrap_err().field, "mass_kg");
    }

    #[test]
    fn stall_speed_is_plausible() {
        // Mindestgeschwindigkeit im Horizontalflug auf Meereshöhe: Größenordnung
        // 70–100 m/s für ein schweres Angriffsflugzeug ohne Landeklappen.
        let w = F29_SPEC.mass_kg * 9.80665;
        let v_stall = (2.0 * w / (1.225 * F29_SPEC.wing_area_m2 * F29_SPEC.cl_max())).sqrt();
        assert!((60.0..110.0).contains(&v_stall), "v_stall = {v_stall}");
    }
}
