//! Attack tests from SMLib's confirmed defects, batch 3: the exact meshes (`geom-mesh`).
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. SMLib's meshes are
//! polygon bodies of doubles, ours are exact meshes on a grid, so a defect is taken where its
//! cause has a counterpart here: a boolean of solids that share a face, a thin strip or a tiny
//! overlap; a point on a vertex, an edge or a face; a point far from the origin or at the frame's
//! edge; a distance compared with a clearance; a flat or empty point set; a contour through a
//! vertex; a triangulation with a hole that touches its outline. Each test asserts what the
//! kernel owes for that input: the exact answer (every answer here is compared with an oracle
//! written apart from the code), or a defined refusal, and never a panic, a hang or a wrong
//! answer.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::manual_midpoint,
    clippy::type_complexity,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::range_minus_one,
    clippy::similar_names,
    clippy::unreadable_literal,
    reason = "tests: native arithmetic, casts and indexing, an independent oracle written as the formula reads, prose that names defect files, and a failure stops the test"
)]

use std::collections::BTreeSet;

use geom_mesh::boolean::{Outcome, boolean};
use geom_mesh::cdt::{Cdt, CdtError, Site, triangulate};
use geom_mesh::classify::{Classifier, Located};
use geom_mesh::contact::{Closest, closest_feature, point_triangle, triangles};
use geom_mesh::envelope::{Envelope, half_side};
use geom_mesh::exact::build::{Polygons, build};
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::{ExactMesh, MeshError, MeshKind};
use geom_mesh::hull::Hull;
use geom_mesh::minkowski::minkowski;
use geom_mesh::slice::{Cut, point, slice};
use geom_num::distance::MAX_CLEARANCE;
use geom_num::grid::{COORD_MAX, GridStep, P2, P3};
use geom_num::plane::Plane;
use geom_num::wide::I256;
use geom_num::{Exactness, NumError, Sign};
use geom_op::{OpId, OpPath};
use geom_poly::winding::WindingTest;
use geom_run::{Budget, Exec, WorkMeter};

mod shapes;

use shapes::{cuboid, g, soup, source};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 26))
}

