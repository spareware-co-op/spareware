//! Attack tests from SMLib's confirmed defects, batch 2: `geom-surface`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: the right point or derivative, from an independent
//! formula or a finite difference, or a defined refusal, and never a panic, a hang, a NaN or a
//! wrong value.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::manual_midpoint,
    clippy::type_complexity,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::needless_range_loop,
    clippy::range_minus_one,
    reason = "tests: native arithmetic, casts and indexing, platform maths on an independent oracle (the formula as it is written), prose that names defect files, and a failure stops the test"
)]

use geom_curve::{Basis as Leaf, CurveError, Frame, Line, Trimmed};
use geom_nurbs::{Direction, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, Offset, ParamSurface, Plane, Revolution, Sphere, SurfaceKind, Torus,
    partial_index,
};

const RIGHT: Limit = Limit::FromRight;
const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 38))
}

fn frame() -> Frame<3> {
    Frame::standard().unwrap()
}

fn agree(a: [f64; 3], b: [f64; 3], tol: f64, what: &str) {
    for d in 0..3 {
        assert!((a[d] - b[d]).abs() <= tol, "{what}: {a:?} against {b:?}");
    }
}

/// solidcore plan/upstream-issues/UB-68--smplane-hazards.md (b4de2149): `SmPlane(O, (0, 0, 1e-13))`
/// (a plane from a normal of almost no length) and `Reverse` on a plane with an inverted domain
/// were hazards. The inputs are the defects': a plane made of vectors of length 1e-13, and every
/// analytic kind reversed in u and in v and then back. A reversed surface at the negated
/// parameter is the original at the original parameter, its domain is the negated one, and the
/// plane of tiny vectors still gives the points its vectors say.
#[test]
fn ub_68_reversing_each_analytic_kind_and_a_plane_of_tiny_vectors() {
    let kinds = [
        (
            "plane",
            SurfaceKind::Plane(
                Plane::new(
                    [1.0, 2.0, 3.0],
                    [2.0, 0.0, 0.0],
                    [0.0, 3.0, 0.0],
                    ((0.0, 1.0), (0.0, 1.0)),
                )
                .unwrap(),
            ),
        ),
        (
            "tiny plane",
            SurfaceKind::Plane(
                Plane::new(
                    [0.0; 3],
                    [1e-13, 0.0, 0.0],
                    [0.0, 1e-13, 0.0],
                    ((0.0, 1.0), (0.0, 2.0)),
                )
                .unwrap(),
            ),
        ),
        (
            "cylinder",
            SurfaceKind::Cylinder(
                Cylinder::new([0.0; 3], 2.0, frame(), ((0.0, 1.5), (0.0, 3.0))).unwrap(),
            ),
        ),
        (
            "cone",
            SurfaceKind::Cone(
                Cone::new([0.0; 3], 1.0, 0.5, frame(), ((0.0, 1.0), (0.0, 2.0))).unwrap(),
            ),
        ),
        (
            "sphere",
            SurfaceKind::Sphere(
                Sphere::new([0.0; 3], 3.0, frame(), ((0.0, 1.5), (-0.25, 0.5))).unwrap(),
            ),
        ),
        (
            "torus",
            SurfaceKind::Torus(
                Torus::new([0.0; 3], 5.0, 1.0, frame(), ((0.0, 1.0), (-0.5, 0.5))).unwrap(),
            ),
        ),
    ];
    for (name, k) in &kinds {
        let ((u0, u1), (v0, v1)) = k.domain();
        for dir in [Direction::U, Direction::V] {
            let rev = k.reversed(dir, &mut meter()).unwrap();
            assert!(rev.deviation >= 0.0, "{name}");
            let rev = rev.value;
            let want = match dir {
                Direction::U => ((-u1, -u0), (v0, v1)),
                Direction::V => ((u0, u1), (-v1, -v0)),
            };
            assert_eq!(rev.domain(), want, "{name} {dir:?}");
            for tu in [0.0, 0.3, 0.5, 1.0] {
                for tv in [0.0, 0.4, 1.0] {
                    let (u, v) = (u0 + tu * (u1 - u0), v0 + tv * (v1 - v0));
                    let (ru, rv) = match dir {
                        Direction::U => (-u, v),
                        Direction::V => (u, -v),
                    };
                    let a = k.point(u, v, SIDES).unwrap();
                    let b = rev.point(ru, rv, SIDES).unwrap();
                    let scale = 1.0 + a.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
                    agree(a, b, 1e-12 * scale, &format!("{name} {dir:?} ({u}, {v})"));
                }
            }
            // And back: the original domain and points.
            let back = rev.reversed(dir, &mut meter()).unwrap().value;
            assert_eq!(back.domain(), k.domain(), "{name} {dir:?}");
            let (u, v) = (u0 + 0.3 * (u1 - u0), v0 + 0.6 * (v1 - v0));
            let scale = 1.0
                + k.point(u, v, SIDES)
                    .unwrap()
                    .iter()
                    .fold(0.0_f64, |m, x| m.max(x.abs()));
            agree(
                k.point(u, v, SIDES).unwrap(),
                back.point(u, v, SIDES).unwrap(),
                1e-12 * scale,
                name,
            );
        }
    }
    // The plane of tiny vectors gives the points its vectors say, exactly where they are exact.
    let tiny = Plane::new(
        [0.0; 3],
        [1e-13, 0.0, 0.0],
        [0.0, 1e-13, 0.0],
        ((0.0, 1.0), (0.0, 2.0)),
    )
    .unwrap();
    let p = tiny.point(0.5, 1.5, SIDES).unwrap();
    agree(p, [0.5e-13, 1.5e-13, 0.0], 1e-28, "tiny plane");
    // Vectors parallel at any length, or with a zero one, are not a plane.
    for (a, b) in [
        ([1e-13, 0.0, 0.0], [3e-13, 0.0, 0.0]),
        ([0.0; 3], [0.0, 1.0, 0.0]),
        ([1.0, 1.0, 0.0], [-2.0, -2.0, 0.0]),
    ] {
        assert_eq!(
            Plane::new([0.0; 3], a, b, ((0.0, 1.0), (0.0, 1.0))).err(),
            Some(CurveError::Degenerate),
            "{a:?} {b:?}"
        );
    }
}

