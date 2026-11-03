//! Was vom Cockpit und vom Piloten zu sehen ist (M-032, M-035):
//! - Blick links, rechts, hinten (Strg+J/L/K): Vollbild wie im Original mit
//!   einer festen, gefüllten Silhouette – seitlich ein schmaler Cockpitrand,
//!   hinten Sitz, Kopfstütze und Kanzelrahmen.
//! - Ausgestiegen: der Fallschirm als 3D-Kuppel mit Leinen über dem Piloten
//!   und ein freier Kopfblick per Maus, solange er am Schirm hängt.

use crate::HUD_LAYER;
use crate::sim::{SimState, SimSystems};
use crate::terrain::{to_bevy_mesh, vertex_color_material};
use crate::view::{ViewMode, ViewSettings};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use f29_core::sensors::LookDir;
use f29_render::mesh::{MeshData, srgb_to_linear};
use f29_render::palette;

pub struct CockpitPlugin;

impl Plugin for CockpitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeadLook>().add_systems(Startup, setup).add_systems(
            Update,
            (head_look, show_frames, show_chute)
                .chain()
                .in_set(SimSystems::Present)
                .before(crate::view::update_camera_and_aircraft),
        );
    }
}

/// Kopfblick am Schirm (rad): links positiv, oben positiv.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct HeadLook {
    pub yaw: f32,
    pub pitch: f32,
}

/// Mausempfindlichkeit (rad je Pixel).
const MOUSE_RAD_PER_PX: f32 = 0.003;
/// Der Schirm: Höhe der Kuppel über dem Piloten (m), Radius am Rand (m).
const CHUTE_TOP_M: f32 = 8.0;
const CHUTE_RIM_M: f32 = 6.0;
const CHUTE_RADIUS_M: f32 = 4.5;
const CHUTE_GORES: usize = 12;
/// Gurt über dem Kopf (m) und halbe Breite einer Leine (m).
const HARNESS_M: f32 = 0.6;
const RISER_M: f32 = 0.03;

#[derive(Component)]
struct Frame(LookDir);

#[derive(Component)]
struct Chute;

/// Silhouetten in Bildschirmanteilen (x, y je −1..1, y oben) als konvexe
/// Vielecke; eigene Formen, das Original liefert keine.
fn frame_polygons(look: LookDir) -> Vec<Vec<[f32; 2]>> {
    let mirror = |p: &[[f32; 2]]| p.iter().map(|&[x, y]| [-x, y]).rev().collect::<Vec<_>>();
    // Blick nach links: Kanzelrand unten, vorn (rechts im Bild) der Kanzelbogen.
    let left = vec![
        vec![[-1.0, -1.0], [1.0, -1.0], [1.0, -0.78], [-1.0, -0.86]],
        vec![[0.78, -0.78], [0.9, -0.78], [0.74, 1.0], [0.62, 1.0]],
    ];
    match look {
        LookDir::Forward => Vec::new(),
        LookDir::Left => left,
        LookDir::Right => left.iter().map(|p| mirror(p)).collect(),
        LookDir::Aft => vec![
            // Kanzelrahmen links und rechts
            vec![[-1.0, -1.0], [-0.62, -1.0], [-0.8, -0.5], [-1.0, -0.4]],
            mirror(&[[-1.0, -1.0], [-0.62, -1.0], [-0.8, -0.5], [-1.0, -0.4]]),
            // Sitz mit Kopfstütze
            vec![[-0.3, -1.0], [0.3, -1.0], [0.26, -0.45], [-0.26, -0.45]],
            vec![[-0.18, -0.45], [0.18, -0.45], [0.15, -0.18], [-0.15, -0.18]],
        ],
    }
}

