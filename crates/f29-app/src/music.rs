//! Hintergrundmusik (M-060): `assets/music/theme.ogg` (Ogg Vorbis) läuft in
//! Schleife auf allen Bildschirmen außerhalb des Cockpits und pausiert im
//! Flug, wie im Original. Ohne Datei bleibt es still.

use crate::sim::{SimState, SimSystems};
use bevy::asset::io::file::FileAssetReader;
use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::prelude::*;

const FILE: &str = "music/theme.ogg";

pub struct MusicPlugin {
    /// Gesamtlautstärke 0..1.
    pub volume: f32,
}

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MusicVolume(self.volume))
            .add_systems(Startup, start)
            .add_systems(Update, follow.in_set(SimSystems::Present));
    }
}

#[derive(Resource)]
struct MusicVolume(f32);

#[derive(Component)]
struct Music;

fn start(mut commands: Commands, assets: Res<AssetServer>, volume: Res<MusicVolume>) {
    if FileAssetReader::get_base_path().join("assets").join(FILE).exists() {
        let settings = PlaybackSettings::LOOP.paused().with_volume(Volume::Linear(volume.0));
        commands.spawn((AudioPlayer::new(assets.load(FILE)), settings, Music));
    }
}

/// Außerhalb des Cockpits spielen, im Flug pausieren.
fn follow(state: Res<SimState>, sinks: Query<&AudioSink, With<Music>>) {
    for sink in &sinks {
        match (state.covered(), sink.is_paused()) {
            (true, true) => sink.play(),
            (false, false) => sink.pause(),
            _ => {}
        }
    }
}
