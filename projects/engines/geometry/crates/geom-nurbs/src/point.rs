//! The control points the NURBS algorithms accept and return: `geom-math`'s finite points in two
//! and three dimensions, and plain coordinate arrays.
//!
//! What it does: one algorithm serves 2D and 3D (chapter 7: "One curve family serves 2D and 3D").
//! Curves and surfaces are generic over the dimension D and store coordinates as `[f64; D]`;
//! [`Point`] converts `geom-math`'s [`F2`] and [`F3`], and arrays, to and from those coordinates.
//!
//! Guarantees:
//!
//! - **Exact both ways.** `coords` copies the components, and `from_coords` builds the point from
//!   them unchanged.
//! - **Finite.** `from_coords` refuses a coordinate that is NaN or infinite with
//!   [`NurbsError::NotFinite`], as `geom-math`'s constructors do.
//!
//! Method: none; a trait and its implementations, which only copy components.
//!
//! Cost: constant; nothing allocates.

use geom_math::{F2, F3};

use crate::NurbsError;

/// A point of dimension `D` the NURBS algorithms can take and give back.
pub trait Point<const D: usize>: Copy {
    /// The coordinates.
    fn coords(self) -> [f64; D];

    /// The point with these coordinates.
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotFinite`] if a coordinate is NaN or infinite.
    fn from_coords(coords: [f64; D]) -> Result<Self, NurbsError>;
}

impl Point<2> for F2 {
    fn coords(self) -> [f64; 2] {
        self.to_array()
    }

    fn from_coords([x, y]: [f64; 2]) -> Result<Self, NurbsError> {
        Ok(F2::new(x, y)?)
    }
}

impl Point<3> for F3 {
    fn coords(self) -> [f64; 3] {
        self.to_array()
    }

    fn from_coords([x, y, z]: [f64; 3]) -> Result<Self, NurbsError> {
        Ok(F3::new(x, y, z)?)
    }
}

impl<const D: usize> Point<D> for [f64; D] {
    fn coords(self) -> [f64; D] {
        self
    }

    fn from_coords(coords: [f64; D]) -> Result<Self, NurbsError> {
        if coords.iter().all(|c| c.is_finite()) {
            Ok(coords)
        } else {
            Err(NurbsError::NotFinite)
        }
    }
}
