//! Exact geometric predicates on grid points: orientation in 2D and 3D, in-circle and in-sphere.
//!
//! Every topological decision in the kernel is one of these questions (chapter 6.2): which way do
//! three points turn, which side of a plane is a point on, is a point inside a circle or a sphere.
//! Each answer is a [`Sign`], never a number.
//!
//! Guarantees:
//!
//! - **Exact for every input.** No rounding ever decides a sign, and `Zero` means exactly
//!   degenerate. Each predicate documents its degenerate cases, including which sign a degenerate
//!   but non-zero answer has.
//! - **The same answer in every build profile; no overflow, no panic.** Outside its tests this
//!   module uses no native `+`, `-` or `*` on a fixed-width integer, and no cast: every operation
//!   is a checked method (`checked_sub`, `checked_mul`, `checked_add`, `checked_neg`) that returns
//!   `None` instead of wrapping. So nothing here depends on overflow checks being on, which they
//!   are not in a default release build. tests/attack_predicate.rs scans this file's code to keep
//!   it that way.
//! - **Deterministic.** Integer arithmetic only: the same answer on every target.
//!
//! Two evaluation paths, both exact:
//!
//! 1. **The fast path**, taken for every grid input. Every grid coordinate has magnitude at most
//!    [`COORD_MAX`](crate::grid::COORD_MAX) = 2^40 − 1, which [`Coord`](crate::grid::Coord)
//!    enforces. Orientation runs in `i128`; in-circle and in-sphere compute their factors in
//!    `i128` and sum the products in [`I256`](crate::wide::I256). Each predicate's doc comment
//!    carries the bit-budget derivation proving that none of its checked operations returns `None`
//!    for any grid input.
//! 2. **The total path**, reached only if a fast-path operation returns `None`, which the budgets
//!    prove never happens. It evaluates the same determinant from the raw `i64` coordinates in
//!    [`I384`](crate::wide::I384), which holds every intermediate for any `i64` coordinates
//!    whatsoever, not only grid ones (the derivation is written in the source where the total
//!    path begins). So a mistake in a fast-path budget, or a coordinate that somehow broke the grid
//!    invariant, would cost time, never a wrong sign. The total path's own operations are checked
//!    too, and none of them can return `None` for any value its argument types can hold. If one
//!    somehow did, the predicate would return `Zero`, and that answer would be wrong, since it
//!    claims exact degeneracy: with no exact value left and no panic allowed, there is no right
//!    answer to give.
//!
//! The fast-path budgets are derived for [`COORD_BITS`](crate::grid::COORD_BITS) = 40, and a
//! compile-time assertion below stops the build if it is ever raised: the written derivations
//! would then be false (at 41 bits, orient3d's bound 6δ³ already exceeds 2^127).
//!
//! Method: the determinant forms of Shewchuk, *Adaptive Precision Floating-Point Arithmetic and
//! Fast Robust Geometric Predicates*, Discrete & Computational Geometry 18 (1997)
//! (<https://doi.org/10.1007/PL00009321>; reference code
//! <https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c>): orientation is the
//! determinant of the edge vectors, and in-circle and in-sphere are the orientation of the points
//! lifted onto the paraboloid z = |p|², written relative to the query point so every entry is a
//! small difference. Shewchuk evaluates these in floating point with adaptive expansions; on the
//! grid the inputs are integers below 2^40, so the same determinants evaluate exactly in
//! fixed-width integers. There is no floating-point filter yet: a generated filter (chapter 6.2) is
//! a later piece, and it may only ever return early with the sign this exact path gives.
//!
//! Conventions: [`orient2d`] and [`incircle`] have the signs of `predicates.c`. [`orient3d`] and
//! [`insphere`] have the **opposite** signs to `predicates.c`, whose `orient3d` is positive when
//! `d` lies *below* the plane of `a`, `b`, `c`; here it is positive when `d` lies on the side that
//! the right-hand normal (b − a) × (c − a) points to. Code ported from Shewchuk-based libraries
//! must account for this.
//!
//! Notation in the bit budgets: M = `COORD_MAX` = 2^40 − 1 and δ = 2M = 2^41 − 2 < 2^41, the
//! largest magnitude of a difference of two coordinates. Every bound is on absolute values, and a
//! sum's bound also bounds each of its partial sums, so the order of evaluation never matters.

use crate::grid::{COORD_BITS, P2, P3};
use crate::sign::Sign;
use crate::wide::{I256, I384};

// Every fast-path bit budget in this module assumes coordinates of at most 40 bits.
const _: () = assert!(COORD_BITS <= 40, "the budgets below assume 40 bits");

// ---------------------------------------------------------------------------------------------
// The fast path: `i128`, then `I256` for the in-circle and in-sphere sums.
// ---------------------------------------------------------------------------------------------

/// p − q for two coordinates, as an `i128`.
///
/// Budget: on the grid |p|, |q| ≤ M, so |p − q| ≤ δ < 2^41. For any `i64` values at all
/// |p − q| ≤ 2^64 − 1, so `None` cannot occur even off the grid.
fn diff(p: i64, q: i64) -> Option<i128> {
    i128::from(p).checked_sub(i128::from(q))
}

/// a·d − b·c, the 2 × 2 determinant with rows (a, b) and (c, d), or `None` if it does not fit
/// `i128`. Each caller bounds it.
fn det2(a: i128, b: i128, c: i128, d: i128) -> Option<i128> {
    a.checked_mul(d)?.checked_sub(b.checked_mul(c)?)
}

/// The difference p − q of two 2D points, components in `i128`.
#[derive(Clone, Copy)]
struct D2 {
    x: i128,
    y: i128,
}

impl D2 {
    /// p − q. Budget: each component at most δ < 2^41 ([`diff`]).
    fn sub(p: [i64; 2], q: [i64; 2]) -> Option<D2> {
        let ([px, py], [qx, qy]) = (p, q);
        Some(D2 {
            x: diff(px, qx)?,
            y: diff(py, qy)?,
        })
    }

