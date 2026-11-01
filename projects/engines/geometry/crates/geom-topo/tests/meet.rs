//! Vertices where three planes meet (G134): the plane pool, the exact sums' refusal, and the
//! certified measures that take such a vertex.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::unwrap_used,
    reason = "test fixtures; the rules are for kernel code"
)]

use geom_curve::{CurveKind, Line};
use geom_math::Box3;
use geom_num::grid::P3;
use geom_num::plane::{Lpi, Plane, Tpi};
use geom_num::{Exactness, Sign};
use geom_op::{OpContext, OpErrorDetail, QueryDetail};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane as Flat, SurfaceKind};
use geom_topo::mass::{self, Corner};
use geom_topo::query::{
    self, QueryError, certified_loop_area, certified_shell_volume, corner, face_area, loop_area,
    point, shell_side, shell_volume, volume,
};
use geom_topo::query_ops;
use geom_topo::{
    Arena, Body, BodyKind, Builder, Edge, Face, Fin, Ix, Loop, PointKind, Region, RegionKind,
    Sense, Shell, TopoError, Vertex,
};

/// A meter with no cap and no cancellation.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The index of row `row`.
fn ix<K>(row: u32) -> Ix<K> {
    Ix::new(row).unwrap()
}

/// The grid point (x, y, z).
fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// The plane where coordinate `k` is `at`, its normal along that axis.
fn axis(k: usize, at: i128) -> Plane {
    let mut n = [0; 3];
    n[k] = 1;
    Plane::new(n, -at).unwrap()
}

/// The corners of a box, by bits, x first.
const FACES: [[u32; 4]; 6] = [
    [0, 2, 3, 1],
    [4, 5, 7, 6],
    [0, 1, 5, 4],
    [2, 6, 7, 3],
    [0, 4, 6, 2],
    [1, 3, 7, 5],
];

/// A box from `o` of side `s`, one shell, every corner the meeting of the plane pool's planes.
fn meeting_box(o: i64, s: i64) -> Builder {
    let mut b = Builder::new();
    let a = &mut b.arenas;
    a.regions
        .extend(&[
            Region {
                kind: RegionKind::Solid,
                first_shell: Some(ix(0)),
            },
            Region {
                kind: RegionKind::Void,
                first_shell: None,
            },
        ])
        .unwrap();
    a.shells
        .extend(&[Shell {
            region: ix(0),
            next: None,
            first_face: Some(ix(0)),
        }])
        .unwrap();
    for k in 0..3 {
        for at in [o, o + s] {
            a.planes.push(axis(k, i128::from(at))).unwrap();
        }
    }
    let at = |bits: u32, k: u32| o + s * i64::from(bits >> k & 1);
    for bits in 0..8 {
        let place = [0, 1, 2].map(|k| ix(k * 2 + (bits >> k & 1)));
        a.points.push(PointKind::Meet(place)).unwrap();
        a.vertices.push(Vertex { point: ix(bits) }).unwrap();
    }
    let flat = Flat::new(
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        ((0.0, 1.0), (0.0, 1.0)),
    )
    .unwrap();
    a.surfaces.push(SurfaceKind::Plane(flat)).unwrap();
    let bound = Box3::new(p(o, o, o), p(o + s, o + s, o + s)).unwrap();
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    for (f, ring) in FACES.iter().enumerate() {
        let f = f as u32;
        a.faces
            .push(Face {
                shell: ix(0),
                next: (f < 5).then(|| ix(f + 1)),
                surface: ix(0),
                sense: Sense::Forward,
                first_loop: Some(ix(f)),
                bound,
            })
            .unwrap();
        a.loops
            .push(Loop {
                face: ix(f),
                next: None,
                first_fin: Some(ix(f * 4)),
            })
            .unwrap();
        for k in 0..4_u32 {
            let (s, t) = (ring[k as usize], ring[((k + 1) % 4) as usize]);
            let key = (s.min(t), s.max(t));
            let e = pairs.iter().position(|&q| q == key).map_or_else(
                || {
                    pairs.push(key);
                    pairs.len() as u32 - 1
                },
                |e| e as u32,
            );
            a.fins
                .push(Fin {
                    loop_: ix(f),
                    next: ix(f * 4 + (k + 1) % 4),
                    prev: ix(f * 4 + (k + 3) % 4),
                    edge: ix(e),
                    sense: if s < t {
                        Sense::Forward
                    } else {
                        Sense::Reversed
                    },
                    radial: ix(f * 4 + k),
                    pcurve: None,
                })
                .unwrap();
        }
    }
    for (e, &(s, t)) in pairs.iter().enumerate() {
        let c = |v: u32| [at(v, 0), at(v, 1), at(v, 2)].map(|x| x as f64);
        let (from, to) = (c(s), c(t));
        let d = [0, 1, 2].map(|k| to[k] - from[k]);
        a.curves
            .push(CurveKind::Line(Line::new(from, d, (0.0, 1.0)).unwrap()))
            .unwrap();
        a.edges
            .push(Edge {
                first_fin: None,
                start: Some(ix(s)),
                end: Some(ix(t)),
                curve: ix(e as u32),
            })
            .unwrap();
    }
    b
}

