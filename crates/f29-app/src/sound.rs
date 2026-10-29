//! Klang: der Datenstrom aus `f29-audio` als eigene Bevy-Audioquelle.
//!
//! Die Simulation schreibt jedes Bild eine Momentaufnahme in einen geteilten
//! Speicher; der Audio-Thread liest sie blockweise und synthetisiert. Musik
//! (z. B. im Startbildschirm) kann später als gewöhnlicher `AudioPlayer` mit
//! einer Ogg-Datei daneben laufen – dafür genügt das Bevy-Feature `vorbis`.

use crate::sim::{SimState, SimSystems};
use bevy::audio::{AddAudioSource, ChannelCount, Decodable, SampleRate, Source};
use bevy::prelude::*;
use f29_audio::{AudioInput, SAMPLE_RATE, Synth};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Samples pro Block, nach dem der Audio-Thread neue Eingaben übernimmt (≈ 6 ms).
const BLOCK: usize = 256;

pub struct SoundPlugin {
    /// Gesamtlautstärke 0..1.
    pub volume: f32,
}

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        let link = SoundLink { shared: Arc::new(Mutex::new(AudioInput::default())), volume: self.volume };
        app.add_audio_source::<SynthStream>()
            .insert_resource(link)
            .add_systems(Startup, start_stream)
            .add_systems(Update, feed_stream.in_set(SimSystems::Present));
    }
}

/// Verbindung vom Spiel zum Audio-Thread.
#[derive(Resource, Clone)]
struct SoundLink {
    shared: Arc<Mutex<AudioInput>>,
    volume: f32,
}

/// Endloser Synthesizer-Datenstrom als Audioquelle.
#[derive(Asset, TypePath)]
pub struct SynthStream {
    shared: Arc<Mutex<AudioInput>>,
    voice: Arc<[f32]>,
    volume: f32,
}

pub struct SynthDecoder {
    synth: Synth,
    shared: Arc<Mutex<AudioInput>>,
    buf: Vec<f32>,
    pos: usize,
}

impl Iterator for SynthDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.pos >= self.buf.len() {
            // Vergiftetes Lock (Panik im Spiel-Thread): letzte Eingabe behalten.
            if let Ok(input) = self.shared.lock() {
                self.synth.set_input(*input);
            }
            self.synth.render(&mut self.buf);
            self.pos = 0;
        }
        let v = self.buf[self.pos];
        self.pos += 1;
        Some(v)
    }
}

impl Source for SynthDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).expect("1 Kanal")
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(SAMPLE_RATE).expect("Abtastrate > 0")
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for SynthStream {
    type Decoder = SynthDecoder;

    fn decoder(&self) -> SynthDecoder {
        let mut synth = Synth::new(self.voice.clone());
        synth.set_volume(self.volume);
        SynthDecoder { synth, shared: self.shared.clone(), buf: vec![0.0; BLOCK], pos: BLOCK }
    }
}

/// Ansage der Bodenwarnung aus `assets/sounds/pull_up.wav` (PCM, 8/16 Bit),
/// falls vorhanden.
fn custom_pull_up_voice() -> Option<Arc<[f32]>> {
    let mut roots = vec![PathBuf::from("assets")];
    if let Ok(dir) = std::env::var("CARGO_MANIFEST_DIR") {
        roots.insert(0, PathBuf::from(dir).join("assets"));
    }
    for root in roots {
        let path = root.join("sounds").join("pull_up.wav");
        if let Ok(bytes) = std::fs::read(&path) {
            match f29_audio::wav::read_mono(&bytes, SAMPLE_RATE) {
                Some(samples) if !samples.is_empty() => {
                    info!("PULL-UP-Ansage: {}", path.display());
                    return Some(samples.into());
                }
                _ => warn!(
                    "{} ist kein unterstütztes PCM-WAV (8/16 Bit) – nutze die eingebaute Ansage",
                    path.display()
                ),
            }
        }
    }
    None
}

fn start_stream(mut commands: Commands, link: Res<SoundLink>, mut streams: ResMut<Assets<SynthStream>>) {
    let voice = custom_pull_up_voice().unwrap_or_else(f29_audio::default_pull_up_voice);
    let handle = streams.add(SynthStream { shared: link.shared.clone(), voice, volume: link.volume });
    commands.spawn(AudioPlayer(handle));
}

fn feed_stream(link: Res<SoundLink>, state: Res<SimState>) {
    let input = AudioInput::from_flight(&state.sim.flight_data(), state.paused);
    if let Ok(mut shared) = link.shared.lock() {
        *shared = input;
    }
}
