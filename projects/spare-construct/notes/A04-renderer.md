# A04: Renderer: our own on wgpu, or three.js first

## The question

The 3D and 2D views come from a GPU renderer fed straight from the core ([ARCHITECTURE.md](../ARCHITECTURE.md), Rendering and front end). The map gives two options, three.js ("fast start, widely used, WebGPU with WebGL2 fallback") or our own renderer on wgpu, and leans to wgpu "so desktop and browser share one renderer in the core's language". Its question is: "Our wgpu renderer from day one, or three.js first and replace it later?" The piece asks for a recommendation covering instancing, culling and browser support. The decision is made once for Spare Construct and Spare Studio ([KIT-A27-F1-P1](../../kit/FEATURES.md)), and its result is the `engine-render` crate, which "waits on decision: renderer (Spare Construct piece A04)" ([lib.rs](../../engines/crates/engine-render/src/lib.rs)).

Sources were read on 12 November 2026: wgpu at commit [`c92258f`](https://github.com/gfx-rs/wgpu/tree/c92258fcae03572ced056fa9162370d3ebcf8143) (main, after release 30.0.0), three.js at [`5ec3113`](https://github.com/mrdoob/three.js/tree/5ec3113785ff183027f320335b1907a4be6bcd0a) (r188 in development), Bevy at [`26df274`](https://github.com/bevyengine/bevy/tree/26df274845b039cacccf34bde524344241ffb2e6) (0.20 in development) as the largest renderer built on wgpu, That Open's fragments 3.4.0 at [`bc3f688`](https://github.com/ThatOpen/engine_fragment/tree/bc3f6880d3698154349174954bc391e5345c3d19) as the open BIM viewer built on three.js, and this repo's app. Browser support was read from the WebGPU group's [implementation status](https://github.com/gpuweb/gpuweb/wiki/Implementation-Status) and [caniuse](https://caniuse.com/webgpu) on the same day. A small spike built a wgpu program for the browser (below). "Unverified" marks anything not confirmed in source or by measurement.

## What the renderer has to do

From the map and the notes already accepted or written:

- **Where it runs.** People work in the browser; there is no local model file ([K08](../../kit/notes/K08-cloud-foundation.md), decided). The map's desktop shell is overtaken by K08 until that section is rewritten. So the renderer must run in a browser tab, on the machines people already have.
- **Scale.** 60 frames a second with 1,000,000 instances (Performance targets). One draw call for every copy of a type, so 2,000 doors cost one call; frustum and occlusion culling on the GPU, simplified meshes in the distance, meshlet clusters for very dense models (Rendering, low level).
- **Fed by the core, not parsed.** Core and client talk through one binary protocol; geometry comes as ready-to-upload GPU buffers, and the core sends only what is in view (Runtime shape). The browser core runs in a Web Worker.
- **City coordinates.** Positions go to the GPU relative to the camera, so NZTM values never jitter (Data model, Rendering).
- **2D views.** Lines at true weights at any zoom and patterns as GPU fills, from the drawing engine's output (Views and drawings, Rendering).
- **Picking.** A GPU ID buffer for picking, snapping against exact kernel geometry (Rendering).
- **More than a viewer.** Photo-real rendering and VR are built in (CON-A16), agents can render any view to an image to check their own work ([CON-A19-F3-P11](../FEATURES.md)), and cloud rendering runs on co-op servers (CON-A23-F6-P3). Spare Studio runs its image and video effects as GPU compute on the same renderer ([Spare Studio map](../../spare-studio/ARCHITECTURE.md), GPU image pipeline).
- **Nothing it computes feeds the model.** GPU arithmetic is not bit-identical across vendors, so wgpu and its kin are allowed "in programs, for display only" ([kernel 29](../../engines/geometry/architecture/29-sources-licences.md), sources table).

What exists today: the app's 3D view is three.js 0.170.0 from a CDN ([head.html](../app/head.html)), with a `WebGLRenderer`, one `Mesh` per element, rebuilt whole on each update ([connect.js](../app/connect.js), `startThree`). That is "three.js first", already in place.

## Options

| | A. Our own renderer on wgpu | B. three.js first, replaced later | C. three.js as the product renderer |
| --- | --- | --- | --- |
| What it is | `engine-render` in Rust, in the core's worker, drawing through WebGPU (and WebGL2 as a fallback) | Keep and grow today's three.js view; write `engine-render` later and swap | Grow the three.js view into the product, with our own shaders where needed |
| Licence | MIT or Apache-2.0 | MIT | MIT |
| Language | Rust, the core's | JavaScript, the interface's | JavaScript |
| One draw per type | `draw(0..n, 0..instances)`; indirect draws | `InstancedMesh`, or `BatchedMesh` with multi-draw | Same as B |
| GPU culling | Compute plus indirect draws on WebGPU; none on WebGL2 | Possible through WebGPURenderer's compute and indirect draws; none on WebGL2 | Same as B |
| Outside the browser | Native on Vulkan, Metal and DirectX 12, headless on a server | Needs a browser, or headless Chrome on a server | Same as B |

Babylon.js (Apache-2.0) was listed in the map as a renderer option. It was not read in source for this note; it sits where three.js does on every row above that this note decides on (*unverified*), so it is not taken further.

## Evidence

### Instancing

Both do one draw call per type. In wgpu an instanced draw is one call with an instance range, and the spike below draws n boxes with `pass.draw(0..36, 0..n)` from a per-instance vertex buffer. In three.js, an `InstancedMesh` draws its instances in one call, and `BatchedMesh` packs many geometries into one buffer and draws them with multi-draw ([BatchedMesh.js L278-L281](https://github.com/mrdoob/three.js/blob/5ec3113785ff183027f320335b1907a4be6bcd0a/src/objects/BatchedMesh.js#L278-L281)). That Open's fragments draw a whole BIM model this way on three.js. So instancing does not decide between the options; what feeds it does (below, "The data path").

### Culling

**On the CPU, three.js culls an `InstancedMesh` as one object.** Its culling uses a single bounding sphere for all instances ([InstancedMesh.js L151-L170](https://github.com/mrdoob/three.js/blob/5ec3113785ff183027f320335b1907a4be6bcd0a/src/objects/InstancedMesh.js#L151-L170)): 2,000 doors are drawn or skipped together. `BatchedMesh` culls each object, by looping over every instance on every frame where the camera moved, transforming its bounding sphere and testing it against the frustum, in JavaScript ([BatchedMesh.js L1612-L1665](https://github.com/mrdoob/three.js/blob/5ec3113785ff183027f320335b1907a4be6bcd0a/src/objects/BatchedMesh.js#L1612-L1665)). At 1,000,000 instances that is a million sphere tests per frame on the thread that draws; its cost was not measured here (*unverified*), but it is the work the map moves to the GPU.

**That Open moves culling off the drawing thread.** fragments culls and picks levels of detail in a worker, over its own box tree, and the main thread only draws what comes back; it skips the worker call when the camera has not moved ([view-manager.ts L21-L53](https://github.com/ThatOpen/engine_fragment/blob/bc3f6880d3698154349174954bc391e5345c3d19/packages/fragments/src/FragmentsModels/src/model/view-manager.ts#L21-L53); `bounding-boxes/`, `lod/`, `multithreading/`). That is the same split Spare Construct's map already has: the core, in a worker, answers "what is in this view" from its spatial index and sends only that ([kernel 19](../../engines/geometry/architecture/19-meshes-spatial-clash.md), Storage and scale). Chunk-level culling is therefore the core's on either option, not the renderer's.

**On the GPU, both need WebGPU.** wgpu's WebGL2 limits have no storage buffers and no compute ([limits.rs L690-L715](https://github.com/gfx-rs/wgpu/blob/c92258fcae03572ced056fa9162370d3ebcf8143/wgpu-types/src/limits.rs#L690-L715)): "WebGL2, and GLES3.0 devices do not support compute" ([L1159-L1162](https://github.com/gfx-rs/wgpu/blob/c92258fcae03572ced056fa9162370d3ebcf8143/wgpu-types/src/limits.rs#L1159-L1162)), and indirect drawing needs compute (L1165-L1167). Bevy states the consequence: "No GPU preprocessing is supported on WebGL because of the lack of compute shader support" ([gpu_preprocessing.rs L107-L114](https://github.com/bevyengine/bevy/blob/26df274845b039cacccf34bde524344241ffb2e6/crates/bevy_render/src/batching/gpu_preprocessing.rs#L107-L114)). three.js's WebGPU backend runs compute and indirect draws ([WebGPUBackend.js L1946-L1997](https://github.com/mrdoob/three.js/blob/5ec3113785ff183027f320335b1907a4be6bcd0a/src/renderers/webgpu/WebGPUBackend.js#L1946-L1997), [L2226-L2238](https://github.com/mrdoob/three.js/blob/5ec3113785ff183027f320335b1907a4be6bcd0a/src/renderers/webgpu/WebGPUBackend.js#L2226-L2238)), and its WebGL2 fallback refuses an indirect dispatch ([WebGLBackend.js L1059-L1061](https://github.com/mrdoob/three.js/blob/5ec3113785ff183027f320335b1907a4be6bcd0a/src/renderers/webgl-fallback/WebGLBackend.js#L1059-L1061)). So GPU culling is possible in both and only on WebGPU. In three.js it is our own compute code written in its node language against its internals; three.js ships no GPU culling of its own that was found in `src/` (*unverified* beyond a search for indirect draws).

**Multi-draw indirect is native only.** On WebGPU, wgpu's `multi_draw_indirect` is a loop of single indirect draws; only DirectX 12 and Vulkan 1.2 issue the whole buffer in one call with a GPU-side count ([features.rs L573-L590](https://github.com/gfx-rs/wgpu/blob/c92258fcae03572ced056fa9162370d3ebcf8143/wgpu-types/src/features.rs#L573-L590)). In the browser, a GPU-culled frame is one indirect draw per type and level of detail. That is still one call per type, which is what the map asks.

**Meshlets are native only today.** Bevy's virtual-geometry meshlets need 64-bit texture atomics, which wgpu marks native only ([meshlet/mod.rs L94-L126](https://github.com/bevyengine/bevy/blob/26df274845b039cacccf34bde524344241ffb2e6/crates/bevy_pbr/src/meshlet/mod.rs#L94-L126); [features.rs L926-L934](https://github.com/gfx-rs/wgpu/blob/c92258fcae03572ced056fa9162370d3ebcf8143/wgpu-types/src/features.rs#L926-L934)). The map's "meshlet clusters for very dense models" therefore means, in a browser, a method without 64-bit atomics, such as the cluster culling and hierarchical levels of detail the kernel map lists to study in nanite-webgpu (MIT; kernel 29, *not read here*), or the core's simplified chunk meshes (Storage and scale).

### Browser support

WebGPU is on by default in Chrome and Edge from 113 on Windows and macOS (Linux on some hardware from 144 and 147), in Safari from 26 on macOS and iOS, and in Firefox from 141 on Windows and 145 to 147 on macOS; Firefox on Linux and on Android has it in Nightly or behind a flag ([implementation status](https://github.com/gpuweb/gpuweb/wiki/Implementation-Status), read 12 November 2026). caniuse counts 86.8% of global use as full support and 1.8% partial (Safari desktop), from StatCounter's October 2026 data ([caniuse](https://caniuse.com/webgpu)). So roughly one browser in ten needs a WebGL2 fallback, and those fall in a few known places: Firefox on Linux, older Safari, some Android devices.

Both options have that fallback. three.js's `WebGPURenderer` falls back to WebGL2 by itself when WebGPU is missing ([WebGPURenderer.js L22-L69](https://github.com/mrdoob/three.js/blob/5ec3113785ff183027f320335b1907a4be6bcd0a/src/renderers/webgpu/WebGPURenderer.js#L22-L69)). wgpu has a `webgl` backend for WebAssembly beside `webgpu` ([wgpu/Cargo.toml L61-L85](https://github.com/gfx-rs/wgpu/blob/c92258fcae03572ced056fa9162370d3ebcf8143/wgpu/Cargo.toml#L61-L85)), with WebGL2's limits, so no compute. Neither fallback can run GPU culling, so on those browsers culling stays at the core's chunk level plus whole-type culling, and the frame-rate target is for WebGPU browsers only. Which browsers must meet which targets is the map's own open question on browser use.

### The data path

The core runs in a Web Worker and builds GPU-ready buffers (Runtime shape). wgpu can draw from a worker: a surface can be made from an `OffscreenCanvas` ([surface.rs L324-L334](https://github.com/gfx-rs/wgpu/blob/c92258fcae03572ced056fa9162370d3ebcf8143/wgpu/src/api/surface.rs#L324-L334)). With option A the renderer lives in the same worker and the same WebAssembly memory as the core, so a chunk's buffers go from the core's arrays to the GPU with no copy to another thread. With three.js on the page's thread, buffers cross from the worker as transferred `ArrayBuffer`s, which is cheap but means two copies of state (the core's and the scene graph's) and the page's thread doing per-frame work such as `BatchedMesh`'s culling loop. three.js can also run in a worker on an `OffscreenCanvas` (*unverified* for `WebGPURenderer`), but its scene is then still a second, JavaScript copy of what the core holds.

Today's app shows the cost of the scene-graph path at small scale: each update disposes every geometry and rebuilds one `Mesh` per element, flattened to non-indexed triangles ([connect.js](../app/connect.js), `C.three.update`). That is fine for a demo and is what any renderer built on the core's chunk buffers replaces.

### City coordinates and 2D lines

Neither library gives camera-relative positions by itself. In wgpu it is a few lines of our own shader: each instance carries its offset from a chunk origin, and the camera's position relative to that origin is subtracted in double-single or on the CPU per chunk, as the spike's shader takes offsets relative to the camera. In three.js it is a floating origin moved on the CPU, or custom shader nodes. Lines at true weight are screen-space quads in either: our own shader in A, or three.js's `Line2` and `LineMaterial` from its examples in B (*unverified* for the WebGPU renderer). Neither decides the choice.

### Outside the browser

The map needs rendering where no browser is: agents rendering a view to check their work (CON-A19-F3-P11), cloud rendering on co-op servers (CON-A23-F6-P3), and Spare Studio's GPU compute effects. wgpu runs natively on Vulkan, Metal and DirectX 12, headless or into a window, with the same code; three.js on a server needs a headless browser. That favours one renderer in Rust for every program and the server.

### Size of the download

The spike is a cdylib that makes a wgpu device, compiles one WGSL shader and draws instanced boxes into a canvas: wgpu 30.0.0, Rust 1.99.0, `opt-level = "s"`, link-time optimisation, panic abort, stripped, before `wasm-bindgen` and `wasm-opt`. It built to 1.03 MB (225 KB after gzip) with the webgpu backend, 3.12 MB (941 KB gzip) with the webgl backend, and 3.50 MB (1.03 MB gzip) with both. The WebGPU backend calls the browser's own WebGPU, while the WebGL backend brings wgpu-core, wgpu-hal and naga's shader translation into the download, which is why it is larger. Not run: no browser on the build machine. three.js's build was not measured here.

### Keeping up

wgpu released eight breaking major versions from 23.0.0 (October 2024) to 30.0.0 (July 2026), from the repo's tags; the spike hit three API changes from earlier versions in its first build (surface textures, presenting through the queue, optional vertex layouts). three.js changes its API less sharply but moves fast too: today's app pins r170, and r188 is in development. Either way the renderer is pinned and upgraded on purpose. wgpu is used at scale by Bevy, Firefox's WebGPU and others (*unverified* beyond Bevy, read here).

## Recommendation

1. **Our own renderer on wgpu (option A) as the product renderer, in `engine-render`.** This confirms the map's lean, for reasons that changed with K08. Instancing does not decide it: both draw a type in one call. Culling does not decide it either: chunk culling is the core's on both options, and GPU culling needs WebGPU on both. What decides it is that the renderer can live in the core's worker and memory and draw the core's chunk buffers directly, that the same code renders headless on the server for agents and cloud rendering and runs Spare Studio's GPU effects, and that GPU culling written in WGSL is ours to keep, not code written against another library's internals.
2. **Keep three.js in the app until then, behind the protocol.** Today's three.js view is not thrown away; it is the stand-in, like the regenerator's box stand-in for the kernel. Have it read the same chunk buffers the protocol will send (one per type and level of detail, positions relative to a chunk origin) through one module, so swapping in `engine-render` changes that module and nothing else. Moving it to `BatchedMesh` or `InstancedMesh` per type now would also stop the full rebuild on every update. This is UI work that waits on nothing.
3. **Browser support: WebGPU first, WebGL2 as a lesser fallback.**
   - Build `engine-render` with wgpu's `webgpu` backend as the default.
   - On browsers without WebGPU, use wgpu's `webgl` backend: instancing and the core's chunk culling, no GPU culling, shading and lines only. The page tells the person what they are missing in plain words (CON-A1-F1-P3, hardware check). The 60 fps at 1,000,000 instances target applies to WebGPU browsers.
   - Decide in Design, from measurement, whether the fallback's extra download is worth its users, or whether it is a separate, smaller build loaded only when WebGPU is missing.
4. **Culling, in three layers, each a named method:**
   - The core's spatial index answers "what chunks are in view" and streams those, near chunks in full and far ones simplified (Storage and scale; That Open's worker culling is the browser precedent).
   - On WebGPU, a compute pass tests each instance's box against the frustum and writes one indirect draw per type and level of detail (Bevy's GPU preprocessing on wgpu, read here, is the method to follow).
   - Occlusion by a depth pyramid from the previous frame, two-pass, as Bevy's occlusion culling is understood to do (`occlusion_culling`, imported in gpu_preprocessing.rs; *unverified*, to read in Design). Meshlets wait for a browser method without 64-bit atomics.
5. **Precision and picking:** instance offsets relative to chunk origins and the camera's position relative to the chunk subtracted per chunk on the CPU in `f64`, so the GPU sees small `f32` values; an ID buffer pass for picking, as the map says.
6. **Benchmarks in Design.** The Phase 0 harness renders a generated city of 1,000,000 instances in 2,000 types in Chrome, Safari and Firefox on one mid-range laptop: frame time with the camera moving, culling time, first frame after open, memory, and download size. The three.js stand-in is measured on the same scene as the yardstick. If `engine-render` is not at least as fast as the stand-in, revisit this note.

## Open risks

- **A renderer is a large piece of work.** three.js has years of materials, lighting, post-processing and text that `engine-render` starts without. Keeping three.js as the stand-in (item 2) covers the time before it is ready; photo-real rendering (CON-A16) is its own later piece either way.
- **WebGPU in browsers is young.** Bugs and limits differ by browser and driver; Firefox on Linux and many Android devices do not have it yet. Measured per browser in the benchmark above.
- **The fallback's cost.** wgpu's WebGL2 backend is larger than its WebGPU one (spike) and does less. If the fallback is little used, a separate small build may be better; to decide from data.
- **wgpu changes often.** Eight major versions in under two years. Pin it and upgrade on purpose, as every dependency here is.
- **Text in 3D and on sheets** (tags, dimensions) needs glyph rendering; `engine-text` shapes and lays it out, and how glyphs reach the GPU (atlas or vector) is a Design question.
- **The spike is small.** It measured download size only, without `wasm-opt`, and did not run in a browser.

## Sources

| Source | Licence | Used for |
| --- | --- | --- |
| [wgpu](https://github.com/gfx-rs/wgpu/tree/c92258fcae03572ced056fa9162370d3ebcf8143) | MIT or Apache-2.0 | Backends, WebGL2 limits, indirect drawing, surfaces; spike |
| [three.js](https://github.com/mrdoob/three.js/tree/5ec3113785ff183027f320335b1907a4be6bcd0a) | MIT | Instancing, per-object culling, WebGPU renderer and its fallback |
| [Bevy](https://github.com/bevyengine/bevy/tree/26df274845b039cacccf34bde524344241ffb2e6) | MIT or Apache-2.0 | GPU culling on wgpu and where it runs; meshlet requirements |
| [That Open fragments](https://github.com/ThatOpen/engine_fragment/tree/bc3f6880d3698154349174954bc391e5345c3d19) | MIT | BIM-scale culling and levels of detail in a worker, on three.js |
| [WebGPU implementation status](https://github.com/gpuweb/gpuweb/wiki/Implementation-Status), [caniuse](https://caniuse.com/webgpu) | Wiki and data pages | Browser support |
| [Babylon.js](https://github.com/BabylonJS/Babylon.js) | Apache-2.0 | Named in the map; not read here |
| Kernel map, chapters [19](../../engines/geometry/architecture/19-meshes-spatial-clash.md) and [29](../../engines/geometry/architecture/29-sources-licences.md) | This repo | Spatial index; display-only rule for GPU code |
| [K08](../../kit/notes/K08-cloud-foundation.md) | This repo | Browser only |
