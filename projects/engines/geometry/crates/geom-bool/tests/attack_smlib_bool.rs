//! Attack tests from SMLib's confirmed defects, batch 5: flat booleans and topology, `geom-bool`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). Only defects a
//! C++ run confirmed are taken, and only inputs that are flat: boxes, convex prisms and half-spaces.
//! Where the defect's own input is curved, the test rebuilds its flat counterpart (the same
//! contact, the same coincidence, the same nesting) and says so. The case is built from the
//! defect's description in our own API; no fixture is copied. Each test asserts what the kernel
//! owes for that input: the exact answer (exact volume, face, edge and vertex counts, Euler
//! characteristic, the full checker passing) or a defined refusal, and never a panic, a hang or a
//! wrong answer.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and indexing, prose that names defect files, and a failure stops the test"
)]

use geom_bool::combine::{Combined, combine, intersect, subtract, unite};
use geom_bool::half_space::half_space;
use geom_bool::operand::{BooleanInput, Operand, OperandKey, OperandKind, Selection};
use geom_bool::rebuild::{Carrier, rebuild_with};
use geom_check::{Level, check};
use geom_make::{BoxInput, PrismInput, make_box, prism};
use geom_num::plane::Plane;
use geom_num::{Exactness, P3, Steps};
use geom_op::{CombineDetail, ErrorKind, OpContext, OpErrorDetail, OpId, OpPath};
use geom_poly::convex::Piece;
use geom_poly::merge::{Merge, Outward, Rules};
use geom_poly::winding::Winding;
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::query::shell_volume;
use geom_topo::{Body, BodyKind, FlatSurface, RegionKind, Sense, SurfaceKind};

const SERIAL: Exec<'static> = Exec::serial();

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn key(n: u8) -> OperandKey {
    OperandKey([n; 16])
}

fn cuboid(low: [i64; 3], size: [u64; 3]) -> Body {
    let input = BoxInput {
        min: p(low[0], low[1], low[2]),
        size: size.map(Steps),
    };
    make_box(&ctx(), &input).unwrap().result.body
}

/// A prism over `base` (corners as x, y pairs) from height `z` up by `height`.
fn column(base: &[[i64; 2]], z: i64, height: u64) -> Body {
    let input = PrismInput {
        base: base.iter().map(|c| p(c[0], c[1], z)).collect(),
        height: Steps(height),
    };
    prism(&ctx(), &input).unwrap().result.body
}

fn solid(n: u8, body: &Body) -> Operand<'_> {
    Operand {
        key: key(n),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

fn cutter(n: u8, body: &Body) -> Operand<'_> {
    Operand {
        kind: OperandKind::Cutter,
        ..solid(n, body)
    }
}

/// Six times `body`'s volume.
fn sixfold(body: &Body) -> i128 {
    if body.arenas().faces.is_empty() {
        return 0;
    }
    // The sum over the shells, as `volume` takes it, also for a body of two lumps that touch at an
    // edge, which is a general body.
    (0..body.arenas().shells.len())
        .map(|row| {
            let shell = geom_topo::ShellIx::new(row).unwrap();
            shell_volume(body, shell, &mut meter())
                .unwrap()
                .to_i128()
                .unwrap()
        })
        .sum()
}

