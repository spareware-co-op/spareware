# 18. Drawings: sections, projections and hidden lines

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Evidence: [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md) section 6; [06](../../../kit/notes/G01-evidence/06-2d-drafting.md) sections 1 to 4 and 6; [10](../../../kit/notes/G01-evidence/10-github-sweep.md) sections 1 to 3; [07](../../../kit/notes/G01-evidence/07-persistent-naming.md) sections 6 to 8; [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md) sections A1, B and D; [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) section 4 for plane-based predicates; [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md) sections 3 and 6 for loop tests and certified bounds; [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md) section 1 for subdivision surfaces; [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md) for SMLib. Sources first read for this chapter are pinned where they are cited: Krbn at `09d96d3`, HL for Rhino at `4d9570e`, the IFC 4.3 development repository at `7256780`, and papers by DOI. OpenCascade's HLRBRep is LGPL, so the chapter cites only report 02 and plain descriptions of its failures. The session that wrote this chapter read its source at `53706f7` (HLRBRep_Data.cxx, HLRBRep_Hider.cxx and HLRBRep_HLRToShape.hxx). Under rule 6 of section 29.20 those observations go into a method note in `geometry/methods/`, checked by a second reviewer, before any code is written. That note is still owed. Coding sessions work from it and the papers, never from the source.

A drawing turns the 3D model into the 2D linework people build from: plans, sections, elevations and details of a building; orthographic, section and detail views of a part; long and cross sections of a road; assembly drawings of a circuit board. The kernel's job is the geometry of that linework: which curves, which parts of them are seen, what each line is, and where it came from. How lines look (weights, dashes, hatch patterns, which classes are shown) is program data keyed on what the kernel returns. Drawing never changes the model. A 3D cutaway is a boolean (section 11); a section drawing is a query. All of it lives in `geom-draw`.

## 18.1 Where the open world stands, and how Spare Kit leads

- **OpenCascade's HLRBRep** is the only open hidden-line code on a B-rep that the sweep found ([10](../../../kit/notes/G01-evidence/10-github-sweep.md), section 3, gap 2). It tests each edge against each face that might hide it, in projection, with bounding boxes rejecting the rest ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 6). It is LGPL, so we study its method only, under rule 6 of section 29.20. The reading recorded above found the failures that show why we take its organisation and nothing more:
  - Whether an edge lies under a face is decided with a depth tolerance that is a fixed fraction of the scene's overall extent, as large as a hundredth in some face tests. On a 100 m building a hundredth is a metre.
  - Failures are silent. A face whose parameter bounds look wrong is skipped and hides nothing. If hiding one edge by one face fails, the failure is dropped and the loop moves on; only debug builds report anything.
  - Silhouettes are approximated, and the 2D output edges are not linked back to the source shape ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 6).
