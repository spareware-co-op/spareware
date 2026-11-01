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
    accept: Accept,                  // the least exact result taken: Certified at ε_fit unless set; Exact fails rather than return Certified
    reuse: Option<ArrangementCache>, // 11.6: may speed a re-cut, never changes the result
}
```

**What the caller accepts.** `accept` is the least exact result the caller takes, written with section 14.3's `Accept`: a class and, for Certified, the largest bound. The default is `Certified { pos: ε_fit, gap: ε_ang }`, section 9's limit and the operation's tolerance ceiling (section 6.5). A result outside it fails with `ExactnessNotAccepted` (11.11). Nothing is lowered silently. OQ-11.15 asks for this one field, and this one error, in every operation's contract.

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

**As built (G123).** `geom-poly` has the canonical planes and the orders above. `plane::canonical` divides a plane by the greatest common divisor of its coefficients and fixes the first non-zero normal component positive, and a `PlaneTable` holds one entry per canonical plane at the narrowest width, `i64` or `i128` (23.9), so two operands on one plane share one index and a face records which way it looks along it. `order` reduces each order the paragraph above names: along a line it is the classify test times the sign of a determinant of normals; rays about a vertex in a face are a determinant of normals, 252 bits; flaps about an edge need less, since the two normals' cross product is parallel to the line, so one component of each, 167 bits, decides. Both turn orders are total from a first ray or flap. Parallel planes, and a plane parallel to the line that must cross it, are `Degenerate`. `geom_num::Plane::new` makes a plane from coefficients and checks the budget of 6.2 again. The ladder of 23.2 comes with G124: these predicates go straight to the exact path, through the classify test's generated filter where they use it.

**As built (G125).** `geom-poly`'s `convex_pieces` takes a face's plane and its rings, each side a plane with the inside on its positive side, and returns convex pieces: an explicit stack of cells, each cut by the next edge in the order given, so a caller passes edges in name order. A piece's bounds are put in turn by the angles of their lines, not by the planes that end them, since a third bound can touch a corner without holding an edge. A side parallel to the face, two adjacent sides on one line, and rings that leave a piece without an end, such as a hole with no outline, are `Degenerate`. Whole-body conversion, with its broad phase that leaves untouched faces alone, comes with the boolean's entry (G130).

**As built (G126).** `geom-poly`'s `clip` splits a convex piece by a plane with exact predicates alone: each corner's side is decided on the meeting of the face and two bounds, and a new corner is the meeting of the face, a bound and the cut, so the only plane a part gains is the cut's. `crosses` asks whether two pieces pass through each other, and `local_cuts` cuts each polygon of a box by every polygon that crosses it, in the order given. A polygon is kept as a list of its convex pieces rather than EMBER's small binary tree: the list is that tree's leaves, and step 3 reads only leaves. Two polygons on one plane never cross here; their overlaps are 11.5's and come with the emission of pieces (G127).

**As built (G127).** `geom-poly`'s `winding` holds the sparse vectors of 11.2 and 11.1's tests, in postfix so that reading one takes no recursion, and `trace::classify` gives each piece of a box its two vectors. A path starts at a corner of the piece, moved in along its two edges and off its face, and runs along the line of an edge to the first face of the box, along the piece's plane to the first edge of the box, and along the box's edges to its reference corner: five segments at most, where the paper's section 4.4 takes three, because a crossing outside the box would meet no polygon of it and the box's own faces are the only planes known to stay in it. Starting from a corner needs no point inside a thin polygon, so the paper's randomised offsets have no counterpart. Ties take no tolerance: every plane of the path moves by its own infinitesimal, in a fixed order, and a sign that would be zero comes from the first term in those infinitesimals, a 3 × 3 determinant of normals. `local_cuts` now also cuts a polygon by one that ends on it, as a polygon clipped by a box does where it left the box, and polygons on one plane by each other's bounds where they overlap, so a piece of one lies wholly inside the other or outside it, and of an overlap the lowest polygon's copy is the one emitted.

**As built (G128).** `geom-poly`'s `subdivide::arrange` halves a box along its longest axis, on odd half steps, until it holds at most `LEAF_POLYGONS` polygons, and solves each leaf with `local_cuts` and `classify`. An odd half step holds no polygon that comes from grid data, so every polygon that ends on a piece reaches it inside one leaf; a split plane that does hold a polygon, as a committed plane can, is passed over. A box stops being split when a split no longer separates its polygons, which is what keeps faces on one plane from dividing down to the grid. Each level of the tree is one phase of the parallel helper and the leaves another, in tree order. The upper box's reference point walks up the box's edge from the lower box's, on the same moved planes a trace uses. Step 4's early outs are read off the test itself, in Kleene's three values: a box is dropped when every test has one answer for every count of every entry with a polygon in it. The two worst cases of the paper's figures 24 and 25 are tests.

**As built (G129).** The merge and the publication are two steps in two crates, so `geom-mesh` can take the first (19.5). `geom-poly`'s `merge::select` turns what a test keeps to face out of the result, and `merge::merge` makes it faces, edges and shells: every corner is sorted by its exact coordinates, every side of a piece by its line's Plücker coordinates, and along one line pieces of one sheet on both sides of a stretch cancel, so what is left are the stretches of boundary, merged where their fins are the same. The cuts of the subdivision and of the convex split disappear in that sweep, which is why a result is the same however the boxes fell. An edge's numbers come from the shape alone, a run is cut where another run ends on its line, and each fin keeps the input piece that holds it, whose `Outward::polygon` names its source. A sheet's source faces as the result does where it can (11.5). Where a face touches itself at a vertex, a ring stays in its corner of the face: two holes that meet at a corner are one ring past both. `geom-bool`'s `rebuild` keeps `Carrier` and writes what `merge` returned into arenas: a vertex on the grid is a grid point and any other a `Meet` of its lowest valid triple; an edge's line is a shadow between rounded ends, and one whose ends round to a single point stands a step long from the end on the grid, since group 2 holds a grid vertex to its line's end exactly and a meeting to within a step. An edge's two planes are its two faces' planes, read through its fins, so the body stores them once. Shells are nested by the winding vector, over the shells, of a point in front of each.

**As built (G130).** `geom-bool`'s `operand::prepare` turns bodies into what the arrangement takes: keys sorted and checked, one entry per solid, a test per selection, and every face as convex polygons whose bounds are the planes across its edges, with a plane stood on the edge where the face across lies on the same plane or there is none, as along a sheet's rim. `combine::combine` arranges them once and reads every selection from the same pieces, returning one list of bodies per selection. Unite, subtract, intersect and where are winding tests. Imprint keeps the target's pieces and hands the others' to `merge_with` as cuts, which hold a seam open along any stretch one lies on. A cut that ends inside a face leaves it whole, so a face never carries a slit. Split puts each piece of another operand inside the target in twice, back to back, so that radial order walks each cell into a shell of its own. A wall with one cell on both sides separates nothing and comes out; taking those away changes no cell, so one more merge finishes, and each cell is rebuilt as a body. Nothing left to separate gives the target whole with `bool.cutter_does_not_separate`. Two faces back to back lie at one angle about an edge, so the order there is the one that alternates: the face whose material lies behind comes first. A cavity is held by the outer shell around a point behind it, not in front, because a cell may fill it back to back. Keep and trim merge a sheet's pieces with `Rules::open`, where an edge may have one fin; trim holds seams along the other operands' pieces and keeps the faces that hold its seeds, refusing a seed on no piece or between two. Section uses no faces: a side of a piece lies in another polygon only where the two polygons meet, because a local cut runs along that polygon's plane, so a stretch where pieces of two owners on different planes both have a side is a stretch of the curve. Operands that touch along a face therefore meet around the contact's outline, not across it. `half_space::half_space` builds the solid behind a plane through three grid points, inside the range box, from clipped box sides and a cap, without an arrangement; a boxed half-space is the same call. A result less exact than `accept` allows is refused with `CombineDetail::NotAccepted`, whose kind is `ExactnessNotAccepted`. Still to come: general fuse and cells, which need regions that share faces (G137), the batch rules (G138), and wire targets for keep and trim.

**As built (G133).** `geom-bool`'s `certificate` runs on every result after the checker: `certify` walks each edge's radial ring of fins and refuses a solid or general body whose neighbours run the same way or come in an odd number, an edge that ends where it starts, and a face with no loop or a loop of fewer than three fins; `labelled` refuses a kept piece of a winding selection that does not have the test false in front and true behind. Faces not kept on purpose are the point of the other selections, so they skip `labelled`. Each check has a test that breaks a body the way it looks for, through `Builder::edit`. Planes need no check here, since a plane outside its budget cannot be made, and `Origin` records wait for names (G132). A transaction takes no code: operands are shared borrows, a `Body` never changes, and a failure returns none.

**As built (G131).** `geom-bool`'s `unite`, `subtract` and `intersect` take any number of operands, sorted by key, in one arrangement; nothing about them is pairwise. A union of many operands equals the same bodies united one after another, as corners, shells, faces and edges, and two calls with the operands in different orders give equal bodies; the same holds for a subtraction of many tools. A thousand boxes unite in one call, and that test is the benchmark's shape, with the timed gate in G140.

**As built (G132).** `geom-bool`'s `names::name` gives every entity of a result its name and history record, and `combine` publishes in that name order. Each polygon knows its operand and its face, so a face's sources are its pieces' sources. One source that the operand keeps: its own name, Unchanged if no fin moved, Modified if not. A tool's: `Created { op, role: tool(key), from: [name] }`, Generated, its source deleted. A name two operands brought: the same with `operand(key)`, and `bool.names_qualified`. One source in several faces: `Split { of, piece: ByTools }`, the tools being the other operands' faces on the planes the face's fins lie on, the side the lowest tool's. Several sources: `Merged`. Flush faces that brought one name are each qualified first, then merged, and generated from it, since a merge of one name would say nothing. A split body counts a face split across all its cells, though each cell holds one piece. An edge between two faces of one kept operand keeps that edge's name, Unchanged if its ends are the input's and Modified if it alone carries that edge on; anything else is `Bounded` by its faces and ended by the faces at its vertices. A vertex keeps a kept operand's name where that operand had one at exactly that point. A region is named for the least face name of its shell, because a face's number moves with its coordinates and a name must not. Names several rows share take weak ordinals by vertex number, which is the exact lexicographic order, and the telling apart repeats, because a body comes back in holding the weak names of an earlier call. An operand with no names gives a body with no history. The section's wires and half-spaces carry no names yet.

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
- new vertices are computed from the carriers' definitions, never from an earlier fit or vertex, and committed to the grid. Each adds commit rounding ((√3/2)q as a distance in 3D, about ε_fit/74; section 6.5) plus the width of its enclosure;
- a merge adds its representative's proven distance from its carriers.

A typical boolean therefore adds about ε_fit/74 plus the enclosure width. If the sum would pass the bound the caller accepts (ε_fit by default, 11.1), the operation does what section 9 requires: it fails with `ExactnessNotAccepted` and offers the larger bound as an explicit choice. Owner action G120 settled OQ-11.1 (32.21): every entity is certified directly against its carriers, and bounds add only along each entity's own history, so a part cut 200 times keeps its measured bound.

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
- **Closing near contacts is a separate operation.** A caller who wants nearly coincident faces treated as one runs `close_near_contacts(d)` first. It is a repair operation in `geom-bool` (section 10.2). For each pair of faces, edges or vertices of different operands within d, it adopts one carrier for both: that of the same-facing source with the lowest key and name. It recomputes every affected vertex, exactly on plane-based bodies, re-runs the checker, and reports how far each entity actually moved. A vertex can move much further than d where a neighbouring plane is nearly parallel, so the movement is measured, not assumed. The result is Certified(largest actual movement), or the operation fails. It is explicit, logged, and capped by the operation's tolerance ceiling (section 6.5). The default cap is OQ-11.7.
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
- **Machining and boards.** Stock-removal simulation over thousands of tool positions is not done as thousands of B-rep subtractions. It runs on the exact mesh boolean (section 19), whose paper benchmarks exactly this iterated milling case (EMBER Fig. 14, after Nehring-Wirxel 2021), or on a field model (section 16). Either way it cuts the swept volumes of the tool's moves (section 10.2). A B-rep of the finished part is made by an explicit cut when it is wanted. Board layers are usually built by 2D region booleans per layer and then extruded (`geom-2d`, section 14, and the board toolkit, section 26).
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

This is section 9's rule, applied by one entry point in `geom-combine`, an internal crate this chapter proposes (OQ-11.2). It calls each representation's crate. `engine-geometry` only exposes its schema.

| Operands | What runs | Result class |
| --- | --- | --- |
| All plane-based B-rep | Polyhedral path | The least exact input; the path adds nothing |
| B-rep, any curved, subdivision or Tolerant | Curved path | Certified(ε), by 11.4 |
| Any exact mesh | The exact mesh boolean in `geom-mesh`, on the same `geom-poly` arrangement and indicators; curved B-rep operands are tessellated with a certified bound (section 19) | Plane-based operands as above; curved ones Certified(their bound plus the tessellation bound) |
| Any field | The field boolean in `geom-field`; other operands enter as a certified distance plus a winding number, the winding number exact for closed plane-based bodies (section 16.3) | Certified when meshed |
| Any point cloud | Refused with `InvalidOperand`: clouds are reference only | — |

- The tessellation bound defaults to whatever keeps the result within ε_fit. A coarser bound must be asked for, and is recorded.
- Nothing falls back silently. A failed B-rep boolean offers the mesh or field path as a remedy, and the caller, person or AI, must choose it.
- On request, a closed exact mesh can first be lifted exactly to a plane-based body, so the result stays a B-rep (OQ-11.4).

**Previews.** When `OpContext` asks for a preview precision (section 10.1), as when a door is dragged, curved operands are evaluated at a coarser certified bound or on the mesh path. The result carries its real class (Certified at the coarser bound), is marked as a preview, and is never stored or logged as model geometry. Plane-based operands need no preview, because the exact path is already the fast one.

## 11.9 History and names

Every output entity gets a name and a history record under section 20.

**Names from operands.** A target (of subtract, imprint, split, keep or trim) keeps its names. A tool's faces that enter the result, such as the reveals of an opening, are named by the tool's key: `Created { op, role: tool(key), from: [their own name] }`. Two doors of the same type therefore give different reveal names, and adding a third door renames nothing. In unite, intersect, where and general fuse, faces keep their names. If two operands bring the same name (copies, arrays, a body united with a moved copy of itself), those faces are qualified by their operand's key, `Created { op, role: operand(key), from: [their own name] }`, with a `NamesQualified` diagnostic (OQ-11.9).

**A qualified face is Generated.** Only an unchanged or one-to-one modified entity keeps its name (section 20.2). A face qualified by a tool's or an operand's key changes its name, so its history is never Unchanged or Modified. It is Generated, with role `tool(key)` or `operand(key)`, from its source face, and the source is listed as deleted (section 20.3). This holds whether or not the face was trimmed or reversed.

| What happened to an input face | History | Name |
| --- | --- | --- |
| Passed through untouched, under its own name | Unchanged | Its own name |
| Enters the result qualified by a tool's or an operand's key | Generated, with role `tool(key)` or `operand(key)`, from the source face; the source is listed as deleted | `Created { op, role, from: [its own name] }` |
| Trimmed, or split with one piece kept | Modified | The same (one to one) |
| Split into several kept pieces | Split, with the cutting faces and side | `Split { of, piece: ByTools { tools, side } }` |
| Merged with flush faces | Merged | `Merged { of }` |
| Shares an overlap piece with coincident faces | Modified, from every coincident face | From the coincident faces facing the same way as the result: one gives its own name, several give `Merged`. Faces facing the other way appear in the history, not the name |
| Not in the result | Deleted | — |
| A new section edge or corner | Generated, from the faces that meet there | `Bounded` by those faces |

This follows OpenCascade's rule for booleans: Generated for new intersection edges and vertices, and Modified for overlaps ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 3). We add one case: a face qualified by a key is Generated too, because its name changes.

- **Several edges between the same faces.** `Bounded` alone cannot tell them apart, and a corrugated sheet cut by a plane makes hundreds. Adding the faces that end each edge would, but it needs a change to section 20.2's type: `Bounded { kind, by, ends, ord: u16 }` (OQ-11.3). The owner accepted the `ends` field and not the second ordinal (owner action G78, 32.19): edges between the same faces are told apart by the faces that end them, and those that still collide take section 20.2's weak ordinal, ordered along the edge in a canonical direction.
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
| `ExactnessNotAccepted` | The result would be less exact than `accept` (11.1): Exact was asked and an operand is curved or not Exact, or the summed bound would pass the accepted bound (ε_fit by default) | Accept the lower class or the larger bound explicitly; facet explicitly |
| `UnboundedResult` | The selection would include the outside | Rewrite the test |
| `NonManifoldResult` | Only with `ManifoldRule::Require` | Accept a General body; split it |
| `FrameRange` | The operands do not fit one frame at the finest grid step involved | Work in a frame placed at the overlap |
| `BudgetExceeded` | A stage's deterministic work count or logical memory passes the budget (11.12) | Raise it; split the call; ask for a preview |
| `SeedOnBoundary`, `SeedOutside` | A seed lies on a face, or in no cell | Move the seed |

`Cancelled` and `OutOfMemory` need no remedy and are never logged (11.12). Successful results carry diagnostics instead of failing: `EmptyResult`, `TouchingOnly`, `ToolMissesHost`, `CutterDoesNotSeparate`, `Sliver`, `ShortEdge`, `SeedsShareCell`, `NamesQualified`, `WeakName`. A broken internal invariant is a bug. It returns `Internal` with what is needed to reproduce it, never a panic.

**The engine never:** rounds a plane-based vertex; perturbs a tie; grows a tolerance, or snaps faces together inside a boolean; decides topology from a tessellation, as Truck does ([03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md), section 1); retries with a looser tolerance; falls back to another representation unasked; drops a tool or a face silently; breaks a tie by memory address, hash order or thread timing; trusts an unchecked promise that operands only touch; uses an angle to decide topology; makes a decision that is neither exact nor certified.

## 11.12 Parallel work and determinism

- **Polyhedral path.** Each subdivision step is a pure function of its box, its polygons and its reference point (the paper's section 4.5.4). The tree of boxes depends only on the input: split positions come from the box's own data, with ties broken by (operand key, name). Boxes run on any number of threads through the parallel helper of section 21, and results are joined in tree order, not in the order threads finish. The canonical rebuild (11.3) makes the bytes independent of the split anyway.
- **Curved path.** Interference pairs and face splits are pure work per pair and per face, sorted by name before use. Merges are decided in name order, in one stage, before any later decision uses them.
- **Budgets.** Work is charged by section 23.10's schedule: input and output entity counts, pairs of entities whose integer boxes overlap (a mathematical count, however the broad phase finds them), and refinement iterations (certified subdivision levels, Newton steps). Nothing is charged per predicate call, box visited or cache probe. So filters, early outs and the arrangement cache (11.6) cannot change a count, and a cached re-cut charges exactly what a full run does. Each count belongs to a fixed item, such as an operand pair or an output face, never to a box of the subdivision, and items are summed in a fixed order at the end of each stage. Work that is stolen and redone is never counted twice. Logical memory is counted the same way, from entity and scratch counts at fixed sizes. Each stage also checks a deterministic estimate before it starts. So `BudgetExceeded`, for work or for logical memory, happens at the same stage on every machine, carries the budget, the stage and the work used, and may be written to the operation log. Real out-of-memory returns `OutOfMemory`. A time limit is the caller's deadline, which the cancellation token turns into `Cancelled`. Neither is logged and nothing is applied, because neither the machine nor a clock may change the model.
- **Checked** by the section 21 matrix: every target, 1, 2, 3 and 8 threads, golden hashes of canonical output.

## 11.13 What is unsolved, and how Spare Kit leads

- **Certified surface intersection near tangency.** No open method guarantees the right topology for every near-tangent pair of NURBS surfaces. A 2026 paper still calls complex intersection topology "an open challenge" ([04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), section 6). No open kernel offers certified general NURBS booleans: GoTools is AGPL, BRL-CAD's are tolerance-based, and hyperbrep covers listed special cases only ([10](../../../kit/notes/G01-evidence/10-github-sweep.md), section 3; [vetting](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md)). Spare Kit leads by deciding analytic pairs exactly, certifying what can be certified (the topology tests of [Bartoň, Elber and Hanniel](https://gershon.cs.technion.ac.il/papers/TopolGuarSSI.pdf) and later work, section 12), and using one fixed contact radius for the rest. Meanwhile it promises that a near-tangent case either gets the right topology within its stated bound or fails with `NearContact` and a remedy. It never returns wrong topology quietly.
- **A certified generalised winding number for trimmed NURBS.** None is published, and Axom's is fast but unproven. A research spike tries to add a proven error bound to its quadrature. Meanwhile the certified ray method of 11.4 gives proven answers at a higher cost.
- **Coincidence of NURBS beyond affine changes of parameter.** Two surfaces that are the same shape under any other change of parameters are found only by the certified distance test, within the contact radius.
- **Speed at 40 bits in WebAssembly.** EMBER's timings are for 26-bit inputs in native code. `I384` arithmetic in wasm32, which has no 128-bit multiply, is unmeasured ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) OQ 7; OQ-11.11). The f64 filters settle most tests first, and the benchmark gates of section 23 decide whether more is needed.
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

**What we take, and how.** EMBER, Nehring-Wirxel's integer plane arithmetic and the winding-number papers are implemented from the papers. OpenCascade's General Fuse (LGPL) is studied for method only. For GPL projects we read only their published papers, never their source. For CGAL's Nef polyhedra that is Hachenberger, Kettner and Mehlhorn ([2007](https://doi.org/10.1016/j.comgeo.2006.11.009)). For libigl's copyleft arrangement code (which depends on GPL CGAL) and Blender's exact boolean, it is Zhou et al. 2016, on which they build (for Blender this is unverified). Axom's winding numbers and geogram's radial sorting (both BSD-3) may be ported, with their notices kept. Manifold (Apache-2.0) supplies EMBER's test cases and its regression meshes. Hypermesh is MIT, forked from MPL-2.0 boolmesh, and its licence history is still to be confirmed ([vetting](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md)). We treat it as a design to re-derive, not code to copy; either licence would allow that. Test models follow section 29.15. ABC: not used until an owner decides. NIST models: fetched by pinned hash once an owner confirms the terms. Thingi10K: only models whose own licence allows our use, each model's licence recorded in the corpus manifest.

## 11.15 Decisions

What this chapter decides, in brief. The sections above give the reasons.

1. One arrangement engine. An exact plane-based builder (the EMBER method) runs when every operand is a plane-based body. A certified General-Fuse-style builder runs when any operand is curved, a subdivision surface, or Tolerant. One shared back end selects, rebuilds, names and checks. Every boolean is a selection, and several selections in one call share one arrangement.
2. Classification uses sparse winding vectors (sorted entry and count lists), grouped by role. The tools of a cut, or the operands of a union, are summed into one entry, as EMBER's Fig. 21 does. Cells are connected pieces of space found by connectivity; vectors only label them. A selection that is true on the outside fails with `UnboundedResult`.
3. Cutters (sheets, wires, room separators) enter the arrangement with a zero transition. This is our own extension of EMBER, with a written proof obligation and tests. Sheet and wire targets are served by Keep and Trim, classified by the other operands' entries or by seeds, never by their own.
4. EMBER is implemented clean-room at 40-bit coordinates with `I320` and `I384` predicates, ties broken by (operand key, name), no randomness, explicit stacks for every tree walk, and a canonical rebuild into maximal faces. Results do not depend on the subdivision or the thread count.
5. Plane-based faces re-enter the exact path by keeping each edge's two defining planes and each vertex's canonical triple, and by splitting non-convex faces only along their own edge planes (an autopartition). Iterated cutting therefore never rounds and never leaves the bit budget.
6. The exact path is chosen by the operands' form (plane-based, exactly planar, planes within budget, checked when each plane is built), not by their class label. A Certified plane-based body still uses the exact arrangement and keeps its class. Tolerant bodies take the curved path.
7. Curved-path decisions are made about carriers, never about stored approximations: exact integer tests for the listed analytic cases on grid data, and certified enclosures otherwise. ε_fit is used as a fixed contact radius only when certification cannot separate two entities. Merges are validated against every carrier within the remaining bound, so repeated operations cannot drift.
8. Booleans never snap or treat near-coincident faces as coincident beyond the contact radius. Closing near contacts is a separate, explicit, logged repair operation, `close_near_contacts(d)`, that measures the actual movement and returns Certified(movement). `Contact::Within` is removed from the boolean input.
9. Curved classification uses certified signed ray crossing from a grid point in a box proven clear of the other operands, with rays kept clear of their edges by more than their bound. A certified generalised winding number for trimmed NURBS is treated as research and is not used for decisions.
10. Bounds follow section 9 and add up. Each boolean's own term is kept to about ε_fit/74 plus enclosure widths, because faces never move, new edges are procedural, and new vertices are computed from carrier definitions. A result whose summed bound would pass the bound the caller accepts (ε_fit by default) fails with `ExactnessNotAccepted`.
11. Subdivision operands take part through their limit surface as a carrier (Certified). A boolean never converts them to NURBS, which would make them Approximate.
12. Every operand carries a stable, caller-supplied key. Targets keep their names. Tool faces that become reveals are always named by the tool's key. Colliding names in symmetric selections are qualified by key, with a diagnostic. A face qualified by a key is recorded as Generated from its source, never as Unchanged or Modified. Section edges and new corners are Generated.
13. Batch cutting is Atomic by default, with `SkipFailedTools` as an option, and every tool is reported, including `ToolMissesHost`. Incremental re-cuts use an explicit, immutable arrangement cache that can change speed but never results, and is never model state.
14. Rooms are cells named by seeds. Separators enter as cutters and room limits as half-spaces. A leak between rooms is reported with the location and width of its narrowest passage. IFC second-level space boundaries (2a and 2b) are read from cell adjacency.
15. Indicators, early-out derivation and piece selection live in `geom-poly`, so `geom-mesh` reuses them. The cross-representation dispatcher goes in a new internal crate, `geom-combine`, below the facade (proposed in OQ-11.2).
16. `BudgetExceeded` is decided from deterministic per-stage counts charged by section 23.10's schedule, never per predicate call, box visited or cache probe, and from logical memory. It carries the budget, the stage and the work used. Real out-of-memory (`OutOfMemory`) and a caller's deadline (`Cancelled`) are never logged.
17. Previews evaluate curved operands at a coarser certified bound or on the mesh path, carry their real class, are marked as previews, and are never stored or logged.
18. For GPL projects (CGAL Nef, Blender's exact boolean, libigl's copyleft code) only published papers are read, never source. LGPL OpenCascade is studied for method only. Test models follow section 29.15: ABC waits for an owner's decision, NIST models wait for an owner to confirm their terms, and Thingi10K models are used only where their own licence allows, each recorded in the corpus manifest.

## 11.16 Open questions

Each question has a stable ID, OQ-11.N, which this chapter and others cite. An answered question keeps its ID, and its answer is recorded with it. Section 6.5's "chapter 11, open question 8" is OQ-11.8. A question in an evidence report is cited by the report and its own number, such as "[05] OQ 7".

- **OQ-11.1** How section 9's "bounds add up" counts across repeated booleans. The chapter follows the literal rule. Each curved boolean then adds about ε_fit/74 (commit rounding of new vertices, (√3/2)q in 3D) plus the widths of their enclosures. So a curved part fails with `ExactnessNotAccepted` after roughly 74 booleans, sooner once enclosure widths are counted, although sections 2 and 6.5 promise curved geometry within ε_fit. Faces never move and new edges are procedural, so every entity's true error can be certified directly against the stored carriers (checker group 2). Recommended: a body's bound is the largest proven distance of any entity from its carriers, re-certified per operation, and "add up" applies to chains of conversions where the carriers themselves are approximations (for example SubD to NURBS to offset). Confirmed by owner action G120 (32.21), with 13.2's one rule and ε_step.
- **OQ-11.2** Crate list changes (section 27). (a) The one boolean across representations needs a home below the facade, because `engine-geometry` is the stable tier and `geom-convert` does not depend on `geom-bool`. Proposed: a new internal crate, `geom-combine` (depending on bool, mesh, field and convert); the alternative is adding bool to `geom-convert`'s dependencies. (b) The curved broad phase needs a certified box tree, but `geom-bool` does not depend on `geom-spatial`. Recommended: a plain box tree in `geom-math`, where boxes already live, shared by `geom-bool` and `geom-spatial`; the alternative is adding `geom-spatial` to `geom-bool`'s dependencies. (c) Indicators and piece selection sit in `geom-poly` so that `geom-mesh` can reuse them. That needs no dependency change, but `geom-poly`'s one-line job should mention them.
- **OQ-11.3** Section 20.2's `Bounded` name cannot tell apart several edges between the same two faces. A corrugated NURBS sheet cut by a plane makes hundreds, and `ord: u8` overflows at 256. Proposed: `Bounded { kind, by, ends: SortedSet<NameId>, ord: u16 }`, keeping the faces an edge lies between separate from the faces that end it. Settled by owner action G78 (32.19): `ends` is taken, `ord` is not, because edges that still collide take `Weak`, whose `u32` ordinals do not overflow.
- **OQ-11.4** A closed, exact mesh can be lifted exactly to a plane-based body, so a B-rep result is possible with no loss. The chapter follows section 9 and offers the lift only on request. Should a closed exact mesh mixed with B-rep operands be lifted by default?
- **OQ-11.5** Patents: no patent search has been done for EMBER, Nehring-Wirxel's integer plane arithmetic, or Campen and Kobbelt's plane-based booleans. AGENTS.md section 5 asks that possible patents be raised with an owner. It is listed as the first build piece, before any `geom-poly` code merges.
- **OQ-11.6** EMBER has no public code (a GitHub search on 2026-10-21 found none; unverified). Should an owner ask the RWTH authors whether a reference implementation or more test data can be shared under a permissive licence, to shorten the clean-room work and strengthen testing?
- **OQ-11.7** `close_near_contacts` needs a default cap. Section 6.5 defines ε_heal (640q, capped at 6,400q) only for healing at import. Options: reuse those values for this explicit repair, or add a ladder rung of its own. Settled by owner action G241 (32.30): ε_heal's values, 640q by default and at most 6,400q, within the operation's tolerance ceiling.
- **OQ-11.8** Please confirm ε_fit as the curved path's contact radius, which gives it a second meaning beyond "largest certified construction error". As defined here, it is used only when certification cannot separate two entities. Entities touch only when the enclosure still contains zero at the refinement limit and its upper bound is strictly below ε_fit; anything wider fails deterministically with `NearContact`.
- **OQ-11.9** Names in symmetric selections (unite, intersect, where, general fuse). The chapter keeps operands' names and qualifies them by operand key only where two operands bring the same name, with a `NamesQualified` diagnostic. That honours section 20.2's "an unchanged entity keeps its name", but adding a colliding operand renames faces. The alternative, always qualifying in symmetric selections, is stable but departs from 20.2. Owner's choice.
- **OQ-11.10** Batch default: Atomic (one bad tool fails the call) or `SkipFailedTools` (cut the rest and report). Section 10.1's "one failure does not fail the list" may point to `SkipFailedTools`. The chapter chooses Atomic for transactional safety.
- **OQ-11.11** Risk: `I320` and `I384` arithmetic on wasm32, which has no 128-bit multiply, is unmeasured ([05] OQ 7). If the section 23 gates fail, options include a double-double filter stage before the wide integers. This needs a benchmark early in the build order.
- **OQ-11.12** Licences to confirm. Hypermesh's history (forked from MPL-2.0 boolmesh, relicensed MIT under a "zalo" notice) is unconfirmed, though either licence would allow re-deriving its design. Test models follow section 29.15. ABC: not used until an owner decides. NIST models: fetched by pinned hash once an owner confirms the terms. Thingi10K: only models whose own licence allows our use, each recorded.
- **OQ-11.13** Subdivision operands rely on section 16 delivering certified bounds for Stam evaluation near extraordinary points. If it cannot, booleans with subdivision operands will fail near those points (with an explicit NURBS conversion as the remedy) rather than silently become Approximate.
- **OQ-11.14** Where this revision differs from the review's proposed fixes: (a) the review suggested intersecting IfcBoxedHalfSpace with its box, but the IFC 4.3 text says the box "does not alter the final result", so the chapter treats it as the plain half-space; (b) `Contact::Within` was removed from the boolean rather than repaired, and replaced by the separate `close_near_contacts` operation; (c) the exact path is chosen by the operands' plane-based form, not by an Exact class label, so a Certified plane-based body still uses the exact arrangement, while Tolerant bodies are excluded as the review asked; (d) for milling, EMBER itself benchmarks iterated milling on the exact path (its Fig. 14), so the exact mesh boolean is named as the established method alongside fields. Dexel models are not named, because no chapter defines one.
- **OQ-11.15** One acceptance field and one error kind for "less exact than the caller takes". This chapter uses section 14.3's `Accept` (a class and, for Certified, the largest bound; default Certified at ε_fit) and its error, `ExactnessNotAccepted`. Sections 12.2, 13.2, 13.20, 15.10 and 28.2 express the same limit as a ceiling and refuse with `ToleranceExceeded`, which section 6.5 uses for Tolerant bodies. Proposed: `Accept` in section 10.1's contract for every operation, `ExactnessNotAccepted` in section 22's `ErrorKind`, and `ToleranceExceeded` kept for Tolerant bodies only.

## 11.17 Build pieces

The pieces that build this chapter, each with what done means and what it needs first. Chapter 30 is to put every chapter's pieces in build order.

1. **Patent check for plane-based exact booleans (owner action).** Small.
   - Done when: an owner has searched for patents on EMBER, Nehring-Wirxel's integer plane arithmetic and Campen and Kobbelt's plane-based booleans, and recorded the result in the decision log, before the first `geom-poly` piece merges (AGENTS.md section 5; OQ-11.5).
   - Depends on: nothing.
2. **Plane type and arrangement predicates (`geom-poly`).** Medium. G02 has built part of it in `geom-num` instead: the plane through three grid points with its construction check, three-plane points, classification in `I384` and the commit to the grid (`plane.rs`). What remains here is gcd normalisation and sharing, coincidence, parallelism, the three orders and the filters (30.0).
   - Done when: Code: an integer plane type that refuses, at construction, coefficients beyond the section 6.2 budget (normal 83 bits, offset 125 bits at 40-bit coordinates); three-plane points (`I256` weight, `I320` coordinates); classify (`I384`); plane coincidence and parallelism; order of points along an edge; order of edges around a vertex within a face; radial order of faces around an edge. Each sits behind a generated f64 filter, and each module carries its written bit-budget derivation. Tests: every coordinate at ±(2^40 − 1); differential fuzzing against a num-rational oracle; Kani sign proofs on bounded inputs. Benchmark gate: the filter settles at least 99% of calls on the EMBER case list; the exact-path cost is measured on x86-64, aarch64 and wasm32 and reported against section 23.
   - Depends on: the section 6 numbers layer in `geom-num`: `I256`, `I320` and `I384`, and the predicate filter generator.
3. **Plane-based faces into convex polygons (`geom-poly`).** Medium.
   - Done when: Code: every edge keeps its two defining planes (its faces' planes, or the face plane and the plane of the face that made a seam or imprint); every vertex keeps a canonical triple, the lowest valid one by (operand key, name); non-convex faces and faces with holes are split by an autopartition along their own edge planes, with edges taken in name order and an explicit stack; faces outside every other operand's box pass through unconverted; the split can be exported as part of the arrangement cache. Tests: property tests that the pieces exactly cover the face and that every new corner is a triple of existing planes; 300 successive single-opening cuts of one wall, with every plane checked against the budget after each cut; faces with hundreds of holes. Benchmark gate: conversion costs no more than 10% of a 300-opening cut.
   - Depends on: plane type and arrangement predicates; plane-based bodies in `geom-topo` (sections 6.3 and 8).
4. **Polygon clipping and per-polygon local cuts (`geom-poly`).** Medium.
   - Done when: Code: exact clipping of convex plane-based polygons; the per-polygon binary space partition of EMBER section 4.3, run on an explicit stack; overlapping coplanar polygons emitted once, by the lowest (operand key, name). Tests: property tests that pieces are disjoint and cover the input exactly, unchanged under operand reordering and under copies with equal names; a depth test on EMBER Figs. 24 and 25; a fuzz target. Benchmark gate: EMBER Fig. 25 (20 cubes sharing their top and bottom planes) within 3 times the paper's 5.9 ms on comparable hardware.
   - Depends on: plane type and arrangement predicates; plane-based faces into convex polygons.
5. **Sparse winding vectors and exact tracing (`geom-poly`).** Medium.
   - Done when: Code: sparse winding vectors (sorted entry and count lists) with entries grouped by role; zero-transition cutters, with the written proof note that they change no traced vector; three-segment exact paths; deterministic interior points from a fixed offset sequence; reference-point propagation; EMBER's no-self-intersection and no-nested-parts flags set from checked bodies. Tests: agreement with brute-force exact classification on random nested, overlapping and touching solids, with and without cutters; vector length equals the true overlap depth. Benchmark: tracing cost against vector depth, for up to 2,000 operands.
   - Depends on: plane type and arrangement predicates; polygon clipping and per-polygon local cuts.
6. **Indicators, early outs and piece selection (`geom-poly`).** Small.
   - Done when: Code: `WindingTest` expressions (`Inside(key)`, `AtLeast(n)`, `And`, `Or`, `Not`) with the boundedness check that raises `UnboundedResult`; early-out rules derived mechanically from a selection, including pieces it keeps on purpose; piece selection callable by `geom-mesh` without `geom-bool`. Tests: for random expressions and inputs, early outs never drop a piece that full evaluation keeps. Benchmark: early-out hit rate on the batch-cut suite.
   - Depends on: sparse winding vectors and exact tracing.
7. **Adaptive subdivision and the parallel task tree (`geom-poly`).** Large.
   - Done when: Code: an explicit task queue with no recursion; split positions from box data, with (operand key, name) tie-breaks; work and logical memory charged by section 23.10's schedule (entity counts, pairs of overlapping integer boxes, refinement iterations; never predicate calls, boxes visited or cache probes), per fixed item and summed in a fixed order (11.12); per-stage estimates; cancellation checks. Tests: bit-identical output and identical work counts for 1, 2, 3 and 8 threads and on every section 21 target; the EMBER case list (Manifold's Apache-2.0 list; only Thingi10K models whose own licence allows our use, each recorded, section 29.15) agrees with an exact rational oracle. Benchmark gates: geometric mean on the EMBER case list within 3 times the paper's 1.6 ms on a comparable 8-core machine; EMBER Fig. 24 (20 by 20 beams) within 3 times its 4.5 ms.
   - Depends on: polygon clipping and local cuts; sparse winding vectors; indicators and early outs; the section 21 parallel helper and determinism CI.
8. **Canonical rebuild into bodies (`geom-bool`).** Large.
   - Done when: Code: maximal faces; maximal edges that keep their defining planes; exact vertex identity with canonical triples; loops; shells by radial order; cells by connectivity and nesting; redundant-edge removal and `KeepSeams`; General bodies for non-manifold results; arena indices in name order. Tests: output bytes identical across subdivision thresholds, thread counts and operand orders; every corpus result passes the section 8.3 checker. Benchmark gate: rebuild under 50% of arrangement time on the EMBER case list.
   - Depends on: adaptive subdivision and the parallel task tree; the section 8 topology builder and arenas (`geom-topo`).
9. **Operands, selections and the boolean schema (`geom-bool`).** Medium.
   - Done when: Code: operand keys and `DuplicateOperand`; solid and cutter operands; half-spaces (plain, IfcBoxedHalfSpace as the plain half-space, IfcPolygonalBoundedHalfSpace as the half-space intersected with its prism) inside the frame range box; all eleven selections, several per arrangement; the `accept` field with `ExactnessNotAccepted`; `CutterDoesNotSeparate`. Tests: the shared-face table of 11.5; edge and corner touching; nested and overlapping solids; partial cutters; sheet and wire targets under Keep and Trim. Benchmark gate: three selections from one arrangement cost under 1.2 times one.
   - Depends on: indicators, early outs and piece selection; canonical rebuild into bodies.
10. **Boolean names and history (`geom-bool` with `geom-name`).** Medium.
    - Done when: Code: the 11.9 rules: target names kept; tool-qualified names; qualification of colliding names with `NamesQualified`; every qualified face recorded as Generated from its source face, with the source listed as deleted; Split `ByTools`; Merged; coincident pieces named and carried by the same source; Generated section edges and corners named `Bounded`; weak ordinals for colliding edges until the section 20.2 type change is decided, then end-face disambiguation; cell names. Tests: section 20.3 invariants, and section 28.3's law that untouched entities keep their names, property-tested on every selection; names identical under operand reordering, exact grid translations and quarter turns; two doors of one type give distinct reveal names, and adding a third renames nothing. Benchmark gate: naming under 15% of boolean time.
    - Depends on: canonical rebuild; operands and selections; the section 20 naming core (`geom-name`); OQ-11.3 on the `Bounded` type.
11. **Arrangement certificate and transactional result (`geom-bool`).** Small.
    - Done when: Code: the 11.10 certificate on every result, wired to `geom-check`. Tests: mutation tests show that each check catches the fault it targets; failed and cancelled operations leave inputs untouched. Benchmark gate: certificate under 10% of boolean time.
    - Depends on: canonical rebuild into bodies; the section 8.3 checker (`geom-check`).
12. **Cells, rooms and space boundaries (`geom-bool`).** Medium.
    - Done when: Code: seeded cells named by key; separators as cutters; upper and lower limits as half-spaces; `SeedOnBoundary` and `SeedOutside`; `SeedsShareCell` with the widest-path narrowest-passage report; cell adjacency, "what is beyond this face" and 2a/2b space-boundary classification; clash volumes with one entry per element. Tests: a sample storey of walls, slabs, columns and rooms matching hand-computed boundaries; a deliberate 0.003 mm leak reported at the right place; clash volumes checked against exact volumes. Benchmark gate: a storey of 2,000 elements and 100 rooms within 500 ms native on 8 cores, also reported on wasm32.
    - Depends on: operands, selections and the boolean schema; boolean names and history.
13. **Batch cutting (`geom-bool`).** Medium.
    - Done when: Code: n-tool subtraction in one arrangement with a summed entry; a per-tool report with `ToolMissesHost`; Atomic and `SkipFailedTools`; many hosts per call, returned in input order. Tests: IFC wall-opening fixtures; openings sharing jambs; tools that miss or only touch. Benchmark gates: a wall with 300 openings cut and certified within 20 ms native on 8 cores; time grows linearly in host faces plus tools.
    - Depends on: operands and selections; boolean names and history; arrangement certificate.
14. **Arrangement cache and incremental re-cut (`geom-bool`).** Medium.
    - Done when: Code: an immutable arrangement cache keyed by a content hash of host and tools, returned beside the result and accepted in `reuse`; only boxes the moved tool touches are re-solved; a cache that does not match its inputs is ignored. Tests: the incremental result equals a full recompute bit for bit, and charges the same work count, across random edit sequences. Benchmark gate: moving one door in a 300-opening wall is re-cut and certified within 16 ms native and 48 ms on wasm32 (initial gates, to be confirmed in section 23).
    - Depends on: batch cutting; plane-based faces into convex polygons.
15. **Polyhedral verification corpus (`geom-test`, `geom-fuzz`).** Medium.
    - Done when: Tests: exact-volume oracle fuzzing of random grid boxes (the hypermesh pattern); vol(A union B) + vol(A intersect B) = vol(A) + vol(B); A minus A is empty; commutation with exact transforms; EMBER Figs. 24 and 25; Manifold's regression meshes; out-of-process differential runs against Manifold and OpenCascade, in CI only; golden hashes in the determinism matrix. Only models whose own licence allows our use are fetched, and each model's licence is recorded in the corpus manifest (section 29.15).
    - Depends on: operands and selections; names; certificate; batch cutting; cells.
16. **Curved builder data model, broad phase and contact rule (`geom-bool`).** Large.
    - Done when: Code: `PointGeom` and `CurveGeom` variants; a certified box tree; the exact analytic decision table of 11.4 with written bit budgets; certified decisions on carriers with a fixed refinement limit; the contact rule at ε_fit (touch only when the enclosure contains zero and its upper bound is strictly below it); merges validated against every carrier within the remaining bound; `NearContact` and `CoincidenceChain` with evidence; merges decided before any later predicate uses them. Tests: analytic pairs at extreme coordinates; 50 successive near-contact booleans, then a certified check of every vertex against all its carriers. Benchmark: broad-phase pairs per second, reported against section 23.
    - Depends on: plane type and arrangement predicates; section 12 certified intersections (`geom-intersect`); a home for the box tree (OQ-11.2).
17. **Interferences, paves, pave blocks and common blocks (`geom-bool`).** Large.
    - Done when: Code: interferences lowest dimension first, with lower-dimension reuse; certified ordering of paves; pave blocks; common blocks, exact within one analytic kind and by the contact rule across kinds; face information with On, In and section groups; split edges with history. Tests: collinear wall edges; cylinders sharing a line; an arc against a NURBS arc; a section curve through an existing corner. Benchmark: a 1,000-face mechanical part against one tool, reported against section 23.
    - Depends on: curved builder data model, broad phase and contact rule.
18. **Face splitting for flat and analytic carriers (`geom-bool`; half of an Extra large piece, split).** Large.
    - Done when: Code: loops traced in parameter space by exact orientation on flat faces, and by certified tangents, then curvature, then up to third derivatives on quadrics, else `UndecidedOrder`; seams and poles; coincident analytic faces as single pieces; a clippy lint forbidding angle functions in topology modules. Tests: planes tangent to cylinders; coaxial cylinders; cones through their poles; the differential test that all-flat inputs through the curved builder equal the polyhedral builder's output bit for bit. Benchmark: face splits per second.
    - Depends on: interferences, paves, pave blocks and common blocks; canonical rebuild; section 12 analytic intersections.
19. **Face splitting for NURBS, procedural and subdivision carriers, and same-domain faces (`geom-bool`; half of an Extra large piece, split).** Large.
    - Done when: Code: the same splitting for NURBS, procedural and subdivision-limit carriers; exact NURBS coincidence after checks for reversed, swapped and affinely reparameterised directions, degree elevation and common refinement, including trimmed sub-patches; a certified two-sided distance bound over the overlap; `NearCoincident` with evidence. Tests: a Class-A near-tangent set; core and cavity; matching panels; sheet-metal contact; reversed and reparameterised copies; trimmed sub-patches. Benchmark: same-domain detection cost.
    - Depends on: face splitting for flat and analytic carriers; section 12 certified surface-surface intersection; section 16 certified limit-surface evaluation.
20. **Certified ray classification (`geom-bool`).** Large.
    - Done when: Code: one classification per connected face group; a certified clear box on the face; a grid point in it; a fixed sequence of ray directions, each kept only if it stays further than the other operand's bound from every edge and meets no face tangentially; exact crossings on flat faces and certified Bernstein ray-surface intersection on curved ones; `NearContact` when no ray qualifies. Tests: analytic volumes; queries close to surfaces; Certified bodies with gaps up to their bound; agreement with exact classification on flat input. Benchmark: classifications per second.
    - Depends on: face splitting for analytic carriers (and for NURBS carriers for NURBS operands); section 12 ray-surface intersection.
21. **Research spike: a certified generalised winding number for trimmed NURBS.** Medium.
    - Done when: a written note gives either a proven error bound for the Spainhour, Gunderman and Weiss quadrature (starting from Axom, BSD-3, with its notice kept) plus a prototype measured against the ray method, or a documented negative result. Not on the critical path.
    - Depends on: certified ray classification (as the reference to beat).
22. **Curved path integration, bounds and Tolerant operands (`geom-bool`).** Large.
    - Done when: Code: the curved builder feeding the shared back end; summed bounds with per-term accounting, and `ExactnessNotAccepted` past the accepted bound (ε_fit by default); Tolerant operands within their stored tolerances, with `ToleranceExceeded`; subdivision operands through the limit surface; every 11.11 error with its remedies. Tests: after 50 successive cuts the reported bound equals the sum of the recorded terms and every entity is certified within it; the full determinism matrix. Benchmark: the curved suites, reported against section 23.
    - Depends on: the canonical rebuild, selections, naming and certificate pieces; every curved-path piece above.
23. **`close_near_contacts` repair operation (`geom-bool`).** Medium.
    - Done when: Code: finds pairs of faces, edges and vertices of different operands within an explicit d; adopts one carrier per pair (the same-facing source with the lowest key and name); recomputes affected vertices, exactly on plane-based bodies and certified otherwise; re-runs the checker; reports each entity's actual movement; returns Certified(largest movement) or fails; capped by the operation's tolerance ceiling. Tests: walls 0.003 mm apart closed, with the movement computed exactly; a nearly parallel neighbouring plane whose vertex would move further than the cap is refused; the operation is logged and replays bit for bit.
    - Depends on: canonical rebuild into bodies; curved builder data model; OQ-11.7 on the cap.
24. **Curved boolean corpus (`geom-test`).** Large.
    - Done when: Tests: BIM (round columns through slabs, walls on arcs with openings, pipes and ducts through walls); mechanical (drilled holes, bosses, cuts beside fillets); boards (thousands of vias); Class-A NURBS cuts near tangency; subdivision operands; NIST STEP models, fetched by pinned hash once an owner confirms the terms, and no ABC models until an owner decides (section 29.15); out-of-process differential runs against OpenCascade; KNOWN, IMPROVED and FAILED statuses as in section 28.
    - Depends on: curved path integration.
25. **Cross-representation boolean entry point (`geom-combine`).** Medium.
    - Done when: Code: dispatch per 11.8; exactness-class propagation, with the default tessellation bound keeping results within ε_fit; remedies offering the mesh and field paths explicitly; point-cloud refusal; exact lifting of closed meshes on request; previews at a coarser certified bound, marked and never logged; the operation schema exposed through `engine-geometry`. Tests: one per operand mix, including the class each returns. Benchmark: dispatch overhead under 1% of the cheapest boolean.
    - Depends on: operands and selections; curved path integration; the mesh boolean (section 19); the field boolean (section 16); `geom-convert`; OQ-11.2 on the crate.
26. **Boolean performance gates (`geom-bench`).** Small.
    - Done when: Benchmarks: instruction-count gates and wall-clock trends for the EMBER case list and Figs. 24 and 25, the 300-opening wall and the moved door, a storey's cells, 10,000 holes in a slab, and the curved suites, native and wasm32; the `I384` cost on wasm32 measured and reported against section 23; a regression over 5% blocks a merge (principle 12).
    - Depends on: adaptive subdivision; batch cutting; incremental re-cut; cells; curved path integration.
