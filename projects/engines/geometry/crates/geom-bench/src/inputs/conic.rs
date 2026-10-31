//! Inputs for conics (G190): [`side`] (`Ellipse2::side`), [`meets`] (`Ellipse2::meets`),
//! [`of_ellipse`] (`Crossing2::of_ellipse`), [`against_ellipse`] (`Crossing2::against_ellipse`),
//! [`meeting`] (`Ellipse2::meeting`) and [`compare`] (`quartic::compare`).
//!
//! Classes:
//!
//! - `typical`: ellipses about points within ±2^30 steps, of an axis about as large, against points
//!   and carriers as far away; pairs on one centre, a quarter turn apart, that cross four times.
//! - `degenerate`: a vertex on its ellipse, a carrier exactly tangent to it along a Pythagorean axis,
//!   a crossing on its own ellipse, and an ellipse inside the circle on its major axis, which touches
//!   it at both ends, so the multiple roots take the whole exact path.
//! - `nearly_degenerate`: a tiny ellipse a long way from the origin, so the answer is exact and small
//!   beside the size of the coordinates and the filter cannot decide; and pairs whose major axes
//!   differ by a step, whose roots lie a hair apart.
//! - `extreme`: ellipses at the frame's edge with the longest axes and the largest ratio.
//!
//! Guarantees: each class's batch is deterministic (its seed is the operation and class name)
//! and its inputs take the path its class names, which `tests/inputs.rs` checks.
//!
//! Method: Shewchuk's input classes (<https://doi.org/10.1007/PL00009321>, section 6), built from
//! the shared draws of `inputs` as the crossing inputs are.
//!
//! Cost: a batch of up to 64 inputs, built once per benchmark run.

use geom_num::carrier::{Carrier, Rational2};
use geom_num::conic::{Conic, Ellipse2, Meeting};
use geom_num::grid::P2;
use geom_num::one_root::{Crossing2, Root};
use geom_num::quartic::{Quartic, RealRoot};

use super::{Class, batch_of, extreme2, p2, rng, typical2};
use crate::rng::SplitMix64;

/// Directions along which a length is whole.
const TRIPLES: [(i64, i64); 4] = [(3, 4), (5, 12), (8, 15), (20, 21)];

/// Reduced ratios, each below 1.
const RATIOS: [(i64, i64); 4] = [(3, 5), (5, 13), (8, 17), (20, 29)];

/// The longest axis part an ellipse may have.
const TOP: i64 = (1 << 41) - 1;

/// One of `list`.
fn pick(g: &mut SplitMix64, list: [(i64, i64); 4]) -> Option<(i64, i64)> {
    list.get(g.range(0, 3) as usize).copied()
}

/// An ellipse about a typical point, with an axis about as large.
fn typical(g: &mut SplitMix64) -> Option<Ellipse2> {
    let (p, q) = pick(g, RATIOS)?;
    let major = [g.range(1 << 20, 1 << 30), g.range(1 << 20, 1 << 30)];
    Ellipse2::new(typical2(g), major, p, q).ok()
}

/// An ellipse at the frame's edge at every budget.
fn extreme(g: &mut SplitMix64) -> Option<Ellipse2> {
    let major = [TOP, -TOP];
    Ellipse2::new(extreme2(g), major, (1 << 21) - 1, 1 << 21).ok()
}

/// An ellipse along a Pythagorean axis, of `k` times it, about `centre`: with the axis and vertex.
fn along(
    g: &mut SplitMix64,
    centre: P2,
    k: i64,
) -> Option<(Ellipse2, (i128, i128), (i128, i128))> {
    let (a, b) = pick(g, TRIPLES)?;
    let (p, q) = pick(g, RATIOS)?;
    let e = Ellipse2::new(centre, [a * k, b * k], p, q).ok()?;
    let vertex = (
        i128::from(centre.x.get() + a * k),
        i128::from(centre.y.get() + b * k),
    );
    Some((e, (i128::from(a), i128::from(b)), vertex))
}

/// [`along`] about a typical point.
fn typical_along(g: &mut SplitMix64) -> Option<(Ellipse2, (i128, i128), (i128, i128))> {
    let (centre, k) = (typical2(g), g.range(1 << 8, 1 << 20));
    along(g, centre, k)
}

/// [`along`] tiny, and a long way from the origin.
fn far_along(g: &mut SplitMix64) -> Option<(Ellipse2, (i128, i128), (i128, i128))> {
    let centre = p2(1 << 39, -(1 << 38));
    let k = g.range(1, 1 << 8);
    along(g, centre, k)
}

/// Ellipses and rational points, for the batch `op`.
#[must_use]
pub fn side_from(op: &str, class: Class) -> Vec<(Ellipse2, Rational2)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| Some((typical(&mut g)?, typical2(&mut g).into()))),
        Class::Degenerate => batch_of(|_| {
            let (e, _, (x, y)) = typical_along(&mut g)?;
            Some((e, p2(x as i64, y as i64).into()))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let (e, (a, b), (x, y)) = far_along(&mut g)?;
            let w = 1_i128 << 60;
            Some((e, Rational2::new(x * w + a, y * w + b, w).ok()?))
        }),
        Class::Extreme => batch_of(|_| Some((extreme(&mut g)?, extreme2(&mut g).into()))),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Inputs for `Ellipse2::side`.
