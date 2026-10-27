# Spare Kit geometry kernel

Start here. This folder is the geometry kernel every Spareware program builds on: one Rust kernel for 2D drafting, buildings, survey and civil, mechanical parts, car surfaces, manufacturing, circuit boards, free-form shapes, meshes and point clouds. This page is the ten-minute tour. To change anything, read [AGENTS.md](AGENTS.md) next; every word you do not know is in [GLOSSARY.md](GLOSSARY.md).

**Status.** The architecture is proposed in decision note [G01](../../kit/notes/G01-geometry-kernel.md) and mapped in full in [ARCHITECTURE.md](ARCHITECTURE.md); the owner's decisions it waits for are in [chapter 32](architecture/32-decisions.md). Phase 1 of the build (ground and numbers) is under way. Built so far: `geom-num`, `geom-run`, `geom-math`, `geom-test`, `geom-fuzz`, `geom-gen` and `geom-bench`. Every other crate in [crates/](crates/) is a scaffold: its `lib.rs` states its job, layer, invariants and sections, and names the board pieces that will fill it in.

## The kernel in five sentences

1. Every coordinate is a whole number of grid steps (156.25 nm for buildings and sites, 6.25 nm for parts and boards), so the data the kernel decides from is exact.
2. Every question about what touches, crosses or contains what is answered with exact integer arithmetic; curved geometry carries a proven error bound and says so.
3. Every operation is a pure function that returns a valid result or a structured error naming what failed, where, and what would make it succeed.
4. Every face, edge and corner is named by the operation that made it, so references survive edits or fail with a reason, never by guessing.
5. Results are bit-identical on every computer, in the browser and on any number of cores.

## The ten-minute tour

### 1. The rules (one minute)

[ARCHITECTURE.md](ARCHITECTURE.md) opens with thirteen principles: exact where topology is decided, certified bounds where exact is impossible, every result stating its exactness, never an invalid result, deterministic, names that never break silently, pure functions with no global state, one kernel for 2D and 3D, built to last, readable by people and AI, proven methods in our own code, measured, and safe Rust. They are not aspirations: [AGENTS.md](AGENTS.md) turns each into checklist items, and most items name the CI check that enforces them.

### 2. The numbers, by following one predicate (three minutes)

Everything rests on `geom-num` ([chapter 6](architecture/06-numbers.md)). A coordinate is an `i64` count of grid steps, never more than 2^40 − 1 steps from its frame's origin (`geom_num::grid::COORD_MAX`). Because coordinates are bounded integers, every "which side is this on" question fits in fixed-size integers and is answered exactly, as a `Sign`: negative, zero or positive.

Follow `orient2d`, "which way do three points turn", through the code to see how the whole kernel works:

1. **A description.** `crates/geom-gen/src/predicates.rs` holds `ORIENT2D`, five lines in a tiny language: the inputs, the differences, the determinant, and `sign det`.
2. **Generated code.** `cargo run -p geom-gen -- write` turns the description into `crates/geom-num/src/generated/orient2d.rs`: a fast `f64` filter with a proven error constant, and an exact integer path. Its header carries a bit-range certificate for every intermediate value and the filter's whole error analysis. No constant is typed by hand, and CI fails if the committed file differs from what the generator writes (`cargo run -p geom-gen -- check`). A second, independent implementation in `geom-gen`'s tests re-derives every number.
3. **The public predicate.** `crates/geom-num/src/predicate.rs` calls the filter first (for `orient2d`, only on the targets where the filter is measured to pay) and the exact path when the filter cannot be sure, and returns `Result<Sign, NumError>`. The error can come only from a defect, never from an input.
4. **Tests that try to break it.** `crates/geom-num/tests/generated_predicates.rs` compares the generated code with an independent big-integer determinant at every sign combination of ±(2^40 − 1), on millions of random inputs, and on degenerate inputs moved one grid step either way. Each randomised test prints its seed, and `SPAREKIT_SEED` replays it.
5. **A benchmark with a gate.** `crates/geom-bench` measures the filter and the exact path on typical, degenerate, nearly degenerate and extreme inputs, counting instructions under Valgrind and operators under WebAssembly fuel. A rise of more than 5% blocks the pull request.
6. **The map.** The module header (what it does, what it guarantees, the method it follows with a link, and its cost) is read into [MAP.json](MAP.json), so a person or an agent can find it by searching.

