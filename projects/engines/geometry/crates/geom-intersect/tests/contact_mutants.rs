//! Contacts held to their exact answers and their exact work (12.9, 12.10): a cone's apex resting
//! on a plane, a sphere touching a plane at its pole and near its other one, a sphere whose normal
//! points in, and a cylinder resting on a plane, each in both orders of the pair. Work ends runs,
//! so the work an analysis charges is part of its answer, and each is held to the unit, with each
//! reading to its recorded bits.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::contact::analyse;
use geom_intersect::general::subdivide;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 25))
}

/// Analyses the pair in both orders and holds each to its recorded work and readings.
fn holds_to(
    (a, b): (&SurfaceKind, &SurfaceKind),
    (floor, ceiling): (f64, f64),
    recorded: [(u128, &str); 2],
) {
    for ((one, two), (work, readings)) in [(a, b), (b, a)].into_iter().zip(recorded) {
        let found = subdivide(one, two, floor, &mut meter()).unwrap();
        let mut m = meter();
        let read = analyse(one, two, &found, floor, ceiling, &mut m).unwrap();
        assert_eq!(format!("{read:?}"), readings);
        assert_eq!(m.used().work, work, "{read:?}");
    }
}

fn plane(origin: [f64; 3], u: [f64; 3], v: [f64; 3], r: f64) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(origin, u, v, ((-r, r), (-r, r))).unwrap())
}

/// The plane z = 0 a little off the origin, so that no cut falls on a contact.
fn level() -> SurfaceKind {
    plane(
        [0.0625, 0.03125, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        9.0,
    )
}

/// The cone x² + y² = z² about the z axis over heights `v`, its apex at the origin.
fn cone(v: (f64, f64)) -> SurfaceKind {
    let frame = Frame::standard().unwrap();
    SurfaceKind::Cone(Cone::new([0.0; 3], 0.0, 1.0, frame, ((0.0, 2.0), v)).unwrap())
}

/// The whole sphere of radius 5 about `centre`.
fn ball(centre: [f64; 3]) -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(
            centre,
            5.0,
            Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    )
}

/// The apex is a pole of the cone's height, at the start of its domain or at its end, and the
/// plane meets the cone there alone: a lone pole, one singular point that no branch leaves, over
/// the whole line v = 0 of the cone.
#[test]
fn a_cones_apex_on_a_plane_at_either_end_of_its_height_is_one_singular_point() {
    let on_cone = "Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 0.0 }";
    let on_plane = "Interval { lo: -0.06591796875, hi: -0.0615234375 }, \
                    Interval { lo: -0.03515625, hi: -0.03076171875 }";
    let rest = "gap: Interval { lo: 0.0, hi: 0.005190500885124343 }, \
                contact: Singular { branches: 0 } }]";
    let first = format!("[Point {{ at: [{on_cone}, {on_plane}], {rest}");
    let second = format!("[Point {{ at: [{on_plane}, {on_cone}], {rest}");
    for v in [(0.0, 4.0), (-4.0, 0.0)] {
        holds_to(
            (&cone(v), &level()),
            (1.0 / 16.0, 1.0 / 16.0),
            [(5645, &first), (5647, &second)],
        );
    }
}

/// A plane tangent to the sphere about the origin at (27/205, 36/205, −40/41)·5, latitude about
/// −0.4296 half-turns: near the south pole but not at it, so it is read in the sphere's own chart,
/// one tangent point where it touches, never put at the pole.
#[test]
fn a_plane_touching_a_sphere_near_its_south_pole_is_read_where_it_touches() {
    let at = [5.0 * 27.0 / 205.0, 5.0 * 36.0 / 205.0, -5.0 * 40.0 / 41.0];
    let along = [-0.8, 0.6, 0.0];
    let up = [24.0 / 41.0, 32.0 / 41.0, 9.0 / 41.0];
    holds_to(
        (&ball([0.0; 3]), &plane(at, along, up, 9.0)),
        (1.0 / 64.0, 1.0 / 16.0),
        [
            (
                18_704,
                "[Point { at: [Interval { lo: 0.29516723530086547, hi: 0.2951672353008677 }, \
                 Interval { lo: -0.42955342504544614, hi: -0.42955342504544475 }, \
                 Interval { lo: -4.01468935120884e-15, hi: 4.006924326692184e-15 }, \
                 Interval { lo: -6.392583305350423e-15, hi: 6.298483132808708e-15 }], \
                 gap: Interval { lo: -1.5229552036822156e-14, hi: 1.536115408315578e-14 }, \
                 contact: IsolatedTangent }]",
            ),
            (
                18_447,
                "[Point { at: [Interval { lo: -4.777255301842622e-15, hi: 4.891556418810474e-15 }, \
                 Interval { lo: -9.923431093258219e-15, hi: 9.783597774103383e-15 }, \
                 Interval { lo: 0.29516723530086525, hi: 0.2951672353008679 }, \
                 Interval { lo: -0.42955342504544636, hi: -0.4295534250454445 }], \
                 gap: Interval { lo: -1.8026230914462632e-14, hi: 1.7889754718264806e-14 }, \
                 contact: IsolatedTangent }]",
            ),
        ],
    );
}

