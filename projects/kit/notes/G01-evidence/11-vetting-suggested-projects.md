# Vetting: the projects suggested to the owner

*Evidence for [G01](../G01-geometry-kernel.md), the Spare Kit geometry kernel decision note. Written 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of each project at the pinned commits linked below. GPL and LGPL code is described in the agent's own words, never copied. Claims it could not confirm from a primary source are marked unverified.*

The owner was sent an AI search summary recommending Truck, cadcore, csgrs, GeoRust geo, RGeometry and SolidCraft. Each project was evaluated against its actual source, then challenged by two independent skeptics (one re-checking facts, one arguing the opposite verdict). Read for G01 on 2026-10-21.

## Truck

**Repo url:** https://github.com/ricosjp/truck

**Licence:** Apache-2.0 for the whole workspace. Evidence: the root LICENSE file (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/LICENSE) and `license = "Apache-2.0"` in every crate's Cargo.toml, for example https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/Cargo.toml. It works with MPL-2.0, so porting is legally allowed. Its dependencies are permissive: cgmath 0.18, which is unmaintained (issue #126); matext4cgmath; ruststep; spade; rclite; parking_lot; wgpu, which only the renderer uses. External PRs require a CLA (CLA-assistant on PR #134).

**Activity:** The repo was created on 2020-12-26 and last pushed on 2026-10-19. It has 1,661 stars, 124 forks and 37 open issues plus PRs (136 issue/PR numbers in total). The bus factor is 1: ytanimura has 2,602 contributions, and the next contributor has 9. 18 people have ever contributed. Over the past 12 months there were 261 commits, almost all by Yoshinori Tanimura. The last crates.io release was on 2024-09-20 (truck-shapeops 0.4.0, truck-modeling 0.6.0, truck-geometry 0.5.0, truck-stepio 0.3.0). Master has more than two years of unreleased fixes. Issue #135 shows released crates panicking and hanging on problems that only master fixes, and the maintainer says v0.7 is coming 'soon' with no date. External PRs that fix booleans are still open and unmerged: #110 (coplanar), #111 (STEP booleans) and #134 (classifying untouched faces). In #121 the maintainer says RICOS uses Truck internally for 'RICOS Generative CAE', where geometry performance needs are lower than in interactive CAD. In #126 the maintainer plans a migration from cgmath to nalgebra, which will break the whole API.

**Claims**

