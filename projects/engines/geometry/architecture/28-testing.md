# 28. Testing and verification

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Evidence: [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md) section D, with [01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md) sections 3 to 6, [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) sections 4, 7 and 9, [03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md) sections 4 and 5, [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md) sections 2, 5 and 9, [06](../../../kit/notes/G01-evidence/06-2d-drafting.md) sections 3 and 5, [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md) sections 1 and 2, [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), the Truck, SolidCraft, csgrs and GeoRust vettings in [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), and the tools and corpora in [10](../../../kit/notes/G01-evidence/10-github-sweep.md).

The promises in section 3 are only as good as the checks behind them, and the evidence shows what happens without them. IfcOpenShell's C++ geometry library has 27 unit tests, and its users find the regressions. Truck's boolean suite is one smoke test that asserts nothing about the result. SolidCraft fuzzes well and still fails 13 of its 96 seeded designs, with per-platform overrides on its expected results. OpenCascade has about 18,300 test files and still documents boolean cases it cannot solve ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), sections A3 and D; [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), Truck and SolidCraft). So in Spare Kit, testing is part of the architecture: every promise has a named check, every check runs by itself in continuous integration (CI), and a failing check blocks the merge.

Four words are used throughout:

- An **oracle** is whatever decides whether a test's output is right.
- A **fixture** is a stored test case: its inputs, as an operation log, and its expected result.
- A **golden hash** is the stored SHA-256 of a fixture's canonical output. Every later run must reproduce it bit for bit.
- To **refute** a bound is to find a point that breaks it. A sampled check can refute a bound; it can never prove one.

## 28.1 Every promise has a check

| Promise | Checked by |
| --- | --- |
| Never an invalid result (principle 4) | The full checker after every operation in every test build, and three independent checks behind it (28.4) |
| Exact predicates (principle 1) | A big-integer oracle, the extreme values, machine-checked filter bounds, proofs of each bit budget (28.6, 28.7) |
| Honest bounds (principles 2 and 3) | Exact references and closed-form answers: Certified(ε) must be within ε; dense refutation sampling (28.3, 28.9) |
| Exactness never lowered unasked; no tolerance ever grows; every operation has a ceiling (sections 6.5, 9) | Contract laws on every operation (28.2) |
| Transactional and cancellable (section 10.1) | Cancellation and budget injection; a failed operation changes no hash (28.2) |
| Deterministic (principle 5) | The determinism matrix, budget tests, the toolchain guard, golden hashes (28.13) |
| Names never break silently (principle 6) | History laws; reference-resolution fixtures (28.3) |
| Pure, with no global state (principle 7) | A ban on statics and thread-locals; every case run twice in one process (28.8, 28.13) |
| No panics, bounded recursion (principle 13) | Lints, fuzzing, bounded proofs, a recursion check, a small WebAssembly stack (28.5, 28.7, 28.8); the stack's size and one call's share of it are not set yet (31.7) |
| Permissive dependencies only (principle 11) | A licence gate on the dependency graph (28.8) |
| Measured (principle 12) | Benchmark gates (23.13, 28.14) |
| Agrees with the world | Reference implementations, outside kernels, corpora (28.9, 28.10, 28.12); never a proprietary program (28.11) |
| Built to last (principle 9) | The format museum; byte-identical round trips (28.13) |
| Grows without gaps (section 26.1) | The completeness gate (28.2) |

**Where the code lives.** Section 27's three development-only crates hold it, with `geom-gen`, a fourth that chapter 27 proposes and G11 has built. `geom-test` holds fixtures, generators, reference implementations, oracle clients and comparisons, the check that no kernel crate depends on a development crate (G11), and, as binaries, the CI tools: the completeness gate, the golden store, the minimiser and the triage tool. `geom-fuzz` holds fuzz harnesses, `geom-bench` benchmarks, and `geom-gen` the generators whose output is committed. Test frameworks such as bolero and proptest may be development dependencies of any kernel crate, so a harness can sit beside the code it tests. Crates used as outside oracles may be development dependencies of `geom-test` only. No kernel crate has a normal dependency on any of them.

## 28.2 Unit tests, property tests and the contract every operation keeps

A unit test checks one function on chosen inputs. A property test states a law that must hold for every input and checks it on thousands of generated inputs; when one fails, the tool shrinks it to the smallest input that still fails. Harnesses are written once with bolero (MIT), whose `check!` harness runs as a property test, as a fuzz target and under the Kani model checker. bolero 0.13's test mode, though, draws a fresh seed for each random input from the operating system, prints a seed only for a failing input, and offers no way to fix them, so two runs try different inputs: a `check!` run is the short fuzz of 28.17, not a fixed-seed property test. So `geom-fuzz` also runs each harness on bytes from `geom-test`'s seeded generator, printing `seed: <hex>` and replaying with `SPAREKIT_SEED`, on every target, WebAssembly included, where bolero's test mode cannot run (G11). proptest (MIT or Apache-2.0) is used where its input strategies fit better ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section D).

Every public function has unit tests and runnable doc examples. Every operation also keeps the contract below, which `geom-test` checks for all of them through the operation schemas of section 22:

- **Transactional.** The operation returns a result that passes the checker, or an error, and either way its inputs' hashes are unchanged. Work units are deterministic, so cancellation and budget exhaustion can be injected at a chosen check point: at every check point for fixtures below a size limit, and at a seeded, logged sample of them for large ones (a wall with 300 openings has too many to try them all). Each injected run must stop with `Cancelled` or `BudgetExceeded`, return no partial result and change no hash.
- **Errors tell the truth.** Every error kind has a test that triggers it. Where a remedy gives a number ("radius at most 4.2 mm"), the test applies it: the operation must succeed at that value and fail one grid step beyond it, or one step of the operation's stated resolution for angles and counts.
- **Exactness is never lost unasked.** A result is no more exact than its least exact input, and its bound is at least the sum of theirs (section 9). A Certified(ε) result has ε at most ε_fit, the operation's ceiling or the caller's request, whichever applies; beyond it the operation returns `ExactnessNotAccepted` (section 22; `ToleranceExceeded` is for Tolerant data only). An Exact input with no request for approximation gives an Exact result, or an error that offers the alternative.
- **No tolerance grows.** On a Tolerant body, no edge or vertex tolerance rises across any operation, and healing only lowers them (section 6.5). OpenCascade's `checkmaxtol` guards against growth beyond a limit ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section D); here any growth fails.
- **The range holds.** Any result that would leave ±2^40 steps of its frame origin is a typed error, tested at 2^40 − 1 and at 2^40 steps.
- **The schema's example runs,** so the documentation and the AI tools generated from it cannot drift from the kernel.

**The completeness gate.** A CI check reads `MAP.json` and the operation registry and refuses a merge when anything section 26.1 requires is missing. It turns that table's checklist into a machine check:

