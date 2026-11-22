# Family recipes, format 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3. Its JSON Schema is [`schema/family-recipe-1.schema.json`](schema/family-recipe-1.schema.json) (draft 2020-12); [`src/read.rs`](src/read.rs), [`src/check.rs`](src/check.rs) and [`src/write.rs`](src/write.rs) are the reader library; the three files in [`recipes/`](recipes) are examples; the choices behind it are in [the recipe format note](../../notes/CON-A7-F1-P1-recipe-format.md). **We may stop writing an old version. We never stop reading it.**

A family is the template a door, window, piece of furniture or fitting is made from. Its recipe is one JSON document. Read with the [parameter values](../construct-params/FORMAT.md) it stores, the [element types](../construct-types/FORMAT.md) its types become, and the [categories](../construct-categories/FORMAT.md) it names.

## Two layers of rules

- **Shape**: what the schema says. The reader takes exactly what the schema takes, and a test holds the two to it on every example and on thousands of changed copies of a recipe.
- **Meaning**: what the schema cannot see, checked by the reader library against the project's categories and materials (below, "Rules a checker keeps"). Every problem is reported, in document order, at its place as a JSON Pointer (RFC 6901), such as `/sketches/0/constraints/6/a`.

## The document

An object with exactly these members, written in this order:

| Member | Value |
| --- | --- |
| `format` | `1` |
| `id` | The family's permanent id (letters, digits, `.`, `_`, `-`; 1 to 64; starts with a letter or digit). Every version of the family keeps it |
| `name` | The name people see |
| `category` | The family's category: a construct-categories id, such as `300` (doors) |
| `parameters` | Its parameters |
| `planes` | The planes its sketches lie on |
| `sketches` | Its sketches |
| `steps` | Its solid steps, in the order they are built |
| `symbols` | Its 2D symbols |
| `hosting` | What it is placed on |
| `types` | Its built-in types |

Everything a recipe names is named by a **key**: a lowercase letter, then up to 63 lowercase letters, digits and `_`. Keys are unique within each list (parameters, planes, sketches, steps, symbols, types), and within one sketch its points, lines, arcs and circles share one set. A parameter's key is its name in formulas.

Lengths are never JSON numbers. A **stored** value is construct-params' stored text (`length:820`, `material:glass-plate`, `choice:left`), in its one spelling. A **formula** is construct-formulas' language: `width / 2 - frame`, `45 mm`, `frame_depth / 2`; a bare number is in millimetres.

### Coordinates

The family's origin is its insertion point. For a wall-hosted family, x runs along the wall, y across it, z up. A plane is parallel to one of three family planes, which also fix a sketch's two coordinates and the way an extrusion runs:

| `base` | A sketch's first and second coordinates | Normal (extrusions run along it) |
| --- | --- | --- |
| `xy` | x, y | +z |
| `xz` | x, z | +y |
| `yz` | y, z | +x |

### Parameters

