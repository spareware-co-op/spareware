# Wall types, versions 1 and 2

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [element types format](../construct-types/FORMAT.md) whose system block it fills and the [parameter values](../construct-params/FORMAT.md) whose length spelling it uses. **We may stop writing an old version. We never stop reading it.** [`src/stored.rs`](src/stored.rs) and [`src/wall_type.rs`](src/wall_type.rs) are the reader library; the decisions behind it are in [the wall layers note](../../notes/CON-A4-F2-P1-wall-layers.md).

## Where it sits

A wall type is a type record of category `100` (walls) whose system block has the owner tag `wall`: the `props` key `@system.wall` holds the text below, and construct-types stores it untouched. Its `@params` lists `wall.flipped` and `wall.location-line`, the two parameters each wall instance holds.

## The text

Compact JSON, no spaces, the fields in exactly this order:

| Field | Value |
| --- | --- |
| `version` | `1` for a type that wraps nothing, `2` for one that wraps (job 6.1) |
| `core` | `{"exterior":e,"interior":i}`: each a count of layers on that boundary's exterior side, `0 <= e < i <= n` |
| `wrap` | Version 2 only: `{"ends":<ends>,"inserts":<inserts>}`, the type's wrapping at free ends (`none`, `exterior`, `interior`) and at inserts (`none`, `exterior`, `interior`, `both`), not both `none` |
| `layers` | The layers in order from the exterior face (first) to the interior face (last), each as below |

| Layer field | Value |
| --- | --- |
| `function` | `structure`, `substrate`, `thermal` (insulation or an air space), `finish` or `membrane` |
| `thickness` | A length in construct-params' stored spelling, `length:q0:<steps>` (steps of the length grid, 6,400 to the millimetre; `length:q0:80000` is 12.5 mm): never below 0, and 0 only for a membrane |
| `material` | The material's permanent ID ([construct-materials](../construct-materials/src/material.rs)) |
| `wraps` | `true` or `false`: whether it wraps where the type (or a wall's own choice) wraps its side, at free ends and at inserts |

```json
{"version":2,"core":{"exterior":1,"interior":2},"wrap":{"ends":"exterior","inserts":"both"},"layers":[{"function":"finish","thickness":"length:q0:80000","material":"plasterboard-gypsum","wraps":true},{"function":"structure","thickness":"length:q0:576000","material":"timber-douglas-fir","wraps":false},{"function":"finish","thickness":"length:q0:80000","material":"plasterboard-gypsum","wraps":true}]}
```

## Rules a reader checks

A text that breaks one is refused, never repaired: it is the JSON above with no other field, spacing or order; the version is one this reader knows (1 or 2); version 1 has no `wrap` and version 2 has one that wraps something; at least one layer; every thickness a length in its one spelling, not below 0, and 0 only for a membrane; the layers add up to at most 10 m; both boundaries within the layers and the exterior before the interior; at least one core layer thicker than 0; no core layer wraps. Writing what was read gives the same bytes.

## What is not stored

Total thickness, face depths, location lines and offsets are worked from the text each time and never stored. An instance's location line and side are its own parameter values (`choice:<key>` with the keys `wall-centre`, `core-centre`, `exterior-finish-face`, `interior-finish-face`, `exterior-core-face`, `interior-core-face`, and `yesno:yes` or `yesno:no`).

## Wrapping

What the wrapping does is note [CON-A4-F2-P4-wall-wrapping](../../notes/CON-A4-F2-P4-wall-wrapping.md) and [`src/wrap.rs`](src/wrap.rs): a layer whose flag says it wraps turns the corner at a free end, or into a door's or window's opening, on the side the type's wrapping names there, and runs across the end of the core to its far face (to its middle where both sides wrap at an insert). A wall may choose its own wrapping over its type's (its record's `wrapping`, construct-model's FORMAT.md, "Walls").

## Later versions

Anything a later job stores per layer will be a new version here, read beside versions 1 and 2 for ever.
