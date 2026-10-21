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

**Across documents.** None of these says which document an entity is in, and a copy of a document keeps the same `OpId`s and names. So a reference into another document, such as a consultant's linked model, also holds a `DocRef { id, version }`: the linked document's permanent 128-bit ID from `kit-core`, and the version the reference was made against. That version's header fixes the linked document's kernel edition and `NameScheme` (section 24.4). A foreign file is linked through its import (section 24.8). Imported into a document of its own, it is linked like any other document. Imported into the host, its entities are named by that import's `OpId` and need no `DocRef`.

## 20.2 How each operation names what it makes

```rust
enum TopoName {
    Created { op: OpPath, role: Role, from: SortedVec<NameId> },  // made new by this step
    Split   { op: OpPath, of: NameId, piece: PieceKey },           // one input became several
    Merged  { op: OpPath, of: SortedSet<NameId> },                 // several inputs became one
    Bounded { kind: EdgeOrVertex, by: SortedSet<NameId> },         // named by the faces it bounds
    Weak    { base: NameId, ord: Ordinal },                        // a name that still collides, told apart by order
}
enum PieceKey {                                       // an enum, so a later NameScheme can add other keys
    ByTools { tools: SortedSet<NameId>, side: Side }, // which cutting faces, which side
}
struct Ordinal { n: u32, of: u32 }                    // the n-th, counting from 0, of the `of` entities sharing `base`
```

- An unchanged entity keeps its name. **An entity modified one-to-one keeps its name too**; the change is recorded in the history, not in the name.
- **Roles are part of every operation's contract.** An extrusion names its faces `cap.start`, `cap.end` and `side(source edge)`; a wall's layer faces are `exterior`, `interior`, `top`, `base`, `end0`, `end1`; a fillet names `roll(edge)`. This is how Revit's stable door references (left, right, centre, top) work, made general.
- A split is named by the faces that cut it and the side. Only when two pieces still collide is an ordinal used: they take `Weak` names, ordered by the rule below.
- Everything is sorted by name, never by the order a kernel happened to report it or by memory address.
- A duplicate name that the operation's contract does not expect is a bug caught by tests. In production it still becomes `Weak`, never a random suffix.

**Weak names.** `Weak` is the one form for every name that would otherwise collide, whatever its variant. The colliding entities share the collided name as `base`, and the ordinal tells them apart.

- It serves split pieces that share tools and side, section 11.7's unseeded cells, section 11.9's edges between the same two faces, and names that rest only on position: a division set by count (section 16.4), faces inside an imported run (section 19.5) and imported items with no persistent ID (section 24.8).
- A name is **weak** if it is a `Weak` node or is built on one at any depth, since it changes when that ordinal does. `Weak` is part of the canonical encoding that `NameScheme v1` hashes (section 24.3), so a name's own bytes say whether it is weak.
- `n` and `of` are `u32`: an imported mesh run can hold millions of faces, and a cut corrugated sheet hundreds of edges. `of` is part of the name, so when the number of colliding entities changes, every old weak name stops matching, and its references report `WeakChanged` (20.4) instead of quietly landing on a neighbour.
- Whether `Bounded` should also hold the faces that end an edge is section 11's open question 3. Until an owner decides, edges that collide take `Weak`.

**Ordinal order.** `NameScheme v1` fixes one order for every ordinal in every operation. No kernel edition may change it, because ordinals are hashed into names that files keep forever.

| Step | Rule |
| --- | --- |
| Reference | What the entities are ordered on. For split pieces, the entity split. Otherwise, of the base name's parent faces and edges that every colliding entity touches, the one whose `NameId` sorts lowest. With none, the body's own frame |
| Key | Each entity's least point on the reference: the one that comes first by the first parameter, then the second (in a frame: x, then y, then z) |
| Ties | Entities that share their least point, such as two arcs leaving one vertex, are compared by their vertices, each list sorted the same way, like words in a dictionary. Then, in a parameter space, by the direction each leaves the shared point, decided as section 11.7 decides radial order |
| Comparison | Exact. Curved geometry is compared by certified refinement of the true geometry, never by an enclosure an algorithm happened to compute. A comparison still undecided at section 12.3's floor, or a tie still standing, fails the operation with `UndecidedOrder`, never a guess |

