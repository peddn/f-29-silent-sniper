//! F-29 Silent Sniper – Bevy-Frontend.
//!
//! Dieses Crate ist bewusst dünn: Es liest Eingaben, lässt den
//! Simulationskern (`f29-core`) mit festem Zeitschritt laufen und stellt das
//! Ergebnis dar. Geometrie (`f29-render`) und HUD (`f29-hud`) entstehen
//! engine-neutral und werden hier nur noch in Bevy-Typen übersetzt.
//!
//! Kommandozeile:
//!
//! ```text
//! f29-silent-sniper [--seed N] [--view cockpit|chase] [--start runway|air]
//!                   [--screenshot DATEI.png [--after SEKUNDEN]]
//! ```

mod airfield;
mod capture;
mod hud;
mod input;
mod mfd;
mod retro;
mod sim;
mod terrain;
mod view;

use bevy::prelude::*;
use bevy::window::PresentMode;

/// Render-Layer des 2D-Overlays (HUD und Instrumentenbrett).
pub const HUD_LAYER: usize = 1;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed = arg(&args, "--seed").and_then(|v| parse_u64(&v)).unwrap_or(sim::DEFAULT_SEED);
    // Standard: Start auf der Bahn der Heimatbasis; `--start air` für den Luftstart.
    let config = if arg(&args, "--start").as_deref() == Some("air") {
        f29_core::SimConfig::new(seed)
    } else {
        f29_core::SimConfig::runway(seed)
    };
    let mut view_settings = view::ViewSettings::default();
    if arg(&args, "--view").as_deref() == Some("chase") {
        view_settings.mode = view::ViewMode::Chase;
    }
    let auto_capture = arg(&args, "--screenshot").map(|path| capture::AutoCapture {
        path,
        after_s: arg(&args, "--after").and_then(|v| v.parse().ok()).unwrap_or(8.0),
    });

    App::new()
        .insert_resource(ClearColor(view::sky_color()))
        .insert_resource(view_settings)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "F-29 Silent Sniper".into(),
                resolution: (1280, 800).into(),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            sim::SimPlugin { config },
            airfield::AirfieldPlugin,
            input::InputPlugin,
            view::ViewPlugin,
            terrain::TerrainPlugin,
            hud::HudPlugin,
            mfd::MfdPlugin,
            retro::RetroPlugin,
            capture::CapturePlugin { auto: auto_capture },
        ))
        .run();
}

/// Wert nach einem Schalter, z. B. `--seed 42`.
fn arg(args: &[String], name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.get(i + 1).cloned()
}

/// Dezimal oder `0x…` hexadezimal.
fn parse_u64(v: &str) -> Option<u64> {
    match v.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => v.parse().ok(),
    }
}
