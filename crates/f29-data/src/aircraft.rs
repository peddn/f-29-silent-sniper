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
    /// Anstellwinkel, ab dem die Abrisswarnung kommt (muss unter `alpha_stall_rad` liegen).
    /// Es gibt bewusst **keinen** Anstellwinkelbegrenzer: Wer zu hart zieht, reißt ab.
    pub alpha_warning_rad: f64,
    // --- Strömungsabriss (stetig, Stärke 0..1 aus dem Fahrtdefizit) ---
    /// Relatives Fahrtdefizit unter der Abrissgeschwindigkeit, bei dem der
    /// Abriss seine volle Stärke erreicht (z. B. 0,3 = 30 % zu langsam).
    pub stall_deficit_full: f64,
    /// Zusätzliche g-Last, die voller Zug am Knüppel für die Abrissgeschwindigkeit
    /// anfordert (v_s = v_s1g · √(1 + Zug · stall_stick_g)).
    pub stall_stick_g: f64,
    /// Zeitkonstante, mit der die Abrissstärke dem Fahrtdefizit folgt (s).
    pub stall_time_constant_s: f64,
    /// Abkippen bei vollem Abriss: Drehrate (rad/s), mit der die Nase aus
    /// jeder Fluglage Richtung senkrecht nach unten fällt.
    pub stall_pitch_break: f64,
    /// Anteil der Ruderwirkung, der bei vollem Abriss erhalten bleibt (0..1).
    pub stall_control_factor: f64,
    /// Anteil des Auftriebs, der bei vollem Abriss bleibt (0..1).
    pub stall_lift_factor: f64,
    /// Verstärkung der Schiebewinkel-Kompensation (Gierdämpfer).
    pub yaw_damper_gain: f64,
}

/// Der F-29 „Silent Sniper“: einsitziges Tarnkappen-Angriffsflugzeug mit
/// zwei Triebwerken ohne Nachbrenner.
///
/// Die Werte sind bewusst eigenständig gewählt: ein schweres, untermotorisiertes
/// Angriffsflugzeug (Schub/Gewicht ≈ 0,38) mit hohem induziertem Widerstand
/// der facettierten Zelle. Harte Kurven kosten viel Fahrt, das Steigvermögen
/// ist begrenzt, und wer zu hart zieht, reißt die Strömung ab und sackt durch.
pub const F29_SPEC: AircraftSpec = AircraftSpec {
    name: "F-29",
    mass_kg: 21_500.0,
    wing_area_m2: 70.0,

    thrust_max_n: 80_000.0,
    thrust_density_exponent: 0.75,
    engine_spool_time_s: 2.5,

    cl0: 0.04,
    cl_alpha: 3.6,
    alpha_stall_rad: 0.30,
    post_stall_lift_drop: 8.0,
    cd0: 0.024,
    induced_drag_k: 0.20,
    airbrake_cd: 0.07,
    cy_beta: 0.65,

    max_roll_rate: 2.6,
    max_pitch_rate: 0.45,
    max_yaw_rate: 0.20,
    rate_time_constant_s: 0.18,
    control_q_ref_pa: 14_000.0,
    g_limit_pos: 7.0,
    g_limit_neg: 2.5,
    alpha_warning_rad: 0.25,
    stall_deficit_full: 0.3,
    stall_stick_g: 4.0,
    stall_time_constant_s: 0.3,
    stall_pitch_break: 1.0,
    stall_control_factor: 0.15,
    stall_lift_factor: 0.35,
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
        let positive: [(&'static str, f64); 23] = [
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
            ("alpha_warning_rad", self.alpha_warning_rad),
            ("stall_pitch_break", self.stall_pitch_break),
            ("stall_deficit_full", self.stall_deficit_full),
            ("stall_stick_g", self.stall_stick_g),
            ("stall_time_constant_s", self.stall_time_constant_s),
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
        if self.alpha_warning_rad >= self.alpha_stall_rad {
            return Err(err(
                "alpha_warning_rad",
                "muss kleiner als alpha_stall_rad sein, sonst kommt die Warnung zu spät".into(),
            ));
        }
        if !(0.0..=1.0).contains(&self.stall_control_factor) {
            return Err(err("stall_control_factor", "muss zwischen 0 und 1 liegen".into()));
        }
        if self.stall_deficit_full > 1.0 {
            return Err(err("stall_deficit_full", "muss höchstens 1 sein".into()));
        }
        if !(0.0..=1.0).contains(&self.stall_lift_factor) {
            return Err(err("stall_lift_factor", "muss zwischen 0 und 1 liegen".into()));
        }
        if self.alpha_stall_rad > 0.6 {
            return Err(err("alpha_stall_rad", "unplausibel groß (> 0.6 rad)".into()));
        }
        if self.g_limit_pos < 1.0 {
            return Err(err("g_limit_pos", "muss mindestens 1 g erlauben".into()));
        }
        Ok(())
    }

    /// Abrissgeschwindigkeit im 1-g-Horizontalflug bei Luftdichte `density` (m/s).
    pub fn stall_speed_1g(&self, density: f64) -> f64 {
        (2.0 * self.mass_kg * 9.806_65 / (density * self.wing_area_m2 * self.cl_max())).sqrt()
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
    fn validation_rejects_warning_above_stall() {
        let spec = AircraftSpec { alpha_warning_rad: F29_SPEC.alpha_stall_rad + 0.01, ..F29_SPEC };
        let e = spec.validate().unwrap_err();
        assert_eq!(e.field, "alpha_warning_rad");
    }

    #[test]
    fn validation_rejects_non_finite_values() {
        let spec = AircraftSpec { mass_kg: f64::NAN, ..F29_SPEC };
        assert_eq!(spec.validate().unwrap_err().field, "mass_kg");
    }

    #[test]
    fn thrust_to_weight_is_low() {
        let tw = F29_SPEC.thrust_max_n / (F29_SPEC.mass_kg * 9.80665);
        assert!((0.3..0.45).contains(&tw), "Schub/Gewicht {tw}");
    }

    #[test]
    fn stall_speed_is_plausible() {
        // Mindestgeschwindigkeit im Horizontalflug auf Meereshöhe: Größenordnung
        // 60–80 m/s (120–155 kt) für ein schweres Angriffsflugzeug ohne Landeklappen.
        let w = F29_SPEC.mass_kg * 9.80665;
        let v_stall = (2.0 * w / (1.225 * F29_SPEC.wing_area_m2 * F29_SPEC.cl_max())).sqrt();
        assert!((60.0..80.0).contains(&v_stall), "v_stall = {v_stall}");
    }
}
