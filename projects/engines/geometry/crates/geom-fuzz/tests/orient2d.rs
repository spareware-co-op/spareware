//! The orient2d target (`geom_fuzz::orient2d`) under every engine of chapter 28.5: one law and one
//! oracle, `geom_fuzz::orient2d::check`, behind two bolero front doors and the seeded leg.
//!
//! - `orient2d_fuzz` is the fuzz target: `bolero::check!` over raw bytes, each decoded by
//!   `geom_fuzz::orient2d::harness`. Under `cargo test` it is bolero's test mode: it replays the
//!   files in `tests/__fuzz__/orient2d/corpus` and `crashes`, then runs [`ITERATIONS`] random byte
//!   strings (`BOLERO_RANDOM_SEED=<seed>`, printed with any failure, replays one). Under Kani the
//!   same test is a proof over its bounded bytes, so the two share their corpus and crash files:
//!   raw bytes for one function.
//! - `orient2d_proof_predicate`, `orient2d_proof_exact_path` and `orient2d_proof_filter` are the
//!   bounded proof, one claim of `check` each: `bolero::check!` over draws of the proof's domain
//!   (`geom_fuzz::orient2d::proof_case` with `PROOF_BOUND`), each checked by `check_claim` for its
//!   claim. Under `cargo kani --tests -p geom-fuzz --harness orient2d_proof_exact_path --exact`
//!   (and the others) bolero's Kani engine makes every draw symbolic, so Kani proves the claim for
//!   every case of the domain, and the three together prove `check`'s claims. Under `cargo test`
//!   each is a property test over the same domain, so the proof's statement is also tested on
//!   stable Rust on every operating system.
//! - Kani proves the predicate's claim in parts, one job each: `orient2d_proof_predicate_part_0`
//!   to `_part_63` prove it for the draws of one class each
//!   (`geom_fuzz::orient2d::predicate_part`: the six coordinates' signs), and
//!   `orient2d_proof_predicate_cover` proves that every draw of the domain is in one of those
//!   classes, so together they prove it for the whole domain. `orient2d_proof_predicate` itself
//!   is the same claim over the whole domain as a test, on every operating system.
//! - Kani proves the filter's claim one step at a time, one job each:
//!   `orient2d_proof_filter_exact_stand_in`, `orient2d_proof_filter_predicate_stand_in` and
//!   `orient2d_proof_filter_answer` prove each step of `check_with` after the one reference
//!   (`geom_fuzz::orient2d::check_filter_step`), so together they prove the claim.
//!   `orient2d_proof_filter` itself is the claim whole as a test, on every operating system.
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

mod common;

use bolero::{ValueGenerator, check, produce};
use common::holds;
use geom_fuzz::Finding;
use geom_fuzz::orient2d::{
    self, Claim, FilterStep, MAX_INPUT_BYTES, PREDICATE_PARTS, PROOF_BOUND, ProofDraw, harness,
    predicate_part, proof_case,
};
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the fuzz target in bolero's test mode.
const ITERATIONS: usize = 10_000;

/// Random draws per `cargo test` run of the proof's domain in bolero's test mode.
const PROOF_ITERATIONS: usize = 10_000;

/// Byte strings in the seeded leg.
///
/// Sized for an optimised build by the release suite's time on GitHub's Linux runner (14 Nov 2026:
/// 20,000 cases in 0.014 s), about 0.7 s; a build with debug assertions runs the first tenth
/// ([`seeded::for_this_build`]).
const SEEDED_CASES: u32 = 1_000_000;

/// The longest byte string the fuzz target and the seeded leg try. The harness reads at most
/// [`MAX_INPUT_BYTES`] (73) bytes; the rest let some strings carry bytes nobody reads, which must
/// change nothing.
const MAX_LEN: u16 = 96;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

