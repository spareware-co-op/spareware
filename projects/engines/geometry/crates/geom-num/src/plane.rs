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
//! - Every result is exact. A [`Sign`] is decided by integer arithmetic alone; no rounding ever
//!   decides it. Only [`Tpi::to_grid`] rounds: it rounds the exact rational point to the nearest
//!   grid point (ties to even) and says whether it rounded.
//! - Nothing overflows, and nothing could wrap silently if it did. A [`Plane`] can only be made by
//!   [`Plane::through`] from grid points (within 2^40 − 1 steps of the origin), which checks the
//!   plane's budget below at construction (chapter 6.2); a [`Tpi`] can only be made by
//!   [`Tpi::new`], which checks that its homogeneous coordinates fit their types. Every
//!   intermediate fits its type by the derivation written on each function, and every operation,
//!   on `i128` and on wide integers alike, is checked, so none of this depends on a build with
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
//!   26-bit inputs (evidence note 05, section 4). The bounds are safe rather than tight (a
//!   triangle inside the frame cannot make every product maximal at once): planes and three-plane
//!   points built from the corners of the frame's cube, the largest the frame allows, reach 82,
//!   122, 248, 288 and 372 bits for n, d, w, X and n · X + d · w (tested).
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
//! Computer Programming*, volume 2, section 4.3.1).
//!
//! If a bound were wrong, no answer would silently be wrong:
//!
//! - a plane over its budget would be refused by [`Plane::through`], and three planes whose
//!   homogeneous coordinates do not fit their types by [`Tpi::new`], both with
//!   `OutOfRange { value: 0 }`. A real out-of-range report always carries a value outside the
//!   frame, so the value 0, which lies inside every frame, marks a budget failure and cannot be
//!   mistaken for one;
//! - [`Plane::side_of_tpi`] and [`Tpi::to_grid`] work in [`I384`]; should a step there not fit,
//!   they repeat the whole computation in [`I512`], where no step can overflow for any value the
//!   types hold (derivations on each), so the answer is still exact;
//! - [`Plane::side`] and [`Tpi::homogeneous`] cannot fail for any plane, point or [`Tpi`]: `side`
//!   works from bounds checked when its inputs were made, and `homogeneous` repeats a computation
//!   that [`Tpi::new`] already completed for the same planes.
//!
//! The arms that remain exist only because checked arithmetic returns an `Option`; they give a
//! defined answer rather than a panic. Those of [`Plane::side`] ([`Sign::Zero`]) and
//! [`Tpi::homogeneous`] (w = 0, which no real three-plane point has) cannot be reached by any
//! input. The last arms of [`Plane::side_of_tpi`] ([`Sign::Zero`]) and [`Tpi::to_grid`]
//! (`OutOfRange { value: 0 }`) could be reached only if the checked wide arithmetic itself
//! reported an overflow for a result that fits (it is tested against big integers in its own
//! module).

use core::cmp::Ordering;

use crate::error::NumError;
use crate::grid::{COORD_BITS, COORD_MAX, Coord, P3, Rounded};
use crate::sign::Sign;
use crate::wide::{I256, I320, Wide};
// Named in the documentation only; the code reaches them as `Wide<6>` and `Wide<8>`.
#[cfg(doc)]
use crate::wide::{I384, I512};

/// The budget for each plane normal component: |n_i| < 2^83 = 2^(2B+3) (chapter 6.2).
const NORMAL_LIMIT: u128 = 1 << 83;

/// The budget for a plane offset: |d| < 2^125 = 2^(3B+5) (chapter 6.2).
const OFFSET_LIMIT: u128 = 1 << 125;

/// The error for a computation that did not fit its budget. A real out-of-range report always
/// carries a value outside the frame, so the value 0, inside every frame, cannot be mistaken for
/// one (module docs, "If a bound were wrong").
const OVER_BUDGET: NumError = NumError::OutOfRange { value: 0 };

/// 2^41 − 1: a coordinate X_i / w rounds to a grid point inside the frame exactly when
/// 2 |X_i| < (2^41 − 1) |w| (derivation on [`Tpi::to_grid`]). Evaluated at compile time, where an
/// overflow is an error.
const ROUNDS_INSIDE: i64 = 2 * COORD_MAX + 1;

/// Limbs of [`I384`], the width every wide budget here fits (limit 2^383).
const BUDGET_LIMBS: usize = 6;