Beside the predicates, `geom-num` holds the rest of the numbers layer: wide integers (`I256` to `I512`) with every step checked; planes and the points where three planes meet, kept implicit until committed to the grid; intervals rounded outward, for certified bounds on curved geometry; the exactness classes (Exact, Certified, Approximate); the tolerance ladder (`geom_num::ladder`), the one place thresholds come from; correctly rounded maths functions; double-double arithmetic and an exact sum of doubles; and the one random generator (`geom_num::rng`), keyed by explicit seeds.

### 3. The layers and the crates (two minutes)

The kernel is a stack of crates in thirteen layers ([chapter 27](architecture/27-crates-folders.md)): numbers and the operation contract at the bottom; then geometry; topology and names; intersections; the validity checker; arrangements and booleans; meshes and spatial search; other representations; shared operation services; operations; domain toolkits; format and exchange; and the facade at the top. A crate depends only on lower layers. Programs depend only on the facade, [`engine-geometry`](../crates/engine-geometry), the one stable interface; every `geom-*` crate is internal and may change.

Each crate has one job, written at the top of its `lib.rs`. [crates/README.md](crates/README.md) explains the scaffold, and the crate table in code (`../crates/engine-geometry/tests/scaffold/table.rs`) is checked against every `Cargo.toml` on every pull request: the layers, the dependencies, `no_std`, no `unsafe`, and one lint posture copied from `geom-num`.

### 4. How a change is checked (two minutes)

Eleven workflows in `.github/workflows/` guard the kernel. [AGENTS.md](AGENTS.md), section 3, lists the commands to run them yourself.

| Workflow | What it proves |
| --- | --- |
| `kernel.yml` | Formatting, clippy with the kernel's lints, and every test, natively on six runners (Linux, Windows and macOS on x86-64 and arm64) in debug and release, and on 32-bit WebAssembly under wasmtime and Node; the golden digests that prove the same bits everywhere, rerun whenever the toolchain pin or `Cargo.lock` changes, and the proof that a one-bit change to `libm` fails them |
| `kernel-build.yml` | The crate table, and every kernel crate built on its own with warnings denied, natively, for WebAssembly, and for a target with no `std` |
| `kernel-gate.yml` | The merge gate: licences, advisories and sources (cargo-deny), chapter 21's bans on platform maths, hash maps, clocks and randomness, no global state, and `no_std` builds, each shown to fail on a seeded violation |
| `kernel-dev.yml` | The development crates on WebAssembly, every generated file current, and every header complete with `MAP.json` current |
| `kernel-bench.yml` | The count gates: no benchmark more than 5% more expensive than on the merge base |
| `kernel-fuzz.yml` | The fuzz targets: each `geom-fuzz` target under libFuzzer for a minute and under ClusterFuzzLite, and its bounded Kani proofs, the same harness under every engine |
| `kernel-fuzz-campaign.yml` | A long libFuzzer campaign, on demand: the wide integers against a big-integer oracle for 25 CPU hours, started by the `fuzz-campaign` label on a pull request or by hand |
| `kernel-golden.yml` | The golden store: every golden is in the store, every pinned file unchanged, and any edit to a golden carries its record and an owner's approval |
| `kernel-quality.yml` | The mutation and coverage gates (28.8): every mutant in a change caught, or kept in the quality ledger with its reason; every line and branch of the foundation set run, unless the ledger lists it as awaiting its proof; 90% of lines elsewhere; and Miri over the test suites weekly |
| `kernel-nightly.yml` | Every night, the exact predicates' verification suite and attack tests with a fresh seed, printed so a failure replays |
| `kernel-certificates.yml` | The filters' error bounds, machine-checked: Gappa's proofs checked by Rocq and FPTaylor's certificates checked by HOL Light, each tool in a container pinned by digest (orient2d and orient3d today) |

