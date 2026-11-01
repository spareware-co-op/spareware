//! Points in 2D: grid points, fractions, and points stored as their definitions.
//!
//! What it does: a [`Pt2`] is one of chapter 14.2's four classes. A grid point is whole steps and a
//! rational point homogeneous integers. The other two are implicit: where two carriers cross, stored
//! as the carriers and which crossing, or where an ellipse meets another conic, stored as the two
//! and which root. [`Pt2::place`] reads a point's definition into exact numbers each time it is
//! asked, and every question is asked of those: a rounded copy is never kept, so nothing decided
//! here can differ from what the definition says (chapter 6.3).
//!
//! Guarantees:
//!
//! - **Exact.** [`Pt2::on`] is `true` exactly when the point is on the curve: an exact sign, or for a
//!   root of a quartic an exact test for a common root.
//! - **Closed.** Every carrier has rational coefficients, so a crossing of a line or circle with a
//!   circle, or of a line with an ellipse, is a one-root point, and every budget is
//!   [`geom_num::one_root`]'s; an ellipse with a circle or another ellipse is a conic root, within
//!   [`geom_num::quartic`]'s. No line is built through a one-root point (14.2).
//! - **Typed failure.** Curves that do not meet, a name the table does not hold and a pair of kinds
//!   a point class does not cover are each a [`PointError`], never a guess.
//!
//! Method: the point classes and implicit points of chapter 14.2
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`) and 6.3
//! (`projects/engines/geometry/architecture/06-numbers.md`).
//!
//! Cost: reading a one-root point is one `Crossing2` construction, a few dozen products; reading a
//! conic root is a quartic and its roots, as [`geom_num::quartic`] says. No allocation.

use core::fmt;

use geom_num::carrier::Rational2;
use geom_num::conic::{Conic, Ellipse2, Meeting};
use geom_num::grid::P2;
use geom_num::one_root::{Crossing2, Root};
use geom_num::quartic::{Quartic, RealRoot, compare};
use geom_num::{NumError, Sign};
use geom_op::{NumDetail, OpErrorDetail};

use crate::accept::Accept;
use crate::carrier::{Carriers, Circle2, Curve2, CurveIx};

/// Why a point could not be read, or a result could not be given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointError {
    /// A step of the numbers layer failed: curves that do not meet are
    /// [`NumError::Degenerate`].
    Num(NumError),
    /// The table holds no carrier at this place.
    NoCarrier(CurveIx),
    /// A crossing of these two kinds of curve is not this class of point: an ellipse with a circle
    /// or an ellipse is a conic root, and a conic root's first carrier is an ellipse.
    WrongClass,
    /// The conic has fewer real roots along the ellipse than this one names.
    NoSuchRoot(u8),
    /// The result would be less exact than the caller accepted: what it would reach, and what was
    /// accepted.
    ExactnessNotAccepted {
        /// What the result would be.
        would_reach: geom_num::Exactness,
        /// What the caller accepted.
        accepted: Accept,
    },
}

impl From<NumError> for PointError {
    fn from(e: NumError) -> PointError {
        PointError::Num(e)
    }
}

impl fmt::Display for PointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PointError::Num(e) => write!(f, "{e}"),
            PointError::NoCarrier(at) => write!(f, "no carrier at {}", at.get()),
            PointError::WrongClass => f.write_str("these curves make another class of point"),
            PointError::NoSuchRoot(k) => write!(f, "no real root at place {k}"),
            PointError::ExactnessNotAccepted { .. } => {
                f.write_str("the result would be less exact than the caller accepted")
            }
        }
    }
}

impl core::error::Error for PointError {}

impl From<PointError> for OpErrorDetail {
    /// What an operation reports: chapter 22's kind by meaning. A name or a class that does not fit
    /// is a degenerate input.
    fn from(e: PointError) -> OpErrorDetail {
        match e {
            PointError::Num(e) => e.into(),
            PointError::ExactnessNotAccepted { .. } => NumDetail::BoundOverCeiling.into(),
            PointError::NoCarrier(_) | PointError::WrongClass | PointError::NoSuchRoot(_) => {
                NumError::Degenerate.into()
            }
        }
    }
}

/// A point of the plane, as chapter 14.2 stores it. Two of these are one place or not by what they
/// say, which takes the exact questions below, so they have no `==`.
#[derive(Clone, Copy, Debug)]
pub enum Pt2 {
    /// Whole grid steps.
    Grid(P2),
    /// Fractions of bounded size.
    Rational(Rational2),
    /// Where two carriers cross: `root` says which crossing, by their order along `a`, or for two
    /// circles which side of the way from `a`'s centre to `b`'s. Two lines have one crossing, and
    /// `root` says nothing.
    Crossing {
        /// The first carrier.
        a: CurveIx,
        /// The second.
        b: CurveIx,
        /// Which crossing.
        root: Root,
    },
    /// Where an ellipse meets another conic.
    ConicRoot {
        /// The ellipse.
        a: CurveIx,
        /// The other conic.
        b: CurveIx,
        /// Which root, by order along the ellipse from θ = −π.
        k: u8,
    },
}

/// Where a point is, exactly.
#[derive(Clone, Copy, Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "a place is read and dropped at once, never stored, and the crate does not allocate"
)]
pub enum Place {
    /// A grid, rational or one-root point.
    OneRoot(Crossing2),
    /// A root of the quartic of a conic along an ellipse.
    Conic {
        /// The ellipse.
        ellipse: Ellipse2,
        /// The other conic's equation along it.
        quartic: Quartic,
        /// The root.
        root: RealRoot,
    },
}

/// The carrier at `at`.
fn curve(carriers: &Carriers, at: CurveIx) -> Result<&Curve2, PointError> {
    carriers.get(at).ok_or(PointError::NoCarrier(at))
}

/// Where the carriers `a` and `b` cross.
fn crossing(a: &Curve2, b: &Curve2, root: Root) -> Result<Crossing2, PointError> {
    let flipped = match root {
        Root::First => Root::Second,
        Root::Second => Root::First,
    };
    Ok(match (a, b) {
        (Curve2::Line(l), Curve2::Line(m)) => Crossing2::of_point(&l.meet(m)?)?,
        (Curve2::Line(l), Curve2::Circle(Circle2::Grid(c)))
        | (Curve2::Circle(Circle2::Grid(c)), Curve2::Line(l)) => Crossing2::of_circle(l, c, root)?,
        (Curve2::Line(l), Curve2::Circle(Circle2::Round(c)))
        | (Curve2::Circle(Circle2::Round(c)), Curve2::Line(l)) => Crossing2::of_round(l, c, root)?,
        (Curve2::Line(l), Curve2::Ellipse(e)) | (Curve2::Ellipse(e), Curve2::Line(l)) => {
            Crossing2::of_ellipse(l, e, root)?
        }
        (Curve2::Circle(Circle2::Grid(c)), Curve2::Circle(Circle2::Grid(d))) => {
            Crossing2::of_circles(c, d, root)?
        }
        (Curve2::Circle(Circle2::Round(c)), Curve2::Circle(Circle2::Round(d))) => {
            Crossing2::of_rounds(c, d, root)?
        }
        (Curve2::Circle(Circle2::Round(c)), Curve2::Circle(Circle2::Grid(d))) => {
            Crossing2::of_round_and_circle(c, d, root)?
        }
        (Curve2::Circle(Circle2::Grid(c)), Curve2::Circle(Circle2::Round(d))) => {
            Crossing2::of_round_and_circle(d, c, flipped)?
        }
        (Curve2::Circle(_) | Curve2::Ellipse(_), Curve2::Ellipse(_))
        | (Curve2::Ellipse(_), Curve2::Circle(_)) => return Err(PointError::WrongClass),
    })
}

impl Pt2 {
    /// Reads the point's definition into exact numbers.
    ///
    /// # Errors
    ///
    /// [`PointError::Num`] with [`NumError::Degenerate`] when its carriers do not meet;
    /// [`PointError::NoCarrier`], [`PointError::WrongClass`] and [`PointError::NoSuchRoot`] as they
    /// say.
    pub fn place(&self, carriers: &Carriers) -> Result<Place, PointError> {
        match self {
            Pt2::Grid(p) => Ok(Place::OneRoot(Crossing2::of_point(&(*p).into())?)),
            Pt2::Rational(p) => Ok(Place::OneRoot(Crossing2::of_point(p)?)),
            Pt2::Crossing { a, b, root } => Ok(Place::OneRoot(crossing(
                curve(carriers, *a)?,
                curve(carriers, *b)?,
                *root,
            )?)),
            Pt2::ConicRoot { a, b, k } => {
                let Curve2::Ellipse(ellipse) = curve(carriers, *a)? else {
                    return Err(PointError::WrongClass);
                };
                let conic = curve(carriers, *b)?.conic()?;
                let Meeting::At { quartic, roots } = ellipse.meeting(&conic)? else {
                    return Err(PointError::Num(NumError::Degenerate));
                };
                let root = roots
                    .iter()
                    .nth(usize::from(*k))
                    .ok_or(PointError::NoSuchRoot(*k))?;
                Ok(Place::Conic {
                    ellipse: *ellipse,
                    quartic,
                    root: *root,
                })
            }
        }
    }

    /// Whether the point is exactly on the carrier at `on`.
    ///
    /// # Errors
    ///
    /// As for [`Pt2::place`].
    pub fn on(&self, carriers: &Carriers, on: CurveIx) -> Result<bool, PointError> {
        let target = curve(carriers, on)?;
        Ok(match self.place(carriers)? {
            Place::OneRoot(z) => match target {
                Curve2::Line(l) => z.side(l)? == Sign::Zero,
                Curve2::Circle(Circle2::Grid(c)) => z.against_circle(c)? == Sign::Zero,
                Curve2::Circle(Circle2::Round(c)) => z.against_round(c)? == Sign::Zero,
                Curve2::Ellipse(e) => z.against_ellipse(e)? == Sign::Zero,
            },
            Place::Conic {
                ellipse,
                quartic,
                root,
            } => on_quartic(&ellipse, &quartic, &root, &target.conic()?)?,
        })
    }
}

/// Whether the root `root` of `quartic` along `ellipse` is a root of `other` along it too.
fn on_quartic(
    ellipse: &Ellipse2,
    quartic: &Quartic,
    root: &RealRoot,
    other: &Conic,
) -> Result<bool, PointError> {
    let Meeting::At {
        quartic: along,
        roots,
    } = ellipse.meeting(other)?
    else {
        return Ok(true);
    };
    for candidate in roots.iter() {
        if compare(quartic, root, &along, candidate)? == Sign::Zero {
            return Ok(true);
        }
    }
    Ok(false)
}
