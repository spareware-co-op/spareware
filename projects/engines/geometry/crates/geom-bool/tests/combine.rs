//! Bodies in, bodies out: the boolean's entry against closed forms and the checker (G130, G131).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic, and a failure stops the test"
)]

use geom_bool::combine::{COMBINE, combine, intersect, subtract, unite};
use geom_bool::half_space::half_space;
use geom_bool::operand::{
    BooleanInput, Operand, OperandKey, OperandKind, PrepareError, Selection, Term,
};
use geom_bool::rebuild::{Carrier, RebuildError, rebuild_with};
use geom_check::{Level, check};
use geom_make::{BoxInput, Made, cylinder, make_box, prism};
use geom_num::plane::Plane;
use geom_num::{Exactness, NumError, P3, Steps};
use geom_op::{CombineDetail, ErrorKind, OpContext, OpErrorDetail, OpId, OpPath};
use geom_poly::convex::Piece;
use geom_poly::merge::{Merge, Outward, Rules};
use geom_poly::winding::Winding;
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::query::volume;
use geom_topo::{Body, BodyKind, Builder, FlatSurface, Sense, SurfaceKind};
use proptest::prelude::*;

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn cuboid(low: [i64; 3], size: [u64; 3]) -> Body {
    let input = BoxInput {
        min: p(low[0], low[1], low[2]),
        size: size.map(Steps),
    };
    let made: Made = make_box(&ctx(), &input).unwrap().result;
    made.body
}

fn key(n: u8) -> OperandKey {
    OperandKey([n; 16])
}

fn solid(n: u8, body: &Body) -> Operand<'_> {
    Operand {
        key: key(n),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

/// Six times `body`'s volume.
fn sixfold(body: &Body) -> i128 {
    volume(body, &mut meter()).unwrap().to_i128().unwrap()
}

fn checked(body: &Body) {
    let report = check(body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(report.passed(), "{report:?}");
}

const SERIAL: Exec<'static> = Exec::serial();

#[test]
fn the_schema_says_what_the_entry_does() {
    assert_eq!(COMBINE.name, "combine.boolean");
    assert_eq!(COMBINE.inputs.len(), 3);
    assert_eq!(COMBINE.outputs.len(), 1);
    assert!(
        COMBINE
            .errors
            .iter()
            .any(|e| e.detail == "DuplicateOperand")
    );
}

#[test]
fn two_boxes_unite_cut_and_meet() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([20, 20, 20], [40, 40, 40]);
    let both = [solid(1, &a), solid(2, &b)];
    let u = unite(&ctx(), &SERIAL, &both).unwrap();
    assert_eq!(u.exactness, Exactness::Exact);
    assert_eq!(sixfold(&u.result), 6 * (2 * 64_000 - 8_000));
    checked(&u.result);
    let s = subtract(&ctx(), &SERIAL, key(1), &both).unwrap();
    assert_eq!(sixfold(&s.result), 6 * (64_000 - 8_000));
    checked(&s.result);
    let i = intersect(&ctx(), &SERIAL, &both).unwrap();
    assert_eq!(sixfold(&i.result), 6 * 8_000);
    checked(&i.result);
}

#[test]
fn one_arrangement_gives_every_selection() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([20, 20, 20], [40, 40, 40]);
    let input = BooleanInput {
        operands: vec![solid(1, &a), solid(2, &b)],
        select: vec![
            Selection::Unite,
            Selection::Intersect,
            Selection::Subtract { target: key(2) },
            Selection::Where(vec![
                Term::Inside(key(1)),
                Term::Inside(key(2)),
                Term::Not,
                Term::And,
            ]),
        ],
        merge: Merge::RemoveRedundant,
        accept: None,
    };
    let out = combine(&ctx(), &SERIAL, &input).unwrap();
    let volumes: Vec<i128> = out.result.iter().flatten().map(sixfold).collect();
    assert_eq!(volumes, [6 * 120_000, 6 * 8_000, 6 * 56_000, 6 * 56_000]);
}

#[test]
fn a_door_through_a_wall_leaves_an_opening() {
    let wall = cuboid([0, 0, 0], [100, 10, 60]);
    let door = cuboid([40, -5, 0], [20, 20, 40]);
    let out = subtract(&ctx(), &SERIAL, key(1), &[solid(1, &wall), solid(2, &door)]).unwrap();
    assert_eq!(sixfold(&out.result), 6 * (100 * 10 * 60 - 20 * 10 * 40));
    checked(&out.result);
}