/// solidcore plan/upstream-issues/UB-172--smplane-evaluate-wraps-its-loop-bound-for-bonlyupperhalf.md
/// (b4de2149): `SmPlane::Evaluate` with the upper half of the partials and a three-entry array
/// wrapped its loop bound and wrote far past the array. The input is the defect's: the partials of
/// a plane at every order from 0 to 6. The list has (n + 1)(n + 2) / 2 entries, the point and the
/// two vectors are exact, and every partial of order 2 and more is zero.
#[test]
fn ub_172_the_partials_of_a_plane_to_any_order_are_its_vectors_and_zeros() {
    let a = [2.0, 0.5, 0.0];
    let b = [0.0, 3.0, -1.0];
    let o = [1.0, 2.0, 3.0];
    let plane = Plane::new(o, a, b, ((0.0, 1.0), (0.0, 1.0))).unwrap();
    let kind = SurfaceKind::Plane(plane.clone());
    for order in 0..=6usize {
        for region in [
            ((0.3, 0.3), (0.4, 0.4)),
            ((0.0, 1.0), (0.0, 1.0)),
            ((0.2, 0.7), (0.1, 0.1)),
        ] {
            let direct = plane
                .partials_over(region, SIDES, order, &mut meter())
                .unwrap();
            let through = kind
                .partials_over(region, SIDES, order, &mut meter())
                .unwrap();
            assert_eq!(direct.len(), (order + 1) * (order + 2) / 2, "order {order}");
            assert_eq!(through.len(), direct.len());
            for i in 0..=order {
                for j in 0..=(order - i) {
                    let e = direct[partial_index(i, j).unwrap()];
                    for d in 0..3 {
                        let (lo, hi) = (e[d].lo(), e[d].hi());
                        let (u0, u1) = region.0;
                        let (v0, v1) = region.1;
                        let (w0, w1) = match (i, j) {
                            (0, 0) => (
                                o[d] + (a[d] * u0).min(a[d] * u1) + (b[d] * v0).min(b[d] * v1),
                                o[d] + (a[d] * u0).max(a[d] * u1) + (b[d] * v0).max(b[d] * v1),
                            ),
                            (1, 0) => (a[d], a[d]),
                            (0, 1) => (b[d], b[d]),
                            _ => (0.0, 0.0),
                        };
                        assert!(
                            lo <= w0 + 1e-12 && hi >= w1 - 1e-12,
                            "order {order}, partial ({i}, {j}), axis {d}: [{lo}, {hi}] against [{w0}, {w1}]"
                        );
                        if (i + j) >= 1 {
                            assert!(
                                lo >= w0 - 1e-12 && hi <= w1 + 1e-12,
                                "order {order}, partial ({i}, {j}), axis {d}: [{lo}, {hi}] is wider than [{w0}, {w1}]"
                            );
                        }
                    }
                }
            }
        }
    }
}

