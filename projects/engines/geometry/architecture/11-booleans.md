# 11. Booleans and the arrangement engine

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Evidence: [01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md) and [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) for the exact plane-based method, [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md) for General Fuse and tolerant modelling, [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md) for winding numbers and the other representations, [07](../../../kit/notes/G01-evidence/07-persistent-naming.md) for naming, [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md) for the limits of curved intersection, [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md) for how BIM booleans fail today, and [10](../../../kit/notes/G01-evidence/10-github-sweep.md), [03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md) and the [vetting report](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md) for other open designs.

A boolean combines shapes: unite (join them), subtract (cut one with others), intersect (keep the overlap). Spare Kit does not have one algorithm per boolean. It has one **arrangement engine**. It first computes the **arrangement** of all the inputs, once: every face of every input is cut wherever it meets another face, so that no two pieces cross, and the space they divide falls into **cells**, connected pieces of space with no face inside them (the regions of section 8). Each cell is labelled with the inputs it lies inside. Every result is then a **selection** from that one arrangement. A union keeps the cells inside any input. A cut keeps the cells inside the wall and outside every opening. A room is a cell inside the storey and outside every wall and slab.

OpenCascade's General Fuse is organised the same way: fuse, cut, common, section, splitter and cells all read one intersection result ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 3; [boolean_operations.md L752-L770](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L752-L770)). Two builders make the arrangement, and one back end turns it into results:

| Part | Used when | Method | Exactness |
| --- | --- | --- | --- |
| Polyhedral builder, `geom-poly` | Every operand is a plane-based body (11.3) | EMBER local arrangements on integer planes | Exact: the arrangement adds no error |
| Curved builder, `geom-bool` | Any operand has a curved face, is a subdivision surface, or is Tolerant | General-Fuse-style pave blocks over the certified intersections of section 12 (11.4) | Certified(ε) |
| Shared back end | Always | Selection by winding numbers (`geom-poly`); rebuild into bodies, names and history, the validity check (`geom-bool`) | Keeps the builder's class |

## 11.1 What goes in and what comes out

**Operands.** Each operand is a valid body (it has passed the checker of section 8.3), with a key and a kind. An operand that fails the checker is refused with `InvalidOperand`.

