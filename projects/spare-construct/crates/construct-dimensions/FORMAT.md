# Dimensions, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) whose element record and reference it uses unchanged, the [parameter values](../construct-params/FORMAT.md) it stores and the [views](../construct-views/FORMAT.md) it sits in. **We may stop writing an old version. We never stop reading it.** [`src/dimension.rs`](src/dimension.rs) and [`src/style.rs`](src/style.rs) are the reader library. A document made before dimensions existed has no record of these kinds and reads as having none.

## A dimension

A dimension is an element record of the dimensions category in one view: a linear dimension (a string of segments between references), an ordinate one (each reference's distance from the first), an angular one (the angle between two lines), or a radial, diameter or arc length one (of a curve). It has no shape: nothing builds it, and it holds no value (see "What is not stored").

| Field | On a dimension record |
| --- | --- |
| `id` | The dimension's permanent ID |
| `document` | The document it belongs to |
| `kind` | `dimension` |
| `name` | `Dimension`, always |
| `level`, `placement` | `0`, `{"x":0,"y":0,"z":0}` |
| `recipe` | `{"owner":"construct","schema":1,"op":"dimension","op_version":1,"args":{}}`: builds nothing; its `op_version` is this format's version |
| `references` | The references the dimension is bound to, in the order they are measured (below): two to 64 for a linear or ordinate one, no two neighbours the same; exactly two for an angular one; exactly one for a radial, diameter or arc length one |
| `props` | The entries below |

| `props` key | On | Value |
| --- | --- | --- |
| `@category` | Every dimension | `5000`, the dimensions category ([construct-categories](../construct-categories/FORMAT.md)) |
| `@dimension.view` | Every dimension | The ID of the view it is in. Today a floor plan or a ceiling plan |
| `@dimension.layout` | Every dimension | `choice:horizontal`, `choice:vertical`, `choice:aligned` (an offset line), `choice:angular`, `choice:radial`, `choice:diameter` or `choice:arc_length` |
| `@dimension.y` | A horizontal one | Stored length text: the y of its offset line (measuring along x) |
| `@dimension.x` | A vertical one | Stored length text: the x of its offset line (measuring along y) |
| `@dimension.through.x`, `@dimension.through.y` | An aligned one | Stored length text: a point its offset line goes through |
| `@dimension.direction.x`, `@dimension.direction.y` | An aligned one | Stored length text: a vector, not zero, the way it measures. Only its direction matters |
| `@dimension.first`, `@dimension.second` | An angular one | `choice:forward` or `choice:backward`: which way along its first and its second reference's line the angle opens, `forward` being the way the line runs as its element draws it (a wall from its start to its end, a grid from its first point to its second). The two rays name one of the four angles two crossing lines make |
| `@dimension.radius` | An angular one | Stored length text, above zero: the radius of its arc, about the point where the lines meet |
| `@dimension.toward.x`, `@dimension.toward.y` | A radial or a diameter one | Stored length text: a vector, not zero, the way its line runs from the curve's centre |
| `@dimension.offset` | An arc length one | Stored length text: how far its arc is outside the arc it measures (below zero: inside) |
| `@dimension.ordinate` | An ordinate one | `yesno:yes`. A linear one has no such key. Only with an offset line |
| `@dimension.equal` | A linear one with equalities | `text:` and groups of segment numbers, from 0, as `0,1;3,4,5`: segments to be equal (below) |
| `@dimension.style` | A dimension in a style other than the first built-in one | The style's ID (below). None: the first built-in style, `dimension-style.ticks`, which is never written |
| `@dimension.text` | A dimension with a text in place of its values | `text:` and the text (below) |
| `@dimension.prefix`, `@dimension.suffix` | A dimension with words before or after its values | `text:` and the words |

No other key is allowed, and a record that is not written exactly as the writer would write it is refused on reading. A dimension holds no parameter values. A linear dimension writes none of the keys an ordinate, angular, radial, diameter or arc length one has, nor any of the style and override keys when it has none, so a linear dimension's record is the same bytes as before they existed.

Every length is a `Length`, the kernel's grid step (1/6,400 mm) in the one stored spelling (`length:q0:<steps>`), so 2.5 m is `length:q0:16000000` and nothing is rounded to millimetres. A coordinate or a direction is within 2^40 - 1 steps (171.8 km) of the origin, the grid of [construct-grids](../construct-grids/FORMAT.md).

## References

Each entry of `references` is construct-model's own reference, written as `{"doc":null,"element":<ID>,"name":<feature>,"kind":"dimension.reference","want":"carrier","signature":null}`: the element, and the part of it that is measured, named by what it is and never by a position in a list. Only these are read:

| `name` | Measures to |
| --- | --- |
| `wall.face.exterior`, `wall.face.interior` | A finish face of a wall (a layered wall's outermost layers; the old box wall's sides). The exterior is the side on the right walking the wall from start to end |
| `wall.line` | The wall's reference line (its location line, as drawn) |
| `grid.line` | A grid line: a straight one for a linear, ordinate or angular dimension; a curved one (its arc) for a radial, diameter or arc length one |
| `level` | A level: a horizontal plane at its elevation. It has a place in a section or an elevation only, so a plan refuses it today |
| `centre` | The centre plane of a door, a window or a plain opening hosted on a wall, across the wall |
| `edge.start`, `edge.end` | The edge of that opening nearer the wall's start, or nearer its end |

A curved wall's faces and line (job 14.4) will be read as curves under the same names; nothing here changes when they are.

A dimension's own reader accepts any element ID: whether it is there is asked each time the value is worked out, so deleting an element is never blocked by a dimension and never leaves one wrong.

## What is not stored

- **The value.** Each value is worked out from where its references are, every time it is asked, so it can never be out of date. Where a reference is, is asked of the model through [`Locator`](src/locate.rs): a *point*, a *line* or a *curve* (a circle's arc: its centre, radius and the angles it runs between, anticlockwise).
  - A linear segment is the distance between where two neighbours meet the offset line (a point at the foot of its perpendicular, a line where it crosses; a line along the offset line has no place along it and is reported), worked in whole numbers of any size; the nearest whole step is taken once, at the end, and the answer says whether it is the true value. A horizontal or vertical dimension between references on whole steps is exact. An ordinate value is the same distance from the first reference, below zero against the line's direction.
  - An angle is between the two rays the record names, from 0 to 180 degrees, in construct-params' angle units (1/9,000,000 of a degree). It is exact when it is a multiple of 45 degrees, which is when the two directions' whole-number cross and dot products are zero or of one size; every other angle between directions of whole steps is irrational in degrees (Niven's theorem), and is the nearest whole unit, within half a unit and 2^-80 of one ([`src/trig.rs`](src/trig.rs) proves the bound).
  - A radius is the curve's own (exact); a diameter twice it; an arc length its radius times its sweep, the nearest whole step, within half a step and 2^-70 of one, never exact. A whole circle has no arc length.
- **A deleted or unusable reference.** It gives a warning, in the shape construct-warnings (piece A77) gives (`kind` `dimension.reference_missing`, `dimension.reference_parallel` or `dimension.out_of_range`; `severity` `warning`; `elements` the dimension, then the element referenced; `words` a sentence naming the reference by its position and what it is), and what depends on it has no value. A curve where a straight line is needed (`a curved grid has no straight line to measure to`) and a straight line where a curve is needed are such references.
- **The witness lines, the arcs, the leaders and the text's place.** All follow from the references and the layout, each point within a step of the true one.
- **What a place means.** A person puts an angular, radial, diameter or arc length dimension by a point; the commands work the layout out from it once ([`placed`](src/place.rs)) and store the layout, never the point.

## Equalities

`@dimension.equal` names segments of a linear dimension that are to be equal (the "EQ" idea of a dimension string). Groups list their segments once each, ascending; groups are in order of their first segment and share none; every segment is one the dimension has. [`Measured::equalities`](src/measure.rs) reports whether each group is equal now, exactly (`equal`) and to the grid (`holds`: the values, each the nearest step, within one step of each other).

Stored alone (`dimension.equal`), the groups are data and nothing moves. Made so (`dimension.equalize`), the elements the references are of move in one step, by the rule in [`src/equal.rs`](src/equal.rs): each group's run keeps its two ends; a segment of the run outside the group keeps its length; the group's segments share the rest equally, each reference at the nearest step of its exact place. Moving a reference afterwards moves nothing else: the equality is then reported as not holding until it is made again.

While a dimension holds equalities its references are not added to or taken away (the segments would no longer be the ones named); clear the equalities first.

## Overrides

`@dimension.text`, `@dimension.prefix` and `@dimension.suffix` change what the dimension shows, never its value: each is one line of text, not empty, with no space at either end and at most 120 characters. What is shown is the text in place of each value (or the value), with the prefix before it and the suffix after it, a space between each. They are the dimension's: a string's every segment shows them. The true value is still worked out and listed beside what is shown.

```json
{"id":"D-1","document":"DOC-1","kind":"dimension","name":"Dimension","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"dimension","op_version":1,"args":{}},"props":{"@category":"5000","@dimension.first":"choice:backward","@dimension.layout":"choice:angular","@dimension.radius":"length:q0:5120000","@dimension.second":"choice:forward","@dimension.view":"V-1"},"references":[{"doc":null,"element":"G-1","name":"grid.line","kind":"dimension.reference","want":"carrier","signature":null},{"doc":null,"element":"G-2","name":"grid.line","kind":"dimension.reference","want":"carrier","signature":null}]}
```

## A dimension style

A dimension style says how a dimension is drawn and how its values read. Two are built in and are never records: `dimension-style.ticks` ("Ticks": a tick at each witness line, the text above the line, the project's units, a 1.5 mm gap, a 2 mm extension, 2.5 mm text, a centre mark) and `dimension-style.arrows` ("Arrows": the same with arrowheads). Their IDs cannot be another style's. A project's own style is an element record of the dimensions category with no shape:

| Field | On a style record |
| --- | --- |
| `id` | The style's permanent ID |
| `document` | The document it belongs to |
| `kind` | `dimension.style` |
| `name` | Its name: 1 to 80 characters, no space at either end, unique among the document's styles and the built-in ones, case aside |
| `level`, `placement` | `0`, `{"x":0,"y":0,"z":0}` |
| `recipe` | `{"owner":"construct","schema":1,"op":"dimension.style","op_version":1,"args":{}}` |
| `references` | None |

| `props` key | Value |
| --- | --- |
| `@category` | `5000` |
| `@dimension.style.marker` | `choice:tick`, `choice:arrow` or `choice:dot`: the mark where each witness line meets the dimension's line |
| `@dimension.style.text` | `choice:above` (the text above the line, turned along it) or `choice:inline` (in the line, which is broken for it) |
| `@dimension.style.length.unit`, `@dimension.style.length.places` | Both or neither: `choice:mm` or `choice:m`, and `whole:` the places (at most the unit's: 2 for mm, 5 for m). Neither: lengths as the project shows them |
| `@dimension.style.angle.places` | `whole:` the places angles are shown to in degrees (at most 9). None: as the project shows them |
| `@dimension.style.gap` | Stored length text, 0 to 50 mm: how far a witness line stops short of its reference, as printed on paper |
| `@dimension.style.extension` | Stored length text, 0 to 50 mm: how far a witness line runs past the dimension's line, on paper |
| `@dimension.style.height` | Stored length text, above 0 and at most 50 mm: how high the text is, on paper |
| `@dimension.style.centre` | `yesno:yes` or `yesno:no`: whether a radius or a diameter marks its curve's centre |

No other key is allowed, and a record not written as the writer writes it is refused. A dimension naming a style that is not there is drawn in the first built-in style; the commands never leave one so (`dimension.style.delete` is refused while a dimension is in the style, and an undo that would leave one so is turned back).

```json
{"id":"S-1","document":"DOC-1","kind":"dimension.style","name":"Site, metres","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"dimension.style","op_version":1,"args":{}},"props":{"@category":"5000","@dimension.style.angle.places":"whole:0","@dimension.style.centre":"yesno:no","@dimension.style.extension":"length:q0:19200","@dimension.style.gap":"length:q0:6400","@dimension.style.height":"length:q0:19200","@dimension.style.length.places":"whole:3","@dimension.style.length.unit":"choice:m","@dimension.style.marker":"choice:dot","@dimension.style.text":"choice:inline"},"references":[]}
```

## Commands and the read-only view

The dimension commands are construct-api's `cloud::dimensions` ([construct-model's FORMAT.md](../construct-model/FORMAT.md) lists them): `dimension.create`, `dimension.reference.add`, `dimension.reference.remove`, `dimension.move`, `dimension.equal`, `dimension.equalize`, `dimension.style`, `dimension.override`, `dimension.delete`, `dimension.style.create`, `dimension.style.change` and `dimension.style.delete`, each one undo step. The server lists a view's dimensions with their values worked out, and the document's styles, at `GET /api/projects/{project}/models/{model}/views/{view}/dimensions`: read-only, as of the log's head.
