# 31. Open risks

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

This chapter lists the real risks to the kernel succeeding: being finished, keeping its promises of exactness and determinism, being fast enough to use, and staying legally clean. For each risk it says what could go wrong, how likely that is, how bad it would be, what the plan already does about it, and the early warning: a test or measurement that would show the risk coming true while there is still time to act. Where an early warning sets a threshold the chapters do not already set, it is marked "proposed".

Evidence is cited as in the chapters: a bracketed report number such as [03] is a report in G01-evidence, and a plain number such as 12.14 is a section of this set. Where a rating depends on an owner decision in section 32, the risk says so.

## 31.1 How to read the ratings

| Rating | Low | Medium | High |
| --- | --- | --- | --- |
| Likelihood | No sign of it yet; several things would have to go wrong | Plausible; parts of the evidence point to it | Already partly seen in the evidence, or the default outcome if nothing is done |

| Impact | What it means |
| --- | --- |
| Medium | One feature, format, oracle or data set waits or is weaker |
| High | A major capability is late, weaker than promised, or slower than its targets |
| Severe | A principle in section 3 fails, or the kernel is never finished |

## 31.2 The risks at a glance

| | Risk | Likelihood | Impact | First early warning |
| --- | --- | --- | --- | --- |
| R1 | Never finishing: the scope, and Fornjot's lesson | High | Severe | The first vertical slice is not running end to end before curved work starts |
| R2 | Certified surface intersection stays incomplete | High | Severe | Flag counts per class in the intersection acceptance harness |
| R3 | Exact arcs and conics in 2D cost too much | Medium | High | Generated widths and costs from piece "2D-01" |
| R4 | Wide integers are slow in WebAssembly | Medium native, High browser | High | The committed wide-integer benchmark report |
| R5 | Exactness costs too much time or memory at building scale | Medium | High | Plane memory on the generated 230,000-element building |
| R6 | Error bounds pile up until curved work refuses | High under the literal rule; under 32.3, Low for booleans, Medium for fitted chains | High | Long-chain law tests |
| R7 | A mistake in our own proofs or bit budgets | Medium | Severe | Any disagreement with an exact oracle |
| R8 | The same answer everywhere breaks | Medium | High | A golden-hash mismatch on any target |
| R9 | Names collide or depend on the wrong thing | High | High | Weak ordinals and Ambiguous results in the naming corpus |
| R10 | Imported data cannot be healed without raising tolerances | Medium to High | High | Healed fraction on the STEP corpus |
| R11 | Too strict to use | Medium | High | Refusals per 1,000 operations in scenario runs |
| R12 | Research items do not land | High | Medium each | How often each "unsupported" error is hit |
| R13 | Proven-bound maths is late or does not port | Low for the five functions ported (G05); Medium for the rest | High | The maths piece not merged before the first survey piece |
| R14 | Keeping every old version forever | Medium | High | Format museum failures; size of kept versions |
| R15 | Hostile files | Medium | Medium to High | Reader fuzzing crashes, hangs or memory spikes |
| R16 | Patents on methods we use | Low each, Medium overall | Severe for core methods | Unsearched families on the build path |
| R17 | Licences and terms awaiting confirmation | High | Medium each | Number and age of ledger rows waiting on an owner |
| R18 | Copyleft or non-free code hiding inside permissive sources | High that it is met; Medium that it gets in | High | A banned-source scanner hit |
| R19 | Learning from proprietary programs crosses the line | Medium | High | An expected value that cites Revit or AutoCAD behaviour |
| R20 | Copyright in AI-written code is uncertain | Medium | Medium | Rulings on AI authorship |
| R21 | Legal work has no budget or counsel | High | High | Items waiting on counsel, and their age |
| R22 | Owner decisions and reviews become the bottleneck | High | High | Pieces blocked on an owner answer |
| R23 | The map contradicts itself | High | Medium, rising | Unresolved conflicts in 31.7 |
| R24 | Agent-written work shares blind spots | High | High | "Unverified" reaching a Stable module |
| R25 | Thin expertise and many handovers | High | High | Handovers per piece |
| R26 | The test machines cost more than the co-op can pay | Medium to High | High | Any matrix leg skipped |
| R27 | Programs work around a late kernel | Medium | High | Geometry code appearing in program crates |

## 31.3 Technical risks

### R1. Never finishing: the scope, and Fornjot's lesson

**What could go wrong.** The thirteen drafted chapters alone hold 373 build pieces, 112 of them Large and 20 Extra large, for every kind of geometry every future program might need. Work could stall part way, so the kernel serves no program fully while programs build their own geometry. Fornjot, the most serious open Rust B-rep kernel, shut down in 2026 with its goals "never reached". Its author's listed mistakes were: extrapolating from early success (progress hit a cliff after moving from distance fields to B-rep), prototyping too late, making small changes where a re-architecture was needed, building a generic kernel instead of a focused tool, half-measures after funding fell, and never finishing the move to the new architecture ([03] section 2). Spare Kit is generic by design, which is the fourth mistake exactly, so it needs strong counterweights. Mapping everything before writing code also pulls against the second mistake, prototyping too late.

**Likelihood:** High. **Impact:** Severe.

**Mitigation built into the plan.**
- The whole architecture is mapped before any code, so the core should not need re-architecting part way (lessons three and six).
- The answer to lesson two is to prototype the hardest parts before anything depends on them: the vertical slice below, and spikes before commitments. The spikes are the Verus twins, filter certificates, the fuzz toolchain, the recursion check, the volume mesher design, certify mode, exact rank and the curved winding number.
- The build order (section 30) proves each layer before the next.
- Spare Construct is the focused first user: its ten targets become kernel scenarios that block merges (23.1, 28.14).
- Domain toolkits wait until two programs need them (26.1).
- Research is kept apart from promises. Unsolved items return named errors meanwhile (13, 16).
- Fornjot also found that splitting into many crates did not pay off ([03] section 2). The set names more than 40 kernel crates once the proposed ones are counted. The consolidated crate-list note (R23) is where that count is challenged.

**Early warning (proposed).**
- A first vertical slice runs end to end before any curved-path piece starts: a wall with 300 openings cut exactly, named, checked, saved, reopened and replayed bit for bit on every section 21 target.
- Merged pieces per month are tracked against the build order.
- A piece handed over more than twice is split or re-scoped.
- The owner reviews scope whenever a layer takes more than twice the effort its piece sizes imply.

### R2. Certified surface intersection stays incomplete

**What could go wrong.** Every curved boolean, blend, offset, curved drawing and curved clash rests on surface-surface intersection (12). No open method guarantees the right topology for every near-tangent pair of NURBS surfaces, and a 2026 paper still calls this an open challenge (12.14). Default mode could leave so many flagged regions on real models that curved operations fail often. A flagged region is a small area the kernel could not prove either way, returned with its reason. Near-tangent contacts and singular crossings are the likely cases, such as fillets meeting at near tangency (12.9). Every kernel measured so far loses answers there. Certify mode might be too slow to serve as the fallback. And some cases that are never flagged can still be slow: a surface against its own offset one step away needs very fine subdivision before it separates (12.8).

**Likelihood:** High that some classes stay flagged; Medium that this blocks product use. **Impact:** Severe.

**Mitigation built into the plan.**
- Nothing is dropped: an unprovable part comes back as a flagged region with a reason and a size (12.2).
- Planes, spheres, cylinders and cones, and tori in special positions, are intersected in closed form, with exact decisions when their defining data is exact (12.7). Flat solids never reach this code (11.3). Together these cover most of a building. General quadric pairs take the filtered path until their several-thousand-bit predicates land, and inexact data, such as a STEP cone given by an angle, is Certified.
- Certify mode is opt-in, and an operation can escalate one region at a time (12.14).
- A failed B-rep operation offers the mesh or field route explicitly, never silently (9).
- The ssi-corpus is the acceptance gate (12.17). Certify mode must pass all 119 cases. Default mode must leave zero flagged regions in the transversal, seam-and-pole, coincident, patch-boundary and high-degree classes.

**Early warning.**
- Flag counts and sizes per corpus class, published on every pull request by piece "Intersection oracles and acceptance harness". They may not grow.
- The robustness scoreboard per operation family (28).
- The measured cost of certify mode, and of extreme default-mode cases such as a NURBS against its own offset at one step (pieces "Certify mode (a)" to "(c)").
- Flagged regions per 1,000 curved booleans on piece "Curved boolean corpus".

### R3. Exact arcs and conics in 2D cost too much

**What could go wrong.** Chapter 14 decides every crossing and touch of lines, arcs and ellipses exactly. Constructed tangencies need rational positions far finer than the grid. The deepest predicates may need several thousand bits, and conic roots more. These widths are estimates, not yet derived, and the earlier 1,280-bit estimate was withdrawn (14). The cost has not been measured on any target. Tangency-heavy drawings are exactly where the cheap floating-point filters fail. Drafting could become slow, or ordinary fillets and offsets could hit the construction budget and return NotRepresentable.

**Likelihood:** Medium. **Impact:** High. The 2D layer sits under drafting, sketches (15), survey linework (17), drawings (18) and board copper (14).

**Mitigation built into the plan.**
- Every width is generated from its expression, never typed by hand (14).
- A fixed construction budget applies. Beyond it, inputs commit to the grid with 6.3's re-check, or the operation returns NotRepresentable with remedies.
- Three fallbacks are named for the owner, each used only for the class of predicate whose cost fails: certified decisions on ellipse pairs, a coarser angle target for constructions that stay in 2D, or certified decisions on constructed circles (32.2).
- Rounding constructed circle centres to the grid, as the brief's literal wording says, is not a fallback. A 5 mm fillet with a rounded centre opens a 47 µm gap, five times ε_fit (14).

**Early warning.**
- Piece "2D-01": the generated widths, the filter hit rate and the exact-path cost on a tangency-heavy drawing corpus, natively and in wasm32, compared with chapter 14's estimates.
- The rate of NotRepresentable and ExactnessNotAccepted on the DXF corpus (piece "2D-28").

### R4. Wide integers are slow in WebAssembly

**What could go wrong.** Exact predicates run in 256-, 320- and 384-bit integers, and the chapters propose wider ones: 576 and 768 bits for skeleton events (13), 576 and 640 for hidden lines (18), and several thousand for general quadric pairs (12). WebAssembly has no 128-bit multiply. V8, Chrome's engine, was not on the wide-arithmetic proposal's implementation list at the pinned commit. None of this has been measured ([05] open question 7). Buildings are also full of designed ties, such as walls sharing a face or aligned openings. There a filter can never settle the sign, so the exact path always runs. Browser builds could miss Spare Construct's targets by large factors.

**Likelihood:** Medium natively, High in the browser. **Impact:** High.

**Mitigation built into the plan.**
- The predicate step ladder settles shared planes by identity, and small cases in 64-bit integers, before any filter runs (23).
- The generator writes width-aware code with bit-range certificates.
- Browser packages ship with and without wide arithmetic and are chosen at load (23).
- A double-double filter stage is held in reserve (11).
- The benchmark comes early in the build order.

**Early warning.**
- The committed report of piece "Wide-integer variants and WebAssembly benchmark", covering x86-64, aarch64, Wasmtime, Node, Chrome, Firefox and Safari.
- The filter must settle at least 99% of calls on the EMBER case list (piece "Plane type and arrangement predicates").
- Step statistics: the share of calls that reach the exact path on scenario models (piece "Predicate step ladder, rounded-input filters and step statistics").

**Measured (piece G42; report `projects/kit/notes/G42-variants-report.md`).** The 384-bit exact path, classifying a three-plane point against a plane (`tpi_side`), takes 691 ns natively on x86-64 and 460 ns on aarch64. As shipped, with 64-bit limbs, it takes 1,697 ns in Node 24, 2,835 ns in Wasmtime 50, and 2,088 ns in Wasmtime with wide arithmetic: 2.5 to 4.1 times native x86-64. In the width-aware 32-bit-limb variant, it takes 1,026 ns in Node and 1,305 ns in Wasmtime with or without wide arithmetic: 1.5 to 1.9 times. In browsers it takes 1,045 ns in Chromium 136, 745 ns in WebKit 18.4 and 633 ns in Safari 26.6.2, against 1,749, 910 and 1,062 ns as shipped; Firefox 137, in Playwright's build, ran every variant five to six times slower than the others (5,703 ns with limbs). No browser measured validates the wide-arithmetic build with default settings. The cause is that on `wasm32` every 64-bit-limb product, and every overflow-checked 64-bit product, is a call to `__multi3`, while a 32-bit-limb product is one native multiply. Wide arithmetic makes the 64-bit-limb products single instructions, and halves the smaller predicates' time where an engine has it; V8 (Node, Chrome) does not validate it. The risk stands at Medium in the browser, held there by the limb variant for the deepest predicates; the factor at the 576-bit and wider budgets the chapters propose is still unmeasured.
- Browser scenario runs against the proposed target of within three times native (17).

### R5. Exactness costs too much time or memory at building scale

**What could go wrong.** Spare Construct's targets are tight. The kernel's proposed share is 4 ms for a door move and 1 s for opening a 230,000-element building (23.1). A full re-cut meets 4 ms only for hosts with up to about 50 openings. Above that, a local re-cut must give bit-identical output, which needs the arrangement to split cells at positions that do not depend on content. That is still open (23). Plane coefficients reach 83 and 125 bits, so one plane may cost 64 to 96 bytes (23). A 50 km road corridor at ε_fit may hold millions of vertices (17). And the reference machine all targets assume is not yet chosen.

**Likelihood:** Medium. **Impact:** High.

**Mitigation built into the plan.**
- Instances share geometry (19, 23).
- N-ary cuts and batch evaluation.
- A deterministic parallel helper.
- Stored display data, so opening a model needs no regeneration (23).
- Named fallbacks for plane memory: compact widths, floating-point copies made on demand, planes shared across instances.
- Corridors are coarser only on request.

**Early warning.**
- Piece "Arena layout and plane memory benchmarks" on the generated 230,000-element building.
- Piece "Local re-cut": CI equality of local and full re-cuts, and door moves at 3, 50 and 300 openings, natively and in wasm32.
- The corridor benchmark (proposed: 50 km in 60 s and 4 GB) before ε_fit is confirmed for earthworks.
- Nightly scenario runs against each kernel share on the reference machine (32.16).

### R6. Error bounds pile up until curved work refuses

