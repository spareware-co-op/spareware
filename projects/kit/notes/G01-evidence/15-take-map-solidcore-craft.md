# 15 What we take: solidcore and the Craft apps, piece by piece

*Evidence for [G01](../G01-geometry-kernel.md). Written 2026-11-05 by a Claude Opus 5.5 research agent that read the outside repositories through GitHub's API and raw files. Nothing was cloned, built, run or tested. It follows [11](11-vetting-suggested-projects.md), [12](12-vetting-cadcraft.md) and [13](13-vetting-smlib.md) and its addendum, and decision [32.44](../../../engines/geometry/architecture/32-decisions.md). Licences are settled by the owner and are not discussed here.*

## 1. What this note is

For each board piece in [30-build-order](../../../engines/geometry/architecture/30-build-order.json) where an outside unit can help, one entry says what to take, what we must re-derive, what we must not take, and whether the piece can start now. Part A covers the kernel. Part B covers the kit and Spare Construct. Part C ranks the ten best takes.

**Our architecture stays the spine.** The earlier recheck found that solidcore's topology is tolerance-based (inherited from SMLib), that "green" there means within 1e-9 relative of the C++, and that its results rest on the platform's `sin` and `cos`. Its [certified.rs](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/geom/certified.rs) says so in its header: Higham bounds, with `cos`, `sin` and `atan2` taken as within one ulp. A GitHub code search of the solidcore repository for `orient2d`, `BigInt` and `robust predicate` returns no hits (run 2026-11-05 against the default branch, which has moved on from b4de2149). So every take below is a design, a case list or test data. Arithmetic is always re-derived under our exact predicates, certified bounds, bit-identical results (wasm32 included), kernel-level naming and the [chapter 28](../../../engines/geometry/architecture/28-testing.md) gates.

### Pins

