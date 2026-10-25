//! Spare Kit geometry kernel, development only: the shared test support every kernel test uses.
//!
//! Job (chapter 28 of the kernel architecture, `projects/engines/geometry/architecture/28-testing.md`,
//! section 28.1, "Where the code lives"): `geom-test` holds the kernel's fixtures, test-data
//! generators, reference implementations, oracle clients and comparisons, and, as binaries, the CI
//! tools: the completeness gate, the golden store, the minimiser and the triage tool. Crates used as
//! outside oracles may be development dependencies of this crate only (28.10).
//!
//! Layer: Development, in chapter 27's proposed crate table: a development crate, outside the
//! thirteen layers of the crates that ship. It never ships, and its dependencies are as needed
//! (chapter 27) within the licence allow-list of 29.2. No kernel crate may have a normal
//! dependency on it, only a development dependency (chapter 27; 28.1).
//!
//! The first piece (G11) holds the parts every later test leans on, G15 adds the map, and G16 the
//! source ledger and the banned-source scan:
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
//! - [`ledger`]: the kernel's source ledger and notices (29.20, rules 4 to 6):
//!   `projects/engines/geometry/PROVENANCE.md` and `projects/engines/geometry/NOTICE`, generated
//!   from the map's `Source:` and `Consulted:` records, the hand-kept licence texts of ported
//!   sources (`projects/engines/geometry/notices/`) and the method notes of the method-only
//!   procedure (`projects/engines/geometry/methods/`), each note held to its template. The binary
//!   `provenance` writes both files (`-- write`) and checks them in CI (`-- check`).
//! - [`scan`]: the banned-source scanner (29.20, rules 2 and 7): the routine names, macros and
//!   constants of the numerical book chapter 29.4 bans, copyleft licence notices, statements of
//!   origin that name a copyleft project, and Autodesk hatch pattern and linetype names, in every
//!   file of the kernel's crates and the facade's. The binary `source-scan` runs it in CI
//!   (`-- check`). This crate's own description must pass it too, so it names the markers only
//!   by kind; [`scan::markers`] lists them.
//!
//! Fixtures, reference implementations, oracle clients and the CI tools arrive with the pieces that
//! need them. The sister development crates are `geom-fuzz` (fuzz harnesses), `geom-gen` (every
//! generator whose output is committed) and `geom-bench` (benchmarks). `geom-bench` is built by
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
//! Sections: 26, 27, 28 and 29 of the kernel architecture
//! (`projects/engines/geometry/architecture/`), in part: the map for people and AI of 26.3;
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
//! - `serde_json` 1.0, to read `cargo metadata`, and `cargo_metadata` 0.23.1, its typed model and
//!   the command that runs it (piece G12a put it in place of a field-by-field reading; 29.1).
//! - `toml` 0.9.6, so the map reads each crate's `Cargo.toml` as cargo does, and `proc-macro2`
//!   1.0.107, Rust's lexer, so it reads source as rustc does (piece G12a put both in place of
//!   readers written here; 29.1).
//! - `pulldown-cmark` 0.13.5, a development dependency without its default features (the HTML
//!   writer and the command line), so `tests/attack_g17.rs` reads the kernel's README.md,
//!   AGENTS.md and GLOSSARY.md as Markdown.
//! - `unicode-normalization` 0.1.26, a development dependency, so the same test sorts the
//!   glossary's headwords with their accents set aside.
//! - `num-rational` 0.4, a development dependency only: the exact oracle of the scanner's cited
//!   Cash-Karp fixture, whose tableau its tests check against the order conditions in rational
//!   arithmetic.
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
//!   `thiserror-impl`, `num-rational`, `num-bigint` and `num-integer`.
//! - MIT OR Apache-2.0 OR LGPL-2.1-or-later: `r-efi` (UEFI targets only).
//! - (MIT OR Apache-2.0) AND Unicode-3.0: `unicode-ident` and `unicode-normalization`.
//! - Zlib OR Apache-2.0 OR MIT: `tinyvec`, which `unicode-normalization` brings in.
//! - MIT OR Apache-2.0: `unicase`, which `pulldown-cmark` brings in.
//! - Unlicense OR MIT: `memchr`.
//! - MIT: `zmij`, `winnow`, `cargo_metadata` and `pulldown-cmark`.
#![forbid(unsafe_code)]

pub mod deps;
pub mod grid;
pub mod ledger;
pub mod licence;
pub mod map;
pub mod prop;
pub mod rng;
pub mod scan;
pub mod seed;

pub use rng::{ByteSource, Entropy, SplitMix64};
pub use seed::Seed;
