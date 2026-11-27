# CON-A4-F2-P4: Layer wrapping at free ends and at inserts

**Status, 2026-11-28:** Decided for the build (piece A91, job 6.1). The rule is construct-walls' `wrap` module (`turned`, pure, exact integers); the type's wrapping is stored with its layers ([construct-walls' FORMAT.md](../crates/construct-walls/FORMAT.md), version 2) and a wall's own choice over it in its record ([construct-model's FORMAT.md](../crates/construct-model/FORMAT.md), "Walls", `wrapping`); free ends are wrapped by the join rule (note [CON-A4-F2-P2-wall-joins](CON-A4-F2-P2-wall-joins.md)) and inserts by construct-regen's kernel builder. The layers and their wraps flag are [note CON-A4-F2-P1-wall-layers](CON-A4-F2-P1-wall-layers.md).

## The question

A layered wall's layers stopped flush at every free end and at every opening's sides. In real construction the finishes are turned round the end of a wall and returned into the reveal of a door or a window, so the frame's end is covered: a lining returns into the reveal, a cladding is turned round a free end. Job 6.1 had to settle which layers turn, where, how far, how that is set by the wall type and by a wall of its own, and how the turned layers meet the others without overlap or gap.

## The rule

Each layer has a flag, wraps or not (job 3.4; only a shell layer may wrap). The wall type says where its layers wrap: at free ends (none, exterior or interior) and at inserts (none, exterior, interior or both). A wall may choose either for itself, over its type's (`wall.wrap`), or read its type's again.

- A layer turns where its flag says it wraps and its side wraps there. A free end is a wall end where no joint is made (no wall meets it, or joins are not allowed there, or the rule leaves the joint unjoined). An insert is a door or a placed family's opening; a plain wall opening is a hole and never wraps.
- A turned layer runs across the end of every layer between it and the core and across the core, to the core's far face. Where both sides wrap at an insert, each side's turned layers run to the core's middle and meet there (the middle rounded to the step nearer the exterior when the core is an odd number of steps thick), as a lining returned from each face meets the frame of a window set in the middle of the wall. Where only one of two wrapping sides has a wrapping layer, it runs to the core's far face as if it were the only one.
- Wrapping layers on one side nest: the outermost turns first, the next turns inside it, and so on. Every layer a turned layer covers is set back from the end or the opening's edge by the thicknesses of the turned layers in front of it. Layers on a side that does not wrap stop flush.
- At an insert the opening is set back in each layer along the wall at both jambs, and at its head and its sill where they are inside the wall (a door to the floor has no sill); each turned layer's part round the opening is built as boxes: its two jambs the full set-back height, then a head and a sill between them. A turned part stops at the wall's base and top.

So the turned layers fill exactly what the set-back layers give up: the pieces of a wrapped end tile the wall's end, and at an insert the layers' volumes add up to the wall less the opening, exactly (construct-regen's kernel suite pins both).

```
 Exterior cladding c (wraps), cavity t, frame S (the core), lining i; a free end on the right,
 wrapped on the exterior:

   c c c c c c c c c c c c c|     the cladding runs to the end, and turns across the cavity
   t t t t t t t t t t t|c c|     and the frame to the frame's inner face;
   S S S S S S S S S S S|c c|     the cavity and the frame stop short by the cladding;
   i i i i i i i i i i i i i|     the lining is flush.
```

## Where the answer lives

- The type's wrapping: its `@system.wall` text, version 2, only when it wraps something; a type that wraps nothing keeps version 1, so every type made before keeps its bytes.
- A wall's own: its recipe argument `wrapping`, only when it has one.
- Derived: the regenerator fills the wall's recipe with `wrap` (its wrapping with each layer's flag and the core) when it wraps anything, so both builders and the page read one answer. A wall that wraps nothing is filled exactly as before.
- Free ends are among the joined pieces (the recipe's `joins`), so every builder that builds joined pieces builds them. Inserts are the kernel builder's: each layer's own cutters are in the body's `grown`, and each turned part is a piece of its layer in the body and the displays. The stand-in builds its one box as before.

## Angles

At a free end of a wall at an angle, each layer's corners are set back by the true distance along the wall rounded once to the grid, the same vector at every face, so the turned parts share their corners exactly with the layers they cover. At inserts, a wall at an angle hosts openings only with piece A83; its turned parts at an angle are the second half of job 6.1, with A83 on main.

## Not decided here

Wrapping round the openings of walls at an angle (with A83); a layer that wraps to the depth of a window's frame rather than to the core's middle; wrapping at a joint that is left unjoined beside another wall, where the turned layer meets that wall's face (today it turns as at any free end).
