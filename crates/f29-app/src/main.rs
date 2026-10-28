//! F-29 Silent Sniper – Bevy-Frontend.
//!
//! Dieses Crate ist bewusst dünn: Es liest Eingaben, lässt den
//! Simulationskern (`f29-core`) mit festem Zeitschritt laufen und stellt das
//! Ergebnis dar. Geometrie (`f29-render`) und HUD (`f29-hud`) entstehen
//! engine-neutral und werden hier nur noch in Bevy-Typen übersetzt.

mod hud;
mod input;
mod retro;
mod sim;
mod terrain;
mod view;

use bevy::prelude::*;
use bevy::window::PresentMode;

/// Render-Layer des 2D-Overlays (HUD und Instrumentenbrett).
pub const HUD_LAYER: usize = 1;

fn main() {
    let seed = parse_seed().unwrap_or(sim::DEFAULT_SEED);

    App::new()
        .insert_resource(ClearColor(view::sky_color()))
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
            sim::SimPlugin { seed },
            input::InputPlugin,
            view::ViewPlugin,
            terrain::TerrainPlugin,
            hud::HudPlugin,
            retro::RetroPlugin,
        ))
        .run();
}

/// `--seed <zahl>` oder `--seed 0x<hex>` von der Kommandozeile.
fn parse_seed() -> Option<u64> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == "--seed")?;
    let v = args.get(i + 1)?;
    match v.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => v.parse().ok(),
    }
}
