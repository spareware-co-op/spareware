//! The exact analytic decision table of the certified curved path (11.4, G242).
//!
//! What it does: the decisions the curved builder takes with no enclosure at all, on grid points
//! and on quadrics held by grid points and whole numbers of steps. A grid point against a plane, a
//! sphere, a cylinder, a cone or a torus ([`point_sphere`], [`point_cylinder`], [`point_cone`],
//! [`point_torus`]; the plane's is [`geom_num::plane::Plane::side`]). A plane against a sphere or a
//! cylinder, apart, tangent or crossing ([`plane_sphere`], [`plane_cylinder`]). Two planes, two
//! spheres or two cylinders coinciding ([`planes_coincide`], [`spheres_coincide`],
//! [`cylinders_coincide`]). The quadrics are `geom_num::quadric`'s, the planes are
//! `geom_num::plane`'s; this module adds only what a boolean asks of them.
//!
//! The ranges: coordinates of at most 2^40 − 1 (so differences of at most 2^41), radii of at most
//! 2^41 − 1, a cone's slope numerator and denominator below 2^40, and planes within `Plane`'s own
//! budget (normal below 2^83, offset below 2^125). The bits are our derivation, step by step in
//! each function, and each is held by an extreme-value test against `num-bigint`:
//!
//! | Decision | Largest value | Type |
//! | --- | --- | --- |
//! | point with plane (`Plane::side`) | below 2^126 | `i128` |
//! | point with sphere | below 2^85 | `i128` |
//! | point with cylinder | below 2^170 | [`I256`] |
//! | point with cone | below 2^249 | [`I256`] |
//! | point with torus | below 2^255 | [`I256`] |
//! | plane with sphere or cylinder | below 2^251 | [`I256`] |
//! | two planes, two cylinders coinciding | below 2^209 | [`I256`] |
//!
//! Guarantees:
//!
//! - **Decided or refused.** Every answer is exact. Every step is a checked one, and a step that
//!   does not fit is [`NumError::BudgetExceeded`]: an input outside the ranges above, never a wrong
//!   sign.
//! - **No square root, no division, no angle.** Each decision is the sign of a polynomial of the
//!   inputs: a quadric's implicit equation with its axis cleared of its length (11.4).
//! - **Degenerate shapes are refused.** An axis of two equal points, a zero slope denominator
//!   and a radius of 2^41 or more are [`NumError::Degenerate`] or [`NumError::BudgetExceeded`].
//!
//! Method: Goldman and Miller's implicit forms of the quadrics (SMA '91, doi:10.1145/112515.112545;
//! GMIP 57(1), 1995, doi:10.1006/gmip.1995.1006), as `geom_num::quadric` takes them, here for a point
//! and a plane. The implicit equations are as follows. A sphere: |x|² − r². A cylinder about the axis through a with
//! direction d: |x × d|² − r²|d|². A cone with apex a, axis direction d and slope p/q:
//! q²|x × d|² − p²(x · d)², both nappes. A torus about the axis through c with direction e,
//! major radius R and minor radius r: |e|²(|x|² + R² − r²)² − 4R²(|e|²|x|² − (x · e)²), which is
//! the tube's equation multiplied by |e|² so that no division is needed, as `geom_num::quadric`
//! writes it for a line (Goldman and Miller; chapter 12.7). A plane n · x + d against a sphere
//! (centre s, radius r): the distance is |n · s + d| / |n|, so it is compared as
//! (n · s + d)² against r²|n|². Against a cylinder it crosses unless n · d = 0, and then the axis
//! is parallel and the same comparison is made at a point of the axis.
//!
//! Cost: constant, a few dozen wide multiplications. No allocation. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the chapter's: p a point, x its offset, d an axis, r a radius, n a normal"
)]

use geom_num::grid::P3;
use geom_num::plane::Plane;
use geom_num::quadric::{Cone, Cylinder, Sphere, Torus};
use geom_num::sign::Sign;
use geom_num::wide::I256;
use geom_num::{NumError, Steps};

/// A coordinate difference or other whole number, as the wide type.
fn wide(v: i128) -> I256 {
    I256::from_i128(v)
}

/// `a · b`, or the error of a product that does not fit.
fn mul(a: &I256, b: &I256) -> Result<I256, NumError> {
    a.checked_mul(b).ok_or(NumError::BudgetExceeded)
}

/// `a + b`, or the error of a sum that does not fit.
fn add(a: &I256, b: &I256) -> Result<I256, NumError> {
    a.checked_add(b).ok_or(NumError::BudgetExceeded)
}

