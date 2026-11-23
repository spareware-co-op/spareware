# Family recipes, format 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3. Its JSON Schema is [`schema/family-recipe-1.schema.json`](schema/family-recipe-1.schema.json) (draft 2020-12); [`src/read.rs`](src/read.rs), [`src/check.rs`](src/check.rs) and [`src/write.rs`](src/write.rs) are the reader library; the three files in [`recipes/`](recipes) are examples; the choices behind it are in [the recipe format note](../../notes/CON-A7-F1-P1-recipe-format.md). **We may stop writing an old version. We never stop reading it.**

A family is the template a door, window, piece of furniture or fitting is made from. Its recipe is one JSON document. Read with the [parameter values](../construct-params/FORMAT.md) it stores, the [element types](../construct-types/FORMAT.md) its types become, and the [categories](../construct-categories/FORMAT.md) it names.

## Two layers of rules

- **Shape**: what the schema says. The reader takes exactly what the schema takes, and a test holds the two to it on every example and on thousands of changed copies of a recipe.
- **Meaning**: what the schema cannot see, checked by the reader library against the project's categories and materials (below, "Rules a checker keeps"). Every problem is reported, in document order, at its place as a JSON Pointer (RFC 6901), such as `/sketches/0/constraints/6/a`.

## The document

An object with these members, written in this order (all but `type_table` always there):

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
| `type_table` | Only if it has one: its type table, more types as rows of values |

Everything a recipe names is named by a **key**: a lowercase letter, then up to 63 lowercase letters, digits and `_`. Keys are unique within each list (parameters, planes, sketches, steps, symbols, types; the built-in types and the type table's rows are one list), and within one sketch its points, lines, arcs and circles share one set. A parameter's key is its name in formulas.

Lengths are never JSON numbers. A **stored** value is construct-params' stored text (`length:q0:5248000`, `material:glass-plate`, `choice:left`), in its one spelling. A **formula** is construct-formulas' language: `width / 2 - frame`, `45 mm`, `frame_depth / 2`; a bare number is in millimetres.

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

### Type table

`{"columns":[<key>..],"rows":[{"key":..,"name":..,"values":[<stored>..]}]}`: many types written the way a catalogue of sizes is, one column per parameter they set and one row per type. A row sets, column by column, one value for each column, in column order; any other parameter reads its default. Each row is a type of the family exactly as a built-in type is: its key and name are unique among the built-in types and the rows together, and it becomes a family type the same way. The recipe's **named types** are its built-in types in order, then its rows in order.

```json
"type_table": {
  "columns": ["width", "height"],
  "rows": [
    {"key": "d620", "name": "620 x 2040", "values": ["length:q0:3968000", "length:q0:13056000"]}
  ]
}
```

The member was added to format 1 by piece A59 as an additive change: it is optional and written only when present, so every recipe that read before reads the same, writes the same bytes and keeps its content id.

## Rules a checker keeps

A recipe that breaks one is refused, never repaired:

- Names are not only spaces; built-in type names are unique without regard to case or end spaces.
- No two things in one list (or one sketch) share a key; no two parameters share a permanent id.
- Every definition and every stored value is one construct-params takes, in its one spelling; a material value is a material the project has.
- Every formula reads only parameters that exist and are lengths, angles, numbers or whole numbers; its units agree with what it gives (a length everywhere outside the parameters); no parameters are worked out from each other in a loop; worked out at the defaults it does not fail.
- Every point, line, curve, plane, sketch, step and parameter a key names exists and is of the sort wanted; a step's inputs come earlier than it.
- Profiles, holes and openings are closed loops of lines; symbols and openings are on the plane their view needs.
- The category, every subcategory and every material is the project's; every subcategory is the family's category or one of its subcategories.
- A type sets only parameters the type holds that are not worked out; a type table's columns are such parameters, each once, and each row has exactly one value per column, each one its column's.

## The written form

One spelling, so the same recipe is always the same bytes and reading then writing gives the same bytes:

- JSON, indented two spaces, one member or item to a line, `": "` after a member's name, `[]` for an empty list, one newline at the end.
- Members in the order given above; `when` and `type_table` written only when present.
- Integers in plain decimal (a reader takes any integral JSON number, so `1.0` reads as `1`).
- Strings escaped only where JSON must: `\"`, `\\`, `\b`, `\f`, `\n`, `\r`, `\t`, other control characters as `\u00XX` in lowercase; everything else as it is, in UTF-8.

A recipe version is named by its **content id**: `sha256:` and the SHA-256 of these bytes in lowercase hexadecimal.

## A family's types

Each named type (a built-in type or a type table row) becomes a construct-types type (version 2, with a system block) in the family's category, listing every parameter of the family and holding the values the type sets. Its block's owner tag is `family`, so its record's `props` hold `@system.family` with compact JSON in exactly this form:

```json
{"version":1,"family":"01KAXF3M1Y0D00R5W1NG000001","recipe":"sha256:<64 hex digits>","type":"d820"}
```

## Evaluation

What a recipe gives when it is built for one type ([`src/evaluate.rs`](src/evaluate.rs): CON-A7-F1-P2, with named types and type tables, P3, and limits, P6; the choices are in [the recipe evaluation note](../../notes/CON-A7-F1-P2-recipe-evaluation.md)). Nothing here changes the format: it is how the format already reads.

1. **Values.** A type's values are stored text by parameter key: a named type's (a built-in type's as written, or a type table row's, column by column), or values a caller gives. Each must be for a parameter that exists, that the type holds, and that has no formula, in its one stored spelling of that parameter's kind and units. Every other parameter reads its default. A named type's refusal names the type and the parameter: a row with no value for a column, a value of another kind or units, a parameter set twice. Every formula is then worked out over the lengths, angles, numbers and whole numbers; a formula that fails, or gives the wrong units, is refused at its parameter.
2. **Sketches.** Only the sketches a step uses are read. Each becomes the sketch solver's own sketch, every constraint length its formula's value at these values, and is **solved from where it is stored**: its stored points and radii are the solver's start, and the solver's least-change steps find the answer nearest what was drawn. A part of the sketch no constraint holds stays where it is drawn (an under-constrained sketch evaluates); the answer is on the grid by the solver's own rounding rule, checked against every constraint within the model tolerance (0.01 mm) after rounding, and the same on every machine and every run. A sketch the solver refuses (its constraints conflict at these values, no answer is found from the stored start, rounding breaks a constraint, or the solver cannot tell) is refused naming the sketch and the first constraint the solver names. The single-swing door's 720, 820 and 920 mm types all evaluate, each to its own frame and panel.
3. **Steps**, in the order written. A step a later step uses is not drawn on its own; every other step is one of the family's solids, with its subcategory, its material (fixed, or the material parameter's value) and its visibility.

