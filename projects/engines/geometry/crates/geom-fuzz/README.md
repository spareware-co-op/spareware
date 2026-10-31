# geom-fuzz: the kernel's fuzz targets and bounded proofs

`geom-fuzz` is a development crate of the Spare Kit geometry kernel (chapter 27). It holds the
fuzz targets of chapter 28.5 and the bounded Kani proofs of chapter 28.7 that run on the same
harnesses. Nothing here ships, and no kernel crate depends on it.

This page is the layout every target follows, chosen by piece G14 (the fuzz toolchain spike), and
how each engine runs a target, on your machine and in CI. The first target built to it is
`orient2d` (`src/orient2d.rs`): `geom-num`'s orient2d, its exact path and its `f64` filter must
agree with the exact determinant of orient2d's definition, computed from scratch. The others:
`predicates` (`src/predicates.rs`, G11), every exact predicate answers and swapping two points
flips its sign; `wide` (`src/wide.rs`, G26), the wide integers agree with `num-bigint` on
every operation at every width, `I128` to the reserved 768 bits; `intersect`
(`src/intersect.rs`, G158), an intersection's items keep their invariants against an all-pairs
oracle, and a pair asked either way round makes one call; `project` (`src/project.rs`, G162),
a nearest point is never beaten by a sample of the kind; `level` (`src/level.rs`, G168), a level
curve misses nothing a dense grid can see; and, for Gate 2 (decision 32.44:
a target per phase-2 code piece), `curves` (`src/curves.rs`) and `surfaces` (`src/surfaces.rs`),
every curve and surface kind built from any bytes keeps the kind contract kit
(`geom_curve::contract`, `geom_surface::contract`) under any typed transform.

## One law, four engines

A target is one law and its oracle, written once as a plain function, and run by every engine:

| Engine | What it does | Where it runs | Test it runs |
| --- | --- | --- | --- |
| bolero's test mode | Replays the target's stored corpus and crash files, then tries 10,000 random inputs | `cargo test`, every operating system (kernel.yml, kernel-dev.yml) | `<target>_fuzz` |
| The seeded leg | Runs the harness on 20,000 byte strings from a printed seed, the same on every target | `cargo test`, every target, WebAssembly included | `<target>_seeded` |
| libFuzzer, under cargo-bolero | Coverage-guided fuzzing with the address sanitizer, on a nightly compiler | Linux and macOS; a minute per target on every pull request (kernel-fuzz.yml) | `<target>_fuzz` |
| ClusterFuzzLite | OSS-Fuzz's fuzzing service for pull requests, on the same libFuzzer binary | Pull requests (kernel-fuzz.yml), in OSS-Fuzz's container | `<target>_fuzz` |
| Kani | Proves the law for every case of a bounded domain | Linux and macOS; every pull request (kernel-fuzz.yml) | `<target>_proof` |

The three fuzzing engines drive the same function on the same bytes, so a corpus or crash file
from any of them replays in the others and in `cargo test`.

## The layout of a target

Each target `<target>` has a module `src/<target>.rs` and a test file `tests/<target>.rs`.

The module holds, with a header in the grammar of `projects/engines/geometry/HEADERS.md`:

- `Case`: what one input decodes to.
- `check(&Case) -> Result<(), Finding>`: the law and its oracle. It never panics by itself; a
  broken law is a `Finding` that names the law, the input and the values. Only the kernel code
  under test can panic, and a panic fails every engine.
- `check_with(...)`: `check` on functions handed in, so unit tests can feed it broken versions
  and show it catches each one (28.4, fault injection: a check that cannot fail proves nothing).
- `decode(&[u8]) -> (Case, usize)`: any bytes to a valid case, aimed at the hard cases with
  `geom-test`'s grid sampler, reading at most `MAX_INPUT_BYTES`, a bound derived on the constant
  and reached by a unit test.
