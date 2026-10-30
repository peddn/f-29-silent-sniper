//! Tastatur → quantisierte Steuereingaben (Joystick: `joystick.rs`).

use crate::mfd::MfdView;
use crate::sim::{SimState, SimSystems};
use crate::view::{ViewMode, ViewSettings};
use bevy::app::AppExit;
use bevy::input::keyboard::Key;
use bevy::prelude::*;
use f29_core::ControlFrame;
use f29_core::controls::{flags, weapon_flags};
use f29_core::sensors::LookDir;

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
        if std::mem::take(&mut self.radar_toggle) {
            f |= flags::RADAR_TOGGLE;
        }
        f |= ControlFrame::look_flags(std::mem::take(&mut self.look));
        ControlFrame::from_analog(self.pitch, self.roll, self.yaw, self.throttle, f).with_weapons(
            std::mem::take(&mut self.weapons)
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
    // Zielerfassung: V Radar, I/L/K/J Blickrichtung voraus/rechts/hinten/links
    if keys.just_pressed(KeyCode::KeyV) {
        pilot.radar_toggle = true;
    }
    for (key, dir) in [
        (KeyCode::KeyI, LookDir::Forward),
        (KeyCode::KeyL, LookDir::Right),
        (KeyCode::KeyK, LookDir::Aft),
        (KeyCode::KeyJ, LookDir::Left),
    ] {
        if keys.just_pressed(key) {
            pilot.look = dir.code();
        }
    }

    // Bordkanone: Rücktaste (gehalten) feuert sie immer; die Leertaste
    // feuert die gewählte Waffe – gehalten die Kanone, wenn sie gewählt ist.
    pilot.gun = keys.pressed(KeyCode::Backspace);
    pilot.trigger = keys.pressed(KeyCode::Space);

    // Waffen: Leertaste auslösen, O Schacht auf/zu, Tab nächste Waffe (GUN,
    // BAY 1–4); C Düppel, T Fackel, M Störsender ein/aus, U Infrarotstörer
    for (key, flag) in [
        (KeyCode::Space, weapon_flags::RELEASE),
        (KeyCode::KeyO, weapon_flags::BAY_DOORS),
        (KeyCode::Tab, weapon_flags::NEXT_BAY),
        (KeyCode::KeyC, weapon_flags::CHAFF),
        (KeyCode::KeyT, weapon_flags::FLARE),
        (KeyCode::KeyM, weapon_flags::ECM),
        (KeyCode::KeyU, weapon_flags::IR_JAMMER),
    ] {
        if keys.just_pressed(key) {
            pilot.weapons |= flag;
        }
    }
}

fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    logical: Res<ButtonInput<Key>>,
    mut state: ResMut<SimState>,
    mut view: ResMut<ViewSettings>,
    mut mfd: ResMut<MfdView>,
    mut pilot: ResMut<PilotInput>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        state.save_replay();
        exit.write(AppExit::Success);
    }
    if keys.just_pressed(KeyCode::KeyP) {
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
    if shift && keys.just_pressed(KeyCode::F3) {
        view.retro = view.retro.next();
    }
    if shift && keys.just_pressed(KeyCode::F4) {
        view.hud = !view.hud;
    }
    // HUD-Modus: NAV → AG → AA
    if keys.just_pressed(KeyCode::KeyH) {
        view.hud_mode = view.hud_mode.next();
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
