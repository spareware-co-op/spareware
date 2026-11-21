//! A pole line read against the other surface (chapter 12.10), held to what each case is known to
//! give: a pole proven on the other surface's axis is not proven apart from it when it may touch,
//! and a pole whose own enclosure is wider than the floor is not read as within the floor of a
//! surface that lies over two floors from it.
//!
//! The ball x² + y² + (z − 1)² = 1 rests on the apex of the cone x² + y² = z², its south pole on
//! the apex and on the cone's axis: they meet in the circle z = 1 of radius 1 and in the apex
//! alone. And a ball of radius 1024 whose south pole sits a few doubles from the origin, read
//! against a plane at 45 degrees to its axis that passes 4.5 · 2⁻⁴³ from that pole.
//!
//! How two enclosures meet is held too, in marches over pairs built by hand: an enclosure that
//! reaches a pole's value at one end is not held on the pole, and one held at a closing
//! parameter's end meets another across the seam only if that one is held at the other end.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_curve::Frame;
use geom_intersect::general::{Settled, Subdivided, subdivide};
use geom_intersect::march::march;
use geom_intersect::result::{CurveEnd, Test, Unresolved};
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind};

#[test]
fn a_ball_resting_on_a_cones_apex_keeps_the_apex() {
    let ball = SurfaceKind::Sphere(
        Sphere::new(
            [0.0, 0.0, 1.0],
            1.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    );
    let cone = SurfaceKind::Cone(
        Cone::new(
            [0.0; 3],
            0.0,
            1.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 2.0)),
        )
        .unwrap(),
    );
    for ball_first in [true, false] {
        let (one, two) = if ball_first {
            (&ball, &cone)
        } else {
            (&cone, &ball)
        };
        let found = subdivide(
            one,
            two,
            1.0 / 64.0,
            &mut WorkMeter::new(Budget::of_work(1 << 24)),
        )
        .unwrap();
        // The circle is settled; the apex, where no branch leaves, is a point the search keeps
        // in a flagged pair holding both the ball's south pole and the cone's apex.
        assert!(!found.settled.is_empty(), "{found:?}");
        let holds = found.flagged.iter().any(|f| {
            let (b, c) = if ball_first {
                (f.first, f.second)
            } else {
                (f.second, f.first)
            };
            b.1.0 == -0.5 && c.1.0 == 0.0
        });
        assert!(
            holds,
            "ball first {ball_first}: the apex is lost: {:?}",
            found.flagged
        );
    }
}

#[test]
fn a_pole_wider_than_the_floor_is_not_read_onto_a_plane_two_floors_off() {
    // The ball of radius r = 1024 about c = (0, rs, rs), s the double nearest 1/√2, about the axis
    // (0, 1, 1)/√2: its south pole c − r(0, 1, 1)/√2 is (0, z, z) with z = r(s − 1/√2), a few
    // doubles above 0, and its enclosure is 2⁻⁴⁰ wide, four floors of 2⁻⁴².
    let r = 1024.0;
    let s = core::f64::consts::FRAC_1_SQRT_2;
    let floor = 1.0 / 4_398_046_511_104.0;
    let unit = floor / 2.0;
    // z = r(s² − 1/2)/(s + 1/√2), and s + 1/√2 > 1.4: z is under half a 2⁻⁴³.
    let lift = s.mul_add(s, -0.5);
    assert!(lift > 0.0 && r * lift / 1.4 < 0.5 * unit, "{lift:e}");
    // A cap of 64 floors about the pole, and the plane z = 5 · 2⁻⁴³, at 45 degrees to the axis: it
    // passes over 4.5 · 2⁻⁴³, more than two floors, from the pole, which is on no curve of the two.
    let eps = 64.0 * floor / (r * core::f64::consts::PI);
    let ball = SurfaceKind::Sphere(
        Sphere::new(
            [0.0, r * s, r * s],
            r,
            Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 1.0]).unwrap(),
            ((0.0, 2.0), (-0.5, -0.5 + eps)),
        )
        .unwrap(),
    );
    let w = 256.0 * floor;
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 5.0 * unit],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-w, w), (-w, w)),
        )
        .unwrap(),
    );
    let found = subdivide(
        &ball,
        &plane,
        floor,
        &mut WorkMeter::new(Budget::of_work(1 << 23)),
    )
    .unwrap();
    // The pole's enclosure reaches past the floor, so the pole is undecided, not on the plane:
    // no start at it, and the pair holding it is flagged as an undecided exclusion.
    for pair in &found.settled {
        for x in &pair.starts {
            assert!(
                !(x[1].lo() == -0.5 && x[1].hi() == -0.5),
                "a start at the pole: {pair:?}"
            );
        }
    }
    assert!(!found.flagged.is_empty(), "{found:?}");
    for f in &found.flagged {
        assert_eq!(
            f.why,
            Unresolved::UndecidedAtFloor {
                test: Test::Exclusion
            },
            "{f:?}"
        );
    }
    assert!(
        found.flagged.iter().any(|f| f.first.1.0 == -0.5),
        "{found:?}"
    );
}