- `harness(&[u8]) -> Result<(), Finding>`: `decode`, then `check`. This is the function every
  fuzzing engine drives (the crate's `Harness` type).
- For a target with a proof: `ProofDraw`, `proof_case(bound, &ProofDraw) -> Option<Case>` and
  `PROOF_BOUND`, the bounded domain the proof covers. The module header says exactly what is
  proven and what is only tested (28.7).

The test file holds three tests:

```rust
#[test]
fn <target>_fuzz() {
    bolero::check!()
        .with_iterations(ITERATIONS)
        .with_max_len(MAX_LEN)
        .for_each(|data: &[u8]| harness(data));
}

#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
#[cfg_attr(kani, kani::unwind(10))]
fn <target>_proof() {
    bolero::check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .for_each(|draw| holds(prove(PROOF_BOUND, draw)));
}

#[test]
fn <target>_seeded() -> Result<(), seeded::Failure> {
    seeded::run("<target> fuzz target, seeded", Seed::DEFAULT, SEEDED_CASES, MAX_LEN, harness)
}
```

Rules:

- A bolero test is a fuzz target exactly when its name ends in `_fuzz`. ClusterFuzzLite's build
  keeps those and drops every other bolero test, such as a proof's property test or a test that
  shows a failure is reported.
- The fuzz test takes raw bytes and hands them to `harness`, never a typed generator, so every
  engine's corpus and crash files are the harness's own input.
- `check!` is written in the `#[test]` function itself, never in a helper it calls. bolero names
  a target after the function `check!` is written in (`bolero-engine`'s `TargetLocation`), and
  cargo-bolero builds a target by running the test of that name, so a target in a helper cannot
  be built for libFuzzer or ClusterFuzzLite ("no targets matched"). `tests/attack_g14.rs` refuses
  one.
- No bolero test is `#[should_panic]`. cargo-bolero's `build-clusterfuzz` builds each test
  binary by running one of its bolero tests, the first it listed (in no fixed order), with
  `CARGO_BOLERO_SELECT` set, and there `check!` returns without running its body
  (`bolero-engine`'s `TargetLocation::should_run`). A `#[should_panic]` test fails in that mode,
  so ClusterFuzzLite's build fails on the runs that happen to pick it. A test that shows bolero
  fails catches the panic itself and accepts a body that never ran only in that mode
  (`tests/predicates.rs`, `bolero_fails_on_a_finding`). `tests/attack_g14.rs` refuses one.
- The proof test drives `check` itself, with a typed bolero generator of the proof's domain.
  Under Kani every generated value is symbolic, so the proof covers the whole domain; under
  `cargo test` the same test is a property test over that domain, on every operating system.
  Aiming at hard cases is pointless there: Kani covers every case.
- The proof test's body panics on a broken law, through `holds`, rather than returning the
  `Finding`. Under Kani, bolero's engine turns a returned error into its `PanicError`, whose
  constructor may capture a backtrace (`bolero-engine`'s `panic.rs`), and Kani would have to
  explore that capture; a panic is a failed property and nothing more, and Kani's `panic!`
  formats nothing. Under `cargo test` bolero catches the panic and prints the finding with the
  failing draw. The fuzz test still returns the `Finding`, which the fuzzers report.
- A finding is made with `Finding::new(law, || detail)`, so its detail is written only when it is
  reported, and never under Kani, which traces every value itself; formatting symbolic values
  would only enlarge the problem Kani solves.
- Each Kani bound is measured, not guessed: trial harnesses ran on the hosted runner until each
  bound was chosen, then were removed. For orient2d (PR #209) the proof's bound is 1 (bound 0
  took 3½ minutes, 1 took 19, and 3, 7 and 15 did not finish within the job's hour) and the fuzz
  target was proven on every byte string of at most 4 bytes (8 minutes; 8, 16 and 24 bytes did not
  finish). Once G28 routed orient2d to its exact paths on 64-bit targets, 4 bytes no longer
  finished within the hour, so the fuzz target's bound is 2. After G37 rewrote the wide integers
  the proof reaches, the bound-1 proof took 47 and then 75 minutes with kissat as Kani's SAT
  solver (runs 40291338626 and 40308963161: the hosted runners' speed varies), and CaDiCaL ran
  past three hours, so the proof uses kissat and its job has 120 minutes. A new
  target, or a change to the code a proof reaches, repeats the trials before its bounds are set.
- The proof test sets Kani's unwind bound above the longest loop a passing case runs (10 for
  orient2d: six coordinates), so a loop Kani cannot bound is cut rather than unwound forever.
  Kani's unwinding assertions, on by default, fail the proof if a cut loop can be reached, so
  the bound hides nothing.
- Corpus and crash files live in `tests/__fuzz__/<target>/corpus` and `tests/__fuzz__/<target>/crashes`
  (bolero names the folder after the test, without `_fuzz`). `cargo test` replays both on every
  run, so a crash found by any engine and saved there stays a test forever (28.16). Corpora only
  grow.
- A new target is registered in `.github/workflows/kernel-fuzz.yml`: its `_fuzz` test in the
  libFuzzer matrix, its `_proof` test in the Kani matrix, and every crate it fuzzes in the path
  filters. `tests/attack_g14.rs` fails until it is.

## Running a target

From the repository root.

**bolero's test mode and the seeded leg**, on any operating system with the workspace's compiler:

```text
cargo test -p geom-fuzz --test orient2d
```

A failure in test mode prints `BOLERO_RANDOM_SEED=<seed>`; set that variable to replay it. The
seeded leg prints `seed: <hex>`; set `SPAREKIT_SEED=<hex>` to replay it.

**libFuzzer**, on Linux or macOS. The sanitizer and coverage flags need a nightly compiler; CI
pins the one below. cargo-bolero builds under the workspace's `fuzz` profile (root `Cargo.toml`:
release optimisation with debug assertions and overflow checks on):

```text
cargo install cargo-bolero --version 0.13.5 --locked --no-default-features --features libfuzzer
rustup toolchain install nightly-2026-10-01 --profile minimal
cargo bolero test orient2d_fuzz --package geom-fuzz --toolchain nightly-2026-10-01 --engine libfuzzer -T 10min
```

`-T` bounds the run; `--runs`, `--jobs` and `--max-input-length` are the other limits. A crash is
written to `tests/__fuzz__/orient2d/crashes`, where `cargo test` replays it.

**Campaigns.** `.github/workflows/kernel-fuzz-campaign.yml` runs `wide_fuzz` under libFuzzer in
five shards of five hours, 25 CPU hours, for G26's "24 hours clean". It runs only on demand: when
the label `fuzz-campaign` is added to a pull request, or by hand from the Actions tab once the file
is on `main`. Each shard uploads any crash and the corpus it grew.

**Kani**, on Linux or macOS:

```text
cargo install kani-verifier --version 0.68.0 --locked
cargo kani setup
cargo kani --tests -p geom-fuzz --harness orient2d_proof --exact
```

`cargo bolero test orient2d_proof --engine kani`, from `projects/engines/geometry/crates/geom-fuzz`
with cargo-bolero's `kani` feature, runs `cargo kani --tests --harness orient2d_proof`, the same
proof.

`cfg(kani)`, which marks the proof tests, is declared to the compiler by `build.rs`
(`cargo::rustc-check-cfg`), not in the manifest's `[lints]` table, which repeats `geom-num`'s
exactly.

Two traps, found by the first proof. First, Kani 0.68.0 cannot compile a read of an enum's niche when the
niche starts above 2^64: `codegen_get_discriminant` in
`kani-compiler/src/codegen_cprover_gotoc/codegen/rvalue.rs` unwraps `u64::try_from(niche_start)`,
unchanged on Kani's main branch on 2026-10-25. rustc makes such a niche when it widens an enum's
tag to 128 bits, which it does for an enum whose data starts with an `i128` or `u128`; an `Option`
or `Result` around that enum then marks its other variant with the tag value 2^128 − 1.
`geom_num::NumError` was such an enum, so the proof failed to compile in `P2::new`; under Kani its tag
is now one byte (`#[cfg_attr(kani, repr(u8))]`, explained on the type; built normally its layout
is rustc's own, because a one-byte tag in every build made `interval::new` 12.7% dearer). Any other kernel enum that carries an `i128` or
`u128` and is wrapped in an `Option` or `Result` needs the same until Kani is fixed; the Kani job
fails to compile, never passes, when one is missed. Second, a proof whose body returns its
finding, or formats one, never finishes: the first orient2d proofs, the smallest domain among
them, were still in CBMC's symbolic execution when the job's hour ran out, in bolero's error
path (its backtrace capture) and in the formatting of the finding's detail. Hence `holds` and
`Finding::new`, above.

**ClusterFuzzLite**, with Docker, through OSS-Fuzz's helper (as ClusterFuzzLite's build
integration guide describes):

```text
git clone https://github.com/google/oss-fuzz
python oss-fuzz/infra/helper.py build_fuzzers --external --engine libfuzzer --sanitizer address <path to this repository>
```

On Windows only the first two run. libFuzzer and Kani need Linux or macOS; CI runs them on every
pull request that touches the code a target fuzzes.

**A crash from CI.** Each job uploads the crashing input as an artifact
(`libfuzzer-crashes-<target>` or `clusterfuzzlite-artifacts`). Save it in
`tests/__fuzz__/<target>/crashes/`, and `cargo test` fails on it until the bug is fixed, then
keeps it as a test.

## Why there is no cargo-fuzz wrapper

Chapter 28.5 asks for a thin cargo-fuzz target per harness, because ClusterFuzzLite's Rust guide
expects the cargo-fuzz layout, unless cargo-bolero's `build-clusterfuzz` command makes the wrappers
unnecessary, which it left to this spike. The spike's answer: it does, so there are no wrappers.

- **It works.** `build-clusterfuzz` (bin/cargo-bolero/src/build_clusterfuzz.rs) builds the tests
  with libFuzzer exactly as `cargo bolero test --engine libfuzzer` does and packs each test binary
  with one start-up script per bolero test, named `<test>_fuzzer`. ClusterFuzzLite finds targets
  by walking `$OUT` and takes an executable whose name ends in `_fuzzer`, the scripts included
  (infra/utils.py, `get_fuzz_targets` and `is_fuzz_target_local`), and passes libFuzzer's
  arguments, which the script hands to the test. The binary itself is named `<binary>.cargo`, an
  extension ClusterFuzzLite does not take for a target.
- **One check had to be replaced.** OSS-Fuzz's build check (infra/base-images/base-runner/
  bad_build_check) runs `file` and `objdump` on each target to see that it is an x86-64 binary
  with the address sanitizer compiled in, and a script is neither. kernel-fuzz.yml turns that
  check off (`bad-build-check: false`), and `.clusterfuzzlite/build.sh` makes the same checks on
  the binary each script starts: an x86-64 ELF executable, at least 1,000 calls into the address
  sanitizer, at least 100 instrumented edges, and a clean run of 64 inputs. The build fails if
  any does not hold.
- **The wrappers would need a licence the kernel has not allowed.** A cargo-fuzz target links
  `libfuzzer-sys`, whose manifest's licence is `(MIT OR Apache-2.0) AND NCSA`. NCSA is not on
  chapter 29.2's allow-list, so the crate cannot enter the kernel workspace's `Cargo.lock` without
  a row in 29.2 and a kernel owner's yes. cargo-bolero's libFuzzer engine is already in it, under
  MIT.
- **One binary for every fuzzing engine.** ClusterFuzzLite fuzzes the very binary cargo-bolero
  fuzzes, from the same test, so there is no second copy of a target to fall out of step.

Limits, written down so nobody trips on them:

- ClusterFuzzLite's `parallel-fuzzing` stays off. libFuzzer's `-jobs` and `-fork` modes start
  the binary again without the start-up script's arguments, which has not been tried here.
- The build installs `g++` in OSS-Fuzz's image, because bolero's libFuzzer engine compiles its
  own copy of libFuzzer with the system's C++ compiler, and turns off OSS-Fuzz's C and C++ flags
  for that compile, which would make libFuzzer instrument itself.
- ClusterFuzzLite runs in code-change mode on pull requests, with no storage repository, so each
  run starts from the corpus files committed here and fuzzes every target. Batch fuzzing, a
  corpus kept between runs and hours per target are the nightly fuzzing of 28.5, which G07 (where
  it runs) and G10 (the nightly workflows and machines) set up.

## What the orient2d proof covers

`orient2d_proof` drives `check` over every case `proof_case` gives with bound `PROOF_BOUND`:
every one of the six coordinates within `PROOF_BOUND` grid steps of 0 or of an edge
±(2^40 − 1) of the frame, in any combination. Kani proves, for every such case, that orient2d
answers with the reference's sign, that the exact path returns the reference's value, that the
filter declines or gives the reference's sign, and that nothing on the way panics or overflows.
Near the edges the filter's products round, so the proof covers the filter's error bound as well
as exact arithmetic. Everywhere else the claims are tested, not proven. Wider Kani proofs of the
numbers layer are piece G41's.

## Sources

Read for this layout, every one under a licence chapter 29.2 lets us read; no code was copied:

- bolero and cargo-bolero 0.13 (MIT), at commit `7642943b6c1f8a9b5e824dd1cd29b7c8e56068b8` of
  <https://github.com/camshaft/bolero>: `bin/cargo-bolero/src/build_clusterfuzz.rs`,
  `kani.rs`, `libfuzzer.rs`, `project.rs`, `selection.rs`, `test_target.rs`; the book's
  `library-installation.md` (the `fuzz` profile); and, from the published crates,
  `bolero-kani` 0.13.2 and `bolero-generator` 0.13.6's `kani.rs` (how a generator becomes
  symbolic) and `bolero-engine` 0.13.6's `target_location.rs` (test names and corpus folders).
- ClusterFuzzLite (Apache-2.0), at commit `52ecc61cb587ee99c26825a112a21abf19c7448c` of
  <https://github.com/google/clusterfuzzlite>: `docs/build-integration/rust_lang.md`,
  `docs/build_integration.md`, `docs/running-clusterfuzzlite/github_actions.md` and both
  actions' `action.yml`.
- OSS-Fuzz (Apache-2.0), at commit `ac54c4f21555fabcf24e19bad3213ac194ee9f85` of
  <https://github.com/google/oss-fuzz>: `infra/base-images/base-runner/bad_build_check` and
  `test_all.py`, `infra/utils.py`, `infra/cifuzz/build_fuzzers.py`, `run_fuzzers.py` and
  `fuzz_target.py`, and `infra/base-images/base-builder-rust/Dockerfile` and
  `base-builder/compile`.
- cargo-fuzz 0.13.2 (MIT OR Apache-2.0), `src/templates.rs` and `src/project.rs`, for the layout
  and flags of the wrappers not built; `libfuzzer-sys` 0.4.13's licence from crates.io.
- Kani 0.68.0 (MIT OR Apache-2.0), `kani-driver/src/call_cargo.rs`, for how `cargo kani --tests`
  builds a test target, `kani-compiler/src/codegen_cprover_gotoc/codegen/rvalue.rs` and
  `statement.rs` at tag `kani-0.68.0`, for the niche trap above, and the kani-github-action's
  `src/install-kani.sh` at commit `f838096619a707b0f6b2118cf435eaccfa33e51f`, for how CI installs
  it.
- cargo-bolero 0.13.5's `build_clusterfuzz.rs`, `list.rs`, `selection.rs` and `test_target.rs`
  from the published crate, and `bolero-engine` 0.13.6's `target_location.rs`, for how a target is
  named and selected (the rule that `check!` sits in the test itself).
- The Cargo book's "Build Scripts" chapter, `cargo::rustc-check-cfg`, for `build.rs`.
