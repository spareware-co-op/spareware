//! Temporary: Kani trial harnesses for choosing `geom_fuzz::orient2d::PROOF_BOUND` and the length
//! of the byte strings Kani proves the fuzz target on. The `orient2d_trial_b<n>` tests prove
//! `geom_fuzz::orient2d::check` on the proof's domain with bound n; the `orient2d_bytes_trial_l<n>`
//! tests prove the fuzz target's own harness, `geom_fuzz::orient2d::harness`, on every byte string
//! of at most n bytes. CI shows how far each can go within the job's budget. Removed once both are
//! chosen.
//!
//! Each test calls `check!` itself, never through a helper: bolero names a target after the
//! function `check!` is written in, and cargo-bolero selects a target by that name, so a target
//! written in a helper cannot be built for libFuzzer or `ClusterFuzzLite`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
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
use geom_fuzz::orient2d::{self, ProofDraw, harness, proof_case};

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

fn prove(bound: u64, draw: &ProofDraw) -> Result<(), Finding> {
    let case = proof_case(bound, draw).ok_or_else(|| {
        Finding::new("every draw of the proof's domain names a grid case", || {
            format!("{draw:?}")
        })
    })?;
    orient2d::check(&case)
}

#[cfg_attr(target_family = "wasm", ignore = "bolero's test mode catches panics")]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_trial_b0() {
    check!()
        .with_generator(proof_draws(0))
        .with_iterations(100)
        .for_each(|draw| holds(prove(0, draw)));
}

#[cfg_attr(target_family = "wasm", ignore = "bolero's test mode catches panics")]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_trial_b1() {
    check!()
        .with_generator(proof_draws(1))
        .with_iterations(100)
        .for_each(|draw| holds(prove(1, draw)));
}

#[cfg_attr(target_family = "wasm", ignore = "bolero's test mode catches panics")]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_trial_b7() {
    check!()
        .with_generator(proof_draws(7))
        .with_iterations(100)
        .for_each(|draw| holds(prove(7, draw)));
}

#[cfg_attr(target_family = "wasm", ignore = "bolero's test mode catches panics")]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_trial_b15() {
    check!()
        .with_generator(proof_draws(15))
        .with_iterations(100)
        .for_each(|draw| holds(prove(15, draw)));
}

#[cfg_attr(target_family = "wasm", ignore = "bolero's test mode catches panics")]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_bytes_trial_l4() {
    check!()
        .with_max_len(4)
        .with_iterations(100)
        .for_each(|data: &[u8]| holds(harness(data)));
}

#[cfg_attr(target_family = "wasm", ignore = "bolero's test mode catches panics")]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_bytes_trial_l8() {
    check!()
        .with_max_len(8)
        .with_iterations(100)
        .for_each(|data: &[u8]| holds(harness(data)));
}

#[cfg_attr(target_family = "wasm", ignore = "bolero's test mode catches panics")]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_bytes_trial_l16() {
    check!()
        .with_max_len(16)
        .with_iterations(100)
        .for_each(|data: &[u8]| holds(harness(data)));
}

#[cfg_attr(target_family = "wasm", ignore = "bolero's test mode catches panics")]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
#[cfg_attr(kani, kani::unwind(10))]
fn orient2d_bytes_trial_l24() {
    check!()
        .with_max_len(24)
        .with_iterations(100)
        .for_each(|data: &[u8]| holds(harness(data)));
}
