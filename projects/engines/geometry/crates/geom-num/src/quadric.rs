//! Exact decisions about lines and the surfaces of revolution on grid data: how a line meets a
//! sphere, cylinder, cone or torus, and the positions of two axes (chapter 12.6 and 12.7).
//!
//! What it does: a [`Line`] runs through two grid points; a [`Sphere`], [`Cylinder`], [`Cone`] or
//! [`Torus`] is held by grid points and whole numbers of steps, which is what chapter 12.7 calls
//! exact data. [`line_sphere`], [`line_cylinder`] and [`line_cone`] give the sign of a line's
//! discriminant with each, in a form with no square root and no division, through generated filters
//! and exact paths. [`polynomial`] writes a line's equation with a surface as whole-number
//! coefficients of a degree-2 or degree-4 polynomial in the line's parameter, and [`roots`] and
//! [`roots_between`] count its real roots with [`crate::sturm`]. [`parallel`], [`axis_gap`],
//! [`tangent_cylinder_cone`] and [`tangent_cone_cone`] are the tests of Goldman and Miller's special
//! positions.
//!
//! Guarantees, for coordinates of at most 2^40 − 1, so differences d, e and w of at most 2^41, and
//! radii of at most 2^41 − 1:
//!
//! - **Discriminants within their budgets.** Each generated module carries the bound of every
//!   step: line with sphere 169 bits, with cylinder 253, with cone 334. `None` from a generated
//!   path can only mean an input outside its declared range, and is
//!   [`NumError::BudgetExceeded`].
//! - **The tangent spheres' budgets.** |u|² is below 2^170 and each L below 2^170, for sizes of at
//!   most 2^41 and slopes of at most 2^40: a cylinder and a cone are below 2^(82+81+340) = 2^503, in
//!   `Wide<8>`, and two cones below 2^(80+81+84+340) = 2^585, in `Wide<10>`. Both are past chapter
//!   6.2's 384 bits and wait for an owner's acceptance for these rows (31.7), with
//!   [`crate::sturm`]'s `Wide<54>`.
//! - **A line with a torus has B = 256.** With D = d·d, H = d·w, W = w·w, E = e·e, all below 2^84,
//!   and k = R² − r² below 2^82 in size: E·P(t)² has coefficients below 2^84·2^171 = 2^255, for
//!   P = D·t² + 2H·t + W + k, whose square's largest coefficient is H² + ... below 2^171; and
//!   4R²·M(t) has coefficients below 2^84·2^170 = 2^254, for M = E·|x|² − (x·e)². So every coefficient
//!   of [`polynomial`] is below 2^256, as [`crate::sturm`]'s budget asks, and each step fits an
//!   `I320`.
//! - **The same polynomial in two arithmetics.** [`polynomial`] is one generic function over
//!   [`Ring`], so the interval filter's coefficients enclose the exact path's by construction.
//! - **Decided or refused.** A filter's answer is proven (each generated module says how, and an
//!   interval's sign is proven by outward rounding); otherwise the exact path answers.
//!
//! Method: Lagrange's identity for each discriminant, written out in the descriptions in
//! `crates/geom-gen/src/predicates.rs`, with no square root or division; the torus's implicit
//! equation (|x|² + R² − r²)² = 4R²(|x|² − (x·ê)²) with the axis cleared of its length; the special
//! positions of Goldman and Miller (SMA '91, doi:10.1145/112515.112545; GMIP 57(1), 1995,
//! doi:10.1006/gmip.1995.1006), including the sphere tangent to both quadrics about the point where
//! their axes meet.
//!
//! Cost: each predicate is one filter, and one exact path where the filter cannot tell. [`roots`]
//! is at most 60 interval operations, and a [`crate::sturm::Chain`] where those cannot tell. No
//! allocation. Recursion: none.

use crate::error::NumError;
use crate::generated::{
    axis_gap as gap, cross_norm, line_cone as cone_delta, line_cone_lead,
    line_cylinder as cylinder_delta, line_sphere as sphere_delta,
};
use crate::grid::{P3, Steps};
use crate::interval::Interval;
use crate::sign::Sign;
use crate::sturm::{self, Big, Chain, Point, Roots};
use crate::wide::{I320, Wide};

