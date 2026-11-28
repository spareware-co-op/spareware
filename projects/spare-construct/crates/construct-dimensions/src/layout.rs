//! How a dimension lies in its view: which way it measures and where it is drawn.
//!
//! A linear or ordinate dimension has an offset line, the line its value is read along and its
//! text sits on; each witness line runs from a reference to it. Horizontal and vertical
//! dimensions measure along x and along y; an aligned one measures along a stated direction (a
//! wall's, say), through a stated point. An angular dimension is drawn as an arc about the point
//! where its two lines meet, at a stated radius, between the two rays it names; a radial or a
//! diameter dimension as a line from an arc's centre in a stated direction; an arc length
//! dimension as an arc beside the one it measures, a stated distance outside it. All of it is
//! data: nothing here moves it when a reference moves, and none of it is a value.

use crate::error::DimensionError;
use construct_grids::{LIMIT, Point};
use construct_params::Length;

/// A vector in plan that is not zero, read for its direction only, never its size: the way an
/// aligned dimension measures, or the way a radial one points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Direction {
    dx: Length,
    dy: Length,
}

fn within(length: Length) -> bool {
    length.steps().unsigned_abs() <= LIMIT.steps().unsigned_abs()
}

impl Direction {
    /// A direction.
    ///
    /// # Errors
    /// [`DimensionError::ZeroDirection`] for a zero vector; [`DimensionError::OutOfRange`] past
    /// the kernel's grid.
    pub fn new(dx: Length, dy: Length) -> Result<Self, DimensionError> {
        if dx.steps() == 0 && dy.steps() == 0 {
            return Err(DimensionError::ZeroDirection);
        }
        if !within(dx) || !within(dy) {
            return Err(DimensionError::OutOfRange(
                "a direction is beyond the kernel's grid".to_owned(),
            ));
        }
        Ok(Self { dx, dy })
    }

    /// Along x.
    #[must_use]
    pub fn dx(self) -> Length {
        self.dx
    }

    /// Along y.
    #[must_use]
    pub fn dy(self) -> Length {
        self.dy
    }

    /// The vector in steps.
    #[must_use]
    pub fn steps(self) -> [i64; 2] {
        [self.dx.steps(), self.dy.steps()]
    }
}

/// Which way along one of an angular dimension's lines its angle opens: the way the line runs (as
/// its element draws it: a wall from its start to its end, a grid from its first point), or the
/// other way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ray {
    /// The way the line runs.
    Forward,
    /// Against it.
    Backward,
}

impl Ray {
    /// The key it is stored under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Backward => "backward",
        }
    }

    /// The ray a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        [Self::Forward, Self::Backward]
            .into_iter()
            .find(|ray| ray.key() == key)
    }

    /// The vector of the ray along a line running `d`.
    #[must_use]
    pub fn along(self, d: [i64; 2]) -> Option<[i64; 2]> {
        match self {
            Self::Forward => Some(d),
            Self::Backward => Some([d[0].checked_neg()?, d[1].checked_neg()?]),
        }
    }
}

/// How a dimension lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layout {
    /// Measures along x; its offset line is the horizontal line at `y`.
    Horizontal {
        /// Where the offset line is, up the view.
        y: Length,
    },
    /// Measures along y; its offset line is the vertical line at `x`.
    Vertical {
        /// Where the offset line is, across the view.
        x: Length,
    },
    /// Measures along `direction`; its offset line goes through `through` that way.
    Aligned {
        /// A point of the offset line.
        through: Point,
        /// Which way it runs.
        direction: Direction,
    },
    /// The angle between two lines, drawn as an arc of `radius` about the point where they meet,
    /// from the `first` line's ray to the `second` line's.
    Angular {
        /// Which way along the first reference's line the angle opens.
        first: Ray,
        /// Which way along the second's.
        second: Ray,
        /// The radius of its arc, above zero.
        radius: Length,
    },
    /// An arc's or a circle's radius, drawn from its centre `toward` a direction.
    Radial {
        /// The way its line runs from the centre.
        toward: Direction,
    },
    /// An arc's or a circle's diameter, drawn through its centre along a direction.
    Diameter {
        /// The way its line runs from the centre to the side its text is on.
        toward: Direction,
    },
    /// An arc's length, drawn as an arc `offset` outside it (inside when below zero).
    ArcLength {
        /// How far the dimension's arc is from the arc measured, outward.
        offset: Length,
    },
}

impl Layout {
    /// A horizontal layout.
    ///
    /// # Errors
    /// [`DimensionError::OutOfRange`] past the kernel's grid.
    pub fn horizontal(y: Length) -> Result<Self, DimensionError> {
        Self::checked(Self::Horizontal { y })
    }

    /// A vertical layout.
    ///
    /// # Errors
    /// [`DimensionError::OutOfRange`] past the kernel's grid.
    pub fn vertical(x: Length) -> Result<Self, DimensionError> {
        Self::checked(Self::Vertical { x })
    }

    /// An aligned layout.
    ///
    /// # Errors
    /// [`DimensionError::OutOfRange`] past the kernel's grid.
    pub fn aligned(through: Point, direction: Direction) -> Result<Self, DimensionError> {
        Self::checked(Self::Aligned { through, direction })
    }

    /// An angular layout.
    ///
    /// # Errors
    /// [`DimensionError::OutOfRange`] for a radius not above zero or past the kernel's grid.
    pub fn angular(first: Ray, second: Ray, radius: Length) -> Result<Self, DimensionError> {
        Self::checked(Self::Angular {
            first,
            second,
            radius,
        })
    }

