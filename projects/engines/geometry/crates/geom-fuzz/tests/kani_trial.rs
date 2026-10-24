//! Temporary: Kani trial harnesses for choosing `geom_fuzz::orient2d::PROOF_BOUND`. Each proves
//! `geom_fuzz::orient2d::check` on the proof's domain with one bound, so CI can show how far the
//! bound can go within the job's budget. Removed once the bound is chosen.
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

use bolero::{ValueGenerator, check, produce};
use geom_fuzz::Finding;
use geom_fuzz::orient2d::{self, ProofDraw, proof_case};

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
    let case = proof_case(bound, draw).ok_or_else(|| Finding {
        law: "every draw of the proof's domain names a grid case".to_owned(),
        detail: format!("{draw:?}"),
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
        .for_each(|draw| prove(0, draw));
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
        .for_each(|draw| prove(1, draw));
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
        .for_each(|draw| prove(7, draw));
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
        .for_each(|draw| prove(15, draw));
}
