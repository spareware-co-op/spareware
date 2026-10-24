# Vetting: CADCraft

*Evidence for [G01](../G01-geometry-kernel.md), the Spare Kit geometry kernel decision note. Written 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of each project at the pinned commits linked below. GPL and LGPL code is described in the agent's own words, never copied. Claims it could not confirm from a primary source are marked unverified.*

CADCraft (github.com/storytold/cadcraft, Apache-2.0) is an open AutoCAD-style 2D drafting app in Rust by the ArtCraft team, whom a Spareware owner is inviting under the Spareware umbrella. Each subsystem was evaluated against its actual source, then challenged by an independent skeptic. Read for G01 on 2026-10-21.

## overview

**Commit sha:** 7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1

**Findings:** All links use B = https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1. Line counts come from downloading every .rs file at this SHA through raw URLs (no clone), so they are measured, not estimated.

WHAT IT IS
- An AutoCAD clone for 2D drafting in Rust (egui). Its stated target is "Autodesk AutoCAD 2027" parity (B/ROADMAP.md#L5) and its stage is pre-alpha, about 38% "ready for real work" (B/ROADMAP.md#L3-L16).
- Repo metadata: created 2026-10-07, 6,158 stars, 2,714 forks, default branch main, version 0.10.0 (B/Cargo.toml#L7).
- The workspace is 12 library crates, 3 apps and xtask (B/Cargo.toml#L1-L26, B/docs/architecture.md#L11-L30).

SIZE: 193,684 lines of Rust in 618 .rs files, tests included (8.2 MB)
- crates/engine: 97,341 lines (commands, prompts, snaps, selection, undo, LISP interpreter, and most of the curve editing algorithms)
- crates/ui-egui: 29,979
- crates/io: 23,106 (DXF mapping, PDF, SVG and PNG)
- crates/doc: 10,689 (drawing database)
- crates/render: 9,514
- crates/geom: 5,031 (only 2.6% of the code)
- crates/fonts: 4,754
- crates/constraints: 4,283 (its own Levenberg-Marquardt solver)
- apps/cadcraft: 2,792
- apps/cadcraft-cli: 1,236
- crates/dxf: 1,171
- xtask: 1,097
- crates/mcp: 1,075
- crates/dwg: 791
- apps/cadcraft-web: 549
- crates/color: 276
- Growth: architecture.md measured about 57k lines on 2026-10-10 (B/docs/architecture.md#L11), so about 137k lines were added in 10 days.

INTERNAL DEPENDENCY GRAPH (from every Cargo.toml)
- Layers are enforced by `cargo xtask layers` (B/xtask/src/layers.rs#L35-L75, B/AGENTS.md#L31). Nothing below L5 may use egui, wgpu, winit or rfd.
- geom (L0): serde only (B/crates/geom/Cargo.toml#L11-L12).
- dxf and dwg: standalone.
- color (L0): serde.
- doc (L1): geom, color.
- constraints, fonts and render (L2): constraints uses geom and doc; fonts uses geom, skrifa, harfrust and unicode-bidi; render uses geom, color, doc, fonts, tiny-skia, lyon_tessellation, and rayon on native only.
- io (L3): geom, color, doc, dxf, dwg, render, fonts, miniz_oxide, quick-xml.
- engine (L4): geom, color, doc, dxf, constraints, fonts, render, craft-docpath (a git dependency) and rstar (B/crates/engine/Cargo.toml#L12-L29).
- ui-egui and mcp (L5); then the apps.
- The comments in layers.rs name crates this repo does not have ("raster", "compose", "gpu"; B/xtask/src/layers.rs#L56-L60). It looks like a template carried over from a sibling Craft app.

IS THE GEOMETRY KERNEL SEPARABLE? No. geom is a clean L0 crate, but it is not a kernel.
- It is 2D only: everything uses Vec2. Vec3 and Mat4 exist only to carry entity Z (B/docs/geometry-parity.md#L35).
- Its types are shaped by DXF:
  - An Arc is stored as a centre, a radius and start/end angles, so endpoints are recomputed with sin/cos (B/crates/geom/src/curve.rs#L55-L62).
  - A Spline is a 2D "DXF SPLINE style" curve that also carries AutoCAD's fit-point options (B/crates/geom/src/spline.rs#L6-L23).
- Core operations live in engine command modules, tied to `&mut Session` and the doc `EntityKind`:
  - offset_kind and offset_polyline (B/crates/engine/src/cmd/modify.rs#L568-L733)
  - trim (B/crates/engine/src/cmd/modify.rs#L867)
  - extend, fillet, break and join in the same file
  - trim_other and extend_other in modify2.rs
  - 2D region booleans (B/crates/engine/src/cmd/inquiry/regions.rs#L1-L7)
- The project's own audit says the "geometry kernel" is spread over crates/geom, the curve code in crates/engine/src/cmd, and crates/constraints (B/docs/geometry-parity.md#L7).
- The document model mirrors AutoCAD's entity list one-to-one: LwPolyline, Trace, Solid, Face3d, Region, Unknown raw DXF tags (B/crates/doc/src/entity.rs#L978-L1009).
- It has no topology, B-rep, surfaces, meshes, subdivision or implicits. 3D modelling is at 2%, with "300–450 h" estimated for the 3D kernel (B/ROADMAP.md#L34, B/ROADMAP.md#L78). The architecture doc confirms no solids, surfaces or meshes (B/docs/architecture.md#L43-L46).

AGENT AND HEADLESS ARCHITECTURE (the strongest part)
- Every action is a statically registered CommandSpec: id, label, menu, aliases, a free-text `params` doc, a JSON `run`, and an optional interactive prompt machine (B/crates/engine/src/cmd/mod.rs#L64-L78).
- `Session::execute(id, &serde_json::Value)` is the one entry point (B/crates/engine/src/lib.rs#L887).
- The command line, menus, scripts, CLI, JSON control channel and MCP all reach the same commands (B/AGENTS.md#L29-L30).
- MCP tools: command_line, script, execute, list_commands, inspect_drawing, query_entities, render, find_text, plus screenshot and ui_click when connected to the app (B/crates/mcp/src/tools.rs#L9-L40).
- Parameter schemas are free text, not typed.

BUILD TARGETS
- Native: macOS universal, Windows x64/x86/arm64, Linux x86_64/aarch64/riscv64, and FreeBSD (README lines 285-345).
- wasm32: the eframe web app, WebGPU with a WebGL2 fallback (B/apps/cadcraft-web/Cargo.toml).
- DWG is native only (B/crates/dwg/Cargo.toml#L11-L13).
- The wasm gate is only `cargo check` on L0–L4 plus ui-egui (B/xtask/src/main.rs#L121-L148). No tests run on wasm.

HISTORY
- First commit 2026-10-06T00:15Z (ebfebbd). 586 commits on main up to 2026-10-20.
- Cadence is in bursts: 198 commits on 10-10, 118 on 10-11, 109 on 10-13, 83 on 10-19, and 1–8 on most other days.
- The API lists 73 contributors. Top committers: vangel76 199, marcoarnulfo 67, MarekSpodBiedry 62, echelon (Brandon Thomas, the author) 53.
- 1,757 issues and PRs: 961 PRs (529 merged, 378 open) and 796 issues (352 open).
- AI share: commit messages carry 608 "Co-Authored-By: Claude Opus 5.5" trailers, plus 25 Sonnet, 15 Haiku and 7 Fable. 459 of 586 commit messages (78%) name an AI model.
- The first version was "built in one session (≈ 6.9 h elapsed, ≈ 11.5 agent-hours)" (B/ROADMAP.md#L132). In practice the whole codebase is agent-written, with humans steering.

RELATED REPOS
- getcadcraft is a different organisation (created 2026-10-10, getcadcraft.com, "CadCraft, BimCraft and six more"), not the storytold author.
- getcadcraft/cadcraft is the JavaScript product.
- getcadcraft/cadcraft-rust (SHA 3daa3406b70d010e2210f109b186f3f2d9480aaf) is a roughly 390k-line Rust port created 2026-10-15. It includes:
  - cc-manifold, a Rust port of Manifold 3.5.4 mesh booleans plus Clipper2, with output bit-equal to the wasm build (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/manifold/src/lib.rs)
  - ports of fdlibm, so maths matches V8
  - io-ifc, io-skp, its own io-dwg, and bim, civil and part crates
- It is mesh-based, not B-rep, and needs its own evaluation.

PARTNERSHIP SIGNAL
- Open issue #414 (https://github.com/storytold/cadcraft/issues/414) proposes "SolidCraft, a parametric 3D CAD craft on a shared B-rep kernel", as a craft-libs crate with CADCraft as its first user. It has a 200–400 h kernel spike with kill gates and would evaluate Truck first.
- No maintainer has replied yet.
- craft-libs (Apache-2.0) has no geometry crate today (it has ai-core, cms, docpath, i18n, media, pdf, print, settings and ui).

**Robustness exactness:** All arithmetic is plain f64. The project's own audit puts geometry at about 40% (B/docs/geometry-parity.md#L11-L17).

Tolerances
- One absolute `EPS = 1e-9` in drawing units (B/crates/geom/src/lib.rs#L32-L33).
- Ad hoc absolute and relative thresholds elsewhere:
  - 1e-14 relative in line_line_infinite (B/crates/geom/src/intersect.rs#L8)
  - 1e-9 parameter tolerance (B/crates/geom/src/intersect.rs#L19)
  - 1e-12·r² tangency (B/crates/geom/src/intersect.rs#L38)
  - 1e-12 after scaling in circle_circle (B/crates/geom/src/intersect.rs#L72-L82)
  - tessellation at 1e-3 and 1e-4 world units
- There is no tolerance context relative to the geometry, and no robust or exact predicates: the audit says "Robust predicates … none" (B/docs/geometry-parity.md#L31-L39).

Exact only for lines, arcs and circles
- Closed-form intersections are written with care for cancellation: the line–circle intersection is measured from the foot of the perpendicular, and circles are normalised by scale.
- Ellipse and spline intersections, closest points and offsets are all approximated:
  - The closest point on an ellipse comes from a polyline at chord tolerance 1e-3 × the major axis (B/crates/geom/src/curve.rs#L284-L288). On a 10 m ellipse that allows millimetre-scale snap error.
  - Offsetting an ellipse produces a tessellated polyline, and offsetting a spline produces a lossy 40-point refit (B/crates/engine/src/cmd/modify.rs#L586-L629).
  - Polyline offset joins neighbouring segments with no self-intersection or loop cleanup (B/crates/engine/src/cmd/modify.rs#L685-L733, B/docs/geometry-parity.md#L68-L77).
  - Intersections involving ellipses and splines use polygon approximations (B/docs/geometry-parity.md#L57-L66).
- Hatch and region boundaries are polygonised, with vertices merged on a grid at `tol` (B/docs/geometry-parity.md#L107-L115).

Failures and degeneracy
- 2D region booleans handle only non-crossing boundaries. Overlapping ones return "not available yet" (B/crates/engine/src/cmd/inquiry/regions.rs#L1-L7). That is an honest refusal, not a silent failure, but it is far from complete booleans.
- The never-crash policy:
  - crate-root deny lints on unwrap, expect and panic, and `unsafe_code = forbid` (B/Cargo.toml#L59-L60)
  - catch_unwind around every command (B/AGENTS.md#L14-L22)
  - capped loops and allocations
  - degenerate input returns None, not NaN or a panic (B/crates/geom/src/nurbs.rs#L1-L7)
- Robustness defects reported in issues:
  - #1326: FILLET picked the wrong tangent branch and overwrote the base circle, corrupting geometry (closed).
  - #926: circle_circle reported a false tangency for very different radii (closed).
  - #919: the constraint solver's absolute tolerance accepted unsatisfied angular constraints (closed).
  - #1690: HATCH fails to find a boundary that includes an ellipse (open).
  - #1602: far-off geometry produces 8e8-unit extents (open).
  - #1603: the web build panics with "capacity overflow" on large DXF files (open).
  - #166: extents pollution (open).
- The large-coordinate (UTM) behaviour is "untested" by their own audit (B/docs/geometry-parity.md#L38). Only the property tests check known answers at 1e6–1e7.

**Determinism:** Not designed for bit-identical results, and evidence shows it is not bit-identical across platforms.
- geom calls std f64 sin, cos, sin_cos, atan2, acos and hypot (B/crates/geom/src/vec.rs#L22, B/crates/geom/src/vec.rs#L44, B/crates/geom/src/vec.rs#L59, B/crates/geom/src/mat.rs#L32). Their precision depends on the platform's libm, so native and wasm32 results can differ.
- Arc endpoints are derived from stored angles, so every arc endpoint depends on those calls.
- There is no libm crate, no FMA or contraction policy, and no cross-target golden comparison.
- Issue #804 (https://github.com/storytold/cadcraft/issues/804): a hatch test fails on Windows/MSVC by 3e-16 against other platforms. It was handled by comparing with a tolerance (PR #762), not by making the maths deterministic.
- Display-list building uses rayon on native and is sequential on wasm (B/crates/render/src/lib.rs#L563, B/crates/render/src/lib.rs#L755). Whether the parallel output order is preserved is unverified.
- HashMaps in dxf_write.rs appear to be lookups only. Whether file output is deterministic is unverified.
- The wasm gate is `cargo check` only. No wasm tests run (B/xtask/src/main.rs#L121-L148).
- Contrast: the unrelated getcadcraft/cadcraft-rust takes determinism seriously, with fdlibm ports, no fused multiply-add, and a Manifold port that is bit-equal to its wasm build.

**Tests and quality:** Tests
- 2,212 #[test] functions: engine 1,052, ui-egui 315, io 309, render 142, geom 84, doc 65, fonts 62, apps/cadcraft 48, constraints 37, cli 25, dxf 18, mcp 18, color 4, dwg 4, web 1.
- geom has a property-based and hostile-input suite: a deterministic xorshift generator drives a few thousand cases per property, including NaN, infinities, subnormals and 1e300, with known answers at 1e6–1e7 (B/crates/geom/src/property_tests.rs, described in B/docs/geometry-parity.md#L129-L139).
- No cargo-fuzz targets and no criterion benchmarks. Real-file corpora live in separate repos that I did not check (B/AGENTS.md#L28).
- No numeric comparison against reference answers has been run: "No numeric comparison against AutoCAD has been run yet" (B/docs/geometry-parity.md#L21-L24).

CI
- The GitHub workflows are FreeBSD nightly build and test, packaging lint, nix eval, release, and windows-arm64. None runs `cargo test` or `xtask ci` on PRs.
- "Run cargo xtask ci on every PR" is still listed as upcoming work (B/ROADMAP.md#L108). The quality gate is local: fmt, clippy -D warnings, tests, assets, layers and the wasm check (B/AGENTS.md#L34).
- CodeRabbit and SonarCloud are configured.

Code quality
- `unsafe` is forbidden throughout (B/Cargo.toml#L59-L60) and the panic lints are denied.
- The layering is enforced mechanically.
- The code is breadth-first "presence quality": about 25 commands per agent-hour on the first build (B/ROADMAP.md#L38-L45).
- Many options were stubs at first and are being filled in through community PRs, 60 in one day (B/ROADMAP.md#L129).

Docs
- Unusually candid self-audits: architecture.md, geometry-parity.md, gaps.md and file-format-parity.md, each with file:line evidence and revision history.
- The plan/ folder (architecture, ADR 0001 "geometry libraries", AutoCAD observations) is gitignored and cannot be verified (B/AGENTS.md#L8-L12).

**Legal:** Project licence
- MIT OR Apache-2.0 (B/Cargo.toml#L9, B/NOTICE#L1-L4). GitHub detects only Apache-2.0.
- Either licence is compatible with an MPL-2.0 Spare Kit; keep the notices if anything is ported. The ArtCraft brand assets in docs/brand are excluded.

Dependencies: all 485 Cargo.lock packages checked on crates.io, 0 lookup errors
- Nothing is GPL, AGPL or LGPL-only.
- MPL-2.0:
  - acadrust 0.6.4, used at runtime for DWG read/write, native only, unmodified (B/crates/dwg/Cargo.toml#L11-L13, B/NOTICE#L12-L15).
  - colored, test-only (via egui_kittest → dify).
- Dual-licence options exist on self_cell (Apache-2.0 OR GPL-2.0-only) and r-efi (MIT/Apache/LGPL). Choose the permissive option.
- Other licences present: Unicode-3.0 (ICU4X crates), BSL-1.0, ISC, BSD-2/3, Zlib, and OFL-1.1 + Ubuntu-font-1.0 in egui's default fonts (UI only).
- geom itself depends only on serde (MIT/Apache).
- The git dependencies craft-ui and craft-docpath come from storytold/craft-libs (repo licence Apache-2.0; the per-crate licence fields are unverified).

Clean-room claims (owner must check against Spare Kit's stricter rule)
1. AutoCAD is installed on the dev machine and is "observed" black-box. Screenshots are kept in a gitignored folder, and UI work is compared against them (B/AGENTS.md#L25, B/AGENTS.md#L43). Behaviour mimicry of proprietary software is knowledge taken from Autodesk software, which Spare Kit forbids even though no code is copied.
2. xtask/data/menu-catalog.txt commits AutoCAD for Mac's menu tree verbatim by name, for parity counting (B/xtask/data/menu-catalog.txt). Command ids are AutoCAD's command names (B/AGENTS.md#L29).
3. DXF comes from Autodesk's published DXF Reference. That is acceptable as a public spec.
4. DWG is based on the Open Design Alliance specification (B/AGENTS.md#L25), which was itself produced by reverse-engineering Autodesk's format, plus acadrust. acadrust's provenance is not stated in its README or NOTICE (unverified). DWG must be flagged as an inside-knowledge format.
5. ACIS/SAT has "no public spec" by their own note, and REGION ACIS data is not written (B/docs/file-format-parity.md#L90).
6. Issue #40 raises a trademark name-change concern.

On the positive side
- The policy bans reading GPL, LGPL and AGPL sources (LibreDWG, LibreCAD, QCAD, OpenCASCADE, SolveSpace, planegcs) (B/AGENTS.md#L25).
- Asset attribution is enforced in CI (B/ATTRIBUTION.md#L1-L14).
- The geometry code cites textbook sources (Piegl & Tiller in B/crates/geom/src/spline_fit.rs#L1-L4; "textbook algorithms" in B/crates/geom/src/nurbs.rs#L6).

The getcadcraft repos are a different owner, and their io-skp (SketchUp, no public spec) and io-dwg would need separate legal review.

**Strengths**

- A clean, mechanically enforced crate layering: `cargo xtask layers` checks the dependency graph, and nothing below the UI layer may touch the UI toolkit. This is a good pattern for Spare Kit's own workspace (B/xtask/src/layers.rs#L35-L75).
- Everything is a command: one registry reached by the GUI, CLI, scripts, the JSON control channel and MCP, and programmatic calls never open dialogs. This is the same philosophy as Spare Kit's headless command API and MCP server (B/crates/engine/src/cmd/mod.rs#L64-L78, B/crates/mcp/src/tools.rs#L9-L40).
- A strict never-crash discipline: unsafe forbidden, panic lints denied, a catch_unwind guard that restores the drawing, hostile-input regression tests, and capped allocations (B/AGENTS.md#L14-L22).
- Unusually honest self-audit docs that separate exact operations from approximations, with file:line evidence (B/docs/geometry-parity.md).
- Permissive licence (MIT OR Apache-2.0), and the geom crate has a single dependency (serde).
- Very high velocity and community pull: about 194k lines in 14 days, 73 contributors, 529 merged PRs, 6.1k stars. This is a large potential pool of testers and contributors for a shared kernel.
- Native plus wasm builds on many platforms, with signed release packaging.

**Weaknesses**

- It has no geometry kernel: geom is a 5k-line, 2D-only, DXF-shaped curve library (arcs stored as angles, splines carrying AutoCAD fit options). There are no topology, B-rep, surfaces, meshes, subdivision or implicits.
- Core geometric operations (offset, trim, extend, fillet, join, region booleans) live in engine command modules, tied to `&mut Session` and the DXF entity enum. They cannot be extracted cleanly.
- Tolerances are absolute (EPS 1e-9 plus ad hoc thresholds), there are no robust predicates, and ellipse and spline intersections, closest points and offsets are tessellation approximations (closest point at 1e-3 × the major axis).
- It is not deterministic across targets: platform libm transcendentals, a documented 3e-16 cross-platform test failure (#804), and the wasm path is compile-checked only.
- Booleans exist only for 2D regions whose boundaries do not touch; overlapping ones are refused. 3D is at 2%, and the 300–450 h estimate for a 3D kernel is unrealistically low (#414 itself argues more).
- Code is effectively 100% agent-generated at breadth-first presence quality. There is no PR CI running the tests, no fuzzing and no benchmarks, and template residue from sibling apps shows (B/xtask/src/layers.rs#L56-L60).
- The product goal is AutoCAD mimicry, developed by black-box observation of AutoCAD, with AutoCAD's menu tree committed. This conflicts with Spare Kit's legal rule, and DWG goes through the ODA spec and acadrust, whose provenance is undocumented.
- The repo is two weeks old and its architecture is still being written: the plan/ folder and the ADR on geometry libraries are gitignored and cannot be verified.

**Verdict:** PARTNER

**Verdict reason:** Taking code from it is ruled out:
- No CADCraft code meets the bar for a decades-long kernel: geom is 2D, DXF-shaped, uses absolute tolerances, has no exact predicates or B-rep, is not deterministic, and its key operations are mixed into the command engine.
- So DEPEND and PORT are ruled out for the kernel, and its geometry methods are textbook or approximate. There is nothing to port that Spare Kit should not write itself from first principles.

Why partner anyway:
- The team needs exactly what G01 is building. Their 3D work sits at 2%.
- Open issue #414 asks for a shared, permissively licensed B-rep kernel crate with CADCraft as its first user. No maintainer has replied yet.
- They share the headless "everything is a command plus MCP" philosophy and use MIT/Apache licences that are compatible with MPL-2.0.
- Their community is large and fast-moving.
- A partnership where Spare Kit is the shared kernel and CADCraft one consumer of it is worth more than any of their code.

Conditions:
1. Collaborate at the kernel boundary only: CADCraft depends on Spare Kit, and nothing flows back from CADCraft's engine.
2. The owner must resolve the clean-room mismatch first: black-box observation of AutoCAD, the committed AutoCAD menu catalogue, and DWG via the ODA spec and acadrust.
3. Spare Kit's own rules on exactness, robust predicates, determinism and architecture apply without dilution.

If those conditions cannot be met, the fallback is STUDY: learn from its non-geometry engineering practices.

**What to take**

- STUDY: CI-enforced crate layering, a DAG table plus a ban on UI crates below a given layer, unit-tested on a model built from cargo metadata. Reuse the idea for Spare Kit's crate graph. https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/xtask/src/layers.rs#L35-L75
- STUDY: a single command registry with a JSON execute entry point shared by the GUI, CLI and MCP. Spare Kit should use typed, machine-readable parameter schemas instead of the free-text `params: &str`. https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/engine/src/cmd/mod.rs#L64-L78 and https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/engine/src/lib.rs#L887
- STUDY: the set of agent-facing MCP tools (command_line, execute, list_commands, inspect_drawing, query_entities, render, find_text) as a reference for the G01 MCP surface. https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/mcp/src/tools.rs#L9-L40
- STUDY: the never-crash policy (crate-root deny lints, a catch_unwind guard at the command boundary, hostile-input regression tests, capped allocations). https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/AGENTS.md#L14-L22
- STUDY: the hostile-f64 property-test harness idea (deterministic xorshift with NaN, infinities, subnormals, 1e300, and known answers at 1e6–1e7). https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/geom/src/property_tests.rs
- STUDY: the geometry-parity audit format (per operation: exact, sampled or missing, with file:line evidence) as a template for honest Spare Kit capability docs. https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/geometry-parity.md
- AVOID (anti-patterns to design out): arcs stored as angles (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/geom/src/curve.rs#L55-L62), an absolute global EPS (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/geom/src/lib.rs#L32-L33), tessellated closest-point and offset operations (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/geom/src/curve.rs#L284-L288, https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/engine/src/cmd/modify.rs#L586-L733), and geometry operations living in the command engine.
- PARTNER entry point: issue #414, the proposal for a shared B-rep kernel crate with CADCraft as its first consumer. Answer it with Spare Kit as that kernel. https://github.com/storytold/cadcraft/issues/414
- SEPARATE EVALUATION NEEDED (different owner): getcadcraft/cadcraft-rust. Its cc-manifold is a Rust port of Manifold 3.5.4 plus Clipper2, bit-equal to the wasm build, and it ports fdlibm for deterministic maths. This is relevant to G01's mesh layer and determinism. https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/manifold/src/lib.rs

### Challenge 1

**Refuted:** True

**Corrected verdict:** STUDY

**Errors found**

- FACT, determinism: PR #762 did not fix #804 by comparing with a tolerance. It removed a redundant clip in crates/render/src/hatch.rs (+11 -4), so the full-detail and view-limited calls now produce identical lines. The PR body also says MSVC's sin/cos round differently from glibc's and macOS's. That is direct evidence of cross-platform libm divergence, which strengthens the non-determinism finding, but the stated method is wrong. https://github.com/storytold/cadcraft/pull/762
- FACT, citation: B/Cargo.toml#L59-L60 contains only `unsafe_code = "forbid"`. The deny lints on unwrap, expect and panic are attributes on each crate root (for example B/crates/geom/src/lib.rs#L7, rule at B/AGENTS.md#L22). Those lints do not include clippy::indexing_slicing or arithmetic checks, and production geom code indexes raw despite the 'get() not [i]' rule (B/AGENTS.md#L18). Examples: B/crates/geom/src/spline.rs#L153-L191 (basis functions, and dense Gaussian elimination that panics on a non-square input).
- FACT, licences: Cargo.lock has 485 [[package]] entries, but only 467 are registry crates. The other 18 are 16 workspace members and 2 git sources (craft-libs at revisions 82a24b0 and 5c874d5, B/Cargo.lock#L935-L940), so '485 checked on crates.io' is overstated. My own crates.io audit of all 467 confirms the substance: 0 errors, nothing GPL, AGPL or LGPL-only, MPL-2.0 only in acadrust 0.6.4 and colored 2.2.0, and the dual-licence options self_cell (Apache-2.0 OR GPL-2.0-only) and r-efi 5.3.0/6.0.0 (MIT OR Apache-2.0 OR LGPL-2.1+).
- UNVERIFIED -> VERIFIED, with a new defect: the parallel display list keeps drawing order (par_chunks + collect, then append in order; B/crates/render/src/lib.rs#L556-L585). However, every rayon chunk shares one AtomicUsize block-expansion budget through a Relaxed fetch_add (B/crates/render/src/lib.rs#L478-L484, #L565-L571). Once MAX_BLOCK_EXPANSION is reached, which blocks get dropped depends on thread scheduling. The native screen list can then differ from run to run and from the sequential wasm path.
- MISSED, robustness: never-crash is often achieved by silently substituting a value, which produces plausible but wrong geometry instead of an error. Examples: Ellipse::closest returns the ellipse centre if tessellation yields nothing (B/crates/geom/src/curve.rs#L284-L288); norm_angle turns NaN or infinite angles into 0 (B/crates/geom/src/lib.rs#L38-L44); ccw_sweep turns any sweep ≤ 1e-9 rad into a full 2π circle (B/crates/geom/src/lib.rs#L47-L50); offset_polyline uses the unjoined start when two offset segments fail to meet (B/crates/engine/src/cmd/modify.rs#L699); offset_points falls back to (0,0) (modify.rs#L658). The evaluation's 'degenerate input returns None' holds for nurbs.rs, not in general. This philosophy conflicts with G01's 'never returns invalid geometry, never silently fails'.
- MISSED, wasm safety: the catch_unwind guard (B/crates/engine/src/guard.rs#L11-L100) does nothing on wasm32-unknown-unknown, where panics abort. #1603 is a web-module abort ('capacity overflow') from an uncapped `reserve(parts...sum())` (B/crates/render/src/lib.rs#L579-L581), which contradicts 'capped allocations'. A commenter on #1603 confirms the module dies with no recovery. https://github.com/storytold/cadcraft/issues/1603
- MISSED, determinism detail: Vec2::len uses hypot (B/crates/geom/src/vec.rs#L35), so every length and distance depends on libm, not just the trig calls. 'No FMA or contraction policy' is mostly moot in Rust: rustc never contracts floating-point operations implicitly, and geom has no mul_add. The cross-target risk comes from libm transcendentals and hypot.
- MISSED, test coverage: the praised property and hostile-input suite covers only lines, circles, arcs, bounds and vector identities (B/crates/geom/src/property_tests.rs#L96-L413). Ellipses, splines, NURBS operations, region tracing and offsets, which are exactly the approximated code paths, have no property tests.
- MISSED, robustness: region and hatch vertices are merged by rounding to grid keys (B/crates/geom/src/region.rs#L47-L52, #L141-L149), and pairwise segment intersections use an absolute 1e-9 parameter tolerance with no snap-rounding (region.rs#L117-L133, intersect.rs#L19). Two points 0.1·tol apart on either side of a bin boundary stay separate, while points about 1.4·tol apart inside one bin merge. The arrangement is therefore not topologically robust.
- MISSED, partnership and legal: #414's terms do not match Spare Kit. It specifies 'MIT/Apache', the kernel living in craft-libs (Apache-2.0), and 'Clean room: observe SOLIDWORKS black-box only'. Spare Kit is MPL-2.0 and forbids knowledge taken from proprietary software, and CADCraft's NOTICE treats MPL as a singled-out weak-copyleft exception (B/NOTICE#L12-L15). #414 has 5 comments, none from a maintainer (echelon, Brandon Thomas, has not replied), 11 days after it opened. https://github.com/storytold/cadcraft/issues/414
- MISSED, legal: CADCraft's requirements are written in terms of AutoCAD behaviour, including Autodesk's internal library model ('Relative to geometry (the ACIS/AcGe tolerance context)', B/docs/geometry-parity.md#L36, and AutoCAD columns throughout #L33-L127). Community bug reports specify 'EXPECTED BEHAVIOR (AutoCAD)' (#1326). Any kernel serving CADCraft as its first consumer would receive requirements derived from proprietary software, so the evaluation's condition that 'nothing flows back' cannot be enforced.
- MISSED, verifiability: CADCraft's governing standards repo storytold/craftrules (B/AGENTS.md#L5, #L15) returns 404, so it is private or unavailable. The geometry-library ADR in plan/ is gitignored. Neither the never-crash standard nor the library and licence decisions can be verified.
- MINOR: the MCP tool list omits new_drawing, open, save, cancel and ui_inspect (B/crates/mcp/src/tools.rs#L31-L40). The contributors API returns 72, not 73 (73 is the count of distinct commit authors). The search API shows 528 merged PRs, not 529. 10-12 had 33 commits and 10-06 had 19, not '1–8'. I counted 609 Opus and 458 AI-named commit messages against the evaluation's 608 and 459. None of these change any finding.
- MINOR, staleness of the self-audits the evaluation praises: architecture.md#L43-L44 says there are no regions, multilines or tolerance frames, but EntityKind has Helix, MLine, Region and Tolerance (B/crates/doc/src/entity.rs#L987, #L995, #L1002, #L1007). geometry-parity.md#L76 says arc bulges are not recomputed on polyline offset, but offset_polyline now recomputes them (B/crates/engine/src/cmd/modify.rs#L713-L725). spline.rs now carries end tangents, knot parametrisation and fit tolerance (B/crates/geom/src/spline.rs#L19-L22, spline_fit.rs#L1-L4), against geometry-parity.md#L50. file-format-parity still says acadrust 0.6.3, while 0.6.4 is the version in use.

**Argument:** B = https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1. I re-read the cited files at this SHA through raw URLs, pulled all 586 commits, queried the issues and PRs, and ran my own crates.io licence lookup on all 467 registry crates.

What holds:
- Size: 618 .rs files, 8.27 MB. geom is 5,031 lines with 84 tests.
- The layering table, the engine and io dependencies, Session::execute (lib.rs#L887), the CommandSpec with a free-text `params` field, and the MCP tools.
- 2D-only geom with angle-stored arcs and an absolute EPS = 1e-9.
- Tessellated closest point on an ellipse (curve.rs#L284-L288) and the lossy 40-point spline offset (modify.rs#L621-L629).
- Region booleans refuse overlapping boundaries (regions.rs#L1-L7).
- Platform libm calls (sin, cos, atan2, hypot) and the wasm gate being `cargo check` only (xtask/src/main.rs#L121-L148).
- No CI on pull requests: freebsd.yml and windows-arm64.yml run on PRs only when packaging files change.
- The ROADMAP figures (38%, 2% 3D, 300–450 h), the issue states, and the getcadcraft facts (the cc-manifold header confirms the Manifold 3.5.4 port bit-equal to the manifold-3d wasm build, and crates/part/src/fdlibm.rs exists).
- The finding 'not a separable kernel; DEPEND and PORT ruled out' stands and is reinforced.

What is wrong:
- The PR #762 method: it removed a redundant clip so the outputs became identical, not a tolerance comparison.
- The citation for the lint location, and the '485 on crates.io' count.

What was missed (pinned in errors_found):
- Never-crash works by silent substitution. Examples: the ellipse closest-point fallback returns the centre, NaN angles become 0, and a sweep ≤ 1e-9 rad becomes a full circle. This fails G01's 'never returns invalid geometry'.
- catch_unwind does nothing on wasm, where panics abort (#1603).
- A thread-scheduling-dependent block-expansion budget in the parallel render.
- Property tests cover none of the approximated curve types.
- Region tracing merges vertices on a rounded grid with no snap-rounding.

Arguing against PARTNER:
- Its central evidence, #414, asks for something Spare Kit is not: an MIT/Apache kernel in the Apache-2.0 craft-libs, built clean-room by black-box observation of SOLIDWORKS. Spare Kit is MPL-2.0 and bans knowledge taken from proprietary software. No maintainer has answered #414 in 11 days.
- CADCraft's whole requirement stream is AutoCAD behaviour, including the 'ACIS/AcGe tolerance context' in geometry-parity.md#L36 and bug reports titled 'EXPECTED BEHAVIOR (AutoCAD)' (#1326). A kernel whose first consumer is CADCraft would take in proprietary-derived requirements through every issue. The evaluation's condition that nothing flows back cannot be enforced, and its condition that 'the owner resolves the clean-room mismatch' would require CADCraft to change what it is.
- The governing standards, craftrules (404) and plan/, cannot be verified.

The evaluation already names STUDY as the fallback when its conditions cannot be met, and the evidence shows they cannot. For G01 the verdict is STUDY:
- the CI-enforced crate layering
- the single command registry and MCP surface, with typed schemas instead of free text
- the hostile-input test idea, extended to every curve type
- the honest audit format, without the AutoCAD column

An organisational relationship with the ArtCraft author is a separate owner decision and must not shape G01's requirements.

## geometry-2d

**Commit sha:** 7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1

**Findings:** Base for all links: B = https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1

PROJECT FACTS. Created 2026-10-07. At this SHA it has 586 commits and 73 contributors (vangel76 199, echelon 53). Version 0.10.0. AI agents write it at high speed: AGENTS.md is "instructions for agents", and the docs give estimates in "Opus 5.5 hours" (B/docs/geometry-parity.md#L11-L17). The 2D kernel is the crate `cadcraft-geom` (about 2k lines at bd2412f, about 5k now). Its only dependency is serde (B/crates/geom/Cargo.toml#L11-L12). Curve editing lives in engine/cmd (modify.rs, modify2.rs, curves.rs, trimextend.rs) and hatch filling in render/hatch.rs. The repo's own audit says: "geometry ≈ 40%… Ellipses and splines are handled by tessellating them… tolerances are absolute… Robust predicates: none" (B/docs/geometry-parity.md#L11-L17, #L33-L39).

REPRESENTATION
- Numbers: plain f64 `Vec2`/`Vec3` (B/crates/geom/src/vec.rs#L5-L10). One global absolute `EPS = 1e-9` (B/crates/geom/src/lib.rs#L33).
- Exact (analytic) curves:
  - Line (B/crates/geom/src/curve.rs#L5-L10).
  - Arc stored as center, radius and start/end angles, always CCW (#L55-L62). Endpoints are recomputed with cos/sin, so they do not equal the polyline vertices bit for bit.
  - Circle.
  - Ellipse in DXF form: center, major vector, ratio, parameter range (#L224-L232).
  - Polyline: `PolyVertex` holds a bulge plus start and end widths (#L413-L423). Bulge to arc conversion is in `bulge_to_arc` (#L434-L452).
  - Spline: a rational B-spline with knots, weights, fit points, closed flag and `FitOptions` (B/crates/geom/src/spline.rs#L6-L23).
- Document entities (B/crates/doc/src/entity.rs#L977-L1009): Line, Point, Circle, Arc, Ellipse, LwPolyline, Polyline3d, Spline, Helix, Ray, XLine, Text, MText, AttDef, Insert, Dimension, MLine, Leader, MLeader, Hatch, Solid, Trace, Face3d, Region, Viewport, Image, Wipeout, Table, Tolerance, Unknown.
  - Entities break down into `Prim` (Seg, Circle, Ellipse, Spline, Point, Infinite, Fill) for snapping, trimming and selection (#L1011-L1027).
  - Text, blocks and dimensions expose only insertion points as prims. Their geometry comes from the renderer.
  - A block reference (Insert) carries a 2D `Mat3` transform (#L480-L511).
- Hatch and region loops can only be bulge polylines (HatchLoop at B/crates/doc/src/entity.rs#L734-L744; Region at B/crates/doc/src/entity/region.rs#L1-L16). Ellipse and spline curves cannot be boundary edges:
  - DXF hatch ellipse and spline edges are tessellated when a file is read (B/crates/io/src/dxf_read.rs#L711 and #L767).
  - A hatch made by picking a point gets a flattened outer loop (zero bulges): B/crates/engine/src/cmd/hatch.rs#L212. Issue #1167 is open: a circle hatch boundary becomes 158 edges, with area 5025.22 instead of 5026.55.
- Clothoids are not supported (open issue #1357). There is no 3D kernel.

INTERSECTIONS (B/crates/geom/src/intersect.rs)
- Line–line uses a cross product with a relative parallel threshold of 1e-14·|r||s| (#L4-L14). Segment parameters get a fixed 1e-9 slack (#L17-L21). Collinear or overlapping segments return None.
- Line–circle measures from the foot of the perpendicular, which avoids cancellation on long lines. Tangency is decided by |δ| ≤ 1e-12·r² (#L24-L52).
- Circle–circle is scaled by the largest of d, r1 and r2, and h² ≤ 1e-12 is treated as tangent (#L55-L89).
  - Concentric circles (d == 0) return nothing, even when the circles are coincident.
  - Issue #926 describes this false tangency for circles with very different radii. It is closed with "no longer working on this", and the code at this SHA is unchanged.
- Arc filtering checks the angle sweep with a 1e-12 slack (B/crates/geom/src/lib.rs#L53).
- Pairs involving an ellipse or spline:
  - In snaps they are intersected as polylines: ellipse chords at 1e-3×major, spline chords at an absolute 1e-3 (B/crates/engine/src/snap.rs#L125-L145).
  - In TRIM and EXTEND the cutting edges are tessellated (ellipse at 1e-4×major, spline at an absolute 1e-4; B/crates/engine/src/cmd/modify.rs#L847-L864). The curve being trimmed is sampled (ellipse 720 samples, spline 512) and each hit is bisected 60 times on a side function (B/crates/engine/src/cmd/modify2.rs#L1883-L1932, #L1979-L1990).
  - In FILLET, curve offsets are sampled (720 samples for circles and ellipses) and refined by Newton's method with a finite-difference Jacobian (B/crates/geom/src/fillet.rs#L1-L8, #L33-L40, #L191-L280).
- Infinite lines are faked as long segments, with a different constant in each place:
  - ±1e7 in snaps (B/crates/engine/src/snap.rs#L129).
  - ±1e8 in trim and extend (B/crates/engine/src/cmd/modify.rs#L820, #L851, #L1071).
  - ±1e9 in selection (B/crates/engine/src/select.rs#L50).

SNAPPING (B/crates/engine/src/snap.rs)
- Supports OSMODE End, Mid, Cen, Nod, Qua, Int, Ins, Per, Tan, Nea, GCen, App, Ext and Par.
- Intersection snaps test every pair among at most 200 nearby prims (#L259-L294).
- Ellipse and spline perpendicular, tangent and mid points use 512 samples plus bisection (B/crates/engine/src/snap/curves.rs#L1-L13).
- Good point: under a non-uniform block transform, arcs become exact ellipses. `similarity_scale` (#L744-L751) and `xform_ellipse` (conjugate semi-diameters to principal axes, #L787-L803) handle this.

EDIT OPERATIONS
- TRIM and EXTEND.
  - Prompt handling, Fence, Crossing and Edge mode: B/crates/engine/src/cmd/trimextend.rs.
  - Line, arc, circle and polyline geometry: B/crates/engine/src/cmd/modify.rs#L866-L1118.
  - Ellipses and splines: B/crates/engine/src/cmd/modify2.rs#L1934-L2044.
  - Cut parameters are kept when they fall in (1e-9, 1-1e-9).
  - In Quick mode the object is erased when the error string contains "intersect" (B/crates/engine/src/cmd/trimextend.rs#L33-L52).
- OFFSET (B/crates/engine/src/cmd/modify.rs#L567-L733).
  - Lines, circles and arcs are exact.
  - An ellipse becomes an LWPOLYLINE built from 1e-3×major tessellation (#L586-L603).
  - A spline is tessellated at 1e-3, the points are offset, decimated to 40 and refitted (#L621-L630), so the result is lossy.
  - A polyline is offset per segment and joined at neighbour intersections. There is no cleanup of self-intersections or loops, and collapsed arcs are dropped. When a join point is not found it falls back to `s.start()`, which silently bends the previous segment (#L699).
- FILLET.
  - Line–line uses a closed form (#L1133-L1206).
  - Line, arc and circle pairs use a TTR built from offset intersections (B/crates/engine/src/cmd/curves.rs#L261-L282).
  - Ellipses and splines use `tangent_circles`.
  - Polyline corners: B/crates/engine/src/cmd/modify2.rs#L1795-L1869.
- CHAMFER: lines and polylines only.
- BREAK (B/crates/engine/src/cmd/modify.rs#L1460-L1564): line, circle, arc and polyline only.
- JOIN: greedy chaining with an absolute 1e-6 tolerance (#L1581-L1661). Splines join through exact NURBS work: degree elevation, rational conics and weight rescaling (B/crates/geom/src/nurbs.rs#L1-L7, #L360-L431).
- STRETCH moves the vertices that fall inside a window. ARRAY applies transforms.
- The NURBS toolkit follows the textbook: Boehm knot insertion, split, clamp, Bézier decomposition, elevation, adaptive Gauss–Legendre arc length and its inverse (B/crates/geom/src/nurbs.rs). Fit splines follow Piegl & Tiller chapter 9 (B/crates/geom/src/spline_fit.rs#L1-L4).

HATCH, BOUNDARY AND REGION FINDING (B/crates/geom/src/region.rs#L1-L11)
- Steps: polygonise every curve; split segments at crossings using a uniform-grid broad phase; merge vertices by rounding to tol-sized bins (#L47-L52); cast a +X ray from the pick point; trace the face by sharpest left turn using atan2 angles (#L235-L305). Islands are connected components after pruning dangling edges. HPGAPTOL closes gaps by joining nearest open ends.
- Coarse registration bug: a segment that spans more than 4096 cells is registered only on a coarse lattice (#L97-L107), so real intersections can be missed. Issue #929 is closed but not fixed, and the code is unchanged at this SHA.
- Curves are polygonised with a tolerance of drawing extents / 20000 (B/crates/engine/src/cmd/hatch.rs#L37-L60).
  - Result: on a 20 km site plan in mm the tolerance is about 2 m. `arc_segments` clamps the tolerance to r (B/crates/geom/src/lib.rs#L66-L78), so a circle with r ≤ 2 m becomes 2 segments and an r = 5 m circle becomes a square.
  - This is derived from the code and was not run.
- Issue #1690 (open): tessellation gaps where lines meet an ellipse give "Valid hatch boundary not found".
- Pattern lines are scanline families clipped even-odd. A family needing more than 20,000 lines is skipped silently (B/crates/render/src/hatch.rs#L266, #L425-L491, return at #L450-L452).

2D BOOLEANS
- UNION, SUBTRACT and INTERSECT only work for regions whose boundaries do not cross or touch. Otherwise they return the error "Combining regions whose boundaries overlap is not available yet." (B/crates/engine/src/cmd/inquiry/regions.rs#L1-L7, #L75-L132).
- Region area and MASSPROP use a shoelace sum over tessellated loops in absolute coordinates (B/crates/doc/src/entity/region.rs#L95-L120). A bulge polyline's area and centroid are computed exactly, relative to a local origin (B/crates/geom/src/curve.rs#L510-L581).

RELATED PROJECTS. getcadcraft/cadcraft and getcadcraft/cadcraft-rust (Apache-2.0) are a separate organisation, not the ArtCraft author.
- cadcraft-rust's cad-geom is a one-for-one port of a JavaScript geom.js, using f64 with EPS 1e-9 and ellipses cut into 72 steps (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/cad-geom/src/geom.rs#L19-L28). It is no better for this subsystem.
- It contains two things worth noting: jsmath.rs, a fdlibm sin/cos port that matches V8 bit for bit (…/crates/cad-geom/src/jsmath.rs#L1-L9), and a port of Shewchuk's orient2d (…/crates/civil/src/predicates.rs#L1-L5).

**Robustness exactness:** Arithmetic: plain f64 throughout. No exact or adaptive predicates and no interval or expansion arithmetic, which the repo itself states ("Robust predicates: none", B/docs/geometry-parity.md#L37). Tolerances are mostly absolute and ad hoc:
- EPS 1e-9 (B/crates/geom/src/lib.rs#L33).
- Parameter slack 1e-9 (B/crates/geom/src/intersect.rs#L19, #L103).
- Tangency 1e-12 relative (#L72-L80).
- Tessellation at 1e-3 or 1e-4 world units (B/crates/engine/src/snap.rs#L125-L145, B/crates/engine/src/cmd/modify.rs#L847-L864).
- JOIN fuzz 1e-6 (B/crates/engine/src/cmd/modify.rs#L1611).
- Cramer's-rule determinant 1e-14 (B/crates/engine/src/cmd/curves.rs#L284-L303).
- Hatch tessellation at extents/20000 (B/crates/engine/src/cmd/hatch.rs#L42).
- Pseudo-infinite lines at ±1e7, ±1e8 and ±1e9, so behaviour changes beyond those coordinates (snap.rs#L129, modify.rs#L851/#L1071, select.rs#L50).

Some care is taken:
- Line–circle measures from the foot of the perpendicular (intersect.rs#L24-L52).
- Circle–circle and three-point circles are normalised to the local scale (intersect.rs#L55-L89; curve.rs#L143-L167).
- Areas and centroids use a local origin (curve.rs#L510-L581).
- `Vec2::near` avoids overflow when squaring (vec.rs).

Near-tangent behaviour:
- Line and circle tangency are threshold decisions, so they depend on scale.
- For circles with very different radii, a real two-point crossing collapses to a single "tangent" chord foot that need not lie on the small circle (#926, closed with "no longer working on this", code unchanged: intersect.rs#L80).
- Ellipse and spline tangencies have no sign change, so sample-and-bisect misses them (modify2.rs#L1909-L1926). Two crossings within one sample step (1/720 or 1/512 of the domain) give zero or one hit.
- Fillet Newton uses a finite-difference Jacobian with 40 steps and an absolute determinant threshold of 1e-300 (fillet.rs#L191-L226). It can fail to converge at a double root, and the solution is then silently dropped.

Coincident and collinear behaviour:
- line_line returns None for parallel segments. Collinear overlaps are therefore never split in the planar arrangement (region.rs#L117-L139), so overlapping drafted lines produce wrong faces or no face. Only OVERKILL handles collinear overlap.
- Concentric coincident circles return no intersection.
- Vertex merging by rounding to bins is not transitive (region.rs#L47-L52).
- Face tracing compares atan2 angles with a 1e-12 threshold (region.rs#L235-L266).

Degeneracy and failure handling:
- The code never panics: clippy denies unwrap, expect and panic (B/crates/geom/src/lib.rs#L7-L8), and inputs are capped.
- Many failures are still silent or lossy:
  - A hatch pattern family over 20,000 lines is skipped (render/hatch.rs#L450-L452).
  - Intersection snaps consider only 200 prims (snap.rs#L263).
  - Boundary search takes only 64 ray hits (region.rs#L292).
  - The coarse broad phase drops pairs (#929 not fixed, region.rs#L97-L107).
  - Polyline offset bends a segment when the join fails (modify.rs#L699).
  - An ellipse becomes a polyline on offset, and spline offset decimates to 40 points (modify.rs#L586-L630).
- Region booleans fail honestly on overlap with "not available yet" (inquiry/regions.rs#L132). They are not wrong, but there are no real 2D booleans.

Failures seen in issues:
- #1690 (open): HATCH fails where lines meet an ellipse, because of tessellation gaps.
- #1167 (open): hatch area error from a flattened circle.
- #1014 (open): HATCH fails in complex DWGs.
- #1559 (open): fillet of a circle and a line reported as corrupting the circle.
- #929 and #926: real bugs closed without fixes.
- The issue tracker's "completed" state is unreliable.

**Determinism:** Determinism is not a stated goal. Nothing aims for bit-identical results across platforms, and no tests check it; the only "deterministic" hits in the repo are test RNGs.
- Geometry uses Rust std f64 sin, cos, tan, atan2, acos and hypot (e.g. B/crates/geom/src/vec.rs, curve.rs). Native builds call the platform libm (glibc, MSVC CRT, Apple libm), while wasm32 uses Rust's libm port. Last-bit differences are therefore expected between native and the WASM build (apps/cadcraft-web).
- Topology decisions depend directly on those values:
  - atan2 turning angles in region tracing (B/crates/geom/src/region.rs#L235-L266).
  - Threshold tests such as tangency at 1e-12 and sweep slack at 1e-12.
  - Sample-and-bisect root finding.
  So platform drift can change face structure or the number of intersections in near-degenerate cases.
- Run-to-run on one platform looks deterministic. HashMap and HashSet (RandomState) appear in region.rs, but the outputs are sorted or keyed by insertion order (#L140-L182, #L401-L403), and the work budget is independent of order. rayon is used only in render (B/crates/render/src/lib.rs#L563).
- No FMA control and no libm crate.
- Related project: getcadcraft-rust addresses this explicitly by porting fdlibm so results match V8 bit for bit (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/cad-geom/src/jsmath.rs#L1-L9). That technique is relevant to Spare Kit; their geometry is not.

**Tests and quality:** Tests:
- crates/geom has about 84 #[test] functions, including a property and hostile-input suite. It uses a fixed-seed xorshift generator with no external crate and checks NaN, ±inf, subnormals, 1e300 and known answers at 1e6–1e7 coordinates (B/crates/geom/src/property_tests.rs#L1-L50).
- About 200 engine integration test files cover the edit semantics, for example crates/engine/tests/trim_closed_polyline.rs, trim_extend_options.rs, offset_closed_spline.rs, offset_ellipse_side.rs, fillet_chamfer.rs, modify_curves.rs, snap_intersections_aperture.rs, snap_curves.rs, hatch_options.rs, region.rs and join_close.rs (tree at the SHA).
- No fuzz targets for geometry (no fuzz dir in the tree). No benchmarks of geometry accuracy. No cross-platform or WASM equivalence tests. No known-answer suite comparing against exact results for ellipses or splines (planned in B/docs/geometry-parity.md#L19-L24).

Code quality:
- `#![forbid(unsafe_code)]` and clippy deny unwrap, expect, panic, unimplemented, todo and unreachable in geom (B/crates/geom/src/lib.rs#L7-L8). Unsafe is forbidden workspace-wide.
- Every loop has a cap. `Session::execute` runs under catch_unwind. The quality gate `cargo xtask ci` runs fmt, clippy -D warnings, tests, layers and the wasm build (AGENTS.md).
- The code is readable, with doc comments on most functions.
- Every edit is a JSON `CommandSpec` that the CLI, the control channel and MCP all reach, so people and agents use the same API (B/docs/architecture.md).

Docs:
- B/docs/geometry-parity.md is a candid self-audit with file:line evidence. It is partly stale: it still says there is no fit tolerance, no HPGAPTOL, no helix and that bulges are not recomputed, while the code now has these.

Process:
- Velocity is extreme (586 commits in 14 days, 73 contributors, much of it AI-generated), and SonarCloud and CodeRabbit were only just added.
- Issues get closed as "completed" without fixes (#926, #929), so review depth is uncertain.

**Legal:** Licences:
- Workspace licence is "MIT OR Apache-2.0" (B/Cargo.toml#L8; B/NOTICE). The GitHub API reports Apache-2.0. Both are compatible with MPL-2.0 Spare Kit, whether depended on or ported with attribution.
- geom crate dependencies: serde only (MIT/Apache).
- engine adds rstar, serde_json, thiserror and log (MIT/Apache).
- render adds rayon (MIT/Apache), tiny-skia (BSD-3) and lyon_tessellation (MIT/Apache).
- io/dwg uses acadrust under MPL-2.0, unmodified (B/NOTICE). It is outside this subsystem.
- Git dependencies craft-docpath and craft-ui come from storytold/craft-libs. Their licences were not checked (unverified).
- No CLA file was found (unverified). There are 73 contributors.

Clean-room process: the owner must check this before reusing anything.
- AGENTS.md line 25 states that AutoCAD is installed on the dev machine and is observed black-box (run, used with synthetic drawings, screenshotted). Reading, disassembling or copying AutoCAD files is forbidden, GPL sources are not read, and formats come from Autodesk's published DXF Reference and the ODA's public DWG specification.
- Much of the edit behaviour is explicitly AutoCAD-observed:
  - B/crates/engine/src/cmd/modify.rs#L1489: "AutoCAD removes the CCW portion from the first to the second point".
  - CHAMFER refusal "as in AutoCAD" (#L1122-L1124).
  - Issue #1167: "AutoCAD 2027 for Mac, observed black-box".
- This is behavioural knowledge obtained by observing proprietary software. It is not code copying or disassembly, but Spareware's rule says "never use code or knowledge copied or reverse-engineered from proprietary software". The owner must rule on whether black-box observation of AutoCAD counts.
- The geometry mathematics itself is textbook and low risk: Boehm, Piegl & Tiller (B/crates/geom/src/spline_fit.rs#L1-L4), Gauss–Legendre, closed-form intersections (B/crates/geom/src/nurbs.rs#L1-L7; fillet.rs#L1-L8).
- The DWG path relies on the ODA DWG spec, itself a reverse-engineering product. Flag this, though it is not this subsystem.

AI provenance:
- The code was largely written by AI agents (AGENTS.md; "Opus 5.5 hours" estimates in docs; issues signed "Written by Claude Code"). Whether that code can be copyrighted, and so how strong the licence grant is, is uncertain. The owner should check.

getcadcraft/cadcraft-rust (a separate organisation, Apache-2.0):
- It has an io-skp crate, a SketchUp reader for a proprietary format. Flag it if anything from there is considered (unverified).
- Its orient2d port comes from robust-predicates, which is Unlicense.

**Strengths**

- Panic-free discipline: geom forbids unsafe and denies unwrap/expect/panic, every loop is capped, and hostile inputs are tested (B/crates/geom/src/lib.rs#L7-L8, property_tests.rs)
- Numerically careful details where it matters: line–circle measured from the foot of the perpendicular, circle–circle normalised to local scale, three-point circle in relative coordinates, shoelace/area/centroid about a local origin (B/crates/geom/src/intersect.rs#L24-L89, curve.rs#L143-L167, #L510-L607)
- Exact area and centroid for bulge polylines using the circular-segment formulas (B/crates/geom/src/curve.rs#L510-L581)
- Full textbook NURBS curve toolkit: knot insertion, split, clamp, Bézier decomposition, degree elevation, exact rational conic arcs, spline join with weight rescaling, adaptive Gauss–Legendre arc length and its inverse; plus DXF-style fit splines with knot parametrisation, end tangents and fit tolerance (B/crates/geom/src/nurbs.rs, spline_fit.rs)
- Exact affine image of ellipses and arcs under non-uniform block transforms (B/crates/engine/src/snap.rs#L744-L803)
- Planar face tracing finds islands made of separate lines and closes gaps up to HPGAPTOL, a good behavioural spec for hatch and boundary (B/crates/geom/src/region.rs)
- Region booleans refuse overlapping input with an explicit 'not available yet' instead of returning a wrong answer (B/crates/engine/src/cmd/inquiry/regions.rs#L132)
- Broad AutoCAD-class 2D command surface (295 commands), with every edit a JSON CommandSpec reachable from CLI, control channel and MCP, the same API for people and agents
- Candid self-audit document with file:line evidence (B/docs/geometry-parity.md)
- Large integration test corpus (about 200 engine test files) and a stream of real-world DWG bug reports, valuable as a regression oracle

**Weaknesses**

- f64 only, with no exact or adaptive predicates, as the repo itself admits (B/docs/geometry-parity.md#L37)
- Absolute, ad hoc tolerances everywhere (1e-9, 1e-12, 1e-14, tessellation at 1e-3 or 1e-4 world units, JOIN at 1e-6), so behaviour depends on scale
- Infinite lines faked as ±1e7 (snap), ±1e8 (trim/extend) and ±1e9 (select) segments, which is inconsistent and breaks at survey coordinates in mm (snap.rs#L129, modify.rs#L851/#L1071, select.rs#L50)
- Ellipse and spline intersections, snaps, closest points and cutting edges are all done on tessellations or samples. An intersection snap on an ellipse lands on a chord (snap.rs#L125-L145)
- OFFSET of an ellipse returns an LWPOLYLINE; OFFSET of a spline decimates to 40 fit points; polyline offset has no loop or self-intersection cleanup and silently bends a segment when a join fails (modify.rs#L586-L733)
- Hatch and region loops can only be bulge polylines. Picked hatch boundaries are flattened at extents/20000, so small circles in large drawings collapse to 2 to 4 segments; DXF ellipse and spline hatch edges are tessellated on import (hatch.rs#L42/#L212, dxf_read.rs#L711/#L767; #1167, #1690)
- No real 2D booleans: only regions whose boundaries do not touch can be combined (inquiry/regions.rs#L1-L7)
- Planar arrangement ignores collinear overlaps, merges vertices non-transitively by rounding to bins, decides turns by atan2 with 1e-12 thresholds, and its coarse broad phase can drop intersections (#929 closed but not fixed, region.rs#L97-L107)
- Near-tangent and double roots are missed by sample-and-bisect and by finite-difference Newton; false tangency for circles with very different radii (#926 closed but not fixed, intersect.rs#L80)
- Silent caps: a hatch pattern family over 20,000 lines is dropped (also in plots); intersection snaps consider only 200 prims; boundary search takes 64 ray hits
- Not deterministic across platforms (std libm transcendental functions; native and WASM differ); no tests for it
- Quick-mode TRIM decides to erase an object by matching 'intersect' in an error string (trimextend.rs#L33-L52)
- Arcs are stored as angles, and connectivity is decided by tolerance rather than shared exact vertices or topology
- No clothoids, no 3D kernel; Z is dropped in 2D operations
- Issue states are unreliable, development is extremely fast and AI-generated, and review depth is uncertain

**Verdict:** PARTNER

**Verdict reason:** Do not port or depend on any of CADCraft's 2D geometry code. Judged strictly against Spare Kit's goals it fails almost every one:
- f64 only, with no robust predicates.
- Absolute tolerances.
- Ellipses and splines handled by tessellation.
- Flattened hatch boundaries.
- No real 2D booleans.
- Silent caps and fallbacks.
- No cross-platform determinism.
- Known bugs closed without fixes.

As code it is STUDY-only. The partnership case is still strong and fits the PARTNER definition of building together instead of taking code:
- CADCraft's own roadmap lists exactly what G01 is building and marks it unstarted: exact ellipse and spline geometry, relative tolerances, correct offsets, and the 3D kernel at 300–450 h (B/docs/roadmap.md#L53, #L87; B/docs/geometry-parity.md#L141-L150).
- Issue #414 already proposes a shared B-rep kernel crate for the "craft" family.
- CADCraft brings what Spare Kit's kernel needs from outside: an AutoCAD-class 2D command layer (295 JSON commands over CLI and MCP), a large integration test corpus, a community of 73 contributors and real DWG failure cases (#1167, #1690, #1014). These make an acceptance oracle for an exact 2D kernel.

The right shape is that Spare Kit owns and designs the exact kernel, and CADCraft replaces its `cadcraft-geom` crate with it. CADCraft can take an MPL-2.0 dependency; it already ships acadrust under MPL. AutoCAD-observed command behaviour stays in the CADCraft app layer and out of Spare Kit.

Before any tests or semantics move into Spare Kit, two things need checking: the owner's ruling on black-box observation of AutoCAD, and the provenance of AI-generated code.

**What to take**

- STUDY B/crates/geom/src/region.rs#L1-L483 (planar face tracing, islands from loose curves, HPGAPTOL gap bridging) as a behavioural spec and source of test cases. Re-implement it on an exact arrangement with collinear-overlap handling and exact arcs and curves as edges
- STUDY B/crates/geom/src/curve.rs#L510-L581 (exact area and centroid of bulge polylines about a local origin), a small pattern Spare Kit's exact versions should match
- STUDY B/crates/geom/src/intersect.rs#L24-L52 (line–circle built from the foot of the perpendicular to avoid cancellation on long lines) as a reminder for the floating-point construction stage after exact predicates
- STUDY B/crates/engine/src/snap.rs#L744-L803 (similarity_scale and xform_ellipse: arcs under non-uniform block transforms become exact elliptical arcs via conjugate semi-diameters)
- STUDY B/crates/geom/src/nurbs.rs and B/crates/geom/src/spline_fit.rs as a feature checklist for Spare Kit's NURBS curve API: Boehm insertion, split, elevation, exact rational conic arcs, join with weight rescaling, arc length and its inverse, and DXF FitOptions (knot parametrisation chord/sqrt/uniform, end tangents, fit tolerance)
- STUDY the headless edit command schemas and modes (TRIM/EXTEND Fence, Crossing, Edge and Quick; FILLET/CHAMFER Trim/No trim and Polyline; OFFSET Through/Erase/Layer) in B/crates/engine/src/cmd/trimextend.rs and modify.rs#L17-L100 as a reference for Spare Kit's command and MCP surface. Legal check first, because the semantics were observed from AutoCAD
- STUDY B/crates/engine/tests/*.rs (trim_closed_polyline, trim_extend_options, offset_*, fillet_chamfer, modify_curves, snap_*, hatch_*, region, join_close) and B/crates/geom/src/property_tests.rs (hostile-input generator) as regression scenarios; recompute every expected value from exact mathematics
- Turn the observed failure modes into Spare Kit acceptance tests: #929 (broad phase drops pairs), #926 (disparate-radius false tangency), #1167 (flattened hatch circle area), #1690 (ellipse–line hatch gaps), pseudo-infinite lines at 1e7–1e9, hatch tolerance relative to extents, offset loop cleanup
- Related project only: STUDY getcadcraft/cadcraft-rust crates/cad-geom/src/jsmath.rs (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/cad-geom/src/jsmath.rs#L1-L9), a fdlibm port giving bit-identical transcendental functions, as a technique for Spare Kit's native and WASM bit-identity goal

### Challenge 1

**Refuted:** True

**Corrected verdict:** STUDY

**Errors found**

- Region area / MASSPROP is described wrongly. Region::area() adds the exact bulge area of each loop (B/crates/doc/src/entity/region.rs#L58-L61 calls Polyline::area(), which is exact about the first vertex, B/crates/geom/src/curve.rs#L510-L537), and RegionProps.area = self.area() (region.rs#L119). Only the centroid and second moments use a tessellated shoelace in absolute coordinates (region.rs#L98-L118, fine_tol at #L67-L71). Those moments are also divided by the tessellated area `a`, while the reported area is the exact one. B = https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1
- #926 and #929 are misread. The same outside auditor, PierrunoYT, filed both (they are part of the 35-issue audit #947) and closed both on 2026-10-18 with 'I'm no longer working on this'. 'completed' is GitHub's default reason when an author closes an issue. Fixes exist as OPEN, unmerged PRs: #976 (judges tangency against the smaller circle, with a regression test) and #975 (tests very long segments against every segment). So the issues do not show maintainers marking bugs done without fixing them, and they do not support 'the completed state is unreliable'. The real issue is fixes left unmerged.
- Missed evidence of weak review: there is no CI test gate. The GitHub check-runs and status APIs return nothing for the pinned SHA or for the head of PR #976. The in-repo workflows (freebsd.yml, windows-arm64.yml, nix.yml, packaging-lint.yml at the SHA) run only on packaging or nix paths, on the release branch, or on a nightly or weekly schedule. 'cargo xtask ci' is a local rule for agents (AGENTS.md L34), not enforced on PRs.
- 'About 200 engine integration test files' is wrong: the tree at the SHA has 164 files under crates/engine/tests/. The geom count of 84 #[test] is correct.
- '295 commands' is not supported. At the SHA, crates/engine/src registers 413 distinct CommandSpec::new ids. README L43 mentions 290 commands only as a historical calibration, and ROADMAP L22 measures 245/491 menu items. The source of 295 is unverified.
- Determinism: 'No FMA control' is not a real risk. rustc/LLVM never fuses a*b+c into an FMA unless mul_add is called explicitly, and crates/geom has no mul_add. The real cross-platform risk is std libm transcendentals and hypot (Vec2::len uses hypot, B/crates/geom/src/vec.rs#L34-L36), which the evaluation identifies correctly.
- getcadcraft jsmath.rs is overstated as 'bit-identical transcendental functions'. Its own header (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/cad-geom/src/jsmath.rs#L1-L9) says: tan, atan, atan2 and acos come from the `libm` crate; pow uses the platform powf, which may differ in the last bit on wasm; sin and cos above 823,549 fall back to libm. Its goal is to match V8. The technique Spare Kit can reuse is routing every transcendental through the pure-Rust `libm` crate or a correctly-rounded implementation, not this port. cadcraft-rust is also described by its authors as 'A test version' (3 stars).
- The craft-libs licence was marked unverified. The GitHub API reports storytold/craft-libs as Apache-2.0. The license fields of the individual crates (craft-ui, craft-docpath) were not checked.
- Missed a data-loss path. Quick mode is the default (TRIMEXTENDMODE defaults to 1: B/crates/engine/src/cmd/trimextend.rs#L19-L21, sysvars/table.rs#L74). In Quick mode an object is erased whenever its error text contains 'intersect' (trimextend.rs#L33-L52). These messages all match: 'Circle must intersect cutting edges at two or more points.' (B/crates/engine/src/cmd/modify.rs#L927), 'Ellipse must intersect…' (modify2.rs#L1948) and 'Spline must intersect…' (modify2/curve_edits.rs#L41,#L44). So a Quick-mode pick on a circle that meets a single tangent line, or a line ending on it, erases the whole circle: line_circle returns one point at tangency (intersect.rs#L43-L45). A sample-and-bisect miss on an ellipse or spline can erase it in the same way. Derived from the code, not run. What AutoCAD does in this case was not checked.
- Missed: Spline::tessellate(tol) does not bound chord error. It samples uniformly in parameter with n = 2·sqrt(control-polygon length / tol)·degree, clamped to 8–2000 and then to at most 4000 (B/crates/geom/src/spline.rs#L92-L102). The 'absolute 1e-3/1e-4' spline tolerances used in snaps, TRIM cutting edges, hatch and OFFSET are therefore nominal. Long or tightly curved splines are under-sampled and nothing reports it.
- The region vertex merge is worse than 'not transitive'. Each crossing point is rebuilt separately on each of its two segments (line.at(t) from that segment's own parameter, region.rs#L126-L131 and #L159-L162) and keyed by round((p-origin)/tol) with no neighbour-bin check (#L47-L52). Two copies an ulp apart that straddle a bin edge become two vertices, so the planar graph comes apart at that crossing.
- Missed pin: ellipses and closed splines are tessellated in hatches made by selecting objects too, not only on DXF import. closed_loop flattens a full ellipse at 1e-3×major and a closed spline at an absolute 1e-3 (B/crates/engine/src/cmd/hatch.rs#L97-L107).
- The PARTNER evidence is overstated. #414 is a third-party proposal (by lpgn) with no maintainer reply; the commenters are bajraan, Lion-1209, pwmaudio-maker and aiongg. It asks for the craft family's 'MIT/Apache' licensing, puts the shared kernel in craft-libs (Apache-2.0) and suggests evaluating Truck first. It gives no sign that CADCraft would adopt an MPL-2.0 Spare Kit kernel as its L0 crate. CADCraft's geometry-dependency policy lives in plan/adr/0001-geometry-libraries.md, which is gitignored (AGENTS.md L9, L12), and storytold/craftrules returns 404. The acadrust precedent is MPL but limited to DWG, 'used unmodified as a separate library' (NOTICE).
- The 'acceptance oracle' asset cannot be audited legally. AutoCAD observations are kept in plan/autocad/01-observed-ui.md ('measured observations of the running reference app'), and plan/ is gitignored (AGENTS.md L8, L12). The geometry docs target 'Autodesk AutoCAD 2027' (B/docs/geometry-parity.md#L3) and plan known-answer tests measured 'in both apps black-box' (#L21-L24). Where the expected values in the tests came from cannot be checked.
- Minor: #1559 is listed as a failure without noting that contributor Hamza0101 could not reproduce it on main 0b32c2e. The 20,000-line hatch family cap applies to full builds; a view-limited build allows 100,000 (B/crates/render/src/hatch.rs#L264-L270, #L449).

**Argument:** Most of the evaluation's technical findings hold when checked against the source at 7fef8cb:
- f64 only, with one EPS = 1e-9 and ad hoc tolerances.
- Exact line, arc and circle intersections, but with tangency decided by fixed thresholds.
- Ellipses and splines handled by tessellation.
- Flattened outer loops for picked hatches (#1167 reproduces: 158 edges, area 5025.22).
- The coarse grid broad phase.
- Booleans that refuse overlapping boundaries.
- Silent caps.
- No cross-platform determinism.
The 2 m tolerance on a 20 km plan and the collapse of small circles to 2 or 4 segments check out arithmetically (arc_segments clamps tol to r, and step = π gives n = 2).

Several stated facts are wrong or overstated:
- Region area is exact, not tessellated.
- #926 and #929 were self-closed by the auditor who filed them, and fix PRs #975 and #976 sit open. The tracker's state is not the problem; unmerged fixes and the lack of any PR CI gate are.
- 164 test files, not about 200.
- 413 command specs, not 295.
- FMA is a non-issue in Rust.
- jsmath.rs is not fully bit-identical.

The evaluation also missed defects that make the code harsher than stated:
- Quick-mode TRIM, the default, erases a circle, ellipse or spline that meets a cutting edge only once, because it matches 'intersect' in an error message.
- Spline tessellation tolerances are nominal, not chord-error bounds.
- The region merge can split coincident crossing points, which disconnects the graph.

On the verdict: PARTNER rests on three pillars, and the evidence weakens each one.

1. Issue #414 is an unanswered third-party proposal. It puts the shared kernel in craft-libs under MIT/Apache and suggests evaluating Truck first. That conflicts with the proposed shape, in which Spare Kit owns an MPL-2.0 kernel that replaces cadcraft-geom. CADCraft's dependency ADR is private (plan/ is gitignored) and craftrules is not public, so nothing shows the maintainers would accept this.
2. The roadmap gap is real, but CADCraft's own plan is incremental f64 patching: 'subdivision + Newton with a relative tolerance', costed at 90–140 Opus-hours. That is not an exact-predicate kernel effort, and no maintainer has asked for an external kernel.
3. The command layer is app-level. The test corpus and real-world failure cases are useful only as scenarios whose expected values must be recomputed from exact mathematics, as the evaluation itself says. That is a STUDY use. The behaviour spec behind those tests is AutoCAD black-box observation recorded in gitignored, unauditable files. Spareware bans knowledge 'copied or reverse-engineered from proprietary software', so treating CADCraft as a co-builder or oracle for Spare Kit's kernel risks exactly that contamination until the owner rules.

CADCraft also brings no kernel expertise. It has no robust predicates, no CI gate on PRs, much of its code is AI-written at very high speed, and known-good fixes sit unmerged.

For the geometry-2d subsystem, the verdict should therefore be STUDY: use it as a behavioural spec, a source of failure-mode tests and a NURBS feature checklist. Re-implement all of it on exact predicates, and import nothing whose expected values came from observing AutoCAD. Recruiting CADCraft as a downstream consumer of Spare Kit is an organisation-level decision the owner can pursue separately (the invitation already exists). It does not follow from this subsystem's quality, and it would need three things first:
- Maintainer agreement to take an MPL-2.0 L0 dependency.
- The owner's ruling on black-box observation.
- A CI gate on CADCraft's side.

## geometry-3d

**Commit sha:** storytold/cadcraft 7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1 (main, 2026-10-20, v0.10.0). Also read: getcadcraft/cadcraft 93b7f5eb58b94e726a5d5333dd792cca8f1d7a14, getcadcraft/cadcraft-rust 3daa3406b70d010e2210f109b186f3f2d9480aaf, getcadcraft/partcraft bdf4475db1e0a719e0de095da9725ba24248d2db. Open PRs: storytold #1005 head a0c55eabb6e319e161b93f87acc2f4d7b0a216be, #1675 head 8ec05d30df388e1d0cce1b2ecf3e3380ebf22fa9.

**Findings:** TWO DIFFERENT PROJECTS SHARE THE NAME. storytold/cadcraft (ArtCraft org, 6,160 stars, Rust, MIT OR Apache-2.0) is the one whose author is being invited. getcadcraft/* is a separate organisation: "The Data Design Group, Inc.", org created 2026-10-10 (https://github.com/getcadcraft/cadcraft/blob/93b7f5eb58b94e726a5d5333dd792cca8f1d7a14/NOTICE). It ships a JavaScript CadCraft and a Rust "test version" (cadcraft-rust, 3 stars). They share no code. Only the getcadcraft family has any 3D modelling.

A) storytold/cadcraft: NO 3D MODELLING EXISTS.
- The README says "2D and 3D CAD application" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/README.md#L13). The project's own docs contradict that. "3D geometry doesn't exist" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/geometry-parity.md#L11-L17). "Regions, 3D solids, surfaces, meshes (ACIS/B-rep, NURBS surfaces, subdivision) ... **no** (M11)" (same file #L55). The gap list says "F24 No 3D: UCS, 3D views, orbit, solids, surfaces, meshes, sections ... 300–450 h" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/gaps.md#L48). The roadmap puts "3D modelling and rendering" at 2% (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/ROADMAP.md#L34 and #L78). M11 is "≈ 2%" and 3D is scheduled "After beta ... solids per plan/adr/0001" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/roadmap.md#L87 and #L99-L100). plan/adr/0001 is not in the public tree, so the kernel plan is unverified.
- Kernel crate: `cadcraft-geom` is about 2,067 lines of f64 code for 2D drafting: lines, arcs, bulges, ellipses, rational B-spline curves, planar region tracing (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/architecture.md#L15). It has a Vec3 type and Mat4, but 3D is only used to carry Z on entities (geometry-parity.md#L35). The data model has 3D entities only as passive records: Polyline3d, Face3d, the 2D Solid/Trace (architecture.md#L39-L46). The geom crate depends only on serde, so there is no Truck, OCCT or Manifold (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/geom/Cargo.toml).
- Booleans: UNION, SUBTRACT and INTERSECT exist for 2D REGIONs only, and only when the boundaries do not cross or touch. Overlapping boundaries report "not available yet" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/engine/src/cmd/inquiry/regions.rs#L1-L7; gaps.md#L45).
- 3D I/O: 3DSOLID, BODY, SURFACE and MESH are kept as Unknown raw tags and written back (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/file-format-parity.md#L90-L91). ACIS SAB in the R2013+ ACDSDATA section is kept as opaque bytes (merged PR #781; https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/io/tests/acis_data_store.rs#L1-L4). Its own REGIONs are written as LWPOLYLINE plus xdata, because "ACIS has no public spec" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/io/src/dxf_region.rs#L1-L6). There is no STEP, IGES, STL, SAT or 3DS I/O (file-format-parity.md#L117; gaps.md#L86).
- Open, unmerged work is views only, not modelling. PR #1005 adds VPOINT, 3DORBIT, shaded styles and polyface meshes read/write/explode. Its projection code just maps entity points through Mat4 (https://github.com/storytold/cadcraft/blob/a0c55eabb6e319e161b93f87acc2f4d7b0a216be/crates/render/src/project.rs#L1-L3). PR #1675 integrates UCS and 3D navigation.
- Community proposals with no code: issue #414 "SolidCraft ... on a shared B-rep kernel" proposes a `craft-brep` crate in storytold/craft-libs, evaluates Truck first, and gates on booleans failing <1% of a corpus. Issue #1737 asks for STEP/STL import and mesh-to-CAD. storytold/craft-libs has no geometry crate (crates: ai-core, cms, media, pdf, print, ui, ...).

B) getcadcraft family: FACETED MESH 3D, no B-rep.
- JS CadCraft 3D tab (EXTRUDE with taper/path, REVOLVE, SWEEP, PRESSPULL, BOX, CYLINDER, WEDGE, UNION/SUBTRACT/INTERSECT, VIEWBASE flat views, GLB/OBJ/STL export) (https://github.com/getcadcraft/cadcraft/blob/93b7f5eb58b94e726a5d5333dd792cca8f1d7a14/app/commands3d.js#L1-L20). It runs on ModelCraft's SketchUp-style polygon kernel `m3d/` (PROVENANCE.md#L33). Curves are polygons with 48 steps per turn (https://github.com/getcadcraft/cadcraft/blob/93b7f5eb58b94e726a5d5333dd792cca8f1d7a14/app/shapes3d.js#L21). Booleans go through the vendored manifold-3d 3.5.4 wasm. Coordinates are recentred to the bounding-box middle, welded at 0.1 µm, sent to Manifold as Float32Array, then snapped back to a 1 µm grid (https://github.com/getcadcraft/cadcraft/blob/93b7f5eb58b94e726a5d5333dd792cca8f1d7a14/app/boolean3d.js#L43-L62 and #L68-L86). Help text: "curved faces stay made of flat facets", and only closed solids combine (https://github.com/getcadcraft/cadcraft/blob/93b7f5eb58b94e726a5d5333dd792cca8f1d7a14/app/help/3d-solids.md#L62). The 3D model is saved in DWG/DXF as JSON in xdata on POINTs in block CADCRAFT_3D, plus 3DFACEs on layer CC-3D for other programs (https://github.com/getcadcraft/cadcraft/blob/93b7f5eb58b94e726a5d5333dd792cca8f1d7a14/app/model3d.js#L22-L25). ACIS inside 3DSOLID, REGION and BODY is located and kept, not interpreted (https://github.com/getcadcraft/cadcraft/blob/93b7f5eb58b94e726a5d5333dd792cca8f1d7a14/app/dwg/acis.js#L1-L16).
- cadcraft-rust `cc-manifold` (~6.8k lines) is a pure-Rust port of Manifold 3.5.4's mesh boolean: boolean3/boolean_result, edge_op, face_op, polygon triangulation, collider, lazy CSG tree, hull, Minkowski, Extrude/Revolve/TrimByPlane, and a 2D CrossSection on a Clipper2 port (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/manifold/src/lib.rs#L1-L48). It does not port SDF, smoothing/refinement, Split, Slice or Project (#L23). It was validated bit-equal to the wasm on 11,093 cases, runs 1.80× the wasm speed natively and 1.59× as wasm32 (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/ROADMAP.md#L79-L81). Internal math is f64 with epsilon = max(1e-12·bbox, f32::EPSILON·bbox for single-precision input) (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/manifold/src/imp.rs#L20 and #L198-L205). Output is narrowed to f32 MeshGL (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/manifold/src/export.rs#L16-L38).
- `cc-m3d` is a one-for-one transliteration of ModelCraft's model.js: planar polygon faces/edges, push_pull, follow_me, soften, "sticky geometry". It matches V8 float-op order and V8 Math.* at zero tolerance (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/m3d/src/lib.rs#L1-L8; ops.rs#L87, #L1097). It uses an absolute EPS = 1e-6 m (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/m3d/src/vec.rs#L12). Solidity is checked by edge-face counts (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/m3d/src/solids.rs#L1-L27).
- PartCraft kernel: feature history rebuilt as mesh booleans on cc-manifold. Fillet, chamfer, shell and draft are built "approximately". Other exact-only features (sweep, loft, surfaces, STEP import) say "not built" (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/part/src/kernel.rs#L1-L10 and #L39-L40). STEP export is FACETED_BREP only, AP242 or AP214 (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/part/src/export.rs#L598 and #L699). Binary STL export exists (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/io-m3d/src/stl.rs#L1-L2). IFC import has a STEP Part 21 scanner (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/io-ifc/src/step.rs#L1-L4). It tessellates IfcAdvancedBrep NURBS and analytic faces with a Bowyer-Watson Delaunay in (u,v) (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/io-ifc/src/geo/nurbs.rs#L1-L12). That is tessellation for display, not a B-rep data structure.
- PartCraft dropped its OpenCascade exact kernel because OCCT is LGPL (https://github.com/getcadcraft/partcraft/blob/bdf4475db1e0a719e0de095da9725ba24248d2db/app/help/solid-kernels.md#L24-L26; exact-client.js). Its K.1 spike benchmarked truck 0.6 and monstertruck 0.4.1 against OCCT and Manifold on 10 closed-form parts. Both Rust kernels fail coplanar unions ("bracket", "bracket-touch") and the curved×curved "tee" at any tolerance. truck panics on one case and is 40–100× slower than OCCT. monstertruck's fillet chain fails, and 2 of 4 STEP files are invalid (https://github.com/getcadcraft/partcraft/blob/bdf4475db1e0a719e0de095da9725ba24248d2db/tests/tools/partcraft-test/k1-spike/REPORT.md#L3-L6, #L36-L66). It estimates its own future "cc-brep" at 50–60 days with booleans as the gate (#L108). No cc-brep exists yet.

Distance from a full B-rep kernel: storytold is at zero (no 3D topology, no surfaces, no 3D booleans). getcadcraft has a robust mesh boolean plus a polygon modeller, with no exact curves or surfaces in topology, no NURBS surfaces, no exact fillets, no STEP B-rep import or export, and no IGES. Both are essentially the whole distance from what Spare Kit G01 needs.

**Robustness exactness:** storytold: there is no 3D arithmetic to judge. The 2D kernel it would build on is self-described as using one absolute EPS = 1e-9 plus ad-hoc 1e-12/1e-14 and 1e-3/1e-4 world-unit tessellation steps, with "Robust predicates: none" and large coordinates "untested" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/geometry-parity.md#L35-L38; https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/geom/src/lib.rs#L32-L33). Region booleans refuse touching or crossing boundaries instead of solving them, an honest refusal rather than a silent failure (regions.rs#L1-L7). The open 3D-view PR's picking and snaps off the XY plane are "approximate" (PR #1005 body).

getcadcraft: Manifold's boolean guarantees manifold output through symbolic perturbation in floating point. It reports status codes rather than failing silently, but it is not exact. Tolerance is relative to the bbox, and single-precision input raises it to f32::EPSILON·scale, about 12 µm on a 100 m model (imp.rs#L198-L205). Output vertices are f32 (export.rs#L16-L38), which gives roughly 6 cm resolution at UTM coordinates without recentring. The JS works around this by recentring, welding at 0.1 µm and re-snapping to a 1 µm grid (boolean3d.js#L43-L86), which is a lossy round trip. Curved surfaces are 48-gon or "Curves"-setting facets, so volume errors of 0.002–0.16% are measured against closed forms (k1 REPORT.md#L36-L46). Fillet, chamfer, shell and draft are approximate on straight edges only (kernel.rs#L39-L40). cc-m3d uses absolute EPS = 1e-6 m and ad-hoc 1e-9/1e-12/1e-14 thresholds (m3d/src/vec.rs#L12 and nearby). The Rust port hardens degeneracy and hostile input: out-of-bounds, overflow and infinite-loop paths in the C++ become Status::Internal or ResultTooLarge, with input caps MAX_TRIS = 2^23 (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/manifold/src/lib.rs#L35-L45 and #L116). Failures seen in testing: in the K.1 spike, truck and monstertruck fail coplanar and curved booleans and truck panics on the tee case. Those failures belong to Truck, not CadCraft, but they are directly relevant evidence for G01.

**Determinism:** storytold: no 3D. The 2D kernel is f64 with no stated cross-platform bit-identity work. A wasm build exists (apps/cadcraft-web), but I found no native-vs-wasm golden comparisons for geometry (unverified absence).

getcadcraft-rust treats determinism as a primary design rule, and the techniques are directly relevant to G01:
- cc-manifold follows Manifold's sequential paths (MANIFOLD_PAR=OFF), never fuses multiply-add (-ffp-contract=off equivalent), and ports Manifold's deterministic msun acos/sin/cos/atan2. Output is bit-equal to the manifold-3d wasm on 11,093 cases, both natively and compiled to wasm32 (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/manifold/src/lib.rs#L26-L46; ROADMAP.md#L79-L81; tests/golden.rs#L1-L5).
- cc-m3d and PartCraft use the libm crate's fdlibm to match V8's Math.* bit for bit. They are checked against the frozen JS at zero tolerance over seeds 1–10 (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/ROADMAP.md#L69-L70; THIRD_PARTY.md).

Hazards:
- Manifold mesh IDs come from one process-wide counter, so results depend on global call order. The golden test must run all cases in one function, and the PartCraft rebuild must keep every solid alive to reproduce the wasm's CSG collapse (golden.rs#L7-L8; part/src/kernel.rs#L5-L9). This is reproducible but stateful, and unsafe under parallel use.
- The libc++ multimap::find order dependence is noted as version-sensitive (lib.rs#L46).
- Parity is with a JS/wasm reference, not with a mathematical definition. Determinism is "same as the old implementation", not "exact".

**Tests and quality:** storytold: no 3D tests because there is no 3D code. The 2D geom crate has property tests (crates/geom/src/property_tests.rs). `#![forbid(unsafe_code)]` and denies on unwrap/panic are in geom (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/geom/src/lib.rs#L7-L8). The ACIS passthrough has a synthetic-SAB test (crates/io/tests/acis_data_store.rs). Docs are thorough and self-critical (geometry-parity.md audits its own tolerances), but they drift: architecture.md#L43 still says no regions, although REGION landed in #510. The README overclaims "2D and 3D". All code is agent-written at high velocity (Claude Code attribution on commits and PRs).

getcadcraft-rust:
- cc-manifold has golden bit-parity tests (tests/golden.rs plus a 167 KB golden.json), hostile-input tests (tests/hostile.rs#L1-L3) and a cargo-fuzz target fuzz/fuzz_targets/manifold_bool.rs. It has `forbid(unsafe_code)` and denies unwrap, expect, panic, indexing_slicing, arithmetic_side_effects and lossy casts (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/manifold/src/lib.rs#L67-L85).
- cc-m3d has kernel_geometry.rs, kernel_model.rs and hostile.rs tests plus fuzz targets m3d_import, m3d_load and m3d_tools. PartCraft has part_regenerate and part_read fuzz targets. CI uses cargo xtask ci plus deny.toml.
- Weak point: the oracle is the old JS, so tests prove parity, not geometric correctness. The only closed-form geometric checks found are in the PartCraft K.1 spike. JS CadCraft's 3D test is tests/bin/test-cadcraft-3d.mjs.
- Code quality is high on safety lints, but the architecture is a transliteration of JavaScript: serde_json Value everywhere, f64 ids, JS truthiness helpers, V8 emulation. That is not a shape for a decades-long kernel.

**Legal:** storytold/cadcraft:
- Licence: MIT OR Apache-2.0 at the workspace level (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/Cargo.toml#L9). LICENSE-MIT and LICENSE-APACHE are present, and GitHub shows Apache-2.0. Both are compatible with MPL-2.0.
- Clean-room policy: AutoCAD is installed on the dev machine and observed black-box. "Never read, disassemble or copy anything inside the AutoCAD bundle." Formats come from Autodesk's DXF reference and the ODA DWG spec. GPL/LGPL sources are never read (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/AGENTS.md#L25). For 3D there is nothing to inherit.
- Flags for the owner: (1) the DWG path depends on acadrust 0.6.4 (MPL-2.0, unmodified) (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dwg/Cargo.toml#L11-L13). Its provenance was not checked here. (2) The ODA "Open Design Specification" is ODA's own reverse-engineered description of Autodesk's format. Under Spare Kit's rule that is format knowledge needing an owner decision. (3) ACIS SAT/SAB has no public spec, as the project itself states (dxf_region.rs#L1-L6), so any future ACIS reading would need inside knowledge. The current approach, opaque passthrough, is the clean option. (4) Black-box observation of AutoCAD may be subject to Autodesk's EULA. The owner should check.

getcadcraft:
- Licence: Apache-2.0 throughout. "CadCraft" is claimed as a trademark of The Data Design Group, and forks must rename (NOTICE). That may conflict with storytold's "CADCraft"; both claims are unverified.
- cc-manifold is a port of Manifold (Apache-2.0) with notices kept. It also ports Clipper2 (BSL-1.0), libc++ sort (Apache-2.0 WITH LLVM-exception), PhysX tri_dist (BSD-3-Clause), tbtSVD (MIT), disjoint-sets (Zlib) and msun (SunPro notice) (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/THIRD_PARTY.md#L38-L48). All are permissive and compatible as dependencies of an MPL-2.0 project. Ported Apache files must keep their Apache notices and cannot be relicensed to MPL. The crate is publish=false, so using it would mean a git dependency or a vendored copy.
- Flags: (1) The DWG/ACIS framing comes from the ODA spec plus "experiment" on AutoCAD sample files and files saved by the proprietary ODA File Converter, with GPL LibreDWG and libdxfrw binaries as black-box oracles (https://github.com/getcadcraft/cadcraft/blob/93b7f5eb58b94e726a5d5333dd792cca8f1d7a14/PROVENANCE.md#L19-L21; app/dwg/acis.js#L1-L9). This is knowledge derived by observing proprietary tools' output and needs owner review. (2) The SketchUp .skp reader (cc-io-skp) takes its format knowledge from OpenSKP (MIT), itself a reverse-engineering of a proprietary Trimble format (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/io-skp/NOTICE#L1-L8). Under Spare Kit's rule, do not take it. (3) The getcadcraft/cadcraft fixtures/folder/Cad contains real-looking .rvt and .rfa Revit files of 0.2–1.25 MB ("Tower Central.rvt", "Blank-2014.rvt"). These are likely files produced by Autodesk software and must not be copied into Spareware. (4) PartCraft's exact kernel was OpenCascade (LGPL) via replicad-opencascadejs, now removed from the release but still loadable via a localStorage dev key (https://github.com/getcadcraft/partcraft/blob/bdf4475db1e0a719e0de095da9725ba24248d2db/app/exact-client.js). It also runs OCCT as a test-only oracle in the K.1 baseline. Any brep*.js code in partcraft was written against OCCT's API and is LGPL-adjacent in design, so study only.

**Strengths**

- storytold is candid about the gap. Its docs state there is no 3D, no robust predicates and absolute tolerances, with file:line evidence (docs/geometry-parity.md). That makes it easy to see that CADCraft needs a kernel rather than has one.
- storytold keeps foreign ACIS solids byte-identical through DWG/DXF round trips: a 625-solid permit set kept all SAB bytes (PR #781). This is the right minimal behaviour for data it cannot model and a pattern Spare Kit should copy for unknown foreign geometry.
- Everything in storytold is a CommandSpec reachable from the command line, CLI, MCP and control channel. A Spare Kit 3D kernel could be wired in as commands (EXTRUDE, UNION, ...) without changing its interaction model.
- getcadcraft's cc-manifold is a pure-Rust Manifold 3.5.4 boolean, bit-identical to the wasm on 11,093 cases, faster than the wasm, with no unsafe code, panic- and indexing-denying lints, hostile-input caps and a fuzz target. It is a strong reference for deterministic, hardened mesh booleans.
- The getcadcraft K.1 spike is a reproducible benchmark of truck and monstertruck against OCCT and Manifold on closed-form parts. It is the best public evidence found that current Rust B-rep crates fail coplanar and curved booleans: exactly the failure modes G01 must solve.
- getcadcraft-rust's determinism discipline (no FMA, sequential paths, fdlibm transcendental functions, golden bit-parity tests) is a worked example of bit-identical native vs wasm32 output.

**Weaknesses**

- storytold/cadcraft has no 3D modelling at all: no solids, B-rep, NURBS surfaces, meshes beyond polyface records in an open PR, 3D booleans, extrude, revolve, sweep, loft or fillets, and no STEP, IGES or STL. The README's '2D and 3D' is aspirational.
- storytold's geometry foundation is tolerance-based (absolute EPS 1e-9, ad-hoc thresholds) with no robust predicates, and ellipse/spline intersections are tessellated. Building 3D on it would contradict Spare Kit's exactness rule.
- The storytold 3D plan (plan/adr/0001) is not public, so the planned kernel approach is unknown. The 300–450 h estimate is far below what a robust B-rep kernel costs; PartCraft's own estimate is 50–60 days before booleans even pass its spike.
- getcadcraft 3D is faceted only: curves are 48-gons or setting-based polygons, f32 mesh I/O, a lossy 1 µm re-snap, approximate fillets on straight edges only, and faceted STEP export only. There is no exact surface, NURBS surface, B-rep topology, STEP B-rep import or IGES.
- cc-manifold uses process-global mesh-ID state and needs call-order-sensitive object lifetimes to reproduce the wasm. It is pinned to Manifold 3.5.4 parity rather than tracking upstream, and is publish=false inside a 'test version' repo of a different organisation.
- getcadcraft-rust is a line-by-line JavaScript transliteration (serde_json Value, f64 ids, V8 emulation). Its tests prove parity with old JS, not geometric correctness. That is not the architecture for a decades-long kernel.
- Legal noise in the getcadcraft family needs owner review: knowledge derived by observing proprietary tools for DWG/ACIS, an SKP format reverse-engineered via OpenSKP, Revit files committed as fixtures, OCCT/LGPL history in PartCraft, and a competing 'CadCraft' trademark claim.

**Verdict:** STUDY

**Verdict reason:** Measured against Spare Kit's goals (exact B-rep with analytic and NURBS surfaces, booleans that never silently fail, exact/robust predicates, bit-identical native and wasm), there is nothing here to depend on or port for 3D. storytold/cadcraft, the invited author's project, has no 3D modelling code: its own docs put 3D at about 2% and leave the kernel unbuilt (M11, 300–450 h, ADR not public). The 3D that exists under the CadCraft name belongs to a different organisation (getcadcraft / The Data Design Group). That is a faceted mesh stack: a SketchUp-style polygon modeller plus Manifold mesh booleans, f32 mesh I/O, 48-gon curves and faceted-only STEP. It is a different architecture from an exact B-rep kernel. Some parts are worth learning from: cc-manifold's deterministic, hardened Rust port (useful when Spare Kit builds its own mesh-boolean subsystem), its golden bit-parity and hostile-input testing method, the K.1 spike's corpus and its measured truck/monstertruck failures, and storytold's opaque ACIS passthrough. A partnership with the ArtCraft author may be worth pursuing at project level, because CADCraft M11 and the community proposal in issue #414 (a shared Rust B-rep kernel, Truck-first, with a <1% boolean-failure gate) both need exactly what G01 builds. In that case CADCraft would use Spare Kit, not supply kernel code. Its 2D geometry (absolute tolerances, no robust predicates) shows no kernel method to adopt.

**What to take**

- Study (method, not code, for B-rep): the K.1 spike corpus and harness. Ten parts with closed-form volumes (plate with holes, coplanar bracket, bracket-touch, tee, chained fillets, ...) run across OCCT, Manifold, truck and monstertruck, with STEP read-back validation. Re-create an equivalent clean corpus as the first G01 boolean gate. https://github.com/getcadcraft/partcraft/blob/bdf4475db1e0a719e0de095da9725ba24248d2db/tests/tools/partcraft-test/k1-spike/REPORT.md#L17-L66
- Study the evidence that truck 0.6 and monstertruck 0.4.1 fail coplanar unions and curved-curved booleans at any tolerance, that truck panics on the tee case, and that 2 of 4 STEP outputs are invalid. Use this to argue against basing Spare Kit on Truck without re-testing. https://github.com/getcadcraft/partcraft/blob/bdf4475db1e0a719e0de095da9725ba24248d2db/tests/tools/partcraft-test/k1-spike/REPORT.md#L36-L66 and the spike crates at tests/tools/partcraft-test/k1-spike/truck/src/lib.rs
- Study cc-manifold's determinism and hardening techniques for Spare Kit's own mesh-boolean subsystem: sequential paths, no FMA, ported msun transcendental functions, bit-parity golden tests, UB-to-status mapping, input caps, and the deny lints for indexing and arithmetic side effects. Upstream Manifold (Apache-2.0) itself remains a candidate method for mesh booleans. https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/manifold/src/lib.rs#L1-L120 and tests/golden.rs, tests/hostile.rs, fuzz/fuzz_targets/manifold_bool.rs
- Study Manifold's epsilon model as a counter-example to avoid: a bbox-relative epsilon raised to f32::EPSILON·scale for single-precision input. Spare Kit should keep f64 or exact coordinates end to end. https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/manifold/src/imp.rs#L109-L111 and #L198-L205; export.rs#L16-L38
- Copy the pattern, not the code: keep foreign ACIS (SAT/SAB) bodies as opaque bytes bound to their entity handles and write them back unchanged. Do not interpret them, since there is no public spec. https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/io/tests/acis_data_store.rs#L1-L4 (PR #781) and dxf_region.rs#L1-L6
- Study the IFC route as a clean STEP-family source: an ISO 10303-21 lazy index/parser and NURBS/analytic face tessellation in (u,v) with Bowyer-Watson Delaunay, written from public IFC4/ISO specs. It is a reference for a Spare Kit STEP Part 21 reader and a surface tessellator test oracle. https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/io-ifc/src/step.rs#L1-L10 and crates/io-ifc/src/geo/nurbs.rs#L1-L12
- Note for project-level talks: storytold issue #414 (shared craft-brep kernel proposal with a spike gate) and CADCraft roadmap M11 (3D kernel unbuilt) are the opening to offer Spare Kit as CADCraft's 3D kernel. https://github.com/storytold/cadcraft/issues/414 ; https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/roadmap.md#L87
- Do NOT take: cc-io-skp (SKP knowledge reverse-engineered via OpenSKP), the getcadcraft DWG/ACIS framing derived from ODA File Converter and AutoCAD sample-file experiments, the Revit fixture files, PartCraft brep*.js (written against OpenCascade/LGPL APIs), or cc-m3d (a JS-transliterated polygon modeller with absolute EPS 1e-6).

### Challenge 1

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- cc-manifold size is wrong. The evaluation says about 6.8k lines. At 3daa3406 the crate's src/*.rs files total 13,279 lines, or 13,548 with tests/golden.rs and tests/hostile.rs. Without the Clipper2 port (engine.rs 2,263, sort.rs 578, mod.rs 338) it is still about 10.1k. Example file: https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/crates/manifold/src/clipper/engine.rs (2,263 lines).
- storytold geom size is stale. '~2,067 lines' comes from docs/architecture.md#L15, which was written from bd2412f on 2026-10-10. At 7fef8cbe, crates/geom/src totals 5,031 lines (nurbs.rs 782, spline_fit.rs 897, region.rs 714, curve.rs 629, ...). geometry-parity.md is also partly stale. It still says spline fitting has 'no knot parameterisation choice' (#L50), but geom/src/lib.rs#L29 exports FitOptions and KnotParam. The tolerance facts the evaluation relies on still hold (EPS = 1e-9 at lib.rs#L33).
- The lint claim is overstated. The evaluation says cc-manifold denies unwrap, expect, panic, indexing_slicing, arithmetic_side_effects and lossy casts. Per lib.rs#L67-L70, the last two are denied only on the input/output path files. Almost every algorithm module allows them with a module-wide #![allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_possible_wrap)]: boolean3.rs#L10, boolean_result.rs#L11, csg.rs#L18, edge_op.rs#L8, polygon.rs#L12, imp.rs#L7, clipper/engine.rs#L14 (all at 3daa3406).
- The wasm32 bit-parity claim is not supported. ROADMAP.md#L79-L81 reports identical output to the manifold-3d wasm on 11,093 cases. For wasm32 it reports only speed ('1.59× compiled to wasm32'). CI only runs `cargo check --target wasm32-unknown-unknown` (xtask/src/main.rs#L181-L193) and runs tests natively on Linux, Windows and macOS (.github/workflows/ci.yml). The committed golden test has 18 cases: golden.json has 18 'cases', with source 'forks/tools/mfdiff.mjs --golden'. forks/tools/ is not in the public tree, so the 11,093-case result is a self-report that cannot be reproduced from the repo. Native Rust matching the C++ wasm on the 18 committed cases is verifiable. Rust-as-wasm32 bit parity is unverified.
- The f32 precision at survey coordinates is understated. f32 spacing is 0.03 m at 5e5, 0.125 m at 1.7e6, 0.5 m at 5.9e6 (NZTM/UTM northings) and 1 m at 1e7. '~6 cm at UTM' applies only to values of roughly 0.5–1.05e6. Manifold's tolerance scale is Box3::scale(), the largest absolute coordinate, not the bbox size (crates/manifold/src/linalg.rs#L290-L293). So '12 µm on a 100 m model' holds only near the origin. Without recentring, a model at a 5.9e6 m northing gets a tolerance of about 0.70 m (imp.rs#L198-L205). This makes the evaluation's conclusion stronger, not weaker.
- f32 input is the only path. The evaluation describes single-precision input as one case ('for single-precision input'). In fact cc-manifold's only mesh API is f32: Mesh.vert_properties is Vec<f32> (lib.rs#L191-L204), and import always calls set_epsilon(-1.0, true) (import.rs#L208). Upstream Manifold's double-precision MeshGL64 path was not ported. Spare Kit could not feed it f64 coordinates without a rewrite.
- Missed legal and 3D point: storytold's own DWG dependency already interprets ACIS. acadrust 0.6.4 (MPL-2.0, crates/dwg/Cargo.toml#L13) ships a full ACIS SAT/SAB parser and writer with B-rep topology types (body→lump→shell→face→loop→coedge→edge→vertex) and primitive builders (box, cylinder, cone, sphere, torus): https://github.com/hakanaktt/acadrust/blob/8d5c46d423941ad96575760bf77979f8c46156fb/src/entities/acis/mod.rs#L1-L22 and primitives.rs#L1-L18. That is about 350 KB of code with no provenance statement. CADCraft itself says ACIS has no public spec (dxf_region.rs#L1-L6). Under Spare Kit's rule this needs an owner flag. The evaluation only said that acadrust's provenance was 'not checked'. Whether Spatial's old published SAT v7 format notes are a legitimate source is unverified.
- Conflicting evidence on the ACIS round trip. The evaluation quotes file-format-parity.md#L90-L91 as 'kept as Unknown raw tags and written back', but drops the same row's '**dropped by DWG save** (acadrust)'. That doc was updated on 2026-10-17, after PR #781 merged on 2026-10-13. The strength 'byte-identical through DWG/DXF round trips (625 solids)' rests only on PR #781's agent-written description of a private permit set. The project's own current doc contradicts it, so it is unverified.
- The planned K.1 kernel is in JavaScript. PartCraft's planned cc-brep is option (b), 'our own cc-brep in JS' (partcraft REPORT.md#L113, bdf4475d). The 50–60 'corrected days' are to match today's OCCT feature set (#L108). The evaluation presents cc-brep without saying it is a JS kernel for the JS apps, which matters if it is read as a possible Rust kernel.
- The mesh-ID hazard is mis-described. The evaluation calls it 'unsafe under parallel use', but MESH_ID_COUNTER is an AtomicU32 with fetch_add (imp.rs#L24-L30), so it is memory-safe. The real hazard is that numbering, and so result order, becomes non-deterministic when meshes are built concurrently.
- Missed point that strengthens the partnership note: CADCraft already accepts MPL-2.0 dependencies used unmodified (storytold NOTICE: acadrust 'used unmodified as a separate library'; crates/dwg/Cargo.toml#L12 cites plan/adr/0001). docs/roadmap.md#L99 schedules solids 'per plan/adr/0001', the same ADR. An MPL-2.0 Spare Kit kernel is therefore licence-feasible as CADCraft's 3D dependency. However, no maintainer (OWNER/MEMBER) has replied on #414: all five commenters are NONE or CONTRIBUTOR. #1737 got only a bot reply.
- Minor citations. VIEWBASE is not in commands3d.js#L1-L20; it is documented in app/help/3d-solids.md#L66-L68 (93b7f5eb). The 'straight edges between flat faces' limit for fillet and chamfer is in crates/part/src/kernel_edge.rs#L1-L3 and #L89, not kernel.rs#L39-L40. The Revit fixtures are 0.17–1.26 MB ('Wall shelf.rfa' is 167,424 bytes), not 0.2–1.25 MB.

**Argument:** The verdict holds after checking. The headline findings check out against source at the pinned SHAs:

- storytold/cadcraft has no 3D modelling. Evidence: geometry-parity.md#L15 and #L55, gaps.md#L48, roadmap.md#L87 (M11 at 2%), regions.rs#L1-L7 (2D region booleans that refuse touching boundaries), and an open PR (#1005) that adds views only. A search of the issue tracker finds no kernel work beyond proposals #414 and #1737, with no maintainer reply on either. storytold/craft-libs has no geometry crate.
- The getcadcraft family is a separate organisation, created 2026-10-10 (NOTICE confirms it). Its 3D is faceted: an m3d polygon modeller, 48-step curves, manifold-3d booleans, FACETED_BREP-only STEP export, and fillets that work only on straight edges. PartCraft dropped OpenCascade because it is LGPL. The K.1 report's numbers match the evaluation (coplanar and tee failures, the truck panic, 40–100× slower, 2 of 4 STEP files invalid).

The case for something harsher, REJECT, is real but falls short:
- Nothing in storytold can be studied for a B-rep kernel.
- The STUDY items come from a different organisation with legal noise. It has committed Revit files, uses the ODA File Converter as an oracle, reverse-engineered SKP via OpenSKP, and has OpenCascade history.
- cc-manifold is a parity port of JavaScript, pinned to Manifold 3.5.4. It takes f32 only in and out, with no MeshGL64 path. Its tolerance scales with absolute coordinate size, about 0.7 m at NZTM northings. Its mesh-ID counter is process-wide. Its algorithm modules allow unchecked arithmetic. Its 11,093-case parity claim cannot be reproduced from the repo: only 18 golden cases are committed, and wasm32 parity is never run in CI.
- The K.1 spike is about ten agent-built parts, run once. Upstream Manifold, which is Apache-2.0 and maintained, is the better thing to study for mesh booleans.

That still leaves things worth studying, which is a reason not to REJECT. The determinism techniques are concrete and applied: sequential paths, no fused multiply-add, ported msun functions, bit-level golden tests against a reference, and mapping undefined behaviour and hostile input to status codes with input caps and fuzzing. The K.1 corpus is a useful template for the first boolean gate. storytold's opaque ACIS passthrough is the clean pattern for foreign solids.

The case for something more generous, PORT of cc-manifold as Spare Kit's mesh-boolean layer, fails on the owner's 'best architecture' rule. The f32-only API conflicts with full scale. Global state conflicts with deterministic parallel use. Results depend on JS-parity object lifetimes. It is publish=false and tracks a frozen upstream.

PARTNER for this subsystem fails too, because there is no kernel on their side to co-develop. The partnership point stays a project-level note. It is now stronger, because CADCraft already accepts unmodified MPL-2.0 dependencies (the acadrust precedent, ADR 0001).

The supporting facts need correcting even though the verdict stands: crate sizes, lint scope, the wasm32 parity claim, f32 and tolerance magnitudes, the contradicted ACIS DWG round trip, and the missed acadrust ACIS parser, whose provenance is an owner flag.

Unverified: the contents of plan/adr/0001, which is not public; the 11,093-case harness; PR #781's real-file result; and acadrust's ACIS knowledge sources.

## formats-legal

**Commit sha:** 7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1 (storytold/cadcraft main, 2026-10-20). Also examined: acadrust v0.6.4 8d5c46d423941ad96575760bf77979f8c46156fb (the version in CADCraft's Cargo.lock) and HEAD daaa37a6d466d488d2ed401addf1ca8b50c6572a; getcadcraft/cadcraft 93b7f5eb58b94e726a5d5333dd792cca8f1d7a14; getcadcraft/cadcraft-rust 3daa3406b70d010e2210f109b186f3f2d9480aaf

**Findings:** REPO FACTS. storytold/cadcraft was created 2026-10-07, about 6,150 stars, release 0.10.0. The workspace licence is "MIT OR Apache-2.0" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/Cargo.toml#L9). GitHub reports only Apache-2.0 because it detects LICENSE-APACHE. The code is very largely agent-written: the docs estimate remaining work in "Opus 5.5 hours" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/file-format-parity.md#L9-L13).

DXF IS ITS OWN CODE. There are two layers.
(1) crates/dxf is a standalone L0 tag reader and writer. It reads ASCII and binary DXF and pre-2007 code pages, and decodes \U+ and \M+ escapes. It says it was "Written from Autodesk's published DXF Reference" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dxf/src/lib.rs#L1-L6). Its only dependencies are thiserror, log and encoding_rs (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dxf/Cargo.toml#L1-L14). It has a binary sentinel and a MAX_TAGS limit of 50M, checked before allocation (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dxf/src/lib.rs#L164-L183). Reals are written in Rust's shortest round-trip form (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dxf/src/lib.rs#L391-L398). Code-page tables are in https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dxf/src/codepage.rs#L1-L12.
(2) crates/io maps tags to the document model. This covers dxf_read.rs (108 KB), dxf_write.rs (116 KB), dxf_raw.rs (passthrough of unknown data), dxf_ext.rs, dxf_mleader.rs and dxf_table.rs. Comments cite the DXF Reference throughout. SPLINE keeps its exact knots, weights and fit data (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/io/src/dxf_read.rs#L339-L361). Bulges and hatch boundary bulges are kept too (dxf_read.rs L628-L692).
- Where the Reference is silent, CADCraft writes its own records rather than guessing Autodesk's: the CADCRAFT_ANNOTATION XRECORD (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/io/src/dxf_ext.rs#L762-L765) and CADCRAFT hatch xdata (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/io/src/dxf_write.rs#L1309-L1313).
- Elsewhere it writes things "as AutoCAD writes it" (dxf_write.rs L471-L473, L1391-L1393). That knowledge comes from looking at AutoCAD's output.
- Dynamic-block objects (ACAD_EVALUATION_GRAPH, BLOCK…PARAMETER and ACTION records, AcDbBlockRepBTag) are read, and changed visibility states are written "as AutoCAD writes them … checked with an AutoCAD dynamic block" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/file-format-parity.md#L99). These objects are not fully specified in the public DXF Reference, so this knowledge came from inspecting AutoCAD files: FLAG.
- The DXF writer emits R2000 structure under each version's $ACADVER, writes 36 header variables where AutoCAD writes about 280, and declares ANSI_1252 (file-format-parity.md L67, L92, L69).

DWG IS NOT ITS OWN CODE. crates/dwg is a 39 KB "bridge". It uses the acadrust crate (MPL-2.0, "used unmodified") to convert DWG to DXF bytes, which CADCraft's own DXF reader then parses. Saving runs the other way: CADCraft DXF, then acadrust DxfReader, then acadrust DwgWriter.
- Sources: https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dwg/Cargo.toml#L1-L13 and https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dwg/src/lib.rs#L1-L6.
- The read runs in acadrust failsafe mode inside catch_unwind. Skipped records are reported as notes (lib.rs L67-L145).
- Limits are 512 MB of DWG and 1 GB of DXF, enforced by a capped writer (lib.rs L24-L39, L97-L107). DWG text expands to about 6 times its size as DXF.
- The bridge patches acadrust bugs: table merges, table-style flags, dimension flag bits "(ODA DWG specification, common dimension data)", $DIMSTYLE and $TEXTSTYLE names, and layout owners (lib.rs L216-L300, L473-L545).
- On read it deliberately drops orphaned DGN line-style components and leaves a note (lib.rs L296-L307, issue #1482). On save it drops entities and objects acadrust cannot write (lib.rs L352-L355).
- It writes AC1015 to AC1032, at least AC1018 unless 2000 is asked for (lib.rs L10-L12, L449-L454).
- On wasm32 every DWG function is a stub that returns an error (lib.rs L556-L575). PR #1378 proposes enabling it.
- Documented state: DWG about 30% read and 15% write. Custom-class entities and objects, xdata on table records, and colour books are lost through DWG. There are no real-file tests (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/file-format-parity.md#L30-L59). Pre-R13 DWG cannot be read (L42).

ACADRUST PROVENANCE (the dependency CADCraft relies on), github.com/hakanaktt/acadrust, MPL-2.0, by Hakan AK. It is used by CADCraft and by the GPLv3 OpenCADStudio (https://github.com/hakanaktt/acadrust/blob/daaa37a6d466d488d2ed401addf1ca8b50c6572a/README.md#L237-L239).
(a) It is a Rust port of ACadSharp (C#, MIT, Albert Domenech).
- The early README said "Inspired by ACadSharp" (https://github.com/hakanaktt/acadrust/blob/db60e63bbfcc/README.md#L10, and still at https://github.com/hakanaktt/acadrust/blob/83cef0f26f8b/README.md#L10). The credit is gone from v0.6.4 and HEAD.
- Module headers now say "Based on the reference `DwgAppInfoWriter`" or "`DwgLZ77AC18Compressor`" (https://github.com/hakanaktt/acadrust/blob/8d5c46d423941ad96575760bf77979f8c46156fb/src/io/dwg/dwg_stream_writers/app_info_writer.rs#L20, https://github.com/hakanaktt/acadrust/blob/8d5c46d423941ad96575760bf77979f8c46156fb/src/io/dwg/compression.rs#L23). The same pattern appears in bit_reader.rs L7, header_reader.rs L7, merged_reader.rs L18 and file_header_ac18.rs L21 at HEAD.
- Those are ACadSharp class names (src/ACadSharp/IO/DWG/DwgStreamReaders/DwgHeaderReader.cs, DwgMergedReader.cs, DwgStreamReaderBase.cs and others). The folder layout is mirrored: file_headers/, dwg_stream_readers/, dwg_stream_writers/.
- The LICENSE file is MPL-2.0 only. A code search found no ACadSharp MIT notice in the repo.
(b) It was written with GPL-3.0+ LibreDWG source open. Checked at the locked v0.6.4:
- "Field layouts follow LibreDWG `dwg2.spec`" (https://github.com/hakanaktt/acadrust/blob/8d5c46d423941ad96575760bf77979f8c46156fb/src/objects/object_context_data.rs#L47).
- "Mirrors libredwg `dwg_resbuf_value_type`" (https://github.com/hakanaktt/acadrust/blob/8d5c46d423941ad96575760bf77979f8c46156fb/src/io/dwg/dwg_stream_writers/object_writer/objects.rs#L21), plus "(libredwg dwg.spec: HANDLE_VECTOR_N…)" at L1589.
- "field names follow libredwg's `acds.spec`" (https://github.com/hakanaktt/acadrust/blob/8d5c46d423941ad96575760bf77979f8c46156fb/src/io/dwg/dwg_writer.rs#L1975).
- LibreDWG's f256_power[] and rsgen[] tables were copied into tests, with the note "Source: LibreDWG/src/reedsolomon.c" (https://github.com/hakanaktt/acadrust/blob/8d5c46d423941ad96575760bf77979f8c46156fb/src/io/dwg/reed_solomon.rs#L697-L714).
- More at HEAD: "Per LibreDWG dwg.spec" and "per ODA spec / LibreDWG" (https://github.com/hakanaktt/acadrust/blob/daaa37a6d466d488d2ed401addf1ca8b50c6572a/src/io/dwg/dwg_stream_writers/object_writer/entities.rs#L5619-L5662); "the public libredwg spec" (same file, L4393-L4395); "LibreDWG's JUMP specification" (https://github.com/hakanaktt/acadrust/blob/daaa37a6d466d488d2ed401addf1ca8b50c6572a/tests/r12_entity_stream.rs#L12-L14).
(c) It also cites the ODA "Open Design Specification for .dwg files" by section (https://github.com/hakanaktt/acadrust/blob/daaa37a6d466d488d2ed401addf1ca8b50c6572a/src/io/dwg/crc.rs#L16-L19).
(d) It uses proprietary programs as oracles. Fields were "verified against ODA exports" (https://github.com/hakanaktt/acadrust/blob/daaa37a6d466d488d2ed401addf1ca8b50c6572a/src/entities/section_symbol.rs#L12-L14). Its R12 corpus was made in BricsCAD and checked with "AutoCAD 2027 AUDIT" (https://github.com/hakanaktt/acadrust/blob/daaa37a6d466d488d2ed401addf1ca8b50c6572a/tests/fixtures/r12/cad-created/README.md). DWG fixtures of unstated origin are committed (tests/fixtures/dynamic-blocks/*.dwg).
(e) Good point: it writes "Generated by acadrust" AppInfo and no TrustedDWG watermark (app_info_writer.rs L39-L74 at HEAD).

CADCRAFT'S "CLEAN-ROOM" DEFINITION. The only written rule is one paragraph (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/AGENTS.md#L25, identical in CLAUDE.md L25):
- AutoCAD is installed on the dev machine and may be observed black-box. Screenshots go to plan/autocad/screenshots and are never committed.
- Nothing inside the AutoCAD bundle may be read, except "names/listings".
- No copying of SHX, .pat, .lin, CUI/PGP, icons or help text.
- Formats come from the DXF Reference and "the Open Design Alliance's public DWG specification".
- "Never copy or link GPL/LGPL/AGPL code (LibreDWG, …) — don't even read their sources." The acadrust dependency breaks this rule (see b).
- Behaviour notes and the library ADR live in plan/ (plan/autocad/01-observed-ui.md, 04-menu-tree.txt, plan/adr/0001-geometry-libraries.md), and "plan/ is gitignored" (AGENTS.md L8-L12). The shared standards repo storytold/craftrules returns HTTP 404, meaning it is private. So the process records are not public and cannot be audited: UNVERIFIED.
- The parity docs describe listing the AutoCAD 2027 for Mac bundle's file and module names, and observing the running app (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/target-app-parity.md#L44-L56, https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/ui-parity.md#L20-L22, https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/file-format-parity.md#L17-L28).
- There is no two-team separation. The same agents observe AutoCAD and write the code.

THINGS THAT LOOK COPIED FROM AUTODESK.
(1) xtask/data/menu-catalog.txt is committed: 560 lines, the AutoCAD 2027 for Mac menu tree "recorded from the running AutoCAD" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/xtask/data/menu-catalog.txt#L1-L60, https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/target-app-parity.md#L53-L54).
(2) The standard linetype and hatch library claims "the dash and spacing values are our own" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/doc/src/library.rs#L1-L5). Yet its names, its description strings and its values match the widely published acad.lin and acad.pat entries:
- linetypes, e.g. "Border __ __ . __ __ . __ __ ." [.5,-.25,.5,-.25,0,-.25], CENTER [1.25,-.25,.25,-.25], "Fenceline circle ----0-----0----", "Zig zag /\/\/\/\/" (library.rs L16-L50);
- hatches, e.g. ANSI31 "Iron, brick, stone masonry" 45° 0.125, "Staircase effect", "Random dot and stone pattern" (library.rs L102-L228).
I compared these from general knowledge only, not against the Autodesk files, which must not be read. This needs owner verification. ATTRIBUTION.md L19-L20 and NOTICE L18-L21 assert originality. This looks like model recall of Autodesk content inside "clean-room" code.
(3) Command ids are AutoCAD's command names "in lower case" by rule (AGENTS.md L29). They include Express Tools names: BURST, TXT2MTXT, MOCORO, BREAKLINE (commit log). Prompts reuse AutoCAD wording, e.g. "Specify center point for circle" and "Specify first point on circle" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/engine/src/cmd/draw.rs#L757-L767). It also reuses system-variable names and AutoLISP.
(4) A "ViewCube" label appears in the menus (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/ui-egui/src/menus.rs#L40). I believe ViewCube is an Autodesk trademark (unverified).
(5) Good points: icons are drawn in code, the stroke font is original, test fixtures are synthetic (e.g. https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/io/tests/dynblock_read.rs#L1-L6), and CTB/STB/PC3 are not read because there is "no public spec" (file-format-parity.md L120). DGN is an owner decision (L116).

FORMAT DEPENDENCIES AND LICENCES.
- acadrust 0.6.4 is MPL-2.0 (provenance problems above). Its own dependencies are permissive: nom, byteorder, flate2, indexmap, foldhash (Zlib), encoding_rs, oem_cp, bitflags, itoa, ryu, memmap2, rayon, web-time (https://github.com/hakanaktt/acadrust/blob/daaa37a6d466d488d2ed401addf1ca8b50c6572a/Cargo.toml).
- CADCraft io uses miniz_oxide, quick-xml, encoding_rs, tiny-skia (BSD-3), lyon, skrifa and harfrust (Cargo.toml L37-L48; https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/io/Cargo.toml#L11-L28).
- craft-docpath and craft-ui come from git storytold/craft-libs, Apache-2.0 (Cargo.toml L28, L51).
- NOTICE names acadrust as the only weak-copyleft dependency (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/NOTICE#L10-L15).
- There is no cargo-deny licence gate. The tree has no deny.toml.

THE OTHER "CADCRAFT" REPOS. getcadcraft belongs to The Data Design Group, Inc., which claims "CadCraft" as its trademark (https://github.com/getcadcraft/cadcraft/blob/93b7f5eb58b94e726a5d5333dd792cca8f1d7a14/TRADEMARKS.md#L7). This is a different owner from storytold.
- It has its own DWG reader and writer "cc-dwg" (JavaScript, app/dwg/*.js), written clean-room from ODA spec v5.4.1, with a written PROVENANCE.md (https://github.com/getcadcraft/cadcraft/blob/93b7f5eb58b94e726a5d5333dd792cca8f1d7a14/PROVENANCE.md#L1-L27).
- LibreDWG dwgread, libdxfrw and the ODA File Converter are used only as black-box test oracles.
- cadcraft-rust ports cc-dwg to crates/io-dwg ("No DWG library's source was read") (https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf/NOTICE#L6-L7).
- Its SketchUp reader takes "format knowledge" from OpenSKP (MIT, itself presumably reverse-engineered: FLAG) (NOTICE L14-L16).
- It ports Manifold (Apache-2.0) (NOTICE L18-L37).
- Issue #40 notes that Autodesk partner "Cadcraft AB" exists. Issue #1554 reports a malware impersonator, cadcraft-app/cadcraft.

**Robustness exactness:** Arithmetic. DXF reals are parsed as f64 and written in shortest round-trip form, so DXF to f64 to DXF is lossless (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dxf/src/lib.rs#L391-L398). acadrust's DXF text writer also writes round-trip-exact reals: Display output, or {:.16e} outside 1e-15 to 1e15 (https://github.com/hakanaktt/acadrust/blob/daaa37a6d466d488d2ed401addf1ca8b50c6572a/src/io/dxf/writer/text_writer.rs#L89-L150). So the DWG to DXF-text to model bridge keeps doubles exact. It does pay a full text serialise and parse, about 6 times the DWG size (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dwg/src/lib.rs#L24-L39).

Silent value changes. Both writers turn NaN and infinity into "0.0" (dxf lib.rs L393-L395; acadrust text_writer.rs L94-L101). Value::as_f64 maps unparsable or non-finite input to 0.0 (dxf lib.rs L51-L58). Neither case is reported.

Degeneracy and hostile input.
- MAX_TAGS is checked before allocation (dxf lib.rs L164-L183).
- DWG input and output size caps use a capped writer (dwg lib.rs L97-L107).
- catch_unwind wraps the acadrust read and write (lib.rs L75-L79, L98-L99, L497-L516).
- Failsafe partial reads report skipped records by type, so "a partial read never passes for a complete one" (lib.rs L67-L145).
- Ranges read from the file are clamped, e.g. table merges (lib.rs L227-L245).
- Hostile-input tests: hostile_dxf_does_not_panic, hostile_extension_data_never_panics, damaged_dwg_opens_partially_and_says_what_it_left_out, oversized_dwg_and_dxf_are_rejected_early (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/io/src/tests.rs#L337, L966, L990, L1521).
- The project-wide rule is no panics and no unsafe (AGENTS.md L14-L22; Cargo.toml L59-L60).

Data loss is documented but still happens on save. Unknown DXF content is passed through raw (#528). Through DWG, custom-class entities and objects, xdata on table records, colour books and reactors are dropped (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/file-format-parity.md#L91-L98). Orphaned DGN components are dropped on read (lib.rs L296-L307). Unwritable unknowns are dropped on save (lib.rs L352-L355).

Failures seen in issues:
- #1076: a 47.8 MB DWG takes 16 to 28 s and peaks at 4.5 GB RSS.
- #1088: an 11.5 MB DWG exceeds the 1 GB DXF limit.
- #57: a 10 MB DWG took about 2 minutes.
- #1050–#1054, #1057, #1094 and #1650/#1651: dimension, text, style and layout data written so that AutoCAD shows it wrongly. These were found by black-box checks in AutoCAD.
- #1463: ZWCAD shows empty layouts.
- #1687: BricsCAD refuses CADCraft DWGs.
- #1581: XCLIP is ignored.
- #1179: fit-point splines from AutoCAD rebuild into a different curve.
The bridge compensates for several acadrust version-specific bugs by hand (lib.rs L216-L300).

There are no exact predicates. None are needed in this layer, since it only parses and serialises.

**Determinism:** DXF path. CADCraft's own tag writer uses Rust's float formatting, which is platform-independent and gives the same output natively and in wasm. Entity ordering in dxf_write was not audited: unverified.

DWG path. It is native-only. On wasm32 every function returns an error (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dwg/src/lib.rs#L556-L575). So native and WebAssembly behave differently by construction: the web build cannot open DWG at all (file-format-parity.md L51). PR #1378 would enable it, but catch_unwind does not catch panics under wasm's usual panic=abort, so a malformed DWG could kill the page.

acadrust determinism risks:
- CadDocument.objects is a std HashMap with a per-process random hash seed (https://github.com/hakanaktt/acadrust/blob/daaa37a6d466d488d2ed401addf1ca8b50c6572a/src/document.rs#L1236-L1243).
- The DXF writer sorts the remaining objects by handle (section_writer.rs L5751-L5752), but other paths iterate in hash order, e.g. values().find_map at L7438.
- It uses rayon on native.
- Bit-identical DWG and DXF output across runs and platforms is therefore UNVERIFIED and not guaranteed. No test asserts byte-stable output across processes.

getcadcraft/cadcraft-rust takes determinism more seriously. It carries fdlibm sin/cos copies so results match V8 (NOTICE L9-L12), and its io-dwg crate denies arithmetic side-effects and lossy casts (crates/io-dwg/src/lib.rs L9-L18). Not verified further.

**Tests and quality:** Tests:
- crates/io/src/tests.rs has 79 #[test]s, plus about 60 integration test files in crates/io/tests (round trips of dimstyles, mleaders, mlines, hatch associativity, xrefs, viewports, tables, codepages, DXF versions, DWG header variables, DWG layout owners, roundtrip_audit). There is also crates/dxf/tests/binary_write.rs and the CLI tests skipped_records.rs and undrawn_entities.rs.
- Every input is synthetic: inline strings or CADCraft's own output. "No test opens a file made by another program, and no external validator runs in CI". ezdxf audit was run by hand once (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/file-format-parity.md#L128-L135).
- A real-file corpus is a listed alpha blocker (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/gaps.md#L16-L17). Users have offered corpora (#1041, #1297, #1592).
- crates/io/tests/acad_interop.rs pins AutoCAD-observed expectations (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/io/tests/acad_interop.rs#L1-L4).
- There is no fuzzing (no fuzz/ directory in the tree). getcadcraft/cadcraft-rust does have fuzz/ targets.

Quality:
- Workspace unsafe_code = "forbid". Each crate denies unwrap, expect and panic.
- `cargo xtask ci` runs fmt, clippy, tests, the assets gate, the layers gate and the wasm build (AGENTS.md L31-L34).
- The `cargo xtask assets` gate enforces ATTRIBUTION rows. There is no cargo-deny licence or provenance gate.
- Docs are extensive and honest about gaps, with per-format parity tables and revision history. Some are stale: README says DWG write is "R2000" (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/README.md#L113) while the code writes 2000 to 2018, and file-format-parity says acadrust 0.6.3 while the lockfile has 0.6.4.
- Development pace is extreme: about 1,750 issues and PRs in two weeks, mostly merged in batches by the maintainer, and agent-authored. Review depth per change is UNVERIFIED.

**Legal:** Licences:
- CADCraft is MIT OR Apache-2.0 (Cargo.toml L9; README L387-L402). Copyright is "ArtCraft Team and the CADCraft contributors" with no CLA (NOTICE L1-L4). This is compatible with porting into MPL-2.0 Spare Kit if notices are kept.
- The ArtCraft brand assets are not free, and forks must remove them (docs/brand/LICENSE-brand.txt; README L398-L401).
- acadrust is MPL-2.0. That class of licence is allowed, but see the provenance problems below.
- craft-libs is Apache-2.0. The other format dependencies are permissive.

Clean-room evidence. There is one paragraph of rules (AGENTS.md L25). The detailed records (plan/, plan/adr/0001, plan/autocad/*, storytold/craftrules) are private or gitignored and cannot be audited. There is no two-team separation.

What a Spareware owner must check before any reuse or partnership:
1. acadrust's GPL exposure. It openly follows LibreDWG (GPL-3.0+) source: "Field layouts follow LibreDWG dwg2.spec", "Mirrors libredwg dwg_resbuf_value_type", acds.spec field names, and constant tables copied from LibreDWG/src/reedsolomon.c into tests (links in findings, v0.6.4). This breaks Spareware's "GPL never; LGPL study only" rule and CADCraft's own "don't even read their sources" rule. Never depend on or port acadrust.
2. acadrust's missing attribution. It is a port of ACadSharp (MIT): class names "Based on the reference DwgHeaderReader" and the like, and an earlier README credit that was removed. No ACadSharp MIT notice ships. That is an attribution defect that CADCraft inherits in its binaries.
3. DWG knowledge comes from reverse engineering. The ODA "Open Design Specification for .dwg files" carries ODA copyright and an informational-use disclaimer. Secondary sources (Library of Congress format description) say the ODA derived it by reverse engineering. LibreDWG is likewise reverse-engineered. Under the Spare Kit rule, DWG is a format that can only be read with knowledge reverse-engineered from proprietary software. FLAG it and get an explicit owner decision before any DWG code, whether from CADCraft, acadrust, getcadcraft cc-dwg or anyone else.
4. The process ran AutoCAD to copy its behaviour. AutoCAD 2027 was installed and observed to replicate its behaviour and record its menu tree. Autodesk's Acceptable Use Policy reportedly bars using its Offerings "to reverse engineer or replicate the functionality of any Offering", including AI training. I took this from a search snippet; the exact current wording is UNVERIFIED (the page returned 403). That is contract risk for whoever ran AutoCAD, and knowledge-taint risk for derived code. Issue fixes also rely on AutoCAD, BricsCAD, ZWCAD and the ODA File Converter as oracles, and an "Autodesk sample DWG" (#634).
5. A menu hierarchy recorded from AutoCAD for Mac is committed (xtask/data/menu-catalog.txt). It is a risk to the selection and arrangement of menus. Exclude it.
6. The linetype and hatch library repeats Autodesk text despite claiming originality. Its names, descriptions and dash values match acad.lin and acad.pat (my general-knowledge comparison, which needs verification by someone permitted to compare). That contradicts library.rs L1-L5, ATTRIBUTION.md L19-L20 and NOTICE L18. It is concrete evidence that AI-written "clean-room" code reproduces memorised proprietary content. Exclude it, or regenerate it independently with a recorded process.
7. Autodesk command names, prompts and names in the UI. Command ids are AutoCAD command names by policy, and prompts reuse AutoCAD wording. Express Tools command names are used. "ViewCube" (believed to be an Autodesk trademark) and "AutoLISP" appear in the UI. These are low copyright weight but are trademark and trade-dress questions.
8. Name conflict. "CADCraft" (ArtCraft), "CadCraft" claimed as a trademark by The Data Design Group, Inc. (getcadcraft TRADEMARKS.md L7), and Autodesk partner "Cadcraft AB" (issue #40) all overlap. A malware impersonator also exists (#1554). Resolve this before any branding or umbrella announcement.
9. Copyright in AI-generated code. Ownership and copyrightability of mostly agent-generated code, and the absence of a CLA, affect relicensing to MPL-2.0 and any warranties.
10. Hidden records. Ask the author for plan/adr/0001, plan/autocad/01-observed-ui.md and craftrules/standards before trusting the clean-room claim.

getcadcraft has better written provenance: PROVENANCE.md, a clean-room cc-dwg from ODA 5.4.1, GPL tools used only as black boxes, and SPDX/NOTICE discipline. It is still subject to items 3 and 4, plus the SKP format knowledge from OpenSKP. It is a different owner from the person being invited.

**Strengths**

- Its own permissive (MIT/Apache) DXF tag layer, crates/dxf, has no dependencies beyond thiserror, log and encoding_rs. It reads ASCII and binary DXF and pre-2007 code pages, handles \U+ and \M+ escapes, and checks size limits before allocating
- Doubles round-trip exactly through both CADCraft's and acadrust's DXF text writers (shortest round-trip formatting)
- SPLINE keeps its exact knots, weights and fit data. Bulge polylines and hatch bulge boundaries are kept exactly
- Unknown entities, objects, xdata, extension dictionaries and reactors are passed through DXF with handle remapping (#528), so data the program does not model survives a DXF save
- A partial read is never presented as complete: failsafe DWG reading reports skipped records by type with a warning on save, and limits are checked before the expensive steps
- No-panic and no-unsafe discipline, hostile-input regression tests, and catch_unwind isolation of the third-party reader
- Honest, detailed per-format parity documentation that lists every known loss and gap
- Written clean-room rule against Autodesk assets (icons, fonts, patterns, screenshots, AutoCAD-produced fixtures) and an attribution gate for assets in CI
- The architecture is headless and agent-first: every action is a command reachable from the CLI and MCP, which matches Spare Kit's command API and MCP goal

**Weaknesses**

- DWG is not CADCraft's code. It is a DWG-to-DXF-text bridge through acadrust, which costs about 6 times the size in memory, takes seconds to minutes on large files, and loses custom objects and some foreign data on save
- acadrust's provenance is tainted: it was written with LibreDWG (GPL) source open and copies LibreDWG constants, and it ports ACadSharp (MIT) without the MIT notice. This contradicts CADCraft's own clean-room rule
- DWG does not work in WebAssembly (it is stubbed out), so native and web builds behave differently; catch_unwind gives no protection under wasm panic=abort
- Deterministic output is not established: acadrust's document uses std HashMap with a random seed and rayon; there are no byte-stability tests
- NaN and infinity are silently written as 0.0, and unparsable reals silently read as 0.0
- Every file test is synthetic: no real-file corpus, no CI validator, no fuzzing
- The clean-room process is a single team observing a running AutoCAD. Its records (plan/, craftrules) are private, so the process cannot be audited
- Committed content echoes Autodesk: the AutoCAD menu tree, acad.lin/acad.pat names, descriptions and values despite the 'our own' claim, AutoCAD prompt wording, and the ViewCube label
- Knowledge of dynamic blocks and many 'as AutoCAD writes it' behaviours comes from inspecting AutoCAD output files, not from a public specification
- There are conflicting 'CadCraft' trademarks, and an Autodesk partner is named Cadcraft AB
- The DXF writer emits R2000 structure under later version numbers, writes only 36 header variables, and declares ANSI_1252

**Verdict:** STUDY

**Verdict reason:** Measured against Spare Kit's goals, this subsystem fails on legal provenance and on architecture.

On provenance: its DWG support depends on acadrust, which is built from GPL LibreDWG source knowledge and ships without ACadSharp's MIT notice. Its clean-room process relies on running AutoCAD to copy its behaviour, keeps its records private, and has already let Autodesk linetype and hatch text into code labelled "our own".

On architecture: DWG goes through a DXF-text bridge, cannot run in WebAssembly, has no proven deterministic output, loses data on save, and is tested only on synthetic files.

So DEPEND and PORT are out for DWG. For DXF, CADCraft's own MIT/Apache tag layer is clean in intent and licence-compatible. But it is small and R2000-structured, and Spare Kit should write its own DXF layer to its exactness and determinism standards, using CADCraft's limits, partial-read notes and unknown-data passthrough as design references.

PARTNER is not justified on this subsystem. The author's agent-first, everything-is-a-command approach does align with Spare Kit. But any partnership first needs owner checks 1–10: replacing acadrust, opening the private clean-room records, removing the Autodesk-derived content, and settling the name conflict.

**What to take**

- Study crates/dxf as a minimal, dependency-light DXF tag layer with size limits checked before allocation, binary-sentinel detection and code-page decoding: https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dxf/src/lib.rs#L164-L183 and https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dxf/src/codepage.rs#L1-L12. Reimplement it rather than copy it, and report non-finite values as errors instead of writing 0.0
- Learn the pattern that a partial import never passes for a complete one: failsafe reads with typed skip notes and a warning when saving over the original: https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dwg/src/lib.rs#L67-L145
- Learn input and output caps enforced before the expensive step, with a capped writer: https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dwg/src/lib.rs#L24-L39 and L97-L107
- Learn the unknown-data passthrough design: foreign entities, objects, xdata and reactors kept raw and written back with handle remapping (crates/io/src/dxf_raw.rs, crates/doc/src/foreign.rs; documented at https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/file-format-parity.md#L91-L100)
- Learn exact SPLINE import that keeps knots and weights and infers knot parametrisation for refits: https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/io/src/dxf_read.rs#L339-L361
- Use the per-format parity-table method (version-by-version read/write, what is lost, how it was tested) as a template for Spare Kit's format docs: https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/file-format-parity.md
- Use the clean-room and asset rules as a starting text, then tighten them: https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/AGENTS.md#L25-L26. Spare Kit should add two-team separation, no running of proprietary apps by code authors, a per-file provenance log, a dependency provenance audit (acadrust shows why), and a check for memorised proprietary content in AI output
- Use getcadcraft's PROVENANCE.md as a better example of written provenance and of how GPL tools may be used only as black-box oracles: https://github.com/getcadcraft/cadcraft/blob/93b7f5eb58b94e726a5d5333dd792cca8f1d7a14/PROVENANCE.md#L1-L27
- Do not take: acadrust (any version), xtask/data/menu-catalog.txt, crates/doc/src/library.rs linetype and hatch data, AutoCAD prompt strings, or any 'as AutoCAD writes it' knowledge (dynamic blocks and similar). Do not use the ODA-spec-derived DWG layouts without an explicit owner decision on DWG

### Challenge 1

**Refuted:** True

**Corrected verdict:** STUDY

**Errors found**

- ACadSharp notice claim is false. acadrust at the locked v0.6.4 ships ACadSharp's MIT licence, 'Copyright (c) 2021 Albert Domenech', at https://github.com/hakanaktt/acadrust/blob/8d5c46d423941ad96575760bf77979f8c46156fb/tests/fixtures/dynamic-blocks/LICENSE#L1-L3. The same notice is in tests/fixtures/dynamic-properties/LICENSE at HEAD daaa37a. The LICENSE and the fixture DWGs were added in one commit, 16644a6a617a. So both 'a code search found no ACadSharp MIT notice in the repo' and 'DWG fixtures of unstated origin' are wrong. What still holds: no notice covers the ported src/. The README credit was removed, the root LICENSE is MPL-only, and a GitHub code search for 'ACadSharp' returns 0 hits.
- Missed proprietary-API use in acadrust. A C# plugin built against AutoCAD's managed API (Autodesk.AutoCAD.DatabaseServices, CommandMethod) is committed and generates a committed fixture: https://github.com/hakanaktt/acadrust/blob/8d5c46d423941ad96575760bf77979f8c46156fb/tests/BlockNamesFixtures.cs#L1-L10 and https://github.com/hakanaktt/acadrust/blob/8d5c46d423941ad96575760bf77979f8c46156fb/tests/anonymous-names.md#L3-L4 ('a synthetic AutoCAD 2025 drawing generated by BlockNamesFixtures.cs'), producing tests/anonymous-names.dwg. This is stronger evidence than item (d) ('verified against ODA exports').
- The determinism finding understates acadrust. At v0.6.4 it has tests that assert byte-identical DXF and DWG writes across runs, each on a freshly built document. Each new std HashMap gets new RandomState keys, so hash-order dependence would fail these tests: https://github.com/hakanaktt/acadrust/blob/8d5c46d423941ad96575760bf77979f8c46156fb/tests/deterministic_output.rs#L42-L56. It also asserts that 25 repeated reads are equal (tests/issue51.rs L107-L121 at v0.6.4), and the writer sorts objects by handle for that reason (src/io/dxf/writer/section_writer.rs L5558-L5559 at v0.6.4). The cited find_map (HEAD L7438) selects the material whose xdictionary is the xrecord's owner, which is unique in valid files, so it is not shown to affect output order. What stays true: nothing tests cross-process or cross-platform bytes. CADCraft's own only 'stability' test compares entity counts: https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/io/src/tests.rs#L206-L213.
- 'Pre-R13 DWG cannot be read' is not supported by the code. is_dwg accepts any 'AC10'+2 digits, which includes AC1009 (R11/12) and AC1001–AC1006: https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dwg/src/lib.rs#L15-L17. The bridge already special-cases AC1009 documents (same file L362, L377). The locked acadrust 0.6.4 claims R12/R13/R14 DWG read and write (https://github.com/hakanaktt/acadrust/blob/8d5c46d423941ad96575760bf77979f8c46156fb/README.md#L68-L74) and ships src/io/dwg/r12/. The evaluator repeated a stale doc row (file-format-parity.md L42). Whether R12 reading actually works is UNVERIFIED.
- Hatch exactness is misstated. Only polyline-type loops keep their bulges. Edge-type loops are converted: arc edges become recomputed endpoints plus a bulge, elliptic-arc edges are tessellated at major-axis×1e-3, and spline edges are tessellated at a fixed absolute 1e-3: https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/io/src/dxf_read.rs#L646-L770. The model can only hold point+bulge vertices: https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/doc/src/entity.rs#L736-L744. So exact ellipse and spline hatch boundaries are lost on import and saved back as point chains, with a scale-blind tolerance. The evaluator cited L628-L692 as 'kept'.
- SPLINE is not kept exactly in general. Control and fit points are Vec2 (dxf_read.rs L36-L40; https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/geom/src/spline.rs#L8-L16), so the Z of a 3D spline is dropped. Degree is clamped to 1..10 (dxf_read.rs L340). An invalid spline is silently refitted from its fit points, or rebuilt from its control points with new knots, and no note is given (dxf_read.rs L355-L358). Other silent repairs: ELLIPSE ratio clamped to [1e-9,1] (L273), arc radius abs() (L260). The DXF model is planar 2D only.
- The 'limits checked before the expensive steps' strength is overstated. Only the DWG input size is checked first (dwg lib.rs L72-L74). acadrust's full decode into a CadDocument (L75-L79) has no memory bound. The 1 GB DXF cap applies only while the already-built document is serialised (L97-L107), and the resulting text is then parsed into Vec<Tag>. Issue #1076: a 47.8 MB DWG peaks at 4.54 GB RSS. A 512 MB DWG passes the input cap.
- The quality gate is not enforced in CI. `cargo xtask ci` is run locally or by agents. No GitHub workflow runs the Rust tests on PRs or on pushes to main. freebsd.yml runs only on the release branch, nightly, and on packaging-path PRs (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/.github/workflows/freebsd.yml#L5-L14); nix.yml only evaluates the flake. gaps.md says so itself: 'No CI runs the Rust tests on pull requests … open PRs land without a gate; regressions go unseen' (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/docs/gaps.md#L22). So the hostile-input regression tests are not a merge gate for about 1,750 batch-merged agent PRs.
- DWG losses are worse than stated, and there is real-file evidence. Issue #1041 ran a 23-DWG real corpus against the ODA File Converter and BricsCAD. BricsCAD V26 opens 9/23 CADCraft DWGs. Through DWG save: 3DSOLID 2580→5, SORTENTSTABLE entries 2680→0, annotative context 957→0, DIMASSOC 1034→50, custom-class objects 55,639→17,143. 'Loses custom objects and some foreign data' understates this. Contributors also use the proprietary ODA File Converter as the reference.
- Silent value changes were missed. Code-page decoding turns invalid bytes into U+FFFD and reads Johab and other DOS code pages as ANSI_1252 (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/dxf/src/codepage.rs#L8-L12; file-format-parity.md L69). The writers turn CR/LF inside strings into spaces (dxf lib.rs L416, L514), skip group codes outside i16, and clamp integers (L439, L463-L469). drop_unwritable's count is discarded (dwg lib.rs L513), so the bridge gives no note for objects it drops on DWG save.
- A licence inconsistency was missed. library.rs declares itself 'CC0 / public domain' (https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/doc/src/library.rs#L3), while Cargo.toml L9 and NOTICE L3 say MIT OR Apache-2.0. This is the same file the evaluator suspects of reproducing acad.lin/acad.pat.
- Minor fact errors. (a) crates/io/Cargo.toml L11-L28 has no runtime dependency on tiny-skia (dev-only), lyon, skrifa or harfrust, and it has acadrust 0.6.4 as a dev-dependency (L27). (b) MAX_TAGS is checked before allocation only for ASCII (dxf lib.rs L184-L188); binary checks after each tag is pushed (L377-L380), and binary DXF is R13+ only (L345; parity L66). (c) In parity L99, 'checked with an AutoCAD dynamic block' refers to DWG open/save keeping the data, not to how visibility states are written; the ellipsis joins two different clauses.

**Argument:** The main legal verdict holds. acadrust follows LibreDWG's GPL source: 'Field layouts follow LibreDWG dwg2.spec' (object_context_data.rs L47 at v0.6.4), 'Mirrors libredwg dwg_resbuf_value_type' (objects.rs L21), field names from acds.spec (dwg_writer.rs L1975), and the f256_power table taken from 'LibreDWG/src/reedsolomon.c' (reed_solomon.rs L697-L714). I checked every one at 8d5c46d. This breaks both Spare Kit's rule and CADCraft's own AGENTS.md L25. I also confirmed these, with the cited lines:
- the ACadSharp class-name port;
- the committed AutoCAD menu tree (560 lines);
- the AutoCAD prompt wording;
- the ViewCube label;
- linetype values and descriptions that match acad.lin from general knowledge (BORDER, CENTER, DASHDOT, DIVIDE, PHANTOM and others);
- the private plan/ and craftrules records (404);
- the WASM stubs and the getcadcraft provenance and trademark files.
So DEPEND and PORT stay out for DWG.

Several findings are wrong or overstated, so refuted=true. Two of them cut against the evaluator's harshness:
- acadrust does carry ACadSharp's MIT notice, though only for fixtures.
- acadrust does test byte-identical DXF and DWG output across runs.

Most of the corrections make the case harsher than the evaluator did:
- acadrust compiles a plugin against AutoCAD's own .NET API to create committed fixtures.
- CADCraft's DXF model is planar 2D. 3D SPLINE Z is dropped, and invalid splines are silently refitted.
- Exact ellipse and spline hatch boundary edges are tessellated on import, at a fixed, scale-blind tolerance.
- The size limits do not bound memory (4.5 GB for a 47.8 MB DWG).
- No CI runs the tests on PRs or on main. gaps.md admits regressions go unseen.
- The real-file corpus (#1041) shows BricsCAD refusing 14 of 23 DWGs. 3DSOLIDs, draw order and annotative data are almost entirely lost through DWG.

Opposite case for PORT or PARTNER: I looked for anything best-in-class in this subsystem and found nothing. The tag layer is small and clean in licence. But it zeroes bad numbers silently, replaces bad bytes with U+FFFD silently, and writes R2000 structure under later version numbers. The model above it is lossy for exact 2D curves, which is exactly what Spare Kit must keep. The best ideas are good but ordinary:
- typed skip notes, so a partial read never passes as complete;
- raw passthrough with handle remapping;
- honest parity tables.
ezdxf and getcadcraft's PROVENANCE practice match or beat them.

Opposite case for REJECT: this is stronger than the evaluator allowed. dxf_write.rs and the dynamic-block code carry knowledge taken from observing AutoCAD output ('as AutoCAD writes it'). The DXF code reading also has undocumented provenance for the \M+ code-page mapping (UNVERIFIED). So reading the code itself carries knowledge-taint risk under Spare Kit's legal rule.

STUDY survives only in a narrower form:
- Study the written design patterns and documentation methods: failsafe notes, caps placed before allocation, passthrough, parity tables, and getcadcraft's PROVENANCE.md.
- Do not study crates/io's AutoCAD-derived serialisation code or the dynamic-block code.
- Never touch acadrust.
- DWG in any form still needs an explicit owner decision, because ODA spec v5.4.1 and LibreDWG are both reverse-engineered.

## headless-api

**Commit sha:** storytold/cadcraft 7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1 (main, 2026-10-20). Comparison only: getcadcraft/cadcraft-rust 3daa3406b70d010e2210f109b186f3f2d9480aaf

**Findings:** Link base B = https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1

SCALE AND ORIGIN
- Repo created 2026-10-07. At HEAD: about 192k lines of Rust (counted from the downloaded files), 413 CommandSpec::new registrations, 240 interactive prompt machines, 122 commands marked noundo, and 2,184 #[test] attributes.
- The first version was built by AI agents in one session (B/ROADMAP.md#L132).
- Layering runs L0 geom up to L4 engine, then L5 ui-egui and mcp, then the apps. `cargo xtask layers` enforces that nothing below L5 uses egui, wgpu or winit (B/docs/architecture.md#L13-L33).

1. HOW A COMMAND IS DEFINED: a registry of function pointers taking untyped JSON
- Each command is a CommandSpec (B/crates/engine/src/cmd/mod.rs#L60-L78) with these fields:
  - id: the AutoCAD command name in lower case
  - label, menu path, shortcut and aliases
  - params: a free-text doc string, not a schema
  - enabled: fn(&Session)
  - run: fn(&mut Session, &serde_json::Value) -> Result<Value>
  - undoable
  - an optional interactive factory returning Box<dyn Interactive>
  - transparent
- The registry is a static OnceLock<Vec<CommandSpec>> built by joining each module's specs() (#L170-L216). Alias lookup checks user aliases before built-in ones (#L218-L229).
- Each command parses its own parameters by hand with helpers such as f64_req, point_param, size_param and targets (#L233-L309).
- targets() falls back to the current selection when no handle is given (#L295-L309), so a command's result depends on hidden session state.
- Example: CIRCLE's params doc is the string "{center, radius} | {center, diameter} | {p1, p2} | {p1, p2, p3}" (B/crates/engine/src/cmd/draw.rs#L26-L30).
- Unknown parameter keys are ignored without error. Issue #1225 is open: `rectang {"widht":5}` succeeds and draws a rectangle with no width.

2. TWO IMPLEMENTATIONS PER COMMAND
Programmatic calls go through Session::execute with JSON (B/crates/engine/src/lib.rs#L886-L938). Interactive calls go through Session::start, input and cmdline. Those drive an Interactive state machine (B/crates/engine/src/prompt.rs#L184-L239) with these parts:
- Input: Point, Deferred, Keyword, Text, Pick, Enter, Cancel
- Prompt: message, keywords, default, an Accept mask, a base point and a picker
- Step: Continue, Done, Cancel
- preview, drag and editing_mtext hooks for the UI

The machine changes the drawing itself instead of producing a typed operation and running it. CIRCLE shows the difference: the interactive finish_circle adds the entity and also sets the CIRCLERAD header variable (draw.rs#L864-L868), while the JSON run_circle does not (draw.rs#L154-L174).

Confirmed drift between the two paths: issue #1410 (open). Typed 3DPOLY loses Z because Poly3dM stores Vec2, while the JSON form keeps Z. Typed coordinates also drop their third value in parse_point_with (prompt.rs#L264-L300).

The command line also accepts `cmd {json}`, which goes into the JSON path (lib.rs#L1483-L1495). Free text at Command: is evaluated as AutoLISP (lib.rs#L1505-L1510).

3. TRANSACTIONS AND UNDO: whole-drawing snapshots
- DocState holds an Arc<Drawing> plus undo and redo Vec<Snapshot{label, doc, selection}> (lib.rs#L112-L146). The drawing's collections are copy-on-write (B/crates/doc/src/store.rs#L20-L31), so a snapshot is cheap (B/docs/architecture.md#L45).
- execute keeps the Arc from before the call and then does four things:
  - It catches a panic and restores that Arc (lib.rs#L896-L910).
  - It runs the post-command passes: group pruning, constraint inference and solve, associativity (lib.rs#L1324-L1336).
  - It refuses results containing NaN or infinite values and restores the drawing (B/crates/engine/src/finite.rs#L18-L67, which walks serde output of changed chunks only).
  - It pushes one undo step if the Arc changed (lib.rs#L925-L936).
- An ordinary Err is NOT rolled back. The undo push is not conditional on Ok, so a command that changes the drawing and then returns an error leaves its partial changes in place (as an undo step).
  - Issue #933 reported this for ARRAYEDIT. It was closed without a fix ("no longer working on this").
  - Issue #1039 lists transactions and dry-run as not done.
- Interactive commands record their undo step in finish() (lib.rs#L1283-L1322). Transparent commands and LISP routines fold into one step.
- Undo control (B/crates/engine/src/undoctl.rs#L1-L100):
  - MAX_UNDO is 2000.
  - UNDO Mark, BEgin/End and Control All/None/One are implemented as depth markers on the stack.
- undo() and redo() swap snapshots (lib.rs#L1631-L1654).
- Undo history exists only in memory. It is labelled by command label only; there is no actor, no parameters and no timestamp.

4. NO OPERATION LOG
- Session.log is a Vec<String> of echoed command-line text, trimmed by 1,000 lines once it passes 5,000 (lib.rs#L643, #L834-L842).
- A search of every .rs and .md file found no journal, replay, op log or action recorder.
- The roadmap lists an "action recorder" under M12 as not done (B/docs/roadmap.md#L88).
- Nothing can be replayed, synced or audited from typed records. The only replay is re-running a .scr script or a LISP file.

5. HEADLESS
- The engine Session has no UI dependency and is fully drivable in-process.
- cadcraft-cli (B/apps/cadcraft-cli/src/main.rs#L1-L12) offers info, convert, run, commands, mcp, perf and sample. run takes a file, --script, --script-file and repeated --cmd 'id {json}', and ends with --save or --export.
- The app exposes a JSON-lines control channel on 127.0.0.1:PORT (B/docs/control-protocol.md). It has bounded connections, line size, queue and timeouts. A timeout reports state not-run or may-have-run (#L19-L31).
- The full method set lives in the UI crate (B/crates/ui-egui/src/control.rs#L286-L296). The MCP crate re-implements a headless subset (B/crates/mcp/src/backend.rs#L199-L260), and tests keep the two reply shapes the same (B/crates/mcp/tests/cmdline_parity.rs#L1-L49).
- About 50 UI-only commands live in the UI crate (B/crates/ui-egui/src/menus.rs#L17-L73). Dialogs are thin: they emit (cmd, params) and call app.run, which calls Session::execute (B/crates/ui-egui/src/managers.rs#L20-L66, B/crates/ui-egui/src/lib.rs#L316-L342).
- Calls through engine.execute never open a dialog, because null params become {} (control.rs#L291-L295).
- Session fields are pub (docs, running, log, settings and others; lib.rs#L632-L697). "Everything is a command" is therefore a convention (B/AGENTS.md#L29-L32), not something the type system enforces. View pan and zoom inside an active viewport edit the Viewport entity with no undo step (lib.rs#L239-L272).

6. MCP
- Hand-written JSON-RPC 2.0 over stdio, with no MCP SDK dependency (B/crates/mcp/src/server.rs#L10-L134). It negotiates protocol versions 2025-06-18, 2025-03-26 and 2024-11-05, and offers tools and two resources.
- About 12 generic tools (B/crates/mcp/src/tools.rs#L7-L43):
  - command_line (types like a user)
  - script
  - execute {command, params:{type:object}}
  - list_commands, inspect_drawing, query_entities, render (PNG), find_text
  - new_drawing, open, save, cancel
  - screenshot, ui_inspect, ui_click (only when connected to the app)
- Commands are not one MCP tool each with a schema. All about 400 commands go through the single execute tool with an untyped params object, and agents discover parameters from the free-text params docs.
- Tool arguments are checked against a small JSON-Schema subset: type, items, minItems, maxItems, minimum (tools.rs#L53-L99).
- The Remote bridge never resends a mutating call after a lost reply. It says the call "may or may not have been applied" (backend.rs#L145-L189).
- No idempotency keys. No per-agent permissions, scopes or approvals. No record of what an agent did.
- The control port has no authentication at HEAD: it binds 127.0.0.1 with no token (B/apps/cadcraft/src/control_server.rs#L51-L60). PR #1290 (open) reports that any local process, or a web page posting to localhost, could run engine.execute, script, lisp and app.save. It proposes a 128-bit token.
- Open PRs that would add parts of an agent contract: #1043 (an unknown handle becomes an error), #1047 (stable error codes), #1040 (structuredContent and bbox). Issue #1039 lists the rest: strict addressing, errors as data, writes reporting changed handles, transactions, dry-run.

7. SCRIPTING AND PLUGINS
- .scr scripts run through the same tokenizer as the command line, and nested scripts are refused (lib.rs#L1603-L1629).
- An AutoLISP interpreter of about 200 KB lives in crates/engine/src/lisp. It supports command, entget/entmod, ssget and get... functions, and a routine counts as one undo step (B/docs/autolisp.md#L17-L41).
- There is no plugin API. Rust/WASM plugins are an owner decision that has not been built (B/docs/gaps.md#L138, B/docs/customization.md#L12). Issue #1544 (open) says add-ons can only exist as forks today.

COMPARISON WITH SPAREWARE (EXTENDING.md 2.1)
| Spareware rule | CADCraft |
|---|---|
| Typed operations | Untyped JSON with free-text docs |
| One path for screen, agent, script, plugin, undo and sync | Two paths per command, with confirmed drift |
| Runs in a transaction | Rolls back on panic or non-finite results only, not on ordinary errors |
| Written to an operation log | No op log |
| API, CLI, MCP and docs generated from one schema | Hand-written MCP with one generic execute tool |
| Idempotency keys | None |
| See and undo everything an agent did | Not possible: undo carries no actor |
| Sandboxed WASM plugins | None |
CADCraft does meet two parts of the rule: dialogs emit commands, and the CLI, MCP and control channel all reach the same registry.

GETCADCRAFT/CADCRAFT-RUST (a different organization, created 2026-10-10; not the ArtCraft author)
Link base G = https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf
- Its MCP is stricter: additionalProperties:false on every tool, defaults filled in, and a regex matcher with no backtracking (G/crates/mcp/src/schema.rs#L1-L7, G/crates/mcp/src/defs.rs#L8-L15).
- Separate read and write roots confine file access (G/crates/mcp/src/confine.rs#L1-L7), and a token module exists.
- Commands are async blocks awaiting Prompter::ask, polled by a Runner with no runtime that also works on wasm (G/crates/engine/src/prompt.rs#L1-L6).
- Doc::change(label, f) records one undo step holding snapshots of only what changed (G/docs/architecture.md#L69-L79).
- It is still not schema-generated typed operations with an op log. It is a port of a JavaScript app.

**Robustness exactness:** - Arithmetic: f64 everywhere in the command layer. Command checks use hard-coded absolute tolerances, for example 1e-12 for duplicate points (B/crates/engine/src/cmd/mod.rs#L254-L264) and radius > 1e-12 in run_circle (B/crates/engine/src/cmd/draw.rs#L170). There are no exact predicates at this layer; it relies on cadcraft-geom's f64 geometry.
- Input checks: f64 parameters are filtered with is_finite (mod.rs#L236-L253). size_param refuses a bad size instead of clamping it or taking its absolute value. B/crates/engine/tests/json_param_validation.rs covers degenerate inputs such as a negative radius, a zero-length line and coincident fit points.
- Unknown parameter names are accepted silently (#1225, open). A well-formed handle that names nothing is a silent no-op, for example {moved: 0} (#1039; fix PR #1043 open).
- Panic safety:
  - Production crates deny unwrap, expect and panic, and forbid unsafe (B/crates/engine/src/lib.rs#L8-L9).
  - catch_unwind guards execute (lib.rs#L896-L910) and every step of an interactive machine: factory, begin, input, prompt and preview (B/crates/engine/src/guard.rs#L1-L60).
  - After a panic the drawing is restored to the Arc it had before the command.
- Validity guard: results with NaN or infinite numbers, or overflowing extents, are refused and rolled back, checking changed chunks only (B/crates/engine/src/finite.rs#L1-L67).
- Not atomic on error: execute pushes an undo step whenever the drawing changed, whether or not the command succeeded (lib.rs#L925-L936). A command that changes the drawing and then returns an error leaves the change in place (#933, closed without a fix).
- Degeneracy handling is per command and written by hand in each run function.
- Failures confirmed in issues:
  - #1410: typed 3DPOLY drops Z; the JSON form does not (the two paths differ).
  - #1225: unknown parameters ignored.
  - #933: partial changes kept on error.
  - #1290: unauthenticated control port; parser blow-ups from hostile HATCH and SVG input.
  - #1078 and #1075: saving and opening block the UI thread, so queued control requests time out.

**Determinism:** No cross-platform or native-vs-WASM determinism tests were found. A search for "determinis", "bit-identical" and "reproducib" finds only fixed-seed property tests in geom (B/crates/geom/src/property_tests.rs#L3) and render-order fixes. Arithmetic is std f64 including sin/cos, which Rust does not guarantee to be bit-identical between native and wasm32 (unverified for CADCraft specifically).

Within one process, a JSON execute with explicit handles is deterministic:
- Handles are allocated in sequence from handseed (B/crates/doc/src/lib.rs#L183-L200).
- Blocks are kept in a BTreeMap (doc lib.rs#L98).
- A HashMap is used only as the handle index (B/crates/doc/src/store.rs#L24). Whether any output depends on its iteration order is unverified.

Typed command-line input depends on state the UI feeds into the Session, so replaying a script or a command_line call is not reproducible from the text alone:
- cursor, viewport_px, pickbox, tracking state and the view (lib.rs#L657-L674).
- Direct distance entry follows the cursor's direction (lib.rs#L1580-L1598).
- Pick apertures scale with pixel size (lib.rs#L878-L882, #L1691).
- JSON commands with no handles act on the current selection (cmd/mod.rs#L295-L309).

Undo restores the exact earlier snapshot (shared Arc), so undo and redo are exact by construction. The constraint post-pass (a Levenberg-Marquardt solver) runs after every command (lib.rs#L1332-L1336), so the output of every edit depends on solver floating-point behaviour; it is not bit-checked across platforms.

By contrast, getcadcraft/cadcraft-rust ports fdlibm sin/cos to match V8 byte for byte (G/docs/architecture.md#L79). That is a method worth noting for bit-identical native and WASM results.

**Tests and quality:** Tests:
- About 2,184 #[test] attributes across crates and apps, about 1,052 of them in engine.
- More than 140 integration test files in crates/engine/tests: undo_control.rs, undo_count.rs, json_param_validation.rs, query_entities.rs and others.
- hostile_params_never_panic runs every registered command against 8 fixed hostile payloads (B/crates/engine/src/tests.rs#L668-L684).
- every_interactive_command_starts_and_cancels starts each interactive machine, feeds it a point, then cancels it (tests.rs#L686-L703).

MCP tests:
- An end-to-end session where an agent draws a plate with holes and checks the result only through MCP (B/crates/mcp/src/tests.rs#L34).
- Invalid-params tests (tests.rs#L86).
- Parity tests between the headless and app reply shapes (B/crates/mcp/tests/cmdline_parity.rs#L1-L49).
- CLI tests, including an MCP connect test (B/apps/cadcraft-cli/tests/mcp_connect.rs).

Fuzzing and gaps:
- No cargo-fuzz and no property-based fuzzing of command parameters. The hostile test is a fixed list of payloads.
- No test checks systematically that the JSON form and the interactive form produce the same result, and #1410 shows they drift.

CI:
- At this SHA, .github/workflows holds only freebsd (nightly, through a shared storytold/craft-common-actions workflow pinned to @main), nix eval, packaging-lint, release and windows-arm64 (weekly).
- No per-PR test workflow is in the repo. Quality gates run locally through `cargo xtask ci` (B/AGENTS.md#L34). Org-level required checks are unverified.

Code quality:
- No unsafe (forbid). Clippy denies unwrap, expect and panic.
- Doc comments are good and docs are extensive: docs/architecture.md, control-protocol.md, mcp.md, autolisp.md, gaps.md.
- Very large files: crates/engine/src/lib.rs is 81 KB, cmd/modify.rs 127 KB, draw2.rs 99 KB.
- The code is agent-written at very high speed (ROADMAP.md#L132), and the issue count is high (731 open issues and PRs).

**Legal:** LICENCES
- storytold/cadcraft: the workspace licence is "MIT OR Apache-2.0" (B/Cargo.toml#L9, B/NOTICE). GitHub reports Apache-2.0. Either is compatible with porting methods into MPL-2.0 Spare Kit.
- Dependencies of the command, headless and MCP layers, all permissive:
  - serde and serde_json (MIT/Apache)
  - thiserror and log (MIT/Apache)
  - rstar (MIT/Apache)
  - craft-docpath from storytold/craft-libs (Apache-2.0, git dependency pinned to rev 82a24b0)
- The MCP layer is written by hand, with no SDK dependency.
- acadrust (MPL-2.0, used unmodified for DWG) is outside this subsystem.
- The shared standards repo storytold/craftrules, cited in AGENTS.md#L5 and #L15, returns 404. Its contents are unverified.

CLEAN-ROOM EVIDENCE AND FLAGS
- AGENTS.md#L25 forbids reading the AutoCAD bundle and GPL/LGPL CAD sources (LibreCAD, QCAD, FreeCAD, SolveSpace, OCCT). It allows black-box observation of an installed AutoCAD: running it, taking screenshots, and recording measured observations in plan/autocad/*.
- plan/ is gitignored (AGENTS.md#L8-L12), so the observation record cannot be audited.
- The whole command surface is modelled on what was observed:
  - command ids, aliases, prompt strings and keyword sets
  - system variable names and bit codes, for example the UNDOCTL bits (B/crates/engine/src/undoctl.rs#L50-L73)
  - menu paths
  - the reference app's menu tree transcribed by name into B/xtask/data/menu-catalog.txt and used as the parity metric (B/xtask/src/parity.rs#L1-L4)
  - AutoLISP compatibility (B/docs/autolisp.md)
- Spareware's rule bans knowledge copied or reverse-engineered from proprietary software. OWNER MUST CHECK whether command vocabulary, prompt text and sysvar semantics obtained by observing AutoCAD count as such knowledge. Spare Kit should not adopt CADCraft's command catalog, prompt strings or menu tree. The generic method (a registry, prompt state machines, a JSON form, snapshot undo) is not Autodesk-specific and is safe to learn from.
- Whether AutoCAD prompt and help strings are copyrightable is unverified.

AI PROVENANCE
- The code is mostly AI-agent-written (ROADMAP.md#L132). Whether it accidentally reproduces GPL CAD code cannot be verified. No CLA or DCO was found (unverified).

GETCADCRAFT/CADCRAFT-RUST
- Apache-2.0, owned by a different organization.
- NOTICE says its DWG reader is written from the ODA specification. Its SketchUp reader uses format knowledge "read as a map" from OpenSKP, which is reverse-engineering of a proprietary format: FLAG, do not use. It ports Manifold (Apache-2.0) and fdlibm (G/NOTICE).

**Strengths**

- Every user-visible action is a registered command with a stable id. The command line, menus, scripts, CLI, control channel and MCP all reach the same registry (B/crates/engine/src/cmd/mod.rs#L170-L229, B/docs/architecture.md#L48-L55)
- Dialogs are thin: each emits a (command, params) pair and runs it through app.run, so no dialog-only logic changes the drawing (B/crates/ui-egui/src/managers.rs#L45-L66)
- Undo uses copy-on-write whole-drawing Arc snapshots: cheap, exact, simple, and easy to roll back on failure (B/crates/engine/src/lib.rs#L112-L146, B/crates/doc/src/store.rs#L20-L31)
- Results with NaN or infinite values are refused and the drawing restored, checking only the chunks that changed (B/crates/engine/src/finite.rs#L18-L67)
- Panic guard around execute and every step of an interactive machine, restoring the earlier drawing (B/crates/engine/src/guard.rs#L1-L60)
- After a lost reply the MCP bridge never resends a mutating call, and the control server reports not-run or may-have-run on timeout (B/crates/mcp/src/backend.rs#L145-L189, B/docs/control-protocol.md#L24-L31)
- The prompt machine model is an explicit Prompt/Accept/Input/Step contract that a UI or an agent can read through cmdline.state (B/crates/engine/src/prompt.rs#L9-L65, #L184-L239)
- Fully usable headless: an in-process MCP server, `cadcraft-cli run` with scripts and JSON commands, PNG/SVG/PDF export, and an MCP end-to-end test (B/crates/mcp/src/tests.rs#L34)
- Post-command passes (groups, constraints, associativity) run inside the command's own undo step (B/crates/engine/src/lib.rs#L1324-L1336)
- The author's stated values match Spareware's on this subsystem: headless, MCP-first, never crash, clean-room, Rust and WASM (B/AGENTS.md#L14-L34)

**Weaknesses**

- Commands are untyped: run takes serde_json::Value and params is a free-text doc string. There is no schema, unknown keys are ignored silently (#1225), and nothing can be generated from the commands (B/crates/engine/src/cmd/mod.rs#L60-L78, #L108-L111)
- Each command has two implementations. The interactive machine changes the drawing directly instead of producing a typed operation, and the two paths drift (#1410; CIRCLERAD is set only by the interactive path, draw.rs#L154-L174 vs #L864-L868)
- Not transactional on error: execute pushes an undo step and keeps partial changes when a command returns Err; only panics and non-finite results roll back (B/crates/engine/src/lib.rs#L925-L936; #933 closed without a fix)
- No operation log, replay, sync, actor attribution or idempotency keys. Session.log is trimmed display text and undo history lives only in memory (lib.rs#L643, #L834-L842; B/docs/roadmap.md#L88)
- MCP has one generic execute tool with params of type object for about 400 commands. Commands are not per-tool schemas, writes do not report the handles they changed, and errors are free text (B/crates/mcp/src/tools.rs#L13-L14; #1039, #1047 open)
- Control port has no authentication at HEAD, so local processes or web pages posting to localhost can drive it (B/apps/cadcraft/src/control_server.rs#L51-L60; PR #1290 open)
- Session state is pub, so the command rule is a convention, not enforced by types. Typed input depends on UI-fed cursor and view state, so command-line replay is not reproducible from the text alone (lib.rs#L632-L697, #L1580-L1598)
- The full control method set lives in the UI crate, and the headless MCP backend re-implements a subset (B/crates/mcp/src/backend.rs#L199-L260)
- No plugin API: add-ons must fork (#1544; B/docs/gaps.md#L138)
- The command vocabulary, prompts, sysvars and menu tree come from black-box observation of AutoCAD, whose record is gitignored, a legal flag under Spareware's rule (B/AGENTS.md#L25, B/xtask/data/menu-catalog.txt)
- No per-PR test CI in .github/workflows. The hostile-input test is a fixed list of 8 payloads, not fuzzing (B/crates/engine/src/tests.rs#L668-L684)

**Verdict:** STUDY

**Verdict reason:** Do not depend on or port CADCraft's command layer; study it only. The licence would allow porting (MIT OR Apache-2.0), but the architecture falls short of Spare Kit's rule that every action is a typed operation that runs in a transaction, is written to an operation log, and is generated from one schema into the API, CLI, MCP and docs.

The gaps are structural, not bugs:
- Commands are untyped JSON function pointers with free-text parameter docs.
- Every command has a second interactive implementation that changes the drawing directly, and the two have drifted (#1410).
- A command that fails with an error keeps its partial changes.
- There is no operation log, replay, actor attribution or idempotency key.
- MCP is one generic execute tool.
- The control port has no authentication.
- The command vocabulary was obtained by observing AutoCAD, which needs owner legal review before any part of the catalog is used.

Several methods are worth learning:
- copy-on-write snapshot rollback
- refusing NaN or infinite results
- the panic guard around interactive machines
- explicit prompt machines that an agent can read
- dialogs that only emit commands
- lost-reply semantics (not-run or may-have-run)
- the MCP end-to-end test pattern

The author's stated rules (everything is a command, headless, MCP, never crash) match Spareware's. CADCraft's own issue #1039 asks for the agent contract that Spare Kit's typed operation layer would provide. So a project-level partnership is plausible if CADCraft adopts Spare Kit's typed op layer and the owners clear the AutoCAD-observation provenance. For this subsystem's code, the verdict is STUDY.

**What to take**

- Copy-on-write snapshot undo and rollback: keep the drawing Arc from before the command and restore it on failure. Learn the rollback; extend it to ordinary errors, which CADCraft does not roll back (B/crates/engine/src/lib.rs#L886-L938, B/crates/doc/src/store.rs#L20-L31)
- Refusing non-finite results by walking serde output of only the changed chunks, then restoring and returning a clear error. A good pattern for Spare Kit's 'never returns invalid geometry' check after each operation (B/crates/engine/src/finite.rs#L18-L67)
- The panic guard around every step of an interactive machine, with a fallback prompt and the command ended at the next input (B/crates/engine/src/guard.rs#L1-L60)
- The explicit Prompt/Accept/Input/Step contract, readable as state by agents (cmdline.state). Spare Kit should instead have prompt machines produce typed operations rather than change the model directly (B/crates/engine/src/prompt.rs#L9-L65, #L184-L239)
- Dialogs return (command, params) and run through the same path as everything else (B/crates/ui-egui/src/managers.rs#L45-L66)
- The lost-reply contract: never resend a mutation after a lost reply, and report not-run or may-have-run. Pair it with Spare Kit idempotency keys (B/crates/mcp/src/backend.rs#L145-L189, B/docs/control-protocol.md#L19-L31)
- Post-command passes (constraints, associativity, groups) run inside the command's own undo step (B/crates/engine/src/lib.rs#L1324-L1336)
- Undo control semantics (Mark, BEgin/End, Control) kept as depth markers on one stack, without each command knowing about them. Study the concept only, not the AutoCAD names or bit codes (B/crates/engine/src/undoctl.rs#L1-L100)
- Test patterns: every command against hostile payloads, every interactive command starts and cancels, an MCP end-to-end agent session checked only through MCP. Extend these into property fuzzing generated from schemas (B/crates/engine/src/tests.rs#L668-L703, B/crates/mcp/src/tests.rs#L34)
- Issue #1039 as a ready checklist of what agents need: strict addressing, errors as data, writes reporting handles, transactions, dry-run, output sized for a context window (https://github.com/storytold/cadcraft/issues/1039)
- From getcadcraft/cadcraft-rust (study only, different author): strict MCP schemas with additionalProperties:false and a linear-time regex matcher (G/crates/mcp/src/schema.rs#L1-L7); read and write root confinement (G/crates/mcp/src/confine.rs#L1-L7); async commands awaiting questions with no runtime, which works on wasm (G/crates/engine/src/prompt.rs#L1-L6); undo that snapshots only what changed (G/docs/architecture.md#L69-L79)

### Challenge 1

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- FACT, outdated security claim. The weakness bullet says web pages posting to localhost can drive the control port at HEAD. That is wrong. At HEAD the server rejects any HTTP request line and closes the connection, and nothing sent afterwards runs (B/apps/cadcraft/src/control_server.rs#L140-L173 is_http_request_line -> Line::Reject; B/docs/control-protocol.md#L15-L17). This landed in commit 972ae7888c on 2026-10-16, a day after PR #1290 (opened 2026-10-15) reported the fetch() vector. Still true at HEAD: there is no token, so any local process can drive the port (control_server.rs#L51-L60). B = https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1
- FACT, incomplete. #933 was closed by its reporter PierrunoYT on 2026-10-18 with 'no longer working on this' (state_reason 'completed'). A fix exists as open, unmerged PR #971. It adds a hand-written snapshot restore inside run_arrayedit, and the PR notes that hatch `boundary` already restores itself the same way. 'No fix at HEAD' is accurate. What the evaluation missed: the project's own remedy is per-command manual rollback, not a generic transaction in Session::execute (lib.rs#L926-L936 still pushes undo whether or not the command succeeded).
- FACT, wrong. 'A HashMap is used only as the handle index (store.rs#L24)' is false. HashMap/HashSet appear in 13 engine files, for example cmd/overkill.rs L9, L191, L204, L237, L424; cmd/draworder.rs L72; cmd/inquiry/count.rs L109; cmd/blocks/xref/bind.rs L101. Mitigating: overkill sorts bucket keys before iterating (overkill.rs#L446-L447). Whether every use avoids order-dependent output is unverified.
- FACT, overstated. The determinism section says the LM constraint solver 'runs after every command', so every edit depends on solver floating point. In fact constraints::after_command returns early unless the command succeeded and is undoable (ok flag, lib.rs#L919 and L1329-L1331), and the drawing has constraints that are no longer all satisfied (B/crates/engine/src/cmd/constraints.rs#L1135-L1153).
- FACT, overstated. The NaN/infinity refusal applies only to undoable commands that return Ok (lib.rs#L921-L924; interactive finish L1287-L1291). The 122 .noundo() commands are never checked; most are view or inquiry commands.
- FACT, wrong date. getcadcraft/cadcraft-rust was created 2026-10-15T23:57:59Z, not 2026-10-10. getcadcraft/cadcraft was created 2026-10-13 (gh api repos/getcadcraft/*).
- FACT, imprecise. 'Free text at Command: is evaluated as AutoLISP' is too broad. Only `(expr)` and `!var` go to LISP; any other token starts a command (lib.rs#L1505-L1512).
- FACT, minor counts. At the pinned SHA there are 415 CommandSpec::new calls (not 413), 2,214 #[test] (not 2,184) and 193,684 .rs lines. The 240 `.interactive(` registrations are backed by 156 `impl Interactive` types. 122 noundo and 1,052 engine tests are correct. None of this changes the findings.
- MISSED, robustness/WASM. The panic guard listed as a strength does not work in WebAssembly. wasm32-unknown-unknown uses panic=abort by default, so catch_unwind in Session::execute (lib.rs#L896) and guard.rs catches nothing there. CADCraft sets no panic strategy (Cargo.toml#L71-L76) and installs no panic hook. It does ship the web build (apps/cadcraft-web; AGENTS.md#L33 'Never break wasm'). So 'restores the drawing on panic' holds natively only. This is standard Rust target behaviour, not executed against CADCraft.
- MISSED, multi-actor undo corruption (from reading the code, not executed). Session::execute never checks `running` (lib.rs#L887-L938), but start() cancels a running command (lib.rs#L997-L999). An agent's engine.execute that arrives while a person is mid-command (ui-egui control.rs#L291-L295 -> app.run -> execute) pushes its own undo step. When the person's command finishes, it pushes its pre-start snapshot (lib.rs#L1316-L1321), so undoing the person's command also erases the agent's edit. This directly concerns Spareware's 'people and agents share one command API'.
- MISSED, cancel is not rollback. finish() ignores `cancelled` (lib.rs#L1301), and B/crates/engine/src/tests.rs#L660-L664 asserts that `line 0,0 1,0 2,0` followed by Esc keeps 2 entities. Interactive machines write to the drawing input by input, so the interactive path is non-transactional by design, not only on Err.
- MISSED, the command layer is 2D-only. Input::Point(Vec2) (prompt.rs#L188), point_value returns Vec2 (cmd/mod.rs#L271-L285), and typed Z is parsed then dropped (prompt.rs#L293-L296). The model has no solids, surfaces, meshes or UCS (docs/architecture.md#L43-L46). This layer could not carry a 3D BIM kernel's API as-is, which reinforces not porting it.
- MISSED, MCP file access is unconfined. The open, save and render tools take any path (B/crates/mcp/src/tools.rs#L23-L34), and crates/mcp and apps/cadcraft-cli contain no confinement code. The 'Not changed' section of PR #1290 notes that LISP open/write-line reach any path. By contrast, getcadcraft confines reads and writes to named roots (G/crates/mcp/src/confine.rs#L1-L7).

**Argument:** The STUDY verdict holds, and the core findings check out against storytold/cadcraft at 7fef8cbe:

- CommandSpec takes untyped serde_json::Value with a free-text params doc (cmd/mod.rs#L60-L78, L108-L111).
- Commands with an interactive form have a second implementation that writes to the drawing directly (CIRCLERAD is set at draw.rs#L864-L868 but not in run_circle at L154-L174), and the two have drifted (#1410, open).
- execute pushes an undo step whether or not the command succeeded (lib.rs#L926-L936).
- The log is trimmed display text (lib.rs#L834-L842), and there is no op log, actor or idempotency key.
- MCP has one generic execute tool with params:{type:object} (tools.rs#L13-L14).
- Unknown keys are ignored silently (#1225, open).
- Licence is MIT OR Apache-2.0 (Cargo.toml#L9, NOTICE). craft-libs is Apache-2.0. storytold/craftrules returns 404.
- There is no per-PR CI: the five workflows trigger only on schedules, the release branch or packaging paths, and recent merged PR heads have 0 check runs.
- The clean-room flags are accurate (AGENTS.md#L8-L12, L25; xtask/src/parity.rs#L1-L4; menu-catalog.txt has 560 lines).

The generous case (PORT or PARTNER) does not survive. The best parts are solid but standard:
- the lost-reply contract (not-run / may-have-run) and its MCP mapping (backend.rs#L145-L189)
- the bounded control server that fails closed on bad framing
- the copy-on-write chunked store with validation of changed chunks only (finite.rs)
- thin dialogs that emit (cmd, params) (managers.rs#L20-L66)

None of these is the schema-generated, typed, transactional, logged operation that Spare Kit requires. Porting the layer would mean importing an AutoCAD-shaped, 2D-only, untyped surface.

The harsh case (REJECT) gains real ammunition the evaluation missed:
- The panic guard does nothing in WebAssembly, because panics abort there.
- An agent's execute can land in the middle of a person's interactive command and corrupt undo grouping.
- Cancelling an interactive command keeps its partial edits.
- The prompt/input layer is Vec2-only.
- MCP file access is unconfined.

These reinforce 'do not depend or port', but they do not erase the value of the generic methods. Snapshot rollback (extended to Err and cancel), the post-edit validity sweep, the lost-reply semantics, the agent-readable prompt state and the MCP end-to-end test pattern are still worth learning. For partnership, issue #1039 is a good map of what agents need. The getcadcraft async-command design is the better model to study: one implementation serves both interactive and scripted use, with no runtime and working on wasm.

The factual errors are secondary:
- The web-page attack on the control port is already closed at HEAD (since commit 972ae7888c).
- The HashMap claim is false.
- The solver and NaN-check scope are overstated.
- The getcadcraft creation date is wrong.

None of them flips a key finding or the verdict.

## tests-quality

**Commit sha:** storytold/cadcraft 7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1 (main, 2026-10-20). Compared repos: getcadcraft/cadcraft-rust 3daa3406b70d010e2210f109b186f3f2d9480aaf and getcadcraft/cadcraft 93b7f5eb58b94e726a5d5333dd792cca8f1d7a14

**Findings:** SC = https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1

SCALE. The repo was created on 2026-10-07. The first commit, d9ab00c, added 30,261 lines. There are now about 194k lines of Rust, 586 commits on main and 72 author names. 458 of those commits (78%) carry a Claude co-author or "Generated with" trailer. The project has 528 merged PRs, 378 open PRs and 352 open issues, 13 of them critical.

TESTS
- 2,212 #[test] by static count. The nightly FreeBSD x86_64 run 39365360118 passed 2,204 tests, failed 0 and ignored 0.
- By crate: engine 1,052, ui-egui 315, io 309, render 142, geom 84 (for 5,031 lines), constraints 37.
- Almost every test is a hand-written example. There is one property-test file with 14 tests, using a home-made fixed-seed xorshift generator. It feeds hostile f64 values (NaN, inf, subnormals, raw bit patterns) and checks known answers at 1e6-1e7 coordinates: SC/crates/geom/src/property_tests.rs#L1-L51 and #L411-L437.
- There is no proptest, no cargo-fuzz and no criterion. The "hostile" tests are fixed lists of inputs, for example SC/crates/io/src/tests.rs#L337-L350.
- There is no coverage tool, cargo-deny or cargo-audit. Dependabot runs daily and CodeRabbit reviews PRs. The SonarCloud badge is static, and I could not verify that a scan actually runs.

TEST DESIGN DEFECT. Session::execute catches panics with catch_unwind and turns them into EngineError::Internal (SC/crates/engine/src/lib.rs#L886-L910). The tests that are meant to prove nothing panics call execute and then ignore the result with `let _ =`. Examples: hostile_params_never_panic at SC/crates/engine/src/tests.rs#L668-L684, and SC/crates/engine/tests/modify_curves.rs#L339-L360. No test asserts that Internal never comes back, and no panic hook is installed. So these tests cannot fail when a command panics.

CI
- Main has no workflow that runs the tests on each PR or push. The only workflows are freebsd, nix, packaging-lint, release and windows-arm64: SC/.github/workflows/freebsd.yml#L6-L14.
- The project's own docs say so (SC/docs/gaps.md#L21 and #L118). PRs #847 and #1498, which would add that CI, are still open.
- The tests run only as a nightly FreeBSD VM job and on release pushes.
- The quality gate is `cargo xtask ci`, run locally: fmt, clippy -D warnings, test, assets, layers, wasm (SC/xtask/src/main.rs#L151-L200).
- The wasm step is only `cargo check` (#L134-L149). No test ever executes in wasm.

PANICS AND UNSAFE
- unsafe_code = forbid for the whole workspace (SC/Cargo.toml#L60).
- Every crate root denies unwrap, expect, panic, todo, unimplemented and unreachable (SC/crates/geom/src/lib.rs#L7-L8). The only allows are in test modules.
- clippy::indexing_slicing is NOT denied, and the geom crate indexes directly. Examples: the spline basis and the Gaussian solve at SC/crates/geom/src/spline.rs#L144-L191.
- The catch_unwind guard (SC/crates/engine/src/guard.rs#L1-L7) does nothing on wasm32, where a panic aborts. The web app has no panic hook. Open issue #1603 is a capacity-overflow panic that kills the web build.

GEOMETRY BUGS IN THE TRACKER
Two bugs were closed by their reporter without any fix and are still in main:
- #926, circle-circle false tangency. intersect.rs was last changed by 35a8617 on 2026-10-10. The code at SC/crates/geom/src/intersect.rs#L55-L89 treats a normalised h2 <= 1e-12 as a tangency. I ran a line-for-line Python copy of that code (not the Rust itself). A circle of radius 1 centred on a circle of radius 1e7 gave 1 intersection instead of 2. At radius 1e6 the y result was off by 4.4e-5. The property test missed this because it only uses radii between 1 and 40 (#L186-L213).
- #929. The grid that pairs segments for intersection testing skips cells on very long segments, so real crossings can be missed (SC/crates/geom/src/region.rs#L100-L107).

Fixed with regression tests:
- #704: b209bb5
- #705: c033516, test in modify_curves.rs
- #338: PR #339, a check that edit results stay finite
- #779: PR #788

Still open:
- #1559, critical: FILLET changes the circle; #1326 was the same report, closed without a fix
- #1690 and #1014: hatch boundary not found
- #1179: AutoCAD fit-point splines rebuild as a different curve
- #1697: TAN/PER snaps can land in an arc's gap
- #1603: web build panic (see above)

THE PROJECT'S OWN GEOMETRY AUDIT (SC/docs/geometry-parity.md)
- Geometry is about 40% done; 3D does not exist (#L11-L17).
- One absolute EPS = 1e-9, no robust predicates, large coordinates untested (#L36-L38).
- Ellipses and splines are handled by tessellation (#L62-L64).
- Polyline offset does not recompute bulges and does not clean up loops (#L76).
- No AutoCAD numeric comparison has been run yet (#L21-L24).

DOCS
- Detailed parity and gap documents cite file:line evidence. That is good.
- SC/docs/architecture.md#L11-L15 is out of date: it says 57k lines and 2,067 lines for geom.
- plan/ is gitignored (SC/AGENTS.md#L8-L12), so the architecture plan, ADR 0001 on geometry libraries and the AutoCAD observation notes are not public.
- About 51% of the public items in geom have doc comments (113 of 223), and there is no missing_docs lint.

GETCADCRAFT
Who runs it:
- The getcadcraft org belongs to The Data Design Group, Inc. (GitHub user ddgconsultant, Richard M. Davis). The org was created 2026-10-10.
- It claims "CadCraft" as a trademark: https://github.com/getcadcraft/cadcraft/blob/93b7f5eb58b94e726a5d5333dd792cca8f1d7a14/TRADEMARKS.md
- It is not a fork of storytold. Their CadCraft is a JavaScript app. I found no shared code; the only overlap is a similar AGENTS.md template.

getcadcraft/cadcraft (the JavaScript app): 9 commits. Its Node test workflow has failed on every push since 2026-10-19.

getcadcraft/cadcraft-rust (GR = https://github.com/getcadcraft/cadcraft-rust/blob/3daa3406b70d010e2210f109b186f3f2d9480aaf):
- About 377k lines, 739 #[test], built in 5 days under a single "CadCraft" identity.
- AGENTS.md requires AI-assistant co-author trailers to be stripped from commits (GR/AGENTS.md#L539).
- Much stronger quality infrastructure:
  - CI runs on every push: fmt, clippy, test, layers, wasm, cargo-deny and a fuzz smoke run, plus 900 s of fuzzing nightly and a Windows/macOS test matrix (GR/.github/workflows/ci.yml#L1-L50, GR/xtask/src/main.rs#L67-L82).
  - cargo-deny with a licence allowlist (GR/deny.toml).
  - 42 cargo-fuzz targets, one of them manifold_bool.
  - indexing_slicing and arithmetic_side_effects are denied in the IO and manifold crates.
  - Deterministic maths: fdlibm sin/cos ported to match V8 (GR/crates/cad-geom/src/jsmath.rs#L1-L9).
  - A Rust port of Manifold 3.5.4 with golden tests that require bit-identical output (GR/crates/manifold/src/lib.rs#L8-L45, GR/crates/manifold/tests/golden.rs#L1-L37).
- But CI failed in 47 of the last 81 runs; the latest failure is a Windows takeoff test. The geometry is a port of their JS app with absolute tolerances, and code that returns an empty result where the JS threw an error. Shewchuk orient2d is used only in the civil crate.

Overall: getcadcraft has the better test and determinism process. storytold has the larger community and the more substantial test suite. Neither has an exact or robust kernel.

**Robustness exactness:** - Arithmetic is plain f64 everywhere. There are no exact or adaptive predicates (SC/docs/geometry-parity.md#L37).
- Tolerances:
  - one absolute EPS = 1e-9 (SC/crates/geom/src/lib.rs#L33)
  - fixed 1e-9 and 1e-12 thresholds in intersections (SC/crates/geom/src/intersect.rs#L19 and #L38)
  - a relative 1e-14 cut-off for parallel lines (#L8)
  - an absolute 1e-14 pivot in the spline fit solve (SC/crates/geom/src/spline.rs#L169-L191)
- Line, arc and circle results come from closed formulas. Ellipses and splines are tessellated for intersection, offset, trim and closest point, using 1e-3 or 1e-4 world-unit steps or 720 samples plus bisection.
- Region and hatch work on polygonised curves:
  - vertices are merged by grid-key rounding
  - pairing uses a broad phase that can miss pairs on long segments (SC/crates/geom/src/region.rs#L100-L107, #929, still in main)
  - hard caps: 60k segments, 4,096 islands, 20M tested pairs
- Degenerate input is handled by finiteness checks and silent fallbacks, for example an invalid spline evaluates to its first control point (spline.rs#L70-L73). The never-crash rule therefore produces quiet wrong answers rather than reported errors.
- Bugs found and confirmed in main:
  - #926: circle-circle false tangency when radii are far apart. In my Python copy of the code, a radius-1 circle centred on a radius-1e7 circle gives 1 intersection instead of 2. At radius 1e6 the y error is 4.4e-5.
  - #929: pairs skipped by the coarse grid.
- Fixed: the parameter-at-length panic (#704), arc-length convergence (#705) and edits overflowing to infinity (#338).
- Still open: hatch-boundary failures (#1014, #1690), FILLET damaging the circle (#1559), different rebuild of AutoCAD fit-point splines (#1179).
- There is no 3D and no B-rep.

**Determinism:** Determinism is not a design goal and nothing tests it.
- geom calls std sin, cos, atan2 and acos (platform libm) in about 25 places.
- Issue #804 shows a 3e-16 difference on Windows. The fix, PR #762, says MSVC's sin and cos round differently from glibc's and macOS's, and removed a clip operation rather than making the maths deterministic.
- The wasm build is only `cargo check`ed (SC/xtask/src/main.rs#L134-L149). No test compares native and wasm output.
- Some care is visible: island order from a HashMap is sorted (SC/crates/geom/src/region.rs#L401-L402), and the property tests use fixed seeds.
- rayon is used only for building display lists (crates/render/src/lib.rs L563, L755).

getcadcraft-rust, by contrast, is built around determinism:
- It ports fdlibm and msun so its results match V8 bit for bit, and records that the platform libm sometimes flipped a ceil or a nearest-candidate choice (GR/crates/cad-geom/src/jsmath.rs#L1-L9).
- It ports Manifold with no fused multiply-adds and checks golden output bit for bit.

**Tests and quality:** storytold/cadcraft:
- 2,212 tests, all passing in the FreeBSD nightly. They are example-based: unit tests, command integration tests, DXF/DWG/PDF round trips, egui_kittest UI tests and a LISP corpus.
- geom has only 84 tests, 14 of them property-style with a home-made generator.
- No fuzzing, no proptest, no benchmarks, no coverage measurement, no cargo-deny.
- No CI gate on PRs (gaps.md S1; PRs #847 and #1498 are open). Lint cleanliness on main depends on contributors running `cargo xtask ci` themselves; I did not run clippy, so whether main is clean is unverified.
- The "never panic" tests through Session::execute cannot detect panics, because catch_unwind turns them into ignored errors.
- unsafe is forbidden and panic macros are denied by lint. Slice indexing is not denied.
- The documents are long and honest, with file:line evidence, but architecture.md is out of date and the design notes in plan/ are private.
- Bugs mostly get regression tests (#704, #705, #338). Two geometry audit issues (#926, #929) were closed without fixes.

getcadcraft/cadcraft-rust:
- Better process: a CI gate on every push, 42 fuzz targets, golden parity tests, cargo-deny, a pinned toolchain, stricter lints.
- Fewer tests per line (739 for 377k lines), and CI fails more often than it passes (47 of the last 81 runs).

**Legal:** storytold/cadcraft:
- The workspace licence is MIT OR Apache-2.0 (SC/Cargo.toml#L9; both licence files are present). GitHub shows only Apache-2.0. Either is compatible with MPL-2.0.
- Main dependencies: egui (MIT/Apache), tiny-skia (BSD-3), rstar, kurbo, skrifa and harfrust (permissive).
- For DWG it uses acadrust 0.6.4, licensed MPL-2.0 and used unmodified (SC/crates/dwg/Cargo.toml). Where acadrust's DWG knowledge comes from is unverified.

Clean-room questions to flag:
- AGENTS.md says AutoCAD is installed on the development machine and observed black-box. Screenshots and "measured observations of the running reference app" are kept in the private plan/ folder (SC/AGENTS.md#L8 and #L25). Issues routinely describe "EXPECTED BEHAVIOR (AutoCAD)". This is behaviour observation of proprietary software, not code reverse engineering, but under the Spare Kit rule it needs owner review.
- File formats come from Autodesk's DXF Reference and the ODA "Open Design Specification for .dwg". The ODA specification is itself the result of reverse-engineering Autodesk's DWG format, so it must be flagged.
- crates/io/tests/acis_data_store.rs handles ACIS data stored in DWG. Where that format knowledge comes from is unverified.
- The geom algorithms look like textbook methods (de Boor basis and span search, closed-form intersections). I saw no proprietary-derived content in geom.
- 78% of commits are co-authored by AI. Owners should consider provenance and copyrightability.
- The "CADCraft" name may conflict with an Autodesk partner called Cadcraft AB (issue #40) and with The Data Design Group's "CadCraft" trademark claim.

getcadcraft:
- Apache-2.0, and the names are reserved as trademarks.
- cc-io-skp takes its SketchUp format knowledge from OpenSKP, a reverse-engineered proprietary format. Flag it (GR NOTICE L14-L19).
- DWG is written from the ODA specification. Flag it.
- The Manifold port (Apache-2.0), Clipper2 (BSL-1.0), fdlibm (Sun notice) and robust-predicates (Unlicense) are all permissive.
- AI authorship is deliberately hidden (GR/AGENTS.md#L539).
- Where the golden DWG fixtures come from is unverified.

**Strengths**

- Workspace-wide `unsafe_code = forbid`, and every production crate root denies unwrap, expect and panic, enforced by lint (SC/Cargo.toml#L60, SC/crates/geom/src/lib.rs#L7-L8)
- A large, all-passing suite: 2,204 tests passed and 0 failed in the FreeBSD nightly, covering commands, round trips, UI and LISP
- The property-test file feeds hostile f64 values (NaN, inf, subnormal, raw bits) and checks known answers at 1e6-1e7 coordinates (SC/crates/geom/src/property_tests.rs)
- Most crash fixes land with a regression test (#704, #705, #338, #779)
- `cargo xtask layers` enforces the crate layering, and every command is reachable headless through CLI, control channel and MCP, with an all-commands smoke test
- Honest self-audit documents with file:line evidence (docs/geometry-parity.md, docs/gaps.md)
- Very active community: 72 authors and 528 merged PRs in two weeks

**Weaknesses**

- No CI test gate on PRs or on main; the tests run only in a nightly FreeBSD job (SC/docs/gaps.md#L118)
- No fuzzing, no proptest, no benchmarks, no coverage tool, no cargo-deny
- The tests that claim nothing panics cannot detect a panic: Session::execute catches it with catch_unwind and the result is ignored (SC/crates/engine/src/lib.rs#L896, SC/crates/engine/src/tests.rs#L668-L684)
- Slice indexing is not denied by lint, and geom indexes directly (SC/crates/geom/src/spline.rs#L144-L191)
- The catch_unwind safety net does nothing on wasm and there is no panic hook (#1603)
- f64 only, with absolute epsilons and no robust predicates; ellipses and splines are tessellated; no 3D
- Two geometry audit bugs (#926, #929) were closed without a fix and are still in main
- No determinism policy: platform libm differences were seen on Windows (#804/#762), and wasm is never tested, only checked
- Silent fallbacks give invalid or degenerate results instead of errors (SC/crates/geom/src/spline.rs#L70-L73)
- Tests compare f64 against f64 closed forms, with no exact oracle, and the random ranges are too narrow to expose scale bugs
- Docs are out of date (architecture.md), and the design plan and ADRs are private (plan/ is gitignored)
- The code is 78% AI co-authored and two weeks old; there is a backlog of 378 open PRs

**Verdict:** STUDY

**Verdict reason:** The quality practices are worth copying. These are the never-crash lint header, forbidding unsafe, hostile-input property tests, enforced layering, and one command API shared by CLI, control channel and MCP. But the geometry code and its tests fall far short of Spare Kit's bar:
- There is no exact or robust arithmetic, no determinism, and no wasm testing.
- There is no fuzzing and no CI gate.
- Some "never panic" tests cannot detect a panic.
- Known geometry bugs are closed without fixes.

So there is nothing to depend on or port.

getcadcraft-rust has a better test and determinism process (fuzzing, cargo-deny, bit-identical golden tests, fdlibm), and that process is worth studying. Its kernel is still a JS-parity mesh and 2D port with failing CI, and its provenance is opaque.

Partnering with the CADCraft author is a people and product question outside this subsystem. Issue #414 shows CADCraft wants a shared B-rep kernel, so it would fit as a consumer of Spare Kit, not as a source of kernel code or tests.

**What to take**

- The pattern of SC/crates/geom/src/property_tests.rs: a hostile-f64 generator (NaN, inf, subnormal, raw bits) plus known answers at large coordinates. Rebuild it on proptest with exact (rational or adaptive) oracles and wider scale ranges.
- The never-crash lint header (SC/crates/geom/src/lib.rs#L7-L8) and SC/clippy.toml. Also deny indexing_slicing and arithmetic_side_effects, as GR/crates/manifold/src/lib.rs#L79-L80 does.
- The crate-layering check in SC/xtask/src/layers.rs, and the all-commands smoke test in SC/crates/engine/src/tests.rs (every_interactive_command_starts_and_cancels).
- A lesson: tests must assert that no internal error came back, or switch catch_unwind off under test. Otherwise the panic guard hides panics (SC/crates/engine/src/lib.rs#L886-L910).
- A lesson from #926 (SC/crates/geom/src/intersect.rs#L55-L89): a fixed threshold on a normalised value is not scale-safe. Use robust predicates plus an error-aware tangency classification.
- A lesson from #929 (SC/crates/geom/src/region.rs#L100-L107): the broad phase must be conservative.
- A lesson from #804/#762 and GR/crates/cad-geom/src/jsmath.rs#L1-L9: Spare Kit must own its transcendental functions (an fdlibm/msun port or the libm crate), avoid fused multiply-adds, and test bit-identical output between native and wasm in CI.
- From getcadcraft-rust, the CI design: a gate on every push with fuzz smoke runs, nightly fuzzing, cargo-deny and an OS matrix (GR/.github/workflows/ci.yml, GR/xtask/src/main.rs#L67-L82, GR/deny.toml).
- From getcadcraft-rust, the bit-identical golden differential tests against a reference implementation (GR/crates/manifold/tests/golden.rs#L1-L37).
- The parity and gap documents with file:line evidence (SC/docs/geometry-parity.md), as a model for honest capability docs.

### Challenge 1

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- First commit misstated. The root commit is ebfebbdbf4236192f9d5df219e59698407f6dacd ('first commit', 2026-10-06, 7 lines: .gitignore and README). d9ab00c (+30,261/-5) is its child: the first commit with code, not the first commit. Minor.
- kurbo is called a 'main dependency', but no crate declares it at 7fef8cb. It appears only transitively in Cargo.lock (L2069-2071, pulled in by peniko). The direct dependencies are in SC/Cargo.toml L13-57, and geom depends only on serde (SC/crates/geom/Cargo.toml). Minor.
- 'The web app has no panic hook' is literally false. SC/apps/cadcraft-web/src/main.rs L114 calls eframe::WebRunner::new() (eframe 0.36.2, Cargo.lock L1085-1086). That constructor installs PanicHandler through std::panic::set_hook (egui tag 0.36.0, crates/eframe/src/web/web_runner.rs L39-45 and web/panic_handler.rs). The substance still holds: CADCraft never calls has_panicked or panic_summary, so nothing tells the user or recovers their work. #1603 says the user is 'left on the Start tab with no message'.
- The getcadcraft-rust lint strictness is understated. clippy::indexing_slicing is denied in all 30 crate roots, not only the IO and manifold crates (for example GR/crates/cad-geom/src/lib.rs L16-24, GR/crates/geom/src/lib.rs L11, GR/crates/engine/src/lib.rs L18-25). arithmetic_side_effects is denied in about 19 crates, including engine (L29), cad, bim, drawing, live, markup, mcp, part, takeoff and every io-* crate. GR/AGENTS.md L86 makes indexing_slicing mandatory in every crate root. The point of the finding stands, and is stronger: GR's lint header is the one to copy.
- The SonarCloud question marked unverified can be answered. The SonarCloud project storytold_cadcraft exists (api/components/show), but api/project_analyses/search returns total 0 and api/measures/component returns an empty measures list (checked 2026-10-21). No scan has ever completed, so the README L30 badge is decorative.
- #926 and #929: 'closed without any fix' is true for main, but it leaves out that fix PRs exist. #976 (for #926) and #975 (for #929), both by MarekSpodBiedry, are open and unmerged. Reporter PierrunoYT closed both issues on 2026-10-18 with 'no longer working on this'. This is the review backlog, not maintainers dismissing the bugs.
- #926 is worse than reported. I ran a line-for-line Python copy of SC/crates/geom/src/intersect.rs L55-89 (not the Rust itself). With R=1e6 and the small circle on the rim, c1×c2 gives 2 points with a y error of 4.4e-5, but c2×c1 gives a single false tangent point. With R=3e6 and R=1e7, both orders give 1 point. So the function is not symmetric at scale. The symmetry property test only samples radii from 1 to 40 (SC/crates/geom/src/property_tests.rs L188-215).
- 'Issue #414 shows CADCraft wants a shared B-rep kernel' overstates it. #414 is a proposal from community user lpgn. No maintainer (echelon/storytold) has replied, and only community members have commented. CADCraft's own plan is milestone M11, 3D, estimated at 300-450 h (SC/docs/roadmap.md L87). Its kernel decision is in the private plan/adr/0001 (SC/AGENTS.md L9, L12).
- The cited project audit is stale on one point. SC/docs/geometry-parity.md L38 says large coordinates are 'untested' (last reviewed 2026-10-10). SC/crates/geom/src/property_tests.rs L411-437 now tests known answers at shifts of 1e6, 3e6 and 1e7. The evaluation cites both without noting that they conflict. Minor.
- The getcadcraft licences are not all inside Spare Kit's list. GR/deny.toml L14-27 also allows OFL-1.1, Unicode-3.0, ISC, CC0-1.0 and MIT-0. The orient2d port in GR/crates/civil/src/predicates.rs L1-5 is Unlicense. None of these is on Spare Kit's allowlist (MIT, Apache-2.0, BSD, Zlib, Boost, MPL-2.0), so anything copied from there needs owner sign-off. Minor.
- Context, not an error. FreeBSD run 39365360118 concluded 'failure' overall, because the aarch64 job failed at 'Start the FreeBSD VM'. The x86_64 job 118440857007 passed; summing its log gives 2,204 passed, 0 failed, 0 ignored. The evaluation's x86_64 scoping is correct.
- Context on getcadcraft-rust CI, not an error. At the pinned commit 3daa340 (run 39453778319), the Linux gate job (fmt, clippy, test, deny, fuzz smoke) and macOS both passed. Only the Windows test job failed, at crates/io-takeoff/tests/exports.rs:211 (bid_summary_and_proposal). The overall count is right: 47 failures and 34 successes out of 81 completed runs.
- Minor count drift: 379 open PRs on 2026-10-21, not 378.

**Argument:** The STUDY verdict holds; none of the errors above changes it. I checked the evaluation against the storytold tarball at 7fef8cbe, the getcadcraft-rust tarball at 3daa3406, the GitHub API and the CI logs.

FACTS that check out:
- Size and tests: 193,684 lines of Rust and 2,212 #[test]. Per crate: engine 1,052, ui-egui 315, io 309, render 142, geom 84 (5,031 lines), constraints 37.
- History: 586 commits, 458 with a Claude co-author trailer, 72 author names. 528 merged PRs, 352 open issues, 13 marked critical.
- Licence and lints:
  - The licence is MIT OR Apache-2.0 (Cargo.toml L9), and unsafe_code is forbidden (L60).
  - All 15 crate and app roots carry the deny(unwrap, expect, panic, …) header. A scan found no production unwrap.
  - indexing_slicing is never enabled. The allow-indexing-slicing-in-tests setting in clippy.toml does nothing without it.
- The panic-guard test defect is real:
  - Session::execute wraps commands in catch_unwind (engine/src/lib.rs L896).
  - hostile_params_never_panic (tests.rs L667-684) and hostile_curve_edits_do_not_panic (modify_curves.rs L339-366) throw away the result with `let _ =`. tolerance.rs L126 only checks is_err(), which an Internal error also satisfies.
  - The only tests that assert Internal are guard.rs's own injected-panic tests.
- CI: no workflow on main runs the tests on PRs or pushes (freebsd.yml L6-14, gaps.md row 6 and S1). xtask's wasm step is only `cargo check` (L134-149).
- Code: the intersect.rs tangency threshold, the region.rs L100-107 coarse grid, the spline.rs fallbacks and the 1e-14 pivot are all as described. Doc-comment coverage is 113 of 224 public items.
- Issues and legal:
  - The #804/#762 wording matches the issue and PR.
  - acadrust is MPL-2.0 and used unmodified.
  - AGENTS.md L25 confirms that AutoCAD is observed black-box and that the ODA DWG specification is used.
- getcadcraft-rust:
  - 739 tests outside vendor/ and about 377k lines.
  - 42 fuzz targets. AGENTS.md L539 strips assistant trailers from commits.
  - jsmath.rs ports fdlibm. The Manifold port uses no fused multiply-adds, and its golden tests require bit-identical output.

FIT, arguing the generous side (could this be DEPEND, PORT or PARTNER?): I looked for anything best-in-class in storytold's test and quality system. I found none.
- The best practices here belong to getcadcraft-rust: indexing_slicing denied in every crate, arithmetic_side_effects denied, fuzzing, cargo-deny, a CI gate, and bit-identical golden diffing.
- storytold's catch_unwind net is an anti-pattern under Spare Kit's 'never return invalid geometry' rule. It turns kernel bugs into recoverable command errors, and it hides them from tests. On wasm it does nothing, so the browser build loses the user's drawing.
- The geometry has no exact or robust predicates and no B-rep. Scale bugs stay unfixed while fixes wait in an open-PR backlog of 379 (#975 and #976).
- The SonarCloud badge is backed by zero analyses.
- So nothing qualifies for DEPEND or PORT.

FIT, arguing the harsh side (should it be REJECT?):
- Some patterns are still worth recording:
  - the hostile-f64 generator idea
  - the every-command start-and-cancel smoke test
  - the parity and gap documents with file:line evidence
  - one command API shared by the CLI, the control channel and MCP
- Lessons from #926, #929, #804 and the panic-guard design are also worth writing down.
- That is study material, not REJECT.

PARTNER is a people question outside this subsystem. CADCraft's 3D kernel plan (M11) is undecided in public. #414 is a community proposal, not the maintainers' position.