| What is added | The gate requires |
| --- | --- |
| An operation | Unit and property tests, a fuzz target, a benchmark, a test for each error kind, the history laws, its family's laws (28.3), a passing example |
| A query | Tests, and a benchmark that checks its stated cost bound |
| A curve or surface kind | A test of its intersection with every existing kind, directly or by the certified NURBS route, filling one row and column of the kind-by-kind matrix; conversion to NURBS within its stated error; transform tests; a file-tag round trip |
| A representation | A test of every new edge in the conversion graph, both ways, against its stated class and bound; tests of its part in the one boolean |
| An exchange format | Round trips of real files from the corpus; a reader fuzz target |
| A domain toolkit | The operation checklist for each of its operations, and its own laws |

The matrix and the checklists are generated from `MAP.json`, so the gate grows with the kernel.

## 28.3 The laws of every family

Each family has laws checked on generated inputs. This table is the minimum this chapter owns; each family's design chapter adds its own. "Exactly" means checked in exact arithmetic. Everything else is checked within the operation's certified bound, and the bound is refuted, if it is wrong, by an independent dense sample in higher precision.

| Family (crate) | Laws |
| --- | --- |
| Booleans and cells (`geom-bool`, `geom-poly`) | For flat solids, exactly: vol(A ∪ B) + vol(A ∩ B) = vol A + vol B; vol(A − B) = vol A − vol(A ∩ B); A − A is empty; A ∪ A = A; imprint leaves volume unchanged; cells partition the space, their volumes summing to the whole. The membership law below. As shapes: union and intersection commute; grid translations, 90° rotations and mirrors commute with every boolean; a batch cut equals the same cuts one at a time, in any order |
| Intersections (`geom-intersect`) | Symmetric in its arguments; every returned point lies on both inputs within the bound; plane with quadric gives the exact conic of its closed form; completeness exactly as section 12 promises, scored on the exact-truth SSI corpus (28.12) |
| NURBS (`geom-nurbs`) | Evaluation within its bound of exact rational evaluation; knot insertion and degree elevation within their stated bound of the exact result; knot removal reports its true error |
| Curve and surface kinds (`geom-curve`, `geom-surface`) | Conversion to NURBS within its stated error; transforms change a kind only as section 7 says (a circle becomes an ellipse under non-uniform scale); grid translations, 90° rotations and mirrors are exact |
| Sweeps and lofts (`geom-sweep`) | A flat extrusion's volume is area times height, exactly; a revolve's volume matches Pappus's theorem (area times the distance its centroid travels) within its bound; a loft passes through every section within its bound |
| Blends, offsets, shells (`geom-blend`, `geom-offset`) | Fillet faces lie at the radius from both supporting faces within ε_fit; offsets and shell walls lie at their distance within their bound; continuity across blend edges is what was asked for (G1, G2 or G3), stated as a certified number |
| 2D drafting (`geom-2d`) | The boundary identity below, and exact rational areas for polygonal regions; trim, extend and join are idempotent; offsetting by d then by −d returns the original where no feature vanished; arcs stay arcs |
| Sketches (`geom-sketch`) | Solutions satisfy every constraint within the solver's certified residual; degrees of freedom, redundant and conflicting constraints agree with the exact rank of the constraint Jacobian, computed in rationals at seeded generic points; the same solution branch on every target and run |
| Meshes and tessellation (`geom-mesh`) | On closed manifold shells, every edge is used exactly twice, in opposite directions; on sheets, boundary edges are used once and match the face loops; on General bodies, edge uses match the fins around each edge. The stated deviation is a certified bound. Mesh booleans obey the boolean laws exactly |
| Volume meshes (`geom-mesh`) | No inverted tetrahedron, by exact orient3d; the boundary conforms to the body; for flat bodies, tetrahedron volumes sum exactly to the body's volume |
| Spatial, clash, contact (`geom-spatial`) | Every query equals an all-pairs brute force exactly; clearance intervals contain the reference distance; the time of first contact is never later than the true one, checked exactly for translating flat bodies, where it is a rational number |
| Subdivision (`geom-subd`) | Regular faces equal their bicubic B-spline patches exactly; limit points within their certified bound of the exact rational reference of section 16 |
| Fields (`geom-field`) | Interval enclosures contain the point value at every sample, evaluated in higher precision; meshes from fields lie within their certified distance bound |
| Point clouds (`geom-cloud`) | Index queries equal brute force; an Approximate(ε) fit states the deviation an independent recomputation over all points finds |
| Terrain (`geom-terrain`) | The Delaunay empty-circle property, by exact incircle; every breakline is an edge; contour points lie on the surface at their level exactly; net volume equals the exact difference of the two surfaces' volumes to a datum |
| Alignments (`geom-align`) | A clothoid's curvature is linear in arc length within its bound; continuity at joins is as specified; stationing strictly increases; points match Fresnel integrals computed in arbitrary precision |
| Skeletons and roofs (`geom-skeleton`) | Faces partition the footprint exactly; each roof face is planar at its edge's slope, exactly |
| Manufacturing operations | Slices equal the reference's exact plane sections; a swept tool volume contains the tool at every sampled pose and lies within its bound of the true envelope |
| Conversions (`geom-convert`) | Every edge in the conversion graph meets its stated class and bound, both ways |
| The one boolean across representations | Dispatch follows section 9 and the result's class follows its rule; a B-rep case that cannot be certified returns an error offering the mesh or field path, never a silent fallback |
| Numbers and frames (`geom-num`, `geom-math`) | Committing a point never flips a predicate that decided topology, or the implicit form is kept; coarse to fine (×25) is exact and fine to coarse is marked rounded; neighbouring frames shift exactly; map-projection round trips stay within commit rounding |
| Mass properties | Exact rationals for flat solids; certified against closed form for curved ones |
| Names and history (`geom-name`) | The rules of section 20.3, composition included; no duplicate names; untouched entities keep their names; the same names on every target |
| Formats (`geom-format`, exchange crates) | Write, read, write is byte-identical; reading what was written gives back the same value; readers never panic on any bytes |
| Drawings (`geom-draw`) | 28.15 |

Domain toolkits add their own laws through the same gate: a sheet-metal part unfolds and refolds within its bound; a board's copper clearances are decided exactly for lines and arcs.

Three laws need care, because their obvious forms are wrong:

- **Volumes of flat solids are fractions, not integers.** A boolean's vertices are three-plane points in homogeneous integer coordinates (section 6.3), so six times a volume is a ratio of big integers. The tests compute it exactly with dashu (MIT or Apache-2.0). Six times the volume is a whole number only when every vertex is an explicit grid point ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 9); section 10.2 states it more broadly, and the open questions say so.
- **Areas bounded by arcs are not rational.** A circular segment's area involves r²θ, which is transcendental. So regions with arcs are checked on their boundaries: after every boundary piece is split at every crossing, the signed sum of the oriented pieces of A ∪ B and A ∩ B must equal that of A and B. Each piece is keyed by its supporting line or circle and its end points. Those are one-root numbers, a + b√c with rational a, b and c, compared exactly by repeated squaring: the method of CGAL's circle-segment traits, rebuilt in our own words ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 3). The identity implies the area law for every region and never evaluates a transcendental number. Areas are also compared within their certified bounds.
- **Point membership has three answers.** Classification returns In, On or Out. For a point off every input boundary (checked by exact predicates), its class in A op B equals op applied to its classes in A and B. A point On the result must lie On the boundary of an input. Booleans are regularised (slivers that bound no volume are dropped), so no simpler law holds on boundaries.

