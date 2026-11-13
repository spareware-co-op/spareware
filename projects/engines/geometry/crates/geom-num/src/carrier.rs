//! Lines and rational points in 2D, exact: integer carriers, the rational points where two of them
//! meet, and the side, order and equality questions on both.
//!
//! A straight drafting edge is stored as its carrier, the whole line a · x + b · y = c it lies on
//! (chapters 14.2 and 15.10), as a flat face is stored as its plane. Where two carriers meet is a
//! **depth-one point**, held as three integers X, Y and W meaning the point (X / W, Y / W), so the
//! meeting needs no division and lies exactly on both. A T-junction on a sloping wall is such a point:
//! rounded to the grid it would leave a gap, and split the wall's one carrier into two (15.10).
//! [`Rational2`] holds these and every other point of 14.2's rational class within the same budget.
//! No carrier is ever defined from a rational point, which is what keeps every width here fixed.
//!
//! Guarantees:
//!
//! - Every result is exact, or an error. A [`Sign`] is decided by integer arithmetic alone, after a
//!   generated `f64` filter whose answers are proven to be the exact sign or no answer (chapter
//!   6.2); nothing rounds a point to answer a question about it.
//! - Nothing overflows silently. A [`Carrier`] and a [`Rational2`] can only be made within the
//!   budget below, and every step is a checked operation or the generated exact path, whose
//!   certificate holds every step in its type for every input in these budgets (`generated`). A
//!   step that did not fit would be [`NumError::BudgetExceeded`], never an answer. Every width in
//!   the table is asserted when the crate compiles, and tested at the extreme values.
//!
//!   | Quantity | Bound | Width with its sign |
//!   | --- | --- | --- |
//!   | A carrier's a and b | 2^41 | |
//!   | A carrier's c | 2^82 | |
//!   | A rational point's X and Y | 2^124 | |
//!   | A rational point's W | 2^83 | |
//!   | Side of a carrier | a · X + b · Y − c · W | 168 bits |
//!   | Order along a carrier | (q − p) · (b, −a) · `W_p` · `W_q` | 251 bits |
//!   | Comparing one coordinate | X · `W_q` − `X_q` · W | 209 bits |
//!   | Which way three points turn | their rows' determinant | 335 bits |
//!
//! - A weight is positive. [`Rational2::new`] turns the three integers' signs so that W > 0, so no
//!   predicate here multiplies by the sign of a weight.
//! - Deterministic: integers only, except the generated filters; no platform maths and no
//!   allocation.
//!
//! Method: chapter 15.10's forms and bounds, which are 32.26's decision on the 2D numbers
//! (`projects/engines/geometry/architecture/32-decisions.md`, owner action G186); a meeting of lines
//! in homogeneous integer coordinates is Cramer's rule without its division, and classifying a
//! homogeneous point without dividing is the plane-based method of [`crate::plane`] in two dimensions
//! (Campen and Kobbelt 2010, <https://doi.org/10.1111/j.1467-8659.2009.01609.x>; Attene 2020's
//! indirect predicates, <https://doi.org/10.1016/j.cad.2020.102856>, method only, re-derived here).
//! Each sign goes through its generated filter first and its generated exact path when the filter
//! cannot decide (`carrier_side`, `carrier_order`, `rational_cmp` and `rational_orient`, chapter 6.2).
//!
//! Cost: constant time and no allocation: a few `i128` products to make a point, and a generated
//! filter of at most 20 `f64` operations before an exact path of at most 26 fixed-width ones.

use crate::error::NumError;
use crate::generated;
use crate::grid::{Coord, P2};
use crate::sign::Sign;

/// A carrier's a and b are below 2 to this in magnitude: the difference of two grid coordinates.
pub const NORMAL_BITS: u32 = 41;
/// A carrier's c is below 2 to this in magnitude.
pub const OFFSET_BITS: u32 = 82;
/// A rational point's X and Y are below 2 to this in magnitude: 2 · 2^82 · 2^41, from Cramer's rule.
pub const POINT_BITS: u32 = 124;
/// A rational point's W is below 2 to this in magnitude: 2 · 2^41 · 2^41.
pub const WEIGHT_BITS: u32 = 83;

