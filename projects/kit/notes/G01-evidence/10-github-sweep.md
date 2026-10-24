*Evidence for [G01](../G01-geometry-kernel.md), the Spare Kit geometry kernel decision note. Written 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of each project at the pinned commits linked below. GPL and LGPL code is described in the agent's own words, never copied. Claims it could not confirm from a primary source are marked unverified.*

Wide sweep for the Spare Kit geometry kernel: what else exists, besides the nine areas the other agents cover

I checked every repo below with `gh api` (licence file, last commit on the default branch, stars) and read core source for the important ones. Licences come from the LICENSE or COPYING files, or from per-file SPDX headers where I say so. Anything I could not confirm is marked *unverified*. Activity is given as "last = date of last commit on the default branch · ★ = stars", as of 2026-10-21.

Most projects here are not best-in-class at anything that matters to a kernel. They wrap OCCT, use plain floats with tolerances, are academic one-offs, or carry a licence we cannot use. None of the C++ libraries promise bit-identical results between native and WASM. Most use TBB threads, the platform libm, or SIMD that varies by CPU. So nearly every useful item below is PORT or STUDY. DEPEND is limited to small pure-Rust crates and display or development tooling.

---

## 1. The 15 that genuinely matter

1. **GeometricTools (David Eberly). Boost Software License 1.0. PORT.**
   - It is the largest permissively licensed catalogue of geometric queries. GTE/Mathematics has 565 headers, including 112 intersection (`Intr*`), 77 distance (`Dist*`) and 19 containment (`Cont*`) files.
   - It also has `BSNumber`/`BSRational` exact arithmetic. `BSPrecision` works out at compile time how many bits a predicate needs. That lets the predicate run exactly in a fixed-size integer (`UIntegerFP32<N>`) with no heap allocation, which is the deterministic, allocation-free exact path we want.
   - Quality is high: one author, a consistent header-only C++ style, file-level version stamps, and a PDF derivation for most algorithms.
   - Weakness: most queries are written over `Real` in floating point, with exactness only where Eberly opted in. Port the derivations, then re-certify each one with our own predicates.

2. **Geogram (Bruno Lévy, Inria). BSD-3-Clause. PORT.** The other agent covers its exact predicates (PCK); the parts beyond PCK are:
   - `mesh_surface_intersection.*`: intersection vertices carry exact coordinates, with boolean classification.
   - `mesh_CSG*`: a compiler for a subset of OpenSCAD's `.csg` format, using those exact booleans.
   - `delaunay/CDT_2d.cpp`: a constrained 2D Delaunay triangulation with exact intersection points.
   - Parallel 3D Delaunay and CVT remeshing.
   - **The booleans agent should hear about this.** It is a second permissively licensed exact mesh-boolean code besides Cherchi and EMBER.

3. **Axom `primal` (LLNL). BSD-3-Clause. PORT.**
   - `winding_number.hpp` computes generalized winding numbers against Bézier and NURBS curves and patches. The package also has curved-polygon containment, clipping and intersection volume.
   - This is the best open method for deciding "inside or outside" a trimmed-NURBS model. It keeps that answer decidable when an imported STEP/IFC model has small gaps, which makes it a robustness fallback.

4. **libigl. MPL-2.0 core. PORT selected functions.**
   - Worth porting: fast winding numbers, signed distance, AABB trees.
   - Licence trap: `include/igl/copyleft` is GPL-3, and `include/igl/triangle` wraps Shewchuk's Triangle, which is not free. Because of `LICENSE.GPL`, GitHub labels the whole repo GPL-3.0. Port only from MPL files and cite each file path.

5. **JTS Topology Suite. EPL-2.0 or EDL-1.0 (the EDL text is BSD-3). PORT under EDL.**
   - OverlayNG with a snap-rounding noder, `IsValidOp` validity rules, and buffering, with about 20 years of hardening.
   - Use it as the reference and test oracle for 2D regions, next to Clipper2 and i_overlay (2D agent).
   - GEOS is the LGPL-2.1 C++ port of the same code, so read JTS rather than GEOS.

6. **spade. MIT OR Apache-2.0. DEPEND.**
   - Pure Rust: constrained Delaunay triangulation, Delaunay refinement, Voronoi diagrams and natural-neighbour interpolation, on Shewchuk's predicates via the `robust` crate.
   - It is the right base for terrain models (TINs) with breaklines.
   - startin (MIT) adds vertex deletion and five interpolation methods worth porting.

7. **Clothoids (Bertolazzi and Frego). BSD-2-Clause. PORT.**
   - G1 and G2 clothoid fitting, clothoid lists, distance, intersection and Fresnel integrals. It is the reference code for road and rail alignments.
   - Pair it with libOpenDRIVE and odrSpiral (both Apache-2.0) for OpenDRIVE road semantics.
   - Everything is double precision, so we add certified error bounds.

