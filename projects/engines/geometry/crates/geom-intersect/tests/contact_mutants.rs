//! Contacts held to their exact answers and their exact work (12.9, 12.10): a cone's apex resting
//! on a plane, a sphere touching a plane at its pole and near its other one, a sphere whose normal
//! points in, a cylinder resting on a plane, and surfaces too close to part at the floor (bowls and
//! ridges) whose clusters reach the edges of their domains in different ways, each in both orders
//! of the pair. Work ends runs, so the work an analysis charges is part of its answer, and each is
//! held to the unit, with each reading to its recorded bits.

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
use geom_nurbs::{Basis, Surface};
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
/// turned chart (`pole_contact.rs`); and the same sphere flipped along its axis, whose normal points
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

/// z = h + (x² + y²)/1024 over the square [−r, r]² of a frame put at `at` and turned an eighth of a
/// turn if `turned`, in Bézier form.
fn bowl(r: f64, h: f64, at: (f64, f64), turned: bool) -> SurfaceKind {
    let basis = || Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let half = core::f64::consts::FRAC_1_SQRT_2;
    let (cos, sin) = if turned { (half, half) } else { (1.0, 0.0) };
    // The bowl's lowest point in the frame's own coordinates.
    let (cp, cq) = (-(cos * at.0 + sin * at.1), -(-sin * at.0 + cos * at.1));
    // The Bernstein coefficients of (t − c)²/1024 over [−r, r], and the points they sit over.
    let along = |c: f64| {
        let w = |a: f64, b: f64| (a - c) * (b - c) / 1024.0;
        [(-r, w(-r, -r)), (0.0, w(-r, r)), (r, w(r, r))]
    };
    let mut net = Vec::new();
    for (p, zp) in along(cp) {
        for (q, zq) in along(cq) {
            let (x, y) = (at.0 + cos * p - sin * q, at.1 + sin * p + cos * q);
            net.push([x, y, h + zp + zq]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap())
}

/// A small bowl one unit above a wide one of the same shape: too close to part at the floor, so
/// they overlap. The small one's cluster is all of it and so reaches every edge of its domain; the
/// wide one's reaches none, and one surface at an edge is enough.
#[test]
fn a_bowl_over_a_wider_one_overlaps_though_only_the_small_one_reaches_an_edge() {
    let side = "(0.0, 1.0), (0.0, 1.0)";
    holds_to(
        (
            &bowl(256.0, 1.0, (0.0, 0.0), false),
            &bowl(1024.0, 0.0, (0.0, 0.0), false),
        ),
        (256.0, 1024.0),
        [
            (
                5492,
                &format!(
                    "[Overlap {{ first: ({side}), second: ((0.3125, 0.675048828125), \
                     (0.3125, 0.675048828125)), coincidence: Within(Steps(432)) }}]"
                ),
            ),
            (
                4049,
                &format!(
                    "[Overlap {{ first: ((0.3596343994140625, 0.68359375), \
                     (0.3596343994140625, 0.68359375)), second: ({side}), \
                     coincidence: Within(Steps(463)) }}]"
                ),
            ),
        ],
    );
}

/// The same shape turned an eighth of a turn and hung one unit above a bowl's side: the pairs too
/// close to part lie across a diagonal of each domain, so their patches leave holes in their hulls
/// and the cluster is not an overlap. Each patch is counted once, however many pairs hold it.
#[test]
fn a_turned_bowl_over_the_side_of_another_leaves_holes_so_it_is_no_overlap() {
    holds_to(
        (
            &bowl(512.0, 0.0, (0.0, 0.0), false),
            &bowl(384.0, 1.0, (-512.0, 0.0), true),
        ),
        (128.0, 1024.0),
        [
            (
                1_351_309,
                "[Undecided { first: ((0.0, 0.545166015625), (0.0, 1.0)), \
                 second: ((0.0, 1.0), (0.0, 1.0)), why: HigherOrderContact }]",
            ),
            (
                1_460_130,
                "[Undecided { first: ((0.0, 1.0), (0.0, 1.0)), \
                 second: ((0.0, 0.5735015869140625), (0.0, 1.0)), why: HigherOrderContact }]",
            ),
        ],
    );
}

/// z = h + 192·(s − s³/3) for s = x/512, over x in [−512, 512] and y in `ys`: a ridge steepest
/// along x = 0 and level at x = ±512, u running along y (linear) and v along x (cubic), in Bézier
/// form; or, `swapped`, u along x and v along y.
fn ridge(ys: (f64, f64), h: f64, swapped: bool) -> SurfaceKind {
    let line = || Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let cubic = || Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    // The Bernstein coefficients of s − s³/3 over [−1, 1] are −2/3, −2/3, 2/3, 2/3.
    let column = [
        (-512.0, -128.0),
        (-512.0 / 3.0, -128.0),
        (512.0 / 3.0, 128.0),
        (512.0, 128.0),
    ];
    let mut net = Vec::new();
    if swapped {
        for (x, z) in column {
            for y in [ys.0, ys.1] {
                net.push([x, y, h + z]);
            }
        }
        return SurfaceKind::Nurbs(Surface::new(cubic(), line(), &net).unwrap());
    }
    for y in [ys.0, ys.1] {
        for (x, z) in column {
            net.push([x, y, h + z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(line(), cubic(), &net).unwrap())
}

/// Two copies of the ridge 24 units apart: their boxes meet only where the ridge is steep, so the
/// clusters lie in from the ridge's level ends, and each reaches one side of one domain alone. That
/// is an edge, so each overlaps, whichever side it is: the start of the upper ridge's u and the end
/// of the lower one's; the end of both; or, with u and v swapped, the start of the upper one's v
/// and the end of the lower one's.
#[test]
fn ridges_too_close_to_part_overlap_where_each_reaches_one_side_alone() {
    // Each cluster's part along y on the first surface and on the second, then along x; along y
    // is u, or v if `swapped`.
    let reads = |(y1, y2): (&str, &str), xs: [(&str, &str); 3], within: u64, swapped: bool| {
        let each = xs.map(|(x1, x2)| {
            let (first, second) = if swapped {
                (format!("({x1}, {y1})"), format!("({x2}, {y2})"))
            } else {
                (format!("({y1}, {x1})"), format!("({y2}, {x2})"))
            };
            format!(
                "Overlap {{ first: {first}, second: {second}, \
                 coincidence: Within(Steps({within})) }}"
            )
        });
        format!("[{}]", each.join(", "))
    };
    // Along x, with the upper ridge first and with the lower one first.
    let upper_xs = [
        (
            "(0.19140625, 0.299072265625)",
            "(0.2109527587890625, 0.3125)",
        ),
        (
            "(0.3596343994140625, 0.545166015625)",
            "(0.4257965087890625, 0.52734375)",
        ),
        (
            "(0.6057281494140625, 0.68359375)",
            "(0.5735015869140625, 0.7765960693359375)",
        ),
    ];
    let lower_xs = [
        (
            "(0.19140625, 0.299072265625)",
            "(0.2109527587890625, 0.3125)",
        ),
        (
            "(0.4375, 0.545166015625)",
            "(0.4257965087890625, 0.52734375)",
        ),
        (
            "(0.6057281494140625, 0.68359375)",
            "(0.5735015869140625, 0.675048828125)",
        ),
    ];
    let (rising, falling, lower) = ((0.0, 512.0), (512.0, 0.0), (-256.0, 256.0));
    let start_end = ("(0.0, 0.68359375)", "(0.3125, 1.0)");
    let end_start = ("(0.4375, 1.0)", "(0.0, 0.52734375)");
    for swapped in [false, true] {
        holds_to(
            (&ridge(rising, 24.0, swapped), &ridge(lower, 0.0, swapped)),
            (128.0, 1024.0),
            [
                (3084, &reads(start_end, upper_xs, 241, swapped)),
                (1557, &reads(end_start, lower_xs, 237, swapped)),
            ],
        );
    }
    let ends = ("(0.4375, 1.0)", "(0.3125, 1.0)");
    holds_to(
        (&ridge(falling, 24.0, false), &ridge(lower, 0.0, false)),
        (128.0, 1024.0),
        [
            (3090, &reads(ends, upper_xs, 241, false)),
            (1866, &reads(ends, lower_xs, 241, false)),
        ],
    );
}

/// The hanging sphere at a floor of 4: its cluster at the pole reaches past an eighth of a turn
/// from it, so it is never read in the turned chart, and stays undecided at the work recorded.
#[test]
fn a_cluster_reaching_past_the_cap_is_not_read_in_the_turned_chart() {
    holds_to(
        (&ball([0.0625, 0.03125, -5.0]), &level()),
        (4.0, 1.0 / 16.0),
        [
            (
                44_435,
                "[Undecided { first: ((0.0, 2.0), (0.18359375, 0.5)), \
                 second: ((-7.2421875, 3.15087890625), (-3.375, 4.978729248046875)), \
                 why: HigherOrderContact }]",
            ),
            (
                34_426,
                "[Undecided { first: ((-3.61669921875, 3.3046875), (-3.61669921875, 3.3046875)), \
                 second: ((0.0, 2.0), (0.2765960693359375, 0.5)), why: HigherOrderContact }]",
            ),
        ],
    );
}

/// A cylinder of radius 5 units resting on a plane touches it along a line, in a band of
/// √(2δ/Δκ) (`attack_g171.rs`), held to its recorded width and gap.
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

/// A cylinder of radius 4 about the axis a = (0, 1, 1)/√2, and a plane through its line at
/// u = 1/2 sunk 1/16 into it, spanned by x and a tilted 1/1024 toward that line's normal e₂: the
/// plane's normal is never at right angles to a, so no normal of the cylinder is parallel to it
/// and there is no contact. The axis is tilted, so the box of a patch's normals is wider than the
/// normals: at a floor of 1 subdivision cannot part them near the line, and solve parts them below
/// the floor. With the cylinder first, the look then settles every pair of that cluster, none of
/// which holds a branch's start, so nothing is left to read.
#[test]
fn a_shallow_plane_whose_normals_solve_parts_below_the_floor_is_settled() {
    let s = core::f64::consts::FRAC_1_SQRT_2;
    let r = 4.0;
    let h = 1.0 / 16.0;
    let tilt = 1.0 / 1024.0;
    let tube = SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            r,
            Frame::new([1.0, 0.0, 0.0], [0.0, s, s]).unwrap(),
            ((0.1, 0.9), (-2.0, 2.0)),
        )
        .unwrap(),
    );
    let e2 = [0.0, s, -s];
    let shallow = SurfaceKind::Plane(
        Plane::new(
            e2.map(|t| (r - h) * t),
            [1.0, 0.0, 0.0],
            [0.0, s + tilt * e2[1], s + tilt * e2[2]],
            ((-3.0, 3.0), (-3.0, 3.0)),
        )
        .unwrap(),
    );
    holds_to(
        (&tube, &shallow),
        (1.0, 1.0 / 16.0),
        [
            (144_813, "[]"),
            (
                704_500,
                "[Undecided { first: ((-3.0, 1.1015625), (-3.0, 3.0)), \
                 second: ((0.46602401733398435, 0.9), (-2.0, 2.0)), \
                 why: TangentBand { width: Steps(8) } }]",
            ),
        ],
    );
}

/// The saddle z = (x² − y²)/8 over [−4, 4]², exact in Bézier form.
fn saddle() -> SurfaceKind {
    let basis = || Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    // The Bernstein coefficients of t² over [−4, 4], and the points they sit over.
    let along = [(-4.0, 16.0), (0.0, -16.0), (4.0, 16.0)];
    let mut net = Vec::new();
    for (x, zx) in along {
        for (y, zy) in along {
            net.push([x, y, (zx - zy) / 8.0]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap())
}

/// A plane tangent to a saddle at its centre: solve proves the one zero, a crossing of four
/// half-branches, but the branches counted ending on the cluster are not four (the cluster at a
/// floor of 1/2 runs out to the saddle's edges), so the point is left undecided as a higher-order
/// contact, never read as the crossing alone.
#[test]
fn a_saddle_on_a_plane_whose_branches_are_not_its_zeros_is_undecided() {
    holds_to(
        (&level(), &saddle()),
        (1.0 / 2.0, 1.0 / 16.0),
        [
            (
                1381,
                "[Undecided { first: ((-9.0, 9.0), (-9.0, 9.0)), \
                 second: ((0.0, 1.0), (0.0, 1.0)), why: HigherOrderContact }]",
            ),
            (
                2756,
                "[Undecided { first: ((0.4375, 1.0), (0.0, 0.4375)), \
                 second: ((3.15087890625, 9.0), (-9.0, -3.375)), \
                 why: UndecidedAtFloor { test: Exclusion } }, \
                 Undecided { first: ((0.0, 0.4375), (0.0, 1.0)), \
                 second: ((-9.0, -3.375), (3.15087890625, 9.0)), \
                 why: UndecidedAtFloor { test: Exclusion } }, \
                 Undecided { first: ((0.0, 1.0), (0.0, 1.0)), \
                 second: ((-9.0, 4.978729248046875), (-9.0, 4.978729248046875)), \
                 why: HigherOrderContact }]",
            ),
        ],
    );
}
