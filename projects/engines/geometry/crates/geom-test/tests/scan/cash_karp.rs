//! An embedded Runge-Kutta pair of orders 5 and 4 with Cash and Karp's coefficients, and automatic
//! step-size control: the banned-source scanner's cited fixture.
//!
//! This file is the counterpart of the scanner's seeded positives. The same method sits in a book
//! whose code the kernel may never take (chapter 29.4 and 29.6), so chapter 29.20, rule 7, asks
//! that a routine like this be written from the primary sources only, with its own names and with
//! every constant derived from a named source and cited. This file is written that way, the scan
//! must pass it (`tests/scan_seeded.rs`), and the tests there show it is a working method rather
//! than a strawman: its tableau meets every order condition exactly, in rational arithmetic, and
//! the integrator reaches the exact solutions of test problems within its tolerance.
//!
//! Guarantees:
//!
//! - **The tableau is Cash and Karp's**, kept as exact rationals; each coefficient becomes a double
//!   by one division of two integers, rounded to nearest.
//! - **A step is accepted or retried, never silently wrong.** [`attempt`] returns the fifth-order
//!   solution and the scaled error estimate; [`integrate`] accepts a step whose scaled error is at
//!   most 1, retries a smaller one otherwise, and fails with [`Failure`] when the step size
//!   underflows, the step budget runs out or a value stops being finite.
//! - **Deterministic.** Sums run in index order, and the square root and power come from
//!   `geom_num::math`, the same bits on every target.
//!
//! Method: J. R. Cash and A. H. Karp, "A variable order Runge-Kutta method for initial value
//! problems with rapidly varying right-hand sides", *ACM Transactions on Mathematical Software*
//! 16(3), 1990, 201 to 222, <https://doi.org/10.1145/79505.79507>, for the six-stage pair and its
//! coefficients; E. Hairer, S. P. Nørsett and G. Wanner, *Solving Ordinary Differential Equations
//! I: Nonstiff Problems*, 2nd edition, Springer, 1993, section II.4, "Automatic Step Size
//! Control", <https://doi.org/10.1007/978-3-540-78862-1>, for the error norm and the step-size
//! rule.
//!
//! The constants, each derived from those sources:
//!
//! - The two solutions of the pair differ by an estimate of the local error of the lower-order
//!   one, of order q = 4, which behaves like the step size to the power q + 1. So the step size
//!   that would make the scaled error 1 is the current one times (1/err)^(1/(q+1)), an exponent of
//!   1/5 (Hairer, Nørsett and Wanner, II.4).
//! - The safety factor 0.9, which makes the next step likely to be accepted: II.4 names 0.8 and
//!   0.9 among its usual choices.
//! - The largest growth factor 5: II.4 takes it between 1.5 and 5. The smallest factor 0.2 is its
//!   reciprocal, so one rejection shrinks the step no faster than one acceptance can grow it. Right
//!   after a rejection the step may not grow (II.4).
//! - The error norm: the root mean square, over the components, of each component's difference
//!   divided by atol + rtol · max(|y0|, |y1|) (II.4).
//!
//! Cost: six evaluations of the right-hand side per attempted step, and memory linear in the
//! dimension.

use geom_num::math::sqrt;
use geom_num::math::unproven::pow;

/// A rational coefficient: numerator and denominator.
pub type Rational = (i64, i64);

/// The nodes: the fraction of the step at which each of the six stages evaluates.
pub const NODES: [Rational; 6] = [(0, 1), (1, 5), (3, 10), (3, 5), (1, 1), (7, 8)];

/// The coupling coefficients: how much of each earlier stage's slope each stage starts from.
pub const COUPLING: [&[Rational]; 6] = [
    &[],
    &[(1, 5)],
    &[(3, 40), (9, 40)],
    &[(3, 10), (-9, 10), (6, 5)],
    &[(-11, 54), (5, 2), (-70, 27), (35, 27)],
    &[
        (1631, 55296),
        (175, 512),
        (575, 13824),
        (44275, 110_592),
        (253, 4096),
    ],
];

/// The weights of the fifth-order solution, which the integrator advances with.
pub const FIFTH_ORDER: [Rational; 6] = [
    (37, 378),
    (0, 1),
    (250, 621),
    (125, 594),
    (0, 1),
    (512, 1771),
];

/// The weights of the embedded fourth-order solution, used only to estimate the error.
pub const FOURTH_ORDER: [Rational; 6] = [
    (2825, 27648),
    (0, 1),
    (18575, 48384),
    (13525, 55296),
    (277, 14336),
    (1, 4),
];

/// The lower order of the pair, q.
pub const LOWER_ORDER: i32 = 4;

/// The safety factor on the step-size rule.
pub const SAFETY_FACTOR: f64 = 0.9;

/// The most a step may grow after an accepted step.
pub const LARGEST_GROWTH: f64 = 5.0;

/// The most a step may shrink after a rejected one.
pub const SMALLEST_SHRINK: f64 = 0.2;

/// A rational coefficient as the nearest double.
#[must_use]
pub fn value((numerator, denominator): Rational) -> f64 {
    numerator as f64 / denominator as f64
}

/// The tolerances of the error norm.
#[derive(Clone, Copy, Debug)]
pub struct Tolerance {
    /// The absolute tolerance, atol.
    pub absolute: f64,
    /// The relative tolerance, rtol.
    pub relative: f64,
}

