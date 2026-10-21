//! Exact geometric predicates on grid points: orientation in 2D and 3D, in-circle and in-sphere.
//!
//! Every topological decision in the kernel is one of these questions (chapter 6.2): which way do
//! three points turn, which side of a plane is a point on, is a point inside a circle or a sphere.
//! Each answer is a [`Sign`], never a number.
//!
//! Guarantees:
//!
//! - **Exact for every input.** Every grid coordinate has magnitude at most
//!   [`COORD_MAX`](crate::grid::COORD_MAX) = 2^40 − 1, which [`Coord`](crate::grid::Coord) enforces,
//!   so each determinant is evaluated in integers wide enough for its largest possible value. No
//!   rounding ever decides a sign, and `Zero` means exactly degenerate.
//! - **No overflow, no panic.** Each function's doc comment carries its bit-budget derivation:
//!   orientation fits `i128`, in-circle and in-sphere fit [`I256`](crate::wide::I256) with more than
//!   40 bits to spare. The wide step uses checked arithmetic, and its `None` arm is proven
//!   unreachable (see `sign_of_sum_of_products` in the source).
//! - **Deterministic.** Integer arithmetic only: the same answer on every target.
//!
//! Method: the determinant forms of Shewchuk, *Adaptive Precision Floating-Point Arithmetic and Fast
//! Robust Geometric Predicates*, Discrete & Computational Geometry 18 (1997)
//! (<https://doi.org/10.1007/PL00009321>; reference code
//! <https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c>): orientation is the
//! determinant of the edge vectors, and in-circle and in-sphere are the orientation of the points
//! lifted onto the paraboloid z = |p|², written relative to the query point so every entry is a
//! small difference. Shewchuk evaluates these in floating point with adaptive expansions; on the
//! grid the inputs are integers below 2^40, so the same determinants evaluate exactly in fixed-width
//! integers. There is no floating-point filter yet: a generated filter (chapter 6.2) is a later
//! piece, and it may only ever return early with the sign this exact path gives.
//!
//! Conventions: [`orient2d`] and [`incircle`] have the signs of `predicates.c`. [`orient3d`] and
//! [`insphere`] have the **opposite** signs to `predicates.c`, whose `orient3d` is positive when `d`
//! lies *below* the plane of `a`, `b`, `c`; here it is positive when `d` lies on the side that the
//! right-hand normal (b − a) × (c − a) points to. Code ported from Shewchuk-based libraries must
//! account for this.
//!
//! Notation in the bit budgets: M = `COORD_MAX` = 2^40 − 1 and δ = 2M = 2^41 − 2 < 2^41, the largest
//! magnitude of a difference of two coordinates. Every bound is on absolute values, and a sum's
//! bound also bounds each of its partial sums, so the order of evaluation never matters.

use crate::grid::{P2, P3};
use crate::sign::Sign;
use crate::wide::Wide;

/// The difference p − q of two 2D grid points, components in `i128`.
///
/// Budget: |p.x|, |q.x| ≤ M, so each component has magnitude at most δ < 2^41.
#[derive(Clone, Copy)]
struct D2 {
    x: i128,
    y: i128,
}

impl D2 {
    /// p − q. Each subtraction is of two values below 2^40 in magnitude: below 2^41, no overflow.
    fn sub(p: P2, q: P2) -> D2 {
        D2 {
            x: i128::from(p.x.get()) - i128::from(q.x.get()),
            y: i128::from(p.y.get()) - i128::from(q.y.get()),
        }
    }

    /// The 2 × 2 determinant with rows `self` and `other`: self.x · other.y − self.y · other.x.
    ///
    /// Budget: each product at most δ² < 2^82, the difference at most 2δ² < 2^83.
    fn cross(self, other: D2) -> i128 {
        self.x * other.y - self.y * other.x
    }

    /// The squared length x² + y². Budget: at most 2δ² < 2^83.
    fn norm2(self) -> i128 {
        self.x * self.x + self.y * self.y
    }
}

