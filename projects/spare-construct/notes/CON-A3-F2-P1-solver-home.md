# CON-A3-F2-P1: where the sketch solver lives for now

Part of CON-A3-F2-P1 (sketch solver, [FEATURES.md](../FEATURES.md)). Status: decided by the build's coordinator on 22 November 2026 for the first version; built in piece CON-A3-F2-P1a. An owner may revisit it when the kernel's solver lands.

## The question

Floors, roofs, rooms, stairs and families need a 2D sketch solver now. Two documents seem to disagree about where it lives:

- This program's [ARCHITECTURE.md](../ARCHITECTURE.md) lists a `solver` crate of its own ("Regeneration and constraints"), and the build order puts the first solver job in `crates/construct-solver`.
- The kernel's [chapter 15](../../engines/geometry/architecture/15-sketches.md) designs the solver as the kernel crate `geom-sketch` (pieces G216 to G228, a scaffold today), and CON-A27-F1-P2 says the kernel's solver is "the same solver as CON-A3-F2-P1". Programs reach geometry only through `engine-geometry` ([EXTENDING.md](../../../EXTENDING.md), section 1).

## Options

1. Wait for `geom-sketch`. Nothing in the first version that needs an outline can start.
2. Build the solver inside the kernel now. That is the kernel's own work, under its exactness rules and its pieces, and is not this program's to do.
3. Build Construct's sketch interface in `construct-solver` now (grid steps in, a report out; whole millimetres until piece A39, note CON-A2-F3-P1), with an interim engine of our own behind it, private to the crate, following chapter 15's method. When `geom-sketch` lands, it answers behind the same types through `engine-geometry` and the interim engine goes.

## Evidence

- No permissively licensed Rust solver fits. Chapter 15.16 read the source of ezpz (MIT) and fiksi (Apache-2.0 OR MIT) at pinned commits and refused both as dependencies: run-time processor dispatch in faer, hash-order plans, panics in library paths and priority levels that drop constraints. Of the crates published since, arael-sketch-solver (MIT) depends on faer and on a clock crate (web-time) in its manifest, and laman (MIT OR Apache-2.0) was three weeks old with a few dozen downloads (crates.io, 22 November 2026). planegcs is LGPL and SolveSpace GPL; neither was read as code.
- Chapter 15 is the written method (no separate A11 note exists; STATUS.md's A11 row is the research it stands for), and it already describes planegcs's dependency groups in its own words.
- The pieces that need a solver soon (wall profiles, floors, roofs, rooms, family sketches) are all on one plane and need points, lines and, next, arcs.

## Decision

Option 3. `construct-solver` holds the public sketch and outcome types and one call, `solve`. The engine is the private module `engine`, written as our own code from chapter 15: least-change damped Gauss-Newton steps, derivatives by dual numbers (num-dual), rank, free motions and dependency groups from a singular value decomposition (nalgebra, default features off, its source read for determinism), a work limit in steps, and a commit to the kernel's grid (whole millimetres until piece A39) with a check against the model tolerance. The crate's header and the module's header say it is interim and name G216 to G228 and CON-A27-F1-P2. Its tests use only the public types, so they judge the kernel's solver on the day it answers.

## Open risks

- The interim engine solves one block with a dense decomposition. It is fine for room and family sketches of a few hundred constraints; a 5,000-segment outline needs chapter 15.5's decomposition and sparse factors, which are the kernel's design.
- Its commit rounds each point to the nearest grid step (1/6,400 mm; whole millimetres until piece A39, note CON-A2-F3-P1) and refuses anything that then misses by more than 0.01 mm. Half a step is far inside that, so sloped shapes are accepted; chapter 15.10's exact forms (integer carriers, depth-one points) are the kernel's and hold every constraint exactly, not within the tolerance.
- Root choices other than a point's side of a line are not stored: a sketch solves to the shape nearest the one drawn. Continuation (15.7) and dragging (15.9) come with arcs in the next job or with the kernel.
