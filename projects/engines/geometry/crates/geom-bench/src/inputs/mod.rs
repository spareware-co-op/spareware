//! The inputs every benchmark runs on: fixed batches, built the same way on every target.
//!
//! Each operation is measured on a batch of [`BATCH`] inputs from one [`Class`]. A batch is built
//! from a fixed seed ([`crate::rng`]), so the same work is measured on every run and every machine,
//! and each class is checked to be what it claims by this crate's tests (`tests/inputs.rs`): for
//! example every degenerate predicate input really gives an exact zero, every nearly degenerate one
//! a nonzero sign too small for a floating-point filter to certify, and every overflow input really
//! overflows. The golden digests pin every batch, so an input cannot change unnoticed.
//!
//! Method: the input categories robust-geometry work tests and times separately, because each takes
//! a different path through the code: general position, nearly degenerate, exactly degenerate (all
//! three are Shewchuk's, 1997, section 6: <https://doi.org/10.1007/PL00009321>), and the extremes of
//! the number range. Exactly degenerate points on circles and spheres come from Pythagorean triples
//! and quadruples (<https://en.wikipedia.org/wiki/Pythagorean_triple>,
//! <https://en.wikipedia.org/wiki/Pythagorean_quadruple>): integer points at one exact distance
//! from a centre. Nearly degenerate triangles and tetrahedra with huge coordinates are unimodular
//! lattice bases (<https://en.wikipedia.org/wiki/Unimodular_matrix>), built with the extended
//! Euclidean algorithm (Knuth, *The Art of Computer Programming*, volume 2, section 4.5.2): their
//! area or volume is the smallest nonzero one the grid allows.
//!
//! Guarantees: every batch is a pure function of its operation and class, the same on every
//! target and in every build profile; every catalogued batch is full ([`BATCH`] inputs; the tests
//! check it) and is what its class claims; a class an operation lacks gives an empty batch; and no
//! builder panics.
//!
//! Cost: building a batch takes O([`BATCH`]) generator draws (at most `ATTEMPTS` = 64 · [`BATCH`]
//! candidates, so it always ends), once per benchmark, outside every measured run.

use core::fmt;

use geom_num::grid::{COORD_MAX, Coord, P2, P3};

use crate::rng::{SplitMix64, seed_for};

pub mod crossing;
pub mod dd;
pub mod eft;
pub mod exact_sum;
pub mod exactness;
pub mod generated;
pub mod grid;
pub mod growth;
pub mod interval;
pub mod math;
pub mod plane;
pub mod predicate;
pub mod real;
pub mod rng;
pub mod sign;
pub mod wide;
pub mod wide32;

/// Inputs per batch. Large enough that the call and loop around the batch are a small share of
/// each count, small enough that every batch and its results fit the simulated 32 KiB first-level
/// data cache.
pub const BATCH: usize = 64;

/// How many candidates a builder may try before giving up on filling its batch. Builders reject a
/// candidate only in rare cases (for example three random points that happen to be collinear), so
/// this bound is never approached; the tests check that every batch is full.
const ATTEMPTS: usize = 64 * BATCH;

/// Half the side of the box typical coordinates come from: 2^30 grid steps, about ±168 m at the
/// building step Q0 (156.25 nm). A building or a site.
pub const TYPICAL_REACH: i64 = 1 << 30;

/// How far below the largest magnitude an extreme coordinate may be: extreme coordinates are
/// within 2^10 steps of ±(2^40 − 1).
pub const EXTREME_SLACK: i64 = 1 << 10;

/// The largest coordinate magnitude, 2^40 − 1.
pub(crate) const M: i64 = COORD_MAX;

/// What kind of inputs a batch holds. Each operation has the classes that take a distinct path
/// through it; asking for a class an operation does not have gives an empty batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    /// General position at everyday scale: coordinates within ±2^30 grid steps, doubles of
    /// ordinary size. The answer is never an exact zero.
    Typical,
    /// Exactly degenerate: collinear, coplanar, cocircular or cospherical points, a point on a
    /// plane, planes that meet in no single point. The answer is an exact zero or a `Degenerate`
    /// error: the case a floating-point filter can never settle.
    Degenerate,
    /// Nearly degenerate: the answer is not zero, but it is the smallest nonzero value the grid
    /// allows next to terms many orders of magnitude larger (a point one step off a line, circle,
    /// plane or sphere; a triangle of the smallest area with huge coordinates), so a floating-point
    /// filter cannot certify its sign and the exact path decides it.
    NearlyDegenerate,
    /// The largest magnitudes the types allow: coordinates within 2^10 steps of ±(2^40 − 1) (with
    /// the configurations that reach each predicate's largest value first), full-width integers,
    /// doubles near overflow and underflow, subnormals.
    Extreme,
    /// The exact result does not fit: a checked operation returns `None`, or an `OutOfRange` error.
    Overflow,
    /// Three planes that meet exactly on a grid point, so committing the point rounds nothing.
    OnGrid,
    /// A grid-step conversion asked to go the wrong way: the `WrongDirection` error.
    WrongDirection,
}

