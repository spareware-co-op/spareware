//! The curved builder's vertices and distances (G242, G243): how far a grid point lies from a
//! plane in whole steps, the floor every projection is refined to, the representative of a
//! merged cluster, which vertices are merged and recorded as merged, which vertex-edge meetings
//! are asked, and the bound a new vertex is held to.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::cast_precision_loss,
    reason = "test code: a failure should stop loudly"
)]

use geom_bool::carrier::{Bound, CarrierIx, PointGeom, SortedVec};
use geom_bool::distance::Carriers;
use geom_bool::interfere::{
    Arrangement, BuildError, EdgeIn, Interfered, Interference, VertexIn, interfere,
};
use geom_bool::merge::{default_remaining, merge_cluster};
use geom_bool::rule::ContactError;
use geom_curve::{Circle, CurveKind, Frame, Line, ParamCurve};
use geom_num::plane::Plane;
use geom_num::{P3, Steps};
use geom_run::{Budget, Exec, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 30))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// The least whole number of steps a grid point is from a plane, exactly: on a plane along the
/// axes, on one whose normal has a whole length, on one whose distance is irrational, and at the
/// far edge of the grid.
#[test]
fn a_grid_point_s_bound_from_a_plane_is_its_distance_rounded_up() {
    let cases = [
        ([0, 0, 1], 0, p(0, 0, 1000), 1000),
        // (3, 4, 0) · p = 7000, |n| = 5.
        ([3, 4, 0], 0, p(1000, 1000, 0), 1400),
        // 1000 / √2 = 707.1: 708 steps is the least whole number past it.
        ([1, 1, 0], 0, p(1000, 0, 0), 708),
        ([0, 0, 1], -5, p(0, 0, 0), 5),
        ([0, 0, 1], 0, p(0, 0, (1 << 40) - 1), (1 << 40) - 1),
    ];
    for (n, d, at, steps) in cases {
        let mut carriers = Carriers::default();
        let ix = carriers.add_plane(&Plane::new(n, d).unwrap()).unwrap();
        let (bound, _) = carriers
            .upper_bound(at, CarrierIx::Plane(ix), 8, &mut meter())
            .unwrap();
        assert_eq!(bound, Bound(Steps(steps)), "{n:?} {at:?}");
    }
}

/// A circle of half a step's diameter through two grid points, and the grid point a step above
/// the first: at the finest refinement its distance from the circle is enclosed to well within a
/// hundredth of a step, though the whole circle is smaller than a step. A projection with a floor
/// a step or more wide would take the whole circle as one undecided span and enclose the distance
/// only to the circle's width.
#[test]
fn a_distance_is_refined_below_the_size_of_a_small_circle() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let circle = Circle::new([0.25, 0.0, 0.0], 0.25, frame, (0.0, 2.0)).unwrap();
    let mut carriers = Carriers::default();
    let ix = carriers.add_curve(CurveKind::Circle(circle)).unwrap();
    let truth = (0.25_f64 * 0.25 + 1.0).sqrt() - 0.25;
    let d = carriers
        .distance(p(0, 1, 0), CarrierIx::Curve(ix), 8, &mut meter())
        .unwrap();
    assert!(d.lo() <= truth && truth <= d.hi(), "{d:?}");
    assert!(d.hi() - d.lo() < 0.01, "{d:?}");
}

/// The circle's centre seen against the circle stored as a NURBS: every point of it is nearest, a
/// continuum the search finds only by halving the curve down to its floor, where it joins the
/// spans it cannot decide and says they are no one point. The distance is then the widest
/// enclosure, found within a small budget.
#[test]
fn a_continuum_of_nearest_points_is_found_at_the_floor() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let circle = Circle::new([0.0, 0.0, 0.0], 10.0, frame, (0.0, 2.0)).unwrap();
    let nurbs = CurveKind::Circle(circle)
        .to_nurbs(&mut meter())
        .unwrap()
        .value
        .curve;
    let mut carriers = Carriers::default();
    let ix = carriers.add_curve(CurveKind::Nurbs(nurbs)).unwrap();
    let mut small = WorkMeter::new(Budget::of_work(1 << 22));
    let d = carriers
        .distance(p(0, 0, 0), CarrierIx::Curve(ix), 8, &mut small)
        .unwrap();
    assert!(d.lo() <= 10.0 && 10.0 <= d.hi(), "{d:?}");
}

