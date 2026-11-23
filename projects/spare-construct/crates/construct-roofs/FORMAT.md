# Roof types and roofs, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [floor format](../construct-slabs/FORMAT.md) whose boundary a roof's footprint is and whose layers' text a roof type's block holds, the [element types format](../construct-types/FORMAT.md) whose system block a roof type fills, the [cloud format](../construct-model/FORMAT.md) whose element record and recipe a roof uses unchanged, the [levels format](../construct-levels/FORMAT.md) whose tie it carries and the [parameter values](../construct-params/FORMAT.md) whose length, slope and angle spellings it uses. **We may stop writing an old version. We never stop reading it.** [`src/roof_type.rs`](src/roof_type.rs), [`src/roof.rs`](src/roof.rs) and [`src/record.rs`](src/record.rs) are the reader library; the decisions behind it are in [the roof note](../../notes/CON-A4-F4-P3-simple-roof.md).

## A roof type

A roof type is a type record of category `600` (roofs) whose system block has the owner tag `roof`: the `props` key `@system.roof` holds its layers from the top (outer) face down in exactly a floor type's text ([construct-slabs' format](../construct-slabs/FORMAT.md), "A floor type": compact JSON `{"version":1,"layers":[{"function","thickness","material"}, ...]}`, read and refused by the same rules). Each thickness is measured square to the layer's faces. Its `@params` lists no parameter of its own yet.

```json
{"version":1,"layers":[{"function":"finish","thickness":"length:q0:3520","material":"steel-aisi-1025"},{"function":"membrane","thickness":"length:q0:0","material":"membrane-polyethylene"},{"function":"structure","thickness":"length:q0:960000","material":"timber-douglas-fir"}]}
```

## A roof

A roof is an element record like any other, of the roofs category:

| Field | On a roof record |
| --- | --- |
| `kind` | `roof` |
| `level`, `placement`, `references` | `0`, `{"x":0,"y":0,"z":0}`, `[]`: the footprint's corners are where the roof is |
| `recipe` | `{"owner":"construct","schema":1,"op":"roof","op_version":1,"args":{"edges":<the edges>,"footprint":<the footprint>,"layers":{"type":"layers"}}}` |
| `props` | `@category` `600`, `@type` its roof type's ID, and its base's tie: `@level` and `@level.offset` (construct-levels) |

**Where it sits.** Its bottom face crosses every footprint edge at its base, its level's elevation plus its offset, and its layers stand on the bottom face: in its own frame the base is `z = 0`.

**The footprint** is a floor's boundary in its stored form ([construct-slabs' format](../construct-slabs/FORMAT.md), "A floor", the boundary), read and refused by the same rules, with `holes` always `[]` in version 1.

**The edges** are every edge of the footprint's outer loop once, in the loop's order, each `{"id":"<edge ID>","overhang":"<length>"}` with `"pitch":"<slope or angle>"` added when the roof rises from that edge:

| Field | Value |
| --- | --- |
| `id` | The footprint edge's permanent ID |
| `overhang` | How far the roof runs on past the edge in plan: construct-params' stored length, `length:q0:<steps>`, never below 0 |
| `pitch` | Only on an edge the roof rises from: construct-params' stored slope, `slope:<rise>/<run>` in lowest terms (`slope:1/4`), or stored angle, `angle:<units>` in the kernel's angle units, 9,000,000 to the degree (`angle:270000000` is 30 degrees) |

A lean-to 4 m by 3 m, rising at 1 in 4 from its edge `o0` with 400 mm over it, its base on level `L-2` (every JSON object's keys are in sorted order, as construct-model writes a recipe's arguments):

```json
{"id":"R-1","document":"DOC-1","kind":"roof","name":"Lean-to","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"roof","op_version":1,"args":{"edges":[{"id":"o0","overhang":"length:q0:2560000","pitch":"slope:1/4"},{"id":"o1","overhang":"length:q0:0"},{"id":"o2","overhang":"length:q0:0"},{"id":"o3","overhang":"length:q0:0"}],"footprint":{"holes":[],"outer":[{"id":"o0","line":0},{"id":"o1","line":1},{"id":"o2","line":2},{"id":"o3","line":3}],"sketch":{"constraints":[],"lines":[{"end":1,"start":0},{"end":2,"start":1},{"end":3,"start":2},{"end":0,"start":3}],"points":[{"x":0,"y":0},{"x":25600000,"y":0},{"x":25600000,"y":19200000},{"x":0,"y":19200000}]},"version":1},"layers":{"type":"layers"}}},"props":{"@category":"600","@level":"L-2","@level.offset":"length:q0:0","@type":"roof-steel"},"references":[]}
```

Edges that are not exactly as written are refused, never repaired: an edge out of the footprint's order or missing, a field added, a length or a pitch in another spelling (`slope:2/8`, an old whole-millimetre `length:400`). And a roof that breaks a rule is refused in words naming its edges:

