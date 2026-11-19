//! Inputs for 2D circles and arcs (G188): [`round_grid`] (`Round::side_of_grid`), [`round_side`]
//! (`Round::side`), [`round_meets`] (`Round::meets`), [`circle_side`] (`Circle2::side`),
//! [`circle_meets`] (`Circle2::meets`) and [`circle_gap`] (`generated::circle_gap`, which
//! `Circle2::against` calls).
//!
//! Classes:
//!
//! - `typical`: circles about points within ±2^30 steps, of a radius about as large, against points
//!   and carriers as far away; only answers that are not zero.
//! - `degenerate`: a point on its circle, and a carrier exactly tangent to it, by a Pythagorean
//!   triple.
//! - `nearly_degenerate`: a tiny circle a long way from the origin, or two near neighbours, so the
//!   answer is exact and small beside the size of the coordinates and the filter cannot decide.
//! - `extreme`: points at the frame's edge, and carriers and rational centres at their budgets.
//!
//! Guarantees: each class's batch is deterministic (its seed is the operation and class name)
//! and its inputs take the path its class names, which `tests/inputs.rs` checks.
//!
//! Method: Shewchuk's input classes (<https://doi.org/10.1007/PL00009321>, section 6), built from
//! the shared draws of `inputs` as the crossing inputs are.
//!
//! Cost: a batch of up to 64 inputs, built once per benchmark run.

use geom_num::Sign;
use geom_num::arc::{Circle2, Round};
use geom_num::carrier::{Carrier, Rational2};
use geom_num::grid::P2;

use super::{Class, batch_of, extreme2, p2, rng, typical2, within};
use crate::rng::SplitMix64;

/// A grid point as raw step counts.
fn raw(p: P2) -> [i64; 2] {
    [p.x.get(), p.y.get()]
}

/// The point where two typical carriers meet: a rational centre.
fn typical_centre(g: &mut SplitMix64) -> Option<Rational2> {
    let first = Carrier::through(typical2(g), typical2(g)).ok()?;
    let second = Carrier::through(typical2(g), typical2(g)).ok()?;
    first.meet(&second).ok()
}

/// The point where two carriers at the frame's edge meet.
fn extreme_centre(g: &mut SplitMix64) -> Option<Rational2> {
    let first = Carrier::through(extreme2(g), extreme2(g)).ok()?;
    let second = Carrier::through(extreme2(g), extreme2(g)).ok()?;
    first.meet(&second).ok()
}

/// A tiny circle a long way out: a rational centre a few steps from a typical point, through it.
fn tiny_round(g: &mut SplitMix64) -> Option<(P2, Round)> {
    let on = typical2(g);
    let [x, y] = raw(on).map(i128::from);
    let weight = i128::from(g.range(3, 50));
    let (dx, dy) = (i128::from(within(g, 40)), i128::from(within(g, 40)));
    let centre = Rational2::new(weight * x + dx, weight * y + dy, weight).ok()?;
    Some((on, Round::new(on, centre)))
}

/// A circle of a Pythagorean radius, and the ray to a point of it: (3k, 4k) from its centre.
fn pythagorean(g: &mut SplitMix64) -> Option<(P2, i64)> {
    let k = g.range(1, 1 << 20);
    Some((typical2(g), k))
}

/// Grid points against rounds, for `Round::side_of_grid`.
#[must_use]
pub fn round_grid(class: Class) -> Vec<(Round, P2)> {
    round_grid_from("arc::round_grid", class)
}

/// [`round_grid`]'s classes drawn from the seed of `op`.
pub(crate) fn round_grid_from(op: &str, class: Class) -> Vec<(Round, P2)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let (r, z) = (
                Round::new(typical2(&mut g), typical_centre(&mut g)?),
                typical2(&mut g),
            );
            (r.side_of_grid(z) != Ok(Sign::Zero)).then_some((r, z))
        }),
        Class::Degenerate => batch_of(|_| {
            let (c, k) = pythagorean(&mut g)?;
            let [x, y] = raw(c);
            Some((
                Round::new(p2(x + 3 * k, y + 4 * k), c.into()),
                p2(x - 4 * k, y + 3 * k),
            ))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let (on, r) = tiny_round(&mut g)?;
            let [x, y] = raw(on);
            Some((r, p2(x + g.range(-3, 3), y + g.range(1, 3))))
        }),
        Class::Extreme => batch_of(|_| {
            Some((
                Round::new(extreme2(&mut g), extreme_centre(&mut g)?),
                extreme2(&mut g),
            ))
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Rational points against rounds, for `Round::side`.
#[must_use]
pub fn round_side(class: Class) -> Vec<(Round, Rational2)> {
    round_side_from("arc::round_side", class)
}

/// [`round_side`]'s classes drawn from the seed of `op`: [`round_grid`]'s, each point made rational
/// at a weight of its own, and in general position a meeting of carriers.
pub(crate) fn round_side_from(op: &str, class: Class) -> Vec<(Round, Rational2)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let (r, z) = (
                Round::new(typical2(&mut g), typical_centre(&mut g)?),
                typical_centre(&mut g)?,
            );
            (r.side(&z) != Ok(Sign::Zero)).then_some((r, z))
        }),
        Class::Extreme => batch_of(|_| {
            Some((
                Round::new(extreme2(&mut g), extreme_centre(&mut g)?),
                extreme_centre(&mut g)?,
            ))
        }),
        _ => round_grid_from(op, class)
            .into_iter()
            .map(|(r, z)| (r, z.into()))
            .collect(),
    }
}

