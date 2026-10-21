# 15. Sketches and constraint solving

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Evidence: [06](../../../kit/notes/G01-evidence/06-2d-drafting.md) sections 5 and 7, [10](../../../kit/notes/G01-evidence/10-github-sweep.md) sections 1 to 3, [07](../../../kit/notes/G01-evidence/07-persistent-naming.md), [03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md) section 4, [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) sections 7 and 8, the SolidCraft vetting in [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), Hoffmann, Lomonosov and Sitharam's decomposition papers (linked in section 15.5), and source read for this chapter at pinned commits: ezpz ([`915882c`](https://github.com/KittyCAD/ezpz/tree/915882cc731da31042ce494c89b10334ee57837a)), fiksi ([`ff48505`](https://github.com/endoli/fiksi/tree/ff4850546738fb82e95ff27c4db63e924d938473)), faer ([`0539947`](https://github.com/sarah-quinones/faer-rs/tree/0539947ffb757a739d7e703a7d2fa0c792a909c1)) and SolveSpace's solver, for method only ([`4707fa2`](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/system.cpp)).

A sketch is a set of 2D curves on a plane plus the rules that relate them: "these two lines are perpendicular", "this arc is tangent to that line", "this width is 900 mm". The rules are called constraints. A constraint solver moves the curves until every rule holds, or explains exactly why it cannot. Every program needs one:

- **Spare Construct:** family profiles that flex with their parameters; wall, floor, roof and ceiling boundaries; opening outlines; stair runs and railing paths; sweep profiles; locks and locked dimensions between elements (section 15.15).
- **Mechanical and car design:** part sketches, blocks that move as rigid groups, and Class-A curve layouts with curvature-continuous joins.
- **Spare Electronics:** board outlines, cut-outs and mounting-hole patterns, on the fine q1 grid.
- **2D drafting (section 14):** AutoCAD-class geometric and dimensional constraints on drafting entities.

Least-squares survey adjustment is not this solver's job; it belongs to section 17. Road alignments are fitted by `geom-align` (section 17), and a sketch only references them.

The solver is `geom-sketch`. It depends only on `geom-num`, `geom-math` and `geom-curve` (section 27). Each solve is an operation under the contract of section 10: pure, transactional, budgeted, deterministic, and returning either a solved sketch with a full report or a typed error. Bringing model geometry into a sketch, and turning a solved sketch into a profile, needs a crate that also sees topology and names; section 15.14 proposes one.

## 15.1 Where a sketch lives

A sketch lives on a plane. The plane record says how 2D sketch coordinates map into the 3D frame (section 6.1):

```rust
struct SketchPlane {
    carrier: PlaneRef,   // an exact integer plane, or a persistent carrier reference (section 20.4)
    frame: PlaneFrame,
}
enum PlaneFrame {
    // a sketch point (s, t) is exactly origin + s·u + t·v
    Lattice { origin: GridPoint3, u: IVec3, v: IVec3 },
    // orthonormal axes from deterministic maths; placing a 2D point in 3D rounds
    Rotated { origin: GridPoint3, x_axis: Dir3, y_axis: Dir3 },
}
```

- **Lattice frames are exact both ways.** Plans, sections and elevations on axis-aligned planes use unit grid vectors for u and v, so a sketch point is a grid point. A plane with a small whole-number normal, such as a 5-in-12 roof pitch, also contains a lattice of grid points; u and v are its two shortest basis vectors, and the solver measures lengths and angles through their Gram matrix (the table of their dot products), so a 3 m rafter measures 3 m. This chapter proposes lattice frames only where each lattice cell is at most 16 grid steps across, so that committing to the lattice moves a point by at most 16 steps (2.5 µm at q0).
- **Rotated frames serve every other plane:** an elevation of a wall at 37° in plan, a sloping face of a machined part. The sketch is solved and committed on its own 2D grid at the frame's step. Placing a committed point in 3D rounds it, by at most √3/2 of a step. The 3D construction that uses it (section 13) takes its planes from the exact carrier, so the swept body is Exact as data, and its distance from the sketch's intent is reported.

The choice between these two kinds is a section 6 decision (open question).

## 15.2 Entities and their variables

A variable is one unknown number the solver may change. Each entity owns a few.

| Entity | Variables | Notes |
| --- | --- | --- |
| Point | x, y | |
| Line segment, ray, infinite line | its two points | For a ray or an infinite line, the second point only fixes the direction |
| Circle | centre, radius | |
| Circular arc | centre, radius, start point, end point | Two equations keep the endpoints on the circle. Endpoints are ordinary points, so joins between curves are plain coincidences |
| Ellipse and elliptical arc | centre, major-axis vector, minor radius; each arc end is a point plus a curve parameter | Parameters, not polar angles (section 7). The axis needs no angle variable |
| NURBS spline | control points, or fit points; weights fixed unless asked; degree and knots fixed during one solve | Fit points are described below |
| Parameter on a curve | t | One per point that slides along an ellipse or a spline, as planegcs does |
| Scalar | one value | A named length or angle that several constraints share |
| Placement | a translation and a small rotation | A rigid group, such as a block or a nested profile, that moves only as a whole (section 15.15) |
| Reference | none | Model geometry brought onto the plane (section 15.11). It never moves |

- **Construction geometry.** Any entity can be flagged construction. It takes part in constraints but never enters a profile. A family's reference lines are construction lines with roles.
- **Offset chains.** An offset of a chain of lines and arcs is a named macro: each piece becomes a parallel line or a concentric arc, tied by constraints the offset operation authors. Changing the distance is a value change (section 15.7).
- **Other curve kinds are reference-only.** Parabolas, hyperbolas, involutes, helices and clothoids can be constrained against, never solved for. Involute gear teeth come from a gear toolkit's parameters (section 26), and alignments from `geom-align`.
- **Fit-point splines.** Their variables are the fit points. Within one solve the parametrisation and knots are frozen, taken from the stored spline, so control points are a fixed linear function of the fit points and the Jacobian is exact. After convergence the spline is re-interpolated with the new fit points and solved again, until the control points move by less than q/64 or a fixed number of rounds is used. The last parametrisation is stored with the spline, and the report says whether it settled. The versioned interpolation lives in `geom-nurbs` and is reached through `geom-curve`'s spline kind.
- **Grid.** A sketch uses its frame's grid step, so a wall outline is solved at q0 and a board outline at q1. While solving, each coordinate is an `f64` copy of a grid value, which is exact because coordinates stay below 2^40.
- **Keys and authoring order.** `geom-sketch` identifies every entity and constraint by a 128-bit key and an authoring sequence number, both supplied by the caller. The facade sets the key to the item's persistent name (section 20) and the sequence number to its operation's position in the log. In this chapter "older" and "newer" always mean the sequence number; keys only break the ties left after it. Plan order, factorisation order and therefore floating-point results depend on this order, so it is fixed per kernel edition, and a change of `NameScheme` is an edition change (section 26.2).

