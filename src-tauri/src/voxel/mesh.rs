use bevy::prelude::*;
use bevy::render::render_resource::{PrimitiveTopology, VertexFormat};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, MeshVertexAttribute};
use super::{Chunk, CHUNK_SIZE};

/// Our custom 32-bit packed vertex attribute.
/// We use a unique ID (987654321) to identify this in the specialization phase.
pub const ATTRIBUTE_PACKED_VOXEL: MeshVertexAttribute = MeshVertexAttribute::new(
    "PackedVoxel",
    987654321,
    VertexFormat::Uint32,
);

/// Generates a Mesh for a voxel chunk using simple face-culling.
/// Faces hidden between two solid voxels are skipped to save GPU time.
pub fn generate_chunk_mesh(chunk: &Chunk) -> Mesh {
    let mut packed_vertices: Vec<u32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let voxel = chunk.voxels[Chunk::get_index(x, y, z)];
                if voxel.id == 0 {
                    continue; // Skip empty air voxels
                }

                // Definition of the 6 cardinal directions to check for occlusion.
                let neighbors = [
                    (x as i32, y as i32 + 1, z as i32, 0), // Up
                    (x as i32, y as i32 - 1, z as i32, 1), // Down
                    (x as i32 + 1, y as i32, z as i32, 2), // East
                    (x as i32 - 1, y as i32, z as i32, 3), // West
                    (x as i32, y as i32, z as i32 + 1, 4), // North
                    (x as i32, y as i32, z as i32 - 1, 5), // South
                ];

                for (nx, ny, nz, face_idx) in neighbors {
                    // Culling logic: A face is only "exposed" (drawable) if the 
                    // neighboring voxel is air (id == 0) or is outside the chunk boundary.
                    let is_exposed = if nx < 0 || nx >= CHUNK_SIZE as i32 || 
                                       ny < 0 || ny >= CHUNK_SIZE as i32 || 
                                       nz < 0 || nz >= CHUNK_SIZE as i32 {
                        true
                    } else {
                        chunk.voxels[Chunk::get_index(nx as usize, ny as usize, nz as usize)].id == 0
                    };

                    if is_exposed {
                        add_face(&mut packed_vertices, &mut indices, x as u32, y as u32, z as u32, face_idx, voxel.id);
                    }
                }
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    // We insert our custom Uint32 attribute. 
    // Standard POSITION attributes in Bevy MUST be Float32x3, which is why we use a custom one.
    mesh.insert_attribute(ATTRIBUTE_PACKED_VOXEL, packed_vertices); 
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Adds 4 vertices and 6 indices to form one cube face.
fn add_face(positions: &mut Vec<u32>, indices: &mut Vec<u32>, x: u32, y: u32, z: u32, face: u32, material_id: u16) {
    let start_idx = positions.len() as u32;
    
    // ABSOLUTE TRUTH CCW OFFSETS (Right Hand Rule Outward Normals)
    // Vertices are ordered counter-clockwise relative to the face normal.
    // Bevy (and most GPUs) use CCW to determine which side of a triangle is the "front".
    let offsets = match face {
        0 => [[0, 1, 1], [1, 1, 1], [1, 1, 0], [0, 1, 0]], // Up (+Y)
        1 => [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]], // Down (-Y)
        2 => [[1, 0, 1], [1, 0, 0], [1, 1, 0], [1, 1, 1]], // East (+X)
        3 => [[0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]], // West (-X)
        4 => [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]], // North (+Z)
        5 => [[1, 0, 0], [0, 0, 0], [0, 1, 0], [1, 1, 0]], // South (-Z)
        _ => unreachable!(),
    };

    for [ox, oy, oz] in offsets {
        let vx = x + ox;
        let vy = y + oy;
        let vz = z + oz;
        
        // 32-BIT BITMASK PACKING
        // Bits 0-5:   X Position (0-63)
        // Bits 6-11:  Y Position (0-63)
        // Bits 12-17: Z Position (0-63)
        // Bits 18-20: Normal Index (0-7, represent 6 cardinal directions)
        // Bits 21-31: Material ID (0-2047)
        let packed = (vx & 0x3F) | 
                     ((vy & 0x3F) << 6) | 
                     ((vz & 0x3F) << 12) | 
                     ((face & 0x7) << 18) | 
                     (((material_id as u32) & 0x7FF) << 21);
        
        positions.push(packed);
    }

    // Add two triangles to form the quad
    indices.extend_from_slice(&[
        start_idx, start_idx + 1, start_idx + 2,
        start_idx, start_idx + 2, start_idx + 3,
    ]);
}
