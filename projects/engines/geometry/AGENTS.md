# Working on the geometry kernel

This file is for every agent and person who changes anything in `projects/engines/geometry/` or in the facade, `projects/engines/crates/engine-geometry`. The repository's own [AGENTS.md](../../../AGENTS.md) still applies in full: check out a piece before touching anything, work only on its branch, keep the legal rules, sign off every commit, and hand over properly. This file adds what is particular to the kernel:

1. [Before you start](#1-before-you-start): what to read, and where things are.
2. [The pull-request checklist](#2-the-pull-request-checklist): the thirteen principles of [ARCHITECTURE.md](ARCHITECTURE.md), as questions every pull request answers.
3. [The gates](#3-the-gates): the commands CI runs, to run yourself before you push.
4. [Dependencies](#4-dependencies): use good crates, or convert good code; never hand-roll around a dependency.
5. [How to add a module](#5-how-to-add-a-module).
6. [How to add a predicate](#6-how-to-add-a-predicate).
7. [How to add a crate](#7-how-to-add-a-crate).
8. [The randomised-method rule](#8-the-randomised-method-rule): the rule every kernel method that uses random numbers follows.
9. [Playbooks still to come](#9-playbooks-still-to-come).
10. [When something does not fit](#10-when-something-does-not-fit).

Words you do not know are in [GLOSSARY.md](GLOSSARY.md). The ten-minute tour is [README.md](README.md). A path in backticks is written from the repository root when it starts with `projects/` or `.github/`, and from this folder, `projects/engines/geometry/`, otherwise.

## 1. Before you start

- **Your piece.** Read its entry in the work board, [architecture/30-build-order.md](architecture/30-build-order.md) (the same entries as data in `architecture/30-build-order.json`), its GitHub issue (`gh issue view <number> --repo spareware-co-op/spareware`), and every chapter of [architecture/](architecture/) it cites. "Done when" is the contract; 30.1's standard bar (tests, benchmark, determinism, readable headers, safe Rust) applies to every code piece on top of it.
- **The stored research comes first.** The evidence behind every decision is in [../../kit/notes/G01-evidence/](../../kit/notes/G01-evidence/README.md). Read it before any web lookup. Follow the method it names, and read the actual source of anything you port, at a pinned commit.
- **Find where things are in `MAP.json`.** [MAP.json](MAP.json) is generated from every crate and module header (section 5). Each crate has its job, layer, invariants, sections and catalogue family; each module its job, guarantees, method and cost; each error enum its kinds; each outside source its licence and pinned link. Search it for a word ("interval", "Blend", "orient3d") to find the crate and file. [crates/README.md](crates/README.md) explains the crate scaffold, and chapter 27 ([architecture/27-crates-folders.md](architecture/27-crates-folders.md)) gives every crate its one job and layer.
- **The toolchain is pinned.** `rust-toolchain.toml` at the repository root pins Rust 1.99.0 with rustfmt, clippy and the `wasm32-wasip1` and `wasm32-unknown-unknown` targets; the first `cargo` command in the repository installs it. The merge gate's build also needs `rustup target add wasm32v1-none`, and its licence check needs cargo-deny 0.20.2 (`cargo install --locked cargo-deny --version 0.20.2`). The workspace is edition 2024: `gen` is a reserved keyword.
- **Stay in your piece.** If something outside it looks wrong, say so in the pull request; do not fix it there. Never put local paths, personal data, or any firm's or client's project in the repository.

## 2. The pull-request checklist

Every pull request that touches the kernel meets each principle below. Each item names the check that enforces it. "Review" means only a reviewer checks it today; where a board piece will automate the check, it is named. A pull request that breaks a principle does not merge.

### 1. Exact where it decides topology

- [ ] Every decision about whether things touch, cross, contain or order is an exact predicate that returns a `geom_num::Sign`, on grid points or implicit points. No `f64` comparison, epsilon or rounding decides one. (Review; chapter 6.2.)
- [ ] A new predicate is generated from a description, never written with a hand-typed filter constant (section 6 below). (`cargo run --locked -p geom-gen -- check`; geom-gen's independent checker, `crates/geom-gen/tests/predicate_checker.rs`.)
- [ ] Every fixed-width integer step in kernel code is a checked method (`checked_*`), and every bit budget is derived in the module header and tested with every coordinate at ±(2^40 − 1). (clippy denies `arithmetic_side_effects`; tests.)
- [ ] A tie that means something (two faces sharing a plane) is handled explicitly. A tie an algorithm must break uses symbolic perturbation ordered by persistent keys, never by memory address. (Review; chapter 6.2; piece G30.)

### 2. Certified where exact is impossible

- [ ] Every curved value that feeds a decision carries a proven bound from interval or Bernstein arithmetic, rounded outward (`geom_num::interval`), never a bound found by sampling. (Review; chapter 6.4.)
- [ ] Nothing certified rests on a maths function without a proven error bound (`geom_num::math::unproven`). An enclosure that needs one returns `NumError::UnprovenMaths`. (Review; tests.)

### 3. Every result states its exactness

- [ ] A result says whether it is Exact, Certified with a bound or Approximate with a measured deviation (`geom_num::Exactness`). Its class is the least exact of its inputs', and bounds add. (Review; chapter 9.)
- [ ] Nothing lowers the class, or returns a bound above the ceiling (ε_fit unless the caller accepts more), unless the caller asked. Otherwise it is an error (`NumError::ToleranceExceeded` in `geom-num`, `ExactnessNotAccepted` in an operation). (Review; the contract laws of G103 once they exist.)

### 4. Never an invalid result

- [ ] Inputs are never modified. A function that fails leaves everything exactly as it was, and reports a typed error, never a partial result. (Review; tests of the failure path.)
- [ ] An operation's result passes the validity checker before it is returned. (Once the checker, G84, and the contract laws, G103, exist; until then no operations exist.)

### 5. Deterministic

- [ ] No platform maths (every maths function comes from `geom_num::math`), no `HashMap` or `HashSet`, no `f64::max` or `f64::min` (use the total-order helpers), no `Iterator::sum` or `Iterator::product` (write the order out, or use `ExactSum`), no clocks, no thread identity, and no randomness except through `geom_num::rng`. (Every kernel crate's `clippy.toml` bans them by path; `cargo clippy ... -- -D warnings` and the merge gate's `gate_clippy`.)
- [ ] Parallel work goes only through `geom-run`'s helper, whose chunks do not depend on the thread count. Ties are broken by persistent names or keys, never by memory address. (Review; chapter 21; chapter 23.4.)
- [ ] The tests give the same results natively and on `wasm32-wasip1`, and every golden digest is SHA-256 of canonical bytes. (`kernel.yml` runs the suite on six native runners and under wasmtime and Node.)
- [ ] A method that uses random numbers follows [the randomised-method rule](#8-the-randomised-method-rule). (`crates/geom-num/tests/rng_ban.rs` reads rule 1 from this file; clippy bans the rest.)

### 6. Names never break silently

- [ ] Every topological entity an operation makes is named from the operation that made it, the operation emits its history, and a reference resolves exactly, follows a recorded split or merge, or fails with a reason, never by geometric guessing. (Review against chapter 20 until `geom-op` and `geom-name` exist, pieces G82 and G87 to G89, and the history laws, G88.)

### 7. Pure, immutable and headless

- [ ] No `static`, `thread_local!`, `OnceLock`, `LazyLock` or other lazy global in any crate that ships, its `#[cfg(test)]` modules included. A table is a `const`. (The scaffold check's `globals` rule: `cargo test --locked -p engine-geometry --test scaffold`.)
- [ ] Every input is an argument. Kernel crates are `no_std`: no files, no environment, no screen, no printing. (The scaffold check, and the merge gate's `gate_build`, which builds every kernel crate for `wasm32v1-none`, a target with no `std`.)

### 8. One kernel for 2D and 3D

- [ ] 2D work uses the same number types, curve kinds and naming as 3D; no parallel family of 2D types. (Review; chapters 7 and 14.)

### 9. Built to last

- [ ] Nothing changes meaning in place. A change to what an existing function returns is a new version beside the old one (a new operation version, kernel edition, generator definition or file-format version), with its own golden values, and old ones keep working. (Review; chapter 26.2; the golden digests in `crates/geom-num/tests/determinism.rs` and `crates/geom-num/tests/rng_determinism.rs` fail on an edit in place.)
- [ ] Programs reach the kernel only through `engine-geometry`. (The scaffold check refuses a dependency on an internal `geom-*` crate from outside the kernel.)

### 10. Readable by people and AI

- [ ] Every crate and module header is complete in the grammar of [HEADERS.md](HEADERS.md): a summary, `Guarantees`, a `Method` with a link, a `Cost`, and a `Source:` or `Consulted:` record for every link to outside code. `MAP.json` is regenerated and committed. (`cargo run --locked -p geom-test --bin map -- check`.)
- [ ] Small modules with one job. Every public item has a doc comment, and every error enum and each of its variants is documented. (`missing_docs` and clippy's pedantic group, warnings denied; the map's `Error doc` rule.)
- [ ] No unexplained constant: every threshold comes from the tolerance ladder in `geom_num::ladder`, and every constant has its derivation beside it. (Review; chapters 6.5 and 27.)

### 11. Proven methods, our code, raised to our bar

- [ ] The pull request names the published method it follows, and says which open project's source was read and at which commit. (Review; the map checks that every module names a method with a link.)
- [ ] Dependencies follow [section 4](#4-dependencies): well-known permissive crates rather than hand-rolling; never GPL or AGPL code; LGPL sources read for method only, with a method note. (`gate_deny`; the map's `Source`, `Consulted` and `Depend` rules.)
- [ ] Anything ported keeps its origin's notice: a `Source:` record with the pinned link, the copyright lines and what changed for Spare Kit, an `SPDX-License-Identifier:` line, and the crate's `license` field naming the licence beside MPL-2.0. Where the source has a known weakness, the port fixes it and the pull request says how. (The map's `Source` and `SPDX` rules; review.)

### 12. Measured

- [ ] Unit tests, property tests that print their seed and replay with `SPAREKIT_SEED`, a fuzz target where the piece asks for one, and a benchmark. (Review; 30.1's standard bar.)
- [ ] A new public function of `geom-num` has a benchmark in `geom-bench`, or an entry in `geom_bench::catalogue::UNMEASURED` saying why not. (`crates/geom-bench/tests/coverage.rs`.)
- [ ] No benchmark's instruction, cache, heap or WebAssembly-fuel count rises by more than 5%. (`kernel-bench.yml`, against the merge base.)

### 13. Safe Rust

- [ ] No `unsafe` code, and no `unwrap`, `expect`, `panic!`, unchecked indexing, `todo!`, `unimplemented!` or `unreachable!` outside tests. (`unsafe_code = "forbid"` and clippy's denials in every crate that ships; the merge gate's `overrides` rule refuses a lint lowered in source outside test code.)
- [ ] Recursion is bounded, with the bound stated in the module header; prefer an explicit stack. (Review until the recursion check, G13.)
- [ ] A long loop charges its budget and polls cancellation through `geom-run`, and never unwinds. (Review; chapter 23.10.)

### And the process

- [ ] The piece is checked out and the branch is `piece/<ID>` (root AGENTS.md, section 3).
- [ ] Every commit is signed off (`git commit -s`); the pull request says what changed, how it was checked, every outside source, and `Closes #<issue>`.
- [ ] Every gate in section 3 passes locally.
- [ ] Anything in "done when" that this piece does not finish is listed in the pull request with the board piece that owns it, never stubbed and called done.

## 3. The gates

CI runs these on every pull request that touches the kernel. Run them yourself, in this order, before you push; a pull request whose gates fail does not merge. `KERNEL_CRATES` is every crate in `crates/`, as the workflows build it:

```bash
KERNEL_CRATES=$(for d in projects/engines/geometry/crates/*/; do printf -- '-p %s ' "$(basename "$d")"; done)
```

Run everything from the repository root. The first build of the whole kernel takes a few minutes and a few gigabytes of disk.

| Command | What fails it | Workflow |
| --- | --- | --- |
| `cargo fmt --check $KERNEL_CRATES` | Formatting | `kernel.yml` |
| `cargo clippy $KERNEL_CRATES --all-targets --locked -- -D warnings` | Any lint: the kernel's denials, chapter 21's bans, pedantic warnings, missing docs | `kernel.yml` |
| `cargo test $KERNEL_CRATES --locked --no-fail-fast` | Any test, debug build | `kernel.yml` (also in release, with overflow checks on) |
| `cargo test $KERNEL_CRATES --target wasm32-wasip1 --release --locked --no-fail-fast`, without `geom-test`, `geom-fuzz`, `geom-gen` and `geom-bench` | Any kernel test on 32-bit WebAssembly | `kernel.yml`, under wasmtime and Node |
| `cargo test --locked -p geom-test -p geom-fuzz -p geom-gen --target wasm32-wasip1` | The development crates on WebAssembly | `kernel-dev.yml` |
| `cargo run --locked -p geom-gen -- check` | A committed generated file differs from what its generator writes, or a generated folder holds an orphan | `kernel-dev.yml` |
| `cargo run --locked -p geom-test --bin map -- check` | An incomplete crate or module header, an uncredited link to outside code, or a stale `MAP.json` | `kernel-dev.yml` |
| `cargo test --locked --package engine-geometry --test scaffold` | The crate table, the layer rule, `no_std` and `forbid(unsafe_code)`, the lint posture, the ban on global state, the gate's configuration | `kernel-build.yml`, `kernel-gate.yml` |
| `cargo test --locked --package engine-geometry --test scaffold -- --ignored gate_deny --nocapture --test-threads 1` | cargo-deny: licences off chapter 29.2's allow-list, advisories, sources, bans; an unrecorded licence choice | `kernel-gate.yml` |
| `cargo test --locked --package engine-geometry --test scaffold -- --ignored gate_clippy --nocapture --test-threads 1` | clippy over every crate that ships, with every feature selection, and a probe that breaks each rule once | `kernel-gate.yml` |
| `cargo test --locked --package engine-geometry --test scaffold -- --ignored gate_build --nocapture --test-threads 1` | A kernel crate that does not build for `wasm32v1-none` (no `std`) or `wasm32-unknown-unknown` | `kernel-gate.yml` |
| `cargo test --locked -p geom-bench --release` | The benchmarks' inputs, golden digests and wiring; a public `geom-num` function neither measured nor excused | `kernel-bench.yml` (the count gates run there too, on Linux) |
| `cargo bolero test orient2d_fuzz --package geom-fuzz --engine libfuzzer` and `cargo kani --tests -p geom-fuzz --harness orient2d_proof --exact`, on Linux or macOS (the crate README says how to install both) | A fuzz target that finds a case breaking its law, or a bounded proof that fails | `kernel-fuzz.yml` (with ClusterFuzzLite) |
| `cargo run --locked -p geom-test --bin golden -- check` | A golden outside the store, a pinned file changed, or a golden edited without its record | `kernel-golden.yml` (and every golden on every target in `kernel.yml`) |

If you touch `geom-bench`, also run `cargo clippy --locked --target x86_64-unknown-linux-gnu -p geom-bench --all-targets -- -D warnings` (`rustup target add x86_64-unknown-linux-gnu`; its instruction-count benchmarks compile only for Linux). On another platform the count gates themselves cannot run; `geom-bench`'s crate header explains how to run them on Linux.

**WebAssembly without wasmtime.** cargo runs `wasm32-wasip1` test binaries through the program in `CARGO_TARGET_WASM32_WASIP1_RUNNER`. CI uses wasmtime, or Node's WASI with this script, the one `kernel.yml` writes (save it anywhere outside the repository):

```js
// wasi-run.mjs: runs a wasm32-wasip1 test binary under Node's WASI, as cargo's runner.
import { readFile } from 'node:fs/promises';
import { WASI } from 'node:wasi';
import { argv, env, exit } from 'node:process';

const [file, ...args] = argv.slice(2);
const wasi = new WASI({ version: 'preview1', args: [file, ...args], env, returnOnExit: true });
const module = await WebAssembly.compile(await readFile(file));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
exit(wasi.start(instance));
```

```bash
export CARGO_TARGET_WASM32_WASIP1_RUNNER="node --no-warnings /path/to/wasi-run.mjs"
```

**After the gates.** `cargo run --locked -p geom-gen -- write` and `cargo run --locked -p geom-test --bin map -- write` regenerate what the two checks compare; commit what they write. Never edit a generated file or `MAP.json` by hand.

## 4. Dependencies

The owner's rule, for every crate: **use good stuff, or convert good stuff.** Use a well-known, maintained crate under a permissive licence rather than writing your own, and never hand-roll a workaround to avoid a dependency: no home-made parsers, hashes, formats or function-pointer tricks. Write the kernel's own code only where a principle truly needs it (exact arithmetic a library cannot guarantee, determinism, `no_std`), and say why in the module header. Chapter 29.1 holds the full rule; piece G12a writes the owner's direction of 2026-10-22 into it.

- **Licences.** Only chapter 29.2's allow-list: `MIT`, `MIT-0`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `BSL-1.0`, `ISC`, `Zlib`, `0BSD`, `Unlicense`, `CC0-1.0`, `Unicode-3.0`, `Unicode-DFS-2016`, and `MPL-2.0`. Never GPL or AGPL code. LGPL sources may be read for their method only, under 29.20's rule 6, with a method note.
- **Pin it and note it.** Add the crate with its exact version (`name = "1.2.3"`; `Cargo.lock` is committed and every command runs `--locked`), and a comment beside it in `Cargo.toml` saying what it is for and its licence.
- **Inside a kernel crate.** Every crate that ships in a kernel crate is vetted against the principles (deterministic, `no_std` where required, audited, pinned) and listed in the one allow-list, `deny-no-std.toml`'s `allow`, which today holds `libm` alone; the scaffold table's `registry` list says which kernel crate may name it. Adding one is an owner's decision through that file. The crate's header records its licence in a `Licences` list (the map's `Depend` rule).
- **In a development crate or a test.** A development dependency of any crate, and any dependency of `geom-test`, `geom-fuzz`, `geom-gen` or `geom-bench`, needs only a licence on the allow-list. A development crate's header lists every outside package it brings in, with its licence, and `crates/geom-test/tests/licences.rs` checks that list against `cargo tree`.
- **Licence choices.** Where a crate offers a choice (`MIT OR Apache-2.0`), the merge gate needs the choice recorded: add a row `("<crate>", "<expression exactly as Cargo.lock's package declares it>", "<choice>")` to `LICENCE_CHOICES` in `projects/engines/crates/engine-geometry/tests/scaffold/licence.rs`, in name order, choosing the first identifier of 29.2's list the expression offers (so `MIT` whenever it is offered). `gate_deny` names any crate whose choice is missing.

## 5. How to add a module

A module is one file with one job. These steps take a module from nothing to a passing pull request.

1. **Choose the crate.** The module belongs to the crate whose one job covers it (chapter 27; each crate's job in `MAP.json`). If none does, stop: a new crate needs the owner (section 7).
2. **Create the file and declare it.** `src/<name>.rs` (or `src/<parent>/<name>.rs` inside a module folder), and `pub mod <name>;` (or `mod <name>;` for a private one) in `lib.rs` or the parent module, in name order with the others. In `geom-num`, also add the file to `GEOM_NUM` in `crates/geom-bench/tests/coverage.rs`, which reads every `geom-num` module's source to find its public functions; that test names a module it cannot read.
3. **Write the header.** The file opens with `//!` lines, before any attribute or `use` line, in the grammar of [HEADERS.md](HEADERS.md). A module header has, each field opening its own paragraph:

   ```rust
   //! One sentence saying what the module is for, ending in a full stop.
   //!
   //! What it does: optional, a fuller description.
   //!
   //! Guarantees:
   //!
   //! - **A short claim.** What holds for every input, stated so a test can attack it.
   //!
   //! Method: the published method, with a link: a web address, a DOI written
   //! `doi:10.1145/2063384.2063405`, or a repository path in backticks under
   //! `projects/engines/geometry/` or `projects/kit/notes/` that ends in `.md` or `/` and exists.
   //!
   //! Cost: time, memory, allocation and recursion.
   ```

   `Method: none; <the reason>.` fits only a module that defines no function outside trait implementations. A link to outside code anywhere in the header or a doc comment needs a provenance record in the header, pinned to a full commit: `Consulted: <name>, <licence>, <pinned link>, <what it was consulted for>` for a method you read, or a `Source: PORT from ...` record and an `SPDX-License-Identifier:` line for code you ported (HEADERS.md, "Provenance lines"). A module that ports nothing may carry `SPDX-License-Identifier: MPL-2.0` or no SPDX line.
4. **Write the code to the kernel's rules.** Kernel crates are `no_std` (`#![cfg_attr(not(test), no_std)]` in `lib.rs`), so `core` only: write a fused multiply-add as `geom_num::math::fma` and a square root as `geom_num::math::sqrt`. Every fixed-width step is a checked method, and a cast that could lose a value needs its bound written beside it. No `unwrap`, `expect`, `panic!` or indexing: return a `Result` with a documented error enum, and use `get`. A public error enum's name contains `Error`, it is declared at the top level of its module, and the enum and each variant have a doc comment. The checklist in section 2 has the rest.
5. **Write the tests.** Unit tests go in a `#[cfg(test)] mod tests` at the end of the file; tests of the public interface go in the crate's `tests/<name>.rs`. Test code may use native arithmetic and casts: copy the `#[allow(...)]` block an existing test file or test module of the same crate opens with. Test the guarantees in the header one by one: at the extremes (every coordinate at ±(2^40 − 1)), on degenerate inputs and one step from them, and against an independent oracle (`num-bigint`, `num-rational` or `dashu` for exact values). Random test inputs come from a fixed default seed, printed as `seed: <hex>`, which `SPAREKIT_SEED=<hex>` replaces (copy the pattern in `crates/geom-num/tests/generated_predicates.rs`, or use `geom_test::seed` in a crate that may depend on `geom-test`). Write an `attack_<piece>.rs` file of tests that try to break every claim your piece makes.
6. **Measure it.** A public `geom-num` function gets a benchmark in `geom-bench`, or a reasoned entry in `geom_bench::catalogue::UNMEASURED`: `geom-bench`'s crate header, "Changing the benchmarks", lists the files a benchmark touches, and `crates/geom-bench/tests/golden.rs` prints the new `golden.txt`. Other crates measure what their piece asks.
7. **Regenerate the map.** `cargo run --locked -p geom-test --bin map -- write`, and commit `MAP.json` with the module.
8. **Run every gate** in section 3, then commit (`git commit -s`).

## 6. How to add a predicate

A predicate in this kernel is generated, never hand-written (chapter 6.2): its `f64` filter, its error constant, its exact integer path and the bit-range certificate of every intermediate all come from a short description, so no constant is ever typed by hand. The generator is `geom-gen`; its output lands in `crates/geom-num/src/generated/`. The language is in `crates/geom-gen/src/predicate_lang.rs`'s header: `predicate`, `input ...: point2`, `point3` or `rounded <bits> in <group>`, `let <name> = <expression>` with `+`, `-` and `*`, and `sign <name>`. Read its rules on names before you choose yours: the language reserves the names the emitted code uses, and a `let` may not take a coordinate's loaded name (`a_x` for `a.x`).

1. **Describe it.** In `crates/geom-gen/src/predicates.rs`, add the description as a `pub const` beside `ORIENT2D` and the others: a comment line saying what it decides and its sign convention, then the description. Write the expression in the form the generator should evaluate; every operation is evaluated exactly as written, left to right.
2. **Register it.** In the same file:
   - Add a `Predicate` to `PREDICATES`: its `generator` name `predicate-<name>` with `_` written `-`, its `constant` (the constant's name, as a string), its `text` (the constant), its `output` `projects/engines/geometry/crates/geom-num/src/generated/<name>.rs`, and a `Comparison`. The comparison's `name` is `<name>`; its `shewchuk` and `fpg` give the published bounds for the same predicate, or `—` with the reason in brackets, as `plane_side` does; and its `serves` says what the predicate evaluates and what calls it. The generated module's header prints `serves` inside the sentence "The `f64` filter and the exact path of {serves}.", so write it as the end of that sentence, with no closing full stop, and say "Nothing calls it yet" when nothing does (step 5).
   - Add a `generate_<name>` function like `generate_orient2d`, and add the new file to the `COMMITTED` list of that file's tests.
   - Name the new generator in the file's header, whose first sentence lists every generator, and update the count of predicates in its `Cost` line.

   Then add a `Generator` to `GENERATORS` in `crates/geom-gen/src/registry.rs`. Raise each array's length by one.
3. **Generate it.** `cargo run --locked -p geom-gen -- write` writes `crates/geom-num/src/generated/<name>.rs`, with the description, the certificate and the error analysis in its header, and adds `pub mod <name>;` to `crates/geom-num/src/generated/mod.rs`. Read the certificate: every intermediate's type is the narrowest that holds it, and a result wider than the widest type the generator emits is refused. Never edit the generated files; change the description and generate again.
4. **Check it independently.** Add the new file to `COMMITTED` in `crates/geom-gen/tests/predicate_checker.rs`. That checker is a second implementation of the language, the certificate and the error analysis; it re-derives every number and every line of the committed module and must agree.
5. **Give it a public entry point, if anything calls it.** A predicate the kernel uses is called from a function in `geom-num` (as `geom_num::predicate::orient2d` calls `generated::orient2d`), returning `Result<Sign, NumError>`, with its sign convention, its degenerate cases and its bit-budget derivation in the doc comment. Then the generated module's `serves` names it.
6. **Test it against exact arithmetic** in `crates/geom-num/tests/` (follow `generated_predicates.rs`): the filter never gives a sign the exact path contradicts, on random inputs across the whole grid and on degenerate inputs moved one step either way, so both outcomes of the filter occur; the exact path equals an independent big-integer evaluation (`num-bigint`) at every sign combination of ±(2^40 − 1) and on random inputs, and is never `None`; and the predicate's algebraic laws hold (swapping points, translation by a grid vector). Copy the `#[allow(...)]` block that `generated_predicates.rs` opens with. A predicate the kernel calls is also checked by chapter 28.6's second method where one exists, as `crates/geom-num/tests/attack_g28.rs` checks the four explicit-point predicates against the `robust` crate's adaptive ones.
7. **Measure it.** In `geom-bench` (`crates/geom-bench/`): add the module to `GENERATED_PARTS` in `crates/geom-bench/tests/coverage.rs`; benchmark `filter` and `value` on the four input classes (typical, degenerate, nearly degenerate, extreme), each needing its inputs in `crates/geom-bench/src/inputs/generated.rs`, its kernel in `crates/geom-bench/src/kernels.rs` (module `generated`), its `Op` in `crates/geom-bench/src/catalogue.rs` and its `#[bench::...]` lines and group entry in `crates/geom-bench/benches/instructions/gate.rs`; list `exact` in `UNMEASURED` (one call to `value`, which is measured, and the sign of the value), as the other predicates do. `crates/geom-bench/tests/inputs.rs` checks that each class is what it claims and `crates/geom-bench/tests/attack_g06.rs` checks every result against exact arithmetic, so each needs the new benchmarks too. Run `cargo test --locked -p geom-bench --release`; `crates/geom-bench/tests/golden.rs` fails and prints the new `golden.txt`, which replaces `crates/geom-bench/src/golden.txt`.
8. **Regenerate the map and run the gates.** `cargo run --locked -p geom-test --bin map -- write` (the generated module is a new module of `geom-num`), then every gate in section 3. `cargo run --locked -p geom-gen -- check` must report every output current.

A predicate whose filter has not been proven by FPTaylor or Gappa says "checked, not proven" (chapter 28.6); every generated module does today, until pieces G35 and G40.

## 7. How to add a crate

Crates come from chapter 27's crate table, which an owner settles (owner action G08), so a new crate needs the owner's change to that table first. Once it is agreed, the mechanical steps are:

1. The folder `crates/<name>/`, with the package named as the folder and no `path =` in `[lib]` or `[[bin]]`.
2. A crate header in `crates/<name>/src/lib.rs` in HEADERS.md's crate grammar: the summary (`Spare Kit geometry kernel: <topic>.`), `One job: `, `Layer: <n>`, an `Invariants` list, `Sections: ` naming its chapters, and `Family: ` if its job is a catalogue family.
3. A kernel crate copies `geom-num`'s `clippy.toml` and its `[lints]` tables exactly, is `no_std`, and forbids unsafe code. Development crates (`geom-test`, `geom-fuzz`, `geom-gen`, `geom-bench`) are exempt from the lint posture.
4. Its row in the crate table, `projects/engines/crates/engine-geometry/tests/scaffold/table.rs`, with exactly the kernel crates its `Cargo.toml` names, and the count of present crates in `../crates/engine-geometry/tests/attack_g09.rs` (`present.len()`) raised by one.
5. `cargo run --locked -p geom-test --bin map -- write`, and every gate in section 3.

## 8. The randomised-method rule

This is the rule piece G32 wrote in `geom_num::rng`'s module header and piece G17 lifted here, as the board asked: word for word, except that its links to `geom_num::rng`'s items are written as full paths, "here" names the module, the golden test's path is written from this folder, and "this piece's" reading is named as piece G32's. `crates/geom-num/tests/rng_ban.rs` reads rule 1 from this file and fails unless it names every crate the kernel's `clippy.toml` bans.

A kernel method may use random numbers (RANSAC fitting, random sampling, randomised layout or grouping, the seeds of a stochastic lattice) only as a pure, deterministic function of its inputs. It follows every one of these:

1. **One generator.** Every random number comes from `geom_num::rng`: Philox4x32-10 with the
   layout and draw definitions written there. No other generator, no entropy, no clock, no
   memory address, no process or thread id, and no hash-map order. Each kernel crate's
   `clippy.toml`, a copy of `geom-num`'s, bans by path every such source in std and core
   (including the x86 hardware random-number and cycle-counter instructions), and the entry
   points of `rand`, `rand_core`, `getrandom`, `hashbrown`, `foldhash`, `fastrand` and `rayon`'s
   thread index. The merge gate's dependency allow-list (piece G12) keeps every other crate out
   of kernel crates. What no path can name, such as a memory address turned into an integer, is
   a review rule (chapter 21).
2. **The seed is an input.** The seed is an explicit input of the operation, stored with its
   other inputs in the operation log. A default seed is a written constant, recorded in the log
   like any other default. `OpContext` holds no randomness (chapter 10.1), and no seed is ever
   derived from the time, the machine or the run.
3. **One stream per consumer, numbered by position.** Each independent consumer (one RANSAC
   trial, one item of a parallel loop, one call of a randomised sub-method) draws from streams
   no other consumer uses. A stream id is fixed by the consumer's position in the input or by a
   numbering written in the method, never by a thread, arrival order or timing, and never by
   which batch of a parallel loop runs it: batch sizes (the parallel helper's `ChunkPlan`) may be
   retuned without a new edition (chapter 23.4), so a stream numbered by batch would change
   results. A method takes its streams as a `geom_num::rng::StreamRange`: an
   operation starts from `geom_num::rng::StreamRange::all` of its seed, and a method cuts its range with
   `geom_num::rng::StreamRange::chunk`, one chunk size per range (`geom_num::rng::StreamRange::philox` counts as chunks of
   one), giving chunk i to consumer i. A consumer that needs both a stream of its own and a
   sub-method's streams takes a chunk and cuts that. A sub-method numbers its consumers inside the
   range it is given, so nested methods never reuse a stream, and a range too small for what is
   asked is an error, never a wrap. Parallel loops use the one shared parallel helper (chapters
   21 and 23.4), so the result is the same on any number of threads, and the method joins the
   thread-count replay test.
4. **The draws are part of the version, and nothing in `geom_num::rng` changes in place.** The
   generator, its layout and each draw function's definition, and which streams a method uses, which draw
   functions it calls and in what order, all belong to the operation's version (chapters 26.2
   and 17.10: "the generator is part of the operation's version"). Changing any of them makes a
   new operation version, and the old version keeps its behaviour forever: an operation version
   keeps the numeric code it shipped with, new goldens go beside the old, and the format museum
   replays old logs (chapter 28.13). So a new generator, layout or draw definition is added
   beside the old one, under its own name, with its own golden hash and a decision note (the
   numbers layer is core, chapter 26.1), and every operation version that used the old one keeps
   it. The golden hashes in `crates/geom-num/tests/rng_determinism.rs` pin every definition in
   `geom_num::rng`, so an edit in place fails CI on every target.
5. **Honest results.** A result chosen by random sampling is never claimed to be more than its
   deterministic checks prove: a fit is Approximate, with its deviation certified (chapter
   17.10), and a randomised search that finds nothing returns an error, never a guess. A stream
   that runs out is an error (`geom_num::rng::RngError::Exhausted`), never a wrap.
6. **Tests are separate.** Test and fuzz generators may use other generators. Their seeds are
   fixed or printed (chapter 28.17), and no kernel code can reach them.

**Open question for the owner.** Chapter 28.17 says "The kernel has no randomness. Seeds belong
to test generators only", and 28.19 says the test suite never lets "a seed" decide a replayable
result. Chapters 16.3, 16.4, 17.10 and 29 instead make a seed an explicit input of RANSAC fitting
and the other randomised methods, which rule 2 follows. Read literally the two cannot both hold:
a seeded fit's result is decided by its seed. This rule reads 28.17 and 28.19 as forbidding
hidden seeds (any seed that is not a recorded input), under which "the kernel has no randomness"
still holds, because what a randomised method draws is a fixed function of its recorded inputs.
That reading is piece G32's, not an owner's decision. It is raised for chapter 31.7's open
questions, and the rule stands provisionally until it is decided.

## 9. Playbooks still to come

Chapter 26.3 asks this file for a playbook for each kind of addition. Each arrives with the piece that builds what it adds to, and is proven by a dry run:

| Playbook | Piece | Until then |
| --- | --- | --- |
| Add an operation, add a query | G117 | Chapter 26.1's table says what each must come with; chapter 28.2 lists what the completeness gate will require |
| Add a curve or surface kind | G60, with the kind contract and its test kit | Chapter 26.1 and chapter 7 |
| Add an exchange format, a representation or a domain toolkit | G480 | Chapter 26.1, and chapter 24 for formats |

## 10. When something does not fit

- **Start with "Can we".** When something looks unsupported, write how it could be done and what it would take. Only the legal rules are a hard no.
- **A check you cannot satisfy in your piece.** The map lists a known gap in `geom_test::map::GAPS` (file, rule, exact message, reason, and the piece that closes it), and a folder not yet in the tree in `geom_test::map::PENDING`; the scaffold check has its own `header_gaps` and `pending` lists in its table. A listed gap hides exactly that one problem, and fails the check once it no longer occurs. Use them only for what another piece truly owns, and name that piece.
- **A decision the chapters leave open.** Choose by the principles and the architecture, write the reasoning in the module header or the pull request, and add the question to chapter 31.7's open questions. Never change a decision in `ARCHITECTURE.md` or a chapter without a decision note an owner has accepted.
- **Out of tokens.** Commit, push, and open a draft pull request with a Handover section (root AGENTS.md, section 6).
