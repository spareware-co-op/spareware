//! Carriers: the whole unbounded curves 2D geometry lies on, and the table that names them.
//!
//! What it does: a carrier is the whole curve under an entity, the infinite line under a segment
//! or the full circle under an arc (chapter 14.2). [`Curve2`] is the closed list of the kinds this
//! layer decides on exactly: a [`Line2`], a [`Circle2`] and an [`Ellipse2`], each the form
//! `geom-num` holds, so that this crate adds no number type of its own (principle 8). A point made
//! from carriers names them by their [`CurveIx`] in a [`Carriers`] table.
//!
//! Guarantees:
//!
//! - **Budgets are geom-num's.** A [`Line2`] is a [`Carrier`] and stays within 15.10's budget; a
//!   constructed line keeps it, because a rational direction is the normal of a [`Carrier`] and an
//!   offset by whole steps only moves c. A circle is a grid centre with a whole radius, or a
//!   rational centre through a grid point; an ellipse is 32.25's. So every predicate on them has
//!   the fixed width its module derives, and nothing here widens one.
//! - **Exact.** [`Curve2::side`] is the sign of a predicate of `geom-num`, zero exactly on the curve.
//! - **Stable names.** A [`CurveIx`] is a place in the table, which only grows, and never a memory
//!   address.
//!
//! Method: the carriers and point classes of chapter 14.2
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`) and the integer forms of 15.10
//! (`projects/engines/geometry/architecture/15-sketches.md`), adopted as owner actions G185 and G186
//! (`projects/engines/geometry/architecture/32-decisions.md`, sections 32.25 and 32.26).
//!
//! Cost: constant time per question, as `geom-num`'s predicates are; the table is a `Vec`, one
//! `push` for each carrier.

use alloc::vec::Vec;

use geom_num::arc::{Circle2 as GridCircle, Round};
use geom_num::carrier::{Carrier, Rational2};
use geom_num::conic::{Conic, Ellipse2};
use geom_num::{NumError, Sign};

/// A line: a · x + b · y = c, whole numbers within 15.10's budget.
pub type Line2 = Carrier;

/// A circle with an exact squared radius.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Circle2 {
    /// A drawn or imported circle: a grid centre and a whole number of steps.
    Grid(GridCircle),
    /// A constructed one: a rational centre, through a grid point.
    Round(Round),
}

impl Circle2 {
    /// Where the rational point `z` lies: [`Sign::Negative`] inside, [`Sign::Zero`] on it,
    /// [`Sign::Positive`] outside.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step of the exact path did not fit, which no input in the
    /// budget can reach.
    pub fn side(&self, z: &Rational2) -> Result<Sign, NumError> {
        match self {
            Circle2::Grid(c) => c.side(z),
            Circle2::Round(c) => c.side(z),
        }
    }

    /// Its equation.
    ///
    /// # Errors
    ///
    /// As for [`Circle2::side`].
    pub fn conic(&self) -> Result<Conic, NumError> {
        match self {
            Circle2::Grid(c) => Conic::of_circle(c),
            Circle2::Round(c) => Conic::of_round(c),
        }
    }
}

/// A carrier: the whole curve an entity lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Curve2 {
    /// A line.
    Line(Line2),
    /// A circle.
    Circle(Circle2),
    /// An ellipse.
    Ellipse(Ellipse2),
}

impl Curve2 {
    /// Where the rational point `z` lies against it: [`Sign::Zero`] exactly on it; off a line, which
    /// side, as [`Carrier::side`] says; off a closed curve, [`Sign::Negative`] inside and
    /// [`Sign::Positive`] outside.
    ///
    /// # Errors
    ///
    /// As for [`Circle2::side`].
    pub fn side(&self, z: &Rational2) -> Result<Sign, NumError> {
        match self {
            Curve2::Line(l) => l.side(z),
            Curve2::Circle(c) => c.side(z),
            Curve2::Ellipse(e) => e.side(z),
        }
    }

    /// Its equation, as a conic.
    ///
    /// # Errors
    ///
    /// As for [`Circle2::side`].
    pub fn conic(&self) -> Result<Conic, NumError> {
        match self {
            Curve2::Line(l) => Conic::of_carrier(l),
            Curve2::Circle(c) => c.conic(),
            Curve2::Ellipse(e) => Conic::of_ellipse(e),
        }
    }
}

/// A carrier's place in a [`Carriers`] table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CurveIx(u32);

impl CurveIx {
    /// Its place.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// The carriers of a drawing, each named by its place.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Carriers {
    curves: Vec<Curve2>,
}

impl Carriers {
    /// No carriers.
    #[must_use]
    pub const fn new() -> Carriers {
        Carriers { curves: Vec::new() }
    }

    /// Adds `curve` and says where it is.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] past 2^32 carriers, which a [`CurveIx`] cannot name.
    pub fn push(&mut self, curve: Curve2) -> Result<CurveIx, NumError> {
        let at = u32::try_from(self.curves.len()).map_err(|_| NumError::BudgetExceeded)?;
        self.curves.push(curve);
        Ok(CurveIx(at))
    }

    /// The carrier at `at`, if the table has one there.
    #[must_use]
    pub fn get(&self, at: CurveIx) -> Option<&Curve2> {
        usize::try_from(at.0).ok().and_then(|k| self.curves.get(k))
    }

    /// How many carriers it holds.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.curves.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.curves.is_empty()
    }
}