#[test]
fn many_tools_cut_in_one_call() {
    let wall = cuboid([0, 0, 0], [300, 10, 60]);
    let tools: Vec<Body> = (0..6)
        .map(|i| cuboid([20 + 45 * i, -5, 10], [20, 20, 30]))
        .collect();
    let mut operands = vec![solid(0, &wall)];
    for (i, tool) in tools.iter().enumerate() {
        operands.push(solid(i as u8 + 1, tool));
    }
    let out = subtract(&ctx(), &SERIAL, key(0), &operands).unwrap();
    assert_eq!(sixfold(&out.result), 6 * (300 * 10 * 60 - 6 * 20 * 10 * 30));
    checked(&out.result);
}

#[test]
fn a_result_goes_straight_back_in() {
    let mut body = cuboid([0, 0, 0], [200, 40, 40]);
    let whole = 200 * 40 * 40;
    let mut left = whole;
    for i in 0..10 {
        let tool = cuboid([5 + 19 * i, -5, 10], [10, 50, 20]);
        let out = subtract(&ctx(), &SERIAL, key(1), &[solid(1, &body), solid(2, &tool)]).unwrap();
        left -= 10 * 40 * 20;
        assert_eq!(sixfold(&out.result), 6 * left, "cut {i}");
        checked(&out.result);
        body = out.result;
    }
}

#[test]
fn a_tilted_prism_cuts_and_comes_back() {
    let block = cuboid([0, 0, 0], [60, 60, 60]);
    let base = vec![p(10, 5, -5), p(50, 15, -5), p(30, 45, -5)];
    let input = geom_make::PrismInput {
        base,
        height: Steps(80),
    };
    let tool = prism(&ctx(), &input).unwrap().result.body;
    let out = subtract(
        &ctx(),
        &SERIAL,
        key(1),
        &[solid(1, &block), solid(2, &tool)],
    )
    .unwrap();
    // The triangle's area is 700, through all 60 of the block.
    assert_eq!(sixfold(&out.result), 6 * (216_000 - 700 * 60));
    checked(&out.result);
    let again = cuboid([20, 20, 20], [60, 60, 60]);
    let more = unite(&ctx(), &SERIAL, &[solid(1, &out.result), solid(2, &again)]).unwrap();
    checked(&more.result);
}

#[test]
fn the_order_of_the_operands_does_not_count() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([20, 20, 20], [40, 40, 40]);
    let forward = unite(&ctx(), &SERIAL, &[solid(1, &a), solid(2, &b)]).unwrap();
    let backward = unite(&ctx(), &SERIAL, &[solid(2, &b), solid(1, &a)]).unwrap();
    assert_eq!(forward.result, backward.result);
}

#[test]
fn seams_stay_and_come_back_in() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([40, 0, 0], [40, 40, 40]);
    let input = BooleanInput {
        operands: vec![solid(1, &a), solid(2, &b)],
        select: vec![Selection::Unite],
        merge: Merge::KeepSeams,
        accept: None,
    };
    let seamed = combine(&ctx(), &SERIAL, &input)
        .unwrap()
        .result
        .remove(0)
        .remove(0);
    checked(&seamed);
    let tool = cuboid([30, 10, 10], [20, 20, 40]);
    let out = subtract(
        &ctx(),
        &SERIAL,
        key(1),
        &[solid(1, &seamed), solid(2, &tool)],
    )
    .unwrap();
    assert_eq!(sixfold(&out.result), 6 * (128_000 - 20 * 20 * 30));
    checked(&out.result);
}

#[test]
fn a_cutter_cuts_and_counts_nothing() {
    let block = cuboid([0, 0, 0], [40, 40, 40]);
    let other = cuboid([20, 20, 20], [40, 40, 40]);
    let cutter = Operand {
        kind: OperandKind::Cutter,
        ..solid(2, &other)
    };
    let out = unite(&ctx(), &SERIAL, &[solid(1, &block), cutter]).unwrap();
    assert_eq!(sixfold(&out.result), 6 * 64_000);
}

