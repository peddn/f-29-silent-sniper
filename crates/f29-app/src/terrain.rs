//! Terrain-Streaming: Kacheln um das Flugzeug im Hintergrund erzeugen.

use crate::sim::{SimState, SimSystems};
use crate::view::WorldOrigin;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::tasks::futures::check_ready;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use f29_render::MeshData;
use f29_render::coords;
use f29_render::terrain_mesh::{CHUNK_SIZE, ChunkKey, Lod, build_chunk};
use std::collections::HashMap;

/// Sichtweite in Metern (Nebel endet kurz davor).
pub const VIEW_DISTANCE: f64 = 26_000.0;
/// Höchstzahl gleichzeitig laufender Kachel-Aufträge.
const MAX_TASKS: usize = 12;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainStreamer>().add_systems(Startup, setup).add_systems(
            Update,
            (stream_chunks, position_chunks)
                .chain()
                .in_set(SimSystems::Present)
                .after(crate::view::update_camera_and_aircraft),
        );
    }
}

/// Übersetzt engine-neutrale Geometrie in ein Bevy-Mesh.
pub fn to_bevy_mesh(data: MeshData) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, data.colors)
}

/// Unbeleuchtetes Material: Licht ist in die Vertexfarben eingebacken.
pub fn vertex_color_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        fog_enabled: true,
        ..default()
    }
}

#[derive(Default)]
struct ChunkSlot {
    entity: Option<Entity>,
    shown: Option<Lod>,
    pending: Option<(Lod, Task<Mesh>)>,
}

#[derive(Resource, Default)]
struct TerrainStreamer {
    chunks: HashMap<ChunkKey, ChunkSlot>,
    material: Handle<StandardMaterial>,
    seed: Option<u64>,
}

#[derive(Component)]
struct TerrainChunk(ChunkKey);

fn setup(mut streamer: ResMut<TerrainStreamer>, mut materials: ResMut<Assets<StandardMaterial>>) {
    streamer.material = materials.add(vertex_color_material());
}

fn stream_chunks(
    mut commands: Commands,
    mut streamer: ResMut<TerrainStreamer>,
    state: Res<SimState>,
    origin: Res<WorldOrigin>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let streamer = &mut *streamer;
    let terrain = state.sim.terrain();

    // Neuer Seed (Neustart mit anderem Gelände): alles verwerfen.
    if streamer.seed != Some(terrain.seed()) {
        for (_, slot) in streamer.chunks.drain() {
            if let Some(e) = slot.entity {
                commands.entity(e).despawn();
            }
        }
        streamer.seed = Some(terrain.seed());
    }

    let (north, east) = (origin.0.x, origin.0.y);
    let center = ChunkKey::containing(north, east);
    let r = (VIEW_DISTANCE / CHUNK_SIZE).ceil() as i32 + 1;

    // Gewünschte Kacheln, nächste zuerst
    let mut wanted: Vec<(f64, ChunkKey, Lod)> = Vec::new();
    for dn in -r..=r {
        for de in -r..=r {
            let key = ChunkKey { north: center.north + dn, east: center.east + de };
            let d = key.distance_to(north, east);
            if let Some(lod) = Lod::for_distance(d, VIEW_DISTANCE) {
                wanted.push((d, key, lod));
            }
        }
    }
    wanted.sort_by(|a, b| a.0.total_cmp(&b.0));

    // Fertige Aufträge übernehmen
    for (key, slot) in &mut streamer.chunks {
        let Some((lod, task)) = slot.pending.as_mut() else {
            continue;
        };
        let Some(mesh) = check_ready(task) else {
            continue;
        };
        let lod = *lod;
        slot.pending = None;
        let handle = meshes.add(mesh);
        match slot.entity {
            Some(e) => {
                commands.entity(e).insert(Mesh3d(handle));
            }
            None => {
                let e = commands
                    .spawn((
                        Mesh3d(handle),
                        MeshMaterial3d(streamer.material.clone()),
                        Transform::default(),
                        TerrainChunk(*key),
                    ))
                    .id();
                slot.entity = Some(e);
            }
        }
        slot.shown = Some(lod);
    }

    // Neue Aufträge vergeben
    let mut in_flight = streamer.chunks.values().filter(|s| s.pending.is_some()).count();
    let pool = AsyncComputeTaskPool::get();
    for (_, key, lod) in &wanted {
        if in_flight >= MAX_TASKS {
            break;
        }
        let slot = streamer.chunks.entry(*key).or_default();
        let pending_lod = slot.pending.as_ref().map(|(l, _)| *l);
        if slot.shown == Some(*lod) || pending_lod == Some(*lod) {
            continue;
        }
        let (terrain, key, lod) = (terrain.clone(), *key, *lod);
        let task = pool.spawn(async move { to_bevy_mesh(build_chunk(&terrain, key, lod)) });
        slot.pending = Some((lod, task));
        in_flight += 1;
    }

    // Kacheln weit außerhalb der Sichtweite entfernen (mit Hysterese)
    let keep = VIEW_DISTANCE + CHUNK_SIZE;
    streamer.chunks.retain(|key, slot| {
        if key.distance_to(north, east) <= keep {
            return true;
        }
        if let Some(e) = slot.entity {
            commands.entity(e).despawn();
        }
        false
    });
}

/// Kacheln relativ zum Floating Origin positionieren.
fn position_chunks(origin: Res<WorldOrigin>, mut chunks: Query<(&TerrainChunk, &mut Transform)>) {
    for (chunk, mut t) in &mut chunks {
        t.translation = Vec3::from_array(coords::relative_yup(chunk.0.origin_ned(), origin.0));
    }
}
