//! Inputs for the exact decisions on lines and quadrics (pieces G159 and G166, decision 32.44): lines
//! against a sphere, a cylinder, a cone and a torus, parallel axes, pairs whose axes meet, and the
//! pencils of a sphere and a cylinder, in general position and touching inside.
//!
//! Class `typical` only: each input lies within ±2^20 grid steps of the origin, radii and slopes
//! from 1 to 2^16, so every call succeeds. Refusals and the budgets' extremes are tested in
//! `geom-num` (`tests/attack_g159.rs`).
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the call's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the calls of `geom-num`'s attack test, under no budget. Each is measured as chapter 28.14
//! sets benchmarks out, `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few draws of [`SplitMix64`] per input, outside every measured run.

use geom_num::grid::{P3, Steps};
use geom_num::pencil::{Entry, Form, cylinder_form, sphere_form};
use geom_num::quadric::{Cone, Cylinder, Line, Slope, Sphere, Torus};

use super::{Class, batch_of, p3, rng, within};
use crate::rng::SplitMix64;

/// The reach of an input's coordinates, in grid steps.
const REACH: i64 = 1 << 20;

/// A size in steps.
fn size(g: &mut SplitMix64) -> Steps {
    Steps(g.range(1, 1 << 16).unsigned_abs())
}

/// A point within the reach.
fn point(g: &mut SplitMix64) -> P3 {
    p3(within(g, REACH), within(g, REACH), within(g, REACH))
}

/// Two different points.
fn pair(g: &mut SplitMix64) -> (P3, P3) {
    let a = point(g);
    let mut b = point(g);
    while a == b {
        b = point(g);
    }
    (a, b)
}

fn line(g: &mut SplitMix64) -> Line {
    let (a, b) = pair(g);
    Line { a, b }
}

fn cone(g: &mut SplitMix64) -> Cone {
    let (apex, along) = pair(g);
    let slope = Slope {
        rise: size(g).0,
        run: size(g).0,
    };
    Cone { apex, along, slope }
}

fn cylinder(g: &mut SplitMix64) -> Cylinder {
    let (base, along) = pair(g);
    Cylinder {
        base,
        along,
        radius: size(g),
    }
}

/// A batch of `build`'s values, for the typical class alone.
fn typical<T>(class: Class, name: &str, mut build: impl FnMut(&mut SplitMix64) -> T) -> Vec<T> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(name, class);
    batch_of(|_| Some(build(&mut g)))
}

/// Lines and spheres.
#[must_use]
pub fn spheres(class: Class) -> Vec<(Line, Sphere)> {
    typical(class, "quadric::spheres", |g| {
        let s = Sphere {
            centre: point(g),
            radius: size(g),
        };
        (line(g), s)
    })
}

/// Lines and cylinders.
#[must_use]
pub fn cylinders(class: Class) -> Vec<(Line, Cylinder)> {
    typical(class, "quadric::cylinders", |g| (line(g), cylinder(g)))
}

/// Lines and cones.
#[must_use]
pub fn cones(class: Class) -> Vec<(Line, Cone)> {
    typical(class, "quadric::cones", |g| (line(g), cone(g)))
}

/// Lines and tori.
#[must_use]
pub fn tori(class: Class) -> Vec<(Line, Torus)> {
    typical(class, "quadric::tori", |g| {
        let (centre, along) = pair(g);
        let t = Torus {
            centre,
            along,
            major: size(g),
            minor: size(g),
        };
        (line(g), t)
    })
}

/// A pair of directions, each between two grid points.
#[must_use]
pub fn directions(class: Class) -> Vec<(Line, Line)> {
    typical(class, "quadric::directions", |g| (line(g), line(g)))
}

/// Two points of an axis and a point beside it, as the first line's ends and the second's first.
#[must_use]
pub fn gaps(class: Class) -> Vec<(Line, Line)> {
    typical(class, "quadric::gaps", |g| (line(g), line(g)))
}

/// Cylinders and cones.
#[must_use]
pub fn tangents(class: Class) -> Vec<(Cylinder, Cone)> {
    typical(class, "quadric::tangents", |g| (cylinder(g), cone(g)))
}

/// A sphere and a cylinder through it, about the sphere's centre: four simple roots or two.
#[must_use]
pub fn simple_pencils(class: Class) -> Vec<(Form<Entry>, Form<Entry>)> {
    typical(class, "quadric::simple_pencils", |g| {
        let radius = size(g).0 + (1 << 16);
        let centre = point(g);
        let sphere = Sphere {
            centre,
            radius: Steps(radius),
        };
        let cylinder = Cylinder {
            base: point(g),
            along: point(g),
            radius: size(g),
        };
        (
            sphere_form(&sphere, centre).unwrap_or([[Entry::ZERO; 4]; 4]),
            cylinder_form(&cylinder, centre).unwrap_or([[Entry::ZERO; 4]; 4]),
        )
    })
}

/// Viviani's window: a sphere of radius 2r and a cylinder of radius r through its centre, touching
/// inside, a double root of rank three.
#[must_use]
pub fn nodal_pencils(class: Class) -> Vec<(Form<Entry>, Form<Entry>)> {
    typical(class, "quadric::nodal_pencils", |g| {
        let r = g.range(1, 1 << 16);
        let [x, y, z] = [within(g, REACH), within(g, REACH), within(g, REACH)];
        let centre = p3(x, y, z);
        let sphere = Sphere {
            centre,
            radius: Steps(r.unsigned_abs().saturating_mul(2)),
        };
        let cylinder = Cylinder {
            base: p3(x + r, y, z),
            along: p3(x + r, y, z + 1),
            radius: Steps(r.unsigned_abs()),
        };
        (
            sphere_form(&sphere, centre).unwrap_or([[Entry::ZERO; 4]; 4]),
            cylinder_form(&cylinder, centre).unwrap_or([[Entry::ZERO; 4]; 4]),
        )
    })
}
