//! Boxes around a curve or a surface over a span an answer writes on its seam, for the targets
//! that hold an answer's points to the shapes they lie on (crossings, natural, piercing).
//!
//! What it does: an answer writes a span on the seam of its curve or surface
//! ([`geom_intersect::Seam`]): on a whole turn it starts in the window and may run past the end
//! (decision 32.104). [`curve_box`] and [`surface_box`] cut such a span into the stretches of the
//! domain it covers, bound the shape over each and join the boxes, so a target can ask whether a
//! point's boxes on two shapes meet. The helpers live in a module of their own, named only by the
//! targets that use them, so every other target's build stays without the crates they name.
//!
//! Guarantees:
//!
//! - **Every stretch is bounded.** A span across a whole turn's join is two stretches, one each
//!   side of it, and both are in the box; a surface's spans give up to two stretches each, and
//!   every pair is bounded.
//! - **A typed finding, never a panic.** A stretch the shape cannot bound, or boxes that cannot be
//!   joined, is the finding "a point's box is in the domain".
//!
//! Method: decision 32.104 (`projects/engines/geometry/architecture/32-decisions.md`), a span on a
//! whole turn read modulo the turn: [`geom_intersect::Seam::spans`] for the stretches, each
//! shape's own `bound` over them, and the boxes' union.
//!
//! Cost: at most two bounds of a curve and four of a surface, each as its kind bounds one span.

use core::fmt;

use geom_curve::ParamCurve;
use geom_intersect::Seam;
use geom_num::interval::Interval;
use geom_nurbs::Bounds;
use geom_run::WorkMeter;
use geom_surface::{ParamSurface, SurfaceKind};

use crate::Finding;

/// The finding that a point's box could not be bounded over the domain it covers.
fn outside(e: impl fmt::Debug) -> Finding {
    Finding::new("a point's box is in the domain", || format!("{e:?}"))
}

/// A box around `curve` over `span`, a span an answer writes on `seam`, the curve's
/// ([`Seam::of_curve`]). On a whole turn it starts in the window and may run past the end
/// (decision 32.104), so each stretch of the domain it covers is bounded and the boxes joined.
///
/// # Errors
///
/// The finding "a point's box is in the domain" if the curve cannot bound a stretch or the boxes
/// cannot be joined.
pub fn curve_box<C: ParamCurve<D>, const D: usize>(
    curve: &C,
    seam: Seam,
    span: Interval,
    meter: &mut WorkMeter<'_>,
) -> Result<Bounds<D>, Finding> {
    let (first, second) = pieces(seam, span);
    let mut out = curve.bound(first, meter).map_err(outside)?;
    if let Some(part) = second {
        let more = curve.bound(part, meter).map_err(outside)?;
        out = out.union(&more).map_err(outside)?;
    }
    Ok(out)
}

/// The stretches of the domain `span` covers on `seam`: the first always, and a second round a
/// whole turn's join.
fn pieces(seam: Seam, span: Interval) -> ((f64, f64), Option<(f64, f64)>) {
    let [first, second] = seam.spans(span);
    (first.unwrap_or((span.lo(), span.hi())), second)
}

/// A box around `surface` over the spans `u` and `v` an answer writes on its seams
/// ([`Seam::of_surface`]): each pair of the stretches of the domain they cover, up to two each
/// round a whole turn (decision 32.104), bounded and the boxes joined.
///
/// # Errors
///
/// The finding "a point's box is in the domain" if the surface cannot bound a pair of stretches or
/// the boxes cannot be joined.
pub fn surface_box(
    surface: &SurfaceKind,
    (u, v): (Interval, Interval),
    meter: &mut WorkMeter<'_>,
) -> Result<Bounds<3>, Finding> {
    let [su, sv] = Seam::of_surface(surface);
    let ((u0, u1), (v0, v1)) = (pieces(su, u), pieces(sv, v));
    let mut out = surface.bound((u0, v0), meter).map_err(outside)?;
    for pair in [u1.map(|p| (p, v0)), v1.map(|p| (u0, p)), u1.zip(v1)]
        .into_iter()
        .flatten()
    {
        let more = surface.bound(pair, meter).map_err(outside)?;
        out = out.union(&more).map_err(outside)?;
    }
    Ok(out)
}