**What could go wrong.** Section 9 says bounds add up. Read literally, each curved boolean adds about ε_fit/128, so a curved part fails with ExactnessNotAccepted after about 128 booleans (11). Chains such as a fillet on a fillet, or an offset of an offset, eventually return it too (28.2; section 22). Even under 32.3's recommended reading, fits still add along each face's own history. Each fitting step may spend up to ε_step = ε_fit/4 (13.2), so on one face the fifth fit at full budget refuses. Lofts, network surfaces, knot merging, rational-to-polynomial approximation and subdivision-to-NURBS conversion are all fits. Ordinary long editing histories could start to fail.

**Likelihood:** High under the literal rule. If 32.3's recommendation is accepted: Low for booleans and procedural kinds, Medium for chains of fitted operations. **Impact:** High.

**Mitigation built into the plan.**
- Faces never move and new edges are procedural, so true error can be re-proven against the stored carriers each time (11).
- Procedural definitions add no error, and each step's bound is proven against the exact operation on its stored inputs (13).
- Section 32.3 asks the owner to confirm this reading, and says plainly where it stops.

**Early warning (proposed).**
- Long-chain laws in the law catalogue, each reporting the summed bound beside the re-proven one: 300 successive curved booleans on one part; ten-deep fillet and offset chains; and a chain of five fitting operations on one face.
- Any ExactnessNotAccepted or ToleranceExceeded in a Spare Construct scenario run.

### R7. A mistake in our own proofs or bit budgets

**What could go wrong.** Several load-bearing results are ours, not published:
- the bit budgets above 384 bits (13, 18);
- EMBER's extension to cutters (11);
- interior-only refinement in tessellation, and the error bound credited to Waldron (19);
- the isotopy proof for Manifold Dual Contouring, which says the output mesh has the same shape, topologically, as the true surface (16);
- pipe and torus outlines as offsets of the projected spine (18);
- symbolic division at collapsed edges (12).

A wrong derivation means an overflow or a wrong sign, which is a silently wrong answer: the worst failure for a kernel meant to be iron-clad. The checker is a consistency check, not an independent oracle (28.4). In `geom-num` (G02) a wrong bit budget can no longer do this: each derivation's arithmetic is asserted when the crate compiles, so a wrong figure stops the build; every fixed-width step is a checked method; and a budget that still failed falls to an exact total path (`I384` for the explicit predicates, `I512` for planes) or returns `BudgetExceeded`, never a sign (6.2). The risk stays for code that does not follow that rule, such as generated limb code (23.3) and other crates, and for derivations that are not arithmetic, such as the proofs listed above.

**Likelihood:** Medium. **Impact:** Severe if it reaches users unseen.

**Mitigation built into the plan.**
- Each predicate module carries its written derivation and a test at ±(2^40 − 1).
- Each derivation's arithmetic is a compile-time assertion, every fixed-width step is checked, and an exact total path stands behind each budget (6.2; built in `geom-num`, G02).
- Differential fuzzing against exact rational oracles.
- Kani overflow proofs for every limb step. Kani is a tool that checks a Rust function for every possible input in a range, here proving that no step overflows.
- Verus twins of the generated predicate code (Verus checks written proofs about Rust code by machine), and FPTaylor or Gappa certificates for the filters (tools that prove bounds on floating-point rounding error) (28.6, 28.7).
- Reference implementations written by a different author without reading kernel code (28.9).
- An independent big-integer checker, and a refutation-only checker for curved bodies that shares no code with the kernel (28.4).
- Each unpublished proof is a reviewed deliverable with a named fallback (16, 19).

**Early warning.**
- Any disagreement between a predicate and its rational oracle. It blocks the merge.
- Surviving mutants in the foundation set (28.8). A mutant is a deliberately broken copy of the code; one that no test catches shows a gap in the tests.
- A failed proof spike (pieces "Verus twin spike" and "Filter-bound certificates").
- A proof review that does not pass.

### R8. The same answer everywhere breaks

**What could go wrong.** Any of these could change a bit somewhere: a Rust or LLVM upgrade, the planned move from libm to correctly rounded CORE-MATH functions, a change in a browser engine, or a CI runner that retires. Shared models would then diverge between machines, and replays would stop matching. The move to CORE-MATH has happened for five functions (G05) and changed bits as expected: of G02's 7,677 maths results, 92 moved by one ulp to the correctly rounded value (`exp(1)`, for example, from 2.7182818284590455 to 2.718281828459045), no error changed, and `geom-num`'s golden changed with them, before any edition existed (28.13, 32.12).

**Likelihood:** Medium over the years. **Impact:** High.

**Mitigation built into the plan.**
- The replay corpus runs on every target, in debug and release, at 1, 2, 3 and 8 threads, on every pull request (21).
- The toolchain is pinned, numeric dependencies are vendored per edition, and each operation version stays on the numeric code it shipped with (28.13).
- Results change only through a new kernel edition with a reported upgrade (26.2).
- Models open from stored geometry, so a mismatch never stops a file opening.

**Early warning.**
- A golden-hash mismatch on any leg. A golden hash is the recorded fingerprint of a correct result.
- Any hash change in a toolchain-bump pull request, which re-baselines on its own (23, 28).

### R9. Names collide or depend on the wrong thing

**What could go wrong.** Dimensions, hosted doors and AI references all hold persistent names. The evidence already shows gaps:
- Bounded names overflow at 256 edges between the same two faces (11).
- Breaking TIN ties by name hash makes contours depend on what the points are called (17).
- Adding an operand to a symmetric boolean can rename faces (11).
- Free-form silhouettes fall back to weak ordinals (18).

References would then break, or differ because of naming rather than geometry.

**Likelihood:** High: gaps are already seen in the evidence. Each one found so far has a fix in 31.7. **Impact:** High.

**Mitigation built into the plan.**
- Every name is derived from the operation that made it, sorted, and never random (20.2).
- The history contract is a property test on every operation (20.3).
- Weak ordinals are marked as weak.
- Naming fixtures (28).
- The name scheme is versioned, so a fix is a new scheme, never a reinterpretation.

**Early warning.**
- The share of weak ordinals and Ambiguous resolutions in the naming corpus and in scenario regenerations (piece "Naming and reference-resolution fixtures").
- Any duplicate name in a test. That is a bug (20.2).

### R10. Imported data cannot be healed without raising tolerances

**What could go wrong.** Real STEP and IFC files are full of gaps and near misses. Spare Kit heals within ε_heal and never raises a tolerance (6.5, 24.9). So many files may stay Tolerant, and later operations then refuse, or they may come in as Sheet bodies with free edges. Proven 3D vertex rounding has no open implementation and a worst case of O(n^4) or worse (24). People judge a kernel by its first import.

**Likelihood:** Medium to High. **Impact:** High.

**Mitigation built into the plan.**
- The healing order of 24.9.
- Sewing only on unique matches.
- An exact Sheet or General body offered as the first remedy.
- A finer frame offered when rounding fails.
- IfcConvert and web-ifc as outside oracles (28).

**Early warning.**
- The healed fraction per step on the STEP corpus. Its first measurement is the baseline, which may only rise (piece "geom-heal: healing Tolerant bodies").
- The share of IFC sample imports that end Tolerant or as Sheet bodies.
- ExportRoundingFlip counts on export round trips.

### R11. Too strict to use

**What could go wrong.** The kernel refuses where other programs guess: ExactnessNotAccepted, NearContact, BranchLost, flagged regions, read-only bodies opened with fallbacks, CurvedSkeletonUnsupported. People used to Revit or Rhino "just working" may find it obstructive, even when each refusal is correct.

**Likelihood:** Medium. **Impact:** High for adoption.

**Mitigation built into the plan.**
- Every error names the entities involved and gives numeric remedies (22).
- Drafting commands pass a standard acceptance, recorded in the operation log (14).
- Explicit Approximate, mesh and field routes are one remedy away (9, 13).
- Previews run at coarser bounds (11).

**Early warning (proposed).**
- Refusals per 1,000 operations in Spare Construct scenario runs and early users' sessions, by error kind.
- The share of refusals where the first remedy succeeds.

### R12. Research items do not land

**What could go wrong.** Some capabilities depend on open research:
- a certified winding number for trimmed NURBS (11);
- curved skeletons for arc eaves, degenerate 3D mitre events, a proven bound for rotation-minimising frames, more G2 corner classes, and certified general swept volumes (13);
- certified field meshing at sharp CSG edges, and eigen-tables for darts and corners (16);
- mitred grading on sloping feature lines (17).

Some may stay open for years, leaving gaps where the incumbents give an approximate answer.

**Likelihood:** High that some stay open. **Impact:** Medium each; together a visible product gap.

**Mitigation built into the plan.**
- Each returns a named error, or an Approximate result on request, until it lands (13, 16).
- Research has its own pieces and never blocks the core.
- Nothing degrades silently. Interim states are allowed only when labelled (32.3).

**Early warning.**
- How often each "unsupported" error kind is hit in the Class-A corpus and in scenario runs.
- The outcomes of the research spikes, recorded in their pieces.

### R13. Proven-bound maths is late or does not port

**What could go wrong.** Survey angles and COGO (coordinate geometry: bearings and distances), spirals, rotated frames, lattices and minimal-surface fields all need sinpi, cospi, atan2, exp and ln with proven error bounds and the same bits on every target (16, 17, 21). The plan was to port CORE-MATH's correctly rounded functions, and G05 has ported those five: safe, panic-free `no_std` Rust with written proofs. It builds for wasm32-unknown-unknown and wasm32v1-none, and every test passes on wasm32-wasip1 under wasmtime with the golden of native x86-64 in release, debug, with overflow checks and with `-C target-cpu=native`, and natively at 1, 2, 3, 8 and 16 threads. Natively it runs within twice `libm`'s time (over three runs on one machine: sinpi 0.96 to 1.28 times, cospi 0.89 to 0.95, exp 1.33 to 1.37, ln 1.59 to 1.80, atan2 0.75 to 0.88). G47 built `Interval::atan2` (12.3), put the oracle check on ten million inputs per function and every input of CORE-MATH's worst-case lists in the nightly workflow, and measured wasm32: WebAssembly has no fused multiply-add instruction, so `libm` computes one in software, about 54 ns a call in Wasmtime against 21 ns for Dekker's two-product. `exp`'s one fused multiply-add is the exact error of a product, which Dekker's two-product gives bit for bit, so on wasm32 it now uses that (evidence note 05, section 7), and in Wasmtime the five run at sinpi 1.5 to 1.7 times `libm`, cospi 0.9 to 1.0, exp 1.7 to 1.9 (2.6 to 2.9 before), ln 1.2 to 1.3, and atan2 2.2 to 2.6 (three runs each on one loaded machine; WebAssembly fuel counts agree). What remains: correct rounding is proven only under conditions (21); radian `sin` and `cos` did not port to the proven standard, and no piece is listed for them (29.4); `atan2` on wasm32 is over twice `libm`'s time, because its first fast path needs two rounded fused multiply-adds, which no exact transformation gives bit for bit (31.7); and the Node, Linux, macOS and arm64 legs have run only in CI. If a port gives different bits on some target, R8 follows.

**Likelihood:** Low for the five functions ported; Medium for radian sin and cos, and for atan2's speed on wasm32. **Impact:** High for survey, civil and lattices.

**Mitigation built into the plan.**
- Angle units are to be reduced exactly, in integers, to an octant before any floating-point number is formed (17.2); that step is not built yet.
- The port came first, so the interim polynomial once planned was not needed (G05).
- Differential tests against a big-number oracle and CORE-MATH's worst-case lists, and bit identity on every CI target.
- The two maths pieces are one board piece, "Proven-bound sinpi, cospi, atan2, exp and ln".
- The ports are benchmarked against libm (G05); libm's audit is still to come (piece "NaN-free scalar, total order and deterministic maths module").

