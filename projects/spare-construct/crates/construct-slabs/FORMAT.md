# Floor types and floors, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [element types format](../construct-types/FORMAT.md) whose system block a floor type fills, the [cloud format](../construct-model/FORMAT.md) whose element record and recipe a floor uses unchanged, the [levels format](../construct-levels/FORMAT.md) whose tie it carries and the [parameter values](../construct-params/FORMAT.md) whose length spelling it uses. **We may stop writing an old version. We never stop reading it.** [`src/stored.rs`](src/stored.rs), [`src/floor_type.rs`](src/floor_type.rs), [`src/boundary.rs`](src/boundary.rs) and [`src/floor.rs`](src/floor.rs) are the reader library; the decisions behind it are in [the floor note](../../notes/CON-A4-F4-P1-floor-from-boundary.md).

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
| `sketch` | The sketch solver's sketch (construct-solver's `Sketch`), as it writes itself: `points` (each `{"x":<steps>,"y":<steps>}`, numbered from 0 in order), `lines` (each `{"start":<point>,"end":<point>}`), `constraints` (always `[]` in version 1) and `curves` (only when there are arcs) |
| `outer` | The outer loop's edges in order, each ending where the next starts and the last where the first starts |
| `holes` | Each hole's edges, the same way; `[]` when there is none |

An edge is `{"id":"<edge ID>","line":<line>}`, or `{"id":"<edge ID>","arc":<curve>,"reversed":<true or false>}` for an arc, which version 1 holds and refuses (see below). An edge ID is a permanent ID unique in the floor: it names the floor's side face along that edge.

A 4 m by 3 m right triangle tied 20 mm below level `L-1`, its edges `a`, `b` and `c` (in the record, every JSON object's keys are in sorted order, as construct-model writes a recipe's arguments):

```json
{"id":"F-1","document":"DOC-1","kind":"floor","name":"Ground floor","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"floor","op_version":1,"args":{"boundary":{"holes":[],"outer":[{"id":"a","line":0},{"id":"b","line":1},{"id":"c","line":2}],"sketch":{"constraints":[],"lines":[{"end":1,"start":0},{"end":2,"start":1},{"end":0,"start":2}],"points":[{"x":0,"y":0},{"x":25600000,"y":0},{"x":0,"y":19200000}]},"version":1},"layers":{"type":"layers"}}},"props":{"@category":"500","@level":"L-1","@level.offset":"length:q0:-128000","@type":"floor-slab"},"references":[]}
```

A boundary that is not exactly as written is refused, never repaired; and one that breaks a rule is refused in words naming the loop and the edge: no constraints; no arc edge (for now: the engine seam's extrusion takes polygons only); every edge a line the sketch has, no two edges with one ID or one line; nothing in the sketch on no edge; each loop at least three edges, closed, none of no length, crossing or touching nothing of itself; every hole wholly inside the outer loop touching nothing of it; no two holes overlapping, touching or one inside the other.

## What a command gives

A command gives a boundary in a plain form, `{"outer":[<corner>, ...],"holes":[[<corner>, ...], ...]}`, each corner `{"id":"<edge ID>","at":[x,y]}` in grid steps, the edge from it to the next; `id` may be left out, for `o<n>` on the outer loop and `h<k>-<n>` on hole `k` (both from 0), and `holes` may be left out. It is written as a new sketch, the outer loop's points and lines first, then each hole's, and stored in the form above.

The server's commands (`floor.create`, `floor.boundary`: construct-model's FORMAT.md, "Commands") take the same form with each `at` as two typed lengths (`["8 m", "4000"]`), read in the document's units and turned into grid steps before this crate reads it, so a person never types steps. A boundary they take also has an area of a whole number of square grid steps; typed lengths, on the hundredth of a millimetre at the finest, always give one.

## What a builder takes

Before it is built, a floor's recipe is filled from its type (construct-regen's `slabs`): its `args` become exactly `loops` and `layers`. `loops` is the outer loop anticlockwise seen from above, then each hole clockwise, a loop drawn the other way run backwards from its first corner, each corner `{"id":"<edge ID>","at":[x,y]}` in grid steps with the edge from it to the next. `layers` is the type's layers from the top down, each `{"function":"<function>","material":"<material ID>","thickness":<grid steps>}`. This form is never stored in a record: it is what a builder is handed, and its bytes go into no hash but the built shape's.

## What is worked out and never stored

A floor's area (the shoelace sum over grid steps, holes subtracted), its perimeter (every edge of every loop, each to the nearest step), its thickness, its volume (per layer, the area by the layer's thickness) and its mass (per layer, from the material's density where the project's materials hold one) are its read-only parameters `floor.area`, `floor.perimeter`, `floor.thickness`, `floor.volume` and `floor.mass` (kilograms), worked from the record each time. Each layer's volume and mass are `floor.layer-<k>.volume` and `floor.layer-<k>.mass` (kilograms), layer `k` counted from 1 at the top, named `Layer <k> (<function>, <material>) volume` and `... mass (kg)`. A mass has no value while the library lacks the layer's material, and the floor's none while any layer's has none. The server's parameters view lists them after the parameters a floor's category has, each shown in the document's units, with `from` `read-only`.

## Later versions

Arc edges and constraints in the sketch are held already and refused; allowing them takes a check away and changes no byte. A core in a floor type, sloped floors and slab edges will be new versions here, read beside version 1 for ever.
