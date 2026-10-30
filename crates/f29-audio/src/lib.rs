//! Klang von F-29 Silent Sniper: prozedurale Synthese im Retro-Stil.
//!
//! Das Crate kennt keine Engine. [`Synth`] erzeugt einen Mono-Datenstrom aus
//! einer [`AudioInput`]-Momentaufnahme der Simulation; das Frontend reicht ihn
//! nur an die Soundkarte weiter. So sind alle Klänge ohne Soundkarte testbar
//! und lassen sich als WAV-Hörproben rendern (`f29-headless sound`).
//!
//! Klänge:
//! - **Triebwerk:** zwei leicht verstimmte, weich gefilterte Rechtecktöne
//!   (Brummen, Tonhöhe nach Drehzahl), dazu dunkles, im Takt des Brummens
//!   pulsierendes Rauschen (Strahl, Pegel und Helligkeit nach Schub) und ein
//!   leises Pfeifen. Am Boden ein tiefes Rollgeräusch.
//! - **Fahrwerk / Klappen:** Motorbrummen bzw. -surren, solange sie fahren;
//!   das Fahrwerk verriegelt mit einem dumpfen Schlag, die Klappen mit einem
//!   Klicken.
//! - **Luftbremse:** Zischen beim Aus- und Einfahren.
//! - **Aufsetzen:** Reifenquietschen und Schlag (hart: lauter), Absturz: Krachen.
//! - **Abrisswarnung:** hoher, pulsierender Alarmton, solange die Warnung
//!   anliegt.
//! - **Ansagen „PULL UP“ und „OVER G“:** Samples (`sounds/*.wav`, im Crate
//!   eingebettet), in Abständen, solange die Warnung anliegt; die
//!   Bodenwarnung hat Vorrang.
//! - **Detonationen:** tiefer, abfallender Bass mit verzerrtem, knisterndem
//!   Krachen; Treffer am eigenen Flugzeug zusätzlich metallisch.
//!
//! Spätere Musik (z. B. im Startbildschirm) läuft im Frontend als eigener
//! Kanal neben diesem Datenstrom und berührt ihn nicht.

pub mod dsp;
pub mod wav;

use dsp::{Noise, OnePole, Phase, pulse, saw, sine, triangle};
pub use f29_core::BlastKind;
use f29_core::{FlightData, Status};
use std::sync::Arc;

/// Abtastrate des Datenstroms.
pub const SAMPLE_RATE: u32 = 44_100;
/// Abstand der Ansagen (s, von Beginn zu Beginn).
pub const PULL_UP_REPEAT_S: f32 = 1.6;
/// Eingebettete Ansagen (PCM-WAV, mit espeak-ng erzeugt und bearbeitet).
const PULL_UP_WAV: &[u8] = include_bytes!("../sounds/pull_up.wav");
const OVER_G_WAV: &[u8] = include_bytes!("../sounds/over_g.wav");
/// Tonhöhe des Abriss-Alarms (Hz) und seine Pulsfrequenz (Hz).
pub const STALL_ALARM_HZ: f32 = 1_250.0;
pub const STALL_PULSE_HZ: f32 = 6.0;
/// Raketenwarnung: Wechselton in diesem Takt (s je Ton) und Frequenzen.
const MISSILE_BEEP_S: f32 = 0.12;
const MISSILE_TONES_HZ: [f32; 2] = [2_100.0, 1_500.0];

/// Ein Aufsetzen, das einen Klang auslöst.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TouchdownCue {
    /// Eindeutige Kennung (Tick des Aufsetzens) – derselbe Wert klingt nur einmal.
    pub id: u64,
    pub hard: bool,
}

/// Ein Radarping, der das Flugzeug erreicht hat (M-020).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PingCue {
    /// Eindeutige Kennung (Tick des Pings) – derselbe Wert klingt nur einmal.
    pub id: u64,
    /// Treffer: Der Impuls war stärker als das Stealth-Level.
    pub hit: bool,
}

