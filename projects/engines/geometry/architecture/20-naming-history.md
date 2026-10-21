# 20. Persistent naming and history

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Evidence: [07](../../../kit/notes/G01-evidence/07-persistent-naming.md), with OpenCascade's history contract in [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md).

The problem: a reference such as "fillet this edge", "dimension to this face" or "this door is hosted on that wall face" is stored against something that regeneration rebuilds. Kernels that name faces by their position in a list (Face1, Face2) silently point at the wrong face after an upstream change. FreeCAD fought this for fifteen years; its 2024 fix still draws a random number for duplicate names, starts each document's counter at a random offset, and, when a name is lost, picks the nearest-looking face by geometry. All three make names differ between machines.

## 20.1 Three kinds of identity

| Identity | What it is | Lives |
| --- | --- | --- |
| `OpId` | The permanent ID of one operation, set when the operation is authored and stored in the operation log. An `OpPath` adds sub-step labels | In the log and in files, forever |
| `NameId` | A persistent name: a 128-bit hash of a canonical encoding of how the entity was made. Names are hash-consed, so a name is a node in a shared graph that refers to its parents by their `NameId` | In files, forever. The hash function and encoding are versioned (`NameScheme v1`) |
| Index | A typed index into a body version's arena | In memory only, never persisted |

## 20.2 How each operation names what it makes

```rust
enum TopoName {
    Created { op: OpPath, role: Role, from: SortedVec<NameId> },  // made new by this step
    Split   { op: OpPath, of: NameId, piece: PieceKey },           // one input became several
    Merged  { op: OpPath, of: SortedSet<NameId> },                 // several inputs became one
    Bounded { kind: EdgeOrVertex, by: SortedSet<NameId>, ord: u8 },// named by the faces it bounds
}
enum PieceKey {
    ByTools { tools: SortedSet<NameId>, side: Side }, // which cutting faces, which side
    Ordinal { n: u16, of: u16 },                      // last resort, ordered in parameter space; marked weak
}
```

- An unchanged entity keeps its name. **An entity modified one-to-one keeps its name too**; the change is recorded in the history, not in the name.
- **Roles are part of every operation's contract.** An extrusion names its faces `cap.start`, `cap.end` and `side(source edge)`; a wall's layer faces are `exterior`, `interior`, `top`, `base`, `end0`, `end1`; a fillet names `roll(edge)`. This is how Revit's stable door references (left, right, centre, top) work, made general.
- A split is named by the faces that cut it and the side. Only when two pieces still collide is an ordinal used, ordered in the parent surface's parameter space, and marked weak.
- Everything is sorted by name, never by the order a kernel happened to report it or by memory address.
- A duplicate name is a bug caught by tests. In production it gets a weak ordinal, never a random suffix.

## 20.3 History every operation must emit

For every output entity: unchanged, modified from, generated from (with role), split from (with tools and side), or merged from; and the list of deleted inputs. The rules, checked by property tests on every operation: a deleted input has no modified outputs; an input's generated and modified sets do not overlap; every output traces to an input or is generated; histories compose across operations. This is OpenCascade's `BRepTools_History` contract, adopted as a tested invariant.

## 20.4 How a reference resolves

A reference is `{ element, name, kind, want, signature }`, where `want` is `One`, `AllPieces` or `Carrier`.

1. If the name exists in the element's current body, that is the answer.
2. Otherwise the resolver walks forward through the recorded splits, merges and modifications. One descendant resolves with a note that it followed history. Several resolve if the reference wants all pieces, and otherwise return `Ambiguous` with the candidates. None returns `Deleted` with the operation that deleted it.
3. **Carrier references**, the default for dimensions and hosting, resolve to the underlying plane, axis or surface. When a door opening splits a wall face, every piece shares its carrier, so a dimension to "the exterior face of the wall" does not break.
4. **There is no geometric guessing.** The stored signature is used only to suggest a repair in the interface. An accepted repair is a new operation in the log, so every copy of the model applies the same fix.
5. An unresolved reference is model state, the same on every machine: the feature shows `UnresolvedRef` with the reason.