**Early warning (proposed).**
- The merged maths piece is not merged before piece "Angle units and core COGO" is ready to start.
- Any golden-hash difference in the maths module between targets.
- The benchmark against libm (the piece's target: within 2 times).

### R14. Keeping every old version forever

**What could go wrong.** Permanence means keeping a decoder for every schema version, every operation version on its original numeric code, and every name scheme (24, 26.2, 28.13). The kept code grows for decades and must still build on future toolchains. And when a better checker finds that an old operation version returned an invalid body, replaying that version must now fail, so a result changes inside an edition (28).

**Likelihood:** Medium. **Impact:** High over decades.

**Mitigation built into the plan.**
- Files store evaluated geometry and open without replay (26.2).
- The format museum reads every archived file on every target, big-endian included (28).
- An independent Python reader is written from SPEC.md alone (24).
- The rule recommended in 32.4: the edition promise covers valid results only, and a replay that a better checker now rejects is reported element by element, like an upgrade.

**Early warning.**
- A failure in the format museum, or a disagreement with the Python reader.
- Build time and size of the kept versions, tracked per release (proposed).

### R15. Hostile files

**What could go wrong.** The kernel reads files from anyone, in browsers and on servers. A crafted SKG, STEP, IFC or DXF file could hang a reader, exhaust memory, or reach a path the checker misses.

**Likelihood:** Medium. **Impact:** Medium to High.

**Mitigation built into the plan.**
- Mandatory range checks in every reader, and again after numbers reach the grid.
- Byte-source readers whose results cannot depend on how reads are split, fuzzed.
- Trust as an explicit read option, with the full checker for unknown sources.
- Hardened XML and zip readers.
- A Kani harness on the native reader (24, 28).

**Early warning.**
- Fuzzing crashes, hangs or memory spikes on any reader.
- Peak memory per input byte on the reader fuzz corpus (proposed).

## 31.4 Legal risks

### R16. Patents on methods we use

**What could go wrong.** A patent protects a method, so writing our own code does not escape it. Only subdivision, T-splines, watertight trimming and lattices have been searched. Sixteen catalogue families have had no search at all (29.17), including booleans, Class-A blends, naming, offsets, clash, hidden lines and skeleton roofs. Nobody has searched EMBER, the method under the whole flat-solid core (11). Even inside the areas already searched, 18 methods adopted in chapters 13 and 16 have had no search, among them PCCM, Manifold Dual Contouring and Instant Meshes (13, 16). Known active patents sit near planned work: Autodesk's lattice hollowing (US 10,635,088), and the watertightCAD family (US 10,614,195 and two related patents), which is close to edge-first tessellation. The status of the patents found so far was read from Google Patents and is an assumption until counsel confirms it (16). Apache-2.0's patent grant does not clearly cover a re-implementation.

**Likelihood:** Low for any one family; Medium over decades. **Impact:** Severe for a core method, which could be pulled after files depend on it; Medium for a niche one.

**Mitigation built into the plan.**
- The patent register. No family enters the Build stage without a recorded search and owner review, and CI enforces it (29.17; piece "Patent register").
- The EMBER check is the first boolean piece.
- Versioned operations let a method be replaced while old files still open from stored geometry (26.2).
- Section 32.7 asks who searches, and who pays.

**Early warning.**
- The register's count of unsearched families and methods on the build path.
- Any hit in a search.

### R17. Licences and terms awaiting confirmation

**What could go wrong.** Some work waits on answers that may be slow, or "no":
- McNeel's written openNURBS grant (geom-3dm waits);
- the legal review of OpenSubdiv's TOST licence;
- NLib's chain of title: decided by the owner on 2026-10-24 (32.8), who accepted NVIDIA's Apache-2.0 publication for all of the SMLib release, NLib and the IntegrityWare-lineage modules included, so this item no longer waits. The risk left is small but real: if a predecessor owner (GeomWare, SMS or IntegrityWare) ever showed that NVIDIA lacked the right, the affected ports would be rewritten from the primary sources, which their Source and credit lines make findable;
- Gerber's terms, which ask for no derivative versions without Ucamco's approval;
- IPC-2581, ODB++, EPSG and LINZ data, and ISO's EXPRESS schemas;
- test data with no licence (ABC, the IFC4.x-IF alignment set);
- many smaller items (24, 29).

Worse than waiting would be using something before it clears and having to remove it later.

**Likelihood:** High that some answers are slow or negative. **Impact:** Medium each: one format, oracle or data set.

**Mitigation built into the plan.**
- Every source has an Allowed and a Planned column, and Planned never exceeds Allowed (29.1).
- Work waits for the owner.
- Methods come from papers.
- Unclear data is fetch-only.
- An exact SPDX allow-list is checked by cargo-deny on every pull request (28.8, 29.2).

**Early warning.**
- The number and age of ledger rows waiting on an owner (proposed: listed in each status report).
- Any licence-gate failure.

### R18. Copyleft or non-free code hiding inside permissive sources

**What could go wrong.** A file under a permissive licence turns out to come from GPL or non-free work. The evidence has already found several cases:
- Numerical Recipes routines in SMLib, NLib and solidcore;
- a Fresnel routine in Clothoids adapted from a Wiley book;
- acadrust drawing on LibreDWG;
- NVIDIA's parsers mirroring OpenCascade's LGPL readers;
- Geogram's bundled TetGen (AGPL), Triangle and HLBFGS (non-free), which fTetWild inherits;
- Lifted Surfacing's build pulling in Adobe Research Licence code and libigl's GPL module;
- hypermesh relicensed under a notice for someone not among its contributors;
- AI-written code reproducing known tables (29).

If such code entered Spare Kit, it would have to be torn out and rewritten clean, possibly after files depend on its behaviour. Spareware is never relicensed, so there is no other way out.

**Likelihood:** High that tainted sources are met, because several already have been. Medium that tainted code gets into Spare Kit, given the checks below. **Impact:** High.

**Mitigation built into the plan.**
- Chain-of-title checks before reading (29.20, rule 2).
- The method-only procedure, with separate reading and writing sessions (29.20).
- Source and Consulted lines in every module header, which generate PROVENANCE.md.
- The banned-source scanner on every pull request (piece "Banned-source scanner").
- Numerical Recipes excluded everywhere, with clean replacements (29.4).
- A tainted method can be replaced by a new operation version (26.2).

**Early warning.**
- A scanner hit.
- A module without a pinned Source line.
- A new "taken from" or "based on" comment found during an audit.

### R19. Learning from proprietary programs crosses the line

**What could go wrong.** The plan compares Spare Kit with Revit and AutoCAD on co-op-authored models (28.11). Done carelessly, that becomes copying a proprietary program's behaviour, which Spareware forbids (29). CADCraft was built by observing AutoCAD as a black box ([12]). Some of the evidence read Parasolid, ACIS and Revit documentation through third-party copies (29.20). The Revit API terms check has not been done.

**Likelihood:** Medium. **Impact:** High.

**Mitigation built into the plan.**
- Truth tests wait for the API terms check, and use only co-op-authored models.
- Differences are judged against our own specification, never adopted as one.
- No expected value comes from CADCraft or from observing proprietary software (14).
- Third-party documentation copies are "stop and ask" (29).
- Section 32.10 sets the line: watching a proprietary program only checks our results against our own specification, on co-op models, and never sets behaviour, expected values or requirements.

**Early warning (proposed).**
- A review checklist item that flags any fixture or pull request whose expected value cites Revit or AutoCAD behaviour rather than our specification.

### R20. Copyright in AI-written code is uncertain

**What could go wrong.** Chapter 29 notes that copyright in largely AI-generated code is legally uncertain, for solidcore, CADCraft and csgrs. The same question applies to Spare Kit, which agents largely write. Spareware relies on copyright staying spread across its contributors, so that nobody can relicense it. If contributions carry thin copyright, that protection, and MPL-2.0's force against closed forks, weaken. This is our reading of 29's open question applied to ourselves. It is unverified and is for counsel.

**Likelihood:** Medium that the question stays open. **Impact:** Medium. The code still works and stays open; protection weakens.

**Mitigation built into the plan.** None beyond counsel's view. DCO sign-off and contributors keeping their copyright (AGENTS.md section 6) say who holds whatever rights exist, but they do not make AI-written code more protectable. Nothing merges on AI review alone (README).
- Proposed: each pull request records the human direction and review behind it, which counsel may find helps. AGENTS.md section 6 does not ask for this today.

**Early warning.**
- Court rulings or official guidance on AI authorship where the co-op operates (an owner watch item).

### R21. Legal work has no budget or counsel

**What could go wrong.** The set leans on lawyers throughout: a patent search before the first geom-poly merge (32.7), a legal review of OpenSubdiv's TOST licence (32.9), a reading of Onshape's terms for the ABC dataset and of step-conformance's parts (32.11), the Revit and AutoCAD terms checks (32.10), any clearance of DWG (README), and counsel's view on AI-written code (R20). K06's cost model leaves legal advice out, because no public rates were found. If no lawyer is funded, all of these wait, and under 32.7's recommendation the boolean core cannot start.

**Likelihood:** High. **Impact:** High.

**Mitigation built into the plan.**
- Work that needs counsel waits rather than going ahead without it.
- Meanwhile methods come from papers and unclear data stays fetch-only (29).
- Section 32.7 asks the owner to fund the work, and names a fallback if a lawyer cannot be funded in time.

**Early warning (proposed).**
- The number of items waiting on counsel, and the age of the oldest, in STATUS.md.
- The first geom-poly piece ready to start with no recorded search.

## 31.5 People and process risks

### R22. Owner decisions and reviews become the bottleneck

**What could go wrong.** The set raises 212 open questions and routes dozens of approvals to "an owner" or "a kernel owner": golden-hash changes, reference implementations, ledger rows, method notes, patent reviews and CI cost. Kernel owners are not named anywhere. The README also puts any change to an architecture decision to a member vote, and the set proposes dozens of amendments to the spine. A poll for each would stall the work. Pieces queue behind decisions, and contributors' tokens go elsewhere.

**Likelihood:** High. **Impact:** High.

**Mitigation built into the plan.**
- Section 32 narrows the owner's part to twelve decisions and one member vote, which covers G01, those decisions and one consolidated amendment note together (32.5).
- After that vote, kernel owners accept most later decision notes, as 26.1 allows. Principles, the grid and tolerance ladders, the same-answer and edition promises and the licence stay with the owner (32.5).
- Section 31.7 gives every other question a home.
- Every piece has a "done when" test, so a review is checkable.
- AGENTS.md's checkout and handover rules.

**Early warning (proposed).**
- The number of build pieces blocked on an owner answer, and the age of the oldest, reported in STATUS.md.
- Spine amendments waiting outside the consolidated note.

### R23. The map contradicts itself

**What could go wrong.** Chapters drafted in parallel already disagree:
- three answers for the one constrained Delaunay triangulation: our own code (17), a port of spade (19), a port of Geogram's (29);
- instance records of 56 bytes (19) against up to 88 (23);
- arena-ordered against name-ordered file encoding (23, 24);
- a box tree in geom-math (11, 23) or in a new geom-bvh (19);
- linear algebra in geom-num (15, 29) or in a new geom-linalg (16);
- the maximal-face merge, with T-junction insertion, kept in geom-bool (11) or moved into geom-poly so geom-mesh can share it (19);
- two pieces for the same proven-bound maths functions (16, 17), since merged into one on the board (30) and built by G05;
- about fifteen proposed new crates on top of about thirty, and dozens of new dependencies. Fornjot found that splitting into many crates did not pay off ([03] section 2).

A future person or AI who reads one chapter could build the wrong thing, against the owner's bar that nobody gets confused.

**Likelihood:** High (already present). **Impact:** Medium, rising with every merged piece.

**Mitigation built into the plan.**
- One decision note per change (26.1).
- MAP.json is generated from the code and checked in CI (26.3).
- Section 31.7 lists every conflict with where it is settled.
- Proposed: one consolidated note covering the crate list and the spine amendments, accepted with G01 before the first kernel code merges (32.5).

**Early warning.**
- Unresolved conflicts in 31.7 (proposed target: zero before the Build stage).
- A MAP.json diff failing.
- The same question asked twice by different pieces.

### R24. Agent-written work shares blind spots

**What could go wrong.** AI agents will write and review most of the kernel. A plausible derivation can be wrong. A claim marked "unverified" can lose its marker when it is copied. The README sets Claude Opus 5.5 or newer as everyone's minimum model, so in practice nearly all writing and reviewing comes from one model family. Two sessions of the same model are far less independent than two people, so a reference implementation by a "different author" (28.9) may repeat the kernel's own mistake.

**Likelihood:** High: with one model family, shared blind spots are the expected outcome. **Impact:** High.

**Mitigation built into the plan.** The main counterweight is checks that do not come from a model:
- Exact oracles, which decide by arithmetic, not judgement.
- Outside oracles from different lineages: CGAL, OpenCascade, Manifold, SMLib.
- Formal proofs where they reach.
- Property and fuzz tests.
- A kernel owner reviews every reference implementation (28.9).
- Proposed: for the foundation set, at least one reference implementation written by a person or by a different model family. A different model family may need an owner exception to the README's model rule.

**Early warning (proposed).**
- A CI check that no Stable-tier module header or doc comment still says "unverified".
- The disagreement rate between reference implementations and the kernel, reviewed per release.

### R25. Thin expertise and many handovers

**What could go wrong.** The kernel needs rare skills: interval analysis, computer algebra, formal proofs in Verus and Kani, surface intersection, subdivision theory. Pieces are done in token-sized sittings and handed over, so a Large piece may pass through several sessions and lose context each time. Few reviewers can judge a 768-bit predicate derivation or an isotopy proof.

**Likelihood:** High. **Impact:** High.

**Mitigation built into the plan.**
- Small pieces with "done when" tests.
- Handover sections in draft pull requests.
- Module headers state the method, the invariants and the cost.
- GLOSSARY.md and the AGENTS.md playbooks (26.3).
- Proposed in 32.5: a paid or academic reviewer for proof deliverables when no kernel owner can review them.

**Early warning (proposed).**
- Handovers per piece, with a piece split after two.
- Pieces in review for more than two weeks.
- Proof deliverables waiting for a qualified reviewer.

### R26. The test machines cost more than the co-op can pay

**What could go wrong.** The determinism promise needs, on every pull request, macOS runners on both chip types and aarch64 Linux runners. It also needs a dedicated benchmark machine, nightly fuzzing hours, a private oracle registry, and licensed Revit and AutoCAD machines (28). GitHub retired its macOS 13 image on 4 December 2025 and said x86-64 macOS ends when the macOS 15 image retires in fall 2027 (changelog, 2025-09-19); its runner reference, read 2026-10-22, also lists a `macos-26-intel` image, so the date is not settled. When Intel images end, the kernel's x86-64 macOS leg needs a self-hosted machine or the owner's decision to drop the target (32.4). Today every pull-request leg, Windows on Arm included, runs on GitHub's standard hosted runners, free and unlimited for public repositories, and this repository is public (G02's `kernel.yml`), so the per-pull-request matrix costs nothing until then. What it did cost was waiting: GitHub's free plan runs 20 jobs at once and one kernel push starts about 125, so the organisation moved to GitHub Enterprise on 2026-10-28 (500 at once, US$21 a month for its one seat; 32.4, K06). If the matrix is trimmed to save money, the promise is no longer checked.

**Likelihood:** Medium to High. **Impact:** High.

**Mitigation built into the plan.**
- Deterministic count gates are cheap and run on every pull request; wall clock runs nightly.
- Each check runs at a stated tier (28.17).
- Costs are recorded in K06.
- OSS-Fuzz is an option (28).
- Section 32.4 asks the owner who provides and pays for the machines.

**Early warning.**
- Monthly CI cost against K06.
- Any matrix leg skipped or made optional.

### R27. Programs work around a late kernel

**What could go wrong.** If the kernel lags, Spare Construct or Spare Studio may write their own floating-point geometry so they can ship, as the old map planned with a Manifold port. Two geometry stacks then diverge, and moving to the kernel later changes models.

**Likelihood:** Medium. **Impact:** High.

**Mitigation built into the plan.**
- The layering rule: programs depend only on engine-geometry, and shared needs move into the kernel (5, 25).
- The build order puts Spare Construct's first needs first: flat solids, cuts, names and drawings.

**Early warning (proposed).**
- A scan of program crates for geometric predicates, boolean code or geometry dependencies.
- Program pieces asking for geometry the build order has not reached yet.

## 31.6 What we accept, knowingly

These are not risks to manage. They are limits the plan states openly.

- Default-mode surface intersection does not promise correct topology in every degenerate configuration (12.14).
- Bounds alone cannot decide whether two general curved surfaces truly touch. Contacts are bounded and reported instead (12.9).
- A sketch can have very many solutions. The intended one is kept by stored root choices, not found again (15.17).
- The kernel never computes results on a GPU or with a JIT (23), so field evaluation is slower than in tools that do. Display may use both outside the kernel.
- A foreign file's round trip may move lengths by at most q/2 (24).
- Platforms that break IEEE floating-point rules (32-bit x87 PCs, 32-bit Arm NEON) are outside the same-answer guarantee (21).

## 31.7 Open technical questions

Chapters 11 to 29 raised 212 open questions. This appendix merges the ones that ask the same thing. Each is filed under the chapter whose text its answer changes, so spine amendments sit under chapters 6 to 10, 20 and 27. A row gives the question, the chapters that raised it, and where it is decided: a build piece by its title in section 30, a decision note, an editorial fix, or another team. Kernel owners decide unless the row says otherwise.

