//! Flugplätze: Bahnen und PAPI-Anflughilfe darstellen.

use crate::sim::{SimState, SimSystems};
use crate::terrain::{to_bevy_mesh, vertex_color_material};
use crate::view::WorldOrigin;
use bevy::prelude::*;
use f29_core::DVec3;
use f29_render::runway_mesh::{self, PapiLight};
use f29_render::{coords, palette};

pub struct AirfieldPlugin;

impl Plugin for AirfieldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Airfields>().add_systems(
            Update,
            (spawn_airfields, position_airfields, update_papi)
                .chain()
                .in_set(SimSystems::Present)
                .after(crate::view::update_camera_and_aircraft),
        );
    }
}

#[derive(Resource, Default)]
struct Airfields {
    /// Seed, für den die Bahnen erzeugt wurden.
    seed: Option<u64>,
    entities: Vec<Entity>,
    white: Handle<Mesh>,
    red: Handle<Mesh>,
}

/// Fester Weltpunkt (NED) eines Bahn- oder Lichtobjekts.
#[derive(Component)]
struct Anchor(DVec3);

#[derive(Component)]
struct Papi {
    light: PapiLight,
    runway: usize,
}

fn spawn_airfields(
    mut commands: Commands,
    mut fields: ResMut<Airfields>,
    state: Res<SimState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let terrain = state.sim.terrain();
    if fields.seed == Some(terrain.seed()) {
        return;
    }
    for e in fields.entities.drain(..) {
        commands.entity(e).despawn();
    }
    fields.seed = Some(terrain.seed());
    let material = materials.add(vertex_color_material());
    fields.white = meshes.add(to_bevy_mesh(runway_mesh::light_box(palette::srgb(6))));
    fields.red = meshes.add(to_bevy_mesh(runway_mesh::light_box(palette::srgb(15))));
    for (i, rw) in terrain.runways().iter().enumerate() {
        let center = DVec3::new(rw.center[0], rw.center[1], -rw.elevation_m);
        let e = commands
            .spawn((
                Mesh3d(meshes.add(to_bevy_mesh(runway_mesh::runway(rw)))),
                MeshMaterial3d(material.clone()),
                Transform::default(),
                Anchor(center),
            ))
            .id();
        fields.entities.push(e);
        for light in runway_mesh::papi_lights(rw) {
            let e = commands
                .spawn((
                    Mesh3d(fields.red.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    Anchor(DVec3::new(light.north, light.east, -light.elevation_m)),
                    Papi { light, runway: i },
                ))
                .id();
            fields.entities.push(e);
        }
    }
}

/// Relativ zum Floating Origin positionieren (wie die Terrain-Kacheln).
fn position_airfields(origin: Res<WorldOrigin>, mut q: Query<(&Anchor, &mut Transform)>) {
    for (anchor, mut t) in &mut q {
        t.translation = Vec3::from_array(coords::relative_yup(anchor.0, origin.0));
    }
}

/// PAPI: weiß über dem Grenzwinkel, rot darunter; nur für die passende
/// Landerichtung sichtbar.
fn update_papi(
    state: Res<SimState>,
    fields: Res<Airfields>,
    mut q: Query<(&Papi, &mut Mesh3d, &mut Visibility)>,
) {
    let p = state.render_pose().position;
    let runways = state.sim.terrain().runways();
    for (papi, mut mesh, mut vis) in &mut q {
        let Some(rw) = runways.get(papi.runway) else {
            continue;
        };
        let want_vis =
            if papi.light.faces(rw, p.x, p.y) { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want_vis {
            *vis = want_vis;
        }
        let white = papi.light.is_white(p.x, p.y, -p.z);
        let handle = if white { &fields.white } else { &fields.red };
        if mesh.0 != *handle {
            mesh.0 = handle.clone();
        }
    }
}
