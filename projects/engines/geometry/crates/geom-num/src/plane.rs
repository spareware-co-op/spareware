//! Plane-based geometry, exact: integer planes, the implicit point where three planes meet, which
//! side of a plane such a point lies on, and committing it to the grid.
//!
//! A flat-faced solid is stored as its planes, and every vertex is the meeting of three of them
//! (chapter 6.3). A plane is an integer equation n · p + d = 0 made from three grid points. A
//! vertex is a [`Tpi`] (three-plane intersection) and is never rounded inside an operation: its exact
//! position is X / w in homogeneous integer coordinates, and every question about it is answered
//! exactly from that definition. Booleans make new solids out of the same input planes, so the
//! numbers never grow however many operations run: cutting a wall 300 times never drifts.
//!
//! Guarantees:
//!
//! - Every result is exact. A [`Sign`] is decided by integer arithmetic alone; no rounding ever
//!   decides it. Only [`Tpi::to_grid`] rounds: it rounds the exact rational point to the nearest
//!   grid point (ties to even) and says whether it rounded.
//! - Nothing overflows. A [`Plane`] can only be made by [`Plane::through`] from grid points (within
//!   2^40 − 1 steps of the origin), so its coefficients obey the budget below, and every
//!   intermediate fits its type by the derivation written on each function. In summary, with
//!   B = 40 and every coordinate below 2^B in magnitude:
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
//!   At B = 26 the same formulas give EMBER's published 256-bit bound. The bounds are safe rather
//!   than tight (a triangle inside the frame cannot make every product maximal at once): planes
//!   and three-plane points built from the corners of the frame's cube, the largest the frame
//!   allows, reach 82, 122, 248, 288 and 372 bits for n, d, w, X and n · X + d · w (tested).
//! - Deterministic: integers only, no floating point, no platform maths, no allocation.
//!
//! Methods followed: plane-based geometry with vertices as three-plane intersections in homogeneous
//! integer coordinates, as in Campen and Kobbelt 2010
//! ([doi:10.1111/j.1467-8659.2009.01609.x](https://doi.org/10.1111/j.1467-8659.2009.01609.x)),
//! Nehring-Wirxel, Trettner and Kobbelt 2021
//! ([doi:10.1016/j.cad.2021.103015](https://doi.org/10.1016/j.cad.2021.103015)) and EMBER 2022
//! (Trettner, Nehring-Wirxel and Kobbelt,
//! [doi:10.1145/3528223.3530181](https://doi.org/10.1145/3528223.3530181)). Classifying a
//! homogeneous point by multiplying by the sign of w, so that nothing is divided, follows Attene
//! 2020's indirect predicates ([doi:10.1016/j.cad.2020.102856](https://doi.org/10.1016/j.cad.2020.102856); method
//! only, re-derived here). The rounding division is restoring binary long division (Knuth, *The Art
//! of Computer Programming*, volume 2, section 4.3.1).
//!
//! If a bound were wrong: the wide arithmetic is still checked at every step. Each wide computation
//! runs in a private function that returns `None` when a checked operation does not fit. The
//! derivations prove that this never happens for any value these types can hold, so the public
//! functions never see it; they still give a defined answer rather than a panic: [`Tpi::new`] and
//! [`Tpi::to_grid`] return [`NumError::Degenerate`], [`Tpi::homogeneous`] returns w = 0 (which no
//! real three-plane point has, so it cannot be mistaken for one) and [`Plane::side_of_tpi`] returns
//! [`Sign::Zero`].

use core::cmp::Ordering;

use crate::error::NumError;
use crate::grid::{COORD_BITS, COORD_MAX, Coord, P3, Rounded};
use crate::sign::Sign;
use crate::wide::{I256, I320, I384};

