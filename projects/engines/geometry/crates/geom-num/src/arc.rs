//! Circles and arcs in 2D, exact: inside or outside, tangent or crossing, and order along an arc.
//!
//! An arc's ends are grid points and its centre is a rational point on their perpendicular bisector
//! (chapter 15.10), so both ends lie on its circle exactly and a line joined to an arc shares its end
//! exactly. A circle drawn or imported has a grid centre and, when its radius drives it, a whole
//! number of steps as its radius. Each is a sign here, of a polynomial in whole numbers, so no square
//! root is ever taken and no tolerance decides a tangency: it is a discriminant that is exactly zero
//! (chapter 14.2). What one such point of contact is, as a number, is G189's.
//!
//! Guarantees:
//!
//! - Every result is exact, or an error. A [`Sign`] is decided by integer arithmetic alone, after a
//!   generated `f64` filter whose answers are the exact sign or no answer (chapter 6.2).
//! - Nothing overflows silently. A [`Round`], an [`Arc2`] and a [`Circle2`] can only be made within
//!   the budget of [`crate::carrier`], and every width below is what the generated certificate
//!   derives for every input in that budget, tested at the extreme values.
//!
//!   | Question | As a polynomial | Width with its sign |
//!   | --- | --- | --- |
//!   | Grid point against a [`Round`] | (Z − P) · (W(Z + P) − 2C) | 169 bits |
//!   | Rational point against a [`Round`] | (Z − vP) · (w(Z + vP) − 2vC) | 336 bits |
//!   | Carrier against a [`Round`] | h² − ((wP − C)²)(a² + b²), h = a · C − c · w | 336 bits |
//!   | Rational point against a [`Circle2`] | (Z − vC)² − r²v² | 253 bits |
//!   | Carrier against a [`Circle2`] | h² − r²(a² + b²), h = a · C − c | 168 bits |
//!   | Two [`Circle2`]s | d² − t², t the sum or difference of their radii | 85 bits |
//!
//!   Chapter 15.10 writes 170, 337 and 253 bits for the first, second and fourth, and adds the side
//!   of an arc's chord for its centre, 169 bits, which [`crate::carrier::Carrier::side`] answers.
//! - Which way an arc runs is fixed: counter-clockwise from its start to its end, about its centre.
//!   A point of the circle is on the arc exactly when it is not to the left of the chord from start
//!   to end; points of the arc come in the order they turn from its start.
//! - Deterministic: integers only, except the generated filters; no platform maths and no
//!   allocation.
//!
//! Method: chapter 15.10's forms, the power of a point (Steiner, 1826) written as a difference of two
//! squares so that a grid point costs no squares; three points of a circle in cyclic order turn
//! counter-clockwise, which orders an arc by [`Rational2::orient`] with no angle; a line's distance
//! from a centre against the radius, squared, as the discriminant of the quadratic where they meet
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`, section 14.2). CGAL's circle-segment
//! traits have the same model; its papers only, as 14.2 says, its source not read (Fogel, Halperin
//! and Wein 2012, <https://doi.org/10.1007/978-3-642-17283-0>).
//!
//! Cost: constant time and no allocation: a generated filter, then at worst an exact path of a few
//! dozen fixed-width operations.

use crate::carrier::{Carrier, Rational2, fits, product_sum};
use crate::error::NumError;
use crate::generated;
use crate::grid::P2;
use crate::sign::Sign;

/// A radius is below 2 to this in magnitude, the difference of two grid coordinates.
pub const RADIUS_BITS: u32 = crate::carrier::NORMAL_BITS;

/// How a line or a circle meets a circle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Contact {
    /// They have no point in common.
    Apart,
    /// They touch at one point, the discriminant being exactly zero.
    Tangent,
    /// They cross at two points.
    Crossing,
}

impl Contact {
    /// The contact a sign of distance² − radius² means.
    const fn of(sign: Sign) -> Contact {
        match sign {
            Sign::Positive => Contact::Apart,
            Sign::Zero => Contact::Tangent,
            Sign::Negative => Contact::Crossing,
        }
    }
}

/// How two circles lie.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pair {
    /// Each outside the other.
    Apart,
    /// Touching from outside.
    TouchingOutside,
    /// Crossing at two points.
    Crossing,
    /// One inside the other, touching.
    TouchingInside,
    /// One inside the other, not touching.
    Nested,
    /// One circle.
    Same,
}

/// The circle about a rational centre through a grid point: 15.10's defining point, and the circle
/// under every [`Arc2`]. Its squared radius is exact, ‖on − centre‖².
#[derive(Clone, Copy, Debug)]
pub struct Round {
    on: P2,
    centre: Rational2,
}

impl Round {
    /// The circle about `centre` through `on`.
    #[must_use]
    pub const fn new(on: P2, centre: Rational2) -> Round {
        Round { on, centre }
    }

    /// The grid point it was made through.
    #[must_use]
    pub const fn on(&self) -> P2 {
        self.on
    }

    /// Its centre.
    #[must_use]
    pub const fn centre(&self) -> Rational2 {
        self.centre
    }

    /// Where the grid point `z` lies: [`Sign::Negative`] inside, [`Sign::Zero`] on it,
    /// [`Sign::Positive`] outside. The sign of ‖z − C‖² − ‖P − C‖² = (z − P) · (z + P − 2C), which
    /// has no squares of C and so stays narrow.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step of the exact path did not fit, which no input in the
    /// budget can reach.
    pub fn side_of_grid(&self, z: P2) -> Result<Sign, NumError> {
        let p = self.on;
        let ([cx, cy], w) = self.centre.coordinates();
        generated::round_grid::filter(z, p, cx, cy, w)
            .or_else(|| generated::round_grid::exact(z, p, cx, cy, w))
            .ok_or(NumError::BudgetExceeded)
    }

    /// Where the rational point `z` lies, as [`Round::side_of_grid`] says. Its weight is positive, so
    /// the value multiplied through by it and the centre's keeps its sign.
    ///
    /// # Errors
    ///
    /// As for [`Round::side_of_grid`].
    pub fn side(&self, z: &Rational2) -> Result<Sign, NumError> {
        let p = self.on;
        let ([x, y], v) = z.coordinates();
        let ([cx, cy], w) = self.centre.coordinates();
        generated::round_rational::filter(x, y, v, p, cx, cy, w)
            .or_else(|| generated::round_rational::exact(x, y, v, p, cx, cy, w))
            .ok_or(NumError::BudgetExceeded)
    }

    /// How the carrier `l` meets the circle: the sign of its distance from the centre squared, less
    /// the radius squared, all in whole numbers, which is the discriminant of where they meet.
    ///
    /// # Errors
    ///
    /// As for [`Round::side_of_grid`].
    pub fn meets(&self, l: &Carrier) -> Result<Contact, NumError> {
        let p = self.on;
        let ([a, b], c) = l.coefficients();
        let ([cx, cy], w) = self.centre.coordinates();
        generated::round_line::filter(a, b, c, p, cx, cy, w)
            .or_else(|| generated::round_line::exact(a, b, c, p, cx, cy, w))
            .map(Contact::of)
            .ok_or(NumError::BudgetExceeded)
    }
}

/// The part of a [`Round`] running counter-clockwise from a grid point to another.
#[derive(Clone, Copy, Debug)]
pub struct Arc2 {
    round: Round,
    end: P2,
}

impl Arc2 {
    /// The arc running counter-clockwise about `centre` from `start` to `end`.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] when `start` and `end` are one point, or `centre` is not on their
    /// perpendicular bisector, so that no circle about it holds both.
    pub fn new(start: P2, end: P2, centre: Rational2) -> Result<Arc2, NumError> {
        if start == end {
            return Err(NumError::Degenerate);
        }
        let round = Round::new(start, centre);
        if round.side_of_grid(end)? != Sign::Zero {
            return Err(NumError::Degenerate);
        }
        Ok(Arc2 { round, end })
    }

    /// The perpendicular bisector of `p` and `q`, 2(q − p) · x = ‖q‖² − ‖p‖², halved when it can be,
    /// where 15.10 puts every arc's centre.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] when `p` and `q` are one point; [`NumError::BudgetExceeded`] for a
    /// chord so long and so placed that 2(q − p) stays over a carrier's budget.
    pub fn bisector(p: P2, q: P2) -> Result<Carrier, NumError> {
        let (px, py) = (i128::from(p.x.get()), i128::from(p.y.get()));
        let (qx, qy) = (i128::from(q.x.get()), i128::from(q.y.get()));
        let dx = qx.checked_sub(px).ok_or(NumError::BudgetExceeded)?;
        let dy = qy.checked_sub(py).ok_or(NumError::BudgetExceeded)?;
        let c = product_sum(qx, qx, qy, qy)?
            .checked_sub(product_sum(px, px, py, py)?)
            .ok_or(NumError::BudgetExceeded)?;
        let doubled = |v: i128| v.checked_mul(2).ok_or(NumError::BudgetExceeded);
        if c.rem_euclid(2) == 0 {
            Carrier::new(dx, dy, c.div_euclid(2))
        } else {
            Carrier::new(doubled(dx)?, doubled(dy)?, c)
        }
    }

    /// The circle it lies on.
    #[must_use]
    pub const fn round(&self) -> Round {
        self.round
    }

    /// Where it starts.
    #[must_use]
    pub const fn start(&self) -> P2 {
        self.round.on
    }

    /// Where it ends.
    #[must_use]
    pub const fn end(&self) -> P2 {
        self.end
    }

    /// Whether the point `z` of its circle lies on the arc, its ends included: the arc is the part
    /// of the circle not to the left of the chord from start to end.
    ///
    /// # Errors
    ///
    /// As for [`Round::side_of_grid`].
    pub fn holds(&self, z: &Rational2) -> Result<bool, NumError> {
        let chord = Carrier::through(self.round.on, self.end)?;
        Ok(chord.side(z)? != Sign::Positive)
    }

    /// Which of two points of the arc comes first from its start: [`Sign::Positive`] when `a` comes
    /// before `b`, [`Sign::Zero`] when they are one point or either is the start. Points of a circle
    /// in the order they run counter-clockwise always turn counter-clockwise, so this is the turn of
    /// the start, `a` and `b`, 335 bits, and it needs no angle and no centre.
    ///
    /// # Errors
    ///
    /// As for [`Round::side_of_grid`].
    pub fn order(&self, a: &Rational2, b: &Rational2) -> Result<Sign, NumError> {
        Rational2::orient(&self.round.on.into(), a, b)
    }
}

/// A circle about a grid point with a whole number of steps as its radius.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Circle2 {
    centre: P2,
    radius: i128,
}

impl Circle2 {
    /// The circle about `centre` of radius `radius`.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] for a radius of 2^41 steps or more, and [`NumError::Degenerate`]
    /// for one that is not positive.
    pub const fn new(centre: P2, radius: i128) -> Result<Circle2, NumError> {
        if !fits(radius, RADIUS_BITS) {
            return Err(NumError::BudgetExceeded);
        }
        if radius <= 0 {
            return Err(NumError::Degenerate);
        }
        Ok(Circle2 { centre, radius })
    }

    /// Its centre.
    #[must_use]
    pub const fn centre(&self) -> P2 {
        self.centre
    }

    /// Its radius in steps.
    #[must_use]
    pub const fn radius(&self) -> i128 {
        self.radius
    }

    /// Where the rational point `z` lies: [`Sign::Negative`] inside, [`Sign::Zero`] on it,
    /// [`Sign::Positive`] outside.
    ///
    /// # Errors
    ///
    /// As for [`Round::side_of_grid`].
    pub fn side(&self, z: &Rational2) -> Result<Sign, NumError> {
        let ([x, y], v) = z.coordinates();
        generated::circle_side::filter(x, y, v, self.centre, self.radius)
            .or_else(|| generated::circle_side::exact(x, y, v, self.centre, self.radius))
            .ok_or(NumError::BudgetExceeded)
    }

    /// How the carrier `l` meets the circle, as [`Round::meets`] says.
    ///
    /// # Errors
    ///
    /// As for [`Round::side_of_grid`].
    pub fn meets(&self, l: &Carrier) -> Result<Contact, NumError> {
        let ([a, b], c) = l.coefficients();
        generated::circle_line::filter(a, b, c, self.centre, self.radius)
            .or_else(|| generated::circle_line::exact(a, b, c, self.centre, self.radius))
            .map(Contact::of)
            .ok_or(NumError::BudgetExceeded)
    }

    /// The sign of the squared distance between the centres, less `t` squared.
    fn gap(&self, other: &Circle2, t: i128) -> Result<Sign, NumError> {
        let (a, b) = (self.centre, other.centre);
        generated::circle_gap::filter(a, b, t)
            .or_else(|| generated::circle_gap::exact(a, b, t))
            .ok_or(NumError::BudgetExceeded)
    }

    /// How this circle and `other` lie, from the distance between their centres against the sum and
    /// the difference of their radii, squared, so no root is taken.
    ///
    /// # Errors
    ///
    /// As for [`Round::side_of_grid`].
    pub fn against(&self, other: &Circle2) -> Result<Pair, NumError> {
        let sum = self.radius.checked_add(other.radius);
        let difference = self.radius.checked_sub(other.radius);
        let (Some(sum), Some(difference)) = (sum, difference) else {
            return Err(NumError::BudgetExceeded);
        };
        Ok(
            match (self.gap(other, sum)?, self.gap(other, difference)?) {
                (Sign::Positive, _) => Pair::Apart,
                (Sign::Zero, _) => Pair::TouchingOutside,
                (_, Sign::Positive) => Pair::Crossing,
                (_, Sign::Zero) if difference == 0 => Pair::Same,
                (_, Sign::Zero) => Pair::TouchingInside,
                (_, Sign::Negative) => Pair::Nested,
            },
        )
    }
}
