//! Step 1 of healing (19.7): coordinates rounded into the frame, once.
//!
//! What it does: [`quantise()`] takes each coordinate as a double in some unit, with how many grid
//! steps make that unit, and rounds the exact product to the nearest step, ties to even. The
//! result is a [`RawMesh`] on the grid and a report of what moved.
//!
//! Guarantees:
//!
//! - **Rounded once.** A double is taken at its exact value (24.8, "Numbers to grid"); the product
//!   with the unit is exact, by [`DD::exact_product`], and is rounded once, never through a second
//!   double. Each coordinate moves by at most half a step.
//! - **Reported.** The result says how many coordinates moved and the largest move; a quantised
//!   import is Exact relative to these grid points, and the move is recorded apart (24.8, "import
//!   deviation").
//! - **Refused, never clamped.** A coordinate that is not a number, or that rounds outside the
//!   frame, fails with its vertex.
//!
//! Method: 24.8's numbers to grid for binary floats
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`); Dekker's exact product
//! through a fused multiply-add (<https://doi.org/10.1007/BF01397083>), then a rounding of the
//! exact double-double ([`DD::round_shifted`]).
//!
//! Cost: one exact product and one rounding per coordinate; no allocation but the result's.

use alloc::vec::Vec;

use geom_num::dd::DD;
use geom_num::grid::{Coord, P3};

use super::{HealError, RawMesh};

/// A mesh rounded onto the grid, and what the rounding moved.
#[derive(Clone, Debug, PartialEq)]
pub struct Quantised {
    /// The mesh on the grid.
    pub mesh: RawMesh,
    /// How many coordinates were not whole steps already.
    pub moved: u32,
    /// The largest move of one coordinate, in steps: at most 1/2.
    pub largest: f64,
}

/// `points`, each coordinate in units of `unit` grid steps, rounded to the nearest step (ties to
/// even) as a raw mesh of the polygons `corners[face_start[f]..face_start[f + 1]]`. A unit is a
/// whole number of steps: a millimetre is 6,400 steps of `Q0` (`GridStep::steps_per`).
///
/// # Errors
///
/// [`HealError::NoUnit`] for a unit of 0; [`HealError::NotFinite`] for a NaN or infinite
/// coordinate; [`HealError::OutOfFrame`] for one that rounds past ±(2^40 − 1) steps; and the raw
/// mesh's own faults for corners and offsets that do not fit.
pub fn quantise(
    points: &[[f64; 3]],
    unit: u32,
    corners: Vec<u32>,
    face_start: Vec<u32>,
) -> Result<Quantised, HealError> {
    if unit == 0 {
        return Err(HealError::NoUnit);
    }
    let scale = f64::from(unit);
    let mut vertices = Vec::new();
    vertices
        .try_reserve(points.len())
        .map_err(|_| crate::exact::MeshError::OutOfMemory)?;
    let (mut moved, mut largest) = (0_u32, 0.0_f64);
    for (vertex, point) in (0_u32..).zip(points) {
        let mut steps = [0_i64; 3];
        for (slot, &value) in steps.iter_mut().zip(point) {
            if !value.is_finite() {
                return Err(HealError::NotFinite { vertex });
            }
            let (coord, off) = rounded(value, scale).ok_or(HealError::OutOfFrame { vertex })?;
            if let Some(off) = off {
                moved = moved.saturating_add(1);
                if off > largest {
                    largest = off;
                }
            }
            *slot = coord.get();
        }
        let [x, y, z] = steps;
        vertices.push(P3::new(x, y, z)?);
    }
    let mesh = RawMesh::new(vertices, corners, face_start)?;
    Ok(Quantised {
        mesh,
        moved,
        largest,
    })
}

/// `value` times `scale` rounded to the nearest step, ties to even, with how far it moved when it
/// moved; `None` outside the frame. `value` is finite, so the product is `None` only when it
/// overflows.
fn rounded(value: f64, scale: f64) -> Option<(Coord, Option<f64>)> {
    let product = DD::exact_product(value, scale)?;
    let near = product.round_shifted(0)?;
    let coord = Coord::from_i128(near.value()).ok()?;
    if near.is_exact() {
        return Some((coord, None));
    }
    let off = product.sub(DD::from_i64(coord.get())).ok()?.abs().to_f64();
    Some((coord, Some(off)))
}
