# 26. How the kernel grows

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

The kernel will be extended for decades, by people and AI agents who were not here when it was written, for programs nobody has thought of yet. Growth is part of the design, not an afterthought. The rule underneath it all: **a closed, carefully guarded core, and an open edge that grows along written contracts.**

## 26.1 What can be added, and how

| To add | Where it goes | What it must come with | Who says yes |
| --- | --- | --- | --- |
| An operation | A module in the operation crate it belongs to | Its schema (inputs with units, outputs, the roles it names, the errors it returns, its exactness), the history contract, the checker postcondition, unit, property and fuzz tests, a benchmark, and a doc comment naming the published method | Kernel owners, after a short design note |
| A query | `engine-geometry` and the crate that owns the data | Its schema, tests, and a cost bound | Kernel owners |
| A curve or surface kind | A new enum variant in `geom-curve` or `geom-surface` | Evaluation, derivatives, conservative bounds, conversion to NURBS with an error, transforms, intersection with every existing kind (directly, or through an explicit and certified NURBS route), a file-format tag and reader, and tests | Kernel owners, with a decision note: kinds are part of the file format forever |
| A representation | A new crate, and new edges in the conversion graph | Its exactness rules, conversions both ways with bounds, and how it takes part in the one boolean | A decision note an owner accepts |
| An exchange format | A `geom-<format>` crate | Round-trip tests against real files, and a legal check for any closed format | Kernel owners |
| A domain toolkit | A `geom-<domain>` crate when two programs need it, otherwise inside the program | Built only from existing operations and geometry kinds, adding its own roles, schemas and checks. Examples: sheet metal (bends, unfolding), threads, gears, layered circuit boards (traces with widths, pads, vias, copper pours with clearance), roofs, stairs, moulds (draft, parting lines, shrinkage), machining (toolpath strategies), 3D-printing preparation | Kernel owners for kernel crates; the program's owners otherwise |
| A plugin's own shapes | The plugin, through the command API, in its sandbox | The plugin composes kernel operations. The result is stored as the plugin's recipe plus the evaluated geometry in native kinds, so the file opens and draws correctly even where the plugin is not installed | The plugin directory's review |

The core (the numbers layer, the geometry kinds, the topology, naming and the file format) changes only through a decision note. Everything else grows freely along the contracts above. If two programs need the same thing, it moves down into the kernel; neither keeps a copy (the root layering rule).

## 26.2 Versions, so old models never change meaning

- **Every operation is versioned.** The operation log records each operation's name and version. Fixing a bug or improving a result makes a new version. The old version's behaviour is kept.
- **Kernel editions.** A document records the kernel edition it was built with. Within an edition, results never change, so replaying a log gives the same model, bit for bit. A new edition may change results. A document moves to it only by an explicit upgrade operation that reports every difference, element by element. This is how Rust's editions let the language change without breaking old code.
- **Evaluated geometry is stored, not only recipes.** A document opens from its stored geometry without replaying anything, so a model saved in 2027 opens in 2077 exactly as it was, even if the operations that made it have long since been improved.
- **Names and formats are never reinterpreted.** A new naming scheme or format version is a new version number, and readers for every old one are kept (section 24).
- **Multi-user sessions agree first.** Everyone editing one model at once uses the same kernel edition, agreed when they join.

## 26.3 Finding your way, for people and AI

A newcomer, human or AI, should be able to find where anything is done, what it guarantees and why, without asking anyone.

| File | What it holds |
| --- | --- |
| `geometry/README.md` | Start here: what the kernel is, the ten-minute tour, where everything lives |
| `geometry/ARCHITECTURE.md` | The decisions in this note, kept current. Every later change links its decision note |
| `geometry/AGENTS.md` | Playbooks for agents and people: how to add an operation, a geometry kind, a format, a domain toolkit; the checklist each pull request must pass; the principles in section 3; the randomised-method rule, which G32 wrote in `geom_num::rng`'s header and G17 lifted here |
| `geometry/GLOSSARY.md` | Every term defined once in plain words: fin, pcurve, pave block, region, ε_fit, Certified, role, carrier |
| `geometry/MAP.json` | A machine-readable map of every crate, module, operation, geometry kind, invariant and error kind, with links. Generated from annotations in the code and checked in CI, so it can never drift from the code |
| Each crate's `lib.rs` | Its one job, the invariants it keeps, and the sections of this note it implements |
| Each module's header | What it does, what it guarantees, the published method it follows with a link, and its cost |
| The invariant and error catalogues | Every invariant the checker enforces and every error the kernel can return, with an example of each, generated from the code |

Agents get the same map people do. An AI asked to "add a fillet option" reads `AGENTS.md`, finds blends in `MAP.json`, reads the module header and the invariants it must keep, and finds the playbook's checklist. The checklist is enforced by CI, not by memory.
