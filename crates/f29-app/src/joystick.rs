//! Joystick (Voreinstellung: Logitech Extreme 3D Pro unter Linux).
//!
//! Die Belegung steht als Text in `config/joystick.cfg`
//! ([`joystick_config`](crate::joystick_config)); `joystick.cfg` im
//! Startverzeichnis oder `--joystick DATEI` ersetzt sie. Mit
//! `--joystick-debug` meldet die Konsole jede Achse und Taste in der
//! Schreibweise der Datei.
//!
//! Knüppel und Drehgriff übersteuern die Tastatur, solange sie ausgelenkt
//! sind; der Schubregler setzt den Schub, sobald er bewegt wird.

use crate::input::{PilotInput, flight_input};
use crate::joystick_config::{Action, AxisBinding, AxisId, ButtonId, JoystickConfig, linux};
use crate::mfd::MfdView;
use crate::sim::{SimState, SimSystems};
use crate::view::{ViewMode, ViewSettings};
use bevy::input::gamepad::{
    GamepadConnection, GamepadConnectionEvent, RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent,
};
use bevy::prelude::*;
use f29_core::controls::weapon_flags;
use f29_core::sensors::LookDir;
use std::collections::HashMap;

/// Belegung und Diagnosemodus.
#[derive(Resource, Debug, Clone)]
pub struct JoystickSettings {
    pub config: JoystickConfig,
    /// Rohwerte aller Achsen und Tasten auf der Konsole melden.
    pub debug: bool,
}

pub struct JoystickPlugin {
    pub settings: JoystickSettings,
}

impl Plugin for JoystickPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.settings.clone())
            .add_systems(Update, (connections, debug_raw))
            .add_systems(Update, joystick_input.in_set(SimSystems::Input).after(flight_input));
    }
}

/// Belegung laden: `--joystick DATEI`, sonst `joystick.cfg` im
/// Startverzeichnis, sonst die mitgelieferte. Ein Fehler in der Datei fällt
/// auf die mitgelieferte zurück und wird gemeldet.
pub fn load(path: Option<&str>) -> JoystickConfig {
    let (path, explicit) = match path {
        Some(p) => (p.to_string(), true),
        None => ("joystick.cfg".to_string(), false),
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => match crate::joystick_config::parse(&text) {
            Ok(cfg) => {
                eprintln!("Joystick-Belegung aus {path}");
                cfg
            }
            Err(e) => {
                eprintln!("{path}: {e} – mitgelieferte Belegung gilt");
                JoystickConfig::default()
            }
        },
        Err(e) => {
            if explicit {
                eprintln!("{path}: {e} – mitgelieferte Belegung gilt");
            }
            JoystickConfig::default()
        }
    }
}

fn to_axis(a: AxisId) -> GamepadAxis {
    match a {
        AxisId::X => GamepadAxis::LeftStickX,
        AxisId::Y => GamepadAxis::LeftStickY,
        AxisId::Z => GamepadAxis::LeftZ,
        AxisId::Rx => GamepadAxis::RightStickX,
        AxisId::Ry => GamepadAxis::RightStickY,
        AxisId::Rz => GamepadAxis::RightZ,
        AxisId::Abs(n) => GamepadAxis::Other(linux::abs_code(n)),
        AxisId::Raw(n) => GamepadAxis::Other(n),
    }
}

fn from_axis(a: GamepadAxis) -> AxisId {
    match a {
        GamepadAxis::LeftStickX => AxisId::X,
        GamepadAxis::LeftStickY => AxisId::Y,
        GamepadAxis::LeftZ => AxisId::Z,
        GamepadAxis::RightStickX => AxisId::Rx,
        GamepadAxis::RightStickY => AxisId::Ry,
        GamepadAxis::RightZ => AxisId::Rz,
        GamepadAxis::Other(n) => linux::abs_number(n).map_or(AxisId::Raw(n), AxisId::Abs),
    }
}

