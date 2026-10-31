# 14. 2D drafting

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Evidence: [06](../../../kit/notes/G01-evidence/06-2d-drafting.md), [12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md) (sections geometry-2d and formats-legal), [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) (sections 3, 4, 7 and 9), [03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md) (section 4), [10](../../../kit/notes/G01-evidence/10-github-sweep.md) (GeometricTools and JTS), [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md) (csgrs and geo) and [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md) (section B).

The 2D layer is the drafting kernel under every flat drawing Spareware makes. It is one crate, `geom-2d`, on the same grid, predicates, curve kinds and names as the 3D kernel (principle 8). Stored curves are never approximated. Every "does this cross, touch or contain that" question about lines, arcs and ellipses is answered exactly; everything else carries a certified bound and says so.

| Who | What they need from `geom-2d` |
| --- | --- |
| Drafting and sheets, in any program | Every DXF-class entity, edits, snaps, hatches, dimensions, blocks, viewports |
| Building models | Room and area boundaries, plan booleans for wall-layer joins ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section B), clash in plan |
| Survey and civil | Linework from coded points with arcs, parcel areas, breaklines for terrain (section 17) |
| Circuit boards | Traces with widths, pads, copper pours with clearance and exact design-rule checks, on the q1 grid |
| Manufacturing | Profiles to cut, tool-radius offsets, pocketing, region booleans for nesting |
| Drawings and sketches | Visibility merge, cut-region hatching and dimensions on section 18's drawing curves; the curves section 15 constrains |

Beyond the `num`, `math` and `curve` that G01's section 27 listed, `geom-2d` needs `topo`, `name`, `intersect` and the box tree `bvh`; chapter 27's crate table, accepted in 32.13, records them.

## 14.1 Frames and entities

2D geometry lives in a 2D frame (a sketch plane, a view or a sheet) placed in the project frame. Like any frame it picks a grid step: q0 for buildings, sites, survey and most sheets; q1 for boards, parts and the sheets that show them. DXF's object coordinate system is handled only at import and export, in `geom-dxf`.

The entity model follows the public DXF reference ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 1), in our own words. Layers, colours, line weights and styles are program data: each entity carries an opaque attribute key that every edit passes through.

