//! Where two lines cross in the plane, exact: the implicit point where the line through two grid
//! points crosses the line through two others, which side of a third line it lies on, and
//! committing it to the grid.
//!
//! Two segments of a 2D drawing or a face's boundary that cross meet at such a point. Like a
//! three-plane point ([`crate::plane::Tpi`]), it is kept as its definition and never rounded inside
//! an operation (chapter 6.3), and every question about it is answered exactly from that
//! definition.
//!
//! Guarantees:
//!
//! - Every result is exact, or an error. A [`Sign`] is decided by integer arithmetic alone, never
//!   by a rounded crossing point, so an arrangement built on these answers is consistent.
//! - Nothing overflows silently. Every step is a checked operation in `i128` or a fixed-width
//!   [`Wide`] integer, and a step that did not fit would be reported as
//!   [`NumError::BudgetExceeded`], never answered; for grid points no step can fail (the budgets
//!   on [`Ssi::homogeneous`] and [`Ssi::orient2d`]).
//! - Deterministic: integers only, except the generated `f64` filters, whose answers are proven to
//!   be the exact sign or no answer (chapter 6.2); no platform maths and no allocation.
//! - **Bounded proofs, and what is only tested** (chapter 28.7; piece G41 collects them here).
//!   Verus proves that the generated exact paths of `ssi_weight` and `ssi_side`, which every
//!   answer here rests on, compute their values exactly and never overflow for every input in
//!   their declared ranges (piece G39: the twins in `verus/generated/`, token for token the shipped
//!   modules), on [`Wide`] arithmetic that is itself proven exact (G37). The routing between
//!   filter and exact path, the homogeneous coordinates' budgets and the rounding to the grid are
//!   tested against exact oracles, not proven (`tests/attack_g29.rs`, `tests/verification.rs`
//!   and this module's tests), and the filters' error bounds are checked against exact
//!   arithmetic until G40 certifies them.
//!
//! Method: the crossing point of two lines in homogeneous integer coordinates, classified against
//! a line by multiplying by the sign of the weight so that nothing is divided, the plane-based
//! method of [`crate::plane`] in two dimensions (Campen and Kobbelt 2010,
//! <https://doi.org/10.1111/j.1467-8659.2009.01609.x>; Attene 2020's indirect predicates,
//! <https://doi.org/10.1016/j.cad.2020.102856>, method only, re-derived here). Each sign goes
//! through its generated filter first and its generated exact path when the filter cannot decide
//! (`ssi_weight` and `ssi_side`, chapter 6.2), then through the hand-written path.
//!
//! Cost: constant time and no allocation: a few `i128` products for the weight and the
//! numerators, two products in [`I256`] for a side, and one restoring long division per
//! coordinate to commit, as in [`crate::plane`].

use crate::error::NumError;
use crate::generated;
use crate::grid::{Coord, P2, Rounded};
use crate::plane::{BUDGET_LIMBS, TOTAL_LIMBS, commit_axes, difference};
use crate::sign::Sign;
use crate::wide::{I256, I320, Wide};

/// The point where the line from `a` to `b` crosses the line from `c` to `d`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ssi {
    /// The first line's two points, then the second line's.
    points: [P2; 4],
}

