//! Klang von F-29 Silent Sniper: prozedurale Synthese im Retro-Stil.
//!
//! Das Crate kennt keine Engine. [`Synth`] erzeugt einen Mono-Datenstrom aus
//! einer [`AudioInput`]-Momentaufnahme der Simulation; das Frontend reicht ihn
//! nur an die Soundkarte weiter. So sind alle Klänge ohne Soundkarte testbar
//! und lassen sich als WAV-Hörproben rendern (`f29-headless sound`).
//!
//! Klänge:
//! - **Triebwerk:** gefiltertes Rauschen (Strahl), zwei leicht verstimmte,
//!   weich gefilterte Rechtecktöne (Turbine) und ein leises Pfeifen. Tonhöhe
//!   folgt der Drehzahl, Pegel und Helligkeit dem Schub.
//! - **Wind:** wächst mit der Fahrt, Luftbremse verstärkt ihn.
//! - **Fahrwerk / Klappen:** Motorbrummen bzw. -surren, solange sie fahren;
//!   das Fahrwerk verriegelt mit einem dumpfen Schlag, die Klappen mit einem
//!   Klicken.
//! - **Luftbremse:** Zischen beim Aus- und Einfahren.
//! - **Aufsetzen:** Reifenquietschen und Schlag (hart: lauter), Absturz: Krachen.
//! - **Abrisswarnung:** gesprochenes „STALL“ in Abständen, solange die
//!   Warnung anliegt. Die Stimme ist ein Sample, das das Frontend durch eine
//!   eigene Aufnahme ersetzen kann.
//!
//! Spätere Musik (z. B. im Startbildschirm) läuft im Frontend als eigener
//! Kanal neben diesem Datenstrom und berührt ihn nicht.

pub mod dsp;
pub mod voice;
pub mod wav;

use dsp::{Noise, OnePole, Phase, pulse, saw, sine, triangle};
use f29_core::{FlightData, Status};
use std::sync::Arc;

/// Abtastrate des Datenstroms.
pub const SAMPLE_RATE: u32 = 44_100;
/// Abstand der gesprochenen Abrisswarnung (s, von Beginn zu Beginn).
pub const STALL_REPEAT_S: f32 = 1.7;

/// Ein Aufsetzen, das einen Klang auslöst.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TouchdownCue {
    /// Eindeutige Kennung (Tick des Aufsetzens) – derselbe Wert klingt nur einmal.
    pub id: u64,
    pub hard: bool,
}

/// Momentaufnahme der Simulation für den Klang.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AudioInput {
    /// Triebwerksstellung 0..1.
    pub engine: f32,
    /// Triebwerk läuft (Treibstoff vorhanden).
    pub running: bool,
    /// Fahrt, m/s.
    pub tas: f32,
    /// Fahrwerk, Klappen, Luftbremse 0..1.
    pub gear: f32,
    pub flaps: f32,
    pub airbrake: f32,
    pub on_ground: bool,
    pub stall_warning: bool,
    pub touchdown: Option<TouchdownCue>,
    pub crashed: bool,
    /// Pause: alles verstummt weich.
    pub paused: bool,
}

impl AudioInput {
    pub fn from_flight(fd: &FlightData, paused: bool) -> Self {
        Self {
            engine: fd.engine as f32,
            running: !fd.warnings.flameout,
            tas: fd.tas_mps as f32,
            gear: fd.gear as f32,
            flaps: fd.flaps as f32,
            airbrake: fd.airbrake as f32,
            on_ground: fd.on_ground,
            stall_warning: fd.warnings.stall,
            touchdown: fd.touchdown.map(|t| TouchdownCue { id: t.tick, hard: t.hard }),
            crashed: matches!(fd.status, Status::Crashed { .. }),
            paused,
        }
    }
}

/// Ein einmaliger Klang.
#[derive(Debug, Clone)]
enum OneShot {
    /// Dumpfer Schlag (Fahrwerk verriegelt, Aufsetzen).
    Thump { t: f32, freq: f32, gain: f32, decay: f32 },
    /// Kurzes Klicken.
    Click { t: f32, gain: f32 },
    /// Zischen (Luftbremse).
    Hiss { t: f32, gain: f32, dur: f32, lp: OnePole },
    /// Reifenquietschen.
    Squeal { t: f32, gain: f32, phase: Phase },
    /// Absturz.
    Crash { t: f32, lp: OnePole },
    /// Abspielen eines Samples (Ansage).
    Sample { pos: usize, gain: f32 },
}

