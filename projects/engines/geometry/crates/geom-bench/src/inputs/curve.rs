//! Inputs for `geom-curve`'s hot paths (pieces G60 to G64 and G67, decision 32.44): parameters
//! on an exact helix (its point, and its certified enclosure with two derivatives), bulge
//! polylines for their conversion to NURBS, stations on a clothoid transition, and transition
//! lengths for their conversion to NURBS.
//!
//! The helix and the transition are fixed per operation and built here, as `geom-curve`'s benchmarks build them. Class `typical` only:
//!
//! - Helix parameters: in its domain [0, 8] (four turns), at 2^−49 resolution.
//! - Bulge polylines: four vertices within ±10 of the origin, each bulge in (−1, 1), closed.
//! - Stations: in the transition's length [0, 120], at 2^−45 resolution.
//! - Lengths: in [50, 200], each making its own clothoid from straight to radius 650.
//!
//! The kinds' refusals (a parameter outside the domain, a degenerate polyline, a transition too
//! long for its panel cap) are tested in `geom-curve` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the cases of `geom-curve`'s benchmarks (`benches/analytic.rs`, `compound.rs`,
//! `enclosure.rs` and `transition.rs`), at one size each.
//! Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few draws of [`SplitMix64`] per value, once per batch, outside every measured run.

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;
use geom_curve::{CurvatureLaw, CurveError, CurveKind, Frame, Helix, HelixAxes, Transition};
use geom_run::{Budget, WorkMeter};

/// A value in [0, 1) at 2^−52 resolution.
fn unit(g: &mut SplitMix64) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "the value is below 2^53, so the conversion is exact"
    )]
    let v = (g.next_u64() >> 12) as f64;
    v / (1u64 << 52) as f64
}

/// `count` values in [`lo`, `hi`) for the operation named `op`.
fn values(op: &str, class: Class, lo: f64, hi: f64) -> Vec<f64> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(op, class);
    batch_of(|_| Some(lo + (hi - lo) * unit(&mut g)))
}

/// Parameters for `CurveKind::point` on [`helix`].
#[must_use]
pub fn helix_point(class: Class) -> Vec<f64> {
    values("curve::helix_point", class, 0.0, 8.0)
}

/// Parameters for `CurveKind::enclose` on [`helix`].
#[must_use]
pub fn helix_enclose(class: Class) -> Vec<f64> {
    values("curve::helix_enclose", class, 0.0, 8.0)
}

/// Bulge polylines (vertices, bulges) for `CurveKind::to_nurbs`.
#[must_use]
pub fn bulge_to_nurbs(class: Class) -> Vec<(Vec<[f64; 2]>, Vec<f64>)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("curve::bulge_to_nurbs", class);
    batch_of(|_| {
        let vertices = (0..4)
            .map(|_| [20.0 * unit(&mut g) - 10.0, 20.0 * unit(&mut g) - 10.0])
            .collect();
        let bulges = (0..4).map(|_| 2.0 * unit(&mut g) - 1.0).collect();
        Some((vertices, bulges))
    })
}

/// Stations for `Transition::point` on [`transition`] of length 120.
#[must_use]
pub fn transition_point(class: Class) -> Vec<f64> {
    values("curve::transition_point", class, 0.0, 120.0)
}

/// Lengths for `Transition::to_nurbs`, each its own clothoid.
#[must_use]
pub fn transition_to_nurbs(class: Class) -> Vec<f64> {
    values("curve::transition_to_nurbs", class, 50.0, 200.0)
}

/// The helix `geom-curve`'s benchmark measures: four turns of radius 2 and pitch 0.5.
#[must_use]
pub fn helix() -> Option<CurveKind<3>> {
    let frame = Frame::new([3.0, 4.0, 0.0], [0.0, 0.0, 1.0]).ok()?;
    let axes = HelixAxes::Frame {
        frame,
        radius: 2.0,
        pitch: 0.5,
    };
    Helix::new([1.0, -2.0, 3.5], axes, (0.0, 8.0))
        .ok()
        .map(CurveKind::Helix)
}

/// The clothoid from straight to radius 650 of `length`, starting at (1000, 2000) towards
/// (3, 4), as `geom-curve`'s benchmark builds it.
///
/// # Errors
///
/// The transition's own refusals; none for a length from 50 to 200.
pub fn transition(length: f64) -> Result<Transition<2>, CurveError> {
    let law = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 1.0 / 650.0,
    };
    Transition::new(
        [1000.0, 2000.0],
        [3.0, 4.0],
        length,
        law,
        &mut WorkMeter::new(Budget::UNLIMITED),
    )
}