/// The line through two grid points, parametrised as a + t·(b − a).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    /// Its point at t = 0.
    pub a: P3,
    /// Its point at t = 1.
    pub b: P3,
}

/// A sphere: a grid centre and a whole number of steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sphere {
    /// Its centre.
    pub centre: P3,
    /// Its radius.
    pub radius: Steps,
}

/// A cylinder about the axis through two grid points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cylinder {
    /// One point of its axis.
    pub base: P3,
    /// Another.
    pub along: P3,
    /// Its radius.
    pub radius: Steps,
}

/// The tangent of a cone's half-angle, as a fraction of whole numbers below 2^40.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slope {
    /// The numerator.
    pub rise: u64,
    /// The denominator.
    pub run: u64,
}

/// A double cone: its apex, another point of its axis, and its slope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cone {
    /// Its apex.
    pub apex: P3,
    /// Another point of its axis.
    pub along: P3,
    /// Its slope.
    pub slope: Slope,
}

/// A torus about the axis through two grid points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Torus {
    /// Its centre.
    pub centre: P3,
    /// Another point of its axis.
    pub along: P3,
    /// The distance from the centre to the middle of the tube.
    pub major: Steps,
    /// The tube's radius.
    pub minor: Steps,
}

/// What a line can meet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// A sphere.
    Sphere(Sphere),
    /// A cylinder.
    Cylinder(Cylinder),
    /// A cone.
    Cone(Cone),
    /// A torus.
    Torus(Torus),
}

/// `filter`'s answer, or `exact`'s, or the error of an input outside the description's range.
fn decide(filter: Option<Sign>, exact: impl FnOnce() -> Option<Sign>) -> Result<Sign, NumError> {
    filter.map_or_else(|| exact().ok_or(NumError::BudgetExceeded), Ok)
}

/// A radius as a generated path takes it.
fn steps(r: Steps) -> i128 {
    i128::from(r.0)
}

/// The sign of a quarter of the discriminant of `line` with `sphere`: positive for two crossings,
/// zero for a tangent line, negative for none. A line whose points are one point gives its point's
/// nearness to nothing, so the caller refuses it first.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for a radius of 2^41 or more.
pub fn line_sphere(line: &Line, sphere: &Sphere) -> Result<Sign, NumError> {
    let (a, b, c, r) = (line.a, line.b, sphere.centre, steps(sphere.radius));
    decide(sphere_delta::filter(a, b, c, r), || {
        sphere_delta::exact(a, b, c, r)
    })
}

/// The sign of the discriminant of `line` with `cylinder`, as [`line_sphere`]'s, when the line is
/// not parallel to the axis, and zero or negative when it is ([`parallel`] tells them apart).
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for a radius of 2^41 or more.
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the descriptions and of the module header's formulas"
)]
pub fn line_cylinder(line: &Line, cylinder: &Cylinder) -> Result<Sign, NumError> {
    let (a, b, c, f, r) = (
        line.a,
        line.b,
        cylinder.base,
        cylinder.along,
        steps(cylinder.radius),
    );
    decide(cylinder_delta::filter(a, b, c, f, r), || {
        cylinder_delta::exact(a, b, c, f, r)
    })
}

/// The sign of the discriminant of `line` with `cone`. It holds for every line: with
/// [`line_cone_steep`] zero it is the square of the linear coefficient.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for a slope of 2^40 or more.
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the descriptions and of the module header's formulas"
)]
pub fn line_cone(line: &Line, cone: &Cone) -> Result<Sign, NumError> {
    let (a, b, c, f) = (line.a, line.b, cone.apex, cone.along);
    let (s, t) = (i128::from(cone.slope.rise), i128::from(cone.slope.run));
    decide(cone_delta::filter(a, b, c, f, s, t), || {
        cone_delta::exact(a, b, c, f, s, t)
    })
}

/// The sign of the leading coefficient of a line's quadratic with `cone`: positive when the line is
/// steeper than the cone, zero when it is parallel to a ruling, so the quadratic drops to a line.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for a slope of 2^40 or more.
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the descriptions and of the module header's formulas"
)]
pub fn line_cone_steep(line: &Line, cone: &Cone) -> Result<Sign, NumError> {
    let (a, b, c, f) = (line.a, line.b, cone.apex, cone.along);
    let (s, t) = (i128::from(cone.slope.rise), i128::from(cone.slope.run));
    decide(line_cone_lead::filter(a, b, c, f, s, t), || {
        line_cone_lead::exact(a, b, c, f, s, t)
    })
}