/// The longest byte string Kani proves the fuzz target on: under Kani, `orient2d_fuzz` proves the
/// harness for every byte string of at most this many bytes (`geom_fuzz::orient2d`, what is proven
/// and what is tested). The longest that fits the Kani job's budget: in the trials of PR #209,
/// strings of at most 4 bytes proved in 8 minutes and 8, 16 and 24 bytes did not finish within the
/// job's hour; after G28 routed orient2d to its exact paths on 64-bit targets, 4 bytes no longer
/// finished either, so the bound is 2 (the crate README).
const KANI_MAX_LEN: usize = 2;

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

/// The proof's body for one draw and one claim: the case it names, checked for that claim
/// (`geom_fuzz::orient2d::check_claim`; the three claims together are `check`).
fn prove(bound: u64, draw: &ProofDraw, claim: Claim) -> Result<(), Finding> {
    let case = proof_case(bound, draw).ok_or_else(|| {
        Finding::new("every draw of the proof's domain names a grid case", || {
            format!("{draw:?}")
        })
    })?;
    orient2d::check_claim(&case, claim)
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_fuzz() {
    check!()
        .with_iterations(ITERATIONS)
        .with_max_len(if cfg!(kani) {
            KANI_MAX_LEN
        } else {
            usize::from(MAX_LEN)
        })
        .for_each(|data: &[u8]| holds(harness(data)));
}

/// The predicate's claim over the whole domain, as a test on every operating system; Kani proves
/// it in parts ([`orient2d_proof_predicate_cover`]).
///
/// The bounded proof is three claims (`geom_fuzz::orient2d::check_claim`):
/// the predicate's sign, the exact path's value and the filter's sign, each for every case of the
/// domain, which together prove what one harness of `check` did. Proved as one, the claims took
/// one solve of 63 minutes on the hosted runner (run 43688165033). Apart, on a development
/// machine, the exact path's and the filter's took about 7 minutes each and the predicate's 29:
/// the predicate's own fast and total paths, never reached on the grid, are still in its formula,
/// and splitting its domain did not shorten it (an eighth of the domain took 13 to 28 minutes).
///
/// Under Kani, the SAT solver is kissat (`CaDiCaL` ran past three hours on the whole). Loops are
/// unwound at most 10 times. Every loop a passing case runs is shorter: the six coordinates,
/// drawn, mapped and checked one by one. The loops the bound cuts are in the formatting of a
/// [`Finding`], whose digit loops run on symbolic values and would otherwise unwind forever; only
/// a broken claim reaches them, and Kani's unwinding assertions fail the proof if any cut loop
/// can be reached, so the bound never hides a failure.
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
fn orient2d_proof_predicate() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove(PROOF_BOUND, draw, Claim::Predicate)));
}

/// The cover of the predicate's proof in parts, for one draw: its class is one of the
/// [`PREDICATE_PARTS`] the parts prove.
fn covered(draw: &ProofDraw) -> Result<(), Finding> {
    let part = predicate_part(draw);
    if part < PREDICATE_PARTS {
        Ok(())
    } else {
        Err(Finding::new(
            "every draw of the proof's domain is in one of the predicate's parts",
            || format!("{draw:?}: part {part}"),
        ))
    }
}

/// One part of the predicate's proof, for one draw: the predicate's claim when the draw is of
/// class `part`, and nothing asked of a draw of any other class, which is that class's part's.
fn prove_part(bound: u64, draw: &ProofDraw, part: u32) -> Result<(), Finding> {
    if predicate_part(draw) == part {
        prove(bound, draw, Claim::Predicate)
    } else {
        Ok(())
    }
}