/// [`meeting_box`], published.
fn body(o: i64, s: i64) -> Body {
    meeting_box(o, s)
        .publish(BodyKind::Solid, ix(1), &mut meter())
        .unwrap()
}

#[test]
fn the_pool_is_an_arena_of_its_own() {
    let b = meeting_box(0, 10);
    assert_eq!(b.arenas.rows(Arena::Plane), 6);
    assert_eq!(Arena::Plane.name(), "plane");
    assert_eq!(Arena::ALL.last(), Some(&Arena::Plane));
    assert!(b.arenas.memory().bytes > Builder::new().arenas.memory().bytes);
}

#[test]
fn a_point_says_which_kind_it_is() {
    let at = PointKind::from(p(1, 2, 3));
    assert_eq!(at, PointKind::Grid(p(1, 2, 3)));
    assert_eq!(at.grid(), Some(p(1, 2, 3)));
    assert_eq!(PointKind::Meet([ix(0), ix(1), ix(2)]).grid(), None);
}

#[test]
fn publishing_refuses_a_meeting_of_planes_the_pool_does_not_have() {
    let mut b = meeting_box(0, 10);
    b.arenas
        .points
        .set(ix(3), PointKind::Meet([ix(0), ix(2), ix(9)]))
        .unwrap();
    assert_eq!(
        b.publish(BodyKind::Solid, ix(1), &mut meter()).unwrap_err(),
        TopoError::NoSuchRow {
            arena: Arena::Plane,
            row: 9
        }
    );
}

#[test]
fn publishing_is_charged_for_the_meetings_and_a_stop_is_a_stop() {
    let mut used = None;
    for work in 0..64 {
        let mut m = WorkMeter::new(Budget::of_work(work));
        if meeting_box(0, 10)
            .publish(BodyKind::Solid, ix(1), &mut m)
            .is_ok()
        {
            used = Some(work);
            break;
        }
    }
    assert!(used.is_some());
}

#[test]
fn the_exact_sums_refuse_a_vertex_with_no_grid_point() {
    let w = body(0, 10);
    let v: Ix<Vertex> = ix(2);
    assert_eq!(point(&w, v), Err(QueryError::Implicit(v)));
    let m = &mut meter();
    let implicit = QueryError::Implicit(ix(0));
    assert_eq!(loop_area(&w, ix(0), m), Err(implicit));
    assert_eq!(face_area(&w, ix(0), m), Err(implicit));
    assert_eq!(shell_volume(&w, ix(0), m), Err(implicit));
    assert_eq!(volume(&w, m), Err(implicit));
    assert_eq!(shell_side(&w, ix(0), ix(0), m), Err(implicit));
    assert_eq!(implicit.to_string(), "vertex 0 has no grid point");
}

#[test]
fn a_corner_is_the_exact_place() {
    let w = body(0, 10);
    let Corner::Tpi(t) = corner(&w, ix(7)).unwrap() else {
        panic!("a meeting of planes");
    };
    assert_eq!(t.to_grid().unwrap().value(), p(10, 10, 10));
    assert_eq!(
        corner(&w, ix(99)).map(|_| ()),
        Err(QueryError::NoSuchRow),
        "a row past the arena"
    );
}

#[test]
fn a_meeting_of_parallel_planes_has_no_corner() {
    let mut b = meeting_box(0, 10);
    b.arenas
        .points
        .set(ix(3), PointKind::Meet([ix(0), ix(1), ix(2)]))
        .unwrap();
    let w = b.publish(BodyKind::Solid, ix(1), &mut meter()).unwrap();
    assert_eq!(corner(&w, ix(3)).map(|_| ()), Err(QueryError::Overflow));
}

#[test]
fn the_certified_measures_hold_the_true_values() {
    let w = body(-5, 10);
    let m = &mut meter();
    let [x, y, z] = certified_loop_area(&w, ix(0), m).unwrap();
    assert!(x.contains(0.0) && y.contains(0.0));
    assert!(z.contains(-200.0), "{z:?}");
    let six = certified_shell_volume(&w, ix(0), m).unwrap();
    assert!(six.contains(6_000.0), "{six:?}");
    assert_eq!(six.certain_sign(), Some(Sign::Positive));
}

#[test]
fn a_small_shell_far_from_the_origin_keeps_its_sign() {
    let far = 1_099_511_627_000;
    let w = body(far, 7);
    let six = certified_shell_volume(&w, ix(0), &mut meter()).unwrap();
    assert!(six.contains(6.0 * 343.0), "{six:?}");
    assert_eq!(six.certain_sign(), Some(Sign::Positive));
    assert!(six.hi() - six.lo() < 1.0, "{six:?}");
}