#[test]
fn refusals_name_what_is_wrong() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let detail = |e: geom_op::OpError| *e.detail();
    let twice = unite(&ctx(), &SERIAL, &[solid(1, &a), solid(1, &a)]).unwrap_err();
    assert_eq!(
        detail(twice),
        OpErrorDetail::Combine(CombineDetail::DuplicateOperand)
    );
    let missing = subtract(&ctx(), &SERIAL, key(9), &[solid(1, &a)]).unwrap_err();
    assert_eq!(
        detail(missing),
        OpErrorDetail::Combine(CombineDetail::UnknownOperand)
    );
    let input = BooleanInput {
        operands: vec![solid(1, &a)],
        select: vec![Selection::Where(vec![Term::And])],
        merge: Merge::RemoveRedundant,
        accept: None,
    };
    let bad = combine(&ctx(), &SERIAL, &input).unwrap_err();
    assert_eq!(bad.kind(), ErrorKind::InvalidInput);
    let outside = BooleanInput {
        operands: vec![solid(1, &a)],
        select: vec![Selection::Where(vec![Term::Inside(key(1)), Term::Not])],
        merge: Merge::RemoveRedundant,
        accept: None,
    };
    let unbounded = combine(&ctx(), &SERIAL, &outside).unwrap_err();
    assert_eq!(
        detail(unbounded),
        OpErrorDetail::Combine(CombineDetail::UnboundedResult)
    );
}

#[test]
fn a_half_space_clips_a_block_under_a_roof() {
    let block = cuboid([0, 0, 0], [60, 60, 60]);
    let roof = half_space([p(60, 0, 0), p(60, 60, 0), p(0, 0, 60)], &mut meter()).unwrap();
    checked(&roof);
    let out = intersect(&ctx(), &SERIAL, &[solid(1, &block), solid(2, &roof)]).unwrap();
    assert_eq!(sixfold(&out.result), 6 * 108_000);
    checked(&out.result);
}

#[test]
fn half_spaces_face_every_way_and_steep_ones_too() {
    let block = cuboid([0, 0, 0], [60, 60, 60]);
    // Behind x = 20, y = 30 and z = 45, each way round, and a plane steepest along each axis.
    let cases: [([P3; 3], i128); 8] = [
        ([p(20, 0, 0), p(20, 1, 0), p(20, 0, 1)], 20 * 3600),
        ([p(20, 0, 0), p(20, 0, 1), p(20, 1, 0)], 40 * 3600),
        ([p(0, 30, 0), p(0, 30, 1), p(1, 30, 0)], 30 * 3600),
        ([p(0, 30, 0), p(1, 30, 0), p(0, 30, 1)], 30 * 3600),
        ([p(0, 0, 45), p(1, 0, 45), p(0, 1, 45)], 45 * 3600),
        ([p(0, 0, 45), p(0, 1, 45), p(1, 0, 45)], 15 * 3600),
        // The plane x + y = 60, halving the block either way.
        ([p(60, 0, 0), p(0, 60, 0), p(60, 0, 1)], 108_000),
        ([p(60, 0, 0), p(60, 0, 1), p(0, 60, 0)], 108_000),
    ];
    for (points, want) in cases {
        let space = half_space(points, &mut meter()).unwrap();
        let out = intersect(&ctx(), &SERIAL, &[solid(1, &block), solid(2, &space)]).unwrap();
        assert_eq!(sixfold(&out.result), 6 * want, "{points:?}");
        checked(&out.result);
    }
}

#[test]
fn a_half_space_needs_three_points_that_make_a_plane() {
    let line = [p(0, 0, 0), p(1, 1, 1), p(2, 2, 2)];
    assert!(matches!(
        half_space(line, &mut meter()),
        Err(RebuildError::Num(_))
    ));
}

#[test]
fn a_result_less_exact_than_the_caller_takes_is_refused() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([20, 20, 20], [40, 40, 40]);
    let step = geom_num::GridStep::Q0;
    let rough = Exactness::Certified {
        bound: Steps(4),
        step,
    };
    let loose = Operand {
        exactness: rough,
        ..solid(2, &b)
    };
    let input = |accept| BooleanInput {
        operands: vec![solid(1, &a), loose],
        select: vec![Selection::Unite],
        merge: Merge::RemoveRedundant,
        accept,
    };
    let taken = combine(&ctx(), &SERIAL, &input(None)).unwrap();
    assert_eq!(taken.exactness, rough);
    let wide = Exactness::Certified {
        bound: Steps(4),
        step,
    };
    assert!(combine(&ctx(), &SERIAL, &input(Some(wide))).is_ok());
    let tight = Exactness::Certified {
        bound: Steps(1),
        step,
    };
    for accept in [Exactness::Exact, tight] {
        let refused = combine(&ctx(), &SERIAL, &input(Some(accept))).unwrap_err();
        assert_eq!(refused.kind(), ErrorKind::ExactnessNotAccepted);
        assert_eq!(
            *refused.detail(),
            OpErrorDetail::Combine(CombineDetail::NotAccepted { would: rough })
        );
    }
    let over = Exactness::Certified {
        bound: Steps(1000),
        step,
    };
    let far = Operand {
        exactness: over,
        ..solid(2, &b)
    };
    let beyond = BooleanInput {
        operands: vec![solid(1, &a), far],
        select: vec![Selection::Unite],
        merge: Merge::RemoveRedundant,
        accept: None,
    };
    assert_eq!(
        combine(&ctx(), &SERIAL, &beyond).unwrap_err().kind(),
        ErrorKind::ExactnessNotAccepted
    );
}

