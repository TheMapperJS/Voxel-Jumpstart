# **Architecting Infinite-Scale Voxel Renderers in Bevy 0.18: A GPU-Driven Clipmap Approach**

## **Introduction**

The pursuit of vast, unconstrained draw distances in volumetric and voxel-based environments represents one of the most formidable challenges in real-time computer graphics. Traditional polygon-based rendering methodologies rely on sparse geometric distribution, where environments are composed of relatively few, highly detailed models. In contrast, voxel environments mandate the generation, management, and rendering of billions of discrete cubic elements. Pushing the horizon to the mathematical limits of the projection matrix—achieving what is colloquially termed "insane" draw distances—routinely exhausts computational resources through a combination of memory bandwidth saturation, PCIe bus congestion, and draw-call overhead.1  
Historically, achieving these distances required the integration of highly specialized external libraries or the development of completely custom rendering engines, as general-purpose engines lacked the low-level API access necessary for such aggressive optimization. However, the release of Bevy 0.18 introduces a paradigm shift. The engine's internal architecture has matured significantly, introducing high-performance, low-level graphics primitives natively.4 Features such as multi-draw indirect (MDI) capabilities via BinnedRenderPhaseType::MultidrawableMesh, robust compute shader integrations, asynchronous task pooling, and the ExtendedMaterial API provide the requisite foundation for a fully GPU-driven pipeline.5  
This comprehensive report details the architectural blueprint required to construct a voxel renderer capable of infinite-scale draw distances strictly within the native Bevy 0.18 ecosystem, without relying on any external voxel crates, and crucially, without catastrophic hardware degradation. By synthesizing CPU-side multithreaded chunk generation with GPU-based geometry clipmaps, aggressive 32-bit vertex compression, and compute-driven frustum culling, an application can render tens of billions of potential voxels at interactive framerates. Furthermore, this architecture ensures full compatibility with Bevy's state-of-the-art environmental systems, including Solari raytracing, Atmosphere Occlusion, and Volumetric Fog, demonstrating that monumental scale does not necessitate the sacrifice of visual fidelity.4

## **Architectural Evolution and Bevy 0.18 Integration**

The transition from Bevy 0.17 to 0.18 brings several architectural mandates that fundamentally alter how custom renderers are constructed. Before delving into spatial data structures, the foundational configuration of the engine must be addressed to ensure maximum performance and compatibility with the underlying wgpu backend.11  
A defining feature of Bevy 0.18 is the introduction of high-level scenario-driven Cargo feature collections.4 For a dedicated voxel renderer, extraneous engine subsystems must be culled at compile time to optimize binary size and compilation speed. Developers can now utilize the 3d\_bevy\_render and mesh collections, allowing the application to explicitly opt out of 2D or UI rendering pipelines if they are unnecessary for the immediate simulation.13  
Furthermore, the migration to Bevy 0.18 necessitates specific adaptations within the render graph. Pipeline descriptors, specifically RenderPipelineDescriptor and ComputePipelineDescriptor, no longer retain a BindGroupLayout directly.14 Instead, the architecture utilizes a BindGroupLayoutDescriptor, containing BindGroupLayoutEntry elements, to dynamically request layouts from the PipelineCache.14 This caching mechanism drastically reduces the overhead of pipeline specialization, which is a critical factor when managing the numerous specialized pipelines required for chunked voxel rendering. Additionally, the automatic creation and management of the Aabb (Axis-Aligned Bounding Box) component for entities containing meshes simplifies the preliminary CPU-side culling phases before data is dispatched to the GPU.14

## **Spatial Data Structures: The Voxel Clipmap**

To render environments stretching thousands of units in every direction, the spatial data structure must conceptually decouple the world's absolute theoretical size from the memory footprint required to maintain it in Random Access Memory (RAM) and Video RAM (VRAM).15  
The industry standard for dense volumetric data has historically oscillated between simple 3D arrays, HashMaps, and Sparse Voxel Octrees (SVOs).16 While 3D arrays offer $O(1)$ random access, their memory requirements scale cubically, making them immediately untenable for large distances due to the sheer volume of empty space they must explicitly store.17 Sparse Voxel Octrees and their more advanced derivative, Sparse Voxel Directed Acyclic Graphs (SVO-DAGs), solve the sparsity problem and provide implicit level-of-detail (LOD).2 However, SVOs incur significant pointer-chasing latency during traversal. The complex hierarchical pointer structures cause frequent cache misses on the GPU, bottlenecking the memory controllers and preventing maximum throughput.2  
The most performant approach for rendering massive, continuous terrain without external dependencies is the 3D Geometry Clipmap.3 The clipmap architecture abandons hierarchical trees in favor of nested, flat arrays.

### **Theoretical Foundation of Geometry Clipmaps**

Originally formulated by Hugues Hoppe for 2D terrain heightmaps, the geometry clipmap caches geometry in a set of nested, regular grids centered around the viewer.3 Adapted for 3D volumetric data, a voxel clipmap consists of concentric, cubic volumetric boundaries, termed "rings" or "levels of detail" (LODs).20  
Let $L$ denote the total number of clipmap levels, where $l \\in \[0, L-1\]$. Each level $l$ maintains a uniform grid of voxel chunks with a constant dimensional extent, such as a grid of $N \\times N \\times N$ chunks.21 The critical insight is that the physical dimension of the voxels within each chunk doubles at each successive level. If a voxel at level 0 represents a $1 \\times 1 \\times 1$ meter volume, a voxel at level 1 represents a $2 \\times 2 \\times 2$ meter volume, and a voxel at level $l$ represents a $2^l \\times 2^l \\times 2^l$ meter volume. Consequently, the spatial coverage of level $l$ scales exponentially by $2^l$, while the number of geometric primitives strictly remains constant.  
This nested configuration guarantees that the highest resolution geometry is exclusively generated and rendered immediately adjacent to the camera, while exponentially coarser geometry populates the distant horizon. Because the number of chunks remains constant per level, the memory and processing constraints are mathematically bounded, unconditionally preventing system resource exhaustion regardless of the theoretical draw distance.3

