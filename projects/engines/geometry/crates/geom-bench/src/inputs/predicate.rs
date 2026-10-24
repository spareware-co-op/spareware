//! Inputs for the exact predicates: [`orient2d`], [`orient3d`], [`incircle`] and [`insphere`].
//!
//! Classes:
//!
//! - `typical`: random points within ±2^30 steps, never degenerate.
//! - `degenerate`: exactly collinear, coplanar, cocircular or cospherical points.
//! - `nearly_degenerate`: the smallest nonzero answer the grid allows, next to terms of 2^70 and
//!   more. For orientation, a triangle or tetrahedron whose edges form a unimodular lattice basis
//!   (`unimodular2`, `unimodular3`), so its doubled area or sextupled volume is exactly ±1
//!   with edges of up to 2^38 steps. For the circle and sphere tests, a point whose squared distance
//!   from the centre of an integer circle or sphere of radius R is R² + 1 (the point (R, 1) from the
//!   centre), the closest any grid point off the circle can be. No floating-point filter can certify
//!   these signs (the tests check it against Shewchuk's error bounds); the exact path decides them.
//! - `extreme`: coordinates within 2^10 steps of ±(2^40 − 1); the first input is the
//!   configuration that reaches the predicate's largest determinant.
//!
//! Method: Shewchuk's three input classes (random, nearly degenerate, exactly degenerate; 1997,
//! section 6, <https://doi.org/10.1007/PL00009321>) and the extremes of the frame; unimodular bases
//! from the extended Euclidean algorithm (Knuth, volume 2, section 4.5.2); Pythagorean triples and
//! quadruples for integer circles and spheres (<https://en.wikipedia.org/wiki/Pythagorean_quadruple>).
//!
//! Guarantees: every batch is full, the same on every target, and its classes are what they
//! claim (`tests/inputs.rs`, against exact determinants and Shewchuk's filter bounds); every
//! sign is checked against an exact determinant by `tests/attack_g06.rs`.
//!
//! Cost: a few draws of [`crate::rng::SplitMix64`] per input, once per batch, outside every
//! measured run.

use std::collections::BTreeSet;

use geom_num::grid::{P2, P3};
use geom_num::{NumError, Sign, predicate};

use super::{
    Class, M, TYPICAL_REACH, batch_of, extended_gcd, extreme2, extreme3, p2, p3, pick, rng, take,
    typical2, typical3, within,
};
use crate::rng::SplitMix64;

/// Half the typical reach: where degenerate configurations are centred, so their points stay
/// within the typical box.
const HALF: i64 = TYPICAL_REACH / 2;

/// Largest component of a direction or spanning vector in degenerate inputs.
const STEP: i64 = 1 << 12;

/// Largest multiple of a direction or spanning vector in degenerate inputs.
const MULTIPLE: i64 = 1 << 16;

/// Largest scale applied to a Pythagorean triple or quadruple in degenerate inputs.
const SCALE: i64 = 1 << 20;

/// The range of scales for nearly degenerate circles and spheres, [2^24, 2^26]: radii of at least
/// 3 · 2^24 steps, large enough that one unit of squared distance is far below every floating-point
/// filter's error bound (relative to the determinant's terms the answer shrinks as one over the
/// radius squared; the tests check the bound for every input), and small enough that every point
/// stays within 2^32 steps of the origin.
const NEAR_SCALE: (i64, i64) = (1 << 24, 1 << 26);

/// Whether a predicate certainly answered with a nonzero sign: `Positive` or `Negative`. A zero, or
/// an error (`BudgetExceeded`, which no grid input can cause), is not general position, so a
/// builder that needs general position rejects the candidate rather than guess.
fn nonzero(answer: Result<Sign, NumError>) -> bool {
    matches!(answer, Ok(Sign::Positive | Sign::Negative))
}

/// The scale of a degenerate (up to 2^20) or nearly degenerate ([`NEAR_SCALE`]) configuration.
fn scale(g: &mut SplitMix64, class: Class) -> i64 {
    if class == Class::NearlyDegenerate {
        g.range(NEAR_SCALE.0, NEAR_SCALE.1)
    } else {
        g.range(1, SCALE)
    }
}

/// Where nearly degenerate configurations are anchored: within ±2^20 steps of the origin, so their
/// edges of up to 2^38 + 1 steps stay inside the frame.
const ANCHOR: i64 = 1 << 20;

