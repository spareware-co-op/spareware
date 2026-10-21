//! Plane-based geometry, exact: integer planes, the implicit point where three planes meet, which
//! side of a plane such a point lies on, and committing it to the grid.
//!
//! A flat-faced solid is stored as its planes, and every vertex is the meeting of three of them
//! (chapter 6.3). A plane is an integer equation n · p + d = 0 made from three grid points. A
//! vertex is a [`Tpi`] (three-plane intersection) and is never rounded inside an operation: its
//! exact position is X / w in homogeneous integer coordinates, and every question about it is
//! answered exactly from that definition. Booleans make new solids out of the same input planes,
//! so the numbers never grow however many operations run: cutting a wall 300 times never drifts.
//!
//! Guarantees:
//!
//! - Every result is exact, or an error. A [`Sign`] is decided by integer arithmetic alone; no
//!   rounding ever decides it. Only [`Tpi::to_grid`] rounds: it rounds the exact rational point to
//!   the nearest grid point (ties to even) and says whether it rounded, moving each coordinate by
//!   at most [`COMMIT_PER_AXIS`](crate::ladder::COMMIT_PER_AXIS), half a step.
//! - Nothing overflows, and nothing could wrap silently if it did. A [`Plane`] can only be made by
//!   [`Plane::through`] from grid points (within 2^40 − 1 steps of the origin), which checks the
//!   plane's budget below at construction (chapter 6.2); a [`Tpi`] can only be made by
//!   [`Tpi::new`], which checks that its homogeneous coordinates fit their types. Every
//!   intermediate fits its type by the derivation written on each function; the arithmetic of
//!   every derivation, and every figure in the table below, is asserted when the crate compiles
//!   (the budget exponents in the source, from `GRID_DIFFERENCE_BITS` on); and every operation, on
//!   `i128` and on wide integers alike, is checked, so none of this depends on a build with
//!   overflow checks. In summary, with B = 40 and every coordinate below 2^B in magnitude:
//!
//!   | Quantity | Bound | Type |
//!   | --- | --- | --- |
//!   | Plane normal n | 2^(2B+3) = 2^83 | `i128` |
//!   | Plane offset d | 2^(3B+5) = 2^125 | `i128` |
//!   | Grid point against a plane, n · p + d | 2^126 | `i128` |
//!   | Three-plane denominator w | 2^(6B+12) = 2^252 | [`I256`] |
//!   | Three-plane numerators X | 2^(7B+14) = 2^294 | [`I320`] |
//!   | Three-plane point against a plane, n · X + d · w | 2^(9B+19) = 2^379 | [`I384`] |
//!
//!   At B = 26 the same formulas give 9B + 19 = 253 bits, within the 256 bits EMBER uses for its
//!   26-bit inputs (evidence note 05, section 4). The bounds are safe rather than tight: they
//!   bound every factor separately, and a triangle inside the frame cannot make every product
//!   maximal at once. With M = 2^40 − 1, what grid points can reach is |`n_i`| ≤ 4M² and
//!   |d| ≤ 4M³ (82 and 122 bits; derivations on [`Plane::through`]), |n · p + d| ≤ 16M³ (124
//!   bits, on [`Plane::side`]), and from the plane bounds, because each of w, `X_i` and
//!   n · X + d · w is a determinant whose largest magnitude over a box of entries lies at a corner
//!   of the box, |w| ≤ 256M^6, |`X_i`| ≤ 256M^7 and |n · X + d · w| ≤ 4096M^9 (248, 288 and 372
//!   bits; derivations on [`Tpi::homogeneous`] and [`Plane::side_of_tpi`]). Planes and
//!   three-plane points built from the corners of the frame's cube reach every one of these
//!   exactly (tested).
//! - Deterministic: integers only, no floating point, no platform maths, no allocation.
//!
//! Methods followed: plane-based geometry with vertices as three-plane intersections in
//! homogeneous integer coordinates, as in Campen and Kobbelt 2010
//! ([doi:10.1111/j.1467-8659.2009.01609.x](https://doi.org/10.1111/j.1467-8659.2009.01609.x)),
//! Nehring-Wirxel, Trettner and Kobbelt 2021
//! ([doi:10.1016/j.cad.2021.103015](https://doi.org/10.1016/j.cad.2021.103015)) and EMBER 2022
//! (Trettner, Nehring-Wirxel and Kobbelt,
//! [doi:10.1145/3528223.3530181](https://doi.org/10.1145/3528223.3530181)). Classifying a
//! homogeneous point by multiplying by the sign of w, so that nothing is divided, follows Attene
//! 2020's indirect predicates
//! ([doi:10.1016/j.cad.2020.102856](https://doi.org/10.1016/j.cad.2020.102856); method only,
//! re-derived here). The rounding division is restoring binary long division (Knuth, *The Art of
//! Computer Programming*, volume 2, section 4.3.1). The reach of the bounds uses Hadamard's
//! determinant inequality (J. Hadamard, 1893, *Bulletin des Sciences Mathématiques* 17).
//!
//! No answer is silently wrong, even if a bound were wrong:
//!
//! - a wrong derivation is a build error: its arithmetic is asserted when the crate compiles;
//! - [`Plane::side_of_tpi`] and [`Tpi::to_grid`] work in [`I384`], the budgeted path; should a step
//!   there not fit, they repeat the whole computation on the total path, in [`I512`], where no step
//!   can overflow for any value the types hold (derivations on each, also asserted at compile
//!   time), so the answer is still exact. (The predicates have a total path of their own, in
//!   [`crate::predicate`]; the term means the same in both: a path that holds every value of its
//!   argument types.)
//! - every other step that does not fit is reported as [`NumError::BudgetExceeded`], never
//!   answered: [`Plane::through`] and [`Tpi::new`] refuse a plane or a point over its budget, and
//!   [`Plane::side`], [`Plane::side_of_tpi`], [`Tpi::homogeneous`] and [`Tpi::to_grid`] return a
//!   `Result` for it, so no `Sign`, coordinates or point is ever made up.
//!
//! No input can reach a `BudgetExceeded`: `through` and `side` work from bounds checked when their
//! points and plane were made, under the asserted arithmetic; `homogeneous`, and the first step of
//! `side_of_tpi` and `to_grid`, repeat a computation [`Tpi::new`] completed for the same planes;
//! and the total path cannot overflow. Each could be reached only if the checked arithmetic
//! itself, Rust's on `i128` or [`Wide`]'s (tested against big integers in its own module),
//! reported an overflow for a result that fits; then the caller gets the error.

use core::cmp::Ordering;