/// A plane square to an axis, its normal along +`axis` when `sign` is 1.
fn square(axis: usize, sign: i128, at: i64) -> Plane {
    let mut n = [0; 3];
    n[axis] = sign;
    Plane::new(n, -sign * i128::from(at)).unwrap()
}

/// A rectangular sheet square to `axis` at `at`, over `low` to `high` on the next two axes.
fn sheet(axis: usize, at: i64, low: [i64; 2], high: [i64; 2]) -> Body {
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let piece = Piece {
        face: square(axis, 1, at),
        bounds: vec![
            square(v, 1, low[1]),
            square(u, -1, high[0]),
            square(v, -1, high[1]),
            square(u, 1, low[0]),
        ],
    };
    let unit = |k: usize| [0, 1, 2].map(|i| if i == k { 1.0 } else { 0.0 });
    let mut origin = [0.0; 3];
    origin[axis] = at as f64;
    let wide = (-1e9, 1e9);
    let surface = FlatSurface::new(origin, unit(u), unit(v), (wide, wide)).unwrap();
    let carrier = Carrier {
        surface: SurfaceKind::Plane(surface),
        sense: Sense::Forward,
    };
    let pieces = [Outward {
        piece,
        polygon: 0,
        turned: false,
        source: 0,
        front: Winding::new(),
        back: Winding::new(),
    }];
    let rules = Rules {
        cuts: &[],
        open: true,
    };
    let body = rebuild_with(
        &pieces,
        &[carrier],
        Merge::RemoveRedundant,
        rules,
        &mut meter(),
    );
    let body = body.unwrap();
    assert_eq!(body.kind(), BodyKind::Sheet);
    checked(&body);
    body
}

fn cutter(n: u8, body: &Body) -> Operand<'_> {
    Operand {
        kind: OperandKind::Cutter,
        ..solid(n, body)
    }
}

/// `selection` of `operands`, in one call.
fn picked(operands: Vec<Operand<'_>>, selection: Selection) -> geom_op::OpOutput<Vec<Vec<Body>>> {
    let input = BooleanInput {
        operands,
        select: vec![selection],
        merge: Merge::RemoveRedundant,
        accept: None,
    };
    combine(&ctx(), &SERIAL, &input).unwrap()
}

#[test]
fn an_imprint_splits_faces_and_changes_no_shape() {
    let slab = cuboid([0, 0, 0], [100, 100, 20]);
    let column = cuboid([40, 40, 20], [20, 20, 50]);
    let selection = Selection::Imprint { target: key(1) };
    let out = picked(vec![solid(1, &slab), solid(2, &column)], selection.clone());
    let body = &out.result[0][0];
    assert_eq!(sixfold(body), 6 * 200_000);
    assert_eq!(body.arenas().faces.len(), 7);
    checked(body);
    let line = sheet(0, 30, [-10, -10], [110, 110]);
    let cut = picked(vec![solid(1, &slab), cutter(2, &line)], selection.clone());
    let body = &cut.result[0][0];
    assert_eq!(sixfold(body), 6 * 200_000);
    // Top, bottom and the two long sides each in two.
    assert_eq!(body.arenas().faces.len(), 10);
    checked(body);
    // A line that stops inside a face leaves the face whole.
    let short = sheet(0, 30, [10, -10], [50, 110]);
    let cut = picked(vec![solid(1, &slab), cutter(2, &short)], selection);
    let body = &cut.result[0][0];
    assert_eq!(body.arenas().faces.len(), 6);
    checked(body);
}

