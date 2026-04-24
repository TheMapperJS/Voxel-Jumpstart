pub mod material;
pub mod mesh;
pub mod world;
pub mod clipmap;
pub mod camera;

use bevy::prelude::*;
use bevy::pbr::{ExtendedMaterial, MaterialPlugin};
use material::VoxelMaterialExtension;
use world::{spawn_chunk_tasks, handle_chunk_tasks};
use clipmap::{update_clipmap, Clipmap, ClipmapConfig};
use camera::move_player;

pub type VoxelMaterial = ExtendedMaterial<StandardMaterial, VoxelMaterialExtension>;

pub struct VoxelPlugin;

impl Plugin for VoxelPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<VoxelMaterial>::default())
           .init_resource::<Clipmap>()
           .init_resource::<ClipmapConfig>()
           .add_systems(Update, (update_clipmap, spawn_chunk_tasks, handle_chunk_tasks, move_player).chain());
    }
}


/// Represents a single voxel type/material.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct Voxel {
    pub id: u16,
}

/// A chunk of voxels.
pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

#[derive(Component)]
pub struct Chunk {
    pub position: IVec3,
    pub level: u32,
    pub voxels: Box<[Voxel; CHUNK_VOLUME]>,
}

impl Chunk {
    pub fn new(position: IVec3, level: u32) -> Self {
        Self {
            position,
            level,
            voxels: Box::new([Voxel::default(); CHUNK_VOLUME]),
        }
    }

    pub fn get_index(x: usize, y: usize, z: usize) -> usize {
        x + y * CHUNK_SIZE + z * CHUNK_SIZE * CHUNK_SIZE
    }
}