fn step() -> OpPath {
    OpPath::new(OpId::from_bytes([7; 16]))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn mesh(input: &Polygons) -> ExactMesh {
    build(input, &Exec::serial(), &mut meter()).unwrap()
}

/// The box from `low` to `high`, its corners given as they are (so it may reach the frame's
/// edge), of one run named by `name`.
fn cube(low: [i64; 3], high: [i64; 3], name: u8) -> ExactMesh {
    let [x0, y0, z0] = low;
    let [x1, y1, z1] = high;
    let mut input = soup(
        vec![
            g(x0, y0, z0),
            g(x1, y0, z0),
            g(x1, y1, z0),
            g(x0, y1, z0),
            g(x0, y0, z1),
            g(x1, y0, z1),
            g(x1, y1, z1),
            g(x0, y1, z1),
        ],
        &[
            &[0, 3, 2, 1],
            &[4, 5, 6, 7],
            &[0, 1, 5, 4],
            &[1, 2, 6, 5],
            &[2, 3, 7, 6],
            &[3, 0, 4, 7],
        ],
    );
    input.runs = vec![source(name)];
    mesh(&input)
}

/// A box of five faces: the floor of `cuboid(4, 4, 4)` is missing, so it has four boundary edges.
fn open_box() -> ExactMesh {
    mesh(&soup(
        vec![
            g(0, 0, 0),
            g(4, 0, 0),
            g(4, 4, 0),
            g(0, 4, 0),
            g(0, 0, 4),
            g(4, 0, 4),
            g(4, 4, 4),
            g(0, 4, 4),
        ],
        &[
            &[4, 5, 6, 7],
            &[0, 1, 5, 4],
            &[1, 2, 6, 5],
            &[2, 3, 7, 6],
            &[3, 0, 4, 7],
        ],
    ))
}

/// A prism of `outline`, counter-clockwise from above, from z = 0 to `height`.
fn prism_of(outline: &[(i64, i64)], height: i64) -> Polygons {
    let n = outline.len() as u32;
    let mut vertices = Vec::new();
    for z in [0, height] {
        for &(x, y) in outline {
            vertices.push(g(x, y, z));
        }
    }
    let mut faces: Vec<Vec<u32>> = vec![(0..n).rev().collect(), (n..2 * n).collect()];
    for i in 0..n {
        let j = (i + 1) % n;
        faces.push(vec![i, j, j + n, i + n]);
    }
    let refs: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
    soup(vertices, &refs)
}

/// The corner of a tetrahedron at `corner`, its three legs `side` long along the axes.
fn tetra(corner: [i64; 3], side: i64) -> Polygons {
    let [x, y, z] = corner;
    soup(
        vec![
            g(x, y, z),
            g(x + side, y, z),
            g(x, y + side, z),
            g(x, y, z + side),
        ],
        &[&[0, 2, 1], &[0, 3, 2], &[0, 1, 3], &[1, 2, 3]],
    )
}

/// The octahedron of the points at distance `r` along each axis.
fn octahedron(r: i64) -> Polygons {
    // 0 and 1 are the points on x (+, -), 2 and 3 on y, 4 and 5 on z.
    let vertices = vec![
        g(r, 0, 0),
        g(-r, 0, 0),
        g(0, r, 0),
        g(0, -r, 0),
        g(0, 0, r),
        g(0, 0, -r),
    ];
    let mut faces: Vec<[u32; 3]> = Vec::new();
    for a in 0..2_u32 {
        for b in 0..2_u32 {
            for c in 0..2_u32 {
                let (x, y, z) = (a, 2 + b, 4 + c);
                // The corner whose three signs are all positive turns x, y, z. Each sign flipped
                // turns the face over.
                if (a + b + c) % 2 == 0 {
                    faces.push([x, y, z]);
                } else {
                    faces.push([x, z, y]);
                }
            }
        }
    }
    let refs: Vec<&[u32]> = faces.iter().map(|f| f.as_slice()).collect();
    soup(vertices, &refs)
}

fn six_volume(m: &ExactMesh) -> I256 {
    match m.volume(&mut meter()).unwrap() {
        Volume::Exact(six) => six,
        Volume::Certified(_) => panic!("every corner here is on the grid"),
    }
}

fn six(v: i128) -> I256 {
    I256::from_i128(6 * v)
}

fn one(operands: &[&ExactMesh], test: WindingTest) -> Outcome {
    boolean(operands, &[test], &step(), &Exec::serial(), &mut meter())
        .unwrap()
        .pop()
        .unwrap()
}

type Box3 = ([i64; 3], [i64; 3]);

/// The cells of a box.
fn cells(b: Box3) -> i128 {
    (0..3)
        .map(|k| i128::from(b.1[k] - b.0[k]))
        .product::<i128>()
}

/// The cells two boxes share.
fn shared(a: Box3, b: Box3) -> i128 {
    (0..3)
        .map(|k| i128::from((a.1[k].min(b.1[k]) - a.0[k].max(b.0[k])).max(0)))
        .product::<i128>()
}

/// Every boolean of two boxes is a closed solid of exactly the volume that counting cells gives.
fn pair_is_exact(a: Box3, b: Box3) {
    let (ma, mb) = (cube(a.0, a.1, 1), cube(b.0, b.1, 2));
    let (va, vb, vi) = (cells(a), cells(b), shared(a, b));
    let united = one(&[&ma, &mb], WindingTest::unite());
    assert_eq!(six_volume(&united.mesh), six(va + vb - vi), "{a:?} {b:?}");
    assert_eq!(united.mesh.kind(), MeshKind::Closed, "{a:?} {b:?}");
    assert!(united.mesh.is_solid(), "{a:?} {b:?}");
    if vi > 0 {
        let met = one(&[&ma, &mb], WindingTest::intersect(2));
        assert_eq!(six_volume(&met.mesh), six(vi), "{a:?} {b:?}");
        assert_eq!(met.mesh.kind(), MeshKind::Closed, "{a:?} {b:?}");
        assert!(met.mesh.is_solid(), "{a:?} {b:?}");
    }
    if vi < va {
        let less = one(&[&ma, &mb], WindingTest::subtract(0));
        assert_eq!(six_volume(&less.mesh), six(va - vi), "{a:?} {b:?}");
        assert_eq!(less.mesh.kind(), MeshKind::Closed, "{a:?} {b:?}");
        assert!(less.mesh.is_solid(), "{a:?} {b:?}");
    }
    if vi < vb {
        let less = one(&[&ma, &mb], WindingTest::subtract(1));
        assert_eq!(six_volume(&less.mesh), six(vb - vi), "{a:?} {b:?}");
        assert_eq!(less.mesh.kind(), MeshKind::Closed, "{a:?} {b:?}");
        assert!(less.mesh.is_solid(), "{a:?} {b:?}");
    }
}

/// solidcore plan/upstream-issues/WR-473--booleans-of-skins-with-coincident-sides-lose-the-shared-section.md
/// (b4de2149), confirmed by a C++ run: two solids whose side faces coincide over part of their
/// height (one from z = 0 to 40, the other from z = 20 to 60) gave a difference that was empty,
/// an intersection that was all of the first, and a union that failed, because the section
/// where the second side starts was never cut into the first side. Ours must give the exact
/// volume of each, a closed solid every time, whichever of the sides coincide.
#[test]
fn wr_473_solids_with_coincident_sides_keep_the_shared_section() {
    let a: Box3 = ([0, 0, 0], [12, 8, 40]);
    // All four sides flush, the second reaching 20 past the first.
    pair_is_exact(a, ([0, 0, 20], [12, 8, 60]));
    // Three sides flush (x = 0, y = 0 and x = 12), the fourth inside the first.
    pair_is_exact(a, ([0, 0, 20], [12, 4, 60]));
    // Two sides flush.
    pair_is_exact(a, ([0, 0, 20], [6, 4, 60]));
    // One side flush, and the pieces the other way round.
    pair_is_exact(a, ([0, 2, 20], [12, 6, 60]));
    pair_is_exact(([0, 0, 20], [12, 8, 60]), a);
    // Flush sides and a flush end: the second inside the first, touching three of its faces.
    pair_is_exact(a, ([0, 0, 0], [12, 4, 20]));
}

/// solidcore plan/upstream-issues/WR-619--union-across-a-strip-within-tolerance-merges-both-solids-into-the-outside.md
/// (b4de2149), confirmed by a C++ run: two solids whose faces cross at a small angle and stay
/// within tolerance of each other across a narrow strip, with coplanar tops and bottoms, gave a
/// union with no solid region and volume 0, reported as success. The strip here is one step
/// across at its widest: one box, and a prism whose side meets the box's wall at an angle of one
/// part in a thousand over 1000 steps. Ours must give the exact volumes (the overlap is a
/// triangle of 500 square steps, 10 high).
#[test]
fn wr_619_a_union_across_a_narrow_strip_is_the_exact_solid() {
    let a = cube([0, 0, 0], [1000, 1000, 10], 1);
    let mut input = prism_of(&[(0, 999), (2000, 1001), (2000, 3000), (0, 3000)], 10);
    input.runs = vec![source(2)];
    let b = mesh(&input);
    // 4,000,000 square steps for the prism, 1,000,000 for the box, 500 shared.
    let (va, vb, vi) = (10_000_000_i128, 40_000_000_i128, 5_000_i128);
    assert_eq!(six_volume(&a), six(va));
    assert_eq!(six_volume(&b), six(vb));
    let united = one(&[&a, &b], WindingTest::unite());
    assert_eq!(six_volume(&united.mesh), six(va + vb - vi));
    assert_eq!(united.mesh.kind(), MeshKind::Closed);
    assert!(united.mesh.is_solid());
    let met = one(&[&a, &b], WindingTest::intersect(2));
    assert_eq!(six_volume(&met.mesh), six(vi));
    assert!(met.mesh.is_solid());
    let a_less = one(&[&a, &b], WindingTest::subtract(0));
    assert_eq!(six_volume(&a_less.mesh), six(va - vi));
    assert!(a_less.mesh.is_solid());
    let b_less = one(&[&a, &b], WindingTest::subtract(1));
    assert_eq!(six_volume(&b_less.mesh), six(vb - vi));
    assert!(b_less.mesh.is_solid());
}

/// solidcore plan/upstream-issues/WR-79--small-closed-intersection-loops-are-missed-or-collapsed-to.md
/// (b4de2149), confirmed by a C++ run: a closed intersection loop far above the tolerance but
/// tiny (a sphere dipping just into a plane) was missed or came back as one point, so the overlap
/// was lost. On a grid the smallest overlap is one step: two boxes sharing a corner cell, a slab
/// one step thick, and a puck one step deep cut out of a face. Ours must keep every one of them,
/// exactly.
#[test]
fn wr_79_the_smallest_overlaps_are_kept_exactly() {
    let big: Box3 = ([0, 0, 0], [1000, 1000, 1000]);
    // One cell at a corner.
    pair_is_exact(big, ([999, 999, 999], [2000, 2000, 2000]));
    // A slab one step thick, across a whole face.
    pair_is_exact(big, ([999, 0, 0], [2000, 1000, 1000]));
    pair_is_exact(big, ([0, 0, 999], [1000, 1000, 2000]));
    // A puck one step deep and 100 by 100, standing in a face.
    pair_is_exact(big, ([400, 400, 999], [500, 500, 1500]));
    // A rod one step square, through the middle of the box.
    pair_is_exact(big, ([500, 500, -5], [501, 501, 1005]));
}

/// solidcore plan/upstream-issues/UB-175--tiny-booleans-reach-the-teardown-through-the-public-api.md
/// (b4de2149), with the C++ run under AddressSanitizer in its addendum
/// UB-175--addendum-2026-10-26-asan-confirms-api-brep-guard.md: booleans of tiny operands crashed
/// in the cleanup of the intersection tables. Ours, on boxes a step or two across: every
/// boolean of identical unit boxes, of unit boxes that touch, and of two that share one cell
/// must come out whole and exact.
#[test]
fn ub_175_booleans_of_tiny_operands_come_out_whole() {
    let unit: Box3 = ([0, 0, 0], [1, 1, 1]);
    // The same box twice: every face coincides.
    pair_is_exact(unit, unit);
    // Touching on a face, then offset on it.
    pair_is_exact(unit, ([1, 0, 0], [2, 1, 1]));
    pair_is_exact(unit, ([0, 0, 1], [1, 1, 2]));
    // Two steps across, sharing one cell.
    pair_is_exact(([0, 0, 0], [2, 2, 2]), ([1, 1, 1], [3, 3, 3]));
    // A unit box inside a box of two, on a corner and on an edge.
    pair_is_exact(([0, 0, 0], [2, 2, 2]), unit);
    pair_is_exact(([0, 0, 0], [2, 2, 2]), ([1, 0, 0], [2, 1, 1]));
}

/// solidcore plan/upstream-issues/WR-360--smmerge-manifoldboolean-leaves-a-slit-loop-where-solids-touch.md
/// (b4de2149), reported on SolidCraft's fork and not reproduced in a C++ run here: a union of
/// solids that touch left a slit loop where they met. Ours: solids that touch on a whole face, on
/// part of a face and on a face with an offset must unite into one closed solid with no slit, and
/// subtracting one that only touches must leave the other whole.
#[test]
fn wr_360_solids_that_touch_leave_no_slit() {
    // A whole face on a whole face.
    pair_is_exact(([0, 0, 0], [8, 8, 8]), ([8, 0, 0], [16, 8, 8]));
    // A smaller face on a bigger one, inside it.
    pair_is_exact(([0, 0, 0], [8, 8, 8]), ([8, 2, 2], [12, 6, 6]));
    // Offset, so the faces overlap in a part of each.
    pair_is_exact(([0, 0, 0], [8, 8, 8]), ([8, 4, 4], [16, 12, 12]));
    // Touching from below, and from the side the other way round.
    pair_is_exact(([0, 0, 0], [8, 8, 8]), ([2, 2, -4], [6, 6, 0]));
    pair_is_exact(([8, 2, 2], [12, 6, 6]), ([0, 0, 0], [8, 8, 8]));
}

/// solidcore plan/upstream-issues/ND-258--booleanpostprocess-deletes-intersection-edges-in-pointer.md,
/// ND-2--addendum-2026-10-30-boolean-post-process-vertices.md and
/// ND-261--shell-of-rounded-box-depends-on-heap-layout.md (b4de2149), each confirmed by a C++ run:
/// a boolean's result depended on pointer order and heap layout, so one call on one input gave
/// different bodies in different runs. Ours: the same call twice gives the same outcome in every
/// field, and three overlapping solids in any of the six orders give the same volume, face count
/// and vertex count, equal to counting cells.
#[test]
fn nd_258_booleans_do_not_depend_on_order_or_repetition() {
    let boxes: [Box3; 3] = [
        ([0, 0, 0], [6, 6, 6]),
        ([3, 3, 3], [9, 9, 9]),
        ([6, 0, 2], [12, 4, 8]),
    ];
    let meshes: Vec<ExactMesh> = boxes
        .iter()
        .zip(1..)
        .map(|(b, n)| cube(b.0, b.1, n))
        .collect();
    let mut want = 0_i128;
    for x in 0..14 {
        for y in 0..14 {
            for z in 0..14 {
                let inside = boxes.iter().any(|(l, h)| {
                    x >= l[0] && x < h[0] && y >= l[1] && y < h[1] && z >= l[2] && z < h[2]
                });
                want += i128::from(inside);
            }
        }
    }
    let refs: Vec<&ExactMesh> = meshes.iter().collect();
    assert_eq!(
        one(&refs, WindingTest::unite()),
        one(&refs, WindingTest::unite())
    );
    let mut seen: Vec<(I256, usize, usize)> = Vec::new();
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let ordered: Vec<&ExactMesh> = order.iter().map(|&i| &meshes[i]).collect();
        let united = one(&ordered, WindingTest::unite());
        assert_eq!(six_volume(&united.mesh), six(want), "{order:?}");
        assert!(united.mesh.is_solid(), "{order:?}");
        seen.push((
            six_volume(&united.mesh),
            united.mesh.faces(),
            united.mesh.parts().verts.len(),
        ));
    }
    assert!(seen.windows(2).all(|w| w[0] == w[1]), "{seen:?}");
}