/// Why an integration stopped without reaching its end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Failure {
    /// The step size became too small to move the time forward.
    StepTooSmall {
        /// The time reached.
        at: f64,
    },
    /// The step budget ran out.
    OutOfSteps {
        /// The time reached.
        at: f64,
    },
    /// A value stopped being finite.
    NotFinite,
}

/// One attempted step: the fifth-order solution and the scaled error estimate.
#[derive(Clone, Debug, PartialEq)]
pub struct Attempt {
    /// The fifth-order solution at the end of the step.
    pub solution: Vec<f64>,
    /// The scaled error estimate: at most 1 means the step meets the tolerance.
    pub error: f64,
}

/// The larger of two doubles, written as a comparison.
fn larger(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

/// One step of the pair of size `size` from `state` at `time`, for the right-hand side `rhs`.
///
/// # Errors
///
/// [`Failure::NotFinite`] when a stage, the solution or the error stops being finite.
pub fn attempt<F: Fn(f64, &[f64]) -> Vec<f64>>(
    rhs: &F,
    time: f64,
    state: &[f64],
    size: f64,
    tolerance: Tolerance,
) -> Result<Attempt, Failure> {
    let mut slopes: Vec<Vec<f64>> = Vec::with_capacity(NODES.len());
    for (node, coupling) in NODES.iter().zip(COUPLING) {
        let mut point = state.to_vec();
        for (coefficient, slope) in coupling.iter().zip(&slopes) {
            let weight = size * value(*coefficient);
            for (component, rate) in point.iter_mut().zip(slope) {
                *component += weight * rate;
            }
        }
        let slope = rhs(time + value(*node) * size, &point);
        if slope.iter().any(|s| !s.is_finite()) {
            return Err(Failure::NotFinite);
        }
        slopes.push(slope);
    }
    let combine = |weights: &[Rational; 6]| -> Vec<f64> {
        let mut out = state.to_vec();
        for (weight, slope) in weights.iter().zip(&slopes) {
            let factor = size * value(*weight);
            for (component, rate) in out.iter_mut().zip(slope) {
                *component += factor * rate;
            }
        }
        out
    };
    let fifth = combine(&FIFTH_ORDER);
    let fourth = combine(&FOURTH_ORDER);
    let mut squares = 0.0;
    for ((start, high), low) in state.iter().zip(&fifth).zip(&fourth) {
        let scale = tolerance.absolute + tolerance.relative * larger(start.abs(), high.abs());
        let scaled = (high - low) / scale;
        squares += scaled * scaled;
    }
    let mean = squares / state.len() as f64;
    let error = sqrt(mean).map_err(|_| Failure::NotFinite)?.get();
    if !error.is_finite() || fifth.iter().any(|v| !v.is_finite()) {
        return Err(Failure::NotFinite);
    }
    Ok(Attempt {
        solution: fifth,
        error,
    })
}

/// The factor the next step size is the current one times, after a step with scaled error
/// `error`: (1/err)^(1/(q+1)) times the safety factor, kept between the smallest shrink and the
/// largest growth, and no larger than 1 right after a rejection.
///
/// # Errors
///
/// [`Failure::NotFinite`] when the error is not finite.
pub fn step_factor(error: f64, after_rejection: bool) -> Result<f64, Failure> {
    let largest = if after_rejection { 1.0 } else { LARGEST_GROWTH };
    if error == 0.0 {
        return Ok(largest);
    }
    let exponent = 1.0 / f64::from(LOWER_ORDER + 1);
    let ideal = SAFETY_FACTOR
        * pow(1.0 / error, exponent)
            .map_err(|_| Failure::NotFinite)?
            .get();
    Ok(if ideal < SMALLEST_SHRINK {
        SMALLEST_SHRINK
    } else if ideal > largest {
        largest
    } else {
        ideal
    })
}

/// Integrates `rhs` from `state` at `start` to `end`, starting with step size `first_size`, taking
/// at most `budget` attempted steps.
///
/// # Errors
///
/// [`Failure`] when the step size underflows, the budget runs out or a value stops being finite.
pub fn integrate<F: Fn(f64, &[f64]) -> Vec<f64>>(
    rhs: &F,
    start: f64,
    state: &[f64],
    end: f64,
    first_size: f64,
    tolerance: Tolerance,
    budget: usize,
) -> Result<Vec<f64>, Failure> {
    let mut time = start;
    let mut current = state.to_vec();
    let mut size = first_size;
    let mut rejected = false;
    for _ in 0..budget {
        if time >= end {
            return Ok(current);
        }
        let last = time + size >= end;
        let step = if last { end - time } else { size };
        if time + step == time {
            return Err(Failure::StepTooSmall { at: time });
        }
        let tried = attempt(rhs, time, &current, step, tolerance)?;
        if tried.error <= 1.0 {
            time = if last { end } else { time + step };
            current = tried.solution;
            size = step * step_factor(tried.error, rejected)?;
            rejected = false;
        } else {
            size = step * step_factor(tried.error, true)?;
            rejected = true;
        }
    }
    if time >= end {
        Ok(current)
    } else {
        Err(Failure::OutOfSteps { at: time })
    }
}