/// The difference p − q of two 3D grid points, components in `i128`.
///
/// Budget: each component has magnitude at most δ < 2^41.
#[derive(Clone, Copy)]
struct D3 {
    x: i128,
    y: i128,
    z: i128,
}

impl D3 {
    /// p − q. Each subtraction is of two values below 2^40 in magnitude: below 2^41, no overflow.
    fn sub(p: P3, q: P3) -> D3 {
        D3 {
            x: i128::from(p.x.get()) - i128::from(q.x.get()),
            y: i128::from(p.y.get()) - i128::from(q.y.get()),
            z: i128::from(p.z.get()) - i128::from(q.z.get()),
        }
    }

    /// The squared length x² + y² + z². Budget: at most 3δ² < 2^84 (3 < 2^2, δ² < 2^82).
    fn norm2(self) -> i128 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }
}

/// The 3 × 3 determinant with rows p, q, r: the scalar triple product p · (q × r).
///
/// Budget: each component of q × r is a difference of two products of differences, at most
/// 2δ² < 2^83; each of the three terms of the dot product is at most δ · 2δ² = 2δ³ < 2^124; the sum
/// is at most 6δ³ < 2^126 (6 < 2^3, δ³ < 2^123). Every intermediate fits `i128` (|v| < 2^127), and
/// so does the negation of the result.
fn det3(p: D3, q: D3, r: D3) -> i128 {
    let cx = q.y * r.z - q.z * r.y;
    let cy = q.z * r.x - q.x * r.z;
    let cz = q.x * r.y - q.y * r.x;
    p.x * cx + p.y * cy + p.z * cz
}

/// x₀·y₀ + x₁·y₁ + … evaluated in `Wide<L>`, or `None` if a product or a partial sum does not fit.
fn sum_of_products<const L: usize>(terms: &[(i128, i128)]) -> Option<Wide<L>> {
    terms.iter().try_fold(Wide::<L>::ZERO, |sum, &(x, y)| {
        let product = Wide::<L>::from_i128(x).checked_mul(&Wide::<L>::from_i128(y))?;
        sum.checked_add(&product)
    })
}

/// The exact sign of x₀·y₀ + x₁·y₁ + … for `i128` factors: the wide step of [`incircle`] and
/// [`insphere`].
///
/// Evaluated in [`I256`](crate::wide::I256) (range ±2^255). Every product of two `i128` values fits
/// (|x · y| ≤ 2^127 · 2^127 = 2^254), and the callers' bit budgets keep every partial sum below
/// 2^212, so for every grid input no checked operation returns `None`.
///
/// Fallback, unreachable: should an `I256` operation return `None` anyway, the same sum is evaluated
/// again in [`I512`](crate::wide::I512) (range ±2^511). There nothing can overflow for any input
/// whatsoever: each product is at most 2^254 in magnitude and a slice holds fewer than 2^64 terms,
/// so every partial sum is below 2^318. The final `Zero` arm therefore cannot be reached for any
/// input; it exists only because checked arithmetic returns an `Option`, and it never panics.
///
/// Because the fallback is exact, a wrong answer can never reveal that it was taken, so this
/// module's unit tests check the budget itself: at every extreme configuration and on random
/// points, the `I256` evaluation succeeds and stays within the documented bound.
fn sign_of_sum_of_products(terms: &[(i128, i128)]) -> Sign {
    match sum_of_products::<4>(terms) {
        Some(sum) => sum.signum(),
        None => sum_of_products::<8>(terms).map_or(Sign::Zero, |sum| sum.signum()),
    }
}

