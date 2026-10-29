//! Waffen in der 3D-Ansicht (M-014): Bomben und Flugkörper im Flug, der
//! Bremsschirm der Fallschirmbombe und ein kurzer Blitz beim Einschlag.

use crate::sim::{SimState, SimSystems};
use crate::view::WorldOrigin;
use bevy::prelude::*;
use f29_core::DVec3;
use f29_core::weapons::{WeaponEvent, WeaponKind};
use f29_render::{coords, palette};
use std::collections::BTreeMap;

pub struct WeaponsPlugin;

impl Plugin for WeaponsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponVisuals>().add_systems(Startup, setup).add_systems(
            Update,
            (sync_weapons, spawn_blasts, animate_blasts)
                .chain()
                .in_set(SimSystems::Present)
                .after(crate::view::update_camera_and_aircraft),
        );
    }
}

/// Dauer des Einschlagblitzes (s) und größter Radius (m).
const BLAST_S: f32 = 1.2;
const BLAST_RADIUS_M: f32 = 35.0;

#[derive(Resource, Default)]
struct WeaponVisuals {
    /// Waffen-Nummer → Entität.
    entities: BTreeMap<u32, Entity>,
    /// Zuletzt ausgewertetes Ereignis (Tick), für die Blitze.
    last_event_tick: u64,
    /// Seed der Simulation: bei Neustart alles entfernen.
    seed: Option<u64>,
    body: Handle<Mesh>,
    chute: Handle<Mesh>,
    sphere: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    chute_material: Handle<StandardMaterial>,
}

#[derive(Component)]
struct WeaponModel;

#[derive(Component)]
struct Chute;

/// Einschlagblitz an festem Weltpunkt (NED).
#[derive(Component)]
struct Blast {
    at: DVec3,
    age: f32,
    radius: f32,
}

fn color(index: usize) -> Color {
    let [r, g, b] = palette::srgb(index);
    Color::srgb(r, g, b)
}

fn setup(
    mut visuals: ResMut<WeaponVisuals>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    visuals.body = meshes.add(Cuboid::new(0.4, 0.4, 3.2));
    visuals.chute = meshes.add(Cone { radius: 0.9, height: 1.4 });
    visuals.sphere = meshes.add(Sphere::new(1.0).mesh().ico(2).expect("Ikosaeder"));
    visuals.material =
        materials.add(StandardMaterial { base_color: color(13), perceptual_roughness: 0.9, ..default() });
    visuals.chute_material =
        materials.add(StandardMaterial { base_color: color(10), unlit: true, cull_mode: None, ..default() });
}

fn place(at: DVec3, origin: DVec3) -> Vec3 {
    Vec3::from_array(coords::relative_yup(at, origin))
}

fn rotation(q: f29_core::DQuat) -> Quat {
    let [x, y, z, w] = coords::quat_ned_to_yup(q);
    Quat::from_xyzw(x as f32, y as f32, z as f32, w as f32).normalize()
}

/// Entitäten zu den Waffen im Flug anlegen, bewegen und entfernen.
fn sync_weapons(
    mut commands: Commands,
    state: Res<SimState>,
    origin: Res<WorldOrigin>,
    mut visuals: ResMut<WeaponVisuals>,
    mut models: Query<&mut Transform, (With<WeaponModel>, Without<Chute>)>,
    mut chutes: Query<(&ChildOf, &mut Visibility), With<Chute>>,
) {
    let seed = state.sim.config().seed;
    let restarted = visuals.seed != Some(seed) || state.sim.flight_data().tick < visuals.last_event_tick;
    if restarted {
        for (_, e) in std::mem::take(&mut visuals.entities) {
            commands.entity(e).despawn();
        }
        visuals.seed = Some(seed);
        visuals.last_event_tick = 0;
    }
    let flying = state.sim.weapons_in_flight();
    // Gelandete Waffen entfernen.
    let gone: Vec<u32> =
        visuals.entities.keys().copied().filter(|id| !flying.iter().any(|w| w.id == *id)).collect();
    for id in gone {
        if let Some(e) = visuals.entities.remove(&id) {
            commands.entity(e).despawn();
        }
    }
    for w in flying {
        let translation = place(w.position, origin.0);
        let rot = rotation(w.attitude());
        match visuals.entities.get(&w.id) {
            Some(&e) => {
                if let Ok(mut t) = models.get_mut(e) {
                    t.translation = translation;
                    t.rotation = rot;
                }
                for (parent, mut vis) in &mut chutes {
                    if parent.parent() == e {
                        *vis = if w.chute_open() { Visibility::Inherited } else { Visibility::Hidden };
                    }
                }
            }
            None => {
                let mut entity = commands.spawn((
                    Mesh3d(visuals.body.clone()),
                    MeshMaterial3d(visuals.material.clone()),
                    Transform::from_translation(translation).with_rotation(rot),
                    WeaponModel,
                ));
                if w.kind == WeaponKind::Retarded {
                    // Schirm hinter dem Heck (Modell-Nase zeigt nach −z).
                    entity.with_child((
                        Mesh3d(visuals.chute.clone()),
                        MeshMaterial3d(visuals.chute_material.clone()),
                        Transform::from_xyz(0.0, 0.0, 3.2)
                            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                        Visibility::Hidden,
                        Chute,
                    ));
                }
                let id = entity.id();
                visuals.entities.insert(w.id, id);
            }
        }
    }
}

/// Für jeden neuen Einschlag einen Blitz erzeugen.
fn spawn_blasts(
    mut commands: Commands,
    state: Res<SimState>,
    mut visuals: ResMut<WeaponVisuals>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let last = visuals.last_event_tick;
    let mut newest = last;
    for &(tick, e) in state.sim.weapon_events() {
        if tick <= last {
            continue;
        }
        newest = newest.max(tick);
        if let WeaponEvent::Impact { kind, position, .. } = e {
            let radius = BLAST_RADIUS_M * (kind.spec().lethal_radius_m / 30.0).clamp(0.4, 1.0) as f32;
            commands.spawn((
                Mesh3d(visuals.sphere.clone()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: color(15),
                    emissive: LinearRgba::rgb(4.0, 1.6, 0.4),
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                })),
                Transform::from_scale(Vec3::splat(0.1)),
                Blast { at: position, age: 0.0, radius },
            ));
        }
    }
    visuals.last_event_tick = newest;
}

fn animate_blasts(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<SimState>,
    origin: Res<WorldOrigin>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut blasts: Query<(Entity, &mut Blast, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
) {
    let dt = if state.paused { 0.0 } else { time.delta_secs() };
    for (e, mut blast, mut t, mat) in &mut blasts {
        blast.age += dt;
        let k = blast.age / BLAST_S;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        // Schnell aufblähen, dann verblassen.
        let grow = 1.0 - (1.0 - k.min(0.4) / 0.4).powi(2);
        t.translation = place(blast.at, origin.0);
        t.scale = Vec3::splat(blast.radius * grow.max(0.05));
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.base_color.set_alpha(1.0 - k);
        }
    }
}
