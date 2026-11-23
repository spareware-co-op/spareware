# Grid lines and reference planes, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) whose element record it uses unchanged, the [parameter values](../construct-params/FORMAT.md) it stores its geometry in, and the [categories](../construct-categories/FORMAT.md) it belongs to. **We may stop writing an old version. We never stop reading it.** [`src/record.rs`](src/record.rs) is the reader library.

A grid line and a reference plane are element records like any other (the records in the database are the truth). Their geometry is in the record's recipe, in stored text of a length (`length:q0:<steps>`, the kernel's grid step, 1/6,400 mm: 6,000 mm is `length:q0:38400000`) or of an angle (`angle:<units>`, 1/9,000,000 of a degree), so what is stored is exact and no float is in it. A reader writes none of these texts by hand: `Value::stored()` and `Value::from_stored()` of construct-params do it.

## A grid line

| Field | On a grid record |
| --- | --- |
| `id` | The grid's permanent ID |
| `document` | The document it belongs to |
| `kind` | `grid` |
| `name` | The name people see: trimmed, not empty, at most 64 characters, no control character; unique among the document's grids without regard to case |
| `level`, `placement`, `references` | `0`, `{"x":0,"y":0,"z":0}`, `[]`: a grid is vertical and has no elevation or level of its own (which levels it shows on, and how far it is drawn in each view, are not stored yet) |
| `recipe` | `owner` `construct`, `schema` 1, `op_version` 1, and `op` and `args` below |
| `props` | `@category`: `3700`. Any other key (a parameter's value, a type) is not this format's and is kept as it is |

| `op` | `args` (each value a stored text; keys sorted) |
| --- | --- |
| `grid.line` | `x0`, `y0`: where it starts; `x1`, `y1`: where it finishes (lengths). The two points differ |
| `grid.arc` | `cx`, `cy`: the centre (lengths); `radius`: above zero (length); `from`, `to`: the angles it runs between, anticlockwise from the positive x axis, each from 0 up to but not including 360 degrees, not equal (angles) |

Every coordinate, and an arc's centre plus or minus its radius, is at most 1,099,511,627,775 steps (2^40 - 1, 171.8 km) from the origin, either way.

```json
{"id":"G-A","document":"DOC-1","kind":"grid","name":"A","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"grid.line","op_version":1,"args":{"x0":"length:q0:0","x1":"length:q0:38400000","y0":"length:q0:0","y1":"length:q0:0"}},"props":{"@category":"3700"},"references":[]}
```

```json
{"id":"G-B","document":"DOC-1","kind":"grid","name":"B","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"grid.arc","op_version":1,"args":{"cx":"length:q0:0","cy":"length:q0:0","from":"angle:0","radius":"length:q0:6400000","to":"angle:810000000"}},"props":{"@category":"3700"},"references":[]}
```

## A reference plane

| Field | On a reference plane record |
| --- | --- |
| `id`, `document` | As for a grid |
| `kind` | `refplane` |
| `name` | The name people see, as for a grid, or `""` for a plane with none. Unique among the document's reference planes without regard to case, when it has one |
| `level`, `placement`, `references` | As for a grid |
| `recipe` | `owner` `construct`, `schema` 1, `op_version` 1, and `op` and `args` below |
| `props` | `@category`: `3800`, and any other key as for a grid |

| `op` | `args` |
| --- | --- |
| `refplane.vertical` | `x0`, `y0`, `x1`, `y1`: a line in plan, as for `grid.line`; the plane stands vertical through it |
| `refplane.horizontal` | `z`: the elevation (a length, negative below the project's zero), within the same limit |

```json
{"id":"P-1","document":"DOC-1","kind":"refplane","name":"Mid","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"refplane.vertical","op_version":1,"args":{"x0":"length:q0:19200000","x1":"length:q0:19200000","y0":"length:q0:0","y1":"length:q0:6400000"}},"props":{"@category":"3800"},"references":[]}
```

```json
{"id":"P-2","document":"DOC-1","kind":"refplane","name":"","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"refplane.horizontal","op_version":1,"args":{"z":"length:q0:-1920000"}},"props":{"@category":"3800"},"references":[]}
```

## Rules a reader checks

Reading refuses what writing would not give, so one datum has one byte string: a kind other than the two; a level, placement or reference; a missing or wrong `@category`; a recipe of another owner, schema or version, or an operation of the other kind; arguments other than exactly the keys above; a value that is not stored text of the right kind in its one spelling (the old whole-millimetre `length:6000` reads as a length elsewhere but is not what is written here); a name that is not trimmed; a shape the rules refuse (a line of two equal points, an arc of no radius or no sweep, a coordinate past the limit, an angle outside 0 up to 360 degrees).

Names are not checked when a document is read: the rules bind what commands write, and a document that somehow holds two grids of one name still reads. Two grids may not share a name and two named planes may not, without regard to case; a grid and a plane may. An ID is a grid's or a plane's, never both, and never another element's.

## What is derived from a datum

Its display shape, built from the recipe by construct-regen's datum builder (a derived row, never the truth): the body is the recipe's `op` and `args` as canonical JSON (the same on every machine, so the checkpoints compare it); the display is in whole millimetres, a chain of points no more than 1 mm from the geometry for an arc, each point the nearest millimetre, with a ribbon mesh 3,000 mm high (a stand-in until levels and views give a datum its extent); a horizontal plane has no place in plan and an empty display. A datum has no faces to name.
