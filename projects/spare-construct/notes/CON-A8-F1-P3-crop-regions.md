# CON-A8-F1-P3: A crop region is stored on the view, exact, and served; clipping is the next step

**Status, 2026-11-24:** Decided for the build (piece A79, job 7.7). The data and the commands are in `construct-views` and construct-api's `cloud::views`; the stored form is in the crate's [FORMAT.md](../crates/construct-views/FORMAT.md). The view range (CON-A8-F1-P2) was already stored, validated and feeding the plan's cut height; this piece adds the crop.

## The question

A plan of a whole building does not always go on a sheet whole: a person draws a box, or an L, around the part to show. What does a view hold for that, how is it checked, and what does the drawing do with it?

## The decisions

- **A crop is a region and two switches.** The region is a rectangle (two opposite corners, minimum strictly below maximum on both axes) or a closed outline (the shaped crop). The switches are `on` (it applies, so the view's extent is the region's) and `shown` (its outline is drawn in the view). The region stays when the crop is off or hidden, so switching back loses nothing.
- **The coordinates are the view's own.** For a plan, the project's x and y. For a section or an elevation, the distance along its line from the first end, and the height above the project's zero.
- **A shaped crop is checked exactly.** At least three corners, no corner twice in a row, no two edges that cross, touch or lie along each other (neighbours share only their one corner and may not fold back), and an area that is not zero. The arithmetic is whole-step integer arithmetic on the lengths; a value too large for it is refused as out of range. No float and no tolerance: one grid step of difference is a difference. Which way round an outline runs is kept as given. This is written in the crate rather than taken from a library because the project's rule is exact decisions on stored lengths, which a float or 32-bit geometry library cannot give (the same reason as the drawing's overlay).
- **An annotation crop is a second region of its own**, optional, held and checked the same way. The feature text names annotation crops; what an annotation crop clips (notes, dimensions) comes when annotation belongs to a view, so for now it is stored and served.
- **Which views hold one:** floor plans, ceiling plans, sections and elevations. A three-dimensional view (its box is a section box, a different thing), a drafting view and a legend are refused by name.
- **Commands:** `view.crop.set` (a region when the view has none; later any of region, annotation, on), `view.crop.clear`, `view.crop.show` (shown or hidden). Each is one undo step; one that changes nothing is no step. The other view commands and the generic element commands refuse the crop's parts, so there is no back door. A duplicate keeps the crop.
- **The server lists** each view's range (the four ties and, when they resolve, the heights) and crop.
- **A viewport reads the crop for its size** while it is on: the extent given to the sheet layout is the region's box, exact, in place of the plan's box rounded outward to whole millimetres.
- **The view range needed no new command:** `view.set` with `range` already sets it, validated (top at or above the cut, the cut at or above the bottom, the bottom at or above the depth; a level that does not exist refused), with the default the drawing has always used (1200 mm for the cut). The plan drawing already takes its cut height from the stored range; a test shows the same wall drawn at two cut heights.

## Not decided here

- **Clipping the drawing to the crop.** The drawing crate has an exact clip of a straight line to an area, but only inside its private overlay module, behind no public function, and it works on closed loops with labels, not on a view's lines and hatches. Writing the public clip is the next lane's first step (see the pull request's Handover). Until then the crop changes only a viewport's size.
- What a crop means for which elements the view lists as shown (an element wholly outside the crop is not shown).
- Far clipping of sections and elevations, scope boxes, dependent views (A8-F1-P6, which depends on this) and plan regions (A8-F1-P5).
- The page: dragging a crop, a view range dialog.