## 15.3 Constraints

Each constraint kind has one written residual: a function that is zero exactly when the constraint holds.

| Group | Kinds |
| --- | --- |
| Incidence | coincident; point on curve (line, circle, arc, ellipse, spline); midpoint; point at a curve parameter; collinear; concentric |
| Direction | horizontal; vertical; parallel; perpendicular; angle between lines; angle of a line to the frame |
| Size | distance from point to point, point to curve, and line to parallel line; horizontal and vertical distance; length; equal length; radius, diameter, equal radius; arc length (on ellipses and splines by fixed-node Gauss–Legendre quadrature with an interval bound on its error); arc sweep; ellipse radii |
| Smoothness | tangent, between any pair of line, circle, arc, ellipse and spline, at a shared point or anywhere; curvature continuity (G2) and its rate (G3) at a shared point, for Class-A layouts |
| Symmetry and pattern | symmetric about a line or a point; linear and circular patterns; equal spacing |
| Fixing | fix a point, a value or a whole entity; rigid group |
| Relations | linear and polynomial relations between scalars, such as d1 = 2 × d2 |
| References | any of the above against reference geometry |

Rules that make every constraint unambiguous:

- **Driving or reference.** A driving dimension is a constraint. A reference dimension only measures; it is reported, never solved.
- **Root choices are stored.** Many constraints can be met more than one way: which side of a line, internal or external tangency, the sense of an angle, which of two crossings. Each choice is recorded on the constraint when it is authored, decided by exact predicates (section 6.2) on the geometry as drawn, and kept until someone changes it. If the predicate returns Zero (a point drawn exactly on the line), or there is no drawing because an AI authored the constraint as data, the author must state the choice, or the operation fails with `RootChoiceUndetermined`. Choices on ellipses and splines use a certified sign that may return Unknown, and the operation refuses when it does. ezpz allows a side to be left `Undefined` and infers it from the initial values on every solve ([constraints.rs L106-L125](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/constraints.rs#L106-L125), [lib.rs L182-L186](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/lib.rs#L182-L186)), so the same sketch can solve to a different shape after a small edit.
- **Priorities only order.** A constraint may carry a priority. It orders pivot preference (section 15.8) and removal suggestions, and nothing else: no constraint is ever left out of a solve because of it. ezpz's priority levels do drop constraints: it adds lower levels until a solve fails, then returns the last solve that succeeded ([lib.rs L199-L245](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/lib.rs#L199-L245)). Spare Kit does not follow that.
- **Equalities only, in this edition.** Inequalities (a gap of at least 5 mm) are checked after the solve and reported as violated limits with numbers, never solved. Solving with them (an active-set method) is a recorded extension.

Family formulas, conditionals and lookup tables are evaluated by the program's command layer, which hands the kernel numbers and the polynomial relations above. The kernel knows no walls or doors (section 5).

**Adding a constraint kind** follows section 26's playbook: one residual written once, its singular set (section 15.4), its root choices, its commit class (section 15.10), its text-format tag, unit and property tests, and a benchmark. Derivatives are generated, never written.

## 15.4 Equations

The Jacobian is the table of how fast each residual changes as each variable moves. The solver and its diagnosis both run on it, so it must be right and well behaved.

- **Every residual is a length, and every variable acts as one.** An angle residual is scaled by a characteristic length of its lines; a tangency residual is a distance. Variables that are not lengths are scaled so they act as lengths: a curve parameter t by the curve's speed |C′(t)| at the start of the solve, an angle scalar by the block's size, each scale rounded to a power of two so scaling stays exact. One tolerance then serves every constraint, stated in grid steps, and the rank tests of section 15.8 do not depend on units.
- **Smooth forms.** Residuals avoid square roots and divisions by variables wherever they can. A distance d > 0 uses (|PQ|² − d²) / 2d. It is a polynomial, so it has a derivative everywhere, and at the solution it equals |PQ| − d to first order, with a gradient of length one. A distance of zero is authored as a coincidence, not a distance.
- **Singular sets are declared.** Some residuals still have places where a derivative does not exist or vanishes: the direction of a zero-length line, a zero radius, an ellipse with equal radii when its axis is a variable, a spline with zero speed. Each residual declares its singular set. Before differentiating, the solver checks the start against it with exact predicates. At a singular start the stated remedy is a small perturbation derived from keys, which the report records. If that does not clear it, the operation returns `Degenerate`, naming the entities. The differentiation scalar carries a NaN tripwire, as section 21 requires.
- **Derivatives are generated.** Each residual is written once. Its derivatives come from forward-mode automatic differentiation (computing with dual numbers, which carry a value and its derivative together) over the same code and the kernel's own maths functions. Tests check them against central differences and interval enclosures. ezpz types its partial derivatives by hand, in a 2,944-line file ([constraints.rs](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/constraints.rs)); SolidCraft's solver uses a dense finite-difference Jacobian ([11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), SolidCraft). Hand-typed derivatives drift from their residuals, and finite differences blur exactly the rank decisions that diagnosis depends on.
- **Exact scaling.** Before solving, a block is moved by a whole number of grid steps to sit near the origin, and divided by a power of two close to its size. Both are exact in floating point, so conditioning improves and nothing rounds. fiksi scales by a root-mean-square size for the same reason ([assemble/mod.rs L22-L60](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/assemble/mod.rs#L22-L60)), but dividing by that rounds.

## 15.5 Breaking the problem apart

Most sketches are many small problems joined loosely. Solving them separately is faster, settles root choices one piece at a time, and lets diagnosis point at the piece that failed. The plan is computed from structure alone, in exact integers, so it is the same on every machine. Every tie is broken by authoring order, then key. Every graph algorithm here is iterative, with an explicit stack counted against the operation's budget, never recursive (principle 13).

| Stage | Runs on | Gives |
| --- | --- | --- |
| 1. Substitution | All equations | An equality between two single variables (coincident points, a horizontal or vertical line, equal radii) merges them into one variable. SolveSpace runs this pass before each solve, and planegcs aliases such parameters ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 5) |
| 2. Components | The bipartite graph of variables and equations: two kinds of node, with an edge from each equation to each variable in it | Parts that do not connect, solved independently |
| 3. Structural counts | Each component, by Dulmage–Mendelsohn decomposition | An over-determined part (more equations than unknowns: redundancy or conflict candidates), an under-determined part and a chain of square blocks; exact structural DOF counts |
| 4. Rigid clusters | Each component's geometric graph: entities are nodes weighted by their freedoms, constraints are edges weighted by their equations, plus a ground node | Clusters solved in their own local frames, then placed |
| 5. Block order | Each cluster, and whatever no cluster covers | Dulmage–Mendelsohn square blocks in solve order, and an under-determined remainder solved by least change (section 15.6) |

**Stage 3.** A matching pairs each equation with a different variable it contains. Hopcroft and Karp find a largest one in O(m√n) time ([doi](https://doi.org/10.1137/0202019)). From it, the Dulmage–Mendelsohn decomposition (Dulmage and Mendelsohn 1958, [doi](https://doi.org/10.4153/CJM-1958-052-0); Pothen and Fan 1990, [doi](https://doi.org/10.1145/98267.98287)) splits the system as the table says. It is exact and fast, and it already finds most structural over- and under-constraint. fiksi's single-pass decomposer matches expressions to variables in the same spirit ([lib.rs L154-L184](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/lib.rs#L154-L184)).

**Stage 4.** Equation counting misses rigid motion: a triangle fixed by three lengths is rigid, yet free to slide and turn, so stage 3 lumps it into the under-determined part. The Hoffmann–Lomonosov–Sitharam Modified Frontier method finds clusters that are rigid by themselves (minimal dense subgraphs: sets of entities whose constraints leave them at most D freedoms, where D = 3 in 2D), solves each in its own frame, replaces it by one node, and repeats ([Part I](https://doi.org/10.1006/jsco.2000.0402), [Part II](https://doi.org/10.1006/jsco.2000.0403)). Dense subgraphs are found by max-flow, the classical network computation of how much can be pushed through pipes of given capacities. The authors' own analysis gives the costs ([CP'97](https://doi.org/10.1007/BFb0017460); [1998 chapter, section 3](https://www.cs.purdue.edu/cgvlab/www/resources/papers/Hoffmann-Geometric_constraint_solving_and_Applications-1998-Geometric_constraint_decomposition.pdf)). Finding a dense subgraph takes O(n(m + n)) time for n entities and m constraints. Reducing it to a minimal one takes O(n²(m + n)) in general, and O(n(m + n)) when valence or over-constraint is bounded, which holds for sketches built interactively. The whole plan repeats this at most n times. Finding a *minimum* dense subgraph is NP-hard, as the same work proves. fiksi implements the Modified Frontier plan but searches exhaustively for a minimum one, which its own comment calls "very slow even for moderately-sized graphs" ([recursive_assembly.rs L486-L498](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/analyze/graph/recursive_assembly.rs#L486-L498)). It is not fiksi's default decomposer ([lib.rs L234](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/lib.rs#L234)).

- **Ground.** Fixed geometry, references and the sketch's frame form one ground cluster with no freedom. A cluster that includes ground is placed in the frame; any other cluster keeps D freedoms. fiksi documents this case as not yet handled ([lib.rs L186-L200](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/lib.rs#L186-L200)).
- **Weights for other variables.** A scalar is a node of weight 1, and a relation between scalars is an edge between them. A point-on-curve constraint with its own parameter counts as weight 1, its net effect (two equations, one new unknown).
- **Placing clusters with least change.** A cluster that keeps its freedoms is placed by the rigid motion that best fits its starting position, a least-squares fit (Kabsch 1976, [doi](https://doi.org/10.1107/S0567739476001873)), so geometry the person did not constrain stays where it was drawn.
- **Size limit.** Above a component size N, stage 4 is skipped and stages 3 and 5 run alone, and the report says so. N is the size at which the cluster search costs more than the solve it saves, measured by the benchmark in the build order. Until it is measured, N is 2,000 entities.

**When counts and numbers disagree.** Counting can be fooled by geometry: two parallel constraints can make a third dependent in a way no count sees, and a block that counts as square can be singular at the configuration as drawn. Such a block is solved by least change, and the report names the geometric degeneracy and the constraints in it, rather than trusting either side. The witness method (Michelucci and Foufou 2006, [doi](https://doi.org/10.1016/j.cad.2006.01.005)) is the published basis for this check; its later W-decomposition splits systems that counting cannot ([HAL](https://hal.archives-ouvertes.fr/hal-00691703)).

The decomposition and the block solver are written for any D, and for any variable kind that declares its freedoms, so 3D sketches and assembly mates (D = 6) reuse them (section 15.15).

## 15.6 Solving

Each block is solved by Gauss–Newton steps (each step solves the equations linearised at the current point), damped toward shorter steps when a full step would overshoot, as in Moré's Levenberg–Marquardt method ([doi](https://doi.org/10.1007/BFb0067700)).

- **The full step is the least-change step.** W holds one weight per variable. The full step is, among all least-squares solutions of J·δ = −r, the one with the smallest W-weighted size. It comes from a complete orthogonal decomposition: a pivoted QR of J·W⁻¹, then a second QR that removes dependent rows (Golub and Van Loan, *Matrix Computations*, 4th edition, section 5.4). With no dependent equations this is exactly SolveSpace's method, a QR of (J·W⁻¹)ᵀ and two triangular solves ([system.cpp L299-L334](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/system.cpp#L299-L334)). Moré's implementation takes MINPACK's basic solution when no damping is needed, which sets free variables to zero in pivot order. In an under-determined block, which is most sketches while they are drawn, that makes the sketch jump. Spare Kit's full step is the limit of the damped steps as damping goes to zero, so least change holds at every damping level.
- **The damped step comes from QR, never from normal equations.** The damped step is the least-squares solution of J stacked on √λ·W, computed by QR (a factorisation into an orthogonal matrix and a triangular one). Forming JᵀJ or JJᵀ squares the condition number, which is how much an error in the equations can be magnified in the answer. SolveSpace's code avoids JJᵀ for this reason ([system.cpp L299-L334](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/system.cpp#L299-L334)), and fiksi solves the stacked system ([lm.rs L40-L61](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/solve/lm.rs#L40-L61)). ezpz forms JᵀJ and factors it by Cholesky ([newton.rs L63-L77](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/solver/newton.rs#L63-L77)).
- **Convergence in grid steps.** A block has converged when every residual is at most τ = q/64, a 64th of a grid step and far below the commit rounding of q/2. Because the block is scaled to its own size, this means the same at 2 m and at 20 km. SolveSpace stops at an absolute 1e-8, and ezpz at an absolute 1e-8 with a separate 1e-4 to call a constraint satisfied ([solver.rs L73-L80](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/solver.rs#L73-L80), [lib.rs L43](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/lib.rs#L43)). Absolute values do not scale ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 7).
- **Budgets in work, not time.** Iterations and factorisations count against the operation's budget, and the cancellation token is checked once per iteration. Running out returns `BudgetExceeded` or `Cancelled`, never a best-so-far answer.

**Targets**, for benchmarks on section 23's benchmark machine. Solve targets cover positions, the DOF count, constraint marks and the plan. Diagnosis targets cover the parts of the report computed on request (section 15.8).

| Case | Target | Covers |
| --- | --- | --- |
| A 200-constraint family sketch | under 1 ms | Solve and full report |
| A 5,000-segment floor outline with its coincidences and directions | under 50 ms | Solve and DOF count |
| One drag frame of a 1,000-entity sketch | under 4 ms | Positions only |
| Free motions of one entity in a 10,000-unknown outline | under 5 ms | Diagnosis |
| Dependency groups of a conflict among up to 256 unknowns | under 20 ms | Diagnosis |

For scale, ezpz reports 2,000 equations solved in about 3 ms ([README](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/README.md); self-reported).

**Deterministic linear algebra.** Dense and sparse QR, the complete orthogonal decomposition, triangular solves and column ordering live in `geom-num`, with its deterministic maths, because NURBS fitting and blends need the same routines.

- **Scalar loops, for this edition.** `geom-num`'s factorisations use plain scalar loops in a fixed order: no SIMD (one instruction working on several numbers at once), no `mul_add`, no threads inside one factorisation. Section 21 allows fixed-width SIMD with a fixed lane order and explicit `mul_add`. `geom-num` gives them up for now because scalar loops are the simplest to prove bit-identical across x86, ARM and WebAssembly, and WebAssembly has no fused multiply-add instruction, so `mul_add` runs there as a slow library call. A later edition may add fixed-width SIMD, checked by the replay matrix.
- **Ordering.** The column ordering that keeps sparse factors small follows COLAMD (Davis, Gilbert, Larimore and Ng 2004, [doi](https://doi.org/10.1145/1024074.1024079)), written in safe Rust with ties broken by variable order. fiksi's `colamd_rs` is a mechanical C2Rust translation that contains `unsafe` ([README L21](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/colamd_rs/README.md#L21), [colamd.rs L49](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/colamd_rs/src/colamd.rs#L49)), so principle 13 rules out that code. COLAMD's BSD-3 licence allows a port.
- **Why not faer.** faer, which ezpz uses, chooses SIMD kernels at run time: its `pulp` dependency is built at the AVX2 level `x86-v3`, and its `std` feature, which ezpz turns on, adds an x86-only matrix-multiply crate ([faer Cargo.toml L29-L54](https://github.com/sarah-quinones/faer-rs/blob/0539947ffb757a739d7e703a7d2fa0c792a909c1/faer/Cargo.toml#L29-L54), [ezpz Cargo.toml L23](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/Cargo.toml#L23), [pulp README L1-L2](https://github.com/sarah-quinones/pulp/blob/601de0c071ba32df3fd7ff33fc4218c9bd712aba/README.md#L1-L2)). Different kernels add in different orders, so its results are not expected to be bit-identical across x86, ARM and WebAssembly. This is inferred from the manifests, not tested; [06](../../../kit/notes/G01-evidence/06-2d-drafting.md) left it open.

## 15.7 Changing values: continuation

A dimension edit, a family flexed to another type, a formula's new result and a moved model reference all change the values a sketch must meet. Solving straight from the old shape to the new values in one jump can land on another solution: an arc's tangency on the other side, a triangle's mirror image, a linkage folded the other way. So every such change runs by continuation:

1. The changed values move together along a straight path from old to new, starting with one step.
2. After each step the block is solved from the previous step's solution, and every stored root choice is checked with certified signs (section 6.2's filters, with an interval fallback, on the current values). A sign that cannot be certified counts as at risk.
3. A step that does not converge, puts a root choice at risk, or drops the block's smallest pivot below the independence threshold of section 15.8 (so passing near a singular configuration is never missed) is halved. After 10 halvings at one place, the operation fails with `BranchLost`, naming the constraint and the value along the path where the solution could not be followed.

The schedule depends only on the stored state and the new values, so it is the same everywhere. Flexing a family across a type table runs one continuation per type from the stored state, in parallel through section 21's helper. The decomposition, the sparsity pattern and the column ordering depend only on structure, so they are computed once and reused. Moving a mechanism through a range, such as a door swing or a linkage, is the same continuation with a driving value, keeping every step.

## 15.8 Degrees of freedom, redundancy and conflicts

A degree of freedom (DOF) is one independent way the sketch can still move. The rank is the number of independent equations, and DOF = variables − rank.

**Rank.** The rank comes from QR with column pivoting (Businger and Golub 1965, [doi](https://doi.org/10.1007/BF01436084)) of Jᵀ, in which each column is one equation. A pivot is what is left of a column once the columns already chosen are taken out of it; each step takes the column with the largest. Among columns within a factor of 2 of the largest, pivoting prefers the higher priority, then the older constraint. The choice stays numerically sound, and the equations left over are the ones a person would expect to question. The same factorisation gives the directions in which the sketch can still move.

**Thresholds, derived rather than chosen.** After the scaling of section 15.4 the block's size is 1, and every entry of J is a ratio of lengths. Let S be the block's size in grid steps.

| Pivot σ | Verdict | Why |
| --- | --- | --- |
| At least σ_ind = 2^-12 | Independent | Residuals within τ then pin the geometry to within τ/σ ≤ 64q, which is ε_fit |
| At most σ_dep = 1/(64S) | Dependent | Moving the block by its whole size in that direction changes no residual by more than τ |
| Between | Near-singular | Reported with the constraints involved, never rounded either way |

For a 1 m sketch at q0, σ_dep is about 2^-29; for a 20 km site, about 2^-43. σ_dep is never set below a bound on the factorisation's own rounding error (Higham 2002, chapter 19, [doi](https://doi.org/10.1137/1.9780898718027)). When that bound decides, the report says the block is too large to tell dependence at the grid's resolution. Pivoted QR estimates the smallest singular values well in practice but not always (Kahan's example), so a pivot within a factor of 16 of either threshold is confirmed by a singular value decomposition of the small group involved.

**Where it is judged.** Redundancy and conflict are judged at the converged point, or at the least-squares best when the solve stops above tolerance. Rank is also judged at a witness: the start moved by small amounts derived from keys. A dependence that appears at the solution but not at the witness comes from this configuration only, such as three points on one line under a triangle's distances. It is reported as a special configuration, with the constraints involved.

**The rule.**

- Converged, and every bounded residual within ε_fit after commit: success. Near-singular groups, redundant groups and special configurations are warnings in the report.
- Not converged, or the commit cannot be bounded within ε_fit: an error (`NotConverged`, `Conflict` or `ToleranceExceeded`), and nothing changes.

**Free motions,** computed on request. The null space of J (the directions in which no residual changes) is the sketch's remaining freedom. Its basis is put in reduced echelon form over the variable order: each basis vector has 1 at its own leading variable and 0 at the others' leading variables, so the basis is unique. It is reported per entity in plain terms: "P3 moves along (0.6, 0.8)", "the radius of C1 is free", "the whole sketch turns about P1". The three rigid motions of an unplaced sketch are reported separately. For large blocks the basis is kept sparse and computed per entity or per query, under the budget. ezpz lists under-constrained variables by how much they take part in the null space ([find_dof.rs](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/solver/find_dof.rs)); Spare Kit reports the motions themselves. Under-constrained is not an error: most sketches are while they are being drawn.

**Dependency groups.** This is planegcs's method, described in our own words ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 5). An equation the pivoting leaves out is a combination of the chosen ones, and solving R₁₁z = r₁₂ gives the coefficients. The equations whose coefficients are at least 2^-12 of the largest (the same ratio as σ_ind), together with the one left out, form its dependency group. Groups are computed only for the rank-deficient part of the system. A group found from sparse QR is confirmed by dense QR on its own small subsystem, because sparse QR is known to miss rank on some structures (planegcs's own comment and [FreeCAD issue 10903](https://github.com/FreeCAD/FreeCAD/issues/10903); [06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 5). Each group is then classed:

- **Redundant:** the group holds at the solution. A fourth equal-length constraint on a square is redundant. It is allowed, solved by least squares, and reported as a warning.
- **Conflicting:** the group cannot hold. This is an error. The operation fails and nothing changes.
- **Partly redundant:** a constraint with several equations, of which only some are dependent, as planegcs reports. An example is a coincidence whose y equation repeats a horizontal constraint.

The constraint to suggest removing is chosen by a fixed rule: the lowest priority first, then the one in the most groups, then the newest. Solving without it confirms the suggestion. SolveSpace's approach, one rank test per constraint removed ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 5), runs in the tests as a cross-check on small groups.

**Remedies with numbers.** For each driving dimension in a conflicting group, the solver solves without it and measures the value it then takes, at the least-change solution. Where the feasible range is cheap to bound, by continuation along that one value until the block turns singular, it is given too. The error then reads: "D7 (4,200 mm) conflicts with D3 and D5. Without D7 it measures 4,150 mm, and any value from 3,900 to 4,180 mm fits. Or make D7 a reference dimension, or remove D3." A single value is never presented as the only one that works.

**Conflicts that are not rank problems.** Three lengths of 1, 1 and 5 cannot close a triangle, yet their equations are independent. The solve stops above tolerance at its least-squares best and returns `NotConverged` with the residuals, and the same remedy search gives values that would work (here, a third side of at most 2). The method is local, so `NotConverged` says "no solution found from this start", never "no solution exists".

**Constants.** Every constant in this chapter (τ = q/64, the two rank thresholds, the pivot-preference factor of 2, the confirmation band of 16, the group coefficient of 2^-12, 10 halvings, 256 drag waypoints, the cluster size limit, the 16-step lattice cell) is proposed for a solver block in section 6.5's ladder, versioned with the kernel edition and calibrated on the test corpus before the first edition freezes it.

## 15.9 Dragging

Dragging is a solve with a moving target, and its frames are previews.

1. Each frame starts from the previous frame's solution, so the sketch follows a continuous path rather than jumping to another solution.
2. The dragged variables are set to the cursor and held there, and the rest of the sketch is projected back onto the constraints by the least-change step of section 15.6. Constraints are never softened.
3. If the constraints cannot hold with the dragged variables held, those are released and the whole sketch is projected from the cursor position, so the dragged geometry stops at the nearest position the constraints allow. The frame reports `AtLimit` with the constraints that stop it.

This follows SolveSpace's method, which weights dragged parameters so its minimum-norm step moves them least ([system.cpp L275-L284](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/system.cpp#L275-L284)), taken to its limit: the dragged variables are held, not weighted by a constant.

- After each frame the stored root choices are checked as in section 15.7. A step that would flip one is halved. If it still flips, the drag stops at the last good frame with `DragLimit`, naming the constraints that block it. `AtLimit` and `DragLimit` are preview statuses, not errors.
- Frames are solved at the preview precision of `OpContext`, with a fixed work budget each. A frame that runs out shows the previous frame. Frames are shown, never stored.
- **Release** is one logged operation: "move these targets along this path, from this stored state, keeping these root choices". The path is the accepted frame targets, at most 256 of them (above that, every k-th one with k the smallest whole number that fits, plus the last). Replay runs the continuation of section 15.7 through them at final precision, then commits (section 15.10). If the committed result differs from the last preview by more than ε_fit, the report says by how much and which entities moved.

## 15.10 From solution to stored geometry

The solver works in floating point; the kernel stores grid values (section 6.1). Commit is the step between, and it is where sketch solvers usually lose exactness without saying so. Spare Kit makes declared incidences exact by construction and gives every other constraint a proven bound.

**Exact forms.** Straight lines are stored as their carriers, as section 6.3 stores flat faces as their planes:

- **Carriers.** A carrier is an integer line n·X = c, with each part of n below 2^41 and |c| below 2^82. A line between two grid points takes n from their difference. A solved direction is rounded once, to n = (round(2^40 cos φ), round(2^40 sin φ)) divided by their greatest common divisor, which turns it by at most 2^-40 rad. Rounding c moves a line by at most 1/(2|n|) of a step, finer than the grid.
- **Depth-one points.** A depth-one point is the meeting of two carriers, stored in homogeneous coordinates: three integers X, Y and W meaning the point (X/W, Y/W), so the meeting needs no division. |X| and |Y| stay below 2^124 and |W| below 2^83. A point that slides freely along a carrier is its meeting with the nearest grid row or column, whichever crosses it more steeply. Carriers are never defined from depth-one points, which keeps every predicate within a fixed width.
- **Arcs.** An arc's endpoints are grid points, and its centre is a rational point on their perpendicular bisector, 2(Q − P)·X = |Q|² − |P|²: the bisector's meeting with the normal at a tangent end, or otherwise with the nearest grid row or column. Then |CP| = |CQ| exactly, so an arc's endpoints lie exactly on its own circle, and a line joined to an arc shares its endpoint exactly.
- **Circles.** A full circle has a grid or depth-one centre and either a whole number of steps as its radius, when the radius is driving, or a defining grid point on it.

| Relation | After commit |
| --- | --- |
| Coincident points; curves joined end to end | **Exact.** One stored point |
| Horizontal, vertical, fixed coordinates; horizontal and vertical distances; symmetry about a horizontal or vertical line | **Exact.** Equal or mirrored grid coordinates |
| Parallel, perpendicular and collinear lines | **Exact.** Parallel lines share n; perpendicular lines use n turned a quarter turn; collinear lines share one carrier |
| A corner between two lines; a point on a line inside its length (a T-junction) | **Exact.** A depth-one point |
| An arc's endpoints on its own circle; tangency where a line meets an arc at one end | **Exact.** The arc form above, with the centre on the line's normal |
| A driving radius of a full circle; concentric circles | **Exact.** A whole number of steps; one shared centre |
| Sloping distances and lengths; angles other than multiples of 90°; an arc tangent at both ends (a fillet); points on circles other than a defining point; all incidences on ellipses and splines, spline fit points included; symmetry about a sloping line | **Bounded.** Each residual is evaluated on the stored geometry in interval arithmetic (arithmetic on ranges, rounded outward so the true value is always inside), and its bound is reported in model units |

The rows that use carriers, depth-one points and rational centres are Exact once sections 6.2 and 14 adopt these forms (open question); until then those constraints are only bounded. A spline's fit points after commit lie within √2·q/2 of where the solve put them, because each curve point is an average of control points with non-negative weights, so rounding the control points moves the curve by at most the largest rounding. Rounding a 1 m line's ends at q0 can tilt it by about 2 × 10^-7 rad, thousands of times section 6.5's ε_ang, which is why angles and tangencies outside the exact rows are only bounded.

Why the T-junction row matters: a wall meeting a sloping wall at a rounded point leaves a gap or an overlap of up to about 0.1 µm, and the sloping wall's face splits into two faces on two planes. With a depth-one point the junction lies exactly on the line, and both pieces keep one carrier, which section 20.4's carrier references rely on.

**Predicate widths.** The 2D arrangement (section 14) must decide exactly on these forms. The widths below, sign bit included, are derived for this chapter from the bounds above. Each predicate's module re-derives its own and tests it at the extreme values, as section 6.2 requires.

| Predicate | Width |
| --- | --- |
| Orientation of three grid points | 84 bits |
| Side of a carrier, for a depth-one point | 168 bits |
| Order of two depth-one points along a carrier | 251 bits |
| Orientation of three depth-one points | 335 bits |
| Side of an arc's chord, for its centre | 169 bits |
| A grid point against an arc's circle, as (Z − P)·(Z + P − 2C), which avoids squares | 170 bits |
| A depth-one point against an arc's circle | 337 bits |
| A depth-one point against a grid-centred circle with a whole-number radius | 253 bits |

All fit in the 384-bit width section 6.2 already plans. Where lines and arcs cross between their endpoints the crossings involve square roots; those points and their predicates belong to section 14.

**The commit plan.** Exact relations hold only if quantities are rounded in the right order: a master is rounded once, and whatever depends on it is computed from it exactly.

1. Build the graph of exact relations from the table. Nodes are stored quantities (grid points, carriers, arc centres, radii); edges are the relations that define one from others.
2. Walk it in authoring order, then key. A quantity defined by quantities already placed is computed from them exactly. Any other is a master, rounded once from its solved value in the block's own integer space, as round(local value × 2^k) + offset with all three integers, so nothing is rounded twice.
3. A relation whose quantities are all placed already closes a cycle; an example is a slot whose two straight sides must both be tangent to both end arcs. It is checked exactly. If it holds, it is Exact; if not, it is bounded, and the report names it. Cycles are broken in the same order, so the same relation gives way on every machine.

**Three checks then run, in order:**

1. **Root choices.** Every stored root choice is decided again with exact predicates on the committed geometry. A flip here can only come from rounding within a few grid steps of a degenerate configuration. It fails the commit with `CommitFlip`, naming the constraint and the place. Flips on the way to new values are `BranchLost` (section 15.7).
2. **Ceiling.** Every bounded residual must be at most ε_fit (section 6.5: 0.01 mm at q0, 0.4 µm at q1), plus the bound of any certified reference it uses (section 15.11), or the commit fails with `ToleranceExceeded`. No tolerance grows.
3. **Certificate.** Blocks are certified in plan order with the Krawczyk test (Krawczyk 1969, [doi](https://doi.org/10.1007/BF02234767)), which proves in interval arithmetic that an exact solution exists inside a stated box. Each block takes the boxes already proven for the blocks before it as interval inputs, so the radius it proves is the distance from the stored geometry to the exact solution of the whole sketch, with free variables held at their committed values. A redundant group is certified with its dependent equations set aside, and those keep only their residual bounds. A block too large or too nearly singular for its budget is marked `Unproven` with the reason, and its constraints keep their residual bounds.

**What the result is.** Each constraint is marked `Holds::Exact` or `Holds::Within { residual }`. Certified(ε), in section 9's sense, is kept for geometric distance: a block's proven radius. The sketch's curves are stored on the grid and on integer carriers, so the profile it hands on is Exact as data. How well that profile meets the design intent travels in the report, the operation's diagnostics and its history, and is never folded into a body's exactness class.

## 15.11 References from the model

A reference is model geometry that a sketch constrains against: a wall face, a grid, a level, an edge of a part. References are persistent (section 20.4), by default to the carrier, so a sketch line locked to a wall's exterior face stays locked when a door splits that face. Before the solve, each reference is resolved and brought onto the sketch plane by a rule stored with it:

| Reference | Conversion | Class carried into constraints |
| --- | --- | --- |
| In the sketch plane (levels, grids and location lines in a plan) | None | Exact |
| Points and straight edges off the plane | Projection along the plane's normal: exact onto axis-aligned lattice planes, rounded otherwise | Exact, or within the rounding |
| Circles and arcs | Projection: a circle stays a circle on a parallel plane and becomes an ellipse on a tilted one | Exact when parallel and on the grid; otherwise Certified, with the ellipse's rounding |
| NURBS edges | Projection of the control points (projection along a direction is affine, so the result is a NURBS) | Exact when the projected control points are grid points; otherwise Certified |
| A face cut by the plane | Section: the intersection curve of section 12 | Certified(ε ≤ ε_fit) |
| Helices, clothoids, offset and intersection curves | Converted to NURBS with their certified error (section 7), then as above | Certified |

A constraint against a reference that arrives Certified(ε) carries ε into its mark and into the ceiling check. A reference that cannot resolve leaves its constraints `UnresolvedRef` with the reason. Nothing guesses a replacement.

## 15.12 Determinism, in one place

- The plan is computed from structure alone, in exact integers, with ties broken by authoring order, then key.
- Arithmetic is IEEE basic operations and the kernel's own maths functions only (section 21), under the linear-algebra rules of section 15.6. Independent blocks and types may be solved in parallel through section 21's shared helper, and results are collected in plan order.
- Every solve starts from stored geometry and stored root choices, never from screen state.
- A start is perturbed only as the stated remedy for a singular start (section 15.4) or as the witness of section 15.8, by amounts derived from keys, and the report says so. fiksi perturbs every start by default, from a fixed seed ([lib.rs L235](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/lib.rs#L235), [assemble/mod.rs L47](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/assemble/mod.rs#L47)): deterministic, but it moves every solve's starting point.
- Budgets count work, never time. Time affects only drag previews, which are never stored; a release replays its logged path.
- Reports are ordered by authoring order and key, and the null-space basis is canonical, so two machines print the same report.
- Section 21's replay corpus includes sketch solves, continuations, drags and commits, on every target and thread count.

## 15.13 What people and AI see

Every solve returns a report, in one shape for people and AI:

```rust
struct SketchReport {
    dof: u32,                                  // remaining freedom, not counting the sketch's placement
    plan: Vec<PlanStep>,                       // stages, clusters and blocks in solve order, with sizes
    holds: Vec<(ConstraintKey, Holds)>,        // Exact, or Within { residual } in model units
    certificates: Vec<(BlockId, Certificate)>, // Proven { radius } to the whole sketch's exact solution, or Unproven { reason }
    redundant: Vec<DependencyGroup>,           // consistent but dependent constraints
    near_singular: Vec<NearSingularGroup>,     // { constraints, margin }: rank too close to call
    special: Vec<SpecialConfiguration>,        // dependence that comes from this configuration only
    limits: Vec<LimitViolation>,               // inequalities, checked after the solve
    path: Option<PathReport>,                  // continuation steps and halvings
    work: WorkUsed,                            // iterations and factorisations
}
// Asked for separately, under a budget: the free motions of chosen entities,
// and the dependency groups of any set of constraints.
```

Failures are `OpError`s (section 22). Each sketch kind refines a kind of the shared base:

| Sketch kind | Base kind | Carries |
| --- | --- | --- |
| `Conflict` | `NoSolution` | The groups, the suggested removal and the numeric remedies |
| `NotConverged` | `NoSolution` | The unsatisfied constraints and their residuals in model units |
| `BranchLost` | `NoSolution` | The constraint, and the value along the path where it was lost |
| `RootChoiceUndetermined` | `Ambiguous` | The constraint and its possible choices |
| `Degenerate` | `Degenerate`, added to the base | A zero-length line, a zero radius, a folded arc, or a singular start the remedy did not clear |
| `CommitFlip`, `ToleranceExceeded` | `ToleranceExceeded` | The constraint, the place and the bound |
| `UnresolvedRef` | `UnresolvedRef` (section 20.4) | The reference and the reason |
| `BudgetExceeded`, `Cancelled` | The base kinds | The work used |

**History.** A solve creates and deletes nothing. Every entity whose geometry changed is modified one-to-one and keeps its name; every other entity is unchanged (section 20.3).

**A text form.** A canonical text form of a sketch problem (plane, entities, constraints, root choices and values) round-trips exactly. Grid values are written as whole numbers of steps, and other numbers as the shortest decimal that reads back to the same `f64`. The grammar is versioned (`sketch v1`). It starts in the Preview tier and becomes Stable with the facade (section 25). Tests, bug reports and AI agents use it, on the model of ezpz's problem files ([README](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/README.md)).

## 15.14 Names, profiles and sweeps

- **Names.** Every entity and constraint is named by the operation that authored it (section 20.2), and constraints refer to entities by name, never by position. Deleting an entity renumbers nothing.
- **Roles.** A family sketch's reference lines carry roles (left, right, centre, top) that become the carriers hosted elements and dimensions bind to.
- **Profiles need their own crate.** Turning a solved sketch into a profile means building closed regions with `geom-2d`'s arrangement (section 14), choosing regions by name, checking them, and building a planar face with the topology builder. Bringing references in (section 15.11) needs topology, names and intersections too. No crate in section 27 sees both sketches and topology, and an operation may not live in the facade (section 26.1). This chapter proposes `geom-profile`, depending on `geom-sketch`, `geom-2d`, `geom-topo`, `geom-name`, `geom-check` and `geom-intersect`, with one job: carrying geometry between sketches and the model. The facade only exposes its schemas. Its errors say what is wrong with a profile: an open loop, a self-intersection, a dangling curve, a region that is not there. Construction geometry never enters a profile. Where sketch curves cross, the pieces are named as splits by the crossing curves and the side.
- **Sweeps.** An extrusion names its side faces `side(<sketch curve name>)` and its caps `cap.start` and `cap.end` (section 20.2). Changing a dimension changes geometry but not names, so a fillet on the face swept from line L4 still finds it.
- **Tangency travels as intent.** A tangency declared in the sketch is recorded on the shared vertex, and a sweep carries it to the edge it makes. Code downstream reads that record and does not re-derive smoothness from angles, which after commit hold only to the bounds of section 15.10.
- **When a sketch stops solving during regeneration.** An upstream change, such as a moved wall or a new type value, can leave a stored sketch unsolvable. The upstream edit itself succeeded, so "nothing changes" cannot apply. The sketch becomes model state, `SketchUnsolved { error }`, keeping its last committed geometry marked stale, the same on every machine, as section 20.4 does for references. Features downstream see a typed failure, never stale geometry passed off as current. The type is part of `engine-geometry`'s schemas, and the layer that regenerates documents records it. A program may instead refuse the upstream edit; either way the log records the outcome.

## 15.15 Beyond one plane: placements, assemblies and project locks

The solver's core is written so that the same machinery serves more than flat sketches.

- **Placements.** A placement variable is a frame for a rigid group: a translation, and a small rotation about its stored rotation, re-centred after each accepted step. That is 3 freedoms in 2D and 6 in 3D. A placement whose rotation the constraints fix structurally to a multiple of 90° (parallel to a frame axis or to an exactly known direction) commits as an exact quarter-turn transform. Any other commits as a general rotation, which rounds, and the result says so (section 7). In 2D sketches, placements carry blocks and nested profiles.
- **3D sketches** (pipe routes, railing paths in space, wire frames) are `geom-sketch` with 3D entities and D = 6. The decomposition, solver, diagnosis and commit are the same code.
- **Assembly mates** (coincident, concentric, distance, angle, parallel, tangent, and gear and screw relations between parts) act on placements. They live in a `geom-assembly` toolkit once two programs need them (section 26). Moving a mechanism is the continuation of section 15.7, and collision along the way is section 10's time-of-first-contact query.
- **Locks between elements** (locked alignments and dimensions across a whole project, as Revit has) are a datum system that the program builds. Spare Construct turns each locked element's driving datums (location-line ends, reference planes, grid lines; levels as scalars) into sketch entities, one system per plan level or elevation, and each lock into a constraint. Components keep a lock between two walls a two-wall problem. The result is written back as element edits in one logged operation. The kernel solves; it still knows no walls.

The interfaces for variable kinds and for D are fixed now, so these extensions add code without changing the core.

## 15.16 Depend, port or study

| Project | Licence | What it has | What rules it out as a dependency | Verdict |
| --- | --- | --- | --- | --- |
| ezpz (Zoo) | MIT | 25 constraint kinds over points, lines, circles and arcs ([constraints.rs L37-L92](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/constraints.rs#L37-L92)); priority levels; freedom analysis; `libm`; a text problem format with 28 test problems; property tests, a fuzz target and a WebAssembly build | faer's run-time SIMD; JᵀJ; absolute tolerances; hand-typed derivatives; priority levels that drop constraints; root choices inferred on every solve; panics in library paths ([`assert_eq!` in the solver step](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/solver/newton.rs#L103), [a mutex `unwrap`](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/lib.rs#L293), [`unreachable!`](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/lib.rs#L366)); no dependency groups or redundancy classes; no decomposition; no ellipses or splines | **Study.** Port its test problems into our corpus, and its residual formulas where they meet our rules, keeping the MIT notice. Use it as a test oracle. [10](../../../kit/notes/G01-evidence/10-github-sweep.md) (section 1, item 10) named it a candidate to depend on after an audit; this is that audit |
| fiksi (Linebender) | Apache-2.0 OR MIT | The Modified Frontier decomposition; Levenberg–Marquardt on stacked sparse QR; its own pure-Rust sparse QR and Cholesky (`solvi`, with no `unsafe` in the factorisations); scale normalisation; `no_std` | Version 0.1, last commit May 2026; only points, lines, circles and lengths; recursive assembly experimental, not the default, and not yet handling constraints or fixings relative to the global frame ([lib.rs L186-L200](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/lib.rs#L186-L200), [L234](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/lib.rs#L234)); an exhaustive search for a minimum dense subgraph (an NP-hard problem), seeded by iterating `hashbrown` hash sets with its default hasher ([recursive_assembly.rs L5](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/analyze/graph/recursive_assembly.rs#L5), [L579](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/analyze/graph/recursive_assembly.rs#L579), [Cargo.toml L24](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/Cargo.toml#L24)), which its own docs give as foldhash's `RandomState` ([collections.rs L4-L5](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/collections.rs#L4-L5)), randomly seeded according to foldhash, so the plan can differ between runs (inferred from the code, not run); a density test that reads with the opposite sign to its own doc comment ([L488 against L505 and L618](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/analyze/graph/recursive_assembly.rs#L486-L618); not confirmed by running); an over-constraint finder that assumes every variable is free and pivots on the first entry above an absolute 1e-8 ([numerical/mod.rs L8-L124](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/analyze/numerical/mod.rs#L8-L124)); platform maths by default, since "`std` overrides `libm`" ([lib.rs L12](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/lib.rs#L12)); `unsafe` in `colamd_rs` | **Study closely.** Port `solvi`'s sparse QR structure and the stacked-QR step after an audit, keeping the Apache-2.0 OR MIT notice. Use it as a test oracle |
| planegcs (FreeCAD) | LGPL-2.1-or-later | DogLeg and Levenberg–Marquardt; dependency groups from QR of Jᵀ; redundant, conflicting and partly redundant classes | LGPL | **Method only,** in our own words |
| SolveSpace, libslvs, rust_slvs | GPL-3 | Substitution passes, minimum-norm steps, drag weights, rank tests | GPL | **Method only** |
| SolidCraft's sketch solver | MIT OR Apache-2.0 | Levenberg–Marquardt on a dense finite-difference Jacobian, capped at 4,000 variables per component ([11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md)) | Finite differences; dense only | **Reject** |
| D-Cubed and other proprietary solvers | Proprietary | — | — | **Excluded** |

Neither permissive solver is a base to build on. Each would need its numerics, determinism and panics rewritten and its entity set tripled. Porting their good parts into `geom-sketch`, and running both as differential test oracles (solved positions compared within tolerance, not bits), gets their value without their limits. The sweep found no mature permissive solver at all ([10](../../../kit/notes/G01-evidence/10-github-sweep.md), section 3, gap 6), so Spare Kit builds the first.

## 15.17 What is unsolved, and how Spare Kit leads

- **Which solution the person means.** A well-constrained sketch can have exponentially many solutions, since each tangency or distance can often be met two ways, and choosing the intended one is open in general (Bettig and Hoffmann 2011, [doi](https://doi.org/10.1115/1.3593408)). Spare Kit stores every root choice, reaches new values by continuation from the stored state, and refuses rather than flips. It promises the same choice on every machine, and a named error when the choice cannot be kept.
- **Exact rigidity.** Counting decides generic rigidity exactly only for points joined by distances in the plane (Laman 1970, [doi](https://doi.org/10.1007/BF01534980)). With angles, tangencies and parallels, combinatorial decomposition can be fooled ([Zou et al. 2022](https://arxiv.org/abs/2202.13795)). Spare Kit runs both and reports where they disagree. Its next step is exact rank by fraction-free elimination (Bareiss 1968, [doi](https://doi.org/10.1090/S0025-5718-1968-0226829-0)) in wide integers on a rational witness, so redundancy is decided exactly rather than by threshold. Until that is measured and built, the three-way verdict of section 15.8 holds.
- **Certified solutions.** None of the open solvers in the evidence proves that an exact solution exists near what it returns; they report residuals. The chained Krawczyk certificate of section 15.10 does.
- **Exact fillets.** An arc tangent at both ends generally needs irrational tangent points. Exact rational forms exist for some cases, but their bit widths grow with the precision required. Until they are worked out with section 14, fillets in sketches are bounded, not exact.

## 15.18 What the solver never does

- Never moves fixed, reference or model geometry.
- Never drops, relaxes or reorders a constraint silently, and never returns a partly solved sketch as a success.
- Never flips a stored root choice without an error.
- Never refers to an entity by position, or picks a replacement for a lost reference.
- Never flattens arcs, ellipses or splines in order to solve them.
- Never uses hand-typed or finite-difference derivatives, platform maths functions, hash-order iteration, reductions whose order depends on lane width, or wall-clock time in anything that affects a stored result.
- Never rounds a value twice, and never grows a tolerance to make a commit pass.