/// A cluster of an exact grid point and a point certified on a line: its representative is the
/// exact point, though the certified one lies nearer the line, and it is certified on the line
/// within its own distance from it rounded up.
#[test]
fn a_cluster_s_representative_is_its_exact_point() {
    let mut carriers = Carriers::default();
    let line = Line::new([0.0, 0.0, 0.0], [1000.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
    let on = CarrierIx::Curve(carriers.add_curve(CurveKind::Line(line)).unwrap());
    let members = [
        PointGeom::Grid(p(0, 1, 0)),
        PointGeom::Certified {
            at: p(0, 0, 0),
            on: SortedVec::from_items([on]),
            within: Bound(Steps(0)),
        },
    ];
    let merged = merge_cluster(&carriers, &members, default_remaining(), &mut meter()).unwrap();
    let PointGeom::Certified {
        at,
        on: carried,
        within,
    } = &merged
    else {
        panic!("{merged:?}");
    };
    assert_eq!((*at, carried), (p(0, 1, 0), &SortedVec::from_items([on])));
    // A step from the line, its enclosure rounded up to whole steps.
    assert!(
        Bound(Steps(1)) <= *within && *within <= Bound(Steps(2)),
        "{within:?}"
    );
}

/// An arrangement of grid vertices and straight edges, each edge between two of the vertices.
fn arrangement(vertices: &[(P3, u32)], edges: &[(usize, usize)]) -> Arrangement {
    let mut arr = Arrangement::default();
    for &(at, operand) in vertices {
        arr.vertices.push(VertexIn {
            point: PointGeom::Grid(at),
            operand,
        });
    }
    let coords = |q: P3| [q.x.get() as f64, q.y.get() as f64, q.z.get() as f64];
    for &(a, b) in edges {
        let (o, e) = (coords(vertices[a].0), coords(vertices[b].0));
        let line = Line::new(o, [e[0] - o[0], e[1] - o[1], e[2] - o[2]], (0.0, 1.0)).unwrap();
        let curve = arr.carriers.add_curve(CurveKind::Line(line)).unwrap();
        arr.edges.push(EdgeIn {
            curve,
            ends: Some([a, b]),
            operand: vertices[a].1,
        });
    }
    arr
}

fn run(arr: &Arrangement, remaining: Bound) -> Result<Interfered, BuildError> {
    interfere(arr, remaining, &Exec::serial(), &mut meter())
}

/// Two operands each with a corner at the origin: those two inputs, and only those, are merged,
/// and the merge is recorded for each of them. An operand's own two vertices at one point stay
/// two: only vertices of different operands are merged.
#[test]
fn only_vertices_of_different_operands_are_merged_and_each_merge_is_recorded() {
    let arr = arrangement(
        &[
            (p(0, 0, 0), 0),
            (p(100, 0, 0), 0),
            (p(0, 0, 0), 1),
            (p(0, 100, 0), 1),
        ],
        &[],
    );
    let out = run(&arr, default_remaining()).unwrap();
    assert_eq!(out.vertices.len(), 3);
    assert_eq!(out.merged[0], out.merged[2]);
    let v = out.merged[0];
    assert_eq!(
        out.interferences,
        [
            Interference::VertexVertex {
                input: 0,
                vertex: v
            },
            Interference::VertexVertex {
                input: 2,
                vertex: v
            },
        ]
    );
    let arr = arrangement(&[(p(0, 0, 0), 0), (p(0, 0, 0), 0)], &[]);
    let out = run(&arr, default_remaining()).unwrap();
    assert_eq!(out.vertices.len(), 2);
    assert_eq!(out.interferences, []);
}

/// Two operands' edges leaving their merged corner at the origin: the corner is each edge's own
/// end, so it is no vertex-edge meeting of either, though it lies on both.
#[test]
fn an_edge_s_own_end_is_no_meeting_of_it() {
    let arr = arrangement(
        &[
            (p(0, 0, 0), 0),
            (p(100, 0, 0), 0),
            (p(0, 0, 0), 1),
            (p(0, 100, 0), 1),
        ],
        &[(0, 1), (2, 3)],
    );
    let out = run(&arr, default_remaining()).unwrap();
    assert!(
        out.interferences
            .iter()
            .all(|i| !matches!(i, Interference::VertexEdge { .. })),
        "{:?}",
        out.interferences
    );
    assert!(out.paves.iter().all(|list| list.len() == 2));
}

/// Two edges crossing at (1.5, 0.5): the new vertex is the grid point (2, 0), 0.63 steps from one
/// line and 0.32 from the other, so it is certified within a step of both. Held to a remaining
/// bound of one step it stands; held to none it is refused, naming a carrier it is too far from.
#[test]
fn a_new_vertex_is_held_to_the_remaining_bound() {
    let arr = arrangement(
        &[
            (p(0, 0, 0), 0),
            (p(3, 1, 0), 0),
            (p(0, 1, 0), 1),
            (p(3, 0, 0), 1),
        ],
        &[(0, 1), (2, 3)],
    );
    let out = run(&arr, Bound(Steps(1))).unwrap();
    assert_eq!(out.vertices.len(), 5);
    let PointGeom::Certified { at, on, within } = &out.vertices[4] else {
        panic!("{:?}", out.vertices[4]);
    };
    assert_eq!((*at, on.len(), *within), (p(2, 0, 0), 2, Bound(Steps(1))));
    match run(&arr, Bound(Steps(0))) {
        Err(BuildError::Contact(ContactError::CoincidenceChain(chain))) => {
            assert_eq!(chain.remaining, Bound(Steps(0)));
            assert!(chain.distance.hi() > 0.0, "{chain:?}");
        }
        other => panic!("{other:?}"),
    }
}
