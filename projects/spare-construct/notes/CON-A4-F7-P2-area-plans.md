# CON-A4-F7-P2: Area plans

**Status, 2026-11-28:** Decided for the build (piece A92, job 14.6). construct-rooms' `area` and `line` modules and construct-regen's `rooms` and `flat` modules do it; the stored form is in the [rooms FORMAT.md](../crates/construct-rooms/FORMAT.md) ("Areas") and the [views FORMAT.md](../crates/construct-views/FORMAT.md); the commands are construct-api's `cloud/areas.rs`.

## The question

An area is not a room: it is a region the model's author draws or picks to measure (the gross area of a building, the rentable area of a tenancy), by a rule that is the author's and the country's, not the walls'. Job 14.6 had to settle what an area scheme is, what bounds an area, and how a plan of areas is stored.

## The decisions

- **An area scheme is a record.** Gross, rentable or another measure, named, unique by name (case aside). It has no shape. Areas and boundary lines name it under `@area.scheme`; deleting a scheme in use is refused, saying how many.
- **Area boundaries are their own records.** An area boundary line (`area_line`) is a segment on a level in a scheme, never a room separation line: a room reads walls and room lines, an area reads the boundary lines of its scheme and level and nothing else. So the gross area can run to the outside of the exterior walls and the rentable area to a tenancy's finish faces over the same walls, by drawing different lines.
- **An area is found by the room search.** The same exact search over lines alone: a point inside a closed loop of boundary lines, enclosed, not enclosed or redundant by ID. Its loop is filled before it is built, its area and perimeter are worked out, never stored, and it has no height. A changed line or area builds again the areas of its level.
- **Picking a wall takes its face.** `arealine.pick` makes a boundary line along the face of the wall's outline in plan (as the walls build now, joints and all) nearest the click, exactly on the grid. It is its own record and does not follow the wall afterwards: an area is measured by a rule, and the rule is drawn. Where two picked walls' faces do not meet at a corner (one runs through the other) the loop has a gap and the area says it is not enclosed; a line drawn across closes it. Trimming and extending at corners, as a picked set of walls would want, is a later job.
- **An area plan is a view.** `ViewKind::AreaPlan`: a plan's level and range and the scheme it shows, so it is in the project browser, on sheets and in the views listing like any plan. The page draws in it the walls, the areas and boundary lines of its scheme and no rooms; a floor plan draws no area. The server draws floor plans only, so an area plan is drawn by the page from the elements.
- **The Areas schedule** lists each area with number, scheme, state, area and perimeter, exact; lines are no rows.
- **Warnings.** An area whose boundary lines do not close is a warning, `area.not_enclosed`, as a room's is.

## Not decided here

- National measurement standards (A4-F7-P3) and colour fills (A4-F7-P4).
- Rentable rules that read the walls' layers (to the finish face, the core): a scheme's measure is a word today; the rule is the lines drawn.
