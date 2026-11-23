# CON-A7-F4-P2: the first door and window templates

CON-A7-F4-P2 (configuration templates, [FEATURES.md](../FEATURES.md)), first part: job 6.5 of the build plan, piece A62. The templates are data in [`library/doors-windows`](../library/doors-windows/README.md), written in [family recipe format 1](../crates/construct-families/FORMAT.md); their tests are `crates/construct-families/tests/library.rs`.

## The questions

Which templates come first; how each is drawn so that every size in its table solves to the right shape; which sizes the tables hold and from where; how doors and windows are shown in plan and elevation and at which detail level; which materials they name; and whether the format and its evaluator can say everything a real door or window needs.

## Evidence

- The build plan names single swing, fixed and sliding first; FEATURES.md lists double, bifold, casement and awning for later.
- Evaluation solves a sketch from where it is stored, by least change (the recipe evaluation note). An unsigned distance (`distance`) has a mirror answer; a `point_line_distance` with a side has none.
- Format 1 can say a parameter's units and formula, a sketch's constraints, extrusions, subtractions and unions, a symbol's view, detail levels and a condition on a choice, and a type table of rows. Formulas are exact: construct-formulas refuses `sin`, `cos` and the other inexact functions until the kernel's certified functions exist. The sketch solver has no angle constraint.
- Evaluation built only the sketches a step uses; nothing drew a symbol at a type's values, so a symbol could not be checked at any size but the one it was drawn at.
- The default material library has, each with a published density and its source: `timber-douglas-fir` and `timber-white-oak` (USDA Forest Products Laboratory, Wood Handbook, FPL-GTR-282), `aluminium-6061` (MIL-HDBK-5J) and `glass-plate` (PNNL-15870 Rev. 1).
- Public standards: ISO 21723 (modular coordination, the basic module M = 100 mm); ISO 2776 (modular coordination: coordinating sizes for door sets); ISO 7519 (construction drawings: general principles of presentation, including how doors and windows and their opening are shown); ISO 128 (technical drawings: lines).

## Decision

- **Three templates first**: a single swing door, a fixed window and a sliding window (two sashes on two tracks, one fixed, one sliding). A sliding door is the sliding window with a floor-level sill and a door's sizes and sections; it waits for job 6.6's generator, which can make it from this template's pattern without a fourth hand-written file.
- **Every point is held by its two coordinates.** Each sketch has three fixed points making two datum lines through the family's origin (`base`, along the sketch's first axis; `axis`, along its second). Every other point is held by a `point_line_distance` with a side from each datum (or lies on one), its length a formula of the parameters. So each point's place is a formula, every sketch is exactly held, no mirror answer exists for the solver to flip to, and every type solves in one step. Points are drawn at the default type's values. This is longer to write than chained constraints; it is the form a generator (job 6.6) can write without judgement.
- **Width and height are the coordinating size**: the outside of the frame, the size a wall opening is set out to (ISO 21723's coordinating dimension). The door's leaf size is worked out from it (`leaf_width = width - 2 * frame`, `leaf_height = height - frame`).
- **The size series** are multiples of the basic module M = 100 mm (ISO 21723). Doors: widths 7M to 10M at a height of 21M, and 9M and 10M at 24M: door set sizes in whole modules, the way ISO 2776 sets coordinating sizes for door sets. Windows: widths and heights in steps of 3M (600 to 2,400 mm), the multimodule we chose for window openings. The tables are ours, chosen from the module; they copy no list.
- **Sections** are typical of each construction, not of any product: a door frame 40 mm wide and 100 mm deep with a 40 mm leaf on the face it opens towards; window frames 50 mm wide, 70 mm deep for a fixed light with 24 mm insulating glass in its middle, 100 mm deep for a sliding window with 50 mm sashes 40 mm thick on two tracks 10 mm apart, meeting over one sash width, with 20 mm glass. Each is a type parameter a project changes.
- **Plan symbols** (ISO 7519's conventions): a door's leaf drawn open at a right angle from its hinge, with the quarter arc its latch edge sweeps, the radius the leaf's width; one symbol per hand, drawn on the `swing` choice. A window shows its frame's two faces and ends, the jambs, and one line for each glass (each sash's glass line on its own track for a sliding window), as plans conventionally draw a window cut through.
- **Elevation symbols** (ISO 7519): a side-hung leaf's two lines from the latch side's corners meeting half way up the hinge side; a sliding sash's arrow along its middle, pointing the way it opens; a fixed light has no mark (no mark means it does not open).
- **Detail levels**: coarse shows the symbols only (the opening's outline and the jambs or frame lines, the swing and the marks); medium and fine show the solids (the frame's profile, the leaf, the sashes, the glass) with the swing and the marks; the outline and frame-line symbols are coarse only, so they never draw over the solids' own lines. Lines are drawn with the subcategory's object style (ISO 128's line types are the styles' concern).
- **Materials**: frame, leaf and sash materials are material parameters whose defaults are in the default library (above, with their sources); glass is its own solid with the fixed material `glass-plate`. A project binds others by setting the parameter.
- **Handing**: the door's hand is the instance choice `swing` (left or right, seen in the front elevation, from the side the leaf opens away from). The sliding window's moving sash is on the right, opening left; the other hand is the placed window mirrored.
- **Names**: file `<stem>.json`, family id `library.doors-windows.<stem>`, parameter ids `<family id>.<key>`, type keys a letter for the template and `<width>x<height>` in millimetres (`d900x2100`), type names `<width> x <height>`.

## Changed in the evaluator (small and additive)

- `construct_families::symbols()` and `symbols_within()`: a recipe's symbols drawn at one type's values, each symbol's sketch solved exactly as a step's sketch is, within the same limits, every edge read back as its solved line, arc or circle (`DrawnSymbols`, `DrawnSymbol`, `Edge`). Without it no symbol could be checked at more than the one size it was drawn at. Nothing that read, validated or evaluated before changes; the schema, reader, writer and checker are untouched.

## Owed

- **The swing angle shown in plan.** A door drawn open at another angle (30 or 45 degrees, as some offices draw it) needs either an angle constraint in the sketch solver or exact trigonometry in formulas; neither exists (formulas refuse `sin` and `cos` until the kernel's certified functions arrive). The template draws the leaf at 90 degrees, the convention that shows the full swing clearance, and has no angle parameter, rather than one that would change nothing.
- **Instance values in evaluation.** Evaluation takes values a type holds only, so the door's `swing` reads its default there; `symbols()` hands every symbol with its condition, and the view decides which to draw.
- The other configurations (double, bifold, casement, awning, sliding door), feet-and-inch tables, and door undercut, threshold, ironmongery and glazing beads.

## What the next jobs need

- Job 6.4 (hosted families): every template declares `{"host":"wall"}` with its opening as the outside of the frame in the elevation sketch (`threshold` or `outer_bottom`, then the outer right, head or top, and left lines). The cut is the coordinating size, through the wall's whole thickness; the frames are centred on the wall's centre plane (`y = 0`), the door leaf flush with the face it opens towards.
- Job 6.6 (generator): a template is filled by setting its type parameters (`width`, `height`, and the section parameters) or by writing type table rows; every point is a formula of those, so any size in the module solves. A new configuration is written the same way: datum lines, then two holds per point, drawn at its default type.

## Open risks

- The sizes are checked against the test engine's bounds and the symbols against their solved points; a real engine's solids are judged by the generic evaluation suite when the wiring job links one.
- The size series is ours, from the module; a project in another market needs its own table, which the format already allows.
