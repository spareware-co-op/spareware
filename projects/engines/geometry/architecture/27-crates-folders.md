# 27. Crates and folders

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

The kernel lives in Spare Kit's engines layer, in its own folder, so that programs that do not use 3D (Spare Ride, Spare Food) never build it.

```text
projects/engines/
├─ crates/engine-geometry/        the stable public facade (exists today, empty)
└─ geometry/
   ├─ README.md                   start here: what the kernel is, how to find anything
   ├─ ARCHITECTURE.md             this note's decisions, kept current after acceptance
   ├─ AGENTS.md                   playbooks for adding operations, kinds, formats, toolkits
   ├─ GLOSSARY.md                 every term, defined once
   ├─ MAP.json                    generated map of crates, modules, operations, invariants, errors
   └─ crates/                     one folder per crate, folder name = crate name
```

The root `Cargo.toml` gains one member line, `projects/engines/geometry/crates/*`.

| Crate | One job | Depends on |
| --- | --- | --- |
| `geom-num` | Grid coordinates, wide integers, exact predicates and filters, intervals rounded outward, deterministic maths, the tolerance ladder | — |
| `geom-math` | Points, vectors, frames, exact and general transforms, boxes | num |
| `geom-nurbs` | NURBS algorithms: basis, evaluation, knot operations, degree change, fitting | num, math |
| `geom-curve` | Every curve kind in 2D and 3D: evaluation, bounds, projection, conversion | num, math, nurbs |
| `geom-surface` | Every surface kind: evaluation, bounds, projection, conversion | num, math, nurbs, curve |
| `geom-intersect` | Certified curve-curve, curve-surface and surface-surface intersection | num, math, curve, surface |
| `geom-topo` | Bodies to vertices in arenas; the builder; attributes; regions and cells | num, math, curve, surface |
| `geom-name` | Persistent names, history, reference resolution | num, topo |
| `geom-check` | The validity checker | topo, intersect |
| `geom-poly` | Exact plane-based polyhedra and their arrangements | num, math, topo |
| `geom-bool` | The arrangement engine: booleans, imprint, split, cells, batch cuts | poly, intersect, topo, name, check |
| `geom-sweep` | Extrude, revolve, sweep, loft, network surfaces, sectioned solids | topo, surface, name, check |
| `geom-blend` | Fillets, chamfers, vertex blends | bool, intersect, surface |
| `geom-offset` | Offsets, shells, thickening, drafts, face edits | bool, intersect, surface |
| `geom-2d` | Drafting entities, the 2D arrangement, regions, hatches, 2D booleans and offsets, edits, snaps | num, math, curve |
| `geom-sketch` | The constraint solver | num, math, curve |
| `geom-mesh` | Exact meshes, watertight tessellation, mesh booleans, levels of detail, repair | num, math, topo, poly |
| `geom-spatial` | Bounding-volume hierarchy, spatial queries, distance, clearance, clash | num, math, mesh |
| `geom-subd` | Catmull-Clark subdivision surfaces | num, math, nurbs, surface |
| `geom-field` | Implicit fields: expression graph, evaluation, interval bounds, meshing | num, math, mesh |
| `geom-cloud` | Point clouds as reference: index, snapping, fitting | num, math, spatial |
| `geom-terrain` | TINs from points and breaklines, contours, volumes, terrain solids | num, math, 2d, mesh, topo |
| `geom-align` | Alignments, spirals, stationing, corridors | num, math, curve, sweep |
| `geom-skeleton` | Straight skeletons and roof generation | num, math, 2d, topo |
| `geom-draw` | Sections, projections, hidden lines, drawing curves with provenance | topo, name, 2d, intersect, spatial |
| `geom-convert` | The conversion graph between representations | topo, mesh, subd, field, nurbs |
| `geom-format` | The native file format and its versions | everything stored |
| `geom-ifc`, `geom-step`, `geom-dxf`, `geom-3dm`, `geom-meshio` | Geometry mapping to and from each exchange format | format and the kinds they map |
| Domain toolkits, added when two programs need them: `geom-sheet` (sheet metal), `geom-thread`, `geom-board` (layered circuit boards), `geom-stair` and others | One domain each, built only from existing operations and kinds (section 26) | the operation crates they compose |
| `geom-test`, `geom-fuzz`, `geom-bench` | The shared test corpus and generators, fuzz targets, benchmarks (development only) | as needed |
| `engine-geometry` | The stable facade, its schemas and bindings | all of the above |

Rules for every crate, so the kernel stays readable at millions of lines:

- One job per crate, stated in the first lines of its `lib.rs`, with the invariants it keeps and the sections of this note it implements.
- One concept per module. A module starts with what it does, what it guarantees, the published method it follows (with a link), and its cost.
- Concrete types across crate boundaries; generics stay inside a crate, so compile times stay short.
- `#![forbid(unsafe_code)]`, and `clippy` denies `unwrap`, `expect`, `panic` and unchecked indexing in kernel code.
- Every tolerance and threshold comes from the ladder in `geom-num`, named and justified. No magic numbers.
- Files stay small enough to read whole. OpenCascade's 5,000-line files are the warning.

**Amendments, accepted 2026-10-28 (owner action G08, 32.13).** The folder tree, the crate table and the rules above copy G01 section 27 word for word, as G01 wrote them. The chapters written since proposed about twenty new crates, many new dependencies and some new rules. A cross-chapter review also found gaps in the tables themselves: no crate owned the types every operation returns, no crate was given a layer, and some things had two owners or none. Everything is listed here, so that one page shows the whole graph and its cycle check. The owner accepted every item as proposed, with chapter 30.5's four amendments, in 32.13: the crate table, the layers, the folder tree and the rules below supersede the ones above wherever they differ, and the chapters that proposed them have been brought into line. OQ-13.3 means chapter 13's open question 3, numbered the way chapter 11 numbers its own.

**What "Depends on" means.** The column lists every direct dependency, meaning every kernel crate named in the crate's own `Cargo.toml`. It does not list crates reached only through others, because Cargo gives a crate no access to those. Development crates never appear, except as development dependencies. CI reads every `Cargo.toml` and fails when one differs from this page, when an edge breaks the layer rule below, or when the graph has a cycle. This is the layering check chapter 14.14 takes from CADCraft. As built (G09), CI compares each `Cargo.toml` with a copy of the table in code, `engine-geometry/tests/scaffold/table.rs`, which also holds what this page does not: each crate's state (built, under way in another piece, or deferred), the edges waiting for a crate under way, and known gaps in headers. Nothing yet checks that this page and that file agree, so both change in the same pull request (31.7). The rows of G01's table above mix complete and partial lists (OQ-18.2); the accepted table below lists every direct dependency, except where a toolkit's design note will settle the rest.

**Layers.** Section 5's diagram has operations depend only on the core. The crate table above already has operations depending on operations: `geom-blend` and `geom-offset` on `geom-bool`, and `geom-align` on `geom-sweep`. The chapters add more, because some operations are services that others build on. Section 5 forbids depending on another operation's *internals*, not on its public interface (OQ-13.3). The rule, accepted in 32.13: **a crate depends only on crates in lower layers, or on a crate in its own layer along an edge this table allows, and only on that crate's public interface.** The graph stays acyclic. Programs still depend only on the facade.

| Layer, lowest first | Crates | Edges allowed inside the layer |
| --- | --- | --- |
| 1. Numbers and the contract | `geom-run`, `geom-num`, `geom-linalg`, `geom-op` | num on run; linalg on num; op on run and num |
| 2. Geometry | `geom-math`, `geom-bvh`, `geom-nurbs`, `geom-curve`, `geom-surface` | bvh and nurbs on math; curve on math and nurbs; surface on math, nurbs and curve |
| 3. Topology | `geom-topo`, `geom-name` | name on topo |
| 4. Intersections | `geom-intersect` | — |
| 5. Checker | `geom-check` | — |
| 6. Arrangements | `geom-poly`, `geom-bool`, `geom-2d` | bool on poly |
| 7. Meshes and spatial | `geom-mesh`, `geom-spatial` | spatial on mesh |
| 8. Other representations | `geom-subd`, `geom-field`, `geom-cloud` | — |
| 9. Shared operation services | `geom-skeleton`, `geom-sweep`, `geom-make` | sweep on skeleton |
| 10. Operations | `geom-blend`, `geom-offset`, `geom-sketch`, `geom-profile`, `geom-terrain`, `geom-align`, `geom-draw`, `geom-convert`, `geom-combine`, `geom-panel`, `geom-analysis` | profile on sketch; combine on convert |
| 11. Domain toolkits | `geom-sheet`, `geom-thread`, `geom-board`, `geom-stair`, `geom-civil`, `geom-assembly` and later ones | board on sheet (13.16) |
| 12. Format and exchange | `geom-format`, `geom-exchange`, `geom-p21`, `geom-heal`, and the format crates `geom-ifc`, `geom-step`, `geom-dxf`, `geom-3dm`, `geom-meshio`, `geom-landxml`, `geom-gerber`, `geom-ipc2581` | p21 on format; exchange on format and p21; heal on exchange; each format crate on format, exchange and heal; step and ifc also on p21 |
| 13. Facade | `engine-geometry`; beside it `engine-geometry-c`, `engine-geometry-wasm`, `engine-geometry-py` and `geom-pool` | engine-geometry on pool, behind its `threads` feature; each binding crate on engine-geometry |