/// Der Synthesizer. Ein Exemplar pro Datenstrom.
#[derive(Debug)]
pub struct Synth {
    sr: f32,
    input: AudioInput,
    noise: Noise,
    stall_voice: Arc<[f32]>,
    // geglättete Parameter
    rpm: OnePole,
    thrust: OnePole,
    wind: OnePole,
    master: OnePole,
    gear_motor: OnePole,
    flap_motor: OnePole,
    roll: OnePole,
    // Oszillatoren und Filter
    tone_a: Phase,
    tone_b: Phase,
    whine: Phase,
    motor: Phase,
    flap: Phase,
    lfo: Phase,
    rumble_lp: OnePole,
    tone_lp: OnePole,
    wind_lp: OnePole,
    wind_hp: OnePole,
    motor_lp: OnePole,
    roll_lp: OnePole,
    // Ereigniserkennung
    prev_gear: f32,
    prev_flaps: f32,
    prev_airbrake: f32,
    last_touchdown: Option<u64>,
    crashed: bool,
    stall_timer: f32,
    shots: Vec<OneShot>,
    volume: f32,
}

impl Synth {
    /// `stall_voice` ist die Ansage für die Abrisswarnung (Mono, [`SAMPLE_RATE`]).
    pub fn new(stall_voice: Arc<[f32]>) -> Self {
        Self {
            sr: SAMPLE_RATE as f32,
            input: AudioInput::default(),
            noise: Noise::new(0x0F29_0A0D),
            stall_voice,
            rpm: OnePole::default(),
            thrust: OnePole::default(),
            wind: OnePole::default(),
            master: OnePole::default(),
            gear_motor: OnePole::default(),
            flap_motor: OnePole::default(),
            roll: OnePole::default(),
            tone_a: Phase::default(),
            tone_b: Phase(0.37),
            whine: Phase::default(),
            motor: Phase::default(),
            flap: Phase::default(),
            lfo: Phase::default(),
            rumble_lp: OnePole::default(),
            tone_lp: OnePole::default(),
            wind_lp: OnePole::default(),
            wind_hp: OnePole::default(),
            motor_lp: OnePole::default(),
            roll_lp: OnePole::default(),
            prev_gear: f32::NAN,
            prev_flaps: f32::NAN,
            prev_airbrake: f32::NAN,
            last_touchdown: None,
            crashed: false,
            stall_timer: 0.0,
            shots: Vec::new(),
            volume: 1.0,
        }
    }

    /// Mit der eingebauten, synthetischen Ansage.
    pub fn with_default_voice() -> Self {
        Self::new(default_stall_voice())
    }

    /// Gesamtlautstärke 0..1.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Neue Momentaufnahme; löst Einmal-Klänge aus (Verriegeln, Aufsetzen …).
    pub fn set_input(&mut self, input: AudioInput) {
        let first = self.prev_gear.is_nan();
        if !first {
            // Fahrwerk erreicht eine Endlage: Schlag
            if self.prev_gear < 0.999 && input.gear >= 0.999 {
                self.shots.push(OneShot::Thump { t: 0.0, freq: 62.0, gain: 0.45, decay: 0.09 });
            } else if self.prev_gear > 0.001 && input.gear <= 0.001 {
                self.shots.push(OneShot::Thump { t: 0.0, freq: 75.0, gain: 0.3, decay: 0.06 });
            }
            // Klappen erreichen eine Endlage: Klick
            let flap_end = |v: f32| v <= 0.001 || v >= 0.999;
            if !flap_end(self.prev_flaps) && flap_end(input.flaps) {
                self.shots.push(OneShot::Click { t: 0.0, gain: 0.25 });
            }
            // Luftbremse beginnt sich zu bewegen: Zischen
            if self.prev_airbrake <= 0.01 && input.airbrake > 0.01 {
                self.shots.push(OneShot::Hiss { t: 0.0, gain: 0.22, dur: 0.6, lp: OnePole::default() });
            } else if self.prev_airbrake >= 0.99 && input.airbrake < 0.99 {
                self.shots.push(OneShot::Hiss { t: 0.0, gain: 0.14, dur: 0.4, lp: OnePole::default() });
            }
        }
        if let Some(td) = input.touchdown
            && self.last_touchdown != Some(td.id)
        {
            self.last_touchdown = Some(td.id);
            if !first {
                let g = if td.hard { 1.6 } else { 1.0 };
                self.shots.push(OneShot::Squeal { t: 0.0, gain: 0.11 * g, phase: Phase::default() });
                self.shots.push(OneShot::Thump { t: 0.0, freq: 52.0, gain: 0.35 * g, decay: 0.12 });
            }
        }
        if input.crashed && !self.crashed && !first {
            self.shots.push(OneShot::Crash { t: 0.0, lp: OnePole::default() });
        }
        self.crashed = input.crashed;
        if !input.stall_warning {
            self.stall_timer = 0.0;
        }
        self.prev_gear = input.gear;
        self.prev_flaps = input.flaps;
        self.prev_airbrake = input.airbrake;
        self.input = input;
    }

