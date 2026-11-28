# CON-A10-F1-P2: Grouping, totals, formulas, edits back to the model, the sheet and view lists, a schedule on a sheet

**Status, 2026-11-28:** Decided for the build (piece A94, job 8.8; parts CON-A10-F1-P1 rest, P2 and P4). It follows [CON-A10-F1-P1](CON-A10-F1-P1-schedules.md), which it extends and does not change. The crate `construct-schedules` follows it; the stored forms are in its [FORMAT.md](../crates/construct-schedules/FORMAT.md).

## The question

A schedule that only lists rows is not what an architect expects. The first half left grouping, totals, formula columns, editing a value from the table, the sheet and view lists and drawing a schedule on a sheet as owed.

## The decisions

- **Grouping is a property of a sort key.** A key with a header or a footer groups; groups nest in the order of the keys, and a group is the rows that agree on that key and every key before it. This is the one model the feature text names (sorting and grouping with a header and a footer for each group) and it needs no second list. The table is a pure function of the schedule and the elements ([`table`](../crates/construct-schedules/src/table.rs)): header, rows and footer items in order, never stored.
- **Itemize every instance** is one flag on the schedule. Off, the rows of a group that agree in every column are folded into one with a count; the totals still count every element, so a folded row of three doors adds three doors.
- **Totals are exact.** Sum, count, minimum and maximum per column, from the exact values (construct-params' own addition: lengths in grid steps, areas and volumes in square and cubic steps, numbers as decimals), never from shown text and never through a float. A sum of text is refused in words; a total of nothing is empty. Footers and the grand total carry the number of elements too.
- **Calculated fields use the project's formula engine.** The formula language is construct-formulas', so units, exactness and the type check are the one language's, not a second one written here (the crate depends on it; no home-made expression code). A formula names the schedule's other fields by their name written the way a formula can spell it; it is checked with a value of each input's kind when the schedule is made (a wrong unit or an unknown field is refused in the engine's words), and worked out for each row from its values. A row that lacks a value, or divides by zero, has an empty cell: the table is not stopped by one element.
- **An edit in a cell is the element's own command.** `schedule.cell` is turned by the project's commands into `element.param` (or `element.set` for a name, `sheet.set`, `view.rename`) and run as it, in the same step. So it is one undo step, the element is claimed like any edit, and the element's own rules judge the value. What is not the element's to change is refused in words before anything runs: the type's name, a parameter the type holds, a quantity, a calculated field. No second way of writing an element was made.
- **The sheet list and the view list are schedules** of the sheets and views categories, listable like any other, with quantities for the facts a sheet and a view have (number, paper, how many views and schedules stand on a sheet; kind, scale, detail level and the sheet a view stands on). Everything else (grouping, totals, sorting, filters, a placement on a sheet) is therefore the same code. The model has no revisions yet, so there is no revision list; it is the same kind of schedule when revisions exist.
- **A schedule on a sheet is its own element**, a placement record naming the schedule and the sheet and the top left corner on the paper, exact. It is drawn from the live table every time it is asked for (`draw`): text and lines at exact places, by a stated layout rule (row height, text height, characters of a fixed width) that does not depend on any typeface. The sheet listing carries the drawing; a page fits the text to its font. Deleting the schedule or the sheet deletes the placements in the same step, and an undo restores both.
- **Stored forms are additions.** Every new entry is absent when it is at its default, so an existing schedule is the same bytes and every older record still reads.

## Interim and later

- Nothing is interim. The layout rule of a drawn schedule is ours and stated; printing a sheet with its schedule is the print job (8.7), and splitting a long schedule across sheets, conditional formatting and images in cells are CON-A10-F1-P5.
- Key schedules and note blocks (CON-A10-F1-P3) and the revision list (needs CON-A11-F1-P4) are not here.
- A wall's length, area and volume are fields in the fix that adds them to the Walls schedule; this piece's totals work on them as on any quantity.