/// Limbs of [`I512`] (limit 2^511), the fallback width, where no step of [`Plane::side_of_tpi`]
/// or [`Tpi::to_grid`] can overflow for any value the types hold.
const FALLBACK_LIMBS: usize = 8;

/// A plane: the points p with n · p + d = 0, with integer coefficients.
///
/// Made only by [`Plane::through`], which checks the bit budget at construction: |n_i| < 2^83 and
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
    /// Errors: `Degenerate` when a, b and c are collinear (repeated points included), decided
    /// exactly: n = 0. `OutOfRange { value: 0 }` if a coefficient were over its budget
    /// (|n_i| ≥ 2^83 or |d| ≥ 2^125) or an `i128` step did not fit. That cannot happen for grid
    /// points, by the bit budget below; it is checked because every exact computation on planes
    /// relies on the budget, and chapter 6.2 requires the plane type to check it at construction.
    ///
    /// Bit budget (every coordinate |x| ≤ M = 2^40 − 1), every step checked, in `i128` (limit
    /// 2^127):
    /// - each difference u_i = b_i − a_i, v_i = c_i − a_i has |u_i| ≤ 2M = 2^41 − 2;
    /// - each product u_j · v_k is at most (2^41 − 2)^2 < 2^82 in magnitude, so each normal
    ///   component n_i = u_j · v_k − u_k · v_j has |n_i| ≤ 2 (2^41 − 2)^2 < 2^83;
    /// - each product n_i · a_i is below 2^83 · 2^40 = 2^123, so every partial sum of
    ///   d = −(n_x a_x + n_y a_y + n_z a_z) stays below 3 · 2^123 < 2^125.
    ///
    /// The largest values grid points reach are smaller. Each n_i is twice the signed area of the
    /// triangle a, b, c projected onto a coordinate plane, which lies in a square of side 2M, so
    /// |n_i| ≤ (2M)^2 = 4M² < 2^82. And d = −det[a; b; c], at most 4M³ < 2^122 in magnitude,
    /// because the largest 3 × 3 determinant with entries in [−1, 1] is 4. Both are reached
    /// (tested).
    pub const fn through(a: P3, b: P3, c: P3) -> Result<Plane, NumError> {
        match coefficients(a, b, c) {
            Some(([0, 0, 0], _)) => Err(NumError::Degenerate),
            Some((n, d)) if within_budget(&n, d) => Ok(Plane { n, d }),
            _ => Err(OVER_BUDGET),
        }
    }

    /// The normal n; |n_i| < 2^83, checked at construction.
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
    /// Bit budget: |n_i| < 2^83 and |d| < 2^125, checked when the plane was made, and |p_i| < 2^40,
    /// checked when the point was made. So each product n_i · p_i is below 2^123 and their sum
    /// below 3 · 2^123; with |d| < 2^125 = 4 · 2^123, every partial sum stays below
    /// 7 · 2^123 < 2^126, in `i128`. Every step is checked all the same; the `Zero` arm for a step
    /// that does not fit cannot be reached by any plane and point.
    #[must_use]
    pub const fn side(&self, p: P3) -> Sign {
        match dot_point(&self.n, p) {
            Some(np) => match np.checked_add(self.d) {
                Some(v) => Sign::of_i128(v),
                None => Sign::Zero,
            },
            None => Sign::Zero,
        }
    }

    /// Which side of the plane the point where three planes meet is on, exactly. `Zero` means the
    /// point lies exactly on the plane; `Positive` means on the side the normal points to.
    ///
    /// With the point's homogeneous coordinates (X, w) from [`Tpi::homogeneous`], the point is
    /// X / w, so n · (X / w) + d = (n · X + d · w) / w, whose sign is the sign of n · X + d · w
    /// times the sign of w. Nothing is divided or rounded.
    ///
    /// Bit budget: |n_i| < 2^83 and |X_i| < 2^294, so each n_i · X_i is below 2^377 and the three
    /// together below 3 · 2^377; |d| < 2^125 and |w| < 2^252, so d · w is below 2^377. Every
    /// partial sum stays below 4 · 2^377 = 2^379, in [`I384`] (limit 2^383).
    ///
    /// Fallback, unreachable: should a step in [`I384`] not fit anyway, the same sum is computed
    /// again in [`I512`] (limit 2^511), where nothing can overflow for any value the types hold:
    /// |n_i|, |d| ≤ 2^127, |X_i| ≤ 2^319 and |w| ≤ 2^255, so each product is at most 2^446 and
    /// every partial sum below 4 · 2^446 = 2^448. So the sign is exact even if the budget above
    /// were wrong. The final `Zero` arm could be reached only if the checked wide arithmetic
    /// itself were wrong (module docs, "If a bound were wrong").
    #[must_use]
    pub fn side_of_tpi(&self, t: &Tpi) -> Sign {
        // Always `Some`: `Tpi::new` completed this computation for the same planes.
        let Some((x, w)) = t.homogeneous_checked() else {
            return Sign::Zero;
        };
        classify::<BUDGET_LIMBS>(&self.n, self.d, &x, &w)
            .or_else(|| classify::<FALLBACK_LIMBS>(&self.n, self.d, &x, &w))
            .unwrap_or(Sign::Zero)
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

/// n_x p_x + n_y p_y + n_z p_z for a plane normal and a grid point, in `i128`, every step
/// checked: `None` if a step does not fit. Budgets on [`Plane::through`] and [`Plane::side`].
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
    let (Some(nx), Some(ny), Some(nz)) =
        (mul_sub(uy, vz, uz, vy), mul_sub(uz, vx, ux, vz), mul_sub(ux, vy, uy, vx))
    else {
        return None;
    };
    let n = [nx, ny, nz];
    match dot_point(&n, a) {
        // The guard makes the negation exact: only `i128::MIN` has no negation in `i128`.
        Some(na) if na != i128::MIN => Some((n, -na)),
        _ => None,
    }
}

