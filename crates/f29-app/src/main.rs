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
//!                   [--volume 0..100] [--no-sound]
//!                   [--loadout fall,chute,lgb,agm (ohne: Vorschlag des Auftrags)] [--night] [--no-awacs] [--no-stealth] [--no-damage]
//!                   [--rules cold|limited|war]
//!                   [--joystick DATEI] [--joystick-debug]
//!                   [--screenshot DATEI.png [--after SEKUNDEN]]
//! ```

mod airfield;
mod capture;
mod cockpit;
mod fighters;
mod hud;
mod input;
mod joystick;
mod joystick_config;
mod mfd;
mod retro;
mod sim;
mod sound;
mod targets;
mod terrain;
mod tgt_camera;
mod view;
mod weapons;

use bevy::prelude::*;
use bevy::window::PresentMode;

/// Render-Layer des 2D-Overlays (HUD und Instrumentenbrett).
pub const HUD_LAYER: usize = 1;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed = arg(&args, "--seed").and_then(|v| parse_u64(&v)).unwrap_or(sim::DEFAULT_SEED);
    // Standard: Start auf der Bahn der Heimatbasis; `--start air` für den Luftstart.
    let mut config = if arg(&args, "--start").as_deref() == Some("air") {
        f29_core::SimConfig::new(seed)
    } else {
        f29_core::SimConfig::runway(seed)
    };
    // Nachtflug: dunkle Darstellung, stark verkürzte Sensorreichweiten.
    config.spawn.night = args.iter().any(|a| a == "--night");
    // Ohne freundliches AWACS: keine Datalink-Daten zu fernen Zielen.
    config.spawn.awacs = !args.iter().any(|a| a == "--no-awacs");
    config.spawn.visible = args.iter().any(|a| a == "--no-stealth");
    config.spawn.invulnerable = args.iter().any(|a| a == "--no-damage");
    // Beladung der vier Schächte; ohne Angabe der Vorschlag des Auftrags.
    config.spawn.loadout = None;
    if let Some(text) = arg(&args, "--loadout") {
        match f29_core::weapons::Loadout::parse(&text) {
            Some(l) => config.spawn.loadout = Some(l),
            None => eprintln!(
                "--loadout: vier Einträge aus fall, chute, lgb, agm, arm, ship, aim9, aim120, cam oder - erwartet"
            ),
        }
    }
    // Regelset (Konfliktlage): Kalter Krieg, begrenzter Konflikt, offener Krieg.
    if let Some(text) = arg(&args, "--rules") {
        match f29_core::rules::Conflict::from_code(&text) {
            Some(c) => config.spawn.conflict = c,
            None => eprintln!("--rules: cold, limited oder war erwartet"),
        }
    }
    let mut view_settings = view::ViewSettings::default();
    if arg(&args, "--view").as_deref() == Some("chase") {
        view_settings.mode = view::ViewMode::Chase;
    }
    // Klang: Lautstärke in Prozent, `--no-sound` schaltet das Audiogerät ganz ab.
    let volume = if args.iter().any(|a| a == "--no-sound") {
        0.0
    } else {
        arg(&args, "--volume")
            .and_then(|v| v.parse::<f32>().ok())
            .map_or(0.8, |v| (v / 100.0).clamp(0.0, 1.0))
    };
    // Joystick: Belegung aus Datei (Voreinstellung Logitech Extreme 3D Pro),
    // `--joystick-debug` meldet Achsen und Tasten auf der Konsole.
    let joystick = joystick::JoystickSettings {
        config: joystick::load(arg(&args, "--joystick").as_deref()),
        debug: args.iter().any(|a| a == "--joystick-debug"),
    };
    let auto_capture = arg(&args, "--screenshot").map(|path| capture::AutoCapture {
        path,
        after_s: arg(&args, "--after").and_then(|v| v.parse().ok()).unwrap_or(8.0),
    });

    let window = WindowPlugin {
        primary_window: Some(Window {
            title: "F-29 Silent Sniper".into(),
            resolution: (1280, 800).into(),
            present_mode: PresentMode::AutoVsync,
            ..default()
        }),
        ..default()
    };
    let mut app = App::new();
    app.insert_resource(ClearColor(view::sky_color())).insert_resource(view_settings);
    if volume > 0.0 {
        app.add_plugins(DefaultPlugins.set(window)).add_plugins(sound::SoundPlugin { volume });
    } else {
        // Ohne Klang wird auch kein Audiogerät geöffnet.
        app.add_plugins(DefaultPlugins.set(window).disable::<bevy::audio::AudioPlugin>());
    }
    app.add_plugins((
        sim::SimPlugin { config },
        airfield::AirfieldPlugin,
        targets::TargetsPlugin,
        fighters::FightersPlugin,
        weapons::WeaponsPlugin,
        input::InputPlugin,
        joystick::JoystickPlugin { settings: joystick },
        view::ViewPlugin,
        terrain::TerrainPlugin,
        hud::HudPlugin,
        mfd::MfdPlugin,
        retro::RetroPlugin,
        tgt_camera::TgtCameraPlugin,
        cockpit::CockpitPlugin,
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