    /// The 2 × 2 determinant with rows `self` and `other`: self.x · other.y − self.y · other.x.
    ///
    /// Budget: each product at most δ² < 2^82, the difference at most 2δ² < 2^83.
    fn cross(self, other: D2) -> Option<i128> {
        det2(self.x, self.y, other.x, other.y)
    }

    /// The squared length x² + y². Budget: at most 2δ² < 2^83.
    fn norm2(self) -> Option<i128> {
        let xx = self.x.checked_mul(self.x)?;
        xx.checked_add(self.y.checked_mul(self.y)?)
    }
}

/// The difference p − q of two 3D points, components in `i128`.
#[derive(Clone, Copy)]
struct D3 {
    x: i128,
    y: i128,
    z: i128,
}

impl D3 {
    /// p − q. Budget: each component at most δ < 2^41 ([`diff`]).
    fn sub(p: [i64; 3], q: [i64; 3]) -> Option<D3> {
        let ([px, py, pz], [qx, qy, qz]) = (p, q);
        Some(D3 {
            x: diff(px, qx)?,
            y: diff(py, qy)?,
            z: diff(pz, qz)?,
        })
    }

    /// The squared length x² + y² + z². Budget: at most 3δ² < 2^84 (3 < 2^2, δ² < 2^82).
    fn norm2(self) -> Option<i128> {
        let xx = self.x.checked_mul(self.x)?;
        let yy = self.y.checked_mul(self.y)?;
        let zz = self.z.checked_mul(self.z)?;
        xx.checked_add(yy)?.checked_add(zz)
    }
}

/// The 3 × 3 determinant with rows p, q, r: the scalar triple product p · (q × r).
///
/// Budget: each component of q × r is a difference of two products of differences, at most
/// 2δ² < 2^83; each of the three terms of the dot product is at most δ · 2δ² = 2δ³ < 2^124; the sum
/// is at most 6δ³ < 2^126 (6 < 2^3, δ³ < 2^123). Every intermediate fits `i128` (|v| < 2^127), and
/// so does the negation of the result.
fn det3(p: D3, q: D3, r: D3) -> Option<i128> {
    let x = p.x.checked_mul(det2(q.y, q.z, r.y, r.z)?)?;
    let y = p.y.checked_mul(det2(q.z, q.x, r.z, r.x)?)?;
    let z = p.z.checked_mul(det2(q.x, q.y, r.x, r.y)?)?;
    x.checked_add(y)?.checked_add(z)
}

/// x₀·y₀ + x₁·y₁ + … for `i128` factors, evaluated in [`I256`] (range ±2^255), or `None` if a
/// product or a partial sum does not fit: the wide step of [`incircle`] and [`insphere`].
///
/// Every product of two `i128` values fits (|x · y| ≤ 2^127 · 2^127 = 2^254), and the callers' bit
/// budgets keep every partial sum below 2^212, so for every grid input the result is `Some`.
/// Because the exact total path stands behind it, a failure here could never show up as a wrong
/// sign, so this module's unit tests check the budget itself. They cover every choice of points
/// with coordinates in {−M, 1 − M, 0, M − 1, M} for in-circle and in {−M, 0, M} for in-sphere
/// (each set of points once, with every choice of query point: the order of the others only
/// permutes the terms), and random points; every factor, product and sum stays within its bound.
fn sum_of_products(terms: &[(i128, i128)]) -> Option<I256> {
    terms.iter().try_fold(I256::ZERO, |sum, &(x, y)| {
        let product = I256::from_i128(x).checked_mul(&I256::from_i128(y))?;
        sum.checked_add(&product)
    })
}

/// orient2d's determinant on the fast path. Budget on [`orient2d`].
fn orient2d_fast(a: [i64; 2], b: [i64; 2], c: [i64; 2]) -> Option<i128> {
    D2::sub(b, a)?.cross(D2::sub(c, a)?)
}

/// orient3d's determinant on the fast path. Budget on [`orient3d`].
fn orient3d_fast(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3]) -> Option<i128> {
    det3(D3::sub(b, a)?, D3::sub(c, a)?, D3::sub(d, a)?)
}

/// The (lift, 2 × 2 determinant) pairs whose products sum to [`incircle`]'s determinant. Budgets
/// as derived there: every factor below 2^83, every product below 2^166, the sum below 2^168.
fn incircle_terms(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) -> Option<[(i128, i128); 3]> {
    let a = D2::sub(a, d)?;
    let b = D2::sub(b, d)?;
    let c = D2::sub(c, d)?;
    Some([
        (a.norm2()?, b.cross(c)?),
        (b.norm2()?, c.cross(a)?),
        (c.norm2()?, a.cross(b)?),
    ])
}

/// incircle's determinant on the fast path. Budget on [`incircle`].
fn incircle_fast(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) -> Option<I256> {
    sum_of_products(&incircle_terms(a, b, c, d)?)
}

/// The (lift, signed 3 × 3 cofactor) pairs whose products sum to [`insphere`]'s determinant.
/// Budgets as derived there: lifts below 2^84, cofactors below 2^126, every product below 2^210,
/// the sum below 2^212.
fn insphere_terms(
    a: [i64; 3],
    b: [i64; 3],
    c: [i64; 3],
    d: [i64; 3],
    e: [i64; 3],
) -> Option<[(i128, i128); 4]> {
    let a = D3::sub(a, e)?;
    let b = D3::sub(b, e)?;
    let c = D3::sub(c, e)?;
    let d = D3::sub(d, e)?;
    Some([
        (a.norm2()?, det3(b, c, d)?),
        (b.norm2()?, det3(a, c, d)?.checked_neg()?),
        (c.norm2()?, det3(a, b, d)?),
        (d.norm2()?, det3(a, b, c)?.checked_neg()?),
    ])
}

