//! A pole line read against the other surface (chapter 12.10), held to what each case is known to
//! give: a pole proven on the other surface's axis is not proven apart from it when it may touch,
//! and a pole whose own enclosure is wider than the floor is not read as within the floor of a
//! surface that lies over two floors from it.
//!
//! The ball x² + y² + (z − 1)² = 1 rests on the apex of the cone x² + y² = z², its south pole on
//! the apex and on the cone's axis: they meet in the circle z = 1 of radius 1 and in the apex
//! alone. And a ball of radius 1024 whose south pole sits a few doubles from the origin, read
//! against a plane at 45 degrees to its axis that passes 4.5 · 2⁻⁴³ from that pole.

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
use geom_intersect::general::subdivide;
use geom_intersect::result::{Test, Unresolved};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Plane, Sphere, SurfaceKind};

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