fn to_button(b: &ButtonId) -> GamepadButton {
    match b {
        ButtonId::Number(n) => GamepadButton::Other(linux::button_code(*n)),
        ButtonId::HatUp => GamepadButton::DPadUp,
        ButtonId::HatRight => GamepadButton::DPadRight,
        ButtonId::HatDown => GamepadButton::DPadDown,
        ButtonId::HatLeft => GamepadButton::DPadLeft,
        ButtonId::Raw(n) => GamepadButton::Other(*n),
        ButtonId::Named(name) => match *name {
            "South" => GamepadButton::South,
            "East" => GamepadButton::East,
            "North" => GamepadButton::North,
            "West" => GamepadButton::West,
            "C" => GamepadButton::C,
            "Z" => GamepadButton::Z,
            "LeftTrigger" => GamepadButton::LeftTrigger,
            "LeftTrigger2" => GamepadButton::LeftTrigger2,
            "RightTrigger" => GamepadButton::RightTrigger,
            "RightTrigger2" => GamepadButton::RightTrigger2,
            "Select" => GamepadButton::Select,
            "Start" => GamepadButton::Start,
            "Mode" => GamepadButton::Mode,
            "LeftThumb" => GamepadButton::LeftThumb,
            _ => GamepadButton::RightThumb,
        },
    }
}

fn from_button(b: GamepadButton) -> ButtonId {
    match b {
        GamepadButton::DPadUp => ButtonId::HatUp,
        GamepadButton::DPadRight => ButtonId::HatRight,
        GamepadButton::DPadDown => ButtonId::HatDown,
        GamepadButton::DPadLeft => ButtonId::HatLeft,
        GamepadButton::Other(n) => linux::button_number(n).map_or(ButtonId::Raw(n), ButtonId::Number),
        named => {
            let text = format!("{named:?}");
            crate::joystick_config::BUTTON_NAMES
                .iter()
                .find(|n| **n == text)
                .map_or(ButtonId::Raw(u32::MAX), |n| ButtonId::Named(n))
        }
    }
}

/// Wert einer belegten Achse mit Richtung; `None`, wenn der Stick sie nicht hat.
fn read(pad: &Gamepad, b: Option<AxisBinding>) -> Option<f64> {
    let b = b?;
    let v = f64::from(pad.get(to_axis(b.axis))?);
    Some(if b.invert { -v } else { v })
}

/// Joystick → Steuerorgane, Waffen und Anzeigen.
#[allow(clippy::too_many_arguments)]
fn joystick_input(
    settings: Res<JoystickSettings>,
    gamepads: Query<&Gamepad>,
    mut pilot: ResMut<PilotInput>,
    mut view: ResMut<ViewSettings>,
    mut mfd: ResMut<MfdView>,
    mut state: ResMut<SimState>,
    mut last_throttle: Local<Option<f64>>,
) {
    let cfg = &settings.config;
    let dz = f64::from(cfg.deadzone);
    let live = |v: Option<f64>| v.filter(|v| v.abs() >= dz);
    for pad in &gamepads {
        // Knüppel und Drehgriff: übersteuern die Tastatur, solange ausgelenkt.
        let (roll, pitch) = (live(read(pad, cfg.roll)), live(read(pad, cfg.pitch)));
        if roll.is_some() || pitch.is_some() {
            pilot.roll = roll.unwrap_or(0.0);
            pilot.pitch = pitch.unwrap_or(0.0);
        }
        if let Some(yaw) = live(read(pad, cfg.yaw)) {
            pilot.yaw = yaw;
        }
        // Schubregler: gilt, sobald er bewegt wird (bis dahin Tastatur).
        if let Some(v) = read(pad, cfg.throttle) {
            if last_throttle.is_some_and(|l| (l - v).abs() > 0.01) {
                pilot.throttle = ((v + 1.0) * 0.5).clamp(0.0, 1.0);
            }
            if last_throttle.is_none_or(|l| (l - v).abs() > 0.01) {
                *last_throttle = Some(v);
            }
        }
        for (button, action) in &cfg.buttons {
            let b = to_button(button);
            let (held, pressed) = (pad.pressed(b), pad.just_pressed(b));
            match action {
                Action::Gun => pilot.gun |= held,
                Action::Fire => {
                    pilot.trigger |= held;
                    if pressed {
                        pilot.weapons |= weapon_flags::RELEASE;
                    }
                }
                _ if !pressed => {}
                Action::NextWeapon => pilot.weapons |= weapon_flags::NEXT_BAY,
                Action::BayDoors => pilot.weapons |= weapon_flags::BAY_DOORS,
                Action::Chaff => pilot.weapons |= weapon_flags::CHAFF,
                Action::Flare => pilot.weapons |= weapon_flags::FLARE,
                Action::Radar => pilot.radar_toggle = true,
                Action::NextSteerpoint => pilot.next_steerpoint = true,
                Action::LookForward => pilot.look = LookDir::Forward.code(),
                Action::LookRight => pilot.look = LookDir::Right.code(),
                Action::LookAft => pilot.look = LookDir::Aft.code(),
                Action::LookLeft => pilot.look = LookDir::Left.code(),
                Action::Gear => pilot.gear = !pilot.gear,
                Action::Flaps => pilot.flaps = !pilot.flaps,
                Action::Airbrake => pilot.airbrake = !pilot.airbrake,
                Action::MapZoom => mfd.0.cycle_zoom(),
                Action::ZoomIn => mfd.0.zoom_in(),
                Action::ZoomOut => mfd.0.zoom_out(),
                Action::MapMode => mfd.0.toggle_map_mode(),
                Action::MfdPage => mfd.0.next_page(),
                Action::HudMode => view.hud_mode = view.hud_mode.next(),
                Action::View => {
                    view.mode = match view.mode {
                        ViewMode::Cockpit => ViewMode::Chase,
                        ViewMode::Chase => ViewMode::Cockpit,
                    }
                }
                Action::Pause => state.paused = !state.paused,
            }
        }
    }
}