/// Whether plane coefficients obey the budget of chapter 6.2: |n_i| < 2^83 and |d| < 2^125.
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
    /// Errors: `Degenerate` when the normals are linearly dependent, so the planes meet in no point
    /// or in a whole line or plane (two parallel planes, three planes through one line, a plane
    /// given twice). Decided exactly: w = det(n_p, n_q, n_r) = 0. `OutOfRange { value: 0 }` if a
    /// step did not fit its type, which the budget rules out for planes made by
    /// [`Plane::through`] (module docs, "If a bound were wrong").
    pub fn new(p: Plane, q: Plane, r: Plane) -> Result<Tpi, NumError> {
        let t = Tpi { planes: [p, q, r] };
        match t.homogeneous_checked() {
            Some((_, w)) if w.is_zero() => Err(NumError::Degenerate),
            Some(_) => Ok(t),
            None => Err(OVER_BUDGET),
        }
    }

    /// The three planes, in the order given to [`Tpi::new`].
    #[must_use]
    pub const fn planes(&self) -> &[Plane; 3] {
        &self.planes
    }

    /// The point's homogeneous coordinates: numerators X = (X_x, X_y, X_z) and denominator w, with
    /// the point at (X_x / w, X_y / w, X_z / w). w is never zero, and may be negative; swapping two
    /// planes negates both X and w.
    ///
    /// For planes (n_1, d_1), (n_2, d_2), (n_3, d_3), by Cramer's rule:
    /// w = n_1 · (n_2 × n_3) = det(n_1, n_2, n_3) and
    /// X = −(d_1 (n_2 × n_3) + d_2 (n_3 × n_1) + d_3 (n_1 × n_2)).
    /// (Check: n_1 · X = −d_1 w, because n_1 · (n_3 × n_1) = n_1 · (n_1 × n_2) = 0; likewise for
    /// the other two planes.)
    ///
    /// Bit budget (|n_i| < 2^83, |d| < 2^125, both checked when the planes were made):
    /// - each product of two normal components is below 2^166, so each component of a cross
    ///   product is below 2^167, in [`I256`];
    /// - each product n_1i · (n_2 × n_3)_i is below 2^83 · 2^167 = 2^250, so every partial sum
    ///   of w stays below 3 · 2^250 < 2^252, in [`I256`] (limit 2^255);
    /// - each product d_k · (cross product)_i is below 2^125 · 2^167 = 2^292, so every partial
    ///   sum of X_i stays below 3 · 2^292 < 2^294, in [`I320`] (limit 2^319).
    ///
    /// [`Tpi::new`] completed this same computation for the same planes, so it always succeeds:
    /// the arm for a step that does not fit, which would return w = 0 (no real three-plane point
    /// has it), cannot be reached.
    #[must_use]
    pub fn homogeneous(&self) -> ([I320; 3], I256) {
        self.homogeneous_checked().unwrap_or(([I320::ZERO; 3], I256::ZERO))
    }

    /// [`Tpi::homogeneous`] with every wide operation checked: `None` only if a budget were wrong.
    fn homogeneous_checked(&self) -> Option<([I320; 3], I256)> {
        let [p, q, r] = &self.planes;
        let c_qr = cross(&q.n, &r.n)?;
        let c_rp = cross(&r.n, &p.n)?;
        let c_pq = cross(&p.n, &q.n)?;
        let w = dot(&p.n, &c_qr)?;
        let (dp, dq, dr) = (I320::from_i128(p.d), I320::from_i128(q.d), I320::from_i128(r.d));
        let mut x = [I320::ZERO; 3];
        for (xi, ((a, b), c)) in x.iter_mut().zip(c_qr.iter().zip(c_rp.iter()).zip(c_pq.iter())) {
            let sum = dp
                .checked_mul(&a.resize::<5>()?)?
                .checked_add(&dq.checked_mul(&b.resize::<5>()?)?)?
                .checked_add(&dr.checked_mul(&c.resize::<5>()?)?)?;
            *xi = sum.checked_neg()?;
        }
        Some((x, w))
    }

    /// Commits the point to the grid: each coordinate X_i / w rounded to the nearest grid step,
    /// decided exactly in wide integers. A coordinate exactly halfway between two steps rounds to
    /// the even one (round half to even, IEEE 754's default; it is unbiased and symmetric, so
    /// mirroring a model mirrors its rounded points). `exact` is true when the point already lies
    /// on the grid, so nothing moved; otherwise each coordinate moved by at most half a step, the
    /// commit-rounding bound of the tolerance ladder (chapter 6.5).
    ///
    /// Committing is the only place a constructed point is rounded (chapter 6.3). Whether rounding
    /// flipped any predicate is the caller's to re-check.
    ///
    /// Errors: `OutOfRange` when a rounded coordinate lies outside the frame (more than 2^40 − 1
    /// steps from the origin), for the first such coordinate in the order x, y, z. Three nearly
    /// dependent planes can meet up to about 2^294 steps away, beyond `i128`, so its `value` is
    /// that rounded coordinate saturated to `i128`: exact whenever it lies strictly between
    /// `i128::MIN` and `i128::MAX`, while `i128::MAX` stands for 2^127 − 1 or more and `i128::MIN`
    /// for −2^127 or less. The two ends differ because `i128` does: −2^127 fits it, 2^127 does not.
    /// The exact position, however far, is always available from [`Tpi::homogeneous`]. The value
    /// is never 0 except for the unreachable failure described in the module docs ("If a bound
    /// were wrong").
    ///
    /// Method and budget, per coordinate, after making w positive by negating both X_i and w if
    /// needed (|X_i| < 2^294, 0 < w < 2^252), in [`I384`] (limit 2^383):
    /// - the rounded value is inside the frame exactly when |X_i| / w < 2^40 − 1/2, because the tie
    ///   at 2^40 − 1/2 rounds to the even 2^40, which is outside. That is 2 |X_i| < (2^41 − 1) w,
    ///   with 2 |X_i| < 2^295 and (2^41 − 1) w < 2^293;
    /// - inside the frame the quotient q = floor(|X_i| / w) is below 2^40. Outside it, the value
    ///   saturates if |X_i| ≥ 2^127 w (2^127 w < 2^379); otherwise q is below 2^127;
    /// - restoring long division then finds q one bit at a time, 40 or 127 bits, and rounds by
    ///   comparing twice the remainder with w: above rounds up, equal is the tie (round up only
    ///   when q is odd), below keeps q. Its own budget (every intermediate below 2^379) is on the
    ///   private `divide_half_even`. The result is at most 2^127, which fits `u128`.
    ///
    /// Fallback, unreachable: should a step in [`I384`] not fit anyway, the whole commit is
    /// computed again in [`I512`] (limit 2^511), where nothing can overflow for any value the
    /// types hold: with |X_i| ≤ 2^319 and 0 < w ≤ 2^255, 2 |X_i| ≤ 2^320, (2^41 − 1) w < 2^296,
    /// 2^127 w ≤ 2^382, and the division stays below 2^382. So the commit is exact even if the
    /// budget above were wrong.
    pub fn to_grid(&self) -> Result<Rounded<P3>, NumError> {
        // Always `Some`: `Tpi::new` completed this computation for the same planes.
        let Some((x, w)) = self.homogeneous_checked() else {
            return Err(OVER_BUDGET);
        };
        commit::<BUDGET_LIMBS>(&x, &w)
            .or_else(|| commit::<FALLBACK_LIMBS>(&x, &w))
            .unwrap_or(Err(OVER_BUDGET))
    }
}

