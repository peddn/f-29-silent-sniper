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
use f29_core::sensors::LookDir;
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

/// Drehung der Kamera um die Hochachse für einen Blick aus dem Cockpit
/// (M-035, Y-up: links positiv); dieselben Richtungen wie die Zielwahl.
fn look_yaw(look: LookDir) -> f32 {
    -look.offset() as f32
}

#[derive(Resource, Debug, Clone)]
pub struct ViewSettings {
    pub mode: ViewMode,
    pub hud: bool,
    /// HUD-Modus (H blättert NAV → AG → AA).
    pub hud_mode: f29_hud::HudMode,
    pub retro: RetroMode,
    /// Weitwinkelsicht ohne Instrumentenbrett (Cockpit, Shift+F5).
    pub wide: bool,
    /// Blick aus dem Cockpit (Strg+I/J/K/L); nur vorn mit HUD.
    pub look: LookDir,
    /// Ausgestiegen: Blick des Piloten am Schirm, ohne Brett (M-032).
    pub ejected: bool,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            mode: ViewMode::Cockpit,
            hud: true,
            hud_mode: f29_hud::HudMode::default(),
            retro: RetroMode::Full,
            wide: false,
            look: LookDir::Forward,
            ejected: false,
        }
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

pub fn layout_for(window: &Window, view: &ViewSettings) -> ScreenLayout {
    let aspect = window.width() / window.height().max(1.0);
    // Vollbild: Weitwinkelsicht, Blick zur Seite oder nach hinten (wie im
    // Original ohne Brett), ausgestiegen.
    let side_look = view.mode == ViewMode::Cockpit && view.look != LookDir::Forward;
    if (view.wide && view.mode == ViewMode::Cockpit) || side_look || view.ejected {
        ScreenLayout::wide(aspect)
    } else {
        ScreenLayout::cockpit(aspect)
    }
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

/// Beschränkt die 3D-Kamera auf den Bereich über dem Instrumentenbrett
/// (Weitwinkelsicht: ganzes Fenster) und setzt ihren Sichtwinkel.
fn update_viewport(
    window: Single<&Window, With<PrimaryWindow>>,
    view: Res<ViewSettings>,
    mut camera: Single<(&mut Camera, &mut Projection), With<MainCamera>>,
) {
    let (w, h) = (window.physical_width(), window.physical_height());
    if w == 0 || h == 0 {
        return;
    }
    let layout = layout_for(&window, &view);
    let (camera, projection) = &mut *camera;
    if let Projection::Perspective(p) = &mut **projection
        && p.fov != layout.vertical_fov
    {
        p.fov = layout.vertical_fov;
    }
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
    mut view: ResMut<ViewSettings>,
    head: Res<crate::cockpit::HeadLook>,
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
    let night = state.sim.night();
    let sky = sky_for(night);
    if clear.0 != sky {
        clear.0 = sky;
    }
    let pose = state.render_pose();
    origin.0 = pose.position;
    let rot = rotation(pose.attitude);

    let (ac_transform, ac_visibility) = &mut *aircraft;
    ac_transform.translation = Vec3::ZERO;
    ac_transform.rotation = rot;

    let (cam, retro, fog) = &mut *camera;
    fog.color = if night { night_sky_color() } else { haze_color() };
    let pilot = state.sim.flight_data().pilot;
    view.ejected = pilot.is_some();
    if let Some(p) = pilot {
        // Am Piloten: Weltursprung dort, das Flugzeug fliegt allein weiter.
        origin.0 = p.position;
        ac_transform.translation = place(pose.position, origin.0);
        **ac_visibility = Visibility::Visible;
        cam.translation = Vec3::ZERO;
        cam.rotation = rotation(pilot_attitude(&p))
            * Quat::from_rotation_y(head.yaw)
            * Quat::from_rotation_x(head.pitch);
        **retro = RetroEffect::from_mode(view.retro).with_night(night);
        return;
    }
    match view.mode {
        ViewMode::Cockpit => {
            **ac_visibility = Visibility::Hidden;
            cam.translation = rot * EYE_OFFSET;
            cam.rotation = rot * Quat::from_rotation_y(look_yaw(view.look));
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

/// Blick des ausgestiegenen Piloten: in Flugrichtung beim Ausstieg, etwas
/// nach unten; im Sitz taumelnd, am Schirm leicht pendelnd.
fn pilot_attitude(p: &f29_core::PilotInfo) -> f29_core::DQuat {
    let t = p.seconds;
    let tumble = (1.0 - p.chute) * (-t * 0.8).exp();
    let pitch = -0.15 + tumble * 0.5 * (t * 7.0).sin() + p.chute * 0.04 * (t * 0.9).sin();
    let roll = tumble * 0.6 * (t * 5.0).sin() + p.chute * 0.07 * (t * 1.3).sin();
    f29_core::DQuat::from_euler(p.heading + tumble * 0.3 * (t * 3.0).sin(), pitch, roll)
}

/// Fahrwerk zeigen, sobald es halb draußen ist.
fn update_gear(state: Res<SimState>, mut gear: Single<&mut Visibility, With<LandingGear>>) {
    let want = if state.sim.aircraft().gear > 0.5 { Visibility::Inherited } else { Visibility::Hidden };
    if **gear != want {
        **gear = want;
    }
}

/// Simulationsposition (NED) relativ zum Weltursprung in Bevy-Koordinaten.
pub fn place(at: DVec3, origin: DVec3) -> Vec3 {
    Vec3::from_array(coords::relative_yup(at, origin))
}

/// Simulationslage (NED) als Bevy-Rotation.
pub fn rotation(q: f29_core::DQuat) -> Quat {
    let [x, y, z, w] = coords::quat_ned_to_yup(q);
    Quat::from_xyzw(x as f32, y as f32, z as f32, w as f32).normalize()
}