/// A plane: the points p with n · p + d = 0, with integer coefficients.
///
/// Made only by [`Plane::through`], so its coefficients always obey the bit budget:
/// |n_i| < 2^83 and |d| < 2^125 (the exact bounds are on [`Plane::through`]).
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
    /// exactly: n = 0.
    ///
    /// Bit budget (every coordinate |x| ≤ 2^40 − 1):
    /// - each difference u_i = b_i − a_i, v_i = c_i − a_i has |u_i| ≤ 2^41 − 2, which fits `i64`;
    /// - each product u_j · v_k is at most (2^41 − 2)^2 < 2^82 in magnitude, so each normal
    ///   component n_i = u_j · v_k − u_k · v_j has |n_i| ≤ 2 (2^41 − 2)^2 < 2^83, in `i128`;
    /// - each product n_i · a_i is below 2^83 · 2^40 = 2^123, so every partial sum of
    ///   d = −(n_x a_x + n_y a_y + n_z a_z) stays below 3 · 2^123 < 2^125, in `i128` (limit 2^127).
    pub const fn through(a: P3, b: P3, c: P3) -> Result<Plane, NumError> {
        let (ax, ay, az) = (a.x.get(), a.y.get(), a.z.get());
        let ux = (b.x.get() - ax) as i128;
        let uy = (b.y.get() - ay) as i128;
        let uz = (b.z.get() - az) as i128;
        let vx = (c.x.get() - ax) as i128;
        let vy = (c.y.get() - ay) as i128;
        let vz = (c.z.get() - az) as i128;
        let n = [uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx];
        let [nx, ny, nz] = n;
        if nx == 0 && ny == 0 && nz == 0 {
            return Err(NumError::Degenerate);
        }
        let d = -(nx * ax as i128 + ny * ay as i128 + nz * az as i128);
        Ok(Plane { n, d })
    }

    /// The normal n; |n_i| < 2^83.
    #[must_use]
    pub const fn n(&self) -> [i128; 3] {
        self.n
    }

    /// The offset d; |d| < 2^125.
    #[must_use]
    pub const fn d(&self) -> i128 {
        self.d
    }

    /// Which side of the plane a grid point is on: the sign of n · p + d. `Zero` means exactly on
    /// the plane; `Positive` means on the side the normal points to.
    ///
    /// Bit budget: |n_i| < 2^83 and |p_i| < 2^40, so each product n_i · p_i is below 2^123 and
    /// their sum below 3 · 2^123; with |d| < 2^125 = 4 · 2^123, every partial sum stays below
    /// 7 · 2^123 < 2^126, in `i128`.
    #[must_use]
    pub const fn side(&self, p: P3) -> Sign {
        let [nx, ny, nz] = self.n;
        let v = nx * p.x.get() as i128 + ny * p.y.get() as i128 + nz * p.z.get() as i128 + self.d;
        Sign::of_i128(v)
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
    #[must_use]
    pub fn side_of_tpi(&self, t: &Tpi) -> Sign {
        // `None` is impossible by the budget above (module docs, "If a bound were wrong").
        self.side_of_tpi_checked(t).unwrap_or(Sign::Zero)
    }

    /// [`Plane::side_of_tpi`] with every wide operation checked: `None` only if a budget were wrong.
    fn side_of_tpi_checked(&self, t: &Tpi) -> Option<Sign> {
        let (x, w) = t.homogeneous_checked()?;
        let mut acc = I384::from_i128(self.d).checked_mul(&w.resize::<6>()?)?;
        for (&ni, xi) in self.n.iter().zip(x.iter()) {
            let term = I384::from_i128(ni).checked_mul(&xi.resize::<6>()?)?;
            acc = acc.checked_add(&term)?;
        }
        Some(acc.signum().mul(w.signum()))
    }
}

/// The point where three planes meet, kept implicit: it is stored as its three planes, never as
/// rounded coordinates.
///
/// Made only by [`Tpi::new`], which checks that the planes meet in exactly one point, so the
/// denominator w of [`Tpi::homogeneous`] is never zero. Equality is equality of the three planes in
/// order, not of the points: the same point made from other planes compares unequal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tpi {
    /// The three planes, in the order given.
    planes: [Plane; 3],
}

