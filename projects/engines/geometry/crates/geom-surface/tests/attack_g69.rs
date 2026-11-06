//! What piece G69's law catalogue found in `geom-surface`, kept as permanent tests (chapter
//! 28.16).
//!
//! - **A revolution turns about its axis point.** A surface of revolution about an axis through a
//!   point a off the origin was placed at the origin: its profile moved by −a, in its points, its
//!   bounds and its NURBS form alike. Now it is placed at a: at angle 0 it is the profile, and
//!   each point of the profile turns on a circle about the axis.
//! - **Grid transforms are exact.** A cylinder, cone, sphere or torus on a frame whose unit
//!   vectors are irrational stated a rounding of about 10⁻¹⁵ under a quarter turn, a mirror and a
//!   whole-step translation, though each moves it exactly (chapter 7). Now a grid transform that
//!   moves the centre exactly states deviation 0, and one that rounds the centre, or a transform
//!   that is not of the grid, still states its rounding.
//! - **The contract kit steps past huge ends.** Testing that a kind refuses outside its domain, the
//!   kit stepped one unit before the start, which beyond 2^53 in magnitude rounds back to the
//!   start itself, inside: `geom-fuzz`'s `surfaces` target found it reporting kinds that rightly
//!   accepted it. Now each step is at least one double.
#![allow(
    clippy::unwrap_used,
    clippy::float_cmp,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "tests: a failure stops the test, exactness is equality, and the names are the formulas'"
)]

use geom_curve::trimmed::Trimmed;
use geom_curve::{Basis, Frame, Line};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, TypedTransform};
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, ParamSurface, Revolution, Sphere, SurfaceKind, Torus};

const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

#[test]
fn a_revolution_turns_about_its_axis_point() {
    // The segment from (5, 0, 0) to (5, 0, 1) about the vertical axis through (1, 2, 3).
    let line = Line::new([5.0, 0.0, 0.0], [0.0, 0.0, 1.0], (0.0, 1.0)).unwrap();
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let profile = Trimmed::new(Basis::Line(line), (0.0, 1.0)).unwrap();
    let r = Revolution::new(profile, [1.0, 2.0, 3.0], f, (0.0, 1.5)).unwrap();
    // At angle 0, the profile.
    assert_eq!(r.point(0.0, 0.0, SIDES).unwrap(), [5.0, 0.0, 0.0]);
    assert_eq!(r.point(0.0, 0.5, SIDES).unwrap(), [5.0, 0.0, 0.5]);
    // A half turn about the axis: (1, 2) − ((5, 0) − (1, 2)) = (−3, 4), the height kept.
    assert_eq!(r.point(1.0, 0.0, SIDES).unwrap(), [-3.0, 4.0, 0.0]);
    // A quarter turn: (1, 2) + the quarter turn of (4, −2), (2, 4), is (3, 6).
    let q = r.point(0.5, 0.0, SIDES).unwrap();
    assert!(
        (q[0] - 3.0).abs() < 1e-14 && (q[1] - 6.0).abs() < 1e-14 && q[2] == 0.0,
        "{q:?}"
    );
    // Its NURBS form agrees.
    let n = SurfaceKind::Revolution(r).to_nurbs(&mut meter()).unwrap();
    let s = &n.value.surface;
    let at0 = s.point(0.0, 0.0, SIDES).unwrap();
    assert!(
        at0.iter()
            .zip([5.0, 0.0, 0.0])
            .all(|(a, b)| (a - b).abs() <= 1e-12),
        "{at0:?}"
    );
}

fn frame() -> Frame<3> {
    Frame::new([1.0, 0.3, 0.2], [0.1, -0.4, 1.0]).unwrap()
}

fn kinds(c: [f64; 3]) -> [SurfaceKind; 4] {
    let f = frame();
    [
        SurfaceKind::Cylinder(Cylinder::new(c, 2.5, f, ((0.0, 2.0), (-1.0, 1.0))).unwrap()),
        SurfaceKind::Cone(Cone::new(c, 1.5, 0.5, f, ((0.0, 2.0), (0.0, 1.0))).unwrap()),
        SurfaceKind::Sphere(Sphere::new(c, 2.5, f, ((0.0, 2.0), (-0.5, 0.5))).unwrap()),
        SurfaceKind::Torus(Torus::new(c, 5.0, 1.5, f, ((0.0, 2.0), (0.0, 2.0))).unwrap()),
    ]
}

#[test]
fn grid_transforms_of_analytic_surfaces_are_exact() {
    let m = &mut meter();
    let grid = [
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::Y, 1)),
        TypedTransform::Exact(GridTransform::mirror(Axis::X)),
        TypedTransform::Exact(GridTransform::translation([7, -3, 5]).unwrap()),
    ];
    for k in kinds([1.5, -2.25, 3.0]) {
        for t in &grid {
            let r = k.transformed(t, m).unwrap();
            assert_eq!(r.deviation, 0.0, "{k:?} under {t:?}");
            assert_eq!(
                core::mem::discriminant(&r.value),
                core::mem::discriminant(&k)
            );
        }
    }
}

#[test]
fn rounding_moves_of_analytic_surfaces_state_their_rounding() {
    let m = &mut meter();
    // A whole-step translation that rounds the centre (0.1 + 1000 is not a double).
    let far = TypedTransform::Exact(GridTransform::translation([1000, 0, 0]).unwrap());
    // An exact rotation that is not of the grid, at a centre it moves exactly (the origin).
    let r = TypedTransform::from(
        RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap(),
    );
    for k in kinds([0.1, 0.2, 0.3]) {
        assert!(k.transformed(&far, m).unwrap().deviation > 0.0, "{k:?}");
    }
    for k in kinds([0.0, 0.0, 0.0]) {
        assert!(k.transformed(&r, m).unwrap().deviation > 0.0, "{k:?}");
    }
}

#[test]
fn the_contract_kit_steps_past_huge_domain_ends() {
    let m = &mut meter();
    let huge = 1_152_921_504_606_846_976.0; // 2^60
    let t = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    for u in [(-huge, 0.0), (0.0, huge), (-huge, huge)] {
        let p =
            geom_surface::Plane::new([0.0; 3], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0], (u, (0.0, 1.0)))
                .unwrap();
        let params = [(u.0, 0.0), (u.0 / 2.0, 0.5), (u.1 / 2.0, 0.25), (u.1, 1.0)];
        assert_eq!(
            geom_surface::contract::check(&p, &params, &t, m),
            Ok(()),
            "{u:?}"
        );
    }
}
