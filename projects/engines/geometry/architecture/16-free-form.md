# 16. Free-form: subdivision surfaces and fields

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Evidence: [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md) throughout, with [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md) for NURBS, [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) for intervals and maths functions, [10](../../../kit/notes/G01-evidence/10-github-sweep.md) for open projects and [01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md) for meshes. Papers and pinned source are linked wherever a claim rests on them.

Free-form shapes are those not made of planes, cylinders and other named surfaces: a sculpted roof, a car's body side, a bone-like bracket, the lattice inside a 3D-printed part. This section covers the two representations from section 9 that exist mainly for them, subdivision surfaces and fields; the conversions between all five representations; the tools that turn a free-form design into pieces that can be made; and how Class-A surface quality is measured. Two rules run through it. A shape never changes representation or loses exactness unless the caller asked, and the result says what was lost (principle 3). And every guarantee below is either proven in the module that gives it, or marked as not yet proven.

## 16.1 Subdivision surfaces

**What a subdivision surface is.** A designer shapes a control net: a coarse mesh of polygons, often called a cage. A fixed rule splits every polygon into smaller ones and moves each point to a weighted average of its neighbours. Repeated forever, the rule converges to a smooth limit surface. The net is the handle; the limit surface is the shape.

**One scheme: Catmull–Clark.** Spare Kit uses the Catmull–Clark rules ([Catmull and Clark 1978](https://doi.org/10.1016/0010-4485(78)90110-0)) for polygons of any size, with semi-sharp creases ([DeRose, Kass and Truong 1998](https://doi.org/10.1145/280814.280826)). A crease's sharpness is how many refinement steps it stays sharp before the smooth rule takes over: 0 is smooth, 10 or more is permanently sharp. A dart is a vertex where a crease ends inside the surface; a corner is a vertex held sharp. Catmull–Clark fits the kernel because a regular face (a quad whose four corners are each shared by exactly four quads, with no crease nearby) is exactly a bicubic B-spline patch, so it enters a NURBS B-rep with nothing lost. Loop subdivision's regular patch is a triangular quartic box spline. A tensor-product NURBS can hold it only as a degenerate patch with one edge collapsed to a point, which CAD handles badly ([09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), section 1).

A vertex where other than four edges meet is an extraordinary vertex, and the number of its edges is its valence. Around one, the limit surface is an infinite sequence of shrinking rings of bicubic patches. No finite NURBS reproduces that exactly.

**The control net is exact.**

```rust
struct SubdNet {                                   // geom-surface: the exact definition of a SubD surface
    points: Vec<GridPoint>,                        // on the frame's grid, so the net is Exact
    faces: FaceTable<NetVert>,                     // polygons of any size, consistently oriented
    edge_sharpness: SortedVec<(NetEdge, Sharpness)>, // NetEdge: the sorted pair of its end vertices
    vertex_sharpness: SortedVec<(NetVert, Sharpness)>,
    holes: SortedSet<NetFace>,                     // faces kept in the net but left out of the surface
    face_varying: Vec<FvarChannel>,                // per-corner data such as texture coordinates
    options: SubdOptions,
}
```

- `NetVert`, `NetEdge` and `NetFace` are local to the stored net. They are not topology indices, and references never hold them: references hold persistent names (section 20).
- Sharpness is stored as the exact single-precision value that USD and OpenSubdiv store, so a net round-trips bit for bit. NaN and negative values are refused when the net is built.
- Non-manifold nets (an edge with more than two faces, faces that share only a vertex) are accepted, as OpenSubdiv accepts them ([subdivision_surfaces.rst L296-L386](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/documentation/subdivision_surfaces.rst#L296-L386)). OpenSubdiv always makes a non-manifold edge infinitely sharp, and a non-manifold vertex an infinitely sharp corner unless it lies on a non-manifold crease ([topologyRefinerFactory.cpp L186-L306](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/opensubdiv/far/topologyRefinerFactory.cpp#L186-L306)). Spare Kit applies the same rules and checks them against OpenSubdiv. Such a net converts to a General body (section 8.1). Operations that need a manifold input refuse it with an error naming the edges.

**Option parity.** Every option OpenSubdiv and USD carry is stored, so nets round-trip with them and with the tools that use them ([options.h L36-L65](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/opensubdiv/sdc/options.h#L36-L65), [crease.h L50-L70](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/opensubdiv/sdc/crease.h#L50-L70)):

- boundary interpolation: none, edges only, or edges and corners;
- face-varying interpolation: six modes, from fully smooth to fully linear;
- creasing method: Uniform, where a crease's sharpness drops by one at each step, or Chaikin, where it is also averaged with the neighbouring crease edges so it varies smoothly along the crease;
- the triangle rule: Catmull–Clark or smooth;
- creases, corners and holes.

Their mapping to USD attributes belongs to section 24. USD has no attribute for the creasing method ([schema.usda L1118-L1224](https://github.com/PixarAnimationStudios/OpenUSD/blob/ee47c679abde5b467a7b6a41f3b2285564a4222e/pxr/usd/usdGeom/schema.usda#L1118-L1224)), so Chaikin travels as Spare Kit metadata: the attribute `uniform token spareware:creaseMethod = "chaikin"` on the Mesh prim, absent meaning uniform, written by `geom-meshio` (owner action G312, 32.32).

**Other schemes are imported, not kept live.** A bilinear net becomes bilinear NURBS patches: Exact for quads; Certified by the commit rounding for other polygons, whose centre and edge midpoints are rounded to the grid. A Loop net becomes its refined limit mesh, Certified by the hull bound below, since Loop's masks are also non-negative. Both keep the original net as recipe metadata, so USD export gives it back.

**A mesh used as a net is a new shape.** "Use this mesh as a net" is an operation of its own, not a conversion: the limit surface lies inside the mesh and can be centimetres from it. Making a subdivision surface that follows a mesh is fitting (section 16.3).

**Where the code lives.** Section 7 lists the subdivision limit surface as a surface kind. So `geom-surface` holds `SubdNet`, the local subdivision rules (the masks for smooth, boundary, crease, dart and corner vertices) and the evaluator. `geom-subd` holds the editable SubD body: building and validating nets, edits, whole-net refinement and the PCCM patches for NURBS conversion. It calls the same rules, so there is one implementation of them and no dependency cycle.

The surface kind stores a reference to the net in the body's geometry pool and the net face it covers. The net is what the file stores (section 26.2): exact grid points, sharpness, options, and the edition of the eigen-tables. Everything the evaluator builds from it is a deterministic cache, never stored.

**Evaluating the limit surface.** The surface over one net face depends only on that face's one-ring (the faces sharing a vertex with it). For each face the evaluator builds a patch tree whose leaves are regular patches, Stam patches or residual discs:

1. **Regular faces:** the bicubic B-spline patch whose control points are net points, evaluated with the NURBS algorithms.
2. **Faces at a smooth interior extraordinary vertex:** after one or two refinement steps, so every face is a quad and no face touches two extraordinary vertices, Stam's exact evaluation ([Stam 1998](https://doi.org/10.1145/280814.280945)). The subdivision matrix maps the control points around the vertex to those one step finer. The patch is written in that matrix's eigenvectors, and a point at (u, v) is found by working out which ring it lies in and scaling by powers of the eigenvalues: constant time, no repeated subdivision.
3. **Extraordinary vertices on a boundary or on an infinitely sharp crease:** Lacewell and Burley's extension of Stam's method ([2007](https://doi.org/10.1080/2151237X.2007.10129243)). Along an infinitely sharp crease each side subdivides exactly as a boundary does, so the boundary tables serve both.
4. **Semi-sharp creases:** local refinement around the crease (feature-adaptive refinement, [Nießner et al. 2012](https://doi.org/10.1145/2077341.2077347)) until its sharpness is used up. The faces are then one of the cases above.
5. **Everything else:** darts, corner vertices that are extraordinary, valence 2, and valences above the tables. The region around the vertex is refined until it is smaller than ε_fit and is then bounded by its hull. Slower, and still Certified.

At an extraordinary vertex itself, the position and the two tangents come from limit masks: fixed weightings of the control points that give them directly. Face-varying data, such as texture coordinates, is evaluated at the limit too, in each of OpenSubdiv's six modes.

**The eigen-tables are constants.** A generator in `geom-gen`, the development-only crate that holds every generator (27, accepted in 32.13), computes each valence's eigenvalues and eigenvectors offline in high precision, with a permissively licensed multiprecision library used only in development. It rounds them once to `f64` and checks them in, and continuous integration regenerates and compares. The tables cover valences 3 to 50, the range of Stam's published data ([Stam's page](https://www.dgp.toronto.edu/~stam/reality/Research/SubdivEval/index.html)), plus Lacewell and Burley's boundary cases, and belong to the kernel edition (section 26.2). The generator also:

- proves, with rigorous interval arithmetic on the small blocks the matrix splits into by rotational symmetry, that each matrix can be diagonalised, which Stam's method assumes, and fails loudly if one cannot;
- emits, per valence, a proven bound on the error that rounding the tables to `f64` introduces.

Powers of eigenvalues use the kernel's own repeated squaring, not `powi`, and terms such as cos(2π/n) come from the deterministic maths module (section 21).

Where Spare Kit leads: Zorin and Kristjansson generalised Stam's method to rules with boundaries and creases and demonstrated it for Loop ([2002](https://doi.org/10.1007/s003710100149)). The same for OpenSubdiv's Catmull–Clark dart and corner rules is not in print. Spare Kit derives those tables, checks them against the oracles below, and adds them in a later edition. Until then, case 5 applies.

**Certified bounds.** Catmull–Clark's masks, crease and corner rules included, have non-negative weights that sum to one. So every piece of the limit surface lies inside the convex hull of its control points (the smallest convex shape containing them), at any level of refinement. That hull is the conservative bound section 7 requires of a surface kind. The module header proves non-negativity rule by rule, the smooth triangle rule included, and a test checks the code against the proof. Refinement runs in `f64` in grid units; its rounding is bounded per level and added. The Stam path adds the table bound and an a-priori bound on its own `f64` evaluation. The sum is the surface's certified bound.

**Derivatives near an extraordinary vertex.** In Stam's parameterisation each ring is half the size of the last in parameter space, and λ times the size in space, where λ is the subdivision matrix's second-largest eigenvalue. So first derivatives grow like (2λ)^k on ring k. They grow without limit at valence 5 and above (λ ≈ 0.55 at valence 5) and shrink to zero at valence 3 (λ ≈ 0.41). The kind therefore reports derivative bounds per ring, and `Unbounded` for any parameter box that touches the vertex. In their place it gives a normal cone: a cone containing every surface normal over the box, which stays finite. Projection, intersection and parameter-space curves near the vertex work ring by ring, never by Newton steps across it, and tessellation there uses the hull instead of the curvature-based chord bound. Section 7's surface trait needs `Unbounded` as a possible answer; this is raised for the owner.

**Continuity is computed, not assumed.** For each net vertex the kernel records what the rules guarantee there, from the vertex's type and its eigenvalues:

| Where | Guaranteed | Source |
| --- | --- | --- |
| Regular regions | Curvature-continuous (C2) | The bicubic B-spline |
| Smooth interior extraordinary vertex | Tangent-plane continuous (G1), not curvature-continuous | [Peters and Reif 2008](https://doi.org/10.1007/978-3-540-76406-9) |
| Extraordinary vertex on a boundary, crease or dart; concave corner | Nothing: smoothness can fail, and concave corners fold | [Biermann, Levin and Zorin 2000](https://doi.org/10.1145/344779.344841) |
| Along an infinitely sharp crease | Position only (G0), by design | — |

Where nothing is guaranteed, the kernel computes a certified kink bound (the widest angle between normals across the vertex, from normal cones) and stores it. Biermann, Levin and Zorin give modified rules that fix these cases. They also change the surface OpenSubdiv makes, so offering them as an option is raised for the owner; their method is used, not their code.

**In a B-rep.** A SubD body converts exactly to a B-rep with SubD faces.

- **Faces.** One B-rep face per quad net face. An n-sided net face becomes n faces, one per corner quarter, each named `Created { role: sub(corner), from: net face }`, where the corner is named by its net vertex, not by a counter. A reference to the net face that wants all pieces resolves to all n. Each face has one rectangular parameter domain, where its pcurves and trims live.
- **Edges.** An edge's primary geometry is an isoparametric line of the limit surface: section 7's curve-on-surface kind. Away from extraordinary vertices that line is a cubic B-spline, also stored as an exact NURBS curve. Both faces evaluate it from the same boundary control data, so the checker verifies that they meet combinatorially (same data, same rules), not by sampling.
- **Continuity.** Each edge records its continuity from the table above.
- **Booleans and intersections** treat SubD faces like any curved kind. Every ring around an extraordinary vertex is an exact bicubic patch, so an intersection works ring by ring until the remaining disc is smaller than ε_fit, then bounds that disc by its hull. The result is Certified.

**Conversion to NURBS** is never implicit. An exporter to STEP, IFC or any format without subdivision surfaces takes an explicit conversion option and returns the class of every face.

| Faces | Method | Class |
| --- | --- | --- |
| Regular | The bicubic patch, merged with neighbours into one larger patch where a block of regular faces allows ("packed") | Exact |
| Regular only after refinement (next to n-gons or semi-sharp creases, away from extraordinary vertices) | Refined control points committed to the grid | Certified: a B-spline moves no further than its control points, so the bound is the commit rounding |
| Around smooth interior and boundary extraordinary vertices | PCCM(ℓ), Peters's construction ([Peters 2000](https://doi.org/10.1145/344779.344908), [paper](https://www.cise.ufl.edu/research/SurfLab/papers/00pccm.pdf)). The net is refined ℓ times. One bicubic NURBS patch covers each quad of the coarsest all-quad net, with four-fold end knots and up to two interior double knots per direction. Patches agree with the limit surface except near the vertex, join at least G1 there, and pass through its limit point. The region where they differ halves with each refinement, so ℓ is chosen from the requested ε. Boundaries use Catmull–Clark's extra outer layer, as the paper does | Approximate, as section 9 says, with a certified bound stored |
| Darts and extraordinary corners | No published G1 construction exists for these rules. The region is refined until smaller than the requested ε and closed with patches that meet their neighbours G0; a caller who requires G1 gets an error with remedies instead | Approximate, G0, with a certified bound stored |

- **The certified bound.** On each ring, split at the patch's knots, the limit surface and the PCCM patch are both bicubic polynomials over the same parameter squares. So the Bernstein coefficients of their difference bound the distance between them. (Bernstein coefficients are the control points of a polynomial written in Bernstein form, and a polynomial piece lies within their hull.) The disc left after K rings is bounded by both hulls. Each face stores this certified bound and, separately, the deviation measured at a stated set of parameter points, labelled as measured. Section 9 classes these faces Approximate; with a proven bound they could be Certified, which is raised for the owner.
- **G1 after commit.** Peters's tangency conditions have cos(2π/n) coefficients, so G1 cannot survive rounding the control points to the grid. Each edge stores its continuity as data: designed G1, plus the certified largest kink after commit. With 10 mm control spacing that kink is of the order of 10^-5 rad at q0 and 25 times smaller at q1, far above ε_ang (about 5.8 × 10^-11 rad). Operations recognise designed tangency from these records, not by comparing angles with ε_ang. Whether section 6.5 needs a rule for such designed tangencies is raised for the owner.
- **Names.** A packed patch is named `Merged { of: the net faces it covers }`, so a reference to one net face resolves to the packed face containing it.
- **Not used:** Loop and Schaefer's approximation and its variant with Gregory patches (patches whose inner control points blend between two positions), because their geometry is only position-continuous (C0) along edges at extraordinary vertices ([09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), section 1). Curvature-continuous constructions of higher degree ([Karčiauskas and Peters](https://doi.org/10.1007/3-540-27157-0_9)) can come later as new operation versions.

**Editing the net.** Operations: move points, extrude faces, insert and slide edge loops, bevel, bridge, fill holes, merge and split faces, mirror with symmetry, set crease and corner sharpness, and use a mesh as a net.

- Each names its outputs by role, such as an extrude's `cap` and `side(edge)` (section 20).
- Edits commit every new point to the grid, so the net stays Exact. Fitting a net so its limit surface follows points or a mesh is Approximate, with the measured deviation.
- B-rep faces made from a net face carry that face's name, so a reference to a roof's front face survives later net edits, or reports that the face was deleted.
- Refinement depth counts against the operation's budget, and long refinements check for cancellation at every level.

**OpenSubdiv as an oracle only.** OpenSubdiv 3.7.0's licence is Apache-2.0 with section 6 changed, not a standard licence, so no code is ported until a legal check ([09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), section 1). It runs only in continuous integration, as a separate program in its own container that exchanges only files with our code (rule 8 of section 29.20), compared on:

- refinement topology at every level, which must be identical;
- refined points, and limit positions and tangents at refined vertices ([`PrimvarRefiner::Limit`](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/opensubdiv/far/primvarRefiner.h#L158-L161), which can run in double precision, [L38-L39](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/opensubdiv/far/primvarRefiner.h#L38-L39)), to a stated tolerance, because its arithmetic is not ours;
- its newer evaluator, Bfr, at regular faces only, because Bfr approximates irregular faces by refining to fixed default levels ([surfaceFactory.h L99-L140](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/opensubdiv/bfr/surfaceFactory.h#L99-L140)).

The exact reference is our own. Every weight in the position rules is rational, and stored sharpness values are binary fractions, so limit positions at refined vertices are rational combinations of integer net points. `geom-test` computes them in exact rational arithmetic, and the Stam evaluator is tested against them.

**Patents.** Status is from Google Patents, which says it is an assumption, not a legal conclusion; the owner confirms before release, and the full record belongs in section 29.

- Pixar's semi-sharp crease and hybrid subdivision family ([US 6,300,960](https://patents.google.com/patent/US6300960B1/en), [US 6,489,960](https://patents.google.com/patent/US6489960B2/en), [RE41677](https://patents.google.com/patent/USRE41677E1/en), [US 6,222,553](https://patents.google.com/patent/US6222553)) and Stam's exact evaluation ([US 6,389,154](https://patents.google.com/patent/US6389154)) have expired. Spare Kit uses both methods.
- The T-spline core patent ([US 7,274,364](https://patents.google.com/patent/US7274364B2/en)) expired in 2024, but Autodesk's conversion of B-rep faces to trim-free T-splines ([US 9,269,189](https://patents.google.com/patent/US9269189B1/en)) runs to 2034 and the "watertightCAD" reparameterisation ([US 10,614,195](https://patents.google.com/patent/US10614195B2/en)) to about 2037. T-splines are never a geometry kind, and neither route is ever offered.
- The Loop–Schaefer application ([US 2008/0043023](https://patents.google.com/patent/US20080043023A1/en)) was abandoned and never granted.
- The other methods this section adopts have not yet been searched, and none is built before its search is recorded (section 16.6 and the open questions).

**Outside the kernel core.** Vector displacement and multiresolution detail, standard in film and game pipelines, are data on top of a limit surface. They belong to a content-creation domain toolkit (section 26), which stores them against net-face names.

**Never:** a live Loop or T-spline surface; a silent conversion to NURBS; GPU tessellation feeding back into geometry; copying openNURBS's subdivision code. Its licence file grants nothing, so [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md) (section 1) advises not reading it for porting; [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md) (section 1) finds openNURBS fine to read, because the grant is on McNeel's web page, but not to copy without written confirmation. How `geom-3dm` reads Rhino's SubD objects is raised for the owner.

## 16.2 Fields

**What a field is.** A field (an implicit or function-represented shape, [Pasko et al. 1995](https://doi.org/10.1007/BF02464333)) is a function f(x, y, z) that is negative inside the shape, positive outside and zero on its surface. Unions and intersections become min and max, offsets become subtraction, and smooth blends, lattices and variable thickness become formulas. None of these can fail. The difficulty moves to turning a field into a mesh or a B-rep ([09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), section 2), and that is where Spare Kit certifies.

**Numbers in a field.** Section 6.1 holds here too. Positions (a sphere's centre, a frame's origin) are grid points. Lengths (radii, thicknesses, periods, blend radii) are whole grid steps. Directions (a half-space's normal, a cylinder's axis, a frame's axes) are integer vectors. Only dimensionless coefficients, such as a blend's shape factor, are `f64`, and they are finite. Evaluation is in `f64` grid units (section 6.4).

**The expression graph.** A field is a graph of small operations: add, min, sine, distance to a sphere.

- **Hash-consed, with canonical bytecode.** Identical subgraphs are stored once. The graph is written in one fixed order, children before parents, ties broken by hash, so the same field always gives the same bytes. A node's content hash is a 128-bit hash of those bytes; it keys caches and the operation log. When two nodes' hashes match, their bytes are compared; a mismatch is a typed error, never a merge.
- **A persistent name per node.** Each node an operation creates also gets a `NameId` from `Created { op, role, from }` (section 20), such as `lattice.struts` or `blend(a, b)`. Editing a constant re-runs the same operation, so the node keeps its name while its content hash changes, and references survive parameter edits. A test edits a constant, re-meshes, and checks that every reference still resolves.
- **Only bit-exact rewrites.** Canonicalising may sort the two operands of add, multiply, min and max (commutative in IEEE arithmetic and in the kernel's total order) and merge duplicates. It never reassociates a sum or rewrites algebra, both of which change `f64` results.
- **Versioned opcodes.** The opcode set is part of the file format. A new opcode, or a changed meaning, is a new version, and old versions are read forever.

| Opcode family | Examples |
| --- | --- |
| Inputs | x, y, z; constants; named parameters bound at evaluation |
| Arithmetic | Add, subtract, multiply, divide, negate, abs, square, square root, min, max, clamp |
| Maths functions | sinpi, cospi (sine and cosine of π times the argument), atan2, exp, ln, floor, mod (for repeating patterns) |
| Primitives | Half-space, sphere, box, cylinder, cone, torus, capsule, extrusion and revolution of a 2D region |
| Blends | Smooth union, intersection and difference, each with a radius |
| Lattices | Triply periodic minimal surfaces, strut sets with their own bounding-volume hierarchy, unit-cell repetition |
| Bodies | Signed distance to a B-rep, mesh or SubD body, by reference. `geom-field` defines the trait and `geom-convert` implements it, so the field crate never handles B-rep topology itself |
| Sampled data | A dense grid of values (a stress map, a CT scan) interpolated by a B-spline and bounded by its coefficients; sparse grids for large scans, following OpenVDB's concepts (Apache-2.0, [10](../../../kit/notes/G01-evidence/10-github-sweep.md)) |
| Sweeps | The volume a moving shape sweeps (below) |

- **Partial operations are checked when the field is built.** For each square root, division, logarithm and so on, the interval of its argument over the field's domain must lie where the operation is defined. If it does not, building fails with an error naming the subgraph. An author who means to clamp writes the clamped form, such as `sqrt_clamped`. So evaluation never meets an undefined point and never makes a NaN.
- **Periodic patterns use sinpi and cospi of a phase**, t = x / period. Their peaks fall at exact half-integers of t, so whether an interval of t contains a peak is an exact comparison ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) also prefers sinpi and cospi).

**The field body.** A field becomes a body when it has a domain:

```rust
struct FieldBody {
    graph: FieldRef,     // canonical bytecode, by content hash
    name: NameId,        // persistent (section 20)
    domain: GridBox,     // a box on the grid; the body is { p in domain : f(p) <= 0 }
    class: Exactness,    // Certified, or Approximate if it refers to an Approximate body
}
```

- **Validity.** The checker's rules for fields: the domain is declared and bounded; every partial operation passes its domain check; interval evaluation over the domain succeeds; and the field's Lipschitz bound (the fastest its value can change with distance) over the domain is computed from the interval gradient, never typed by hand. A bare gyroid is a valid graph, but becomes a body only inside a domain.
- **Class.** The shape is the zero set of the formula in exact real arithmetic. `f64` cannot decide its sign at every point, so every query is answered with intervals and the body is Certified. A field that refers to an Approximate body (a fitted scan) is Approximate. A field made only of primitives with min, max and negation has the class of the B-rep it equals (Exact for flat primitives), and its queries go to that B-rep (below).
- **Point classification** returns `Inside`, `Outside`, or `OnWithin(ε)` when the point's interval contains zero.
- **Mass properties.** Volume and centroid come as certified intervals from the octree below: cells proven inside give a lower bound, cells not proven outside an upper one, refined until the gap is as small as asked. Certified volume is what light-weighting lattices are for. Surface area comes from the certified mesh, with a bound from each cell's normal cone.

**Evaluation.** One reference interpreter defines every answer.

- **Point:** `f64`.
- **Interval:** for a box, an interval guaranteed to contain every value f takes in it. It is computed in round-to-nearest and widened outward with `next_down` and `next_up` after every operation, as section 6.4 fixes. This is CGAL's fallback method when it cannot change the rounding mode ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 3; LGPL, method only). Geogram's `intervalRN` widens instead by multiplying each bound by 1 ± ε, Harris's method, with a special case for very small values ([interval_nt.h L455-L462](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/interval_nt.h#L455-L462), [L656-L690](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/interval_nt.h#L656-L690)). `next_up` and `next_down` need no special case at any magnitude.
- **Containment is proven, then tested.** Each opcode's module header carries the written argument that its interval contains the true values and the point evaluator's own result. A property test checks the code against it.
- **Maths functions** come from the deterministic maths module (section 21), and each must carry a proven error bound. Intervals widen by that bound. The `libm` crate's sin, cos, exp and log have no proven bound ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 7), so a build piece of section 21 adds proven-bound sinpi, cospi, exp and ln with interval versions. It is built (G05), straight from the correctly rounded CORE-MATH ports, with no interim polynomial stage: `Interval::sinpi`, `cospi`, `exp` and `ln` widen each end by one step and find the peaks of sinpi and cospi by exact comparison (the method of [Moore, Kearfott and Cloud 2009](https://doi.org/10.1137/1.9780898717716), section 5.1). Once G05 merges, a field that uses sinpi, cospi, exp or ln can be certified, which lifts the block on certified meshing of minimal-surface lattices. `atan2`, an opcode in the table above, has had its interval version since G47 (`Interval::atan2`, 12.3), so a field that uses it can be certified too. Until a function has its proven version, a field that uses it can be evaluated and meshed as Approximate, but not certified, and the build order says so.
- **Gradient:** forward-mode automatic differentiation, for points and for intervals.
- **Batch:** many points or boxes per call, in fixed chunks, budgeted and cancellable. The loops are plain Rust over arrays. The compiler may vectorise them without changing any result, because Rust never reorders floating-point operations.
- **One evaluator in the kernel.** Kernel crates are safe, `no_std` Rust (principle 13, section 21). A JIT writes and calls machine code, which needs `unsafe`. GPU arithmetic is not bit-identical by contract: WGSL allows division and square root that are not correctly rounded, and `f64` is often missing. So JIT and GPU evaluators exist only outside the kernel, for display, such as ray-marched previews ([Hart 1996](https://doi.org/10.1007/s003710050084)), and never feed geometry. Fidget's JIT is not ported; WebAssembly uses its interpreter too ([README L80-L114](https://github.com/mkeeter/fidget/blob/17853f767b05a5b70bb835a63238ca3174f1204f/README.md?plain=1#L80-L114)).

**Tape pruning.** A tape is the straight-line list of operations the interpreter runs. Over a box where intervals show that one operand of a min or max always wins, the other branch is dropped, giving a shorter tape for that box and everything inside it ([Keeter 2020](https://doi.org/10.1145/3386569.3392429)). This is what makes a lattice of millions of struts cheap: a small box sees only a few. Spare Kit prunes only when the two intervals are strictly separated. Because every interval contains the point evaluator's result, the pruned tape then returns bit-identical values.

Fidget and libfive's library are MPL-2.0, Spare Kit's own licence, so their methods and, where useful, their code are ported with their notices kept. Fidget's intervals are single precision and not rounded outward ([interval.rs L5-L9](https://github.com/mkeeter/fidget/blob/17853f767b05a5b70bb835a63238ca3174f1204f/fidget-core/src/types/interval.rs#L5-L9)), so its arithmetic is replaced. libfive Studio and its Guile bindings are GPL and are not read.

**Meshing with a certificate.**

1. **A balanced octree on the grid.** Space is split into cubes whose edges are power-of-two multiples of the grid step, so every corner is a grid point. Neighbouring cubes differ by at most one level, as the certificate below requires.
2. **Classify.** A cube whose interval excludes zero is wholly inside or outside, and stops. The others split, to a depth set by the requested ε and the budget.
3. **Certified corner signs.** Each corner is evaluated as a one-point interval. If that interval contains zero, the corner takes one declared convention (treated as outside, as Plantinga and Vegter treat a zero value) and its cubes are marked for the level-set argument in step 5.
4. **Contour.** Manifold Dual Contouring ([Schaefer, Ju and Warren 2007](https://doi.org/10.1109/TVCG.2007.1012), building on [Ju et al. 2002](https://doi.org/10.1145/566654.566586)) puts one vertex per surface sheet in each cube, at the point that best fits the surface's local tangent planes (it minimises a quadratic error function), which keeps sharp edges sharp. Its output is manifold and watertight. The method and code come from Fidget's mesher and libfive's ([dc_tree.inl L94-L112](https://github.com/libfive/libfive/blob/918387a9220dbac2f7ab32fb35e274c6794439cf/libfive/src/render/brep/dc/dc_tree.inl#L94-L112)). Fidget documents that its meshes may self-intersect or miss thin features, and that its feature threshold is tuned by hand ([lib.rs L1-L14](https://github.com/mkeeter/fidget/blob/17853f767b05a5b70bb835a63238ca3174f1204f/fidget-mesh/src/lib.rs#L1-L14), [qef.rs L84-L96](https://github.com/mkeeter/fidget/blob/17853f767b05a5b70bb835a63238ca3174f1204f/fidget-mesh/src/qef.rs#L84-L96)). Here the threshold is a named parameter recorded in the log, vertices are clamped to their cube and committed to the grid, and the exact mesh checker (section 19) rejects any self-intersection, so the mesher refines there or fails.
5. **Certify each cube.** A cube is certified if its interval excludes zero, or if every two gradients in it have a positive inner product, so no two normals in the cube are more than 90 degrees apart. That is Plantinga and Vegter's condition, 0 ∉ ⟨∇f(C), ∇f(C)⟩ ([2004](https://doi.org/10.1145/1057432.1057465)). They prove that on a balanced octree where every cube meets it, their own vertex construction gives a mesh isotopic to the surface (deformable into it without tearing or merging), for a smooth f with no critical point on the surface. Where corners took the convention in step 3, the mesh is isotopic to the level set f = −η, for an η just larger than those corners' intervals and smaller than every other corner's |f|; inside a certified cube that level set is isotopic to the surface. This argument is part of the written proof below. Their proof covers their construction, not Manifold Dual Contouring. A written proof that Manifold Dual Contouring on such cubes is also isotopic is a deliverable, reviewed before the mesher may return Certified. If that proof fails, certified output uses Plantinga and Vegter's construction, and Manifold Dual Contouring serves the Approximate path.
6. **The bound ε is the sum of three parts.**
   - Mesh to surface: over each triangle, |f| ≤ F. Let g be a lower bound on the gradient's length over a box enlarged by F/g around the triangle, enlarged again until it fits. Following the gradient from any triangle point reaches the surface within F/g.
   - Surface to mesh: from the cube certificate. Each certified cube's piece of surface lies in the cube and the mesh covers every cube the surface crosses, so the distance is bounded by the cube's size, tightened by its normal cone. The derivation is part of the written proof.
   - Commit rounding: √3·q/2.

   The mesher refines until the sum meets the requested ε.
7. **Cubes that fail** are where two sheets meet at a sharp edge made by a min or max, and where a feature may be thinner than a cube (the interval straddles zero but every corner has the same sign). Plantinga and Vegter's guarantee assumes a smooth surface, and no published method certifies meshes of fields with such sharp edges ([09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), open question 5).
   - Where Spare Kit leads: the graph is known, so at a min or max whose operands are both active in a cube, each operand's sheet is certified on its own, and the sharp edge is certified as the meeting of two certified sheets (both gradients, and their cross product, bounded away from zero). This is a proposal and a research piece, not yet proven.
   - Meanwhile every failed cube is listed with its box and reason (`SharpFeature`, `ThinFeatureRisk`, `DepthLimit`). The result is Certified only if no cube fails. Otherwise the operation returns an error offering a finer depth, the exact B-rep route below, or an explicit Approximate result with its measured deviation.
8. **Names.** Each triangle records the field node active at its centre. `geom-convert` groups triangles by active node into faces named by role `from(node name)`, so a reference to "the lattice's outer skin" survives re-meshing and parameter edits. Single triangles are never offered as references.

**Default ε and budget.** The default ε is ε_fit, as section 9 requires, and callers may ask for coarser. At q1, ε_fit is 0.4 µm, and a million-strut lattice meshed to that would need billions of triangles. So before work starts the mesher estimates the triangle count and, if it exceeds the budget, refuses with `BudgetExceeded` and remedies (a coarser ε, or slicing) rather than running out part-way. For 3D printing, slicing is the recommended route.

**Exact where the field allows.** Many fields are booleans of exact primitives. `geom-convert` recognises subgraphs made only of primitives with min, max and negation, and sends them to the B-rep boolean (section 11): Exact for flat faces, Certified for curved. This works because primitive constants are grid values (above). Only blends, lattices, minimal surfaces and general formulas are meshed ([09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), section 3).

**Lattices and minimal surfaces,** for 3D printing, heat exchangers and light-weighting:

- **Triply periodic minimal surfaces:** gyroid, Schwarz P and D, diamond and others, by their trigonometric formulas. These formulas approximate the true minimal surfaces ([von Schnering and Nesper 1991](https://doi.org/10.1007/BF01313411)); the kernel's gyroid is exactly the zero set of its formula, and its documentation says so. Sheet form is |f| ≤ t; network form is f ≤ t. Neither gives constant wall thickness, so the kernel answers certified minimum wall thickness and pore size (the largest sphere that fits in the void) queries, which decide printability.
- **Strut lattices:** unions of capsules along a graph of nodes and struts, from a unit cell (body-centred cubic, octet and others) repeated through space, or from any graph supplied.
- **Graded and conformal lattices:** cell size and strut thickness are themselves fields, and a lattice can follow a body's shape by mapping its unit cell through a parameterisation. Evaluating at a point x then needs the parameter that maps to x. It is found by interval Newton or the Krawczyk method (interval forms of Newton's method whose test proves that a box holds exactly one solution, or none) ([Moore, Kearfott and Cloud 2009](https://doi.org/10.1137/1.9780898717716)). Where neither test succeeds the box is split, and at the depth limit the interval is unbounded, which makes the mesher refine or fail. It never guesses. The mapping's stretch enters the Lipschitz bound.
- **Stochastic lattices** (foams, Voronoi): seeds are input data, made by a separate generator operation with a stated seed and algorithm, because `OpContext` holds no randomness (section 10.1).
- **Infill:** lattice ∩ (body as a field) gives the infill. The exact B-rep skin stays a separate body. Autodesk's active US 10,635,088 covers hollow topology generation with lattices (claims not reviewed), so: stop and ask before building lattice hollowing (section 29.17).

**Variable-thickness shells.** A shell is {x : |d(x)| ≤ t(x)/2}, where d is the signed distance to a mid-surface (any B-rep, mesh or SubD body, as a field) and t is a field: a constant, a formula, or sampled data such as a stress map. The thickness equals t only where t varies slowly, so the kernel measures the actual minimum and maximum thickness, certified, and reports them rather than assuming. Constant-thickness B-rep shells are section 13's.

**Smooth blends.** Blend opcodes join shapes with a rounded transition of a stated radius, following [Ricci 1973](https://doi.org/10.1093/comjnl/16.2.157), R-functions ([Shapiro 2007](https://doi.org/10.1017/S096249290631001X)) and blending of CSG trees ([Wyvill, Guy and Galin 1999](https://doi.org/10.1111/1467-8659.00365)). Each opcode states two things. Its radius of influence, outside which it equals plain min or max, keeps a blend over thousands of shapes cheap after pruning. Its Lipschitz bound is what certification relies on. A blended field is no longer a true distance, so later offsets of it are measured, not assumed. When a B-rep fillet cannot be built (section 13), a field blend is one of the explicit alternatives its error may offer, never a silent substitute.

**Swept volumes.** The volume a shape sweeps as it moves (a cutter along a toolpath, a door swinging, a crane slewing) is a field: at each point, the minimum over time of the moving shape's field. Intervals over space and time, splitting both, bound it like any field. So it is meshed by the certified mesher like any other field. A faster route is the Lifted Surfacing method (section 13.17), re-implemented from the paper and its erratum only. The repository's own files are Apache-2.0, but the method's core comes from dependencies under Adobe's non-commercial research licence, and its build uses GPL CGAL. So no file of the repository or of those dependencies is read, and a patent check comes first. Sellán's swept-volumes (MIT) is studied ([10](../../../kit/notes/G01-evidence/10-github-sweep.md), item 14). Whether Lifted Surfacing's output can be certified is unverified, so that route is Approximate until its certificate is proven. Sweeps of profiles along paths are section 13's; clearance and clash are section 19's.

**Fields as attributes.** A graded material, a temperature or a simulation result is a field attached to a body as an attribute (section 8), not a shape. It uses the same graph and evaluator and carries no exactness class.

**Booleans and manufacturing.** A boolean with a field operand is a min or max node (section 9). It cannot fail, and the result's class is that of its least exact operand. For 3D printing a field is sliced straight into layers: each layer's outline is contoured with the two-dimensional version of the certified mesher (Plantinga and Vegter cover curves too) and handed to `geom-2d` as regions, so a lattice that would need billions of triangles never becomes a mesh.

**Never:** single-precision or unrounded intervals; changing the rounding mode; platform maths functions; a JIT or GPU result feeding geometry; a claim of correct topology without a certificate.

## 16.3 The conversion graph

Every conversion between representations is an operation in `geom-convert`, and every edge returns its class and its bound. A path's class is the least exact of its edges, and bounds add (section 9). An edge is Exact only if its output is the same shape as its input. A point cloud claims no coverage: its points lie on the surface, exactly or within ε, and nothing more.

| From | To | Method | Class |
| --- | --- | --- | --- |
| B-rep | Mesh | Watertight tessellation: each edge sampled once and shared by its faces ([Sheng and Hirsch 1992](https://doi.org/10.1016/0010-4485(92)90011-X); section 19) | Exact for flat faces; Certified(ε) for curved |
| B-rep | SubD | A face that is a non-rational bicubic B-spline with uniform knots and no end multiplicity, untrimmed or trimmed only along knot lines, is exactly a net whose boundary interpolation is "none": OpenSubdiv then leaves the outer ring of faces out of the surface ([topologyRefinerFactory.cpp L180-L184](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/ffffe6fb821dd59eabd8ed630720115ab74f76a3/opensubdiv/far/topologyRefinerFactory.cpp#L180-L184)) | Exact |
| | | Every other face is fitted ([Ma and Zhao 2000](https://doi.org/10.1109/GMAP.2000.838259)) | Approximate |
| B-rep | Field | Distance from a bounding-volume hierarchy and certified closest points; sign from section 11.4's certified count of signed ray crossings, a proven integer, exact for plane-based bodies. Generalized winding numbers tolerate small gaps ([Jacobson et al. 2013](https://doi.org/10.1145/2461912.2461916), [Barill et al. 2018](https://doi.org/10.1145/3197517.3201337), and for trimmed NURBS [Spainhour, Gunderman and Weiss](https://doi.org/10.1145/3797957), implemented in LLNL Axom, BSD-3, [10](../../../kit/notes/G01-evidence/10-github-sweep.md)), but Axom computes them by quadrature with no proven bound (11.4). Until section 11.13's research spike proves a bound, they give only an Approximate sign and never decide inside from outside | Certified(ε); the sign is exact for closed plane-based bodies |
| B-rep | Point cloud | Samples on the surface | Certified |
| Mesh | B-rep | A faceted body, coplanar triangles merged exactly | Exact |
| | | Smooth surfaces fitted: primitives found by RANSAC ([Schnabel et al. 2007](https://doi.org/10.1111/j.1467-8659.2007.01016.x)), a quad layout (Instant Meshes and QuadriFlow, both BSD-style), B-spline fitting ([Eck and Hoppe 1996](https://doi.org/10.1145/237170.237271)) | Approximate, with a deviation map |
| Mesh | SubD | A net fitted so its limit surface follows the mesh. (Using the mesh itself as a net is not a conversion; section 16.1) | Approximate |
| Mesh | Field | Exact winding number for closed plane-based meshes; certified distance | Certified(ε) |
| Mesh | Point cloud | Vertices; samples | Exact; Certified |
| SubD | B-rep | SubD faces (16.1) | Exact |
| | | NURBS faces (16.1) | Exact, Certified or Approximate, face by face |
| SubD | Mesh | Adaptive refinement until each patch's hull is within ε of its flat quad, edges refined once and shared | Certified(ε) |
| SubD | Field | Through its SubD-faced B-rep | Certified(ε) |
| SubD | Point cloud | Samples | Certified |
| Field | Mesh | Certified meshing (16.2) | Certified(ε); Approximate only when asked |
| Field | B-rep | Recognised primitive booleans, through the B-rep boolean | Exact or Certified |
| | | The rest: a mesh, then a faceted body | Certified(ε) |
| | | The rest fitted as NURBS | Approximate |
| Field | SubD | A mesh, then a fitted net | Approximate |
| Field | Point cloud | Interval Newton root-finding along rays | Certified |
| Point cloud | Mesh | Screened Poisson reconstruction ([Kazhdan and Hoppe 2013](https://doi.org/10.1145/2487228.2487237); PoissonRecon is MIT) | Approximate, with a deviation map |
| Point cloud | B-rep, SubD, field | RANSAC primitives and fitting | Approximate, with a deviation map |

- **Randomised and heuristic methods** follow one rule. RANSAC samples at random; Instant Meshes and QuadriFlow use a random generator (pcg32) and parallel loops. The seed is an explicit operation input, the generator is a fixed counter-based one in `geom-num`, and parallel loops use the shared fixed-chunk helper (section 21), so each method joins the thread-count replay test. QuadriFlow's bundled code keeps its notices: LEMON under the Boost licence and a MiniSat-derived SAT solver under the MIT licence ([3rd at 810b7a09](https://github.com/hjwdzh/QuadriFlow/tree/810b7a0967c35b0dc85b4464e3835e26a756c967/3rd)). Its bundled pcg32 is not taken: the counter-based generator above replaces it.
- **The route query.** `geom-convert` also answers "which routes take this body to that representation, and what class would each give", so a person or an AI chooses before anything is computed.
- A point cloud never becomes part of a solid. A conversion from one makes a new, separate body, marked Approximate (section 9).
- Routes that would need trim-free T-splines or watertightCAD reparameterisation are never offered (16.1).
- Section 9 says B-rep operands enter a field boolean as "exact distance plus winding number". Distance to a curved face involves square roots and closest-point solves, so it is Certified, as the table says; the wording is raised for the owner.

## 16.4 Surface division, panels and unrolling

Free-form designs are built from pieces: glass panels, bent metal, timber laths, moulded parts.

**Surface division.** Revit's divided surface lays a grid in the surface's own parameters (u, v), and on an unevenly parameterised surface its panels distort ([09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), section 4). Spare Kit divides in a parameterisation the caller chooses. A parameterisation is a flat map of the surface, and each kind keeps a different property:

| Parameterisation | What it keeps | Method |
| --- | --- | --- |
| Native | The surface's own (u, v) lines, exactly | — |
| Arc length | Equal spacing along one family of lines | Certified arc length |
| Conformal | Angles, as closely as possible | LSCM ([Lévy et al. 2002](https://doi.org/10.1145/566654.566590)) |
| Conformal with control | Angles, with a chosen boundary shape or cone points | BFF ([Sawhney and Crane 2017](https://doi.org/10.1145/3132705); MIT code) |
| As rigid as possible | Lengths and areas, as closely as possible | ARAP ([Liu et al. 2008](https://doi.org/10.1111/j.1467-8659.2008.01290.x)) |

- **Grids** are set by count, by spacing, or by families of levels, planes or curves (certified surface–plane intersections, section 12), each from an anchor (start, centre, end or a named reference) in a stated direction, and may be rotated or offset. Tile patterns (rectangles, triangles, hexagons, rhombi or any supplied tile) are mapped through the grid.
- **The flat map is checked before it is used.** LSCM and ARAP can flip triangles, and LSCM, ARAP and BFF with a free boundary can overlap themselves. So after flattening, the flat coordinates are committed and checked with exact predicates: every triangle positively oriented, and no two triangles overlapping (exact segment-intersection and point-in-triangle tests with `geom-num`'s predicates, inside `geom-mesh` itself; section 14's 2D arrangement is an operation, which a core crate does not use). If the check fails, the operation returns an error with remedies: add a seam along listed curves; use a fixed convex boundary, where Tutte's barycentric map is always one-to-one ([Tutte 1963](https://doi.org/10.1112/plms/s3-13.1.743), [Floater 1997](https://doi.org/10.1016/S0167-8396(96)00031-3)), or BFF with that boundary; or use a method built to be one-to-one ([Smith and Schaefer 2015](https://doi.org/10.1145/2766947)).
- **The map is approximate; the points are not.** Discrete maps are computed on a certified tessellation whose vertices carry their (u, v). A division point is located in the checked flat map with exact 2D predicates, its (u, v) is interpolated, and the surface is evaluated there, so every node lies on the true surface within its evaluation bound.
- **Output:** nodes, division curves on the surface, and tiles, each tile with its size, its angle distortion and the planarity of its corners (an exact orientation test on the committed points). Results are per tile, so one failed tile does not fail ten thousand.
- **Names.** A grid line is named by its anchor and its signed offset from it: the third line from the start stays the third line when the far end moves. Lines that come from a family of planes or curves are named by the `NameId` of the plane or curve. Nodes are named by their two lines. When a count shrinks, the nodes beyond it are reported `Deleted`. A division set by count alone moves every node when its count changes, so its names are marked weak, as section 20.2 marks ordinals. A history property test edits counts, spacings and anchors.

**Panelisation and rationalisation.** Rationalising means replacing a free-form design with pieces that can be made.

- **Planar quads,** for glass. Meshes of nearly planar quads that approximate the design are found by optimisation: conical meshes ([Liu et al. 2006](https://doi.org/10.1145/1141911.1141941)), in which the four faces around each node touch a common cone, which gives node axes and offset meshes at constant distance; and ShapeUp's projection solver ([Bouaziz et al. 2012](https://doi.org/10.1111/j.1467-8659.2012.03171.x); ShapeOp is MPL-2.0 by its file headers and is ported only once its LICENSE file is confirmed (section 29.11), and libigl's `planarize_quad_mesh` is in its MPL-2.0 core).
- **Exactly flat panels.** An optimiser only makes quads nearly flat, so Spare Kit then builds each panel exactly. Each panel has one plane with integer coefficients; each node has one axis line (from the conical mesh, or a chosen direction as an integer vector); and each corner is where the node's axis meets the panel's plane, an implicit point (section 6.3). Every panel is exactly planar in this form. Committing corners to the grid moves them at most √3·q/2 off the plane, half the diagonal of a grid cube; half a step holds only for a plane square to an axis. The exact distance is computed per corner and reported. The offsets between neighbouring panels' corners along each axis, and the kink angle across each joint, are computed and reported. The panelled surface as a whole is Approximate against the design surface, with its deviation measured.
- **Single-curved panels,** bent without stretching: developable strips ([Pottmann et al. 2008](https://doi.org/10.1145/1360612.1360675)), stored as ruled surfaces, with their departure from true developability reported.
- **Timber laths and planks:** strips bounded by geodesics (curves that do not turn sideways within the surface), which flatten to nearly straight strips, so laths bend onto the surface without bending edgewise ([Pottmann et al. 2010](https://doi.org/10.1145/1833349.1778780)).
- **Mould reuse:** grouping panels into a few shared mould types ([Eigensatz et al. 2010](https://doi.org/10.1145/1778765.1778782)) is a mixed discrete optimisation solved by heuristics, and no open method proves the best grouping. Spare Kit certifies each result's deviations and kinks, follows the seed rule above, and claims nothing about optimality.
- Circle and sphere packings and torsion-free support structures ([09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), section 4) are later options. Evolute's tools are commercial; only the published papers are used.

**Frames on a division: curtain walls.** A curtain wall is a division of a wall's face, with a profile along each grid line and a panel in each cell. The kernel knows nothing about walls (section 13's element table): Spare Construct owns the curtain-wall element, its types and its rules. `geom-panel` gives it one operation, frames on a division, which also serves skylights, gridshells and framed facades.

- **Any face.** The division lies on a planar, sloped or curved face and is set out as above. On a plane its grid lines are straight; on a curved face they are curves on the surface.
- **Members.** A profile is swept along each grid line, and along any boundary edge the caller lists (section 13.4). Its up direction is the face normal along the line, so the profile stays square to a curved face. Each member's class follows 13.4's rules.
- **Corner members.** Where two faces of one system meet at an edge, such as a wall corner or a fold between flat facets, the member on that edge takes the bisector of the two face normals as its up direction, so it sits evenly on both faces.
- **Joins at nodes.** Per line family or per node, the caller chooses **through** (the member runs on uncut), **butt** (the member stops against the face of a through member) or **mitre** (two members are cut on their bisecting plane, as 13.4 mitres a path corner). The cuts come from the operation's one arrangement step (section 13.1), not from trimming members afterwards, so a node where three or more members meet is decided by the same exact predicates as a boolean.
- **Infill.** Each cell gets an infill: the cell's region on the face, thickened (section 13.11); an exact flat panel (above), for glass on a curved face; or, where cells repeat, one definition placed as instances (section 19.12), so a thousand identical panels store one shape. A cell can also be left open for the program to fill with an element of its own, such as a door.
- **Names.** Grid lines keep their anchor names (above). A member is named by its line (or boundary edge) and the crossing lines at its two ends, `member(line, from, to)`; a through member's ends are the lines where it stops. An infill is named by the lines around its cell, `infill(cell)`. Moving a line keeps every name. Adding a line splits the members and cells it crosses, removing one merges them, and the history records both (section 20.2). Spare Construct maps these roles to its mullions and panels, so a tag on a mullion survives grid edits.
- **Local change.** Openings through the system use 23.5's local re-cut. Moving one grid line likewise rebuilds only the members and infills it touches, and continuous integration compares the result with a full rebuild.

**Unrolling with strain maps.** Unrolling lays a surface flat so it can be cut from sheet.

- **Developable kinds** (plane, cylinder, cone, linear extrusion) have exact flat maps. The flat pattern's curves are fitted to the map, so the pattern is Certified.
- **Other surfaces** cannot unroll without stretching, because their Gaussian curvature (the product of the two principal curvatures) is not zero. They are flattened with BFF or ARAP, checked as above, and cut along seams where needed ([Sharp and Crane 2018](https://doi.org/10.1145/3197517.3201356); strip unfolding, [Mitani and Suzuki 2004](https://doi.org/10.1145/1186562.1015711)). Making a mesh more developable first ([Stein, Grinspun and Crane 2018](https://doi.org/10.1145/3197517.3201303)) changes the design, so it is a separate operation.
- **The strain map** gives, for every triangle, the two principal stretches of the flattening (how much the most and least stretched directions change length), and from them length strain, area change and angle change, with their extremes and distribution.
- **The caller states the strain the material allows.** Exceeding it is an error that names the regions, with remedies (seams along listed curves, splitting the panel). The flat pattern is a `geom-2d` body, Certified for developable kinds and otherwise Approximate with its strain recorded, and each of its edges is named after the 3D edge it came from (`unrolled(edge)`).

Discrete parameterisation, flattening, flat-map checks and strain maps live in `geom-mesh`, because texture coordinates need them too. Exact flat maps of developable kinds live in `geom-surface`. Division, panels, frames, mould grouping and the unroll operation form a new crate, `geom-panel`, used by Spare Construct's facades and by Spare Studio. It is a kernel operation crate, not a domain toolkit (section 26): it adds methods of its own and composes core crates and other operations. There is one unroll operation, this one. `geom-sheet`'s unfold (section 13.16) adds only what is sheet metal's own, the neutral-fibre rule for bends and refolding, and calls it for every other face. Recording both in section 27 is raised for the owner. The linear solves all of these need (fixed ordering, no parallel floating-point reductions) use the deterministic dense and sparse routines in `geom-num`, the same ones the sketch solver and NURBS fitting use (section 15.6). Any factorisation they need that 15.6 does not list, such as sparse Cholesky for ARAP's fixed matrix, is added to `geom-num` under the same rules.

## 16.5 Class-A surface quality

Class-A surfaces are the visible ones on cars and products, where reflections must flow without a kink or a wobble: joins that are curvature-continuous (G2, often G3), and no inflections that were not designed. Making them is section 13 (blends with chosen continuity) and section 7 (NURBS). Measuring them is here, and every measurement is data a person or an AI can read.

| Continuity | Means | Measured as |
| --- | --- | --- |
| G0 | The surfaces meet | Largest gap |
| G1 | They share a tangent plane | Largest angle between normals |
| G2 | They share curvature in every direction | Largest difference in normal curvature over all directions |
| G3 | Their curvature also changes at the same rate | Largest difference in the cross-boundary derivative of normal curvature |

- **Continuity across edges:** a certified maximum of each measure along the whole edge, from interval and Bernstein bounds over its parameter, refined until the bound is as tight as asked, with the parameter where the maximum occurs. Never a sample of points.
- **Curvature:** principal, Gaussian and mean curvature per face, with certified minimum and maximum, and dense maps for display.
- **Sections:** curvature combs for display, and counts of inflections and curvature extremes along a section curve. On a polynomial piece, the sign changes of its Bernstein coefficients bound the number of roots, and refining isolates them, but only simple roots. A repeated or nearly repeated root, such as curvature touching zero at a flat point, never resolves. So the result separates proven roots from unresolved intervals, each reported as "possible multiple root in [t0, t1]" with bounds on |p| and |p′| there. A double root can be proven with the Krawczyk method on (p, p′). Curves that are not polynomial use intervals the same way.
- **Light lines:** isophotes ([Poeschl 1984](https://doi.org/10.1016/0167-8396(84)90028-1)); reflection lines ([Klass 1980](https://doi.org/10.1016/0010-4485(80)90447-9)), which depend on an eye point; and highlight lines ([Beier and Chen 1994](https://doi.org/10.1016/0010-4485(94)90073-6)), which do not. Each is a curve on the surface, traced with the certified curve machinery of section 12. For each edge the kernel reports the largest kink angle where the lines cross it: across a G1 join they meet but kink, and across a G2 join they continue smoothly. Zebra stripes are the bands between highlight lines of parallel lights. A renderer draws them; the kernel gives the lines and their kinks, so an AI can check a surface without looking at it.
- **Draft:** the draft angle against a pull direction, with the certified minimum per face, for moulds.
- **Gap and flush** between neighbouring panels of separate bodies (a car's door and wing): the certified gap across the joint and the offset along the normal, along its whole length.
- **Deviation maps** against a scan or a reference mesh: certified distance from surface to mesh or point cloud, using section 19's distance queries.
- **Thresholds are inputs.** A car program sets its own tolerances. The kernel reports certified values and pass or fail against the limits it is given, and hard-codes none.
- **SubD at extraordinary vertices** is at best G1, and at boundary, crease and dart vertices not even that (16.1). A Class-A check on a SubD body reports this, and Class-A work converts those regions to NURBS and rebuilds them with G2 blends (section 13).
- Shaded zebra stripes and curvature colours on screen are display only and never feed back.

The maths lives in `geom-surface`. The analysis, which walks a body's faces and edges, uses pcurves and traces curves, lives in a new internal crate, `geom-analysis`; `engine-geometry` only exposes its schemas.

## 16.6 Where it lives

| Crate | Its part of this section | Depends on |
| --- | --- | --- |
| `geom-surface` | `SubdNet`, the local subdivision rules, the SubD limit surface kind (patch tree, Stam evaluation, constant tables, hull bounds, normal cones); exact flat maps of developable kinds; Class-A maths | as section 27 |
| `geom-subd` | The editable SubD body, validation, whole-net refinement, net edits, PCCM patches | num, math, nurbs, surface, **op** |
| `geom-field` | The graph, bytecode, interpreter, intervals, gradients, pruning, field bodies, certified meshing and slicing, lattices, shells, blends, sweeps | num, math, mesh, **op, 2d** |
| `geom-mesh` | The exact meshes fields and subdivision surfaces become; parameterisation, flattening, exact overlap checks and strain maps | num, math, topo, poly |
| `geom-convert` | Every edge of the conversion graph, bodies as fields, primitive recognition, naming of converted faces, the route query | topo, mesh, subd, field, nurbs, **op, check, bool, cloud, spatial, intersect, sweep** |
| `geom-cloud` | Primitive detection and fitting for the point-cloud edges | num, math, spatial |
| `geom-panel` (new) | Division, patterns, panelisation, exact panels, frames on a division, mould grouping, the one unroll operation | **num, topo, op, check, surface, intersect, poly, mesh, 2d, sweep, offset, bool** |
| `geom-analysis` (new, internal) | Continuity, curvature, roots, light lines, draft, gap and flush, deviation | **surface, topo, op, intersect, spatial** |
| `geom-test` | Oracle harnesses run only in continuous integration (OpenSubdiv, Fidget, libfive), with OpenSubdiv, libfive and any other C++ oracle run as separate programs, each in its own container (rule 8 of section 29.20); the exact rational SubD oracle; test corpora | as needed |

Additions to section 27 are in bold. None makes a cycle: no crate added as a dependency here depends, directly or through others, on the crate that now uses it. `geom-mesh` checks flat maps with its own exact tests; 27's accepted table (32.13) also gives it `geom-2d`, a crate of the layer below it, so this is an edge down a layer, not a core crate depending on an operation. Deterministic linear algebra is in its own crate, `geom-linalg` (15.6). Chapter 27's crate table, accepted in 32.13, takes the two new crates and these additions, with each `name` edge added only for naming made a `geom-op` edge, as above. It lists every direct dependency, and where its rows differ from this table, chapter 27's hold.

**Patents and licences first.** The patents and licences this section relies on are cleared by one build piece: a recorded Google Patents search for each adopted method not yet searched (feature-adaptive refinement, Lacewell and Burley, PCCM, Manifold Dual Contouring, interval tape pruning, screened Poisson, Instant Meshes, QuadriFlow, conical meshes, ShapeUp, BFF, generalized winding numbers, mould reuse, Lifted Surfacing), the open flags (Autodesk US 10,635,088 on lattices, nTop's pending filings, the watertightCAD family members, panelisation patents), and counsel's sign-off. A method's search, and an owner's review of anything it finds, come before any piece that uses it enters the Build stage, as section 29.17 and rule 10 of 29.20 require. So lattices and infill, PCCM, feature-adaptive refinement, Manifold Dual Contouring, panelisation and frames, and mould reuse each depend on that piece, not only their Stable tier. Results go in the patent register, `geometry/PATENTS.md` (section 29.17). No piece of this section reaches the Stable tier before counsel's sign-off.
