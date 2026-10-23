# Spare Kit numeric layer: exact and robust arithmetic, predicates, constructions, tolerances, FP determinism

*Evidence for [G01](../G01-geometry-kernel.md), the Spare Kit geometry kernel decision note. Written 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of each project at the pinned commits linked below. GPL and LGPL code is described in the agent's own words, never copied. Claims it could not confirm from a primary source are marked unverified.*

All repository links are pinned to the commit SHAs I read on 2026-10-21. "Derived" means my own arithmetic, not taken from a source. *Unverified* means I could not confirm it from a primary source.

**Bottom line**
- **Integer grid.** Put every kernel coordinate on an integer grid. With |coord| < 2^40 grid units, orient2d and orient3d are exact in plain `i128`. Every construction a polyhedral core needs fits in at most 384-bit fixed-width integers. The hot path then needs no rationals, no expansions and no rounding-mode tricks.
- **Grid quantum q = 156.25 nm.** That is 1/6400 mm. It is the largest quantum that makes both 0.01 mm and 1/256 inch exact integers, and it gives ±171.8 km per frame.
- **Implicit constructions.** Keep constructed points implicit inside an operation (homogeneous integer points and planes). Round only at commit.
- **What Rust already guarantees.** + − × ÷, sqrt and fma are IEEE-exact and Rust never contracts.
- **What is left to police.** Every remaining source of nondeterminism can be caught by lints or CI:
  - libm transcendentals;
  - NaN bits;
  - `max`/`min` on ±0;
  - `algebraic_*` and "relaxed" operations;
  - rayon reduction order;
  - HashMap iteration order;
  - address-ordered tie-breaking.

## 1. Shewchuk adaptive predicates