#[test]
fn a_curved_face_has_no_certified_volume() {
    let mut b = meeting_box(0, 10);
    let sphere = geom_surface::Sphere::new(
        [0.0; 3],
        1.0,
        geom_curve::Frame::standard().unwrap(),
        ((-1.0, 1.0), (-0.5, 0.5)),
    )
    .unwrap();
    b.arenas
        .surfaces
        .set(ix(0), SurfaceKind::Sphere(sphere))
        .unwrap();
    let w = b.publish(BodyKind::Solid, ix(1), &mut meter()).unwrap();
    assert_eq!(
        certified_shell_volume(&w, ix(0), &mut meter()),
        Err(QueryError::NotPlanar(ix(0)))
    );
}

#[test]
fn a_certified_measure_stops_when_its_meter_does() {
    let w = body(0, 10);
    for work in [0, 1, 3, 9] {
        let mut m = WorkMeter::new(Budget::of_work(work));
        assert!(matches!(
            certified_shell_volume(&w, ix(0), &mut m),
            Err(QueryError::Stopped(_))
        ));
    }
}

#[test]
fn the_bounding_box_takes_the_stored_boxes_and_says_it_is_not_tight() {
    let w = body(-5, 10);
    let (found, tight) = query::bounding_box(&w, &mut meter()).unwrap().unwrap();
    assert_eq!(found, Box3::new(p(-5, -5, -5), p(5, 5, 5)).unwrap());
    assert!(!tight);
}

#[test]
fn an_operation_reports_the_vertex() {
    let w = body(0, 10);
    let id = geom_op::OpId::from_bytes([5; 16]);
    let ctx = OpContext::new(geom_op::OpPath::new(id), Budget::UNLIMITED);
    let e = query_ops::six_volume(&ctx, &w, Exactness::Exact).unwrap_err();
    assert_eq!(
        *e.detail(),
        OpErrorDetail::Query(QueryDetail::ImplicitVertex { vertex: 0 })
    );
}

#[test]
fn a_corner_of_any_kind_has_an_exact_side() {
    let x = axis(0, 5);
    assert_eq!(Corner::Grid(p(6, 0, 0)).side(&x), Ok(Sign::Positive));
    assert_eq!(Corner::Grid(p(5, 0, 0)).side(&x), Ok(Sign::Zero));
    let t = Tpi::new(axis(0, 4), axis(1, 0), axis(2, 0)).unwrap();
    assert_eq!(Corner::Tpi(t).side(&x), Ok(Sign::Negative));
    let l = Lpi::new(p(0, 0, 0), p(10, 0, 0), axis(0, 7)).unwrap();
    assert_eq!(Corner::Lpi(l).side(&x), Ok(Sign::Positive));
}

#[test]
fn a_loop_is_measured_from_its_first_corner() {
    assert_eq!(mass::about(&[]), p(0, 0, 0));
    assert_eq!(mass::about(&[Corner::Grid(p(1, 2, 3))]), p(1, 2, 3));
    let t = Tpi::new(axis(0, 4), axis(1, 2), axis(2, 9)).unwrap();
    assert_eq!(mass::about(&[Corner::Tpi(t)]), p(4, 2, 9));
    let l = Lpi::new(p(0, 0, 0), p(10, 0, 0), axis(0, 7)).unwrap();
    assert_eq!(mass::about(&[Corner::Lpi(l)]), p(7, 0, 0));
}

#[test]
fn a_loop_with_no_corners_adds_no_volume() {
    let zero = geom_num::interval::Interval::point(0.0).unwrap();
    let none = mass::add_loop_volume(zero, &[], p(0, 0, 0), &mut meter()).unwrap();
    assert_eq!(none, zero);
}

#[test]
fn the_volume_does_not_depend_on_where_it_is_measured_from() {
    let corners: Vec<Corner> = [p(0, 0, 0), p(3, 0, 0), p(0, 4, 0), p(0, 0, 5)]
        .into_iter()
        .map(Corner::Grid)
        .collect();
    let fan =
        |a: usize, b: usize, c: usize| [corners[a].clone(), corners[b].clone(), corners[c].clone()];
    let faces = [fan(0, 2, 1), fan(0, 1, 3), fan(0, 3, 2), fan(1, 2, 3)];
    for origin in [p(0, 0, 0), p(100, -7, 3), p(1_000_000, 1, 1)] {
        let mut sum = geom_num::interval::Interval::point(0.0).unwrap();
        for face in &faces {
            sum = mass::add_loop_volume(sum, face, origin, &mut meter()).unwrap();
        }
        assert!(sum.contains(60.0), "{sum:?}");
    }
}
