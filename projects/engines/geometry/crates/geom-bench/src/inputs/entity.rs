//! Inputs for `geom-2d`'s entity model (piece G194, decision 32.44): a polyline built and checked,
//! and a shape put through a transform's classification.
//!
//! Class `typical` only: each [`polyline`] input is [`VERTICES`] vertices on the grid with a bulge
//! for each segment, and each [`under`] input is a polyline, a circle or a segment with a
//! classification drawn from all five invertible kinds, mirrored or not.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the entity table of chapter 14.1 and the transform rules of 14.10, measured as chapter
//! 28.14 sets benchmarks out, `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few dozen draws of [`SplitMix64`](crate::rng::SplitMix64) per input, once per batch,
//! outside every measured run.

use geom_2d::carrier::{Circle2, Line2};
use geom_2d::entity::{Ends, Frac, Polyline, Shape};
use geom_2d::point::Pt2;
use geom_math::{Classification, TransformKind};
use geom_num::arc::Circle2 as GridCircle;
use geom_num::grid::P2;

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// The vertices of a benchmark's polyline.
pub const VERTICES: usize = 8;

/// A grid point near the origin.
fn point(g: &mut SplitMix64) -> Option<P2> {
    P2::new(g.range(-1000, 1000), g.range(-1000, 1000)).ok()
}

/// The vertices and bulges of one polyline: one bulge for each of the open polyline's segments.
fn parts(g: &mut SplitMix64) -> Option<(Vec<P2>, Vec<Frac>)> {
    let vertices = (0..VERTICES).map(|_| point(g)).collect::<Option<Vec<P2>>>()?;
    let bulges = (1..VERTICES)
        .map(|_| Frac::new(g.range(-3, 3), g.range(1, 5)).ok())
        .collect::<Option<Vec<Frac>>>()?;
    Some((vertices, bulges))
}

/// Vertices and bulges for `Polyline::new`.
#[must_use]
pub fn polyline(class: Class) -> Vec<(Vec<P2>, Vec<Frac>)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("entity::polyline", class);
    batch_of(|_| parts(&mut g))
}

/// A shape and the classification of a transform to put it through.
#[must_use]
pub fn under(class: Class) -> Vec<(Shape, Classification)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("entity::under", class);
    batch_of(|_| {
        let shape = match g.range(0, 2) {
            0 => {
                let (vertices, bulges) = parts(&mut g)?;
                Shape::Polyline(Polyline::new(vertices, bulges, None, false).ok()?)
            }
            1 => Shape::Circle(Circle2::Grid(
                GridCircle::new(point(&mut g)?, i128::from(g.range(1, 500))).ok()?,
            )),
            _ => Shape::Line {
                carrier: Line2::new(0, 1, 0).ok()?,
                ends: Ends::Segment(Pt2::Grid(point(&mut g)?), Pt2::Grid(point(&mut g)?)),
            },
        };
        let kind = match g.range(0, 4) {
            0 => TransformKind::Identity,
            1 => TransformKind::Translation,
            2 => TransformKind::Rigid,
            3 => TransformKind::UniformScale,
            _ => TransformKind::NonUniform,
        };
        Some((
            shape,
            Classification {
                kind,
                mirror: g.range(0, 1) == 1,
            },
        ))
    })
}
