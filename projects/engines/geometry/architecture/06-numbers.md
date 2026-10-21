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

Why a grid and not floating point: Revit and OpenCascade store doubles and pay for it with tolerances that leak into every decision. On a grid, the data the kernel decides from is exact, and the only rounding happens at known points: when a new point is committed (at most half a step, 78 nm) and when curved geometry is fitted (at most the certified bound below).

## 6.2 Exact predicates

A predicate is a yes, no or "exactly on it" question: which side of this plane is this point, does this point lie inside this circle. Topology is decided only by predicates, and every predicate in the kernel is exact.

- **They return a sign, never a number:** `Negative`, `Zero` or `Positive`.
- **They are computed in fixed-size integers.** With coordinates under 2^40, orientation in 2D and 3D fits in `i128`; circle and sphere tests in 256 bits; the deepest plane-based test (a point made by three planes, tested against a fourth plane) in 384 bits. Each predicate's module carries the written derivation of its bit budget and a test at the extreme values (every coordinate at ±(2^40 − 1)). Evidence: the bit-budget table in [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 4, matching EMBER's published 256-bit bound at 26-bit inputs.
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
| Commit rounding | at most q/2 | 78 nm | 3.1 nm | Making an implicit point explicit |
| Fit bound, ε_fit | 64q | 0.01 mm | 0.4 µm | The largest certified error of any curved construction (intersection curves, fitted surfaces, offsets of NURBS) that feeds topology |
| Angular bound, ε_ang | 2^-34 rad | about 5.8 × 10^-11 rad | the same | Angles in curved code only. It equals ε_fit over the frame's range (64q over 2^40 q), so it is the same at every step; the same construction as Parasolid's 1e-11 |
| Import healing, ε_heal | 640q by default, capped at 6,400q | 0.1 mm, capped at 1 mm | 4 µm, capped at 40 µm | Closing gaps in imported data, once, at import, recorded in the operation log |
| Short-edge warning | 3,200q | 0.5 mm | 20 µm | A warning only. There is no hard minimum edge length; the true minimum is one grid step |

- The exact polyhedral core uses no tolerance at all.
- **Tolerant bodies exist only as an import state.** An imported body whose edges do not quite meet carries per-edge tolerances up to ε_heal, marked Tolerant. No operation ever raises a tolerance. Operations either work within the existing ones or refuse with `ToleranceExceeded`, and healing turns a Tolerant body into a Certified or Exact one. This is Parasolid's published model ("exact by default, tolerant only where needed") with OpenCascade's monotone growth removed.
- Every operation has a tolerance ceiling. Exceeding it is an error that rolls back, never silent growth (Parasolid's `max_tol`, made mandatory).
