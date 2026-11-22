# Spare Construct parameter values, version 2

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) it sits in. **We may stop writing an old version. We never stop reading it.** [`src/value.rs`](src/value.rs), [`src/definition.rs`](src/definition.rs) and [`src/units.rs`](src/units.rs) are the reader library. Why every stored number is whole, and why a length counts the geometry kernel's grid steps: [note CON-A2-F3-P1](../../notes/CON-A2-F3-P1-stored-units.md).

## A value

A value is stored as text, `<kind>:<payload>`. Each value has exactly one spelling: reading checks that writing the value again gives the same text, so the same values always give the same bytes and the same record hash.

A length counts grid steps of the geometry kernel: its standard step `Q0`, 156.25 nanometres, 1/6,400 of a millimetre (geometry architecture chapter 6.1). A millimetre is 6,400 steps and 0.01 mm is 64, so every size to two decimals of a millimetre is exact, and a length goes to the kernel as it is stored. An area counts square steps (40,960,000 to the square millimetre) and a volume cubic steps (262,144,000,000 to the cubic millimetre), so a product of lengths is exact. The `q0:` in their payloads names the step, so a version 1 number of millimetres can never be read as steps.

| Kind | Payload | Example |
| --- | --- | --- |
| `length` | `q0:` and whole grid steps, as a signed decimal integer with no plus sign, no leading zero and no `-0` | `length:q0:15360000` (2,400 mm), `length:q0:80000` (12.5 mm) |
| `area` | `q0:` and whole square grid steps, the same way | `area:q0:235929600000000` (5.76 m²) |
| `volume` | `q0:` and whole cubic grid steps, the same way | `volume:q0:393216000000000000000` (1.5 m³) |
| `angle` | Whole angle units of the geometry kernel, 1/2,500 of an arc-second (9,000,000 to the degree; geometry architecture chapter 17), as a signed decimal integer as for `length` | `angle:405000000` |
| `slope` | `rise/run` in lowest terms, the run above zero, a flat slope `0/1` | `slope:1/40` |
| `number` | An exact decimal with no trailing zeros, no plus sign and no `-0`, at most 28 places | `number:2.5` |
| `whole` | A whole number, as for `angle` | `whole:3` |
| `text` | The text itself, which may hold anything, colons included | `text:D01` |
| `yesno` | `yes` or `no` | `yesno:yes` |
| `choice` | The key of one of the definition's choices, never its label | `choice:oak` |
| `element` | Another element's permanent ID | `element:01JD0000000000000000000000` |
| `material` | A material's permanent ID | `material:concrete-30` |
| `link` | An http or https address as the WHATWG URL standard serialises it | `link:https://example.org/` |

A length, an angle and a whole number fit a signed 64-bit integer; an area and a volume a signed 128-bit one (a cubic metre is about 2.6 x 10^20 cubic steps). No value holds a floating-point number.

### Version 1

Version 1 (22 Nov 2026, before any record was written with it) stored a length, area and volume as whole millimetres, square millimetres and cubic millimetres, with no `q0:`: `length:2400`, `area:5760000`, `volume:1`. A payload with no `q0:` is read as version 1, in its one spelling (no plus sign, no leading zero, no `-0`), and converted exactly: 2,400 mm is 15,360,000 steps. Writing it again gives version 2's spelling, so the record's bytes change once. A version 1 length too large for 64 bits of steps (over about 1.4 x 10^15 mm) is not a stored value. Every other kind's spelling is the same in both versions.

## On an element

An element record's `props` (the cloud format, "An element") holds a value under its parameter's permanent ID: `{"H":"length:q0:13440000","M":"text:D01"}`. `props` is sorted by key. A key that is no parameter's ID is not a parameter value, and is kept and read as it was.

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

How values are shown is a JSON row of a project or view, one `{"unit","places"}` per measured kind, in this order: `length`, `area`, `volume`, `angle`, `slope`. Units: `mm`, `m`, `mm2`, `m2`, `mm3`, `m3`, `degree`, `percent`, `ratio`. Places: 0 to each unit's most, so that a shown value is always a whole number of steps and reads back: `mm` 2, `m` 5 (0.01 mm, the model tolerance), `mm2` 4, `mm3` 6, every other unit 9. A shown value is rounded half to even to its places. Typed text is never rounded to the grid: a length, area or volume that is not a whole number of steps (0.001 mm) is refused, naming the nearest value below and above. Display settings never change a stored value.
