# Spare Arch status

**Stage: Mapping.** Roadmap phase 0 (Foundations) has not started.

- Architecture map written: [ARCHITECTURE.md](ARCHITECTURE.md), 23 sections at high and low level.
- 21 decisions open (table near the end of the map). No code yet.
- Design stage starts when an owner accepts decision notes for the first six decisions: kernel start, storage layout, entity store, incremental engine, renderer, multi-user.

## Work board

Sizes: Small fits a little spare, Medium a fair amount, Large a lot. Status: Open, Claimed, In review, Handed over, Done.

| ID | Piece | Kind | Size | Done when | Status |
| --- | --- | --- | --- | --- | --- |
| A01 | Kernel: how Manifold does booleans, and the order to port it to Rust | Research note | Large | Names Manifold's method, data structures and a port order, with links into its source | Open |
| A02 | Entity store: our own or bevy_ecs | Decision note | Medium | Recommendation backed by both codebases, covering chunked storage and permanent IDs | Open |
| A03 | Incremental engine: the Salsa crate or our own | Decision note | Medium | Recommendation covering parallel rebuilds and cancelling | Open |
| A04 | Renderer: our own on wgpu, or three.js first | Decision note | Medium | Recommendation covering instancing, culling and browser support | Open |
| A05 | Multi-user: server-ordered operation log or CRDT | Decision note | Medium | Recommendation covering offline work, conflicts and determinism | Open |
| A06 | Project file: SQLite container or a custom chunk file | Decision note | Small | Recommendation covering crash safety, size and streaming | Open |
| A07 | Family recipe format: JSON with a schema, or our own language | Decision note | Medium | Recommendation with a door family written both ways | Open |
| A08 | Plugins: WebAssembly components on wasmtime | Research note | Medium | What plugin authors in Rust, C#, TypeScript and Python can use today | Open |
| A09 | Study Pascal editor and ifc-lite | Research note | Medium | What we can reuse under their licences, and what to learn from | Open |
| A10 | Study ArtCraft's repo layout and agent workflow | Research note | Small | How they build from scratch in Rust with agents, and what to copy | Open |
| A11 | Sketch solver: how planegcs works (method only; it is LGPL) | Research note | Medium | The method described in our own words, with a port plan | Open |
| A12 | Drawing engine: hidden line removal for plans and sections | Research note | Large | How open tools do it, and a recommended method | Open |
| A13 | Clash: broad and narrow phase, true-distance clearance | Research note | Medium | Recommended method, with how clearance avoids false hits | Open |
| A14 | Licence: MPL-2.0 core with Apache-2.0 SDK, or alternatives | Decision note | Small | Options compared for contributors and add-in authors; an owner decides | Open |
