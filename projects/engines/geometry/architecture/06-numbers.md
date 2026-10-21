# 6. Numbers: the grid, exact predicates and tolerances

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

This section decides the foundation everything else stands on. Evidence: [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), with [01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md) for plane-based booleans and [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md) for tolerant modelling.

## 6.1 The grid

Every coordinate the kernel stores is a whole number of grid steps.

- **The standard grid step, q0, is 156.25 nanometres: 1/6400 of a millimetre.** It is the largest step that makes both 0.01 mm (64 steps) and 1/256 inch (635 steps) exact whole numbers, so metric and imperial models both round-trip exactly. One millimetre is 6,400 steps and one inch is 162,560.
- **Coordinates are `i64` in a local frame, kept within 2^40 steps of its origin: ±171.8 km at q0.** That covers any building, any site and a 50 km road corridor in one frame. Keeping within 40 bits is what lets every predicate below run in fixed-size integers, whatever the step.
- **Frames hold the real world.** Each model has a project frame. Its link to a map projection (for example NZTM2000) is a frame record: the coordinate system's ID, the origin in grid steps (an NZTM northing is about 2^45 steps), a rotation and a scale factor. Survey and map data are converted into the frame once, in double-double arithmetic, and rounded to the grid. Networks wider than ±171.8 km, such as a regional road model, use several frames on a shared lattice, so moving between neighbouring frames is an exact integer shift.
- **A ladder of grid steps, each exactly 25 times finer than the one above.** Every frame picks one:

  | Step | Size | Exact units | Range per frame | For |
  | --- | --- | --- | --- | --- |
  | q0 | 156.25 nm | 0.01 mm, 1 mm, 1/256 inch | ±171.8 km | Buildings, sites, roads, survey |
  | q1 | 6.25 nm | 1 µm, 0.01 mm, 1 mil (1/1000 inch), 1/256 inch | ±6.87 km | Mechanical parts, products, circuit boards |
  | q2 | 0.25 nm | 1 nm, and everything above | ±275 m | Reserved, for example chip layout |

  A finer step divides every coarser one exactly, so a building's coordinates convert into a part's fine frame by multiplying by 25, with nothing lost. A precision part or a circuit board placed in a building keeps its own fine frame, and a boolean between the two converts the building's geometry into the fine frame and stays exact. Converting fine to coarse rounds, and the result says so. Why 25: 156.25 nm is the largest step making 0.01 mm and 1/256 inch exact, and 6.25 nm, which is 156.25 nm divided by 25, is the largest step that also makes 1 µm and 1 mil exact (it is the greatest common divisor of 1 µm, 1 mil, 0.01 mm and 1/256 inch). This is decision 1 in section 32.
- **Angles have a unit of their own: proposed in section 17.2, pending decision (chapter 17, open question 16).** Survey angles and site-grid rotations are stored as whole **angle units** (`AngleUnits`, an `i64`), each 1/2,500 of an arc-second. It is the largest unit that makes both 0.01″ and 0.00001 gon (0.0324″) whole numbers, as q0 does for 0.01 mm and 1/256 inch. A full circle is 3,240,000,000 units, and ε_ang (section 6.5) is about a thirtieth of one unit. Finer angles round half to even to the nearest unit. The word "step" is kept for lengths only.

Why a grid and not floating point: Revit and OpenCascade store doubles and pay for it with tolerances that leak into every decision. On a grid, the data the kernel decides from is exact, and the only rounding happens at known points: when a new point is committed (at most half a step per coordinate, 78 nm at q0; as a distance in 3D, at most (√3/2)q, about 135 nm) and when curved geometry is fitted (at most the certified bound below).

## 6.2 Exact predicates

A predicate is a yes, no or "exactly on it" question: which side of this plane is this point, does this point lie inside this circle. Topology is decided only by predicates, and every predicate in the kernel is exact.

