# CON-A2-F3-P1: Stored values are exact whole units

**Status, 2026-11-22:** Decided for the build (decision 3 of the build plan). Stored values are exact whole units; floating-point numbers are only for display and sums. The crate `construct-params` follows it, and its [FORMAT.md](../crates/construct-params/FORMAT.md) is the stored form.

## The question

The map said "SI internally, metres in f64" ([ARCHITECTURE.md](../ARCHITECTURE.md), Data model) and part CON-A2-F3-P1 said "SI units inside in f64". The cloud format already rules that numbers in a recipe are whole millimetres ([construct-model's FORMAT.md](../crates/construct-model/FORMAT.md), "An element"), and the geometry kernel stores lengths and angles as whole numbers. A parameter value is an input to the edit log, hashed with its record, so the two rules could not both hold.

## The decision

- A stored length is a whole number of millimetres, an area of square millimetres and a volume of cubic millimetres, each a signed 64-bit integer.
- A stored angle is a whole number of the kernel's angle units, 1/2,500 of an arc-second ([geometry architecture chapter 17](../../engines/geometry/architecture/17-survey-terrain-civil.md), "Angles are whole numbers"), so a parameter's angle and a recipe's angle agree.
- A slope is a ratio of two whole numbers in lowest terms; a number without a unit is an exact decimal.
- No stored value holds a floating-point number. Display units, rounding (half to even, to a stated number of places) and sums worked for display are a view of the stored value and never change it.

## Why

- Determinism: a record is named by the hash of its bytes, and every machine must replay the log to the same bytes. Whole numbers print one way everywhere; a float's text depends on how it is printed, and sums of floats depend on their order.
- One rule, not two: FORMAT.md already stores recipe numbers as whole millimetres, and the kernel rounds onto whole steps. A parameter that drives a recipe holds the same number the recipe does.
- Nothing is lost that a parameter needs: a millimetre is the smallest size a drawing or schedule shows, and 64 bits hold a length of over 9 million million kilometres. Finer geometry lives on the kernel's own grid, not in parameters.
