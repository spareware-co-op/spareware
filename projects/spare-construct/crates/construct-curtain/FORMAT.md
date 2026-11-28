# Curtain walls, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [element types format](../construct-types/FORMAT.md) whose system block it fills, the [parameter values](../construct-params/FORMAT.md) whose length spelling it uses, and [construct-model's FORMAT.md](../construct-model/FORMAT.md) ("Curtain walls") for the element record. **We may stop writing an old version. We never stop reading it.** [`src/stored.rs`](src/stored.rs), [`src/edits.rs`](src/edits.rs) and [`src/record.rs`](src/record.rs) are the reader library; the decisions behind it are in [the curtain walls note](../../notes/CON-A4-F6-P1-curtain-walls.md).

Every length below is in construct-params' stored spelling, `length:q0:<steps>` (steps of the length grid, 6,400 to the millimetre; `length:q0:9600000` is 1.5 m).

## A curtain wall type

A type record of category `200` (curtain walls) whose system block has the owner tag `curtain`: the `props` key `@system.curtain` holds this text, compact JSON, no spaces, the fields in exactly this order:

| Field | Value |
| --- | --- |
| `version` | `1` |
| `vertical` | The layout of the lines that run up the wall, standing at distances along it from its start: as below |
| `horizontal` | The layout of the lines that run along the wall, standing at heights above its base |
| `panel` | What each cell holds unless a wall swaps it: a panel, as below |
| `mullions` | `{"interior-vertical":p,"interior-horizontal":p,"border-vertical":p,"border-horizontal":p}`, each a profile as below or `null` for none |
| `crossing` | `vertical` or `horizontal`: which mullion runs through where two cross |

A layout is one of `{"layout":"none"}`; `{"layout":"spacing","spacing":<length>,"justify":"start"|"centre"|"end"}`; `{"layout":"count","count":<1 to 1000>}`; `{"layout":"most","spacing":<length>}` (the fewest equal panels none wider). A spacing is above 0.01 mm.

A panel is one of `{"kind":"glazed","thickness":<length>,"offset":<length>,"material":<id>}`, the same with `"kind":"solid"`, `{"kind":"empty"}`, or `{"kind":"door","family":"sha256:<64 hex>","type":<key>}` (a family recipe of the library by its content id and one of its types by its key). A thickness is above 0, at most 2 m, and an even number of steps (it is centred on its offset); an offset is at most 2 m either way, positive toward the exterior.

A profile is `{"shape":"rectangle","width":<length>,"depth":<length>,"offset":<length>,"material":<id>}`: its width along the face it stands on, its depth across the wall, both above 0, at most 2 m and an even number of steps; its offset as a panel's.

```json
{"version":1,"vertical":{"layout":"spacing","spacing":"length:q0:9600000","justify":"centre"},"horizontal":{"layout":"spacing","spacing":"length:q0:6400000","justify":"start"},"panel":{"kind":"glazed","thickness":"length:q0:153600","offset":"length:q0:0","material":"glass-plate"},"mullions":{"interior-vertical":{"shape":"rectangle","width":"length:q0:320000","depth":"length:q0:960000","offset":"length:q0:0","material":"timber-douglas-fir"},"interior-horizontal":null,"border-vertical":null,"border-horizontal":null},"crossing":"vertical"}
```

## A curtain wall's own edits

The recipe argument `grid` of a curtain wall (construct-model's FORMAT.md, "Curtain walls"), left out when it has none. A JSON object with exactly these fields:

| Field | Value |
| --- | --- |
| `added` | `{"vertical":[<length>...],"horizontal":[<length>...]}`: grid lines the wall adds, each by where it stands |
| `removed` | The same: lines its type lays out that the wall removes |
| `omitted` | `[{"line":"vertical"|"horizontal","at":<length>,"from":<length>}...]`: segments it leaves out, each by its line and the crossing it starts at (`0` for the first) |
| `panels` | `[{"at":[<along>,<up>],"panel":<panel>}...]`: panels it swaps, each by the corner its cell starts at |

Every list is in order (lengths ascending; segments by line direction, vertical first, then place, then start; panels by along, then up) without repeats.

## Rules a reader checks

A text that breaks one is refused, never repaired: it is the JSON above with no other field, spacing or order; the version is one this reader knows; every length is in its one spelling; every value passes its check (a spacing, a count, a size, a door's family and type); a grid with no edits is no argument, not an empty object; lists are in order without repeats. Writing what was read gives the same bytes.

## What is not stored

Where each line, panel and mullion stands, every solid, the quantities (length, height, area, panels and their area, glazed area, mullions and their length) and which edits have no place in the grid (a removed line its type no longer lays out, a segment of a line that is gone, a swap of a panel that has merged) are worked out from the text each time and never stored.

## Later versions

A mullion profile other than a rectangle, a grid line's own offset or angle, and curved curtain walls will be new versions here, read beside version 1 for ever.
