# 8. Topology: how shapes are stored

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Evidence: [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md) sections 1, 5, 8 and 9; [03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md) section 5; [07](../../../kit/notes/G01-evidence/07-persistent-naming.md).

## 8.1 Entities

The kernel uses the boundary-representation structure that Parasolid documents publicly, with explicit fins and regions. It is richer than OpenCascade's, which has no fin entity and keys curves on faces by pointer.

| Entity | What it is |
| --- | --- |
| Body | One shape. Its kind is Solid, Sheet (open surfaces), Wire (curves), Acorn (points) or General (mixed and non-manifold) |
| Region | A connected volume of a body, solid or void. Every body has one infinite void region outside it. Regions make cellular models: rooms and spaces are regions bounded by shared faces |
| Shell | A connected set of faces bounding a region |
| Face | A bounded piece of one surface, with a sense (which side is material) |
| Loop | A closed boundary of a face: one outer loop and any number of holes |
| Fin | The use of an edge by a loop, in one direction. In non-manifold models fins around an edge are kept in radial order |
| Edge | A bounded piece of one curve, between two vertices (or closed) |
| Vertex | A point |

Geometry attaches by index into the body's geometry pools: a face to a surface, an edge to a 3D curve, and a fin on a curved face to a curve in that face's parameter space (a pcurve). Each edge has one primary geometry: its 3D curve, exact or certified. Pcurves are derived and checked against it. An imported Tolerant edge has its pcurves as primary and its 3D curve derived, as Parasolid and ACIS do. The two are never kept in loose agreement by a growing tolerance.

## 8.2 Storage

- **Arenas with typed indices.** Each body stores each entity kind in a vector, addressed by a 32-bit typed index (`FaceIx`, `EdgeIx` and so on), assigned in persistent-name order (next point). No pointers, no reference counting between entities, no identity by memory address. Truck, Fornjot and OpenCascade all used address identity and all lost determinism through it; Fornjot ended on exactly this arena design ([03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md), section 2).
- **Indices follow names, in every operation.** The owner accepted this amendment (owner action G76, 32.17). The decision note says indices are "assigned in operation order", which leaves the order to how each operation happens to run. Instead, the builder assigns indices in persistent-name order (section 20.2), as the boolean rebuild already does (section 11.3). Fins, loops and shells have no names of their own, so they follow a fixed rule derived from the names of the faces, edges and vertices they join. A body's arenas then depend only on what it contains, never on how it was built. A full re-cut, a local re-cut (section 23.5) and a re-cut helped by an arrangement cache (section 11.6) give the same indices, on any number of threads. A parallel builder may fill scratch arrays in input order with section 23.4's `count_then_fill`, but it sorts them by name before the body is published.
- **So index order is name order.** A tie broken by index (section 23.7) is broken by name. Tables written in arena order (section 24.3) are in the name order that section 23.8's content hash asks for, which settles chapter 24, open question 4. Every operation's tests check that its output does not change when its inputs' arenas are shuffled (32.17). The cost: names are hashes, so entities near each other in space are not near each other in memory, and adding or removing an entity shifts every index after it. With chunks cut at fixed row counts (section 24.2), such a version shares few chunks with the one before. An edit that only changes entities in place shifts nothing. Section 23.9's benchmarks measure the cost. If it matters, the remedy is another canonical key, such as a spatial cell before the name, as section 17.3 chunks terrain; never a return to build order.
- **Immutable after building.** A body is made through a builder and is never changed afterwards. An edit makes a new version. Large bodies are split into shared chunks so a new version copies only what changed. Undo, multi-user replay and "try and see" come free; OpenCascade's optional safe mode becomes the only mode.
- **Ordered containers only.** Maps that can be iterated are ordered by stable keys. Hash maps are used only where they are never iterated.
- **Indices are not names.** An index is valid inside one body version. Persistent names (section 20) are what references and files hold.

## 8.3 The validity checker

Every operation's result passes the checker before it is returned. The checker runs in groups, like Parasolid's published body check:

