//! The tolerance ladder (chapter 6.5): the only tolerances in the kernel.
//!
//! Guarantees: there is no per-entity tolerance that grows. Every threshold is one of these,
//! counted in grid steps of the frame, so it scales with the step: a circuit board's fit bound (on
//! step `Q1`) is 25 times tighter than a building's (on step `Q0`). The exact polyhedral core uses
//! none of them.
//!
//! | Name | Steps | At Q0 | At Q1 | Used for |
//! | --- | --- | --- | --- | --- |
//! | Commit rounding | 1/2 per axis, ties to even; under 1 in 2D and 3D | 78.125 nm; 110 nm 2D, 135 nm 3D | 3.125 nm; 4.4 nm 2D, 5.4 nm 3D | Making an implicit point explicit |
//! | Fit bound | 64 | 0.01 mm | 0.4 µm | The largest certified error of a curved construction that feeds topology |
//! | Angular bound | 2^-34 rad | about 5.8 × 10^-11 rad | the same | Angles in curved code; equals the fit bound over the frame's range |
//! | Import healing | 640, capped at 6,400 | 0.1 mm, cap 1 mm | 4 µm, cap 40 µm | Closing gaps in imported data, once |
//! | Short-edge warning | 3,200 | 0.5 mm | 20 µm | A warning only; the true minimum edge is one step |
//!
//! Derivations, with q the frame's step (156.25 nm at `Q0`, 6.25 nm at `Q1`):
//!
//! - **Commit rounding** ([`COMMIT_PER_AXIS`], [`COMMIT_DISTANCE_2D`], [`COMMIT_DISTANCE_3D`]):
//!   committing rounds each coordinate to the nearest step, ties to even (chapter 6.3)
//!   ([`Tpi::to_grid`](crate::plane::Tpi::to_grid)), so it moves each by at most q/2, one half
//!   step; a tie moves it by exactly q/2, never more. As a distance in n dimensions that is at
//!   most √n · q/2: (√2/2)q ≈ 0.707q in 2D and (√3/2)q ≈ 0.866q in 3D, 110 nm and 135 nm at
//!   `Q0`. Both are below one step, so one whole step is a certified ceiling for each:
//!   n · (1/2)² ≤ 1² for n ≤ 4, checked when the crate compiles.
//! - **Fit bound** ([`FIT`]): 64q is 10 µm = 0.01 mm at `Q0` (64 × 156.25 nm) and 0.4 µm at `Q1`.
//! - **Angular bound** ([`ANGULAR`], and its exponent [`ANGULAR_LOG2`]): the fit bound over the
//!   frame's range, 64q / 2^40 q = 2^6 / 2^40 = 2^−34 rad, the same at every step; checked when
//!   the crate compiles.
//! - **Import healing** ([`HEAL_DEFAULT`], [`HEAL_CAP`]): 640q = 0.1 mm and 6,400q = 1 mm at `Q0`
//!   (6,400 steps of `Q0` are one millimetre, [`crate::grid`]).
//! - **Short-edge warning** ([`SHORT_EDGE_WARNING`]): 3,200q = 0.5 mm at `Q0`.
//!
//! The rungs chapter 6.5 proposes but an owner has not accepted (the per-operation bound, the
//! resolution floor, solver convergence, the smallest triangle altitude and the others) are not
//! here; each joins this module when accepted.
//!
//! Method: the tolerance ladder of chapter 6.5
//! (`projects/engines/geometry/architecture/06-numbers.md`). Each constant's derivation is above,
//! and `geom-gen`'s grid-steps table recomputes every rung exactly from these constants and checks
//! it against them.
//!
//! Cost: none at run time for the constants; [`quarter_nm`] is one checked multiplication.

use crate::grid::{COORD_BITS, GridStep, Steps};
use crate::real::Real;

/// A count of half grid steps: the one rung of the ladder that is not a whole number of steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct HalfSteps(pub u64);

/// The largest certified error of any curved construction that feeds topology: 64 steps.
pub const FIT: Steps = Steps(64);

/// The default distance import healing may close: 640 steps.
pub const HEAL_DEFAULT: Steps = Steps(640);

/// The most import healing may ever close: 6,400 steps.
pub const HEAL_CAP: Steps = Steps(6_400);

/// Edges shorter than this are reported with a warning, never refused: 3,200 steps.
pub const SHORT_EDGE_WARNING: Steps = Steps(3_200);

/// Committing an implicit point to the grid moves each coordinate by at most this: one half step
/// ([`Tpi::to_grid`](crate::plane::Tpi::to_grid) rounds to the nearest step, ties to even).
pub const COMMIT_PER_AXIS: HalfSteps = HalfSteps(1);

/// A certified ceiling, in whole steps, on how far committing moves a 2D point: the exact
/// distance is at most (√2/2) steps ≈ 0.707, below one step.
pub const COMMIT_DISTANCE_2D: Steps = Steps(1);

