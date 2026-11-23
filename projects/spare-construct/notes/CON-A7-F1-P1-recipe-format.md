# CON-A7-F1-P1: the family recipe format

Part of CON-A7-F1-P1 (family recipe format, [FEATURES.md](../FEATURES.md)); board piece A07's question. Status: decided by the build's coordinator on 22 November 2026 for the first version ("Family recipes are JSON with a schema", STATUS.md); built in piece A44. The format itself is [construct-families' FORMAT.md](../crates/construct-families/FORMAT.md).

## The question

A family recipe must be text people review in git, AIs write directly, and the door and window generator fills from a spec ([ARCHITECTURE.md](../ARCHITECTURE.md), "Families"). JSON with a schema, or a language of our own?

## Options

1. JSON, with a published JSON Schema and a reader library that checks what a schema cannot.
2. A small typed language of our own, with its own parser, printer and editor support.

## Evidence

- Every tool an AI or a script reaches for already reads and writes JSON, and JSON Schema (draft 2020-12, [json-schema.org](https://json-schema.org/draft/2020-12)) lets any of them check a recipe's shape before it is sent; CON-A7-F5-P2 asks for exactly that ("a published schema with examples so any AI can write valid recipes").
- The parts that need a language already have one: formulas are construct-formulas' language, kept as text inside the JSON, so a recipe needs no second language for expressions.
- A schema cannot see meaning (a formula's units, a point a constraint names, a step that comes later, a material the project lacks). That is the reader library's, which every client uses anyway.
- The schema is checked in the tests with the `jsonschema` crate (0.62.0, MIT; its source was read: draft 2020-12, an integer is any number with no fractional part, default features off so it fetches nothing). It is a test dependency only.

## Decision

Option 1.

- **One document per recipe**, with `format`, a permanent family `id`, and every part the FORMAT.md lists. Everything the format names is reused by name from the crates that own it: construct-params definitions and stored values, construct-formulas formulas, construct-solver's sketch items and constraints, the kernel `Engine`'s operations, construct-materials and construct-categories ids; a family's types are construct-types types with a `@system.family` block, as wall types carry `@system.wall`.
- **Shape and meaning are kept apart.** The reader takes exactly what the schema takes (a test changes a recipe at every place, thousands of ways, and fails if the two ever disagree); the checker reports every problem of meaning at its JSON Pointer (RFC 6901).
- **One written form.** Two-space JSON, members in a fixed order, integers plain, minimal escaping, one final newline. The same recipe is always the same bytes, so a recipe version is named by the SHA-256 of them (its content id), and git diffs show only real changes.
- **Lengths as text**: construct-params' stored spelling, or formulas; never a JSON number, so no float ever enters a recipe.

## How versions change

- The `format` number is the format's version, and each version has its own schema file (`family-recipe-<n>.schema.json`). A reader reads every version there has been, for ever (EXTENDING.md, section 3); a writer writes the newest.
- Within a version nothing changes meaning. A new kind of step, constraint, host or view, a new required member, or any change in how existing text is read, is a new version.
- A **breaking change** is anything after which a recipe that was valid is refused, or reads to a different family: removing or renaming a member, key or word; narrowing a range or pattern; changing a coordinate convention, a normal, a unit or a default; changing the written form (which changes every content id). A breaking change is always a new `format` number, never an edit of version 1.
- **Not breaking**, and so allowed without a new version: a new optional member, written only when present, so every recipe that read before reads to the same family and keeps its bytes and content id (the type table, `type_table`, added so by piece A59; note CON-A7-F1-P2-recipe-evaluation); fixing the reader library so it refuses what the schema already refused, clearer error text, new example recipes, and new parameter kinds or category and material ids that construct-params, construct-categories or construct-materials add (the recipe names them; the format does not list them).
- A recipe version never changes: an edit is a new version with a new content id, and a family type names the version it was made from.

## Open risks

- Lengths change spelling when Length moves to the kernel's grid steps (piece A39): `length:820` becomes `length:q0:5248000`. The examples and the pinned content ids are re-spelt then, as the walls lane's are; the schema is unchanged (a stored value is any text there).
- The `Engine` interface has `extrude` and `subtract` today; `unite` is in the format because the build plan names it, and needs a trait method before job 4.4 can build it. Loft and the other operations ARCHITECTURE.md lists (sweep, revolve, blend) are later versions.
- A sketch's arcs cannot be part of an extrusion's profile yet (the `Engine` takes polygons), and an angle cannot be constrained (the solver has no angle constraint), so a door's swing is chosen by a symbol condition on a choice parameter rather than drawn at an angle.
- Connectors (CON-A7-F1-P1 lists them), nested families, type tables loaded from a spreadsheet and recipe limits are later parts; each will be a new version where it adds members.
