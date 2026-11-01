//! Ellipses in 2D, exact: inside or outside, how a line meets one, and how two meet.
//!
//! An ellipse has its centre and major axis on the grid and its axis ratio a reduced fraction p / q
//! of bounded size (32.25), so its equation has whole-number coefficients:
//! p²(u · M)² + q²(u · M⊥)² = p²‖M‖⁴, with u the point less the centre and M the major axis. Each
//! decision about one is then a sign, and no ellipse is ever replaced by a polygon. That is what a
//! hatch boundary needs where a line ends on an ellipse: the end is exactly on both.
//!
//! What it does: with a line an ellipse meets at a one-root point, [`Crossing2::of_ellipse`]
//! ([`crate::one_root`]). With another conic it meets where a quartic has its roots: writing a point
//! of the ellipse by the tangent of half its parameter, t = tan(θ / 2), and putting that into the
//! other's equation gives a polynomial of degree four in t with whole-number coefficients, each of
//! whose real roots is one point of the ellipse, its multiplicity the order of contact there and its
//! place in t the point's place along the ellipse from θ = −π. So there is one quartic and no
//! second variable, and no two points can share a root. [`Ellipse2::meeting`] returns it and its
//! roots ([`crate::quartic`]); order along the other curve is the same question asked of it.
//!
//! Guarantees:
//!
//! - Every result is exact, or an error. A sign comes from a generated filter and exact path, or from
//!   [`crate::quartic`]; nothing is sampled, flattened or rounded.
//! - No root is lost, doubled or misnamed: a tangency is a root of even multiplicity, exactly, and
//!   the ellipse's own point θ = π is the root t = ∞.
//! - Within the budget. An ellipse's major axis is below 2^41 in each part, its q at most 2^21, and
//!   its quartic against a [`Conic`] of this module has coefficients within
//!   [`crate::quartic::COEFFICIENT_BITS`], asserted when the crate compiles.
//! - Deterministic: integers only, except the generated filters; no platform maths and no
//!   allocation.
//!
//! Method: the rational parametrisation of a conic by the tangent of half its angle, with chapter
//! 14.2's real-root isolation (Collins and Akritas 1976, <https://doi.org/10.1145/800205.806346>)
//! and exact common-root tests (`projects/engines/geometry/architecture/14-2d-drafting.md`, section
//! 14.2). The bound on q and the stored form are 32.25's
//! (`projects/engines/geometry/architecture/32-decisions.md`).
//!
//! Cost: constant time and no allocation; [`Ellipse2::meeting`] is a few dozen products and then
//! [`crate::quartic`]'s costs.

use crate::arc::{Circle2, Contact, RADIUS_BITS, Round};
use crate::carrier::{Carrier, Rational2, fits};
use crate::error::NumError;
use crate::generated;
use crate::grid::P2;
use crate::quartic::{Big, COEFFICIENT_BITS, Quartic, Roots};
use crate::sign::Sign;

/// An ellipse's q is at most 2 to this: 32.25's bound.
pub const RATIO_BITS: u32 = 21;

// The largest coefficient of an ellipse's quartic: against an ellipse, a form's coefficient, below
// 2^125, by a product of two of the point's numerators, each below 2^63 and with three terms, in
// three sums; against a round, below that, as its w is below 2^84 and its x and y below 2^126.
const _: () = assert!(125 + 128 + 2 < COEFFICIENT_BITS);

/// An ellipse: centre, major-axis vector and axis ratio p / q, 0 < p ≤ q, reduced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ellipse2 {
    centre: P2,
    major: [i64; 2],
    p: i64,
    q: i64,
}

/// The greatest common divisor, by Euclid's algorithm.
const fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let Some(r) = a.checked_rem(b) else {
            return 0;
        };
        a = b;
        b = r;
    }
    a
}

fn budget<T>(v: Option<T>) -> Result<T, NumError> {
    v.ok_or(NumError::BudgetExceeded)
}

fn mul(a: &Big, b: &Big) -> Result<Big, NumError> {
    budget(a.checked_mul(b))
}

