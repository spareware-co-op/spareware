//! How a dimension lies in its view: which way it measures and where its offset line is.
//!
//! The offset line is the line the dimension's value is read along and its text sits on; each
//! witness line runs from a reference to it. Horizontal and vertical dimensions measure along x
//! and along y; an aligned one measures along a stated direction (a wall's, say), through a stated
//! point. The line is data: nothing here moves it when a reference moves.

use crate::error::DimensionError;
use construct_grids::{LIMIT, Point};
use construct_params::Length;

/// The way an aligned dimension measures: a vector in plan that is not zero. Only its direction
/// matters, never its size.
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

    fn checked(layout: Self) -> Result<Self, DimensionError> {
        let inside = match layout {
            Self::Horizontal { y: c } | Self::Vertical { x: c } => within(c),
            Self::Aligned { through, .. } => through.in_range(),
        };
        if inside {
            Ok(layout)
        } else {
            Err(DimensionError::OutOfRange(
                "the offset line is beyond the kernel's grid".to_owned(),
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
        }
    }

    /// The offset line: a point on it, and the vector it runs along, in steps.
    #[must_use]
    pub fn line(&self) -> (Point, [i64; 2]) {
        let zero = Length::from_steps(0);
        match *self {
            Self::Horizontal { y } => (Point::new(zero, y), [1, 0]),
            Self::Vertical { x } => (Point::new(x, zero), [0, 1]),
            Self::Aligned { through, direction } => {
                (through, [direction.dx.steps(), direction.dy.steps()])
            }
        }
    }

    /// The layout with its offset line moved to `line`: the y of a horizontal one, the x of a
    /// vertical one.
    ///
    /// # Errors
    /// [`DimensionError::WrongLayout`] for an aligned layout, which is moved by
    /// [`Layout::with_through`]; [`DimensionError::OutOfRange`] past the grid.
    pub fn with_line(&self, line: Length) -> Result<Self, DimensionError> {
        match self {
            Self::Horizontal { .. } => Self::horizontal(line),
            Self::Vertical { .. } => Self::vertical(line),
            Self::Aligned { .. } => Err(DimensionError::WrongLayout(
                "An aligned dimension's line is moved by the point it goes through.".to_owned(),
            )),
        }
    }

    /// The aligned layout with its offset line through `through`, its direction kept.
    ///
    /// # Errors
    /// [`DimensionError::WrongLayout`] for a horizontal or vertical layout;
    /// [`DimensionError::OutOfRange`] past the grid.
    pub fn with_through(&self, through: Point) -> Result<Self, DimensionError> {
        match self {
            Self::Aligned { direction, .. } => Self::aligned(through, *direction),
            _ => Err(DimensionError::WrongLayout(
                "A horizontal or vertical dimension's line is moved by its position across."
                    .to_owned(),
            )),
        }
    }
}
