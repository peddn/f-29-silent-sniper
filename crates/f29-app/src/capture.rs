//! Screenshots: F12 von Hand, oder automatisch per Kommandozeile
//! (`--screenshot bild.png --after 8`) – danach beendet sich das Spiel.
//! Die automatische Variante dient Tests und Dokumentation.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use std::path::PathBuf;

/// Automatischer Screenshot nach einer festen Spielzeit.
#[derive(Resource, Debug, Clone)]
pub struct AutoCapture {
    pub path: String,
    pub after_s: f32,
}

pub struct CapturePlugin {
    pub auto: Option<AutoCapture>,
}

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, manual_capture);
        if let Some(auto) = &self.auto {
            app.insert_resource(auto.clone()).add_systems(Update, auto_capture);
        }
    }
}

/// Wartezeit nach dem Auslösen, bis die Datei sicher geschrieben ist.
const EXIT_DELAY_S: f32 = 2.0;

fn manual_capture(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, mut counter: Local<u32>) {
    if !keys.just_pressed(KeyCode::F12) {
        return;
    }
    let dir = PathBuf::from("screenshots");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!("Screenshot-Ordner nicht anlegbar: {e}");
        return;
    }
    *counter += 1;
    let path = dir.join(format!("f29-{:03}.png", *counter));
    info!("Screenshot: {}", path.display());
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
}

fn auto_capture(
    mut commands: Commands,
    time: Res<Time<Real>>,
    capture: Res<AutoCapture>,
    mut taken_at: Local<Option<f32>>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    match *taken_at {
        None if now >= capture.after_s => {
            info!("Automatischer Screenshot: {}", capture.path);
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(capture.path.clone()));
            *taken_at = Some(now);
        }
        Some(t) if now >= t + EXIT_DELAY_S => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