/// A sphere hanging from the plane z = 0 at its north pole is one tangent point there, read in the
/// turned chart (pole_contact.rs); and the same sphere flipped along its axis, whose normal points
/// in, is left undecided at its top pole, the turned chart never read.
#[test]
fn a_sphere_at_a_plane_by_its_pole_charges_its_recorded_work() {
    let hanging = ball([0.0625, 0.03125, -5.0]);
    holds_to(
        (&hanging, &level()),
        (1.0 / 16.0, 1.0 / 16.0),
        [
            (
                581_829,
                "[Point { at: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.5, hi: 0.5 }, \
                 Interval { lo: -1.7347234759768148e-17, hi: 1.0408340855860915e-17 }, \
                 Interval { lo: -2.447639532942543e-18, hi: 1.889169156999523e-18 }], \
                 gap: Interval { lo: -5.329070518200785e-15, hi: 3.5527136788005246e-15 }, \
                 contact: IsolatedTangent }]",
            ),
            (
                580_030,
                "[Point { at: [Interval { lo: -1.7347234759768126e-17, hi: 1.0408340855860895e-17 }, \
                 Interval { lo: -1.7906731663020902e-18, hi: 5.148220737605164e-18 }, \
                 Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.5, hi: 0.5 }], \
                 gap: Interval { lo: -3.552713678800509e-15, hi: 5.329070518200762e-15 }, \
                 contact: IsolatedTangent }]",
            ),
        ],
    );
    let SurfaceKind::Sphere(s) = &hanging else {
        panic!("{hanging:?}");
    };
    let flipped = SurfaceKind::Sphere(s.flipped_axially().unwrap());
    holds_to(
        (&flipped, &level()),
        (1.0 / 16.0, 1.0 / 16.0),
        [
            (
                601_096,
                "[Undecided { first: ((0.0, 2.0), (-0.5, -0.4969320632517338)), \
                 second: ((-0.10177461057901382, 0.083838548976928), \
                 (-0.10177461057901382, 0.083838548976928)), why: HigherOrderContact }]",
            ),
            (
                589_391,
                "[Undecided { first: ((-0.0684746503829956, 0.04889405518770218), \
                 (-0.0684746503829956, 0.04889405518770218)), \
                 second: ((0.0, 2.0), (-0.5, -0.49701976776123047)), why: HigherOrderContact }]",
            ),
        ],
    );
}

/// A cylinder of radius 5 units resting on a plane touches it along a line, in a band of
/// √(2δ/Δκ) (attack_g171.rs), held to its recorded width and gap.
#[test]
fn a_cylinder_on_a_plane_is_a_band_of_its_recorded_width() {
    let unit = 1024.0;
    let resting = SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0, 0.0, 5.0 * unit],
            5.0 * unit,
            Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap(),
            ((0.1, 0.9), (-unit, unit)),
        )
        .unwrap(),
    );
    let flat = plane(
        [64.0, 32.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        12.0 * unit,
    );
    holds_to(
        (&resting, &flat),
        (8.0, unit),
        [
            (
                1_138_913,
                "[Curve { first: ((0.4998004790752021, 0.5001163945973987), (-1024.0, 1024.0)), \
                 second: ((-68.1237733012793, -59.76132023334503), \
                 (-1056.3903977652508, 992.7156678206502)), \
                 contact: Tangent { band: Steps(396), \
                 gap: StepsInterval { lo: Steps(0), hi: Steps(16) } } }]",
            ),
            (
                1_620_643,
                "[Curve { first: ((-68.27738782399683, -55.27009272342548), \
                 (-1058.0596579536796, 996.5337758198439)), \
                 second: ((0.4998269924013584, 0.500207770265709), (-1024.0, 1024.0)), \
                 contact: Tangent { band: Steps(392), \
                 gap: StepsInterval { lo: Steps(0), hi: Steps(15) } } }]",
            ),
        ],
    );
}
