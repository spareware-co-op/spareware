//! Attacks on piece G65's claims that its kind contract run does not reach: each kind refuses the
//! data it cannot be (a domain past a turn by a rounding, ends that are not ordered or finite, a
//! degenerate radius); reversals and splits in v as well as u keep the surface; the image of an
//! analytic kind under a map that is not a similarity keeps the contract itself; a ruled surface
//! between curves of different degrees and knots converts on one knot vector; a placed surface's
//! derivatives do not move with its translation; and the accessors answer with what was given.
//!
//! Covers: SurfaceKind::Affine.

#![allow(
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "tests: exact comparisons and indexing, the formulas' names, and a failure stops the test"
)]

use geom_curve::{Basis as Leaf, CurveError, Frame, Line, Trimmed};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_nurbs::{Basis, Curve, Direction, Limit, NurbsError};
use geom_run::{Budget, WorkMeter};
use geom_surface::contract::check;
use geom_surface::{
    Cone, Cylinder, Extrusion, ParamBox, ParamSurface, Plane, Revolution, Ruled, Sphere,
    SurfaceKind, Torus,
};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

fn z_frame() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap()
}

fn line(a: [f64; 3], d: [f64; 3]) -> Trimmed<3> {
    Trimmed::whole(Leaf::Line(Line::new(a, d, (0.0, 1.0)).unwrap())).unwrap()
}

fn cubic(knots: Vec<f64>, pts: &[[f64; 3]]) -> Trimmed<3> {
    let basis = Basis::new(3, knots).unwrap();
    Trimmed::whole(Leaf::Nurbs(Curve::new(basis, pts).unwrap())).unwrap()
}

fn params(((a, b), (c, d)): ParamBox) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for i in 0..=4 {
        for j in 0..=4 {
            out.push((
                (a + (b - a) * f64::from(i) / 4.0).clamp(a, b),
                (c + (d - c) * f64::from(j) / 4.0).clamp(c, d),
            ));
        }
    }
    out
}

fn transforms() -> Vec<TypedTransform> {
    vec![
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1)),
        TypedTransform::Exact(GridTransform::mirror(Axis::Y)),
        TypedTransform::from(
            RationalTransform::rotation(Axis::Y, Rotation::direction(3, 4).unwrap()).unwrap(),
        ),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap()),
    ]
}

fn near(p: [f64; 3], q: [f64; 3], tol: f64) -> bool {
    p.iter().zip(&q).all(|(x, y)| (x - y).abs() <= tol)
}

/// One kind of every analytic and swept sort, bounded.
fn kinds() -> Vec<SurfaceKind> {
    let f = z_frame();
    let spline = cubic(
        vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
        &[
            [2.0, 0.0, 0.0],
            [3.0, 0.5, 1.0],
            [2.5, -0.5, 2.0],
            [4.0, 0.0, 3.0],
            [3.0, 1.0, 4.0],
        ],
    );
    vec![
        SurfaceKind::Plane(
            Plane::new(
                [1.0, 2.0, 3.0],
                [1.0, 0.5, 0.0],
                [0.0, 1.0, 2.0],
                ((-1.0, 2.0), (0.5, 3.0)),
            )
            .unwrap(),
        ),
        SurfaceKind::Cylinder(
            Cylinder::new([1.0, 0.0, 0.0], 2.0, f, ((0.25, 1.5), (-1.0, 2.0))).unwrap(),
        ),
        SurfaceKind::Cone(Cone::new([0.0; 3], 2.0, 0.5, f, ((0.0, 1.0), (-1.0, 2.0))).unwrap()),
        SurfaceKind::Sphere(
            Sphere::new([0.0, 1.0, 0.0], 3.0, f, ((0.0, 1.5), (-0.25, 0.4))).unwrap(),
        ),
        SurfaceKind::Torus(Torus::new([0.0; 3], 5.0, 2.0, f, ((0.0, 1.0), (0.25, 1.75))).unwrap()),
        SurfaceKind::Revolution(
            Revolution::new(spline.clone(), [0.0; 3], f, (0.25, 1.75)).unwrap(),
        ),
        SurfaceKind::Extrusion(
            Extrusion::new(spline.clone(), [1.0, -1.0, 0.5], (0.5, 3.0)).unwrap(),
        ),
        SurfaceKind::Ruled(
            Ruled::new(
                line([2.0, 0.0, 0.0], [-2.0, 2.0, 0.0]),
                spline.clone(),
                (0.0, 1.0),
            )
            .unwrap(),
        ),
        // A rational first curve against a polynomial second one.
        SurfaceKind::Ruled(
            Ruled::new(spline, line([2.0, 0.0, 5.0], [-2.0, 2.0, 0.0]), (0.0, 1.0)).unwrap(),
        ),
    ]
}