/// solidcore plan/upstream-issues/UB-88--smcone-step-to-nurb-conversion-dereferences-a-missing-polar.md
/// and UB-153--smcone-createoffsetsurface-dereferences-an-unchecked.md (b4de2149): a cylinder with
/// a negative radius converted to NURBS without its polar curve and was dereferenced later, and the
/// offset of a bounded cone with bottom radius 0 and a negative top radius, offset so that the
/// radii leave their range, dereferenced an unchecked result. The inputs are the defects': a
/// cylinder of radius -1.8, cones whose radius runs through zero or is negative, and their offsets
/// by distances that take the radius past zero. Each is refused, or evaluates to finite points
/// on the cone (the distance from the axis is the magnitude of the radius), and converts to NURBS
/// with a finite deviation or a typed refusal.
#[test]
fn ub_88_ub_153_negative_radii_and_offsets_past_zero_are_refused_or_exact() {
    let f = frame();
    for r in [-1.8, 0.0, -1e-300, f64::NAN, f64::NEG_INFINITY] {
        assert!(
            Cylinder::new([0.0; 3], r, f, ((0.0, 2.0), (0.0, 4.3))).is_err(),
            "cylinder radius {r}"
        );
    }
    // Radius r(v) = r0 + slope v over v in [0, 2]: negative throughout, through zero, and from zero.
    for (r0, slope) in [
        (0.0, -1.0),
        (-1.0, -0.5),
        (1.0, -1.0),
        (0.5, -0.25),
        (0.0, 1.0),
    ] {
        let cone = match Cone::new([0.0; 3], r0, slope, f, ((0.0, 2.0), (0.0, 2.0))) {
            Ok(c) => c,
            Err(e) => {
                assert!(!matches!(e, CurveError::Defect), "{r0} {slope}: {e:?}");
                continue;
            }
        };
        let kind = SurfaceKind::Cone(cone);
        for (u, v) in [(0.0, 0.0), (0.3, 0.7), (1.0, 1.0), (1.7, 2.0), (2.0, 2.0)] {
            let p = kind.point(u, v, SIDES).unwrap();
            let radius = (r0 + slope * v).abs();
            assert!(p.iter().all(|x| x.is_finite()), "{r0} {slope}: {p:?}");
            assert!(
                (p[0].hypot(p[1]) - radius).abs() < 1e-12,
                "{r0} {slope} ({u}, {v}): {p:?}"
            );
            assert!((p[2] - v).abs() < 1e-12);
        }
        match kind.to_nurbs(&mut meter()) {
            Ok(n) => assert!(
                n.deviation.is_finite() && n.deviation >= 0.0,
                "{r0} {slope}"
            ),
            Err(e) => assert!(!matches!(e, CurveError::Defect), "{r0} {slope}: {e:?}"),
        }
        // The offsets, including those that take the radius past zero: refused, or finite.
        for distance in [0.25, -0.25, 1.0, -1.0, 5.0, -5.0] {
            let off = match Offset::new(kind.clone(), distance) {
                Ok(o) => o,
                Err(e) => {
                    assert!(
                        !matches!(e, CurveError::Defect),
                        "{r0} {slope} {distance}: {e:?}"
                    );
                    continue;
                }
            };
            for (u, v) in [(0.3, 0.7), (1.0, 1.0), (1.7, 1.9)] {
                match off.point(u, v, SIDES) {
                    Ok(p) => assert!(
                        p.iter().all(|x| x.is_finite()),
                        "{r0} {slope} {distance} ({u}, {v}): {p:?}"
                    ),
                    Err(e) => assert!(
                        !matches!(e, CurveError::Defect),
                        "{r0} {slope} {distance}: {e:?}"
                    ),
                }
            }
            match SurfaceKind::Offset(off).to_nurbs(&mut meter()) {
                Ok(n) => assert!(!n.deviation.is_nan(), "{r0} {slope} {distance}"),
                Err(e) => assert!(
                    !matches!(e, CurveError::Defect),
                    "{r0} {slope} {distance}: {e:?}"
                ),
            }
        }
    }
}