- The **key** is supplied by the caller and stable: an element's permanent ID, or an `OpPath`. It breaks ties and qualifies names (11.9). Two operands with the same key fail with `DuplicateOperand`.
- A **solid** operand counts towards inside and outside.
- A **cutter** cuts faces and cells but changes nothing about what is inside what. It is usually a sheet (open surfaces) or a wire (curves). Room separation lines and split lines are cutters.
- A **half-space** (everything on one side of a plane), which IFC uses to clip a wall under a roof, enters as the solid bounded by its plane and the frame's range box, with corners at ±(2^40 − 1) steps, so it stays exact. IFC's boxed half-space gives the same result as the plain one; its box is only a search aid ([IfcBoxedHalfSpace](https://github.com/buildingSMART/IFC4.3.x-development/blob/5d08761e63557bd3ded140139eede665ec31511c/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcBoxedHalfSpace.md)). Its polygonal bounded half-space, used for beam copes and cutbacks ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section B), enters as the half-space intersected with the prism its boundary sweeps out ([IfcPolygonalBoundedHalfSpace](https://github.com/buildingSMART/IFC4.3.x-development/blob/5d08761e63557bd3ded140139eede665ec31511c/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcPolygonalBoundedHalfSpace.md)). The prism is flat unless its boundary has arcs.
- Operands on different grid steps are first brought to the finest one, exactly (section 6.1). Operands that then do not fit one frame fail with `FrameRange`.

**Selections.**

| Selection | Keeps | Typical uses |
| --- | --- | --- |
| Unite | Cells inside any solid operand | Joining wall layers, massing, assemblies |
| Subtract | Cells inside the target and outside every tool | Openings, drilled holes, pockets, cut and fill against a terrain solid |
| Intersect | Cells inside every solid operand | Overlap volumes, trimming to a boundary |
| Where | Cells passing a test on winding numbers, such as "inside at least two" | Clash volumes, layered regions |
| General fuse | Every cell and every face, shared faces once | Parts that must share faces, analysis meshes |
| Imprint | The target, its faces split where the others meet it; shape unchanged | Finish regions, split lines on a slab, contact areas |
| Split | The target, cut into separate bodies by the others | A wall split at a level, a slab cut along a line |
| Cells | Chosen cells, as bodies that share faces, named by seeds | Rooms, spaces, zones |
| Keep | The pieces of a sheet or wire target that pass a winding test on the solids | A facade surface clipped to a site volume; rebar or pipe centrelines clipped to a section box |
| Trim | A sheet or wire target, cut by the others, keeping the pieces chosen by seeds | Class-A surface trimming, mould parting surfaces, trimming linework |
| Section | Only the curves where operands meet, as a wire body | Plan cuts, slicing for 3D printing |

The cut regions a drawing hatches come from Keep with a planar sheet as the target. Section 18 builds drawings from them.

```rust
pub struct Operand {
    key: OperandKey,    // stable, caller-supplied: an element's permanent ID or an OpPath
    body: BodyRef,
    kind: OperandKind,  // Solid or Cutter
}

pub enum Selection {
    Unite,
    Subtract { target: OperandKey },                  // the other solids are tools
    Intersect,
    Where(WindingTest),                               // a small expression: Inside(key), AtLeast(n), And, Or, Not
    GeneralFuse,
    Imprint { target: OperandKey },
    Split { target: OperandKey },
    Cells { test: WindingTest, seeds: Vec<CellSeed> },
    Keep { target: OperandKey, test: WindingTest },   // a sheet or wire target
    Trim { target: OperandKey, seeds: Vec<PieceSeed> },
    Section,
}

pub struct BooleanInput {
    operands: Vec<Operand>,
    select: Vec<Selection>,          // one arrangement, several results
    merge: Merge,                    // RemoveRedundant (default) or KeepSeams: 11.5
    manifold: ManifoldRule,          // AllowGeneral (default) or Require: 11.5
    batch: BatchRule,                // Atomic (default) or SkipFailedTools: 11.6
    require: Option<Exactness>,      // e.g. Exact: fail rather than return Certified
    reuse: Option<ArrangementCache>, // 11.6: may speed a re-cut, never changes the result
}
```

Several selections cost one arrangement: a drawing asks for the cut body and its section together, a storey for its cut walls and its rooms. Region booleans for 2D drafting follow the same compute-once, select-many design in `geom-2d` (section 14).

## 11.2 Winding numbers decide every piece

The **winding number** of a closed surface at a point counts how many times the surface wraps around it: 1 inside a solid, 0 outside, 2 inside two overlapping copies. For closed input it is found exactly by counting **signed** crossings along a path from a point whose winding number is known: each crossing adds or subtracts 1, depending on which way the face points. Counting crossings without signs (ray parity) breaks where solids overlap; signed counting does not. The **generalised winding number** of Jacobson, Kavan and Sorkine-Hornung ([2013](https://doi.org/10.1145/2461912.2461916)) extends the idea to surfaces with gaps and holes, which matters for curved bodies (11.4).

**Winding vectors.** Each point that is not on a face has a **winding vector**: one winding number per entry. Each piece of face has a vector on its front and one on its back. Vectors are **sparse**: a sorted list of (entry, count) pairs for the entries that are not zero, so the outside is the empty list. In a building a point is rarely inside more than two or three elements, so the lists stay short however many operands there are. Memory grows with the number of pieces times that depth, not times the number of operands.

**Entries are grouped by role.** An entry is one operand, or a group of operands whose numbers are summed. EMBER does this for its "inside at least k of these spheres" example ([Trettner, Nehring-Wirxel and Kobbelt 2022](https://doi.org/10.1145/3528223.3530181), section 5.4 and Fig. 21). Every solid operand is a valid solid, so its winding number is never negative, and "outside every tool" means "the tools' sum is zero". A subtraction of 300 openings therefore uses two entries, host and tools. Cells use two, envelope and obstacles. Where and clash queries, which must say which elements overlap, keep one entry per operand. EMBER's authors note that very many entries would need further optimisation (their section 6); sparse lists and grouping are that optimisation. Each piece also records the face it came from, so identity never depends on the vector.

**Selection.** A selection is an **indicator**: a test that says in or out for a vector. A piece is kept when the indicator differs on its two sides, oriented so that the material is behind it. Pieces with the same answer on both sides are dropped unless imprint, split, general fuse, cells or trim ask for them. This is EMBER's formulation (its section 3.4), which follows Zhou, Grinspun, Zorin and Jacobson's mesh arrangements ([2016](https://doi.org/10.1145/2897824.2925901)). A test that says "in" for the empty vector would give an infinite body. The engine refuses it with `UnboundedResult`, as hypermesh, the exact mesh boolean in the csgrs project, does ([vetting](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md); [winding.rs L36-L150](https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/src/winding.rs#L36-L150)).

**Cells are connected pieces of space, labelled by vectors.** Two rooms on one storey have the same vector (inside the envelope, outside every wall), and they are still two cells, because no path joins them without crossing a face. The rebuild (11.3) finds cells by connectivity. Vectors only label them.

**Cutters carry a zero transition.** In EMBER each polygon carries a **transition**: how crossing it changes the vector. A cutter's transition is zero. Its polygons still cut every polygon they cross and are classified like any other, but crossing them changes nothing, so the front and back of a cutter piece have the same vector, and the cells on its two sides are told apart by connectivity alone. EMBER does not have cutters: this is our extension. Its proof obligation, written in the module, is that a vector traced along any path is the same with or without zero-transition polygons, so EMBER's guarantees for every other polygon still hold. Tests check it.

**Sheet and wire targets** have no inside of their own. Keep and Trim cut them by the other operands, then choose each piece by the other operands' entries at that piece (Keep) or by connectivity to a seed (Trim). A piece is never classified by its own entry.

**On the flat path every winding number is exact**, from exact predicates along exact paths (11.3). On the curved path each is a proven integer under the conditions in 11.4.

## 11.3 The exact polyhedral path

**When it runs.** Every operand is a **plane-based body** (section 6.3): every face lies exactly on its plane, every vertex is the meeting of three of the body's planes, and every plane is within the section 6.2 budget. At 40-bit coordinates a plane's normal needs at most 83 bits and its offset at most 125 ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 4). The plane type checks this when it is built, so no plane outside the budget can reach a boolean. Walls on straight paths, slabs, flat roofs, stairs, faceted IFC solids and terrain solids from triangulated ground models are all plane-based. That is most of a building.

- The operands' class does not decide the path. A plane-based body that is Certified(d), because it was rounded or snapped, gives a Certified(d) result (section 9: the least exact input wins). The arrangement itself adds nothing.
- A Tolerant body (section 6.5) does not qualify, flat or not, because its faces do not meet exactly. It takes the curved path.
- Flat faces that are not exactly planar are split at import (section 24), never inside a boolean.

**The method.** EMBER (Exact Mesh Booleans via Efficient and Robust local arrangements), by Trettner, Nehring-Wirxel and Kobbelt ([ACM TOG 2022](https://doi.org/10.1145/3528223.3530181)), on the integer plane arithmetic of Nehring-Wirxel, Trettner and Kobbelt ([2021](https://doi.org/10.1016/j.cad.2021.103015)). It is the fastest published exact method. Its benchmark used 1,000 pairs of models from Thingi10K, a public set of 10,000 3D-printing models, with 1,000 to 100,000 faces each. Its parallel implementation took a geometric mean of 1.6 ms per boolean, at about 20,000 input faces, on an 8-core i9-9900K. It sped up 5.5 times on 8 cores. Inputs had 26-bit coordinates, in 256-bit arithmetic (paper section 5; [01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md), section 5). We found no public code (unverified; a GitHub search on 2026-10-21 found none), so it is written clean-room from the papers. Manifold's Apache-2.0 harness encodes the paper's test cases, and we reuse it ([ember_tests](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/extras/ember_tests/README.md)).

**How faces enter.** EMBER takes convex polygons, each a supporting plane plus one bounding plane per edge. Our faces are often not convex: a wall face with a window has a hole. EMBER's own section 3.2 explains why such a face cannot simply be re-triangulated: a segment between two three-plane points generally cannot be represented within the bit budget. So:

1. Every edge of a plane-based body keeps the two planes that define its line. These are its two faces' planes or, where two faces share a plane (a seam kept by `KeepSeams`, an imprinted line), the face's plane and the plane of the face that made the edge.
2. A face that may meet another operand (by a broad phase on bounding boxes) is split into convex pieces only along its own edges' planes: a binary space partition of the face by its own edges, called an **autopartition**. Every piece edge then lies on an existing plane and every new corner is the meeting of three existing planes, so the budget holds. Edges are taken in name order, so the split is deterministic.
3. Faces that can meet nothing pass through untouched, without conversion.

The convex split is internal: the rebuild merges the pieces back into faces.

**Five steps:**

1. **Adaptive subdivision.** The bounding box of the operands is split in two along an axis, again and again, until each box holds few polygons. Polygons crossing a split are clipped exactly. Each box carries a **reference point** with a known vector. It is moved into each new box by tracing a short path and adding up the transitions of the faces it crosses, so every box can be solved on its own.
2. **Local cuts.** In each small box, every polygon is cut by every polygon that crosses it, using a small **binary space partition** per polygon: a tree that splits the polygon by planes into convex pieces. Where polygons on one plane overlap, the overlap is emitted once (11.5). Afterwards no piece crosses another.
3. **Classification.** Each piece gets its front and back vectors by tracing a path of at most three straight segments from the reference point. Every segment lies on input planes, so the path itself is exact.
4. **Early outs.** Whole boxes are dropped when the indicator proves they cannot contribute: in a cut, a box outside the wall, or inside a tool and holding no other face. These rules are derived mechanically from the selection, including the pieces it keeps on purpose (the paper's section 4.5.2). Every operand is a checked body, so each carries EMBER's no-self-intersection flag, and single-shell bodies its no-nested-parts flag as well. These flags skip most leaf work away from other operands (its section 4.5.1).
5. **Rebuild** into a body, in `geom-bool` (below).

**Where we differ from the paper:**

- **Bit budget.** EMBER takes 26-bit coordinates and stays within 256 bits. Ours are 40 bits (section 6.1). A three-plane point's coordinates need 294 bits (`I320`), and the deepest test, that point against a fourth plane, needs 379 bits (`I384`) ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 4). The engine also orders points along an edge, edges around a vertex within a face, and faces around an edge. By our derivation each reduces to the classify test or to the sign of a 3×3 determinant of plane normals (about 252 bits), so all fit within 384 bits. Each module carries its derivation and an extreme-value test (section 6.2).
- **Ties.** Where coplanar polygons overlap, the paper lets the lowest input index emit the overlap (its section 4.3). We use the lowest (operand key, name), so neither operand order nor copies with equal names can change the result.
- **No randomness.** The paper finds a point inside a thin polygon with randomised offsets (its section 4.4). We use a fixed sequence.
- **No recursion.** The box tree, each polygon's partition (defined recursively in the paper's section 4.3), the broad-phase tree and the walks around edges and loops all use explicit stacks, so their depth never threatens a WebAssembly stack (principle 13). A depth test runs on the paper's worst cases: 10 by 10 crossing beams and 20 cubes sharing their top and bottom planes (its Figs. 24 and 25).
- **A real body.** The paper outputs convex polygons with T-junctions (a vertex in the middle of a neighbour's edge) and leaves topology to a later pass (its section 6). The rebuild makes a body by canonical rules:
  - pieces on the same plane, from the same sources, with the same vectors on each side, merge into maximal faces, so the cuts of the subdivision and the convex split disappear;
  - edges are maximal runs between the same faces, each keeping its two defining planes;
  - two vertices are the same when one lies on all three planes of the other, an exact test, and a vertex on more than three planes is defined by its lowest valid triple by (operand key, name);
  - faces join into shells by their order around each edge, and shells into cells by connectivity and nesting, using exact point classification;
  - arena indices are assigned in name order.

  So the bytes of a result do not depend on how the boxes were split or how many threads ran.

**It never rounds under iteration.** Every face of the result lies on an input plane, every edge on two, every vertex on three, and every edge keeps its planes. The result has the same form and budget as the input and goes straight back in. Cutting a wall 300 times, one cut after another, rounds nothing and drifts nothing. A test does exactly that and checks every plane against the budget after each cut. This path uses no tolerance at all.

## 11.4 The certified curved path

**When it runs.** Any operand has a curved face (round columns, walls on arcs, pipes and ducts, drilled holes, car body panels, blends), is a subdivision surface, or is Tolerant.

**The method.** OpenCascade's General Fuse, studied for method only (it is LGPL) and described in our own words ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 3; [boolean_operations.md L299-L387](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L299-L387)), rebuilt on exact and certified decisions:

1. **Broad phase.** A tree of certified boxes around every vertex, edge and face finds the pairs that may touch. Pairs are listed in name order.
2. **Interferences, lowest dimension first:** vertex with vertex, vertex with edge, vertex with face, edge with edge, edge with face, face with face, each computed by section 12. A result found at a lower dimension is reused above it, so a section curve through an existing corner ends at that corner instead of making a second one nearby. Every merge (below) is decided in this stage, and the merged entity replaces its members in every later decision.
3. **Paves and blocks.** A **pave** is a point on an edge where something touches it, with its parameter. A **pave block** is the piece of edge between two neighbouring paves. A **common block** groups pave blocks of different edges that lie on the same curve, such as collinear wall edges or two cylinders sharing a line.
4. **Face information.** For each face the engine records three groups of pave blocks and vertices, as OpenCascade's FaceInfo does: those on its boundary (On), those of other operands lying inside it (In), and those made by sections across it.
5. **Splitting faces.** Each face is cut along its In and section edges in its own parameter space. Loops are traced by ordering the edges around each vertex. Flat faces use exact orientation predicates. Curved faces use certified tangent directions, then curvature, then derivatives up to third order, matching the G3 blends the kernel builds (section 10.2). If the order is still not proven, the operation fails with `UndecidedOrder`. Floating-point angles are never compared. OpenCascade traces loops by the smallest floating-point angle, and its own documentation shows such decisions failing when a model is scaled ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), sections 2 and 3).
6. **Classification** (below).
7. **The shared back end** selects, rebuilds, names and checks, as in 11.3.

For all-flat input the curved builder must give exactly the polyhedral builder's result, bit for bit. A differential test holds it to that.

**What the builder stores.** Exact and certified geometry live side by side, and every entity says which it is:

```rust
enum PointGeom {
    Grid(GridPoint),            // a committed grid point: exact
    Planes([PlaneIx; 3]),       // the meeting of three planes: exact, implicit
    Certified { at: GridPoint, on: SortedVec<CarrierIx>, within: Bound }, // proven within `within` of where its carriers meet
}
enum CurveGeom {
    Line([PlaneIx; 2]),         // the meeting of two planes: exact
    Kind(CurveIx),              // a circle, ellipse or other kind, stored as itself (section 7)
    Meeting { of: [SurfaceIx; 2], fit: Option<FitIx> }, // a procedural intersection curve, with an optional cached fit
}
```

A **carrier** is the surface a face lies on, or the curve an edge lies on. A boolean never moves a carrier: every face of the result lies on a carrier of an operand.

**Every decision is about carriers: exact first, certified next.**

- Decisions between flat faces use the polyhedral predicates, so the flat parts of a mixed body stay exact.
- These decisions on grid data are exact integer tests. The bit counts are by our derivation, and each module carries its derivation and an extreme-value test:

  | Exact decision (grid points; axes through grid points; radii in whole steps) | Bits at 40-bit coordinates | Type |
  | --- | --- | --- |
  | A grid point against a plane or a sphere | about 126 | `i128` |
  | A grid point against a cylinder | about 169 | `I256` |
  | A grid point against a cone given by grid points, or a torus | about 253 | `I256` |
  | A plane against a sphere or a cylinder: crossing, tangent or apart | about 253 | `I256` |
  | Two planes, two spheres or two cylinders coinciding | about 208 | `I256` |

- Everything else is certified, including three-plane points against quadrics (their integers pass 384 bits) and every point built from a curved carrier, such as where a line meets a cylinder. Section 12 decides whether some of those get exact algebraic predicates later; the boolean uses whatever section 12 provides and never assumes more.

**Certified decisions and the one contact radius.** A certified decision computes an **enclosure**, an interval proven to contain the true value, from the carriers' definitions, and refines it up to a fixed limit. There are three outcomes:

- The enclosure excludes zero. The question is decided: apart, or crossing on a proven side.
- It still contains zero at the limit, and its upper bound is strictly below ε_fit. The entities **touch** there and become one. ε_fit is the **contact radius**: the ladder's fixed value, never stored per entity and never grown.
- Otherwise the operation fails with `NearContact`, with the enclosure as evidence and a remedy (11.11). It never guesses.

Entities that can be proven apart stay apart, however close. As on the flat path, a thin piece between them is real.

**Merges are checked against carriers.** When entities touch, one representative takes their place: the most exact member (an exact vertex, if the cluster has one), otherwise the grid point nearest the certified meeting, chosen by a fixed rule. It is accepted only if it is proven within the result's remaining bound (see "How bounds add up" below) of every carrier of every member, computed from the carriers' definitions. If it is not, the operation fails with `CoincidenceChain`. Because the representative is measured against carriers, which never move, and never against an earlier representative, repeated operations cannot walk a vertex away. Parasolid likewise requires coincidence to be transitive for a body to be valid ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 2). A test runs 50 successive near-contact booleans and then certifies every vertex against all its carriers.

**Common blocks.** Two curves of the same analytic kind on grid data, such as two circles or two lines, are compared exactly. Curves of different kinds, or NURBS, are compared by the certified test above. A circle stored as a circle and a NURBS close to it form a common block only by the contact rule. OpenCascade never lets curves of different analytic kinds form common blocks ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 3).

**Classification.** Faces connected across edges that are not section edges share one vector with respect to the other operands, so each connected group needs one classification. On a Certified body, faces meet only to within its bound, so right at a seam the true winding number may not be an integer. The engine therefore classifies away from seams:

1. It takes a small certified box on a face of the group, proven clear of every face of the other operands. Every point in the box has the same winding number, and it is an integer.
2. It picks a grid point in that box and counts signed crossings along a ray, trying directions from a fixed sequence. A ray is used only if it is proven to stay further than the other operand's bound from every edge of that operand, so it cannot slip through a gap at a seam, and if it meets no face tangentially. Otherwise the next direction is tried.
3. Crossings are found exactly on flat faces, and on curved faces by certified ray-surface intersection with Bernstein subdivision (section 12).
4. If no clear box or no usable ray is found within a fixed limit, the operation fails with `NearContact`.

The answer is a proven integer. A generalised winding number for trimmed NURBS would avoid the ray. Spainhour, Gunderman and Weiss give a fast method ([doi](https://doi.org/10.1145/3797957)), implemented in LLNL Axom (BSD-3, [winding_number.hpp L751-L930](https://github.com/LLNL/axom/blob/95d267a301789fa94d125cb936f8ba0908ebd57f/src/axom/primal/operators/winding_number.hpp#L751-L930)). Axom computes it by quadrature with relative tolerances and rounds to the nearest integer, with no proven bound. Making it certified is research (11.13). Until then it is not used for decisions.

**Subdivision operands.** A subdivision surface takes part through its limit surface, a surface kind of its own (section 7), evaluated by Stam's exact method with certified bounds (section 16). The result is Certified, as section 9 says the limit surface is. A boolean never converts it to NURBS, which would make it Approximate near extraordinary points. That conversion is an explicit operation the caller may run first. Where a decision near an extraordinary point cannot be certified, the boolean fails with a remedy.

**Tolerant operands.** Imported bodies still marked Tolerant (section 6.5) take part within their stored edge tolerances, used as fixed tubes. Tolerant edges that pass into the result keep their stored tolerances. Every new entity is certified to the ladder and gets no tolerance of its own. If the result would need more than an existing tolerance, the operation fails with `ToleranceExceeded` and offers healing.

**How bounds add up.** Section 9's rule applies: a result's bound is the sum of its operands' bounds and the operation's own term. The engine keeps its own term small and never builds on an earlier approximation:

- faces are never moved, so they add nothing;
- new edges are procedural intersection curves defined by their two carriers (section 7), so they add nothing. Their cached fit only speeds decisions, and a decision taken on it carries the fit's bound in its own certificate;
- new vertices are computed from the carriers' definitions, never from an earlier fit or vertex, and committed to the grid. Each adds commit rounding (q/2, which is ε_fit/128) plus the width of its enclosure;
- a merge adds its representative's proven distance from its carriers.

A typical boolean therefore adds about ε_fit/128. If the sum would pass ε_fit, the operation does what section 9 requires: it fails with `ExactnessWouldDrop` and offers the larger bound as an explicit choice. Open question 1 asks whether a direct certification of every entity against its carriers may replace the summed bound. That would keep a part cut 200 times at its measured bound.

**Unlike OpenCascade**, a face that fits into no shell is an internal error, not a warning. Coincidence is always checked, never taken on the caller's word as OpenCascade's glue option does. No tolerance ever grows ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), sections 2 and 3).

## 11.5 Coplanar, coincident and touching faces

Shared faces are the normal case in buildings: walls meet walls, walls stand on slabs, openings line up with faces. Floating-point kernels struggle here. Manifold redesigned its tie rules three times in ten months ([01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md), section 1). IfcOpenShell retries OpenCascade with a fuzzy tolerance ten times larger, up to 10,000 times the model precision, whenever a result looks doubtful ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section A3). Spare Kit handles shared faces explicitly and exactly, and never perturbs them: a perturbation would turn a shared face into an arbitrary tiny gap or overlap (section 6.2).

- **Found exactly.** Two planes coincide when their coefficients are proportional, and two cylinders when their axes and radii agree. Both are integer tests on grid data (11.4).
- **One piece, every side counted.** Where faces of different operands overlap on one plane, the overlap becomes a single piece. Its front and back vectors are traced on each side, so they count every coincident face, whatever its orientation. Its name and its carrier come from the same source: the coincident face facing the same way as the result with the lowest (operand key, name).

Where wall A's end face lies against wall B's side face, facing the opposite way:

| Selection | Result at the shared face |
| --- | --- |
| Unite | Inside on both sides, so it disappears: one solid |
| Subtract A − B | A is unchanged there; B's outline can be imprinted on request |
| Intersect | An empty body, with a `TouchingOnly` diagnostic |
| General fuse, cells | Kept once, as an internal face between A's cell and B's cell |

- **Flush faces merge.** Faces from different operands that end up side by side on one plane, facing the same way, such as two walls flush on the outside, become one face with a merged name. `Merge::KeepSeams` keeps the seam when a program needs both names.
- **Touching at an edge or a corner.** Two boxes that share only an edge unite into a truly non-manifold shape. The engine returns a General body (section 8.1) rather than nudging them apart. `ManifoldRule::Require` makes this an error that names the edges.
- **Nearly coincident.** On the exact path, faces 0.003 mm apart are different faces, and the thin piece between them is real. It gets a `Sliver` diagnostic when it is thinner than the short-edge warning (3,200q). A boolean never snaps faces together. Imported gaps are healed once, at import (section 6.5).
- **Closing near contacts is a separate operation.** A caller who wants nearly coincident faces treated as one runs `close_near_contacts(d)` first. It is a repair operation in `geom-bool` (section 10.2). For each pair of faces, edges or vertices of different operands within d, it adopts one carrier for both: that of the same-facing source with the lowest key and name. It recomputes every affected vertex, exactly on plane-based bodies, re-runs the checker, and reports how far each entity actually moved. A vertex can move much further than d where a neighbouring plane is nearly parallel, so the movement is measured, not assumed. The result is Certified(largest actual movement), or the operation fails. It is explicit, logged, and capped by the operation's tolerance ceiling (section 6.5). The default cap is open question 7.
- **Curved coincidence.**
  - Coincident analytic surfaces are found exactly (11.4).
  - Two NURBS surfaces are first compared exactly. The engine checks for reversed or swapped parameter directions and for an affine change of parameters. It then raises both to a common degree, refines them to common knots, and compares control points and weights in exact rational arithmetic. Cutting the larger surface at the smaller one's parameter bounds first also finds a patch that is a trimmed piece of another, as in core and cavity, matching panels and sheet-metal contact. This test is rare and slow, and runs only when cheaper tests cannot tell the faces apart.
  - Otherwise the engine bounds the distance between the two faces in both directions over their overlap, by subdivision with Bernstein bounds (section 12). If they are proven within the contact radius, they coincide, and one carrier is adopted as above. If they are proven apart somewhere, they do not coincide. If neither can be proven, the operation fails with `NearCoincident`, with the region and the bound as evidence.

## 11.6 Batch cutting

Facades have hundreds of openings, and circuit boards thousands of drilled holes. The engine cuts them in one call and one arrangement.

- **One arrangement for every tool**, with the tools summed into one entry (11.2). Tools that overlap or share faces, such as two openings sharing a jamb, need no union first.
- **Flat hosts and tools** take the polyhedral path. Faces no tool can reach pass through untouched, names and all (11.3), and early outs drop every box without a tool. The cost is linear in the host's faces, for the pass-through and the checks, plus local work around each tool.
- **Curved tools** (holes, bosses, round ducts) take the curved path. The broad phase pairs only host with tool, and tool with tool where both meet the host. Then one General-Fuse pass runs, with the same summed entry. Planes meeting cylinders on grid data are decided exactly, so a slab drilled with 10,000 holes keeps exact decisions.
- **Every tool is accounted for.** The result lists, for each tool, the faces it cut, or `ToolMissesHost` if it touched nothing. IfcOpenShell drops cutters that miss the bounding box, only touch, or look too narrow, and missing wall subtractions are among its open issues ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section A3). Here a subtraction cannot go missing silently.
- **Atomic by default.** If a tool cannot be cut (possible only on the curved path; an invalid tool is refused earlier as `InvalidOperand`), the call fails and names it. `BatchRule::SkipFailedTools` cuts the rest and returns the failures beside the result. Which tools are skipped depends only on the inputs.
- **Many hosts, one call:** one result per host, in input order. Hosts run in parallel.
- **Machining and boards.** Stock-removal simulation over thousands of tool positions is not done as thousands of B-rep subtractions. It runs on the exact mesh boolean, whose paper benchmarks exactly this iterated milling case (EMBER Fig. 14, after Nehring-Wirxel 2021), or on a field or dexel model (sections 16 and 19). Either way it cuts the swept volumes of the tool's moves (section 10.2). A B-rep of the finished part is made by an explicit cut when it is wanted. Board layers are usually built by 2D region booleans per layer and then extruded (`geom-2d`, section 14, and the board toolkit, section 26).
- **When one door moves.** Programs keep each host's uncut body ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section B), so a moved door re-cuts only its host. To make the re-cut fast, the result can come with an **arrangement cache**: an immutable value holding the host's subdivision and convex split, keyed by a content hash of the host and its tools. The caller may pass it back in `reuse`. The engine then re-solves only the boxes the moved tool touches, which EMBER names as future work (its section 5.1). The cache changes speed, never the result: a property test requires the incremental result to equal a full recompute, bit for bit. A cache that does not match its inputs is ignored. It is never stored as model state.

## 11.7 Cells, rooms and non-manifold results

General fuse and cells keep the whole arrangement: a General body (section 8.1) whose regions are the cells, each labelled with its vector, and whose shared faces appear once, with a cell on each side.

- **Radial order.** Where more than two faces meet at an edge, their fins are kept in order around it. On flat faces the order comes from the sign of a determinant of plane normals, exactly. On curved faces it comes from certified tangent planes, then curvature, up to third order, else `UndecidedOrder`. This extends the radial sorting of Lévy's exact mesh CSG ([arXiv 2405.12949](https://arxiv.org/abs/2405.12949); geogram, BSD-3, [mesh_surface_intersection.h](https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/mesh/mesh_surface_intersection.h#L68-L73)) to curved faces.
- **Regularised by default.** Unite, subtract and intersect drop faces, edges and points that bound no volume. General fuse, cells, imprint and split keep them, since that is their purpose. A cutter that only partly enters a target cannot split it. Split then returns the target whole, with a `CutterDoesNotSeparate` diagnostic. General fuse keeps the part of the cutter inside the target as an internal face.
- **Rooms and spaces.** A room is a cell inside the storey's envelope and outside every wall, slab and column.
  - The program passes one **seed** per room: a point and a permanent key, like the room point people place in BIM tools. The kernel returns the cell containing each seed, named by its key, so a room keeps its identity when a wall moves.
  - Room separation lines enter as cutters (sheets standing on the line). A room's upper and lower limits, where they are not slabs, enter as half-spaces.
  - A seed on a face fails with `SeedOnBoundary`, and a seed in no cell with `SeedOutside`.
  - **Leaks.** Two seeds in one cell give one merged cell and a `SeedsShareCell` diagnostic that says where the rooms join. Over the arrangement's convex pieces, the engine finds the path between the two seeds whose narrowest point is widest, and reports that narrowest passage, with its location and width. A 0.003 mm gap where two walls fail to meet shows up at once, in its place. The remedies are to extend the join or add a separator. The width is a measurement for people, not a decision, and is labelled Approximate.
  - Each face of a room records the element face it lies on and what is on the other side: another room, an element, or the outside. These are IFC's second-level space boundaries, type 2a (a space on the other side) and type 2b (an element on the other side) ([IfcRelSpaceBoundary2ndLevel](https://github.com/buildingSMART/IFC4.3.x-development/blob/5d08761e63557bd3ded140139eede665ec31511c/docs/schemas/core/IfcProductExtension/Entities/IfcRelSpaceBoundary2ndLevel.md)), which energy analysis needs. They come straight from cell adjacency. Topologic's CellComplex shows the value of this design. Topologic is AGPL, so we take the concept only, from its published papers ([10](../../../kit/notes/G01-evidence/10-github-sweep.md)).
- **Clash volumes.** `Where(AtLeast(2))` over one entry per element returns exactly the volumes where two or more elements overlap, each labelled with the elements involved. Hard clashes come back as solids, not only as touching pairs (section 19).
- **Names of cells.** A seeded cell is `Created { op, role: seed(key), from: [] }`. An unseeded cell from general fuse is `Created { op, role: cell, from: <the names of its bounding faces> }`, with a weak ordinal only if two still collide.

## 11.8 One boolean across representations

This is section 9's rule, applied by one entry point in `geom-combine`, an internal crate this chapter proposes (open question 2). It calls each representation's crate. `engine-geometry` only exposes its schema.

| Operands | What runs | Result class |
| --- | --- | --- |
| All plane-based B-rep | Polyhedral path | The least exact input; the path adds nothing |
| B-rep, any curved, subdivision or Tolerant | Curved path | Certified(ε), by 11.4 |
| Any exact mesh | The exact mesh boolean in `geom-mesh`, on the same `geom-poly` arrangement and indicators; curved B-rep operands are tessellated with a certified bound (section 19) | Plane-based operands as above; curved ones Certified(their bound plus the tessellation bound) |
| Any field | The field boolean in `geom-field`; other operands enter as exact distance plus winding number (section 16) | Certified when meshed |
| Any point cloud | Refused with `InvalidOperand`: clouds are reference only | — |

- The tessellation bound defaults to whatever keeps the result within ε_fit. A coarser bound must be asked for, and is recorded.
- Nothing falls back silently. A failed B-rep boolean offers the mesh or field path as a remedy, and the caller, person or AI, must choose it.
- On request, a closed exact mesh can first be lifted exactly to a plane-based body, so the result stays a B-rep (open question 4).

**Previews.** When `OpContext` asks for a preview precision (section 10.1), as when a door is dragged, curved operands are evaluated at a coarser certified bound or on the mesh path. The result carries its real class (Certified at the coarser bound), is marked as a preview, and is never stored or logged as model geometry. Plane-based operands need no preview, because the exact path is already the fast one.

## 11.9 History and names

Every output entity gets a name and a history record under section 20.

**Names from operands.** A target (of subtract, imprint, split, keep or trim) keeps its names. A tool's faces that enter the result, such as the reveals of an opening, are named by the tool's key: `Created { op, role: tool(key), from: [their own name] }`. Two doors of the same type therefore give different reveal names, and adding a third door renames nothing. In unite, intersect, where and general fuse, faces keep their names. If two operands bring the same name (copies, arrays, a body united with a moved copy of itself), those faces are qualified by their operand's key in the same way, with a `NamesQualified` diagnostic (open question 9).

| What happened to an input face | History | Name |
| --- | --- | --- |
| Passed through untouched | Unchanged | Its own name, or its tool-qualified name |
| Trimmed, or split with one piece kept | Modified | The same (one to one) |
| Split into several kept pieces | Split, with the cutting faces and side | `Split { of, piece: ByTools { tools, side } }` |
| Merged with flush faces | Merged | `Merged { of }` |
| Shares an overlap piece with coincident faces | Modified, from every coincident face | From the coincident faces facing the same way as the result: one gives its own name, several give `Merged`. Faces facing the other way appear in the history, not the name |
| Not in the result | Deleted | — |
| A new section edge or corner | Generated, from the faces that meet there | `Bounded` by those faces |

This follows OpenCascade's rule for booleans: Generated only for new intersection edges and vertices, and Modified for overlaps ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 3).

- **Several edges between the same faces.** `Bounded` alone cannot tell them apart, and a corrugated sheet cut by a plane makes hundreds. Adding the faces that end each edge would, but it needs a change to section 20.2's type: `Bounded { kind, by, ends, ord: u16 }` (open question 3). Until an owner accepts it, such edges take section 20.2's weak ordinal, ordered along the edge in a canonical direction.
- **Split faces keep their carrier.** When a door cut splits a wall's exterior face, a carrier reference to that face (section 20.4) still resolves, because every piece lies on the same plane.
- **Tool faces become reveals.** A door whose void names its faces `jamb.left`, `head` and `sill` gives stable references to the reveals, through the tool-qualified names above.
- **Pieces of a split body** are named `ByTools`, by the cutting faces and the side. **Cells** are named by their seeds (11.7).
- Everything is ordered by (operand key, name), never by memory address, hash order or thread timing. Section 20.3's history rules are property-tested on every selection.

## 11.10 The validity postcondition

Nothing is returned until it passes:

1. **The checker of section 8.3**: the cheap groups always, the full groups in debug and CI builds. For curved results, group 2 certifies every vertex and edge against its carriers at the result's bound.
2. **The arrangement certificate**, cheap enough to run on every result:
   - every edge's fins pair up: two opposite fins in a solid, and in a General body fins alternating cell sides in radial order;
   - every kept face separates two cells the indicator tells apart, unless the selection keeps it on purpose;
   - each cell's label agrees with every face bounding it;
   - no edge has zero length and no face zero area;
   - on plane-based results, every plane is within budget;
   - every output entity has an origin, and every input has descendants or is listed as deleted.

   This is the closure certificate hypermesh runs before publishing a mesh ([surface_arrangement.rs L4336-L4456](https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/src/surface_arrangement.rs#L4336-L4456)), extended to cells, planes and history.
3. **A transaction.** Inputs are never changed. A failure or cancellation leaves the model exactly as it was. There is no partial result.

## 11.11 Failure behaviour and remedies

On valid input the polyhedral path fails only for reasons the caller controls: a budget, a cancellation, an unbounded selection, or a rule the caller set. The curved path can also fail where a certified decision is impossible. Every failure is an `OpError` (section 22):

| Error | When | Remedies offered |
| --- | --- | --- |
| `InvalidOperand` | An operand fails the checker, or is a point cloud | For a gap: heal it (at import, within ε_heal). For a self-intersection: regularise the operand by uniting it with itself, which is exact on plane-based input |
| `DuplicateOperand` | Two operands share a key | Give each a distinct key |
| `NearContact` | A curved decision is still undecided at the refinement limit and wider than the contact radius | Move one operand by at least the reported distance; close the gap first with `close_near_contacts`; the mesh path, Certified within its tessellation bound |
| `NearCoincident` | Two NURBS faces neither provably coincide nor provably part | As for `NearContact` |
| `CoincidenceChain` | A merge's representative cannot be proven within the remaining bound of all its carriers | `close_near_contacts` with the reported width; move apart |
| `UndecidedOrder` | Edges at a vertex, or faces at an edge, cannot be ordered up to third derivatives | Move one operand; the mesh path |
| `ToleranceExceeded` | A Tolerant operand would need a larger tolerance | Heal the operand first |
| `ExactnessWouldDrop` | The caller required Exact and an operand is curved or not Exact, or the summed bound would pass ε_fit | Accept the lower class explicitly; facet explicitly |
| `UnboundedResult` | The selection would include the outside | Rewrite the test |
| `NonManifoldResult` | Only with `ManifoldRule::Require` | Accept a General body; split it |
| `FrameRange` | The operands do not fit one frame at the finest grid step involved | Work in a frame placed at the overlap |
| `BudgetExceeded` | A stage's deterministic work count passes the budget | Raise it; split the call; ask for a preview |
| `SeedOnBoundary`, `SeedOutside` | A seed lies on a face, or in no cell | Move the seed |

`Cancelled` needs no remedy. Successful results carry diagnostics instead of failing: `EmptyResult`, `TouchingOnly`, `ToolMissesHost`, `CutterDoesNotSeparate`, `Sliver`, `ShortEdge`, `SeedsShareCell`, `NamesQualified`, `WeakName`. A broken internal invariant is a bug. It returns `Internal` with what is needed to reproduce it, never a panic.

**The engine never:** rounds a plane-based vertex; perturbs a tie; grows a tolerance, or snaps faces together inside a boolean; decides topology from a tessellation, as Truck does ([03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md), section 1); retries with a looser tolerance; falls back to another representation unasked; drops a tool or a face silently; breaks a tie by memory address, hash order or thread timing; trusts an unchecked promise that operands only touch; uses an angle to decide topology; makes a decision that is neither exact nor certified.

## 11.12 Parallel work and determinism

- **Polyhedral path.** Each subdivision step is a pure function of its box, its polygons and its reference point (the paper's section 4.5.4). The tree of boxes depends only on the input: split positions come from the box's own data, with ties broken by (operand key, name). Boxes run on any number of threads through the parallel helper of section 21, and results are joined in tree order, not in the order threads finish. The canonical rebuild (11.3) makes the bytes independent of the split anyway.
- **Curved path.** Interference pairs and face splits are pure work per pair and per face, sorted by name before use. Merges are decided in name order, in one stage, before any later decision uses them.
- **Budgets.** Work units (tasks and predicate calls) are counted per task, deterministically, and summed in tree order at the end of each stage. Work that is stolen and redone is never counted twice. Each stage also checks a deterministic estimate before it starts. So `BudgetExceeded` happens at the same stage on every machine, carries only the budget and that stage's total, and may be written to the operation log. Running out of time or memory is a cancellation: the operation is not applied and nothing about it is logged, because a clock must never change the model.
- **Checked** by the section 21 matrix: every target, 1, 2, 3 and 8 threads, golden hashes of canonical output.

## 11.13 What is unsolved, and how Spare Kit leads

- **Certified surface intersection near tangency.** No open method guarantees the right topology for every near-tangent pair of NURBS surfaces. A 2026 paper still calls complex intersection topology "an open challenge" ([04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), section 6). No open kernel offers certified general NURBS booleans: GoTools is AGPL, BRL-CAD's are tolerance-based, and hyperbrep covers listed special cases only ([10](../../../kit/notes/G01-evidence/10-github-sweep.md), section 3; [vetting](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md)). Spare Kit leads by deciding analytic pairs exactly, certifying what can be certified (the topology tests of [Bartoň, Elber and Hanniel](https://gershon.cs.technion.ac.il/papers/TopolGuarSSI.pdf) and later work, section 12), and using one fixed contact radius for the rest. Meanwhile it promises that a near-tangent case either gets the right topology within its stated bound or fails with `NearContact` and a remedy. It never returns wrong topology quietly.
- **A certified generalised winding number for trimmed NURBS.** None is published, and Axom's is fast but unproven. A research spike tries to add a proven error bound to its quadrature. Meanwhile the certified ray method of 11.4 gives proven answers at a higher cost.
- **Coincidence of NURBS beyond affine changes of parameter.** Two surfaces that are the same shape under any other change of parameters are found only by the certified distance test, within the contact radius.
- **Speed at 40 bits in WebAssembly.** EMBER's timings are for 26-bit inputs in native code. `I384` arithmetic in wasm32, which has no 128-bit multiply, is unmeasured ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), open question 7). The f64 filters settle most tests first, and the benchmark gates of section 23 decide whether more is needed.
- **Names that still collide** fall back to weak ordinals (section 20.2).

## 11.14 Crates and sources

| Crate | What it holds for booleans |
| --- | --- |
| `geom-poly` | Plane-based polygons and points; the arrangement predicates; conversion of plane-based faces into convex polygons; EMBER subdivision, local cuts, sparse winding vectors and tracing; indicators, early-out derivation and piece selection, shared by the B-rep and mesh booleans |
| `geom-bool` | Operands and selections; the curved builder; the rebuild into bodies; boolean naming; the arrangement certificate; batch cuts and the arrangement cache; cells; `close_near_contacts` |
| `geom-intersect` | The interferences and ray-surface intersections the curved builder uses (section 12) |
| `geom-topo`, `geom-name`, `geom-check` | Regions and fins (section 8), names and history (section 20), the checker (section 8.3) |
| `geom-mesh`, `geom-field` | The mesh and field booleans (sections 19 and 16) |
| `geom-combine` (proposed) | The one boolean entry point across representations (11.8) |
| `engine-geometry` | The boolean's schema in the stable facade |

**What we take, and how.** EMBER, Nehring-Wirxel's integer plane arithmetic and the winding-number papers are implemented from the papers. OpenCascade's General Fuse (LGPL) is studied for method only. For GPL projects we read only their published papers, never their source. For CGAL's Nef polyhedra that is Hachenberger, Kettner and Mehlhorn ([2007](https://doi.org/10.1016/j.comgeo.2006.11.009)). For libigl's copyleft arrangement code (which depends on GPL CGAL) and Blender's exact boolean, it is Zhou et al. 2016, on which they build (for Blender this is unverified). Axom's winding numbers and geogram's radial sorting (both BSD-3) may be ported, with their notices kept. Manifold (Apache-2.0) supplies EMBER's test cases and its regression meshes. Hypermesh is MIT, forked from MPL-2.0 boolmesh, and its licence history is still to be confirmed ([vetting](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md)). We treat it as a design to re-derive, not code to copy; either licence would allow that. Test models from Thingi10K and the ABC set carry per-model licences, some of them non-commercial. They are fetched at test time and filtered by licence, and each model's licence is recorded in the corpus manifest.