/// Whether b − a and d − c are parallel, or either is zero.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] if the exact path failed, which grid points cannot cause.
pub fn parallel(a: P3, b: P3, c: P3, d: P3) -> Result<bool, NumError> {
    decide(cross_norm::filter(a, b, c, d), || {
        cross_norm::exact(a, b, c, d)
    })
    .map(|s| s == Sign::Zero)
}

/// The sign of the distance from `c` to the axis through `a` and `b`, less `g` steps.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for a |g| of 2^42 or more.
pub fn axis_gap(a: P3, b: P3, c: P3, g: Steps) -> Result<Sign, NumError> {
    let g = steps(g);
    decide(gap::filter(a, b, c, g), || gap::exact(a, b, c, g))
}

/// `exact`'s sign, after `filter`'s if it has one.
fn signed<const L: usize>(
    filter: Option<Interval>,
    exact: impl FnOnce() -> Option<Wide<L>>,
) -> Result<Sign, NumError> {
    decide(filter.and_then(Interval::certain_sign), || {
        exact().map(|v| v.signum())
    })
}

/// The factors of a pair of meeting axes through `p` along `e` and `c` along `g`: with
/// u = e × g and h = c − p, the axes meet at p + (L₁ / |u|²)·e = c + (L₂ / |u|²)·g, for
/// L₁ = (h × g)·u and L₂ = (h × e)·u. Returns (|u|², L₁, L₂, |e|², |g|²).
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the descriptions and of the module header's formulas"
)]
fn meeting<R: Ring>(p: P3, q: P3, c: P3, f: P3) -> Option<[R; 5]> {
    let e: [R; 3] = minus(q, p)?;
    let g: [R; 3] = minus(f, c)?;
    let h: [R; 3] = minus(c, p)?;
    let u = outer(e, g)?;
    Some([
        inner(u, u)?,
        inner(outer(h, g)?, u)?,
        inner(outer(h, e)?, u)?,
        inner(e, e)?,
        inner(g, g)?,
    ])
}

/// r²(s² + t²)·|u|⁴ − s²·|e|²·L₁²: the cylinder's tangent sphere's squared radius, r², against the
/// cone's, s²/(s² + t²) times |L₁·e / |u|²|², denominators cleared.
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the descriptions and of the module header's formulas"
)]
fn cylinder_cone<R: Ring>(cylinder: &Cylinder, cone: &Cone) -> Option<R> {
    let [uu, first, _, ee, _] = meeting::<R>(cone.apex, cone.along, cylinder.base, cylinder.along)?;
    let r = size::<R>(cylinder.radius.0)?;
    let s = size::<R>(cone.slope.rise)?;
    let t = size::<R>(cone.slope.run)?;
    let ss = s.times(s)?;
    r.times(r)?
        .times(ss.plus(t.times(t)?)?)?
        .times(uu)?
        .times(uu)?
        .minus(ss.times(ee)?.times(first)?.times(first)?)
}

/// s₁²(s₂² + t₂²)·|e|²·L₁² − s₂²(s₁² + t₁²)·|g|²·L₂²: the two cones' tangent spheres' squared radii,
/// denominators cleared.
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the descriptions and of the module header's formulas"
)]
fn cone_cone<R: Ring>(first: &Cone, second: &Cone) -> Option<R> {
    let [_, one, two, ee, gg] = meeting::<R>(first.apex, first.along, second.apex, second.along)?;
    let (s, t) = (size::<R>(first.slope.rise)?, size::<R>(first.slope.run)?);
    let (v, w) = (size::<R>(second.slope.rise)?, size::<R>(second.slope.run)?);
    let (ss, vv) = (s.times(s)?, v.times(v)?);
    let left = ss
        .times(vv.plus(w.times(w)?)?)?
        .times(ee)?
        .times(one)?
        .times(one)?;
    let right = vv
        .times(ss.plus(t.times(t)?)?)?
        .times(gg)?
        .times(two)?
        .times(two)?;
    left.minus(right)
}

