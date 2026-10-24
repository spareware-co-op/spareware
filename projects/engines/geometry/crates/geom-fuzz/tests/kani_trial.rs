//! Temporary: Kani trial harnesses for choosing `geom_fuzz::orient2d::PROOF_BOUND`. Each proves
//! `geom_fuzz::orient2d::check` on the proof's domain with one bound, so CI can show how far the
//! bound can go within the job's budget. Removed once the bound is chosen.

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

fn trial(bound: u64) {
    check!()
        .with_generator(proof_draws(bound))
        .with_iterations(100)
        .for_each(|draw| prove(bound, draw));
}

#[cfg_attr(target_family = "wasm", ignore = "bolero's test mode catches panics")]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
fn orient2d_trial_b0() {
    trial(0);
}

#[cfg_attr(target_family = "wasm", ignore = "bolero's test mode catches panics")]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
fn orient2d_trial_b1() {
    trial(1);
}

#[cfg_attr(target_family = "wasm", ignore = "bolero's test mode catches panics")]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
fn orient2d_trial_b7() {
    trial(7);
}

#[cfg_attr(target_family = "wasm", ignore = "bolero's test mode catches panics")]
#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
fn orient2d_trial_b15() {
    trial(15);
}