/// Pythagorean triples (a, b, c) with a² + b² = c², a ≠ b: eight integer points (±a, ±b), (±b, ±a)
/// on the circle of radius c.
const TRIPLES: [(i64, i64, i64); 8] = [
    (3, 4, 5),
    (5, 12, 13),
    (8, 15, 17),
    (7, 24, 25),
    (20, 21, 29),
    (12, 35, 37),
    (9, 40, 41),
    (28, 45, 53),
];

/// Pythagorean quadruples (a, b, c, d) with a² + b² + c² = d²: up to 48 integer points (signed
/// permutations) on the sphere of radius d.
const QUADRUPLES: [(i64, i64, i64, i64); 10] = [
    (1, 2, 2, 3),
    (2, 3, 6, 7),
    (1, 4, 8, 9),
    (4, 4, 7, 9),
    (2, 6, 9, 11),
    (6, 6, 7, 11),
    (3, 4, 12, 13),
    (2, 10, 11, 15),
    (1, 12, 12, 17),
    (8, 9, 12, 17),
];

/// Batches of three 2D points for [`predicate::orient2d`].
#[must_use]
pub fn orient2d(class: Class) -> Vec<[P2; 3]> {
    orient2d_from("predicate::orient2d", class)
}

/// [`orient2d`]'s classes drawn from the seed of `op`: the batch another benchmark of the same
/// predicate measures (the generated filter and exact value, [`super::generated`]), on inputs of its own.
pub(crate) fn orient2d_from(op: &str, class: Class) -> Vec<[P2; 3]> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let t @ [a, b, c] = [(); 3].map(|()| typical2(&mut g));
            nonzero(predicate::orient2d(a, b, c)).then_some(t)
        }),
        Class::Degenerate => batch_of(|_| {
            // a, a + s·d and a + t·d lie on one line.
            let [ax, ay] = [(); 2].map(|()| within(&mut g, HALF));
            let [dx, dy] = [(); 2].map(|()| within(&mut g, STEP));
            let [s, t] = [(); 2].map(|()| within(&mut g, MULTIPLE));
            let along = |k: i64| p2(ax + k * dx, ay + k * dy);
            Some([along(0), along(s), along(t)])
        }),
        Class::NearlyDegenerate => batch_of(|n| {
            // a, a + u, a + v with det[u; v] = 1: twice the area is exactly 1.
            let [u, v] = unimodular2(&mut g)?;
            let [ax, ay] = [(); 2].map(|()| within(&mut g, ANCHOR));
            let at = |[x, y]: [i64; 2]| p2(ax + x, ay + y);
            // Every other triple is reversed, so both signs occur.
            Some(if n % 2 == 0 {
                [at([0, 0]), at(u), at(v)]
            } else {
                [at([0, 0]), at(v), at(u)]
            })
        }),
        Class::Extreme => batch_of(|n| {
            Some(if n == 0 {
                // Half the bounding square: the largest orient2d determinant, 4M².
                [p2(-M, -M), p2(M, -M), p2(M, M)]
            } else {
                [(); 3].map(|()| extreme2(&mut g))
            })
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Batches of four 3D points for [`predicate::orient3d`].
#[must_use]
pub fn orient3d(class: Class) -> Vec<[P3; 4]> {
    orient3d_from("predicate::orient3d", class)
}

/// [`orient3d`]'s classes drawn from the seed of `op`: the batch another benchmark of the same
/// predicate measures (the generated filter and exact value, [`super::generated`]), on inputs of its own.
pub(crate) fn orient3d_from(op: &str, class: Class) -> Vec<[P3; 4]> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let t @ [a, b, c, d] = [(); 4].map(|()| typical3(&mut g));
            nonzero(predicate::orient3d(a, b, c, d)).then_some(t)
        }),
        Class::Degenerate => batch_of(|_| {
            // Every point is a + s·u + t·v: all in the plane through a spanned by u and v.
            let [ax, ay, az] = [(); 3].map(|()| within(&mut g, HALF));
            let [ux, uy, uz] = [(); 3].map(|()| within(&mut g, STEP));
            let [vx, vy, vz] = [(); 3].map(|()| within(&mut g, STEP));
            let mut point = || {
                let [s, t] = [(); 2].map(|()| within(&mut g, MULTIPLE));
                p3(
                    ax + s * ux + t * vx,
                    ay + s * uy + t * vy,
                    az + s * uz + t * vz,
                )
            };
            Some([p3(ax, ay, az), point(), point(), point()])
        }),
        Class::NearlyDegenerate => batch_of(|n| {
            // a, a + u, a + v, a + w with det[u; v; w] = 1: six times the volume is exactly 1.
            let [u, v, w] = unimodular3(&mut g)?;
            let [ax, ay, az] = [(); 3].map(|()| within(&mut g, ANCHOR));
            let at = |[x, y, z]: [i64; 3]| p3(ax + x, ay + y, az + z);
            Some(if n % 2 == 0 {
                [at([0; 3]), at(u), at(v), at(w)]
            } else {
                [at([0; 3]), at(v), at(u), at(w)]
            })
        }),
        Class::Extreme => batch_of(|n| {
            Some(if n == 0 {
                // The regular tetrahedron on alternate corners of the bounding cube: the largest
                // orient3d determinant, 16M³.
                [p3(-M, -M, -M), p3(M, M, -M), p3(M, -M, M), p3(-M, M, M)]
            } else {
                [(); 4].map(|()| extreme3(&mut g))
            })
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Batches of four 2D points for [`predicate::incircle`].
#[must_use]
pub fn incircle(class: Class) -> Vec<[P2; 4]> {
    incircle_from("predicate::incircle", class)
}

/// [`incircle`]'s classes drawn from the seed of `op`: the batch another benchmark of the same
/// predicate measures (the generated filter and exact value, [`super::generated`]), on inputs of its own.
pub(crate) fn incircle_from(op: &str, class: Class) -> Vec<[P2; 4]> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let t @ [a, b, c, d] = [(); 4].map(|()| typical2(&mut g));
            nonzero(predicate::incircle(a, b, c, d)).then_some(t)
        }),
        Class::Degenerate | Class::NearlyDegenerate => batch_of(|n| {
            // Three (degenerate: four) distinct points of the eight (±a, ±b), (±b, ±a), scaled by
            // k about a centre: all at distance k·c from it. A nearly degenerate fourth point is
            // the centre plus (k·c, ±1), at squared distance (k·c)² + 1.
            let (a, b, c) = pick(&mut g, &TRIPLES)?;
            let k = scale(&mut g, class);
            let [cx, cy] = [(); 2].map(|()| within(&mut g, HALF));
            let mut ring = ring(a, b);
            let mut point = || {
                let (x, y) = take(&mut g, &mut ring)?;
                Some(p2(cx + k * x, cy + k * y))
            };
            let [p, q, r] = [point()?, point()?, point()?];
            let s = if class == Class::Degenerate {
                point()?
            } else {
                let side = if n % 2 == 0 { 1 } else { -1 };
                p2(cx + k * c, cy + side)
            };
            Some([p, q, r, s])
        }),
        Class::Extreme => batch_of(|n| {
            Some(if n == 0 {
                // Three corners of the bounding square and its centre: the largest value, 8M⁴.
                [p2(-M, -M), p2(M, -M), p2(M, M), p2(0, 0)]
            } else {
                [(); 4].map(|()| extreme2(&mut g))
            })
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Batches of five 3D points for [`predicate::insphere`].
#[must_use]
pub fn insphere(class: Class) -> Vec<[P3; 5]> {
    insphere_from("predicate::insphere", class)
}

/// [`insphere`]'s classes drawn from the seed of `op`: the batch another benchmark of the same
/// predicate measures (the generated filter and exact value, [`super::generated`]), on inputs of its own.
pub(crate) fn insphere_from(op: &str, class: Class) -> Vec<[P3; 5]> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let t @ [a, b, c, d, e] = [(); 5].map(|()| typical3(&mut g));
            nonzero(predicate::insphere(a, b, c, d, e)).then_some(t)
        }),
        Class::Degenerate | Class::NearlyDegenerate => batch_of(|n| {
            // Four distinct signed permutations of a quadruple, scaled by k about a centre: all at
            // distance k·d from it. They are not coplanar, so the sphere through them is unique.
            // The fifth point is another of them (degenerate), or the centre plus (k·d, ±1, 0), at
            // squared distance (k·d)² + 1 (nearly degenerate).
            let (a, b, c, d) = pick(&mut g, &QUADRUPLES)?;
            let k = scale(&mut g, class);
            let [cx, cy, cz] = [(); 3].map(|()| within(&mut g, HALF));
            let mut sphere = signed_permutations(a, b, c);
            let mut point = || {
                let [x, y, z] = take(&mut g, &mut sphere)?;
                Some(p3(cx + k * x, cy + k * y, cz + k * z))
            };
            let four @ [p, q, r, s] = [point()?, point()?, point()?, point()?];
            // The sphere through the four is unique only if they are not coplanar.
            if !nonzero(predicate::orient3d(p, q, r, s)) {
                return None;
            }
            let fifth = if class == Class::Degenerate {
                point()?
            } else {
                let side = if n % 2 == 0 { 1 } else { -1 };
                p3(cx + k * d, cy + side, cz)
            };
            let [p, q, r, s] = four;
            Some([p, q, r, s, fifth])
        }),
        Class::Extreme => batch_of(|n| {
            Some(if n == 0 {
                // The regular tetrahedron on alternate corners and the cube's centre: the largest
                // value, 48M⁵.
                [
                    p3(-M, -M, -M),
                    p3(M, M, -M),
                    p3(M, -M, M),
                    p3(-M, M, M),
                    p3(0, 0, 0),
                ]
            } else {
                [(); 5].map(|()| extreme3(&mut g))
            })
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Two lattice vectors u, v with det[u; v] = u_x v_y − u_y v_x = 1 and components up to 2^37 in
/// magnitude: a unimodular basis of the 2D grid, or `None` (about two times in five) when the
/// random first vector is not primitive, so the caller draws again.
///
/// u = (p, q) with gcd(p, q) = 1 and |p|, |q| in [2^35, 2^36]; the extended Euclidean algorithm
/// gives p·s + q·t = 1 with |s| ≤ |q| and |t| ≤ |p|, so v = (−t, s) has det[u; v] = p·s + q·t = 1.
/// Adding −u, 0 or u to v keeps the determinant, spreads the batch, and keeps |v_i| ≤ 2^37.
pub(crate) fn unimodular2(g: &mut SplitMix64) -> Option<[[i64; 2]; 2]> {
    let [p, q] = [(); 2].map(|()| g.sign() * g.range(1 << 35, 1 << 36));
    let (gcd, s, t) = extended_gcd(p, q);
    if gcd != 1 {
        return None;
    }
    let k = g.range(-1, 1);
    Some([[p, q], [k * p - t, k * q + s]])
}

/// Three lattice vectors u, v, w with det[u; v; w] = 1 and components up to 2^38 + 1 in magnitude:
/// a unimodular basis of the 3D grid, or `None` when [`unimodular2`] draws again.
///
/// From a 2D basis (p, q), (r, s) with det 1: u = (p, q, a) and v = (r, s, b) for any a, b, and
/// e_z = (0, 0, 1), have det[u; v; e_z] = p·s − q·r = 1. Adding ±u ± v to e_z keeps the determinant
/// and makes every component of w large too. With |a|, |b| ≤ 2^37 every component of u and v is
/// at most 2^37 and every component of w at most 2^38 + 1.
pub(crate) fn unimodular3(g: &mut SplitMix64) -> Option<[[i64; 3]; 3]> {
    let [[p, q], [r, s]] = unimodular2(g)?;
    let [a, b] = [(); 2].map(|()| within(g, 1 << 37));
    let u = [p, q, a];
    let v = [r, s, b];
    let (i, j) = (g.sign(), g.sign());
    let w = [i * p + j * r, i * q + j * s, 1 + i * a + j * b];
    Some([u, v, w])
}

/// The eight points (±a, ±b) and (±b, ±a), all at distance √(a² + b²) from the origin.
fn ring(a: i64, b: i64) -> Vec<(i64, i64)> {
    let mut points = Vec::with_capacity(8);
    for (x, y) in [(a, b), (b, a)] {
        for (sx, sy) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
            points.push((sx * x, sy * y));
        }
    }
    points
}

/// Every point (±x, ±y, ±z) over the permutations (x, y, z) of (a, b, c), without repeats, in a
/// fixed order: all at distance √(a² + b² + c²) from the origin.
fn signed_permutations(a: i64, b: i64, c: i64) -> Vec<[i64; 3]> {
    let orders = [
        [a, b, c],
        [a, c, b],
        [b, a, c],
        [b, c, a],
        [c, a, b],
        [c, b, a],
    ];
    let mut points = BTreeSet::new();
    for [x, y, z] in orders {
        for signs in 0..8u8 {
            let s = |bit: u8| if (signs >> bit) & 1 == 1 { -1 } else { 1 };
            points.insert([s(0) * x, s(1) * y, s(2) * z]);
        }
    }
    points.into_iter().collect()
}
