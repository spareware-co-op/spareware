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
//! Layer: Development, in chapter 27's proposed crate table: a development crate, outside the
//! thirteen layers of the crates that ship. It never ships, and its dependencies are as needed
//! (chapter 27) within the licence allow-list of 29.2. No kernel crate depends on it, by chapter
//! 27's rule for `geom-gen` above, which is stricter than the development layer's rule that no
//! kernel crate has a normal dependency on a development crate (chapter 27; 28.1).
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
//! Generators today:
//!
//! - [`grid_steps`], the exact table of the grid steps, common units on each step and the
//!   tolerance ladder, from `geom-num`'s own constants, written to
//!   `projects/engines/geometry/crates/geom-gen/generated/grid-steps.md`;
//! - [`predicates`], the predicate generator of chapter 6.2: from a description of each predicate
//!   on grid points and on rounded inputs, `f64` copies of wider integers ([`predicate_lang`]),
//!   it derives, exactly, the bit-range certificate of every intermediate, the exact path's
//!   integer widths and the `f64` filter's error constant, input rounding included
//!   ([`predicate_derive`], Meyer and Pion's method), and writes the filter, the width-aware
//!   exact path and the certificate into `geom-num`'s `src/generated/` ([`predicate_emit`]). An
//!   independent checker in `tests/predicate_checker.rs` re-derives the certificate and the
//!   constant and reads every line of the committed code.
//! - [`quadrature`], the Gauss–Legendre and Gauss–Kronrod rules `geom-num` integrates with,
//!   computed in exact rationals (Golub and Welsch's method for the Gauss rules, Kronrod's
//!   definition for his extensions) and each node and weight rounded once, correctly, into
//!   `geom-num`'s `src/quadrature_rules/`.
//!
//! Invariants this crate keeps:
//!
//! - **Pure generators.** A generator reads nothing (no files, no environment, no clock) and gives
//!   the same bytes on every target, so `check` means the same everywhere; `tests/purity.rs` checks
//!   the generator modules' source, and each generator's unit test compares its output with the
//!   committed file on every target CI runs, wasm32 included. A generator's seed is fixed in the
//!   registry; `SPAREKIT_SEED` never applies to generators, whose outputs are committed and must
//!   reproduce.
//! - **Exact.** Generators compute with integers and exact fractions ([`exact`], and
//!   [`bignum`]'s naturals and polynomials in the unit roundoff), never with rounded floating
//!   point, unless an output is defined as rounded (the filter constant, rounded up once).
//! - **Bytes, exactly.** Outputs use `\n` line ends, and each generated folder's `.gitattributes`
//!   turns off line-ending conversion for them, so a Windows checkout compares the committed bytes.
//! - **Development only**, so it may use `std` (chapter 27); `#![forbid(unsafe_code)]`; no
//!   `unwrap`, `expect`, `panic!` or indexing outside tests.
//!
//! Sections: chapter 27's proposed rule "Generated code and data have one home: `geom-gen`" and its
//! `geom-gen` row, in the kernel architecture (`projects/engines/geometry/architecture/`), with the
//! seed discipline of 28.5 and 28.17 for its generators; and, for the first generator, the grid
//! steps of 6.1 and the tolerance ladder of 6.5, tabulated from `geom-num`'s constants.
//!
//! Dependencies: `geom-num` (MPL-2.0, ours), whose constants the generators tabulate, and
//! `geom-test` (MPL-2.0, ours), for the seed type and `SplitMix64`. Their own dependencies are
//! recorded in their headers. Its tests also use `proc-macro2` 1.0 (a development dependency,
//! without its default `proc-macro` feature), to compare the Verus twin's tokens with the shipped
//! code's; `geom-test` already brings it.
//!
//! Licences of every outside package this crate brings into a build itself, on every target
//! (`cargo tree --target all`), each record checked by `geom-test`'s `tests/licences.rs`: the
//! exact big integers and rationals of the quadrature-rules generator (`dashu-int`, `dashu-ratio`
//! and what they bring), and its tests' `proc-macro2` and what that brings. Every other outside
//! package in its build comes through `geom-num` or `geom-test`. Where a package offers a choice,
//! it is taken under the permissive licences offered (MIT or Apache-2.0):
//!
//! - MIT OR Apache-2.0: `cfg-if`, `dashu-base`, `dashu-int`, `dashu-ratio`, `proc-macro2`,
//!   `static_assertions`.
//! - Apache-2.0: `num-modular`.
//! - (MIT OR Apache-2.0) AND Unicode-3.0: `unicode-ident`.
#![forbid(unsafe_code)]

pub mod bignum;
pub mod exact;
pub mod files;
pub mod generator;
pub mod grid_steps;
pub mod predicate_derive;
pub mod predicate_emit;
pub mod predicate_lang;
pub mod predicates;
pub mod quadrature;
pub mod registry;

pub use files::{Folder, Registry};
pub use generator::{GenError, Generate, Generator};