/// Carriers against rounds, for `Round::meets`.
#[must_use]
pub fn round_meets(class: Class) -> Vec<(Round, Carrier)> {
    round_meets_from("arc::round_meets", class)
}

/// [`round_meets`]'s classes drawn from the seed of `op`.
pub(crate) fn round_meets_from(op: &str, class: Class) -> Vec<(Round, Carrier)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let r = Round::new(typical2(&mut g), typical_centre(&mut g)?);
            Some((
                r,
                Carrier::through(typical2(&mut g), typical2(&mut g)).ok()?,
            ))
        }),
        Class::Degenerate => batch_of(|_| {
            let (c, k) = pythagorean(&mut g)?;
            let [x, y] = raw(c).map(i128::from);
            let k = i128::from(k);
            // 3x + 4y = 3cx + 4cy + 25k touches the circle of radius 5k at (3k, 4k) from c.
            let l = Carrier::new(3, 4, 3 * x + 4 * y + 25 * k).ok()?;
            let on = p2((x + 3 * k) as i64, (y + 4 * k) as i64);
            Some((Round::new(on, c.into()), l))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let (on, r) = tiny_round(&mut g)?;
            let [x, y] = raw(on).map(i128::from);
            Some((
                r,
                Carrier::new(1, 0, x + i128::from(g.range(-2, 2)) + y - y).ok()?,
            ))
        }),
        Class::Extreme => batch_of(|_| {
            let r = Round::new(extreme2(&mut g), extreme_centre(&mut g)?);
            Some((
                r,
                Carrier::through(extreme2(&mut g), extreme2(&mut g)).ok()?,
            ))
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Rational points against grid-centred circles, for `Circle2::side`.
#[must_use]
pub fn circle_side(class: Class) -> Vec<(Circle2, Rational2)> {
    circle_side_from("arc::circle_side", class)
}

/// [`circle_side`]'s classes drawn from the seed of `op`.
pub(crate) fn circle_side_from(op: &str, class: Class) -> Vec<(Circle2, Rational2)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let c = Circle2::new(typical2(&mut g), i128::from(g.range(1, 1 << 30))).ok()?;
            let z = typical_centre(&mut g)?;
            (c.side(&z) != Ok(Sign::Zero)).then_some((c, z))
        }),
        Class::Degenerate => batch_of(|_| {
            let (c, k) = pythagorean(&mut g)?;
            let [x, y] = raw(c);
            let on: Rational2 = p2(x + 3 * k, y - 4 * k).into();
            Some((Circle2::new(c, i128::from(5 * k)).ok()?, on))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let c = typical2(&mut g);
            let [x, y] = raw(c);
            let near: Rational2 = p2(x + g.range(-4, 4), y + g.range(-4, 4)).into();
            Some((Circle2::new(c, i128::from(g.range(1, 6))).ok()?, near))
        }),
        Class::Extreme => batch_of(|_| {
            let c = Circle2::new(extreme2(&mut g), (1 << 41) - 1).ok()?;
            Some((c, extreme_centre(&mut g)?))
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Carriers against grid-centred circles, for `Circle2::meets`.
#[must_use]
pub fn circle_meets(class: Class) -> Vec<(Circle2, Carrier)> {
    circle_meets_from("arc::circle_meets", class)
}

/// [`circle_meets`]'s classes drawn from the seed of `op`.
pub(crate) fn circle_meets_from(op: &str, class: Class) -> Vec<(Circle2, Carrier)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let c = Circle2::new(typical2(&mut g), i128::from(g.range(1, 1 << 30))).ok()?;
            Some((
                c,
                Carrier::through(typical2(&mut g), typical2(&mut g)).ok()?,
            ))
        }),
        Class::Degenerate => batch_of(|_| {
            let (c, k) = pythagorean(&mut g)?;
            let [x, y] = raw(c).map(i128::from);
            let k = i128::from(k);
            let l = Carrier::new(3, 4, 3 * x + 4 * y + 25 * k).ok()?;
            Some((Circle2::new(c, 5 * k).ok()?, l))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let c = typical2(&mut g);
            let [x, y] = raw(c).map(i128::from);
            let l = Carrier::new(1, 1, x + y + i128::from(g.range(-5, 5))).ok()?;
            Some((Circle2::new(c, i128::from(g.range(1, 4))).ok()?, l))
        }),
        Class::Extreme => batch_of(|_| {
            let c = Circle2::new(extreme2(&mut g), (1 << 41) - 1).ok()?;
            Some((
                c,
                Carrier::through(extreme2(&mut g), extreme2(&mut g)).ok()?,
            ))
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Two grid points and a length, for `generated::circle_gap`.
pub(crate) fn gap_from(op: &str, class: Class) -> Vec<(P2, P2, i128)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let (a, b) = (typical2(&mut g), typical2(&mut g));
            Some((a, b, i128::from(within(&mut g, 1 << 20))))
        }),
        Class::Degenerate => batch_of(|_| {
            let (c, k) = pythagorean(&mut g)?;
            let [x, y] = raw(c);
            Some((c, p2(x + 3 * k, y + 4 * k), i128::from(5 * k)))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            // 1 + k² − k² = 1, beside squares of 2^60: no double holds the difference.
            let c = typical2(&mut g);
            let [x, y] = raw(c);
            let k = g.range(1 << 28, 1 << 30);
            Some((c, p2(x + 1, y + k), i128::from(k)))
        }),
        Class::Extreme => {
            batch_of(|_| Some((extreme2(&mut g), extreme2(&mut g), (1_i128 << 42) - 1)))
        }
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}
