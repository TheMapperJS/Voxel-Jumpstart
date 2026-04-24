## Events and Messages
- **Events**: Now primarily push-based, handled via `app.observe(...)` or `world.observe(...)`.
- **Messages**: The new pull-based alternative to the old `EventReader`. Use `MessageReader<T>` to read buffered messages in a system. `T` must implement the `Message` trait.
- **Input**: Many input events (like Gamepad events) have moved to the Message system. Check if `MouseMotion` is now a Message.
- **Query::get_single()**: Removed. Use `Query::single()` which returns `Result<T, QuerySingleError>`.
- **Single system parameter**: Use `Single<&Transform, With<Camera3d>>` for a more ergonomic way to access a single entity's components.
- **despawn_recursive()**: Use `despawn()` on `EntityCommands` as it might be recursive by default in 0.18, or check `DespawnRecursiveExt`.

## Core Mesh and Indices
- **Mesh**: `bevy::prelude::Mesh`
- **Indices**: `bevy::mesh::Indices`
- **PrimitiveTopology**: `bevy::render::render_resource::PrimitiveTopology` (also in `bevy::prelude`).

## Material and Shaders
- **ShaderRef**: `bevy::shader::ShaderRef`
- **MaterialExtension**: `bevy::pbr::MaterialExtension`
- **ExtendedMaterial**: `bevy::pbr::ExtendedMaterial`
- **MaterialPlugin**: `bevy::pbr::MaterialPlugin`

## Vertex Layouts
- **MeshVertexBufferLayoutRef**: `bevy::mesh::MeshVertexBufferLayoutRef`
- **VertexBufferLayout**: `bevy::mesh::VertexBufferLayout`
- **VertexAttribute**, **VertexFormat**, **VertexStepMode**: `bevy::render::render_resource::*`

## Asset Usages
- **RenderAssetUsages**: `bevy::asset::RenderAssetUsages`

## Notes
- Bevy 0.18 has moved many types out of the top-level `bevy::render` module and into more specialized crates like `bevy_mesh`, `bevy_shader`, etc., which are then re-exported in `bevy::mesh`, `bevy::shader`, etc.
- If a type is "private" in a re-export, check the most direct public module or `bevy::prelude`.