use crate::error::NumError;
use crate::grid::{COORD_BITS, COORD_MAX, Coord, P3, Rounded};
use crate::sign::Sign;
use crate::wide::{I256, I320, Wide};
// Named in the documentation only; the code reaches them as `Wide<6>` and `Wide<8>`.
#[cfg(doc)]
use crate::wide::{I384, I512};

// The bit budget as exponents: each quantity below has magnitude less than 2 to its constant, for
// grid coordinates of B = `COORD_BITS` bits (M = 2^B − 1). A product of magnitudes below 2^a and
// 2^b is below 2^(a+b); a sum of two terms below 2^a is below 2^(a+1), and of three or four terms
// below 2^(a+2). The derivations in words are on the functions that use each bound. The assertions
// after the constants check their arithmetic, and the figures the documentation states, when the
// crate compiles, so a wrong bound is a build error, never a wrong answer.

/// A difference of two grid coordinates: |`b_i` − `a_i`| ≤ 2M < 2^(B+1) = 2^41
/// ([`Plane::through`]). Not the bound of [`crate::predicate`]'s total path, which takes any two
/// `i64` values (64 bits).
const GRID_DIFFERENCE_BITS: usize = COORD_BITS as usize + 1;

/// A plane normal component, a difference of two products of coordinate differences:
/// 2^(2B+3) = 2^83 ([`Plane::through`]).
const NORMAL_BITS: usize = 2 * GRID_DIFFERENCE_BITS + 1;

/// A plane offset, a sum of three products `n_i` · `a_i`; the same bound holds for `n_x`
/// `p_x` + `n_y` `p_y` + `n_z` `p_z` at any grid point p: 2^(3B+5) = 2^125 ([`Plane::through`]).
const OFFSET_BITS: usize = NORMAL_BITS + COORD_BITS as usize + 2;

/// A grid point against a plane, n · p + d, two terms below 2^(3B+5): 2^(3B+6) = 2^126
/// ([`Plane::side`]).
const SIDE_BITS: usize = OFFSET_BITS + 1;

/// A component of the cross product of two normals: 2^(4B+7) = 2^167 ([`Tpi::homogeneous`]).
const CROSS_BITS: usize = 2 * NORMAL_BITS + 1;

/// The three-plane denominator w = `n_1` · (`n_2` × `n_3`), three products: 2^(6B+12) = 2^252
/// ([`Tpi::homogeneous`]).
const W_BITS: usize = NORMAL_BITS + CROSS_BITS + 2;

/// A three-plane numerator `X_i`, three products `d_k` · (cross product)_i: 2^(7B+14) = 2^294
/// ([`Tpi::homogeneous`]).
const X_BITS: usize = OFFSET_BITS + CROSS_BITS + 2;

/// A three-plane point against a plane, n · X + d · w, four products each below 2^(9B+17) (d · w
/// included, asserted below): 2^(9B+19) = 2^379 ([`Plane::side_of_tpi`]).
const CLASSIFY_BITS: usize = NORMAL_BITS + X_BITS + 2;

/// `i128` holds every magnitude below 2^127 (and −2^127 itself).
const I128_BITS: usize = i128::BITS as usize - 1;

/// The number of 64-bit limbs of a [`Wide`] width, read from a value of it, so that the checks
/// below follow the widths the code uses.
const fn limbs<const L: usize>(_: &Wide<L>) -> usize {
    L
}

/// [`I256`] holds every magnitude below 2^255: a `Wide<L>` holds every magnitude below
/// 2^(64L − 1), and −2^(64L − 1) itself.
const I256_BITS: usize = 64 * limbs(&I256::ZERO) - 1;

/// [`I320`] holds every magnitude below 2^319.
const I320_BITS: usize = 64 * limbs(&I320::ZERO) - 1;

/// [`I384`], the budgeted width of [`Plane::side_of_tpi`] and [`Tpi::to_grid`], holds every
/// magnitude below 2^383.
const BUDGET_BITS: usize = 64 * BUDGET_LIMBS - 1;

/// [`I512`], the width of their total path, holds every magnitude below 2^511.
const TOTAL_BITS: usize = 64 * TOTAL_LIMBS - 1;

/// Checks, at compile time, the arithmetic of every bit budget in this module, and the figures its
/// documentation states (at B = 40).
const _: () = {
    assert!(GRID_DIFFERENCE_BITS == 41 && NORMAL_BITS == 83);
    assert!(OFFSET_BITS == 125 && SIDE_BITS == 126);
    assert!(CROSS_BITS == 167 && W_BITS == 252);
    assert!(X_BITS == 294 && CLASSIFY_BITS == 379);
    // `i128`: n · p + d, the largest of the plane computations; every product and every partial
    // sum of n and d before it is smaller.
    assert!(SIDE_BITS <= I128_BITS);
    // I256: the cross products of normals, and w.
    assert!(CROSS_BITS <= I256_BITS && W_BITS <= I256_BITS);
    // I320: X.
    assert!(X_BITS <= I320_BITS);
    // I384, `side_of_tpi`: d · w is no larger than n_i · X_i, so the four terms bound the sum.
    assert!(OFFSET_BITS + W_BITS <= NORMAL_BITS + X_BITS);
    assert!(CLASSIFY_BITS <= BUDGET_BITS);
    // I384, `to_grid`: 2 |X_i| is below 2^(X_BITS + 1); 2^41 − 1 is below 2^41, so
    // (2^41 − 1) w is below 2^(41 + W_BITS); and 2^127 w bounds both the saturation test and
    // 2T = 2^bits w for bits ≤ 127.
    assert!(X_BITS < BUDGET_BITS);
    assert!(ROUNDS_INSIDE < 1 << GRID_DIFFERENCE_BITS);
    assert!(GRID_DIFFERENCE_BITS + W_BITS <= BUDGET_BITS);
    assert!(I128_BITS + W_BITS <= BUDGET_BITS);
    // I512, for every value the types hold: |n_i|, |d| ≤ 2^127, |X_i| ≤ 2^319 and |w| ≤ 2^255.
    // The types' minimum values reach these bounds, so each must be strictly below the limit.
    // `side_of_tpi`: four terms, each at most 2^(127 + 319) (d · w is no larger).
    assert!(I256_BITS <= I320_BITS);
    assert!(I128_BITS + I320_BITS + 2 < TOTAL_BITS);
    // `to_grid`: 2 |X_i|, (2^41 − 1) w, and 2^127 w (which bounds 2T).
    assert!(I320_BITS + 1 < TOTAL_BITS);
    assert!(GRID_DIFFERENCE_BITS + I256_BITS < TOTAL_BITS);
    assert!(I128_BITS + I256_BITS < TOTAL_BITS);
};

