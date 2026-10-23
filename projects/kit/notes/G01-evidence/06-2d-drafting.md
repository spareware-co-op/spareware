# Spare Kit geometry kernel: 2D drafting, 2D booleans and offsets, sketch solving

*Evidence for [G01](../G01-geometry-kernel.md), the Spare Kit geometry kernel decision note. Written 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of each project at the pinned commits linked below. GPL and LGPL code is described in the agent's own words, never copied. Claims it could not confirm from a primary source are marked unverified.*

All repositories were read at pinned commits:

| Repo | Commit |
|---|---|
| Clipper2 | `a6e7294` |
| iOverlay | `117637e` |
| cavalier_contours | `22887a8` |
| CGAL | `cbc3b81` |
| LibreCAD | `2af4873` |
| SolveSpace | `4707fa2` |
| FreeCAD | `9946038` |
| ezpz | `915882c` |
| fiksi | `ff48505` |
| JTS | `5a0a736` |
| OCCT | `53706f7` |
| clipper2-rust | `e8a250a` |
| iCurve | `37b5ee8` |

Every DXF citation points to Autodesk's public 2018 DXF reference (static `cloudhelp` pages).

---

## 1. The 2D entity model a full drafting system needs

**Curve primitives**

- **Line and its infinite forms.** XLINE is stored as a point plus a unit direction ([XLINE](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-55080553-34B6-40AA-9EE2-3F3A3A2A5C0A.htm)); RAY is the half-infinite version ([RAY](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-638B9F01-5D86-408E-A2DE-FA5D6ADBD415.htm)). The kernel needs these as true unbounded carriers. EXTEND, apparent intersection and construction geometry all intersect against them.
- **Circle and arc.** Stored as centre, radius, and start/end angle, counter-clockwise in the entity's own coordinate system (OCS) ([ARC](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-0B14D8F1-0EBA-44BF-9108-57D8CE614BC8.htm)).
- **Ellipse and elliptical arc.** Stored as centre, a major-axis *vector* relative to the centre, a minor/major ratio, and start/end *parameters*. These are parameters, not angles: 0 and 2π for a full ellipse ([ELLIPSE](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-107CB04F-AD4D-4D2F-8EC9-AC90888063AB.htm)).
  - The kernel must keep parameter and polar angle as distinct types.
  - The offset of an ellipse is not a conic; offsets in general are higher-degree algebraic curves ([Farouki & Neff 1990](https://doi.org/10.1016/0167-8396(90)90023-K)). So "offset ellipse" must produce a NURBS approximation within a stated tolerance.
- **Lightweight polyline.** Per-vertex x/y, per-vertex start/end width (codes 40/41), or a constant width (43), plus a per-vertex bulge (42) and a closed flag ([LWPOLYLINE](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-748FC305-F3F2-4F74-825A-61F04D757A50.htm)).
  - Bulge is the ratio of sagitta to half the chord. Its sign gives the direction: positive is counter-clockwise ([ezdxf docs, MIT](https://github.com/mozman/ezdxf/blob/7b202613e0d689955b8932a22bcbcf68bdd684f0/docs/source/dxfentities/lwpolyline.rst#L25-L39)).
  - Bulge equals tan(sweep/4), so an arc segment is fully defined by two endpoints and one scalar. That makes it the natural exact arc encoding for polylines.
- **Spline.** Flags for closed, periodic, rational, planar and linear; degree, knots and control points; optional weights; *and* optional fit points with start/end tangents and a fit tolerance ([SPLINE](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-E1F884F8-AA90-4864-A215-3182D47A9C74.htm)).
  - Recommendation: the canonical geometry is NURBS (control points, knots, weights). Fit data is kept as *creation intent*. The fit-to-control conversion must be a versioned, deterministic algorithm.

**Area entities**

- **Hatch** ([HATCH](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-C6C71CED-CE0F-4184-82A5-07AD6241F15B.htm)):
  - A solid-fill flag and an associativity flag.
  - An island rule (code 75): odd parity, outermost only, or ignore islands.
  - Pattern type: user-defined, predefined or custom; plus angle, scale and a "double" flag.
  - Seed points.
  - Gradient fields (450–470).
  - Boundary loops ([Boundary Path Data](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-DC5215D6-E73F-4DFF-8BE9-01CA9610FAEE.htm)): each loop carries flags (external, polyline, derived, textbox, outermost). A loop is *either* a bulge polyline *or* a list of edges, where an edge is a line, a circular arc, an elliptic arc, or a spline. Each loop also lists references to its source boundary objects (codes 97/330); this is what makes a hatch associative.
  - Pattern lines ([Pattern Data](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-7C05C0EC-B0FB-4A86-A164-B9E5C6C03990.htm)): each has an angle, a base point, an offset vector, and dash lengths. So a pattern is an infinite family of parallel dashed lines, clipped to the region.
- **Region.** In DXF this is opaque proprietary modeler data ([REGION](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-644BF0F0-FD79-4C5E-AD5A-0053FCC5A5A4.htm)). Spare Kit must define its own Region: oriented outer loops with nested holes, using the same loop type as hatch boundaries.

**Text**

- **TEXT** has a first and a second alignment point, plus a 72/73 justification matrix (left/centre/right/aligned/middle/fit × baseline/bottom/middle/top) ([TEXT](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-62E5383D-8A14-47B4-BFC4-35824CAE8363.htm)).
- **MTEXT** has an insertion point, attachment points 1–9, a reference width, an x-direction vector, line spacing, and columns ([MTEXT](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-5E5DB93B-F8D3-4433-ADF7-E92E250D2BAB.htm)).
- The kernel stores anchor, attachment and direction. Glyph layout lives outside the kernel, but it must return an extents box. That box is needed for hatch "textbox" islands (loop flag 8).

**Dimensions**

- **Common fields** ([Common Dimension Group Codes](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-EDD54EAC-A339-4EBA-AEA6-EC8066505E2B.htm)):
  - The name of an anonymous block that holds the rendered dimension picture.
  - A definition point and the text midpoint.
  - Type values 0–6: rotated, aligned, angular, diameter, radius, angular 3-point, ordinate. Bit flags: 32 (block used only by this dimension), 64 (ordinate is X-type), 128 (text placed by the user).
  - A read-only measured value (42).
  - User text, where `<>` stands for the measured value.
  - The dimension style.
- **Per-type fields:** definition points 13–16 for aligned, angular and ordinate ([angular](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-09821B78-9F8E-43BA-82F2-8C931485EDC9.htm), [ordinate](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-72F01288-0D63-43E8-8179-8CE3BA544C40.htm)).
- **Arc-length dimension:** ARC_DIMENSION is *not* in the DXF reference ([ezdxf note](https://github.com/mozman/ezdxf/blob/7b202613e0d689955b8932a22bcbcf68bdd684f0/docs/source/dxfentities/arcdim.rst#L7-L8)).
- **Associativity** lives in a separate DIMASSOC object ([DIMASSOC](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-C0B96256-A911-4B4D-85E6-EB4AF2C91E27.htm)):
  - A bitmask saying which of up to four points are associated.
  - A "trans-space" flag: paper-space dimension on model geometry.
  - For each point: an object-snap type (0–13: endpoint, mid, centre, node, quadrant, intersection, insertion, perpendicular, tangent, nearest, apparent intersection, parallel, start); the main object ID with its sub-entity type and index; an optional second object for intersections; a parameter for the "nearest" snap; and the cached snap point.
  - Takeaway: associativity is a **typed snap reference**, made of (entity, snap kind, parameter, optional second entity). The dimension picture is derived data and is regenerated.

**Leaders, blocks, viewports**

- **Leader:** vertices, straight or spline path, arrowhead flag, hookline, and a hard reference to its annotation (code 340) ([LEADER](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-396B2369-F89F-47D7-8223-8B7FB794F9F3.htm), [MLEADER](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-72D20B8C-0F5E-4993-BEB7-0FCF94F32BE0.htm)).
- **Block insert:** block name, insertion point, x/y/z scale, rotation, and a rows × columns array with spacing ([INSERT](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-28FA4CFB-9D5E-4880-9F11-36C97578252F.htm)). Nested inserts compose as affine transforms. Two consequences:
  - Non-uniform scale turns circles and arcs into ellipses and elliptical arcs, so **transforming an entity can change its type**.
  - Negative scale (mirror) reverses arc orientation.
- **Viewport:** paper-space centre, width and height; view centre in display coordinates; view direction, target and twist; view height (drawing scale = paper height ÷ view height); front/back clip planes; frozen layers; and an optional clipping entity for non-rectangular viewports (340) ([VIEWPORT](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-2602B0FB-02E4-4B9A-B03C-B1D904753D34.htm)).

**Coordinate frames.** Recommendation: kernel 2D entities live in plain 2D coordinates of a sketch, view or sheet frame plus one placement. The DXF OCS / extrusion convention is handled only at the import/export boundary.

---

## 2. 2D booleans and offsets

**Clipper2** (Boost licence, [LICENSE](https://github.com/AngusJohnson/Clipper2/blob/a6e7294703ae329576462f9471b356971f53010d/LICENSE)), an extension of [Vatti 1992](https://doi.org/10.1145/129902.129906).

- **Scanline loop** ([engine.cpp L2118-2145](https://github.com/AngusJohnson/Clipper2/blob/a6e7294703ae329576462f9471b356971f53010d/CPP/Clipper2Lib/src/clipper.engine.cpp#L2118-L2145)): pop the next scanline; insert local minima into the active edge list; process horizontals; compute intersections inside the scanbeam; do the top-of-scanbeam work.
- **Coordinates and predicates:**
  - Coordinates are 64-bit integers, limited to `MAX_COORD = INT64_MAX>>2` ([core.h L65-69](https://github.com/AngusJohnson/Clipper2/blob/a6e7294703ae329576462f9471b356971f53010d/CPP/Clipper2Lib/include/clipper2/clipper.core.h#L65-L69)).
  - The orientation test is **exact**, using 128-bit products ([CrossProductSign L751-785](https://github.com/AngusJohnson/Clipper2/blob/a6e7294703ae329576462f9471b356971f53010d/CPP/Clipper2Lib/include/clipper2/clipper.core.h#L751-L785)).
  - Intersection *points*, however, are computed in double precision and truncated ([L949-973](https://github.com/AngusJohnson/Clipper2/blob/a6e7294703ae329576462f9471b356971f53010d/CPP/Clipper2Lib/include/clipper2/clipper.core.h#L949-L973)). They are then patched when rounding pushes them outside the scanbeam ([AddNewIntersectNode L2345-2377](https://github.com/AngusJohnson/Clipper2/blob/a6e7294703ae329576462f9471b356971f53010d/CPP/Clipper2Lib/src/clipper.engine.cpp#L2345-L2377)).
  - So predicates are exact but constructions are rounded. There is no formal snap-rounding guarantee; *my inference:* near-degenerate inputs can shift topology slightly.
- **Offsetting:**
  - Join types Square, Bevel, Round, Miter; end types Polygon, Joined, Butt, Square, Round ([offset.h L19-26](https://github.com/AngusJohnson/Clipper2/blob/a6e7294703ae329576462f9471b356971f53010d/CPP/Clipper2Lib/include/clipper2/clipper.offset.h#L19-L26)).
  - Round joins are **flattened** using an arc tolerance that defaults to delta × 0.002 ([offset.cpp L29](https://github.com/AngusJohnson/Clipper2/blob/a6e7294703ae329576462f9471b356971f53010d/CPP/Clipper2Lib/src/clipper.offset.cpp#L29), [L273-297](https://github.com/AngusJohnson/Clipper2/blob/a6e7294703ae329576462f9471b356971f53010d/CPP/Clipper2Lib/src/clipper.offset.cpp#L273-L297)).
  - The raw offset is cleaned up by a Union using the Positive or Negative fill rule ([L574-633](https://github.com/AngusJohnson/Clipper2/blob/a6e7294703ae329576462f9471b356971f53010d/CPP/Clipper2Lib/src/clipper.offset.cpp#L574-L633)).
  - The offset step uses `acos`/`sin`/`cos`. Rust documents its `sin` precision as "non-deterministic" across platforms ([f64 docs](https://doc.rust-lang.org/std/primitive.f64.html)), so Clipper2 offsets are not bit-identical between native and WebAssembly unless trig goes through `libm`.

**Rust ports of Clipper2**

- [clipper2-rust](https://github.com/larsbrubaker/clipper2-rust/blob/e8a250a4e9fed28e7e37dcc471e75ff62aaf8164/README.md):
  - Pure Rust, BSL-1.0, v1.2.0, no `unsafe`.
  - It *claims* 463 tests and an "exact behavioral match" with the C++ library. That claim is self-reported and *unverified*.
  - It was made by MatterHackers using Claude.
  - It uses `std` trig ([offset.rs L699-701](https://github.com/larsbrubaker/clipper2-rust/blob/e8a250a4e9fed28e7e37dcc471e75ff62aaf8164/src/offset.rs#L699-L701)).
- [tirithen/clipper2](https://github.com/tirithen/clipper2/blob/87c1ab857aa95d9c33f1b201bda026a8534fc5c6/README.md): MIT OR Apache-2.0. A safe wrapper over the C++ code via `clipper2c-sys`, so building it for WebAssembly needs a C++ toolchain.
- geo-clipper wraps the older Clipper1 (ISC, per crates.io).

**iOverlay** ([MIT OR Apache-2.0, v9.1.0](https://github.com/iShape-Rust/iOverlay/blob/117637e3a65804973da2bfd7c59f42773f4dcc0d/iOverlay/Cargo.toml#L1-L10); it powers the booleans in `geo`, per [README L13](https://github.com/iShape-Rust/iOverlay/blob/117637e3a65804973da2bfd7c59f42773f4dcc0d/iOverlay/README.md#L13)).

- Integer engines in 16, 32 or 64 bits, with coordinates limited to ±2^(N−2) ([L629-662](https://github.com/iShape-Rust/iOverlay/blob/117637e3a65804973da2bfd7c59f42773f4dcc0d/iOverlay/README.md#L629-L662)). Float APIs map onto that integer grid, optionally at a fixed scale.
- Crossings ([cross_solver.rs L76-135](https://github.com/iShape-Rust/iOverlay/blob/117637e3a65804973da2bfd7c59f42773f4dcc0d/iOverlay/src/split/cross_solver.rs#L76-L135)): classified with exact integer orientation tests.
- Crossing points ([L164-221](https://github.com/iShape-Rust/iOverlay/blob/117637e3a65804973da2bfd7c59f42773f4dcc0d/iOverlay/src/split/cross_solver.rs#L164-L221)): rounded with wide-integer division. They are snapped to nearby endpoints within a radius that grows on each pass ([snap_radius.rs](https://github.com/iShape-Rust/iOverlay/blob/117637e3a65804973da2bfd7c59f42773f4dcc0d/iOverlay/src/split/snap_radius.rs#L5-L29)), which works like snap rounding.
- Fill rules include positive and negative, and it can produce OGC-valid output.
- **`EdgeOverlay` carries user data on every edge through splits and merges** ([L237-308](https://github.com/iShape-Rust/iOverlay/blob/117637e3a65804973da2bfd7c59f42773f4dcc0d/iOverlay/README.md#L237-L308)). This is provenance, and it matters for section 6.
- Buffering builds round joins on the integer grid using CORDIC. The README explicitly recommends `MathMode::Integer` "for cross-platform deterministic construction" ([L492-533](https://github.com/iShape-Rust/iOverlay/blob/117637e3a65804973da2bfd7c59f42773f4dcc0d/iOverlay/README.md#L492-L533)). Arcs are flattened.

**cavalier_contours** (MIT OR Apache-2.0, v0.9.0)

- **Capabilities and limits** ([README L39-67](https://github.com/jbuckmccready/cavalier_contours/blob/22887a824d22cd8ab50968a551f34cdb4508127f/README.md#L39-L67)):
  - Polylines made of lines and **true arcs, which are never approximated**.
  - Bulge must stay within ±1 (at most a half circle per segment).
  - Offsets support **round joins only**.
  - Booleans work only between **two closed, non-self-intersecting** polylines.
  - Multi-polyline offsetting supports shapes with islands.
- **Offset algorithm** ([pline_offset.rs L1-23](https://github.com/jbuckmccready/cavalier_contours/blob/22887a824d22cd8ab50968a551f34cdb4508127f/cavalier_contours/src/polyline/internal/pline_offset.rs#L1-L23)):
  1. Build a raw offset of every segment.
  2. Join neighbouring offsets with arcs. The join's bulge (tan of a quarter of the sweep) is computed from dot and cross products using only `sqrt`, no trig ([raw_pline_offset.rs L290-325](https://github.com/jbuckmccready/cavalier_contours/blob/22887a824d22cd8ab50968a551f34cdb4508127f/cavalier_contours/src/polyline/internal/raw_pline_offset.rs#L290-L325)).
  3. Record self-intersections as "contacts".
  4. Cut the raw offset into slices.
  5. Discard any slice that comes closer to the source than the offset distance (within a tolerance).
  6. Stitch the remaining slices back together using contact identity.
- **Boolean algorithm:** find intersections, slice both polylines at them, prune slices by a point-containment test for the chosen operation, then stitch ([pline_boolean.rs L381-409](https://github.com/jbuckmccready/cavalier_contours/blob/22887a824d22cd8ab50968a551f34cdb4508127f/cavalier_contours/src/polyline/internal/pline_boolean.rs#L381-L409), [L635-638](https://github.com/jbuckmccready/cavalier_contours/blob/22887a824d22cd8ab50968a551f34cdb4508127f/cavalier_contours/src/polyline/internal/pline_boolean.rs#L635-L638)).
- **Robustness:**
  - Plain `f64` with **absolute** tolerances (`pos_equal_eps` 1e-5, `offset_dist_eps` 1e-4) ([pline_types.rs L65-107](https://github.com/jbuckmccready/cavalier_contours/blob/22887a824d22cd8ab50968a551f34cdb4508127f/cavalier_contours/src/polyline/pline_types.rs#L65-L107)). Absolute tolerances do not scale with model size.
  - Uses `atan2`, `atan` and `tan` in places ([base_math.rs L275, L554-567](https://github.com/jbuckmccready/cavalier_contours/blob/22887a824d22cd8ab50968a551f34cdb4508127f/cavalier_contours/src/core/math/base_math.rs#L554-L567)).
  - Hashing uses fixed seeds, so it behaves identically on WebAssembly ([Cargo.toml L26-30](https://github.com/jbuckmccready/cavalier_contours/blob/22887a824d22cd8ab50968a551f34cdb4508127f/cavalier_contours/Cargo.toml#L26-L30)).

**Other curve-aware engines**

- **iCurve** (MIT, v0.2.0, three stars, so immature):
  - Booleans on lines, Béziers and elliptic arcs, where elliptic arcs are held as rational quadratics.
  - **Topology is decided on adaptive chords placed on iOverlay's integer grid.** Each piece keeps its source curve and parameter interval, and the result is reconstructed as curves.
  - It admits that arcs "may no longer lie exactly on their supporting ellipse after snapping" ([README L144-204](https://github.com/iShape-Rust/iCurve/blob/37b5ee8691aa0463205225e2f56b12ef6dac91a2/iCurve/README.md#L144-L204)).
- **linesweeper:** a float Bentley–Ottmann sweep with proven "error bars" for Bézier paths. Early beta, now moved to Radicle ([README](https://github.com/jneem/linesweeper/blob/52b4a11bfa5d7c574b8178268fb5ef3f063f63da/README.md), [sweep.typ](https://github.com/jneem/linesweeper/blob/52b4a11bfa5d7c574b8178268fb5ef3f063f63da/docs/sweep.typ)). Licence: the repo has both MIT and Apache files; *unverified* which applies.

**Verdict on arcs.**

- Clipper2 and iOverlay **flatten** arcs.
- cavalier keeps **exact arc geometry** but uses approximate float predicates.
- iCurve keeps **curve identity** but decides topology from chords.
- Only CGAL's circle-segment traits (GPL) give exact arc *topology*.
- No permissive library offers exact, scale-invariant guarantees with arcs.

---

## 3. 2D arrangements and region finding

**CGAL Arrangement_2** is [GPL-3.0-or-later](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Arrangement_on_surface_2/package_info/Arrangement_on_surface_2/license.txt) or commercial, so **method study only**. Described in our own words:

- **Storage:** a doubly-connected edge list (DCEL). Each face has an outer boundary cycle, inner cycles (holes), and isolated vertices ([manual](https://doc.cgal.org/latest/Arrangement_on_surface_2/index.html)).
- **Insertion:** one curve at a time by walking its "zone" ([Arrangement_zone_2.h](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Arrangement_on_surface_2/include/CGAL/Arrangement_zone_2.h)), or many curves at once by plane sweep ([Surface_sweep_2 visitors](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Arrangement_on_surface_2/include/CGAL/Surface_sweep_2/Arr_insertion_ss_visitor.h)).
- **Overlay** of two arrangements, with face data merged ([Arr_overlay_2.h](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Arrangement_on_surface_2/include/CGAL/Arr_overlay_2.h)).
- **Point location:** naive, walk-along-a-line, landmarks, or randomised trapezoidal map.
- **Curves** are split into x-monotone pieces.
- **Exact arcs:** the circle-segment traits represent each point coordinate as a + b√c with a, b, c rational ("one-root numbers"), so line–circle and circle–circle intersections are exact for rational centres and rational squared radii ([Circle_segment_2.h L8, L40-46](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Arrangement_on_surface_2/include/CGAL/Arr_geometry_traits/Circle_segment_2.h#L40-L46); [traits L40-50](https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Arrangement_on_surface_2/include/CGAL/Arr_circle_segment_traits_2.h#L40-L50)).

**Planar face extraction: JTS Polygonizer.** JTS is dual EPL-2.0 / [EDL-1.0](https://github.com/locationtech/jts/blob/5a0a73632216e3e9d277150336078ec423fd215f/LICENSE_EDLv1.txt); EDL is BSD-style and usable.

- Input must already be **noded**, meaning edges meet only at their endpoints.
- It reports dangles, cut edges and invalid rings as errors ([Polygonizer.java L27-50](https://github.com/locationtech/jts/blob/5a0a73632216e3e9d277150336078ec423fd215f/modules/core/src/main/java/org/locationtech/jts/operation/polygonize/Polygonizer.java#L27-L50)).
- Minimal rings come from sorting edges by angle around each node and choosing the next edge counter-clockwise ([PolygonizeGraph.java L304-309](https://github.com/locationtech/jts/blob/5a0a73632216e3e9d277150336078ec423fd215f/modules/core/src/main/java/org/locationtech/jts/operation/polygonize/PolygonizeGraph.java#L304-L309)).
- Rings are then classified as shells or holes, and holes are assigned to shells.

**Noding: snap rounding.** All output vertices are forced onto a grid via "hot pixels". Vertices cannot be pre-rounded independently, because that alone can change topology ([SnapRoundingNoder.java L29-56](https://github.com/locationtech/jts/blob/5a0a73632216e3e9d277150336078ec423fd215f/modules/core/src/main/java/org/locationtech/jts/noding/snapround/SnapRoundingNoder.java#L29-L56); [Hobby 1999](https://www.nokia.com/bell-labs/publications-and-media/publications/practical-segment-intersection-with-finite-precision-output)).

**Robustness issues with arcs.**

- A tangency is a double root, so float intersection either loses it or reports two near-duplicate points.
- LibreCAD shows the symptoms: a special `TangentFinder` fallback, an on-entity tolerance of 1e-4 marked "fixme", and a tangent tolerance of 1e-7 ([rs_information.cpp L50-53](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/librecad/src/lib/information/rs_information.cpp#L50-L53), [L486-519](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/librecad/src/lib/information/rs_information.cpp#L486-L519)).
- Iron-clad options:
  - (a) Exact algebraic arc predicates in the CGAL style.
  - (b) iCurve-style chord topology on a grid, with curve provenance.
- **How the drafting commands use this:**
  - BOUNDARY / hatch pick-point is point location in the arrangement of visible curves. Islands are the inner boundary cycles; text boxes are added as extra loops.
  - Room boundaries are the same operation run on wall-line arrangements.

---

## 4. Drafting edit operations and snapping

LibreCAD is GPLv2 ([LICENSE](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/LICENSE)), so method study only.

**Intersection** ([rs_information.cpp L413-546](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/librecad/src/lib/information/rs_information.cpp#L413-L546)):

1. Bounding-box prefilter.
2. Skip neighbouring segments of the same spline.
3. Dedicated solvers for line–line, line–arc and arc–arc.
4. Everything else becomes a general conic (6-coefficient implicit form) handed to a simultaneous-quadratic solver.
   - Via quartic elimination, or via a "projective pencil" (the singular members of C1 + λC2 come from a cubic determinant and factor into lines) ([rs_math.cpp L1880-1890](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/librecad/src/lib/math/rs_math.cpp#L1880-L1890)).
   - Then Newton-style verification and refinement ([L2225-2245](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/librecad/src/lib/math/rs_math.cpp#L2225-L2245)).
5. Each hit is checked to be on both entities (absolute tolerance) and flagged if tangent.
6. Splines are handled as point sequences.

**Snapping** ([rs_snapper.cpp L701-746](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/librecad/src/lib/actions/rs_snapper.cpp#L701-L746)):

- Each enabled mode (endpoint, centre, middle, distance, intersection, on-entity, grid) proposes a candidate; the one **closest to the cursor wins**.
- If the winner is farther than a pixel-derived range, the point stays free ([L834-872](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/librecad/src/lib/actions/rs_snapper.cpp#L834-L872)).
- Intersection snap takes the entity nearest the cursor and intersects it with **every** other visible entity, a linear scan ([rs_entitycontainer.cpp L1305-1342](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/librecad/src/lib/engine/document/container/rs_entitycontainer.cpp#L1305-L1342)).
- "Visual snap" guide lines (tangent, normal, angle steps, ortho from a vertex) provide extension and tracking behaviour.

**Fillet** ([rs_modification.cpp L2488-2515](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/librecad/src/lib/modification/rs_modification.cpp#L2488-L2515)):

1. Offset both entities by the radius toward the picked side.
2. Intersect the two offsets to get the fillet centre.
3. Project the centre onto each entity to get the tangent points.
4. Trim.

Inside a polyline it is limited to adjacent line segments. Trim, bevel and stretch are at [L2008](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/librecad/src/lib/modification/rs_modification.cpp#L2008), [L2303](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/librecad/src/lib/modification/rs_modification.cpp#L2303) and [L2708](https://github.com/LibreCAD/LibreCAD/blob/2af4873798fe5e2b4119720c351fc66835cad5bc/librecad/src/lib/modification/rs_modification.cpp#L2708).

**Spare Kit method (our design)**

- **Work in parameter space.** Every edit runs on exact curve types and returns curve parameters, not points. Trim, break and join cut or merge at intersection parameters.
- **EXTEND** intersects the entity's *carrier*: line → infinite line, arc → full circle, elliptical arc → full ellipse. Splines have no canonical extension, so this needs a policy.
- **LENGTHEN** works in arc length. Ellipse and spline arc length need deterministic quadrature.
- **Fillet** generalises to line, arc, ellipse and spline through offset–intersect–project. Ellipses and splines need numeric offsets plus Newton refinement.
- **CHAMFER** uses distances or a distance and an angle along each curve.
- **STRETCH** moves only the vertices inside the window and keeps bulge and tangent relationships.
- **ARRAY, MIRROR, SCALE, ALIGN** are exact affine transforms, keeping the "type may change" rule from section 1.
- **DIVIDE / MEASURE** place equal arc-length parameters.
- **Snap kinds** should be exactly the DIMASSOC list: endpoint, mid, centre, node, quadrant, intersection, insertion, perpendicular, tangent, nearest, apparent intersection, parallel, start. Extension is added as a tracking snap.
  - Apparent intersection = intersection of the view-plane projections of two 3D curves.
  - Tangent from a point: closed form for circles; a quartic for ellipses.
  - Perpendicular and nearest: closest-point root finding on the curve parameter.
- **Each snap returns a typed reference** (entity, kind, parameter or parameters), so dimensions and constraints can bind to it.
- **Use a spatial index** (R-tree) instead of LibreCAD's O(N) scan.

---

## 5. Constrained 2D sketch solvers

**SolveSpace** is [GPL-3](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/COPYING.txt), so method study only.

- **Equations and Jacobian:** constraints are symbolic expressions, so the Jacobian comes from exact symbolic partial derivatives.
- **Pre-passes:** a substitution pass removes equations of the form a − b = 0 between parameters ([system.cpp L99-130](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/system.cpp#L99-L130)). Equations with a single unknown are then solved on their own ([L547-569](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/system.cpp#L547-L569)).
- **Newton step** ([L273-352](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/system.cpp#L273-L352)):
  - The step is the minimum-norm least-squares solution.
  - It is computed by sparse QR of Jᵀ, avoiding JJᵀ because forming it squares the condition number (the code comment explains this).
  - Dragged parameters are weighted so the solver prefers to leave them where the user put them.
- **Iteration** ([L354-432](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/system.cpp#L354-L432)):
  - Backtracking line search, up to 8 halvings, at most 50 iterations.
  - Converged when every residual is below 1e-8. That is `LENGTH_EPS/100` ([L18](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/system.cpp#L18), [defs.h L7](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/defs.h#L7)), an absolute value that does not scale.
- **Degrees of freedom:** DOF = number of parameters − rank, with rank from sparse QR ([L240-259](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/system.cpp#L240-L259)).
- **Failure reporting:**
  - If it does not converge, it lists the constraints whose residuals are still non-zero.
  - If the system is rank-deficient, it removes each constraint in turn, recomputes the rank, and reports those whose removal restores full rank. This is one QR per constraint, with a timeout ([L469-513](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/system.cpp#L469-L513)).
  - Free parameters are found with one rank test per parameter ([L668-687](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/system.cpp#L668-L687)).
- **No structural graph decomposition** inside the solver; SolveSpace's decomposition is the user's sketch groups.
- **libslvs licensing:** the repo carries only GPL-3. Its Python package's MIT classifier ([pyproject L16](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/pyproject.toml#L16)) conflicts with that; **treat libslvs as GPL-3**. `rust_slvs` is GPL-3.

**FreeCAD planegcs** is [LGPL-2.1-or-later](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Sketcher/App/planegcs/GCS.cpp#L1), so method study only.

- **Solvers:** BFGS, Levenberg–Marquardt, or DogLeg; DogLeg is the default ([GCS.h L62-67](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Sketcher/App/planegcs/GCS.h#L62-L67)).
- **Defaults:** convergence 1e-10, QR pivot threshold 1e-13 ([GCS.cpp L485-499](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Sketcher/App/planegcs/GCS.cpp#L485-L499)).
- **Decomposition** ([L1741-1810](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Sketcher/App/planegcs/GCS.cpp#L1741-L1810)): connected components of the parameter–constraint graph, plus aliasing of parameters tied by equality constraints.
- **Diagnosis** ([L4773-4790](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Sketcher/App/planegcs/GCS.cpp#L4773-L4790)):
  - QR of Jᵀ. It uses dense QR below 1000 DOF because sparse QR "is known to fail rank detection" on some structures ([L4846-4853](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Sketcher/App/planegcs/GCS.cpp#L4846-L4853), [issue 10903](https://github.com/FreeCAD/FreeCAD/issues/10903)).
  - Columns of R beyond the rank define **conflict groups** of constraints ([L5459-5480](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Sketcher/App/planegcs/GCS.cpp#L5459-L5480)).
  - A "popularity contest" removes the constraint that appears in the most groups and re-solves. If the system then solves, the removed constraints were **redundant**; otherwise they are **conflicting** ([L5480-5600](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Sketcher/App/planegcs/GCS.cpp#L5480-L5600)).
  - It also reports "partially redundant" constraints.
  - Tags map solver rows back to the user's sketch constraints.

**Permissive alternatives**

- **ezpz** (KittyCAD/Zoo; [MIT, v0.2.29](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/Cargo.toml#L9-L25); depends on `faer` and `libm`):
  - Levenberg–Marquardt on sparse Cholesky of JᵀJ + λI ([newton.rs L27-135](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/solver/newton.rs#L27-L135)).
  - **Priority levels:** it adds lower-priority constraints until the system becomes unsatisfiable, then returns the last satisfiable solve ([lib.rs L199-245](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/lib.rs#L199-L245)).
  - **Freedom analysis:** column-pivoted QR with rank tolerance 1e-8 × the largest diagonal, a null-space basis, and per-variable "participation" to list underconstrained variables ([find_dof.rs](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/solver/find_dof.rs#L1-L104)).
  - Constraints cover lines, circles and arcs only ([constraints.rs L40-92](https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/constraints.rs#L40-L92)).
  - It has a text problem format, which is readable by AIs.
  - Bit-determinism of `faer` across targets is *unverified*.
- **fiksi** ([Apache-2.0 OR MIT, v0.1.0](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/Cargo.toml#L24)):
  - Optimisers: LM or L-BFGS ([solve/mod.rs L17-25](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/solve/mod.rs#L17-L25)).
  - **Graph decomposition** with the Hoffmann–Lomonosov–Sitharam Modified Frontier algorithm, producing a recombination plan of rigid clusters (D = 3 in 2D) ([recursive_assembly.rs L146-163](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/analyze/graph/recursive_assembly.rs#L146-L163)). Includes a minimum-dense-subgraph search ([L485-495](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/analyze/graph/recursive_assembly.rs#L485-L495)).
  - Witness-method style incremental Gauss–Jordan elimination to find overconstraints ([numerical/mod.rs L10-33, L119-123](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/analyze/numerical/mod.rs#L10-L33)).
  - Optional `libm` feature ([lib.rs L8-12](https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/lib.rs#L8-L12)).
  - Early stage; last commit May 2026.
- **Not usable:** GeoSolver forks (licence NOASSERTION, *unverified*). D-Cubed is proprietary; excluded.

**Literature**

- Owen 1991: graph analysis into triconnected subproblems, then algebraic solve ([DBLP](https://dblp.uni-trier.de/pid/78/6444.html)).
- Hoffmann, Lomonosov & Sitharam 2001, decomposition–recombination planning, Parts [I](https://doi.org/10.1006/jsco.2000.0402) and [II](https://doi.org/10.1006/jsco.2000.0403).
- Survey: [Zou et al. 2022](https://arxiv.org/abs/2202.13795), which argues under- and over-constrained parts must be found before numerical solving.

**Reporting to users and AIs (recommended).** Return a deterministic, structured diagnosis containing:

- The DOF count.
- For each free entity, its null-space motions in plain terms, e.g. "P1 translates along (0.6, 0.8)".
- Conflict groups: sets of constraint IDs where removing any one makes the system consistent.
- Redundant constraints.
- Unsatisfied constraints with residuals in model units.
- Optionally, the decomposition plan (clusters and their order).

Orderings must be by stable IDs.

---

## 6. Plans and sections from 3D: the 2D side

**OCCT hidden-line removal** (LGPL-2.1 + [exception](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/OCCT_LGPL_EXCEPTION.txt); method reference)

- Output edges are classified visible/hidden × sharp (C0), smooth (G1), sewn (C2), outline (silhouette), and isoparametric ([HLRBRep_HLRToShape.hxx L32-59](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingAlgorithms/TKHLR/HLRBRep/HLRBRep_HLRToShape.hxx#L32-L59)).
- The 2D edges "are not included in the data structure" of the source shape, so provenance is weak.

**FreeCAD TechDraw shows the failure mode** ([DimensionAutoCorrect.cpp L24-58](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/TechDraw/App/DimensionAutoCorrect.cpp#L24-L58)):

- Dimensions reference edges by sub-element **index** and keep a saved copy of the geometry.
- After regeneration, they search for identical geometry to re-link.
- The "most similar" second phase is unimplemented. So index-based provenance breaks when the model changes.

DXF's VIEWPORT (view direction, target, twist, view height, clip planes) plus DIMASSOC's trans-space flag show a paper-space annotation bound to model geometry *through* a viewport (section 1).

**Recommended representation (our design)**

- **Drawing curve.** Each element of a view result is an exact 2D curve (line, arc, elliptical arc, or NURBS) carrying:
  - Provenance: element ID; persistent face/edge ID; kind (Cut, Projection-visible, Projection-hidden, Silhouette, Beyond); and the 3D edge's parameter range when the edge is exact.
  - A style key: category, subcategory, material and phase. Linetype and lineweight are resolved later from view settings.
  - A depth interval used for draw order.
- **Fill region.** Cut faces become regions whose loops are drawing curves, with material provenance. Region hatching (poché) is a hatch with a generated boundary.
- **Silhouettes.** These are view-dependent and have no 3D edge. Their provenance is (face ID, silhouette, view parameters), and anything snapped to them re-resolves by that key.
- **Incremental update:**
  - Cache cut and projection results per (element, view), keyed by element geometry hash and view parameters.
  - When an element changes, recompute its curves, then re-run visibility only for elements whose screen-space bounding box overlaps the changed area.
  - The visibility merge itself is a 2D overlay. iOverlay's `EdgeOverlay` (data propagated through splits and merges) and iCurve's source-curve + parameter-interval tracking are permissive precedents.
- **Dimensions and tags** store (element ID, persistent edge ID, snap kind, parameter) and resolve through the provenance map, never through an index.

---

## 7. Recommendation: the 2D layer of Spare Kit

**Numeric policy (decide first)**

- **Topology on an integer grid.** Coordinates are i64 on a fixed grid. A 1 nm unit within iOverlay's ±2^62 limit spans about ±4.6×10⁹ m, which is full scale.
- **Exact predicates.** Orientation via i128 products, as in [Clipper2](https://github.com/AngusJohnson/Clipper2/blob/a6e7294703ae329576462f9471b356971f53010d/CPP/Clipper2Lib/include/clipper2/clipper.core.h#L751-L785) and [iOverlay](https://github.com/iShape-Rust/iOverlay/blob/117637e3a65804973da2bfd7c59f42773f4dcc0d/iOverlay/src/split/cross_solver.rs#L76-L135). Circle predicates need roughly 4× the coordinate bits (i256), so use a small bignum type.
- **Constructions in f64** using only IEEE basic operations, `sqrt` and explicit `mul_add`, all of which Rust guarantees correctly rounded ([f64 docs](https://doc.rust-lang.org/std/primitive.f64.html)).
- **All transcendental functions via the `libm` crate** (MIT, per crates.io), as ezpz does. `std` trig is not deterministic.
- **Fixed-seed hashing** and stable-ID ordering everywhere.

**Entity types.** Ship the section 1 list as kernel types with exact parameters:

- Line, Ray, XLine, Circle, Arc, Ellipse/EllipticalArc (parameter-based), BulgePolyline (with widths), NURBS (with optional fit intent).
- Loop and Region (outer + holes).
- Hatch: region + pattern family + gradient.
- TextAnchor, Dimension (typed snap references + style), Leader, BlockDef/Insert (affine; transforms may change type), Viewport.
- Implement the rule "transforming an entity can change its type" at the type level.

**Exact arcs.** Keep arcs exact as (endpoints + bulge) or (centre, radius, angles). Never flatten in stored data.

**Booleans and offsets: port order**

1. **iOverlay** (MIT/Apache) as a dependency for straight-segment booleans, region cleanup and hatch clipping. It is integer-based, has `EdgeOverlay` provenance, and supports deterministic integer construction.
2. **cavalier_contours** (MIT/Apache) for exact-arc polyline offsets. Patch it to use grid-relative (scale-invariant) tolerances and route trig through `libm`. Clipper2's offsets flatten arcs; use clipper2-rust (BSL) only as a cross-check oracle.
3. **Curve booleans:** implement the iCurve pattern ourselves (chord topology on the grid, provenance, curve reconstruction). Also evaluate depending on iCurve (MIT) itself once it matures.
4. **Research track:** exact one-root-number (a + b√c) arc predicates, the method described in section 3, implemented in our own words and code.

**Arrangement / region finding.** DCEL + plane sweep + snap-rounded noding (methods from Hobby and JTS) + JTS-style minimal-ring extraction with dangle/cut-edge reporting + point location for BOUNDARY and hatch picks. Write it ourselves in Rust; the JTS EDL licence also permits porting.

**Edit ops and snaps.** Parameter-space operations (section 4); snap references matching DIMASSOC's 13 kinds plus extension; an R-tree for candidate search.

**Solver**

- Base it on ezpz (MIT) or fiksi (Apache/MIT), with:
  - LM with sparse factorisation.
  - fiksi's Hoffmann–Lomonosov–Sitharam decomposition and witness-method overconstraint detection.
  - The planegcs-style conflict/redundant split, re-implemented in our own words: QR of Jᵀ, conflict groups, removal test.
  - Dense rank checks below about 1000 DOF.
  - Scale-relative tolerances.
- Extend the constraint set to ellipses and splines.

**Order of work**

1. Numeric policy and entity types, with DXF import/export.
2. Intersections and snaps.
3. Edit ops.
4. Arrangement and hatch.
5. Offsets (cavalier) and booleans (iOverlay).
6. Solver.
7. View provenance layer.

---

## Adopt / Avoid / Open questions

**Adopt**

- iOverlay as a dependency: MIT/Apache, integer, provenance support.
- cavalier_contours (fork/port): exact-arc offsets.
- ezpz and/or fiksi as solver bases: MIT; Apache/MIT.
- `libm`, `robust` (MIT/Apache), `kurbo` (Apache/MIT).
- JTS methods (EDL): polygonizer and snap rounding.
- DIMASSOC-style typed snap references.
- Persistent-ID provenance on every drawing curve.

**Avoid**

- Copying code from CGAL (GPL), LibreCAD (GPLv2), SolveSpace / libslvs / rust_slvs (GPL-3), or planegcs / TechDraw (LGPL). Study methods only.
- Proprietary ACIS region data.
- D-Cubed.
- `std` trig in deterministic paths.
- Absolute epsilons (cavalier 1e-5, SolveSpace 1e-8).
- Index-based references (the TechDraw failure).
- Flattened arcs in stored geometry.

**Open questions**

- Grid resolution (1 nm?) and whether spline and ellipse geometry also snaps to the grid.
- Exact arc predicates (i256 / one-root numbers) now, or chord topology first?
- Whether `faer`'s results are bit-identical between native and WebAssembly (*unverified*).
- Extend semantics for splines.
- An ARC_DIMENSION representation (undocumented in DXF).
- Spline fit-point → control-point algorithm, versioned for byte-stability.
- Whether clipper2-rust's behavioural-equivalence claim holds (*unverified*).
- linesweeper's licence and its move to Radicle (*unverified*).
- The licence status of GeoSolver forks (*unverified*).
