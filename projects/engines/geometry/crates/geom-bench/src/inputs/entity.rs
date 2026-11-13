//! Inputs for `geom-2d`'s entity model (piece G194, decision 32.44): a polyline built and checked,
//! and a shape put through a transform's classification.
//!
//! Class `typical` only: each [`polyline`] input is [`VERTICES`] vertices on the grid with a bulge
//! for each segment, each [`under`] input is a polyline, a circle or a segment with a
//! classification drawn from all five invertible kinds, mirrored or not, and each [`arrange`] and
//! [`arrange_curves`] input one drawing.
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

use geom_2d::arrange::Curve;
use geom_2d::carrier::{Circle2, Line2};
use geom_2d::entity::{Ends, Frac, Polyline, Shape};
use geom_2d::point::Pt2;
use geom_math::{Classification, TransformKind};
use geom_num::arc::{Arc2, Circle2 as GridCircle};
use geom_num::carrier::Rational2;
use geom_num::conic::Ellipse2;
use geom_num::grid::P2;

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// The vertices of a benchmark's polyline.
pub const VERTICES: usize = 8;

/// A grid point near the origin.
fn point(g: &mut SplitMix64) -> Option<P2> {
    P2::new(g.range(-1000, 1000), g.range(-1000, 1000)).ok()
}

/// A polyline's vertices and bulges, and the number of the placement to put it through.
pub type Placed = ((Vec<P2>, Vec<Frac>), i64);

/// The vertices and bulges of one polyline: one bulge for each of the open polyline's segments.
fn parts(g: &mut SplitMix64) -> Option<(Vec<P2>, Vec<Frac>)> {
    let vertices = (0..VERTICES)
        .map(|_| point(g))
        .collect::<Option<Vec<P2>>>()?;
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

/// Segments of one drawing: 24 random segments between grid points in a 200 by 200 square.
#[must_use]
pub fn arrange(class: Class) -> Vec<Vec<(P2, P2)>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("entity::arrange", class);
    batch_of(|_| {
        (0..24)
            .map(|_| Some((point_in(&mut g, 200)?, point_in(&mut g, 200)?)))
            .collect::<Option<Vec<(P2, P2)>>>()
    })
}

/// Segments of one drawing to insert one by one: 24 random segments in a 200 by 200 square.
#[must_use]
pub fn zone(class: Class) -> Vec<Vec<(P2, P2)>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("entity::zone", class);
    batch_of(|_| {
        (0..24)
            .map(|_| Some((point_in(&mut g, 200)?, point_in(&mut g, 200)?)))
            .collect::<Option<Vec<(P2, P2)>>>()
    })
}

/// A grid point in the square from 0 to `span`.
fn point_in(g: &mut SplitMix64, span: i64) -> Option<P2> {
    P2::new(g.range(0, span), g.range(0, span)).ok()
}

/// One drawing of segments, arcs and ellipses that `arrange_curves` arranges: 10 segments between
/// grid points of the band from (0, 0) to (200, 600); 5 arcs between grid points of the square from
/// 0 to 100, each about a point of its bisector a whole number of quarter chords from the middle,
/// up to four, so that its circle (radius at most 159, centre at most 142 from the square) stays
/// below y = 400; and 3 ellipses of ratio below one, about centres at least 50 apart in x at heights
/// from 440 to 480, each major axis at most 15 in a part (so at most 22 long), so that no ellipse
/// meets a circle or another ellipse and the segments cross them all.
#[must_use]
pub fn arrange_curves(class: Class) -> Vec<Vec<Curve>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("entity::arrange_curves", class);
    batch_of(|_| {
        let mut curves = Vec::with_capacity(18);
        for _ in 0..10 {
            let a = P2::new(g.range(0, 200), g.range(0, 600)).ok()?;
            let b = P2::new(g.range(0, 200), g.range(0, 600)).ok()?;
            curves.push(Curve::Segment(a, b));
        }
        for _ in 0..5 {
            let (a, b) = (point_in(&mut g, 100)?, point_in(&mut g, 100)?);
            let [px, py, qx, qy] = [a.x.get(), a.y.get(), b.x.get(), b.y.get()].map(i128::from);
            let s = i128::from(g.range(-4, 4));
            let x = 2 * (px + qx) - s * (qy - py);
            let y = 2 * (py + qy) + s * (qx - px);
            let centre = Rational2::new(x, y, 4).ok()?;
            curves.push(Curve::Arc(Arc2::new(a, b, centre).ok()?));
        }
        for i in 0..3 {
            let centre = P2::new(40 + 60 * i + g.range(-5, 5), g.range(440, 480)).ok()?;
            let major = [g.range(-15, 15), g.range(-15, 15)];
            let q = g.range(2, 5);
            let p = g.range(1, q - 1);
            curves.push(Curve::Ellipse(Ellipse2::new(centre, major, p, q).ok()?));
        }
        Some(curves)
    })
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

/// Vertices and bulges for `polyline_length`: the polyline's own shape, drawn apart from
/// [`polyline`]'s so that the two benchmarks have different inputs.
#[must_use]
pub fn length(class: Class) -> Vec<(Vec<P2>, Vec<Frac>)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("entity::length", class);
    batch_of(|_| parts(&mut g))
}

/// A polyline and the number of one of eight placements (`Similarity`): four quarter turns, a
/// mirror in a row, a mirror in a column, a whole scale of three and a shift.
#[must_use]
pub fn place(class: Class) -> Vec<Placed> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("entity::place", class);
    batch_of(|_| Some((parts(&mut g)?, g.range(0, 7))))
}