/// Where a point of a tetrahedron's corner at the origin, legs `side` long, is.
fn tetra_located(side: i64, i: i64, j: i64, k: i64) -> Located {
    if i < 0 || j < 0 || k < 0 || i + j + k > side {
        Located::Winding(0)
    } else if i == 0 || j == 0 || k == 0 || i + j + k == side {
        Located::OnSurface
    } else {
        Located::Winding(1)
    }
}

/// Every grid point round a tetrahedron at `origin`, by its three planes and its slanted one.
fn tetra_is_located(origin: [i64; 3], side: i64) {
    let m = mesh(&tetra(origin, side));
    let c = Classifier::new(&m, &Exec::serial(), &mut meter()).unwrap();
    let mut seen = [0_usize; 3];
    for i in -3..=side + 3 {
        for j in -3..=side + 3 {
            for k in -3..=side + 3 {
                let q = p(origin[0] + i, origin[1] + j, origin[2] + k);
                let want = tetra_located(side, i, j, k);
                assert_eq!(
                    c.locate(q, &mut meter()).unwrap(),
                    want,
                    "{q:?} at {origin:?}"
                );
                match want {
                    Located::Winding(0) => seen[0] += 1,
                    Located::Winding(_) => seen[1] += 1,
                    Located::OnSurface => seen[2] += 1,
                }
            }
        }
    }
    assert!(seen.iter().all(|&n| n > 100), "{seen:?}");
}