| Project | Commit read | Note |
| --- | --- | --- |
| solidcore | [b4de2149](https://github.com/tangency-labs/solidcore/tree/b4de2149aadbd737268091c51d8390a7d3cdda70) | The head named in the brief. The default branch was already at f06f897e when I looked (2026-11-05 08:06 UTC), so some paths may have moved. |
| SolidCraft | [6029a38e](https://github.com/tangency-labs/solidcraft/tree/6029a38ea05da8f37517da6b06624a8bec18adf5) | Head on 2026-11-05. |
| CADCraft | [c4d9e6ac](https://github.com/storytold/cadcraft/tree/c4d9e6ac64bea4bb54b23fcbf32c21dc997f279c) | Head on 2026-11-05. |
| BIMCraft | [238442f5](https://github.com/IPedrax/BIMCraft/tree/238442f5c448b28c5c90f9cdfedc105fd327f9eb) | Head on 2026-11-05. |

### Status key

Status was read from the work queue and `gh pr list` on 2026-11-05, and from main at 87568874.
- **main**: merged.
- **PR #n**: open pull request, not merged.
- **branch**: built and tested on a piece branch, no pull request yet (from the work queue).
- **not started**: no branch and no pull request.

Two owner actions in this note's scope are already decided and on main: G157 ([32.23](../../../engines/geometry/architecture/32-decisions.md), relaxed by 32.44) and G239 ([32.28](../../../engines/geometry/architecture/32-decisions.md), PR [#337](https://github.com/spareware-co-op/spareware/pull/337)). G409 is on main too (PR [#359](https://github.com/spareware-co-op/spareware/pull/359)).

### What shapes every entry

**Most Phase 7 pieces cannot start yet.** G254 (frame rules and laws), G253 (surface extension), G262 (blend feasibility) and G275 (flat-body offset) all depend on a gate: G156 (gate 4, exact core) or G233 (gate 6, 2D and sketches). Neither has started. Where an entry says "not now", this is why. What can start now is test data, case lists and specifications.

### Not confirmed

- I did not run solidcore, its corpora, difftest or any outside test. Pass counts come from the outside projects' own pages and the earlier rechecks.
- I did not read every file I link. Where a link is to a file I only read the header of, the entry says "header read".
- The defect-log counts by area (section 2) come from file names only, matched by keyword, so they are approximate. Which entries a C++ run confirmed is in each file and in the project's [defect page](https://tangency-labs.github.io/solidcore/issues.html). I did not count them again.
- Line ranges in Part B came from a delegated read. I spot-checked five of them (CADCraft `offset.rs`, BIMCraft `engine.rs` and `ids.rs`, SolidCraft `name_lookup.rs` and `oracle.rs`) and all matched.
- Nothing here says whether an upstream maintainer would accept our gates, or whether our work would flow back to them.

## Part A: the kernel

### 2. The defect log and the attack tests (board: the geom-test attack entries of every piece)

**Outside.** The [`plan/upstream-issues/`](https://github.com/tangency-labs/solidcore/tree/b4de2149/plan/upstream-issues) folder at b4de2149 holds 1,102 files. By file-name prefix: 694 `WR` (wrong result), 307 `UB` (undefined behaviour), 65 `HANG`, 21 `ND` (nondeterminism), 5 `DOC`, and a few others. Some files are addenda to an earlier entry. [Sample entry with a change and its test names](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/WR-254--addendum-2026-10-31-analytic-corner-failure-false.md). The site's own count is [1,050 entries](https://tangency-labs.github.io/solidcore/issues.html), 351 confirmed by a C++ run (earlier recheck).

**Take.** The defect, the input that triggers it, and the stated right answer. Rebuild each input from its description in our own API. Assert the exact answer where our API is exact, or a typed refusal. Never copy a fixture.

**Already done.** PR [#567](https://github.com/spareware-co-op/spareware/pull/567) (batch 1): 38 tests in 7 crates cover 45 confirmed defects. All pass, so none hit our kernel. It skips topology, booleans, blends, fillets, healing, sweep topology, file reading, and defects that need closest-point or projection.

**Next batches, by piece.** Each batch waits only for its piece to have code, not for the piece to merge.

| Batch | Pieces it tests | Example defects | Waits for |
| --- | --- | --- | --- |
| 2 | G162 projection and inversion | [WR-28](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/WR-28--smprojectedcurve-makeorthonormal-is-inverted.md), [UB-169](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/UB-169--addendum-2026-11-01-sm-api-projection-overflowing-normal.md), [HANG-32](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/HANG-32--smbrep-createparallelprojectioncurves-spins-on-a-subnormal.md), plus the ones #567 skipped (WR-740, WR-444, WR-72, HANG-20, HANG-50) | PR [#465](https://github.com/spareware-co-op/spareware/pull/465) |
| 2b | G44 to G69 again, the entries #567 left because no C++ run confirmed them (WR-24, WR-8, WR-34 and others) | as listed in #567's body | nothing: pieces on main |
| 3 | G163 to G176 intersections | the file names with `intersect` or `ssi`, about 36 by keyword | PRs [#492](https://github.com/spareware-co-op/spareware/pull/492) to [#527](https://github.com/spareware-co-op/spareware/pull/527) |
| 4 | G146 to G153 meshes (point classification, hulls, envelope) | ray, classification and distance entries, about 18 by keyword | PRs [#459](https://github.com/spareware-co-op/spareware/pull/459) to [#483](https://github.com/spareware-co-op/spareware/pull/483) |
| 5 | G127 to G144 flat booleans, topology, local-operation driver | topology and boolean entries, about 115 by keyword | branches and PR [#540](https://github.com/spareware-co-op/spareware/pull/540) |
| 6 | G247 mass | UB-49, UB-71, UB-102, UB-103, UB-155 (empty arrays, null dereferences in the volume classes) | G247's code |
| 7 | G255 to G259 sweeps and lofts | [HANG-13](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/HANG-13--smapicreatelinearsweep-of-a-full-circle-over-a-negative.md) (full-circle sweep over a negative distance), [HANG-43](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/HANG-43--smapicreatecurvesweepfromfaces-does-not-return-for-a-path.md), [HANG-11](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/HANG-11--smcone-adjuststepuvdomain-never-returns-for-a-reversed-sweep.md) | G256 |
| 8 | G262 to G269 blends | [WR-643](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/WR-643--fillet-run-on-behind-the-corner-takes-the-mirror-ball.md), [WR-254](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/WR-254--addendum-2026-10-31-analytic-corner-failure-false.md), [HANG-39](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/HANG-39--smbrep-createcornerblend-takes-tens-of-seconds-or-more-with.md), about 89 by keyword | G262 |
| 9 | G270 to G277 offsets and shells | [HANG-53](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/HANG-53--offset-member-distance-1e-300-subdivides-for-tens-of-seconds.md) (offset by 1e-300), [HANG-26](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/HANG-26--the-offset-self-intersection-of-a-surface-of-revolution-s.md), [HANG-35](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/HANG-35--closed-shells-a-few-zone-tolerances-across-take-minutes-to.md), about 80 by keyword | G270 |
| 10 | G419 and G420 healing | [HANG-24](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/HANG-24--smstitch-stitchintosolid-never-returns-from-a-zero-b-rep.md), [HANG-30](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/HANG-30--smapistitch-smbrep-never-returns-on-a-sheet-with-a.md), [HANG-33](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/HANG-33--smstitch-stitchintoshell-never-returns-when-dmaxstitchtol3d.md), [ND-1](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/upstream-issues/ND-1--addendum-2026-11-01-stitch-closure-split-confirmed-on-bull.md) | G419 |
| 11 | G415 and G427 file reading | `UB-1` (old ASCII `.smb` files) and the reader entries, about 47 by keyword | G415 |

The nondeterminism entries ([ND-1, ND-3, ND-491, ND-493](https://github.com/tangency-labs/solidcore/tree/b4de2149/plan/upstream-issues)) matter most to us, since bit-identical results are a goal solidcore does not claim. They are the best candidates for the cross-platform replay gates of chapter 28.

**Do not take.** Their fix flags (`UpstreamFixes`), which switch between upstream and fixed behaviour. We have one behaviour.

**Can start now:** batches 2b at once; the others as their piece lands. Needs only a reading of each file and a test file per crate.

### 3. Offsets and 2D fillets (G62, G66, G203, G205)

**G62 (offset curves) and G66 (offset surface): on main.**
- *Outside:* [`curve/offset_curve.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/curve/offset_curve.rs) (header read; a port of SMLib's offset curve), [`surface/offset_surface.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/surface/offset_surface.rs) (175 KB, not read), commit [c1497cca](https://github.com/tangency-labs/solidcore/commit/c1497cca) (an offset below the B-rep tolerance is refused, with its test [`offset_negative_approximation_tolerance.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/tests/offset_negative_approximation_tolerance.rs)), and the offset-surface third-derivative defects WR-160 to WR-162, already in PR #567.
- *Take:* the c1497cca case (a distance below tolerance) as an attack test of our G66; the cusp and small-surface hang entries HANG-21 and HANG-26 (batch 9).
- *Re-derive:* nothing; both pieces are done.
- *Not take:* their offset caches. Ours are certified by interval bounds already (G66's done-when).
- *Start now:* yes, as attack tests only.

**G203 2D offsets: not started (deps: G193 PR [#547](https://github.com/spareware-co-op/spareware/pull/547); G197 and G202 not started).**
- *Outside:* solidcore has no 2D polygon offset with joins that I found; its `offset_curve.rs` offsets a curve in space. CADCraft has one: [`offset.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/engine/src/cmd/modify/offset.rs) (526 lines). Lines, circles and arcs are exact; ellipses and splines are tessellated; polylines offset each segment and join neighbours at the nearest intersection of the extended segments (`offset_polyline` from line 139, `join_point` from line 189). The only join style is extend-to-intersect. Its own [parity table](https://github.com/storytold/cadcraft/blob/c4d9e6ac/docs/geometry-parity.md) says loops are not cleaned; I found no cleanup code either.
- *Take:* the edge cases from CADCraft's tests as inputs: [`offset_move.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/engine/tests/it/offset_move.rs), [`offset_ellipse_side.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/engine/tests/it/offset_ellipse_side.rs), [`offset_closed_spline.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/engine/tests/it/offset_closed_spline.rs), [`trim_closed_polyline.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/engine/tests/it/trim_closed_polyline.rs). The done-when already names the cavalier_contours tests; these are a second source.
- *Re-derive:* everything geometric: rational-normal raw offsets, four join and three end styles, exact winding cleanup, certified procedural offsets.
- *Not take:* extend-to-intersect as the only join; tessellating ellipses and splines; one absolute `EPS = 1e-9` ([geom/lib.rs](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/geom/src/lib.rs)).
- *Start now:* no. Needs G193, G197 and G202.

**G205 fillet, chamfer and blend curves: not started (deps: G193 PR #547; G204 not started).**
- *Outside:* CADCraft [`geom/fillet.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/geom/src/fillet.rs) (412 lines): circles tangent to two curves by intersecting sampled offset curves, then Newton with a finite-difference Jacobian. [`modify2/fillet_pline.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/engine/src/cmd/modify2/fillet_pline.rs) (240 lines): polyline fillet and chamfer with a bulge for the arc corner, and distance and angle chamfers. Test files [`fillet_chamfer.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/engine/tests/it/fillet_chamfer.rs) and `extend_polyline_arc_end.rs`. From solidcore, the contract in [generic-blend section 3.5](https://github.com/tangency-labs/solidcore/blob/b4de2149/docs/generic-blend.md): every refusal returns the largest size that builds, computed from the cause's own geometry, never by trying again.
- *Take:* the refusal contract for `RadiusTooLarge` (the done-when already wants the largest fitting radius); the list of operation modes (polyline mode, distance and angle chamfer) as the surface of the API; the test inputs.
- *Re-derive:* the construction. Ours uses exact tangent constructions on exact fractions (G193, decision 32.2 option A).
- *Not take:* sampled offsets plus Newton with a numeric Jacobian.
- *Start now:* no. Needs G193 and G204.

### 4. SMLib as an oracle (G157, G161) and the second reference (G113)

**What is already done.** G157 is on main: 32.23 says yes to running SMLib's reference build as an out-of-process oracle, limited to NLib's evaluation, knot insertion and removal, and degree elevation, built from pinned source in the oracle repository's own container and never authoritative. 32.44 then lifted the Numerical Recipes stubbing, so those routines may now be compiled. G161's kernel side is built on branch `piece/G161` (work queue: jobs, judging and exact settling; harness `smlib-nlib` in the oracle repository; first run passed, twice, identical). It has no pull request yet. Its done-when ("SMLib's 100% classes") is superseded by 32.23, as the queue records.

**Take for G161, now that 32.44 holds.** The stubbed classes were LU, SVD, the Brent minimiser and the integrator ([32.6](../../../engines/geometry/architecture/32-decisions.md)). They can now join the oracle runs, one class at a time, under 32.23's own rule that a class is used only while repeated runs agree bit for bit. Candidates, not decisions: linear solves, and the integrator that backs mass properties ([`mass/integrator.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/mass/integrator.rs), an adaptive Cash-Karp integrator). The "never counts" list in 32.23 (fitting, booleans, fillets, offsets, validity, the intersector) is about determinism and does not change.

**Take for G113 (oracle runner and triage), from solidcore's [difftest](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/difftest/README.md).** It runs one unit through the C++ and the Rust port on the same inputs. Ideas worth copying into our runner, all from its README:
- `--perturb`: change one item of each case's Rust output and pass only if every perturbed case is caught. It tests the comparator itself.
- `--heap-pads`: rerun the C++ side with a padded heap and list cases whose output changes, which finds address-dependent output. It is a flag, not a gate.
- `--fuzz N`: probe a unit's guard with N mutated cases, each in a child process.
- A regression ratchet against the base revision's report, with a [waiver file](https://github.com/tangency-labs/solidcore/blob/b4de2149/plan/difftest-waivers.toml) where every entry needs a reason a reviewer can check.

**Re-derive.** Our harness stays in the separate oracle repository and exchanges only files (28.10, 32.6). We do not link or vendor difftest. Its units are SMLib-versus-port; ours are our kernel versus a reference.

**Not take.** difftest's prebuilt C++ libraries (`scripts/fetch-cpp.sh`); 32.23 says the oracle builds from pinned source, never from a prebuilt reference. Its "green means within 1e-9 relative" rule; our comparisons are settled by exact or interval truth.

**Start now:** G113 is claimed by a session, so these are inputs to that session. The G161 extension waits for its branch to merge.

### 5. The Sweep kind decision (G239, chapter 13 question 5)

**It is decided.** [32.28](../../../engines/geometry/architecture/32-decisions.md) (2026-10-28, on main) says the Sweep kind stores its whole definition: the full `FrameRule`, twist and scale laws, bit-exact station frames for a rotation-minimising frame, and a tapered revolve's profile morph. The helix kind gains a radius rate. Chapter 13 still words the station-frame encoding as "open question 5" at [lines 115 and 206](../../../engines/geometry/architecture/13-sweeps-blends-offsets.md); that wording is stale.

**What solidcore does** (commits [36d9e16c](https://github.com/tangency-labs/solidcore/commit/36d9e16c) and [f144e061](https://github.com/tangency-labs/solidcore/commit/f144e061), issue #2572). A path sweep keeps a `ProceduralSweep` definition: [`S(w, t) = T(t) + x(t) C_x(w) + y(t) C_y(w) + z(t) C_z(w)`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/surface/procedural_sweep/path.rs), where the frame is a property of the path alone and does not depend on how the sweep sampled it. The frame is in closed form where every path span is a line (constant frame) or a circular arc (a turn about the arc's axis by the tangent's turn angle). The net's distance from the definition is bounded by a convex-combination argument: `|B - S| <= max_i max_v |P_i - S_i|`, each term bounded over cells by a Taylor expansion with a forward-error floor of `gamma_K` and a split budget (`MAX_CELLS`, 400,000). A path with a fitted-spline span "has no closed form; its certified enclosure is a follow-up", so its wall keeps no definition yet (same file, and [`path-spline.md`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/tests/data/sweep_definition_ledger/path-spline.md) in the sweep ledger).

**Comparison with 32.28.**
- Same principle: the stored definition is the shape and does not depend on sampling.
- Different mechanism: solidcore stores nothing but the closed form, so it has no answer for spline paths. We store station frames bit for bit, so we have one, and our closeness to the ideal frame is measured, not proven (13.22, last bullet).
- Solidcore classifies a span as a line or arc by sampling at 12 parameters within 1e-11 of the path's size. Ours decides by exact kind tests.

**Take.** Nothing that changes 32.28. Two additions for G254 and G256, both for the owner to accept or not:
1. For paths made only of exact lines and arcs, the frame has a closed form. That could be a proven bound for those paths, where chapter 13 says only "measured" (13.22). This is an addition to the decision, not a reversal.
2. The cell-and-Taylor bound with a rounding floor and an explicit split budget, as a template for the certified cache of a Sweep face. The arithmetic is theirs in `f64` with a `2^-30` slack on cell bounds; ours is rebuilt on our interval type.

**Not take.** Classification by sampled tolerance; "no definition for spline paths".

**Start now:** the note and the owner question, yes. The code waits for G254, which waits for G156 and G233.

### 6. Mass properties (G247)

**Status:** not started. Deps: G246 not started; G179 PR [#456](https://github.com/spareware-co-op/spareware/pull/456); G129 built on a branch.

**Outside.** [`mass/integrator/properties.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/mass/integrator/properties.rs) (header read) computes edge, face and B-rep properties by nested adaptive Runge-Kutta integration ([`integrator.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/mass/integrator.rs)). It has no interval enclosure. It gives a "no material" area and an `estimate_accuracy`, neither certified. SolidCraft's [oracle](https://github.com/tangency-labs/solidcraft/blob/6029a38e/docs/oracle.md): 90 cases, 69 analytic with formulas in [`tools/oracle-analytic/cases_*.py`](https://github.com/tangency-labs/solidcraft/tree/6029a38e/tools/oracle-analytic), 21 from FreeCAD. Each case's [`expected.json`](https://github.com/tangency-labs/solidcraft/tree/6029a38e/tests/oracle) holds volume, area, centre of mass, bounding box and the method. Also `mass_properties_tests.rs` (8 tests).

**Take.**
- The 69 analytic cases as closed-form truth for G179's corpus (PR #456) and G247's "within bound of closed form" tests. We re-derive each value exactly or as an interval from the formula in the `method` string and never trust their stored decimals (their checker uses relative 1e-3 for volume and area, [`oracle.rs`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/apps/solidcraft-cli/src/oracle.rs) lines 12 to 16).
- A trap list from solidcore's tests. The integrand of a face jumps at knots where a surface is G2 but not C2, so a quadrature must split at knots ([`issue_2091_precise_volume_spans.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/tests/issue_2091_precise_volume_spans.rs), defects WR-494, WR-563 and WR-670, named in the `properties.rs` header). Also [`issue_383_tangent_torus_volume.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/tests/issue_383_tangent_torus_volume.rs).
- Where a B-rep has a void, a volume-separating face adds its volume only where it bounds a non-void region (WR-535, same header).

**Re-derive.** The quadrature: certified (interval remainder per span), exact rationals for flat bodies, and certified intervals for curved ones. Our unit of truth is a bound, not a tolerance.

**Not take.** The Cash-Karp integrator as an algorithm. It is adaptive with no certificate, and returns zero on failure by default (its header says so).

**Start now:** the extraction of the 69 analytic cases into G179's corpus, yes (G179 is built). The quadrature waits for G246.

### 7. Sweeps and lofts (G256 to G259)

**Status:** all not started. G256 needs G254 and G255 (not started) and G175 (PR #527). G257 needs G256. G258 needs G257 and G238 (main). G259 needs G254, G58 (main) and G176 (PR [#525](https://github.com/spareware-co-op/spareware/pull/525)).

**G256 general sweeps and swept disks.**
- *Outside:* [`sweep/topology_sweep/`](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore/src/sweep/topology_sweep) (engine 74 KB, store 115 KB, merge 54 KB; not read beyond sizes), [`sweep/directed.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/sweep/directed.rs), the ops wrapper [bf935920](https://github.com/tangency-labs/solidcore/commit/bf935920), 95 sweep cases in the [kernel-case corpus](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore/tests/kernel_cases) (counted by file name), its [`known_failing.json`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/tests/kernel_cases/known_failing.json), the [sweep definition ledger](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore/tests/data/sweep_definition_ledger) (path-closed, path-scaled, path-spline, path-translational, screw-cylinder), tests [`general_path_sweep.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/tests/general_path_sweep.rs) and `issue_1462_sweep_attributes.rs`. SolidCraft oracle cases 16, 49 and 50 (sweeps).
- *Take:* which sweeps can keep a definition (the ledger is the list); the sweep-topology cases; the attribute-through-sweep case (a naming test for 13.19).
- *Re-derive:* frame rules, mitres with the mirror test, the two self-overlap tests, self-contact (13.3). None of it is in solidcore in the form chapter 13 wants.
- *Not take:* the sampled line-or-arc test (section 5).
- *Start now:* no. Case list extraction only.

**G257 rail and helical sweeps.**
- *Outside:* [`sweep/screw.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/sweep/screw.rs) (a profile carried by a screw motion with an optional radial law, header read) and [`surface/procedural_sweep.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/surface/procedural_sweep.rs). The definition is `S(w, t) = o + R_k(theta)(P(w) - o + e delta(theta)) + k pitch theta / (2 pi)`. The net is a rational quadratic arc per span (weights `1, cos h, 1`), and the bound has a closed form per span: a lift error `E_l(x) = K x / (1 + x^2) - 2 atan x` whose largest value is at `x*^2 = (K - 2) / (K + 2)`, and an offset error found by interval branch and bound. `atan` is enclosed by its alternating series, not libm.
- *Take:* the derivation as the certified cache for a helicoid face (a radial law that is piecewise linear in the angle is a tapered thread). It matches 32.28's radius rate: our conical helix is Exact, and only the swept face carries a cache.
- *Re-derive:* in our interval type. Their enclosure of `atan` by series is a useful pattern, since we ban libm in decisions.
- *Not take:* "lifted within a tolerance" walls as the definition. Ours are Exact helices swept.
- *Start now:* a note, yes; code no.

**G258 geom-thread.** *Outside:* 31 thread cases in the kernel-case corpus (`threads`), tessellation data [`smlib_thread_in_bore`](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore-tess/tests/data) and `smlib_thread_run_out`. *Take:* the cases as inputs for run-outs and tapered threads. *Not take:* thread forms; the done-when ships none. *Start now:* no.

**G259 lofts and swept blends.**
- *Outside:* skinning in [`sweep/api_surfaces.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/sweep/api_surfaces.rs) (header not read), difftest [`issue_1036_skin_fixture.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/difftest/tests/issue_1036_skin_fixture.rs), SolidCraft [`loft_blend_tests.rs`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/crates/kernel/src/loft_blend_tests.rs) (7 tests) and oracle case 17, whose `expected.json` records where two kernels legitimately differ on a loft.
- *Take:* the skinning cases with coincident or pleated sides ([`issue_1058_pleated_skins_speck_face.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/tests/issue_1058_pleated_skins_speck_face.rs)); case 17's "legitimate difference" note as a documented-difference pattern for our oracle runs.
- *Re-derive:* correspondence rules, G1 and G2 ends, certified knot merging.
- *Start now:* no.

### 8. Blends (G262 to G269): solidcore's generic blend

**Status:** all not started. Deps: G262 needs G144 (PR #540), G171 (PR [#494](https://github.com/spareware-co-op/spareware/pull/494)), G68 (main), G156 and G233 (not started). G264 also needs G168 (PR [#473](https://github.com/spareware-co-op/spareware/pull/473)). G267 and G268 need G260 (not started).

**Outside, the whole.** The design page [docs/generic-blend.md](https://github.com/tangency-labs/solidcore/blob/b4de2149/docs/generic-blend.md) (154 KB) and the code in [`crates/solidcore/src/blend/`](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore/src/blend): `fillet_section/` (the section, its definition and its certificate), `fillet_exact` (the exact classes), `blend_setback/`, `blend_corner/`, `fillet_cliff_overflow.rs`, `band_room.rs`, `blend_refusal.rs`. Its one rule: every case is one construction, a spine, two rails and a section; a failure is fixed in that construction, never by a fallback. The kernel-case corpus has [265 fillet cases](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore/tests/kernel_cases) (counted by file name). Its open work (about 16 issues, #2343 to #2406, in section 6 of the page) tells us where it is still weak: several concave rounds at one vertex, four blends over three faces, chamfers meeting at a vertex, a G2 setback fill.

| Piece | Outside, exact place | Take | Re-derive |
| --- | --- | --- | --- |
| **G262 feasibility and preview** | [Section 3.3](https://github.com/tangency-labs/solidcore/blob/b4de2149/docs/generic-blend.md) (fold and room checks), [3.5](https://github.com/tangency-labs/solidcore/blob/b4de2149/docs/generic-blend.md) (refusals name the size that fits), `band_room.rs`, `blend_setback/convexity.rs`, ops `blend/room.rs` | The causes of refusal and a size for each: a polygon face taken whole, a collapsed offset (a ball as wide as the face it rolls on), a fold, no room. "When the bound is open, give the bound less 1,000 edge tolerances" is a concrete rule. The page's gallery of refusals. | Our bound is certified from interval curvature; theirs is closed form or solved per cause. Our chains and clusters (13.10 steps 1 to 4) have no counterpart. |
| **G263 analytic blends and chamfers** | [Section 4](https://github.com/tangency-labs/solidcore/blob/b4de2149/docs/generic-blend.md), table of exact classes (plane/plane, plane/cylinder, plane/cone at any angle, two cones, cone/sphere, a round on a round), with minimal bidegrees (4,2), (6,2), (8,2), (12,2) from Dahl and Krasauskas; [`fillet_exact.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/blend/fillet_exact.rs) with `rational.rs`; [`chamfer_side.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/blend/chamfer_side.rs) (a chamfer sized along either face) | The table of exact classes: it is longer than our 13.11 list, and it picks the case by classification, not a try-chain. The chamfer-side rule. | The choice by exact integer tests (13.10). The rational surfaces' coefficients under our exact types. |
| **G264 rolling-ball** | [Sections 2.1 to 2.4](https://github.com/tangency-labs/solidcore/blob/b4de2149/docs/generic-blend.md); [`fillet_section/certificate.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/blend/fillet_section/certificate.rs) (header read: Krawczyk's operator proves a unique root per span; Hermite remainder `h^4 / 384`), `interval.rs`, `tay.rs`; commit [3ef19b2a](https://github.com/tangency-labs/solidcore/commit/3ef19b2a) (a round on a round) | The certification method: unique-root proof over each span box, knots inserted until the bound is a tenth of the zone tolerance. It matches our "traced by the certified tracer" (13.10) and could back G170 and G264's tangency check. The round stored as its definition with a certified net. | Their stations stop Newton at `1e-12 (1 + abs x)` and bound the remainder as an added floor; ours must carry the exact-or-interval answer from the tracer (G168). Rounding bounds in our types. |
| **G265 overflows and clusters** | [Section 3.4](https://github.com/tangency-labs/solidcore/blob/b4de2149/docs/generic-blend.md) (overflow trimmed at the crease, in place), [`fillet_cliff_overflow.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/blend/fillet_cliff_overflow.rs), [`blend_overflow_first.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore-ops/tests/blend_overflow_first.rs) | The "keep the blend, move the sharp edge" policy as a compared option against our four-type ordered policy. The gallery cases. | The ordered policy (smooth, cliff, cliff-end, notch) and cluster transactions. |
| **G266 variable-radius** | [`fillet_executive/law_radii.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/blend/fillet_executive/law_radii.rs), tests `fillet_law_face_owned_drop_point.rs`, the page's "Variable rounds" gallery | The law cases; the "variable rounds as the union of their balls" step (step 3 of the page's section 6). | Radius laws in our law-curve kind. |
| **G267 Class-A Gn** | [`fillet_section/`](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore/src/blend/fillet_section) (`GnLaw`, `law_series`, a Hermite certificate per span), the "Gⁿ sections" gallery | A certified Gn section generator exists and is the closest prior art. Its cases are our benchmark inputs. | Hartmann sections with fullness (13.10) and G3 are ours to derive; I did not check whether theirs is Hartmann. |
| **G268, G269 vertex blends** | [Section 3.2](https://github.com/tangency-labs/solidcore/blob/b4de2149/docs/generic-blend.md); [`blend_setback.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/blend/blend_setback.rs) and [`blend_setback/certify.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/blend/blend_setback/certify.rs) (a bivariate Bernstein convexity certificate of a G1 fill); [`blend_corner/`](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore/src/blend/blend_corner) (ball, chamfer, end, mixed, setback, swept); [`fillet_corner/topology/`](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore/src/blend/fillet_corner/topology) (file names list the old classes: closed_nx1, concave_4x2, concave_nxn, end_corners, mixed_3x2, nx1_end, swept_turn, join, ball) | The corner events (Turn, Join, Ball, End, Setback) as an organising idea. The old class names as the generated-fixture matrix our G269 done-when asks for. The Bernstein convexity certificate is the single most reusable algorithm here for G1 setback patches. | Transfinite patches and sphere corners with exact G1 certificates. |

**Not take (blends as a whole).** The old fillet code is being deleted upstream, and its tolerance-based topology, `UpstreamFixes` switches and `try-chain` classifiers should not be ported. The page says the same.

**Chapter 13 is stale on one point.** It says SMLib's fixtures stay excluded. 32.44 lifted that: use them from upstream's pinned source at test time, never copied into our repository. G262's done-when ("no case judged feasible fails on SMLib's 98 fillet fixtures") can now be run that way.

**Start now:** none of the code. The specification work (G263's class table into chapter 13, G262's refusal contract) can start now as a chapter edit.

### 9. Offsets, shells and thicken (G270, G275, G277)

**Status:** not started. G270 needs G66 (main) and G253 (not started). G275 needs G274 and G144 (PR #540). G277 needs G275, G270 and G248.

- *Outside:* [`offset/`](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore/src/offset): `offset_executive/` (two 220 KB files and parts for loops, edge uses, cavities), [`offset_geometry_creation.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/offset/offset_geometry_creation.rs) (235 KB), [`wall_room.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/offset/wall_room.rs), [`draft.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/src/offset/draft.rs), the ops wrapper [`offset/`](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore-ops/src/offset) with its tests, [commit 5d1c9ab5](https://github.com/tangency-labs/solidcore/commit/5d1c9ab5) (extension past a side with certified margins). Tests: 54 files by name with `offset`, `shell` or `thicken` in [`crates/solidcore/tests/`](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore/tests), including `issue_1060_offset_mitred_*`, `issue_1795_shell_hole_through_arc.rs`, `hang_51_extrusion_old_offset.rs`; 36 shell cases in the kernel-case corpus; SolidCraft oracle cases 44 to 46 (shell) and its [`shell_regressions.rs`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/crates/engine/tests/shell_regressions.rs) (4 tests).
- *Take:* the shell and offset case lists and the mitred offset cases. G277's done-when names "SMLib's 25 shell fixtures": solidcore's 36 shell cases are a larger set, in the record format described below. Extension with certified margins (5d1c9ab5) as a case list for G253.
- *Re-derive:* the join logic. Our intersect join is the weighted 3D skeleton at time d (13.11, G274), not solidcore's per-edge executive. Round joins by re-intersection under certified intersection.
- *Not take:* the executive's tolerance-driven vertex and edge-use repair.
- *Start now:* no. A converter from their body records to ours is a separate step; I did not check how closely [`solidcore-record`](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore-record/src) matches our [G94 and G95](../../../engines/geometry/architecture/30-build-order.json) formats.

### 10. Mesh healing (G298)

**Status:** PR [#562](https://github.com/spareware-co-op/spareware/pull/562) (into piece/G148), deps PR #478 and PR #469 open, G156 and G181 not started.

- *Outside:* I found nothing. A search of solidcore's file list for weld, repair, hole and sliver finds only test data and unrelated B-rep tests, and BIMCraft's `triangulation.rs` was not read.
- *Take:* nothing.
- *Start now:* it is already built. No outside input.

### 11. B-rep healing (G418, G419, G420)

**Status:** not started. G418 needs G415 (open), G413 (not started), G177 (PR [#533](https://github.com/spareware-co-op/spareware/pull/533)), G175 (PR #527). G419 needs G418 and G171 (PR #494). G420 needs G419.

- *Outside:* [`heal/`](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/solidcore/src/heal) (`heal_data/` p1 to p3, `stitch/` p1 and p2, `gap.rs`, `face_props.rs`; all headers read are ports of SMLib's healer), difftest's [`heal_two_edge_sliver.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/difftest/tests/heal_two_edge_sliver.rs) and [fuzz regressions](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/difftest/tests/fuzz_regressions) (`heal.gap`, `heal.heal_data_p2`), and the STEP builder [`step-brep`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/step-brep/README.md).
- *Take, from `step-brep`'s README:* the order of operations. Build from the file's own topology first (shared edges and vertices as the file joins them), and stitch geometrically only if that fails. Leave out failing faces and report the first failing face, up to 16 faces or 1% of the body. Compare the two builds by volume at 1e-4 relative. It also gives a measured trade-off on an 87-file corpus: healing every body fixed more curved bodies (suspect 45 to 37) but raised timeouts (9 to 25), and healing only on tessellation failure gave the best mix. That is the shape of our "healed fraction per step only rises" baseline (G420), and a cost warning for G427.
- *Take, file pcurves:* [`pcurves.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/step-brep/src/pcurves.rs) accepts a file's pcurve only if it runs along the edge, lies within tolerance of the 3D edge, chains around the loop (shifted by whole periods on a periodic surface) and stays in the face's UV domain. This is the list of checks for G418's `TrimDisagreement`.
- *Re-derive:* every decision as certified or exact. Our healing never raises a tolerance (G419's done-when); SMLib's healer works by tolerance growth.
- *Not take:* the healer's tolerance-escalation logic, and the stitch tolerance parameter (`dMaxStitchTol3D`, see HANG-33).
- *Start now:* no.

### 12. STEP (G409, G427 to G429) and the Part 21 codec (G415)

**Status:** G409 on main. G415 open (deps G355, G382, G406 not started). G427 to G429 not started. G427 needs G418, G420, G416.

- *Outside:* [`step-import`](https://github.com/tangency-labs/solidcore/tree/b4de2149/crates/step-import/src) is "a clean-room Part 21 reader for the AP203/AP214 B-rep subset": [`part21.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/step-import/src/part21.rs) with untrusted-input `Limits` (4,000,000 entities, list depth 32, reference depth 64), [`map.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/step-import/src/map.rs) (88 KB, entity-to-geometry mapping), the 11 short-name aliases such as `FCSRF` for `FACE_SURFACE` in [`lib.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/step-import/src/lib.rs), [`validate.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/step-import/src/validate.rs). [`step-export`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/step-export/src/lib.rs) (80 KB): its header describes planar `MANIFOLD_SOLID_BREP` and `FACETED_BREP` with shared points and an AP203 product structure, and says unsupported geometry is rejected, not approximated. The earlier recheck says analytic and NURBS output is also written through [`brep_to_model.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/step-brep/src/brep_to_model.rs); I did not confirm which export paths are live. BIMCraft has a 408-line [`step.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/step.rs) Part 21 parser with hostile-input limits. SolidCraft has [`tests/step-conformance/`](https://github.com/tangency-labs/solidcraft/tree/6029a38e/tests/step-conformance) (18 files, generated from formulas, exact volume known) and a [`step-regressions`](https://github.com/tangency-labs/solidcraft/tree/6029a38e/tests/step-regressions) folder. solidcore's 87-file STEP corpus is not checked in ([step-brep README](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/step-brep/README.md)).
- *Take, G415:* the three limits as a starting table, and the short-name aliases. The hostile-input test pattern.
- *Take, G418:* `map.rs` as a cross-check list of which entities AP203 and AP214 files really use. Ours is generated from the EXPRESS schema (G416), so this is a coverage check, not a source.
- *Take, G427:* the conformance files and their exact-volume checks as an early test; the topology-first order (section 11).
- *Take, G429:* the shared-entity and welding rules of `step-export` as an output checklist; its refusal to write a non-watertight mesh as closed ("never claims the mesh is watertight").
- *Re-derive:* AP242, assemblies, tessellated geometry, validation properties, persistent IDs and PMI (G428, G429, G476). `step-import` does none of these ("B-rep subset"). Byte-identical round trips are ours.
- *Not take:* its Rust-side flat encoding for the C++ side of difftest.
- *Start now:* no. G415 needs G355, G382 and G406.

### 13. The law catalogue (G283) and the closed-form truth (G179, G252)

**Status:** G283 not started; deps G252, G277, G280, G282, G247 none started.
- *Outside:* SolidCraft's analytic oracle (section 6), [`docs/kernel-ladder.md`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/docs/kernel-ladder.md) (201 lines, 12 tiers from primitives to sheet metal, with 21 fillet and 15 shell and offset cases), and the [kernel-case corpus's `invariants`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/tests/kernel-cases/README.md): of 372 cases, 324 have analytic answers and 48 only invariants.
- *Take:* the ladder as an ordering for the law catalogue; the invariant-only cases as candidates for laws that need no closed form (volume of a sweep equals area times height, offset by d then -d).
- *Re-derive:* each law as an exact rational or a certified number (28.3). Their tolerance is relative 1e-3.
- *Not take:* "pass within 1e-3".
- *Start now:* the ladder and invariant list as a note for G283, yes. G283 itself waits for G277 and G247.

### 14. Failure catalogues as fixtures (G465) and the kernel-case corpus (G114)

**Outside.** The solidcore [kernel rig corpus](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/tests/kernel_cases/README.md): every case of SolidCraft's rig plus new ones, as data. Counts by area from file names: fillets 265, booleans 124, sweeps 95, sheet-metal 78, surfaces 62, shell 36, import 33, construction 32, threads 31, history 30, assemblies 26, with a 170-case ladder. An entry holds the traced operations, the operand bodies as records, and the expected result. Entries compare numbers exactly. [`known_failing.json`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore/tests/kernel_cases/known_failing.json) lists those that fail today. The runner is [`kernel_cases.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore-ops/tests/kernel_cases.rs) (38 KB). SolidCraft's [`docs/kernel-cases.md`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/docs/kernel-cases.md) records results; many cases fail, and these are real hard geometry.

**Take.** The case descriptions and the list of known failures as an outside failure catalogue for G465 (which depends on G252, G423, G427, G444, none started). The bodies are in solidcore's record format; converting them to ours needs a converter, and I did not scope it. The `history` cases (30) are naming cases and feed G141 (section 15).

**Re-derive.** Every expected value, from exact truth, not from a trace of their kernel.

**Not take.** Their expected numbers where they come from their own kernel's output.

**Start now:** the list of known failures and areas, yes, as a note. Data conversion waits for a decision on the record format.

## Part B: the kit and Spare Construct

### 15. Persistent naming from kernel history

*Feeds:* [chapter 20](../../../engines/geometry/architecture/20-naming-history.md) (20.4 how a reference resolves), G141 (naming and reference fixtures: built on a branch, 13 fixtures pass), G88 and G155; and Spare Construct's references in [A02](../../../spare-construct/notes/A02-entity-store.md).

- *Outside:* SolidCraft's [naming guide](https://github.com/tangency-labs/solidcraft/blob/6029a38e/docs/book/src/dev/naming.md) (163 lines), [`lineage.rs`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/crates/doc/src/lineage.rs) (776 lines, composes histories across the operations of one feature), [`history_names.rs`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/crates/doc/src/history_names.rs) (602 lines; pieces of one split face are told apart by the faces that bound them, for example `F2:top#[F2:side:l4,F4:side:l4]`, with numbers only where neighbours do not separate them), [`name_lookup.rs`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/crates/doc/src/name_lookup.rs) (549 lines; the rule that a split, a merge, a name on several bodies or a pre-format-14 name finds nothing and the feature asks for a reselect), and [`naming_history_tests.rs`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/crates/engine/src/naming_history_tests.rs) (847 lines, 23 tests). solidcore's side: the provenance hook in [`provenance.rs`](https://github.com/tangency-labs/solidcore/blob/b4de2149/crates/solidcore-ops/src/provenance.rs), and 30 `history` kernel cases.
- *Take:* the 23 scenarios (an upstream cut splitting a face, a blend following a moved lid, a merged face asking for reselect, mirrored twins, geometry fallback, a sketch on a replaced face) as G141's next batch of fixtures, written in our terms. The refuse-to-guess rule is already our principle (chapter 20); their scenario list is evidence of what breaks. The "found by geometry, not exact history" report is a good typed-result design for an `Ambiguous` or `Unverified` answer.
- *Re-derive:* ours are kernel-level `NameId` and `TopoName` with roles (G87), not strings. A name that survives is a kernel guarantee and is replayed bit for bit.
- *Not take:* geometric re-identification as a fallback (ours has none), string names, the legacy-format mark.
- *Start now:* writing the fixture list, yes. Adding the fixtures needs G132 (boolean names) merged.

### 16. The command and MCP layer

*Feeds:* Spare Construct [ARCHITECTURE.md, "Headless API, AI and add-ins"](../../../spare-construct/ARCHITECTURE.md) and the `construct-api` crate (which has a `schema.rs`). That document lists an `mcp` crate; no such crate exists in `projects/spare-construct/crates` yet. I propose a Construct note on the command layer, not yet on the board.

- *Outside, SolidCraft:* [`crates/engine/src/schema/mod.rs`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/crates/engine/src/schema/mod.rs) (typed JSON-Schema contracts for each command; check levels off, warn and strict), [`crates/mcp/src/server.rs`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/crates/mcp/src/server.rs) lines 33 to 59 (the agent instructions: plan numbers first, batch all-or-nothing, name features, verify with `measure`), the `expect` argument on `execute` and `batch` (flags an error if the volume or box differs; documented in [docs/mcp.md](https://github.com/tangency-labs/solidcraft/blob/6029a38e/docs/mcp.md)), and [`skills/solidcraft-commands/SKILL.md`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/skills/solidcraft-commands/SKILL.md) (how to add a command).
- *Outside, BIMCraft:* [`engine.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/engine.rs): `Session::execute` wraps each command in `catch_unwind` and rolls back on error or panic, a read-only mode for agents, atomic scripts, grouped transactions. The test that sweeps every command with hostile parameters: [`tests.rs` line 1893](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/tests.rs). Its MCP server is hand-written JSON-RPC over stdio ([`apps/bimcraft-cli/src/mcp.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/apps/bimcraft-cli/src/mcp.rs), 668 lines, 13 tests) with a `--connect` bridge to the running window by localhost port and token.
- *Outside, CADCraft:* [`CommandSpec`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/engine/src/cmd.rs) and its [MCP tools](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/mcp/src/tools.rs) (small surface: `command_line`, `script`, `execute`, `query_entities`, `render`).
- *Take:* the `expect` verification argument; the hostile-parameter sweep as a Construct test pattern; the agent instruction text as a starting outline; the read-only and atomic-batch modes.
- *Re-derive:* our command schema is generated from Rust types ([ARCHITECTURE.md line 618](../../../spare-construct/ARCHITECTURE.md)) and our commands replay deterministically from an operation log (A05); theirs return changed counts by category.
- *Not take:* BIMCraft's 12,777-line single registry file; JSON-in, JSON-out functions with no schema; undo as snapshots (section 18).
- *Start now:* yes, as a note. No code depends on it.

### 17. CADCraft's DXF types and drawing database

*Feeds:* G430 to G433 (DXF read and write, not started: G430 needs G210, G413, G213), G443 (oracles: ezdxf and LibreCAD), G214 (2D corpus), and Construct [A02](../../../spare-construct/notes/A02-entity-store.md) and [A06](../../../spare-construct/notes/A06-project-file.md).

- *Outside:* the [drawing database](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/doc/src/lib.rs) (`Drawing`, lines 92 to 132: header, layers, linetypes, styles, blocks, model store, layouts, views, UCS, constraints, handle seed), [`entity.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/doc/src/entity.rs) (30 entity kinds, lines 65 to 97), the copy-on-write [`store.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/doc/src/store.rs) (chunks of 1,024 `Arc<Entity>` and 64 sharded handle maps, so a clone copies pointers and an edit copies one chunk and one shard), the DXF [tag reader and writer](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/dxf/src/lib.rs) (R2000 to R2018, ASCII and binary, tag-count limits), unknown data kept raw and written back ([`foreign.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/doc/src/foreign.rs)), and a model-level round-trip census, [`roundtrip.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/io/src/roundtrip.rs): draw, save, reopen and list every difference by category.
- *Take:* the entity-kind list as G431's coverage checklist; the round-trip census design for G440 (conformance harness); the keep-unknown-data-raw rule; the chunk and sharded-handle numbers as a data point for A02.
- *Re-derive:* DXF coordinates come in to the grid with a recorded rounding (G413); our DXF reader is built to our types, not theirs (f64, one absolute EPS).
- *Not take:* their hatch and linetype library ([32.24](../../../engines/geometry/architecture/32-decisions.md), evidence 12). Their stated DXF fidelity: the project's own [file-format parity table](https://github.com/storytold/cadcraft/blob/c4d9e6ac/docs/file-format-parity.md) puts it at about 30% of AutoCAD's formats, and the only DXF in the repository is one 48 KB example. So their round trips prove their own files, not third-party files. We need ezdxf (G443) as the outside check.
- *Start now:* no for G430 to G433. The census design can go into G440 now.

### 18. BIMCraft's element model, families, phases, design options, IFC, `.rvt` reader and undo

*Feeds:* Construct [ARCHITECTURE.md "Data model", "Families", "Interop"](../../../spare-construct/ARCHITECTURE.md), the open decisions "Phases and design options", "Revit import depth" and "Family recipe format" (A07), A02, A05, and kernel pieces G422, G423, G426 and G441.

| Topic | Outside (BIMCraft 238442f5) | Take | Re-derive or leave |
| --- | --- | --- | --- |
| **Element model and checks** | [`model.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/model.rs) (7,838 lines): an `Element` enum of about 60 variants; [`ids.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/ids.rs) (six-digit ids from 100000 and a `u128` GUID; references hold GUIDs, never ids; a chunked copy-on-write store with a revision journal); one `Model::check` after every command (about 220 error sites) with rollback | Rule list for "what must always hold" as input to a Construct model-check design; the references-hold-GUIDs-not-ids rule; the change journal (`changed_since`). | One enum for 60 variants and wall-specific checks do not fit a data-driven element model (ARCHITECTURE.md "Data model"). The recorded rules come from Revit behaviour, not our spec. |
| **Families** | [`family.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/family.rs) (2,595 lines): parameters, types, forms (profile swept on a plane, or revolved), voids, formulas with `if()`, connectors; [docs/areas/families.md](https://github.com/IPedrax/BIMCraft/blob/238442f5/docs/areas/families.md). Nested families and type catalogs are not built (its roadmap lists them). | A concrete example for A07 (family recipe format, JSON or our own language): the forms and formula set. | Our recipe is a constrained sketch plus solid steps on our kernel, not forms with f64 bounds. |
| **Phases and design options** | [`phasing.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/phasing.rs) (1,347 lines, 37 tests): phases, phase filters, overrides, option sets, status per view; [docs/areas/phasing.md](https://github.com/IPedrax/BIMCraft/blob/238442f5/docs/areas/phasing.md) | Test cases for the open decision "Phases and design options: day one". The 37 tests are the most thorough element-state tests in the repository. | Branches as options (ARCHITECTURE.md "Multi-user and history") is ours. |
| **IFC in and out** | Import: [`import.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/import.rs) (4,403 lines, 27 tests, IFC2x3 and IFC4; every element is measured from its geometry, then classified; unsupported items are left out and listed). Export: [`ifc.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/ifc.rs) (2,736 lines, 32 tests, IFC4 only, about 116 entity names; GlobalId is the element GUID). Validation: [`ci/validate_ifc.py`](https://github.com/IPedrax/BIMCraft/blob/238442f5/ci/validate_ifc.py) (IfcOpenShell schema and rule check, builds a shape for every product) and [`ci/ifc-coverage.txt`](https://github.com/IPedrax/BIMCraft/blob/238442f5/ci/ifc-coverage.txt) (one of every element kind, written to IFC in CI). | The CI recipe, as is, for G441 (IfcConvert and web-ifc oracles). The left-out-and-listed reporting as a model for G422 and G423 typed results. The hostile-IFC test. | Geometry from IFC is ours (G422 to G424). Their IFC4 export writes extrusions, swept disks and faceted breps, not our kinds. No real `.ifc` sample files are in the repository, only inline strings. |
| **The fenced `.rvt` reader** | [`crates/rvt`](https://github.com/IPedrax/BIMCraft/tree/238442f5/crates/rvt) (6,910 lines of source, 5,782 of tests, 196 tests; a separate crate that the core never depends on, taken by the apps through a default-on Cargo feature; in-process, not a separate process). It reads the compound-file container, gzip members, `BasicFileInfo`, the schema (4,418 to 4,690 classes per release), the element table, and element records; it decodes walls, floors, roofs, openings, columns, rooms, pipes and ducts. Every format finding has a row in [`docs/rvt-provenance.md`](https://github.com/IPedrax/BIMCraft/blob/238442f5/docs/rvt-provenance.md) (387 lines), with a table of sample files, their sizes and SHA-256 sums, and measured left-out counts (for example 1,791 elements read and 84 left out in one file). Not read: stairs, railings, groups, families, curtain-wall panels, sloped floors, linked models. Real-file tests run only when a sample folder is set; no Autodesk file is committed. | The provenance document's structure (a finding table per release, with measured results) and the left-out report, for Construct's Revit-import work. The synthetic fixture builders ([`fixture.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/rvt/src/fixture.rs), [`project_fixture.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/rvt/src/project_fixture.rs)) as test-file generators. | Construct's [Interop table](../../../spare-construct/ARCHITECTURE.md) lists direct `.rvt` reading as "only once an owner clears it". I found no record of that clearance, so I did not plan any use beyond reading. The reader is for models coming in, never going out. |
| **Undo** | [`engine.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/engine.rs) line 46: 200 steps, each command rolled back on failure. CADCraft [`doc_state.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/engine/src/doc_state.rs) and [`undoctl.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/engine/src/undoctl.rs): whole-drawing snapshots over a copy-on-write store, 2,000 steps. | The copy-on-write store makes snapshots cheap. A data point for A02. | A05's plan is undo from the operation log, so neither snapshot design is adopted. |
| **Geometry** | [`csg.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/csg.rs) (BSP, `f64`, `EPS = 1e-5` mm, capped work meter, used only for family voids) and [`geom.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/geom.rs) (plan only) | None. | Not for our kernel: floating point and an epsilon. |
| **Other** | Schedules with calculated fields, filters, sort, group and totals ([`schedule.rs`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/schedule.rs), 1,452 lines, 12 tests); a hand-written XLSX writer; ISO sheets and a hand-written PDF writer; a `tools/revit-crosscheck` script that rebuilds a described tower in Revit (the Revit result file is not in the repository and the test is ignored). | The schedule field model for Construct's schedule work. | Our PDF uses the `pdf-writer` crate (ARCHITECTURE.md Interop). |

**BIMCraft's own claims are uneven.** Its README says file format 12; the code says [`FORMAT: u32 = 17`](https://github.com/IPedrax/BIMCraft/blob/238442f5/crates/core/src/model.rs). I did not run any test. Multi-user is "next" there, not built.

### 19. Other takes that save real work

- **CADCraft's constraint solver** ([`lm.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/constraints/src/lm.rs), [`dof.rs`](https://github.com/storytold/cadcraft/blob/c4d9e6ac/crates/constraints/src/dof.rs), [tests](https://github.com/storytold/cadcraft/tree/c4d9e6ac/crates/constraints)): damped least-change solving that names the conflicting constraints, and degrees of freedom by pivoted QR. *Feeds* A11 (sketch solver) and G217 and G231 (sketch corpus and the exact-rank spike). *Take:* the 16 DOF tests and the 18 solver tests as corpus cases, and the idea of naming the conflict set in an error. *Re-derive:* DOF by pivoted floating-point QR decides rank by tolerance; G231 asks for an exact rank.
- **SolidCraft's parametric document** ([`params.rs`](https://github.com/tangency-labs/solidcraft/blob/6029a38e/crates/doc/src/params.rs), [configurations](https://github.com/tangency-labs/solidcraft/blob/6029a38e/docs/configurations.md), [assembly mass properties](https://github.com/tangency-labs/solidcraft/blob/6029a38e/docs/assembly-properties.md)): one expression graph for user parameters, sketch dimensions and feature inputs with cycle detection. *Feeds* Construct's regeneration chapter (ARCHITECTURE.md "Regeneration and constraints") and A03. *Take:* the "one graph" shape. Not read in depth: the timeline file and the evaluator.
- **SolidCraft's SSI corpus folder** ([`tests/ssi-corpus`](https://github.com/tangency-labs/solidcraft/tree/6029a38e/tests/ssi-corpus), 122 files). Our 12.17 corpus is the 119-case corpus; I did not compare the two. *Feeds* G160 and G180. Check for overlap before taking.
- **CADCraft's architecture layering** (`cargo xtask layers`, [docs/architecture.md](https://github.com/storytold/cadcraft/blob/c4d9e6ac/docs/architecture.md)) and solidcore's own [docs/layers.md](https://github.com/tangency-labs/solidcore/blob/b4de2149/docs/layers.md) (kernel, record, convert, ops, tess; a check enforces the edges). *Feeds* A10 (ArtCraft's layout and agent workflow) and chapter 27. Ours already has a crate table (27); take only the idea of a mechanical edge check if chapter 27 has none.

## Part C: the ten best takes, ranked

Value for least work, with whether each can start now.

| # | Take | Feeds | Can start now? |
| --- | --- | --- | --- |
| 1 | Attack-test batches 2 and 2b from the defect log (section 2) | geom-test of G44 to G69, then G162 | **Yes.** Batch 2b needs nothing. Batch 2 waits for PR #465. |
| 2 | SolidCraft's 69 analytic oracle cases into the closed-form truth corpus (section 6) | G179 (PR #456), G247, G283 | **Yes.** G179 is built. Re-derive each value from its formula. |
| 3 | Solidcore's table of exact blend classes and the refusal-names-the-size contract (section 8) | Chapter 13.10 and 13.11 text, G262, G263, G205 | **Yes** as a specification edit. Code waits for G262. |
| 4 | The 23 SolidCraft naming scenarios as the next naming fixtures (section 15) | G141, G88, chapter 20 | **Yes** to write the list. Fixtures wait for G132. |
| 5 | difftest's perturb check, heap-pad check, fuzz probe and waiver rules (section 4) | G113, G161 | **Yes** as input to G113's session. |
| 6 | The topology-first healing order, the pcurve checks and the measured healing trade-off (section 11) | G418 to G420, G427 | **Yes** as a design note. Code waits for G415 and G413. |
| 7 | Certified blend method: Krawczyk unique-root proof per span plus a Hermite remainder, and the Bernstein convexity certificate for setback fills (section 8) | G264, G170, G268 | **Yes** as a method note. Code waits for G262 and G168. |
| 8 | solidcore's closed-form screw-wall bound and its cell-and-Taylor sweep bound, and the line-and-arc frame proof (sections 5 and 7) | G254, G256, G257, 13.22 | **Yes** as a note and an owner question. Code waits for G254. |
| 9 | BIMCraft's IFC CI recipe (`validate_ifc.py` and `ifc-coverage.txt`) and the left-out-and-listed report (section 18) | G441, G422, G423, G426 | **Yes** for the CI recipe once the oracle runner (G113) is on main. IFC code is not started. |
| 10 | The command-layer patterns: `expect` verification, hostile-parameter sweep, read-only and atomic modes (section 16) | Construct `construct-api` and a new command-layer note | **Yes**, as a note. |

Close behind: the kernel-case corpus as an outside failure catalogue (section 14, needs a record converter), CADCraft's round-trip census for G440 (section 17), and the STEP limits and short-name aliases for G415 (section 12, needs G355, G382 and G406 first).

## What would change my picture

- A different record format in solidcore (the converter in sections 9 and 14 depends on it).
- A maintainer reply on whether upstream would accept our gates. Not covered here.
- A real run of any of the outside corpora. I ran none.