- **They return a sign, never a number:** `Negative`, `Zero` or `Positive`.
- **They are computed in fixed-size integers.** With coordinates under 2^40 (B = 40 bits), orientation in 2D and 3D fits in `i128`; circle and sphere tests in 256 bits; the deepest boolean test (a point made by three planes, tested against a fourth plane) in 384 bits. That is the budget for booleans and every arrangement predicate (section 11.3). Each predicate's module carries the written derivation of its bit budget and a test at the extreme values (every coordinate at ±(2^40 − 1)). Evidence: the bit-budget table in [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 4, matching EMBER's published 256-bit bound at 26-bit inputs.
- **Planes have a budget too.** A plane through three grid points has integer coefficients, with a normal of at most 2B+3 = 83 bits and an offset of at most 3B+5 = 125 bits at B = 40 ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 4). The 384-bit budget assumes these limits, so the plane type checks them at construction.
- **Some predicates outside booleans need wider integers.** Other chapters propose these, still fixed-size and behind the same generated filters, pending an owner's decision:

  | Predicate | Bits at B = 40 | Type | Proposed in |
  | --- | --- | --- | --- |
  | Order of two straight-skeleton (roof) events by height (two three-plane points) | 13B+27 = 547 | `I576` | 13.15; chapter 13, open question 2 |
  | Order of two hidden-line events along an edge, view along a grid axis | about 13B+27 = 547 | `I576` | 18.6; chapter 18, open question 1 |
  | The same, for any view direction and for perspective | about 15B+33 = 633 | `I640` | 18.6; chapter 18, open question 1 |
  | Order of two mitre-offset events in time (four moving planes) | about 720 | `I768` | 13.11; chapter 13, open question 2 |
  | Pencil predicates for two quadrics in general position | several thousand, by a rough count | To be derived | 12.7; chapter 12, open question 7 |
  | 2D predicates on rational constructions (rational directions, rational centres) | not yet derived; possibly several thousand (unverified) | To be derived | 14; chapter 14, open question 2 |

  Each needs what the 384-bit tests have: a written derivation of its bit budget, a test at the extreme values and a generated filter. Each also costs more. By section 23.2's rule, a schoolbook product grows with the square of its number of 64-bit limbs, so a product in `I576`, `I640` or `I768` (9, 10 or 12 limbs) costs about 5, 6 or 9 times one in 256 bits. Section 23's cost model, section 28.7's proofs and section 29.3's list of wide integers cover only `I256`, `I320` and `I384` so far; each wider type an owner accepts joins them there.
- **A cheap filter goes first.** Each predicate first runs in `f64` with a semi-static error bound (the method of Meyer and Pion's generator, as used in Geogram's PCK, BSD-3) and returns at once when the sign is certain. Only uncertain cases fall through to the exact integer path. There is no interval stage and no change of rounding mode, which Rust forbids and WebAssembly cannot do.
- **Filter constants are generated, not hand-typed**, from a small description of each predicate, the way Geogram's PCK generator does it. Hand-typed error bounds are where predicate libraries go wrong.
- **Degenerate ties are handled on purpose.** Where an algorithm needs every sign to be non-zero (Delaunay triangulation, ray parity), it uses symbolic perturbation (Simulation of Simplicity, Edelsbrunner and Mücke 1990), ordered by persistent names, never by memory address as Geogram's default does. Where a tie means something real (two walls sharing a face), booleans handle it explicitly, because a perturbation would turn a shared face into an arbitrary tiny gap or overlap.

## 6.3 Constructions stay implicit until committed