/// a × b for two plane normals (|a_i|, |b_i| < 2^83): each component below 2^167, in [`I256`].
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

/// a · c for a normal a (|a_i| < 2^83) and a cross product of normals c (|c_i| < 2^167): each
/// product below 2^250, every partial sum below 3 · 2^250 < 2^252, in [`I256`].
fn dot(a: &[i128; 3], c: &[I256; 3]) -> Option<I256> {
    let mut acc = I256::ZERO;
    for (&ai, ci) in a.iter().zip(c.iter()) {
        acc = acc.checked_add(&I256::from_i128(ai).checked_mul(ci)?)?;
    }
    Some(acc)
}

/// The sign of n · X + d · w times the sign of w, in `Wide<L>`: [`I384`] on the budgeted path,
/// [`I512`] as the fallback. `None` if a step does not fit. Budgets for both on
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
/// `Wide<L>`: [`I384`] on the budgeted path, [`I512`] as the fallback. The outer `Option` is
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
    Some(Ok(Rounded { value: P3 { x, y, z }, exact }))
}

/// 2^127 in `Wide<L>`, made as 2^126 + 2^126 so that no step leaves `i128`, or `None` if it does
/// not fit `Wide<L>`. A rounded coordinate at or beyond 2^127 · w saturates in `OutOfRange`.
fn two_pow_127<const L: usize>() -> Option<Wide<L>> {
    let half = Wide::<L>::from_i128(1 << 126);
    half.checked_add(&half)
}