/// `a − b`, or the error of a difference that does not fit.
fn sub(a: &I256, b: &I256) -> Result<I256, NumError> {
    a.checked_sub(b).ok_or(NumError::BudgetExceeded)
}

/// The square of a whole number held in 128 bits.
fn square(a: i128) -> Result<I256, NumError> {
    let w = wide(a);
    mul(&w, &w)
}

/// The triple `a − b` of coordinates, each below 2^41 in size.
fn minus(a: P3, b: P3) -> Result<[i128; 3], NumError> {
    let part = |p: i64, q: i64| -> Result<i128, NumError> {
        p.checked_sub(q)
            .map(i128::from)
            .ok_or(NumError::BudgetExceeded)
    };
    Ok([
        part(a.x.get(), b.x.get())?,
        part(a.y.get(), b.y.get())?,
        part(a.z.get(), b.z.get())?,
    ])
}

/// The dot product of two triples of at most 2^41 in size each: below 2^84, in `i128`.
fn dot(a: &[i128; 3], b: &[i128; 3]) -> Result<i128, NumError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    a0.checked_mul(b0)
        .and_then(|x| x.checked_add(a1.checked_mul(b1)?))
        .and_then(|x| x.checked_add(a2.checked_mul(b2)?))
        .ok_or(NumError::BudgetExceeded)
}

/// The cross product of two triples of at most 2^41 in size each: each component below 2^83.
fn cross(a: &[i128; 3], b: &[i128; 3]) -> Result<[i128; 3], NumError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    let part = |p: i128, q: i128, r: i128, s: i128| -> Option<i128> {
        p.checked_mul(q)?.checked_sub(r.checked_mul(s)?)
    };
    match (
        part(a1, b2, a2, b1),
        part(a2, b0, a0, b2),
        part(a0, b1, a1, b0),
    ) {
        (Some(x), Some(y), Some(z)) => Ok([x, y, z]),
        _ => Err(NumError::BudgetExceeded),
    }
}

/// The sum of squares of a triple of at most 2^84 in size each: below 2^170, in [`I256`].
fn norm_squared(a: &[i128; 3]) -> Result<I256, NumError> {
    let [a0, a1, a2] = *a;
    add(&add(&square(a0)?, &square(a1)?)?, &square(a2)?)
}

/// A radius as a whole number, or the error of one of 2^41 or more.
fn radius(r: Steps) -> Result<i128, NumError> {
    if r.0 >= 1 << 41 {
        Err(NumError::BudgetExceeded)
    } else {
        Ok(i128::from(r.0))
    }
}

/// An axis direction `to − from`, refused if its two points are one point.
fn axis(from: P3, to: P3) -> Result<[i128; 3], NumError> {
    let d = minus(to, from)?;
    if d == [0, 0, 0] {
        Err(NumError::Degenerate)
    } else {
        Ok(d)
    }
}

/// The sign of the difference of two wide values.
fn compare(a: &I256, b: &I256) -> Result<Sign, NumError> {
    Ok(sub(a, b)?.signum())
}

/// How a plane meets a quadric.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Meeting {
    /// They have no point in common.
    Apart,
    /// They touch and do not cross.
    Tangent,
    /// They cross.
    Crossing,
}

/// Which side of a sphere a grid point is on: the sign of |p − c|² − r². `Positive` is outside,
/// `Zero` on the surface, `Negative` inside.
///
/// Bit budget: each difference is below 2^41 in size, its square below 2^82, the sum of three
/// below 2^83.6, and r² is below 2^82, so the difference is below 2^84, in `i128`; the
/// comparison is made in [`I256`] all the same, every step checked.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for a radius of 2^41 or more.
pub fn point_sphere(p: P3, sphere: &Sphere) -> Result<Sign, NumError> {
    let x = minus(p, sphere.centre)?;
    let r = radius(sphere.radius)?;
    compare(&norm_squared(&x)?, &square(r)?)
}

/// Which side of a cylinder a grid point is on: the sign of |x × d|² − r²|d|² for x = p − a and d
/// the axis direction. `Positive` is outside the cylinder, `Zero` on it, `Negative` inside.
///
/// Bit budget: x and d are below 2^41 in size, so each component of x × d is below 2^83 and its
/// square below 2^166, and the sum of three is below 2^167.6. |d|² is below 2^83.6 and r² below
/// 2^82, so r²|d|² is below 2^166. The difference is below 2^168, in [`I256`] (limit 2^255).
///
/// # Errors
///
/// [`NumError::Degenerate`] for an axis of one point. [`NumError::BudgetExceeded`] for a radius of
/// 2^41 or more.
pub fn point_cylinder(p: P3, cylinder: &Cylinder) -> Result<Sign, NumError> {
    let d = axis(cylinder.base, cylinder.along)?;
    let x = minus(p, cylinder.base)?;
    let r = radius(cylinder.radius)?;
    let lateral = norm_squared(&cross(&x, &d)?)?;
    let allowed = mul(&square(r)?, &norm_squared(&d)?)?;
    compare(&lateral, &allowed)
}