/// The budget for each plane normal component: |`n_i`| < 2^83 = 2^(2B+3) (chapter 6.2).
const NORMAL_LIMIT: u128 = 1 << NORMAL_BITS;

/// The budget for a plane offset: |d| < 2^125 = 2^(3B+5) (chapter 6.2).
const OFFSET_LIMIT: u128 = 1 << OFFSET_BITS;

/// 2^41 − 1: a coordinate `X_i` / w rounds to a grid point inside the frame exactly when
/// 2 |`X_i`| < (2^41 − 1) |w| (derivation on [`Tpi::to_grid`]). Evaluated at compile time, where an
/// overflow is an error.
const ROUNDS_INSIDE: i64 = 2 * COORD_MAX + 1;

/// 2^126: half of 2^127, the first rounded coordinate [`Tpi::to_grid`] cannot report exactly in
/// `i128`. Evaluated at compile time, where an overflow is an error.
const TWO_POW_126: i128 = 1 << 126;

/// Limbs of [`I384`], the width every wide budget here fits (limit 2^383).
const BUDGET_LIMBS: usize = 6;

/// Limbs of [`I512`] (limit 2^511), the width of the total path, where no step of
/// [`Plane::side_of_tpi`] or [`Tpi::to_grid`] can overflow for any value the types hold.
const TOTAL_LIMBS: usize = 8;

/// A plane: the points p with n · p + d = 0, with integer coefficients.
///
/// Made only by [`Plane::through`], which checks the bit budget at construction: |`n_i`| < 2^83 and
/// |d| < 2^125. These are safe bounds, not maxima: planes through grid points reach at most
/// 4M² < 2^82 and 4M³ < 2^122, with M = 2^40 − 1 (both derivations on [`Plane::through`]).
///
/// The plane is oriented: [`Plane::side`] is positive on the side the normal n points to.
/// Equality is equality of coefficients, not of point sets: the same plane made from other points
/// may have a scaled or opposite normal and compares unequal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Plane {
    /// The normal, (b − a) × (c − a) for the points it was made from.
    n: [i128; 3],
    /// The offset, −n · a.
    d: i128,
}

impl Plane {
    /// The plane through three grid points, oriented so that [`Plane::side`] is positive on the
    /// side (b − a) × (c − a) points to: a, b, c turn counter-clockwise seen from there. For any
    /// grid point p, `Plane::through(a, b, c)?.side(p)` is the 3D orientation of a, b, c, p.
    ///
    /// n = (b − a) × (c − a) and d = −n · a, exactly.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] when a, b and c are collinear (repeated points included), decided
    /// exactly: n = 0. [`NumError::BudgetExceeded`] if a coefficient were over its budget
    /// (|`n_i`| ≥ 2^83 or |d| ≥ 2^125) or an `i128` step did not fit. That cannot happen for grid
    /// points, by the bit budget below; it is checked because every exact computation on planes
    /// relies on the budget, and chapter 6.2 requires the plane type to check it at construction.
    ///
    /// Bit budget (every coordinate |x| ≤ M = 2^40 − 1), every step checked, in `i128` (limit
    /// 2^127):
    /// - each difference `u_i` = `b_i` − `a_i`, `v_i` = `c_i` − `a_i` has |`u_i`| ≤ 2M = 2^41 − 2;
    /// - each product `u_j` · `v_k` is at most (2^41 − 2)^2 < 2^82 in magnitude, so each normal
    ///   component `n_i` = `u_j` · `v_k` − `u_k` · `v_j` has |`n_i`| ≤ 2 (2^41 − 2)^2 < 2^83;
    /// - each product `n_i` · `a_i` is below 2^83 · 2^40 = 2^123, so every partial sum of
    ///   d = −(`n_x` `a_x` + `n_y` `a_y` + `n_z` `a_z`) stays below 3 · 2^123 < 2^125.
    ///
    /// The largest values grid points reach are smaller. Each `n_i` is twice the signed area of the
    /// triangle a, b, c projected onto a coordinate plane, which lies in a square of side 2M, so
    /// |`n_i`| ≤ (2M)^2 = 4M² < 2^82. And d = −det[a; b; c], at most 4M³ < 2^122 in magnitude,
    /// because the largest 3 × 3 determinant with entries in [−1, 1] is 4 (reasoning on
    /// [`Tpi::homogeneous`]). Both are reached (tested).
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b and c are the points and n and d the coefficients of the formulas above"
    )]
    pub const fn through(a: P3, b: P3, c: P3) -> Result<Plane, NumError> {
        match coefficients(a, b, c) {
            Some(([0, 0, 0], _)) => Err(NumError::Degenerate),
            Some((n, d)) if within_budget(&n, d) => Ok(Plane { n, d }),
            _ => Err(NumError::BudgetExceeded),
        }
    }

    /// The normal n; |`n_i`| < 2^83, checked at construction.
    #[must_use]
    pub const fn n(&self) -> [i128; 3] {
        self.n
    }

    /// The offset d; |d| < 2^125, checked at construction.
    #[must_use]
    pub const fn d(&self) -> i128 {
        self.d
    }

    /// Which side of the plane a grid point is on: the sign of n · p + d. `Zero` means exactly on
    /// the plane; `Positive` means on the side the normal points to.
    ///
    /// Bit budget: |`n_i`| < 2^83 and |d| < 2^125, checked when the plane was made, and
    /// |`p_i`| < 2^40, checked when the point was made. So each product `n_i` · `p_i` is below
    /// 2^123 and their sum below 3 · 2^123; with |d| < 2^125 = 4 · 2^123, every partial sum stays
    /// below 7 · 2^123 < 2^126, in `i128`.
    ///
    /// Reach: n · p + d is −det[[a, 1], [b, 1], [c, 1], [p, 1]] for the points a, b, c the plane
    /// was made from. Over coordinates in [−M, M] its largest magnitude lies at a corner of that
    /// box (reasoning on [`Tpi::homogeneous`]), where it is M³ times a 4 × 4 determinant of
    /// entries ±1, at most 16 by Hadamard's inequality. So |n · p + d| ≤ 16M³ < 2^124, which the
    /// frame's corners reach (tested).
    ///
    /// Every step is checked all the same, and a step that does not fit is reported, not answered.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step did not fit `i128`, which no plane and point can
    /// cause: their bounds are checked when they are made, and the arithmetic above is asserted
    /// when the crate compiles (module docs, "No answer is silently wrong").
    pub const fn side(&self, p: P3) -> Result<Sign, NumError> {
        match dot_point(&self.n, p) {
            Some(np) => match np.checked_add(self.d) {
                Some(v) => Ok(Sign::of_i128(v)),
                None => Err(NumError::BudgetExceeded),
            },
            None => Err(NumError::BudgetExceeded),
        }
    }

    /// Which side of the plane the point where three planes meet is on, exactly. `Zero` means the
    /// point lies exactly on the plane; `Positive` means on the side the normal points to.
    ///
    /// With the point's homogeneous coordinates (X, w) from [`Tpi::homogeneous`], the point is
    /// X / w, so n · (X / w) + d = (n · X + d · w) / w, whose sign is the sign of n · X + d · w
    /// times the sign of w. Nothing is divided or rounded.
    ///
    /// Bit budget: |`n_i`| < 2^83 and |`X_i`| < 2^294, so each `n_i` · `X_i` is below 2^377 and the
    /// three together below 3 · 2^377; |d| < 2^125 and |w| < 2^252, so d · w is below 2^377. Every
    /// partial sum stays below 4 · 2^377 = 2^379, in [`I384`] (limit 2^383).
    ///
    /// Reach: n · X + d · w is the 4 × 4 determinant with rows (`n_k`, `d_k`), the three planes
    /// first and this plane last (expanding it along the last row gives n · X + d · w). Over the
    /// box |`n_i`| ≤ 4M², |d| ≤ 4M³ that bounds planes through grid points, its largest magnitude
    /// lies at a corner of the box (reasoning on [`Tpi::homogeneous`]), where it is (4M²)³ (4M³)
    /// times a 4 × 4 determinant of entries ±1, at most 16 by Hadamard's inequality (4^(4/2) = 16).
    /// So |n · X + d · w| ≤ 4096M^9 < 2^372, which planes through corners of the frame's cube reach
    /// (tested).
    ///
    /// Total path, unreachable: should a step in [`I384`] not fit anyway, the same sum is computed
    /// again in [`I512`] (limit 2^511), where nothing can overflow for any value the types hold:
    /// |`n_i`|, |d| ≤ 2^127, |`X_i`| ≤ 2^319 and |w| ≤ 2^255, so each product is at most 2^446 and
    /// every partial sum below 4 · 2^446 = 2^448. So the sign is exact even if the budget above
    /// were wrong.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if the point's homogeneous coordinates could not be recomputed
    /// ([`Tpi::homogeneous`]) or the total path failed too. Neither can happen: [`Tpi::new`]
    /// completed the first for the same planes, and nothing on the total path can overflow; they
    /// could only if the checked wide arithmetic itself were wrong (module docs, "No answer is
    /// silently wrong"), and then this reports it rather than answering.
    pub fn side_of_tpi(&self, t: &Tpi) -> Result<Sign, NumError> {
        let (x, w) = t.homogeneous()?;
        classify::<BUDGET_LIMBS>(&self.n, self.d, &x, &w)
            .or_else(|| classify::<TOTAL_LIMBS>(&self.n, self.d, &x, &w))
            .ok_or(NumError::BudgetExceeded)
    }
}