/// Eine Detonation, die einen Klang auslöst.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlastCue {
    /// Eindeutige Kennung – dieselbe Detonation klingt nur einmal.
    pub id: u64,
    pub kind: BlastKind,
    /// Lautstärke nach Entfernung 0..1 (Treffer am Flugzeug immer 1).
    pub gain: f32,
}

/// Entfernung (m), in der eine Detonation nur noch halb so laut ist.
const BLAST_HALF_M: f64 = 1_500.0;
/// Leiser als so klingt eine ferne Detonation nicht.
const BLAST_MIN_GAIN: f32 = 0.15;

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
    /// Bodenwarnung (PULL UP).
    pub pull_up_warning: bool,
    /// Überlastung (OVER G).
    pub over_g_warning: bool,
    pub touchdown: Option<TouchdownCue>,
    /// Jüngster Radarping (Radarwarnempfänger).
    pub ping: Option<PingCue>,
    /// Raketenwarnung: Eine feindliche Rakete verfolgt das Flugzeug.
    pub missile_warning: bool,
    /// Jüngste Detonation.
    pub blast: Option<BlastCue>,
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
            pull_up_warning: fd.warnings.pull_up,
            over_g_warning: fd.warnings.over_g,
            touchdown: fd.touchdown.map(|t| TouchdownCue { id: t.tick, hard: t.hard }),
            ping: fd.threat.last_ping.filter(|p| p.age_s < 0.5).map(|p| PingCue { id: p.tick, hit: p.hit }),
            missile_warning: fd.warnings.missile,
            blast: fd.blast.map(|b| BlastCue {
                id: b.id,
                kind: b.kind,
                gain: if b.kind == BlastKind::AircraftHit {
                    1.0
                } else {
                    ((1.0 / (1.0 + b.distance_m / BLAST_HALF_M)) as f32).max(BLAST_MIN_GAIN)
                },
            }),
            crashed: matches!(fd.status, Status::Crashed { .. }),
            paused,
        }
    }
}

/// Pegel der Klanggruppen 0..1 – für Hörproben einzelner Klänge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mix {
    /// Triebwerk und Rollgeräusch.
    pub engine: f32,
    /// Fahrwerks- und Klappenmotoren.
    pub systems: f32,
    /// Einmal-Klänge (Verriegeln, Zischen, Aufsetzen, Absturz).
    pub events: f32,
    /// Warnungen: Abriss-Alarm und Ansagen.
    pub voice: f32,
}

impl Default for Mix {
    fn default() -> Self {
        Self { engine: 1.0, systems: 1.0, events: 1.0, voice: 1.0 }
    }
}

impl Mix {
    /// Alles außer dem Triebwerk.
    pub const WITHOUT_ENGINE: Mix = Mix { engine: 0.0, systems: 1.0, events: 1.0, voice: 1.0 };
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
    Sample { data: Arc<[f32]>, pos: usize, gain: f32 },
    /// Piepton des Radarwarnempfängers.
    Beep { t: f32, freq: f32, gain: f32, dur: f32, phase: Phase },
    /// Detonation: tiefer, abfallender Bass, verzerrtes dunkles Rauschen und
    /// Knistern; `ring` > 0 gibt einen metallischen Klang dazu (Treffer am
    /// eigenen Flugzeug).
    Boom { t: f32, gain: f32, decay: f32, cutoff: f32, ring: f32, lp: OnePole, lp2: OnePole, bass: Phase },
}

