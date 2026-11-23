# Levels and ties to them, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) whose element record it uses unchanged and the [parameter values](../construct-params/FORMAT.md) it stores. **We may stop writing an old version. We never stop reading it.** [`src/level.rs`](src/level.rs), [`src/tie.rs`](src/tie.rs) and [`src/implied.rs`](src/implied.rs) are the reader library.

## A level

A level is an element record like any other (the records in the database are the truth), of the levels category. It has no shape: it is a datum, and nothing builds it.

| Field | On a level record |
| --- | --- |
| `id` | The level's permanent ID |
| `document` | The document it belongs to: each model has its own levels |
| `kind` | `level` |
| `name` | The name people see: trimmed, not empty, unique in the document without regard to case |
| `level`, `placement`, `references` | `0`, `{"x":0,"y":0,"z":0}`, `[]` |
| `recipe` | `{"owner":"construct","schema":1,"op":"level","op_version":1,"args":{}}`: builds nothing; its `op_version` is this format's version |
| `props` | The entries below, and nothing else |

| `props` key | Value |
| --- | --- |
| `@category` | `3600`, the levels category ([construct-categories](../construct-categories/FORMAT.md)) |
| `@elevation` | Its height above the project's zero in construct-params' stored length text, version 2's spelling only: `length:q0:20480000` is 3200 mm. Negative below zero |
| `@storey` | `yesno:yes` for a building storey (ISO 16739-1's building storey: a slice of the building), `yesno:no` for a reference datum |

```json
{"id":"L-1","document":"DOC-1","kind":"level","name":"First floor","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"level","op_version":1,"args":{}},"props":{"@category":"3600","@elevation":"length:q0:20480000","@storey":"yesno:yes"},"references":[]}
```

A record that is not exactly as written is refused, never repaired: a reader names the record and why.

## The rules a command checks

Levels are listed lowest first: by elevation, then by name without regard to case, then by ID. Two levels may share an elevation, and at most one of them is a storey there: a storey is a slice of the building, so the storey above another is always one level. A level's name is unique in its document, case aside; a level may change the case of its own name. A level is not deleted while anything is tied to it, and the refusal says how many are. These bind what commands write; reading a document never applies them, so a document always reads.

## A tie

An element is tied to a level by the level's ID and an offset, in its own record's `props`:

| `props` key | On | Value |
| --- | --- | --- |
| `@level` | An element tied to a level | The level's permanent ID |
| `@level.offset` | The same | Its base's offset from the level, in stored length text (`length:q0:0`) |
| `@top.level` | A wall whose top is tied to a level | That level's permanent ID |
| `@top.offset` | The same | The top's offset from it |

Each pair is written together or not at all. An element's base sits at its level's elevation plus its offset (`height_of`, the one function); a wall's height, when its recipe reads it (construct-model's FORMAT.md, `{"tie":"top"}`), is its top's height less its base's. Moving a level writes nothing but the level's record: what is tied to it is built again from its own unchanged record.

A plan view's range is four more ties, under `@view.range.top`, `@view.range.cut`, `@view.range.bottom` and `@view.range.depth`, each with its `.offset` after the key, written together as the pairs above ([`RANGE_TIES`](src/tie.rs); construct-views' FORMAT.md gives them their meaning). A level one of them names is tied to as much as one an element's base is: `tied_to` counts a view once however many of its ties name the level, and a level it counts is not deleted.

## The old integer

Every element record carries the integer field `level`, which before this version was all a level was. It is read so, for ever:

- A record with a tie is on the level of its tie; its integer is not read. Every record written from this version writes `0` there.
- A record with no tie and the integer `0` is on no level. That is what a type, the parameter table, a Workshop part and an element made without a level always carried.
- A record with no tie and any other integer `n` is on the **implied level** of `n`: ID `n` in decimal (`1`, `12`), or `m` followed by the digits of `-n` below zero (`m1`), named `Level n`, a storey, at `(n - 1) x 3000 mm` for `n > 0` and `n x 3000 mm` for `n < 0`. Levels -1, 1 and 2 are at -3000, 0 and 3000 mm.

An implied level has no record. It is listed while anything is tied to it, and a tie naming its ID always finds it. When a command changes it (renames it, moves it, makes it a storey or not), the command writes its record under that ID, and from then on the record is that level; take the record away, as an undo does, and the implied level is back. A record of an ID in the implied form that is not a level (an element with the ID `1`) is not that level: a tie to `1` stands on the implied level, which can then be read and not changed, since changing it would write its record over that element (refused: `That element already exists.`). The rule depends on nothing but the records, so every replay gives the same levels.

## Later versions

A level of one building of a site will carry one more reserved key naming the building; version 1 records read as levels of no particular building. Grids and reference planes are datums of their own format (construct-grids).
