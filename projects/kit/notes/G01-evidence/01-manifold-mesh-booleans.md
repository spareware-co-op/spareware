*Evidence for [G01](../G01-geometry-kernel.md), the Spare Kit geometry kernel decision note. Written 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of each project at the pinned commits linked below. GPL and LGPL code is described in the agent's own words, never copied. Claims it could not confirm from a primary source are marked unverified.*

Manifold and robust mesh booleans: evidence report for Spare Kit's geometry kernel

Pins used throughout:
- Manifold HEAD: `d056b8fb70de8c8e6d6bee2e639b71c20aa08d01` (2026-10-20). M = https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01
- Last Manifold release: v3.5.4, `ce50d78021d64507f89e8c9fc2c2e51018117857` (2026-09-25).
- manifold-rust: `6b784dff1526781460152299bc2ea5a519278ea2`.
- Cherchi booleans: `7bd6c2697695f555f9a15d7e154e084d841316f0`.
- Attene Indirect_Predicates: `f8f243d18a6851fe3128b7e09af5b3574259d700`.
- libigl: `7100764c2a2833284a8bdfa9b948d2b3bf124624`.
- geogram: `40d72165ceeeb0a61977d6fdf74d929bfdd77b7b`.

Manifold licence: Apache-2.0 (https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/LICENSE).

Headline: Manifold's boolean is not exact. Its tie-handling was redesigned three times in ten months, and the latest version (unreleased, merged 15–20 Oct 2026) introduced a non-manifold/crash regression that was fixed five days later. For an iron-clad, bit-deterministic native+Wasm kernel, the evidence favours a plane-based fixed-width-integer boolean (EMBER / Nehring-Wirxel), written clean-room from the papers. Manifold should be used for its data model, its non-boolean modules (via the Apache-2.0 Rust port) and its tests.

---

1. Manifold's boolean, end to end

Lineage. The wiki credits Julian Smith's dissertation for the method (https://github.com/elalish/manifold/wiki/Manifold-Library). Manifold adds parallelism, a BVH broad phase and its own perturbation scheme. The Smith & Dodgson CAD 2007 paper is cited from memory, *unverified*.

The core idea (Smith) is a hierarchy of predicates by dimension:
- vertex–edge in x-projection: Shadow01, M/src/boolean3.cpp#L172-L202
- edge–edge in xy: Kernel11, #L204-L296
- vertex–face along +z: Kernel02, #L298-L371
- edge–face: Kernel12, #L373-L447

Each higher-dimension crossing count is a signed sum of lower-dimension "shadow" decisions, so every decision is made once and higher-level results cannot contradict each other. Notation is explained in M/src/boolean3.h#L26-L44.

