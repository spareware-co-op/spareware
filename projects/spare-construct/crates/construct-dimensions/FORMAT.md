# Dimensions, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) whose element record and reference it uses unchanged, the [parameter values](../construct-params/FORMAT.md) it stores and the [views](../construct-views/FORMAT.md) it sits in. **We may stop writing an old version. We never stop reading it.** [`src/dimension.rs`](src/dimension.rs) is the reader library. A document made before dimensions existed has no record of this kind and reads as having none.

## A dimension

A dimension is an element record of the dimensions category, a linear dimension in one view: a string of segments between references. It has no shape: nothing builds it, and it holds no value (see "What is not stored").

| Field | On a dimension record |
| --- | --- |
| `id` | The dimension's permanent ID |
| `document` | The document it belongs to |
| `kind` | `dimension` |
| `name` | `Dimension`, always |
| `level`, `placement` | `0`, `{"x":0,"y":0,"z":0}` |
| `recipe` | `{"owner":"construct","schema":1,"op":"dimension","op_version":1,"args":{}}`: builds nothing; its `op_version` is this format's version |
| `references` | The references the dimension is bound to, in the order they are measured: two to 64, no two neighbours the same (below) |
| `props` | The entries below |

| `props` key | On | Value |
| --- | --- | --- |
| `@category` | Every dimension | `5000`, the dimensions category ([construct-categories](../construct-categories/FORMAT.md)) |
| `@dimension.view` | Every dimension | The ID of the view it is in. Today a floor plan or a ceiling plan |
| `@dimension.layout` | Every dimension | `choice:horizontal`, `choice:vertical` or `choice:aligned` |
| `@dimension.y` | A horizontal one | Stored length text: the y of its offset line (measuring along x) |
| `@dimension.x` | A vertical one | Stored length text: the x of its offset line (measuring along y) |
| `@dimension.through.x`, `@dimension.through.y` | An aligned one | Stored length text: a point its offset line goes through |
| `@dimension.direction.x`, `@dimension.direction.y` | An aligned one | Stored length text: a vector, not zero, the way it measures. Only its direction matters |
| `@dimension.equal` | A dimension with equalities | `text:` and groups of segment numbers, from 0, as `0,1;3,4,5`: segments to be equal (below) |

No other key is allowed, and a record that is not written exactly as the writer would write it is refused on reading. A dimension holds no parameter values.

Every length is a `Length`, the kernel's grid step (1/6,400 mm) in the one stored spelling (`length:q0:<steps>`), so 2.5 m is `length:q0:16000000` and nothing is rounded to millimetres. A coordinate or a direction is within 2^40 - 1 steps (171.8 km) of the origin, the grid of [construct-grids](../construct-grids/FORMAT.md).

## References

Each entry of `references` is construct-model's own reference, written as `{"doc":null,"element":<ID>,"name":<feature>,"kind":"dimension.reference","want":"carrier","signature":null}`: the element, and the part of it that is measured, named by what it is and never by a position in a list. Only these are read:

| `name` | Measures to |
| --- | --- |
| `wall.face.exterior`, `wall.face.interior` | A finish face of a wall (a layered wall's outermost layers; the old box wall's sides). The exterior is the side on the right walking the wall from start to end |
| `wall.line` | The wall's reference line (its location line, as drawn) |
| `grid.line` | A straight grid line. An arc has no straight line to measure to |
| `level` | A level: a horizontal plane at its elevation. It has a place in a section or an elevation only, so a plan refuses it today |
| `centre` | The centre plane of a door, a window or a plain opening hosted on a wall, across the wall |
| `edge.start`, `edge.end` | The edge of that opening nearer the wall's start, or nearer its end |

A dimension's own reader accepts any element ID: whether it is there is asked each time the value is worked out, so deleting an element is never blocked by a dimension and never leaves one wrong.

## What is not stored

- **The value.** Each segment's value is worked out from where its two references are, every time it is asked, so it can never be out of date. Where a reference is, is asked of the model through [`Locator`](src/locate.rs): a *point* is read at the foot of the perpendicular from it to the offset line, a *line* where it meets the offset line (a line along the offset line has no place along it and is reported). A segment's value is the distance between the two places along the offset line, worked in whole numbers of any size; the nearest whole step is taken once, at the end, and the answer says whether it is the true value. A horizontal or vertical dimension between references on whole steps is exact.
- **A deleted or unusable reference.** It gives a warning, as construct-warnings' own `Warning` (and the warnings view lists it too; `kind` `dimension.reference_missing`, `dimension.reference_parallel` or `dimension.out_of_range`; `severity` `warning`; `elements` the dimension, then the element referenced; `words` a sentence naming the reference by its position and what it is), and the segments it ends have no value.
- **The witness lines and the text's place.** Both follow from the references and the offset line.

## Equalities

`@dimension.equal` names segments that are to be equal (the "EQ" idea of a dimension string). It is stored as data and **not enforced yet**: nothing moves an element to make them so, because that needs the sketch solver on elements. [`Measured::equalities`](src/measure.rs) reports whether each group is equal now, by the exact values. Groups list their segments once each, ascending; groups are in order of their first segment and share none; every segment is one the dimension has. While a dimension holds equalities its references are not added to or taken away (the segments would no longer be the ones named); clear the equalities first.

```json
{"id":"D-1","document":"DOC-1","kind":"dimension","name":"Dimension","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"dimension","op_version":1,"args":{}},"props":{"@category":"5000","@dimension.layout":"choice:horizontal","@dimension.view":"V-1","@dimension.y":"length:q0:32000000"},"references":[{"doc":null,"element":"W-1","name":"wall.face.exterior","kind":"dimension.reference","want":"carrier","signature":null},{"doc":null,"element":"W-2","name":"wall.face.interior","kind":"dimension.reference","want":"carrier","signature":null}]}
```

## Commands and the read-only view

The dimension commands are construct-api's `cloud::dimensions` ([construct-model's FORMAT.md](../construct-model/FORMAT.md) lists them): `dimension.create`, `dimension.reference.add`, `dimension.reference.remove`, `dimension.move`, `dimension.equal` and `dimension.delete`, each one undo step. The server lists a view's dimensions with their values worked out at `GET /api/projects/{project}/models/{model}/views/{view}/dimensions`: read-only, as of the log's head.