8. **Straight skeletons, which hip and gable roofs are built from. CGAL `Straight_skeleton_2` (GPL-3.0-or-later): STUDY. campskeleton (Apache-2.0): PORT the method.**
   - CGAL is the only robust implementation, and since 5.6 it handles weighted skeletons and their extrusion.
   - campskeleton (Tom Kelly) implements weighted skeletons with negative weights, the method behind procedural roofs and extrusions. It works in doubles with tolerances.
   - There is no permissively licensed robust implementation, so we must lead here (see section 3).

9. **The Hyper stack in the csgrs repo (Tim Schmidt). csgrs is MIT; the `hyperreal` crate is Apache-2.0. STUDY closely, and consider contacting the author.**
   - This is the closest philosophical match we found: in Rust, with the same aims.
   - `hyperreal`: exact rationals plus lazy computable reals.
   - `hyperlimit`: predicates return either a decided answer with how it was decided, or an explicit "Unknown". They never quietly return false or zero.
   - `hypermesh`: exact arrangement booleans on PWN meshes, with a STRICT policy and an APPROXIMATE_512 policy.
   - `HyperBRep`: an immutable typed-arena B-rep that refuses to publish an invalid model. Its booleans so far cover planes and cylinders only.
   - Caveats: 100 commits in the last 30 days; about 10.3k commits by one maintainer; 35 of the last 100 commits carry AI-assistant markers. It is at version 0.x, the scope sprawls into circuits, physics and packing, and the speed of its computable reals is unproven. It is a design peer, not a dependency.

10. **Sketch constraint solvers in Rust. ezpz (Zoo): MIT, a candidate to DEPEND on after an audit. fiksi (Linebender): Apache-2.0 OR MIT, STUDY or PORT.**
    - ezpz: sparse Newton solving on `faer`, degrees-of-freedom analysis (`find_dof.rs`), `libm` for transcendental functions (good for determinism), a WASM crate, and property tests.
    - fiksi: breaks the problem into a graph and assembles it recursively, solves with Levenberg–Marquardt and L-BFGS, and has its own sparse Cholesky/QR and a COLAMD port.
    - These are the only permissive Rust lineages. planegcs is LGPL and SolveSpace is GPL.

11. **BRL-CAD. LGPL for the libraries. STUDY.**
    - Its ray-evaluated CSG never "fails", because it never builds a boundary.
    - `src/libged/facetize/CMakeLists.txt` now requires `manifold::manifold`, with screened Poisson reconstruction from ray samples as a fallback. That is a working example of graded fallbacks.
    - libbrep's NURBS surface–surface intersection is tolerance-based and not a model to copy.

12. **Blender's exact mesh boolean (Howard Trickey). GPL-2.0-or-later per SPDX header. STUDY the method only.**
    - `mesh_intersect.cc` (3,135 lines) and `mesh_boolean.cc` (3,755 lines), compiled under `WITH_GMP`.
    - Method: exact GMP rational coordinates, then patches, then cells, then classification by winding number.
    - Same family as Cherchi and EMBER; useful as a third design to compare.

13. **The wildmeshing family. fTetWild (MPL-2.0), wildmeshing-toolkit (MIT), topological-offsets (MIT). PORT later.**
    - fTetWild: an envelope method that copes with dirty input.
    - wildmeshing-toolkit: mesh edits are declared as local operations with invariant checks and rollback. This is the right pattern for "never an invalid solid".
    - Topological Offsets (ACM TOG 2025): offsets whose topology is guaranteed.

14. **Sweeps. Lifted Surfacing of Generalized Sweep Volumes (SIGGRAPH Asia 2025): Apache-2.0, PORT candidate. Sellán's swept-volumes: MIT, STUDY.**
    - Lifted Surfacing produces watertight, self-intersection-free sweep boundaries from any time-varying implicit shape. Uses include clearance envelopes, 3D door swings and machine paths.

15. **build123d. Apache-2.0. STUDY.**
    - The best open code-CAD API for both people and AIs: builder and algebra modes, typed selectors.
    - Its weakness is that selectors break when topology changes, which is the persistent-naming problem.

Display only: wgpu, vello, lyon and kurbo (all MIT OR Apache-2.0) and meshoptimizer (MIT) are good to DEPEND on for display. They must never be the kernel's source of truth, because GPU floating point is not bit-identical across vendors. kurbo has a `libm` feature, which matters if we use its curve maths inside the kernel.

---

## 2. Full table

### General geometry and algorithm libraries

