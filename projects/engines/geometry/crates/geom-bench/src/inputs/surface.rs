//! Inputs for `geom-surface`'s hot paths (pieces G65 to G68, decision 32.44): parameter pairs on
//! a torus (its point, and its certified enclosure with first partials) and on an offset cylinder
//! (its point).
//!
//! The surfaces are fixed per operation and built here, as
//! `geom-surface`'s benchmarks build them. Class `typical` only: pairs drawn from each surface's
//! parameter box at 2^−50 resolution. The kinds' refusals (a parameter outside the domain, an
//! offset through a focal point) are tested in `geom-surface` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the cases of `geom-surface`'s benchmarks (`benches/surfaces.rs`, `offsets.rs`), at one
//! size each.
//! Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: two draws of [`SplitMix64`] per pair, once per batch, outside every measured run.

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;
use geom_curve::Frame;
use geom_surface::{Cylinder, Offset, ParamBox, SurfaceKind, Torus};

/// A value in [0, 1) at 2^−52 resolution.
fn unit(g: &mut SplitMix64) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "the value is below 2^53, so the conversion is exact"
    )]
    let v = (g.next_u64() >> 12) as f64;
    v / (1u64 << 52) as f64
}

/// Parameter pairs in the box `((u0, u1), (v0, v1))` for the operation named `op`.
fn pairs(op: &str, class: Class, (u0, u1): (f64, f64), (v0, v1): (f64, f64)) -> Vec<(f64, f64)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(op, class);
    batch_of(|_| Some((u0 + (u1 - u0) * unit(&mut g), v0 + (v1 - v0) * unit(&mut g))))
}

/// Pairs for `SurfaceKind::point` on [`torus`].
#[must_use]
pub fn torus_point(class: Class) -> Vec<(f64, f64)> {
    pairs("surface::torus_point", class, (0.0, 1.5), (-1.0, 1.0))
}

/// Pairs for `SurfaceKind::enclose` on [`torus`].
#[must_use]
pub fn torus_enclose(class: Class) -> Vec<(f64, f64)> {
    pairs("surface::torus_enclose", class, (0.0, 1.5), (-1.0, 1.0))
}

/// Parameter boxes inside `((u0, u1), (v0, v1))`, each side at most `side` long, for the operation
/// named `op`.
fn boxes(
    op: &str,
    class: Class,
    (u0, u1): (f64, f64),
    (v0, v1): (f64, f64),
    side: f64,
) -> Vec<ParamBox> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(op, class);
    batch_of(|_| {
        // A width in (0, side], then a start that keeps the box inside.
        let du = side * (1.0 - unit(&mut g));
        let dv = side * (1.0 - unit(&mut g));
        let u = u0 + (u1 - u0 - du) * unit(&mut g);
        let v = v0 + (v1 - v0 - dv) * unit(&mut g);
        Some(((u, u + du), (v, v + dv)))
    })
}

/// Boxes for `ParamSurface::second_derivative_bound` on [`torus`] (piece G68).
#[must_use]
pub fn torus_second_bound(class: Class) -> Vec<ParamBox> {
    boxes(
        "surface::torus_second_bound",
        class,
        (0.0, 1.5),
        (-1.0, 1.0),
        0.1,
    )
}

/// Boxes for `ParamSurface::normal_derivative_bound` on [`torus`] (piece G68).
#[must_use]
pub fn torus_normal_bound(class: Class) -> Vec<ParamBox> {
    boxes(
        "surface::torus_normal_bound",
        class,
        (0.0, 1.5),
        (-1.0, 1.0),
        0.1,
    )
}

/// Pairs for `SurfaceKind::point` on [`offset_cylinder`].
#[must_use]
pub fn offset_point(class: Class) -> Vec<(f64, f64)> {
    pairs("surface::offset_point", class, (0.0, 0.5), (0.0, 2.0))
}

/// The frame `geom-surface`'s benchmarks use: centred on (1, 0, 0), axis z.
fn frame() -> Option<Frame<3>> {
    Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).ok()
}

/// The torus `geom-surface`'s benchmark measures: radii 5 and 2, part of a turn each way.
#[must_use]
pub fn torus() -> Option<SurfaceKind> {
    Torus::new([0.0; 3], 5.0, 2.0, frame()?, ((0.0, 1.5), (-1.0, 1.0)))
        .ok()
        .map(SurfaceKind::Torus)
}

/// The offset by 0.3 of the cylinder `geom-surface`'s benchmark measures.
#[must_use]
pub fn offset_cylinder() -> Option<SurfaceKind> {
    let cylinder = Cylinder::new([0.0; 3], 3.0, frame()?, ((0.0, 0.5), (0.0, 2.0))).ok()?;
    Offset::new(SurfaceKind::Cylinder(cylinder), 0.3)
        .ok()
        .map(SurfaceKind::Offset)
}
