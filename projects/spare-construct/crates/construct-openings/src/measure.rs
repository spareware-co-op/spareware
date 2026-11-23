//! What a host holds after its openings are cut, exactly, in construct-params' units: what the
//! shape the kernel builds must agree with, layer by layer (the kernel builder's suite measures
//! the built solids against the same numbers).
//!
//! - A layer of a wall along an axis, `length` long, `height` high and `thickness` thick, with
//!   rectangular openings through it ([`wall_layer`]): each face of the layer (its exterior or its
//!   interior side) is `length x height` less every opening's `width x height`, and its volume is
//!   that face by its thickness. The openings lie inside the wall and apart from each other, as
//!   the builder's own checks have passed.
//! - A layer of a floor whose boundary has holes and openings cut in it ([`floor_layer`]): its top
//!   and its bottom face are the outer loop's area less every hole's and every cut outline's, and
//!   its volume is that area by its thickness. The outlines lie inside the floor and apart, as
//!   [`crate::fit::fit_in_floor`] has passed.

use crate::outline::{as_corners, twice_signed_area};
use construct_params::{Area, Length, Volume};
use construct_slabs::{Boundary, Corner, SlabError};

/// A layer after its openings are cut: one of its two large faces, and its volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerAfter {
    /// Each of its two large faces: a wall layer's side, a floor layer's top or bottom.
    pub face: Area,
    /// Its volume.
    pub volume: Volume,
}

/// A wall layer of `length` by `height` by `thickness` with `openings` (each width and height)
/// cut through it; `None` past the range of an area or a volume.
#[must_use]
pub fn wall_layer(
    length: Length,
    height: Length,
    thickness: Length,
    openings: &[(Length, Length)],
) -> Option<LayerAfter> {
    let mut face = length.checked_mul(height)?;
    for &(width, tall) in openings {
        face = face.checked_sub(width.checked_mul(tall)?)?;
    }
    Some(LayerAfter {
        face,
        volume: face.checked_mul(thickness)?,
    })
}

/// A floor layer `thickness` thick whose boundary is `outer` with `holes`, and `cuts` cut through
/// it, every loop's corners in grid steps; `None` when the area is not a whole number of square
/// steps or is past the range of an area or a volume.
#[must_use]
pub fn floor_layer(
    outer: &[[i64; 2]],
    holes: &[Vec<[i64; 2]>],
    cuts: &[Vec<[i64; 2]>],
    thickness: Length,
) -> Option<LayerAfter> {
    let mut twice = twice_signed_area(outer).checked_abs()?;
    for ring in holes.iter().chain(cuts) {
        twice = twice.checked_sub(twice_signed_area(ring).checked_abs()?)?;
    }
    if twice.checked_rem(2)? != 0 {
        return None;
    }
    let face = Area::from_square_steps(twice.checked_div(2)?);
    Some(LayerAfter {
        face,
        volume: face.checked_mul(thickness)?,
    })
}

/// A floor's `boundary` with `cuts` (openings and shafts that cut it, grid steps, its own frame)
/// as more holes: the boundary every layer of the cut floor has, so what a floor reads back
/// (construct-slabs' area, perimeter, volume and mass) is worked out of it by the same code,
/// exactly.
///
/// # Errors
/// [`SlabError`] when the loops with the cuts do not pass the floor's own checks: the cuts are
/// fitted first ([`crate::fit_in_floor`]), so this does not happen for a floor that builds.
pub fn with_cuts(boundary: &Boundary, cuts: &[Vec<[i64; 2]>]) -> Result<Boundary, SlabError> {
    let mut loops: Vec<Vec<Corner>> = boundary
        .profile()
        .into_iter()
        .map(|ring| {
            ring.into_iter()
                .map(|(id, at)| Corner { id: Some(id), at })
                .collect()
        })
        .collect();
    let outer = if loops.is_empty() {
        Vec::new()
    } else {
        loops.remove(0)
    };
    loops.extend(cuts.iter().map(|cut| as_corners(cut)));
    Boundary::from_corners(&outer, &loops)
}