A row whose answer is a decision note, or a note to a section of chapters 6 to 10, 20 or 27, is a spine amendment. Under 32.5 it goes into the consolidated amendment note if it can be settled now. If it waits on a piece's measurement, kernel owners accept a later decision note, except where it touches the principles, the grid and tolerance ladders, the edition promise or the licence, which stay with the owner.

The questions the owner settles in section 32 are listed once, in the first table, and not repeated. Building the first pieces (30.0) answered some questions, listed in the second table, and raised others; a row raised by a build names the piece whose build found it, such as "G02 build".

### Settled by the owner in section 32

| Question | Raised in | Settled by |
| --- | --- | --- |
| Whether circuit boards use q1 or q2 frames, given Gerber's 1 nm coordinates | 24 | 32.1 |
| Circle centres on the grid, as the brief literally says, or rational for constructed circles; exact arcs and conics; the fallbacks if their predicates cost too much (certified ellipse pairs, a coarser 2D angle target, certified decisions on constructed circles) | 14, 15, 17 | 32.2 |
| How "bounds add up" counts across repeated operations | 11, 13, 28 | 32.3 |
| What "certified" allows: a Taylor-style bound; rotation-minimising frames measured, not proven, against the ideal frame; filters "checked, not proven"; refusing at sharp field edges instead of a "topology unproven" class | 12, 13, 16, 28 | 32.3 |
| CI cost and hosting; OSS-Fuzz or co-op machines | 28 | 32.4 |
| The edition guarantee and checker fixes: the guarantee covers valid results only | 28 | 32.4 |
| Who approves as a kernel owner; who reviews proofs; who accepts spine amendments; who sets the kernel's share of Spare Construct's targets | 23, 26, 28, 29 | 32.5 |
| GPL and LGPL tools as out-of-process oracles; reading GPL source; Numerical Recipes code in oracle builds; the method-only procedure as the standard; the Unlicense and CC0-1.0 on the code allow-list | 24, 28, 29 | 32.6; the out-of-process oracles decided yes by owner action G75 |
| Patent searches, who does them and who pays: EMBER, Lifted Surfacing, Topological Offsets, transfinite patches, developable lofts, sheet-metal unfolding, the section 16 methods, Autodesk US 10,635,088, nTop's filings, IntegrityWare's claim, the sixteen unsearched families, and the trade mark and any patent on "Viennese Bend". The Google Patents status of the Pixar, Stam, T-spline and watertightCAD patents is an assumption until counsel confirms | 11, 13, 16, 17, 29 | 32.7, then piece "Patent register" |
| NLib's chain of title, the narrowing of option D, how "notices kept" is done, and an issue to NVIDIA | 12, 29 | 32.8, decided 2026-10-24: A, for all of the SMLib release |
| Writing to EMBER's authors, the csgrs and Hyper author, CADCraft, tangency-labs, McNeel (openNURBS, and so Rhino subdivision exchange) and Pixar (OpenSubdiv's TOST licence); the order of contacts | 11, 14, 16, 24, 29 | 32.9 |
| Revit exports in a public repository; anonymised real projects; an AutoCAD seat and terms check; the ruling on black-box observation of proprietary programs, and so CADCraft's test scenarios; checking our exports in proprietary programs | 14, 28, 29 | 32.10 |
| The licence of our own fixtures; ABC, IFC4.x-IF, Thingi10K, NIST STEP files, the CC BY-ND IFC samples, and step-conformance's exported parts | 28, 29 | 32.11 |
| The crate table, its thirteen layers and rules, `geom-make` and 30.5's three edges in layer 12 | 27, 30 | 32.13 (owner action G08) |
| Rotations: section 7's rounding or chapter 14's rational rotations; one type for 2D and 3D; rotated planes against 6.2's budget | 7, 14 | 32.14 (owner action G44) |
| Time budgets: a deadline only through the caller's cancellation, never a replayable outcome | 10, 23, 28 | 32.15 (owner action G71) |
| The reference machine, and the kernel's shares of Spare Construct's targets | 23, 28 | 32.16 (owner action G72) |
| Arena indices assigned in persistent-name order in every operation (8.2's amendment) | 8, 23, 24 | 32.17 (owner action G76) |
| The native format's CBOR tag, its name and its file extension | 24 | 32.18 (owner action G77) |
| NameScheme v1: the hash and its truncation, the bytes hashed, text rules, and the `TopoName` variants | 11, 20, 24 | 32.19 (owner action G78) |
| The content hash and the NameId hash, and their implementation | 20, 23, 24 | 32.20 (owner action G79) |
| Committed planes and points as Exact or Certified; how bounds add up through a history; ε_step | 6, 9, 11, 13 | 32.21 (owner action G120) |
| The predicate step ladder and rounded-input filters, as an addition to 6.2 | 6, 23 | 32.22 (owner action G121) |
| SMLib's NLib layers as a CI oracle, with its Numerical Recipes routines stubbed out | 12, 29 | 32.23 (owner action G157) |
| How our own hatch patterns and linetypes are authored and reviewed | 14, 29 | 32.24 (owner action G184) |
| The ellipse as a curve kind stored by a bounded axis ratio | 7, 14, 26 | 32.25 (owner action G185) |
| Integer 2D carriers, depth-one points and rational-centre arcs as an addition to 6.2; and 32.2, exact arcs from the start | 6, 14, 15 | 32.26 (owner action G186) |
| `geom-thread`, `geom-stair` and `geom-civil` in the kernel now, or in Spare Construct under 26.1 | 17, 26, 27 | 32.27 (owner action G238) |
| What the Sweep kind stores, and the conical helix | 7, 13, 24 | 32.28 (owner action G239) |
| Predicates of 576 and 768 bits for skeleton and mitre-offset events; the wide integers already in code | 6, 13 | 32.29 (owner action G240) |
| The default and the cap of `close_near_contacts` | 6, 11 | 32.30 (owner action G241) |
| The licence of Wang et al.'s 2021 contact benchmark | 19, 29 | 32.31 (owner action G285) |
| Which crate owns USD subdivision exchange, and how Chaikin creasing is carried | 16, 24 | 32.32 (owner action G312) |
| Alignment transitions as kinds or compositions, and their names | 7, 17, 26 | 32.33 (owner action G357) |
| TIN ties in plan-coordinate order rather than by persistent name | 6, 17, 21 | 32.34 (owner action G358) |
| How imported edges are marked Smooth | 6, 18 | 32.35 (owner action G384) |
| Hidden-line predicates of 576 and 640 bits | 6, 18 | 32.36 (owner action G385) |
| openNURBS: written confirmation from McNeel before porting its reader and writer | 16, 24, 29 | 32.37 (owner action G407): request drafted for the owner to send; open until the reply |
| The LandXML 1.2 schema's terms, and LINZ's LandXML examples | 17, 24, 29 | 32.38 (owner action G408) |
| Generating code from IFC and STEP EXPRESS schemas, and ISO 10303-42's current edition | 24, 29 | 32.39 (owner action G409) |
| Gerber's and IPC-2581's terms, and whether a second program needs board exchange | 24, 26, 29 | 32.40 (owner action G447) |
| Running AutoCAD for comparison, and its seat | 14, 28, 32 | 32.41 (owner action G448) |
| A nightly wall-clock miss blocks merges to `main` except the fix or a revert | 23, 28 | 32.42 (owner action G449) |

### Answered by the pieces built so far

| Question | Raised in | Answer |
| --- | --- | --- |
| Proven-bound sinpi, cospi, atan2, exp and ln: one early piece, not two; the CORE-MATH port effort and its wasm32 build were unverified (risk R13) | 16, 17 | Answered by G05 (issue #174, pull request #191). The board holds one piece. The five functions are ported in safe `no_std` Rust with written proofs, build for wasm32-unknown-unknown and wasm32v1-none, pass every test on wasm32-wasip1 under wasmtime with native's golden, and run within twice `libm`'s time natively (R13). What remains is under chapters 12, 21 and 28 below |
| `geom-math`, `geom-run`, the seeded generator and G25's tests hashed their goldens with 64-bit FNV-1a, where 21 and 28.13 ask for SHA-256 | 21, 28 | Answered: G03, G04, G25 and G32 moved to SHA-256 over the same recorded bytes before merging (their pull requests #189, #182, #193 and #185) |
| G05's CORE-MATH notices sat at the end of each module's documentation, without an SPDX identifier, and `geom-num`'s licence field named MPL-2.0 alone (rule 4, 29.20) | 29 | Answered by G05 (#191): the notices start each ported file under `MPL-2.0 AND MIT`, the crate's licence field says `MPL-2.0 AND MIT`, and `projects/engines/geometry/NOTICE` lists the source |
| ClusterFuzzLite's build-clusterfuzz path was unverified, so each harness was to get a thin cargo-fuzz wrapper (28.5) | 28 | Answered by G14 (issue #207, pull request #209). cargo-bolero 0.13.5's `build-clusterfuzz` packs the libFuzzer-built bolero tests, ClusterFuzzLite fuzzes them on every pull request that touches the code a target fuzzes, and no wrappers are written. One OSS-Fuzz build check cannot read cargo-bolero's start-up scripts, so `.clusterfuzzlite/build.sh` makes the same checks on the binaries; the layout is in `geom-fuzz`'s README |
| When GitHub retires its Intel macOS test machines (R26, 32.4) | 28 | In part. macOS 13 retired on 4 December 2025, and GitHub said x86-64 macOS ends when the macOS 15 image retires in fall 2027 (changelog, 2025-09-19). Its runner reference, read 2026-10-22, also lists `macos-26-intel`, so recheck before relying on the date. Dropping or keeping the target stays the owner's (32.4) |
| What the per-pull-request determinism matrix costs (R26, 32.4, owner action "CI hosting and cost") | 28 | Answered by G10 and owner action G07 (32.4, K06). Every target of chapter 21, and Windows on Arm, runs on GitHub's standard hosted runners (G02's `kernel.yml`), free for this public repository, up to 500 jobs at once on the organisation's GitHub Enterprise plan (US$21 a month, from 2026-10-28). Nightly fuzzing runs on the same runners, the oracle images are rebuilt into the oracle repository's Actions cache and never pushed to a registry (29.20, rule 8), and the benchmark and Revit machines are lent by an owner (28.17). The AutoCAD machine waits for owner action G448 |
| `Interval::atan2`: an enclosure over a box that handles the branch cut on the negative x-axis and the origin; until it existed, fields that use the atan2 opcode (16.2) and intersections that need it were Approximate only | G05 build; 12, 16 | Answered by G47: `Interval::atan2` follows IEEE 1788-2015's `atan2` (domain the plane without the origin, range `(−π, π]`): the whole range where the box meets the negative x-axis from both sides, otherwise the hull of its corners other than the origin, each correctly rounded and widened by one step (the exact 0 kept), and `OutsideDomain` for the origin alone; proven in `math/enclose.rs` and attacked against the big-number reference in `tests/attack_g47.rs` |
| The maths functions' strongest test, every input in CORE-MATH's worst-case lists (about 3.6 million), needed a CORE-MATH checkout at a pinned commit and ran in no CI tier, while the board asks for an oracle on 10 million inputs. Which check G47 keeps, and a nightly job that fetches and pins the lists | G05 build | Answered by G47: both. `kernel-nightly.yml`'s maths job fetches the five lists at the ported commit, checks each by SHA-256 and runs every input (`tests/math_full_worst_cases.rs`), then checks ten million inputs per function from the night's seed against the big-number reference (`tests/math_oracle.rs`); every build checks a prefix of the same inputs |
| The differentiation scalar's NaN tripwire (15.4), after G24 dropped debug tripwires on `Real` (chapter 21's row), and what the board's "fixed-size interval elimination" asks of the interval Newton operators | G52 build; 15, 21, 12 | Answered by G52. The tripwire is the check every operation on a `Real` makes, through `Real::new`: a NaN or an infinity in a value or a derivative of a `Dual` is `NotFinite` at the operation that made it, in every build, never a debug-only panic, and a `Dual` never holds a NaN. "Fixed-size interval elimination" is read as the eliminations at fixed sizes of one to four unknowns: the preconditioner's inverse by Gaussian elimination with partial pivoting (12.4), and Hansen and Sengupta's interval Gauss-Seidel sweep. The block versions on `geom-linalg` stay with G227 |
| How the work-list engine measures a box's image in space (the floor) and a kind's evaluation error (the quarter-step limit), what "evaluated in double-double" means for a box, and how a zero on a split line, or found from two boxes, is reported once | G53 build; 12, 23 | Answered by G53. A system gives, beside its residuals, the position its unknowns stand for in grid steps (`geom_num::isolate::System::position`, written once over `Scalar`): a box's image is the widest coordinate of the enclosure of the position over it, and its evaluation error is the width of that enclosure at its midpoint, all rounding. The error is measured on every box, not only near the floor, since double-double only narrows enclosures and never changes what is proven. Double-double evaluation is midpoint-radius ball arithmetic on `DD` with the radius rounded up (`geom_num::ball`); `exp`, `ln`, `sinpi`, `cospi` and `atan2` go through their interval versions, whose proven forms are in doubles, and a box whose balls fail where doubles succeed (a ball cannot record that a square is never negative) is enclosed in doubles. Undecided boxes at the floor, and refinements that stop shrinking, are taken again in double-double before they are flagged or refused. Zeros on split lines are proven by ε-inflation of the narrowed box (Rump); each proof box is grown while it still proves its one zero, and later boxes and earlier flagged regions are cut against proof boxes (Schichl and Neumaier's exclusion regions), so no zero is returned twice and no flagged region overlaps a proof box. A zero on the domain's boundary is flagged, since the proof box stays in the domain. The refinement cap's margin is eight steps (`REFINE_MARGIN`, its reason beside it). The floor and the quarter-step limit stay named in `geom_num::isolate` until an owner accepts them as rungs of 6.5 |

### Chapter 6: Numbers

