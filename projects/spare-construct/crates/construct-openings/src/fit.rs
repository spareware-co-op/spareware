//! Where openings and shafts cut a floor, decided before any engine is asked, exactly on the grid.
//!
//! - [`fit_in_floor`]: which of the outlines handed to a floor cut it, and every refusal. A floor
//!   opening lies wholly inside its floor; a shaft over the floor cuts it, and a shaft wholly
//!   beside the floor in plan passes it by. Each outline cuts a hole through every layer: the
//!   floor's boundary with the outlines as more holes must pass the floor's own checks
//!   (construct-slabs' `Boundary`: every hole inside the outer loop touching nothing of it, no two
//!   holes overlapping or touching). Refused, each naming the element: an opening outside its
//!   floor, an opening or a shaft across the floor's edge or touching it, two of them overlapping
//!   or touching, one over a hole of the floor's own, a shaft that takes the whole floor, one key
//!   twice.
//! - [`reach`]: whether a shaft passes a floor, from its base and top and the floor's bottom and
//!   top faces: wholly through it, missing it, or ending inside its thickness (refused by the
//!   caller, [`crate::OpeningError::EndsInFloor`]: a shaft that cut part of a floor's layers would
//!   be a recess, which is not built).

use crate::error::{End, Named, OpeningError};
use crate::outline::as_corners;
use construct_params::Length;
use construct_slabs::{Boundary, Ring, SlabError};

/// An outline handed to a floor, in the floor's own frame, in grid steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cutter {
    /// The opening or shaft.
    pub element: Named,
    /// Its corners in the floor's frame.
    pub corners: Vec<[i64; 2]>,
}

/// The floor's own refusal, worded for the element it was asked about.
fn floors_own(element: &Named, floor: &str, error: &SlabError) -> OpeningError {
    OpeningError::Malformed {
        element: element.clone(),
        why: format!("{floor}'s own boundary: {error}"),
    }
}

/// The refusal of a cutter's own loop, as a loop of its own.
fn own_loop(element: &Named, error: &SlabError) -> OpeningError {
    let element = element.clone();
    match error {
        SlabError::TooFewSides { sides, .. } => OpeningError::TooFewCorners {
            element,
            corners: *sides,
        },
        SlabError::ZeroLength { .. } => OpeningError::CornersTogether { element },
        SlabError::Crossing { .. } => OpeningError::CrossesItself { element },
        _ => OpeningError::OutOfRange { element },
    }
}

/// Which of `cutters` cut a floor of loops `outer` and `holes` (grid steps, its own frame), by
/// their index in `cutters`, in the order of their keys; or the first refusal, in that order,
/// naming the floor as `floor` says (`floor F-1`, or `its floor` where its ID is not known).
///
/// # Errors
/// As the module header says.
pub fn fit_in_floor(
    floor: &str,
    outer: &[[i64; 2]],
    holes: &[Vec<[i64; 2]>],
    cutters: &[Cutter],
) -> Result<Vec<usize>, OpeningError> {
    let mut order: Vec<usize> = (0..cutters.len()).collect();
    order.sort_by(|&a, &b| {
        let key = |i: usize| cutters.get(i).map(|c| &c.element.key);
        key(a).cmp(&key(b))
    });
    let outer_corners = as_corners(outer);
    let mut cutting = Vec::with_capacity(order.len());
    let mut previous: Option<&Named> = None;
    for &at in &order {
        let Some(cutter) = cutters.get(at) else {
            continue;
        };
        let element = &cutter.element;
        if previous.is_some_and(|p| p.key == element.key) {
            return Err(OpeningError::Twice {
                element: element.clone(),
            });
        }
        previous = Some(element);
        let ring = as_corners(&cutter.corners);
        match Boundary::from_corners(&outer_corners, std::slice::from_ref(&ring)) {
            Ok(_) => cutting.push(at),
            Err(SlabError::HoleOutside { .. }) => {
                if element.what == crate::What::Opening {
                    return Err(OpeningError::OutsideHost {
                        element: element.clone(),
                        host: floor.to_owned(),
                    });
                }
                // Beside the floor, or round the whole of it.
                if Boundary::from_corners(&ring, std::slice::from_ref(&outer_corners)).is_ok() {
                    return Err(OpeningError::TakesFloor {
                        element: element.clone(),
                        host: floor.to_owned(),
                    });
                }
            }
            Err(SlabError::HoleCrossesOuter { .. }) => {
                return Err(OpeningError::AcrossEdge {
                    element: element.clone(),
                    host: floor.to_owned(),
                });
            }
            Err(
                error @ (SlabError::TooFewSides {
                    ring: Ring::Hole(_),
                    ..
                }
                | SlabError::ZeroLength {
                    ring: Ring::Hole(_),
                    ..
                }
                | SlabError::Crossing {
                    ring: Ring::Hole(_),
                    ..
                }),
            ) => return Err(own_loop(element, &error)),
            Err(error) => return Err(floors_own(element, floor, &error)),
        }
    }
    let own = holes.len();
    let mut all: Vec<Vec<_>> = holes.iter().map(|h| as_corners(h)).collect();
    for &at in &cutting {
        if let Some(cutter) = cutters.get(at) {
            all.push(as_corners(&cutter.corners));
        }
    }
    let cutter_at = |hole: usize| {
        hole.checked_sub(own)
            .and_then(|k| cutting.get(k))
            .and_then(|&at| cutters.get(at))
            .map(|c| &c.element)
    };
    match Boundary::from_corners(&outer_corners, &all) {
        Ok(_) => Ok(cutting),
        Err(SlabError::HolesOverlap { first, second, .. }) => {
            match (cutter_at(first), cutter_at(second)) {
                (Some(a), Some(b)) => Err(OpeningError::Overlap {
                    element: a.clone(),
                    other: b.clone(),
                }),
                (None, Some(element)) | (Some(element), None) => Err(OpeningError::OverHole {
                    element: element.clone(),
                    host: floor.to_owned(),
                }),
                (None, None) => Err(OpeningError::Malformed {
                    element: cutters
                        .first()
                        .map_or_else(|| Named::opening(""), |c| c.element.clone()),
                    why: format!("{floor}'s own holes overlap"),
                }),
            }
        }
        Err(error) => Err(OpeningError::Malformed {
            element: cutters
                .first()
                .map_or_else(|| Named::opening(""), |c| c.element.clone()),
            why: format!("{floor}'s own boundary: {error}"),
        }),
    }
}

/// How a shaft meets a floor, from their heights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    /// It misses the floor: wholly above or below it, or touching its top or bottom face only.
    Misses,
    /// It passes wholly through the floor's thickness: it cuts every layer.
    Through,
    /// One of its ends lies inside the floor's thickness.
    EndsInside(End),
}

/// How a shaft from `base` up to `top` meets a floor from `bottom` up to `floor_top`, every
/// height from the project's zero.
#[must_use]
pub fn reach(base: Length, top: Length, bottom: Length, floor_top: Length) -> Reach {
    if top <= bottom || base >= floor_top {
        Reach::Misses
    } else if base <= bottom && top >= floor_top {
        Reach::Through
    } else if base > bottom {
        Reach::EndsInside(End::Base)
    } else {
        Reach::EndsInside(End::Top)
    }
}