1. **Combinatorial:** every loop closes, every fin has a partner or is a recorded boundary, shells are closed and consistently oriented, regions nest correctly, and Euler's formula holds.
2. **Geometric consistency:** vertices lie on their edges' curves and their faces' surfaces, edges lie on their faces, and pcurves match their 3D curves. These are certified with bounds, not sampled. OpenCascade samples 23 points, and its own documentation shows edges that pass and are still invalid.
3. **Self-intersection:** no face crosses itself; no two faces cross except along shared edges.
4. **Exactness record:** every body's exactness class and bounds are consistent with its geometry.

The cheap groups run after every operation in every build. The full set runs in debug builds, in continuous integration, on every import, and on request.

**As built (G84).** `geom-check`'s `check(body, exactness, level, meter)` runs the groups a `Level` asks for, charging the caller's meter for every row it visits, and returns a `Report`: each fault a `FaultKind` at an entity, in a fixed order, the first 256 kept and the rest counted, as Parasolid's body check takes a largest number of faults to return. The cheap groups are 1 and 4, both linear in the body's rows; the combinatorial check is the one that keeps the kernel's own loops finite (23.11). `Level::of_build()` is the full set in debug builds and with the crate's `full` feature, the cheap groups otherwise. Groups 2 and 3 are reported as missing until piece G85 and those after it build them, never as passed.

Group 1 runs in two stages, as Parasolid gates its check. The first walks every list and ring once, marking each row it visits, so it ends on any body however its links are broken: a list that comes back on itself, a child listed twice, by the wrong parent or by none, a ring of fins that does not close round its loop or its edge, an edge with one end. The second runs only on sound structure: what each body kind may hold (no faces in a wire or an acorn, no edge without fins in a solid or a sheet, no vertex on no edge outside an acorn or a general body); each loop closes; fin partners (two per edge in a solid, one or two in a sheet, running opposite ways and in one shell); each shell's faces one connected set; the regions (a solid region has a shell, a solid body's shells belong to solid regions, a sheet, wire or acorn body has no solid region); and Euler's formula for each shell of a solid or a sheet, V − E + 2F − L even and at most 2, with a closed edge without vertices counting one vertex and a sheet's boundary cycles added. A general body may be non-manifold, so it is held to the first stage and to what its kind may hold. One shell lying inside another, and a loop running the right way round its face, need geometry: groups 2 and 3.

Group 4 takes the exactness an operation returns with the body (`geom_op::OpOutput`'s `exactness`). A body recorded Exact must have only planar faces, straight edges and straight pcurves, the B-rep kinds chapter 9 makes exact; a Certified or Approximate bound is group 2's to judge against the geometry.

**As built (G85).** Groups 2 and 3 run for primitive bodies, each only on a body the groups before it passed, since they walk the rings group 1 proves finite and compare the carriers group 2 certified. Nothing in the architecture gives the checker a tolerance of its own, so the bound is the body's exactness record, in the steps of its coordinates: 0 for Exact, the proven bound for Certified, the measured deviation for Approximate. Group 2 checks every vertex on its faces' surfaces and at its edges' curve ends, and every edge on its faces. On a plane given by integers a vertex is checked exactly, (n · p + d)² ≤ b² · |n|² in 320-bit integers; every other distance is a certified upper bound in interval arithmetic that stays exact where the doubles are: closed forms for planes given by doubles (unchecked in a body recorded Exact), cylinders, cones (a point past the apex fails), spheres and ring tori; line segments on planes, cylinders and cones; circles against the ideal circle of each surface's family, by the Lipschitz bound of the distance; circle ends by certified sines and cosines of half-turns. Group 3 tests every pair of a face's edges and every pair of faces: two segments exactly with `orient3d` and 2D orientations, meeting only at a shared vertex; a segment and a circle, and two circles, by certified closed forms; two faces on crossing planes, both strictly convex flat polygons of one loop, by each edge of one against the other; a plane perpendicular to a cylinder's or cone's axis, or a plane and a sphere, meeting along the full circle they share; parallel planes certainly apart. Surfaces are embedded by kind (a cone's faces lie on one side of its apex by group 2). Any carrier or pair without a certified test (a NURBS or swept surface, a horn or spindle torus, an ellipse, a pcurve, two curved faces, a face with holes against a crossing plane) is a fault, `Unchecked` or `PairUnchecked`, never a pass; G102, G134, G177 and G322 widen what is certified, and loop orientation and shell nesting remain for them. Every row and pair visited is one meter unit.

