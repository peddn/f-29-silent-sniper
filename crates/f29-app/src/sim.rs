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
}

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        let state = SimState::new(self.config.clone());
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
    accumulator: f64,
    prev: Pose,
    curr: Pose,
    recorder: Replay,
}

impl SimState {
    pub fn new(config: SimConfig) -> Self {
        let sim = Sim::new(config);
        let pose = Pose::of(&sim);
        let recorder = Replay::new(sim.config().clone());
        Self { sim, paused: false, accumulator: 0.0, prev: pose, curr: pose, recorder }
    }

    /// Zwischen den letzten beiden Ticks interpolierte Lage für die Darstellung.
    pub fn render_pose(&self) -> Pose {
        Pose::lerp(self.prev, self.curr, (self.accumulator / DT).clamp(0.0, 1.0))
    }

    /// Neustart mit derselben Konfiguration; der bisherige Flug wird als
    /// Replay gesichert.
    pub fn restart(&mut self) {
        self.save_replay();
        *self = Self::new(self.sim.config().clone());
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

fn step_simulation(time: Res<Time>, mut state: ResMut<SimState>, mut pilot: ResMut<PilotInput>) {
    if state.paused {
        return;
    }
    let state = &mut *state;
    state.accumulator += time.delta_secs_f64().min(0.25);
    let mut steps = 0;
    while state.accumulator >= DT && steps < MAX_STEPS_PER_FRAME {
        let frame = pilot.take_frame();
        state.prev = state.curr;
        state.sim.step(frame);
        state.recorder.push(frame);
        state.curr = Pose::of(&state.sim);
        state.accumulator -= DT;
        steps += 1;
    }
    if steps == MAX_STEPS_PER_FRAME {
        // Rückstand verwerfen statt ihn aufzuholen
        state.accumulator = state.accumulator.min(DT);
    }
}