fn checked(body: &Body) {
    let report = check(body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(report.passed(), "{report:?}");
}

/// Vertices, edges and faces.
fn vef(body: &Body) -> (usize, usize, usize) {
    let a = body.arenas();
    (
        a.vertices.len() as usize,
        a.edges.len() as usize,
        a.faces.len() as usize,
    )
}

/// V - E + F - R, with R the rings (loops beyond each face's first): the sum over shells of
/// 2 - 2 * genus.
fn euler(body: &Body) -> i64 {
    let (v, e, f) = vef(body);
    let rings = i64::from(body.arenas().loops.len()) - f as i64;
    v as i64 - e as i64 + f as i64 - rings
}

fn shells(body: &Body) -> usize {
    body.arenas().shells.len() as usize
}

/// How many regions are material, and how many are void (the outside counts as one void).
fn regions(body: &Body) -> (usize, usize) {
    let kinds = &body.arenas().regions.kind;
    let solid = kinds.iter().filter(|k| **k == RegionKind::Solid).count();
    (solid, kinds.len() as usize - solid)
}

/// A hollow cube: 100 on a side with a sealed cavity from 20 to 80.
fn hollow() -> Body {
    let outer = cuboid([0, 0, 0], [100, 100, 100]);
    let inner = cuboid([20, 20, 20], [60, 60, 60]);
    let out = subtract(
        &ctx(),
        &SERIAL,
        key(1),
        &[solid(1, &outer), solid(2, &inner)],
    )
    .unwrap();
    out.result.body
}

/// solidcore plan/upstream-issues/WR-265--manifold-boolean-void-shell-first.md (b4de2149): a cube
/// 100 less the box 10..90 x 10..90 x 10..110 (an open cup) united with the box 0..100 x 0..100 x
/// 90..100 (a lid) is one solid with a sealed 80^3 void, volume 488000, in either operand order;
/// SMLib put the cavity's shell first in the solid region and its own check refused the result.
/// Ours must give the same body either way: two shells (outer 6 faces, cavity 6), V - E + F = 4,
/// and the full checker passing.
#[test]
fn wr_265_a_lid_that_seals_a_cup_gives_a_valid_body_with_a_void() {
    let cube = cuboid([0, 0, 0], [100, 100, 100]);
    let pocket = cuboid([10, 10, 10], [80, 80, 100]);
    let cup = subtract(
        &ctx(),
        &SERIAL,
        key(1),
        &[solid(1, &cube), solid(2, &pocket)],
    )
    .unwrap()
    .result
    .body;
    checked(&cup);
    assert_eq!(sixfold(&cup), 6 * (1_000_000 - 80 * 80 * 90));
    let lid = cuboid([0, 0, 90], [100, 100, 10]);
    for (cup_first, label) in [(true, "cup first"), (false, "lid first")] {
        let operands = if cup_first {
            [solid(1, &cup), solid(2, &lid)]
        } else {
            [solid(2, &lid), solid(1, &cup)]
        };
        let sealed = unite(&ctx(), &SERIAL, &operands).unwrap().result.body;
        assert_eq!(sixfold(&sealed), 6 * 488_000, "{label}");
        checked(&sealed);
        assert_eq!(vef(&sealed), (16, 24, 12), "{label}");
        assert_eq!(shells(&sealed), 2, "{label}");
        assert_eq!(euler(&sealed), 4, "{label}");
    }
}

/// WR-265 (same file): "unions, differences and intersections of a body that already has a void
/// keep the right order". A hollow cube united with a box on its outside, cut by a box that
/// pierces the cavity wall, and cut by a box that stays in the material: each exact and valid.
#[test]
fn wr_265_a_body_that_already_has_a_void_keeps_it_through_every_operation() {
    let h = hollow();
    checked(&h);
    assert_eq!(sixfold(&h), 6 * (1_000_000 - 216_000));
    // Joined on its outside: the cavity is untouched.
    let wing = cuboid([90, 40, 40], [40, 20, 20]);
    let joined = unite(&ctx(), &SERIAL, &[solid(1, &h), solid(2, &wing)])
        .unwrap()
        .result
        .body;
    assert_eq!(sixfold(&joined), 6 * (784_000 + 16_000 - 4_000));
    checked(&joined);
    assert_eq!(shells(&joined), 2);
    assert_eq!(euler(&joined), 4);
    // A bore from outside into the cavity opens it: one shell, no void left but the outside.
    let bore = cuboid([70, 40, 40], [40, 20, 20]);
    let opened = subtract(&ctx(), &SERIAL, key(1), &[solid(1, &h), solid(2, &bore)])
        .unwrap()
        .result
        .body;
    assert_eq!(sixfold(&opened), 6 * (784_000 - 8_000));
    checked(&opened);
    assert_eq!(shells(&opened), 1);
    assert_eq!(euler(&opened), 2);
    // A pocket in the wall that stops short of the cavity: the cavity stays sealed.
    let pocket = cuboid([85, 40, 40], [15, 20, 20]);
    let pocketed = subtract(&ctx(), &SERIAL, key(1), &[solid(1, &h), solid(2, &pocket)])
        .unwrap()
        .result
        .body;
    assert_eq!(sixfold(&pocketed), 6 * (784_000 - 6_000));
    checked(&pocketed);
    assert_eq!(shells(&pocketed), 2);
    // Intersection with a block that holds the cavity: the cavity stays.
    let block = cuboid([10, 10, 10], [80, 80, 80]);
    let part = intersect(&ctx(), &SERIAL, &[solid(1, &h), solid(2, &block)])
        .unwrap()
        .result
        .body;
    assert_eq!(sixfold(&part), 6 * (512_000 - 216_000));
    checked(&part);
    assert_eq!(shells(&part), 2);
    assert_eq!(euler(&part), 4);
}

/// solidcore plan/upstream-issues/WR-442--... and WR-443--... (b4de2149): a closed shell nested
/// in another (a solid lump floating in a hollow) was taken as a void's second shell, and a
/// difference with the hollow body removed what lay in the hollow. Their input is a full
/// rotational sweep, which we do not have yet; the flat counterpart is a hollow cube with a box
/// floating inside its cavity. Union: the lump is a second material region, three shells; the
/// difference leaves the hollow cube alone; the intersection is empty.
#[test]
fn wr_442_wr_443_a_lump_floating_in_a_cavity_is_a_lump_and_a_difference_leaves_it_alone() {
    let h = hollow();
    let floater = cuboid([40, 40, 40], [20, 20, 20]);
    let both = unite(&ctx(), &SERIAL, &[solid(1, &h), solid(2, &floater)])
        .unwrap()
        .result
        .body;
    assert_eq!(sixfold(&both), 6 * (784_000 + 8_000));
    checked(&both);
    assert_eq!(shells(&both), 3);
    assert_eq!(euler(&both), 6);
    let rest = subtract(&ctx(), &SERIAL, key(1), &[solid(1, &h), solid(2, &floater)])
        .unwrap()
        .result
        .body;
    assert_eq!(sixfold(&rest), 6 * 784_000);
    checked(&rest);
    assert_eq!(shells(&rest), 2);
    // The floater less the hollow cube is the floater: the cavity holds it all.
    let kept = subtract(&ctx(), &SERIAL, key(2), &[solid(1, &h), solid(2, &floater)])
        .unwrap()
        .result
        .body;
    assert_eq!(sixfold(&kept), 6 * 8_000);
    checked(&kept);
    assert_eq!(vef(&kept), (8, 12, 6));
    let none = intersect(&ctx(), &SERIAL, &[solid(1, &h), solid(2, &floater)])
        .unwrap()
        .result
        .body;
    assert_eq!(none.arenas().faces.len(), 0);
    // And a hollow cube in a hollow cube's cavity: nesting three deep.
    let inner = {
        let a = cuboid([35, 35, 35], [30, 30, 30]);
        let b = cuboid([40, 40, 40], [20, 20, 20]);
        subtract(&ctx(), &SERIAL, key(1), &[solid(1, &a), solid(2, &b)])
            .unwrap()
            .result
            .body
    };
    let nest = unite(&ctx(), &SERIAL, &[solid(1, &h), solid(2, &inner)])
        .unwrap()
        .result
        .body;
    assert_eq!(sixfold(&nest), 6 * (784_000 + 27_000 - 8_000));
    checked(&nest);
    assert_eq!(shells(&nest), 4);
    assert_eq!(euler(&nest), 8);
}

/// Twice the area of a convex polygon, counter-clockwise.
fn area2(base: &[[i64; 2]]) -> i128 {
    let mut sum = 0i128;
    for (i, a) in base.iter().enumerate() {
        let b = base[(i + 1) % base.len()];
        sum += i128::from(a[0]) * i128::from(b[1]) - i128::from(b[0]) * i128::from(a[1]);
    }
    sum
}

type Run = Result<geom_op::OpOutput<Combined>, geom_op::OpError>;

/// The answer an operation owes: the exact body, or a defined refusal that is not an internal
/// error and not a budget stop. True when it was the body.
fn exact_or_refused(got: Run, want_sixfold: i128, what: &str) -> bool {
    match got {
        Ok(out) => {
            let body = &out.result.body;
            assert_eq!(sixfold(body), want_sixfold, "{what}");
            let report = check(body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
            assert!(report.passed(), "{what}: {report:?}");
            true
        }
        Err(e) => {
            assert!(
                !matches!(e.kind(), ErrorKind::Internal | ErrorKind::BudgetExceeded),
                "{what}: {e:?}"
            );
            false
        }
    }
}

/// solidcore plan/upstream-issues/WR-473--booleans-of-skins-with-coincident-sides-lose-the-shared-section.md
/// (b4de2149): two skins whose side faces coincide between z = 20 and z = 40 (A through z 0..40, B
/// through z 20..60) gave A - B empty, A and B all of A, and a failing union. The skins are
/// curved; the flat counterpart is two hexagonal prisms on one base, z 0..40 and z 20..60. The
/// exact answers: A - B is the lower 20, A and B the shared 20, A or B the whole 60; each a
/// hexagonal prism of 8 faces, 18 edges, 12 vertices.
#[test]
fn wr_473_prisms_with_coincident_sides_share_exactly_the_common_section() {
    let hex = [
        [20, 0],
        [10, 17],
        [-10, 17],
        [-20, 0],
        [-10, -17],
        [10, -17],
    ];
    let area = area2(&hex) / 2;
    assert_eq!(area, 1020);
    let a = column(&hex, 0, 40);
    let b = column(&hex, 20, 40);
    let both = [solid(1, &a), solid(2, &b)];
    let cases = [
        (
            subtract(&ctx(), &SERIAL, key(1), &both).unwrap(),
            20,
            "A - B",
        ),
        (
            subtract(&ctx(), &SERIAL, key(2), &both).unwrap(),
            20,
            "B - A",
        ),
        (intersect(&ctx(), &SERIAL, &both).unwrap(), 20, "A and B"),
        (unite(&ctx(), &SERIAL, &both).unwrap(), 60, "A or B"),
    ];
    for (out, height, what) in cases {
        let body = &out.result.body;
        assert_eq!(sixfold(body), 6 * area * height, "{what}");
        checked(body);
        assert_eq!(vef(body), (12, 18, 8), "{what}");
        assert_eq!(euler(body), 2, "{what}");
    }
    // B inside A's height, on the same sides: A - B is two separate prisms, A and B is B.
    let mid = column(&hex, 10, 20);
    let both = [solid(1, &a), solid(2, &mid)];
    let ends = subtract(&ctx(), &SERIAL, key(1), &both)
        .unwrap()
        .result
        .body;
    assert_eq!(sixfold(&ends), 6 * area * 20);
    checked(&ends);
    assert_eq!(vef(&ends), (24, 36, 16));
    assert_eq!(shells(&ends), 2);
    let common = intersect(&ctx(), &SERIAL, &both).unwrap().result.body;
    assert_eq!(sixfold(&common), 6 * area * 20);
    assert_eq!(vef(&common), (12, 18, 8));
    let whole = unite(&ctx(), &SERIAL, &both).unwrap().result.body;
    assert_eq!(sixfold(&whole), 6 * area * 40);
    assert_eq!(vef(&whole), (12, 18, 8));
}

/// The parallelogram base whose slanted face has slope 0.1 and starts `k` below y = 200.
fn slanted(k: i64) -> Body {
    column(
        &[
            [180, 200 - k],
            [220, 204 - k],
            [220, 244 - k],
            [180, 240 - k],
        ],
        0,
        40,
    )
}

/// solidcore plan/upstream-issues/WR-614--crossing-of-planes-within-tolerance-across-a-strip-fails-the-merge.md
/// and WR-619--union-across-a-strip-within-tolerance-merges-both-solids-into-the-outside.md
/// (b4de2149): two cubes, one turned atan(0.1) so its face crosses the other's at a small angle
/// and stays within tolerance of it across a narrow strip, with the top and bottom faces
/// coplanar. The union failed (614) or came back with no solid region, volume 0, as a success
/// (619). Ours is exact, so the strip is real: a box A (y up to 200) and a parallelogram prism B
/// whose slanted face has slope 0.1 and starts `k` below A's face. For k = 1 and 2 they overlap in
/// a thin triangle of 5 k^2 (times the height 40); the answers are exact and the checker passes,
/// for all four operations.
#[test]
fn wr_614_wr_619_a_slanted_face_across_a_narrow_strip_gives_exact_volumes() {
    let a = cuboid([-200, 160, 0], [400, 40, 40]);
    let a_volume = 400 * 40 * 40;
    for k in [1i64, 2] {
        let b = slanted(k);
        let b_volume = 40 * 40 * 40;
        let shared = 5 * i128::from(k * k) * 40;
        let both = [solid(1, &a), solid(2, &b)];
        let u = unite(&ctx(), &SERIAL, &both).unwrap().result.body;
        assert_eq!(sixfold(&u), 6 * (a_volume + b_volume - shared), "k = {k}");
        checked(&u);
        assert_eq!(regions(&u), (1, 1), "k = {k}: one lump of material");
        let d = subtract(&ctx(), &SERIAL, key(1), &both)
            .unwrap()
            .result
            .body;
        assert_eq!(sixfold(&d), 6 * (a_volume - shared), "k = {k}");
        checked(&d);
        let e = subtract(&ctx(), &SERIAL, key(2), &both)
            .unwrap()
            .result
            .body;
        assert_eq!(sixfold(&e), 6 * (b_volume - shared), "k = {k}");
        checked(&e);
        let i = intersect(&ctx(), &SERIAL, &both).unwrap().result.body;
        assert_eq!(sixfold(&i), 6 * shared, "k = {k}");
        checked(&i);
    }
}

/// WR-614 and WR-619 (same files), the near cases on either side of the strip: the slanted face
/// starts on A's face (k = 0, only an edge of B lies on A's face) or one step above it (k = -1, a
/// gap). The union is the sum of the two volumes; the intersection is empty; the difference
/// leaves A whole. The union at k = 0 is `wr_614_wr_619_union_of_an_edge_lying_in_a_face`.
#[test]
fn wr_614_wr_619_a_slanted_face_that_only_touches_or_just_misses_adds_the_volumes() {
    let a = cuboid([-200, 160, 0], [400, 40, 40]);
    for k in [0i64, -1] {
        let b = slanted(k);
        let both = [solid(1, &a), solid(2, &b)];
        if k == -1 {
            let sum = 6 * (400 * 40 * 40 + 40 * 40 * 40);
            assert!(
                exact_or_refused(unite(&ctx(), &SERIAL, &both), sum, "union"),
                "a gap of one step is two lumps, exactly"
            );
        }
        exact_or_refused(
            subtract(&ctx(), &SERIAL, key(1), &both),
            6 * 400 * 40 * 40,
            "A less B",
        );
        let meet = intersect(&ctx(), &SERIAL, &both).unwrap().result.body;
        assert_eq!(meet.arenas().faces.len(), 0, "k = {k}");
    }
}

/// The same defect class (WR-614, WR-619, WR-727) with B's lower edge lying in the interior of A's
/// face. The union's volume is right, 6 * (A + B), and A's face is split along B's edge, so the
/// edge has four faces and the body passes the full check (it was returned as `Ok` with A's face
/// whole, failing it with `FacesCross`). It is a general body: two lumps joined along an edge.
#[test]

fn wr_614_wr_619_union_of_an_edge_lying_in_a_face() {
    let a = cuboid([-200, 160, 0], [400, 40, 40]);
    let b = slanted(0);
    let both = [solid(1, &a), solid(2, &b)];
    let sum = 6 * (400 * 40 * 40 + 40 * 40 * 40);
    exact_or_refused(unite(&ctx(), &SERIAL, &both), sum, "union");
}

/// solidcore plan/upstream-issues/WR-727--topological-vertex-ignores-a-point-contacts-vertex-loop.md
/// (b4de2149): two solids that touch at one point inside an edge of one of them; the cleanup took
/// the contact vertex for removable, joined the edge through it and moved the other face's vertex
/// to the far end of the edge ("Bad Vertex Gap"), with the volume right. The input there is a
/// sphere on a seam; the flat counterpart is a prism whose apex touches the interior of a box's
/// top edge. The volume is the sum, and the result is a valid body or a defined refusal; it is
/// never a body the checker refuses.
#[test]
fn wr_727_two_solids_that_touch_at_a_point_inside_an_edge_stay_valid() {
    let block = cuboid([0, 0, 0], [100, 100, 100]);
    // The apex (50, 100, 100) is inside the block's top edge from (0, 100, 100) to (100, 100, 100).
    let wedge = column(&[[50, 100], [70, 120], [30, 120]], 100, 40);
    let wedge_volume = 400 * 40;
    let both = [solid(1, &block), solid(2, &wedge)];
    let sum = 6 * (1_000_000 + wedge_volume);
    exact_or_refused(unite(&ctx(), &SERIAL, &both), sum, "union at the edge");
    exact_or_refused(
        subtract(&ctx(), &SERIAL, key(1), &both),
        6 * 1_000_000,
        "block less wedge",
    );
    exact_or_refused(
        subtract(&ctx(), &SERIAL, key(2), &both),
        6 * wedge_volume,
        "wedge less block",
    );
    // The apex on a corner of the block: the same, with a vertex touching a vertex.
    let corner = column(&[[100, 100], [120, 120], [100, 120]], 100, 40);
    let both = [solid(1, &block), solid(2, &corner)];
    exact_or_refused(
        unite(&ctx(), &SERIAL, &both),
        6 * (1_000_000 + 200 * 40),
        "union at the corner",
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
    )
    .unwrap();
    assert_eq!(body.kind(), BodyKind::Sheet);
    checked(&body);
    body
}

fn pieces_of(target: &Body, sheet: &Body) -> Vec<Combined> {
    let input = BooleanInput {
        operands: vec![solid(1, target), cutter(2, sheet)],
        select: vec![Selection::Split { target: key(1) }],
        merge: Merge::RemoveRedundant,
        accept: None,
    };
    combine(&ctx(), &SERIAL, &input)
        .unwrap()
        .result
        .into_iter()
        .next()
        .unwrap()
}

/// solidcore plan/upstream-issues/WR-254--manifoldboolean-keeps-a-face-lying-in-the-slicing-sheet-in.md
/// (b4de2149): a box joined to a slab, sliced by a sheet on the plane of the slab's face (the
/// plane x = -3.5); SMLib kept the face lying in the sheet in both parts, so the box part had 7
/// faces, one outside the box, and failed the check. Ours cuts a box joined to a slab by a sheet
/// on the plane where they meet: two bodies, the box with 6 faces and the slab with 6, both
/// valid, the volumes exact. The sheet is cut again on a plane that is the slab's own face, and
/// on one a step either side of it.
#[test]
fn wr_254_a_sheet_on_the_plane_where_a_box_meets_a_slab_cuts_two_clean_parts() {
    let slab = cuboid([-35, -50, -10], [55, 100, 50]);
    let stud = cuboid([-80, 0, 0], [45, 30, 30]);
    let joined = unite(&ctx(), &SERIAL, &[solid(1, &slab), solid(2, &stud)])
        .unwrap()
        .result
        .body;
    checked(&joined);
    assert_eq!(sixfold(&joined), 6 * (55 * 100 * 50 + 45 * 30 * 30));
    // On the slab's own face: the sheet covers all of it and more.
    let on_face = sheet(0, -35, [-100, -100], [100, 100]);
    let mut parts = pieces_of(&joined, &on_face);
    parts.sort_by_key(|c| sixfold(&c.body));
    assert_eq!(parts.len(), 2);
    assert_eq!(sixfold(&parts[0].body), 6 * 45 * 30 * 30);
    assert_eq!(sixfold(&parts[1].body), 6 * 55 * 100 * 50);
    for part in &parts {
        checked(&part.body);
        assert_eq!(vef(&part.body), (8, 12, 6));
    }
    // One step inside the slab and one step inside the stud: the slab keeps the stud's foot.
    for (at, stud_part, slab_part) in [
        (-34, 45 * 30 * 30 + 100 * 50, 54 * 100 * 50),
        (-36, 44 * 30 * 30, 55 * 100 * 50 + 30 * 30),
    ] {
        let off = sheet(0, at, [-100, -100], [100, 100]);
        let mut parts = pieces_of(&joined, &off);
        parts.sort_by_key(|c| sixfold(&c.body));
        assert_eq!(parts.len(), 2, "plane x = {at}");
        let want = {
            let mut w = [6 * stud_part, 6 * slab_part];
            w.sort_unstable();
            w
        };
        assert_eq!(sixfold(&parts[0].body), want[0], "plane x = {at}");
        assert_eq!(sixfold(&parts[1].body), want[1], "plane x = {at}");
        for c in &parts {
            checked(&c.body);
        }
    }
}

/// solidcore plan/upstream-issues/WR-579--... (bicycle frame tubes ending 3.7e-4 short of each
/// other, united), WR-619 and UB-175 (b4de2149): solids a hair apart, touching, or overlapping by
/// a hair, where SMLib fails, returns no solid, or reads freed proto edges. Ours is exact on the
/// grid, so a hair is one step: two bars end to end, with the gap -1 (overlap of one step), 0
/// (they share a face) and 1 and 2 (apart), each also offset sideways so the faces only partly
/// overlap. Union, both differences and the intersection are the exact sums and overlaps.
#[test]
fn wr_579_wr_619_solids_a_step_apart_touching_or_a_step_into_each_other_are_exact() {
    let a = cuboid([0, 0, 0], [100, 20, 20]);
    for gap in [-1i64, 0, 1, 2] {
        for (dy, dz) in [(0i64, 0i64), (5, 7), (-19, 0), (0, 19)] {
            let b = cuboid([100 + gap, dy, dz], [100, 20, 20]);
            let both = [solid(1, &a), solid(2, &b)];
            // The overlap: a slice of width 1 when gap is -1, cut down by the sideways offset.
            let wide = i128::from((20 - dy.abs()).max(0)) * i128::from((20 - dz.abs()).max(0));
            let overlap = if gap < 0 { wide } else { 0 };
            let each = 100 * 20 * 20;
            let what = format!("gap {gap} offset ({dy}, {dz})");
            let u = unite(&ctx(), &SERIAL, &both).unwrap().result.body;
            assert_eq!(sixfold(&u), 6 * (2 * each - overlap), "{what}");
            checked(&u);
            let d = subtract(&ctx(), &SERIAL, key(1), &both)
                .unwrap()
                .result
                .body;
            assert_eq!(sixfold(&d), 6 * (each - overlap), "{what}");
            checked(&d);
            let e = subtract(&ctx(), &SERIAL, key(2), &both)
                .unwrap()
                .result
                .body;
            assert_eq!(sixfold(&e), 6 * (each - overlap), "{what}");
            checked(&e);
            let i = intersect(&ctx(), &SERIAL, &both).unwrap().result.body;
            assert_eq!(sixfold(&i), 6 * overlap, "{what}");
            if overlap > 0 {
                checked(&i);
            } else {
                assert_eq!(i.arenas().faces.len(), 0, "{what}");
            }
        }
    }
}

/// solidcore plan/upstream-issues/UB-175--tiny-booleans-reach-the-teardown-through-the-public-api.md,
/// HANG-35--closed-shells-a-few-zone-tolerances-across-take-minutes-to.md and
/// WR-209--smapicreatebox-reports-success-for-incomplete-box-topology.md (b4de2149): boxes a few
/// zone tolerances across (SMLib: use after free in the proto topology teardown, minutes to place
/// a shell in its region, a box built without its six faces and reported as a success). Ours
/// counts in grid steps, so the smallest boxes are 1 to 3 steps: every operation of two such
/// boxes overlapping in every way is the exact answer, fast, and valid.
#[test]
fn ub_175_hang_35_wr_209_boxes_a_few_steps_across_boolean_exactly() {
    for size in [1u64, 2, 3] {
        let a = cuboid([0, 0, 0], [size; 3]);
        assert_eq!(vef(&a), (8, 12, 6));
        checked(&a);
        let s = i128::from(size);
        for shift in 0..=(size as i64 + 1) {
            let b = cuboid([shift, 0, 0], [size; 3]);
            let both = [solid(1, &a), solid(2, &b)];
            let common = (s - i128::from(shift)).max(0) * s * s;
            let u = unite(&ctx(), &SERIAL, &both).unwrap().result.body;
            assert_eq!(sixfold(&u), 6 * (2 * s * s * s - common), "{size} {shift}");
            checked(&u);
            let d = subtract(&ctx(), &SERIAL, key(1), &both)
                .unwrap()
                .result
                .body;
            assert_eq!(sixfold(&d), 6 * (s * s * s - common), "{size} {shift}");
            if common < s * s * s {
                checked(&d);
            }
            let i = intersect(&ctx(), &SERIAL, &both).unwrap().result.body;
            assert_eq!(sixfold(&i), 6 * common, "{size} {shift}");
            if common > 0 {
                checked(&i);
            }
        }
    }
}

/// solidcore plan/upstream-issues/ND-258--booleanpostprocess-deletes-intersection-edges-in-pointer.md
/// and ND-2--results-that-depend-on-pointer-keyed-container-order.md (b4de2149): the result's
/// loop starts depended on the heap layout (pointer-hash order), confirmed by moving the heap
/// with dummy allocations before each run. Ours must give the same body, bit for bit, however
/// the heap is laid out: every operation of boxes, a prism and a half-space, run with allocations
/// of many sizes kept alive around it.
#[test]
fn nd_258_nd_2_the_result_does_not_depend_on_where_the_heap_put_things() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([20, 20, 20], [40, 40, 40]);
    let c = column(&[[10, 5], [50, 15], [30, 45]], -5, 80);
    let h = half_space(
        ctx().path(),
        [p(60, 0, 0), p(60, 60, 0), p(0, 0, 60)],
        &mut meter(),
    )
    .unwrap();
    let run = || {
        let three = [solid(1, &a), solid(2, &b), solid(3, &c)];
        vec![
            unite(&ctx(), &SERIAL, &three).unwrap().result.body,
            subtract(&ctx(), &SERIAL, key(1), &three)
                .unwrap()
                .result
                .body,
            intersect(&ctx(), &SERIAL, &[solid(1, &a), solid(4, &h)])
                .unwrap()
                .result
                .body,
            intersect(&ctx(), &SERIAL, &[solid(2, &b), solid(3, &c)])
                .unwrap()
                .result
                .body,
        ]
    };
    let first = run();
    let mut keep: Vec<Vec<u8>> = Vec::new();
    for round in 0..12usize {
        // Move the heap: allocations of different sizes, some kept, some dropped.
        for k in 0..(40 + 17 * round) {
            let block = vec![round as u8; 8 + (k * 37 + round * 101) % 4096];
            if k % 3 != 0 {
                keep.push(block);
            }
        }
        assert_eq!(run(), first, "heap round {round}");
    }
}

/// solidcore plan/upstream-issues/WR-191--smmerge-manifoldboolean-leaves-both-contexts-in-boolean-mode.md
/// (b4de2149): after a boolean failed, every later operation ran in boolean mode, because the flag
/// was cleared only on the success returns. Ours holds no such state: after each way a boolean can
/// refuse (a duplicate operand key, a budget that runs out at every stage) the inputs are
/// unchanged and the next boolean gives the same body as one on a fresh start.
#[test]
fn wr_191_a_refused_boolean_leaves_nothing_behind_for_the_next() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([20, 20, 20], [40, 40, 40]);
    let (a0, b0) = (a.clone(), b.clone());
    let fresh = unite(&ctx(), &SERIAL, &[solid(1, &a), solid(2, &b)])
        .unwrap()
        .result
        .body;
    // A duplicate key is refused.
    let twice = unite(&ctx(), &SERIAL, &[solid(1, &a), solid(1, &b)]);
    assert_eq!(twice.unwrap_err().kind(), ErrorKind::InvalidInput);
    // A budget that runs out, early and late.
    for budget in [0u64, 1, 10, 100, 1_000, 10_000] {
        let small = OpContext::new(
            OpPath::new(OpId::from_bytes([7; 16])),
            Budget::new(budget, u64::MAX),
        );
        match unite(&small, &SERIAL, &[solid(1, &a), solid(2, &b)]) {
            Ok(out) => assert_eq!(out.result.body, fresh),
            Err(e) => assert_eq!(e.kind(), ErrorKind::BudgetExceeded, "{e:?}"),
        }
        assert_eq!(a, a0);
        assert_eq!(b, b0);
        let after = unite(&ctx(), &SERIAL, &[solid(1, &a), solid(2, &b)])
            .unwrap()
            .result
            .body;
        assert_eq!(after, fresh, "after a budget of {budget}");
    }
}