## 8.4 Attributes

An attribute is data hung on a body or on one of its named entities: a fire rating on a wall face, a cosmetic thread on a cylinder (section 13.4), a survey point's ID (section 17.2), an edge's continuity record (section 18.7), a temperature field on a body (section 16.2). Section 27 gives attributes to `geom-topo` and section 24.4 stores them, but the decision note does not define them. This section is therefore a proposed amendment to it, pending decision (section 32). The idea is Parasolid's published attribute classes ([07](../../../kit/notes/G01-evidence/07-persistent-naming.md), section 5; [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 8), taken as a concept only (section 29.5) and rebuilt on section 20's names and history.

**Definitions and values.**

- A **definition** gives the attribute's owner and name (`kit.geom` for the kernel's own, otherwise a program's or plugin's ID, as in section 24.4's recipe envelope), its version, the kinds of entity that may carry it, the schema of its value, and its **class**: what its values do at each event below. A definition never changes. A new class or value schema is a new version, and a file holds every definition its bodies use (section 24.4).
- A **value** is canonical data under section 24.3's rules, so two values are equal exactly when their bytes are. The kernel carries values it does not understand, so one program keeps another's fire ratings through its own edits.
- Values are keyed by **persistent name**, in memory as in files, never by index. So only the body itself and its named entities carry attributes; fins, loops and shells never do.

**Events and actions.** Each event is read from the history every operation already emits (section 20.3).

| Event | When, in the history | Actions a class may choose |
| --- | --- | --- |
| Unchanged | The entity is unchanged | None: the value stays |
| Changed | Modified from one input, keeping its name | Keep, or delete |
| Transformed | Modified by a transform operation (section 10.2), which passes its transform | Keep, transform or delete, chosen for each kind of transform (move, rotate, mirror, scale). Transform moves the value with the entity, by section 7's rules. Only value kinds the kernel knows how to move can choose it: points, directions, frames, lengths and fields |
| Split | Split from one input | Propagate (every piece gets the value), or delete |
| Merged | Merged from several inputs | Keep if equal (kept only when every input carries an equal value, otherwise dropped with a diagnostic), combine (the entity carries each input's value, keyed by that input's name), or delete |
| Copied | Generated from one input as its copy, which the operation's schema declares, as for a pattern's instances | Copy, or delete |
| Generated | Generated from inputs (with a role), other than as a copy | None: the entity starts without values, unless the operation's schema sets one, as a fillet sets its edges' continuity record (section 18.7) |
| Deleted | In the list of deleted inputs | None: the value goes with the entity |

**Rules.**

- **Attributes follow the history.** Applying the classes is one shared, pure step in `geom-name`, beside the history it reads: input values, definitions and the operation's history in, output values out. No operation writes attribute code of its own. A property test on every operation checks that the output's values are exactly what the classes and the history give. So the history that keeps references right (section 20.4) keeps attributes right too.
- **No callbacks.** Parasolid lets a program's callback replace a class's behaviour ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 8). Here the class is the whole behaviour. A callback would run program code inside a pure operation (principle 7), and a reader must carry attributes through edits where their owner is absent (section 24.4).
- **Attributes never decide shape.** No operation reads an attribute to decide a body's geometry, topology or names, and an attribute is never part of a name, so renaming a survey point changes no name (section 17.2). An operation that uses attributes for another output, as drawings use the continuity record (section 18.7), lists them as inputs in its schema.
- **Setting one is an operation.** Setting, changing or removing a value is a logged operation like any other, with every entity unchanged in its history, so every replica agrees. Section 18.7's marking of Smooth edges is one. Values sit beside the topology, as section 24.4 stores them in a record of their own, so an attribute edit never changes geometry.