/// Der Synthesizer. Ein Exemplar pro Datenstrom.
#[derive(Debug)]
pub struct Synth {
    sr: f32,
    input: AudioInput,
    noise: Noise,
    pull_up: Arc<[f32]>,
    over_g: Arc<[f32]>,
    // geglättete Parameter
    rpm: OnePole,
    thrust: OnePole,
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
    rumble_lp2: OnePole,
    motor_lp: OnePole,
    roll_lp: OnePole,
    // Ereigniserkennung
    prev_gear: f32,
    prev_flaps: f32,
    prev_airbrake: f32,
    last_touchdown: Option<u64>,
    last_ping: Option<u64>,
    last_blast: Option<u64>,
    crashed: bool,
    pull_up_timer: f32,
    missile_timer: f32,
    missile_tone: usize,
    alarm: Phase,
    alarm_pulse: Phase,
    alarm_gate: OnePole,
    alarm_lp: OnePole,
    shots: Vec<OneShot>,
    volume: f32,
    mix: Mix,
}

impl Synth {
    pub fn new() -> Self {
        let sample = |wav: &[u8]| -> Arc<[f32]> {
            wav::read_mono(wav, SAMPLE_RATE).expect("eingebettetes PCM-WAV").into()
        };
        Self {
            sr: SAMPLE_RATE as f32,
            input: AudioInput::default(),
            noise: Noise::new(0x0F29_0A0D),
            pull_up: sample(PULL_UP_WAV),
            over_g: sample(OVER_G_WAV),
            rpm: OnePole::default(),
            thrust: OnePole::default(),
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
            rumble_lp2: OnePole::default(),
            motor_lp: OnePole::default(),
            roll_lp: OnePole::default(),
            prev_gear: f32::NAN,
            prev_flaps: f32::NAN,
            prev_airbrake: f32::NAN,
            last_touchdown: None,
            last_ping: None,
            last_blast: None,
            crashed: false,
            pull_up_timer: 0.0,
            missile_timer: 0.0,
            missile_tone: 0,
            alarm: Phase::default(),
            alarm_pulse: Phase::default(),
            alarm_gate: OnePole::default(),
            alarm_lp: OnePole::default(),
            shots: Vec::new(),
            volume: 1.0,
            mix: Mix::default(),
        }
    }

