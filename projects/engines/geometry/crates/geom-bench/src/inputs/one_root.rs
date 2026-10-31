//! Inputs for one-root numbers (G189): [`of_circle`] (`Crossing2::of_circle`), [`of_round`]
//! (`Crossing2::of_round`), [`cmp_x`] (`Crossing2::cmp_x`), [`side`] (`Crossing2::side`),
//! [`against_circle`] (`Crossing2::against_circle`) and [`order_crossings`]
//! (`Arc2::order_crossings`).
//!
//! Classes:
//!
//! - `typical`: circles about points within ±2^30 steps, with carriers across them, whose crossings
//!   lie far apart, so the filter decides.
//! - `degenerate`: one place reached two ways, by a Pythagorean triple, or a crossing on its own
//!   carrier or circle, so the exact path runs and the answer is zero.
//! - `nearly_degenerate`: a carrier a step from tangent, so two crossings lie a hair apart and the
//!   filter cannot decide.
//! - `extreme`: circles at the frame's edge with the largest radius, and carriers at their budgets.
//!
//! Guarantees: each class's batch is deterministic (its seed is the operation and class name)
//! and its inputs take the path its class names, which `tests/inputs.rs` checks.
//!
//! Method: Shewchuk's input classes (<https://doi.org/10.1007/PL00009321>, section 6), built from
//! the shared draws of `inputs` as the crossing inputs are.
//!
//! Cost: a batch of up to 64 inputs, built once per benchmark run.

use geom_num::arc::{Arc2, Circle2, Round};
use geom_num::carrier::Carrier;
use geom_num::grid::P2;
use geom_num::one_root::{Crossing2, Root};

use super::{Class, batch_of, extreme2, p2, rng, typical2};
use crate::rng::SplitMix64;

/// A circle about a typical point, a carrier through its centre's neighbourhood, and which meet.
fn typical_pair(g: &mut SplitMix64) -> Option<(Carrier, Circle2)> {
    let centre = typical2(g);
    let radius = i128::from(g.range(1 << 20, 1 << 30));
    let circle = Circle2::new(centre, radius).ok()?;
    let near = p2(
        centre.x.get() + g.range(-9, 9),
        centre.y.get() + g.range(-9, 9),
    );
    let l = Carrier::through(near, typical2(g)).ok()?;
    Crossing2::of_circle(&l, &circle, Root::First).ok()?;
    Some((l, circle))
}

/// A circle at the frame's edge with the largest radius, and a carrier across it.
fn extreme_pair(g: &mut SplitMix64) -> Option<(Carrier, Circle2)> {
    let centre = extreme2(g);
    let circle = Circle2::new(centre, (1 << 41) - 1).ok()?;
    let l = Carrier::through(centre, extreme2(g)).ok()?;
    Crossing2::of_circle(&l, &circle, Root::First).ok()?;
    Some((l, circle))
}

/// A circle of radius 5k about a typical point, and the carrier tangent to it at (3k, 4k), moved in
/// by `by`.
fn tangent(g: &mut SplitMix64, by: i128) -> Option<(Carrier, Circle2)> {
    let (centre, k) = (typical2(g), i128::from(g.range(1 << 8, 1 << 20)));
    let (x, y) = (i128::from(centre.x.get()), i128::from(centre.y.get()));
    let l = Carrier::new(3, 4, 3 * x + 4 * y + 25 * k - by).ok()?;
    Some((l, Circle2::new(centre, 5 * k).ok()?))
}

/// Both crossings of a pair.
fn both(l: &Carrier, c: &Circle2) -> Option<(Crossing2, Crossing2)> {
    Some((
        Crossing2::of_circle(l, c, Root::First).ok()?,
        Crossing2::of_circle(l, c, Root::Second).ok()?,
    ))
}

/// Carriers and circles for `Crossing2::of_circle`.
#[must_use]
pub fn of_circle(class: Class) -> Vec<(Carrier, Circle2)> {
    let mut g = rng("one_root::of_circle", class);
    match class {
        Class::Typical => batch_of(|_| typical_pair(&mut g)),
        Class::Degenerate => batch_of(|_| tangent(&mut g, 0)),
        Class::Extreme => batch_of(|_| extreme_pair(&mut g)),
        _ => Vec::new(),
    }
}

/// Carriers and rounds for `Crossing2::of_round`.
#[must_use]
pub fn of_round(class: Class) -> Vec<(Carrier, Round)> {
    let mut g = rng("one_root::of_round", class);
    let round = |(l, c): (Carrier, Circle2)| {
        let on = p2(c.centre().x.get() + c.radius() as i64, c.centre().y.get());
        (l, Round::new(on, c.centre().into()))
    };
    match class {
        Class::Typical => batch_of(|_| typical_pair(&mut g).map(round)),
        Class::Degenerate => batch_of(|_| tangent(&mut g, 0).map(round)),
        _ => Vec::new(),
    }
}

