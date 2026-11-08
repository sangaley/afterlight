use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::events::*;
use super::generation::generate_chunk;

const CHUNK_SIZE: f32 = 512.0;

/// Updates loaded chunks based on ship position
pub fn update_chunks(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    ship_query: Query<&Transform, With<Ship>>,
    world_state: Res<WorldState>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut chunk_events: MessageWriter<ChunkEntered>,
    mut prev_chunk: Local<Option<IVec2>>,
) {
    let Ok(ship_transform) = ship_query.single() else {
        return;
    };

    let ship_pos = ship_transform.translation.truncate();
    let current_chunk = world_to_chunk(ship_pos);

    // Send ChunkEntered event when chunk changes
    if *prev_chunk != Some(current_chunk) {
        *prev_chunk = Some(current_chunk);
        chunk_events.write(ChunkEntered {
            chunk_pos: current_chunk,
        });
    }

    // Check which chunks should be loaded
    let render_distance = chunk_manager.render_distance;
    let mut chunks_to_load = Vec::new();

    for x in (current_chunk.x - render_distance)..=(current_chunk.x + render_distance) {
        for y in (current_chunk.y - render_distance)..=(current_chunk.y + render_distance) {
            // Don't load chunks above the surface or below max depth (500m)
            if y > 1 || y < -11 {
                continue;
            }
            let chunk_pos = IVec2::new(x, y);
            if !chunk_manager.loaded_chunks.contains_key(&chunk_pos) {
                chunks_to_load.push(chunk_pos);
            }
        }
    }

    for chunk_pos in chunks_to_load {
        let entity = generate_chunk(&mut commands, &asset_server, chunk_pos, world_state.seed);
        chunk_manager.loaded_chunks.insert(chunk_pos, entity);
    }

    // Unload distant chunks
    let chunks_to_unload: Vec<IVec2> = chunk_manager
        .loaded_chunks
        .keys()
        .filter(|pos| {
            (pos.x - current_chunk.x).abs() > render_distance + 1
                || (pos.y - current_chunk.y).abs() > render_distance + 1
        })
        .copied()
        .collect();

    for chunk_pos in chunks_to_unload {
        if let Some(entity) = chunk_manager.loaded_chunks.remove(&chunk_pos) {
            commands.entity(entity).despawn();
        }
    }
}

fn world_to_chunk(pos: Vec2) -> IVec2 {
    IVec2::new(
        (pos.x / CHUNK_SIZE).floor() as i32,
        (pos.y / CHUNK_SIZE).floor() as i32,
    )
}
