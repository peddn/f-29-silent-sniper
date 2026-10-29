//! Zielkamera der TGT-Seite (M-013): eine zweite 3D-Kamera mit schmalem
//! Sichtfeld rendert das optisch aufgeschaltete Ziel in eine Textur; ein
//! Nachbearbeitungseffekt macht daraus ein körniges Schwarzweiß-Kamerabild
//! (`assets/shaders/tgt_camera.wesl`). Das Bild liegt als Sprite im Bildfeld
//! der Seite, die Linien des MFDs (Rahmen, Fadenkreuz) darüber.
//!
//! Die Kamera läuft nur, solange die TGT-Seite offen und das Ziel optisch
//! erfasst ist; sonst kostet sie nichts.

use crate::HUD_LAYER;
use crate::mfd::MfdView;
use crate::sim::{SimState, SimSystems};
use crate::view::{WorldOrigin, layout_for, sky_color};
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat};
use bevy::render::{RenderApp, extract_component::ExtractComponent, render_resource::ShaderType};
use bevy::shader::ShaderRef;
use bevy::window::PrimaryWindow;
use f29_hud::{RightPage, target_image_rect};
use f29_render::coords;

/// Bildzeilen der Kameratextur (bewusst grob, wie ein frühes TV-Bild).
const LINES_PX: u32 = 144;
/// Das Ziel füllt etwa diesen Anteil der Bildhöhe.
const TARGET_FILL: f64 = 0.45;
/// Grenzen des vertikalen Sichtfelds (rad).
const FOV_MIN_RAD: f32 = 0.0015;
const FOV_MAX_RAD: f32 = 0.35;
/// Nahebene: das eigene Flugzeug liegt davor und wird nicht gezeichnet.
const NEAR_M: f32 = 15.0;
/// Kameraposition unter dem Rumpf (Y-up, relativ zum Flugzeugzentrum).
const MOUNT: Vec3 = Vec3::new(0.0, -1.5, -3.0);

pub struct TgtCameraPlugin;

impl Plugin for TgtCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FullscreenMaterialPlugin::<TgtCameraEffect>::default())
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                update.in_set(SimSystems::Present).after(crate::view::update_camera_and_aircraft),
            );
    }
}

/// Uniform-Daten des Kameraeffekts (16 Byte).
#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Default, Debug, PartialEq)]
#[extract_app(RenderApp)]
pub struct TgtCameraEffect {
    /// Sekunden (für das wandernde Rauschen).
    pub time: f32,
    /// Stärke des Bildrauschens.
    pub noise: f32,
    /// Zeilenzahl des Bildes (für die Zeilenstruktur).
    pub lines: f32,
    pub _pad: f32,
}

impl FullscreenMaterial for TgtCameraEffect {
    fn fragment_shader() -> ShaderRef {
        "shaders/tgt_camera.wesl".into()
    }
}

#[derive(Component)]
struct TgtCamera;

#[derive(Component)]
struct TgtScreen;

#[derive(Resource)]
struct TgtImage(Handle<Image>);

fn new_image(width: u32) -> Image {
    let mut image = Image::new_target_texture(width, LINES_PX, TextureFormat::Rgba8UnormSrgb, None);
    image.sampler = ImageSampler::nearest();
    image
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let handle = images.add(new_image(LINES_PX * 2));
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            is_active: false,
            clear_color: ClearColorConfig::Custom(sky_color()),
            ..default()
        },
        RenderTarget::from(handle.clone()),
        Projection::Perspective(PerspectiveProjection {
            fov: 0.05,
            near: NEAR_M,
            far: (crate::terrain::VIEW_DISTANCE * 1.5) as f32,
            ..default()
        }),
        Tonemapping::None,
        DebandDither::Disabled,
        Msaa::Off,
        TgtCameraEffect { time: 0.0, noise: 1.0, lines: LINES_PX as f32, _pad: 0.0 },
        Transform::default(),
        TgtCamera,
    ));
    commands.spawn((
        Sprite { image: handle.clone(), custom_size: Some(Vec2::ONE), ..default() },
        Transform::from_xyz(0.0, 0.0, -6.0),
        RenderLayers::layer(HUD_LAYER),
        Visibility::Hidden,
        TgtScreen,
    ));
    commands.insert_resource(TgtImage(handle));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update(
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    state: Res<SimState>,
    view: Res<MfdView>,
    origin: Res<WorldOrigin>,
    image: Res<TgtImage>,
    mut images: ResMut<Assets<Image>>,
    mut camera: Single<
        (&mut Camera, &mut Transform, &mut Projection, &mut TgtCameraEffect),
        (With<TgtCamera>, Without<TgtScreen>),
    >,
    mut screen: Single<(&mut Transform, &mut Sprite, &mut Visibility), (With<TgtScreen>, Without<TgtCamera>)>,
) {
    let (cam, cam_transform, projection, effect) = &mut *camera;
    let (sprite_transform, sprite, visibility) = &mut *screen;
    let h = window.height();
    let mfd = layout_for(&window).mfd_layout();
    let fd = state.sim.flight_data();
    let target = fd.target.filter(|t| t.optical);
    let (Some(m), Some(t), RightPage::Target) = (mfd, target, view.0.right_page) else {
        cam.is_active = false;
        **visibility = Visibility::Hidden;
        return;
    };

    // Bildfeld der Seite → Sprite; Texturbreite folgt dem Seitenverhältnis.
    let rect = target_image_rect(m.right_screen);
    let size = Vec2::new(rect.max[0] - rect.min[0], rect.max[1] - rect.min[1]) * h;
    let c = rect.center();
    sprite_transform.translation = Vec3::new(c[0] * h, c[1] * h, -6.0);
    sprite.custom_size = Some(size);
    let width = ((LINES_PX as f32 * size.x / size.y.max(1.0)).round() as u32).clamp(16, 1024);
    if let Some(mut img) = images.get_mut(&image.0)
        && img.width() != width
    {
        img.resize(Extent3d { width, height: LINES_PX, depth_or_array_layers: 1 });
    }

    // Kamera unter dem Rumpf, Blick auf die Zielmitte, Zoom nach Entfernung.
    let pose = state.render_pose();
    let [x, y, z, w] = coords::quat_ned_to_yup(pose.attitude);
    let rot = Quat::from_xyzw(x as f32, y as f32, z as f32, w as f32).normalize();
    let eye = rot * MOUNT;
    let tgt = &state.sim.targets()[t.index];
    let [n, e, d] = tgt.center_ned();
    let aim = Vec3::from_array(coords::relative_yup(f29_core::DVec3::new(n, e, d), origin.0));
    cam_transform.translation = eye;
    cam_transform.look_at(aim, Vec3::Y);
    let range = (aim - eye).length().max(1.0) as f64;
    let half = (tgt.class.radius() / TARGET_FILL).atan2(range) as f32;
    if let Projection::Perspective(p) = &mut **projection {
        p.fov = (2.0 * half).clamp(FOV_MIN_RAD, FOV_MAX_RAD);
    }
    // Mit der Entfernung wird das Bild unruhiger.
    effect.time = time.elapsed_secs_wrapped();
    effect.noise = 0.6 + 0.8 * (t.range / f29_core::sensors::OPTICAL_RANGE_M) as f32;
    cam.is_active = true;
    **visibility = Visibility::Inherited;
}