/// Ein 2D-Netz aus konvexen Vielecken (als Fächer zerlegt).
fn polygon_mesh(polys: &[Vec<[f32; 2]>]) -> Mesh {
    let mut positions = Vec::new();
    for p in polys {
        for k in 1..p.len() - 1 {
            for v in [p[0], p[k], p[k + 1]] {
                positions.push([v[0], v[1], 0.0]);
            }
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
}

/// Kuppel von innen gesehen (Normalen nach unten), Bahnen abwechselnd
/// signalrot und weiß.
fn chute_mesh() -> MeshData {
    let mut m = MeshData::with_capacity(CHUTE_GORES * 4);
    let ring = |k: usize, r: f32, y: f32| {
        let a = k as f32 / CHUTE_GORES as f32 * std::f32::consts::TAU;
        [r * a.cos(), y, r * a.sin()]
    };
    let mid_r = CHUTE_RADIUS_M * 0.75;
    let mid_y = CHUTE_RIM_M + (CHUTE_TOP_M - CHUTE_RIM_M) * 0.7;
    for k in 0..CHUTE_GORES {
        let color = srgb_to_linear(if k % 2 == 0 { palette::srgb(15) } else { palette::srgb(6) });
        let n = [0.0, -1.0, 0.0];
        let top = [0.0, CHUTE_TOP_M, 0.0];
        let (a, b) = (ring(k, mid_r, mid_y), ring(k + 1, mid_r, mid_y));
        let (c, d) = (ring(k, CHUTE_RADIUS_M, CHUTE_RIM_M), ring(k + 1, CHUTE_RADIUS_M, CHUTE_RIM_M));
        // Beide Umlaufrichtungen: von innen und außen sichtbar.
        for (p, q, r) in [(top, a, b), (a, c, d), (a, d, b)] {
            m.push_raw(p, q, r, n, color);
            m.push_raw(p, r, q, n, color);
        }
        // Leine vom Rand zum Gurt über dem Kopf: zwei gekreuzte schmale
        // Streifen, damit sie aus jeder Richtung zu sehen ist.
        let line = srgb_to_linear(palette::srgb(12));
        let harness = [0.0, HARNESS_M, 0.0];
        for w in [[RISER_M, 0.0, 0.0], [0.0, 0.0, RISER_M]] {
            let off = |p: [f32; 3], s: f32| [p[0] + w[0] * s, p[1], p[2] + w[2] * s];
            for (p, q, r) in [
                (off(c, -1.0), off(c, 1.0), off(harness, 0.05)),
                (off(c, -1.0), off(harness, 0.05), off(harness, -0.05)),
            ] {
                m.push_raw(p, q, r, n, line);
                m.push_raw(p, r, q, n, line);
            }
        }
    }
    m
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut colors: ResMut<Assets<ColorMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let [r, g, b] = palette::mix(0, 13, 0.28);
    let frame_color = colors.add(Color::srgb(r, g, b));
    for look in [LookDir::Left, LookDir::Right, LookDir::Aft] {
        commands.spawn((
            Mesh2d(meshes.add(polygon_mesh(&frame_polygons(look)))),
            MeshMaterial2d(frame_color.clone()),
            Transform::from_xyz(0.0, 0.0, -5.0),
            Visibility::Hidden,
            RenderLayers::layer(HUD_LAYER),
            Frame(look),
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(to_bevy_mesh(chute_mesh()))),
        MeshMaterial3d(materials.add(StandardMaterial { cull_mode: None, ..vertex_color_material() })),
        Transform::default(),
        Visibility::Hidden,
        Chute,
    ));
}

/// Silhouette der aktuellen Blickrichtung, auf Fenstergröße gestreckt.
fn show_frames(
    window: Single<&Window, With<PrimaryWindow>>,
    view: Res<ViewSettings>,
    state: Res<SimState>,
    mut frames: Query<(&Frame, &mut Transform, &mut Visibility)>,
) {
    let scale = Vec3::new(window.width() * 0.5, window.height() * 0.5, 1.0);
    let cockpit = view.mode == ViewMode::Cockpit && !view.ejected && !state.covered();
    for (frame, mut t, mut vis) in &mut frames {
        let want = if cockpit && frame.0 == view.look { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        t.scale = scale;
    }
}

/// Kopfblick per Maus, solange der Pilot am Schirm hängt; der Zeiger ist dann
/// gefangen und unsichtbar.
fn head_look(
    state: Res<SimState>,
    motion: Res<AccumulatedMouseMotion>,
    mut look: ResMut<HeadLook>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let fd = &state.fd;
    let hanging = fd.pilot.is_some_and(|p| p.chute > 0.0) && !fd.mission_over && !state.paused;
    let grab = if hanging { CursorGrabMode::Locked } else { CursorGrabMode::None };
    if cursor.grab_mode != grab {
        cursor.grab_mode = grab;
        cursor.visible = !hanging;
    }
    if !hanging {
        *look = HeadLook::default();
        return;
    }
    look.yaw -= motion.delta.x * MOUSE_RAD_PER_PX;
    look.pitch = (look.pitch - motion.delta.y * MOUSE_RAD_PER_PX).clamp(-1.4, 1.5);
}

/// Der Schirm über dem Piloten: entfaltet sich, bleibt aufrecht.
fn show_chute(state: Res<SimState>, mut chute: Single<(&mut Transform, &mut Visibility), With<Chute>>) {
    let (t, vis) = &mut *chute;
    let open = state.fd.pilot.map_or(0.0, |p| p.chute as f32);
    let want = if open > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
    if **vis != want {
        **vis = want;
    }
    let spread = 0.3 + 0.7 * open;
    t.translation = Vec3::ZERO;
    t.scale = Vec3::new(spread, 1.0, spread);
}