impl Class {
    /// Every class, in a fixed order.
    pub const ALL: [Class; 7] = [
        Class::Typical,
        Class::Degenerate,
        Class::NearlyDegenerate,
        Class::Extreme,
        Class::Overflow,
        Class::OnGrid,
        Class::WrongDirection,
    ];

    /// The class's name in benchmark identifiers: `typical`, `degenerate`, `nearly_degenerate`,
    /// `extreme`, `overflow`, `on_grid` or `wrong_direction`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Class::Typical => "typical",
            Class::Degenerate => "degenerate",
            Class::NearlyDegenerate => "nearly_degenerate",
            Class::Extreme => "extreme",
            Class::Overflow => "overflow",
            Class::OnGrid => "on_grid",
            Class::WrongDirection => "wrong_direction",
        }
    }

    /// The class with this name, if there is one.
    #[must_use]
    pub fn named(name: &str) -> Option<Class> {
        Class::ALL.into_iter().find(|class| class.name() == name)
    }
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The generator for operation `op`'s batch of class `class`.
pub(crate) fn rng(op: &str, class: Class) -> SplitMix64 {
    SplitMix64::new(seed_for(op, class.name()))
}

/// Up to [`BATCH`] items from `make`, which is told how many items are already accepted and returns
/// `None` to reject a candidate. Tries at most [`ATTEMPTS`] times, so it always ends.
pub(crate) fn batch_of<T>(mut make: impl FnMut(usize) -> Option<T>) -> Vec<T> {
    let mut out = Vec::with_capacity(BATCH);
    for _ in 0..ATTEMPTS {
        if out.len() == BATCH {
            break;
        }
        if let Some(item) = make(out.len()) {
            out.push(item);
        }
    }
    out
}

/// A coordinate of `v` grid steps, saturated into the frame. Every builder computes coordinates
/// inside the frame, so the saturation never acts; it only lets the conversion be total.
pub(crate) fn coord(v: i64) -> Coord {
    Coord::new(v.clamp(-M, M)).unwrap_or(Coord::ZERO)
}

/// A 2D grid point from raw step counts (saturated into the frame, see [`coord`]).
pub(crate) fn p2(x: i64, y: i64) -> P2 {
    P2 {
        x: coord(x),
        y: coord(y),
    }
}

/// A 3D grid point from raw step counts (saturated into the frame, see [`coord`]).
pub(crate) fn p3(x: i64, y: i64, z: i64) -> P3 {
    P3 {
        x: coord(x),
        y: coord(y),
        z: coord(z),
    }
}

/// A coordinate within ±`reach` steps.
pub(crate) fn within(g: &mut SplitMix64, reach: i64) -> i64 {
    g.range(-reach, reach)
}

/// A coordinate within [`EXTREME_SLACK`] steps of +M or −M.
pub(crate) fn extreme(g: &mut SplitMix64) -> i64 {
    g.sign() * (M - g.range(0, EXTREME_SLACK))
}

/// A typical 2D point.
pub(crate) fn typical2(g: &mut SplitMix64) -> P2 {
    p2(within(g, TYPICAL_REACH), within(g, TYPICAL_REACH))
}

/// A typical 3D point.
pub(crate) fn typical3(g: &mut SplitMix64) -> P3 {
    p3(
        within(g, TYPICAL_REACH),
        within(g, TYPICAL_REACH),
        within(g, TYPICAL_REACH),
    )
}

/// An extreme 2D point.
pub(crate) fn extreme2(g: &mut SplitMix64) -> P2 {
    p2(extreme(g), extreme(g))
}

/// An extreme 3D point.
pub(crate) fn extreme3(g: &mut SplitMix64) -> P3 {
    p3(extreme(g), extreme(g), extreme(g))
}

/// A pseudo-random element of `items`, or `None` when it is empty.
pub(crate) fn pick<T: Copy>(g: &mut SplitMix64, items: &[T]) -> Option<T> {
    items.get(g.index(items.len())).copied()
}

/// Removes and returns a pseudo-random element of `pool`, or `None` when it is empty.
pub(crate) fn take<T>(g: &mut SplitMix64, pool: &mut Vec<T>) -> Option<T> {
    let i = g.index(pool.len());
    (i < pool.len()).then(|| pool.swap_remove(i))
}

/// The extended Euclidean algorithm (Knuth, volume 2, section 4.5.2, algorithm X): for a, b not
/// both zero, `(g, u, v)` with g = gcd(a, b) ≥ 0 and a·u + b·v = g. Every intermediate is bounded
/// by max(|a|, |b|), so nothing overflows for inputs inside the frame.
pub(crate) fn extended_gcd(a: i64, b: i64) -> (i64, i64, i64) {
    let (mut r0, mut r1) = (a, b);
    let (mut u0, mut u1) = (1i64, 0i64);
    let (mut v0, mut v1) = (0i64, 1i64);
    while r1 != 0 {
        let q = r0.div_euclid(r1);
        (r0, r1) = (r1, r0 - q * r1);
        (u0, u1) = (u1, u0 - q * u1);
        (v0, v1) = (v1, v0 - q * v1);
    }
    if r0 < 0 {
        (-r0, -u0, -v0)
    } else {
        (r0, u0, v0)
    }
}