/// q − p for two coordinates, in `i128`. Checked like every step here, although it always fits:
/// |q − p| < 2^64 for any two `i64` values.
const fn difference(q: Coord, p: Coord) -> Option<i128> {
    (q.get() as i128).checked_sub(p.get() as i128)
}

/// p · q − r · s in `i128`, every step checked: `None` if a step does not fit.
const fn mul_sub(p: i128, q: i128, r: i128, s: i128) -> Option<i128> {
    match (p.checked_mul(q), r.checked_mul(s)) {
        (Some(pq), Some(rs)) => pq.checked_sub(rs),
        _ => None,
    }
}

/// `n_x` `p_x` + `n_y` `p_y` + `n_z` `p_z` for a plane normal and a grid point, in `i128`, every
/// step checked: `None` if a step does not fit. Budgets on [`Plane::through`] and [`Plane::side`].
const fn dot_point(n: &[i128; 3], p: P3) -> Option<i128> {
    let [nx, ny, nz] = *n;
    let (Some(tx), Some(ty), Some(tz)) = (
        nx.checked_mul(p.x.get() as i128),
        ny.checked_mul(p.y.get() as i128),
        nz.checked_mul(p.z.get() as i128),
    ) else {
        return None;
    };
    match tx.checked_add(ty) {
        Some(s) => s.checked_add(tz),
        None => None,
    }
}

/// n = (b − a) × (c − a) and d = −n · a in `i128`, every step checked: `None` if a step does not
/// fit, which the budget on [`Plane::through`] rules out.
#[allow(
    clippy::many_single_char_names,
    reason = "a, b and c are the points and n and d the coefficients of the formulas above"
)]
const fn coefficients(a: P3, b: P3, c: P3) -> Option<([i128; 3], i128)> {
    let (Some(ux), Some(uy), Some(uz), Some(vx), Some(vy), Some(vz)) = (
        difference(b.x, a.x),
        difference(b.y, a.y),
        difference(b.z, a.z),
        difference(c.x, a.x),
        difference(c.y, a.y),
        difference(c.z, a.z),
    ) else {
        return None;
    };
    let (Some(nx), Some(ny), Some(nz)) = (
        mul_sub(uy, vz, uz, vy),
        mul_sub(uz, vx, ux, vz),
        mul_sub(ux, vy, uy, vx),
    ) else {
        return None;
    };
    let n = [nx, ny, nz];
    match dot_point(&n, a) {
        // Checked like every step: only `i128::MIN` has no negation in `i128`.
        Some(na) => match na.checked_neg() {
            Some(d) => Some((n, d)),
            None => None,
        },
        None => None,
    }
}

/// Whether plane coefficients obey the budget of chapter 6.2: |`n_i`| < 2^83 and |d| < 2^125.
const fn within_budget(n: &[i128; 3], d: i128) -> bool {
    let [nx, ny, nz] = *n;
    nx.unsigned_abs() < NORMAL_LIMIT
        && ny.unsigned_abs() < NORMAL_LIMIT
        && nz.unsigned_abs() < NORMAL_LIMIT
        && d.unsigned_abs() < OFFSET_LIMIT
}

