//! Eine Aktionsliste für Tastatur und Joystick: Beide melden nur, welche
//! [`Action`] gedrückt bzw. gehalten ist; ausgeführt wird an genau einer
//! Stelle ([`apply_actions`]). Die Tastaturbelegung steht hier als Tabelle,
//! die des Joysticks in `config/joystick.cfg`. Achsen (Knüppel, Schub) liest
//! jedes Gerät selbst (`input.rs`, `joystick.rs`).

use crate::input::PilotInput;
use crate::joystick_config::Action;
use crate::mfd::MfdView;
use crate::sim::{SimState, SimSystems};
use crate::view::{ViewMode, ViewSettings};
use bevy::app::AppExit;
use bevy::input::keyboard::Key;
use bevy::prelude::*;
use f29_core::controls::weapon_flags;
use f29_core::sensors::LookDir;

pub struct ActionsPlugin;

impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Actions>().add_systems(
            Update,
            (keyboard_actions, apply_actions.after(crate::joystick::joystick_input))
                .chain()
                .in_set(SimSystems::Input),
        );
    }
}

/// Aktionen dieses Frames: (Aktion, eben gedrückt, gehalten).
#[derive(Resource, Debug, Default)]
pub struct Actions(pub Vec<(Action, bool, bool)>);

impl Actions {
    pub fn push(&mut self, action: Action, pressed: bool, held: bool) {
        if pressed || held {
            self.0.push((action, pressed, held));
        }
    }
}

/// Umschalttaste einer Tastenkombination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mod {
    /// Ohne Shift und Strg.
    None,
    Shift,
    Ctrl,
}

/// Tastaturbelegung (physische Tasten; Y/X für den Kartenzoom folgen der
/// Beschriftung, siehe [`keyboard_actions`]).
const KEYS: [(KeyCode, Mod, Action); 34] = [
    (KeyCode::Space, Mod::None, Action::Fire),
    (KeyCode::Backspace, Mod::None, Action::Gun),
    (KeyCode::KeyO, Mod::None, Action::BayDoors),
    (KeyCode::Tab, Mod::None, Action::NextWeapon),
    (KeyCode::KeyC, Mod::None, Action::Chaff),
    (KeyCode::KeyT, Mod::None, Action::Flare),
    (KeyCode::KeyM, Mod::None, Action::Ecm),
    (KeyCode::KeyU, Mod::None, Action::IrJammer),
    (KeyCode::KeyV, Mod::None, Action::Radar),
    (KeyCode::KeyN, Mod::None, Action::NextSteerpoint),
    (KeyCode::KeyI, Mod::None, Action::LookForward),
    (KeyCode::KeyL, Mod::None, Action::LookRight),
    (KeyCode::KeyK, Mod::None, Action::LookAft),
    (KeyCode::KeyJ, Mod::None, Action::LookLeft),
    (KeyCode::KeyG, Mod::None, Action::Gear),
    (KeyCode::KeyF, Mod::None, Action::Flaps),
    (KeyCode::KeyB, Mod::None, Action::Airbrake),
    (KeyCode::KeyH, Mod::None, Action::HudMode),
    (KeyCode::F3, Mod::None, Action::MapMode),
    (KeyCode::F4, Mod::None, Action::MfdPage),
    (KeyCode::KeyP, Mod::None, Action::Pause),
    (KeyCode::KeyR, Mod::None, Action::Restart),
    (KeyCode::Escape, Mod::None, Action::Quit),
    (KeyCode::KeyT, Mod::Shift, Action::TimeWarp),
    (KeyCode::F1, Mod::Shift, Action::ViewCockpit),
    (KeyCode::F2, Mod::Shift, Action::ViewChase),
    (KeyCode::F3, Mod::Shift, Action::Retro),
    (KeyCode::F4, Mod::Shift, Action::HudToggle),
    (KeyCode::F5, Mod::Shift, Action::WideView),
    (KeyCode::KeyE, Mod::Ctrl, Action::Eject),
    (KeyCode::KeyI, Mod::Ctrl, Action::CockpitForward),
    (KeyCode::KeyJ, Mod::Ctrl, Action::CockpitLeft),
    (KeyCode::KeyK, Mod::Ctrl, Action::CockpitAft),
    (KeyCode::KeyL, Mod::Ctrl, Action::CockpitRight),
];

/// Tastatur → Aktionen.
pub fn keyboard_actions(
    keys: Res<ButtonInput<KeyCode>>,
    logical: Res<ButtonInput<Key>>,
    mut actions: ResMut<Actions>,
) {
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let active = match (shift, ctrl) {
        (false, false) => Mod::None,
        (true, false) => Mod::Shift,
        (false, true) => Mod::Ctrl,
        (true, true) => return,
    };
    for &(key, m, action) in &KEYS {
        // Escape beendet immer, auch mit Umschalttaste.
        if m == active || action == Action::Quit {
            actions.push(action, keys.just_pressed(key), keys.pressed(key));
        }
    }
    let typed = |c: &str| logical.just_pressed(Key::Character(c.into()));
    actions.push(Action::ZoomIn, typed("y") || typed("Y"), false);
    actions.push(Action::ZoomOut, typed("x") || typed("X"), false);
}

/// Führt alle Aktionen des Frames aus.
#[allow(clippy::too_many_arguments)]
pub fn apply_actions(
    mut actions: ResMut<Actions>,
    mut pilot: ResMut<PilotInput>,
    mut view: ResMut<ViewSettings>,
    mut mfd: ResMut<MfdView>,
    mut state: ResMut<SimState>,
    mut exit: MessageWriter<AppExit>,
) {
    pilot.gun = false;
    pilot.trigger = false;
    let cockpit = |view: &mut ViewSettings, look: LookDir| {
        view.mode = ViewMode::Cockpit;
        view.look = look;
    };
    for (action, pressed, held) in std::mem::take(&mut actions.0) {
        match action {
            Action::Gun => pilot.gun |= held,
            // Nach dem Missionsende blättert der Auslöser die Nachbesprechung.
            Action::Fire if state.sim.mission_over() => view.debrief += usize::from(pressed),
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
            Action::Ecm => pilot.weapons |= weapon_flags::ECM,
            Action::IrJammer => pilot.weapons |= weapon_flags::IR_JAMMER,
            Action::Eject => pilot.weapons |= weapon_flags::EJECT,
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
            Action::CockpitForward | Action::ViewCockpit => cockpit(&mut view, LookDir::Forward),
            Action::CockpitLeft => cockpit(&mut view, LookDir::Left),
            Action::CockpitAft => cockpit(&mut view, LookDir::Aft),
            Action::CockpitRight => cockpit(&mut view, LookDir::Right),
            Action::ViewChase => view.mode = ViewMode::Chase,
            Action::Retro => view.retro = view.retro.next(),
            Action::HudToggle => view.hud = !view.hud,
            Action::WideView => view.wide = !view.wide,
            Action::Pause => state.paused = !state.paused,
            Action::TimeWarp => state.cycle_time_scale(),
            Action::Restart => {
                state.restart();
                *pilot = PilotInput::matching(&state.sim);
                view.debrief = 0;
            }
            Action::Quit => {
                state.save_replay();
                exit.write(AppExit::Success);
            }
        }
    }
}