This replaces section 5's three boxes (Numbers, Core, Operations) with thirteen layers. Section 5's diagram and layer table change to match, and so does chapter 13's word "core" for `geom-poly` and `geom-bool`, which this table puts in Arrangements.

**Who owns what, decided once.** Several things had two owners or none. Accepted in 32.13, one owner each:

- **The operation contract: a new `geom-op`.** Every operation returns the same contract (section 10.1), but no crate owns its types, and they must sit below every crate that uses them. `geom-sketch` returns `OpError`s naming `NameId`s (15.13) but sees no topology. `geom-intersect` returns exactness classes (12.2). `geom-poly`, `geom-2d` and `geom-mesh` order and emit names (11.3, 14.4, 19.2). `geom-name` sits above `geom-topo`, and putting the types in `geom-topo` (OQ-19.2) would hang topology under the sketch solver and the intersector, which use none. So `geom-op`, on `geom-run` and `geom-num` only, owns:
  - `OpContext`;
  - the shared parts of `OpOutput` (history, names, exactness, diagnostics), to which each crate adds its own result type (bodies, meshes, a solved sketch), the way each `OpError` refines its shared base;
  - the `OpError` base: `ErrorKind`, `Evidence`, `Remedy`;
  - the exactness class, `OpId` and `OpPath`;
  - the `NameId` value and its hashing under `NameScheme`; `TopoName`, `Role` and `PieceKey`;
  - the history records of 20.3, chapter 13's records of topology changes included;
  - the operation schema types (section 22).

  Evidence carries `geom-num` values (grid coordinates, parameters, intervals), so `geom-op` needs no geometry crate. `geom-name` keeps the name graph and its hash-consing, and references and their resolution (20.4, 17.12). The rule: a crate that only makes names and history depends on `geom-op`. A crate that resolves references also depends on `geom-name`: `geom-2d` (14.8), `geom-profile` (15.11), `geom-terrain` and `geom-align` (17.12), and those already listed with it above. So the `name` edges that chapters 13 and 16 add for naming alone (`geom-blend`, `geom-offset`, `geom-subd`, `geom-field`, `geom-convert`, `geom-panel`, `geom-analysis`) become `geom-op` edges, unless a crate's design note shows it resolves references.
- **Budgets and cancellation: `geom-run`.** `geom-run` holds the parallel helper, the budgets, the work meter and the `Cancel` trait (23.4, 23.10). `geom-num`'s work-list engine charges work and checks cancellation (12.3), so `geom-num` depends on `geom-run`, not on nothing. `IntersectCtx` (12.2) takes `geom-run`'s work meter and `Cancel` in place of its own `WorkUnits` and `CancelToken`.
- **The spatial index: a new `geom-bvh`.** Today the box tree has five homes, with different build and tie rules: `geom-math` (OQ-11.2, 23.7), a new `geom-bvh` (OQ-19.1), a Morton cell index in `geom-mesh::index` as a stopgap (19.9, 19.14), a bulk-loaded R-tree in `geom-2d` (14.8, 23.12) and the tree in `geom-spatial` (19.9). Fields keep a strut tree too (16.2). Accepted: one deterministic tree of integer boxes, 2D and 3D, in `geom-bvh` (run, num, math). It is built top-down in one pass by the binned surface-area heuristic with integer costs, ties broken by entry key (a persistent name, or an arena index inside one operation), and in parallel only through `geom-run`'s helper. Every answer is sorted by key and does not depend on the tree's shape (19.9, 23.7). `geom-bool`, `geom-intersect`, `geom-mesh`, `geom-2d`, `geom-terrain`, `geom-draw`, `geom-field` and `geom-spatial` all use it, and the cell-index stopgap is withdrawn. `geom-spatial` keeps distance, clearance, clash, instancing and contact over time. The alternative was a module of `geom-math`, which would then depend on `geom-run` (OQ-11.2, OQ-23.4); 32.13 declined it. EMBER's adaptive cells inside `geom-poly` (11.3) and the point clouds' out-of-core octree in `geom-cloud` (23.12) are parts of their algorithms, not general indexes, and stay where they are.
- **Linear algebra: a new `geom-linalg`.** 15.6 puts the deterministic solvers in `geom-num`; 16.6 proposes `geom-linalg` (OQ-15.7, OQ-16.1, OQ-29.16). Accepted: `geom-linalg`, on `geom-num` only, in layer 1, so `geom-nurbs` (fitting), `geom-sketch`, `geom-mesh`, `geom-cloud` and `geom-panel` share one implementation and `geom-num` stays numbers. The alternative, a module of `geom-num`, was declined (32.13).
- **Validity: each representation checks its own.** `geom-check` is the B-rep checker (8.3). The mesh group lives in `geom-mesh::exact` (19.2) and the drawing checker in `geom-draw` (18.10). Accepted: each crate's operations run their own representation's group as their postcondition (principle 4). The validity query across representations, which 19.2 puts in the facade, goes to `geom-combine`.
- **The facade: no kernel logic.** `engine-geometry` holds the operation schemas, the stable value types, re-exports of kernel crates' public items, and the conversion of `kit-core`'s permanent IDs and units at the boundary (section 5; the instance keys of 19.12). It holds no operation, query, dispatch, hashing, cache, or trait implementation that computes anything. Logic in the facade would sit in the Stable tier (section 25), where every change is a versioning event, and no internal crate could reach it: `geom-draw` and `geom-analysis` use `geom-spatial` directly, and would get no analytic distances. Where today's facade logic goes:

