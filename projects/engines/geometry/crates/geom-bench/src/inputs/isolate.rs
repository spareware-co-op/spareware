//! Inputs for the root-isolation engine, `geom_num::isolate`, and the double-double balls of
//! `geom_num::ball`: domains and targets for [`cubic_1`] and [`circle_line_2`] on the systems
//! [`Cubic`] and [`crate::inputs::newton::CircleLine`], and boxes for [`ball_cubic`].
//!
//! Each isolation runs the whole engine on its domain: the work list, exclusion, the operator
//! steps, inflation, splitting, the floor, refinement to the target and the growth of each proof
//! box, under a meter with no budget. Classes:
//!
//! - `typical`: a domain holding every zero of the system, of random size, refined to the
//!   resolution floor or to a millionth of a step; every zero is proven and nothing is flagged.
//! - `extreme`, in rotation, the other ways an isolation ends: a domain ending exactly on a zero
//!   (flagged, since no box inside the domain can hold it in its interior); a target no arithmetic
//!   reaches (`CapReached`); a domain with no zero (excluded at once); and, for [`Cubic`], a
//!   domain split exactly through a zero (proven by inflation).
//!
//! For the balls, `typical` boxes are up to half a unit wide within three units of the origin;
//! `extreme` ones are points (radius zero), boxes near the top of the doubles (the cubic
//! overflows, `NotFinite`), and boxes of tiny numbers near the bottom.
//!
//! Guarantees: every batch is full and the same on every target, and its classes are what they
//! claim: every typical isolation proves every zero and flags nothing, every extreme batch ends in
//! exactly the outcomes listed above, and every typical ball succeeds while the extreme batch holds
//! a `NotFinite` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the outcomes chapter 12.3's engine distinguishes (proven, flagged at the floor, a cap
//! reached) and the double-double evaluation it falls back to
//! (`projects/engines/geometry/architecture/12-intersections.md`), each with its own class, on
//! systems whose zeros are known exactly.
//!
//! Cost: a few draws of [`SplitMix64`] per input, once per batch, outside every measured run.

use geom_num::NumError;
use geom_num::interval::Interval;
use geom_num::isolate::System;
use geom_num::newton::Residuals;
use geom_num::real::Real;
use geom_num::scalar::Scalar;

use super::newton::CircleLine;
use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// `(x − 1/4)(x + 3/2)(x − 2)`, with zeros 1/4, −3/2 and 2, the unknown in units of 64 grid steps.
#[derive(Clone, Copy, Debug)]
pub struct Cubic;

impl Residuals<1> for Cubic {
    fn residuals<S: Scalar>(&self, [x]: [S; 1]) -> Result<[S; 1], NumError> {
        let a = x.sub(constant(0.25)?)?;
        let b = x.add(constant(1.5)?)?;
        let c = x.sub(constant(2.0)?)?;
        Ok([a.mul(b)?.mul(c)?])
    }
}

impl System<1, 1> for Cubic {
    fn position<S: Scalar>(&self, [x]: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([constant::<S>(64.0)?.mul(x)?])
    }
}

/// The circle and the line of [`CircleLine`], the unknowns in units of 16 grid steps.
impl System<2, 2> for CircleLine {
    fn position<S: Scalar>(&self, [x, y]: [S; 2]) -> Result<[S; 2], NumError> {
        let scale = constant::<S>(16.0)?;
        Ok([scale.mul(x)?, scale.mul(y)?])
    }
}

/// The constant `v` in any scalar.
fn constant<S: Scalar>(v: f64) -> Result<S, NumError> {
    Ok(S::constant(Real::new(v)?))
}