impl Tpi {
    /// The point where planes `p`, `q` and `r` meet.
    ///
    /// Errors: `Degenerate` when the normals are linearly dependent, so the planes meet in no point
    /// or in a whole line or plane (two parallel planes, three planes through one line, a plane
    /// given twice). Decided exactly: w = det(n_p, n_q, n_r) = 0. The budget for w is on
    /// [`Tpi::homogeneous`].
    pub fn new(p: Plane, q: Plane, r: Plane) -> Result<Tpi, NumError> {
        // `None` is impossible by the budget; it would be reported as degenerate (module docs).
        match det(&p.n, &q.n, &r.n) {
            Some(w) if !w.is_zero() => Ok(Tpi { planes: [p, q, r] }),
            _ => Err(NumError::Degenerate),
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
    /// Bit budget (|n_i| < 2^83, |d| < 2^125):
    /// - each product of two normal components is below 2^166, so each component of a cross
    ///   product is below 2^167, in [`I256`];
    /// - each product n_1i · (n_2 × n_3)_i is below 2^83 · 2^167 = 2^250, so every partial sum of
    ///   w stays below 3 · 2^250 < 2^252, in [`I256`] (limit 2^255);
    /// - each product d_k · (cross product)_i is below 2^125 · 2^167 = 2^292, so every partial sum
    ///   of X_i stays below 3 · 2^292 < 2^294, in [`I320`] (limit 2^319).
    #[must_use]
    pub fn homogeneous(&self) -> ([I320; 3], I256) {
        // `None` is impossible by the budget above; w = 0 would mark it (module docs).
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
    /// steps from the origin), for the first such coordinate in the order x, y, z. Its `value` is
    /// that rounded coordinate, saturated to `i128::MIN` or `i128::MAX` when it lies beyond the
    /// `i128` range (three nearly dependent planes can meet up to about 2^294 steps away).
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
    ///   when q is odd), below keeps q. Its own budget (every intermediate below 2^380) is on the
    ///   private `divide_half_even`. The result is at most 2^127, which fits `u128`.
    pub fn to_grid(&self) -> Result<Rounded<P3>, NumError> {
        // `None` is impossible by the budget above; it would be reported as degenerate (module
        // docs).
        self.to_grid_checked().unwrap_or(Err(NumError::Degenerate))
    }

    /// [`Tpi::to_grid`] with every wide operation checked: `None` only if a budget were wrong.
    fn to_grid_checked(&self) -> Option<Result<Rounded<P3>, NumError>> {
        let (x, w) = self.homogeneous_checked()?;
        if w.is_zero() {
            return Some(Err(NumError::Degenerate));
        }
        let w_negative = w.is_negative();
        let w = w.resize::<6>()?;
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

/// det(a, b, c) = a · (b × c) for three plane normals: below 2^252, in [`I256`] (derivation on
/// [`Tpi::homogeneous`]).
fn det(a: &[i128; 3], b: &[i128; 3], c: &[i128; 3]) -> Option<I256> {
    dot(a, &cross(b, c)?)
}

/// 2^127 as an [`I384`]: a rounded coordinate at or beyond 2^127 · w saturates in `OutOfRange`.
const TWO_POW_127: I384 = I384::from_le_limbs([0, 1 << 63, 0, 0, 0, 0]);

/// One coordinate X_i / w rounded half to even onto the grid, as in [`Tpi::to_grid`] (method and
/// budget there). `w` is the denominator's magnitude (positive, below 2^252) and `w_negative` its
/// original sign. The outer `Option` is `None` only if the budget were wrong; the inner `Result`
/// is the coordinate and whether it was exact, or `OutOfRange`.
fn round_axis(x: &I320, w: &I384, w_negative: bool) -> Option<Result<(Coord, bool), NumError>> {
    let x = x.resize::<6>()?;
    let negative = x.is_negative() != w_negative;
    let magnitude = if x.is_negative() { x.checked_neg()? } else { x };
    let twice = magnitude.checked_add(&magnitude)?;
    let frame = I384::from_i64(2 * COORD_MAX + 1).checked_mul(w)?;
    let bits = if twice < frame {
        COORD_BITS
    } else if magnitude < TWO_POW_127.checked_mul(w)? {
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

/// num / den rounded to the nearest integer, ties to even, and whether it was exact, for
/// 0 ≤ num < 2^bits · den, 0 < den < 2^252 and 1 ≤ bits ≤ 127 (`bits` is clamped to that range).
///
/// Restoring binary long division (Knuth, volume 2, section 4.3.1), finding the quotient
/// q = floor(num / den) one bit at a time from bit bits − 1 down to bit 0. Instead of comparing the
/// remainder ρ with den · 2^k at step k, it compares ρ · 2^(bits−1−k) with the fixed
/// T = den · 2^(bits−1), doubling the scaled remainder between steps: the same comparisons, with
/// one multiplication in all instead of one per step. Before step k, ρ < den · 2^(k+1), so the
/// scaled remainder is below 2T; one subtraction of T brings it below T. At the end the scaled
/// remainder is ρ · 2^(bits−1) with 0 ≤ ρ < den, so comparing 2ρ with den (above rounds up, equal
/// is the tie, which rounds up only when q is odd) is comparing twice the scaled remainder with T.
///
/// Bit budget: 2^(bits−1) ≤ 2^126 fits `i128`; T < 2^252 · 2^126 = 2^378 and every scaled
/// remainder, doubled, stays below 2T < 2^379, in [`I384`] (limit 2^383); q < 2^127 and the
/// rounded result is at most 2^127, in `u128`.
fn divide_half_even(num: &I384, den: &I384, bits: u32) -> Option<(u128, bool)> {
    let bits = bits.clamp(1, 127);
    let top = I384::from_i128(1i128 << (bits - 1)).checked_mul(den)?;
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
    let twice = scaled.checked_add(&scaled)?;
    let up = match twice.cmp(&top) {
        Ordering::Greater => true,
        Ordering::Equal => quotient & 1 == 1,
        Ordering::Less => false,
    };
    Some((quotient + u128::from(up), scaled.is_zero()))
}