/// The point where three planes meet, kept implicit: it is stored as its three planes, never as
/// rounded coordinates.
///
/// Made only by [`Tpi::new`], which checks that the planes meet in exactly one point and that the
/// point's homogeneous coordinates fit their types, so the denominator w of [`Tpi::homogeneous`]
/// is never zero. Equality is equality of the three planes in order, not of the points: the same
/// point made from other planes compares unequal.
///
/// Cost: only the planes are stored, so [`Plane::side_of_tpi`] and [`Tpi::to_grid`] recompute the
/// homogeneous coordinates on every call (three [`I256`] cross products, a dot product and nine
/// [`I320`] products), and [`Tpi::new`] computes them once to check them and keeps nothing.
/// Classifying one point against k planes repeats that work k times. Storing (X, w) in the `Tpi`
/// would save it for 152 more bytes per point; that trade is for chapter 23's cost model to
/// decide, and is not made here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tpi {
    /// The three planes, in the order given.
    planes: [Plane; 3],
}

impl Tpi {
    /// The point where planes `p`, `q` and `r` meet.
    ///
    /// Computes the point's homogeneous coordinates once, every step checked (budget on
    /// [`Tpi::homogeneous`]), so every `Tpi` is known to have them.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] when the normals are linearly dependent, so the planes meet in no
    /// point or in a whole line or plane (two parallel planes, three planes through one line, a
    /// plane given twice). Decided exactly: w = det(`n_p`, `n_q`, `n_r`) = 0.
    /// [`NumError::BudgetExceeded`] if a step did not fit its type, which the budget rules out for
    /// planes made by [`Plane::through`] (module docs, "No answer is silently wrong").
    #[allow(
        clippy::many_single_char_names,
        reason = "p, q and r are the planes, and t, w and x the point, as in the formulas"
    )]
    pub fn new(p: Plane, q: Plane, r: Plane) -> Result<Tpi, NumError> {
        let t = Tpi { planes: [p, q, r] };
        match t.homogeneous_checked() {
            Some((_, w)) if w.is_zero() => Err(NumError::Degenerate),
            Some(_) => Ok(t),
            None => Err(NumError::BudgetExceeded),
        }
    }

    /// The three planes, in the order given to [`Tpi::new`].
    #[must_use]
    pub const fn planes(&self) -> &[Plane; 3] {
        &self.planes
    }

    /// The point's homogeneous coordinates: numerators X = (`X_x`, `X_y`, `X_z`) and denominator w,
    /// with the point at (`X_x` / w, `X_y` / w, `X_z` / w). w is never zero, and may be negative;
    /// swapping two planes negates both X and w.
    ///
    /// For planes (`n_1`, `d_1`), (`n_2`, `d_2`), (`n_3`, `d_3`), by Cramer's rule:
    /// w = `n_1` · (`n_2` × `n_3`) = det(`n_1`, `n_2`, `n_3`) and
    /// X = −(`d_1` (`n_2` × `n_3`) + `d_2` (`n_3` × `n_1`) + `d_3` (`n_1` × `n_2`)).
    /// (Check: `n_1` · X = −`d_1` w, because `n_1` · (`n_3` × `n_1`) = `n_1` · (`n_1` × `n_2`) = 0;
    /// likewise for the other two planes.)
    ///
    /// Bit budget (|`n_i`| < 2^83, |d| < 2^125, both checked when the planes were made):
    /// - each product of two normal components is below 2^166, so each component of a cross
    ///   product is below 2^167, in [`I256`];
    /// - each product `n_1i` · (`n_2` × `n_3`)_i is below 2^83 · 2^167 = 2^250, so every partial
    ///   sum of w stays below 3 · 2^250 < 2^252, in [`I256`] (limit 2^255);
    /// - each product `d_k` · (cross product)_i is below 2^125 · 2^167 = 2^292, so every partial
    ///   sum of `X_i` stays below 3 · 2^292 < 2^294, in [`I320`] (limit 2^319).
    ///
    /// Reach, for planes through grid points: w is the determinant of the normals, and `X_i`, up to
    /// sign, the same determinant with column i replaced by the offsets (Cramer's rule). A
    /// determinant is affine in each entry, so moving one entry to an end of its interval never
    /// lowers the magnitude for some choice of end; over a box of entries the largest magnitude
    /// therefore lies at a corner, where each entry is plus or minus its bound. Taking the bounds
    /// out of the columns leaves a determinant of entries ±1. A 3 × 3 one is at most 4 in
    /// magnitude: Hadamard's inequality bounds it by 3^(3/2) < 6, and it is a multiple of 4
    /// (subtracting the first row from the others leaves two rows of 0 and ±2). With |`n_i`| ≤ 4M²
    /// and |d| ≤ 4M³ ([`Plane::through`]), |w| ≤ 4 (4M²)³ = 256M^6 < 2^248 and
    /// |`X_i`| ≤ 4 (4M²)² (4M³) = 256M^7 < 2^288. Three planes through corners of the frame's cube
    /// reach both (tested).
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step did not fit its type. [`Tpi::new`] completed this
    /// same computation for the same planes, so it cannot happen; were the checked arithmetic
    /// itself wrong, this reports it rather than returning coordinates.
    pub fn homogeneous(&self) -> Result<([I320; 3], I256), NumError> {
        self.homogeneous_checked().ok_or(NumError::BudgetExceeded)
    }

    /// [`Tpi::homogeneous`] with every wide operation checked: `None` only if a budget were wrong.
    #[allow(
        clippy::many_single_char_names,
        reason = "p, q and r are the planes, and w and x the point, as in the formulas"
    )]
    fn homogeneous_checked(&self) -> Option<([I320; 3], I256)> {
        let [p, q, r] = &self.planes;
        let c_qr = cross(&q.n, &r.n)?;
        let c_rp = cross(&r.n, &p.n)?;
        let c_pq = cross(&p.n, &q.n)?;
        let w = dot(&p.n, &c_qr)?;
        let (dp, dq, dr) = (
            I320::from_i128(p.d),
            I320::from_i128(q.d),
            I320::from_i128(r.d),
        );
        let mut x = [I320::ZERO; 3];
        for (xi, ((a, b), c)) in x
            .iter_mut()
            .zip(c_qr.iter().zip(c_rp.iter()).zip(c_pq.iter()))
        {
            let sum = dp
                .checked_mul(&a.resize::<5>()?)?
                .checked_add(&dq.checked_mul(&b.resize::<5>()?)?)?
                .checked_add(&dr.checked_mul(&c.resize::<5>()?)?)?;
            *xi = sum.checked_neg()?;
        }
        Some((x, w))
    }

    /// Commits the point to the grid: each coordinate `X_i` / w rounded to the nearest grid step,
    /// decided exactly in wide integers. A coordinate exactly halfway between two steps rounds to
    /// the even one (round half to even, IEEE 754's default; it is unbiased and symmetric, so
    /// mirroring a model mirrors its rounded points). Chapter 6 bounds the commit at half a step
    /// (6.5) but states a tie rule only for angles (6.1, half to even); the commit follows that
    /// rule. [`Rounded::is_exact`] is true when the point already lies on the grid, so nothing
    /// moved; otherwise each coordinate moved by at most half a step, the commit-rounding rung of
    /// the tolerance ladder, [`COMMIT_PER_AXIS`](crate::ladder::COMMIT_PER_AXIS) (chapter 6.5), and
    /// the point by less than one step, [`COMMIT_DISTANCE_3D`](crate::ladder::COMMIT_DISTANCE_3D).
    ///
    /// Committing is the only place a constructed point is rounded (chapter 6.3). Whether rounding
    /// flipped any predicate is the caller's to re-check.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] when a rounded coordinate lies outside the frame (more than
    /// 2^40 − 1 steps from the origin), for the first such coordinate in the order x, y, z. Three
    /// nearly
    /// dependent planes can meet far beyond `i128`: |`X_i`| ≤ 256M^7 < 2^288 and |w| ≥ 1, so the
    /// point lies less than 2^288 steps away (derivation on [`Tpi::homogeneous`]). So `value` is
    /// that rounded coordinate saturated to `i128`: exact whenever it lies strictly between
    /// `i128::MIN` and `i128::MAX`, while `i128::MAX` stands for 2^127 − 1 or more and `i128::MIN`
    /// for −2^127 or less. The two ends differ because `i128` does: −2^127 fits it, 2^127 does not.
    /// The exact position, however far, is always available from [`Tpi::homogeneous`].
    /// [`NumError::BudgetExceeded`] if the homogeneous coordinates could not be recomputed or the
    /// total path failed too, which cannot happen (module docs, "No answer is silently wrong").
    ///
    /// Method and budget, per coordinate, after making w positive by negating both `X_i` and w if
    /// needed (|`X_i`| < 2^294, 0 < w < 2^252), in [`I384`] (limit 2^383):
    /// - the rounded value is inside the frame exactly when |`X_i`| / w < 2^40 − 1/2, because the
    ///   tie at 2^40 − 1/2 rounds to the even 2^40, which is outside. That is
    ///   2 |`X_i`| < (2^41 − 1) w, with 2 |`X_i`| < 2^295 and (2^41 − 1) w < 2^293;
    /// - inside the frame the quotient q = floor(|`X_i`| / w) is below 2^40. Outside it, the value
    ///   saturates if |`X_i`| ≥ 2^127 w (2^127 w < 2^379); otherwise q is below 2^127;
    /// - restoring long division then finds q one bit at a time, 40 or 127 bits, and rounds by
    ///   comparing twice the remainder with w: above rounds up, equal is the tie (round up only
    ///   when q is odd), below keeps q. Its own budget (every intermediate below 2^379) is on the
    ///   private `divide_half_even`, which verifies its preconditions rather than assuming them.
    ///   The result is at most 2^127, which fits `u128`.
    ///
    /// Total path, unreachable: should a step in [`I384`] not fit anyway, the whole commit is
    /// computed again in [`I512`] (limit 2^511), where nothing can overflow for any value the
    /// types hold: with |`X_i`| ≤ 2^319 and 0 < w ≤ 2^255, 2 |`X_i`| ≤ 2^320, (2^41 − 1) w < 2^296,
    /// 2^127 w ≤ 2^382, and the division stays below 2^382. So the commit is exact even if the
    /// budget above were wrong.
    pub fn to_grid(&self) -> Result<Rounded<P3>, NumError> {
        let (x, w) = self.homogeneous()?;
        commit::<BUDGET_LIMBS>(&x, &w)
            .or_else(|| commit::<TOTAL_LIMBS>(&x, &w))
            .unwrap_or(Err(NumError::BudgetExceeded))
    }
}