#[test]
fn a_split_gives_one_body_per_cell() {
    let wall = cuboid([0, 0, 0], [100, 10, 60]);
    let tool = cuboid([40, -5, -5], [20, 20, 70]);
    let selection = Selection::Split { target: key(1) };
    let out = picked(vec![solid(1, &wall), solid(2, &tool)], selection.clone());
    let mut volumes: Vec<i128> = out.result[0].iter().map(sixfold).collect();
    volumes.sort_unstable();
    assert_eq!(
        volumes,
        [6 * 12_000, 6 * 24_000, 6 * 24_000],
        "both ends and the middle"
    );
    out.result[0].iter().for_each(checked);
    let line = sheet(0, 30, [-10, -10], [20, 70]);
    let out = picked(vec![solid(1, &wall), cutter(2, &line)], selection.clone());
    let mut volumes: Vec<i128> = out.result[0].iter().map(sixfold).collect();
    volumes.sort_unstable();
    assert_eq!(volumes, [6 * 18_000, 6 * 42_000]);
    out.result[0].iter().for_each(checked);
    let inside = cuboid([10, 2, 10], [20, 6, 20]);
    let out = picked(vec![solid(1, &wall), solid(2, &inside)], selection);
    let mut volumes: Vec<i128> = out.result[0].iter().map(sixfold).collect();
    volumes.sort_unstable();
    assert_eq!(volumes, [6 * 2_400, 6 * (60_000 - 2_400)]);
    out.result[0].iter().for_each(checked);
}

#[test]
fn a_cutter_that_stops_short_leaves_the_target_whole_and_says_so() {
    let wall = cuboid([0, 0, 0], [100, 10, 60]);
    let short = sheet(0, 30, [-10, -10], [20, 30]);
    let selection = Selection::Split { target: key(1) };
    let out = picked(vec![solid(1, &wall), cutter(2, &short)], selection);
    assert_eq!(out.result[0].len(), 1);
    assert_eq!(sixfold(&out.result[0][0]), 6 * 60_000);
    assert_eq!(out.result[0][0].arenas().faces.len(), 6);
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.code == "bool.cutter_does_not_separate")
    );
}

#[test]
fn keep_clips_a_sheet_to_a_solid() {
    let block = cuboid([0, 0, 0], [40, 40, 40]);
    let facade = sheet(0, 20, [-10, -10], [50, 50]);
    let selection = Selection::Keep {
        target: key(2),
        test: vec![Term::Inside(key(1))],
    };
    let out = picked(vec![solid(1, &block), cutter(2, &facade)], selection);
    let kept = &out.result[0][0];
    assert_eq!(kept.kind(), BodyKind::Sheet);
    assert_eq!(kept.arenas().faces.len(), 1);
    assert_eq!(kept.arenas().edges.len(), 4);
    checked(kept);
}

#[test]
fn trim_keeps_the_pieces_that_hold_the_seeds() {
    let target = sheet(0, 0, [0, 0], [40, 40]);
    let knife = sheet(1, 20, [-10, -10], [50, 50]);
    let operands = || vec![cutter(1, &target), cutter(2, &knife)];
    let trim = |seed: P3| Selection::Trim {
        target: key(1),
        seeds: vec![seed],
    };
    let out = picked(operands(), trim(p(0, 10, 10)));
    let kept = &out.result[0][0];
    assert_eq!(kept.kind(), BodyKind::Sheet);
    assert_eq!(kept.arenas().faces.len(), 1);
    checked(kept);
    let input = |selection| BooleanInput {
        operands: operands(),
        select: vec![selection],
        merge: Merge::RemoveRedundant,
        accept: None,
    };
    let refused = |seed| {
        let err = combine(&ctx(), &SERIAL, &input(trim(seed))).unwrap_err();
        *err.detail()
    };
    assert_eq!(
        refused(p(5, 10, 10)),
        OpErrorDetail::Combine(CombineDetail::SeedOutside)
    );
    assert_eq!(
        refused(p(0, 20, 10)),
        OpErrorDetail::Combine(CombineDetail::SeedOnBoundary)
    );
}

