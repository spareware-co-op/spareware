# 25. Public interface and stability

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

The kernel follows Spareware's three stability tiers (EXTENDING.md, section 4).

| Tier | What | Promise |
| --- | --- | --- |
| Stable | `engine-geometry`: the operation schemas, the value types (grid points, curves, surfaces, bodies, names, errors), the native file format | Semantic versioning. A breaking change is a new major version that runs beside the old one; formats are read forever |
| Preview | New operations and types being tried | May change. Not usable by published plugins. Graduate or drop within 12 months |
| Internal | Every `geom-*` crate | May change at any time. Nothing outside the kernel depends on them |

- Programs, the C interface, the WebAssembly bindings and the Python bindings all go through `engine-geometry`. The bindings sit in their own crates beside it, `engine-geometry-c`, `engine-geometry-wasm` and `engine-geometry-py`, which hold no geometry logic and are the only crates in the kernel's family that may contain `unsafe`, audited and limited to the foreign-function boundary (27, accepted in 32.13).
- **Geometry moves between programs with nothing lost.** Every Spareware program reads and writes the same native geometry through the same kernel. A building modelled in Spare Construct opens in a rendering or 3D studio program as the same bodies, with the same names and the same exactness, and can stay linked so edits flow through. There is no export and re-import between our own programs, and so no translation loss.
- `cargo-semver-checks` runs on `engine-geometry` before every release.
- Plugins reach geometry only through Spare Construct's or Spare Studio's command API, never by linking kernel crates (EXTENDING.md, section 1).
