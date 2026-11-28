# CON-A2-F7-P1: Warnings as data

**Status, 2026-11-24:** Decided for the build (piece A77, job 5.8, the data half). The crate `construct-warnings` follows it and its [FORMAT.md](../crates/construct-warnings/FORMAT.md) is the read-only form; construct-api's `cloud::warnings` reads the document into it and the server serves `/warnings`. The warnings panel (A2-F7-P2) is the next piece.

## The question

Problems in a model were only refusals of a command: nothing listed the problems a model holds. The feature text asks for them "as a list linked to the elements, never as a dialog that stops work", readable by people, AI and scripts.

## The decisions

- **A warning is a value:** its kind (a stable name), its severity (`notice`, `warning`, `error`), the elements involved (permanent IDs, in a stated order) and a sentence for people.
- **Worked out, not stored.** One function takes the document's current elements and gives the warnings now. The same function after any edit gives the new list, so a warning can never go stale, and nothing needs to be regenerated, undone or redone. The server reads the document as it stands (`At::Now`), the same walls and rooms the room view reads.
- **Identity is the kind plus the elements** (`kind|id|id`), so the same problem keeps the same identity across edits that do not touch it, and an edit that fixes it removes it.
- **Nothing a person decides is stored, because the feature text names none.** It speaks of seeing problems, listing them, and (the next piece) filtering, zooming to them and offering a fix. If accepting a warning is wanted later, it is a command in the log keyed by the warning's identity, with undo and redo like any other, and the list marks it; this piece adds no table and no command.
- **The order:** by kind name, then by the elements' IDs ascending, each identity once; a warning's own elements are in ascending ID order. The same document gives the same list byte for byte.
- **A new kind is one more entry in `RULES`** (name, severity, the function that finds its warnings), with its own tests.

## What counts as an overlap (`wall.overlap`)

Two walls overlap along their length when all of these hold, tested exactly on whole numbers (no tolerance anywhere):

1. They are on the same level (the level their base is tied to).
2. Their reference lines are exactly parallel: the cross product of their direction vectors is zero (either way round).
3. Their outlines in plan, as the builders lay them out (each wall's strip of its layers, the old box wall's box), share a region of positive area. For two convex outlines that is the separating-axis test: no edge of either has the whole of the other on or beyond it.

So they overlap when they run along the same line and share any length, however little, and any width, whatever each one's thickness: a thin wall inside a thick one, a wall lying on top of itself, two walls whose faces cross by one grid step. They do **not** overlap when they only meet at an end (end to end on one line), when they run side by side with their faces touching or a gap between, at a corner or a T or a cross (not parallel, so a join, whose outlines meeting is what a join is), or on different levels. The thickness is the wall's own: for a wall whose layers are symmetric about its reference line this is the same as comparing centre lines and thicknesses (two parallel centre lines closer than half the sum of the thicknesses, with shared length), and for a wall whose layers sit to one side the outline is what is built, so it is what is compared. A wall that does not build, or has no level, is not read, as in every view.

The outline of an oblique wall is rounded once to the kernel's grid (wall_plan's rule); the test is on those grid corners, so what is judged is what is built.

## What counts as not enclosed (`room.not_enclosed`)

What construct-rooms already finds ([CON-A4-F7-P1](CON-A4-F7-P1-rooms.md)): the room's point has no closed loop of the walls of its level round it. A room the search refuses (a point in a wall, a curved wall) and an enclosed room are not warnings of this list; the refusal has its own words on the room's view. A redundant room is, as `room.redundant` (below).

## What counts as a redundant room (`room.redundant`)

Added 28 Nov 2026 (issue 1330). Two rooms placed in one enclosed space are a modelling mistake an architect expects to be told about: the room search already reads the second as redundant (it has no area), but the list said nothing. The warning's elements are the redundant room and then the room that keeps the space (the one whose ID comes first), so both are named; it is `warning` severity like a room that is not enclosed. Its sentence names the keeper as the model lists it (number and name), by its ID where the model does not list it.

## What counts as an opening across a joined wall (`insert.crosses_joined_wall`)

Added 28 Nov 2026 (issue 1332). A door, window or plain opening is cut through its host wall's whole thickness, over the stretch of the host's line it takes. A second wall that meets the host (a partition on a T, or a wall at a corner) and stands across that stretch is a wall the opening would have to cut. Nothing refused this and nothing warned. It is a warning, not a refusal, because the model still builds (the host is cut and the joined wall is not), and a person may be mid-edit.

Decided, exact and on whole numbers (no tolerance): for an insert with the stretch from `low` to `high` along its host's axis, every other wall on the host's level is a conflict when all of these hold: its reference line is not exactly parallel to the host's (so it joins, not overlaps: overlap is `wall.overlap`); its outline in plan meets the host's outline, an edge shared included (the separating axis test, an edge's strict outer side); and its outline stands across a positive length of the stretch (a wall that only touches the stretch's end does not count; one that lies in it by one grid step does). Walls on other levels are never joined. The insert's stretch is the opening the wall's builder places, asked without the engine by the same rules that refuse a door past a wall's end (construct-regen's `inserts`, `kernel::openings`), so what is warned about is what would be built. Heights are not compared: walls stand from their level to the next, so a joined wall on the host's level stands across the opening's height.

The elements, in this order: the insert, its host wall, the joined wall. An insert whose host is not a wall of the model (it does not build) is not judged.

## The page (piece A77, the page half)

The page shows the list and keeps nothing of its own: no rule, no copy of one, no stored warning. The status bar always shows the count ("No warnings", "1 warning", "3 warnings") as a button, and the project browser has a Warnings part with each warning's severity and the server's sentence; an empty list says so in words. The list is read from `/warnings` when a model opens and again, after a short pause so entries arriving together are read once, after every entry of the log that changes the model: another person's edit, the person's own, an undo and a redo (claims and releases change no warning and are skipped). A late answer to an earlier read is dropped. Choosing a warning selects the element it names, as the page's other lists do; the page selects one element at a time, so a warning of two walls offers a button for each. Nothing is accepted or dismissed.

## Not decided here

Filters, zooming to a warning's elements, fixes, accepting a warning, and kinds that need more of the product (a door off its wall, a floor with no walls under it).
