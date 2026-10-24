# Method note: the history contract of OpenCascade's operations

| Field | Value |
| --- | --- |
| Source | OpenCascade Technology (OCCT): the history class `BRepTools_History`, the history section of its boolean operations specification, and the naming chapter of its application framework guide |
| Licence | LGPL-2.1-only WITH OCCT-exception-1.0 |
| Ledger row | Chapter 29.10, "OCCT `TNaming` and `BRepTools_History`" (PORT METHOD, owned by 20.3 and 20.4); chapter 29.5, "OpenCascade (OCCT)" |
| Pinned link | https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/BRepTools/BRepTools_History.hxx |
| Files read | At commit `53706f78a8640c4407bdcc6752df13a270b159f2`: `src/ModelingData/TKBRep/BRepTools/BRepTools_History.hxx` lines 27 to 88; `dox/specification/boolean_operations/boolean_operations.md` lines 2953 to 2968; `dox/user_guides/ocaf/ocaf.md` lines 954 to 985 |
| Read by | The G01 research session, one Claude Opus 5.5 agent conversation, on 2026-10-21, which described these files in plain words and copied no code in evidence reports 02 (section 3, "History") and 07 (sections 3 and 6): `projects/kit/notes/G01-evidence/02-occt-parasolid-acis.md` and `projects/kit/notes/G01-evidence/07-persistent-naming.md` |
| Written by | The G16 session, one Claude Opus 5.5 agent conversation, on 2026-10-24, from those two reports and chapter 20.3 only. It did not open the OCCT source. Where this note goes beyond the reports (the composition rules below), it says so and derives what it states |
| Second review | The approval of the pull request from piece G16 that adds this note: its reviewer checks the note against the files above for copied expression, not for method (29.20, rule 6, step 4). No code is written from the note before that approval |
| Code written from it | None yet. The history every operation emits belongs to the operation contract in `geom-op` and the names in `geom-name` (chapters 20.3 and 27); the piece that builds it writes its code from this note and the papers below, in a session that has not read the OCCT source |

## The problem

An operation turns input shapes into output shapes. Everything that refers to a part of a shape needs to know, for each part of the result, where it came from, and which parts of the inputs are gone: persistent names (chapter 20.2), references and their resolution (20.4), attributes that must follow a face through a split, drawings that keep their dimensions, and undo. A concrete case: a slot cut across a block splits the block's top face in two, creates new side faces in the slot, and removes nothing. A fillet on one of the block's edges removes that edge and creates the rounded face in its place. If each operation reports this in its own way, or reports it by the position of parts in a list, every consumer has to guess, and names break.

## The method

The source answers with one record per operation, relating parts of the inputs to parts of the result by exactly three relations:

1. **Generated.** A part of the result was made from a part of an input, and is a new thing rather than that part changed: the edge where two faces cross is generated from those faces; the vertex where three faces meet is generated from them; a face swept by a profile edge is generated from that edge.
2. **Modified.** A part of the result is a part of an input after a change: the same face moved, trimmed or extended; one of the pieces of a face that was split; or one face merged from several coplanar faces, which is then modified from each of them.
3. **Removed.** A part of an input has no counterpart in the result.

A part that passes through the operation unchanged is neither: it is simply in the result as it was.

The record is kept for vertices, edges, faces and solids. Aggregates (wires, shells and compounds) are not tracked: they are derived from the parts they hold, by selection.

Which of generated or modified applies is decided by rules on dimension and joining. A result part of the same kind as the input part it stands for, carrying on that part, is modified from it; a part made where inputs meet, which none of them was before, is generated. For booleans the specification is explicit: generated applies only to the new vertices and edges where the arguments intersect, and parts where the arguments overlap or coincide count as modified.

The source's application framework guide states what an algorithm must report so that names can be built on the record: for a solid, all of its faces; for an open shell, its faces and the edges on its open boundary; for a wire, its edges and their end vertices. Seam edges are tracked; degenerate parts are left out.

Histories of operations run one after another compose: the history of the chain is computed from the history of each step. The reports record that the source defines this composition with fixed rules, but not the rules themselves. The rules below are derived here from the meaning of the three relations, not read from the source; they are what Spare Kit's tests should check. For two steps, writing an unchanged part as its own image:

- the modified images of an input through the chain are the modified images, in the second step, of its modified images in the first;
- its generated images through the chain are everything the second step makes from its generated images in the first (generated, modified or unchanged), together with what the second step generates from its modified images in the first: once a part is generated from an input, its descendants stay generated from that input;
- it is removed by the chain when it has no modified image through the chain.

Longer chains compose two steps at a time. This composition is associative, because each rule only follows images forward and the choice between generated and modified never depends on how the steps were grouped.

## Invariants

These are the source's contract as the reports record it, each a property a test can check on every operation:

1. An input part is never both removed and modified.
2. For any input part, its generated parts and its modified parts do not overlap.
3. Every part of the result is an input part that passed through unchanged, or is generated or modified from some input part.
4. Generated or modified is chosen by the dimension and joining rules above, the same way for every operation of a kind.
5. Histories compose across chained operations, and the composed history keeps invariants 1 to 3.

Chapter 20.3 adopts the same rules for Spare Kit, stated over its own vocabulary (unchanged, modified, generated with a role, split with its tools and side, merged, and deleted), and adds one case that the source's naming framework records separately, as a primitive evolution with no old shape (report 07, section 3): an operation that makes parts from nothing, such as a box, generates them from no input, with a role (20.2's `Created`), so invariant 3 reads "every output traces to an input or is generated".

## Failure cases

Each is a case Spare Kit's tests should cover.

- **Removed yet generating.** A fillet removes the edge it rounds and generates the rounded face from it. Removed and generated together is allowed; removed and modified together is not (invariant 1).
- **Splits.** One input face becomes several result faces, each modified from it. The record alone does not tell the pieces apart; Spare Kit names them by the faces that cut them and the side (20.2's `Split` with `ByTools`), and only when that still collides by an ordinal (`Weak`).
- **Merges.** Several input faces become one, modified from each. Spare Kit names the result `Merged` over the sorted set of sources.
- **Two paths to one part.** In a chain, a part can come to relate to one input by both relations: a piece modified from a face, merged in a later step with a face generated from that same face. Invariant 2 then fails after composition unless a rule chooses one relation. The reports do not record the source's rule, so the piece that builds Spare Kit's history decides it, writes it into chapter 20.3's contract and tests it.
- **Report order as identity.** A consumer that takes the order in which an operation reports its parts as their identity renames parts whenever that order changes (report 07, section 1, "Reorder"). Spare Kit sorts every record by persistent name (20.2).
- **Identity in memory.** The source relates shapes by their identity in memory, its shared topology handles, so a record keyed that way does not survive saving, reloading or another machine (report 02, section 1; report 07, section 3). Spare Kit keys every relation by persistent name, never by address or arena index.
- **Aggregates.** A reference to a wire, shell or compound has no history of its own; it is resolved from the history of its parts.

## Published sources

- D. Kripac, "A mechanism for persistently naming topological entities in history-based parametric solid models", *Computer-Aided Design* 29(2), 1997, 113 to 122, <https://doi.org/10.1016/S0010-4485(96)00040-1>: a face history graph recording creation, split, merge and deletion, the relations above in their earliest published form.
- V. Capoyleas, X. Chen and C. M. Hoffmann, "Generic naming in generative, constraint-based design", *Computer-Aided Design* 28(1), 1996, <https://doi.org/10.1016/0010-4485(95)00014-3>.
- D. Marcheix and G. Pierra, "A survey of the persistent naming problem", *Proceedings of the Seventh ACM Symposium on Solid Modeling and Applications*, 2002, 13 to 22, <https://doi.org/10.1145/566282.566288>: invariant parts named by the operation, contingent parts by their neighbourhood.
- R. Bidarra, P. J. Nyirenda and W. F. Bronsvoort, "A feature-based solution to the persistent naming problem", *Computer-Aided Design and Applications* 2, 2005, 517 to 526, <https://doi.org/10.1080/16864360.2005.10738401>.
- The kernel architecture's own statement of the contract: chapter 20.3, `projects/engines/geometry/architecture/20-naming-history.md`.
