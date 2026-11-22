# CON-A4-F2-P1: Wall layers, the core and location lines

**Status, 2026-11-22:** Decided for the build (job 3.4, the data half of CON-A4-F2-P1). The crate `construct-walls` follows it, and its [FORMAT.md](../crates/construct-walls/FORMAT.md) is the stored form.

## The question

A layered wall is the heart of a building model: its type says what it is made of, and every later job (its shape, its joins, its openings, rooms, schedules and drawings) reads that. Job 3.4 had to settle what a wall type holds, how its core is set, where its faces and location lines lie, how layers meet at joins and wrap at ends, and how it is stored, before any geometry is built.

## How layered walls are built and drawn

A wall is read from the outside in: a finish that meets the weather or the room, often a cavity or insulation, a sheet that keeps water or vapour out, the part that carries the load (the core: a frame, blockwork, concrete), then the inner lining. The core is what the wall is structurally; the shells are hung on it. Where walls meet, the load-bearing parts run through and the linings stop against them; at a free end or the side of an opening, linings and wraps turn the corner to cover the core. In a cut plan each layer shows its material's pattern, the lines that bound the core are heavier than the lines between finishes, and a sheet is a single line.

## The decision

- **Layers**, in order from the exterior face to the interior face, each with a function, a thickness, a material and whether it wraps.
- **Functions and priority**: structure (1), substrate (2), thermal: insulation or an air space (3), finish (4), membrane (none). At a join the stronger priority (lower number) runs through and cuts the weaker. A tie is not broken by order or by side: neither cuts, they meet on the join line, and if their materials are the same they are drawn as one. A membrane never cuts and stops at the first layer it meets.
- **Thickness**: construct-params' `Length`, never below 0, and 0 only for a membrane; the layers add up to at most 10 m.
- **Core**: two boundaries, each a count of layers on its exterior side, so there is exactly one contiguous core; it must hold a layer thicker than 0. Refusals name the layer by its index from the exterior face.
- **Location lines**: wall centreline, core centreline, exterior and interior finish faces, exterior and interior core faces. An offset from the reference line is the chosen line's depth less the face's depth, positive toward the exterior side; a flipped wall negates every offset. Which side of a drawn line is the exterior is job 4.1's to name.
- **Halves**: every length is worked in `Length`'s own unit. The two centrelines are a half; where the unit cannot hold that half exactly the line is refused, never rounded. With whole millimetres that refuses the centre of an odd thickness; with the kernel grid unit (piece A39) every length of up to two decimals of a millimetre halves exactly.
- **Wrapping**: one yes or no per layer, the same at free ends and at opening sides. Only a shell layer may wrap; a wrapping layer turns the corner and runs across the end of the core to its far face, and a layer that does not wrap stops flush.
- **Edits** are pure and give a valid type or a refusal. A layer inserted at a boundary goes into the shell beside it; a moved layer takes the zone of the place it lands in; removing a layer closes the boundaries round it.
- **Stored**: the layers and core as canonical JSON in the type's system block (owner `wall`) in construct-types; the location line and the flip are instance parameters `wall.location-line` and `wall.flipped`, so changing a wall's type keeps both and the chosen line stays on the reference line.

## Why

- Priority from the function, not a number per layer, keeps every type's joins consistent: the same function behaves the same in every type.
- A tie left unbroken, and same-material layers drawn as one, gives clean drawings where two walls of one build-up meet, with no rule that depends on which wall was drawn first.
- Boundaries as counts make a split or empty core impossible to write, so only an empty or weightless core needs a refusal.
- No float and no rounding keeps every machine replaying the same offsets, as the stored-units note requires ([CON-A2-F3-P1](CON-A2-F3-P1-stored-units.md)).
- The system block keeps a wall type readable by construct-types alone, and keeps layers out of property lists and schedules, where a text parameter would show.
