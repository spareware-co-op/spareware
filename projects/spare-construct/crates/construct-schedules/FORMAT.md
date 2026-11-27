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

A field key is one of: `name` (the element's name, text), `type` (the name of its type, text, empty when it has none), `parameter:<id>` (a parameter by its definition's permanent ID, of any kind), `quantity:<name>` (a read-only quantity the product works out and never stores, by the name it shows it under). A comparison is one of `equals`, `not_equals`, `less`, `less_or_equal`, `greater`, `greater_or_equal`, `contains`, `begins_with`.

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
