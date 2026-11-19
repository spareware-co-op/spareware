# A07: Family recipe format: JSON with a schema, or our own language

## The question

A family is "a recipe written as data: parameters, a constrained sketch and solid steps", edited in a family editor, written directly by AIs, and produced by the door and window generator ([ARCHITECTURE.md](../ARCHITECTURE.md), Families). The map wants a text format "so diffs, reviews and AI edits work in git", and asks: "JSON, or a purpose-built language for recipes?" Its decisions table leans to JSON with a schema. The piece asks for a recommendation with a door family written both ways.

The door is in [`A07-door/`](A07-door/): [`single-swing-door.family.json`](A07-door/single-swing-door.family.json) and the same door in a sketch of a purpose-built language, [`single-swing-door.family`](A07-door/single-swing-door.family). Both say the same thing: 14 parameters (three of them formulas), 8 reference planes, 3 sketches, 2 solids and a void cut in the host, 3 plan symbols, handing by mirror and flip, and 3 types.

Sources were read on 12 November 2026: Zoo's KCL (kcl-lib 0.2.196, in [KittyCAD/modeling-app](https://github.com/KittyCAD/modeling-app/tree/3334e6fddd9163d9b60188a597a6ed18cdeb9075) at `3334e6f`), the purpose-built CAD language with the most open code; the Pascal editor at [`51bf407`](https://github.com/pascalorg/editor/tree/51bf4071c10d0189c2499ec2069e580b74169106), a browser BIM editor whose procedural items are JSON recipes for agents; the Common Expression Language's [language definition](https://github.com/cel-expr/cel-spec/blob/21ec6f4e52e8d694687ff9ed7408d3eccbb955d7/doc/langdef.md); schemars 1.2.3; and the kernel's map (chapters 24 and 25). "Unverified" marks anything not confirmed in source.

## What a recipe has to do

From the map and features:

- **Hold** parameters with formulas, reference planes and constrained sketches, solid steps (extrude, sweep, revolve, blend, void), 2D symbols for plan and elevation, visibility by view and detail level, materials, MEP connectors, types as named parameter sets, nested recipes with linked parameters, and the host it declares and the void it cuts (Families; [CON-A7-F1-P1](../FEATURES.md)).
- **Be written by AIs** from "a published schema with examples so any AI can write valid recipes" (CON-A7-F5-P2), through the MCP server (CON-A7-F5-P1), and by the door and window generator from a spreadsheet row (CON-A7-F4-P1).
- **Be edited in a family editor** that "flexes" the family and loads it back (CON-A7-F2): the editor changes the recipe, so the format must take machine edits without losing what people wrote.
- **Diff and review in git**, from one shared, versioned library (Families, low level; CON-A7-F6-P4).
- **Evaluate safely.** Time, memory and size limits, "so a shared family cannot hang the app or fill its memory" (CON-A7-F1-P6). Formulas use "a small, safe expression language" (Data model).
- **Fit the kernel.** Every recipe sits in the kernel's recipe envelope: owner namespace, the owner's schema version, operation and version, inputs by content ID and name, an opaque payload ([24.4](../../engines/geometry/architecture/24-file-format-exchange.md#244-what-a-file-holds)). The kernel describes every operation's inputs and outputs in JSON Schema 2020-12 for its MCP tools ([25, as built G100](../../engines/geometry/architecture/25-public-interface.md)). Stored records are deterministic CBOR (24.1). `construct-model`'s `Recipe` already holds a family's arguments as a sorted JSON map with whole millimetres, so a hash never depends on how a float prints ([geometry.rs](../crates/construct-model/src/geometry.rs)).
- **Deterministic.** The same recipe and parameters give the same body on every machine (ground rule 6).

## Options

| | A. JSON with a schema | B. Our own language | C. JSON with a text view on top |
| --- | --- | --- | --- |
| What it is | Recipe objects in JSON, checked by a JSON Schema generated from Rust types; formulas as strings in one small expression language | A purpose-built typed language with units, like KCL | A as the stored form, plus a readable text form that converts both ways |
| Parser to write | The expression language only | The whole language, its formatter and its language server | A's, plus the text form's both ways |
| How an AI is told the rules | The JSON Schema itself, as the kernel's MCP tools already are | A grammar and documentation in prose | The JSON Schema |
| How an editor saves a change | Set a field | Rewrite source text and keep the person's layout and comments | Set a field; the text form is regenerated |
| The door | 116 lines, 4,649 bytes | 59 lines, 2,374 bytes, without comments | Both |

## Evidence

### The door, both ways

The JSON door is about twice the size of the language door, and the difference is almost all punctuation and repeated keys: `{ "name": "Width", "kind": "length", "scope": "type", "default": "820 mm" }` against `type Width: length = 820 mm`. Changing one type's width is a one-line diff in both. What a reader sees differs more in the geometry steps: the language's `extrude(FrameOutline, between: Exterior .. Interior)` reads as what it does, where JSON spreads it over keys.

Both need an expression language. The JSON door has 20 expressions with operators in strings, beside its literals with units and plain names, such as `"Width - 2 * FrameWidth - 2 * Clearance"`, a conditional (`"Opens == 'in' ? Interior : Exterior"`) and plane arithmetic (`"Left + FrameWidth"`). Parameter formulas need it anyway: Spare Construct's map already asks for "a small, safe expression language" for every parameter, not only in families. So option A is not "no language"; it is one small language for values, inside a data format for structure, and option B is a large language for both.

### Pascal: JSON recipes for agents, already in a BIM editor

Pascal's procedural items are JSON recipes with parameters, material slots, parts and shapes, validated by a zod schema ([recipe.ts](https://github.com/pascalorg/editor/blob/51bf4071c10d0189c2499ec2069e580b74169106/packages/core/src/procedural-items/recipe.ts); a fixture: [cabinet_two_doors_drawer.json](https://github.com/pascalorg/editor/blob/51bf4071c10d0189c2499ec2069e580b74169106/packages/core/src/procedural-items/__fixtures__/cabinet_two_doors_drawer.json)). Its agent surface is "the schema a design must follow and the validation that is the authority over it", served to agents as an MCP resource ([design.ts L24-L33](https://github.com/pascalorg/editor/blob/51bf4071c10d0189c2499ec2069e580b74169106/packages/core/src/procedural-items/design.ts#L24-L33)). It caps a recipe's bytes, nesting depth, expressions, shapes and triangles ([recipe.ts L121-L137](https://github.com/pascalorg/editor/blob/51bf4071c10d0189c2499ec2069e580b74169106/packages/core/src/procedural-items/recipe.ts#L121-L137)), which is CON-A7-F1-P6 already built. Repeated parts take a `count` and read their `index`, so a row of identical doors or shelves needs no loop.

Its expressions are JSON trees, `{ "op": "mul", "args": [{ "op": "sub", "args": ["index", 0.5] }, 0.86] }` for `(index - 0.5) * 0.86`, with a fixed set of operators ([recipe.ts L25-L53](https://github.com/pascalorg/editor/blob/51bf4071c10d0189c2499ec2069e580b74169106/packages/core/src/procedural-items/recipe.ts#L25-L53)). That is safe and needs no parser, but nobody would review a door's formulas written that way; it is the reason this note puts formulas in strings. Pascal also keeps an escape hatch: a door can be built from "a three.js script" instead of its parametric frame ([door.ts L76](https://github.com/pascalorg/editor/blob/51bf4071c10d0189c2499ec2069e580b74169106/packages/core/src/schema/nodes/door.ts#L76)), which a deterministic, safely evaluated recipe cannot allow.

### KCL: what a purpose-built language costs

KCL is a typed CAD language in which "all numbers in KCL include units" ([units.kcl L1-L8](https://github.com/KittyCAD/modeling-app/blob/3334e6fddd9163d9b60188a597a6ed18cdeb9075/rust/kcl-lib/std/units.kcl#L1-L8)), with sketches, constraints and solid steps in its standard library. It shows both what a language gives and what it costs:

- **Size.** Its parser is 19,173 lines of Rust (on the `winnow` parser crate) and its executor 51,259 lines, with a 13,016-line standard library written in KCL, plus a formatter, an unparser and a language server in their own crates (from the repo at `3334e6f`). That is before any geometry: KCL sends modelling commands over a WebSocket to Zoo's own engine ([engine/mod.rs L20-L21, L180](https://github.com/KittyCAD/modeling-app/blob/3334e6fddd9163d9b60188a597a6ed18cdeb9075/rust/kcl-lib/src/engine/mod.rs#L20-L21)).
- **Editors must rewrite source.** For point-and-click sketching, KCL writes solved values back into the code as `var` literals (`circle(start = [var 1.84mm, var -0.32mm], ...)`, [sketch.kcl L886-L899](https://github.com/KittyCAD/modeling-app/blob/3334e6fddd9163d9b60188a597a6ed18cdeb9075/rust/kcl-lib/std/sketch.kcl#L886-L899)), so the editor is a program that edits programs. Spare Construct's family editor and door generator would have to do the same.
- **Numbers are floats.** A KCL number is an `f64` with a unit type ([kcl_value.rs L107-L112](https://github.com/KittyCAD/modeling-app/blob/3334e6fddd9163d9b60188a597a6ed18cdeb9075/rust/kcl-lib/src/execution/kcl_value.rs#L107-L112)). Spare Construct's values go to the kernel's exact grid (kernel chapter 6), so its expression language needs its own number rules whichever option is chosen.

KCL is MIT-licensed, so its method and code can be studied and reused with credit; its units-in-types idea is worth taking into the expression language below.

### Telling an AI the rules

The kernel already describes each operation to AI agents in JSON Schema 2020-12 (25, as built G100), and schemars 1.2.3 (MIT) generates JSON Schema 2020-12 from Rust types by default ([generate.rs L24-L40](https://github.com/GREsau/schemars/blob/v1.2.3/schemars/src/generate.rs#L24-L40)). With option A, the family schema is generated from the same Rust types that evaluate recipes, and solid steps reuse the kernel's own operation schemas, so documentation, AI tools and the evaluator never drift apart, as the map asks for commands. Many model providers can hold their output to a supplied JSON Schema (*unverified* across providers). With option B, an AI learns the language from prose and examples, and each mistake is found only by our parser; KCL ships a language server and documentation generator to close that gap.

### Formulas: a small, safe expression language

CEL, the Common Expression Language, defines the properties the map asks for: "side-effect-free: a CEL program only computes an output from its inputs" and "terminating: CEL programs cannot loop forever" ([langdef.md L12-L13](https://github.com/cel-expr/cel-spec/blob/21ec6f4e52e8d694687ff9ed7408d3eccbb955d7/doc/langdef.md#L12-L13)), with a written grammar and a test suite (Apache-2.0); cel-rust (MIT) implements it. CEL has no units and uses doubles, so it is the method to follow, not the evaluator to take as is. What the door's formulas need is small: arithmetic, comparisons, `?:`, `min`, `max`, rounding, a few functions, parameter and plane names, `host.` and `type.` reads, and literals with units.

### Determinism and canonical bytes

A JSON recipe's bytes are canonical once keys are sorted and numbers fixed, as `construct-model` already does for recipe arguments and as RFC 8785 (JSON Canonicalization Scheme) defines in general. A record stores it as deterministic CBOR (24.1), which maps one to one onto JSON's data model for these values. A language's source has layout and comments that are not the recipe, so a content hash must be over its parsed form, and the canonical form is then a second format anyway (*derived*).

## Recommendation

1. **JSON with a schema (option A), as the map leans, with formulas as strings in one small typed expression language.** Structure is data: what the family editor, the door generator, AI agents, the MCP server and the kernel all exchange. Values are expressions: what people write and review. A full language would buy shorter files at the cost of a parser, formatter, language server and source-rewriting editor several times the size of the evaluator itself (KCL's parser and executor are about 70,000 lines), and every machine writer of recipes would then be writing source code.
2. **The schema comes from Rust types.** Recipe types live in `construct-families` with `serde` (MIT or Apache-2.0) and `schemars` (MIT), pinned. The schema is generated, published with examples for AI (CON-A7-F5-P2), and checked in CI against the door and the template recipes (CON-A7-F1-P5). Solid steps refer to the kernel's operation schemas by owner and version rather than restating them, so the recipe follows the kernel's 24.4 envelope.
3. **The expression language, named methods:**
   - **Grammar and guarantees:** CEL's: side-effect free, terminating, no loops, no user functions; written with a well-known parser crate (`winnow`, MIT, as KCL uses), not by hand.
   - **Units in the types:** KCL's idea: every number carries a unit, lengths add only to lengths, and `820 mm` and `2 ft 8 in` are both lengths.
   - **Exact numbers:** literals and arithmetic over exact rationals, rounded once to the kernel's grid when a value reaches a geometry step; angles by the kernel's angle units; trigonometric functions only through the kernel's certified functions. The precise rules are a Design question with the kernel's owners.
   - **Limits:** a cap on expression size and nesting, and on a recipe's bytes, parameters, steps and nested depth, in the manner of Pascal's `RECIPE_LIMITS` (CON-A7-F1-P6).
4. **Repetition without loops:** a step may take a `count` and read `index`, as Pascal's parts do, for mullions, shelves and balusters. Arrays from a parameter come later, if a family needs them.
5. **Canonical form:** keys in schema order or sorted, formulas stored as written and also as their parsed form's hash, values with units as strings in the text form, and deterministic CBOR in records (24.1). The text form in the shared library's git repository is formatted by one tool, so diffs show only real changes.
6. **Keep option C open.** If people later want to write families as text, a readable syntax over the same JSON model (the language file in `A07-door/` is what it might look like) can be added as a view that converts both ways. The stored form, the schema and every writer stay as they are.

## Open risks

- **JSON is verbose for people.** Most people will meet recipes through the family editor and AI, not the file; reviewers will read diffs. If reviewing JSON recipes proves hard, option C is the remedy, at the cost of a second syntax to keep in step.
- **JSON has no comments.** The schema allows a `note` field on every object, so explanations survive machine edits, which source comments in a language often do not.
- **The expression language is still a language.** It needs a grammar, a parser on a well-known crate, type checking with units, and tests; it is small, but not free. CEL's conformance tests are a model for its own.
- **Constraint sketches are not all rectangles.** The door's sketches are rectangles between planes; arched heads, glazing bars and profiles need sketch entities and constraints, which follow the kernel's sketch schemas (chapter 15) and the solver (A11). Their JSON shape is a Design question.
- **Revit families do not map cleanly.** Conversion (CON-A7-F8) needs its own legal clearance and its own note; nothing here depends on reading Revit's files.

## Sources

| Source | Licence | Used for |
| --- | --- | --- |
| [KCL in KittyCAD/modeling-app](https://github.com/KittyCAD/modeling-app/tree/3334e6fddd9163d9b60188a597a6ed18cdeb9075) | MIT | A purpose-built CAD language: units, sketches, cost, editor round trip |
| [Pascal editor](https://github.com/pascalorg/editor/tree/51bf4071c10d0189c2499ec2069e580b74169106) | MIT | JSON recipes with a schema for agents, limits, repetition, expression trees |
| [CEL language definition](https://github.com/cel-expr/cel-spec/blob/21ec6f4e52e8d694687ff9ed7408d3eccbb955d7/doc/langdef.md) | Apache-2.0 | Guarantees of a safe expression language |
| [cel-rust](https://github.com/cel-rust/cel-rust) | MIT | A Rust CEL implementation; not read in source |
| [schemars 1.2.3](https://github.com/GREsau/schemars/tree/v1.2.3) | MIT | JSON Schema 2020-12 from Rust types |
| [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785) | IETF | Canonical JSON |
| Kernel map, chapters [24](../../engines/geometry/architecture/24-file-format-exchange.md) and [25](../../engines/geometry/architecture/25-public-interface.md) | This repo | Recipe envelope, CBOR records, operation schemas |
