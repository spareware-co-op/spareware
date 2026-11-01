//! The arrangement predicates: what EMBER's five steps ask beyond which side of a plane a point
//! lies on (11.3): where points fall along a line, how lines turn about a vertex within a face, and
//! how faces turn about an edge. Each reduces to the classify test or to a sign of normals.
//!
//! What it does: a [`Line`] is where two planes meet. [`along`] orders two points of a line, each
//! the meeting of the line with a third plane. [`same_point`] says whether two three-plane points
//! are one point: 11.3's test, that one lies on all three planes of the other. Within a plane, a
//! [`Ray`] runs from a vertex along the line where that plane meets another, and [`turn`] and
//! [`around`] order rays. About a line, a [`Flap`] is a half-plane bounded by the line, and
//! [`turn_about`] and [`about`] order flaps.
//!
//! Each reduction, with n the normals (F a face's plane; P, Q a line's; C, D, G, H, R, S others)
//! and v = `n_P` × `n_Q`:
//!
//! - **Along a line.** On x(t) = `x_0` + t v, a plane C reads C(x(t)) = C(`x_0`) + t (`n_C` · v), so
//!   C at the point `t_D` where D meets the line is (`n_C` · v)(`t_D` − `t_C`). Hence
//!   sign(`t_D` − `t_C`) = sign(C at that point) · sign(`n_C` · v), and `n_C` · v is
//!   det(`n_P`, `n_Q`, `n_C`).
//! - **Rays in a plane.** For rays along `n_F` × `n_G` and `n_F` × `n_H`,
//!   ((`n_F` × `n_G`) × (`n_F` × `n_H`)) · `n_F` = |`n_F`|² det(`n_F`, `n_G`, `n_H`), so a turn is the sign
//!   of that determinant.
//! - **Flaps about a line.** A flap on plane R runs along `n_R` × v. Then
//!   ((`n_R` × v) × (`n_S` × v)) · v = |v|² det(`n_R`, `n_S`, v), and since `n_R` × `n_S` is parallel to
//!   v, that determinant's sign is sign((`n_R` × `n_S`)_k) · sign(`v_k`) for any k with `v_k` ≠ 0.
//!
//! Guarantees:
//!
//! - **Exact, and within the 384-bit budget** (6.2, 11.3). [`along`] and [`same_point`] are
//!   `Plane::side_of_tpi`. A determinant of normals is a three-plane point's weight, 252 bits. A
//!   component of a cross product of normals is below 2 · 2^166 = 2^167, in `I256`. Rays and flaps
//!   compare two such signs, or the signs of two normals' components, and multiply nothing wider.
//! - **Each order is one order.** [`around`] and [`about`] are a total order on the rays or flaps
//!   as angles from `first`, turning positively: `first` itself comes first, then the strict left,
//!   the opposite ray, and the strict right, each side by [`turn`] or [`turn_about`].
//! - **Refused, never guessed.** A plane parallel to a line where a point on it is asked for, and
//!   two parallel planes as a line, are [`NumError::Degenerate`].
//!
//! Method: chapter 11.3's "The engine also orders points along an edge, edges around a vertex
//! within a face, and faces around an edge"
//! (`projects/engines/geometry/architecture/11-booleans.md`), on EMBER (Trettner, Nehring-Wirxel
//! and Kobbelt 2022, <https://doi.org/10.1145/3528223.3530181>); the identities are Binet-Cauchy's
//! and the vector triple product's, re-derived above.
//!
//! Cost: constant and no allocation. [`along`] is two three-plane determinants and one classify;
//! the others are at most three determinants or seven cross products of normals.

use core::cmp::Ordering;

use geom_num::plane::{Plane, Tpi, normals_sign};
use geom_num::wide::I256;
use geom_num::{NumError, Sign};