/// B's column (a parallelogram prism like [`slanted`]) standing on A's top face (y = 200) with its
/// lower edge in that face at `x`, from height `z` up by `height`. Its slanted face rises away
/// from A, so the two share only that edge.
fn leaning_on_top(x: i64, z: i64, height: u64) -> Body {
    column(
        &[[x, 200], [x + 40, 204], [x + 40, 244], [x, 240]],
        z,
        height,
    )
}

/// The mirror of [`leaning_on_top`] under A's bottom face (y = 160).
fn leaning_under_bottom(x: i64, z: i64, height: u64) -> Body {
    column(
        &[[x, 160], [x, 120], [x + 40, 116], [x + 40, 156]],
        z,
        height,
    )
}

/// Every operation on A (a 400 by 40 by 40 box) and `bodies`, each touching A along an edge that
/// lies in the interior of one of A's faces. They share no volume, so the union is the sum, A less
/// any of them leaves A whole, any of them less A is itself, and the intersection is empty. The
/// union must pass the full check, with `faces` faces, as the contact edge is an edge of A's face
/// too. Returns the union.
fn edge_in_a_face_all_ways(a: &Body, bodies: &[&Body], faces: usize) -> Body {
    let mut operands = vec![solid(1, a)];
    for (at, body) in bodies.iter().enumerate() {
        operands.push(solid(2 + at as u8, body));
    }
    let a_volume = 6 * 400 * 40 * 40;
    let others: i128 = bodies.iter().map(|body| sixfold(body)).sum();
    let u = unite(&ctx(), &SERIAL, &operands).unwrap().result.body;
    assert_eq!(sixfold(&u), a_volume + others);
    checked(&u);
    assert_eq!(u.arenas().faces.len() as usize, faces);
    let d = subtract(&ctx(), &SERIAL, key(1), &operands)
        .unwrap()
        .result
        .body;
    assert_eq!(sixfold(&d), a_volume);
    checked(&d);
    for (at, body) in bodies.iter().enumerate() {
        let e = subtract(&ctx(), &SERIAL, key(2 + at as u8), &operands)
            .unwrap()
            .result
            .body;
        assert_eq!(sixfold(&e), sixfold(body), "operand {at} less the rest");
        checked(&e);
    }
    let i = intersect(&ctx(), &SERIAL, &operands).unwrap().result.body;
    assert_eq!(i.arenas().faces.len(), 0);
    u
}