/// insphere's determinant on the fast path. Budget on [`insphere`].
fn insphere_fast(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3], e: [i64; 3]) -> Option<I256> {
    sum_of_products(&insphere_terms(a, b, c, d, e)?)
}

// ---------------------------------------------------------------------------------------------
// The total path: the same determinants in `I384`, for any `i64` coordinates.
//
// Budget for any `i64` coordinates: each coordinate is in [−2^63, 2^63 − 1], so each difference
// has magnitude at most 2^64 − 1 < 2^64. Then each 2 × 2 determinant is below 2 · 2^128 = 2^129;
// each squared length below 2^129 in 2D and 3 · 2^128 < 2^130 in 3D; each 3 × 3 determinant below
// 3 · 2^64 · 2^129 < 2^195; each in-circle product below 2^129 · 2^129 = 2^258 and their sum below
// 3 · 2^258 < 2^260; each in-sphere product below 2^130 · 2^195 = 2^325 and their sum below
// 4 · 2^325 = 2^327. Everything stays 56 bits or more below the I384 limit of 2^383, so for every
// value of the argument types no checked operation here returns `None`.
// ---------------------------------------------------------------------------------------------

/// The total path's integer type: holds every intermediate of every predicate for any `i64`
/// coordinates (budget above).
type Total = I384;

/// p − q for two coordinates in [`Total`]: below 2^64 in magnitude.
fn total_diff(p: i64, q: i64) -> Option<Total> {
    Total::from_i64(p).checked_sub(&Total::from_i64(q))
}

/// p − q for two 2D points in [`Total`].
fn total_sub2(p: [i64; 2], q: [i64; 2]) -> Option<[Total; 2]> {
    let ([px, py], [qx, qy]) = (p, q);
    Some([total_diff(px, qx)?, total_diff(py, qy)?])
}

/// p − q for two 3D points in [`Total`].
fn total_sub3(p: [i64; 3], q: [i64; 3]) -> Option<[Total; 3]> {
    let ([px, py, pz], [qx, qy, qz]) = (p, q);
    Some([
        total_diff(px, qx)?,
        total_diff(py, qy)?,
        total_diff(pz, qz)?,
    ])
}

/// a·d − b·c in [`Total`]: below 2^129 for differences of `i64` values.
fn total_det2(a: &Total, b: &Total, c: &Total, d: &Total) -> Option<Total> {
    a.checked_mul(d)?.checked_sub(&b.checked_mul(c)?)
}

/// The squared length of a difference vector in [`Total`]: below 2^129 in 2D, 2^130 in 3D.
fn total_norm2(p: &[Total]) -> Option<Total> {
    let mut sum = Total::ZERO;
    for x in p {
        sum = sum.checked_add(&x.checked_mul(x)?)?;
    }
    Some(sum)
}

/// The 3 × 3 determinant p · (q × r) in [`Total`]: below 2^195 for differences of `i64` values.
fn total_det3(p: &[Total; 3], q: &[Total; 3], r: &[Total; 3]) -> Option<Total> {
    let ([px, py, pz], [qx, qy, qz], [rx, ry, rz]) = (p, q, r);
    let x = px.checked_mul(&total_det2(qy, qz, ry, rz)?)?;
    let y = py.checked_mul(&total_det2(qz, qx, rz, rx)?)?;
    let z = pz.checked_mul(&total_det2(qx, qy, rx, ry)?)?;
    x.checked_add(&y)?.checked_add(&z)
}

/// orient2d's determinant (b − a) × (c − a) for any `i64` coordinates: below 2^129.
fn orient2d_total(a: [i64; 2], b: [i64; 2], c: [i64; 2]) -> Option<Total> {
    let [ux, uy] = total_sub2(b, a)?;
    let [vx, vy] = total_sub2(c, a)?;
    total_det2(&ux, &uy, &vx, &vy)
}

/// orient3d's determinant ((b − a) × (c − a)) · (d − a) for any `i64` coordinates: below 2^195.
fn orient3d_total(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3]) -> Option<Total> {
    let u = total_sub3(b, a)?;
    let v = total_sub3(c, a)?;
    let w = total_sub3(d, a)?;
    total_det3(&u, &v, &w)
}

/// incircle's determinant, the same formula as on [`incircle`], for any `i64` coordinates: below
/// 2^260.
fn incircle_total(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) -> Option<Total> {
    let a = total_sub2(a, d)?;
    let b = total_sub2(b, d)?;
    let c = total_sub2(c, d)?;
    let ([ax, ay], [bx, by], [cx, cy]) = (&a, &b, &c);
    let ta = total_norm2(&a)?.checked_mul(&total_det2(bx, by, cx, cy)?)?;
    let tb = total_norm2(&b)?.checked_mul(&total_det2(cx, cy, ax, ay)?)?;
    let tc = total_norm2(&c)?.checked_mul(&total_det2(ax, ay, bx, by)?)?;
    ta.checked_add(&tb)?.checked_add(&tc)
}

/// insphere's determinant, the same formula as on [`insphere`], for any `i64` coordinates: below
/// 2^327.
fn insphere_total(
    a: [i64; 3],
    b: [i64; 3],
    c: [i64; 3],
    d: [i64; 3],
    e: [i64; 3],
) -> Option<Total> {
    let a = total_sub3(a, e)?;
    let b = total_sub3(b, e)?;
    let c = total_sub3(c, e)?;
    let d = total_sub3(d, e)?;
    let ta = total_norm2(&a)?.checked_mul(&total_det3(&b, &c, &d)?)?;
    let tb = total_norm2(&b)?.checked_mul(&total_det3(&a, &c, &d)?)?;
    let tc = total_norm2(&c)?.checked_mul(&total_det3(&a, &b, &d)?)?;
    let td = total_norm2(&d)?.checked_mul(&total_det3(&a, &b, &c)?)?;
    ta.checked_sub(&tb)?.checked_add(&tc)?.checked_sub(&td)
}