/// The sign of a product.
const fn times(a: Sign, b: Sign) -> Sign {
    match (a, b) {
        (Sign::Zero, _) | (_, Sign::Zero) => Sign::Zero,
        (Sign::Positive, Sign::Positive) | (Sign::Negative, Sign::Negative) => Sign::Positive,
        _ => Sign::Negative,
    }
}

/// `s` turned over when `reversed`.
const fn turned(s: Sign, reversed: bool) -> Sign {
    match (s, reversed) {
        (Sign::Positive, true) => Sign::Negative,
        (Sign::Negative, true) => Sign::Positive,
        _ => s,
    }
}

/// The sign of det(`n_a`, `n_b`, `n_c`), exactly: zero when the normals are linearly dependent.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`], which planes within budget cannot reach.
pub fn normals_det(a: &Plane, b: &Plane, c: &Plane) -> Result<Sign, NumError> {
    normals_sign(a, b, c)
}

/// `a` × `b` in `i128`, if every step fits.
fn cross_in_words(a: [i128; 3], b: [i128; 3]) -> Option<[i128; 3]> {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    let minor =
        |s: i128, t: i128, u: i128, v: i128| s.checked_mul(t)?.checked_sub(u.checked_mul(v)?);
    Some([
        minor(ay, bz, az, by)?,
        minor(az, bx, ax, bz)?,
        minor(ax, by, ay, bx)?,
    ])
}

/// A plane's coefficients in `i64`, if they fit.
pub(crate) fn plane_words(p: &Plane) -> Option<([i64; 3], i64)> {
    let [x, y, z] = p.n();
    Some((
        [
            i64::try_from(x).ok()?,
            i64::try_from(y).ok()?,
            i64::try_from(z).ok()?,
        ],
        i64::try_from(p.d()).ok()?,
    ))
}

/// `a` · `b` for `a` and `b` in `i64`, in `i128`: |a b| ≤ 2^126, so the wrapping product is exact,
/// and is one multiplication where a checked one is a call; only the sums are checked.
pub(crate) fn wide_product(a: i64, b: i64) -> i128 {
    i128::from(a).wrapping_mul(i128::from(b))
}

/// `a` × `b` for `a` and `b` in `i64`, in `i128` and then `i64`, if that fits.
fn cross_of_words(a: [i64; 3], b: [i64; 3]) -> Option<[i64; 3]> {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    let minor = |s: i64, t: i64, u: i64, v: i64| {
        i64::try_from(wide_product(s, t).checked_sub(wide_product(u, v))?).ok()
    };
    Some([
        minor(ay, bz, az, by)?,
        minor(az, bx, ax, bz)?,
        minor(ax, by, ay, bx)?,
    ])
}

/// `a` · `b` for `a` and `b` in `i64`, in `i128`.
fn dot_of_words(a: [i64; 3], b: [i64; 3]) -> Option<i128> {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    wide_product(ax, bx)
        .checked_add(wide_product(ay, by))?
        .checked_add(wide_product(az, bz))
}

/// X and w in `i64`, if they fit.
fn point_words(x: [i128; 3], w: i128) -> Option<([i64; 3], i64)> {
    let [first, second, third] = x;
    Some((
        [
            i64::try_from(first).ok()?,
            i64::try_from(second).ok()?,
            i64::try_from(third).ok()?,
        ],
        i64::try_from(w).ok()?,
    ))
}

/// `a` · `b` in `i128`, if every step fits.
fn dot_in_words(a: [i128; 3], b: [i128; 3]) -> Option<i128> {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    ax.checked_mul(bx)?
        .checked_add(ay.checked_mul(by)?)?
        .checked_add(az.checked_mul(bz)?)
}