/// a × b for two plane normals (|`a_i`|, |`b_i`| < 2^83): each component below 2^167, in [`I256`].
fn cross(a: &[i128; 3], b: &[i128; 3]) -> Option<[I256; 3]> {
    let [ax, ay, az] = *a;
    let [bx, by, bz] = *b;
    Some([
        product_difference(ay, bz, az, by)?,
        product_difference(az, bx, ax, bz)?,
        product_difference(ax, by, ay, bx)?,
    ])
}

/// p · q − r · s for normal components below 2^83: each product below 2^166, the difference below
/// 2^167, in [`I256`].
fn product_difference(p: i128, q: i128, r: i128, s: i128) -> Option<I256> {
    let pq = I256::from_i128(p).checked_mul(&I256::from_i128(q))?;
    let rs = I256::from_i128(r).checked_mul(&I256::from_i128(s))?;
    pq.checked_sub(&rs)
}

/// a · c for a normal a (|`a_i`| < 2^83) and a cross product of normals c (|`c_i`| < 2^167): each
/// product below 2^250, every partial sum below 3 · 2^250 < 2^252, in [`I256`].
fn dot(a: &[i128; 3], c: &[I256; 3]) -> Option<I256> {
    let mut acc = I256::ZERO;
    for (&ai, ci) in a.iter().zip(c.iter()) {
        acc = acc.checked_add(&I256::from_i128(ai).checked_mul(ci)?)?;
    }
    Some(acc)
}

/// The sign of n · X + d · w times the sign of w, in `Wide<L>`: [`I384`] on the budgeted path,
/// [`I512`] on the total path. `None` if a step does not fit. Budgets for both on
/// [`Plane::side_of_tpi`].
fn classify<const L: usize>(n: &[i128; 3], d: i128, x: &[I320; 3], w: &I256) -> Option<Sign> {
    let mut acc = Wide::<L>::from_i128(d).checked_mul(&w.resize::<L>()?)?;
    for (&ni, xi) in n.iter().zip(x.iter()) {
        let term = Wide::<L>::from_i128(ni).checked_mul(&xi.resize::<L>()?)?;
        acc = acc.checked_add(&term)?;
    }
    Some(acc.signum().mul(w.signum()))
}

