//! A section's or an elevation's line: where it is, which way it looks and how far.
//!
//! Seen from above, the line runs from one end to the other across the plan; the view looks to its
//! left or its right as one walks from the first end to the second, and sees as far as its depth,
//! measured square to the line. What the line crosses is cut; what lies between it and the depth,
//! within its two ends, is seen. A section's cut and an elevation's picture plane are the same
//! data: the difference is the drawing's, and a later job's.

use crate::error::ViewError;
use construct_params::Length;

/// A point of the plan, in the project's frame: its x and y.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    /// Along the model's x axis.
    pub x: Length,
    /// Along the model's y axis.
    pub y: Length,
}

/// Which side of the line, walking from its first end to its second, the view looks to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// To the left, looking along the line from its first end.
    Left,
    /// To the right.
    Right,
}

impl Side {
    /// The key it is stored under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    /// The side a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

/// A section's or elevation's line, direction and depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CutLine {
    from: Point,
    to: Point,
    look: Side,
    far: Length,
}

impl CutLine {
    /// A line from one end to the other, looking to `look` as far as `far`.
    ///
    /// # Errors
    /// [`ViewError::EmptyLine`] if the ends are one point, [`ViewError::NoDepth`] if `far` is not
    /// above zero.
    pub fn new(from: Point, to: Point, look: Side, far: Length) -> Result<Self, ViewError> {
        if from == to {
            return Err(ViewError::EmptyLine);
        }
        if far <= Length::from_steps(0) {
            return Err(ViewError::NoDepth);
        }
        Ok(Self {
            from,
            to,
            look,
            far,
        })
    }

    /// The first end.
    #[must_use]
    pub fn from(&self) -> Point {
        self.from
    }

    /// The second end.
    #[must_use]
    pub fn to(&self) -> Point {
        self.to
    }

    /// The side it looks to.
    #[must_use]
    pub fn look(&self) -> Side {
        self.look
    }

    /// How far it sees, square to the line.
    #[must_use]
    pub fn far(&self) -> Length {
        self.far
    }
}