/// The point where three planes meet, as X and w in `i128`, for a point read against many planes,
/// as a segment's ends and a polygon's corners are. The 4 × 4 determinant of its three planes and
/// a fourth, s, expands along the offsets to `n_s` · X + `d_s` · w (6.3), so a reading is four
/// products where the generated predicate redoes the whole determinant. Every step is checked, so
/// a reading is exact or `None`, and the caller falls back to the predicate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Meeting {
    x: [i128; 3],
    w: i128,
    /// X and w in `i64`, where they fit: a reading against a plane whose coefficients fit too is
    /// then four single multiplications.
    words: Option<([i64; 3], i64)>,
}

impl Meeting {
    /// The point where `p`, `q` and `r` meet: X = −`d_p` (`n_q` × `n_r`) + `d_q` (`n_p` × `n_r`) −
    /// `d_r` (`n_p` × `n_q`) and w = det(`n_p`, `n_q`, `n_r`), the cofactors of the offsets' column;
    /// `None` if a step leaves `i128`, as for planes near the budget's edge.
    pub(crate) fn new(p: &Plane, q: &Plane, r: &Plane) -> Option<Self> {
        let point = Self::from_words(p, q, r).or_else(|| Self::from_wide(p, q, r))?;
        Some(Meeting {
            words: point_words(point.x, point.w),
            ..point
        })
    }

    /// [`Meeting::new`] for planes whose coefficients, and whose normals' cross products, fit
    /// `i64`, as a building's do: every product is of two `i64`s.
    fn from_words(p: &Plane, q: &Plane, r: &Plane) -> Option<Self> {
        let ((n_p, d_p), (n_q, d_q), (n_r, d_r)) =
            (plane_words(p)?, plane_words(q)?, plane_words(r)?);
        let (qr, pr, pq) = (
            cross_of_words(n_q, n_r)?,
            cross_of_words(n_p, n_r)?,
            cross_of_words(n_p, n_q)?,
        );
        let mut x = [0; 3];
        for (slot, ((a, b), c)) in x.iter_mut().zip(qr.iter().zip(&pr).zip(&pq)) {
            *slot = wide_product(d_q, *b)
                .checked_sub(wide_product(d_p, *a))?
                .checked_sub(wide_product(d_r, *c))?;
        }
        Some(Meeting {
            x,
            w: dot_of_words(n_p, qr)?,
            words: None,
        })
    }

    /// [`Meeting::new`] in `i128` throughout.
    fn from_wide(p: &Plane, q: &Plane, r: &Plane) -> Option<Self> {
        let (n_p, n_q, n_r) = (p.n(), q.n(), r.n());
        let (qr, pr, pq) = (
            cross_in_words(n_q, n_r)?,
            cross_in_words(n_p, n_r)?,
            cross_in_words(n_p, n_q)?,
        );
        let mut x = [0; 3];
        for (slot, ((a, b), c)) in x.iter_mut().zip(qr.iter().zip(&pr).zip(&pq)) {
            *slot = q
                .d()
                .checked_mul(*b)?
                .checked_sub(p.d().checked_mul(*a)?)?
                .checked_sub(r.d().checked_mul(*c)?)?;
        }
        Some(Meeting {
            x,
            w: dot_in_words(n_p, qr)?,
            words: None,
        })
    }

    /// The sign of the 4 × 4 determinant of the three planes and `s`, rows in that order: `n_s` · X
    /// + `d_s` · w, or `None` if a step leaves `i128`.
    pub(crate) fn reading(&self, s: &Plane) -> Option<Sign> {
        if let (Some((x, w)), Some((n, d))) = (self.words, plane_words(s)) {
            let value = dot_of_words(n, x)?.checked_add(wide_product(d, w))?;
            return Some(Sign::of_i128(value));
        }
        let value = dot_in_words(s.n(), self.x)?.checked_add(s.d().checked_mul(self.w)?)?;
        Some(Sign::of_i128(value))
    }

    /// X and w, as [`Tpi::homogeneous`](geom_num::plane::Tpi::homogeneous) gives them: the same
    /// cofactors, so the same values.
    pub(crate) const fn coordinates(&self) -> ([i128; 3], i128) {
        (self.x, self.w)
    }

