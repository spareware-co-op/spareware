# A13: Clash: broad and narrow phase, true-distance clearance

## The question

Spare Construct's map wants clash detection to run live inside the model, test every discipline against every other, and measure clearance as a true minimum distance, "not an inflated box, which gives false hits" ([ARCHITECTURE.md](../ARCHITECTURE.md), Clash detection). The piece asks for a recommended method, and for how clearance avoids false hits.

Since the piece was written, the Spare Kit geometry kernel map (G01, [issue #169](https://github.com/spareware-co-op/spareware/issues/169), [PR #176](https://github.com/spareware-co-op/spareware/pull/176), not merged yet) has taken on the geometric half. Its section [19.11](../../engines/geometry/architecture/19-meshes-spatial-clash.md#1911-clash) settles the broad phase, the narrow phase and true-distance clearance. It leaves A13 the product half: "rules, ignore conditions, grouping into issues, BCF exchange, standard New Zealand clearances, whether live clash is on by default, how `NearLimit` is shown, and a benchmark of this method on a real model". It also says that if A13's benchmark shows the method is wrong, the section changes.

So this note does three things:

1. It checks G01's geometric method against the source code of the open tools that already do clash.
2. It measures the broad phase on a real model.
3. It recommends the product half.

Numbers such as 19.11 are chapters of G01's map, linked at the paths they will have once PR #176 merges. Until then, read them on PR #176.

Sources were read on 22 October 2026: IfcOpenShell at `f44ef5a` (LGPL-3.0, method only), Manifold 3.5.4 at `ce50d78` (Apache-2.0), parry 0.31.1 at `3383f51` (Apache-2.0) and buildingSMART's BCF-XML 3.0 at `bc48611` (CC BY-ND 4.0). "Unverified" marks anything not confirmed in source code or on an official page. No Rust toolchain was available, so the benchmark counts work (pairs and box tests), not time.

## Options

| | A. Triangles in floating point (IfcOpenShell's IfcClash) | B. Overlap first, then the shortest gap (Manifold's `MinGap`) | C. Exact contact classes (G01, 19.10 and 19.11) |
| --- | --- | --- | --- |
| Broad phase | Box tree over elements, then an oriented-box test per pair | The other mesh's face boxes, grown by the search length, queried against this mesh's tree | Integer boxes in a two-level tree (instances, then each definition), grown by the clearance |
| Narrow phase | Triangle against triangle in `f64` world coordinates | A boolean intersection, then triangle-to-triangle distance in floating point | Exact predicates on grid meshes; closed forms for planes, cylinders and spheres; certified tessellations for the rest |
| Touch against overlap | A tolerance (2 mm by default) or an `allow_touching` switch | Not separated: touching counts as a gap of 0 | Decided exactly, with no tolerance |
| Clearance | Smallest triangle distance below c | Smallest gap, capped at the search length | Exact decision, or a proven interval |
| Licence | LGPL-3.0: method only | Apache-2.0 | Our own (MPL-2.0) |

## Evidence

### How IfcOpenShell does it

IfcClash is the only open clash tool found that works on whole building models. Its Python layer, [ifcclash.py](https://github.com/IfcOpenShell/IfcOpenShell/blob/f44ef5a74df94a3d81d1b9e5c135240785a335bc/src/ifcclash/ifcclash/ifcclash.py#L60-L73), defines a clash set as group A against group B (or A against itself) in one of three modes:

- **intersection**, with a tolerance;
- **collision**, with `allow_touching`;
- **clearance**, with a distance.

The groups are chosen by including or excluding elements through IfcOpenShell's selector queries ([L172-L215](https://github.com/IfcOpenShell/IfcOpenShell/blob/f44ef5a74df94a3d81d1b9e5c135240785a335bc/src/ifcclash/ifcclash/ifcclash.py#L172-L215)). There are no rules about relationships (same system, host, assembly). Selection is the only way to leave a pair out.

The geometry lives in C++, in [bvh_tree.h](https://github.com/IfcOpenShell/IfcOpenShell/blob/f44ef5a74df94a3d81d1b9e5c135240785a335bc/src/ifcgeom/kernels/opencascade/bvh_tree.h). Every element is triangulated in world coordinates ([L93-L96](https://github.com/IfcOpenShell/IfcOpenShell/blob/f44ef5a74df94a3d81d1b9e5c135240785a335bc/src/ifcgeom/kernels/opencascade/bvh_tree.h#L93-L96)), and each pair goes through a run of floating-point shortcuts:

- **Oriented boxes shrunk by the tolerance.** A pair is dropped when one oriented box, shrunk by the tolerance, misses the other ([L166-L171](https://github.com/IfcOpenShell/IfcOpenShell/blob/f44ef5a74df94a3d81d1b9e5c135240785a335bc/src/ifcgeom/kernels/opencascade/bvh_tree.h#L166-L171)).
- **Near-coplanar triangles skipped.** In collision mode, triangle pairs whose normals are within about 0.25° are skipped ([L950-L953](https://github.com/IfcOpenShell/IfcOpenShell/blob/f44ef5a74df94a3d81d1b9e5c135240785a335bc/src/ifcgeom/kernels/opencascade/bvh_tree.h#L950-L953)). Two faces lying on one another are never tested.
- **Depth is guessed, then capped.** Intersection depth is a protrusion distance or a piercing length, capped at twice the smallest half-size of the element's oriented box "for intuitive results" ([L816-L819](https://github.com/IfcOpenShell/IfcOpenShell/blob/f44ef5a74df94a3d81d1b9e5c135240785a335bc/src/ifcgeom/kernels/opencascade/bvh_tree.h#L816-L819), [L1333](https://github.com/IfcOpenShell/IfcOpenShell/blob/f44ef5a74df94a3d81d1b9e5c135240785a335bc/src/ifcgeom/kernels/opencascade/bvh_tree.h#L1333)).
- **Clearance stops early.** Clearance takes the smallest triangle-to-triangle distance below c. It stops at the first hit unless `check_all` is set, and always stops once a distance falls under 0.1 mm ([L1086-L1092](https://github.com/IfcOpenShell/IfcOpenShell/blob/f44ef5a74df94a3d81d1b9e5c135240785a335bc/src/ifcgeom/kernels/opencascade/bvh_tree.h#L1086-L1092)).
- **Defaults.** Intersection tolerance 2 mm, clearance 50 mm ([tree.h L91-L93](https://github.com/IfcOpenShell/IfcOpenShell/blob/f44ef5a74df94a3d81d1b9e5c135240785a335bc/src/ifcgeom/tree.h#L91-L93)).
- **Results in thread order.** Results are collected from threads behind a mutex ([L225-L229](https://github.com/IfcOpenShell/IfcOpenShell/blob/f44ef5a74df94a3d81d1b9e5c135240785a335bc/src/ifcgeom/kernels/opencascade/bvh_tree.h#L225-L229)), so their order changes from run to run.

Each shortcut is reasonable in floating point, and each one can drop or invent a hit near a boundary: the shrunk box, the coplanar skip, the cap, the early stop. G01's exact tests (19.10) replace every one of them with a decision. The 2 mm tolerance exists mostly to stop designed contacts being reported as clashes. Under G01 those pairs are an exact `Touch`, so they need no tolerance at all.

**Grouping.** IfcClash groups hits with scikit-learn's OPTICS clustering on the midpoint of each hit, 3 m apart by default ([L260-L315](https://github.com/IfcOpenShell/IfcOpenShell/blob/f44ef5a74df94a3d81d1b9e5c135240785a335bc/src/ifcclash/ifcclash/ifcclash.py#L260-L315)).

**BCF export.** One BCF topic per hit, titled with both elements' classes and names. The viewpoint is aimed at one hit point and selects both elements ([L225-L243](https://github.com/IfcOpenShell/IfcOpenShell/blob/f44ef5a74df94a3d81d1b9e5c135240785a335bc/src/ifcclash/ifcclash/ifcclash.py#L225-L243)). A fresh run makes fresh topics, so a consultant's comments can't follow an issue from one run to the next.

### How Manifold does it

[`Manifold::MinGap`](https://github.com/elalish/manifold/blob/ce50d78021d64507f89e8c9fc2c2e51018117857/src/manifold.cpp#L1120-L1126) first runs a boolean intersection and returns 0 if anything is left. Only then does it measure the gap ([properties.cpp L442-L461](https://github.com/elalish/manifold/blob/ce50d78021d64507f89e8c9fc2c2e51018117857/src/properties.cpp#L442-L461)): it grows the other mesh's face boxes by the search length, queries them against this mesh's [collider](https://github.com/elalish/manifold/blob/ce50d78021d64507f89e8c9fc2c2e51018117857/src/collider.h#L263-L290) (a radix tree over Morton codes, with boxes refitted in place), and keeps the smallest triangle-to-triangle distance. That triangle distance is taken from PhysX, under BSD-3 ([tri_dist.h L24-L26](https://github.com/elalish/manifold/blob/ce50d78021d64507f89e8c9fc2c2e51018117857/src/tri_dist.h#L24-L26)).

This confirms the order G01 uses: overlap first, then distance among boxes grown by c. It also shows the gap G01 closes. Manifold answers a number in floating point, and touching gives the same 0 as a deep overlap, so it can't tell a duct resting on a beam from a duct through it.

### How parry keeps a live tree

parry's `Bvh` can update one leaf and report whether anything changed. If the leaf's stored ("fattened") box still contains the new box, the tree is left untouched ([bvh_insert.rs L10-L21](https://github.com/dimforge/parry/blob/3383f51cbbe9af70565427e7a66c605e1557c1fc/src/partitioning/bvh/bvh_insert.rs#L10-L21)). Its pair traversal can then visit only the pairs where at least one leaf changed ([bvh_traverse_bvtt.rs L15-L20](https://github.com/dimforge/parry/blob/3383f51cbbe9af70565427e7a66c605e1557c1fc/src/partitioning/bvh/bvh_traverse_bvtt.rs#L15-L20)). Its parallel traversal returns pairs "in the exact same order" as the sequential one ([L207-L214](https://github.com/dimforge/parry/blob/3383f51cbbe9af70565427e7a66c605e1557c1fc/src/partitioning/bvh/bvh_traverse_bvtt.rs#L207-L214)). Tree quality is restored a little at a time, about 5% of leaves per call ([bvh_optimize.rs L45-L84](https://github.com/dimforge/parry/blob/3383f51cbbe9af70565427e7a66c605e1557c1fc/src/partitioning/bvh/bvh_optimize.rs#L45-L84)).

This is the live half of 19.9 and 19.11 ("an edit refits the moved instance's box, queries its neighbours and re-tests only pairs involving it"). One difference is worth keeping. Fattened boxes suit physics, where everything moves every frame. A model changes in discrete edits, so exact integer boxes (19.9) cost nothing extra and keep the candidate list tight.

### The broad phase on a real model

The contributor ran this on an architecture-and-structure Revit model of theirs. The element boxes were read through the Revit connection and counted by [broadphase_count.py](A13-bench/broadphase_count.py), which is in this folder so anyone can rerun it. No model data is in the repo, and the note reports totals only.

The model has 1,459 elements with boxes: 479 walls, 336 beams, 185 generic models, 139 columns, 130 floors, 54 doors, 46 windows, 36 ceilings, 20 roofs, 15 foundations, 13 stairs and 6 pipes. That is 963 architecture, 490 structure and 6 services. The longest side of an element's box has a median of 2.78 m and a 95th percentile of 10.89 m. Two elements are site-scale: a site slab and a site generic model, both over 200 m across. 37 boxes are points, lines or sheets with no thickness. One requested element had no box at all.

The Revit connection reports boxes in feet, rounded to 0.01 ft (about 3 mm), so every box was grown outward by half a step before counting. The tree is a plain median split with leaves of up to four, standing in for G01's binned surface-area tree, so its box-test counts are an upper bound.

| Clearance c | Candidate pairs, whole model | Structure × architecture | Door or window × wall | Boxes only touching | Pairs without the 2 site-scale elements | Neighbours per edit: median | p95 | max | max without site-scale | Box tests per edit |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 16,070 | 5,180 | 667 | 3,544 | 14,701 | 16 | 56 | 1,178 | 336 | 211 |
| 25 mm | 17,007 | 5,627 | 692 | | 15,621 | 17 | 61 | 1,179 | 336 | 215 |
| 50 mm | 18,008 | 6,015 | 725 | | 16,600 | 18 | 63 | 1,201 | 347 | 219 |
| 100 mm | 19,716 | 6,931 | 777 | | 18,300 | 19 | 69 | 1,201 | 349 | 226 |
| 300 mm | 28,641 | 9,982 | 1,331 | | 27,122 | 29 | 94 | 1,235 | 403 | 266 |
| 600 mm | 35,346 | 12,526 | 1,936 | | 33,756 | 39 | 111 | 1,260 | 445 | 293 |

*"Boxes only touching" counts pairs whose boxes overlap by no more than two rounding steps on some axis, measured at c = 0 only.*

What the numbers say:

- **A local edit is small.** Moving a typical element brings 16 candidates, and 56 at the 95th percentile, found with about 200 box tests out of 1,459 elements. G01's method fits a live edit as long as the narrow phase stays under roughly 0.3 ms a pair at the 95th percentile. That figure is *unverified* until the Rust build measures it.
- **Slabs and roofs drive the worst ordinary edits.** The elements with most neighbours, other than the site generic model, are two sloped roofs (337 and 325) and floor slabs (182 to 188). A flat box around a slab encloses everything standing on it. Most of those pairs are designed contacts, which G01 settles as an exact `Touch`, so the fast path matters more than the slow one.
- **One site-scale element touches everything.** The site generic model is a candidate for 1,178 of the 1,459 elements. Editing it is a global edit, not a local one.
- **About one pair in five is a designed contact.** 3,544 of the 16,070 pairs at c = 0 only touch within the rounding. A floating-point tool needs a tolerance to stay quiet about them; G01 needs none.
- **Choosing sets removes most of the work.** A structure-against-architecture test reads 5,180 pairs, a third of the whole model.
- **Hosted elements are a small share.** A rule leaving out doors and windows against walls removes at most 667 pairs (4%). That count includes every wall the opening sits beside, because this model draws finish layers as separate thin walls (74 of 479 walls are under 100 mm thick).
- **Clearance multiplies work slowly.** Going from 0 to 100 mm adds 23% more pairs; 600 mm, the size of a maintenance zone, more than doubles them.

At this density (22 neighbours per element at c = 0), the 230,000-element reference model in the map's performance targets would have about 2.5 million candidate pairs at c = 0. That is an extrapolation, *unverified*. An MEP-heavy model will be denser: this one has only 6 pipes.

### The BCF format

BCF 3.0's topic has a GUID, a type, a status, labels, a priority, an assignee, a stage, a due date, comments and viewpoints ([markup.xsd L52-L119](https://github.com/buildingSMART/BCF-XML/blob/bc48611d0d7a1587f028a2b69677a1aafd5cd0a8/Schemas/markup.xsd#L52-L119)). A viewpoint's components select, hide or colour elements by IFC GUID, with an optional authoring tool ID ([visinfo.xsd L109-L166](https://github.com/buildingSMART/BCF-XML/blob/bc48611d0d7a1587f028a2b69677a1aafd5cd0a8/Schemas/visinfo.xsd#L109-L166)). BCF has no field for a clash pair, a distance or a rule. Those have to travel in the description, labels or a snippet. The schema repository is CC BY-ND 4.0 ([LICENSE](https://github.com/buildingSMART/BCF-XML/blob/bc48611d0d7a1587f028a2b69677a1aafd5cd0a8/LICENSE)), so we can implement the format and ship the schemas unchanged, but not modified copies of them (*unverified* legal reading; see risks).

## Recommendation

**Adopt G01's method (19.9 to 19.11) for the geometry, unchanged.** The benchmark does not show it is wrong. The open tools confirm its order (boxes grown by c, overlap first, then distance). Every heuristic in the floating-point tools near a boundary (the tolerance, the coplanar skip, the capped depth, the early stop) is a place where G01 gives an exact or proven answer instead.

**How clearance avoids false hits:** a box is only ever used to find candidates, never to report a clash. A candidate is reported only once its `Contact` is `WithinClearance` (proven 0 < gap < c), `Overlap` or `SurfacesCross`. A box grown by c can only add candidates; it can't add hits. And because boxes are rounded outward, no real pair is dropped.

On top of that, Spare Construct builds the product half as follows.

### 1. Tests

A test is data: a name, set A, set B (or "within A"), a type and its values, and the rules that apply. This is IfcClash's clash set with relationship rules added.

- **Sets** are queries over the model: discipline, category, level, consultant model, system, or any parameter. They are stored as queries, not lists, so new elements join the set.
- **Types:** *hard*, with a depth tolerance t that defaults to 0, and *clearance*, with a gap c. Depth is computed only when t > 0, as 19.10 already says.
- A project starts with a short default list: structure × architecture, structure × services, services × services and services × architecture, all hard with t = 0.

### 2. Rules (ignore conditions)

Rules are predicates over the entity store's relationship tables ([A02 note](A02-entity-store.md), in review as [PR #167](https://github.com/spareware-co-op/spareware/pull/167)). They run before any geometry is read (19.11, step 1). Each rule is named and versioned, and each removed pair can say which rule removed it, so the AI and people can audit a quiet result.

| Rule | Leaves out |
| --- | --- |
| Hosted | An opening element against its host (a door in its wall) |
| Same assembly or group | Parts of one assembly or group |
| Same system | Elements connected in one MEP system (a pipe and its own fitting) |
| Covering | Insulation or lining against the element it covers |
| Joined | Elements in a join relationship (wall-to-wall, beam-to-column cut-back) |
| Accepted pair | One pair a person accepted, with a reason, keyed by both elements' geometry content, so it comes back if either one changes |

Pairs that only `Touch` are never clashes and need no rule. A test can still ask for them, for example "beams must bear on columns".

### 3. How each result is shown

| G01 `Contact` | Status | Counts as a clash? | Shown as |
| --- | --- | --- | --- |
| `Overlap`, depth ≥ t | Clash | Yes | The overlap volume when the kernel gives one, otherwise the witness point, with depth |
| `Overlap`, depth < t | Below tolerance | No (listed under the test) | Muted |
| `SurfacesCross` (a raw mesh, inside unknown) | Clash, unproven inside | Yes | Witness point, marked "mesh: inside unknown" |
| `WithinClearance` | Clearance | Yes, in clearance tests | Gap interval and the two closest points |
| `Touch` | Contact | No | Only when a test asks for it |
| `NearLimit` | Check | Its own count, never hidden, never merged into clashes | The interval, the threshold it straddles, and the kernel's own accuracy figure, for example "cannot decide below 5 µm" |
| `Clear` | | No | Nothing |
| Budget error for a pair | Not tested | Its own count | Listed, with a button to run it in the background |

`NearLimit` gets its own status because it means "the geometry itself can't say". That's a different message from "clash" or "fine", and an AI reading the results must see the difference too.

### 4. Grouping into issues

Issue identity must survive edits, because an issue carries comments, an owner and history. Clustering by position (IfcClash's OPTICS) regroups when anything moves, so issues would split and merge under people's comments. Instead:

- **Groups come from stable keys**, chosen per test: by element (all hits of one duct), by level and zone, by system, or by element type. The default is by element: the element from the lower-priority set, the one that usually has to move (services before structure).
- **An issue is keyed by its test and group key**, so it stays the same issue as its hits change. It closes itself when its group is empty, as the map asks, and reopens if hits come back.
- **Ordering** within and across groups uses persistent keys only, as G01 does for pair results, so every machine shows the same list.
- **Spatial clustering** remains available as a view ("show hits near each other"), never as issue identity.

### 5. BCF exchange

- **One BCF topic per issue, not per hit.** The topic GUID is a UUID version 5 of the project and the issue key, so exporting again updates the same topic, and a consultant's comments stay attached.
- **Components** select every element in the group by IFC GUID, with our permanent ID as the authoring tool ID. The viewpoint looks at the group's witness points from outside the group's box.
- **What BCF has no field for** (test, rule, status, gap or depth interval) goes into the labels and the description. A snippet holds the issue as JSON with a published schema of our own, so another Spare Construct reads it back exactly.
- **Importing** matches topics by GUID first, then by the IFC GUIDs of their components. A topic made by another tool becomes an ordinary issue.

### 6. Live clash: on by default

The counts above support it. Make three changes to how the map words it:

- **Separate the edit's frame from the clash pass.** The 16 ms frame is for the edit's own regeneration. Clash runs straight after as a cancellable job on the shared pool, and its warnings land a few frames later. The map's line "A clash shows as a warning within the edit's own time budget" would become "within 100 ms of a local edit". 100 ms is a proposal, to be checked by the Build-stage benchmark below. Changing the map needs this note accepted by an owner.
- **Treat site-scale edits as global edits.** Editing an element that has more than a set number of candidates (proposed: 1,000) is handled like a global edit in regeneration: it shows progress and can be cancelled. Small edits near a big element stay cheap, because 19.12's two-level tree reads only the big element's faces near the query.
- **Live tests are the project's own tests.** Consultants' linked models join live only when a person says so; otherwise they wait for batch runs.

### 7. Standard New Zealand clearances

Don't ship clearance values taken from standards. Standards New Zealand says standards content "cannot be copied or used in AI, software, or digital tools without a licence", and it names tables and formulas ([Standards NZ, 27 February 2026](https://www.standards.govt.nz/news-and-updates/using-standards-content-in-digital-and-ai-tools-what-you-need-to-know)). Joint AS/NZS standards, such as the plumbing and wiring rules, aren't among the building standards MBIE sponsors for free reading ([sponsored building standards](https://www.standards.govt.nz/get-standards/sponsored-standards/building-related-standards)), and free reading isn't a licence to embed them anyway.

Can we still help people? Yes, in three ways:

- Ship clearance tests as data with empty values. Each one carries a reference to the clause it comes from (number and title only), for a firm to fill in from its own copy.
- Let firms share filled-in test sets between their own projects.
- Ask an owner whether the co-op wants a Standards NZ digital licence. Whether a bare number from a standard is protected is a legal question this note doesn't answer.

Maintenance space around plant usually comes from the manufacturer's data, so equipment families can carry their own clearance zones as geometry. Then a plain hard test finds them.

### 8. Benchmark for the Build stage

The counts above settle the broad phase. When the Rust code exists, the benchmark that blocks merges should add:

- the narrow-phase time per pair, by `Contact` kind and by pair kind (flat, analytic, certified);
- the time from edit to warning, at p50 and p95, for an ordinary edit, a slab or roof edit, and a site-scale edit;
- batch throughput in pairs per second per core, and the memory of the pair cache;
- at least one MEP-heavy model, since this sample has 6 pipes.

`broadphase_count.py` stays as the quick check anyone can run on boxes from any tool.

## Open risks

1. **Services are missing from the sample.** The model has 6 pipes and no ducts, so the density in plant rooms and ceiling voids, where clash matters most, is unmeasured.
2. **The counts lean high.** Boxes came rounded to about 3 mm and were grown outward. Revit's boxes may also be loose around rotated families (*unverified*). Exact integer boxes would give fewer candidates.
3. **Time is unmeasured.** No Rust toolchain was available. The 0.3 ms-per-pair figure and the 100 ms target are proposals, not measurements.
4. **Patents.** G01 section [29.17](../../engines/geometry/architecture/29-sources-licences.md#2917-patents-to-avoid) lists clash, clearance and continuous collision detection as families no patent search has covered. The clash crates wait for its "Patent register" piece, and so does any product feature that needs them.
5. **Standards licensing.** Point 7 needs an owner's decision before any clearance values are shipped.
6. **BCF licence.** Implementing a format published under CC BY-ND is believed to be fine, as long as the schemas are only shipped unchanged (*unverified*). An owner should confirm this.
7. **G01 isn't merged.** If 19.9 to 19.11 change in review, this note follows them.
8. **Raw consultant meshes.** A mesh that is not closed can be `SurfacesCross` but never `Overlap` (19.11). Consultant IFC models need G01's healing (19.7) before their hits can be proven overlaps. Until then they show as "mesh: inside unknown".

## Sources

| Source | Licence | Used for |
| --- | --- | --- |
| [IfcOpenShell](https://github.com/IfcOpenShell/IfcOpenShell/tree/f44ef5a74df94a3d81d1b9e5c135240785a335bc) `ifcclash.py`, `tree.h`, `bvh_tree.h` | LGPL-3.0 | Method only: clash sets, modes, grouping, BCF export, and the floating-point shortcuts |
| [Manifold](https://github.com/elalish/manifold/tree/ce50d78021d64507f89e8c9fc2c2e51018117857) 3.5.4: `MinGap`, collider, `tri_dist.h` | Apache-2.0 (triangle distance from PhysX, BSD-3) | Overlap-first order; box query grown by the search length |
| [parry](https://github.com/dimforge/parry/tree/3383f51cbbe9af70565427e7a66c605e1557c1fc) 0.31.1: `bvh_insert.rs`, `bvh_traverse_bvtt.rs`, `bvh_optimize.rs` | Apache-2.0 | Change-detecting live tree; pair order that doesn't depend on threads |
| [BCF-XML 3.0](https://github.com/buildingSMART/BCF-XML/tree/bc48611d0d7a1587f028a2b69677a1aafd5cd0a8) `markup.xsd`, `visinfo.xsd` | CC BY-ND 4.0 | Topic and viewpoint fields |
| [Standards New Zealand: using standards content in digital and AI tools](https://www.standards.govt.nz/news-and-updates/using-standards-content-in-digital-and-ai-tools-what-you-need-to-know) | Web page | Licence needed for standards content in software |
| [Standards New Zealand: sponsored building standards](https://www.standards.govt.nz/get-standards/sponsored-standards/building-related-standards) | Web page | Which standards MBIE sponsors for free reading |
| G01 kernel map, sections [19.9](../../engines/geometry/architecture/19-meshes-spatial-clash.md#199-the-bounding-volume-hierarchy) to [19.12](../../engines/geometry/architecture/19-meshes-spatial-clash.md#1912-instancing-for-huge-assemblies) and [29.17](../../engines/geometry/architecture/29-sources-licences.md#2917-patents-to-avoid) ([PR #176](https://github.com/spareware-co-op/spareware/pull/176)) | Ours | The geometric method checked here |
| A real Revit model, element boxes only, read through the Revit connection | The contributor's; not in the repo | The benchmark totals |
