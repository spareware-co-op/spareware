//! Spare Kit geometry kernel, development only: the fuzz harnesses.
//!
//! Job (chapter 28 of the kernel architecture, `projects/engines/geometry/architecture/28-testing.md`,
//! section 28.5): every operation, every reader and the operation-sequence generator get a fuzz
//! target here. Fuzzing feeds a program huge numbers of machine-made inputs, steered towards code
//! not yet reached, and watches for anything wrong: a panic, a broken law, an invalid result, a
//! changed input. This first piece (G11) has one target, [`predicates`]: `geom-num`'s exact
//! predicates never panic, and swapping two points flips their sign, on arbitrary grid points.
//!
//! How a harness is built (28.5, "Tooling"): each harness is a plain function from bytes to
//! `Result<(), Finding>`, here in the library, so one function serves every engine:
//!
//! - **bolero's test mode**, on stable Rust, on Linux, Windows and macOS: `tests/predicates.rs`
//!   wraps the harness in `bolero::check!`, and `cargo test -p geom-fuzz` runs it. bolero first
//!   replays every file in the target's `corpus` and `crashes` folders (below), so a found crash
//!   stays a test forever (28.16), then runs a fixed number of random inputs. bolero 0.13 draws one
//!   seed per random input from the operating system's random source (`bolero/src/test/mod.rs`,
//!   `rng_tests`), prints the seed of a failing input as `BOLERO_RANDOM_SEED=<seed>`, and replays
//!   exactly that input when the variable is set. It prints no seed for an input that passes, and
//!   offers no way to choose them, so this leg is the "short fuzz" of 28.17: two runs of the same
//!   code may try different inputs, and every failure it finds replays. Under WebAssembly
//!   (wasm32-wasip1) this leg is ignored: bolero reads those folders from the host and catches
//!   panics, and WASI allows neither.
//! - **The seeded leg** ([`seeded::run`]), on stable Rust, on every target, WebAssembly included:
//!   the same harness on byte strings from `geom-test`'s `SplitMix64`, from a seed printed as
//!   `seed: <hex>` and replayed by `SPAREKIT_SEED`. Pull requests run it from the fixed default
//!   seed, so the same cases run every time on every target (28.17); a nightly run (a later
//!   piece) that sets `SPAREKIT_SEED` to a fresh value prints it the same way.
//! - **libFuzzer, coverage-guided, with cargo-bolero, on Linux** (nightly fuzzing; not run by the
//!   pull-request workflow). Once per machine:
//!
//!   ```text
//!   sudo apt install binutils-dev libunwind-dev
//!   cargo install -f cargo-bolero
//!   ```
//!
//!   then, from the repository root, with a nightly toolchain installed (`rustup toolchain install
//!   nightly`), because the sanitizer flags cargo-bolero passes are nightly-only:
//!
//!   ```text
//!   cargo bolero test predicates_fuzz --package geom-fuzz --toolchain nightly --engine libfuzzer -T 10min
//!   ```
//!
//!   The test name is the `#[test]` function's; `-T` bounds the run (any `humantime` duration);
//!   `--runs`, `--jobs` and `--max-input-length` are the other limits; `--sanitizer` defaults to
//!   `address`. Options are those of cargo-bolero 0.13 (`bin/cargo-bolero/src/project.rs` and
//!   `test.rs` at commit 7642943, read 2026-10-21). libFuzzer's corpus and crashes are kept in
//!   `tests/__fuzz__/predicates/corpus` and `tests/__fuzz__/predicates/crashes`, which the test
//!   mode above replays on every `cargo test`. The same command with `--engine afl`,
//!   `--engine honggfuzz` or `--engine kani` runs the other engines bolero supports.
//!
//! What is not here yet: 28.5 also asks for a thin cargo-fuzz wrapper per harness, because
//! `ClusterFuzzLite` expects the cargo-fuzz layout; it belongs with the piece that sets up
//! pull-request fuzzing, and calls these same functions. Targets for operations, readers and
//! operation sequences arrive with them.
//!
//! Rules this crate keeps: development only (no kernel crate depends on it; `geom-test`'s
//! dependency rule checks that), so it may use `std` (chapter 27); `#![forbid(unsafe_code)]`; no
//! `unwrap`, `expect`, `panic!` or indexing outside tests, so a harness reports a law it found
//! broken as a [`Finding`] and only the kernel code under test can panic; no randomness of its own
//! (seeds come from `geom-test` and are printed; bolero's come from bolero, as above).
//!
//! Dependencies, every one permissive (29.1):
//!
//! - `geom-num` (MPL-2.0, ours), the code under test, and `geom-test` (MPL-2.0, ours), for seeds,
//!   `SplitMix64` and the hard-case grid sampler. Their own dependencies are recorded in their
//!   headers.
//! - `bolero` 0.13.7, a development dependency only: the `check!` front end, with its test-mode
//!   engine and, under cargo-bolero, the engine chosen (`bolero-libfuzzer`, `bolero-afl`,
//!   `bolero-honggfuzz` or `bolero-kani`).
//!
//! Licences of every outside package this crate brings into a build, through `bolero`, on every
//! target (`cargo tree --target all`), each record checked by `geom-test`'s `tests/licences.rs`.
//! Where a package offers a choice, it is taken under the permissive licences offered (MIT or
//! Apache-2.0):
//!
//! - MIT: `bolero`, `bolero-engine`, `bolero-generator`, `bolero-generator-derive`,
//!   `bolero-libfuzzer`, `bolero-afl`, `bolero-honggfuzz`, `bolero-kani`, `pretty-hex` and `winnow`.
//! - MIT OR Apache-2.0: `anyhow`, `cfg-if`, `lazy_static`, `either`, `rand`, `rand_core`,
//!   `rand_chacha`, `rand_xoshiro`, `getrandom`, `libc`, `ppv-lite86`, `proc-macro-crate`,
//!   `toml_edit`, `toml_datetime`, `indexmap`, `hashbrown`, `equivalent`, `serde`, `serde_core`,
//!   `serde_derive`, `proc-macro2`, `quote`, `syn`, `cc`, `find-msvc-tools` and `shlex`.
//! - (MIT OR Apache-2.0) AND Unicode-3.0: `unicode-ident`.
//! - BSD-2-Clause OR Apache-2.0 OR MIT: `zerocopy` and `zerocopy-derive`.
//! - Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT: `wasip2` and `wit-bindgen` (WASI targets
//!   only).
//! - MIT OR Apache-2.0 OR LGPL-2.1-or-later: `r-efi` (UEFI targets only).
#![forbid(unsafe_code)]

use core::fmt;

pub mod predicates;
pub mod seeded;

/// A law a harness found broken, with the input that broke it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// The law, in words, for example "orient2d(b, a, c) = −orient2d(a, b, c)".
    pub law: String,
    /// The decoded input and the values that broke the law.
    pub detail: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "broken law: {}\n{}", self.law, self.detail)
    }
}

impl std::error::Error for Finding {}

/// A harness: a plain function from a fuzzer's bytes to a verdict, the shape every engine drives.
pub type Harness = fn(&[u8]) -> Result<(), Finding>;
