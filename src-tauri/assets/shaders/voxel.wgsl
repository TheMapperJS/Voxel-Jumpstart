#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    forward_io::VertexOutput,
}

struct Vertex {
    // Our 4-byte packed voxel vertex
    @location(0) packed: u32,
    // Required for GPU-driven instancing in 0.18
    @builtin(instance_index) instance_index: u32,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    // --- BITWISE UNPACKING ---
    // We reverse the bit-shifting done on the CPU.
    // X, Y, Z: 6 bits each (values 0-63)
    let x = f32(vertex.packed & 0x3Fu);
    let y = f32((vertex.packed >> 6u) & 0x3Fu);
    let z = f32((vertex.packed >> 12u) & 0x3Fu);
    
    // Normal: 3 bits (0-7 indexing 6 cardinal directions)
    let normal_idx = (vertex.packed >> 18u) & 0x7u;
    
    // Material ID: 11 bits (up to 2048 materials)
    let material_id = (vertex.packed >> 21u) & 0x7FFu;

    let local_position = vec3<f32>(x, y, z);

    // Static lookup table for the 6 cardinal directions.
    let normals = array<vec3<f32>, 6>(
        vec3<f32>( 0.0,  1.0,  0.0), // 0: Up
        vec3<f32>( 0.0, -1.0,  0.0), // 1: Down
        vec3<f32>( 1.0,  0.0,  0.0), // 2: East
        vec3<f32>(-1.0,  0.0,  0.0), // 3: West
        vec3<f32>( 0.0,  0.0,  1.0), // 4: North
        vec3<f32>( 0.0,  0.0, -1.0)  // 5: South
    );
    
    // 1. Get the world-from-local matrix for this specific chunk instance.
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    
    // 2. Transform the unpacked local position to world space.
    let world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local, 
        vec4<f32>(local_position, 1.0)
    );
    
    // 3. Transform the normal using Bevy's normal transformation helper.
    let world_normal = mesh_functions::mesh_normal_local_to_world(
        normals[normal_idx],
        vertex.instance_index
    );

    // Fill the standard Bevy VertexOutput structure to maintain PBR compatibility.
    out.world_position = world_position;
    out.world_normal = world_normal;
    
    // Transform world position to clip space (screen space).
    out.position = position_world_to_clip(world_position.xyz);
    
    return out;
}
