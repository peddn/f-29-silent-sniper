//! Steuereingaben pro Tick.
//!
//! Eingaben werden **quantisiert**, bevor sie die Simulation erreichen. So
//! ist das, was im Spiel geflogen wird, exakt dasselbe, was ein Replay
//! speichert und abspielt – unabhängig davon, ob der Wert ursprünglich von
//! Tastatur, Gamepad oder einem Autopiloten kam.

/// Bitflags für Schalter und einmalige Ereignisse.
pub mod flags {
    /// Luftbremse ausgefahren (gehalten).
    pub const AIRBRAKE: u8 = 1 << 0;
    /// Nächster Navigationspunkt (Flanke: nur im Tick des Tastendrucks setzen).
    pub const NEXT_STEERPOINT: u8 = 1 << 1;
    /// Landeklappen ausgefahren (gehalten).
    pub const FLAPS: u8 = 1 << 2;
    /// Fahrwerk ausgefahren (gehalten).
    pub const GEAR: u8 = 1 << 3;
    /// Radar ein/aus umschalten (Flanke).
    pub const RADAR_TOGGLE: u8 = 1 << 4;
    /// Bits 5–7: Blickrichtung der Zielerfassung wählen (Flanke, Kennung
    /// `sensors::LookDir::code`, 0 = keine Änderung).
    pub const LOOK_SHIFT: u8 = 5;
    pub const LOOK_MASK: u8 = 0b111 << LOOK_SHIFT;
}

/// Bitflags der Waffenbedienung (eigenes Byte; Flanken bis auf `GUN`).
pub mod weapon_flags {
    /// Waffe des gewählten Schachts auslösen.
    pub const RELEASE: u8 = 1 << 0;
    /// Schachtklappen öffnen bzw. schließen.
    pub const BAY_DOORS: u8 = 1 << 1;
    /// Nächste Waffe wählen (GUN, BAY 1–4).
    pub const NEXT_BAY: u8 = 1 << 2;
    /// Bordkanone feuern, gleich welche Waffe gewählt ist (gehalten).
    pub const GUN: u8 = 1 << 3;
    /// Abzug der gewählten Waffe gehalten: feuert die Kanone, wenn sie gewählt
    /// ist (Schachtwaffen lösen über die Flanke `RELEASE` aus).
    pub const TRIGGER: u8 = 1 << 4;
    /// Düppel abwerfen.
    pub const CHAFF: u8 = 1 << 5;
    /// Fackel abwerfen.
    pub const FLARE: u8 = 1 << 6;
}

/// Ein Tick Steuereingabe, quantisiert.
///
/// Vorzeichen: `pitch` > 0 = ziehen (Nase hoch), `roll` > 0 = rechts rollen,
/// `yaw` > 0 = Nase nach rechts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ControlFrame {
    pub pitch: i16,
    pub roll: i16,
    pub yaw: i16,
    /// 0 = Leerlauf, `u16::MAX` = Vollschub.
    pub throttle: u16,
    pub flags: u8,
    /// Waffenbedienung, siehe [`weapon_flags`].
    pub weapons: u8,
}

const AXIS_MAX: f64 = i16::MAX as f64;

fn quantize_axis(v: f64) -> i16 {
    if !v.is_finite() {
        return 0;
    }
    (v.clamp(-1.0, 1.0) * AXIS_MAX).round() as i16
}

impl ControlFrame {
    /// Aus analogen Werten (-1..1 bzw. 0..1) erzeugen.
    pub fn from_analog(pitch: f64, roll: f64, yaw: f64, throttle: f64, flags: u8) -> Self {
        let throttle = if throttle.is_finite() {
            (throttle.clamp(0.0, 1.0) * f64::from(u16::MAX)).round() as u16
        } else {
            0
        };
        Self {
            pitch: quantize_axis(pitch),
            roll: quantize_axis(roll),
            yaw: quantize_axis(yaw),
            throttle,
            flags,
            weapons: 0,
        }
    }

    /// Dieselbe Eingabe mit Waffenbedienung.
    pub fn with_weapons(self, weapons: u8) -> Self {
        Self { weapons, ..self }
    }

    pub fn has_weapon(&self, flag: u8) -> bool {
        self.weapons & flag != 0
    }

    pub fn pitch_f(&self) -> f64 {
        (f64::from(self.pitch) / AXIS_MAX).max(-1.0)
    }

    pub fn roll_f(&self) -> f64 {
        (f64::from(self.roll) / AXIS_MAX).max(-1.0)
    }

    pub fn yaw_f(&self) -> f64 {
        (f64::from(self.yaw) / AXIS_MAX).max(-1.0)
    }

    pub fn throttle_f(&self) -> f64 {
        f64::from(self.throttle) / f64::from(u16::MAX)
    }

    pub fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    /// Gewählte Blickrichtung der Zielerfassung (Kennung 1..4, 0 = keine).
    pub fn look_code(&self) -> u8 {
        (self.flags & flags::LOOK_MASK) >> flags::LOOK_SHIFT
    }

    /// Flag-Bits für eine Blickrichtungs-Kennung.
    pub fn look_flags(code: u8) -> u8 {
        (code << flags::LOOK_SHIFT) & flags::LOOK_MASK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantization_roundtrip() {
        let c = ControlFrame::from_analog(1.0, -1.0, 0.5, 1.0, flags::AIRBRAKE);
        assert_eq!(c.pitch_f(), 1.0);
        assert_eq!(c.roll_f(), -1.0);
        assert!((c.yaw_f() - 0.5).abs() < 1e-4);
        assert_eq!(c.throttle_f(), 1.0);
        assert!(c.has(flags::AIRBRAKE));
        assert!(!c.has(flags::NEXT_STEERPOINT));
    }

    #[test]
    fn clamps_and_rejects_nan() {
        let c = ControlFrame::from_analog(5.0, f64::NAN, -9.0, -1.0, 0);
        assert_eq!(c.pitch, i16::MAX);
        assert_eq!(c.roll, 0);
        assert_eq!(c.yaw, -i16::MAX);
        assert_eq!(c.throttle, 0);
    }
}