`geom-name` sorts whatever an operation reports by this rule. So no name depends on the order an algorithm produced its results in, section 12.15's output order included, nor on the canonical operand encoding, which is versioned with the edition.

## 20.3 History every operation must emit

For every output entity: unchanged, modified from, generated from (with role), split from (with tools and side), or merged from; and the list of deleted inputs. The rules, checked by property tests on every operation: a deleted input has no modified outputs; an input's generated and modified sets do not overlap; every output traces to an input or is generated; histories compose across operations. This is OpenCascade's `BRepTools_History` contract, adopted as a tested invariant.

## 20.4 How a reference resolves

A reference is `{ doc, element, name, kind, want, signature }`, where `want` is `One`, `AllPieces` or `Carrier`. `doc` is empty for the reference's own document and a `DocRef` (20.1) for a link.

1. If the name exists in the element's current body, that is the answer: `Exact`.
2. Otherwise the resolver walks forward through the recorded splits, merges and modifications. One descendant resolves as `FollowedHistory`. Several resolve if the reference wants all pieces, and otherwise return `Ambiguous` with the candidates. None returns `Deleted` with the operation that deleted it. Where the history the walk needs was pruned, the answer is `HistoryPruned`, never `Deleted` (section 24.4).
3. **Weak names** (20.2) resolve with `weak` set, so the interface can ask for a check. A weak name that is gone while its base still names entities returns `WeakChanged` with them as candidates: the set it was counted in has changed, so a person or AI confirms the choice. A span on a source modified since it was stored returns `WeakChanged` too (section 18.8).
4. **Carrier references**, the default for dimensions and hosting, resolve to the underlying plane, axis or surface. When a door opening splits a wall face, every piece shares its carrier, so a dimension to "the exterior face of the wall" does not break.
5. **There is no geometric guessing.** The stored signature is used only to suggest a repair in the interface. An accepted repair is a new operation in the log, so every copy of the model applies the same fix.
6. An unresolved reference is model state, the same on every machine: the feature shows `UnresolvedRef` with the outcome and its reason.

Every outcome is one value of a closed type, so programs and AI agents can handle each case:

```rust
enum Resolution {
    Exact           { target: Target, weak: bool },    // step 1; weak if the name or a result is weak
    FollowedHistory { target: Target, weak: bool },    // step 2: one descendant, or every piece
    Ambiguous       { candidates: SortedVec<NameId> }, // step 2: several, and `want` is One
    WeakChanged     { candidates: SortedVec<NameId> }, // step 3: a person or AI confirms
    Deleted         { by: OpPath },                    // step 2
    HistoryPruned   { by: Option<OpPath> },            // step 2: the PruneHistory operation; None for a
                                                       // linked version shared without its history
    Unresolved      { reason: UnresolvedReason },      // none of the above: the reason says why
}
enum Target { One(NameId), Pieces(SortedVec<NameId>), Carrier(CarrierIx) } // what `want` asked for
```

Other chapters add reasons, never outcomes. Section 17.12's placement references resolve their `Ref` this way and then find a position. A chapter that needs a new reason adds it to this table:

| Reason | When | Section |
| --- | --- | --- |
| `ElementGone` | The element itself no longer exists | 20.4 |
| `LinkMissing` | The linked document or file is not available, or this kernel cannot read its version | 20.4 |
| `LinkOlder` | The linked version supplied is older than the one the reference was made against, and does not yet hold the name | 20.4 |
| `NotInView` | The target lies outside the drawing's view | 18.8 |
| `OffSurface` | A plan position falls outside a surface's boundary, or in a hole or void | 17.12 |
| `PastEnd` | A station lies beyond the end of the alignment | 17.12 |

**References into other documents.** A reference with a `doc` resolves by the same steps, inside the version of that document the program supplies (for Spare Construct, the package last taken in). With none available, it is `Unresolved` with `LinkMissing`. If that version is newer than the one the reference was made against, step 2 walks the linked document's history between the two; where the supplied version does not carry that history, the answer is `HistoryPruned`. If it is older and does not yet hold the name, the answer is `Unresolved` with `LinkOlder`. The host never regenerates a linked document. It reads that document's stored geometry and names, whose meaning no edition changes (section 26.2), so a link may be on a different kernel edition from the host. This is how models stay linked between programs (section 25).