impl Ssi {
    /// The point where the line from `a` to `b` crosses the line from `c` to `d`.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] when the lines meet in no point or in a whole line: they are
    /// parallel or the same line, or a = b, or c = d. Decided exactly: w = (b − a) × (d − c) = 0.
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b, c and d are the two lines' points, p and q the third line's, and w the weight, as in the formulas"
    )]
    pub fn new(a: P2, b: P2, c: P2, d: P2) -> Result<Ssi, NumError> {
        let s = Ssi {
            points: [a, b, c, d],
        };
        match s.offsets() {
            Some((oa, ob)) if oa == ob => Err(NumError::Degenerate),
            Some(_) => Ok(s),
            None => Err(NumError::BudgetExceeded),
        }
    }

    /// The four points, in the order given to [`Ssi::new`].
    #[must_use]
    pub const fn points(&self) -> [P2; 4] {
        self.points
    }

    /// oa = orient2d(c, d, a) and ob = orient2d(c, d, b), in `i128`: how far a and b lie on
    /// either side of the second line.
    fn offsets(&self) -> Option<(i128, i128)> {
        let [a, b, c, d] = self.points;
        Some((cross(c, d, a)?, cross(c, d, b)?))
    }

    /// The point's homogeneous coordinates: numerators X and denominator w, with the point at
    /// X / w. With oa = orient2d(c, d, a) and ob = orient2d(c, d, b), the point a + t (b − a) lies
    /// on the line from c to d for t = oa / (oa − ob), so X = oa b − ob a and w = oa − ob =
    /// (b − a) × (d − c), never zero.
    ///
    /// Bit budget: with coordinates within ±M (M = 2^40 − 1), each of oa, ob is a 2 × 2
    /// determinant of differences within ±2M, whose largest magnitude lies at a corner of the box,
    /// at most 4M² (82 bits); so |w| ≤ 8M² and |`X_i`| ≤ 8M³ (123 bits), all in `i128`.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step does not fit, which no grid points can cause (the
    /// budget above).
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b, c and d are the two lines' points, p and q the third line's, and w the weight, as in the formulas"
    )]
    pub fn homogeneous(&self) -> Result<([i128; 2], i128), NumError> {
        let [a, b, _, _] = self.points;
        let Some((oa, ob)) = self.offsets() else {
            return Err(NumError::BudgetExceeded);
        };
        let (Some(x), Some(y), Some(w)) = (
            ssi_axis(oa, ob, b.x, a.x),
            ssi_axis(oa, ob, b.y, a.y),
            oa.checked_sub(ob),
        ) else {
            return Err(NumError::BudgetExceeded);
        };
        Ok(([x, y], w))
    }

    /// Which side of the line from `p` to `q` the point lies on: orient2d(p, q, x) for the
    /// crossing point x, in [`crate::predicate::orient2d`]'s convention: `Positive` when p, q, x
    /// turn counter-clockwise, `Zero` exactly when x lies on the line.
    ///
    /// orient2d(p, q, ·) is affine, so orient2d(p, q, X / w) = (oa · orient2d(p, q, b) −
    /// ob · orient2d(p, q, a)) / w, whose sign is the sign of oa · ob₂ − ob · oa₂ (with
    /// oa₂ = orient2d(p, q, a), ob₂ = orient2d(p, q, b)) times the sign of w.
    ///
    /// Bit budget: each orient2d of grid points is at most 4M² (82 bits), so each product is at
    /// most 16M⁴ and their difference at most 32M⁴ < 2^165, in [`I256`]. Total path, unreachable:
    /// should a step not fit anyway, the same difference is computed in [`I320`], where two
    /// products of `i128`s (at most 2^254 each) cannot overflow.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if an orient2d does not fit an `i128` or the total path
    /// failed, neither of which grid points can cause.
    pub fn orient2d(&self, p: P2, q: P2) -> Result<Sign, NumError> {
        self.routed(p, q)
            .map_or_else(|| self.orient2d_hand(p, q), Ok)
    }

    /// [`Ssi::orient2d`] through the generated predicates: `ssi_side` times `ssi_weight`, each
    /// filter first and its generated exact path otherwise. `None` only when a generated exact
    /// path does not fit its certified types.
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b, c and d are the two lines' points, p and q the third line's, and w the weight, as in the formulas"
    )]
    fn routed(&self, p: P2, q: P2) -> Option<Sign> {
        let [a, b, c, d] = self.points;
        let weight = generated::ssi_weight::filter(a, b, c, d)
            .or_else(|| generated::ssi_weight::exact(a, b, c, d))?;
        let side = generated::ssi_side::filter(a, b, c, d, p, q)
            .or_else(|| generated::ssi_side::exact(a, b, c, d, p, q))?;
        Some(side.mul(weight))
    }

    /// [`Ssi::orient2d`] by hand: oa · ob₂ − ob · oa₂ in [`I256`], then in [`I320`], times the
    /// sign of w.
    #[inline(never)]
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b, c and d are the two lines' points, p and q the third line's, and w the weight, as in the formulas"
    )]
    fn orient2d_hand(&self, p: P2, q: P2) -> Result<Sign, NumError> {
        let [a, b, _, _] = self.points;
        let (Some((oa, ob)), Some(oa2), Some(ob2)) =
            (self.offsets(), cross(p, q, a), cross(p, q, b))
        else {
            return Err(NumError::BudgetExceeded);
        };
        let Some(w) = oa.checked_sub(ob) else {
            return Err(NumError::BudgetExceeded);
        };
        let s = match side::<4>(oa, ob2, ob, oa2) {
            Some(s) => Some(s),
            None => side::<5>(oa, ob2, ob, oa2),
        };
        match s {
            Some(s) => Ok(s.mul(Sign::of_i128(w))),
            None => Err(NumError::BudgetExceeded),
        }
    }

    /// The point committed to the grid: each coordinate `X_i` / w rounded half to even, by the same
    /// code that commits every implicit point ([`crate::plane::Tpi::to_grid`]), with whether both
    /// coordinates were exact.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] for the first coordinate outside the frame (two lines can cross
    /// far outside it), and [`NumError::BudgetExceeded`] as for [`Ssi::homogeneous`].
    pub fn to_grid(&self) -> Result<Rounded<P2>, NumError> {
        let ([x, y], w) = self.homogeneous()?;
        let x = [I320::from_i128(x), I320::from_i128(y)];
        let w = I256::from_i128(w);
        commit_axes::<BUDGET_LIMBS, 2>(&x, &w)
            .or_else(|| commit_axes::<TOTAL_LIMBS, 2>(&x, &w))
            .unwrap_or(Err(NumError::BudgetExceeded))
            .map(|([x, y], exact)| Rounded::new(P2 { x, y }, exact))
    }
}

/// (b − a) × (c − a) = orient2d(a, b, c) for grid points, in `i128`: `None` if a step does not
/// fit, which coordinates within the frame cannot cause.
fn cross(a: P2, b: P2, c: P2) -> Option<i128> {
    let (bx, by, cx, cy) = (
        difference(b.x, a.x)?,
        difference(b.y, a.y)?,
        difference(c.x, a.x)?,
        difference(c.y, a.y)?,
    );
    bx.checked_mul(cy)?.checked_sub(by.checked_mul(cx)?)
}

/// One numerator of [`Ssi::homogeneous`], oa · `b_i` − ob · `a_i`, in `i128`: `None` if a step does
/// not fit, which grid points cannot cause.
fn ssi_axis(oa: i128, ob: i128, bc: Coord, ac: Coord) -> Option<i128> {
    oa.checked_mul(i128::from(bc.get()))?
        .checked_sub(ob.checked_mul(i128::from(ac.get()))?)
}

/// The sign of oa · ob₂ − ob · oa₂ in `Wide<L>`: `None` if a step does not fit.
fn side<const L: usize>(oa: i128, ob2: i128, ob: i128, oa2: i128) -> Option<Sign> {
    let left = Wide::<L>::product_i128(oa, ob2)?;
    let right = Wide::<L>::product_i128(ob, oa2)?;
    Some(left.checked_sub(&right)?.signum())
}
