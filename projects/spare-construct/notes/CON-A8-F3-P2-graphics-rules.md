# CON-A8-F3-P2: Graphics rules say how a category looks in one view

**Status, 2026-11-24:** Decided for the build (piece A71). The data and the commands are in `construct-views` and construct-api's `cloud::views`; the stored form is in the crate's [FORMAT.md](../crates/construct-views/FORMAT.md). The plan drawing applies the rules and the page shows and edits them (second half of A71, below).

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
- **Read-only in the server:** `GET .../views/<view>/graphics`: the rules a view holds with the category's name and parent (`rules`), and, for the page's editor, `looks` (every category the view can say anything about, each edge as `resolve` answers it, with its origin), the pens, line patterns and fill patterns a rule may name. The page resolves nothing: it shows what the server answers.

## How the drawing and the page use it (second half of A71)

- construct-drawing stays below construct-views. It holds a trait, `look::Looks`, that answers for a category and an edge the whole look (shown, pen width, colour, pattern, fill, halftone, transparency), and `Styles` carries an optional `&dyn Looks`. The server's `ViewLooks` answers it by calling `construct_views::resolve` (through construct-api's `look_in`), so the one rule stays in one place. With no `Looks` the drawing is as before; with a view that has no rules `resolve` gives the project's object styles, and a test pins that both write the same bytes.
- Hidden is left out before the regions are merged, so a hidden category's cut hatches go with its lines. A cut fill rule replaces the pattern and colour of a region that has a material; a region with no material draws no fill, as before.
- Halftone is half strength ([`look::HALFTONE_PERCENT`]) and transparency takes its percentage off; the product is one `opacity` attribute in whole percent, written only below full strength. Line patterns are still drawn solid (the SVG writer's old limit, not this piece's).
- The page's Graphics dialog is reached from the open view's line in the project browser. A control sends only the part it holds (`view.graphics.set` starts from how the element looks now, per the rule above) and Clear is `view.graphics.clear`; refusals are the server's words beside the controls, undo and redo are the log's, and the plan the server draws follows in every browser because every `view.graphics.*` entry reloads the view's plan.

## Interim

The project stores no pens, line patterns or object styles yet, so the commands and the resolution run against construct-styles' defaults (`LineStyles::default_metric`, `ObjectStyles::default_metric`) in one place, `read_world`. The job that stores them replaces those two lines. Fill patterns are the drawing's default set (above). Nothing interim is an engine, so the interim register has no row.

## Why

- Whole styles keep "which level gave it" a single answer, which the drawing and the page need to show a person where a look comes from.
- Resolving per edge matches how a cut and a projection are different things to draw (a cut has a hatch and a heavier pen; what is beyond does not).
- Keeping rules on the view's own record, not in a table of their own, means a view that is duplicated, deleted, undone or replayed takes its rules with it with no new code.

## Not decided here

- A section's, an elevation's and a three-dimensional view's drawing applying the rules (only the floor plan is drawn today).
- View templates (8.5), element overrides (8.5), view filters (A8-F3-P3), phase graphics (9.5) and design option visibility: each adds to this and reads through the same `resolve`.
- Storing a project's pens, line patterns, object styles and fill patterns, so that a rule's pen and pattern are checked against the project's own.