/// For a cylinder and a cone whose axes meet and are not parallel: the sign of the square of the
/// radius of the sphere about that point tangent to the cylinder, less that of the sphere tangent to
/// the cone. Zero is the position of two shared conics. An interval decides first; at degree 15 the
/// polynomial is past what `geom-gen`'s filters lay out, and outward rounding needs no constant.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for a radius or slope past 2^53, or a step that does not fit, which
/// the budget in the module header rules out.
pub fn tangent_cylinder_cone(cylinder: &Cylinder, cone: &Cone) -> Result<Sign, NumError> {
    signed(cylinder_cone::<Interval>(cylinder, cone), || {
        cylinder_cone::<Wide<8>>(cylinder, cone)
    })
}

/// [`tangent_cylinder_cone`] for two cones: `first`'s sphere against `second`'s.
///
/// # Errors
///
/// As [`tangent_cylinder_cone`].
pub fn tangent_cone_cone(first: &Cone, second: &Cone) -> Result<Sign, NumError> {
    signed(cone_cone::<Interval>(first, second), || {
        cone_cone::<Wide<10>>(first, second)
    })
}

/// A plane through three grid points that do not lie in a line: its normal is (b − a) × (c − a).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plane {
    /// A point of the plane.
    pub a: P3,
    /// A second.
    pub b: P3,
    /// A third.
    pub c: P3,
}

/// `plane`'s normal.
fn normal<R: Ring>(plane: &Plane) -> Option<[R; 3]> {
    outer(minus(plane.b, plane.a)?, minus(plane.c, plane.a)?)
}

fn ball<R: Ring>(plane: &Plane, centre: P3, radius: Steps) -> Option<R> {
    let n = normal::<R>(plane)?;
    let along = inner(n, minus::<R>(centre, plane.a)?)?;
    let r = size::<R>(radius.0)?;
    r.times(r)?.times(inner(n, n)?)?.minus(along.times(along)?)
}

/// The sign of r²·|n|² − (n·(p − a))², for the plane and the ball of `radius` about `centre`:
/// positive where the plane cuts the ball, zero where it touches, negative where it misses. It is
/// also how a plane parallel to a cylinder's axis stands against the cylinder, with `centre` on the
/// axis.
///
/// Budget: |n| < 2^85 and |n·(p − a)| < 2^129, so the square is below 2^258; r² < 2^106 and
/// |n|² < 2^172, so the product is below 2^278. `Wide<6>` holds both.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for a radius that does not fit an `i64`.
pub fn plane_ball(plane: &Plane, centre: P3, radius: Steps) -> Result<Sign, NumError> {
    signed(ball::<Interval>(plane, centre, radius), || {
        ball::<Wide<6>>(plane, centre, radius)
    })
}

fn along_axis<R: Ring>(plane: &Plane, p: P3, q: P3) -> Option<R> {
    inner(normal::<R>(plane)?, minus(q, p)?)
}

/// The sign of n·(q − p): zero exactly when the direction p to q lies along the plane.
///
/// # Errors
///
/// As [`plane_ball`].
pub fn plane_along(plane: &Plane, p: P3, q: P3) -> Result<Sign, NumError> {
    signed(along_axis::<Interval>(plane, p, q), || {
        along_axis::<Wide<6>>(plane, p, q)
    })
}

/// Whether the direction p to q is the plane's normal direction, exactly.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] if a step does not fit, which the budget of [`plane_ball`] rules out.
pub fn plane_across(plane: &Plane, p: P3, q: P3) -> Result<bool, NumError> {
    let n = normal::<Wide<6>>(plane).ok_or(NumError::BudgetExceeded)?;
    let e = minus::<Wide<6>>(q, p).ok_or(NumError::BudgetExceeded)?;
    let parts = outer(n, e).ok_or(NumError::BudgetExceeded)?;
    Ok(parts.iter().all(Wide::is_zero))
}

fn section<R: Ring>(plane: &Plane, cone: &Cone) -> Option<R> {
    let n = normal::<R>(plane)?;
    let e: [R; 3] = minus(cone.along, cone.apex)?;
    let (s, t) = (size::<R>(cone.slope.rise)?, size::<R>(cone.slope.run)?);
    let (s2, t2) = (s.times(s)?, t.times(t)?);
    let dot = inner(n, e)?;
    let left = dot.times(dot)?.times(s2.plus(t2)?)?;
    left.minus(s2.times(inner(n, n)?)?.times(inner(e, e)?)?)
}

