//! HUD und Instrumentenbrett: Display-Liste aus `f29-hud` als 2D-Gizmo-Linien.

use crate::HUD_LAYER;
use crate::mfd::{MapAssets, MfdView};
use crate::sim::{SimState, SimSystems};
use crate::view::{ViewMode, ViewSettings, layout_for};
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use f29_hud::{HudOptions, Ink};
use f29_render::palette;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_gizmo_group::<HudGizmos>()
            .add_systems(Startup, setup)
            .add_systems(Update, (scale_line_width, draw_hud).chain().in_set(SimSystems::Present));
    }
}

/// Eigene Gizmo-Gruppe, die nur die Overlay-Kamera sieht.
#[derive(Default, Reflect, GizmoConfigGroup)]
struct HudGizmos;

#[derive(Component)]
struct PanelBackground;

fn setup(mut commands: Commands, mut config_store: ResMut<GizmoConfigStore>) {
    let (config, _) = config_store.config_mut::<HudGizmos>();
    config.render_layers = RenderLayers::layer(HUD_LAYER);
    config.line.width = 2.0;

    let [r, g, b] = palette::mix(0, 13, 0.18);
    commands.spawn((
        Sprite::from_color(Color::srgb(r, g, b), Vec2::ONE),
        Transform::from_xyz(0.0, 0.0, -10.0),
        RenderLayers::layer(HUD_LAYER),
        PanelBackground,
    ));
}

fn ink_color(ink: Ink) -> Color {
    match ink {
        Ink::Hud => Color::srgb(0.35, 1.0, 0.45),
        Ink::HudDim => Color::srgb(0.15, 0.62, 0.25),
        Ink::Warning => Color::srgb(1.0, 0.33, 0.25),
        Ink::HudWarning => Color::srgb(0.35, 0.63, 1.0),
        Ink::Panel => Color::srgb(0.82, 0.84, 0.86),
        Ink::PanelDim => Color::srgb(0.42, 0.44, 0.48),
        Ink::Needle => Color::srgb(1.0, 0.86, 0.3),
        Ink::Mfd => Color::srgb(0.4, 0.87, 0.93),
        Ink::MfdDim => Color::srgb(0.2, 0.47, 0.53),
        Ink::MfdGrid => Color::srgb(0.13, 0.27, 0.33),
    }
}

/// Linienstärke wächst mit der Auflösung – das HUD sieht überall gleich aus.
///
/// Eigenes System, weil `Gizmos` den `GizmoConfigStore` selbst liest und
/// Bevy zwei Zugriffe auf dieselbe Ressource in einem System ablehnt.
fn scale_line_width(
    window: Single<&Window, With<PrimaryWindow>>,
    mut config_store: ResMut<GizmoConfigStore>,
) {
    let (config, _) = config_store.config_mut::<HudGizmos>();
    config.line.width = (window.physical_height() as f32 / 420.0).clamp(1.0, 6.0);
}

fn draw_hud(
    window: Single<&Window, With<PrimaryWindow>>,
    state: Res<SimState>,
    view: Res<ViewSettings>,
    mfd: Res<MfdView>,
    maps: Res<MapAssets>,
    mut gizmos: Gizmos<HudGizmos>,
    mut panel: Single<(&mut Transform, &mut Sprite), With<PanelBackground>>,
) {
    let (w, h) = (window.width(), window.height());
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let layout = layout_for(&window);

    // Hintergrund des Instrumentenbretts
    let panel_h = layout.panel_fraction * h;
    let (transform, sprite) = &mut *panel;
    sprite.custom_size = Some(Vec2::new(w, panel_h));
    transform.translation = Vec3::new(0.0, -h * 0.5 + panel_h * 0.5, -10.0);

    let fd = state.sim.flight_data();
    // Ein optisch erfasstes Ziel zeigt die Zielkamera (tgt_camera.rs) statt des Drahtgitters.
    let opts = HudOptions {
        show_hud: view.hud && view.mode == ViewMode::Cockpit,
        hud_mode: view.hud_mode,
        show_panel: true,
        mfd: mfd.0,
        camera_feed: true,
    };
    let list = f29_hud::build(&fd, &layout, &opts, maps.map.as_deref());
    for line in &list.lines {
        gizmos.line_2d(
            Vec2::new(line.a[0], line.a[1]) * h,
            Vec2::new(line.b[0], line.b[1]) * h,
            ink_color(line.ink),
        );
    }
    if state.paused {
        let mut pause = f29_hud::DisplayList::new();
        pause.text_mid([0.0, 0.25], 0.03, f29_hud::Align::Center, "PAUSE", Ink::Warning);
        for line in &pause.lines {
            gizmos.line_2d(Vec2::from_array(line.a) * h, Vec2::from_array(line.b) * h, ink_color(line.ink));
        }
    }
}
