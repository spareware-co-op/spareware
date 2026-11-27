# Drawings, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [parameter values](../construct-params/FORMAT.md) whose stored length it uses. **We may stop writing an old version. We never stop reading it.** [`src/drawing.rs`](src/drawing.rs) is the reader library (`Drawing::to_bytes`, `Drawing::from_bytes`); the decisions are note [CON-A8-F2-P1](../../notes/CON-A8-F2-P1-drawing-types.md).

## A stored drawing

One view's drawing as canonical JSON: the same model and view always give the same bytes, and a reader takes only bytes in that one form (a list in another order is refused, never sorted for the writer).

| Field | Value |
| --- | --- |
| `format` | `1` |
| `view` | `{"looking":...,"cut":...}`: `down`, `north`, `south`, `east` or `west`; the cut plane's place along that axis as a stored length (`length:q0:<steps>`) |
| `lines` | Every line: `kind` (`cut` or `projected`), `source` (`element`, `entity` `face` or `edge`, persistent `name`, `role`), `category` (a category id), `shape` (`{"segment":{"start":P,"end":P}}` or `{"arc":{"centre":P,"start":P,"end":P,"turn":...}}`) |
| `regions` | Each element's cut region: `element`, `category`, `material` (a material id or `null`), `loops` (outlines counter-clockwise, holes clockwise, each from its least point) |

A point `P` is `{"x":...,"y":...,"den":...}`: `x / den` and `y / den` grid steps (6,400 a millimetre), in lowest terms over a positive denominator. Lines are in order of kind (cut first), source, category and shape; regions of element, category, material and loops. The pinned example is [`tests/two_walls_plan.json`](tests/two_walls_plan.json).

A section's or an elevation's drawing (`section::section`) is a drawing of this same form, version 1 unchanged: its `view` looks `north`, `south`, `east` or `west`, its `cut` the place of its plane along that axis (a north or south view's y, an east or west view's x); its point `(x, y)` is `right · p` and the height (the table in `src/view.rs`). A section's cut lines and regions are as a plan's; an elevation has none. What is seen beyond the plane are `projected` lines: the outline of each element's shadow between the plane and the view's depth, one line for each straight run, its `source` the face it lies in (`entity` `face`) or the edge where two faces meet (`entity` `edge`, `name` the two faces' names, the lesser first, joined by `+`, `role` their roles in the same order). The view's depth, its width (its line's ends) and its levels and grids are not stored in the drawing; they are applied when it is printed.

## What is not stored

Merged regions (`merge::merge`: the regions of one category and material that touch, as one), hatch lines (`hatch::hatch`) and the picture (`picture::picture`: the drawing ready to print at one scale, each line at its pen's printed width) are derived from a stored drawing and the project's styles, materials and fill patterns each time it is printed, and never stored. The fill patterns are `hatch::FillPatterns` in this crate until construct-styles holds them as project data.

## SVG

`svg::Svg` writes a picture as one SVG document: model millimetres, x to the right and y negated (SVG's y runs down), `width` and `height` the paper size at the scale with 5 mm of paper all round. Hatches come first, one `path` each with `data-material`, square ends; then every line, a `line` or arc `path` with `data-element` and `data-role`, round ends; then, from `Svg::labelled`, each name a section's or an elevation's levels and grids carry, a `text` (`font-family="sans-serif"`, `font-size` its printed height at the scale in model millimetres, `text-anchor` `start` or `middle`, a middle one `dominant-baseline="central"`, `fill` its colour, `data-element` and `data-role` `level` or `grid`), the five characters XML reserves written as entities. Numbers are exact where the decimal ends and to the nanometre otherwise. Pinned: [`tests/two_walls_plan.svg`](tests/two_walls_plan.svg) and [`tests/two_walls_and_one.svg`](tests/two_walls_and_one.svg).

## Cropped

`crop::clip` cuts a picture to a view's crop outline (the corners of construct-views' `Region`, in the drawing's own coordinates, either way round): every stroke and every hatch line keeps the pieces inside the outline or along it, cut at the exact crossing (a fraction of the grid off the axes), each piece keeping the pen, colour and source of its line; a line wholly outside goes; nothing is drawn along the cut. An arc is kept or dropped whole when its circle is clear of the outline, and refused (`DrawingError::Crop`) when the circle meets it. Nothing about a crop is stored in a drawing: it is applied when the picture is printed. A section's or an elevation's crop is held in its own frame (the distance along its line from the first end, and the height) and mapped to the drawing's points by `SectionView::point` before it is applied; the server first keeps the picture to the view's width, between its line's two ends, the same way.