Persistent names also get fixtures: parametric models (a wall with doors, an extrusion with fillets, a face split by openings) with the expected resolution of every stored reference after each upstream edit. A reference that resolves differently without saying so fails (section 20.4).

## 28.4 The checker, and what stands behind it

Every test build, including the release builds in the determinism matrix, runs all four checker groups of section 8.3 after every operation, and debug builds also hash every input before and after. But the checker is each operation's own postcondition, and it uses the same `geom-intersect` as the operations do (section 27). On its own it is a consistency check: it catches a skipped check or a broken invariant, not a mistake the operation and the checker make together. Three independent checks stand behind it.

1. **Fault injection, both ways.** Valid bodies from the corpus are broken on purpose, one fault at a time: a flipped fin, an open loop, a vertex moved off its surface by just more than its bound, two crossing faces, a wrongly nested shell, a stated bound lowered below the real error. The checker must catch every fault with the right error kind. It must also pass every valid body in the corpus and every closed-form construction, so a checker that rejects too much is caught too. A missed fault is a checker bug, and fuzz results are void until it is fixed. OpenCascade's documentation shows why: its sampled edge check passes edges that are invalid ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 5).
2. **A second checker for flat bodies and meshes,** written from the definitions in big-integer arithmetic with no shared code. It must agree with `geom-check` on the whole corpus.
3. **A refutation checker for curved bodies,** in `geom-test`, sharing no code with `geom-intersect` or `geom-check`. It samples every edge, face and pair of faces densely, in double-double or arbitrary precision, and looks for a vertex off its curve, a pcurve off its 3D curve, two faces crossing away from a shared edge, or a stated bound smaller than the error it finds. It can only ever say "found invalid". Each find is a bug in the operation or in `geom-check`.

Shipped release builds run only the cheap groups (section 8.3). The determinism matrix therefore has a leg in the shipped configuration, and its hashes must equal those of the full-checker leg. That proves the checker never changes a result.

## 28.5 Fuzzing every operation

Fuzzing feeds a program huge numbers of machine-made inputs, steered towards code not yet reached, and watches for anything wrong.