A construction makes a new point or plane: where three planes meet, where a line hits a plane. Rounding a constructed point and then deciding things about the rounded point is the classic way kernels lose consistency (CGAL's own manual warns of it; [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 3).

- Inside an operation, constructed points stay implicit: a point is stored as "the meeting of these three planes" in homogeneous integer coordinates, and predicates act on that definition exactly (the method of Attene's indirect predicates and EMBER's plane-based arithmetic, re-derived in our own code because Attene's library is LGPL and changes the rounding mode).
- **Flat-faced solids stay plane-based for good.** A polyhedral solid is stored as its planes, and every vertex is a meeting of planes. Booleans make new solids out of the same input planes, so cutting a wall 300 times never rounds anything and never drifts (section 11).
- A point is rounded to the grid only when it must become explicit (export, a 2D drawing, a curved construction). The rounded result is re-checked with exact predicates. If rounding would flip anything, the implicit form is kept instead.

## 6.4 Curved geometry: deterministic and certified

Curves and surfaces other than lines and planes are evaluated in `f64` arithmetic in grid units, which Rust computes exactly to IEEE rules on every target. The maths functions (`sin`, `cos`, `atan2`, `exp` and so on) come only from one pure-Rust library, never the platform's, because the standard ones differ by platform (section 21). Every curved computation that feeds a decision carries a certified bound from interval or Bernstein arithmetic, rounded outward with `next_up` and `next_down` (stable in Rust since 1.86).

## 6.5 The tolerance ladder

There is no per-entity tolerance that grows. There is one fixed ladder, derived from the frame's grid step q, so it scales with the step: a circuit board's fit bound is 25 times tighter than a building's.

| Name | In steps | At q0 (buildings) | At q1 (parts, boards) | Used for |
| --- | --- | --- | --- | --- |
| Commit rounding | at most q/2 per coordinate; as a distance, (√2/2)q in 2D and (√3/2)q in 3D | 78 nm per axis; 110 nm in 2D, 135 nm in 3D | 3.1 nm per axis; 4.4 nm in 2D, 5.4 nm in 3D | Making an implicit point explicit |
| Fit bound, ε_fit | 64q | 0.01 mm | 0.4 µm | The largest certified error of any curved construction (intersection curves, fitted surfaces, offsets of NURBS) that feeds topology |
| Angular bound, ε_ang | 2^-34 rad | about 5.8 × 10^-11 rad | the same | Angles in curved code only. It equals ε_fit over the frame's range (64q over 2^40 q), so it is the same at every step; the same construction as Parasolid's 1e-11 |
| Import healing, ε_heal | 640q by default, capped at 6,400q | 0.1 mm, capped at 1 mm | 4 µm, capped at 40 µm | Closing gaps in imported data, once, at import, recorded in the operation log |
| Short-edge warning | 3,200q | 0.5 mm | 20 µm | A warning only. There is no hard minimum edge length; the true minimum is one grid step |

- The exact polyhedral core uses no tolerance at all.
- **Tolerant bodies exist only as an import state.** An imported body whose edges do not quite meet carries per-edge tolerances up to ε_heal, marked Tolerant. No operation ever raises a tolerance. Operations either work within the existing ones or refuse with `ToleranceExceeded`, and healing turns a Tolerant body into a Certified or Exact one. This is Parasolid's published model ("exact by default, tolerant only where needed") with OpenCascade's monotone growth removed.
- Every operation has a tolerance ceiling. Exceeding it is an error that rolls back, never silent growth (Parasolid's `max_tol`, made mandatory).

**Proposed rungs, pending decision.** Principle 10 and section 27 require every threshold to come from this ladder. Other chapters need values, or new meanings, that the table above does not hold. They are listed here with their derivations, without changing that table. An owner accepts or rejects each; an accepted rung moves into the table above and into the ladder module in `geom-num`. Chapters cite each by its name here.

| Name | In steps | At q0 | At q1 | Derivation and use | Proposed in |
| --- | --- | --- | --- | --- | --- |
| Per-operation bound, ε_step | ε_fit/4 = 16q | 2.5 µm | 0.1 µm | The most one operation's own commits and fits may add to an entity's bound. It replaces unnamed fractions of ε_fit | 13.2; chapter 13, open question 4 |
| Resolution floor | 1q | 156.25 nm | 6.25 nm | The box size below which intersection subdivision stops and undecided boxes are flagged. Nothing smaller than one step can be stored | 12.3; chapter 12, open question 3 |
| Solver convergence, τ | q/64 | 2.4 nm | about 0.1 nm | A sketch block has converged when every residual is at most τ, far below commit rounding. The rest of the solver block (rank thresholds, pivot factor, confirmation band, halvings, drag waypoints, cluster limit, lattice cell) is listed in 15.8 | 15.6 and 15.8; chapter 15, open question 5 |
| Smallest triangle altitude, h_min | 8q | 1.25 µm | 50 nm | Tessellation refinement never makes a triangle with an altitude under h_min, so commit rounding ((√3/2)q per vertex) cannot collapse or flip it. Derived from commit rounding; it adds no tolerance | 19.3; chapter 19, open question 3 |
| Level-of-detail bound, δ_k | 2^k·ε_fit | 2^k × 0.01 mm | 2^k × 0.4 µm | The certified bound of display tessellation level k | 19.4; no open question yet |
| Contact radius | ε_fit, a second meaning of an existing rung | 0.01 mm | 0.4 µm | On the curved boolean path, two entities touch only when their enclosure still contains zero at the refinement limit and its upper bound is strictly below ε_fit | 11.4; chapter 11, open question 8 |
| Near-contact cap | Not set: ε_heal's values (640q, capped at 6,400q) or a rung of its own | — | — | The default cap of the explicit `close_near_contacts` repair. Until decided, it is capped by the operation's tolerance ceiling | 11.5; chapter 11, open question 7 |
| Smooth-marking angle, θ | None in the kernel: a recorded parameter of each marking operation | — | — | The angle below which an explicit, logged import operation marks edges Smooth. ε_ang is far too tight for imported fillets and faceted meshes | 18.7; chapter 18, open question 4 |
