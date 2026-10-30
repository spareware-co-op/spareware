//! Inputs for the interval Newton operators, `geom_num::newton`, and the Jacobians of
//! `geom_num::dual`: boxes for [`krawczyk_2`], [`hansen_sengupta_2`], [`krawczyk_4`] and
//! [`hansen_sengupta_4`] on the two fixed systems [`CircleLine`] and [`Four`], and points and boxes
//! for `dual::jacobian` on [`Four`] ([`jacobian_real`], [`jacobian_interval`]).
//!
//! The systems are written once over `geom_num::scalar::Scalar`, as every residual is, so each
//! benchmark runs the residuals' own code on dual intervals and on point intervals, the
//! preconditioner and the operator. Classes:
//!
//! - `typical`: boxes around a zero, of half-widths from 10^−6 to 1/4 (two unknowns) or 1/10
//!   (four), off centre by up to half their half-width: one step proves the zero, the operators'
//!   whole path. For the Jacobians, points and boxes of width up to 1/5 near [`Four`]'s zero.
//! - `extreme`, in rotation, every other way a step ends. For [`CircleLine`]: a box holding both
//!   zeros, whose midpoint Jacobian is singular (`Singular`); a box the line misses (`NoZero`, from
//!   the residuals' range); a box both curves cross but not together (`NoZero`, from the
//!   operator's image); a box so wide its residuals overflow (`NotFinite`); and a wide box around a
//!   zero (`Narrowed`). For [`Four`]: a box across `atan2`'s cut (`OutsideDomain`), a box reaching
//!   below zero in `ln`'s argument (`OutsideDomain`), a box whose `exp` overflows (`NotFinite`), a
//!   box away from the zero (`NoZero`), and a wide box around the zero (`Narrowed`). The
//!   Jacobians' extreme class is points and boxes at those refusals.
//!
//! Guarantees: every batch is full and the same on every target, and its classes are what they
//! claim: every typical box is proven with the zero inside, every extreme batch ends in exactly the
//! verdicts and errors listed above, and every typical Jacobian succeeds while every extreme one
//! is refused (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the cases an interval Newton step decides, as chapter 12.4 lists them (exclusion,
//! uniqueness, refinement) and as `geom_num::newton`'s module header proves them
//! (`projects/engines/geometry/architecture/12-intersections.md`), each with its own class, and
//! boxes around a zero at widths a decade apart, as the root-isolation engine (G53) will hand
//! them over.
//!
//! Cost: a few draws of [`SplitMix64`] per input, once per batch, outside every measured run.

use geom_num::NumError;
use geom_num::interval::Interval;
use geom_num::newton::Residuals;
use geom_num::real::Real;
use geom_num::scalar::Scalar;

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// The circle x² + y² = 25 and the line x − y = 1, with zeros (4, 3) and (−3, −4).
#[derive(Clone, Copy, Debug)]
pub struct CircleLine;

impl Residuals<2> for CircleLine {
    fn residuals<S: Scalar>(&self, [x, y]: [S; 2]) -> Result<[S; 2], NumError> {
        Ok([
            x.square()?.add(y.square()?)?.sub(constant(25.0)?)?,
            x.sub(y)?.sub(constant(1.0)?)?,
        ])
    }
}

/// exp(a) = b, ln(b) + a·c = 0, atan2(c, d) = π/4 and √(c + d + 2) = 2, with zero (0, 1, 1, 1):
/// every proven maths function but `sinpi` and `cospi`, and the fused multiply-add.
#[derive(Clone, Copy, Debug)]
pub struct Four;

