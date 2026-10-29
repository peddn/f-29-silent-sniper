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
}

/// Fester Weltpunkt (NED) eines Ziels.
#[derive(Component)]
struct TargetAnchor(DVec3);

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
    for t in state.sim.targets() {
        let mesh = meshes.add(to_bevy_mesh(target_mesh::target(t.class, t.civilian)));
        let e = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform::from_rotation(Quat::from_rotation_y(-(t.heading as f32))),
                TargetAnchor(DVec3::new(t.north, t.east, -t.elevation)),
            ))
            .id();
        targets.entities.push(e);
    }
}

/// Relativ zum Floating Origin positionieren.
fn position_targets(origin: Res<WorldOrigin>, mut q: Query<(&TargetAnchor, &mut Transform)>) {
    for (anchor, mut t) in &mut q {
        t.translation = Vec3::from_array(coords::relative_yup(anchor.0, origin.0));
    }
}