/// The point X / w committed to the grid, as in [`Tpi::to_grid`] (method and budgets there), in
/// `Wide<L>`: [`I384`] on the budgeted path, [`I512`] on the total path. The outer `Option` is
/// `None` if a step does not fit `Wide<L>`, or if w = 0, which no [`Tpi`] has; the inner `Result`
/// is the commit, or `OutOfRange` for the first coordinate outside the frame.
fn commit<const L: usize>(x: &[I320; 3], w: &I256) -> Option<Result<Rounded<P3>, NumError>> {
    if w.is_zero() {
        return None;
    }
    let w_negative = w.is_negative();
    let w = w.resize::<L>()?;
    let w = if w_negative { w.checked_neg()? } else { w };
    let mut coords = [Coord::ZERO; 3];
    let mut exact = true;
    for (coord, xi) in coords.iter_mut().zip(x.iter()) {
        match round_axis(xi, &w, w_negative)? {
            Ok((c, e)) => {
                *coord = c;
                exact &= e;
            }
            Err(e) => return Some(Err(e)),
        }
    }
    let [x, y, z] = coords;
    Some(Ok(Rounded::new(P3 { x, y, z }, exact)))
}

/// 2^127 in `Wide<L>`, made as 2^126 + 2^126 so that no step leaves `i128`, or `None` if it does
/// not fit `Wide<L>`. A rounded coordinate at or beyond 2^127 · w saturates in `OutOfRange`.
fn two_pow_127<const L: usize>() -> Option<Wide<L>> {
    let half = Wide::<L>::from_i128(TWO_POW_126);
    half.checked_add(&half)
}

/// One coordinate `X_i` / w rounded half to even onto the grid, as in [`Tpi::to_grid`] (method and
/// budgets there). `w` is the denominator's magnitude (positive) and `w_negative` its original
/// sign. The outer `Option` is `None` if a step does not fit `Wide<L>`; the inner `Result` is the
/// coordinate and whether it was exact, or `OutOfRange`.
fn round_axis<const L: usize>(
    x: &I320,
    w: &Wide<L>,
    w_negative: bool,
) -> Option<Result<(Coord, bool), NumError>> {
    let x = x.resize::<L>()?;
    let negative = x.is_negative() != w_negative;
    let magnitude = if x.is_negative() { x.checked_neg()? } else { x };
    let twice = magnitude.checked_add(&magnitude)?;
    let frame = Wide::<L>::from_i64(ROUNDS_INSIDE).checked_mul(w)?;
    let bits = if twice < frame {
        COORD_BITS
    } else if magnitude < two_pow_127::<L>()?.checked_mul(w)? {
        127
    } else {
        let value = if negative { i128::MIN } else { i128::MAX };
        return Some(Err(NumError::OutOfRange { value }));
    };
    let (rounded, exact) = divide_half_even(&magnitude, w, bits)?;
    // rounded ≤ 2^127: its negation always fits `i128`; a positive 2^127 saturates.
    let value = if negative {
        0i128.checked_sub_unsigned(rounded).unwrap_or(i128::MIN)
    } else {
        i128::try_from(rounded).unwrap_or(i128::MAX)
    };
    Some(Coord::from_i128(value).map(|c| (c, exact)))
}