/// Checks that a reversal in either direction is the same surface on the negated parameter (its
/// domain negated in that direction), that a split in either direction is the same surface on
/// each side, and that neither direction splits at an end.
fn reverses_and_splits(k: &SurfaceKind, m: &mut WorkMeter<'_>) {
    let ((a, b), (c, d)) = k.domain();
    for dir in [Direction::U, Direction::V] {
        let r = k.reversed(dir, m).unwrap();
        let want = match dir {
            Direction::U => ((-b, -a), (c, d)),
            Direction::V => ((a, b), (-d, -c)),
        };
        assert_eq!(r.value.domain(), want, "{k:?}, {dir:?}");
        let tol = r.deviation + 1e-12;
        for (u, v) in params(k.domain()) {
            let p = k.point(u, v, SIDES).unwrap();
            let (ru, rv) = match dir {
                Direction::U => (-u, v),
                Direction::V => (u, -v),
            };
            let q = r.value.point(ru, rv, SIDES).unwrap();
            assert!(near(p, q, tol), "{k:?}, {dir:?} at ({u}, {v}): {p:?} {q:?}");
        }
        let (lo, hi) = match dir {
            Direction::U => (a, b),
            Direction::V => (c, d),
        };
        for end in [lo, hi] {
            assert_eq!(
                k.split(dir, end, m).err(),
                Some(CurveError::ParameterOutside),
                "{k:?}, {dir:?}"
            );
        }
        let mid = f64::midpoint(lo, hi);
        let (l, h) = k.split(dir, mid, m).unwrap();
        let (dl, dh) = match dir {
            Direction::U => (((a, mid), (c, d)), ((mid, b), (c, d))),
            Direction::V => (((a, b), (c, mid)), ((a, b), (mid, d))),
        };
        assert_eq!(
            (l.value.domain(), h.value.domain()),
            (dl, dh),
            "{k:?}, {dir:?}"
        );
        let tol = l.deviation + h.deviation + 1e-12;
        for (u, v) in params(k.domain()) {
            let t = match dir {
                Direction::U => u,
                Direction::V => v,
            };
            let part = if t <= mid { &l.value } else { &h.value };
            let q = part.point(u, v, SIDES).unwrap();
            let p = k.point(u, v, SIDES).unwrap();
            assert!(near(p, q, tol), "{k:?}, split {dir:?} at ({u}, {v})");
        }
    }
}

/// Claim: every kind reverses and splits in both directions, and keeps the contract.
#[test]
fn every_kind_reverses_and_splits_in_both_directions() {
    let m = &mut meter();
    let turn = TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap());
    for k in kinds() {
        reverses_and_splits(&k, m);
        check(&k, &params(k.domain()), &turn, m).unwrap_or_else(|e| panic!("{k:?}: {e}"));
    }
}

