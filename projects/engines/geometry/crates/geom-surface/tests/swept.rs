//! The swept surface kinds (piece G65) under the kind contract: surfaces of revolution, linear
//! extrusions and ruled surfaces on line, arc and NURBS profiles keep every law under exact,
//! rational, certified, non-uniform and mirroring transforms, and convert to NURBS within their
//! stated error.
//!
//! Covers: SurfaceKind::Revolution, SurfaceKind::Extrusion, SurfaceKind::Ruled.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests: native arithmetic and indexing, the formulas' names, and a failure stops the test"
)]

use geom_curve::{Basis as Leaf, Circle, Frame, Line, ParamMap, Trimmed};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_nurbs::{Basis, Curve, Direction, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::contract::check;
use geom_surface::{Extrusion, ParamBox, ParamSurface, Revolution, Ruled, SurfaceKind};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
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

fn line(a: [f64; 3], d: [f64; 3]) -> Trimmed<3> {
    Trimmed::whole(Leaf::Line(Line::new(a, d, (0.0, 1.0)).unwrap())).unwrap()
}

fn arc(c: [f64; 3], r: f64, x: [f64; 3], n: [f64; 3], span: (f64, f64)) -> Trimmed<3> {
    let circle = Circle::new(c, r, Frame::new(x, n).unwrap(), span).unwrap();
    Trimmed::whole(Leaf::Circle(circle)).unwrap()
}

fn spline() -> Trimmed<3> {
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let pts = [
        [2.0, 0.0, 0.0],
        [3.0, 0.5, 1.0],
        [2.5, -0.5, 2.0],
        [4.0, 0.0, 3.0],
        [3.0, 1.0, 4.0],
    ];
    let w = [1.0, 2.0, 1.0, 0.5, 1.0];
    Trimmed::whole(Leaf::Nurbs(Curve::new_rational(basis, &pts, &w).unwrap())).unwrap()
}

fn kinds() -> Vec<SurfaceKind> {
    let axis = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    vec![
        // A cone's side, a sphere-like zone and a freeform vase, turned about z.
        SurfaceKind::Revolution(
            Revolution::new(
                line([1.0, 0.0, 0.0], [1.0, 0.0, 2.0]),
                [0.0; 3],
                axis,
                (0.0, 1.5),
            )
            .unwrap(),
        ),
        SurfaceKind::Revolution(
            Revolution::new(
                arc(
                    [0.0, 0.0, 1.0],
                    2.0,
                    [1.0, 0.0, 0.0],
                    [0.0, -1.0, 0.0],
                    (-0.4, 0.4),
                ),
                [0.0, 0.0, 0.0],
                axis,
                (-1.0, 1.0),
            )
            .unwrap(),
        ),
        SurfaceKind::Revolution(Revolution::new(spline(), [0.0; 3], axis, (0.25, 1.75)).unwrap()),
        // Extrusions of an arc and of the spline.
        SurfaceKind::Extrusion(
            Extrusion::new(
                arc([0.0; 3], 3.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], (0.0, 1.0)),
                [0.0, 1.0, 4.0],
                (-1.0, 2.0),
            )
            .unwrap(),
        ),
        SurfaceKind::Extrusion(Extrusion::new(spline(), [1.0, -1.0, 0.5], (0.0, 3.0)).unwrap()),
        // Ruled: two arcs on one map; an arc and a line on different maps.
        SurfaceKind::Ruled(
            Ruled::new(
                arc([0.0; 3], 2.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], (0.0, 1.0)),
                arc(
                    [0.0, 0.0, 3.0],
                    4.0,
                    [1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0],
                    (0.0, 1.0),
                ),
                (0.0, 1.0),
            )
            .unwrap(),
        ),
        SurfaceKind::Ruled(
            Ruled::new(
                Trimmed::whole(Leaf::Line(
                    Line::new([2.0, 0.0, 0.0], [-2.0, 2.0, 0.0], (0.0, 1.0)).unwrap(),
                ))
                .unwrap(),
                arc(
                    [0.0, 0.0, 2.0],
                    2.0,
                    [1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0],
                    (0.0, 1.0),
                ),
                (0.0, 1.0),
            )
            .unwrap(),
        ),
    ]
}

#[test]
fn every_swept_kind_keeps_the_contract() {
    for (i, k) in kinds().into_iter().enumerate() {
        for t in transforms() {
            check(&k, &params(k.domain()), &t, &mut meter())
                .unwrap_or_else(|e| panic!("kind {i}: {k:?} under {t:?}: {e}"));
        }
    }
}

#[test]
fn swept_kinds_convert_within_their_error() {
    let m = &mut meter();
    for (i, k) in kinds().into_iter().enumerate() {
        let n = k.to_nurbs(m).unwrap();
        assert!(n.deviation < 1e-9, "kind {i}: {}", n.deviation);
    }
    // A cylinder turned from an axis-parallel line on whole numbers converts exactly.
    let axis = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let r = Revolution::new(
        line([3.0, 0.0, 0.0], [0.0, 0.0, 2.0]),
        [0.0; 3],
        axis,
        (0.0, 2.0),
    )
    .unwrap();
    let n = r.to_nurbs(m).unwrap();
    assert_eq!(n.deviation, 0.0);
    assert!(matches!(n.value.u, ParamMap::Circular(_)));
    // Transforms keep the kind, on the same parameters.
    let squash = TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap());
    assert!(matches!(
        r.transformed(&squash, m).unwrap().value,
        SurfaceKind::Revolution(_)
    ));
    let p = r
        .point(0.5, 1.0, (Limit::FromRight, Limit::FromRight))
        .unwrap();
    assert!((p[0]).abs() < 1e-15 && (p[1] - 3.0).abs() < 1e-15 && (p[2] - 2.0).abs() < 1e-15);
    let (l, rr) = r.split(Direction::V, 0.5, m).unwrap();
    assert_eq!((l.deviation, rr.deviation), (0.0, 0.0));
}