/// num / den rounded to the nearest integer, ties to even, and whether it was exact, in
/// `Wide<L>`, for 0 ≤ num < 2^bits · den, den > 0 and 1 ≤ bits ≤ 127. `None` if a step does not
/// fit `Wide<L>`, or if any of these preconditions does not hold: every one is verified, none
/// assumed.
///
/// Restoring binary long division (Knuth, volume 2, section 4.3.1), finding the quotient
/// q = floor(num / den) one bit at a time from bit bits − 1 down to bit 0, doubling q and adding
/// the new bit at each step. Instead of comparing the remainder ρ with den · 2^k at step k, it
/// compares ρ · 2^(bits−1−k) with the fixed T = den · 2^(bits−1), doubling the scaled remainder
/// between steps: the same comparisons, with one multiplication in all instead of one per step.
/// Before step k, ρ < den · 2^(k+1), so the scaled remainder is below 2T; one subtraction of T
/// brings it below T. At the end the scaled remainder is ρ · 2^(bits−1) with 0 ≤ ρ < den, so
/// comparing 2ρ with den (above rounds up, equal is the tie, which rounds up only when q is odd)
/// is comparing twice the scaled remainder with T.
///
/// How each precondition is verified: the signs of num and den are tested first. bits = 0 fails
/// the checked bits − 1, and from bits = 128 the checked 2^(bits−1) does not fit `i128`. And if
/// num ≥ 2^bits · den = 2T, the first step leaves the scaled remainder at T or more, and every
/// later step keeps it there (or a doubling does not fit `Wide<L>`), so a final scaled remainder
/// below T proves that the quotient fitted in `bits` bits.
///
/// Bit budget: 2^(bits−1) ≤ 2^126 fits `i128`; T = den · 2^(bits−1), and every scaled remainder,
/// doubled, stays below 2T. On the [`I384`] path den < 2^252, so 2T < 2^379 (limit 2^383); on the
/// [`I512`] total path den ≤ 2^255, so 2T ≤ 2^382 (limit 2^511). q < 2^bits ≤ 2^127, so q, every
/// doubling of it and the rounded result (at most 2^127) fit `u128`.
fn divide_half_even<const L: usize>(
    num: &Wide<L>,
    den: &Wide<L>,
    bits: u32,
) -> Option<(u128, bool)> {
    if num.is_negative() || den.is_negative() || den.is_zero() {
        return None;
    }
    // 2^(bits − 1): `None` for bits = 0, and from bits = 128, where 2^127 does not fit `i128`.
    let step = 2i128.checked_pow(bits.checked_sub(1)?)?;
    let top = Wide::<L>::from_i128(step).checked_mul(den)?;
    let mut quotient = 0u128;
    let mut scaled = *num;
    for k in (0..bits).rev() {
        quotient = quotient.checked_add(quotient)?;
        if scaled >= top {
            scaled = scaled.checked_sub(&top)?;
            quotient = quotient.checked_add(1)?;
        }
        if k > 0 {
            scaled = scaled.checked_add(&scaled)?;
        }
    }
    if scaled >= top {
        // num ≥ 2^bits · den: the quotient did not fit in `bits` bits.
        return None;
    }
    let twice = scaled.checked_add(&scaled)?;
    let up = match twice.cmp(&top) {
        Ordering::Greater => true,
        Ordering::Equal => quotient & 1 == 1,
        Ordering::Less => false,
    };
    Some((quotient.checked_add(u128::from(up))?, scaled.is_zero()))
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    //! The private rounding division on its own. No public call can pass it a negative numerator,
    //! a denominator that is not positive, a bit count outside 1 to 127 or a quotient too wide,
    //! so these tests check directly that each such input is refused, as its documentation says,
    //! and that every result equals round half to even computed from `u128` floor and remainder:
    //! exhaustively for small values, and at the largest quotient, 2^127, and the largest
    //! denominators of both widths.

    use core::cmp::Ordering;

    use super::{BUDGET_LIMBS, TOTAL_LIMBS, divide_half_even};
    use crate::wide::Wide;

    /// num / den rounded half to even, and whether it was exact, from `u128` floor and remainder.
    fn oracle(num: u128, den: u128) -> (u128, bool) {
        let (q, r) = (num / den, num % den);
        let up = match (2 * r).cmp(&den) {
            Ordering::Greater => true,
            Ordering::Equal => q % 2 == 1,
            Ordering::Less => false,
        };
        (q + u128::from(up), r == 0)
    }

    fn wide<const L: usize>(v: u128) -> Wide<L> {
        Wide::from_i128(i128::try_from(v).unwrap())
    }

    /// 2^n − 1 in `Wide<L>`, by doubling.
    fn below_pow2<const L: usize>(n: u32) -> Wide<L> {
        let one = Wide::<L>::one();
        let mut v = one;
        for _ in 0..n {
            v = v.checked_add(&v).unwrap();
        }
        v.checked_sub(&one).unwrap()
    }

    /// Every num from 0 to a little past 2^bits · den, for den up to 20 and bits up to 8: the
    /// oracle's answer while the quotient fits in `bits` bits, `None` from 2^bits · den on.
    fn small_values_against_the_oracle<const L: usize>() {
        for bits in 1..=8u32 {
            for den in 1..=20u128 {
                let limit = den << bits;
                for num in 0..limit + 3 {
                    let got = divide_half_even(&wide::<L>(num), &wide::<L>(den), bits);
                    let want = (num < limit).then(|| oracle(num, den));
                    assert_eq!(got, want, "{num} / {den} in {bits} bits, {L} limbs");
                }
            }
        }
    }

    #[test]
    fn small_values_match_the_oracle_in_both_widths() {
        small_values_against_the_oracle::<BUDGET_LIMBS>();
        small_values_against_the_oracle::<TOTAL_LIMBS>();
    }

    #[test]
    fn every_precondition_is_verified() {
        let w = |v: i128| Wide::<BUDGET_LIMBS>::from_i128(v);
        let div = |num: i128, den: i128, bits: u32| divide_half_even(&w(num), &w(den), bits);
        assert_eq!(div(-1, 3, 40), None, "negative numerator");
        assert_eq!(div(-6, -3, 40), None, "negative denominator");
        assert_eq!(div(6, -3, 40), None, "negative denominator only");
        assert_eq!(div(5, 0, 40), None, "zero denominator");
        assert_eq!(div(0, 0, 40), None, "zero over zero");
        assert_eq!(div(1, 3, 0), None, "no bits");
        assert_eq!(div(1, 3, 128), None, "128 bits");
        assert_eq!(div(1, 3, u32::MAX), None, "u32::MAX bits");
        assert_eq!(div(3 << 40, 3, 40), None, "num = 2^bits · den");
        // Just below: 2^40 − 1/3 rounds up to 2^40, which needs no 41st quotient bit.
        assert_eq!(div((3 << 40) - 1, 3, 40), Some((1 << 40, false)));
        assert_eq!(div(0, 7, 1), Some((0, true)), "zero numerator");
        // Far above 2^bits · den, where the scaled remainder grows until it does not fit.
        let huge = Wide::<BUDGET_LIMBS>::max_value();
        let got = divide_half_even(&huge, &w(1), 127);
        assert_eq!(got, None, "the largest numerator");
    }

    #[test]
    fn the_largest_quotient_rounds_to_2_pow_127() {
        let two = Wide::<BUDGET_LIMBS>::from_i128(2);
        let div = |num: Wide<BUDGET_LIMBS>| divide_half_even(&num, &two, 127);
        let one = Wide::<BUDGET_LIMBS>::one();
        let below = below_pow2::<BUDGET_LIMBS>(128);
        // 2^128 − 1 over 2: q = 2^127 − 1 (odd), remainder 1, a tie, so it rounds up to 2^127.
        assert_eq!(div(below), Some((1 << 127, false)));
        // 2^128 − 2 over 2 is 2^127 − 1 exactly.
        let exact = below.checked_sub(&one).unwrap();
        assert_eq!(div(exact), Some(((1 << 127) - 1, true)));
        // 2^128 over 2 needs a 128th quotient bit: refused.
        let over = below.checked_add(&one).unwrap();
        assert_eq!(div(over), None);
    }

    /// q · den + r for an odd den, in `Wide<L>`, rounded with 127 bits: it rounds up exactly when
    /// 2r > den (2r = den cannot happen for odd den), and (q + 1) · den is exact while q + 1 fits
    /// in 127 bits.
    fn largest_denominator<const L: usize>(den: Wide<L>) {
        let one = Wide::<L>::one();
        let den_less_one = den.checked_sub(&one).unwrap();
        // (den − 1) / 2, halving through the limbs from the top.
        let half_down = {
            let limbs = den_less_one.to_le_limbs();
            let mut out = [0u64; L];
            let mut carry = 0u64;
            for (o, &l) in out.iter_mut().zip(limbs.iter()).rev() {
                *o = (l >> 1) | (carry << 63);
                carry = l & 1;
            }
            Wide::<L>::from_le_limbs(out)
        };
        let half_up = half_down.checked_add(&one).unwrap();
        let top = (1u128 << 127) - 1;
        for q in [0, 1, 2, (1 << 40) - 1, top - 1, top] {
            let q_wide = Wide::<L>::from_i128(i128::try_from(q).unwrap());
            let base = q_wide.checked_mul(&den).unwrap();
            for (r, want) in [
                (Wide::<L>::ZERO, (q, true)),
                (one, (q, false)),
                (half_down, (q, false)),
                (half_up, (q + 1, false)),
                (den_less_one, (q + 1, false)),
            ] {
                let got = divide_half_even(&base.checked_add(&r).unwrap(), &den, 127);
                assert_eq!(got, Some(want), "q = {q}, r = {r:?}");
            }
            let got = divide_half_even(&base.checked_add(&den).unwrap(), &den, 127);
            let want = (q < top).then_some((q + 1, true));
            assert_eq!(got, want, "q + 1 = {}", q + 1);
        }
    }

    #[test]
    fn the_largest_denominators_of_both_widths() {
        // 2^252 − 1, just below the I384 path's bound on w, and 2^255 − 1, the largest I256.
        largest_denominator(below_pow2::<BUDGET_LIMBS>(252));
        largest_denominator(below_pow2::<TOTAL_LIMBS>(255));
    }
}