| Placed in the facade by a chapter | Home, accepted in 32.13 |
| --- | --- |
| The validity query over the B-rep and mesh groups (19.2) | `geom-combine` |
| Dispatch across representations (19.5) | `geom-combine`, as 11.8 already says |
| The trait through which `geom-spatial` reaches `geom-intersect`'s closed-form distances (19.10, OQ-19.7) | None: `geom-spatial` depends on `geom-intersect` directly, with no cycle, and on `geom-poly` for containment (19.11) |
| Content hashing, `CacheKey` and the reference memo store (23.8) | `geom-format`, which owns the canonical bytes they hash (24.2, 24.3) |
| The map from sketch items to persistent names (15.2) | `geom-sketch`, which holds `NameId` values through `geom-op` |
| Queries "in `engine-geometry` and the crate that owns the data" (26.1) | The crate that owns the data; the facade exposes its schema |

**The crate table, accepted in 32.13.** Every crate, with its layer from the table above, a job line rewritten from where the chapters place things, and every direct dependency. `base` stands for `geom-run`, `geom-num` and `geom-op` together. Other additions to the rows of G01's crate table above are in bold, as in 16.6, and chapter 30.5's amendments say "from 30.5". A `name` edge proposed only for naming is covered by `base`, by the rule above. A list may still be short where a crate will use a geometry crate it does not yet name; its design note adds the edge through this route.

