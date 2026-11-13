# A22: View range: Revit's model or a simpler one

**Recommendation in one line:** keep Revit's four planes (top, cut plane, bottom, view depth) as what a plan view stores, because they map one to one onto the kernel's drawing bands and cost nothing to keep, and make it simple in the interface: one cut height most people change, the rest from the view's template. This answers the map's open question in favour of matching, with a simpler face.

## The question

A plan is a horizontal cut plus a view depth. Which planes bound what a plan draws, and how, decides what Spare Construct passes to the kernel's drawing engine and what every plan view stores. The map asks "Match Revit's view range model exactly (top, cut, bottom, depth), or simplify it?" ([ARCHITECTURE.md](../ARCHITECTURE.md), Views and drawings, Discuss). A12's note (#794, in review) closes the hidden-line question against kernel chapter 18 and leaves this one to Spare Construct: "Which planes bound the bands for a plan, ceiling plan or section ... is the map's open question". The board asks for a recommendation covering plan regions, the planes passed to the kernel (chapter 18.2) and what each plan view stores.

Sources were read on 13 November 2026 in this repository: kernel chapter [18](../../engines/geometry/architecture/18-drawings.md), A12, the map, the features list, and the app and server as built. How Revit's view range behaves is described as its users know it and was not checked against Autodesk's documentation; it is marked *unverified* where it matters. Only the interface structure may be followed, never Autodesk's text or icons (AGENTS.md, 5).

## The options

| | A. Revit's four planes | B. Two numbers: cut height and depth | C. Revit's planes, simpler face |
| --- | --- | --- | --- |
| Stored per plan view | Top, cut, bottom and view depth, each a level and an offset | Cut height and depth below; top is always the level above | As A |
| Shown to a person | All four, every time | Two | The cut height; the rest in an advanced panel and the template |
| Overhead elements (wall cabinets, beams, skylights) | Drawn between the cut and the top | Drawn up to the level above, always | As A |
| Office standards from Revit carry across | Yes (*unverified* per office) | Partly: anything set by a top below the next level is lost | Yes |
| Kernel cost | None (below) | None | None |

## The evidence

