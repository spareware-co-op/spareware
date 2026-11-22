//! Clipping and local cuts: parts that add up, and no piece crossing another (G126, 11.3).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and short names, and a failure stops the test"
)]

use geom_num::plane::{Plane, Tpi};
use geom_poly::convex::{ConvexError, Piece};
use geom_poly::cut::{Clipped, clip, crosses, local_cuts};
use geom_run::{Budget, WorkMeter};
use proptest::prelude::*;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The plane where coordinate `axis` is `at`, facing up that axis.
fn up(axis: usize, at: i64) -> Plane {
    let mut n = [0i128; 3];
    n[axis] = 1;
    Plane::new(n, -i128::from(at)).unwrap()
}

/// The same plane, facing down.
fn down(axis: usize, at: i64) -> Plane {
    let mut n = [0i128; 3];
    n[axis] = -1;
    Plane::new(n, i128::from(at)).unwrap()
}

/// A rectangle on the plane where coordinate `axis` is `at`, facing up that axis, over `u` and `v`
/// along the next two axes in turn.
fn rect(axis: usize, at: i64, u: (i64, i64), v: (i64, i64)) -> Piece {
    let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
    Piece {
        face: up(axis, at),
        bounds: vec![up(b, v.0), down(a, u.1), down(b, v.1), up(a, u.0)],
    }
}

/// A piece's corners on the grid; cuts here fall on whole steps.
fn corners(piece: &Piece) -> Vec<[i64; 3]> {
    let k = piece.bounds.len();
    (0..k)
        .map(|i| {
            let t = Tpi::new(piece.face, piece.bounds[i], piece.bounds[(i + 1) % k]).unwrap();
            let r = t.to_grid().unwrap().value();
            [r.x.get(), r.y.get(), r.z.get()]
        })
        .collect()
}

/// Twice a piece's area, positive when it turns counter-clockwise about its normal.
fn doubled_area(piece: &Piece) -> i128 {
    let axis = piece.face.n().iter().position(|c| *c != 0).unwrap();
    let up_axis = piece.face.n()[axis] > 0;
    let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
    let c = corners(piece);
    let sum: i128 = (0..c.len())
        .map(|i| {
            let (p, q) = (c[i], c[(i + 1) % c.len()]);
            i128::from(p[a]) * i128::from(q[b]) - i128::from(p[b]) * i128::from(q[a])
        })
        .sum();
    if up_axis { sum } else { -sum }
}

