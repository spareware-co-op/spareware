//! The orient2d target (`geom_fuzz::orient2d`) under every engine of chapter 28.5: one law and one
//! oracle, `geom_fuzz::orient2d::check`, behind two bolero front doors and the seeded leg.
//!
//! - `orient2d_fuzz` is the fuzz target: `bolero::check!` over raw bytes, each decoded by
//!   `geom_fuzz::orient2d::harness`. Under `cargo test` it is bolero's test mode: it replays the
//!   files in `tests/__fuzz__/orient2d/corpus` and `crashes`, then runs [`ITERATIONS`] random byte
//!   strings (`BOLERO_RANDOM_SEED=<seed>`, printed with any failure, replays one). The same test is
//!   the libFuzzer target of `cargo bolero test orient2d_fuzz --package geom-fuzz --engine
//!   libfuzzer` and the target `ClusterFuzzLite` runs (`.clusterfuzzlite/build.sh`), so the three
//!   share their corpus and crash files: raw bytes for one function.
//! - `orient2d_proof` is the bounded proof: `bolero::check!` over draws of the proof's domain
//!   (`geom_fuzz::orient2d::proof_case` with `PROOF_BOUND`), each checked by `check`. Under
//!   `cargo kani --tests -p geom-fuzz --harness orient2d_proof --exact` (or
//!   `cargo bolero test orient2d_proof --engine kani`, which runs that command) bolero's Kani
//!   engine makes every draw symbolic, so Kani proves the claims for every case of the domain.
//!   Under `cargo test` it is a property test over the same domain, so the proof's statement is
//!   also tested on stable Rust on every operating system.
//! - `orient2d_seeded` runs the harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same cases on every target, WebAssembly included.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use bolero::{ValueGenerator, check, produce};
use geom_fuzz::Finding;
use geom_fuzz::orient2d::{self, MAX_INPUT_BYTES, PROOF_BOUND, ProofDraw, harness, proof_case};
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the fuzz target in bolero's test mode.
const ITERATIONS: usize = 10_000;

/// Random draws per `cargo test` run of the proof's domain in bolero's test mode.
const PROOF_ITERATIONS: usize = 10_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 20_000;

/// The longest byte string the fuzz target and the seeded leg try. The harness reads at most
/// [`MAX_INPUT_BYTES`] (73) bytes; the rest let some strings carry bytes nobody reads, which must
/// change nothing.
const MAX_LEN: u16 = 96;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

/// Draws of the proof's domain with bound `bound`: for each of the six coordinates, near an edge
/// or not, negative or not, and an offset in `0..=bound`. Under Kani each choice is symbolic,
/// constrained to exactly these values.
fn proof_draws(bound: u64) -> impl ValueGenerator<Output = ProofDraw> {
    (
        produce::<[bool; 6]>(),
        produce::<[bool; 6]>(),
        core::array::from_fn::<_, 6, _>(|_| 0..=bound),
    )
        .map_gen(|(near_edge, negative, offset)| ProofDraw {
            near_edge,
            negative,
            offset,
        })
}

/// The proof's body for one draw: the case it names, checked.
fn prove(bound: u64, draw: &ProofDraw) -> Result<(), Finding> {
    let case = proof_case(bound, draw).ok_or_else(|| {
        Finding::new("every draw of the proof's domain names a grid case", || {
            format!("{draw:?}")
        })
    })?;
    orient2d::check(&case)
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn orient2d_fuzz() {
    check!()
        .with_iterations(ITERATIONS)
        .with_max_len(usize::from(MAX_LEN))
        .for_each(|data: &[u8]| harness(data));
}

/// Under Kani, loops are unwound at most 10 times. Every loop a passing case runs is shorter:
/// the six coordinates, drawn, mapped and checked one by one. The loops the bound cuts are in the
/// formatting of a [`Finding`], whose digit loops run on symbolic values and would otherwise
/// unwind forever; only a broken claim reaches them, and Kani's unwinding assertions fail the
/// proof if any cut loop can be reached, so the bound never hides a failure.
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| prove(PROOF_BOUND, draw));
}

#[test]
fn orient2d_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "orient2d fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}
