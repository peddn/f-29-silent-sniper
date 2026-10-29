//! MFDs: Kartendaten und Satellitenbild im Hintergrund erzeugen, Bildschirm-
//! hintergründe und das Satellitenbild als Sprites unter die HUD-Linien legen.

use crate::HUD_LAYER;
use crate::sim::{SimState, SimSystems};
use crate::view::layout_for;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::futures::check_ready;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::window::PrimaryWindow;
use f29_hud::map::{MAP_HALF_EXTENT_M, map_center};
use f29_hud::{MapData, MapMode, MfdState, Rect};
use f29_render::palette;
use f29_render::sat_map::SatelliteMap;
use std::sync::Arc;

/// Pixel je Kante des Satellitenrasters (Kartengebiet 200 km ⇒ ≈ 98 m/px).
const SAT_RASTER_N: usize = 2_048;
/// Pixel je Kante des Satelliten-Ausschnitts im MFD.
const SAT_VIEW_PX: u32 = 256;

pub struct MfdPlugin;

impl Plugin for MfdPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MfdView>()
            .init_resource::<MapAssets>()
            .add_systems(Startup, setup)
            .add_systems(Update, (load_maps, place_screens).chain().in_set(SimSystems::Present));
    }
}

/// Bedienzustand der MFDs (Kartenmodus, Zoom, Seite).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct MfdView(pub MfdState);

/// Kartendaten des aktuellen Seeds; `None`, solange sie berechnet werden.
#[derive(Resource, Default)]
pub struct MapAssets {
    seed: Option<u64>,
    pub map: Option<Arc<MapData>>,
    sat: Option<Arc<SatelliteMap>>,
    map_task: Option<Task<MapData>>,
    sat_task: Option<Task<SatelliteMap>>,
    sat_image: Handle<Image>,
}

#[derive(Component)]
enum Screen {
    Left,
    Right,
    Center,
    Satellite,
}

fn setup(mut commands: Commands, mut assets: ResMut<MapAssets>, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        Extent3d { width: SAT_VIEW_PX, height: SAT_VIEW_PX, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    assets.sat_image = images.add(image);

    let [r, g, b] = palette::mix(0, 1, 0.25);
    let screen_color = Color::srgb(r, g, b);
    for screen in [Screen::Left, Screen::Right, Screen::Center] {
        commands.spawn((
            Sprite::from_color(screen_color, Vec2::ONE),
            Transform::from_xyz(0.0, 0.0, -8.0),
            RenderLayers::layer(HUD_LAYER),
            Visibility::Hidden,
            screen,
        ));
    }
    commands.spawn((
        Sprite { image: assets.sat_image.clone(), custom_size: Some(Vec2::ONE), ..default() },
        Transform::from_xyz(0.0, 0.0, -6.0),
        RenderLayers::layer(HUD_LAYER),
        Visibility::Hidden,
        Screen::Satellite,
    ));
}

/// Startet bei neuem Seed die Hintergrundberechnung und übernimmt fertige Ergebnisse.
fn load_maps(mut assets: ResMut<MapAssets>, state: Res<SimState>) {
    let terrain = state.sim.terrain();
    if assets.seed != Some(terrain.seed()) {
        assets.seed = Some(terrain.seed());
        assets.map = None;
        assets.sat = None;
        let pool = AsyncComputeTaskPool::get();
        let (t, route, targets) = (terrain.clone(), state.sim.route().to_vec(), state.sim.targets().to_vec());
        assets.map_task = Some(pool.spawn(async move { MapData::for_terrain(&t, &route, &targets) }));
        let t = terrain.clone();
        assets.sat_task =
            Some(pool.spawn(async move {
                SatelliteMap::build(&t, map_center(&t), MAP_HALF_EXTENT_M, SAT_RASTER_N)
            }));
    }
    if let Some(task) = assets.map_task.as_mut()
        && let Some(map) = check_ready(task)
    {
        assets.map = Some(Arc::new(map));
        assets.map_task = None;
    }
    if let Some(task) = assets.sat_task.as_mut()
        && let Some(sat) = check_ready(task)
    {
        assets.sat = Some(Arc::new(sat));
        assets.sat_task = None;
    }
}

/// Bildschirmrechteck (Bildschirmeinheiten) → Sprite-Transform in Pixeln.
fn fit(rect: Rect, h: f32, z: f32, transform: &mut Transform, sprite: &mut Sprite) {
    let c = rect.center();
    let size = Vec2::new(rect.max[0] - rect.min[0], rect.max[1] - rect.min[1]) * h;
    transform.translation = Vec3::new(c[0] * h, c[1] * h, z);
    sprite.custom_size = Some(size);
}

/// Hintergründe der MFDs und das Satellitenbild platzieren und aktualisieren.
fn place_screens(
    window: Single<&Window, With<PrimaryWindow>>,
    state: Res<SimState>,
    view: Res<MfdView>,
    assets: Res<MapAssets>,
    mut images: ResMut<Assets<Image>>,
    mut screens: Query<(&Screen, &mut Transform, &mut Sprite, &mut Visibility)>,
) {
    let h = window.height();
    let layout = layout_for(&window);
    let mfd = layout.mfd_layout();
    let fd = state.sim.flight_data();
    let sat_visible =
        view.0.map_mode == MapMode::Satellite && assets.sat.is_some() && f29_hud::displays_on(&fd);
    for (screen, mut transform, mut sprite, mut vis) in &mut screens {
        let Some(m) = mfd else {
            *vis = Visibility::Hidden;
            continue;
        };
        let (rect, z, show) = match screen {
            Screen::Left => (m.left_screen, -8.0, true),
            Screen::Right => (m.right_screen, -8.0, true),
            Screen::Center => (m.center, -8.0, true),
            Screen::Satellite => (m.left_screen, -6.0, sat_visible),
        };
        fit(rect, h, z, &mut transform, &mut sprite);
        *vis = if show { Visibility::Inherited } else { Visibility::Hidden };
    }
    if sat_visible
        && let Some(sat) = &assets.sat
        && let Some(mut image) = images.get_mut(&assets.sat_image)
    {
        let px = sat.render_view(
            [fd.position.x, fd.position.y],
            fd.heading,
            view.0.map_range_m(),
            SAT_VIEW_PX as usize,
        );
        image.data = Some(px);
    }
}