    /// The sign of w, det(`n_p`, `n_q`, `n_r`): zero when the three planes do not meet in a point.
    pub(crate) const fn weight(&self) -> Sign {
        Sign::of_i128(self.w)
    }

    /// Each coordinate `X_i` / w rounded to the nearest integer, ties to even, as
    /// `Tpi::to_grid` rounds: ties to even are the same whether the magnitude or the signed value
    /// is rounded. `None` for w zero, or a step that leaves `i128`.
    pub(crate) fn rounded(&self) -> Option<[i128; 3]> {
        let turn = self.w.is_negative();
        let w = if turn { self.w.checked_neg()? } else { self.w };
        if w == 0 {
            return None;
        }
        let mut out = [0; 3];
        for (slot, x) in out.iter_mut().zip(self.x) {
            let x = if turn { x.checked_neg()? } else { x };
            let (floor, rest) = (x.checked_div_euclid(w)?, x.checked_rem_euclid(w)?);
            let twice = rest.checked_mul(2)?;
            let up = twice > w || (twice == w && floor.checked_rem_euclid(2)? == 1);
            *slot = if up { floor.checked_add(1)? } else { floor };
        }
        Some(out)
    }

    /// Each coordinate's floor, ⌊`X_i` / w⌋, in `i64`: the point lies in the unit cell from it.
    /// One division of machine words each where the point fits them. `None` for w zero, or a
    /// floor past `i64`.
    pub(crate) fn floors(&self) -> Option<[i64; 3]> {
        if let Some((x, w)) = self.words
            && w > 0
        {
            let [a, b, c] = x;
            return Some([
                a.checked_div_euclid(w)?,
                b.checked_div_euclid(w)?,
                c.checked_div_euclid(w)?,
            ]);
        }
        let turn = self.w.is_negative();
        let w = if turn { self.w.checked_neg()? } else { self.w };
        if w == 0 {
            return None;
        }
        let mut out = [0; 3];
        for (slot, x) in out.iter_mut().zip(self.x) {
            let x = if turn { x.checked_neg()? } else { x };
            *slot = i64::try_from(x.checked_div_euclid(w)?).ok()?;
        }
        Some(out)
    }
}

/// The grid cells around some points, a step wider each way: every point within a unit of one
/// of them, an infinitesimal move included, lies inside. Two spans that miss share no point.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Span {
    pub(crate) low: [i64; 3],
    pub(crate) high: [i64; 3],
}

impl Span {
    /// The span of `points`, if every one has its floors.
    pub(crate) fn of(points: impl IntoIterator<Item = Option<Meeting>>) -> Option<Self> {
        let mut low = [i64::MAX; 3];
        let mut high = [i64::MIN; 3];
        for point in points {
            let floors = point?.floors()?;
            for ((l, h), v) in low.iter_mut().zip(&mut high).zip(floors) {
                *l = (*l).min(v);
                *h = (*h).max(v);
            }
        }
        // A point lies in [floor, floor + 1) on each axis; a step more each way holds the moves.
        Some(Span {
            low: low.map(|v| v.saturating_sub(1)),
            high: high.map(|v| v.saturating_add(2)),
        })
    }

    /// Whether the two spans are apart on some axis.
    pub(crate) fn misses(&self, other: &Span) -> bool {
        let apart = |(a, b): (&Span, &Span)| a.high.iter().zip(&b.low).any(|(h, l)| h < l);
        apart((self, other)) || apart((other, self))
    }
}

/// p · s − q · r, within 2^167 in magnitude for normal components below 2^83.
fn minor(p: i128, q: i128, r: i128, s: i128) -> Result<I256, NumError> {
    let product = |a: i128, b: i128| I256::from_i128(a).checked_mul(&I256::from_i128(b));
    product(p, s)
        .zip(product(q, r))
        .and_then(|(a, b)| a.checked_sub(&b))
        .ok_or(NumError::BudgetExceeded)
}

