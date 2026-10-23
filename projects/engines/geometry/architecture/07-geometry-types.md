# 7. Geometry types

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

One curve family serves 2D and 3D. Each kind is stored as itself, exactly, never converted to NURBS behind the user's back: arcs stay arcs, so dimensioning, snapping, offsets and file exchange stay exact. Conversion to NURBS is on demand and returns the NURBS, whether it is exact, and its maximum error. Evidence: [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), sections 2 and 7.

**Curves (2D and 3D)**

- Line, ray, infinite line
- Circle and arc; ellipse and elliptical arc (by parameter, not polar angle); parabola; hyperbola
- Helix (exact; neither STEP nor IFC has one)
- Clothoid, and IFC 4.3's cosine, sine and second-, third- and seventh-order polynomial spirals, for road and rail alignments
- Polynomial curve; circular involute
- NURBS, rational or not, periodic or clamped
- Bulge polyline (lines and arcs with optional widths, as in DXF), composite curve, trimmed curve
- Offset curve (2D, 3D, and IFC's offset by distances); curve on surface (pcurve)
- Intersection curve: procedural, defined by its two surfaces, with a cached fitted curve and its certified error

**Surfaces**

- Plane; cylinder; cone; sphere; torus, including the degenerate "lemon" and "apple" forms
- Surface of revolution; linear extrusion; ruled surface
- Sweep (profile, path and frame rule); fixed-reference and directrix sweeps as IFC defines them
- Offset surface
- Rolling-ball blend: procedural, with a cached NURBS and its error
- NURBS
- Subdivision limit surface (section 16)

**How they are written in code.** Storage uses closed enums, one variant per kind: exhaustive matching, stable serialisation, and easy to read for people and AI. Behaviour uses traits: a curve can evaluate, give derivatives, give a conservative bound over any parameter interval, give derivative bounds, convert to NURBS with an error, split, reverse and transform. Procedural kinds carry their definition plus an optional cached approximation and its error. Trimming belongs to the topology's loops, not to a surface kind. Every algorithm's doc comment names its source: a NURBS Book algorithm number or a paper's DOI.

**Transforms can change a kind.** A non-uniform scale turns a circle into an ellipse, and a mirror reverses an arc. Transforms are typed so that this is explicit. A floating-point transform counts as Rigid, or as a uniform scale, only when every length is kept, or scaled by one factor, within a relative ε_ang, certified in interval arithmetic (G03; a proposed second meaning of ε_ang, 6.5). Translations by whole grid steps, rotations by multiples of 90 degrees and mirrors stay exact; other rotations round to the grid and the result says so.
