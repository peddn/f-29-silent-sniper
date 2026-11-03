//! MFDs: Kartendaten sowie Höhen- und Satellitenbild im Hintergrund erzeugen,
//! Bildschirmhintergründe und das Kartenbild als Sprites unter die HUD-Linien
//! legen.

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
use f29_hud::map::map_half_extent;
use f29_hud::{MapData, MapMode, MfdState, Rect};
use f29_render::map_raster::{MapRaster, MapStyle};
use f29_render::palette;
use std::sync::Arc;

/// Pixel je Kante der Kartenraster über das ganze Gebiet (500 km ⇒ ≈ 122 m/px).
const SAT_RASTER_N: usize = 4_096;
/// Pixel je Kante des Kartenausschnitts im MFD.
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

/// Kartendaten der Mission (Route, Ziele) und Kartenraster des Geländes
/// (`None`, solange sie berechnet werden).
#[derive(Resource, Default)]
pub struct MapAssets {
    /// Mission (Seed, Konfliktlage) bzw. Gelände, zu denen die Daten gehören.
    mission: Option<(u64, f29_core::rules::Conflict)>,
    seed: Option<u64>,
    pub map: Option<Arc<MapData>>,
    /// Kartenraster je Modus: Höhenbild (Gitter), Satellitenbild.
    rasters: [Option<Arc<MapRaster>>; 2],
    raster_tasks: [Option<Task<MapRaster>>; 2],
    map_image: Handle<Image>,
}

impl MapAssets {
    /// Mission, zu der [`MapAssets::map`] gehört.
    pub fn mission(&self) -> Option<(u64, f29_core::rules::Conflict)> {
        self.mission
    }

    /// Höhenbild des Geländes, sobald berechnet.
    pub fn height_raster(&self) -> Option<&MapRaster> {
        self.rasters[0].as_deref()
    }
}

#[derive(Component)]
enum Screen {
    Left,
    Right,
    Center,
    Map,
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
    assets.map_image = images.add(image);

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
        Sprite { image: assets.map_image.clone(), custom_size: Some(Vec2::ONE), ..default() },
        Transform::from_xyz(0.0, 0.0, -6.0),
        RenderLayers::layer(HUD_LAYER),
        Visibility::Hidden,
        Screen::Map,
    ));
}

/// Baut bei neuer Mission die Kartendaten neu; startet bei neuem Gelände
/// die Berechnung der Raster im Hintergrund und übernimmt fertige.
fn load_maps(mut assets: ResMut<MapAssets>, state: Res<SimState>) {
    let (sim, terrain) = (&state.sim, state.sim.terrain());
    let mission = (sim.config().seed, sim.config().spawn.conflict);
    if assets.mission != Some(mission) {
        assets.mission = Some(mission);
        assets.map = Some(Arc::new(MapData::for_terrain(terrain, sim.route(), sim.targets())));
    }
    if assets.seed != Some(terrain.seed()) {
        assets.seed = Some(terrain.seed());
        assets.rasters = [None, None];
        let pool = AsyncComputeTaskPool::get();
        for (k, style) in [MapStyle::Height, MapStyle::Satellite].into_iter().enumerate() {
            let t = terrain.clone();
            assets.raster_tasks[k] = Some(pool.spawn(async move {
                MapRaster::build(&t, [0.0, 0.0], map_half_extent(&t), SAT_RASTER_N, style)
            }));
        }
    }
    for k in 0..2 {
        if let Some(task) = assets.raster_tasks[k].as_mut()
            && let Some(raster) = check_ready(task)
        {
            assets.rasters[k] = Some(Arc::new(raster));
            assets.raster_tasks[k] = None;
        }
    }
}

/// Bildschirmrechteck (Bildschirmeinheiten) → Sprite-Transform in Pixeln.
fn fit(rect: Rect, h: f32, z: f32, transform: &mut Transform, sprite: &mut Sprite) {
    let c = rect.center();
    let size = Vec2::new(rect.max[0] - rect.min[0], rect.max[1] - rect.min[1]) * h;
    transform.translation = Vec3::new(c[0] * h, c[1] * h, z);
    sprite.custom_size = Some(size);
}

/// Hintergründe der MFDs und das Kartenbild platzieren und aktualisieren.
fn place_screens(
    window: Single<&Window, With<PrimaryWindow>>,
    state: Res<SimState>,
    view: Res<MfdView>,
    settings: Res<crate::view::ViewSettings>,
    assets: Res<MapAssets>,
    mut images: ResMut<Assets<Image>>,
    mut screens: Query<(&Screen, &mut Transform, &mut Sprite, &mut Visibility)>,
) {
    let h = window.height();
    let layout = layout_for(&window, &settings);
    let fd = &state.fd;
    // Nach dem Missionsende deckt die Nachbesprechung das Brett.
    let mfd = layout.mfd_layout().filter(|_| !state.covered());
    let raster = match view.0.map_mode {
        MapMode::Grid => &assets.rasters[0],
        MapMode::Satellite => &assets.rasters[1],
    }
    .as_ref()
    .filter(|_| f29_hud::displays_on(fd));
    for (screen, mut transform, mut sprite, mut vis) in &mut screens {
        let Some(m) = mfd else {
            *vis = Visibility::Hidden;
            continue;
        };
        let (rect, z, show) = match screen {
            Screen::Left => (m.left_screen, -8.0, true),
            Screen::Right => (m.right_screen, -8.0, true),
            Screen::Center => (m.center, -8.0, true),
            Screen::Map => (m.left_screen, -6.0, raster.is_some()),
        };
        fit(rect, h, z, &mut transform, &mut sprite);
        *vis = if show { Visibility::Inherited } else { Visibility::Hidden };
    }
    if let Some(raster) = raster
        && let Some(mut image) = images.get_mut(&assets.map_image)
    {
        let px = raster.render_view(
            [fd.position.x, fd.position.y],
            fd.heading,
            view.0.map_range_m(),
            SAT_VIEW_PX as usize,
        );
        image.data = Some(px);
    }
}