/// Which way the points a, b, c turn.
///
/// The sign of det[b − a; c − a] = (b.x − a.x)(c.y − a.y) − (b.y − a.y)(c.x − a.x), twice the signed
/// area of the triangle a, b, c. `Positive` when a, b, c turn counter-clockwise (c lies to the left
/// of the directed line from a to b), `Negative` when they turn clockwise, `Zero` exactly when they
/// are collinear, which includes any two coinciding. Swapping any two points flips the sign. Same
/// sign as `orient2d` in Shewchuk's `predicates.c`.
///
/// Bit budget, all in `i128`: each difference is at most δ < 2^41; each of the two products at most
/// δ² < 2^82; their difference at most 2δ² < 2^83, far below 2^127. The largest value reached is
/// δ² = 4M² (a triangle that is half the bounding square, such as (−M, −M), (M, −M), (M, M)).
#[must_use]
pub fn orient2d(a: P2, b: P2, c: P2) -> Sign {
    Sign::of_i128(D2::sub(b, a).cross(D2::sub(c, a)))
}

/// Which side of the plane through a, b, c the point d lies on.
///
/// The sign of det[b − a; c − a; d − a] = ((b − a) × (c − a)) · (d − a), six times the signed volume
/// of the tetrahedron a, b, c, d. `Positive` when d lies on the side the right-hand normal
/// (b − a) × (c − a) points to, so that a, b, c turn counter-clockwise seen from d; `Negative` on
/// the other side; `Zero` exactly when the four points are coplanar, which includes a, b, c being
/// collinear. Swapping any two points flips the sign. This is the opposite sign to `orient3d` in
/// Shewchuk's `predicates.c` (see the module documentation).
///
/// Bit budget, all in `i128`: each difference is at most δ < 2^41; each component of
/// (c − a) × (d − a) at most 2δ² < 2^83; each of the three products in the dot product with b − a
/// at most 2δ³ < 2^124; the sum at most 6δ³ < 2^126 (6 < 2^3, δ³ < 2^123), below 2^127. The largest
/// value reached is 2δ³ = 16M³: the regular tetrahedron on alternate corners of the bounding cube,
/// the largest tetrahedron in a cube (a third of its volume).
#[must_use]
pub fn orient3d(a: P3, b: P3, c: P3, d: P3) -> Sign {
    Sign::of_i128(det3(D3::sub(b, a), D3::sub(c, a), D3::sub(d, a)))
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
/// `Negative` when strictly outside; both flip if a, b, c turn clockwise; `Zero` exactly when d is on
/// the circle. If a, b, c are collinear and distinct the circle degenerates to their line and `Zero`
/// means d is on it; if two of them coincide every d gives `Zero`. Swapping any two of the four
/// points flips the sign. Same sign as `incircle` in Shewchuk's `predicates.c`.
///
/// Bit budget: each difference is at most δ < 2^41. In `i128`: each lift |p′|² is at most
/// 2δ² < 2^83, each 2 × 2 determinant p′ × q′ at most 2δ² < 2^83. In [`I256`](crate::wide::I256)
/// (the wide step, with its proven-unreachable fallback to I512): each of the three products at most
/// 4δ⁴ < 2^166 (δ⁴ < 2^164); the sum at most 12δ⁴ < 2^168 (12 < 2^4), 87 bits below the I256 limit
/// of 2^255. The largest value reached over all points with coordinates in {−M, 0, M} is
/// 8M⁴ = δ⁴ / 2: three corners of the bounding square and d at its centre.
#[must_use]
pub fn incircle(a: P2, b: P2, c: P2, d: P2) -> Sign {
    sign_of_sum_of_products(&incircle_terms(a, b, c, d))
}

/// The (lift, 2 × 2 determinant) pairs whose products sum to [`incircle`]'s determinant. Budgets
/// as derived there: every factor below 2^83, every product below 2^166, the sum below 2^168.
fn incircle_terms(a: P2, b: P2, c: P2, d: P2) -> [(i128, i128); 3] {
    let a = D2::sub(a, d);
    let b = D2::sub(b, d);
    let c = D2::sub(c, d);
    [(a.norm2(), b.cross(c)), (b.norm2(), c.cross(a)), (c.norm2(), a.cross(b))]
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
/// `orient3d(a, b, c, d)` is `Negative`; `Zero` exactly when e is on the sphere. If a, b, c, d are
/// coplanar there is no single sphere through them: if they lie on one circle or one line (which
/// includes two coinciding) every e gives `Zero`; otherwise `Zero` means e is in their plane.
/// Swapping any two of the five points flips the sign. This is the opposite sign to `insphere` in
/// Shewchuk's `predicates.c`, matching this module's [`orient3d`] (see the module documentation).
///
/// Bit budget: each difference is at most δ < 2^41. In `i128`: each lift |p′|² is at most
/// 3δ² < 2^84; each 3 × 3 determinant at most 6δ³ < 2^126 (the same derivation as [`orient3d`]),
/// so negating one cannot overflow. In [`I256`](crate::wide::I256) (the wide step, with its
/// proven-unreachable fallback to I512): each of the four products at most 18δ⁵ < 2^210 (18 < 2^5,
/// δ⁵ < 2^205); the sum at most 72δ⁵ < 2^212 (72 < 2^7), 43 bits below the I256 limit of 2^255.
/// The largest value reached by points at the corners and centre of the bounding cube is
/// 48M⁵ = 3δ⁵ / 2: a regular tetrahedron on alternate corners and e at the centre.
#[must_use]
pub fn insphere(a: P3, b: P3, c: P3, d: P3, e: P3) -> Sign {
    sign_of_sum_of_products(&insphere_terms(a, b, c, d, e))
}

/// The (lift, signed 3 × 3 cofactor) pairs whose products sum to [`insphere`]'s determinant.
/// Budgets as derived there: lifts below 2^84, cofactors below 2^126, every product below 2^210,
/// the sum below 2^212.
fn insphere_terms(a: P3, b: P3, c: P3, d: P3, e: P3) -> [(i128, i128); 4] {
    let a = D3::sub(a, e);
    let b = D3::sub(b, e);
    let c = D3::sub(c, e);
    let d = D3::sub(d, e);
    [
        (a.norm2(), det3(b, c, d)),
        (b.norm2(), -det3(a, c, d)),
        (c.norm2(), det3(a, b, d)),
        (d.norm2(), -det3(a, b, c)),
    ]
}

#[cfg(test)]
mod tests {
    //! The wide step on its own. Its fallback is exact, so no wrong sign can reveal that it was
    //! taken: these tests check the bit budgets directly (every factor, product and sum within its
    //! documented bound, in `I256`, at every extreme configuration and on random points), and drive
    //! the fallback with sums no grid input can produce, to show it is exact, not merely present.

    use proptest::prelude::*;

    use super::{incircle_terms, insphere_terms, sign_of_sum_of_products, sum_of_products};
    use crate::grid::{COORD_MAX, P2, P3};
    use crate::sign::Sign;
    use crate::wide::I256;

    const M: i64 = COORD_MAX;

    /// Whether −2^bits < v < 2^bits, for bits < 255.
    fn below(v: I256, bits: u32) -> bool {
        let mut limbs = [0u64; 4];
        limbs[(bits / 64) as usize] = 1 << (bits % 64);
        let bound = I256::from_le_limbs(limbs);
        bound.checked_neg().unwrap() < v && v < bound
    }

    /// The I256 evaluation succeeds, and the factors, every product and the sum stay strictly below
    /// 2^x_bits, 2^y_bits, 2^product_bits and 2^sum_bits in magnitude.
    fn assert_within(terms: &[(i128, i128)], [x_bits, y_bits, product_bits, sum_bits]: [u32; 4]) {
        for &(x, y) in terms {
            assert!(x.unsigned_abs() < 1 << x_bits && y.unsigned_abs() < 1 << y_bits, "{terms:?}");
            assert!(below(sum_of_products::<4>(&[(x, y)]).unwrap(), product_bits), "{terms:?}");
        }
        let sum = sum_of_products::<4>(terms).expect("I256 must hold the sum");
        assert!(below(sum, sum_bits), "{terms:?}");
    }

    const INCIRCLE_BITS: [u32; 4] = [83, 83, 166, 168];
    const INSPHERE_BITS: [u32; 4] = [84, 126, 210, 212];

    #[test]
    fn budgets_hold_at_the_extremes() {
        let v = [-M, 0, M];
        // incircle: every ordered choice of four points with coordinates in {−M, 0, M}.
        let square: Vec<P2> = v.iter().flat_map(|&x| v.iter().map(move |&y| P2::new(x, y).unwrap())).collect();
        for code in 0..square.len().pow(4) {
            let i = |k: u32| square[code / square.len().pow(k) % square.len()];
            assert_within(&incircle_terms(i(0), i(1), i(2), i(3)), INCIRCLE_BITS);
        }
        // insphere: every ordered choice of five points among the corners and the centre of the cube.
        let corner = |bit: i64| if bit == 0 { -M } else { M };
        let mut cube: Vec<P3> =
            (0..8i64).map(|k| P3::new(corner(k & 1), corner(k >> 1 & 1), corner(k >> 2 & 1)).unwrap()).collect();
        cube.push(P3::new(0, 0, 0).unwrap());
        for code in 0..cube.len().pow(5) {
            let i = |k: u32| cube[code / cube.len().pow(k) % cube.len()];
            assert_within(&insphere_terms(i(0), i(1), i(2), i(3), i(4)), INSPHERE_BITS);
        }
    }

    /// A coordinate anywhere on the grid or at an edge.
    fn coord() -> impl Strategy<Value = i64> {
        prop_oneof![-M..=M, Just(M), Just(-M)]
    }

    proptest! {
        #[test]
        fn budgets_hold_on_random_points(c in proptest::array::uniform::<_, 15>(coord())) {
            let p2 = |k: usize| P2::new(c[k], c[k + 1]).unwrap();
            assert_within(&incircle_terms(p2(0), p2(2), p2(4), p2(6)), INCIRCLE_BITS);
            let p3 = |k: usize| P3::new(c[k], c[k + 1], c[k + 2]).unwrap();
            assert_within(&insphere_terms(p3(0), p3(3), p3(6), p3(9), p3(12)), INSPHERE_BITS);
        }
    }

    #[test]
    fn fallback_to_i512_is_exact_when_i256_overflows() {
        let max = i128::MAX; // 2^127 − 1
        let min = i128::MIN; // −2^127
        let cases: [(&[(i128, i128)], Sign); 5] = [
            // 3 (2^127 − 1)² > 2^255 − 1: the third partial sum leaves I256.
            (&[(max, max); 4], Sign::Positive),
            // 3 × (−2^127 (2^127 − 1)) < −2^255: the third partial sum leaves I256 below its minimum.
            (&[(min, max); 4], Sign::Negative),
            // 2^254 + 2^254 = 2^255 leaves I256 at the second partial sum; the total is exactly zero.
            (&[(min, min), (min, min), (min, max), (min, 1), (min, max), (min, 1)], Sign::Zero),
            // The third partial sum leaves I256 and the total comes back to −1.
            (&[(max, max), (max, max), (max, max), (max, -max), (max, -max), (max, -max), (1, -1)], Sign::Negative),
            // The same, ending at +1.
            (&[(max, max), (max, max), (max, max), (max, -max), (max, -max), (max, -max), (1, 1)], Sign::Positive),
        ];
        for (terms, expected) in cases {
            assert_eq!(sum_of_products::<4>(terms), None, "this case must exercise the fallback");
            assert_eq!(sign_of_sum_of_products(terms), expected);
        }
    }

    #[test]
    fn i256_holds_every_single_product() {
        // Every product of two i128 values fits I256, including the largest, 2^254.
        for (x, y) in [(i128::MIN, i128::MIN), (i128::MIN, i128::MAX), (i128::MAX, i128::MAX)] {
            assert!(sum_of_products::<4>(&[(x, y)]).is_some());
        }
        assert_eq!(sign_of_sum_of_products(&[]), Sign::Zero);
    }
}
