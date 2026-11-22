# CON-A2-F3-P1: Stored values are exact whole units, lengths in the kernel's grid steps

**Status, 2026-11-23:** Decided for the build (piece A39). Stored values are exact whole units; floating-point numbers are only for display and sums. A length is a whole number of the geometry kernel's grid steps, `Q0`, 1/6,400 of a millimetre. This replaces the first form of this note (22 Nov 2026), which stored whole millimetres; that ruling is withdrawn, for the reasons below. `construct-params`, `construct-solver` and `construct-formulas` (one units system, below) follow it; [construct-params' FORMAT.md](../crates/construct-params/FORMAT.md) (version 2) is the stored form of a value.

## The question

The map said "SI internally, metres in f64" ([ARCHITECTURE.md](../ARCHITECTURE.md), Data model) and part CON-A2-F3-P1 said "SI units inside in f64". A parameter value is an input to the edit log, hashed with its record, so it must be exact and print one way. The first answer was whole millimetres, matching construct-model's stand-in recipes. Whole millimetres turned out too coarse for a building program:

- A wall at an angle with an exact length cannot have whole-millimetre ends. The sketch solver rounded every solved point to a whole millimetre and refused anything that then missed by more than 0.01 mm, so an arc ended at 45 degrees on a 1 m radius, an equilateral triangle of 1 m sides and a circle tangent to a sloped line were all refused.
- Everyday sizes could not be stored: 12.5 mm plasterboard, 6.38 mm laminated glass, 0.55 mm steel sheet, a 0.25 mm membrane.

## The decision

- A stored length is a whole number of the kernel's grid steps `Q0`: 156.25 nanometres, 1/6,400 mm ([geometry architecture chapter 6.1](../../engines/geometry/architecture/06-numbers.md); `GridStep::Q0` in `engine-geometry`, where `Q0.steps_per` gives 6,400 to the millimetre). A signed 64-bit integer.
- An area is a whole number of square steps (40,960,000 to the square millimetre) and a volume of cubic steps (262,144,000,000 to the cubic millimetre), each a signed 128-bit integer: 64 bits of cubic steps would not hold one cubic metre. A length times a length is then an area exactly, and an area times a length a volume.
- A stored angle is a whole number of the kernel's angle units, 1/2,500 of an arc-second ([chapter 17](../../engines/geometry/architecture/17-survey-terrain-civil.md), "Angles are whole numbers"), as before.
- A slope is a ratio of two whole numbers in lowest terms; a number without a unit is an exact decimal, as before.
- Typed text is never rounded onto the grid: a length that is not a whole number of steps (0.001 mm is 6.4 steps) is refused, naming the nearest values below and above. A display format shows at most 0.01 mm, so every value shown is on the grid and reads back.
- The sketch solver takes and gives points and radii in grid steps, rounds each solved value to the nearest step (ties to even, the kernel's rule), and checks every constraint against the model tolerance, 0.01 mm (64 steps), after rounding, as before.
- The stored spelling names the step (`length:q0:15360000`), so a version 1 value in whole millimetres (`length:2400`) can never be misread as steps: it is still read, exactly, as millimetres, and written back in the new spelling.
- A box in the spatial index stays in whole millimetres, construct-model's `Bounds`: it is a cache, never the truth, and a box made from grid geometry is rounded outward (the low corner down, the high corner up to the next whole millimetre), so it still holds its shape and a query never misses an element. construct-model's stand-in recipes and placements stay whole millimetres, every one an exact number of steps. Printed pen widths stay whole micrometres: they are sizes on paper, not model lengths.

## One units system (piece A47, 23 Nov 2026)

The program has one units system, so a length in one part of it can never be added to a length in another and be wrong.

- `construct-params` owns quantities, kinds, units, typed text and display: one `Kind`, one `Length`, one `Angle`, one `Value`, and one unit table, `Unit`, each unit with its exact factor in stored units.
- `construct-formulas` owns the formula language only: it reads a formula, type-checks it against `construct-params`' kinds, and works it out to a `construct-params` value. It depends on `construct-params`, never the reverse. (Pull request #1207 had planned the reverse, `construct-params` on `construct-formulas`; that plan is withdrawn.) Its exact rationals stay inside evaluation, held in the stored units themselves (grid steps, square and cubic steps, angle units), and its answer lands on its kind once, at the end: a length, area or volume rounds to the nearest grid step (square, cubic step), ties to even, exactly as the sketch solver rounds a solved point, and an angle rounds the same way to the angle unit; a number is an exact decimal. Dividing a length is everyday work (three equal panes of a 1000 mm opening are 333.33328125 mm each, the three within one step of 1000 mm); a rounded value is at most half a step, 78 nm, from the exact answer, far inside the 0.01 mm model tolerance, and the rounding is deterministic, so the same formula gives the same bytes everywhere. Only a value a person types off the grid is refused, naming the nearest values (the typed-text rule above, unchanged). A value too large to store, and a division by zero, are refused.
- The unit table holds centimetres, inches and feet (64,000, 162,560 and 1,950,720 steps, all whole) so that formulas written before it existed keep their answers (`2 ft 8 in` is still 812.8 mm). Typed text and display stay metric: typed text does not read them and they are not display units. Turning on imperial typing and display is its own later job: it needs fractions of an inch and a display rule.
- Workshop's parts are built on the kernel's finer grid for parts, `Q1` (25 steps to one `Q0` step); a parameter's value is still `construct-params`' `Length`, which lands on `Q1` exactly.

## Why

- The kernel's own rule. The kernel stores every coordinate as whole grid steps, and its interface asks a program to convert its own units once, at its seam (`geom-op`'s engine module: "Lengths in grid steps ... on `Q0` one millimetre is 6,400 steps exactly"). A length in the same steps is handed over with nothing rounded.
- Exact everyday sizes. 0.01 mm is 64 steps, so every size to two decimals of a millimetre, and 1/256 inch (635 steps), is exact.
- Sloped and curved work is accepted. A solved point lands at most half a step, 78 nm, from the true point on each axis, far inside the 0.01 mm model tolerance, so only a sketch no grid point can meet, such as a line under a micrometre long held parallel to a long sloped one, is still refused.
- Determinism, as before: a record is named by the hash of its bytes, and every machine must replay the log to the same bytes. Whole numbers print one way everywhere; a float's text depends on how it is printed, and sums of floats depend on their order.
- Room enough. 64 bits of steps hold about 1.4 x 10^9 km; the kernel's own frame is 2^40 steps, 171.8 km, which the sketch solver keeps to. A micrometre is not a whole number of `Q0` steps (6.4); work that needs one is a mechanical part on the kernel's finer `Q1` step, not a building length.
