# Method note: what FreeCAD's element map teaches about persistent names

| Field | Value |
| --- | --- |
| Source | FreeCAD's element map, the topological naming mitigation merged for FreeCAD 1.0: the element map and mapped names of its application layer, the routine of its Part workbench that builds a result's element map, and the reference rescue in its property links |
| Licence | LGPL-2.1-or-later |
| Ledger row | Chapter 29.10, "FreeCAD element map" (PORT METHOD; STUDY under the method-only procedure, owned by chapter 20) |
| Pinned link | https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Part/App/TopoShapeExpansion.cpp |
| Files read | At commit `9946038aace97af288eb28f2727a169f9d1158c7`: `src/Mod/Part/App/TopoShapeExpansion.cpp` lines 926 to 943, 1277 to 1312, 1363 to 1417 and 1420 to 2008; `src/App/ElementMap.h` lines 1 to 6; `src/App/ElementMap.cpp` lines 77 to 106, 554 to 694 and 756 to 780; `src/App/MappedName.h` lines 46 to 56; `src/App/ElementNamingUtils.h` lines 51 to 92; `src/App/StringHasher.h` lines 55 to 75; `src/App/StringHasher.cpp` lines 262 to 297; `src/App/IndexedName.h`; `src/App/GeoFeature.cpp` lines 112 to 138; `src/App/PropertyLinks.cpp` lines 434 to 535; `src/App/Document.cpp` lines 126 to 138 and 3448 to 3449 |
| Read by | The G01 research session, one Claude Opus 5.5 agent conversation, on 2026-10-21, which described these files in plain words and copied no code in evidence report 07, sections 1, 2 and 8: `projects/kit/notes/G01-evidence/07-persistent-naming.md` |
| Written by | The G16 session, one Claude Opus 5.5 agent conversation, on 2026-10-24, from report 07 and chapter 20 only. It did not open the FreeCAD source |
| Second review | The approval of the pull request from piece G16 that adds this note: its reviewer checks the note against the files above for copied expression, not for method (29.20, rule 6, step 4). No code is written from the note before that approval |
| Code written from it | None yet. Persistent names belong to `geom-name` (chapters 20.2 and 27); the piece that builds them writes its code from chapter 20, this note and the papers below, in a session that has not read the FreeCAD source |

## The problem

A reference such as "round this edge" or "dimension to this face" is stored against a shape that regeneration rebuilds from scratch. Report 07 records each way it breaks, with a case:

- **Renumbering.** Parts named by their position in a traversal (the third face) land on another part when an earlier edit adds or removes one.
- **Split.** A slot cut across a block divides the edge to be rounded into two; the rounding must decide which it meant. In a building: a door opening that reaches the base of a wall splits the wall's outer face, and a dimension to "the outer face" becomes ambiguous.
- **Merge.** A union or a clean-up step fuses coplanar faces, so two references now point at one face.
- **A changing count.** Adding a hole to an extruded profile changes how many side faces there are.
- **Consumed parts.** A fillet deletes the edge it rounds, yet the fillet must still be re-evaluated against that edge's name.
- **Reorder.** When one input yields several outputs, a name that counts them in the order the underlying kernel reports them changes when that order does. Merging a shared operation log reorders operations in exactly this way.

## The method

FreeCAD's answer, in plain words:

1. Every shape carries a two-way map between each part's positional name and one or more persistent names. A persistent name is built from a fixed vocabulary of markers (a part modified from a source, generated from one, both, named from the parts above or below it, an operation tag, an index, a child, a duplicate).
2. For each operation, the names of parts that pass through unchanged are copied from the inputs.
3. For every input vertex, edge and face, the routine asks the underlying kernel's history which result parts it modified and which it generated (the relations of [occt-history-contract.md](occt-history-contract.md)). Each result part collects the set of input parts it came from, sorted by kind of part, then source, then name. Sources reported only at the level of whole solids or shells are held back, to be used only if nothing more precise names the part.
4. A result part's name is composed from its first source's name, the relation, the position at which the kernel reported it, a short list of further sources, the operation's code and the owning object's tag. Long names are interned in a per-document table and replaced by short identifiers.
5. A downward pass names each vertex or edge still unnamed after a named face or edge above it, with an ordinal. An upward pass names each face or edge still unnamed from the parts below it, but only when all of those are named; for a face, only its outer boundary is used.
6. If anything is still unnamed, the passes run once more using the sources held back in step 3.
7. When two parts would get the same name, a duplicate suffix is added. When a stored name is no longer found, a search by geometry looks for a matching part, widening its tolerance step by step, and accepts the first unique match.