/// The cover of the predicate's proof in parts: every draw of the domain is in one of the
/// [`PREDICATE_PARTS`] classes of `geom_fuzz::orient2d::predicate_part`, the sign patterns of the
/// six coordinates. `orient2d_proof_predicate_part_<k>` proves the predicate's claim for every
/// draw of class k and asks nothing of any other draw ([`prove_part`]); with this harness, which
/// proves that no draw is outside classes 0 to 63, the parts prove the claim for every draw of
/// the domain, which is what [`orient2d_proof_predicate`] proved as one harness. The domain, the
/// bound, the solver, the unwinding and Kani's checks are that harness's, in every part.
///
/// Why in parts: as one harness the claim took 35 to 46 minutes on the hosted runner, nine tenths
/// of it kissat's last solve (2004 of 2195 seconds in job 133673657660), and once ran past the
/// job's hour. Fixing the six signs is what shortens that solve most for the inputs it fixes:
/// measured on the hosted runner, a class of the six signs took 4 to 9 minutes of job time, a
/// class of the x coordinates' three signs 10 and 13, and classes of the six near-edge flags, the
/// six offsets or all of point a 6 to 11; `tests/attack_g14.rs` holds the parts, this harness
/// and Kani's matrix to one another.
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_cover() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(covered(draw)));
}

/// Class 0 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_0() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 0)));
}

/// Class 1 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_1() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 1)));
}

/// Class 2 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_2() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 2)));
}

/// Class 3 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_3() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 3)));
}

/// Class 4 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_4() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 4)));
}

/// Class 5 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_5() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 5)));
}

/// Class 6 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_6() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 6)));
}

/// Class 7 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_7() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 7)));
}

/// Class 8 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_8() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 8)));
}

/// Class 9 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_9() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 9)));
}

/// Class 10 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_10() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 10)));
}

/// Class 11 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_11() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 11)));
}

/// Class 12 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_12() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 12)));
}

/// Class 13 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_13() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 13)));
}

/// Class 14 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_14() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 14)));
}

/// Class 15 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_15() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 15)));
}

/// Class 16 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_16() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 16)));
}

/// Class 17 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_17() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 17)));
}

/// Class 18 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_18() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 18)));
}

/// Class 19 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_19() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 19)));
}

/// Class 20 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_20() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 20)));
}

/// Class 21 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_21() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 21)));
}

/// Class 22 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_22() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 22)));
}

/// Class 23 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_23() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 23)));
}

/// Class 24 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_24() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 24)));
}

/// Class 25 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_25() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 25)));
}

/// Class 26 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_26() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 26)));
}

/// Class 27 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_27() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 27)));
}

/// Class 28 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_28() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 28)));
}

/// Class 29 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_29() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 29)));
}

/// Class 30 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_30() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 30)));
}

/// Class 31 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_31() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 31)));
}

/// Class 32 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_32() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 32)));
}

/// Class 33 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_33() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 33)));
}

/// Class 34 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_34() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 34)));
}

/// Class 35 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_35() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 35)));
}

/// Class 36 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_36() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 36)));
}

/// Class 37 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_37() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 37)));
}

/// Class 38 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_38() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 38)));
}

/// Class 39 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_39() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 39)));
}

/// Class 40 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_40() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 40)));
}

/// Class 41 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_41() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 41)));
}

/// Class 42 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_42() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 42)));
}

/// Class 43 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_43() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 43)));
}

/// Class 44 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_44() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 44)));
}

/// Class 45 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_45() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 45)));
}

/// Class 46 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_46() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 46)));
}

/// Class 47 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_47() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 47)));
}

/// Class 48 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_48() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 48)));
}

/// Class 49 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_49() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 49)));
}

/// Class 50 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_50() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 50)));
}

/// Class 51 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_51() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 51)));
}

/// Class 52 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_52() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 52)));
}

/// Class 53 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_53() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 53)));
}

/// Class 54 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_54() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 54)));
}

/// Class 55 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_55() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 55)));
}

/// Class 56 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_56() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 56)));
}

/// Class 57 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_57() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 57)));
}

/// Class 58 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_58() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 58)));
}

/// Class 59 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_59() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 59)));
}

/// Class 60 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_60() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 60)));
}

/// Class 61 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_61() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 61)));
}

/// Class 62 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_62() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 62)));
}

/// Class 63 of the predicate's claim ([`orient2d_proof_predicate_cover`] says how the parts
/// share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_predicate_part_63() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_part(PROOF_BOUND, draw, 63)));
}

