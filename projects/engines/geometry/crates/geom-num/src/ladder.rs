//! The tolerance ladder (chapter 6.5): the only tolerances in the kernel.
//!
//! There is no per-entity tolerance that grows. Every threshold is one of these, counted in grid
//! steps of the frame, so it scales with the step: a circuit board's fit bound (on step `Q1`) is 25
//! times tighter than a building's (on step `Q0`). The exact polyhedral core uses none of them.
//!
//! | Name | Steps | At Q0 | At Q1 | Used for |
//! | --- | --- | --- | --- | --- |
//! | Commit rounding | at most half a step | 78.125 nm | 3.125 nm | Making an implicit point explicit |
//! | Fit bound | 64 | 0.01 mm | 0.4 µm | The largest certified error of a curved construction that feeds topology |
//! | Angular bound | 2^-34 rad | | | Angles in curved code; equals the fit bound over the frame's range |
//! | Import healing | 640, capped at 6,400 | 0.1 mm, cap 1 mm | 4 µm, cap 40 µm | Closing gaps in imported data, once |
//! | Short-edge warning | 3,200 | 0.5 mm | 20 µm | A warning only; the true minimum edge is one step |

use crate::grid::{COORD_BITS, GridStep, Steps};

/// The largest certified error of any curved construction that feeds topology: 64 steps.
pub const FIT: Steps = Steps(64);

/// The default distance import healing may close: 640 steps.
pub const HEAL_DEFAULT: Steps = Steps(640);

/// The most import healing may ever close: 6,400 steps.
pub const HEAL_CAP: Steps = Steps(6_400);

/// Edges shorter than this are reported with a warning, never refused: 3,200 steps.
pub const SHORT_EDGE_WARNING: Steps = Steps(3_200);

/// Committing an implicit point to the grid moves it by at most this many half steps per axis.
pub const COMMIT_MAX_HALF_STEPS: u64 = 1;

/// Log base 2 of the angular bound: the angular bound is 2^-34 radians.
pub const ANGULAR_LOG2: i32 = -34;

/// The angular bound, 2^-34 radians (about 5.8 × 10^-11), exactly representable.
pub const ANGULAR: f64 = 1.0 / (1u64 << 34) as f64;

/// A length in steps of `step`, expressed exactly in quarter-nanometres (for reports and messages).
#[must_use]
pub const fn quarter_nm(steps: Steps, step: GridStep) -> u128 {
    steps.0 as u128 * step.quarter_nm() as u128
}

/// Checks, at compile time, that the angular bound is the fit bound over the frame's range:
/// 64 / 2^40 = 2^6 / 2^40 = 2^-34.
const _: () = {
    assert!(FIT.0 == 1 << 6);
    assert!(6 - COORD_BITS as i32 == ANGULAR_LOG2);
    assert!(HEAL_DEFAULT.0 <= HEAL_CAP.0);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ladder_values_in_real_units() {
        // 1 mm = 4,000,000 quarter-nm.
        assert_eq!(quarter_nm(FIT, GridStep::Q0), 40_000); // 0.01 mm
        assert_eq!(quarter_nm(FIT, GridStep::Q1), 1_600); // 0.4 µm
        assert_eq!(quarter_nm(HEAL_DEFAULT, GridStep::Q0), 400_000); // 0.1 mm
        assert_eq!(quarter_nm(HEAL_CAP, GridStep::Q0), 4_000_000); // 1 mm
        assert_eq!(quarter_nm(SHORT_EDGE_WARNING, GridStep::Q0), 2_000_000); // 0.5 mm
        assert_eq!(quarter_nm(SHORT_EDGE_WARNING, GridStep::Q1), 80_000); // 20 µm
        assert_eq!(ANGULAR, 5.820_766_091_346_741e-11);
    }
}
