# The family library

Family recipes of Spare Construct's own, as data: each file is one family recipe in [family recipe format 1](../crates/construct-families/FORMAT.md), checked against its [published schema](../crates/construct-families/schema/family-recipe-1.schema.json). There is no code in this folder. The doors and windows have their own [README](doors-windows/README.md); the starter content below is described by the [content library note](../notes/CON-A7-F5-content-library.md).

| Folder | Family | Placed | Types |
| --- | --- | --- | --- |
| `furniture` | [Desk](furniture/desk.json) | stands on a level | 1200 x 600, 1500 x 750, 1800 x 800 |
| | [Chair](furniture/chair.json) | stands on a level | Side, Dining, Lounge |
| | [Bed](furniture/bed.json) | stands on a level | Single, Double, Queen |
| | [Sofa](furniture/sofa.json) | stands on a level | Armchair, Two seater, Three seater |
| `casework` | [Base cabinet](casework/base-cabinet.json) | stands on a level | 450, 600, 900 wide |
| | [Wall cabinet](casework/wall-cabinet.json) | hangs on a wall's face | 450, 600, 900 wide |
| `plumbing` | [WC pan](plumbing/wc-pan.json) | stands on a level | Standard, Accessible, Compact |
| | [Basin](plumbing/basin.json) | hangs on a wall's face | Hand basin, Standard, Wide |
| | [Shower tray](plumbing/shower-tray.json) | stands on a level | 900 x 900, 1000 x 1000, 1200 x 900 |
| `lighting` | [Ceiling light](lighting/ceiling-light.json) | stands on a level, at the height of its instance value | Panel 600 x 600, Panel 1200 x 600, Batten 1200 x 150 |
| `planting` | [Tree](planting/tree.json) | stands on a level | Small, Medium, Large |
| `generic` | [Generic box](generic/generic-box.json) | stands on a level | Small, Medium, Long |

Each is in its category's built-in category (furniture, casework, plumbing fixtures, lighting fixtures, planting, generic models), so graphics rules and schedules pick it up.

## The frame

The origin is the middle of the footprint at the family's lowest point; a family that hangs against a wall has its origin on the wall's face, its back on `y = 0`, its front out to `+y`. `width` runs along x, `depth` along y, `height` is the overall height; the type table's columns are those sizes (and the few others a type changes, such as a chair's seat height). A family that is hung or lit at a height holds it as an instance value (`mount_height` for a wall cabinet and a ceiling light, `rim_height` for a basin), measured from the wall's base or the level.

## Sizes, and where they come from

The sizes are ours, from how this furniture and these fittings are commonly made. Where a public source sets them, it is named; none of the tables or names is copied from anyone's catalogue.

- **Desks**: working height about 730 mm, depths 600 to 800 mm (the working-height range of EN 527-1, office work tables).
- **Base cabinets and wall cabinets**: base units 900 mm high and 600 mm deep with widths in the 150 mm series from 450 mm, wall units 320 mm deep and 720 mm high hung with their underside at 1450 mm; kitchen furniture's coordinating sizes are set by EN 1116, which these follow in kind.
- **WC pans**: a seat about 420 mm high for the standard pan; 460 mm for the accessible one, within the range New Zealand Standard NZS 4121 (design for access and mobility: buildings and associated facilities) sets for accessible pans. Basins hang with their rim at 850 mm; an accessible basin is hung lower (set the instance value from the current edition of NZS 4121).
- **Beds**: frames a little larger than the mattress sizes commonly sold in New Zealand (single 915 x 1880, double 1370 x 1880, queen 1530 x 2030 mm).
- **Ceiling lights**: 600 mm modules, 6M of the 100 mm basic module of ISO 21723 (modular coordination), so a panel fills one tile of a 600 mm ceiling grid.
- **Chairs, sofas, trees, the generic box**: common sizes; a project sets its own by changing the type.

## What a recipe is built from

Every solid is an extruded rectangle (the tree's canopy an octagon) drawn in one plan sketch whose points are each held by two distances from two datum lines through the origin, as the [door and window templates](doors-windows/README.md) are, so every size in a type's row solves to the same shape. The basin and the shower tray take a recess out of a block with `subtract`. Materials are parameters whose defaults are in the default metric material library; a project binds others. Plan symbols: `plan_outline` (the footprint, at every detail level) and the lines inside it (legs, back, bowl) at medium and fine.

## How to add one

1. Start from the nearest recipe. Keep the datum lines (`o`, `e`, `t`; lines `base` and `axis`) and hold every new point by two `point_line_distance` constraints with a side (or `point_on_line` for a zero), each length a formula of the parameters.
2. Draw every point at the default type's values, in stored lengths (`length:q0:<steps>`, 6,400 steps to the millimetre).
3. Keep `width`, `depth` and `height` as the overall box, so the flex test judges it as it judges the others.
4. Write the file in the format's one written form: reading and writing it must give the same bytes.
5. Add it to `pieces()` in `crates/construct-families/tests/content_library.rs` with its category, hosting and the ranges its sizes may take, add it to the baked list in `crates/construct-regen/src/families.rs`, and run those crates' tests.

## Naming

- File: `<stem>.json`, the stem in lowercase words joined by `-`.
- Family id: `library.<folder>.<stem>`; it never changes, so a project's placed families keep their link.
- Parameter ids: `<family id>.<key>`.
- Type keys: lowercase words (`two_seater`), or a letter and the sizes (`d1500x750`); type names as people say them.