| Project | Licence (exact) | Activity | Best at; quality | Verdict |
|---|---|---|---|---|
| [GeometricTools](https://github.com/davideberly/GeometricTools) | BSL-1.0 | last 2026-10-12 · ★1.4k | Queries plus BSNumber/BSRational/BSPrecision; excellent, documented | **PORT** |
| [Geogram](https://github.com/BrunoLevy/geogram) | BSD-3-Clause | last 2026-10-20 · ★2.6k | Exact mesh intersection and CSG, exact CDT_2d, 3D Delaunay, CVT; excellent | **PORT** |
| [Axom (LLNL)](https://github.com/LLNL/axom) | BSD-3-Clause | last 2026-10-20 · ★197 | Generalized winding numbers for NURBS curves and patches; lab-grade but HPC-heavy | **PORT** (primal only) |
| [libigl](https://github.com/libigl/libigl) | MPL-2.0 core; GPL-3 in `igl/copyleft`; Triangle wrapper is non-free | last 2026-09-04 · ★5.1k | Winding numbers, signed distance; one function per file | **PORT** (MPL files only) |
| [Geometry Central](https://github.com/nmwsharp/geometry-central) | MIT | last 2026-06-13 · ★1.3k | Intrinsic triangulations, signed heat method, quadric simplification, remeshing; very clean | **PORT** (signed heat method for inside/outside on broken meshes) |
| [CinoLib](https://github.com/mlivesu/cinolib) | MIT | last 2026-09-11 · ★1.1k | Any polygonal or polyhedral cell in one structure; optional hooks to Triangle (non-free) and TetGen (AGPL) | **STUDY** (data structure) |
| [PMP library](https://github.com/pmp-library/pmp-library) | MIT | last 2026-08-28 · ★1.5k | Compact half-edge mesh; remeshing, decimation, hole filling, fairing; clean | **PORT** (mesh utilities) |
| OpenMesh (RWTH GitLab) | BSD-3 *unverified* | not checked | PMP is its successor | **REJECT** |
| [vcglib](https://github.com/cnr-isti-vclab/vcglib) / MeshLab | GPL-3.0 | last 2026-07-04 | Broad mesh processing | **STUDY** |
| [MeshLib](https://github.com/MeshInspector/MeshLib) | Proprietary, non-commercial and education only | last 2026-10-20 | Fast booleans and repair | **REJECT** (licence) |
| [MeshFix V2.1](https://github.com/MarcoAttene/MeshFix-V2.1) | GPL-3 (gpl-3.0.txt; no LICENSE file) | last 2026-05-21 | Mesh repair | **STUDY** |
| [trimesh](https://github.com/mikedh/trimesh) | MIT | last 2026-10-02 · ★3.7k | Python mesh tooling | **DEPEND** (dev and test harness only) |
| [Polyscope](https://github.com/nmwsharp/polyscope) | MIT | last 2026-09-06 | Debug viewer | **DEPEND** (dev only) |
| [rerun](https://github.com/rerun-io/rerun) | MIT OR Apache-2.0 | last 2026-10-20 · ★11.6k | Rust-native visual debugging and logging | **DEPEND** (dev only; good for visual regression tests) |
| [gptoolbox](https://github.com/alecjacobson/gptoolbox) | MIT and Apache-2.0 licence files | last 2026-10-09 | MATLAB prototypes | **STUDY** |
| [GoTools (SINTEF)](https://github.com/SINTEF-Geometry/GoTools) | AGPL-3.0 | last 2025-11-25 | Spline intersection, trimmed volumes | **STUDY** |
| IRIT (Elber) | Free for non-commercial use only (user manual); no official GitHub found | — | Free-form algorithms | **REJECT** (read the papers instead) |
| [Hyper / csgrs](https://github.com/timschmidt/csgrs) | MIT (csgrs); Apache-2.0 (hyperreal) | last 2026-10-20 · ★261 | Certified predicates, exact arrangement booleans, typed-arena B-rep; rigorous docs, 0.x, one maintainer | **STUDY closely** |
| [baby_shark](https://github.com/dima634/baby_shark) | MIT | last 2025-09-14 · ★98 | Rust mesh processing | **REJECT** (nothing best-in-class) |
| [parry](https://github.com/dimforge/parry) | Apache-2.0 | last 2026-10-06 | Rust BVH, ray casts, distance | **STUDY** (defer to the Rust building-blocks agent) |
| [FCL](https://github.com/flexible-collision-library/fcl) / [coal](https://github.com/coal-library/coal) | BSD-3-Clause / BSD-3-Clause | last 2026-09-09 / 2026-10-20 | GJK/EPA distance, clearance | **STUDY** (for clash piece A13) |
| [Embree](https://github.com/RenderKit/embree) | Apache-2.0 | last 2026-10-15 · ★2.8k | Fastest CPU ray tracing; no WASM, CPU-specific SIMD | **REJECT** as a dependency; STUDY its BVH builders |

### Booleans and CSG beyond Manifold, EMBER and Cherchi

| Project | Licence | Activity | Best at; quality | Verdict |
|---|---|---|---|---|
| [Blender](https://github.com/blender/blender) `blenlib/intern/mesh_boolean.cc`, `mesh_intersect.cc` | GPL-2.0-or-later (SPDX) | last 2026-10-20 | Exact GMP rational booleans; Trickey's cell classification | **STUDY** (method only) |
| [OpenSCAD](https://github.com/openscad/openscad) | GPL-2.0 (COPYING, with a CGAL linking exception) | last 2026-10-18 · ★10.5k | Manifold bundled as a submodule (`src/geometry/manifold`); large CSG test corpus | **STUDY** |
| [BRL-CAD](https://github.com/BRL-CAD/brlcad) (read via [mirror](https://github.com/brlcad/BRL-CAD); the main repo returned 404 to our API token) | LGPL for libraries; BSD or public domain for build files and data (COPYING); exact LGPL version *unverified* | last 2026-10-12 · ★1.0k | Ray-evaluated CSG; facetize built on Manifold with Poisson fallback; tolerance-based NURBS SSI | **STUDY** |
| [JSketcher](https://github.com/xibyte/jsketcher) | Not OSI: MIT text plus a requirement to assign copyright of modifications to Autodrop3d LLC | last 2023-10-30 · ★1.7k | 1,332-line tolerance-based B-rep boolean (`eqTol`/`TOLERANCE`); real modelling moved to OCCT in WASM | **STUDY** (weak) |
| [csg.js](https://github.com/evanw/csg.js) | MIT | last code 2012-08-20 | BSP booleans: not robust, polygons fragment heavily | **REJECT** |
| [JSCAD](https://github.com/jscad/OpenJSCAD.org) | MIT | last 2026-02-22 · ★3.3k | Same BSP lineage as csg.js | **STUDY** (API only) |
| [three-bvh-csg](https://github.com/gkjohnson/three-bvh-csg) | MIT | last 2026-09-26 | Fast BVH-accelerated floating-point CSG; not robust | **REJECT** for the kernel; STUDY for interactive preview |
| [mcut](https://github.com/cutdigital/mcut) | LGPL-3.0, or commercial | last 2025-09-04 | Mesh cutting | **STUDY** |
| [Cork](https://github.com/gilbo/cork) | LGPL-3.0, bundles non-free Triangle | last 2016-03-29 | Historic | **REJECT** |

### B-rep apps and code-CAD

| Project | Licence | Activity | Best at; quality | Verdict |
|---|---|---|---|---|
| [chili3d](https://github.com/xiangechen/chili3d) | AGPL-3.0 | last 2026-09-19 · ★4.9k | Browser CAD on OCCT WASM | **REJECT** the code; STUDY the UX |
| [replicad](https://github.com/sgenoud/replicad) | MIT | last 2026-09-04 | OCCT WASM code-CAD | **STUDY** (API) |
| [CadQuery](https://github.com/CadQuery/cadquery) | Apache-2.0 | last 2026-10-18 · ★6.0k | OCCT Python API; large test suite | **STUDY** (API, selectors, test cases) |
| [build123d](https://github.com/gumyr/build123d) | Apache-2.0 | last 2026-10-20 · ★3.5k | Best code-CAD API design | **STUDY** |
| [Dune 3D](https://github.com/dune3d/dune3d) | GPL-3.0 | last 2026-10-17 · ★2.1k | OCCT plus SolveSpace solver; workplane UX | **STUDY** (UX) |
| [CADmium](https://github.com/CADmium-Co/CADmium) | Elastic License 2.0; archived | last 2025-09-05 | — | **REJECT** |
| [Zoo modeling-app](https://github.com/KittyCAD/modeling-app) (KCL language) | MIT | last 2026-10-20 · ★1.3k | KCL is open (`rust/kcl-lib`); the geometry engine is closed (README: the 3D view is a video stream from their hosted engine) | **STUDY** (language) |
| [datum](https://github.com/Artis-98/datum) | MIT | last 2026-10-15 · ★16 | Small Windows parametric CAD | **REJECT** (immature) |

### Constraint solvers

| Project | Licence | Activity | Best at; quality | Verdict |
|---|---|---|---|---|
| [ezpz](https://github.com/KittyCAD/ezpz) | MIT | last 2026-08-05 · ★53 | Sparse Newton on faer, DOF analysis, libm, WASM, property tests | **DEPEND candidate / PORT** |
| [fiksi](https://github.com/endoli/fiksi) | Apache-2.0 OR MIT | last 2026-05-02 · ★14 | Graph decomposition, LM and L-BFGS, own sparse linear algebra; early | **STUDY / PORT** |
| [kiwi (Cassowary)](https://github.com/nucleic/kiwi) | BSD-3-Clause | last 2026-10-16 | Linear constraints only | **REJECT** for sketches (possible for sheet layout) |
| [Frontier](https://github.com/Geoplexity/Frontier) | GPL-3.0 | last 2019-03-01 | Sitharam's DR-planning decomposition | **STUDY** (method) |
| [NoteCAD](https://github.com/NoteCAD/NoteCAD) | No licence file (all rights reserved) | last 2026-03-17 | — | **REJECT** |
| [2d_geometric_constraint_solver](https://github.com/AntonEvmenenko/2d_geometric_constraint_solver), [dlineate](https://github.com/marcuswu/dlineate) | MIT / MIT | 2026 | Toy-level | **REJECT** |

### Implicit / SDF (beyond libfive and Fidget, which another agent covers)

| Project | Licence | Activity | Verdict |
|---|---|---|---|
| [Curv](https://github.com/curv3d/curv) | Apache-2.0; archived | last 2023-09-27 | **STUDY** (SDF language) |
| [ImplicitCAD](https://github.com/Haskell-Things/ImplicitCAD) | AGPL-3.0 | last 2026-03-20 | **STUDY** |
| [SDFX](https://github.com/deadsy/sdfx) | MIT | last 2026-08-29 | **STUDY** (Go; weak) |
| [fogleman/sdf](https://github.com/fogleman/sdf) | MIT | last 2024-08-10 | **STUDY** |

### Volumes, tetrahedral meshing, remeshing

| Project | Licence | Activity | Best at; quality | Verdict |
|---|---|---|---|---|
| [OpenVDB](https://github.com/AcademySoftwareFoundation/openvdb) | Apache-2.0 (now; formerly MPL-2.0) | last 2026-10-20 · ★3.4k | Sparse level sets, CSG on level sets; C++/TBB | **PORT** concepts later |
| [fTetWild](https://github.com/wildmeshing/fTetWild) | MPL-2.0 | last 2026-09-23 | Robust tetrahedralization of dirty input | **PORT** later |
| [TetWild](https://github.com/Yixin-Hu/TetWild) | MPL-2.0, but contains CGAL GPL code (README) | last 2023-04-27 | Rational-number version | **STUDY** |
| [wildmeshing-toolkit](https://github.com/wildmeshing/wildmeshing-toolkit) | MIT | last 2026-10-20 | Local operations with invariants and rollback | **STUDY** (architecture) |
| [Gmsh](https://gmsh.info/LICENSE.txt) | GPL-2.0-or-later with linking exception | active | FEM meshing | **STUDY** |
| [Netgen](https://github.com/NGSolve/netgen) | LGPL-2.1 | last 2026-10-19 | FEM meshing | **STUDY** |
| [Mmg](https://github.com/MmgTools/mmg) | LGPL-3.0-or-later | last 2026-01-20 | Remeshing, level-set discretization | **STUDY** |
| [PolyFem](https://github.com/polyfem/polyfem) | MIT | last 2026-10-13 | Finite-element analysis | **REJECT** for the kernel (later analysis module) |
| [Instant Meshes](https://github.com/wjakob/instant-meshes) | BSD-3-Clause | last code 2019-11-03 | Field-aligned quad remeshing | **PORT** if quad remeshing is needed |

### Offsets and sweeps

| Project | Licence | Activity | Verdict |
|---|---|---|---|
| [topological-offsets](https://github.com/wildmeshing/topological-offsets) (Zint et al. 2025) | MIT | last 2025-04-14 | **PORT** candidate |
| [voroffset](https://github.com/geometryprocessing/voroffset) | MIT | last 2024-01-17 | **STUDY** |
| [offsets-and-remeshing](https://github.com/daniel-zint/offsets-and-remeshing) | GPL-3.0 | last 2024-07-28 | **STUDY** |
| [PCO](https://github.com/Alan-Leo-Wong/PCO) | GPL-3.0 | last 2025-03-06 | **STUDY** |
| [OffsetCrust](https://github.com/zih-an/offsetcrust) | No licence | last 2026-08-01 | **REJECT** |
| [PFPOffset](https://github.com/iGame-Lab/PFPOffset) | BSD-3-Clause | last 2023-10-19 | **STUDY** (weak) |
| [swept-volumes (Sellán)](https://github.com/sgsellan/swept-volumes) | MIT | last 2024-07-18 | **STUDY / PORT** |
| [Lifted Surfacing sweeps](https://github.com/Jurwen/Swept-Volume) | Apache-2.0 | last 2026-01-16 | **PORT** candidate |
| [offset-mesh-survey](https://github.com/Bigger-and-Stronger/offset-mesh-survey) | no licence (reading list) | 2026-09-25 | Useful index |

### Straight skeletons and roofs

| Project | Licence | Activity | Best at; quality | Verdict |
|---|---|---|---|---|
| [CGAL Straight_skeleton_2](https://github.com/CGAL/cgal) | GPL-3.0-or-later (package `license.txt`) | last 2026-10-17 | Exact; weighted skeletons and extrusion since 5.6 | **STUDY** |
| [surfer2](https://github.com/cgalab/surfer2) | GPL-3 | last 2021-02-25 | Triangulation-based method (Palfrader and Held) | **STUDY** |
| [campskeleton](https://github.com/twak/campskeleton) | Apache-2.0 | last 2023-10-03 | Weighted, negative weights, procedural extrusions; doubles with tolerances | **PORT** method |
| [polyskel](https://github.com/Botffy/polyskel) | LGPL-3.0 | last 2020-10-12 | Felkel's algorithm; known bugs | **STUDY** |
| [StrandedKitty/straight-skeleton](https://github.com/StrandedKitty/straight-skeleton) | MIT repo, but wraps CGAL (GPL) via WASM | last 2026-03-24 | — | **REJECT** |
| [kendzi-math](https://github.com/kendzi/kendzi-math) | BSD-3-Clause | last 2016-02-21 | Java Felkel | **STUDY** |
| [geo-buffer](https://github.com/1011-git/geo-buffer) | Apache-2.0 | last 2023-07-07 | Rust buffering via skeleton | **STUDY** |
| [raybevel](https://github.com/tylermorganwall/raybevel) | No licence file | last 2026-03-22 | — | **REJECT** |
| [streets-gl](https://github.com/StrandedKitty/streets-gl) | MIT | last 2025-08-21 | Roof shapes from OpenStreetMap | **STUDY** |

### Clothoids and road alignment

| Project | Licence | Activity | Verdict |
|---|---|---|---|
| [Clothoids](https://github.com/ebertolazzi/Clothoids) | BSD-2-Clause | last 2026-07-14 | **PORT** |
| [G1fitting](https://github.com/ebertolazzi/G1fitting) | BSD-2-Clause | last 2026-03-24 | **PORT** (subset) |
| [odrSpiral (DLR)](https://github.com/DLR-TS/odrSpiral) | Apache-2.0 | last 2025-05-13 | **PORT** (small reference) |
| [libOpenDRIVE](https://github.com/pageldev/libOpenDRIVE) | Apache-2.0 | last 2025-12-31 | **STUDY / PORT** (lane geometry) |
| [pyclothoids](https://github.com/phillipd94/pyclothoids) | MIT | last 2025-05-11 | **REJECT** (wrapper) |
| [qaws](https://github.com/soufianekhiat/qaws) | MIT | last 2026-03-12 | **REJECT** (nothing best-in-class) |
| [clothoid-rs](https://github.com/sunsided/clothoid-rs) | No licence | last 2026-04-28 | **REJECT** |
| [openbimrs/ifc](https://github.com/openbimrs/ifc) (`ifc-alignment` crate) | AGPL-3.0 | last 2026-10-20 | **REJECT** |

### Terrain models (TINs) and terrain

| Project | Licence | Activity | Verdict |
|---|---|---|---|
| [spade](https://github.com/Stoeoef/spade) | MIT OR Apache-2.0 | last 2026-03-24 · ★340 | **DEPEND** |
| [startin](https://github.com/hugoledoux/startin) | MIT | last 2025-07-28 | **STUDY / PORT** (vertex deletion, interpolation) |
| [Delaunator](https://github.com/mapbox/delaunator) / [delaunator-rs](https://github.com/mourner/delaunator-rs) | ISC / ISC | 2026 | **REJECT** (no constraints) |
| [martini](https://github.com/mapbox/martini) / [delatin](https://github.com/mapbox/delatin) | ISC / ISC | 2025 | **STUDY** (display level of detail) |
| [hmm](https://github.com/fogleman/hmm) | MIT | last 2020-12-07 | **STUDY** |
| [tin-terrain](https://github.com/heremaps/tin-terrain) | MIT; archived | last 2023-03-08 | **REJECT** |
| [hexasphere.js](https://github.com/arscan/hexasphere.js) | No licence; irrelevant (hex-tiled sphere generator) | 2021 | **REJECT** |

### Point clouds

| Project | Licence | Activity | Verdict |
|---|---|---|---|
| [PDAL](https://github.com/PDAL/PDAL) | BSD-3-Clause | last 2026-10-20 | **DEPEND** as an external ingest tool only |
| [Open3D](https://github.com/isl-org/Open3D) | MIT | last 2026-09-16 · ★14.1k | **PORT** selected (RANSAC plane detection, ICP registration) |
| [PCL](https://github.com/PointCloudLibrary/pcl) | BSD-3-Clause | last 2026-10-17 | **STUDY** |
| [PoissonRecon](https://github.com/mkazhdan/PoissonRecon) | MIT | last 2026-04-29 | **PORT** (point cloud to mesh) |
| [las-rs](https://github.com/gadomski/las-rs) | MIT | last 2026-10-19 | **DEPEND** (LAS/LAZ reading and writing) |
| [Potree](https://github.com/potree/potree) | BSD-2-Clause | last 2026-01-08 | **STUDY** (octree streaming) |
| [CloudCompare](https://github.com/CloudCompare/CloudCompare) | GPL-2.0-or-later | last 2026-10-17 | **STUDY** |

### Polygon triangulation

| Project | Licence | Activity | Verdict |
|---|---|---|---|
| [earcut](https://github.com/mapbox/earcut) / [georust/earcut](https://github.com/georust/earcut) | ISC / MIT, Apache-2.0 and ISC licence files | 2026 | **DEPEND** for display only (not robust) |
| [poly2tri](https://github.com/jhasse/poly2tri) | BSD-3-Clause | last 2026-10-20 | **REJECT** (fails on duplicate and collinear points) |
| [libtess2](https://github.com/memononen/libtess2) | SGI Free Software License B 2.0 | last 2025-10-15 | **STUDY** (winding rules) |
| [CDT (artem-ogre)](https://github.com/artem-ogre/CDT) | MPL-2.0 | last 2026-10-13 | **STUDY** (reference; spade covers this in Rust) |
| [iTriangle](https://github.com/iShape-Rust/iTriangle) | MIT OR Apache-2.0 | last 2026-10-14 | **STUDY** (integer coordinates; pairs with i_overlay, 2D agent) |

### Hidden lines and drawing generation

| Project | Licence | Activity | Best at; quality | Verdict |
|---|---|---|---|---|
| [Krbn](https://github.com/vpalos/Krbn) | MIT | last 2026-07-25 · ★240 | Exact silhouettes of quadrics in closed form; visible/hidden split analytically; byte-identical SVG on a given platform | **STUDY** |
| [ln](https://github.com/fogleman/ln) | MIT | last 2017-02-23 | Visibility by sampling and ray casts | **REJECT** |
| [hidden-lines (Rhino HL)](https://github.com/kenny-t-vo/hidden-lines) | MIT | last 2026-10-20 | Raster ID buffer plus sampling (not exact); good line-class and snap-rounding rules for architectural drawings | **STUDY** (rules only) |
| Blender Freestyle | GPL | — | — | **STUDY** |
| [ezdxf](https://github.com/mozman/ezdxf) | MIT | last 2026-10-20 | DXF semantics | **STUDY** |
| [LibreDWG](https://github.com/LibreDWG/libredwg) | GPL-3.0 | last 2026-10-13 | DWG reading | **STUDY** |

### GPU and display

| Project | Licence | Verdict |
|---|---|---|
| [wgpu](https://github.com/gfx-rs/wgpu) | MIT OR Apache-2.0 | **DEPEND** (renderer) |
| [vello](https://github.com/linebender/vello) | MIT OR Apache-2.0 | **DEPEND** (2D display) |
| [lyon](https://github.com/nical/lyon) | MIT OR Apache-2.0 | **DEPEND** (path tessellation for display) |
| [kurbo](https://github.com/linebender/kurbo) | Apache-2.0 OR MIT | **DEPEND / PORT** (has `libm` feature) |
| [meshoptimizer](https://github.com/zeux/meshoptimizer) | MIT | **DEPEND** (levels of detail, meshlets) |
| [nanite-webgpu](https://github.com/Scthe/nanite-webgpu) | MIT | **STUDY** |

### 2D / GIS

| Project | Licence | Verdict |
|---|---|---|
| [JTS](https://github.com/locationtech/jts) | EPL-2.0 OR EDL-1.0 (BSD-3 text) | **PORT** under EDL |
| [GEOS](https://github.com/libgeos/geos) | LGPL-2.1 | **STUDY** |
| [geo](https://github.com/georust/geo) | MIT OR Apache-2.0 | **DEPEND** for the site/GIS layer (uses i_overlay, robust, spade) |

### BIM data and apps

| Project | Licence | Verdict |
|---|---|---|
| [Speckle SDK](https://github.com/specklesystems/speckle-sharp-sdk) / [server](https://github.com/specklesystems/speckle-server) | Apache-2.0; two server folders under Speckle's "Enterprise Edition" licence | **STUDY** (Rhino-like Brep/Trim schema; native-blob `BrepX`/`SolidX`; no algorithms) |
| [XbimEssentials](https://github.com/xBimTeam/XbimEssentials) / [XbimGeometry](https://github.com/xBimTeam/XbimGeometry) | CDDL-1.0 (not on our list); geometry is OCCT-based | **REJECT** |
| [That Open engine_components](https://github.com/ThatOpen/engine_components) / [fragments](https://github.com/ThatOpen/engine_fragment) | MIT / MIT | **STUDY** (fragments format; geometry comes from web-ifc) |
| [Topologic](https://github.com/wassimj/Topologic) / [topologicpy](https://github.com/wassimj/topologicpy) | AGPL-3.0 / LGPL-3.0 | **STUDY** (CellComplex concept for spaces) |
| [val3dity](https://github.com/tudelft3d/val3dity) | GPL-3.0 | **STUDY** (ISO 19107 validation rules, rewritten in our own words) |
| [Thingi10K](https://github.com/Thingi10K/Thingi10K) | Apache-2.0 (code); each model has its own licence | **DEPEND** as a test corpus (check model licences) |

---

## 3. Gaps where no open project is good enough, so we must lead

1. **Exact, permissively licensed weighted straight skeleton and roof generator.** Only CGAL (GPL) and surfer2 (GPL) are robust. campskeleton (Apache-2.0) has the right weighted, procedural-extrusion model but uses doubles. We should port Kelly's model onto our exact predicates.
2. **Exact hidden-line removal and section drawings from the B-rep.** We need linework that keeps persistent names and carries architectural line classes. OCCT's HLR (LGPL, another agent's area) is the only B-rep implementation. Krbn does this exactly but only for quadrics. Rhino HL and ln work by sampling.
3. **Certified, deterministic NURBS surface–surface intersection.** None exists in the open. GoTools is AGPL, BRL-CAD's is tolerance-based, and Hyper only handles planes and cylinders so far.
4. **Robust offset, shell and thicken on B-reps (layered walls).** Open work is mesh-only (topological offsets, PCO). On the B-rep side, OCCT is the only path, and it fails often.
5. **Alignment to solid.** Clothoid and IFC alignments swept into exact corridors with stationing. The parts exist (Clothoids, libOpenDRIVE); nobody has integrated them with a kernel.
6. **A mature permissive sketch solver with deterministic numerics, DOF diagnosis and decomposition.** ezpz and fiksi are young; the mature ones are LGPL or GPL.
7. **Terrain as a solid.** TIN plus breaklines, then cut/fill booleans against building solids. spade supplies the TIN; the integration is ours.
8. **Bit-identical native and WASM results.** None of the C++ libraries promise it (TBB threads, platform libm, SIMD). Krbn only claims determinism "on a given platform". This is why almost every item above is PORT rather than DEPEND.

---

## 4. Projects referencing or offering help to Spareware

None. GitHub code, issue and PR searches for "spareware" outside `spareware-co-op` found only unrelated namesakes. The single fork is [parametrix/spareware](https://github.com/parametrix/spareware), from an "itinerant architect and coder" who works on Revit API projects; it is not a geometry project. The other contacts are individual join requests: [#103](https://github.com/spareware-co-op/spareware/issues/103), [#116](https://github.com/spareware-co-op/spareware/issues/116), [#117](https://github.com/spareware-co-op/spareware/issues/117) and [#160](https://github.com/spareware-co-op/spareware/issues/160). No geometry library has offered help. The Hyper/csgrs author is the most aligned person to approach.

---

## Verification notes

- Read but unconfirmed: BRL-CAD's exact LGPL version (COPYING says LGPL without a version); OpenMesh's licence; whether IRIT has a newer licence.
- Not re-checked: the GPL-2.0-only versus GPL-2.0-or-later wording in OpenSCAD's per-file headers.
- The BRL-CAD organisation repo returned 404 to our API token; I read it through the `brlcad/BRL-CAD` mirror (a fork of `BRL-CAD/brlcad`).


Sources outside GitHub: [Gmsh LICENSE](https://gmsh.info/LICENSE.txt), [IRIT user manual](https://gershon.cs.technion.ac.il/irit/user_man.html), [Robust and Feature-Preserving Offset Meshing (arXiv)](https://arxiv.org/html/2412.15564v1), [crates.io API](https://crates.io).
