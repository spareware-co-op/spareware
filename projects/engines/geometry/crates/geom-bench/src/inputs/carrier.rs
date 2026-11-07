//! Inputs for 2D carriers and rational points (G187): [`side`] (`Carrier::side`), [`order`]
//! (`Carrier::order`), [`meet`] (`Carrier::meet`), [`cmp_x`] (`Rational2::cmp_x`) and [`orient`]
//! (`Rational2::orient`).
//!
//! Classes:
//!
//! - `typical`: carriers through random points within ±2^30 steps, and the points where two such
//!   carriers meet; only answers that are not zero.
//! - `degenerate`: a point on its carrier; one place at two weights; three points on one carrier.
//! - `nearly_degenerate`: a carrier moved one unit of c, a point on it a few millionths of a step off,
//!   so the answer is exact and small and the filter cannot decide by margin.
//! - `extreme`: carriers and points from grid points within 2^10 steps of the frame's edge, which
//!   reach 15.10's budgets.
//!
//! Guarantees: each class's batch is deterministic (its seed is the operation and class name)
//! and its inputs take the path its class names, which `tests/inputs.rs` checks.
//!
//! Method: Shewchuk's input classes (<https://doi.org/10.1007/PL00009321>, section 6), built from
//! the shared draws of `inputs` as the crossing inputs are.
//!
//! Cost: a batch of up to 64 inputs, built once per benchmark run.

use geom_num::carrier::{Carrier, Rational2};

use super::{Class, batch_of, extreme2, rng, typical2};
use crate::rng::SplitMix64;

/// A carrier through two typical points.
fn typical_carrier(g: &mut SplitMix64) -> Option<Carrier> {
    Carrier::through(typical2(g), typical2(g)).ok()
}

/// A carrier through two points at the frame's edge.
fn extreme_carrier(g: &mut SplitMix64) -> Option<Carrier> {
    Carrier::through(extreme2(g), extreme2(g)).ok()
}

/// The point where two typical carriers meet.
fn typical_point(g: &mut SplitMix64) -> Option<Rational2> {
    typical_carrier(g)?.meet(&typical_carrier(g)?).ok()
}

/// The point where two carriers at the frame's edge meet.
fn extreme_point(g: &mut SplitMix64) -> Option<Rational2> {
    extreme_carrier(g)?.meet(&extreme_carrier(g)?).ok()
}

/// `l` with c one more: the next line of its family, a fraction of a step away.
fn moved(l: &Carrier) -> Option<Carrier> {
    let ([a, b], c) = l.coefficients();
    Carrier::new(a, b, c.checked_add(1)?).ok()
}

/// One place at twice the weight.
fn doubled(p: &Rational2) -> Option<Rational2> {
    let ([x, y], w) = p.coordinates();
    Rational2::new(x.checked_mul(2)?, y.checked_mul(2)?, w.checked_mul(2)?).ok()
}

/// Two points on a typical carrier, a fraction of a step apart.
fn close_pair(g: &mut SplitMix64) -> Option<(Carrier, Rational2, Rational2)> {
    let (l, m) = (typical_carrier(g)?, typical_carrier(g)?);
    let (p, q) = (l.meet(&m).ok()?, l.meet(&moved(&m)?).ok()?);
    Some((l, p, q))
}

/// Carriers and points for `Carrier::side`.
#[must_use]
pub fn side(class: Class) -> Vec<(Carrier, Rational2)> {
    side_from("carrier::side", class)
}

/// [`side`]'s classes drawn from the seed of `op`.
pub(crate) fn side_from(op: &str, class: Class) -> Vec<(Carrier, Rational2)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let (l, p) = (typical_carrier(&mut g)?, typical_point(&mut g)?);
            (l.side(&p) != Ok(geom_num::Sign::Zero)).then_some((l, p))
        }),
        Class::Degenerate => batch_of(|_| {
            let (l, m) = (typical_carrier(&mut g)?, typical_carrier(&mut g)?);
            Some((l, l.meet(&m).ok()?))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let (l, m) = (typical_carrier(&mut g)?, typical_carrier(&mut g)?);
            Some((moved(&l)?, l.meet(&m).ok()?))
        }),
        Class::Extreme => batch_of(|_| Some((extreme_carrier(&mut g)?, extreme_point(&mut g)?))),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// A carrier and two points for `Carrier::order`.