/// One coordinate X_i / w rounded half to even onto the grid, as in [`Tpi::to_grid`] (method and
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
/// `Wide<L>`, for 0 ≤ num < 2^bits · den, den > 0 and 1 ≤ bits ≤ 127 (`bits` is clamped to that
/// range). `None` if a step does not fit `Wide<L>`, or if num is not below 2^bits · den.
///
/// Restoring binary long division (Knuth, volume 2, section 4.3.1), finding the quotient
/// q = floor(num / den) one bit at a time from bit bits − 1 down to bit 0. Instead of comparing
/// the remainder ρ with den · 2^k at step k, it compares ρ · 2^(bits−1−k) with the fixed
/// T = den · 2^(bits−1), doubling the scaled remainder between steps: the same comparisons, with
/// one multiplication in all instead of one per step. Before step k, ρ < den · 2^(k+1), so the
/// scaled remainder is below 2T; one subtraction of T brings it below T. At the end the scaled
/// remainder is ρ · 2^(bits−1) with 0 ≤ ρ < den, so comparing 2ρ with den (above rounds up, equal
/// is the tie, which rounds up only when q is odd) is comparing twice the scaled remainder with T.
///
/// The precondition is verified, not assumed: if num ≥ 2^bits · den = 2T, the first step leaves
/// the scaled remainder at T or more, and every later step keeps it there, so a final scaled
/// remainder below T proves that the quotient fitted in `bits` bits.
///
/// Bit budget: 2^(bits−1) ≤ 2^126 fits `i128`; T = den · 2^(bits−1), and every scaled remainder,
/// doubled, stays below 2T. On the [`I384`] path den < 2^252, so 2T < 2^379 (limit 2^383); on the
/// [`I512`] fallback den ≤ 2^255, so 2T ≤ 2^382 (limit 2^511). q < 2^127, so the rounded result
/// is at most 2^127, in `u128`.
fn divide_half_even<const L: usize>(
    num: &Wide<L>,
    den: &Wide<L>,
    bits: u32,
) -> Option<(u128, bool)> {
    let bits = bits.clamp(1, 127);
    // bits − 1 ≤ 126 after the clamp: the shift neither overflows nor reaches the sign bit.
    let top = Wide::<L>::from_i128(1i128 << (bits - 1)).checked_mul(den)?;
    let mut quotient = 0u128;
    let mut scaled = *num;
    for k in (0..bits).rev() {
        if scaled >= top {
            scaled = scaled.checked_sub(&top)?;
            quotient |= 1u128 << k;
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