- **claim**: 1. It is the most established, production-tested, pure-Rust B-rep CAD kernel; **verdict**: partly; **evidence**: Among pure-Rust NURBS B-rep kernels it is the oldest and most visible: created in 2020, 1,661 stars, about 145k crates.io downloads for truck-topology. Fornjot is archived and has no NURBS. 'Production-tested' is weak. (a) The boolean suite is one smoke test with no assertions on the result: https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/src/transversal/integrate/tests.rs#L4-L27. There are a few unit tests on parabola surfaces, and no coplanar, tangent or vertex-on-face cases and no fuzzing. (b) The known users are moving away. CADmium (CADmium-Co/CADmium), built on Truck, is archived and its authors later built CADara on OpenCASCADE (HN comment 50066980). SolidCraft patches Truck heavily and is migrating to solidcore (tangency-labs/solidcraft issue #1597). (c) The only other production use is RICOS's in-house CAE, which the maintainer states in issue #121 and I could not verify independently. (d) The bus factor is 1, and the newest crates.io release is from Sept 2024.
- **claim**: 2. It handles NURBS, meshes and topology natively; **verdict**: partly; **evidence**: Correct that it has NURBS (truck-geometry/src/nurbs: bspcurve.rs, bspsurface.rs, knot_vec.rs, nurbscurve.rs, nurbssurface.rs), meshes (truck-polymesh, truck-meshalgo) and half-edge-free B-rep topology (truck-topology: Vertex, Edge, Wire, Face, Shell, Solid in https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-topology/src/lib.rs#L108-L176). The caveats matter for Spare Kit. The modelling layer's curve type has only Line, BSpline, NURBS and IntersectionCurve, and its surface type has only Plane, BSpline, NURBS and RevolutedCurve. Analytic cylinders, cones, spheres and tori are not first-class modelling surfaces: https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-modeling/src/geometry.rs#L24-L32 and #L126-L134. Edges store no p-curves; tessellation and booleans recover (u,v) by Newton search. An edge made by a boolean is a procedural IntersectionCurve: a leader B-spline plus two surfaces, evaluated by Newton projection. Its `subs` unwraps that projection and panics when it fails to converge: https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-geometry/src/decorators/intersection_curve.rs#L4-L37 and #L220.
- **claim**: 3. It leverages WebGPU for fast performance; **verdict**: false; **evidence**: wgpu is a dependency only of truck-platform and truck-rendimpl, the rendering crates (truck-platform/Cargo.toml: `wgpu = "30.0.1"`). A code search of the repo finds no ComputePipeline. The kernel crates (geometry, topology, meshalgo, shapeops, modeling) depend on no GPU crate and run on the CPU, with rayon for some work. The README's 'Rust and WebGPU' line is aspirational: https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/README.md. Booleans compare every face of one solid with every face of the other, with no spatial culling: https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/src/transversal/loops_store/mod.rs#L462-L476. lib.rs says BSP acceleration is future work (#L9), and issue #68 reports that booleans are slow.
- **claim**: 4. It is split into small, independently usable crates; **verdict**: true; **evidence**: There are 15 workspace member crates (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/Cargo.toml), each published separately on crates.io: base, geotrait, geometry, topology, polymesh, meshalgo, modeling, shapeops, stepio, platform, rendimpl, plus drafting, assembly, derivers and js, some of them unreleased. This is the 'Theseus' ship' design the README describes. Caveat: the crates are not independent at the algorithm level. truck-shapeops needs truck-meshalgo because booleans are driven by meshes, and everything is tied to cgmath types and to the global truck_base::TOLERANCE.
- **claim**: 5. It powers the parametric modeler SolidCraft (github.com/bherbruck/solidcraft); **verdict**: partly; **evidence**: bherbruck/solidcraft redirects to tangency-labs/solidcraft, which was created 2026-10-07 and has about 8.2k stars. HN item 50064840 is the SolidCraft Show HN, not a Truck post. SolidCraft depends on truck-modeling, truck-topology, truck-shapeops, truck-meshalgo and truck-stepio 0.x: https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/Cargo.toml#L47-L52. It replaces geometry, shapeops, stepio, meshalgo and matext4cgmath with vendored, patched forks (#L146-L151). Its vendor README lists more than 20 fixes to Truck: panics, hangs, wrong second derivatives, random face order and boolean misclassification (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/vendor/README.md). It reads STEP with its own reader and uses only Truck's writer (vendor/README.md#L84). It wraps Truck booleans in a retry layer that nudges operands and offsets faces when a boolean fails (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/robustness.md#L93-L120). It has announced that it is replacing Truck with solidcore, a Rust port of NVIDIA's SMLib (SolidCraft issues #1597 and #331; #2661 is titled 'Tests that survive the move to solidcore'). So Truck runs SolidCraft today, but as a heavily patched stopgap that is being replaced.
- **claim**: 6. It is a true alternative to OpenCascade for NURBS, boundaries and STEP files; **verdict**: false; **evidence**: Booleans: the crate's own docs say 'supported only for shapes where faces intersect transversally. Cases where faces are tangent to each other are not yet supported.' (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/src/lib.rs#L5-L9). OR of two cubes sharing a face returns None (#57, open since Feb 2024). Coincident faces and edges return None or panic (#114). There are no difference or xor operations, only and(a, not(b)) (#85). Fillets work only on a single edge whose end vertices each touch exactly three faces (lib.rs#L11-L14). STEP: the reader covers a subset of entities with no OFFSET_CURVE/OFFSET_SURFACE, COMPOSITE_CURVE or RECTANGULAR_TRIMMED_SURFACE: https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-stepio/src/in/mod.rs#L25-L80. An unconvertible edge is dropped with an eprintln and the later edges are silently misnumbered (#133). Fusion 360 products fail to load (#136). NaN placements panic (#123, #128). STEP-loaded solids cannot be passed to the booleans because of a missing trait impl (#91, fix PR #111 unmerged). OCCT has none of these gaps at this level.

**Architecture:** The workspace (Cargo.toml) is a stack of crates:
- truck-base: the cgmath64 re-export, a global `TOLERANCE = 1e-6` (absolute), Newton solver, bounding boxes, and a deterministic hash. https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-base/src/tolerance.rs#L5-L9
- truck-geotrait: traits such as ParametricCurve/Surface, BoundedCurve, Cut, Invertible, SearchParameter, SearchNearestParameter and ParameterDivision1D/2D.
- truck-geometry: knot vectors, B-spline and NURBS (nurbs/), specified primitives (line, circle, plane, sphere, torus, conics), and decorators (IntersectionCurve, PCurve, RevolutedCurve, Processor for transforms, homotopy, offset, RBF fillet surfaces).
- truck-topology: Vertex, Edge, Wire, Face, Shell, Solid.
- truck-polymesh and truck-meshalgo: meshes, tessellation (spade CDT), and the triangle-collision code that booleans use.
- truck-modeling: closed Curve/Surface enums and builder sweeps (tsweep, rsweep, homotopy).
- truck-shapeops: booleans, fillet and healing.
- truck-stepio: STEP I/O via ruststep.
- truck-drafting: a new, unreleased 2D crate.
- truck-assembly.
- truck-platform and truck-rendimpl: wgpu rendering.
- truck-js: wasm bindings.

Topology is generic over <P, C, S> and has no half-edges. Each Vertex, Edge and Face holds an `Arc<Mutex<_>>` of its geometry. Edge and Face carry an orientation bool. Wire is a VecDeque<Edge> and Shell is a Vec<Face>. Identity is the raw pointer address: `ID(ptr as usize)` (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-base/src/id.rs#L5-L12; lib.rs#L221-L253). There are no p-curves, no tolerances per vertex or edge, and no persistent naming. `Solid::new` checks only combinatorial conditions (non-empty, connected, closed, no singular vertices) and panics on failure through `remove_try`: https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-topology/src/solid.rs#L10-L31.

The hardest algorithm is the boolean, which is driven by meshes (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/src/transversal/integrate/mod.rs#L75-L132):
1. Triangulate both shells at the boolean tolerance.
2. For every pair of faces (no culling), intersect the triangle meshes (`extract_interference`).
3. Chain the segments into polylines by snapping endpoints to a 2e-6 integer grid (polyline_construction/mod.rs#L26-L35).
4. Newton-project each polyline vertex onto both surfaces (intersection_curve/mod.rs#L26-L51).
5. Classify each curve's side from the sign of `n0 × t · n1` at its midpoint (loops_store/mod.rs#L55-L70).
6. Split faces into loops and divide them; pieces are built with `Face::debug_new`, which is unchecked in release builds (divide_face/mod.rs#L92).
7. Classify faces that no curve touches by counting hashed-ray crossings against the other solid's mesh (integrate/mod.rs#L97-L125). PR #134 shows this ignores face orientation and gives the wrong threshold for inverted operands.
8. Replace the polyline curves with quadratic B-spline leaders (integrate/mod.rs#L55-L73).
9. Return `Option<Solid>`, built with the panicking `Solid::new` (#L154, #L177).

**Robustness:** The arithmetic is plain f64 throughout. Every test uses one global absolute tolerance, TOLERANCE = 1e-6 (TOLERANCE2 = 1e-12). There are no exact or adaptive predicates anywhere in the boolean path. Triangle-triangle intersection uses `dotapnor * dotaqnor > 0.0` and `signum(... + TOLERANCE2)` tests (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-meshalgo/src/analyzers/collision.rs#L147-L172). For coplanar triangles both dot products are 0, which gives a 0/0 parameter. Only the tessellation's constrained Delaunay step inherits robust predicates, from spade.

The topology of the intersection depends on the tessellation, so loops smaller than the chord tolerance, tangent contacts and double points can be missed or merged. SolidCraft's ROADMAP says equal-radius crossing cylinders 'touch at two points, where the mesh intersection cannot trace the curves' (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/ROADMAP.md#L631-L640). Snapping points to a grid splits points that are within tolerance but straddle a cell boundary.

Degenerate cases are not handled. Coplanar, tangent and touching faces fail by design (lib.rs#L7-L8, issues #57 and #114). Failure is either `None` with no reason or a panic:
- `Solid::new`, when the result shell is not closed (#134);
- `IntersectionCurve::subs` `.unwrap()`, when Newton fails (intersection_curve.rs#L220). This panic is in #135 and in SolidCraft #2862 and #2357, where a ring of 90 segments panics and is then refused after 21 minutes;
- `mat.invert().unwrap()` in curve derivatives (#L173, #L185), where the matrix is singular at tangency;
- `nonpositive_tolerance!` asserts when NaN arrives from STEP placements (#123, #128).

In release builds, `Face::debug_new` skips face validation, so non-simple wires can pass through silently. The only validation is combinatorial; there is no geometric self-intersection or edge-on-surface check. In the released 0.x crates, moving a solid made by a boolean can hang forever: matext4cgmath's Newton loop has no step limit (#135). Master no longer calls it from `transform_by`, but the underlying loop is unchanged.

Testing is thin. Booleans have one end-to-end smoke test (integrate/tests.rs) and about 10 unit tests on synthetic parabola and sphere surfaces. There is no fuzzing, no volume or identity checks, and no coplanar or tangent regressions. proptest is used in the geometry crates. SolidCraft had to build its own fuzzer: random designs plus boolean volume identities (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/robustness.md). On top of Truck it still retries failed booleans by nudging operands and separating coincident faces (#L93-L120). Fillets are single-edge only, solved by Newton (lib.rs#L11-L14).

**Determinism:** Truck is not deterministic, by construction:
- Entity identity is the heap address (id.rs#L5-L12), and Face hashes its surface pointer (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-topology/src/face.rs#L1125-L1131).
- Shell adjacency and `connected_components` iterate FxHashMaps keyed by those pointers (shell.rs#L321-L327 and #L1150-L1158). So the order of faces and shells in a boolean result changes with allocator and ASLR state. SolidCraft documents that 'a boolean's faces came out in a different order on every run' and patched it (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/vendor/README.md#L17-L19).
- Polyline chaining picks `graph.iter().next()` over a hash map (polyline_construction/mod.rs#L87-L90).
- Ray directions for inside/outside tests come from a deterministic splitmix hash of the point's bits, which is good (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-base/src/hash.rs#L15-L43). But they are turned into a vector with `f64::cos` and `sin` (hash.rs#L121-L128), which come from the platform libm and are not guaranteed bit-identical between native and wasm32.
- Newton iterations, tessellation-dependent intersection topology and a global absolute tolerance make results sensitive to the last bit.

In practice, SolidCraft's robustness goldens give different topology and even different success or failure on Windows versus Linux for 16 of 96 seeds (tangency-labs/solidcraft issue #924). CI runs a wasm test job (.gitlab-ci.yml wasm-test), but nothing compares native and wasm output bit for bit.

**Strengths**

- Apache-2.0, pure Rust, `unsafe_code` warned on and builds for wasm32 (CI has a wasm-test job), so licence and toolchain fit Spare Kit's constraints.
- Clean separation of geometry traits (truck-geotrait) from concrete geometry (truck-geometry) and generic topology (truck-topology<P,C,S>). Topology can be instantiated with 2D geometry, which is how truck-drafting reuses it.
- A reasonably complete B-spline/NURBS toolkit with property tests: knot vectors, de Boor evaluation, higher-order derivatives (CurveDers/SurfaceDers on master), knot insertion, least-squares fitting, and the Processor/Revolved/Homotopy decorators (truck-geometry/src/nurbs, /decorators).
- Clear topological validity errors (EmptyShell, NotConnected, NotClosedShell, NotManifold, NotSimpleWire, ...) and a `try_new` alternative to every panicking constructor.
- Readable, well-commented code from essentially one author, with a candid status section that admits the boolean limits.
- Its documented failures are a ready-made source of regression cases: Truck issues #57, #114, #134 and #135, plus SolidCraft's vendor README, which lists more than 20 diagnosed defects.

**Weaknesses**

- The boolean architecture is fundamentally mesh-driven. It finds surface intersections from triangle-mesh collisions at the user's tolerance, so the topology of the result depends on the tessellation. This cannot be made exact or robust without replacing it.
- No exact or adaptive predicates. One global absolute TOLERANCE of 1e-6, and sign tests on raw f64 dot and cross products for in/out and side classification.
- Coplanar, tangent and touching faces are unsupported, by the crate's own docs. This is the most common case in building models (walls meeting slabs), and the issues have been open since 2024 (#57, #114).
- Failure is `Option::None` with no reason, or a panic (Solid::new, IntersectionCurve::subs unwrap, matrix-inverse unwrap, tolerance asserts). Release builds skip face validation (Face::debug_new). This fails Spare Kit's 'never return an invalid solid, never fail silently' rule.
- Boolean-produced edges are procedural IntersectionCurves, re-solved by Newton on every evaluation. There are no stored p-curves, and evaluation can fail or hang.
- Not deterministic: pointer-address IDs and hash-ordered traversal (Arc<Mutex<_>> per entity), libm trig in ray directions, and cross-platform outcome flips seen downstream (SolidCraft #924).
- No analytic cylinder, cone, sphere or torus in the modelling layer, so there are no closed-form surface intersections. No shell, offset or draft solids. Fillets are single-edge only. No 2D drafting at AutoCAD level (truck-drafting is new and unreleased). No subdivision or implicit modelling.
- The boolean is O(F0×F1) with no spatial acceleration (#68).
- Partial STEP coverage, with silent data loss (#133, #136, #123, #128).
- Bus factor of 1. crates.io releases are two years stale. External boolean fixes sit unmerged. A breaking migration from cgmath to nalgebra is planned (#126).
- The best-known downstream user (SolidCraft) vendors and patches four crates, wraps booleans in nudge-and-retry, and is migrating to an SMLib port. CADmium, the earlier Truck-based CAD, is archived.

**Verdict:** STUDY

**Verdict reason:** Judged against the best architecture rather than convenience, Truck's core is the wrong design for Spare Kit. Its booleans are driven by triangle-mesh collision, and intersection curves are refined by Newton under one absolute f64 tolerance. They have no exact predicates and no coplanar or tangent handling. They report failure by None or panic. Entity identity is a heap address, so results are not deterministic. None of this can be fixed by patching: SolidCraft tried with 20+ vendored fixes and an operand-nudging retry layer, and is now replacing Truck. DEPEND would bring these flaws into the foundation for decades. PORT is legally possible because Apache-2.0 works with MPL-2.0, but porting would copy the very architecture Spare Kit must avoid. REJECT is too strong, because some of its geometry-layer work and its well-documented failure cases are worth learning from.

**What to take**

- Trait decomposition for curves and surfaces (ParametricCurve/Surface, BoundedCurve, Cut, Invertible, SearchParameter, SearchNearestParameter, ParameterDivision) in truck-geotrait. It is a useful map of the capabilities a kernel needs; issue #126 records the maintainer's own complaint that the trait bounds are too verbose. https://github.com/ricosjp/truck/tree/78d699b0792b39e27c2bf073b682986701644e91/truck-geotrait/src
- B-spline and NURBS algorithms and their property tests as a reference implementation to check Spare Kit's own against: knot_vec.rs, bspcurve.rs, bspsurface.rs, nurbscurve.rs and nurbssurface.rs in https://github.com/ricosjp/truck/tree/78d699b0792b39e27c2bf073b682986701644e91/truck-geometry/src/nurbs. Note that SolidCraft found the released 0.5.0 bspsurface uuder/vvder wrong for degree ≥ 3; master has rewritten them with der_mn.
- Topology validity vocabulary: the Solid::try_new checks (closed, connected, no singular vertices) and the error enum in https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-topology/src/solid.rs#L10-L31 and errors.rs. Treat these as a minimum; Spare Kit also needs geometric validity checks.
- Deterministic, hash-seeded ray directions for point-in-solid tests (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-base/src/hash.rs#L15-L43). The idea is right, but the trig would have to be replaced with something cross-platform exact.
- Lessons to design against, which are also a regression corpus: Truck issues #57 (cubes sharing a face), #114 (coincident boxes), #134 (disjoint or void tool classification), #135 (shallow-loft cut panic and transform hang) and #91; and SolidCraft's catalogue of fixed Truck failures at https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/vendor/README.md. Turn each into a Spare Kit test.
- Test-oracle method from SolidCraft's fuzzer: random grid-snapped primitives, then checking V(A∪B)+V(A∩B)=V(A)+V(B) and V(A−B)=V(A)−V(A∩B), plus per-step solid validity. https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/robustness.md
- STEP entity checklist (truck-stepio Table, https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-stepio/src/in/mod.rs#L25-L80) and the Apache-2.0 step-conformance corpus (github.com/dan335/step-conformance, cited in #136) for testing a STEP reader.

### Challenge 1: FACTS: I re-checked the licence, the activity numbers, all s

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- Wrong: 'The only validation is combinatorial; there is no geometric self-intersection or edge-on-surface check.' Truck has an opt-in edge-on-surface check. Face::is_geometric_consistent tests each edge's end vertices against its curve and calls surface.include(curve) (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-topology/src/face.rs#L1084-L1101, edge.rs#L390-L403). Shell and Solid wrap it (solid.rs#L136-L147). Two caveats make the critique stronger, not weaker. No constructor and no boolean calls it. And the modelling-layer IncludeCurve impl hits `unimplemented!()` for every IntersectionCurve edge, so running the check on any boolean result panics (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-modeling/src/geometry.rs#L175-L196, lines 183, 189, 195 and 209). There is still no self-intersection check.
- Overstated: 'Booleans compare every face of one solid with every face of the other, with no spatial culling' and 'O(F0×F1) with no spatial acceleration'. At the face level it is true: every face pair is visited with no face bounding-box reject (loops_store/mod.rs#L462-L478). But each pair's mesh intersection uses sweep-and-prune over triangle intervals projected onto a hashed axis, plus a per-triangle AABB reject, before the exact triangle test (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-meshalgo/src/analyzers/collision.rs#L4-L16, #L79-L145, #L211-L230).
- Mis-cited: #68 does not support 'O(F0×F1)'. Its profiling comments put the cost in truck_geotrait::algo::surface::presearch, which allocates a fresh basis-function Vec on every subs call (about 50% of the time), not in face-pair complexity (https://github.com/ricosjp/truck/issues/68, comments by twitchyliquid64 and LtdJorge, Oct-Nov 2024).
- Wrong licence evidence: '`license = "Apache-2.0"` in every crate's Cargo.toml'. truck-assembly and truck-drafting have no license field (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-assembly/Cargo.toml, .../truck-drafting/Cargo.toml). The root Apache-2.0 LICENSE still covers them, so the conclusion stands. Note that truck-stepio on master now depends on the unpublished, unlabelled truck-assembly 0.1.0.
- Activity nuance: the contributors API also lists an anonymous identity 'truck' <truck@noreply.ritc.jp> with 245 commits, every one titled 'cargo upgrade'. It is an automated dependency bumper, not a person. 53 of the roughly 264 commits since 2025-10-19 are these bumps, so about 20% of the '261 commits in 12 months' are not development. 'Bus factor 1' and 'next human contributor has 9' are still correct (gh api repos/ricosjp/truck/contributors?anon=1; commits?author=truck@noreply.ritc.jp).
- Over-attribution: the 16 of 96 Windows/Linux outcome flips are presented as Truck non-determinism 'in practice'. Issue tangency-labs/solidcraft#924 itself says the likely cause (platform libm feeding the kernel's tolerance decisions) is 'not confirmed'. SolidCraft's robustness doc also says the seeded generator feeds geometry back into later commands, so 'the same RNG seed can therefore generate different command coordinates or picks on different math libraries' (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/robustness.md#L66-L70). The flips belong to the SolidCraft stack plus its fuzzer, not to Truck alone.
- Weak evidence: '#114: coincident faces and edges return None or panic'. In #114 case 1 the reporter writes 'and() returns None, causing panic', and the panic is the reporter's own `.expect()` (https://github.com/ricosjp/truck/issues/114). Truck does panic internally on other paths (Solid::new at integrate/mod.rs#L154 and #L177, the IntersectionCurve::subs unwrap, polyline_construction/mod.rs#L33 `cast::<i64>().unwrap()`), but #114 only shows None.
- Nuance: #126 does not describe a straight cgmath-to-nalgebra swap. The maintainer plans v0.7 first, then a `ControlPoint` abstraction implemented for both cgmath and nalgebra types, as a planned breaking change to the core traits (https://github.com/ricosjp/truck/issues/126#issuecomment, ytanimura 2026-07-09).
- Minor pin and count slips: the SolidCraft [patch.crates-io] block is L146-L152; matext4cgmath is on L152, outside the cited L146-L151 (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/Cargo.toml#L146-L152). truck-topology has 142,949 crates.io downloads, not about 145k (145k is truck-modeling). The workspace has 17 members: 15 truck crates plus example-pages-generator and readme-generator.
- Missed: the live Apache-2.0 fork virtualritz/monstertruck (created 2023-02-23, pushed 2026-10-07, 35 stars). The maintainer names it in #121. It is cited in #85, #91 and #114 as having Result-returning booleans, difference and symmetric_difference, and shapeops and stepio merged into one crate. Any STUDY of Truck should note it, though it keeps the same mesh-driven boolean lineage (https://github.com/ricosjp/truck/issues/121).
- Missed panic path: Curve::lift_up hits `unimplemented!("intersection curve cannot connect by homotopy")` for IntersectionCurve, so homotopy or loft from any boolean-made edge panics (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-modeling/src/geometry.rs#L92-L111).
- Missed, and it strengthens the determinism finding: even integer-keyed FxHashMaps, such as the polyline graph keyed by i64 grid cells, are not portable between wasm32 and 64-bit native. FxHasher's constant and rotation depend on target_pointer_width (https://github.com/rust-lang/rustc-hash/blob/f16da569fcb867fa1bc3b5dba18c9b8f7ca99483/src/lib.rs#L78-L81 and #L176-L181). So `graph.iter().next()` (polyline_construction/mod.rs#L87-L90) picks a different start on the two targets, although it is stable run to run on one target.
- Missed dependency hygiene: #125 reports the unmaintained proc-macro-error advisory (truck-derivers depends on proc-macro-error 1.0.4) and an lz4_flex vulnerability (https://github.com/ricosjp/truck/issues/125).

**Argument:** Not refuted. STUDY stands, and all six claim verdicts hold. The errors are in supporting detail, and none of them makes Truck a better fit for Spare Kit.

**Verified correct**
- **Licence.** The root licence is Apache-2.0, and every dependency is permissive: cgmath, matext4cgmath and ruststep are Apache-2.0; spade, rclite, parking_lot, rustc-hash and vtkio are MIT OR Apache-2.0.
- **Activity.** The metadata matches: created 2020-12-26, pushed 2026-10-19, 1,661 stars, 124 forks, 33 open issues and 4 open PRs (37 in total), 136 numbers used. Every crate's last crates.io release was 2024-09-20. The #135 panic and hang, and the maintainer's 'v0.7 soon, no date', are as described. PRs #110, #111 and #134 are open and unmerged, and #134 carries the CLA bot.
- **Booleans.** The mesh-driven pipeline in integrate/mod.rs#L75-L132 is as described:
  - the 2e-6 grid snap (polyline_construction#L26-L35);
  - Newton projection through search_triple (intersection_curve/mod.rs#L26-L51);
  - the `n0×t·n1 > 0` side test (loops_store#L55-L70);
  - unchecked Face::debug_new in release builds (divide_face#L92);
  - the hashed-ray count with a fixed `>= 1` threshold;
  - quadratic B-spline leaders;
  - the panicking Solid::new.
- **Panics and limits.** `subs` unwraps at intersection_curve.rs#L220, and `mat.invert().unwrap()` appears at #L173 and #L185. The modelling Curve and Surface enums have no analytic cylinder, cone, sphere or torus. lib.rs documents the transversal-only boolean limit and the single-edge fillet limit.
- **No compute shaders.** wgpu appears only in the platform and rendimpl crates, with no compute usage.
- **Determinism.** The pointer-based IDs and Face hash, the FxHashMap adjacency, and the libm cos/sin in take_one_unit are confirmed.
- **STEP.** The reader has no OFFSET, COMPOSITE or RECTANGULAR_TRIMMED entities, and Curve3D cannot be built from IntersectionCurve, which is issue #91.
- **SolidCraft.** The Cargo.toml dependencies and vendor patches, the more than 20 fixes in vendor/README (including the face-order fix at L17-L19), the nudge-and-retry layer, the move to solidcore (#1597, #331, #2661), the ROADMAP lines on equal-radius cylinders, and #2357 and #2862 are all as cited.
- **Other users.** CADmium is archived. HN 50064840 is the SolidCraft Show HN, and comment 50066980 says what was cited.

**What is wrong**
- **Geometric validity.** The sentence saying Truck has no edge-on-surface check is false. An opt-in check exists (is_geometric_consistent with IncludeCurve), but it is never run automatically, and it panics with `unimplemented!()` on every edge a boolean creates.
- **Culling.** 'No spatial culling' is overstated. Face pairs are checked all against all, but each pair uses triangle sweep-and-prune plus AABB rejection.
- **Mis-cited sources.** Issue #68 blames basis-function allocation, not O(F0×F1). The #924 platform flips are not shown to come from Truck alone.
- **Licence evidence.** truck-assembly and truck-drafting have no license field.
- **Activity count.** About 20% of the recent commits are automated 'cargo upgrade' bumps.
- **Minor slips.** A line pin is off by one, the truck-topology download count is rounded up, and the workspace has 17 members, not 15.

**Is the verdict too harsh or too generous?**
- **Not too harsh.** No error found makes the core usable under Spare Kit's rules. It still has no exact predicates, its intersection topology depends on the tessellation, it has no coplanar or tangent support, it fails by None or panic, and its IDs are pointer-hashed. My corrections add panic paths and a wasm-versus-native ordering source rather than removing any.
- **Not too generous.** REJECT would discard material worth studying under Apache-2.0: the NURBS algorithms and their property tests, the geotrait decomposition, the drafting crate's reuse of topology over Point2, and a well-documented failure corpus. That includes Truck's own issues and SolidCraft's vendor notes.
- **Not DEPEND or PORT.** DEPEND and PORT would bring in the mesh-driven boolean architecture. The live monstertruck fork does not change this, because it keeps the same boolean lineage, but the evaluation should mention it.

### Challenge 2: FIT: argue against STUDY by looking for any part of Truck th

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- Minor factual slip: truck-topology has 142,949 total crates.io downloads, not 'about 145k'. The 145,168 figure belongs to truck-modeling (crates.io API, read 2026-10-21). truck-shapeops has only 14,464, about a tenth of the others. So the booleans are the least-used part of Truck, and the most exercised path is STEP reading plus tessellation (truck-stepio has 108,643). This strengthens the 'partly' verdict on claim 1.
- Minor citation slip: SolidCraft's [patch.crates-io] block is at https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/Cargo.toml#L147-L152, not #L146-L151. The matext4cgmath patch is on L152, outside the cited range.
- The mechanism behind one determinism point is misread. The polyline-chaining graph is an FxHashMap keyed by PointIndex, which is an integer grid coordinate, not a pointer (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/src/transversal/polyline_construction/mod.rs#L26-L35 and #L53). So `graph.iter().next()` (#L87-L90) gives the same order on every run on one target. It changes between native x86_64 and wasm32 because rustc-hash's FxHasher multiplier depends on pointer width (https://github.com/rust-lang/rustc-hash/blob/c13e7ccca705e6255387a2ebc6dca142d6881621/src/lib.rs#L78-L81). I believe hashbrown's probe-group width also differs by target (unverified). The point still counts against bit-identical native and wasm output, but ASLR is not the cause here.
- Missed determinism mechanism, which strengthens the verdict. Tessellation jitter is a hash of the bits of `surface.subs(..)` (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-geotrait/src/algo/surface.rs#L254-L259). Every truck-modeling cylinder, cone, torus and revolve is a RevolutedCurve, evaluated through `Matrix3::from_axis_angle`, which uses platform sin and cos (truck-geometry/src/decorators/revolved_curve.rs#L12). The triangle sweep-and-prune axis also comes from `take_one_unit`, which uses libm cos and sin (truck-meshalgo/src/analyzers/collision.rs#L211-L215). Booleans are driven by meshes, so a last-bit libm difference can change the mesh layout and then the boolean's topology between native and wasm. The deterministic-hash idea praised in what_to_take is defeated by its own inputs.
- Evidence overstated: SolidCraft #924 does not pin a Truck defect. The issue says 'Likely cause (not confirmed)'. The failing stack includes SolidCraft's own offset and fillet code: several flips are 'offset: a corner whose faces don't meet...'. A later comment shows CI-built binaries failing on a different Linux machine. So it shows that the downstream stack is not deterministic, but the root cause is unverified and should be labelled that way.
- Missed: the active Apache-2.0 fork virtualritz/monstertruck (pushed 2026-10-07). The maintainer names it in issue #121. Its README says it keeps 'the upstream-derived single-ray algorithm' for booleans and wraps it in a Result API. It adds difference and symmetric_difference, T-splines and a rewritten fillet engine. It forked ruststep as step-p21 because two syntax fixes had sat unreleased for four years. This matters for the activity and ecosystem picture and for the 'known users are moving away' line, and it shows the mesh-driven boolean has not been re-architected even in the fork. It does not change the verdict.
- Missed in what_to_take: truck-base/src/ders.rs (https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-base/src/ders.rs#L6-L7, #L81-L97, #L174-L186, #L815-L830). It holds derivative jets up to order 10 for curves and surfaces: rational derivatives, derivatives of a magnitude, Leibniz products, and Faa di Bruno composition of a surface with a uv-curve. IntersectionCurve::der_n uses it to give exact higher-order derivatives of an intersection curve (intersection_curve.rs#L151-L210). It is pure arithmetic with no libm, and monstertruck has already ported it. It is the strongest PORT candidate. Even so it is study-only: it is tied to cgmath and fixed [V;11] and [[V;11];11] arrays, and the best design is a generic Taylor-mode jet.
- Missed: the rolling-ball and approximate fillet surface family and offset geometry (truck-geometry/src/decorators/rbf_surface/, af_surface.rs, edge_blend.rs, offset/), with tests in truck-geometry/tests/rbf_surface.rs and offset.rs. This is where most upstream work went in 2025-26: fillet commits from 2025-04 to 2026-03, offset on 2026-05-05. It is useful as a reference for defining blend surfaces with variable-radius functions and contact curves.
- Framing of one weakness: the boolean-edge representation (a leader curve plus two exact surfaces, solved by Newton with a mid-plane constraint; intersection_curve.rs#L4-L37) is an exact, surface-authoritative representation. As far as I know, Parasolid intersection curves use the same chart-plus-surfaces idea (unverified: I found no public spec). The defect is the evaluation path: `.unwrap()` at #L220 and `invert().unwrap()` at #L173 and #L185. The representation is sound. Spare Kit should adopt the representation with bounded-failure evaluation and stored charts or p-curves, not avoid it.
- Claim-verdict calibration (judgment, not a factual error). Claim 3 'false' is strong: wgpu does power Truck's GPU renderer (truck-platform/Cargo.toml#L24, truck-rendimpl), and the README's 'Rust and WebGPU' line refers to that. It is false only for kernel computation, so 'partly' is more precise. The renderer is also rarely used: about 11k downloads for truck-platform and 15.6k for truck-rendimpl. Claim 5 ('powers SolidCraft') is literally true today, and 'partly' reflects SolidCraft's announced move to solidcore.
- Minor dependency omissions. truck-geotrait takes rand 0.10 and getrandom (wasm_js) as runtime dependencies, used only by public random-test helpers (truck-geotrait/src/traits/curve.rs#L378-L477). truck-derivers depends on the unmaintained proc-macro-error (open issue #125). All licences are still permissive.
- Minor: truck-assembly, truck-drafting and truck-js do not exist on crates.io (crates.io API 404). Master truck-stepio depends on `truck-assembly = 0.1.0` (truck-stepio/Cargo.toml), so v0.7 cannot be published until a new crate is.

**Argument:** I re-checked the evaluation against master 78d699b and the issue tracker. Licence, activity, the issue citations, the mesh-driven boolean pipeline, the panics, `Face::debug_new`, pointer-address IDs and the STEP gaps are all accurate. They check out: 1,661 stars, 37 open issues and PRs, 261 commits in 12 months, 2024-09-20 releases, a CLA on #134, and the 'truck' contributor being a cargo-upgrade bot.

Arguing the opposite (PORT or DEPEND), the best candidates are:
- ders.rs: higher-order derivative jets with Leibniz and Faa di Bruno rules, pure arithmetic, already ported by monstertruck.
- The exact IntersectionCurve representation: a leader plus two surfaces, with exact higher-order derivatives.
- Seeding jitter and ray directions from a hash of the geometry instead of an RNG.
- Rolling-ball fillet surface definitions.
- Topology generic over geometry, with mapped and try_mapped.

None meets the owner's 'absolute best architecture' bar. ders.rs is tied to the unmaintained cgmath and to fixed-size arrays; a generic Taylor jet is better. IntersectionCurve's evaluation panics. The hash seeding is undermined by libm trig in RevolutedCurve and in take_one_unit, which feeds the mesh-driven booleans and so breaks bit-identical native and wasm output. The topology identity is a heap address.

DEPEND is ruled out on every axis: the only boolean test is a smoke test, there is a single absolute TOLERANCE, coplanar and tangent cases are unsupported by design, failure is None or a panic, there is no determinism, the bus factor is 1, releases are stale, and a cgmath-to-nalgebra API break is planned. The fork monstertruck keeps the same boolean algorithm, and SolidCraft is leaving Truck for an SMLib port.

REJECT would be too harsh. The Apache-2.0 geometry layer, the derivative algebra, the fillet surface definitions and the documented failure corpus (Truck #57, #114, #134, #135; SolidCraft's vendor README) are worth studying and turning into tests.

My corrections are minor slips (download count, a line range), a misread determinism mechanism (FxHash pointer width, not ASLR, for the polyline graph), an overstated downstream attribution (SolidCraft #924's root cause is unconfirmed), some calibration on claims 3 and 5, and missed items (monstertruck, ders.rs, fillet surfaces). None overturns a key finding, and STUDY stands.

## cadcore

**Repo url:** https://github.com/YATSKOVSKYI/cadcore (HEAD read: 4c7c358bc1f9487070de50efdc1f7e8303f04ca1; published-release source: branch release/0.1.23-mit at aeed2d86a6d18201bd0a2506e4d9b05baa2bdff7)

**Licence:** MIT. The LICENSE file at HEAD is MIT, copyright 2025-2026 Dmytro Yatskovskyi (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/LICENSE). Each crate also carries its own MIT LICENSE file. Versions 0.1.0 to 0.1.21 were published under PolyForm-Noncommercial-1.0.0. On 2026-10-16 the author added an MIT grant covering them (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/LICENSE-HISTORY.md#L1-L47), but crates.io metadata for those versions still says PolyForm. Versions 0.1.22 and 0.1.23 are MIT. The only third-party dependencies are slotmap 1.1.1 (Zlib) and version_check (MIT or Apache-2.0). The licence is compatible with MPL-2.0, so it does not block use. The engineering does.

**Activity:** Created 2026-05-27. Last push 2026-10-16, but that commit only changed licence docs. The last code commit is c618cd2 on 2026-08-22.
- Commits and people: 33 commits, all by YATSKOVSKYI, 1 contributor, no PRs.
- GitHub: 43 stars, 7 forks.
- Issues: #1 is open (an implicit-geometry feature request). #2 is open: a third party found the released STEP writer unreadable by OpenCASCADE. #3 is closed (the licence mismatch).
- crates.io: 24 versions. The latest is 0.1.23 (2026-10-16), a licence-only release whose Rust sources the maintainer says are byte-identical to 0.1.22.
- Downloads: 574 (cadcore) to 1,136 (cadcore-math) in total, and zero public reverse dependencies.
- Size: master has about 34k lines of Rust in 56 files, 6,026 of them in the single file cadcore-ops/src/union.rs. The published crates have 24 .rs files; lib.rs reports 5.5K SLoC and ranks it #1894 in Graphics APIs.
- Release state: the cadcore-union crate is unpublished, and master cannot be published as-is because cadcore-ops depends on it (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-ops/Cargo.toml#L15-L19). No published version contains any union.
- No CI: .github is absent.
- Tests reference a private consumer repo, gcode-viewer-rs (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/CLAUDE.md#L101-L102).

**Claims**

- **claim**: It is a rising alternative optimised to bypass expensive C++ dependencies; **verdict**: partly; **evidence**: True that it has no C++. The lockfile holds only the 7 workspace crates plus slotmap and version_check (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/Cargo.lock), and there is no unsafe code. The rest does not hold.

"Rising" is unsupported: 43 stars, one author, a 5-month history, the last code change on 2026-08-22, and zero public reverse dependencies on crates.io.

"Alternative" (the README says it replaces OpenCASCADE) is false. The scope is set by its own source:
- The topology has only four surface types: Plane, Cylinder, Sphere and Torus (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-topo/src/entities.rs#L294-L305).
- The legacy union says it is "not a generic OCCT-style boolean kernel" and targets "axis-aligned, equal-radius cylinders" (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-ops/src/union.rs#L1-L16).
- The published crates contain no union at all (https://github.com/YATSKOVSKYI/cadcore/blob/aeed2d86a6d18201bd0a2506e4d9b05baa2bdff7/crates/cadcore-ops/src/lib.rs).
- **claim**: It specialises in mathematically exact analytic curves and surfaces mapping directly to STEP entities with zero polygonal approximations; **verdict**: false; **evidence**: The primitive surfaces are exact and are written as exact STEP surfaces, but derived geometry is sampled:
- Intersection edges are stored as sampled polylines: `EdgeGeom::Polyline(Vec<Point3>)`, described as "tolerance-sampled surface–surface intersection curves" (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-topo/src/entities.rs#L217-L236).
- The STEP writer emits those edges as degree-1 B-splines or cubic interpolants (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-step/src/entities.rs#L479-L485, https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-step/src/interp.rs#L1-L12). Their pcurves are uv polylines (`emit_polyline2d`).
- The intersection tracer marches in fixed 0.01 mm steps (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/geom/intersect.rs#L22-L48).
- The swept-tube surface uses sampled rotation-minimizing-frame (RMF) stations (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-geom/src/swept.rs#L240-L270).
- The kernel has no NURBS; they exist only inside the STEP writer. ConeSurf exists but is not a valid face type.
- The "maps directly to STEP" part is also broken in releases. The published writer wraps ADVANCED_FACE's single surface in an aggregate (https://github.com/YATSKOVSKYI/cadcore/blob/aeed2d86a6d18201bd0a2506e4d9b05baa2bdff7/crates/cadcore-step/src/writer.rs#L204-L214), which OpenCASCADE reads as zero faces (issue #2, third-party report, not re-run by me).
- HEAD still writes a LINE that references a DIRECTION instead of a VECTOR at three sites (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-step/src/writer.rs#L1604-L1607, #L1761-L1764, #L1856-L1859).
- **claim**: It has an O(N) filament sweep solver that skips heavy, slow Boolean operations; **verdict**: partly; **evidence**: It is O(N) because it emits one templated face per path segment: a cylinder per line, a torus per arc, end disks, and miter ellipses as shared boundaries (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-ops/src/sweep.rs#L1-L14, #L571-L668, #L724-L912). That is an ordinary pipe sweep. OpenCASCADE's BRepOffsetAPI_MakePipe also sweeps a profile along a spine with no booleans, so the README's claim of "O(N²) Boolean unions" in OCCT is a strawman (https://dev.opencascade.org/doc/refman/html/class_b_rep_offset_a_p_i___make_pipe.html).

It skips booleans by not checking for overlap:
- The only self-intersection check is torus arc radius ≤ profile radius (sweep.rs #L739-L748).
- Nothing detects a path coming within 2r of itself, or short segments between miters.
- Near-180° turns fall back to a shared perpendicular circle (sweep.rs #L1960-L1990). This leaves overlapping tubes, i.e. a self-intersecting solid.
- Loops are placeholders with no real edges (sweep.rs #L1659-L1664).

Once filaments overlap, as in real scaffolds, the author needs the 6,026-line union. Verdict: a narrow special case (circular profile, line or arc spine, no self-proximity), not a general method.
- **claim**: README: exported STEP files are watertight and "open flawlessly in SolidWorks, Inventor, Fusion 360, FreeCAD, Rhino, and CATIA"; **verdict**: false; **evidence**: Issue #2 (2026-10-14, still open) shows every STEP file from the published 0.1.22 comes back from OpenCASCADE with zero faces, so FreeCAD and occt-import-js cannot open it. The cause is the two entity-type errors above. The license-only 0.1.23 release did not fix it.

The repo's own check is `examples/validate_step.rs`, which only counts ORIENTED_EDGE uses in the writer's own text. There is no CI and no round trip through an independent reader. README line: https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/README.md#L31-L32.
- **claim**: cadcore-union is an "Industrial Boolean-union engine" / a "Parasolid-grade universal entry point"; **verdict**: false; **evidence**: Its own docs say: "Only the planar-face case (e.g. boxes / prisms) is wired today; faces whose ... SSI is not yet supported are ... skipped when they do [meet]" (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/union.rs#L27-L34). The pipeline is at stage P0, i.e. only the validation step runs, and the legacy union stays the production path (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/lib.rs#L10-L15).

- Surface pairs it does not support return no intersection: `_ => Vec::new()` (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/ssi.rs#L112-L113).
- Known failures are marked #[ignore]: "torus×torus not watertight yet (~300 open)" (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/mod.rs#L510) and "47 open edges on a real scaffold" (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/halfspace.rs#L1355).

**Architecture:** Seven crates in one workspace (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/Cargo.toml#L1-L11).

cadcore-math: f64 Point3, Vec3, UnitVec3, Mat3, Frame3, Transform3 and Interval, with one global absolute `EPS = 1e-10` (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-math/src/lib.rs#L63-L72).

cadcore-geom:
- Curves: Line3, Circle3, Ellipse3 and BezierCubic.
- Surfaces: Plane3, CylSurf, ConeSurf, SphereSurf and TorusSurf.
- SweptTubeSurface, a sampled-RMF tube.
- Intersection: a closed-form cylinder∩cylinder for equal radii only, and marching-squares and Levenberg–Marquardt intersection tracers.
- A uv DCEL arrangement.

cadcore-topo: a slotmap arena B-rep (Vertex, Edge, CoEdge, Loop, Face, Shell, Solid) with typed IDs. Every arena is a `pub` field with no invariant enforcement (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-topo/src/brep.rs#L20-L36).
- The face carrier enum is limited to Plane, Cylinder, Sphere and Torus (entities.rs #L294-L305).
- Edges are Line, Circle, Ellipse or Polyline (#L217-L236). Edges have no tolerances, and faces store no pcurves.
- Face extent is not general. A `FaceExtent` enum of feature templates carries it: Cylinder, PartialCylinder, Disk, PartialDisk, TorusFillet, RoundedRectCap, CylinderArcFace (an \"electrode\") and so on. Only the `Trimmed` variant uses real loops (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-topo/src/entities.rs#L84-L204).
- So swept solids carry placeholder loops, and edges first appear inside the STEP writer.
- `fuse_solids` just concatenates the shells of every solid into one Solid (brep.rs #L299-L319).

cadcore-ops: sweep.rs (2,802 lines; pipe sweeps plus box, rounded-box and plate builders) and boolean.rs (a half-space cut per cylinder face). union.rs (6,026 lines) is the legacy scaffold union. It is organised as passes over woodpile cases, uses 70 eprintln or env-var toggles, and leaves out solids by name: anything named `electrode*` is skipped (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-ops/src/union.rs#L3618-L3623).

cadcore-union (unpublished): the replacement union. It has a staged pipeline, a tolerance ladder, a refine-onto-surface oracle, a predictor–corrector intersection tracer, a uv DCEL, ray-cast point-in-solid, and validation gates. It is at P0–P3 (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/README.md#L72-L80).

cadcore-step: a write-only AP203/AP214 writer (writer.rs is 3,872 lines) with no reader. It writes NURBS only for torus patches and interpolated intersection curves.

cadcore: a facade that re-exports everything.

Not present at all: general NURBS curves or surfaces, offsets, fillets or blends beyond the sweep torus, subdivision, implicit modelling, meshes or tessellation, 2D drafting, a STEP reader, and difference or intersection booleans (union only).

**Robustness:** Arithmetic is plain f64 with absolute, millimetre-scale tolerances spread through the code:
- `EPS = 1e-10`; the legacy union uses `linear_tolerance 1e-4` (union.rs #L58-L69).
- The cut uses `PARALLEL_TOL = 0.05` (about 3°) and `COINCIDENT_TOL = 1e-6` (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-ops/src/boolean.rs#L356-L357).
- Ad-hoc constants such as 0.08 mm and 1e-4 appear inside the union.
- The new engine sets weld = 2e-2 and sliver = 5e-2 mm (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/tolerance.rs#L1-L50).

There are no exact or adaptive predicates and no interval arithmetic anywhere.

Point-in-solid is ray-parity over 6 fixed directions. If every direction grazes something, it falls back to an unchecked count (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/contain.rs#L24-L44).

Intersection (SSI):
- Seeds come from pairwise centreline sampling, and only traces that close are kept. Budget, Diverged and Tangency traces are dropped silently (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/geom/composite.rs#L327-L398).
- With the default step of 0.01 mm and 4,096 points, a loop longer than about 41 mm cannot close. Unions at building scale would lose their intersections. This is inferred from source, not executed.
- The closed-form cylinder∩cylinder returns nothing for unequal radii or |cos| > 0.9. The source comment itself warns this leaves \"untrimmed self-intersecting overlaps\" (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-geom/src/intersect.rs#L45-L81).

Degenerate cases are handled by heuristic rejection, which leaves faces untrimmed and the solid invalid:
- The legacy union's phantom-loop guard does \"reject the loop (leave untrimmed)\" (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-ops/src/union.rs#L810-L817).
- The mutual-bite case is skipped \"for now\".
- Torus fillets and sphere caps are \"preserved untouched\", i.e. merged into the shell without being trimmed.

The half-space cut has three problems:
- A face that straddles the plane is kept whole, which leaves material past the plane, and only an eprintln reports it (boolean.rs #L163-L177, #L313-L318).
- Oblique axial cuts are classified by the axis endpoints only and capped with a disk perpendicular to the axis, so the cap is not on the cutting plane (#L396-L437, #L262-L283).
- No operation returns a validity Result. The new engine's union returns `(BRep, ShellId)` and expects \"the caller validates\".

Tests:
- 219 #[test] functions, no fuzzing or property tests, no CI.
- Six tests are #[ignore]d as known failures: torus×torus, real-scaffold fuse (4 dropped faces), real-scaffold cut (47 open edges), and two coplanar-cap cases (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/tests/step_export.rs#L837, #L881).

From the issue tracker, the only real external failure report is #2: published STEP is unreadable by OCCT, and it is not fixed at HEAD or in any release. The author's README admits the legacy union \"was grown case-by-case ... every new geometric situation ... needs hand-written passes\" (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/README.md#L10-L15).

**Determinism:** The code is not designed for determinism, and nothing tests native against WebAssembly.
- It uses f64 throughout, with about 172 calls to std sin, cos, atan2, acos, tan, sqrt and similar. The Rust std docs say the transcendental functions have platform-dependent precision, so native and wasm results can differ.
- Results can also flip within one platform. A comment says a HashMap's random iteration order made near-tangent vertex merges \"flip otherwise-identical layers between manifold and open\" from run to run. The fix was a BTreeMap at that one site (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-ops/src/union.rs#L370-L376). That shows topology depends on tolerance comparisons right at the threshold.
- The 88 other HashMap and HashSet uses are mostly lookups, so run-to-run behaviour is probably stable after that fix. Bit-identity across platforms is unverified.
- Issue #2's reporter built it to wasm: 134 KB, zero imports, about 2 ms per part. That shows it compiles to wasm, not that results match native.

**Strengths**

- Pure Rust with almost no dependencies (slotmap and version_check), no unsafe code, and MIT licensed. It builds to small wasm (issue #2).
- Its typed slotmap IDs follow the same arena-plus-typed-handle pattern Spare Kit would use, though without enforced invariants.
- Its notes are honest about scope and known failures: the union.rs header, the cadcore-union README, and the #[ignore] reasons.
- It records concrete lessons about what STEP importers accept: every EDGE_CURVE used exactly twice with opposite senses; LINE must reference a VECTOR; ADVANCED_FACE takes a single surface; write degree-1 B-splines instead of POLYLINE because SolidWorks drops POLYLINE edges; physically wind loops because SolidWorks ignores FACE_BOUND flags; SolidWorks knits at about 1 µm. Sources: https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/CLAUDE.md#L1-L114 and https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/README.md#L17-L41. These are the author's empirical claims, not verified by me.
- It has an explicit tolerance ladder with a runtime ordering check (refine ≤ model ≤ knit), and in-process validation gates: edge used twice, edge-to-surface deviation, and uv wire closure, winding and self-intersection.

**Weaknesses**

- It is a filament-scaffold tool, not a kernel. Faces can only be Plane, Cylinder, Sphere or Torus; the kernel has no NURBS, cones, offsets, general fillets, subdivision, implicits, meshing, 2D drafting or STEP reader.
- The topology is template-driven: face shape lives in a `FaceExtent` enum of named features like RoundedRectCap and an electrode CylinderArcFace. Swept solids have placeholder loops, and edges exist only in the STEP writer. This is the opposite of a general, invariant-checked B-rep.
- Topology can be built invalid freely: the arenas are public fields, `fuse_solids` concatenates shells, and nothing returns a validity Result.
- Intersection curves are stored as polylines and pcurves as uv polylines, so it is not exact.
- There are no exact predicates; it uses absolute mm tolerances and ad-hoc constants, and a fixed trace budget of about 41 mm. Scale-dependent and unsuitable for building-scale coordinates.
- Booleans fail silently: unsupported surface pairs return no curves, rejected loops leave faces untrimmed, straddling faces are kept, and oblique cuts get wrong caps. The general union handles only planar faces, and there is no difference or intersection operation.
- The sweep does no self-intersection detection apart from torus radius ≤ profile radius. Near-reversals and paths that pass close to themselves produce self-overlapping solids.
- Published releases write STEP that OCCT cannot read (issue #2). HEAD still has the LINE→DIRECTION bug at three sites. There is no CI and no round trip through an independent reader.
- It is a one-person, 5-month-old project with its real regression suite in a private repo. The legacy union is 6,026 lines of case-specific passes with env-var debugging. Some source comments are mojibake (corrupted encoding) in boolean.rs.
- No determinism design: std transcendental functions and threshold-sensitive merges; one documented run-to-run nondeterminism fixed by a BTreeMap.

**Verdict:** REJECT

**Verdict reason:** Measured against Spare Kit's goals, cadcore fails on every axis that matters:
- Representation: exact NURBS plus analytic surfaces in a general, invariant-enforcing B-rep is required. cadcore has four analytic surface types, feature templates instead of real topology, and polyline intersection edges.
- Robustness: exact predicates and booleans that never fail silently are required. cadcore uses f64 absolute epsilons and leaves faces untrimmed on reject paths.
- Determinism: nothing is designed or tested for it.
- Scale: the tolerances and trace budget are tuned for 0.25 mm filaments.
- Maturity: there is no CI, a broken STEP release, and known failures parked under #[ignore].

The headline "O(N) sweep without booleans" is a plain pipe sweep with no self-intersection checks. It is a special case, not a method to port. OCCT's MakePipe is the textbook equivalent.

The licence (MIT) is fine, but nothing here matches the best architecture, so it should not be a dependency or a port. It is not worth a study track either: the few lessons it holds (STEP importer pitfalls, a tolerance ladder, validity gates) exist in more authoritative form in ISO 10303 and CAx-IF guidance and in OCCT's BRepCheck.

**What to take**

- Nothing to depend on or port. Optional 10-minute read, as a negative example for Spare Kit's STEP export tests: issue #2 (https://github.com/YATSKOVSKYI/cadcore/issues/2). It shows why a writer must round-trip through an independent reader such as OCCT in CI, rather than only checking its own output with string counts (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-step/examples/validate_step.rs).
- Optional: the importer-behaviour notes in https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/CLAUDE.md#L1-L114 and https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/README.md#L17-L41. Use them as a checklist of interop test cases (SolidWorks drops POLYLINE edges and ignores FACE_BOUND orientation, knits at about 1 µm, and requires uv wires that do not self-intersect). Each item is the author's unverified claim, to be confirmed against real importers.

### Challenge 1: FACTS: re-verify the licence, activity numbers, claim verdic

**Refuted:** False

**Corrected verdict:** REJECT

**Errors found**

- Wrong file count. HEAD has 65 .rs files, not 56. The git tree at 4c7c358 lists 65 *.rs blobs totalling 34,038 lines. The 34k-line total, union.rs at 6,026, sweep.rs at 2,802 and writer.rs at 3,872 are all correct. The published release has 24 .rs files, which is also correct. Tree: https://github.com/YATSKOVSKYI/cadcore/tree/4c7c358bc1f9487070de50efdc1f7e8303f04ca1
- Wrong method description: 'The general union handles only planar faces' (claim 5 and the weaknesses list). It also says the pipeline 'is at stage P0, i.e. only the validation step runs'. Both rest on stale doc comments (boolean/union.rs#L31-L34 and cadcore-union/src/lib.rs#L10-L15). The code at HEAD dispatches SSI for Plane×Plane, Plane×Cylinder, Cylinder×Cylinder, Torus×Cylinder and Torus×Torus, plus Plane×Torus as opt-in (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/ssi.rs#L67-L113). It also handles parallel cylinders through `parallel_cyl_cyl` (#L449-L466). union_n gathers Cylinder and Torus carriers for seam and triple-point splitting (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/union.rs#L56-L116). Tests not marked #[ignore] union a cylinder through a box, two crossing cylinders of unequal radii, a thin cylinder through a thick one, a cylinder cutting a torus elbow, two clean tori and parallel stacked cylinders (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/mod.rs#L208-L241, #L326-L336, #L344, #L480, #L544). I did not run these tests, and there is no CI. They also only count edge uses. Spheres are the only face type with no SSI. The evaluation also contradicts itself: its architecture section says 'P0–P3'.
- Wrong method description: 'Torus fillets and sphere caps are "preserved untouched", i.e. merged into the shell without being trimmed.' This quotes the stale v1 header of the legacy union (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-ops/src/union.rs#L9-L13). The same file defines `TorusFace` as 'A torus *fillet* face ... slated for trimming' (#L108-L117). It also runs a torus×cylinder trim pass that pushes torus holes and notches (#L532-L560, and torus_holes at #L716-L745). Torus fillets are trimmed at HEAD. Only sphere caps are untouched, and that is unverified beyond the header.
- Imprecise: 'The intersection tracer marches in fixed 0.01 mm steps.' The step is adaptive downward: `step_eff = opts.step * cl.clamp(0.15, 1.0)` (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/geom/intersect.rs#L235-L237). Torus×torus and plane×torus use 0.02 mm (ssi.rs#L134, #L231). The '~41 mm loop cannot close' bound still holds as an upper bound for the default options, because the step never grows. But it applies only to traced pairs, not to plane×plane or plane×cylinder, so 'unions at building scale would lose their intersections' is overgeneralised.
- Mis-sourced strength. The evaluation credits 'LINE must reference a VECTOR', 'ADVANCED_FACE takes a single surface' and 'SolidWorks drops POLYLINE edges' to CLAUDE.md#L1-L114 and cadcore-union/README.md#L17-L41. Neither file contains any of them. The POLYLINE note is at https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-step/src/entities.rs#L479-L485. The LINE/DIRECTION note is a code comment at writer.rs#L2346. The ADVANCED_FACE point is a third party's diagnosis in issue #2 (https://github.com/YATSKOVSKYI/cadcore/issues/2), which the author fixed in writer.rs#L679-L689. It is not one of the author's own empirical notes.
- Quote taken out of context. The evaluation says the closed-form cyl∩cyl 'source comment itself warns this leaves "untrimmed self-intersecting overlaps"'. The comment explains why a strict perpendicularity test was not used, not the behaviour of the current |cos|>0.9 rejection (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-geom/src/intersect.rs#L68-L80). The evaluation's inference may still hold, but the attribution is wrong. The evaluation also leaves out that the new engine's cyl_cyl handles any angle, unequal radii and the parallel case through the tracer (ssi.rs#L449-L466), so the cadcore-geom limit now applies only to the legacy path.
- Claim 2 ('zero polygonal approximations') is called 'false' without separating HEAD from the release. The published 0.1.22 and 0.1.23 sources have no EdgeGeom::Polyline, no B_SPLINE output and no SSI. cadcore-geom there contains only curves.rs and surfaces.rs (https://github.com/YATSKOVSKYI/cadcore/blob/aeed2d86a6d18201bd0a2506e4d9b05baa2bdff7/crates/cadcore-topo/src/entities.rs). For the shipped sweep and cut, the claim holds geometrically, apart from the broken STEP entities. It is false only for the unreleased union output at HEAD. 'Partly' would be more precise. The evaluation also missed stronger HEAD evidence: even plane×cylinder, where the exact conic is known, is sampled into 96 points and returned as a polyline (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/ssi.rs#L713-L721).
- Licence nit. The evaluation says crates.io metadata for 0.1.0 to 0.1.21 'still says PolyForm'. For 0.1.0 it says `non-standard`, and only 0.1.1 to 0.1.21 say PolyForm-Noncommercial-1.0.0 (crates.io API /api/v1/crates/cadcore; https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/LICENSE-HISTORY.md#L31-L35). The MIT grant and MIT for 0.1.22 and 0.1.23 are confirmed.
- Missed context from issue #2. After patching F1 and F2, the third-party reporter read a box, a filleted sweep rod, a rounded box and a half_space_cut output back through OCCT. All four came back closed, with 0 boundary edges and 0 non-manifold edges, and the reporter concluded 'the kernel's topology and geometry are sound' (https://github.com/YATSKOVSKYI/cadcore/issues/2). The README claim of 'open flawlessly' is still false for every release. But the only independent test found the released primitives valid once the serializer was fixed. The evaluation also missed that cadcore-union's own halfspace.rs header says the legacy cadcore-ops cut 'silently does nothing' on fused shells (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/halfspace.rs#L5-L8). That is another silent failure in its favour.
- Minor determinism and count nits. Counting sin, cos, atan2, acos, tan and sqrt over all crates gives about 201 calls, not about 172. sqrt is correctly rounded under IEEE 754 and is not one of the platform-dependent transcendentals, so it should not be counted with them. 'Master cannot be published as-is because cadcore-ops depends on cadcore-union' is overstated: cadcore-union could be published first. The more immediate blocker is that master is still at workspace version 0.1.22, which is already taken.

**Argument:** The REJECT verdict stands, and so do most key facts.

Confirmed:
- Licence: MIT at HEAD and in each crate. LICENSE-HISTORY grants MIT for 0.1.0 to 0.1.21. Dependencies are slotmap 1.1.1 (Zlib) and version_check 0.9.5.
- Activity: created 2026-05-27, 33 commits by one author, 43 stars, 7 forks. Last code commit c618cd2 on 2026-08-22. 4c7c358 and aeed2d8 change only licence docs and manifests.
- Issues #1 to #3 as described.
- crates.io: 24 versions, downloads 574 to 1,136, no external reverse dependencies, cadcore-union unpublished. lib.rs: 5.5K SLoC, #1894 in Graphics APIs.
- No .github directory and no CI.
- Every pinned citation I re-read says what the evaluation claims: 219 tests and 6 #[ignore]s; the 4 FaceGeom types; FaceExtent templates; public arenas; fuse_solids concatenating shells; EPS 1e-10; tolerances; 6-direction ray parity with an unchecked fallback; open traces dropped; LINE→DIRECTION at writer.rs L1607, L1764 and L1859; the aggregate ADVANCED_FACE in the release; straddling faces kept with an eprintln; oblique cut caps perpendicular to the axis; the electrode* skip; the BTreeMap nondeterminism comment; 70 eprintln/env-var lines; mojibake.

The material errors are in method description. Stale doc comments were taken as fact. At HEAD the new union is not planar-only and not 'P0 validation only': it has SSI and passing-looking tests for plane, cylinder and torus pairs, including unequal radii and parallel tubes. The legacy union does trim torus fillets. The evaluation therefore understates what HEAD does.

This does not change the verdict. Every curve the new engine produces is an f64 polyline, even where a closed-form conic is known (plane×cylinder is sampled at 96 points). Tolerances are absolute mm search radii (weld 2e-2, sliver 5e-2). Unsupported pairs, including anything with a sphere, return an empty Vec, and non-closing traces are dropped silently. The tests only count edge uses. Nothing gives exactness, exact predicates, invariant-enforced topology, NURBS or determinism.

The verdict is not too harsh. The useful parts are a refine-onto-emitted-surface stage, triple-point welding, a tolerance ladder and SolidWorks importer quirks. They belong in an interop test checklist, which the evaluation's optional what_to_take already covers. They are not an architecture worth a STUDY track.

Claim 2 would be more precise as 'partly', since the published release really is all-analytic. No claim verdict flips direction, so refuted=false.

### Challenge 2: FIT: the evaluation says REJECT, so I looked for any part of

**Refuted:** True

**Corrected verdict:** REJECT

**Errors found**

- Claim 2 ('exact analytic curves/surfaces mapping directly to STEP, zero polygonal approximations') should be 'partly', not 'false'. The published crates (0.1.22/0.1.23, the only version anyone can depend on) have no sampled geometry at all. EdgeGeom is Line|Circle|Ellipse only (https://github.com/YATSKOVSKYI/cadcore/blob/aeed2d86a6d18201bd0a2506e4d9b05baa2bdff7/crates/cadcore-topo/src/entities.rs#L132-L139). FaceGeom is Plane|Cylinder|Sphere|Torus (same file #L199-L208). The release STEP writer and entities.rs contain no B_SPLINE or POLYLINE emission (grep count 0). Polyline edges, cubic interpolants and torus NURBS exist only on unpublished master, as output of the union. What is broken in releases is the STEP syntax (issue #2), not exactness. The SweptTubeSurface evidence also misleads: it is not a face carrier (FaceGeom has no such variant), and for a circular profile the point set is exact. The RMF stations only affect the v parametrisation (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-geom/src/swept.rs#L325-L333, #L405-L410).
- Factual error: the weaknesses section says the general union 'handles only planar faces', and claim 5 quotes a stale docstring. The actual dispatch in boolean/ssi.rs intersects plane×plane, plane×cylinder, cylinder×cylinder (any radii, via the tracer), torus×cylinder and torus×torus, plus plane×torus when opted in (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/ssi.rs#L67-L114). Non-ignored tests assert watertight unions for these cases: cylinder through box, crossing unequal-radius cylinders, thin through thick, a cylinder cutting a torus elbow, and clean torus×torus (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/mod.rs#L209-L242, #L327-L344, #L480-L508). Claim 5's verdict of 'false' still holds: sphere pairs fall through to `_ => Vec::new()`, and torus×torus elbows plus a real-scaffold cut are #[ignore]d.
- The 'P0 only, only the validation step runs' statement is true of the UnionPipeline facade (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/pipeline.rs#L14-L17), not of the crate. union_n, fuse_scaffold and half_space_cut are live construction paths, and the legacy union already imports cadcore_union's tracer (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-ops/src/union.rs#L1342-L1348). The evaluation also contradicts itself: the claims section says P0, the architecture section says P0–P3. It also omits cadcore-union's own fused-shell half_space_cut, a restricted difference (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/halfspace.rs#L1-L33, #L94-L99), so 'union only' is slightly overstated.
- Size error: master has 65 .rs files (34,038 lines), not 56 (git tree of 4c7c358). Minor: the crates.io licence field for 0.1.0 is 'non-standard', not PolyForm (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/LICENSE-HISTORY.md#L31-L35; crates.io API).
- Determinism detail is wrong: sqrt is listed among the platform-dependent functions, but Rust documents f64::sqrt as correctly rounded per IEEE 754 squareRoot and 'guaranteed not to change' (https://doc.rust-lang.org/std/primitive.f64.html#method.sqrt). sin and atan2, by contrast, are documented as varying 'even within the same execution', which undercuts 'probably stable run-to-run'. Calls at HEAD: sin 37, cos 40, sin_cos 23, atan2 69, acos 7, asin 1, tan 5, hypot 4, plus 43 sqrt that should not be counted.
- Missed determinism hazard: environment variables change the geometry, not just debug output. In the legacy union these are CADCORE_ELBOW_TRIM, CADCORE_CORNER_TRIM, CADCORE_CORNER_TRACER, CADCORE_CAP_TRIM, CADCORE_MUT_ZBAND and CADCORE_MUT_ZBAND_EXCL (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-ops/src/union.rs#L783, #L1211, #L1342-L1345, #L2046, #L2526-L2533). In the new scaffold engine they are CADCORE_WIN_MAX, CADCORE_DEDUP_R and CADCORE_NO_XPASS (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/arrange/scaffold.rs#L298-L318). The same input gives different solids depending on the process environment, and wasm has no environment.
- Misquote: the cylinder∩cylinder comment does not warn that the unequal-radius or |cos|>0.9 rejection leaves overlaps. It explains that a strict perpendicularity test would leave them, which is why the test was loosened (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-geom/src/intersect.rs#L69-L81). The inference is still valid; the attribution is not.
- Imprecise: the tracer step is nominal and shrinks to as little as 0.15× near tangency (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/geom/intersect.rs#L235-L238). The ~41 mm closable-loop limit is therefore an upper bound, which makes the evaluation's scale point stronger.
- Imprecise: 'pcurves are uv polylines'. On cylinder faces with 4 or more points, the pcurve is a cubic B-spline that shares the 3D curve's parametrisation (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-step/src/writer.rs#L1080-L1124). Missed defect: on torus faces the same SURFACE_CURVE pairs a cubic 3D curve with a degree-1 pcurve of a different parametrisation (writer.rs #L1127-L1147).
- Missed STEP silent failures. A face with no surface id is skipped with `None => continue`, and a face with no bounds is written as ADVANCED_FACE('',(),...) (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-step/src/writer.rs#L636-L639, #L677-L681).
- Missed boolean silent failures. Plane×torus SSI is off by default in the union, so an end cap meeting a neighbour's torus elbow gets no curve (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/boolean/ssi.rs#L28-L40). The author admits the legacy cut 'silently does nothing' on a fused shell (halfspace.rs #L4-L8). Faces without a template are classified by their carrier's frame origin, and a solid with no template cylinders returns Unchanged (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-ops/src/boolean.rs#L179-L200).
- Missed robustness gap, inferred from source and not executed. rounded_path_from_polyline checks each fillet setback against its own segment only, never that two adjacent setbacks fit together. A short middle segment then yields a reversed Line from the previous exit back to the next entry. That produces a self-overlapping sweep with no error (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-ops/src/sweep.rs#L310-L336).
- The validation-gate strength is overstated. Gate 4 checks a polyline edge only at its own defining points, which lie on the surface by construction, and never checks the emitted cubic between them. It also skips spheres (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-union/src/validate/distance.rs#L34, #L45). The wire gate skips torus and sphere faces (validate/wires.rs #L12-L15). The 'watertight' unit tests check edge-use counts only, and one deliberately uses unequal radii 'to dodge the equal-radius tangency', which is the dominant scaffold case (boolean/mod.rs #L223-L226).
- Missed positive evidence (the FIT side). The third-party reporter on issue #2 patched the two serializer bugs and round-tripped through OCCT. The 20 mm box, filleted rod, rounded box and half_space_cut output all came back closed and manifold with zero boundary edges. They wrote that 'the kernel's topology and geometry are sound' (https://github.com/YATSKOVSKYI/cadcore/issues/2). The evaluation cites only the negative half. This is real but narrow: it covers analytic primitives, not booleans.
- The verdict_reason overreaches when it says the lessons 'exist in more authoritative form in ISO 10303 and CAx-IF guidance'. The SolidWorks/SpaceClaim importer quirks (faces with POLYLINE edges dropped, FACE_BOUND flags ignored, ~1 µm knit) are not in ISO 10303, and the evaluation's own what_to_take recommends reading them. It also misses one lesson: TOROIDAL_SURFACE faces bounded only by two minor circles are ambiguous between the short and long band, so importers pick the wrong one. The author's workaround is an exact rational NURBS elbow (https://github.com/YATSKOVSKYI/cadcore/blob/4c7c358bc1f9487070de50efdc1f7e8303f04ca1/crates/cadcore-step/src/nurbs.rs#L1-L24). This belongs in the optional interop checklist (author's claim, unverified).

**Argument:** The REJECT verdict should stand. I set refuted=true only because one claim verdict and several facts are wrong, not because the overall verdict is.

**Errors in either direction**
- **Too harsh on claim 2.** The published 0.1.22/0.1.23 crates are purely analytic. They have Line/Circle/Ellipse edges, Plane/Cylinder/Sphere/Torus faces, and no spline or polyline output. Sampled intersection curves exist only in unpublished master. The claim deserves 'partly', not 'false'.
- **Wrong about the new union's coverage.** The evaluation quoted stale docs. The cadcore-union code really does union planes, cylinders of any radii and tori, with passing watertight tests. The 'planar only' and 'only validation runs' statements are wrong.
- **Smaller errors:**
  - 65 .rs files, not 56.
  - sqrt is correctly rounded, so it is not a determinism risk.
  - The cylinder∩cylinder comment is misattributed.
  - The tracer step is not fixed; it shrinks near tangency.
  - On cylinder faces the pcurves are cubic B-splines, not polylines.

**Steelman for STUDY (the FIT lens)**
The best case for STUDY goes like this:
- An independent OCCT round-trip (issue #2) found the analytic sweep's geometry and topology sound once the two serializer bugs were patched.
- The new engine follows sound principles: refine every trim curve onto the emitted geometry, treat each triple point as a single source of truth, keep an explicit tolerance ladder, and validate in-process.
- It records importer quirks that ISO 10303 does not, such as the toroidal-band ambiguity and SolidWorks dropping faces with POLYLINE edges.

**Why that case fails**
- **Every algorithm is textbook.** The pieces are:
  - Wang et al. 2008 double-reflection rotation-minimising frames.
  - Piegl & Tiller A8.1 (revolved NURBS) and A9.1 (cubic interpolation).
  - Predictor–corrector marching with cyclic projection.
  - Six-direction ray-parity point-in-solid.
  - A uv DCEL.

  None is implemented better than its source.
- **The implementations are scaffold-specific.** They rely on:
  - absolute millimetre constants (weld 0.02, sliver 0.05, seed gap 0.4, trace budget of 4096 × 0.01 mm);
  - feature-template FaceExtent variants;
  - name-based exclusion of solids called `electrode*`.
- **More silent failures than the evaluation listed.**
  - In the union, plane×torus intersection is off by default, and unsupported surface pairs return no curves.
  - In the cut, the legacy half-space cut does nothing on fused shells.
  - In the STEP writer, faces are dropped or written with empty bounds.
  - The fillet path can reverse on a short segment, giving a self-overlapping sweep with no error.
- **Weaker validation gates than claimed.**
  - The deviation gate checks a polyline only at its own defining points, which are on the surface by construction.
  - Torus and sphere faces skip the wire gate, and sphere faces also skip the deviation gate.
  - The tests deliberately avoid the equal-radius tangency case.
- **A new determinism hazard.** Environment variables switch geometric algorithms, so output depends on the process environment, and wasm has none.

**What to take from it**
The only unique value is an interop checklist of author-reported importer behaviour. That is a 10-minute read, not a method to port. The evaluation's what_to_take already covers it, apart from the missed toroidal-band item.

Corrected verdict: REJECT, with the optional interop-checklist reading.

## csgrs

**Repo url:** https://github.com/timschmidt/csgrs (crate: https://crates.io/crates/csgrs; pins used: main 1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586, release v0.20.1 f346c74974757aefdfc344313bbbc682ba711257)

**Licence:** csgrs is MIT: "Copyright (c) 2025 Timothy Schmidt" plus "Copyright (C) 2011 Evan Wallace". The second notice is inherited from csg.js (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/LICENSE). Workspace crates on main have their own licences. hypermesh is MIT, with the notice "Copyright (c) 2026 zalo" (crates/hypermesh/LICENSE, same SHA). hyperreal is Apache-2.0, with MIT notices kept for files derived from tialaramex/realistic (crates/hyperreal/LICENSE.txt, LICENSE-MIT, README L531-543). hyperlimit and hypertri are MIT OR Apache-2.0. hypercurve, hyperbrep, hyperbrep-io and hypersolve are Apache-2.0. hyperlattice is MIT (each crate's Cargo.toml). All of these are compatible with MPL-2.0. The core dependency graph is small and permissive: num/num-bigint for hyperreal (MIT/Apache); dxf, stl_io, gltf, image, ttf-parser and gerber_* for IO (MIT or MIT/Apache, checked on crates.io). One catch in the old release: v0.20.1's default features include image-io, which pulls in contour_tracing, and that crate is EUPL-1.2, a copyleft licence outside Spare Kit's allowed list (https://github.com/timschmidt/csgrs/blob/f346c74974757aefdfc344313bbbc682ba711257/Cargo.toml#L57 and #L84 and #L132-L136). Its hershey dependency is Apache-2.0 OR LGPL-3.0, which is fine if Apache is chosen.

**Activity:** Created 2025-01-05; last push 2026-10-20. About 11,266 commits, counted from the pagination link header. 261 stars, 34 forks, 5 open issues. 20 listed contributors, but timschmidt has 10,301 commits; the next are aljen (381) and tialaramex (213). Commit rate is about 40-45 a day (45 on 2026-10-19, 40 on 2026-10-20). The commits carry "Co-Authored-By: Claude Opus 5.5", so the code is written mostly by an AI agent working to AGENTS.md rules. The last crates.io release is 0.20.1 (2025-07-24), with 44,297 total downloads. Main is unreleased 0.23.0, built from about 20 path crates. hypermesh and hyperbrep are not on crates.io at all. hyperreal, hyperlimit, hypertri, hypercurve and hyperlattice last published 2026-06-08. APIs change often, and the READMEs announce "direct API migration with no compatibility adapter" (crates/hyperbrep/README.md, main SHA).

**Claims**

- **claim**: It is a lightweight, optionally multithreaded Constructive Solid Geometry kernel built natively in Rust; **verdict**: partly; **evidence**: Native Rust: true for both versions. Lightweight and optionally multithreaded: true only of the old release. v0.20.1's readme opens with 'A fast, optionally multithreaded' (https://github.com/timschmidt/csgrs/blob/f346c74974757aefdfc344313bbbc682ba711257/readme.md#L3) and has a rayon-backed `parallel` feature (Cargo.toml#L85-L93 at f346c749). Main has no rayon and no `parallel` feature in csgrs (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/Cargo.toml#L206-L207). Only hyperlimit has an optional `parallel` feature, and csgrs does not enable it. Main is not lightweight: about 3,500 files across roughly 20 crates, with exact bignum arithmetic throughout. It also no longer calls itself a kernel; it is 'the constructive-solid-geometry grammar' over Hypermesh/Hypercurve 'without introducing another geometry kernel' (readme.md#L1-L21 at main).
- **claim**: It is excellent for union, difference and intersection on solid geometry; **verdict**: false; **evidence**: The published 0.20.1 is the csg.js BSP algorithm in f64. union/difference/intersection are the clip_to/invert/build sequence (https://github.com/timschmidt/csgrs/blob/f346c74974757aefdfc344313bbbc682ba711257/src/mesh/mod.rs#L476-L589). They return a bare Mesh with no error path. The Shewchuk orient3d result is thresholded with EPSILON=1e-8, which removes its exactness (src/mesh/plane.rs#L257-L293; src/float_types.rs#L22-L23). Split vertices are rounded f64 interpolations, and fragments under 3 vertices are silently dropped (plane.rs#L396-L415). Issues show the consequences. #8: coplanar caps lost. #38 (open): T-junctions, so output is not manifold. #84 (open): extruded text not manifold. #21 and #110 (open): infinite BSP recursion and stack overflow; on #110 the maintainer replied 'This is likely an epsilon tolerance issue'. #107: wrong faces until the user raised EPSILON to 1e-6. The unit tests only check that output is non-empty and the bounding box is right; no manifold or watertightness assertion (src/tests.rs#L532-L597). Main's hypermesh is a different, serious design: exact arrangement plus a closure certificate (see architecture). But under its default STRICT policy, the repository's own boolean_operations example fails on a sampled sphere because coplanarity cannot be decided (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/scripts/run-examples.sh#L10-L13; crates/hypermesh/FUTURE_WORK.md#L3-L18). Booleans are on triangle meshes only.
- **claim**: It has powerful 2D-to-3D extrusion, handles polygons with holes; **verdict**: partly; **evidence**: Holes are supported in both versions. v0.20.1: extrude, extrude_vector, revolve, sweep and loft triangulate geo polygon interiors (https://github.com/timschmidt/csgrs/blob/f346c74974757aefdfc344313bbbc682ba711257/src/sketch/extrudes.rs#L87-L150). Its sweep places the profile perpendicular to each vertex's outgoing tangent, with no mitring and no self-intersection handling (extrudes.rs#L824-L1000). Main: try_extrude, extrude_vector, extrude_twisted, revolve, try_sweep and loft on CurveRegion2 with holes (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/readme.md#L341-L346). Curved boundaries are sampled into chords: the docs say 'its local certainty certifies decisions about the sampled mesh, not exact preservation of the curved' surface (readme.md#L275-L278). Loft makes no promise of 'an embedded surface' (readme.md#L385-L391). The `extrude` example is itself a tracked known failure, because sweep frame transport 'flattens an undecided rotation into InvalidArguments' (scripts/run-examples.sh#L10-L13). Output is a triangle mesh, not a B-rep with analytic faces, so 'powerful' is not supported.
- **claim**: It imports and exports STL, OBJ and STEP; **verdict**: partly; **evidence**: STL and OBJ: true in both versions (v0.20.1 src/io/mod.rs#L1-L17 at f346c749; main src/io/{stl,obj,ply,amf,gltf,vrml,dxf,svg,gerber}.rs). STEP: false for csgrs. v0.20.1 lists 'STEP/IGES import / export' under Todo (https://github.com/timschmidt/csgrs/blob/f346c74974757aefdfc344313bbbc682ba711257/readme.md#L551-L552), and main has no STEP module in src/io. STEP exists only as the separate, unpublished hyperbrep-io crate, described as 'bounded Part 21 syntax for its planned STEP exchange layer' (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/readme.md#L14-L16). It covers a limited analytic subset (prism, torus, cone, axial exchange tests in crates/hyperbrep-io/Cargo.toml).

**Architecture:** Two architectures share the name.

(A) The published v0.20.1 (f346c749). `Mesh<S>` is a Vec of `Polygon<S>`; each polygon has `Vertex{pos, normal}` and a `Plane` stored as three points (src/mesh/plane.rs#L91-L97). The BSP `Node{plane, front, back, polygons}` (src/mesh/bsp.rs#L14-L30) picks splitting planes heuristically from a sample of 20 polygons (bsp.rs#L77-L111). Booleans are csg.js clip_to/invert, after partitioning out faces outside the other operand's bounding box (src/mesh/mod.rs#L97-L105, #L476-L589). `Sketch<S>` wraps geo::GeometryCollection, using geo/i_overlay for 2D Booleans (src/sketch/mod.rs#L36-L38). There is no topology: polygon soup, no shared vertices, no half-edges. The manifold check quantises to 1e-6 and counts edges (src/mesh/manifold.rs#L25-L64). One Real type is either f32 or f64 (src/float_types.rs).

(B) Main (1bd74092). The workspace stacks as hyperreal (scalar) → hyperlattice (Point3/Vector3/matrix of Real) → hyperlimit (policy-aware predicates) → hypertri (exact CDT/earcut) → hypermesh (exact triangle-mesh Booleans) / hypercurve (exact 2D curves and regions) / hyperbrep (exact B-rep, early) → csgrs (modelling grammar and IO). Readme L1-L21 and the root Cargo.toml (#L118-L126) describe this.

hyperreal::Real is a rational scale times a symbolic or lazy computable-real expression (Boehm-style constructive reals, derived from tialaramex/realistic). Equality is structural; signs are refined on request (crates/hyperreal/README.md L5-L50, L531-L543).

hypermesh::boolean (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/src/boolean.rs#L21-L48) runs these steps:
1. Validate closed piecewise-winding-number (PWN) input.
2. Build an exact BVH and all pairwise face intersections.
3. corefine_surface: per-face exact constrained triangulation via hypertri.
4. assemble_surface_cells: a radial cell complex built from exact radial ordering around edges.
5. Seed absolute winding vectors with exact ray casts, then propagate them across facets (src/surface_arrangement.rs#L4023-L4049, #L2021-L2435).
6. Evaluate a Boolean truth DAG over the winding vectors. Multiple operands and multiple roots reuse one arrangement (src/winding.rs#L36-L105).
7. Certify the selected output (#L4336-L4456): no degenerate or duplicate triangles, every edge used equally in both directions, no boundary edges; otherwise a typed OpenOutput error.

This is the Mesh Arrangements method (Zhou, Grinspun, Zorin, Jacobson 2016), confirmed in crates/hypermesh/PERFORMANCE.md#L1202-L1222. Output vertices are explicit Point3<Real> in one shared arena. Coordinates are constructed, not plane-based or implicit.

hyperbrep is a typed immutable arena (vertices, edges, edge uses, wires, faces, shells, solids; Curve3, Pcurve, Surface). It has plane, sphere, cylinder, cone, torus and tensor NURBS carriers. Booleans so far are restricted to plane/cylinder prismatic and axial-shell domains; 'General surface arrangements ... remain open' (crates/hyperbrep/README.md#L1-L16, #L54-L55).

**Robustness:** v0.20.1 is not robust. It calls robust::orient3d (Shewchuk) and then compares the determinant with ±EPSILON (1e-8 in f64). That turns the exact predicate into a scale-dependent tolerance on a volume-like quantity (https://github.com/timschmidt/csgrs/blob/f346c74974757aefdfc344313bbbc682ba711257/src/mesh/plane.rs#L257-L293). Intersection points are f64 interpolations, re-tested against later planes. Spanning edges with |denominator| ≤ EPSILON are skipped, and fragments under 3 vertices are dropped silently (#L396-L415). `Plane::from_normal` panics on a degenerate normal (#L221-L225). T-junctions are never fixed (readme Todo #L507), and error handling is 'fix up ... eliminate panics' (#L527). Real failures:
- coplanar caps vanish (#8);
- infinite recursion or stack overflow building the BSP (#21; #110 still open);
- missing faces until EPSILON is hand-edited (#107);
- non-manifold output (#38, #84, both open).
No fuzzing or property tests; Boolean tests only check bounding boxes (src/tests.rs#L532-L597).

Main's hypermesh is exact by design. Its stated contract: no epsilon merge anywhere, so 'a point at 1e-12 outside a cube is outside' (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/properties.md#L85). Every predicate takes an explicit PredicatePolicy (crates/hyperlimit/src/predicate.rs#L333-L365):
- STRICT accepts only exact or certified answers. An undecided case becomes a typed error, never false.
- APPROXIMATE_512 ends with a deterministic interval at 2^-512 and calls the value zero if that interval contains 0 (crates/hyperlimit/src/resolve.rs#L254-L271). This is a tiny but real tolerance, and the result is labelled Approximate512Consumed.

Coplanar, coincident, tangent, nested, nonmanifold-but-balanced and winding-multiplicity inputs are handled from exact cell truth. Open, non-PWN and degenerate inputs are typed failures (crates/hypermesh/README.md#L167-L183). Output passes the closure certificate before release (surface_arrangement.rs#L4336-L4456). Tests: unit tests for coincident, tangent and nested shells and 40 operands (surface_arrangement.rs#L4907-L6916); integration tests (crates/hypermesh/tests/boolean.rs); 8 libFuzzer targets, including an exact-volume oracle for axis-aligned box Booleans (crates/hypermesh/fuzz/fuzz_targets/boolean_box_oracle.rs).

Known limits on main:
- Coordinates from transcendental sampling (sin/cos of most angles) make exact coplanarity undecidable. Under STRICT, Booleans and hulls on csgrs's own 32×16 sphere fail; the repo tracks boolean_operations, convex_hull and extrude as known example failures (scripts/run-examples.sh#L10-L13; crates/hypermesh/FUTURE_WORK.md#L3-L18).
- Coordinates are explicit rationals or expressions, so bit length grows across cascaded operations. The 11,894×11,894-triangle rotated-copy Boolean 'previously reached about 116 GiB RSS' and is kept as an ignored test (crates/hypermesh/PERFORMANCE.md#L1197-L1200).
- Output allows balanced non-manifold edges (README#L183). Closed-PWN validation of loft/sweep output does not mean the result is embedded (readme.md#L385-L391).
- properties.md says outright that its requirements 'are not a claim that the implementation already satisfies them' (#L3).
- Correctness bugs keep surfacing elsewhere in the stack. Commit 170648c6 (2026-10-20) fixed hypersolve returning 'a Decided representation of a root that is not there', which breaks its own P11 rule ('never wrong, only unsure').

**Determinism:** v0.20.1 is not built for bit-identical results across targets. It uses f64 with std trig in primitives (platform libm on native, a different libm on wasm), tolerance tests on EPSILON, and an optional rayon path (src/mesh/bsp_parallel.rs).

Main is deterministic by design but not shown to be bit-identical across targets:
- Decisions use exact integer bignum arithmetic (num-bigint) or certified rational intervals. APPROXIMATE_512 is also integer-interval based (hyperlimit/src/resolve.rs#L254-L271).
- The f64 cache in hyperreal only serves lossy export views (crates/hyperreal/src/real/convert.rs#L278-L375).
- Internal hash maps use a fixed-seed hasher, StorageIdentityHasher (crates/hypermesh/src/storage_hash.rs#L1-L53). It mixes with from_ne_bytes, which is endian-dependent but the same on x86, ARM and wasm32.
- No threads in the default csgrs graph.
- properties.md P27 requires repeated serialized runs to give identical geometry and checksums (#L95), and there are determinism unit tests (surface_arrangement.rs#L4934).

Unverified: I found no test comparing native and wasm32 output bit for bit. tests/wasm_certainty.rs and tests/wasm/*.ts only check that certainty receipts survive the JS boundary. I also did not audit whether any output order depends on hash-map iteration order.

**Strengths**

- Main's hypermesh is a real exact Mesh-Arrangements Boolean: corefinement, radial cell complex, absolute winding vectors, a truth DAG, and an output closure certificate. Undecided cases return typed errors, never silent wrong output (crates/hypermesh/src/surface_arrangement.rs#L4023-L4456 at 1bd74092).
- Multi-operand, multi-root Boolean programs (Not/And/Or/Xor DAG) are answered from one shared arrangement. Results that contain the exterior are reported as UnboundedBooleanOutput instead of being forced into a finite mesh (crates/hypermesh/src/winding.rs#L36-L105; README#L146-L166).
- Explicit predicate policy plus a certainty receipt (STRICT vs APPROXIMATE_512 → Certified vs Approximate512Consumed) carried through the API, FFI and WASM. This is a clean way to make approximation visible (crates/hyperlimit/src/predicate.rs#L333-L365).
- properties.md is a well-written acceptance contract that separates numerical certainty, topological validity and geometric fidelity, under the ordering exactness > completeness > performance (properties.md#L7-L24). Philosophically it lines up closely with Spare Kit.
- Oracle-based fuzzing: axis-aligned box Booleans are checked against an independently computed exact volume (crates/hypermesh/fuzz/fuzz_targets/boolean_box_oracle.rs). There is also differential benchmarking against CGAL EPECK, OpenCascade, manifold-rust and boolmesh.
- Licences across the core stack are permissive (MIT / Apache-2.0) and dependencies are small (num, serde optional).

**Weaknesses**

- The released crate (0.20.1, the only csgrs on crates.io) is csg.js BSP in f64 with a 1e-8 epsilon threshold on orient3d. It produces non-manifold output, drops coplanar faces, recurses until the stack overflows, and returns no errors. Issues #38, #84 and #110 are still open. It fails every iron-clad requirement.
- Main's Booleans work only on triangle meshes. There are no exact Booleans on analytic or NURBS surfaces; hyperbrep covers only plane/cylinder prismatic and axial-shell domains, and 'General surface arrangements ... remain open' (crates/hyperbrep/README.md#L54-L55). That is far short of the B-rep Spare Kit needs.
- Using computable reals as the coordinate type makes exact zero tests undecidable for trig-derived geometry. Under STRICT, Booleans fail on csgrs's own sampled sphere and the extrude example fails (scripts/run-examples.sh#L10-L13). The 'never fail' half of the iron-clad standard is not met; the alternative policy is a 2^-512 tolerance.
- Constructed vertices are stored as explicit exact coordinates, not in a plane-based or indirect form, so bit length grows across cascaded operations. The 11,894-triangle rotated-copy Boolean reached about 116 GiB RSS and is excluded from routine gates (crates/hypermesh/PERFORMANCE.md#L1197-L1200).
- Unreleased and unstable: about 40 AI-agent commits a day, breaking API migrations with no adapters, hypermesh and hyperbrep never published. Correctness bugs keep appearing; for example, hypersolve declared a root that does not exist, fixed 2026-10-20 in 170648c6. The project's own properties.md says its properties are aspirations, not met guarantees.
- Essentially one human owner: 10,301 of about 11,300 commits are by timschmidt. Docs are very large and dense (READMEs of 300-850 lines of contract prose), so the 'readable by people' goal is only partly met.
- v0.20.1's default feature set pulls in contour_tracing, which is EUPL-1.2 copyleft and not on Spare Kit's allowed list.

**Verdict:** STUDY

**Verdict reason:** REJECT anything from the published csgrs 0.20.1. Its method is the csg.js BSP with float epsilons, the architecture Spare Kit is explicitly trying to get away from: no exact topology, silent loss of faces, non-manifold output, stack overflows.

Main is a different story: an exact Mesh-Arrangements Boolean with policy-explicit certified predicates and output closure certificates. Its stated goals match Spare Kit's closely. It still cannot be a dependency, or a whole-system port, under the 'absolute best architecture' rule, for four reasons:
1. Booleans are on triangle meshes only. Analytic and NURBS B-rep Booleans barely exist in hyperbrep.
2. The coordinate model (Boehm-style computable reals) makes exact coplanarity undecidable for ordinary trig-derived geometry, so strict Booleans fail on the repo's own sphere example. The best architecture keeps topology decisions in rational or algebraic numbers of bounded degree, with combinatorial certificates from the generators.
3. Explicit constructed coordinates lead to unbounded bit growth (116 GiB on one 11.9k-triangle case). A plane-based or indirect-predicate representation avoids this.
4. The code is unreleased, agent-generated at high churn, maintained by one person, and still finding correctness bugs.

What is worth learning from are its arrangement pipeline, truth-DAG multi-root Booleans, certainty receipts, closure certification and oracle-based fuzzing. Spare Kit should re-derive those inside its own B-rep-first, bounded-precision design.

**What to take**

- Output closure certificate: reject degenerate or duplicate triangles, require every edge to be used equally in both directions, allow no boundary edges, and return a typed error before publishing anything. Small, generic and worth reproducing as Spare Kit's last gate after a Boolean: https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/src/surface_arrangement.rs#L4336-L4456
- Multi-operand Boolean truth DAG over winding-number vectors. Many result roots come from one arrangement, and unbounded (exterior-containing) results are rejected explicitly: https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/src/winding.rs#L36-L150 and README https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/README.md#L146-L183
- Pipeline shape for exact mesh Booleans (Mesh Arrangements): exact BVH → pairwise intersections → per-face exact CDT corefinement → radial cell assembly → ray-seeded winding propagation. Study it as the reference design for Spare Kit's mesh layer, while replacing explicit constructed coordinates with plane-based or indirect points: https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/src/surface_arrangement.rs#L4023-L4049 (with corefine_surface at #L2473 and assemble_surface_cells at #L578)
- Policy plus certainty-receipt API pattern: STRICT returns Unknown, which becomes a typed error; an approximate terminal is always labelled. Useful for Spare Kit's predicate layer API: https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hyperlimit/src/predicate.rs#L333-L365 and https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hyperlimit/src/resolve.rs#L254-L271
- Acceptance-contract structure: three independent claims (numerical certainty, topological validity, geometric fidelity), an exactness-first ordering, and closure properties P17-P20 and P24. A good template for Spare Kit's G01 property spec: https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/properties.md#L7-L95
- Exact-oracle fuzzing of Booleans: random axis-aligned boxes checked against an independently computed exact volume: https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/fuzz/fuzz_targets/boolean_box_oracle.rs
- A design lesson, recorded by the project itself: coordinates from sin/cos make exact coplanarity undecidable. Generators should emit rational parametrisations or carry combinatorial coplanarity certificates, never trig-sampled coordinates, into topology: https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/FUTURE_WORK.md#L3-L18
- An anti-pattern to record for Spare Kit: the v0.20.1 code that thresholds an exact orient3d with an epsilon and then splits with rounded f64 interpolation: https://github.com/timschmidt/csgrs/blob/f346c74974757aefdfc344313bbbc682ba711257/src/mesh/plane.rs#L257-L293 and #L396-L415

### Challenge 1: FACTS: I re-checked the licences, activity numbers, all four

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- The evaluation understates how far hyperbrep's Booleans go, and this is the largest factual error. It says Booleans are 'restricted to plane/cylinder prismatic and axial-shell domains' and that 'There are no exact Booleans on analytic or NURBS surfaces'. The hyperbrep README lists exact B-rep Booleans for many specific carrier pairings: parallel cylinders, coincident cone frustums, identical tori, coaxial torus with sphere, cylinder or cone, torus/slab and plane/torus cuts, sphere/sphere (equal, contained and partially overlapping), sphere/slab, coaxial sphere/cylinder (all Boolean ops), coaxial full-turn revolutions including NURBS meridians, and coplanar region transfer onto NURBS tensor patches (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hyperbrep/README.md#L370-L470). The code exists, for example crates/hyperbrep/src/topology/validation/sphere.rs (993 lines; certified_sphere_cylinder_union_shell at L678). The accurate statement: exact analytic and NURBS-carrier Booleans exist for a list of special cases, mostly aligned, coaxial or coincident, but there is no general surface-surface arrangement ('General surface arrangements ... remain open', README#L54-L55). The verdict does not change.
- The 116 GiB figure is pinned on a cause the source never states. The evaluation says 'Explicit constructed coordinates lead to unbounded bit growth (116 GiB on one 11.9k-triangle case)'. The source is a historical section, '2026-07-28: original full-resolution mesh measurements' (PERFORMANCE.md#L1182). It says only that 'the corresponding CSGRS workload previously reached about 116 GiB RSS' and gives no cause (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/PERFORMANCE.md#L1197-L1200). Neither the hypermesh README nor PERFORMANCE.md discusses bit-length or coefficient growth. Bit growth from explicit rational coordinates is a reasonable theoretical worry, but here it is the evaluator's inference. 'Previously' also means the figure may not describe current hypermesh.
- The hyperreal licence history is wrong. The evaluation says hyperreal keeps 'MIT notices ... for files derived from tialaramex/realistic'. In fact crates/hyperreal/LICENSE-MIT reads 'Copyright (c) 2026 Timothy Schmidt' (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hyperreal/LICENSE-MIT#L1-L3), and upstream realistic is Apache-2.0 on crates.io (realistic 0.8.2: 'Apache-2.0'). The MIT file is the maintainer's own notice, not one kept from realistic. Compatibility with MPL-2.0 is unaffected.
- Activity: GitHub lists 23 contributors (25 with anonymous ones), not 20 (gh api repos/timschmidt/csgrs/contributors?per_page=100 gives length 23). The commit count (11,266), the top-three commit counts, the 45 and 40 commits/day on 10-19 and 10-20, stars, forks, issue counts, dates and crates.io publish dates all checked out.
- Main's loft does not take 'CurveRegion2 with holes'. Its signature is `solid::loft(sections: &[Vec<Point3>])`, and the docs say 'Loft sections are corresponding simple planar loops' (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/readme.md#L346 and #L385-L386). The vector-extrude function is named `try_extrude_vector` (L341).
- The evaluation's quote drops a word that narrows its meaning. The source sentence is 'its local certainty certifies decisions about the sampled mesh, not exact preservation of the curved font surface' (readme.md#L276-L277, inside the text/font section); the evaluation renders it as 'the curved' surface', which makes it read as general. The general point that curved boundaries are sampled is still supported by L275: 'try_extrude retains its documented finite-profile path for curved boundaries'.
- Wrong citation: main Cargo.toml #L118-L126 is the workspace `exclude` list, not a description of the stack. The path-crate dependency stack is at https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/Cargo.toml#L152-L159. Relatedly, 'built from about 20 path crates' is loose: csgrs depends directly on 8 path crates (plus hypersolve via [patch] at L199-L204); the workspace has 22 crate directories.
- Issue states are slightly off. Claim 2's evidence says '#21 and #110 (open)', but #21 was closed on 2025-04-24; only #38, #84 and #110 are open. #8 was closed the day it was filed (2025-02-05). #107 (2025-11-11) and #110 (2025-12-09) were filed after the 0.20.1 release, against the BSP code on main at that time, which uses the same EPSILON method. That does support the method critique, though strictly they were not reports against 0.20.1 itself.
- The determinism reasoning is incomplete, though I found no defect. The evaluation notes that StorageIdentityHasher hashes identically on x86, ARM and wasm32. Equal hash values do not give equal HashMap iteration order across targets: hashbrown builds its control tag from the top 7 bits of a usize-width hash and picks a different SIMD group width per target (SSE2, NEON or generic) (https://github.com/rust-lang/hashbrown/blob/f7c44c608517ec79e0134d5747458aeb0aa8b86e/src/control/tag.rs#L35-L43; src/control/group/mod.rs#L8-L51). I spot-checked the hypermesh uses. The output certificate (surface_arrangement.rs#L4360-L4456) and the mesh.rs#L350/L433/L1677-L1691 checks only look up keys, count, or apply order-independent all(). No defect found, but cross-target bit-identity stays unverified, as the evaluation itself says.
- What the evaluation missed: what_to_take lists only mesh-layer ideas and leaves out hyperbrep's construction contract. ModelBuilder staging means 'invalid references, disconnected wires, mismatched edge endpoints, and unsupported edge/pcurve/surface agreement cannot be published as a Model'. References are revision-scoped and builders reject foreign keys (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hyperbrep/README.md#L24-L36). There is also `validate_edge_image`, which checks exact pcurve/edge image agreement (L38-L43). These line up directly with Spare Kit's B-rep-first 'never returns an invalid solid' requirement and should be on the study list.

**Argument:** Neither the overall verdict nor any of the four claim verdicts is wrong, but several factual details are.

What checks out:
- MIT licence with the Evan Wallace notice.
- The EUPL-1.2 contour_tracing that v0.20.1's default image-io feature pulls in (Cargo.toml L57, L84, L132-136; crates.io confirms EUPL-1.2), and hershey as Apache-2.0 OR LGPL-3.0.
- Every workspace crate licence (each crate's Cargo.toml).
- hypermesh and hyperbrep are not on crates.io.
- Commit totals and daily rate, and Claude co-author trailers on the 30 most recent commits.
- The v0.20.1 code: orient3d thresholded at ±EPSILON (1e-8) at plane.rs L286-291, the panic at L223-224, the denom>EPSILON skip and dropping of fragments under 3 vertices at L396-415, the BSP picking split planes from a 20-polygon sample, and Boolean tests that only check bounding boxes. tests.rs has no is_manifold assertion.
- The quotes from issues #107 and #110.
- The hypermesh pipeline: build_surface_arrangement L4023-4049; ray-seeded winding at L2182-2435; the closure certificate L4336-4456, where has_no_boundary also requires zero unbalanced, degenerate and duplicate triangles.
- The truth DAG in winding.rs, the STRICT and APPROXIMATE_512 semantics (predicate.rs L333-365, resolve.rs L254-271), 8 fuzz targets including the exact box-volume oracle, the 40-operand test, properties.md L3/L85/L95, FUTURE_WORK.md, the run-examples known failures, no rayon in csgrs's default graph, and the hypersolve bug fixed in 170648c6.

So claim 1 'partly', claim 2 'false' (for the published crate), claim 3 'partly' and claim 4 'partly' (STEP only in the unpublished hyperbrep-io) all stand.

The errors are in the description of method and in the support for verdict reasons 1 and 3:
- hyperbrep is materially more capable than stated. It has exact B-rep Booleans for a catalogue of special-case analytic pairings (spheres, cones, tori, coaxial sphere/cylinder, revolutions with NURBS meridians) but no general surface-surface intersection.
- The 116 GiB figure comes from a historical note with no cause given. 'Unbounded bit growth' is the evaluator's inference, presented as documented fact.
- Smaller errors: the hyperreal MIT notice, the contributor count, loft's input type, a truncated quote, a wrong Cargo.toml citation, and #21 labelled open.

Neither correction moves the verdict. Spare Kit needs general, never-failing analytic/NURBS B-rep Booleans. hyperbrep still answers only enumerated aligned or coaxial cases and returns typed failures elsewhere, and the mesh layer still fails STRICT on trig-sampled input. The code remains unreleased, churns at about 40 agent commits a day, and has essentially one maintainer.

REJECT would be too harsh: hypermesh's certified arrangement and closure certificate, plus hyperbrep's publish-only-valid ModelBuilder contract, are high-value designs to re-derive. PORT or DEPEND would be too generous, given the instability and the trig-coplanarity limitation. STUDY is correct. The study list should add hyperbrep's construction and validation contract, not only mesh-layer ideas.

### Challenge 2: FIT: the verdict was STUDY, so argue toward PORT by looking 

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- The 116 GiB case is wrongly used as evidence of bit growth across cascaded operations. weaknesses #4 and verdict_reason #3 present it as proof that explicit constructed coordinates grow without bound. In fact the case is one intersection run under APPROXIMATE_512, and its expected result is certified empty. The second input is the YeahRight mesh mapped by [z+1, y+12, 1-x]: an exact axis permutation plus an integer shift, so inputs are dyadic and there is no trig. The repo's text says only that the 'earlier CSGRS workload' reached 116 GiB and gives no cause. Bit growth across cascades is still a plausible risk, but it is unverified here. https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/competitive/support.rs#L608-L623 ; https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/tests/competitive.rs#L978-L993 ; https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/PERFORMANCE.md#L730-L733 ; https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/PERFORMANCE.md#L1197-L1200
- The undecidability objection (verdict_reason #2) blames the kernel's coordinate model when the cause is the generator. For rational-coordinate meshes, which includes every STL/OBJ import and every f64 input (hypertri lifts each float to its exact value), all hypermesh STRICT predicates are rational sign tests and always decidable. The failures come from csgrs's trig-sampled 32x16 sphere. FUTURE_WORK shows spheres with coordinates in Q(sqrt2, sqrt3) already decide: sphere(8,24,12) gives a certified hull. hyperreal already has the rational and quadratic-surd structural tier the evaluation recommends; Boehm-style refinement is only the fallback. This is a generator-design lesson (already what_to_take #7), not proof that the kernel architecture is unsound. https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/FUTURE_WORK.md#L5-L11 ; https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypertri/README.md#L41-L42 ; https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hyperreal/README.md#L142-L147
- The weakness 'the never-fail half of the iron-clad standard is not met' misquotes Spare Kit. Its rule is 'booleans never silently fail', and STRICT returns typed errors, which meets that wording (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/src/boolean.rs#L21-L48 ; https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hyperlimit/src/predicate.rs#L341-L365). The real gap is completeness and reliability on ordinary input, and the evaluation should say that instead.
- Missed hypercurve, the component closest to Spare Kit's 'AutoCAD-class 2D drafting with exact arcs'. It is an exact planar kernel for lines, circular arcs, Beziers, B-splines and NURBS, with regularized Booleans, region offset, stroke, fillet, chamfer, straight skeleton and an exact SVG round trip (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypercurve/README.md#L1-L18 , #L383-L439 , #L482-L490). Against porting it: it still has typed blockers for retraced components and for generated circle/parallel pairs (#L370-L372). It is also 476k lines, and its tests were skipped in CI on main because Clippy failed first (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/properties.md#L182-L190 , #L208-L215). So it belongs under STUDY / what_to_take, not PORT.
- Missed hyperbrep's own 2026-10-20 review, which is much harsher than the evaluation's summary. It says nothing in the kernel is yet general. About 12 shape-family recognizers are the only admission path. Surface-surface intersection is a pairwise match table, and two cylinders at any nonzero angle return UnsupportedIntersection, so drilling a hole perpendicular into a cylinder fails. About 20 sites set certificate flags to true and later code trusts them without rechecking, which is a direct risk of publishing invalid solids. csgrs does not even depend on hyperbrep. https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hyperbrep/docs/DEVELOPMENT_PLAN.md#L2529-L2585 ; https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/properties.md#L216-L219
- Scale and readability are understated. The evaluation says 'about 3,500 files' (that count is correct: 3,499 blobs), but properties.md records about 1.43 million lines of first-party Rust across 22 crates and 1,367 commits in one month. It also records Point2, Aabb, Sign and Classification each defined in several crates, 110 *_with_context twin functions, and hypersolve at 77.52% line coverage against its 85% gate. https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/properties.md#L182-L215
- Benchmark results that cut both ways are left out. In favour (self-reported): hypermesh won 46/54 small portable workloads against CGAL EPECK. Against: it lost the 4,512-triangle YeahRight Booleans to the f64 kernels boolmesh and manifold-rust, and of the 12 small Boolean cases it won only the three disjoint-box operations (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/PERFORMANCE.md#L712-L725). On a 1,140-facet convex-hull/box case it took 930.7 ms against CGAL EPECK's 9.0 ms, about 100x slower (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/PERFORMANCE.md#L1133-L1136). Exact, but not best-in-class at scale.
- Licence provenance missed. hypermesh started as a fork of komietty/boolmesh (MPL-2.0): both repos share init commit 2ecdb0d4 (https://github.com/komietty/boolmesh/commit/2ecdb0d479ba649b3b5aad0ece5cc574cbad2079 ; https://github.com/timschmidt/hypermesh/commit/2ecdb0d479ba649b3b5aad0ece5cc574cbad2079). It was relicensed MPL-2.0 to MIT in rewrite commit b8a60e54 (2026-07-02), which deleted almost all prior src (https://github.com/timschmidt/hypermesh/commit/b8a60e5470b1f5fad6a75902636ceb2d55a4168c). The new notice, 'Copyright (c) 2026 zalo', names someone who appears nowhere among the repo's contributors (timschmidt 3,962 commits, komietty 158). Either licence is compatible with MPL-2.0, but provenance should be confirmed with the author before reuse. The same history shows the pipeline rewritten at least twice since May 2026: boolmesh fork, then EMBER local BSP, then surface arrangement. Dev-only test oracles also include rug, which is LGPL-3.0+ (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hyperreal/Cargo.toml#L36 ; crates/hyperlattice/Cargo.toml#L21), and symbolica, which has a non-standard proprietary licence (crates/hyperlattice/Cargo.toml#L23). These are not in the release graph but would block porting the test suites as they stand.
- Nuance: the claim 'coordinates are constructed, not plane-based' is true of stored coordinates but incomplete. Inside one operation, each constructed vertex carries a plane-based combinatorial identity (SourceEdgePlane or PlaneTriple), which the arena uses to deduplicate before falling back to numeric interning (https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/src/polygon.rs#L35-L48 ; https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/src/surface_arrangement.rs#L103-L150). The identities are dropped at output, so cascaded operations re-materialize coordinates. This hybrid is worth recording in what_to_take.
- Minor mis-pin: the evaluation cites root Cargo.toml #L118-L126 for the crate stack, but those lines are the workspace exclude list. The path dependencies that define the stack are at https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/Cargo.toml#L152-L159

**Argument:** Verdict: STUDY stands. Nothing in csgrs is both best-in-class and portable, but two of the evaluation's four architecture objections are weaker than it claims, and it misses the stack's strongest candidate, hypercurve.

What checked out: almost every pinned claim. That covers the per-crate licences and that hypermesh, hyperbrep and hyperbrep-io are unpublished. The activity figures match (11,266 commits, 3,499 files, timschmidt 10,301). The v0.20.1 code really does put a 1e-8 threshold on orient3d, silently drop fragments and panic in from_normal. The EUPL-1.2 contour_tracing dependency is real. Issues #8, #21, #38, #84, #107 and #110, the three STRICT known-failure examples, the closure certificate, the truth DAG, the policy/receipt code and commit 170648c6 all say what the evaluation says.

The best case for PORT:
1. hypermesh is a real exact Mesh-Arrangements Boolean with a closure certificate. Self-reported, it beats CGAL EPECK on 46/54 small workloads. On rational input, which covers every mesh import, its STRICT mode decides every case. The "undecidable" failures come from csgrs's trig-sampled sphere generator, not the kernel.
2. The 116 GiB figure the evaluation uses as proof of cascaded bit growth is one Boolean on dyadic inputs whose result is empty, and the repo gives no cause.
3. hypercurve, which the evaluation ignored, is an exact planar kernel for lines, arcs, Beziers and NURBS with region Booleans, offsets, fillets and chamfers. That is exactly the "AutoCAD-class drafting with exact arcs" Spare Kit needs.

Why PORT still fails:
- **Size and review:** the project's own 2026-10-20 review records about 1.43M lines of Rust across 22 crates (hypercurve alone 476k) and 1,367 commits in a month. It also lists duplicated core types, hypercurve tests not running in CI on main, and hypersolve below its coverage gate.
- **hypermesh at scale:** it loses 4.5k-triangle Booleans to the f64 kernels boolmesh and manifold-rust. It is about 100x slower than CGAL EPECK on a 1,140-facet hull/box case. Its pipeline has been rewritten at least twice since May 2026, and its licence history needs confirming: it was forked from MPL-2.0 boolmesh and relicensed MIT under an odd "zalo" notice.
- **hyperbrep:** its own review says nothing in it is general. It only accepts shapes matching about 12 recognizers, a perpendicular hole drilled into a cylinder fails, and about 20 places mark solids as checked without actually checking them.

Recommended correction: keep STUDY. Add to what_to_take:
- hypercurve's exact line/arc region arrangement and offset design;
- hypermesh's plane-triple vertex identities;
- the self-review targets in properties.md, read as a list of failure modes to avoid.

Drop the bit-growth claim, or mark it unverified. Restate the trig objection as a generator lesson rather than a kernel defect. The overall verdict is not refuted; the errors are in supporting reasoning and omissions, and the omissions mostly make the case for STUDY stronger.

## GeoRust geo and geo-types

**Repo url:** https://github.com/georust/geo

**Licence:** MIT OR Apache-2.0 (dual). The repo root holds LICENSE-MIT and LICENSE-APACHE (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/LICENSE-MIT). The Cargo manifests declare `license = "MIT OR Apache-2.0"` in geo/Cargo.toml#L5 (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/Cargo.toml#L5) and geo-types/Cargo.toml#L4. The GitHub API reports "NOASSERTION" only because there are two licence files. Licences of the dependencies, per crates.io: i_overlay MIT OR Apache-2.0, i_float MIT, robust MIT OR Apache-2.0, spade MIT OR Apache-2.0, rstar MIT OR Apache-2.0, earcut MIT OR Apache-2.0, geographiclib-rs MIT, float_next_after MIT. proj is optional and binds the C PROJ library (MIT). All of these fit Spare Kit's licence policy. The jts-test-runner/resources/testxml files are copied from JTS (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/jts-test-runner/README.md). JTS is EPL-2.0/EDL-1.0, and I did not check which licence covers those XML files (unverified).

**Activity:** Created 2015-01-16. pushed_at 2026-10-18. Latest commit on main is c12769f (2026-09-19, "Make the point-to-segment distance scale-invariant (#1613)"). 1,951 stars, 272 forks, about 140 contributors (anonymous included). 105 open issues and 30 open PRs. Last release: geo 0.33.1 on 2026-04-20 and geo-types 0.7.20 on 2026-08-02. Release request #1618 is still open. Downloads: geo 24.4M, geo-types 29.7M. Maintenance is active and the project is the most-used Rust GIS vector-geometry crate. Its boolean engine lags upstream: geo pins i_overlay ">=4.5.1, <4.6.0" (geo/Cargo.toml#L43) while i_overlay is at 9.1.0 (2026-10-14). A RelateNG port is in progress in open PRs #1590 and #1591.

**Claims**

- **claim**: It is the definitive 2D geometry kernel for the Rust ecosystem; **verdict**: partly; **evidence**: It is the dominant Rust GIS geometry crate (24.4M/29.7M downloads, 1,951 stars), but it does not describe itself as a geometry kernel. geo/Cargo.toml says `description = "Geospatial primitives and algorithms"` with keywords gis/geo/geography/geospatial (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/Cargo.toml#L1-L13), and the README is titled 'Geospatial Primitives, Algorithms, and Utilities'. It is not a kernel in the CAD sense. The data model is 2D straight-segment geometry only, with no Z (Coord has just x and y: https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo-types/src/geometry/coord.rs#L23-L31). It has no curves, no topology structure and no validity-by-construction. Its hardest algorithm, booleans, is not its own code: it hands off entirely to the third-party i_overlay crate (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/bool_ops/mod.rs#L91-L107). georust.org itself also lists GEOS (LGPL C++) bindings alongside geo. For curves, other Rust crates are the reference points: kurbo for Béziers and cavalier_contours for bulge/arc polylines.
- **claim**: It provides points, polylines and polygons with holes and optimised algorithms for distance, convex hulls, intersections and area; **verdict**: true; **evidence**: True as a list of features, but correctness gaps are real. The Geometry enum has Point, Line, LineString, Polygon, Multi*, GeometryCollection, Rect and Triangle (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo-types/src/geometry/mod.rs#L50-L61). Polygon is an exterior plus Vec of interior LineStrings (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo-types/src/geometry/polygon.rs#L64-L69). geo/src/algorithm has area.rs, convex_hull/ (quickhull and graham), euclidean_distance, line_intersection.rs, a simplified Bentley-Ottmann sweep (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/sweep/mod.rs#L1-L67) and DE-9IM relate. 'Optimised' is only partly true. The sweep checks all x-overlapping pairs, and its own comment calls it 'more susceptible to pathological data'. Quickhull picks the furthest point with a plain f64 dot product and `partial_cmp().unwrap()` (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/convex_hull/qhull.rs#L137-L158). Issue #1566 (open) shows ConvexHull returning a self-intersecting, non-convex ring. Open panic issues: distance #1596 and #1604, interior_point #1578, voronoi #1558, contains overflow #921, integer Line::delta overflow #1001, convex hull #531.
- **claim**: (Owner question) It has exact arcs, circles, ellipses, splines or bulge polylines suitable for CAD drafting; **verdict**: false; **evidence**: There is no curve type of any kind. The Geometry enum is straight segments only (geo-types/src/geometry/mod.rs#L50-L61). Code search of the repo for ellipse, bezier, bulge, spline and CircularString returns 0 hits. 'Circles' exist only as polygonal approximations: Buffer's LineCap::Round samples cos/sin into N vertices (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/buffer.rs#L284-L300), and the doc example only checks that the area is close to πr² with epsilon 2e-1 (buffer.rs#L83-L87).
- **claim**: (Owner question) Its predicates use the robust crate; **verdict**: partly; **evidence**: Only orient2d is exact. GeoNum binds floats to RobustKernel and integers to SimpleKernel (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/lib.rs#L395-L430). RobustKernel casts to f64 and calls robust::orient2d, which is Shewchuk's adaptive predicate (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/kernels/robust.rs#L14-L43). The Kernel trait exposes only orient2d, square distance and dot_product_sign. The trait default and SimpleKernel use naive arithmetic that can overflow on integers (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/kernels/mod.rs#L27-L57; issue #1001 open). There is no exact incircle in geo; Delaunay work goes through the spade crate. Constructions are plain f64: line_intersection ports JTS RobustLineIntersector (exact classification, then a conditioned float intersection) and falls back to a nearest-endpoint heuristic when ill-conditioned (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/line_intersection.rs#L75-L95, #L200-L230, #L297-L313).
- **claim**: (Owner question) Its boolean engine is i_overlay; **verdict**: true; **evidence**: BooleanOps converts rings to i_overlay paths and calls `FloatOverlay::with_subj_and_clip_custom(..., OverlayOptions::ogc(), ...).overlay(op, fill_rule)` (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/bool_ops/mod.rs#L91-L107; adapter at https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/bool_ops/i_overlay_integration.rs#L1-L100). Buffer and offset also delegate to i_overlay (buffer.rs#L4-L22). The pinned i_overlay 4.5.1 builds a FloatPointAdapter (https://github.com/iShape-Rust/iOverlay/blob/2d3bedad29a08436fc75a8e92e6c5ff37b45e7b5/iOverlay/src/float/overlay.rs#L140-L160) on i_float 1.16. That adapter scales the bounding box by 2^(29 - floor(log2(half-extent))) onto an i32 grid (https://github.com/iShape-Rust/iFloat/blob/4cd5702a0f2596c10a0716c64437ff9cbb0696eb/src/adapter.rs#L16-L50; IntPoint is i32: https://github.com/iShape-Rust/iFloat/blob/4cd5702a0f2596c10a0716c64437ff9cbb0696eb/src/int/point.rs#L7-L9). Booleans are therefore integer snap-rounding: robust, but every output vertex is rounded. Issue #1493 shows that s.union(&s) changes the area of a 100-gon. Maintainers confirmed it is expected float/grid behaviour and advised pre-snapping.
- **claim**: (Owner question) Its data model is suitable for an AutoCAD-class 2D kernel; **verdict**: false; **evidence**: Coord<T> is a plain x/y pair, generic over CoordNum (f64 default, also f32 and integers) (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo-types/src/lib.rs#L103-L121). LineString is a newtype over Vec<Coord> (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo-types/src/geometry/line_string.rs#L135). Polygon guarantees only that rings are closed and explicitly 'does not enforce validity at this time' (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo-types/src/geometry/polygon.rs#L20-L69). The model also lacks: curve segments, per-vertex bulge, shared-edge topology (half-edge/DCEL), persistent IDs, and any tolerance or precision model attached to geometry. This is the OGC Simple Features model, built for GIS polygons.

**Architecture:** The workspace (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/Cargo.toml) has six members: geo-types, geo-traits, geo, jts-test-runner, geo-test-fixtures and geo-benches.

- **geo-types** holds the OGC Simple Features value types. Coord{x,y} is in geometry/coord.rs#L23-L31. Line{start,end} is in line.rs#L12-L14. LineString(pub Vec<Coord>) is in line_string.rs#L135. Polygon{exterior, interiors} is in polygon.rs#L66-L69. There are Multi* variants, plus Rect, Triangle, GeometryCollection, and the Geometry enum in geometry/mod.rs#L50-L61. Coordinates are generic over CoordNum and CoordFloat (geo-types/src/lib.rs#L103-L121).
- **geo-traits** gives zero-copy access traits over foreign geometry.
- **geo** is a flat collection of about 90 algorithm modules under geo/src/algorithm (https://github.com/georust/geo/tree/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm). Each algorithm is an extension trait implemented per geometry type, for example Area, ConvexHull, BooleanOps, Relate, Buffer and Validation.
- **Predicates** live in algorithm/kernels: a Kernel trait selected through GeoNum::Ker (geo/src/lib.rs#L395-L430).
- **Topology** is not stored. It is rebuilt per call: relate/geomgraph is a port of JTS GeometryGraph for DE-9IM, and PreparedGeometry caches it.
- **Booleans and buffer/offset** call out to i_overlay (bool_ops/mod.rs#L91-L107, buffer.rs#L4-L22).
- **Triangulation** uses earcut and spade (CDT), the latter also used for MakeValid repair.
- **Spatial indexing** uses rstar.
- **Large GIS surface**: geodesic, haversine, rhumb and vincenty measures, plus optional proj.

The library is purely 2D with no Z, and geometry is plain Vecs of points: no curves, no B-rep/DCEL and no IDs. Validity is checked on demand by the Validation trait, with an InvalidPolygon error taxonomy (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/validation/polygon.rs#L20-L49). It is not enforced by construction.

**Robustness:** Arithmetic is f64 throughout. Only the orientation predicate is exact (Shewchuk orient2d through the robust crate: kernels/robust.rs#L14-L43). Constructions are float: the segment intersection point is a conditioned homogeneous-coordinate computation with a nearest-endpoint fallback (line_intersection.rs#L297-L313), and its own docs admit the point 'may be snapped to one of the end-points' (L16-L25). Other algorithms mix the exact predicate with non-robust float selection steps; quickhull is an example (qhull.rs#L137-L158). That is why open issue #1566 sees ConvexHull return a self-intersecting ring for large coordinates.

Booleans are only as robust as i_overlay. It snap-rounds to an i32 grid at about 2^29 per half-extent (iFloat adapter.rs#L37-L42 @4cd5702), so it does not panic on overlaps or degeneracies, but:
- output vertices are moved, so union(s,s) is not equal to s (#1493);
- results are always a MultiPolygon with no error channel, and inputs must already be valid ('strictly well-defined only on valid geometries', bool_ops/mod.rs#L29-L38);
- older OGC-invalid outputs (holes touching at two points) were fixed only by enabling OverlayOptions::ogc() in i_overlay 4.4 (#1470, closed 2026-03-01);
- buffer panicked inside i_shape on a polygon with an empty interior ring that geo's own Validation accepted (#1597, closed 2026-08-21).

The old in-house sweep booleans panicked repeatedly (#913, #976, #1064, #1103, #1174, #1189, #1193); those reproducers are kept as regression tests (bool_ops/tests.rs#L190-L460). Open panics remain in distance (#1596, #1604), interior_point with -0.0 (#1578), voronoi (#1558), contains with large coordinates (#921) and integer overflow (#1001).

Testing: unit tests, a JTS XML conformance runner (jts_overlay_tests at bool_ops/tests.rs#L128-L131; jts-test-runner README says not all JTS tests are handled), and two fuzz targets (separable_distance, simplify). CI only builds the fuzz targets with `cargo build --bins` and never runs them (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/.github/workflows/test.yml#L163-L177). There is no boolean fuzzing and no property-based validity check of outputs.

**Determinism:** There is no determinism guarantee and no test for it.

- **WASM:** CI only runs `cargo check --target wasm32-unknown-unknown` for geo-traits, geo-types and geo. It never runs tests in WASM, and nothing compares native and WASM outputs (test.yml#L192-L204).
- **Trig:** algorithms call f64 trig through num-traits/std: Rotate uses sin_cos (affine_ops.rs#L418), Skew uses tan (L456-L457), and round buffer caps use cos/sin (buffer.rs#L284-L300). Native builds use the platform libm while wasm32 uses Rust's libm port, so bit-identical results are not guaranteed. That is inferred, not tested here (unverified).
- **i_overlay grid scale:** the boolean adapter derives its grid scale from log2 of the bounding box (iFloat adapter.rs#L37-L42). This makes boolean output depend on the extent of all inputs taken together, not on each geometry alone.
- **Threading:** the default features enable i_overlay/allow_multithreading and rayon (geo/Cargo.toml#L16, #L21). Whether output is invariant to thread count is unverified.

Core +,-,*,/ and sqrt are IEEE-deterministic, so the simple Euclidean algorithms are likely reproducible. Nothing is pinned or asserted.

**Strengths**

- Licence is clean (MIT OR Apache-2.0), and every non-optional dependency is MIT or Apache.
- Mature, actively maintained, widely used (24M+ downloads), and the de facto interchange types for Rust GIS. geo-types/geo-traits would be a sensible GIS import/export boundary format.
- Exact Shewchuk orient2d is used for classification in segment intersection and convex hull (kernels/robust.rs#L14-L43).
- A clean Kernel trait abstraction selects predicates per scalar type (kernels/mod.rs#L27-L63; lib.rs#L395-L430). This is a useful pattern to study.
- A full DE-9IM relate with PreparedGeometry exists, ported from JTS, and a RelateNG port is underway (PRs #1590 and #1591).
- JTS XML conformance-suite harness (jts-test-runner) and a regression corpus built from real boolean-failure issues (bool_ops/tests.rs#L190-L460).
- The Validation trait has a precise InvalidPolygon taxonomy (validation/polygon.rs#L20-L49), including the simply-connected-interior check.
- Good docs, honest about limits (Polygon validity is not enforced; boolean validity preconditions are stated).

**Weaknesses**

- No curves at all: no arcs, circles, ellipses, splines or bulge polylines. Circles are sampled polygons (buffer.rs#L284-L300). This disqualifies it for AutoCAD-class drafting.
- 2D only with no Z (Coord{x,y}). No topology structure and no persistent IDs, so it cannot support B-rep or associative drafting.
- Validity is not enforced by construction (polygon.rs#L46-L55). Booleans assume valid inputs and return MultiPolygon with no error channel (bool_ops/mod.rs#L29-L38, #L91-L107).
- Booleans are outsourced to i_overlay's i32 snap-rounding. Output vertices move, so s ∪ s ≠ s (#1493). Precision is about bbox/2^30, and the grid depends on the combined extent of the inputs.
- geo pins i_overlay 4.5.x while upstream is at 9.1.0 (geo/Cargo.toml#L43), so geo is five majors behind its own boolean engine.
- Robustness is uneven: exact predicates, float constructions, and non-robust selection steps. There are open panics and wrong results (#1566 convex hull, #1596/#1604 distance, #1578, #1558, #921, #1001).
- Fuzzing is token: two targets, built but never run in CI, none for booleans. WASM is only type-checked. There is no determinism testing.
- Its algorithms are mostly Rust ports of JTS methods (RobustLineIntersector, GeometryGraph), so the primary source is JTS/GEOS, not geo.

**Verdict:** STUDY

**Verdict reason:** Measured against the target architecture (exact analytic and NURBS curves, exact or robust constructions, validity by construction, booleans that never silently alter or fail, bit-identical results natively and in WASM), geo is the wrong foundation.

- Its data model is OGC Simple Features: straight-segment polylines in 2D f64, with no curve, bulge or topology representation. Curves cannot be added without replacing the core types.
- Its only exact component is Shewchuk orient2d. Every construction is float, and several algorithms mix exact predicates with non-robust selection.
- Its booleans are not its own. They snap-round through an i32 grid in an i_overlay pinned five majors behind upstream, which perturbs output even for s ∪ s.

DEPEND would put a GIS polygon model at the centre of a CAD kernel. PORT has nothing to offer that the original JTS/GEOS sources and the Shewchuk/robust predicates do not offer more directly. It is not REJECT, because a few parts are worth learning from:
- the Kernel-per-scalar abstraction;
- the JTS XML conformance harness;
- the boolean-failure regression corpus and issue reproducers, useful as adversarial tests;
- the InvalidPolygon validity taxonomy;
- the DE-9IM relate/PreparedGeometry design.

At most, geo-types/geo-traits could be an optional GIS interchange adapter at the I/O edge of Spare Kit, never part of the kernel. i_overlay itself, an integer snap-rounding polygon boolean engine, deserves its own evaluation. It is still polygon-only and cannot carry exact arcs.

**What to take**

- Kernel-per-scalar predicate dispatch, so a scalar type chooses exact or naive predicates at compile time. Spare Kit should extend this to incircle, exact segment-intersection construction, and arc/arc and arc/segment predicates: https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/kernels/mod.rs#L25-L63 and https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/lib.rs#L395-L430
- The JTS XML conformance-test harness pattern, which runs an external reference suite against the kernel. Check the licence of the XML files before reusing them: https://github.com/georust/geo/tree/c12769fdf7453df1dd590e6e3bca28913d3c0564/jts-test-runner
- Boolean regression corpus built from real panics (gh_issue_913/976/1064/1103/1174/1189/1193, disconnected-interior and touching-holes cases), to import as adversarial inputs for Spare Kit's 2D region booleans: https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/bool_ops/tests.rs#L190-L460
- Issue reproducers as stress inputs: #1566 (huge-magnitude convex hull), #1493 (s ∪ s drift under snap-rounding), #1470 (OGC-invalid hole touching), #1597 (empty ring), #1604 (very short segments), #1578 (-0.0): https://github.com/georust/geo/issues/1566 https://github.com/georust/geo/issues/1493 https://github.com/georust/geo/issues/1470 https://github.com/georust/geo/issues/1597 https://github.com/georust/geo/issues/1604 https://github.com/georust/geo/issues/1578
- The InvalidPolygon validity taxonomy as a checklist for Spare Kit's region-validity invariants (enforce them by construction instead of checking on demand): https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/validation/polygon.rs#L20-L49
- The JTS RobustLineIntersector port, as a documented example of the classic 'exact classify, float construct, heuristic fallback' approach and its failure modes, which Spare Kit should surpass with exact or interval-certified constructions: https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/line_intersection.rs#L75-L313
- DE-9IM IntersectionMatrix and PreparedGeometry as API semantics for 2D topological predicates. Study JTS RelateNG directly and watch the geo port PRs #1590/#1591: https://github.com/georust/geo/tree/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/relate
- Optional, outside the kernel: geo-types/geo-traits as a GIS interchange format at Spare Kit's I/O boundary (MIT/Apache): https://github.com/georust/geo/tree/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo-traits

### Challenge 1: FACTS: I re-checked the licence, activity numbers, every cla

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- Wrong method description (grid scale). The evaluation says i_float scales by 2^(29 - floor(log2(half-extent))). The exponent is actually rounded half away from zero. adapter.rs computes `max.log2().to_i32()` (https://github.com/iShape-Rust/iFloat/blob/4cd5702a0f2596c10a0716c64437ff9cbb0696eb/src/adapter.rs#L37-L38), and f64::to_i32 is `(self + 0.5_f64.copysign(self)) as i32` (https://github.com/iShape-Rust/iFloat/blob/4cd5702a0f2596c10a0716c64437ff9cbb0696eb/src/float/number.rs#L211-L213). The scaled half-extent therefore lands in [2^28.5, 2^29.5), not [2^29, 2^30). The 'about bbox/2^30' precision statement survives. The determinism section also misses that log2 and exp2 here go through the libm crate (number.rs#L176-L178; adapter.rs#L41-L42), so the grid-scale step does not depend on the platform.
- 'Only orient2d is exact' is slightly wrong. Kernel::dot_product_sign's default implementation calls Self::orient2d(zero, u, (-v.y, v.x)) (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/kernels/mod.rs#L49-L56). Under RobustKernel the dot-product sign is therefore also exact, through Shewchuk orient2d. Only square_euclidean_distance is naive.
- Mis-cited issue. Claim 4 cites #1001 as evidence that the Kernel default and SimpleKernel overflow on integers. #1001 is a u64 subtraction overflow in geo-types Line::delta / Coord::sub, not the kernel (https://github.com/georust/geo/issues/1001). The kernel-overflow issue is #921: its backtrace shows an i32 multiply overflow inside `geo::algorithm::kernels::Kernel::orient2d` (https://github.com/georust/geo/issues/921). The robustness section calls #921 'contains with large coordinates', but it is integer overflow in SimpleKernel, not float magnitude.
- #1597 is presented as resolved: 'buffer panicked inside i_shape ... (#1597, closed 2026-08-21)'. It was closed with state_reason=not_planned. urschrei wrote 'This isn't a geo issue. Report it upstream to i-shape.' (https://github.com/georust/geo/issues/1597). The polygon buffer path still sends rings straight to i_overlay outline (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/buffer.rs#L396-L397). buffer.rs has not changed since a6e15a8 (2026-03-01). crates.io has only i_shape 1.18.0 for the ~1.18.0 requirement. The panic on a polygon that passes Validation is therefore still present at c12769f and belongs in the open-panics list.
- Minor: 'the pinned i_overlay 4.5.1'. The requirement is `version = "4.5.1, < 4.6.0"` (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/Cargo.toml#L43). A fresh resolve picks i_overlay 4.5.2 (crates.io, 2026-05-18). 4.5.2 has the same i_float ~1.16.0 dependency, so the adapter analysis still holds.
- Understated: the RelateNG port is spread over seven open PRs, not two: #1587, #1588, #1589, #1590, #1591, #1598 and #1599 (https://github.com/georust/geo/pull/1587 ... https://github.com/georust/geo/pull/1599).
- Missed point (the evaluation's 'unverified' licence note can be settled). The repo ships the JTS licences next to the XML corpus: jts-test-runner/resources/LICENSE_EDLv1.txt and LICENSE_EPLv2.txt (https://github.com/georust/geo/tree/c12769fdf7453df1dd590e6e3bca28913d3c0564/jts-test-runner/resources). JTS content is offered under EPL-2.0 or EDL-1.0, which is BSD-style (https://github.com/locationtech/jts/blob/5a0a73632216e3e9d277150336078ec423fd215f/LICENSES.md). The XML conformance files can therefore be reused under EDL-1.0, which fits Spare Kit's BSD-permissive policy.
- Missed determinism defect. MakeValid collects boundary edges into a std HashSet, which uses RandomState and so iterates in a different order in each process, and traces rings with `for &start_handle in &boundary_edges` (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/repair_polygon/mod.rs#L178, #L516-L525). assemble_polygons never normalises the order. The order of output polygons and holes, and each ring's start vertex, can therefore change from run to run even on one native platform. That is a concrete counter-example, not just 'no guarantee'.
- Missed silent-loss and tolerance issues in MakeValid. assemble_polygons drops a hole without any error when none of its vertices is strictly inside an exterior ring (repair_polygon/mod.rs#L703-L713: '// If no vertex is unambiguously inside an exterior, the hole is dropped'). Repair also snaps with a fixed absolute radius of 1e-4 whatever the units (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/triangulate_delaunay.rs#L27; repair_polygon/mod.rs#L305). Both directly contradict the 'never silently fail' requirement.
- Minor completeness: the evaluation's dependency licence list leaves out sif-itree, rand, rand_pcg, thiserror, log and num-traits (geo/Cargo.toml#L33-L46). I checked all six on crates.io and each is MIT OR Apache-2.0, so the conclusion that every non-optional dependency is MIT or Apache still holds.
- Minor link error: PreparedGeometry is cited under geo/src/algorithm/relate. It actually lives at geo/src/algorithm/indexed/prepared_geometry.rs. relate/mod.rs only re-exports it, with a deprecation note (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/indexed/prepared_geometry.rs#L30-L36).

**Argument:** Most of the evaluation checks out against the source, and none of its claim verdicts or its overall verdict is wrong.

Activity and licence: these match exactly.
- Stars 1,951, forks 272, created 2015-01-16, pushed_at 2026-10-18.
- Head c12769f (2026-09-19, #1613). 140 contributors including anonymous. 105 open issues and 30 open PRs.
- geo 0.33.1 (2026-04-20), geo-types 0.7.20 (2026-08-02). Downloads 24,433,204 and 29,705,782.
- i_overlay 9.1.0 (2026-10-14), and majors 5 through 9 all exist, so 'five majors behind' is correct.
- MIT OR Apache-2.0 in every manifest, and every dependency licence I checked is permissive.

Source citations: every one I checked is accurate.
- Coord{x,y}, the Geometry enum with no curve variants, and Polygon's 'does not enforce validity'.
- GeoNum → RobustKernel/SimpleKernel and RobustKernel → robust::orient2d.
- BooleanOps → FloatOverlay::with_subj_and_clip_custom(OverlayOptions::ogc()), where the adapter is built from the combined subject and clip extent (overlay.rs#L152-L153).
- The sampled round cap in Buffer, the Quickhull f64 dot product with partial_cmp().unwrap(), the JTS RobustLineIntersector port with its nearest-endpoint fallback, and the simplified-sweep comments.
- The InvalidPolygon taxonomy, the CI fuzz step that only runs `cargo build --bins`, and the WASM step that only runs `cargo check`.
- sin_cos and tan in affine_ops.

Issue reports: the cited issues say what the evaluation says.
- Code search finds 0 hits for ellipse, bezier, bulge, spline and CircularString.
- In #1493, maintainers advise pre-snapping. #1470 was fixed by #1500 (i_overlay 4.4 with OGC options).
- #1566 reproduces as described. I traced its cause and it confirms the evaluation's attribution to non-robust float selection. For the reporter's points, both hull_set dot products round to 576460752306102528 (exact values …468 and …404). max_by then returns the later point, (0,1), instead of the true extreme (0,0). The exact-predicate partitions overlap, so (0,0) is emitted twice.

The errors are in method details and issue citations, not in verdicts:
- floor vs round in the i_float scale exponent;
- dot_product_sign is also exact;
- #1001 cited where #921 is the kernel overflow;
- #1597 presented as resolved, though it was closed as not planned and the panic remains;
- 4.5.1 vs 4.5.2;
- two RelateNG PRs listed out of seven.

The evaluation also misses MakeValid defects that strengthen its own case:
- output order depends on HashSet iteration, so it changes between runs;
- a hole can be dropped silently;
- snapping uses an absolute 1e-4 radius.

The 'unverified' JTS XML licence can be settled: the files can be reused under EDL-1.0, which is BSD-style.

Verdict: STUDY stands. DEPEND or PORT would be wrong, because there are no curves, no topology, and no Z. Validity is not enforced, booleans are outsourced to i_overlay's i32 snap-rounding, and, as the MakeValid HashSet traversal shows, there is no determinism guarantee. REJECT would be too harsh, because the regression corpus, the issue reproducers, the InvalidPolygon checklist and the JTS conformance harness are concrete adversarial test assets. The harness can now be reused with a clear licence. The only change I would make to what_to_take is to add the MakeValid failure modes (non-determinism, silent hole drop, absolute snap tolerance) as anti-patterns.

### Challenge 2: FIT: the evaluation's verdict is STUDY, so I looked for any 

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- #1597 was not fixed. The evaluation says buffer 'panicked' (past tense) and cites the issue as closed 2026-08-21. It was closed as not_planned: the maintainer said 'This isn't a geo issue. Report it upstream to i-shape.' (https://github.com/georust/geo/issues/1597; state_reason=not_planned). i_overlay 4.5.1 needs i_shape ~1.18.0, and crates.io has only 1.18.0, with no 1.18.x patch. So Buffer still panics on an empty interior ring that geo's own Validation accepts, and geo hands boolean/buffer panics to an upstream it pins five majors behind.
- 'Only orient2d is exact' is slightly wrong. Kernel::dot_product_sign reduces to orient2d(0, u, (-v.y, v.x)) using exact negation and swap, so under RobustKernel the sign of the dot product is exact too (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/kernels/mod.rs#L45-L56). square_euclidean_distance is the only inexact Kernel method. The 'partly' verdict is unaffected.
- The RelateNG port is a stack of seven open, unmerged PRs, not two: #1587, #1588, #1589, #1590, #1591, #1598 and #1599 (gh search 'repo:georust/geo RelateNG is:pr'). It is further from landing than the evaluation implies.
- The 'unverified' JTS XML licence question can be answered. JTS content is offered under EPL-2.0 or EDL-1.0, and EDL-1.0 is a BSD-3-Clause text (https://github.com/locationtech/jts/blob/5a0a73632216e3e9d277150336078ec423fd215f/LICENSES.md#L7 and LICENSE_EDLv1.txt). The testxml files are usable under Spare Kit's BSD allowance. It also means JTS, the upstream of geo's relate, validity, monotone-chain and line-intersector code, can be PORTED directly. GEOS (LGPL) is study-only, so the evaluation's 'JTS/GEOS' should not lump them together.
- The grid-scale formula is slightly off. i_float computes the exponent as 29 - to_i32(log2(half-extent)), and to_i32 rounds half away from zero; it is not floor (https://github.com/iShape-Rust/iFloat/blob/4cd5702a0f2596c10a0716c64437ff9cbb0696eb/src/adapter.rs#L37-L42; number.rs#L211-L213). Grid step is between h/2^29.5 and h/2^28.5, so 'about bbox/2^30' is roughly right. log2 and exp2 come from the pure-Rust libm crate (number.rs#L176-L178), so the grid scale itself is platform-deterministic. That is a determinism fact the evaluation missed.
- Missed weakness: snapping is not limited to one grid cell. geo passes Solver::default() (bool_ops/mod.rs#L99-L105). That is Solver::AUTO with Precision::HIGH, a snap radius that starts at 1 grid unit and doubles every split pass (https://github.com/iShape-Rust/iOverlay/blob/2d3bedad29a08436fc75a8e92e6c5ff37b45e7b5/iOverlay/src/core/solver.rs#L46-L50, #L132-L138; split/solver_list.rs#L21-L57; snap_radius.rs#L8-L16, capped at 2^60). Vertex displacement therefore grows on dense near-degenerate inputs. AUTO also switches between list, tree and fragment solvers at 4,000 and 16,000 segments (solver.rs#L140-L142), so the code path depends on input size.
- Missed silent-failure mode: unary_union picks its fill rule from the winding of the first ring it sees (Positive if CCW, else Negative) and never checks the rest (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/bool_ops/mod.rs#L262-L282). The docs require consistent winding (#L223-L224), but geo-types does not enforce winding, so an input wound the other way is silently dropped from the union. That violates 'booleans never silently fail'.
- Missed evidence that union is not associative: test_unary_union_errors asserts only that iterative pairwise union and unary_union agree within 1e-5 relative area, because 'error is accumulated differently' (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/bool_ops/tests.rs#L89-L112). Boolean results depend on how operations are grouped, which matters for the determinism requirement.
- Several strengths the evaluation cites have not been released. InteriorNotSimplyConnected validation (#1472), the scale-invariant point-to-segment distance (#1613) and others sit under 'Unreleased'. The last geo release is 0.33.1 from 2026-04-20, six months ago, and release request #1618 is open (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/CHANGES.md#L3-L11, #L47).
- The causal claim 'that is why #1566 returns a self-intersecting ring' (the float dot-product step in quickhull) is an inference. The issue gives a reproducer but no root cause (https://github.com/georust/geo/issues/1566). Treat it as unverified.
- Missed candidate, with a provenance warning: MakeValid (repair_polygon) implements the prepair method with an error channel (https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/src/algorithm/repair_polygon/mod.rs#L1-L14, #L195-L230). The reference implementation, tudelft3d/prepair, is GPL-3.0. PR #1518 says it follows 'prepair's stack-based reconstruct() approach' and that its test expectations came from running prepair (https://github.com/georust/geo/pull/1518). Spare Kit must not port this code. Implement from the Ledoux et al. 2014 paper instead.
- Missed GeoRust candidate for a separate evaluation: the robust crate (listed on georust.org, MIT OR Apache-2.0, no_std) provides Shewchuk orient2d, orient3d, incircle and insphere (https://github.com/georust/robust/blob/654f34cb8cdb3ae21bf59ef3472f92652942cd74/src/lib.rs#L77, #L173, #L571, #L1332). The FMA PR #27 was closed without merging, so no FMA path is in play, which favours bit-identical native and WASM output. It is f64-only and has no symbolic perturbation. It is directly relevant to the 3D kernel, more so than anything in geo itself.
- Minor gap: the dependency licence list leaves out sif-itree, rand and rand_pcg (geo/Cargo.toml#L44-L46), and the thiserror, log and num-traits dependencies. All are MIT OR Apache-2.0, so the licence conclusion stands.

**Argument:** I could not overturn STUDY. Every headline fact I checked holds. Activity metrics hold: 1,951 stars, pushed 2026-10-18, 105 open issues and 30 open PRs, about 140 contributors, geo 0.33.1 and geo-types 0.7.20, i_overlay pinned to 4.5.x against upstream 9.1.0. The data model is OGC Simple Features in 2D f64 with no curves; code search finds 0 hits for ellipse, bezier, bulge, spline and CircularString. orient2d goes through robust, booleans are delegated to i_overlay's i32 grid, and CI only builds the fuzz targets and only type-checks for WASM.

The strongest case for PORT is this: geo is the only permissive Rust codebase that combines
- full DE-9IM relate with PreparedGeometry;
- a complete OGC validity checker, including simply-connected interiors;
- point-in-polygon structures that classify using only exact orient2d against input vertices, with no constructions (MonotonicPolygons, IntervalTreeMultiPolygon);
- a MakeValid implementation of prepair, whose reference code is GPL.

That case fails on four counts:
1. The relate, validity, monotone-chain and line-intersector code are ports of JTS. JTS is available under EDL-1.0 (BSD-3), which Spare Kit's policy allows, so the upstream can be ported directly. JTS is also more mature and already ships RelateNG, which in geo is seven unmerged PRs.
2. MakeValid has GPL provenance: its PR says it follows prepair's reconstruct() and took its test expectations from prepair. It should be rebuilt from the paper, not ported.
3. Every one of these algorithms assumes straight-segment f64 rings and cannot carry exact arcs.
4. Recheck turned up more reasons not to DEPEND:
   - #1597 (Buffer panic on input geo calls valid) was closed not_planned, not fixed.
   - unary_union silently drops polygons wound the wrong way.
   - i_overlay doubles its snap radius every split pass, so vertex movement is not bounded to one grid cell.
   - Pairwise union and unary_union disagree; the tests accept 1e-5 relative difference.

REJECT would be too harsh. The geo-specific regression corpus and the 2025-2026 issue reproducers are worth importing as adversarial tests, and nothing else supplies them: boolean panics #913 to #1193, #1493 (s ∪ s drift), #1566 (non-convex hull), #1578 (-0.0), #1597 (empty ring), #1604 (very short segments). STUDY stands, with a few corrections:
- take JTS testxml and JTS algorithms from JTS itself, under EDL;
- give the robust crate, and possibly spade and geo-index, their own evaluations, since they are closer to best-in-class for Spare Kit than geo;
- note that i_overlay's grid-scale computation is deterministic (it uses libm), but its output depends on the combined input extent, the order operations are grouped in, and the number of snapping passes.

The evaluation's verdict and its claim verdicts stay correct; the errors above are minor factual slips and omissions.

## RGeometry

**Repo url:** https://github.com/rgeometry/rgeometry (pinned at 6d2c21445ed0d7713c4553b368903392505f9cd0, 2026-10-12)

**Licence:** The Unlicense, a public-domain dedication that is permissive in practice, plus a WAIVER file. Sources: LICENSE https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/LICENSE, WAIVER https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/WAIVER, and `license = "Unlicense"` in https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/Cargo.toml. Every release on crates.io is Unlicense. Dependencies: apfp 0.1.1 (Unlicense), num/num-bigint/num-rational/array-init/claims (MIT/Apache-2.0) and rand 0.9 (MIT/Apache-2.0). The optional `rug` feature pulls in rug 1.28, which is LGPL-3.0+ and links C GMP/MPFR. That feature must never be enabled under Spare Kit's licence rules. Nothing in the default feature set is copyleft.

**Activity:** Created 2021-04-29. Last push 2026-10-15 (flake.lock bots). The last substantive commits were 2026-09-14: Chan's hull (#239) and a monotone-polygon collinear fix (#243). 182 stars, 19 forks, 10 open issues and PRs together. 8 contributors, but one author (lemmih) wrote 305 commits; the next human contributors have 1-2 each. There are 64 non-bot commits since 2025-10-21, concentrated in a Nov 2025 to Jan 2026 burst, and the repo has AGENTS.md/CLAUDE.md, so it is AI-assisted. Latest release 0.12.0 on 2026-01-06. crates.io: 21,253 total downloads, 184 recent. What the history shows: the $50 bounty for polygon Booleans (#80) and the Boolean PR (#117) were closed unfilled, the half-edge planar graph PR (#49) and an earlier Delaunay PR (#84) were closed unmerged, and efficient line intersection (#92) and exact `intersection_point` (#53, open since 2021) are still open. In short, a single maintainer runs it as a hobby and teaching project.

**Claims**

- **claim**: It is pure-Rust and safe; **verdict**: partly; **evidence**: The default build is pure Rust. However: (a) the optional `rug` feature binds C GMP and is LGPL (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/Cargo.toml, `rug = { version = "1.28", optional = true }`; impl at https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/lib.rs#L1057-L1190). (b) There is no `#![forbid(unsafe_code)]`, and there is one `unsafe` pointer cast from Point to Vector that relies on repr(transparent) (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/data/vector.rs#L75-L78). (c) 'Safe' does not mean panic-free: earclip panics with `expect("no valid ears found - polygon may be degenerate")` (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/triangulation/earclip.rs#L64-L71), and `locate`, earclip, CDT and visibility all `assert!` that the polygon has no holes.
- **claim**: It is a computational geometry library for academic algorithms, including ear-clipping triangulation and Melkman convex hull; **verdict**: true; **evidence**: The README pitches it as a learning and mentoring project (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/README.md). Ear clipping: https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/triangulation/earclip.rs#L11-L104. Melkman is a textbook deque implementation: https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/convex_hull/melkman.rs#L12-L85. Also present: Graham scan, gift wrapping, Chan, naive and Bentley-Ottmann segment intersection, CDT by ear-clip plus Lawson flips, naive O(n^2) kernel, naive visibility, and polygon generators (2-opt, monotone, star). All are 2D, and all except the Bentley-Ottmann sweep work on simple polygons only (src tree at the pinned SHA).
- **claim**: It implements 'geometric hashing'; **verdict**: partly; **evidence**: Geometric hashing in the usual sense (the affine-invariant object-recognition method) does not exist here. What exists is Z-order/Morton hashing (bit interleaving of quantised coordinates) used only to speed up ear-clip candidate checks, as in earcut.js: https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/zhash.rs#L1-L129 and https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/triangulation/earclip.rs#L106-L211. The phrase comes from rgeometry.org ('Ear clipping with geometric hashing'), and the summary turned it into a separate algorithm.
- **claim**: It compiles cleanly to WebAssembly; **verdict**: true; **evidence**: CI builds the whole workspace for wasm32-unknown-unknown. `nix flake check` includes `documentation-check`, which embeds all wasm demos (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/flake.nix#L79-L99, #L190-L198, #L327-L328). The interactive doc examples run as wasm. Caveats: CI only compiles to wasm and runs no tests there; the `rug` feature cannot target wasm; and outputs are not bit-identical between wasm32 and 64-bit native (see determinism).
- **claim**: (own docs) Bentley-Ottmann runs in O((n+k) log n) using an order-statistics tree for the status structure; **verdict**: false; **evidence**: The doc claim is at https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/intersection/bentley_ottmann.rs#L13-L29. In the code, the status is a plain `Vec<usize>` with linear `position`/`contains`, a linear `segments_containing` scan per event, and a full `sort_by` of the active list after every event (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/intersection/bentley_ottmann.rs#L294-L360). That is roughly O((n+k)·m log m) with m active segments, and every comparison does allocating BigRational arithmetic. It only reports intersecting pairs, not points or an arrangement, and it is BigRational-only (#L59-L73). The crate's default `segment_intersections` and `Polygon::validate` still use the naive O(n^2) pair loop (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms.rs#L12-L13, https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/data/polygon.rs#L176-L200).
- **claim**: (own docs/AGENTS.md) It uses exact arithmetic and supports generic number types; **verdict**: partly; **evidence**: Number model: the `PolygonScalar` trait (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/lib.rs#L134-L224) is implemented for i8 through i64 and isize, BigInt, BigRational, f32 and f64 (#L862-L865, #L1052-L1055). Predicates (orientation, slope and edge-normal comparisons, incircle, cmp_dist) are exact for every type. Integers use apfp `int_signum!` with widening. Floats use apfp's Shewchuk-style adaptive `apfp_signum!`, while f64 `cmp_dist` converts to BigRational (#L280-L860). Constructions are not exact: `approx_intersection_point` is plain arithmetic in T (#L215-L248, used by https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/data/line.rs#L46-L57), the kernel says results are approximate for integers and floats (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/kernel/naive.rs#L15-L25), and issue #53 (exact intersection_point) has been open since 2021. Only `BigRational` gives exact constructions, and it allocates. There are no exact-rational-with-filter or lazy-exact constructions as in CGAL Epeck.
- **claim**: (rgeometry.org/correctness) All algorithms are tested with 8, 16, 32 and 64-bit integers and f32/f64; **verdict**: false; **evidence**: Bentley-Ottmann only accepts BigRational, and its property test covers at most 5 non-vertical segments with coordinates in [-5,5] (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/intersection/bentley_ottmann.rs#L627-L655). Many other modules do run i8 and f32 property tests, e.g. https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/convex_hull/melkman.rs#L199-L232. Counted at the pinned SHA: 207 `#[test]` and 48 `#[proptest]` functions.

**Architecture:** One library crate, `rgeometry`: about 12.9k lines under src/ including inline tests. It sits beside `rgeometry-wasm` (playground bindings, wasm-bindgen pinned =0.2.100) and 9 demo crates (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/Cargo.toml).

Number layer: `PolygonScalar` plus `TotalOrd` in src/lib.rs (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/lib.rs#L94-L224). Every predicate is a trait method on the scalar type and takes raw `[T;2]` coordinates. Edge-normal variants avoid building rounded intermediate vectors.

Data (src/data):
- `Point<T, const N>` is a repr(transparent) `[T;N]` (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/data/point.rs#L17-L22). N is generic but every predicate and algorithm is 2D-only.
- Also: Vector, Line/LineSoS, LineSegment with Inclusive/Exclusive endpoints, Triangle and IntersectionSet.
- `Polygon<T>` is a flat `Vec<Point>` plus `rings: Vec<Vec<PointId>>` (ring 0 is the CCW outer boundary, the rest are CW holes) with reverse indices (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/data/polygon.rs#L113-L157). `PolygonConvex` wraps it.

What is missing:
- No topology structure: no half-edge or DCEL (PR #49 closed unmerged), no planar arrangement, no overlay.
- No curves of any kind: no arcs, circles, splines or NURBS.
- No 3D algorithms and no meshes beyond triangle index triples.
- No Booleans or offsets.
- The only spatial index is a Z-order hash.

Algorithms (src/algorithms): convex_hull (graham, gift_wrapping, melkman, chan), intersection (naive, bentley_ottmann), triangulation (earclip, constrained_delaunay), polygonization (monotone, star, two_opt), kernel/naive, visibility/naive, zhash. Orientation and the SoS tie-break are in https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/orientation.rs. The hardest algorithm present is the BigRational Bentley-Ottmann pair reporter described above. There is nothing comparable to a B-rep, Boolean or curve-curve intersection.

**Robustness:** Arithmetic and predicates:
- Predicate signs are exact for every supported scalar. There are no epsilons.
- f32/f64 use apfp 0.1.1 `apfp_signum!`, a macro-generated staged evaluator (an f64 pass with an error bound, then double-double, then exact expansions in stack buffers, using Dekker split and two_product without FMA) (https://docs.rs/crate/apfp/0.1.1/source/src/analysis/adaptive_signum.rs, https://docs.rs/crate/apfp/0.1.1/source/src/expansion.rs).
- I found no overflow, underflow or non-finite handling in apfp's adaptive_signum.rs (grep for overflow/underflow/is_finite/NaN returns nothing). Exactness therefore holds only inside the usual Shewchuk no-overflow/no-underflow range. apfp's GitHub repository (github.com/lemmih/apfp) returns 404, so it can only be audited through docs.rs. Unverified: its behaviour near the exponent limits.
- Integers use apfp `int_signum!` with widening, so the predicates never overflow. f64 `cmp_dist` converts to BigRational and panics on NaN/inf (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/lib.rs#L1192-L1194).
- `Point::new` does not reject NaN; only `new_nn` does (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/data/point.rs#L100-L117).

Constructions are inexact except with BigRational (see claims). Nothing like snap rounding or exact-construction kernels.

Degeneracies:
- Segment-segment classification handles collinear overlap and endpoint touching with inclusive/exclusive endpoints (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/data/line_segment.rs#L302-L360).
- Point-in-polygon and visibility use 'leaning' SoS rays (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/data/polygon.rs#L202-L232, https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/visibility/naive.rs#L13-L60).
- The general `SoS::new(a,b,c)` breaks collinear ties from point indices alone. It ignores coordinates, so it is not a full Edelsbrunner-Mücke expansion (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/orientation.rs#L554-L592).
- CDT does not flip on cocircular (incircle == Equal) configurations (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/triangulation/constrained_delaunay.rs#L89-L91).
- Holes are asserted unsupported in locate, earclip, CDT and visibility, and earclip can panic on degenerate input.

Known failures, all fixed per the CHANGELOG (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/CHANGELOG.md):
- Melkman bug (#129)
- line-intersection panic (#120)
- Polygon::new panic (#63)
- visibility crash for f64 (#83)
- wrong perpendicular in the rug predicate (#201)
- monotone-polygon collinear error (#243)

The issue tracker has almost no user bug reports, which reflects a small user base rather than proven robustness.

Tests: 207 unit tests and 48 `#[proptest]` functions, 14 proptest-regression files, and shrinkable polygon strategies (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/testing.rs#L33-L165). Fast algorithms are cross-checked against naive ones, mostly on i8 coordinates. There is no cargo-fuzz harness, and Codecov runs in CI.

**Determinism:** Predicates are exact, so they are deterministic on every target. The algorithms built on them are not deterministic across targets.

1. Ear clipping picks ears with `SmallRng::seed_from_u64(0xDEADBEEF)` and `random_range` (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/triangulation/earclip.rs#L11-L37, #L421-L432). In rand 0.9.5, which is locked, SmallRng is Xoshiro256++ on 64-bit targets and Xoshiro128++ on 32-bit targets such as wasm32. rand explicitly documents it as non-portable (https://github.com/rust-random/rand/blob/3474ec047279e5de32676c50154ff7d76d12a94e/src/rngs/small.rs#L13-L35). Native x86_64 and wasm32 therefore pick different ears and can return different, though valid, triangulations. Results can also change with any rand upgrade.
2. CDT starts from that ear-clip output. It fills its flip queue by iterating a std `HashMap` with per-process random `RandomState` seeds (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/triangulation/constrained_delaunay.rs#L28-L42), and it never flips cocircular quads. For cocircular input, such as rectangles and grids, which are everywhere in buildings, the diagonal choice depends on the platform's RNG and can vary between runs.
3. Bentley-Ottmann uses a BTreeMap event queue, so its order is deterministic. Its HashMaps are only used for membership checks.
4. Approximate f64 constructions are plain IEEE ops. Rust does not contract them to FMA, so they are reproducible but inexact.
5. No test anywhere checks reproducibility across targets, and no tests run on wasm.

Verdict: it does not meet Spare Kit's bit-identical native/WebAssembly requirement.

**Strengths**

- Every 2D predicate is exact for every scalar type, including overflow-free integer predicates by widening and adaptive exact f32/f64 signs, with no epsilons anywhere (src/lib.rs#L280-L860).
- Its API pattern avoids rounded intermediates by evaluating sign expressions on the original coordinates (edge-normal predicates). Regression tests show the naive f64 sign error they prevent (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/lib.rs#L1200-L1419).
- Good property-testing discipline: generic tests run over i8 coordinates to force degeneracies, fast algorithms are cross-checked against naive references, and polygon shrinkers keep failing cases valid (src/testing.rs).
- Leaning SoS rays handle degenerate point-in-polygon and visibility cleanly.
- Unlicense, few dependencies, readable code with interactive wasm doc examples, and CI-verified wasm32 compile.

**Weaknesses**

- Scope is about 1% of what Spare Kit needs. It is 2D only, with straight line segments only: no arcs, circles, conics or NURBS, no 3D, no B-rep, no meshes beyond triangle triples, no Booleans (bounty #80 and PR #117 closed unfilled), no offsets, no half-edge, DCEL or arrangement (PR #49 closed unmerged).
- Constructions are inexact unless you use allocating BigRational. Exact intersection_point has been open since 2021 (#53), and there are no filtered or lazy exact constructions.
- The Bentley-Ottmann implementation contradicts its own doc. It claims O((n+k) log n) with an order-statistics tree but uses a Vec status that is fully re-sorted at every event. It is BigRational-only, reports only intersecting pairs, and its property test covers at most 5 non-vertical segments. Polygon validation still uses the O(n^2) naive check.
- Holes are unsupported: `assert!` in locate, earclip, CDT and visibility. Earclip can panic on degenerate input, NaN is not rejected by `Point::new`, and f64 cmp_dist panics on non-finite input. That contradicts 'never fail silently, never crash'.
- Not deterministic across targets: SmallRng is pointer-width dependent, and CDT seeds its flip order from HashMap RandomState.
- Exact float signs rest on apfp 0.1.1: one author, about 1k downloads, GitHub repo returns 404, and no overflow/underflow handling.
- Effectively a single maintainer (305 of about 350 human commits), hobby and teaching scope, docs out of date (AGENTS.md still cites geometry-predicates), and 184 recent downloads.

**Verdict:** REJECT

**Verdict reason:** RGeometry is a well-meant 2D teaching library of textbook polygon algorithms over straight segments. No component in it could serve as a dependency or porting source for an exact B-rep, NURBS, drafting and mesh kernel.

- It has no curves, Booleans, topology structure, 3D or exact constructions.
- Its one advanced algorithm, Bentley-Ottmann, is not even O((n+k) log n) as documented.
- It breaks the iron-clad requirements: it panics or asserts on holes and degeneracies, and it is not bit-identical between native and wasm32.

Its exact-predicate layer is sound, but it is a thin wrapper over apfp, an unaudited 0.1 crate whose repository returns 404. Shewchuk's predicates, Geogram PCK and CGAL's filtered kernels are the canonical and better sources for that technique. None of its algorithms is best-in-class for CAD. The Unlicense would allow porting, but the owner's rule forbids porting anything that is not best-architecture, and nothing here qualifies.

**What to take**

- Minor, for test-suite design only and not a reason to adopt: run every algorithm's property tests generically over tiny integer coordinates (i8) so that collinear, duplicate and touching degeneracies come up constantly, and cross-check each fast algorithm against a naive reference. Pair this with shrinkers that keep polygons valid while minimising failures (cut ears, simplify coordinates). See https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/testing.rs#L33-L165 and https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/convex_hull/melkman.rs#L199-L232.
- Minor design reference: apfp's `apfp_signum!` / `int_signum!`. These declarative macros size stack expansion buffers and error bounds at compile time from an arbitrary +,-,*,square expression, giving allocation-free staged exact signs without a build script (https://docs.rs/crate/apfp/0.1.1/source/src/analysis/adaptive_signum.rs). It is worth comparing against Geogram PCK's predicate generator and Shewchuk when Spare Kit designs its own predicate-generation tooling. Do not depend on it: the repo is a 404, it has no overflow/underflow/non-finite handling, and it is at version 0.1.
- Cautionary example: rand's `SmallRng` used inside a geometric algorithm silently breaks native/wasm32 bit-identity (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/triangulation/earclip.rs#L19, https://github.com/rust-random/rand/blob/3474ec047279e5de32676c50154ff7d76d12a94e/src/rngs/small.rs#L13-L16). Spare Kit should forbid non-portable RNGs and HashMap iteration order in kernel code, for example with a clippy `disallowed_types` rule.

### Challenge 1: FACTS: I re-checked the licence, the activity numbers, every

**Refuted:** True

**Corrected verdict:** REJECT

**Errors found**

- MAJOR, wrong key claim: 'Predicate signs are exact for every supported scalar' and strength #1 ('adaptive exact f32/f64 signs') are false for f32/f64. apfp's double-double stage uses |lo| as its error bound, which does not bound the rounding that dd_add/dd_mul accumulate. After renormalisation |lo| <= ulp(hi)/2, so any nonzero rounding noise is certified as a sign. Code: https://github.com/rgeometry/apfp/blob/175ab9ac94d7713d36de38aee5659470cc23b390/src/analysis/adaptive_signum.rs#L70-L113 and the macro that trusts it at #L1395-L1415. rgeometry routes f64 orientation through it: https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/lib.rs#L668-L681 calls apfp orient2d at https://github.com/rgeometry/apfp/blob/175ab9ac94d7713d36de38aee5659470cc23b390/src/geometry/f64.rs#L34-L35. Counterexample: a=(1000.25, 3000.75), b=(3, 9), c=(0.001, 0.003). All three lie exactly on y=3x in f64 (exact rational determinant = 0). orient(a,b,c) returns +1 (CCW) from the dd stage on a residual of 1.26e-29 that is pure rounding noise. The cyclic permutation orient(b,c,a) reaches the exact stage, so the predicate also contradicts itself. Over 400k random exactly-collinear trials with |coords| <= 1e6, 16,028 of the 17,020 inputs that reached the dd stage got a wrong sign. Why apfp's own tests miss it: the dd-stage generator pins c=(0,0) (https://github.com/rgeometry/apfp/blob/175ab9ac94d7713d36de38aee5659470cc23b390/tests/geometry_tests.rs#L440-L455), which makes every difference exact. Verified by a line-by-line Python transcription of the pinned source in IEEE binary64 with no FMA (a local scratch script, not kept). The Rust crate itself was NOT built or run. The docs.rs source of 0.1.1 matches.
- MAJOR, wrong key claim: the fast filter is also invalid for nested expressions. `_apfp_fast_mul_fold` sets abs=|computed product| and discards the factors' magnitudes (https://github.com/rgeometry/apfp/blob/175ab9ac94d7713d36de38aee5659470cc23b390/src/analysis/adaptive_signum.rs#L884-L897). incircle multiplies by cancelling differences (https://github.com/rgeometry/apfp/blob/175ab9ac94d7713d36de38aee5659470cc23b390/src/geometry/f64.rs#L105-L114), so its claimed bound is too small. Counterexample, with abc CCW: a=(483.08218714741673, 564.6469568865), b=(253.26266023669837, 208.89351342072268), c=(250.60137058023122, 199.23040015746668), d=(1089.033297626453, -695.0590919649426). The true determinant is -3.63e-5 (outside). The fast value is +2.86e-4 against a claimed bound of 2.23e-4, so it returns +1 (inside). That happened in 50 of 21,731 fast-certified near-cocircular trials. rgeometry's f64 incircle (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/lib.rs#L839-L858) drives the CDT flip decision (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/triangulation/constrained_delaunay.rs#L89-L91). Same verification method (sim\incircle.py), not run in Rust. Both bugs came in with #199 (2026-01-04), which replaced the Shewchuk port geometry-predicates with apfp, so 0.12.0 regressed. Consequences: what_to_take #2 ('allocation-free staged exact signs', a design reference) should be a cautionary example, and the claim that apfp is unsafe only 'near the exponent limits' understates the problem.
- Factual: 'apfp's GitHub repository returns 404, so it can only be audited through docs.rs' is wrong. Only the crates.io link (lemmih/apfp) is dead. The source is at https://github.com/rgeometry/apfp: Unlicense, created 2025-10-11, last push 2026-01-04, 41 commits by lemmih, with tests/ and benches/. 0.1.1 corresponds to commit 175ab9ac94d7713d36de38aee5659470cc23b390.
- Factual: 'Every release on crates.io is Unlicense' is false. crates.io API: 0.1.0, 0.2.0 and 0.2.1 (May 2021) are 'Unlicense OR BSD-2-Clause'. Still permissive, so this is minor.
- Claim 'pure-Rust and safe' (partly): the evidence understates the unsafe code. Each apfp_signum! call site expands `unsafe { &mut *buffer.as_mut_ptr() }` on `MaybeUninit::<[f64; N]>::uninit()` into rgeometry's own code (https://github.com/rgeometry/apfp/blob/175ab9ac94d7713d36de38aee5659470cc23b390/src/analysis/adaptive_signum.rs#L1409-L1413). The call sites are https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/lib.rs#L733, #L762, #L786, #L810 and #L831. The std docs say that turning that pointer into a reference is UB unless the value is initialised. So the vector.rs cast is not the only unsafe code. The verdict stays 'partly'.
- Activity numbers are slightly off. GitHub contributors API: there are 313 human commits (305 lemmih + 8 others), not 'about 350'; 351 counts dependabot's 38. Non-bot commits since 2025-10-21 number 66 if dependabot and the 12 flake.lock auto-updates are excluded, and 78 if the flake.lock updates are included. Neither is 64.
- Missed point: integer constructions can overflow. For fixed ints, approx_intersection_point evaluates cubic terms in the doubled type, i16 for i8 and i128 for i64 (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/lib.rs#L485-L497 with #L226-L247). Example: i8 lines (127,0)-(0,127) and (-128,-1)-(127,1) give x_num = -4,112,768, which does not fit i16. That panics with overflow checks and wraps silently in a dependent's release build. The callers are Line::intersection_point (line.rs#L46-L57), the kernel and visibility (visibility/naive.rs#L194). The evaluation only calls these constructions 'approximate'. Worked out from the source, not run.
- Minor or unverified: for a lone cocircular quad such as a rectangle, the CDT diagonal comes from the ear-clip RNG, which is deterministic on a given platform. Variation between runs caused by HashMap RandomState only needs several interacting flips, and that is unverified. Also, given the incircle bugs above, the 'CDT never flips cocircular quads' claim holds only for exact scalar types. The README's 'learning and mentoring project' framing comes from its Contribute section, not its main description.

**Argument:** REJECT stands. If anything, the evaluation was too generous: it praised the one layer it called sound, and that layer is faulty for f32/f64.

**Checked and correct:**
- **Licence:** Unlicense plus WAIVER. The rug/gmp-mpfr-sys feature is LGPL-3.0+. The other deps are MIT/Apache.
- **Repo facts:** 182 stars, 19 forks, created 2021-04-29, 0.12.0 released 2026-01-06, 21,253 total downloads and 184 recent.
- **Issues:** #80 bounty closed not_planned. PRs #117, #49 and #84 were closed for inactivity without merging. #92 and #53 are still open.
- **Bentley-Ottmann:** the doc's O((n+k) log n) and order-statistics-tree claims are false. The status is a Vec, re-sorted after every event, and the code is BigRational-only. The property test covers at most 5 segments.
- **Validation:** `segment_intersections` and validate still use the naive pair loop.
- **Tests:** 207 #[test] (200 in src plus 7 in tests/two_opt.rs), 48 #[proptest] and 14 regression files.
- **WebAssembly:** CI compiles wasm32 only, with doCheck=false.
- **Determinism:** SmallRng is pointer-width dependent (rand 0.9.5 small.rs#L13-L35), and CDT seeds its queue from a HashMap.
- **Panics and NaN:** holes trigger asserts, earclip has the expect, `Point::new` accepts NaN, and f64 cmp_dist panics on non-finite input.
- **Doc drift:** AGENTS.md still cites geometry_predicates.
- **Website claim:** the rgeometry.org/correctness statement really is false. Besides Bentley-Ottmann, the kernel module is tested only with BigRational.

**What is wrong:** the central technical judgement, that 'predicate signs are exact for every supported scalar'. A faithful transcription of apfp 0.1.1 shows two bugs, each with a concrete counterexample:
- The double-double stage trusts |lo| as its error bound. Exactly collinear f64 points then get reported as CW or CCW, and inconsistently under permutation.
- The fast filter's bound for nested products is invalid. f64 incircle returns the opposite sign on non-degenerate near-cocircular input.

These came in with the January 2026 switch from geometry-predicates (a Shewchuk port) to apfp. apfp's tests miss them because they pin c=(0,0). Smaller errors: apfp's source is not a 404 (it lives at rgeometry/apfp), the early releases were dual-licensed, the commit counts are off, and macro-expanded unsafe code is uncounted.

**Effect on the verdict:** these errors strengthen REJECT, they do not overturn it. RGeometry fails Spare Kit's 'exact/robust predicates' requirement even for 2D f64, not just on scope. The 'what_to_take' item that presents apfp_signum! as a design reference should be downgraded to a cautionary example. The lessons for Spare Kit's predicate generator:
- Derive certified error bounds for every stage, as Shewchuk and Geogram PCK do. Never use the low word of a double-double as the bound.
- Test the predicates with exactly degenerate inputs whose coordinate differences are inexact.

The i8-coordinate property-testing idea is still worth taking.

### Challenge 2: FIT: argue the opposite verdict. The evaluation says REJECT,

**Refuted:** True

**Corrected verdict:** REJECT

**Errors found**

- WRONG KEY CLAIM: 'Predicate signs are exact for every supported scalar' (robustness section, strength #1, claim 6 evidence). For f64 this is false. apfp's double-double stage has no error bound. dd_signum returns +1 whenever hi > |lo|, which is true for every normalised nonzero double-double (https://github.com/rgeometry/apfp/blob/175ab9ac94d7713d36de38aee5659470cc23b390/src/analysis/adaptive_signum.rs#L104-L113). apfp_signum! returns that value straight away (#L1404-L1407), yet dd_mul is inexact: t = hi*lo + lo*hi is rounded (#L87-L94). An exact zero or a tiny determinant can therefore come back with a nonzero or wrong sign. I re-implemented apfp's stages operation-for-operation in Python (IEEE binary64, no FMA, same as Rust). It gives a nonzero sign for exactly cocircular rectangle corners, for example incircle((0.1,0.1),(0.3,0.1),(0.3,0.7),(0.1,0.7)) gives +1 where the exact answer is 0. That happened in 6,191 of 30,000 random cases. It also mis-signs near-collinear orient2d inputs with a magnitude spread above about 2^60, e.g. a=(4781121038414167.0, 8544709910879239.0), b=(1.8858197351673115e-15, 3.3702937975735738e-15), c=(298820064900885.44, 534044369429952.44) gives -1 where the exact answer is +1. rgeometry's f32/f64 cmp_slope and incircle call these functions directly (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/lib.rs#L667-L680, #L839-L858). Not yet run against the compiled crate: the results come from a faithful simulation.
- MISSED: apfp's fast f64 filter is also unsound for nested expressions. In _apfp_fast_mul_fold a product's magnitude bound is `abs = value.abs()`, the computed product, instead of the product of the operand bounds (https://github.com/rgeometry/apfp/blob/175ab9ac94d7713d36de38aee5659470cc23b390/src/analysis/adaptive_signum.rs#L884-L897). Cancellation inside cofactors such as incircle's is therefore not covered. Simulated example: incircle of (43173.56698465645, 28667.830178213742), (-47972.52629126492, 17392.471573365903), (-49564.774328724045, 12035.981936164644), (51845.3446085995, -611.2692635691369). The exact determinant is -54.38. The fast stage computes 7168 against an error bound of 5715 and returns +1. apfp's own tests miss both of these defects because the dd-stage case generator puts c at (0,0), which makes every difference, and so dd_mul, exact (https://github.com/rgeometry/apfp/blob/175ab9ac94d7713d36de38aee5659470cc23b390/tests/geometry_tests.rs#L422-L457).
- MISSED (more serious than the determinism finding): rgeometry's f64 constrained_delaunay can loop forever. It flips whenever incircle == Greater and has no iteration cap (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/algorithms/triangulation/constrained_delaunay.rs#L44-L118, #L89-L91). For cocircular quads the f64 predicate above returns +1 for both diagonals. Simulated example: the rectangle (53.59,36.57),(54.38,36.57),(54.38,42.71),(53.59,42.71) starting from triangles [0,1,2],[0,2,3] makes incircle(1,2,0,3), (0,1,3,2), (3,0,2,1) and (2,3,1,0) all return +1, so the flips cycle with period 4 for good. Across 2-decimal 'room' rectangles, 4,840 of 16,000 (rectangle × start) runs never terminate. The CDT tests use only BigRational (constrained_delaunay.rs#L333-L360), so CI cannot catch this. The evaluation's line 'CDT does not flip on cocircular configurations' is true only for exact scalar types.
- WRONG: 'Integers use apfp int_signum! with widening, so the predicates never overflow'. int_signum! evaluates every expression in a fixed 256-bit sign-magnitude BigInt<4>. bigint_mul_4_4_4 truncates ('may truncate') and add drops the carry (https://github.com/rgeometry/apfp/blob/175ab9ac94d7713d36de38aee5659470cc23b390/src/analysis/integer_signum.rs#L34, #L226-L285). The i64 incircle needs about 260 bits (https://github.com/rgeometry/apfp/blob/175ab9ac94d7713d36de38aee5659470cc23b390/src/geometry/i64.rs#L105). Simulated example: (8796166007532356747, 8421084910433602170), (-8426591825030515278, 8905907099900633129), (4702011651656458147, -7452785458314055208), (-8641818473904439699, -8296493021032679017) gives +1 where the exact answer is -1. That affects 4 in 20k random full-range i64 inputs, so it only happens near the extremes of the range. It contradicts the rgeometry doc's guarantee that 'intermediate calculations will not overflow' (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/lib.rs#L41-L44).
- MISSED: the integer constructions overflow at ordinary CAD magnitudes. The i32 approx_intersection_point widens only to i64 (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/lib.rs#L485-L497), and approx_intersection_point_basic forms part_a*(x3-x4), a degree-3 product (#L226-L248). Segments (0,0)-(3000000,3000001) and (0,3000000)-(3000000,0), which is 3 km in mm, give x_num = -2.7e19, beyond the i64 range. That panics in debug builds. In a dependent's release build it wraps silently, because rgeometry's `[profile.release] overflow-checks = true` (Cargo.toml) applies only when rgeometry is the workspace root. The function is used by Line::intersection_point (https://github.com/rgeometry/rgeometry/blob/6d2c21445ed0d7713c4553b368903392505f9cd0/src/data/line.rs#L46-L57), kernel::naive (src/algorithms/kernel/naive.rs#L129) and visibility::naive (src/algorithms/visibility/naive.rs#L194).
- WRONG: 'apfp's GitHub repository (github.com/lemmih/apfp) returns 404, so it can only be audited through docs.rs'. Only the crates.io repository field is stale. The source is public at https://github.com/rgeometry/apfp (created 2025-10-11, Unlicense, 0 stars, single author). Release 0.1.1 is commit 175ab9ac94d7713d36de38aee5659470cc23b390, and the repo includes CI scripts that check the generated asm for allocations and assertions (https://github.com/rgeometry/apfp/blob/175ab9ac94d7713d36de38aee5659470cc23b390/nix/check-no-allocations.sh).
- what_to_take #2 should be reversed. apfp's apfp_signum!/int_signum! design is not a sound 'design reference'. It is a second cautionary example: a staged filter needs a rigorous bound at every stage, including the double-double stage, and magnitude bounds must propagate as |a|·|b| through nested products. For a predicate generator Spare Kit should study Shewchuk, Geogram PCK and CGAL's static and dynamic filters. The one reusable idea is CI that checks generated predicate asm for no allocation and no panic paths.
- Minor (verified correct, no change): Unlicense/WAIVER, rug being LGPL and optional, SmallRng being Xoshiro256++ on 64-bit and Xoshiro128++ on 32-bit (rand 0.9.5 at 3474ec0, small.rs#L13-L35), HashMap RandomState seeding the CDT queue, Bentley-Ottmann's Vec status with a re-sort on every event versus its doc claim, the 5-segment [-5,5] proptest, 207 #[test] (200 in src + 7 in tests/two_opt.rs) and 48 active #[proptest], hole asserts, the issue states for #80/#117/#49/#84 (closed) and #92/#53 (open), 182 stars and 8 contributors including one anonymous, 21,253/184 downloads, and 0.12.0 released 2026-01-06.

**Argument:** The REJECT verdict stands, and the evidence makes it stronger. What I got wrong in the evaluation is the reasoning, not the verdict.

To argue the opposite verdict I needed a best-in-class component. The only real candidate was the one the evaluation itself praised most: the exact-predicate layer. It is a set of allocation-free, compile-time-sized macros in apfp that give a staged exact sign for any expression, with CI checks on the generated asm. If it were sound it would deserve STUDY as a template for Spare Kit's predicate generator.

Checked against apfp's actual source (github.com/rgeometry/apfp — not a 404), it is not sound:
- The double-double stage accepts any nonzero double-double result without an error bound.
- The fast filter's magnitude bound ignores cancellation inside nested products.
- The integer path truncates at 256 bits.

Faithful operation-for-operation simulations show the consequences:
- f64 incircle returns nonzero for exactly cocircular rectangles.
- The fast f64 incircle stage returns the wrong sign for ordinary near-cocircular points.
- orient2d gives the wrong sign at extreme magnitude spreads.
- i64 incircle gives the wrong sign near the range limits.
- RGeometry's f64 constrained Delaunay triangulation never terminates on about 30% of simple decimal 'room' rectangles, because both diagonals test as 'flip'. A non-terminating triangulator is worse than the cross-target nondeterminism the evaluation flagged.

On top of that, the i32 integer intersection construction overflows its i64 intermediate at about 3e6 coordinates, which is routine for site coordinates in mm. That breaks the crate's documented no-overflow guarantee and wraps silently in a dependent's release build.

So the evaluation was too generous in two places: its top strength ('every predicate exact for every scalar type') and its recommendation of apfp as a design reference. Everything else holds:
- Scope is about 1% of what Spare Kit needs: 2D, straight segments only, no curves, Booleans, topology or 3D.
- Bentley-Ottmann does not match its own documentation.
- It panics on holes.
- It is not bit-identical between native and wasm32.
- It has a single hobby maintainer.

Nothing here is best-in-class or meets the owner's 'best architecture' rule. The only things worth keeping are lessons: i8 property testing with naive cross-checks, CI asm checks for no allocation or panic in predicates, and two cautions. Kernel code must never use non-portable RNGs or HashMap iteration order, and every stage of an adaptive predicate filter must have a rigorous error bound.

Caveat: the apfp failures come from a line-by-line Python re-implementation of its arithmetic. I did not compile or run the crate (that needs a crates.io download I was not cleared to make), so confirming them on the compiled binary is still open.

## SolidCraft

**Repo url:** https://github.com/tangency-labs/solidcraft

**Licence:** MIT OR Apache-2.0, at the user's option. Root files LICENSE-MIT and LICENSE-APACHE (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/LICENSE-MIT, .../LICENSE-APACHE). The workspace sets `license = "MIT OR Apache-2.0"` at Cargo.toml L10 (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/Cargo.toml#L10). The vendored truck-geometry, truck-shapeops, truck-meshalgo, truck-stepio and matext4cgmath are Apache-2.0 (© RICOS) and egui-wgpu is MIT OR Apache-2.0 (vendor/README.md). Other dependencies are permissive: cadmpeg is Apache-2.0, spade is MIT OR Apache-2.0, tiny-skia is BSD-3. AGENTS.md bans reading GPL/LGPL code (OCCT, FreeCAD, SolveSpace). The licence is compatible with MPL-2.0, so porting would be legally allowed.

**Activity:** The first commit is dafce1f on 2026-10-07T17:01Z ("M0: workspace, geometry foundation, sketch constraint solver, kernel boundary"). The repo was created on 2026-10-07 and last pushed on 2026-10-20T22:47Z, so it was 14 days old when this was checked (2026-10-21). In that time it has 1,807 commits, about 137 contributors (bherbruck 780, jyviko 104, NOWN-DEV 92 …), 8,232 stars and 950 forks. It has 1,365 open issues (2,064 in total) and 384 open PRs. 607 issues labelled area:kernel are open, out of 653 ever filed. The latest release is v0.18.1 (2026-10-20); 0.16.1 through 0.18.1 shipped in three days. It is about 23 MB of Rust in 1,139 files, 4.47 MB of it in crates/kernel. Development is openly AI-agent-driven: ROADMAP.md says "≈ 82 agent hours remain", AGENTS.md drives the work, and issues are often machine-written (for example #2862). The companion kernel repo tangency-labs/solidcore was created on 2026-10-12, is Apache-2.0 and has 132 stars.

**Claims**

- **claim**: It is a 'major' parametric modeler in the Rust ecosystem; **verdict**: partly; **evidence**: It is major by visibility: 8,232 stars, 950 forks, about 137 contributors and 1,807 commits (gh api repos/tangency-labs/solidcraft). It is not major by maturity. It is 14 days old (first commit dafce1f, 2026-10-07). Its own README says 'Early but usable for simple parts' (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/README.md#L68). Its own robustness fuzz shows 27.1% of random 8-step designs failing, 0.9% ending as an invalid body and 0.2% with volume moving the wrong way (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/robustness.md#L157-L168). 607 kernel issues are open.
- **claim**: It is a parametric modeler (sketch, constraints, feature timeline); **verdict**: true; **evidence**: It has its own Levenberg–Marquardt sketch solver with a dense finite-difference Jacobian (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/sketch/src/solver.rs#L1-L13). A feature timeline with incremental evaluation lives in crates/doc (layer L2, https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/architecture.md). The README lists extrude, revolve, sweep, loft, fillet, shell, patterns, assemblies and sheet metal.
- **claim**: It is pure Rust, i.e. part of the Rust ecosystem; **verdict**: true; **evidence**: The workspace is all Rust crates with `unsafe_code = "forbid"` (Cargo.toml L104). It builds for wasm32 (browser app). The kernel crate root forbids unsafe and denies unwrap, expect and panic (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/lib.rs#L12-L13).
- **claim**: It is powered by Truck; **verdict**: partly; **evidence**: Today, yes. crates/kernel is the only crate that uses truck-modeling 0.6, truck-shapeops 0.4, truck-meshalgo 0.4, truck-stepio 0.3 and truck-topology 0.6 (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/Cargo.toml#L46-L52). `Body` wraps `Arc<truck_modeling::Solid>` (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/body.rs#L15-L60), and the general boolean is truck-shapeops (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/ops.rs#L1907-L1936). Three caveats. (1) It does not run stock truck: patched copies of truck-geometry, truck-shapeops, truck-meshalgo, truck-stepio and matext4cgmath replace the crates.io versions (Cargo.toml L146-L152; https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/vendor/README.md). (2) Much of the kernel is its own code because 'truck has no fillets, chamfers, shells or face edits' (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/book/src/dev/kernel.md). That covers blends, offset, its own STEP reader, its own tess/ module, and a planar BSP boolean plus 2D prism booleans that are tried before truck. (3) The owner has announced that truck will be replaced: 'The real fix is solidcore, the new B-rep kernel SolidCraft will move to' (issue #1597). SolidCore is a Rust port of NVIDIA SMLib (https://github.com/tangency-labs/solidcore/blob/6a019c57131d5fee6e57ab7dc4a62201a32e0216/README.md). See also issue #331.
- **claim**: The repo is github.com/bherbruck/solidcraft; **verdict**: partly; **evidence**: The GitHub API redirects that URL to tangency-labs/solidcraft. Cargo.toml still lists repository = github.com/bherbruck/solidcraft (L13).

**Architecture:** Layers (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/architecture.md):
- L0 geom: vectors, planes, profiles, meshes, and a kernel-independent 'native B-rep' persistence schema.
- L1 sketch (constraint solver), kernel (the B-rep boundary over truck) and render.
- L2 doc: parameters, expressions, timeline, persistent naming.
- L3 io.
- L4 engine: every action is a registered command.
- L5 gpu, ui-egui, mcp.
- Apps: desktop, CLI and web.

The kernel crate is 182 files, about 4.5 MB (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/lib.rs). It exports operations such as boolean, fillet/chamfer families, shell/offset, sweep/loft, surface tools (bridge, fair, fill, match, T-spline sheet), STEP/IGES I/O and measure. No truck type appears in its public API.

Topology and geometry are truck's. `Solid` is `truck_modeling::Solid`, i.e. truck_topology's Solid/Shell/Face/Wire/Edge/Vertex, using Arc-pointer identity, over truck_modeling's Curve and Surface enums. Those enums cover lines, B-spline and NURBS, revolved curves and surfaces, planes, and IntersectionCurve: two surfaces plus a polyline 'leader', refined pointwise by Newton projection.

`Body` (body.rs L21-L60) adds:
- `size`, the bounding-box diagonal: 'Every kernel tolerance is a fraction of it' (L23-L25, L1063-L1078).
- Cached volume and meshes, paint, split lines, face-source certificates.
- Exact operation history (face and edge lineage) for persistent naming.

Validity (body.rs L1653-L1674) means only: truck shell_condition == Closed, every face meshes, and the meshed volume is finite and positive. It does not check self-intersection or edge-on-surface consistency.

Hardest algorithm: the boolean. The core is truck-shapeops, vendored at https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/vendor/truck-shapeops/src/transversal/integrate/mod.rs#L91-L143. It works in four steps:
1. Triangulate both shells at tolerance `tol`.
2. Intersect the meshes to get polyline leaders and build IntersectionCurves.
3. Divide faces along the loops.
4. Classify unknown faces by ray-crossing counts against the triangle mesh.

Around it, `boolean_whole_with` is a roughly 600-line ladder of strategies (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/ops.rs#L1222-L1826). In order it tries:
1. A swept-prism shortcut.
2. An exact-in-2D prism boolean.
3. A mesh-sampled 'trivial' containment shortcut.
4. Coincident-face glue and imprint.
5. Bore union.
6. A 'push apart' of coplanar faces.
7. truck at tolerances size×2e-3 and size×5e-4.
8. The same with both operands shifted by JITTER×1% of size (L193, L1532-L1655).
9. Sliced and local planar booleans, sphere-lens and seam-face splitting.
10. A BSP polygon fallback.
11. A 'tool nudge' screw motion of 2e-6 rad and 2e-6×size, with three screw axes each tried both ways (L1685-L1772, SCREWS L1864-L1865).
12. Grown faces.
13. Feature-scale tolerances.
14. Finally an 'UnverifiedShortcut' that returns the mesh-sampled answer (L1817-L1825).

The outer `boolean` (L919-L949) adds per-lump retries, voids, reseam and tangent-cylinder splitting. `boolean_via` (L965-L1035) applies the retry post-check. Every truck call runs inside `guard`, a catch_unwind (lib.rs L251-L271).

Sweeps, lofts and blends are SolidCraft's own code: blend.rs (159 KB), blend_corner, curveblend, sweep_path and others. They build truck faces through truck_modeling builders.

docs/native-brep.md defines a schema-3 persistence model that does not depend on truck: analytic cylinder, cone, sphere and torus records, splines, rational splines, procedural surface_intersection, and resource caps. The 'truck adapter writes it exactly' (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/native-brep.md).

**Robustness:** Arithmetic is plain f64 everywhere. The repo has no exact or adaptive predicates: a code search finds no orient2d or orient3d. Tolerances scale with the bounding-box diagonal, for example tol_m = size×5e-4 clamped by meeting size (ops.rs L1239) and truck tolerances size×2e-3 and size×5e-4 (L1603-L1604).

The 'exact planar' fallback is not exact arithmetic. It is a csg.js-style BSP on f64 planes, retried at eps = size×{1e-8, 1e-7, 3e-8, 1e-9, 3e-7} (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/polybool.rs#L1-L4 and L380-L382).

A result is accepted on approximate checks:
- Meshed-volume bounds with 0.2% slack (`plausible`, ops.rs L1158-L1165).
- Ray-parity membership at 1,200 Halton points, with a mismatch below 0.4% allowed (L214-L248, L1326).
- When a retry was used, a mesh-distance check that nothing changed outside the tool's box (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/boolean_guard.rs#L1-L11).

Degenerate cases are handled by heuristics: coplanar faces are pushed apart on temporary geometry, the tool is nudged, both operands are shifted, tolerances are refined, and seams and tangent cylinders are split. Nudged and unverified results are returned with a timeline warning, not refused (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/robustness.md#L95-L140).

Its own fuzz figures (docs/robustness.md L157-L168): 27.1% of designs failed, 0.9% gave an invalid body, 0.2% moved volume the wrong way and 18% exceeded 90 s. The boolean volume identity held in only 80.8%.

Known failures:
- #1649 (open): a union of two crossing cylinders loses about 9% of its volume (2955.58 → 2694.71 mm³) when both are moved by about 1.2 km. That is fatal for BIM site coordinates. It is an ignored test at https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/tests/boolean_props.rs#L333-L343.
- #2862 and #2357: truck-geometry panics at intersection_curve.rs:133, then 'Union: no result'; #2357 is refused after 21 minutes.
- #1559: truck's `unimplemented!` 'intersection curve cannot connect by homotopy'.
- #1983: booleans stop terminating after a faceted fallback.
- #2947, #2945: the boolean gives up on seams and small B-spline faces.

vendor/README.md lists dozens of patched truck defects:
- Second derivatives of NURBS surfaces of degree 3 or more were wrong by a factor of 1/(p−1) (L79).
- Face order came out in hash order (L18).
- Panics inside IntersectionCurve::subs (L135).
- Endless loops in meshing (L150) and in Newton root refinement (L248).
- The sign of a zero-area loop was random.
- Debug assertions reject valid geometry, so CI turns them off (Cargo.toml L121-L128).

Tests are many but shallow at the black-box level:
- Unit and regression tests named after issues.
- A 75/76 oracle of reference parts.
- 96 seeded robustness goldens.
- proptest invariants for commutativity, translation and scale invariance, and determinism. These are skipped on PR and main CI because 'a known kernel bug would turn main red at random' (boolean_props.rs L7-L13).
- A mutation-testing pilot whose black-box suites caught 0/19 tessellation mutants, 0/14 offset/shell mutants and 10/44 boolean mutants (#2662, #2666, #2665).

**Determinism:** Determinism holds within a process only, after patches. Native and WebAssembly results are not bit-identical, and results differ across platforms.

Within a run:
- truck's connected components came out in hash order of face addresses. SolidCraft patched this to keep shell order (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/vendor/truck-shapeops/src/transversal/integrate/mod.rs#L224-L240).
- #707 (closed) recorded a hole cut that 'cuts or fails from run to run'.
- A proptest checks only that the same input gives the same volume bits twice in one process (boolean_props.rs L224-L231).
- An attempt budget makes 'gives up' behaviour the same on every machine (ops.rs L881-L911).

Across platforms, #924 (open) found:
- 16 of 96 robustness seeds build differently on Windows, including ok↔failed outcome flips and topology changes.
- More differences on macOS arm64, and on some Linux hosts even with the CI-built binaries.
- The cause was traced to a one-ULP difference in the system libm `hypot` (21.000000000000004 vs 21.000000000000007). That flipped a later corner cut onto a different topological path, leaving a boundary about 8 mm off its surface.

The goldens now carry per-OS override records, and docs/robustness.md L65-L70 says they 'do not certify cross-platform geometric equivalence'. CI runs Linux only. The kernel uses platform libm through std f64 functions, rayon in parallel natively (serial on wasm), and truck's Arc-pointer IDs. I found no test comparing native against wasm output, so native/wasm equivalence is unverified and very likely fails given the libm sensitivity.

**Strengths**

- vendor/README.md is the most detailed public catalogue anywhere of how truck fails in production use: mesh-based SSI, seams, projection divergence, NURBS derivative bugs, hash-order nondeterminism, panics and endless loops. Each item is tied to an issue and a regression test. It is first-hand evidence for the Spare Kit decision not to build on truck.
- A strong robustness methodology: a seeded random design generator that snaps placements to a 2.5 mm grid so coincidences and tangencies are common. Each seed runs in its own process with a timeout and is minimised with `fuzz-one --minimise`. It checks the boolean volume identities V(A∪B)+V(A∩B)=V(A)+V(B) and V(A−B)=V(A)−V(A∩B), and keeps per-seed topology and measure goldens that fail on any change (docs/robustness.md).
- Property-test invariants worth copying: commutativity, translation invariance with offsets of 1e3 and 1e5, scale invariance, self-operations, and repeat determinism (crates/kernel/tests/boolean_props.rs).
- An independent post-condition check on boolean results: the result is closed, the volume moves the right way, and the surface outside the tool's box is unchanged (crates/kernel/src/boolean_guard.rs). It is a useful idea for a validation layer that does not trust the algorithm.
- Face and edge lineage recorded inside the boolean as it creates pieces (vendor truck-shapeops transversal/history.rs, crates/kernel/src/history.rs) feeds exact persistent naming instead of naming faces geometrically after the fact.
- A persistence schema independent of any kernel (docs/native-brep.md): versioned records, exact binary64 round-trip, explicit resource caps, and a clean split between validation and trust.
- Engineering hygiene: no panics in production code, an unsafe ban, crash reports with replay, mutation-testing pilot, and hostile-input caps.
- The licence (MIT OR Apache-2.0) is fully compatible with MPL-2.0.

**Weaknesses**

- It is not a kernel. It is a 14-day-old, AI-agent-built CAD application whose kernel layer is a wrapper around a patched truck, and its owner has declared that wrapper slated for replacement by solidcore (#1597, #331).
- Booleans are mesh-based. truck-shapeops triangulates both shells, traces intersection curves from mesh polylines, and classifies faces by ray parity against triangles. This is the opposite of exact-predicate B-rep booleans.
- Robustness comes from a heuristic retry ladder: shifted operands, refined tolerances, faces pushed apart, the tool nudged by 2e-6, BSP at five epsilons, and a final 'UnverifiedShortcut'. Nudged and unverified results can be returned with only a warning. That breaks the Spare Kit rule 'booleans never silently fail'.
- No exact or robust predicates. Tolerances are fractions of the bounding-box diagonal and acceptance uses meshed volumes with 0.2% slack, so the results are not position-invariant: #1649 loses 9% of a volume about 1.2 km from the origin.
- Its own fuzzing gives a 27% design failure rate, 0.9% invalid bodies, and a boolean identity that holds in only 80.8% of cases. 607 kernel issues are open, and truck panics and hangs are still reported (#2862, #2357, #1983, #1559).
- Results are not deterministic across platforms (#924): a one-ULP libm difference flips topology. Goldens need per-OS overrides, and native/wasm bit-identity is neither tested nor achievable as built.
- Validity is weak: closed shells, faces that mesh and positive volume. There is no self-intersection or geometric-consistency check.
- Test depth is low at the black-box level: mutation testing shows 0 of 19 tessellation mutants caught, and property tests are kept out of PR CI because of known failures.
- The pace (1,807 commits and 23 MB of Rust in two weeks, written mainly by agents) makes it hard for people to read. Large files reach 125–180 KB (ops.rs, prism.rs, blend.rs).

**Verdict:** STUDY

**Verdict reason:** Judged against the absolute best architecture, SolidCraft's kernel is a cautionary example, not a foundation. It wraps truck's mesh-based boolean in f64 heuristics, nudges and volume-plausibility checks, and can return unverified results. It is not deterministic across platforms, and its own owner is replacing it. That rules out DEPEND. It is also an application, not a library. PORT is also ruled out, because none of its core methods (retry ladder, BSP epsilon fallback, mesh membership tests) belong in an iron-clad kernel, even though the licence would allow it. It is still worth studying for three things: the most complete real-world record of truck's failure modes, a mature robustness and fuzzing methodology with property invariants, and its ideas for a validation layer, boolean lineage and kernel-independent persistence. The more relevant object for Spare Kit is SolidCore and its upstream, NVIDIA usd-brep/SMLib (Apache-2.0). That should be evaluated as its own piece.

**What to take**

- Truck failure catalogue, as evidence to reject truck and as a test-case list: https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/vendor/README.md (shapeops L7-L58, geometry L60-L80, meshalgo L102-L240, matext4cgmath L242-L252).
- Robustness fuzz design: grid-snapped random designs, process-per-seed timeouts, minimiser, boolean volume identities, seeded goldens that fail on any change. See https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/robustness.md and crates/kernel/src/fuzz.rs at the same SHA.
- Property invariants to adopt as hard gates. Spare Kit should require these to pass, not skip them: commutativity, translation invariance at offsets of 1e3 and 1e5, scale invariance, self-ops, repeat determinism. See https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/tests/boolean_props.rs#L155-L265.
- Independent post-condition idea, re-implemented with exact predicates instead of meshes: closed result, monotone volume, nothing changed outside the tool's region. See https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/boolean_guard.rs#L1-L11.
- Lineage recorded during the boolean (face piece to operand face, edge to operand edge or face pair) for persistent naming. See https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/vendor/truck-shapeops/src/transversal/history.rs and docs/book/src/dev/naming.md.
- Persistence schema principles: versioned geometry records separate from runtime types, exact f64 round-trip, resource caps, cache treated as provisional until re-evaluated. See https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/native-brep.md.
- Cross-platform determinism lesson: one ULP from libm `hypot` flipped topology. Spare Kit must use its own deterministic math (no platform libm) and exact predicates. See issue #924: https://github.com/tangency-labs/solidcraft/issues/924.
- Anti-patterns to avoid, as documented: the retry and nudge ladder at https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/ops.rs#L1222-L1826, and mesh-ray face classification at https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/vendor/truck-shapeops/src/transversal/integrate/mod.rs#L91-L143.
- Follow-up lead, not part of this piece: evaluate SolidCore (https://github.com/tangency-labs/solidcore, Apache-2.0, a port of NVIDIA SMLib with C++ difftest; its README says it is about a third ported and nothing works end to end yet) and the upstream NVIDIA-Omniverse/usd-brep SMLib directly.

### Challenge 1: FACTS: I re-checked the licence, the activity numbers, each 

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- #1649 offset is wrong by a factor of 1,000. The evaluation says 'about 1.2 km'. The move is (1000, -700, 300) in mm, which is about 1.26 m. The test says 'moved 1000 mm away' (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/tests/boolean_props.rs#L333-L344), and the issue is titled 'loses volume when both are moved 1000 mm' (https://github.com/tangency-labs/solidcraft/issues/1649). The correction makes the finding worse for SolidCraft: a 9% volume loss happens at about 1 m from the origin.
- The fuzz figures are presented as current, but they are stale. The 27.1% failed, 0.9% invalid, 0.2% wrong-way and 18% over 90 s numbers come from commit 1bf377c1, authored 2026-10-08 (project day 2), and have not been updated since, about 1,700 commits ago. The doc itself says 'Both figures are from before the last two fixes below' and that the run was on 'a heavily loaded machine (load average 60–80)', where timeouts are 'mostly a design whose booleans are very slow ... not a hang' (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/robustness.md#L150-L155). The evaluation uses them as current evidence in claim 1, in robustness and in weaknesses.
- 'The boolean volume identity held in only 80.8%' misreads the table. The rows are holds 80.8%, a boolean failed 1.2%, and not run (design timed out) 18.0%. They sum to 100%, so no case had a violated identity. Of the cases actually run, it held in about 98.5% (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/robustness.md#L165-L169).
- The five-epsilon BSP retry is attributed to the wrong function. The retry at eps = size×{1e-8, 1e-7, 3e-8, 1e-9, 3e-7} is in `faceted_boolean`, which takes a mesh tool such as a thread groove (polybool.rs L345-L420, loop at L380-L382). The 'exact planar' fallback `planar_boolean`, which the boolean ladder calls, runs the BSP once at eps = size×1e-9 (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/polybool.rs#L438-L457). So 'BSP at five epsilons' in the ladder weakness is inaccurate. The underlying criticism, f64 epsilon BSP rather than exact arithmetic, still stands.
- Overstated: 'Nudged and unverified results are returned with a timeline warning, not refused' and 'booleans ... silently fail'. For join and cut, any result that needed a retry (nudge or UnverifiedShortcut) must pass boolean_guard::check_retry or it is refused with an error (ops.rs L1019-L1030; robustness.md L129-L136). It is also not silent, because the timeline shows a warning (robustness.md L95-L98). The real gaps are different. check_retry returns Ok immediately for Intersect and for mesh bodies (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/boolean_guard.rs#L137-L140), so an unverified intersect is blocked only by the #3244 deep-crossing test (ops.rs L1802-L1825). And accepted results are approximate: they can be moved by 2e-6×size.
- Missed: the 1,200-point ray-parity membership check is skipped entirely when the operand and result meshes together have more than 60,000 triangles. In that case `mismatch` returns None, which `is_none_or` accepts (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/ops.rs#L214-L217 and L1325-L1327).
- #924 cause is overgeneralised. Only one case was traced to the one-ULP libm `hypot` difference: a single Linux host's issue_851 and seed-13 failures, in a comment by IngoMaindorfer on 2026-10-15. That case was later fixed by #1294. The original Windows 16/96 divergence is still 'Likely cause (not confirmed)', and commenters say the matches 'do not establish a common root cause' (https://github.com/tangency-labs/solidcraft/issues/924). There is also newer evidence the evaluation does not give: a 2026-10-18 comment reports Linux main CI itself failing goldens for seeds 13, 67 and 94.
- Missed point relevant to the WebAssembly requirement: `guard` (catch_unwind) protects only native builds. 'Natively a panic inside the kernel is caught, but the web build aborts on any panic' (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/robustness.md#L199-L203). The vendored truck-shapeops says the same: 'no panics (wasm can't catch them)' (vendor/truck-shapeops/src/transversal/integrate/mod.rs L119, L131). truck panics are still reported (#2862, #2357). So the strength 'no panics in production code' is overstated.
- Minor: 'no exact or adaptive predicates' holds only for SolidCraft's own code. The kernel's face triangulation uses spade's constrained Delaunay triangulation (crates/kernel/src/tess/cdt.rs L13-L14), and spade depends on the `robust` crate, which provides Shewchuk adaptive orient2d/incircle (Cargo.lock L4408-L4417). These predicates are not used in any boolean or topology decision, so the conclusion stands.
- Minor ladder-order errors. The sliced and local planar, sphere-lens and seam-face strategies run at attempt==1, before the shifted truck tries in that iteration (ops.rs L1532-L1605), not after them. Imprint union (L1461-L1473) runs after push-apart (L1408-L1448), not as step 4.
- Minor: the claim 'pure Rust' (verdict true) is true for the kernel crate. The native app is not pure Rust: it links the C library zstd through cadmpeg (Cargo.toml L70 says 'pure Rust apart from zstd; native builds only'; Cargo.lock has zstd-sys at L6304). Also, 'CI runs Linux only' is slightly off. Windows jobs exist for shaders and a startup smoke test (ci.yml L643-L697), but all kernel tests and goldens run on Linux. The wasm job only compiles and is triggered manually (ci.yml L783-L797).

**Argument:** These all check out against source: the licence (MIT OR Apache-2.0 at Cargo.toml L10, both licence files present, vendored truck crates under Apache-2.0) and every activity number. Activity: 1,807 commits; first commit dafce1f on 2026-10-07; 137 contributors (bherbruck 780); 2,064 issues, 1,365 open; 383–384 open PRs; 653/607 area:kernel issues; v0.18.1 on 2026-10-20; 1,139 .rs files totalling 23.09 MB, of which crates/kernel is 182 files and 4.47 MB. All cited issue titles and states, the vendor/README.md defect lines, the body.rs validity definition, the truck-shapeops pipeline (mesh, then loops, then divide, then ray-parity classification against triangles), the JITTER, SCREWS and UnverifiedShortcut code, the plausibility slack of 0.2%, and the property-test CI exclusion also check out. The five claim verdicts all hold.

The errors I found are in the evidence, not in the verdicts, and they cut both ways:
- More damning than stated: #1649 fails at about 1.26 m, not 1.2 km; the guard does nothing in WebAssembly, where truck panics abort; membership checks are skipped on large meshes; the retry post-check never runs for Intersect.
- Less damning than stated: the 27% failure and 80.8% identity figures are day-2 snapshots, and the identity figure is misread (no identity violation is recorded); join and cut results from retries are checked and refused if wrong, not returned silently; the 'exact planar' BSP uses one epsilon, not five.

None of this changes the decisive facts for Spare Kit:
- The booleans are mesh-based truck-shapeops wrapped in f64 tolerance heuristics, with no exact predicates.
- Results differ across platforms (#924, including Linux CI itself) and native/WebAssembly equivalence is untested.
- Validity is limited to closed shells, faces that mesh and positive volume.
- The owner is replacing the kernel with solidcore (#1597).

DEPEND and PORT stay ruled out. REJECT would be too harsh, because the truck failure catalogue, the fuzz and property-test methodology, the retry post-check idea, boolean lineage and the kernel-independent persistence schema are worth studying. STUDY stands. The evaluation should fix the 1.2 km figure, date the fuzz statistics, and restate the retry and unverified-result behaviour precisely.

### Challenge 2: FIT, arguing against the STUDY verdict. I looked for any Sol

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- Wrong distance in #1649: the evaluation says 'about 1.2 km' twice. The real move is (1000, -700, 300) mm, about 1.26 m. Units are mm, and the test comment says 'moved 1000 mm away' (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/tests/boolean_props.rs#L333-L344; issue #1649 body). The defect is about 1000x worse for BIM site coordinates than stated. The property test's offsets of 1e3 and 1e5 are 1 m and 100 m (boolean_props.rs#L187-L201).
- The #924 root cause is overstated. The issue body says 'Likely cause (not confirmed)' for the 16 Windows seeds. The hypot one-ULP isolation is in IngoMaindorfer's comment of 2026-10-15, and it covers something narrower: a different Linux host running CI-built binaries, the old issue_851 test and the seed-13 shard. The same comment says 'These matches do not establish a common root cause' and that #1294 fixed that case. The Windows override records now cover 5 seeds, not 16 (platforms.windows in https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/tests/robustness/goldens.json). The conclusion still holds: libm-sensitive results are not deterministic across platforms.
- The fuzz figures are presented as current, but they are stale. The results table (docs/robustness.md L148-L170) was last changed by commit 1bf377c1 on 2026-10-10. It says 'Both figures are from before the last two fixes' (L154-L155), and the run was on a machine with load average 60-80 (L150-L152). The seeded goldens at the pinned SHA show 83 of 96 seeds ok and 13 failed: 8 shell offset-corner errors, 4 boolean failures, 1 lumps error (tests/robustness/goldens.json). That is about 13.5% failing, not 27.1%. It is still disqualifying for an iron-clad kernel.
- The oracle figure is outdated. The evaluation quotes '75/76' from README L68-L70. At the same SHA, the generated docs/oracle.md reports '83 / 83 cases pass; sources: 62 analytic, 21 freecad', and tests/oracle holds 83 recipe cases (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/docs/oracle.md).
- Missed: the retry guard has exemptions. check_retry returns Ok at once for every Intersect and for any mesh body (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/boolean_guard.rs#L137-L140). For an empty result it also returns Ok when meshing fails (L145). Intersections that came from ToolNudge or UnverifiedShortcut are therefore never post-checked. That backs 'booleans can silently fail' more strongly than the evaluation says. In the other direction, 'nudged and unverified results are returned with only a warning' leaves out that union and cut retries must pass check_retry or are refused (ops.rs L1019-L1030; robustness.md L129-L136). #2357 shows a 21-minute join refused this way.
- 'No panics in production code' is wrong as stated, and the guard does not protect WebAssembly. guard is catch_unwind (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/kernel/src/lib.rs#L251-L271). docs/robustness.md L199-L203 says 'the web build aborts on any panic'. The deny(unwrap/panic) lints at lib.rs L12 cover only SolidCraft's own crate: vendor/ is excluded from the workspace and its lints (Cargo.toml L4-L5). Open issues #2862 (unwrap at vendored truck-geometry intersection_curve.rs:133, panicking 6 times) and #1559 (truck unimplemented!) show the vendored code still panics.
- 'Arithmetic is plain f64 everywhere; no adaptive predicates' is slightly overstated. The trimmed-face tessellator uses spade 2.15.1 (Cargo.toml L53-L55; crates/kernel/src/tess/cdt.rs). spade depends on robust 1.2.0, which provides Shewchuk's adaptive orient2d and incircle (Cargo.lock L4408-L4417 and L3597-L3600). The boolean and classification path does use plain f64 only, so the core point stands.
- 'Determinism holds within a process only' is too strict. The per-seed goldens record exact face and edge counts and are checked in separate processes across CI shards on Linux, and they pass (docs/robustness.md L39-L49). Repeatability holds on one platform with one libm. What fails is cross-platform and cross-libm determinism.
- Missed in the architecture: L0 crates/geom does not depend on truck. Its Cargo.toml lists only serde and thiserror. It has its own NURBS code (bspline.rs) and spline spaces with exact knot insertion and degree elevation (spline_space.rs). It has Gordon and Coons network surfaces with G1/G2 cross-boundary conditions (https://github.com/tangency-labs/solidcraft/blob/090119a5c56ed7b5dc1e2a591b7b91a7140a2dc9/crates/geom/src/network.rs#L1-L25), loop fill, T-splines (tspline/) and a BVH. The kernel also has Gram-matrix fairing (crates/kernel/src/fair.rs). So 'Topology and geometry are truck's' is true of the B-rep only. These are the strongest PORT candidates, but they are textbook Piegl–Tiller and Sederberg implementations. They use f64 and dense solvers (MAX_SPACE_DIMENSION 2048). Their scope is narrow: T-splines take odd degrees and rectangular index domains only (tspline/mod.rs L12-L13), and fill does height fields only (fill.rs L1-L5). They are 14 days old. They are not best-in-class next to SMLib.
- Minor misplacements. The per-lump and void retries are in boolean_via (ops.rs L966-L992), not in boolean (L919-L949). The random design generator is crates/engine/src/fuzz.rs (robustness.md L18); crates/kernel/src/fuzz.rs only holds the boolean-identity checks.
- Missed take-aways, all kernel-agnostic and MIT/Apache-2.0. (1) The oracle corpus: 83 recipe cases, 62 of them with closed-form expected values and formula scripts in tools/oracle-analytic. (2) The attempt budget (ops.rs L881-L911), which gives up after a fixed count of tries instead of on a clock, so give-up behaviour is the same on every machine and in wasm. (3) The mutation-testing rule: mutants are judged by black-box suites only, so the tests survive a kernel swap (docs/mutation-testing.md L1-L20).

**Argument:** I tried to make the case for PORT and could not. The activity, licence, size and file counts are all accurate: 1,807 commits; 138 contributors including anonymous; 1,365 open and 2,064 total issues; 607 of 653 kernel issues open; 1,139 .rs files totalling 23.09 MB; 182 kernel files totalling 4.47 MB; MIT OR Apache-2.0. The five claim verdicts also stand. The boolean ladder description in ops.rs L1222-L1826 is accurate.

I then looked for any part good enough to port.

- **Persistent naming and history** (crates/kernel/src/history.rs L1-L120; docs/book/src/dev/naming.md) is the best design idea. Its data model is kernel-agnostic: FaceOrigin, Change, EdgeOrigin::Meet, composition by substitution, pieces told apart by their neighbours, and an explicit 'unverified' flag. But it is small, close to the familiar Modified/Generated/Deleted history semantics, missing on many operations, and easy to rebuild from the document. That makes it STUDY, not PORT.
- **crates/geom surface construction** (Gordon, fairing, fill, T-splines) is clean, panic-free and well cited. It is textbook-level, f64 only, narrow in scope and unproven, so it falls short of the 'absolute best' bar.
- **The tessellator** is a sensible design, but black-box tests caught 0 of 19 tessellation mutants (#2662).
- **The sketch solver** uses Levenberg–Marquardt on a dense finite-difference Jacobian, capped at 4,000 variables per component (solver.rs L1-L13, L35-L40).
- **The STEP reader** cannot read offset surfaces or pcurve-only edges (docs/step-import.md).
- **2D drafting** is only an early structural data type (docs/drawing-geometry.md, W01a).
- **Blends** do not support fillets between two curved faces (robustness.md L230).

The core kernel is disqualified on stronger evidence than the evaluation gave:

- Booleans are mesh-based and use f64 heuristics.
- The retry guard skips every intersection (boolean_guard.rs L137-L140).
- catch_unwind cannot catch the vendored truck panics in WebAssembly, where they abort.
- A crossing-cylinder union loses 9% of its volume about 1.26 m from the origin, not 1.2 km.
- Results are not deterministic across platforms or libm versions.

The factual errors I found are in the supporting evidence. Some make SolidCraft look better than it is (1.26 m, not 1.2 km; the guard exemptions; panics in wasm). Others make it look worse (fuzz figures 11 days stale, with current goldens at 13.5% failing; oracle at 83/83 rather than 75/76). None changes the verdict. STUDY is correct: the robustness methodology, oracle corpus, naming model, persistence schema and truck failure catalogue are worth learning from, and none of the code belongs in Spare Kit.
