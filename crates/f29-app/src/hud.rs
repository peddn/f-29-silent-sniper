//! HUD und Instrumentenbrett: Display-Liste aus `f29-hud` als 2D-Gizmo-Linien.

use crate::HUD_LAYER;
use crate::mfd::{MapAssets, MfdView};
use crate::sim::{SimState, SimSystems};
use crate::view::{ViewMode, ViewSettings, layout_for};
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use f29_core::sensors::LookDir;
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

    commands.spawn((
        Sprite::from_color(Color::BLACK, Vec2::ONE),
        Transform::from_xyz(0.0, 0.0, -10.0),
        RenderLayers::layer(HUD_LAYER),
        PanelBackground,
    ));
}

fn ink_color(ink: Ink) -> Color {
    let [r, g, b] = ink.srgb();
    Color::srgb(r, g, b)
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

#[allow(clippy::too_many_arguments)]
fn draw_hud(
    window: Single<&Window, With<PrimaryWindow>>,
    state: Res<SimState>,
    view: Res<ViewSettings>,
    mfd: Res<MfdView>,
    maps: Res<MapAssets>,
    intel_view: Res<crate::ui::IntelView>,
    mut gizmos: Gizmos<HudGizmos>,
    mut panel: Single<(&mut Transform, &mut Sprite), With<PanelBackground>>,
) {
    let (w, h) = (window.width(), window.height());
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let layout = layout_for(&window, &view);

    let fd = &state.fd;
    // Hintergrund des Instrumentenbretts; außerhalb des Cockpits füllt er das
    // ganze Fenster um das Bild der UI.
    let covered = state.covered();
    let panel_h = if covered { h } else { layout.panel_fraction * h };
    let (transform, sprite) = &mut *panel;
    sprite.custom_size = Some(Vec2::new(w, panel_h));
    let [r, g, b] = if covered { palette::mix(0, 13, 0.08) } else { palette::mix(0, 13, 0.18) };
    sprite.color = Color::srgb(r, g, b);
    transform.translation = Vec3::new(0.0, -h * 0.5 + panel_h * 0.5, -10.0);

    // Ein optisch erfasstes Ziel zeigt die Zielkamera (tgt_camera.rs) statt des Drahtgitters.
    // Ausgestiegen: Schirm immer.
    let opts = HudOptions {
        show_hud: fd.pilot.is_some()
            || (view.hud && view.mode == ViewMode::Cockpit && view.look == LookDir::Forward),
        hud_mode: view.hud_mode,
        show_panel: true,
        mfd: mfd.0,
        camera_feed: true,
    };
    // Außerhalb des Cockpits zeichnet die UI (ui.rs); auf dem
    // Bewaffnungsbildschirm liegen Umriss und Schächte in ihrem Rahmen darüber.
    let intel = state.menu == Some(crate::menu::Screen::Intel);
    let (list, unit) = if let Some((map, v)) = maps.map.as_deref().zip(intel_view.view).filter(|_| intel) {
        let topic = crate::menu::topic(&state);
        (
            f29_hud::intel::draw(map, &fd.threat.sites, topic, v, crate::ui::map_rect()),
            crate::ui::unit(&window),
        )
    } else if state.menu == Some(crate::menu::Screen::Arming) {
        (f29_hud::arming::draw(&crate::menu::arming(&state), crate::ui::picture()), crate::ui::unit(&window))
    } else if covered {
        (f29_hud::DisplayList::new(), h)
    } else {
        (f29_hud::build(fd, &layout, &opts, maps.map.as_deref()), h)
    };
    for line in &list.lines {
        gizmos.line_2d(Vec2::from_array(line.a) * unit, Vec2::from_array(line.b) * unit, ink_color(line.ink));
    }
    let banner = if state.paused { "PAUSE".to_string() } else { format!("{}X", state.time_scale) };
    if state.paused || state.time_scale > 1 {
        let mut pause = f29_hud::DisplayList::new();
        pause.text_mid([0.0, 0.25], 0.03, f29_hud::Align::Center, &banner, Ink::Warning);
        for line in &pause.lines {
            gizmos.line_2d(Vec2::from_array(line.a) * h, Vec2::from_array(line.b) * h, ink_color(line.ink));
        }
    }
}