#[test]
fn a_section_is_the_curve_where_operands_meet() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([20, 20, 20], [40, 40, 40]);
    let out = picked(vec![solid(1, &a), solid(2, &b)], Selection::Section);
    let curves = &out.result[0][0];
    assert_eq!(curves.kind(), BodyKind::Wire);
    assert_eq!(curves.arenas().edges.len(), 6);
    assert_eq!(curves.arenas().vertices.len(), 6);
    checked(curves);
    let plan = sheet(2, 20, [-10, -10], [50, 50]);
    let out = picked(vec![solid(1, &a), cutter(2, &plan)], Selection::Section);
    let curves = &out.result[0][0];
    assert_eq!(curves.arenas().edges.len(), 4);
    checked(curves);
    let apart = cuboid([100, 0, 0], [10, 10, 10]);
    let out = picked(vec![solid(1, &a), solid(2, &apart)], Selection::Section);
    assert_eq!(out.result[0][0].arenas().edges.len(), 0);
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.code == "bool.empty_result")
    );
}

#[test]
fn a_refused_preparation_says_why() {
    let k = key(1);
    for (error, text) in [
        (
            PrepareError::DuplicateOperand(k),
            "two operands share a key",
        ),
        (
            PrepareError::UnknownOperand(k),
            "a selection names no solid operand",
        ),
        (
            PrepareError::NotPlaneBased(k),
            "an operand has a face not on integers",
        ),
        (
            PrepareError::Broken(k),
            "an operand has a face that cannot enter",
        ),
    ] {
        assert_eq!(error.to_string(), text);
    }
    let num = PrepareError::from(NumError::Degenerate).to_string();
    assert!(num.starts_with("an operand that cannot enter"));
    let stopped = WorkMeter::new(Budget::new(0, u64::MAX))
        .charge(1)
        .unwrap_err();
    assert_eq!(
        PrepareError::Stopped(stopped).to_string(),
        "the meter stopped the preparation"
    );
    let block = cuboid([0, 0, 0], [4, 4, 4]);
    for selection in [
        Selection::Imprint { target: key(9) },
        Selection::Split { target: key(9) },
        Selection::Keep {
            target: key(9),
            test: vec![Term::Inside(key(1))],
        },
        Selection::Trim {
            target: key(9),
            seeds: Vec::new(),
        },
    ] {
        let input = BooleanInput {
            operands: vec![solid(1, &block)],
            select: vec![selection],
            merge: Merge::RemoveRedundant,
            accept: None,
        };
        let refused = combine(&ctx(), &SERIAL, &input).unwrap_err();
        assert_eq!(
            *refused.detail(),
            OpErrorDetail::Combine(CombineDetail::UnknownOperand)
        );
    }
}

#[test]
fn any_budget_gives_the_answer_or_stops_cleanly() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([20, 20, 20], [40, 40, 40]);
    let whole = unite(&ctx(), &SERIAL, &[solid(1, &a), solid(2, &b)]).unwrap();
    let mut stops = 0;
    for budget in (0..8000).step_by(11) {
        let small = OpContext::new(
            OpPath::new(OpId::from_bytes([7; 16])),
            Budget::new(budget, u64::MAX),
        );
        match unite(&small, &SERIAL, &[solid(1, &a), solid(2, &b)]) {
            Ok(out) => assert_eq!(out.result, whole.result),
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::BudgetExceeded, "{e:?}");
                stops += 1;
            }
        }
    }
    assert!(stops > 0);
}

#[test]
fn keep_takes_a_sheet_that_lies_on_a_solids_face() {
    let block = cuboid([0, 0, 0], [40, 40, 40]);
    let on_face = sheet(0, 0, [10, 10], [30, 30]);
    let selection = Selection::Keep {
        target: key(2),
        test: vec![Term::Inside(key(1))],
    };
    let out = picked(vec![solid(1, &block), cutter(2, &on_face)], selection);
    assert_eq!(out.result[0][0].arenas().faces.len(), 1);
}

#[test]
fn a_half_space_on_a_side_of_the_range_box_is_the_whole_box() {
    let m = geom_num::grid::COORD_MAX;
    let whole = half_space([p(m, 0, 0), p(m, 1, 0), p(m, 0, 1)], &mut meter()).unwrap();
    assert_eq!(whole.arenas().faces.len(), 6);
    checked(&whole);
}