fn add(a: &Big, b: &Big) -> Result<Big, NumError> {
    budget(a.checked_add(b))
}

fn sub(a: &Big, b: &Big) -> Result<Big, NumError> {
    budget(a.checked_sub(b))
}

fn big(v: i64) -> Big {
    Big::from_i64(v)
}

/// A polynomial in t of degree at most two, lowest first.
type Quadratic = [Big; 3];

/// The product of two quadratics.
fn product(a: &Quadratic, b: &Quadratic) -> Result<[Big; 5], NumError> {
    let ([a0, a1, a2], [b0, b1, b2]) = (a, b);
    Ok([
        mul(a0, b0)?,
        add(&mul(a0, b1)?, &mul(a1, b0)?)?,
        add(&add(&mul(a0, b2)?, &mul(a1, b1)?)?, &mul(a2, b0)?)?,
        add(&mul(a1, b2)?, &mul(a2, b1)?)?,
        mul(a2, b2)?,
    ])
}

/// `sum` with `factor` times `terms` added.
fn gather(sum: [Big; 5], factor: &Big, terms: &[Big; 5]) -> Result<[Big; 5], NumError> {
    let [s0, s1, s2, s3, s4] = sum;
    let [t0, t1, t2, t3, t4] = terms;
    let plus = |s: Big, t: &Big| add(&s, &mul(factor, t)?);
    Ok([
        plus(s0, t0)?,
        plus(s1, t1)?,
        plus(s2, t2)?,
        plus(s3, t3)?,
        plus(s4, t4)?,
    ])
}

impl Ellipse2 {
    /// The ellipse about `centre` with major-axis vector `major` and axis ratio p / q.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] for a part of `major` of 2^41 or more or a q past 2^21;
    /// [`NumError::Degenerate`] for a zero `major`, or a ratio not in (0, 1] or not reduced.
    pub fn new(centre: P2, major: [i64; 2], p: i64, q: i64) -> Result<Ellipse2, NumError> {
        let [mx, my] = major;
        let (wide_x, wide_y) = (i128::from(mx), i128::from(my));
        if !fits(wide_x, RADIUS_BITS) || !fits(wide_y, RADIUS_BITS) || q > 1 << RATIO_BITS {
            return Err(NumError::BudgetExceeded);
        }
        if (mx == 0 && my == 0) || p <= 0 || p > q || gcd(p, q) != 1 {
            return Err(NumError::Degenerate);
        }
        Ok(Ellipse2 {
            centre,
            major,
            p,
            q,
        })
    }

    /// The circle of `c` as an ellipse: its major axis along x, its ratio 1.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if its radius, which [`Circle2::new`] bounded, did not fit.
    pub fn of_circle(c: &Circle2) -> Result<Ellipse2, NumError> {
        let radius = budget(i64::try_from(c.radius()).ok())?;
        Ellipse2::new(c.centre(), [radius, 0], 1, 1)
    }

    /// Its centre.
    #[must_use]
    pub const fn centre(&self) -> P2 {
        self.centre
    }

    /// Its major-axis vector.
    #[must_use]
    pub const fn major(&self) -> [i64; 2] {
        self.major
    }

    /// Its axis ratio: p and q.
    #[must_use]
    pub const fn ratio(&self) -> (i64, i64) {
        (self.p, self.q)
    }

    /// Where the rational point `z` lies: [`Sign::Negative`] inside, [`Sign::Zero`] on it,
    /// [`Sign::Positive`] outside.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step of the exact path did not fit, which no input in the
    /// budget can reach.
    pub fn side(&self, z: &Rational2) -> Result<Sign, NumError> {
        let ([x, y], w) = z.coordinates();
        let k = self.centre;
        let [mx, my] = self.major.map(i128::from);
        let (p, q) = (i128::from(self.p), i128::from(self.q));
        generated::ellipse_side::filter(x, y, w, k, mx, my, p, q)
            .or_else(|| generated::ellipse_side::exact(x, y, w, k, mx, my, p, q))
            .ok_or(NumError::BudgetExceeded)
    }

