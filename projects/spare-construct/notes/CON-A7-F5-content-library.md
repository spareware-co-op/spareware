# CON-A7-F5: the starter content library, and families that stand on a level

CON-A7 (family content beyond doors and windows, [FEATURES.md](../FEATURES.md)): job 14.9 of the build plan, piece A90. The recipes are data in [`library`](../library/README.md) (furniture, casework, plumbing, lighting, planting, generic), written in [family recipe format 1](../crates/construct-families/FORMAT.md); their tests are `crates/construct-families/tests/content_library.rs` and, on the kernel builder, `the_starter_content_builds_and_hangs_without_cutting` in construct-regen's kernel suite.

## The questions

Which families come first; how a family that is not hosted in a wall is placed, turned and raised; how casework and a fixture hang against a wall's face; what a type and an instance hold; how the sizes are chosen without copying a list; and how the page offers them.

## Evidence

- The recipe format already says what a family is placed on (`unhosted`, `floor`, `face`, `wall`); until now only `wall` was placed. The commands, the builders and the page placed a door or a window and nothing else.
- A wall's base is a tie to a level and an offset from it (construct-levels). Moving a level moves everything tied to it, and no element record changes.
- The turn a placed family is built at is the evaluator's `Turn`, whole quarter turns, exact on the grid. The any-angle rule (piece A80, construct-modify: an exact rotation of every point to within a grid step) is not on main yet.
- Sizes are common practice; public sources where they exist are named in the library's README (NZS 4121 for accessible fixtures, EN 527-1 and EN 1116 for desks and kitchen base units, ISO 21723 for the 600 mm ceiling grid). No vendor's family, name, table or default list is used.

## Decision

- **Twelve recipes first**, each with three types, each our own design: desk, chair, bed, sofa; base cabinet, wall cabinet; WC pan, basin, shower tray; ceiling light; tree; generic box. Each is built from boxes (an extruded rectangle each, held by distances from two datum lines as the door and window templates are), the basin and the tray with a recess taken out by `subtract`, the tree's canopy an octagon.
- **One frame for all**: the origin is the middle of the footprint at the family's lowest point (a family that hangs against a wall has its origin on the wall's face, its back on `y = 0` and its front out to `+y`); `width` is along x, `depth` along y, `height` the overall height. The type table's columns are those sizes, so a flex test is the same for every recipe.
- **A type holds its sizes; an instance holds the height it hangs at** (`mount_height` of a wall cabinet and a ceiling light, `rim_height` of a basin), so the same type is placed at any height and a schedule of types counts them.
- **Detail levels**: solids at medium and fine; coarse shows the plan symbols only. Every recipe has a `plan_outline` symbol (the footprint, at every level) and detail symbols (legs, the back, the bowl) at medium and fine. The plan draws a family from its built shape's outline and parts, as it does today; the symbols are the family's own lines for a view that wants them.
- **A family that stands is hosted on nothing.** Its record has no face reference and no host offsets: `placement` holds its point in whole millimetres, `@level` and `@level.offset` its level and its offset above it (the wall's own tie), and `@place.turn` a stored angle. Nothing derived is stored; the builders are handed the turn as `turn`, as a hosted family's is.
- **The turn sits behind a type of ours**, `construct_regen::families::Heading`, which holds any construct-params angle. Interim, until piece A80 is on main: only whole quarter turns are built, and any other angle is refused in words by `family.place` and `family.turn`, and is the build failure `BadTurn` for a record that holds one. When the any-angle rule lands, `Heading` hands it the angle and the refusal goes; no record is rewritten. A family hung on a wall at an angle waits the same way (the wall's own refusal, `NotAHost`, as a door's).
- **A family that hangs on a face cuts nothing.** It is seated on the side's face (the wall's two sides a wall's thickness apart), turned so its front faces out, and only has to lie along the wall (`"family C-1: it crosses the wall's end"`); the wall is built whole, with the same bytes as without it. Two such families, or one over a window, are not checked against each other: overlap of furniture is a clash check's (`construct-clash`), not a refusal.
- **Commands**: the existing family commands, extended and one added. `family.place` takes either a wall (`host`, `side`, `along`) or a point (`x`, `y`, `turn`, `level`, `up`), by what the family is placed on, and refuses the other in words; `family.move` takes the point, the level and the offset of one that stands; `family.turn` is new; `family.retype`, `family.set` and `family.delete` are as they were. Every one is one ordered log entry, one undo step, with the claim, as the door's are; the generic commands refuse a placed family and name the command to use.
- **The page**: a Component tool beside the family tool, with the category, the family and the type picked in the properties palette (the family tool now lists only what cuts a wall), the family's instance values, and for a family that stands its turn and its offset; a click in the plan places one that stands, a click on a wall one that hangs. A selected one shows its point, turn and offset, or its place along its wall and side; dragging one that stands moves it with `family.move`.

## Owed

- Any angle, with piece A80: `Heading` is the one place that changes.
- Families that stand on a floor's surface (`hosting` `floor`), and a recipe's hosting on any face (a ceiling, a column).
- Plan symbols drawn by the plan from the family's own symbols, with the cut and the projected outline drawn in their own line styles (today the plan draws the built shape's outline).
- A schedule of the content by category and type (the schedules crate), and a clash check of furniture against walls.
- Families stored in the document (job 7.6): the library is baked in at build time.
