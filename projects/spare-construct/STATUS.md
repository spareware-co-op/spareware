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
| A18 | Spare Construct AGENTS.md | Docs | Small | `projects/spare-construct/AGENTS.md` says what to read first, what each crate, the server and the app own, the commands that build and test each, and the program's own rules, adding to the root AGENTS.md and never weakening it | [#798](https://github.com/spareware-co-op/spareware/issues/798) |
| A19 | Never-crash lints for Spare Construct's crates and server | Build | Medium | Every Spare Construct crate and the server carry the kernel's `[lints]` tables, CI's clippy denies warnings over all of them, and every lint raised is fixed without an allow, except where the kernel's own rules allow one (a cast with its bound written beside it, test code) | [#799](https://github.com/spareware-co-op/spareware/issues/799) |
| A20 | 3D view: one instanced mesh per element type | Build | Small | The app's three.js view draws one instanced mesh per element type instead of one mesh per element, looks and behaves the same, and the pull request gives its draw calls before and after | [#800](https://github.com/spareware-co-op/spareware/issues/800) |
| A21 | Project locks: from the start, or families first | Decision note | Small | Recommendation on locks between elements across a project from the start, or constraints inside families only at first, starting from kernel chapter 15.15 and covering scale, how locks are logged and what waits on the kernel | [#801](https://github.com/spareware-co-op/spareware/issues/801) |
| A22 | View range: Revit's model or a simpler one | Decision note | Small | Recommendation on matching Revit's view range (top, cut plane, bottom, view depth) or a simpler model, covering plan regions, the planes passed to the kernel's drawing engine (chapter 18.2) and what each plan view stores | [#802](https://github.com/spareware-co-op/spareware/issues/802) |