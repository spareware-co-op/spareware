*Evidence for [G01](../G01-geometry-kernel.md), the Spare Kit geometry kernel decision note. Written 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of each project at the pinned commits linked below. GPL and LGPL code is described in the agent's own words, never copied. Claims it could not confirm from a primary source are marked unverified.*

OCCT, Parasolid and ACIS: B-rep kernel evidence for the Spare Kit geometry kernel

**Sources and pinning.** All OCCT links point to commit `53706f78a8640c4407bdcc6752df13a270b159f2` (master, 2026-10-20). I read the files with `gh api` and raw `curl`, and copied no code; everything below is in my own words. Vendor material is public documentation only:
- **Parasolid:** pages from the v12 and V35 Functional Description, hosted on a third-party site (q-solid.com, plain HTTP).
- **ACIS:** pages from the R10 documentation, hosted on a University of Arizona site.
- **Caveat:** both are unofficial copies of vendor documentation. If provenance matters to the decision note, re-check against Siemens and Spatial's own pages.

"*unverified*" marks inferences I did not confirm in source.

---

## 1. Topology data model

**Shape is a handle to shared topology.** OCCT splits a shape into two parts:
- `TopoDS_TShape` holds the shareable topology: a list of child shapes plus a 16-bit state word ([TShape.hxx L36-61, L65-82, L184-187](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/TopoDS/TopoDS_TShape.hxx#L36-L187)).
- `TopoDS_Shape` is a by-value triple of TShape handle, `TopLoc_Location` and `TopAbs_Orientation` ([Shape.hxx L323-327](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/TopoDS/TopoDS_Shape.hxx#L323-L327); [modeling_data.md L784-797](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/modeling_data/modeling_data.md?plain=1#L784-L797)).
- **Three equality levels** ([Shape.hxx L260-280](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/TopoDS/TopoDS_Shape.hxx#L260-L280)):
  - *partner*: same TShape.
  - *same*: same TShape and same Location.
  - *equal*: also the same orientation.
- **Sharing.** A cube edge is one TEdge, used FORWARD by one face and REVERSED by the other.

**Types and orientation.**
- The type order is COMPOUND > COMPSOLID > SOLID > SHELL > FACE > WIRE > EDGE > VERTEX (+SHAPE) ([TopAbs_ShapeEnum.hxx L20-56](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKG3d/TopAbs/TopAbs_ShapeEnum.hxx#L20-L56)).
- Orientation is FORWARD / REVERSED / INTERNAL / EXTERNAL ([TopAbs_Orientation.hxx L20-46](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKG3d/TopAbs/TopAbs_Orientation.hxx#L20-L46)).
- Mixed-dimensional and non-manifold models are allowed ([modeling_data.md L581](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/modeling_data/modeling_data.md?plain=1#L581)).
- There is **no explicit coedge, fin or half-edge** entity. Face-edge use is implied by wire membership plus orientation.

**Locations.**
- `TopLoc_Location` is a chain of shared `TopLoc_Datum3D` elements raised to powers ([TopLoc_Location.hxx L30-34](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/FoundationClasses/TKMath/TopLoc/TopLoc_Location.hxx#L30-L34)).
- Equality compares datum identity, **not numbers**: two identical transforms built separately are unequal ([modeling_data.md L739](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/modeling_data/modeling_data.md?plain=1#L739)).
- The docs say datums carry no scale ([L764](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/modeling_data/modeling_data.md?plain=1#L764)). But `Shape::Location/Move` only rejects scaling or mirroring if the caller opts in (default is off) ([Shape.hxx L95-103, L313-321](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/TopoDS/TopoDS_Shape.hxx#L95-L321)).

**Geometry attachments.**
- `BRep_TVertex`: a point, a tolerance, and point representations on curves and surfaces ([L29-66](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/BRep/BRep_TVertex.hxx#L29-L66)).
- `BRep_TEdge`: a tolerance, SameParameter / SameRange / Degenerated flags, and a list of curve representations ([L29-81](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/BRep/BRep_TEdge.hxx#L29-L81)). The representation kinds are ([BRep_CurveRepresentation.hxx L39-90](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/BRep/BRep_CurveRepresentation.hxx#L39-L90)):
  - a 3D curve;
  - a pcurve on a surface;
  - a pair of pcurves on a closed surface (seams);
  - a regularity (continuity) record between two surfaces;
  - a 3D polygon, a polygon on a triangulation, or a 2D polygon.
- `BRep_TFace`: a surface, a Location, a tolerance, a NaturalRestriction flag and optional triangulations. The surface may be shared between faces; triangulations may not ([L29-50, L138-144](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/BRep/BRep_TFace.hxx#L29-L144)).
- **Pcurve lookup is keyed by (surface pointer, location), not by face.** If no pcurve is stored and the surface is a plane, one is silently projected on the fly ([BRep_Tool.cxx L359-405](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/BRep/BRep_Tool.cxx#L359-L405)).

**Strengths**
- Cheap instancing and orientation flips.
- One uniform hierarchy, including compounds and mixed dimensions.
- Graded identity (partner / same / equal).

**Weaknesses**
- **Mutable shared state.**
  - TShapes are heap objects edited in place. `TopoDS_Builder::Add/Remove` only guard a "Free" flag ([TopoDS_Builder.cxx L45-110](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/TopoDS/TopoDS_Builder.cxx#L45-L110)).
  - `BRep_Builder::Update*` throws only on "Locked", otherwise mutates and sets Modified ([BRep_Builder.cxx L570-577](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/BRep/BRep_Builder.cxx#L570-L577)).
  - Setting Modified clears the cached "Checked" flag ([TShape.hxx L101-109](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/TopoDS/TopoDS_TShape.hxx#L101-L109)).
  - Any edit is visible to every shape that shares the TShape.
- **Identity is a memory address.** `std::hash<TopoDS_Shape>` hashes the raw TShape pointer ([Shape.hxx L329-341](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/TopoDS/TopoDS_Shape.hxx#L329-L341)). Iteration order of hash maps keyed by shapes therefore depends on allocation order. *Unverified* whether this changes outputs in practice.
- **Redundant representations.** Keeping a 3D curve plus N pcurves per edge forces SameParameter synchronisation (§2).

## 2. Tolerant modelling

**What a tolerance means (BRep format spec).**
- Vertex tolerance: the largest distance from the vertex to any of its representations ([brep_format.md L1781-1784](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/brep_format.md?plain=1#L1781-L1784)).
- Edge tolerance: the largest Hausdorff-style distance from the edge to any of its curve representations ([L1851-1853](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/brep_format.md?plain=1#L1851-L1853)).
- Face tolerance: the largest distance from the face to its surface ([L1872-1874](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/brep_format.md?plain=1#L1872-L1874)).

**Every entity is tolerant, with a floor.**
- Reads clamp each tolerance up to `Precision::Confusion()` = 1e-7 ([BRep_Tool.cxx L169-181, L915-929, L1348-1365](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/BRep/BRep_Tool.cxx#L169-L1365); [Precision.hxx L123, L165, L235, L334](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/FoundationClasses/TKernel/Precision/Precision.hxx#L123-L334)).

**How tolerances grow.**
- `UpdateTolerance` only ever raises the value: it takes the max of old and new ([TVertex L43-45](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/BRep/BRep_TVertex.hxx#L43-L45), [TEdge L50-52](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/BRep/BRep_TEdge.hxx#L50-L52)).
- `BRepLib::UpdateTolerances` enforces vertex ≥ edge ≥ face, and only by increasing values ([BRepLib.cxx L1744-1745, L1843](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKTopAlgo/BRepLib/BRepLib.cxx#L1744-L1843)).
- Booleans grow tolerances themselves:
  - A vertex-on-edge or vertex-on-face hit sets the vertex tolerance to the larger of its old value and (distance + edge or face tolerance).
  - Merged vertices become the bounding sphere of the input tolerance spheres ([boolean_operations.md L157-185](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L157-L185)).
- Healing fixes deviations mostly by raising tolerance ([shape_healing.md L1943-1960](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/shape_healing/shape_healing.md?plain=1#L1943-L1960), [L573-602](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/shape_healing/shape_healing.md?plain=1#L573-L602)).
- The simple offset algorithm accepts tolerance growth as the price of closing gaps ([modeling_algos.md L2225](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/modeling_algos/modeling_algos.md?plain=1#L2225)).

**SameRange and SameParameter.**
- *SameRange*: every representation of the edge uses one parameter interval.
- *SameParameter*: at any parameter t, the 3D curve and every pcurve-on-surface agree to within the edge tolerance.
- `BRepLib::SameParameter` re-fits pcurves to achieve this, or raises the tolerance ([BRepLib.hxx L66-83, L159-175](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKTopAlgo/BRepLib/BRepLib.hxx#L66-L175)).

**How large tolerances cause failures downstream.** All of these are documented by OCCT itself:
- **Feedback loop.** After a vertex's tolerance grows, the PaveFiller re-runs vertex-vertex, vertex-edge and vertex-face intersections for it (`RepeatIntersection`) ([BOPAlgo_PaveFiller.cxx L359-422](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_PaveFiller.cxx#L359-L422)). Tolerance effects are non-local.
- **Self-interference caused by tolerance.** A valid edge whose two vertex spheres overlap, or a 50-unit vertex that swallows unrelated sub-shapes, makes an input that BRepCheck passes but the Boolean cannot use ([L2506-2525](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L2506-L2525)).
- **Gaps "closed" by vertex spheres.** A large vertex can bridge a gap, but the path across it is undefined, so face building fails ([L2553-2570](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L2553-L2570)).
- **Two straight edges through the same two fat vertices.** OCCT documents this as "cannot be solved in general" ([L2590-2635](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L2590-L2635)).
- **Acquired self-interference.** Arguments become self-interfering through tolerances inherited from another argument ([L2636-2660](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L2636-L2660)).
- **Scale defeats angle-based decisions.** Scaling a cylinder by 1e6 makes pcurve slopes indistinguishable to `acos` ([L2526-2552](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L2526-L2552)).

**Contrast with Parasolid and ACIS: exact by default, tolerant only where needed.**
- **Parasolid** ([Session & Local Precision, V35 ch.16](http://www.q-solid.com/Parasolid_Docs_V35/chapters/fd_chap.017.html)):
  - Fixed session precision: 1e-8 linear and 1e-11 angular.
  - All geometry must sit inside a "size box" of 1000 units (recommended unit: metres).
  - Coincidence must be transitive for a body to be valid.
  - Untoleranced edges and vertices are *exact*, with an implied precision of half the session precision.
  - Local precision ("tubes and spheres") is set only on edges and vertices, mainly for imported data. Some B-surface operations also use it internally.
  - Making an edge tolerant **deletes its 3D curve** and stores SP-curves (pcurves) on each fin. An optional "nominal" 3D curve can be kept.
  - A vertex's precision must be at least the largest precision of its edges.
  - Edges are split at discontinuities in the surface's parameter space.
  - `PK_EDGE_optimise` can lower a precision again.
  - The exact versus tolerant edge representation is described in [Model Structure §14.8](http://www.q-solid.com/Parasolid_Docs_V35/chapters/fd_chap.015.html).
  - Origin paper: D. J. Jackson, *Boundary representation modelling with local tolerances*, ACM SMA '95, pp. 247–254 ([PDF, scanned](https://ftp1.cs.wisc.edu/pub/users/prem/jackson-SM-95.pdf)). I confirmed the abstract only; the scan was not machine-readable.
- **ACIS** ([How tolerant modeling works](http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/KERN/KERN/06TMOD/0001.HTM), [Tolerant edges](http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/KERN/KERN/06TMOD/0002.HTM), [Tolerant vertices](http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/KERN/KERN/06TMOD/0003.HTM), [options](http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/KERN/KERN/06TMOD/0004.HTM)):
  - Exact models use the global resolution `SPAresabs`.
  - TEDGE, TCOEDGE and TVERTEX are introduced only where needed. On a TCOEDGE the pcurve is the primary geometry and the 3D curve is computed lazily.
  - Edge tolerance is the largest gap between equal-parameter points on its coedges.
  - The system maintains tolerances; users cannot set them.
  - The `*_tolerant_hot` options let local operations and blending treat near-tangent edges as tangent, to avoid ill-conditioned intersections.

## 3. Boolean Operations Algorithm (BOPAlgo / BOPDS / IntTools)

**Architecture.**
- The *Intersection Part* (`BOPAlgo_PaveFiller`) fills an indexed data structure, `BOPDS_DS`, which holds:
  - per-shape info in arrays addressed by integer index;
  - pools of pave blocks and face info;
  - same-domain maps;
  - one typed interference array per pair type (VV, VE, VF, EE, EF, FF, VZ, EZ, FZ, ZZ)
  ([BOPDS_DS.hxx L48-63, L479-502](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPDS/BOPDS_DS.hxx#L48-L502)).
- The *Building Part* reads that DS. General Fuse, BOP, Section, Splitter, MakerVolume and CellsBuilder are all different selections from the same intersection result ([boolean_operations.md L752-770](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L752-L770)).

**Interferences.**
- Two shapes interfere wherever their geometry is closer than the sum of their tolerances.
- The six B-rep pair types are processed in increasing dimension, so lower-dimension results suppress redundant higher-dimension work.
- Four "non-B-rep" containment types (V/E/F/Z inside a solid) are handled separately ([L139-291](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L139-L291)).

**Paves, pave blocks and common blocks** ([L299-387](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L299-L387); [PaveBlock L183-193](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPDS/BOPDS_PaveBlock.hxx#L183-L193); [CommonBlock L135-137](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPDS/BOPDS_CommonBlock.hxx#L135-L137)):
- A *pave* is a (vertex, parameter) pair on a curve.
- A *pave block* is the piece of curve between neighbouring paves.
- The *shrunk range* is the part of a pave block outside its vertex tolerance spheres. Only that part may interfere with anything else.
- A *common block* groups coincident pave blocks from different edges, plus the faces they lie on, with one tolerance.
- *FaceInfo* records, per face, the pave blocks and vertices that are In, On, or newly made by sections.

**PaveFiller stages, in source order** ([PerformInternal L234-353](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_PaveFiller.cxx#L234-L353); stage tables in [L567-750](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L567-L750)):
1. **Init.** Build the DS and a box-overlap pair iterator (fuzzy value added to boxes; optional oriented boxes) ([L174-213](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_PaveFiller.cxx#L174-L213)).
2. **VV.** Connected chains of touching vertices become one new vertex.
3. **VE.** Add paves to edges.
4. Split pave blocks at the new paves.
5. **EE.** Find vertex-type hits and overlap-type (common block) hits. New vertices are merged by running a nested PaveFiller on them.
6. **VF**, then **EF**, then RepeatIntersection for vertices whose tolerance grew.
7. **ForceInterfEE/EF.**
8. **FF** via `IntTools_FaceFace`: section curves and points.
9. **MakeSplitEdges**, then **MakeBlocks**. Section curves get paves from existing On/In vertices plus "technological" end vertices; draft section edges are re-intersected by another nested PaveFiller.
10. Self-interference check, micro-edge removal, **MakePCurves**, and degenerated-edge processing.
- Non-analytic face pairs get a hard-coded floor of 5e-6 on their tolerance ([ToleranceFF, PaveFiller_6.cxx L3939-3961](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_PaveFiller_6.cxx#L3939-L3961)).

**Building the result.**
- Split each face: gather the split edges and section edges, then build new loops with `BOPAlgo_BuilderFace` ([L30-74](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_BuilderFace.hxx#L30-L74)). Loops are traced by a minimum-clockwise-angle walk in UV space, with 2D tolerances derived from vertex tolerances ([WireSplitter_1.cxx L85-110, L358-620](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_WireSplitter_1.cxx#L85-L620)). This is a floating-point angle decision, not an exact predicate.
- Find same-domain faces ([Builder_2.cxx L233, L580](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_Builder_2.cxx#L233-L580)).
- Classify faces as inside solids and build split solids ([Builder_3.cxx L97, L413](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_Builder_3.cxx#L97-L413)). `BOPAlgo_BuilderSolid` makes closed shells, sorts them into holes and growths, and reports unused faces as a warning ([L32-56](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_BuilderSolid.hxx#L32-L56)).
- `BOPAlgo_BOP` then selects pieces by state: `BuildRC` / `BuildShape` / `BuildSolid` ([L583-1097](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_BOP.cxx#L583-L1097)).

**Options** ([BOPAlgo_Options.hxx L25-35](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_Options.hxx#L25-L35); [Builder.hxx L42-74](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_Builder.hxx#L42-L74)):
- **Fuzzy**: an extra user tolerance to absorb gaps and embeddings in imported data ([L2667-2678](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L2667-L2678)).
- **Glue (Shift / Full)**: skips FF and other work when the user *promises* inputs only coincide. This is not checked ([L2738-2765](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L2738-L2765)).
- **Non-destructive ("safe") mode**: inputs are never modified (no tolerance growth, no added pcurves). It is **off by default**, and the docs recommend it for undo ([L2808-2818](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L2808-L2818)).
- Parallel mode and oriented bounding boxes.

**History.**
- `BRepTools_History` defines Generated, Modified and Removed relations between input and output shapes ([L27-88](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/BRepTools/BRepTools_History.hxx#L27-L88)). Rules:
  - an input cannot be both removed and modified;
  - an input's generated and modified sets must not overlap;
  - every output is an input or traces back to one;
  - histories compose across chained operations.
- For Booleans, "Generated" applies only to intersection vertices and edges; overlaps count as "Modified" ([L2953-2968](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L2953-L2968)).

**Errors.**
- `Perform` converts any exception into `AlertIntersectionFailed` ([L215-230](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_PaveFiller.cxx#L215-L230)).
- Alerts carry the offending shape, e.g. TooSmallEdge, NotSplittableEdge, BadPositioning, AcquiredSelfIntersection, BuildingPCurveFailed, SolidBuilderUnusedFaces ([BOPAlgo_Alerts.hxx L21-145](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_Alerts.hxx#L21-L145)).

**Known failure modes (OCCT's own words, [L2443-2660](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L2443-L2660)):**
- Bugs in the lower-level intersection, projection, approximation and classification algorithms propagate directly into wrong results.
- Inputs must be valid *and* free of self-interference, and the arguments must be at least C1.
- Edges whose curves are of different analytic types never form common blocks; the result is forced to a vertex.
- The tolerance pathologies listed in §2.
- Input validity is "necessary but insufficient" ([L2477](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L2477)).

## 4. Surface-surface intersection (IntPatch / IntWalk)

**Dispatch.** `IntPatch_Intersection::Perform` sorts each surface into a type and routes the pair ([L1066-1345](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKGeomAlgo/IntPatch/IntPatch_Intersection.cxx#L1066-L1345)):
- Plane, cylinder, sphere and cone count as quadrics. Torus counts only conditionally.
- **Quadric–quadric:** closed-form (`IntPatch_ImpImpIntersection`). If that fails it silently falls back to the parametric–parametric path ([L1789-1797](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKGeomAlgo/IntPatch/IntPatch_Intersection.cxx#L1789-L1797)).
- **Implicit–parametric:** `IntPatch_ImpPrmIntersection` (the implicit equation evaluated on the parametric surface, then marching).
- **Parametric–parametric:**
  - Start points come from intersecting the two triangulations (`IntPolyh`; default 10×10 samples per surface) ([IntPolyh_Intersection.hxx L30-44](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKGeomAlgo/IntPolyh/IntPolyh_Intersection.hxx#L30-L44)).
  - `IntWalk_PWalking` then marches from each start point with steps controlled by deflection. It stops at domain boundaries or at a tangent point ([L31-54](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKGeomAlgo/IntWalk/IntWalk_PWalking.hxx#L31-L54)).

**Turning a walk into an edge.** In `IntTools_FaceFace`, the marched polyline (in u1,v1,u2,v2) is approximated by one 3D B-spline plus two 2D pcurves (`GeomInt_WLApprox`, degree 4–8). The achieved error is then measured and **becomes the section edge's tolerance** ([L1176-1410](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/IntTools/IntTools_FaceFace.cxx#L1176-L1410); `ComputeTolReached3d` [L614](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/IntTools/IntTools_FaceFace.cxx#L614)).

**Why SSI is the hardest part.**
1. **Completeness.** Sampled start points can miss small closed loops and isolated points.
2. **Conditioning.** At tangential and near-tangent contacts the system becomes singular, so the curve is undefined within the tolerance band.
3. **Singularities.** Branch points, poles and seams.
4. **Overlaps.** Coincident surfaces meet in a 2D region, not a curve.
5. **No exact representation.** The result generally cannot be written exactly in NURBS (algebraic degree up to 324 for two bicubic patches; *unverified*, standard in the literature). So approximation injects tolerance back into the topology.
6. **Consistency.** The 3D curve and both pcurves must agree.

Reference: Patrikalakis, Maekawa & Cho, *Shape Interrogation for CAD/CAM*, chapters 4–6 ([online](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/)). The OCCT spec also notes IntPatch does not compute tolerance-based "common zones", which limits what the EE and EF steps can feed into FF ([L2587-2589](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L2587-L2589)).

## 5. Validity checking and shape healing

**`BRepCheck_Analyzer` checks, by entity.** Statuses are listed in [BRepCheck_Status.hxx L20-58](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKTopAlgo/BRepCheck/BRepCheck_Status.hxx#L20-L58).
- **Vertex:** point lies on its curve, curve-on-surface and surface representations, using the larger of the vertex and edge (or face) tolerance ([BRepCheck_Vertex.cxx L145-270](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKTopAlgo/BRepCheck/BRepCheck_Vertex.cxx#L145-L270)).
- **Edge:**
  - 3D curve present and unique;
  - pcurve present on each face;
  - SameRange and SameParameter flags truthful;
  - degenerate flag and range valid;
  - free or multiply-connected edges flagged.
  The curve-versus-pcurve check is **sampled at a fixed number of points (`NCONTROL`=23)** unless the "exact" method is chosen ([BRepCheck_Edge.cxx L62, L415-436](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKTopAlgo/BRepCheck/BRepCheck_Edge.cxx#L62-L436); [BRepLib_ValidateEdge.hxx L23-38](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKTopAlgo/BRepLib/BRepLib_ValidateEdge.hxx#L23-L38)). OCCT itself shows that splitting a "valid" edge can expose its invalidity ([L2463-2477](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/specification/boolean_operations/boolean_operations.md?plain=1#L2463-L2477)).
- **Wire:** closed in 3D and in 2D, connected, not self-intersecting in UV, orientation consistent ([BRepCheck_Wire.cxx L283-1074](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKTopAlgo/BRepCheck/BRepCheck_Wire.cxx#L283-L1074)).
- **Face:** wires do not intersect, wires are properly nested (imbrication), wire orientation correct, face is orientable ([BRepCheck_Face.cxx L173-587](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKTopAlgo/BRepCheck/BRepCheck_Face.cxx#L173-L587)).
- **Shell:** closed (each oriented edge used exactly twice), connected, consistently oriented, no multi-connexity ([BRepCheck_Shell.cxx L237-824](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKTopAlgo/BRepCheck/BRepCheck_Shell.cxx#L237-L824)).
- **Solid:** shells properly nested, no more than one growth, nothing outside the solid ([BRepCheck_Solid.cxx L183-330](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKTopAlgo/BRepCheck/BRepCheck_Solid.cxx#L183-L330)).

**Gaps in the checker.**
- Tolerance-value checks are marked "NYI" (not yet implemented) ([Analyzer.hxx L46-58](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKTopAlgo/BRepCheck/BRepCheck_Analyzer.hxx#L46-L58)).
- There is no global face-face self-intersection check. That lives separately in `BOPAlgo_ArgumentAnalyzer`, which also checks small edges, tangency, merge problems, continuity and curve-on-surface ([L29-91](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKBO/BOPAlgo/BOPAlgo_ArgumentAnalyzer.hxx#L29-L91)).
- By contrast, Parasolid's `PK_BODY_check` runs gated groups ([Checking ch.8](http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.09.html)): geometry, B-geometry continuity, topology-geometry consistency, size box, face self-intersection, loops, **face-face**, inside-out shells, and corrupt data. Parasolid also states that its functions assume valid inputs.

**ShapeFix repairs** ([shape_healing.md L99-118, L316-410, L573-602](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/shape_healing/shape_healing.md?plain=1#L99-L602)). The list shows which invariants a kernel must defend:
- missing 3D curves or pcurves;
- pcurves reversed or shifted by a period on closed surfaces;
- a missing seam;
- SameParameter deviations;
- unordered or disconnected wire edges;
- small edges;
- self-intersecting or lacking wires;
- degenerate edges at poles;
- wire orientation and nesting on the face, and natural bounds;
- incoherent face orientation in shells (non-orientable shells such as a Möbius strip are split);
- solid orientation (material inside);
- tolerance clamping between min and max.
Fixes work by raising tolerance, changing topology, or changing geometry, gated by `ModifyTopology` / `ModifyGeometry` flags. Every heal is a trade between these three.

## 6. Hidden line removal (HLRBRep / HLRAlgo)

- **Method.** Each edge is compared with each face (O(E·F), with min/max box rejection). There are two variants ([modeling_algos.md L3245-3300](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/modeling_algos/modeling_algos.md?plain=1#L3245-L3300)):
  - `HLRBRep_Algo`: exact.
  - `HLRBRep_PolyAlgo`: on triangulations, giving polygonal output.
- **Outlines.** Silhouettes are computed per face by `Contap_Contour` (where the surface normal is perpendicular to the view direction), approximated, and inserted as edges; isolines come from a hatcher ([HLRTopoBRep_DSFiller.cxx L64-107, L479](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKHLR/HLRTopoBRep/HLRTopoBRep_DSFiller.cxx#L64-L479)).
- **Hiding.** For each face, interferences (In and On) with each unrejected edge are found in projection. Complex transitions are resolved, and hidden, on-face and boundary intervals are subtracted from the edge's visible intervals ([HLRBRep_Hider.cxx L41-84](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKHLR/HLRBRep/HLRBRep_Hider.cxx#L41-L84); [HLRAlgo_EdgeStatus.hxx L26-33, L73-76](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKHLR/HLRAlgo/HLRAlgo_EdgeStatus.hxx#L26-L76)).
- The driver loop has fast paths for plane, cylinder, cone, sphere and torus faces ([HLRBRep_InternalAlgo.cxx L613-700](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKHLR/HLRBRep/HLRBRep_InternalAlgo.cxx#L613-L700)).
- **Output classes:** sharp, smooth, sewn and outline edges, each visible or hidden, plus isoparametric lines.
- **Limitations:** points and infinite entities are not processed.

## 7. Fillets (ChFi3d) and offsets (BRepOffset)

**Fillets.**
- **Method.**
  - Contours ("spines") propagate along tangent-continuous edges ([ChFiDS_Spine.hxx L45-56](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKFillet/ChFiDS/ChFiDS_Spine.hxx#L45-L56)).
  - The blend surface is computed by **marching** a rolling-ball constraint system along the spine, with deflection and tolerance controls ([MakeFillet.hxx L57-93](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKFillet/BRepFilletAPI/BRepFilletAPI_MakeFillet.hxx#L57-L93)).
  - Corners are solved by case analysis on how many stripes meet: one, two, three or more, with a "last chance" catch-all ([ChFi3d_Builder.cxx L602-720](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKFillet/ChFi3d/ChFi3d_Builder.cxx#L602-L720)).
  - Topology is rebuilt with the **legacy TopOpeBRep Boolean data structure**, not BOPAlgo ([L47-54, L162-400](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKFillet/ChFi3d/ChFi3d_Builder.cxx#L47-L400)).
- **Pain points** ([MakeFillet.hxx L296-311, L366-380](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKFillet/BRepFilletAPI/BRepFilletAPI_MakeFillet.hxx#L296-L380); [ChFiDS_ErrorStatus.hxx L20-28](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKFillet/ChFiDS/ChFiDS_ErrorStatus.hxx#L20-L28)):
  - No up-front feasibility analysis; errors appear only during construction.
  - Unsupported: contour ends where four or more edges meet, and a fillet that runs off its limiting face.
  - Failure statuses: WalkingFailure, StartsolFailure, TwistedSurface.
  - On corner failure it can return a shape **with a hole** (`BadShape`).

**Offsets.**
- **Two algorithms** ([modeling_algos.md L2208-2240](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/modeling_algos/modeling_algos.md?plain=1#L2208-L2240)):
  - **Join:** offset each surface, extend it analytically, intersect neighbours in 3D and 2D, rebuild loops, faces, shells and the solid ([MakeOffset.cxx L827-1072, L2838-3716](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKOffset/BRepOffset/BRepOffset_MakeOffset.cxx#L827-L3716)).
  - **Simple:** map surfaces and pcurves directly, average vertex positions, and let tolerances absorb the gaps.
- **Pain points:**
  - The angular tolerance is derived from tolerance ÷ offset distance (asin) ([L871-873](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKOffset/BRepOffset/BRepOffset_MakeOffset.cxx#L871-L873)).
  - Edges that are partly sharp and partly tangent are rejected (MixedConnectivity) ([L880-893](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKOffset/BRepOffset/BRepOffset_MakeOffset.cxx#L880-L893)).
  - Global self-intersection resolution ("auto unwinding") is **commented out** ([L921-924](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKOffset/BRepOffset/BRepOffset_MakeOffset.cxx#L921-L924)).
  - Error enum includes BadNormals, C0Geometry, NotConnectedShell, CannotTrim/Fuse/Extent ([BRepOffset_Error.hxx L20-35](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKOffset/BRepOffset/BRepOffset_Error.hxx#L20-L35)).
- **Parasolid by contrast** documents explicit topology-change classes for offset, taper and tweak: vertex splitting, repair of self-intersecting faces, removal of clashing faces, removal of mirrored faces. Faces whose curvature radius is below the offset are deleted ([Local Ops ch.12](http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.13.html)). Wound healing after face deletion offers cap, grow-from-parent, grow-from-child, shrink, or "rubber" ([ch.14](http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.15.html)).

## 8. Public concepts from Parasolid, ACIS and ShapeManager

**Parasolid.**
- **Partitions and rollback** ([ch.41](http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.42.html)):
  - A *partition* is a self-contained set of bodies, orphan geometry and transforms that can be rolled back independently.
  - References between partitions are forbidden. Booleans must stay within one partition, and bodies move only by copy (or a rule for newly created bodies).
  - *PMARK*: a partition state. *MARK*: a whole-session state. *DELTA*: the creates, modifies and deletes between adjacent pmarks.
  - Deltas are streamed through an application-supplied "delta frustrum" (the application decides where they are stored).
  - Stated uses: undo, recovery from failed operations, "try and see" modelling, faster feature replay, branching rollback.
- **Identity** ([ch.2](http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.03.html)):
  - *Tags* are session-unique integer handles. They die on delete, merge or rollback.
  - Tag persistence: a truncated face keeps its tag; when a face splits, one piece keeps it and the others get new tags.
  - *Identifiers* are unique per body and persist in saved files.
  - Errors are returned as codes, *plus* a separate failure token for "the call succeeded but the operation failed".
- **Model structure** ([V35 ch.14](http://www.q-solid.com/Parasolid_Docs_V35/chapters/fd_chap.015.html)):
  - Body, region (solid or void; there is always an infinite void region), shell, face, loop, **fin** (the oriented use of an edge by a loop), edge, vertex.
  - Fins are ordered radially around their edge. Body types: acorn, wire, sheet, solid, general.
- **Attributes** ([ch.46](http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.47.html)):
  - An attribute *definition* has a class that fixes its behaviour on split, merge, transform, copy and delete. Callbacks can replace or supplement that behaviour.
  - Definitions are session-level, cannot be changed, and survive rollback.
- **Booleans** ([ch.9](http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.10.html)):
  - Imprinting, then region classification.
  - Global versus local Booleans: local ones do not guarantee topological consistency.
  - Matched-topology hints, which play the role of OCCT's glue.
  - `default_tol` / `max_tol` **put a ceiling on tolerance growth** for each operation.
  - Merge options, fence option, and local face checking.

**ACIS.**
- **History and roll** ([bulletins](http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/KERN/KERN/07HIST/0001.HTM), [boards](http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/KERN/KERN/07HIST/0002.HTM), [APIs](http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/KERN/KERN/07HIST/0003.HTM), [delta states](http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/KERN/KERN/07HIST/0006.HTM)):
  - Copy-on-write: each changed entity produces a bulletin holding its old and new records.
  - Bulletins are grouped into a board per API call, boards into delta states, and delta states into history streams.
  - **A failed API call is rolled back automatically at the next call.**
  - Bulletin boards can be compressed.
- **Tolerant entities:** TEDGE, TCOEDGE, TVERTEX (see §2).
- ACIS supports attributes; the details of how they behave on split and merge are *unverified*.

**Autodesk ShapeManager.**
- Forked from ACIS 7.0 in November 2001; used in Inventor, AutoCAD and Fusion ([Wikipedia](https://en.wikipedia.org/wiki/ShapeManager)).
- Revit's use of it is *unverified*. No technical documentation was found.

## 9. Lessons for Spare Kit

**What OCCT got right**
- **Compute the arrangement once, select many times.** One intersection data structure serves fuse, cut, common, section, splitter and cells. Cells and regions map directly onto BIM spaces and wall splitting.
- **Explicit pave-block and common-block bookkeeping** for overlaps, with integer-indexed arrays inside the Boolean.
- **A formal history contract** with composition laws.
- **Typed alerts that carry shapes.**
- **A safe mode, a fuzzy option, and a checker paired with a healer.** The checker and healer together enumerate the invariants.
- **Separating use (location and orientation) from definition.**

**What to do differently in Rust, and why**
1. **Make topology immutable and versioned.** Store each body version in an arena with structural sharing; edits produce new versions. This makes OCCT's off-by-default safe mode the only mode, and makes undo free (as ACIS deltas and Parasolid pmarks do, but by construction). Reason: OCCT mutates shared TShapes in place (§1).
2. **Use explicit IDs, never pointers.** Use arena index plus generation, and deterministic ordered containers. Pair session handles with persisted per-body identifiers, and specify split and merge persistence rules (Parasolid tags and identifiers). Reason: OCCT hashes raw addresses (§1).
3. **Be exact by default and tolerant by explicit, bounded exception.**
   - Fix a session precision and a size box: Parasolid's model, 1e-8 within 1000 units, scaled to BIM metres.
   - Tolerant edges and vertices exist only as an import state.
   - Give every operation a `max_tol` ceiling. Exceeding it is an error that rolls back, never silent growth. Reason: the growth feedback loops in §2.
   - Choose one primary edge geometry (Parasolid: 3D curve for exact edges, pcurves on fins for tolerant ones) rather than OCCT's always-redundant 3D curve plus pcurves.
4. **Add explicit coedges or fins with radial ordering, and regions.** Pcurves belong to coedges. Reason: OCCT keys pcurves by surface pointer and silently projects planes (§1).
5. **Treat every operation as a transaction with a total validity postcondition.**
   - Prefer certified checks (interval or Bernstein bounds) over 23-point sampling.
   - Include global face-face self-intersection, as Parasolid does.
   - If the check fails, return an error and keep the previous version. This is how "never returns an invalid solid" is guaranteed.
6. **Error model:** return `Result<T, KernelError>` with a typed cause, entity IDs and diagnostics. Partial results are only explicit debug artefacts. OCCT today mixes exceptions, IsDone flags, alerts and hole-y BadShape results.
7. **Determinism:**
   - Parallelise only pure per-pair work and merge results sorted by stable keys.
   - Ship a pinned pure-Rust libm, because transcendental functions differ between native and wasm.
   - Never let NaN payloads matter (wasm NaN bits are non-deterministic, [spec](https://webassembly.github.io/spec/core/exec/numerics.html#nan-propagation)).
   - Replace angle-based (`atan2`/`acos`) topology decisions with exact orientation predicates ([Shewchuk](https://www.cs.cmu.edu/~quake/robust.html)). OCCT's loop building uses angle comparisons (§3) and fails at extreme scale (§2).
8. **Treat SSI as specialised layers:** closed-form for quadric pairs, used exactly for planar-heavy BIM; certified subdivision or loop detection for NURBS start points; marching last. Record the approximation error as a first-class bound.
9. **Build blends and offsets on the same arrangement engine, with a written topology-change taxonomy** (Parasolid's local ops). OCCT fillets rely on the legacy TopOpeBRep, and offset self-intersection handling is disabled (§7).
10. **Readability:**
    - Keep modules small; OCCT has files of 4.4k–5.5k lines (BOPAlgo_PaveFiller_6.cxx, BRepOffset_MakeOffset.cxx).
    - Put named, justified constants in one place instead of magic floors like 5e-6, `NCONTROL`=23 and the 1e-7 clamp.
    - No commented-out algorithms.
    - State invariants in types.

---

**Adopt**
- The GF / PaveFiller split between intersection and building, with an integer-indexed DS.
- Pave blocks, common blocks and FaceInfo.
- Cells and regions.
- The history G/M/R semantics.
- Typed alerts.
- Fuzzy as an explicit, bounded option.
- Partitioned rollback and deltas (Parasolid / ACIS).
- Tag and identifier persistence rules.
- Attribute classes for split and merge behaviour.
- Exact-by-default precision with a size box.
- Per-operation tolerance ceilings (`max_tol`).
- A grouped validity checker that includes face-face checks.
- A topology-change taxonomy for local operations.

**Avoid**
- Mutable shared topology.
- Pointer-based hashing and identity.
- Location equality by datum identity.
- Silent scale in locations.
- A 3D curve plus N pcurves synchronised only by tolerance.
- Unbounded, monotone tolerance growth.
- Sampled-only validity checks.
- Silent fallbacks (plane projection; ImpImp to PrmPrm without a report).
- Unchecked glue promises.
- Partial "BadShape" results.
- Angle-based floating-point topology decisions.
- A legacy second Boolean engine inside fillets.

**Open questions**
1. Exact rational arithmetic for planar and quadric BIM geometry, versus floating point with certified bounds everywhere?
2. Edge primary geometry: 3D curve (Parasolid exact), pcurve (ACIS tolerant), or both behind a typed tolerant state?
3. BIM size box and units: a km-scale site with sub-mm detail is about a 1e7–1e9 range. Is a single session precision enough?
4. How much tolerant import (IFC, STEP, DWG) must Booleans accept, and what is the default `max_tol`?
5. SSI completeness guarantee for NURBS: which loop-detection criterion?
6. HLR for drawings: exact for analytic faces, with certified-deflection polygonal output for NURBS?
7. Persistent-naming policy for Revit-style element references across splits and merges.
8. Whether the third-party mirrors of Parasolid and ACIS documentation are an acceptable citation source for the decision note.
9. *Unverified:* whether OCCT's address-ordered maps actually cause run-to-run output differences.
