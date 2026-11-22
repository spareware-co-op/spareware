//! What a surface's own data proves about its seams and poles, read exactly (chapter 12.10): a
//! torus's poles are the half-turns of its tube where it meets its axis, each found once; an affine
//! image closes only on a side it keeps as an angle and that is a whole turn long. A sphere's poles
//! on the axis of a cylinder that crosses it are proven apart from it as points, so nothing is left
//! undecided there. And a pole of the second surface of a pair, where four half-branches meet, ends
//! each of them there, as one of the first does.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::general::subdivide;
use geom_intersect::march::march;
use geom_intersect::result::CurveEnd;
use geom_intersect::{Charts, charts};
use geom_run::{Budget, WorkMeter};
use geom_surface::{AffineSurface, Cone, Cylinder, Fun, Plane, Sphere, SurfaceKind, Torus};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

fn up() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap()
}

/// The horn torus of radii 1 and 1 about the z axis on tube turns `tube`: its tube meets the axis
/// where cos πv = -1, an odd number of half-turns, so at v = -1 and v = 1 on [-1, 1], and at v = 1
/// alone on [0, 2].
fn horn(tube: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Torus(Torus::new([0.0; 3], 1.0, 1.0, up(), ((0.0, 2.0), tube)).unwrap())
}

/// Closes in both, poles at `v` in v, no creases.
fn closed_with_poles(v: Vec<f64>) -> Charts {
    Charts {
        closes: [true, true],
        poles: (Vec::new(), v),
        creases: (Vec::new(), Vec::new()),
    }
}

#[test]
fn a_horn_torus_has_a_pole_at_each_whole_turn_of_its_tube_on_the_axis_and_once() {
    // From v = -1: the half-turns -2/2 ... 2/2 are each visited, the first one included.
    assert_eq!(
        charts(&horn((-1.0, 1.0))),
        closed_with_poles(vec![-1.0, 1.0])
    );
    // From v = 0: the half-turns 0 ... 4/2, of which only 2/2 is on the axis, read once.
    assert_eq!(charts(&horn((0.0, 2.0))), closed_with_poles(vec![1.0]));
    // Up to v = 1/2: the outer half of the tube, no half-turn past its end is read, so the axis at
    // v = 1 is not a pole of it.
    let outer = charts(&horn((-0.5, 0.5)));
    assert_eq!(outer.closes, [true, false]);
    assert_eq!(outer.poles, (Vec::new(), Vec::new()));
}

/// An affine image of a tube on `domain`, `angular` saying which sides are angles.
fn affine(domain: ((f64, f64), (f64, f64)), angular: (bool, bool)) -> SurfaceKind {
    let terms = [
        (Fun::Cos, Fun::One, [3.0, 0.0, 0.0]),
        (Fun::Sin, Fun::One, [0.0, 2.0, 0.0]),
        (Fun::One, Fun::Cos, [0.0, 0.0, 1.0]),
    ];
    SurfaceKind::Affine(AffineSurface::from_terms([0.0; 3], &terms, domain, angular).unwrap())
}

#[test]
fn an_affine_image_closes_only_on_an_angular_side_a_whole_turn_long() {
    let half = (0.0, 1.0);
    let turn = (0.0, 2.0);
    for (domain, angular, closes) in [
        // Angles, but half a turn long: neither closes.
        ((half, half), (true, true), [false, false]),
        // A turn long, but not angles: neither closes.
        ((turn, turn), (false, false), [false, false]),
        // A turn long and angles: both close.
        ((turn, (-1.0, 1.0)), (true, true), [true, true]),
        // One of each.
        ((turn, turn), (true, false), [true, false]),
        ((half, turn), (false, true), [false, true]),
    ] {
        assert_eq!(
            charts(&affine(domain, angular)).closes,
            closes,
            "{domain:?} {angular:?}"
        );
    }
}

/// The sphere of radius 2 about the origin, poles at (0, 0, -2) and (0, 0, 2).
fn ball() -> SurfaceKind {
    SurfaceKind::Sphere(Sphere::new([0.0; 3], 2.0, up(), ((0.0, 2.0), (-0.5, 0.5))).unwrap())
}

/// The cylinder of radius 1 about the z axis from z = -3 to z = 3, its seam a quarter-turn from the
/// sphere's: the sphere's poles are on its axis, 1 from it, and it crosses the sphere in the
/// circles at z = -√3 and z = √3.
fn post() -> SurfaceKind {
    let turned = Frame::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    SurfaceKind::Cylinder(
        Cylinder::new([0.0, 0.0, -3.0], 1.0, turned, ((0.0, 2.0), (0.0, 6.0))).unwrap(),
    )
}

#[test]
fn a_spheres_poles_on_a_cylinders_axis_are_proven_apart_from_it() {
    let (ball, post) = (ball(), post());
    assert_eq!(charts(&ball).poles.1, vec![-0.5, 0.5]);
    for (one, two) in [(&ball, &post), (&post, &ball)] {
        let found = subdivide(one, two, 1.0 / 64.0, &mut meter()).unwrap();
        // Each pole is read as a point, 1 from the cylinder, so no pair at it is left undecided.
        assert!(found.flagged.is_empty(), "{:?}", found.flagged);
        assert!(!found.settled.is_empty(), "{found:?}");
    }
}

#[test]
fn a_double_cones_apex_second_in_the_pair_ends_four_half_branches_there() {
    // The plane x = 0 holds two whole lines of the double cone x² + y² = z², crossing at its apex
    // at v = 0, a pole of the cone's v side. With the cone second in the pair, each of the four
    // half-branches still ends there, singular, none joined through it (12.8, step 8).
    let cone = SurfaceKind::Cone(
        Cone::new(
            [0.0; 3],
            0.0,
            1.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-4.0, 4.0)),
        )
        .unwrap(),
    );
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        )
        .unwrap(),
    );
    assert_eq!(charts(&cone).poles.1, vec![0.0]);
    let found = subdivide(&plane, &cone, 1e-3, &mut meter()).unwrap();
    assert!(found.flagged.is_empty(), "{:?}", found.flagged);
    let traced = march(&plane, &cone, &found, 1e-3, 1e-2, &mut meter()).unwrap();
    assert_eq!(traced.branches.len(), 4, "{traced:?}");
    assert!(traced.unproven.is_empty(), "{:?}", traced.unproven);
    for branch in &traced.branches {
        let mut ends = branch.ends;
        ends.sort_by_key(|e| *e == CurveEnd::Singular);
        assert_eq!(ends, [CurveEnd::Boundary, CurveEnd::Singular], "{branch:?}");
        assert!(branch.passes.is_empty(), "{branch:?}");
    }
}
