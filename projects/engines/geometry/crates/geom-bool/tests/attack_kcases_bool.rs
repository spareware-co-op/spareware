//! Attack tests from solidcore's kernel-case corpus: flat booleans, `geom-bool`.
//!
//! The corpus is `crates/solidcore/tests/kernel_cases/` in solidcore (tangency-labs/solidcore,
//! Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70): one entry per operation-level
//! case, with the operands as B-rep records, the operation, and an expected answer derived by hand
//! (a volume, a face count). The entries here are the ones whose operands are flat and whose
//! operation is a boolean: boxes, prisms and their differences, joined, cut or intersected.
//! Each test rebuilds the operands from the entry's records in our own API (boxes by their
//! corners, a prism by its base, a wedge by a half-space; an operand that has a hole or a step is
//! built with our own difference or union, and the test builds it first to check its volume), runs
//! the operation, and holds the result to what the entry's `expected` says: the volume exactly, the
//! lump and shell counts, and the face (or vertex, edge and face) counts where the entry gives them.
//! A mismatch in a face count is also a finding: the entry's counts are of faces merged across
//! coplanar neighbours, as SolidCraft reads them.
//!
//! One millimetre is [`QUARTER_NM_PER_MM`] grid steps (4 000 000), so every coordinate of the
//! corpus (at most four decimals) is a whole number of steps and the arithmetic stays exact. Each
//! test cites its entry as `<area>/<case>.json`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and indexing, prose that names corpus files, and a failure stops the test"
)]

use geom_bool::combine::{Combined, intersect, subtract, unite};
use geom_bool::half_space::half_space;
use geom_bool::operand::{Operand, OperandKey, OperandKind};
use geom_check::{Level, check};
use geom_make::{BoxInput, PrismInput, make_box, prism};
use geom_num::grid::QUARTER_NM_PER_MM;
use geom_num::{Exactness, P3, Steps};
use geom_op::{OpContext, OpId, OpPath};
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::Body;
use geom_topo::query::shell_volume;

const SERIAL: Exec<'static> = Exec::serial();

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn key(n: u8) -> OperandKey {
    OperandKey([n; 16])
}

/// Millimetres to grid steps (every corpus coordinate is a whole number of them).
fn st(mm: f64) -> i64 {
    (mm * QUARTER_NM_PER_MM as f64).round() as i64
}

fn pt(x: f64, y: f64, z: f64) -> P3 {
    P3::new(st(x), st(y), st(z)).unwrap()
}

/// Six times a volume given in cubic millimetres as a decimal string of at most nine decimals,
/// in cubic steps: the unit of [`sixfold`].
fn six(volume_mm3: &str) -> i128 {
    let (whole, frac) = volume_mm3.split_once('.').unwrap_or((volume_mm3, ""));
    assert!(frac.len() <= 9, "at most nine decimals");
    let frac9 = format!("{frac:0<9}");
    let nano: i128 =
        whole.parse::<i128>().unwrap() * 1_000_000_000 + frac9.parse::<i128>().unwrap();
    // (4 000 000)^3 steps^3 per mm^3, over 1e9 for the nano.
    6 * nano * 64_000_000_000
}

/// How a corpus entry's operand is built.
#[derive(Clone)]
enum Shape {
    /// A box by its least corner and its sizes, in mm.
    Cuboid([f64; 3], [f64; 3]),
    /// A prism over a convex base (counter-clockwise from above) at `z`, `height` high, in mm.
    Column(Vec<[f64; 2]>, f64, f64),
    /// The first less the second.
    Minus(Box<Shape>, Box<Shape>),
    /// The two united.
    Plus(Box<Shape>, Box<Shape>),
    /// The first cut by the half-space behind the plane through three points (in mm).
    Cut(Box<Shape>, [[f64; 3]; 3]),
}

fn bx(min: [f64; 3], size: [f64; 3]) -> Shape {
    Shape::Cuboid(min, size)
}

fn minus(a: Shape, b: Shape) -> Shape {
    Shape::Minus(Box::new(a), Box::new(b))
}

fn plus(a: Shape, b: Shape) -> Shape {
    Shape::Plus(Box::new(a), Box::new(b))
}

