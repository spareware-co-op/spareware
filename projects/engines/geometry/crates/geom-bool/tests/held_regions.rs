//! Regions two edges' intersection could not decide (G243, chapter 11.4 steps 2 and 3): held
//! until the common blocks are known, and accepted only where a common block covers the region on
//! both its edges, or where it touches a vertex both edges pass through. Two circles of radius
//! 1105 whose centres lie 2210 steps apart, the second's radius a double's step past 1105, are
//! such a region: they overlap by a fraction of a step's billionth around (1105, 0), and their
//! closed form cannot hold the two crossings apart, so it makes them one tangent band there.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_bool::carrier::PointGeom;
use geom_bool::interfere::{
    Arrangement, BuildError, EdgeIn, Entity, Interfered, Interference, VertexIn, interfere,
};
use geom_bool::merge::default_remaining;
use geom_curve::{Circle, CurveKind, Frame, ParamCurve};
use geom_intersect::Unresolved;
use geom_num::P3;
use geom_run::{Budget, Exec, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 30))
}

/// The first circle, about the origin, starting from `start` (a unit vector in z = 0).
fn first(start: [f64; 2]) -> Circle<3> {
    let frame = Frame::new([start[0], start[1], 0.0], [0.0, 0.0, 1.0]).unwrap();
    Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (0.0, 2.0)).unwrap()
}

/// The second circle, about (2210, 0), a double's step wider.
fn second() -> Circle<3> {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    Circle::new([2210.0, 0.0, 0.0], 1105.0_f64.next_up(), frame, (0.0, 2.0)).unwrap()
}

/// Closed edges on `curves`, each of its own operand from 0, then grid vertices of the operand
/// after them.
fn arrangement(curves: Vec<CurveKind<3>>, vertices: &[[i64; 2]]) -> Arrangement {
    let mut arr = Arrangement::default();
    for (operand, curve) in (0..).zip(curves) {
        let curve = arr.carriers.add_curve(curve).unwrap();
        arr.edges.push(EdgeIn {
            curve,
            ends: None,
            operand,
        });
    }
    let operand = u32::try_from(arr.edges.len()).unwrap();
    for &[x, y] in vertices {
        arr.vertices.push(VertexIn {
            point: PointGeom::Grid(P3::new(x, y, 0).unwrap()),
            operand,
        });
    }
    arr
}

fn run(arr: &Arrangement) -> Result<Interfered, BuildError> {
    interfere(arr, default_remaining(), &Exec::serial(), &mut meter())
}

/// The refused pair and why.
fn refused(got: Result<Interfered, BuildError>) -> (Entity, Entity, Unresolved) {
    match got {
        Err(BuildError::Unresolved {
            first, second, why, ..
        }) => (first, second, why),
        other => panic!("{other:?}"),
    }
}

/// The two circles alone: the band touches no vertex and no block covers it, so it is refused.
/// With a vertex at (1105, 0), which lies on both circles to within what an enclosure tells, the
/// band touches a vertex both pass through and stands.
#[test]
fn a_band_stands_only_at_a_vertex_both_circles_pass_through() {
    let curves = || {
        vec![
            CurveKind::Circle(first([1.0, 0.0])),
            CurveKind::Circle(second()),
        ]
    };
    let (a, b, why) = refused(run(&arrangement(curves(), &[])));
    assert_eq!((a, b), (Entity::Edge(0), Entity::Edge(1)));
    assert!(matches!(why, Unresolved::TangentBand { .. }), "{why:?}");
    let out = run(&arrangement(curves(), &[[1105, 0]])).unwrap();
    let v = out.merged[0];
    for edge in [0, 1] {
        assert!(
            out.interferences
                .contains(&Interference::VertexEdge { vertex: v, edge }),
            "{:?}",
            out.interferences
        );
    }
}

/// (1104, 47) lies on the first circle and two steps off the second, (1106, 47) on the second and
/// two steps off the first: each is a vertex of one circle only, near the band but no vertex both
/// pass through, so the band is still refused.
#[test]
fn a_band_near_a_vertex_of_one_circle_only_is_refused() {
    let curves = vec![
        CurveKind::Circle(first([1.0, 0.0])),
        CurveKind::Circle(second()),
    ];
    let arr = arrangement(curves, &[[1104, 47], [1106, 47]]);
    let (a, b, _) = refused(run(&arr));
    assert_eq!((a, b), (Entity::Edge(0), Entity::Edge(1)));
}

/// The first circle's half from (0, −1105) through (1105, 0) to (0, 1105) as an arc with its own
/// ends, and a third operand's NURBS copy of that arc between the same two points: one block by the
/// contact rule, which covers the band on the arc and on the copy. The second circle, started so
/// that the band lies halfway round it, is in no block, and a band needs a block on both its edges,
/// so it is refused.
#[test]
fn a_band_needs_a_block_on_both_its_edges() {
    let frame = Frame::new([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let half = Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (0.0, 1.0)).unwrap();
    let copy = CurveKind::Circle(half.clone())
        .to_nurbs(&mut meter())
        .unwrap()
        .value
        .curve;
    let frame = Frame::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let round = Circle::new([2210.0, 0.0, 0.0], 1105.0_f64.next_up(), frame, (0.0, 2.0)).unwrap();
    let mut arr = Arrangement::default();
    for (operand, curve) in [
        (0, CurveKind::Circle(half)),
        (1, CurveKind::Circle(round)),
        (2, CurveKind::Nurbs(copy)),
    ] {
        let ends = (operand != 1).then(|| {
            [-1105, 1105].map(|y| {
                arr.vertices.push(VertexIn {
                    point: PointGeom::Grid(P3::new(0, y, 0).unwrap()),
                    operand,
                });
                arr.vertices.len() - 1
            })
        });
        let curve = arr.carriers.add_curve(curve).unwrap();
        arr.edges.push(EdgeIn {
            curve,
            ends,
            operand,
        });
    }
    let (a, b, _) = refused(run(&arr));
    assert!([a, b].contains(&Entity::Edge(1)), "{a:?} {b:?}");
    // Without the second circle the arc and its copy are one block and their region stands.
    arr.edges.remove(1);
    let out = run(&arr).unwrap();
    assert_eq!(out.common.len(), 1, "{:?}", out.common);
}
