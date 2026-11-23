# Floor types and floors, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [element types format](../construct-types/FORMAT.md) whose system block a floor type fills, the [cloud format](../construct-model/FORMAT.md) whose element record and recipe a floor uses unchanged, the [levels format](../construct-levels/FORMAT.md) whose tie it carries and the [parameter values](../construct-params/FORMAT.md) whose length spelling it uses. **We may stop writing an old version. We never stop reading it.** [`src/stored.rs`](src/stored.rs), [`src/floor_type.rs`](src/floor_type.rs), [`src/boundary.rs`](src/boundary.rs) and [`src/floor.rs`](src/floor.rs) are the reader library; the decisions behind it are in [the floor note](../../notes/CON-A4-F4-P1-floor-from-boundary.md), and those behind a sketched outline, a slope and slab edges ([`src/shape.rs`](src/shape.rs)) in [the slope note](../../notes/CON-A4-F4-P1-floor-slope-and-edges.md).

## A floor type

A floor type is a type record of category `500` (floors) whose system block has the owner tag `floor`: the `props` key `@system.floor` holds the text below, and construct-types stores it untouched. Its `@params` lists no parameter of its own yet.

Compact JSON, no spaces, the fields in exactly this order:

| Field | Value |
| --- | --- |
| `version` | `1` |
| `layers` | The layers from the top face (first) down to the bottom face (last), each as below |

| Layer field | Value |
| --- | --- |
| `function` | `structure`, `substrate`, `thermal` (insulation or an air space), `finish` or `membrane`: a wall layer's five, with the same join priorities |
| `thickness` | A length in construct-params' stored spelling, `length:q0:<steps>` (steps of the length grid, 6,400 to the millimetre; `length:q0:320000` is 50 mm): never below 0, and 0 only for a membrane |
| `material` | The material's permanent ID ([construct-materials](../construct-materials/src/material.rs)) |

```json
{"version":1,"layers":[{"function":"finish","thickness":"length:q0:320000","material":"screed-cement"},{"function":"structure","thickness":"length:q0:960000","material":"concrete-regular"}]}
```

A text that breaks a rule is refused, never repaired: it is the JSON above with no other field, spacing or order; the version is one this reader knows; at least one layer; every thickness a length in its one spelling, not below 0, and 0 only for a membrane; at least one layer thicker than 0; the layers add up to at most 10 m. Writing what was read gives the same bytes. The total thickness and the depth of each layer face are worked from the text each time and never stored.

## A floor

A floor is an element record like any other, of the floors category:

| Field | On a floor record |
| --- | --- |
| `kind` | `floor` |
| `level`, `placement`, `references` | `0`, `{"x":0,"y":0,"z":0}`, `[]`: the boundary's corners are where the floor is |
| `recipe` | `{"owner":"construct","schema":1,"op":"floor","op_version":1,"args":{"boundary":<the boundary>,"layers":{"type":"layers"}}}` |
| `props` | `@category` `500`, `@type` its floor type's ID, and its tie: `@level` and `@level.offset` (construct-levels) |

**Where it sits.** Its top face stands at its level's elevation plus its offset, and its layers hang below in order: in its own frame the top face is at `z = 0` and the bottom face at `z = -T`, `T` its type's total thickness.

**The boundary** is `{"version":1,"sketch":<sketch>,"outer":[<edge>, ...],"holes":[[<edge>, ...], ...]}`:

