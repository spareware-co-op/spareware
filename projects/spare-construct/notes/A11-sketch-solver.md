# A11: Sketch solver: how planegcs works (method only; it is LGPL)

## The question

Family sketches, wall profiles and floor outlines need a 2D constraint solver (parallel, perpendicular, equal, distance, tangent, fixed), and the map names FreeCAD's planegcs as the method to follow, "studied and re-implemented, not copied, because it is LGPL" ([ARCHITECTURE.md](../ARCHITECTURE.md), Regeneration and constraints). The piece asks for the method described in our own words, with a port plan.

## Finding: the kernel has done this piece

Since the board was written, the sketch solver moved into Spare Kit's geometry kernel, and its map describes the method in our own words, planegcs included, with a build plan:

- **The method.** Chapter 15 of the kernel map ([15-sketches.md](../../engines/geometry/architecture/15-sketches.md)) covers entities and their variables (15.2), the constraint catalogue (15.3), equations (15.4), breaking the problem apart (15.5), solving (15.6), continuation when values change (15.7), degrees of freedom, redundancy and conflicts (15.8), dragging (15.9), committing a solution to exact stored geometry (15.10), references from the model (15.11) and determinism (15.12).
- **planegcs's part, described in our words.** The kernel takes from planegcs three things, method only: substitution of variables that a constraint makes equal (15.5, stage 1), dependency groups from a QR factorisation to tell redundant, conflicting and partly redundant constraints apart (15.8), and the confirmation of sparse rank by dense QR on small subsystems (15.8). Its sources table records planegcs as LGPL-2.1-or-later, method only, in our own words (15.16; [29](../../engines/geometry/architecture/29-sources-licences.md), planegcs row), from the reading in [G01 evidence 06](../../kit/notes/G01-evidence/06-2d-drafting.md), section 5.
- **What the kernel does beyond planegcs.** Structural decomposition by Dulmage and Mendelsohn and rigid clusters before any numerics (15.5); Gauss-Newton steps damped in Moré's Levenberg-Marquardt manner, from QR and never from normal equations (15.6); convergence in fractions of a grid step, so a 2 m sketch and a 20 km one mean the same (15.6); budgets in work units and cooperative cancelling (15.6, 23.10); and certificates on committed solutions (15.10).
- **The port plan is the kernel's build order** ([30-build-order.md](../../engines/geometry/architecture/30-build-order.md)): dense and sparse linear algebra (G49, G50, G215), sketch planes and 2D to 3D maps (G216), the sketch model and equation framework (G217), the constraint catalogues for points, lines, circles and arcs, then ellipses, then splines (G218 to G220), structural analysis (G221), the block solver and continuation (G222), rigid clusters (G223), degrees of freedom and conflict diagnosis (G224), the commit plan, checks and certificates (G225 to G227), dragging (G228), profiles from sketches (G229), unsolved sketches and project datum systems (G230), and the sketch corpus with oracles (G231), which runs FreeCAD's planegcs and SolveSpace out of process as oracles only (kernel [28](../../engines/geometry/architecture/28-testing.md)). An owner's patent search on constraint solving and sketch diagnosis is G183. Several of these are already open as pull requests (for example G224, G225 to G228 and G231).

So the method and the port plan the board asks for exist, in the place where every program can use them. Writing them again here would make a second copy to keep in step.

## What stays Spare Construct's

The kernel solves; "it still knows no walls" (15.15). Three things remain for Spare Construct:

1. **Project locks.** Locked alignments and dimensions between elements across a project are "a datum system that the program builds": Spare Construct turns each locked element's driving datums (location-line ends, reference planes, grid lines, levels) into sketch entities, one system per plan level or elevation, each lock into a constraint, and writes the result back as element edits in one logged operation (15.15). Whether to have project locks at the start is the map's open decision ("families first, project locks later"), on the features list as [CON-A3-F2-P4](../FEATURES.md), a small decision note with no board piece yet.
2. **Family sketches in the recipe.** The family recipe's sketches, constraints and reference planes (A07) are written against the kernel's sketch schema, so a family's sketch is solved by the kernel's solver and nothing in `construct-families` re-implements it.
3. **The sketch editor.** Constraint glyphs, dimension editing, dragging, and showing the degrees of freedom and conflicts the kernel reports (15.13) are interface work for the family editor (CON-A7-F2-P1) and sketch tools.

## Recommendation

1. **Take kernel chapter 15 as this piece's answer,** and close A11 against it. The method in our own words is 15.5 to 15.8 with planegcs's part named; the port plan is G216 to G231 in the kernel's build order.
2. **Add CON-A3-F2-P4 (project locks decision) to the board** as Spare Construct's remaining solver question, small, with 15.15 as its starting point.
3. **Keep the solver out of Spare Construct's crates.** `construct-solver` holds only the program's side: building datum systems from elements for project locks, and writing results back as edits. Its `lib.rs` says what it waits on; that line should name kernel chapter 15 and A07 when the crate reaches Design.

## Open risks

- **The kernel's solver is not built yet.** Spare Construct's family editor and door generator wait on G217 to G229. That is the kernel's schedule, not this piece's.
- **Project locks at building scale are untested.** One datum system per plan level could hold thousands of constraints; the kernel's benchmarks (15.6, targets) cover sketches, not a whole project's locks.

## Sources

| Source | Licence | Used for |
| --- | --- | --- |
| Kernel map, chapters [15](../../engines/geometry/architecture/15-sketches.md), [28](../../engines/geometry/architecture/28-testing.md), [29](../../engines/geometry/architecture/29-sources-licences.md) and [30](../../engines/geometry/architecture/30-build-order.md) | This repo | Method, planegcs's part, oracles, build order |
| [G01 evidence 06](../../kit/notes/G01-evidence/06-2d-drafting.md) | This repo | The reading of planegcs, method only |

No planegcs source was read for this note.
