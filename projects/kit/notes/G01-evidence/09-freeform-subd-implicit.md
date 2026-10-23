# Spare Kit geometry kernel: representations and operations beyond exact B-rep for free-form work that stays reliable

*Evidence for [G01](../G01-geometry-kernel.md), the Spare Kit geometry kernel decision note. Written 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of each project at the pinned commits linked below. GPL and LGPL code is described in the agent's own words, never copied. Claims it could not confirm from a primary source are marked unverified.*

Researched 2026-10-21. Every repository link is pinned to a commit SHA. "Licence" means the LICENSE file or file headers I actually read. *unverified* means I could not confirm it from a primary source. "Inference" means my own reasoning, not something a source states.

**Summary.** No single representation is both exact and guaranteed never to fail. Spare Kit needs:
- four first-class geometry kinds:
  - exact B-rep (analytic surfaces plus NURBS)
  - Catmull–Clark SubD
  - implicit fields (F-rep)
  - exact-predicate meshes
- point clouds as reference data, not as solids;
- a conversion graph in which every edge is labelled exact, certified-ε or heuristic;
- results that carry their exactness class and accumulated ε, so that nothing is downgraded silently.

---

## 1. Subdivision surfaces

**Basics**
- **Regular faces are exact B-splines.** A regular Catmull–Clark face is exactly a uniform bicubic B-spline patch.
  - Catmull & Clark 1978: [doi:10.1016/0010-4485(78)90110-0](https://doi.org/10.1016/0010-4485(78)90110-0).
  - OpenSubdiv's Bfr overview says regular regions are "a single parametric patch… uniform bicubic B-spline for Catmull-Clark": [bfr_overview.rst L684-695](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/documentation/bfr_overview.rst?plain=1#L684-L695).
  - The patch type is `REGULAR` ("regular B-Spline patch for the Catmark scheme"): [patchDescriptor.h L40-46](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/opensubdiv/far/patchDescriptor.h#L40-L46).
- **Loop subdivision does not fit a B-rep well.** Its regular patch is a triangular quartic box spline with 12 control points ([patchDescriptor.h L40, L136](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/opensubdiv/far/patchDescriptor.h#L130-L140)). That is not a tensor-product NURBS, so Loop surfaces cannot drop into a NURBS B-rep. Loop's 1987 thesis: [MSR page](https://www.microsoft.com/en-us/research/publication/smooth-subdivision-surfaces-based-on-triangles/).
- **Extraordinary vertices (EVs) cannot be exact NURBS.** At an EV the limit surface is an infinite sequence of polynomial rings.
  - Stam evaluates it exactly at any parameter through an eigenbasis: [doi:10.1145/280814.280945](https://doi.org/10.1145/280814.280945).
  - Catmull–Clark is only tangent-plane continuous at EVs: Peters & Reif, *Subdivision Surfaces* [doi:10.1007/978-3-540-76406-9](https://doi.org/10.1007/978-3-540-76406-9).
  - So no finite NURBS reproduces an EV region exactly. That is my inference from the infinite-ring structure, consistent with Rhino's docs below.
- **Creases.** Semi-sharp creases come from DeRose, Kass & Truong: [doi:10.1145/280814.280826](https://doi.org/10.1145/280814.280826). OpenSubdiv encodes them as follows:
  - Sharpness 0 is smooth and ≥10 is infinite. Rules are SMOOTH, DART, CREASE and CORNER: [crease.h L50-70](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/opensubdiv/sdc/crease.h#L50-L70).
  - Creasing method is `CREASE_UNIFORM` or `CREASE_CHAIKIN`.
  - Vertex boundary interpolation is NONE, EDGE_ONLY or EDGE_AND_CORNER.
  - There are six face-varying linear modes, and Catmark or "smooth" triangle weights: [options.h L36-65](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/opensubdiv/sdc/options.h#L36-L65).

**OpenSubdiv (version 3.7.0)**
- **Licence:** the "Tomorrow Open Source Technology License 1.0", which is Apache-2.0 with only §6 (Trademarks) changed ([LICENSE.txt L1-4, L141-144](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/LICENSE.txt#L1-L4)).
  - It keeps the Apache §3 patent grant ([L76-90](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/LICENSE.txt#L76-L90)), but that grant covers only "the Work", not a clean-room re-implementation.
  - GitHub reports the licence as NOASSERTION because it is not an SPDX-standard licence, so it needs a legal check before any code is ported.
- **Feature-adaptive refinement:** Nießner et al. 2012, [doi:10.1145/2077341.2077347](https://doi.org/10.1145/2077341.2077347). In `PatchTableFactory`:
  - default `maxIsolationLevel` is 10;
  - end-cap types are BILINEAR, BSPLINE, GREGORY_BASIS (the default) and LEGACY_GREGORY: [patchTableFactory.h L38-55](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/opensubdiv/far/patchTableFactory.h#L38-L55).
  - Gregory patches are rational blends of interior points in (u,v), not NURBS.
- **Bfr is approximate at irregular faces, not exact Stam evaluation.**
  - Defaults are `_approxLevelSmooth(2)` and `_approxLevelSharp(6)`: [surfaceFactory.h L99-140](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/opensubdiv/bfr/surfaceFactory.h#L99-L140).
  - Irregular regions are "a set of patches in a hierarchy resulting from local subdivision", and Bfr "intentionally hides the details" ([L684-695](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/documentation/bfr_overview.rst?plain=1#L684-L695)).

**SubD to NURBS methods**
- **Peters, "Patching Catmull-Clark meshes"** ([doi:10.1145/344779.344908](https://doi.org/10.1145/344779.344908)): standard bicubic NURBS patches, G1 at EVs. This is the CAD-grade route.
- **Loop & Schaefer ACC** ([doi:10.1145/1330511.1330519](https://doi.org/10.1145/1330511.1330519)): the geometry is only C0 at edges touching an EV, with separate tangent patches for shading. Not CAD-grade.
- **Gregory variant** ([doi:10.1145/1618452.1618497](https://doi.org/10.1145/1618452.1618497)): also not CAD-grade.
- **Karčiauskas & Peters, guided C2 constructions** ([doi:10.1007/3-540-27157-0_9](https://doi.org/10.1007/3-540-27157-0_9)): higher degree, better shape.
- **Ma:** a CAD overview ([doi:10.1016/j.cad.2004.08.008](https://doi.org/10.1016/j.cad.2004.08.008)) and Catmull–Clark surface fitting ([doi:10.1109/GMAP.2000.838259](https://doi.org/10.1109/GMAP.2000.838259)).
- **Truncated hierarchical Catmull–Clark** for local refinement and IGA: [doi:10.1016/j.cma.2015.03.019](https://doi.org/10.1016/j.cma.2015.03.019).

**Rhino SubD (public docs only)**
- The ToNURBS page says "extraordinary vertices can only be approximated". Options are G0, G1, G1x, G1xx and G2, and "Forcing G2 adds bulges or dents". Packed mode merges quad grids into larger surfaces ([docs.mcneel.com](https://docs.mcneel.com/rhino/8/help/en-us/commands/tonurbs.htm)).
- The same page claims "Continuity on a SubD at extraordinary vertices is G2". That conflicts with standard Catmull–Clark theory, which gives only G1 there. McNeel may use modified rules near EVs (*unverified*).
- The C++ API `ON_SubDToBrepParameters` documents None, LocalG1, LocalG1x, LocalG1xx and LocalG2, with the deviation ordering between them ([developer.rhino3d.com](https://developer.rhino3d.com/api/cpp/class_o_n___sub_d_to_brep_parameters.html)).
- `opennurbs_subd*.cpp` is publicly visible, but the openNURBS LICENSE file says "All Rights Reserved" plus a warranty disclaimer and the zlib text, with no grant ([LICENSE L1-6](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/LICENSE#L1-L6)). Do not read it for porting.

**Patents (Google Patents status; legal status *unverified*)**
- **T-splines core, US 7,274,364:** Sederberg/BYU, now Autodesk. Priority 2003-03-26. Status "Expired – Lifetime", anticipated expiration 2024-03-26 ([link](https://patents.google.com/patent/US7274364B2/en)).
- **Autodesk US 9,269,189:** converts B-rep faces to trim-free T-splines. **Active, adjusted expiration 2034-02-01** ([link](https://patents.google.com/patent/US9269189B1/en)).
- **Pixar semi-sharp / hybrid subdivision family:**
  - US 6,222,553 is "Expired – Lifetime" ([link](https://patents.google.com/patent/US6222553)).
  - US 6,300,960, US 6,489,960 and RE41677 were filed in 1997. Their expiry (probably around 2017) is *unverified*.
- **Stam, US 6,389,154** (exact evaluation of box-spline subdivision): "Expired – Lifetime" ([link](https://patents.google.com/patent/US6389154)).
- **Loop–Schaefer ACC, US 2008/0043023 A1:** abandoned in 2012 and never granted ([link](https://patents.google.com/patent/US20080043023A1/en)).

**Recommendation: yes, SubD should be a first-class surface type, Catmull–Clark only.**
- Support n-gons and triangles through Catmull–Clark rules, with Uniform and Chaikin semi-sharp creases.
- Match OpenSubdiv's boundary and face-varying options so cages round-trip with USD, OpenSubdiv and Rhino.
- Write our own Rust evaluator:
  - regular faces as bicubic B-splines;
  - EVs by Stam's method in f64, using eigen-tables shipped as constants rather than computed at runtime (needed for bit-identity);
  - creases near an EV refined until sharpness is exhausted.
- Convert to B-rep as follows:
  - regular regions become exact NURBS (packed);
  - EV one-rings use Peters-style G1 bicubic patches;
  - store the Hausdorff deviation, measured against the exact evaluator, on each face.
- Use OpenSubdiv as a test oracle, not as a dependency.
- Do not make T-splines first-class: the core patent has lapsed, but derivative patents run to 2034.

## 2. Implicit, field and SDF modelling

**Theory**
- F-rep: Pasko et al. [doi:10.1007/BF02464333](https://doi.org/10.1007/BF02464333).
- Interval arithmetic with recursive subdivision for implicit CSG: Duff [doi:10.1145/133994.134027](https://doi.org/10.1145/133994.134027) and Snyder [doi:10.1145/142920.134024](https://doi.org/10.1145/142920.134024).
- Affine arithmetic: [doi:10.1023/B:NUMA.0000049462.70970.b6](https://doi.org/10.1023/B:NUMA.0000049462.70970.b6).
- Interval methods compared: [doi:10.1016/S0167-8396(02)00146-2](https://doi.org/10.1016/S0167-8396(02)00146-2).
- Blending CSG trees: Wyvill, Guy & Galin [doi:10.1111/1467-8659.00365](https://doi.org/10.1111/1467-8659.00365).

**libfive**
- **Licence:** the library, stdlib and Python bindings are MPL-2.0. libfive-guile and Studio are GPL-2.0-or-later ([README L54-63](https://github.com/libfive/libfive/blob/918387a9220dbac2f7ab32fb35e274c6794439cf/README.md?plain=1#L54-L63)). Port only from `libfive/`.
- **Meshing** is described as "watertight, manifold, hierarchical, feature-preserving" ([L10-13](https://github.com/libfive/libfive/blob/918387a9220dbac2f7ab32fb35e274c6794439cf/README.md?plain=1#L10-L13)). In `dc_tree.inl`:
  - interval evaluation classifies cells as EMPTY or FILLED and pushes a pruned tape ([L94-112](https://github.com/libfive/libfive/blob/918387a9220dbac2f7ab32fb35e274c6794439cf/libfive/src/render/brep/dc/dc_tree.inl#L94-L112));
  - each leaf gets a manifoldness test from its corner mask ([L268-269](https://github.com/libfive/libfive/blob/918387a9220dbac2f7ab32fb35e274c6794439cf/libfive/src/render/brep/dc/dc_tree.inl#L268-L269));
  - vertices are placed by a mass point from max-rank intersections plus a QEF ([L481-495](https://github.com/libfive/libfive/blob/918387a9220dbac2f7ab32fb35e274c6794439cf/libfive/src/render/brep/dc/dc_tree.inl#L481-L495)).
  - Simplex and hybrid meshers also exist.

**Fidget (MPL-2.0, the same licence as the Spare Kit core)**
- **Evaluation:** graphs become tapes, tapes are simplified using interval results, and a JIT for aarch64 and x86_64 handles f32 point, interval, SIMD and gradient evaluation. There is also a canonical tape bytecode ([README L16-29](https://github.com/mkeeter/fidget/blob/17853f767b05a5b70bb835a63238ca3174f1204f/README.md?plain=1#L16-L29)).
- **WASM has no JIT.** It uses the interpreter only ([L80-87, L112-114](https://github.com/mkeeter/fidget/blob/17853f767b05a5b70bb835a63238ca3174f1204f/README.md?plain=1#L80-L114)).
- **Mesher:** Manifold Dual Contouring (MDC). Output is "Manifold / Watertight / Preserving sharp features", but "may contain self-intersections, and are not guaranteed to catch thin features" ([fidget-mesh lib.rs L1-14](https://github.com/mkeeter/fidget/blob/17853f767b05a5b70bb835a63238ca3174f1204f/fidget-mesh/src/lib.rs#L1-L14)). Vertices are `f32` ([L69-73](https://github.com/mkeeter/fidget/blob/17853f767b05a5b70bb835a63238ca3174f1204f/fidget-mesh/src/lib.rs#L69-L73)).
- **Intervals are f32 and not rigorous:** "does not set rounding modes, so it may not be _perfect_" ([interval.rs L5-9](https://github.com/mkeeter/fidget/blob/17853f767b05a5b70bb835a63238ca3174f1204f/fidget-core/src/types/interval.rs#L5-L9)).
- **Sharp features rely on a tuned threshold.** The QEF rank cutoff of 1e-3 is "very much a tuned value (alas!)" ([qef.rs L84-96](https://github.com/mkeeter/fidget/blob/17853f767b05a5b70bb835a63238ca3174f1204f/fidget-mesh/src/qef.rs#L84-L96)).
- **Octree builds are cancellable** through a `CancelToken` ([octree.rs L46-52](https://github.com/mkeeter/fidget/blob/17853f767b05a5b70bb835a63238ca3174f1204f/fidget-mesh/src/octree.rs#L46-L52)).

**Manifold `LevelSet` (Apache-2.0)**
- Uses marching tetrahedra on a body-centred cubic grid, snaps points to the surface, and accepts an SDF that "need not be a true distance, or even continuous" ([sdf.cpp L446-470](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/src/sdf.cpp#L446-L470)).
- Root finding is ITP with a tolerance ([L152-180](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/src/sdf.cpp#L152-L180)).
- It has no QEF, so sharp features are rounded off (inference from the method).

**Contouring theory**
- Dual contouring: [doi:10.1145/566654.566586](https://doi.org/10.1145/566654.566586).
- Manifold Dual Contouring: [doi:10.1109/TVCG.2007.1012](https://doi.org/10.1109/TVCG.2007.1012).
- Extended marching cubes: [doi:10.1145/383259.383265](https://doi.org/10.1145/383259.383265).
- Dual marching cubes: [doi:10.1109/PCCGA.2004.1348336](https://doi.org/10.1109/PCCGA.2004.1348336).
- **Guaranteed topology:** Plantinga & Vegter's interval-certified isotopic meshing ([doi:10.1145/1057432.1057465](https://doi.org/10.1145/1057432.1057465)) assumes a smooth, regular surface. It does not cover the sharp edges that CSG creates.

**Why implicits "never fail", and where failure moves to**
- nTop's blog says implicit booleans and offsets never fail because min and max are always defined ([ntop.com](https://www.ntop.com/resources/blog/how-implicits-succeed-where-b-reps-fail/)). That is vendor marketing.
- The real picture:
  - min/max of SDFs is no longer a true distance near corners and in the interior, so later offsets are approximate;
  - smooth-min blends are non-local;
  - the failure moves to meshing: grid resolution, thin features, and the sharp-feature heuristics above.
- **Lattices:** TPMS are closed-form. Beam lattices are unions of capsules and need interval pruning or tape simplification to stay local in cost.
- **Patent flags:**
  - Autodesk US 10,635,088, "hollow topology generation with lattices" ([link](https://patents.google.com/patent/US10635088)); claims not reviewed.
  - nTop is described as using "patent-pending" technology ([engineering.com](https://www.engineering.com/?p=125149)); *unverified*.

**Recommendation: a first-class `Field` type**
- A hash-consed expression graph serialised as canonical bytecode.
- f64 evaluation, with intervals rounded outward after every operation.
- One deterministic interpreter as the reference. JIT, SIMD or GPU paths are allowed only if they are bit-identical to it on a test corpus.
- Transcendental functions come from one pure-Rust implementation, never platform libm. This is an inference: std float functions may call the platform libm, which would break native/WASM bit-identity.
- Exact-distance opcodes that reference B-rep faces (see §3).
- Meshing by MDC, porting the method (and MPL-2.0 code where useful) from Fidget and libfive, followed by certification:
  - interval-bound the field over each output triangle to get a Hausdorff bound;
  - report AMBIGUOUS cells at maximum depth as thin-feature risk.

## 3. Representation conversion with guarantees

**SubD to NURBS**
- Exact on regular Catmull–Clark regions.
- Approximate at EVs, certified against the Stam evaluator (§1).

**NURBS to SubD**
- Exact only for non-rational, uniform-knot bicubic surfaces, which are themselves Catmull–Clark cages (inference from the regular-patch equivalence).
- Otherwise fitted, using Ma & Zhao (above).

**B-rep/NURBS to mesh**
- Approximate with a certifiable chord bound.
- Watertight only if each edge is tessellated once and both adjacent faces' parameter-space triangulations are constrained to it (Sheng & Hirsch, [doi:10.1016/0010-4485(92)90011-X](https://doi.org/10.1016/0010-4485(92)90011-X)).
- The root cause of gaps is mismatch between trim curves and 3D edges ([doi:10.1007/s11831-017-9220-9](https://doi.org/10.1007/s11831-017-9220-9)).
- Two patent-encumbered routes to avoid in core:
  - **Watertight trimmed NURBS via T-splines** ([doi:10.1145/1360612.1360678](https://doi.org/10.1145/1360612.1360678)) overlaps Autodesk US 9,269,189.
  - **"watertightCAD" reparametrisation** ([doi:10.1016/j.cad.2019.05.034](https://doi.org/10.1016/j.cad.2019.05.034)) is covered by US 10,614,195 (University of Texas / University of Utah). It is **Active, anticipated expiration 2037-02-15** ([link](https://patents.google.com/patent/US10614195B2/en)). Family members US 10,339,266 and US 10,614,256 have *unverified* status.

**B-rep to implicit**
- Exact as a definition: signed distance is sign × the minimum distance to the faces.
- **Sign** should come from generalized winding numbers, which are robust to gaps:
  - meshes: Jacobson et al. [doi:10.1145/2461912.2461916](https://doi.org/10.1145/2461912.2461916);
  - fast version: Barill et al. [doi:10.1145/3197517.3201337](https://doi.org/10.1145/3197517.3201337);
  - trimmed NURBS directly: Spainhour, Gunderman & Weiss [doi:10.1145/3797957](https://doi.org/10.1145/3797957), implemented in LLNL Axom (BSD-3) [winding_number.hpp L751-930](https://github.com/LLNL/axom/blob/95d267a301789fa94d125cb936f8ba0908ebd57f/src/axom/primal/operators/winding_number.hpp#L751-L930).
- **Distance** comes from a BVH plus a closest-point Newton solve with interval bounds, giving an ε-certified value.
- **Broken input:** the signed heat method ([doi:10.1145/3658220](https://doi.org/10.1145/3658220); geometry-central, MIT, [signed_heat_method.h](https://github.com/nmwsharp/geometry-central/blob/019669ddabda05e0f71fa3587cfb3c1dadf19cb8/include/geometrycentral/surface/signed_heat_method.h)).

**Implicit to mesh**
- Watertight and manifold via MDC.
- Isotopy (correct topology) only under Plantinga–Vegter conditions (§2).

**Implicit to B-rep**
- There is no general exact method.
- Exactness trick: keep the CSG graph, recognise plane, sphere, cylinder, cone and torus primitives and their booleans, and route those to the exact B-rep. Only blends, lattices and TPMS become meshes or fitted NURBS.

**Mesh to B-rep (reverse engineering)**
- Heuristic only. It must output a deviation map. The pipeline:
  - primitive detection: RANSAC, [doi:10.1111/j.1467-8659.2007.01016.x](https://doi.org/10.1111/j.1467-8659.2007.01016.x);
  - quad layout:
    - Instant Meshes, BSD-style licence ([LICENSE](https://github.com/wjakob/instant-meshes/blob/7b3160864a2e1025af498c84cfed91cbfb613698/LICENSE.txt)) — [doi:10.1145/2816795.2818078](https://doi.org/10.1145/2816795.2818078);
    - QuadriFlow, BSD-style licence ([LICENSE](https://github.com/hjwdzh/QuadriFlow/blob/810b7a0967c35b0dc85b4464e3835e26a756c967/LICENSE.txt)) — [doi:10.1111/cgf.13498](https://doi.org/10.1111/cgf.13498);
    - mixed-integer quadrangulation: [doi:10.1145/1576246.1531383](https://doi.org/10.1145/1576246.1531383);
    - surveys: [doi:10.1111/cgf.12014](https://doi.org/10.1111/cgf.12014) and [doi:10.1111/cgf.13153](https://doi.org/10.1111/cgf.13153);
  - B-spline fitting: Eck & Hoppe, [doi:10.1145/237170.237271](https://doi.org/10.1145/237170.237271);
  - full pipelines: [doi:10.1016/S0010-4485(96)00054-1](https://doi.org/10.1016/S0010-4485(96)00054-1) and [doi:10.1016/j.cad.2013.06.004](https://doi.org/10.1016/j.cad.2013.06.004).

**Mesh to SubD**
- Exact as a cage: any polygon mesh is a valid cage.
- The limit surface is not the mesh (it shrinks), so matching the mesh means fitting.

**Mesh to smooth surface**
- Manifold builds weighted cubic Bézier edge tangents ([smoothing.cpp L53-63](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/src/smoothing.cpp#L53-L63)). The result is approximate.

## 4. Hard free-form operations

**Fillets and blends, including variable radius**
- References:
  - rolling-ball blends: [doi:10.1016/0010-4485(89)90046-8](https://doi.org/10.1016/0010-4485(89)90046-8);
  - survey: [doi:10.1016/0010-4485(94)90023-X](https://doi.org/10.1016/0010-4485(94)90023-X);
  - Gn blending: [doi:10.1007/PL00013398](https://doi.org/10.1007/PL00013398).
- Method:
  - find contact curves as the intersection of the offset surfaces at radius r(s);
  - sweep rational-quadratic arcs between them;
  - fit the result as NURBS.
- It fails when the radius exceeds the local radius of curvature, or when the blend runs over other edges (needs vertex blends and setbacks).
- OCCT's ChFi3d is LGPL-2.1 ([repo](https://github.com/Open-Cascade-SAS/OCCT/tree/53706f78a8640c4407bdcc6752df13a270b159f2)): method study only.
- Fallback: an implicit smooth-min blend, flagged as approximate.

**Shelling and thickening doubly curved surfaces**
- NURBS offsets are generally not NURBS, so they must be approximated:
  - [doi:10.1016/S0010-4485(98)00066-9](https://doi.org/10.1016/S0010-4485(98)00066-9);
  - survey: [doi:10.1016/S0010-4485(99)00013-5](https://doi.org/10.1016/S0010-4485(99)00013-5).
- Trim local and global self-intersections with distance maps: [doi:10.1016/j.cad.2005.08.002](https://doi.org/10.1016/j.cad.2005.08.002).
- For variable thickness t(x), or anything organic, use a field shell {0 ≤ d(x) ≤ t(x)}.

**Lofts and network surfaces**
- Skinning: [doi:10.1016/0010-4485(88)90002-4](https://doi.org/10.1016/0010-4485(88)90002-4).
- Gordon surfaces come from Gordon 1969, *J. Math. Mech.* 18(10). I found no DOI; the citation is *unverified*.
- **TiGL (Apache-2.0) is the best portable source.**
  - Skinning with automatic parameters, a maximum degree and C2 closure: [CTiglCurvesToSurface.h L36-62](https://github.com/DLR-SC/tigl/blob/f05c1e301bb462df1135f9c5778771a7ebe22d2d/src/geometry/CTiglCurvesToSurface.h#L36-L62).
  - Curve network: computes profile×guide intersection parameters, sorts the curves, and errors on more than two intersections: [CTiglInterpolateCurveNetwork.cpp L66-128](https://github.com/DLR-SC/tigl/blob/f05c1e301bb462df1135f9c5778771a7ebe22d2d/src/geometry/CTiglInterpolateCurveNetwork.cpp#L66-L128).
  - Gordon surface = S_profiles + S_guides − S_tensor, after common knot vectors: [CTiglGordonSurfaceBuilder.cpp L154-215](https://github.com/DLR-SC/tigl/blob/f05c1e301bb462df1135f9c5778771a7ebe22d2d/src/geometry/CTiglGordonSurfaceBuilder.cpp#L154-L215).
  - Paper: [doi:10.1007/s11786-019-00401-y](https://doi.org/10.1007/s11786-019-00401-y).

**Panelisation and rationalisation**
- Core references:
  - Pottmann et al., *Architectural Geometry*, Bentley Institute Press 2007, ISBN 978-1-934493-04-5 ([TU Wien](https://www.geometrie.tuwien.ac.at/ig/publications/oldpub/2007/buch/book.html));
  - survey: [doi:10.1016/j.cag.2014.11.002](https://doi.org/10.1016/j.cag.2014.11.002).
- Methods:
  - planar-quad and conical meshes: [doi:10.1145/1141911.1141941](https://doi.org/10.1145/1141911.1141941);
  - paneling with mould reuse: [doi:10.1145/1778765.1778782](https://doi.org/10.1145/1778765.1778782);
  - single-curved panels: [doi:10.1145/1360612.1360675](https://doi.org/10.1145/1360612.1360675);
  - quad-dominant planar meshes: [doi:10.1111/j.1467-8659.2010.01776.x](https://doi.org/10.1111/j.1467-8659.2010.01776.x);
  - circle and sphere packing: [doi:10.1145/1661412.1618485](https://doi.org/10.1145/1661412.1618485);
  - geodesic patterns: [doi:10.1145/1833349.1778780](https://doi.org/10.1145/1833349.1778780).
- Evolute's tools are commercial; use only the papers.
- **libigl:**
  - `planarize_quad_mesh` implements ShapeUp ([header](https://github.com/libigl/libigl/blob/7100764c2a2833284a8bdfa9b948d2b3bf124624/include/igl/planarize_quad_mesh.h); [doi:10.1111/j.1467-8659.2012.03171.x](https://doi.org/10.1111/j.1467-8659.2012.03171.x)).
  - Licence: core is MPL-2.0, but `igl::copyleft` is GPL ([copyleft/README L1-15](https://github.com/libigl/libigl/blob/7100764c2a2833284a8bdfa9b948d2b3bf124624/include/igl/copyleft/README.md?plain=1#L1-L15)).
  - GitHub's repo-level detector reports **GPL-3.0**, so automated licence tooling will mislabel it.
- **ShapeOp (MPL-2.0 per file headers; no LICENSE file in the mirror):**
  - constraints include Plane, Circle, Sphere, Similarity, Rectangle, Parallelogram and Angle ([Constraint.h L85-486](https://github.com/EPFL-LGG/ShapeOp/blob/9a79b808c5e4144b8b6f0f429df7273f61e95547/libShapeOp/src/Constraint.h#L85-L486));
  - paper: [doi:10.1007/978-3-319-24208-8_42](https://doi.org/10.1007/978-3-319-24208-8_42).

**Surface division and pattern mapping (Revit-style "divided surface", in our own words)**
- The feature:
  - lays a U/V grid on a surface by count, by distance, or by min/max spacing;
  - maps tile patterns through (u,v);
  - places panel families at the grid points ([Revit API: Rationalizing a Surface](https://help.autodesk.com/cloudhelp/2016/ENU/Revit-API/files/GUID-63B37608-4112-4424-B46A-ED61ACE95E7F.htm)).
- Its weakness: isoparametric division distorts panels on non-uniform parameterisations ([forum](https://forums.autodesk.com/t5/revit-architecture-forum/facade-pattern/td-p/13865168)).
- Better: divide in a chosen parameterisation:
  - LSCM: [doi:10.1145/566654.566590](https://doi.org/10.1145/566654.566590);
  - BFF (MIT): [repo](https://github.com/GeometryCollective/boundary-first-flattening/tree/6924b2fd22fc26cb25d532a8130a0b44454dbeb0), [doi:10.1145/3132705](https://doi.org/10.1145/3132705);
  - ARAP parameterisation: [doi:10.1111/j.1467-8659.2008.01290.x](https://doi.org/10.1111/j.1467-8659.2008.01290.x);
  - stripe patterns: [doi:10.1145/2767000](https://doi.org/10.1145/2767000), in geometry-central (MIT, [surface/](https://github.com/nmwsharp/geometry-central/tree/019669ddabda05e0f71fa3587cfb3c1dadf19cb8/include/geometrycentral/surface)).

**Unrolling and developable surfaces**
- Exact (isometric) unrolling is possible only where Gaussian curvature is zero.
- Otherwise it is approximate, and a strain map should be reported:
  - strip unfolding: [doi:10.1145/1186562.1015711](https://doi.org/10.1145/1186562.1015711);
  - seam cutting: [doi:10.1145/3197517.3201356](https://doi.org/10.1145/3197517.3201356);
  - developability flow: [doi:10.1145/3197517.3201303](https://doi.org/10.1145/3197517.3201303);
  - discrete orthogonal geodesic nets: [doi:10.1145/3180494](https://doi.org/10.1145/3180494);
  - curved folding: [doi:10.1145/1360612.1360674](https://doi.org/10.1145/1360612.1360674).
- Conformal maps (LSCM, BFF with cone singularities) are exact only in the discrete sense.

## 5. Parametric and computational design runtime

**How existing tools do it**
- **Sverchok** (GPL-3; method study only):
  - topological sort ([update_system.py L162-172](https://github.com/nortikin/sverchok/blob/e3da4dc0c602c9753ba20ed5d7898cc953be79c4/core/update_system.py#L162-L172));
  - a set of "outdated" nodes that propagates changes;
  - a generator that yields between nodes, which gives cooperative cancellation via `CancelError` and per-node timing ([L453-480](https://github.com/nortikin/sverchok/blob/e3da4dc0c602c9753ba20ed5d7898cc953be79c4/core/update_system.py#L453-L480)).
- **Rhino.Compute** (MIT, [LICENSE.md](https://github.com/mcneel/compute.rhino3d/blob/78e202f8da358b39f929ebb9ef424ed0b628488f/LICENSE.md)):
  - caches results by an MD5 hash of the request ([DataCache.cs L25-40](https://github.com/mcneel/compute.rhino3d/blob/78e202f8da358b39f929ebb9ef424ed0b628488f/src/compute.geometry/DataCache.cs#L25-L40)) and an in-memory Hops cache ([MemoryCache.cs L10-23](https://github.com/mcneel/compute.rhino3d/blob/78e202f8da358b39f929ebb9ef424ed0b628488f/src/hops/MemoryCache.cs#L10-L23));
  - content-addressed caching is only sound if solves are deterministic.
- **Fidget:** bulk evaluators ([eval/bulk.rs](https://github.com/mkeeter/fidget/blob/17853f767b05a5b70bb835a63238ca3174f1204f/fidget-core/src/eval/bulk.rs)) and cancellation tokens.
- **Manifold:** n-ary `BatchBoolean` and `BatchUnion` ([csg_tree.cpp L412-496](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/src/csg_tree.cpp#L412-L496)).
- **Speckle** (Apache-2.0):
  - meshes are flat vertex lists plus faces prefixed by their vertex count ([Mesh.cs L13-35](https://github.com/specklesystems/speckle-sharp-sdk/blob/994761589c7a4d8a405ccbd3ee02ad3b1f048252/src/Speckle.Sdk/Objects/Models/Geometry/Mesh.cs#L13-L35));
  - `BrepX`, `ExtrusionX` and `SubDX` are opaque native blobs plus a **mandatory** `displayValue` mesh ([BrepX.cs L11-33](https://github.com/specklesystems/speckle-sharp-sdk/blob/994761589c7a4d8a405ccbd3ee02ad3b1f048252/src/Speckle.Sdk/Objects/Models/Geometry/BrepX.cs#L11-L33));
  - the blob formats are "3dm", "dwg" and "sat" ([RawEncoding.cs L20-25](https://github.com/specklesystems/speckle-sharp-sdk/blob/994761589c7a4d8a405ccbd3ee02ad3b1f048252/src/Speckle.Sdk/Objects/Models/Other/RawEncoding.cs#L20-L25));
  - so exact free-form exchange through Speckle depends on proprietary formats.
- **Fornjot** (0BSD) has shut down: "Its goals were never reached" ([README L1](https://github.com/hannobraun/fornjot/blob/7e856686b8105d21ce4673c10622f6ef206b89af/README.md?plain=1#L1)). It was a Rust B-rep kernel built on "reliability over features"; the lesson is about scope.

**What the kernel must provide**
1. Pure operations with canonically serialised inputs, so the kernel version plus a content hash can key a memo cache.
2. Bit-identical results on native and WASM, and single- and multi-threaded:
   - fixed work partitioning and ordered reductions;
   - no dependence on hash-map iteration order;
   - no platform libm.
3. Batch, structure-of-arrays APIs: evaluate N points per call, and n-ary booleans.
4. Cancellation tokens plus time and memory budgets, checked at bounded work quanta.
5. Per-item `Result`s, so one failed panel out of 10,000 does not fail the whole list.
6. Preview and final tolerances as explicit parameters, cached separately.
7. Immutable geometry with structural sharing between variants.
8. An open, documented exact encoding that always carries a display mesh, its tolerance, and provenance.

## 6. Reliability under extreme complexity

**How kernels fail, and the proven responses**
- **Near-tangent surface intersection and small loops:**
  - loop detection: [doi:10.1016/0167-8396(88)90029-5](https://doi.org/10.1016/0167-8396(88)90029-5);
  - lower-dimensional formulation: [doi:10.1145/237748.237751](https://doi.org/10.1145/237748.237751);
  - [doi:10.1016/S0167-8396(96)00024-6](https://doi.org/10.1016/S0167-8396(96)00024-6);
  - multivariate solver: [doi:10.1145/376957.376958](https://doi.org/10.1145/376957.376958);
  - prefer Bernstein-basis arithmetic for conditioning: [doi:10.1016/0167-8396(87)90012-4](https://doi.org/10.1016/0167-8396(87)90012-4).
  - SISL and GoTools are **AGPL-3.0** ([SISL](https://github.com/SINTEF-Geometry/SISL/tree/e0a82b1a851e932657f3e1af25033fbbcd33e907), [GoTools](https://github.com/SINTEF-Geometry/GoTools/tree/9b58396b7746ea97bfc51a8c540cee1b5c5eccdc)): method study only.
- **Tolerance growth:** Parasolid-style local tolerances ([doi:10.1145/218013.218067](https://doi.org/10.1145/218013.218067)).
  - Mitigation: store per-entity tolerance, cap it, and refuse an operation rather than grow tolerance past the cap.
- **Coincident or near-coincident operands:** OCCT's Boolean options include ([docs](https://occt3d.com/dev/doc/overview/html/specification__boolean_operations.html); LGPL, method only):
  - fuzzy tolerance;
  - gluing for coinciding arguments;
  - non-destructive mode;
  - oriented bounding boxes;
  - parallel processing.
- **Scale:**
  - BVH/OBB hierarchies: [doi:10.1007/BF02434040](https://doi.org/10.1007/BF02434040);
  - interval or affine pruning;
  - n-ary booleans.
- **Inside/outside classification:** generalized winding numbers (§3) instead of ray parity.
- **Robust meshing of messy input:** envelope methods [doi:10.1145/3386569.3392385](https://doi.org/10.1145/3386569.3392385) and mesh arrangements [doi:10.1145/2897824.2925901](https://doi.org/10.1145/2897824.2925901).
- **Theory:** robustness in geometric computation, [doi:10.1115/1.1375815](https://doi.org/10.1115/1.1375815).
- **Process:**
  - validate after every operation, and heal deterministically;
  - fall back from B-rep to mesh or field only explicitly, with ε recorded;
  - keep test corpora, fuzzing and oracles: OpenSubdiv for SubD, libfive and Fidget for fields, Manifold for meshes.

## 7. Recommendation

**First-class representations**
1. Exact B-rep, analytic plus NURBS (covered by other agents).
2. Exact-predicate mesh solids: the universal fallback and common exchange form.
3. Implicit `Field` (§2).
4. Catmull–Clark SubD (§1).
5. Point clouds as indexed reference data only; they never take part in booleans.

Not first-class:
- T-splines, because of patents running to 2034.
- Loop SubD: import it as a cage and approximate.
- Voxels: only as a cache of a field.

**One model, one boolean**
- Every body carries an exactness class (Exact, Certified(ε) or Heuristic) plus provenance.
- Dispatch:
  - if all operands are exact B-rep (including SubD regular regions as NURBS), use the B-rep boolean;
  - if a B-rep boolean fails, return a structured error offering an *explicit* fallback;
  - if any operand is a field, use a field boolean (B-rep operands enter as exact-distance plus winding-number fields), with lazy output or a mesh at ε;
  - if any operand is a mesh, use the mesh boolean on certified watertight tessellations.
- The result's class is the least exact input class, and ε accumulates.
- Lattice infill: field ∩ (B-rep as a field) produces the infill. The exact B-rep skin stays a separate body.

**Build order and sources**
- **P0, alongside the B-rep and mesh work:**
  - certified watertight tessellation (Sheng–Hirsch method);
  - winding-number classification (port from Axom, BSD-3);
  - BVH.
- **P1, field engine:** port from Fidget and the libfive kernel (both MPL-2.0), with f64 and rounded intervals; MDC mesher; B-rep as field.
- **P2, mesh geometry processing:**
  - BFF and geometry-central (MIT);
  - libigl core (MPL-2.0): `lscm`, `arap`, `planarize_quad_mesh`;
  - a ShapeOp-style constraint solver (MPL-2.0);
  - Division objects, unrolling.
- **P3, NURBS constructions:**
  - skinning, curve networks and Gordon surfaces (port from TiGL, Apache-2.0);
  - offsets with self-intersection trimming (papers);
  - certified surface-intersection improvements.
- **P4, SubD:** written from the papers, with OpenSubdiv as oracle; Peters-style conversion to NURBS.
- **P5:** constant- then variable-radius rolling-ball fillets, and shelling free-form B-rep.
- **P6:** mesh or point cloud to B-rep reverse engineering (heuristic).

## Adopt / Avoid / Open questions

**Adopt**
- **Catmull–Clark SubD with semi-sharp creases:**
  - our own Stam evaluator with constant eigen-tables;
  - Peters-style G1 conversion with stored deviation;
  - OpenSubdiv option parity.
- **Field engine:**
  - a canonical bytecode graph;
  - f64 evaluation with outward-rounded intervals;
  - tape pruning;
  - an MDC mesher with certification (Fidget and libfive methods; MPL-2.0 code may be reused).
- **Winding-number classification:** Axom (BSD-3) and Barill et al.
- **Gordon surfaces, skinning and curve networks:** TiGL (Apache-2.0).
- **Constraint-based rationalisation:** ShapeUp/ShapeOp (MPL-2.0).
- **Parameterisation:** BFF and geometry-central (MIT); LSCM and ARAP (libigl MPL-2.0 core).
- **Runtime contract:**
  - content-hash caching;
  - per-item results;
  - cancellation tokens;
  - n-ary booleans;
  - display mesh plus an open exact encoding.
- **Exactness class and ε on every body.**

**Avoid**
- Code (method study only):
  - libfive Studio and Guile bindings (GPL-2+), and libigl `copyleft/` (GPL);
  - SISL and GoTools (AGPL-3.0);
  - OCCT (LGPL-2.1);
  - Sverchok (GPL-3).
- Porting openNURBS `opennurbs_subd*`: the LICENSE file has no grant.
- Trim-free T-spline conversion (US 9,269,189, to 2034) and watertightCAD reparametrisation (US 10,614,195, to about 2037).
- f32 or non-rounded intervals, platform libm, and JIT paths that are not bit-checked against the interpreter.
- Silent fallback from B-rep to mesh.

**Open questions**
1. Does the OpenSubdiv licence (Apache-2.0 with a modified §6) combine cleanly with MPL-2.0? It needs legal review before any code is ported.
2. Patents still unverified:
   - expiry of the Pixar family (US 6,300,960, US 6,489,960, RE41677);
   - status of US 10,339,266 and US 10,614,256;
   - claims of Autodesk lattice patent US 10,635,088;
   - nTop's pending filings;
   - any Evolute or Pottmann panelisation patents (not searched).
3. Rhino's claim of "G2 at extraordinary vertices": what modified rules does it imply? This matters for exact Rhino SubD round-tripping.
4. Which correctly rounded or fixed transcendental library should be used for native/WASM bit-identity? This is shared with the exact-arithmetic agent.
5. Can Plantinga–Vegter-style isotopy guarantees be extended to fields with CSG sharp edges? If not, which post-hoc certificate is acceptable for "iron-clad"?
6. Is the variable-radius fillet fallback an implicit blend, or a refusal? This is a product decision.
7. What is the minimum G1 deviation for SubD to NURBS at high-valence EVs that still meets facade fabrication tolerance? This needs measurement against the Stam oracle.
