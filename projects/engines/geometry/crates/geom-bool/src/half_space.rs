//! Half-spaces: everything on one side of a plane, as a solid operand (11.1).
//!
//! What it does: [`half_space`] builds the solid bounded by a plane and the frame's range box, with
//! corners at ±(2^40 − 1) steps. It is a body like any other, so it enters a boolean as a solid and
//! stays exact: IFC's half-space that clips a wall under a roof is one of these, and its boxed
//! half-space is the same one, since the box there is only a search aid.
//!
//! How it runs: each side of the range box is clipped to the plane, and the plane itself, bounded
//! by the four box sides least square to it, is clipped to the other two. Those pieces go straight
//! to [`rebuild`]: they already bound the solid, so no arrangement is needed.
//!
//! Guarantees:
//!
//! - **Exact.** The plane is the one through three grid points, and every corner is a meeting of
//!   integer planes.
//! - **Below the normal.** The material lies on the plane's negative side, so a plane through
//!   `a`, `b`, `c` keeps what is behind (b − a) × (c − a).
//! - **Refused, never guessed.** Three points on one line are [`RebuildError::Num`].
//!
//! Method: chapter 11.1 (`projects/engines/geometry/architecture/11-booleans.md`), and the IFC
//! entities it cites
//! ([IfcBoxedHalfSpace](https://github.com/buildingSMART/IFC4.3.x-development/blob/5d08761e63557bd3ded140139eede665ec31511c/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcBoxedHalfSpace.md)).
//!
//! Cost: constant, and what [`rebuild`] costs on seven pieces.

use alloc::vec::Vec;

use geom_num::grid::COORD_MAX;
use geom_num::plane::Plane;
use geom_num::{NumError, P3};
use geom_poly::convex::Piece;
use geom_poly::cut::{Clipped, clip};
use geom_poly::merge::{Merge, Outward};
use geom_poly::winding::Winding;
use geom_run::WorkMeter;
use geom_topo::{Body, FlatSurface as Flat, Sense, SurfaceKind};

use crate::rebuild::{Carrier, RebuildError, rebuild};

/// The widest domain a flat carrier in the range box needs: past the box's diagonal on any axis.
const DOMAIN: (f64, f64) = (-1e13, 1e13);

/// A plane square to `axis`, at `at`, its normal along +`axis` when `upward`.
fn square(axis: usize, at: i64, upward: bool) -> Result<Plane, NumError> {
    let (one, offset) = if upward {
        (1, i128::from(at).saturating_neg())
    } else {
        (-1, i128::from(at))
    };
    let normal = [0usize, 1, 2].map(|k| if k == axis { one } else { 0 });
    Plane::new(normal, offset)
}

/// The sides of the range box around `axis`, inside positive, in turn about +`axis`.
fn around_axis(axis: usize) -> Result<[Plane; 4], NumError> {
    let (u, v) = (axis.saturating_add(1) % 3, axis.saturating_add(2) % 3);
    Ok([
        square(v, -COORD_MAX, true)?,
        square(u, COORD_MAX, false)?,
        square(v, COORD_MAX, false)?,
        square(u, -COORD_MAX, true)?,
    ])
}

/// A carrier for a side of the box square to `axis`.
fn side_carrier(axis: usize, upward: bool) -> Result<Carrier, NumError> {
    let at = |k: usize, v: f64| [0usize, 1, 2].map(|i| if i == k { v } else { 0.0 });
    let (u, v) = (axis.saturating_add(1) % 3, axis.saturating_add(2) % 3);
    #[allow(
        clippy::cast_precision_loss,
        reason = "the range box's corner is below 2^40, which a double holds"
    )]
    let reach = COORD_MAX as f64;
    let origin = at(axis, if upward { reach } else { -reach });
    let across = at(v, if upward { 1.0 } else { -1.0 });
    let surface = Flat::new(origin, at(u, 1.0), across, (DOMAIN, DOMAIN));
    Ok(Carrier {
        surface: SurfaceKind::Plane(surface.map_err(|_| NumError::Degenerate)?),
        sense: Sense::Forward,
    })
}