impl Residuals<4> for Four {
    fn residuals<S: Scalar>(&self, [a, b, c, d]: [S; 4]) -> Result<[S; 4], NumError> {
        Ok([
            a.exp()?.sub(b)?,
            a.mul_add(c, b.ln()?)?,
            c.atan2(d)?.sub(S::pi().div(constant(4.0)?)?)?,
            c.add(d)?.add(constant(2.0)?)?.sqrt()?.sub(constant(2.0)?)?,
        ])
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

/// A box around `zero` with one half-width for every side, from 10^−6 to `top` (one of six scales
/// a decade apart, then up to twice that), each side off centre by up to half the half-width.
fn around<const N: usize>(g: &mut SplitMix64, zero: [f64; N], top: f64) -> Option<[Interval; N]> {
    const SCALES: [f64; 6] = [1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1e-1];
    let scale = SCALES.get(g.index(SCALES.len())).copied().unwrap_or(1e-6);
    let half = top * scale * uniform(g, 1.0, 2.0);
    let mut out = [between(0.0, 0.0)?; N];
    for (o, z) in out.iter_mut().zip(zero) {
        let shift = uniform(g, -0.5, 0.5) * half;
        *o = between(z + shift - half, z + shift + half)?;
    }
    Some(out)
}

/// Boxes for the operators on [`CircleLine`].
fn circle_line(op: &str, class: Class) -> Vec<[Interval; 2]> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|n| {
            let zero = if n % 2 == 0 { [4.0, 3.0] } else { [-3.0, -4.0] };
            around(&mut g, zero, 1.25)
        }),
        Class::Extreme => batch_of(|n| {
            let g = &mut g;
            match n % 5 {
                // Symmetric about the origin, so the midpoint Jacobian's first row is zero.
                0 => {
                    let (a, b) = (uniform(g, 5.0, 6.0), uniform(g, 5.0, 6.0));
                    Some([between(-a, a)?, between(-b, b)?])
                }
                1 => Some([between(0.0, 1.0)?, between(2.0, 3.0)?].map(|i| shifted(g, i))),
                2 => Some([between(3.5, 3.9)?, between(2.8, 3.2)?]),
                3 => Some([between(-1e200, 1e200)?, between(-1.0, 1.0)?]),
                // Around (4, 3) only, by 3.5 to 4.5 on each side: too wide for one step.
                _ => {
                    let mut side = || uniform(g, 3.5, 4.5);
                    Some([
                        between(4.0 - side(), 4.0 + side())?,
                        between(3.0 - side(), 3.0 + side())?,
                    ])
                }
            }
        }),
        _ => Vec::new(),
    }
}

/// Boxes for the operators on [`Four`].
fn four(op: &str, class: Class) -> Vec<[Interval; 4]> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| around(&mut g, [0.0, 1.0, 1.0, 1.0], 0.5)),
        Class::Extreme => batch_of(|n| {
            let g = &mut g;
            let near = |g: &mut SplitMix64| between(uniform(g, -0.1, 0.0), uniform(g, 0.0, 0.1));
            match n % 5 {
                0 => Some([
                    near(g)?,
                    between(0.9, 1.1)?,
                    between(-1.0, 1.0)?,
                    between(-1.0, -0.5)?,
                ]),
                1 => Some([
                    near(g)?,
                    between(-0.5, 1.1)?,
                    between(0.9, 1.1)?,
                    between(0.9, 1.1)?,
                ]),
                2 => Some([
                    between(700.0, 800.0)?,
                    between(0.9, 1.1)?,
                    between(0.9, 1.1)?,
                    between(0.9, 1.1)?,
                ]),
                3 => Some([
                    between(2.0, 3.0)?,
                    between(0.9, 1.1)?,
                    between(0.9, 1.1)?,
                    between(0.9, 1.1)?,
                ]),
                // Around the zero by 0.5 to 0.7 on each side: too wide for one step.
                _ => {
                    let mut out = [between(0.0, 0.0)?; 4];
                    for (o, z) in out.iter_mut().zip([0.0, 1.0, 1.0, 1.0]) {
                        let (lo, hi) = (uniform(g, 0.5, 0.7), uniform(g, 0.5, 0.7));
                        *o = between(z - lo, z + hi)?;
                    }
                    Some(out)
                }
            }
        }),
        _ => Vec::new(),
    }
}

/// `i` moved by up to a hundredth, so the batch's boxes differ.
fn shifted(g: &mut SplitMix64, i: Interval) -> Interval {
    let s = uniform(g, -0.01, 0.01);
    Interval::new(i.lo() + s, i.hi() + s).unwrap_or(i)
}