**The kernel takes exactly these planes.** A drawing call's `ViewDef` holds `cut: Option<CutDef>` and `bands: Bands`, "the GridPlanes bounding Overhead, Primary and Beyond" (18.2). **Overhead** runs from the top clip up to the cut plane, not including it; **Primary** from the cut plane to the bottom plane, both included; **Beyond** past the bottom plane to the far clip. Overhead geometry "has visibility among itself and hides nothing below the cut", and the chapter says outright that "Spare Construct's view range and a mechanical section's depth map onto the bands". Revit's top is the top clip, its cut plane the cut, its bottom the bottom plane and its view depth the far clip (*unverified* as a statement of Revit's definitions; the correspondence of four planes to four planes is exact). Option B is a special case of the same call. So the kernel does not choose; the program and its users do.

**Plans are integer planes.** Band planes are `GridPlane`s and a plan looks down a grid axis, so every visibility decision in a plan is exact (18.2). A view range that stores each plane as a level and a whole-millimetre offset, as placements already do ([`element.rs`](../crates/construct-model/src/element.rs), `Placement`), gives grid planes with no rounding.

**Roles and representations are separate from the range.** `NeverCut` items (furniture) are drawn whole in projection with their parts above the cut counted as Primary, and `NoOcclude` items hide nothing (18.2, roles). Coarse, fine and single-line forms are "the program's choice", sent as different geometry (18.2). So the view range only has to say where the planes are; what is cut, what is drawn whole and at which detail are family visibility rules (A07) and view settings (CON-A8-F3), not more planes.

**Plan regions are crops with their own bands.** A `ViewDef` has one set of bands and an optional `crop` in the picture plane (18.2). A plan region (a split level, a high window; [CON-A8-F1-P5](../FEATURES.md)) is a second view request over the same items, cropped to the region, with its own bands, drawn in the same call (a sheet set is one call, 18.9) and composed by Spare Construct: the plan minus the region, plus the region. The seam between them is a crop line in both drawings, so it is exact; whether the seam itself draws a line is a graphics rule.

**Ceiling plans and underlays are the same data.** A reflected ceiling plan looks up, with its sheet placement mirrored exactly (18.2, the table of drawings): the same four planes, read upwards. An underlay ([CON-A8-F8-P1](../FEATURES.md)) of the level below is another view request with that level's range, drawn halftone. Neither needs another model.

**The app and server load by level today, and a range crosses levels.** The browser asks the server for the pieces of one level in an area (`connect.js`, `/area?level=...`; the area index is keyed by document, level and 5 m cell in [`keys.rs`](../crates/construct-model/src/keys.rs)). A plan whose top is the level above, or whose view depth reaches the level below, needs elements of other levels whose boxes cross those planes. With option A or C, the area query has to take a height range, or the index a level span, before plans draw overhead and beyond elements correctly. Option B narrows the overhead side to one level up but has the same need below.

## Recommendation

1. **Store Revit's four planes in every plan view** (option C's data): top, cut plane, bottom and view depth, each as a level reference and a whole-millimetre offset, so a range follows its levels when a level height changes. Ceiling plans store the same, read upwards.
2. **Make it simple on screen:** the view's properties show the cut height; the four planes are in an advanced panel and in view templates ([CON-A8-F3-P4](../FEATURES.md)), with starter templates per country ([CON-A1-F3-P2](../FEATURES.md)) setting the usual values. Interface text in our own words.
3. **Pass the planes straight to the kernel:** cut plane to `CutDef::Plane`, and top, bottom and view depth to the three band planes. Nothing in Spare Construct re-implements banding, cutting or visibility.
4. **Plan regions as cropped view requests** with their own four planes, composed by Spare Construct, in the same drawing call as the plan.
5. **Load by height, not only by level:** when plans are built (CON-A8-F1-P2), the area query takes the view range's lowest and highest planes and returns every element whose box crosses them, from any level. This changes the server's area query and may change the area index, so it belongs in that piece's design.
6. **Check against Revit only through exported data:** Gate C's plan comparisons (A12) use the view range Revit reports for each compared view, so the two programs draw from the same planes.

## What waits on the kernel

Drawing itself: view input and exact view predicates through hidden lines and line classes (G386 to G402, A12), with G383 (the owner's patent search) first. Recommendations 1, 2 and 5 are Spare Construct's data and server work and wait on nothing in the kernel; 3 and 4 wait on `draw`.

## Open risks

- **Revit's exact rules** for elements that cross the cut, the bottom or the view depth (which are drawn cut, in projection or as beyond), and its defaults per view type, are *unverified* here. The kernel's band intervals are exact and stated (18.2); where an office's habits differ, the difference is in roles and graphics rules, which a truth test against exported Revit plans would show.
- **Ranges that cross many levels** (an atrium plan with a deep view depth) pull many elements into one plan; the area query of recommendation 5 needs a bound and a benchmark.
- **Plan-region seams** in composed drawings are exact lines, but their look (drawn or hidden) is a graphics question left to CON-A8-F3.

## Sources

| Source | Licence | Used for |
| --- | --- | --- |
| Kernel map, chapter [18](../../engines/geometry/architecture/18-drawings.md) (18.2, 18.9) | This repository | `ViewDef`, bands, roles, representations, crops, ceiling plans, one call per sheet set |
| A12 (#794, in review) | This repository | What stays Spare Construct's: the view range |
| [ARCHITECTURE.md](../ARCHITECTURE.md), [FEATURES.md](../FEATURES.md) | This repository | The question; view range, plan regions, templates, underlays |
| [`construct-model/src/keys.rs`](../crates/construct-model/src/keys.rs), [`element.rs`](../crates/construct-model/src/element.rs), [`app/connect.js`](../app/connect.js) | This repository | Loading by level and cell today; whole-millimetre placements |
