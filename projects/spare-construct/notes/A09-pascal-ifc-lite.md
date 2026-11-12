# A09: Study Pascal editor and ifc-lite

## The question

The map lists two open projects to study: the Pascal editor, a "browser BIM editor to study; cuts openings, has MCP" (MIT), and ifc-lite, a "Rust/WebAssembly IFC core with headless SDK and MCP to study" (MPL-2.0) ([ARCHITECTURE.md](../ARCHITECTURE.md), Sources). The piece asks what we can reuse under their licences, and what to learn from.

Sources were read on 12 November 2026: Pascal at [`51bf407`](https://github.com/pascalorg/editor/tree/51bf4071c10d0189c2499ec2069e580b74169106) (`@pascal-app/core` 1.1.1) and ifc-lite at [`b085c65`](https://github.com/LTplus-AG/ifc-lite/tree/b085c65b50c4355e5d0db202f4f486f5703b48c9) (Rust workspace 28.2.0; `@ifc-lite/renderer` 7.1.0). Both are large and change daily, so this note reads the parts that bear on Spare Construct's open pieces, not everything. Figures from a project's own README are marked as theirs; "unverified" marks anything else not confirmed in source.

## What each project is

**Pascal** is a "local-first, MIT-licensed building editor" whose agents "read, edit, validate, and save the same scenes", "built with React Three Fiber and WebGPU" ([README L4-L10](https://github.com/pascalorg/editor/blob/51bf4071c10d0189c2499ec2069e580b74169106/README.md#L4-L10)). It is TypeScript throughout: `packages/core` (schema, commands, building systems, agent operations, procedural items), `editor`, `viewer`, `mcp`, `cli`, `ifc-converter` and `geometry-script`, about 1,600 source files outside tests in core, editor and viewer. Its 3D is three.js 0.187 through React Three Fiber ([core package.json](https://github.com/pascalorg/editor/blob/51bf4071c10d0189c2499ec2069e580b74169106/packages/core/package.json)); IFC goes through web-ifc ([ifc-converter package.json](https://github.com/pascalorg/editor/blob/51bf4071c10d0189c2499ec2069e580b74169106/packages/ifc-converter/package.json)). Copyright is held by Pascal Group Inc. ([LICENSE](https://github.com/pascalorg/editor/blob/51bf4071c10d0189c2499ec2069e580b74169106/LICENSE)).

**ifc-lite** parses, views, queries, edits, validates and exports IFC "entirely client-side": "a Rust core compiled to WASM does the parsing and geometry, a WebGPU renderer puts it on screen" ([README L30](https://github.com/LTplus-AG/ifc-lite/blob/b085c65b50c4355e5d0db202f4f486f5703b48c9/README.md#L30)). It reads IFC2X3, IFC4, IFC4X3 and IFC5 (IFCX), and beside IFC opens glTF, LandXML, point clouds and Gaussian splats (L32). Its Rust workspace has 17 crates (core parser, geometry, processing, clash, diff, export, LandXML, FFI, Python, WebAssembly bindings and more), and its npm side has 66 packages, among them a renderer, MCP server, BCF, IDS, CRDT collaboration on IFCX and connectors to document platforms. Its geometry bundles NVIDIA's usd-brep SMLib through Emscripten beside an in-tree exact kernel ([licenses/retained/README.md](https://github.com/LTplus-AG/ifc-lite/blob/b085c65b50c4355e5d0db202f4f486f5703b48c9/licenses/retained/README.md)).

## What the licences allow

Spare Construct is Apache-2.0 ([K07](../../kit/notes/K07-licence.md); map, Community and licence).

- **Pascal, MIT:** its code may be copied into our files, with its copyright notice kept and the source named in the pull request (AGENTS.md, step 5). It is TypeScript on three.js, so what could be copied is interface code and tests, not the Rust core. AI-written code and provenance are not stated in the repository (*unverified* either way); as with CADCraft (kernel 29), anything copied gets a provenance check first.
- **ifc-lite, MPL-2.0:** a file-level copyleft. Its packages and crates may be used as dependencies, unmodified or with our changes published under MPL-2.0 in those files, inside an Apache-2.0 program. Its code may not be copied into our own Apache-2.0 files. The kernel's source table treats web-ifc, also MPL-2.0, as study and oracle ([29](../../engines/geometry/architecture/29-sources-licences.md), web-ifc row), and Spare Construct's interop plan writes its own IFC reader and writer with web-ifc and IfcOpenShell as references (map, Interop). The same treatment fits ifc-lite.
- **ifc-lite's geometry has its own provenance question.** It bundles SMLib. The kernel's owners accepted SMLib's own routines on conditions (kernel 29, Numerical Recipes rows; [G01 evidence 13](../../kit/notes/G01-evidence/13-vetting-smlib.md)), and those conditions are the kernel's to apply. Nothing from ifc-lite's geometry is a candidate for porting here.

## What to learn, piece by piece

### Renderer (A04)

ifc-lite wrote its own WebGPU renderer in TypeScript rather than use three.js or Babylon.js, which it supports only as integration examples on WebGL (README L64, L123, L295-L296). Its renderer is 55,902 lines of TypeScript outside tests, and asks for WebGPU only (`navigator.gpu`, [device.ts L69](https://github.com/LTplus-AG/ifc-lite/blob/b085c65b50c4355e5d0db202f4f486f5703b48c9/packages/renderer/src/device.ts#L69)); no WebGL path of its own was found (*unverified* beyond a search). What it does for large BIM models, each a method A04's renderer can follow:

- **Camera-relative instances in split precision:** one `(drawable - camera)` high and low pair per occurrence, packed on the CPU in `f64`, in its own buffer so a frame needs one write instead of one per occurrence ([instanced-rte.ts L6-L17](https://github.com/LTplus-AG/ifc-lite/blob/b085c65b50c4355e5d0db202f4f486f5703b48c9/packages/renderer/src/instanced-rte.ts#L6-L17)); frustum tests in the same eye-relative frame, because national-grid boxes against an absolute `f32` matrix lose precision ([rte-frustum.ts L6-L13](https://github.com/LTplus-AG/ifc-lite/blob/b085c65b50c4355e5d0db202f4f486f5703b48c9/packages/renderer/src/rte-frustum.ts#L6-L13)). This is the map's "positions relative to the camera", worked out for NZTM-sized coordinates.
- **Spatial batches:** draw batches keyed by grid cell as well as colour, because a model-wide batch has a model-sized box and "per-batch frustum/contribution culling almost never fires" ([chunk-grid.ts L6-L17](https://github.com/LTplus-AG/ifc-lite/blob/b085c65b50c4355e5d0db202f4f486f5703b48c9/packages/renderer/src/chunk-grid.ts#L6-L17)). That is Spare Construct's chunks reaching the GPU.
- **Contribution culling:** skip draws whose box projects below a pixel threshold, raised while the camera moves ([contribution-cull.ts L6-L17](https://github.com/LTplus-AG/ifc-lite/blob/b085c65b50c4355e5d0db202f4f486f5703b48c9/packages/renderer/src/contribution-cull.ts#L6-L17)).
- **Device loss:** the renderer recovers when the browser or driver drops the GPU device (`device-recovery.ts`, `device.ts`), which any WebGPU renderer has to handle.

This supports A04's recommendation of our own renderer and shows its size: a mature BIM renderer is tens of thousands of lines.

### IFC reading and checking (interop)

- **Parser method:** a STEP tokenizer on `nom` (MIT) with entity scanning on `memchr` (MIT or Unlicense), lazy decoding of attributes and an event-based streaming mode ([rust/core/src/lib.rs L1-L15](https://github.com/LTplus-AG/ifc-lite/blob/b085c65b50c4355e5d0db202f4f486f5703b48c9/rust/core/src/lib.rs#L1-L15)), with a columnar store on the TypeScript side (`parseColumnar`, README L79-L109). Its README gives about 1.2 GB/s for tokenizing and 50 MB/s for a full parse, and a 2.6 MB gzipped WebAssembly module (L282-L287, theirs). Those are the yardsticks for our own reader.
- **Checking geometry against an oracle:** ifc-lite verifies its output "element-by-element against IfcOpenShell across the public benchmark corpus", 99.9% agreement by its README (L283, theirs). Spare Construct's IFC export and import can use the same practice, with IfcOpenShell out of process as the kernel's testing chapter already allows for LGPL oracles (kernel 28).
- **Streaming first render:** geometry in batches across workers, so the first triangles show while the file still parses (L282, L284). The same order suits Spare Construct's "first view in under 3 s" target.

### AI agents (CON-A19)

Both projects expose the model to agents through MCP, and Pascal's design is the more developed:

- **Resources an agent reads first:** an agent guide, the design schema, constraints, the current scene and a summary (`packages/mcp/src/resources/`), and prompts for working from a brief, iterating on feedback and renovating from photos (`packages/mcp/src/prompts/`).
- **High-level operations, not only primitives:** about a hundred operations such as `create-room`, `furnish-room`, `place-in-room`, `wall-opening`, `door-clearance`, `room-checks` and `verify-scene` ([agent-operations](https://github.com/pascalorg/editor/tree/51bf4071c10d0189c2499ec2069e580b74169106/packages/core/src/agent-operations)), each with fixture cases (`__fixtures__/*-cases.ts`) that act as tests of what an agent can achieve. Door clearance is checked for layout tools ("Furniture that overlaps a door clear zone is reported as blocking the door", [door-clearance.ts](https://github.com/pascalorg/editor/blob/51bf4071c10d0189c2499ec2069e580b74169106/packages/core/src/agent-operations/door-clearance.ts)).
- **The schema is the authority:** agent designs are validated against a published schema and rules, served as an MCP resource (A07 reads this in detail).

ifc-lite runs `ifc-lite mcp model.ifc` as an MCP server over stdio or HTTP and answers questions with `ifc-lite ask` (README L239). For Spare Construct, the lesson is that tools for agents should come in two layers: the command API, one tool per command, generated from the schema (map, Headless API), and a smaller set of task operations with their own test cases.

### Family recipes (A07)

Pascal's procedural items are JSON recipes with parameters, slots, parts and shapes, repetition by `count` and `index`, expression trees and size limits. A07 reads them in full and recommends the same shape, with formulas as strings instead of trees.

### Collaboration (A05)

ifc-lite has CRDT collaboration on IFCX (`@ifc-lite/collab`, `@ifc-lite/collab-server`; README L273). A05 recommends a server-ordered log for the model, with CRDTs for free text. ifc-lite's packages were not read for this note; how its CRDT handles constraints between elements is worth a look when A05's design is written (*unverified*).

## Recommendation

1. **Reuse from Pascal (MIT) only where it is TypeScript we would write anyway,** and only after a provenance check: interface patterns, MCP resource and prompt structure, and the idea of agent operations with fixture cases. Copied code keeps Pascal's notice and is listed in the pull request. Its core does not move into Rust as code.
2. **Use ifc-lite (MPL-2.0) as a reference and an oracle, not a source of code.** Its parser method and speed are the yardstick for our own IFC reader; its renderer's methods (split-precision camera-relative instances, spatial batches, contribution culling, device recovery) go into `engine-render`'s design note; and an unmodified ifc-lite build can sit beside IfcOpenShell as an out-of-process oracle in IFC tests. Nothing is copied into our files.
3. **Do not depend on ifc-lite in the core.** It would bring in an MPL-2.0 dependency with a bundled SMLib whose provenance the kernel handles under its own conditions, and an IFC reader is one of the parts ground rule 1 says to write ourselves for ownership.
4. **Take these into the pieces that own them:** A04's renderer design (methods above), Interop's reader benchmark (ifc-lite's figures as yardsticks) and truth tests (oracle comparison per element), and CON-A19's MCP design (resources, prompts, task operations with cases).

## Open risks

- **Both projects move fast.** ifc-lite pushes many times a day; Pascal too. Line links here are pinned to commits and will age.
- **Study, not audit.** The licence and provenance of Pascal's code were read from its LICENSE file only; anything copied needs a closer look.
- **ifc-lite's figures are its own.** Parse speeds and agreement rates come from its README and documentation; they become ours only when the interop benchmark runs them.

## Sources

| Source | Licence | Used for |
| --- | --- | --- |
| [Pascal editor](https://github.com/pascalorg/editor/tree/51bf4071c10d0189c2499ec2069e580b74169106) | MIT | Agent operations, MCP resources and prompts, recipes, stack |
| [ifc-lite](https://github.com/LTplus-AG/ifc-lite/tree/b085c65b50c4355e5d0db202f4f486f5703b48c9) | MPL-2.0 | Renderer methods, IFC parser method, oracle practice, MCP |
| [web-ifc](https://github.com/ThatOpen/engine_web-ifc) | MPL-2.0 | Pascal's IFC dependency; the kernel's treatment of MPL code |
| Kernel map, chapters [28](../../engines/geometry/architecture/28-testing.md) and [29](../../engines/geometry/architecture/29-sources-licences.md) | This repo | Oracles, MPL and SMLib rows |
| [K07](../../kit/notes/K07-licence.md) | This repo | Apache-2.0 for everything |