/// The fast path's sign when it succeeded, otherwise the sign of the total path's value.
///
/// The total path's `None` arm cannot be reached for any value of its argument types (budget
/// above). If it were, `Zero` would be a wrong answer (see the module documentation).
fn fast_else_total(fast: Option<Sign>, total: impl FnOnce() -> Option<Total>) -> Sign {
    match fast {
        Some(sign) => sign,
        None => total().map_or(Sign::Zero, |value| value.signum()),
    }
}

/// A 2D grid point's coordinates.
fn xy(p: P2) -> [i64; 2] {
    [p.x.get(), p.y.get()]
}

/// A 3D grid point's coordinates.
fn xyz(p: P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

// ---------------------------------------------------------------------------------------------
// The predicates.
// ---------------------------------------------------------------------------------------------

/// Which way the points a, b, c turn.
///
/// The sign of det[b − a; c − a] = (b.x − a.x)(c.y − a.y) − (b.y − a.y)(c.x − a.x), twice the
/// signed area of the triangle a, b, c. `Positive` when a, b, c turn counter-clockwise (c lies to
/// the left of the directed line from a to b), `Negative` when they turn clockwise, `Zero` exactly
/// when they are collinear, which includes any two coinciding. Swapping any two points flips the
/// sign. Same sign as `orient2d` in Shewchuk's `predicates.c`.
///
/// Bit budget, all in `i128`: each difference is at most δ < 2^41; each of the two products at most
/// δ² < 2^82; their difference at most 2δ² < 2^83, far below 2^127. The largest value over the
/// whole grid is δ² = 4M², reached by a triangle that is half the bounding square, such as
/// (−M, −M), (M, −M), (M, M): no triangle in a square has more than half its area.
#[must_use]
pub fn orient2d(a: P2, b: P2, c: P2) -> Sign {
    let [a, b, c] = [a, b, c].map(xy);
    let fast = orient2d_fast(a, b, c).map(Sign::of_i128);
    fast_else_total(fast, || orient2d_total(a, b, c))
}

/// Which side of the plane through a, b, c the point d lies on.
///
/// The sign of det[b − a; c − a; d − a] = ((b − a) × (c − a)) · (d − a), six times the signed
/// volume of the tetrahedron a, b, c, d. `Positive` when d lies on the side the right-hand normal
/// (b − a) × (c − a) points to, so that a, b, c turn counter-clockwise seen from d; `Negative` on
/// the other side; `Zero` exactly when the four points are coplanar, which includes a, b, c being
/// collinear. Swapping any two points flips the sign. This is the opposite sign to `orient3d` in
/// Shewchuk's `predicates.c` (see the module documentation).
///
/// Bit budget, all in `i128`: each difference is at most δ < 2^41; each component of
/// (c − a) × (d − a) at most 2δ² < 2^83; each of the three products in the dot product with b − a
/// at most 2δ³ < 2^124; the sum at most 6δ³ < 2^126 (6 < 2^3, δ³ < 2^123), below 2^127. The largest
/// value over the whole grid is 2δ³ = 16M³: the regular tetrahedron on alternate corners of the
/// bounding cube, the largest tetrahedron in a cube (a third of its volume).
#[must_use]
pub fn orient3d(a: P3, b: P3, c: P3, d: P3) -> Sign {
    let [a, b, c, d] = [a, b, c, d].map(xyz);
    let fast = orient3d_fast(a, b, c, d).map(Sign::of_i128);
    fast_else_total(fast, || orient3d_total(a, b, c, d))
}

/// Whether d lies inside the circle through a, b, c.
///
/// The sign of the lifted determinant, written relative to d with p′ = p − d:
///
/// ```text
/// | |a′|²  a′.x  a′.y |
/// | |b′|²  b′.x  b′.y |  =  |a′|²·(b′ × c′) + |b′|²·(c′ × a′) + |c′|²·(a′ × b′)
/// | |c′|²  c′.x  c′.y |
/// ```
///
/// where p′ × q′ = p′.x·q′.y − p′.y·q′.x. When a, b, c are not collinear this equals orient2d's
/// determinant of (a, b, c) times (r² − |d − o|²), o and r being the circle's centre and radius.
/// So: `Positive` when d lies strictly inside the circle and a, b, c turn counter-clockwise,
/// `Negative` when strictly outside; both flip if a, b, c turn clockwise; `Zero` exactly when d is
/// on the circle. Swapping any two of the four points flips the sign. Same sign as `incircle` in
/// Shewchuk's `predicates.c`.
///
/// Degenerate cases, signs included: when a, b, c are collinear the determinant equals orient2d's
/// determinant of (a, b, d) times (c − a) · (c − b). So if two of a, b, c coincide every d gives
/// `Zero`. If they are distinct, the circle degenerates to their line: `Zero` exactly when d is on
/// it, and otherwise the sign of `orient2d(a, b, d)` when c lies outside the segment from a to b,
/// the opposite sign when c lies strictly between a and b. (Derivation: swapping c and d flips the
/// sign; apply the identity above to the circle through a, b, d; and a point c on the line through
/// a and b has power (c − a) · (c − b) with respect to every circle through a and b.)
///
/// Bit budget: each difference is at most δ < 2^41. In `i128`: each lift |p′|² is at most
/// 2δ² < 2^83, each 2 × 2 determinant p′ × q′ at most 2δ² < 2^83. In [`I256`] (the wide step): each
/// of the three products at most 4δ⁴ < 2^166 (δ⁴ < 2^164); the sum at most 12δ⁴ < 2^168
/// (12 < 2^4), 87 bits below the I256 limit of 2^255. Over all points with coordinates in
/// {−M, 0, M} the largest value is 8M⁴ = δ⁴ / 2, reached by three corners of the bounding square
/// and its centre. That is not claimed to be the largest over the whole grid, which is not derived
/// here; nothing depends on it, since the proof uses only the bound 12δ⁴.
#[must_use]
pub fn incircle(a: P2, b: P2, c: P2, d: P2) -> Sign {
    let [a, b, c, d] = [a, b, c, d].map(xy);
    let fast = incircle_fast(a, b, c, d).map(|sum| sum.signum());
    fast_else_total(fast, || incircle_total(a, b, c, d))
}

/// Whether e lies inside the sphere through a, b, c, d.
///
/// The sign of the lifted determinant, written relative to e with p′ = p − e, lift column first:
///
/// ```text
/// | |a′|²  a′.x  a′.y  a′.z |
/// | |b′|²  b′.x  b′.y  b′.z |  =  |a′|²·[b′ c′ d′] − |b′|²·[a′ c′ d′]
/// | |c′|²  c′.x  c′.y  c′.z |     + |c′|²·[a′ b′ d′] − |d′|²·[a′ b′ c′]
/// | |d′|²  d′.x  d′.y  d′.z |
/// ```
///
/// where [p′ q′ r′] is the 3 × 3 determinant with those rows. When a, b, c, d are not coplanar this
/// equals orient3d's determinant of (a, b, c, d) times (r² − |e − o|²), o and r being the sphere's
/// centre and radius. So: `Positive` when e lies strictly inside the sphere and
/// `orient3d(a, b, c, d)` is `Positive`, `Negative` when strictly outside; both flip if
/// `orient3d(a, b, c, d)` is `Negative`; `Zero` exactly when e is on the sphere. Swapping any two
/// of the five points flips the sign. This is the opposite sign to `insphere` in Shewchuk's
/// `predicates.c`, matching this module's [`orient3d`] (see the module documentation).
///
/// Degenerate cases, signs included. If a, b, c, d are coplanar there is no single sphere through
/// them, and:
///
/// - If a, b, c are not collinear, the determinant equals orient3d's determinant of (a, b, c, e)
///   times (|d − o|² − r²), o and r now being the centre and radius of the circle through a, b, c.
///   So if d is on that circle every e gives `Zero`. Otherwise `Zero` means exactly that e is in
///   their plane, and off it the sign is that of `orient3d(a, b, c, e)` when d lies strictly
///   outside the circle, the opposite when strictly inside.
/// - If a, b, c are collinear (which makes a, b, c, d coplanar for every d), the determinant equals
///   minus orient3d's determinant of (a, b, d, e) times (c − a) · (c − b). So if two of a, b, c
///   coincide, or d is on their line, every e gives `Zero`. Otherwise `Zero` means exactly that e
///   is in the plane of a, b, d, and off it the sign is that of `orient3d(a, b, d, e)` when c lies
///   strictly between a and b, the opposite when c lies outside the segment.
///
/// (Derivation: swapping d and e flips the sign; apply the identity above to the sphere through
/// a, b, c, e; a point in the plane of a, b, c has the same power with respect to that sphere as
/// with respect to the circle where the sphere meets the plane. For collinear a, b, c swap c and d
/// first, and use the power (c − a) · (c − b) as for [`incircle`].)
///
/// Bit budget: each difference is at most δ < 2^41. In `i128`: each lift |p′|² is at most
/// 3δ² < 2^84; each 3 × 3 determinant at most 6δ³ < 2^126 (the same derivation as [`orient3d`]),
/// so negating one cannot overflow. In [`I256`] (the wide step): each of the four products at most
/// 18δ⁵ < 2^210 (18 < 2^5, δ⁵ < 2^205); the sum at most 72δ⁵ < 2^212 (72 < 2^7), 43 bits below the
/// I256 limit of 2^255. Over all points with coordinates in {−M, 0, M} the largest value is
/// 48M⁵ = 3δ⁵ / 2, reached by a regular tetrahedron on alternate corners of the bounding cube and
/// its centre. That is not claimed to be the largest over the whole grid, which is not derived
/// here; nothing depends on it, since the proof uses only the bound 72δ⁵.
#[must_use]
pub fn insphere(a: P3, b: P3, c: P3, d: P3, e: P3) -> Sign {
    let [a, b, c, d, e] = [a, b, c, d, e].map(xyz);
    let fast = insphere_fast(a, b, c, d, e).map(|sum| sum.signum());
    fast_else_total(fast, || insphere_total(a, b, c, d, e))
}

#[cfg(test)]
mod tests {
    //! The two paths on their own. On the grid the fast path always succeeds, so no public call
    //! reaches the total path, and since the total path is exact, taking it could never show up as
    //! a wrong sign. So these tests check the fast path's bit budgets directly (every factor,
    //! product and sum within its bound, on the point sets each test names and on random points),
    //! check that the total path gives the same value wherever the fast path runs, and drive the
    //! total path with coordinates far off the grid, up to the limits of `i64`, against an
    //! independent `num-bigint` determinant.

    use num_bigint::BigInt;
    use proptest::prelude::*;

    use super::{
        Total, fast_else_total, incircle_fast, incircle_terms, incircle_total, insphere_fast,
        insphere_terms, insphere_total, orient2d_fast, orient2d_total, orient3d_fast,
        orient3d_total, sum_of_products,
    };
    use crate::grid::COORD_MAX;
    use crate::sign::Sign;
    use crate::wide::{I256, Wide};

    const M: i64 = COORD_MAX;

    /// Coordinates at an edge of the grid, one step inside it, or at its centre.
    const LEVELS_5: [i64; 5] = [-M, 1 - M, 0, M - 1, M];
    /// Coordinates at an edge of the grid or at its centre.
    const LEVELS_3: [i64; 3] = [-M, 0, M];

    const INCIRCLE_BITS: [u32; 4] = [83, 83, 166, 168];
    const INSPHERE_BITS: [u32; 4] = [84, 126, 210, 212];

    /// 2^bits as an `I256`, for bits < 255.
    fn pow2(bits: u32) -> I256 {
        let mut limbs = [0u64; 4];
        limbs[(bits / 64) as usize] = 1 << (bits % 64);
        I256::from_le_limbs(limbs)
    }

    /// Whether |v| < 2^bits, for bits < 255.
    fn below(v: I256, bits: u32) -> bool {
        let bound = pow2(bits);
        bound.checked_neg().unwrap() < v && v < bound
    }

    /// |v|.
    fn magnitude(v: I256) -> I256 {
        if v.is_negative() {
            v.checked_neg().unwrap()
        } else {
            v
        }
    }

    /// k · M^n as an `I256`.
    fn times_m_to_the(k: i64, n: u32) -> I256 {
        let mut v = I256::from_i64(k);
        for _ in 0..n {
            v = v.checked_mul(&I256::from_i64(M)).unwrap();
        }
        v
    }

    /// The fast path's value widened to the total path's type.
    fn widen(v: I256) -> Total {
        v.resize::<6>().unwrap()
    }

    /// The fast path's (lift, determinant) pairs against their budget: for bits = [x, y, p, s], the
    /// factors below 2^x and 2^y, every product below 2^p, and the sum, which must fit `I256`,
    /// below 2^s. Returns the sum.
    fn assert_within(terms: &[(i128, i128)], bits: [u32; 4]) -> I256 {
        let [x_bits, y_bits, product_bits, sum_bits] = bits;
        for &(x, y) in terms {
            assert!(x.unsigned_abs() < 1 << x_bits, "{terms:?}");
            assert!(y.unsigned_abs() < 1 << y_bits, "{terms:?}");
            let product = sum_of_products(&[(x, y)]).unwrap();
            assert!(below(product, product_bits), "{terms:?}");
        }
        let sum = sum_of_products(terms).expect("I256 must hold the sum");
        assert!(below(sum, sum_bits), "{terms:?}");
        sum
    }

    /// Every 2D point with both coordinates in `levels`.
    fn square(levels: &[i64]) -> Vec<[i64; 2]> {
        let mut points = Vec::new();
        for &x in levels {
            for &y in levels {
                points.push([x, y]);
            }
        }
        points
    }

    /// Every 3D point with all three coordinates in `levels`.
    fn cube(levels: &[i64]) -> Vec<[i64; 3]> {
        let mut points = Vec::new();
        for &x in levels {
            for &y in levels {
                for &z in levels {
                    points.push([x, y, z]);
                }
            }
        }
        points
    }

    /// Whether every coordinate of every point is in {−M, 0, M}.
    fn on_levels_3(points: &[&[i64]]) -> bool {
        let on = |p: &&[i64]| p.iter().all(|v| LEVELS_3.contains(v));
        points.iter().all(on)
    }

    /// orient2d on every ordered triple of points with coordinates in {−M, 1 − M, 0, M − 1, M}, and
    /// incircle with every d and every unordered a, b, c from the same points (the order of a, b, c
    /// only permutes the three terms, up to sign). The fast path succeeds within its budget, the
    /// total path gives the same value, the largest orient2d value is the documented 4M², and the
    /// largest incircle value over the points in {−M, 0, M} is the documented 8M⁴.
    #[test]
    fn fast_path_budgets_hold_at_the_extremes_in_2d() {
        let points = square(&LEVELS_5);
        let mut largest = 0u128;
        for &a in &points {
            for &b in &points {
                for &c in &points {
                    let det = orient2d_fast(a, b, c).expect("i128 must hold orient2d");
                    assert!(det.unsigned_abs() < 1 << 83, "{a:?} {b:?} {c:?}");
                    let total = orient2d_total(a, b, c);
                    assert_eq!(total, Some(Total::from_i128(det)), "{a:?} {b:?} {c:?}");
                    largest = largest.max(det.unsigned_abs());
                }
            }
        }
        assert_eq!(largest, 4 * (M as u128).pow(2));

        let mut largest = I256::ZERO;
        for &d in &points {
            for (i, &a) in points.iter().enumerate() {
                for (j, &b) in points.iter().enumerate().skip(i) {
                    for &c in points.iter().skip(j) {
                        let terms = incircle_terms(a, b, c, d).expect("i128 holds the terms");
                        let sum = assert_within(&terms, INCIRCLE_BITS);
                        assert_eq!(incircle_fast(a, b, c, d), Some(sum));
                        let total = incircle_total(a, b, c, d);
                        assert_eq!(total, Some(widen(sum)), "{:?}", [a, b, c, d]);
                        if on_levels_3(&[&a, &b, &c, &d]) {
                            largest = largest.max(magnitude(sum));
                        }
                    }
                }
            }
        }
        assert_eq!(largest, times_m_to_the(8, 4));
    }

    /// orient3d with every base point a and every unordered b, c, d, and insphere with every e and
    /// every unordered a, b, c, d (the order of a, b, c, d only permutes the four terms, up to
    /// sign), all from the 27 points with coordinates in {−M, 0, M}: the bounding cube's corners,
    /// edge midpoints, face centres and centre. The fast path succeeds within its budget, the total
    /// path gives the same value, and the largest values are the documented 16M³ and 48M⁵.
    #[test]
    fn fast_path_budgets_hold_at_the_extremes_in_3d() {
        let points = cube(&LEVELS_3);
        let mut largest = 0u128;
        for &a in &points {
            for (i, &b) in points.iter().enumerate() {
                for (j, &c) in points.iter().enumerate().skip(i) {
                    for &d in points.iter().skip(j) {
                        let det = orient3d_fast(a, b, c, d).expect("i128 must hold orient3d");
                        assert!(det.unsigned_abs() < 1 << 126, "{:?}", [a, b, c, d]);
                        let total = orient3d_total(a, b, c, d);
                        assert_eq!(total, Some(Total::from_i128(det)), "{:?}", [a, b, c, d]);
                        largest = largest.max(det.unsigned_abs());
                    }
                }
            }
        }
        assert_eq!(largest, 16 * (M as u128).pow(3));

        let mut largest = I256::ZERO;
        for &e in &points {
            for (i, &a) in points.iter().enumerate() {
                for (j, &b) in points.iter().enumerate().skip(i) {
                    for (k, &c) in points.iter().enumerate().skip(j) {
                        for &d in points.iter().skip(k) {
                            let terms = insphere_terms(a, b, c, d, e).expect("i128 holds terms");
                            let sum = assert_within(&terms, INSPHERE_BITS);
                            assert_eq!(insphere_fast(a, b, c, d, e), Some(sum));
                            let total = insphere_total(a, b, c, d, e);
                            assert_eq!(total, Some(widen(sum)), "{:?}", [a, b, c, d, e]);
                            largest = largest.max(magnitude(sum));
                        }
                    }
                }
            }
        }
        assert_eq!(largest, times_m_to_the(48, 5));
    }

    /// A coordinate anywhere on the grid or at an edge.
    fn coord() -> impl Strategy<Value = i64> {
        prop_oneof![-M..=M, Just(M), Just(-M)]
    }

    proptest! {
        #[test]
        fn fast_path_budgets_hold_on_random_points(
            v in proptest::array::uniform::<_, 15>(coord()),
        ) {
            let p2 = |k: usize| [v[k], v[k + 1]];
            let (a, b, c, d) = (p2(0), p2(2), p2(4), p2(6));
            let det = orient2d_fast(a, b, c).unwrap();
            prop_assert_eq!(orient2d_total(a, b, c), Some(Total::from_i128(det)));
            let sum = assert_within(&incircle_terms(a, b, c, d).unwrap(), INCIRCLE_BITS);
            prop_assert_eq!(incircle_total(a, b, c, d), Some(widen(sum)));

            let p3 = |k: usize| [v[k], v[k + 1], v[k + 2]];
            let (a, b, c, d, e) = (p3(0), p3(3), p3(6), p3(9), p3(12));
            let det = orient3d_fast(a, b, c, d).unwrap();
            prop_assert_eq!(orient3d_total(a, b, c, d), Some(Total::from_i128(det)));
            let sum = assert_within(&insphere_terms(a, b, c, d, e).unwrap(), INSPHERE_BITS);
            prop_assert_eq!(insphere_total(a, b, c, d, e), Some(widen(sum)));
        }
    }

    // -----------------------------------------------------------------------------------------
    // The total path against an independent determinant, up to the limits of i64.
    // -----------------------------------------------------------------------------------------

    const LO: i64 = i64::MIN;
    const HI: i64 = i64::MAX;

    /// The value a `Wide` holds, read from its two's-complement limbs.
    fn big<const L: usize>(w: &Wide<L>) -> BigInt {
        let limbs = w.to_le_limbs();
        let bytes: Vec<u8> = limbs.iter().flat_map(|l| l.to_le_bytes()).collect();
        BigInt::from_signed_bytes_le(&bytes)
    }

    /// The determinant by cofactor expansion along the first row.
    fn laplace(m: &[Vec<BigInt>]) -> BigInt {
        let Some((first, rest)) = m.split_first() else {
            return BigInt::from(1);
        };
        let mut det = BigInt::from(0);
        for (j, x) in first.iter().enumerate() {
            let minor: Vec<Vec<BigInt>> = rest.iter().map(|row| without(row, j)).collect();
            let term = x * laplace(&minor);
            det += if j % 2 == 0 { term } else { -term };
        }
        det
    }

    /// `row` without its entry `j`.
    fn without(row: &[BigInt], j: usize) -> Vec<BigInt> {
        let mut out = row.to_vec();
        out.remove(j);
        out
    }

    /// The determinant of the homogeneous rows [p, 1], or [p, |p|², 1] with the lift, negated if
    /// asked: orient2d is det[a 1; b 1; c 1], orient3d −det[a 1; …; d 1], incircle
    /// det[a |a|² 1; …; d |d|² 1] and insphere −det[a |a|² 1; …; e |e|² 1].
    fn oracle(points: &[&[i64]], lift: bool, negate: bool) -> BigInt {
        let rows: Vec<Vec<BigInt>> = points
            .iter()
            .map(|p| {
                let mut row: Vec<BigInt> = p.iter().map(|&v| BigInt::from(v)).collect();
                if lift {
                    row.push(p.iter().map(|&v| BigInt::from(v) * v).sum::<BigInt>());
                }
                row.push(BigInt::from(1));
                row
            })
            .collect();
        let det = laplace(&rows);
        if negate { -det } else { det }
    }

    /// The total path's value equals the oracle's and stays below 2^bits in magnitude.
    fn assert_exact(value: Option<Total>, expected: BigInt, bits: u64, what: &str) {
        let value = big(&value.unwrap_or_else(|| panic!("{what}: I384 overflowed")));
        assert_eq!(value, expected, "{what}");
        assert!(value.bits() <= bits, "{what}: over 2^{bits}");
    }

    fn exact_orient2d(a: [i64; 2], b: [i64; 2], c: [i64; 2]) {
        let expected = oracle(&[&a, &b, &c], false, false);
        let what = format!("orient2d {:?}", [a, b, c]);
        assert_exact(orient2d_total(a, b, c), expected, 129, &what);
    }

    fn exact_orient3d(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3]) {
        let expected = oracle(&[&a, &b, &c, &d], false, true);
        let what = format!("orient3d {:?}", [a, b, c, d]);
        assert_exact(orient3d_total(a, b, c, d), expected, 195, &what);
    }

    fn exact_incircle(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) {
        let expected = oracle(&[&a, &b, &c, &d], true, false);
        let what = format!("incircle {:?}", [a, b, c, d]);
        assert_exact(incircle_total(a, b, c, d), expected, 260, &what);
    }

    fn exact_insphere(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3], e: [i64; 3]) {
        let expected = oracle(&[&a, &b, &c, &d, &e], true, true);
        let what = format!("insphere {:?}", [a, b, c, d, e]);
        assert_exact(insphere_total(a, b, c, d, e), expected, 327, &what);
    }

    /// Every ordering of `items`.
    fn orderings<T: Copy>(items: &[T]) -> Vec<Vec<T>> {
        if items.is_empty() {
            return vec![Vec::new()];
        }
        let mut out = Vec::new();
        for i in 0..items.len() {
            let mut rest = items.to_vec();
            let first = rest.remove(i);
            for mut tail in orderings(&rest) {
                tail.insert(0, first);
                out.push(tail);
            }
        }
        out
    }

    /// p with the axes whose bit is set in `mask` reflected about −1/2 (v ↦ −1 − v), which swaps
    /// i64::MIN and i64::MAX and keeps every coordinate an `i64`.
    fn reflect<const D: usize>(p: [i64; D], mask: usize) -> [i64; D] {
        std::array::from_fn(|k| if (mask >> k) & 1 == 1 { !p[k] } else { p[k] })
    }

    /// The configurations where the total path's intermediates peak, at the limits of `i64`: for
    /// orient2d half the bounding square, for orient3d the regular tetrahedron on alternate
    /// corners, for incircle and insphere a corner and the opposite corner (largest lift) with the
    /// corners adjacent to the opposite one (largest determinant). Every order and reflection, and
    /// orient2d and incircle on every choice of points with coordinates in {i64::MIN, 0, i64::MAX}.
    #[test]
    fn total_path_is_exact_where_its_intermediates_peak_at_the_limits_of_i64() {
        for mask in 0..4 {
            for p in orderings(&[[LO, LO], [HI, LO], [HI, HI]]) {
                let p: Vec<[i64; 2]> = p.into_iter().map(|q| reflect(q, mask)).collect();
                exact_orient2d(p[0], p[1], p[2]);
            }
            for p in orderings(&[[HI, HI], [HI, LO], [LO, HI], [LO, LO]]) {
                let p: Vec<[i64; 2]> = p.into_iter().map(|q| reflect(q, mask)).collect();
                exact_incircle(p[0], p[1], p[2], p[3]);
            }
        }
        for mask in 0..8 {
            for p in orderings(&[[LO, LO, LO], [HI, HI, LO], [HI, LO, HI], [LO, HI, HI]]) {
                let p: Vec<[i64; 3]> = p.into_iter().map(|q| reflect(q, mask)).collect();
                exact_orient3d(p[0], p[1], p[2], p[3]);
            }
            let base = [
                [HI, HI, HI],
                [HI, HI, LO],
                [HI, LO, HI],
                [LO, HI, HI],
                [LO, LO, LO],
            ];
            for p in orderings(&base) {
                let p: Vec<[i64; 3]> = p.into_iter().map(|q| reflect(q, mask)).collect();
                exact_insphere(p[0], p[1], p[2], p[3], p[4]);
            }
        }
        let points = square(&[LO, 0, HI]);
        for &a in &points {
            for &b in &points {
                for &c in &points {
                    exact_orient2d(a, b, c);
                    for &d in &points {
                        exact_incircle(a, b, c, d);
                    }
                }
            }
        }
    }

    /// Any `i64` coordinate, with the extremes and their neighbours often.
    fn any_coord() -> impl Strategy<Value = i64> {
        prop_oneof![
            Just(LO),
            Just(HI),
            Just(LO + 1),
            Just(HI - 1),
            Just(0i64),
            any::<i64>(),
        ]
    }

    proptest! {
        #[test]
        fn total_path_is_exact_for_any_i64_coordinates(
            v in proptest::array::uniform::<_, 15>(any_coord()),
        ) {
            let p2 = |k: usize| [v[k], v[k + 1]];
            exact_orient2d(p2(0), p2(2), p2(4));
            exact_incircle(p2(0), p2(2), p2(4), p2(6));
            let p3 = |k: usize| [v[k], v[k + 1], v[k + 2]];
            exact_orient3d(p3(0), p3(3), p3(6), p3(9));
            exact_insphere(p3(0), p3(3), p3(6), p3(9), p3(12));
        }
    }

    /// The fast path's answer is used whenever it has one, without evaluating the total path;
    /// otherwise the total path's sign. The last case cannot arise (the total path never fails),
    /// and pins the documented answer if it ever did: `Zero`.
    #[test]
    fn fast_else_total_routes_as_documented() {
        let never = || -> Option<Total> { panic!("the total path must not run") };
        for sign in [Sign::Negative, Sign::Zero, Sign::Positive] {
            assert_eq!(fast_else_total(Some(sign), never), sign);
        }
        let cases = [(-3, Sign::Negative), (0, Sign::Zero), (7, Sign::Positive)];
        for (v, sign) in cases {
            assert_eq!(fast_else_total(None, || Some(Total::from_i64(v))), sign);
        }
        assert_eq!(fast_else_total(None, || None), Sign::Zero);
    }

    /// Every product of two `i128` values fits `I256`, including the largest, 2^254.
    #[test]
    fn i256_holds_every_single_product() {
        let extremes = [i128::MIN, i128::MAX];
        for x in extremes {
            for y in extremes {
                let product = sum_of_products(&[(x, y)]).unwrap();
                assert_eq!(big(&product), BigInt::from(x) * BigInt::from(y));
            }
        }
        assert_eq!(sum_of_products(&[]), Some(I256::ZERO));
    }
}