// The budget's arithmetic, in the order the table above gives it: the two sums of a meeting, then
// the largest term of each predicate, with what its sums add and its sign bit.
const _: () = {
    assert!(OFFSET_BITS + NORMAL_BITS + 1 == POINT_BITS);
    assert!(NORMAL_BITS + NORMAL_BITS + 1 == WEIGHT_BITS);
    assert!(NORMAL_BITS + POINT_BITS == 165 && OFFSET_BITS + WEIGHT_BITS == 165);
    assert!(POINT_BITS + WEIGHT_BITS + 1 + NORMAL_BITS + 1 + 1 == 251);
    assert!(POINT_BITS + WEIGHT_BITS + 1 + 1 == 209);
    assert!(2 * POINT_BITS + WEIGHT_BITS + 3 + 1 == 335);
};

/// The largest magnitude below 2 to `bits`.
const fn below(bits: u32) -> i128 {
    (1_i128 << bits).saturating_sub(1)
}

/// Whether `v` is below 2 to `bits` in magnitude.
const fn fits(v: i128, bits: u32) -> bool {
    v.unsigned_abs() <= below(bits).unsigned_abs()
}

/// A whole line in the plane: a · x + b · y = c, in integers within the budget.
///
/// It runs along (b, −a), so a point on its positive side lies to the left of the way it runs, as
/// the third point of [`crate::predicate::orient2d`] does. One line has many carriers, each a whole
/// multiple of the smallest: [`Carrier::same`] says whether two are one line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Carrier {
    a: i128,
    b: i128,
    c: i128,
}

impl Carrier {
    /// The carrier a · x + b · y = c.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] for a coefficient over its budget, and [`NumError::Degenerate`]
    /// when a and b are both zero, which is no line.
    pub const fn new(a: i128, b: i128, c: i128) -> Result<Carrier, NumError> {
        if !fits(a, NORMAL_BITS) || !fits(b, NORMAL_BITS) || !fits(c, OFFSET_BITS) {
            return Err(NumError::BudgetExceeded);
        }
        if a == 0 && b == 0 {
            return Err(NumError::Degenerate);
        }
        Ok(Carrier { a, b, c })
    }

