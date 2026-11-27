# Column types and columns, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [element types format](../construct-types/FORMAT.md) whose system block a column type fills, the [cloud format](../construct-model/FORMAT.md) whose element record and recipe a column uses unchanged, the [levels format](../construct-levels/FORMAT.md) whose ties a column carries, the [grids format](../construct-grids/FORMAT.md) whose grid lines a column may stand on, and the [parameter values](../construct-params/FORMAT.md) whose length and angle spellings it uses. **We may stop writing an old version. We never stop reading it.** [`src/stored.rs`](src/stored.rs), [`src/column_type.rs`](src/column_type.rs) and [`src/column.rs`](src/column.rs) are the reader library; the decisions behind it are in [the columns note](../../notes/CON-A5-F1-P1-columns.md).

## A column type

A column type is an element type of the columns category (`1100`) whose system block has the owner tag `column` and one line of compact JSON in one spelling:

```json
{"version":1,"section":{"kind":"rect","width":"length:q0:2560000","depth":"length:q0:2560000"},"material":"concrete-regular"}
```

| Field | Holds |
| --- | --- |
| `version` | `1` |
| `section` | `{"kind":"rect","width":<length>,"depth":<length>}` or `{"kind":"round","diameter":<length>}`, each length stored text (`length:q0:<steps>`) |
| `material` | A construct-materials ID |

Every size is above zero, a whole even number of grid steps (so a column's centre is on a step) and at most 5 m. The fields are in the order shown and nothing else is allowed; a reader refuses another spelling of the same data, an old whole-millimetre length, a version it does not read and a size the rules refuse.

## A column

| Field | On a column record |
| --- | --- |
| `id`, `document` | The column's permanent ID, and its document |
| `kind` | `column` |
| `name` | The name people see; the ID unless given |
| `level`, `placement`, `references` | `0`, `{"x":0,"y":0,"z":0}`, `[]`: the position is in the recipe, in exact steps |
| `props` | `@category`: `1100`; `@type`: its column type's ID; `@level` and `@level.offset`: its base's level and offset (construct-levels' tie); `@top.level` and `@top.offset`: its top's level and offset, only for a column whose top is at a level |
| `recipe` | `owner` `construct`, `schema` 1, `op` `column`, `op_version` 1, and the arguments below |

| Argument | Holds |
| --- | --- |
| `at` | `{"point":[<length>,<length>]}`, or `{"grids":[<grid ID>,<grid ID>],"cross":<n>}`: where the two grid lines cross, `cross` counting their crossings from 0 in order of x then y (a straight line against a straight line crosses once) |
| `turn` | The turn about the column's centre, anticlockwise, a stored angle (`angle:<units>`, 1/9,000,000 of a degree), any value |
| `section` | `{"type":"section"}`: the section is read from the column's type when it is built |
| `height` | `{"tie":"top"}`: the height from the base to the top's level, read when it is built; or a stored length, an unconnected height above the base |

```json
{"id":"C-2a","document":"DOC-1","kind":"column","name":"C-2a","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"column","op_version":1,"args":{"at":{"grids":["G-2","G-A"],"cross":0},"height":{"tie":"top"},"section":{"type":"section"},"turn":"angle:0"}},"props":{"@category":"1100","@level":"L-g","@level.offset":"length:q0:0","@top.level":"L-1","@top.offset":"length:q0:0","@type":"column-concrete-400x400"},"references":[]}
```

A column's base stands at its base level's elevation plus its offset (construct-levels' one function, `height_of`), and its top is its top level's elevation plus its offset, or the unconnected height above the base. The height above zero and at most 100 m is checked when it is built and by every command: a top at its base is `NoHeight`, a top below its base `TopBelowBase`, saying how far. A column on two grid lines stands where they cross as the document stands, so it follows them when they move; two grids that do not cross, or cross fewer times than `cross`, and a grid that is gone fail the build as `BadGrid`, and the column is built again when that is put right.

## The ring the builders take

What is built is derived, never stored: the filled recipe holds only `ring` (the section's corners about the column's position, turned, in grid steps, anticlockwise from above), `height` (a stored length), `material` and `grids` (the grid lines it stands on, for the regenerator's reads). A rectangle has four corners, at half its width and half its depth from the centre, turned. A round section is a regular polygon of 48 corners on its circle (interim: the engine seam's extrusion takes polygons only). A turn rotates each corner's offset from the centre and rounds it once to the grid step, half away from zero: exact for quarter turns, and within half a step plus 2^-24 of a step otherwise (interim, replaced by construct-modify's exact turn).

## What a column reads back

Worked out each time, never stored, as read-only parameters under fixed IDs: `column.height` (length), `column.area` (the section's area as built, in square steps), `column.volume` (the area times the height, exact in cubic steps) and `column.mass` (kilograms to three places, from the type's material in construct-materials' library; none when the library lacks it).

## Rules a reader checks

Reading refuses what writing would not give, so one column has one byte string: a record of another kind; a level, placement or reference; a recipe of another owner, schema, version or operation; arguments other than exactly the four above; a value that is not stored text of the right kind in its one spelling; a position that is neither a point nor two grids; a `height` that is neither the top's tie nor a stored length; a column whose height is its top's tie and that has no top tie.
