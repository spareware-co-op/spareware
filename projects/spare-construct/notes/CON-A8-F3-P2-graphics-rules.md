# CON-A8-F3-P2: Graphics rules say how a category looks in one view

**Status, 2026-11-24:** Decided for the build (piece A71). The data and the commands are in `construct-views` and construct-api's `cloud::views`; the stored form is in the crate's [FORMAT.md](../crates/construct-views/FORMAT.md). The plan drawing applying the rules, the page and the two-browser test are later jobs.

## The question

A drawing shows walls heavier than furniture, grey for what is not the subject, a door's swing arc dashed, a floor's cut hatched. The project's object styles (A8-F3-P1) say how a category looks everywhere; a view must be able to say otherwise for itself without changing the project: a ceiling plan with the floor halftone, a section with the cut walls in a heavy line, an elevation that hides the furniture.

## The decisions

- **A rule is for a category or a subcategory and one edge.** The edge is what the view cuts (`cut`) or what it sees beyond the cut plane (`projection`), the two edges of construct-styles' `Edge`. A subcategory is a category in the table with a parent, so one key (`CategoryId`) serves both, and the rule for walls does not need a second list for "core cut line".
- **A rule is a whole style, never a part of one.** Pen (the line weight, by number so that the view's scale prints it thick or thin), line pattern, line colour, fill (cut only), halftone, transparency (a whole percentage, 0 to 100) and shown. The feature text names the list in a sentence, not as fields; this is what we chose. A rule that names only some parts is a command's convenience: the command starts from how the element looks in that view now and writes the whole result. The reason is the resolution rule below: if a rule were partial, two levels could each give a part and an answer would have two origins; with whole styles every answer has one.
- **The fill is a name and a colour, or none.** `None` is "no rule about it": the element's material fills the cut, as the drawing does today. A projection has no fill. The names are the drawing's nine default fill patterns; the crate holds the names as a list and the server's tests pin the list to the drawing's, because the views crate sits below the drawing and cannot ask it.
- **One resolution rule** (`construct_views::resolve`): subcategory's rule in the view, else the category's rule in the view, else the project's object style (the subcategory's, which is its parent's where it has none). Each edge is resolved by itself, so a subcategory with only a projection rule takes its category's cut rule. The project level has no fill, no halftone, no transparency and is shown, because object styles have none of those. Every answer carries its origin (`subcategory`, `category`, `project`) and the pen's printed width at the view's scale. A hidden subcategory under a shown category stays hidden, and the other way round: the nearest rule wins, not the most restrictive.
- **Which views hold rules.** Plans, sections and elevations hold both edges. A three-dimensional view cuts nothing and holds projection rules only. A drafting view and a legend show no model and hold none. The kind is checked when a command writes and again when a record is read, so a record cannot say what its kind cannot hold.
- **Stored on the view record** under one key, `@view.graphics`, as JSON text in the one spelling, written only when there is a rule. A view without the key has no rules, so every old view reads. The reader refuses anything the writer would not give, as every other key of the record.
- **Which categories.** Any but the views category, which no view draws. Annotation categories take the annotation pens, as the object styles do; a cut rule only for a category that can be cut.
- **Commands.** `view.graphics.set` and `view.graphics.clear`, each one record written, one undo step, the claim taken, refusals in words. Setting or clearing what is already so writes nothing and is no step. The other view commands refuse a category, edge or style; `view.duplicate` copies the rules. The generic element commands already refuse a view, so they are no way round the checks.
- **Read-only in the server:** `GET .../views/<view>/graphics`, the rules a view holds with the category's name and parent. It does not list categories the view says nothing about, and it does not resolve: resolving is the drawing's call.

## Interim

The project stores no pens, line patterns or object styles yet, so the commands and the resolution run against construct-styles' defaults (`LineStyles::default_metric`, `ObjectStyles::default_metric`) in one place, `read_world`. The job that stores them replaces those two lines. Fill patterns are the drawing's default set (above). Nothing interim is an engine, so the interim register has no row.

## Why

- Whole styles keep "which level gave it" a single answer, which the drawing and the page need to show a person where a look comes from.
- Resolving per edge matches how a cut and a projection are different things to draw (a cut has a hatch and a heavier pen; what is beyond does not).
- Keeping rules on the view's own record, not in a table of their own, means a view that is duplicated, deleted, undone or replayed takes its rules with it with no new code.

## Not decided here

- The plan drawing applying the rules, a section's, an elevation's and a three-dimensional view's drawing, and the page showing and editing them.
- View templates (8.5), element overrides (8.5), view filters (A8-F3-P3), phase graphics (9.5) and design option visibility: each adds to this and reads through the same `resolve`.
- Storing a project's pens, line patterns, object styles and fill patterns, so that a rule's pen and pattern are checked against the project's own.