Pipeline (constructor M/src/boolean3.cpp#L653-L739, assembly M/src/boolean_result.cpp#L696-L983):
1. Broad phase. Each edge box of one mesh, padded by 2·tol, is queried against the other mesh's face BVH (boolean3.cpp#L520-L531). Results are gathered per thread and then stable-sorted by (edge, face) (#L474-L549).
2. Intersect12 runs both ways (P edges × Q faces, Q edges × P faces). It produces signed crossing counts x12 and positions v12.
3. Winding03 (#L562-L638). Vertices joined by edges that no face crosses are merged with union-find. One representative per component gets its winding number from a +z ray using Kernel02, and the value is flood-filled to the rest of the component.
4. Result. Winding numbers become inclusion counts per operation (constants c1/c2/c3, boolean_result.cpp#L708-L710 and #L789-L802), and vertices are duplicated |i| times (#L835-L846). Points along each cut edge are paired by sorting (PairUp, #L284-L302), with debug asserts on odd counts. New edges are added per (P face, Q face) pair, and polygonal faces are triangulated (Face2Tri, #L936). Then come property interpolation (#L949), provenance (#L952), RemoveDegenerates (#L955), Morton sort (#L967) and new mesh IDs (#L968).

Why the output is manifold. The guarantee is topological: PairUp only needs start/end points to alternate, and the parity argument ensures an even count. The source itself says pairing becomes a heuristic if the geometric order is wrong (boolean_result.cpp#L286-L290). The guarantee requires manifold input; non-manifold MeshGL is rejected with an error status (M/README.md#L68). The wiki sets ε-valid output from ε-valid input as a goal it says cannot be proven, and says self-overlapping input may triangulate into overlaps. The triangulator docs say the same (M/src/polygon.cpp#L1072-L1086).

Ties and coplanar faces have changed three times:
- v3.5.4 (released): ties are only exact float equality, broken by a sign derived from vertex or face normals (https://github.com/elalish/manifold/blob/ce50d78021d64507f89e8c9fc2c2e51018117857/src/shared.h#L94-L96). Faces equal only up to roundoff were not treated as ties, which left internal double walls (PR #1877, https://github.com/elalish/manifold/pull/1877).
- #1877 (merged 2026-10-15): values within the tolerance count as ties.
- #1887 (merged 2026-10-20, https://github.com/elalish/manifold/pull/1887): ties are decided by one finite "Moved" configuration (boolean3.cpp#L93-L139, #L162-L170).
  - Q moves outward by tol/4.
  - P moves outward for union, inward for difference/intersection, by 1.125×tol/4, plus a per-axis shift in proportions 1/61, 1/89, 1/113.
  - Displacements keep up to three incident face planes parallel (#L47-L91).
  - Union expands both operands; difference and intersection contract P and expand Q (#L657-L661).
  - The tolerance TieTol is min(εP, εQ) (#L32-L37).
- #1898 (merged 2026-10-20, https://github.com/elalish/manifold/pull/1898) fixed a regression from #1877. Tolerance-shifted decisions reached beyond the broad-phase padding, which produced non-manifold output and crashed release builds. The fix clamps `Intersect` to its segments (M/src/shared.h#L109-L138) and pads the broad phase by 2·tol.

Epsilon. ε = relative precision × bBox.Scale() (M/src/properties.cpp#L408), recomputed from each result's own bounding box with no history (PR #1836, https://github.com/elalish/manifold/pull/1836).
- `Scale()` is the largest absolute coordinate (M/include/manifold/common.h#L316-L323), so ε depends on position and results are not translation-invariant.
- The default relative precision is 1e-12 (M/src/utils.h#L40). It is a process-global atomic set by `Quality::SetRelativePrecision` (M/src/manifold.cpp#L50-L65).
- `RemoveDegenerates` collapses and swaps edges below ε (M/src/edge_op.cpp#L58-L79), so the output geometry moves by up to about ε.

Is it exact? No. Intersection points are constructed in double precision and rounded, and nothing is held in an exact representation between cascaded operations. It can still produce wrong geometry in these cases:
- Features genuinely closer than the tolerance without being equal. #1887's own fuzz shows operand-order-dependent results or zero-volume "flaps" in 14–50 of 10,000 trials when corners are jittered by 0.9·tol. It describes this regime as best effort and says it is not uniformly better than before.
- Self-overlapping (non-ε-valid) input.
- Large absolute coordinates, where ε grows coarse.
- Accumulated rounding across chained operations.

What it does guarantee: manifold topology given manifold input, and correct volume within tolerance according to the PR evidence.

2. Data structures

- `Manifold::Impl` (M/src/impl.h#L28-L72) holds:
  - double `vertPos_`, `halfedge_`, flat `properties_` with `numProp_` values per property vertex
  - `vertNormal_`, `faceNormal_`, `halfedgeTangent_`
  - `meshRelation_` (originalID; a map meshID → {originalID, 3×4 transform, backSide, hasNormals}; per-triangle `triRef`)
  - the BVH `collider_`, `epsilon_` and `status_`
- Halfedges are structure-of-arrays (M/src/shared.h#L246-L348):
  - triangles are implicit (halfedge index = 3·tri+i)
  - arrays are `start_`, `paired_` and `propVert_`; the end vertex is read from the next halfedge (refactor #1709)
  - `propVert` separates geometric vertices from property vertices, so UV and normal seams don't break manifoldness
- MeshGL (M/include/manifold/mesh.h#L102-L163) is the interchange format:
  - interleaved `vertProperties` (xyz first), `triVerts`
  - `mergeFromVert`/`mergeToVert` to re-weld property seams
  - `runIndex`/`runOriginalID`/`runTransform`/`runFlags`
  - per-triangle `faceID`, `halfedgeTangent`, `tolerance`
  - `MeshGL64` is the double-precision variant
- Provenance: `TriRef` = {meshID, originalID, faceID, triID} (M/src/shared.h#L355-L371).
  - `UpdateReference` copies each output triangle's TriRef from its source triangle in P or Q and offsets Q's meshIDs (M/src/boolean_result.cpp#L505-L542).
  - `GetMeshGL` sorts triangles into runs by (originalID, meshID) (M/src/impl.h#L555-L566).
  - Users can reserve originalIDs and pass their own faceID through every boolean (M/include/manifold/manifold.h#L206-L208).
- What this means for persistent naming:
  - Input face identity survives booleans.
  - New intersection edges and vertices get no identity.
  - Vertex order is Morton-sorted.
  - meshIDs come from a process-global atomic counter (M/src/impl.cpp#L71-L75, read at boolean_result.cpp#L510, snapshotted at M/src/csg_tree.cpp#L276-L278), so they depend on scheduling and are not usable as stable names.
- CrossSection no longer uses Clipper2 at HEAD. It routes through Manifold's own float arrangement "boolean2" (M/src/cross_section.cpp#L26-L28; staged in PR #1722, https://github.com/elalish/manifold/pull/1722).
  - Its ε comes from a Smith-style error budget (M/src/boolean2_predicates.cpp#L46-L53).
  - It is position-inclusive and documented as deliberately not translation-invariant (M/src/boolean2.cpp#L225-L236).
- SDF: `LevelSet` runs marching tetrahedra on a body-centred cubic grid and snaps grid points to the surface (M/src/sdf.cpp#L444-L472); grid indexing is integer-only (#L108-L116).
- Smooth/Refine: halfedge tangents are weighted cubic Béziers (M/src/smoothing.cpp#L53-L70), and `Refine*` subdivides onto the interpolated surface (M/src/manifold.cpp#L796-L870).
- Collider: a radix-tree BVH over Morton codes (M/src/collider.h#L76-L345). Describing it as Karras-style is my own characterisation.

3. Parallelism and determinism

- Parallelism is TBB, behind `MANIFOLD_PAR`, which is OFF by default (M/CMakeLists.txt#L56). `autoPolicy` stays serial below 1e4 elements (M/src/parallel.h#L34-L58).
- `reduce` is documented as non-deterministic for non-associative operations (M/src/parallel.h#L464-L495). Volume and area use a serial Kahan sum, so they are deterministic (M/src/properties.cpp#L300-L329).
- CSG trees are now evaluated in batches of four with a serial-number tie-break, to make ordering deterministic (M/src/csg_tree.cpp#L31-L41, #L425-L450).
- Maintainers' position (issue #1177, https://github.com/elalish/manifold/issues/1177):
  - serial builds are expected to repeat run to run (stable sorts)
  - parallel builds have accepted races
  - cross-machine results are not guaranteed, citing FMA (x86 vs ARM) and libm trig
  - an earlier deterministic-parallel option was removed
- Open reports: #1233 (cross-architecture output differences) and #1320 (parallel nondeterminism with coplanar unions).
- No `-ffp-contract=off` is set; MSVC builds use `/fp:precise` (M/CMakeLists.txt#L165).
- CI determinism check (M/.github/workflows/manifold.yml#L79-L86, #L170-L175, #L600-L622; https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/scripts/determinism_cases.sh):
  - compares hex-float OBJ output across Linux GCC 14, macOS ARM and Windows MSVC
  - covers only six small tests, and only in serial builds
  - does not compare Wasm (which builds serial-only, M/README.md#L76) or TBB builds
- Verdict: not bit-deterministic across platforms or thread counts by contract.
- The Rust port manifold-rust (Apache-2.0, ports v3.5.0) claims bit-identical output in parallel and sequential builds by parallelising only order-preserving stages, and uses musl-derived trig (https://github.com/larsbrubaker/manifold-rust/blob/6b784dff1526781460152299bc2ea5a519278ea2/Cargo.toml#L41-L69; maintainer discussion in #1774, https://github.com/elalish/manifold/issues/1774). *Unverified* by me.

4. Testing

- GoogleTest suites at HEAD: boolean_test 69 TESTs, boolean_complex 21, manifold_test 86, samples 12. #1887 reports 572 passing.
- Named determinism and coplanar regressions are in M/test/boolean_test.cpp#L185-L222, #L629-L660 and #L1137; the #1897 reproducer is at #L1230.
- 30 OBJ regression meshes: https://github.com/elalish/manifold/tree/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/test/models
- 8 sample programs (Menger sponge, gyroid, condensed matter, …) under samples/src.
- Fuzzing: one Google FuzzTest target, random union/difference chains of up to 100 transformed cubes.
  - The coordinate domain is deliberately limited to |v| in [0.1, 10] because larger numbers may hit precision issues (M/test/manifold_fuzz.cpp#L36-L44, #L97).
  - It is built in CI (manifold.yml `build_fuzzer`); whether it fuzzes continuously is *unverified*.
- CI also runs ASan/UBSan and TSAN workflows, a weekly benchmark and a PR performance guard.
- Large stress campaigns appear only in PR descriptions, not in CI: 10,800 voxel meshes in #1887, and 18,000 jittered unions in #1898.

5. Comparison with the state of the art

EMBER (Trettner, Nehring-Wirxel, Kobbelt, ACM TOG 41(4) Art. 39, 2022; PDF: https://www.graphics.rwth-aachen.de/media/papers/339/ember_exact_mesh_booleans_via_efficient_and_robust_local_arrangements.pdf)
- Representation: plane-based, with fixed-width homogeneous integer coordinates.
  - Input: 26-bit integer coordinates, or EMBER's own previous output, so it is closed under iterated CSG.
  - Intermediate values never exceed 256 bits.
  - No bignums or rationals; the types are statically sized (§2.2, p.39:3; §4).
- Quantisation happens only at import and export. The paper gives about 15 nm resolution for a 1 m³ scene (§3.3).
- Inputs must induce a piecewise-constant integer generalised winding number (PWN) field. Self-intersecting and non-manifold PWN input is allowed (§3.4).
- Algorithm: one adaptive kd subdivision pass, local BSPs per leaf, and generalised winding-number vectors traced locally from propagated reference points.
- Speed:
  - geometric mean 1.6 ms per boolean on 1,000 Thingi10K pairs of 1k–100k faces (i9-9900K)
  - 3.86 ms vs QuickCSG 36 ms vs Zhou's Mesh Arrangements 9.6 s on one example (Fig. 15)
  - 5.5× speed-up on 8 cores
- Each output polygon is a subset of an input polygon, so attribute and ID transfer is trivial.
- Stated limitation: exactness holds only inside the integer representation; export rounding can reintroduce tiny self-intersections (§6).
- Code: I found no public repository (*unverified*). Treat it as method-only.

Nehring-Wirxel, Trettner, Kobbelt 2021, "Fast Exact Booleans for Iterated CSG using Octree-Embedded BSPs" (CAD; https://arxiv.org/abs/2103.02486)
- Supplies the integer plane arithmetic EMBER builds on.
- Octree-embedded BSPs for iterated CSG; reports about 2.5M mesh–plane cuts per second per core.
- No code release found (*unverified*).

Campen & Kobbelt 2010, "Exact and Robust (Self-)Intersections for Polygonal Meshes" (CGF 29(2); https://www.graphics.rwth-aachen.de/media/papers/campen_2010_eg_021.pdf)
- Plane-based BSPs, built only in octree cells where intersections occur.
- After minimal input quantisation, standard floating point can be used safely.
- Predecessor of EMBER. Code availability *unverified*.

Zhou, Grinspun, Zorin, Jacobson 2016, "Mesh Arrangements for Solid Geometry" (https://cims.nyu.edu/gcl/papers/zhou2016mas.pdf)
- Exact via CGAL's Epeck kernel (lazy exact with filters).
- Pipeline: resolve self-intersections, extract cells, propagate winding numbers, select faces (https://github.com/libigl/libigl/blob/7100764c2a2833284a8bdfa9b948d2b3bf124624/include/igl/copyleft/cgal/mesh_boolean.cpp#L206-L315). It also includes a heuristic `relabel_small_immersed_cells` step (#L315).
- Handles PWN soups. It is slow: about 25× slower than Cherchi 2022 on average, and about 2,500× slower than EMBER on Fig. 15.
- Licence: the file headers are MPL-2.0 (mesh_boolean.h#L1-L9), but the code lives in `igl/copyleft/cgal`. The repository ships both LICENSE.GPL and LICENSE.MPL2, and the CGAL dependency is GPL/LGPL.

Cherchi, Pellacini, Attene, Livesu 2022, "Interactive and Robust Mesh Booleans" (TOG 41(6); https://www.gianmarcocherchi.com/pdf/interactive_exact_booleans.pdf)
- Uses floating-point arrangements with Attene's indirect predicates, plus exact ray casting for labelling.
- About 25× faster than libigl on average.
- Inputs must be manifold, watertight and free of self-intersections (https://github.com/gcherchi/InteractiveAndRobustMeshBooleans/blob/7bd6c2697695f555f9a15d7e154e084d841316f0/README.md).
- Cascaded booleans are explicitly unsupported, because output is snapped to floats (paper §7.1).
- The authors call EMBER the fastest method but note it needs integer conversion (§2).
- Licence: the authors' own code is MIT (…/LICENSE). It bundles Indirect_Predicates, whose LICENSE file is LGPL-2.1 text while its README says LGPL-3.0-or-later (https://github.com/MarcoAttene/Indirect_Predicates/blob/f8f243d18a6851fe3128b7e09af5b3574259d700/README.md#L78-L82).
- That library needs `-frounding-math` (README#L55) and switches the FPU to round-up (include/implicit_point.hpp#L41). Wasm has no rounding-mode control, so this design cannot run bit-deterministically on Wasm as is.

Lévy 2024/2025, "Exact predicates, exact constructions and combinatorics for mesh CSG" (https://arxiv.org/abs/2405.12949)
- Exact co-refinement of polygon soups, then a Weiler model built by radially sorting faces around non-manifold edges. Supports multi-operand CSG and some mesh repair.
- Arithmetic: expansions or multi-precision floats, with exact homogeneous points.
- Implemented in geogram under BSD-3-Clause: https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/LICENSE and src/lib/geogram/mesh/mesh_surface_intersection.h#L68-L73, #L345-L353. Speed *unverified*.

Others
- trueform (arXiv 2607.15905, July 2026): PolyForm Noncommercial or paid commercial licence (https://github.com/polydera/trueform/blob/1ee282dc0938378cb8c8048ae8e9d5f903a66ccd/LICENSE). Avoid.
- manifold-rust's "robust" engine (Apache-2.0): Barki 2015 classification plus the Zhou arrangement, on dashu rationals (MIT OR Apache-2.0) with Shewchuk-style static filters.
  - It reports a Thingi10K sweep with 0 failures (src/robust/mod.rs#L1-L50; README#L15-L45; *unverified*).
  - Its CrossSection uses clipper2-rust (BSL-1.0).

6. Recommendation for Spare Kit's mesh path

Pick an EMBER-style plane-based, fixed-width-integer boolean as the authoritative mesh boolean (for display, clash and IFC-mesh import), alongside the exact B-rep kernel. Why:
- Exact by construction with no tolerances.
- Integer-only arithmetic is bit-identical on native and Wasm: no rounding modes, no FMA contraction, no libm.
- Fastest published exact method.
- Closed under iterated CSG, so chained operations are never re-rounded.
- Persistent naming falls out of it. Every output face lies on an input plane, every new edge is the intersection of two input planes, and every vertex the intersection of three. Topology can therefore be named by the generating input-plane IDs, which Manifold cannot do.

Port order:
1. Arithmetic layer, clean room.
   - Statically sized integer types up to 256 bits, with bit budgets taken from Nehring-Wirxel 2021 and EMBER §3–4.
   - Filtered f64 predicates using static error bounds only, never rounding-mode switching. Shewchuk's are public domain; geogram's PCK/expansion code is BSD-3 and portable with attribution.
2. Plane-based primitives from EMBER §3: plane from integer triangle, 3-plane homogeneous point, classify, segment–polygon tests.
3. Mesh data model adapted from Manifold (Apache-2.0; reimplement or port with notices): halfedge SoA, `propVert` seams, MeshGL-style runs, `originalID`/`faceID` provenance.
   - Replace the global atomic meshID counter with deterministic, content- or tree-derived IDs.
4. BVH broad phase following Manifold's collider design, or EMBER's own subdivision.
5. EMBER core: adaptive subdivision with reference-point winding-vector propagation, local BSPs, winding-transition segment tracing.
   - Make parallel output assembly order fixed in tree order; EMBER's determinism under work stealing is *unverified*.
6. Import pipeline:
   - quantise in a local project frame, since BIM georeferencing would waste the 26-bit range
   - check PWN
   - for non-PWN IFC soups, fall back to a Lévy/Zhou-style exact arrangement (method from geogram BSD-3 or the manifold-rust robust engine, Apache-2.0)
7. Export: snap to f64 only at the display/file boundary. Answer clash queries on the exact representation.
8. Non-boolean features (LevelSet, Smooth/Refine, hull, Minkowski) from manifold-rust (Apache-2.0); they are float-based but not topology-critical.
9. Tests:
   - port Manifold's test/models and samples, Thingi10K pairs and EMBER's supplemental pairs (Thingi10K models carry their own licences)
   - copy Manifold's hex-float cross-platform diff CI and extend it to wasm32 and to several thread counts
   - run differential tests against manifold-rust's robust engine and libigl, in CI only

---

Adopt
- EMBER / Nehring-Wirxel plane-based integer method (method only, clean room).
- Generalised winding-number vectors for classification and for multi-operand clash.
- Manifold's halfedge, MeshGL and provenance schema; manifold-rust for the non-boolean modules.
- geogram (BSD-3) expansion arithmetic and Lévy's Weiler-model approach for repairing soups.
- Manifold's test corpus and its determinism-CI pattern, extended to Wasm and thread counts.

Avoid
- Manifold's Boolean3 as the source of truth: float-based, tolerance- and position-dependent, global precision setting, tie rules redesigned three times in ten months.
- Manifold's boolean2/CrossSection for drafting.
- Attene's indirect predicates (LGPL, rounding-mode switching), CGAL code (GPL/LGPL), trueform (noncommercial licence).
- Global atomic IDs, unordered float reductions, output that depends on hash order.
- Rounding to f64 between chained booleans.

Open questions
- Coordinate bit budget for site-scale BIM: 2^26 grid steps is about 671 m at 0.01 mm resolution. Wider grids need the bit widths re-derived.
- How to keep exact B-rep tessellations watertight on the integer grid.
- EMBER has no public code: is it available from RWTH on request? Clean-room implementation will take real effort.
- Repair semantics for non-PWN IFC input.
- A 3D snap-rounding policy for export.
- Whether Manifold's #1887 tie-breaking holds up once it is released.
