use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::NoAutoAabb;
use futures_lite::future;
use super::{Chunk, Voxel, mesh, VoxelMaterial, CHUNK_SIZE};
use noise::{NoiseFn, Perlin};

/// Component attached to a chunk entity while it is generating in the background.
#[derive(Component)]
pub struct ChunkTask(pub Task<(Chunk, Mesh)>);

/// Stores assets that all voxel chunks share, like the PBR material.
#[derive(Resource)]
pub struct VoxelAssets {
    pub material: Handle<VoxelMaterial>,
}

/// System that detects new Chunk entities and spawns background tasks to generate them.
pub fn spawn_chunk_tasks(
    mut commands: Commands,
    query: Query<(Entity, &Chunk), (Without<ChunkTask>, Without<Mesh3d>)>,
) {
    let thread_pool = AsyncComputeTaskPool::get();
    let perlin = Perlin::new(12345); // Seed should ideally be in a Resource

    for (entity, chunk_data) in &query {
        let chunk_pos = chunk_data.position;
        let chunk_level = chunk_data.level;
        let perlin = perlin.clone();
        
        // DISPATCH GENERATION TO BACKGROUND THREAD
        let task = thread_pool.spawn(async move {
            let mut chunk = Chunk::new(chunk_pos, chunk_level);
            
            // Critical for LOD: The voxel size doubles every level.
            let voxel_size = 2.0f64.powi(chunk_level as i32);
            let chunk_world_size = voxel_size * CHUNK_SIZE as f64;
            
            for x in 0..CHUNK_SIZE {
                for y in 0..CHUNK_SIZE {
                    for z in 0..CHUNK_SIZE {
                        // We must scale the noise sampling based on voxel_size 
                        // to ensure the terrain shapes match across all LOD rings.
                        let world_x = chunk_pos.x as f64 * chunk_world_size + (x as f64 * voxel_size);
                        let world_y = chunk_pos.y as f64 * chunk_world_size + (y as f64 * voxel_size);
                        let world_z = chunk_pos.z as f64 * chunk_world_size + (z as f64 * voxel_size);
                        
                        // Sample Perlin noise for basic terrain height.
                        let noise_val = perlin.get([world_x * 0.01, world_z * 0.01]);
                        let height = (noise_val * 20.0 + 10.0) as f64;
                        
                        if world_y < height {
                            chunk.voxels[Chunk::get_index(x, y, z)] = Voxel { id: 1 };
                        }
                    }
                }
            }

            // Generate the optimized packed mesh on the background thread.
            let mesh = mesh::generate_chunk_mesh(&chunk);
            (chunk, mesh)
        });

        commands.entity(entity).insert(ChunkTask(task));
    }
}

/// System that polls background tasks and applies the finished Meshes to the entities.
pub fn handle_chunk_tasks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<(Entity, &mut ChunkTask)>,
    voxel_assets: Res<VoxelAssets>,
) {
    for (entity, mut task) in &mut query {
        // poll_once allows us to check for completion without blocking the main loop.
        if let Some((_chunk, mesh)) = future::block_on(future::poll_once(&mut task.0)) {
            
            // MANUAL AABB
            // Because our vertices are 32-bit packed integers, Bevy cannot automatically 
            // calculate the bounding box for frustum culling. We provide it manually.
            let aabb = Aabb::from_min_max(Vec3::ZERO, Vec3::splat(CHUNK_SIZE as f32));

            commands.entity(entity)
                .insert(Mesh3d(meshes.add(mesh)))
                .insert(MeshMaterial3d(voxel_assets.material.clone()))
                .insert(aabb)
                .insert(NoAutoAabb) // Prevents Bevy from overwriting our manual Aabb.
                .remove::<ChunkTask>();
        }
    }
}