/// Anmelden und Abmelden von Joysticks auf der Konsole.
fn connections(settings: Res<JoystickSettings>, mut events: MessageReader<GamepadConnectionEvent>) {
    for e in events.read() {
        match &e.connection {
            GamepadConnection::Connected { name, vendor_id, product_id } => {
                let id = match (vendor_id, product_id) {
                    (Some(v), Some(p)) => format!(" ({v:04x}:{p:04x})"),
                    _ => String::new(),
                };
                eprintln!("Joystick verbunden: {name}{id}");
                if settings.debug {
                    let c = &settings.config;
                    let axis = |b: Option<AxisBinding>| {
                        b.map_or("-".to_string(), |b| {
                            format!("{}{}", b.axis, if b.invert { " invert" } else { "" })
                        })
                    };
                    eprintln!(
                        "  Belegung: roll = {}, pitch = {}, yaw = {}, throttle = {}",
                        axis(c.roll),
                        axis(c.pitch),
                        axis(c.yaw),
                        axis(c.throttle)
                    );
                    for (b, a) in &c.buttons {
                        eprintln!("  {} = {b}", a.name());
                    }
                }
            }
            GamepadConnection::Disconnected => eprintln!("Joystick getrennt"),
        }
    }
}

/// `--joystick-debug`: jede Achsenbewegung (ab 0,05 Änderung) und jeden
/// Tastendruck in der Schreibweise der Belegungsdatei melden.
fn debug_raw(
    settings: Res<JoystickSettings>,
    mut axes: MessageReader<RawGamepadAxisChangedEvent>,
    mut buttons: MessageReader<RawGamepadButtonChangedEvent>,
    mut shown: Local<HashMap<(Entity, GamepadAxis), f32>>,
    mut down: Local<HashMap<(Entity, GamepadButton), bool>>,
) {
    if !settings.debug {
        axes.clear();
        buttons.clear();
        return;
    }
    for e in axes.read() {
        let last = shown.entry((e.gamepad, e.axis)).or_insert(f32::NAN);
        if last.is_nan() || (e.value - *last).abs() >= 0.05 || (e.value.abs() >= 0.999 && *last != e.value) {
            *last = e.value;
            eprintln!("Achse {:<8} {:+.2}", from_axis(e.axis).to_string(), e.value);
        }
    }
    for e in buttons.read() {
        let is_down = e.value > 0.5;
        let was = down.insert((e.gamepad, e.button), is_down).unwrap_or(false);
        if is_down != was {
            let what = if is_down { "gedrückt" } else { "losgelassen" };
            eprintln!("Taste {:<8} {what}", from_button(e.button).to_string());
        }
    }
}
