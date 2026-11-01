//! Feindliche Jäger als 3D-Modelle (M-022): ein Modell je Jäger, sichtbar,
//! solange er fliegt und in Sichtweite ist (Tag 15 km, Nacht 5 km). In der
//! Ferne wächst das Modell auf mindestens gut einen Retro-Bildpunkt – ein
//! Punkt am Himmel – und der Dunst verschluckt es nicht.

use crate::sim::{SimState, SimSystems};
use crate::terrain::{to_bevy_mesh, vertex_color_material};
use crate::view::{WorldOrigin, place, rotation};
use bevy::prelude::*;
use f29_core::DQuat;
use f29_render::model;

/// Mindestgröße: so viel Bogenmaß überspannt das Modell mindestens (ein
/// Retro-Bildpunkt sind rund 2,5 mrad).
const MIN_ANGULAR: f32 = 0.006;
const LENGTH_M: f32 = 14.0;
/// Sichtweite am Tag bzw. in der Nacht (m).
const SIGHT_DAY_M: f32 = 15_000.0;
const SIGHT_NIGHT_M: f32 = 5_000.0;

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
    let material = materials.add(StandardMaterial { fog_enabled: false, ..vertex_color_material() });
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
    let sight = if state.sim.config().spawn.night { SIGHT_NIGHT_M } else { SIGHT_DAY_M };
    for (m, mut t, mut vis) in &mut q {
        let at = list.get(m.0 as usize).filter(|f| f.airborne()).map(|f| (f, place(f.position, origin.0)));
        let Some((f, at)) = at.filter(|(_, at)| at.length() <= sight) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Visible;
        t.translation = at;
        t.rotation = rotation(DQuat::from_euler(f.heading, f.pitch, f.bank));
        t.scale = Vec3::splat((t.translation.length() * MIN_ANGULAR / LENGTH_M).max(1.0));
    }
}