#[test]
fn a_seam_that_ends_off_the_grid_comes_back_in() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([40, 0, 0], [40, 40, 40]);
    let call = |operands, selection| BooleanInput {
        operands,
        select: vec![selection],
        merge: Merge::KeepSeams,
        accept: None,
    };
    let united = call(vec![solid(1, &a), solid(2, &b)], Selection::Unite);
    let seamed = combine(&ctx(), &SERIAL, &united)
        .unwrap()
        .result
        .remove(0)
        .remove(0);
    // 3y + z = 101 meets the seam on top at y = 61/3.
    let tilt = half_space([p(0, 30, 11), p(1, 30, 11), p(0, 31, 8)], &mut meter()).unwrap();
    let meeting = call(
        vec![solid(1, &seamed), solid(2, &tilt)],
        Selection::Intersect,
    );
    let cut = combine(&ctx(), &SERIAL, &meeting)
        .unwrap()
        .result
        .remove(0)
        .remove(0);
    checked(&cut);
    let off_grid = (0..cut.arenas().points.len()).any(|at| {
        matches!(
            cut.arenas().points.get(geom_topo::Ix::new(at).unwrap()),
            Some(geom_topo::PointKind::Meet(_))
        )
    });
    assert!(off_grid);
    let tool = cuboid([30, 5, 5], [20, 5, 5]);
    let again = subtract(&ctx(), &SERIAL, key(1), &[solid(1, &cut), solid(2, &tool)]).unwrap();
    checked(&again.result);
}

#[test]
fn a_loop_may_start_anywhere_along_a_side() {
    let slab = cuboid([0, 0, 0], [100, 100, 20]);
    // A sheet that stands on the top: the side faces gain a vertex in the middle of a side.
    let line = sheet(0, 30, [-10, 20], [110, 110]);
    let selection = Selection::Imprint { target: key(1) };
    let mut out = picked(vec![solid(1, &slab), cutter(2, &line)], selection);
    let body = out.result.remove(0).remove(0);
    assert_eq!(body.arenas().faces.len(), 7);
    let tool = cuboid([40, 40, -5], [20, 20, 40]);
    let want = 6 * (200_000 - 20 * 20 * 20);
    let arenas = body.arenas();
    let mut turns = 0;
    for row in 0..arenas.loops.len() {
        let ix = geom_topo::Ix::new(row).unwrap();
        let start = arenas.loops.get(ix).unwrap().first_fin.unwrap();
        let mut at = arenas.fins.get(start).unwrap().next;
        while at != start {
            let mut builder = Builder::edit(&body);
            builder.arenas.loops.first_fin.set(ix, Some(at)).unwrap();
            let turned = builder.publish(body.kind(), body.outside(), &mut meter());
            let turned = turned.unwrap();
            let pair = [solid(1, &turned), solid(2, &tool)];
            let cut = subtract(&ctx(), &SERIAL, key(1), &pair).unwrap();
            assert_eq!(sixfold(&cut.result), want);
            turns += 1;
            at = arenas.fins.get(at).unwrap().next;
        }
    }
    assert!(turns > 18);
}

#[test]
fn a_section_through_a_tilted_face_meets_off_the_grid() {
    let block = cuboid([0, 0, 0], [60, 60, 60]);
    let base = vec![p(-20, 10, -5), p(31, -17, -5), p(30, 45, -5)];
    let input = geom_make::PrismInput {
        base,
        height: Steps(80),
    };
    let tool = prism(&ctx(), &input).unwrap().result.body;
    let out = picked(vec![solid(1, &block), solid(2, &tool)], Selection::Section);
    let curves = &out.result[0][0];
    assert_eq!(curves.kind(), BodyKind::Wire);
    let off_grid = (0..curves.arenas().points.len()).any(|at| {
        matches!(
            curves.arenas().points.get(geom_topo::Ix::new(at).unwrap()),
            Some(geom_topo::PointKind::Meet(_))
        )
    });
    assert!(off_grid);
    checked(curves);
}

#[test]
fn a_curved_operand_waits_for_the_curved_path() {
    let round = cylinder(
        &ctx(),
        &geom_make::RoundInput {
            base: p(0, 0, 0),
            radius: Steps(16),
            height: Steps(16),
            step: geom_num::GridStep::Q0,
        },
    )
    .unwrap()
    .result
    .body;
    let err = unite(&ctx(), &SERIAL, &[solid(1, &round)]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Unsupported);
}

#[test]
fn a_small_budget_stops_the_call() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([20, 20, 20], [40, 40, 40]);
    let small = OpContext::new(
        OpPath::new(OpId::from_bytes([7; 16])),
        Budget::new(30, u64::MAX),
    );
    let got = unite(&small, &SERIAL, &[solid(1, &a), solid(2, &b)]);
    assert!(got.is_err());
}