/// Which side of a double cone a grid point is on: the sign of q²|x × d|² − p²(x · d)² for x = p −
/// apex, d the axis direction and p/q the slope. `Positive` is outside both nappes, `Zero` on the
/// surface, `Negative` inside.
///
/// Bit budget: |x × d|² is below 2^167.6 as for the cylinder, (x · d)² below (3·2^82)² = 2^167.2,
/// and p², q² below 2^80, so each product is below 2^248 and the difference below 2^249, in
/// [`I256`].
///
/// # Errors
///
/// [`NumError::Degenerate`] for an axis of one point or a slope with a zero denominator.
/// [`NumError::BudgetExceeded`] for a slope part of 2^40 or more.
pub fn point_cone(p: P3, cone: &Cone) -> Result<Sign, NumError> {
    if cone.slope.run == 0 {
        return Err(NumError::Degenerate);
    }
    if cone.slope.rise >= 1 << 40 || cone.slope.run >= 1 << 40 {
        return Err(NumError::BudgetExceeded);
    }
    let d = axis(cone.apex, cone.along)?;
    let x = minus(p, cone.apex)?;
    let along = square(dot(&x, &d)?)?;
    let lateral = norm_squared(&cross(&x, &d)?)?;
    let run = square(i128::from(cone.slope.run))?;
    let rise = square(i128::from(cone.slope.rise))?;
    compare(&mul(&run, &lateral)?, &mul(&rise, &along)?)
}

/// Which side of a torus a grid point is on: the sign of E(|x|² + R² − r²)² − 4R²(E|x|² − (x · e)²)
/// for x = p − centre, e the axis direction, E = |e|², R the major and r the minor radius.
/// `Positive` is outside the tube, `Zero` on it, `Negative` inside it.
///
/// Bit budget: E and |x|² are below 2^83.6; k = R² − r² is below 2^82 in size, so |x|² plus k is
/// below 2^84.6 and its square below 2^169.2; the first term is below 2^252.8. E|x|² and (x · e)²
/// are each below 2^167.2 and their difference is not negative (Cauchy–Schwarz) and below 2^167.2;
/// times 4R², below 2^84, it is below 2^251.2. The difference is below 2^253.5, in [`I256`] (limit
/// 2^255).
///
/// # Errors
///
/// [`NumError::Degenerate`] for an axis of one point. [`NumError::BudgetExceeded`] for a radius of
/// 2^41 or more.
pub fn point_torus(p: P3, torus: &Torus) -> Result<Sign, NumError> {
    let e = axis(torus.centre, torus.along)?;
    let x = minus(p, torus.centre)?;
    let (major, minor) = (radius(torus.major)?, radius(torus.minor)?);
    let big_e = norm_squared(&e)?;
    let x_squared = norm_squared(&x)?;
    let k = sub(&square(major)?, &square(minor)?)?;
    let tube = add(&x_squared, &k)?;
    let first = mul(&big_e, &mul(&tube, &tube)?)?;
    let along = square(dot(&x, &e)?)?;
    let perpendicular = sub(&mul(&big_e, &x_squared)?, &along)?;
    let four = wide(4);
    let second = mul(&mul(&four, &square(major)?)?, &perpendicular)?;
    compare(&first, &second)
}

/// n · s + d, the plane's value at a grid point, in `i128` (`Plane::side`'s own bit budget).
fn value_at(plane: &Plane, s: P3) -> Result<i128, NumError> {
    let n = plane.n();
    let [s0, s1, s2] = [s.x.get(), s.y.get(), s.z.get()].map(i128::from);
    let [n0, n1, n2] = n;
    n0.checked_mul(s0)
        .and_then(|x| x.checked_add(n1.checked_mul(s1)?))
        .and_then(|x| x.checked_add(n2.checked_mul(s2)?))
        .and_then(|x| x.checked_add(plane.d()))
        .ok_or(NumError::BudgetExceeded)
}

