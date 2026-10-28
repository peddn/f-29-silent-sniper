//! Tastatur und Gamepad → quantisierte Steuereingaben.

use crate::mfd::MfdView;
use crate::sim::{SimState, SimSystems};
use crate::view::{ViewMode, ViewSettings};
use bevy::app::AppExit;
use bevy::input::keyboard::Key;
use bevy::prelude::*;
use f29_core::ControlFrame;
use f29_core::controls::flags;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, (SimSystems::Input, SimSystems::Step, SimSystems::Present).chain())
            .add_systems(Update, (flight_input, hotkeys).in_set(SimSystems::Input));
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
    next_steerpoint: bool,
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
        }
    }
}

impl PilotInput {
    /// Steuerorgane passend zum Startzustand (Schub, Fahrwerk).
    pub fn matching(sim: &f29_core::Sim) -> Self {
        Self { throttle: sim.config().spawn.throttle, gear: sim.aircraft().gear > 0.5, ..Self::default() }
    }

    /// Erzeugt die Eingabe für einen Tick und verbraucht einmalige Ereignisse.
    pub fn take_frame(&mut self) -> ControlFrame {
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
        ControlFrame::from_analog(self.pitch, self.roll, self.yaw, self.throttle, f)
    }
}

const DEADZONE: f32 = 0.08;
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

fn flight_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    time: Res<Time>,
    mut pilot: ResMut<PilotInput>,
) {
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
    if keys.just_pressed(KeyCode::KeyB) {
        pilot.airbrake = !pilot.airbrake;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        pilot.flaps = !pilot.flaps;
    }
    if keys.just_pressed(KeyCode::KeyG) {
        pilot.gear = !pilot.gear;
    }
    if keys.just_pressed(KeyCode::KeyN) {
        pilot.next_steerpoint = true;
    }

    // Gamepad: übersteuert die Tastatur, sobald ein Stick ausgelenkt ist.
    for pad in &gamepads {
        let get = |a: GamepadAxis| pad.get(a).unwrap_or(0.0);
        let dz = |v: f32| if v.abs() < DEADZONE { 0.0 } else { f64::from(v) };
        let (lx, ly) = (dz(get(GamepadAxis::LeftStickX)), dz(get(GamepadAxis::LeftStickY)));
        if lx != 0.0 || ly != 0.0 {
            pilot.roll = lx;
            pilot.pitch = -ly; // Stick nach vorn = drücken
        }
        let rt = f64::from(pad.get(GamepadButton::RightTrigger2).unwrap_or(0.0));
        let lt = f64::from(pad.get(GamepadButton::LeftTrigger2).unwrap_or(0.0));
        if rt > 0.05 || lt > 0.05 {
            pilot.yaw = rt - lt;
        }
        let ry = dz(get(GamepadAxis::RightStickY));
        pilot.throttle = (pilot.throttle + ry * THROTTLE_RATE * dt).clamp(0.0, 1.0);
        if pad.just_pressed(GamepadButton::East) {
            pilot.airbrake = !pilot.airbrake;
        }
        if pad.just_pressed(GamepadButton::South) {
            pilot.next_steerpoint = true;
        }
        if pad.just_pressed(GamepadButton::North) {
            pilot.gear = !pilot.gear;
        }
        if pad.just_pressed(GamepadButton::West) {
            pilot.flaps = !pilot.flaps;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    logical: Res<ButtonInput<Key>>,
    gamepads: Query<&Gamepad>,
    mut state: ResMut<SimState>,
    mut view: ResMut<ViewSettings>,
    mut mfd: ResMut<MfdView>,
    mut pilot: ResMut<PilotInput>,
    mut exit: MessageWriter<AppExit>,
) {
    let pad = |b: GamepadButton| gamepads.iter().any(|g| g.just_pressed(b));
    if keys.just_pressed(KeyCode::Escape) {
        state.save_replay();
        exit.write(AppExit::Success);
    }
    if keys.just_pressed(KeyCode::KeyP) || pad(GamepadButton::Start) {
        state.paused = !state.paused;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        state.restart();
        *pilot = PilotInput::matching(&state.sim);
    }
    // Ansichten: Shift + F-Taste
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if shift && keys.just_pressed(KeyCode::F1) {
        view.mode = ViewMode::Cockpit;
    }
    if shift && keys.just_pressed(KeyCode::F2) {
        view.mode = ViewMode::Chase;
    }
    if pad(GamepadButton::Select) {
        view.mode = match view.mode {
            ViewMode::Cockpit => ViewMode::Chase,
            ViewMode::Chase => ViewMode::Cockpit,
        };
    }
    if shift && keys.just_pressed(KeyCode::F3) {
        view.retro = view.retro.next();
    }
    if shift && keys.just_pressed(KeyCode::F4) {
        view.hud = !view.hud;
    }
    // MFDs: F3 Karte Gitter/Satellit, F4 nächste Seite rechts,
    // Y/X zoomen nach Tastenbeschriftung (deutsche und US-Tastatur).
    if !shift && keys.just_pressed(KeyCode::F3) {
        mfd.0.toggle_map_mode();
    }
    if !shift && keys.just_pressed(KeyCode::F4) {
        mfd.0.next_page();
    }
    let typed = |c: &str| logical.just_pressed(Key::Character(c.into()));
    if typed("y") || typed("Y") {
        mfd.0.zoom_in();
    }
    if typed("x") || typed("X") {
        mfd.0.zoom_out();
    }
}