| Field | Value |
| --- | --- |
| `version` | `1` |
| `sketch` | The sketch solver's sketch (construct-solver's `Sketch`), as it writes itself: `points` (each `{"x":<steps>,"y":<steps>}`, numbered from 0 in order, as drawn), `lines` (each `{"start":<point>,"end":<point>}`), `constraints` (construct-solver's constraints as it writes them, `[]` for a floor drawn without any) and `curves` (only when there are arcs) |
| `outer` | The outer loop's edges in order, each ending where the next starts and the last where the first starts |
| `holes` | Each hole's edges, the same way; `[]` when there is none |

An edge is `{"id":"<edge ID>","line":<line>}`, or `{"id":"<edge ID>","arc":<curve>,"reversed":<true or false>}` for an arc, which version 1 holds and refuses (see below). An edge ID is a permanent ID unique in the floor: it names the floor's side face along that edge.

A 4 m by 3 m right triangle tied 20 mm below level `L-1`, its edges `a`, `b` and `c` (in the record, every JSON object's keys are in sorted order, as construct-model writes a recipe's arguments):

```json
{"id":"F-1","document":"DOC-1","kind":"floor","name":"Ground floor","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"floor","op_version":1,"args":{"boundary":{"holes":[],"outer":[{"id":"a","line":0},{"id":"b","line":1},{"id":"c","line":2}],"sketch":{"constraints":[],"lines":[{"end":1,"start":0},{"end":2,"start":1},{"end":0,"start":2}],"points":[{"x":0,"y":0},{"x":25600000,"y":0},{"x":0,"y":19200000}]},"version":1},"layers":{"type":"layers"}}},"props":{"@category":"500","@level":"L-1","@level.offset":"length:q0:-128000","@type":"floor-slab"},"references":[]}
```

**A sketched floor.** A sketch with constraints is solved by construct-solver (its `solve`, the solver family recipes use, from the points as drawn, within its default budget), and the floor's corners are the solved points, never the drawn ones; a sketch with none is its own solution. The stored points stay as drawn, so the same stored boundary always solves to the same corners; a dimension changed (a distance, a point's distance from a line, a radius or a diameter) is the same sketch with that constraint's steps changed and the points drawn where the last solve put them.

A boundary that is not exactly as written is refused, never repaired; and one that breaks a rule is refused in words naming the loop and the edge: no arc edge (for now: the engine seam's extrusion takes polygons only); every edge a line the sketch has, no two edges with one ID or one line; nothing in the sketch on no edge; a sketch with constraints that the solver solves (over-constrained, not converged, out of work or refused at rounding is refused with the solver's reason); each loop, as solved, at least three edges, closed, none of no length, crossing or touching nothing of itself; every hole wholly inside the outer loop touching nothing of it; no two holes overlapping, touching or one inside the other.

## A sloped floor, and slab edges

A floor with a slope arrow or slab edges is the same record with `op_version` `2` and one or both of these in its recipe's `args` beside `boundary` and `layers`; a floor with neither is written as version `1`, byte for byte as above. A version 1 recipe with either, or a version 2 recipe with neither, is refused.

`slope` is `{"at_tail":<length>,"head":[x,y],"tail":[x,y]}` with `"at_head":<length>` or `"slope":<slope>`: the arrow from its tail to its head in grid steps, the top face's height at the tail, and either its height at the head or a rise over a run along the arrow (construct-params' stored slope, `slope:<rise>/<run>`, a fall below zero), every height from the floor's tied height. The top face is the plane through the tail's height rising along the arrow and level across it. It is refused when the tail is the head; when a rise over a run is given along an arrow that is not a whole number of grid steps long; and when it puts any corner of any loop at a height that is not a whole grid step (the edge that starts there is named). Layer thicknesses are plumb: each layer hangs straight down from the sloped top face by its thickness.

```json
{"at_head":"length:q0:-256000","at_tail":"length:q0:0","head":[25600000,0],"tail":[0,0]}
```

