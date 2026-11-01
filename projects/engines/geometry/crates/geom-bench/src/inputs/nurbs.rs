//! Inputs for `geom-nurbs`'s hot paths (pieces G56, G58 and G59, decision 32.44): parameters for
//! evaluating a clamped cubic curve, point sets for its interpolation, and interval control points
//! for the Bernstein bound.
//!
//! Class `typical` only:
//!
//! - Parameters: drawn from the curve's domain [0, 17] at 2^−48 resolution
//!   ([`cubic`] is the curve).
//! - Point sets: twelve points one unit apart in x, each y and z within ±8 of a slow ramp, so
//!   consecutive points are distinct and chord-length parameters increase.
//! - Interval control points: four, each coordinate within ±2^10 and between a point and an
//!   interval up to 2^−20 wide, as a certified curve's coefficients are.
//!
//! The operations' refusals (a parameter outside the domain, too few points, a weight not
//! positive) are tested in `geom-nurbs` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the cases of `geom-nurbs`'s benchmarks (`benches/nurbs.rs`, `fit.rs` and
//! `bernstein.rs`): a clamped cubic, chord-length interpolation by version 1 of the fit, and the
//! bound of a cubic Bernstein piece, at one size each.
//! Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few draws of [`SplitMix64`] per value, once per batch, outside every measured run.

use geom_num::interval::Interval;

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;
use geom_nurbs::{Basis, Curve};

/// A value in [0, 1) at 2^−52 resolution.
fn unit(g: &mut SplitMix64) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "the value is below 2^53, so the conversion is exact"
    )]
    let v = (g.next_u64() >> 12) as f64;
    v / (1u64 << 52) as f64
}

/// Parameters for `Curve::point` on [`cubic`], in its domain [0, 17].
#[must_use]
pub fn curve_point(class: Class) -> Vec<f64> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("nurbs::curve_point", class);
    batch_of(|_| Some(unit(&mut g) * 17.0))
}

/// Twelve-point sets for `Curve::interpolate`.
#[must_use]
pub fn interpolate(class: Class) -> Vec<Vec<[f64; 3]>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("nurbs::interpolate", class);
    batch_of(|_| {
        Some(
            (0..12u32)
                .map(|k| {
                    let x = f64::from(k);
                    let y = 0.5 * x + 16.0 * unit(&mut g) - 8.0;
                    let z = 16.0 * unit(&mut g) - 8.0;
                    [x, y, z]
                })
                .collect(),
        )
    })
}

/// Four interval control points for `BernsteinCurve::bound`.
#[must_use]
pub fn bernstein_bound(class: Class) -> Vec<Vec<[Interval; 3]>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("nurbs::bernstein_bound", class);
    batch_of(|_| {
        let mut points = Vec::with_capacity(4);
        for _ in 0..4 {
            let mut coordinate = || {
                let mid = 2048.0 * unit(&mut g) - 1024.0;
                let width = unit(&mut g) / (1u64 << 20) as f64;
                Interval::new(mid, mid + width).ok()
            };
            points.push([coordinate()?, coordinate()?, coordinate()?]);
        }
        Some(points)
    })
}

/// The curve [`curve_point`] evaluates: cubic, 20 control points on a slow curve, clamped
/// uniform knots, domain [0, 17].
#[must_use]
pub fn cubic() -> Option<Curve<3>> {
    let mut knots = vec![0.0; 4];
    knots.extend((1..=16u32).map(f64::from));
    knots.extend([17.0; 4]);
    let basis = Basis::new(3, knots).ok()?;
    let points: Vec<[f64; 3]> = (0..20u32)
        .map(|i| {
            let x = f64::from(i);
            [x, 0.01 * x * x, 5.0 - 0.25 * x]
        })
        .collect();
    Curve::new(basis, &points).ok()
}