/// Which conic a plane cuts from a cone: the sign of (n·e)²(s² + t²) − s²|n|²|e|². Positive is an
/// ellipse or circle, zero a parabola, negative a hyperbola; through the apex they are a point, one
/// ruling and two.
///
/// Budget: (n·e)² < 2^260 and s² + t² < 2^107, so the left is below 2^367, and the right below 2^364.
/// `Wide<8>` holds both.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for a slope that does not fit an `i64`.
pub fn plane_cone(plane: &Plane, cone: &Cone) -> Result<Sign, NumError> {
    signed(section::<Interval>(plane, cone), || {
        section::<Wide<8>>(plane, cone)
    })
}

/// How two cones through one apex stand: for a direction at `first`'s half-angle from its axis and at
/// `second`'s from the axis of `second`, on its upper nappe if `upper`, the sign of what is left for
/// the third coordinate. Positive is two rulings, zero one along which they touch, negative none.
///
/// With e and f the axes, s/t and u/v the slopes, the unit-vector sum 1 − g² − c² − d² ± 2cdg, over
/// a common denominator, is X ± Y, where X = (EF − D²)PQ − AQ·EF − BP·EF and
/// Y² = 4AB·D²·EF·PQ, for E = |e|², F = |f|², D = e·f, A = t², P = s² + t², B = v², Q = u² + v²; its
/// sign is read off X, the sign of Y and X² − Y².
///
/// Budget: EF < 2^172 and PQ < 2^216 for slopes of 53 bits, so |X| < 2^391, X² < 2^782, and Y² the
/// same. `Wide<14>` holds them all.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for a slope that does not fit an `i64`.
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the formula above"
)]
pub fn shared_apex(first: &Cone, second: &Cone, upper: bool) -> Result<Sign, NumError> {
    type W = Wide<14>;
    let e: [W; 3] = minus(first.along, first.apex).ok_or(NumError::BudgetExceeded)?;
    let f: [W; 3] = minus(second.along, second.apex).ok_or(NumError::BudgetExceeded)?;
    let run = |c: &Cone| -> Option<(W, W)> {
        let (s, t) = (size::<W>(c.slope.rise)?, size::<W>(c.slope.run)?);
        let t2 = t.times(t)?;
        Some((t2, s.times(s)?.plus(t2)?))
    };
    let parts = (|| {
        let (a, p) = run(first)?;
        let (b, q) = run(second)?;
        let (ee, ff, ef) = (inner(e, e)?, inner(f, f)?, inner(e, f)?);
        let both = ee.times(ff)?;
        let dd = ef.times(ef)?;
        let pq = p.times(q)?;
        let x = both
            .minus(dd)?
            .times(pq)?
            .minus(a.times(q)?.times(both)?)?
            .minus(b.times(p)?.times(both)?)?;
        let y2 = W::of(4)?
            .times(a)?
            .times(b)?
            .times(dd)?
            .times(both)?
            .times(pq)?;
        Some((x, ef, x.times(x)?.minus(y2)?))
    })();
    let (x, ef, difference) = parts.ok_or(NumError::BudgetExceeded)?;
    // Y has the sign of D, or against it on the lower nappe, and X the sign it has.
    let y_up = if upper {
        sign_of(&ef) == Sign::Positive
    } else {
        ef.is_negative()
    };
    let x_up = sign_of(&x) == Sign::Positive;
    Ok(if ef.is_zero() {
        sign_of(&x)
    } else if x_up && y_up {
        Sign::Positive
    } else if !x_up && !y_up {
        Sign::Negative
    } else {
        // X and Y pull apart, and the larger of X² and Y² decides, with X's sign.
        match (sign_of(&difference), x_up) {
            (Sign::Zero, _) => Sign::Zero,
            (Sign::Positive, true) | (Sign::Negative, false) => Sign::Positive,
            _ => Sign::Negative,
        }
    })
}

/// The sign of `v`.
fn sign_of(v: &Wide<14>) -> Sign {
    if v.is_zero() {
        Sign::Zero
    } else if v.is_negative() {
        Sign::Negative
    } else {
        Sign::Positive
    }
}

