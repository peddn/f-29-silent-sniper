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
        let mut synth = Synth::new();
        synth.set_volume(self.volume);
        SynthDecoder { synth, shared: self.shared.clone(), buf: vec![0.0; BLOCK], pos: BLOCK }
    }
}

fn start_stream(mut commands: Commands, link: Res<SoundLink>, mut streams: ResMut<Assets<SynthStream>>) {
    let handle = streams.add(SynthStream { shared: link.shared.clone(), volume: link.volume });
    commands.spawn(AudioPlayer(handle));
}

fn feed_stream(link: Res<SoundLink>, state: Res<SimState>) {
    let input = AudioInput::from_flight(&state.sim.flight_data(), state.paused);
    if let Ok(mut shared) = link.shared.lock() {
        *shared = input;
    }
}