### **Ring-Shifting and Toroidal Addressing**

As the viewer translates through the environment, the clipmap must continuously update to maintain its viewer-centric alignment.9 Naively destroying and recreating chunk entities at the boundaries of the clipmap rings introduces catastrophic memory allocation overhead, ECS thrashing, and pipeline stalling.  
To resolve this within Bevy's Entity Component System (ECS), the architecture must implement a toroidal (wrap-around) addressing scheme for the chunk entities.22 The underlying memory array or ECS storage for a given clipmap level operates as a 3D ring buffer. When the camera crosses a predetermined chunk boundary threshold, the logical center of the clipmap shifts. The chunks that fall outside the trailing edge of the new boundary are not despawned; instead, their spatial indices are logically wrapped around to the leading edge of the movement vector using a modulo arithmetic operation.  
The calculation for the updated logical index within the toroidal array can be expressed as:

$$Index\_{new} \= (Index\_{old} \+ \\Delta Position) \\pmod{N}$$  
Once the logical index is shifted, the visual representation of that entity is invalidated, and an asynchronous generation request is dispatched to populate the chunk with the new boundary geometry.23 This shifting logic minimizes entity fragmentation in the Bevy ECS, keeping the structural footprint of the world entirely static in memory while the procedural data flowing through it remains dynamically localized to the viewer.22

### **Comparative Spatial Data Structure Analysis**

The superiority of the voxel clipmap over alternative structures for this specific use case becomes evident when comparing their operational profiles under extreme load.

| Data Structure | Access Time | Worst-Case Overhead | Suitability for Infinite Draw Distances | GPU Memory Access Pattern |
| :---- | :---- | :---- | :---- | :---- |
| **3D Array (Dense)** | $O(1)$ | $O(\\infty)$ | Entirely unsuitable; memory scales cubically with distance regardless of sparsity.17 | Perfectly contiguous, optimal cache utilization. |
| **HashMap (Sparse)** | $O(1)$ amortized | Hash Collisions | Acceptable for persistent storage, but introduces high overhead for dynamic GPU streaming.16 | Fragmented; poor spatial locality on GPU. |
| **SVO / DAG** | $O(\\log n)$ | High Pointer Overhead | Excellent memory compression, but traversal latency heavily throttles rendering performance.2 | High rate of cache misses due to pointer chasing. |
| **3D Clipmap** | $O(1)$ per ring | Redundant overlap | Optimal. Strictly bounded memory overhead while providing exponential distance coverage.3 | Contiguous per chunk; predictable linear reads. |

By utilizing the 3D Clipmap, the rendering architecture fundamentally guarantees that "insane" draw distances are merely a function of modifying a scaling multiplier, rather than a hardware-breaking accumulation of raw data.

## **Memory Bandwidth and Pipeline Data Compression**

The raw data of a voxel world—even when strictly constrained by a mathematical clipmap—translates to millions of potential triangles per frame. The primary bottleneck in rendering such massive geometry is almost never the Arithmetic Logic Unit (ALU) capacity of the modern GPU, but rather the memory bandwidth required to ferry vertex data from VRAM to the shader execution units.25

### **The Fallacy of Standard Vertex Layouts**

A standard Bevy Mesh utilizing the default StandardMaterial pipeline expects a comprehensive, uncompressed vertex layout.26 Typically, a standard 3D graphics vertex comprises several high-precision vectors:

* Position: vec3\<f32\> (12 bytes)  
* Normal: vec3\<f32\> (12 bytes)  
* UV Coordinates: vec2\<f32\> (8 bytes)

This equates to 32 bytes per vertex. For a chunked voxel engine utilizing greedy meshing algorithms, a single $32 \\times 32 \\times 32$ chunk might easily contain 10,000 vertices.27 Across an array of 10,000 chunks necessary for a vast draw distance, this results in over three gigabytes of raw vertex data being pushed across the PCIe bus continuously. Attempting to render this via standard methods will effectively halt the rendering pipeline, violating the core constraint of the user's objective.27

### **Extreme Compression: 32-Bit Packed Vertices**

Because voxel geometry is rigidly aligned to an integer grid, floating-point precision is entirely superfluous during the storage and transmission phases.28 The spatial coordinates, normal directions, and material identifiers can be aggressively quantized into a single 32-bit unsigned integer (u32). This technique reduces the memory footprint of a vertex by an extraordinary 800% (from 32 bytes to exactly 4 bytes).17  
A highly optimized 32-bit bitmask for a chunk-local voxel vertex can be structured to contain all necessary rendering parameters within the 32 available bits.

| Attribute | Bits Allocated | Bit Range | Description |
| :---- | :---- | :---- | :---- |
| **X Position** | 6 bits | 0 \- 5 | The X coordinate within a $32 \\times 32 \\times 32$ chunk. The 6th bit allows values up to 63, providing a margin for LOD skirts. |
| **Y Position** | 6 bits | 6 \- 11 | The Y coordinate within the chunk limits. |
| **Z Position** | 6 bits | 12 \- 17 | The Z coordinate within the chunk limits. |
| **Normal** | 3 bits | 18 \- 20 | Represents the 6 possible faces of a perfect cube (Up, Down, North, South, East, West). |
| **Material ID** | 11 bits | 21 \- 31 | A lookup index mapping to an array of texture coordinates or predefined material properties (supports 2,048 distinct materials). |

