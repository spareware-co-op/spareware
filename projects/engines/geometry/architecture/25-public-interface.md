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

**As built (G100).** `engine-geometry` holds no function and no implementation; a test reads its source to keep it so (27). Its `value` module re-exports the Stable value types, each a kernel crate's own: grid points and steps, exactness, the curve and surface kinds, bodies, names and history, errors, and an operation's context and output; anything it does not re-export is internal, and it depends on exactly the kernel crates it re-exports. Its `operations::OPERATIONS` lists every operation as a `schema::Operation`: the operation's schema (22) and its tier, `Stable` or `Preview { since }`, the date a preview entered, so a release check can find one older than 12 months. The list is empty until the first operation piece (G101) adds its entry; every operation piece adds one (30.1). `geom-gen` writes the API documentation (`generated/api.md`) and the MCP tool descriptions (`generated/mcp-tools.json`, MCP 2026-07-28, inputs and outputs in JSON Schema 2020-12) from that list, and CI fails until they are regenerated. `kernel-release.yml` runs `cargo-semver-checks` on `engine-geometry` against the release before each new one; the first release is the first baseline. `kit-core`'s permanent IDs and units are not converted yet, because `kit-core` holds no code until its program reaches the Design stage (31.7).
