# 30. Build order

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

This chapter is the kernel's work board: every piece of work, in the order it is built, with what "done" means. It merges the build pieces drafted in chapters 11 to 29, adds the pieces the foundation needs that no chapter listed, turns every owner decision into a piece, and closes each phase with a gate. The rule is the owner's: **build layer by layer, and prove each layer before the next one leans on it.** Piece G01 is the decision note itself, G02 to G06 were opened before the board was numbered (30.0), and the board starts at G07. The list behind the tables is committed beside this chapter as `architecture/30-build-order.json`, and the tables are generated from it (30.7).

## 30.0 Pieces already under way

Five pieces were opened and started before this board was numbered, so the board itself starts at G07. Each covers some of the board's pieces, in full or in part. A covered piece closes when the piece covering it merges; a partly covered piece keeps the rest of its scope open. The JSON records each as `covered_by` or `partly_covered_by`, and the cells below say what is left.

| Piece | Issue | Pull request | What it builds | Covers in full | Covers in part, and what is left |
| --- | --- | --- | --- | --- | --- |
| G02 | [#171](https://github.com/spareware-co-op/spareware/issues/171) | [#177](https://github.com/spareware-co-op/spareware/pull/177) | The numbers layer, `geom-num`: grid, ladder, wide integers (`I256`, `I320`, `I384` and `I512`), exact explicit-point predicates, planes and three-plane points with their commit to the grid, error-free transformations, intervals, the NaN-free scalar, the maths module, exactness classes; the first kernel CI and golden-hash determinism test | G22 | G24 (left: vendoring and auditing `libm`, its benchmarks, and the debug tripwires G02 left out, 31.7); G26 (left: the widths' acceptance, `I512` included, 6.2; reserving `I576` and `I768`; 24 hours of differential fuzzing; a benchmark per operation on aarch64); G28 (built: filter-boundary oracle inputs, the `robust` cross-check, and the benchmark on three targets through the instruction-count gate's aarch64 leg; the generated filters came with G27); G29 (built: the plane through three grid points, three-plane points, classification in `I384` and the commit; left: line-plane points, 2D segment crossings and filters); G31 (left: the regression comparison of `libm` on a million inputs per function); G123 (built in `geom-num`: the plane type and its construction check; left: gcd normalisation, sharing, coincidence, parallelism and the orders); G10 and G19 (built: the first CI jobs on every pull-request target, and the SHA-256 golden hash of `geom-num`'s corpus; left: G10's nightly, weekly, toolchain-change and release workflows and its machines, G19's golden store keyed by edition, perturbing allocator and repeat and shuffle hooks, and the three gaps 21 lists); G12 (`geom-num`'s `clippy.toml` and lints) |
| G03 | [#172](https://github.com/spareware-co-op/spareware/issues/172) | [#189](https://github.com/spareware-co-op/spareware/pull/189) | Geometry maths and frames, `geom-math` | | G45 (built: 3D points, vectors, boxes and exact, rational and certified transforms; left: transforms on 2D points, vectors and boxes, and `FBox2`); G48 (built: frame records with Map and Ground kinds, correctly rounded conversion both ways, lattices, `FrameMismatch` and `FrameRangeExceeded`; left: linear units, epochs, deformation models, decimal parsing and the rotation form, 17.1). Its own double-double type is replaced by G25's, onto which `geom-math` moves once both merge |
| G04 | [#173](https://github.com/spareware-co-op/spareware/issues/173) | [#182](https://github.com/spareware-co-op/spareware/pull/182) | The deterministic parallel helper, budgets and cancellation, `geom-run` | G23 | G33 (built: `geom-run`'s helper and serial executor; left: `geom-pool` and rayon's audit, the kernel-wide `clippy` bans, and the replay hashes on every target with chunk sizes forced; its name and folder are open, 31.7) |
| G05 | [#174](https://github.com/spareware-co-op/spareware/issues/174) | [#191](https://github.com/spareware-co-op/spareware/pull/191) | Correctly rounded maths functions with proven error bounds, in `geom-num`: CORE-MATH ports of `sinpi`, `cospi`, `exp`, `ln` and `atan2`, and interval versions of the first four. They replace G02's unproven `sinpi` and `cospi`, which G03's rotations call | | G47 (left: `Interval::atan2`; the oracle check, since the worst-case lists run by hand rather than 10 million inputs in CI, 31.7; speed on wasm32; and the exact reduction of integer angles, unless piece "Angle units and core COGO" takes it, 17.2 and 31.7) |
| G06 | [#175](https://github.com/spareware-co-op/spareware/issues/175) | [#190](https://github.com/spareware-co-op/spareware/pull/190) | The benchmark harness and its count gates, `geom-bench` | | G18 (built in full; left: a passing run of the WebAssembly fuel and growth gate, which failed on #190 on 21 October; a first comparison by the instruction and heap gate, which on #190 only recorded, because its merge base has no benchmarks; and the owner's ruling on G02's deliberate fuel rises, 32.12); G11 (the `geom-bench` crate and its benchmarks) |

Board pieces have also been built ahead of their turn, on G02's unmerged branch, before the pieces their board entries need had merged or been recorded (30.1). G09 also waits for owner action G08, so it is built to chapter 27's proposed table.

| Piece | Issue | Pull request | State | Its own scope | Also covers in part |
| --- | --- | --- | --- | --- | --- |
| G09 | [#178](https://github.com/spareware-co-op/spareware/issues/178) | [#183](https://github.com/spareware-co-op/spareware/pull/183) | Built | Met: 42 new crates beside `geom-num`, and the facade's scaffold check. Not created: `geom-math`, `geom-run`, `geom-pool`, `geom-bench` and the development crates, which arrive with their pieces; the six toolkits and `geom-gerber` and `geom-ipc2581`, which wait for the two-program rule (27); the binding crates, built in phase 13 | G12 (the crate-graph check, per-crate `no_std` and wasm32 builds, one lint posture); G15 (an interim header grammar) |
| G11 | [#179](https://github.com/spareware-co-op/spareware/issues/179) | [#184](https://github.com/spareware-co-op/spareware/pull/184) | Built | `geom-test`, `geom-fuzz` and `geom-gen`; the fourth crate, `geom-bench`, is G06's | G12 (the rule that no kernel crate depends on a development crate); G14 (built: `geom-fuzz`'s layout, and its first target run by bolero's test mode, libFuzzer under cargo-bolero, Kani and ClusterFuzzLite) |
| G25 | [#181](https://github.com/spareware-co-op/spareware/issues/181) | [#193](https://github.com/spareware-co-op/spareware/pull/193) | Built | Met: double-double arithmetic and `ExactSum` in `geom-num`, with the 2-times bar held by the collection entry points on a quiet machine | |
| G32 | [#180](https://github.com/spareware-co-op/spareware/issues/180) | [#185](https://github.com/spareware-co-op/spareware/pull/185) | Built | Met, with the randomised-method rule in `geom_num::rng`'s header until G17 lifts it into `AGENTS.md`; its goldens run only in the whole-suite legs (21) | G12 (the `clippy` bans on randomness, clocks and thread identity) |
| G12 | [#187](https://github.com/spareware-co-op/spareware/issues/187) | None yet | Under way | | |
| G15 | [#188](https://github.com/spareware-co-op/spareware/issues/188) | None yet | Under way | | |
| G21 | [#192](https://github.com/spareware-co-op/spareware/issues/192) | None yet | Under way | | |
| G27 | [#186](https://github.com/spareware-co-op/spareware/issues/186) | None yet | Under way | | |

The first five were started ahead of the tooling pieces their board entries need (G09 to G21), and G03 and G05 build phase-2 work (G45, G47, G48) before gate 1 (G43). Gate 1 still requires every phase-1 piece. Every pull request above is stacked on G02's, and in what order they merge, given their gates and G08, is the owner's decision 32.12. None of them yet meets the whole standard bar of 30.1, because its tools are still being built: fuzz targets wait for `geom-fuzz`'s layout (G11, G14), so G03's "fuzz" tests are property tests and G04 and G25 have none; and benchmarks wait for `geom-bench`'s count gates (G06), so G03's, G04's, G05's and G25's are timing harnesses outside them (23.13). G25's cases are not yet in `geom-num`'s shared golden corpus.

## 30.1 How to read the board

- **A piece** is one pull request's worth of work, sized for one contributor's tokens: Small, Medium or Large (S, M and L in the tables). Nothing on the board is larger than Large; anything bigger in a chapter draft has been split.
- **IDs are in build order.** A piece's ID is always higher than the IDs of the pieces it needs.
- **"Needs" is the authority.** A piece may start when every piece it needs has merged or been recorded. Needs include gate pieces, owner actions and, in one place, pieces on Spare Construct's board (`CON-A18-F2-P1` and `CON-A18-F2-P2`), which must be done too.
- **Phases end at gates.** Each phase's last piece is its gate (lane Gate): a review that runs the phase's gate condition and has a kernel owner record the result, with the CI runs that show it, in the decision log. The gate needs every piece of its phase except those marked not gating in 30.5. Every piece of a phase needs the gates its phase starts after (30.2): directly when it needs nothing else in its phase, otherwise through a piece it needs. So no piece can start before the layers beneath it are proven. The generator checks this for every piece.
- **Lanes** (A, B, C, ...) are tracks inside a phase that run side by side. Pieces in different lanes can be worked on at the same time by different contributors; inside a lane, follow "Needs".
- **Owner actions are pieces** (crate "owner action", lane Owner), because work waits on them. Each is a decision or check only an owner can make, recorded in G01 section 32, the decision log or, for a patent search, the patent register. An owner action needs nothing, except that a patent search needs the decision on who searches. So each can be settled at any time, and should be settled early: the table in 30.5 shows what waits on it.
- **Crates** follow chapter 27's proposed table and its thirteen layers, plus the additions in 30.5. Every need points to a piece in the same crate or in a crate below it in that graph; the generator checks this too. A need marked **(tests)** is one only the piece's tests use, so it may point upwards. If the owner refuses a crate amendment, the piece moves to the crate the owner names and nothing else on the board changes.

**The standard bar.** Every piece that adds kernel code also meets these, so the "done when" of each piece lists only what is particular to it:

| Every code piece | What it means |
| --- | --- |
| Tests | Unit tests; property tests with fixed seeds on pull requests and fresh seeds nightly (bolero's test mode draws its own seeds, so it counts as the short fuzz, 28.2); a fuzz target with no panic and no hang |
| Benchmark | At least one, under the 5% count gate (principle 12) |
| Determinism | Its cases join the replay corpus and are bit-identical on every target, build mode and thread count of section 21 |
| Operations | The contract laws, history laws, checker postcondition and completeness gate of 28.2; a schema in `engine-geometry` |
| Readable | Crate and module headers complete, so `MAP.json` lists it; every outside source credited (section 29) |
| Safe | No `unsafe`, no panic path, bounded recursion, cooperative cancellation (principle 13) |

The tools behind the bar are built first: the first numbers pieces (G22 and G23) need every one of them, and every later piece inherits them through gate 1. The contract tooling (G100, G103) is inherited the same way through gate 3.

## 30.2 The phases and their gates

| Phase | What it builds | Starts after | Gate |
| --- | --- | --- | --- |
| 1. Ground and numbers | The workspace, CI on every target, the merge gates, the test and benchmark harness, `MAP.json` and the first docs; budgets and the parallel helper; the numbers layer: grid, ladder, wide integers, generated filters, exact predicates, intervals, deterministic maths; their proofs | Nothing | G43: predicates pass extremes and oracles; Verus proofs; filters proven or labelled; determinism matrix green |
| 2. Math types and geometry kinds | Points, frames and transforms; linear algebra, root finding and the certified root engine; the box tree; NURBS; every curve and surface kind with bounds | Gate 1 | G70: the kind contract, the NURBS reference and the root engine hold; laws green for a week |
| 3. Topology, names, contract and format core | Arenas, builder, attributes, checker groups 1 to 4 for primitives, the canonical encoder, persistent names, history and resolution, the operation contract, the facade and schemas, the native format for bodies and names, the operation log, fixtures and replay, primitives and core queries, the reference runner | Gate 2 | G118: primitives pass every law and checker group; save and reload bit-identical; topology proven; fuzzing clean |
| 4. Exact core | Plane-based polyhedra, the exact arrangement engine, flat and n-ary booleans, cells and batch cuts, the local re-cut, checker groups 2 and 3 for plane-based bodies, the second checker, exact meshes and their boolean, exact distance and Delaunay, the first law catalogue | Gate 3 | G156: exact agreement with the rational reference; every fault caught; law rows pass; Manifold and CGAL triaged |
| 5. Certified intersections | Projection, curve-curve, curve-surface, closed-form and general surface intersection, tangent and degenerate cases, the intersection curve kind, certified checker groups 2 and 3, the refutation checker | Gate 3, alongside phase 4; lane C also waits for phase 4's checker pieces | G181: no wrong-topology verdict and flag counts under their ceilings; no curve-pair miss |
| 6. 2D drafting and sketches | Exact 2D predicates, the arrangement, regions, 2D booleans and offsets, edits, snaps, hatches, blocks and dimensions; the sketch solver, its diagnosis, exact commits and certificates; profiles | Gate 5 | G233: agreement with the one-root reference; JTS and geo corpora; sketch harness green |
| 7. Curved modelling | The certified curved boolean and B-rep queries; extrude, revolve, sweeps, lofts, networks; fillets, chamfers and corners; offsets, shells, drafts and face edits; skeletons, roofs and build-ups; exact swept volumes | Gates 4 and 6 | G284: no Failed case in the curved corpora; never-a-hole fuzzing clean; closed forms within bounds |
| 8. Tessellation, spatial index and clash | Certified watertight tessellation, display levels, distance, clash and time of first contact, healing, reconstruction, volume meshing | Gates 4 and 5, alongside phases 6 and 7 | G308: closure certificates; no clash miss or false hit; Thingi10K meshed or refused |
| 9. Free-form | Subdivision surfaces and their conversions, fields and their certified mesher, the conversion graph, the one boolean across representations, parameterisation, panelisation, Class-A analysis, reverse engineering | Gates 7 and 8 | G355: OpenSubdiv agrees; field meshes certified; conversion laws hold; clearance recorded |
| 10. Survey, terrain and civil | COGO, alignment curve kinds, exact Delaunay TINs with breaklines, contours, volumes, toposolids, grading, alignments, corridors, point-cloud queries and fits | Gate 7, alongside phases 8 and 9 | G382: byte-identical TINs; volumes within N·q³; corridor gate met; oracles triaged |
| 11. Drawings | Exact sections and hidden lines, certified curved drawings, line classes, the drawing checker, the incremental engine, developed and civil sections | Gates 7 and 8; two pieces also wait for phases 9 and 10 | G406: cover-once; every fault caught; incremental equals full |
| 12. Native format and exchange | The remaining native records, cold open, the permanence harness; IFC, STEP, DXF (read and write), mesh, LandXML and 3dm exchange, rounding and healing on import, the conformance harness and its oracles | Gates 9, 10 and 11 | G444: the independent reader agrees; writers round-trip and validate; healed fraction at its baseline |
| 13. Scale, truth, bindings, toolkits and the first Stable release | Spare Construct and cross-program scenarios, wall-clock gates, the C, WebAssembly and Python bindings, browser builds, Revit and AutoCAD truth tests, certify mode and research, domain toolkits | Gate 12 (toolkits start when a program needs them) | G482: the first Stable `engine-geometry`, released when the scenarios meet their shares and every truth-test difference is triaged |

Each gate's full condition is the "done when" of its gate piece in 30.4. Wall-clock targets before phase 13 are recorded nightly on the reference runner (G107) and gated in instruction counts; they become merge gates in G452, once the owner has fixed the reference machine and the kernel's shares.

**This is not "flat faces first, curves later".** From phase 2 every type, the topology, the checker and the names carry curved geometry. The exact plane-based path is built in phase 4 because the curved path in phase 7 is tested against it: a curved boolean given only flat input must return the same bytes. Each layer's proof is what the next layer leans on.

## 30.3 Phase 1 in detail

Phase 1 is built first, so it is specified in full. It has three lanes and two owner actions.

- **Owner.** The crate table and layers (G08) and CI hosting and cost (G07) come first, because the scaffold and the runners wait on them.
- **Lane A, the ground.** The workspace and every crate folder of the accepted section 27 table, then CI on the whole section 21 matrix (x86-64 Linux, Windows and macOS; aarch64 Linux and macOS; wasm32 under wasmtime and Node), the merge gate with the crate-graph check, the test, fuzz, benchmark and generator crates, the count gates, the determinism matrix and golden store, the toolchain guard, the mutation, coverage and Miri gates, and the map for people and AI (`MAP.json`, `README.md`, `AGENTS.md`, `GLOSSARY.md`, provenance and notices).
- **Lane B, budgets and the numbers layer.** Budgets and cancellation, then the parallel helper, in `geom-run`; the grid and tolerance ladder, the NaN-free scalar and deterministic maths, double-double and exact sums, wide integers, the predicate generator in `geom-gen`, the exact predicates on explicit and implicit points, symbolic perturbation, intervals for correctly rounded operations, and the seeded generator. Both lane B roots need every tool behind the standard bar, so nothing in lane B merges unchecked.
- **Lane C, the proofs.** The predicate verification suite, Verus proofs of the wide integers and predicates, the filter-certificate spike and then certificates for every filter, Kani harnesses, and the WebAssembly cost report for wide integers, which tells section 31 early whether I384 on wasm32 is a risk.

What gate 1 means in practice: a reviewer can pick any predicate in `geom-num`, read its bit budget in the module header, see the Verus or Kani proof (or the "checked, not proven" label and its open issue), see the oracle and extreme-value tests run on the last pull request, and see the same golden hash on all seven targets at 1, 2, 3 and 8 threads (eight if the owner accepts Windows on Arm, 32.4; on wasm32 the thread-count tests are skipped or run on one thread until a threaded target is tested, 21).

## 30.4 The board

Each table gives a piece's ID, title, crate, size, lane and needs. Its done-when is in `30-build-order.json` and in its GitHub issue.

### Phase 1: Ground and numbers (37 pieces)

Lanes: A the ground (workspace, CI, gates, harness, map); B budgets, the parallel helper and the numbers layer; C proofs and verification. Owner actions are in lane Owner; the gate is G43.

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G07 | CI hosting and cost (owner) | owner action | S | Owner | — |
| G08 | Crate table and layers (owner) | owner action | S | Owner | — |
| G09 | Kernel workspace and crate scaffold | workspace | S | A | G08 |
| G10 | CI matrix and check tiers | CI | M | A | G09, G07 |
| G11 | Test, fuzz, bench and generator crates | geom-test, geom-fuzz, geom-bench, geom-gen | S | A | G09 |
| G12 | Merge gate: licences, lints and builds | CI | M | A | G10, G11 |
| G13 | Recursion check | CI | S | A | G12 |
| G14 | Fuzz toolchain spike | geom-fuzz | S | A | G12 |
| G15 | MAP.json generator and module-header grammar | geom-test (tool) | M | A | G09, G12 |
| G16 | Provenance, notices, method notes and banned-source scan | geom-test (tool) | M | A | G15, G12 |
| G17 | README, AGENTS.md and GLOSSARY.md, first edition | geometry/ docs | M | A | G15 |
| G18 | Benchmark harness and count gates | geom-bench | L | A | G12 |
| G19 | Determinism matrix and golden store | geom-test, CI | M | A | G12 |
| G20 | Toolchain and dependency guard | CI | S | A | G19 |
| G21 | Mutation and coverage gates | CI | S | A | G11, G12 |
| G22 | Grid steps, coordinates and the tolerance ladder | geom-num | M | B | G12, G13, G14, G15, G16, G18, G19, G21 |
| G23 | Budgets, work meter and cancellation | geom-run | M | B | G12, G13, G14, G15, G16, G18, G19, G21 |
| G24 | NaN-free scalar, total order and deterministic maths module | geom-num | M | B | G22, G12 |
| G25 | Double-double arithmetic and ExactSum | geom-num | M | B | G24 |
| G26 | Wide integers I128 to I384 | geom-num | M | B | G22 |
| G27 | Predicate generator | geom-gen (output in geom-num) | L | B | G26, G24 |
| G28 | Explicit-point predicates | geom-num | M | B | G27 |
| G29 | Implicit-point predicates | geom-num | M | B | G28 |
| G30 | Symbolic perturbation ordered by keys | geom-num | S | B | G28, G29 |
| G31 | Outward-rounded intervals | geom-num | M | B | G24, G25 |
| G32 | Counter-based seeded generator | geom-num | S | B | G24 |
| G33 | Deterministic parallel helper (geom-run, geom-pool) | geom-run, geom-pool | L | B | G23, G19 |
| G34 | Predicate verification suite | geom-test | M | C | G28, G29, G30 |
| G35 | Filter-certificate spike | geom-gen, CI | S | C | G34, G10 |
| G36 | Verus twin spike | geom-gen, geom-num | S | C | G27 |
| G37 | Verus proofs: wide integers | geom-num | M | C | G36, G26 |
| G38 | Verus proofs: orientation and circle predicates | geom-num | M | C | G37, G28 |
| G39 | Verus proofs: plane-based and implicit-point predicates | geom-num | L | C | G37, G29 |
| G40 | Filter-bound certificates for every filter | geom-gen, CI | M | C | G35 |
| G41 | Kani harnesses for the numbers layer | geom-num | M | C | G14, G26, G24, G22 |
| G42 | Wide-integer variants and WebAssembly benchmark | geom-num | L | C | G28, G29, G18, G14, G41 |
| G43 | Gate 1: ground and numbers proven | gate review | S | Gate | G17, G20, G31, G32, G33, G38, G39, G40, G42 |

### Phase 2: Math types and geometry kinds (27 pieces)

Lanes: A math types, maths functions and every curve and surface kind; B linear algebra, the root engine and the box tree; C NURBS, its reference and the kind laws. Owner actions are in lane Owner; the gate is G70.

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G44 | Rotation representation (owner) | owner action | S | Owner | — |
| G45 | Points, vectors, boxes, frames and typed transforms | geom-math | M | A | G22, G28, G43 |
| G46 | Rational directions and rotations | geom-math | M | A | G45, G44 |
| G47 | Proven-bound sinpi, cospi, atan2, exp and ln | geom-num | L | A | G31, G24, G43 |
| G48 | Frame records, units, epochs and lattices | geom-math | M | A | G45, G25, G47 |
| G49 | Dense linear algebra: decompositions | geom-linalg | L | B | G24, G16, G43 |
| G50 | Dense linear algebra: SVD and eigenvalues | geom-linalg | M | B | G49 |
| G51 | Root finding and quadrature | geom-num | M | B | G31, G43 |
| G52 | Dual numbers and interval Newton operators | geom-num | M | B | G31, G47 |
| G53 | Certified root-isolation engine | geom-num | L | B | G52, G51, G33 |
| G54 | The box tree | geom-bvh | L | B | G45, G33 |
| G55 | Morton-sorted cell index | geom-bvh | S | B | G54 |
| G56 | NURBS basis, evaluation and knot operations | geom-nurbs | L | C | G45, G16 |
| G57 | Reference: exact rational NURBS | geom-test | M | C | G56 |
| G58 | NURBS interpolation and approximation to a stated error | geom-nurbs | M | C | G56, G49 |
| G59 | Bernstein toolkit | geom-nurbs | L | C | G56, G31, G27 |
| G60 | Curve and surface traits, closed enums and the kind contract | geom-curve, geom-surface | M | A | G45, G56, G31 |
| G61 | Analytic curve kinds | geom-curve | L | A | G60 |
| G62 | Composite, trimmed, bulge, offset and pcurve kinds | geom-curve | M | A | G61, G58 |
| G63 | Spiral kinds with certified evaluation | geom-curve | L | A | G61, G47, G51 |
| G64 | Spiral conversion and transforms | geom-curve | M | A | G63, G58 |
| G65 | Analytic and swept surface kinds | geom-surface | L | A | G60, G61 |
| G66 | Offset surface kind | geom-surface | M | A | G65, G58, G59 |
| G67 | Analytic and transcendental enclosures | geom-curve, geom-surface | M | A | G65, G63, G31, G47 |
| G68 | Certified derivative bounds for every kind | geom-curve, geom-surface | M | A | G67, G59 |
| G69 | Law catalogue for curves, surfaces and NURBS | geom-test | M | C | G62, G65, G66, G64, G57, G68 |
| G70 | Gate 2: math types and kinds proven | gate review | S | Gate | G46, G48, G50, G53, G55, G69 |

### Phase 3: Topology, names, contract and native format core (48 pieces)

Lanes: A topology, contract, checker, facade, primitives and queries; B the canonical encoder, names, history, attributes, resolution, fault injection and proofs; C native format core, operation log, fixtures, replay, oracle tooling and the reference runner. Owner actions are in lane Owner; the gate is G118.

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G71 | Time budgets (owner) | owner action | S | Owner | — |
| G72 | Reference machine and kernel shares (owner) | owner action | S | Owner | — |
| G73 | Who searches for patents (owner) | owner action | S | Owner | — |
| G74 | Patent search: persistent naming and reference resolution (owner) | owner action | S | Owner | G73 |
| G75 | Copyleft tools as CI oracles (owner) | owner action | S | Owner | — |
| G76 | Arena indices in name order (owner) | owner action | S | Owner | — |
| G77 | CBOR tag and file name (owner) | owner action | S | Owner | — |
| G78 | NameScheme v1 (owner) | owner action | S | Owner | — |
| G79 | Hash functions (owner) | owner action | S | Owner | — |
| G80 | Topology arenas with typed indices | geom-topo | L | A | G45, G60, G70 |
| G81 | Builder, Euler operators and attributes | geom-topo | L | A | G80, G76 |
| G82 | Operation contract and error types | geom-op | M | A | G33, G70 |
| G83 | Budgets in the operation contract | geom-op | M | A | G82, G71 |
| G84 | Checker framework, groups 1 and 4 | geom-check | M | A | G81, G82 |
| G85 | Checker groups 2 and 3 for primitives | geom-check | M | A | G84 |
| G86 | Canonical encoder and safe hash | geom-op | M | B | G79, G70 |
| G87 | Persistent names: NameId, TopoName and roles | geom-op, geom-name | M | B | G81, G86, G78, G74 |
| G88 | History records and their laws | geom-op, geom-test | M | B | G87 |
| G89 | Reference resolution | geom-name | M | B | G88 |
| G90 | Attribute definitions and classes | geom-topo, geom-name | M | B | G88 |
| G91 | Deterministic CBOR decoder and text form | geom-format | M | C | G86 |
| G92 | Record layer, byte sources and range checks | geom-format | M | C | G91 |
| G93 | Native format specification v1 | geom-format | M | C | G92, G60, G80, G87, G48, G77 |
| G94 | Body records | geom-format | L | C | G93, G81 |
| G95 | Name, history, recipe and attribute records | geom-format | M | C | G94, G89, G90 |
| G96 | Operation log and canonical output hash | geom-format, geom-test | M | C | G95, G82 |
| G97 | Fixture format, runner and minimiser | geom-test | M | C | G96, G19 |
| G98 | Determinism matrix for operation logs | geom-test, CI | M | C | G97, G33, G83 |
| G99 | Format museum | geom-test | S | C | G98, G20 |
| G100 | Facade, operation schemas and generated interfaces | engine-geometry | L | A | G82, G89 |
| G101 | Primitives and transforms | geom-make | M | A | G100, G85, G88 |
| G102 | Core queries | geom-topo, geom-check | M | A | G101 (tests) |
| G103 | Contract laws and completeness gate | geom-test | M | A | G101, G15, G97 |
| G104 | Batch runner | geom-op | M | A | G82, G33 |
| G105 | Content hashes and cache keys | geom-format | M | A | G94 |
| G106 | Reference mode and fast-path equality | CI | S | C | G105, G98 |
| G107 | Reference runner and nightly timings | geom-bench, CI | S | C | G18, G72, G70 |
| G108 | Containers and trust on open | geom-format, geom-test | M | C | G94, G84 |
| G109 | PruneHistory operation | geom-name | S | B | G95 (tests), G101 (tests) |
| G110 | Checker fault injection: first catalogue | geom-test | M | B | G85, G101 |
| G111 | Topology model proof and model-based tests | geom-test, geom-topo | L | B | G81 |
| G112 | Kani harnesses: arenas, builder and native reader | geom-test, geom-topo, geom-format | M | B | G81, G41, G92 |
| G113 | Oracle runner and triage tooling | geom-test, CI | L | C | G97, G10, G75 |
| G114 | Corpus register and fetcher | geom-test | M | C | G97 |
| G115 | Patent register | geometry/PATENTS.md | M | B | G15, G73, G70 |
| G116 | Adversarial generators and sequence fuzzing | geom-fuzz | L | B | G101, G97, G14, G110 |
| G117 | Playbooks: add an operation or a query | geometry/AGENTS.md | S | A | G103, G17 |
| G118 | Gate 3: topology, names, contract and format core proven | gate review | S | Gate | G99, G102, G104, G106, G107, G108, G109, G111, G112, G113, G114, G115, G116, G117 |

### Phase 4: Exact core: polyhedra, flat booleans and exact meshes (38 pieces)

Lanes: A the exact arrangement engine, flat and n-ary booleans, cells and batch cuts; B checkers, references, naming fixtures, local operations and the first law catalogue; C exact meshes, distance predicates and Delaunay. Owner actions are in lane Owner; the gate is G156.

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G119 | Patent search: slicing and toolpaths (owner) | owner action | S | Owner | G73 |
| G120 | Committed planes and how bounds add up (owner) | owner action | S | Owner | — |
| G121 | Predicate step ladder (owner) | owner action | S | Owner | — |
| G122 | Patent search: booleans, cellular models and batch cutting (owner) | owner action | S | Owner | G73 |
| G123 | Plane type, canonical planes and arrangement predicates | geom-poly | M | A | G29, G30, G81, G122, G118 |
| G124 | Predicate step ladder | geom-num, geom-poly | M | A | G123, G42, G121 |
| G125 | Plane-based faces into convex polygons | geom-poly | M | A | G123 |
| G126 | Polygon clipping and local cuts | geom-poly | M | A | G125 |
| G127 | Sparse winding vectors, tracing and selection | geom-poly | L | A | G126 |
| G128 | Adaptive subdivision and the parallel task tree | geom-poly | L | A | G127, G98 |
| G129 | Canonical rebuild into bodies | geom-bool | L | A | G128, G84 |
| G130 | Operands, selections and the boolean schema | geom-bool | M | A | G129 |
| G131 | N-ary cut and union entry points | geom-bool | M | A | G130 |
| G132 | Boolean names and history | geom-bool | M | A | G130, G88 |
| G133 | Arrangement certificate and transactions | geom-bool | S | A | G129 |
| G134 | Checker groups 2 and 3 for plane-based bodies | geom-check | M | B | G85, G29, G123 (tests) |
| G135 | Checker fault injection for plane-based bodies, and the second checker | geom-test | M | B | G134, G133, G110 |
| G136 | References: flat booleans, classification, mass and contact | geom-test | L | B | G97, G118 |
| G137 | Cells, rooms and space boundaries | geom-bool | M | A | G132 |
| G138 | Batch cutting | geom-bool | M | A | G132, G133 |
| G139 | Arrangement cache and local re-cut | geom-bool | L | A | G138, G105 (tests) |
| G140 | Polyhedral corpus and flat boolean gates | geom-test, geom-bench | M | A | G137, G139, G136, G114 |
| G141 | Naming and reference fixtures | geom-test | M | B | G132, G89, G97 |
| G142 | Plane commit routine | geom-poly | M | B | G123, G31, G120 |
| G143 | Per-entity error-bound bookkeeping | geom-topo, geom-op | S | B | G88, G142 (tests), G120 |
| G144 | Local-operation driver and topology-change taxonomy | geom-bool, geom-op | L | B | G130, G142, G143 |
| G145 | Placed instances | geom-topo | M | C | G80, G102, G118 |
| G146 | Exact mesh data model and checker group | geom-mesh | L | C | G123, G87 |
| G147 | Exact point classification on meshes | geom-mesh | M | C | G146, G55, G127 |
| G148 | Mesh boolean and run history | geom-mesh | M | C | G147, G88 |
| G149 | Slices, hulls and faceted B-rep | geom-mesh | M | C | G148, G119 |
| G150 | Minkowski sums of flat solids | geom-mesh | S | C | G149 |
| G151 | Exact distance and contact predicates | geom-num, geom-mesh | M | C | G27, G146 |
| G152 | Polyhedral envelope with integer corners | geom-mesh | M | C | G146, G55, G29 |
| G153 | Exact constrained Delaunay triangulation | geom-mesh | M | C | G30, G45, G118 |
| G154 | Oracles: Manifold, CGAL and mesh booleans | geom-test | M | C | G113, G148, G140 |
| G155 | Law catalogue: numbers, frames, booleans, cells, mass, names and formats | geom-test | M | B | G136, G140, G95, G102, G131 |
| G156 | Gate 4: exact core proven | gate review | S | Gate | G124, G135, G141, G144, G145, G150, G151, G152, G153, G154, G155 |

### Phase 5: Certified intersections (25 pieces)

Lanes: A projection, curve pairs and closed-form surface pairs; B general surface intersection and degenerate cases; C acceptance harness, oracles, curved checker and truth corpus. Owner actions are in lane Owner; the gate is G181.

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G157 | SMLib as a CI oracle (owner) | owner action | S | Owner | — |
| G158 | Intersection result types and canonical order | geom-intersect | S | A | G60, G53, G118 |
| G159 | Bit budgets for exact algebraic decisions | geom-num, geom-intersect | M | A | G27, G59, G118 |
| G160 | Intersection acceptance harness | geom-test | L | C | G158, G113, G114 |
| G161 | Intersection oracles: SMLib and geomdl | geom-test | M | C | G160, G157 |
| G162 | Projection and inversion | geom-intersect | M | A | G158, G67, G68 |
| G163 | Curve-curve intersection and curve self-intersection | geom-intersect | L | A | G162, G159 |
| G164 | Curve-surface intersection | geom-intersect | L | A | G163 |
| G165 | Closed-form SSI: planes and natural quadrics | geom-intersect | L | A | G164 |
| G166 | Closed-form SSI: general quadric pairs | geom-intersect | L | A | G165 |
| G167 | Closed-form SSI: tori, revolutions and extrusions | geom-intersect | M | A | G165 |
| G168 | Implicit-parametric tracer and level curves | geom-intersect | L | B | G165 |
| G169 | General SSI 1: subdivision, loop freedom, start points | geom-intersect | L | B | G164, G59 |
| G170 | General SSI 2: certified marching | geom-intersect | L | B | G169 |
| G171 | Tangent, singular and coincident analysis | geom-intersect | L | B | G170 |
| G172 | Bound propagation across inputs | geom-intersect | S | B | G171 |
| G173 | Poles, seams and knot lines | geom-intersect | L | B | G170, G168 |
| G174 | Exclusion of intersections known by construction | geom-intersect | M | B | G171 |
| G175 | Procedural intersection curve with certified fit | geom-curve, geom-intersect, geom-format | M | B | G170, G58, G94 |
| G176 | Surface self-intersection | geom-intersect | M | B | G174, G163 |
| G177 | Checker groups 2 and 3 for curved geometry | geom-check | M | C | G134, G162, G176 |
| G178 | Curved refutation checker | geom-test | M | C | G135, G177 |
| G179 | Closed-form truth corpus | geom-test | M | C | G97, G65, G118 |
| G180 | Law catalogue for intersections | geom-test | M | C | G69, G172, G160 |
| G181 | Gate 5: certified intersections proven | gate review | S | Gate | G161, G166, G167, G173, G175, G178, G179, G180 |

### Phase 6: 2D drafting and sketches (52 pieces)

Lanes: A exact 2D predicates, the arrangement, regions, booleans and offsets; B edits, snaps, dimensions, hatches, the 2D corpus and the 2D and sketch laws; C the sketch solver, its commits and certificates, and profiles. Owner actions are in lane Owner; the gate is G233.

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G182 | Patent search: clash, clearance and continuous collision detection (owner) | owner action | S | Owner | G73 |
| G183 | Patent search: constraint solving and sketch diagnosis (owner) | owner action | S | Owner | G73 |
| G184 | Pattern and linetype library process (owner) | owner action | S | Owner | — |
| G185 | Ellipses in 2D (owner) | owner action | S | Owner | — |
| G186 | Depth-one 2D numbers (owner) | owner action | S | Owner | — |
| G187 | 2D predicates: side and order on carriers and depth-one points | geom-num | L | A | G27, G34, G186, G181 |
| G188 | 2D predicates: circles and rational-centre arcs | geom-num | M | A | G187 |
| G189 | 2D predicates: two-radical comparisons | geom-num | M | A | G188 |
| G190 | Exact conic predicates | geom-num | L | A | G189, G185 |
| G191 | Reference: 2D arrangement in one-root numbers | geom-test | L | C | G97, G181 |
| G192 | Carriers, implicit points and commits | geom-2d | L | A | G188, G46, G87 |
| G193 | Tangent constructions | geom-2d | L | A | G192, G190 |
| G194 | Drafting entity model | geom-2d | M | A | G192, G62, G81 |
| G195 | Spline and spiral contacts in 2D | geom-2d | M | A | G192, G163 |
| G196 | Arrangement of lines, arcs and conics | geom-2d | L | A | G194, G190 |
| G197 | Arrangement with certified curves | geom-2d | M | A | G196, G195 |
| G198 | Snaps, tracking and the 2D index | geom-2d | M | B | G194, G89, G54 |
| G199 | Arrangement updates and point location | geom-2d | M | A | G196, G198 |
| G200 | Region finding | geom-2d | M | A | G199 |
| G201 | Drafting entities to bodies and back | geom-2d | M | A | G196, G88 |
| G202 | 2D booleans | geom-2d | M | A | G199, G201 |
| G203 | 2D offsets | geom-2d | L | A | G193, G197, G202 |
| G204 | Trim, extend, break, join and cleanup | geom-2d | M | B | G194, G199 |
| G205 | Fillet, chamfer and blend curves | geom-2d | M | B | G193, G204 |
| G206 | Transform and length edits | geom-2d | M | B | G194, G195, G51 |
| G207 | 2D distance and clearance | geom-2d | M | B | G198, G195, G190, G182 |
| G208 | Blocks, inserts, clipping and viewports | geom-2d | M | B | G202, G198, G46 |
| G209 | Dimensions, leaders and text anchors | geom-2d | M | B | G208 |
| G210 | Hatches and linetypes | geom-2d | M | B | G200, G206, G209 |
| G211 | Our own pattern and linetype library | geom-2d (data) | S | B | G210, G184 |
| G212 | Wide polylines and snap rounding | geom-2d | M | B | G203, G199 |
| G213 | Spline fitting and biarcs | geom-2d | M | B | G58, G193 |
| G214 | 2D corpus, oracles and fuzzing | geom-test, geom-fuzz | M | B | G202, G203, G191, G113 |
| G215 | Deterministic sparse linear algebra | geom-linalg | L | C | G49, G181 |
| G216 | Sketch planes and 2D-3D maps | geom-sketch | M | C | G48, G46, G181 |
| G217 | Sketch model and equation framework | geom-sketch | L | C | G52, G62, G183, G181 |
| G218 | Constraint catalogue: points, lines, circles and arcs | geom-sketch | L | C | G217 |
| G219 | Constraint catalogue: ellipses and elliptical arcs | geom-sketch | M | C | G218 |
| G220 | Constraint catalogue: splines, G2 and G3 | geom-sketch | L | C | G218, G58 |
| G221 | Structural analysis | geom-sketch | M | C | G217 |
| G222 | Block solver and continuation | geom-sketch | L | C | G215, G218, G221 |
| G223 | Rigid clusters, generic in D | geom-sketch | L | C | G221, G222 |
| G224 | Degrees of freedom and conflict diagnosis | geom-sketch | L | C | G222, G50 |
| G225 | Commit plan and exact incidences | geom-sketch | L | C | G224, G188 |
| G226 | Commit checks: root choices, residual bounds and the ceiling | geom-sketch | M | C | G225 |
| G227 | Chained Krawczyk certificates | geom-sketch | M | C | G226 |
| G228 | Dragging | geom-sketch | M | C | G226 |
| G229 | Profiles from sketches (geom-profile) | geom-profile | L | C | G227, G216, G200, G177 |
| G230 | Unsolved sketches and project datum systems | geom-sketch, geom-profile | M | C | G229, G228 |
| G231 | Sketch corpus, oracles and exact-rank spike | geom-test, geom-fuzz | M | C | G224, G113 |
| G232 | Law catalogue: 2D and sketches | geom-test | M | B | G191, G214, G231 |
| G233 | Gate 6: 2D drafting and sketches proven | gate review | S | Gate | G205, G207, G211, G212, G213, G219, G220, G223, G230, G232 |

### Phase 7: Curved modelling: booleans, sweeps, blends and offsets (51 pieces)

Lanes: A the curved boolean and B-rep queries; B sweeps, lofts, networks and the thread toolkit; C fillets, corners, the Class-A corpus and the law catalogue; D offsets, skeletons, roofs and build-ups, drafts, face edits, swept volumes, the stair toolkit and one research spike. Owner actions are in lane Owner; the gate is G284.

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G234 | Patent search: straight-skeleton roofs (owner) | owner action | S | Owner | G73 |
| G235 | Patent search: shells, offsets and thickening (owner) | owner action | S | Owner | G73 |
| G236 | Patent search: Class-A blends and variable fillets (owner) | owner action | S | Owner | G73 |
| G237 | Patent search: direct face edits (owner) | owner action | S | Owner | G73 |
| G238 | Toolkit homes before a second program (owner) | owner action | S | Owner | — |
| G239 | What the Sweep kind stores (owner) | owner action | S | Owner | — |
| G240 | 576- and 768-bit predicates (owner) | owner action | S | Owner | — |
| G241 | close_near_contacts cap (owner) | owner action | S | Owner | — |
| G242 | Curved builder data model, broad phase and contact rule | geom-bool | L | A | G130, G54, G171, G175, G156, G233 |
| G243 | Interferences, paves and common blocks | geom-bool | L | A | G242 |
| G244 | Face splitting: flat and analytic carriers | geom-bool | L | A | G243, G167, G166 |
| G245 | Face splitting: NURBS and procedural carriers | geom-bool | L | A | G244, G173 |
| G246 | Certified ray classification | geom-bool | L | A | G244 |
| G247 | Queries: classify a point and certified mass properties | geom-bool, geom-topo | M | A | G246, G179, G129 |
| G248 | Curved path integration and Tolerant operands | geom-bool | L | A | G245, G246, G132, G133 |
| G249 | close_near_contacts repair operation | geom-bool | M | A | G242, G142, G241 |
| G250 | Research spike: certified winding number for trimmed NURBS | geom-bool | M | D | G246 |
| G251 | Wide integers I576 and I768 | geom-num | M | D | G240, G156, G233 |
| G252 | Curved boolean corpus and gates | geom-test, geom-bench | L | A | G248, G179, G178 |
| G253 | Surface extension | geom-surface | M | B | G65, G68, G156, G233 |
| G254 | Frame rules and laws | geom-surface | L | B | G61, G47, G68, G239, G156, G233 |
| G255 | Extrude and revolve | geom-sweep | M | B | G253, G144, G200 (tests) |
| G256 | General sweeps and swept disks | geom-sweep | L | B | G254, G255, G175 |
| G257 | Rail and helical sweeps | geom-sweep | L | B | G256 |
| G258 | geom-thread toolkit | geom-thread | M | B | G257, G238 |
| G259 | Lofts and swept blends | geom-sweep, geom-nurbs | L | B | G254, G58, G176 |
| G260 | Network surfaces, Coons and N-sided fills | geom-sweep, geom-surface | L | B | G259, G163 |
| G261 | Developable lofts | geom-sweep | M | B | G259 |
| G262 | Blend feasibility and preview query | geom-blend | L | C | G144, G171, G68, G156, G233 |
| G263 | Constant fillets: analytic blends and chamfers | geom-blend | L | C | G262, G248 |
| G264 | Constant fillets: rolling-ball blends | geom-blend, geom-surface | L | C | G263, G168 |
| G265 | Constant fillets: overflows and clusters | geom-blend | L | C | G264 |
| G266 | Variable-radius blends and trimlines | geom-blend | L | C | G265, G236 |
| G267 | Class-A Gn sections and bridge blends | geom-blend | L | C | G266, G260 |
| G268 | Vertex blends: setbacks and sphere corners | geom-blend | L | C | G265, G260 |
| G269 | Vertex blends: remaining corner classes | geom-blend | L | C | G268 |
| G270 | Surface offsets with certified caches | geom-offset | L | D | G66, G253, G235 |
| G271 | Exact weighted straight skeleton | geom-skeleton | L | D | G142, G196, G251, G234 |
| G272 | Roof generation | geom-skeleton | M | D | G271, G203 |
| G273 | 3D skeleton: four-plane event predicate | geom-num | M | D | G27, G251 |
| G274 | 3D skeleton: vertex splits and event simulation | geom-skeleton | L | D | G273, G271 |
| G275 | Body offset, shell and thicken: flat bodies | geom-offset | L | D | G274, G144, G235 |
| G276 | Roof build-ups by thickening | geom-offset | M | D | G272, G275 |
| G277 | Body offset, shell and thicken: curved bodies | geom-offset | L | D | G275, G270, G248 |
| G278 | Draft | geom-offset | L | D | G253, G168, G144 |
| G279 | Face edits and defeaturing | geom-offset | L | D | G260, G144, G253, G237 |
| G280 | Swept volumes: exact routes | geom-sweep | L | D | G257, G248, G131 |
| G281 | geom-stair toolkit | geom-stair | M | D | G257, G259, G238 |
| G282 | Chapter 13 corpus and Class-A corpus | geom-test | M | C | G267, G277, G114 |
| G283 | Law catalogue: sweeps, blends, offsets, manufacturing and curved mass | geom-test | M | C | G252, G277, G280, G282, G247 |
| G284 | Gate 7: curved modelling proven | gate review | S | Gate | G249, G261, G269, G276, G278, G279, G283 |

### Phase 8: Tessellation, spatial index and clash (24 pieces)

Lanes: A tessellation and display; B distance, clash and contact; C healing, reconstruction and volume meshes. Owner actions are in lane Owner; the gate is G308.

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G285 | Contact benchmark licence (owner) | owner action | S | Owner | — |
| G286 | Patent search: simplification, remeshing and subdivision to NURBS (owner) | owner action | S | Owner | G73 |
| G287 | Patent review: edge-first tessellation (owner) | owner action | S | Owner | G73 |
| G288 | Edge-first certified tessellation | geom-mesh | L | A | G146, G153, G68, G162, G177 (tests), G287, G156, G181 |
| G289 | Thin-part check and incremental tessellation | geom-mesh | M | A | G288, G55 |
| G290 | Display boundary and level-of-detail ladder | geom-mesh | S | A | G288 |
| G291 | Display simplifier and meshlets | geom-mesh | L | A | G290, G286 |
| G292 | Analytic closed-form distances | geom-intersect | M | B | G165, G151, G182, G156, G181 |
| G293 | Certified distance for curved bodies | geom-spatial | M | B | G289, G147, G54, G182 |
| G294 | Certified GJK filter | geom-spatial | S | B | G149, G151, G156, G181 |
| G295 | Instance tree for huge assemblies | geom-spatial | M | B | G145, G48, G54, G156, G181 |
| G296 | Clash pipeline | geom-spatial | L | B | G292, G293, G294, G295, G148 |
| G297 | Time of first contact | geom-spatial | M | B | G151, G54, G182, G285, G156, G181 |
| G298 | Mesh healing | geom-mesh | M | C | G148, G147, G156, G181 |
| G299 | Reconstruct by winding number | geom-mesh | L | C | G298, G128, G151 |
| G300 | Envelope-bounded decimation and remeshing | geom-mesh | M | C | G152, G286, G156, G181 |
| G301 | Volume mesher design spike | geom-mesh | M | C | G152, G156, G181 |
| G302 | Volume mesher: insertion | geom-mesh | L | C | G301, G147 |
| G303 | Volume mesher: improvement, tags and regions | geom-mesh | L | C | G302, G298, G137 (tests) |
| G304 | Mesh offsets, hollow and thicken | geom-mesh | L | C | G152, G151, G302 |
| G305 | Reconstruct by envelope remesh | geom-mesh | S | C | G303, G299 |
| G306 | Prism and hexahedral meshing of 2.5D bodies | geom-mesh | M | C | G153, G255 (tests), G156, G181 |
| G307 | Law catalogue for meshes, spatial and contact | geom-test | M | B | G296, G300, G136 |
| G308 | Gate 8: tessellation, spatial index and clash proven | gate review | S | Gate | G291, G297, G304, G305, G306, G307 |

### Phase 9: Free-form: subdivision, fields and conversions (47 pieces)

Lanes: A subdivision surfaces, their conversions and face splitting; B fields; C conversions, the one boolean across representations and swept volumes as fields; D parameterisation, panels, Class-A analysis and reverse engineering. Owner actions are in lane Owner; the gate is G355.

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G309 | Patent search and legal check: general swept volumes (owner) | owner action | S | Owner | G73 |
| G310 | Patent search: point-cloud fitting and registration (owner) | owner action | S | Owner | G73 |
| G311 | Patent search: panelisation (owner) | owner action | S | Owner | G73 |
| G312 | USD exchange home (owner) | owner action | S | Owner | — |
| G313 | Patent and licence clearance for chapter 16 methods (owner) | owner action | S | Owner | G73 |
| G314 | OpenSubdiv oracle driver | geom-test | M | A | G113, G284, G308 |
| G315 | SubD net, options and uniform refinement | geom-surface, geom-subd | M | A | G60, G87, G314 |
| G316 | Creases, darts and local refinement | geom-surface, geom-subd | M | A | G315 |
| G317 | Exact SubD oracle and eigen-tables | geom-gen, geom-surface, geom-test | M | A | G316 |
| G318 | SubD limit surface kind | geom-surface, geom-format | L | A | G317, G68, G94 |
| G319 | Face-varying evaluation and SubD tessellation | geom-subd, geom-mesh | M | A | G318, G288 |
| G320 | Intersections at extraordinary vertices | geom-intersect | M | A | G318, G173 |
| G321 | SubD face intersections with every kind | geom-intersect | L | A | G320, G180 |
| G322 | SubD to B-rep with SubD faces | geom-convert, geom-check | M | A | G321 |
| G323 | SubD to NURBS: packed patches and PCCM | geom-subd, geom-convert | L | A | G322, G286 |
| G324 | Face splitting on subdivision-limit carriers | geom-bool | M | A | G321, G318, G322 (tests) |
| G325 | SubD net editing | geom-subd | L | A | G316, G88 |
| G326 | Field graph, bytecode and node names | geom-field | M | B | G87, G45, G284, G308 |
| G327 | Field interpreter | geom-field | L | B | G326, G47, G52, G33 |
| G328 | Field body contract | geom-field | M | B | G327 |
| G329 | Field opcodes: primitives, blends and shells | geom-field | L | B | G328 |
| G330 | Field opcodes: lattices | geom-field | L | B | G329, G313 |
| G331 | Field mesher: octree and dual contouring | geom-field | L | B | G328, G146 |
| G332 | Field mesher: certificate, names, 2D and slices | geom-field | L | B | G331, G200 |
| G333 | Bodies as fields | geom-convert | L | B | G327, G162, G322, G54 |
| G334 | Field primitive recognition to B-rep booleans | geom-convert | M | B | G326, G248, G255 |
| G335 | Conversion graph and route query | geom-convert | M | C | G319, G322, G332, G333, G334, G323 |
| G336 | One boolean across representations | geom-combine | M | C | G335, G248, G148, G324 |
| G337 | Representation dispatch for modelling operations | geom-combine | M | C | G336, G304, G329, G277 (tests) |
| G338 | Swept volumes as fields: space-time opcode and grid | geom-field | L | C | G332, G327 |
| G339 | Swept volumes as fields: contouring | geom-field | L | C | G338 |
| G340 | Swept volumes as fields: extraction and the general route | geom-convert | L | C | G339, G333, G280, G309 |
| G341 | Discrete parameterisation and flattening | geom-mesh | L | D | G215, G196, G146, G284, G308 |
| G342 | Surface division and pattern mapping | geom-panel | L | D | G341, G168 |
| G343 | Unrolling with strain maps | geom-panel | M | D | G341, G201 |
| G344 | Panelisation: planar quads and exact panels | geom-panel | L | D | G342, G123, G32, G311 |
| G345 | Panelisation: strips and mould reuse | geom-panel | L | D | G344, G343 |
| G346 | Class-A analysis: continuity across edges | geom-analysis | M | D | G68, G284, G308 |
| G347 | Class-A analysis: curvature extremes | geom-analysis | M | D | G346 |
| G348 | Class-A analysis: light lines | geom-analysis | M | D | G346, G168 |
| G349 | Class-A analysis: draft, gap, flush and deviation | geom-analysis | M | D | G346, G293 |
| G350 | Reverse engineering: primitives and quad layout | geom-cloud, geom-mesh, geom-convert | L | D | G335, G32, G215, G310 |
| G351 | Reverse engineering: surface fitting and Poisson | geom-convert | L | D | G350, G58, G315 |
| G352 | Signed heat reconstruction | geom-field | M | D | G298, G332, G215 |
| G353 | Law catalogue for free-form and conversions | geom-test | M | C | G335, G330, G314 |
| G354 | SubD exchange: USD and OBJ | geom-meshio | M | C | G316, G312 |
| G355 | Gate 9: free-form proven | gate review | S | Gate | G325, G337, G340, G345, G347, G348, G349, G351, G352, G353, G354 |

### Phase 10: Survey, terrain and civil (27 pieces)

Lanes: A COGO and TINs; B toposolids, grading and flow; C alignment curve kinds, alignments, corridors and civil compositions; D point clouds. Owner actions are in lane Owner; the gate is G382.

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G356 | Patent search: terrain, corridors and alignments (owner) | owner action | S | Owner | G73 |
| G357 | Alignment curve kinds (owner) | owner action | S | Owner | — |
| G358 | TIN tie order (owner) | owner action | S | Owner | — |
| G359 | Angle units and core COGO | geom-num, geom-2d | M | A | G48, G47, G284 |
| G360 | COGO intersections, arcs and parcels | geom-2d | M | A | G359, G196 |
| G361 | Survey points and 3D linework | geom-2d, geom-curve | S | A | G81, G61, G58, G87, G284 |
| G362 | Exact Delaunay core | geom-terrain | L | A | G153, G30, G146, G358, G356, G284 |
| G363 | Breaklines, crossings and given triangulations | geom-terrain | L | A | G362, G212 |
| G364 | Parallel, chunked and incremental TIN build | geom-terrain | L | A | G363, G88 |
| G365 | TIN surface operations | geom-terrain | L | A | G364, G202 |
| G366 | Contours, cut and fill, volumes | geom-terrain | L | A | G365, G212, G25 |
| G367 | Toposolid | geom-terrain | L | B | G365, G248, G137 |
| G368 | Grading: straight pieces, rounded corners and the envelope | geom-terrain | L | B | G365, G203 |
| G369 | Grading: cut and fill switching, targets and benches | geom-terrain | M | B | G368 |
| G370 | Grading: arcs, helices and skeleton mitres | geom-terrain | L | B | G368, G271 |
| G371 | Transition kinds, gradient and segmented reference curves | geom-curve | M | C | G357, G284 |
| G372 | Spiral intersections with every kind | geom-intersect | L | C | G64, G164, G371 |
| G373 | Alignment model, stations and projection | geom-align | L | C | G63, G162, G48, G87, G371, G356 |
| G374 | Terrain and station reference forms | geom-name, geom-terrain, geom-align | M | C | G365, G373, G89 |
| G375 | Corridor sectioned solid and faceting | geom-sweep | L | C | G371, G256, G123 |
| G376 | Corridor ranges, names and junctions | geom-align | M | C | G375, G248, G373 |
| G377 | Flow, sight and raster DEM input | geom-terrain | M | B | G365 |
| G378 | geom-civil compositions | geom-civil | M | C | G366, G368, G376, G377, G369, G238 |
| G379 | Point-cloud reference queries and snaps | geom-cloud | M | D | G87, G54, G284 |
| G380 | Point-cloud fitting and TIN from a cloud | geom-cloud, geom-terrain | L | D | G379, G32, G58, G364, G310 |
| G381 | Law catalogue and oracles for ground | geom-test | M | C | G366, G373, G113, G380 |
| G382 | Gate 10: survey, terrain and civil proven | gate review | S | Gate | G360, G361, G367, G370, G372, G374, G381 |

### Phase 11: Drawings (24 pieces)

Lanes: A sections, flat hidden lines, classes, checker, references, storage and the incremental engine; B curved contours and curved hidden lines, face maps, perspectives and developed sections. Owner actions are in lane Owner; the gate is G406.

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G383 | Patent search: hidden lines, sections and drawings (owner) | owner action | S | Owner | G73 |
| G384 | Smooth-edge marking (owner) | owner action | S | Owner | — |
| G385 | View predicates beyond 384 bits (owner) | owner action | S | Owner | — |
| G386 | Drawing corpus and benchmark scenes | geom-test, geom-bench | M | A | G81, G114, G284, G308 |
| G387 | View and scene input | geom-draw | M | A | G45, G386, G383 |
| G388 | Exact view predicates | geom-num | L | A | G27, G30, G385, G284, G308 |
| G389 | Exact plane sections of every body kind | geom-draw | L | A | G387, G123, G149, G200 |
| G390 | Closed-form sections and outlines of quadrics, pipes and tori | geom-intersect, geom-draw | L | A | G389, G165, G62 |
| G391 | Certified free-form sections and composite sections | geom-draw | L | A | G390, G170, G248 (tests) |
| G392 | Certified contour generators | geom-intersect | L | B | G390, G168 |
| G393 | Contours near extraordinary vertices | geom-intersect | L | B | G392, G318 |
| G394 | Exact hidden lines for plane-based bodies | geom-draw | L | A | G388, G389, G54 |
| G395 | Bodies that run into each other | geom-draw | M | A | G394, G248 (tests) |
| G396 | Certified hidden lines for curved bodies | geom-draw | L | B | G394, G391, G392 |
| G397 | Projected curve kinds and canonical drawing form | geom-draw | M | A | G387, G62 |
| G398 | Line classes, bands and continuity records | geom-draw, geom-topo | M | A | G394, G384 |
| G399 | Drawing checker and regression | geom-draw, geom-test | M | A | G394, G398, G136 |
| G400 | Visible-face map and shadows | geom-draw | L | B | G396, G199 |
| G401 | Drawing references: dimensions and overrides | geom-draw | M | A | G397, G394, G209 |
| G402 | Incremental drawing engine and parallel views | geom-draw | L | A | G398, G105 (tests) |
| G403 | Perspective and oblique projections | geom-draw | M | B | G396 |
| G404 | Developed and civil sections | geom-draw | L | B | G396, G371, G373 (tests) |
| G405 | Drawing storage, explode and export | geom-draw, geom-format | M | A | G397, G204, G212 |
| G406 | Gate 11: drawings proven | gate review | S | Gate | G393, G395, G399, G400, G401, G402, G403, G404, G405 |

### Phase 12: Native format complete and exchange (38 pieces)

Lanes: A the rest of the native format; B exchange foundations and healing; C IFC; D STEP; E DXF, meshes, clouds, LandXML, 3dm, notes, the conformance harness and its oracles. Owner actions are in lane Owner; the gate is G444.

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G407 | openNURBS licence confirmation (owner) | owner action | S | Owner | — |
| G408 | LandXML terms (owner) | owner action | S | Owner | — |
| G409 | Generating code from STEP and IFC schemas (owner) | owner action | S | Owner | — |
| G410 | Records for meshes, SubD, fields, 2D, display and clouds | geom-format | L | A | G94, G146, G318, G326, G209, G290, G379, G355, G382, G406 |
| G411 | Stored display data and safe cold open | geom-format, geom-mesh | M | A | G410, G108, G291 |
| G412 | Permanence harness: independent reader | geometry/tools/format-reader, geom-test | M | A | G410, G99, G405 |
| G413 | Exchange basics: units, numbers to grid, frames, reports | geom-exchange | M | B | G48, G92, G355, G382, G406 |
| G414 | Hardened XML and zip readers | geom-exchange | M | B | G92, G355, G382, G406 |
| G415 | ISO 10303-21 codec | geom-p21 | M | B | G92, G355, G382, G406 |
| G416 | EXPRESS code generation and validator | geom-gen (output in the format crates) | L | B | G415, G409 |
| G417 | Rounding imported polyhedra | geom-heal | L | B | G413, G298, G142, G134 |
| G418 | ISO 10303-42 geometry mapping | geom-exchange | L | B | G415, G413, G177, G175 |
| G419 | Healing: curve certification and re-intersection | geom-heal | L | B | G418, G171 |
| G420 | Healing: tangent edges, vertices, sewing and orientation | geom-heal | L | B | G419 |
| G421 | IFC sweep mapping and sectioned sweeps | geom-ifc | L | C | G416, G413, G256, G259, G375, G373 |
| G422 | IFC import: profiles, CSG, clipping and openings | geom-ifc | L | C | G416, G417, G138, G413, G421 |
| G423 | IFC import: B-reps, tessellations, mapped items and placements | geom-ifc | L | C | G422, G420 |
| G424 | IFC 4.3 terrain, alignment and corridor mapping | geom-ifc | L | C | G423, G376, G363, G373 |
| G425 | Kind-by-format export matrix | geom-convert, geom-test | M | C | G335, G413 |
| G426 | IFC export | geom-ifc | L | C | G424, G425 |
| G427 | STEP import: B-rep and Tolerant bodies | geom-step | L | D | G418, G420, G416 |
| G428 | STEP import: assemblies, tessellation and properties | geom-step | L | D | G427, G295 |
| G429 | STEP export | geom-step | L | D | G428, G425 |
| G430 | DXF read: 2D curves, hatches, text and thickness | geom-dxf | L | E | G210, G413, G213 |
| G431 | DXF read: blocks, inserts, dimensions and leaders | geom-dxf | L | E | G430, G208, G209 |
| G432 | DXF read: 3D faces, meshes and references | geom-dxf | L | E | G431, G146, G318 |
| G433 | DXF write, binary DXF and code pages | geom-dxf | L | E | G432, G425 |
| G434 | Mesh formats: STL, OBJ, PLY, glTF and 3MF | geom-meshio | L | E | G414, G413, G425 |
| G435 | 3MF manufacturing extensions | geom-meshio | M | E | G434, G330 |
| G436 | Point-cloud exchange rules | geom-exchange, geom-cloud | S | E | G413, G379 |
| G437 | LandXML 1.2 mapping | geom-landxml | M | E | G414, G363, G373, G360, G408 |
| G438 | Rhino .3dm | geom-3dm | L | E | G418, G420, G322, G407 |
| G439 | Decision notes for formats not yet owned | notes | M | E | G355, G382, G406 |
| G440 | Exchange conformance harness | geom-test | M | E | G426, G429, G433, G434, G113 |
| G441 | Oracles: IfcConvert and web-ifc | geom-test | S | E | G440 |
| G442 | Oracle: OpenCascade DRAW | geom-test | M | E | G440 |
| G443 | Oracles: NIST STEP analyser, ezdxf and LibreCAD | geom-test | M | E | G440 |
| G444 | Gate 12: native format and exchange proven | gate review | S | Gate | G411, G412, G435, G436, G437, G441, G442, G443 |

### Phase 13: Scale, truth, bindings, toolkits and the first Stable release (38 pieces)

Lanes: A Spare Construct scenarios, wall-clock gates, bindings, browser builds and the release; B cross-program scenarios; C truth tests and failure catalogues; D certify mode and research; E domain toolkits, placements, 3D sketches and mates, built when a program needs them. Owner actions are in lane Owner; the gate is G482.

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G445 | Patent search: circuit-board pours and clearance (owner) | owner action | S | Owner | G73 |
| G446 | Patent search: sheet-metal bending and unfolding (owner) | owner action | S | Owner | G73 |
| G447 | Gerber and IPC-2581 terms (owner) | owner action | S | Owner | — |
| G448 | AutoCAD truth tests (owner) | owner action | S | Owner | — |
| G449 | Wall-clock merge rule (owner) | owner action | S | Owner | — |
| G450 | Spare Construct scenario generator and calibration | geom-bench | M | A | G386, G18, G444 |
| G451 | Spare Construct target scenarios | geom-bench | L | A | G450, G139, G402, G296, G230, G411, G289 |
| G452 | Wall-clock and scaling gates | geom-bench, CI | M | A | G451, G10, G107, G449 |
| G453 | C interface | engine-geometry-c | M | A | G100, G444 |
| G454 | WebAssembly bindings | engine-geometry-wasm | M | A | G100, G444 |
| G455 | Python bindings | engine-geometry-py | M | A | G100, G444 |
| G456 | Browser builds: threads, memory ceiling and cancellation | geom-pool, engine-geometry-wasm | M | A | G42, G83, G451, G454 |
| G457 | Cross-program scenarios: 2D, clouds, design, civil | geom-bench | L | B | G450, G210, G379, G344, G376 |
| G458 | Cross-program scenarios: mechanical, boards, manufacturing, meshes | geom-bench | L | B | G450, G265, G207, G280, G303, G332 |
| G459 | Preview precision | geom-op, geom-format | S | A | G105, G288, G248, G444 |
| G460 | Revit truth-test harness | geom-test | M | C | G97, G248, G444, CON-A18-F2-P1, CON-A18-F2-P2 |
| G461 | Revit batch: walls, openings, floors and roofs | geom-test | M | C | G460, G272, G139 |
| G462 | Revit batch: stairs, curtain grids, structure, MEP and rebar | geom-test | M | C | G460, G281, G265, G256 |
| G463 | Revit batch: toposolids and Revit's failure cases | geom-test | M | C | G460, G367 |
| G464 | AutoCAD-class drafting truth tests | geom-test | M | C | G433, G205, G200, G214, G448, G444 |
| G465 | Outside failure catalogues as fixtures | geom-test | M | C | G252, G423, G427, G444 |
| G466 | Certify mode: candidates and design note | geom-intersect | M | D | G171, G160, G444 |
| G467 | Certify mode: implementation | geom-intersect | L | D | G466, G320, G175 |
| G468 | Certify mode: local escalation and default policy | geom-intersect | M | D | G467 |
| G469 | Research: certifying field meshes at sharp CSG features | geom-field | L | D | G332, G444 |
| G470 | Research: eigen-tables for dart and corner vertices | geom-surface | M | D | G318, G444 |
| G471 | Research: curved-eave skeletons | geom-skeleton | L | D | G272, G165, G444 |
| G472 | geom-sheet toolkit | geom-sheet | L | E | G277, G261, G343, G256, G446, G444 |
| G473 | geom-board copper operations | geom-board | L | E | G203, G207, G210, G206, G445, G444 |
| G474 | Board exchange: Gerber X2 and X3 | geom-gerber | L | E | G473, G413, G447 |
| G475 | Board exchange: IPC-2581 | geom-ipc2581 | L | E | G473, G413, G447 |
| G476 | STEP AP242 semantic PMI | geom-step | L | E | G429, G444 |
| G477 | Placement variables at D = 3 and D = 6 | geom-sketch | L | E | G223, G222, G224, G444 |
| G478 | 3D sketch entities | geom-sketch | L | E | G477 |
| G479 | Assembly mates and mechanism stepping | geom-assembly | L | E | G477, G297 |
| G480 | Playbooks: formats, representations, toolkits | geometry/AGENTS.md | S | A | G117, G440, G335, G444 |
| G481 | Third-party notices for shipped artefacts | CI | S | A | G16, G100, G453, G454, G455 |
| G482 | First Stable release of engine-geometry | engine-geometry | M | A | G452, G456, G412, G461, G464, G440, G481, G480, G313, G457, G458, G459, G462, G463, G465 |

## 30.5 What the board waits on

**Owner actions.** 52 pieces are decisions or checks only an owner can make. Each is listed below with the pieces that need it directly; section 32 sets out the choices. A piece that waits on one may still be designed and spiked, but it does not merge until the owner action is recorded.

| ID | Owner action | Needed directly by |
| --- | --- | --- |
| G07 | CI hosting and cost | G10 |
| G08 | Crate table and layers | G09 |
| G44 | Rotation representation | G46 |
| G71 | Time budgets | G83 |
| G72 | Reference machine and kernel shares | G107 |
| G73 | Who searches for patents | G74, G115, G119, G122, G182, G183, G234, G235, G236, G237, G286, G287, G309, G310, G311, G313, G356, G383, G445, G446 |
| G74 | Patent search: persistent naming and reference resolution | G87 |
| G75 | Copyleft tools as CI oracles | G113 |
| G76 | Arena indices in name order | G81 |
| G77 | CBOR tag and file name | G93 |
| G78 | NameScheme v1 | G87 |
| G79 | Hash functions | G86 |
| G119 | Patent search: slicing and toolpaths | G149 |
| G120 | Committed planes and how bounds add up | G142, G143 |
| G121 | Predicate step ladder | G124 |
| G122 | Patent search: booleans, cellular models and batch cutting | G123 |
| G157 | SMLib as a CI oracle | G161 |
| G182 | Patent search: clash, clearance and continuous collision detection | G207, G292, G293, G297 |
| G183 | Patent search: constraint solving and sketch diagnosis | G217 |
| G184 | Pattern and linetype library process | G211 |
| G185 | Ellipses in 2D | G190 |
| G186 | Depth-one 2D numbers | G187 |
| G234 | Patent search: straight-skeleton roofs | G271 |
| G235 | Patent search: shells, offsets and thickening | G270, G275 |
| G236 | Patent search: Class-A blends and variable fillets | G266 |
| G237 | Patent search: direct face edits | G279 |
| G238 | Toolkit homes before a second program | G258, G281, G378 |
| G239 | What the Sweep kind stores | G254 |
| G240 | 576- and 768-bit predicates | G251 |
| G241 | close_near_contacts cap | G249 |
| G285 | Contact benchmark licence | G297 |
| G286 | Patent search: simplification, remeshing and subdivision to NURBS | G291, G300, G323 |
| G287 | Patent review: edge-first tessellation | G288 |
| G309 | Patent search and legal check: general swept volumes | G340 |
| G310 | Patent search: point-cloud fitting and registration | G350, G380 |
| G311 | Patent search: panelisation | G344 |
| G312 | USD exchange home | G354 |
| G313 | Patent and licence clearance for chapter 16 methods | G330, G482 |
| G356 | Patent search: terrain, corridors and alignments | G362, G373 |
| G357 | Alignment curve kinds | G371 |
| G358 | TIN tie order | G362 |
| G383 | Patent search: hidden lines, sections and drawings | G387 |
| G384 | Smooth-edge marking | G398 |
| G385 | View predicates beyond 384 bits | G388 |
| G407 | openNURBS licence confirmation | G438 |
| G408 | LandXML terms | G437 |
| G409 | Generating code from STEP and IFC schemas | G416 |
| G445 | Patent search: circuit-board pours and clearance | G473 |
| G446 | Patent search: sheet-metal bending and unfolding | G472 |
| G447 | Gerber and IPC-2581 terms | G474, G475 |
| G448 | AutoCAD truth tests | G464 |
| G449 | Wall-clock merge rule | G452 |

**Not gating.** These pieces do not hold up their phase's gate, because they wait on something outside the layer's proof: G250 (Research spike: certified winding number for trimmed NURBS); G258 (geom-thread toolkit); G281 (geom-stair toolkit); G378 (geom-civil compositions); G438 (Rhino .3dm); G439 (Decision notes for formats not yet owned); G466 (Certify mode: candidates and design note); G467 (Certify mode: implementation); G468 (Certify mode: local escalation and default policy); G469 (Research: certifying field meshes at sharp CSG features); G470 (Research: eigen-tables for dart and corner vertices); G471 (Research: curved-eave skeletons); G472 (geom-sheet toolkit); G473 (geom-board copper operations); G474 (Board exchange: Gerber X2 and X3); G475 (Board exchange: IPC-2581); G476 (STEP AP242 semantic PMI); G477 (Placement variables at D = 3 and D = 6); G478 (3D sketch entities); G479 (Assembly mates and mechanism stepping). They still need the gates before them, and any piece that needs one of them waits for it.

**Other boards.** G460 needs Spare Construct's `CON-A18-F2-P1` (the Revit exporter add-in) and `CON-A18-F2-P2` (the Revit API terms). Map projections come from kit-ops (PROJ, section 17.1), which has no piece on Spare Kit's board yet; no kernel piece waits on it, because frame records take map conversions as data.

**Crates.** The board uses chapter 27's proposed table: `geom-run`, `geom-op`, `geom-linalg` and `geom-bvh` low in the graph, `geom-profile`, `geom-combine`, `geom-panel` and `geom-analysis` among the operations, `geom-exchange`, `geom-p21`, `geom-heal`, `geom-landxml`, `geom-gerber` and `geom-ipc2581` beside the formats, `geom-pool` and the three binding crates beside the facade, the toolkits `geom-thread`, `geom-stair`, `geom-sheet`, `geom-board`, `geom-civil` and `geom-assembly`, and the development crate `geom-gen`. It needs four more amendments, which section 32 lists with chapter 27's:

- **`geom-make`** (layer 9, depending on `geom-run`, `geom-num`, `geom-op`, `geom-math`, `geom-curve`, `geom-surface`, `geom-topo` and `geom-check`): the Make and Transform families of 10.2, primitives, transforms and patterns, which no crate in chapter 27 owns. They cannot live in `geom-topo`, because every operation runs the checker before it returns (principle 4), and `geom-check` sits above `geom-topo`.
- **`geom-p21` on `geom-format`**, for the byte sources it parses.
- **`geom-exchange` on `geom-format`, `geom-p21`, `geom-intersect` and `geom-check`**: its ISO 10303-42 mapping parses Part 21 entities and builds certified Tolerant bodies. Chapter 27 lists its dependencies only "from its job".
- **`geom-heal` on `geom-exchange`**: rounding imported polyhedra uses its numbers-to-grid rules.

Where a chapter and chapter 27 disagree, the board follows chapter 27: linear algebra in `geom-linalg`, not `geom-num`; one box tree in `geom-bvh`, with the Morton index beside it; content hashes and cache keys in `geom-format`; the operation contract, names and history records in `geom-op`; no logic in the facade, so the batch driver sits in `geom-op`, dispatch across representations in `geom-combine`, and preview precision in `geom-op` and `geom-format`; corridor solids in `geom-sweep`; roofs in `geom-skeleton`. Format crates may map the toolkit types they carry, as `geom-gerber` maps `geom-board`'s.

## 30.6 How the chapter drafts were merged

- **Duplicates merged.** Deterministic linear algebra (chapters 15, 16 and 29) is one track in `geom-linalg`. Proven-bound maths (chapters 16 and 17) is one piece. The interval engine and Krawczyk operators (12 and 15) are one numbers track. The licence gate (29) joined the merge gate (28). The benchmark harness and count gates (23 and 28) are one piece. The oracle runner (28 and 29) is one piece, and each oracle joins the phase whose code it checks. The corpus register (28 and 29), the Lifted Surfacing route (13 and 16) and the local re-cut (11 and 23) are each one piece. The box trees of chapters 11, 19 and 23 are one tree, as chapter 27 proposes.
- **Extra-large pieces split.** Constant-radius fillets into analytic, rolling-ball and overflow pieces; variable and Class-A blends into two; vertex blends into two; the 3D skeleton into a predicate and a simulation; body offsets into flat and curved; the IFC and STEP readers by entity group; DXF into three readers and a writer; healing into two; the field opcodes and the field mesher into two each; general swept volumes into a space-time opcode, contouring and extraction; Class-A analysis into four; panelisation and reverse engineering into two each; the 2D predicates into three; board exchange into Gerber and IPC-2581; 3D sketches and mates into placements, 3D sketches and mates; the Spare Construct and cross-program scenarios into two each; dense linear algebra into decompositions and spectral methods; filter certificates into a spike and the roll-out.
- **Merges undone, so sizes are honest.** Operands and selections apart from the n-ary entry points; the constraint catalogue's circles and ellipses; structural analysis and rigid clusters; the sketch commit plan, commit checks and certificates; the intersection harness and its SMLib and geomdl oracles; grading's envelope and its cut-and-fill switching; certify mode's implementation and escalation; the two research items of phase 13; SubD to B-rep and SubD to NURBS; the conformance harness and its three oracle pieces; the topology proof and the Kani harnesses; the IFC sweep mapping and the rest of IFC import.
- **Placed to keep the crate graph acyclic.** NameId hashing uses the canonical encoder in `geom-op`, not `geom-format`'s codec. Surface extension and frame rules in `geom-surface` keep data and evaluation; guide and rail intersection, mitres and fold checks are built in `geom-sweep`. The corridor solid takes sections already placed along a directrix curve, so `geom-sweep` does not need `geom-align`. Roofs are built from skeleton planes with the builder; layered build-ups by thickening are a `geom-offset` piece. The general swept-volume route's extraction sits in `geom-convert`. Terrain and station references are a resolution trait in `geom-name` implemented by `geom-terrain` and `geom-align`. Swept volumes of flat tools use chapter 13's plane-based Minkowski route, not `geom-mesh`'s. Where a piece's tests need a higher crate, the need is marked (tests).
- **Added, because the spine needs them and no chapter listed them:** the workspace scaffold; the grid and tolerance ladder; the NaN-free scalar and maths module; double-double arithmetic; wide integers, including I576 and I768 for the skeleton predicates; the predicate generator; the explicit and implicit predicates; symbolic perturbation; intervals; budgets and cancellation in phase 1; math types, frames and transforms; NURBS basis and fitting; every curve and surface kind; topology arenas, the builder and attributes; checker groups 1 to 4 for primitives and the first fault catalogue; the canonical encoder and safe hash; names, history and resolution; the operation contract and error types; the facade and its schemas; primitives and core queries; the operation log; the reference runner; queries that classify points and give certified mass properties; three law-catalogue pieces; face splitting on subdivision carriers; DXF writing; the C, WebAssembly and Python bindings; the README, `AGENTS.md`, `GLOSSARY.md`, `MAP.json` generator and the playbooks; a gate review for each phase; every owner action; and the first Stable release.
- **Moved to the end.** Sheet metal, circuit-board copper and exchange, semantic PMI, placements, 3D sketches and assembly mates are domain work. Section 26.1 builds them when a program needs them, so they wait in phase 13 and do not hold up the release.

## 30.7 How the board changes

The board is generated from one list. `architecture/30-build-order.json` holds every piece with its done-when and is committed beside this chapter; each piece's GitHub issue, titled `<ID>: <piece>`, carries the same done-when. The generator, `geometry/tools/board/gen.py`, rebuilds the tables of 30.4, or the same tables with a Done when column, and checks that every need exists, that no piece needs a later phase, that every piece inherits its gates and that every need follows the crate graph. CI fails if the tables and the list differ.

Once issues exist, IDs never change. The rule that a piece's ID is higher than its needs holds for this first board only. After that a new piece takes the next free number, and its "Needs" alone say where it fits. A piece found too big is split with `tools/piece.py split`, which makes `G123a` and `G123b`. A new operation, kind, representation, format or toolkit is added by the playbooks of section 26.1, which say what its pieces must contain. Today the board holds 476 pieces, G07 to G482: 412 pieces of work, 52 owner actions and 12 gate reviews, with the first Stable release as the thirteenth gate.
