//! Simulation mit festem Zeitschritt und Interpolation für die Darstellung.

use crate::input::PilotInput;
use bevy::prelude::*;
use f29_core::replay::Replay;
use f29_core::{DQuat, DT, DVec3, Sim, SimConfig};
use std::path::PathBuf;

pub const DEFAULT_SEED: u64 = 0x0F29;
/// Schutz gegen die „Todesspirale“ nach langen Rucklern.
const MAX_STEPS_PER_FRAME: u32 = 24;

pub struct SimPlugin {
    pub config: SimConfig,
    /// Mit der Menüführung beginnen (sonst gleich fliegen).
    pub menu: bool,
}

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        let mut state = SimState::new(self.config.clone());
        state.menu = self.menu.then_some(crate::menu::Screen::Identification);
        let pilot = PilotInput::matching(&state.sim);
        app.insert_resource(state)
            .insert_resource(pilot)
            .add_systems(Update, step_simulation.in_set(SimSystems::Step));
    }
}

/// Reihenfolge innerhalb eines Frames: Eingabe → Simulation → Darstellung.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimSystems {
    Input,
    Step,
    Present,
}

/// Lage und Position zu einem Tick.
#[derive(Debug, Clone, Copy)]
pub struct Pose {
    pub position: DVec3,
    pub attitude: DQuat,
}

impl Pose {
    fn of(sim: &Sim) -> Self {
        let a = sim.aircraft();
        Self { position: a.position, attitude: a.attitude }
    }

    /// Lineare Interpolation der Position, normierte Interpolation der Lage.
    fn lerp(a: Self, b: Self, t: f64) -> Self {
        let mut q = b.attitude;
        let dot = a.attitude.w * q.w + a.attitude.x * q.x + a.attitude.y * q.y + a.attitude.z * q.z;
        if dot < 0.0 {
            q = DQuat::from_wxyz(-q.w, -q.x, -q.y, -q.z);
        }
        let l = |x: f64, y: f64| x + (y - x) * t;
        Self {
            position: a.position + (b.position - a.position) * t,
            attitude: DQuat::from_wxyz(
                l(a.attitude.w, q.w),
                l(a.attitude.x, q.x),
                l(a.attitude.y, q.y),
                l(a.attitude.z, q.z),
            )
            .normalize(),
        }
    }
}

#[derive(Resource, Debug)]
pub struct SimState {
    pub sim: Sim,
    pub paused: bool,
    /// Zeitraffer: Simulationstakte je Echtzeit-Takt (1, 2 oder 4; M-034).
    pub time_scale: u32,
    accumulator: f64,
    prev: Pose,
    curr: Pose,
    recorder: Replay,
    /// Offener Menübildschirm vor dem Flug (M-036) und Cursor darin.
    pub menu: Option<crate::menu::Screen>,
    pub cursor: usize,
    /// Bewaffnung: Fokus auf diesem Schacht statt auf der Liste.
    pub arm_bay: Option<usize>,
}

impl SimState {
    pub fn new(config: SimConfig) -> Self {
        let sim = Sim::new(config);
        let pose = Pose::of(&sim);
        let recorder = Replay::new(sim.config().clone());
        Self {
            sim,
            paused: false,
            time_scale: 1,
            accumulator: 0.0,
            prev: pose,
            curr: pose,
            recorder,
            menu: None,
            cursor: 0,
            arm_bay: None,
        }
    }

    /// Zeitraffer weiterschalten: 1× → 2× → 4× → 1×; nicht, solange ein
    /// Grund zum Abbruch besteht (z. B. Fahrwerk draußen).
    pub fn cycle_time_scale(&mut self) {
        self.time_scale = match self.time_scale {
            1 if !self.sim.interrupts_time_warp() => 2,
            2 => 4,
            _ => 1,
        };
    }

    /// Zwischen den letzten beiden Ticks interpolierte Lage für die Darstellung.
    pub fn render_pose(&self) -> Pose {
        Pose::lerp(self.prev, self.curr, (self.accumulator / DT).clamp(0.0, 1.0))
    }

    /// Neustart mit derselben Konfiguration; der bisherige Flug wird als
    /// Replay gesichert.
    pub fn restart(&mut self) {
        self.reset(self.sim.config().clone());
    }

    /// Neue Simulation mit `config`; das Menü bleibt, wo es ist.
    pub fn reset(&mut self, config: SimConfig) {
        self.save_replay();
        let (menu, cursor, arm_bay) = (self.menu, self.cursor, self.arm_bay);
        *self = Self::new(config);
        (self.menu, self.cursor, self.arm_bay) = (menu, cursor, arm_bay);
    }

    /// Deckt ein Bildschirm (Menü oder Nachbesprechung) die Ansicht zu?
    pub fn covered(&self) -> bool {
        self.menu.is_some() || self.sim.mission_over()
    }

    /// Schreibt den bisherigen Flug nach `replays/last.replay`.
    pub fn save_replay(&mut self) {
        if self.recorder.tick_count() == 0 {
            return;
        }
        self.recorder.final_hash = Some(self.sim.state_hash());
        let dir = PathBuf::from("replays");
        let path = dir.join("last.replay");
        let result =
            std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, self.recorder.to_text()));
        match result {
            Ok(()) => info!(
                "Replay gespeichert: {} ({} Ticks) – abspielen mit `cargo run -p f29-headless -- replay {}`",
                path.display(),
                self.recorder.tick_count(),
                path.display()
            ),
            Err(e) => warn!("Replay konnte nicht gespeichert werden: {e}"),
        }
    }
}

fn step_simulation(
    time: Res<Time>,
    mut state: ResMut<SimState>,
    mut pilot: ResMut<PilotInput>,
    view: Res<crate::view::ViewSettings>,
) {
    // Menü bzw. Nachbesprechung: Die Simulation steht still.
    if state.paused || state.covered() {
        return;
    }
    let state = &mut *state;
    state.accumulator += time.delta_secs_f64().min(0.25) * f64::from(state.time_scale);
    let mut steps = 0;
    while state.accumulator >= DT && steps < MAX_STEPS_PER_FRAME {
        // Beschädigtes Triebwerk: Der Hebel fällt auf den verfügbaren
        // Höchstschub zurück und lässt sich nicht darüber schieben.
        let cap = state.sim.systems().capability(f29_core::systems::System::Engine);
        pilot.throttle = pilot.throttle.min(cap);
        let frame = pilot.take_frame(view.hud_mode);
        state.prev = state.curr;
        state.sim.step(frame);
        state.recorder.push(frame);
        state.curr = Pose::of(&state.sim);
        state.accumulator -= DT;
        steps += 1;
        if state.time_scale > 1 && state.sim.interrupts_time_warp() {
            state.time_scale = 1;
            state.accumulator = state.accumulator.min(DT);
        }
    }
    if steps == MAX_STEPS_PER_FRAME {
        // Rückstand verwerfen statt ihn aufzuholen
        state.accumulator = state.accumulator.min(DT);
    }
}