/// The exact corners of `body`, sorted, and its counts of shells, faces and edges: its shape.
fn shape(body: &Body) -> (Vec<[i64; 3]>, [u32; 3]) {
    let a = body.arenas();
    let mut corners = Vec::new();
    for row in 0..a.points.len() {
        match a.points.get(geom_topo::Ix::new(row).unwrap()) {
            Some(geom_topo::PointKind::Grid(g)) => {
                corners.push([g.x.get(), g.y.get(), g.z.get()]);
            }
            other => panic!("not on the grid: {other:?}"),
        }
    }
    corners.sort_unstable();
    (corners, [a.shells.len(), a.faces.len(), a.edges.len()])
}

#[test]
fn a_thousand_operands_unite_in_one_arrangement() {
    let mut boxes = Vec::new();
    for i in 0..10 {
        for j in 0..10 {
            for k in 0..10 {
                boxes.push(cuboid([3 * i, 3 * j, 3 * k], [2, 2, 2]));
            }
        }
    }
    let operands: Vec<Operand<'_>> = boxes
        .iter()
        .enumerate()
        .map(|(i, b)| Operand {
            key: OperandKey((i as u128).to_be_bytes()),
            ..solid(0, b)
        })
        .collect();
    let out = unite(&ctx(), &SERIAL, &operands).unwrap();
    assert_eq!(out.result.arenas().shells.len(), 1000);
    assert_eq!(sixfold(&out.result), 6 * 8 * 1000);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn many_at_once_is_one_after_another_in_any_order(
        boxes in prop::collection::vec(
            ([-6i64..6, -6i64..6, -6i64..6], [1u64..6, 1u64..6, 1u64..6]),
            2..6,
        ),
        turn in 0usize..6,
    ) {
        let bodies: Vec<Body> = boxes.iter().map(|(low, size)| cuboid(*low, *size)).collect();
        let operands: Vec<Operand<'_>> = bodies
            .iter()
            .enumerate()
            .map(|(i, b)| solid(i as u8, b))
            .collect();
        let mut shuffled = operands.clone();
        let spin = turn % shuffled.len();
        shuffled.rotate_left(spin);
        let whole = unite(&ctx(), &SERIAL, &shuffled).unwrap().result;
        prop_assert_eq!(&whole, &unite(&ctx(), &SERIAL, &operands).unwrap().result);
        let mut held = bodies[0].clone();
        for (i, body) in bodies.iter().enumerate().skip(1) {
            let pair = [solid(0, &held), solid(i as u8, body)];
            held = unite(&ctx(), &SERIAL, &pair).unwrap().result;
        }
        prop_assert_eq!(shape(&whole), shape(&held));
        let cut = subtract(&ctx(), &SERIAL, key(0), &shuffled).unwrap().result;
        let mut left = bodies[0].clone();
        for (i, body) in bodies.iter().enumerate().skip(1) {
            let pair = [solid(0, &left), solid(i as u8, body)];
            left = subtract(&ctx(), &SERIAL, key(0), &pair).unwrap().result;
        }
        prop_assert_eq!(shape(&cut), shape(&left));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn any_boxes_keep_their_volumes(
        boxes in prop::collection::vec(
            ([-6i64..6, -6i64..6, -6i64..6], [1u64..6, 1u64..6, 1u64..6]),
            2..4,
        )
    ) {
        let bodies: Vec<Body> = boxes.iter().map(|(low, size)| cuboid(*low, *size)).collect();
        let operands: Vec<Operand<'_>> = bodies
            .iter()
            .enumerate()
            .map(|(i, b)| solid(i as u8, b))
            .collect();
        let inside = |x: i64, y: i64, z: i64, k: &([i64; 3], [u64; 3])| {
            let (l, s) = k;
            x >= l[0] && x < l[0] + s[0] as i64
                && y >= l[1] && y < l[1] + s[1] as i64
                && z >= l[2] && z < l[2] + s[2] as i64
        };
        let mut count = 0i128;
        for x in -7..12 {
            for y in -7..12 {
                for z in -7..12 {
                    count += i128::from(boxes.iter().any(|k| inside(x, y, z, k)));
                }
            }
        }
        let out = unite(&ctx(), &SERIAL, &operands).unwrap();
        if out.result.kind() == BodyKind::Solid {
            prop_assert_eq!(sixfold(&out.result), 6 * count);
            checked(&out.result);
        }
    }
}