    /// Füllt `out` mit Mono-Samples (−1..1).
    pub fn render(&mut self, out: &mut [f32]) {
        let sr = self.sr;
        let dt = 1.0 / sr;
        let inp = self.input;
        // Koeffizienten einmal pro Block
        let a_param = OnePole::coeff_time(0.25, sr);
        let a_rpm = OnePole::coeff_time(if inp.running { 0.6 } else { 2.5 }, sr);
        let a_fast = OnePole::coeff_time(0.04, sr);
        let a_master = OnePole::coeff_time(0.15, sr);

        let rpm_target = if inp.running { 0.55 + 0.45 * inp.engine } else { 0.08 };
        let thrust_target = if inp.running { inp.engine } else { 0.0 };
        let speed = (inp.tas / 260.0).clamp(0.0, 1.5);
        let wind_target = 0.22 * speed * speed + 0.18 * inp.airbrake * speed;
        let gear_moving = if inp.gear > 0.001 && inp.gear < 0.999 { 1.0 } else { 0.0 };
        let flaps_moving = if inp.flaps > 0.001 && inp.flaps < 0.999 { 1.0 } else { 0.0 };
        let roll_target = if inp.on_ground { (inp.tas / 60.0).clamp(0.0, 1.0) } else { 0.0 };
        let master_target = if inp.paused || inp.crashed { 0.0 } else { self.volume };
        // Nach dem Absturz soll das Krachen noch zu hören sein.
        let shots_master = if inp.paused { 0.0 } else { self.volume };

        // Abrisswarnung: Ansage in festen Abständen
        if inp.stall_warning && !inp.paused && !inp.crashed {
            self.stall_timer -= out.len() as f32 * dt;
            if self.stall_timer <= 0.0 {
                self.stall_timer = STALL_REPEAT_S;
                self.shots.push(OneShot::Sample { pos: 0, gain: 0.55 });
            }
        }

        for o in out.iter_mut() {
            let rpm = self.rpm.process(rpm_target, a_rpm);
            let thrust = self.thrust.process(thrust_target, a_param);
            let wind = self.wind.process(wind_target, a_param);
            let master = self.master.process(master_target, a_master);
            let gm = self.gear_motor.process(gear_moving, a_fast);
            let fm = self.flap_motor.process(flaps_moving, a_fast);
            let roll = self.roll.process(roll_target, a_param);
            let n = self.noise.sample();

            // Triebwerk: Strahlrauschen, Turbinenton, Pfeifen
            let rumble_fc = 180.0 + 1_600.0 * thrust;
            let rumble =
                self.rumble_lp.process(n, OnePole::coeff(rumble_fc, sr)) * (0.10 + 0.42 * thrust) * rpm;
            let f0 = 38.0 + 115.0 * rpm;
            let tone = 0.5
                * (pulse(self.tone_a.advance(f0, sr), 0.3) + pulse(self.tone_b.advance(f0 * 1.008, sr), 0.3));
            let tone = self.tone_lp.process(tone, OnePole::coeff(350.0 + 900.0 * rpm, sr)) * 0.075 * rpm;
            let whine = sine(self.whine.advance(480.0 + 1_250.0 * rpm, sr)) * (0.003 + 0.009 * rpm * rpm);
            // Wind: bandbegrenztes Rauschen
            let w_low = self.wind_lp.process(n, OnePole::coeff(1_400.0, sr));
            let w_high = self.wind_hp.process(w_low, OnePole::coeff(250.0, sr));
            let wind_s = (w_low - w_high) * wind;
            // Motoren
            let lfo = sine(self.lfo.advance(7.0, sr));
            let motor_raw = saw(self.motor.advance(92.0 * (1.0 + 0.02 * lfo), sr));
            let motor =
                self.motor_lp.process(motor_raw, OnePole::coeff(420.0, sr)) * 0.11 * gm * (0.8 + 0.2 * lfo);
            let flap = triangle(self.flap.advance(250.0 * (1.0 + 0.025 * lfo), sr)) * 0.035 * fm;
            // Rollgeräusch am Boden
            let rollnoise = self.roll_lp.process(n, OnePole::coeff(120.0, sr)) * 0.35 * roll;

            let mut x = (rumble + tone + whine + wind_s + motor + flap + rollnoise) * master;
            x += self.render_shots(dt) * shots_master;
            *o = x.tanh();
        }
        self.shots.retain(|s| !shot_finished(s, self.stall_voice.len()));
    }