    /// How the carrier `l` meets it: the sign of the discriminant of where they meet, zero exactly
    /// at a tangency.
    ///
    /// # Errors
    ///
    /// As for [`Ellipse2::side`].
    pub fn meets(&self, l: &Carrier) -> Result<Contact, NumError> {
        let ([a, b], d) = l.coefficients();
        let k = self.centre;
        let [mx, my] = self.major.map(i128::from);
        let (p, q) = (i128::from(self.p), i128::from(self.q));
        generated::ellipse_line::filter(a, b, d, k, mx, my, p, q)
            .or_else(|| generated::ellipse_line::exact(a, b, d, k, mx, my, p, q))
            .map(|sign| match sign {
                Sign::Positive => Contact::Crossing,
                Sign::Zero => Contact::Tangent,
                Sign::Negative => Contact::Apart,
            })
            .ok_or(NumError::BudgetExceeded)
    }

    /// The quadratic form of its equation: A, B and C of A x² + B x y + C y², about its centre.
    pub(crate) fn form(&self) -> Result<[Big; 3], NumError> {
        let [mx, my] = self.major.map(big);
        let (p2, q2) = (
            mul(&big(self.p), &big(self.p))?,
            mul(&big(self.q), &big(self.q))?,
        );
        let (xx, yy) = (mul(&mx, &mx)?, mul(&my, &my)?);
        let xy = mul(&mx, &my)?;
        let shear = sub(&p2, &q2)?;
        Ok([
            add(&mul(&p2, &xx)?, &mul(&q2, &yy)?)?,
            mul(&add(&xy, &xy)?, &shear)?,
            add(&mul(&p2, &yy)?, &mul(&q2, &xx)?)?,
        ])
    }

    /// p² · L², of the equation's right side: L = ‖M‖².
    pub(crate) fn level(&self) -> Result<Big, NumError> {
        let [mx, my] = self.major.map(big);
        let length = add(&mul(&mx, &mx)?, &mul(&my, &my)?)?;
        let scaled = mul(&big(self.p), &length)?;
        mul(&scaled, &scaled)
    }

    /// Its point at t, as numerators over q(1 + t²), about `origin`: each a quadratic in t.
    fn numerators(&self, origin: P2) -> Result<([Quadratic; 2], Quadratic), NumError> {
        let [mx, my] = self.major.map(big);
        let (p, q) = (big(self.p), big(self.q));
        let dx = sub(&big(self.centre.x.get()), &big(origin.x.get()))?;
        let dy = sub(&big(self.centre.y.get()), &big(origin.y.get()))?;
        let twice_p = add(&p, &p)?;
        let x = [
            mul(&q, &add(&dx, &mx)?)?,
            sub(&big(0), &mul(&twice_p, &my)?)?,
            mul(&q, &sub(&dx, &mx)?)?,
        ];
        let y = [
            mul(&q, &add(&dy, &my)?)?,
            mul(&twice_p, &mx)?,
            mul(&q, &sub(&dy, &my)?)?,
        ];
        Ok(([x, y], [q, big(0), q]))
    }

    /// `other`'s equation along this ellipse: a polynomial in t = tan(θ / 2) of degree at most four.
    ///
    /// # Errors
    ///
    /// As for [`Ellipse2::side`].
    pub fn quartic(&self, other: &Conic) -> Result<Quartic, NumError> {
        let ([x, y], d) = self.numerators(other.origin)?;
        let Conic {
            xx,
            xy,
            yy,
            x: lx,
            y: ly,
            constant,
            ..
        } = other;
        let zero = big(0);
        let mut sum = [zero; 5];
        sum = gather(sum, xx, &product(&x, &x)?)?;
        sum = gather(sum, xy, &product(&x, &y)?)?;
        sum = gather(sum, yy, &product(&y, &y)?)?;
        sum = gather(sum, lx, &product(&x, &d)?)?;
        sum = gather(sum, ly, &product(&y, &d)?)?;
        sum = gather(sum, constant, &product(&d, &d)?)?;
        Ok(Quartic::new(sum))
    }