| `op` | What it builds |
| --- | --- |
| `extrude` | The profile less its holes, its corners where the sketch's points lie on the sketch's plane, swept along the plane's normal from the lower of `start` and `end` to the higher (written either way round; equal heights are refused). Heights are measured from the plane: a corner lies at the plane's `offset` plus the lower height |
| `subtract` | The target less the tool. An extrusion used as a tool is a **void**: it cuts the one target its `subtract` names, and nothing else |
| `unite` | The two as one solid |

The result for a type is kept by its **cache key**: the recipe's content id, then every parameter without a formula in recipe order with its type value or default in stored text, then any other value given. The same recipe and the same values are built once; a changed value is a new key, so only that type is built again. Named types with the same values (a row repeating a built-in type) share one result, and a recipe's named types are evaluated one by one through the same cache; one type's refusal does not stop the rest.

**Limits.** One evaluation is held to four counts, never to a clock, so a limit is reached at the same place on every machine and every run, and is refused there, never with a partial result:

| Limit | What it counts | Default |
| --- | --- | --- |
| Solver steps | The sketch solver's own work count, all sketches of the evaluation together | 4,000 |
| Solid steps | Steps run, each one engine call | 1,000 |
| Formula work | Bytes of formula text worked out: each parameter's formula and value, and every plane, constraint and extrusion formula | 256 KiB |
| Result size | Profile corners handed to the engine, holes included | 100,000 |

The single-swing door uses under a hundredth of each. What an evaluation used is given with its result.

A refusal names the step being built, the place (type, a type's value, parameter, plane, sketch, sketch item, constraint or step) and the reason.

## Versions

A later format is `format` 2 with its own schema file beside this one; format 1 is read for ever. What counts as a breaking change, and what does not, is in [the recipe format note](../../notes/CON-A7-F1-P1-recipe-format.md).