`edges` is a list of one or more slab edges, each `{"depth":<length>,"edge":"<edge ID>","foot":<length>,"kind":"thickened","width":<length>}` or `{"depth":<length>,"edge":"<edge ID>","kind":"downturn","width":<length>}`: a profile run below the floor's bottom face along that edge of its boundary (outer loop or hole), flush with its side face, `width` in from the edge at the underside, `depth` below it, a thickened edge tapering to `foot` at its bottom (above zero and at most its width). Where two edges with the same profile meet they are mitred on the corner's bisector; where an edge meets a plain one it ends square. Refused, naming the edge: an edge the boundary lacks, or one edge twice; a width, foot or depth not above zero, a foot wider than the width, a depth beyond 10 m; a piece whose corners fall between grid steps (a width across an edge not along x or y lands only where the edge's length divides it); two different profiles meeting at a corner; a square end at a corner sharper than a right angle; a piece reaching another side of the floor or whose ends cross; two pieces overlapping; a volume that is not a whole number of cubic grid steps. A floor with both a slope and slab edges is refused.

```json
{"depth":"length:q0:1920000","edge":"e0","foot":"length:q0:1280000","kind":"thickened","width":"length:q0:2560000"}
```

## What a command gives

A command gives a boundary in a plain form, `{"outer":[<corner>, ...],"holes":[[<corner>, ...], ...]}`, each corner `{"id":"<edge ID>","at":[x,y]}` in grid steps, the edge from it to the next; `id` may be left out, for `o<n>` on the outer loop and `h<k>-<n>` on hole `k` (both from 0), and `holes` may be left out. It is written as a new sketch, the outer loop's points and lines first, then each hole's, and stored in the form above.

The server's commands (`floor.create`, `floor.boundary`: construct-model's FORMAT.md, "Commands") take the same form with each `at` as two typed lengths (`["8 m", "4000"]`), read in the document's units and turned into grid steps before this crate reads it, so a person never types steps. A boundary they take also has an area of a whole number of square grid steps; typed lengths, on the hundredth of a millimetre at the finest, always give one.

## What a builder takes

Before it is built, a floor's recipe is filled from its type (construct-regen's `slabs`): its `args` become exactly `loops` and `layers`. `loops` is the outer loop anticlockwise seen from above, then each hole clockwise, a loop drawn the other way run backwards from its first corner, each corner `{"id":"<edge ID>","at":[x,y]}` in grid steps with the edge from it to the next. `layers` is the type's layers from the top down, each `{"function":"<function>","material":"<material ID>","thickness":<grid steps>}`. A version 2 floor's filled recipe keeps its version and adds `tops` for a slope (the top face's height at each corner of each loop, in the order `loops` lists them, in grid steps) and `edges` for slab edges (each `{"id":"<edge ID>","kind":"<kind>","top":[[x,y,z] x4],"foot":[[x,y,z] x4]}`: its plan at the floor's underside and at its foot, anticlockwise seen from above, along the edge and back along its inner face, in grid steps). The builder lifts each layer's bottom face onto the sloped plane and builds the layer straight up from it by its thickness, and unites the slab edges with the lowest layer. This form is never stored in a record: it is what a builder is handed, and its bytes go into no hash but the built shape's.

## What is worked out and never stored

A floor's area (the shoelace sum over grid steps, holes subtracted), its perimeter (every edge of every loop, each to the nearest step), its thickness, its volume (per layer, the area by the layer's thickness) and its mass (per layer, from the material's density where the project's materials hold one) are its read-only parameters `floor.area`, `floor.perimeter`, `floor.thickness`, `floor.volume` and `floor.mass` (kilograms), worked from the record each time. Each layer's volume and mass are `floor.layer-<k>.volume` and `floor.layer-<k>.mass` (kilograms), layer `k` counted from 1 at the top, named `Layer <k> (<function>, <material>) volume` and `... mass (kg)`. A mass has no value while the library lacks the layer's material, and the floor's none while any layer's has none. A sloped floor also reads back `floor.top-area` (its top face's area, the plan area times the root of one plus the gradient squared, to the nearest square step: exact only where that root is whole in ratio), `floor.highest` and `floor.lowest` (from its tied height: the top face's highest corner, and the bottom face's lowest corner); a floor with slab edges `floor.edge-<edge ID>.length` and `floor.edge-<edge ID>.volume` for each, named `Slab edge <edge ID> (<kind>) length` and `... volume`, and its lowest point is then the deepest foot. `floor.area`, `floor.thickness` and `floor.volume` are the slab's: a sloped floor's volume is its plan area times its thickness exactly, and slab edges are counted apart. The server's parameters view lists them after the parameters a floor's category has, each shown in the document's units, with `from` `read-only`.

## Later versions

Arc edges are held already and refused; allowing them takes a check away and changes no byte. Constraints in the sketch were held and refused in the first version and are now solved, which changed no byte. A core in a floor type, and slab edges under a sloped floor, will be new versions here, read beside versions 1 and 2 for ever.