- a footprint with a hole (an opening through a roof is job 4.2's);
- an overhang below 0; a pitch that does not rise (a slope or an angle of 0 or less), or is steeper than 10 in 1 (about 84.3 degrees), or an angle of 90 degrees or more;
- **the form**: no edge with a pitch is a flat roof; one is mono-pitch, rising from it to the opposite edge; two opposite edges at one slope are a gable, rising from both to a ridge midway. Two edges that meet at a corner, three or four are hips ("every edge slopes: hips come with job 6.2"), and two opposite edges at different slopes an uneven gable (job 6.2), both refused;
- a pitch on a footprint that is not a rectangle whose edges run along the plan's x and y axes (job 6.2);
- an overhang on a flat roof whose footprint has an edge off the axes (job 6.2); two edges on one line with different overhangs, or overhangs that fold the eaves outline back over itself;
- a roof that does not rise a grid step to its ridge or its high side.

A slope held as an angle is worked as the slope of rise `round(10^9 tan a)` over a run of 10^9 (30 degrees: `577350269/1000000000`); two edges are at one slope when those slopes are equal.

## What a command gives

A command gives a roof in a plain form, `{"footprint":<boundary>,"edges":{"<edge ID>":{"overhang":<grid steps>,"pitch":"<stored slope or angle>"}, ...}}`, the footprint in construct-slabs' plain form (its corners in grid steps), `edges` optional, each edge's fields optional (no overhang, no pitch), an edge the footprint lacks refused. It is stored in the form above.

## What a builder takes

Before it is built, a roof's recipe is filled from its type (construct-regen's `roofs`): its `args` become exactly `form`, `slopes`, `outline` and `layers`:

| Argument | Value |
| --- | --- |
| `form` | `flat`, `mono` or `gable` |
| `slopes` | The edges the roof rises from, in the footprint's order: none, one or two |
| `outline` | The eaves outline anticlockwise from above, each corner `{"id":"<edge ID>","at":[x,y]}` in grid steps: the footprint edge whose eaves, high side or verge runs from it to the next |
| `layers` | Each layer from the top down, `{"function","material","thickness"}` (its thickness in grid steps, square to its faces) and, for a layer with a thickness, `solid` |

A `solid` is `{"extrude":[[x,y,z], ...],"by":[x,y,z]}` for a flat roof's layer (its outline at its lower face, extruded straight up to its upper face) or `{"loft":[[[x,y,z], ...],[[x,y,z], ...]]}` for a sloped roof's (its section across the roof at one verge and the same section at the other, corner `i` joined to corner `i`: the lower face's corners from the eaves across, then the upper face's back). Every point is in grid steps of the roof's frame.

Where each corner is: each face's upright height above the bottom face is `h * sqrt(1 + s^2)` for a face `h` square to it on slope `s` (`h` itself on a flat roof), the nearest step, a half up; in the frame of the first sloping edge, the eaves are at `-overhang` across the footprint, the high side or far eaves at the far edge plus its overhang, the verges at each end edge's overhang past the footprint; a gable's ridge is at half the footprint's width (the nearest step, a half up) and the nearest step to `s` times that high; each eaves corner is the nearest step to the plane's height there; every face's corners are the bottom face's moved up by its whole height. This form is never stored in a record: it is what a builder is handed, and its bytes go into no hash but the built shape's.

On the kernel's edition each layer with a thickness is a solid of its own (the extrusion or the loft) and its faces are named, under `construct/roof`, `top`, `bottom`, `face.<k>` and `side.<edge ID>` on a flat roof, and `top.<edge>`, `bottom.<edge>`, `face.<k>.<edge>` for each slope (named by the edge it rises from) and `side.<edge ID>` on a sloped one. The stand-in builds the box around the layers, its faces named `box.<role>` for the outer roles; its displays draw each layer.

## What is worked out and never stored

A roof's plan area (its eaves outline's, overhangs included), its slope area (its top face as laid: the plan area times `sqrt(1 + s^2)`, the nearest square step), its thickness, its volume and mass (per layer, each layer's volume its solid's exactly, its mass from the material's density where the project's materials hold one), a sloped roof's pitch (as given) and a gable's ridge height (its top face along the ridge above the base) are its read-only parameters `roof.area`, `roof.slope-area`, `roof.thickness`, `roof.volume`, `roof.mass` (kilograms), `roof.pitch` and `roof.ridge-height`, worked from the record each time, in that order. Each layer's volume and mass are `roof.layer-<k>.volume` and `roof.layer-<k>.mass` (kilograms), layer `k` counted from 1 at the top, named `Layer <k> (<function>, <material>) volume` and `... mass (kg)`. An area or a volume that falls between two whole square or cubic steps is refused; footprints and overhangs on the hundredth of a millimetre never meet it.

## Later versions

Hips, valleys, uneven gables and sloped roofs on other footprints (roof solving), openings, eaves cut square to the slope, and soffits, fascias and gutters will be new versions here, read beside version 1 for ever.