    /// The carrier running from `p` to `q`: a = −(q.y − p.y), b = q.x − p.x, c = a · p.x + b · p.y.
    ///
    /// Each difference of grid coordinates is below 2^41 and c, two products of it by a coordinate,
    /// below 2 · 2^41 · 2^40 = 2^82, so no pair of grid points can leave the budget.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] when `p` and `q` are one point.
    #[allow(
        clippy::many_single_char_names,
        reason = "p and q are the points, a, b and c the line's coefficients, as in the formula"
    )]
    pub fn through(p: P2, q: P2) -> Result<Carrier, NumError> {
        let wide = |v: Coord| i128::from(v.get());
        let a = wide(p.y)
            .checked_sub(wide(q.y))
            .ok_or(NumError::BudgetExceeded)?;
        let b = wide(q.x)
            .checked_sub(wide(p.x))
            .ok_or(NumError::BudgetExceeded)?;
        let c = product_sum(a, wide(p.x), b, wide(p.y))?;
        Carrier::new(a, b, c)
    }

    /// The coefficients: `[a, b]` and c.
    #[must_use]
    pub const fn coefficients(&self) -> ([i128; 2], i128) {
        ([self.a, self.b], self.c)
    }

    /// The way the carrier runs: (b, −a).
    #[must_use]
    pub const fn direction(&self) -> [i128; 2] {
        [self.b, 0 - self.a]
    }

    /// Which side of the carrier `p` lies on: the sign of a · X + b · Y − c · W, W being positive.
    /// Positive to the left of the way the carrier runs, zero on it.
    ///
    /// Bit budget: |a · X| and |b · Y| are below 2^165 and so is |c · W|, so the value is below
    /// 3 · 2^165, 168 bits with its sign, 15.10's figure, which the generated certificate re-derives.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step of the exact path did not fit, which no input in the
    /// budget can reach.
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b and c are the line's coefficients, x, y and w the point's, as in the formula"
    )]
    pub fn side(&self, p: &Rational2) -> Result<Sign, NumError> {
        let (a, b, c) = (self.a, self.b, self.c);
        let (x, y, w) = (p.x, p.y, p.w);
        generated::carrier_side::filter(a, b, c, x, y, w)
            .or_else(|| generated::carrier_side::exact(a, b, c, x, y, w))
            .ok_or(NumError::BudgetExceeded)
    }

    /// Which way along the carrier `q` lies from `p`: the sign of (q − p) · (b, −a), positive when
    /// `q` lies ahead of `p`, zero when they are level. For points off the carrier it is the order
    /// of their feet on it.
    ///
    /// Bit budget: each of `X_q` · `W_p` − `X_p` · `W_q` and its y twin is below 2 · 2^207, a product
    /// by a or b below 2^249, and their sum below 2^250, 251 bits with its sign, 15.10's figure.
    ///
    /// # Errors
    ///
    /// As for [`Carrier::side`].
    pub fn order(&self, p: &Rational2, q: &Rational2) -> Result<Sign, NumError> {
        let (a, b) = (self.a, self.b);
        let (xp, yp, wp) = (p.x, p.y, p.w);
        let (xq, yq, wq) = (q.x, q.y, q.w);
        generated::carrier_order::filter(a, b, xp, yp, xq, yq, wp, wq)
            .or_else(|| generated::carrier_order::exact(a, b, xp, yp, xq, yq, wp, wq))
            .ok_or(NumError::BudgetExceeded)
    }

    /// Where this carrier meets `other`: Cramer's rule without its division, X = c₁ · b₂ − c₂ · b₁,
    /// Y = a₁ · c₂ − a₂ · c₁ and W = a₁ · b₂ − a₂ · b₁, each two products within the budget, so
    /// the point lies exactly on both.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] when the carriers are parallel, or one line.
    pub fn meet(&self, other: &Carrier) -> Result<Rational2, NumError> {
        let (p, q) = (self, other);
        Rational2::new(
            product_difference(p.c, q.b, q.c, p.b)?,
            product_difference(p.a, q.c, q.a, p.c)?,
            product_difference(p.a, q.b, q.a, p.b)?,
        )
    }

    /// Whether this carrier runs parallel to `other`, the same way or the other: W = 0 in
    /// [`Carrier::meet`].
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step did not fit, which no carrier can reach.
    pub fn parallel(&self, other: &Carrier) -> Result<bool, NumError> {
        let weight = product_difference(self.a, other.b, other.a, self.b)?;
        Ok(weight == 0)
    }

    /// Whether this carrier and `other` are one line, whichever way each runs: parallel, with
    /// X = Y = 0 in [`Carrier::meet`].
    ///
    /// # Errors
    ///
    /// As for [`Carrier::parallel`].
    pub fn same(&self, other: &Carrier) -> Result<bool, NumError> {
        let (p, q) = (self, other);
        Ok(p.parallel(q)?
            && product_difference(p.c, q.b, q.c, p.b)? == 0
            && product_difference(p.a, q.c, q.a, p.c)? == 0)
    }
}

/// p · q − r · s, each product within `i128` for any values in the budget.
fn product_difference(p: i128, q: i128, r: i128, s: i128) -> Result<i128, NumError> {
    p.checked_mul(q)
        .zip(r.checked_mul(s))
        .and_then(|(m, n)| m.checked_sub(n))
        .ok_or(NumError::BudgetExceeded)
}

/// p · q + r · s, as [`product_difference`].
fn product_sum(p: i128, q: i128, r: i128, s: i128) -> Result<i128, NumError> {
    p.checked_mul(q)
        .zip(r.checked_mul(s))
        .and_then(|(m, n)| m.checked_add(n))
        .ok_or(NumError::BudgetExceeded)
}

/// A point of the plane at (X / W, Y / W), integers within the budget and W positive.
///
/// One place has many such triples, each a whole multiple of the smallest, so two points are
/// compared by [`Rational2::same`], never field by field.
#[derive(Clone, Copy, Debug)]
pub struct Rational2 {
    x: i128,
    y: i128,
    w: i128,
}

