# 12. Intersections

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Evidence: [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md) sections 3, 5 and 6; [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md) section 4; [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), parts "overview-legal", "nurbs-core" and "intersection-booleans"; [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) sections 4, 7 and 8; [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md) sections 1, 2 and 6; [10](../../../kit/notes/G01-evidence/10-github-sweep.md) sections 1 and 3; [12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md), part "geometry-2d". Primary papers and pinned source are linked where they are used.

Almost every operation on curved geometry asks four questions. Where do these two curves meet? Where does this curve pierce this surface? Where do these two surfaces cross? Which point of this shape is nearest to that point? The crate `geom-intersect` answers the first three, for every pair of geometry kinds in section 7. The fourth, projection, lives with each kind in `geom-curve` and `geom-surface` (section 27). It is specified here because it runs on the same machinery.

Surface-surface intersection (SSI) is the hardest part of any boundary-representation kernel. OpenCascade seeds it from a 10 by 10 sample grid and stops marching at tangent points ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 4). SMLib drops near-tangent loops and reports success on truncated curves ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "intersection-booleans"). No open project offers certified, deterministic NURBS SSI ([10](../../../kit/notes/G01-evidence/10-github-sweep.md), section 3, gap 3). Spare Kit leads here, under one rule:

**Every part of an answer is returned with a proof, or as a flagged region that says why it could not be proven. Nothing is dropped, guessed or silently trimmed.**

Two terms carry the whole chapter. An **enclosure** is a box, in parameter space or in grid steps, proven to contain the true value. Every point and parameter this crate returns is an enclosure, never a bare number. A **proof** is either exact integer arithmetic (section 6.2) or interval and Bernstein bounds rounded outward (section 6.4). Bounds can prove that a quantity is not zero, but never that it is exactly zero. So every "exactly zero" decision (two surfaces touch, a curve lies in a surface, two surfaces coincide) is made by exact arithmetic on exact data, or by identity of the defining data, or it is not claimed.

## 12.1 What the crate does, and what it leaves to others

