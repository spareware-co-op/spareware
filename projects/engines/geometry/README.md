# Spare Kit geometry kernel

Start here. This folder is the geometry kernel every Spareware program builds on: one Rust kernel for 2D drafting, buildings, survey and civil, mechanical parts, car surfaces, manufacturing, circuit boards, free-form shapes, meshes and point clouds.

**Status:** being designed. The architecture is proposed in decision note [G01](../../kit/notes/G01-geometry-kernel.md) and mapped in full in [ARCHITECTURE.md](ARCHITECTURE.md). There is no code yet. An owner is building the kernel (piece G01, [issue #169](https://github.com/spareware-co-op/spareware/issues/169)); please do not start kernel work of your own. To help, comment on that issue.

## The kernel in five sentences

1. Every coordinate is a whole number of grid steps (156.25 nm for buildings and sites, 6.25 nm for parts and boards), so the data the kernel decides from is exact.
2. Every question about what touches, crosses or contains what is answered with exact integer arithmetic; curved geometry carries a proven error bound and says so.
3. Every operation is a pure function that returns a valid result or a structured error naming what failed, where, and what would make it succeed.
4. Every face, edge and corner is named by the operation that made it, so references survive edits or fail with a reason, never by guessing.
5. Results are bit-identical on every computer, in the browser and on any number of cores.

## Where things live

| Path | What it holds |
| --- | --- |
| [ARCHITECTURE.md](ARCHITECTURE.md) | The principles, the layers, and the index of every chapter |
| [architecture/](architecture/) | One file per chapter, numbered to match G01 |
| [../crates/engine-geometry](../crates/engine-geometry) | The stable public facade, the only crate programs depend on |
| `crates/` | The kernel's internal crates (`geom-*`), added piece by piece as the build order allows |
| [../../kit/notes/G01-evidence](../../kit/notes/G01-evidence/README.md) | The research behind every decision, each report reading the actual source it cites |
| [PROVENANCE.md](PROVENANCE.md) | Generated: every outside source the code takes, from the headers' `Source:` and `Consulted:` lines (chapter 29.20) |
| [NOTICE](NOTICE) | Generated: every ported source's licence text and copyright lines, and the credit owed to every source consulted; ships with every binary |
| [notices/](notices/README.md) | The licence text of each ported source, copied exactly, which `NOTICE` reproduces |
| [methods/](methods/README.md) | Method notes of the method-only procedure, for methods read in LGPL code (chapter 29.20, rule 6) |

Still to come, as the detailed design proceeds: `AGENTS.md` (playbooks for adding operations, geometry kinds, formats and domain toolkits), `GLOSSARY.md` (every term defined once), and `MAP.json` (a generated, CI-checked map of every crate, module, operation, invariant and error).