/// `n_a` × `n_b`.
fn cross(a: &Plane, b: &Plane) -> Result<[I256; 3], NumError> {
    let ([ax, ay, az], [bx, by, bz]) = (a.n(), b.n());
    Ok([
        minor(ay, az, by, bz)?,
        minor(az, ax, bz, bx)?,
        minor(ax, ay, bx, by)?,
    ])
}

/// The sign of each component of `v`.
fn signs(v: &[I256; 3]) -> [Sign; 3] {
    let [x, y, z] = v;
    [x.signum(), y.signum(), z.signum()]
}

/// The line where two planes meet, running along `n_p` × `n_q`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    p: Plane,
    q: Plane,
    /// The signs of `n_p` × `n_q`'s components.
    direction: [Sign; 3],
}

impl Line {
    /// The line where `p` and `q` meet.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] for parallel planes, which meet in no line or in a whole plane.
    pub fn new(p: Plane, q: Plane) -> Result<Line, NumError> {
        let direction = signs(&cross(&p, &q)?);
        if direction == [Sign::Zero; 3] {
            return Err(NumError::Degenerate);
        }
        Ok(Line { p, q, direction })
    }

    /// The two planes.
    #[must_use]
    pub const fn planes(&self) -> [Plane; 2] {
        [self.p, self.q]
    }

    /// Where `plane` crosses the line.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] for a plane parallel to the line.
    pub fn point(&self, plane: &Plane) -> Result<Tpi, NumError> {
        Tpi::meeting(self.p, self.q, *plane)
    }
}

/// The order along `line` of the points where `c` and `d` cross it: [`Ordering::Less`] when `c`'s
/// comes first in the line's direction.
///
/// # Errors
///
/// [`NumError::Degenerate`] for a plane parallel to the line.
pub fn along(line: &Line, c: &Plane, d: &Plane) -> Result<Ordering, NumError> {
    let [p, q] = line.planes();
    let crossing = normals_det(&p, &q, c)?;
    if crossing == Sign::Zero {
        return Err(NumError::Degenerate);
    }
    let other = line.point(d)?;
    Ok(match times(c.side_of_tpi(&other)?, crossing) {
        Sign::Positive => Ordering::Less,
        Sign::Negative => Ordering::Greater,
        Sign::Zero => Ordering::Equal,
    })
}