/// solidcore plan/upstream-issues/WR-408--torus-rebuilt-from-its-nurb-mirrors-a-generating-interval-below-its-equator.md
/// (b4de2149): a torus rebuilt from its NURBS mirrored a generating interval that starts below the
/// equator (a half disc whose arc runs from -90 to 90 degrees, revolved a quarter). The input is
/// the defect's: a torus whose v interval starts at -1/2 (and others below the equator), over a
/// quarter turn in u. The NURBS form of the surface holds the torus's points, v below the equator
/// included, within the certified deviation.
#[test]
fn wr_408_a_torus_below_its_equator_converts_to_the_same_surface() {
    let (major, minor) = (5.0, 1.5);
    let at = |u: f64, v: f64| {
        let pi = std::f64::consts::PI;
        let ring = major + minor * (pi * v).cos();
        [
            ring * (pi * u).cos(),
            ring * (pi * u).sin(),
            minor * (pi * v).sin(),
        ]
    };
    for (du, dv) in [
        ((0.0, 0.5), (-0.5, 0.5)),
        ((0.0, 0.5), (-0.5, 0.0)),
        ((0.0, 0.5), (-1.0, 0.0)),
        ((-0.25, 0.25), (-0.25, 0.75)),
        ((0.0, 1.0), (-0.5, 1.5)),
        ((0.0, 2.0), (-0.5, 0.5)),
    ] {
        let t = Torus::new([0.0; 3], major, minor, frame(), (du, dv)).unwrap();
        // The torus itself is the formula, at the corners and inside.
        for (su, sv) in [(0.0, 0.0), (0.37, 0.61), (1.0, 1.0), (0.5, 0.2)] {
            let (u, v) = (du.0 + su * (du.1 - du.0), dv.0 + sv * (dv.1 - dv.0));
            agree(
                t.point(u, v, SIDES).unwrap(),
                at(u, v),
                1e-12 * 10.0,
                "torus point",
            );
        }
        let n = match t.to_nurbs(&mut meter()) {
            Ok(n) => n,
            Err(e) => {
                assert!(!matches!(e, CurveError::Defect), "{du:?} {dv:?}: {e:?}");
                continue;
            }
        };
        for (su, sv) in [(0.0, 0.0), (0.2, 0.7), (0.5, 0.5), (0.9, 0.1), (1.0, 1.0)] {
            let (u, v) = (du.0 + su * (du.1 - du.0), dv.0 + sv * (dv.1 - dv.0));
            let (iu, iv) = (n.value.u.enclose(u).unwrap(), n.value.v.enclose(v).unwrap());
            let (mu, mv) = (iu.lo() * 0.5 + iu.hi() * 0.5, iv.lo() * 0.5 + iv.hi() * 0.5);
            let p = n.value.surface.point(mu, mv, (RIGHT, RIGHT)).unwrap();
            agree(
                p,
                at(u, v),
                1e-9 + n.deviation,
                &format!("nurbs of the torus {du:?} {dv:?} at ({u}, {v})"),
            );
        }
    }
}

