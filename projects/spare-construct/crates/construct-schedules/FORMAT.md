# Schedules, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) whose element record it uses unchanged, the [parameter values](../construct-params/FORMAT.md) it stores and the [phase fields](../construct-phases/FORMAT.md) it holds. **We may stop writing an old version. We never stop reading it.** [`src/schedule.rs`](src/schedule.rs) is the reader library.

## A schedule

A schedule is an element record like any other (the records in the database are the truth), of the schedules category. It has no shape: nothing builds it. It holds a query, never rows: the rows are worked out from the document's elements each time they are asked for ([`rows`](src/rows.rs)), so nothing about them is stored and a schedule never goes stale.

| Field | On a schedule record |
| --- | --- |
| `id` | The schedule's permanent ID |
| `document` | The document it belongs to |
| `kind` | `schedule` |
| `name` | The name people see: trimmed, not empty, unique in the document without regard to case |
| `level`, `placement`, `references` | `0`, `{"x":0,"y":0,"z":0}`, `[]` |
| `recipe` | `{"owner":"construct","schema":1,"op":"schedule","op_version":1,"args":{}}`: builds nothing; its `op_version` is this format's version |
| `props` | The entries below, and nothing else |

| `props` key | Value |
| --- | --- |
| `@category` | `7200`, the schedules category ([construct-categories](../construct-categories/FORMAT.md)) |
| `@schedule.lists` | The number of the category the schedule lists, no leading zero: `300`. Elements of that category and of its subcategories are listed |
| `@schedule.fields` | The columns, in order, as JSON text: a list of field keys, no spaces: `["name","parameter:P-mark","quantity:Area"]` |
| `@schedule.filters` | The filters, as JSON text: a list of `{"field":<key>,"is":<comparison>,"to":<value>}`, keys in that order, `to` a value in [construct-params' stored spelling](../construct-params/FORMAT.md) (`"length:q0:5760000"`). A row is listed only when every filter holds |
| `@schedule.sorting` | The sorting, first key first, as JSON text: a list of `{"field":<key>,"order":"ascending"}` or `"descending"` |
| `@staging` | The schedule's own staging fields, as the phases format spells them: `{"created":"ph-a","demolished":null,"design_option":null}` |

A field key is one of: `name` (the element's name, text), `type` (the name of its type, text, empty when it has none), `parameter:<id>` (a parameter by its definition's permanent ID, of any kind), `quantity:<name>` (a read-only quantity the product works out and never stores, by the name it shows it under), `calculated:<name>` (one of the schedule's own calculated fields, by its name). A comparison is one of `equals`, `not_equals`, `less`, `less_or_equal`, `greater`, `greater_or_equal`, `contains`, `begins_with`.

The quantities, by the top-level category that has them (the table is [`QUANTITIES`](src/field.rs); a test in construct-api reads each from the product's own rows and compares):

| Category | Quantities, with their kind |
| --- | --- |
| Floors | `Area` (area), `Perimeter` (length), `Thickness` (length), `Volume` (volume), `Mass (kg)` (number) |
| Roofs | `Area`, `Slope area` (area), `Thickness`, `Volume`, `Mass (kg)`, `Ridge height` (length) |
| Rooms | `State` (text: `enclosed`, `not enclosed`, `redundant`, `refused`), `Area`, `Perimeter`, `Volume`; the last three only when the room is enclosed |

Every other category has `name`, `type` and the parameters bound to it.

A record that is not exactly as written is refused, never repaired: a reader names the record and why. The same schedule is always the same bytes. A record whose fields are no longer the category's (a parameter taken off it since) still reads; `Schedule::check` says in words that it no longer stands, and its rows have an empty cell there and no row passes a filter on it.

```json
{"id":"S-1","document":"DOC-1","kind":"schedule","name":"Door schedule","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"schedule","op_version":1,"args":{}},"props":{"@category":"7200","@schedule.fields":"[\"name\",\"parameter:P-mark\"]","@schedule.filters":"[{\"field\":\"parameter:P-sill\",\"is\":\"greater_or_equal\",\"to\":\"length:q0:5760000\"}]","@schedule.lists":"300","@schedule.sorting":"[{\"field\":\"name\",\"order\":\"descending\"}]","@staging":"{\"created\":\"phase-new\",\"demolished\":null,\"design_option\":null}"},"references":[]}
```

## The rows

The rows are not stored and have no format of their own; the server's read-only view gives them as JSON (`GET .../schedules/<schedule>`): the schedule, its `columns` (`field`, `name`, `kind`) and its `rows`, each `{"element", "cells"}` with one cell for each column, `{"value", "shown"}`: `value` the exact value in the stored spelling (or `null`), `shown` the same in the document's units (`"900 mm"`).

## Order

Numbers (lengths, areas, volumes, angles, slopes, decimals, whole numbers) are ordered by exact value. Text is ordered by one rule: lower-cased with Unicode's own mapping, then by the exact text. There is no locale and no natural ordering (`D10` comes before `D2`). A choice, an element link and a material are ordered by their stored key, a yes or no with no first. A row with no value for a sort key comes after every row that has one, in either direction. Rows left level are in element ID order, so the order is total.

## Grouping, itemizing and totals

Added by A94 as optional entries: a schedule that uses none of them is written as it always was, byte for byte, and every older record still reads. **We may stop writing an old version. We never stop reading it.**

- **Grouping.** A sort key with `"header":true` or `"footer":true` groups: the rows that agree on that key and on every key before it are one group, nested in the order of the keys. The header is the key's field name and the group's value as the rows show it (`Level: L1`; empty for rows with no value); the footer is the number of elements in the group and the totals of the columns that have them. A group's value is compared exactly, as a filter compares.
- **Itemize every instance.** On (no entry), every element is a row. Off (`@schedule.itemize` is `no`), the rows of one group that agree in every column are folded into one, where the first of them stood, with a count of the elements it stands for. Whatever is folded, a total counts every element.
- **Totals.** `sum` is of lengths, areas, volumes, angles, numbers and whole numbers, added exactly (construct-params' own addition: a length in grid steps, an area in square steps, a decimal exactly; a sum that does not fit is reported, never wrapped or rounded). `min` and `max` are of those and slopes, by exact value. `count` is of any field: the number of elements with a value there. A total of nothing is empty, not zero (a count is `0`). A total is on a column the schedule lists; a sum of text is refused in words.
- **Grand total.** One footer for the whole table: the number of elements and every column total.

## Calculated fields

A calculated field is a formula over other fields, in construct-formulas' language: `Width * Height` of two lengths is an area, `Width / 2 + 50 mm` a length, `Height / Width` a plain number. Units are the language's: `Width + 2` is refused for want of a unit on the `2`, an area is not a length, and every answer is landed exactly on the kind the field gives (a length on the nearest grid step, half to even). A formula names a field by its name with every run of characters that is not an ASCII letter or digit written as one `_` (`Mass (kg)` is `Mass_kg`), and may name any field of the schedule's category and the calculated fields before it in the list. Text, choices, yes or no, links and materials cannot be used in a formula. The formula is checked with a value of each input's kind, so a wrong unit or an unknown field is refused when the schedule is made, in construct-formulas' words; for a row it is worked out from the row's values, and a row that lacks a value the formula needs, or whose values divide by zero, has an empty cell. A calculated field's name is not any other field's name (case aside) and is not read as one in a formula. A calculated field can be listed, sorted, filtered, grouped and totalled like any other, and is read-only: a value typed into its cell is refused in words.

## Editing a cell

Not a stored form: `schedule.cell` is a command that writes the element. A typed value goes back to the element through the command a person uses on it (a parameter by `element.param`, a name by `element.set`, `sheet.set` for a sheet and `view.rename` for a view), so it is one undo step and the element is claimed. Read-only, refused in words: the name of the type, a parameter the element's type holds (one value for every element of the type), a quantity, a calculated field.

## The sheet list and the view list

A schedule of the sheets category (7100) is the sheet list, and one of the views category (7000) the view list. Their quantities are in [`QUANTITIES`](src/field.rs):

| Category | Quantities, with their kind |
| --- | --- |
| Sheets | `Number` (text), `Paper` (text, as the sheet describes it), `Views placed` (whole number), `Schedules placed` (whole number); the sheet's own parameters (drawn by, checked by, issue date...) are its category's parameters and its name is `name` |
| Views | `Kind` (text: `floor plan`, `section`...), `Scale` (whole number, the `n` of 1:n), `Detail level` (text), `On sheet` (text, the number of the sheet the view stands on; no value for a view on none) |

There is no revision in the model yet, so there is no revision list; it is a schedule of the revisions category when that exists (A11-F1-P4).

## A schedule placed on a sheet

A placement is an element record like a schedule, of the schedules category, with no shape: kind `schedule-placement`, name `Schedule on sheet`, level `0`, placement `{"x":0,"y":0,"z":0}`, no references, recipe `{"owner":"construct","schema":1,"op":"schedule-placement","op_version":1,"args":{}}`.

| `props` key | Value |
| --- | --- |
| `@category` | `7200` |
| `@placement.schedule` | The schedule placed, an ID |
| `@placement.sheet` | The sheet it stands on, an ID |
| `@placement.x` | The top left corner's distance across the paper from its left edge, a stored length |
| `@placement.y` | The top left corner's distance up the paper from its bottom edge, a stored length |

A schedule may stand on several sheets but on one sheet only once. The corner is on the paper. Deleting the schedule or the sheet deletes its placements in the same step. Nothing about the drawing is stored: `draw` ([`src/draw.rs`](src/draw.rs)) lays the live table out each time as text and lines at exact places from the corner, across and down, in grid steps: rows 5 mm tall, text 2.5 mm high, a column as wide as its longest text at 1.5 mm a character plus 1 mm each side.
