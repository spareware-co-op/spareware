//! The core queries as operations (22; piece G102): the contract's answer with the body's class,
//! each query failure as a typed error with its base kind, and a stop as the context's error.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{WALL, ix, many_faces, meter, wall, wall_builder, wall_outside};
use geom_curve::{Circle, CurveKind, Frame};
use geom_num::wide::I256;
use geom_num::{Exactness, GridStep, Steps};
use geom_op::detail::QueryDetail;
use geom_op::{ErrorKind, OpContext, OpErrorDetail, OpId, OpPath, Remedy};
use geom_run::Budget;
use geom_surface::{Sphere, SurfaceKind};
use geom_topo::query::QueryError;
use geom_topo::query_ops::{bounding_box, failed, six_volume, twice_area};
use geom_topo::{Body, BodyKind, Builder};

fn ctx(budget: Budget) -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([5; 16])), budget)
}

fn open() -> OpContext<'static> {
    ctx(Budget::UNLIMITED)
}

fn certified() -> Exactness {
    Exactness::Certified {
        bound: Steps(2),
        step: GridStep::Q0,
    }
}

fn rebuilt(edit: impl FnOnce(&mut Builder)) -> Body {
    let mut b = wall_builder(true);
    edit(&mut b);
    b.publish(BodyKind::Solid, wall_outside(), &mut meter())
        .unwrap()
}

fn detail(e: &geom_op::OpError) -> QueryDetail {
    match e.detail() {
        OpErrorDetail::Query(d) => *d,
        other => panic!("{other:?}"),
    }
}

#[test]
fn each_query_answers_with_the_class_it_was_given() {
    let w = wall();
    let [x, y, z] = WALL.map(i128::from);
    let out = six_volume(&open(), &w, Exactness::Exact).unwrap();
    assert_eq!(out.exactness, Exactness::Exact);
    assert_eq!(out.result, I256::from_i128(6 * x * y * z));
    let out = six_volume(&open(), &w, certified()).unwrap();
    assert_eq!(out.exactness, certified());
    let out = twice_area(&open(), &w, ix(0), certified()).unwrap();
    assert_eq!(out.exactness, certified());
    assert_eq!(out.result[0].abs(), 2 * y * z);
    let out = bounding_box(&open(), &w, certified()).unwrap();
    assert_eq!(out.exactness, certified());
    let b = out.result.unwrap();
    assert!(b.tight);
    assert_eq!(b.bound.max().x.get(), WALL[0]);
}

#[test]
fn a_body_with_nothing_in_it_has_no_box() {
    let mut e = Builder::new();
    e.arenas
        .regions
        .push(geom_topo::Region {
            kind: geom_topo::RegionKind::Void,
            first_shell: None,
        })
        .unwrap();
    let none = e.publish(BodyKind::General, ix(0), &mut meter()).unwrap();
    let out = bounding_box(&open(), &none, Exactness::Exact).unwrap();
    assert_eq!(out.result, None);
}

#[test]
fn each_failure_has_its_detail_kind_and_remedy() {
    let curved = rebuilt(|b| {
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let c = Circle::new([0.0; 3], 5.0, frame, (0.0, 1.0)).unwrap();
        b.arenas.curves.set(ix(0), CurveKind::Circle(c)).unwrap();
    });
    let e = six_volume(&open(), &curved, Exactness::Exact).unwrap_err();
    assert!(matches!(detail(&e), QueryDetail::CurvedEdge { edge: 0 }));
    assert_eq!(e.kind(), ErrorKind::Unsupported);
    assert_eq!(
        e.detail().remedies(),
        [Remedy::AwaitPiece { piece: "G177" }]
    );
    let round = rebuilt(|b| {
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let s = Sphere::new([0.0; 3], 5.0, frame, ((0.0, 2.0), (-0.5, 0.5))).unwrap();
        b.arenas
            .surfaces
            .set(ix(0), SurfaceKind::Sphere(s))
            .unwrap();
    });
    let face = (0..round.arenas().faces.len())
        .find(|&f| round.arenas().faces.get(ix(f)).unwrap().surface.row() == 0)
        .unwrap();
    let e = twice_area(&open(), &round, ix(face), Exactness::Exact).unwrap_err();
    assert_eq!(detail(&e), QueryDetail::CurvedFace { face });
    assert_eq!(e.kind(), ErrorKind::Unsupported);
    let sheet = many_faces(2);
    let e = twice_area(&open(), &sheet, ix(1), Exactness::Exact).unwrap_err();
    assert_eq!(detail(&e), QueryDetail::NoLoops { face: 1 });
    let e = six_volume(&open(), &sheet, Exactness::Exact).unwrap_err();
    assert_eq!(detail(&e), QueryDetail::NotSolid);
    assert_eq!(e.kind(), ErrorKind::InvalidInput);
    let e = twice_area(&open(), &sheet, ix(9), Exactness::Exact).unwrap_err();
    assert_eq!(detail(&e), QueryDetail::Malformed);
    assert_eq!(e.kind(), ErrorKind::InvalidInput);
    let ring = rebuilt(|b| {
        b.arenas.faces.next.set(ix(1), Some(ix(0))).unwrap();
    });
    let e = six_volume(&open(), &ring, Exactness::Exact).unwrap_err();
    assert_eq!(detail(&e), QueryDetail::Malformed);
    let m = meter();
    let e = failed(&open(), &m, QueryError::Overflow);
    assert_eq!(detail(&e), QueryDetail::Overflow);
    assert_eq!(e.kind(), ErrorKind::Internal);
    assert_eq!(e.detail().remedies(), [Remedy::Report]);
    for d in [
        QueryDetail::Malformed,
        QueryDetail::CurvedEdge { edge: 3 },
        QueryDetail::CurvedFace { face: 4 },
        QueryDetail::NoLoops { face: 5 },
        QueryDetail::NotSolid,
        QueryDetail::Overflow,
    ] {
        assert_ne!(d.to_string(), "");
        assert_ne!(OpErrorDetail::from(d).to_string(), "");
    }
}

#[test]
fn a_stop_is_the_contexts_error_and_gives_no_answer() {
    let w = wall();
    let tight = ctx(Budget::of_work(3));
    let budget = |e: geom_op::OpError| assert_eq!(e.kind(), ErrorKind::BudgetExceeded);
    budget(six_volume(&tight, &w, Exactness::Exact).unwrap_err());
    budget(twice_area(&tight, &w, ix(0), Exactness::Exact).unwrap_err());
    budget(bounding_box(&tight, &w, Exactness::Exact).unwrap_err());
    // Exactly enough goes through.
    assert!(six_volume(&ctx(Budget::of_work(36)), &w, Exactness::Exact).is_ok());
    assert!(six_volume(&ctx(Budget::of_work(35)), &w, Exactness::Exact).is_err());
}
