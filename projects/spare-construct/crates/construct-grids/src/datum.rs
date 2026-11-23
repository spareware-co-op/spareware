//! A grid line and a reference plane: what each is, before it is stored.
//!
//! A grid line is a vertical thing seen in plan: a [`Shape`] with a name, and no elevation of its
//! own. A reference plane is either a line in plan standing vertical, or a horizontal plane at an
//! elevation, with a name that is optional.

use crate::GridError;
use crate::name::clean_name;
use crate::shape::{LIMIT, Point, Shape};
use construct_params::Length;
use kit_core::Id;

/// A grid line: an element of the grids category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    id: Id,
    name: String,
    shape: Shape,
}

impl Grid {
    /// A grid line, its name cleaned ([`clean_name`]) and its shape checked.
    ///
    /// # Errors
    ///
    /// A name refusal, or the shape's own ([`Shape::check`]).
    pub fn new(id: Id, name: &str, shape: Shape) -> Result<Self, GridError> {
        shape.check()?;
        Ok(Self {
            id,
            name: clean_name(name)?,
            shape,
        })
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Its name, as it was written.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its geometry in plan.
    #[must_use]
    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    /// The same grid under another name.
    ///
    /// # Errors
    ///
    /// A name refusal.
    pub fn renamed(&self, name: &str) -> Result<Self, GridError> {
        Ok(Self {
            name: clean_name(name)?,
            ..self.clone()
        })
    }

    /// The same grid with another shape.
    ///
    /// # Errors
    ///
    /// The shape's own refusal.
    pub fn reshaped(&self, shape: Shape) -> Result<Self, GridError> {
        shape.check()?;
        Ok(Self {
            shape,
            ..self.clone()
        })
    }

    /// The grid's length along itself, the nearest step ([`Shape::length`]).
    #[must_use]
    pub fn length(&self) -> Length {
        self.shape.length()
    }
}

/// A reference plane's geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaneShape {
    /// A line in plan, standing vertical: the plane through it, straight up and down.
    Vertical {
        /// Where the line starts.
        from: Point,
        /// Where it finishes.
        to: Point,
    },
    /// A horizontal plane at an elevation (negative below the project's zero).
    Horizontal {
        /// How high it is.
        elevation: Length,
    },
}

impl PlaneShape {
    /// Whether the shape is one this crate makes.
    ///
    /// # Errors
    ///
    /// [`GridError::ZeroLength`] for a vertical plane of two equal points;
    /// [`GridError::OutOfRange`] past [`LIMIT`].
    pub fn check(&self) -> Result<(), GridError> {
        match *self {
            Self::Vertical { from, to } => Shape::line(from, to).map(|_| ()),
            Self::Horizontal { elevation } => {
                if elevation.steps().unsigned_abs() > LIMIT.steps().unsigned_abs() {
                    return Err(GridError::OutOfRange);
                }
                Ok(())
            }
        }
    }

    /// The shape moved by `dx` and `dy` in plan and `dz` up. A vertical plane has no height of its
    /// own to move, and a horizontal one has no place in plan.
    ///
    /// # Errors
    ///
    /// [`GridError::WrongShape`] for a move the plane cannot make; [`GridError::OutOfRange`].
    pub fn moved(&self, dx: Length, dy: Length, dz: Length) -> Result<Self, GridError> {
        let zero = Length::from_steps(0);
        let moved = match *self {
            Self::Vertical { from, to } => {
                if dz != zero {
                    return Err(GridError::WrongShape(
                        "A vertical reference plane has no height of its own to move.",
                    ));
                }
                let line = Shape::Line { from, to };
                match line.moved(dx, dy)? {
                    Shape::Line { from, to } => Self::Vertical { from, to },
                    Shape::Arc { .. } => return Err(GridError::ZeroLength),
                }
            }
            Self::Horizontal { elevation } => {
                if dx != zero || dy != zero {
                    return Err(GridError::WrongShape(
                        "A horizontal reference plane has no place in plan to move.",
                    ));
                }
                Self::Horizontal {
                    elevation: elevation.checked_add(dz).ok_or(GridError::OutOfRange)?,
                }
            }
        };
        moved.check()?;
        Ok(moved)
    }
}

/// A reference plane: an element of the reference planes category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferencePlane {
    id: Id,
    name: Option<String>,
    shape: PlaneShape,
}

impl ReferencePlane {
    /// A reference plane. A name, if given, is cleaned ([`clean_name`]).
    ///
    /// # Errors
    ///
    /// A name refusal, or the shape's own ([`PlaneShape::check`]).
    pub fn new(id: Id, name: Option<&str>, shape: PlaneShape) -> Result<Self, GridError> {
        shape.check()?;
        Ok(Self {
            id,
            name: name.map(clean_name).transpose()?,
            shape,
        })
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Its name, if it has one.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Its geometry.
    #[must_use]
    pub fn shape(&self) -> &PlaneShape {
        &self.shape
    }

    /// The same plane under another name, or with none.
    ///
    /// # Errors
    ///
    /// A name refusal.
    pub fn renamed(&self, name: Option<&str>) -> Result<Self, GridError> {
        Ok(Self {
            name: name.map(clean_name).transpose()?,
            ..self.clone()
        })
    }

    /// The same plane with another shape.
    ///
    /// # Errors
    ///
    /// The shape's own refusal.
    pub fn reshaped(&self, shape: PlaneShape) -> Result<Self, GridError> {
        shape.check()?;
        Ok(Self {
            shape,
            ..self.clone()
        })
    }
}
