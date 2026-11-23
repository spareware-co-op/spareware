//! A view along a grid axis, its cut plane, and the exact map from model points to view points.
//!
//! What it does: a [`View`] looks along one grid axis ([`Looking`]): down for a plan, or north,
//! south, east or west for a section or elevation on a grid line (note A12 limits the first engine
//! to such views). It cuts the model with the plane square to that axis at [`View::cut`] along it.
//!
//! The map, exact: a view has three whole unit vectors, the direction it looks (`v`), the way up on
//! the sheet (`up`) and the way right (`right = v × up`). A model point `p`, in grid steps, is the
//! view point `(right · p, up · p)` in the same steps, and its depth is `v · p`. Since
//! `right × up = -v`, the drawing is seen from the side the viewer stands on, so a loop turning
//! counter-clockwise round material seen from the viewer turns counter-clockwise in the drawing.
//!
//! | Looking | `v` | `up` | `right` | View point of (x, y, z) | Cut plane |
//! | --- | --- | --- | --- | --- | --- |
//! | Down (a plan) | -z | +y | +x | (x, y) | z = cut, normal +z |
//! | North | +y | +z | +x | (x, z) | y = cut, normal -y |
//! | South | -y | +z | -x | (-x, z) | y = cut, normal +y |
//! | East | +x | +z | -y | (-y, z) | x = cut, normal -x |
//! | West | -x | +z | +y | (y, z) | x = cut, normal +x |
//!
//! The cut plane's normal points at the viewer (`-v`): what lies on the viewer's side is removed,
//! and what lies beyond is kept and drawn.

use construct_params::Length;
use engine_geometry::value::engine::{CutPoint, GridPlane};
use serde::{Deserialize, Serialize};

use crate::DrawingError;
use crate::drawing::ViewPoint;

/// Which way a view looks: along one grid axis.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Looking {
    /// Down, along -z: a plan, north up the sheet.
    Down,
    /// Along +y.
    North,
    /// Along -y.
    South,
    /// Along +x.
    East,
    /// Along -x.
    West,
}

impl Looking {
    /// The direction looked in, `v`.
    #[must_use]
    pub const fn direction(self) -> [i128; 3] {
        match self {
            Self::Down => [0, 0, -1],
            Self::North => [0, 1, 0],
            Self::South => [0, -1, 0],
            Self::East => [1, 0, 0],
            Self::West => [-1, 0, 0],
        }
    }

    /// Toward the viewer, `-v`: the cut plane's normal.
    #[must_use]
    pub const fn toward_viewer(self) -> [i128; 3] {
        match self {
            Self::Down => [0, 0, 1],
            Self::North => [0, -1, 0],
            Self::South => [0, 1, 0],
            Self::East => [-1, 0, 0],
            Self::West => [1, 0, 0],
        }
    }

    /// Up the sheet.
    #[must_use]
    pub const fn up(self) -> [i128; 3] {
        match self {
            Self::Down => [0, 1, 0],
            _ => [0, 0, 1],
        }
    }

    /// Right on the sheet: `v × up`.
    #[must_use]
    pub const fn right(self) -> [i128; 3] {
        match self {
            Self::Down | Self::North => [1, 0, 0],
            Self::South => [-1, 0, 0],
            Self::East => [0, -1, 0],
            Self::West => [0, 1, 0],
        }
    }

    /// The axis looked along: 0, 1 or 2 for x, y or z.
    #[must_use]
    pub const fn axis(self) -> usize {
        match self {
            Self::Down => 2,
            Self::North | Self::South => 1,
            Self::East | Self::West => 0,
        }
    }
}

/// A view along a grid axis, cut by the plane square to it at `cut` along that axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct View {
    /// Which way it looks.
    pub looking: Looking,
    /// Where the cut plane lies along the axis looked along: a plan's cut height, a north or south
    /// section's y, an east or west section's x.
    pub cut: Length,
}

/// `a · b`, exact.
fn dot(a: [i128; 3], b: [i128; 3]) -> Result<i128, DrawingError> {
    let mut sum: i128 = 0;
    for (x, y) in a.into_iter().zip(b) {
        sum = x
            .checked_mul(y)
            .and_then(|t| sum.checked_add(t))
            .ok_or(DrawingError::TooBig)?;
    }
    Ok(sum)
}

impl View {
    /// A plan cut at `height` above the model's origin.
    #[must_use]
    pub const fn plan(height: Length) -> Self {
        Self {
            looking: Looking::Down,
            cut: height,
        }
    }

    /// The cut plane: square to the axis looked along at [`View::cut`], its normal at the viewer.
    ///
    /// # Errors
    ///
    /// [`DrawingError::TooBig`] never for a length of the grid; it is checked all the same.
    pub fn cut_plane(&self) -> Result<GridPlane, DrawingError> {
        let normal = self.looking.toward_viewer();
        let mut along = [0_i128; 3];
        if let Some(c) = along.get_mut(self.looking.axis()) {
            *c = i128::from(self.cut.steps());
        }
        Ok(GridPlane {
            normal,
            offset: dot(normal, along)?,
        })
    }

    /// The view point of the model point `p` moved by `by` grid steps.
    ///
    /// # Errors
    ///
    /// [`DrawingError::TooBig`] when a coordinate passes 128 bits.
    pub fn project(&self, p: &CutPoint, by: [i64; 3]) -> Result<ViewPoint, DrawingError> {
        let mut num = [0_i128; 3];
        for ((n, c), b) in num.iter_mut().zip(p.num).zip(by) {
            *n = i128::from(b)
                .checked_mul(p.den)
                .and_then(|m| m.checked_add(c))
                .ok_or(DrawingError::TooBig)?;
        }
        ViewPoint::new(
            dot(self.looking.right(), num)?,
            dot(self.looking.up(), num)?,
            p.den,
        )
    }
}
