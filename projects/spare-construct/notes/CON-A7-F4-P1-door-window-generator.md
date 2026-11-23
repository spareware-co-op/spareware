# CON-A7-F4-P1: the door and window generator, and its checks (CON-A7-F4-P3)

CON-A7-F4-P1 (door and window generator) and CON-A7-F4-P3 (generator checks), [FEATURES.md](../FEATURES.md): job 6.6 of the build plan, piece A70. The code is [`crates/construct-doorgen`](../crates/construct-doorgen/src/lib.rs); the templates it fills are [`library/doors-windows`](../library/doors-windows/README.md) (note [CON-A7-F4-P2](CON-A7-F4-P2-door-window-templates.md)).

## The questions

What a spec is; how it maps onto a template's parameters and type table; what makes a generated type valid (its solids, its opening cut, its tags) and how that is proved across every size a template allows; which specs are refused and how; and what the generator may not do.

## Evidence

- The templates hold every point by two distances from two datum lines, each a formula of the type parameters, so any size solves to the same shape in one step (note CON-A7-F4-P2). Their opening is the outside of the frame, cut through the wall's whole thickness (note CON-A7-F3-P2).
- Formulas are exact rationals landed on the grid by rounding half to even (construct-formulas): a halved length with an odd step count is rounded by half a step, so a quantity worked from it is no longer exact. The templates halve the width, the frame depth and the glass, and the sliding window also its height and every sash size.
- Formulas work out lengths, angles and numbers only; an area cannot be a formula. A worked-out parameter has no value on a construct-types type (`family_type()` writes only what a type sets), so a schedule reading a type sees stored values only.
- The evaluator builds through any engine behind the kernel's `Engine` trait and hands back the engine's solids; `extents()` gives boxes only.
- ISO 21542 (accessibility of the built environment) measures a door's clear width from the face of the leaf open at a right angle to the stop on the far jamb. ISO 21723 (modular coordination) gives the coordinating size, the outside of the frame.

## Decision

- **A spec** ([`Spec`](../crates/construct-doorgen/src/spec.rs)) is one schedule or catalogue row: a template, a type mark, a size given overall (the coordinating size) or clear, and, where its template has them, the frame width and depth, the leaf thickness, the sash width and thickness and the track gap, the glass thickness, the hand, the sill height, the frame and leaf (or sash) materials, and the thickness of the wall it is for. Anything unset reads the template's default. Every length is a construct-params `Length`.
- **Clear sizes**: a swing door's clear width is its width less two frames and the leaf's thickness (the leaf open at a right angle stands in the opening, as ISO 21542 measures it), its clear height its height less the head; a fixed light's is the glass seen inside the frame; a sliding window's clear width is what the sliding sash leaves open slid fully across (half of the width less two frames and one sash), its clear height the height less two frames. A clear spec becomes the overall size whose clear opening it is, and the measured clear opening must be the one asked for.
- **The mapping**: specs are generated into one family: the template's recipe with the family's own id and name; five tag parameters added after the template's own (type mark, clear width, clear height, leaf area, glass area; the last two only where the template has a leaf or glass), each held by the type with a stored value; and a type table whose columns are the type mark, width, height, every part and type-held material of the template, then the measured tags, one row per spec, every value written. The row's key is the mark in lowercase with spaces and `-` as `_`; its name is the mark and the overall size. The template's own size table is left out: every type of a generated family is one a spec asked for. The tag parameters' ids are `library.doors-windows.<key>`, one for every generated door and window, so a schedule reads one column across them; the template's parameters keep their ids. The hand (a door's `swing`) and sill height are instance parameters: a generated type hands them back as the values a placed unit starts with.
- **The tags are stored, measured values**, not formulas: an area cannot be a formula, and a schedule reads only stored values from a type. Each is measured from what the evaluator built and checked again from the written rows (`verify`), which a later edit to a type must also pass.
- **What the generator may not do**: write geometry. It sets a template's parameters and writes rows and tags; every solid, opening and symbol comes from the template through construct-families' evaluator. The tests hold the generated recipe's planes, sketches, steps, symbols, hosting and category equal to the template's.
- **The rules** (crate `rules`), each with a stable name a refusal is reported by, with the reason and the numbers: only what the template has (`not-in-template`, `hand`: a sliding window's sliding sash is on the right, the other hand is the placed window mirrored); more than zero (`positive`); the template's range (`width`, `height`; ours: a single leaf 600 to 1,200 mm wide and 1,800 to 3,000 mm high, a fixed light 300 to 3,000 mm each way, a sliding window 900 to 3,600 mm by 600 to 2,400 mm); the parts fit the frame's depth and the glass its sash (`fits`); an opening is left (`clear`; a sliding window's sash glass at least 300 by 200 mm, the room its slide mark is drawn in); every halved length an even number of grid steps (`even`), so nothing rounds; the frame no deeper than the wall (`wall`). Then the family: `no-specs`, `other-template`, `mark`, `duplicate-mark`; and what the built type shows (`evaluation`, `opening`, `symbol`, `quantity`, `tag`, `checker`, `round-trip`).
- **The checks** (CON-A7-F4-P3, crate `checks`): the type evaluates within the default limits through a measuring engine of the crate's own behind the `Engine` trait, which keeps each extrusion as it is asked for, so every quantity is exact (areas by the shoelace sum in i128 square steps: the polygon crates work in doubles); every solid is one extrusion along one axis; the opening is one closed rectangle square to the wall, cut through the whole thickness, exactly the frame's outside, with every solid within it along the wall and up and within the frame's depth across it (so it fits any wall at least as thick as its frame); every elevation symbol lies within the opening and every plan symbol within its width; and the tags equal what the solids measure.
- **The flex test** sweeps each template across its range at both ends and the middle (half a millimetre off the whole millimetre) each way, and at the edge of each rule from inside (two grid steps of clear opening, a leaf as deep as its frame, sashes filling the frame's depth, a wall as thick as the frame), and holds each to its quantities worked out by hand; one even step outside each edge (one step for the even rule) is refused by that rule.

## Owed

- Reading a spreadsheet row or a supplier catalogue into specs (column names, typed text in a person's units).
- Fire rating, hardware, manufacturer data and a preview image (ARCHITECTURE.md, "Door and window generator"), the swing direction (the placement's side, not the type's), and the configurations without templates yet (double, bifold, casement, awning, sliding door).
- Commands through the edit log, the page and its test, the browser package, and storing generated family types in a project's document: later jobs.

## What the next jobs need

- A command that generates a family takes the specs and a family id and stores the recipe (job 7.6); a type edited afterwards is checked with `verify` before it is kept.
- The tag parameters' ids are shared by every generated family: a project adds each definition once.

## Open risks

- The ranges are ours, chosen from typical single units; a market that needs wider ones changes one table in `template.rs`, and the flex test follows.
- The measuring engine measures extrusions only; a template that builds a solid by subtraction or union is refused (`evaluation`) until its quantities are measured another way.
