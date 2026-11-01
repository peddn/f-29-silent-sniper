//! Feindliche Jäger als 3D-Modelle (M-022): ein Modell je Jäger, sichtbar,
//! solange er fliegt.

use crate::sim::{SimState, SimSystems};
use crate::terrain::{to_bevy_mesh, vertex_color_material};
use crate::view::{WorldOrigin, place, rotation};
use bevy::prelude::*;
use f29_core::DQuat;
use f29_render::model;

/// Mindestgröße nach Entfernung (Bogenmaß je Meter Modelllänge), damit ein
/// ferner Jäger im Retro-Raster nicht verschwindet.
const MIN_ANGULAR: f32 = 0.0015;
const LENGTH_M: f32 = 14.0;

pub struct FightersPlugin;

impl Plugin for FightersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Models>().add_systems(
            Update,
            (spawn_models, position_models)
                .chain()
                .in_set(SimSystems::Present)
                .after(crate::view::update_camera_and_aircraft),
        );
    }
}

#[derive(Resource, Default)]
struct Models {
    seed: Option<u64>,
    entities: Vec<Entity>,
}

/// Nummer des Jägers.
#[derive(Component)]
struct FighterModel(u32);

fn spawn_models(
    mut commands: Commands,
    mut models: ResMut<Models>,
    state: Res<SimState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let seed = state.sim.terrain().seed();
    if models.seed == Some(seed) {
        return;
    }
    for e in models.entities.drain(..) {
        commands.entity(e).despawn();
    }
    models.seed = Some(seed);
    let mesh = meshes.add(to_bevy_mesh(model::fighter()));
    let material = materials.add(vertex_color_material());
    for f in &state.sim.fighters().list {
        let e = commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Visibility::Hidden,
                FighterModel(f.id),
            ))
            .id();
        models.entities.push(e);
    }
}

fn position_models(
    origin: Res<WorldOrigin>,
    state: Res<SimState>,
    mut q: Query<(&FighterModel, &mut Transform, &mut Visibility)>,
) {
    let list = &state.sim.fighters().list;
    for (m, mut t, mut vis) in &mut q {
        let Some(f) = list.get(m.0 as usize).filter(|f| f.airborne()) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Visible;
        t.translation = place(f.position, origin.0);
        t.rotation = rotation(DQuat::from_euler(f.heading, f.pitch, f.bank));
        t.scale = Vec3::splat((t.translation.length() * MIN_ANGULAR / LENGTH_M).max(1.0));
    }
}