/// Whether two three-plane points are one point: `a` lies on all three planes of `b` (11.3).
///
/// # Errors
///
/// [`NumError::BudgetExceeded`], which planes within budget cannot reach.
pub fn same_point(a: &Tpi, b: &Tpi) -> Result<bool, NumError> {
    for plane in b.planes() {
        if plane.side_of_tpi(a)? != Sign::Zero {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A ray from a vertex of a plane F, along the line where F meets `with`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ray {
    /// The plane that makes the line with F.
    pub with: Plane,
    /// Whether it runs along −`n_F` × `n_with` rather than `n_F` × `n_with`.
    pub reversed: bool,
}

/// Which way `b` turns from `a` about `face`'s normal: `Positive` counter-clockwise seen from where
/// the normal points, `Zero` for rays on one line.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`], which planes within budget cannot reach.
pub fn turn(face: &Plane, a: &Ray, b: &Ray) -> Result<Sign, NumError> {
    let det = normals_det(face, &a.with, &b.with)?;
    Ok(turned(det, a.reversed != b.reversed))
}

/// Whether two rays of `face` on one line run the same way.
fn same_way(face: &Plane, a: &Ray, b: &Ray) -> Result<bool, NumError> {
    let (x, y) = (cross(face, &a.with)?, cross(face, &b.with)?);
    let reversed = a.reversed != b.reversed;
    Ok(signs(&x)
        .iter()
        .zip(signs(&y))
        .find(|(s, _)| **s != Sign::Zero)
        .is_some_and(|(s, t)| turned(times(*s, t), reversed) == Sign::Positive))
}

/// Where a ray falls from `first`: itself, the strict left, the opposite ray, the strict right.
fn place(left: Sign, same: impl FnOnce() -> Result<bool, NumError>) -> Result<u8, NumError> {
    Ok(match left {
        Sign::Positive => 1,
        Sign::Negative => 3,
        Sign::Zero if same()? => 0,
        Sign::Zero => 2,
    })
}

/// The order of two angles of equal place, `b`'s turn from `a` deciding.
const fn within(place: u8, between: Sign) -> Ordering {
    match (place, between) {
        (1 | 3, Sign::Positive) => Ordering::Less,
        (1 | 3, Sign::Negative) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

/// The order of `a` and `b` by angle counter-clockwise from `first`, in `face`.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`], which planes within budget cannot reach.
pub fn around(face: &Plane, first: &Ray, a: &Ray, b: &Ray) -> Result<Ordering, NumError> {
    let x = place(turn(face, first, a)?, || same_way(face, first, a))?;
    let y = place(turn(face, first, b)?, || same_way(face, first, b))?;
    if x == y {
        Ok(within(x, turn(face, a, b)?))
    } else {
        Ok(x.cmp(&y))
    }
}

/// A half-plane bounded by a line, on `plane`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flap {
    /// The plane it lies on, which holds the line.
    pub plane: Plane,
    /// Whether it runs along −`n` × v rather than `n` × v, for v the line's direction.
    pub reversed: bool,
}

/// Which way `b` turns from `a` about `line`: `Positive` counter-clockwise seen looking back along
/// the line's direction, `Zero` for flaps on one plane.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`], which planes within budget cannot reach.
pub fn turn_about(line: &Line, a: &Flap, b: &Flap) -> Result<Sign, NumError> {
    let between = signs(&cross(&a.plane, &b.plane)?);
    let sign = line
        .direction
        .iter()
        .zip(between)
        .find(|(v, _)| **v != Sign::Zero)
        .map_or(Sign::Zero, |(v, s)| times(*v, s));
    Ok(turned(sign, a.reversed != b.reversed))
}

/// Whether two flaps on one plane run the same way: their planes' normals agree, or both are
/// reversed against opposite ones.
fn same_flap(a: &Flap, b: &Flap) -> bool {
    let (x, y) = (a.plane.n(), b.plane.n());
    let agree = x
        .iter()
        .zip(y)
        .find(|(p, _)| **p != 0)
        .is_some_and(|(p, q)| p.signum() == q.signum());
    agree != (a.reversed != b.reversed)
}

/// The order of `a` and `b` by angle from `first`, about `line`.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`], which planes within budget cannot reach.
pub fn about(line: &Line, first: &Flap, a: &Flap, b: &Flap) -> Result<Ordering, NumError> {
    let x = place(turn_about(line, first, a)?, || Ok(same_flap(first, a)))?;
    let y = place(turn_about(line, first, b)?, || Ok(same_flap(first, b)))?;
    if x == y {
        Ok(within(x, turn_about(line, a, b)?))
    } else {
        Ok(x.cmp(&y))
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::arithmetic_side_effects,
        clippy::indexing_slicing,
        clippy::many_single_char_names,
        clippy::unwrap_used,
        reason = "tests: native arithmetic, planes named as in 6.3, and a failure stops the test"
    )]

    use geom_num::Sign;
    use geom_num::plane::{Plane, Tpi, determinant_sign, normals_sign};
    use num_bigint::BigInt;
    use proptest::prelude::*;

    use super::{Meeting, Span};

    /// Planes with small coefficients, so that ties and parallel normals come often, or with
    /// normals below 2^24 and offsets below 2^44, where every step stays below 2^122.
    fn planes(normal: i128, offset: i128) -> impl Strategy<Value = Vec<Plane>> {
        let one = (prop::array::uniform3(-normal..=normal), -offset..=offset)
            .prop_filter_map("a zero normal", |(n, d)| Plane::new(n, d).ok());
        prop::collection::vec(one, 4)
    }

    fn agrees(rows: &[Plane]) {
        let [p, q, r, s] = [rows[0], rows[1], rows[2], rows[3]];
        let at = Meeting::new(&p, &q, &r).unwrap();
        assert_eq!(at.weight(), normals_sign(&p, &q, &r).unwrap());
        assert_eq!(
            at.reading(&s),
            Some(determinant_sign(&p, &q, &r, &s).unwrap())
        );
    }

    proptest! {
        #[test]
        fn a_reading_is_the_determinant_when_ties_are_common(rows in planes(2, 3)) {
            agrees(&rows);
        }

        #[test]
        fn a_reading_is_the_determinant_for_a_building(rows in planes(1 << 24, 1 << 44)) {
            agrees(&rows);
        }
    }

    #[test]
    fn a_meeting_too_wide_for_words_is_none() {
        let big = 1i128 << 80;
        let p = Plane::new([big, 1, 0], 0).unwrap();
        let q = Plane::new([0, big, 1], 0).unwrap();
        let r = Plane::new([1, 0, big], 0).unwrap();
        assert!(Meeting::new(&p, &q, &r).is_none());
        // Normals that fit, offsets that do not: X leaves `i128` though w does not.
        let far = 1i128 << 124;
        let [a, b, c] = [[1, 2, 3], [3, -1, 2], [2, 3, -5]].map(|n| Plane::new(n, far).unwrap());
        assert!(Meeting::new(&a, &b, &c).is_none());
    }

    fn rounds_as_the_grid_does(rows: &[Plane]) {
        let [p, q, r] = [rows[0], rows[1], rows[2]];
        let Some(rounded) = Meeting::new(&p, &q, &r).and_then(|m| m.rounded()) else {
            assert!(Tpi::meeting(p, q, r).is_err());
            return;
        };
        let at = Tpi::meeting(p, q, r).unwrap().to_grid();
        match at {
            Ok(near) => {
                let v = near.value();
                assert_eq!(
                    rounded,
                    [v.x.get(), v.y.get(), v.z.get()].map(i128::from),
                    "{p:?} {q:?} {r:?}"
                );
            }
            Err(_) => assert!(rounded.iter().any(|v| v.unsigned_abs() > (1u128 << 40))),
        }
    }

    proptest! {
        #[test]
        fn a_rounding_is_the_grid_s_with_ties(rows in planes(2, 7)) {
            rounds_as_the_grid_does(&rows);
        }

        #[test]
        fn a_rounding_is_the_grid_s_for_a_building(rows in planes(1 << 24, 1 << 44)) {
            rounds_as_the_grid_does(&rows);
        }

        #[test]
        fn floors_are_exact_floors(rows in planes(1 << 24, 1 << 44)) {
            floors_as_divided(&rows);
        }

        #[test]
        fn floors_are_exact_floors_with_ties(rows in planes(2, 7)) {
            floors_as_divided(&rows);
        }
    }

    /// [`Meeting::floors`] against a floor of big integers, for words and for `i128`.
    fn floors_as_divided(rows: &[Plane]) {
        let [p, q, r] = [rows[0], rows[1], rows[2]];
        let Some(at) = Meeting::new(&p, &q, &r) else {
            return;
        };
        let (x, w) = at.coordinates();
        let wide = Meeting { words: None, ..at };
        for meeting in [at, wide] {
            let got = meeting.floors();
            if w == 0 {
                assert!(got.is_none());
                continue;
            }
            let want = x.map(|v| {
                let (v, w) = if w < 0 {
                    (-BigInt::from(v), -BigInt::from(w))
                } else {
                    (BigInt::from(v), BigInt::from(w))
                };
                // The remainder in [0, w), then an exact quotient.
                let rest = ((&v % &w) + &w) % &w;
                (v - rest) / w
            });
            match got {
                Some(floors) => assert_eq!(floors.map(BigInt::from), want),
                None => assert!(want.iter().any(|v| i64::try_from(v.clone()).is_err())),
            }
        }
    }

    #[test]
    fn normals_whose_cross_product_leaves_i64_take_i128() {
        let big = 1i128 << 40;
        let p = Plane::new([big, 0, 0], -3 * big).unwrap();
        let q = Plane::new([0, big, 0], -4 * big).unwrap();
        let r = Plane::new([0, 0, 1], -5).unwrap();
        let at = Meeting::new(&p, &q, &r).unwrap();
        assert!(at.words.is_none());
        for s in [[1, 1, 1, -12], [1, 1, 1, -11], [2, -1, 3, 7]] {
            let s = Plane::new([s[0], s[1], s[2]], s[3]).unwrap();
            assert_eq!(
                at.reading(&s),
                Some(determinant_sign(&p, &q, &r, &s).unwrap())
            );
        }
        assert_eq!(at.weight(), normals_sign(&p, &q, &r).unwrap());
    }

    #[test]
    fn a_reading_too_wide_for_words_is_none() {
        let [p, q, r] =
            [[1, 0, 0], [0, 1, 0], [0, 0, 1]].map(|n| Plane::new(n, -(1i128 << 60)).unwrap());
        let at = Meeting::new(&p, &q, &r).unwrap();
        let s = Plane::new([1i128 << 70, 0, 0], 0).unwrap();
        assert_eq!(at.reading(&s), None);
        let t = Plane::new([1, 1, 1], 1i128 << 124).unwrap();
        assert_eq!(at.weight(), Sign::Positive);
        assert_eq!(
            at.reading(&t),
            Some(determinant_sign(&p, &q, &r, &t).unwrap())
        );
    }

    fn small_plane() -> impl Strategy<Value = Plane> {
        ([-9i128..9, -9i128..9, -9i128..9], -999i128..999)
            .prop_filter_map("a normal", |(n, d)| Plane::new(n, d).ok())
    }

    proptest! {
        /// Every point a span is made of lies a unit inside it, exactly.
        #[test]
        fn a_span_holds_its_points_a_unit_inside(rows in prop::collection::vec((small_plane(), small_plane(), small_plane()), 1..5)) {
            let points: Vec<Option<Meeting>> = rows.iter().map(|(p, q, r)| Meeting::new(p, q, r)).collect();
            let Some(span) = Span::of(points.iter().copied()) else {
                return Ok(());
            };
            for point in points.iter().flatten() {
                let (x, w) = point.coordinates();
                let (x, w) = if w < 0 { (x.map(|v| -v), -w) } else { (x, w) };
                for ((v, low), high) in x.iter().zip(span.low).zip(span.high) {
                    let at = BigInt::from(*v);
                    let (low, high) = (BigInt::from(low + 1), BigInt::from(high - 1));
                    prop_assert!(at >= &low * w && at <= &high * w);
                }
            }
        }

        /// Spans miss exactly when they are apart on an axis, either way round.
        #[test]
        fn spans_miss_when_apart(a in prop::array::uniform6(-9i64..9), b in prop::array::uniform6(-9i64..9)) {
            let span = |v: [i64; 6]| Span {
                low: [v[0].min(v[1]), v[2].min(v[3]), v[4].min(v[5])],
                high: [v[0].max(v[1]), v[2].max(v[3]), v[4].max(v[5])],
            };
            let (s, t) = (span(a), span(b));
            let apart = (0..3).any(|i| s.high[i] < t.low[i] || t.high[i] < s.low[i]);
            prop_assert_eq!(s.misses(&t), apart);
            prop_assert_eq!(t.misses(&s), apart);
        }
    }
}