/// WR-614, WR-619 and WR-727's class: B's edge lies in the middle of A's face, and the edge runs
/// the face's whole depth. The union is one body of two lumps that touch along that edge: A's top
/// face is split along it, so the edge has four faces (A's two halves and B's two). Difference
/// and intersection are as they were.
#[test]
fn an_edge_in_the_middle_of_a_face_splits_it_in_the_union_only() {
    let a = cuboid([-200, 160, 0], [400, 40, 40]);
    let b = leaning_on_top(0, 0, 40);
    // A's six faces and the top's second half, B's six.
    let u = edge_in_a_face_all_ways(&a, &[&b], 13);
    assert_eq!(vef(&u), (16, 26, 13));
    assert_eq!(regions(&u), (2, 1), "two lumps, joined only along the edge");
}

/// The same with the edge one step in from A's boundary: A's top keeps a strip one step wide.
#[test]
fn an_edge_near_the_boundary_of_a_face_splits_it_in_the_union_only() {
    let a = cuboid([-200, 160, 0], [400, 40, 40]);
    for x in [199, -199] {
        // At -199 B stands over A's left part, a step from its end at -200.
        let b = leaning_on_top(x, 0, 40);
        let u = edge_in_a_face_all_ways(&a, &[&b], 13);
        assert_eq!(vef(&u), (16, 26, 13), "x = {x}");
    }
}

