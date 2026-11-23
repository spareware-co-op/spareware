# CON-A8-F2-P1: The drawing types, and the cut of a plan

**Status, 2026-11-23:** Decided for the build (piece A48): note [A12](A12-hidden-lines.md)'s build-order steps 2 and 3. Code: `crates/construct-drawing`, the kernel seam's `Engine::section` (`geom-op`'s engine module) answered by geom-solidcore's engine, and `Kernel::solid` and `Kernel::section` in construct-regen. All of it is interim: the kernel's own `draw` (chapter 18 of the geometry architecture, `geom-draw`, pieces G387, G389 and G394 to G402) replaces it under the same tests.

## The question

Note A12 chose the method and the order of the work. Before any line can be drawn, Spare Construct needs a drawing as plain values that the kernel's engine can produce one day unchanged, and the first drawing: what a plan's plane cuts. Four things were open: how a view maps model points to drawing points exactly; how a line names where it came from when the engine seam carries no persistent names; what the seam's cut takes and gives; and which engine answers it today.

## The decision

- **Views along grid axes only,** as A12 limits the first engine: a plan looks down, sections and elevations look north, south, east or west. A view has three whole unit vectors, the direction looked (`v`), up the sheet and right (`v × up`); a model point `p` in grid steps is the drawing point `(right · p, up · p)` in the same steps, exactly. The cut plane is square to `v` with its normal at the viewer. The table is in `construct-drawing/src/view.rs`.
- **Coordinates are exact:** whole grid steps, or a fraction of them in lowest terms over a positive denominator where a cut crosses a sloped edge. No float is stored; SVG export writes millimetres exactly where the decimal ends and to the nanometre otherwise, the one rounding of an export (chapter 18.3).
- **Each line carries its source:** the element, the face or edge by its persistent name and role, cut or projected, and its category. Pens, colours and patterns are looked up from construct-styles' object styles at print time; the drawing holds no width (chapter 18.2: weights are paper sizes).
- **Faces are named through their planes for now.** `Engine::section` gives each cut edge the plane of the face it lies in (the face's carrier, chapter 20.4), its normal out of the solid and its coefficients with no common factor. The kernel builder gives the same plane for each named face of a recipe (`Kernel::solid`), so a cut edge finds its face's persistent name by plane. Faces on one plane, a wall's face split by an opening, share it, which is what a carrier reference wants. This stays until the seam carries the kernel's names.
- **The seam's cut:** `Engine::section(solid, plane)` gives closed loops, counter-clockwise round material seen from the plane's normal, each starting at its least point, in the order of those points. A plane that misses or only touches the solid gives no loops, as a value; every loop is checked closed.
- **Who answers it:** geom-solidcore's engine, exactly, from the kernel's own parts: `geom_bool::operand::prepare` for the solid's convex pieces, `geom_poly::cut::local_cuts` and `geom_poly::merge::section` for the meeting curves, and its own joining of them into loops. solidcore's `plane_section` is not the answer: it cuts only solids solidcore made (the kernel's prism and booleans keep no solidcore body), its curves are doubles and name no face. It is the oracle in a test. Unsupported for now, as values: curved faces, planes not square to a grid axis, and a corner of the solid on the plane, where chapter 18.4's tie rules wait for G389.
- **The stored form** is canonical JSON, one form only (`Drawing::to_bytes`, `from_bytes`), so the same model and view give the same bytes.

## Why

- Written to chapter 18's shapes (a view, cut curves keyed by element and source, cut regions keyed by element and material), so the kernel's engine fills the same values and the tests do not change (A12, decision 2).
- Grid-axis views are where every tolerance-based code fails (A12, "Plans and elevations on grid axes are the degenerate case"); with whole unit vectors nothing is rounded.
- Naming by plane needs nothing the seam does not have today, is exact, and matches the carrier references hosted elements already use (construct-model's `Want::Carrier`).
- The kernel already had exact section code for flat-faced pieces; using it follows the owner's rule to use the parts of our own kernel that exist.

## Not yet

Edges seen beyond the cut (A12 step 5), hidden lines (step 6), merged regions of one material and fills (step 4), joins and seams (step 7), pens by band and override (step 10). Two faces of one element on one plane but not joined cannot be told apart by a cut edge yet; no recipe today has two.
