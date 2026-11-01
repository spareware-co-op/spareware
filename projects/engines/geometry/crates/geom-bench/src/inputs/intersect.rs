//! Inputs for `geom-intersect`'s hot paths (pieces G158 and G162, decision 32.44): a list of items
//! put in canonical order, a circle's canonical key, a pair of lines put in canonical order, and a
//! point projected on an arc and on a band of a sphere.
//!
//! Class `typical` only: each list is [`ITEMS`] disjoint points in a shuffled order, each circle,
//! line, sphere and point within ±2^20 grid steps of the origin, so every call succeeds. Refusals are tested in
//! `geom-intersect` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the call's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the calls of `geom-intersect`'s attack tests (`tests/attack_g158.rs`), under an
//! unlimited budget. Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few draws of [`SplitMix64`] per input, outside every measured run.

use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_intersect::{Item, ParamBox, PointContact};
use geom_math::Box3;
use geom_num::Exactness;
use geom_num::interval::Interval;
use geom_surface::{Sphere, SurfaceKind};

use super::{Class, batch_of, p3, rng, within};
use crate::rng::SplitMix64;

/// The reach of an input's coordinates, in grid steps.
const REACH: i64 = 1 << 20;

/// How many items a list holds.
const ITEMS: u32 = 32;

/// A coordinate within the reach. It fits `i32`, so it converts exactly.
fn coordinate(g: &mut SplitMix64) -> f64 {
    f64::from(i32::try_from(within(g, REACH)).unwrap_or(0))
}

/// A point within the reach.
fn point(g: &mut SplitMix64) -> [f64; 3] {
    [coordinate(g), coordinate(g), coordinate(g)]
}

/// `places` in a shuffled order, by Fisher and Yates's method.
fn shuffled(g: &mut SplitMix64, mut places: Vec<u32>) -> Option<Vec<u32>> {
    for i in (1..places.len()).rev() {
        let j = usize::try_from(g.range(0, i64::try_from(i).ok()?)).ok()?;
        places.swap(i, j);
    }
    Some(places)
}

/// The point item at `place`.
fn item(place: u32, xyz: Box3) -> Option<Item> {
    let start = f64::from(place);
    let spans = [
        Interval::new(start, start + 0.5).ok()?,
        Interval::new(0.0, 1.0).ok()?,
    ];
    Some(Item::Point {
        at: ParamBox::new(&spans).ok()?,
        xyz,
        contact: PointContact::Crossing,
        class: Exactness::Exact,
    })
}

/// Lists of [`ITEMS`] disjoint points, shuffled, for `Intersection::new`.
#[must_use]
pub fn items(class: Class) -> Vec<Vec<Item>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::items", class);
    batch_of(|_| {
        let xyz = Box3::new(p3(0, 0, 0), p3(1, 1, 1)).ok()?;
        let places = shuffled(&mut g, (0..ITEMS).collect())?;
        places.into_iter().map(|p| item(p, xyz)).collect()
    })
}

/// Circles for `CanonicalKey::of_curve`.
#[must_use]
pub fn circles(class: Class) -> Vec<CurveKind<3>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::circles", class);
    batch_of(|_| {
        let centre = point(&mut g);
        let radius = f64::from(i32::try_from(g.range(1, 1 << 16)).ok()?);
        let circle = Circle::full(centre, radius, Frame::standard().ok()?).ok()?;
        Some(CurveKind::Circle(circle))
    })
}

/// Pairs of lines for `pair`.
#[must_use]
pub fn lines(class: Class) -> Vec<(CurveKind<3>, CurveKind<3>)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::lines", class);
    let mut line = move || {
        let run = (
            f64::from(i32::try_from(g.range(1, 1 << 16)).ok()?),
            1.0,
            2.0,
        );
        Line::new(point(&mut g), [run.0, run.1, run.2], (0.0, 1.0))
            .ok()
            .map(CurveKind::Line)
    };
    batch_of(|_| Some((line()?, line()?)))
}

/// A radius of up to 2^16 grid steps.
fn radius(g: &mut SplitMix64) -> Option<f64> {
    i32::try_from(g.range(1, 1 << 16)).ok().map(f64::from)
}

/// Three quarters of a circle and a point to project on it.
#[must_use]
pub fn arcs(class: Class) -> Vec<(CurveKind<3>, [f64; 3])> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::arcs", class);
    batch_of(|_| {
        let arc = Circle::new(
            point(&mut g),
            radius(&mut g)?,
            Frame::standard().ok()?,
            (0.0, 1.5),
        );
        Some((CurveKind::Circle(arc.ok()?), point(&mut g)))
    })
}

/// A band of a sphere and a point to project on it.
#[must_use]
pub fn spheres(class: Class) -> Vec<(SurfaceKind, [f64; 3])> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::spheres", class);
    batch_of(|_| {
        let band = Sphere::new(
            point(&mut g),
            radius(&mut g)?,
            Frame::standard().ok()?,
            ((0.0, 1.5), (-0.4, 0.4)),
        );
        Some((SurfaceKind::Sphere(band.ok()?), point(&mut g)))
    })
}
