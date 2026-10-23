*Evidence for [G01](../G01-geometry-kernel.md), the Spare Kit geometry kernel decision note. Written 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of each project at the pinned commits linked below. GPL and LGPL code is described in the agent's own words, never copied. Claims it could not confirm from a primary source are marked unverified.*

Spare Kit kernel: Rust kernels and crates (evidence report, 2026-10-21)

Pinned commits: Truck `78d699b0`, Fornjot `7e856686` (archived), curvo `11842793`, csgrs `1bd74092`, monstertruck `d2ad1ed7`, solidcore `6a019c57`. A licence is taken from the repo's LICENSE file where I link one. Otherwise it comes from the crate's Cargo manifest on crates.io and is marked "(manifest)". Text from GPL/LGPL sources is paraphrased.

Two findings outside this brief may change the decision:
- **NVIDIA published SMLib** (a full NURBS B-rep solid kernel, C++/Python) as [USD BRep](https://github.com/NVIDIA-Omniverse/usd-brep/tree/c9c5979f04cea8c38213004d4f04df0dcfb40d05) on 2026-10-08 under Apache-2.0 ([LICENSE](https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/LICENSE)).
- **A Rust port of SMLib, solidcore,** started on 2026-10-12 (section 3).

## 1. Truck (ricosjp/truck)

**Licence and status.** Licence is Apache-2.0 ([LICENSE](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/LICENSE)). Master is active (last push 2026-10-19), but crates.io lags behind:
- [truck-modeling](https://crates.io/crates/truck-modeling) 0.6.0 was last published 2024-09-20.
- [Issue #135](https://github.com/ricosjp/truck/issues/135) says a boolean panic and a hang are fixed only on master.

**Crate layering** ([workspace](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/Cargo.toml#L1-L20)):
- **truck-base:** a cgmath re-export, tolerance, bounding box, Newton solver and a hash. It depends on cgmath 0.18 ([manifest](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-base/Cargo.toml)), whose last release was 2021-01-03 ([crates.io](https://crates.io/crates/cgmath)).
- **truck-geotrait:** the curve and surface traits.
- **truck-geometry:** knot vectors, B-spline/NURBS, analytic types (line, circle, plane, sphere, torus), and "decorators": a transform wrapper, revolved and extruded curves, `IntersectionCurve`, `PCurve`, offsets and rolling-ball fillet surfaces. It depends only on base and geotrait ([manifest](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-geometry/Cargo.toml)).
- **truck-topology:** topology types, generic over point, curve and surface types.
- **truck-polymesh and truck-meshalgo:** meshes. rayon is enabled only off wasm32, and spade is optional ([manifest](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-meshalgo/Cargo.toml)).
- **truck-modeling:** concrete geometry enums and sweep builders.
- **truck-shapeops:** booleans, healing and fillets. It depends on truck-meshalgo ([manifest](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/Cargo.toml)).
- **truck-stepio:** STEP via [ruststep](https://github.com/ricosjp/ruststep) (Apache-2.0).
- **The rest:** truck-assembly; truck-drafting, a new 2D crate ([lib.rs](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-drafting/src/lib.rs#L1)); truck-platform and truck-rendimpl (wgpu); truck-js.

**Trait design.**
- `ParametricCurve` has associated `Point`/`Vector` types and `subs(t)`, `der`, `der_n`, `ders`, a `Bound`-based `parameter_range` and `period`. `BoundedCurve` adds `front`/`back` ([curve.rs L11-57](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-geotrait/src/traits/curve.rs#L11-L57)).
- `ParametricSurface` has `subs(u,v)`, first and second partial derivatives, `der_mn` and u/v periods ([surface.rs L5-52](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-geotrait/src/traits/surface.rs#L5-L52)).
- Inversion is `SearchParameter<D1|D2>`, with hints and a trial count ([search_parameter.rs](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-geotrait/src/traits/search_parameter.rs#L2-L33)).
- Concrete geometry is closed enums with derive macros. `Curve` is Line, BSplineCurve, NurbsCurve, or an IntersectionCurve of boxed curve and surfaces ([geometry.rs L9-33](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-modeling/src/geometry.rs#L9-L33)).

**Topology representation.**
- `Vertex{point: Arc<Mutex<P>>}`, `Edge{vertices, orientation, curve: Arc<Mutex<C>>}`, `Wire = VecDeque<Edge>`, `Face{boundaries, orientation, surface: Arc<Mutex<S>>}`, `Shell = Vec<Face>`, `Solid{Vec<Shell>}` ([lib.rs L108-176](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-topology/src/lib.rs#L108-L176)).
- It uses parking_lot `Mutex` and, by default, `rclite::Arc` ([L84-94](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-topology/src/lib.rs#L84-L94)).
- Identity is the Mutex's address. `ID::new(ptr)` stores `ptr as usize` ([id.rs L5-12](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-base/src/id.rs#L5-L12)), and VertexID, EdgeID and FaceID are `ID<Mutex<_>>` ([L221-253](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-topology/src/lib.rs#L221-L253)).
- **Consequences:**
  - Geometry is mutable in place through shared handles.
  - Hash and ordering of IDs depend on the allocator, so any observable map order is not reproducible.
  - There are no persistent snapshots.
  - Faces store no pcurves, so tessellation recovers (u,v) by Newton search with 100 trials ([triangulation.rs L24-47](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-meshalgo/src/tessellation/triangulation.rs#L24-L47)).

**Validation is combinatorial only.**
- `Solid::try_new` checks that each shell is non-empty, connected, closed and has no singular vertices. `Solid::new` panics on failure ([solid.rs L10-31](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-topology/src/solid.rs#L10-L31)).
- The topology errors have no geometric checks such as vertex-on-curve, curve-on-surface or self-intersection ([errors.rs](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-topology/src/errors.rs)).

**How booleans work** ([integrate/mod.rs L75-132](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/src/transversal/integrate/mod.rs#L75-L132)):
1. Triangulate both shells at the tolerance.
2. Intersect the meshes to get polyline loops.
3. Refine each polyline point by projecting it onto both surfaces, up to 100 iterations ([intersection_curve/mod.rs L26-51](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/src/transversal/intersection_curve/mod.rs#L26-L51)).
4. Divide the faces and classify them.
5. Classify any remaining "unknown" faces by ray-crossing parity against the other solid's triangle mesh, along a direction derived from a hash.
6. Replace result edges with quadratic B-spline approximations ([L55-73](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/src/transversal/integrate/mod.rs#L55-L73)).

**Boolean robustness limits.**
- **Scope:** transversal intersections only; tangent faces are unsupported ([shapeops lib.rs L1-14](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/src/lib.rs#L1-L14)).
- **Grid snapping:** polyline chaining snaps points to an integer grid of 2×TOLERANCE, so near-equal points on either side of a cell boundary will not merge ([polyline_construction L26-35](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/src/transversal/polyline_construction/mod.rs#L26-L35)).
- **No diagnostics, and a panic:** `and`/`or` return `Option`, then call `Solid::new`, which panics if the result is invalid ([L134-155](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/src/transversal/integrate/mod.rs#L134-L155)).
- **Open issues:** [#114](https://github.com/ricosjp/truck/issues/114) coincident geometry, [#57](https://github.com/ricosjp/truck/issues/57) touching solids, [#68](https://github.com/ricosjp/truck/issues/68) slow, [#135](https://github.com/ricosjp/truck/issues/135) panic and hang.

**Tolerance handling.**
- There is one global absolute `TOLERANCE = 1e-6` (and its square, 1e-12). `near` is cgmath's per-component `abs_diff_eq` ([tolerance.rs L5-20](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-base/src/tolerance.rs#L5-L20)).
- Operations assert `tol >= 1e-6` and panic otherwise ([geotrait lib.rs L19-30](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-geotrait/src/lib.rs#L19-L30)). STEP inputs reach that panic ([#123](https://github.com/ricosjp/truck/issues/123), [#128](https://github.com/ricosjp/truck/issues/128)).

**Determinism.**
- There is a seed-free float hash that canonicalises ±0 and NaN ([hash.rs L3-43](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-base/src/hash.rs#L3-L43)).
- But circles use std `f64::sin`/`cos`, i.e. the platform libm ([circle.rs L16-19](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-geometry/src/specifieds/circle.rs#L16-L19)).
- So native and wasm results are not bit-identical.

**STEP I/O.**
- Input goes through ruststep into a `Table` of std `HashMap`s ([in/mod.rs L24-40](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-stepio/src/in/mod.rs#L24-L40)).
- The crate docs say boolean results cannot be exported yet ([lib.rs L1-7](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-stepio/src/lib.rs#L1-L7)); this may be stale.
- [#133](https://github.com/ricosjp/truck/issues/133): edges that fail to convert are dropped with only a printed warning.

**Tests and CI.**
- proptest is used across crates.
- GitLab CI runs CPU, nightly, wasm, GPU, clippy and fmt jobs ([.gitlab-ci.yml](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/.gitlab-ci.yml#L1-L80)).
- Lints are set in code (`deny(warnings)` in release builds; `unsafe_code` is warn, not forbid) ([lib.rs L71-82](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-topology/src/lib.rs#L71-L82)).

## 2. Fornjot (hannobraun/fornjot)

**Licence and status.** Licence is 0BSD ([LICENSE.md](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/LICENSE.md)). The repo is archived; the README's first line reads "This project has been shut down. Its goals were never reached." ([README L1](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/README.md#L1)).

**Mainline architecture.** The seven crates were folded into a single `fj` crate: fj-math in [e0c8b6c](https://github.com/hannobraun/fornjot/commit/e0c8b6c501b4ef2c1f4edefcd3943f6b7b024111) (2026-02-22), fj-core in [7013a4f](https://github.com/hannobraun/fornjot/commit/7013a4f4c1ec0f74d6cde5ee5636e61659e526f1) (2026-06-11).
- **Stores and handles:**
  - The store is append-only: blocks of 16,384 slots behind `Arc<RwLock>`. Slots are reserved first and filled later so objects can reference each other ([store.rs L1-89](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/storage/store.rs#L1-L89)).
  - A `Handle` holds the store `Arc`, an index and a raw pointer. It dereferences through `unsafe` code, which the author tested under Miri ([handle.rs L70-107](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/storage/handle.rs#L70-L107)).
  - Identity is `ObjectId(ptr as u64)`, and Eq, Hash and Ord all use it ([L126-150](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/storage/handle.rs#L126-L150), [L184-191](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/storage/handle.rs#L184-L191)). So `BTreeSet<Handle>` order follows allocation addresses.
  - `Send`/`Sync` are implemented unsafely with no bound on `T` ([L177-178](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/storage/handle.rs#L177-L178)), which is a soundness hazard.
  - The docs say equality defers to the object, but the code compares ids ([L22-28](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/storage/handle.rs#L22-L28)).
- **Layers:** each `Layer<S>` turns writes into commands that emit events (the "decider" pattern) ([layer.rs L3-42](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/layers/layer.rs#L3-L42)). The layers are geometry, topology, validation and presentation. Geometry is stored outside topology and defined locally: a vertex on a curve, a curve on a surface.
- **Validation layer:**
  - Every inserted object is validated. Errors accumulate, and dropping the layer with unhandled errors panics (or it panics immediately if `panic_on_error` is set) ([validation.rs L18-44](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/layers/validation.rs#L18-L44)).
  - Thresholds come from the tolerance: identical objects may be up to 10×tol apart, and distinct objects must be at least 20×tol apart. The default tolerance is 0.001 ([config.rs L22-68](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/validation/config.rs#L22-L68)).
  - Its best check runs both ways: vertices that are identical must coincide, and vertices that are distinct must not. It is O(N²) ([validate/solid.rs L137-175](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/validate/solid.rs#L137-L175)).
  - Other checks cover half-edge connection, siblings, face winding and multiple references ([checks/](https://github.com/hannobraun/fornjot/tree/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/validation/checks)).
- **Scalar:** an f64 wrapper that rejects NaN and infinity (by panicking) so it can implement Eq, Ord and Hash ([scalar.rs L14-59](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/math/scalar.rs#L14-L59)).

**How the approach evolved** (from the experiment READMEs):
- **2024-10-30:** the author judged the architecture to be at a local maximum and said the core mistake was designing a batch system ([L17](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/experiments/2024-10-30/README.md#L17), [L42](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/experiments/2024-10-30/README.md#L42)).
- **2024-12-09:** a much simpler b-rep graph worked ([L57](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/experiments/2024-12-09/README.md#L57)).
- **2025-03-18:** storing all geometry globally in 3D simplified the graph, but recovering local definitions then needed projection onto curves and surfaces ([L56-60](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/experiments/2025-03-18/README.md#L56-L60)).
- **2025-11-07:** separated owned from shared objects and went back to local coordinates ([L25-27](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/experiments/2025-11-07/README.md#L25-L27)).
- **2025-12-03, approximation-first:** append-only vertex and triangle lists with topology layered on top, built per operation at a given tolerance.
  - Exactness is given up explicitly ([L143](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/experiments/2025-12-03/README.md#L143)).
  - STEP export is left as an open problem ([L153](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/experiments/2025-12-03/README.md#L153)).
  - It was declared a success and merged ([L165](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/experiments/2025-12-03/README.md#L165)).
  - In the merged code (`core/new`), a `Store<T>` is a plain `Vec<T>` and a `Handle` is a `Copy` usize index with derived `Ord` ([store.rs L22-132](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/new/topology/store.rs#L22-L132)). An `Edge` stores `approx: Vec<Point<3>>` ([primitives.rs L124-133](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/crates/fj/src/core/new/topology/primitives.rs#L124-L133)).

**What was abandoned, and the author's published lessons.**
- 2023: the CAD application, runtime model loading and the high-level API were dropped ([A New Direction](https://www.fornjot.app/blog/a-new-direction/)).
- 2026: the project itself ([Shutting Down Fornjot](https://www.fornjot.app/blog/shutting-down-fornjot/)). The listed mistakes:
  - Extrapolating from early success; progress hit a cliff after the switch from signed distance fields to b-rep.
  - Prototyping too late.
  - Sticking to incremental changes when a re-architecture was needed.
  - Building a generic kernel instead of a focused tool.
  - Half-measures after two drops in funding.
  - Never finishing the move to the new architecture.

**Lessons for Spare Kit.**
- Pointer identity was abandoned for index handles, which are deterministic.
- Validation as its own layer, with the two-way identity/coincidence window, is worth copying.
- The local-versus-global geometry trade-off is real; local definitions (pcurves) are needed.
- Approximation-first conflicts with our exactness requirement.
- Splitting into many crates did not pay off for Fornjot.

## 3. curvo and other Rust NURBS / B-rep efforts

- **curvo:** MIT ([LICENSE](https://github.com/mattatz/curvo/blob/11842793ed92640cb78e8ed86d731fdb55e85e34/LICENSE)). The README badge says MIT/Apache, but there is no Apache file and the [Cargo.toml](https://github.com/mattatz/curvo/blob/11842793ed92640cb78e8ed86d731fdb55e85e34/Cargo.toml) says MIT.
  - Status: v0.3.3, active.
  - Scope: NURBS curves and surfaces (interpolate, extrude, loft, sweep, revolve), closest point, curve/plane/surface intersections, 2D curve booleans, offsets, curve fillets, trimmed surfaces, tessellation via spade, Bézier decomposition ([src tree](https://github.com/mattatz/curvo/tree/11842793ed92640cb78e8ed86d731fdb55e85e34/src)).
  - Intersections: bounding-box-tree traversal plus the argmin optimiser, with a relative minimum distance and a cost tolerance ([intersection_curve_curve.rs L56-79](https://github.com/mattatz/curvo/blob/11842793ed92640cb78e8ed86d731fdb55e85e34/src/intersects/curve_curve/intersection_curve_curve.rs#L56-L79)). Numeric, not certified.
  - No solids or topology, no STEP.
  - Depends on nalgebra 0.35, robust, spade, geo, argmin and hashbrown.
  - Verdict: algorithm reference only.
- **opencascade-rs:** LGPL-2.1 ([LICENSE](https://github.com/bschwind/opencascade-rs/blob/32758df23a137f4e7c786c618b0317fc3badb3b2/LICENSE)). cxx bindings to OCCT, so C++ is required. Method study only.
- **monstertruck:** Apache-2.0 fork of Truck ([README](https://github.com/virtualritz/monstertruck/blob/d2ad1ed7aa420b7f32cd2c1e3ad319b46d593652/README.md)).
  - Booleans return `Result<Solid, ShapeOpsError>`.
  - It adds geometry guards (for example an `acos` clamp and pole singularities), rebuilt CDT trimming and idiomatic naming.
  - It hand-ports upstream fixes. It has AGENTS.md and CLAUDE.md files (AI-assisted).
  - The core topology design is presumably still Truck's (*unverified*).
- **csgrs:** MIT ([Cargo.toml](https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/Cargo.toml)). Now an OpenSCAD-like grammar over the "Hyper" crates ([readme](https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/readme.md)):
  - hypermesh: triangle-mesh booleans described as certified.
  - hyperreal: exact rational plus computable-real arithmetic, Apache-2.0. `rug` is only a dev-dependency, so there is no GMP at runtime ([hyperreal Cargo.toml](https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hyperreal/Cargo.toml)).
  - HyperBRep: exact B-rep, still being developed.
  - Floats appear only at boundaries.
  - Verdict: study the exact-number design. It is mesh-first and young.
- **solidcore:** Apache-2.0, with an NVIDIA copyright line ([LICENSE](https://github.com/tangency-labs/solidcore/blob/6a019c57131d5fee6e57ab7dc4a62201a32e0216/LICENSE)). A Rust port of SMLib ([README](https://github.com/tangency-labs/solidcore/blob/6a019c57131d5fee6e57ab7dc4a62201a32e0216/README.md)).
  - Stated goals: no unsafe, no panics, deterministic, no global state, wasm32.
  - Every unit is checked against the C++ to 1e-9 relative by a difftest harness.
  - About a third is ported; it says nothing works end to end yet.
  - It warns that wasm32 stacks have no guard page.
- **vcad:** Apache-2.0, about 90 crates.
  - The topology is a slotmap half-edge arena ([topo lib.rs](https://github.com/ecto/vcad/blob/cdbfe885b0a0ed7b3492e33d7b2f4a1da4f3bd6c/crates/vcad-kernel-topo/src/lib.rs#L3-L30)).
  - The boolean pipeline is AABB filter, surface-surface intersection, classification, then sewing, with a mesh fallback still kept ([booleans lib.rs](https://github.com/ecto/vcad/blob/cdbfe885b0a0ed7b3492e33d7b2f4a1da4f3bd6c/crates/vcad-kernel-booleans/src/lib.rs)).
  - Its breadth (QCD, neutronics, orbit crates) suggests it was generated quickly; depth is *unverified*.
- **foxtrot (Formlabs):** MIT OR Apache-2.0, with both licence files ([repo](https://github.com/Formlabs/foxtrot/tree/69fb37688892246e670fc08eb60b7d3a92d70d3f)). A STEP parser generated from EXPRESS, plus cdt, NURBS and triangulation crates, and a wasm viewer. No commits since 2022.
- **geo:** MIT OR Apache-2.0 (manifest). 2D, GIS-oriented.
  - Booleans come from i_overlay, pinned to versions from 4.5.1 up to but not including 4.6.0, while i_overlay is at 9.1.0. It also uses robust, rstar, spade and earcut ([geo/Cargo.toml L16-43](https://github.com/georust/geo/blob/c12769fdf7453df1dd590e6e3bca28913d3c0564/geo/Cargo.toml#L16-L43)).
  - No arcs.
- **Avoid on licence:**
  - chili3d is AGPL-3.0 and has no Rust code; it is TypeScript plus C++ OCCT ([languages](https://github.com/xiangechen/chili3d)).
  - brepkit is [AGPL-3.0](https://github.com/andymai/brepkit).
  - Aetheris is [AGPL-3.0](https://github.com/yuechen-li-dev/Aetheris).
  - next.BREP.io's Rust kernel (the successor Fornjot points to) uses a custom licence that requires assigning copyright of modifications to Autodrop3d LLC ([LICENSE.md](https://github.com/mmiscool/next.BREP.io_RUST_BREP_KERNEL/blob/d1cbf034c4a9dc3a21be2bcc6f84c3008484716b/LICENSE.md)).

## 4. Building-block crates

**Predicates and exact arithmetic**
- **robust:** MIT OR Apache-2.0, with both licence files ([repo](https://github.com/georust/robust/tree/654f34cb8cdb3ae21bf59ef3472f92652942cd74)). Version 1.2.0, last released 2025-05.
  - Shewchuk's adaptive `orient2d`, `orient3d`, `incircle` and `insphere` only ([lib.rs L77, L173, L571, L1332](https://github.com/georust/robust/blob/654f34cb8cdb3ae21bf59ef3472f92652942cd74/src/lib.rs#L77)). Has a `no_std` feature.
  - Determinism: plain IEEE add and multiply, so bit-identical on conforming targets. There is no overflow/underflow guard.
  - Verdict: depend on it or vendor it (one file). We will need more predicates than these four.
- **geometry-predicates:** MIT; last released 2021-02. Avoid, since robust covers it.
- **num-bigint / num-rational:** MIT OR Apache (manifest), pure Rust, mature. Fine as a slow exact fallback.
- **dashu:** MIT OR Apache (manifest). Pure Rust, `no_std`, with integer, float (any precision) and rational types ([README](https://github.com/cmpute/dashu/blob/a0fc5358a3335e9fce155c5ddf8ae7ca83f24a33/README.md)). The best permissive big-number option.
- **rug / gmp-mpfr-sys:** LGPL-3.0+ (manifest). They build GMP/MPFR/MPC from C. Avoid.
- **malachite:** LGPL-3.0-only (manifest). Its README says parts are derived from GMP, FLINT and MPFR ([README L5-6](https://github.com/mhogrefe/malachite/blob/810ed03d2e8416571a2a989824bdc4fe2ce04bf6/README.md#L5-L6)). Avoid; method study only.
- **hyperreal:** Apache-2.0. Study only.

**Math functions and linear algebra**
- **libm:** MIT ([manifest](https://github.com/rust-lang/compiler-builtins/blob/aeab328368d6ed8dc6acba47b7db6ad08be953d2/libm/Cargo.toml)). Pure Rust; some routines (cbrt, hypot) are ported from CORE-MATH.
  - The default `arch` feature only swaps in hardware versions of exactly-specified operations (sqrt, fma, rounding) ([arch/mod.rs](https://github.com/rust-lang/compiler-builtins/blob/aeab328368d6ed8dc6acba47b7db6ad08be953d2/libm/src/math/arch/mod.rs#L11-L60)), so results should not change across targets.
  - Verdict: depend on it, and make it the only source of transcendental functions.
- **nalgebra:** Apache-2.0 ([LICENSE](https://github.com/dimforge/nalgebra/blob/main/LICENSE)). Generic-heavy, which slows compiles. Avoid in core kernel types.
- **glam:** MIT OR Apache (manifest). The f64 types (`DVec3` and friends) are scalar; SIMD applies to f32 types. Features `libm` and `scalar-math` ([Cargo.toml](https://github.com/bitshifter/glam-rs/blob/f0396ebdb5ce214fef80cfef03899de9b752bc54/Cargo.toml)). Good at the rendering boundary.
- **ultraviolet:** MIT OR Apache OR Zlib (manifest); f32 SIMD focus; last release 2025-04. Avoid.

**Spatial queries and triangulation**
- **parry:** Apache-2.0 ([LICENSE](https://github.com/dimforge/parry/blob/b3c3584ce9be9bf6edb821fd7548b817087131ff/LICENSE)). Collision detection, now built on glam.
  - Its `enhanced-determinism` feature forces libm and turns off SIMD ([parry3d-f64 Cargo.toml](https://github.com/dimforge/parry/blob/b3c3584ce9be9bf6edb821fd7548b817087131ff/crates/parry3d-f64/Cargo.toml)).
  - Its changelog records that SIMD dot products broke cross-platform determinism until that fix ([CHANGELOG L175-177](https://github.com/dimforge/parry/blob/b3c3584ce9be9bf6edb821fd7548b817087131ff/CHANGELOG.md#L175-L177)).
  - Verdict: learn from its GJK/EPA and BVH code; don't put it in the kernel.
- **bvh:** MIT ([LICENSE](https://github.com/svenstaro/bvh/blob/master/LICENSE)); f32 over nalgebra. Write our own f64, deterministic BVH.
- **rstar:** MIT OR Apache (manifest), maintained. An R*-tree, acceptable for a 2D drafting index.
- **spade:** MIT OR Apache-2.0, maintained (2.15.1).
  - Predicates are exact (via robust), but coordinates must lie between 2^-142 and 2^201 ([math.rs L51-81](https://github.com/Stoeoef/spade/blob/c8befc96bbbc1898a89cb19f9f3104a848936374/src/delaunay_core/math.rs#L51-L81)).
  - The basic CDT takes only weakly intersecting constraints ([cdt.rs L106-109](https://github.com/Stoeoef/spade/blob/c8befc96bbbc1898a89cb19f9f3104a848936374/src/cdt.rs#L106-L109)); `add_constraint_and_split` computes intersection points in floats ([L1227](https://github.com/Stoeoef/spade/blob/c8befc96bbbc1898a89cb19f9f3104a848936374/src/cdt.rs#L1227)).
  - Verdict: fine for display meshing, not for kernel truth.
- **cdt (Formlabs):** MIT OR Apache; 0.1.0 (2021), built on geometry-predicates ([manifest](https://github.com/Formlabs/foxtrot/blob/69fb37688892246e670fc08eb60b7d3a92d70d3f/cdt/Cargo.toml)). Unmaintained; learn from it.
- **delaunator:** ISC (manifest). Unconstrained Delaunay only; uses robust.
- **earcutr:** ISC, archived. Its successor, georust/earcut, is MIT OR Apache (manifest). Display only.

**2D curves, paths and polygon booleans**
- **cavalier_contours:** MIT OR Apache.
  - Exact line and arc (bulge) polylines with offsets and booleans; f64 with fuzzy epsilons.
  - Documented limits ([README](https://github.com/jbuckmccready/cavalier_contours/blob/22887a824d22cd8ab50968a551f34cdb4508127f/README.md)): arcs at most a half circle, round joins only, booleans between two closed polylines that don't self-intersect. Has a C FFI.
  - Verdict: the best reference for AutoCAD-style offsets. Port the ideas onto exact predicates.
- **i_overlay:** MIT OR Apache.
  - Integer core (i16/i32/i64) with float adapters and documented coordinate budgets ([README L629-662](https://github.com/iShape-Rust/iOverlay/blob/117637e3a65804973da2bfd7c59f42773f4dcc0d/iOverlay/README.md#L629-L662)). Lines only.
  - Optional rayon via `allow_multithreading`.
  - Verdict: depend on it for linearised 2D regions (hatch, fill, display).
- **clipper2-rust:** BSL-1.0 (manifest). Pure-Rust port of Clipper2 that claims an exact behavioural match with the C++ ([README](https://github.com/larsbrubaker/clipper2-rust)). An alternative to i_overlay.
- **clipper2 (tirithen)** is MIT OR Apache and **geo-clipper** is ISC. Both wrap C++. Avoid.
- **kurbo:** Apache-2.0 OR MIT (manifest). f64 Bézier and arc toolkit with a `libm` feature ([Cargo.toml](https://github.com/linebender/kurbo/blob/9366cda80ebb56f41e5d09509a89dbfe81bc4f5b/kurbo/Cargo.toml)). Learn from it.
- **lyon:** MIT OR Apache (manifest). Tessellation is f32 ([tessellation lib.rs L334-367](https://github.com/nical/lyon/blob/6f2e57ec8624a1e6cfa2b642231c8f477c85c0f2/crates/tessellation/src/lib.rs#L334-L367)). Display only.
- **meshopt:** MIT OR Apache (manifest); bindings to C++. Display only.

**Arenas and persistent data structures**
- **slotmap:** Zlib ([LICENSE](https://github.com/orlp/slotmap/blob/master/LICENSE)). Generational keys; deterministic for a given sequence of operations.
- **la-arena** (rust-analyzer) and **typed_index_collections**: MIT OR Apache (manifest), both maintained.
- **generational-arena:** MPL-2.0, archived. Avoid.
- **thunderdome:** MIT OR Apache (manifest); last release 2023.
- **im:** MPL-2.0+ (manifest), archived. Its maintained fork **imbl** is MPL-2.0+.
- **rpds:** MIT ([LICENSE.md](https://github.com/orium/rpds/blob/main/LICENSE.md)).
- For determinism, use only the ordered maps: hash-map iteration order follows the hasher's seed.

**Concurrency and hashing**
- **rayon:** MIT OR Apache (manifest). Its docs say float `sum`/`reduce` results are not fully deterministic ([mod.rs L1363-1367](https://github.com/rayon-rs/rayon/blob/ee0a00bdb1ab039e178a215ad5712fb7fa58e58f/src/iter/mod.rs#L1363-L1367)). Use `fold_chunks` with a fixed chunk size and combine sequentially ([L2716-2745](https://github.com/rayon-rs/rayon/blob/ee0a00bdb1ab039e178a215ad5712fb7fa58e58f/src/iter/mod.rs#L2716-L2745)).
- **foldhash:** Zlib (manifest). `RandomState` seeds every table differently; `FixedState` is deterministic ([lib.rs](https://github.com/orlp/foldhash/blob/77d8e3dae0d43abe65753e888b9e74efbe4ee3f7/src/fast.rs)).
- **rustc-hash:** Apache-2.0 OR MIT (manifest); unseeded.

**Testing and release tooling**
- **proptest:** MIT OR Apache. Truck uses it everywhere.
- **cargo-fuzz:** MIT OR Apache; libFuzzer, needs nightly.
- **bolero:** MIT; one harness drives both property tests and fuzzers.
- **criterion:** Apache-2.0 OR MIT; moved to the criterion-rs org, 0.8.
- **divan:** MIT OR Apache; last release 2025-04.
- **cargo-semver-checks:** Apache-2.0 OR MIT, active.

## 5. Rust-specific architecture guidance

**Arenas and typed indices versus Rc/Arc graphs.**
- Both mature Rust kernels used pointer identity (Truck's `ID`, Fornjot's `ObjectId`), and so both lost determinism. Fornjot ended with a `Vec<T>` plus a `Copy` usize handle.
- Recommendation: one `Vec`-backed arena per entity kind, with `u32` newtype indices assigned in operation order.
- Geometry stays immutable; an edit creates new entries.
- Never order or hash by address.
- Use slotmap only where deletion with slot reuse inside a session matters.
- Persistent naming across edits belongs in a separate layer.

**Immutable snapshots.**
- Fornjot's append-only store with immutable objects made history cheap, but the shared object graph became the source of its complexity.
- Make kernel operations pure: `fn(&Model, args) -> Result<Model, OpError>`.
- Share structure through `Arc` chunks, or through rpds (MIT) or imbl (MPL-2.0) ordered maps, never hash maps.

**Error types.**
- Truck returns `Option` and panics inside library code. monstertruck has to retrofit `Result`. Fornjot's validation errors carry the entity handles and positions.
- Use one error enum per operation, with structured payloads (entity ids, parameters, points).
- In kernel crates, deny `clippy::unwrap_used`, `expect_used`, `panic` and `indexing_slicing`.
- Bound recursion: wasm32 stacks have no guard page ([solidcore README](https://github.com/tangency-labs/solidcore/blob/6a019c57131d5fee6e57ab7dc4a62201a32e0216/README.md)).
- Use a NaN-free scalar type like Fornjot's, but return `Err` instead of panicking.

**no_std and wasm32.**
- Transcendental functions come from `libm`.
- No `std` `RandomState` maps.
- rayon is unavailable on wasm32-unknown-unknown without a threads setup; Truck gates it off.
- `getrandom` needs its wasm feature; Truck's dev-dependencies show this.

**Float determinism in Rust.**
- These operations exactly match IEEE 754-2008 round-to-nearest-even, with no flush-to-zero: `+ - * / %`, `abs`, `copysign`, `mul_add`, `sqrt`, float casts and comparisons ([RFC 3514 L50-54](https://github.com/rust-lang/rfcs/blob/f63a7efbb9d83ae0616cf752c000cd3df4ca5d3e/text/3514-float-semantics.md#L50-L54)).
- **NaN** sign and payload are non-deterministic, including on wasm ([L56-85](https://github.com/rust-lang/rfcs/blob/f63a7efbb9d83ae0616cf752c000cd3df4ca5d3e/text/3514-float-semantics.md#L56-L85), [L295](https://github.com/rust-lang/rfcs/blob/f63a7efbb9d83ae0616cf752c000cd3df4ca5d3e/text/3514-float-semantics.md#L295)). Compile-time evaluation may pick differently from run time ([L18](https://github.com/rust-lang/rfcs/blob/f63a7efbb9d83ae0616cf752c000cd3df4ca5d3e/text/3514-float-semantics.md#L18)). Rule: never let a NaN out of an operation.
- **FMA is never automatic.** The RFC says strict semantics rule out fusing `a*b+c` ([L230](https://github.com/rust-lang/rfcs/blob/f63a7efbb9d83ae0616cf752c000cd3df4ca5d3e/text/3514-float-semantics.md#L230)). An explicit `mul_add` is correctly rounded ([f64.rs L189-200](https://github.com/rust-lang/rust/blob/19833af7224424e8b53e288097d56b6af43d5bcf/library/std/src/num/f64.rs#L189-L200)).
- **`-C target-cpu`** should change only the speed of an explicit `mul_add` (hardware versus software fma), not its value. Treat that as *unverified* until a native-versus-wasm test passes.
- **std transcendentals** (`sin`, `exp`, `powi`, and so on) have explicitly unspecified precision that varies by platform and version ([f64.rs L311-314](https://github.com/rust-lang/rust/blob/19833af7224424e8b53e288097d56b6af43d5bcf/library/std/src/num/f64.rs#L311-L314)). Ban them with clippy's `disallowed-methods`.
- **x87:** i586 is non-conforming, and on i686 float return values pass through x87 registers, which alters NaN payloads ([L177-178](https://github.com/rust-lang/rfcs/blob/f63a7efbb9d83ae0616cf752c000cd3df4ca5d3e/text/3514-float-semantics.md#L177-L178)). ARM32 NEON flushes to zero ([L180](https://github.com/rust-lang/rfcs/blob/f63a7efbb9d83ae0616cf752c000cd3df4ca5d3e/text/3514-float-semantics.md#L180)). Exclude these targets from the guarantee.
- **SIMD:** horizontal reductions change rounding order (parry's bug above). Keep kernel math scalar, or use fixed lane-order reductions.
- **Parallel reductions:** rayon float reductions are non-deterministic (above).
- Rapier's documented recipe for cross-platform determinism (IEEE targets including wasm, the same insertion order, its own math functions) is the closest Rust precedent ([Rapier](https://rapier.rs/docs/user_guides/rust/determinism)).
- **CI:** hash the bits of every golden output and compare x86_64, aarch64 and wasm32 (run under wasmtime).

**Unsafe policy.**
- Truck only warns on `unsafe_code`. Fornjot's handle used `unsafe` and an unbounded `unsafe impl Send/Sync`. solidcore aims for no unsafe at all.
- Use `#![forbid(unsafe_code)]` in every kernel crate. Any exception lives in one audited crate tested under Miri.

**Workspace layout and compile speed.**
- matklad's guidance ([Large Rust Workspaces](https://matklad.github.io/2021/08/22/large-rust-workspaces.html), [Fast Rust Builds](https://matklad.github.io/2021/09/04/fast-rust-builds.html)):
  - A flat `crates/` folder, with folder name equal to crate name.
  - A virtual root manifest; internal crates at version `0.0.0`.
  - A wide crate graph, not a deep chain.
  - No generics across crate boundaries: a thin generic wrapper calls a non-generic inner function, and closures go through `&dyn`.
  - Proc-macro and serde derives at the leaves, not the foundation.
  - `-D warnings` through `RUSTFLAGS`, not in code (Truck does the opposite).
- Truck's chain (base → geotrait → geometry → topology → meshalgo → shapeops → stepio) with `<P,C,S>` generics everywhere pushes monomorphisation into the leaf crates.
- Prefer concrete enums for geometry and non-generic topology.

## 6. Recommendation

**Depend on:**
- libm (or vendor it).
- robust (or vendor it).
- dashu, with num-rational as the alternative, for the exact fallback.
- i_overlay or clipper2-rust for linearised 2D polygon work only.
- spade for display tessellation only.
- Dev tools: proptest, bolero and cargo-fuzz, criterion or divan, cargo-semver-checks.
- Optional: glam at the render boundary, rstar for the drafting index.

**Write ourselves:**
- f64 point/vector/transform types.
- The predicate suite beyond Shewchuk's four, with interval filters.
- NURBS evaluation and knot operations.
- Curve/surface intersection.
- The arena B-rep with pcurves.
- Validation.
- Booleans that never decide topology from a mesh.
- An f64 BVH.
- STEP and DXF I/O (learn from ruststep and foxtrot).
- Exact line+arc offsets and booleans (learn from cavalier_contours).

**Layering lessons.**
- Truck's split of geometry from topology is good. Booleans depending on meshing is bad: decisions then depend on the tessellation tolerance.
- Fornjot's split into seven crates was undone.
- Proposed graph, wide and concrete: `sk-num` (scalars, predicates, intervals, exact) → `sk-curve2d` / `sk-geom3d` → `sk-topo` (arena + validate) → independent op crates (`sk-bool`, `sk-offset`, `sk-blend`) → `sk-mesh` → `sk-io-*`, plus a shared test-corpus crate.

## Adopt / Avoid / Open questions

**Adopt**
- Index arenas with `u32` typed handles (Fornjot's final design).
- Immutable geometry and pure operations.
- A validation layer with Fornjot's two-way identity/coincidence window, but O(n log n) via a BVH.
- `Result` everywhere.
- `forbid(unsafe_code)`.
- libm-only transcendentals.
- No SIMD or rayon float reductions in kernel math.
- solidcore-style differential testing against a reference kernel.
- proptest plus fuzzing.
- matklad's workspace rules.
- Truck's curve/surface trait vocabulary as a naming reference.

**Avoid**
- `Arc<Mutex>` topology and pointer identity (Truck, old Fornjot).
- Global absolute tolerance constants and asserting on tolerances.
- Mesh-seeded boolean classification.
- Approximation-first (Fornjot's last design).
- nalgebra or parry in kernel types.
- LGPL/GPL/AGPL code: rug, malachite, opencascade-rs, chili3d, brepkit, Aetheris.
- The next.BREP.io licence.
- Archived crates: im, generational-arena, earcutr.
- std `HashMap` iteration.
- std transcendental functions.

**Open questions**
- Should Spare Kit port or study NVIDIA USD BRep (Apache-2.0) directly, or join or fork solidcore rather than start from Truck? This needs a C++-side audit; it is outside this brief.
- Exact-number strategy: adaptive floats with dashu fallback, or hyperreal-style computable reals? Needs benchmarks.
- Which targets get the bit-identical guarantee? i686 and ARM32 are likely excluded.
- Can we prove `mul_add` gives identical bits on native without FMA, native with FMA, and wasm? *Unverified* in CI.
- Persistent topological naming over arena indices is not solved by any Rust kernel reviewed.
- Do spade's or curvo's hashbrown-based internals leak iteration order into outputs? *Unverified*; audit before any use.