## Invariants

What Spare Kit takes from this, each as a property a test can check (chapter 20.2 states them as Spare Kit's rules):

1. A part's name depends only on the operation that made it and the names of its sources: never on positions, report order, documents, clocks or random numbers.
2. A part that passes through an operation unchanged keeps its name; a part modified one-to-one keeps its name too, and the change is recorded in the history.
3. After the operation every part of the result has a name: from its sources, then from the faces it bounds, then from the parts below it; a name that would still collide becomes a weak name ordered by a fixed geometric rule, flagged as weak.
4. The same operation log replayed anywhere gives the same names, bit for bit.

## Failure cases

The element map's idea is sound; report 07 found each of these in its implementation, and chapter 20 rules each one out. Each is a test Spare Kit should carry.

- **A random duplicate suffix.** In release builds the duplicate suffix of step 7 is a random number, so the same model gets different names on different runs. Spare Kit: a duplicate the operation's contract does not expect is a bug caught by tests, and in production it becomes a weak name with a fixed ordinal, never a random suffix (20.2).
- **A random per-document offset.** Operation tags are document object numbers, and a new document starts counting at a random offset, so the same history replayed in a fresh document gives different names. Spare Kit: operation identifiers are fixed when the operation is authored and stored in the log (20.1).
- **Per-document interning.** The short identifiers of step 4 are handed out in order per document, so names do not travel between documents ("hasher mismatch"). Spare Kit: a name is a hash of a canonical encoding, the same in every document (20.1).
- **Report order in names.** The position of step 4 is the order in which the underlying kernel reported its results, so a change in that order renames parts. Spare Kit sorts everything by name, never by report order (20.2).
- **Geometric rescue.** When a name is lost, the search of step 7 guesses by geometry with a growing tolerance, and the changed reference is logged only at a low log level; FreeCAD's own comment calls it heuristic. Spare Kit has no geometric guessing: a reference that does not resolve is an `UnresolvedRef` on every machine, and a repair is a new operation in the log (20.4).
- **Lost references marked, not explained.** A reference that is lost is rewritten with a marker in front of it, and the user sees a broken link. Spare Kit returns a typed resolution (`Deleted`, `Ambiguous`, `WeakChanged`, `HistoryPruned`) with the operation and candidates (20.4).
- **Cost.** The FreeCAD developers' informal measurement on one model of 148 objects showed recompute time rising by about 30% and file size by about 27%. Spare Kit prunes history no live name or reference reaches when it saves (20.4, 24.4), and sets memory budgets for name tables (chapter 20's open questions).

## Published sources

- V. Capoyleas, X. Chen and C. M. Hoffmann, "Generic naming in generative, constraint-based design", *Computer-Aided Design* 28(1), 1996, <https://doi.org/10.1016/0010-4485(95)00014-3>: names tied to the operation and its sources rather than to a model.
- D. Kripac, "A mechanism for persistently naming topological entities in history-based parametric solid models", *Computer-Aided Design* 29(2), 1997, 113 to 122, <https://doi.org/10.1016/S0010-4485(96)00040-1>.
- D. Marcheix and G. Pierra, "A survey of the persistent naming problem", 2002, <https://doi.org/10.1145/566282.566288>, and D. Marcheix's later work on named shells, 2005, <https://www.lias-lab.fr/publications/7550/2006-CAD-CAM-Marcheix.pdf> (doi:10.1109/CAD-CG.2005.12): parts fully determined by the operation, named by it, apart from contingent parts named with their neighbourhood; the split case of a rounded edge.
- R. Bidarra, P. J. Nyirenda and W. F. Bronsvoort, "A feature-based solution to the persistent naming problem", *Computer-Aided Design and Applications* 2, 2005, 517 to 526, <https://doi.org/10.1080/16864360.2005.10738401>: splits told apart by the faces that cut them and the side, which chapter 20.2's `ByTools` takes.
- The element map's design as its author describes it, on the FreeCAD assembly3 wiki page "Topological Naming Algorithm", <https://github.com/realthunder/FreeCAD_assembly3/wiki/Topological-Naming-Algorithm/3d96bd6bf6ae32fa726eee9c067471551ab99f6d>, with its own account of the method's limits and its cost measurement.
- The kernel architecture's own design: chapter 20, `projects/engines/geometry/architecture/20-naming-history.md`.
