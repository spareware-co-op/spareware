# CON-A4-F4-P1: A floor from a boundary

**Status, 2026-11-23:** Decided for the build (piece A55, job 4.3). The crate `construct-slabs` follows it and its [FORMAT.md](../crates/construct-slabs/FORMAT.md) is the stored form; construct-regen's `slabs` fills a floor's recipe, its `floor_plan` reads the filled recipe for both builders, and its `kernel` module's child `floor` builds it through the engine seam.

## The question

A floor is the second system element after the wall: a type with layers, drawn as a closed boundary in plan on a level, with holes for stairs and shafts. Before commands and pages could make floors, job 4.3 had to settle what a floor type holds, where a floor sits on its level, what its boundary is and how it is checked and stored, what it measures exactly, and how the two builders make its shape.

## How floors are built and drawn

A floor is read from the top down: the finish people walk on (a screed, a topping, a board), sometimes a sheet under it, the part that carries the load (a concrete slab, a frame of joists), and sometimes insulation or a lining below. It is drawn as a closed outline in plan, the outline of the room or the building it covers, with openings cut through it. Its finished top is what a level means for a floor: the storey's height is the height people stand at.

## The decisions

- **Layers**, from the top face down, each with a function, a thickness and a material. The five functions and their join priorities are a wall layer's, word for word, so where a floor meets a wall one rule settles which layer runs through.
- **Own copy of the functions.** construct-slabs keeps its own `Function`, pinned equal to construct-walls' by a test through a dev-dependency only. Sharing it would be a small crate both use, with construct-walls re-exporting it so no caller and no stored byte changes; that is an edit to construct-walls, left to a job allowed to make it.
- **No core yet.** A floor type has no core boundaries; a later version of the type's block may add them (for a structural deck, or a floor whose structure's top sits at the level).
- **The level rule:** a floor's top face sits at its level's elevation plus its own offset, and its layers hang below. Its shape has `z = 0` at its top face and `z = -T` at its bottom; its built row holds the height its top face stands at, as every element's holds its base's. A moved level rebuilds every floor on it through the regenerator's `reads` table, with no second mechanism.
- **The boundary** is the sketch solver's geometry (points, lines and arcs in grid steps) and loops naming its lines in order: one outer loop and any number of holes. Every edge has a permanent ID of its own, which names the floor's side faces, so a boundary drawn again with the same edge IDs keeps its sides' names. The checks (no constraints yet, every edge a line of the sketch once, nothing unused, at least three sides, closed, no zero-length edge, no loop crossing or touching itself, holes wholly inside the outer loop, holes apart and not nested) are exact integer tests, and each refusal names the loop and the edge.
- **Constraints are refused** in a floor's sketch until floors are sketched with the solver (job 5.2), and **arcs are refused** for now: the engine seam's extrusion takes polygons only. Both are held in the data and the stored form already, so allowing them is a check taken away, not a format changed. The interim register has the arcs' row.
- **Placement at the origin.** The boundary's corners are absolute plan coordinates in grid steps; a floor's placement stays at the origin.
- **Measures are exact:** the area by the shoelace sum over grid steps, holes subtracted (an odd twice-area is refused, never halved with a remainder lost); the perimeter edge by edge, each to the nearest step; volume per layer; mass from the material's density where the project's materials hold one. They are read-only parameters (`floor.area`, `floor.perimeter`, `floor.thickness`, `floor.volume`, `floor.mass`), worked from the record and never stored.
- **The recipe** holds the stored boundary and `{"type":"layers"}`; the regenerator fills it from the floor's type before it is built, checking the boundary again, into the form both builders take: the loops (outer anticlockwise, holes clockwise, each corner with its edge's ID) and the layers (function, material, thickness), all in grid steps. A floor's `reads` row lists the parameter table and its type, so a change to the type's layers rebuilds every floor of it.
- **The kernel builds one solid per layer** with a thickness: the boundary with its holes, extruded from the layer's bottom face up to its top, through the engine seam only. Faces are named `top`, `bottom`, `face.<k>` between layers (as a layered wall's), and `side.<edge ID>`.
- **The stand-in builds the box around the outer loop**, so a model with floors builds on the default setting and a page has something in 3D; its outline is the outer loop and its deviation says how far the box strays. The kernel's build replaces it once the browser can run the engine (the stand-in's interim row).
- **Default floor types:** a 200 mm concrete slab, a 150 mm slab under a 50 mm screed, and a timber floor of 240 mm joists under 20 mm sheet flooring, the joists held as one solid-timber layer (so their mass is overstated, and said so). Two of their materials, cement screed and particleboard, are not in construct-materials' defaults: a default material there needs a published density with its source, which this job did not have; a project adds its own.

## Why

- A floor reads its type as a wall does, through one argument the regenerator fills, so a type edit rebuilds every floor and a command and a build cannot read a floor differently.
- The level at the finished top is how floors are set out: the storey height is to the finished floor.
- Edge IDs, not positions, name the sides, so a hosted or referenced side face survives an edit of the boundary that keeps its edges.
- Exact integer measures keep every machine and every replay giving the same area and volume, as the stored-units note requires ([CON-A2-F3-P1](CON-A2-F3-P1-stored-units.md)).

## Not decided here

- Sloped floors, slab edges and floors sketched with constraints (job 5.2); floors made by picking walls; openings by shafts (job 4.2); structural decks; a core in a floor type.
- How a floor's layers join a wall's at their meeting (the join jobs).
- The commands (`floor.create`, `floor.boundary`, `floor.defaults`), the server's views of a floor and the page's floor tool: the next part of this job.
