# 10. Operations: the contract and the catalogue

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

## 10.1 The contract

Every operation, from "make a box" to "fillet these 40 edges", has the same shape:

```rust
fn operation(ctx: &OpContext, input: &OpInput) -> Result<OpOutput, OpError>
```

- **`OpContext`** carries the operation's permanent ID from the operation log, its budgets (work units and logical memory), a cancellation token, and the precision level (final, or a coarser preview the caller asked for). It holds no randomness and no clock that can affect the result. A time limit is not a budget: it is the caller's deadline, which its cancellation token turns into a cancellation, and a cancelled run is never logged as the operation's result (owner action G71, 32.15).
- **`OpOutput`** carries:
  - the new bodies, immutable;
  - the **history**: for every output face, edge and vertex, whether it is unchanged, modified, generated, split from, or merged from which inputs, and which inputs were deleted (section 20);
  - the new **names**;
  - each body's **exactness class** and bounds;
  - **diagnostics**: warnings with the entities they concern (a short edge, a near-tangency handled).
- **`OpError`** is typed per operation, on a shared base (section 22). It names the entities involved, the parameter values and points, and what would make it succeed.
- **Transactional.** Nothing is published until the result passes the checker. A failed or cancelled operation changes nothing.
- **Budgeted and cancellable.** Long operations check their budget and the cancellation token at bounded steps and stop cleanly. They never unwind a panic, which WebAssembly builds cannot catch.
- **Batch first.** Operations take many inputs where that is natural: 300 openings cut from a wall in one call; a thousand panels evaluated in one call with one result per item, so one failure does not fail the list. Every item carries a stable key given by the caller, such as the panel's permanent ID. The item's `OpPath` is the batch's `OpPath` plus that key, and its result is keyed by it. Items are never keyed by position, so inserting one item does not change the other items' `OpPath`s, and a batch gives the same names and bits as the same calls made one by one (section 23.6).

## 10.2 The catalogue

Each family below is a section of this note or of its build order.

| Family | Operations |
| --- | --- |
| Make | Points, curves and surfaces of every kind; primitives (box, cylinder, cone, sphere, torus, prism); bodies from faces (sew) |
| 2D | Drafting entities, regions, hatches, 2D booleans and offsets, trim, extend, fillet, chamfer, break, join, stretch, arrays, divide and measure, snaps (section 14) |
| Sketch | Constrained profiles for families, walls and floors (section 15) |
| Sweep | Extrude (with taper), revolve, sweep along a path with frame rules, swept blend, loft through sections with guides, network (Gordon) surfaces, IFC's sectioned solids along alignments (section 13) |
| Combine | Unite, subtract, intersect; general fuse and imprint; split; cells and regions; batch cutting (section 11) |
| Blend | Fillet and chamfer, constant and variable; vertex blends; blends with chosen continuity (G1, G2, G3) for Class-A surfaces (section 13) |
| Offset | Offset faces and bodies, shell, thicken, draft; face move, replace and delete with healing (section 13) |
| Transform | Move, rotate, mirror, scale (uniform and not), patterns |
| Repair | Import healing within ε_heal, sewing, small-feature removal |
| Query | Classify a point, measure, distance and clearance, time of first contact between moving bodies (continuous collision detection that never misses a contact, the Tight Inclusion method, MIT; used by door swings, crane and machine-tool paths, construction sequencing, games and the simulation engine), mass properties (exact in integers for flat solids whose vertices are grid points: twice each face's area vector, six times the volume; a certified interval when vertices are implicit, as most boolean results' are, section 19.2), bounding boxes; surface quality: curvature, continuity across edges, zebra and highlight lines, draft angle against a pull direction |
| Draw | Section cuts, projections, hidden lines, silhouettes (section 18). Line patterns laid out along curves by exact arc length (certified on ellipses and splines); hatch and fill pattern lines clipped exactly to their regions, arcs and islands included; surface patterns (brick coursing, tiles) mapped onto faces. The kernel makes this geometry, with provenance. Which pattern, weight and colour each category uses is the program's view rules; weights, colours and drawing to screen, print and PDF are `engine-draw2d`'s |
| Mesh | Watertight tessellation with a proven bound, levels of detail, mesh booleans, repair, volume (tetrahedral) meshes for simulation (section 19) |
| Manufacture | Swept volumes of moving tools, slicing into layers, parting lines from silhouettes; the strategies themselves (toolpaths, nesting) belong to a manufacturing program or domain toolkit built on these (section 26) |
| Free-form | Subdivision surfaces, fields, conversions, panelisation, surface division, unrolling (section 16) |
| Ground | Terrain from points and breaklines, contours, cut and fill, alignments, corridors, straight skeletons for roofs (sections 13 and 17) |