In Rust, generating the mesh buffer relies on bitwise shifting to pack the array during the greedy meshing phase. The CPU performs this quantization once per chunk update:

Rust

let packed\_vertex: u32 \=   
    (x & 0x3F) |   
    ((y & 0x3F) \<\< 6\) |   
    ((z & 0x3F) \<\< 12\) |   
    ((normal & 0x7) \<\< 18\) |   
    ((material\_id & 0x7FF) \<\< 21);

### **WGSL Memory Alignment and Unpacking**

Bevy's WebGPU Shading Language (WGSL) shaders must be intricately customized to reverse this quantization on the GPU, unpacking the data directly within the vertex shader.7 When laying out structs in WGSL, the compiler enforces strict memory alignment rules.30  
The alignment for a given struct is determined by the equation: AlignOf(S) \= max(AlignOfMember(S, M1),...).30 A common pitfall is utilizing vec3\<f32\> in a storage buffer, which forces a 16-byte alignment, wasting 4 bytes of padding per vector.30 By strictly transmitting an array of u32 scalar values, the alignment is natively 4 bytes, ensuring perfect, unpadded cache-line utilization when the GPU reads the storage buffers.30  
The WGSL vertex shader accepts the single u32 and reconstructs the necessary floating-point data utilizing bitwise masking and shifting operations.32 The ALU operations required for this unpacking are essentially instantaneous compared to the immense latency of memory fetches.

Kodavsnitt

fn unpack\_voxel(packed: u32) \-\> VertexOutput {  
    // Extract grid coordinates utilizing bitwise AND and right-shifts  
    let x \= f32(packed & 0x3Fu);  
    let y \= f32((packed \>\> 6u) & 0x3Fu);  
    let z \= f32((packed \>\> 12u) & 0x3Fu);  
      
    // Extract metadata  
    let normal\_idx \= (packed \>\> 18u) & 0x7u;  
    let material\_id \= (packed \>\> 21u) & 0x7FFu;  
      
    // Position reconstructed relative to chunk space  
    let local\_pos \= vec3\<f32\>(x, y, z);  
      
    // Normal reconstruction via a static lookup table  
    let normals \= array\<vec3\<f32\>, 6\>(  
        vec3\<f32\>( 0.0,  1.0,  0.0), // Up  
        vec3\<f32\>( 0.0, \-1.0,  0.0), // Down  
        vec3\<f32\>( 1.0,  0.0,  0.0), // East  
        vec3\<f32\>(-1.0,  0.0,  0.0), // West  
        vec3\<f32\>( 0.0,  0.0,  1.0), // North  
        vec3\<f32\>( 0.0,  0.0, \-1.0)  // South  
    );  
    let normal \= normals\[normal\_idx\];  
      
    // The unpacked data is subsequently transformed by the view-projection matrix  
}

By transitioning the entire rendering architecture to this 32-bit packed format, the memory required to store the vast geometry of the horizon is thoroughly trivialized.15 This compression serves as the foundational enabler for insane draw distances, leaving ample VRAM bandwidth available for high-resolution textures, complex PBR evaluation, and the raytraced irradiance caches utilized by Bevy's lighting systems.

## **Multithreaded Generation and ECS Task Pooling**

While the GPU shoulders the burden of rendering, the CPU remains strictly responsible for the procedural generation of terrain and the execution of the greedy meshing algorithms. Terrain generation involves computationally expensive multi-octave noise evaluations (e.g., Perlin or Simplex noise) and topological analysis to determine exposed faces. Attempting to execute these calculations synchronously on the main thread will induce severe frame stuttering, destroying the interactive fluidity of the application.24  
The architecture must orchestrate voxel data by strictly segregating domain logic from the render graph.36 Bevy 0.18 provides the AsyncComputeTaskPool, specifically designed to facilitate CPU-intensive work that may span across multiple frames without halting the primary simulation loop.23

### **The Asynchronous Lifecycle of a Voxel Chunk**

The generation and lifecycle management of a chunk relies on a state machine executed within the ECS.

1. **Request Generation:** When the clipmap ring-shifting logic detects a newly required chunk index, an entity is spawned containing the chunk's spatial coordinates and a ChunkLoading marker component.  
2. **Task Dispatch:** A Bevy system queries for all entities possessing the ChunkLoading marker. For each entity, it utilizes the AsyncComputeTaskPool to spawn a non-blocking future.23 This future encapsulates the entire generation and meshing workload.  
3. **Procedural Evaluation:** Within the spawned thread, the noise functions dictate the presence or absence of voxels. Following this, a greedy meshing algorithm iterates over the chunk. Greedy meshing coalesces coplanar faces of identical materials into large rectangular quads, drastically reducing the final vertex count compared to naive meshing.  
4. **Task Polling:** The Task object returned by the thread pool is attached to the entity as a component. A separate synchronization system routinely polls these tasks using future::block\_on(future::poll\_once(\&mut task)) to determine if the background generation is complete.  
5. **Buffer Ingestion:** Once the packed u32 mesh data is returned, the task is consumed. The data is uploaded to the GPU via RenderAssets and allocated into a unified MeshAllocator slab.14 The chunk entity is subsequently flagged as Ready, making its mathematical boundaries available to the GPU-driven culling pipelines.

By utilizing crossbeam channels or asynchronous task polling, the application ensures that regardless of how rapidly the player moves or how far the draw distance extends, the game logic, physics (e.g., Rapier3D), and UI operate at a perfectly stable tick rate, fully utilizing multi-core processor architectures.23

## **GPU-Driven Rendering: Multi-Draw Indirect and Binned Phases**