/// solidcore plan/upstream-issues/WR-537--point-on-a-closed-face-seam-classifies-unknown.md
/// (b4de2149, no C++ run): a point on a closed face's seam came back unknown. Ours has no seams,
/// but a point on a mesh's edge, on its vertex, or on a face's plane outside the face is the same
/// kind of case. Every grid point round a tetrahedron, whose slanted face, edges and corners
/// the axis rays of its neighbours run through, must be told exactly: on the surface on a face,
/// edge or corner, and outside on the extension of a face's plane.
#[test]
fn wr_537_points_on_edges_corners_and_face_planes_are_told_exactly() {
    tetra_is_located([0, 0, 0], 12);
    tetra_is_located([-5, 7, 2], 9);
}

/// solidcore plan/upstream-issues/WR-512--singular-start-inside-direction.md and its addendum
/// WR-512--addendum-2026-11-03-knife-edge-part-confirmed.md (b4de2149; the addendum is
/// confirmed by a C++ run): a curve started along a direction that led into neither domain, at a
/// point where four directions leave, because the choice among the directions at a singular
/// point was wrong. The counterpart in point location is a ray that leaves a point along a
/// direction that runs through vertices and along edges: every grid point round an octahedron,
/// whose six corners have four faces each and whose axes pass through them, must be told by its
/// distance from the centre alone. All of the answers are also the same when asked together.
#[test]
fn wr_512_rays_through_corners_and_along_edges_give_the_right_side() {
    let r = 6;
    let m = mesh(&octahedron(r));
    let c = Classifier::new(&m, &Exec::serial(), &mut meter()).unwrap();
    let mut points = Vec::new();
    let mut want = Vec::new();
    for x in -r - 2..=r + 2 {
        for y in -r - 2..=r + 2 {
            for z in -r - 2..=r + 2 {
                let s = x.abs() + y.abs() + z.abs();
                points.push(p(x, y, z));
                want.push(match s.cmp(&r) {
                    std::cmp::Ordering::Less => Located::Winding(1),
                    std::cmp::Ordering::Equal => Located::OnSurface,
                    std::cmp::Ordering::Greater => Located::Winding(0),
                });
            }
        }
    }
    for (q, w) in points.iter().zip(&want) {
        assert_eq!(c.locate(*q, &mut meter()).unwrap(), *w, "{q:?}");
    }
    let together = c
        .locate_all(&points, &Exec::serial(), &mut meter())
        .unwrap();
    assert_eq!(together, want);
    // Asked again, the same.
    let again = c
        .locate_all(&points, &Exec::serial(), &mut meter())
        .unwrap();
    assert_eq!(again, together);
}

/// solidcore plan/upstream-issues/ND-1--srand-time-null-in-face-sampling-and-shell-classification.md
/// and its addendum ND-1--addendum-2026-11-01-stitch-closure-split-confirmed-on-bull.md
/// (b4de2149; the addendum is confirmed by a C++ run): a point classification seeded its random
/// choices from the clock, so a stitched solid's volume changed from run to run. Ours must give
/// the same answer to the same question, however often it is asked and however the questions are
/// ordered: a mesh is built twice from its faces in two orders and every point, asked in
/// opposite orders, gets one answer from both.
#[test]
fn nd_1_point_location_gives_one_answer_every_time() {
    let forward = mesh(&octahedron(5));
    // The same octahedron, its faces listed the other way round.
    let mut input = octahedron(5);
    let n = input.corners.len() / 3;
    let mut turned = Vec::new();
    for f in (0..n).rev() {
        turned.extend_from_slice(&input.corners[3 * f..3 * f + 3]);
    }
    input.corners = turned;
    let backward = mesh(&input);
    let a = Classifier::new(&forward, &Exec::serial(), &mut meter()).unwrap();
    let b = Classifier::new(&backward, &Exec::serial(), &mut meter()).unwrap();
    let mut points = Vec::new();
    for x in -7..=7 {
        for y in -7..=7 {
            for z in -7..=7 {
                points.push(p(x, y, z));
            }
        }
    }
    let first = a
        .locate_all(&points, &Exec::serial(), &mut meter())
        .unwrap();
    for _ in 0..3 {
        assert_eq!(
            a.locate_all(&points, &Exec::serial(), &mut meter())
                .unwrap(),
            first
        );
    }
    points.reverse();
    let mut second = b
        .locate_all(&points, &Exec::serial(), &mut meter())
        .unwrap();
    second.reverse();
    assert_eq!(second, first);
}

/// solidcore plan/upstream-issues/WR-86--smface-getpointsinface-samples-a-rectangular-face-from-0-not.md
/// (b4de2149), confirmed by a C++ run: sample points of a face whose domain did not start at zero
/// were placed as if it did. Ours: the answer for a body must not depend on where it stands. The
/// same tetrahedron at the origin and a long way out on each axis, then in the far corner of the
/// frame, gives the same answer for every point round it.
#[test]
fn wr_86_a_body_far_from_the_origin_is_classified_as_at_the_origin() {
    tetra_is_located([0, 0, 0], 8);
    tetra_is_located([500_000_000_000, -400_000_000_000, 300_000_000_000], 8);
    tetra_is_located(
        [-1_000_000_000_000, 1_000_000_000_000, -1_000_000_000_000],
        8,
    );
}

/// solidcore plan/upstream-issues/HANG-40--smgrid-step3ddda-steps-along-an-axis-the-ray-does-not-move.md
/// (b4de2149, confirmed by a C++ run in its fuzz addendum), with
/// HANG-29--addendum-2026-11-02-query-face-p1-classify-guard.md (confirmed) and
/// UB-287--heal-infinite-region-ray-fire-reads-a-stale-surface.md (confirmed): a ray fired through
/// a grid with a coordinate of 1e300 never returned, and the ray fired to find the infinite
/// region crashed. Ours: a mesh in the far corner of the frame and one as big as the frame are
/// classified, every point round them, within the meter, and a point of the infinite region far
/// from the mesh is outside.
#[test]
fn hang_40_bodies_at_the_frames_edge_are_classified_and_far_points_are_outside() {
    let m = COORD_MAX;
    // A body in the far corner, queried at the frame's corners too.
    let origin = [m - 20, 3 - m, m - 20];
    tetra_is_located(origin, 8);
    let small = mesh(&tetra(origin, 8));
    let c = Classifier::new(&small, &Exec::serial(), &mut meter()).unwrap();
    for q in [
        p(-m, -m, -m),
        p(m, m, m),
        p(-m, m, -m),
        p(m, -m, m),
        p(0, 0, 0),
        p(m, 3 - m, m),
    ] {
        assert_eq!(
            c.locate(q, &mut meter()).unwrap(),
            Located::Winding(0),
            "{q:?}"
        );
    }
    // A body as big as the frame: every point of it is inside or on its surface.
    let all = cube([-m; 3], [m; 3], 1);
    let c = Classifier::new(&all, &Exec::serial(), &mut meter()).unwrap();
    for x in [-m, 1 - m, 0, m - 1, m] {
        for y in [-m, 1 - m, 0, m - 1, m] {
            for z in [-m, 1 - m, 0, m - 1, m] {
                let on = x.abs() == m || y.abs() == m || z.abs() == m;
                let want = if on {
                    Located::OnSurface
                } else {
                    Located::Winding(1)
                };
                assert_eq!(
                    c.locate(p(x, y, z), &mut meter()).unwrap(),
                    want,
                    "{x} {y} {z}"
                );
            }
        }
    }
}