/// Claim: the image of an analytic kind under a map that is not a similarity keeps the kind
/// contract on its own, and reverses and splits in both directions.
#[test]
fn affine_images_keep_the_contract() {
    let m = &mut meter();
    let squash = TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap());
    let f = z_frame();
    let sources = [
        SurfaceKind::Cylinder(
            Cylinder::new([1.0, 0.0, 0.0], 2.0, f, ((0.25, 1.5), (-1.0, 2.0))).unwrap(),
        ),
        SurfaceKind::Torus(Torus::new([0.0; 3], 5.0, 2.0, f, ((0.0, 1.0), (0.25, 1.75))).unwrap()),
    ];
    for s in sources {
        let image = s.transformed(&squash, m).unwrap().value;
        assert!(matches!(image, SurfaceKind::Affine(_)), "{image:?}");
        for t in transforms() {
            check(&image, &params(image.domain()), &t, m)
                .unwrap_or_else(|e| panic!("{image:?} under {t:?}: {e}"));
        }
        reverses_and_splits(&image, m);
        // Its points are the source's, moved.
        for (u, v) in params(s.domain()) {
            let p = s.point(u, v, SIDES).unwrap();
            let q = image.point(u, v, SIDES).unwrap();
            assert!(
                near([2.0 * p[0], 0.5 * p[1], 3.0 * p[2]], q, 1e-12),
                "({u}, {v})"
            );
        }
    }
}

/// Claim: each analytic kind refuses what it cannot be: a centre or radius that is not finite, a u
/// domain more than a turn wide (by a rounding too), ends that are not ordered or finite, a cone
/// with no radius and no slope, a torus without both radii, and an empty band of a sphere.
#[test]
fn analytic_kinds_refuse_what_they_cannot_be() {
    let f = z_frame();
    let cyl = |c: [f64; 3], r: f64, d: ParamBox| Cylinder::new(c, r, f, d).err();
    let ok: ParamBox = ((0.0, 1.0), (0.0, 1.0));
    assert_eq!(
        cyl([f64::NAN, 0.0, 0.0], 1.0, ok),
        Some(CurveError::NotFinite)
    );
    assert_eq!(
        cyl([0.0; 3], f64::INFINITY, ok),
        Some(CurveError::NotFinite)
    );
    assert_eq!(
        Plane::new([0.0; 3], [f64::NAN, 0.0, 0.0], [0.0, 1.0, 0.0], ok).err(),
        Some(CurveError::NotFinite)
    );
    // −10⁻¹⁷ to 2 is a rounding more than a turn: 2 + 10⁻¹⁷ rounds to 2.
    for u in [
        (1.0, 0.0),
        (0.5, 0.5),
        (-1e-17, 2.0),
        (0.0, 2.5),
        (0.0, f64::INFINITY),
        (f64::NAN, 1.0),
    ] {
        assert_eq!(
            cyl([0.0; 3], 1.0, (u, (0.0, 1.0))),
            Some(CurveError::SpanOutside),
            "{u:?}"
        );
    }
    for v in [(0.0, f64::INFINITY), (1.0, 0.0), (0.5, 0.5)] {
        assert_eq!(
            cyl([0.0; 3], 1.0, ((0.0, 1.0), v)),
            Some(CurveError::SpanOutside),
            "{v:?}"
        );
    }
    assert_eq!(cyl([0.0; 3], 1.0, ((0.0, 2.0), (0.0, 1.0))), None);
    assert_eq!(
        Cone::new([0.0; 3], 0.0, 0.0, f, ok).err(),
        Some(CurveError::Degenerate)
    );
    // An apex at v = 0 is a cone.
    assert!(Cone::new([0.0; 3], 0.0, 1.0, f, ok).is_ok());
    for (big, small) in [(3.0, 0.0), (0.0, 1.0)] {
        assert_eq!(
            Torus::new([0.0; 3], big, small, f, ok).err(),
            Some(CurveError::Degenerate)
        );
    }
    assert_eq!(
        Sphere::new([0.0; 3], 1.0, f, ((0.0, 1.0), (0.2, 0.2))).err(),
        Some(CurveError::SpanOutside)
    );
}

