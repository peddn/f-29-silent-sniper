//! Steuerorgane und Tastatur-Achsen → quantisierte Steuereingaben. Tasten
//! laufen über die gemeinsame Aktionsliste (`actions.rs`), der Joystick über
//! `joystick.rs`.

use crate::sim::SimSystems;
use bevy::prelude::*;
use f29_core::ControlFrame;
use f29_core::controls::{flags, weapon_flags};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, (SimSystems::Input, SimSystems::Step, SimSystems::Present).chain())
            .add_systems(Update, flight_input.in_set(SimSystems::Input));
    }
}

/// Aktueller Zustand der Steuerorgane.
#[derive(Resource, Debug, Clone)]
pub struct PilotInput {
    pub pitch: f64,
    pub roll: f64,
    pub yaw: f64,
    pub throttle: f64,
    pub airbrake: bool,
    pub flaps: bool,
    /// Fahrwerk ausgefahren.
    pub gear: bool,
    /// Wird beim nächsten Tick einmalig gesendet.
    pub(crate) next_steerpoint: bool,
    /// Radar umschalten (einmalig).
    pub(crate) radar_toggle: bool,
    /// Blickrichtung der Zielerfassung wählen (Kennung, einmalig; 0 = keine).
    pub(crate) look: u8,
    /// Waffenbedienung (einmalig, siehe `weapon_flags`).
    pub(crate) weapons: u16,
    /// Abzug der Bordkanone (gehalten).
    pub(crate) gun: bool,
    /// Abzug der gewählten Waffe (gehalten; feuert die gewählte Kanone).
    pub(crate) trigger: bool,
}

impl Default for PilotInput {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            roll: 0.0,
            yaw: 0.0,
            throttle: 0.55,
            airbrake: false,
            flaps: false,
            gear: false,
            next_steerpoint: false,
            radar_toggle: false,
            look: 0,
            weapons: 0,
            gun: false,
            trigger: false,
        }
    }
}

impl PilotInput {
    /// Steuerorgane passend zum Startzustand (Schub, Fahrwerk).
    pub fn matching(sim: &f29_core::Sim) -> Self {
        Self { throttle: sim.config().spawn.throttle, gear: sim.aircraft().gear > 0.5, ..Self::default() }
    }

    /// Erzeugt die Eingabe für einen Tick und verbraucht einmalige Ereignisse.
    /// Eine Blickrichtungstaste trägt die Zielart des HUD-Modus mit (AA:
    /// Jäger, NAV: Landebahnen, AG: Bodenziele).
    pub fn take_frame(&mut self, mode: f29_hud::HudMode) -> ControlFrame {
        let pick = match mode {
            _ if self.look == 0 => 0,
            f29_hud::HudMode::AirToAir => weapon_flags::PICK_AIR,
            f29_hud::HudMode::Nav => weapon_flags::PICK_RUNWAY,
            f29_hud::HudMode::AirToGround => 0,
        };
        let mut f = 0;
        if self.airbrake {
            f |= flags::AIRBRAKE;
        }
        if self.flaps {
            f |= flags::FLAPS;
        }
        if self.gear {
            f |= flags::GEAR;
        }
        if std::mem::take(&mut self.next_steerpoint) {
            f |= flags::NEXT_STEERPOINT;
        }
        if std::mem::take(&mut self.radar_toggle) {
            f |= flags::RADAR_TOGGLE;
        }
        f |= ControlFrame::look_flags(std::mem::take(&mut self.look));
        ControlFrame::from_analog(self.pitch, self.roll, self.yaw, self.throttle, f).with_weapons(
            std::mem::take(&mut self.weapons)
                | pick
                | if self.gun { weapon_flags::GUN } else { 0 }
                | if self.trigger { weapon_flags::TRIGGER } else { 0 },
        )
    }
}

/// Wie schnell digitale Tasten den Knüppel auslenken bzw. zurückführen (1/s).
const KEY_RAMP: f64 = 3.0;
const KEY_RETURN: f64 = 6.0;
const THROTTLE_RATE: f64 = 0.4;

fn ramp(current: f64, target: f64, dt: f64) -> f64 {
    let rate = if target == 0.0 { KEY_RETURN } else { KEY_RAMP };
    let step = rate * dt;
    current + (target - current).clamp(-step, step)
}

fn axis(keys: &ButtonInput<KeyCode>, neg: &[KeyCode], pos: &[KeyCode]) -> f64 {
    let n = neg.iter().any(|k| keys.pressed(*k));
    let p = pos.iter().any(|k| keys.pressed(*k));
    f64::from(i8::from(p) - i8::from(n))
}

/// Tastatur: Knüppel, Seitenruder und Schub.
pub(crate) fn flight_input(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut pilot: ResMut<PilotInput>) {
    let dt = time.delta_secs_f64();

    // Tastatur: Pfeil hoch = drücken (Nase runter), Pfeil runter = ziehen.
    let pitch_t = axis(&keys, &[KeyCode::ArrowUp, KeyCode::KeyW], &[KeyCode::ArrowDown, KeyCode::KeyS]);
    let roll_t = axis(&keys, &[KeyCode::ArrowLeft, KeyCode::KeyA], &[KeyCode::ArrowRight, KeyCode::KeyD]);
    let yaw_t = axis(&keys, &[KeyCode::KeyQ], &[KeyCode::KeyE]);
    pilot.pitch = ramp(pilot.pitch, pitch_t, dt);
    pilot.roll = ramp(pilot.roll, roll_t, dt);
    pilot.yaw = ramp(pilot.yaw, yaw_t, dt);

    let thr = axis(&keys, &[KeyCode::Minus, KeyCode::NumpadSubtract], &[KeyCode::Equal, KeyCode::NumpadAdd]);
    pilot.throttle = (pilot.throttle + thr * THROTTLE_RATE * dt).clamp(0.0, 1.0);
    let presets = [
        (KeyCode::Backquote, 0.0),
        (KeyCode::Digit1, 0.1),
        (KeyCode::Digit2, 0.2),
        (KeyCode::Digit3, 0.3),
        (KeyCode::Digit4, 0.4),
        (KeyCode::Digit5, 0.5),
        (KeyCode::Digit6, 0.6),
        (KeyCode::Digit7, 0.7),
        (KeyCode::Digit8, 0.8),
        (KeyCode::Digit9, 0.9),
        (KeyCode::Digit0, 1.0),
    ];
    for (key, value) in presets {
        if keys.just_pressed(key) {
            pilot.throttle = value;
        }
    }
}