/// solidcore plan/upstream-issues/WR-51--ispointonstartpoint-compares-squared-distances-with-a-length.md
/// (b4de2149), confirmed by a C++ run: a squared distance was compared with a length, so a
/// tolerance of 1e-6 accepted points 1e-3 apart. Ours: a clearance is a length. A point 3 from a
/// face, 5 from an edge and 5 from a corner of a triangle must be nearer than a clearance of
/// 4 and 9 and 25 only where its distance is below it, equal at exactly its distance, and farther
/// for the less. A squared distance read as a length would call 3 against 9 equal, and 5 against
/// 25 equal.
#[test]
fn wr_51_a_clearance_is_a_length_not_a_squared_length() {
    let t = [p(0, 0, 0), p(100, 0, 0), p(0, 100, 0)];
    let cases = [
        (p(10, 10, 3), Closest::Face, 3_u64),
        (p(50, -4, 3), Closest::Edge(0), 5),
        (p(-3, 0, -4), Closest::Vertex(0), 5),
        (p(103, -4, 0), Closest::Vertex(1), 5),
    ];
    for (q, feature, d) in cases {
        assert_eq!(closest_feature(q, t), Ok(feature), "{q:?}");
        let sign = |clearance: u64| point_triangle(q, t, clearance);
        assert_eq!(sign(d), Ok(Sign::Zero), "{q:?} at its own distance");
        assert_eq!(sign(d - 1), Ok(Sign::Positive), "{q:?} just inside");
        assert_eq!(sign(d + 1), Ok(Sign::Negative), "{q:?} just outside");
        assert_eq!(sign(0), Ok(Sign::Positive), "{q:?} at no clearance");
        assert_eq!(sign(d * d), Ok(Sign::Negative), "{q:?} against its square");
        assert_eq!(sign(1000), Ok(Sign::Negative), "{q:?} far");
    }
}

/// solidcore plan/upstream-issues/WR-58--smpointclassification-getgap3d-returns-sm-err-as-a-distance.md
/// (b4de2149; its addendum WR-58--addendum-2026-11-01-point-class-gap3d-error.md is confirmed by
/// a C++ run) and WR-7--smpseudobox-distancetopoint-returns-a-status-as-a-distance.md (no C++
/// run): a failure was returned as the number 1001.0, a distance that callers read as a large
/// gap. Ours: a triangle with no area and a clearance past the limit are errors, never a
/// sign, and the largest clearance is accepted and answered.
#[test]
fn wr_58_a_failed_distance_is_an_error_never_a_sign() {
    let q = p(1, 1, 1);
    let on_a_line = [p(0, 0, 0), p(5, 0, 0), p(10, 0, 0)];
    let one_point = [p(2, 2, 2); 3];
    for flat in [on_a_line, one_point] {
        assert!(
            matches!(point_triangle(q, flat, 3), Err(NumError::Degenerate)),
            "{flat:?}"
        );
        assert!(
            matches!(point_triangle(q, flat, 0), Err(NumError::Degenerate)),
            "{flat:?}"
        );
    }
    let t = [p(0, 0, 0), p(100, 0, 0), p(0, 100, 0)];
    assert!(matches!(triangles(on_a_line, t), Err(NumError::Degenerate)));
    assert!(matches!(triangles(t, one_point), Err(NumError::Degenerate)));
    let above = p(1, 1, 5);
    assert!(matches!(
        point_triangle(above, t, MAX_CLEARANCE + 1),
        Err(NumError::OutOfRange { .. })
    ));
    assert!(matches!(
        point_triangle(above, t, u64::MAX),
        Err(NumError::OutOfRange { .. })
    ));
    assert_eq!(point_triangle(above, t, MAX_CLEARANCE), Ok(Sign::Negative));
    // The far corners of the frame, the widest gap a clearance has to hold.
    let far = [
        p(-COORD_MAX, -COORD_MAX, -COORD_MAX),
        p(-COORD_MAX + 1, -COORD_MAX, -COORD_MAX),
        p(-COORD_MAX, -COORD_MAX + 1, -COORD_MAX),
    ];
    let opposite = p(COORD_MAX, COORD_MAX, COORD_MAX);
    assert_eq!(
        point_triangle(opposite, far, MAX_CLEARANCE),
        Ok(Sign::Negative)
    );
    assert_eq!(point_triangle(opposite, far, 1), Ok(Sign::Positive));
}

/// solidcore plan/upstream-issues/WR-51--ispointonstartpoint-compares-squared-distances-with-a-length.md
/// (b4de2149, confirmed by a C++ run), at the polyhedral envelope: its width is a length too.
/// For widths 1, 2, 10 and 100 round a triangle, a point farther than the width from the triangle
/// is never inside, and a point within the cube's half side of it always is. A width read as a
/// squared length would admit points far past it.
#[test]
fn wr_51_an_envelope_never_admits_a_point_past_its_width() {
    let t = [p(0, 0, 0), p(100, 0, 0), p(0, 100, 0)];
    let depths: [i64; 24] = [
        0, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, -1, -2, -3, -5, -8, -13, -21, -34, -55, -89,
        -144, 233,
    ];
    for epsilon in [1_u64, 2, 10, 100] {
        let h = half_side(epsilon);
        let env = Envelope::new(&[t], epsilon, &Exec::serial(), &mut meter()).unwrap();
        assert_eq!(env.half_side(), h);
        let (mut near, mut far) = (0_usize, 0_usize);
        for x in (-20..=120).step_by(5) {
            for y in (-20..=120).step_by(5) {
                for z in depths {
                    let q = p(x, y, z);
                    let inside = env.contains(q, &mut meter()).unwrap();
                    if point_triangle(q, t, epsilon) == Ok(Sign::Positive) {
                        far += 1;
                        assert!(!inside, "{q:?} is past {epsilon} but inside");
                    }
                    if point_triangle(q, t, h) != Ok(Sign::Positive) {
                        near += 1;
                        assert!(inside, "{q:?} is within {h} but outside {epsilon}");
                    }
                }
            }
        }
        assert!(near > 20 && far > 100, "{epsilon}: {near} {far}");
    }
}