fn around(x: f64, half: f64) -> Interval {
    Interval::new(x - half, x + half).unwrap()
}

fn pt(x: f64) -> Interval {
    Interval::point(x).unwrap()
}

/// One settled pair, nothing flagged.
fn one_pair(
    first: ((f64, f64), (f64, f64)),
    second: ((f64, f64), (f64, f64)),
    starts: Vec<[Interval; 4]>,
) -> Subdivided {
    Subdivided {
        settled: vec![Settled {
            first,
            second,
            starts,
        }],
        flagged: Vec::new(),
    }
}

#[test]
fn an_end_whose_enclosure_reaches_the_apex_is_not_singular() {
    // The plane x = 0 holds the line u = 1/2 of the double cone x² + y² = z², (0, v, v). A pair over
    // v ≥ t holds it from (0, t, t), t = 1/16, over six floors from the apex, to (0, 4, 4). The low
    // start's enclosure of v is [0, 2t]: it reaches the apex's value 0 but is not held there, and
    // the branch's end at it is the pair's edge, not the apex.
    let t = 1.0 / 16.0;
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
    let wall = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        )
        .unwrap(),
    );
    let low = [
        pt(0.5),
        Interval::new(0.0, 2.0 * t).unwrap(),
        around(t, 1e-12),
        around(t, 1e-12),
    ];
    let top = [pt(0.5), pt(4.0), pt(4.0), pt(4.0)];
    for (starts, ends) in [
        (vec![low, top], [CurveEnd::Flagged, CurveEnd::Boundary]),
        (vec![top, low], [CurveEnd::Boundary, CurveEnd::Flagged]),
    ] {
        let found = one_pair(((0.25, 0.75), (t, 4.0)), ((-8.0, 8.0), (-8.0, 8.0)), starts);
        let traced = march(
            &cone,
            &wall,
            &found,
            1.0 / 64.0,
            1.0 / 64.0,
            &mut WorkMeter::new(Budget::of_work(1 << 22)),
        )
        .unwrap();
        assert!(traced.unproven.is_empty(), "{traced:?}");
        assert_eq!(traced.branches.len(), 1, "{traced:?}");
        assert_eq!(traced.branches[0].ends, ends, "{traced:?}");
    }
}

#[test]
fn an_arc_from_a_cylinders_seam_is_open_at_a_start_not_held_at_the_seams_other_end() {
    // z = 0 meets the unit cylinder about z in its circle v = 0; a pair over u in [0, 2/5] holds the
    // arc from (1, 0, 0), at u = 0 on the seam, to (cos 2π/5, sin 2π/5, 0). The starts' enclosures
    // in the plane are wide enough to overlap, so only u tells them apart: u = 2/5 is not the seam's
    // other end, and the arc is open, whichever start it is marched from.
    let cyl = SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            1.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-1.0, 1.0)),
        )
        .unwrap(),
    );
    let ground = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-2.0, 2.0), (-2.0, 2.0)),
        )
        .unwrap(),
    );
    let w = 0.75;
    let a = [pt(0.0), pt(0.0), around(1.0, w), around(0.0, w)];
    let turn = core::f64::consts::PI * 0.4;
    let b = [
        pt(0.4),
        pt(0.0),
        around(turn.cos(), w),
        around(turn.sin(), w),
    ];
    for (starts, ends) in [
        (vec![a, b], [CurveEnd::Boundary, CurveEnd::Flagged]),
        (vec![b, a], [CurveEnd::Flagged, CurveEnd::Boundary]),
    ] {
        let found = one_pair(
            ((0.0, 0.4), (-1.0, 1.0)),
            ((-2.0, 2.0), (-2.0, 2.0)),
            starts,
        );
        let traced = march(
            &cyl,
            &ground,
            &found,
            1e-3,
            1e-3,
            &mut WorkMeter::new(Budget::of_work(1 << 22)),
        )
        .unwrap();
        assert!(traced.unproven.is_empty(), "{traced:?}");
        assert_eq!(traced.branches.len(), 1, "{traced:?}");
        let branch = &traced.branches[0];
        assert_eq!(branch.ends, ends, "{branch:?}");
        assert!(branch.vertices.len() > 2, "{branch:?}");
    }
}