Even with highly compressed geometry and asynchronous generation, submitting tens of thousands of individual chunk draw calls to the GPU via the CPU creates an insurmountable driver overhead.25 In a traditional rendering loop, the CPU must issue a distinct draw\_indexed command for every single mesh, stalling the pipeline as the driver validates and translates each command for the graphics hardware.39  
Bevy 0.18 radically mitigates this limitation by stabilizing Vulkan and DX12 Multi-Draw Indirect (MDI) capabilities, exposed through the BinnedRenderPhaseType::MultidrawableMesh variant.6 A fully GPU-driven pipeline operates on the principle that the CPU should abdicate the responsibility of determining what is drawn. Instead, the CPU merely uploads a unified buffer of all potential chunk data to the GPU, alongside a buffer of drawing commands, allowing the GPU to cull and draw itself autonomously.

### **Implementing Binned Phases in Bevy 0.18**

To exploit MDI, the application must bypass the standard iterative entity drawing mechanisms.6 Bevy's SpecializedMeshPipelines must be utilized in conjunction with a custom queuing system inside the RenderSystems::Queue schedule.6  
All chunk vertices across the entire clipmap are allocated into a massive, contiguous MeshAllocator slab.14 Instead of representing distinct mesh objects, chunks represent contiguous ranges within this monolithic buffer. Chunks are added to an opaque\_phase utilizing BinnedRenderPhaseType::mesh(...), which verifies if MDI is supported by the underlying device hardware.6  
The binning system groups chunks by material and pipeline state, utilizing an Opaque3dBatchSetKey and Opaque3dBinKey. This consolidates the thousands of potential chunks into a singular indirect draw call dispatched to the GPU.6

### **Compute Shader Frustum Culling**

To prevent the GPU from evaluating the billions of vertices located behind the camera or outside the viewing frustum, a compute shader performs aggressive culling. Prior to the main render pass, Bevy dispatches a compute pipeline structured around GPU workgroups.42  
The compute shader operates on a Storage Buffer containing the InstanceData for every loaded chunk.45 Each compute thread is assigned a specific chunk, evaluating its Axis-Aligned Bounding Box (AABB) against the six planes of the camera's view-projection matrix frustum.  
If the chunk is mathematically visible, the thread utilizes an atomic operation (atomicAdd) to increment a global instance counter and write the chunk's draw arguments into a final output buffer.45 This output buffer is structured precisely to match the IndirectParametersIndexed specification required by the graphics API.46

| IndirectParametersIndexed Field | Type | Functionality in Voxel Rendering |
| :---- | :---- | :---- |
| index\_count | u32 | The precise number of indices in the specific chunk, derived during greedy meshing.46 |
| instance\_count | u32 | Dynamically set to 1 by the compute shader if the chunk passes the frustum cull, or remains 0 if culled.46 |
| first\_index | u32 | The offset of the chunk's first index within the monolithic index buffer slab.46 |
| base\_vertex | u32 | The offset of the chunk's first vertex within the monolithic vertex buffer slab.46 |
| first\_instance | u32 | The offset for the specific instance ID used for per-chunk data retrieval.46 |

When Bevy executes the TrackedRenderPass::multi\_draw\_indirect command 41, the GPU driver reads this IndirectParametersIndexed buffer directly from VRAM. If a chunk was culled, its instance\_count is 0, and the GPU hardware instantly skips it.  
The CPU remains completely unaware of how many chunks were drawn or culled. This zero-overhead submission loop is the absolute crux of achieving massive draw distances; it drops the CPU cost of drawing $100,000$ chunks from tens of milliseconds to effectively zero, shifting the entire workload to the massively parallel architecture of the GPU.25

## **Preserving Visual Fidelity: Material Extensions and PBR**

A prevalent pitfall when engineering custom voxel shaders is the inadvertent destruction of the engine's standard rendering features. Custom implementations frequently bypass the core rendering pipeline, resulting in visually flat, unlit geometry devoid of shadows or physical material properties.47 Generating insane draw distances is counterproductive if the visual result resembles primitive, unshaded software rendering.  
Bevy 0.18 elegantly solves this through the stabilization of the ExtendedMaterial and MaterialExtension traits.7 These APIs empower developers to inject custom vertex layouts and unpacking logic while entirely preserving the engine's built-in Physically Based Rendering (PBR) pipeline.7

### **Integrating the Packed Vertex Format via ExtendedMaterial**

To utilize the 32-bit packed vertices within Bevy's PBR system, a struct is defined to act as the extension identifier, building upon the base StandardMaterial.7

Rust

\#  
pub struct VoxelMaterialExtension {}

impl MaterialExtension for VoxelMaterialExtension {  
    fn vertex\_shader() \-\> ShaderRef {  
        "shaders/voxel\_clipmap.wgsl".into()  
    }  
      
    fn specialize(  
        \_pipeline: \&MaterialExtensionPipeline,  
        descriptor: \&mut RenderPipelineDescriptor,  
        layout: \&MeshVertexBufferLayoutRef,  
        \_key: MaterialExtensionKey\<Self\>,  
    ) \-\> Result\<(), SpecializedMeshPipelineError\> {  
        // Override the descriptor to expect a single u32 per vertex  
        // bypassing the standard 32-byte PBR layout.  
        let vertex\_layout \= VertexBufferLayout {  
            array\_stride: 4,  
            step\_mode: VertexStepMode::Vertex,  
            attributes: vec\!\[VertexAttribute {  
                format: VertexFormat::Uint32,  
                offset: 0,  
                shader\_location: 0,  
            }\],  
        };  
        descriptor.vertex.buffers \= vec\!\[vertex\_layout\];  
        Ok(())  
    }  
}

