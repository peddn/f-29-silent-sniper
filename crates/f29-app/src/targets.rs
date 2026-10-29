//! Bodenziele als 3D-Modelle (M-010).

use crate::sim::{SimState, SimSystems};
use crate::terrain::{to_bevy_mesh, vertex_color_material};
use crate::view::WorldOrigin;
use bevy::prelude::*;
use f29_core::DVec3;
use f29_render::{coords, target_mesh};

pub struct TargetsPlugin;

impl Plugin for TargetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Targets>().add_systems(
            Update,
            (spawn_targets, position_targets)
                .chain()
                .in_set(SimSystems::Present)
                .after(crate::view::update_camera_and_aircraft),
        );
    }
}

#[derive(Resource, Default)]
struct Targets {
    /// Seed, für den die Ziele erzeugt wurden.
    seed: Option<u64>,
    entities: Vec<Entity>,
    /// Material der Wracks (dunkel, ohne Eckfarben).
    wreck: Handle<StandardMaterial>,
    intact: Handle<StandardMaterial>,
}

/// Fester Weltpunkt (NED) eines Ziels und sein Index in `Sim::targets`.
#[derive(Component)]
struct TargetAnchor(DVec3, usize);

/// Höhe eines Wracks relativ zum heilen Modell.
const WRECK_HEIGHT: f32 = 0.25;

fn spawn_targets(
    mut commands: Commands,
    mut targets: ResMut<Targets>,
    state: Res<SimState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let seed = state.sim.terrain().seed();
    if targets.seed == Some(seed) {
        return;
    }
    for e in targets.entities.drain(..) {
        commands.entity(e).despawn();
    }
    targets.seed = Some(seed);
    let material = materials.add(vertex_color_material());
    let [r, g, b] = f29_render::palette::srgb(0);
    targets.wreck = materials.add(StandardMaterial {
        base_color: Color::srgb(r, g, b),
        perceptual_roughness: 1.0,
        ..default()
    });
    targets.intact = material.clone();
    for (index, t) in state.sim.targets().iter().enumerate() {
        let mesh = meshes.add(to_bevy_mesh(target_mesh::target(t.class, t.civilian)));
        let e = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform::from_rotation(Quat::from_rotation_y(-(t.heading as f32))),
                TargetAnchor(DVec3::new(t.north, t.east, -t.elevation), index),
            ))
            .id();
        targets.entities.push(e);
    }
}

/// Relativ zum Floating Origin positionieren; zerstörte Ziele als flaches,
/// dunkles Wrack zeigen.
fn position_targets(
    origin: Res<WorldOrigin>,
    state: Res<SimState>,
    targets: Res<Targets>,
    mut q: Query<(&TargetAnchor, &mut Transform, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let sim_targets = state.sim.targets();
    for (anchor, mut t, mut material) in &mut q {
        t.translation = Vec3::from_array(coords::relative_yup(anchor.0, origin.0));
        let destroyed = sim_targets.get(anchor.1).is_some_and(|t| !t.is_intact());
        let (scale, wanted) = if destroyed { (WRECK_HEIGHT, &targets.wreck) } else { (1.0, &targets.intact) };
        t.scale = Vec3::new(1.0, scale, 1.0);
        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
    }
}