#[must_use]
pub fn order(class: Class) -> Vec<(Carrier, Rational2, Rational2)> {
    order_from("carrier::order", class)
}

/// [`order`]'s classes drawn from the seed of `op`.
pub(crate) fn order_from(op: &str, class: Class) -> Vec<(Carrier, Rational2, Rational2)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let l = typical_carrier(&mut g)?;
            let (p, q) = (typical_point(&mut g)?, typical_point(&mut g)?);
            (l.order(&p, &q) != Ok(geom_num::Sign::Zero)).then_some((l, p, q))
        }),
        Class::Degenerate => batch_of(|_| {
            let (l, p) = (typical_carrier(&mut g)?, typical_point(&mut g)?);
            Some((l, p, doubled(&p)?))
        }),
        Class::NearlyDegenerate => batch_of(|_| close_pair(&mut g)),
        Class::Extreme => batch_of(|_| {
            Some((
                extreme_carrier(&mut g)?,
                extreme_point(&mut g)?,
                extreme_point(&mut g)?,
            ))
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Carrier pairs for `Carrier::meet`.
#[must_use]
pub fn meet(class: Class) -> Vec<(Carrier, Carrier)> {
    let mut g = rng("carrier::meet", class);
    match class {
        Class::Typical => batch_of(|_| Some((typical_carrier(&mut g)?, typical_carrier(&mut g)?))),
        Class::Degenerate => batch_of(|_| {
            let l = typical_carrier(&mut g)?;
            Some((l, moved(&l)?))
        }),
        Class::Extreme => batch_of(|_| Some((extreme_carrier(&mut g)?, extreme_carrier(&mut g)?))),
        _ => Vec::new(),
    }
}

/// Point pairs for `Rational2::cmp_x`.
#[must_use]
pub fn cmp_x(class: Class) -> Vec<(Rational2, Rational2)> {
    cmp_from("carrier::cmp_x", class)
}

/// [`cmp_x`]'s classes drawn from the seed of `op`.
pub(crate) fn cmp_from(op: &str, class: Class) -> Vec<(Rational2, Rational2)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let (p, q) = (typical_point(&mut g)?, typical_point(&mut g)?);
            (p.cmp_x(&q) != Ok(geom_num::Sign::Zero)).then_some((p, q))
        }),
        Class::Degenerate => batch_of(|_| {
            let p = typical_point(&mut g)?;
            Some((p, doubled(&p)?))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let (_, p, q) = close_pair(&mut g)?;
            Some((p, q))
        }),
        Class::Extreme => batch_of(|_| Some((extreme_point(&mut g)?, extreme_point(&mut g)?))),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Point triples for `Rational2::orient`.
#[must_use]
pub fn orient(class: Class) -> Vec<[Rational2; 3]> {
    orient_from("carrier::orient", class)
}

/// [`orient`]'s classes drawn from the seed of `op`.
pub(crate) fn orient_from(op: &str, class: Class) -> Vec<[Rational2; 3]> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let t @ [p, q, r] = [
                typical_point(&mut g)?,
                typical_point(&mut g)?,
                typical_point(&mut g)?,
            ];
            (Rational2::orient(&p, &q, &r) != Ok(geom_num::Sign::Zero)).then_some(t)
        }),
        Class::Degenerate => batch_of(|_| {
            let l = typical_carrier(&mut g)?;
            Some([
                l.meet(&typical_carrier(&mut g)?).ok()?,
                l.meet(&typical_carrier(&mut g)?).ok()?,
                l.meet(&typical_carrier(&mut g)?).ok()?,
            ])
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let l = typical_carrier(&mut g)?;
            Some([
                l.meet(&typical_carrier(&mut g)?).ok()?,
                l.meet(&typical_carrier(&mut g)?).ok()?,
                moved(&l)?.meet(&typical_carrier(&mut g)?).ok()?,
            ])
        }),
        Class::Extreme => batch_of(|_| {
            Some([
                extreme_point(&mut g)?,
                extreme_point(&mut g)?,
                extreme_point(&mut g)?,
            ])
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}
