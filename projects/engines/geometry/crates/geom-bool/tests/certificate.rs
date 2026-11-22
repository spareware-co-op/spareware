//! Each check of the arrangement certificate catches the fault it targets, a good result passes
//! them all, and a failed call changes nothing (G133, 11.10).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: a failure stops the test"
)]

use geom_bool::certificate::{Fault, certify, labelled};
use geom_bool::combine::subtract;
use geom_bool::operand::{Operand, OperandKey, OperandKind};
use geom_make::{BoxInput, make_box};
use geom_num::{Exactness, P3, Steps};
use geom_op::{OpContext, OpId, OpPath};
use geom_poly::merge::{Outward, select};
use geom_poly::subdivide::{LEAF_POLYGONS, around, arrange};
use geom_poly::trace::Polygon;
use geom_poly::winding::{Winding, WindingTest};
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::{Body, Builder, FaceIx, FinIx, Ix, Sense};

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn cuboid(low: [i64; 3], size: [u64; 3]) -> Body {
    let input = BoxInput {
        min: P3::new(low[0], low[1], low[2]).unwrap(),
        size: size.map(Steps),
    };
    make_box(&ctx(), &input).unwrap().result.body
}

fn solid(n: u8, body: &Body) -> Operand<'_> {
    Operand {
        key: OperandKey([n; 16]),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

/// The kept pieces of a unit block's boundary, and the test that kept them.
fn pieces() -> (Vec<Outward>, WindingTest) {
    let mut polygons = Vec::new();
    for (face, bounds) in geom_poly_block() {
        polygons.push(Polygon {
            piece: geom_poly::convex::Piece { face, bounds },
            entry: Some(0),
        });
    }
    let cell = around(&polygons).unwrap();
    let found = arrange(
        &Exec::serial(),
        cell,
        Winding::new(),
        &polygons,
        LEAF_POLYGONS,
        &[],
        &mut meter(),
    )
    .unwrap();
    let test = WindingTest::unite();
    (select(&test, &found).unwrap(), test)
}

fn geom_poly_block() -> Vec<(geom_num::plane::Plane, Vec<geom_num::plane::Plane>)> {
    use geom_num::plane::Plane;
    let plane = |axis: usize, sign: i128, d: i128| {
        let mut n = [0; 3];
        n[axis] = sign;
        Plane::new(n, d).unwrap()
    };
    let mut out = Vec::new();
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        out.push((
            plane(axis, 1, -4),
            vec![
                plane(v, 1, 0),
                plane(u, -1, 4),
                plane(v, -1, 4),
                plane(u, 1, 0),
            ],
        ));
        out.push((
            plane(axis, -1, 0),
            vec![
                plane(u, 1, 0),
                plane(v, -1, 4),
                plane(u, -1, 4),
                plane(v, 1, 0),
            ],
        ));
    }
    out
}

#[test]
fn a_good_result_shows_everything() {
    let body = cuboid([0, 0, 0], [4, 4, 4]);
    let (kept, test) = pieces();
    assert_eq!(certify(&body, &mut meter()), Ok(()));
    assert_eq!(labelled(&kept, &test, &mut meter()), Ok(()));
}

#[test]
fn fins_that_run_the_same_way_are_caught() {
    let body = cuboid([0, 0, 0], [4, 4, 4]);
    let mut builder = Builder::edit(&body);
    let fin: FinIx = Ix::new(0).unwrap();
    let sense = match *builder.arenas.fins.sense.get(fin).unwrap() {
        Sense::Forward => Sense::Reversed,
        Sense::Reversed => Sense::Forward,
    };
    builder.arenas.fins.sense.set(fin, sense).unwrap();
    let bad = builder
        .publish(body.kind(), body.outside(), &mut meter())
        .unwrap();
    assert!(matches!(
        certify(&bad, &mut meter()),
        Err(Fault::Fins { .. })
    ));
}

#[test]
fn an_edge_with_one_fin_is_caught() {
    let body = cuboid([0, 0, 0], [4, 4, 4]);
    let mut builder = Builder::edit(&body);
    let fin: FinIx = Ix::new(0).unwrap();
    builder.arenas.fins.radial.set(fin, fin).unwrap();
    let bad = builder
        .publish(body.kind(), body.outside(), &mut meter())
        .unwrap();
    assert_eq!(certify(&bad, &mut meter()), Err(Fault::Fins { edge: 0 }));
}

