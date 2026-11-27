# Detail items: the stored form, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, on top of [construct-model's cloud format](../construct-model/FORMAT.md). **We may stop writing an old version. We never stop reading it.** The reader is this crate; every reader refuses a record the writer would not have written (it writes the record again and compares).

A detail item belongs to **one view** and is drawn only there. It is an element record with no shape: no level (0), no placement, no references, and a recipe of its own kind with no arguments (`Recipe::construct(kind, {})`), so nothing builds it and no schedule or quantity sees it. Every kind begins `detail.` (`KIND_PREFIX`); the regenerator leaves any such record without a shape by that prefix.

Common to every record: `@category` (the kind's category, below, in its one spelling) and `@detail.view` (the ID of the view). Lengths are construct-params' stored length text (`length:q0:<steps>`, 6,400 steps to the millimetre); points are a key with `.x` and `.y`; an angle is a stored angle (`angle:<units>`, 9,000,000 units to the degree), held modulo a full turn.

| Kind | Category | Name | What it holds (`@detail.` prefix) |
| --- | --- | --- | --- |
| `detail.line` | Detail lines (5800) | Detail line | `line.shape` (`straight` or `arc`), `line.style`, `line.start`, `line.end` (points), and for an arc `line.centre` |
| `detail.region` | Filled regions (6000) | Filled region | `region.pattern` (a fill pattern's name), `region.colour` (`#rrggbb`), `region.boundary` (a style or `none`), `region.corner.<nnnn>` (points, in order, the last joined to the first) |
| `detail.masking` | Masking regions (6100) | Masking region | `region.boundary`, `region.corner.<nnnn>`; no pattern and no colour: it is opaque paper |
| `detail.component` | Detail components (5900) | Detail component | `component.recipe` (a library key), `component.position` (a point), `component.rotation` (an angle) |
| `detail.repeat` | Detail components (5900) | Repeating detail component | `component.recipe`, `repeat.from`, `repeat.to` (points), `repeat.spacing` (a length), `component.rotation` (an extra angle) |
| `detail.slope` | Spot dimensions (5400) | Spot slope | `slope.face` (the ID of a floor or a roof), `slope.point` (a point) |
| `detail.linestyle` | Detail lines (5800) | the style's name | `style.pen` (a model pen's number), `style.pattern` (a line pattern's number), `style.colour` (`#rrggbb`) |

A line style is named by an item as `builtin:fine`, `builtin:standard`, `builtin:bold`, `builtin:centre`, `builtin:hidden` or `builtin:dotted` (our six, in `BuiltIn`), or by the ID of a `detail.linestyle` record. A style is never copied into an item. Colours are lower-case only.

An outline has 3 to 1,024 corners and is an exactly simple loop: no corner twice in a row, no two edges that cross, touch or overlap (neighbours share only their one corner and may not fold back), and an area that is not zero; every corner within the kernel's grid (2^40 - 1 steps, 171.8 km). The direction it runs is kept as given. An arc is the sketch solver's: its end lies on the circle its start makes, within 0.01 mm (the solver's `check_drawn`). A repeat's spacing is above zero and it makes at most 2,000 copies.

**Never stored** (worked out each time it is asked): a region's area, a repeat's count and places, a component's placed lines, a spot slope's value, and the hatch lines of a fill.

## A detail component recipe, version 1

A JSON document ([`schema/detail-component-1.schema.json`](schema/detail-component-1.schema.json)); the library's are in [`library/detail-components`](../../library/detail-components). It is not a family recipe: it has no parameters, 3D, host or type table.

```json
{
  "schema": 1,
  "key": "timber-section-90x45",
  "name": "Timber section 90 x 45",
  "lines": [ { "from": ["-45 mm", "-22.5 mm"], "to": ["45 mm", "-22.5 mm"], "style": "standard" } ],
  "fills": [ { "outline": [["-45 mm", "-22.5 mm"], ["45 mm", "-22.5 mm"], ["45 mm", "22.5 mm"]],
               "pattern": "timber", "colour": "#000000" } ]
}
```

`key` is lower-case letters, digits and hyphens (1 to 64); `name` 1 to 64 characters; at most 64 lines and 64 fills, at least one of either. A length is typed text read by construct-params (`45 mm`, `0.045 m`) and written back as the exact millimetres. A line's `style` is one of the six built-in keys; a fill's outline follows the outline rule above and its `pattern` names a fill pattern (checked where the component is drawn); nothing reaches further than 10 m from the origin. `ComponentRecipe::to_json` is the one written form and reading it gives the same recipe.

Placing a component at a position turned by any angle gives each point as the exact turn of the recipe's point about the position, rounded once to the nearest grid step (construct-modify's rule, main's ruling of 28 Nov 2026). A repeat turns each copy to lie along its line (the component's x along it) and then by its extra angle; a copy's place is the nearest step to its exact place along the line.