- **Operands.** A curve, a surface, or an implicit function (one given by an equation F(x) = 0), each restricted to a box of its parameter space. The crate works on carriers (the underlying curve or surface), not on faces. Trimming a result to a face's loops is section 11's job, which uses this crate's 2D curve-curve intersection in parameter space to do it.
- **Kinds are reached through behaviour traits.** Section 7 lists what a kind can do but names no traits, so the trait names here are a proposed amendment to section 7. `ParamCurve` and `ParamSurface` give evaluation, derivatives, and conservative bounds on the value and derivatives over any parameter box. A derivative bound may be `Unbounded`, as section 16.1's subdivision limit surface reports for any box that touches an extraordinary vertex; the kind then gives a normal cone in its place. `ImplicitBound` gives the interval value and gradient of F over a 3D box, and whether F is smooth there. `ImplicitBound` is defined in `geom-surface`, and planes, quadrics, tori and `geom-field`'s fields implement it. So subdivision surfaces (section 16) and fields take part without `geom-intersect` depending upwards. This makes `geom-field` depend on `geom-surface`, which neither section 16.6 nor section 27 lists yet. That dependency is a proposed amendment to both, and it makes no cycle. The closed-form routes of 12.7 ask each operand for its analytic data when it has some.
- **Pairs come from the caller**, found with bounding-volume hierarchies (sections 11 and 19). They usually arrive as a batch with one result per pair, so one hard pair does not fail the batch.
- **Frames.** Operands in different frames are first converted into the finer frame: a factor of 25 per rung (section 6.1). That is exact only when the frames differ by an exact transform (whole-step shifts, quarter turns and mirrors, section 7) and the operands fit the finer frame's range (within 2^40 of its steps: ±6.87 km at q1). Otherwise the pair fails with `FrameRange` (12.16), or, on request, is converted to the coarser frame instead; its items are then Certified, with that rounding in their bound (as in 19.12). Every threshold below uses the grid step q of the frame the pair runs in.
- **Intersections known by construction** (a fillet's contact curves, a sweep's seams, an extrusion's side edges) are passed in. They are excluded from the search only where 12.11 proves nothing else is there.

What is not here:

- Plane-based polyhedral arrangements: `geom-poly` (section 11).
- Exact 2D line and arc predicates for drafting: section 14. This crate shares them.
- Field with field: section 16. Mesh with mesh: sections 11 and 19.
- A curve draped on a terrain model (a TIN, a triangulated irregular network). Section 17 does this in `geom-terrain`: it intersects the plan-view curve with the triangle edges through 12.5, then evaluates each triangle's plane exactly.
- The nearest point on a trimmed face. This is a `geom-topo` query. It projects onto the carrier (12.4), classifies the foot against the face's loops, and falls back to the boundary edges and vertices.
- Nearest point, distance and clearance between bodies: `geom-spatial` (section 19).
- Nearest point within a point cloud: `geom-cloud`. Cloud points are projected onto curves and surfaces by 12.4. Clouds are never intersected.

Where each program meets intersections:

| Program | Typical intersections |
| --- | --- |
| Buildings | Plane with plane (exact); sloped cuts through extruded profiles; curved walls (plane with cylinder; two extrusions with parallel directions); fillets on mullions and handrails |
| Survey and civil | Clothoid alignments with planes; station and offset of a point (projection onto an alignment) |
| Mechanical parts | Quadric pairs (crossing holes, bosses, pipe tees); fillets; threads (a helix with a plane and a cylinder) |
| Cars, Class-A | NURBS with NURBS at shallow angles; curvature-continuous blends that meet nearly tangent |
| Manufacturing | Silhouettes and parting lines; slicing planes through anything; offset tool surfaces; tool envelopes |
| Circuit boards | 2D lines and arcs (section 14); vias as cylinders through planes |
| Free-form | Subdivision surfaces, sweeps, offsets, blends and fields, through the general paths |

## 12.2 The call and its result

```rust
pub fn intersect(ctx: &IntersectCtx, a: &Operand, b: &Operand, opts: &IntersectOpts)
    -> Result<Intersection, IntersectError>;

pub struct IntersectCtx<'a> {
    pub step: GridStep,          // q of the frame the pair runs in (12.1)
    pub budget: WorkUnits,       // this pair's own budget (12.15)
    pub cancel: &'a CancelToken,
    pub mode: Mode,              // Default, or Certify (12.14)
    pub ceiling: Steps,          // the largest bound the caller accepts: ε_fit unless raised
}
pub struct Operand<'a> {
    pub geom: Geom<'a>,          // &dyn ParamCurve, &dyn ParamSurface or &dyn ImplicitBound
    pub domain: ParamBox,        // where to search
    pub bound: Steps,            // how far this data may be from the true shape; 0 for exact data
    pub key: CanonicalKey,       // fixes the operand order (12.15)
}
pub struct IntersectOpts<'a> { pub known: &'a [KnownCurve] }   // 12.11

pub struct Intersection {
    pub items: Vec<Item>,        // in canonical order (12.15)
    pub coverage: Coverage,      // CompleteOutsideFlagged, or TopologyCertified
    pub diagnostics: Vec<Diag>,  // route changes, ties broken, escalations
    pub work: WorkUnits,         // deterministic count of the work done
}
pub enum Item {
    Point   { at: ParamBox, xyz: GridBox, contact: PointContact, class: Exactness },
    Curve   { curve: Curve, contact: CurveContact, ends: [CurveEnd; 2], class: Exactness },
    Overlap { region: OverlapRegion, proof: Coincidence, class: Exactness },
    Flagged { at: ParamBox, xyz: GridBox, why: Unresolved },
}
pub enum PointContact { Crossing, IsolatedTangent, BoundaryTouch, Singular { branches: u16 } }
pub enum CurveContact { Crossing, Tangent { band: Steps, gap: StepsInterval } }
pub enum Coincidence  { Identical, Within(Steps) }
pub enum Unresolved {
    TangentBand { width: Steps },  HigherOrderContact,  NearCoincident { gap: StepsInterval },
    NearMiss { gap: StepsInterval },  BoundExceeded { needed: Steps },  KnownCurveUnproven,
    UndecidedAtFloor { test: Test },   // the test that could not decide: exclusion, loop freedom or uniqueness
    NonSmooth { at: Degeneracy },      // a field crease, a pole, an extraordinary vertex
}
```

- **`ParamBox`** has one interval per unknown: two for curve with curve (t₁, t₂), three for curve with surface (t, u, v), four for surface with surface (u₁, v₁, u₂, v₂). `GridBox` is the same thing in space, in grid steps. `Steps` is a length in grid steps.
- **`CurveEnd`** says why a curve ends: at a parameter boundary, at a singular point, at a flagged region, at a seam or knot line where it continues (12.10), or nowhere (a closed loop).
- **`Curve` is section 7's curve enum.** A result with a closed form is returned as its own kind: Line, Circle, Ellipse, Parabola or Hyperbola, never wrapped. Only results with no closed form become the procedural intersection curve (12.12).
- **Contact types** say how the two shapes meet:
  - Crossing: they cross at an angle.
  - IsolatedTangent: they touch at one point without crossing, like a bowl resting on a plane.
  - BoundaryTouch: they meet at one point only because both domains end there.
  - Singular: several branches meet. A saddle cut by its tangent plane has four half-branches; a cylinder tangent to a torus along a circle has six.
  - Tangent (a curve): they touch along a curve, like a cylinder lying on a plane.

  The list was checked against SMLib's taxonomy ([SmSurfaceIntersector.h L52-112](https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmSurfaceIntersector.h#L52-L112), [SmTypes.h L862-880](https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmTypes.h#L862-L880)) and re-derived. SMLib stores branch directions in a fixed array of eight; ours has no limit.

| SMLib | Spare Kit |
| --- | --- |
| CROSSING point; CROSSING curve | A point on a `Curve` with `Crossing` |
| TANGENT_POINT | `Point` with `IsolatedTangent` |
| TANGENT_CURVE point; TANGENT curve | `Curve` with `Tangent` |
| SINGULARITY | `Point` with `Singular`; the curves through it end there |
| TOUCHING point (at the natural boundaries) | `Point` with `BoundaryTouch` |
| COINCIDENT curve (patches abutting with tangent continuity) | `Curve` with `Tangent` lying on both boundaries |
| COINCIDENCE point; REGION_BOUNDARY curve | `Overlap` and its boundary |
| NEAR_TANGENT | No kind of its own: a `Crossing` curve whose bound reflects the small angle, or a flagged `TangentBand` |

**Exactness of items.** A decision (is there an intersection here, and of which contact type) is Exact when exact predicates make it on exact data. A result is classed as follows:

- **Exact** only when the result is itself exact data. That means implicit rational points and lines (section 6.3): two planes meeting, a line meeting a plane, a tangent point or tangent line of two natural quadrics (a double root of a quadratic, hence rational), and overlaps proven by identity. Like every construction, they are rounded only when committed, and the result then says so.
- **Certified(ε)** for everything else. This includes every algebraic point (an arc-arc crossing involves a square root), conics whose radius involves a square root, and everything in parameter space (pcurves and parameter enclosures), which is never on the grid.
- **Approximate** only when an input is, or when the caller asked for it because a maths function the pair needs has no proven error bound yet (12.3).

Flagged items carry no class. They assert nothing except that the answer inside their proven enclosure is undecided.

**How bounds add up.** Section 9 says a result's class is the least exact of its inputs, and that bounds add up. An operand's `bound` is zero when the crate intersects its exact defining data. When the crate intersects a cache (a blend's fitted NURBS, say), the bound is that cache's certified error. Let θ_min be a proven lower bound on the crossing angle over an item: between the two surfaces, between the curve and the surface, or between two curves in a plane. Then the item's bound is

ε_out = ε_comp + (ε_A + ε_B) / sin θ_min + second-order terms,

where ε_comp is the crate's own certified computation error and the second-order terms are bounded from the curvature bounds over the item's enclosure. Moving two surfaces by ε_A and ε_B moves their crossing by at most (ε_A + ε_B)/sin θ to first order. Tangent contact is the limit θ → 0, where the band grows like a square root instead (12.9). Every item carries Certified(ε_out). If ε_out exceeds the ceiling, the crate returns a flagged `BoundExceeded { needed }` instead. The calling operation then either refuses with `ToleranceExceeded` (section 6.5), or its caller raises the ceiling explicitly. ε_ang (section 6.5) is never a decision threshold here: every angle decision is an interval bound or exact.

**Flagged regions are not errors.** They are part of a complete, honest answer, because everything outside them is proven. The calling operation (section 11, 13 or 18) resolves them in certify mode (12.14), or returns an `OpError` that names the region and its reason.

## 12.3 Shared machinery

All four questions run on one engine of bounds, tests, refinement and subdivision. Intervals, the uniqueness test and the work-list engine live in `geom-num`. Bernstein bounds, Bézier extraction, subdivision and symbolic products (multiplying B-splines exactly, as polynomials) live in `geom-nurbs`. Projection in `geom-curve` and `geom-surface` therefore rests on the same proofs as `geom-intersect`.

**Bounds.** Every kind gives a conservative bound over any parameter box, and bounds on its derivatives (section 7):

| Kind | How it is bounded |
| --- | --- |
| Polynomial and rational (NURBS, Bézier) | Split into Bézier pieces (NURBS Book A5.6). The Bernstein form of a piece (its coefficients in the Bézier basis) bounds it by the convex hull of its control points. Derivative bounds come from the hodograph, the derivative's own control points. Positive weights keep the hull property for rational pieces. Bernstein form is also better conditioned than power form ([Farouki and Rajan 1987](https://doi.org/10.1016/0167-8396(87)90012-4)) |
| Analytic (line, conic, plane, sphere, cylinder, cone, torus, helix) | Closed-form extents over a parameter interval. Where an extent rests on the sine or cosine of the parameter (a helix, or any kind parameterised by angle), it needs the proven-bound maths functions below |
| Transcendental (clothoid and the IFC spirals) | Interval enclosures of the Fresnel integrals, using series and asymptotic expansions with their remainder bounds (the split Bertolazzi and Frego use, BSD-2). Gauss-Legendre quadrature, with its remainder bounded from an interval bound on the high derivative. The sines and cosines inside need the proven-bound maths functions below: section 21's build piece, the same one section 17.8's spirals rest on |
| Procedural (offset, sweep, blend, intersection curve) | From the definition (the base geometry's bound, offset by the distance), or from the cache's bound widened by its certified error |
| Subdivision limit surface | Regular patches are exact bicubic B-splines. Near an extraordinary vertex, the convex hull of the local control points at each subdivision level (12.10) |
| Field | Its interval value over the box (section 16) |

**Where the arithmetic is exact, and where it is certified.** Control points sit on the grid. A Bézier piece whose extraction fits a derived fixed-width budget has exact rational coefficients, held as integers over one common positive denominator. So does its substitution into a plane's or a quadric's equation, and all of its signs, zeros included, are then exact. That holds for single-span pieces, and for knot vectors whose extraction stays within the budget. Each exact routine's module carries its bit-budget derivation and an extreme-value test, as section 6.2 requires.

Everywhere else, coefficients are computed in `f64` with outward rounding: `next_up` and `next_down` after every operation. That path proves every sign it reports, but never a zero. Each box is evaluated relative to a local origin, which is an exact integer shift, so rounding error scales with the box rather than with its distance from the frame origin.

**Maths functions need a proven bound.** An enclosure may use a maths function only once that function has a proven error bound in ulps (units in the last place). The enclosure is then widened by that bound. Principle 2 allows nothing less: a library's documented or tested error is a claim, not a proof.

- `sqrt` qualifies today, because IEEE 754 rounds it correctly (section 21).
- The pinned `libm`'s `sin`, `cos`, `atan2`, `exp` and `log` do not. They come from FreeBSD's msun ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 7), whose error bounds are documented, not proven (section 16.2).
- They qualify once section 21's proven-bound maths piece lands: `sinpi`, `cospi`, `atan2`, `exp` and `log` with proven bounds, and later the correctly rounded CORE-MATH ports, `sin` and `cos` among them (section 29.4).
- Until then, an operation that needs a missing function fails with `UnprovenMaths` (12.16), naming the function and that piece (section 17.2's rule). As section 9 requires, the error offers the explicit alternative. On request, the pair runs, and every item that rests on the function, with the coverage claim for the boxes it decided, is Approximate: its deviation is measured against a correctly rounded reference, not proven (section 16.2's rule). Nothing that rests on such a function is ever labelled Certified.

This rule is proposed for 6.4 and for section 21's maths-module row, so that every chapter cites one place. Sections 16.2 and 17.2 already follow it.

**Tests.**

- **Exclusion.** If two bounds are disjoint, or a Bernstein polynomial's coefficients all share one sign, the box holds no solution. This is a proof, not sampling.
- **Uniqueness.** This is the interval Newton test in Krawczyk's form ([Krawczyk 1969](https://doi.org/10.1007/BF02234767)). Let Y be the inverse of the Jacobian at the box's midpoint. If the Krawczyk operator maps the box X strictly inside itself, X holds exactly one solution, and r = ‖I − Y·J(X)‖ < 1 bounds how fast the iteration contracts. Every isolated point this crate returns has passed this test.
- **Refinement.**
  - In one unknown: bracketing in the manner of Brent (1973), written from the book. A Newton step is accepted only inside the bracket.
  - In two to four unknowns: the Krawczyk operator, or the Hansen-Sengupta operator ([BIT 21, 1981](https://doi.org/10.1007/BF01933165)), is iterated on the box itself. Each step is then certified and shrinks the enclosure. A point Newton step is only an accelerator, kept when it lands inside the proven box.
  - The small linear solves use Gaussian elimination with partial pivoting, written from Golub and Van Loan. Nothing comes from Numerical Recipes.
- **Caps.** Each box gets its own iteration cap, computed from its contraction factor: ⌈log(target width / width) / log r⌉, plus a margin named in `geom-num`. Hitting the cap is an error (12.16), never a best-so-far answer.

**The subdivision engine.** Boxes wait on an explicit work list, never in recursion, so the stack stays bounded on WebAssembly (principle 13). Boxes are split at their midpoints and processed in a fixed order: depth first, lower parameter first, u before v. Each pop counts work units and checks the cancellation token.

**The resolution floor.** A box stops splitting when its image in space is smaller than one grid step of the operation's frame.

- Nothing smaller than one step can be stored. Sections 6.1 and 6.5 make one step the true minimum edge length, so splitting further cannot change what is committed.
- Near the floor, a kind's certified evaluation error must stay below a quarter step. This limit is derived from 6.5's commit-rounding row: it is half of q/2, so commit rounding (at most half a step) stays the largest error, and the two together stay below three quarters of a step, inside the one-step floor. Where it would not, that box is evaluated in double-double arithmetic, which section 6.1 already uses for frame conversion.
- A box still undecided at the floor becomes part of a flagged `UndecidedAtFloor` region, which names the test that could not decide.
- Long tangent regions never reach the floor box by box: 12.9 classifies them early, on boxes far larger than a step.

The floor is a property of the grid, not a tuned depth. Section 27 puts every threshold on the ladder, so both values are proposed as rows of 6.5, pending an owner's decision: "Resolution floor, 1q" and "Evaluation limit near the floor, q/4". Until then, `geom-num` names them as constants derived from the rows they come from: the true minimum edge length of one step, and commit rounding.

## 12.4 Projection and inversion

Projection finds the nearest point, or points, of a curve or surface to a given point. Inversion finds the parameter of a point already known to lie on it. Both serve snapping, dimensions, station and offset on alignments, the checker's vertex-on-surface test, and fitting to point clouds.

- **Analytic kinds use closed forms.** Lines and planes are exact: the foot of the perpendicular is a rational point, rounded on commit. Circles, spheres, cylinders, cones and tori are certified. Where the nearest points form a set, it is returned as a typed result: a point on a cylinder's axis has a whole circle of nearest points.
- **Everything else follows [Ma and Hewitt 2003](https://doi.org/10.1016/S0167-8396(03)00021-9), made certified:**
  1. Split the curve or surface into Bézier pieces. Transcendental kinds use their own spans, with interval bounds.
  2. Keep the best upper bound found so far, the distance to an evaluated point. Discard every piece whose convex-hull distance, a lower bound, is larger.
  3. In each remaining piece, an interior foot satisfies f(t) = C′(t)·(C(t) − P) = 0. For polynomial kinds f is itself a Bernstein polynomial, and if its coefficients share one sign there is no interior foot. This is the certified form of the turning-angle culling in SMLib's global solver ([SmCurve.cpp L10841-11603](https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmCurve.cpp#L10841-L11603)).
  4. Isolate each candidate root, prove it unique, and refine it (12.3). Surfaces solve two equations by Newton's method (The NURBS Book §6.1). NLib is credited, not followed (12.17).
  5. Add the ends of a curve, and the boundary curves and corners of a surface patch, to the candidates. A minimum on the boundary need not be the foot of a perpendicular.
  6. Prove the global minimum: every discarded candidate has a lower bound above the reported distance. If several candidates' distance enclosures overlap, all of them are returned, sorted by parameter. A caller that wants one answer gets the lowest parameter, and the result says a tie was broken. Where the uniqueness test still fails at the floor, the result is a typed `NonIsolated { enclosure }`. That happens when the nearest points form a continuum on a general NURBS, such as a point on the axis of a revolved NURBS.
- **What it replaces.** OpenCascade's [`Extrema_GenExtPS`](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKGeomBase/Extrema/Extrema_GenExtPS.hxx#L47-L58) seeds from a sampling grid and finds every extremum only if that grid "is great enough". NLib's global search samples and runs Newton from the three best candidates ([NL_CrvGeom.cpp L876-1471](https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvGeom.cpp#L876-L1471)).

Stationing a point against a 50 km alignment is the hardest everyday case, because a clothoid has no polynomial form. Its interval bounds (12.3) make the same pruning work, so a station is certified to the frame's grid once the proven-bound maths functions of 12.3 exist. Until then, a station that needs them fails naming that piece, as section 17.2 says.

## 12.5 Curve with curve

- **Closed forms** cover lines, circles and conics with each other. In a plane, line and arc decisions use section 14's exact predicates: whether and how often two curves cross, and in which order along each curve. Arc crossing points involve a square root, so they are Certified.
- **General polynomial pairs** use Bézier clipping ([Sederberg and Nishita 1990](https://doi.org/10.1016/0010-4485(90)90039-F)). Each step builds a **fat line** around one curve: the thinnest strip containing it, taken from the Bernstein coefficients of its distance to a line. The step then cuts away the parts of the other curve's parameter range that lie outside the strip. Clipping converges quadratically at transversal crossings (Schulz 2009; unverified, [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md) section 3). When a clip removes too little, the engine bisects instead. Every candidate is finished by the uniqueness test and refinement.
- **Kinds without a Bernstein form** (clothoids, IFC spirals, helices, offset curves) use the same engine on interval bounds. Boxes are split using the kind's bound and derivative-bound traits, under the same exclusion and uniqueness tests and the same certificates. Bézier clipping is only an accelerator for polynomial pieces.
- **Tangency.** Where the tangents' cross product cannot be excluded from zero, the box is a tangency candidate. 12.9's second-order test decides whether the curves cross, touch, or remain unresolved. Curves that touch without crossing produce no sign change, which is why sample-and-bisect methods miss them. CADCraft misses ellipse and spline tangencies this way, and reports a false tangency for circles of very different radii ([12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md), "geometry-2d").
- **Overlap** is tested first by identity: the same carrier, or the same circle by exact comparison of its grid data. That is Exact. Otherwise the crate uses a certified two-sided distance of at most the ceiling, `Within`, which is not proven identical.
- **In 3D, a meeting is decided only where it can be proven.** Two generic space curves meeting is not stable under any perturbation, so a tolerance would be deciding topology.
  - Line with line, and conics proven by an exact test to share a plane, are decided exactly.
  - Two curves known to lie on one surface, such as two edges of one face, are intersected as curves in that surface's parameter space.
  - Otherwise, a pair whose distance enclosure contains zero, but where zero cannot be proven, is flagged `NearMiss` with its gap.
  - Only an imported Tolerant edge (section 6.5) may count a gap within its recorded tolerance as a meeting, and the operation log records it.

  Vertices of solids are never found this way. They are where three surfaces meet, or where a curve meets a surface, and both are well-posed questions.

## 12.6 Curve with surface

- **Closed forms.**
  - A line with a plane gives an exact implicit point (section 6.3).
  - A line with a quadric gives a quadratic, and a line with a torus gives a quartic. With grid data, two questions are decided exactly within a derived bit budget: how many real roots there are, and whether any root is double (a tangency). The tools are the discriminant and Sturm's method (a sequence of polynomials whose sign changes count the real roots in an interval). Outside the budget the filtered path decides, and the result is Certified.
  - A NURBS curve with a plane gives a Bernstein polynomial; positive weights do not change its signs. It is exact where 12.3's budget holds.
- **General pairs** use the Interval Projected Polyhedron solver, IPP ([Sherbrooke and Patrikalakis 1993](https://doi.org/10.1016/0167-8396(93)90019-Y); [hyperbook §4.9](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node52.html)). IPP solves C(t) = S(u, v) in Bernstein form. For each unknown in turn, it projects the control points onto that unknown's axis, and clips the box to where their convex hull crosses zero. It needs only Bernstein subdivision and a 2D convex hull, and it never discards a root. Bézier clipping on patches ([Nishita, Sederberg and Kakimoto 1990](https://doi.org/10.1145/97880.97916)) was published for rays. Clipping against fat planes generalises it to curves; that generalisation is ours, is proven in its build piece, and serves only as an accelerator.
- **Kinds without a Bernstein form** use 12.5's interval engine. A helix on a coaxial cylinder of the same radius lies in it: that is an `Overlap`, decided exactly from the grid data (same axis, same radius). A helix with a general plane is a transcendental equation, solved on interval bounds.
- **Grazing**, where the curve touches the surface without crossing, and **lying in the surface** are handled as in 12.5: the second-order test, then identity, then certified distance.

## 12.7 Surface with surface in closed form

Most of a building, and most machined parts, are planes, cylinders, cones, spheres and tori. Their intersections are computed in closed form. With exact defining data, every type decision is an exact integer test: whether a plane is tangent to a cylinder, whether two spheres touch, whether two axes are parallel or meet. SMLib fails exactly here ([issue #16](https://github.com/NVIDIA-Omniverse/usd-brep/issues/16), a tangent cylinder and box); Spare Kit decides it exactly.

**Which data is exact:**

- points and radii in whole grid steps;
- directions as integer vectors, for example taken from two grid points;
- planes with integer coefficients;
- a cone given by its apex, an integer axis vector and a rational slope (the tangent of its half-angle).

A cone defined by an angle, as STEP writes it, or any other inexact data, takes the filtered path, and its decisions are then Certified. The **natural quadrics** are the plane, sphere, cylinder and cone. In every row below, the type decision (empty, touching, crossing, which conic) is Exact for exact data. A result is Exact when it is rational, as described in 12.2, and Certified otherwise.

| Pair | Method | Result |
| --- | --- | --- |
| Plane with plane | Exact parallel test; the line is the meeting of the two planes, kept implicit (section 6.3) | Line, overlap, or nothing |
| Plane with sphere; sphere with sphere | Centre and squared radius as exact rationals; empty, touching or circle by exact sign | Circle, or touching point |
| Plane with cylinder or cone | Conic sections, by exact sign tests | Lines, circle, ellipse, parabola or hyperbola |
| Natural quadrics whose intersection contains conics: coaxial, parallel axes, axes meeting (including two cylinders of equal radius whose axes cross, as in a pipe tee) | The geometric method of Goldman and Miller, which detects and computes every conic in the intersection of two natural quadrics ([SMA '91](https://doi.org/10.1145/112515.112545); [GMIP 57(1), 1995](https://doi.org/10.1006/gmip.1995.1006)) | Circles, lines and conics |
| Any two quadrics in general position | The near-optimal parameterisation of Dupont, Lazard, Lazard and Petitjean (J. Symbolic Computation 43(3), 2008: [I](https://doi.org/10.1016/j.jsc.2007.10.006), [II, the classification of pencils](https://doi.org/10.1016/j.jsc.2007.10.012), [III](https://doi.org/10.1016/j.jsc.2007.10.007)), taken as method only, from the papers. Its predicates on the pencil of the two quadrics (the family λA − B of their matrices) need several thousand bits at 40-bit inputs, by a rough count. The build piece derives the budget; until then the filtered path decides | A quartic: an intersection curve whose definition is the exact parameterisation. The number of components and singular points is Exact within the budget |
| Torus with plane, sphere, cylinder or torus in special positions: coaxial; a plane through the axis or perpendicular to it; Villarceau planes (the tilted planes that cut a torus in two circles) | Closed-form circles | Circles |
| Surface of revolution with a coaxial quadric, or with a plane perpendicular to its axis | Reduced by symmetry to curve with curve in the profile's half-plane (12.5) | Circles at each root |
| Surface of revolution with a plane containing its axis | Copies of the profile | The profile's own kind, mirrored |
| Extrusion with a plane parallel to its direction | Reduced to the profile with the plane's trace line | Lines along the direction; Exact where the profile is lines |
| Extrusion with extrusion, or with a cylinder, of parallel direction (curved walls meeting in plan) | Curve with curve of the two profiles, in the cross-section plane (12.5) | Lines along the direction at each crossing; Exact where the crossing is rational |
| Extrusion with any other plane | The cut is an affine copy of the profile | The profile's kind (lines stay lines, arcs become ellipses) |
| Everything else: a torus in general position, NURBS, sweeps, offsets, blends, subdivision surfaces | The general paths, 12.8 | Certified |

GeometricTools (BSL-1.0) has derivations for many of these queries. They are ported and re-certified with our predicates ([10](../../../kit/notes/G01-evidence/10-github-sweep.md), section 1). SMLib's analytic override list ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "intersection-booleans") is the coverage checklist. When OpenCascade's closed form fails, it falls back silently to the general path ([IntPatch_Intersection.cxx L1789-1797](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKGeomAlgo/IntPatch/IntPatch_Intersection.cxx#L1789-L1797)). Here, every change of route is recorded in the diagnostics.

## 12.8 Surface with surface: general pairs

**When one surface is implicit** (a plane, quadric, torus or field, with an equation F(x) = 0), the parametric surface is substituted into it. The intersection is the plane curve g(u, v) = F(S(u, v)) = 0 in the parametric surface's parameter plane. For NURBS against planes, quadrics and tori, g multiplied through by the positive weights is a Bernstein polynomial. It is traced with the certificates of interval methods for implicit curves ([Snyder 1992](https://doi.org/10.1145/142920.134024); [Plantinga and Vegter 2004](https://doi.org/10.1145/1057432.1057465)):

- A box where g's coefficients share one sign holds no curve (Snyder's exclusion).
- A box where ∂g/∂u or ∂g/∂v keeps one sign holds no closed loop, because there the curve is a graph over the other parameter. Its crossings with the box's sides find every piece (Snyder's parameterisability test).
- A box where the gradient of g turns through less than a right angle holds curve pieces of known topology (Plantinga and Vegter's small-normal-variation condition).
- A box where g and its gradient cannot both be excluded from zero may hold a singular point, and goes to 12.9.

Fields with creases (min, max and smooth-min are not differentiable everywhere) cannot pass the uniqueness test across a crease. Such boxes are flagged `NonSmooth`, unless the field reports which branch is active there. This is OpenCascade's implicit-parametric route ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 4), with proofs in place of sampling.

**When both surfaces are parametric** (NURBS with NURBS, sweeps, blends, subdivision surfaces), the pair solves S₁(u, v) = S₂(s, t): three equations in four unknowns. The method follows Hohmeyer's thesis ([UCB/CSD-92-681, 1992](https://www2.eecs.berkeley.edu/Pubs/TechRpts/1992/6140.html)): subdivide both surfaces until no pair of sub-patches (a sub-pair) can hold a closed loop, then find every curve from the sub-pairs' boundaries. Patrikalakis and Maekawa set out the structure ([hyperbook §5.8.2](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node108.html)). Every step is certified:

1. **Cull.** Split both surfaces into Bézier patches. Discard sub-pairs whose bounds are disjoint: boxes first, then fat-plane slabs (the thinnest slab around a patch, the surface form of a fat line).
2. **Prove that a sub-pair is settled.** The **Gauss map** of a patch is the set of its unit normals. It is bounded from the normal patch Sᵤ × Sᵥ, itself a B-spline built by symbolic products ([Piegl and Tiller 1997](https://doi.org/10.1016/S0010-4485(96)00074-7)). A closed loop of intersection needs a **collinear-normal pair**: a point on each surface where both normals lie along the line joining the points. A tangent or singular point is such a pair in which the two points coincide ([Sederberg and Meyers 1988](https://doi.org/10.1016/0167-8396(88)90029-5)).
   - If the two normal bounds exclude both parallel and opposite normals, the sub-pair holds no loop and no tangent or singular point. It is settled.
   - Hohmeyer's criterion proves loop freedom under a less demanding condition, Gauss maps that a plane can separate, so subdivision stops sooner. Its exact statement is in the thesis, which was not re-read for this note; it is re-derived and proven in its build piece. A sub-pair that passes only this test is loop-free, but may still hold a contact point with opposite normals, which step 4 looks for.
3. **Otherwise split** both patches at their midpoints, and repeat from step 1.
4. **Isolate collinear-normal points where they can be isolated.** For a sub-pair whose normal bounds overlap, the crate solves the collinear-normal system with IPP. The system says that n₂ is perpendicular to both tangents of S₁ (the normals are parallel), and that S₁ − S₂ is perpendicular to both tangents of S₁ (the joining line lies along the normal). That is four equations in four unknowns. The crate tries it as soon as the uniqueness test can succeed on the box (the contraction bound r of 12.3 is below one), not at a tuned depth. Each root found is proven unique, and the gap d along the common normal then decides:
   - d excluded from zero on the separating side: no contact there;
   - d excluded from zero on the penetrating side: a small loop surrounds the point, so both patches are split through it, and the loop then crosses the split lines;
   - d not excluded from zero: the point goes to 12.9.

   Where the system's Jacobian cannot be bounded away from singular, the collinear-normal points form a curve or a region. Along tangent contacts and coincidences, the sub-pair goes to 12.9. In transversal pairs, such as coaxial surfaces of revolution or a surface against its own offset, steps 1 and 2 settle every sub-pair as the patches shrink, so no flag results.
5. **Find start points on the boundaries.** Intersect every boundary curve of every settled sub-pair with the other patch (12.6). Because 12.6 never misses a root, every component gets at least one start point. This replaces start points seeded from triangle meshes, which miss loops smaller than the mesh: OpenCascade's `IntPolyh` uses 10 by 10 samples by default ([hxx L30-44](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKGeomAlgo/IntPolyh/IntPolyh_Intersection.hxx#L30-L44)), and verb and truck work the same way ([04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), section 3).
6. **March with certified steps.** Each branch is traced by the method of Barnhill and Kersey ([CAGD 7, 1990](https://doi.org/10.1016/0167-8396(90)90035-P); [hyperbook §5.8.2](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node109.html)). The predictor steps along the tangent N₁ × N₂. The corrector is Newton on four unknowns: the two surface points must coincide and lie on the plane across the step. SMLib's corrector uses the same formulation ([SmSurface.cpp L27439-28044](https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmSurface.cpp#L27439-L28044)). Each step is certified twice:
   - An interval step control after Kearfott and Xing ([SIAM J. Numer. Anal. 31(3), 1994](https://doi.org/10.1137/0731048)) proves that the corrector converges to the branch being traced, so a step cannot jump to a neighbouring branch.
   - The chord between consecutive points stays within what the ceiling leaves after the item's other proven terms in 12.2's ε_out: the rest of ε_comp, the operands' share (ε_A + ε_B)/sin θ_min, and the second-order terms. So the item's whole bound stays within the ceiling. For a step of arc length h, the deviation is at most h²κ_max/8, where κ_max is an interval bound on the intersection curve's curvature over the step's box. That bound comes from the standard formula in the surfaces' normal curvatures and the angle between them ([Ye and Maekawa 1999](https://doi.org/10.1016/S0167-8396(99)00018-7)).

   The step is the largest that both proofs allow.
7. **End every branch for a reason.** A branch ends at a boundary start point (which is then crossed off), back at its own start (a closed loop), at a singular point (each branch out of it is traced separately), at a seam or knot line where it continues (12.10), or where the angle between the surfaces can no longer be bounded away from zero (a tangent region, 12.9). It never ends because a counter ran out.
8. **Account for everything.** Each crossing-type boundary start point ends exactly one branch. Each grazing boundary point, where a branch touches the boundary from inside, ends two half-branches. Each singular point ends exactly as many branches as its analysis found. Everything else lies in a flagged region. A mismatch is an `Internal` error with its evidence, never a success.

Why this is complete: after steps 1 to 4, every sub-pair is disjoint, settled, split through every isolated collinear-normal point, or handed to 12.9. So every component outside a flagged region reaches a sub-pair boundary. Step 5 finds a point on every such crossing, and step 6 traces each one without jumping. The argument can fail only where a test could not decide before the resolution floor, and those places are exactly the flagged regions. The written proof is part of the build piece, including the case of a loop that touches a split line at a single point. Hohmeyer's own guarantee covers surfaces that do not meet at singular points. Here, singular points are found by 12.9's contact analysis, not by the loop criterion.

## 12.9 Tangent, singular and coincident cases

A sub-pair reaches this section when its normal bounds overlap, it cannot be excluded in space, and no isolated collinear-normal point can be proven in it. Every kernel measured so far loses answers here. On an exact-truth corpus, SMLib passes 7 of 10 tangent-curve cases, 3 of 7 isolated tangent points, 3 of 6 singular crossings and 11 of 24 near-degenerate cases ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "intersection-booleans").

- **Second-order analysis.** Where the normals are parallel, the two surfaces' second fundamental forms (their curvature in each direction) are compared in the shared tangent plane. Their difference is a quadratic form:
  - Definite: the point is an isolated tangent point.
  - Indefinite: two branches cross there, and all four half-branches are traced.
  - Zero: the contact is of higher order. Analytic pairs go to a third-order test; other pairs become a flagged `HigherOrderContact`.

  This re-derives the idea of SMLib's Dupin-indicatrix analysis (the indicatrix is the conic that shows a surface's curvature near a point). SMLib's version builds one of its conics with the same curvature on both axes (WR-69), and follows only the first branch out of a singular point ([SmAdvSurfaceIntersector.cpp L868](https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmAdvSurfaceIntersector.cpp#L868)).
- **Whether a contact exists needs exactness.** Whether two surfaces truly touch, rather than miss by a hair or cross in a tiny loop, is an equality. For the closed-form pairs of 12.7 with exact data it is decided exactly, so the common building and machining cases (a cylinder resting on a plane, a sphere seated in a cone) never reach the bands below. Fillets against their walls are known by construction (12.11). For general pairs, bounds can only show that the gap lies in an interval containing zero.
- **Tangent curves.** Along a tangential contact the intersection is ill-conditioned. Suppose the surfaces are known only to within δ (their own bounds plus the computation's), and their normal curvatures across the contact differ by at least Δκ. Then the true intersection can lie anywhere within about √(2δ/Δκ) of the contact curve. When the curvatures differ by that of a 10 mm radius, a δ of one q0 step gives about 358 steps (56 µm). The crate computes this band from its own certified δ and Δκ.
  - If the band is within the ceiling, the crate returns the contact curve with `Tangent { band, gap }`, Certified. Inside the band, the true intersection is this curve, two crossing curves, or nothing; either way the resulting body is within its bound.
  - Otherwise it returns a flagged `TangentBand` with the width.

  It never returns a tangent curve with a bound it has not proven.
- **Coincident surfaces** become overlap regions, decided in this order:
  1. The same carrier: Exact.
  2. The same analytic surface, by exact comparison of grid data: Exact.
  3. Otherwise, a certified two-sided distance of at most the ceiling over the region, `Within(ε)`: Certified.

  The overlap's boundary is each patch's boundary projected onto the other (12.4).
- **Near-coincident is not coincident.** Sometimes, even at the floor, the certified distance can neither be excluded from zero nor bounded by the ceiling. Such a region is flagged `NearCoincident` with its gap. Whether an operation may treat a `Within` overlap as coincident is that operation's decision under its ceiling, and it is recorded (sections 6.5 and 11). SMLib reports near-coincident surfaces as coincident ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), WR-78).

## 12.10 Poles, seams, knot lines and extraordinary vertices

The general paths assume a regular parameterisation: Sᵤ × Sᵥ is never zero and the derivatives are bounded. Real surfaces break that in five ways, and each has its own rule. The corpus class for seams, poles and closed surfaces must pass with no flagged region at all (12.17).

- **Knot lines where the surface is not proven tangent-continuous**, such as a knot of full multiplicity, where a NURBS is only C0. The normals jump there, so the patches are split along those lines before anything else. Marching ends at such a line and restarts on the far side from the same point.
- **Collapsed edges and poles**: a NURBS edge whose control points coincide, the pole of a sphere stored as NURBS, a cone's apex.
  - Whether an edge collapses to one point is an exact test on grid data. That point is then tested against the other surface as a point (12.4).
  - For Gauss-map bounds, the factor that vanishes along the collapsed edge is divided out symbolically. If S(u, 0) is a single point, Sᵤ vanishes along v = 0, and for polynomial patches Sᵤ = v·T(u, v). The reduced patch T × Sᵥ then bounds the normals without the degenerate zero. This is our own derivation, proven in its build piece. NLib's pole-normal evaluation, `N_SrfEvalPtNormalAtPole`, serves only as an out-of-process oracle test under rule 8 (section 29.20), run after an owner's yes.
  - Where the reduced patch still vanishes, a local re-chart is used, or the box is flagged `NonSmooth`.
- **Seams of periodic surfaces.** Patches are split at the seam. A curve that crosses the seam is joined across it: both pieces end at the same point on the seam, which is recorded once.
- **Self-touching analytic surfaces.** The lemon and apple tori touch themselves on their axis. They are split at the axis points, which are tested exactly.
- **Extraordinary vertices of subdivision surfaces**, where other than four faces meet. Stam's parameterisation is singular there. First derivatives scale like (2λ)ᵏ at subdivision level k, where λ is the subdominant eigenvalue: about 0.41 at valence 3 and 0.55 at valence 5. So the derivatives shrink to zero at valence 3 and grow without bound at valence 5 and above. (This is our derivation from [Stam 1998](https://doi.org/10.1145/280814.280945).) Bounds come from the shrinking rings of control points, level by level. The uniqueness test runs in the chart of the characteristic map, where the surface is regular. That use is our design, not a published SSI method, and it is proven in its build piece. Until then, an undecided neighbourhood is flagged `NonSmooth`.

## 12.11 Intersections known by construction

A fillet's contact curves, a sweep's seams and an extrusion's side edges are known when they are made (section 13). Finding them again would be wasted work, and for tangent contacts it would fail. But excluding them blindly could drop a real second intersection close by, such as a blend that curves back through its wall a few steps away. So the exclusion itself must be proven.

- A known curve arrives with its certified bound and its contact type.
- Along it, the crate proves that nothing else meets inside a tube of radius ρ around it.
  - For a crossing contact: the angle between the surfaces is bounded away from zero throughout the tube, so the intersection there is a single curve.
  - For a tangent contact: on the curve, the separation of the two surfaces across it has value and slope zero. An interval bound shows that its second derivative keeps one sign across the tube, so the separation has no other zero there.
- Only the proven tube is excluded. The general search covers everything outside it, and the tube's edge becomes part of the sub-pair boundaries.
- Where the proof fails on some stretch of the curve, that stretch is searched normally. If it still cannot be decided, it is flagged `KnownCurveUnproven`. Nothing is dropped.

## 12.12 The intersection curve

The intersection curve is a procedural kind (section 7). It is defined by its two surfaces and carries a cached fitted curve with a certified error. Results that have a closed form never use it (12.2).

- **Definition.** The two surfaces, the branch's ends (implicit points or boundary parameters), and the certified chain of marched points in (u₁, v₁, u₂, v₂). For the quartics of 12.7, the exact parameterisation is the definition.
- **Where it lives.** `geom-curve` cannot see surfaces (section 27). So the variant stores its two surfaces as references into the geometry pool that holds it (section 8.1), and those references are opaque to `geom-curve`. `geom-curve` evaluates, bounds, splits and converts the curve through its certified cache alone, which is all section 7's curve contract needs. Evaluating the true curve needs both surfaces, so it is a function in `geom-intersect` that takes the curve and the two resolved surfaces. Pcurves follow the same rule: `geom-curve` holds the 2D curve, and mapping it through its surface belongs to `geom-surface`.
- **Cache.** A fitted 3D NURBS and two pcurves, made in `geom-nurbs` by approximation to a stated error (The NURBS Book §9.4): interpolate, remove knots against a per-point error, fit by least squares one degree higher, re-project the parameters, and repeat.
- **Certified bound.** For each span, the distance from the fitted curve to each surface is bounded, either by Bernstein bounds of the difference or by an evaluated value plus a proven second-derivative bound ([04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), section 5). Suppose the surfaces cross at an angle of at least θ_min over the span. Then a point within d₁ of one surface and d₂ of the other lies within about (d₁ + d₂)/sin θ_min of their intersection, with curvature terms bounded over the span's box. The operands' own bounds are added as in 12.2, and the span's bound must not exceed the ceiling. Each pcurve, mapped through its surface, is certified the same way against the 3D curve, which is the edge's primary geometry (section 8.1).
- **A fit that misses the ceiling is refined, never accepted with a larger bound.** OpenCascade turns the error it reached into the new edge's tolerance ([IntTools_FaceFace.cxx L1176-1410](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/IntTools/IntTools_FaceFace.cxx#L1176-L1410)). SMLib returns degree-3 Hermite fits while its documentation claims there is no approximation ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md)). Here, if refinement exhausts the budget, the operation returns an error.
- **More precision on demand.** Any point of the true curve can be computed from the cache by refinement on the definition, proven unique (12.3). So a later operation that needs a tighter bound (a fine-frame part placed in a building, section 6.1) gets it without refitting.

## 12.13 Level curves and self-intersection

The implicit tracer of 12.8 also traces **level curves** g(u, v) = 0 of a function on a single surface. With the normal patch built by symbolic products, each g below is again a Bernstein polynomial for NURBS:

- silhouettes in parallel views, and parting lines for a pull direction d: N·d = 0;
- silhouettes in perspective views from an eye point E: N·(S(u, v) − E) = 0;
- isoclines for draft-angle checks, where N meets d at a given angle (squared, so the condition stays polynomial);
- envelope curves of a moving tool, where the normal is perpendicular to the surface's velocity v: N·v = 0.

Section 18 uses the silhouettes, section 13 the envelopes, and the draft-angle query (section 10.2) the isoclines.

**Self-intersection** serves the checker (section 8.3, group 3) and offsets (section 13).

- A curve piece cannot cross itself if some direction a has C′(t)·a > 0 throughout, because C(t)·a then strictly increases. With Bernstein coefficients, that is a proof.
- A surface patch cannot cross itself if its Gauss map lies in an open hemisphere and its boundary, projected along that hemisphere's axis, is a simple curve ([Volino and Magnenat-Thalmann 1994](https://doi.org/10.1111/1467-8659.1330155)).
- Pieces that fail these tests are split and intersected with each other by the general method, with their shared boundaries excluded as in 12.11.

## 12.14 What is promised, and what honestly is not

| Promise | Default mode | Certify mode |
| --- | --- | --- |
| No intersection outside a flagged region is missed | Proven (12.8) | Proven |
| Every point and curve lies within its stated bound, never more than the ceiling | Proven, with the inputs' bounds added (12.2) | Proven |
| Flagged regions | Possible near tangency, singular points, near-coincidence and degenerate parameterisations, each with its reason and size | Resolved into points, curves and overlaps with their true connections, or an error saying it cannot be done at this precision |
| Correct topology: how many components, how branches connect | Outside flagged regions | Everywhere it succeeds |
| Cost | Subdivision and marching | Much higher; measured by its benchmark before any default is set |

**Certify mode** guarantees that the returned curves have the same topology as the true intersection. It is opt-in per operation. An operation may also escalate a single flagged region to it, which only adds rigour and stays deterministic. It implements a published topology-guaranteed method, chosen in its build piece after reproducing the candidates on the corpus:

- Yang, Jia and Yan, "Topology Guaranteed B-Spline Surface/Surface Intersection", ACM TOG 42(6), SIGGRAPH Asia 2023 ([doi:10.1145/3618349](https://doi.org/10.1145/3618349), CC-BY 4.0). It handles crossings, isolated contacts, boundary contacts, contacts between branches, and high-order contact along a curve, and it reports cases that challenge OpenCascade and ACIS.
- Cheng, Zhang, Xiao and Li, ACM TOG 42(4), SIGGRAPH 2023 ([doi:10.1145/3592452](https://doi.org/10.1145/3592452)), which uses interval algebraic topology for rational surfaces.
- Zhang, Cheng, Liu and Zhang, "Topology guaranteed and error controlled curve tracing for parametric surface-surface intersection", CAGD 118, 2025 ([doi:10.1016/j.cagd.2025.102432](https://doi.org/10.1016/j.cagd.2025.102432)).
- Background:
  - Bartoň, Elber and Hanniel's no-loop and single-component tests ([PDF](https://gershon.cs.technion.ac.il/papers/TopolGuarSSI.pdf));
  - Grandine and Klein's topology-first tracing ([CAGD 14, 1997](https://doi.org/10.1016/S0167-8396(96)00024-6));
  - Huber and Barth's complete and guaranteed SSI ([1999](https://doi.org/10.1007/978-94-017-1247-7_15));
  - Hass, Farouki, Han, Song and Sederberg on consistent trimming ([PDF](https://www.math.ucdavis.edu/~hass/Research/Papers/GeomDesign/Consistency.pdf)).

**What is not promised:**

- Correct topology in every degenerate configuration in default mode ([04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), section 7). The topology of complex SSI is still called an open challenge in 2026 (Gao, Lin and Li, [arXiv 2604.12631](https://arxiv.org/abs/2604.12631)).
- Deciding whether two general curved surfaces truly touch. Bounds alone cannot decide it (12.9).
- Success within any budget. Some configurations of exact data may need more work than the budget allows; the operation then fails with a structured error and its remedies.

How Spare Kit leads meanwhile: it never loses an answer silently, decides analytic tangency exactly, marches with proofs, and is measured against exact truth (12.17).

## 12.15 Determinism

Every method here is iterative, so each follows these rules on top of section 21:

- **Canonical operand order.** Each pair is computed with its operands in a canonical order: by kind, then by a kernel-internal canonical encoding of the defining data. That encoding is versioned with the kernel edition (section 26.2). The order never depends on memory addresses, nor on file-format bytes, whose version can change independently. So intersect(A, B) and intersect(B, A) give the same items with the roles swapped.
- **Fixed traversal.** The work-list order, the midpoint splits and the marching direction (along N₁ × N₂, in canonical order) are fixed.
- **Budgets per pair.** Each pair in a batch gets its own budget, derived deterministically from the operation's budget: equal shares, assigned in canonical pair order. Parallel work is split by pair and collected in pair order, so the thread count changes nothing. Only cancellation stops work at a moment that depends on timing. It then returns `Cancelled`, never a partial result.
- **Derived caps, and caps are errors** (12.3). The crate never returns a best-so-far point, as NLib does alongside an error flag. It never breaks out with success, as SMLib's tracer does below a 1e-10 step ([SmSurfaceIntersector.cpp L5134-5188](https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmSurfaceIntersector.cpp#L5134-L5188)). It never quietly files a failed trace's start point away, as SMLib's 5,000-point cap does.
- **Thresholds in grid steps**, from the tolerance ladder and the floor (12.3). Never a fraction of a surface's size, and never an unexplained number.
- **Arithmetic.** Maths functions come only from the kernel's `libm` module. Intervals are rounded outward. There is no fused multiply-add unless it is written as `mul_add`, and no randomness or clock.
- **Ties** break by parameter order within one operand, and by canonical operand order across operands. A diagnostic says when a tie was broken.
- **Output order.** Items sort by kind, then by their enclosure on the first operand: lower corner, then upper corner, coordinate by coordinate, compared exactly. Enclosures of distinct items are disjoint, so this order is total. A closed loop starts at whichever of its start points comes first in this order. Section 20 uses this order for the ordinals that tell apart two curves joining the same two faces.

## 12.16 Errors

`intersect` returns `Result<Intersection, IntersectError>`, and projection follows the same pattern. Every pair of kinds is supported, so there is no "unsupported" error; `UnprovenMaths` is a temporary gap in the maths module, not a missing pair. The error kinds are:

- `Cancelled`.
- `BudgetExceeded`, with the work spent, the region still undecided, and the remedies: a larger budget, certify mode on that region, or simpler input.
- `FrameRange`: the operands do not fit the finer frame's range (12.1). The remedies are working in a frame placed at the overlap (section 11), or converting to the coarser frame on request.
- `UnprovenMaths`: the pair needs a maths function that has no proven error bound yet (12.3). It names the function and section 21's build piece that adds it, and offers the explicit Approximate alternative.
- `InvalidInput`: a broken precondition, such as a non-positive weight. The checker should already have caught it.
- `Internal`: a proof did not close (12.8, step 8), a derived cap was hit, or an invariant failed. It carries its evidence. It is a bug, and it is never returned as a success.

The calling operation wraps these into its own `OpError` (section 22), adding the faces and edges involved.

## 12.17 What we take, and how we check it

| Source | Licence | What we take | What we do not take |
| --- | --- | --- | --- |
| NLib inside SMLib | Apache-2.0. The chain of title from GeomWare to NVIDIA is unverified ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "overview-legal" and "nurbs-core") | Methods from The NURBS Book, Ma and Hewitt and the Piegl–Tiller papers, in our own words: global projection pruning, Newton projection, symbolic B-spline products, the fitting loop. NLib is consulted only for which methods exist, failure cases and test ideas until an owner signs off its chain of title, then for details (section 29.5). It is never translated. Each module that consulted it carries a credit line | Code structure, sentinels, tolerance globals, and its constants and step order before that sign-off. Never the routines derived from Numerical Recipes in `NL_Math.cpp`, named as section 29.6 names them: the SVD solve and decomposition (`N_SingleValueDecomposeSolve`, `N_RealMatrixSingleValueDecompose`), the Brent minimiser (`N_FuncFindMinima`), the eigen solver (`N_RealMatrixEigenValuesVectors`) and its pythag helper, and the tridiagonal solvers (`M_TridiagonalSystem`, `M_CorneredTridiagonalSystem`). The quadratic solver, which cites Numerical Recipes p.184, is standard and low risk, but is re-derived anyway |
| SMLib intersector | Apache-2.0; studied only | The contact taxonomy, the corrector formulation, the Dupin idea, the analytic dispatch list. Its failure catalogue and its issue reproductions (#1, #16) become test checklists, rebuilt with our own inputs | Any intersection code. Its fixtures, which are SMS-era customer files ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "overview-legal"). Its tolerances (1/1000 of surface size, 20° tangency windows) and heuristics |
| OpenCascade, LNLib | LGPL-2.1 | Method only, in our own words: dispatch by surface class, implicit-parametric marching, and OpenCascade's own list of failures | Code; sampled start points; tolerance growth |
| GeometricTools | BSL-1.0 | Closed-form derivations, ported with the notice kept, and re-certified | Its float-only decisions |
| curvo | MIT | Its curve-curve tests: scale independence, touching, near-coincident, repeat determinism ([L277-433](https://github.com/mattatz/curvo/blob/11842793ed92640cb78e8ed86d731fdb55e85e34/src/intersects/curve_curve/intersection_curve_curve.rs#L277-L433)) | Its optimiser-based solver, which is not certified. Likewise verb's and truck's mesh-seeded SSI |
| SISL and GoTools | AGPL-3.0 | Nothing; SINTEF's papers only | The code is never read |

**SMLib in short.** NLib's projection, symbolic products and fitting are sound sources of method. Its intersector is not a model to follow:

- Its Gauss-map loop test is dead code: the flag that guards it is only ever set to true ([SmAdvSurfaceIntersector.cpp L1383-1474](https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmAdvSurfaceIntersector.cpp#L1383-L1474)). That costs work rather than answers.
- Interior loops rest on one sampled candidate per pair of leaf patches, and candidates within 5° or 2° of tangent are dropped without a signal (L1184, L1311).
- Culling boxes for analytic surfaces are padded by a fixed absolute tolerance rather than certified.
- Truncated traces return success.

On the exact-truth corpus SMLib passes 81 of 119 cases. By the corpus author's rule, which does not count 9 near-degenerate cases below SMLib's own 1e-5 zone tolerance, that is about 81 of 110. The only other engine measured, SolidCraft, passes 47 ([solidcore issue #17](https://github.com/tangency-labs/solidcore/issues/17); [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md)). The detailed critique belongs in section 29.

**Oracles.** Exact truth decides; other kernels only cross-check.

- **Exact truth comes first:**
  - the [ssi-corpus](https://github.com/jyviko/ssi-corpus/tree/b04c8a6093a8b3505d865d1dd874ec3879ccb1f9): 119 SSI cases with exact ground truth and a scorer, under MIT or Apache-2.0. Its Yang, Jia and Yan cases derive from a CC-BY 4.0 paper, so attribution travels with them;
  - closed-form analytic cases of our own;
  - exact rational solutions of small polynomial systems, computed with `dashu` or `num-rational`.

  An adapter places each corpus case on the grid, at a scale where the corpus tolerance is at least ε_fit, and checks that rounding to the grid has not changed the truth. It also maps our flagged regions onto the scorer's verdicts.
- **SMLib** is not an automated comparator for SSI. Rule 8 (section 29.20) allows it only on deterministic layers: evaluation, knot operations, degree change, and fitting without random sampling. Its intersector is nondeterministic on some paths and can report success on wrong results. Its score of 81 of 119 stays only as the number to beat (section 29.15).
  - Running it on the classes where it scored 100% (transversal; seams, poles and closed surfaces; coincident regions; patch boundaries; high degree), and running NLib projection on well-conditioned inputs, are proposals. Both need an owner to amend rule 8 to allow classes whose repeated runs agree bit for bit.
  - Any SMLib run, under rule 8 as it stands or as amended, happens only after an owner's yes, because the build compiles Numerical Recipes-derived routines. It is built from pinned usd-brep and solidcore source in our own isolated container, never from the prebuilt `cpp-ref`, with new shims for the routines solidcore has not ported. It runs out of process, in fresh processes with repeated runs, and is never authoritative.
- **OpenCascade** runs out of process as a separate program, never linked, as a comparator on transversal cases.
- A disagreement is settled by exact truth, never by deferring to an oracle.

**Acceptance gate for SSI:**

- Certify mode passes all 119 corpus cases.
- In default mode, there is no crash, no error on valid input, and no wrong-topology or inaccurate verdict.
- There are zero flagged regions in the transversal, seam-and-pole, coincident, patch-boundary and high-degree classes.
- Elsewhere, every feature the scorer marks incomplete lies inside a returned flagged region. That region must contain a true feature of the matching kind, and be no wider than a derived multiple of the floor, or than the tangent band computed for that case.
- The count and total size of flagged regions per case are published, and may not grow.
- Time per case against the SMLib reference is recorded in the benchmark before any default is set.
- A mutation test checks that the gate fails when an answer is perturbed or a flagged region is enlarged.
- Every case replays bit-identically on every target in section 21.