| Member | Value |
| --- | --- |
| `key` | Its key |
| `definition` | A construct-params definition, with the same five members its own JSON has, in its order: `id` (the permanent id an element's record stores the value under), `name`, `kind` (`length`, `area`, `volume`, `angle`, `slope`, `number`, `whole`, `text`, `yesno`, `choice`, `element`, `material`, `link`), `holder` (`instance` or `type`), `choices` (`[{"key":..,"label":..}]` for a choice, else `[]`) |
| `default` | Its default, stored, **or** |
| `formula` | the formula it is worked out from (never both). A worked-out parameter is a length, angle, number or whole number, and nothing sets it |

### Planes

`{"key":..,"base":"xy"|"xz"|"yz","offset":<formula>}`: parallel to `base`, `offset` along its normal.

### Sketches

`key`, `plane` (a plane's key), then `points`, `lines`, `arcs`, `circles` and `constraints`: construct-solver's own items, by key.

| Item | Members, in order |
| --- | --- |
| point | `key`, `x`, `y` (stored lengths: where it is drawn, the solver's start) |
| line | `key`, `start`, `end` (points) |
| arc | `key`, `centre`, `start`, `end` (points; anticlockwise from start to end), `radius` (stored length) |
| circle | `key`, `centre`, `radius` |

A constraint is one of the solver's, by the same name, `kind` first:

| `kind` | Members after it |
| --- | --- |
| `coincident` | `a`, `b` (points) |
| `horizontal`, `vertical` | `line` |
| `parallel`, `perpendicular`, `equal_length` | `a`, `b` (lines) |
| `distance` | `a`, `b` (points), `length` (formula) |
| `point_line_distance` | `point`, `line`, `length` (formula), `side` (`left` or `right`, looking from the line's start to its end) |
| `fixed` | `point` |
| `point_on_line` | `point`, `line` |
| `point_on_curve` | `point`, `curve` (arc or circle) |
| `concentric`, `equal_radius` | `a`, `b` (curves) |
| `radius`, `diameter` | `curve`, `length` (formula) |
| `tangent_line` | `line`, `curve`, `side` |
| `tangent_curves` | `a`, `b` (curves), `touch` (`outside`, `first_inside`, `second_inside`) |

### Steps

`key`, `op`, the operation's members, then `subcategory` (the family's category or one of its subcategories), `material` (`{"parameter":<key of a material parameter>}` or `{"fixed":<material id>}`) and `visibility` (`{"coarse":..,"medium":..,"fine":..}`, each `true` or `false`).

| `op` | Members | What it makes |
| --- | --- | --- |
| `extrude` | `sketch`, `profile` (line keys), `holes` (lists of line keys), `start`, `end` (formulas) | The profile less its holes, swept along the sketch plane's normal from `start` to `end` |
| `subtract` | `target`, `tool` (earlier steps) | The target less the tool |
| `unite` | `a`, `b` (earlier steps) | The two as one solid |

These are the operations the kernel's `Engine` interface names, and no others. A profile or hole is one closed loop of three or more lines, each sharing a point with the next and the last with the first. A step a later step uses is that step's input and is not drawn on its own.

### Symbols

`key`, `view` (`plan`: from a sketch on an `xy` plane; `elevation`: on an `xz` plane), `sketch`, `edges` (its lines, arcs and circles to draw), `subcategory` (its line style is construct-styles' object style for it), `visibility`, and, only if it has one, `when`: `{"parameter":<a yes or no, or choice, parameter>,"is":<stored value>}`, drawn only while the parameter has that value.

### Hosting

`{"host":"unhosted"}`, `{"host":"floor"}`, `{"host":"face"}`, or `{"host":"wall","opening":{"sketch":..,"profile":[..]}}`: the opening is a closed loop of lines in a sketch on an `xz` plane, cut through the wall's whole thickness.

### Types

`{"key":..,"name":..,"values":[{"parameter":<key>,"value":<stored>}]}`: a built-in type sets values of parameters the type holds; any other reads its default.

## Rules a checker keeps

A recipe that breaks one is refused, never repaired:

- Names are not only spaces; built-in type names are unique without regard to case or end spaces.
- No two things in one list (or one sketch) share a key; no two parameters share a permanent id.
- Every definition and every stored value is one construct-params takes, in its one spelling; a material value is a material the project has.
- Every formula reads only parameters that exist and are lengths, angles, numbers or whole numbers; its units agree with what it gives (a length everywhere outside the parameters); no parameters are worked out from each other in a loop; worked out at the defaults it does not fail.
- Every point, line, curve, plane, sketch, step and parameter a key names exists and is of the sort wanted; a step's inputs come earlier than it.
- Profiles, holes and openings are closed loops of lines; symbols and openings are on the plane their view needs.
- The category, every subcategory and every material is the project's; every subcategory is the family's category or one of its subcategories.
- A type sets only parameters the type holds that are not worked out.

## The written form

One spelling, so the same recipe is always the same bytes and reading then writing gives the same bytes:

- JSON, indented two spaces, one member or item to a line, `": "` after a member's name, `[]` for an empty list, one newline at the end.
- Members in the order given above; `when` written only when present.
- Integers in plain decimal (a reader takes any integral JSON number, so `1.0` reads as `1`).
- Strings escaped only where JSON must: `\"`, `\\`, `\b`, `\f`, `\n`, `\r`, `\t`, other control characters as `\u00XX` in lowercase; everything else as it is, in UTF-8.

A recipe version is named by its **content id**: `sha256:` and the SHA-256 of these bytes in lowercase hexadecimal.

## A family's types

Each built-in type becomes a construct-types type (version 2, with a system block) in the family's category, listing every parameter of the family and holding the values the type sets. Its block's owner tag is `family`, so its record's `props` hold `@system.family` with compact JSON in exactly this form:

```json
{"version":1,"family":"01KAXF3M1Y0D00R5W1NG000001","recipe":"sha256:<64 hex digits>","type":"d820"}
```

## Versions

A later format is `format` 2 with its own schema file beside this one; format 1 is read for ever. What counts as a breaking change, and what does not, is in [the recipe format note](../../notes/CON-A7-F1-P1-recipe-format.md).