- **Tooling.** Each harness is a plain function in `geom-fuzz`. bolero's `check!` wraps it for property tests and for Kani. cargo-bolero runs it with libFuzzer, LLVM's coverage-guided fuzzer, locally and in nightly runs; it also offers AFL, honggfuzz and Kani engines and a minimiser ([cargo-bolero](https://github.com/camshaft/bolero/tree/7642943b6c1f8a9b5e824dd1cd29b7c8e56068b8/bin/cargo-bolero/src)). Pull requests are fuzzed with ClusterFuzzLite (Apache-2.0) in GitHub Actions. Its Rust guide expects the cargo-fuzz layout ([rust_lang.md](https://github.com/google/clusterfuzzlite/blob/52ecc61cb587ee99c26825a112a21abf19c7448c/docs/build-integration/rust_lang.md)), but cargo-bolero's `build-clusterfuzz` command packs the same libFuzzer-built bolero tests for it, so no cargo-fuzz wrappers are written: the spike G14 showed it, and `geom-fuzz`'s README sets out the layout every target follows, the OSS-Fuzz build check that `build-clusterfuzz`'s start-up scripts need replaced, and the licence the wrappers' `libfuzzer-sys` would bring (NCSA, not on 29.2's list).
- **Targets.** One per operation, one per reader, and one for sequences, which generates whole operation logs. Readers (native format, STEP, IFC, DXF, 3dm, meshes, point clouds) must never panic on any bytes, and what they accept must round-trip.
- **Generators aimed at the hard cases.** Placements snap to coarse sub-grids such as 1 mm or 2.5 mm, so coincident faces, shared edges and tangencies are common, as in SolidCraft's fuzzer; some are then moved by exactly one grid step, so near-coincidences are common too. Coordinates reach the full ±2^40 range: Manifold's fuzzer keeps them between 0.1 and 10 to avoid precision trouble, but an exact kernel must hold everywhere ([01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md), section 4; [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), SolidCraft). Generators also make one-step features, extreme counts (300 openings in one wall), deep operation trees and imported Tolerant bodies.
- **The oracle stack.** A case fails if the kernel panics, runs past its budget without a clean error, returns a body the full checker rejects, changes an input, breaks a law of 28.2 or 28.3, or gives a different hash when rerun with another thread count.
- **Robustness is counted, not only crashes.** A typed error is legal, but an error where a person would expect success is a gap. A fixed, seeded set of generated designs per operation family records the share that succeed, and a new operation version may not lower it, with one exception: a case whose old success is shown to be wrong (a broken law, an oracle difference triaged as ours, or an invalid body) may turn into an honest error, and each such case becomes a fixture.
- **Continuously.** Each pull request fuzzes the crates it changes briefly; nightly runs give every target hours. Fuzz corpora only grow. Every generator prints its seed, so any failure replays exactly; bolero's test mode prints only a failing input's seed (28.2).

## 28.6 Exact predicates and their filters

One wrong sign can corrupt any model, so predicates get the heaviest checking ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 9).

- **A big-integer oracle.** Every predicate, including those on implicit points such as the meeting of three planes, is compared with the same formula in arbitrary-precision integers or rationals (`num-bigint` and `num-rational`, or `dashu`; 29.3). Inputs are random across the full range; exactly degenerate (collinear, coplanar, cocircular) and then moved one step either way; and placed just inside and just outside the f64 filter's error bound, so both paths run.
- **The extreme values.** Section 6.2 asks for a test with every coordinate at ±(2^40 − 1). For predicates on explicit points the suite runs every sign combination (4,096 for orient3d's twelve coordinates). Predicates on implicit points have too many coordinates for that, so they run the configurations that maximise each term of the bit-budget derivation.
- **Algebraic laws.** Swapping two points flips the sign; even permutations keep it; moving every point by one grid vector changes nothing. Symbolic-perturbation variants never return `Zero`, agree with the exact sign whenever it is not `Zero`, and give the same answer for any input order with the same persistent names.
- **Filter bounds are proven, not only checked.** A filter makes one claim: if the f64 value exceeds its bound, its sign is right. The predicate generator of section 6.2 emits that claim, for each predicate, as a problem for two independent tools run out of process: FPTaylor (MIT), whose results can be checked as HOL Light proof certificates ([formal](https://github.com/soarlab/FPTaylor/tree/b5a77cae348400f21f83512210d9f43c4bffb381/formal)), and Gappa (CeCILL or GPL, [README](https://gitlab.inria.fr/gappa/gappa/-/blob/17e2e9a96d8702948f900145fca1715910622e8f/README.md)), which writes proofs that the Rocq proof assistant (formerly Coq) checks. The generated constant must be at least the proven bound. A filter's module header and `MAP.json` call it proven only once its certificate checks; until then it says "checked, not proven". This pipeline is our proposal, and its first build piece is a spike: FPTaylor last changed in 2024 and its HOL Light set-up is old.
- **A second method.** orient2d, orient3d, incircle and insphere are also compared with Shewchuk's adaptive predicates in the `robust` crate (MIT or Apache-2.0) ([03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md), section 4). `robust`'s `orient3d` and `insphere` have Shewchuk's signs, the opposite of `geom-num`'s (6.2), so the comparison negates them. `robust` 1.2.0 is a development dependency of `geom-num`, and `tests/attack_g28.rs` runs the comparison on every input of its other tests: the sign corners, random inputs at every scale, one step from each degeneracy, and inputs just inside and just outside each filter's bound (G28).

## 28.7 Formal proofs

A test checks some inputs; a proof covers all of them. Proofs are used where code is small, critical and integer-based ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section D).

| Tool | What it proves | Why this tool |
| --- | --- | --- |
| Verus (MIT) | The wide integers (`I256`, `I320`, `I384`, and any wider width an owner accepts, such as `I512`, 6.2) compute exact sums, products and comparisons. Each predicate's integer formula equals its determinant and cannot overflow for coordinates below 2^40, so section 6.2's bit budgets become checked proofs. Euler operators on an abstract model of the arena keep every fin's partner and loop links consistent and keep Euler's formula | Strong on unbounded integers and data-structure invariants. Its float support is bit-level classification only, so it is not used for float maths |
| Kani (MIT or Apache-2.0) | Bounded proofs: kernel functions cannot panic or overflow for inputs up to a stated size; arena indices stay in bounds; the NaN-free scalar never lets a NaN through; the native reader never panics on any input up to a stated length; short builder sequences keep the checker's combinatorial rules | A bit-precise model checker, floats included, that runs bolero harnesses unchanged. It over-approximates `sqrt`, `sin` and `cos`, so proofs avoid them |

Proofs must cover the code that ships:

- **Predicates, by generated twins (our proposal; the G36 spike proved it on orient2d).** The predicate generator also emits a Verus-annotated twin of each integer path, and CI checks that the twin, with its annotations removed, is token for token the shipped code. No precedent is known, so the first build piece is a spike. If it fails, the fallback is to write the predicate code inside Verus's `verus!` macro and let Verus erase the proof code at compile time, which makes Verus's `vstd` library and macros build dependencies of `geom-num`. That is a DEPEND inside a kernel crate, which 29.1 allows only for `libm`, so the fallback needs an owner's exception to 29.1, a written audit (29.20, rule 3) and a row in the ledger (an open question). As built (G36): `geom-gen`'s generator `verus-orient2d` writes orient2d's twin, `generated/verus-orient2d.rs`, whose `value` comes from the same emitter function as the shipped one plus a `#[verus_spec]` attribute (Verus's documented syntax for specifications on ordinary Rust functions) and a `proof!` hint before each product; `kernel-dev.yml`'s `verus` job verifies it with a pinned Verus release (for every input on the grid, no step overflows and the result is the description's value), and `geom-gen`'s `tests/verus_twin.rs` checks it is the shipped `value` token for token. orient3d's twin verifies the same way. The decision note, [G36](../../../kit/notes/G36-verus-twins.md), recommends the twins and finds the `verus!` fallback unnecessary; twins of rounded inputs and of the wide integers wait for their Verus models (G37 to G39).
- **Topology, by a model.** Verus proves the Euler operators on an abstract model. Model-based tests then tie the model to the code: they drive the real `geom-topo` arena and the model with the same long random operation sequences and require them to agree at every step.
- **Kani harnesses** sit beside the code under `#[cfg(kani)]`. A harness that cannot finish within its CI budget is narrowed, and the module header says exactly what is proven and what is only tested.

On each pull request, every proof whose call graph reaches a changed function runs, and any change to a foundation crate (28.8) runs all foundation proofs. Every proof runs nightly. Not used: Creusot, whose contracts crate is LGPL, and Prusti, which calls itself a prototype.

## 28.8 Mutation testing, coverage and the merge gate

**Mutation testing** asks whether the tests would notice a bug. cargo-mutants (MIT) makes small changes to the code, such as flipping a comparison or deleting a line, and runs the tests. A change no test catches is a surviving mutant and marks a gap. An equivalent mutant is a change that cannot alter behaviour, so no test could catch it. Each pull request mutates its own diff (`--in-diff`); a weekly run mutates everything, split across machines (`--shard`). The **foundation set** is `geom-num`, `geom-poly`, `geom-check`, `geom-name`, the builder in `geom-topo` and the arrangement module of `geom-2d`: the code where "exact" is promised and everything else rests. There, no mutant may survive unless it is marked equivalent with a written reason, and mutants are judged by tests through each crate's public interface, as in SolidCraft, so the suite keeps its strength when internals are rewritten ([11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), SolidCraft). Elsewhere, no new unexplained survivor may merge.

**Coverage** measures which lines and branches the tests run. It is a floor; mutation is the real gate. With cargo-llvm-cov (Apache-2.0 or MIT), the foundation set needs 100% of lines and branches, and every other kernel crate at least 90% of lines. Branch coverage needs a nightly compiler ([README](https://github.com/taiki-e/cargo-llvm-cov/blob/b405aca51cc81a937e6b566450c7566997dcec21/README.md)), so it runs with a pinned nightly on every pull request that touches the foundation set; the instrumented build is never shipped. A branch may be left uncovered only when a Kani or Verus proof shows it cannot be reached; it is then marked `#[cfg_attr(coverage_nightly, coverage(off))]` with a comment citing the proof. MC/DC (modified condition/decision coverage, which SQLite meets in full) is being re-introduced to the Rust compiler ([#161222](https://github.com/rust-lang/rust/pull/161222)); the foundation set adopts it when it lands. These thresholds are our proposal.

**The merge gate** also runs, on every pull request:

- `cargo-deny`: a licence allow-list over the kernel's dependency graph (permissive licences only; no GPL, AGPL or LGPL), security advisories, and allowed sources.
- `clippy` with section 21's disallowed lists, section 27's denials and the checked-arithmetic denials 27 proposes, failing also on clippy's "does not refer to a reachable function" message, so a ban whose path a toolchain renamed cannot lapse in silence (21); and unsafe code forbidden in every kernel crate, by `#![forbid(unsafe_code)]` or the same lint in the manifest.
- A `no_std` build of every kernel crate for `wasm32v1-none`, a target with no `std` at all, and a `wasm32-unknown-unknown` build of every kernel crate and `engine-geometry`. The development crates are tested on wasm32-wasip1 instead: `getrandom`, which proptest and bolero pull in, has no default `wasm32-unknown-unknown` backend. G09's `kernel-build.yml` runs both builds, each crate on its own.
- A check that rejects statics, thread-locals and lazy globals in kernel crates (principle 7).
- A recursion check: every recursive cycle in a kernel crate must state its depth bound in its module header and have a test at that bound on the small WebAssembly stack. The tool that finds cycles is to be chosen in a spike.
- `MAP.json` regenerated and compared (section 26.3), and `cargo-semver-checks` on `engine-geometry`.

Miri, the Rust interpreter that detects undefined behaviour, runs the test suites weekly, to catch it inside dependencies, which `forbid(unsafe_code)` does not reach.

## 28.9 Reference implementations and closed-form truth

**Reference implementations** are slow, simple versions of the core decisions, kept in `geom-test`, exact, and sharing no code with the kernel. Each is written by a different author: from the definitions, without reading the kernel code, by a different contributor or a different agent session, and reviewed by a kernel owner.

| Reference | Arithmetic | Checks |
| --- | --- | --- |
| Flat booleans by brute force over every pair of faces | Rationals | Booleans, cells, imprint, plane sections |
| The 2D arrangement of lines and arcs | One-root numbers | 2D booleans, offsets, regions, hatching boundaries |
| Point classification and mass properties | Rationals | Classification, volumes, areas, centroids |
| Clash, distance and contact by testing every pair | Rationals | Spatial queries, clearance, first contact |
| NURBS evaluation, knot insertion, degree elevation (de Boor's algorithm) | Rationals; every f64 knot and weight is a binary fraction, so nothing rounds | `geom-nurbs` and every kind's NURBS conversion |
| Subdivision limit points at refined vertices, for cages with whole-number sharpness (section 16) | Rationals | `geom-subd` |

On small cases, every pull request compares the kernel with them: exactly for Exact results, and within the stated bound for Certified ones. This is the strongest oracle Spare Kit owns: exact, and needing no outside tool.

**Closed-form truth.** Shapes with answers known in closed form check the Certified paths: boxes in any combination; two crossing cylinders (the Steinmetz solid, 16r³/3); sphere caps; tori (2π²Rr²); revolves by Pappus's theorem; a ball-ended tool swept along a line (a capsule); the Villarceau circles where a plane cuts a torus; and clothoid points from Fresnel integrals computed once in arbitrary precision. Each Certified(ε) result must lie within ε of the truth. A broken bound is a correctness bug. A bound more than 1,000 times the real error is reported as loose. SolidCraft's 62 analytic cases and csgrs's exact box-volume fuzz oracle are the precedents ([11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), SolidCraft and csgrs).

## 28.10 Differential testing against outside oracles

Differential testing runs the same job through another system and investigates every difference. It is how IfcOpenShell issue #9570 was found: a box with a cylindrical hole came out 12.9% too big in one backend while web-ifc and OpenCascade agreed ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section A3).

The rules:

- **Never trusted as truth, never linked, never shipped.** Permissive Rust crates used as oracles are development dependencies of `geom-test` only. Every other oracle runs out of process: a small harness in a separate repository, under the licence its library requires, built from source at a pinned commit into a container image that CI runs and never publishes. Running a program is not linking it; our repository holds only the jobs and the comparison code.
- **The oracle sees what the kernel saw.** Exact oracles get whole grid steps, which are integers. Floating-point oracles get f64 values written so they read back bit for bit (hex floats, or the shortest decimal that round-trips), in the unit each oracle is tuned for. 1/6400 mm is not a binary fraction (6,400 is 2^8 × 25), so converting to millimetres rounds, and that rounding is added to the comparison bound explicitly.
- **Comparisons.** Exact oracles are compared exactly. Others are compared by volume, area, topology counts where well defined, and two-sided Hausdorff distance (the largest distance from any point of one shape to the other), measured by deterministic dense sampling, so it can only refute. The allowance is both sides' stated bounds plus the conversion rounding, capped at our bound plus ε_fit. Anything beyond is settled by recomputing the case in high precision, never accepted because the oracle's own tolerance grew; OpenCascade's tolerances grow by design ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 2).
- **Triage.** Every difference ends in one of four outcomes, and each becomes a fixture (28.16): our bug; their bug, reported upstream with a reproducer; both within their stated bounds; or undecided, settled by high-precision recomputation. Differences are grouped by a signature (operation, oracle, kind of difference, input family). The nightly run fails only on a new signature, and each triaged signature is recorded once.

The starting set is below. The maintained list, with pinned commits, lives in the oracle catalogue in `geom-test`, linked from `MAP.json`.

| Oracle | Licence; how run | Checks |
| --- | --- | --- |
| CGAL: Nef polyhedra on an exact kernel; 2D arrangements with circle-segment traits | GPL or LGPL by package; out of process | Flat booleans and line-and-arc arrangements, compared exactly |
| OpenCascade, through its DRAW harness | LGPL-2.1 with exception; out of process | B-rep booleans, fillets, offsets, hidden lines (`HLRBRep`), STEP reading; `checkshape` and `bopargcheck` as a second opinion on our STEP output |
| SMLib's NLib layer, through solidcore's harness | Apache-2.0; out of process, built from pinned source, and only after an owner's yes, because the build compiles Numerical Recipes-derived routines (29.5; 29.20, rule 8) | NURBS evaluation, knot insertion and removal, degree elevation. Never the fitting routines, which use a hidden random-number generator, and never booleans or validity ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md)); routines solidcore has not ported need new shims |
| geomdl (NURBS-Python) | MIT; out of process | NURBS basis, evaluation, knot operations |
| IfcConvert (OpenCascade, CGAL and Manifold backends); web-ifc | LGPL-3.0; MPL-2.0; out of process | IFC geometry |
| Manifold; manifold-rust's rational engine | Apache-2.0; C++ out of process, the Rust port as a development dependency | Mesh booleans |
| OpenSubdiv | Apache-2.0 with a changed trademark clause; out of process | Catmull-Clark limit points and derivatives |
| libfive's core; Fidget | MPL-2.0; out of process | Field evaluation, enclosures (Fidget's are f32 and not rigorous, [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md) section 2), meshing |
| JTS's XML conformance suite; Clipper2 (through clipper2-rust); i_overlay; cavalier_contours | EDL-1.0, run out of process; BSL-1.0 and MIT or Apache-2.0, development dependencies | 2D regions and offsets |
| ezpz; SolveSpace; FreeCAD's planegcs | MIT, a development dependency; GPL-3 and LGPL-2.1, out of process | Sketch solutions, degrees of freedom, redundant and conflicting constraints ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 5) |
| ezdxf; LibreCAD | MIT; GPL-2.0; both out of process | DXF meaning; drafting edits |
| spade, startin | MIT or Apache-2.0; MIT; development dependencies | Terrain triangulation |
| PROJ; GeographicLib | MIT-style; MIT; out of process | Map projections and frame conversion |
| Krbn | MIT; out of process | Hidden lines of quadric scenes |

Sources: [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md) sections C and D, [01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md) section 6, [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md) section 4, [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md) sections 1 and 2, [10](../../../kit/notes/G01-evidence/10-github-sweep.md) sections 1 and 2, [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md) GeoRust, [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md); PROJ's [COPYING](https://github.com/OSGeo/PROJ/blob/5fa31c29f0a806568ed7bc76c1a557604db4a804/COPYING); LibreCAD's [LICENSE](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/LICENSE). Oracles run nightly; they take too long for every pull request.

## 28.11 Proprietary programs, and interoperability

Proprietary programs are never oracles for kernel behaviour (29.20, rule 8). Nothing that Revit, AutoCAD or any other proprietary program produces becomes an expected value, a tolerance or a fixture, and no difference from one is triaged as our bug. Expected values come only from exact mathematics, closed forms, the reference implementations (28.9) and the open oracles of 28.10, as section 14.14 also requires for drafting. Rule 8 has no exception, and an API terms check does not make one.

- **Interoperability, only if an owner decides.** Rule 8 leaves one check to an owner: whether our exported files open in proprietary programs. If an owner allows it, a check confirms that our exported IFC and DXF files open in other programs, and reports pass or fail. Nothing read back from those programs becomes an expected value.
- **Spare Construct's truth tests.** Spare Construct's feature list plans geometry and drawing comparisons with Revit (CON-A2-F9-P3 and CON-A2-F9-P5). The kernel does not run them. If owners want them, a decision note with legal advice must amend rule 8 first.
- **Building coverage, in plain language.** The building fixtures follow the functional coverage map in [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md) section B, which states Revit-class needs in our own words: walls with layer joins (L, T, X, mitre), walls attached to roofs and floors, hundreds of openings in one wall, sketched floors with shape edits, footprint roofs with slopes, stairs and railings, curtain grids on curved faces, MEP fittings, beam joins and cutbacks, rebar, and toposolids with sub-divisions and cut and fill. The models are co-op-authored, never client data, and are checked by the laws of 28.3, the reference implementations and closed forms.
- **Revit's public failure reports** become fixtures rebuilt from their descriptions with our own inputs (29.20, rule 9): very short curves, failed lofts, joins that will not hold, voids that will not cut, slab shape edits, toposolid sub-divisions. On each, Spare Kit must succeed or return a typed error naming the cause.
- **AutoCAD-class drafting** (offset, trim, extend, fillet, chamfer, hatch, boundary) is checked by the 2D laws of 28.3, the one-root arrangement reference of 28.9, and LibreCAD and ezdxf as out-of-process oracles (28.10). Its needs reach us only in plain language, never as behaviour observed in AutoCAD (29.21).

## 28.12 Corpora

A corpus is a collection of models the tests run over. A manifest in `geom-test` lists each with its source, version, SHA-256, licence, and whether we may store it. What we may not store is fetched at test time, unmodified, and checked against its hash. The starting set:

| Corpus | Licence | Stored or fetched | Used for |
| --- | --- | --- | --- |
| Manifold's test models and samples | Apache-2.0 | Stored, with notice | Mesh booleans, degenerate cases |
| [Thingi10K](https://github.com/Thingi10K/Thingi10K/blob/cbac53559cfade2c4ffb2ef1ae8cae88462eaed4/README.md), 10,000 models from a 3D-printing site, many of them broken | Code Apache-2.0; each model carries one of ten licences | Fetched; only licences an owner clears | Mesh import, repair, booleans, fuzz seeds |
| EMBER's 1,000 boolean pairs (EMBER is the exact mesh boolean section 11 follows), drawn from Thingi10K and listed in Manifold's [benchmark harness](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/extras/ember_tests/README.md) | List Apache-2.0; meshes per Thingi10K | List stored, meshes fetched | Mesh booleans and their benchmark |
| The [ssi-corpus](https://github.com/jyviko/ssi-corpus/tree/b04c8a6093a8b3505d865d1dd874ec3879ccb1f9): 119 surface-intersection cases with exact truth | MIT or Apache-2.0 | Stored, with notice | Section 12's acceptance gate |
| NIST CAD models and STEP files with PMI ([doi:10.18434/mds2-1452](https://doi.org/10.18434/mds2-1452)) | Not under US copyright (17 U.S.C. §105); may be abroad ([NIST statement](https://www.nist.gov/open/copyright-fair-use-and-licensing-statements-srd-data-software-and-technical-series-publications), read 2026-10-21) | Fetched | STEP import, B-rep validity |
| [step-conformance](https://github.com/dan335/step-conformance/blob/96f30bbe09549ef37dbd0a956d70bd8a183fd5d2/README.md#L287-L292): 27 STEP parts, 19 of exactly known shape | Corpus CC0; code MIT or Apache-2.0; its brepkit adapter is AGPL and never taken | Corpus stored | STEP import scored against exact shape |
| ABC dataset, about a million CAD models | Copyright held by each creator | Not used until an owner decides (29.15) | Import robustness at scale |
| CadQuery's test cases | Apache-2.0 | Recipes rebuilt as our operation logs, stored | Mechanical parts and features |
| OpenSCAD's CSG test files | GPL-2.0 | Not used; pending the owner decision in the open questions (29.8 and 29.15: not read, not vendored) | CSG trees |
| buildingSMART Sample-Test-Files | CC BY 4.0 | Stored, with attribution | IFC |
| IFC 4.3 sample models | CC BY-ND | Fetched, unmodified | Advanced B-rep, alignments, tessellation |
| IFC 4.x alignment atomic test set | None given | Not used until an owner decides (29.15) | Spirals, cant, gradients |
| JTS XML conformance suite | EDL-1.0 | Stored | 2D overlay |
| ezdxf's example DXF files | MIT repository; each file's origin to check | Fetched | DXF reading, 2D drafting |
| PDAL and las-rs sample point clouds | BSD-3, MIT; each file's origin to check | Fetched | Point-cloud reading, indexing, fitting |
| rhino3dm's sample 3dm files | MIT repository; each file's origin to check | Fetched | 3dm reading. openNURBS code is not ported until McNeel confirms in writing (29.7; [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), section 1) |
| KiCad demo boards; Gerber reference files | Unverified | Fetched only after a licence check | Circuit boards |
| Failure catalogues: Truck #57, #114, #134, #135; SolidCraft's vendor notes; GeoRust's boolean issues; IfcOpenShell and web-ifc issues; Manifold's tie-rule changes and the regression fixed in #1898; SMLib's known issues | Rebuilt in our own files from the descriptions | Stored (ours) | Adversarial fixtures |
| Generated buildings, cities, 50 km corridors, parts, assemblies, boards, and Class-A surface sets | MPL-2.0 (ours) | Generated | Scale, determinism, benchmarks |

Sources: [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md) section D, [01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md) section 6, [10](../../../kit/notes/G01-evidence/10-github-sweep.md) section 2, [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md). The manifest, not this table, is the maintained list.

No open corpus of Class-A surfaces (car-body panels with curvature-continuous blends) exists. Spare Kit leads by authoring one: generated and hand-made panel sets with G2 and G3 blends, their continuity measured by certified numbers. Client data is never stored (AGENTS.md), and nor are anonymised real projects, because a building's geometry identifies its project; that conflicts with CON-A2-F9-P2, as the open questions say. A DWG corpus waits on the DWG decision (CON-A18-F3-P2).

## 28.13 Determinism, budgets and golden hashes

Section 21 fixes the determinism matrix; this is what runs through it.

- **The replay corpus** is every fixture plus a seeded operation log per operation family, run on every target, build mode and thread count section 21 lists, plus a leg in the shipped checker configuration (28.4).
- **The canonical output** of each case is hashed with SHA-256: each result body in the native format's canonical encoding, with its names, history, exactness class and diagnostics, or the error with its kind, entities and evidence. Errors must be as deterministic as results, because AI agents replay them.
- **Perturbations that must change nothing:** a test allocator that shifts and fragments memory, so any ordering by address shows up; each case run twice in one process, to catch hidden caches; and shuffled inputs, wherever an operation's contract says order does not matter. As built (G19, `geom_test::golden`): every golden runs twice in one process, each run on a heap shifted and fragmented from its own seed, and must give one digest; the perturbation drives the program's own allocator from outside, by pinning and freeing seeded blocks of every size class in a seeded order, because replacing the global allocator means implementing an `unsafe` trait (principle 13), and its tests prove that code ordering by address sees different orders under it; order-free cases run through a shuffle hook in their own order, reversed and in seeded shuffled orders (`geom-num`'s order-free golden: `ExactSum` and double-double arithmetic).
- **Budgets that cannot break replay.** Only work units, and memory counted in units the kernel tallies itself rather than in target-dependent bytes, may decide a replayable outcome. A time budget is enforced by the caller tripping the cancellation token, so the kernel never reads a clock, and a run stopped that way is never written to the log as the operation's result. The tests check this: memory-budget exhaustion gives the identical error and hash on wasm32 and x86-64; a time-budget stop records nothing; the replay corpus runs with work-unit budgets only. SolidCraft's attempt budget, which gives up after a fixed number of tries rather than on a clock, is the precedent ([11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), SolidCraft). Section 10.1 lists a time budget without saying how it is kept out of results; the open questions ask the owner to confirm this reading.
- **Golden hashes are stored per kernel edition and per operation version.** Within an edition, a Stable operation's golden hash never changes (section 26.2). A changed hash is a bug, or an improvement that ships as a new operation version with new goldens beside the old. A pull request that edits a golden says which, and a kernel owner approves it. Before the first edition there is no edition to version by: the golden store (G19, `geom-test/golden/store.json`) holds every golden in one open edition, `pre-edition`, with the record of every change, and how they may change until the first edition is the owner's (32.12). `kernel-golden.yml` refuses a changed digest without its record, a removed golden and any change to a frozen edition, and holds every other edit to an existing golden until a kernel owner approves the pull request's head commit. The first goldens already wait on a kernel owner's approval: `geom-num`'s, which G02 set and G05 changes for the move to CORE-MATH, and the new ones of `geom-math` (G03), `geom-run` (G04), `geom-bench`'s output hashes (G06), G25's tests and the seeded generator (G32). All of them are SHA-256: G03, G04, G25 and G32 moved from 64-bit FNV-1a over the same recorded bytes before merging.
- **The toolchain guard.** `rust-toolchain.toml` at the repository root pins the compiler (Rust 1.99.0, added by G09) and `Cargo.lock` is committed ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 9). Any change to either, or to vendored numeric code, runs every golden and the whole format museum across the matrix, and must leave every hash unchanged. `kernel.yml`, `kernel-build.yml` and `geom-bench`'s `toolchain.txt` repeat the pin, and the scaffold and `geom-bench` tests keep them equal (G06, G09). `geom-bench`'s WebAssembly fuel host is a second workspace with its own `Cargo.lock` (Wasmtime =49.0.0), which this guard does not watch yet, and `geom-num` declares `rust-version = "1.86"`, which no CI leg builds (31.7). Every third-party action in `kernel.yml` (G02) and `kernel-build.yml` (G09) is pinned to a full commit; `geom-bench`'s `kernel-bench.yml` (G06) still names tags and a branch (`actions/checkout@v4`, `Swatinem/rust-cache@v2`, `dtolnay/rust-toolchain@master`), and no rule yet keeps the pins current (31.7). Numeric dependencies (`libm`, and any crate whose output reaches a result) are pinned and vendored per edition, and an operation version keeps the numeric code it shipped with. `libm` is pinned by `Cargo.lock` and CI's `--locked`, not vendored yet (piece "NaN-free scalar, total order and deterministic maths module"). A dependency change that alters any result ships only as a new operation version or a new edition. The move to correctly rounded CORE-MATH functions (section 21; G05 for five functions) shrinks this risk where correct rounding is proven, because there the results are fixed by specification (21, "What correct rounding rests on").
- **The format museum** keeps sample files and operation logs from every released format version and edition; each must open and replay to its recorded hashes forever. Format round trips also run on a big-endian target under emulation, because the file format must not depend on byte order.

No open kernel gates on this. Manifold compares six small serial cases on three platforms, without WebAssembly or threads; SolidCraft's goldens need per-platform overrides ([01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md), section 3; [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), SolidCraft).

## 28.14 Benchmarks

Principle 12 blocks any speed regression over 5%, and Spare Construct's architecture runs its benchmarks on every pull request. Section 23.13 owns the benchmark gates: what each one measures, when it runs and what a miss blocks. This section says how they sit in the test suite, and adds one gate.

**Count gates, on every pull request (23.13).** Wall-clock time on shared CI machines varies by more than 5%, so pull requests are gated on counts that do not vary. The merge base and the change are built in the same job, each with the toolchain pinned in its own tree, and compared; the toolchains differ only in a pull request that bumps the toolchain (23.13):

- **Instruction counts** under Valgrind (which runs a program on a simulated processor and counts what it executes), with gungraun, formerly iai-callgrind (Apache-2.0 or MIT), on single-threaded benchmarks ([README](https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/README.md)).
- **WebAssembly fuel** in wasmtime, a count of executed operations that wasmtime documents as deterministic ([config.rs](https://github.com/bytecodealliance/wasmtime/blob/5afd63588b1f5520391c6a37fc38dfd3ccd607c2/crates/wasmtime/src/config.rs#L627-L646)).
- **Memory:** peak heap bytes from gungraun's DHAT runs, and logical bytes on every target.
- 23.13's other deterministic gates: golden counts, output hashes, reference mode and scenarios at reduced scale.

Each blocks the merge as 23.13 sets out: counts, memory and scenarios on a rise over 5%; golden counts on any change without a version bump; output hashes and reference mode on any difference.

**Growth, a gate this chapter adds to 23.13.** Each scale benchmark runs at n, 2n and 4n, on every pull request, and its work must grow no faster than the operation's stated cost; faster growth blocks the merge. This catches quadratic traps such as web-ifc's stalled face matching on waffle slabs ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section A3). 23.13's table lists it, so that one list holds every gate. As built (G06): n is the operation's size, not the batch length; a doubling may multiply the work by at most 2^(k + 1/2) for a stated cost of size^k, compared exactly in integers; and only the wide integers' add, subtract, negate, compare and multiply at 2, 4 and 8 limbs are scale benchmarks so far. The fixed part of each count, the loop and the operation's fixed cost, damps every ratio. For work one order up, the growing part must exceed about 1.6 times the fixed part (k = 1), or about 2 times (k = 2), before a doubling fails. Comparison's fixed part is about 2,800 of its 3,814 fuel at 2 limbs, so a comparison made quadratic could pass both doublings, and larger sizes do not help: from 16 limbs up the compiled code changes between widths enough to swamp growth. So this check alone does not guarantee that faster growth blocks a merge; the 5% gate catches such a rise on benchmarks the merge base already has. A method that removes the fixed part, larger n, or a weaker promise is open (31.7).

**Wall clock and scaling, nightly and before each release (23.13).** criterion or divan time the benchmarks on the dedicated reference runner, at 1 and 8 threads, natively and in browsers, against the kernel's shares in 23.1. A nightly miss is traced to its change by bisection, and blocks further merges to `main`, except the fix or a revert, until the target is met again.

**Spare Construct's targets** ([ARCHITECTURE.md](../../spare-construct/ARCHITECTURE.md#performance-targets-and-testing), Performance targets) become kernel scenarios: open a 230,000-element building to first view in under 3 s; move a door and see it in under 16 ms; change a level height affecting 2,000 walls in under 1 s; draw a floor plan in under 500 ms, and under 16 ms after an edit. Each scenario gets a kernel budget, its share of the target (23.1), and the nightly wall-clock run checks it against that budget, with the consequence for a miss that 23.13 sets. The mesh boolean is also timed on the EMBER pairs, where EMBER reports a geometric mean of 1.6 ms per boolean ([01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md), section 5).

## 28.15 Drawings

Drawings (section 18) are vectors with names, so they are compared as vectors, not pictures.

- **The gate is exact.** A drawing's canonical output (every curve with its line class, visibility and the names it came from) is hashed like any other result.
- **Laws.** The visible and hidden pieces of every edge cover its projection exactly once, and a cut region's area equals the exact section area.
- **An independent visibility oracle.** For flat scenes, the whole visible arrangement is compared exactly with the reference implementation's exact plane cuts and exact ray tests. For curved scenes, rays are certified, with bounds, and are tested at several deterministic points along each visible and hidden interval, including near both ends, so a missed pair of visibility changes inside one interval is found. Nightly, the number of visibility changes and the set of silhouettes are compared with OpenCascade's hidden-line removal, and quadric scenes with Krbn.
- **Pictures are for people.** When a drawing changes, CI renders before, after and an overlay with [tiny-skia](https://github.com/linebender/tiny-skia/tree/5d4754777746eef0828be166896eaf482c49f8f2) (BSD-3-Clause) for the reviewer, and rerun (MIT or Apache-2.0) serves interactive visual debugging ([10](../../../kit/notes/G01-evidence/10-github-sweep.md)). Pixels never gate.

## 28.16 How a failure becomes a permanent test

Every failure ends as a fixture that runs forever, whatever found it: a fuzz crash, a property counterexample, a missed checker fault, an oracle difference, or a bug report from a person or an AI.

1. **Minimise.** The fuzzer's minimiser or the property tool's shrinking reduces the input. For operation logs, `geom-test` removes operations and inputs one at a time while the failure persists.
2. **Record.**

   ```rust
   struct Fixture {
       id: FixtureId,         // e.g. "bool/0412-coplanar-wall-cut"
       source: Source,        // issue, fuzz seed, oracle triage, failure catalogue, closed form
       log: OpLog,            // operations and inputs, in the native format
       expect: BTreeMap<(Edition, OpVersions), Expect>, // one golden per edition and set of operation versions
       status: Status,
   }
   enum Expect {
       Ok  { hash: Sha256, measures: Measures },   // volume, area, counts, box, exactness class and bound
       Err { kind: ErrorKind, entities: Vec<NameId> },
   }
   enum Status {
       Pass,                      // must match `expect` exactly
       Known { wanted: Expect },  // a typed error today; `wanted` is the success we want
   }
   ```

3. **Prove it catches the bug.** A fixture whose source records a fix in our code must fail on the merge base and pass with the fix; CI checks both. The rule does not apply to oracle outcomes "their bug" or "both within bounds", or to `Known` fixtures.
4. **Keep it.** Fixtures are never deleted; old operation versions are kept forever (section 26.2), and so are their fixtures. Every fixture also seeds the fuzz corpora.

The statuses follow OpenCascade's test suite ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section D). Only a typed error can be `Known`. A run reports Pass, Fail or Improved; Improved means a `Known` fixture now returns its wanted result, and the fixture moves to `Pass`. A panic, a hang, an invalid body, a broken law, a wrong but valid result, or a disagreement with an exact oracle can never be `Known`: each blocks every release until fixed.

## 28.17 When each check runs

| When | What runs |
| --- | --- |
| Every pull request | Unit and property tests with fixed seeds (bolero's test mode excepted, 28.2); the checker after every operation; fast fixtures; reference implementations; predicate oracles and extremes; proofs whose call graph reaches the change (all foundation proofs on a foundation change); the determinism matrix; mutants in the diff; coverage, with branch coverage on foundation changes; the merge gate (28.8); 23.13's count gates and the growth gate (28.14); a short fuzz; the fails-on-base check; the completeness gate |
| Nightly | Property tests with fresh, logged seeds; hours of fuzzing per target; slow fixtures; outside oracles; fetched corpora; all proofs; filter-bound certificates; wall-clock and scaling benchmarks on the dedicated runner, where a miss blocks merges to `main` (23.13) |
| Weekly | The full sharded mutation run; Miri over the test suites |
| Toolchain or dependency change | Every golden and the format museum across the whole matrix |
| Each release and edition | Everything; the format museum; the edition's goldens frozen |

The kernel has no randomness. Seeds belong to test generators only, and every one is printed. Read literally, this sentence and 28.19's last line conflict with the randomised methods of 16.2, 16.3, 16.4 and 17.10, whose seed is an explicit operation input fed to the kernel's one generator, `geom_num::rng` (21). G32 builds to the reading that both forbid hidden seeds, any seed that is not a recorded input, until kernel owners decide (31.7).

## 28.18 What is unsolved, and what Spare Kit promises

- **No open kernel gates merges on bit-identity across platforms, WebAssembly and thread counts.** Spare Kit does (28.13).
- **No complete B-rep kernel has been formally proven.** Spare Kit proves its wide integers, predicate bit budgets, filter bounds and topology model, and tests the rest; every module header says which.
- **Machine-checked filter bounds and proof twins are our proposals,** each behind a spike; the twins' spike (G36) passed on orient2d. Until a filter's certificate checks, it is labelled "checked, not proven" and backed by the exact oracle at the extremes and at the filter boundary.
- **Certified completeness of NURBS surface intersection is open** (section 12). Tests check exactly the guarantee section 12 makes, against exact truth; they cannot check more than is promised.
- **No exact outside oracle exists for curved booleans.** Differences need judgement, so every judgement is recorded as a fixture and never made twice.
- **No open corpus of Class-A surfaces exists.** Spare Kit authors one.

## 28.19 What the test suite never does

- Never links or ships GPL, AGPL or LGPL code, and never treats an outside oracle as truth.
- Never makes a test pass by retrying, nudging operands or widening a tolerance, the way IfcOpenShell and SolidCraft rescue failing booleans ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section A3; [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), SolidCraft).
- Never changes a Stable golden hash within an edition.
- Never accepts a panic, a hang, an invalid body or a wrong result as a known failure.
- Never stores a file whose licence forbids it, and never stores client data.
- Never lets a seed, a clock reading or a memory address decide a replayable result.