| Entity | What the kernel stores |
| --- | --- |
| Line, ray, infinite line | A line carrier (14.2) and zero, one or two end points. Rays and infinite lines are truly unbounded, never long segments |
| Circle, arc | A circle carrier. An arc adds a start and an end and runs counter-clockwise from one to the other; a mirror reverses it (14.10) |
| Ellipse, elliptical arc | Centre, major-axis vector, the axis ratio as a fraction of bounded size, and end points as parameters, not polar angles |
| Spline | NURBS with control points on the grid (or rational, after a rational transform) and knots and weights as fractions of bounded size. Fit points, end tangents and fit tolerance are kept as creation intent, refitted only by a versioned algorithm |
| Polyline | Vertices, a bulge per segment (the tangent of a quarter of the arc's sweep, signed by turning direction; [LWPOLYLINE](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-748FC305-F3F2-4F74-825A-61F04D757A50.htm)) and optional widths, whose outlines are offsets (14.5) |
| Point | A grid point |
| Region | Outer loops with nested holes of curve edges, stored as a planar Sheet body (14.4). DXF's REGION holds proprietary modeller data, so the kernel defines its own |
| Hatch | A boundary, an island rule, and a pattern, solid fill or gradient (14.6) |
| Text | Anchor, attachment, direction, height and reference width; extents come from `engine-text` (14.11) |
| Dimension, leader | Typed references and a style; the picture is derived (14.11) |
| Block, insert | A definition in its own frame; an insert is a typed transform, an optional array and attribute values (14.10) |
| Viewport | A sheet window onto a view, with a scale, a twist and a clip region (14.10) |

Road spirals (section 7) enter plans as alignment curves (section 17).

## 14.2 Exact numbers for lines, circles and ellipses

A **carrier** is the whole unbounded curve an entity lies on: the infinite line under a segment, the full circle under an arc. An **implicit point** (section 6.3) is stored as its definition, such as "where carrier A meets carrier B, first root", and is decided from that definition, never from a rounded copy.

```rust
struct Line2 { a: Int, b: Int, c: Int }   // a·x + b·y = c, whole numbers within a fixed budget
struct Circle2 { centre: Pt2, r2: Ratio } // exact squared radius; centre on the grid, or rational (14.3)
enum Pt2 {
    Grid(P2),                                        // whole grid steps, |x| < 2^40
    Rational(RationalDef),                           // a definition whose coordinates are fractions
    Crossing { a: CurveIx, b: CurveIx, root: Root }, // a line or circle meets a circle, or a line an ellipse
    ConicRoot { a: CurveIx, b: CurveIx, k: u8 },     // an ellipse meets a circle or another conic
}
```

`Root` says which of two crossings is meant, by their order along the first carrier.

- **Lines** have whole-number coefficients. For a line through two grid points |a| and |b| are below 2^41 and |c| below 2^82; constructed lines are larger, within 14.3's construction budget.
- **Circles** have an exact squared radius, a fraction. A drawn or imported circle has its centre on the grid; a circle made by a construction keeps an implicit rational centre (14.3). This is the model of CGAL's circle-segment traits, as its authors describe it ([Fogel, Halperin and Wein 2012](https://doi.org/10.1007/978-3-642-17283-0); for conic arcs, [Wein 2002](https://doi.org/10.1007/3-540-45749-6_76)). CGAL is GPL: papers and public documentation only; source not read (29.2).
- **Ellipses** have their centre and major-axis vector on the grid and an axis ratio p/q of bounded size, so their equations have whole-number coefficients. The owner accepted this as the ellipse kind (owner action G185, 32.25); a ratio read from a file becomes the nearest such fraction, Certified unless it already is one.
- **Splines, spirals and procedural offsets** are section 7's kinds, evaluated with certified bounds.

| Point class | Made by | Coordinates |
| --- | --- | --- |
| Grid | Input and commits | Whole steps |
| Rational | Two lines meeting; a fractional position along a carrier; a grid point moved by a rational vector, rotated or mirrored; the centre of a circle through three points | Fractions of bounded size |
| One-root | A line or circle meeting a circle; a line meeting an ellipse | (a + b√k)/w per coordinate, whole numbers throughout, one square root shared by both |
| Conic root | An ellipse meeting a circle or another conic | A real root of a quartic with whole-number coefficients, kept as its carriers and index |

**Predicates are exact.** Each returns a sign (section 6.2) after a generated f64 filter.

- On grid, rational and one-root points, every question (which side of a line, inside a circle, order along a carrier, equal or not, tangent or crossing) is the sign of a + b√k, or of a + b√k₁ + c√k₂ + d√(k₁k₂) when two crossings are compared. It is decided by repeated squaring in fixed-size integers. The set is closed: every carrier has rational coefficients, so every crossing is a one-root point, and no line is ever built through a one-root point (14.3 makes such lines by rational choices).
- **Tangency** is a discriminant (the quantity whose sign says two, one or no crossings) that is exactly zero. There is no tangency tolerance.
- **Ellipse crossings are decided exactly.** With a line they are one-root points. With a circle or another conic they are the real roots of a quartic with whole-number coefficients; their number, which are tangencies, and their order along each carrier come from real-root isolation by Descartes' rule of signs ([Collins and Akritas 1976](https://doi.org/10.1145/800205.806346)) and exact common-root tests, in fixed-size integers. A position, when needed, is certified within ε_fit. GeometricTools classifies ellipse pairs by the same kind of exact sign tests ([IntrEllipse2Ellipse2.h](https://github.com/davideberly/GeometricTools/blob/e09f5b39a33da6b4bfd0ff8dbb64ddcb7fdfc763/GTE/Mathematics/IntrEllipse2Ellipse2.h#L10-L22); Boost licence, portable with its notice).
- **Spline and spiral crossings are certified**, by section 12's curve-curve intersection, and each contact is `Crossing`, `Touching` or `Undecided` with its interval and place. `Undecided` is never treated as either (14.4).
- **Widths are derived, never typed.** Each predicate's bit budget is generated from its expression, as GeometricTools' BSPrecision does ([BSPrecision.h](https://github.com/davideberly/GeometricTools/blob/e09f5b39a33da6b4bfd0ff8dbb64ddcb7fdfc763/GTE/Mathematics/BSPrecision.h#L10-L11)), and checked by section 6.2's extreme-value tests. Drafting is full of near-tangencies no filter can certify, so the exact paths are benchmarked on native and WebAssembly targets.

**One representation, below every user.** These carriers and point classes are not private to `geom-2d`. They live in `geom-curve`, beside the other curve kinds, and their predicates (side, orientation, order, one-root and conic-root signs) live in `geom-num` with section 6.2's. So `geom-intersect` (12.1, 12.5), `geom-sketch` (15.10), `geom-profile` and `geom-2d` all decide on one form. Section 15.10's commit forms are these types: its integer carrier is a `Line2`; its depth-one point is a `Rational` point defined by two carriers and held as homogeneous integers (X, Y, W), meaning (X/W, Y/W); and its arc, with a rational centre on the chord's bisector, lies on a `Circle2`. 27's crate table records this placement (accepted in 32.13).

**As built (G187).** `geom_num::carrier` holds 15.10's forms and the predicates on them. A `Carrier` is a line a · x + b · y = c within the budget, running along (b, −a) with its positive side on the left, as `orient2d` has it, and a `Rational2` is (X/W, Y/W) with W positive, so no predicate multiplies by the sign of a weight. `Carrier::meet` is the depth-one point, Cramer's rule without its division, and a grid point is a `Rational2` of weight 1. Side, order along a carrier, one coordinate compared and the turn of three points are generated (`carrier_side`, `carrier_order`, `rational_cmp` and `rational_orient`) at 15.10's widths: 168, 251, 209 and 335 bits with their sign bits, the last in `I384`. `geom-2d`'s `Line2` and `Pt2` (G192) are built on these types; a constructed point's larger budget is 14.3's and comes with G192 and G193. Each is checked by the matrix of 28.6, and the Verus twins and the filter certificates are generated like every other predicate's, so `kernel-dev.yml` and `kernel-certificates.yml` are what prove them.

**As built (G188).** `geom_num::arc` holds the circle forms of 15.10 and every sign on them. A `Round` is the circle about a rational centre through a grid point, its squared radius exact; an `Arc2` is the part of one running counter-clockwise from a grid point to another, refused unless its centre is exactly on their bisector; a `Circle2` has a grid centre and a whole number of steps as its radius. Inside or outside is `round_grid`, 169 bits, as (Z − P) · (Z + P − 2C), `round_rational`, 336 bits, and `circle_side`, 253 bits, each within 15.10's figures. Tangency is `round_line`, 336 bits, and `circle_line`, 168 bits, the discriminant of where a line meets a circle, and `Contact::Tangent` means that value is exactly zero. Two `Circle2`s compare the squared distance between their centres with the squares of the sum and difference of their radii (`circle_gap`, 85 bits). The side of an arc's chord for its centre is `Carrier::side`. Order along an arc needs no new predicate and no angle: points of a circle in the order they run turn counter-clockwise, so it is `Rational2::orient` of the start and the two points, and a point of the circle is on the arc exactly when it is not to the left of the chord. Five of these filters are checked, not proven: a term of their scales is formed from more than two inputs beside terms of other groups, the case issue 292 sets out, so Gappa's problems cannot state them yet, and `FPTaylor` and HOL Light check them. How two `Round`s lie needs about 840 bits and comes with G189's wider integers, or with whatever piece first asks it.

**As built (G189).** `geom_num::one_root` holds 14.2's one-root class. A `Crossing2` is where a carrier meets a circle or two circles meet on their radical line, ((aₓ + bₓ√k) / w, (a_y + b_y√k) / w), one root shared by both coordinates and `Root` saying which of the two along the carrier, or which side of the way from the first centre to the second; a tangency is k = 0, and a rational point is the same form with k and b zero, so everything compares with everything. Each crossing keeps its coordinates as outward-rounded intervals, and a comparison whose intervals are apart costs two subtractions. Otherwise `sign_of_root` and `sign_of_two_roots` decide a + b√k and a + b√k₁ + c√k₂ + d√(k₁k₂) by squaring in 80 limbs: for crossings of a carrier and a circle, 505 bits for a side, 1,010 against a circle, 1,506 for two crossings' coordinates and 1,688 for the turn of a grid point and two crossings, and for crossings of two rational circles 1,343, 2,689, 4,857 and 5,041. These are written out in `one_root.rs` rather than generated: `geom-gen`'s language takes `i128` inputs and `f64` filters, and these inputs are wider and these values past 2^1023, so the widths are derived in the module and asserted when it compiles. On a tangency-heavy corpus of 1,728 crossings the filter decided 99.4% of 41,172 comparisons natively and the same on wasm32, whose arithmetic is the same IEEE doubles. Where two circles meet needs no carrier: their radical line's coefficients are past a carrier's budget, so the crossing is written from the two centres directly, which is why its integers take 14 limbs.

## 14.3 Constructions

A construction makes new geometry from old: an offset, a fillet, a tangent line, a rotation. Its ideal result usually needs square roots that do not stay closed, such as a fillet centre where two offsets meet. Exact tangency through repeated constructions needs algebraic numbers whose degree grows at every step. LEDA's and CORE's number types, and csgrs's hypercurve with its symbolic radii ([README](https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypercurve/README.md#L370-L427); Apache-2.0), provide them at a cost that grows with every step ([11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), csgrs). Fixed-size integers cannot, and no open kernel makes tangent constructions exact in fixed-size numbers.

Rounding the ideal result to the grid is no answer. Near a tangency, a one-step move shifts the crossing by about √(2rq): a fillet centre rounded to the grid moves at most q/√2, but its distances to the two lines move in opposite directions and can then differ by up to √2·q. That moves its tangent points by up to about 47 µm and opens a direction gap of up to 0.54 degrees for a 5 mm fillet at q0, about 10^8 times ε_ang. That turns a tangency into a crossing, which section 6.3 forbids.

**Rational choices.** Where the ideal result needs an irrational number, the kernel chooses a nearby rational one of bounded size, keeps it in the implicit point's definition, and builds everything else exactly from it.

- **A rational direction** is a unit vector (a/c, b/c) from a Pythagorean triple (whole numbers with a² + b² = c²), the closest within a requested angle δ, found from the best rational approximations of tan(θ/2). Its numbers have about log₂(1/δ) bits. This is the rational rotation method of [Canny, Donald and Ressler (1992)](https://doi.org/10.1145/142675.142726).
- **A rational position** is a point at a fractional position along an exact carrier, or along a ray from a grid point in a rational direction. A circle whose ideal centre is not rational (a fillet touching an arc, a round join at a crossing vertex) takes a rational centre within η ≈ ε_ang²·r/2 of the ideal.

Every choice obeys four rules: every join is an exact shared point; positions stay within q/2 of the ideal; every tangent join is within ε_ang of tangent; and the choice is made on a stated side, so that a curve leaving a tangent join never touches its neighbour again nearby, clearance zones err away from copper, and tool paths err away from the part. The result is Certified, with its position bound and largest direction gap reported, and Exact when no choice was needed.

| Construction | How | Exact when |
| --- | --- | --- |
| Offset of a line by d | Translate by d·n̂, n̂ a rational unit normal within δ = min(ε_ang, q/(2d)) of the true one; the offset is exactly parallel | The normal is already rational (axis-aligned, or Pythagorean such as 3:4:5) and d is whole steps |
| Offset of an arc by d | Same centre, so concentric stays concentric; squared radius ‖J − c‖² with J = P + d·û, û a rational unit vector near the radius at the arc's end P (or at a rational point within η of P, when P is a crossing) | Radius and d are whole steps |
| Fillet of two lines | Centre where the two offset lines meet, a rational point; the tangent points C − r·n̂ᵢ lie exactly on both the line and the circle | Both normals are rational; otherwise the gap is at most δ |
| Fillet of parallel lines | A semicircle centred on the midline, with an exact squared radius | Always |
| Fillets and tangent circles touching an arc or ellipse, circles tangent to three curves, loops of tangent arcs | Rational centre within η of the ideal; squared radius chosen so the circle meets each neighbour at an exact crossing | Never; Certified |
| Tangent line from a point; common tangent of two circles | A rational direction close enough that the line meets the circle at an exact crossing within ε_ang of tangent; the line ends there | Never; Certified |
| Circle or arc through three points | The circumcentre, a rational point | Always |
| Rotation | A rational rotation within ε_ang of the angle (14.10); whole-number lines stay whole-number lines, so parallel and perpendicular stay exact | Quarter turns; otherwise Certified in angle |

So a constructed circle has a rational centre, by section 6.3's rule that "if rounding would flip anything, the implicit form is kept instead"; drawn and imported circles keep grid centres. Section 32 asks the owner to confirm this reading.

**Budgets and commits.** Constructed numbers stay within a fixed construction budget, so every predicate has a fixed worst-case width. A construction whose inputs would exceed it, such as a long chain of constructions on constructions, first commits those inputs to the grid with section 6.3's re-check. If rounding would flip a decision, it returns `NotRepresentable` with the inputs and the deviation, and two remedies: build from the original sources (an outward offset of an offset is one offset by the summed distance), or accept the deviation. A commit moves a point by at most q/2 per axis (section 6.5), q/√2 in distance.

**Exactness is accepted, never assumed.** An operation never lowers the class without being asked (section 9). Each schema states the classes an operation can return, and each input states the least exact result the caller takes:

```rust
enum Accept { Exact, Certified { pos: Length, gap: Angle }, Approximate { pos: Length } }
```

A result below that returns `ExactnessNotAccepted`, with the bound it would reach and what would keep it exact (a whole-step radius, an axis-aligned direction). Interactive drafting commands in `kit-ops` pass `Certified { pos: ε_fit, gap: ε_ang }` explicitly, on record in the operation log. A deviation beyond the ladder needs an acceptance naming that value.

**Import, survey and export.**

- **DXF data** is converted once, in double-double arithmetic, and rounded to the grid (section 6.1). Each bulge arc's centre becomes a rational point on its chord's perpendicular bisector, chosen within the bound, so the arc passes exactly through both grid vertices and no junction moves.
- **Ends of separate entities** that almost meet, with a gap of at most ε_heal, are recorded as a Tolerant junction (section 6.5) and healed once where healing succeeds; healed linework is Certified or Exact, and every action is recorded. The healing distance is the largest distance between original and healed curves, not vertex movement: at a near-tangent junction the shared point may slide along the curves while the curves move about a grid step. A junction healing cannot close stays Tolerant: operations work within its tolerance or refuse with `ToleranceExceeded`. A gap beyond ε_heal is never Tolerant. Where the file says those ends meet, as the edges of a hatch's boundary path do, it is an error naming the junction, with section 24.8's remedies: leave the linework open with free ends, or raise ε_heal up to its cap of 6,400q. Elsewhere, ends that far apart are simply free ends.
- **Junctions meant as tangent** that meet at a small angle after rounding are listed with their gaps. An explicit repair makes one tangency per arc exact, by moving its centre to where the bisector meets the neighbour's normal: a rational point.
- **Ellipse ratios** become the nearest fraction that keeps the minor axis within q/2; **spline knots and weights** become fractions of bounded size. Each change is certified and recorded.
- **Survey**: an arc through three surveyed points passes exactly through them; points computed from bearings and distances use rational directions, then commit to the grid.
- **Explicit output**, for a format without exact arcs or a display list, is snap-rounded: vertices move to the grid under the "hot pixel" rule that stops rounding from changing topology ([Hobby 1999](https://doi.org/10.1016/S0925-7721(99)00021-8)), as JTS implements it ([SnapRoundingNoder.java](https://github.com/locationtech/jts/blob/5a0a73632216e3e9d277150336078ec423fd215f/modules/core/src/main/java/org/locationtech/jts/noding/snapround/SnapRoundingNoder.java#L29-L56); EDL).

## 14.4 The arrangement and region finding

An **arrangement** is the division of the plane by a set of curves into vertices, edges and faces. `geom-2d` builds it as a **doubly-connected edge list** (DCEL): each edge has two half-edges, one per side, and each face knows its outer boundary, its holes and any lone vertices ([Muller and Preparata 1978](https://doi.org/10.1016/0304-3975(78)90051-8); de Berg, Cheong, van Kreveld and Overmars, *Computational Geometry*, 3rd edition, chapter 2). The DCEL is section 8's structure in the plane: a half-edge is a fin, and the arrangement is a planar General body in immutable chunked arenas with typed indices (section 8.2). A Region is a planar Sheet body and linework a Wire body, so one checker and one naming scheme serve 2D and 3D, and conversions are lossless both ways. The methods follow CGAL's arrangement package as its authors' book describes it ([Fogel, Halperin and Wein 2012](https://doi.org/10.1007/978-3-642-17283-0)), in our own words ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 3). CGAL is GPL: papers and public documentation only; source not read (29.2).

- **Plane sweep** builds it from many curves at once: a vertical line sweeps across, stopping at every end point and crossing ([Bentley and Ottmann 1979](https://doi.org/10.1109/TC.1979.1675432), generalised to curves). Curves are first split into **x-monotone** pieces, which no vertical line crosses twice. Stops are ordered by exact x, then y, then name.
- **Zone insertion** adds one curve by walking the faces it crosses, making a new version that shares every unchanged chunk. **Overlay** combines two arrangements and merges their face labels; booleans, clipping and section 18's visibility merge are overlays.
- **Point location** in a batch-built arrangement uses [Sarnak and Tarjan's](https://doi.org/10.1145/6138.6151) structure, a search tree whose earlier versions stay queryable, built once along the sweep, with a guaranteed logarithmic query. After edits, a query walks from the nearest landmark vertex, found through 14.8's box tree ([Haran and Halperin 2009](https://doi.org/10.1145/1412228.1412237)). CGAL's randomised trapezoid map is not used: its bounds hold only for a random insertion order, which determinism would replace by an order derived from names.
- **Degeneracies are handled on purpose.** Pieces on one carrier merge into one edge that keeps every source; tangencies are touching vertices; any number of curves through one point make one vertex. There is no symbolic perturbation, because in drafting these ties mean something (section 6.2).
- **Edges around a vertex are ordered exactly**: by tangent direction, then signed curvature, then higher derivatives. Two different conics differ by the fourth derivative at the latest, because they meet with multiplicity at most four. A spline contact that cannot be ordered is `Undecided` and reported.
- **Unbounded faces** from rays and infinite lines end on a symbolic frame at infinity that is never committed, as CGAL's fictitious edges do. Implicit points beyond ±2^40 steps are valid; committing one returns `OutOfRange`.
- **Slivers are real.** A near-tangent crossing made by a rational choice can leave a face narrower than a grid step. It is a true face of the stored data, listed in the diagnostics, never merged away.
- **Provenance.** Each edge records its source entities and the parameter interval it covers on each, as iOverlay's `EdgeOverlay` carries data through splits and merges ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 2). Names follow (section 20.2): a piece of a cut entity is `Split` by the entities that cut it and its side.

**Region finding** follows the JTS Polygonizer on the exact arrangement ([Polygonizer.java](https://github.com/locationtech/jts/blob/5a0a73632216e3e9d277150336078ec423fd215f/modules/core/src/main/java/org/locationtech/jts/operation/polygonize/Polygonizer.java#L27-L50); EDL, a BSD-style licence, so portable with its notice):

- **A boundary from a pick point** (DXF's BOUNDARY, or picking a hatch area) is point location: the face holding the point gives an outer loop and its holes. An unbounded face or a leaking loop returns `OpenBoundary` with every gap's place and size. On request, gaps up to a stated distance are closed by bridging segments with the role `gap.bridge`, chosen in order of exact distance, then name, never by moving vertices. An `Undecided` contact is a gap candidate, never a closure.
- **Dangles** (edges with a free end) and **cut edges** (the same face on both sides) are left out of the loops and listed in the diagnostics.
- **Islands** are the face's holes, kept or skipped by the hatch's island rule: odd parity, outermost only, or ignore ([HATCH](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-C6C71CED-CE0F-4184-82A5-07AD6241F15B.htm)).
- **Room boundaries** are the same operation on walls' plan curves from section 18's cut, plus room-separation lines. In 3D, rooms are regions of the cell complex (section 11).
- **Areas** of straight-edged regions are exact fractions (twice the area is a whole number when every vertex is a grid point); with arcs or ellipses they are certified. Both are shown correctly rounded (14.11).

## 14.5 Booleans and offsets

**Booleans** (union, subtract, intersect and exclusive-or, over any number of operands, with the even-odd, non-zero, positive and negative fill rules) work by overlay. Each face is labelled with every operand's **winding number**, the number of times that operand's boundary goes around it, and a truth table, or a tree of operations evaluated once over one arrangement as in csgrs's hypermesh ([11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md)), picks the faces whose loops are extracted. Results are regularised by default: lone edges and points that bound no area are dropped and listed. A result containing the unbounded face returns `UnboundedResult`, as hypermesh does. Touching, overlapping and shared boundaries, such as walls that share a face or pads that touch, are ordinary cases; cavalier_contours' booleans need two closed polylines that do not cross themselves ([pline_boolean.rs](https://github.com/jbuckmccready/cavalier_contours/blob/22887a824d22cd8ab50968a551f34cdb4508127f/cavalier_contours/src/polyline/internal/pline_boolean.rs#L381-L409)). Each output edge is named as split from or merged from its inputs.

**Offsets** follow cavalier_contours' structure (MIT OR Apache-2.0, portable with its licence kept; [pline_offset.rs](https://github.com/jbuckmccready/cavalier_contours/blob/22887a824d22cd8ab50968a551f34cdb4508127f/cavalier_contours/src/polyline/internal/pline_offset.rs#L1-L23)), a raw offset with arc joins, then cleanup, then stitching, with two steps rebuilt to our bar:

1. **Raw offset** by 14.3's rules, one rational unit normal per edge, so every offset edge is one segment exactly parallel to its source and meets its joins within ε_ang. This adapts the approximation in Wein's offset construction ([Wein 2007](https://doi.org/10.1016/j.cad.2007.01.010); described in CGAL's manual, [Minkowski_sum_2.txt](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Minkowski_sum_2/doc/Minkowski_sum_2/Minkowski_sum_2.txt#L450-L530); GPL: papers and public documentation only; source not read, 29.2), which picks rational points on the join circles instead and so bends each offset edge into two segments.
2. **Joins and ends.** A round join is an arc of the circle of radius d about the source vertex, on which the offset edges end exactly. Miter (with a limit), bevel and square joins, and butt, square and round ends, are Clipper2's set ([clipper.offset.h](https://github.com/AngusJohnson/Clipper2/blob/a6e7294703ae329576462f9471b356971f53010d/CPP/Clipper2Lib/include/clipper2/clipper.offset.h#L19-L26); Boost licence).
3. **Cleanup by winding number.** The raw offset with its joins is one closed cycle, the convolution of the shape with a disc, and the offset is the faces with a positive winding number about it ([Minkowski_sum_2.txt](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Minkowski_sum_2/doc/Minkowski_sum_2/Minkowski_sum_2.txt#L436-L448); Clipper2 likewise unions its raw offsets with the positive fill rule, [clipper.offset.cpp](https://github.com/AngusJohnson/Clipper2/blob/a6e7294703ae329576462f9471b356971f53010d/CPP/Clipper2Lib/src/clipper.offset.cpp#L574-L633)). The arrangement decides this exactly. It replaces cavalier's discarding of slices closer to the source than the offset distance, within fixed epsilons of 1e-5 and 1e-4 ([pline_types.rs](https://github.com/jbuckmccready/cavalier_contours/blob/22887a824d22cd8ab50968a551f34cdb4508127f/cavalier_contours/src/polyline/pline_types.rs#L65-L107)), so no distance threshold remains.
4. **Stitching** joins the kept pieces by the identity of their crossings.

Shapes with islands offset outer loops and holes, then combine them by a boolean. An inward offset that consumes a shape returns an empty result with a note: valid, not an error. Offsets of ellipses and splines are neither conics nor splines ([Farouki and Neff 1990](https://doi.org/10.1016/0167-8396(90)90023-K)), so they are section 7's procedural offset curve with a cached NURBS certified within ε_fit. Many offsets of one shape, as pocketing needs, can also come from its Voronoi diagram ([Held 1991](https://doi.org/10.1007/3-540-54103-9)), a strategy for a manufacturing toolkit (section 26).

**Boards.** Copper is designed on the q1 grid. A trace's copper is its centreline offset by half its width with round ends: lines and arcs, Certified unless the trace is axis-aligned or Pythagorean. Design-rule checks never use these outlines; they run on centrelines and half-widths with 14.9's exact clearance test. A copper pour is the zone outline within the board, minus other nets' copper offset by the clearance, minus keep-outs, with **thermal spokes** (thin bridges that join a pad to the pour while limiting heat flow during soldering), islands below a minimum area removed, and a minimum width enforced by offsetting in by half that width and back out. Every rational choice in a pour or keep-out is made on the side away from copper, so the stored clearance zone contains the true one. Tool-radius compensation uses the same operations, with every choice on the side away from the part.

**iOverlay** (MIT OR Apache-2.0) is used only for linearised results to show, and only outside the kernel: screen fills, clip masks and previews while dragging run in `engine-draw2d` or program crates, never in `geom-2d` (29.1). Its integer mode is deterministic ([README](https://github.com/iShape-Rust/iOverlay/blob/117637e3a65804973da2bfd7c59f42773f4dcc0d/iOverlay/README.md#L492-L533)), but it flattens arcs, so its results are labelled Approximate and never stored or fed into a decision. Display flattening takes its chord bound from the view scale, never from the drawing's extents.

## 14.6 Hatches and pattern families

- A **pattern** is a set of line families, each with a direction, a base point, an offset between lines, and dashes, gaps and dots ([Pattern Data](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-7C05C0EC-B0FB-4A86-A164-B9E5C6C03990.htm)). Directions are whole-number vectors; a DXF angle converts to the nearest rational direction (14.3) within ε_ang, with the difference certified and recorded. A coarser bound needs the caller's acceptance naming it (14.3).
- A **hatch** stores its boundary (references to source entities by name when associative, otherwise its own loops), its island rule, and a pattern with scale, angle and origin, a solid fill or a gradient. Associative hatches regenerate when their sources change. If a source is deleted or a loop stops closing, the hatch keeps its last boundary and shows `UnresolvedRef` as model state (section 20.4). Text extents from `engine-text` become hole loops with a margin, like DXF's text-box islands.
- **The picture is derived.** Each family line meets the boundary at exact crossings, the island rule picks the inside intervals, and dashes are placed by certified arc length. Lines are generated on demand, deterministically, and never stored as model truth. Linetypes along any curve work the same way; `engine-draw2d` draws both at true weights.
- **No silent caps.** Lines are counted exactly before any are made. Over the operation's budget the hatch returns `BudgetExceeded` with remedies: a scale that fits, or a solid fill. CADCraft silently skips any family over 20,000 lines in full builds, 100,000 in view-limited ones ([12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md)).
- **Patterns that are made, not drawn** (hatched copper, perforated sheet, paving) produce committed, named regions: each stripe is a region intersected with the boundary.
- **The pattern and linetype library is our own**, authored with a provenance log for every entry by the process the owner set in 32.24 (owner action G184), and never reproduces Autodesk's files. CADCraft's library, labelled its own, has names, descriptions and values the vetting found to match them ([12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md), formats-legal). An imported DXF hatch carries its own pattern lines, so reading one needs no library.

## 14.7 Edits in parameter space

Every edit cuts and joins curves at exact parameters of implicit points, takes many inputs in one call (section 10.1), and names what it makes (section 20.2): trimmed pieces as split by their cutting edges, a fillet arc with the role `fillet` from its two inputs, an offset with the role `offset` from its source.

| Edit | Method | Class |
| --- | --- | --- |
| Trim | Remove the picked piece between consecutive exact crossings with the cutting edges. A trimmed spline is section 7's trimmed curve, the original curve on an implicit parameter interval, so it ends exactly at the crossing | Exact for lines, arcs and ellipses |
| Extend | To the boundary's carrier: a line's infinite line, an arc's full circle, an elliptical arc's full ellipse. A spline extends by a stated policy: its end span's own polynomial (default), a tangent line or a curvature-continuous arc. No crossing returns `NoSolution` with the nearest approach | As the inputs |
| Break | At one or two parameters; a picked point becomes the nearest exact point on the carrier (14.8) | Exact |
| Join | Ends must be the same exact point. Pieces on one carrier become one entity; other chains become section 7's composite curve. A stated gap distance adds named bridges; there is no join fuzz. Making one NURBS of a composite is a separate conversion | Exact; the conversion Certified |
| Fillet, chamfer | Fillet by offset, intersect and project, the classical construction ([Rossignac and Requicha 1986](https://doi.org/10.1016/0167-8396(86)90017-8)), made by 14.3's rules; a semicircle between parallel lines. Chamfer by two distances, or a distance and an angle. Polyline mode does every corner. `RadiusTooLarge` gives the largest radius that fits | As 14.3 |
| Stretch | Vertices inside the window move by an exact grid vector. An arc with both ends inside moves exactly; an arc with one end moved takes a new rational centre on its chord's bisector, keeping its sweep within ε_ang | Exact or Certified |
| Lengthen | By a change, a percentage or a total; the new end is an exact point on the carrier. Arc length uses validated quadrature, whose interval bounds prove the error ([Corliss and Rall 1987](https://doi.org/10.1137/0908069)), with Carlson's elliptic integrals and their error bounds for ellipses ([Carlson 1995](https://doi.org/10.1007/BF02198293)) | Certified in length |
| Arrays | Rectangular by exact translations; polar by one rational rotation per copy, each within ε_ang and exact at quarter turns; along a path by certified arc length | Exact or Certified |
| Mirror, scale, align | Typed transforms (14.10); mirroring reverses arcs | Exact or Certified |
| Divide, measure | Points or blocks at equal lengths: rational on lines; on arcs, exact points on the circle along rational directions, the equal spacing certified; certified on ellipses and splines | As stated |

## 14.8 Snaps and references

A snap is a typed reference, not a point. Its kinds are defined by the geometry they find: a curve's start or end point; its midpoint; a centre; a point entity (a node); a quadrant point, where a circle or ellipse meets its own axes; the crossing of two curves; an insert's or a text's insertion point; the foot of a perpendicular; a tangent point; the nearest point on a curve; the crossing of two 3D curves as seen in a view (an apparent intersection); a point on a curve's carrier beyond its end; a point on a line through a chosen point parallel to a given line; a point on a line through a chosen point in a chosen rational direction (14.3), or where two such lines cross; and the centroid of a closed loop. Only `geom-dxf` maps them to DXF's associativity codes ([DIMASSOC](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-C0B96256-A911-4B4D-85E6-EB4AF2C91E27.htm)), on exchange (section 24).

```rust
struct SnapRef {
    kind: SnapKind,
    on: Reference,             // section 20.4's reference: element, path (14.10), name, kind, want, signature
    with: Option<Reference>,   // the second entity, for intersections, with its own path
    sub: Option<NameId>,       // a part of one entity: a polyline segment, a hatch loop edge
    at: Option<ExactParam>,    // for nearest: a dyadic parameter, or an implicit one
    cached: Pt2,               // the point as last resolved, for display only
}
```

- **Every snap resolves to an exact point.** End points, centres, nodes, quadrants, intersections, insertion points, a line's midpoint, perpendicular feet (on an ellipse, where it meets a conic through the point) and tangent points on circles and ellipses (where the curve meets the point's polar line, a rational line) are 14.2's implicit points. Other snaps, such as an arc's midpoint or a nearest point on a spline, are found with certified bounds, then made exact on the carrier: a point at a dyadic parameter (a fraction with a power-of-two denominator) on a line or a spline, whose data are fractions, so it lies exactly on it; or, on a circle or ellipse, the crossing with a ray from its centre in a rational direction. An apparent intersection is the crossing of section 18's projections of two 3D curves.
- **Dimensions use carrier references by default** (section 20.4's `want`), so trimming a line does not break a dimension to it.
- **A cursor is just an input:** a point and an aperture in grid units. Candidates come from the kernel's one tree of integer boxes, `geom-bvh`, over 2D boxes (19.9; 27, accepted in 32.13); the closest wins and ties break by name. There is no linear scan and no cap on candidates; CADCraft tests at most 200.
- **An AI agent needs no cursor.** It asks for "the intersection of wall W-12's exterior line and grid line C" by names and gets the same reference.
- Dimensions, leaders, hatches and section 15's constraints bind to these references, which resolve as section 20.4 describes.

## 14.9 Distance and clearance

Board design rules and clash in plan need yes-or-no answers that are never wrong.

- **Exact for lines, arcs, and ellipses against lines.** "Is the gap between A and B at least c?" is an exact sign. For a segment and a circle of squared radius ρ it compares the squared distance from the centre to the line with (c + √ρ)², a one-root test. An ellipse against a line uses the ellipse's support function, its extent in a given direction, which adds one square root. Pairs of arcs need a few square roots, still decided by repeated squaring.
- **Three-way for other pairs** (an ellipse against a circle or ellipse, anything against a spline): the distance is a certified interval and the answer is `Pass`, `Fail` or `Undecided` with the interval and place. Design-rule checks and clash report `Undecided` as a possible violation, never as a pass.
- **Batched:** every pair closer than c is found through the box tree and reported sorted by name.

On a board these check copper against other nets' copper, the board edge and holes, **annular rings** (the copper ring around a drilled hole; exact, since pad and hole are concentric) and minimum width (the clearance between a shape's opposite sides). In plan they check footprint overlap and clearance zones such as door swings and turning circles. The rule sets belong to `geom-board` and to programs (section 26); 3D distance and clash are section 19's, in `geom-spatial`.

## 14.10 Blocks, inserts and viewports

- A **block** is a set of entities in its own frame with a base point. An **insert** places it by a transform, with an optional array of rows and columns and attribute values ([INSERT](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-28FA4CFB-9D5E-4880-9F11-36C97578252F.htm)).
- **Transforms are typed** (section 7), in `geom-math`. Exact ones keep grid points on the grid: whole-step translations, quarter turns, mirrors about axis-aligned and 45-degree lines, whole-number scales. Rational ones are exact in fractions: Pythagorean rotations, mirrors about any line with a whole-number direction, fractional scales. Any other rotation uses the closest rational rotation within ε_ang ([Canny, Donald and Ressler 1992](https://doi.org/10.1145/142675.142726)) and is Certified in angle. It is the 2D case of the kernel's one rotation type, an integer quaternion (w, 0, 0, z), so 2D and 3D rotations compose (32.14). Rational transforms compose exactly, so a rotated building keeps square corners; a composition that would exceed the construction budget is re-approximated once, as a single rational transform.
- **Rotated points round once, or stay rational where rounding would flip a decision.** Section 7 rounds a rotated point to the grid once; section 6.3's re-check keeps the rational implicit point wherever rounding would flip a decision, which includes every parallel or perpendicular pair of edges. Inserts and viewports keep their rotation as a transform, so their geometry is not rotated at all (32.14).
- **Transforms can change kinds.** A non-uniform scale turns a circle into an ellipse and an arc into an elliptical arc, with axes from the closed-form principal axes of the 2×2 matrix; where those are irrational they are rational choices and the result is Certified. A negative determinant reverses arcs. A spline maps its control points and keeps its weights.
- **Instances are not copied.** Queries take (entity, transform) pairs: exact and rational transforms apply on the fly, others are made once and cached per definition version and transform. Nested inserts compose exactly and commit at most once. Names inside inserts are paths. A reference to an entity inside nested inserts holds `path`, one step per insert from the outermost in: its `InstanceKey` (section 19.12), plus the row and column for an arrayed insert. The entity's name then names it in the innermost definition. The path resolves level by level under section 20.4's rules, never by search, so a dimension to a door leaf inside a window assembly inside a facade module is stored faithfully. Section 20.4's reference gains this field. Clipping an insert by a region (DXF's XCLIP) is an exact boolean of the instance with the clip.
- A **viewport** is a sheet window onto a 2D frame or a section 18 view, with a scale as an exact ratio, a twist and any region as its clip ([VIEWPORT](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-2602B0FB-02E4-4B9A-B03C-B1D904753D34.htm)). The map from view to sheet is a rational transform applied implicitly; a twist is a rational rotation. Sheet points stay rational until linework must become explicit (a plot, an export), then commit to the sheet's grid, moving at most half a sheet step. Sheet annotation can measure model geometry through the viewport, as DXF's "trans-space" associativity allows.

## 14.11 Dimensions, leaders and text

- **Dimension kinds** are linear, aligned, angular by two lines or three points, radius, diameter, ordinate and arc length. Arc length is not in the DXF reference ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 1), so it is our own record; its DXF mapping belongs to section 24.
- A dimension stores its kind, snap references, placement, style key and optional text, in which `<>` stands for the measured value.
- **Values are correctly rounded.** Lengths, radii and distances between 14.2's points are algebraic numbers, so the shown digits are decided exactly by comparing the value with each rounding boundary. Angles and arc lengths are narrowed as certified intervals until the digits settle, under a budget. A circular arc's length between algebraic points is transcendental (by the Lindemann–Weierstrass theorem), so it never lies exactly on a decimal boundary and the narrowing always ends, though no practical bound on its steps is known; a spline's length or an angle can lie exactly on one. When the budget runs out, the value is shown by the style's tie rule with a typed note. A float kernel can compute 2.0049999 for a length of exactly 2.005 and show 2.00 where the rule says 2.01; this one cannot.
- **The picture is derived.** Extension lines, dimension line, arrowheads and text position are regenerated deterministically from the references and the style, and written into an anonymous block only on DXF export. **Leaders** run from a referenced arrowhead point to their annotation, attaching where the text's extents say.
- **Text.** The kernel stores the anchor, attachment (nine positions), direction, height and reference width, plus single-line text's two alignment points for aligned and fitted justification ([TEXT](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-62E5383D-8A14-47B4-BFC4-35824CAE8363.htm)). It never lays out glyphs. `engine-text` supplies an extents box and, on request, glyph outlines (quadratic and cubic Béziers, which are exactly NURBS). They reach the kernel as data through the command layer (section 5), and the operation log records them as inputs, so a replay never calls `engine-text` and never depends on which fonts a machine has. Outlines become regions for silkscreen, engraving and cutting.

## 14.12 The rest of the drafting set

| Capability | Where | How, and its class |
| --- | --- | --- |
| Explode | Kernel | An insert becomes its entities under the composed transform; a polyline becomes lines and arcs. As the transform |
| Duplicate and overlap cleanup | Kernel | The overlay merges pieces on one carrier; duplicates are removed in name order and listed. Exact |
| Reverse, polyline vertex edits, decurve | Kernel | Insert, remove or move vertices; decurve replaces arcs by chords. Exact |
| Spline fit and edit | Kernel | The versioned fit-point algorithm, following The NURBS Book's chapter 9 method. Certified |
| Blend curve between two ends | Kernel | A NURBS matching end tangents (G1) or curvatures (G2), with rational control points. Certified, continuity within ε_ang |
| Convert to arcs | Kernel | Biarc fitting ([Bolton 1975](https://doi.org/10.1016/0010-4485(75)90086-X); [Meek and Walton 1995](https://doi.org/10.1016/0377-0427(94)00029-Z)) for machine-tool arc moves and old formats, within a requested tolerance that must be accepted (14.3) |
| Multilines; wall-join cleanup in plan | Program | Kernel offsets and multi-operand booleans with provenance (14.5); which layer runs through a junction is the program's rule |
| Wipeouts; centre marks | Kernel | A region, with draw order as program data; a derived picture from a circle reference |
| Geometric tolerance frames; dynamic blocks | Program | Annotation bound by snap references; parameters that choose block variants, seen by the kernel as inserts |
| Survey constructions by bearing, distance, radius and central angle | Kernel, for section 17 | Rational directions; Certified in angle |
| Pad shapes, teardrops (tapered copper where a trace meets a pad), length tuning, solder-mask and paste expansion | `geom-board` | Regions, tangent constructions (14.3), certified arc length (14.7) and offsets (14.5) |

## 14.13 What it guarantees and never does

| Result | Class |
| --- | --- |
| Arrangements, region finding, point location, booleans of lines, arcs and ellipses | Exact on their inputs; an ellipse crossing's position, when needed, certified |
| Offsets, fillets, tangent constructions, rotations | Exact where no rational choice was needed; otherwise Certified: positions within q/2 of the ideal, tangent joins within ε_ang, every join an exact shared point |
| Anything with splines, spirals or procedural offsets | Certified within ε_fit; contacts three-way |
| Lengths, radii and distances between points | Exact values, shown correctly rounded |
| Angles, arc lengths, areas with curves | Certified intervals, shown correctly rounded under a budget |
| Hatch lines, linetype dashes, dimension pictures | Derived pictures, deterministic, not model data |
| iOverlay output | Approximate, display only |
| Clearance | An exact yes or no for lines, arcs and ellipses against lines; otherwise `Pass`, `Fail` or `Undecided` with a certified interval |

Errors are typed on section 22's shared base: `OpenBoundary` with every gap, `NoSolution` with the nearest approach, `RadiusTooLarge` with the largest radius that fits, `BudgetExceeded` with a setting that fits, `ToleranceExceeded` on Tolerant imported linework, `Ambiguous` for references, and four kinds this chapter adds: `ExactnessNotAccepted`, `NotRepresentable`, `OutOfRange` and `UnboundedResult`. An unresolved reference is model state, `UnresolvedRef` (section 20.4), not an error.

`geom-2d` never: flattens stored curves; decides topology from chords, samples or floats; uses an absolute epsilon or fakes an infinite line; merges points by rounding them into bins; rounds a construction where that would flip a decision; lowers exactness without an acceptance; skips work, substitutes a value or caps a search silently; calls a platform maths function or `engine-text`; stores a hatch's lines or a dimension's picture as truth; or takes behaviour, patterns or menus from proprietary drafting software.

## 14.14 What CADCraft teaches

CADCraft is an open AutoCAD-style drafting program in Rust (MIT OR Apache-2.0, about 194,000 lines written in two weeks, largely by AI agents). It is vetted STUDY: nothing in it is fit to port, but it shows clearly what to do and what not to ([12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md)).

- **Learn:** crate layering checked by a tool ([layers.rs](https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/xtask/src/layers.rs#L35-L75)), which Spare Kit applies to section 27's graph in CI; and one command path, every action a registered command reached alike by the interface, the command line, scripts and MCP ([cmd/mod.rs](https://github.com/storytold/cadcraft/blob/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1/crates/engine/src/cmd/mod.rs#L64-L78)), which `kit-ops` does with typed schemas instead of free-text notes. Its failures become acceptance tests, with expected values recomputed from exact mathematics, never taken from CADCraft or from observing AutoCAD: a circle hatch flattened to 158 edges with the wrong area (#1167, open); hatching that fails where lines meet an ellipse (#1690, open); and two bugs closed with their fixes unmerged (pull requests #975 and #976 still open), a broad phase that drops pairs (#929) and a false tangency between circles of very different radii (#926).
- **Avoid:** f64 with an absolute 1e-9 epsilon and ad hoc thresholds; arcs stored as angles; platform maths functions, so native and web builds differ; ellipses and splines as tessellations, so snaps land on chords and offsets become polylines; infinite lines faked as segments of ±10⁷ to ±10⁹ units; silent fallbacks and caps (an ellipse's closest point falls back to its centre, dense hatch families are dropped, and by the vetting's reading of the code the default quick trim erases a circle that a cutting edge meets only once); vertices merged by rounding into bins; and knowledge with bad provenance: DWG through acadrust, which follows GPL LibreDWG source ([12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md), formats-legal), behaviour taken from observing a running AutoCAD, and a pattern library that appears to repeat Autodesk's. Spare Kit's 2D behaviour comes from the public DXF reference and published methods only; DWG is the owner's decision (section 24).

Also worth study: csgrs's hypercurve (Apache-2.0), an exact planar kernel with arcs, booleans, offsets and fillets, but 476,000 lines with open blockers and tests not run in CI ([11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md)); and geo's boolean regression tests (MIT OR Apache-2.0) with the JTS XML suite it carries (EDL-1.0, [LICENSES.md](https://github.com/locationtech/jts/blob/5a0a73632216e3e9d277150336078ec423fd215f/LICENSES.md#L7)), which become adversarial inputs here. CGAL and LibreCAD are GPL: their methods are studied through papers and public documentation only, and their source is not read (29.2).
