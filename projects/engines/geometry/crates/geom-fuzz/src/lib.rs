//! Spare Kit geometry kernel, development only: the fuzz harnesses.
//!
//! Job (chapter 28 of the kernel architecture, `projects/engines/geometry/architecture/28-testing.md`,
//! section 28.5): every operation, every reader and the operation-sequence generator get a fuzz
//! target here. Fuzzing feeds a program huge numbers of machine-made inputs, steered towards code
//! not yet reached, and watches for anything wrong: a panic, a broken law, an invalid result, a
//! changed input. It has fifteen targets so far: [`predicates`] (G11), `geom-num`'s exact
//! predicates never panic and swapping two points flips their sign, on arbitrary grid points;
//! [`orient2d`] (G14), orient2d, its exact path and its `f64` filter agree with the exact
//! determinant of orient2d's definition, computed from scratch, which is also proven by Kani on a
//! bounded domain; [`wide`] (G26), the wide integers agree with `num-bigint` on every
//! operation at every width; [`variants`] (G42), every variant of every generated exact path
//! returns the exact value of its description, evaluated in big integers; [`cbor`] (G91),
//! `geom-format`'s strict CBOR decoder and its text form never panic, refuse inside their input
//! and convert both ways bit for bit; [`cache`] (G105), `geom-format`'s memo store keeps to its
//! bound, forgets the entry used longest ago first and answers only where the caller's budget
//! covers the stored run; [`arenas`] (G80), `geom-topo` publishes exactly the arenas its rules
//! allow, and a body's versions never see each other's edits; [`make`] (G101), the primitives
//! and transforms return only what the full checker passes, keep their laws, and refuse with a
//! typed error; [`sequence`] (G116), whole operation logs replay to the same hash and keep
//! every law at every step; [`intersect`] (G158), an intersection's items keep their invariants
//! against an all-pairs oracle and a pair asked either way round makes one call; [`quadric`]
//! (G159), every exact decision on a line and a quadric agrees with big integers, and a Sturm
//! chain counts the roots a polynomial was built from; [`project`] (G162), a nearest point is
//! never beaten by a sample of the kind; [`crossings`] (G163), every point of any two curves lies
//! on both, and two cubic graphs that change sides have an item between them; [`piercing`] (G164),
//! every point of a curve and a surface lies on both; and [`natural`] (G165), every curve two
//! natural quadrics are said to share lies on each by its own equation.
//! [`adversarial`] holds the generators aimed at chapter 28.5's hard
//! cases, and [`robustness`] counts how many plain designs succeed. The layout every target
//! follows, and how each engine runs it locally and in CI,
//! is in this crate's `README.md`.
//!
//! Layer: Development, in chapter 27's crate table: a development crate, outside the
//! thirteen layers of the crates that ship. It never ships, and its dependencies are as needed
//! (chapter 27) within the licence allow-list of 29.2. No kernel crate may have a normal
//! dependency on it, only a development dependency (chapter 27; 28.1).
//!
//! How a target is built (28.5, "Tooling"; the README gives the layout in full): each target module
//! has a `check` function, the law and its oracle on a decoded case, and a `harness`, a plain
//! function from bytes to `Result<(), Finding>` that decodes the bytes and calls `check`. The
//! harness is what every fuzzing engine drives, so their corpus and crash files are interchangeable;
//! a bounded Kani proof drives `check` itself:
//!
//! - **bolero's test mode**, on stable Rust, on Linux, Windows and macOS: `tests/<target>.rs`
//!   wraps the harness in `bolero::check!` as `<target>_fuzz`, and `cargo test -p geom-fuzz` runs
//!   it. bolero first
//!   replays every file in the target's `corpus` and `crashes` folders (below), so a found crash
//!   stays a test forever (28.16), then runs a fixed number of random inputs. bolero 0.13 draws one
//!   seed per random input from the operating system's random source (`lib/bolero/src/test/mod.rs`,
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
//! - **libFuzzer, coverage-guided, with cargo-bolero, on Linux.** cargo-bolero 0.13.5 rebuilds
//!   the same test with libFuzzer, its sanitizer and coverage flags, which need a nightly
//!   compiler, under the workspace's `fuzz` profile (release optimisation with debug assertions
//!   and overflow checks on; root `Cargo.toml`). From the repository root:
//!
//!   ```text
//!   cargo install cargo-bolero --version 0.13.5 --locked --no-default-features --features libfuzzer
//!   rustup toolchain install nightly-2026-10-01 --profile minimal
//!   cargo bolero test orient2d_fuzz --package geom-fuzz --toolchain nightly-2026-10-01 --engine libfuzzer -T 10min
//!   ```
//!
//!   The test name is the `#[test]` function's; `-T` bounds the run (any `humantime` duration);
//!   `--runs`, `--jobs` and `--max-input-length` are the other limits; `--sanitizer` defaults to
//!   `address`. Options are those of cargo-bolero 0.13 (`bin/cargo-bolero/src/project.rs` and
//!   `test.rs`, credited below). libFuzzer keeps its corpus and crashes in
//!   `tests/__fuzz__/<target>/corpus` and `crashes`, which the test mode above replays on every
//!   `cargo test`. Every pull request that touches the kernel runs this leg for a minute per
//!   target (`.github/workflows/kernel-fuzz.yml`).
//! - **`ClusterFuzzLite`, on every pull request.** `.clusterfuzzlite/build.sh` builds the same
//!   tests with cargo-bolero's `build-clusterfuzz` command, which packs the libFuzzer-built test
//!   binary with one start-up script per bolero test; the build keeps the scripts of the
//!   `<target>_fuzz` tests, and `ClusterFuzzLite` fuzzes each target in turn
//!   (`kernel-fuzz.yml`). So `ClusterFuzzLite` runs the very binary cargo-bolero runs, and no
//!   cargo-fuzz wrapper is needed (the README gives the evidence and the one check this replaces).
//! - **Kani, bounded proofs, on Linux.** A target's `<target>_proof` test wraps `check` in
//!   `bolero::check!` over a generator of the domain to be proven and is marked `kani::proof`
//!   under `cfg(kani)`; Kani 0.68.0 makes every generated value symbolic and proves the law for
//!   all of them: `cargo kani --tests -p geom-fuzz --harness orient2d_proof --exact`, which is
//!   what `cargo bolero test orient2d_proof --engine kani` runs. Under `cargo test` the same test
//!   is a property test over that domain.
//!
//! Targets for operations, readers and operation sequences arrive with them.
//!
//! Invariants this crate keeps:
//!
//! - **Development only.** No kernel crate depends on it (`geom-test`'s dependency rule checks
//!   that), so it may use `std` (chapter 27).
//! - **The kernel's lints: no unsafe code, and no panics of its own.** `#![forbid(unsafe_code)]`,
//!   and no `unwrap`, `expect`, `panic!` or indexing outside tests, so a harness reports a law it
//!   found broken as a [`Finding`] and only the kernel code under test can panic.
//! - **No randomness of its own.** Seeds come from `geom-test` and are printed; bolero's come from
//!   bolero, as above.
//!
//! Sections: chapter 27's `geom-fuzz` row and its rules for the development crates, and 28.5 of
//! the kernel architecture (`projects/engines/geometry/architecture/`), in part: its tooling (one
//! plain harness function, bolero's `check!`, libFuzzer under cargo-bolero, and `ClusterFuzzLite`
//! on pull requests) and two targets, on the swap law and the exact oracle of 28.6; a bounded Kani
//! proof of 28.7, on bolero's harness unchanged; the seeded leg 28.2 asks for; the replay of every
//! stored crash, 28.16; and the short fuzz and fixed seeds of 28.17.
//!
//! Method: [`Finding::new`] leaves a finding's detail unwritten under Kani, as Kani's own `std`
//! leaves the messages of `assert!` and `panic!` unformatted during verification
//! (<https://github.com/model-checking/kani/blob/0d2328a93f0e0ff66132d6bfa1a7d884877cf862/library/std/src/lib.rs>,
//! credited below): Kani reports a failed proof with a trace of every value, and formatting symbolic values
//! would only enlarge the problem it solves. The rest of this root is the [`Finding`] type, its
//! `Display` and `Error` implementations and the [`Harness`] alias, whose shape 28.5 gives. Each
//! harness module names the method it follows.
//!
//! Cost: none at run time for the alias; [`Finding::new`] runs its detail closure once, except
//! under Kani; displaying a [`Finding`] writes its two strings.
//!
//! Dependencies, every one permissive (29.1):
//!
//! - `geom-num` (MPL-2.0, ours), the code under test, and `geom-test` (MPL-2.0, ours), for seeds,
//!   `SplitMix64` and the hard-case grid sampler. Their own dependencies are recorded in their
//!   headers.
//! - `bolero` 0.13.7, a development dependency only: the `check!` front end, with its test-mode
//!   engine and, under cargo-bolero, the engine chosen (`bolero-libfuzzer`, `bolero-afl`,
//!   `bolero-honggfuzz` or `bolero-kani`).
//! - `geom-gen` (MPL-2.0, ours, development only), whose description parser the [`variants`]
//!   target reads the predicates with.
//! - `geom-format` and `geom-run` (MPL-2.0, ours), the code under test of the [`cbor`] target and
//!   the work meter it reads with, and of the [`cache`] target with `geom-op`.
//! - `geom-topo`, `geom-curve`, `geom-surface`, `geom-math` and `geom-run` (MPL-2.0, ours), the code
//!   under test of the [`arenas`] target and the values that fill its arenas.
//! - `geom-make`, `geom-check`, `geom-math`, `geom-op`, `geom-run` and `geom-topo` (MPL-2.0, ours),
//!   the code under test of the [`make`] target and the checker and values it judges them by.
//! - `geom-intersect` (MPL-2.0, ours), the code the [`intersect`] target tests, and `geom-curve`,
//!   `geom-math`, `geom-run` and `geom-surface` (MPL-2.0, ours), what its cases are made of.
//! - `num-bigint` 0.4.8 (MIT OR Apache-2.0), the oracle of the [`wide`] and [`variants`] targets:
//!   exact arbitrary-precision integers, the big-integer oracle chapter 29.3 names, sharing no
//!   code with `geom-num`.
//!
//! Licences of every outside package this crate brings into a build, through `bolero` and
//! `num-bigint`, on every target (`cargo tree --target all`), each record checked by `geom-test`'s
//! `tests/licences.rs`.
//! Where a package offers a choice, it is taken under the permissive licences offered (MIT or
//! Apache-2.0):
//!
//! - MIT: `bolero`, `bolero-engine`, `bolero-generator`, `bolero-generator-derive`,
//!   `bolero-libfuzzer`, `bolero-afl`, `bolero-honggfuzz`, `bolero-kani`, `pretty-hex` and `winnow`.
//! - MIT OR Apache-2.0: `anyhow`, `cfg-if`, `lazy_static`, `either`, `rand`, `rand_core`,
//!   `rand_chacha`, `rand_xoshiro`, `getrandom`, `libc`, `ppv-lite86`, `proc-macro-crate`,
//!   `toml_edit`, `toml_datetime`, `indexmap`, `hashbrown`, `equivalent`, `serde`, `serde_core`,
//!   `serde_derive`, `proc-macro2`, `quote`, `syn`, `cc`, `find-msvc-tools`, `shlex`,
//!   `num-bigint`, `num-integer`, `num-traits` and `autocfg`.
//! - (MIT OR Apache-2.0) AND Unicode-3.0: `unicode-ident`.
//! - BSD-2-Clause OR Apache-2.0 OR MIT: `zerocopy` and `zerocopy-derive`.
//! - Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT: `wasip2` and `wit-bindgen` (WASI targets
//!   only).
//! - MIT OR Apache-2.0 OR LGPL-2.1-or-later: `r-efi` (UEFI targets only).
//!
//! Consulted: bolero 0.13.7 `lib/bolero/src/test/mod.rs`, MIT,
//!   <https://github.com/camshaft/bolero/blob/7642943b6c1f8a9b5e824dd1cd29b7c8e56068b8/lib/bolero/src/test/mod.rs>,
//!   for how its test mode draws a seed per input, prints a failing input's seed and replays it,
//!   as described above. No code copied.
//! Consulted: cargo-bolero 0.13.5 `bin/cargo-bolero/src/project.rs`, MIT,
//!   <https://github.com/camshaft/bolero/blob/7642943b6c1f8a9b5e824dd1cd29b7c8e56068b8/bin/cargo-bolero/src/project.rs>,
//!   for the options of `cargo bolero test` above: the sanitizer and its default, and the
//!   toolchain. No code copied.
//! Consulted: cargo-bolero 0.13.5 `bin/cargo-bolero/src/test.rs`, MIT,
//!   <https://github.com/camshaft/bolero/blob/7642943b6c1f8a9b5e824dd1cd29b7c8e56068b8/bin/cargo-bolero/src/test.rs>,
//!   for its run limits above: `-T`, `--runs`, `--jobs` and `--max-input-length`. No code copied.
//! Consulted: Kani 0.68.0 `library/std/src/lib.rs`, MIT OR Apache-2.0,
//!   <https://github.com/model-checking/kani/blob/0d2328a93f0e0ff66132d6bfa1a7d884877cf862/library/std/src/lib.rs>, for its
//!   `assert!` and `panic!`, which leave their messages unformatted during verification, the rule
//!   [`Finding::new`] follows for a finding's detail. No code copied.
#![forbid(unsafe_code)]