fn split(piece: &Piece, plane: &Plane) -> (Piece, Piece) {
    match clip(piece, plane).unwrap() {
        Clipped::Split(front, back) => (front, back),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_cut_parts_a_rectangle_into_what_adds_up() {
    let piece = rect(2, 0, (0, 4), (0, 6));
    assert_eq!(doubled_area(&piece), 48);
    let (front, back) = split(&piece, &up(0, 1));
    assert_eq!(doubled_area(&front), 36);
    assert_eq!(doubled_area(&back), 12);
    // Both parts keep the turn of the whole, and the cut bounds each.
    assert_eq!(front.bounds.len(), 4);
    assert_eq!(back.bounds.len(), 4);
    assert!(front.bounds.contains(&up(0, 1)));
    assert!(back.bounds.contains(&down(0, 1)));
    assert_eq!(front.face, piece.face);
}

#[test]
fn a_cut_through_a_corner_keeps_both_parts_convex() {
    let piece = rect(2, 0, (0, 4), (0, 4));
    // x = y, through two corners.
    let diagonal = Plane::new([1, -1, 0], 0).unwrap();
    let (front, back) = split(&piece, &diagonal);
    assert_eq!(front.bounds.len(), 3);
    assert_eq!(back.bounds.len(), 3);
    assert_eq!(doubled_area(&front), 16);
    assert_eq!(doubled_area(&back), 16);
}

#[test]
fn a_cut_across_one_corner_leaves_a_triangle_and_a_pentagon() {
    let piece = rect(2, 0, (0, 4), (0, 4));
    // x + y = 2.
    let slant = Plane::new([1, 1, 0], -2).unwrap();
    let (front, back) = split(&piece, &slant);
    assert_eq!(front.bounds.len(), 5);
    assert_eq!(back.bounds.len(), 3);
    assert_eq!(doubled_area(&back), 4);
    assert_eq!(doubled_area(&front), 28);
}

#[test]
fn a_cut_past_a_triangles_first_corner_keeps_the_quadrilateral_in_turn() {
    // (0, 0), (4, 0), (0, 4): the cut x = 1 leaves one corner in front and two behind, so every
    // bound keeps a length behind it and the cut must close the gap at the corner it took.
    let triangle = Piece {
        face: up(2, 0),
        bounds: vec![Plane::new([-1, -1, 0], 4).unwrap(), up(0, 0), up(1, 0)],
    };
    assert_eq!(doubled_area(&triangle), 16);
    let (front, back) = split(&triangle, &up(0, 1));
    assert_eq!(front.bounds.len(), 3);
    assert_eq!(back.bounds.len(), 4);
    assert_eq!(doubled_area(&front), 9);
    assert_eq!(doubled_area(&back), 7);
}

#[test]
fn a_cut_gives_the_same_parts_from_any_first_bound() {
    let base = vec![Plane::new([-1, -1, 0], 4).unwrap(), up(0, 0), up(1, 0)];
    for start in 0..3 {
        let mut bounds = base.clone();
        bounds.rotate_left(start);
        let triangle = Piece {
            face: up(2, 0),
            bounds,
        };
        let (front, back) = split(&triangle, &up(0, 1));
        assert_eq!((doubled_area(&front), doubled_area(&back)), (9, 7));
        let (front, back) = split(&triangle, &down(0, 1));
        assert_eq!((doubled_area(&front), doubled_area(&back)), (7, 9));
    }
}

#[test]
fn a_plane_that_only_touches_leaves_the_piece_whole() {
    let piece = rect(2, 0, (0, 4), (0, 4));
    assert_eq!(clip(&piece, &up(0, -1)), Ok(Clipped::Front));
    assert_eq!(clip(&piece, &up(0, 0)), Ok(Clipped::Front));
    assert_eq!(clip(&piece, &up(0, 4)), Ok(Clipped::Back));
    assert_eq!(clip(&piece, &up(0, 9)), Ok(Clipped::Back));
    assert_eq!(clip(&piece, &up(2, 0)), Ok(Clipped::On));
    assert_eq!(clip(&piece, &down(2, 0)), Ok(Clipped::On));
    // Through one corner only.
    let touch = Plane::new([1, 1, 0], 0).unwrap();
    assert_eq!(clip(&piece, &touch), Ok(Clipped::Front));
}

#[test]
fn two_pieces_cross_only_where_they_pass_through_each_other() {
    let a = rect(2, 0, (-4, 4), (-4, 4));
    let through = rect(0, 0, (-2, 2), (-2, 2));
    assert_eq!(crosses(&a, &through), Ok(true));
    assert_eq!(crosses(&through, &a), Ok(true));
    // On one plane, or on parallel ones: no line to meet on.
    assert_eq!(crosses(&a, &rect(2, 0, (0, 9), (0, 9))), Ok(false));
    assert_eq!(crosses(&a, &rect(2, 3, (-4, 4), (-4, 4))), Ok(false));
    // Standing on it, ending at its plane.
    assert_eq!(crosses(&a, &rect(0, 0, (-2, 2), (0, 2))), Ok(false));
    // Past its edge: the planes cross, the stretches of their line do not meet.
    assert_eq!(crosses(&a, &rect(0, 0, (5, 7), (-2, 2))), Ok(false));
    assert_eq!(crosses(&a, &rect(0, 9, (-2, 2), (-2, 2))), Ok(false));
    // Sharing only a point of that line.
    assert_eq!(crosses(&a, &rect(0, 0, (4, 7), (-2, 2))), Ok(false));
}

#[test]
fn the_stretch_of_a_line_is_its_last_entry_to_its_first_exit() {
    let a = rect(2, 0, (-4, 4), (-4, 4));
    // On x = 2y, a line that crosses a's bounds' planes at y = -4, -2, 2 and 4, between planes
    // where 2x + y is `low` and `high`. The line leaves a where 2x + y = 10.
    let across = |low: i128, high: i128| Piece {
        face: Plane::new([1, -2, 0], 0).unwrap(),
        bounds: vec![
            up(2, -1),
            Plane::new([-2, -1, 0], high).unwrap(),
            down(2, 1),
            Plane::new([2, 1, 0], -low).unwrap(),
        ],
    };
    for (low, high, want) in [
        (-30, 30, true),
        (9, 30, true),
        (10, 30, false),
        (20, 30, false),
        (-30, -9, true),
        (-30, -10, false),
        (-30, -20, false),
    ] {
        assert_eq!(crosses(&a, &across(low, high)), Ok(want), "{low} {high}");
        assert_eq!(crosses(&across(low, high), &a), Ok(want), "{low} {high}");
    }
}

#[test]
fn local_cuts_part_each_polygon_where_another_crosses_it() {
    let a = rect(2, 0, (-4, 4), (-4, 4));
    let b = rect(0, 0, (-2, 2), (-2, 2));
    let cut = local_cuts(&[a.clone(), b.clone()], &mut meter()).unwrap();
    assert_eq!(cut[0].len(), 2);
    assert_eq!(cut[1].len(), 2);
    let area = |p: &[Piece]| p.iter().map(doubled_area).sum::<i128>();
    assert_eq!(area(&cut[0]), doubled_area(&a));
    assert_eq!(area(&cut[1]), doubled_area(&b));
    for x in &cut[0] {
        for y in &cut[1] {
            assert_eq!(crosses(x, y), Ok(false));
        }
    }
}

#[test]
fn a_polygon_nothing_crosses_stays_whole() {
    let a = rect(2, 0, (-4, 4), (-4, 4));
    let b = rect(2, 1, (-4, 4), (-4, 4));
    let cut = local_cuts(&[a.clone(), b], &mut meter()).unwrap();
    assert_eq!(cut[0], [a]);
    assert_eq!(cut[1].len(), 1);
}

#[test]
fn a_polygon_beyond_the_edge_cuts_nothing_though_its_plane_passes_through() {
    let a = rect(2, 0, (-4, 4), (-4, 4));
    let b = rect(0, 0, (5, 7), (-2, 2));
    let cut = local_cuts(&[a.clone(), b.clone()], &mut meter()).unwrap();
    assert_eq!(cut[0], [a]);
    assert_eq!(cut[1], [b]);
}

#[test]
fn a_polygon_that_ends_on_another_cuts_it_along_that_end() {
    let a = rect(2, 0, (-4, 4), (-4, 4));
    // Standing on a: its lower edge lies in a's plane.
    let standing = rect(0, 0, (-2, 2), (0, 2));
    let cut = local_cuts(&[a.clone(), standing.clone()], &mut meter()).unwrap();
    assert_eq!(cut[0].len(), 2);
    assert_eq!(cut[1], [standing]);
    // Beyond a's edge, on the same plane.
    let beyond = rect(0, 0, (5, 7), (0, 2));
    let cut = local_cuts(&[a.clone(), beyond], &mut meter()).unwrap();
    assert_eq!(cut[0], std::slice::from_ref(&a));
    // Ending at a's corner, along one point of the line.
    let past = rect(0, 0, (4, 7), (0, 2));
    let cut = local_cuts(&[a.clone(), past], &mut meter()).unwrap();
    assert_eq!(cut[0], std::slice::from_ref(&a));
    // Off a's plane altogether.
    let above = rect(0, 0, (-2, 2), (1, 2));
    let cut = local_cuts(&[a.clone(), above], &mut meter()).unwrap();
    assert_eq!(cut[0], std::slice::from_ref(&a));
    // Touching a's plane at one corner: a diamond in x = 0, its lowest corner at the origin.
    let diamond = Piece {
        face: up(0, 0),
        bounds: vec![
            Plane::new([0, 1, 1], 0).unwrap(),
            Plane::new([0, 1, -1], 4).unwrap(),
            Plane::new([0, -1, -1], 4).unwrap(),
            Plane::new([0, -1, 1], 0).unwrap(),
        ],
    };
    let cut = local_cuts(&[a.clone(), diamond], &mut meter()).unwrap();
    assert_eq!(cut[0], [a]);
}

#[test]
fn polygons_on_one_plane_cut_each_other_where_they_overlap() {
    let a = rect(2, 0, (0, 4), (0, 4));
    let b = rect(2, 0, (2, 6), (2, 6));
    let cut = local_cuts(&[a.clone(), b.clone()], &mut meter()).unwrap();
    let area = |p: &[Piece]| p.iter().map(doubled_area).sum::<i128>();
    assert_eq!(cut[0].len(), 3);
    assert_eq!(cut[1].len(), 3);
    assert_eq!(area(&cut[0]), doubled_area(&a));
    assert_eq!(area(&cut[1]), doubled_area(&b));
    assert!(cut[0].iter().any(|p| doubled_area(p) == 8));
    // Facing the other way changes nothing; apart they stay whole.
    let turned = Piece {
        face: down(2, 0),
        bounds: b.bounds.iter().rev().copied().collect(),
    };
    assert_eq!(
        local_cuts(&[a.clone(), turned], &mut meter()).unwrap()[0].len(),
        3
    );
    let apart = rect(2, 0, (7, 9), (0, 4));
    assert_eq!(
        local_cuts(&[a.clone(), apart], &mut meter()).unwrap()[0],
        std::slice::from_ref(&a)
    );
    // Side by side along an edge they share no area.
    let beside = rect(2, 0, (4, 8), (0, 4));
    assert_eq!(
        local_cuts(&[a.clone(), beside], &mut meter()).unwrap()[0],
        [a]
    );
}

#[test]
fn the_same_polygons_give_the_same_pieces() {
    let list = [
        rect(2, 0, (-4, 4), (-4, 4)),
        rect(0, 0, (-2, 2), (-2, 2)),
        rect(1, 1, (-3, 3), (-3, 3)),
    ];
    assert_eq!(
        local_cuts(&list, &mut meter()).unwrap(),
        local_cuts(&list, &mut meter()).unwrap()
    );
}

#[test]
fn a_small_budget_stops_the_cuts() {
    let list = [rect(2, 0, (-4, 4), (-4, 4)), rect(0, 0, (-2, 2), (-2, 2))];
    let mut small = WorkMeter::new(Budget::new(0, u64::MAX));
    assert!(matches!(
        local_cuts(&list, &mut small),
        Err(ConvexError::Stopped(_))
    ));
}

fn any_rect() -> impl Strategy<Value = Piece> {
    (0usize..3, -6i64..=6, -8i64..8, 1i64..8, -8i64..8, 1i64..8)
        .prop_map(|(axis, at, u, du, v, dv)| rect(axis, at, (u, u + du), (v, v + dv)))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn after_local_cuts_no_piece_crosses_another_and_every_area_stays(
        list in prop::collection::vec(any_rect(), 1..6)
    ) {
        let cut = local_cuts(&list, &mut meter()).unwrap();
        for (polygon, pieces) in list.iter().zip(&cut) {
            let total: i128 = pieces.iter().map(doubled_area).sum();
            prop_assert_eq!(total, doubled_area(polygon));
            prop_assert!(pieces.iter().all(|p| doubled_area(p) > 0));
        }
        let all: Vec<&Piece> = cut.iter().flatten().collect();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                prop_assert!(!crosses(a, b).unwrap());
            }
        }
    }
}
