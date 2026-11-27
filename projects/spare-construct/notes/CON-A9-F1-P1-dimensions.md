# CON-A9-F1-P1: Linear dimensions are elements bound to references; the value is worked out, never stored

**Status, 2026-11-27:** Decided for the build (piece A81). The crate `construct-dimensions` follows it; the stored form is in its [FORMAT.md](../crates/construct-dimensions/FORMAT.md). This is the data and commands half: the page tool, drawing a dimension in the plan, the section in the two-browser test and the package rebuild are the second half.

## The question

A dimension that is a drawn number goes wrong the moment the model moves. The part's own words ask for "dimensions bound to exact references" that "update when the model moves", and the plan job (7.5) asks that a reference that is deleted is reported by name rather than a wrong number shown.

## The decisions

- **A dimension is an element record of the dimensions category, with no shape.** As a view and a sheet are: records are the truth (main's ruling 7), nothing is built from it, `has_shape` is false for its kind, so regeneration and packages pass it over, and the kind table gives `dimension` its category. It lives in one view, today a floor plan or a ceiling plan.
- **Its references are the model's own references.** construct-model's `Reference` already says "part of another element, by persistent name"; a dimension's entries are of kind `dimension.reference`, their name the part (`wall.face.exterior`, `wall.line`, `grid.line`, `level`, `centre`, `edge.start`, `edge.end`). Nothing new is stored beside them. The parts are named by what they are, so a door that moves along its wall keeps its dimension.
- **The value is never stored.** Each segment is worked out from where its two references are, every time. Where a reference is, is asked of the model behind a trait of ours (`Locator`), so the same tests judge any model: a point is read at the foot of the perpendicular to the offset line, a line where it meets it. The server reads the document's records for the answer (a layered wall's filled recipe and its face offsets, a grid's recipe, a hosted element's offset along its wall and its opening's width, a family's solved opening), at the log's head.
- **The arithmetic is exact.** A position along the offset line is a fraction of whole numbers; the distance between two is the square root of a fraction. Whole numbers of any size (`num-bigint`, MIT OR Apache-2.0, pinned) and construct-params' own `Ratio` hold them, and the nearest whole grid step is taken once, at the end; each segment says whether that step is the true value. Horizontal and vertical dimensions between references on whole steps are always exact. A centre that falls on half a step is the step above (a bound of 78 nm): the one place a locus is rounded.
- **Layout is data too.** A horizontal dimension stores the y of its offset line, a vertical one the x, an aligned one a point and a direction. Nothing moves the line when a reference moves: a person moves it (`dimension.move`).
- **A reference that cannot be used is a warning, in the shape construct-warnings uses.** Kind (`dimension.reference_missing`, `dimension.reference_parallel`, `dimension.out_of_range`), severity, the elements (the dimension, then the one referenced) and a sentence naming the reference by position. The segments it ends have no value; the others keep theirs. construct-warnings (A77) is not on main, so the crate repeats its shape field for field and converts one to one when both are there (the header of `warning.rs`).
- **Equality ("EQ") is stored as data and not enforced.** Groups of segments that are to be equal are kept, and the measurement reports whether each group is equal now by the exact values. Enforcing them means moving elements, which needs the solver on elements; the part's words say "equality", not "driven by". While equalities are stored, references are not added or removed (the numbers of the segments would change meaning).
- **Display text is construct-params'.** The value formatted with the person's `Units` through `Definition::format`. Drawing the text waits for job 7.4: what it will answer (the size of a piece of text) is behind `TextSize`, and `Unmeasured` answers until then, so a label's `fits` is not known rather than guessed.
- **Commands, through the log.** `dimension.create`, `dimension.reference.add`, `dimension.reference.remove`, `dimension.move`, `dimension.equal`, `dimension.delete`: one undo step each, claims on the piece written, refusals in the crates' words, a change that changes nothing writes nothing. The generic element commands are refused for a dimension, naming the command to use. A view that holds dimensions is not deleted. An undo that would leave a dimension in a view that is not there is turned back. Deleting an element a dimension names is not blocked.
- **A read-only view.** `GET .../views/{view}/dimensions` lists a view's dimensions with each segment's value (stored length text and the person's units), its witness lines, its stored equalities and its warnings, as of the log's head.

## Why

- Binding to the part by its name, and reading its place each time, is what lets a dimension follow a moving grid or a door sliding along its wall, and report a deleted wall instead of showing a stale number.
- Exact whole-number arithmetic means a dimension agrees with the kernel to the step; the rounding is in one place and flagged.
- A trait for the model's answer means the exact kernel, a section's frame (job 8.1) and a text engine (job 7.4) each replace one place and no caller.

## Not decided here

Angular, radial, arc and ordinate dimensions, spot levels and slopes (A9-F1-P2), auto dimensions (P3); dimension styles and text overrides (they come with the text of job 7.4); dimensions in sections and elevations, where a level has a place (a section's frame is job 8.1's); enforcing equality; the page tool and the drawing in the plan.
