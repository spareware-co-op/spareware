# Spare Kit geometry kernel: persistent naming, provenance and incremental regeneration

*Evidence for [G01](../G01-geometry-kernel.md), the Spare Kit geometry kernel decision note. Written 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of each project at the pinned commits linked below. GPL and LGPL code is described in the agent's own words, never copied. Claims it could not confirm from a primary source are marked unverified.*

I read the source and pinned every link to a commit SHA on 2026-10-21. These are the pins:

- **FreeCAD** `9946038` (LGPL-2.1-or-later): method only.
- **OCCT** `53706f7` (LGPL-2.1 + OCCT exception): method only.
- **Manifold** `d056b8f` (Apache-2.0).
- **Salsa** `b9041f1` (Apache-2.0 OR MIT).
- **rust-analyzer** `6c1fc43` (MIT OR Apache-2.0).
- **SolveSpace** `4707fa2` (GPL-3.0): method only.

GPL and LGPL methods are described in my own words. No code was copied.

---

## 1. The problem

A reference such as "fillet Edge7" or "dimension to this face" is stored against something that regeneration rebuilds from scratch. Each failure below has a concrete case.

- **Index renumbering.** Most B-rep kernels name sub-shapes by their position in a traversal (Face1..FaceN). FreeCAD's legacy names work this way ([IndexedName.h](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/IndexedName.h)). An upstream edit that adds or removes one face shifts every later index, and the reference then lands silently on a different entity. FreeCAD now marks such a lost reference by rewriting it with a `?` prefix ([GeoFeature.cpp L112-138](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/GeoFeature.cpp#L112-L138), [ElementNamingUtils.h L56-57](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/ElementNamingUtils.h#L56-L57)).
- **Split (1→n).** A slot cut across a block divides the edge to be rounded into two edges, e1 and e2. The rounding step must then decide which of them it means ([Marcheix 2005, §2, Fig. 2](https://www.lias-lab.fr/publications/7550/2006-CAD-CAM-Marcheix.pdf)).
  - BIM case: a door opening that reaches the wall base splits the wall's exterior face into pieces, and a dimension bound to "the exterior face" becomes ambiguous.
  - A live FreeCAD report shows a referenced face lost when part of a stacked coplanar face is removed ([#24530](https://github.com/FreeCAD/FreeCAD/issues/24530)).
- **Merge (n→1).** A refine or union step fuses coplanar faces into one, so two references now point at the same entity. FreeCAD's documented example of a refined name nests both source names ([realthunder wiki @3d96bd6, "Refined face" example](https://github.com/realthunder/FreeCAD_assembly3/wiki/Topological-Naming-Algorithm/3d96bd6bf6ae32fa726eee9c067471551ab99f6d)).
- **Generated count changes.** Adding a hole to an extruded profile changes how many side faces exist. FreeCAD needs a geometric special case just to keep the top and bottom face names stable when holes come and go ([TopoShapeExpansion.cpp L1363-1417](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Part/App/TopoShapeExpansion.cpp#L1363-L1417)).
- **Consumed entities.** A fillet deletes the edge it rounds. A name must therefore survive the entity's disappearance so the fillet can be re-evaluated ([Marcheix 2005 §2](https://www.lias-lab.fr/publications/7550/2006-CAD-CAM-Marcheix.pdf)).
- **Reorder.** When one input produces several outputs, the disambiguating index follows the order the underlying kernel reports them in. Any change to that order renames entities ([wiki, "Limitations"](https://github.com/realthunder/FreeCAD_assembly3/wiki/Topological-Naming-Algorithm/3d96bd6bf6ae32fa726eee9c067471551ab99f6d); [TopoShapeExpansion.cpp L1742-1750](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Part/App/TopoShapeExpansion.cpp#L1742-L1750)). Merging a multi-user operation log reorders features in exactly this way.

## 2. FreeCAD's element map (Zheng Lei, merged in 1.0)

**Status.** FreeCAD 1.0.0 shipped on 2024-11-19 and lists the topological naming problem *mitigation* code first in its release notes ([release](https://github.com/FreeCAD/FreeCAD/releases/tag/1.0.0)). The core files carry "Copyright 2018-2022 Zheng, Lei (realthunder)" and are LGPL-2.1-or-later ([ElementMap.h L1-6](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/ElementMap.h#L1-L6)).

**Data model.**

- Every shape carries an `ElementMap`: a two-way map between an indexed name (`Face3`) and one or more mapped names.
- A mapped name has two parts. An immutable "data" part and an appendable "postfix" part are stored separately so that storage can be shared ([MappedName.h L46-56](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/MappedName.h#L46-L56)).
- Long names are interned in a per-document `StringHasher` as `#hex` IDs. Above a threshold the string is replaced by its SHA-1 hash ([StringHasher.h L55-75](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/StringHasher.h#L55-L75), [StringHasher.cpp L262-297](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/StringHasher.cpp#L262-L297)).
- The name grammar is a fixed set of markers: modified, generated, both, upper, lower, tag, index, child, duplicate ([ElementNamingUtils.h L51-92](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/ElementNamingUtils.h#L51-L92)).

**How a name is generated per operation.** The work happens in `makeShapeWithElementMap` ([L1420-2008](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Part/App/TopoShapeExpansion.cpp#L1420-L2008)).

1. **Copy unchanged names.** Names of sub-shapes that pass through untouched are copied from the inputs ([L1461](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Part/App/TopoShapeExpansion.cpp#L1461)).
2. **Collect sources.** For every input vertex, edge and face, the code asks the OCCT maker which outputs it *modified* ([L1499-1561](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Part/App/TopoShapeExpansion.cpp#L1499-L1561)) and which it *generated* ([L1563-1658](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Part/App/TopoShapeExpansion.cpp#L1563-L1658)).
   - Each output collects a sorted set of candidate sources, ordered by shape type, then source tag, then name ([L1277-1312](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Part/App/TopoShapeExpansion.cpp#L1277-L1312)).
   - Sources reported at solid or shell level are pushed back. They are used only if nothing more precise exists.
3. **Compose the name.** The first source becomes the prefix. The modified or generated marker carries the position the maker reported. Up to four further sources are added in a parenthesised list, which is hashed when the hasher is enabled. The operation code and an owner tag are appended at the end ([L1661-1816](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Part/App/TopoShapeExpansion.cpp#L1661-L1816), [encodeElementName L620-694](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/ElementMap.cpp#L620-L694)).
4. **Downward pass.** Any vertex or edge still unnamed inherits the name of a named face or edge above it, plus an ordinal ([L1818-1904](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Part/App/TopoShapeExpansion.cpp#L1818-L1904)).
5. **Upward pass.** Any face or edge still unnamed is named from its lower elements, but only if all of them are named. For faces, only the outer wire is used ([L1906-2008](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Part/App/TopoShapeExpansion.cpp#L1906-L2008)).
6. **Retry.** If anything is still unnamed, the loop repeats once using the deferred solid-level sources.

**How ambiguous cases resolve.**

- *Duplicate names.* `setElementName` retries up to 100 times, adding a duplicate suffix each time ([ElementMap.cpp L554-617](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/ElementMap.cpp#L554-L617)). **In release builds that suffix is a random number from 1 to 10000** drawn from `std::random_device` ([L756-780](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/ElementMap.cpp#L756-L780)). This is non-deterministic.
- *Missing references.* If the name is gone, FreeCAD searches by geometry. The search widens its tolerance step by step and accepts the first unique match, which is added in Sept 2026 as `rescueDriftedElement` ([PropertyLinks.cpp L434-447](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/PropertyLinks.cpp#L434-L447), [L500-535](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/PropertyLinks.cpp#L500-L535), [commit d45ff36](https://github.com/FreeCAD/FreeCAD/commit/d45ff368a7b1b44f07b4af9f71a7113b12be0139)). The code comment itself calls this "a bit heuristic". The resulting change to the reference is logged at log level only.
- The wiki says history-walking recovery works poorly when the change happened several steps back, and that automatic guessing "needs user attention" ([wiki](https://github.com/realthunder/FreeCAD_assembly3/wiki/Topological-Naming-Algorithm/3d96bd6bf6ae32fa726eee9c067471551ab99f6d)).

**Cost.** In an informal test on 148 objects, recompute time rose from 40 s to 52 s. File size rose from 17.8 to 22.6 MiB uncompressed, and from 2.8 to 3.6 MiB compressed, with the hasher on ([wiki](https://github.com/realthunder/FreeCAD_assembly3/wiki/Topological-Naming-Algorithm/3d96bd6bf6ae32fa726eee9c067471551ab99f6d)).

**Remaining failures (verified).**

- There are 48 open issues labelled "Topic: Toponaming" and 195 closed ([query](https://github.com/FreeCAD/FreeCAD/issues?q=label%3A%22Topic%3A%20Toponaming%22)). Examples:
  - fillet "Invalid edge link" after a nearby pocket ([#26889](https://github.com/FreeCAD/FreeCAD/issues/26889));
  - pad length change breaking downstream sketches ([#29154](https://github.com/FreeCAD/FreeCAD/issues/29154)).
- **"Hasher mismatch".** Hash IDs are allocated sequentially per document ([StringHasher.cpp L297](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/StringHasher.cpp#L297)), so names do not travel between documents ([TopoShapeExpansion.cpp L926-943](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/Mod/Part/App/TopoShapeExpansion.cpp#L926-L943); [#27208](https://github.com/FreeCAD/FreeCAD/issues/27208), [#26192](https://github.com/FreeCAD/FreeCAD/issues/26192)).
- **Tags depend on the document.** Tags are document object IDs, and a new document starts its ID counter at a random offset from 0 to 5000 ([Document.cpp L126-138](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/Document.cpp#L126-L138), [L3448-3449](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/Document.cpp#L3448-L3449)). The same modelling history replayed in a fresh document therefore produces different names.

**Lesson.** The idea is sound: names built from operation plus source names, with up and down propagation. The implementation, however, mixes in OCCT report order, a random suffix, document-local counters and geometric guessing. None of those survive a requirement for bit-identical, multi-replica results.

## 3. OCCT OCAF TNaming (method only)

**Recording.** A `TNaming_NamedShape` holds a list of (old, new) shape pairs that all share one *evolution* value ([TNaming_Evolution.hxx L20-60](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ApplicationFramework/TKCAF/TNaming/TNaming_Evolution.hxx#L20-L60)):

| Evolution | Old shape | New shape |
|---|---|---|
| PRIMITIVE | null | created |
| GENERATED | generator | created |
| MODIFY | before | after a split or merge (a split gives many pairs with the same old shape; a merge gives many pairs with the same new shape) |
| DELETE | deleted | null |
| SELECTED | unused | a named selection |
| REPLACE | exists in the enum but is not in the guide's list | |

`TNaming_Builder` enforces one evolution type per builder ([TNaming_Builder.hxx L30-71](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ApplicationFramework/TKCAF/TNaming/TNaming_Builder.hxx#L30-L71)).

The guide is explicit about what an algorithm must report ([ocaf.md L954-985](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/ocaf/ocaf.md?plain=1#L954-L985)):

- for solids, all faces;
- for open shells, faces plus the edges on open boundaries;
- for wires, edges plus end vertices;
- seam edges are tracked; degenerate entities are excluded;
- aggregates (wire, shell, compound) are *derived* by selection, not tracked.

In the guide's two-box fuse example, faces are named as modifications of a box face, edges as the intersection of two adjacent faces, and vertices as the intersection of three ([L876-893](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/ocaf/ocaf.md?plain=1#L876-L893)).

**Selection and recompute.** `TNaming_Naming::Name` builds a *name*: a small expression tree that is re-solved after every upstream change ([TNaming_Naming.hxx L54-87](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ApplicationFramework/TKCAF/TNaming/TNaming_Naming.hxx#L54-L87)).

- The node types are listed in [TNaming_NameType.hxx L21-35](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ApplicationFramework/TKCAF/TNaming/TNaming_NameType.hxx#L21-L35): IDENTITY, MODIFUNTIL, GENERATION, INTERSECTION, UNION, SUBSTRACTION, CONSTSHAPE, FILTERBYNEIGHBOURGS, ORIENTATION, WIREIN, SHELLIN.
- `BuildName` first identifies the selection from history: what is it a modification or generation of, or what does it intersect? If that is not unique, it adds a neighbour filter, which recursively names adjacent entities ([TNaming_Naming.cxx L1054-1299](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ApplicationFramework/TKCAF/TNaming/TNaming_Naming.cxx#L1054-L1299); Filter steps described at [L714-1003](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ApplicationFramework/TKCAF/TNaming/TNaming_Naming.cxx#L714-L1003)).
- The last resort, CONSTSHAPE, is a plain position index inside a NamedShape ([TNaming_Name.cxx L551-603](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ApplicationFramework/TKCAF/TNaming/TNaming_Name.cxx#L551-L603), [FindIndex L219-233](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ApplicationFramework/TKCAF/TNaming/TNaming_Naming.cxx#L219-L233)).
- `Solve` "updates… else it fails" ([ocaf.md L1000](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/dox/user_guides/ocaf/ocaf.md?plain=1#L1000)). That fail-loud behaviour is good.
- `Name()` has a `Geometry` flag for callers that only need the underlying surface or curve, for example constraints ([L55-57](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ApplicationFramework/TKCAF/TNaming/TNaming_Naming.hxx#L55-L57)). This is directly relevant to BIM dimensions.

**Weaknesses.**

- Names hang off TDF label trees and TShape identity.
- Neighbour filters grow the name with the size of the local topology.
- CONSTSHAPE reintroduces index fragility.

## 4. Academic methods

- **Capoyleas, Chen & Hoffmann 1996, "Generic naming…"** ([doi](https://doi.org/10.1016/0010-4485(95)00014-3)).
  - Method: names are generic, tied to the editable design representation rather than to a particular model.
  - Entities that a feature always creates are named by the profile source entity plus the sweep step.
  - Context-dependent entities are named by the smallest unambiguous topological neighbourhood, plus feature orientation. Matching is covered in a companion paper ([Marcheix 2005 §2](https://www.lias-lab.fr/publications/7550/2006-CAD-CAM-Marcheix.pdf)).
  - Strengths: originated the structural vocabulary and a careful catalogue of ambiguity cases.
  - Weaknesses: names *every* entity, so it is heavy; little on matching during re-evaluation; limited to sweep features.
- **Kripac 1997, "A mechanism for persistently naming…"** ([CAD 29(2):113-122](https://doi.org/10.1016/S0010-4485(96)00040-1); SMA'95 version [doi](https://doi.org/10.1145/218013.218024)).
  - Method: a face-history graph records face creation, split, merge and deletion. Edges and vertices are identified through their faces.
  - Matching does a backward search to an already-matched ancestor face, then a forward search with cross-analysis of neighbouring face cycles.
  - Weaknesses: approximate matches are not recorded as approximate. Pieces can be lost on irregular subdivision ([Marcheix 2005 §2, Fig. 4](https://www.lias-lab.fr/publications/7550/2006-CAD-CAM-Marcheix.pdf)).
  - Strength: history-graph naming scales and is simple.
  - Possible use in Autodesk Mechanical Desktop: *unverified*.
- **Marcheix & Pierra 2002, "A survey of the persistent naming problem"** ([SMA'02 pp. 13-22](https://doi.org/10.1145/566282.566288)). The full text was not accessible to me. Through later work by the same group, I verified a distinction between two kinds of entity:
  - *Invariant* entities are fully determined by the operation and its inputs. They are named ⟨step, invariant stamp⟩, for example ⟨generator, director⟩ for sweeps.
  - *Contingent* entities come from interaction with existing geometry. They are named ⟨step, ordinal⟩ plus a neighbourhood descriptor used for matching.
  - Later work extends this to named shells (face sets) referenced by design intent ([Marcheix 2005 §3-4](https://www.lias-lab.fr/publications/7550/2006-CAD-CAM-Marcheix.pdf), [doi](https://doi.org/10.1109/CAD-CG.2005.12)).
  - Strength: honest separation of what is guaranteed from what is heuristic.
  - Weakness: contingent matching is still heuristic.
- **Bidarra, Nyirenda & Bronsvoort 2005** ([CAD&A 2:517-526](https://doi.org/10.1080/16864360.2005.10738401); [abstract](https://publications-cgv01.ewi.tudelft.nl/papers/626)).
  - Argument: the problem is inherent in referencing boundary entities that do not persist.
  - Method: reference *feature faces*, which persist in a declarative feature model. Name edges by those faces, and automatically add half-space references when an intersection is ambiguous.
  - Strength: robust to reordering, and it carries semantics.
  - Weakness: needs a semantic feature model (cellular model *unverified*), so it is heavier than history naming.
- **Baba-Ali, Marcheix & Skapin 2009** ([CAD&A 6(3):341-350](https://doi.org/10.3722/cadaps.2009.341-350); [HAL](https://hal.science/hal-00383282)).
  - Method: adds shape characteristics. Curved surfaces are decomposed into hump and hollow sub-surfaces (using local extrema and inflexion curves) so that edge matching stays right on non-planar faces.
  - Strength: improves matching on free-form surfaces.
  - Weakness: geometric and tolerance-dependent, so it sits poorly with determinism.
- **Farjana & Han 2018 review** ([AEJ 57(4):2837-2849, CC BY-NC-ND](https://doi.org/10.1016/j.aej.2018.01.007)).
  - Classifies methods as topology-based, geometry-based, hybrid, feature-orientation-based and faceIDGraph-based.
  - Concludes that a combined topology-plus-geometry approach is needed.
  - I read only the abstract; finer details are *unverified*.
- **Context papers.**
  - Raghothama & Shapiro give the first formal treatment (BR-invariance, families of objects), but it does not handle splits and merges ([doi](https://doi.org/10.1145/293145.293148); critique in Marcheix §2).
  - Wang & Nnaji propose geometry-based semantic IDs ([doi](https://doi.org/10.1016/j.cad.2004.11.009)).
  - Cardot et al. 2019 use graph-transformation rules ([doi](https://doi.org/10.14733/cadaps.2019.985-1002)).

## 5. Commercial systems (public documents only)

**Parasolid** (XT Format Reference, Siemens, ~2006; [mirror](http://www.13thmonkey.org/documentation/CAD/Parasolid-XT-format-reference.pdf), [archive](https://web.archive.org/web/20230806012108/http://www.13thmonkey.org/documentation/CAD/Parasolid-XT-format-reference.pdf)).

- Every entity except fins has a node-id that is unique *within the part*. Its stated purpose is to identify the entity within a transmit file (p. 23).
- Attribute definitions declare an action for each event: split, merge, transfer, change, rotate, scale, translate, reflect. The possible actions are:
  - do-nothing, delete or transform;
  - propagate (copy the attribute onto the split-off piece);
  - keep-if-equal (on merge);
  - combine (move attributes from the deleted node onto the survivor).
- Seven predefined classes bundle these actions (pp. 99-101).
- Takeaway: the kernel exposes *provenance policy per attribute*, and applications build naming on top of it. That application-level naming exists is *unverified*.

**Onshape** ([FeatureScript library docs](https://cad.onshape.com/FsDoc/library.html), accessed 2026-10-21).

- Operation `Id`s are hierarchical, and "the full id hierarchy must reflect creation history".
- `qCreatedBy(id, type)` queries by creating operation.
- `evaluateQuery` returns *transient* queries that may become invalid after the context changes.
- `makeRobustQuery` / `startTracking` follow identity-preserving changes and, optionally, splits and merges.
- `unstableIdComponent` marks index-like ID parts as wildcards, and `setExternalDisambiguation` lets authors bind identity to an input entity.
- An Onshape employee states on the forum that IDs are stable only within one microversion ([forum](https://forum.onshape.com/discussion/comment/54649/)). The internal "deterministic ID" algorithm is *unverified*.
- Takeaway: names are keyed by operation, and authors must declare the parts that are unstable.

**Revit API** (public reference via rvtdocs mirror).

- `Reference` is "a stable reference to a geometric object". `ConvertToStableRepresentation` and `ParseFromStableRepresentation` serialise it to and from a string ([Reference](https://rvtdocs.com/2025/Autodesk.Revit.DB.Reference)).
- Community reverse-engineering of that string reads it as `elementUniqueId:index:TYPE`, with a second `symbolUniqueId:geomIndex:SURFACE` triple for family instances. Indices 0-8 are reserved for the family's special reference planes ([Building Coder @9473d6a](https://github.com/jeremytammik/tbc/blob/9473d6aeb48076d3262fd9195835230318ca5054/a/1429_stable_references.html)). This reading is *unverified, inferred by experiment*.
- `GeometryObject.Id` is unique only "in its associated… GeometryElement" ([doc](https://rvtdocs.com/2025/Autodesk.Revit.DB.GeometryObject)).
- **The key BIM pattern is role-named references.** `FamilyInstanceReferenceType` names Left, CenterLeftRight, Right, Front, Back, Top, Bottom and similar, and documents them as stable: dimensions survive swapping the type or family. Generic Strong and Weak references are documented as *not* stable ([doc](https://rvtdocs.com/2025/Autodesk.Revit.DB.FamilyInstanceReferenceType)).
- Walls expose role queries for faces: `HostObjectUtils.GetSideFaces(exterior/interior)`, `GetTopFaces` and `GetBottomFaces` ([doc](https://rvtdocs.com/2025/Autodesk.Revit.DB.HostObjectUtils)).
- Takeaway: Revit scopes persistence to the element, and semantic roles carry most references.

**SolveSpace** (GPL, method only).

- Derived entities get handles from a remap table keyed by (input entity, copy-number or role constant), with roles such as TOP, BOTTOM and LINE_TO_FACE. New values are allocated sequentially, and the table is persisted with the group ([sketch.h L145-162](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/sketch.h#L145-L162), [L302-312](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/sketch.h#L302-L312), [group.cpp L864-876](https://github.com/solvespace/solvespace/blob/4707fa2b9dfd637787c19d91457d1707bc92bf44/src/group.cpp#L864-L876)).
- Result: deterministic given the same history, and simple.

## 6. Provenance through booleans and mesh operations

**OCCT `BRepTools_History`** ([L27-88](https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/src/ModelingData/TKBRep/BRepTools/BRepTools_History.hxx#L27-L88)) defines the cleanest contract available. Relations are Generated, Modified and Removed, kept for vertices, edges, faces and solids. Its invariants:

- a removed entity has no Modified outputs;
- Generated and Modified are disjoint for any input;
- every output is either an input or generated or modified from one;
- generated-versus-modified is chosen by rules on dimension and joining;
- two histories compose with defined rules.

**Manifold** (Apache-2.0).

- Each triangle carries a back-reference to its source: `meshID`, `originalID`, `faceID` and `triID` ([shared.h L355-371](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/src/shared.h#L355-L371)). Booleans copy the input triangle's reference onto output triangles ([boolean_result.cpp L519-527](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/src/boolean_result.cpp#L519-L527)).
- Output is grouped into runs keyed by `runOriginalID` and `runTransform`. User-supplied `faceID`s pass through, and simplification preserves edges between different `faceID`s ([mesh.h L72-89](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/include/manifold/mesh.h#L72-L89), [L125-153](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/include/manifold/mesh.h#L125-L153)).
- **Determinism hazard:** IDs come from a *process-global* `std::atomic` counter with relaxed ordering ([impl.cpp L71-75](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/src/impl.cpp#L71-L75); [manifold.cpp L441-473](https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/src/manifold.cpp#L441-L473)). Only `AsOriginal(id)` with caller-chosen IDs is deterministic.
- Provenance is face-level only. There are no edge or vertex names, and a split face simply shares its `faceID` across pieces.

**What a Spare Kit operation must record** (synthesis):

1. An origin for every output face, edge and vertex: one of Same, Modified, Generated(sources, role), Primitive(role), SplitOf(source, cutting tools, side) or MergeOf(sources).
2. The set of deleted inputs.
3. Operation-declared role keys.
4. Canonical ordering by name, never by pointer or hash-map order.
5. The `BRepTools_History` invariants, checked in tests.

## 7. Incremental engine and deterministic replay

**Salsa's tracked-struct identity is a naming scheme.** Identity is (current query, hash of identity fields, a disambiguator counted per hash within the query) ([tracked_structs.md L19-27](https://github.com/salsa-rs/salsa/blob/b9041f18828ee7dacbd2ee72b63506ecedc223d6/book/src/plumbing/tracked_structs.md?plain=1#L19-L27), [tracked_struct.rs L236-245](https://github.com/salsa-rs/salsa/blob/b9041f18828ee7dacbd2ee72b63506ecedc223d6/src/tracked_struct.rs#L236-L245), [L439-457](https://github.com/salsa-rs/salsa/blob/b9041f18828ee7dacbd2ee72b63506ecedc223d6/src/tracked_struct.rs#L439-L457)). However, `salsa::Id` is database-local (table index plus generation), so it must never be persisted.

**rust-analyzer's `AstId` is the same idea in production.**

- Layout: a 5-bit kind, a 16-bit hash of (name, parent), and an 11-bit occurrence index ([ast_id.rs L1-20](https://github.com/rust-lang/rust-analyzer/blob/6c1fc437b550edf709a3ca55a2a8f809f3d3fcea/crates/span/src/ast_id.rs#L1-L20), [L153-180](https://github.com/rust-lang/rust-analyzer/blob/6c1fc437b550edf709a3ca55a2a8f809f3d3fcea/crates/span/src/ast_id.rs#L153-L180), [L463-490](https://github.com/rust-lang/rust-analyzer/blob/6c1fc437b550edf709a3ca55a2a8f809f3d3fcea/crates/span/src/ast_id.rs#L463-L490)).
- The header warns that one unstable ID can invalidate whole-crate queries.
- The firewall rule is that editing a function body never invalidates global facts ([architecture.md L164-175](https://github.com/rust-lang/rust-analyzer/blob/6c1fc437b550edf709a3ca55a2a8f809f3d3fcea/docs/book/src/contributing/architecture.md?plain=1#L164-L175)). The BIM analogue: editing one door must not invalidate other elements' name tables.

**Other Salsa mechanisms that matter here.**

- *Durability.* LOW, MEDIUM, HIGH and NEVER_CHANGE let validation skip unchanged high-durability inputs ([durability.rs L3-20](https://github.com/salsa-rs/salsa/blob/b9041f18828ee7dacbd2ee72b63506ecedc223d6/src/durability.rs#L3-L20)). Map family and type definitions to HIGH and instance parameters to LOW.
- *Early cutoff (backdating).* If a recomputed value is equal to the old one, dependents are not rerun ([backdate.md](https://github.com/salsa-rs/salsa/blob/b9041f18828ee7dacbd2ee72b63506ecedc223d6/book/src/plumbing/terminology/backdate.md?plain=1#L3)). Stable names are what make cutoff work: if the name table and geometry hash are unchanged, downstream features are skipped.
- *Cancellation.* Salsa cancels by unwinding with `resume_unwind` and `catch_unwind` ([cancelled.rs L12-43](https://github.com/salsa-rs/salsa/blob/b9041f18828ee7dacbd2ee72b63506ecedc223d6/src/cancelled.rs#L12-L43); [architecture.md L377-388](https://github.com/rust-lang/rust-analyzer/blob/6c1fc437b550edf709a3ca55a2a8f809f3d3fcea/docs/book/src/contributing/architecture.md?plain=1#L377-L388)). wasm32 builds commonly use panic=abort, where unwinding is not available (*verify against current toolchain*). Kernel operations should therefore poll a cancellation token and return `Err(Cancelled)` cooperatively. A cancelled operation must never publish partial names.

**Replay rule.** A name must be a pure function of the operation log's contents. Every source of drift found above is barred:

- a random suffix (FreeCAD);
- a random per-document offset (FreeCAD);
- sequential per-document intern IDs (FreeCAD);
- a global atomic counter (Manifold);
- kernel report order (FreeCAD and OCCT).

Operation IDs are assigned *when the operation is authored* and stored in the log, as (replica, sequence) or a content hash, so every machine derives identical names.

## 8. Recommendation for Spare Kit

**Three identity layers.**

1. `OpId`: globally unique and stored in the log. An `OpPath` is an OpId plus sub-step labels, following Onshape's hierarchy rule.
2. `NameId`: a persistent, content-addressed name. It is a 128-bit hash of a canonical, little-endian encoding of a `TopoName`. The hash function is fixed and versioned (`NameScheme v1`) and written into the file. Names are hash-consed in an arena, so a name is a DAG node that refers to its parents by `NameId`. That gives FreeCAD-style sharing without a per-document counter.
3. `EntityId(u32)`: an index into the current B-rep arena, used within a session only and never persisted.

```rust
enum TopoName {
  Created { op: OpPath, role: Role, from: SortedVec<NameId> },        // primitive / generated
  Split   { op: OpPath, of: NameId, key: PieceKey },
  Merged  { op: OpPath, of: SortedSet<NameId> },
  Bounded { kind: Edge|Vertex, by: SortedSet<NameId>, ord: u8 },      // edge = its faces, vertex = its edges/faces
}
enum PieceKey { ByTools { tools: SortedSet<NameId>, side: Side },     // half-space (Bidarra-style)
                Ordinal { n: u16, of: u16 } }                          // weak: flagged
```

**Mapping rules, applied per operation.**

- Unchanged entities keep their name.
- A **1:1 Modified entity keeps its `NameId`**: identity is preserved (Onshape's robust-query semantics). The operation trail is kept in the history, not in the name.
- Generated entities get `Created{op, role, from}`. Roles are part of each operation's contract, for example: extrude `cap.start`, `cap.end`, `side(src)`; wall `ext`, `int`, `top`, `base`, `end0`, `end1`; fillet `roll(edge)`.
- A 1:n split gets `Split` with the cutting-tool names and side. `Ordinal` is used only when tools and side still collide, ordered in the parent surface's own parameter space with bit-identical arithmetic, and is marked weak.
- An n:1 merge gets `Merged`.
- Anything left unnamed takes `Bounded` names, filled from faces downward and then from vertices upward.
- A duplicate name is an internal error under test. In production it gets a weak ordinal; there is never a random suffix.

**History every operation must emit.** `OpHistory { out: Vec<(EntityId, Origin)>, deleted: Vec<NameId> }`, with the item 6 origin variants and the `BRepTools_History` invariants enforced by property tests. Outputs are sorted by `NameId`. The history of a mesh or boolean backend is lifted from per-triangle references in the style of Manifold, using caller-assigned deterministic IDs only.

**References and resolution.**

```rust
struct TopoRef { element: ElementId, name: NameId, kind: EntityKind,
                 want: One | AllPieces | Carrier, sig: GeomSig /* diagnostics only */ }
enum RefError { Deleted{by: OpPath}, Ambiguous{candidates}, WeakChanged{..}, ElementGone }
```

1. Look for an exact name in the target element's table.
2. Otherwise, walk *forward* through recorded Split, Merged and Modified links (as TNaming's current-shape lookup and FreeCAD's related-element lookup do).
   - One descendant: Ok, with a "followed" note.
   - Several: Ok if `want == AllPieces`, otherwise `Ambiguous`.
   - None: `Deleted`.
3. `Carrier` references, which are the default for dimensions and hosting, resolve to the underlying plane, axis or surface (OCCT's `Geometry` flag; Revit's role planes). Pieces of a split wall face share one carrier, so a door cut does not break the dimension.
4. **No automatic geometric fallback.** `GeomSig` is used only to *suggest* repairs in the UI. An accepted repair is a new operation in the log, so all replicas apply the same fix.
5. Errors are deterministic model state: the feature shows `UnresolvedRef` on every machine.

**Engine wiring.**

- Each operation is a memoised query returning (BRep, NameTable, OpHistory).
- Early cutoff compares (geometry hash, NameTable hash).
- Resolution is its own query that depends only on the target element's name table.
- Persisted data uses `NameId` and never Salsa IDs.
- Unreferenced history is pruned at save, keeping the names reachable from live entities and references (FreeCAD marks used strings in a similar way: [ElementMap.cpp L77-106](https://github.com/FreeCAD/FreeCAD/blob/9946038aace97af288eb28f2727a169f9d1158c7/src/App/ElementMap.cpp#L77-L106)).

---

## Adopt / Avoid / Open questions

**Adopt**

- Operation-keyed, content-hashed, hash-consed names.
- 1:1 modification preserves identity.
- The `BRepTools_History` contract as a tested invariant.
- Role keys as part of each operation's API (Revit role references, SolveSpace remap roles).
- Carrier-level references for dimensions.
- Resolution that fails loudly, returning typed errors.
- An `AllPieces` policy.
- Operation IDs fixed at authoring time.
- A rust-analyzer-style firewall per BIM element.
- Cooperative cancellation.

**Avoid**

- Traversal-index names.
- Kernel report order as identity.
- Random or global-counter IDs (FreeCAD duplicate suffix and document offset; Manifold `meshIDCounter_`).
- Per-document sequential interning inside persisted names.
- Silent geometric "rescue" (FreeCAD `rescueDriftedElement`).
- Persisting `salsa::Id`.
- Unwinding-based cancellation on wasm.
- CONSTSHAPE-style index fallbacks.

**Open questions**

1. Which `PieceKey` should apply when a tool face cuts one face into two disconnected pieces on the same side? Is a parameter-space ordinal acceptable? (Needs a test corpus.)
2. Should merges keep the *set* name, or adopt one member's name for BIM continuity?
3. Which hash to use (BLAKE3-128 or SipHash-2-4 with a fixed key), and what is the migration story when `NameScheme` changes?
4. What are the name-table memory budgets at building scale? (FreeCAD reports roughly +30% recompute and +27% file size.)
5. How should concurrent edits that make an existing reference ambiguous be surfaced in the operation-log merge UI?
6. Does wasm32 unwinding become viable enough to reuse Salsa cancellation unchanged? (*unverified*)
7. Should edges and vertices be stored as `Bounded` names, or derived on demand?