/// A carrier for the plane through `points`.
fn cap_carrier(points: [P3; 3]) -> Result<Carrier, NumError> {
    #[allow(
        clippy::cast_precision_loss,
        reason = "a coordinate is below 2^40, and a difference below 2^41, which a double holds"
    )]
    let coordinates = |p: P3| [p.x.get(), p.y.get(), p.z.get()].map(|v| v as f64);
    let [a, b, c] = points.map(coordinates);
    let [ax, ay, az] = a;
    let [bx, by, bz] = b;
    let [cx, cy, cz] = c;
    let surface = Flat::new(
        a,
        [bx - ax, by - ay, bz - az],
        [cx - ax, cy - ay, cz - az],
        (DOMAIN, DOMAIN),
    );
    Ok(Carrier {
        surface: SurfaceKind::Plane(surface.map_err(|_| NumError::Degenerate)?),
        sense: Sense::Forward,
    })
}

/// What of `piece` lies on the positive side of `plane`, the whole of it if it lies in the plane
/// and `on`.
fn kept_of(piece: Piece, plane: &Plane, on: bool) -> Result<Option<Piece>, NumError> {
    Ok(match clip(&piece, plane)? {
        Clipped::Front => Some(piece),
        Clipped::Split(front, _) => Some(front),
        Clipped::On if on => Some(piece),
        Clipped::Back | Clipped::On => None,
    })
}

/// The cap: `plane` bounded by the range box.
fn cap(plane: &Plane) -> Result<Option<Piece>, NumError> {
    let n = plane.n();
    let largest = |k: usize| n.get(k).map_or(0, |v| v.unsigned_abs());
    let mut axis = 0usize;
    for k in [1usize, 2] {
        if largest(k) > largest(axis) {
            axis = k;
        }
    }
    let mut bounds: Vec<Plane> = around_axis(axis)?.to_vec();
    if n.get(axis).is_some_and(|v| *v < 0) {
        bounds.reverse();
    }
    let piece = Piece {
        face: *plane,
        bounds,
    };
    // A plane on a side of the box is that side: the cap takes its place.
    let piece = kept_of(piece, &square(axis, COORD_MAX, false)?, true)?;
    piece
        .map(|p| kept_of(p, &square(axis, -COORD_MAX, true)?, true))
        .transpose()
        .map(Option::flatten)
}

/// The solid behind the plane through `points`, inside the range box.
///
/// # Errors
///
/// [`RebuildError::Num`] for three points on one line.
pub fn half_space(points: [P3; 3], meter: &mut WorkMeter<'_>) -> Result<Body, RebuildError> {
    let [a, b, c] = points;
    let plane = Plane::through(a, b, c)?;
    let behind = Plane::new(
        plane.n().map(i128::saturating_neg),
        plane.d().saturating_neg(),
    )?;
    let mut carriers = Vec::with_capacity(7);
    let mut pieces = Vec::with_capacity(7);
    let mut keep = |piece: Piece, carrier: Carrier| {
        pieces.push(Outward {
            piece,
            polygon: carriers.len(),
            turned: false,
            front: Winding::new(),
            back: Winding::new(),
        });
        carriers.push(carrier);
    };
    for axis in 0..3 {
        for upward in [true, false] {
            let mut bounds = around_axis(axis)?.to_vec();
            if !upward {
                bounds.reverse();
            }
            let at = if upward { COORD_MAX } else { -COORD_MAX };
            let side = Piece {
                face: square(axis, at, upward)?,
                bounds,
            };
            if let Some(kept) = kept_of(side, &behind, false)? {
                keep(kept, side_carrier(axis, upward)?);
            }
        }
    }
    if let Some(kept) = cap(&plane)? {
        keep(kept, cap_carrier(points)?);
    }
    rebuild(&pieces, &carriers, Merge::RemoveRedundant, meter)
}
