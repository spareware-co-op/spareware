# The category table, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, following [construct-model's FORMAT.md](../construct-model/FORMAT.md): JSON, fields in a fixed order, whole numbers only. **We may stop writing an old version. We never stop reading it.** [`src/table.rs`](src/table.rs) is the reader library.

## What is stored

The built-in tree is in code, not in a project: each built-in category has a permanent id that never changes meaning once released, and `tests/pinned.rs` fails if one is edited. A project stores only its own subcategories.

```json
{"format":1,"subcategories":[{"id":1000000,"parent":100,"name":"Fire rating line"}]}
```

| Field | Meaning |
| --- | --- |
| `format` | The table version. A reader refuses a version it does not know. |
| `subcategories` | The project's own subcategories, ascending by `id` |
| `id` | A whole number. Built-in ids are below 1000000; a project's start there and run on by one, never reused (nothing is ever removed) |
| `parent` | The id of a built-in top-level category |
| `name` | The name people see: trimmed, not empty, unique under its parent without regard to case, counting the parent's built-in subcategories |

A project subcategory takes its parent's kind and its parent's "can be cut" flag.

## Rules a reader checks

A table that breaks one is refused, never repaired: ids are exactly 1000000, 1000001, and so on in order; a parent is a built-in top-level category; names are trimmed and unique under their parent, case aside; no field is unknown. The same table always gives the same bytes.

## Element kinds

A kind name (the `op` of a recipe: `wall.straight`, `wall.layered`, `door.single`, `block`; an operation whose recipe `owner` is not `construct` is named with its owner in front: `workshop.part`, `workshop.assembly`) is mapped to its category by `category_of_kind`. An unknown kind is an error value.