    fn render_shots(&mut self, dt: f32) -> f32 {
        let sr = self.sr;
        let mut sum = 0.0;
        for s in &mut self.shots {
            let n = self.noise.sample();
            sum += match s {
                OneShot::Thump { t, freq, gain, decay } => {
                    let v = (sine(*t * *freq) * (-*t / *decay).exp() + n * (-*t / 0.008).exp() * 0.4) * *gain;
                    *t += dt;
                    v
                }
                OneShot::Click { t, gain } => {
                    let v = n * (-*t / 0.004).exp() * *gain;
                    *t += dt;
                    v
                }
                OneShot::Hiss { t, gain, dur, lp } => {
                    let env = (*t / 0.03).min(1.0) * (1.0 - *t / *dur).max(0.0);
                    let hp = n - lp.process(n, OnePole::coeff(2_500.0, sr));
                    *t += dt;
                    hp * env * *gain
                }
                OneShot::Squeal { t, gain, phase } => {
                    let dur = 0.32;
                    let f = 1_350.0 - 450.0 * (*t / dur);
                    let env = (*t / 0.01).min(1.0) * (1.0 - *t / dur).max(0.0);
                    let v = (sine(phase.advance(f, sr)) * 0.8 + n * 0.25) * env * *gain;
                    *t += dt;
                    v
                }
                OneShot::Crash { t, lp } => {
                    let v = lp.process(n, OnePole::coeff(900.0, sr)) * (-*t / 0.7).exp() * 1.6
                        + sine(*t * 38.0) * (-*t / 0.25).exp() * 0.6;
                    *t += dt;
                    v
                }
                OneShot::Sample { pos, gain } => {
                    let v = self.stall_voice.get(*pos).copied().unwrap_or(0.0) * *gain;
                    *pos += 1;
                    v
                }
            };
        }
        sum
    }

    /// Anzahl gerade klingender Einmal-Klänge (für Tests).
    pub fn active_one_shots(&self) -> usize {
        self.shots.len()
    }
}

fn shot_finished(s: &OneShot, voice_len: usize) -> bool {
    match s {
        OneShot::Thump { t, decay, .. } => *t > decay * 8.0,
        OneShot::Click { t, .. } => *t > 0.05,
        OneShot::Hiss { t, dur, .. } => *t >= *dur,
        OneShot::Squeal { t, .. } => *t >= 0.32,
        OneShot::Crash { t, .. } => *t > 3.0,
        OneShot::Sample { pos, .. } => *pos >= voice_len,
    }
}

