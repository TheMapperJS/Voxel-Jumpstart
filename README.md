# Bevy 0.18 + Tauri 2.0: Infinite Voxel Engine Starter

This repository is a high-performance, open-source starter kit for building infinite-scale voxel games. It combines the bleeding-edge **Bevy 0.18** game engine with a **Tauri 2.0** + **React** frontend overlay. 

If you want to build a voxel game with massive draw distances without melting your GPU or choking your CPU, this is the blueprint.

##  Key Features

This project implements a fully custom, memory-optimized voxel rendering pipeline built natively in Bevy 0.18 (no external voxel rendering crates required).

- **Infinite 3D Geometry Clipmap:** The world is rendered using nested, exponential Levels of Detail (LOD). As you move, the rings shift continuously using toroidal addressing. This provides massive spatial coverage with a strictly bounded memory footprint.
- **Extreme Vertex Compression (32-bit Packed Vertices):** Instead of standard 32-byte PBR vertices, this engine packs the position, normal, and material ID of every voxel vertex into a *single* `u32` (4 bytes). This reduces VRAM bandwidth by 800% and makes rendering tens of thousands of chunks possible.
- **PBR-Integrated `ExtendedMaterial`:** The custom WGSL shader (`voxel.wgsl`) unpacks the 32-bit vertices directly on the GPU while maintaining 100% compatibility with Bevy's native lighting, shadows, and Physically Based Rendering (PBR) pipelines.
- **Async Procedural Generation:** Terrain generation (using Perlin noise) and greedy/culled meshing happen asynchronously in the background using Bevy's `AsyncComputeTaskPool`. Your framerate stays perfectly smooth no matter how fast you fly.
- **Tauri 2.0 Input Bridge:** Because Bevy is embedded directly into a Tauri window (bypassing `WinitPlugin`), all input (keyboard, mouse motion, clicking) is captured in React and piped seamlessly into Bevy's internal `MessageReader` systems.

## Controls

Once the engine is running:
- **Left Click:** Lock and hide the cursor (enter First-Person mode).
- **W, A, S, D:** Move around the terrain.
- **Mouse:** Look around.
- **Space:** Jump.
- **Escape:** Unlock the cursor and return control to the UI.

## Getting Started

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Node.js](https://nodejs.org/) (v18+)
- OS-specific Tauri dependencies (C++ Build Tools, WebKit, etc.)

### Installation & Running

1. Install frontend dependencies:
   ```bash
   npm install
   ```

2. Run the application in development mode:
   ```bash
   npm run tauri dev
   ```

## Architecture & Deep Dives

This project was built with a specific architecture to solve the "Voxel Draw Distance Wall". For a deep dive into the math and reasoning behind the clipmap and vertex packing, read the included design document:

**[Read the Voxel Architecture Blueprint (DESIGN.md)](./DESIGN.md)**

### Bevy 0.18 Migration Notes
Building a custom renderer in Bevy 0.18 requires navigating some major API shifts (e.g., the introduction of `Single` system parameters, changes to `MeshVertexBufferLayoutRef`, and the shift from `EventWriter` to `MessageWriter` for input). 

If you are upgrading from Bevy 0.14 or earlier, check out the included **[bevy_018.md](./bevy_018.md)** for a cheat sheet of API changes discovered while building this engine.

## Project Structure

- `src/`: The React frontend. Handles the UI, Start Menu, and captures input events to send to the Rust backend.
- `src-tauri/src/lib.rs`: The Tauri + Bevy integration layer. Initializes the manual Bevy loop, sets up the window handles, and pipes input from React to Bevy.
- `src-tauri/src/voxel/`: The core voxel engine.
  - `clipmap.rs`: Manages the infinite LOD rings and chunk lifecycle.
  - `world.rs`: Asynchronous terrain generation and meshing tasks.
  - `mesh.rs`: Converts voxel data into 32-bit packed geometry.
  - `material.rs`: The Bevy `MaterialExtension` that hooks our custom shader into the PBR pipeline.
  - `camera.rs`: The First-Person player controller and physics.
- `src-tauri/assets/shaders/voxel.wgsl`: The GPU vertex shader that unpacks the compressed voxel data on the fly.

## Contributing

Contributions are welcome! If you want to expand this starter kit, some great next steps would be:
- Implementing true "Greedy Meshing" (currently it uses simple face-culling).
- Adding Solari Raytracing integration.
- Implementing cross-LOD skirts to perfectly hide the seams between clipmap rings.
- Adding block breaking and placing.

---
*Built to jumpstart the next generation of Rust voxel games.*