/// A uniform double in `[lo, hi)`, from the top 53 bits of a draw.
fn uniform(g: &mut SplitMix64, lo: f64, hi: f64) -> f64 {
    let u = (g.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
    lo + (hi - lo) * u
}

/// `[lo, hi]`, or `None` (a rejected candidate) if it is not an interval.
fn between(lo: f64, hi: f64) -> Option<Interval> {
    Interval::new(lo, hi).ok()
}

/// The target `t` in grid steps.
fn target(t: f64) -> Option<Real> {
    Real::new(t).ok()
}

/// Domains and targets for `isolate::isolate` on [`Cubic`].
#[must_use]
pub fn cubic_1(class: Class) -> Vec<([Interval; 1], Real)> {
    let mut g = rng("isolate::cubic_1", class);
    match class {
        Class::Typical => batch_of(|n| {
            let g = &mut g;
            let t = if n % 2 == 0 { 1.0 } else { 1e-6 };
            Some((
                [between(uniform(g, -2.5, -1.6), uniform(g, 2.1, 3.0))?],
                target(t)?,
            ))
        }),
        Class::Extreme => batch_of(|n| {
            let g = &mut g;
            match n % 4 {
                0 => Some(([between(0.25, uniform(g, 0.5, 1.5))?], target(1.0)?)),
                1 => Some(([between(uniform(g, 1.5, 1.9), 2.5)?], target(1e-300)?)),
                2 => Some((
                    [between(uniform(g, -1.3, -1.0), uniform(g, 0.0, 0.2))?],
                    target(1.0)?,
                )),
                _ => {
                    // Split first at 1/4: the midpoint of [1/4 − h, 1/4 + h].
                    let h = f64::from(1u32 << g.index(4)) / 4.0;
                    Some(([between(0.25 - h, 0.25 + h)?], target(1.0)?))
                }
            }
        }),
        _ => Vec::new(),
    }
}

/// Domains and targets for `isolate::isolate` on [`CircleLine`].
#[must_use]
pub fn circle_line_2(class: Class) -> Vec<([Interval; 2], Real)> {
    let mut g = rng("isolate::circle_line_2", class);
    match class {
        Class::Typical => batch_of(|n| {
            let g = &mut g;
            let zero = if n % 2 == 0 { [4.0, 3.0] } else { [-3.0, -4.0] };
            let mut out = [between(0.0, 0.0)?; 2];
            for (o, z) in out.iter_mut().zip(zero) {
                *o = between(z - uniform(g, 0.5, 1.5), z + uniform(g, 0.5, 1.5))?;
            }
            Some((out, target(if n % 4 < 2 { 1.0 } else { 1e-6 })?))
        }),
        Class::Extreme => batch_of(|n| {
            let g = &mut g;
            match n % 3 {
                0 => Some((
                    [between(4.0, uniform(g, 4.5, 5.0))?, between(2.0, 4.0)?],
                    target(1.0)?,
                )),
                1 => Some((
                    [between(3.0, uniform(g, 4.5, 5.0))?, between(2.0, 4.0)?],
                    target(1e-300)?,
                )),
                _ => Some((
                    [between(uniform(g, 0.0, 0.5), 1.0)?, between(2.0, 3.0)?],
                    target(1.0)?,
                )),
            }
        }),
        _ => Vec::new(),
    }
}

/// Boxes for the balls: the cubic of [`Cubic`] evaluated in double-double balls over each box.
#[must_use]
pub fn ball_cubic(class: Class) -> Vec<[Interval; 1]> {
    let mut g = rng("ball::cubic", class);
    match class {
        Class::Typical => batch_of(|_| {
            let g = &mut g;
            let lo = uniform(g, -3.0, 3.0);
            Some([between(lo, lo + uniform(g, 0.0, 0.5))?])
        }),
        Class::Extreme => batch_of(|n| {
            let g = &mut g;
            match n % 3 {
                0 => {
                    let x = uniform(g, -3.0, 3.0);
                    Some([between(x, x)?])
                }
                1 => {
                    let x = uniform(g, 1e300, 2e300);
                    Some([between(x, x * 1.5)?])
                }
                _ => {
                    let x = uniform(g, 1e-300, 2e-300);
                    Some([between(x, x * 3.0)?])
                }
            }
        }),
        _ => Vec::new(),
    }
}
