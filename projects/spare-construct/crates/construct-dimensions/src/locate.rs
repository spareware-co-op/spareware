//! Where a reference is, in the view's own drawing plane: the one question a dimension asks of the
//! model, behind a trait of ours.
//!
//! The model's elements (walls, grids, doors, windows, levels) are not this crate's: whoever
//! holds them (construct-api's `cloud::dimensions` reads the document's records) answers
//! [`Locator::locate`] for each reference with a [`Locus`], or says why it cannot ([`Missing`]).
//! The tests of this crate answer from a plain table ([`Located`]), which is also what the server
//! fills once per request. Geometry is asked only as exact positions in grid steps.

use crate::reference::Reference;
use construct_grids::Point;
use construct_params::{Angle, Length};
use std::collections::BTreeMap;

/// Which part of its circle a curve is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sweep {
    /// The whole circle.
    Full,
    /// An arc, anticlockwise from `from` to `to` (angles from `+x`, anticlockwise, in `[0, 360)`
    /// degrees, not equal): a grid arc's own angles.
    Between {
        /// Where it starts.
        from: Angle,
        /// Where it finishes.
        to: Angle,
    },
}

/// A curve a radial, diameter or arc length dimension measures: a circle's arc, as an element
/// draws it in plan. A curved grid line is one today; a curved wall's faces and line (job 14.4)
/// answer with one too, so its dimensions need nothing new.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Curve {
    /// The circle's centre.
    pub centre: Point,
    /// Its radius, above zero.
    pub radius: Length,
    /// Which part of the circle it is.
    pub sweep: Sweep,
}

/// Where a reference is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locus {
    /// A point: a dimension reads the foot of the perpendicular from it to its offset line.
    Point(Point),
    /// A straight line, through `through` (a point of the reference as drawn, where its witness
    /// line starts) and running `direction` (a vector of steps, not zero): a dimension reads where
    /// it meets its offset line, and an angular one the angle it makes with another line. Its
    /// direction is the way the element draws it (a wall from its start to its end, a grid from
    /// its first point to its second).
    Line {
        /// A point of the reference.
        through: Point,
        /// The vector it runs along, in steps.
        direction: [i64; 2],
    },
    /// A curve: a radial, diameter or arc length dimension reads it; a line dimension cannot.
    Arc(Curve),
}

impl Locus {
    /// A line through `through` running along the vector `(dx, dy)` of lengths.
    #[must_use]
    pub fn line(through: Point, dx: Length, dy: Length) -> Self {
        Self::Line {
            through,
            direction: [dx.steps(), dy.steps()],
        }
    }
}

/// Why a reference cannot be found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// The element it names is not in the model any more.
    Deleted,
    /// The element is there, but has no such part (a grid that is an arc, a wall without that
    /// face), or the part has no place in this view (a level in a plan).
    NoSuchPart,
    /// The element's record cannot be read.
    Unreadable,
}

/// A reference that cannot be found, and why, in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missing {
    /// The kind of reason.
    pub why: Why,
    /// The reason, in a sentence's words: `the element is not in this model`.
    pub detail: String,
}

impl Missing {
    /// The element is gone.
    #[must_use]
    pub fn deleted() -> Self {
        Self {
            why: Why::Deleted,
            detail: "the element is not in this model any more".to_owned(),
        }
    }

    /// The element has no such part here.
    #[must_use]
    pub fn no_such_part(detail: &str) -> Self {
        Self {
            why: Why::NoSuchPart,
            detail: detail.to_owned(),
        }
    }

    /// The element's record cannot be read.
    #[must_use]
    pub fn unreadable(detail: &str) -> Self {
        Self {
            why: Why::Unreadable,
            detail: detail.to_owned(),
        }
    }
}

/// What a dimension asks of the model.
pub trait Locator {
    /// Where `reference` is, in the plane of the dimension's view.
    ///
    /// # Errors
    /// [`Missing`], saying why, when it cannot be found: a dimension then reports it by name and
    /// gives no value that depends on it.
    fn locate(&self, reference: &Reference) -> Result<Locus, Missing>;
}

/// A locator that answers from a table: each reference's answer, found once. A reference not in
/// the table is [`Missing::deleted`].
#[derive(Clone, Debug, Default)]
pub struct Located {
    answers: BTreeMap<Reference, Result<Locus, Missing>>,
}

impl Located {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the answer for a reference, replacing any.
    pub fn set(&mut self, reference: Reference, answer: Result<Locus, Missing>) {
        self.answers.insert(reference, answer);
    }

    /// The table with one more answer.
    #[must_use]
    pub fn with(mut self, reference: Reference, answer: Result<Locus, Missing>) -> Self {
        self.set(reference, answer);
        self
    }
}

impl Locator for Located {
    fn locate(&self, reference: &Reference) -> Result<Locus, Missing> {
        self.answers
            .get(reference)
            .cloned()
            .unwrap_or_else(|| Err(Missing::deleted()))
    }
}
