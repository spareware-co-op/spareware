# Spare Construct status

**Stage: Mapping.** Roadmap phase 0 (Foundations) has not started.

- Architecture map written: [ARCHITECTURE.md](ARCHITECTURE.md), 23 sections at high and low level.
- 21 decisions open (table near the end of the map). No code yet.
- **Traffic and public transport is further along.** A working browser prototype, the [Transport demo](https://spareware.app/spare-construct/#plan:transport) (open it from the discipline button in the Spare Construct demo), already runs a strategic traffic model and a microsimulation on a live map. It was made to help a traffic engineer, who hasn't used it yet, so nobody from the field has checked it. The product is built from the core up on the shared engines, using the prototype to learn from.
- Design stage starts when an owner accepts decision notes for the first six decisions: kernel start, storage layout, entity store, incremental engine, renderer, multi-user.

## Work board

Sizes: Small fits a little spare, Medium a fair amount, Large a lot. Who has a piece and its state live on its issue; check pieces out with `python tools/piece.py checkout <ID>` (see AGENTS.md).

| ID | Piece | Kind | Size | Done when | Issue |
| --- | --- | --- | --- | --- | --- |
| A01 | Kernel: how Manifold does booleans, and the order to port it to Rust | Research note | Large | Names Manifold's method, data structures and a port order, with links into its source | [#16](https://github.com/spareware-co-op/spareware/issues/16) |
| A02 | Entity store: our own or bevy_ecs | Decision note | Medium | Recommendation backed by both codebases, covering chunked storage and permanent IDs | [#17](https://github.com/spareware-co-op/spareware/issues/17) |
| A03 | Incremental engine: the Salsa crate or our own | Decision note | Medium | Recommendation covering parallel rebuilds and cancelling | [#18](https://github.com/spareware-co-op/spareware/issues/18) |
| A04 | Renderer: our own on wgpu, or three.js first | Decision note | Medium | Recommendation covering instancing, culling and browser support | [#19](https://github.com/spareware-co-op/spareware/issues/19) |
| A05 | Multi-user: server-ordered operation log or CRDT | Decision note | Medium | Recommendation covering offline work, conflicts and determinism | [#20](https://github.com/spareware-co-op/spareware/issues/20) |
| A06 | Project file: SQLite container or a custom chunk file | Decision note | Small | Recommendation covering crash safety, size and streaming | [#21](https://github.com/spareware-co-op/spareware/issues/21) |
| A07 | Family recipe format: JSON with a schema, or our own language | Decision note | Medium | Recommendation with a door family written both ways | [#22](https://github.com/spareware-co-op/spareware/issues/22) |
| A08 | Plugins: WebAssembly components on wasmtime | Research note | Medium | What plugin authors in Rust, C#, TypeScript and Python can use today | [#23](https://github.com/spareware-co-op/spareware/issues/23) |
| A09 | Study Pascal editor and ifc-lite | Research note | Medium | What we can reuse under their licences, and what to learn from | [#24](https://github.com/spareware-co-op/spareware/issues/24) |
| A10 | Study ArtCraft's repo layout and agent workflow | Research note | Small | How they build from scratch in Rust with agents, and what to copy | [#25](https://github.com/spareware-co-op/spareware/issues/25) |
| A11 | Sketch solver: how planegcs works (method only; it is LGPL) | Research note | Medium | The method described in our own words, with a port plan | [#26](https://github.com/spareware-co-op/spareware/issues/26) |
| A12 | Drawing engine: hidden line removal for plans and sections | Research note | Large | How open tools do it, and a recommended method | [#27](https://github.com/spareware-co-op/spareware/issues/27) |
| A13 | Clash: broad and narrow phase, true-distance clearance | Research note | Medium | Recommended method, with how clearance avoids false hits | [#28](https://github.com/spareware-co-op/spareware/issues/28) |
| A14 | Licence: Apache-2.0 for everything | Decision note | Small | Options compared for contributors and add-in authors; an owner decided Apache-2.0 on 2026-11-05 | [#29](https://github.com/spareware-co-op/spareware/issues/29) |
| A19a | Never-crash lints: construct-transport's core (jsnum, jsmath, util, project, graph, econ, analysis, importers, results, demand) | Build | Large | Clippy with the kernel's lint set raises nothing in these files, every fix changes code (an allow only as the kernel's rules allow one), `OrderedMap` has no panicking `Index`, and construct-transport's and construct-api's tests pass | [#822](https://github.com/spareware-co-op/spareware/issues/822) |
| A19b | Never-crash lints: construct-transport's calibrate module | Build | Large | As A19a, for `src/calibrate` | [#823](https://github.com/spareware-co-op/spareware/issues/823) |
| A19c | Never-crash lints: construct-transport's feeds module | Build | Large | As A19a, for `src/feeds` | [#824](https://github.com/spareware-co-op/spareware/issues/824) |
| A19d | Never-crash lints: construct-transport's compliance module | Build | Large | As A19a, for `src/compliance` | [#825](https://github.com/spareware-co-op/spareware/issues/825) |
| A19e | Never-crash lints: construct-transport's report module | Build | Large | As A19a, for `src/report` | [#826](https://github.com/spareware-co-op/spareware/issues/826) |
| A19f | Never-crash lints: construct-transport's assign, census, junction and junction_tools modules | Build | Large | As A19a, for those four modules | [#827](https://github.com/spareware-co-op/spareware/issues/827) |
| A19g | Never-crash lints: construct-transport's landuse, safety, sample, edit, studyarea, turns and datahub modules | Build | Large | As A19a, for those seven modules | [#828](https://github.com/spareware-co-op/spareware/issues/828) |
| A19h | Never-crash lints: construct-transport's tests, and construct-transport in CI's clippy | Build | Small | After A19a to A19g: each test file opens with the kernel's test allow block, and `construct-server.yml`'s clippy step lints construct-transport with `-D warnings` | [#829](https://github.com/spareware-co-op/spareware/issues/829) |