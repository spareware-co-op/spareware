# Warnings, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3: the one shape in which a model's problems are read. **Nothing here is stored.** The list is worked out from the document each time it is asked ([`src/lib.rs`](src/lib.rs), `warnings`), so it is always the list for the document as it stands, and undo and redo need nothing of it. The decisions behind it are in [the warnings note](../../notes/CON-A2-F7-P1-warnings.md).

## A warning

| Field | Value |
| --- | --- |
| `identity` | The kind and the elements, joined with `\|`: `wall.overlap\|W-1\|W-2`. The same problem has the same identity for as long as it lasts, however the rest of the model changes |
| `kind` | A stable name: `room.not_enclosed`, `wall.overlap`, `dimension.reference_missing`, `dimension.reference_parallel` or `dimension.out_of_range` |
| `severity` | `notice`, `warning` or `error`; every kind is `warning` |
| `elements` | The permanent IDs of the elements involved, in the order the kind states |
| `words` | A plain sentence for people |

| Kind | Elements | Found when |
| --- | --- | --- |
| `room.not_enclosed` | The room | The walls of its level do not close round its point (construct-rooms' `NotEnclosed`) |
| `wall.overlap` | The two walls, lower ID first | Two walls on one level have exactly parallel reference lines and outlines that share a region of positive area |
| `dimension.reference_missing` | The dimension, then the element it references | A reference of a dimension cannot be found in its view (the element or the feature is gone, or it has no place there) |
| `dimension.reference_parallel` | The dimension, then the element it references | A line reference runs along the dimension's offset line, so it has no place along it |
| `dimension.out_of_range` | The dimension, then the element it references | A value or a point of a dimension does not fit the kernel's grid |

## The list

By kind name, then by the elements' IDs, ascending, each identity once. A model with no problems gives an empty list. The server reads it at `GET /api/projects/<project>/models/<model>/warnings` (the right to read the model), as `{"warnings": [ {identity, kind, severity, elements, words}, ... ]}`.

```json
{"warnings":[{"identity":"room.not_enclosed|R-900","kind":"room.not_enclosed","severity":"warning","elements":["R-900"],"words":"Room 101 Studio is not enclosed: the walls round its point do not close"}]}
```

A new kind is one more entry in `RULES`; it needs a name that is never reused for another problem.