/// solidcore plan/upstream-issues/UB-41--smpointset3d-gettightpbox-on-an-empty-planar-set-reads-m.md
/// (b4de2149, confirmed by a C++ run: a segfault), WR-14--smpointset3d-setuptightpseudobox-discards-its-projections.md
/// (confirmed) and UB-40--smgu-polygonarea-with-no-points-reads-spoints-1.md (no C++ run): the
/// tight box of an empty planar point set read the first point of nothing, a nearly planar set
/// was told apart from a planar one by noise, and the area of no points read before the array.
/// Ours: the hull of no points, of one, of points on a line, of points on a plane and of points
/// one step off a plane are each answered exactly, with a solid made only of the last.
#[test]
fn ub_41_hulls_of_empty_flat_and_nearly_flat_sets_are_answered_exactly() {
    let exec = Exec::serial();
    let none = Hull::of(&[], &mut meter()).unwrap();
    assert!(none.points().is_empty() && none.faces().is_empty() && !none.is_solid());
    assert!(matches!(
        none.mesh(source(1), &exec, &mut meter()),
        Err(MeshError::NotClosed)
    ));
    // One point, however often it is given.
    let lone = Hull::of(&[p(3, 4, 5), p(3, 4, 5), p(3, 4, 5)], &mut meter()).unwrap();
    assert_eq!(lone.points(), [p(3, 4, 5)]);
    assert!(lone.faces().is_empty() && !lone.is_solid());
    // Points on one line: its two ends.
    let line = Hull::of(
        &[p(2, 2, 2), p(0, 0, 0), p(5, 5, 5), p(1, 1, 1), p(3, 3, 3)],
        &mut meter(),
    )
    .unwrap();
    assert_eq!(line.points(), [p(0, 0, 0), p(5, 5, 5)]);
    assert!(line.faces().is_empty() && !line.is_solid());
    // Points on one plane, one of them in the middle of a face and one on an edge: one face of
    // the four corners.
    let flat = Hull::of(
        &[
            p(0, 0, 0),
            p(8, 0, 0),
            p(4, 4, 0),
            p(8, 8, 0),
            p(4, 0, 0),
            p(0, 8, 0),
        ],
        &mut meter(),
    )
    .unwrap();
    assert_eq!(flat.points().len(), 4);
    assert_eq!(flat.faces().len(), 1);
    assert!(!flat.is_solid());
    assert!(matches!(
        flat.mesh(source(1), &exec, &mut meter()),
        Err(MeshError::NotClosed)
    ));
    // One step off the plane: a solid of four faces, whose volume is the determinant, 10^6.
    let nearly = Hull::of(
        &[p(0, 0, 0), p(1000, 0, 0), p(0, 1000, 0), p(1000, 1000, 1)],
        &mut meter(),
    )
    .unwrap();
    assert!(nearly.is_solid());
    assert_eq!(nearly.faces().len(), 4);
    assert_eq!(nearly.points().len(), 4);
    let solid = nearly.mesh(source(1), &exec, &mut meter()).unwrap();
    assert_eq!(six_volume(&solid), I256::from_i128(1_000_000));
    // The box's corners with points inside and on its faces: the eight corners, in any order.
    let mut points = Vec::new();
    for x in 0..=4 {
        for y in 0..=4 {
            for z in 0..=4 {
                points.push(p(x, y, z));
            }
        }
    }
    let boxed = Hull::of(&points, &mut meter()).unwrap();
    assert!(boxed.is_solid());
    assert_eq!(boxed.points().len(), 8);
    assert_eq!(boxed.faces().len(), 6);
    points.reverse();
    assert_eq!(Hull::of(&points, &mut meter()).unwrap(), boxed);
}

/// solidcore plan/upstream-issues/UB-41--smpointset3d-gettightpbox-on-an-empty-planar-set-reads-m.md
/// (b4de2149, confirmed by a C++ run), at the plane slice: a plane that meets nothing, one that
/// touches the mesh only at its lowest and highest points, and one through the middle of a corner
/// are each answered exactly, an empty list of planes is an empty answer, and planes out of order
/// are refused. An octahedron of corners at distance 6: a plane at -7 or -6 or 6 or 7 has no
/// contour (a corner on the plane counts as above it, and a tip closes to nothing); one at 0 is
/// the square through its four corners; one at 3 is the diamond of half-diagonal 3, each turning
/// counter-clockwise round the solid.
#[test]
fn ub_41_a_slice_through_nothing_a_tip_and_a_corner_is_exact() {
    let m = mesh(&octahedron(6));
    let at_z = |h: i64| Plane::new([0, 0, 1], i128::from(-h)).unwrap();
    let heights = [-7, -6, 0, 3, 6, 7];
    let planes: Vec<Plane> = heights.iter().map(|&h| at_z(h)).collect();
    let layers = slice(&m, &planes, &Exec::serial(), &mut meter()).unwrap();
    let counts: Vec<usize> = layers.iter().map(Vec::len).collect();
    assert_eq!(counts, [0, 0, 1, 1, 0, 0]);
    let ring = |layer: usize, h: i64| -> Vec<(i64, i64, i64)> {
        layers[layer][0]
            .cuts()
            .iter()
            .map(|&cut: &Cut| {
                let q = point(&m, &at_z(h), cut).unwrap().to_grid().unwrap();
                (q.x.get(), q.y.get(), q.z.get())
            })
            .collect()
    };
    let twice_area = |ring: &[(i64, i64, i64)]| -> i128 {
        let mut sum = 0_i128;
        for (i, a) in ring.iter().enumerate() {
            let b = ring[(i + 1) % ring.len()];
            sum += i128::from(a.0) * i128::from(b.1) - i128::from(b.0) * i128::from(a.1);
        }
        sum
    };
    let equator = ring(2, 0);
    let mut sorted = equator.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, [(-6, 0, 0), (0, -6, 0), (0, 6, 0), (6, 0, 0)]);
    assert_eq!(twice_area(&equator), 144);
    let diamond = ring(3, 3);
    let mut sorted = diamond.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, [(-3, 0, 3), (0, -3, 3), (0, 3, 3), (3, 0, 3)]);
    assert_eq!(twice_area(&diamond), 36);
    // No planes: no answer, and no error.
    assert!(
        slice(&m, &[], &Exec::serial(), &mut meter())
            .unwrap()
            .is_empty()
    );
    // Out of order, repeated and not parallel.
    for bad in [
        vec![at_z(3), at_z(1)],
        vec![at_z(1), at_z(1)],
        vec![at_z(1), Plane::new([1, 0, 0], -2).unwrap()],
    ] {
        assert!(matches!(
            slice(&m, &bad, &Exec::serial(), &mut meter()),
            Err(MeshError::SlicesOutOfOrder)
        ));
    }
}

