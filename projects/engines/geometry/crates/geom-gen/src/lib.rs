//! Spare Kit geometry kernel, development only: the home of every generator whose output is
//! committed, and the check that each output is current.
//!
//! Job (chapter 27 of the kernel architecture, `projects/engines/geometry/architecture/27-crates-folders.md`,
//! proposed rule "Generated code and data have one home: `geom-gen`"): several chapters generate
//! code or data that is checked in (predicate filters and their certificates, subdivision tables,
//! EXPRESS tables, the native format's field tables, `MAP.json`, `PROVENANCE.md` and `NOTICE`). One
//! development-only crate holds every such generator. Each output is committed into the crate or
//! folder that uses it, and CI reruns every generator and fails on any difference. No kernel crate
//! depends on this crate (`geom-test`'s dependency rule checks that), so whatever a generator needs,
//! a multiprecision library or a proof tool, never enters a kernel build.
//!
//! How it works:
//!
//! - [`generator::Generator`]: one generator: a name, the one output file it owns (a path relative
//!   to the workspace root), and a pure function producing the file's bytes. A generator that uses
//!   randomness takes an explicit seed, fixed in the registry, and draws only from `geom-test`'s
//!   `SplitMix64` seeded with it; one that uses none says so ([`generator::Generate`]).
//! - [`registry::REGISTRY`]: every generator, and every generated folder: a folder only generators
//!   write into, where any file no generator owns is an orphan.
//! - [`files`]: `validate` checks the registry's rules (portable output names that are one file on
//!   every system, every output in a generated folder); `write` writes every output; `check`
//!   regenerates every output in memory and reports every file that differs from its generator, is
//!   missing, or is an orphan.
//! - The binary: `cargo run -p geom-gen -- write`, `cargo run -p geom-gen -- check` (exits 1 and
//!   names every file that differs, is missing or is an orphan) and `cargo run -p geom-gen -- list`.
//!   Each prints every generator's seed first, `seed: <hex>`, or `seed: none` for a generator that
//!   uses no randomness. `--root <dir>` points it at another copy of the workspace.
//!
//! Generators today: [`grid_steps`], the exact table of the grid steps, common units on each step
//! and the tolerance ladder, from `geom-num`'s own constants, written to
//! `projects/engines/geometry/crates/geom-gen/generated/grid-steps.md`.
//!
//! Rules this crate keeps:
//!
//! - **Pure generators.** A generator reads nothing (no files, no environment, no clock) and gives
//!   the same bytes on every target, so `check` means the same everywhere; `tests/purity.rs` checks
//!   the generator modules' source, and each generator's unit test compares its output with the
//!   committed file on every target CI runs, wasm32 included. A generator's seed is fixed in the
//!   registry; `SPAREKIT_SEED` never applies to generators, whose outputs are committed and must
//!   reproduce.
//! - **Exact.** Generators compute with integers and exact fractions ([`exact`]), never with
//!   rounded floating point, unless an output is defined as rounded.
//! - **Bytes, exactly.** Outputs use `\n` line ends, and `generated/.gitattributes` turns off line-
//!   ending conversion for them, so a Windows checkout compares the committed bytes.
//! - **Development only**, so it may use `std` (chapter 27); `#![forbid(unsafe_code)]`; no
//!   `unwrap`, `expect`, `panic!` or indexing outside tests.
//!
//! Dependencies: `geom-num` (MPL-2.0, ours), whose constants the generators tabulate, and
//! `geom-test` (MPL-2.0, ours), for the seed type and `SplitMix64`. Their own dependencies are
//! recorded in their headers.
//!
//! Licences of every outside package this crate brings into a build itself, checked by
//! `geom-test`'s `tests/licences.rs`: none. Every outside package in its build comes through
//! `geom-num` or `geom-test`.
#![forbid(unsafe_code)]

pub mod exact;
pub mod files;
pub mod generator;
pub mod grid_steps;
pub mod registry;

pub use files::{Folder, Registry};
pub use generator::{GenError, Generate, Generator};