fn tilt_of_torus<R: Ring>(plane: &Plane, p: P3, q: P3, major: Steps, minor: Steps) -> Option<R> {
    let n = normal::<R>(plane)?;
    let e: [R; 3] = minus(q, p)?;
    let (big, small) = (size::<R>(major.0)?, size::<R>(minor.0)?);
    let (r2, s2) = (big.times(big)?, small.times(small)?);
    let tilt = inner(n, e)?;
    let spread = inner(n, n)?.times(inner(e, e)?)?.times(r2.minus(s2)?)?;
    spread.minus(tilt.times(tilt)?.times(r2)?)
}

/// How a plane leans to the axis p to q of a torus of radii `major` and `minor`: the sign of
/// |n|²|e|²(R² − r²) − (n·e)²R². Zero is Villarceau's angle, sin θ = r/R from the equator.
///
/// Budget: |n|² < 2^172, |e|² < 2^85 and R² < 2^106, so each side is below 2^364. `Wide<8>` holds it.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for a radius that does not fit an `i64`.
pub fn villarceau(
    plane: &Plane,
    p: P3,
    q: P3,
    major: Steps,
    minor: Steps,
) -> Result<Sign, NumError> {
    signed(tilt_of_torus::<Interval>(plane, p, q, major, minor), || {
        tilt_of_torus::<Wide<8>>(plane, p, q, major, minor)
    })
}

/// What [`polynomial`] computes in: exact integers, or intervals that enclose them.
pub trait Ring: Copy {
    /// The number `v`, for a |v| of at most 2^53.
    fn of(v: i64) -> Option<Self>;
    /// The sum.
    fn plus(self, other: Self) -> Option<Self>;
    /// The difference.
    fn minus(self, other: Self) -> Option<Self>;
    /// The product.
    fn times(self, other: Self) -> Option<Self>;
}

impl<const L: usize> Ring for Wide<L> {
    fn of(v: i64) -> Option<Self> {
        Some(Wide::from_i64(v))
    }
    fn plus(self, other: Self) -> Option<Self> {
        self.checked_add(&other)
    }
    fn minus(self, other: Self) -> Option<Self> {
        self.checked_sub(&other)
    }
    fn times(self, other: Self) -> Option<Self> {
        self.checked_mul(&other)
    }
}

impl Ring for Interval {
    #[allow(
        clippy::cast_precision_loss,
        reason = "the trait's values are at most 2^53 in magnitude, so the conversion is exact"
    )]
    fn of(v: i64) -> Option<Self> {
        Interval::point(v as f64).ok()
    }
    fn plus(self, other: Self) -> Option<Self> {
        self.add(other).ok()
    }
    fn minus(self, other: Self) -> Option<Self> {
        self.sub(other).ok()
    }
    fn times(self, other: Self) -> Option<Self> {
        self.mul(other).ok()
    }
}

/// `p − q`. Each difference is below 2^41 in size, so it fits an `i64` and [`Ring::of`].
pub(crate) fn minus<R: Ring>(p: P3, q: P3) -> Option<[R; 3]> {
    let one = |x: i64, y: i64| R::of(x.checked_sub(y)?);
    Some([
        one(p.x.get(), q.x.get())?,
        one(p.y.get(), q.y.get())?,
        one(p.z.get(), q.z.get())?,
    ])
}

/// `u · v`.
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the descriptions and of the module header's formulas"
)]
pub(crate) fn inner<R: Ring>(u: [R; 3], v: [R; 3]) -> Option<R> {
    let [a, b, c] = u;
    let [x, y, z] = v;
    a.times(x)?.plus(b.times(y)?)?.plus(c.times(z)?)
}

/// `u × v`.
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the descriptions and of the module header's formulas"
)]
fn outer<R: Ring>(u: [R; 3], v: [R; 3]) -> Option<[R; 3]> {
    let [a, b, c] = u;
    let [x, y, z] = v;
    Some([
        b.times(z)?.minus(c.times(y)?)?,
        c.times(x)?.minus(a.times(z)?)?,
        a.times(y)?.minus(b.times(x)?)?,
    ])
}

