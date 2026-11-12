# A12: Drawing engine: hidden line removal for plans and sections

## The question

Plans and sections cut solids with a plane and draw what is beyond in projection, with hidden edges removed or dashed; the map's method is to "project edges and occluders and resolve visibility in 2D on a spatial grid, in parallel by tile", cached per element and cleared through the regeneration graph ([ARCHITECTURE.md](../ARCHITECTURE.md), Views and drawings). The piece asks how open tools do hidden-line removal, and for a recommended method.

## Finding: the kernel has done this piece

The drawing engine's geometry moved into Spare Kit's geometry kernel, and chapter 18 of its map ([18-drawings.md](../../engines/geometry/architecture/18-drawings.md)) answers both halves:

- **How open tools do it** (18.1). OpenCascade's HLRBRep is the only open hidden-line code on a B-rep the kernel's sweep found; it tests each edge against each face that might hide it, in projection, with boxes rejecting the rest, and is LGPL, so its organisation is studied and nothing more. HL for Rhino (MIT) and ln decide visibility by sampling, which the kernel uses only for its line vocabulary and its list of model faults. Blender's Freestyle is GPL and not used.
- **The recommended method** (18.6). Appel's quantitative invisibility: every point of a projected curve has a count of how many times the line of sight leaves material toward the viewer, visible where the count is zero, and the count changes only at events (crossings behind an outline, piercings, cusps). Candidates, occluders from the spatial index in view space, events ordered exactly for flat geometry and certified for curved, counts re-tested directly at each chain's end, and rules (not tolerances) for coincident lines, junctions and faces seen edge-on, which are the normal case in plans.
- **Depth bands** (18.2). Overhead, Primary and Beyond bands bounded by planes the program supplies: Spare Construct's view range.
- **Caching and incremental update** (18.9). Results keyed per item, placement and view; a changed item's lines recomputed, and visibility recomputed only where its old or new footprint overlaps, with early cut-off when its first stage comes out unchanged, "in Salsa's style"; the incremental result equals a full one bit for bit.
- **The build plan** is the kernel's ([30-build-order.md](../../engines/geometry/architecture/30-build-order.md)): G383 (owner's patent search) and G386 to G405, from view input and exact view predicates through sections of every body kind, exact hidden lines for plane-based bodies (G394), certified hidden lines for curved bodies (G396), line classes, the drawing checker, the incremental engine with parallel views (G402), and drawing storage and export (G405).

So the board's done-when is met in the kernel map, where every program that draws can use it.

## What stays Spare Construct's

The kernel draws geometry; Spare Construct decides what a view is and how it looks:

1. **The view range.** Which planes bound the bands for a plan, ceiling plan or section, and whether to match Revit's view range model (top, cut, bottom, depth) or simplify it, is the map's open question (Views and drawings, Discuss). The kernel takes the planes as input (18.2).
2. **Graphics rules.** Line weights, patterns and styles by category, subcategory, cut or projection, filters, phase and design option, and view templates (CON-A8-F3), applied to the kernel's line classes (18.7).
3. **Representations.** Which geometry each element sends per detail level (coarse, fine, single-line) and its role (`Cut`, `NeverCut` for furniture, `NoOcclude`) are the program's choice (18.2), set by family visibility rules (A07).
4. **The drawing cache in the regeneration graph.** Results cached per element and cleared through regeneration (CON-A8-F6-P3) sit in Spare Construct's incremental engine (A03), over the kernel's pure drawing calls.
5. **Output.** Lines at true weight and fills on screen (CON-A8-F2-P4, the renderer, A04) and in vector PDF (CON-A11-F2-P1).
6. **Truth tests.** Plans and sections compared with Revit's within a tolerance, the test behind Gate C (CON-A2-F9-P5).

## Recommendation

1. **Take kernel chapter 18 as this piece's answer** and close A12 against it: open tools in 18.1, the method in 18.6, the build in G386 to G405.
2. **Add a small decision piece for the view range** (match Revit's model or simplify it) to Spare Construct's board, since it fixes the planes Spare Construct passes to the kernel and the parameters every plan view stores.
3. **Leave `construct-views` to the program's side:** view definitions, graphics rules, templates and the drawing cache over the kernel's calls. Its `lib.rs` should name kernel chapter 18, A03 and A04 when the crate reaches Design.

## Open risks

- **The drawing engine is not built yet.** Floor plans wait on G386 to G402; the "under 500 ms first draw, under 16 ms after an edit" target is for the kernel's benchmarks and A03's engine together, and is unmeasured.
- **Revit truth data.** Gate C compares with Revit's own plans; how those are exported within Autodesk's API terms is the map's open item under Interop, not settled here.

## Sources

| Source | Licence | Used for |
| --- | --- | --- |
| Kernel map, chapters [18](../../engines/geometry/architecture/18-drawings.md) and [30](../../engines/geometry/architecture/30-build-order.md) | This repo | Open tools, method, bands, caching, build order |

No LGPL or GPL source was read for this note.
