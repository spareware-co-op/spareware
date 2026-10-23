# Spare Kit geometry kernel: data models, BIM coverage, meshes/clash, verification

*Evidence for [G01](../G01-geometry-kernel.md), the Spare Kit geometry kernel decision note. Written 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of each project at the pinned commits linked below. GPL and LGPL code is described in the agent's own words, never copied. Claims it could not confirm from a primary source are marked unverified.*

All sources fetched 2026-10-21. Pinned SHAs: IfcOpenShell `741d060e` (branch v0.9.0), web-ifc `dd44aa77`, OCCT `53706f78`, Manifold `d056b8fb`, parry `b3c3584c`, meshoptimizer `f0a684f1`, buildingSMART IFC4.3.x-development `7256780b`. Claims I couldn't confirm against a primary source are marked *unverified*. Anything labelled "(our analysis)" is my own reasoning and has no source.

---

## A1. IFC 4.3 (ISO 16739-1:2024) geometry

**Status and licence**
- IFC 4.3.2.0 ("IFC 4.3 ADD2") is ISO 16739-1:2024, status "Official" since April 2024 ([bSI spec database](https://technical.buildingsmart.org/standards/ifc/ifc-schema-specifications/), [ISO 84123](https://www.iso.org/standard/84123.html)).
- Official HTML pages follow the pattern `https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/<Entity>.htm`. They returned HTTP 403 to automated fetches today.
- I therefore read the normative Markdown source in [IFC4.3.x-development@7256780](https://github.com/buildingSMART/IFC4.3.x-development/tree/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource). Its branch `ifc4.3-main` may contain edits made after 4.3.2.0.
- **The documentation is CC BY-ND 4.0** ([LICENSE.md](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/LICENSE.md)). We may link to and quote it, but we may not adapt it. Kernel docs must be written fresh.

Official-page links below use the pattern `H/<E>.htm`, where H = `https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical`.

**Swept solids**
- [IfcExtrudedAreaSolid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcExtrudedAreaSolid.htm).
- [IfcExtrudedAreaSolidTapered](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcExtrudedAreaSolidTapered.htm): each side face is a *ruled surface* between corresponding edges of the start and end sections ([src L3-6](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcExtrudedAreaSolidTapered.md?plain=1#L3-L6)).
  - (our analysis) With polygon sections these quads are generally not planar. A lossless kernel needs a bilinear/ruled surface type, or must record that the faces were triangulated.
- [IfcRevolvedAreaSolid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcRevolvedAreaSolid.htm) and [IfcRevolvedAreaSolidTapered](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcRevolvedAreaSolidTapered.htm).
- [IfcSweptDiskSolid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSweptDiskSolid.htm) and [IfcSweptDiskSolidPolygonal](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSweptDiskSolidPolygonal.htm) ([src L8, L31-35](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcSweptDiskSolid.md?plain=1#L8-L35)):
  - Where the directrix is not tangent-continuous, the solid is mitred at the half angle.
  - Any arc segment's radius must exceed the disk radius.
- [IfcDirectrixCurveSweptAreaSolid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcDirectrixCurveSweptAreaSolid.htm) is **abstract** in 4.3. It carries the optional StartParam/EndParam ([src](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcDirectrixCurveSweptAreaSolid.md)). Its subtypes:
  - [IfcFixedReferenceSweptAreaSolid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcFixedReferenceSweptAreaSolid.htm): the profile x-axis is FixedReference projected onto the plane normal to the directrix; the directrix must be tangent-continuous ([src](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcFixedReferenceSweptAreaSolid.md)).
  - [IfcSurfaceCurveSweptAreaSolid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSurfaceCurveSweptAreaSolid.htm).
  - [IfcDirectrixDerivedReferenceSweptAreaSolid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcDirectrixDerivedReferenceSweptAreaSolid.htm): adds the tangent-plane rotation of, for example, segmented reference curves ([src](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcDirectrixDerivedReferenceSweptAreaSolid.md)).
- [IfcSectionedSolidHorizontal](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSectionedSolidHorizontal.htm) ([src L6-30](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcSectionedSolidHorizontal.md?plain=1#L6-L30)):
  - Points with the *same tag* are interpolated linearly between sections; tags come from `IfcCartesianPointList2D.TagList`.
  - The profile normal comes from IfcAxis2PlacementLinear, not from the directrix tangent.
  - Consecutive arbitrary profiles must have the same number of points.
  - Tangent breaks are mitred.
- Also [IfcSectionedSolid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSectionedSolid.htm), [IfcSectionedSurface](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSectionedSurface.htm) and [IfcSectionedSpine](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSectionedSpine.htm).

**CSG**
- [IfcBooleanResult](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcBooleanResult.htm).
- [IfcBooleanClippingResult](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcBooleanClippingResult.htm) ([src](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcBooleanClippingResult.md)): the first operand is a swept solid or a boolean result, the second is always a half-space, and the operator is always Difference.
- Half-spaces: [IfcHalfSpaceSolid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcHalfSpaceSolid.htm), [IfcPolygonalBoundedHalfSpace](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcPolygonalBoundedHalfSpace.htm), [IfcBoxedHalfSpace](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcBoxedHalfSpace.htm).
- [IfcCsgSolid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcCsgSolid.htm) with primitives: Block, RectangularPyramid, RightCircularCone, RightCircularCylinder, Sphere.

**B-rep**
- [IfcFacetedBrep](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcFacetedBrep.htm), with or without voids.
- [IfcAdvancedBrep](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcAdvancedBrep.htm), with or without voids, built from [IfcAdvancedFace](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcAdvancedFace.htm):
  - Face surfaces may be elementary, swept or B-spline.
  - Edge curves are limited to Line, Conic, Polyline and BSplineCurve.
  - Periodic faces use IfcSeamCurve.
  - IfcIntersectionCurve carries both the 3D curve and the uv p-curves ([AdvancedBrep src](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcAdvancedBrep.md?plain=1#L18), [AdvancedFace src](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcTopologyResource/Entities/IfcAdvancedFace.md)).
- Surfaces: Plane, Cylindrical, Spherical, Toroidal, SurfaceOfLinearExtrusion, SurfaceOfRevolution, [IfcBSplineSurfaceWithKnots](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcBSplineSurfaceWithKnots.htm), [IfcRationalBSplineSurfaceWithKnots](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcRationalBSplineSurfaceWithKnots.htm), RectangularTrimmedSurface, CurveBoundedPlane/Surface.
- Surface models: ShellBasedSurfaceModel and FaceBasedSurfaceModel.

**Tessellated**
- [IfcTriangulatedFaceSet](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcTriangulatedFaceSet.htm). Note the `PnIndex` indirection; IfcOpenShell once ignored it ([#3434](https://github.com/IfcOpenShell/IfcOpenShell/issues/3434)).
- [IfcPolygonalFaceSet](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcPolygonalFaceSet.htm) with IndexedPolygonalFace, with or without voids.
- [IfcTriangulatedIrregularNetwork](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcTriangulatedIrregularNetwork.htm) has per-triangle flags: −2 = void, −1 = hole, 1–7 = breakline bits ([src L16-17](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcTriangulatedIrregularNetwork.md?plain=1#L16-L17)).

**Curves**
- [IfcIndexedPolyCurve](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcIndexedPolyCurve.htm) ([src L9, L29](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometryResource/Entities/IfcIndexedPolyCurve.md?plain=1#L9-L29)):
  - Segments are IfcLineIndex runs or IfcArcIndex three-point arcs.
  - An arc whose three points are collinear *within the context Precision* must be treated as a polyline. **Tolerance is therefore part of the semantics.**
- Others: CompositeCurve, TrimmedCurve, Polyline, Circle, Ellipse, (Rational)BSplineCurveWithKnots, OffsetCurve2D/3D, [IfcOffsetCurveByDistances](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcOffsetCurveByDistances.htm), [IfcPolynomialCurve](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcPolynomialCurve.htm).

**Alignment (new in 4.3)**
- [IfcCurveSegment](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcCurveSegment.htm): a Placement plus SegmentStart/SegmentLength that trim a ParentCurve. The sign of the length gives the sense ([src](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometryResource/Entities/IfcCurveSegment.md)).
- Parent curves:
  - [IfcClothoid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcClothoid.htm): curvature varies linearly with arc length.
  - [IfcCosineSpiral](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcCosineSpiral.htm): κ(s) = 1/A₀ + (1/A₁)·cos(πs/L) ([src](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometryResource/Entities/IfcCosineSpiral.md)).
  - [IfcSineSpiral](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSineSpiral.htm), [IfcSecondOrderPolynomialSpiral](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSecondOrderPolynomialSpiral.htm), and the Third- and SeventhOrder polynomial spirals.
- [IfcGradientCurve](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcGradientCurve.htm): a 2D BaseCurve plus vertical segments. Its parameter equals the BaseCurve parameter ([src](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometryResource/Entities/IfcGradientCurve.md)).
- [IfcSegmentedReferenceCurve](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSegmentedReferenceCurve.htm): cant and superelevation expressed in the base curve's parameter space ([src](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometryResource/Entities/IfcSegmentedReferenceCurve.md)).
- Placements: [IfcAxis2PlacementLinear](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcAxis2PlacementLinear.htm), IfcPointByDistanceExpression, IfcLinearPlacement.
- Spirals have no closed form. IfcOpenShell integrates the unit tangent with 15-point Gauss–Kronrod quadrature ([IfcCurveSegment.cpp L54-76](https://github.com/IfcOpenShell/IfcOpenShell/blob/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/mapping/IfcCurveSegment.cpp#L54-L76)).
  - (our analysis) That makes deterministic sin/cos part of the alignment problem.

**Profiles** ([IfcProfileResource](https://github.com/buildingSMART/IFC4.3.x-development/tree/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcProfileResource/Entities))
- Arbitrary and composite: [IfcArbitraryClosedProfileDef](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcArbitraryClosedProfileDef.htm), ArbitraryProfileDefWithVoids, ArbitraryOpenProfileDef, CenterLineProfileDef, CompositeProfileDef, DerivedProfileDef, MirroredProfileDef, OpenCrossProfileDef.
- Parameterised:
  - Rectangle, plus Hollow and Rounded variants.
  - Circle and CircleHollow.
  - Ellipse.
  - Section shapes: I, AsymmetricI, L, T, U, C, Z, Trapezium.

**Instancing and transforms**
- [IfcMappedItem](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcMappedItem.htm) with IfcRepresentationMap.
- [IfcCartesianTransformationOperator3DnonUniform](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcCartesianTransformationOperator3DnonUniform.htm): Scale2 and Scale3 default to Scale ([src L10](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometryResource/Entities/IfcCartesianTransformationOperator3DnonUniform.md?plain=1#L10)).
  - In IfcOpenShell this goes through OCCT's general transform. Their code comment says that path results "in a conversion to b-spline surfaces for about everything" ([base_utils.cpp L323-337](https://github.com/IfcOpenShell/IfcOpenShell/blob/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/kernels/opencascade/base_utils.cpp#L296-L345)).
  - (our analysis) We need elliptic cylinders and cones, or an exact NURBS fallback. Instances with non-uniform scale cannot be handled as rigid instances.

**Implication for round-trip** (our analysis): store the IFC item tree as the source of truth and treat the evaluated B-rep or mesh as a cache. That is what IfcOpenShell's taxonomy layer does (A3).

---

## A2. ISO 10303-42 / AP242

**Editions**
- ISO 10303-42:2025 is Edition 9 per search. iso.org blocked automated fetch, so this is *unverified* ([ISO 91386](https://www.iso.org/standard/91386.html)). The 2024 edition shows as withdrawn ([ISO 89538](https://www.iso.org/standard/89538.html)).
- AP242: ISO 10303-242:2025 is Edition 4, August 2025 ([ISO 84300](https://www.iso.org/standard/84300.html)). STEP Tools reports Ed. 4 changes no entity attributes ([notes](https://steptools.com/docs/stp_aim/notes_ap242e4.html)).

**Entity inventory** (from the [STEP Tools Part 42:2021 index](https://www.steptools.com/stds/smrl/data/resource_docs/geometric_and_topological_representation/sys/contents.htm))
- *geometry_schema*:
  - Curves: line, conics, clothoid, circular_involute, polyline, b-spline curves (uniform, quasi-uniform, bezier, rational, locally refined), trimmed/composite/reparametrised curves, pcurve, surface_curve, intersection_curve, seam_curve, offset curves.
  - Surfaces: elementary surfaces including degenerate torus and Dupin cyclide; extrusion, revolution, surface_curve_swept and fixed_reference_swept surfaces; b-spline surfaces; rectangular_trimmed, curve_bounded and rectangular_composite surfaces; offset_surface.
  - Also b-spline volumes.
- *topology_schema*:
  - Vertex and edges: vertex_point, edge_curve, oriented_edge, seam_edge, subedge.
  - Paths and loops: path, edge_loop, vertex_loop, poly_loop.
  - Faces: face_bound/face_outer_bound, face_surface, oriented_face, subface.
  - Shells: connected_face_set, open/closed shells (plain and oriented).
  - Helper functions: edge_reversed, list_face_loops and others.
- *geometric_model_schema*:
  - Solids: manifold_solid_brep, brep_with_voids, faceted_brep, csg_solid/boolean_result, primitives, half_space_solid and boxed_half_space.
  - Swept solids: extruded and revolved area/face solids, swept_disk_solid.
  - Surface models, sectioned_spine.
  - Tessellated items: triangulated_face, complex_triangulated_face, cubic_bezier_triangulated_face, tessellated_solid/shell.
- *scan_data_3d_shape_model_schema*: point_cloud_dataset (with normals, colours or intensities) and triangulated_point_cloud_dataset.

**What this means for the kernel** (our analysis; facts linked)
1. Carrier geometry (unbounded or parametric curves and surfaces) is kept separate from bounding topology.
2. Each edge_curve is used by two oriented_edges with a same-sense flag, so we need a coedge/half-edge structure.
3. A face_surface has its own same-sense flag, so face normal and surface normal may disagree.
4. Faces can have several loops, including vertex_loop for apexes and poles.
5. Periodic surfaces need seam edges, and every face-use needs its pcurve. IFC inherits this: IfcAdvancedBrep is adapted from ISO 10303-514 ([src L14](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcAdvancedBrep.md)), so one B-rep importer can serve both STEP and IFC.
6. Imported "tolerant" B-reps (OCCT- or Parasolid-class) need per-vertex and per-edge tolerances. OCCT repairs gaps by *enlarging* vertex and edge tolerances ([shape_healing.md L404-477](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/shape_healing/shape_healing.md?plain=1#L404-L477)). We must import that and keep it auditable.

---

## A3. How IfcOpenShell and web-ifc do it, and what goes wrong

### IfcOpenShell (LGPL-3.0: method study only)

**Pipeline**
1. One mapping file per IFC entity ([mapping/](https://github.com/IfcOpenShell/IfcOpenShell/tree/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/mapping)) converts IFC into a backend-neutral *taxonomy* IR of about 30 kinds ([taxonomy.h L122-155](https://github.com/IfcOpenShell/IfcOpenShell/blob/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/taxonomy.h#L122-L155)):
   - Primitives: point, direction, matrix.
   - Curves and surfaces, including bspline_surface.
   - Topology: edge, loop, face, shell, solid.
   - Constructions: loft, extrusion, revolve, sweep_along_curve, and boolean_result with union, subtraction and intersection ([L1338-1358](https://github.com/IfcOpenShell/IfcOpenShell/blob/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/taxonomy.h#L1338-L1358)).
   - Alignment: piecewise, gradient, cant and offset functions.
2. Each backend implements `convert_impl` per kind; unimplemented kinds throw `not_implemented`. Backends also get hooks for `apply_layerset`, `convert_openings` and `unify_shapes` ([abstract_kernel.h L79-145](https://github.com/IfcOpenShell/IfcOpenShell/blob/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/abstract_kernel.h#L79-L145)).

**Backends**
- **OCCT**: the complete backend.
- **CGAL**: uses `Epick` in simple mode, and `Epeck` (exact constructions) with `Nef_polyhedron_3` when booleans are needed ([cgal_conversion_result.h L62-68](https://github.com/IfcOpenShell/IfcOpenShell/blob/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/kernels/cgal/cgal_conversion_result.h#L62-L68)).
- **Manifold**: added 2026-04-07 in commit [a19d398](https://github.com/IfcOpenShell/IfcOpenShell/commit/a19d398c7878ada59e6f3e020fa946b5227599a1), titled "Vibe code an implementation that uses manifold". It handles only extrusion, shell, solid, boolean_result and openings ([manifold_kernel.h](https://github.com/IfcOpenShell/IfcOpenShell/blob/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/kernels/manifold/manifold_kernel.h)).
- **passthrough**: no geometry processing.
- **`hybrid_kernel`**: tries backends in order, swallows exceptions and returns the first success. It skips non-boolean backends when an element has openings ([hybrid_kernel.h L30-75](https://github.com/IfcOpenShell/IfcOpenShell/blob/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/hybrid_kernel.h#L30-L75)).

**Tolerance strategy**
- OCCT booleans ([boolean_utils.cpp L835-1360](https://github.com/IfcOpenShell/IfcOpenShell/blob/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/kernels/opencascade/boolean_utils.cpp#L835-L1360)):
  - Fuzz starts at precision/100.
  - Operands are pre-unified.
  - Cutters that miss the bounding box, only touch, or are too narrow are dropped.
  - Coplanar cuts fall back to a 2D boolean.
  - Bad results go through ShapeFix.
  - The operation is retried with fuzz ×10, up to precision×10⁴, whenever output edges come close to the fuzz value.
- Manifold has no tolerance at all. IfcOpenShell added "Dilation of 2nd operands as a poor mans fuzziness" in commit [2d5883f](https://github.com/IfcOpenShell/IfcOpenShell/commit/2d5883f9668f4f48484c3798a1c19410ce297272): cutters are pushed out along vertex normals ([manifold_kernel.cpp L111-173](https://github.com/IfcOpenShell/IfcOpenShell/blob/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/kernels/manifold/manifold_kernel.cpp#L111-L173)). Openings are cut by BatchBoolean-union of the cutters followed by one subtraction ([L1412-1456](https://github.com/IfcOpenShell/IfcOpenShell/blob/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/kernels/manifold/manifold_kernel.cpp#L1412-L1456), [L1643](https://github.com/IfcOpenShell/IfcOpenShell/blob/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/kernels/manifold/manifold_kernel.cpp#L1643)).

**Tests**: the C++ geometry library has only 27 unit test cases across three files ([src/ifcgeom/tests](https://github.com/IfcOpenShell/IfcOpenShell/tree/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/tests)). Regressions are found by users.

**Failure clusters from the issue tracker** (open unless noted)
- *Openings and half-spaces*:
  - Missing wall subtractions: [#616](https://github.com/IfcOpenShell/IfcOpenShell/issues/616), [#619](https://github.com/IfcOpenShell/IfcOpenShell/issues/619), [#620](https://github.com/IfcOpenShell/IfcOpenShell/issues/620), [#2072](https://github.com/IfcOpenShell/IfcOpenShell/issues/2072).
  - Manifold half-space clipping is wrong unless the normal points down: [#9767](https://github.com/IfcOpenShell/IfcOpenShell/issues/9767).
  - Cutting an opening from a swept disk trims the directrix instead of subtracting: [#9256](https://github.com/IfcOpenShell/IfcOpenShell/issues/9256).
- *Rebar and swept disks*:
  - Out of memory on rebar: [#1005](https://github.com/IfcOpenShell/IfcOpenShell/issues/1005).
  - CGAL skips IfcArcIndex directrices: [#8106](https://github.com/IfcOpenShell/IfcOpenShell/issues/8106).
  - Inside-out start cap, so volume depends on directrix direction (0.99 vs 0.33): [#9571](https://github.com/IfcOpenShell/IfcOpenShell/issues/9571).
  - Manifold crash on swept disks: [#9762](https://github.com/IfcOpenShell/IfcOpenShell/issues/9762).
- *Advanced B-rep*: an IfcAdvancedBrep box with a cylindrical hole comes out +12.9% in volume, while "web-ifc and OCCT read the same file exactly": [#9570](https://github.com/IfcOpenShell/IfcOpenShell/issues/9570). This is a textbook differential-test catch.
- *Infrastructure*:
  - Section orientation ignored: [#6386](https://github.com/IfcOpenShell/IfcOpenShell/issues/6386).
  - End caps doubled, a regression: [#9820](https://github.com/IfcOpenShell/IfcOpenShell/issues/9820) (closed).
  - IfcSectionedSurface wrong with IfcAxis2PlacementLinear: [#8116](https://github.com/IfcOpenShell/IfcOpenShell/issues/8116).
  - IfcRevolvedAreaSolidTapered not implemented: [#3540](https://github.com/IfcOpenShell/IfcOpenShell/issues/3540).
- *Tessellation*:
  - Surface-curve sweeps produce non-manifold output: [#1541](https://github.com/IfcOpenShell/IfcOpenShell/issues/1541).
  - Vertex-weld threshold after openings: [#7176](https://github.com/IfcOpenShell/IfcOpenShell/issues/7176).
  - Volumes of non-manifold tessellations: [#6125](https://github.com/IfcOpenShell/IfcOpenShell/issues/6125).
  - Manifold's default `circle-segments=0` gives one segment per arc, and reversed arcs come out wrong: [#9768](https://github.com/IfcOpenShell/IfcOpenShell/issues/9768).
- *MEP*: pipe elbows rendered as doughnuts ([#5766](https://github.com/IfcOpenShell/IfcOpenShell/issues/5766), closed); pipe bends not loading ([#9511](https://github.com/IfcOpenShell/IfcOpenShell/issues/9511)).

### web-ifc (MPL-2.0)

**Approach**
- Everything is triangles from the start. Curved geometry gets a *fixed* `_circleSegments = 12` instead of a deviation bound ([IfcGeometryProcessor.h L26-36](https://github.com/ThatOpen/engine_web-ifc/blob/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/src/cpp/web-ifc/geometry/IfcGeometryProcessor.h#L26-L36)).
- NURBS are evaluated with tinynurbs, and faces are triangulated with the CDT library ([nurbs.cpp L6-7](https://github.com/ThatOpen/engine_web-ifc/blob/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/src/cpp/web-ifc/geometry/nurbs.cpp#L6-L7)).
- Booleans are mesh "fuzzy-bools" using a BVH and shared-position normalisation ([fuzzy-bools.h](https://github.com/ThatOpen/engine_web-ifc/blob/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/src/cpp/web-ifc/geometry/operations/boolean-utils/fuzzy-bools.h)).
- Tolerances are absolute (1e-4 for plane intersection and deviation). The code itself says these constants "needs to be rationalised and eliminated!" ([eps.h L14-26](https://github.com/ThatOpen/engine_web-ifc/blob/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/src/cpp/web-ifc/geometry/operations/boolean-utils/eps.h#L14-L26)).

**Failures**
- [#2211](https://github.com/ThatOpen/engine_web-ifc/issues/2211): a coarse plane tolerance makes CDT throw on beams cut by IfcPolygonalBoundedHalfSpace. A per-plane "recovery" then produced a degenerate ~108k-vertex mesh and a 3.4× slowdown, bisected to commit `e908c2f`.
- [#2269](https://github.com/ThatOpen/engine_web-ifc/issues/2269): stray triangles across wall openings.
- [#2097](https://github.com/ThatOpen/engine_web-ifc/issues/2097): quadratic face matching when shells are rebuilt from triangles stalls waffle slabs (2,581 IfcIndexedPolygonalFaceWithVoids).
- [#535](https://github.com/ThatOpen/engine_web-ifc/issues/535): endless loop on an IfcRoof.
- [#1999](https://github.com/ThatOpen/engine_web-ifc/issues/1999): WASM out-of-memory on large MEP models.

**Tests**: per-feature functional specs, for example `BooleanRobustness.spec.js` with `wall-openings-1170/2374.ifc` ([tests/functional](https://github.com/ThatOpen/engine_web-ifc/tree/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional)).

**Lessons for Spare Kit**
1. Keep a recipe IR separate from evaluation.
2. Tessellate to a deviation bound tied to context precision, never to a segment count.
3. Have one documented tolerance model relative to model scale. Avoid retry loops whose result depends on retry order.
4. A failed cut must return a typed diagnostic, never a silent "uncut" or garbage mesh.
5. Rebar, MEP sweeps, half-space cuts and alignment sweeps are where the bugs are, so seed the corpus there.

---

## B. Revit-class geometric coverage (functional description, own words)

**Walls**
- A wall is a path curve (line, arc, ellipse or spline) carrying a stack of compound layers.
- Layer *function priority* decides which layer runs through at a junction, and same-material layers merge ([Autodesk "Layer Join Cleanup"](https://help.autodesk.com/cloudhelp/2014/ENU/Revit/files/GUID-798DE946-C683-4ACD-BA7C-040BF722F033.htm), [wall functions tutorial](https://www.autodesk.com/learn/ondemand/tutorial/wall-functions)).
- Kernel needs (our analysis):
  - Exact offset of line and arc paths per layer.
  - Plan-view 2D booleans to resolve L, T, X, miter and butt joins per layer band.
  - Extrusion to constraints.
  - Attach top/base: trim against a non-planar roof or floor face.
  - Sketch-profile edits and wrap-at-insert.
- In IFC: IfcMaterialLayerSetUsage. IfcOpenShell exposes `apply_layerset` ([abstract_kernel.h L139-140](https://github.com/IfcOpenShell/IfcOpenShell/blob/741d060e4df07e58fcde5cdb7e9d173e5656fb3b/src/ifcgeom/abstract_kernel.h#L139-L140)).

**Openings and voids**: many hosted cutters per host, so cuts should be batched (union all cutters, then one difference), as IfcOpenShell's Manifold path does (A3). Each host should cache its uncut solid.

**Floors and roofs**
- Sketch-boundary slabs, slope arrows, and *shape editing*: points, split lines, and a variable-thickness layer that keeps the bottom planar ([Autodesk 2026 Shape Editing](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-HaveYouTried/files/GUID-A4C780C3-C1B7-4540-94AC-85C6F0481A26.htm), [variable layer](https://help.autodesk.com/cloudhelp/2014/ENU/Revit/files/GUID-48EB9CC8-BF75-435E-AC99-D34726490AFF.htm)).
- (our analysis) The kernel needs a constrained-Delaunay piecewise-planar top surface over sub-element points, layer offsets, and footprint roofs with per-edge slopes (straight-skeleton hips and valleys).

**Stairs and railings**: runs and landings, plus profile sweeps along 3D paths with mitred breaks and balusters spaced along the path. These map to IfcSweptDiskSolid and IfcSurfaceCurveSweptAreaSolid.

**Conceptual massing**
- Forms: extrude, revolve, sweep, swept blend, and loft through profiles; edit by pushing points, edges or faces.
- *Divide surface* creates UV grids, or divides by intersecting levels, planes or curves ([Autodesk help](https://help.autodesk.com/cloudhelp/2023/ENU/Revit-ArchDesign/files/GUID-0471D7EF-3720-4D58-9036-C99C64602EE7.htm)). Pattern or adaptive-point panels are then placed on grid nodes ([workflow](https://help.autodesk.com/cloudhelp/2016/ENU/Revit-Model/files/GUID-39B460C2-4F23-4124-9289-3F82C3DE550A.htm)).
- Needs: NURBS skinning, robust surface parameterisation, curves on surfaces, panel planarity metrics.

**Toposolids and terrain**
- Constrained Delaunay TIN from survey points, contours and breaklines.
- A solid down to a datum, with sub-divisions and excavation.
- Cut/fill as a volume between two TINs.
- In IFC: TIN flags (A1), alignment, and IfcSectionedSolidHorizontal corridors.

**Curtain walls**: grids on planar walls or on mass faces ("by face"); mullions are profile sweeps along grid curves on curved faces. Users report multi-face and diagonal grids as hard ([McNeel thread](https://discourse.mcneel.com/t/rhino-inside-revit-custom-curtain-grid-on-curtain-system-by-face/222271)).

**MEP**: round, rectangular and oval sweeps along routes, plus fittings (elbows as revolved/torus segments, tees, transitions as lofts). Fittings are fragile in IFC tools ([#5766](https://github.com/IfcOpenShell/IfcOpenShell/issues/5766)).

**Structure**
- Beam joins, cutbacks and copes use half-spaces and polygonal-bounded half-spaces, which break CDT in web-ifc ([#2211](https://github.com/ThatOpen/engine_web-ifc/issues/2211)).
- Analytical model: curves and surfaces offset from the physical model.

**Rebar**: a centreline polyline with bend radius becomes line/arc segments swept by a disk; very large counts need instancing (issues [#1005](https://github.com/IfcOpenShell/IfcOpenShell/issues/1005), [#8106](https://github.com/IfcOpenShell/IfcOpenShell/issues/8106), [#9571](https://github.com/IfcOpenShell/IfcOpenShell/issues/9571)).

**Point clouds**: reference only. Revit links indexed ReCap `.rcp`/`.rcs` files ([Autodesk](https://help.autodesk.com/cloudhelp/2016/ENU/Revit-Model/files/GUID-D54DBCA3-DD96-49A2-B61E-48033EA32FCF.htm)); Part 42 has `point_cloud_dataset` (A2). Needs (our analysis): out-of-core octree, snapping, section clipping. No B-rep.

**AutoCAD-class 2D**
- Polylines with bulge arcs (DXF LWPOLYLINE group code 42, [Autodesk DXF](https://help.autodesk.com/cloudhelp/2020/ENU/AutoCAD-DXF/files/GUID-748FC305-F3F2-4F74-825A-61F04D757A50.htm)).
- Exact offset with arcs, trim/extend/fillet/chamfer, region booleans, hatch boundary and island detection, splines (fit points and control points), elliptical arcs, measurements on exact geometry, linetypes along curves, and section and hidden-line projection from 3D.
- Candidate crates:
  - [cavalier_contours@22887a8](https://github.com/jbuckmccready/cavalier_contours/tree/22887a824d22cd8ab50968a551f34cdb4508127f) (MIT OR Apache-2.0): offsets and booleans with true arcs.
  - [iOverlay@117637e](https://github.com/iShape-Rust/iOverlay/tree/117637e3a65804973da2bfd7c59f42773f4dcc0d) (MIT OR Apache-2.0): i16/i32/i64 integer APIs plus float, all fill rules, buffering.
  - Clipper2 (BSL-1.0, C++).

**Public Revit pain points** (these define what "iron-clad" must beat)
- *Short curves*: curve creation refuses anything shorter than `Application.ShortCurveTolerance` ([Revit API help](https://help.autodesk.com/cloudhelp/2018/CSY/Revit-API/files/GUID-A1BCB8D4-5628-45AF-B399-AF573CBBB1D0.htm), [forum](https://forums.autodesk.com/t5/revit-api-forum/why-shortcurvetolerance/m-p/10263926)). Users measure it at about 1/32″ ≈ 0.8 mm ([forum](https://forums.autodesk.com/t5/revit-api-forum/line-is-too-short/m-p/5645855), [AUGI](https://forums.augi.com/archive/index.php/t-117254.html)); the exact value is *unverified*.
- *"Can't create form element"* on lofts ([forum](https://forums.autodesk.com/t5/revit-architecture-forum/issue-creating-surface-for-mass/m-p/11429266/highlight/true), [forum](https://forums.autodesk.com/t5/revit-architecture-forum/loft-massing-issues-in-revit/m-p/10594093/highlight/true)).
- *Join failures*: "Can't keep wall and floor joined" ([forum](https://forums.autodesk.com/t5/revit-architecture-forum/i-keep-on-getting-quot-can-t-keep-wall-and-floor-joined-quot/m-p/8509558/highlight/true)), "…wall and target joined" ([Autodesk support](https://autodesk.com/support/technical/article/caas/sfdcarticles/sfdcarticles/PTB/Can-t-keep-wall-and-target-joined-when-trying-to-attach-a-wall-to-a-roof-in-Revit.html)), walls that won't join ([support](https://autodesk.com/support/technical/article/caas/sfdcarticles/sfdcarticles/Two-walls-just-will-not-join-in-Revit.html)).
- *Void cuts*: "Can't cut instance out of its host" ([Autodesk troubleshooting](https://help.autodesk.com/cloudhelp/2019/ENU/Revit-Troubleshooting/files/GUID-3C9C035F-BFA8-44A5-923A-C4E75F5D5E1C.htm), [forum](https://forums.autodesk.com/t5/revit-architecture-forum/void-error-can-t-cut-out-of-its-host/td-p/10916646)).
- *Shape edit*: "Slab Shape Edit failed… no longer horizontal" ([AUGI](https://forums.augi.com/archive/index.php/t-170698.html)); "too thin" ([Arkance](https://emea.support.arkance.world/hc/cs/articles/36466923622034-Slab-Editing-Problem-Revit)).
- *Toposolids*:
  - Sub-divide fails intermittently on complex shapes ([forum](https://forums.autodesk.com/t5/revit-architecture-forum/revit-lt-2024-toposolid-sub-divide-function/m-p/11937534)).
  - Cut/fill ignores added volumes ([forum](https://forums.autodesk.com/t5/revit-architecture-forum/fill-in-toposolid/m-p/11932263)).
  - Simplify changes volumes ([Arkance](https://ukcommunity.arkance.world/hc/en-us/articles/21566478179986-Revit-2024-1-Simplify-Toposolid-Function)).
- Design response (our analysis):
  - Tolerances relative to context precision and visible to the user.
  - Every refusal names the offending sub-entity and gives a reason.
  - Partial results are allowed but explicitly marked invalid.

---

## C. Meshes, LOD, spatial index, clash

**Watertight tessellation from B-rep: use the OCCT pattern**
- OCCT meshes in two passes: discretise every *edge* once (linear plus angular deflection), then triangulate each face against those shared edge polylines ([mesh.md L74-78](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/mesh/mesh.md?plain=1#L74-L78)).
- Its pipeline includes a "heal discrete model" step for self-intersections that coarse edge sampling introduces ([L112-115](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/mesh/mesh.md?plain=1#L112-L115)).
- Recommendation:
  - Shared edge sampling with deterministic ordering.
  - Faces triangulated by CDT in (u,v) with [spade](https://github.com/Stoeoef/spade/tree/c8befc96bbbc1898a89cb19f9f3104a848936374) (MIT OR Apache-2.0).
  - Record the achieved maximum deviation per face, so the mesh is watertight by construction.
  - Reject fixed segment counts: they are behind web-ifc's 12-segment default and IfcOpenShell [#9768](https://github.com/IfcOpenShell/IfcOpenShell/issues/9768).

**LOD**
- [meshoptimizer](https://github.com/zeux/meshoptimizer/tree/f0a684f11ead064e176775c9b796f82a55910e92) (MIT) provides:
  - `meshopt_simplify` and `simplifyWithAttributes`.
  - A permissive mode.
  - `meshopt_SimplifyLockBorder`, which allows crack-free independent LODs per subset ([README L544-660](https://github.com/zeux/meshoptimizer/blob/f0a684f11ead064e176775c9b796f82a55910e92/README.md?plain=1#L544-L660)).
  - Meshlets, quantisation and compression.
- Rust wrapper: the `meshopt` crate (MIT OR Apache-2.0, [gwihlidal/meshopt-rs](https://github.com/gwihlidal/meshopt-rs)).
- It is C++, so bit-identical native/WASM output is *unverified*. Either compile it to wasm32 and test byte equality, or port the simplifier.

**Spatial index**
- [parry](https://github.com/dimforge/parry/tree/b3c3584ce9be9bf6edb821fd7548b817087131ff) (Apache-2.0) has:
  - A BVH with binned and PLOC builds, refit and validation ([src/partitioning/bvh](https://github.com/dimforge/parry/tree/b3c3584ce9be9bf6edb821fd7548b817087131ff/src/partitioning/bvh)).
  - GJK and EPA ([gjk](https://github.com/dimforge/parry/tree/b3c3584ce9be9bf6edb821fd7548b817087131ff/src/query/gjk), [epa](https://github.com/dimforge/parry/tree/b3c3584ce9be9bf6edb821fd7548b817087131ff/src/query/epa)).
  - Composite-shape distance ([distance_composite_shape_shape.rs](https://github.com/dimforge/parry/blob/b3c3584ce9be9bf6edb821fd7548b817087131ff/src/query/distance/distance_composite_shape_shape.rs)).
  - Trimesh intersection ([mesh_intersection](https://github.com/dimforge/parry/tree/b3c3584ce9be9bf6edb821fd7548b817087131ff/src/transformation/mesh_intersection)) and V-HACD.
  - An `enhanced-determinism` feature that forces libm, IndexMap and scalar maths ([Cargo.toml L48](https://github.com/dimforge/parry/blob/b3c3584ce9be9bf6edb821fd7548b817087131ff/crates/parry3d-f64/Cargo.toml#L48)).
- [rstar](https://github.com/georust/rstar/tree/3b1aee6087f36937603b6ccc33240d664c314a19) (MIT OR Apache-2.0): a dynamic R*-tree for 2D drafting selection and snapping.

**Clash** (our analysis)
1. *Broad phase*: a two-level BVH, with instance AABBs on top and per-definition mesh BVHs below, mirroring IfcMappedItem. Instances are transformed, not copied.
2. *Hard clash*: exact triangle–triangle tests with robust `orient3d` ([Shewchuk](https://www.cs.cmu.edu/~quake/robust.html); Rust port [georust/robust](https://github.com/georust/robust/tree/654f34cb8cdb3ae21bf59ef3472f92652942cd74), MIT OR Apache-2.0).
3. *Clearance / true distance*: BVH-pruned minimum distance between meshes. Report it as the interval [d − δa − δb, d], where δ is each mesh's recorded tessellation deviation, and refine pairs near the threshold with B-rep closest-point Newton iteration.
4. GJK/EPA applies only to convex pieces. EPA's penetration depth is meaningless for non-convex BIM parts unless they are decomposed first.

**Mesh repair for imported meshes**
- Steps (our analysis): weld vertices at a tolerance derived from context precision (IfcOpenShell [#7176](https://github.com/IfcOpenShell/IfcOpenShell/issues/7176)), make orientation consistent across shared edges, then classify inside/outside with generalized winding numbers ([Jacobson et al.](https://igl.ethz.ch/projects/winding-number/), [fast variant](https://www.dgp.toronto.edu/projects/fast-winding-numbers/)).
- Manifold's `Merge` fixes slightly non-manifold input and otherwise returns an error status ([README L68](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/README.md?plain=1#L68)).
- Licences: MeshFix is GPL-3.0, so method only. libigl core is MPL-2.0, but its copyleft modules are GPL. Cherchi's [InteractiveAndRobustMeshBooleans](https://github.com/gcherchi/InteractiveAndRobustMeshBooleans) is MIT, but its [indirect predicates](https://github.com/MarcoAttene/Indirect_Predicates) dependency is LGPL-2.1, so method only.

**Mesh to B-rep**: merge coplanar triangles into planar polygon faces to get a faceted B-rep. Key the planes by hash; web-ifc's quadratic matching ([#2097](https://github.com/ThatOpen/engine_web-ifc/issues/2097)) shows why. Keep TINs as TINs. Don't convert scans.

---

## D. How the best kernels are tested

**OCCT** (LGPL-2.1)
- Two runners: the DRAW Tcl regression suite under `tests/` and GoogleTest unit tests ([tests.md L12-17](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/contribution/tests/tests.md?plain=1#L12-L17)). There are about 18.3k files under `tests/` and 612 GTest `.cxx` files.
- A typical case loads reference shapes, cuts, then asserts area with `checkprops result -s 272.503` plus an image ([bcut_complex/A1](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/tests/boolean/bcut_complex/A1)).
- Every grid's `end` script runs `checkshape` automatically ([tests/boolean/end](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/tests/boolean/end)).
- Logs are scanned with regex rules ([parse.rules](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/tests/parse.rules)).
- Known bugs are encoded as TODO lines, giving three statuses: BAD (known failure), IMPROVEMENT (a TODO stopped matching) and FAILED ([L636-640](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/contribution/tests/tests.md?plain=1#L636-L640)).
- `testdiff` compares CPU, memory and images between runs ([L756-781](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/contribution/tests/tests.md?plain=1#L756-L781)); `checkmaxtol` guards tolerance growth ([L1148-1160](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/contribution/tests/tests.md?plain=1#L1148-L1160)).
- Validity oracles: `checkshape` ([L7025](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/draw_test_harness/draw_test_harness.md?plain=1#L7025)), and `bopargcheck`, which checks small edges, self-interference, tangent faces, mergeable vertices and edges, and curve-on-surface consistency ([L8090-8120](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/draw_test_harness/draw_test_harness.md?plain=1#L8090-L8120)).

**Manifold** (Apache-2.0)
- Google FuzzTest harnesses: random CSG trees of transformed cubes must return `Status()==NoError` ([manifold_fuzz.cpp L39-97](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/test/manifold_fuzz.cpp#L39-L97)), and polygon triangulation must not crash ([polygon_fuzz.cpp L74-100](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/test/polygon_fuzz.cpp#L74-L100)).
- A polygon corpus and named degenerate OBJ fixtures ([test/models](https://github.com/elalish/manifold/tree/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/test/models)).
- Its correctness contract is *ε-validity*: manifold output is guaranteed, ties are broken by symbolic perturbation with no tolerance, and any counterexample is a bug ([wiki](https://github.com/elalish/manifold/wiki/Manifold-Library)).

**Parasolid / ACIS / CGAL**
- Siemens claims "millions of overnight auto-tests", zero tolerance for regressions, and that customer-reported issues are added to the suite ([blog](https://blogs.sw.siemens.com/plm-components/parasolid-delivers-30-years-of-quality-in-industrial-use/)). This is marketing and *unverified*.
- Nothing public was found for ACIS (*unverified*).
- CGAL publishes near-daily testsuite results; the latest build was 6.3-Ic-108 on 2026-10-21 ([CGAL testsuite](https://cgal.geometryfactory.com/CGAL/testsuite/)).
- Benchmark for discipline: SQLite has a 590× test-to-source ratio, 100% branch coverage and 100% MC/DC, and fuzzes continuously ([sqlite.org/testing](https://www.sqlite.org/testing.html)).

**Tools and their limits**
- **Kani** (MIT OR Apache-2.0): bit-precise and supports floats. `sqrt`, `sin` and `cos` are over-approximated, so spurious failures are possible ([feature support](https://model-checking.github.io/kani/rust-feature-support.html)). Practical for bounded proofs: no panic or overflow, predicate sign equals the exact i128/expansion reference, index-structure invariants on small topologies.
- **Verus** (MIT): `vstd` float support is only bit-level classification ([float.rs](https://github.com/verus-lang/verus/blob/f76531736f0905536d7013f6b58432dcaa4ddadf/source/vstd/float.rs)). Use it for integer and topology invariants (half-edge twin/next consistency under Euler operators), not for float maths.
- **Prusti** (MPL-2.0): describes itself as a "prototype".
- **Creusot**: its contracts crate `creusot-std` is **LGPL-2.1-or-later**, which conflicts with our policy as a dependency ([creusot@5a64947](https://github.com/creusot-rs/creusot/tree/5a64947970337d060a1a497d1aa562b1294477e7)).
- **bolero** (MIT): one harness can run as a property test, a fuzzer, or under Kani (`cfg(kani)`) ([bolero@7642943](https://github.com/camshaft/bolero/tree/7642943b6c1f8a9b5e824dd1cd29b7c8e56068b8)).
- **cargo-mutants** (MIT): `--in-diff` mutates only the PR diff, and `--shard k/n` spreads runs across machines ([in-diff](https://github.com/sourcefrog/cargo-mutants/blob/6c720ae82a1784b84cfa06066f7b0d59ed1798d0/book/src/in-diff.md), [shards](https://github.com/sourcefrog/cargo-mutants/blob/6c720ae82a1784b84cfa06066f7b0d59ed1798d0/book/src/shards.md)).
- **Determinism**: in WebAssembly, the sign of a NaN result is non-deterministic; the spec defines a deterministic profile ([spec §NaN propagation](https://webassembly.github.io/spec/core/exec/numerics.html#nan-propagation)). Use pure-Rust [libm](https://github.com/rust-lang/compiler-builtins/tree/master/libm) (MIT) rather than platform libm.

**External corpora and their licences**
- buildingSMART [Sample-Test-Files@80d976a](https://github.com/buildingSMART/Sample-Test-Files/tree/80d976a9b193a26a8e928c3e79bff67af1de68a8) (CC BY 4.0): can be vendored.
- [IFC4.3.x-sample-models@ef22440](https://github.com/buildingSMART/IFC4.3.x-sample-models/tree/ef22440ec501551ccc9689309560bbc7e26aa9d7): folders include advanced-geometric-shape, alignment and tessellated. Final licence is CC BY-ND, so use files unmodified only.
- [IFC4.x-IF Alignment-atomic-testset@75c7c7a](https://github.com/buildingSMART/IFC4.x-IF/tree/75c7c7aa6c523e76a01cb4cf9542f896956fb92d/IFC-files/Alignment-atomic-testset): no licence, so fetch at test time and never vendor.
- NIST MBE PMI STEP test cases ([NIST](https://www.nist.gov/ctl/smart-connected-systems-division/smart-connected-manufacturing-systems-group/mbe-pmi-validation), [data.gov](https://catalog.data.gov/dataset/nist-cad-models-and-step-files-with-pmi)) and the [NIST SFA](https://github.com/usnistgov/SFA) checker. SFA's licence terms are *unverified*; the repo only links the NIST disclaimer.
- [ABC dataset](https://deep-geometry.github.io/abc-dataset/): about 1M STEP/Parasolid models, copyright held by their creators. Fetch only.
- Manifold's `test/models` (Apache-2.0).

### Recommended test architecture
1. **Predicates**: exact and filtered `orient2d`/`orient3d`/`incircle`. Kani proofs of sign agreement with an i128/expansion reference on bounded inputs, plus exhaustive tests on near-degenerate cases.
2. **Validity oracle after every operation in debug builds and CI**, in the style of `checkshape` and `bopargcheck`:
   - Closed, consistently oriented shells.
   - Euler–Poincaré holds.
   - No self-intersection.
   - Edge/pcurve agreement within tolerance.
   - Tolerance not inflated beyond policy.
3. **Property tests** (bolero/proptest):
   - vol(A∪B) + vol(A∩B) = vol A + vol B.
   - A − A = ∅.
   - Booleans commute with rigid transforms.
   - Tessellation is watertight, with deviation ≤ δ.
   - IFC/STEP write→read→write is byte-identical.
4. **Fuzzing**: IFC/STEP Part 21 parsers; structured CSG and sweep generators, following Manifold's domain pattern; a polygon-triangulation corpus.
5. **Regression corpus, OCCT-style**: one fixture per bug, asserting volume, area, topology counts and bounding box, plus a canonical-output hash. Use KNOWN/IMPROVED/FAILED status semantics.
6. **Bit-identity**: the same corpus runs natively and as wasm32 under wasmtime; output hashes must match.
7. **Differential testing (nightly, out-of-process, never linked)**: compare volume, area and Hausdorff distance against IfcConvert (OCCT, CGAL and Manifold backends), web-ifc, OCCT DRAW and Manifold. Triage every disagreement; that is how [#9570](https://github.com/IfcOpenShell/IfcOpenShell/issues/9570) was found.
8. **Mutation testing**: `cargo-mutants --in-diff` on each PR; full sharded runs weekly. Gate on mutation score for predicates and topology.
9. **Coverage**: [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov). Target 100% branch coverage on predicates and topology, ≥90% line coverage elsewhere (our target).
10. **Benchmarks**: [iai-callgrind](https://github.com/iai-callgrind/iai-callgrind) instruction counts as a deterministic CI gate (±2%, our proposal); criterion for wall-clock trends.

---

## Adopt / Avoid / Open questions

**Adopt**
- A recipe IR with an evaluated cache (IfcOpenShell taxonomy pattern).
- Part 42-style coedge B-rep with pcurves, seams and per-entity tolerance.
- Tolerance tied to `IfcGeometricRepresentationContext.Precision`.
- Edge-first watertight tessellation with deviation bounds.
- Batched opening cuts.
- These crates: parry (`enhanced-determinism`), rstar, spade, robust, cavalier_contours, iOverlay, meshoptimizer/meshopt (pending a WASM determinism test), libm.
- These test tools: Kani, Verus (integer/topology only), bolero, cargo-mutants, cargo-llvm-cov, iai-callgrind.
- OCCT-style TODO/IMPROVEMENT corpus semantics.
- Manifold-style ε-validity contract and fuzzing.
- Out-of-process differential oracles.

**Avoid**
- Fixed segment counts (web-ifc).
- Absolute magic epsilons (web-ifc `eps.h`).
- Fuzz-escalation retry loops and "dilation" hacks.
- Swallowing exceptions to fall through backends (`hybrid_kernel`).
- Silently dropping cuts.
- Converting non-uniform scale to B-spline everywhere.
- Linking OCCT, CGAL, IfcOpenShell, MeshFix, Creusot contracts or Indirect_Predicates.
- Adapting CC BY-ND IFC documentation text.
- Platform libm and HashMap iteration order in kernel paths.

**Open questions**
1. Does the co-op allow running LGPL tools (IfcConvert, OCCT DRAW) as CI-only oracles?
2. Do we need a ruled/bilinear surface type for lossless IfcExtrudedAreaSolidTapered, or is triangulation with provenance acceptable?
3. Spiral evaluation: fixed-order quadrature or series? Its determinism must be specified.
4. Does meshoptimizer compiled to WASM produce bit-identical output, or should we port it?
5. Should the clash-distance policy report intervals or exact B-rep distance?
6. Which IFC sample sets can we vendor? IFC4.x-IF has no licence.
7. Is the ISO 10303-42 edition (2025, Ed. 9) confirmed? iso.org was not machine-readable.