The application then constructs the material by wrapping the standard configuration:

Rust

let material \= ExtendedMaterial {  
    base: StandardMaterial {  
        base\_color: Color::WHITE,  
        perceptual\_roughness: 0.8,  
       ..default()  
    },  
    extension: VoxelMaterialExtension {},  
};

7

### **WGSL Fragment Pipeline Preservation**

In the corresponding voxel\_clipmap.wgsl file, the @vertex function unpacks the u32 (as detailed previously) and passes the resultant position, normal, and derived UV data into Bevy's native VertexOutput structure.47  
Because the MaterialExtension preserves the underlying StandardMaterial, the fragment shader does not need to be rewritten from scratch to calculate complex lighting equations. The output from the custom vertex shader is automatically ingested by Bevy's internal @fragment function. This function sequentially applies pbr\_input\_from\_standard\_material, apply\_pbr\_lighting, and main\_pass\_post\_lighting\_processing.47  
This seamless integration ensures that the heavily compressed voxel geometry responds flawlessly to point lights, directional shadows, screen-space ambient occlusion, and emissive bloom, preserving AAA graphic fidelity while maintaining raw data compression limits.47

## **Addressing Geometric Anomalies: LOD Seams and Topological Integrity**

Rendering billions of voxels across exponentially scaling LOD rings inevitably creates spatial artifacts and scale anomalies that must be addressed to preserve the illusion of a continuous world. The most glaring of these artifacts are LOD seams.

### **Resolving Clipmap LOD Seams via Skirts**

As the voxel clipmap transitions from a high-detail ring (e.g., Level $l$) to a lower-detail ring (Level $l+1$), the physical dimensions of the voxels double.21 Because the geometry is generated independently within each isolated chunk on separate asynchronous threads, vertices at the borders of differing LODs will not perfectly align. This topological mismatch results in visible gaps, T-junctions, or "cracks" in the terrain where the background skybox or underlying geometry erroneously bleeds through.50  
While complex triangle-strip stitching algorithms can be employed to procedurally morph and fill these gaps, such methods inject severe code complexity and computational overhead into the meshing task, often requiring neighboring chunks to stall until their adjacent counterparts finish generating.50 The most performant strategy, avoiding runtime overhead entirely, is the implementation of geometric "skirts".53  
During the greedy meshing phase, the algorithm is permitted to sample exactly one voxel outside the chunk's actual boundary. For the geometry generated precisely on the boundary edge of a chunk, an additional set of vertices is extruded downward (towards the center of the planet or negative Y) to form a skirt.53 Crucially, these skirt vertices retain the same normal vectors as the surface plane from which they were extruded. Because the normals are preserved, the lighting calculations on the skirts match the surface, rendering them virtually indistinguishable from standard terrain under flat lighting.53  
When a high-LOD chunk sits adjacent to a low-LOD chunk, the microscopic gaps that would normally appear are obscured by these downward-facing skirts, which act as a perfect occlusion barrier. This eliminates the necessity for dynamic cross-LOD dependency checking or vertex morphing, massively accelerating the chunk generation throughput.

## **Environmental Integration: Solari, Atmosphere, and Photometric Depth Cues**

The perception of immense scale and an infinite draw distance cannot rely solely on the presence of distant geometry; it requires photometric depth cues. Without atmospheric degradation, distant voxel mountains appear flat and artificial, breaking immersion. Bevy 0.18 provides a suite of advanced atmospheric capabilities that synergize flawlessly with the clipmap architecture.4

### **Solari Global Illumination Integrations**

Bevy 0.18's experimental Solari renderer introduces real-time raytraced Global Illumination (GI) without the need for static light baking.9 Solari implements direct diffuse lighting via ReSTIR DI, indirect final gather via ReSTIR GI, and multi-bounce indirect diffuse lighting via a world-space irradiance cache.9 Furthermore, Solari now natively supports specular materials utilizing a multiscattering GGX lobe, which integrates perfectly with the previously established ExtendedMaterial workflow.9  
To ensure that an infinite voxel draw distance does not exponentially stall the raytracer, the Solari architecture includes highly optimized limitations. During the world cache update, indirect rays are artificially clamped to a maximum travel distance of 50 meters.9 This prevents long raytraces from holding up the GPU threadgroups, improving overall frame pacing.  
Furthermore, Solari's LOD system dynamically calculates cell sizes based on the viewer's position using get\_cell\_size(world\_position, view\_position).9 The positions are quantized to generate lookup keys, ensuring that areas closer to the viewer possess higher lighting resolution.9 Consequently, as the voxel clipmap geometry scales logarithmically toward the horizon, the irradiance cache resolution degrades in perfect tandem.  
Bevy 0.18 also addresses historical "GI lag"—where lighting takes too long to fade in dynamic scenes—by implementing an adaptive blend factor.9 The system tracks the change in luminance between frames (luminance\_delta) and mixes the temporal samples with a dynamic alpha value. This allows the vast voxel world to remain stable in static conditions but react instantly to dynamic lighting changes, such as a moving sun or procedural terrain modifications.9

### **Volumetric Fog and Atmospheric Scattering**

Distant voxel LODs, despite representing the geometry accurately, can suffer from geometric aliasing and minor popping artifacts as new toroidal rings shift into existence. By applying Bevy 0.18's VolumetricFog and FogVolume components to the camera and environment, the engine calculates realistic atmospheric scattering using raymarching and anisotropic phase functions.10  
The integration of VolumetricFog relies on several critical parameters:

* ambient\_color and ambient\_intensity: Determines the baseline luminosity of the scattered light, independent of the primary directional light.10  
* step\_count: Dictates the number of raymarching steps to perform. Higher values reduce banding but incur a performance cost.10  
* jitter: Applies a maximum distance offset to the ray origin randomly. This is explicitly intended to function in conjunction with Temporal Anti-Aliasing (TAA) to smooth out the volumetric samples across multiple frames.10

By carefully tuning these parameters alongside the DirectionalLight intensity, distant clipmap rings naturally fade into an atmospheric haze. This creates cinematic "god rays" mapping through voxel canopies, while heavily obscuring the transition lines between the furthest LOD rings.55 The fog prevents the player from perceiving geometric shifts at the horizon, creating a seamless gradient from high-detail foreground voxels to the infinite, scattered sky.54

## **Conclusion**

Obtaining massive, "insane" draw distances in a voxel engine natively within Bevy 0.18 requires a total commitment to GPU-driven paradigms and aggressive data compression. By discarding the traditional reliance on individual entity processing and CPU-bound draw calls, and instead utilizing contiguous MultidrawableMesh binned phases, the CPU is relieved of its traditional rendering bottleneck.6  
The architectural synthesis of 3D geometry clipmaps managed by toroidal arrays 3, coupled with multithreaded chunk generation via the AsyncComputeTaskPool 23, provides an infinitely scalable topological framework. By aggressively packing vertex data into highly optimized 32-bit integers, PCIe memory bandwidth is preserved, while Bevy's MaterialExtension API dynamically unpacks the data directly into the robust PBR and Solari lighting pipelines.7 Finally, the intelligent application of geometric skirts alongside Bevy's Volumetric Fog elegantly obscures structural imperfections and scale transitions.10  
This convergence of techniques leverages the absolute state-of-the-art in graphics programming. It provides an engine-native, external-crate-free methodology for rendering voxel environments that push the boundaries of computational perception, demonstrating that infinite scale and high-fidelity rendering can coexist harmoniously within modern Rust ecosystems.

#### **References**

