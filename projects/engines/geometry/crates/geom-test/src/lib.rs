//! Spare Kit geometry kernel, development only: the shared test support every kernel test uses.
//!
//! Job (chapter 28 of the kernel architecture, `projects/engines/geometry/architecture/28-testing.md`,
//! section 28.1, "Where the code lives"): `geom-test` holds the kernel's fixtures, test-data
//! generators, reference implementations, oracle clients and comparisons, and, as binaries, the CI
//! tools: the completeness gate, the golden store, the minimiser and the triage tool. Crates used as
//! outside oracles may be development dependencies of this crate only (28.10).
//!
//! Layer: Development, in chapter 27's crate table: a development crate, outside the
//! thirteen layers of the crates that ship. It never ships, and its dependencies are as needed
//! (chapter 27) within the licence allow-list of 29.2. No kernel crate may have a normal
//! dependency on it, only a development dependency (chapter 27; 28.1).
//!
//! The first piece (G11) holds the parts every later test leans on, G15 adds the map, G16 the
//! source ledger and the banned-source scan, and G19 the golden store and the determinism matrix:
//!
//! - [`seed`]: the seed discipline. Every seeded test prints `seed: <hex>` before it draws anything,
//!   and the environment variable `SPAREKIT_SEED` replays any printed seed exactly (28.5, 28.17).
//! - [`rng`]: `SplitMix64`, the small, specified, test-only generator behind every seeded test
//!   input, and [`rng::Entropy`], which lets the same test-data builders run from a seed or from a
//!   fuzzer's bytes.
//! - [`grid`]: grid coordinates and points aimed at the hard cases of 28.5.
//! - [`prop`]: proptest, run from a [`seed::Seed`], with grid strategies that draw the same cases on
//!   every target.
//! - [`deps`]: the dependency rule. No kernel crate has a normal dependency on a development crate
//!   (28.1, chapter 27), checked over `cargo metadata` by `tests/dependency_rule.rs` in CI, which
//!   also checks that CI reruns the rule whenever a manifest it reads changes.
//! - [`licence`]: whether a licence expression is permissive, and the licence records each
//!   development crate keeps in its header, which `tests/licences.rs` checks against `cargo tree`.
//! - [`map`]: the map for people and AI (26.3): `projects/engines/geometry/MAP.json`, generated
//!   from every crate and module header under `projects/engines/geometry/crates`, and the
//!   facade's under `projects/engines/crates`, by the grammar in
//!   `projects/engines/geometry/HEADERS.md`, with its `Source:` and `Consulted:` records and their
//!   licences judged by chapter 29.2's classes (29.20), every published dependency, and every
//!   kernel crate's error kinds. The binary `map` writes it (`cargo run -p geom-test --bin map --
//!   write`) and checks it in CI (`-- check`): every header complete, the map current.
//! - [`golden`]: the golden store of 28.13 (`golden/store.json`), keyed by kernel edition, golden
//!   id and operation versions, with the record of every change and the owner-approval rule for
//!   edits; the one SHA-256 every golden is hashed with (the `sha2` crate of the `RustCrypto`
//!   project); the perturbing allocator, the in-process repeat and the input-shuffle hook every
//!   golden runs under; and the runner that checks every golden on every target and build mode.
//!   The binary `golden` lists the
//!   store, checks it against the repository (`-- check`), runs every golden in one CI leg's
//!   configuration (`-- run`) and holds a pull request's edits to the approval rule (`-- diff`).
//! - [`history`]: chapter 20.3's history laws as a kit every operation's tests run (G88):
//!   one history and a chain checked through `geom-op`'s checker, composition from both ends, and
//!   proptest generators of valid histories and chains.
//! - [`canonical`]: the canonical output of one case and its SHA-256, a fixture's golden hash
//!   (28.13; G96): bodies, names, history, exactness and diagnostics, or an error's kind, entities
//!   and evidence.
//! - [`fixture`]: the stored test cases every failure ends as (28.16; G97): [`fixture::Fixture`],
//!   its [`fixture::Expect`] per edition and operation versions and its [`fixture::Status`]; the
//!   operation log in the native format; the runner's Pass, Fail and Improved; the operation-log
//!   minimiser; what a pull request may do to a stored fixture; the fails-on-base list. The binary
//!   `fixture` lists, checks and runs them.
//! - [`oracle`]: differential testing against outside oracles (28.10; G113): the catalogue, the
//!   exact-input writer, exact and bounded comparisons, signatures and the four-outcome triage, and
//!   the first oracle, geomdl, on NURBS. The binary `oracle` writes the jobs and judges the answers;
//!   every harness lives in the oracle repository.
//! - [`corpus`]: the corpus register of 28.12 (G114): every outside corpus with its source, version,
//!   SHA-256, licence and whether we may store it, the licence rules of 29.15 and 32.11, and the
//!   hash-checked fetcher. The binary `corpus` lists, checks and fetches.
//! - [`guard`]: the proof that the toolchain and dependency guard of 28.13 trips: `libm`'s `sin`
//!   changed by one bit must fail the golden `geom-num/determinism`. The binary `guard` patches
//!   that change in and runs the golden (`-- prove`); `kernel.yml` runs it.
//! - [`ledger`]: the kernel's source ledger and notices (29.20, rules 4 to 6):
//!   `projects/engines/geometry/PROVENANCE.md` and `projects/engines/geometry/NOTICE`, generated
//!   from the map's `Source:` and `Consulted:` records, the hand-kept licence texts of ported
//!   sources (`projects/engines/geometry/notices/`) and the method notes of the method-only
//!   procedure (`projects/engines/geometry/methods/`), each note held to its template. The binary
//!   `provenance` writes both files (`-- write`) and checks them in CI (`-- check`).
//! - [`recursion`]: the recursion check (piece G13; 28.8). Every recursive cycle of the crates that
//!   ship, found on a whole-program call graph that type-checks every body as rustc does, with a
//!   model of the standard library; each cycle must have its depth bound stated in its module
//!   header (the `Recursion:` field of `projects/engines/geometry/HEADERS.md`) and a test at the
//!   bound, which runs on a small WebAssembly stack. The binary `recursion` lists the cycles
//!   (`cargo run -p geom-test --bin recursion -- list`), checks them (`-- check`) and runs the
//!   bounds' tests on the small stack (`-- gate`, the merge gate's recursion job).
//! - [`scan`]: the banned-source scanner (29.20, rules 2 and 7): the routine names, macros and
//!   constants of the numerical book chapter 29.4 bans, copyleft licence notices, statements of
//!   origin that name a copyleft project, and Autodesk hatch pattern and linetype names, in every
//!   file of the kernel's crates and the facade's. The binary `source-scan` runs it in CI
//!   (`-- check`). This crate's own description must pass it too, so it names the markers only
//!   by kind; [`scan::markers`] lists them.
//!
//! Piece G21 adds the mutation and coverage gates of section 28.8, and the binary
//! `kernel-quality` that runs them in CI (`.github/workflows/kernel-quality.yml`):
//!
//! - [`foundation`]: the foundation set, the kernel crates the gates cover, and their bars.
//! - [`quality_ledger`]: the quality ledger (`projects/engines/geometry/quality-ledger.json`):
//!   every surviving mutant the kernel keeps, with why, and every `coverage(off)` mark, with its
//!   proof.
//! - [`mutants`]: judges cargo-mutants' results against the ledger.
//! - [`coverage`]: judges cargo-llvm-cov's LCOV data against the bars, and checks every
//!   `coverage(off)` mark's proof.
//! - [`rust_source`]: reads Rust source for them: test code, functions, branch sites and marks.
//! - [`miri`]: the plan of the weekly Miri run, one job per kernel test suite.
//!
//! Gate 1 (piece G43) adds:
//!
//! - [`trip`]: one planted violation for each gate whose verdict is an outside tool's (rustfmt,
//!   Verus, Kani, libFuzzer, `ClusterFuzzLite`, Gungraun, Wasmtime's fuel, Gappa with Rocq, and
//!   HOL Light). The binary `trip` plants it, runs the gate's own command and requires the tool's
//!   report of it (`cargo run -p geom-test --bin trip -- <violation> -- <command>`); each of those
//!   workflow jobs runs it.
//! - `guard prove --golden-run <arguments>`: the toolchain guard's proof through `golden run`, so
//!   the WebAssembly legs are shown to fail on a changed digest too.
//!
//! Piece G57 adds the first reference implementation of section 28.9:
//!
//! - [`nurbs_reference`]: NURBS curves in exact rationals, evaluated by de Boor's algorithm, with
//!   knot insertion, refinement and degree elevation from their blossoms, written from the
//!   definitions without reading `geom-nurbs`. `tests/nurbs_reference.rs` compares `geom-nurbs`
//!   with it on every pull request: knot vectors exactly, points and changed curves within the
//!   bounds `geom-nurbs` states.
//!
//! Piece G136 adds the reference for flat solids:
//!
//! - [`flat_reference`]: booleans, imprint, cells, plane sections, In-On-Out, volumes, centroids,
//!   clash and first contact in exact rationals, by brute force over a full plane arrangement,
//!   written from the definitions alone; `tests/flat_reference.rs` holds it to hand cases.
//!
//! The other reference implementations, oracle clients and the other CI tools arrive
//! Piece G69 adds the law catalogue of section 28.3 for NURBS and the curve and surface kinds:
//!
//! - [`fixed`]: fixed-point numbers of 400 fractional bits with π, cos, sin, exp and square roots,
//!   the higher precision of the catalogue's dense refutation sampler.
//! - [`nurbs_reference::Patch`]: NURBS surfaces in exact rationals, the tensor product of two bases.
//! - `tests/laws_nurbs.rs`, `tests/laws_curves.rs` and `tests/laws_surfaces.rs`: each law of 28.3's
//!   NURBS and kinds rows as a seeded property test, every stated bound refuted, if it is wrong, by
//!   a dense sample of exact or 400-bit values (`tests/truth`, each kind's definition written from
//!   its documentation); fixed seeds on pull requests, fresh ones nightly (`kernel-nightly.yml`).
//!
//! Fixtures, the other reference implementations, oracle clients and the other CI tools arrive
//! with the pieces that need them. The sister development crates are `geom-fuzz` (fuzz harnesses),
//! `geom-gen` (every generator whose output is committed) and `geom-bench` (benchmarks). `geom-bench` is built by
//! piece G06 in its own worktree, so this piece leaves the benchmark to it.
//!
//! Invariants this crate keeps:
//!
//! - **Development only.** No kernel crate depends on it except as a development dependency; the
//!   [`deps`] check enforces that. It may use `std` (chapter 27 exempts the development crates from
//!   `no_std`).
//! - **No randomness of its own.** The kernel has no randomness, and seeds belong to test generators
//!   only (28.17). This crate never reads the operating system's random source or a clock: a seed is
//!   a test's fixed default, a seed the test fixes, or the value of `SPAREKIT_SEED`, and every one
//!   is printed.
//! - **The same data on every target.** Test data comes from integer arithmetic only, and every
//!   word drawn from proptest's stream is a whole `u64`, so a seed gives the same inputs on every
//!   target, 32-bit and 64-bit, and only the kernel's results can differ. Golden hashes in
//!   [`grid`] and [`prop`] check it, and CI runs them natively and under wasm32-wasip1.
//! - **The kernel's lints.** `#![forbid(unsafe_code)]`, and no `unwrap`, `expect`, `panic!` or
//!   indexing outside tests. A helper fails with a typed error and the test decides.
//!
//! Sections: 21, 26, 27, 28 and 29 of the kernel architecture
//! (`projects/engines/geometry/architecture/`), in part: the determinism matrix of 21 and the
//! golden store of 28.13, keyed by the editions of 26.2; the map for people and AI of 26.3;
//! chapter 27's `geom-test` row and its rule that a development crate is never a kernel crate's
//! normal dependency; 28.1, "Where the code lives", for that rule's check; the property tests of
//! 28.2, run from a fixed seed; the generators aimed at the hard cases of 28.5; the outside oracles
//! of 28.10, development dependencies of this crate only; the seed discipline of 28.17; for the
//! development crates' own dependencies, the permissive-licence promise of 28.1's table (the
//! architecture's eleventh principle; 29.1); and, for the map, the `Source:` and `Consulted:`
//! records of 29.20, their licences judged by the classes of 29.2; for the ledger, the
//! `PROVENANCE.md`, `NOTICE` and method notes of 29.20's rules 4 to 6; and for the scanner, the
//! markers of 29.20's rules 2 and 7.
//!
//! Dependencies, every one permissive (29.1):
//!
//! - `geom-num` (MPL-2.0, ours), the numbers layer under test.
//! - `proptest` 1.12, without its default `fork` and `timeout` features, as in `geom-num`. [`prop`]
//!   always runs it from a fixed seed, so its operating-system random path is never taken.
//! - `serde_json` 1.0, to read the golden store, cargo-mutants' output and the quality ledger, and
//!   `serde` 1.0 with its derive, to read the store into typed records; `cargo_metadata` 0.23.1,
//!   `cargo metadata`'s typed model and the command that runs it (piece G12a put it in place of a
//!   field-by-field reading; 29.1).
//! - `toml` 0.9.6, so the map reads each crate's `Cargo.toml` as cargo does, and `proc-macro2`
//!   1.0.107, Rust's lexer, so it reads source as rustc does (piece G12a put both in place of
//!   readers written here; 29.1).
//! - `sha2` 0.11, the SHA-256 of every golden digest (FIPS 180-4).
//! - `rand` 0.10, without its default features, for its Fisher-Yates shuffle, which the shuffle
//!   hook drives from `SplitMix64`; it has no generator of its own here.
//! - `syn` 3.0, `proc-macro2` 1.0 (with span locations) and `quote` 1.0: Rust's grammar for the
//!   golden store's repository check and the recursion check's parser, their line numbers and the
//!   recursion check's token printing.
//! - `petgraph` 0.8, without default features: the recursion check's strongly connected
//!   components (Tarjan's algorithm).
//! - `pulldown-cmark` 0.13.5, a development dependency without its default features (the HTML
//!   writer and the command line), so `tests/attack_g17.rs` reads the kernel's README.md,
//!   AGENTS.md and GLOSSARY.md as Markdown.
//! - `unicode-normalization` 0.1.26, a development dependency, so the same test sorts the
//!   glossary's headwords with their accents set aside.
//! - `num-rational` 0.4, a development dependency only: the exact oracle of the scanner's cited
//!   Cash-Karp fixture, whose tableau its tests check against the order conditions in rational
//!   arithmetic.
//! - `clap` 4.6 (G21), the command line of the `kernel-quality` binary, without colour or terminal
//!   detection.
//! - `lcov` 0.8 (G21), to read the LCOV tracefiles cargo-llvm-cov writes ([`coverage`]).
//! - `regex-lite` 0.1.9, the regular expressions of a full mutation run narrowed to some files
//!   ([`mutants::check_narrowed`]), in the syntax of cargo-mutants' `--re`.
//! - `yaml-rust2` 0.13, a development dependency (G21), which reads
//!   `.github/workflows/kernel-quality.yml` as YAML.
//! - `dashu-int` 0.6 and `dashu-ratio` 0.6 (G57), without their default features: the exact
//!   rationals of [`nurbs_reference`] and the fixed point of [`fixed`] (G69).
//! - `geom-nurbs` (MPL-2.0, ours): the curves `tests/nurbs_reference.rs` (G57) compares with the
//!   reference, and `oracle::geomdl` (G113) with the first outside oracle.
//! - `geom-format` and `geom-run` (MPL-2.0, ours), dependencies (G97): a fixture's operation log is
//!   a standalone native file, and the meter its readers take.
//! - `geom-curve` and `geom-surface` (MPL-2.0, ours), dependencies: the kinds and transforms the
//!   law catalogue checks (G69), and what [`ssi`] builds a corpus case from (G160).
//! - `geom-intersect` (MPL-2.0, ours), a dependency (G160): the result type [`ssi`] reads an answer
//!   from.
//!
//! Licences of every outside package this crate brings into a build, its own and its dependencies'
//! on every target (`cargo tree --target all`), each record checked by `tests/licences.rs`. Where a
//! package offers a choice, it is taken under the permissive licences offered (MIT or Apache-2.0):
//!
//! - MIT OR Apache-2.0: `proptest`, `bit-set`, `bit-vec`, `bitflags`, `chacha20`, `cfg-if`,
//!   `cpufeatures`, `libc`, `rand`, `rand_core`, `rand_xorshift`, `getrandom`, `core_detect`,
//!   `num-traits`, `autocfg`, `regex-syntax`, `unarray`, `serde_json`, `serde`, `serde_core`,
//!   `serde_derive`, `itoa`, `proc-macro2`, `quote`, `syn`, `toml`, `toml_datetime`, `toml_parser`,
//!   `toml_writer`, `serde_spanned`, `camino`, `cargo-platform`, `semver`, `thiserror`,
//!   `thiserror-impl`, `clap`, `clap_builder`, `clap_derive`, `clap_lex`, `anstyle`, `heck`,
//!   `yaml-rust2`, `hashlink`, `hex`, `wait-timeout`, `sha2`, `digest`, `block-buffer`, `crypto-common`, `hybrid-array`,
//!   `typenum`, `petgraph`, `fixedbitset`, `hashbrown`, `indexmap`, `equivalent`, `num-rational`,
//!   `num-bigint`, `num-integer`, `dashu-int`, `dashu-ratio`, `dashu-base`, `regex-lite` and
//!   `static_assertions`.
//! - Apache-2.0: `num-modular`, which `dashu-int` brings in.
//! - MIT/Apache-2.0: `lcov` and `arraydeque` (the older spelling of MIT OR Apache-2.0).
//! - MIT OR Apache-2.0 OR LGPL-2.1-or-later: `r-efi` (UEFI targets only).
//! - (MIT OR Apache-2.0) AND Unicode-3.0: `unicode-ident` and `unicode-normalization`.
//! - Zlib OR Apache-2.0 OR MIT: `tinyvec`, which `unicode-normalization` brings in.
//! - MIT OR Apache-2.0: `unicase`, which `pulldown-cmark` brings in.
//! - Unlicense OR MIT: `memchr`.
//! - MIT: `zmij`, `winnow`, `cargo_metadata` and `pulldown-cmark`.
//! - Zlib: `foldhash`.
#![forbid(unsafe_code)]

pub mod canonical;
pub mod corpus;
pub mod coverage;
pub mod deps;
pub mod fixed;
pub mod fixture;
pub mod flat_reference;
pub mod foundation;
pub mod golden;
pub mod grid;
pub mod guard;
pub mod history;
pub mod ledger;
pub mod licence;
pub mod map;
pub mod miri;
pub mod mutants;
pub mod nurbs_reference;
pub mod oracle;
pub mod prop;
pub mod quality_ledger;
pub mod recursion;
pub mod rng;
pub mod rust_source;
pub mod scan;
pub mod seed;
pub mod ssi;
pub mod trip;

pub use rng::{ByteSource, Entropy, SplitMix64};
pub use seed::Seed;