/// solidcore plan/upstream-issues/UB-189--smface-computelooporientation-reads-an-empty-array-for-an.md
/// (b4de2149, no C++ run): a face with a loop that held nothing was read past its start, because
/// the caller built a body that did not close. The counterpart here is a surface with a hole in
/// it, a box missing its floor. Every query that needs an inside must refuse it, by name and with
/// a remedy, and never answer, panic or loop: the classifier, the slicer, the boolean and the
/// Minkowski sum.
#[test]
fn ub_189_a_surface_that_does_not_close_is_refused_by_every_query_that_needs_an_inside() {
    let open = open_box();
    assert_eq!(open.kind(), MeshKind::Open);
    let closed = cube([0, 0, 0], [4, 4, 4], 2);
    assert!(matches!(
        Classifier::new(&open, &Exec::serial(), &mut meter()),
        Err(MeshError::NotClosed)
    ));
    let plane = Plane::new([0, 0, 1], -2).unwrap();
    assert!(matches!(
        slice(&open, &[plane], &Exec::serial(), &mut meter()),
        Err(MeshError::NotClosed)
    ));
    assert!(matches!(
        boolean(
            &[&open, &closed],
            &[WindingTest::unite()],
            &step(),
            &Exec::serial(),
            &mut meter()
        ),
        Err(MeshError::NotClosed)
    ));
    for (a, b) in [(&open, &closed), (&closed, &open)] {
        assert!(matches!(
            minkowski(
                a,
                b,
                (source(9), GridStep::Q0),
                &Exec::serial(),
                &mut meter()
            ),
            Err(MeshError::NotClosed)
        ));
    }
}

/// solidcore plan/upstream-issues/WR-342--inset-face-clipped-inside-out.md (b4de2149, confirmed
/// on the fork's C++ build) and UB-35--mappointtovoxel-converts-negative-doubles-to-ulong.md
/// (confirmed in batch 1): a face grown by a tool came out inside out, and a point past the edge
/// of a range wrapped. The counterpart in a Minkowski sum, which is a body grown by a tool: a box
/// grown by a unit box is exactly one step larger on every side of its positive side, a convex body
/// grown by itself is exactly twice as large, and a sum that leaves the frame is refused as out of
/// range, never wrapped.
#[test]
fn wr_342_a_body_grown_by_a_tool_is_exact_and_leaving_the_frame_is_refused() {
    let sum = |a: &ExactMesh, b: &ExactMesh| {
        minkowski(
            a,
            b,
            (source(9), GridStep::Q0),
            &Exec::serial(),
            &mut meter(),
        )
    };
    let big = mesh(&cuboid(4, 4, 4));
    let unit = mesh(&cuboid(1, 1, 1));
    let grown = sum(&big, &unit).unwrap();
    assert_eq!(grown.class, Exactness::Exact);
    assert_eq!(grown.mesh.kind(), MeshKind::Closed);
    assert_eq!(grown.mesh.faces(), 6);
    assert_eq!(six_volume(&grown.mesh), six(125));
    // The tetrahedron of legs 6 grown by itself is the one of legs 12: 8 times the volume, 36.
    let a = mesh(&tetra([0, 0, 0], 6));
    let b = mesh(&tetra([0, 0, 0], 6));
    let twice = sum(&a, &b).unwrap();
    assert_eq!(twice.class, Exactness::Exact);
    assert_eq!(twice.mesh.faces(), 4);
    assert_eq!(six_volume(&twice.mesh), six(8 * 36));
    // At the frame's edge: a sum that stays inside is exact, one that leaves is refused.
    let m = COORD_MAX;
    let corner = cube([m - 4; 3], [m; 3], 1);
    let tool = cube([0; 3], [10; 3], 2);
    assert!(matches!(
        sum(&corner, &tool),
        Err(MeshError::Num(NumError::OutOfRange { .. }))
    ));
    let inside = cube([m - 14; 3], [m - 10; 3], 1);
    let stays = sum(&inside, &tool).unwrap();
    assert_eq!(six_volume(&stays.mesh), six(14 * 14 * 14));
}

/// Twice the area of triangle `t` of `c`, signed.
fn twice_area(c: &Cdt<u32>, t: [u32; 3]) -> i128 {
    let [a, b, d] = t.map(|v| c.vertices[v as usize].at);
    let (ax, ay) = (i128::from(a.x.get()), i128::from(a.y.get()));
    (i128::from(b.x.get()) - ax) * (i128::from(d.y.get()) - ay)
        - (i128::from(b.y.get()) - ay) * (i128::from(d.x.get()) - ax)
}

fn xy(c: &Cdt<u32>, v: u32) -> (i64, i64) {
    let a = c.vertices[v as usize].at;
    (a.x.get(), a.y.get())
}

fn site_list(points: &[(i64, i64)]) -> Vec<Site<u32>> {
    points
        .iter()
        .zip(0_u32..)
        .map(|(&(x, y), key)| Site {
            at: P2::new(x, y).unwrap(),
            key,
        })
        .collect()
}

type Segment = ((i64, i64), (i64, i64));

fn ordered(s: Segment) -> Segment {
    (s.0.min(s.1), s.0.max(s.1))
}

/// The triangles of `points` and `constraints` tile their hull of twice-area `hull_twice` with no
/// flat triangle, every piece of every constraint is an edge of a triangle, and the pieces are
/// exactly `pieces`.
fn triangulates_into(
    points: &[(i64, i64)],
    constraints: &[[u32; 2]],
    pieces: &[Segment],
    hull_twice: i128,
) -> Cdt<u32> {
    let c = triangulate(&site_list(points), constraints, None, &mut meter()).unwrap();
    let mut total = 0_i128;
    for &t in &c.triangles {
        let a = twice_area(&c, t);
        assert_ne!(a, 0, "a flat triangle {t:?}");
        total += a.abs();
    }
    assert_eq!(total, hull_twice);
    let mut edges: BTreeSet<(u32, u32)> = BTreeSet::new();
    for &[a, b, d] in &c.triangles {
        for (u, v) in [(a, b), (b, d), (d, a)] {
            edges.insert((u.min(v), u.max(v)));
        }
    }
    for &[a, b] in &c.constraints {
        assert!(edges.contains(&(a.min(b), a.max(b))), "{a} {b}");
    }
    let found: BTreeSet<Segment> = c
        .constraints
        .iter()
        .map(|&[a, b]| ordered((xy(&c, a), xy(&c, b))))
        .collect();
    let want: BTreeSet<Segment> = pieces.iter().map(|&s| ordered(s)).collect();
    assert_eq!(found, want);
    c
}

