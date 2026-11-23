# CON-A7-F1-P2: recipe evaluation, first half

Part of CON-A7-F1-P2 (recipe evaluation, [FEATURES.md](../FEATURES.md)), its first half: job 4.4 of the build plan, piece A57. The second half (constrained sketches solved inside a recipe, type tables, limits) is job 5.5; a hosted family cutting its host is job 6.4. What a recipe gives when evaluated is written in [construct-families' FORMAT.md](../crates/construct-families/FORMAT.md), "Evaluation"; the code is `construct_families::evaluate`.

## The questions

A recipe ([the format note](CON-A7-F1-P1-recipe-format.md)) holds parameters, formulas, sketches with constraints, and solid steps. Evaluating it for one type means working out the values, turning each sketch into a profile, and running the steps through the kernel's `Engine` interface, kept per type. Four things the format leaves to evaluation had to be decided: what this half does with a constrained sketch it may not solve; the order things are worked in; what a void cuts; and what the cache is keyed by.

## Evidence

- The format has no void flag and no "cut everything before" step: its steps are `extrude`, `subtract` (a target and a tool, both earlier steps) and `unite`, the operations the `Engine` trait names. A void is therefore an extrusion that a `subtract` uses as its tool.
- The single-swing door's elevation sketch holds its widths and heights as constraints with formula lengths (`width / 2`, `height`). Its stored points are drawn at 820 by 2040 mm, the default type. Any other width moves points, which only a solve can do.
- construct-solver already measures every constraint exactly from integer differences, cross and dot products on grid points, within the model tolerance (0.01 mm, 64 steps), after it rounds a solve's answer. That same measure answers "does the sketch as stored meet its constraints at these values?" without solving; it is exposed as `construct_solver::check_drawn`, so the measure is written once.
- construct-formulas works out a table of parameters in any order with loops refused and units checked, and reads a single length formula against given values (`evaluate`, `value_of`): the checker of the format already uses both, and evaluation reuses its helpers.
- The `Engine` trait takes a profile's corners either way round and any vector; the test engines and solidcore's engine are judged by one suite.

## Decision

- **Sketches are taken as stored, and checked.** A sketch a step uses is built as the solver's sketch (points, lines, arcs before circles, constraints, each one to one, every constraint length its formula's value in grid steps) and checked with `check_drawn`. If its stored geometry does not meet a constraint at the type's values, evaluation is refused with `Reason::NotSolved`, at that constraint, saying solving inside a recipe is not built yet. It is never built from geometry that does not match its values. Job 5.5 hands the same solver sketch to `construct_solver::solve` instead and takes the solved points: nothing else changes. Sketches no step uses (the door's plan symbols) are not read.
- **Order.** Values first (each type value checked as construct-params checks it, defaults for the rest, then every formula, refused at the first parameter in recipe order whose own formula fails); then the steps in recipe order, each sketch read the first time a step uses it. The engine is checked first: it must work on `Q0`, the step construct-params' lengths are in.
- **Heights.** An extrusion runs from the lower of `start` and `end` to the higher, along the plane's normal (`xy` +z, `xz` +y, `yz` +x); equal heights are refused. A corner lies at the plane's offset plus the lower height.
- **What a void cuts.** Exactly the target its `subtract` step names, nothing else. A step a later step uses (a target, a tool, a united part) is that step's input and is not drawn; every other step is one of the family's solids, in recipe order.
- **The cache key.** The recipe version's content id, then every parameter without a formula in recipe order with the type's value or its default in stored text, then any other value given (which evaluation refuses, so it is never kept). No clock, address or hash order: the same inputs give the same key everywhere. Two types with the same values share their solids; a changed value is a new key and builds only that type. A refusal is not kept. Evaluations are handed out shared (`Arc`), so the engine stays free to mesh them.
- **Limits** go in one place, `evaluate::Limits`, which sets none in this half: formulas keep construct-formulas' own caps, a sketch is only checked, and the engine is asked once per step. Cache eviction is also job 5.5's.
- **Errors** are values: the step being built, the place (type, parameter, plane, sketch, sketch item, constraint or step) and a reason with text for a person.
- No method was added to the `Engine` trait and no new engine operation is asked for, so the interim register gains no row.

## What the next jobs need

- Job 5.5: replace the `check_drawn` call in `src/evaluate/sketch.rs` with a solve (budget from `Limits`); the solved points take the stored ones' place for the corners. Add limits as `Limits` fields. Built-in types are already read by `TypeValues::of`.
- Job 6.4: the hosted opening (`hosting.opening`) is not evaluated here. Its sketch goes through the same `resolve`, and the cut belongs to the host's build (construct-regen's kernel builder).
- A real engine: `tests/evaluate_suite/mod.rs` is generic over `Engine`; the program that links an engine (the server, under its `kernel` feature) includes it through `#[path]`, as it does construct-regen's kernel suite. That is the wiring job's, since it touches the server.

## Open risks

- Until job 5.5, only types whose values leave every used sketch as drawn evaluate: of the door's three built-in types, only the 820 mm one.
- The fixed tolerance check may pass a sketch whose constraint misses by less than 0.01 mm at a type's values; the solids then use the stored points, which are within that tolerance of the exact ones.