    /// How `other` meets this ellipse. [`Meeting::Same`] when every point of it is on `other`;
    /// otherwise the quartic and its roots, in order along this ellipse from θ = −π, each with its
    /// order of contact: even at a tangency, and no roots when they do not meet.
    ///
    /// # Errors
    ///
    /// As for [`Ellipse2::side`].
    pub fn meeting(&self, other: &Conic) -> Result<Meeting, NumError> {
        let quartic = self.quartic(other)?;
        if quartic.is_zero() {
            return Ok(Meeting::Same);
        }
        let roots = quartic.roots()?;
        Ok(Meeting::At { quartic, roots })
    }
}

/// How a conic meets an ellipse.
#[derive(Clone, Copy, Debug)]
pub enum Meeting {
    /// One curve.
    Same,
    /// At these roots of this quartic in the ellipse's t.
    At {
        /// The other's equation along the ellipse.
        quartic: Quartic,
        /// Its real roots, increasing, with their multiplicities.
        roots: Roots,
    },
}

/// A conic's equation with whole-number coefficients, about a grid point:
/// xx · x² + xy · x y + yy · y² + x · x + y · y + constant = 0.
#[derive(Clone, Copy, Debug)]
pub struct Conic {
    origin: P2,
    xx: Big,
    xy: Big,
    yy: Big,
    x: Big,
    y: Big,
    constant: Big,
}

impl Conic {
    /// An ellipse's equation, about its centre.
    ///
    /// # Errors
    ///
    /// As for [`Ellipse2::side`].
    pub fn of_ellipse(e: &Ellipse2) -> Result<Conic, NumError> {
        let [xx, xy, yy] = e.form()?;
        Ok(Conic {
            origin: e.centre,
            xx,
            xy,
            yy,
            x: big(0),
            y: big(0),
            constant: sub(&big(0), &e.level()?)?,
        })
    }

    /// A carrier's equation, about the origin: a · x + b · y − c = 0. Along an ellipse it is the
    /// carrier's two roots times 1 + t², which has none, so a line takes its place in order among
    /// the other curves that meet the ellipse.
    ///
    /// # Errors
    ///
    /// As for [`Ellipse2::side`].
    pub fn of_carrier(l: &Carrier) -> Result<Conic, NumError> {
        let ([a, b], c) = l.coefficients();
        Ok(Conic {
            origin: P2::new(0, 0)?,
            xx: big(0),
            xy: big(0),
            yy: big(0),
            x: Big::from_i128(a),
            y: Big::from_i128(b),
            constant: sub(&big(0), &Big::from_i128(c))?,
        })
    }

    /// A circle's equation, about its centre.
    ///
    /// # Errors
    ///
    /// As for [`Ellipse2::side`].
    pub fn of_circle(c: &Circle2) -> Result<Conic, NumError> {
        let r = Big::from_i128(c.radius());
        Ok(Conic {
            origin: c.centre(),
            xx: big(1),
            xy: big(0),
            yy: big(1),
            x: big(0),
            y: big(0),
            constant: sub(&big(0), &mul(&r, &r)?)?,
        })
    }

    /// The equation of a circle about a rational point through a grid point, about that point:
    /// w(x² + y²) − 2d · (x, y) = 0, with d the centre's numerators there.
    ///
    /// # Errors
    ///
    /// As for [`Ellipse2::side`].
    pub fn of_round(round: &Round) -> Result<Conic, NumError> {
        let on = round.on();
        let ([cx, cy], w) = round.centre().coordinates();
        let (w, cx, cy) = (Big::from_i128(w), Big::from_i128(cx), Big::from_i128(cy));
        let dx = sub(&cx, &mul(&w, &big(on.x.get()))?)?;
        let dy = sub(&cy, &mul(&w, &big(on.y.get()))?)?;
        let turned = |v: &Big| sub(&big(0), &add(v, v)?);
        Ok(Conic {
            origin: on,
            xx: w,
            xy: big(0),
            yy: w,
            x: turned(&dx)?,
            y: turned(&dy)?,
            constant: big(0),
        })
    }
}
