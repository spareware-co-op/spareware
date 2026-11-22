//! A tangent point on the seam of a cylinder, a cone and a torus is one tangent point, whichever
//! surface is first (chapter 12.10; `architecture/decisions/G173-contacts-on-a-seam.md`). Each case
//! is a sphere that touches the surface from outside exactly at its seam (u = 0 = 2), in whole
//! numbers of grid steps, so the tangency is exact: the contact is flagged from both sides of the
//! seam by subdivision, and is decided in the chart that does not cut it. `tangent_on_a_seam.rs`
//! has the sphere against a sphere. Run at the scale of the SSI corpus (1024 steps to the unit,
//! floor 8, ceiling 10.24).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::contact::{Reading, analyse};
use geom_intersect::general::subdivide;
use geom_intersect::result::PointContact;
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Sphere, SurfaceKind, Torus};

const UNIT: f64 = 1024.0;
const FLOOR: f64 = 8.0;
const CEILING: f64 = 10.24;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

/// The frame of the axis z with u = 0 along x.
fn upright() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap()
}

/// The sphere of `radius` units about (`x`, 0, `z`) units, a whole sphere, its poles on the line
/// of `axis`.
fn ball_on(axis: Frame<3>, x: f64, z: f64, radius: f64) -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(
            [x * UNIT, 0.0, z * UNIT],
            radius * UNIT,
            axis,
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    )
}

/// A ball with its poles on the z axis.
fn ball(x: f64, z: f64, radius: f64) -> SurfaceKind {
    ball_on(upright(), x, z, radius)
}

/// A ball with its poles on the y axis, for a contact on the z axis, which would be a pole.
fn ball_about_y(x: f64, z: f64, radius: f64) -> SurfaceKind {
    ball_on(
        Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap(),
        x,
        z,
        radius,
    )
}

/// A cylinder of 3 units about the z axis, from z = -4 to 4. A ball of 2 units about (5, 0, 0)
/// touches it at (3, 0, 0), on its seam.
fn cylinder() -> SurfaceKind {
    SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            3.0 * UNIT,
            upright(),
            ((0.0, 2.0), (-4.0 * UNIT, 4.0 * UNIT)),
        )
        .unwrap(),
    )
}

/// A cone whose radius is 4 units at z = 0 and grows by 3/4 of a unit for each unit up. A ball of 5
/// units about (11, 0, 1) touches it at (7, 0, 4), on its seam: the centre is 3-4-5 from the point
/// along the normal, so the tangency is exact.
fn cone() -> SurfaceKind {
    SurfaceKind::Cone(
        Cone::new(
            [0.0; 3],
            4.0 * UNIT,
            0.75,
            upright(),
            ((0.0, 2.0), (0.0, 8.0 * UNIT)),
        )
        .unwrap(),
    )
}

/// A torus of 3 and 1 units about the z axis. A ball of 2 units about (3, 0, 3) touches its tube
/// from above at (3, 0, 1), on its seam.
fn torus() -> SurfaceKind {
    SurfaceKind::Torus(
        Torus::new(
            [0.0; 3],
            3.0 * UNIT,
            UNIT,
            upright(),
            ((0.0, 2.0), (0.0, 2.0)),
        )
        .unwrap(),
    )
}

fn readings(a: &SurfaceKind, b: &SurfaceKind) -> Vec<Reading> {
    let found = subdivide(a, b, FLOOR, &mut meter()).unwrap();
    analyse(a, b, &found, FLOOR, CEILING, &mut meter()).unwrap()
}

/// Whether `x` encloses a point within a hundredth of `at`.
fn near(x: Interval, at: f64) -> bool {
    x.lo() <= at + 0.01 && at - 0.01 <= x.hi() && x.hi() - x.lo() < 0.01
}

/// The one tangent point of the pair, and where its seam parameter is read, for both orders of the
/// pair: `seam` is the surface with the seam, and the point is at its u = 2 (its seam).
fn one_tangent_point(seam: &SurfaceKind, ball: &SurfaceKind) {
    for (found, seam_first) in [(readings(seam, ball), true), (readings(ball, seam), false)] {
        let [Reading::Point { at, gap, contact }] = found.as_slice() else {
            panic!("{found:#?}");
        };
        assert_eq!(*contact, PointContact::IsolatedTangent, "{at:?}");
        assert!(gap.lo() <= 0.0 && 0.0 <= gap.hi(), "{gap:?}");
        let on_seam = if seam_first { at[0] } else { at[2] };
        assert!(near(on_seam, 2.0), "{at:?}");
    }
}

#[test]
fn a_ball_touching_a_cylinder_on_its_seam_has_one_tangent_point() {
    one_tangent_point(&cylinder(), &ball(5.0, 0.0, 2.0));
}

#[test]
fn a_ball_touching_a_cone_on_its_seam_has_one_tangent_point() {
    one_tangent_point(&cone(), &ball(11.0, 1.0, 5.0));
}

#[test]
fn a_ball_touching_a_torus_on_its_seam_has_one_tangent_point() {
    one_tangent_point(&torus(), &ball_about_y(3.0, 3.0, 2.0));
}

/// A ball of 2 units touching the cylinder of 3 units a few steps past its seam, at the point
/// (c, s) · 3 units, with (c, s) = (m² − n², 2·m·n) / (m² + n²) for m = 1,000,000 and n = 1: a rational
/// direction, computed with divisions only, so the data is the same on every target and the ball's
/// centre is within the rounding of a double of exact tangency. The point is a hundredth of a
/// step along the circle from the seam, so close that its zero cannot be proven in a box that
/// stops at the seam, and the contact is read in the chart that does not cut there, with its whole
/// box a turn past the first chart's start: it is put back inside the domain, near u = 0, not
/// near 2.
#[test]
fn a_ball_touching_a_cylinder_a_hair_past_its_seam_is_read_inside_the_domain() {
    let (m, n) = (1_000_000.0_f64, 1.0_f64);
    let (c, s) = (
        (m * m - n * n) / (m * m + n * n),
        2.0 * m * n / (m * m + n * n),
    );
    let ball = SurfaceKind::Sphere(
        Sphere::new(
            [5.0 * UNIT * c, 5.0 * UNIT * s, 0.0],
            2.0 * UNIT,
            upright(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    );
    let cylinder = cylinder();
    for (found, cylinder_first) in [
        (readings(&cylinder, &ball), true),
        (readings(&ball, &cylinder), false),
    ] {
        let [Reading::Point { at, contact, .. }] = found.as_slice() else {
            panic!("{found:#?}");
        };
        assert_eq!(*contact, PointContact::IsolatedTangent, "{at:?}");
        let u = if cylinder_first { at[0] } else { at[2] };
        // Just past the seam, inside the domain: not a turn on, at 2.
        assert!(u.lo() >= 0.0 && u.hi() <= 0.001, "{at:?}");
    }
}