/// `v + v`.
fn twice<R: Ring>(v: R) -> Option<R> {
    v.plus(v)
}

/// The size `s` as a number of `R`, or `None` above 2^53.
pub(crate) fn size<R: Ring>(s: u64) -> Option<R> {
    R::of(i64::try_from(s).ok()?)
}

/// The polynomial in t, lowest coefficient first, whose roots are where a + t·d meets `surface`,
/// with its degree: 2 for a quadric and 4 for a torus. Either may have a zero leading coefficient.
/// `None` if a step did not fit, or a radius or slope passes 2^53.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the descriptions and of the module header's formulas"
)]
pub fn polynomial<R: Ring>(line: &Line, surface: &Surface) -> Option<([R; 5], usize)> {
    let d: [R; 3] = minus(line.b, line.a)?;
    let zero = R::of(0)?;
    match surface {
        Surface::Sphere(s) => {
            let w: [R; 3] = minus(line.a, s.centre)?;
            let r = size::<R>(s.radius.0)?;
            let low = inner(w, w)?.minus(r.times(r)?)?;
            Some(([low, twice(inner(d, w)?)?, inner(d, d)?, zero, zero], 2))
        }
        Surface::Cylinder(c) => {
            let e: [R; 3] = minus(c.along, c.base)?;
            let w: [R; 3] = minus(line.a, c.base)?;
            let (u, v) = (outer(d, e)?, outer(w, e)?);
            let r = size::<R>(c.radius.0)?;
            let low = inner(v, v)?.minus(r.times(r)?.times(inner(e, e)?)?)?;
            Some(([low, twice(inner(u, v)?)?, inner(u, u)?, zero, zero], 2))
        }
        Surface::Cone(c) => {
            let e: [R; 3] = minus(c.along, c.apex)?;
            let w: [R; 3] = minus(line.a, c.apex)?;
            let (u, v) = (outer(d, e)?, outer(w, e)?);
            let (alpha, beta) = (inner(d, e)?, inner(w, e)?);
            let s = size::<R>(c.slope.rise)?;
            let t = size::<R>(c.slope.run)?;
            let (ss, tt) = (s.times(s)?, t.times(t)?);
            let form = |x: R, y: R| tt.times(x)?.minus(ss.times(y)?);
            let low = form(inner(v, v)?, beta.times(beta)?)?;
            let mid = form(inner(u, v)?, alpha.times(beta)?)?;
            let top = form(inner(u, u)?, alpha.times(alpha)?)?;
            Some(([low, twice(mid)?, top, zero, zero], 2))
        }
        Surface::Torus(t) => {
            let e: [R; 3] = minus(t.along, t.centre)?;
            let w: [R; 3] = minus(line.a, t.centre)?;
            let (dd, dw, ww, ee) = (inner(d, d)?, inner(d, w)?, inner(w, w)?, inner(e, e)?);
            let (alpha, beta) = (inner(d, e)?, inner(w, e)?);
            let major = size::<R>(t.major.0)?;
            let minor = size::<R>(t.minor.0)?;
            let rr = major.times(major)?;
            let low = ww.plus(rr.minus(minor.times(minor)?)?)?;
            let four = twice(twice(rr)?)?;
            // E·P² − 4R²·M, with P = D·t² + 2H·t + (W + k) and M = E·|x|² − (x·e)².
            let scaled = |v: R| ee.times(v);
            let top = scaled(dd.times(dd)?)?;
            let third = scaled(twice(twice(dd.times(dw)?)?)?)?;
            let m2 = ee.times(dd)?.minus(alpha.times(alpha)?)?;
            let m1 = ee.times(dw)?.minus(alpha.times(beta)?)?;
            let m0 = ee.times(ww)?.minus(beta.times(beta)?)?;
            let second = scaled(twice(twice(dw.times(dw)?)?)?.plus(twice(dd.times(low)?)?)?)?
                .minus(four.times(m2)?)?;
            let first = scaled(twice(twice(dw.times(low)?)?)?)?.minus(twice(four.times(m1)?)?)?;
            let constant = scaled(low.times(low)?)?.minus(four.times(m0)?)?;
            Some(([constant, first, second, third, top], 4))
        }
    }
}