| Question | Raised in | Decided in |
| --- | --- | --- |
| Extend 6.2's fixed-width predicates past 384 bits for straight-skeleton events: 576 bits for 2D event order and about 768 bits for 3D mitre offsets (our derivations, to be written out) | 13 | 32.29 (owner action G240): accepted with 6.2's conditions; pieces "Exact weighted straight skeleton" and "3D weighted straight skeleton for mitre offsets" write the derivations |
| The same for exact hidden lines: about 547 bits on grid-axis views and 633 bits for any direction or perspective (576- and 640-bit types), or exact visibility on grid-axis views only | 18 | 32.36 (owner action G385): both widths, every view, on 32.29's conditions; piece "Exact view predicates" |
| Fixed-width integers of several thousand bits on a cold path for general quadric pairs (Dupont, Lazard, Lazard and Petitjean), rather than arbitrary precision | 12 | Piece "Bit budgets for the exact algebraic decisions in intersections" |
| The real widths of 2D construction predicates: rational directions of about 44 bits, tangency positions about 70 bits finer than a step, the deepest possibly thousands of bits | 14 | Piece "2D-01", with its width generator and benchmark |
| Exact incircle on implicit TIN crossings would need about 1,024 bits (an estimate). Not planned; recorded as a limit | 17 | No piece; chapter 17 text |
| A predicate step ladder: identity, narrow i64 and axis checks before the filter, with a filter switched off on a target where it measures slower | 23 | 32.22 (owner action G121): accepted into 6.2 with two conditions; piece "Predicate step ladder, rounded-input filters and step statistics" |
| Integer 2D carriers (n below 2^41, c below 2^82), depth-one 2D points and arcs with rational centres, as an exception to "every stored coordinate is whole grid steps", like 6.3's exception for planes | 14, 15 | 32.26 (owner action G186), with 32.2 decided A; pieces "2D carrier, depth-one point and rational-centre arc predicates in geom-num" and "2D-04" build them |
| One rotation type for 2D and 3D: rational rotations from Pythagorean triples, as quaternions in 3D. Rotated planes then exceed 6.2's 384-bit assumption, so 6.2 and 11 must agree | 14 | 32.14 (owner action G44): one type, integer quaternions; rotated planes outside 6.2's budget are committed, so 384 bits stand; piece G46 builds it |
| Sketch-plane frames: lattice frames only where cells are at most 16 steps, rotated frames elsewhere. Should section 13 build swept bodies on rotated planes from the exact carrier now? | 15 | Piece "Sketch planes and the 2D-3D maps" |
| A committed plane whose proven deviation over its face is within q/2 counts as Exact, as a committed point does | 13 | 32.21 (owner action G120): no, committed planes and points stay Certified; piece "Plane commit routine" |
| A designed irrational tie (four roof planes meeting at one apex) keeps the committed sign and reports a short edge, rather than failing | 13 | Piece "Plane commit routine" |
| Derived values to name on the ladder: a resolution floor of 1q; epsilon_step = epsilon_fit/4; the solver block (tau = q/64, sigma_ind = 2^-12, sigma_dep = 1/(64S) and the rest); h_min = 8q; a default cap for close_near_contacts | 11, 12, 13, 15, 19 | Ladder module in geom-num, after 32.3 |
| Commit rounding is q/2 per coordinate, about 135 nm as a distance at q0. 6.5 should say so | 19 | Editorial fix to 6.5 |
| A named import angle on the ladder for marking imported edges smooth, or no kernel default (chapter 18's choice) | 18 | 32.35 (owner action G384): no kernel default, a recorded parameter; piece "Line classes, bands and continuity records" |
| Designed tangency travels as a recorded intent, and downstream code never compares angles with epsilon_ang. Committed lines tilt by about q/L, and subdivision G1 leaves kinks of about 1e-5 rad | 15, 16, 18 | Decision note to 6.5; pieces "Local-operation driver and topology-change taxonomy" and "Line classes, bands and continuity records" |
| Break TIN ties in plan-coordinate order (x, then y) rather than by persistent name, so identical points give identical surfaces whatever they are called | 17 | 32.34 (owner action G358): yes, for TIN ties only; piece "Exact Delaunay core" |
| A stricter rule for local operations near tolerant edges of imported STEP models than 6.5's "work within, never raise" | 13 | Note to 6.5; piece "Local-operation driver and topology-change taxonomy" |
| May a file's declared precision below 640q lower epsilon_heal's default? Today it can only raise it, up to the cap | 24 | Piece "geom-exchange basics: units, numbers to grid, frames, reports" |
| Wider integers already in code: `geom-num`'s `I512`, the total path of the plane classification and commit (G02), and `geom-math`'s exact stages in `Wide<4>`, `Wide<10>`, `Wide<20>`, `Wide<34>` and `Wide<40>`, up to 2,560 bits (G03). Accept each into 6.2's list, 23's cost model, 28.7's proofs and 29.3, or remove it | G02 and G03 builds | 32.29 (owner action G240): accepted as built, each with its derivation and extreme tests; 23's cost model adds them when measured |
| The commit's tie rule: half to even, chosen by G02 after 6.1's rule for angles, so a coordinate at 2^40 − 1/2 leaves the frame (6.3) | G02 build | Decision note to 6.3 and 6.5, with the ladder (32.5) |
| ε_ang as the relative length bound that classifies floating-point transforms as Rigid or UniformScale (G03), a second meaning of the rung (6.5's proposed rungs) | G03 build | Decision note to 6.5 and 7; piece "Points, vectors, boxes, frames and typed transforms" |

### Chapter 7: Geometry types

| Question | Raised in | Decided in |
| --- | --- | --- |
| Let an arc be stored by three points, or by two end points and a bulge, so IfcArcIndex and DXF bulge arcs with rational centres stay exact | 14, 24 | After 32.2: decision note to 7; piece "2D-04" |
| Ellipse axis ratio as a bounded fraction (denominator about sqrt(2a/q), at most about 2^21), and the rule for converting DXF's floating-point ratio | 14 | 32.25 (owner action G185): the bounded fraction, and the nearest such fraction by continued fractions, Certified unless exact; pieces "2D-04" and "2D-07 DXF import commit and healing" build them |
| NURBS knot and weight storage: f64, or dyadic knots with bounded denominators and small rational weights, which would make many more curved decisions Exact | 12, 14 | Decision note to 7 before piece "Bernstein toolkit in geom-nurbs" |
| Where the IntersectionCurve variant lives: geom-curve with opaque pool references (chapter 12's choice), or geom-surface | 12 | Decision note to 7 and 27; piece "Procedural intersection curve with certified cached fit" |
| Subdivision limit surface: the local rules and evaluator sit in geom-surface, only the net is stored, and the patch tree is a cache (16's choice, against the review's stored hierarchy). geom-intersect and geom-draw reach contours through the surface trait | 12, 16, 18 | Piece "SubD limit surface kind" |
| The surface trait gains an Unbounded derivative-bound result and a normal-cone query, because Stam's derivatives are unbounded from valence 5 | 16 | Piece "SubD limit surface kind" |
| Alignment curve kinds: named kinds or compositions for Helmert, Bloss, the Viennese bend, the cubic variant, and IFC's gradient curve and segmented reference curve; the four LandXML-only spirals; our LandXML-to-IFC mapping | 17 | 32.33 (owner action G357): one transition kind with laws; gradient and segmented reference curves are compositions; the four LandXML-only spirals stay data until published |
| A conical helix (tapered springs, scrolls) as an exact curve kind | 13 | 32.28 (owner action G239): the helix kind gains a radius rate; piece "Helical sweeps and geom-thread" |
| What the Sweep kind stores: the full FrameRule, twist and scale laws, bit-exact station frames and the cylindrical profile morph; draft surfaces as Sweep and Ruled kinds | 13 | 32.28 (owner action G239): all of it; piece "Frame rules and laws"; encoding in "geom-format: body records" |
| A procedural 2D projected-curve kind (a 3D curve plus a view, with a cached certified fit) | 18 | Decision note under 26.1; piece "Projected curve kinds and the canonical drawing form" |
| The TIN as the carrier of toposolid top faces, rather than a new piecewise-flat surface kind | 17 | Piece "Toposolid" |
| STEP B-spline volumes and locally refined splines: add them as kinds, leave them unsupported, or convert them with a bound | 24 | Decision note under 26.1 |
| A variable-width offset curve kind for wide polylines needs its decision note | 14 | Piece "2D-25 Wide polylines" |
| Mirrors keep grid points on the grid only about axis-aligned and 45-degree lines; other whole-number mirror lines are exact in fractions. 7 should say so | 14 | Editorial fix to 7 |

### Chapter 8: Topology

| Question | Raised in | Decided in |
| --- | --- | --- |
| Can geom-topo's arenas hold 2D arrangements efficiently, with fins as half-edges, or is a planar specialisation needed? | 14 | Piece "2D-09 Arrangement of lines, arcs and conics" |
| Re-check 8.1's "the structure Parasolid documents publicly" against Siemens's own pages and cite Weiler, Mäntylä and Jackson, because the evidence read a third-party copy | 29 | Editorial fix to 8.1 |
| Euler's formula for general, non-manifold bodies: the checker holds solids and sheets to it, shell by shell, and a general body only to the first stage of group 1; the non-manifold form (radial-edge counts) is to be chosen | G84 build | Kernel owners, with the first operation that makes a general body |
| Where a body's exactness record lives: the checker takes it beside the body, from the operation's `OpOutput`; whether `Body` itself carries it, as the native format's body index stores it (24.2) | G84 build | Piece "Body records" |
| The full checker in release test builds: `geom-check`'s `full` feature runs every group at `Level::of_build()`; which test builds turn it on, beside the shipped leg of the determinism matrix (28.4) | G84 build | Piece "Determinism matrix for operation logs" |
| Regions when each face names one shell: a solid body's shells belong to its solid regions and the outside void has none, as G80's wall body is built, so a void cavity has no shell of its own; whether faces need a shell on each side, as Parasolid's do, for cellular models (rooms as regions, 8.1) | G84 build | Pieces "Builder, Euler operators and attributes" and "Cells, rooms and space boundaries" |

### Chapter 9: Representations and exactness

| Question | Raised in | Decided in |
| --- | --- | --- |
| Lift a closed exact mesh to a plane-based body by default when it is mixed with B-rep operands, or only on request | 11 | Piece "Cross-representation boolean entry point" |
| Move converted subdivision faces near extraordinary points from Approximate to Certified once their bound is implemented | 16 | Piece "SubD to NURBS: packed patches and PCCM" |
| List RawMesh as an import state, with its rules | 19 | Piece "Exact mesh data model, RawMesh and the mesh checker group" |
| A sketch's stored geometry is Exact as data; how well it meets its constraints lives in diagnostics, not in its class | 15 | Piece "Commit plan and exact incidences" |
| A conversion edge from RawMesh through Field to Mesh, labelled Approximate | 19 | Piece "Conversion graph registry and route query" |
| Wording: field booleans take "certified distance plus winding number (exact sign for closed plane-based bodies)"; the table says "exact topology for 2D lines, arcs and conics" | 14, 16 | Editorial fixes to 9, the second after 32.2 |

### Chapter 10: Operations

| Question | Raised in | Decided in |
| --- | --- | --- |
| Mass properties: exact rationals for flat solids and polygonal regions (whole numbers only when every vertex is a grid point), certified for arcs and curves. Big-rational totals on request? | 17, 19, 28 | Editorial fix to 10.2; pieces "Cut, fill and volumes" and "Exact mesh data model, RawMesh and the mesh checker group", and the law catalogue |
| Line patterns use "certified arc length", not "exact arc length" | 14 | Editorial fix to 10.2 |
| Budgets: time is enforced only by the caller's cancellation, and memory is counted in kernel units, so 32-bit and 64-bit targets agree | 28 | 32.15 (owner action G71), with its note to 10.1; piece "Work meter, logical memory and cancellation" |
| Budget failures: persistent, deterministic model state (recommended), or never stored, like cancellation | 23 | Piece "Work meter, logical memory and cancellation" |
| Default for batch cutting: Atomic (chosen) or SkipFailedTools | 11 | Piece "Batch cutting (geom-bool)" |
| Blend defaults: overflow order; a batch blend all-or-nothing or per cluster; an unprovable G2 corner fails with a G1 offer or falls back with a warning; the default mitre limit; self-contact as an error or a self-union | 13 | Pieces "Blend feasibility analysis and preview query" and "Constant-radius fillets and chamfers with overflows", with Spare Construct's and the mechanical program's owners |
| Should a failed B-rep fillet offer a field blend as an explicit alternative? | 16 | The same pieces, with "Field opcode library" |

### Chapter 11: Booleans

| Question | Raised in | Decided in |
| --- | --- | --- |
| Confirm the departures from the review: IfcBoxedHalfSpace treated as a plain half-space; Contact::Within replaced by close_near_contacts; the exact path chosen by form, not class; milling on the exact path | 11 | Acceptance of chapter 11 |
| Can the arrangement split cells at grid positions that do not depend on content, so a local re-cut is bit-identical to a full one? Otherwise the 4 ms door move holds only up to about 50 openings | 23 | Pieces "Adaptive subdivision and the parallel task tree" and "Local re-cut" |
| Names in symmetric selections: qualify by operand key only on a collision (chosen), or always | 11 | Piece "Boolean names and history" |
| Booleans with subdivision operands near extraordinary vertices depend on section 16's certified Stam bounds; otherwise they fail, with a NURBS conversion as the remedy | 11 | Piece "SubD limit surface kind" (16) |

### Chapter 12: Intersections

| Question | Raised in | Decided in |
| --- | --- | --- |
| Contact policy: epsilon_fit serves as a contact radius only when certification cannot separate two entities. An unproven contact counts only when an input is Certified or Tolerant, never between two Exact inputs, and is always recorded | 11, 12 | Decision note against 6.5; pieces "Curved builder data model, broad phase and contact rule" (11) and "Tangent, singular and coincident analysis" |
| The cost of certify mode, and of extreme default-mode cases (a NURBS against its own offset at one step), is unmeasured; the default escalation policy waits for both | 12 | Pieces "Certify mode (a)" to "Certify mode (c)" |
| Primary details still to re-read: Hohmeyer's loop criterion, Schulz 2009, Nishita-Sederberg-Kakimoto clipping generalised to curves, characteristic-map charts, symbolic division at collapsed edges, and msun's ulp bounds; the licence of the QI code | 12 | Each in its piece: "General SSI part 1", "Poles, seams, knot lines and extraordinary vertices", "Analytic and transcendental enclosures" |
| The ssi-corpus adapter: accept matching flagged regions; place cases on the grid at a scale where the corpus tolerance is at least epsilon_fit; check that rounding kept the truth; carry the CC-BY attribution | 12 | Piece "Intersection oracles and acceptance harness" |
| Review points: valence-3 derivatives vanish rather than blow up (the fix is unchanged), and flagged items carry no exactness class | 12 | Acceptance of chapter 12 |
| Radian `sin`, `cos` and `pow` have no board piece: CORE-MATH's published sources do not support a port to the proven standard (29.4). Add a piece, with new worst-case searches, or state in 12.3, 12.16 and 29.4 that radian enclosures are never certified and callers use half-turns | G05 build; 12, 29 | Kernel owners: a new piece in section 30, or an editorial fix |

### Chapter 13: Sweeps, blends and offsets

| Question | Raised in | Decided in |
| --- | --- | --- |
| Exact sign tests over square-root expressions, for skeletons and offsets with arbitrary edge directions | 13 | A new research piece in section 30 |
| Curved skeletons for arc eaves | 13 | Piece "Curved-eave skeletons (research)" |
| Degenerate 3D mitre-offset events beyond the cross-section rule | 13 | Piece "3D weighted straight skeleton for mitre offsets", plus research |
| Interval certification of Lifted Surfacing output, for general swept volumes | 13, 16 | Piece "Swept volumes: general route" |
| G2 n-sided corners for more corner classes | 13 | Piece "Vertex blends and the corner matrix" |
| A proven bound for rotation-minimising frames (Wang et al., with interval derivatives) | 13 | A new research piece in section 30 |

### Chapter 14: 2D drafting

| Question | Raised in | Decided in |
| --- | --- | --- |
| Constructed circles (fillets, tangent circles, three-point and bulge arcs) get implicit rational centres, while drawn and imported circles stay on the grid. Section 13 stores 3D tangent constructions the same way | 14 | After 32.2: pieces "2D-04" and "2D-05 Tangent constructions" |
| Splines: exact spline-line crossings by root isolation need a bound on knot and weight sizes; the EXTEND default; choosing and versioning the fit-point algorithm | 14 | Pieces "2D-08 Spline and spiral contacts in 2D" and "2D-27 Spline fitting and biarc conversion" |
| Values to set: the bound on whole-number directions converted from DXF hatch angles; the default hatch line budget; the default acceptance kit-ops passes (proposed: Certified with epsilon_fit and epsilon_ang) | 14 | Pieces "2D-21 Hatches and linetypes" and "2D-04" |
| The 2D R-tree: chapter 29 keeps rstar out of kernel crates unless it is ported (29.8). geom-2d writes its own small integer R-tree, or ports rstar; if ported, a test shows its results do not depend on insertion order | 14, 29 | Piece "2D-19 Snaps, tracking and the 2D index" |
| The engines README says engine-draw2d does "2D booleans"; restate that as display-only linearised booleans | 14 | Editorial fix to the engines README |
| Review points kept: the JTS test files are reusable with their notice (EDL-1.0), and snap points are exact on their carriers using fractions | 14 | Acceptance of chapter 14 |

### Chapter 15: Sketches

| Question | Raised in | Decided in |
| --- | --- | --- |
| 3D sketches in geom-sketch and assembly mates in geom-assembly; when the mechanical program's mapping splits that piece | 15 | Piece "3D sketches and assembly mates (geom-assembly)", timed with the mechanical program's mapping |
| Which layer records SketchUnsolved (kit-ops or each program); whether the default is to accept the upstream edit and mark the sketch unsolved, or to refuse it | 15 | Piece "Regeneration state for unsolved sketches", with kit-ops' owners |
| Exact commit of fillets, and of points on circles other than a defining point | 14, 15 | Piece "Commit plan and exact incidences", with "2D-05 Tangent constructions" |
| Exact rank by Bareiss elimination: its cost on blocks of about 1,000 unknowns | 15 | Piece "Exact-rank spike: Bareiss elimination on a rational witness" |
| The size limit N (2,000 until measured) above which rigid-cluster search is skipped, measured on outlines of 5,000 to 10,000 entities | 15 | Piece "Rigid clusters (Hoffmann-Lomonosov-Sitharam), generic in D" |
| Solving with inequalities (an active-set extension) for mechanical design | 15 | Scheduled with the mechanical program's mapping |
| The boundary for family formulas: the kernel takes numbers and polynomial relations, and kit-ops evaluates formulas, conditionals and lookup tables | 15 | Spare Construct's owners |
| Fixed-width SIMD in geom-num's factorisations in a later edition, checked by the replay matrix | 15 | A later edition's decision note |
| faer's behaviour across platforms is inferred from its manifests, not tested; this matters only if depending on faer is ever reconsidered | 15 | The section 21 replay matrix, if reconsidered |
| Confirm fiksi's density-test sign and its randomly seeded hasher by running the code before quoting them | 15 | Piece "Sketch test corpus, oracles and fuzzing" |
| Review departures: the undamped minimum-norm step instead of keeping lambda above zero; hold-and-release dragging instead of a weighted projection | 15 | Acceptance of chapter 15; the corpus can test both |

### Chapter 16: Free-form

| Question | Raised in | Decided in |
| --- | --- | --- |
| Biermann, Levin and Zorin's modified boundary rules as an option, which breaks OpenSubdiv and USD parity | 16 | Piece "SubD control net, options and uniform refinement" |
| Evaluation parity with Rhino at extraordinary vertices (Rhino's G2 claim is unverified) | 16 | Piece "SubD exchange: Rhino .3dm", after McNeel's answer (32.9) |
| The isotopy proof for Manifold Dual Contouring under Plantinga and Vegter's condition is unpublished. If it fails, certified output uses Plantinga and Vegter's own construction | 16 | Piece "Certified field mesher" |
| Is valence 50 the right ceiling for the constant eigen-tables? | 16 | Piece "Eigen-table generator and constant tables" |

### Chapter 17: Survey, terrain and civil

| Question | Raised in | Decided in |
| --- | --- | --- |
| Default for crossing breaklines: Error (chosen) or Prefer(priority); the paste seam rule, or an error instead. Ask practising surveyors | 17 | Piece "Breaklines, crossings and snap rounding" |
| The angle unit (1/2,500 arc-second) and lattice spacing (2^39 steps): confirm that no instrument or legal record needs finer, such as 0.000001 gon | 17 | Piece "Angle units and core COGO" for the angle unit. G48 kept the lattice spacing at 2^39 steps: a power of two, so origins stay whole steps at every grid step, and even, so lattice frames round a midpoint alike |
| Earthworks at epsilon_fit by default: a 50 km corridor may hold millions of vertices, so benchmark first | 17 | Piece "Corridor sectioned solid and faceting" (proposed target: 60 s and 4 GB) |
| IfcMapConversionScaled with unequal X and Y factors, or Z not one: per-axis factors in the frame record, or kept in the foreign recipe (chapter 24's choice) | 24 | Decided in G48: kept in the foreign recipe. The record has one horizontal scale factor and never scales heights (17.1), so a file whose factors differ keeps them as it gave them, and its geometry enters through a recorded transformation |
| IfcCurveSegment's spiral parameter spaces need an implementer agreement; our stations follow segment lengths | 17, 24 | Piece "IFC 4.3 terrain, alignment and corridor mapping", with buildingSMART |
| Mitred grading corners along a sloping feature line: no published method | 17 | A new research piece in section 30 |
| PROJ in the browser: a wasm32 build, or a pure-Rust port of the methods used (PROJ's MIT licence allows it) | 17 | The kit-ops work board |
| The Θ(n²) edge-crossing bound for steepest-descent paths is unverified | 17 | Piece "Flow, sight and raster DEM input" |
| A Ground frame's rotation: `AngleUnits` turned into a direction by proven sinpi and cospi (17.1), the exact integer direction G03 built (32-bit components, about 2^-31 rad, 17.2's second form), or both, and its width. The board builds `AngleUnits` only in piece "Angle units and core COGO", which needs piece "Frame records, units, epochs and lattices", the piece 17.1 says stores them, so either the order or 17.1 changes | G03 build; 6, 17 | Decided in G48: the exact integer direction G03 built (32-bit components) stays the record's form, so every conversion stays exact or correctly rounded; piece "Angle units and core COGO" turns a bearing in `AngleUnits` into that direction with the proven `sinpi` and `cospi`, and the bearing as given stays in the recipe. The board's order stands, and 17.1 says so |
| The widths a frame record allows: G03 takes an origin within 2^60 steps, map inputs within 2^62 m and a scale whose numerator and denominator are below 2^32, so a scale factor given to ten decimal places does not fit | G03 build | Decided in G48: scale factors are ratios of 64-bit integers (19 decimal places), linear units 64-bit ratios between 2^−32 and 2^32 m, origins within 2^60 steps; map values of any size convert exactly or report their exact coordinate, saturated only beyond `i128` |
| Rounding into a frame is half to even, which is not shift-invariant: frames whose origins differ by an odd number of steps may round a midpoint map value to different map positions | G03 build | Decided in G48: half to even stays (6.3). Lattice frames, 2^39 steps apart, agree; other frames on a shared lattice whose origins differ by an odd number of steps may round an exact midpoint differently, as `geom_math::convert` states |
| What an out-of-range conversion reports. `to_frame` reports an estimate (the scaled map value, or the double-double's leading part), which can state a false "at least" bound in a Ground frame with a scale above one. `shift_to` checks overflow before the lattice, so two frames both off the lattice and far apart report `OutOfRange` with a value that is not a coordinate, not `FrameMismatch::NotOnLattice`. Making the values exact means extending the exact machinery past its documented budget | G03 build; 22 | Done in G48: `to_frame` reports the exact rounded coordinate (a bisection with exact signs in 1536-bit integers), saturated only beyond `i128` and then with its true sign, and `shift_to` decides the lattice before the range, reporting the exact offset |
| Moving `geom-math` onto `geom-num`'s double-double type once G03 and G25 have both merged: G03's own `DD` has looser, hand-derived bounds, and the move changes two errors (division by zero becomes `NotFinite`, not `Degenerate`; the root of a negative becomes `OutsideDomain`, not `NegativeSqrt`). No board piece holds the move | G03 and G25 builds | Done in G48: `geom-math` uses `geom_num::dd::DD`, and its own type is gone; the two errors changed as stated |
| `sinpi` and `cospi` take an `f64` in half-turns, and `atan2` returns radians. Who reduces an integer angle to an octant exactly, and turns a radian `atan2` into whole angle units with a certified rounding: piece "Angle units and core COGO", or the maths module, with an `atan2` in half-turns | G05 build; 17 | Piece "Angle units and core COGO". G47 left both steps there: they need the angle unit, which `AngleUnits` defines, and its rounding rule, while the maths module now supplies what they are built from (the exact half-turn `sinpi` and `cospi` with their interval versions, and `Interval::atan2`, whose enclosure divided by an enclosure of π gives half-turns) |

### Chapter 18: Drawings

| Question | Raised in | Decided in |
| --- | --- | --- |
| Chapter 14 now gives rational points, exact arcs and conics, and loops that touch at a point. Still open: how arcs and conics are committed at explode, since snap rounding is proven for segments only | 18 | Pieces "2D-26 Snap rounding for explicit output" and "Drawing storage, versions, explode and export" |
| Dimensions to free-form silhouettes: allow weak ordinal branch keys, or force them onto model or carrier references | 18 | Piece "Drawing references: dimensions and overrides" |

### Chapter 19: Meshes, spatial index and clash

| Question | Raised in | Decided in |
| --- | --- | --- |
| One exact constrained Delaunay triangulation has three answers: our own code with spade as a model (17), a port of spade into geom-mesh::cdt (19), and a port of Geogram's CDT_2d (29). Pick one source and one home | 17, 19, 29 | Piece "Exact constrained Delaunay triangulation, shared with terrain" |
| Review the interior-only refinement proof (Jung's theorem, apex points, three spacing limits) and the 1/2·M·r² bound credited to Waldron 1998. The fallback is edge-level refinement rounds | 19 | Piece "Edge-first certified tessellation" |
| Port meshoptimizer to safe Rust (Large), or keep the C++ library in the renderer and give up bit-identical display chunks | 19 | Piece "Safe-Rust port of meshoptimizer's simplifier and meshlet builder" |
| Simulation meshes: is a boundary proven to lie inside an envelope (rather than keeping every input triangle) acceptable, and what is the default envelope width? | 19 | The simulation engine's owners; piece "Volume mesher design spike" |
| Topological Offsets: confirm its guarantees and whether it needs a background tet mesh; add Panozzo to chapter 13's citation | 19 | A design note before piece "Mesh offsets, hollow and thicken (Topological Offsets method)" |
| Clash for Spare Construct A13: general curved pairs give proven intervals, not single exact numbers; how NearLimit pairs are shown; whether "largest ball inside both" matches A13's depth rule | 19 | Spare Construct A13 |

### Chapter 20: Naming and history

| Question | Raised in | Decided in |
| --- | --- | --- |
| Bounded names become Bounded { kind, by, ends, ord: u16 }, because ord: u8 overflows at 256 edges between the same two faces | 11 | 32.19 (owner action G78): `ends` taken, `ord` not, remaining collisions are `Weak`; piece "Boolean names and history" |
| A regenerated step keeps its OpId. Re-importing a revised file is a new version of the same import operation, so Spare Construct's log treats an import as an editable operation with its source file as an input | 17, 24 | Note to 20.1; Spare Construct's operation log |
| Reword "indices are never persisted" to "never used as a reference outside its body record", because arena positions are record-local in the file | 24 | Editorial fix to 20.1 |
| A new resolution result, HistoryPruned, distinct from Deleted | 24 | Piece "PruneHistory operation and HistoryPruned resolution" |
| The name scheme fixes NFC normalisation of hashed text, and its Unicode version | 24 | 32.19 (owner action G78): NFC under Unicode 18.0.0 for v1; piece "geom-format: names, history, recipe envelope, attributes and provenance records" |
| The 128-bit NameId value and the history record types live in geom-topo, so geom-mesh can use them; geom-name keeps the naming graph and resolution | 19 | 32.13: in geom-op, below every crate that names or records history; geom-name keeps the graph and resolution |

### Chapter 21: Determinism

| Question | Raised in | Decided in |
| --- | --- | --- |
| One zero: a zero `sinpi` or `cospi`, a zero error part of an error-free transformation, a zero interval bound and an exact zero `ExactSum` are +0, while the other maths functions keep IEEE 754's −0 (21). Accept the rule and its exceptions | G02, G05 and G25 builds | Decision note to 21; the same-answer promise stays with the owner (32.5) |
| Debug tripwires on the NaN-free scalar, which the board asks for: G02 has none, and behind its one constructor a tripwire on a `Real` could never fire. Keep the requirement, aim it elsewhere, or drop it. And whether `ExactSum::round_ieee`, which can return NaN by design, stays public with only a written rule against kernel use | G02 and G25 builds | Piece G24, 2026-10-27, decided by research under the owner's standing rule: the tripwires are dropped. `Real`'s only constructor refuses NaN and the infinities in every build, which is stronger than a check in debug builds alone; a tripwire on `eft`'s raw inputs would be a panic in kernel code in debug builds (principle 13) and a branch no test takes in the foundation set (28.8's bar of every branch). "No NaN anywhere" (the G01 evidence, 05) stays enforced by that constructor, the rule that kernel code feeds `eft` only checked scalars, fuzzing and the test that the ports make only the canonical NaN. `ExactSum::round_ieee` stays public under its written rule against kernel use |
| wasm32's thread legs: wasm32-wasip1 has no threads, so the tests at 2, 3 and 8 threads that 21, 23.4 and 30.3 ask for are skipped there (G03, G05) or run every task on one thread (G04). Add a threaded target (wasm32-wasip1-threads, or browser workers), or state that wasm32 runs serially, with pools that permute the order | G03, G04 and G05 builds | Pieces "CI matrix and check tiers" and "Browser builds: threads, memory ceiling and cancellation" |

### Chapter 22: Errors, queries and AI

| Question | Raised in | Decided in |
| --- | --- | --- |
| `kit-core`'s permanent IDs and units, which the facade is to convert at its boundary: `kit-core` holds no code until its program reaches the Design stage, so nothing is converted yet, and lengths and angles cross the facade in the kernel's own grid steps and units | G100 build | Spare Kit's `kit-core`, then the facade |
| How the MCP tool descriptions name the facade's value types: a body, curve, surface or name is an object titled by its type, whose form the command layer serving the tools gives; whether the kernel should publish JSON forms for them | G100 build | Spare Kit's command layer (EXTENDING.md, section 2.2) |
| `cargo-semver-checks` on every pull request (28.8) as well as before each release: until the first Stable release (G482) the facade is version 0.0.0 and every change would read as breaking, so G100 runs it before releases only | G100 build | Piece "First Stable release of engine-geometry" |
| Confirm the new error kinds: ExactnessNotAccepted, NotRepresentable, OutOfRange and UnboundedResult; Degenerate added to the base; RootChoiceUndetermined refining Ambiguous; BranchLost and CommitFlip | 14, 15 | The chapter 22 error catalogue, generated from the code |
| The output shape of the model-diff query used to review a proposed change (22) | 22 | The piece that builds the diff query |
| How `geom-num`'s `NotFinite`, `OutsideDomain`, `WrongDirection`, `DivisorContainsZero`, `NegativeSqrt` and `EmptyInterval` map to base kinds; whether `NumError::BudgetExceeded` (a defect, so `Internal`) and `ToleranceExceeded` (a certified bound, so `ExactnessNotAccepted`) are renamed so their names stop inviting the wrong kind; and a `geom-math` error for its own defects, which now carry `geom-num`'s message (22) | G02, G03 and G05 builds | Answered by G82: `NotFinite` is `Degenerate`, `OutsideDomain` `NoSolution`, `DivisorContainsZero` and `NegativeSqrt` `Undecided`, `EmptyInterval` and `WrongDirection` `Internal`, each with its reason on `geom-op`'s `NumDetail` and in 22. No rename in `geom-num`: `geom-op` wraps every `NumError` into a `NumDetail` named by its meaning (`BitBudget`, `BoundOverCeiling`), and a crate's own error never reaches a caller (22). No `geom-math` error type: its defects wrapped are `Internal` with the operation's path and message |

### Chapter 23: Performance

| Question | Raised in | Decided in |
| --- | --- | --- |
| The reference machine for all targets: proposed an 8-core x86-64 desktop with 32 GB and NVMe for gates, plus an aarch64 laptop nightly | 23, 28 | 32.16 (owner action G72): the owner's spare tower, on its 8 performance cores; no Arm timing reference until one is lent |
| Accept the proposed kernel shares of Spare Construct's ten targets, the browser estimates and the 5 µs overhead per trivial operation until scenarios measure them. Who sets the split is settled in 32.5 | 23, 28 | 32.16 (owner action G72): accepted; piece "Spare Construct scenario generator, calibration and target benchmarks" measures them |
| Benchmark targets in other chapters are proposals to replace with first measurements: drawing targets (a full plan, every open view after a door move, a 50 km corridor plan); gridded LiDAR at 40 s per 10 million points; wasm within three times native; a 1 ms live clash re-test; 30 s to tessellate the reference model | 17, 18, 19 | Each chapter's benchmark piece, gathered in the scenario pieces |
| Instance records: 56 bytes with the box in the tree leaf (19), or up to 88 bytes (23); f64 or integer boxes | 19, 23 | Piece "Placed instances", aligned with "Instance tree for huge and nested assemblies" |
| I320 and I384 arithmetic on wasm32 is unmeasured; a double-double filter stage is the reserve if the section 23 gates fail. Measured by piece G42 (R4): the 384-bit path costs 1.5 to 1.9 times native x86-64 in the limb variant, against 2.5 to 4.1 times as shipped | 11 | Piece "Wide-integer variants and WebAssembly benchmark" |
| Browser threads need pinned nightly Rust with build-std, and cross-origin isolation: ship a separately built threaded variant, or run serial-in-worker until threads reach stable Rust | 23 | Piece "Browser builds: threads, wide arithmetic, memory ceiling and cancellation" |
| Two browser builds, with and without wide arithmetic, chosen at load | 23 | The same piece |
| Plane storage at building scale (estimated at 64 to 96 bytes per plane) and its fallbacks: compact widths, f64 copies made on demand, shared planes | 23 | Piece "Arena layout and plane memory benchmarks" |
| Remote global edits: replay the operations, or fetch the changed chunks by content hash | 23 | Spare Construct's owners |
| Generated exact code (the plain-`i128` and width-aware-limb variants of 23.3) needs native or wrapping multiplies, which `geom-num`'s lints deny: a checked step at every site (cost unmeasured), or an allowance with its written bound and Kani certificate. Piece G42 keeps a checked step at every site. On 32-bit limbs the check costs nothing: the compiler sees both operands below 2^32 and emits one plain multiply. A checked 64-bit product costs a `__multi3` call on wasm32 without wide arithmetic. Each kind of limb step also has a Kani proof that it never fails on its certified operands | G02 build; 23, 27 | Pieces "Predicate generator" and "Wide-integer variants and WebAssembly benchmark" |
| Whether a three-plane point caches its homogeneous coordinates (152 more bytes per point) or recomputes them at each use (6.3) | G02 build | Piece "Plane type, canonical planes and arrangement predicates" |
| A phase's work is bounded by its item count times R, not by R (23.10). Do phases need a size limit? | G04 build | Answered by G83: no size limit. A phase's items are things of the input or output the operation counts and charges for in its charge schedule, so n is fixed by the input, which the caller chose and can split; its worst case, n times R, is bounded by the input and the budget, and the end-of-phase failure is the same everywhere. A limit would be one more number fixed by each operation version that refuses inputs the budget allows, and a runaway in time is the caller's cancellation's to stop (32.15) |
| Cancellation within 1 ms: `geom-run` polls every 64 charges, and its one measurement, under full load, had a 3.5 ms worst case. Measure on an idle machine, and choose the interval for costly items | G04 build | Answered by G83: through `OpContext::run`, an operation charging a unit a step and cancelled from another thread stopped in a median of 2.1 µs, at worst 5.4 µs, over 51 runs on a development machine with other builds running; `geom-op`'s `tests/attack_g83.rs` holds the median under 1 ms on every native leg. The interval stays 64 charges: at 1 ms that allows about 15 µs a charge, so an operation whose single charge stands for more work (a costly item charged once) also calls `WorkMeter::poll` at each such step. Idle-machine timings are the reference runner's (G107) |
| The helper's own cost: serially 27 ns per item against 4 ns for a plain loop on trivial items, measured under load. An overhead budget, and a rule for when small loops bypass the helper | G04 build | Piece "Deterministic parallel helper (geom-run, geom-pool)" |
| A repeated key in a batch: `map_each` takes equal keys in input order, which makes batch-total failures depend on input order (10.1). Which layer rejects a repeated key as `InvalidInput`: `geom-run` or the batch runner | G04 build; 10, 22 | Piece "Batch runner" |
| The count gates as built (G06): a comment on the pull request itself needs a trusted workflow with a write token; the loop around each benchmark dilutes the 5% gate for the cheapest operations (about 46% for interval negation); the growth check is damped by fixed costs (28.14); `catalogue::UNMEASURED` exempts functions from principle 12's benchmark; the fuel host starts each run with 2^50 fuel, so a stuck benchmark runs to the job's timeout; and a failed fuel comparison skips the growth table | G06 build | Piece "Benchmark harness and count gates", with kernel owners |
| Miss counts of 20 or more also move from code or data layout alone. G18a's first run, which changed only the gate's configuration and its build's dependency features, raised L1 misses on `exact_sum` rounding benchmarks by 4.7% and 6.9% on x86-64 (339 to 355, 218 to 233), instructions unchanged and estimated cycles up under 0.1%; the 5% gate on those counts fails such a change. A larger floor, one derived from measured layout variance, or judging every miss count through estimated cycles (23.13) | G18a build | Kernel owners (principle 12 stays with the owner, 32.5); piece "Benchmark harness and count gates" |
| No board piece owns the per-benchmark logical-bytes gate of 23.13's Memory row, or moving the wall-clock benchmarks written before `geom-bench` (G03, G04, G05, G25) under the count gates | G04 and G06 builds | Piece "Benchmark harness and count gates" |
| Principle 12's "every operation ships with a benchmark": each numeric primitive, such as each double-double operation, or at least one per piece, as 30.1's standard bar says | G25 build | Kernel owners; the principles stay with the owner (32.5) |
| The double-double type's fused multiply-adds run as `libm`'s software routine on wasm32, and its products, quotients and roots use them inside each operation, so they cannot fall back to Dekker without changing bits. Confirm the proven FMA algorithms and measure them on wasm32, or choose algorithms without FMA, with other, weaker published bounds; evidence 05, section 1, advised keeping Dekker | G25 build; 17, 23 | Piece "Double-double arithmetic and ExactSum", with a double-double benchmark on wasm32 |
| `atan2` runs at 2.2 to 2.6 times `libm`'s time on wasm32 (G47; the board's target is twice), because WebAssembly has no fused multiply-add instruction and its first fast path needs two rounded ones, `fma(−t₀, x, y)` and `fma(t₀, y, x)`, which `libm` computes in software. Ways: a correctly rounded fused multiply-add faster than `libm`'s for WebAssembly (Boldo and Melquiond's emulation by rounding to odd, doi:10.1109/TC.2007.70819), proven bit for bit against `libm`'s; or re-deriving the fast path's error bound for a numerator and denominator rounded twice, which keeps every result (the fast path returns only correctly rounded values) and moves only which inputs reach the slower paths; or a deterministic WebAssembly fused multiply-add, should one be standardised (`f64x2.relaxed_madd` may round once or twice, evidence note 05). Natively all five are within twice `libm`'s time | G47 build; 21, 23 | Kernel owners: a piece in section 30, or acceptance of the measured ratio for wasm32 against chapter 23's browser estimates |

### Chapter 24: File format and exchange

| Question | Raised in | Decided in |
| --- | --- | --- |
| Canonical body encoding: arena order with fixed 4,096-row chunks (24), or name order (23's draft). Name order first needs an "outputs independent of input arena order" invariant, with a shuffle test | 23, 24 | 32.17 (owner action G76): arena order is name order, so both hold; the shuffle test is the builder's (G81); piece "geom-format: body records" writes it |
| The content hash: SHA-256 or BLAKE3, in a safe portable implementation or an audited exception crate (both common crates use unsafe SIMD); and whether to adopt the name-ordered bucket-tree hash | 23 | Hash: 32.20 (owner action G79), SHA-256 from `sha2`, audited by G86. The bucket tree: piece "Content hashes and cache keys", by benchmark |
| Sign chunks written by a released kernel edition, or rely on per-chunk checker versions and lazy re-checks | 23 | Piece "Container mapping and trust on open" |
| Bodies opened with fallbacks stay read-only until an acknowledgement is logged | 24 | Piece "Container mapping and trust on open" |
| Register CBOR tag 0x534B4746 with IANA, and check the SKG name and the .skg extension for clashes | 24 | 32.18 (owner action G77): the tag is unassigned and the owner sends the request; SKG is clear; the extension becomes `.skgf`, because Skrooge uses `.skg` |
| Proposed figures to replace with measurements: the 64 MiB payload cap, 4,096-row chunks, 500 MB/s CBOR decoding, 100 MB/s Part 21 parsing | 24 | The geom-format and geom-p21 pieces |
| One shared hardened XML and zip crate for the kernel and Spare Office would change section 5's "no other Kit crate" rule | 24 | Spare Kit's owners, by decision note |
| 3D vertex rounding has no open implementation and a steep worst case: apply it only to flagged regions within a budget (chosen), or default imported meshes to q1 frames | 24 | Piece "Rounding imported polyhedra" |
| LAS and COPC: read a scale stored as f64 as the shortest decimal that produces it | 24 | Piece "Point-cloud exchange rules" |
| Items to check against primary sources: MBx-IF practices; DXF MESH subdivision rules and HELIX fields; Part 21 third edition; whether AP242 includes Part 42's clothoid; 3dm mesh vertex precision; E57; Pratt's rational Dupin cyclides | 24 | Each format's piece |
| ARC_DIMENSION has no record in the public DXF reference, so its mapping is open | 14 | Piece "geom-dxf read and write" |
| USD: which crate owns subdivision exchange, and a convention for carrying Chaikin creasing in custom data | 16 | 32.32 (owner action G312): `geom-meshio`; `spareware:creaseMethod`; the terms check stays with piece "Decision notes for formats not yet owned" |
| DWG and other reverse-engineered formats | 14, 24, 29 | Stays REJECT. Only an owner can clear it, after legal advice (README); Spare Construct's owners say whether they want it |
| laz-rs contains unsafe code: allow it in Spare Construct's adapter, or port the codec | 24 | Spare Construct's owners |
| Whether format crates may use std: answered by chapter 24. Readers use no_std byte sources, and point clouds stay in the program adapter | 29 | Answered |

### Chapter 27: Crates and folders

Proposed in 31.5 (R23) and 32.5: one consolidated crate-list note settles all of these. The owner accepted chapter 27's proposed table, layers and rules as that note on 2026-10-28 (owner action G08, 32.13); a row that names 32.13 is settled by it, and its piece builds to it.

| Question | Raised in | Decided in |
| --- | --- | --- |
| A home below the facade for the cross-representation boolean: a new geom-combine, or bool added to geom-convert's dependencies | 11 | 32.13: geom-combine; piece "Cross-representation boolean entry point (geom-combine)" |
| One deterministic box tree below geom-bool, geom-mesh, geom-2d and geom-terrain: in geom-math (11, 23), or in a new geom-bvh (19) | 11, 19, 23 | 32.13: geom-bvh; piece "Deterministic integer box tree and culling" |
| geom-run (a no_std parallel helper, budgets and cancellation) and geom-pool (rayon and Web Workers behind a threads feature). The adapter's name, folder and dependency disagree: `engine-geometry-pool` in `projects/engines/crates/`, with a `no_std` facade that re-exports only the `Pool` trait (23.4), or `geom-pool` in layer 13, with `engine-geometry` on it behind a `threads` feature and allowed `std` (27, 29.4, the board, G04's `pool.rs`, G09's crate table) | 23; G04 and G09 builds | 32.13: geom-pool in layer 13, as built; piece "Deterministic parallel helper (geom-run, geom-pool)" |
| Deterministic linear algebra inside geom-num (15, 29), in a new geom-linalg (16), or in geom-math | 15, 16, 29 | 32.13: geom-linalg; nothing of linear algebra is built in geom-num |
| geom-profile, to carry geometry between sketches and the model | 15 | Piece "geom-profile: references in, profiles out" |
| geom-panel and geom-analysis, with chapter 16's dependency additions | 16 | 32.13: accepted; pieces "Panelisation and rationalisation" and "Class-A analysis" |
| geom-civil in the kernel, so terrain and alignments stay independent, or inside Spare Construct under a strict reading of 26.1; and geom-landxml | 17 | geom-landxml: 32.13. geom-civil: 32.27 (owner action G238), in Spare Construct until a second program needs it; piece "geom-civil compositions" |
| geom-exchange, geom-p21, geom-heal and a development-only EXPRESS generator (geom-express); geom-cloudio withdrawn | 24 | 32.13: the three crates with 30.5's edges; the EXPRESS generator is a module of geom-gen; geom-cloudio withdrawn |
| A development-only geom-proof crate for the predicate generator's proofs, or keep them in geom-num and geom-test. The cargo-verus fallback makes vstd a build dependency of geom-num | 28 | After piece "Verus twin spike" |
| geom-2d also depends on topo, name and intersect; line and arc predicates sit where both crates reach them; conic predicates in geom-num; the 2D R-tree in geom-2d or shared | 12, 14 | 32.13: geom-2d's edges, and the R-tree becomes geom-bvh; where line, arc and conic predicates sit, piece "2D-09 Arrangement of lines, arcs and conics" |
| geom-intersect also depends on geom-nurbs, and an ImplicitBound trait in geom-surface lets fields be traced without upward dependencies | 12 | 32.13: accepted; piece "Implicit-parametric tracer and level curves" |
| geom-mesh also depends on curve, surface and intersect | 19 | 32.13: accepted; piece "Edge-first certified tessellation" |
| geom-poly's job line names indicators and piece selection (decided in 11). Still open: whether the maximal-face merge, with T-junction insertion, moves out of geom-bool's rebuild into geom-poly so geom-mesh can share it (19), or stays in geom-bool (11) | 11, 19 | 32.13: the merge moves into geom-poly, as 27's job line says |
| Chapter 13's placements: frame rules, station frames and surface extension in geom-surface; skinning, Gordon, Coons and N-sided fills in geom-nurbs and geom-surface; plane commit in geom-poly; the local-operation arrangement step and change classification in geom-bool; TopologyChange records in geom-name; isoclines in geom-intersect. Its additions: sweep on poly, intersect, bool and skeleton; offset on skeleton, name and topo; skeleton on poly; blend on name, topo and check. Its reading of section 5: an operation may use another operation's public interface (geom-offset using geom-skeleton's skeleton), never its internals | 13 | 32.13: accepted |
| Chapter 17's additions: terrain on poly, bool, skeleton, curve, surface and name; sweep on poly; align on name; cloud on nurbs and surface. The corridor solid goes in geom-sweep and the corridor model in geom-align | 17 | 32.13: accepted |
| geom-draw's row adds mesh, and every row lists all its direct dependencies | 18 | 32.13: accepted |
| Swept volumes: routes 1 and 2 as a module of geom-sweep, route 3 as a space-time module of geom-field, with no new crate | 13 | 32.13: no new crate; pieces "Swept volumes: exact routes" and "Swept volumes as fields" |
| Closed-form distances as a geom-intersect module reached through a trait the facade implements, or geom-spatial depending on geom-intersect | 19 | 32.13: geom-spatial on geom-intersect; piece "Analytic closed-form distances" |
| The drawing checker in geom-draw (chosen), or one checker crate moved above geom-draw | 18 | Piece "Drawing checker" |
| Continuous collision detection lives in geom-spatial | 19 | Piece "Time of first contact (Tight Inclusion method)" |
| Sheet metal reuses geom-panel's unroll, or unrolling moves to a shared crate; subdivision displacement belongs to a content-creation toolkit | 16 | Decision note under 26.1 when a second program needs it |
| Add geometry/PROVENANCE.md, NOTICE, methods/ and PATENTS.md to 26.3's file table and 27's folder tree | 29 | Editorial fix; piece "Provenance records" |
| Chapter 27's table and the scaffold's `table.rs` (G09) are two copies of the crate graph that nothing compares | G09 build | Piece "MAP.json generator and module-header grammar", or the merge gate |
| The header grammar: G09's interim grammar (job, layer, invariants, sections), its Status and "dependencies still to add" lines, which name board pieces and go stale when a piece splits (30.7), and which chapters a Sections line lists. `geom-num` has no Layer line, which G09's table records as a known gap. G11's development crates state their job but no layer, invariants or sections in that grammar, so the scaffold check fails on them when G09 and G11 have both merged | G09 and G11 builds; 26, 27 | Piece "MAP.json generator and module-header grammar" |
| Content hashes for reuse below `geom-format`: 23.8 puts hashing, `CacheKey` and the memo store in `geom-format` (27, accepted in 32.13), but `geom-bool`'s arrangement cache (11.6), `geom-draw`'s per-element caches (18.9) and `geom-spatial`'s instances (19.11, 19.12) key by content hash and sit below it. Either the caller passes the hash in as a key, or the hash of canonical bytes moves into `geom-op`'s canonical encoder, as NameId hashing already does (30.6) | 11, 18, 19, 23; G08 | Piece "Canonical encoder and safe hash" (G86) |
| One lint posture: which crates carry `geom-num`'s `[lints]` table and `clippy.toml` (G09's check says the crates that ship; G11's test holds the development crates to it word for word); whether `float_cmp = "allow"` and its reason hold everywhere; and which `clippy.toml` bans the binding crates carry | G09 and G11 builds | 32.13: every crate that ships, development crates by G11's test, and the binding crates with `unsafe_code = "deny"`; `float_cmp`, piece "Merge gate: licences, lints and builds" |

### Chapter 28: Testing

| Question | Raised in | Decided in |
| --- | --- | --- |
| Can Kani prove whole 384-bit predicates in reasonable time? If not: per-step proofs, bit-range certificates, extreme values and fuzzing. Piece G42 took the per-step route for the limb variant: 43 proofs, one per kind of limb step, each on every operand its certified bits allow, with the checker, extreme values and a differential fuzz target against big integers. A whole 384-bit predicate was not tried (orient2d's total-path proof already takes about 18 minutes) | 23 | Pieces "Wide-integer variants and WebAssembly benchmark" and "Kani harnesses: foundation" |
| Thresholds: 100% of lines and branches and zero unexplained mutants in the foundation set; 90% elsewhere; the 5% gates; the 1,000-times loose-bound report; the scoreboard rule | 28 | Kernel owners; piece "Mutation and coverage gates" |
| Wall clock: a nightly target miss blocks further merges until it is fixed (proposed), because per-pull-request wall clock varies by more than 5%; work units stay out of the speed gate | 23 | Pieces "Benchmark harness and merge gates (geom-bench)" and "Wall-clock and scaling gates" |
| SMLib's limits as an oracle: booleans, fillets and offsets cannot be automated (they are nondeterministic), and the NLib routines solidcore lacks need new FFI shims and their own audit. Its Numerical Recipes routines are stubbed out, as owner action G157 decided (32.23) | 29 | Piece "Oracles: SMLib NLib and geomdl" |
| Revit's tolerance values (VertexTolerance, ShortCurveTolerance, AngleTolerance), and whether its triangulation states an error bound | 28 | Piece "Revit truth-test harness" |
| One call's stack budget on WebAssembly, which has no guard page: the predicate tests allow 32 KiB per call, a thirty-second of the default 1 MiB, and `ExactSum` puts 41 KB on the stack for one call (23.4). Set the stack the recursion check tests on, and each call's share | G02 and G25 builds; 19, 23 | Piece "Recursion check" |
| Whether the full unit and property suites must also run in debug on wasm32, where they take over an hour, and at which tier; `target-cpu=native` has no wasm32 meaning | G02 build | Piece "CI matrix and check tiers" |
| A minimum supported Rust version: `geom-num` declares 1.86, which no CI leg builds, while the pin is 1.99.0. Promise one and test it, or drop the field | G02 and G32 builds | Piece "Toolchain and dependency guard" |
| `geom-bench`'s fuel host is a second workspace with its own `Cargo.lock`, outside the toolchain guard and chapter 27's map | G06 build | Piece "Toolchain and dependency guard" |
| Keeping the commit pins of third-party CI actions current (Dependabot for actions, or another rule), and pinning `kernel-bench.yml`'s actions, which G06 names by tag or branch | G02 and G06 builds | Piece "Toolchain and dependency guard" |
| "The kernel has no randomness" (28.17) and "never lets a seed decide a replayable result" (28.19), against seeded methods whose seed is an operation input (16.2, 16.3, 16.4, 17.10). G32 reads both as forbidding hidden seeds, and its rule stands on that reading until this is decided | G32 build; 16, 17 | Kernel owners; editorial fix to 28.17 and 28.19 |

### Chapter 29: Sources and licences

| Question | Raised in | Decided in |
| --- | --- | --- |
| hypermesh's history (forked from MPL-2.0 boolmesh, relicensed MIT under a notice), before any reuse | 11, 29 | After the 32.9 contact; ledger row |
| What "vendored" should mean for `libm`, which G24's done-when asks for, when the merge gate allows it only from crates.io and refuses a copy patched in (`deny-no-std.toml`): A, vendor the whole crates.io graph by source replacement; B, port the files the kernel reaches into `geom-num`, making `libm` a PORT (changes 29.1's decision); C, keep the DEPEND and take the audit pinned to the release's checksum as the vendored record. G24's audit recommends C (`projects/kit/notes/G24-libm-audit.md`, section 9) | 29, G24 | Owner decision; piece "NaN-free scalar, total order and deterministic maths module" |
| List Lifted Surfacing's Adobe Research Licence dependencies (mtetcol, space-time-functions, arrangement-benchmark, mtet) and libigl's GPL CGAL module as never read; VolumeRemesher (LGPL by repository) and GMP as method only | 13 | Ledger rows in 29 |
| TiGL is built on OpenCascade types, so its Gordon method is re-derived, not translated; Parasolid concepts read through a mirror are used as concepts only | 13 | Ledger rows; piece "Network surfaces, Coons patches and N-sided fills" |
| Check every Clothoids and G1fitting file header for third-party provenance before any port (Fresnel.cc derives from a book) | 17 | Piece "Spiral evaluation with certified bounds" |
| EPSG's terms of use and the LINZ grid licences (believed to be CC BY 4.0) | 17 | Before PROJ data ships (kit-ops) |
| Standards and format terms: Gerber section 1.4 (confirm the quotes from the PDF), job files, XNC and Excellon, IPC-2581C, ODB++, IDX, IDF 3.0; IGES 5.3; USD (TOST and AOUSD terms), Alembic, FBX; Parasolid XT, ACIS, JT; OpenCascade .brep (NVIDIA's reader mirrors OpenCascade's LGPL one) | 24, 29 | Each format's piece waits for its check |
| ISO's EXPRESS schemas: where they may be obtained, what may be generated from them, and which edition of ISO 10303-42 is current | 24, 29 | 32.39 (owner action G409): the 2025 edition; generation only with a file's own grant or ISO's permission; piece "EXPRESS code generation and schema validator (development tool)" |
| Thread and gear tables are copyrighted standards, so geom-thread takes them as supplied or licensed data | 13 | Piece "Helical sweeps and geom-thread" |
| How entries in our own pattern and linetype library are authored | 14 | 32.24 (owner action G184); piece "2D-22 Our own pattern and linetype library" builds it |
| SMLib's .smp and .smb fixtures: the stricter reading applies, so they are not copied and the configurations are rebuilt | 12 | Answered; ledger row |
| Unverified licence facts: EISPACK, the LAPACK text, ShapeOp, NIST SFA and MBE PMI, BRL-CAD's LGPL version, linesweeper, laz-rs's history, Geogram's other third_party folders, OpenMesh, the 3MF and glTF terms, Lacewell-Burley and Biermann-Levin-Zorin reference code, and the Wang et al. 2021 collision dataset (checked: no licence, waits for the authors, 32.31) | 16, 19, 29 | Before each source is used; pieces "Licence and dependency gate for the kernel workspace" and "Dependency audit" |
| The JTS conformance files are no longer unverified: chapter 14 settles them as EDL-1.0, reusable with their notice | 14, 29 | Editorial fix to chapter 29's list of unverified licence facts |
| Sample data licences: KiCad boards and Gerber reference files, ezdxf examples, PDAL and las-rs clouds, rhino3dm files, LINZ LandXML examples, the buildingSMART alignment set (fetch only), and the LandXML 1.2 schema (which cannot be pinned) | 17, 28 | Piece "Test-corpus register and fetcher"; fetch-only until checked |
| Copyright in largely AI-generated code (solidcore, CADCraft, csgrs) matters only if one of them is promoted to PORT | 29 | Risk R20 |

### Across chapters

| Question | Raised in | Decided in |
| --- | --- | --- |
| Chapter lines that say an item is "raised in section 32" or "pending decision (section 32)" now point at the wrong place for most items. Each should name 32.n for the owner's decisions and 31.7 for everything else: geom-2d's dependencies, geom-bvh, the step ladder, the threaded browser build, the nightly wall-clock rule, the kernel shares, the two topology amendments, epsilon_step and the size rungs, the sketch constants and reference shape, and the crate amendments | 8, 13, 14, 15, 19, 23, 27 | Editorial fix to those chapter lines |
| Chapters 14 (about 6,500 words), 16 (7,000), 19 (9,000) and 24 (8,500) run long. Candidates to split out: 14.12 and 14.14; 16.4 and 16.5; 19.7 and 19.8; 24.11's tables into crate design notes. Chapter 28 keeps its full oracle and corpus tables | 14, 16, 19, 24, 28 | Kernel owners at acceptance, as an editorial choice |