/// An edge in each of two faces of A, the top and the bottom: both split, nothing else changes.
#[test]
fn edges_in_two_faces_split_both_in_the_union_only() {
    let a = cuboid([-200, 160, 0], [400, 40, 40]);
    let above = leaning_on_top(0, 0, 40);
    let below = leaning_under_bottom(60, 0, 40);
    let u = edge_in_a_face_all_ways(&a, &[&above, &below], 20);
    assert_eq!(vef(&u), (24, 40, 20));
}

/// B's edge ends inside A's face, a vertex of B in the face's interior as well: the edge covers
/// only part of the face's depth, so the face would need a slit with no area. The union is a typed
/// refusal, never a body the checker fails; the difference and the intersection are as they were.
#[test]
fn an_edge_that_ends_inside_a_face_is_refused_in_the_union_only() {
    let a = cuboid([-200, 160, 0], [400, 40, 40]);
    let b = leaning_on_top(0, 10, 20);
    let both = [solid(1, &a), solid(2, &b)];
    let refused = unite(&ctx(), &SERIAL, &both).unwrap_err();
    assert_eq!(
        refused.detail(),
        &OpErrorDetail::Combine(CombineDetail::ContactInsideFace)
    );
    assert_eq!(refused.kind(), ErrorKind::Unsupported);
    let d = subtract(&ctx(), &SERIAL, key(1), &both)
        .unwrap()
        .result
        .body;
    assert_eq!(sixfold(&d), sixfold(&a));
    checked(&d);
    let e = subtract(&ctx(), &SERIAL, key(2), &both)
        .unwrap()
        .result
        .body;
    assert_eq!(sixfold(&e), sixfold(&b));
    checked(&e);
    let i = intersect(&ctx(), &SERIAL, &both).unwrap().result.body;
    assert_eq!(i.arenas().faces.len(), 0);
}