use core::fmt;

pub mod adversarial;
pub mod arenas;
pub mod cache;
pub mod cbor;
pub mod crossings;
pub mod intersect;
pub mod make;
pub mod natural;
pub mod orient2d;
pub mod piercing;
pub mod predicates;
pub mod project;
pub mod quadric;
pub mod robustness;
pub mod seeded;
pub mod sequence;
pub mod variants;
pub mod wide;

/// A law a harness found broken, with the input that broke it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// The law, in words, for example "orient2d(b, a, c) = −orient2d(a, b, c)".
    pub law: String,
    /// The decoded input and the values that broke the law.
    pub detail: String,
}

impl Finding {
    /// A finding that `law` is broken, with the detail `detail` writes: the input and the values
    /// that broke it.
    ///
    /// Under Kani (`cfg(kani)`) the detail is left empty and `detail` is never called. Kani
    /// reports a failed proof with a trace of every value, so the detail adds nothing there, and
    /// its formatting, on symbolic values, would only enlarge the problem Kani solves: its digit
    /// loops would have to be unwound or cut. Everywhere else the detail is written in full.
    #[must_use]
    pub fn new(law: &str, detail: impl FnOnce() -> String) -> Self {
        #[cfg(kani)]
        let detail = {
            let _ = detail;
            String::new()
        };
        #[cfg(not(kani))]
        let detail = detail();
        Self {
            law: law.to_owned(),
            detail,
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Written piece by piece rather than through `write!`, so that no formatting
        // specification is interpreted: the cheapest form for Kani, which runs this when a proof
        // fails (bolero's Kani engine turns the error into a string).
        f.write_str("broken law: ")?;
        f.write_str(&self.law)?;
        f.write_str("\n")?;
        f.write_str(&self.detail)
    }
}

impl std::error::Error for Finding {}

/// A harness: a plain function from a fuzzer's bytes to a verdict, the shape every engine drives.
pub type Harness = fn(&[u8]) -> Result<(), Finding>;