/// solidcore plan/upstream-issues/WR-405--hole-touching-its-outline-at-a-corner-is-built-outside-the-face.md
/// and its addendum WR-405--addendum-2026-11-02-default-off-result-measured.md (b4de2149, no C++
/// run): a hole that touched its outline at a corner was built as a second outer loop. Ours: a
/// 20 by 12 outline with a triangular hole whose corner is a point of the outline, and one whose
/// corner is in the middle of an outline edge, must triangulate with every constraint kept as an
/// edge, the one in the middle split there, and the triangles tiling the hull exactly.
#[test]
fn wr_405_a_hole_touching_its_outline_is_triangulated_with_every_constraint_kept() {
    // The outline (0,0) (20,0) (20,12) (0,12), with its left side broken at (0,6), where the
    // hole's corner is: the hole is (0,6) (4,5) (4,7).
    triangulates_into(
        &[(0, 0), (20, 0), (20, 12), (0, 12), (0, 6), (4, 5), (4, 7)],
        &[
            [0, 1],
            [1, 2],
            [2, 3],
            [3, 4],
            [4, 0],
            [4, 5],
            [5, 6],
            [6, 4],
        ],
        &[
            ((0, 0), (20, 0)),
            ((20, 0), (20, 12)),
            ((20, 12), (0, 12)),
            ((0, 12), (0, 6)),
            ((0, 6), (0, 0)),
            ((0, 6), (4, 5)),
            ((4, 5), (4, 7)),
            ((4, 7), (0, 6)),
        ],
        480,
    );
    // The hole's corner (10, 0) is a site in the middle of the outline's bottom constraint.
    triangulates_into(
        &[(0, 0), (20, 0), (20, 12), (0, 12), (10, 0), (14, 3), (6, 3)],
        &[[0, 1], [1, 2], [2, 3], [3, 0], [4, 5], [5, 6], [6, 4]],
        &[
            ((0, 0), (10, 0)),
            ((10, 0), (20, 0)),
            ((20, 0), (20, 12)),
            ((20, 12), (0, 12)),
            ((0, 12), (0, 0)),
            ((10, 0), (14, 3)),
            ((14, 3), (6, 3)),
            ((6, 3), (10, 0)),
        ],
        480,
    );
}

/// solidcore plan/upstream-issues/UB-128--smface-classifyloops-crashes-for-a-plane-with-a-degenerate.md
/// (b4de2149, confirmed by a C++ run: a segfault): a face on a plane whose domain was a sliver
/// crashed in the classification of its loops, at some sizes and not others. Ours: sites in a
/// strip one step thick and a million long, with and without a constraint along its edge, and
/// sites and a crossing constraint at the corners of the frame, must tile their hull exactly.
#[test]
fn ub_128_slivers_and_the_frames_corners_triangulate_exactly() {
    let long = 1_000_000;
    let strip = [
        (0, 0),
        (long, 0),
        (0, 1),
        (long, 1),
        (long / 2, 0),
        (long / 2, 1),
        (long / 4, 1),
    ];
    triangulates_into(&strip, &[], &[], 2 * i128::from(long));
    triangulates_into(
        &strip,
        &[[0, 1]],
        &[((0, 0), (long / 2, 0)), ((long / 2, 0), (long, 0))],
        2 * i128::from(long),
    );
    let m = COORD_MAX;
    let big = [
        (-m, -m),
        (m, -m),
        (m, m),
        (-m, m),
        (0, 0),
        (m - 1, 3),
        (-5, m),
    ];
    // The diagonal from corner to corner passes through the site at the centre, so it is split.
    let c = triangulates_into(
        &big,
        &[[0, 2]],
        &[((-m, -m), (0, 0)), ((0, 0), (m, m))],
        8 * i128::from(m) * i128::from(m),
    );
    assert!(c.triangles.len() >= 6);
}

/// solidcore plan/upstream-issues/UB-38--point-trees-divide-by-zero-in-addtospatialtree.md and
/// UB-40--smgu-polygonarea-with-no-points-reads-spoints-1.md (b4de2149, no C++ run), with
/// UB-36--findcoincidentpairs-reads-getat-0-of-an-empty-bucket-list.md (confirmed by a C++ run:
/// a segfault): a point tree used before it was ready, the area of no points, and a search of an
/// empty list of buckets read what was not there. Ours: no sites, one, two, and sites on one
/// line (with a constraint across a site that splits it) triangulate to no triangles without an
/// error, and two sites at one point, two with one key and a constraint that names no site or the
/// same site twice are refused by name.
#[test]
fn ub_38_empty_and_flat_site_sets_are_answers_and_bad_input_is_named() {
    let none = triangulate::<u32>(&[], &[], None, &mut meter()).unwrap();
    assert!(none.vertices.is_empty() && none.triangles.is_empty() && none.constraints.is_empty());
    let lone = triangulate(&site_list(&[(7, 7)]), &[], None, &mut meter()).unwrap();
    assert_eq!(lone.vertices.len(), 1);
    assert!(lone.triangles.is_empty());
    assert_eq!(
        triangles_of(&[(0, 0), (9, 4)], &[[0, 1]]),
        segments(&[((0, 0), (9, 4))])
    );
    // On one line: the constraint from end to end is cut at the sites between.
    assert_eq!(
        triangles_of(&[(0, 0), (5, 0), (10, 0)], &[[0, 2]]),
        segments(&[((0, 0), (5, 0)), ((5, 0), (10, 0))])
    );
    assert_eq!(
        triangles_of(&[(0, 0), (1, 1), (7, 7), (2, 2)], &[[0, 2]]),
        segments(&[((0, 0), (1, 1)), ((1, 1), (2, 2)), ((2, 2), (7, 7))])
    );
    assert!(matches!(
        triangulate(&site_list(&[(3, 3), (3, 3)]), &[], None, &mut meter()),
        Err(CdtError::DuplicatePoint { .. })
    ));
    let same_key = [
        Site {
            at: P2::new(0, 0).unwrap(),
            key: 7_u32,
        },
        Site {
            at: P2::new(1, 0).unwrap(),
            key: 7,
        },
    ];
    assert!(matches!(
        triangulate(&same_key, &[], None, &mut meter()),
        Err(CdtError::DuplicateKey { .. })
    ));
    let two = site_list(&[(0, 0), (1, 0)]);
    assert!(matches!(
        triangulate(&two, &[[0, 0]], None, &mut meter()),
        Err(CdtError::BadConstraint { index: 0 })
    ));
    assert!(matches!(
        triangulate(&two, &[[0, 1], [0, 5]], None, &mut meter()),
        Err(CdtError::BadConstraint { index: 1 })
    ));
}

fn segments(list: &[Segment]) -> BTreeSet<Segment> {
    list.iter().map(|&s| ordered(s)).collect()
}

/// The pieces of the constraints of sites on a line or a point, and no triangle among them.
fn triangles_of(points: &[(i64, i64)], constraints: &[[u32; 2]]) -> BTreeSet<Segment> {
    let c = triangulate(&site_list(points), constraints, None, &mut meter()).unwrap();
    assert!(c.triangles.is_empty());
    c.constraints
        .iter()
        .map(|&[a, b]| ordered((xy(&c, a), xy(&c, b))))
        .collect()
}