/// A certified ceiling, in whole steps, on how far committing moves a 3D point: the exact
/// distance is at most (√3/2) steps ≈ 0.866, below one step.
pub const COMMIT_DISTANCE_3D: Steps = Steps(1);

/// Log base 2 of the angular bound: the angular bound is 2^-34 radians.
pub const ANGULAR_LOG2: i32 = -34;

/// The angular bound, 2^-34 radians (about 5.8 × 10^-11), exactly representable: built from its
/// bits, the exponent field 1023 − 34 = 989 = 0x3dd and nothing else, and checked when the crate
/// compiles to be that power of two and a `Real`.
pub const ANGULAR: Real = match Real::new(f64::from_bits(0x3dd0_0000_0000_0000)) {
    Ok(r) => r,
    // A finite constant: the compile-time check below fails if this arm were ever taken.
    Err(_) => Real::ZERO,
};

/// A length in steps of `step`, expressed exactly in quarter-nanometres (for reports and
/// messages). Always `Some`: a count below 2^64 times a step of at most 625 < 2^10 quarter-nm is
/// below 2^74, far inside `u128`; the multiplication is checked all the same.
#[must_use]
pub const fn quarter_nm(steps: Steps, step: GridStep) -> Option<u128> {
    // Both casts widen without loss: u64 and u32 into u128.
    (steps.0 as u128).checked_mul(step.quarter_nm() as u128)
}

/// Checks, at compile time, the derivations in the module documentation: the angular bound is the
/// fit bound over the frame's range, 64 / 2^40 = 2^6 / 2^40 = 2^-34, and is that power of two;
/// healing's default is within its cap; and one whole step bounds the commit distance in 2D and
/// 3D: n half steps squared, n · h² / 4 steps², is at most c² for the ceiling c.
const _: () = {
    assert!(FIT.0 == 1 << 6);
    // 2^6 / 2^B = 2^−(B − 6): the exponent is negative, and its magnitude is B − 6.
    assert!(ANGULAR_LOG2 < 0);
    assert!(matches!(COORD_BITS.checked_sub(6), Some(e) if e == ANGULAR_LOG2.unsigned_abs()));
    // 2^−34 is the double with exponent field 1023 − 34 and nothing else.
    let bits = ANGULAR.get().to_bits();
    assert!(bits >> 52 == 1023 - 34 && bits & ((1 << 52) - 1) == 0);
    assert!(ANGULAR_LOG2.unsigned_abs() == 34);
    assert!(HEAL_DEFAULT.0 <= HEAL_CAP.0);
    let h = COMMIT_PER_AXIS.0;
    assert!(2 * h * h <= 4 * COMMIT_DISTANCE_2D.0 * COMMIT_DISTANCE_2D.0);
    assert!(3 * h * h <= 4 * COMMIT_DISTANCE_3D.0 * COMMIT_DISTANCE_3D.0);
};

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;

    #[test]
    fn ladder_values_in_real_units() {
        // 1 mm = 4,000,000 quarter-nm.
        assert_eq!(quarter_nm(FIT, GridStep::Q0), Some(40_000)); // 0.01 mm
        assert_eq!(quarter_nm(FIT, GridStep::Q1), Some(1_600)); // 0.4 µm
        assert_eq!(quarter_nm(HEAL_DEFAULT, GridStep::Q0), Some(400_000)); // 0.1 mm
        assert_eq!(quarter_nm(HEAL_CAP, GridStep::Q0), Some(4_000_000)); // 1 mm
        assert_eq!(
            quarter_nm(SHORT_EDGE_WARNING, GridStep::Q0),
            Some(2_000_000)
        ); // 0.5 mm
        assert_eq!(quarter_nm(SHORT_EDGE_WARNING, GridStep::Q1), Some(80_000)); // 20 µm
        assert_eq!(
            quarter_nm(Steps(u64::MAX), GridStep::Q0),
            Some(u128::from(u64::MAX) * 625)
        );
        assert_eq!(ANGULAR.get(), 5.820_766_091_346_741e-11);
        assert_eq!(ANGULAR.get(), 1.0 / 17_179_869_184.0);
    }

    /// The commit ceilings hold with room: the exact distances (√2/2 and √3/2 steps) are below
    /// one step, at both steps of the table (110 nm and 135 nm at `Q0`).
    #[test]
    fn commit_ceilings_bound_the_exact_distances() {
        let half: f64 = 0.5;
        let d2 = (2.0 * half * half).sqrt();
        let d3 = (3.0 * half * half).sqrt();
        assert!(d2 < 1.0 && d3 < 1.0);
        let q0_nm = 156.25;
        assert!((d2 * q0_nm - 110.485).abs() < 1e-3);
        assert!((d3 * q0_nm - 135.316).abs() < 1e-3);
        assert_eq!(COMMIT_PER_AXIS, HalfSteps(1));
    }
}
