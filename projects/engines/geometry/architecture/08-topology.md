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

- **Arenas with typed indices.** Each body stores each entity kind in a vector, addressed by a 32-bit typed index (`FaceIx`, `EdgeIx` and so on), assigned in operation order. No pointers, no reference counting between entities, no identity by memory address. Truck, Fornjot and OpenCascade all used address identity and all lost determinism through it; Fornjot ended on exactly this arena design ([03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md), section 2).
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
