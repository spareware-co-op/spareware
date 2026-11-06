//! Two patches of one carrier coincide by their data, and `subdivide` says so before it spends any
//! work: a meter of 1 would refuse anything it had to search. Where the data differ, the patches only
//! touch, or a patch wraps onto itself, nothing is claimed and the ordinary search runs.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::IntersectError;
use geom_intersect::general::{Subdivided, subdivide};
use geom_intersect::result::{StepsInterval, Unresolved};
use geom_num::grid::Steps;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, ParamSurface, Plane, Sphere, SurfaceKind, Torus};

const UNIT: f64 = 1024.0;

type Box2 = ((f64, f64), (f64, f64));

fn meter(work: u64) -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(work))
}

fn frame() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap()
}

fn plane(z: f64, domain: Box2) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new([0.0, 0.0, z], [UNIT, 0.0, 0.0], [0.0, UNIT, 0.0], domain).unwrap(),
    )
}

fn cylinder(radius: f64, domain: Box2) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], radius, frame(), domain).unwrap())
}

fn sphere(domain: Box2) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere::new([0.0; 3], UNIT, frame(), domain).unwrap())
}

fn torus(domain: Box2) -> SurfaceKind {
    SurfaceKind::Torus(Torus::new([0.0; 3], 3.0 * UNIT, UNIT, frame(), domain).unwrap())
}

/// The region the shortcut returns for the pair, or `None` if `subdivide` did not take it. A meter
/// of 1 is too small for any search: the shortcut costs nothing, the search is refused.
fn shortcut(a: &SurfaceKind, b: &SurfaceKind) -> Option<Box2> {
    let zero = StepsInterval::new(Steps::ZERO, Steps::ZERO).unwrap();
    match subdivide(a, b, 8.0, &mut meter(1)) {
        Ok(Subdivided { settled, flagged }) => match flagged.as_slice() {
            [pair]
                if settled.is_empty()
                    && pair.why == (Unresolved::NearCoincident { gap: zero })
                    && pair.first == pair.second =>
            {
                Some(pair.first)
            }
            _ => None,
        },
        Err(IntersectError::BudgetExceeded) => None,
        Err(e) => panic!("{e:?}"),
    }
}

#[test]
fn identical_surfaces_cost_nothing() {
    for s in [
        plane(0.0, ((0.0, 4.0), (0.0, 4.0))),
        cylinder(UNIT, ((0.0, 1.0), (0.0, 2.0 * UNIT))),
        sphere(((0.0, 1.0), (-0.25, 0.25))),
        torus(((0.0, 2.0), (0.0, 2.0))),
    ] {
        assert_eq!(shortcut(&s, &s), Some(s.domain()));
    }
}

#[test]
fn patches_of_one_carrier_share_the_common_parameters() {
    let cases = [
        (
            plane(0.0, ((0.0, 3.0), (0.0, 3.0))),
            plane(0.0, ((1.0, 4.0), (1.0, 4.0))),
            ((1.0, 3.0), (1.0, 3.0)),
        ),
        (
            cylinder(UNIT, ((0.0, 1.0), (0.0, 2.0 * UNIT))),
            cylinder(UNIT, ((0.5, 1.5), (UNIT, 3.0 * UNIT))),
            ((0.5, 1.0), (UNIT, 2.0 * UNIT)),
        ),
        (
            sphere(((0.0, 1.0), (-0.25, 0.5))),
            sphere(((0.5, 1.5), (0.0, 0.5))),
            ((0.5, 1.0), (0.0, 0.5)),
        ),
        (
            torus(((0.0, 1.0), (0.0, 1.0))),
            torus(((0.5, 1.5), (0.5, 1.5))),
            ((0.5, 1.0), (0.5, 1.0)),
        ),
    ];
    for (a, b, region) in cases {
        assert_eq!(shortcut(&a, &b), Some(region));
        assert_eq!(shortcut(&b, &a), Some(region));
    }
}

#[test]
fn nothing_is_claimed_where_the_data_differ() {
    let patch = ((0.0, 3.0), (0.0, 3.0));
    // A plane a hair off, a cylinder of another radius, another kind.
    assert_eq!(shortcut(&plane(0.0, patch), &plane(1.0, patch)), None);
    assert_eq!(
        shortcut(
            &cylinder(UNIT, ((0.0, 1.0), (0.0, UNIT))),
            &cylinder(UNIT + 1.0, ((0.0, 1.0), (0.0, UNIT)))
        ),
        None
    );
    assert_eq!(
        shortcut(
            &cylinder(UNIT, ((0.0, 1.0), (0.0, UNIT))),
            &sphere(((0.0, 1.0), (-0.25, 0.25)))
        ),
        None
    );
}

#[test]
fn nothing_is_claimed_where_the_patches_only_touch() {
    // They share an edge, not an area.
    assert_eq!(
        shortcut(
            &plane(0.0, ((0.0, 1.0), (0.0, 1.0))),
            &plane(0.0, ((1.0, 2.0), (0.0, 1.0)))
        ),
        None
    );
    assert_eq!(
        shortcut(
            &cylinder(UNIT, ((0.0, 1.0), (0.0, UNIT))),
            &cylinder(UNIT, ((0.0, 1.0), (UNIT, 2.0 * UNIT)))
        ),
        None
    );
}

#[test]
fn nothing_is_claimed_where_a_patch_wraps_onto_the_other_twice() {
    // [0, 1.5] and [1, 2.5] share [1, 1.5] and, a turn round, [0, 0.5]: two regions.
    assert_eq!(
        shortcut(
            &cylinder(UNIT, ((0.0, 1.5), (0.0, UNIT))),
            &cylinder(UNIT, ((1.0, 2.5), (0.0, UNIT)))
        ),
        None
    );
    // The same longitudes under another name share only a turn round: not the unshifted copy.
    assert_eq!(
        shortcut(
            &sphere(((0.0, 1.0), (-0.25, 0.25))),
            &sphere(((2.0, 3.0), (-0.25, 0.25)))
        ),
        None
    );
}

#[test]
fn nothing_is_claimed_where_two_patches_reach_the_same_pole_apart() {
    // Both reach the north pole, one point, over longitudes that never meet: no shared area.
    assert_eq!(
        shortcut(
            &sphere(((0.0, 0.5), (0.0, 0.5))),
            &sphere(((1.0, 1.5), (0.0, 0.5)))
        ),
        None
    );
}