### 5. The map (one minute)

Every crate and module opens with a header in the grammar of [HEADERS.md](HEADERS.md): its job, its guarantees, the published method it follows with a link, its cost, and a record of every outside source it read or ported. `cargo run -p geom-test --bin map -- write` turns the headers into [MAP.json](MAP.json), with every error kind and every outside source, and CI fails if a header is incomplete or the map is stale. [NOTICE](NOTICE) holds the licence text and credit of every source code was ported from.

### 6. Where the reasons are (one minute)

Every decision has a chapter in [architecture/](architecture/), numbered to match G01, and the [index](ARCHITECTURE.md#the-chapters) says what each settles. Behind the chapters is the research: thirteen evidence reports in [../../kit/notes/G01-evidence/](../../kit/notes/G01-evidence/README.md), each reading the actual source of the projects it cites. [Chapter 29](architecture/29-sources-licences.md) records what may be taken from every source and under which licence, [chapter 31](architecture/31-risks.md) the risks and every open technical question, and [chapter 32](architecture/32-decisions.md) the choices waiting for an owner.

### 7. What comes next (one minute)

[Chapter 30](architecture/30-build-order.md) is the work board: every piece in build order, each with what "done" means, in thirteen phases that each end at a gate an owner records. Phase 1 builds the ground and the numbers and ends when every predicate is proven or labelled and the same bits come out on every target; phase 2 adds points, frames, NURBS and every curve and surface kind; phase 3 adds topology, names, the operation contract and the native format. Each piece is checked out as a branch, `piece/<ID>` (the repository's [AGENTS.md](../../../AGENTS.md)).

## Where things live

| Path | What it holds |
| --- | --- |
| [README.md](README.md) | This tour |
| [ARCHITECTURE.md](ARCHITECTURE.md) | The principles, the layers, and the index of every chapter |
| [AGENTS.md](AGENTS.md) | For anyone changing the kernel: the pull-request checklist, the gates, the dependency rule, how to add a module, a predicate or a crate, and the randomised-method rule |
| [GLOSSARY.md](GLOSSARY.md) | Every term used in phases 1 to 3, defined once |
| [HEADERS.md](HEADERS.md) | The grammar of crate and module headers, which the map reads |
| [MAP.json](MAP.json) | The generated map of every crate, module, error kind and outside source |
| `deny.toml`, `deny-ships.toml`, `deny-no-std.toml` | cargo-deny's rules for the merge gate: licences, advisories and sources; what shipping crates may build with; the one allow-list for kernel crates |
| [architecture/](architecture/) | One file per chapter, numbered to match G01 |
| [crates/](crates/) | The kernel's crates (`geom-*`), one folder each, folder name = crate name |
| [../crates/engine-geometry](../crates/engine-geometry) | The stable public facade, the only crate programs depend on |
| [../../kit/notes/G01-evidence](../../kit/notes/G01-evidence/README.md) | The research behind every decision |
| [PROVENANCE.md](PROVENANCE.md) | Generated: every outside source the code takes, from the headers' `Source:` and `Consulted:` lines (chapter 29.20) |
| [NOTICE](NOTICE) | Generated: every ported source's licence text and copyright lines, and the credit owed to every source consulted; ships with every binary |
| [notices/](notices/README.md) | The licence text of each ported source, copied exactly, which `NOTICE` reproduces |
| [methods/](methods/README.md) | Method notes of the method-only procedure, for methods read in LGPL code (chapter 29.20, rule 6) |