/// The exact path's claim of the bounded proof ([`orient2d_proof_predicate`] says how the three
/// harnesses share it).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_exact_path() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove(PROOF_BOUND, draw, Claim::ExactPath)));
}

/// The filter's claim over the whole domain, as a test on every operating system; Kani proves it
/// one step at a time ([`orient2d_proof_filter_answer`]).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
fn orient2d_proof_filter() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove(PROOF_BOUND, draw, Claim::Filter)));
}

/// One step of the filter's claim, for one draw: the case it names, its reference, and then that
/// step alone (`geom_fuzz::orient2d::check_filter_step`; the three steps together are the claim).
fn prove_filter_step(bound: u64, draw: &ProofDraw, step: FilterStep) -> Result<(), Finding> {
    let case = proof_case(bound, draw).ok_or_else(|| {
        Finding::new("every draw of the proof's domain names a grid case", || {
            format!("{draw:?}")
        })
    })?;
    orient2d::check_filter_step(&case, step)
}

/// The filter's claim in Kani, its last step: the generated filter declines or gives the
/// reference's sign, for every case of the domain.
///
/// `orient2d_proof_filter_answer`, `orient2d_proof_filter_exact_stand_in` and
/// `orient2d_proof_filter_predicate_stand_in` prove `check_filter_step` for each of
/// `geom_fuzz::orient2d::FILTER_STEPS`, over the whole domain, each after the one reference
/// `check_with` takes first. The filter's claim is `check_with` on the reference's two stand-ins
/// and the filter, and `check_with` is that reference and then those three steps, so the three
/// prove what [`orient2d_proof_filter`] proved as one harness. The domain, the bound, the solver,
/// the unwinding and Kani's checks are that harness's, in every step.
///
/// Why in steps: as one harness the claim took 570 to 943 seconds of job time on the hosted
/// runner, nine tenths of it kissat's last solve (723 of 801 seconds of verification in job
/// 134967462570). The claim computes the reference three times, for itself and for the two
/// stand-ins, so that solve held the copies equal and the filter's bound at once. On a
/// development machine the stand-ins alone took 1213 seconds and the filter's step alone 564,
/// where one eighth of the domain under the whole claim took 1026: cutting the domain does not
/// separate the two, and cutting the steps does.
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_filter_answer() {
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_filter_step(PROOF_BOUND, draw, FilterStep::Answer)));
}

/// The filter's claim, its exact path's stand-in: the reference, computed again, returns its own
/// value ([`orient2d_proof_filter_answer`] says how the steps share the claim).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_filter_exact_stand_in() {
    let step = FilterStep::ExactStandIn;
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_filter_step(PROOF_BOUND, draw, step)));
}

/// The filter's claim, its predicate's stand-in: the reference's sign, computed again, is its own
/// sign ([`orient2d_proof_filter_answer`] says how the steps share the claim).
#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode catches panics, which WASI cannot; the seeded leg runs the \
              same law there"
)]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(kissat))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_proof_filter_predicate_stand_in() {
    let step = FilterStep::PredicateStandIn;
    check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .with_iterations(PROOF_ITERATIONS)
        .for_each(|draw| holds(prove_filter_step(PROOF_BOUND, draw, step)));
}

/// The three step harnesses are every step of the filter's claim, each once: a fourth step added
/// to `geom_fuzz::orient2d::FILTER_STEPS` needs its harness here.
#[test]
fn the_filter_proofs_cover_every_step() {
    assert_eq!(
        orient2d::FILTER_STEPS,
        [
            FilterStep::ExactStandIn,
            FilterStep::PredicateStandIn,
            FilterStep::Answer
        ]
    );
}

/// The three proofs' claims are every claim of `check`, each once: a fourth claim added to
/// `geom_fuzz::orient2d::CLAIMS` needs its harness here.
#[test]
fn the_proofs_cover_every_claim() {
    assert_eq!(
        orient2d::CLAIMS,
        [Claim::Predicate, Claim::ExactPath, Claim::Filter]
    );
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
