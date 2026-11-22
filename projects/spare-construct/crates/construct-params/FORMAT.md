# Spare Construct parameter values, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) it sits in. **We may stop writing an old version. We never stop reading it.** [`src/value.rs`](src/value.rs), [`src/definition.rs`](src/definition.rs) and [`src/units.rs`](src/units.rs) are the reader library. Why every stored number is whole: [note CON-A2-F3-P1](../../notes/CON-A2-F3-P1-stored-units.md).

## A value

A value is stored as text, `<kind>:<payload>`. Each value has exactly one spelling: reading checks that writing the value again gives the same text, so the same values always give the same bytes and the same record hash.

| Kind | Payload | Example |
| --- | --- | --- |
| `length` | Whole millimetres, as a signed decimal integer with no plus sign, no leading zero and no `-0` | `length:2400` |
| `area` | Whole square millimetres, the same way | `area:5760000` |
| `volume` | Whole cubic millimetres, the same way | `volume:1500000000` |
| `angle` | Whole angle units of the geometry kernel, 1/2,500 of an arc-second (9,000,000 to the degree; geometry architecture chapter 17), the same way | `angle:405000000` |
| `slope` | `rise/run` in lowest terms, the run above zero, a flat slope `0/1` | `slope:1/40` |
| `number` | An exact decimal with no trailing zeros, no plus sign and no `-0`, at most 28 places | `number:2.5` |
| `whole` | A whole number, as for `length` | `whole:3` |
| `text` | The text itself, which may hold anything, colons included | `text:D01` |
| `yesno` | `yes` or `no` | `yesno:yes` |
| `choice` | The key of one of the definition's choices, never its label | `choice:oak` |
| `element` | Another element's permanent ID | `element:01JD0000000000000000000000` |
| `material` | A material's permanent ID | `material:concrete-30` |
| `link` | An http or https address as the WHATWG URL standard serialises it | `link:https://example.org/` |

Every whole number fits a signed 64-bit integer. No value holds a floating-point number.

## On an element

An element record's `props` (the cloud format, "An element") holds a value under its parameter's permanent ID: `{"H":"length:2100","M":"text:D01"}`. `props` is sorted by key. A key that is no parameter's ID is not a parameter value, and is kept and read as it was.

## A definition

A parameter's definition is a JSON row, its fields in this order:

| Field | Meaning |
| --- | --- |
| `id` | Its permanent ID |
| `name` | The name people see; never empty |
| `kind` | One of the tags above |
| `holder` | `instance` (each element has its own value) or `type` (one value on the type) |
| `choices` | For `choice` only, `[{"key","label"}]` in the order people see them, keys never empty, no key or label twice; empty for every other kind |

`{"id":"P1","name":"Height","kind":"length","holder":"instance","choices":[]}`

## Display settings

How values are shown is a JSON row of a project or view, one `{"unit","places"}` per measured kind, in this order: `length`, `area`, `volume`, `angle`, `slope`. Units: `mm`, `m`, `mm2`, `m2`, `mm3`, `m3`, `degree`, `percent`, `ratio`. Places: 0 to 9. A shown value is rounded half to even to its places. Display settings never change a stored value.

## Shared parameter file, version 1

[`src/shared.rs`](src/shared.rs) is the reader library. A shared parameter's permanent ID is a UUID in its one spelling: 36 characters, lower case, hyphenated, not all zero. The caller gives it; nothing here makes one. The file is JSON, its fields in this order: `format` (1) and `parameters`, each `{"group","definition"}` with the definition's own row (above). Parameters are ordered by group, then name, then ID (byte order). Reading checks that writing the parsed file gives the same bytes, so another spelling, another order or a repeated ID is refused. The same ID met again with another kind is refused; with the same fields it changes nothing.

`{"format":1,"parameters":[{"group":"Fire","definition":{"id":"6f1d2c3a-4b5e-4f60-8a71-92b3c4d5e6f7","name":"Fire rating","kind":"text","holder":"instance","choices":[]}}]}`

## Project parameters, version 1

[`src/project.rs`](src/project.rs) is the reader library. Fields in order: `format` (1), `parameters` (each `{"group","definition"}`, `group` null for a parameter only the project has, ordered by ID) and `bindings` (each `{"parameter","holder","default","categories"}`, ordered by parameter ID). `holder` is `instance` or `type`; `default` is null or a stored value of the parameter's kind (above); `categories` are top-level category IDs of construct-categories, ascending, at least one. Reading checks every rule again (no subcategory, a category that can hold the holder, a default of the right kind) against the project's category table, and that writing gives the same bytes.

## Project information, version 1

[`src/info.rs`](src/info.rs) is the reader library. Fields in order: `format` (1), `extras` (definition rows, ordered by ID) and `values` (parameter ID to stored value, ordered by ID). The built-in fields are text parameters with these permanent IDs: `project-info.name`, `project-info.number`, `project-info.client`, `project-info.address`, `project-info.stage`, `project-info.issue-date`. The issue date is stored as `text:YYYY-MM-DD`, a real calendar date. Reading refuses a value under any ID that is not a built-in field or an extra.