#[test]
fn an_edge_that_ends_where_it_starts_is_caught() {
    let body = cuboid([0, 0, 0], [4, 4, 4]);
    let mut builder = Builder::edit(&body);
    let edge = Ix::new(3).unwrap();
    let start = *builder.arenas.edges.start.get(edge).unwrap();
    builder.arenas.edges.end.set(edge, start).unwrap();
    let bad = builder
        .publish(body.kind(), body.outside(), &mut meter())
        .unwrap();
    assert_eq!(
        certify(&bad, &mut meter()),
        Err(Fault::ZeroLength { edge: 3 })
    );
}

#[test]
fn a_face_without_a_loop_is_caught() {
    let body = cuboid([0, 0, 0], [4, 4, 4]);
    let mut builder = Builder::edit(&body);
    let face: FaceIx = Ix::new(2).unwrap();
    builder.arenas.faces.first_loop.set(face, None).unwrap();
    let bad = builder
        .publish(body.kind(), body.outside(), &mut meter())
        .unwrap();
    assert_eq!(
        certify(&bad, &mut meter()),
        Err(Fault::ZeroArea { face: 2 })
    );
}

#[test]
fn a_loop_of_two_fins_is_caught() {
    let body = cuboid([0, 0, 0], [4, 4, 4]);
    let mut builder = Builder::edit(&body);
    let first: FinIx = Ix::new(0).unwrap();
    let next = *builder.arenas.fins.next.get(first).unwrap();
    builder.arenas.fins.next.set(next, first).unwrap();
    let bad = builder
        .publish(body.kind(), body.outside(), &mut meter())
        .unwrap();
    let got = certify(&bad, &mut meter());
    assert!(matches!(got, Err(Fault::ZeroArea { .. })), "{got:?}");
}

#[test]
fn a_piece_that_separates_nothing_is_caught() {
    let (mut kept, test) = pieces();
    kept[1].back = kept[1].front.clone();
    assert_eq!(
        labelled(&kept, &test, &mut meter()),
        Err(Fault::Label { piece: 1 })
    );
    let (mut kept, test) = pieces();
    kept[2].front = kept[2].back.clone();
    assert_eq!(
        labelled(&kept, &test, &mut meter()),
        Err(Fault::Label { piece: 2 })
    );
}

#[test]
fn a_stopped_meter_is_a_stop() {
    let body = cuboid([0, 0, 0], [4, 4, 4]);
    let (kept, test) = pieces();
    for budget in [0, 12] {
        let mut small = WorkMeter::new(Budget::new(budget, u64::MAX));
        assert!(matches!(certify(&body, &mut small), Err(Fault::Stopped(_))));
    }
    let mut small = WorkMeter::new(Budget::new(0, u64::MAX));
    assert!(matches!(
        labelled(&kept, &test, &mut small),
        Err(Fault::Stopped(_))
    ));
}

#[test]
fn faults_say_what_broke() {
    let stopped = WorkMeter::new(Budget::new(0, u64::MAX))
        .charge(1)
        .unwrap_err();
    assert_eq!(
        Fault::Fins { edge: 4 }.to_string(),
        "edge 4: fins that do not pair up"
    );
    assert_eq!(
        Fault::ZeroLength { edge: 4 }.to_string(),
        "edge 4 has no length"
    );
    assert_eq!(
        Fault::ZeroArea { face: 4 }.to_string(),
        "face 4 has no area"
    );
    assert_eq!(
        Fault::Label { piece: 4 }.to_string(),
        "piece 4 separates nothing"
    );
    assert_eq!(
        Fault::from(stopped).to_string(),
        "the meter stopped the certificate"
    );
}

#[test]
fn a_failed_call_leaves_its_operands_as_they_were() {
    let block = cuboid([0, 0, 0], [40, 40, 40]);
    let tool = cuboid([20, 20, 20], [40, 40, 40]);
    let (before_block, before_tool) = (block.clone(), tool.clone());
    let small = OpContext::new(
        OpPath::new(OpId::from_bytes([7; 16])),
        Budget::new(40, u64::MAX),
    );
    let operands = [solid(1, &block), solid(2, &tool)];
    assert!(subtract(&small, &Exec::serial(), OperandKey([1; 16]), &operands).is_err());
    assert_eq!(block, before_block);
    assert_eq!(tool, before_tool);
}