/// The polar form of `surface`'s equation: the symmetric bilinear form Q(x, y) with Q(x, x) zero
/// exactly on the surface. So a curve B(t) = Σ Bᵢ(t)·Pᵢ has Q(B(t), B(t)) = Σ Bᵢ·Bⱼ·Q(Pᵢ, Pⱼ), which is
/// what the Bernstein coefficients of a quadric over a Bézier piece are made of. `None` for a torus,
/// whose equation is quartic, or if a step did not fit.
///
/// Budget: for a cone, t²·((x − c) × e)·((y − c) × e) is below 2^80·3·2^166 < 2^248 and
/// s²·((x − c)·e)·((y − c)·e) below 2^80·(3·2^82)² < 2^248, so |Q| < 2^249; a sphere's and a
/// cylinder's are smaller. All fit `I320`.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the descriptions and of the module header's formulas"
)]
pub fn polar(surface: &Surface, x: P3, y: P3) -> Option<I320> {
    match surface {
        Surface::Sphere(s) => {
            let r = size::<I320>(s.radius.0)?;
            let (u, v) = (minus(x, s.centre)?, minus(y, s.centre)?);
            inner::<I320>(u, v)?.minus(r.times(r)?)
        }
        Surface::Cylinder(c) => {
            let e: [I320; 3] = minus(c.along, c.base)?;
            let (u, v) = (outer(minus(x, c.base)?, e)?, outer(minus(y, c.base)?, e)?);
            let r = size::<I320>(c.radius.0)?;
            inner(u, v)?.minus(r.times(r)?.times(inner(e, e)?)?)
        }
        Surface::Cone(c) => {
            let e: [I320; 3] = minus(c.along, c.apex)?;
            let (u, v): ([I320; 3], [I320; 3]) = (minus(x, c.apex)?, minus(y, c.apex)?);
            let s = size::<I320>(c.slope.rise)?;
            let t = size::<I320>(c.slope.run)?;
            let across = inner(outer(u, e)?, outer(v, e)?)?;
            let along = inner(u, e)?.times(inner(v, e)?)?;
            t.times(t)?.times(across)?.minus(s.times(s)?.times(along)?)
        }
        Surface::Torus(_) => None,
    }
}

/// `terms` as a chain's integers.
fn widened(terms: &[I320; 5]) -> Option<[Big; 5]> {
    let mut out = [Big::ZERO; 5];
    for (slot, c) in out.iter_mut().zip(terms) {
        *slot = c.resize()?;
    }
    Some(out)
}

/// The exact polynomial's chain and the interval polynomial with its degree.
fn exact_chain(line: &Line, surface: &Surface) -> Result<Chain, NumError> {
    let (terms, _) = polynomial::<I320>(line, surface).ok_or(NumError::BudgetExceeded)?;
    Chain::of(&widened(&terms).ok_or(NumError::BudgetExceeded)?)
}

/// The real roots of `line` with `surface`, each counted once, and whether any root is repeated,
/// through [`crate::sturm::filtered`] first.
///
/// # Errors
///
/// [`NumError::Degenerate`] if the polynomial is a constant: a line of one point, or a line
/// parallel to a cylinder's axis or lying along a cone's ruling. [`NumError::BudgetExceeded`] for a
/// radius of 2^41 or more.
pub fn roots(line: &Line, surface: &Surface) -> Result<Roots, NumError> {
    let enclosed = polynomial::<Interval>(line, surface);
    if let Some(answer) = enclosed.and_then(|(f, degree)| sturm::filtered(&f, degree)) {
        return Ok(answer);
    }
    Ok(exact_chain(line, surface)?.roots())
}

/// The real roots of `line` with `surface` strictly between the parameters `lo` and `hi`, each
/// counted once, through [`crate::sturm::filtered_between`] first.
///
/// # Errors
///
/// As [`roots`], and [`NumError::Degenerate`] unless `lo` is below `hi`.
pub fn roots_between(line: &Line, surface: &Surface, lo: Point, hi: Point) -> Result<u8, NumError> {
    let enclosed = polynomial::<Interval>(line, surface);
    if let Some(answer) =
        enclosed.and_then(|(f, degree)| sturm::filtered_between(&f, degree, lo, hi))
    {
        return Ok(answer);
    }
    exact_chain(line, surface)?.roots_between(lo, hi)
}