#[must_use]
pub fn side(class: Class) -> Vec<(Ellipse2, Rational2)> {
    side_from("conic::side", class)
}

/// A carrier through the ellipse's centre.
fn across(g: &mut SplitMix64, e: &Ellipse2, other: P2) -> Option<Carrier> {
    let _ = g;
    Carrier::through(e.centre(), other).ok()
}

/// Ellipses and carriers, for the batch `op`.
#[must_use]
pub fn meets_from(op: &str, class: Class) -> Vec<(Ellipse2, Carrier)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let e = typical(&mut g)?;
            let other = typical2(&mut g);
            Some((e, across(&mut g, &e, other)?))
        }),
        Class::Degenerate => batch_of(|_| {
            let (e, (a, b), (x, y)) = typical_along(&mut g)?;
            Some((e, Carrier::new(a, b, a * x + b * y).ok()?))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let (e, (a, b), (x, y)) = far_along(&mut g)?;
            Some((e, Carrier::new(a, b, a * x + b * y - 1).ok()?))
        }),
        Class::Extreme => batch_of(|_| {
            let e = extreme(&mut g)?;
            let other = extreme2(&mut g);
            Some((e, across(&mut g, &e, other)?))
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Inputs for `Ellipse2::meets`.
#[must_use]
pub fn meets(class: Class) -> Vec<(Ellipse2, Carrier)> {
    meets_from("conic::meets", class)
}

/// Carriers and ellipses for `Crossing2::of_ellipse`.
#[must_use]
pub fn of_ellipse(class: Class) -> Vec<(Carrier, Ellipse2)> {
    meets_from("conic::of_ellipse", class)
        .into_iter()
        .map(|(e, l)| (l, e))
        .collect()
}

/// Crossings and ellipses for `Crossing2::against_ellipse`.
#[must_use]
pub fn against_ellipse(class: Class) -> Vec<(Crossing2, Ellipse2)> {
    let mut g = rng("conic::against_ellipse", class);
    let on = |e: &Ellipse2, l: &Carrier| Crossing2::of_ellipse(l, e, Root::First).ok();
    match class {
        Class::Typical => batch_of(|_| {
            let e = typical(&mut g)?;
            let other = typical2(&mut g);
            let p = on(&e, &across(&mut g, &e, other)?)?;
            Some((p, typical(&mut g)?))
        }),
        Class::Degenerate => batch_of(|_| {
            let e = typical(&mut g)?;
            let other = typical2(&mut g);
            Some((on(&e, &across(&mut g, &e, other)?)?, e))
        }),
        Class::Extreme => batch_of(|_| {
            let e = extreme(&mut g)?;
            let other = extreme2(&mut g);
            let p = on(&e, &across(&mut g, &e, other)?)?;
            Some((p, extreme(&mut g)?))
        }),
        _ => Vec::new(),
    }
}

/// Ellipses pairs for `Ellipse2::meeting`.
#[must_use]
pub fn meeting(class: Class) -> Vec<(Ellipse2, Ellipse2)> {
    let mut g = rng("conic::meeting", class);
    let turned = |e: &Ellipse2| {
        let [mx, my] = e.major();
        let (p, q) = e.ratio();
        Ellipse2::new(e.centre(), [-my, mx], p, q).ok()
    };
    match class {
        Class::Typical => batch_of(|_| {
            let e = typical(&mut g)?;
            Some((e, turned(&e)?))
        }),
        Class::Degenerate => batch_of(|_| {
            let e = typical(&mut g)?;
            Some((e, Ellipse2::new(e.centre(), e.major(), 1, 1).ok()?))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let e = typical(&mut g)?;
            let [mx, my] = e.major();
            let (p, q) = e.ratio();
            Some((e, Ellipse2::new(e.centre(), [mx, my + 1], p, q).ok()?))
        }),
        Class::Extreme => batch_of(|_| {
            let e = extreme(&mut g)?;
            Some((e, turned(&e)?))
        }),
        _ => Vec::new(),
    }
}

/// A pair's quartic and its first root.
fn first_root(pair: &(Ellipse2, Ellipse2)) -> Option<(Quartic, RealRoot)> {
    let (e, other) = pair;
    match e.meeting(&Conic::of_ellipse(other).ok()?).ok()? {
        Meeting::At { quartic, roots } => Some((quartic, *roots.iter().next()?)),
        Meeting::Same => None,
    }
}

/// Roots of two quartics for `quartic::compare`.
#[must_use]
pub fn compare(class: Class) -> Vec<(Quartic, RealRoot, Quartic, RealRoot)> {
    let pairs = meeting(match class {
        Class::Typical | Class::Degenerate | Class::Extreme => class,
        _ => return Vec::new(),
    });
    let mut out = Vec::new();
    for (i, pair) in pairs.iter().enumerate() {
        let Some((a, r)) = first_root(pair) else {
            continue;
        };
        // One root twice in the degenerate class; a neighbour's otherwise.
        let other = if class == Class::Degenerate {
            Some((a, r))
        } else {
            pairs.get(i + 1).and_then(first_root)
        };
        if let Some((b, s)) = other {
            out.push((a, r, b, s));
        }
    }
    out
}
