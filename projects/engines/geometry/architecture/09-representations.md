# 9. Representations and exactness

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

"The most complex geometry people want, reliably" needs more than one way to represent shape. Spare Kit has five, in one model, each first-class. Evidence: [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), [01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md), [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md).

| Representation | What it is for | Exactness |
| --- | --- | --- |
| B-rep (sections 7 and 8) | Everything editable and drawable: walls, slabs, roofs, families, free-form NURBS surfaces | Exact for flat faces and 2D lines and arcs; Certified for curved |
| Exact mesh | Imported meshes (IFC triangle sets, scans), fast clash, display, the universal fallback | Exact (plane-based, on the grid) |
| Subdivision surface | Organic and sculpted forms, Catmull-Clark with creases | Exact cage; limit surface certified; conversion to NURBS exact on regular regions and approximate with a stored deviation at extraordinary points |
| Field (implicit) | Lattices, variable-thickness shells, smooth blends of many shapes, forms that are simplest as "distance from" | Certified by interval bounds when turned into a mesh or B-rep |
| Point cloud | Scans as reference: snapping, fitting, comparison | Reference only. Never part of a solid |

**Every body carries an exactness class:**

- **Exact:** decided entirely by exact predicates on exact data.
- **Certified(ε):** within a proven bound ε of the true shape, with ε ≤ ε_fit unless the caller asked for more.
- **Approximate(ε):** within a measured, not proven, deviation ε. For example, a subdivision surface's extraordinary points converted to NURBS, or a mesh fitted to a scan.

The class of a result is the least exact of its inputs, and bounds add up. An operation that would lower the class without being asked returns an error offering the explicit alternative instead.

**One boolean across representations.** If all operands are B-rep, the B-rep boolean runs. If any is a mesh, the exact mesh boolean runs on certified tessellations. If any is a field, the field boolean runs (B-rep operands enter as exact distance plus winding number). The result's class follows the rule above. A B-rep boolean that cannot be certified returns an error that offers the mesh or field path explicitly. It never falls back silently.

**The conversion graph.** Every conversion between representations is an operation whose edge is labelled exact, certified or approximate, and which returns its bound. The full graph is in section 16.