- **SMLib**, opened by NVIDIA in October 2026, has a hidden-curve module of IntegrityWare lineage. [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md) excluded it from porting until the owners settled who holds its title; the owner has since accepted NVIDIA's publication (32.8, decided 2026-10-24), so it may be studied and ported with NVIDIA's notice kept (29.6). It was not studied here.
- **FreeCAD TechDraw**, built on HLRBRep, binds dimensions to drawing edges by index and, after a change, searches for identical geometry; the fallback that would look for the most similar geometry is unimplemented ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 6).
- **Krbn** (MIT), a pencil-style renderer, computes exact closed-form silhouettes of quadrics: a conic for spheres and ellipsoids, tangent rulings for cylinders and cones ([quadric.ts L6-L12](https://github.com/vpalos/Krbn/blob/09d96d3d74b50c45c6454c94f5650f56680e0716/src/primitives/quadric.ts#L6-L12), [cylinder.ts L3-L6](https://github.com/vpalos/Krbn/blob/09d96d3d74b50c45c6454c94f5650f56680e0716/src/primitives/cylinder.ts#L3-L6), [cone.ts L3-L8](https://github.com/vpalos/Krbn/blob/09d96d3d74b50c45c6454c94f5650f56680e0716/src/primitives/cone.ts#L3-L8)). For visibility it collects a superset of the places where visibility can change, then decides each piece between them by a depth test at its midpoint ([visibility.ts L1-L14](https://github.com/vpalos/Krbn/blob/09d96d3d74b50c45c6454c94f5650f56680e0716/src/pipeline/visibility.ts#L1-L14), [L187-L194](https://github.com/vpalos/Krbn/blob/09d96d3d74b50c45c6454c94f5650f56680e0716/src/pipeline/visibility.ts#L187-L194)). But a screen-space scan finds its changes at tangencies and, by its own comment, can miss hidden runs thinner than about 4 pixels ([L106-L114](https://github.com/vpalos/Krbn/blob/09d96d3d74b50c45c6454c94f5650f56680e0716/src/pipeline/visibility.ts#L106-L114)). Its depth tests use tolerances relative to the scene ([L75-L82](https://github.com/vpalos/Krbn/blob/09d96d3d74b50c45c6454c94f5650f56680e0716/src/pipeline/visibility.ts#L75-L82)), it has no B-rep topology and no section regions, and it is byte-identical only on one platform ([README L195-L201](https://github.com/vpalos/Krbn/blob/09d96d3d74b50c45c6454c94f5650f56680e0716/README.md?plain=1#L195-L201)).
- **HL for Rhino** (MIT) and ln decide visibility by sampling. HL rasterises faces at 4 pixels a point and samples each line every quarter point ([idbuf.py L39-L40](https://github.com/kenny-t-vo/hidden-lines/blob/4d9570ee7a9d0b6a4ed2ee5c365a637797daaadf/hiddenlines/idbuf.py#L39-L40)); it calls an edge a crease above 20 degrees and a face edge-on below a cosine of 0.0001 ([L49-L50](https://github.com/kenny-t-vo/hidden-lines/blob/4d9570ee7a9d0b6a4ed2ee5c365a637797daaadf/hiddenlines/idbuf.py#L49-L50)). We use two things from it as rules only: its architectural line vocabulary ([README L146](https://github.com/kenny-t-vo/hidden-lines/blob/4d9570ee7a9d0b6a4ed2ee5c365a637797daaadf/README.md?plain=1#L146)) and its list of model faults that break lines, such as two solids running into each other ([README L162-L165](https://github.com/kenny-t-vo/hidden-lines/blob/4d9570ee7a9d0b6a4ed2ee5c365a637797daaadf/README.md?plain=1#L162-L165)). Blender's Freestyle is GPL and not used.

No open project produces exact section drawings from a B-rep, with closed cut regions, named lines and the same result on every machine ([10](../../../kit/notes/G01-evidence/10-github-sweep.md), section 3, gap 2). Spare Kit leads by doing all of it together:

1. **Exact for flat-faced geometry.** Cuts, outlines and visibility of plane-based solids, exact meshes and terrain are decided by integer predicates, with no depth tolerance anywhere.
2. **Closed form or certified for curved geometry.** Quadrics, pipes and tori in closed form; everything else to a proven bound that the result states. Nothing is sampled.
3. **Rules, not tolerances, for the hard cases:** faces seen edge-on, lines that coincide, bodies that run into each other, ties on the cut plane.
4. **Every line knows what it draws:** element, persistent name, kind of line, parameter range. Dimensions and overrides bind through model names and survive model changes.
5. **Closed cut regions,** ready to fill, per element, body region and material.
6. **Incremental, parallel and bit-identical,** stored with the sheet and in the replay corpus of section 21.
7. **Headless.** A person, a script or an AI gets the same data and can ask what any line is and what hides it.

## 18.2 What a drawing call takes

```rust
pub fn draw(ctx: &OpContext, input: &DrawInput) -> Result<OpOutput, OpError>;

pub struct DrawInput {
    pub views: Vec<ViewRequest>,           // one or many: a sheet set is one call (18.9)
    pub reuse: Vec<DrawingRef>,            // earlier drawings of these views, matched by ViewKey: the only cache
}
pub struct ViewRequest { pub key: ViewKey, pub view: ViewDef, pub items: Vec<DrawItem> }

pub struct ViewDef {
    pub frame: FrameId,                    // one frame and grid step for the whole view
    pub projection: Projection,
    pub cut: Option<CutDef>,
    pub bands: Bands,                      // the GridPlanes bounding Overhead, Primary and Beyond
    pub crop: Option<CropRegion>,          // in the picture plane: details, broken-out sections
    pub sheet: SheetPlacement,             // picture plane to sheet; applied at display and export only
}
pub enum Projection {
    Parallel { dir: GridVec3, picture: PicturePlane },  // orthographic if dir is square to the picture, else oblique
    Perspective { eye: GridPoint3, picture: PicturePlane, near: GridPlane },
    Developed { path: PathData },          // unrolled along a path (18.4)
}
pub enum CutDef {
    Plane(GridPlane),                      // an integer plane, as in section 6
    Stepped(Vec<(GridPlane, CropRegion)>), // offset sections: each plane within its strip
    Box(Vec<GridPlane>),                   // up to six planes: a section box
}
```

`draw` has section 10.1's shape, like every operation and query, so section 22 generates its schema and its tool. Its `OpOutput` holds no new bodies, history or names. It carries one result per view, keyed by `ViewKey`: a `Drawing`, or that view's `OpError`, so one failed view does not fail the sheet set (section 10.1, batch first). `reuse` holds earlier drawings of the same views and is the only cache (18.9). As with `reuse` in section 11.6, it may speed a call but never changes its result, and a drawing that does not match its view is ignored.

- **Directions are integer vectors**, each component below 2^40 in size. A grid axis gives plans, elevations and sections on grid lines. A direction that is not already such a vector, such as one square to an inclined face, is rounded to the nearest one and the angle moved (under 2^-39 rad) is recorded with the view, so every visibility decision is exact for the view as recorded.
- **The picture plane** is where the drawing lies. With the direction square to it the view is orthographic; otherwise it is oblique (plan-oblique, elevation-oblique, cavalier, cabinet), and since visibility depends only on the direction, just as exact. Perspective adds a grid eye point and a required near clip plane; a vertical picture plane gives two-point perspective, a tilted one three-point.
- **Drawing coordinates are exact:** rational numbers in an integer basis of the picture plane. On a grid-axis picture plane (plans, ceiling plans, plan-obliques, elevations on grid lines) that basis is the grid itself. Otherwise its vectors have irrational lengths, recorded exactly as squared lengths, and metric numbers are made only for display, export and drafting (18.3).
- **Bands** divide depth by GridPlanes the program supplies: **Overhead** from the top clip up to the cut plane, not including it; **Primary** from the cut plane to the bottom plane, both included; **Beyond** past the bottom plane to the far clip. Curves are split exactly at band planes. A face exactly in the cut plane goes with its body's interior (18.4); anything else exactly on a band plane goes by those intervals. Without a cut, Primary starts at the near clip. Overhead geometry (cabinets above a plan's cut) has visibility among itself and hides nothing below the cut. Spare Construct's view range and a mechanical section's depth map onto the bands.
- **The sheet placement** (position, an exact rotation or mirror, and the scale as an exact ratio) is applied at display and export, so dimensions measure model units and nothing is rounded to place a view.
- **Frames.** A view has one frame and grid step. A finer-frame item, such as a q1 part in a q0 building, promotes the view to the finer step when the view fits within 2^40 of those steps (±6.87 km at q1), which is exact. Otherwise the item is converted down, with the rounding recorded and its class lowered. A view wider than one frame, such as a regional road plan, is split into per-frame sub-views joined by exact lattice shifts, with seams as crop lines; that is exact when no line of sight crosses a seam, as in plans, and any other view across frames returns `InvalidView` with the remedy to put its items in one frame.

```rust
pub struct DrawItem {
    pub element: ElementId,
    pub geometry: ItemGeometry,    // Body | Mesh | Curves3 (model lines, curves on surfaces)
                                   // | Curves2 { band } (symbolic linework) | Marks (points)
    pub placement: ExactTransform, // grid translation, quarter turns, mirrors; exploded views move items here
    pub role: DrawRole,            // Cut | NeverCut | NoOcclude
    pub representation: RepKey,    // the program's choice (coarse, fine, single-line), passed through
    pub material: MaterialKey,
    pub priority: i32,             // which draws on top where faces or lines of different items coincide
    pub patterns: Vec<SurfacePattern>, // model surface patterns on its faces (18.8)
}
```

- **Roles.** `Cut` is the default. `NeverCut` is drawn whole in projection, as programs draw furniture in plan: its parts above the cut count as Primary. `NoOcclude` is drawn with correct visibility and hides nothing.
- **Representations are the program's choice.** Coarse, fine and single-line forms are whichever geometry the program sends, and the key travels into every line's source. Door swings are 2D items in a stated band; cosmetic threads, pitch circles and break lines come from domain toolkits as symbolic items. Everything carries provenance, so no program post-processes the kernel's linework.
- **Fields and point clouds are not items.** A field enters as its certified conversion to a mesh or B-rep (section 16), made by the caller, with its bound. A point cloud is shown through `geom-cloud`'s slab query with the same view; it never cuts, hides or forms regions.

Every kind of drawing is one of these views:

| Drawing | How it is made |
| --- | --- |
| Floor plan; reflected ceiling plan | Parallel, looking down, with a horizontal cut. The ceiling plan looks up, and its sheet placement mirrors it exactly so it lines up with the plan |
| Section, elevation, interior elevation, detail | Parallel with a horizontal direction; a cut for sections; a crop for details |
| Axonometric, plan-oblique, cavalier, cabinet; perspective | Parallel along a diagonal, or oblique to the picture; perspective |
| 3D view with a section box | `CutDef::Box`, with cut regions on every box plane |
| Mechanical orthographic and auxiliary views | Parallel; an auxiliary direction, square to an inclined face, rounded and recorded |
| Full, half, offset, aligned, broken-out, removed and revolved sections | Plane and stepped cuts with crops; removed and revolved sections are cut regions moved by an exact placement |
| Broken (shortened) views; exploded assemblies | Crops with pieces moved by exact translations on the sheet; exact per-item translations |
| Road long sections; elevations of curved walls | `Developed` |
| Road and rail cross sections | Plane cuts at stations (18.4) |
| Circuit board assembly drawings | Parallel along the board normal, in its q1 frame |
| Sun-shadow diagrams | The visible-face map from the sun (18.8) |

## 18.3 The pipeline

| Stage | What it does | Unit of work | Crates |
| --- | --- | --- | --- |
| 1. Prepare | Cut curves and cut regions; candidate curves with their classes; contour generators | One item in one view | `geom-draw`; `geom-intersect` for curved cuts and contours |
| 2. Visibility | Events along each candidate; visible and hidden pieces | One chain of candidates, against occluders found by `geom-spatial` | `geom-draw` |
| 3. Assemble | Merge coincident lines, build cut regions, key every piece, check, sort | One view | `geom-draw`, `geom-2d`, `geom-name` |

The crates follow section 27, with one exception raised as open question 2: `geom-draw` also reads exact meshes from `geom-mesh`, which section 27 reaches only through `geom-spatial`.

**Nothing is rounded to make a drawing.** Points keep the exact form section 6.3 gives them: a grid point where the value is one, otherwise the planes that meet there, by reference to a plane pool kept with the drawing. Curved values are certified, with their bound. Curves stay exact kinds (18.8). That canonical form is what is stored, compared, hashed and queried.

Rounding happens only in two explicit, logged operations: **explode** into section 14 drafting entities, and **export** (DXF, PDF, SVG). Each rounds a point once, by at most half a step, and re-checks the predicates around it, as section 6.3 requires. Where rounding would flip one, the point stays exact if the target can hold it; otherwise the operation snap-rounds and lists every feature that collapsed. Snap rounding is proven to add no crossings, and to move nothing more than about one grid step, for straight segments only (Hobby 1999, [doi](https://doi.org/10.1016/S0925-7721(99)00021-8); [06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 3). So it is applied to straight lines only, and committing arcs and conics waits on section 14 (open question 3). A drawing's topology is never decided from chords.

## 18.4 Cutting

**The cut rule** is Spare Kit's own convention. The cut region of a body is the closure, within the plane, of the body's interior met with the plane: the points of the plane with material on both sides, plus the edges of that set. It applies within the plane the idea of regularised sets from solid modelling (Requicha 1980, [doi](https://doi.org/10.1145/356827.356833)). For flat geometry every case is decided by exact signs; for curved geometry a near-tie is reported (18.6). A named tie corpus covers each case:

- A body that only touches the plane, from either side, is not cut. It is drawn in the band its interior lies in, never as a zero-thickness sliver of cut.
- A face lying in the plane is never a cut; it is drawn in projection, if seen, with its body. A wall standing on a slab exactly at the cut height cuts neither.
- A section through a saddle vertex gives loops that touch at a point, and the region keeps them so.

**Plane-based bodies** (most of a building, every exact mesh, every terrain model) are cut exactly. Each vertex is classified against the plane with section 6.2's exact `classify(point, plane)`. Each crossing edge gives a new point, the meeting of its two face planes and the cut plane, kept implicit, and the loops follow from the topology. Cutting a wall at any height, in any number of views, never drifts.

**Each kind of body** (section 8.1) has a stated rule:

| Body | Cut gives | Hides | Drawn |
| --- | --- | --- | --- |
| Solid | Closed cut regions | Yes | Edges and outlines |
| Sheet (terrain surface, curtain panel, massing face) | Open Cut curves, no region | Yes, from either side | Edges; free boundaries act as outlines |
| Wire | Point marks where it crosses | No | As candidates only |
| Acorn (survey points) | Nothing | No | Named point marks, each visible or hidden by one exact test |
| General and cellular (rooms, wall layers as regions of one body) | A region per body region; a face between two material regions cuts to a line between them | Faces between material and void, and sheet faces | Edges; faces inside material are never seen |
| Tolerant (an import state, section 6.5) | Refused with `ToleranceExceeded` and the remedy "heal first". Drawn only if the caller asks, because its bound is then above ε_fit (section 9). A drawn body reports its Tolerant flag and per-edge tolerances, and the drawing is classed as the body is: Certified(ε), with ε its largest tolerance (section 24.4) | As its kind, if drawn | As its kind, if drawn |
| Approximate | As its kind; the drawing becomes Approximate | As its kind | As its kind |

**Quadrics.** A plane meets a cylinder, cone or sphere in a conic or a pair of lines, found in closed form by restricting the quadric's matrix to the plane, as Krbn does ([quadric.ts L144-L165](https://github.com/vpalos/Krbn/blob/09d96d3d74b50c45c6454c94f5650f56680e0716/src/primitives/quadric.ts#L144-L165)). The result is one of section 7's exact kinds. A plan cut of a vertical cylinder with centre and radius on the grid is an exact grid circle; an oblique cut gives an ellipse with irrational numbers, Certified far below one step.

**Tori.** Planes square to the axis give one or two concentric circles; planes through the axis give two circles of the tube radius. Both are exact, and they are the common plan cuts through horizontal elbows. Any other plane gives a quartic (a spiric section), certified through section 12.

**NURBS, blends and offset surfaces** use section 12's plane-surface intersection, certified within ε_fit and stored as a procedural intersection curve with its cached fit (section 7). **Subdivision surfaces** are cut on their limit surface, which on regular faces is exactly a bicubic B-spline ([09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), section 1), so the NURBS route applies; near extraordinary vertices, see 18.5.

**Mechanical section types** are compositions: stepped sections are plane cuts each clipped to its strip; broken-out sections are cuts limited by a crop; removed and revolved sections are cut regions moved by an exact placement; aligned sections turn one strip into the picture plane, with the turn kept beside the strip and applied at display, like the sheet placement. A section box cuts by up to six planes, with cut regions on each, and must equal drawing the boolean intersection with the box (section 11), which is a property test.

**Civil and developed sections.**

- **Cross sections at a station.** The plane square to an alignment has an irrational normal, so `geom-draw` builds an integer plane: the path's tangent rounded to an integer vector with 40-bit components, through the grid point nearest the station point. The angle moved (under 2^-39 rad, below ε_ang) and the plane's offset from the station point are recorded, and the cut is exact against that recorded plane. The offset is at most (√2/2)q, about 0.71 of a grid step q, for a horizontal normal, and at most (√3/2)q for any other.
- **Developed views** unroll the vertical surface over a path: road long sections, elevations of curved walls. They are a projection kind of their own: a point maps to (the station of its foot on the path, its height), and visibility runs along the path's normals. The path is data, a section 7 curve plus its station map (station equations), which the caller takes from `geom-align`. Developed views are Certified, and exact on straight path segments, which are ordinary parallel views. Stations follow the parameter on clothoids (whose parameter is arc length) and use certified quadrature on other spirals. Inside a curve the Beyond band must stay shallower than the path's smallest radius, where the normals cross; a deeper band is `InvalidView`.
- **Corridor solids** swept along the alignment the view follows (the same carrier, matched by name) are read from the sweep's own parameters, so the vertical profile's lines and parabolic arcs stay exact kinds in (station, height). Vertical exaggeration is a non-uniform scale in the sheet placement, which turns arcs into ellipses (section 7).

**Cut regions.** Each cut region's loops are Cut curves, keyed by element, the body region's name and its material. Poché, the fill of a cut, is a section 14 hatch on the region. Merging same-material regions of joined elements, as wall cleanups do, is a program decision carried out by section 14's 2D union, which keeps each edge's source (iOverlay's `EdgeOverlay` pattern, [06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 2). Overlapping cut regions of different elements are listed in the diagnostics; the items' priority says which draws on top.

## 18.5 Silhouettes

The **contour generator** of a curved face is the curve on it where the line of sight just grazes the surface: the surface normal is square to the line of sight. Its projection is the **apparent outline**, the silhouette a person draws. A **cusp** is where the outline turns back on itself, because the contour generator runs along the line of sight there. Contour generators are computed in `geom-intersect` through the surface trait's evaluation and bounds, so drawings and manufacturing's parting lines (section 10) share one implementation.

- **Plane-based geometry.** Which way a face turns is the exact sign of its normal against the view direction (in perspective, of its plane at the eye). Outline edges follow from coverage (18.6).
- **Quadrics, in closed form.** The contour generator is the quadric's section by the **polar plane** of the eye, or in a parallel view of the view direction's point at infinity (Hartley and Zisserman, *Multiple View Geometry*, 2nd ed., [doi](https://doi.org/10.1017/CBO9780511811685)). This works for every quadric: it gives a cylinder's two tangent rulings and a cone's two generators through the apex. The **dual-conic** route, P·adj(Q)·Pᵀ for the projection matrix P, gives the outline directly but only for spheres and ellipsoids: for a cylinder or cone adj(Q) has rank 1 and the result collapses, which is why Krbn's code returns nothing there ([quadric.ts L167-L189](https://github.com/vpalos/Krbn/blob/09d96d3d74b50c45c6454c94f5650f56680e0716/src/primitives/quadric.ts#L167-L189)) and handles them separately. Looking straight along a cylinder's axis there is no polar plane: the whole surface is edge-on and the outline is its rim circle. In parallel views: a cylinder's outline is two lines at its radius either side of the projected axis (grid lines in an elevation along a grid axis); a sphere's is a circle; a cone's is two lines from the projected apex, tangent to the projected base.
- **Pipes and tori, in closed form.** A pipe is a surface swept by a circle of constant radius r held square to a spine curve: IFC's swept disk solid ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), sections A1 and B), rebar, handrails, pipe and duct bends, and the torus (a pipe round a circular spine). In a parallel view its apparent outline is exactly the two offsets, by r, of the projected spine. Proof: a surface point is X = c(s) + r·n, with n a unit vector square to the spine tangent c′(s), and the surface normal at X is n. X is on the contour when n is also square to the view direction v, so n is parallel to c′ × v, which lies in the picture plane square to the projected tangent; X therefore projects to distance r from the projected spine, along its normal. Where the spine runs along v, the whole circle projects to a circle of radius r. The outline is stored as section 7's offset curve of the projected spine. Where the projected spine bends tighter than 1/r (a pipe seen nearly end-on), the offset has cusps and self-crossings, which are real features of the outline: the offset kind must find them with certified bounds, and hidden-line removal keeps what is seen. The rule covers swept-disk kinds only, and parallel views; perspective uses the general method. A torus seen along its axis gives circles of radius R ± r; seen square to it, two circles of radius r and their two tangent lines.
- **NURBS and other parametric faces, certified.** The silhouette function g(u, v) = (S_u × S_v) · v, or (S_u × S_v) · (S − eye) in perspective, is itself a B-spline in the surface's parameters, formed exactly from the surface (the symbolic method of Elber and Cohen 1990, [doi](https://doi.org/10.1145/97880.97890)). Bernstein bounds on g rule out regions with no contour. Where the bounds show g's gradient stays within a cone of directions, the region holds no singular point and no closed loop, and each branch is traced with a certified bound (the loop tests in [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), section 3). Cusps and singular points solve small polynomial systems built the same exact way and are isolated by subdivision.
- **Subdivision surfaces.** On regular faces the limit surface is exactly a bicubic B-spline, so the same method applies. Near an extraordinary vertex (where other than four faces meet) the limit surface is an infinite sequence of rings that shrink geometrically, each a bicubic patch by Stam's exact evaluation (Stam 1998, [doi](https://doi.org/10.1145/280814.280945)). The silhouette function is certified ring by ring with interval bounds, plus a bound on all remaining rings, until that remainder is smaller than ε; any contour that may pass through it is reported with its place and bound. If the budget runs out first, the result is `CertificationFailed`. ConTesse (Liu, Bénard, Hertzmann and Hoshyari 2023, [doi](https://doi.org/10.1145/3544778)) states when a *sampled* contour can be given consistent visibility. Its conditions concern samples, so they are not a certificate; Spare Kit checks its output against them as a consistency test, and a failure is a bug.
- **Tangent edges seen edge-on** (a fillet's boundary along the line of sight) carry the outline, and that part of the edge is drawn as Outline.

## 18.6 Hidden lines

**The method, in our own words.** Spare Kit follows Appel's quantitative invisibility (Appel 1967, [doi](https://doi.org/10.1145/800196.806007)). Every point of a projected curve has a **count**: the number of times the line of sight from it toward the viewer leaves a solid's material (through a face turned toward the viewer) or crosses a sheet. It is visible where the count is zero; a point inside another body counts that body once. The count changes only where the curve passes behind an outline or free boundary in front of it, where it passes through a face, and at its own vertices and cusps (Appel; Loutrel 1970, [doi](https://doi.org/10.1109/T-C.1970.222898)). We keep OpenCascade's organisation, each candidate tested against the faces that could hide it with bounding boxes rejecting the rest ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 6), and none of its tolerances or silent skips.

1. **Candidates:** cut lines, model edges, contour generators, intersection lines (below), caller-supplied curves, and on request isoparametric lines and axes. Anything on the removed side of the cut, outside the Overhead band, is dropped.
2. **Occluders:** faces in the Primary and Beyond bands and the kept parts of cut bodies, found through `geom-spatial`'s bounding-volume hierarchy built in view space. Overhead faces hide only Overhead lines.
3. **Events:** every crossing behind an outline or free boundary in front, every piercing of a face, every cusp, and every meeting with another visible line. They are ordered exactly for flat geometry and certified for curved. Events at one exact point merge.
4. **Counts:** a chain is one candidate, or several joined end to end at shared vertices. Its starting count comes from one exact test (certified for curved geometry), then changes at each event by a step decided from exact signs: at an edge, the sum over its counting faces (faces of solids turned toward the viewer, and sheet faces) of +1 for a face covering the far side of the crossing and −1 for one covering the near side. **Every build re-tests the count directly at each chain's end.** A mismatch is a bug: the view returns `Internal` with the chain, both counts and what is needed to reproduce it, never a drawing, as in sections 11.11 and 12.8. Debug and CI builds test every piece directly, as Krbn always does.
5. **Coincident lines:** where two candidates overlap in projection (a wall's top and bottom edges in plan), the overlap is found exactly for flat geometry, since both lie in one plane containing the view direction. One drawing curve results: visible beats hidden, then the class earlier in 18.7's table, then the nearer depth, then the higher item priority. Every source is kept.
6. **Junctions:** the end of a partly hidden line is the exact projection of the meeting point, which lies exactly on the line that hides it. Lines never stop short or overshoot, the faults HL's own check hunts for ([README L162](https://github.com/kenny-t-vo/hidden-lines/blob/4d9570ee7a9d0b6a4ed2ee5c365a637797daaadf/README.md?plain=1#L162)).

**Degenerate cases are the normal case in grid-axis drawings, and each has a rule:**

- A face seen exactly edge-on (every wall face in a plan) covers no area and hides nothing, so an edge between a face turned toward the viewer and an edge-on face acts as an outline.
- A point exactly on a face's projected boundary is not covered by that face alone.
- A candidate crossing exactly behind an outline vertex, a piercing exactly at a face's edge, and a starting test whose line of sight passes exactly through an edge or vertex are resolved by perturbing the line of sight symbolically (Simulation of Simplicity, Edelsbrunner and Mücke 1990, [doi](https://doi.org/10.1145/77635.77639)), ordered by persistent names as section 6.2 requires. The total change across a vertex does not depend on the perturbation; only which side records it does.
- A candidate running along another line in projection goes to the coincident-line rule.
- A curve is never hidden by a face it lies on: its own faces, known from topology, and any face exactly in the same plane or on the same carrier (section 20.4). Coincident faces of different items do not hide each other; the items' priority says which draws on top.

**Bodies that run into each other**, a pipe through an unjoined wall or a beam into a column, are everywhere in BIM models, and Appel's method assumes they do not happen. Spare Kit adds:

- **Piercing events.** Where a candidate passes through another body's face, its count changes by one with no outline crossing. For flat geometry the point is the meeting of the candidate's two face planes and the face's plane, exact; for curved geometry it comes from `geom-intersect`, certified.
- **Intersection lines.** The seam where two bodies meet is an edge of neither, yet must be drawn. It is computed for every pair of bodies of different items whose 3D boxes overlap, found through `geom-spatial` as clash detection finds them: exact for flat geometry, certified for curved, with both sources, in its own class. Krbn draws such seams too ([README L124-L125](https://github.com/vpalos/Krbn/blob/09d96d3d74b50c45c6454c94f5650f56680e0716/README.md?plain=1#L124-L125)). Every overlapping pair is also listed in the diagnostics, as HL's checker lists them.

**View predicates for flat geometry.** A parallel view adds one derived plane per outline edge: the plane through the edge that contains the view direction. It is the combination (n₂·v)P₁ − (n₁·v)P₂ of the edge's face planes, so it is exact and integer. An event is the meeting of the candidate's two face planes with a derived plane, kept implicit, and every decision is the sign of an integer expression. Extending [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md)'s bit-budget table (section 4) to coordinates below 2^B: a derived plane needs about 4B + 7 bits for its normal and 5B + 9 for its offset along a grid axis, and 5B + 10 and 6B + 12 for any direction with components below 2^B. The deepest test, ordering two events along one edge, needs about 13B + 27 bits along a grid axis and 15B + 33 in general: about 547 and 633 bits at B = 40. Perspective uses the eye point the same way, at the general width. These are wider than section 6.2's 384 bits, behind the same generated `f64` filters, so the wide path runs only near degenerate cases. Each budget must be written out and tested at the extremes as section 6.2 requires (open question 1).

**Curved geometry, certified.** Events are certified parameter intervals, refined until they separate or are narrower than the bound ε. Two events that cannot be separated within ε are a near-tangency, reported with its place; the sliver between them is below the bound. The promise, Certified(ε), has four parts:

1. Every drawn point lies within ε of the true projected curve.
2. Every switch between visible and hidden lies within ε of the true switch.
3. Every point farther than ε from a switch has the correct visibility, wherever its depth differs from every surface over it by more than ε.
4. Every point within ε in depth of a surface it is not known, by topology or carrier, to lie on is listed as a depth near-coincidence, with its place and gap. A face within ε of tangency to the cut plane is listed as a cut near-tangency.

Certified arithmetic cannot decide that two depths are equal, so equality comes from topology and carriers, never from a tolerance. ε defaults to ε_fit (0.01 mm at q0, 0.4 µm at q1). For a preview the caller passes a coarser ε in model units through the operation context (section 10.1), and the result says so.

**What the kernel never does:** decide visibility by sampling, pixels or ray probes; compare depths with a tolerance; skip a face or an edge-face pair after a failure; use GPU results, which are not bit-identical across vendors ([10](../../../kit/notes/G01-evidence/10-github-sweep.md), section 1).

## 18.7 Line classes

| Class | What it draws | Decided by |
| --- | --- | --- |
| Cut | The boundary of a cut region; the open cut of a sheet | 18.4 |
| Outline | Outline edges of flat geometry; contour generators; tangent edges seen edge-on | 18.5, 18.6 |
| Boundary | Free edges of sheets; wires | Topology |
| Intersection | Seams where bodies of different items meet | 18.6 |
| Sharp | An edge where faces meet at an angle | The edge's continuity record |
| Smooth | An edge where faces meet tangent (G1) but curvature jumps: fillet and tangent edges | The continuity record |
| Sewn | An edge the surface continues across with G2 or better; a periodic surface's seam; an edge between coplanar faces | The continuity record; exact coplanarity |
| Iso | Isoparametric lines of curved faces, on request, as OpenCascade offers them ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 6) | Surface parameters |
| Axis | Centre lines of revolved faces; spines of sweeps, for single-line pipes, ducts and rebar | Carriers (section 20.4) |
| Supplied | The caller's curves: model lines, curves on surfaces, panel division lines, draped breaklines, symbols; and model surface patterns, laid by the kernel from the caller's pattern (18.8) | The item |

The table's order is the priority when lines coincide. Every line also carries:

- **Visibility:** Visible, or Hidden with its count (a `u32` that saturates with a diagnostic rather than overflowing).
- **Band:** Overhead, Primary or Beyond.
- **Depth:** nearest and farthest signed depth along the view axis, from the cut plane or, in perspective, from the eye, kept exact as (p − o)·dir: the true depth times the direction's length, an integer or a rational. It becomes millimetres only for display, for weights by distance as HL draws them.
- **Ends:** free, at a junction with a named line, or at the crop.
- **What lies behind, on request:** for a visible outline, what is directly behind it on each side (nothing, or which face and how far), from which programs weight profiles, bases and overlaps as HL's vocabulary does.

**Continuity is recorded, never guessed by a threshold.** Each edge keeps a record per interval of its parameter, since a blend can run out to tangency along one edge, saying how it was decided:

- **Built:** the operation that made the edge records what it built. A fillet's edges are G1; a G2 blend's are G2 (section 13).
- **From the source:** an import reads the source's own smoothing data where it exists, such as the optional vertex normals of IFC triangulated face sets ([IfcTriangulatedFaceSet.md L22](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcTriangulatedFaceSet.md?plain=1#L22)); shared normals across an edge mark it Smooth.
- **Measured:** otherwise a certified test against ε_ang, with three outcomes: proved, disproved, undecided. Undecided intervals draw Sharp, with a diagnostic.
- **Marked:** an imported STEP fillet is rarely tangent within ε_ang (about 5.8 × 10^-11 rad), and a faceted IFC element's triangle edges are not coplanar, so without more information they draw Sharp, which is what the data says. For such imports an explicit, logged operation marks edges Smooth where faces turn by less than an angle θ, recorded as a parameter of that operation: a decision in the log, not a hidden constant (open question 4). The record and this attribute edit live in `geom-topo`.

Programs choose which classes to show; the kernel returns what it was asked for. Building plans and sections use every class and band. Mechanical drawings use Visible and Hidden, Smooth for tangent edges and Axis for centre lines. Class-A drawings use Outline, Sharp and Smooth as the body's character and feature lines; Sewn edges are patch boundaries, usually left out of presentation drawings and kept for surface checking. Civil sections use cuts through terrain and corridor solids; board assembly drawings use component outlines and their visibility over the board.

## 18.8 Drawing curves, names and references

```rust
pub struct DrawingCurve {
    pub geom: Curve2,                // an exact 2D kind in view coordinates, with its bound
    pub class: LineClass,            // 18.7
    pub vis: Visibility,             // Visible | Hidden { count: u32 }
    pub band: Band,                  // Overhead | Primary | Beyond
    pub depth: DepthRange,           // exact, along the view axis
    pub sources: SortedVec<Source>,  // more than one where lines coincide
    pub piece: DrawPieceKey,         // derived: the source plus the occluders that bound this piece
    pub exactness: Exactness,
}
pub struct Source {
    pub element: ElementId,
    pub entity: NameId,              // the edge, or the face for cuts and contours
    pub what: SourceKind,            // EdgeSpan { t0, t1 } | CutOf | Contour { branch } | Intersection { with }
                                     // | Iso { dir, value } | Axis | Supplied { index } | Pattern { family, index }
}
```

**Which curves keep an exact kind.** A projection keeps a curve's kind where it maps the kind onto itself exactly: lines and conics in every projection (an oblique circle becomes an ellipse whose axes may be irrational: Certified, far below a step); rational NURBS, whose control points a parallel projection maps and whose homogeneous control points a perspective maps to another rational NURBS; and a plane curve seen square-on, so a horizontal clothoid in plan is that clothoid, an alignment's 3D curve in plan is its horizontal base curve, and a helix seen along its axis is a circle. Everything else has no 2D kind in section 7: a helix seen from the side, an alignment in an oblique view, procedural intersection curves, 3D offset curves, developed-view curves. Those are stored as section 7's NURBS conversion with its certified error, marked Certified(ε), with the source curve and parameter range kept in the provenance. A procedural 2D "projected curve" kind would keep them exact by definition; a new kind needs a decision note (section 26.1), so this is open question 7. Drawing curves are never flattened; tessellation is for display only.

**Pieces and their keys.** A piece is a span of one source between two events. Its key, a `DrawPieceKey` (not section 20.2's `PieceKey`, which names split entities), is the source's name plus the names of the occluders that bound it: "edge E between where it passes behind face F and face G". A silhouette has no 3D edge, so its source is (face, Contour, branch key); the branch key is the side of the projected axis for surfaces of revolution and pipes, which is stable, and otherwise an ordinal in the face's parameter space, marked weak, as in section 20.2. Keys are derived values, for display, diffing and questions such as "what hides this line?". Nothing binds to them: a drawing is regenerated, not edited, and has no history leading from one regeneration to the next.

**What binds to a drawing is the model, through names:**

```rust
pub struct LineRef { pub element: ElementId, pub source: NameId, pub class: LineClass, pub span: Option<ParamSpan> }
```

- **Overrides** ("hide this line", "draw it thin") store a `LineRef` and apply to every piece drawn from that source in that class (section 20.4's `AllPieces`), whatever hides it this time. A span limits one to part of the source; if the source has since been modified, the override reports `WeakChanged` ([07](../../../kit/notes/G01-evidence/07-persistent-naming.md), section 8) for a person or AI to confirm, and is never moved by a guess.
- **Dimensions** store, per point, the model reference of section 20.4 (a carrier by default), the view, a snap kind from DIMASSOC's thirteen ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), sections 1 and 4) and a parameter. **Model snaps** (endpoint, midpoint, centre, perpendicular, nearest and the rest) come from the exact 3D geometry, projected, which is exact in grid-axis views. **View snaps** have no 3D point: an apparent intersection, the quadrant of an oblique circle, tangency to an outline. They are found on the drawing's exact curves, record both sources and take those curves' class; in perspective every snap is a view snap. Carrier references keep a dimension to "the exterior face of the wall" valid when a door splits the face. A reference outside the view returns `UnresolvedRef` with `NotInView`; one to a hidden line resolves with a warning. Nothing is found by index or by searching for similar geometry, the failure TechDraw shows. `geom-draw` resolves each anchor to an exact point; the dimension entity itself is section 14's.
- **Cut regions** carry the element, body region and material key, with loops of Cut curves.

**The visible-face map**, on request, gives the visible part of every face as regions: Weiler and Atherton's object-space result (1977, [doi](https://doi.org/10.1145/563858.563896)), built here from the drawing itself. The visible lines divide the picture into cells, and each cell is labelled with the face seen through it by one exact test at a point inside it (certified for curved faces). Programs use it for drafting patterns and colour fills on faces (section 14.6 hatches on its regions), and for answering "what is at this point" for people and AI. Model surface patterns, laid at true size on the faces themselves, are below.

**Shadows.** A sun-shadow diagram is the visible-face map seen from the sun: the part of a face the sun sees is lit, and the rest of every sun-facing face is in shadow (Atherton, Weiler and Greenberg 1978, [doi](https://doi.org/10.1145/800248.807403)). The sun direction is rounded to an integer vector and recorded, so shadows of flat geometry are exact; the shadow regions are then drawn in any view.

**Model surface patterns.** Brick coursing, tiles, stone joints and cladding lines are patterns of real size on model faces. They are drawn in every view, seen or hidden like any other line. Which pattern a material uses is the program's view rule; the kernel lays the lines, with provenance, as section 10.2's Draw family promises.

```rust
pub struct SurfacePattern {
    pub faces: Vec<ModelRef>,     // section 20.4 references; a carrier reference covers every face on that carrier
    pub anchor: PatternAnchor,    // a section 20.4 reference, an exact offset and an integer direction in the face
    pub pattern: PatternDef,      // section 14.6's line families and dashes, in model units
    pub map: Option<FlatMapRef>,  // curved faces only: the flat map the pattern is laid in (section 16.4)
}
```

- **Anchored by name.** The pattern's frame is its anchor: a carrier by default, or a named edge, vertex or datum such as a level, moved by an exact offset and turned to an integer direction lying in the face. The frame moves with what it is anchored to, never with the face's outline, so a door cut into a wall, or a longer wall, shifts no course.
- **Planar faces are exact.** A family's lines are where the face's plane meets a family of parallel integer planes, kept implicit as cut lines are (18.4). The planes are spaced exactly when the length of their integer normal is a whole number, as it is for families along the grid axes. Otherwise each plane's offset is rounded to the nearest whole number on its own, so each line moves at most half a step and nothing builds up along a family; the pattern is then Certified at that bound.
- **Continuous across neighbours.** A pattern anchored to a carrier is laid on the carrier, not on one face. Faces on one carrier, such as a wall face split by a door or two joined walls in one plane, show one pattern: a line's spans on neighbouring faces meet exactly at their shared edge, and the drawing joins them into one line, keeping every source. Faces on different carriers anchored to one datum with the same spacing, such as the two faces at a wall corner, have their courses on the same planes, so the courses meet exactly on the corner edge.
- **Curved faces go through a flat map.** The pattern is laid in a flat map of the face and carried onto it. Developable faces (cylinders, cones, extrusions) use their exact flat maps from `geom-surface`, so courses on a round wall keep their true size, and horizontal courses on a vertical round wall are circles, an exact kind. Other faces use a section 16.4 map, made and checked there and passed in as data, as alignment paths are. Lines are carried into the face's parameter space and evaluated on the surface, so they lie on the face within a certified bound, and the map's distortion is reported with them, as section 16.4 reports each tile's.
- **Clipped exactly to the face.** Lines are clipped to the face's loops, holes included: by exact signs where the loops are plane-based, and with certified crossings in the face's parameter space otherwise (section 12). Dashes are placed by certified arc length, and lines are counted before any are made, with `BudgetExceeded` and a scale that fits when there are too many, as in section 14.6.
- **Drawn like any candidate.** Each line enters hidden-line removal (18.6) as a candidate that its own face never hides and other bodies do. It is drawn in the Supplied class, and its source is the face, with `Pattern { family, index }`.
- **Names that survive edits.** A pattern line is named by the pattern's anchor, its family and its signed index from the anchor, as section 16.4 names grid lines: the twelfth course above the level stays the twelfth course when a door splits the face or the wall grows. The name does not depend on which faces the line crosses.
- **Exactness.** Exact on plane-based faces whose planes are spaced exactly; otherwise Certified, at the rounding or map bound. Pattern lines belong to the drawing, regenerated with it and never stored in the body, like section 14.6's hatch lines.

## 18.9 Caching, incremental update, tiles and parallel work

- **Earlier drawings are the only cache.** `draw` is pure (principle 7). A `Drawing` keeps its stage 1 results keyed by (body content hash, item placement and role, view hash), and a new call reuses what the drawings in `reuse` hold under the same key. There is no global or session cache; memoising across many views belongs to the caller's incremental engine over these pure calls, in Salsa's style ([07](../../../kit/notes/G01-evidence/07-persistent-naming.md), section 7).
- **Instances share work without changing a bit.** Stage 1 is always computed in the item's own frame, on the full and incremental paths alike, and its curves are stored with the exact placement beside them. Identical bodies at grid translations share one result in a parallel view when the translation lies along the cut plane, or there is no cut.
- **A change is local.** A changed item's stage 1 is recomputed. Visibility is recomputed only for chains whose projected boxes overlap the item's old or new footprint in view space. If the new stage 1 output equals the old, nothing downstream runs (Salsa's early cutoff). Region merges are redone only for material groups that touch the footprint. The incremental result must equal a full regeneration bit for bit, which is tested. Updating every open plan within Spare Construct's one-frame target when a door moves is a target for section 23 to measure, not a claim made here.
- **Tiles index the result.** Fixed, power-of-two square tiles of the picture plane in grid units, independent of thread count, sized by a named constant in `geom-num`, serve display streaming, invalidation, and views too large for memory, such as a 50 km corridor plan. A curve is never split at a tile border; it is listed in every tile it crosses. Tile size never changes a drawing's content, so it can be tuned freely.
- **Parallel work** uses section 21's shared helper: stage 1 by item and stage 2 by chain, in fixed chunks ordered by name, results collected in order.
- **Batch first.** A sheet set of many views is one call, with one result per view, keyed by `ViewKey` (18.2). A view that fails returns its own error, and the others still return.

## 18.10 Storage, versions and checking

- **Drawing is a versioned operation** under the kernel edition (section 26.2): within an edition, the same model and view give the same drawing, bit for bit.
- **Sheets store their drawings** in canonical form in the native format (section 24), the same form the replay corpus hashes, and open from it without recomputing. A model saved in 2027 shows the same lines in 2077.
- **A better algorithm never moves old lines silently.** A new edition changes an old document's drawings only through the explicit upgrade operation, which reports every line that moved or changed visibility, sheet by sheet.
- **Every drawing passes a checker** before it is returned, in groups like section 8.3's: (1) cut-region loops close and do not cross, and every drawing curve has a source and a piece key; (2) each source's visible, hidden and cropped pieces cover its projection exactly once; (3) no two drawing curves overlap after merging; (4) exactness records agree with the inputs. Groups 1, 3 and 4 and the per-chain count re-test run in every build; group 2 runs in full in debug builds, CI and on request. The checker lives in `geom-draw`, because `geom-check` sits below it in section 27 and cannot see drawing types.

## 18.11 Exactness, errors and determinism

| What is drawn | Cuts | Outlines | Visibility | Drawing class |
| --- | --- | --- | --- | --- |
| Plane-based solids and sheets, exact meshes, terrain | Exact | Exact | Exact, with 18.6's wider view predicates | Exact |
| Quadrics | Closed-form conics, exact on the grid when their numbers are | Closed form | Certified; exact where every curve involved is a grid line | Certified, usually far below ε_fit |
| Pipes and tori | Exact circles in the special planes; certified quartics otherwise | Closed-form offsets with certified evaluation | Certified | Certified |
| NURBS, blends, offsets, regular subdivision faces | Certified | Certified | Certified | Certified(ε) |
| Subdivision near extraordinary vertices | Certified | Certified ring by ring, or `CertificationFailed` | Certified | Certified(ε) |
| Fields | Through their conversion | Through their conversion | Certified | Certified, bounds added |
| Developed views | Certified; exact on straight path segments | Certified | Certified | Certified |
| Tolerant bodies | Refused, or drawn on request | On request | On request | Certified(ε), with ε the body's largest tolerance (section 24.4) |
| Model surface patterns (18.8) | Not cut; clipped to their faces | — | As for any candidate | Exact on plane-based faces whose planes are spaced exactly; otherwise Certified |

A drawing's class is the least exact of what it draws, and bounds add up (section 9). Sheet rotations and the metric scaling of an off-axis picture plane happen at display and export, outside the canonical form.

**Errors** follow section 22. Each view succeeds or fails whole, so there is never a partial drawing: `InvalidView` (a zero or oversized direction, a perspective with no near clip, an invalid crop, a developed band deeper than the path's radius, a view across frames whose lines of sight cross a seam); `ToleranceExceeded` for a Tolerant body, with the remedies "heal first" or "draw it at its own tolerance"; `BudgetExceeded` and `Cancelled`, which publish nothing; `CertificationFailed`, naming where and the best bound reached, with remedies: raise the budget, accept that bound explicitly, or draw this body from its certified tessellation (section 19), a result marked Approximate because visibility near tangencies is then not proven; and `Internal` for a count re-test mismatch (18.6) or any other broken invariant, with what is needed to reproduce it. **Diagnostics** list near-tangent events, depth near-coincidences, cut near-tangencies, overlapping bodies and cut regions, weak branch keys and saturated counts.

**Determinism** follows section 21. Events order by exact signs, then by names; output sorts by band, class, source name and piece key; every drawing's canonical form is hashed in the replay corpus on every target and thread count. Section 28 adds outside oracles run out of process, never linked ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section D): OpenCascade's HLR, which section 4 already allows; a test-only raster sampler of our own; and Krbn for quadric scenes. Property tests and fuzz targets are listed with each build piece (section 30).

## 18.12 What is still open, and what Spare Kit promises meanwhile

- **Certified contour topology for free-form surfaces.** No open method we found finds every cusp and contour crossing on a NURBS surface with guaranteed topology in every degenerate case; ConTesse's authors show earlier methods did not guarantee the conditions for consistent visibility. Spare Kit promises Certified(ε) as defined in 18.6, reports every event it could not separate, and never claims more.
- **Contours near extraordinary vertices** of subdivision surfaces have no published certified method that we found (the search was not exhaustive). The ring-by-ring certification in 18.5 is our own design and must be proved by tests against Stam's evaluation at high precision before it ships; until then such regions return `CertificationFailed` with their place.
- **The wide view predicates** need their bit budgets written out and their cost measured on WebAssembly, which has no native 128-bit multiply ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), open question 7).
