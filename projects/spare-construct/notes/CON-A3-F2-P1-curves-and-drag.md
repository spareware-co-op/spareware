# CON-A3-F2-P1: curves, tangency and dragging in the interim solver

Part of CON-A3-F2-P1 (sketch solver, [FEATURES.md](../FEATURES.md)), second half. Status: decided in piece A35 on 22 November 2026, for the interim engine in `crates/construct-solver` (see [CON-A3-F2-P1-solver-home](CON-A3-F2-P1-solver-home.md)). The kernel's `geom-sketch` replaces the engine; the stored forms below are this program's until then.

## The questions

1. How are circles and arcs stored, so that whole stored units still hold and an arc's ends are on its circle?
2. How is tangency written, so that the degrees of freedom stay exact?
3. What does a drag do?
4. Which of chapter 15.3's constraint kinds come now, and which later?

## Decisions

**Entities.** Circles and arcs are one list, the curves (`CurveId`). Each has a centre point and a radius; an arc also has a start and an end point and runs anticlockwise about its centre from start to end (chapter 15.2's table). The radius is a solver variable, after every point's variables, so a sketch of points and lines keeps its variable order and its bytes. An arc's ends stay on its circle by two equations the arc owns; they are not constraints, and a conflict or redundancy report never names them (the constraints around them are named).

**Stored form.** A radius is a whole number of grid steps (1/6,400 mm), like every stored length (note CON-A2-F3-P1; whole millimetres until piece A39), rounded once by the same rule as a coordinate. An arc's centre and ends are ordinary grid points. After rounding, each arc end's distance from its centre is measured exactly (integer squared distance, one square root) and must be within the model tolerance (0.01 mm) of the stored radius, or the solve fails with `ArcEndRefused`. So a stored arc's ends lie on its stored circle within 0.01 mm. On whole millimetres an end at 45 degrees on a 1 m arc fell 0.15 mm off and was refused; on the grid it is at most half a step off and is accepted. Chapter 15.10's exact arc form (ends on the grid, the centre a rational point on their bisector) is the kernel's.

**Tangency.** Written two ways, chosen from the sketch's structure before solving:

- Anywhere: a distance. The centre's signed distance from the line's carrier less the radius on the stored side; or the centres' distance less the radii's sum (outside) or difference (one inside the other, stored as `Touch`).
- At a shared point (the line's end is the arc's end, one point or points tied by coincidences; or two arcs share an end): a direction. The line is square to the radius there, or the shared point is in line with both centres.

The reason: at a shared point, the distance form and the point's own place on both curves meet in a double root. The point could slide along the tangent to first order, so the Jacobian loses a rank that the geometry does not lose, and the solver would report a slot as under-constrained. The direction form has full rank there. The side or way is still stored on the constraint, and the commit check measures every tangency as the distance form, so a tangency that solved to the other side is refused.

**Dragging (chapter 15.9, one frame).** `drag(sketch, point, target, budget)` starts from the sketch's stored positions (a solve's answer written back with `Sketch::with_positions`). The point is held at the target as ground and the rest projected back onto the constraints by least change. If that conflicts or does not converge, the point is released and the whole sketch solved from the target, so it stops at the nearest place the constraints allow (`Reach::Limit`, naming the constraints that stop it); if nothing moved, `Reach::Refused`. A fixed point is refused at once. Nothing is weighted or softened, every step counts against the one budget, and the result is committed by the same rounding rule. Frames, root-choice checks along the way and the logged release path are chapter 15.9's and the kernel's.

**Kinds now:** point on line, point on circle or arc (on its whole circle: equalities only, 15.3), concentric, equal radius, radius, diameter, tangent line to curve and curve to curve.

**Kinds later, and why:**

- Angle between lines: construct-params now stores angles in the kernel's angle units, but an angle needs sine and cosine, which chapter 15.12 allows only from the kernel's own maths functions, out of a program's reach; this crate's rule is IEEE `+ - * /` and `sqrt` only. It comes with the kernel solver.
- Midpoint, collinear, symmetry, horizontal and vertical distances, arc sweep and arc length, point-to-curve distance, ellipses and splines: not needed by the outlines the first version draws (floors, roofs, rooms, family profiles of lines and arcs), and each is one residual and one commit measure when it is.

## Open risks

- Rounding to whole millimetres refused arcs whose ends could not sit on whole millimetres within 0.01 mm of their circle, and tangencies at slopes. Since piece A39 rounding is to the grid (half a step, 78 nm), and these solve; a refusal is left only where no grid point near the answer meets a constraint.
- A drag is one frame from the stored state. A large drag can land on another solution branch (a flipped corner), which chapter 15.9's frames and root-choice checks would stop; callers should drag in small steps.
- A conflict among an arc's own equations and fixings alone (an arc between three fixed points at different distances) names no constraint, because fixings have no equations; the first half has the same limit for fixed points under distances.