1. Aokana: A GPU-Driven Voxel Rendering Framework for Open World Games \- arXiv,  [https://arxiv.org/html/2505.02017v1](https://arxiv.org/html/2505.02017v1)  
2. Comparing a Clipmap to a Sparse Voxel Octree for Global Illumination \- Eric Arnebäck,  [https://erkaman.github.io/img/masters\_thesis/eric\_arneback\_masters\_thesis\_101.pdf](https://erkaman.github.io/img/masters_thesis/eric_arneback_masters_thesis_101.pdf)  
3. Chapter 2\. Terrain Rendering Using GPU-Based Geometry Clipmaps | NVIDIA Developer,  [https://developer.nvidia.com/gpugems/gpugems2/part-i-geometric-complexity/chapter-2-terrain-rendering-using-gpu-based-geometry](https://developer.nvidia.com/gpugems/gpugems2/part-i-geometric-complexity/chapter-2-terrain-rendering-using-gpu-based-geometry)  
4. Bevy 0.18 has been released\! \- Reddit,  [https://www.reddit.com/r/bevy/comments/1qc9tbx/bevy\_018\_has\_been\_released/](https://www.reddit.com/r/bevy/comments/1qc9tbx/bevy_018_has_been_released/)  
5. Bevy 0.18,  , [https://bevy.org/news/bevy-0-18/](https://bevy.org/news/bevy-0-18/)  
6. Specialized Mesh Pipeline \- Bevy Engine,  [https://bevy.org/examples/shaders/specialized-mesh-pipeline/](https://bevy.org/examples/shaders/specialized-mesh-pipeline/)  
7. Rust Game Dev Log \#6: Custom Vertex Shading using ExtendedMaterial,  [https://dev.to/mikeam565/rust-game-dev-log-6-custom-vertex-shading-using-extendedmaterial-4312](https://dev.to/mikeam565/rust-game-dev-log-6-custom-vertex-shading-using-extendedmaterial-4312)  
8. BinnedRenderPhaseType in bevy\_render::render\_phase \- Rust,  [https://doc.qu1x.dev/bevy\_trackball/bevy\_render/render\_phase/enum.BinnedRenderPhaseType.html](https://doc.qu1x.dev/bevy_trackball/bevy_render/render_phase/enum.BinnedRenderPhaseType.html)  
9. Realtime Raytracing in Bevy 0.18 (Solari) \- JMS55,  [https://jms55.github.io/posts/2025-12-27-solari-bevy-0-18/](https://jms55.github.io/posts/2025-12-27-solari-bevy-0-18/)  
10. VolumetricFog in bevy::light \- Rust \- Docs.rs,  [https://docs.rs/bevy/latest/bevy/light/struct.VolumetricFog.html](https://docs.rs/bevy/latest/bevy/light/struct.VolumetricFog.html)  
11. Bevy Rendering | Tainted Coders,  [https://taintedcoders.com/bevy/rendering](https://taintedcoders.com/bevy/rendering)  
12. Bevy 0.18 : r/rust \- Reddit,  [https://www.reddit.com/r/rust/comments/1qc4bsa/bevy\_018/](https://www.reddit.com/r/rust/comments/1qc4bsa/bevy_018/)  
13. bevy 0.18.1 \- Docs.rs,  [https://docs.rs/crate/bevy/latest/source/Cargo.toml.orig](https://docs.rs/crate/bevy/latest/source/Cargo.toml.orig)  
14. 0.17 to 0.18 \- Bevy Engine,  [https://bevy.org/learn/migration-guides/0-17-to-0-18/](https://bevy.org/learn/migration-guides/0-17-to-0-18/)  
15. Voxel Vendredi 8 \- Compress That Data\! : r/VoxelGameDev \- Reddit,  , [https://www.reddit.com/r/VoxelGameDev/comments/2g7xeb/voxel\_vendredi\_8\_compress\_that\_data/](https://www.reddit.com/r/VoxelGameDev/comments/2g7xeb/voxel_vendredi_8_compress_that_data/)  
16. Best data structure for massive voxel-based terrain : r/VoxelGameDev \- Reddit,  , [https://www.reddit.com/r/VoxelGameDev/comments/d7rjjc/best\_data\_structure\_for\_massive\_voxelbased\_terrain/](https://www.reddit.com/r/VoxelGameDev/comments/d7rjjc/best_data_structure_for_massive_voxelbased_terrain/)  
17. Uncompressed Format \- Voxel Compression,  , [https://eisenwave.github.io/voxel-compression-docs/uncompressed.html](https://eisenwave.github.io/voxel-compression-docs/uncompressed.html)  
18. Mesh generation, Voxels, and terminals \- This Week in Bevy,  , [https://thisweekinbevy.com/issue/2024-12-30-mesh-generation-voxels-and-terminals](https://thisweekinbevy.com/issue/2024-12-30-mesh-generation-voxels-and-terminals)  
19. WildPixelGames/voxelis: Tiny voxels. Huge worlds. Voxelis — a pure Rust voxel engine based on Sparse Voxel Octree DAG. \- GitHub,  , [https://github.com/WildPixelGames/voxelis](https://github.com/WildPixelGames/voxelis)  
20. kirillsurkov/bevy-clipmap: Render huge 3D worlds using Bevy\! \- GitHub,  , [https://github.com/kirillsurkov/bevy-clipmap](https://github.com/kirillsurkov/bevy-clipmap)  
21. Resources on chunked clipmap lod : r/VoxelGameDev \- Reddit,  , [https://www.reddit.com/r/VoxelGameDev/comments/kol6lt/resources\_on\_chunked\_clipmap\_lod/](https://www.reddit.com/r/VoxelGameDev/comments/kol6lt/resources_on_chunked_clipmap_lod/)  
22. What Causes & How to Fix 3D Printing Layer Shift \- anycubic-store,  , [https://store.anycubic.com/blogs/3d-printing-guides/3d-printing-layer-shift](https://store.anycubic.com/blogs/3d-printing-guides/3d-printing-layer-shift)  
23. AsyncComputeTaskPool in bevy::tasks \- Rust \- Docs.rs,  , [https://docs.rs/bevy/latest/bevy/tasks/struct.AsyncComputeTaskPool.html](https://docs.rs/bevy/latest/bevy/tasks/struct.AsyncComputeTaskPool.html)  
24. GitHub \- Game4all/vx\_bevy: Voxel engine prototype made with the bevy game engine. Serves as a playground for experimenting with voxels, terrain generation, and bevy.,  , [https://github.com/Game4all/vx\_bevy](https://github.com/Game4all/vx_bevy)  
25. High Performance Voxel Engine: Vertex Pooling \- Nick's Blog,  , [https://nickmcd.me/2021/04/04/high-performance-voxel-engine/](https://nickmcd.me/2021/04/04/high-performance-voxel-engine/)  
26. Add vertex attribute descriptor specialization to materials · bevyengine bevy · Discussion \#13386 \- GitHub,  , [https://github.com/bevyengine/bevy/discussions/13386](https://github.com/bevyengine/bevy/discussions/13386)  
27. Vertex buffers generation : r/VoxelGameDev \- Reddit,  , [https://www.reddit.com/r/VoxelGameDev/comments/16x6n5z/vertex\_buffers\_generation/](https://www.reddit.com/r/VoxelGameDev/comments/16x6n5z/vertex_buffers_generation/)  
28. Hardware-Compatible Vertex Compression Using Quantization and Simplification,  , [https://www.cs.jhu.edu/GLAB/papers/Purnomo05.pdf](https://www.cs.jhu.edu/GLAB/papers/Purnomo05.pdf)  
29. WebGPU WGSL,  , [https://webgpufundamentals.org/webgpu/lessons/webgpu-wgsl.html](https://webgpufundamentals.org/webgpu/lessons/webgpu-wgsl.html)  
30. Memory Layout in WGSL | Learn Wgpu \- GitHub Pages,  , [https://sotrh.github.io/learn-wgpu/showcase/alignment/](https://sotrh.github.io/learn-wgpu/showcase/alignment/)  
31. WebGPU Shading Language \- W3C,  , [https://www.w3.org/TR/WGSL/](https://www.w3.org/TR/WGSL/)  
32. How to unpack byte into vec3? \- opengl \- Stack Overflow,  , [https://stackoverflow.com/questions/28400453/how-to-unpack-byte-into-vec3](https://stackoverflow.com/questions/28400453/how-to-unpack-byte-into-vec3)  
33. Cheat sheet for WGSL syntax for developers coming from GLSL. \- GitHub,  , [https://github.com/paulgb/wgsl-cheat-sheet](https://github.com/paulgb/wgsl-cheat-sheet)  
34. Graphics Tech in Cesium \- Vertex Compression,  , [https://cesium.com/blog/2015/05/18/vertex-compression/](https://cesium.com/blog/2015/05/18/vertex-compression/)  
35. Questions on a voxel game with Bevy \- Reddit,  , [https://www.reddit.com/r/bevy/comments/190wu4r/questions\_on\_a\_voxel\_game\_with\_bevy/](https://www.reddit.com/r/bevy/comments/190wu4r/questions_on_a_voxel_game_with_bevy/)  
36. How to separate logic from visualization in an ECS architecture? Should I create specific systems? : r/roguelikedev \- Reddit,  , [https://www.reddit.com/r/roguelikedev/comments/7jizta/how\_to\_separate\_logic\_from\_visualization\_in\_an/](https://www.reddit.com/r/roguelikedev/comments/7jizta/how_to_separate_logic_from_visualization_in_an/)  
37. Shaders / Custom Render Phase \- Bevy Engine,  , [https://bevy.org/examples/shaders/custom-render-phase/](https://bevy.org/examples/shaders/custom-render-phase/)  
38. New Meshes, New Examples, and Compute Shaders \- This Week in Bevy,  , [https://thisweekinbevy.com/issue/2024-04-15-new-meshes-new-examples-and-compute-shaders](https://thisweekinbevy.com/issue/2024-04-15-new-meshes-new-examples-and-compute-shaders)  
39. A walkthrough of bevy's rendering · bevyengine bevy · Discussion \#9897 \- GitHub,  , [https://github.com/bevyengine/bevy/discussions/9897](https://github.com/bevyengine/bevy/discussions/9897)  
40. Vulkan Backend, IK Spiders, and holiday voxels \- This Week in Bevy,  , [https://thisweekinbevy.com/issue/2024-11-11-vulkan-backend-ik-spiders-and-holiday-voxels](https://thisweekinbevy.com/issue/2024-11-11-vulkan-backend-ik-spiders-and-holiday-voxels)  
41. "Draw" Search \- Rust \- Docs.rs,  , [https://docs.rs/bevy/latest/bevy/?search=Draw](https://docs.rs/bevy/latest/bevy/?search=Draw)  
42. bevy/examples/shader\_advanced/compute\_mesh.rs at main \- GitHub,  , [https://github.com/bevyengine/bevy/blob/main/examples/shader\_advanced/compute\_mesh.rs](https://github.com/bevyengine/bevy/blob/main/examples/shader_advanced/compute_mesh.rs)  
43. GPU Instancing · Issue \#89 · bevyengine/bevy \- GitHub,  , [https://github.com/bevyengine/bevy/issues/89?timeline\_page=1](https://github.com/bevyengine/bevy/issues/89?timeline_page=1)  
44. Compute Shaders in Bevy \- YouTube,  , [https://www.youtube.com/watch?v=neyIpnII-WQ](https://www.youtube.com/watch?v=neyIpnII-WQ)  
45. How to reduce number of compute shader invocations when frustum culling on GPU?,  , [https://www.reddit.com/r/GraphicsProgramming/comments/1bs1n11/how\_to\_reduce\_number\_of\_compute\_shader/](https://www.reddit.com/r/GraphicsProgramming/comments/1bs1n11/how_to_reduce_number_of_compute_shader/)  
46. IndirectParametersIndexed in bevy::render::batching::gpu\_preprocessing \- Rust \- Docs.rs,  , [https://docs.rs/bevy/latest/bevy/render/batching/gpu\_preprocessing/struct.IndirectParametersIndexed.html](https://docs.rs/bevy/latest/bevy/render/batching/gpu_preprocessing/struct.IndirectParametersIndexed.html)  
47. How can I use custom vertex attributes without abandoning the built-in PBR shading? : r/bevy \- Reddit,  , [https://www.reddit.com/r/bevy/comments/1j3slog/how\_can\_i\_use\_custom\_vertex\_attributes\_without/](https://www.reddit.com/r/bevy/comments/1j3slog/how_can_i_use_custom_vertex_attributes_without/)  
48. MaterialExtension in bevy::pbr \- Rust \- Docs.rs,  , [https://docs.rs/bevy/latest/bevy/pbr/trait.MaterialExtension.html](https://docs.rs/bevy/latest/bevy/pbr/trait.MaterialExtension.html)  
49. Pushing the rendering limits \- Rust Voxel Engine \- YouTube,  , [https://www.youtube.com/watch?v=23yc2oPEqlg](https://www.youtube.com/watch?v=23yc2oPEqlg)  
50. Struggling with Cross LOD Seam Generation in Procedural Terrain—Any Advice? \- Reddit,  , [https://www.reddit.com/r/proceduralgeneration/comments/1gj10ql/struggling\_with\_cross\_lod\_seam\_generation\_in/](https://www.reddit.com/r/proceduralgeneration/comments/1gj10ql/struggling_with_cross_lod_seam_generation_in/)  
51. Dual Contouring: Seams & LOD for Chunked Terrain \- Nick's Voxel Blog,  , [http://ngildea.blogspot.com/2014/09/dual-contouring-chunked-terrain.html](http://ngildea.blogspot.com/2014/09/dual-contouring-chunked-terrain.html)  
52. Voxel LOD and seam stitching : r/VoxelGameDev \- Reddit,  , [https://www.reddit.com/r/VoxelGameDev/comments/b7w9ip/voxel\_lod\_and\_seam\_stitching/](https://www.reddit.com/r/VoxelGameDev/comments/b7w9ip/voxel_lod_and_seam_stitching/)  
53. Massive Infinite Terrain that Generates Instantly Part 2: LOD & Mesh Stitching \- YouTube,  , [https://www.youtube.com/watch?v=jDM0m4WuBAg](https://www.youtube.com/watch?v=jDM0m4WuBAg)  
54. Phyisically based unified volumetrics system · Issue \#18151 · bevyengine/bevy \- GitHub,  , [https://github.com/bevyengine/bevy/issues/18151](https://github.com/bevyengine/bevy/issues/18151)  
55. God Rays without strong fog and adding atmospheric dust : r/bevy \- Reddit,  , [https://www.reddit.com/r/bevy/comments/1qli01a/god\_rays\_without\_strong\_fog\_and\_adding/](https://www.reddit.com/r/bevy/comments/1qli01a/god_rays_without_strong_fog_and_adding/)