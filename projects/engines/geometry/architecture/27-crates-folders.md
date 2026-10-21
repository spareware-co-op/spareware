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