| Crate | Layer | One job | Depends on | Proposed in |
| --- | --- | --- | --- | --- |
| `geom-run` (new) | 1 | The parallel helper and its serial executor, budgets, the work meter, the `Cancel` trait | — | 23.4, 23.10; OQ-23.4 |
| `geom-num` | 1 | Grid coordinates, wide integers, exact predicates and their generated filters, intervals rounded outward, the error-free transformations and double-double arithmetic (one type, `DD`, built in G25; `geom-math`'s own copy from G03 moves onto it), the budgeted work-list engine, `ExactSum`, the counter-based random generator, angle units, deterministic maths, the tolerance ladder | **run** | 12.3, 16.3, 17.2, 23.4 |
| `geom-linalg` (new) | 1 | Deterministic dense and sparse linear algebra: QR, the complete orthogonal decomposition, triangular solves, column ordering | num | 15.6, 16.6; OQ-15.7, OQ-16.1 |
| `geom-op` (new) | 1 | The operation contract every crate shares (above) | run, num | 10.1, 20, 22; OQ-19.2 |
| `geom-math` | 2 | Points, vectors, frames and their transformation records, exact, rational and general transforms (rational rotations included), boxes | num, **op** | 14.10, 17.1, 17.13 |
| `geom-bvh` (new) | 2 | The one tree of integer boxes, 2D and 3D | run, num, math | 19.9, 23.7; OQ-19.1 |
| `geom-nurbs` | 2 | NURBS algorithms: basis, evaluation, knot operations, degree change, fitting, Bernstein bounds, Bézier extraction, symbolic products, skinning, Gordon and Coons combination | base, math, **linalg** | 12.3, 12.12, 13 (intro), 15.6 |
| `geom-curve` | 2 | Every curve kind in 2D and 3D, spirals included: evaluation, bounds, projection, conversion | base, math, nurbs | 12.4, 17.8 |
| `geom-surface` | 2 | Every surface kind, the subdivision limit surface included (the net, its local rules, Stam evaluation and tables): evaluation, bounds, projection, conversion, extension, frame rules, flat maps of developable kinds, Class-A maths, the `ImplicitBound` trait | base, math, nurbs, curve | 12.1, 13 (intro), 13.9, 16.1, 16.6 |
| `geom-topo` | 3 | Bodies to vertices in arenas; the builder; attributes and continuity records; regions and cells; the instance record | base, math, curve, surface | 18.7, 23.9 |
| `geom-name` | 3 | The name graph and its hash-consing; references, their resolution and the walk through history; surface groups and placement references | base, topo | 17.12, 20.4 |
| `geom-intersect` | 4 | Certified curve-curve, curve-surface and surface-surface intersection; level curves, isoclines and contour generators; closed-form distances | base, math, **nurbs**, curve, surface, **bvh** | 12.3, 12.13, 18.5, 19.10, 19.14, 23.7; OQ-12.1, OQ-19.7 |
| `geom-check` | 5 | The B-rep validity checker | base, topo, intersect | 8.3, 18.10, 19.2 |
| `geom-poly` | 6 | Exact plane-based polyhedra and their arrangements; plane commit; indicators, early-out derivation and piece selection; the maximal-face merge with T-junction insertion, shared by `geom-bool` and `geom-mesh` | base, math, topo | 11.14, 13.2, 19.5; OQ-11.2, OQ-19.4 |
| `geom-bool` | 6 | The arrangement engine: booleans, imprint, split, cells, batch cuts, the arrangement step of local operations, `close_near_contacts` | base, poly, intersect, topo, name, check, **bvh** | 11.4, 11.14, 13 (intro) |
| `geom-2d` | 6 | Drafting entities, the 2D arrangement, regions, hatches, 2D booleans and offsets, edits, snaps and references, 2D distance and clearance, survey linework and COGO | base, math, curve, **topo, name, intersect, bvh** | 14 (intro), 14.8, 14.9, 17.2; OQ-12.1, OQ-14.9 |
| `geom-mesh` | 7 | Exact meshes, watertight tessellation, the exact constrained Delaunay triangulation, levels of detail and display meshes, mesh booleans and modelling, repair, volume meshes, parameterisation and flattening | base, math, topo, poly, **curve, surface, intersect, bvh, linalg, 2d** | 16.4, 16.6, 19.3, 19.14; OQ-16.1, OQ-19.2 |
| `geom-spatial` | 7 | Spatial queries on the box tree, distance, clearance, clash, contact over time, the instance tree | base, math, mesh, **bvh, poly, intersect** | 19.9 to 19.12; OQ-19.7, OQ-19.17 |
| `geom-subd` | 8 | The editable subdivision body: validation, whole-net refinement, net edits, PCCM patches | base, math, nurbs, surface | 16.1, 16.6; OQ-16.1 |
| `geom-field` | 8 | Implicit fields: expression graph, evaluation, interval bounds, gradients, field bodies, certified meshing and slicing, lattices, the signed heat method; implements `ImplicitBound` | base, math, mesh, **surface, 2d, bvh** | 12.1, 16.2, 16.6, 19.14; OQ-12.1, OQ-16.1 |
| `geom-cloud` | 8 | Point clouds as reference: the out-of-core index, snapping, primitive detection and fitting | base, math, spatial, **nurbs, surface, linalg** | 16.6, 17.10, 23.12; OQ-16.1, OQ-17.3 |
| `geom-skeleton` | 9 | Straight skeletons and roof generation | base, math, 2d, topo, **poly** | 13.11, 13.15; OQ-13.3 |
| `geom-sweep` | 9 | Extrude, revolve, sweep, loft, network surfaces, sectioned solids, corridor solids, swept volumes of moving tools | base, topo, surface, name, check, **poly, intersect, bool, skeleton** | 13.1 to 13.4, 13.17, 17.9; OQ-13.3, OQ-17.3 |
| `geom-make` (new, from 30.5) | 9 | The Make and Transform families of 10.2: primitives, transforms and patterns. They cannot live in `geom-topo`, because every operation runs the checker before it returns (principle 4) and `geom-check` sits above `geom-topo` | base, math, curve, surface, topo, check | 10.2, 30.5 |
| `geom-blend` | 10 | Fillets, chamfers, vertex blends | base, bool, intersect, surface, **topo, check** | 13.10; OQ-13.3 |
| `geom-offset` | 10 | Offsets, shells, thickening, drafts, face edits, defeaturing | base, bool, intersect, surface, **skeleton, topo** | 13.11, 13.13, 24.8; OQ-13.3 |
| `geom-sketch` | 10 | The constraint solver, in 2D and 3D | base, math, curve, **linalg** | 15.6, 15.15 |
| `geom-profile` (new) | 10 | Carrying geometry between sketches and the model: references in, regions out, selection by name, profile checks and names | base, sketch, 2d, topo, name, check, intersect | 15.14; OQ-15.3 |
| `geom-terrain` | 10 | TINs from points and breaklines, contours, volumes, terrain solids, grading, flow and sight, `OnSurface` references | base, math, 2d, mesh, topo, **poly, bool, skeleton, curve, surface, name, bvh** | 17.3 to 17.7, 17.11, 17.12; OQ-17.3 |
| `geom-align` | 10 | Alignments, stationing, the corridor model (ranges, stations, names), `Station` references. Spiral kinds are `geom-curve`'s and corridor solids are `geom-sweep`'s | base, math, curve, sweep, **name** | 17.8, 17.9, 17.12; OQ-17.3 |
| `geom-draw` | 10 | Sections, projections, hidden lines, drawing curves with provenance, the drawing checker | base, topo, name, 2d, intersect, spatial, **math, curve, surface, mesh, bvh** | 18.3, 18.10; OQ-18.2 |
| `geom-convert` | 10 | The conversion graph between representations, primitive recognition, the route query | base, topo, mesh, subd, field, nurbs, **check, bool, cloud, spatial, intersect, sweep** | 16.3, 16.6; OQ-16.1 |
| `geom-combine` (new) | 10 | Entry points across representations: the one boolean, dispatch, the validity query | base, bool, mesh, field, convert, **check, subd** | 11.8, 19.2, 19.5; OQ-11.2 |
| `geom-panel` (new) | 10 | Surface division, patterns, panels, mould grouping, unrolling | base, topo, check, surface, intersect, poly, mesh, 2d, linalg | 16.4, 16.6; OQ-16.1 |
| `geom-analysis` (new) | 10 | Surface quality: continuity, curvature, light lines, draft, gap and flush, deviation | base, surface, topo, intersect, spatial | 16.5, 16.6; OQ-16.1 |
| `geom-sheet`, `geom-thread`, `geom-board`, `geom-stair` | 11 | As above | base and the operation crates they compose; `geom-board` also **sheet** | 13.8, 13.16 |
| `geom-civil` (new) | 11 | Corridor daylighting, ground profiles by station, mass haul, sight distance | base, terrain, align | 17 (intro); OQ-17.4 |
| `geom-assembly` (new) | 11 | Assembly mates on placements, and moving mechanisms | base, sketch, spatial | 15.15 |
| `geom-format` | 12 | The native file format, its versions and canonical bytes; content hashes, `CacheKey` and the reference memo store | base and everything stored | 23.8, 24 |
| `geom-exchange` (new) | 12 | What every format crate shares: units as exact ratios, numbers to grid, frames from files, the ISO 10303-42 mapping, the hardened XML and zip reader, reports | base, math, curve, surface, topo (from its job; 24.8 lists none), and from 30.5 **format, p21, intersect, check**: its ISO 10303-42 mapping parses Part 21 entities and builds certified Tolerant bodies | 24.8, 30.5; OQ-24.6 |
| `geom-p21` (new) | 12 | The ISO 10303-21 clear-text encoding: tokenizer, streaming parser, canonical writer | base, and from 30.5 **format**, for the byte sources it parses | 24.8, 30.5; OQ-24.6 |
| `geom-heal` (new) | 12 | Healing and sewing Tolerant B-rep bodies; rounding imported polyhedra | base, topo, check, mesh, and from 30.5 **exchange**, whose numbers-to-grid rules rounding uses | 24.8, 24.9, 30.5; OQ-24.6 |
| `geom-ifc`, `geom-step`, `geom-dxf`, `geom-3dm`, `geom-meshio` | 12 | As above | base, format, **exchange, heal** and the kinds they map; `geom-ifc` and `geom-step` also **p21** | 24.8, 24.11 |
| `geom-landxml` (new) | 12 | LandXML 1.2 surfaces, alignments, profiles, COGO points and parcels | base, format, exchange, 2d, terrain, align | 17.13, 24.11; OQ-17.3, OQ-24.6 |
| `geom-gerber`, `geom-ipc2581` (new) | 12 | Circuit-board layers to and from Gerber X2 and X3, and IPC-2581 | base, format, exchange, 2d | 24.11 |
| `engine-geometry` | 13 | The stable facade: schemas, stable value types, re-exports; no kernel logic | Every kernel crate it re-exports, never a development crate; **geom-pool** behind its `threads` feature; `kit-core` (section 5) | 23.4, 25 |
| `engine-geometry-c`, `engine-geometry-wasm`, `engine-geometry-py` (new) | 13 | The C interface, the WebAssembly bindings, the Python bindings | engine-geometry | 25 |
| `geom-pool` (new) | 13 | Threads for `geom-run`'s helper: rayon on native targets, Web Workers in a browser | run | 23.4; OQ-23.4 |
| `geom-test`, `geom-fuzz`, `geom-bench` | Development | As above | As needed | 28.2 |
| `geom-gen` (new) | Development | Every generator whose output is checked in (below) | As needed | 6.2, 16.1, 23.3, 24.6, 24.11, 26.3, 29.20; OQ-24.6, OQ-28.6 |

**Cycle check.** Every edge in the table points to a lower layer, or along an edge the layer table allows inside one layer, and those same-layer edges form no loop. So the whole graph is acyclic. A script over this table checked both on 2026-10-21, and G09's scaffold check repeats it on every pull request, with 30.5's amendments.

**The folder tree, accepted in 32.13.** Entries marked "added" are new since G01; the rest is the tree above. `architecture/` existed but was missing from G01's tree. Section 26.3's file table has the same entries.

```text
projects/engines/
├─ crates/engine-geometry/        the stable public facade (exists today, empty)
├─ crates/engine-geometry-c/      added: the C interface
├─ crates/engine-geometry-wasm/   added: the WebAssembly bindings
├─ crates/engine-geometry-py/     added: the Python bindings
└─ geometry/
   ├─ README.md                   start here: what the kernel is, how to find anything
   ├─ ARCHITECTURE.md             this note's decisions, kept current after acceptance
   ├─ AGENTS.md                   playbooks for adding operations, kinds, formats, toolkits
   ├─ GLOSSARY.md                 every term, defined once
   ├─ MAP.json                    generated map of crates, modules, operations, invariants, errors
   ├─ NOTICE                      added: every origin licence text and credit, shipped with every binary (29.20)
   ├─ PROVENANCE.md               added: the source ledger, generated from the code (29.20)
   ├─ PATENTS.md                  added: the patent register (29.20)
   ├─ architecture/               added to the tree: the chapters, one file per section
   ├─ format/                     added: SPEC.md, REGISTRY.md and the CDDL schemas of the native format (24.2, 24.6)
   ├─ methods/                    added: method notes from the method-only procedure (29.20, rule 6)
   ├─ tools/format-reader/        added: the second reader of the native format, in Python, written from format/ alone (24.6)
   └─ crates/                     one folder per crate, folder name = crate name
```

The binding crates sit in `projects/engines/crates/`, which the root `Cargo.toml` already lists, so it still gains only the one member line above. The second reader lives in `geometry/tools/format-reader/`, not in the repository's root `tools/`, which holds `piece.py`: it is versioned with the format it checks, and kept apart from the Rust code it must not share.

**Rule changes, accepted in 32.13.**

- **Unsafe code stays out of the kernel and the facade.** A C interface needs `#[unsafe(no_mangle)]` items (the workspace is edition 2024) and raw pointers, which `#![forbid(unsafe_code)]` rejects. The WebAssembly and Python bindings export functions across the same kind of boundary. So the bindings move out of `engine-geometry` into `engine-geometry-c`, `engine-geometry-wasm` and `engine-geometry-py`. They are the only crates in the kernel's family that may contain `unsafe`: an audited exception, limited to the foreign-function boundary, where every `unsafe` block states why it is sound and a second reviewer checks it. They hold no geometry logic and reach the kernel only through `engine-geometry`, so section 25's rule still holds. `#![forbid(unsafe_code)]` stays in every kernel crate and in `engine-geometry`. The manifest lint `[lints.rust] unsafe_code = "forbid"` has the same effect: `geom-num` uses that form alone, and G09's scaffold check accepts either.
- **Kernel crates are `no_std`** (section 21), so the platform's maths functions are not even available. Every `geom-*` crate is `no_std` except `geom-pool`, because threads need `std` (23.4), and the development crates (`geom-test`, `geom-fuzz`, `geom-bench`, `geom-gen`). `engine-geometry` and the binding crates may use `std`, because they hold no kernel logic. Format crates stay `no_std` and read through the `no_std` traits of 24.7. Codecs that need `std` (las-rs, laz) stay in program adapters, as 24.11 says (OQ-29.15).
- **Checked arithmetic, and one lint posture.** Taken from what the built crates enforce: every fixed-width integer step in kernel code is a checked method, and no cast loses a value silently, so no answer depends on overflow checks being on (6.2). `clippy` denies `arithmetic_side_effects`, `cast_possible_truncation`, `cast_possible_wrap` and `cast_sign_loss` beside the four denials in the rules above, and runs its pedantic group with every warning denied in CI (`geom-num`, G02; `geom-math` also denies `cast_precision_loss`, G03). Arithmetic that a published method defines modulo a power of two, Philox's multiplications (G32) and the large accumulator's chunks in Neal's summation (G25), uses `wrapping_*` methods, each citing its specification. G09's scaffold check requires every crate that ships, the kernel crates, `geom-pool` and `engine-geometry`, to carry exactly `geom-num`'s `[lints]` table and `clippy.toml`, and leaves out the development and binding crates. G11's own test instead holds its development crates to `geom-num`'s table word for word, comments included. Every crate that ships carries the posture, and the development crates hold to it by G11's test. The binding crates carry it too, with `unsafe_code = "deny"` in place of `"forbid"`, so each audited block lifts it on the narrowest item with a `// SAFETY:` comment, and clippy's `undocumented_unsafe_blocks` denied (32.13).
- **Toolkits.** A toolkit may depend on another toolkit's public interface along an edge listed here, and the toolkit edges form no loop. Today there is one: `geom-board` reuses `geom-sheet`'s bends (13.16). Nothing below layer 11 depends on a toolkit. Section 26.1's two-program rule holds for every toolkit: it enters the kernel when a second program needs it, and until then it lives in the one program that does. `geom-assembly` (15.15), `geom-gerber` and `geom-ipc2581` (24.11) already say so, and `geom-stair` is the table's own example. Today only Spare Construct needs `geom-civil`. Giving it a kernel home now, as chapter 17 does, is an exception an owner records in section 32 (OQ-17.4); otherwise it moves into Spare Construct's crates, and no kernel crate changes.
- **Generated code and data have one home: `geom-gen`.** Several chapters generate code or data that is checked in, and none named a home for the generator. Accepted: one development-only crate holds every such generator. Its output is committed into the crate or folder that uses it, and CI reruns every generator and fails on any difference. No kernel crate depends on `geom-gen`, so development-only tools, such as the multiprecision library behind 16.1's tables or Verus (OQ-28.6), never enter a kernel build. Proposed here, it would also hold the dependency check above. As built, it does not: the crate-graph check is `engine-geometry`'s scaffold test (G09), and the rule that no kernel crate has a normal dependency on a development crate is in `geom-test` (G11). `geom-gen` holds the generators of committed output and their check, the first being the grid-step table (G11).

| Generator | Its output is committed into | Section |
| --- | --- | --- |
| Predicate filters, width-aware exact paths, bit-range certificates, Verus twins, FPTaylor and Gappa problems | `geom-num` | 6.2, 23.3, 28.6, 28.7; OQ-28.6 |
| Subdivision eigen tables, which 16.1 places in `geom-subd` | `geom-surface`, which uses them | 16.1 |
| EXPRESS types and validator tables; OQ-24.6's `geom-express` becomes this module | The format crates that use them | 24.11; OQ-24.6 |
| Field tables of the native format | `format/` | 24.6 |
| `MAP.json` and the invariant and error catalogues | `geometry/` | 26.3 |
| `PROVENANCE.md` and `NOTICE` | `geometry/` | 29.20 |

**Changes elsewhere, made with the acceptance (32.13).** These files copy or depend on this section and changed with it:

- G01 sections 5 and 27, and `ARCHITECTURE.md`: the diagram and layer table become the thirteen layers, and the tables above take the accepted items. G01 section 32 records each decision; chapter 14's introduction and 19.9 already point there.
- 12.2: `IntersectCtx` uses `geom-run`'s types.
- 13: the word "core" in the introduction; OQ-13.3's `name` edges; records of topology changes in `geom-op`.
- 14: the crate list in the introduction; the R-tree of 14.8 and 23.12 becomes `geom-bvh`.
- 15.2: the item-to-name map moves into `geom-sketch`. 15.6: the solvers move into `geom-linalg`.
- 16.1 and 16.6: the eigen-table generator moves into `geom-gen`; the `name` edges follow the rule above.
- 17: the introduction and OQ-17.3; `geom-civil` follows the toolkit rule.
- 19.2, 19.5, 19.9, 19.10, 19.14 and OQ-19.2, as the ownership list says. Chapter 19's build piece "Morton-sorted cell index in geom-mesh" moves to `geom-bvh`.
- 23.4, 23.7 and 23.8: `geom-num` depends on `geom-run`; the box tree is `geom-bvh`; hashing and the memo store move to `geom-format`.
- 24.6: the second reader's folder.
- 25: the bindings reach `engine-geometry` from their own crates.
- 26.1: the Query row. 26.3: the new file-table entries.
- 28.1: `geom-gen` beside the three development crates (28.1 already names it, because G11 has built it).
