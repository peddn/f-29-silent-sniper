//! Kameras, Ansichten und das Spielerflugzeug.

use crate::HUD_LAYER;
use crate::retro::{RetroEffect, RetroMode};
use crate::sim::{SimState, SimSystems};
use crate::terrain::{VIEW_DISTANCE, to_bevy_mesh, vertex_color_material};
use bevy::camera::Viewport;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use f29_core::DVec3;
use f29_hud::ScreenLayout;
use f29_render::{coords, model, palette};

pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewSettings>()
            .init_resource::<WorldOrigin>()
            .init_resource::<ChaseCamera>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (update_viewport, update_camera_and_aircraft, update_gear)
                    .chain()
                    .in_set(SimSystems::Present),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Cockpit,
    Chase,
}

#[derive(Resource, Debug, Clone)]
pub struct ViewSettings {
    pub mode: ViewMode,
    pub hud: bool,
    pub retro: RetroMode,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self { mode: ViewMode::Cockpit, hud: true, retro: RetroMode::Full }
    }
}

/// Floating Origin: Weltposition (NED), die in Bevy bei (0, 0, 0) liegt.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct WorldOrigin(pub DVec3);

/// Geglätteter Kameraversatz der Verfolgeransicht (Y-up, relativ zum Flugzeug).
#[derive(Resource, Debug, Clone, Copy)]
pub struct ChaseCamera {
    offset: Vec3,
}

impl Default for ChaseCamera {
    fn default() -> Self {
        Self { offset: Vec3::new(0.0, 8.0, 45.0) }
    }
}

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct HudCamera;

#[derive(Component)]
pub struct PlayerAircraft;

#[derive(Component)]
pub struct LandingGear;

/// Augpunkt des Piloten relativ zum Modellzentrum (Y-up, Nase −z).
const EYE_OFFSET: Vec3 = Vec3::new(0.0, 0.9, -4.5);

pub fn sky_color() -> Color {
    let [r, g, b] = palette::srgb(palette::SKY);
    Color::srgb(r, g, b)
}

fn haze_color() -> Color {
    let [r, g, b] = palette::srgb(palette::HAZE);
    Color::srgb(r, g, b)
}

/// Nachthimmel und Nachtdunst (dunkelste Palettenfarbe bzw. ein dunkles Blau).
pub fn night_sky_color() -> Color {
    let [r, g, b] = palette::srgb(0);
    Color::srgb(r, g, b)
}

/// Himmel passend zur Tageszeit des Flugs.
pub fn sky_for(night: bool) -> Color {
    if night { night_sky_color() } else { sky_color() }
}

pub fn layout_for(window: &Window) -> ScreenLayout {
    let aspect = window.width() / window.height().max(1.0);
    ScreenLayout::cockpit(aspect)
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // 3D-Ansicht (obere Bildschirmfläche)
    commands.spawn((
        Camera3d::default(),
        Camera { order: 0, ..default() },
        Projection::Perspective(PerspectiveProjection {
            fov: ScreenLayout::DEFAULT_FOV,
            near: 0.5,
            far: (VIEW_DISTANCE * 1.5) as f32,
            ..default()
        }),
        // Exakte Farben für die Palettenquantisierung: kein Tonemapping, kein Debanding.
        Tonemapping::None,
        DebandDither::Disabled,
        Msaa::Off,
        DistanceFog {
            color: haze_color(),
            falloff: FogFalloff::Linear {
                start: (VIEW_DISTANCE * 0.35) as f32,
                end: (VIEW_DISTANCE * 0.97) as f32,
            },
            ..default()
        },
        RetroEffect::from_mode(RetroMode::Full),
        Transform::default(),
        MainCamera,
    ));

    // 2D-Overlay für HUD und Instrumentenbrett (ganzes Fenster)
    commands.spawn((
        Camera2d,
        Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
        Tonemapping::None,
        Msaa::Off,
        RenderLayers::layer(HUD_LAYER),
        HudCamera,
    ));

    // Spielerflugzeug mit Fahrwerk als Kind (nur ausgefahren sichtbar)
    let material = materials.add(vertex_color_material());
    commands
        .spawn((
            Mesh3d(meshes.add(to_bevy_mesh(model::aircraft()))),
            MeshMaterial3d(material.clone()),
            Transform::default(),
            Visibility::Hidden,
            PlayerAircraft,
        ))
        .with_child((
            Mesh3d(meshes.add(to_bevy_mesh(model::landing_gear()))),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Hidden,
            LandingGear,
        ));
}

