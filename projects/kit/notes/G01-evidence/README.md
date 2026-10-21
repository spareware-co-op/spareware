# G01 evidence

The research behind [G01: the Spare Kit geometry kernel](../G01-geometry-kernel.md). Each report was written on 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of the projects it cites, pinned to exact commits, and the papers and public documents they rest on. Claims a report could not confirm from a primary source are marked unverified. GPL and LGPL code is described in the agents' own words and never copied.

The three vetting reports checked one project in depth: one agent evaluated each part against its source, and an independent skeptic then re-checked the facts and argued the opposite verdict. Both the evaluation and the challenge are kept.

| Report | Question it answers |
| --- | --- |
| [01 Manifold and mesh booleans](01-manifold-mesh-booleans.md) | How Manifold's boolean works, whether it is exact or deterministic, and what the best exact mesh booleans are (EMBER, Cherchi, Zhou, Lévy) |
| [02 OpenCascade, Parasolid and ACIS](02-occt-parasolid-acis.md) | How the most complete open B-rep kernel and the public concepts of the commercial ones handle topology, tolerance, booleans, intersection, validity, hidden lines, fillets and offsets, and where they fail |
| [03 Rust kernels and crates](03-rust-kernels-crates.md) | What Truck, Fornjot and other Rust efforts teach, which Rust crates are fit to depend on, and how to write a deterministic kernel in Rust |
| [04 NURBS and surfaces](04-nurbs-surfaces.md) | The curve and surface types a full kernel needs, the proven algorithm for each operation on them, and watertight tessellation |
| [05 Exact arithmetic and determinism](05-exact-arithmetic-determinism.md) | Exact predicates, constructions, the tolerance policy and grid, and how to get bit-identical results natively and in WebAssembly |
| [06 2D drafting and sketch solving](06-2d-drafting.md) | The AutoCAD-class 2D entity model, 2D booleans and offsets with arcs, arrangements, edit operations, snaps and constraint solvers |
| [07 Persistent naming and history](07-persistent-naming.md) | How to name faces, edges and vertices so references never break silently, and the history every operation must emit |
| [08 Standards, coverage and testing](08-standards-coverage-testing.md) | Every IFC 4.3 and STEP geometry type to support, what a Revit-class tool needs, meshes and clash, and how the best kernels are tested |
| [09 Free-form, subdivision and fields](09-freeform-subd-implicit.md) | Subdivision surfaces, implicit fields, conversions between representations, and the hard free-form operations |
| [10 Wide GitHub sweep](10-github-sweep.md) | Every other relevant open project, graded against the kernel's needs, and the gaps where no open project is good enough |
| [11 Vetting: projects suggested to the owner](11-vetting-suggested-projects.md) | Truck, cadcore, csgrs, GeoRust geo, RGeometry and SolidCraft, each claim checked against source |
| [12 Vetting: CADCraft](12-vetting-cadcraft.md) | Whether CADCraft's code or methods fit the kernel, its legal provenance, and what to learn from it |
| [13 Vetting: NVIDIA SMLib and solidcore](13-vetting-smlib.md) | Whether NVIDIA's newly open NURBS B-rep kernel, or its Rust port, should be the foundation, and what to take from it |
