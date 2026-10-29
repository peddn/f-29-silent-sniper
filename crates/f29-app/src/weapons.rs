//! Waffen in der 3D-Ansicht (M-014): Bomben und Flugkörper im Flug, der
//! Bremsschirm der Fallschirmbombe und ein kurzer Blitz beim Einschlag;
//! dazu die Leuchtspur der Bordkanone und kleine Einschlagwölkchen (M-016).

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
            (sync_weapons, spawn_blasts, animate_blasts, sync_tracers, spawn_puffs, animate_puffs)
                .chain()
                .in_set(SimSystems::Present)
                .after(crate::view::update_camera_and_aircraft),
        );
    }
}

/// Leuchtspur: Länge (m) und Dicke (m) einer Patrone.
const TRACER_LENGTH_M: f32 = 14.0;
const TRACER_WIDTH_M: f32 = 0.35;
/// Leuchtspur erst zeigen, wenn die Patrone so lange unterwegs ist (s, ≈ 60 m).
const TRACER_DELAY_S: f64 = 0.05;
/// Kleinste Größe als Winkel (rad je m Entfernung): Im Retro-Raster (ein
/// virtuelles Pixel ≈ 2–3 mrad) verschwänden Spur und Wölkchen sonst schon
/// nach wenigen hundert Metern.
const MIN_ANGULAR: f32 = 0.004;
/// Einschlagwölkchen der Kanone: Dauer (s) und Radius (m) am Boden bzw. am Ziel.
const PUFF_S: f32 = 0.6;
const PUFF_GROUND_M: f32 = 6.0;
const PUFF_HIT_M: f32 = 8.0;

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
    tracer: Handle<Mesh>,
    tracer_material: Handle<StandardMaterial>,
    puff_ground: Handle<StandardMaterial>,
    puff_hit: Handle<StandardMaterial>,
    /// Entitäten der Leuchtspur, nach Index der Patrone wiederverwendet.
    tracers: Vec<Entity>,
    /// Zuletzt ausgewerteter Kanoneneinschlag (Tick).
    last_gun_tick: u64,
}

#[derive(Component)]
struct Tracer;

/// Einschlagwölkchen der Kanone an festem Weltpunkt (NED).
#[derive(Component)]
struct Puff {
    at: DVec3,
    age: f32,
    radius: f32,
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
    visuals.tracer = meshes.add(Cuboid::new(TRACER_WIDTH_M, TRACER_WIDTH_M, TRACER_LENGTH_M));
    visuals.tracer_material =
        materials.add(StandardMaterial { base_color: color(10), unlit: true, ..default() });
    // Staub hell, damit er sich vom Gelände abhebt.
    visuals.puff_ground = materials.add(StandardMaterial { base_color: color(14), unlit: true, ..default() });
    visuals.puff_hit = materials.add(StandardMaterial { base_color: color(15), unlit: true, ..default() });
}

/// Leuchtspur: eine Entität je Patrone im Flug, in Flugrichtung gestreckt.
fn sync_tracers(
    mut commands: Commands,
    state: Res<SimState>,
    origin: Res<WorldOrigin>,
    mut visuals: ResMut<WeaponVisuals>,
    mut tracers: Query<(&mut Transform, &mut Visibility), With<Tracer>>,
) {
    let rounds = &state.sim.gun().in_flight;
    while visuals.tracers.len() < rounds.len() {
        let e = commands
            .spawn((
                Mesh3d(visuals.tracer.clone()),
                MeshMaterial3d(visuals.tracer_material.clone()),
                Transform::default(),
                Visibility::Hidden,
                Tracer,
            ))
            .id();
        visuals.tracers.push(e);
    }
    for (k, &e) in visuals.tracers.iter().enumerate() {
        let Ok((mut t, mut vis)) = tracers.get_mut(e) else {
            continue;
        };
        match rounds.get(k) {
            // Erst ein Stück vor dem Flugzeug sichtbar.
            Some(r) if r.age_s >= TRACER_DELAY_S => {
                let dir = Vec3::from_array(coords::relative_yup(r.position + r.velocity, r.position));
                // Die Spur liegt hinter der Patrone.
                let at = place(r.position, origin.0) - dir.normalize_or_zero() * (TRACER_LENGTH_M * 0.5);
                // In der Ferne dicker, damit sie im Retro-Raster sichtbar bleibt.
                let width = (at.length() * MIN_ANGULAR / TRACER_WIDTH_M).max(1.0);
                *t = Transform::from_translation(at)
                    .looking_to(dir, Vec3::Y)
                    .with_scale(Vec3::new(width, width, 1.0));
                *vis = Visibility::Inherited;
            }
            _ => *vis = Visibility::Hidden,
        }
    }
}

/// Für jeden neuen Kanoneneinschlag ein kleines Wölkchen.
fn spawn_puffs(mut commands: Commands, state: Res<SimState>, mut visuals: ResMut<WeaponVisuals>) {
    let impacts = &state.sim.gun().impacts;
    let newest = impacts.last().map_or(0, |i| i.tick);
    if newest < visuals.last_gun_tick {
        // Neustart
        visuals.last_gun_tick = 0;
    }
    for i in impacts.iter().filter(|i| i.tick > visuals.last_gun_tick) {
        let (material, radius) = if i.hit {
            (visuals.puff_hit.clone(), PUFF_HIT_M)
        } else {
            (visuals.puff_ground.clone(), PUFF_GROUND_M)
        };
        commands.spawn((
            Mesh3d(visuals.sphere.clone()),
            MeshMaterial3d(material),
            Transform::from_scale(Vec3::splat(0.1)),
            Puff { at: i.position, age: 0.0, radius },
        ));
    }
    visuals.last_gun_tick = visuals.last_gun_tick.max(newest);
}

fn animate_puffs(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<SimState>,
    origin: Res<WorldOrigin>,
    mut puffs: Query<(Entity, &mut Puff, &mut Transform)>,
) {
    let dt = if state.paused { 0.0 } else { time.delta_secs() };
    for (e, mut puff, mut t) in &mut puffs {
        puff.age += dt;
        let k = puff.age / PUFF_S;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        // Aufpuffen, dann wieder schrumpfen.
        let size = (k * std::f32::consts::PI).sin();
        let at = place(puff.at, origin.0);
        // Mindestgröße nach Entfernung (Retro-Raster), etwas über dem Boden.
        let radius = puff.radius.max(at.length() * MIN_ANGULAR * 1.5) * size.max(0.05);
        t.translation = at + Vec3::Y * radius * 0.4;
        t.scale = Vec3::splat(radius);
    }
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
