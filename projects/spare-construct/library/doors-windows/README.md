# Door and window templates

Family templates of Spare Construct's own, as data: each file is one family recipe in [family recipe format 1](../../crates/construct-families/FORMAT.md), checked against its [published schema](../../crates/construct-families/schema/family-recipe-1.schema.json). There is no code in this folder. The choices behind them are in [the templates note](../../notes/CON-A7-F4-P2-door-window-templates.md).

| File | Family | Types |
| --- | --- | --- |
| [`single-swing-door.json`](single-swing-door.json) | A side-hung door: frame (jambs and head), one leaf, hinged left or right | 700, 800, 900, 1000 x 2100; 900, 1000 x 2400 |
| [`fixed-window.json`](fixed-window.json) | A fixed light: frame and glass | 600 x 600 to 1800 x 1200 (8 sizes) |
| [`sliding-window.json`](sliding-window.json) | Two sashes on two tracks, the left fixed, the right sliding | 1200 x 900 to 2400 x 1200 (6 sizes) |

Sizes are millimetres, width by height, of the outside of the frame (the coordinating size the wall opening is set out to).

## What a template is

A family recipe that a project loads as it is, or that the door and window generator fills from a specification. Each one has:

- **Parameters** with units: `width` and `height` (set by each type), the frame's width and depth, the leaf or sash and glass thicknesses, materials, and for windows the sill height (set per placed window); worked-out sizes are formulas (`leaf_width = width - 2 * frame`).
- **Sketches** drawn at the default type and fully held: three fixed points make two datum lines through the origin, and every other point is held by its distance, with a side, from each datum. So every size in the table solves to the same shape, never its mirror.
- **Solids**: the frame, the leaf or sashes, and the glass as its own solid with the material `glass-plate`.
- **Symbols**: in plan, a door's open leaf and its swing arc, and a window's frame and glass lines; in elevation, the opening marks (a side-hung leaf's lines meeting at the hinge side, a sliding sash's arrow) and the outline.
- **Detail levels**: coarse shows the symbols only; medium and fine show the solids, with the swing and opening marks.
- **A type table** of metric sizes in multiples of the 100 mm module.

Coordinates as the format gives them: the origin is the insertion point (the middle of the opening at floor level), x runs along the wall, y across it, z up; the frame is centred on `y = 0`.

## How to add one

1. Start from the template nearest it. Keep the datum lines (`o`, `e`, `t`; lines `base` and `axis`) and hold every new point by two `point_line_distance` constraints with a side, or `point_on_line` for a zero, each length a formula.
2. Draw every point at the default type's values, in stored lengths (`length:q0:<steps>`, 6,400 steps to the millimetre).
3. Name only materials the default library has, or a material parameter for the project to bind.
4. Write the file in the format's one written form (two-space indent, members in the format's order, one newline at the end): reading and writing it must give the same bytes.
5. Add it to the list in `crates/construct-families/tests/library.rs` with the sizes its solids should have, and run that crate's tests.

## Naming

- File: `<stem>.json`, the stem in lowercase words joined by `-` (`sliding-window`).
- Family id: `library.doors-windows.<stem>`; it never changes, so a project's placed families keep their link.
- Parameter ids: `<family id>.<key>`.
- Type keys: one letter for the template (`d` door, `w` fixed window, `s` sliding window) and `<width>x<height>` in millimetres (`d900x2100`); type names `<width> x <height>`.
