*Evidence for [G01](../G01-geometry-kernel.md), the Spare Kit geometry kernel decision note. Written 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of each project at the pinned commits linked below. GPL and LGPL code is described in the agent's own words, never copied. Claims it could not confirm from a primary source are marked unverified.*

Spare Kit geometry kernel: curves and surfaces research report (analytic and NURBS)

Pinned sources (all links below point at these commits):
- openNURBS `eb92af3b`
- IFC4.3.x-development `7256780b`
- IfcOpenShell v0.8.0 `7ab6fca8`
- geomdl `9d2555ae`
- verb `f3ba23cd`
- tinyspline `bb3b02cd`
- curvo `11842793`
- G+Smo `b4875c54`
- SISL `e0a82b1a`
- OCCT `53706f78`
- truck `78d699b0`
- LNLib `20cc0fb4`
- Clothoids `dafdc29a`
- Rust libm (in compiler-builtins) `aeab3283`

---

## 1. openNURBS

### Licence (read from the repository, not the badge)
- The [LICENSE](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/LICENSE#L1-L6) file contains only three things: "Copyright (c) 1993-2018 Robert McNeel & Associates. All Rights Reserved.", a warranty disclaimer, and the zlib licence text.
- It contains no grant of permission. Every source header repeats "All rights reserved" and points to opennurbs.org ([opennurbs_curve.h L1-12](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_curve.h#L1-L12)). GitHub reports the licence as `NOASSERTION`.
- The permission exists only on McNeel's web page ["What is openNURBS?"](https://developer.rhino3d.com/guides/opennurbs/what-is-opennurbs/), which says "There are no restrictions. Neither copyright nor copyleft restrictions apply" and "Commercial use is encouraged."
- Verdict: it is not GPL, and it is fine to read. Before any openNURBS code is copied into MPL-2.0 Spare Kit, get written confirmation from McNeel or a legal review. The grant sits outside the repository, and that is a risk over a decades-long project horizon.

### Class model
**Curves**
- [ON_Curve](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_curve.h#L88) is the abstract base, with a virtual `Evaluate` ([L944](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_curve.h#L944)).
- Concrete curve classes: LineCurve, [ArcCurve](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_arccurve.h#L374-L380), PolylineCurve, [PolyCurve](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_polycurve.h#L804-L808), NurbsCurve, CurveOnSurface, CurveProxy and PolyEdgeCurve.
  - ArcCurve holds `m_arc`, `m_t`, and `m_dim` (2 or 3, so an arc can also serve as a 2D trim).
  - PolyCurve holds `m_segment` and `m_t`.
  - The header list confirms there is no ellipse, conic, helix or spiral curve class. Those exist only as NURBS.
- [ON_NurbsCurve fields](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_nurbscurve.h#L1325-L1386):
  - `m_dim`, `m_is_rat`, `m_order` and `m_cv_count`.
  - Rational control points are stored in homogeneous form.
  - The knot vector length is order + cv_count − 2. This is McNeel's convention with no "superfluous" end knots. STEP and the NURBS Book use cv_count + order knots, so every importer must convert.

**Surfaces**
- [ON_Surface](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_surface.h#L53) offers tolerance-based recognisers `IsPlanar`, `IsSphere`, `IsCylinder`, `IsCone` and `IsTorus` ([L310-374](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_surface.h#L310-L374)). openNURBS recognises quadrics after the fact; it does not store them as types.
- [ON_PlaneSurface](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_planesurface.h#L42) holds a plane plus domain and extents.
- [ON_RevSurface](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_revsurface.h#L29-L56) holds curve, axis, angle interval, `m_t` and `m_bTransposed`. Spheres, cylinders, cones and tori are all RevSurfaces.
- [ON_SumSurface](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_sumsurface.h#L33-L36) is a translational surface: two curves plus a base point.
- [ON_NurbsSurface](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_nurbssurface.h#L1181-L1203).
- [ON_Extrusion](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_beam.h#L49) lives in beam.h.
- [ON_OffsetSurface](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_offsetsurface.h#L277-L286) is labelled "still a work in progress… can not be saved in files, used as a brep surface".

**B-rep** (a tolerant B-rep)
- Geometry pools are kept separate from topology tables: `m_C2`, `m_C3`, `m_S` for geometry and `m_V`, `m_E`, `m_T`, `m_L`, `m_F` for topology ([L4068-4080](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_brep.h#L4068-L4080)).
- Vertex `m_tolerance` ([L140-158](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_brep.h#L140-L158)).
- Edge `m_tolerance` is the maximum distance from the 3D edge curve to any adjacent face ([L362-371](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_brep.h#L362-L371)).
- Trim types are boundary, mated, seam, singular, crvonsrf, ptonsrf and slit ([L432-462](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_brep.h#L432-L462)).
- Trims carry `m_tolerance[2]`, a separate parameter-space accuracy in u and v ([L731-769](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_brep.h#L731-L769)).
- Loop types: outer (counter-clockwise), inner (clockwise), slit, crvonsrf, ptonsrf ([L870-881](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_brep.h#L870-L881)).
- Face fields `m_li` (outer loop first), `m_si` and `m_bRev` ([L1195-1197](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_brep.h#L1195-L1197)).

### What is not included
- McNeel's page lists the gaps: closest point, intersections, meshing, SubD→Brep, interpolation, booleans and mass properties.
- The source agrees. [opennurbs_intersect.h](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_intersect.h#L43-L119) covers only elementary cases (line/line, line/plane, bounding box, sphere).
- What it does have: [de Boor evaluation](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_evaluate_nurbs.cpp#L1248), [de Casteljau](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_evaluate_nurbs.cpp#L157), [knot insertion](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_knot.cpp#L1296), and [degree elevation by repeated increments](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_nurbscurve.cpp#L2781-L2801).
- In short: openNURBS has no SSI and no booleans.

### Is 3dm a good reference for our file model?
- **Ideas worth copying:**
  - Length-prefixed typecode chunks that a reader can skip, with a CRC bit ([3dm.h L17-52](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_3dm.h#L17-L52), [L85-93](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_3dm.h#L85-L93)).
  - Classes tagged by UUID ([L281-347](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_3dm.h#L281-L347)).
  - Geometry pools referenced by index from topology.
  - Explicit tolerances on every vertex, edge and trim.
- **Do not copy the format itself:**
  - The specification is effectively the C++ code.
  - The typecode space belongs to McNeel.
  - It is NURBS-centric: there is no exact ellipse, cone or clothoid.
  - The knot convention is non-standard.
- Treat 3dm as an import/export target only.

## 2. Curve and surface types a full-scale kernel needs

### STEP: ISO 10303-42:2021 ([schema](https://steptools.com/stds/smrl/data/resource_docs/geometric_and_topological_representation/sys/4_schema.htm), extracted)
**Curves**
- `curve` has these subtypes: line, conic (circle, ellipse, hyperbola, parabola), clothoid, circular_involute, pcurve, surface_curve (intersection_curve, seam_curve), offset_curve_2d, offset_curve_3d, curve_replica.
- `bounded_curve` has: polyline, b_spline_curve (bezier, uniform, quasi_uniform or with_knots, ANDOR rational), trimmed_curve, bounded_pcurve, bounded_surface_curve, composite_curve, locally_refined_spline_curve.

**Surfaces**
- elementary: plane, cylindrical, conical, spherical, toroidal; degenerate_toroidal_surface and dupin_cyclide_surface also exist.
- swept: linear extrusion, revolution, surface_curve_swept, fixed_reference_swept.
- bounded: b_spline_surface variants, rectangular_trimmed, curve_bounded, rectangular_composite, locally_refined_spline_surface.
- Also offset_surface, surface_replica and oriented_surface.

**Missing from STEP:** there is no helix, no blend surface and no polynomial spiral entity. Fillets and helices therefore travel as B-splines.

### IFC4X3_ADD2 (subtype tree resolved from IfcOpenShell's generated schema, [example lines](https://github.com/IfcOpenShell/IfcOpenShell/blob/7ab6fca86733a5cda959114e56244eee0a4ec822/src/ifcparse/Ifc4x3_add2-schema.cpp#L3573-L3575))
**Curves (IfcCurve)**
- IfcBoundedCurve:
  - BSplineCurve → WithKnots → Rational.
  - CompositeCurve → CompositeCurveOnSurface → BoundaryCurve → OuterBoundaryCurve; also GradientCurve and SegmentedReferenceCurve.
  - IndexedPolyCurve, Polyline, TrimmedCurve.
- IfcConic: Circle and Ellipse only. There is no parabola or hyperbola.
- IfcLine.
- IfcOffsetCurve: 2D, 3D, and ByDistances.
- IfcPcurve.
- IfcPolynomialCurve.
- IfcSpiral: Clothoid, CosineSpiral, SineSpiral, and Second-, Third- and SeventhOrderPolynomialSpiral.
- IfcSurfaceCurve: Intersection and Seam.

**Surfaces (IfcSurface)**
- Bounded: BSplineSurface variants, CurveBoundedPlane, CurveBoundedSurface, RectangularTrimmedSurface.
- Elementary: Cylindrical, Plane, Spherical, Toroidal. IFC 4.3 has no conical surface.
- SectionedSurface.
- Swept: LinearExtrusion, Revolution.

**Alignment and solid details**
- [IfcClothoid](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometryResource/Entities/IfcClothoid.md#L10-L35) is parameterised by u, not by arc length: s = A·u·√π, κ = A·s/|A³|, A = √(LR). The formula comes from 10303-42.
- [IfcSpiral](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometryResource/Entities/IfcSpiral.md#L6-L11) defines x = ∫cos Θ dl and y = ∫sin Θ dl. Higher-order spirals therefore need numerical quadrature.
- [IfcCurveSegment](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometryResource/Entities/IfcCurveSegment.md#L6-L19): SegmentStart and SegmentLength are length measures "for the foreseeable future".
- [IfcSectionedSolidHorizontal](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcSectionedSolidHorizontal.md#L6-L30):
  - Profile points with the same tag are linearly interpolated between sections.
  - The profile frame comes from IfcAxis2PlacementLinear, not from the directrix tangent.
  - Where the directrix is not tangent-continuous, a half-angle miter is applied.

### Recommended kernel type set
**Curves (2D and 3D)**
- Line, Circle/Arc, Ellipse, Parabola, Hyperbola.
- Helix (exact; neither STEP nor IFC has one).
- Clothoid and the IFC polynomial, sine and cosine spirals.
- PolynomialCurve and CircularInvolute.
- NURBS, with periodic and rational flags.
- Polyline with arc bulges (DWG LWPOLYLINE).
- Composite, Trimmed, Offset (2D, 3D, by-distances), PCurve/SurfaceCurve.
- IntersectionCurve: procedural, with a cached fitted approximation and its error.

**Surfaces**
- Plane, Cylinder, Cone, Sphere, Torus (including the degenerate lemon/apple forms).
- Revolution, LinearExtrusion, Ruled.
- Sweep (profile, rail and frame law).
- Offset.
- RollingBallBlend (procedural).
- NURBS.
- Trimming belongs to B-rep loops, not to a surface type.

### Why keep analytic types exact
1. Exchange semantics. DWG, IFC and STEP carry arcs, circles and cylinders as such, and dimensioning, snapping and radius queries depend on that.
2. Closed-form algorithms. Point projection is closed-form for these types, and plane/quadric intersections yield exact conics: [Goldman & Miller 1991, natural quadrics](https://people.eecs.ku.edu/~jrmiller/Papers/GeomAppNPQSIC.pdf) (*title of the linked PDF unverified*). The NURBS path is iterative instead.
3. Transcendental curves. Clothoids and spirals have no exact rational form, and stationing needs exact arc length.
4. Offsets stay in type: a circle offsets to a circle and a cylinder to a cylinder. A NURBS offset is not a NURBS ([Maekawa 1999, CAD 31(3):165-173](https://doi.org/10.1016/S0010-4485(99)00013-5)).
5. Parametrisation. A rational-quadratic circle does not run at uniform angular speed, and a NURBS sphere has degenerate poles.

Conversion to NURBS should therefore be on demand and return `{nurbs, exact: bool, max_err}`.

## 3. Core algorithms and the proven method for each

Algorithm numbers below are from the NURBS Book, 2nd edition. Each number was verified against geomdl's or verb's source comments, which cite them.

### Basis functions and evaluation
- Basis: A2.1 FindSpan, A2.2 BasisFuns, A2.3 DersBasisFuns, A2.5 (geomdl [helpers.py L24](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/helpers.py#L24), [L144](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/helpers.py#L144), [L292](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/helpers.py#L292), [L382](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/helpers.py#L382)).
- Evaluation: A3.1, A3.5 and A3.6 (verb [Eval.hx L206](https://github.com/pboyer/verb/blob/f3ba23cdccbab77102b686021460cf9779df240f/src/verb/eval/Eval.hx#L206), [L296](https://github.com/pboyer/verb/blob/f3ba23cdccbab77102b686021460cf9779df240f/src/verb/eval/Eval.hx#L296), [L738](https://github.com/pboyer/verb/blob/f3ba23cdccbab77102b686021460cf9779df240f/src/verb/eval/Eval.hx#L738)), plus A3.3 and A3.7 derivative control points (geomdl [L1075](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/helpers.py#L1075), [L1115](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/helpers.py#L1115)). Rational evaluation goes through homogeneous coordinates.
- De Boor and de Casteljau are numerically stable. The Bernstein-basis conditioning is in Farouki & Rajan, CAGD 1987 (*details unverified*).

### Knot operations and degree change
- Knot insertion A5.1 ([verb Modify L581](https://github.com/pboyer/verb/blob/f3ba23cdccbab77102b686021460cf9779df240f/src/verb/eval/Modify.hx#L576-L582)).
- Knot refinement A5.4 ([verb L490](https://github.com/pboyer/verb/blob/f3ba23cdccbab77102b686021460cf9779df240f/src/verb/eval/Modify.hx#L490), [geomdl L828](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/helpers.py#L828)).
- Bézier decomposition A5.6.
- Knot removal with a tolerance, A5.8 ([geomdl L625](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/helpers.py#L625)).
- Degree elevation A5.9 (p.206) and degree reduction A5.11 (p.223, approximate). Both numbers are secondary-sourced, via [pyformex](https://pyformex.nongnu.org/doc-3.0/ref/lib.nurbs_e.html).
- G+Smo has MPL-licensed Boehm insertion and refinement "based on THE NURBS BOOK" ([gsBoehm.h L1-12](https://github.com/gismo/gismo/blob/b4875c54f9d17ff3d14cbd6842b4d2cd6e6cc732/src/gsNurbs/gsBoehm.h#L1-L12), [L92-141](https://github.com/gismo/gismo/blob/b4875c54f9d17ff3d14cbd6842b4d2cd6e6cc732/src/gsNurbs/gsBoehm.h#L92-L141)).

### Point inversion and projection
The NURBS Book §6.1 Newton method with two zero tests is the base. Make it robust as [Ma & Hewitt 2003](https://doi.org/10.1016/S0167-8396(03)00021-9) do:
1. Decompose into Bézier spans.
2. Prune spans using a convex-hull distance lower bound against the best upper bound found so far.
3. Run Newton clamped to the span, with a bisection fallback whenever f(t) = C′·(C−P) changes sign.
4. Use a fixed iteration cap and deterministic tie-breaks.

A second-order alternative is [Hu & Wallner 2005](https://doi.org/10.1016/j.cagd.2004.12.001). Contrast with OCCT's `Extrema_GenExtPS`, which seeds from a user-chosen sampling grid (NbU, NbV) and finds all extrema only if that grid "is great enough" ([hxx L47-58](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKGeomBase/Extrema/Extrema_GenExtPS.hxx#L47-L58)). That is a heuristic; use bound-based pruning instead.

### Curve-curve intersection
- Bézier clipping (Sederberg & Nishita, CAD 22(9):538-549, 1990).
- Alternatives with convergence results: quadratic clipping (Bartoň & Jüttler 2007), knot-insertion intersection (Mørken, Reimers & Schulz 2009), and Schulz 2009 showing Bézier clipping converges quadratically (*all three unverified here*).
- Finish with Newton, flag tangential (double) roots, and run a Hausdorff check to detect overlaps.
- Existing implementations:
  - G+Smo: bounding-box subdivision with a pseudo-curvature cutoff, then projected Gauss-Newton ([cci.h L246-330](https://github.com/gismo/gismo/blob/b4875c54f9d17ff3d14cbd6842b4d2cd6e6cc732/src/gsNurbs/gsCurveCurveIntersection.h#L246-L330), [L350-425](https://github.com/gismo/gismo/blob/b4875c54f9d17ff3d14cbd6842b4d2cd6e6cc732/src/gsNurbs/gsCurveCurveIntersection.h#L350-L425)).
  - curvo: bounding-volume-hierarchy traversal with tolerance, then a solver. Its tests cover scale independence, touching curves, near-coincident intersections and repeat determinism ([L73-120](https://github.com/mattatz/curvo/blob/11842793ed92640cb78e8ed86d731fdb55e85e34/src/intersects/curve_curve/intersection_curve_curve.rs#L73-L120), [tests L277-433](https://github.com/mattatz/curvo/blob/11842793ed92640cb78e8ed86d731fdb55e85e34/src/intersects/curve_curve/intersection_curve_curve.rs#L277-L433)).

### Curve-surface intersection
- Bézier clipping on patches: [Nishita, Sederberg & Kakimoto, SIGGRAPH 1990](https://history.siggraph.org/?p=122078).
- Patrikalakis-Maekawa §5.7.2: subdivision with minimisation, or the Interval Projected Polyhedron (IPP) solver ([hyperbook node93-94](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node94.html)).

### Surface-surface intersection (SSI)
- Taxonomy (lattice, subdivision, marching) and failure modes are in Patrikalakis-Maekawa §5.8.2 ([subdivision](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node108.html), [marching](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node109.html)):
  - Subdivision can miss or invent small loops and blows up in data.
  - Marching needs a start point on every branch.
  - The marching direction n₁×n₂ degenerates where the surfaces are tangent ([§6.4](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node122.html)).
- Marching method: [Barnhill & Kersey, CAGD 7:257-280 (1990)](https://scholars.georgiasouthern.edu/en/publications/a-marching-method-for-parametric-surfacesurface-intersection-3/). It handles tangent and branch points, approximates step length, and relaxes points onto boundaries.
- Start points:
  - Border points: intersect each boundary curve with the other surface.
  - Loops: collinear-normal points split them (Sederberg & Meyers, CAGD 5:161-171, 1988).
  - Gauss-map loop tests: [Hohmeyer 1992](https://www2.eecs.berkeley.edu/Pubs/TechRpts/1992/6140.html).
- [Grandine & Klein 1997](https://acnpsearch.tweb-dev.unibo.it/singlejournalindex/3666035) resolve the topology first, then trace with a differential-algebraic solver.
- OCCT's method, summarised in my own words because it is LGPL:
  - It dispatches by surface class: analytic×analytic in closed form, implicit×parametric by marching on the implicit function, and parametric×parametric seeded from polyhedral approximations.
  - It then marches ([IntWalk_PWalking.hxx L31-54](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKGeomAlgo/IntWalk/IntWalk_PWalking.hxx#L31-L54)); an open line "stops at the tangent point".
  - Finally it fits B-splines.
- Mesh-based SSI: [verb](https://github.com/pboyer/verb/blob/f3ba23cdccbab77102b686021460cf9779df240f/src/verb/eval/Intersect.hx#L49-L69) and [truck](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-shapeops/src/transversal/intersection_curve/mod.rs#L151-L176) intersect the two tessellations, then refine. This can miss loops smaller than the mesh and fails near tangency.

### Offsets
- Analytic types offset exactly.
- NURBS: approximate with an error-bounded fit ([Piegl & Tiller, CAD 31(2):147-156, 1999](https://cse.usf.edu/~lespiegl/research.htm)), then trim self-intersections.
- curvo offsets a tessellated polyline ([L358](https://github.com/mattatz/curvo/blob/11842793ed92640cb78e8ed86d731fdb55e85e34/src/offset/offset_nurbs_curve.rs#L358)), so its offsets are approximate only.

### Construction surfaces
- Lofting and skinning: NURBS Book ch.10 ([LNLib's chapter map](https://github.com/BIMCoderLiang/LNLib/blob/20cc0fb4b22233069a5e10af32b3a34df7faf588/README.md#L17-L29)). Make the curves compatible (A5.9, then merge knots with A5.4), then interpolate across them (A9.1). Approximate skinning is in Piegl & Tiller, CAD 28(9), 1996.
- Sweeps: use rotation-minimising frames by the double-reflection method (Wang, Jüttler, Zheng & Liu, [ACM TOG 27(1) 2008](https://i.cs.hku.hk/GraphicsGroup/publications/pdf/Computation%20of%20rotation%20minimizing%20frames.pdf)). tinyspline implements it ([L1930-2000](https://github.com/msteinbeck/tinyspline/blob/bb3b02cdfaa039aa236a5240f2ba8bce66bea527/src/tinyspline.h#L1930-L2000)).
- Gordon and Coons surfaces: Boolean-sum construction. G+Smo has an MPL `gsCoonsPatch`.

### Fillets and blends
- Rolling ball: the spine is the intersection of the two offset surfaces at distance r, and the cross-sections are circular arcs.
- Store the procedural definition plus a NURBS approximation and its error.
- Survey: [Vida, Martin & Várady, CAD 26(5):341-365, 1994](https://eprints.sztaki.hu/682/).
- Vertex (n-sided) blends remain the hard part.

### Fitting
- A9.1 curve interpolation ([fitting.py L19](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/fitting.py#L19)).
- A9.4 surface interpolation ([L60](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/fitting.py#L60)).
- Least squares, pp.410-413 ([L119](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/fitting.py#L119)).
- A9.7 surface approximation ([L216](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/fitting.py#L216)).
- Knot placement, Eqs. 9.68-9.69 ([L389](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/fitting.py#L389)).

### Clothoids
- Fresnel integrals: [Bertolazzi & Frego, BSD-2-Clause](https://github.com/ebertolazzi/Clothoids/blob/dafdc29acee5c4f25e777c6cffe56ecb39b0ec11/license.txt) use a power series, then a rational approximation, then an asymptotic expansion ([Fresnel.cc L165-300](https://github.com/ebertolazzi/Clothoids/blob/dafdc29acee5c4f25e777c6cffe56ecb39b0ec11/src/Fresnel.cc#L165-L300)).
- The IFC polynomial spirals need fixed-order Gauss–Legendre quadrature.

## 4. Open implementations and licences (verified from LICENSE files)

- **G+Smo** ([MPL-2.0](https://github.com/gismo/gismo/blob/b4875c54f9d17ff3d14cbd6842b4d2cd6e6cc732/LICENSE.txt), with per-file MPL headers).
  - An isogeometric-analysis library: Boehm knot insertion, degree elevation, THB-splines, fitting, Coons patches, trimmed surfaces, and basic curve-curve intersection. It bundles openNURBS for 3dm I/O.
  - Same licence as ours, so code can be ported keeping the MPL notices.
  - Port value: good for knot work and fitting; weak for intersection.
- **SISL** ([AGPL-3.0 plus a §7(b) "producer line" requirement and a paid commercial licence](https://github.com/SINTEF-Geometry/SISL/blob/e0a82b1a851e932657f3e1af25033fbbcd33e907/src/s1859.c#L1-L38)).
  - The most complete open SSI (`s1859`), but AGPL. Do not read the code; use SINTEF's published papers instead.
- **tinyspline** ([MIT](https://github.com/msteinbeck/tinyspline/blob/bb3b02cdfaa039aa236a5240f2ba8bce66bea527/LICENSE)).
  - Curves only, in C: evaluation, knot insertion, split, derivative, conversion to Béziers, degree elevation, Catmull-Rom and natural cubic interpolation, RMF.
  - Small and clean.
- **verb** ([MIT](https://github.com/pboyer/verb/blob/f3ba23cdccbab77102b686021460cf9779df240f/LICENSE)), written in Haxe.
  - Broad coverage, but last pushed 2025-04.
  - Its adaptive curve sampling uses `Math.random()` ([Tess.hx L121-128](https://github.com/pboyer/verb/blob/f3ba23cdccbab77102b686021460cf9779df240f/src/verb/eval/Tess.hx#L121-L128)). That is non-deterministic, so do not port it verbatim.
- **geomdl/NURBS-Python** ([MIT](https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/LICENSE)).
  - The best NURBS-Book-annotated code available. Use it as a test oracle; it is too slow to port as-is.
- **curvo** ([MIT LICENSE](https://github.com/mattatz/curvo/blob/11842793ed92640cb78e8ed86d731fdb55e85e34/LICENSE); the README badge says MIT/Apache). Rust, active (last pushed 2026-10-19).
  - Dependencies: nalgebra, argmin, spade (constrained Delaunay) and robust.
  - Features: trimmed surfaces, curve booleans (Greiner-Hormann, [clip.rs L86-87](https://github.com/mattatz/curvo/blob/11842793ed92640cb78e8ed86d731fdb55e85e34/src/boolean/clip.rs#L86-L87)), fillets, offsets, and boundary-constrained adaptive tessellation.
  - It already replaced verb's random sampling with a golden-ratio sequence "so that the tessellation is reproducible" ([L155-163](https://github.com/mattatz/curvo/blob/11842793ed92640cb78e8ed86d731fdb55e85e34/src/tessellation/tessellation_curve.rs#L155-L163)).
  - It uses std trigonometry, which must be swapped for the libm crate before wasm output can be bit-identical (see §6).
  - It has no general SSI.
  - The best Rust starting point.
- **truck** (Apache-2.0).
  - Rust B-rep kernel. SSI is mesh-based; tessellation shares edge polylines between faces by edge id ([tri.rs L54-98](https://github.com/ricosjp/truck/blob/78d699b0792b39e27c2bf073b682986701644e91/truck-meshalgo/src/tessellation/triangulation.rs#L54-L98)).
- **OCCT GeomAPI/GeomInt** (LGPL-2.1 plus the [OCCT exception](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/OCCT_LGPL_EXCEPTION.txt)). Method study only.
- **LNLib** (LGPL-2.1). Method study only.

## 5. Trimmed-surface tessellation that is watertight and error-bounded

OCCT documents the edge-first pattern ([mesh.md L74-82, L111-116](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/mesh/mesh.md#L74-L116)):
- Edges are discretised first into a "coherent skeleton".
- Face meshing then reuses those edge polygons.
- Linear and angular deflection are separate from interior deflection.
- Self-intersecting wires are healed by refining or splitting edges ([L162-165](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/mesh/mesh.md#L162-L165)).

Recommended pipeline:
1. Sample each edge exactly once, against the error bound of every face that uses it. Store the 3D points and map them to each face's (u,v) through its pcurve.
2. Faces use those exact 3D vertex values, never S(pcurve(t)). Neighbouring faces then share bit-identical vertices, so the mesh is watertight by construction even when the B-rep gap is up to the edge tolerance.
3. Run a constrained Delaunay triangulation in parameter space with robust 2D predicates (spade plus robust).
4. Refine the interior until a provable chordal bound holds. The sufficient condition is deviation ≤ ⅛(h_u²M_uu + 2h_uh_vM_uv + h_v²M_vv), with each M taken from Bernstein bounds of the second-derivative patches (Filip, Magedson & Markot 1986, pp.295-311; *journal attribution unverified*). For curves the bound is ⅛h²‖C″‖. Rational patches need weight-aware bounds.
5. Normal-angle limits are an extra criterion, not a substitute. curvo and verb use only normal heuristics, which are not bounds ([options L7-27](https://github.com/mattatz/curvo/blob/11842793ed92640cb78e8ed86d731fdb55e85e34/src/tessellation/adaptive_tessellation_option.rs#L7-L27)).

View-dependent refinement:
- Convert the object-space bound to pixels per level of detail.
- Make the level-of-detail decision per edge, as the maximum over its faces, so no T-junctions appear.
- GPU trimming is for display only and must never feed back into geometry ([Guthe et al. 2005](https://history.siggraph.org/?p=99895); [Schollmeyer & Fröhlich 2009](https://www.uni-weimar.de/de/gpucast/direct-trimming-of-nurbs-surfaces-on-the-gpu)).

Further reading: [Piegl & Tiller, CAD 27(1) 1995 and 30(1) 1998](https://cse.usf.edu/~lespiegl/research.htm).

## 6. Interval and robust approaches

**Bounds**
- Bernstein and B-spline convex-hull bounds, with positive weights for rationals, plus hodograph derivative bounds, are cheap and conservative.
- Interval arithmetic: Patrikalakis-Maekawa §4.7-4.9 ([interval](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node45.html), [rounded interval](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node46.html), [IPP](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node52.html)).
- Robust interval SSI: Hu, Maekawa, Patrikalakis & Ye, CAD 29(9):617-627, 1997.

**Outward rounding on wasm**
- Wasm and Rust have no directed rounding modes. Emulate it with `next_down`/`next_up`, stable since Rust 1.86 ([RFC 3173](https://rust-lang.github.io/rfcs/3173-float-next-up-down.html)).

**Determinism**
- [Rust std](https://doc.rust-lang.org/std/primitive.f64.html) says `sin`, `cos` and `atan2` precision "is non-deterministic". `sqrt` and `mul_add` are correctly rounded.
- [RFC 3514](https://rust-lang.github.io/rfcs/3514-float-semantics.html): basic operations follow IEEE 754; only NaN bits are non-deterministic.
- Therefore all transcendental functions must go through the [libm crate (MIT)](https://github.com/rust-lang/compiler-builtins/blob/aeab328368d6ed8dc6acba47b7db6ad08be953d2/libm/LICENSE.txt).

**Certified topology**
- Bartoň, Elber & Hanniel use no-loop and single-component tests over a 4-D subdivision and guarantee SSI topology ([paper](https://gershon.cs.technion.ac.il/papers/TopolGuarSSI.pdf)).
- [Hass, Farouki, Han, Song & Sederberg, Adv. Comput. Math. 27 (2007)](https://www.math.ucdavis.edu/~hass/Research/Papers/GeomDesign/Consistency.pdf) couple topology resolution with domain decomposition, so the trimmed surfaces are exactly consistent.
- [Yang, Jia & Yan, SIGGRAPH Asia 2023](https://history.siggraph.org/?p=214449) give a "practical topology-guaranteed" SSI. They report that OCCT and ACIS fail on some of their hard examples.
- [Cheng, Zhang, Xiao & Li, SIGGRAPH 2023, DOI 10.1145/3592452](https://history.siggraph.org/learning/topology-driven-approximation-to-rational-surface-surface-intersection-via-interval-algebraic-topology-analysis-by-cheng-zhang-xiao-and-li) use interval algebraic topology.
- [Gao, Lin & Li, arXiv 2604.12631 (2026)](https://arxiv.org/abs/2604.12631) still call complex SSI topology "an open challenge".

**What is practical**
- Interval and Bernstein exclusion tests are cheap. They guarantee no missed transversal intersections above tolerance, so make them always on.
- Full topology certification is expensive. Make it an opt-in "certify" mode, triggered automatically when monitors detect near-parallel normals or near-singularities.

## 7. Recommendation

### Type system
- **Storage:** closed enums, which give exhaustive matching, stable serialisation and are easy for AIs and people to read:
  - `Curve2`, `Curve3`, `Surface` with one variant per type in §2.
  - Procedural variants (Offset, Intersection, Blend, Sweep) carry their definition plus `approx: Option<(Nurbs, max_err)>`.
- **Behaviour:** traits.
  - `ParamCurve<const D>`: `domain`, `eval`, `derivs(t, n)`, `bound(interval) -> Aabb` (conservative), `deriv_bound(interval, k)`, `to_nurbs(tol) -> {nurbs, exact, max_err}`, `split`, `reverse`, `transform`.
  - `ParamSurface`: the same, in two parameters.
  - Topology stores geometry by index, with openNURBS-style tolerances on every vertex, edge and trim.
- **Documentation:** every algorithm's doc comment names its source (NURBS Book algorithm number or paper DOI), as geomdl does.

### Port order
1. Numeric core: libm, an outward-rounded interval type, basis A2.x. Port from geomdl (MIT) or G+Smo (MPL).
2. Evaluation A3.x/A4.x and knot operations A5.1, A5.4, A5.6, A5.8, A5.9, A5.11. Analytic closed forms. Clothoid from Bertolazzi (BSD-2); IFC spirals by Gauss–Legendre.
3. Projection: Ma-Hewitt pruning plus safeguarded Newton.
4. 2D curve-curve intersection, booleans, offsets, fillets for drafting. Start from curvo (MIT) and its tests; add analytic special cases and DWG bulge polylines.
5. Fitting (A9.x), loft, sweep with RMF, revolve, Coons/Gordon.
6. Shared-edge tessellation (§5).
7. SSI: analytic pairs first, then general SSI via bound-based start points, loop splitting, Barnhill-Kersey marching and flagged tangential handling.
8. Offset surfaces and rolling-ball blends.
9. Opt-in certified SSI mode.

### Guarantees we can honestly promise
- Bit-identical native and wasm results, subject to these rules: no implicit fused multiply-add, libm-only transcendentals, deterministic iteration and tie-break order.
- Evaluation is backward-stable.
- Every NURBS conversion carries an exactness flag and an error bound.
- Tessellation is watertight by construction and meets a provable chordal bound for polynomial patches.
- Intersections are complete for transversal cases above tolerance; tangencies and overlaps are returned as flagged ε-regions.

**Do not promise** correct SSI topology in every degenerate configuration.

## Adopt / Avoid / Open questions

### Adopt
- Exact analytic types, including helix and spirals; NURBS on demand.
- A tolerant B-rep with per-element tolerances, plus 3dm's chunk, length, CRC and UUID ideas.
- Edge-first shared tessellation with derivative-bound refinement.
- Bernstein and interval exclusion tests.
- libm and `next_up`/`next_down`.
- Porting bases: curvo, geomdl, G+Smo, tinyspline, Clothoids.

### Avoid
- SISL source (AGPL).
- Copying OCCT or LNLib code (LGPL).
- Mesh-intersection SSI (as in verb and truck) as the primary method.
- verb's randomised sampling.
- Grid-seeded extrema in the style of OCCT.
- std trigonometry in the kernel.
- Using the 3dm format itself as our native format.
- Incorporating openNURBS code before its licence is confirmed.

### Open questions
- Written licence confirmation from McNeel.
- Exact weight-aware second-derivative bounds for rational patches.
- Whether locally refined splines (LR-B / THB, which are now in STEP) are needed for massing.
- How far to take certified SSI, and its performance budget.
- An IFC implementer agreement on spiral parameter spaces (IfcCurveSegment).
- Exact page and algorithm numbers for NURBS Book ch.10, and the details marked *unverified* above.