    /// Gesamtlautstärke 0..1.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Pegel der Klanggruppen (Standard: alle voll).
    pub fn set_mix(&mut self, mix: Mix) {
        self.mix = mix;
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
        // Radarping: leiser kurzer Ton; ein Treffer doppelt und höher.
        if let Some(p) = input.ping
            && self.last_ping != Some(p.id)
        {
            self.last_ping = Some(p.id);
            if !first {
                if p.hit {
                    self.shots.push(OneShot::Beep {
                        t: 0.0,
                        freq: 1_700.0,
                        gain: 0.10,
                        dur: 0.07,
                        phase: Phase::default(),
                    });
                    self.shots.push(OneShot::Beep {
                        t: -0.11,
                        freq: 1_700.0,
                        gain: 0.10,
                        dur: 0.07,
                        phase: Phase::default(),
                    });
                } else {
                    self.shots.push(OneShot::Beep {
                        t: 0.0,
                        freq: 950.0,
                        gain: 0.05,
                        dur: 0.05,
                        phase: Phase::default(),
                    });
                }
            }
        }
        // Detonationen: Treffer am Ziel lang und kräftig, Fehlschuss kurz und
        // dumpf, Treffer am eigenen Flugzeug laut, scharf und metallisch.
        if let Some(b) = input.blast
            && self.last_blast != Some(b.id)
        {
            self.last_blast = Some(b.id);
            if !first {
                let g = b.gain;
                let boom = |gain: f32, decay: f32, cutoff: f32, ring: f32| OneShot::Boom {
                    t: 0.0,
                    gain,
                    decay,
                    cutoff,
                    ring,
                    lp: OnePole::default(),
                    lp2: OnePole::default(),
                    bass: Phase::default(),
                };
                self.shots.push(match b.kind {
                    BlastKind::TargetHit => boom(1.0 * g, 1.4, 700.0, 0.0),
                    BlastKind::Miss => boom(0.7 * g, 0.45, 350.0, 0.0),
                    BlastKind::AircraftHit => boom(1.3, 0.8, 1_500.0, 640.0),
                });
            }
        }
        if input.crashed && !self.crashed && !first {
            self.shots.push(OneShot::Crash { t: 0.0, lp: OnePole::default() });
        }
        self.crashed = input.crashed;
        if !input.pull_up_warning && !input.over_g_warning {
            self.pull_up_timer = 0.0;
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
        let gear_moving = if inp.gear > 0.001 && inp.gear < 0.999 { 1.0 } else { 0.0 };
        let flaps_moving = if inp.flaps > 0.001 && inp.flaps < 0.999 { 1.0 } else { 0.0 };
        let roll_target = if inp.on_ground { (inp.tas / 60.0).clamp(0.0, 1.0) } else { 0.0 };
        let master_target = if inp.paused || inp.crashed { 0.0 } else { self.volume };
        // Nach dem Absturz soll das Krachen noch zu hören sein.
        let shots_master = if inp.paused { 0.0 } else { self.volume };

        // Ansagen in festen Abständen, die Bodenwarnung hat Vorrang.
        let voice = if inp.pull_up_warning {
            Some(&self.pull_up)
        } else if inp.over_g_warning {
            Some(&self.over_g)
        } else {
            None
        };
        if let Some(data) = voice.filter(|_| !inp.paused && !inp.crashed) {
            let data = data.clone();
            self.pull_up_timer -= out.len() as f32 * dt;
            if self.pull_up_timer <= 0.0 {
                self.pull_up_timer = PULL_UP_REPEAT_S;
                self.shots.push(OneShot::Sample { data, pos: 0, gain: 0.7 });
            }
        }
        // Raketenwarnung: schneller Wechselton aus kurzen Piepsern
        if inp.missile_warning && !inp.paused && !inp.crashed {
            self.missile_timer -= out.len() as f32 * dt;
            if self.missile_timer <= 0.0 {
                self.missile_timer += MISSILE_BEEP_S;
                self.missile_tone ^= 1;
                let freq = MISSILE_TONES_HZ[self.missile_tone];
                self.shots.push(OneShot::Beep {
                    t: 0.0,
                    freq,
                    gain: 0.09,
                    dur: MISSILE_BEEP_S * 0.8,
                    phase: Phase::default(),
                });
            }
        } else {
            self.missile_timer = 0.0;
        }
        let alarm_on = inp.stall_warning && !inp.paused && !inp.crashed;
        let a_gate = OnePole::coeff_time(0.004, sr);
        let a_alarm_lp = OnePole::coeff(3_500.0, sr);

        for o in out.iter_mut() {
            let rpm = self.rpm.process(rpm_target, a_rpm);
            let thrust = self.thrust.process(thrust_target, a_param);
            let master = self.master.process(master_target, a_master);
            let gm = self.gear_motor.process(gear_moving, a_fast);
            let fm = self.flap_motor.process(flaps_moving, a_fast);
            let roll = self.roll.process(roll_target, a_param);
            let n = self.noise.sample();

            // Triebwerk: Brummen (Turbinenton), dunkles Strahlrauschen im Takt
            // des Brummens, leises Pfeifen
            let f0 = 38.0 + 115.0 * rpm;
            let buzz = 0.5
                * (pulse(self.tone_a.advance(f0, sr), 0.3) + pulse(self.tone_b.advance(f0 * 1.008, sr), 0.3));
            let tone = self.tone_lp.process(buzz, OnePole::coeff(350.0 + 900.0 * rpm, sr)) * 0.075 * rpm;
            let rumble_fc = OnePole::coeff(90.0 + 380.0 * thrust, sr);
            let dark = self.rumble_lp2.process(self.rumble_lp.process(n, rumble_fc), rumble_fc);
            let rumble = dark * (0.65 + 0.35 * buzz) * (0.10 + 0.28 * thrust) * rpm;
            let whine = sine(self.whine.advance(480.0 + 1_250.0 * rpm, sr)) * (0.003 + 0.009 * rpm * rpm);
            // Motoren
            let lfo = sine(self.lfo.advance(7.0, sr));
            let motor_raw = saw(self.motor.advance(92.0 * (1.0 + 0.02 * lfo), sr));
            let motor =
                self.motor_lp.process(motor_raw, OnePole::coeff(420.0, sr)) * 0.11 * gm * (0.8 + 0.2 * lfo);
            let flap = triangle(self.flap.advance(250.0 * (1.0 + 0.025 * lfo), sr)) * 0.035 * fm;
            // Rollgeräusch am Boden
            let rollnoise = self.roll_lp.process(n, OnePole::coeff(120.0, sr)) * 0.35 * roll;

            // Abriss-Alarm: weiches Rechteck, im Takt an und aus (40 % an)
            let pulse_on = alarm_on && self.alarm_pulse.advance(STALL_PULSE_HZ, sr) < 0.4;
            let gate = self.alarm_gate.process(if pulse_on { 1.0 } else { 0.0 }, a_gate);
            let beep = self.alarm_lp.process(pulse(self.alarm.advance(STALL_ALARM_HZ, sr), 0.5), a_alarm_lp);
            let alarm = beep * gate * 0.13 * self.mix.voice;
            if !alarm_on && gate < 1e-4 {
                self.alarm_pulse.0 = 0.0;
            }

            let engine = (rumble + tone + whine + rollnoise) * self.mix.engine;
            let systems = (motor + flap) * self.mix.systems;
            let mut x = (engine + systems) * master;
            x += self.render_shots(dt) * shots_master + alarm * shots_master;
            *o = x.tanh();
        }
        self.shots.retain(|s| !shot_finished(s));
    }

    fn render_shots(&mut self, dt: f32) -> f32 {
        let sr = self.sr;
        let mut sum = 0.0;
        let mix = self.mix;
        for s in &mut self.shots {
            let n = self.noise.sample();
            let group =
                if matches!(s, OneShot::Sample { .. } | OneShot::Beep { .. }) { 1.0 } else { mix.events };
            sum += group
                * match s {
                    OneShot::Thump { t, freq, gain, decay } => {
                        let v =
                            (sine(*t * *freq) * (-*t / *decay).exp() + n * (-*t / 0.008).exp() * 0.4) * *gain;
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
                    OneShot::Boom { t, gain, decay, cutoff, ring, lp, lp2, bass } => {
                        // Bass: Druckstoß, der von 90 auf 30 Hz abfällt
                        let f = 30.0 + 60.0 * (-*t / 0.15).exp();
                        let thump = sine(bass.advance(f, sr)) * (-*t / (*decay * 0.5)).exp() * 1.3;
                        // Krachen: dunkles Rauschen, hart übersteuert, dazu
                        // vereinzelte Knister-Impulse
                        let a = OnePole::coeff(*cutoff, sr);
                        let dark = lp2.process(lp.process(n, a), a);
                        let crunch = (dark * 9.0).tanh() * (-*t / *decay).exp() * 0.55;
                        let crackle = if n.abs() > 0.995 {
                            n.signum() * 0.5 * (-*t / (*decay * 0.6)).exp()
                        } else {
                            0.0
                        };
                        let metal = if *ring > 0.0 {
                            (sine(*t * *ring) + 0.5 * sine(*t * *ring * 2.76)) * (-*t / 0.4).exp() * 0.3
                        } else {
                            0.0
                        };
                        let v = (thump + crunch + crackle + metal) * *gain;
                        *t += dt;
                        v
                    }
                    OneShot::Sample { data, pos, gain } => {
                        let v = data.get(*pos).copied().unwrap_or(0.0) * *gain * mix.voice;
                        *pos += 1;
                        v
                    }
                    OneShot::Beep { t, freq, gain, dur, phase } => {
                        // Negative Zeit = Verzögerung (zweiter Ton)
                        let v = if *t >= 0.0 {
                            let env = (*t / 0.004).min(1.0) * ((*dur - *t) / 0.01).clamp(0.0, 1.0);
                            pulse(phase.advance(*freq, sr), 0.5) * env * *gain * mix.voice
                        } else {
                            0.0
                        };
                        *t += dt;
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

fn shot_finished(s: &OneShot) -> bool {
    match s {
        OneShot::Thump { t, decay, .. } => *t > decay * 8.0,
        OneShot::Click { t, .. } => *t > 0.05,
        OneShot::Hiss { t, dur, .. } => *t >= *dur,
        OneShot::Squeal { t, .. } => *t >= 0.32,
        OneShot::Crash { t, .. } => *t > 3.0,
        OneShot::Boom { t, decay, .. } => *t > *decay * 7.0,
        OneShot::Sample { data, pos, .. } => *pos >= data.len(),
        OneShot::Beep { t, dur, .. } => *t >= *dur,
    }
}

impl Default for Synth {
    fn default() -> Self {
        Self::new()
    }
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
            let mut s = Synth::new();
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
        let mut s = Synth::new();
        s.set_input(AudioInput { paused: true, ..flying(1.0) });
        let out = run(&mut s, 2.0);
        assert!(rms(&out[out.len() / 2..]) < 1e-4);
        let mut s = Synth::new();
        s.set_volume(0.0);
        s.set_input(flying(1.0));
        let out = run(&mut s, 2.0);
        assert!(rms(&out[out.len() / 2..]) < 1e-4);
    }

    #[test]
    fn gear_lock_and_touchdown_fire_once() {
        let mut s = Synth::new();
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

    /// Detonationen: jede klingt einmal; Treffer am Flugzeug lauter als ein
    /// ferner Fehlschuss.
    #[test]
    fn blasts_sound_once_and_by_kind() {
        let level = |kind: BlastKind, gain: f32| {
            let mut s = Synth::new();
            s.set_mix(Mix::WITHOUT_ENGINE);
            s.set_input(flying(0.6));
            run(&mut s, 0.1);
            let cue = Some(BlastCue { id: 7, kind, gain });
            s.set_input(AudioInput { blast: cue, ..flying(0.6) });
            assert_eq!(s.active_one_shots(), 1);
            let v = rms(&run(&mut s, 0.5));
            s.set_input(AudioInput { blast: cue, ..flying(0.6) });
            assert_eq!(s.active_one_shots(), 1, "dieselbe Detonation klingt nur einmal");
            v
        };
        let hit = level(BlastKind::AircraftHit, 1.0);
        let target = level(BlastKind::TargetHit, 0.5);
        let miss = level(BlastKind::Miss, 0.2);
        assert!(hit > target && target > miss && miss > 0.0, "{hit} {target} {miss}");
    }

    #[test]
    fn missile_warning_beeps_only_while_active() {
        let mut s = Synth::new();
        s.set_mix(Mix::WITHOUT_ENGINE);
        s.set_input(flying(0.6));
        assert!(rms(&run(&mut s, 0.5)) < 1e-4);
        s.set_input(AudioInput { missile_warning: true, ..flying(0.6) });
        assert!(rms(&run(&mut s, 1.0)) > 0.02);
        s.set_input(flying(0.6));
        run(&mut s, 0.3);
        assert!(rms(&run(&mut s, 0.5)) < 1e-4);
    }

    #[test]
    fn stall_alarm_pulses_only_while_active() {
        let mut s = Synth::new();
        s.set_mix(Mix::WITHOUT_ENGINE);
        s.set_input(flying(0.6));
        let quiet = run(&mut s, 0.5);
        assert!(rms(&quiet) < 1e-4);
        s.set_input(AudioInput { stall_warning: true, ..flying(0.6) });
        let on = run(&mut s, 2.0);
        // Pulsieren: Fenster à 20 ms wechseln zwischen laut und still
        let win = SAMPLE_RATE as usize / 50;
        let levels: Vec<f32> = on.chunks(win).map(rms).collect();
        let loud = levels.iter().filter(|l| **l > 0.03).count();
        let silent = levels.iter().filter(|l| **l < 0.005).count();
        assert!(loud > 20 && silent > 20, "laut {loud}, still {silent}");
        // hoch: viele Nulldurchgänge
        let zc = on.windows(2).filter(|p| (p[0] < 0.0) != (p[1] < 0.0)).count() as f32 / 2.0;
        assert!(zc > 800.0, "Nulldurchgänge {zc}");
        assert_eq!(s.active_one_shots(), 0, "Abriss spricht nicht");
        s.set_input(flying(0.6));
        run(&mut s, 0.1);
        let after = run(&mut s, 0.5);
        assert!(rms(&after) < 1e-4);
    }

    /// OVER G spricht wie PULL UP; liegen beide an, hat PULL UP Vorrang.
    #[test]
    fn over_g_speaks_and_pull_up_has_priority() {
        let first_sample = |input: AudioInput| {
            let mut s = Synth::new();
            s.set_input(flying(0.6));
            run(&mut s, 0.1);
            s.set_input(input);
            run(&mut s, 0.05);
            s.shots.iter().find_map(|o| match o {
                OneShot::Sample { data, .. } => Some(data.len()),
                _ => None,
            })
        };
        let s = Synth::new();
        let over_g = first_sample(AudioInput { over_g_warning: true, ..flying(0.6) });
        assert_eq!(over_g, Some(s.over_g.len()));
        let both = first_sample(AudioInput { over_g_warning: true, pull_up_warning: true, ..flying(0.6) });
        assert_eq!(both, Some(s.pull_up.len()));
        assert_ne!(s.over_g.len(), s.pull_up.len());
    }

    #[test]
    fn pull_up_speaks_repeatedly_only_while_active() {
        let mut s = Synth::new();
        s.set_input(flying(0.6));
        run(&mut s, 1.0);
        assert_eq!(s.active_one_shots(), 0);
        s.set_input(AudioInput { pull_up_warning: true, ..flying(0.6) });
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
    fn no_wind_and_engine_noise_is_dark() {
        // Ohne Triebwerk und ohne bewegte Teile ist es in der Luft still – auch
        // bei hoher Fahrt und ausgefahrener Luftbremse (kein Wind).
        let mut s = Synth::new();
        s.set_mix(Mix::WITHOUT_ENGINE);
        s.set_input(AudioInput { airbrake: 1.0, tas: 300.0, ..flying(1.0) });
        run(&mut s, 1.0);
        let out = run(&mut s, 1.0);
        assert!(rms(&out) < 1e-4, "Restgeräusch {}", rms(&out));
        // Triebwerk bei Vollgas: wenig Energie oberhalb von 2 kHz (Nulldurchgänge)
        let mut s = Synth::new();
        s.set_input(flying(1.0));
        let out = run(&mut s, 2.0);
        let tail = &out[out.len() / 2..];
        let zc = tail.windows(2).filter(|p| (p[0] < 0.0) != (p[1] < 0.0)).count() as f32 / 1.0;
        assert!(zc < 1_500.0, "zu hell: {zc} Nulldurchgänge/s");
    }

    #[test]
    fn crash_is_audible_although_engine_stops() {
        let mut s = Synth::new();
        s.set_input(flying(0.8));
        run(&mut s, 1.0);
        s.set_input(AudioInput { crashed: true, ..flying(0.8) });
        let out = run(&mut s, 0.5);
        assert!(rms(&out) > 0.15, "Absturz zu leise: {}", rms(&out));
    }
}