**Method**
- Numbers are held as *expansions*: sums of non-overlapping doubles, built with error-free TwoSum and TwoProduct (Dekker split, splitter 2^27+1). See [robust lib.rs L1997–L2019](https://github.com/georust/robust/blob/654f34cb8cdb3ae21bf59ef3472f92652942cd74/src/lib.rs#L1997-L2019).
- Each predicate runs four stages:
  - **A:** plain floating-point determinant, accepted if it exceeds an a-priori bound times the permanent.
  - **B:** products of the rounded differences, computed exactly. If every difference tail is zero, this is already exact ([L110–L132](https://github.com/georust/robust/blob/654f34cb8cdb3ae21bf59ef3472f92652942cd74/src/lib.rs#L110-L132)).
  - **C:** adds first-order tail corrections.
  - **D:** fully exact.
- Stage-A bounds (ε = 2^-53): orient2d (3+16ε)ε, orient3d (7+56ε)ε, incircle (10+96ε)ε, insphere (16+224ε)ε. Sources: [predicates.c `exactinit`, L643–L686](https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c) and [robust L53–L66](https://github.com/georust/robust/blob/654f34cb8cdb3ae21bf59ef3472f92652942cd74/src/lib.rs#L53-L66). Paper: [Shewchuk 1997](https://doi.org/10.1007/PL00009321).

**Assumptions**
- It assumes round-to-nearest-even binary64 with no extended precision; the C file suggests `volatile` against x87 registers ([predicates.c L90–L131](https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c); the CMU file cannot be pinned to a SHA).
- **It has no overflow or underflow handling.** Maintainers confirm both the C and Rust versions can give wrong signs or NaN when intermediates underflow ([robust #19](https://github.com/georust/robust/issues/19)).
- geometry-predicates says inputs below 1e-142 or above 1e201 are unsafe ([README L73–L77](https://github.com/elrnv/geometry-predicates-rs/blob/365c139fa9fcd2e429b33c9f9b0e68df21affcfe/README.md#L73-L77)).

**Licence:** public domain ([predicates.c header L9](https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c)), so it can be ported freely into MPL-2.0.

**Rust ports**
- **`robust` 1.2.0** — MIT OR Apache-2.0 ([Cargo.toml](https://github.com/georust/robust/blob/654f34cb8cdb3ae21bf59ef3472f92652942cd74/Cargo.toml)), about 26.7 M downloads.
  - It is a direct transcript with the `exactinit` constants hard-coded.
  - orient2d's stage A was replaced by the [Ozaki et al. filter](https://doi.org/10.1007/s10543-015-0574-9), with the underflow guard deliberately dropped ([L94–L108](https://github.com/georust/robust/blob/654f34cb8cdb3ae21bf59ef3472f92652942cd74/src/lib.rs#L94-L108)).
  - An FMA-based TwoProduct was declined to keep `no_std` ([PR #27](https://github.com/georust/robust/pull/27)).
  - CI is x86 only, with no wasm job ([rust.yml](https://github.com/georust/robust/blob/654f34cb8cdb3ae21bf59ef3472f92652942cd74/.github/workflows/rust.yml)).
  - Predicates return an f64 rather than a sign.
- **`geometry-predicates` 0.3.0** — MIT, last push February 2021. It exposes the expansion primitives and ships `predicates.c` plus a c2rust transpile ([src](https://github.com/elrnv/geometry-predicates-rs/tree/365c139fa9fcd2e429b33c9f9b0e68df21affcfe/src)).

**wasm behaviour**
- wasm f64 arithmetic is IEEE round-to-nearest-even and Rust does not fuse (§7), so Shewchuk's preconditions hold.
- wasm's only float nondeterminism is NaN sign and payload ([numerics, #nan-propagation](https://webassembly.github.io/spec/core/exec/numerics.html#nan-propagation)), which finite inputs never reach.
- Upstream does not test wasm, so parity there is *unverified*, though it is expected by construction.
- Rust's libm uses no hardware fma on wasm32 ([arch/mod.rs L11–L18](https://github.com/rust-lang/compiler-builtins/blob/aeab328368d6ed8dc6acba47b7db6ad08be953d2/libm/src/math/arch/mod.rs#L11-L18)). An FMA TwoProduct would therefore be a slow software call there, so keep Dekker.

## 2. Geogram PCK (BSD-3-Clause)

**Licence:** BSD-3-Clause, Inria ([LICENSE](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/LICENSE)). It can be ported into MPL-2.0 if the notice is kept.

**DSL and generated filters**
- Predicates are written in a small DSL ([orient3d.pck](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/predicates/orient3d.pck)) and compiled to C++ by the [pck generator](https://github.com/BrunoLevy/geogram/tree/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/bin/pck).
- The generated orient3d filter ([orient3d.h L88–L98](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/predicates/orient3d.h#L88-L98)):
  - uses a semi-static bound `eps = 5.11071278299732992696e-15·max1·max2·max3`;
  - returns "uncertain" if the smallest magnitude is below 1.63e-98 or the largest above 5.6e101 — an underflow/overflow guard;
  - then falls back to exact arithmetic ([predicates.cpp L1841–L1851](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/predicates.cpp#L1841-L1851)).
- That guard closes Shewchuk's underflow gap.
- CGAL's hand-written static filter uses the identical constant ([Orientation_3.h L114](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Filtered_kernel/include/CGAL/Filtered_kernel/internal/Static_filters/Orientation_3.h#L114-L139)). Both come from Meyer & Pion's [FPG generator](https://inria.hal.science/inria-00344297).

**expansion_nt**
- Shewchuk expansions as a number type, allocated on the stack with `alloca` ([multi_precision.h L363–L374](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/multi_precision.h#L363-L374)), with an `optimize()` length reduction ([L1174–L1179](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/multi_precision.h#L1174-L1179)).
- TwoProduct uses fma when available. The source warns that compiler contraction must be switched off (`-ffp-contract=off`) or expansion sums break ([L161–L171](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/multi_precision.h#L161-L171)). This is direct evidence that contraction is fatal to expansion arithmetic.

**Symbolic perturbation (SoS)**
- Every `_SOS` predicate ends in a cascade over the sorted points: [side4_3d_exact_SOS L963–L1000](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/predicates.cpp#L963-L1000), [orient_3d_SOS L1853–L1887](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/predicates.cpp#L1853-L1887), and the generic `SOS()` template ([PCK.h L143–L172](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/PCK.h#L143-L172)).
- **The default ordering is `SOS_ADDRESS`, which sorts points by pointer address** ([L83](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/predicates.cpp#L83), [L154–L166](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/predicates.cpp#L154-L166)). That is not reproducible across runs or machines. Lexicographic order is optional. Spare Kit must order by persistent ID.

**exact_nt**
- In the open build, the exact scalar type is `expansion_nt`.
- The comment says the faster `exact_nt` is 10–20× faster and has no overflow/underflow risk, but it comes from Tessael's proprietary GeogramPlus ([exact_geometry.h L66–L71](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/exact_geometry.h#L66-L71), [L464–L472](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/exact_geometry.h#L464-L472)). Spare Kit should not plan on it.

**Intervals**
- `interval_nt` defaults to a round-to-nearest implementation with outward error propagation (Harris' method) ([interval_nt.h L455–L462](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/interval_nt.h#L455-L462)).
- The round-upward variant was rejected because it needs `-frounding-math` ([L831–L845](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/interval_nt.h#L831-L845)).
- The round-to-nearest variant is the one that maps onto Rust and wasm.

**Papers:** [Lévy 2016, PCK](https://doi.org/10.1016/j.cad.2015.10.004); [Lévy 2024, exact mesh CSG](https://arxiv.org/abs/2405.12949), which uses expansion or multiprecision-float kernels (I read the abstract only).

## 3. CGAL kernel design (cited files are LGPL-3.0-or-later; method only)

**Filtered predicates**
- [Filtered_predicate.h L88–L114](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Filtered_kernel/include/CGAL/Filtered_predicate.h#L88-L114) switches the FPU to round-upward and evaluates the predicate on `Interval_nt`. If the sign is certain it returns; otherwise it restores round-to-nearest and re-evaluates exactly. Static filters run before this.
- Without rounding-mode control, CGAL widens intervals with `nextafter` ([FPU.h L347–L358](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Number_types/include/CGAL/FPU.h#L347-L358)). That is the only option in Rust, where changing the rounding mode is undefined behaviour (§7), and in wasm, which has no rounding modes.

**Kernels**
- **Epick** = filtered `Simple_cartesian<double>` ([L60–L62](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Kernel_23/include/CGAL/Exact_predicates_inexact_constructions_kernel.h#L60-L62)): exact predicates, double-precision constructions.
- **Epeck** = a lazy kernel over Gmpq, leda_rational or Quotient<MP_Float>, with interval approximations ([L39–L76](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Kernel_23/include/CGAL/Exact_predicates_exact_constructions_kernel.h#L39-L76)).
- **Lazy_exact_nt** keeps a DAG of operations with interval approximations and computes exactly only on demand ([L47–L58](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Number_types/include/CGAL/Lazy_exact_nt.h#L47-L58)).

**Why constructions are the real problem**
- Epick rounds an intersection point to double. Later exact predicates then answer questions about the rounded point, not the true configuration, and consistency is lost. [Kettner et al. 2008](https://doi.org/10.1016/j.comgeo.2007.06.003) catalogues the resulting failures.
- CGAL's own [manual](https://doc.cgal.org/latest/Kernel_23/index.html) says Epick constructions may be inexact, and that "construct a plane, then test against it" is only safe with exact number types.
- Epeck fixes this with heavy DAGs plus GMP (LGPL).
- Lesson: either keep constructions exact and implicit, or never feed rounded constructions back into topology decisions.

## 4. Indirect predicates and plane-based geometry

**Attene 2020** ([doi](https://doi.org/10.1016/j.cad.2020.102856))
- An implicit point is stored as its defining explicit inputs ([implicit_point.h L52–L63](https://github.com/MarcoAttene/Indirect_Predicates/blob/f8f243d18a6851fe3128b7e09af5b3574259d700/include/implicit_point.h#L52-L63), [L417–L471](https://github.com/MarcoAttene/Indirect_Predicates/blob/f8f243d18a6851fe3128b7e09af5b3574259d700/include/implicit_point.h#L417-L471)):
  - **SSI:** 2D segment–segment intersection;
  - **LPI:** line through 2 points intersected with plane through 3 points;
  - **TPI:** intersection of three planes, defined by 9 points;
  - plus LNC, BPT and TBC types in the code.
- Predicates are evaluated on homogeneous coordinates (λx, λy, λz, λ), which are polynomials in the inputs. The sign of λ corrects the sign of the result, so there is no division and no need for rationals.
- Evaluation cascade: semi-static filter → interval → expansion → bigfloat (`getIntervalLambda`, `getExpansionLambda`, `getBigfloatLambda`).

**Licence and portability**
- The README says LGPL-3.0-or-later ([L79–L82](https://github.com/MarcoAttene/Indirect_Predicates/blob/f8f243d18a6851fe3128b7e09af5b3574259d700/README.md#L79-L82)), but the [LICENSE file](https://github.com/MarcoAttene/Indirect_Predicates/blob/f8f243d18a6851fe3128b7e09af5b3574259d700/LICENSE) contains LGPL-2.1.
- Its numerics come from [nfg](https://github.com/MarcoAttene/nfg/tree/aeec97547a67dc6904ab84b39b15d20c6d62f65a), which is also LGPL-2.1.
- Either way it is LGPL, so method only.
- It requires `-frounding-math` and sets the FPU to round-up ([README L52–L56](https://github.com/MarcoAttene/Indirect_Predicates/blob/f8f243d18a6851fe3128b7e09af5b3574259d700/README.md#L52-L56), [implicit_point.hpp L41](https://github.com/MarcoAttene/Indirect_Predicates/blob/f8f243d18a6851fe3128b7e09af5b3574259d700/include/implicit_point.hpp#L41)). That does not transfer to Rust or wasm.

**Cherchi, Livesu, Attene follow-ups**
- [Mesh arrangements 2020](https://doi.org/10.1145/3414685.3417818) and [interactive booleans 2022](https://doi.org/10.1145/3550454.3555460).
- The code is [MIT](https://github.com/gcherchi/InteractiveAndRobustMeshBooleans/blob/7bd6c2697695f555f9a15d7e154e084d841316f0/LICENSE) but [vendors Attene's LGPL predicates](https://github.com/gcherchi/InteractiveAndRobustMeshBooleans/tree/7bd6c2697695f555f9a15d7e154e084d841316f0/arrangements/external/Indirect_Predicates). The algorithms can be ported; the predicates must be re-derived.
- The maintainers say they have limited capacity to maintain it ([README L3](https://github.com/gcherchi/InteractiveAndRobustMeshBooleans/blob/7bd6c2697695f555f9a15d7e154e084d841316f0/README.md#L3)).

**Plane-based geometry**
- Papers: [Bernstein & Fussell 2009](https://doi.org/10.1111/j.1467-8659.2009.01504.x), [Campen & Kobbelt 2010](https://doi.org/10.1111/j.1467-8659.2009.01609.x), [Nehring-Wirxel, Trettner & Kobbelt 2021](https://doi.org/10.1016/j.cad.2021.103015), [EMBER 2022](https://doi.org/10.1145/3528223.3530181).
- What EMBER does ([PDF](https://www.graphics.rwth-aachen.de/media/papers/339/ember_exact_mesh_booleans_via_efficient_and_robust_local_arrangements.pdf), §3, §3.3, §5, §6):
  - inputs are 26-bit integers per coordinate;
  - each polygon is a supporting plane plus one plane per edge, with integer coefficients;
  - vertices are three-plane intersections in homogeneous integer coordinates;
  - widths are chosen so no intermediate exceeds 256 bits;
  - inputs are scaled to fill 26 bits, about 15 nm in a 1 m³ scene;
  - exactness holds only inside the integer format, and export rounding can reintroduce tiny self-intersections.
- Key property: boolean outputs reuse input planes, so precision does not grow under iterated CSG.
- Manifold ships an EMBER benchmark harness under Apache-2.0 ([extras/ember_tests](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/extras/ember_tests/README.md)). Whether EMBER's own code is public is *unverified*.

**Derived bit budget** for B-bit signed coordinates (|x| < 2^B):

| Quantity | Bits | At B = 26 | At B = 40 | Type at B = 40 |
|---|---|---|---|---|
| Plane normal | 2B+3 | 55 | 83 | i128 |
| Plane offset d | 3B+5 | 83 | 125 | i128 |
| TPI w | 6B+12 | 168 | 252 | I256 |
| TPI x, y, z | 7B+14 | 196 | 294 | I320 |
| classify(TPI, plane) | 9B+19 | 253 | 379 | I384 |

At B = 26 the largest value is 253 ≤ 256, which matches EMBER.

## 5. Symbolic perturbation; Manifold

**Simulation of Simplicity** ([Edelsbrunner & Mücke 1990](https://doi.org/10.1145/77635.77639))
- Coordinate j of point i is perturbed by ε^(2^(i·d−j)).
- The perturbed determinant is a polynomial in ε. Its sign is the sign of the first nonzero sub-determinant in a fixed sequence, so no predicate ever returns 0.
- It needs a total order on points. [Devillers & Teillaud 2011](https://doi.org/10.1016/j.comgeo.2010.09.010) give the form used for Delaunay. Geogram's code (§2) is a readable reference.

**Caveat for CAD**
- SoS makes degeneracies consistent, not meaningful. Coplanar abutting walls become an arbitrary infinitesimal overlap or gap.
- Use SoS in general-position algorithms (CDT, Delaunay, ray parity).
- Handle coplanarity and collinearity explicitly in booleans, as EMBER and Cherchi do.
- None of the sources above provides SoS over implicit points.

**Manifold** (Apache-2.0)
- It is not exact. Coordinates are doubles with a per-mesh epsilon, and values within the smaller mesh's epsilon count as ties ([boolean3.cpp L32–L37](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/src/boolean3.cpp#L32-L37)).
- Ties are decided by a real geometric displacement ([L92–L130](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/src/boolean3.cpp#L92-L130)):
  - vertices move by tol/4 along face-offset directions;
  - P is additionally shifted by per-axis steps of 1/61, 1/89 and 1/113 so grid-aligned slopes cannot cancel.
- `Shadows` compares raw values outside the tolerance and the displacements inside it ([L163–L170](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/src/boolean3.cpp#L163-L170)).
- The semantics are deliberate: where faces coincide, P's face is the outer one.
- It is a model for topological consistency without exactness, not for bit-exact geometry.

## 6. Tolerance policy

**Prior art**
- **Parasolid:** linear precision 1e-8 (metres), angular precision 1e-11 rad, and a size box limiting any solid to 1000 units ([Bentley](https://docs.bentley.com/LiveContent/web/MicroStation%20Help-v28/en/AdvancedUnitSettings.html), [Altair](https://help.altair.com/feko/topics/feko/user_guide/cadfeko/models_parasolid_feko_c.htm)). Derived: 1e-8 / 1e3 = 1e-11, so the angular value is linear precision divided by the size box — a 36.5-bit dynamic range.
- **ACIS:** SPAresabs 1e-6 and SPAresnor 1e-10 are *unverified* (the docs are behind a login). RobotStudio reports SPAresabs = 1e-5 m ([forum](https://forums.robotstudio.com/discussion/comment/11543)).
- **OCCT** (LGPL-2.1, method only):
  - Confusion 1e-7, Angular 1e-12, Intersection = Confusion × 0.01, Approximation = Confusion × 10 ([Precision.hxx L113–L235](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/FoundationClasses/TKernel/Precision/Precision.hxx#L113-L235)).
  - Tolerances are stored per vertex, edge and face ([brep_format.md L1781–L1872](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/brep_format.md#L1781-L1872)).
  - Booleans grow them, e.g. Tol(V) = max(Tol(V), D + Tol(E)) ([boolean_operations.md L143–L183](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md#L143-L183)).
  - That makes coincidence non-transitive and dependent on history — the cause of "tolerance creep".
- **Revit:**
  - Curves shorter than `ShortCurveTolerance` cannot be created ([API help](https://help.autodesk.com/cloudhelp/2018/CSY/Revit-API/files/GUID-A1BCB8D4-5628-45AF-B399-AF573CBBB1D0.htm)).
  - The value is reported as about 1/32 in, roughly 0.8 mm ([McNeel forum](https://discourse.mcneel.com/t/how-to-fix-curve-length-is-too-small-for-revit/145407)); the exact figure is *unverified*.
  - Geometry must stay within a 16 km (10 mile) radius of the internal origin ([Revit 2023 help](https://help.autodesk.com/cloudhelp/2023/ENU/Revit-Model/files/GUID-3F79BF5A-F051-49F3-951E-D3E86F51BECC.htm)); imports warn at 20 miles / 33 km ([troubleshooting](https://help.autodesk.com/cloudhelp/2019/ENU/Revit-Troubleshooting/files/GUID-D30C39FF-3A53-47DE-8E9E-11B2AB808A07.htm)).
  - My reading: 16 km / 0.78 mm ≈ 2^24.3, inside a 53-bit double. Revit spends about 29 bits as headroom for inexact constructions, and pays with a hard minimum edge length that users hit as errors.
  - Spare Kit should make the minimum edge q (exact) and only warn above it.

**Scale arithmetic** (derived)
- 50 km at 0.01 mm is 5×10^9 steps, 32.2 bits, so 33 bits. f64 (53 bits) leaves 20 spare; the f64 ulp at 50 km is 7.3 pm.
- Raw georeferenced coordinates: the f64 ulp at an NZTM northing of 6.2×10^6 m is 0.93 nm, fine for storage. But differences of large nearby values waste about 22 bits.
- f32 ulp is 0.5 m at 6.2×10^6 m and 3.9 mm at 50 km. Rendering must therefore be camera- or tile-relative.
- **A power-of-two metre grid cannot represent 1 mm exactly**: 1 mm = 1048.576 units of 2^-20 m.
- The largest quantum making both 0.01 mm and 1/256 in exact integers is gcd(10 µm, 1/256 in) = **156.25 nm**:
  - 10 µm = 64 q
  - 1 mm = 6400 q
  - 1 in = 162,560 q
  - 1/256 in = 635 q
- If 1 µm must also be exact, the quantum drops to 31.25 nm.

**Recommended policy**
- **Single resolution.** One model-wide resolution q = 156.25 nm. Kernel coordinates are `i64` multiples of q in a local engineering frame, with |n| < 2^40, i.e. ±171.8 km. A 50 km corridor needs 2^38.2.
- **Exact simple transforms.** Rotations by multiples of 90°, mirrors and translations on the grid stay exact.
- **Georeferencing stays outside the kernel.** A frame record holds the CRS id, the origin as an i64 in q (NZTM northing ≈ 3.97×10^13 q, 45 bits), a rotation and a scale. Conversion is done once in double-double and rounded to q: the predicate crate already has TwoSum/TwoProduct, or use [`twofloat`](https://crates.io/crates/twofloat) (BSD-3).
- **A global ladder of derived tolerances, never per-entity:**
  - commit rounding ≤ q/2 = 78 nm per axis;
  - ε_fit = 64q = **0.01 mm** — curve/surface approximation and curved intersections that drive topology;
  - ε_heal = 640q (0.1 mm) by default, capped at 6400q (1 mm), applied **only at import**, logged in the op log, then discarded;
  - short-edge **warning** at 3200q (0.5 mm), never a hard limit;
  - ε_ang = 2^-34 rad ≈ 5.8×10^-11, equal to ε_fit / 171.8 km — the same construction as Parasolid's 1e-11, and used only in tolerant curved code.
- **The exact linear core uses no tolerance at all.**

## 7. Floating-point determinism in Rust

**Basic operations and FMA**
- [RFC 3514 L50–L58](https://github.com/rust-lang/rfcs/blob/f63a7efbb9d83ae0616cf752c000cd3df4ca5d3e/text/3514-float-semantics.md#L50-L58) specifies + − × ÷ %, sqrt, mul_add, casts and comparisons as IEEE 754-2008 with roundTiesToEven and no flush-to-zero. Its [tracking issue #128288](https://github.com/rust-lang/rust/issues/128288) is still open.
- `sqrt` and `mul_add` are documented as correctly rounded and fixed permanently ([std f64.rs L189–L230, L365–L392](https://github.com/rust-lang/rust/blob/19833af7224424e8b53e288097d56b6af43d5bcf/library/std/src/num/f64.rs#L189-L230)).
- **There is no automatic FMA contraction:**
  - those semantics leave no room for it;
  - the proposal to allow contraction by default ([RFC 2686](https://github.com/rust-lang/rfcs/pull/2686)) was withdrawn in 2020;
  - fusion is opt-in only, via `mul_add` (always fused) or the unstable `mul_add_relaxed`, which is documented as nondeterministically fused or not ([core f64.rs L1862–L1902](https://github.com/rust-lang/rust/blob/19833af7224424e8b53e288097d56b6af43d5bcf/library/core/src/num/f64.rs#L1862-L1902), [#151770](https://github.com/rust-lang/rust/issues/151770)).
- C has to fight for the same result: Box2D passes `-ffp-contract=off` because clang contracts under AVX2 ([CMakeLists L49–L61](https://github.com/erincatto/box2d/blob/1205f32246fca953c035b3b95069ddd154cb47d0/CMakeLists.txt#L49-L61)).

**Transcendental functions**
- std's sin/cos/exp/atan2/hypot/cbrt/powi say "The precision of this function is non-deterministic" ([std f64.rs L300–L330, L686–L707](https://github.com/rust-lang/rust/blob/19833af7224424e8b53e288097d56b6af43d5bcf/library/std/src/num/f64.rs#L686-L707)). Results vary by platform and Rust version.
- **`libm`** — MIT, pure Rust ([LICENSE](https://github.com/rust-lang/compiler-builtins/blob/aeab328368d6ed8dc6acba47b7db6ad08be953d2/libm/LICENSE.txt)):
  - sin, cos, atan2, exp, log and pow come from FreeBSD msun;
  - cbrt and hypot are ported from CORE-MATH and correctly rounded ([cbrt.rs L1–L5](https://github.com/rust-lang/compiler-builtins/blob/aeab328368d6ed8dc6acba47b7db6ad08be953d2/libm/src/math/cbrt.rs#L1-L5));
  - its `arch` feature only swaps in correctly rounded hardware operations — sqrt, fma, rint ([arch/mod.rs](https://github.com/rust-lang/compiler-builtins/blob/aeab328368d6ed8dc6acba47b7db6ad08be953d2/libm/src/math/arch/mod.rs#L11-L60)).
- **CORE-MATH** (MIT; [LICENSE](https://gitlab.inria.fr/core-math/core-math/-/blob/0ce0cbc15935e24c656280e5a46c34e89f301b7a/LICENSE)) has correctly rounded binary64 sin, cos, sincos, **sinpi, cospi**, atan2, pow, exp and log ([tree](https://gitlab.inria.fr/core-math/core-math/-/tree/0ce0cbc15935e24c656280e5a46c34e89f301b7a/src/binary64)). Correct rounding makes results identical by specification, which survives any future reimplementation.
- The [`core-math` crate](https://github.com/jdh8/core-math/blob/a074c66b8abae33ed4ce58ff22a99d43d760dec1/README.md) (MIT) binds the C sources; whether it builds for wasm32-unknown-unknown is *unverified*.
- [RLIBM](https://github.com/rutgers-apl/The-RLIBM-Project/blob/a3673fda569a10a86333bf64041aad6fa8d26e96/LICENSE) is MIT but targets 32-bit types.

**NaN**
- NaN payload and sign are nondeterministic, including on wasm ([RFC L63–L85](https://github.com/rust-lang/rfcs/blob/f63a7efbb9d83ae0616cf752c000cd3df4ca5d3e/text/3514-float-semantics.md#L63-L85)).
- The wasm spec's [deterministic profile](https://webassembly.github.io/spec/core/appendix/profiles.html#profile-deterministic) canonicalises NaNs and fixes relaxed SIMD.
- Rule for Spare Kit: the kernel never produces NaN.

**Other traps in std**
- `max`/`min` may return either sign for ±0 ([core f64.rs L1035–L1058](https://github.com/rust-lang/rust/blob/19833af7224424e8b53e288097d56b6af43d5bcf/library/core/src/num/f64.rs#L1035-L1058)).
- `algebraic_*` operations, stable since 1.98, are explicitly nondeterministic ([L1806–L1815](https://github.com/rust-lang/rust/blob/19833af7224424e8b53e288097d56b6af43d5bcf/library/core/src/num/f64.rs#L1806-L1815), [primitive_docs.rs L1293–L1312](https://github.com/rust-lang/rust/blob/19833af7224424e8b53e288097d56b6af43d5bcf/library/core/src/primitive_docs.rs#L1293-L1312)).
- wasm `f64x2_relaxed_madd` may round once or twice ([relaxed_simd.rs L155–L161](https://github.com/rust-lang/rust/blob/19833af7224424e8b53e288097d56b6af43d5bcf/library/stdarch/crates/core_arch/src/wasm32/relaxed_simd.rs#L155-L161)).

**x87, denormals and rounding modes**
- i586 targets use x87 and are non-compliant ([#114479](https://github.com/rust-lang/rust/issues/114479)).
- i686 (SSE2) only loses NaN payloads in C-ABI returns ([#115567](https://github.com/rust-lang/rust/issues/115567); [platform-support L46–L51, L238](https://github.com/rust-lang/rust/blob/19833af7224424e8b53e288097d56b6af43d5bcf/src/doc/rustc/src/platform-support.md#L46-L51)).
- 32-bit ARM NEON flushes denormals to zero ([RFC L180](https://github.com/rust-lang/rfcs/blob/f63a7efbb9d83ae0616cf752c000cd3df4ca5d3e/text/3514-float-semantics.md#L177-L180)).
- Changing MXCSR (rounding mode, DAZ/FTZ) is immediate undefined behaviour ([sse.rs L1521–L1545](https://github.com/rust-lang/rust/blob/19833af7224424e8b53e288097d56b6af43d5bcf/library/stdarch/crates/core_arch/src/x86/sse.rs#L1521-L1545)), and inline assembly must restore floating-point state ([RFC L136–L146](https://github.com/rust-lang/rfcs/blob/f63a7efbb9d83ae0616cf752c000cd3df4ca5d3e/text/3514-float-semantics.md#L136-L146)).
- So any interval arithmetic must be round-to-nearest with outward widening.

**SIMD**
- Reassociation happens only through the explicit `algebraic_*` opt-in.
- The real risk is horizontal reductions that depend on lane width. Rapier forbids `simd8` together with its determinism feature for this reason ([determinism.mdx L23–L33](https://github.com/dimforge/rapier/blob/475caa3ff4646fd9a796b67d14116256f4962bc7/website/docs/user_guides/templates/determinism.mdx#L23-L33)).

**Parallelism and ordering**
- rayon's `sum` reduction order is unspecified, so float results are not fully deterministic ([mod.rs L1362–L1367](https://github.com/rayon-rs/rayon/blob/ee0a00bdb1ab039e178a215ad5712fb7fa58e58f/src/iter/mod.rs#L1362-L1367)).
- rayon's fold split points are nondeterministic ([L1134–L1139](https://github.com/rayon-rs/rayon/blob/ee0a00bdb1ab039e178a215ad5712fb7fa58e58f/src/iter/mod.rs#L1134-L1139)).
- std `HashMap` is randomly seeded ([map.rs L18](https://github.com/rust-lang/rust/blob/19833af7224424e8b53e288097d56b6af43d5bcf/library/std/src/collections/hash/map.rs#L18)).

**How other deterministic projects enforce it**
- **Rapier:** `enhanced-determinism` = `simba/libm_force` + the parry flag ([Cargo.toml L76](https://github.com/dimforge/rapier/blob/475caa3ff4646fd9a796b67d14116256f4962bc7/crates/rapier3d/Cargo.toml#L76)). A test checks that parallel results are bit-identical for any thread count ([thread_count_determinism.rs](https://github.com/dimforge/rapier/blob/475caa3ff4646fd9a796b67d14116256f4962bc7/crates/rapier3d/tests/thread_count_determinism.rs#L274-L295)).
- **Box2D v3:** precise math on MSVC, contraction off, its own atan2/sin/cos, and a determinism test on every PR ([simulation.md L2220–L2240](https://github.com/erincatto/box2d/blob/1205f32246fca953c035b3b95069ddd154cb47d0/docs/simulation.md#L2220-L2240), [math_functions.c L96–L142](https://github.com/erincatto/box2d/blob/1205f32246fca953c035b3b95069ddd154cb47d0/src/math_functions.c#L96-L142), [blog](https://box2d.org/posts/2024/08/determinism/)).

## 8. Exact and big-number options in Rust

**Fixed width — the hot path**
- Native `i128`.
- [`bnum`](https://crates.io/crates/bnum) (MIT OR Apache-2.0, const-generic widths).
- [`crypto-bigint`](https://crates.io/crates/crypto-bigint) (Apache/MIT). It has a constant-time design and is likely slower (*unverified*).
- [`ruint`](https://crates.io/crates/ruint) (MIT, unsigned only).
- Hand-written I256/I320/I384 on u64 limbs is also simple; EMBER uses a separate type per width (§5 of the PDF).

**Expansions**
- A port of the public-domain `predicates.c`, using `robust` or `geometry-predicates` as reference.
- Needed only for decisions on raw f64 before snapping, i.e. at import.

**Arbitrary precision**
- MPL-compatible: [num-rational/num-bigint](https://crates.io/crates/num-rational) (MIT/Apache) and [dashu](https://crates.io/crates/dashu) (MIT/Apache).
- Excluded: **malachite** (LGPL-3.0-only) and **rug** (LGPL-3.0+, links GMP).
- The [tczajka benchmark](https://github.com/tczajka/bigint-benchmark-rs/blob/0ee2089102b5a4f45b186fa86b466c2ce677f191/README.md) ranks malachite ≈ rug ≫ dashu > num-bigint, but it measures million-digit numbers.
- At geometry sizes (100–400 bits) allocation should dominate, so fixed-width stack integers should win. This is *unverified* and needs a benchmark.

**When rationals are actually needed**
- Cascaded constructions without commit-rounding.
- Exact export of implicit points.
- As a test oracle.
- Otherwise, never on the hot path.

## 9. Recommendation for Spare Kit

**Types** — crate `spare-num`, `no_std` + `alloc`, MPL-2.0:
- grid coordinates as `i64` in units of q = 156.25 nm, in `P2`/`P3` types validated to |n| < 2^40 at construction;
- every predicate returns `Sign { Neg, Zero, Pos }`, never an f64;
- fixed-width I256/I320/I384 (own implementation or bnum), each predicate with a written bit-budget derivation plus extreme-value tests (all coordinates at ±(2^40−1));
- implicit point types `Ssi2`, `Lpi3`, `Tpi3`, and an integer `Plane { n: [i128; 3], d: i128 }`;
- exact quantities: 2×area in i128 and 6×volume in I256, giving bit-identical quantity take-off.

**Predicates**
- orient2d and orient3d in i128; incircle and insphere in I256; `classify(point or implicit point, plane)` in ≤ I384; ordering of implicit coordinates.
- `_sos` variants ordered by persistent op-log IDs.
- Each predicate: an f64 semi-static filter in grid units (inputs and differences are exact in f64), then fixed-width exact evaluation.
- No interval stage and no rounding modes.
- Filter constants generated from a DSL at build time, PCK-style.

**Constructions**
- Explicit points live on the grid.
- New points stay implicit during an operation.
- Polyhedral solids stay plane-based (EMBER-style), so iterated booleans never round.
- Materialising vertices (export, 2D drafting) rounds once to q and is re-validated with exact predicates. 2D uses iterated snap rounding. In 3D, if rounding would invert anything, keep the implicit or plane form instead of failing.
- Curved geometry is evaluated deterministically (libm or CORE-MATH ports) to ε_fit, then snapped; exact predicates on the snapped data decide topology.

**Determinism enforcement — CI and lints**
1. Pin `rust-toolchain.toml` and commit `Cargo.lock`.
2. Make kernel crates `no_std`, so std transcendental methods are unavailable.
3. In `clippy.toml`:
   - `disallowed-methods`: f64 transcendentals, `powi`, `max`, `min`, `mul_add_relaxed`, `algebraic_*`;
   - `disallowed-types`: `HashMap`, `HashSet`;
   - deny `float_arithmetic` in the exact modules;
   - leave `suboptimal_flops`/`imprecise_flops` allowed, because their suggestions change results.
4. No f32 in the kernel and no NaN anywhere (tripwires plus fuzzing).
5. One parallel helper: fixed chunk sizes independent of thread count, ordered collect, no float reduction across chunks.
6. CI matrix:
   - targets: x86_64 Linux/Windows/macOS, aarch64 Linux/macOS, wasm32 under wasmtime and Node;
   - plus `-C target-cpu=native`, debug vs release, and RAYON thread counts 1/2/3/8;
   - each run compares golden SHA-256 hashes of canonical outputs from an op-log replay corpus.
7. Exclude i586, armv7 NEON and any relaxed-simd build from the determinism guarantee.
8. Differential fuzzing of every predicate against a num-rational oracle, plus permutation-antisymmetry property tests.

**What to port from where**
- `predicates.c` (public domain): expansions.
- Geogram PCK (BSD-3, keep the notice): the generator, FPG-style filters with overflow/underflow guards, the SOS template, `intervalRN`.
- libm and CORE-MATH (MIT): transcendental functions, preferring sinpi/cospi so that 90° rotations are exact.
- Manifold (Apache-2.0): test harness and EMBER cases.
- Cherchi (MIT): algorithms only, not the vendored predicates.
- **Method only:** CGAL, OCCT, Attene/nfg, and the EMBER paper.

## Adopt / Avoid / Open questions

**Adopt**
- The q = 156.25 nm integer grid with the 2^40 cap.
- Integer-exact predicates behind f64 filters, returning `Sign`.
- Implicit constructions and a plane-based polyhedral core; round only at commit, with validation.
- The single global tolerance ladder; healing only at import.
- Correctly rounded transcendentals as the long-term contract.
- Rapier- and Box2D-style determinism tests.
- The lint and CI rules in §9.

**Avoid**
- Per-entity tolerances that grow (OCCT), and hard minimum-edge limits (Revit).
- Feeding rounded constructions back into decisions (the Epick pattern).
- Directed rounding or FPU-mode changes: undefined behaviour in Rust, impossible in wasm.
- std transcendentals and `powi`; `max`/`min` on signed zero; `algebraic_*`; `mul_add_relaxed`; relaxed-simd.
- HashMap iteration, address-ordered SoS, and float reductions across rayon chunks.
- Shewchuk on unchecked raw f64 (underflow).
- f32 world coordinates.
- LGPL/GPL code (CGAL, OCCT, Indirect_Predicates/nfg, malachite, rug) and GeogramPlus's proprietary `exact_nt`.

**Open questions**
1. Does any product need exact 1 µm? If so the grid becomes 31.25 nm and the range drops to ±34 km at B = 40.
2. Is ±171.8 km per frame enough, or do corridor and network models need multi-frame tiling — and how would booleans work across frames?
3. Plane-based (EMBER) or vertex-based B-rep for the polyhedral core? This decides whether 3D ever rounds.
4. Exact circular arcs in 2D (degree-2 algebraic predicates), or arcs approximated at ε_fit?
5. What guarantees that curved-surface booleans never fail, beyond what the numeric layer provides?
6. Rotations as rational direction vectors or as angles via sinpi/cospi?
7. How fast is i128 and I384 arithmetic on wasm, which has no native 128-bit multiply? Needs a benchmark.
8. How much effort is it to port and verify CORE-MATH sin, cos, atan2 and sinpi to pure Rust?