fn solid(n: u8, body: &Body) -> Operand<'_> {
    Operand {
        key: key(n),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

/// Six times `body`'s volume, summed over its shells.
fn sixfold(body: &Body) -> i128 {
    if body.arenas().faces.is_empty() {
        return 0;
    }
    let mut total = 0i128;
    for row in 0..body.arenas().shells.len() {
        let shell = geom_topo::ShellIx::new(row).unwrap();
        total += shell_volume(body, shell, &mut meter())
            .unwrap()
            .to_i128()
            .unwrap();
    }
    total
}

fn checked(body: &Body, what: &str) {
    let report = check(body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(report.passed(), "{what}: {report:?}");
}

fn vef(body: &Body) -> (usize, usize, usize) {
    let a = body.arenas();
    (
        a.vertices.len() as usize,
        a.edges.len() as usize,
        a.faces.len() as usize,
    )
}

fn build(shape: &Shape) -> Body {
    let body = match shape {
        Shape::Cuboid(min, size) => {
            let input = BoxInput {
                min: pt(min[0], min[1], min[2]),
                size: size.map(|s| Steps(st(s) as u64)),
            };
            make_box(&ctx(), &input).unwrap().result.body
        }
        Shape::Column(base, z, height) => {
            let input = PrismInput {
                base: base.iter().map(|c| pt(c[0], c[1], *z)).collect(),
                height: Steps(st(*height) as u64),
            };
            prism(&ctx(), &input).unwrap().result.body
        }
        Shape::Minus(a, b) => {
            let (a, b) = (build(a), build(b));
            subtract(&ctx(), &SERIAL, key(1), &[solid(1, &a), solid(2, &b)])
                .unwrap()
                .result
                .body
        }
        Shape::Plus(a, b) => {
            let (a, b) = (build(a), build(b));
            unite(&ctx(), &SERIAL, &[solid(1, &a), solid(2, &b)])
                .unwrap()
                .result
                .body
        }
        Shape::Cut(a, points) => {
            let a = build(a);
            let h = half_space(
                ctx().path(),
                points.map(|p| pt(p[0], p[1], p[2])),
                &mut meter(),
            )
            .unwrap();
            intersect(&ctx(), &SERIAL, &[solid(1, &a), solid(2, &h)])
                .unwrap()
                .result
                .body
        }
    };
    checked(&body, "an operand");
    body
}

#[derive(Clone, Copy)]
enum Op {
    Union,
    /// The first operand less the second (the corpus lists the target first).
    Subtract,
}

/// One corpus entry: its file, the operation on its two operands, and what it expects.
struct Case {
    file: &'static str,
    op: Op,
    a: Shape,
    b: Shape,
    /// The expected volume in mm^3, as the entry states it.
    volume: &'static str,
    /// The expected shell count (one lump: one shell, unless the entry says otherwise).
    shells: usize,
    /// The expected face count, where the entry gives one.
    faces: Option<usize>,
    /// The expected vertex, edge and face counts, where the entry gives all three.
    vef: Option<(usize, usize, usize)>,
}

fn case(file: &'static str, op: Op, a: Shape, b: Shape, volume: &'static str) -> Case {
    Case {
        file,
        op,
        a,
        b,
        volume,
        shells: 1,
        faces: None,
        vef: None,
    }
}

fn run(c: &Case) {
    let (a, b) = (build(&c.a), build(&c.b));
    let operands = [solid(1, &a), solid(2, &b)];
    let out: Combined = match c.op {
        Op::Union => unite(&ctx(), &SERIAL, &operands),
        Op::Subtract => subtract(&ctx(), &SERIAL, key(1), &operands),
    }
    .unwrap_or_else(|e| panic!("{}: refused: {e:?}", c.file))
    .result;
    let body = &out.body;
    assert_eq!(sixfold(body), six(c.volume), "{}: volume", c.file);
    checked(body, c.file);
    assert_eq!(
        body.arenas().shells.len() as usize,
        c.shells,
        "{}: shells",
        c.file
    );
    if let Some(f) = c.faces {
        assert_eq!(vef(body).2, f, "{}: faces", c.file);
    }
    if let Some(v) = c.vef {
        assert_eq!(vef(body), v, "{}: vertices, edges, faces", c.file);
    }
    // The inputs are untouched.
    assert_eq!(a, build(&c.a), "{}: first operand changed", c.file);
    assert_eq!(b, build(&c.b), "{}: second operand changed", c.file);
}

macro_rules! entry {
    ($name:ident, $case:expr) => {
        #[test]
        fn $name() {
            run(&$case);
        }
    };
}

fn stock() -> Shape {
    bx([0.0, 0.0, 0.0], [60.0, 40.0, 20.0])
}

// ---------------------------------------------------------------------------------------------
// ladder/02-booleans

entry!(
    ladder_001_a_box_joined_exactly_on_the_stocks_top_face_is_one_box,
    Case {
        vef: Some((8, 12, 6)),
        ..case(
            "ladder/02-booleans/001-coincident-face-join.json",
            Op::Union,
            stock(),
            bx([0.0, 0.0, 20.0], [60.0, 40.0, 10.0]),
            "72000"
        )
    }
);

entry!(
    ladder_002_two_boxes_overlapping_in_plan_with_coplanar_tops_and_bottoms,
    Case {
        faces: Some(10),
        ..case(
            "ladder/02-booleans/002-coplanar-overlap-join.json",
            Op::Union,
            stock(),
            bx([30.0, 20.0, 0.0], [60.0, 40.0, 20.0]),
            "84000"
        )
    }
);

entry!(
    ladder_003_a_pocket_whose_tool_top_is_the_stocks_top_face,
    Case {
        faces: Some(11),
        ..case(
            "ladder/02-booleans/003-pocket-flush-with-top.json",
            Op::Subtract,
            stock(),
            bx([20.0, 10.0, 10.0], [20.0, 20.0, 10.0]),
            "44000"
        )
    }
);

entry!(
    ladder_004_a_cut_tool_that_shares_only_the_stocks_top_front_edge,
    Case {
        vef: Some((8, 12, 6)),
        ..case(
            "ladder/02-booleans/004-edge-on-edge-no-overlap.json",
            Op::Subtract,
            stock(),
            bx([0.0, -10.0, 20.0], [60.0, 10.0, 10.0]),
            "48000"
        )
    }
);

// A triangular prism (a wedge) whose slope meets the bottom face along one line (a knife edge)
// and whose other faces are outside the stock: the wedge is the box 0..60 x -1..41 x 0..20 cut
// by the plane 3 z = x, keeping the side above it (the triangle (0,0), (0,20), (60,20) along y).
entry!(
    ladder_015_a_wedge_cut_whose_slope_meets_the_bottom_face_in_a_knife_edge,
    Case {
        vef: Some((6, 9, 5)),
        ..case(
            "ladder/02-booleans/015-knife-edge-wedge-cut.json",
            Op::Subtract,
            stock(),
            Shape::Cut(
                Box::new(bx([0.0, -1.0, 0.0], [60.0, 42.0, 20.0])),
                [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [60.0, 0.0, 20.0]]
            ),
            "24000"
        )
    }
);

entry!(
    ladder_019_a_notch_through_the_height_whose_end_is_flush_with_the_end_face,
    Case {
        faces: Some(10),
        ..case(
            "ladder/02-booleans/019-notch-flush-with-end-face.json",
            Op::Subtract,
            stock(),
            bx([20.0, 10.0, -1.0], [40.0, 20.0, 22.0]),
            "32000"
        )
    }
);

entry!(
    ladder_020_a_back_wall_joined_on_the_top_flush_with_the_back_and_the_ends,
    Case {
        faces: Some(8),
        ..case(
            "ladder/02-booleans/020-part-back-wall.json",
            Op::Union,
            stock(),
            bx([0.0, 30.0, 20.0], [60.0, 10.0, 15.0]),
            "57000"
        )
    }
);

entry!(
    ladder_09_009_the_stock_shelled_two_outward_open_at_the_top,
    Case {
        faces: Some(11),
        ..case(
            "ladder/09-shells-and-offsets/009-shell-outside-t2.json",
            Op::Subtract,
            bx([-2.0, -2.0, -2.0], [64.0, 44.0, 22.0]),
            bx([0.0, 0.0, 0.0], [60.0, 40.0, 25.0]),
            "13952"
        )
    }
);

// ---------------------------------------------------------------------------------------------
// booleans

entry!(
    booleans_1730_a_cut_that_stops_short_of_the_far_face_leaves_the_skin,
    case(
        "booleans/1730-cut-short-of-far-face-keeps-skin.json",
        Op::Subtract,
        bx([-100.0, 0.0, 0.0], [200.0, 3.75, 95.0]),
        bx([-24.0, -0.5, -1.0], [48.0, 4.0, 91.0]),
        "56130"
    )
);

// The entry's other operand is the application's hidden copy; the boolean is the plate and the
// frame (a 36 x 36 ring, 2 wide, 3 high) joined on its top face: 8000 + 816.
entry!(
    booleans_2464_a_frame_joined_on_a_plates_top_face,
    case(
        "booleans/2464-extrude-join-leaves-hidden-body-alone.json",
        Op::Union,
        bx([0.0, 0.0, 0.0], [40.0, 40.0, 5.0]),
        minus(
            bx([2.0, 2.0, 5.0], [36.0, 36.0, 3.0]),
            bx([4.0, 4.0, 4.0], [32.0, 32.0, 5.0])
        ),
        "8816"
    )
);

entry!(
    booleans_3327_a_cut_ending_exactly_in_a_side_wall_opens_the_wall,
    Case {
        faces: Some(11),
        ..case(
            "booleans/3327-cut-ending-flush-with-side-wall.json",
            Op::Subtract,
            bx([0.0, 0.0, 0.0], [30.0, 30.0, 10.0]),
            bx([20.0, 10.0, 3.0], [10.0, 10.0, 4.0]),
            "8600"
        )
    }
);

entry!(
    booleans_3703_two_extrudes_joined_keep_the_step,
    case(
        "booleans/3703-two-face-extrudes-join-keep-the-step.json",
        Op::Union,
        bx([0.0, 0.0, 0.0], [25.0, 20.0, 45.0]),
        bx([25.0, 0.0, 0.0], [10.0, 20.0, 45.0]),
        "31500"
    )
);

// A plate 100 x 14.5 x 10 with a notch 1.5 x 2 at its end, cut through all by a tool whose wall
// is 0.0022 from a face and whose end stops 2 short of the plate's end.
entry!(
    booleans_423_a_through_all_cut_with_a_wall_a_hair_from_a_face,
    case(
        "booleans/423-through-all-cut-must-not-reach-behind-sketch-plane.json",
        Op::Subtract,
        minus(
            bx([0.0, -7.25, 0.0], [100.0, 14.5, 10.0]),
            bx([98.5, 0.0, -1.0], [2.0, 2.0, 12.0])
        ),
        bx([-2.96, -0.0022, -1.0], [100.96, 3.0022, 12.0]),
        "11527.844"
    )
);

// A 6 x 7 rectangle prism with one corner exactly on the slanted edge of a triangular prism (the
// triangle (0,0), (40,0), (20,30), 10 high), joined on its top.
entry!(
    booleans_4322_a_rectangle_cornered_on_a_slanted_edge_joined_on_top,
    case(
        "booleans/4322-join-rectangle-cornered-on-slanted-edge.json",
        Op::Union,
        Shape::Column(vec![[0.0, 0.0], [40.0, 0.0], [20.0, 30.0]], 0.0, 10.0),
        bx([10.0, 8.0, 10.0], [6.0, 7.0, 10.0]),
        "6420"
    )
);

// The record's tool is a box whose corners are 1e-9 off the exact box (below one step): the box
// 60 x 50 x 10 joined on the top face of the block 40 x 30 x 20.
entry!(
    booleans_4558_a_tool_grown_past_the_block_on_every_side,
    Case {
        faces: Some(11),
        ..case(
            "booleans/4558-join-tool-grown-past-block.json",
            Op::Union,
            bx([-20.0, -15.0, 0.0], [40.0, 30.0, 20.0]),
            bx([-30.0, -25.0, 20.0], [60.0, 50.0, 10.0]),
            "54000"
        )
    }
);

// A plate 40 x 30 x 10 with a 30 x 20 opening through it, less a 1 x 20 x 10 tool whose face is
// the opening's wall, back into the material.
entry!(
    booleans_5462_a_tool_whose_face_is_an_opening_wall_cut_back_into_the_material,
    Case {
        faces: Some(10),
        ..case(
            "booleans/5462-press-pull-inner-wall-of-rectangular-opening.json",
            Op::Subtract,
            minus(
                bx([0.0, 0.0, 0.0], [40.0, 30.0, 10.0]),
                bx([5.0, 5.0, -1.0], [30.0, 20.0, 12.0])
            ),
            bx([4.0, 5.0, 0.0], [1.0, 20.0, 10.0]),
            "5800"
        )
    }
);

entry!(
    crashes_140_a_pad_joined_flat_but_hanging_half_past_the_edge,
    case(
        "crashes/140-join-pad-hanging-past-the-edge.json",
        Op::Union,
        bx([0.0, 0.0, 0.0], [40.0, 30.0, 20.0]),
        bx([30.0, 0.0, 20.0], [20.0, 30.0, 10.0]),
        "30000"
    )
);

entry!(
    history_cov_timeline_edit_a_notch_to_a_depth_that_opens_the_top,
    Case {
        faces: Some(10),
        ..case(
            "history/cov-timeline-edit-notch-depth.json",
            Op::Subtract,
            bx([0.0, 0.0, 0.0], [40.0, 30.0, 10.0]),
            bx([15.0, 0.0, 2.0], [10.0, 10.0, 8.0]),
            "11200"
        )
    }
);

entry!(
    plastic_cov_rest_a_solid_rectangle_on_a_plate,
    Case {
        faces: Some(11),
        ..case(
            "plastic/cov-rest-rectangular.json",
            Op::Union,
            bx([0.0, 0.0, 0.0], [60.0, 40.0, 3.0]),
            bx([22.0, 15.0, 3.0], [16.0, 10.0, 3.0]),
            "7680"
        )
    }
);

// The catch is a rectangle 2 wide, 17 high with a 1 x 3 triangle on its side (the polygon
// (18,2), (20,2), (20,16), (21,16), (20,19), (18,19) along y 17..23), its base 2 below the plate's
// top face: the box 18..20 x 2..19 joined with the triangle cut from a box by the plane 3 x + z = 79.
entry!(
    plastic_cov_snap_fit_whose_base_is_below_the_face_is_absorbed,
    case(
        "plastic/cov-snap-fit-base-below-the-face-is-absorbed.json",
        Op::Union,
        bx([0.0, 0.0, 0.0], [40.0, 40.0, 4.0]),
        plus(
            bx([18.0, 17.0, 2.0], [2.0, 6.0, 17.0]),
            Shape::Cut(
                Box::new(bx([20.0, 17.0, 16.0], [1.0, 6.0, 3.0])),
                [[21.0, 17.0, 16.0], [21.0, 23.0, 16.0], [20.0, 17.0, 19.0]]
            )
        ),
        "6589"
    )
);

// A rib 40 x 21 x 2 drawn 20 in front of a block (it reaches 1 into it).
entry!(
    sweeps_2056_a_rib_grown_along_the_sketch_normal_reaches_the_block,
    case(
        "sweeps/2056-rib-grows-along-the-sketch-normal-to-the-part.json",
        Op::Union,
        bx([0.0, 0.0, 0.0], [60.0, 40.0, 10.0]),
        bx([10.0, -20.0, 4.0], [40.0, 21.0, 2.0]),
        "25600"
    )
);

/// The corner bar of the sweep entries: an L, 8 wide in x, 5 in y, 34 up on the left and 8 deep
/// across the top to x = 40 (the polygon (-4,0), (4,0), (4,26), (40,26), (40,34), (-4,34)).
fn corner_bar() -> Shape {
    plus(
        bx([-4.0, -2.5, 0.0], [8.0, 5.0, 34.0]),
        bx([4.0, -2.5, 26.0], [36.0, 5.0, 8.0]),
    )
}

entry!(
    sweeps_3505_the_flat_wall_of_a_bar_with_a_corner_extruded_outward,
    case(
        "sweeps/3505-extrude-the-flat-wall-of-a-sweep-with-a-corner.json",
        Op::Union,
        corner_bar(),
        bx([4.0, -2.5, 0.0], [3.0, 5.0, 26.0]),
        "3190"
    )
);

entry!(
    sweeps_561_the_end_face_of_a_bar_with_a_corner_extruded_further,
    case(
        "sweeps/561-extrude-the-end-face-of-a-sweep.json",
        Op::Union,
        corner_bar(),
        bx([40.0, -2.5, 26.0], [6.0, 5.0, 8.0]),
        "3040"
    )
);

// ---------------------------------------------------------------------------------------------
// the entries that are not a single boolean

/// booleans/13-split-body-by-a-plane.json: a cube 20 on a side split by the plane z = 10, which
/// the application does as two intersections with a box each side of the plane. The pieces are two
/// bodies of 4000 each, 8000 in all.
#[test]
fn booleans_13_a_cube_split_by_a_plane_is_two_halves_of_4000() {
    let cube = build(&bx([0.0, 0.0, 0.0], [20.0, 20.0, 20.0]));
    let above = build(&bx(
        [-122.2073, -122.2073, 10.0],
        [264.4146, 264.4146, 132.2073],
    ));
    let below = build(&bx(
        [-122.2073, -122.2073, -122.2073],
        [264.4146, 264.4146, 132.2073],
    ));
    let mut total = 0;
    for (half, name) in [(&above, "above"), (&below, "below")] {
        let out = intersect(&ctx(), &SERIAL, &[solid(1, &cube), solid(2, half)])
            .unwrap()
            .result
            .body;
        checked(&out, name);
        assert_eq!(sixfold(&out), six("4000"), "{name}");
        assert_eq!(vef(&out), (8, 12, 6), "{name}");
        total += sixfold(&out);
    }
    assert_eq!(total, six("8000"));
}

/// booleans/3455-join-of-64-separate-pegs.json: one combine of 64 pegs that do not touch took 36
/// seconds in SolidCraft and grew 4 to 9 times per doubling; it must finish in seconds and keep
/// the pegs in one body of 64 shells. A peg is a 48-sided prism of circumradius 2 mm, 1 mm tall,
/// on a 6 mm grid. The volume is
/// the sum of the pegs: 64 times the area of the rounded 48-gon, which is the entry's 801.9529
/// within its four decimals.
#[test]
fn booleans_3455_sixty_four_separate_pegs_joined_in_one_combine() {
    const CENTRES: [[i64; 2]; 64] = [
        [0, 0],
        [0, 6],
        [0, 12],
        [0, 18],
        [0, 24],
        [0, 30],
        [0, 36],
        [6, 0],
        [6, 6],
        [6, 12],
        [6, 18],
        [6, 24],
        [6, 30],
        [6, 36],
        [12, 0],
        [12, 6],
        [12, 12],
        [12, 18],
        [12, 24],
        [12, 30],
        [12, 36],
        [18, 0],
        [18, 6],
        [18, 12],
        [18, 18],
        [18, 24],
        [18, 30],
        [18, 36],
        [24, 0],
        [24, 6],
        [24, 12],
        [24, 18],
        [24, 24],
        [24, 30],
        [30, 0],
        [30, 6],
        [30, 12],
        [30, 18],
        [30, 24],
        [30, 30],
        [36, 0],
        [36, 6],
        [36, 12],
        [36, 18],
        [36, 24],
        [36, 30],
        [42, 0],
        [42, 6],
        [42, 12],
        [42, 18],
        [42, 24],
        [42, 30],
        [48, 0],
        [48, 6],
        [48, 12],
        [48, 18],
        [48, 24],
        [48, 30],
        [54, 0],
        [54, 6],
        [54, 12],
        [54, 18],
        [54, 24],
        [54, 30],
    ];
    let ring = |cx: i64, cy: i64| -> Vec<[i64; 2]> {
        (0..48)
            .map(|k| {
                let t = std::f64::consts::TAU * f64::from(k) / 48.0;
                [st(cx as f64 + 2.0 * t.cos()), st(cy as f64 + 2.0 * t.sin())]
            })
            .collect()
    };
    let height = st(1.0);
    let mut bodies = Vec::new();
    let mut expected = 0i128;
    for c in CENTRES {
        let r = ring(c[0], c[1]);
        let mut area2 = 0i128;
        for (i, a) in r.iter().enumerate() {
            let b = r[(i + 1) % r.len()];
            area2 += i128::from(a[0]) * i128::from(b[1]) - i128::from(b[0]) * i128::from(a[1]);
        }
        expected += 3 * area2 * i128::from(height);
        let input = PrismInput {
            base: r.iter().map(|p| P3::new(p[0], p[1], 0).unwrap()).collect(),
            height: Steps(height as u64),
        };
        bodies.push(prism(&ctx(), &input).unwrap().result.body);
    }
    let operands: Vec<Operand<'_>> = bodies
        .iter()
        .enumerate()
        .map(|(i, b)| solid(u8::try_from(i + 1).unwrap(), b))
        .collect();
    let started = std::time::Instant::now();
    let joined = unite(&ctx(), &SERIAL, &operands).unwrap().result.body;
    let took = started.elapsed();
    assert!(
        took < std::time::Duration::from_secs(120),
        "joining 64 separate pegs took {took:?}"
    );
    checked(&joined, "the joined pegs");
    assert_eq!(joined.arenas().shells.len() as usize, 64);
    assert_eq!(sixfold(&joined), expected);
    // Against the entry's own number (801.9529 mm^3, four decimals).
    let mm3 = sixfold(&joined) as f64 / (6.0 * 64.0e18);
    assert!((mm3 - 801.9529).abs() < 1.0e-3, "{mm3}");
    assert_eq!(vef(&joined), (64 * 96, 64 * 144, 64 * 50));
}