/// Crossing pairs for `Crossing2::cmp_x`.
#[must_use]
pub fn cmp_x(class: Class) -> Vec<(Crossing2, Crossing2)> {
    let mut g = rng("one_root::cmp_x", class);
    match class {
        Class::Typical => batch_of(|_| {
            let (l, c) = typical_pair(&mut g)?;
            both(&l, &c)
        }),
        Class::Degenerate => batch_of(|_| {
            // (3k, 4k) from the centre, on two carriers.
            let (l, c) = tangent(&mut g, 0)?;
            let (p, _) = both(&l, &c)?;
            let (x, y) = (
                i128::from(c.centre().x.get()),
                i128::from(c.centre().y.get()),
            );
            let m = Carrier::new(4, -3, 4 * x - 3 * y).ok()?;
            let q = Crossing2::of_circle(&m, &c, Root::First).ok()?;
            let r = Crossing2::of_circle(&m, &c, Root::Second).ok()?;
            let same = |a: &Crossing2, b: &Crossing2| a.same(b).unwrap_or(false);
            Some((p, if same(&p, &q) { q } else { r }))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let (l, c) = tangent(&mut g, 1)?;
            both(&l, &c)
        }),
        Class::Extreme => batch_of(|_| {
            let (l, c) = extreme_pair(&mut g)?;
            both(&l, &c)
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Crossings and carriers for `Crossing2::side`.
#[must_use]
pub fn side(class: Class) -> Vec<(Crossing2, Carrier)> {
    let mut g = rng("one_root::side", class);
    match class {
        Class::Typical => batch_of(|_| {
            let (l, c) = typical_pair(&mut g)?;
            let (p, _) = both(&l, &c)?;
            Some((
                p,
                Carrier::through(typical2(&mut g), typical2(&mut g)).ok()?,
            ))
        }),
        Class::Degenerate => batch_of(|_| {
            let (l, c) = typical_pair(&mut g)?;
            let (p, _) = both(&l, &c)?;
            Some((p, l))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let (l, c) = typical_pair(&mut g)?;
            let (p, _) = both(&l, &c)?;
            let ([a, b], d) = l.coefficients();
            Some((p, Carrier::new(a, b, d + 1).ok()?))
        }),
        Class::Extreme => batch_of(|_| {
            let (l, c) = extreme_pair(&mut g)?;
            let (p, _) = both(&l, &c)?;
            Some((
                p,
                Carrier::through(extreme2(&mut g), extreme2(&mut g)).ok()?,
            ))
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Crossings and circles for `Crossing2::against_circle`.
#[must_use]
pub fn against_circle(class: Class) -> Vec<(Crossing2, Circle2)> {
    let mut g = rng("one_root::against_circle", class);
    match class {
        Class::Typical => batch_of(|_| {
            let (l, c) = typical_pair(&mut g)?;
            let (p, _) = both(&l, &c)?;
            let d = Circle2::new(typical2(&mut g), i128::from(g.range(1, 1 << 30))).ok()?;
            Some((p, d))
        }),
        Class::Degenerate => batch_of(|_| {
            let (l, c) = typical_pair(&mut g)?;
            let (p, _) = both(&l, &c)?;
            Some((p, c))
        }),
        Class::Extreme => batch_of(|_| {
            let (l, c) = extreme_pair(&mut g)?;
            let (p, _) = both(&l, &c)?;
            Some((p, Circle2::new(extreme2(&mut g), (1 << 41) - 1).ok()?))
        }),
        _ => Vec::new(),
    }
}

/// An arc and two crossings on its circle, for `Arc2::order_crossings`.
#[must_use]
pub fn order_crossings(class: Class) -> Vec<(Arc2, Crossing2, Crossing2)> {
    let mut g = rng("one_root::order_crossings", class);
    match class {
        Class::Typical => batch_of(|_| {
            let (centre, k) = (typical2(&mut g), g.range(1 << 8, 1 << 20));
            let (x, y) = (centre.x.get(), centre.y.get());
            let at = |u: i64, v: i64| -> P2 { p2(x + u * k, y + v * k) };
            let arc = Arc2::new(at(5, 0), at(-5, 0), centre.into()).ok()?;
            let round = arc.round();
            let cross = |a: i128, b: i128, d: i128, root| {
                let (cx, cy) = (i128::from(x), i128::from(y));
                let l = Carrier::new(a, b, a * cx + b * cy + d * i128::from(k)).ok()?;
                Crossing2::of_round(&l, &round, root).ok()
            };
            Some((
                arc,
                cross(0, 1, 1, Root::First)?,
                cross(1, 0, 1, Root::First)?,
            ))
        }),
        _ => Vec::new(),
    }
}
