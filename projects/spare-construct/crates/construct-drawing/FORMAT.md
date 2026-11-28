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

## What is not stored

Merged regions (`merge::merge`: the regions of one category and material that touch, as one), hatch lines (`hatch::hatch`) and the picture (`picture::picture`: the drawing ready to print at one scale, each line at its pen's printed width) are derived from a stored drawing and the project's styles, materials and fill patterns each time it is printed, and never stored. The fill patterns are `hatch::FillPatterns` in this crate until construct-styles holds them as project data.

## SVG

`svg::Svg` writes a picture as one SVG document: model millimetres, x to the right and y negated (SVG's y runs down), `width` and `height` the paper size at the scale with 5 mm of paper all round. Hatches come first, one `path` each with `data-material`, square ends; then every line, a `line` or arc `path` with `data-element` and `data-role`, round ends. Numbers are exact where the decimal ends and to the nanometre otherwise. Pinned: [`tests/two_walls_plan.svg`](tests/two_walls_plan.svg) and [`tests/two_walls_and_one.svg`](tests/two_walls_and_one.svg).

## Cropped

`crop::clip` cuts a picture to a view's crop outline (the corners of construct-views' `Region`, in the drawing's own coordinates, either way round): every stroke and every hatch line keeps the pieces inside the outline or along it, cut at the exact crossing (a fraction of the grid off the axes), each piece keeping the pen, colour and source of its line; a line wholly outside goes; nothing is drawn along the cut. An arc is kept or dropped whole when its circle is clear of the outline, and refused (`DrawingError::Crop`) when the circle meets it. Nothing about a crop is stored in a drawing: it is applied when the picture is printed.