/// Claim: the accessors answer with what was given.
#[test]
fn analytic_kinds_keep_their_data() {
    let f = z_frame();
    let ok: ParamBox = ((0.0, 1.0), (0.0, 1.0));
    assert_eq!(
        Cone::new([0.0; 3], 2.5, -0.5, f, ok)
            .unwrap()
            .radius_and_slope(),
        (2.5, -0.5)
    );
    assert_eq!(
        Sphere::new([0.0; 3], 3.5, f, ((0.0, 1.0), (0.0, 0.5)))
            .unwrap()
            .radius(),
        3.5
    );
    assert_eq!(
        Torus::new([0.0; 3], 5.5, 2.5, f, ok).unwrap().radii(),
        (5.5, 2.5)
    );
    let p = Plane::new([1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 10.0], ok).unwrap();
    assert_eq!(
        p.vectors(),
        ([1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 10.0])
    );
}

/// Claim: a plane unbounded on one side has no NURBS form, and moves only under a transform that
/// carries its vectors exactly; a bounded one splits only inside.
#[test]
fn unbounded_planes_move_only_exactly() {
    let m = &mut meter();
    // Turned by the rotation with cosine 3/5 (not a double), the vectors' images round.
    let half = Plane::new(
        [0.0; 3],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        ((0.0, 1.0), (0.0, f64::INFINITY)),
    )
    .unwrap();
    assert_eq!(half.to_nurbs(m).err(), Some(CurveError::Unbounded));
    let turn = TypedTransform::from(
        RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap(),
    );
    assert_eq!(
        half.transformed(&turn, m).err(),
        Some(CurveError::Unbounded)
    );
    let quarter = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    assert_eq!(half.transformed(&quarter, m).unwrap().deviation, 0.0);
}

/// Claim: a cylinder's bound over a quarter turn from u = 0 holds the point at its end, where the
/// sine term's enclosure has 0 at one end only.
#[test]
fn bounds_keep_terms_that_vanish_at_one_end() {
    let m = &mut meter();
    let c = Cylinder::new([0.0; 3], 2.0, z_frame(), ((0.0, 1.0), (0.0, 1.0))).unwrap();
    let b = c.bound(((0.0, 0.25), (0.0, 1.0)), m).unwrap();
    for u in [0.0, 0.125, 0.25] {
        let p = c.point(u, 0.5, SIDES).unwrap();
        for i in 0..3 {
            assert!(b.lo[i] <= p[i] && p[i] <= b.hi[i], "u = {u}: {p:?} {b:?}");
        }
    }
}

/// Claim: a ruled surface between curves of different degrees and interior knots (repeated in
/// one) converts on one knot vector within its error, exactly between two lines, and refuses
/// curves on different intervals.
#[test]
fn ruled_surfaces_join_curves_of_any_knots() {
    let m = &mut meter();
    let a = cubic(
        vec![0.0, 0.0, 0.0, 0.0, 0.25, 0.25, 1.0, 1.0, 1.0, 1.0],
        &[
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [2.0, 0.5, 0.0],
            [3.0, 1.0, 0.0],
            [4.0, 1.0, 0.0],
            [5.0, 0.0, 0.0],
        ],
    );
    let b = cubic(
        vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.75, 1.0, 1.0, 1.0, 1.0],
        &[
            [0.0, 0.0, 2.0],
            [1.0, -1.0, 2.0],
            [2.0, 0.0, 2.5],
            [3.0, 1.0, 2.0],
            [4.0, 0.0, 2.0],
            [5.0, 1.0, 2.0],
        ],
    );
    let straight = line([0.0, 0.0, 4.0], [5.0, 0.0, 0.0]);
    for (x, y) in [(a.clone(), b), (straight, a)] {
        let r = SurfaceKind::Ruled(Ruled::new(x, y, (0.0, 1.0)).unwrap());
        let n = r.to_nurbs(m).unwrap();
        assert!(n.deviation < 1e-12, "{}", n.deviation);
        for t in transforms() {
            check(&r, &params(r.domain()), &t, m).unwrap_or_else(|e| panic!("{t:?}: {e}"));
        }
    }
    let lines = Ruled::new(
        line([0.0; 3], [1.0, 0.0, 0.0]),
        line([0.0, 1.0, 1.0], [2.0, 0.0, 0.0]),
        (0.0, 1.0),
    )
    .unwrap();
    assert_eq!(lines.to_nurbs(m).unwrap().deviation, 0.0);
    let other = Trimmed::new(
        Leaf::Line(Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 2.0)).unwrap()),
        (0.0, 2.0),
    )
    .unwrap();
    assert_eq!(
        Ruled::new(line([0.0; 3], [0.0, 1.0, 0.0]), other, (0.0, 1.0)).err(),
        Some(CurveError::SpanOutside)
    );
}

