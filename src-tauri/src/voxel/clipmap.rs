use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use super::Chunk;

/// Configuration for the geometry clipmap.
#[derive(Resource)]
pub struct ClipmapConfig {
    pub levels: u32,     // Number of LOD rings (e.g., 4 rings)
    pub ring_size: i32,  // Width of each ring in chunks (e.g., 8x8)
}

impl Default for ClipmapConfig {
    fn default() -> Self {
        Self {
            levels: 4,
            ring_size: 8,
        }
    }
}

/// Stores the entities belonging to each LOD level of the clipmap.
#[derive(Resource, Default)]
pub struct Clipmap {
    // level -> (chunk_pos -> entity)
    pub chunks: HashMap<u32, HashMap<IVec3, Entity>>,
}

/// The core clipmap system. It shifts concentric rings around the camera and
/// performs "hole carving" to ensure LOD levels don't overlap.
pub fn update_clipmap(
    mut commands: Commands,
    mut clipmap: ResMut<Clipmap>,
    config: Res<ClipmapConfig>,
    camera: Single<&Transform, With<Camera3d>>,
) {
    let camera_pos = camera.translation;
    
    for level in 0..config.levels {
        // Each LOD level doubles the size of its voxels.
        let voxel_size = 2.0f32.powi(level as i32);
        let chunk_world_size = voxel_size * super::CHUNK_SIZE as f32;
        
        // Calculate which chunk the camera is currently inside at this LOD.
        // .floor() is required to handle negative world coordinates correctly.
        let center_chunk = (camera_pos / chunk_world_size).floor().as_ivec3();
        
        let level_chunks = clipmap.chunks.entry(level).or_default();
        let half_size = config.ring_size / 2;
        
        // HOLE CARVING LOGIC
        // We calculate the world-space bounds of the PREVIOUS (higher detail) level.
        // Lower detail rings should skip any area already covered by a higher detail ring.
        let prev_voxel_size = if level > 0 { 2.0f32.powi(level as i32 - 1) } else { 0.0 };
        let prev_chunk_world_size = prev_voxel_size * super::CHUNK_SIZE as f32;
        let prev_center_chunk = if level > 0 { (camera_pos / prev_chunk_world_size).floor().as_ivec3() } else { IVec3::ZERO };
        
        let prev_min_world = (prev_center_chunk - IVec3::splat(half_size)).as_vec3() * prev_chunk_world_size;
        let prev_max_world = (prev_center_chunk + IVec3::splat(half_size)).as_vec3() * prev_chunk_world_size;

        let mut valid_chunks = HashSet::new();

        for x in -half_size..half_size {
            // Optimization: Only generate a thin band around the horizon (Y coordinates).
            for y in -2..2 {
                for z in -half_size..half_size {
                    let chunk_pos = center_chunk + IVec3::new(x, y, z);
                    
                    let mut is_inside_prev = false;
                    if level > 0 {
                        let c_min = chunk_pos.as_vec3() * chunk_world_size;
                        let c_max = c_min + Vec3::splat(chunk_world_size);
                        
                        // Check if this lower-LOD chunk is completely eclipsed by the previous ring.
                        is_inside_prev = c_min.x >= prev_min_world.x - 0.1 && c_max.x <= prev_max_world.x + 0.1 &&
                                         c_min.y >= prev_min_world.y - 0.1 && c_max.y <= prev_max_world.y + 0.1 &&
                                         c_min.z >= prev_min_world.z - 0.1 && c_max.z <= prev_max_world.z + 0.1;
                    }
                    
                    if !is_inside_prev {
                        valid_chunks.insert(chunk_pos);
                    }
                }
            }
        }
        
        // Spawn missing chunks
        for &chunk_pos in &valid_chunks {
            if !level_chunks.contains_key(&chunk_pos) {
                // SEAM PREVENTION:
                // Shift lower LODs slightly down (0.5 * level) to prevent flickering (z-fighting)
                // where rings meet. High-detail geometry will sit "on top" of lower detail.
                let y_shift = Vec3::Y * (level as f32 * 0.5);
                
                let entity = commands.spawn((
                    Chunk::new(chunk_pos, level),
                    Transform::from_translation(chunk_pos.as_vec3() * chunk_world_size - y_shift)
                        .with_scale(Vec3::splat(voxel_size)),
                    Visibility::Visible,
                )).id();
                
                level_chunks.insert(chunk_pos, entity);
            }
        }
        
        // Culling: Despawn chunks that have fallen out of the current ring or moved into a higher LOD hole.
        level_chunks.retain(|&pos, &mut entity| {
            if valid_chunks.contains(&pos) {
                true
            } else {
                commands.entity(entity).despawn();
                false
            }
        });
    }
}
