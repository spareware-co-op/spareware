//! An outline in plan: the closed loop of straight edges a floor opening and a shaft cut, its
//! corners in grid steps (construct-params' [`Length`], 6,400 to the millimetre).
//!
//! Checks ([`Outline::new`]), each a refusal naming the element: at least three corners, no two
//! one after the other in one place, and no two edges crossing, touching or running over each
//! other. They are the floor's own loop checks (construct-slabs' `Boundary`, exact orientation
//! tests on the grid), asked of the outline as a loop of its own, so an outline a floor would
//! refuse as a hole is refused here in the same terms. A simple loop always encloses an area.
//!
//! Stored ([`Outline::to_value`]): a JSON array of corners, each `[x,y]` in whole grid steps, in
//! the order drawn, either way round. A corner written as an object (an arc's or a bulge's form)
//! is a curved edge, refused by name ([`OpeningError::Curved`]) until the engine seam's extrusion
//! takes curves (the interim register's row).

use crate::error::{Named, OpeningError};
use construct_params::{Area, Length};
use construct_slabs::{Boundary, Corner, SlabError};
use construct_solver::Point;
use serde_json::Value;

/// An outline every check has passed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outline {
    corners: Vec<[Length; 2]>,
}

/// The refusal of `element` for a single loop's error from the floor's checks.
fn refusal(element: &Named, error: &SlabError) -> OpeningError {
    let element = element.clone();
    match error {
        SlabError::TooFewSides { sides, .. } => OpeningError::TooFewCorners {
            element,
            corners: *sides,
        },
        SlabError::ZeroLength { .. } => OpeningError::CornersTogether { element },
        SlabError::Crossing { .. } => OpeningError::CrossesItself { element },
        SlabError::OutOfRange => OpeningError::OutOfRange { element },
        other => OpeningError::Malformed {
            element,
            why: other.to_string(),
        },
    }
}

/// The corners as the floor's checks take them.
pub(crate) fn as_corners(corners: &[[i64; 2]]) -> Vec<Corner> {
    corners
        .iter()
        .map(|&[x, y]| Corner {
            id: None,
            at: Point { x, y },
        })
        .collect()
}

/// Twice the signed area of a loop of corners in grid steps: above 0 anticlockwise. Exact: every
/// corner is within the kernel's grid (2^40 steps), so no product comes near an `i128`'s range.
pub(crate) fn twice_signed_area(corners: &[[i64; 2]]) -> i128 {
    let mut twice = 0_i128;
    for (at, &[x0, y0]) in corners.iter().enumerate() {
        let [x1, y1] = corners
            .get(at.saturating_add(1))
            .or_else(|| corners.first())
            .copied()
            .unwrap_or([x0, y0]);
        let cross = i128::from(x0)
            .saturating_mul(i128::from(y1))
            .saturating_sub(i128::from(x1).saturating_mul(i128::from(y0)));
        twice = twice.saturating_add(cross);
    }
    twice
}

impl Outline {
    /// An outline of `corners`, checked as the module header says, for `element`.
    ///
    /// # Errors
    /// [`OpeningError::TooFewCorners`], [`OpeningError::CornersTogether`],
    /// [`OpeningError::CrossesItself`] or [`OpeningError::OutOfRange`].
    pub fn new(element: &Named, corners: Vec<[Length; 2]>) -> Result<Self, OpeningError> {
        let steps: Vec<[i64; 2]> = corners.iter().map(|c| c.map(Length::steps)).collect();
        Boundary::from_corners(&as_corners(&steps), &[]).map_err(|e| refusal(element, &e))?;
        Ok(Self { corners })
    }

    /// An outline from whole millimetres, exactly.
    ///
    /// # Errors
    /// As [`Outline::new`], and [`OpeningError::OutOfRange`] for a corner beyond a length's
    /// range.
    pub fn from_mm(element: &Named, corners: &[[i64; 2]]) -> Result<Self, OpeningError> {
        let range = || OpeningError::OutOfRange {
            element: element.clone(),
        };
        let lengths = corners
            .iter()
            .map(|&[x, y]| {
                Ok([
                    Length::checked_from_mm(x).ok_or_else(range)?,
                    Length::checked_from_mm(y).ok_or_else(range)?,
                ])
            })
            .collect::<Result<Vec<_>, OpeningError>>()?;
        Self::new(element, lengths)
    }

    /// Its corners, in the order drawn.
    #[must_use]
    pub fn corners(&self) -> &[[Length; 2]] {
        &self.corners
    }

    /// Its corners in grid steps, moved by `by` grid steps.
    ///
    /// # Errors
    /// [`OpeningError::OutOfRange`] for a corner moved beyond an `i64`.
    pub fn steps_moved(
        &self,
        element: &Named,
        by: [i64; 2],
    ) -> Result<Vec<[i64; 2]>, OpeningError> {
        let [dx, dy] = by;
        let range = || OpeningError::OutOfRange {
            element: element.clone(),
        };
        self.corners
            .iter()
            .map(|[x, y]| {
                Ok([
                    x.steps().checked_add(dx).ok_or_else(range)?,
                    y.steps().checked_add(dy).ok_or_else(range)?,
                ])
            })
            .collect()
    }

    /// The area it encloses, exact.
    ///
    /// # Errors
    /// [`OpeningError::Malformed`] when it is not a whole number of square grid steps (an
    /// outline whose twice-area is odd).
    pub fn area(&self, element: &Named) -> Result<Area, OpeningError> {
        let steps: Vec<[i64; 2]> = self.corners.iter().map(|c| c.map(Length::steps)).collect();
        let twice = twice_signed_area(&steps).saturating_abs();
        if twice.checked_rem(2) != Some(0) {
            return Err(OpeningError::Malformed {
                element: element.clone(),
                why: "its area is not a whole number of square grid steps".to_owned(),
            });
        }
        Ok(Area::from_square_steps(twice.checked_div(2).unwrap_or(0)))
    }

    /// Its stored form: `[[x,y],...]` in whole grid steps.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::Array(
            self.corners
                .iter()
                .map(|[x, y]| Value::from(vec![x.steps(), y.steps()]))
                .collect(),
        )
    }

    /// An outline read from its stored form, checked.
    ///
    /// # Errors
    /// [`OpeningError::Curved`] for a corner written as an object; [`OpeningError::Malformed`]
    /// for anything else not of the stored form; any refusal of [`Outline::new`].
    pub fn from_value(element: &Named, value: &Value) -> Result<Self, OpeningError> {
        let malformed = |why: &str| OpeningError::Malformed {
            element: element.clone(),
            why: why.to_owned(),
        };
        let list = value
            .as_array()
            .ok_or_else(|| malformed("its outline is a list of corners [x,y] in grid steps"))?;
        let mut corners = Vec::with_capacity(list.len());
        for corner in list {
            if corner.is_object() {
                return Err(OpeningError::Curved {
                    element: element.clone(),
                });
            }
            let pair = corner.as_array().map(Vec::as_slice);
            let Some([x, y]) = pair else {
                return Err(malformed(
                    "each corner of its outline is [x,y] in grid steps",
                ));
            };
            let (Some(x), Some(y)) = (x.as_i64(), y.as_i64()) else {
                return Err(malformed(
                    "each corner of its outline is whole grid steps, not a fraction",
                ));
            };
            corners.push([Length::from_steps(x), Length::from_steps(y)]);
        }
        Self::new(element, corners)
    }
}