/// Die eingebaute Ansage „STALL“.
pub fn default_stall_voice() -> Arc<[f32]> {
    voice::speak(&voice::STALL, SAMPLE_RATE).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: usize = 512;

    fn run(synth: &mut Synth, seconds: f32) -> Vec<f32> {
        let mut out = vec![0.0; (seconds * SAMPLE_RATE as f32) as usize];
        for chunk in out.chunks_mut(BLOCK) {
            synth.render(chunk);
        }
        out
    }

    fn rms(s: &[f32]) -> f32 {
        (s.iter().map(|v| v * v).sum::<f32>() / s.len().max(1) as f32).sqrt()
    }

    fn flying(engine: f32) -> AudioInput {
        AudioInput { engine, running: true, tas: 180.0, ..AudioInput::default() }
    }

    #[test]
    fn engine_gets_louder_with_thrust_and_output_is_bounded() {
        let level = |engine: f32| {
            let mut s = Synth::with_default_voice();
            s.set_input(flying(engine));
            let out = run(&mut s, 3.0);
            assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
            rms(&out[out.len() / 2..])
        };
        let (idle, full) = (level(0.0), level(1.0));
        assert!(full > 1.5 * idle, "Leerlauf {idle}, Vollgas {full}");
        assert!(full < 0.5, "zu laut: {full}");
    }

    #[test]
    fn pause_and_volume_silence_the_stream() {
        let mut s = Synth::with_default_voice();
        s.set_input(AudioInput { paused: true, ..flying(1.0) });
        let out = run(&mut s, 2.0);
        assert!(rms(&out[out.len() / 2..]) < 1e-4);
        let mut s = Synth::with_default_voice();
        s.set_volume(0.0);
        s.set_input(flying(1.0));
        let out = run(&mut s, 2.0);
        assert!(rms(&out[out.len() / 2..]) < 1e-4);
    }

    #[test]
    fn gear_lock_and_touchdown_fire_once() {
        let mut s = Synth::with_default_voice();
        s.set_input(AudioInput { gear: 0.5, ..flying(0.5) });
        run(&mut s, 0.1);
        assert_eq!(s.active_one_shots(), 0);
        s.set_input(AudioInput { gear: 1.0, ..flying(0.5) });
        assert_eq!(s.active_one_shots(), 1, "kein Verriegeln");
        run(&mut s, 1.5);
        assert_eq!(s.active_one_shots(), 0);
        let td = Some(TouchdownCue { id: 42, hard: false });
        s.set_input(AudioInput { gear: 1.0, on_ground: true, touchdown: td, ..flying(0.0) });
        assert_eq!(s.active_one_shots(), 2, "Quietschen + Schlag");
        run(&mut s, 1.5);
        s.set_input(AudioInput { gear: 1.0, on_ground: true, touchdown: td, ..flying(0.0) });
        assert_eq!(s.active_one_shots(), 0, "dasselbe Aufsetzen klingt nur einmal");
    }

    #[test]
    fn stall_warning_speaks_repeatedly_only_while_active() {
        let voice = default_stall_voice();
        let mut s = Synth::new(voice.clone());
        s.set_input(flying(0.6));
        run(&mut s, 1.0);
        assert_eq!(s.active_one_shots(), 0);
        s.set_input(AudioInput { stall_warning: true, ..flying(0.6) });
        let mut starts = 0;
        let mut was = false;
        for _ in 0..(4.0 * SAMPLE_RATE as f32 / BLOCK as f32) as usize {
            let mut buf = [0.0; BLOCK];
            s.render(&mut buf);
            let now = s.active_one_shots() > 0;
            if now && !was {
                starts += 1;
            }
            was = now;
        }
        assert!((2..=3).contains(&starts), "Ansagen in 4 s: {starts}");
        s.set_input(flying(0.6));
        run(&mut s, 2.0);
        assert_eq!(s.active_one_shots(), 0);
    }

    #[test]
    fn crash_is_audible_although_engine_stops() {
        let mut s = Synth::with_default_voice();
        s.set_input(flying(0.8));
        run(&mut s, 1.0);
        s.set_input(AudioInput { crashed: true, ..flying(0.8) });
        let out = run(&mut s, 0.5);
        assert!(rms(&out) > 0.15, "Absturz zu leise: {}", rms(&out));
    }
}
