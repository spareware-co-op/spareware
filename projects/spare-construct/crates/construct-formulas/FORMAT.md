# Spare Construct formulas, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3. **We may stop writing an old version. We never stop reading it.** [`src/parse.rs`](src/parse.rs) is the reader library (its header holds the grammar). One units system: a formula's units, kinds and values are [construct-params](../construct-params/FORMAT.md)' own, and this crate depends on construct-params, never the reverse ([note CON-A2-F3-P1](../../notes/CON-A2-F3-P1-stored-units.md)).

## A formula

A formula is text in the grammar of `src/parse.rs`: numbers, units after a number, parameter names, `+ - * /`, comparisons, `&& || !`, `?:` and the functions `min`, `max`, `abs`, `round`, `floor`, `ceil`. At most 1,000 bytes, 32 brackets deep, 16 choices (`?:`), 30 digits in a number.

- A unit after a number is a symbol of construct-params' unit table (`construct_params::Unit`) that has an exact factor: `mm`, `cm`, `m`, `in`, `ft`, `mm2`, `m2`, `mm3`, `m3`, `deg`. Each is a whole number of the stored unit (a centimetre is 64,000 grid steps, an inch 162,560, a foot 1,950,720, a degree 9,000,000 angle units), so a literal is exact. Centimetres, inches and feet are read here only, so formulas written with them keep their answers; typed text and display stay metric.
- A number with no unit standing alone as the whole formula is in its kind's own unit, fixed: millimetres for a length, square millimetres, cubic millimetres, degrees, or a plain number. It never follows a display setting, so a stored formula never changes its meaning.
- A name is a parameter's name as people see it, when it is letters, digits and `_`, starting with a letter or `_`. Renaming a parameter that a formula uses makes that formula refuse with `UnknownName` until it is written again.

## Its one spelling

Where a formula is stored by this version (a parameter definition's `formula` field, construct-params FORMAT.md, "A definition"), it is in its one spelling, which [`canonical`](src/parse.rs) writes:

- one space either side of each binary operator, `?` and `:`, and after each comma; none inside brackets or after a unary `-` or `!`;
- a space between a number and its unit, and between the parts of a measure: `2 ft 8 in`;
- a number with no leading zero but one before the point and no trailing zero after it: `7.5`, `0.5`, `3`;
- brackets only where they change how the formula reads: `a + (b + c)` keeps them, `(a * b) / c` is `a * b / c`; a unary `+` is dropped;
- no space at either end.

`width/2-4mm` is stored `width / 2 - 4 mm`. The spelling reads back to the same formula and is its own spelling. A stored formula in any other spelling is refused when it is checked or worked out (`Spelling`, naming the one spelling); it is never rewritten silently.

## A value a formula gives

A value a formula gives is a construct-params value and is stored, where it is stored, in construct-params' one spelling for its kind (`length:q0:4753920`); nothing about it says a formula gave it. It is landed on its kind by the rule that kind has for typed text: a length, area or volume must be a whole number of grid steps (square, cubic steps), or the formula is refused (`OffGrid`) naming the nearest values below and above; an angle rounds half to even to the kernel's angle unit; a number must be an exact decimal of at most 28 places; a whole number must be whole; a yes or no comes from a comparison.

## Formulas stored before this version

Workshop's part recipes (the [cloud format](../construct-model/FORMAT.md), `workshop.part`) store each parameter's `value` as written, and are not rewritten: version 1 reads every formula they hold, because the grammar only grew (it added `mm2`, `m2`, `mm3` and `m3` after a number, which the earlier reader refused as text it could not read, so no formula that worked held them). Each answer is the same as before, now as a construct-params value: a recipe's `kind` `length` is construct-params' length, `angle` its angle, and `count` its number kind. The one change is that a length a formula gives must land on the grid: one that falls between grid steps (`1000 mm / 3`), which was rounded before, is now refused naming the nearest two, as typed text is.