/// Claim: a translated swept surface's derivatives are the original's: only its points move.
#[test]
fn translations_move_points_not_derivatives() {
    let m = &mut meter();
    let e = SurfaceKind::Extrusion(
        Extrusion::new(
            line([1.0, 0.0, 0.0], [0.0, 2.0, 0.0]),
            [0.0, 0.0, 3.0],
            (0.0, 1.0),
        )
        .unwrap(),
    );
    let shift = TypedTransform::Exact(GridTransform::translation([5, 0, 0]).unwrap());
    let moved = e.transformed(&shift, m).unwrap().value;
    assert!(matches!(moved, SurfaceKind::Extrusion(_)));
    for (u, v) in params(e.domain()) {
        let p = e.partials(u, v, SIDES).unwrap();
        let q = moved.partials(u, v, SIDES).unwrap();
        assert!(
            near(q[0], [p[0][0] + 5.0, p[0][1], p[0][2]], 1e-12),
            "({u}, {v})"
        );
        assert!(
            near(q[1], p[1], 1e-12) && near(q[2], p[2], 1e-12),
            "({u}, {v})"
        );
    }
}

fn quadratic(knots: Vec<f64>, pts: &[[f64; 3]]) -> Trimmed<3> {
    let basis = Basis::new(2, knots).unwrap();
    Trimmed::whole(Leaf::Nurbs(Curve::new(basis, pts).unwrap())).unwrap()
}

/// Claim: a ruled surface with an unclamped profile is refused as such (the NURBS bounds its
/// Taylor forms need take clamped curves), not as a defect; one between two curves broken at the
/// same parameter converts exactly, as they are.
#[test]
fn ruled_surfaces_convert_unclamped_and_broken_curves() {
    let m = &mut meter();
    let unclamped = quadratic(
        vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        &[[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]],
    );
    assert_eq!(unclamped.interval(), (2.0, 3.0));
    let other = Trimmed::new(
        Leaf::Line(
            Line::new(
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                (f64::NEG_INFINITY, f64::INFINITY),
            )
            .unwrap(),
        ),
        (2.0, 3.0),
    )
    .unwrap();
    let r = SurfaceKind::Ruled(Ruled::new(unclamped, other, (0.0, 1.0)).unwrap());
    assert_eq!(
        r.to_nurbs(m).err(),
        Some(CurveError::Nurbs(NurbsError::NotClamped))
    );
    let broken = |z: f64| {
        quadratic(
            vec![0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0],
            &[
                [0.0, 0.0, z],
                [1.0, 1.0, z],
                [2.0, 0.0, z],
                [2.0, 0.5, z],
                [3.0, 1.0, z],
                [4.0, 0.0, z],
            ],
        )
    };
    let r = Ruled::new(broken(0.0), broken(2.0), (0.0, 1.0)).unwrap();
    assert_eq!(r.to_nurbs(m).unwrap().deviation, 0.0);
}