impl Rational2 {
    /// The point (x / w, y / w), the three integers' signs turned if w is negative.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] for an integer over its budget, and [`NumError::Degenerate`] for
    /// w = 0, which is no point.
    pub fn new(x: i128, y: i128, w: i128) -> Result<Rational2, NumError> {
        if !fits(x, POINT_BITS) || !fits(y, POINT_BITS) || !fits(w, WEIGHT_BITS) {
            return Err(NumError::BudgetExceeded);
        }
        if w == 0 {
            return Err(NumError::Degenerate);
        }
        // Each is below 2^124, so turning it cannot overflow.
        let turned = |v: i128| if w < 0 { v.checked_neg() } else { Some(v) };
        match (turned(x), turned(y), turned(w)) {
            (Some(x), Some(y), Some(w)) => Ok(Rational2 { x, y, w }),
            _ => Err(NumError::BudgetExceeded),
        }
    }

    /// The homogeneous coordinates: `[X, Y]` and W, W positive.
    #[must_use]
    pub const fn coordinates(&self) -> ([i128; 2], i128) {
        ([self.x, self.y], self.w)
    }

    /// How this point's x compares with `other`'s: the sign of x − `other`'s x.
    ///
    /// Bit budget: each product is below 2^207, so the difference is below 2^208, 209 bits with
    /// its sign.
    ///
    /// # Errors
    ///
    /// As for [`Carrier::side`].
    pub fn cmp_x(&self, other: &Rational2) -> Result<Sign, NumError> {
        compare(self.x, other.x, self.w, other.w)
    }

    /// How this point's y compares with `other`'s: the sign of y − `other`'s y. As [`Rational2::cmp_x`].
    ///
    /// # Errors
    ///
    /// As for [`Carrier::side`].
    pub fn cmp_y(&self, other: &Rational2) -> Result<Sign, NumError> {
        compare(self.y, other.y, self.w, other.w)
    }

    /// Whether this point and `other` are one place, whatever their weights.
    ///
    /// # Errors
    ///
    /// As for [`Carrier::side`].
    pub fn same(&self, other: &Rational2) -> Result<bool, NumError> {
        Ok(self.cmp_x(other)? == Sign::Zero && self.cmp_y(other)? == Sign::Zero)
    }

    /// Which way `a`, `b` and `c` turn: the sign of the determinant of their homogeneous rows,
    /// positive counter-clockwise, as [`crate::predicate::orient2d`] is on grid points. The weights
    /// are positive, so it needs no correction.
    ///
    /// Bit budget: each 2 × 2 minor is below 2 · 2^248, a product by a weight below 2^332, and the
    /// three terms' sum below 3 · 2^332, 335 bits with its sign, 15.10's figure.
    ///
    /// # Errors
    ///
    /// As for [`Carrier::side`].
    pub fn orient(a: &Rational2, b: &Rational2, c: &Rational2) -> Result<Sign, NumError> {
        let (xa, ya, wa) = (a.x, a.y, a.w);
        let (xb, yb, wb) = (b.x, b.y, b.w);
        let (xc, yc, wc) = (c.x, c.y, c.w);
        generated::rational_orient::filter(xa, ya, xb, yb, xc, yc, wa, wb, wc)
            .or_else(|| generated::rational_orient::exact(xa, ya, xb, yb, xc, yc, wa, wb, wc))
            .ok_or(NumError::BudgetExceeded)
    }
}

impl From<P2> for Rational2 {
    /// A grid point, with weight 1: every grid coordinate is well inside the budget.
    fn from(p: P2) -> Rational2 {
        Rational2 {
            x: i128::from(p.x.get()),
            y: i128::from(p.y.get()),
            w: 1,
        }
    }
}

/// The sign of p / wp − q / wq, the weights being positive.
fn compare(p: i128, q: i128, wp: i128, wq: i128) -> Result<Sign, NumError> {
    generated::rational_cmp::filter(p, q, wp, wq)
        .or_else(|| generated::rational_cmp::exact(p, q, wp, wq))
        .ok_or(NumError::BudgetExceeded)
}
