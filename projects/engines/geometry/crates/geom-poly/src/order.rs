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