/// solidcore plan/upstream-issues/UB-70--smsurfofrevolution-constructor-re-enters-itself-through-the.md
/// (b4de2149): a surface of revolution built on a line from its two-point constructor re-entered
/// its own constructor. The input is the defect's: a line turned about an axis, in the three ways
/// a line can lie against the axis: parallel (a cylinder), meeting it (a cone), crossing it (a
/// disc), and lying on it (nothing to turn). The first three have the radius of the line's point
/// from the axis at every angle, and the last is refused or lies on the axis.
#[test]
fn ub_70_a_line_turned_about_an_axis_in_each_of_its_positions() {
    let axis = frame();
    let line = |a: [f64; 3], b: [f64; 3]| {
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        Trimmed::whole(Leaf::Line(Line::<3>::new(a, d, (0.0, 1.0)).unwrap())).unwrap()
    };
    // (name, from, to): the radius from the axis and the height at the line's parameter t.
    let cases: [(&str, [f64; 3], [f64; 3]); 3] = [
        ("cylinder", [1.0, 0.0, 1.0], [1.0, 0.0, 3.0]),
        ("cone", [1.0, 0.0, 0.0], [3.0, 0.0, 2.0]),
        ("disc", [-1.0, 0.0, 0.5], [2.0, 0.0, 0.5]),
    ];
    for (name, a, b) in cases {
        let radius = |t: f64| (a[0] + (b[0] - a[0]) * t).abs();
        let height = |t: f64| a[2] + (b[2] - a[2]) * t;
        let s = Revolution::new(line(a, b), [0.0; 3], axis, (0.0, 2.0)).unwrap();
        let ((u0, u1), (v0, v1)) = s.domain();
        assert_eq!((u0, u1), (0.0, 2.0), "{name}");
        for iv in 0..=8 {
            let v = v0 + (v1 - v0) * f64::from(iv) / 8.0;
            for iu in 0..=8 {
                let u = u0 + (u1 - u0) * f64::from(iu) / 8.0;
                let p = s.point(u, v, SIDES).unwrap();
                assert!(p.iter().all(|x| x.is_finite()), "{name}: {p:?}");
                assert!(
                    (p[0].hypot(p[1]) - radius(v)).abs() < 1e-12,
                    "{name} ({u}, {v}): radius {} against {}",
                    p[0].hypot(p[1]),
                    radius(v)
                );
                assert!((p[2] - height(v)).abs() < 1e-12, "{name} ({u}, {v}): {p:?}");
            }
        }
    }
    // A line along the axis has nothing to turn: refused, or every point is on the axis.
    match Revolution::new(
        line([0.0, 0.0, 0.0], [0.0, 0.0, 2.0]),
        [0.0; 3],
        axis,
        (0.0, 2.0),
    ) {
        Err(e) => assert!(!matches!(e, CurveError::Defect), "{e:?}"),
        Ok(s) => {
            for (u, v) in [(0.0, 0.0), (0.7, 0.5), (1.9, 1.0)] {
                match s.point(u, v, SIDES) {
                    Ok(p) => assert!(p[0].hypot(p[1]) < 1e-12, "{p:?}"),
                    Err(e) => assert!(!matches!(e, CurveError::Defect), "{e:?}"),
                }
            }
        }
    }
    // More than a turn is refused.
    assert!(
        Revolution::new(
            line([1.0, 0.0, 0.0], [1.0, 0.0, 1.0]),
            [0.0; 3],
            axis,
            (0.0, 2.5)
        )
        .is_err()
    );
}