    /// A radial layout.
    #[must_use]
    pub fn radial(toward: Direction) -> Self {
        Self::Radial { toward }
    }

    /// A diameter layout.
    #[must_use]
    pub fn diameter(toward: Direction) -> Self {
        Self::Diameter { toward }
    }

    /// An arc length layout.
    ///
    /// # Errors
    /// [`DimensionError::OutOfRange`] past the kernel's grid.
    pub fn arc_length(offset: Length) -> Result<Self, DimensionError> {
        Self::checked(Self::ArcLength { offset })
    }

    fn checked(layout: Self) -> Result<Self, DimensionError> {
        let inside = match layout {
            Self::Horizontal { y: c } | Self::Vertical { x: c } | Self::ArcLength { offset: c } => {
                within(c)
            }
            Self::Aligned { through, .. } => through.in_range(),
            Self::Angular { radius, .. } => {
                if radius.steps() <= 0 {
                    return Err(DimensionError::OutOfRange(
                        "an angular dimension's arc needs a radius above zero".to_owned(),
                    ));
                }
                within(radius)
            }
            Self::Radial { .. } | Self::Diameter { .. } => true,
        };
        if inside {
            Ok(layout)
        } else {
            Err(DimensionError::OutOfRange(
                "the dimension's line is beyond the kernel's grid".to_owned(),
            ))
        }
    }

    /// The key it is stored under.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::Horizontal { .. } => "horizontal",
            Self::Vertical { .. } => "vertical",
            Self::Aligned { .. } => "aligned",
            Self::Angular { .. } => "angular",
            Self::Radial { .. } => "radial",
            Self::Diameter { .. } => "diameter",
            Self::ArcLength { .. } => "arc_length",
        }
    }

    /// Whether it has an offset line: a horizontal, vertical or aligned layout, which a linear or
    /// an ordinate dimension has.
    #[must_use]
    pub const fn is_line(&self) -> bool {
        matches!(
            self,
            Self::Horizontal { .. } | Self::Vertical { .. } | Self::Aligned { .. }
        )
    }

    /// How many references a dimension of this layout has: at least two for an offset line (no
    /// most but [`crate::MAX_REFERENCES`]), exactly two for an angle, exactly one for an arc's
    /// radius, diameter or length. The fewest, then the most.
    #[must_use]
    pub const fn references(&self) -> (usize, usize) {
        match self {
            Self::Horizontal { .. } | Self::Vertical { .. } | Self::Aligned { .. } => {
                (2, crate::MAX_REFERENCES)
            }
            Self::Angular { .. } => (2, 2),
            Self::Radial { .. } | Self::Diameter { .. } | Self::ArcLength { .. } => (1, 1),
        }
    }

    /// The offset line of a line layout: a point on it, and the vector it runs along, in steps.
    /// An angular, radial or arc length layout has none: its point and direction are `(0, 0)`
    /// and `[1, 0]` here, and nothing measures along them ([`Layout::is_line`]).
    #[must_use]
    pub fn line(&self) -> (Point, [i64; 2]) {
        let zero = Length::from_steps(0);
        match *self {
            Self::Horizontal { y } => (Point::new(zero, y), [1, 0]),
            Self::Vertical { x } => (Point::new(x, zero), [0, 1]),
            Self::Aligned { through, direction } => {
                (through, [direction.dx.steps(), direction.dy.steps()])
            }
            Self::Angular { .. }
            | Self::Radial { .. }
            | Self::Diameter { .. }
            | Self::ArcLength { .. } => (Point::new(zero, zero), [1, 0]),
        }
    }

    /// The layout with its offset line moved to `line`: the y of a horizontal one, the x of a
    /// vertical one.
    ///
    /// # Errors
    /// [`DimensionError::WrongLayout`] for an aligned layout, which is moved by
    /// [`Layout::with_through`], and for one with no offset line, which is moved by the place a
    /// person puts it ([`crate::placed`]); [`DimensionError::OutOfRange`] past the grid.
    pub fn with_line(&self, line: Length) -> Result<Self, DimensionError> {
        match self {
            Self::Horizontal { .. } => Self::horizontal(line),
            Self::Vertical { .. } => Self::vertical(line),
            Self::Aligned { .. } => Err(DimensionError::WrongLayout(
                "An aligned dimension's line is moved by the point it goes through.".to_owned(),
            )),
            _ => Err(DimensionError::WrongLayout(
                "An angular, radial, diameter or arc length dimension is moved by a place to put it."
                    .to_owned(),
            )),
        }
    }

    /// The aligned layout with its offset line through `through`, its direction kept.
    ///
    /// # Errors
    /// [`DimensionError::WrongLayout`] for any other layout; [`DimensionError::OutOfRange`] past
    /// the grid.
    pub fn with_through(&self, through: Point) -> Result<Self, DimensionError> {
        match self {
            Self::Aligned { direction, .. } => Self::aligned(through, *direction),
            Self::Horizontal { .. } | Self::Vertical { .. } => Err(DimensionError::WrongLayout(
                "A horizontal or vertical dimension's line is moved by its position across."
                    .to_owned(),
            )),
            _ => Err(DimensionError::WrongLayout(
                "An angular, radial, diameter or arc length dimension is moved by a place to put it."
                    .to_owned(),
            )),
        }
    }
}