/// How the plane's distance from a point compares with a radius: the plane is apart from the
/// sphere about the point when (n · s + d)² > r²|n|², tangent when equal, crossing when less.
fn against_radius(plane: &Plane, s: P3, r: i128) -> Result<Meeting, NumError> {
    let value = square(value_at(plane, s)?)?;
    let allowed = mul(&square(r)?, &norm_squared(&plane.n())?)?;
    Ok(match compare(&value, &allowed)? {
        Sign::Positive => Meeting::Apart,
        Sign::Zero => Meeting::Tangent,
        Sign::Negative => Meeting::Crossing,
    })
}

/// How a plane meets a sphere: [`Meeting::Apart`] if no point is shared, [`Meeting::Tangent`] if
/// one is, [`Meeting::Crossing`] if the plane cuts it in a circle.
///
/// Bit budget: |n · s + d| is below 2^126 (the plane's own budget), so its square is below 2^252;
/// r² is below 2^82 and |n|² below 3·2^166 = 2^167.6, so r²|n|² is below 2^249.6. Both fit
/// [`I256`].
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for a radius of 2^41 or more.
pub fn plane_sphere(plane: &Plane, sphere: &Sphere) -> Result<Meeting, NumError> {
    against_radius(plane, sphere.centre, radius(sphere.radius)?)
}

/// How a plane meets a cylinder: [`Meeting::Crossing`] whenever the plane is not parallel to the
/// axis (an ellipse, or a pair of lines never: a plane through no axis direction cuts a full
/// ellipse), and when it is parallel, [`Meeting::Apart`], [`Meeting::Tangent`] along a line, or
/// [`Meeting::Crossing`] in two lines, by the distance of the axis from the plane.
///
/// Bit budget: n · d is below 3·2^83·2^41 = 2^125.6, in `i128`, and the parallel case is
/// [`plane_sphere`]'s at a point of the axis, below 2^252.
///
/// # Errors
///
/// [`NumError::Degenerate`] for an axis of one point. [`NumError::BudgetExceeded`] for a radius of
/// 2^41 or more.
pub fn plane_cylinder(plane: &Plane, cylinder: &Cylinder) -> Result<Meeting, NumError> {
    let d = axis(cylinder.base, cylinder.along)?;
    let r = radius(cylinder.radius)?;
    if dot(&plane.n(), &d)? != 0 {
        return Ok(Meeting::Crossing);
    }
    against_radius(plane, cylinder.base, r)
}

/// Whether two planes are the same set of points, the second a multiple of the first or its
/// negative: every 2 × 2 minor of the coefficients (n₁, d₁) and (n₂, d₂) is zero.
///
/// Bit budget: a minor n₁ᵢ·d₂ − n₂ᵢ·d₁ is below 2·2^83·2^125 = 2^209; n₁ᵢ·n₂ⱼ − n₁ⱼ·n₂ᵢ is below
/// 2^167. Both fit [`I256`].
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] only for planes outside their own budget, which `Plane` refuses.
pub fn planes_coincide(a: &Plane, b: &Plane) -> Result<bool, NumError> {
    let (na, nb) = (a.n(), b.n());
    let (da, db) = (a.d(), b.d());
    let minor = |p: i128, q: i128, r: i128, s: i128| -> Result<bool, NumError> {
        let left = mul(&wide(p), &wide(q))?;
        let right = mul(&wide(r), &wide(s))?;
        Ok(sub(&left, &right)?.is_zero())
    };
    let [a0, a1, a2] = na;
    let [b0, b1, b2] = nb;
    Ok(minor(a0, b1, a1, b0)?
        && minor(a0, b2, a2, b0)?
        && minor(a1, b2, a2, b1)?
        && minor(a0, db, da, b0)?
        && minor(a1, db, da, b1)?
        && minor(a2, db, da, b2)?)
}

/// Whether two spheres are the same: equal centres and equal radii.
#[must_use]
pub fn spheres_coincide(a: &Sphere, b: &Sphere) -> bool {
    a.centre == b.centre && a.radius == b.radius
}

/// Whether two cylinders are the same surface: equal radii, parallel axes, and the second axis's
/// base on the first's line.
///
/// Bit budget: the directions and the offset are below 2^41 in size, so each cross product
/// component is below 2^83, in `i128`.
///
/// # Errors
///
/// [`NumError::Degenerate`] for an axis of one point.
pub fn cylinders_coincide(a: &Cylinder, b: &Cylinder) -> Result<bool, NumError> {
    let (da, db) = (axis(a.base, a.along)?, axis(b.base, b.along)?);
    if a.radius != b.radius {
        return Ok(false);
    }
    let parallel = cross(&da, &db)? == [0, 0, 0];
    let on_line = cross(&minus(b.base, a.base)?, &da)? == [0, 0, 0];
    Ok(parallel && on_line)
}