/// Boxes for `newton::krawczyk` on [`CircleLine`].
#[must_use]
pub fn krawczyk_2(class: Class) -> Vec<[Interval; 2]> {
    circle_line("newton::krawczyk_2", class)
}

/// Boxes for `newton::hansen_sengupta` on [`CircleLine`].
#[must_use]
pub fn hansen_sengupta_2(class: Class) -> Vec<[Interval; 2]> {
    circle_line("newton::hansen_sengupta_2", class)
}

/// Boxes for `newton::krawczyk` on [`Four`].
#[must_use]
pub fn krawczyk_4(class: Class) -> Vec<[Interval; 4]> {
    four("newton::krawczyk_4", class)
}

/// Boxes for `newton::hansen_sengupta` on [`Four`].
#[must_use]
pub fn hansen_sengupta_4(class: Class) -> Vec<[Interval; 4]> {
    four("newton::hansen_sengupta_4", class)
}

/// Points for `dual::jacobian` over reals on [`Four`]: near its zero (typical), or at the
/// refusals (extreme): `atan2`'s origin, `ln` at zero, `exp` overflowing, the square root's
/// argument at zero.
#[must_use]
pub fn jacobian_real(class: Class) -> Vec<[Real; 4]> {
    let mut g = rng("dual::jacobian_real", class);
    let point = |v: [f64; 4]| -> Option<[Real; 4]> {
        let mut out = [Real::ZERO; 4];
        for (o, x) in out.iter_mut().zip(v) {
            *o = Real::new(x).ok()?;
        }
        Some(out)
    };
    match class {
        Class::Typical => batch_of(|_| {
            let g = &mut g;
            point([
                uniform(g, -0.2, 0.2),
                uniform(g, 0.8, 1.2),
                uniform(g, 0.8, 1.2),
                uniform(g, 0.8, 1.2),
            ])
        }),
        Class::Extreme => batch_of(|n| {
            // Eighths, so the square root's argument −s + (s − 2) + 2 is exactly zero.
            let s = 0.5 + (g.index(8) as f64) / 8.0;
            point(match n % 4 {
                0 => [0.0, 1.0, 0.0, 0.0],
                1 => [0.0, 0.0, s, s],
                2 => [800.0, 1.0, s, s],
                _ => [0.0, 1.0, -s, s - 2.0],
            })
        }),
        _ => Vec::new(),
    }
}

/// Boxes for `dual::jacobian` over intervals on [`Four`]: near its zero (typical), or at the
/// refusals (extreme): across `atan2`'s cut, `ln`'s argument reaching zero, `exp` overflowing,
/// the square root's argument below zero.
#[must_use]
pub fn jacobian_interval(class: Class) -> Vec<[Interval; 4]> {
    let mut g = rng("dual::jacobian_interval", class);
    match class {
        Class::Typical => batch_of(|_| {
            let g = &mut g;
            let mut out = [between(0.0, 0.0)?; 4];
            for (o, (lo, hi)) in
                out.iter_mut()
                    .zip([(-0.2, 0.2), (0.8, 1.2), (0.8, 1.2), (0.8, 1.2)])
            {
                let x = uniform(g, lo, hi);
                *o = between(x, x + uniform(g, 0.0, 0.2))?;
            }
            Some(out)
        }),
        Class::Extreme => batch_of(|n| {
            let s = uniform(&mut g, 0.5, 1.5);
            match n % 4 {
                0 => Some([
                    between(0.0, 0.1)?,
                    between(0.9, 1.1)?,
                    between(-s, s)?,
                    between(-2.0, -1.0)?,
                ]),
                1 => Some([
                    between(0.0, 0.1)?,
                    between(0.0, s)?,
                    between(0.9, 1.1)?,
                    between(0.9, 1.1)?,
                ]),
                2 => Some([
                    between(800.0, 800.0 + s)?,
                    between(0.9, 1.1)?,
                    between(0.9, 1.1)?,
                    between(0.9, 1.1)?,
                ]),
                _ => Some([
                    between(0.0, 0.1)?,
                    between(0.9, 1.1)?,
                    between(0.1, 0.1 + s / 10.0)?,
                    between(-2.5, -2.25)?,
                ]),
            }
        }),
        _ => Vec::new(),
    }
}