/// Beschränkt die 3D-Kamera auf den Bereich über dem Instrumentenbrett.
fn update_viewport(
    window: Single<&Window, With<PrimaryWindow>>,
    mut camera: Single<&mut Camera, With<MainCamera>>,
) {
    let (w, h) = (window.physical_width(), window.physical_height());
    if w == 0 || h == 0 {
        return;
    }
    let layout = layout_for(&window);
    let view_h = ((h as f32) * (1.0 - layout.panel_fraction)).round().max(1.0) as u32;
    let size = UVec2::new(w, view_h);
    if camera.viewport.as_ref().map(|v| v.physical_size) != Some(size) {
        camera.viewport = Some(Viewport { physical_position: UVec2::ZERO, physical_size: size, ..default() });
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn update_camera_and_aircraft(
    time: Res<Time>,
    state: Res<SimState>,
    view: Res<ViewSettings>,
    mut origin: ResMut<WorldOrigin>,
    mut chase: ResMut<ChaseCamera>,
    mut camera: Single<
        (&mut Transform, &mut RetroEffect, &mut DistanceFog),
        (With<MainCamera>, Without<PlayerAircraft>),
    >,
    mut aircraft: Single<(&mut Transform, &mut Visibility), (With<PlayerAircraft>, Without<MainCamera>)>,
    mut clear: ResMut<ClearColor>,
) {
    // Tageszeit: Himmel, Dunst und Nachbearbeitung (M-013: nachts kürzere
    // Sensorreichweiten).
    let night = state.sim.config().spawn.night;
    let sky = sky_for(night);
    if clear.0 != sky {
        clear.0 = sky;
    }
    let pose = state.render_pose();
    origin.0 = pose.position;
    let [x, y, z, w] = coords::quat_ned_to_yup(pose.attitude);
    let rot = Quat::from_xyzw(x as f32, y as f32, z as f32, w as f32).normalize();

    let (ac_transform, ac_visibility) = &mut *aircraft;
    ac_transform.translation = Vec3::ZERO;
    ac_transform.rotation = rot;

    let (cam, retro, fog) = &mut *camera;
    fog.color = if night { night_sky_color() } else { haze_color() };
    match view.mode {
        ViewMode::Cockpit => {
            **ac_visibility = Visibility::Hidden;
            cam.translation = rot * EYE_OFFSET;
            cam.rotation = rot;
        }
        ViewMode::Chase => {
            **ac_visibility = Visibility::Visible;
            // Hinter und über dem Flugzeug, nur dem Kurs folgend (nicht der Querlage).
            let fwd = rot * Vec3::NEG_Z;
            let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
            let target = -flat * (model::LENGTH_M * 2.8) + Vec3::Y * (model::LENGTH_M * 0.55);
            let k = 1.0 - (-4.0 * time.delta_secs()).exp();
            chase.offset = chase.offset.lerp(target, k);
            cam.translation = chase.offset;
            cam.look_at(fwd * model::LENGTH_M, Vec3::Y);
        }
    }
    **retro = RetroEffect::from_mode(view.retro).with_night(night);
}

/// Fahrwerk zeigen, sobald es halb draußen ist.
fn update_gear(state: Res<SimState>, mut gear: Single<&mut Visibility, With<LandingGear>>) {
    let want = if state.sim.aircraft().gear > 0.5 { Visibility::Inherited } else { Visibility::Hidden };
    if **gear != want {
        **gear = want;
    }
}
