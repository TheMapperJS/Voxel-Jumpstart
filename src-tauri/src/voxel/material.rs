use bevy::prelude::*;
use bevy::pbr::{MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline};
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use bevy::mesh::MeshVertexBufferLayoutRef;

/// A marker struct for our voxel material extension.
/// It uses the AsBindGroup derive to handle GPU uniform/texture bindings if needed.
#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct VoxelMaterialExtension {}

impl MaterialExtension for VoxelMaterialExtension {
    /// Points to the custom WGSL shader that performs vertex unpacking.
    fn vertex_shader() -> ShaderRef {
        "shaders/voxel.wgsl".into()
    }

    /// We must also override these hooks so that depth/deferred passes use our unpacking logic.
    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/voxel.wgsl".into()
    }

    fn deferred_vertex_shader() -> ShaderRef {
        "shaders/voxel.wgsl".into()
    }

    /// SPECIALIZATION MAGIC
    /// This is where we tell Bevy "Don't expect the standard Float32x3 POSITION attribute".
    /// Instead, we request our custom Uint32 packed attribute.
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // We reference our custom attribute defined in mesh.rs
        use super::mesh::ATTRIBUTE_PACKED_VOXEL;
        
        // get_layout calculates the correct buffer stride and offsets for our Uint32.
        let vertex_layout = layout.0.get_layout(&[
            ATTRIBUTE_PACKED_VOXEL.at_shader_location(0),
        ]).map_err(|e| SpecializedMeshPipelineError::MissingVertexAttribute(e))?;

        // Replace the default vertex buffer configuration with our optimized one.
        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())
    }
}
