//! Plane slices through the public interface (piece G149), against what 19.5 says of them: closed
//! contours on the plane, solid on their left, the same on any pool, and a plane cutting just under
//! itself.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::assert_is_empty,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::manual_midpoint,
    clippy::many_single_char_names,
    reason = "test checks and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroUsize;

use geom_mesh::classify::{Classifier, Located};
use geom_mesh::exact::build::{Corner, Polygons, build};
use geom_mesh::exact::{ExactMesh, MeshError};
use geom_mesh::slice::{Contour, Cut, Point, point, slice};
use geom_num::Sign;
use geom_num::grid::P3;
use geom_num::plane::Plane;
use geom_pool::native::Rayon;
use geom_run::{Budget, Exec, WorkMeter};

mod shapes;

use shapes::{cuboid, frustum, g, join, prism, soup};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 22))
}

fn mesh(input: &Polygons) -> ExactMesh {
    build(input, &Exec::serial(), &mut meter()).unwrap()
}

fn at_z(h: i64) -> Plane {
    Plane::new([0, 0, 1], i128::from(-h)).unwrap()
}

fn cut(m: &ExactMesh, heights: &[i64]) -> Vec<Vec<Contour>> {
    let planes: Vec<Plane> = heights.iter().map(|h| at_z(*h)).collect();
    slice(m, &planes, &Exec::serial(), &mut meter()).unwrap()
}

fn grid_points(m: &ExactMesh, plane: &Plane, c: &Contour) -> Vec<P3> {
    c.cuts()
        .iter()
        .map(|&k| point(m, plane, k).unwrap().to_grid().unwrap())
        .collect()
}

/// Twice the area inside a ring seen from +z, positive counter-clockwise.
fn twice_area(ring: &[P3]) -> i128 {
    let mut sum = 0;
    for (i, a) in ring.iter().enumerate() {
        let b = ring[(i + 1) % ring.len()];
        sum += i128::from(a.x.get()) * i128::from(b.y.get());
        sum -= i128::from(b.x.get()) * i128::from(a.y.get());
    }
    sum
}

#[test]
fn a_box_has_one_square_turning_round_its_solid() {
    let m = mesh(&cuboid(4, 6, 8));
    let layers = cut(&m, &[3]);
    assert_eq!(layers[0].len(), 1);
    let ring = grid_points(&m, &at_z(3), &layers[0][0]);
    assert_eq!(ring.len(), 4);
    assert!(ring.iter().all(|p| p.z.get() == 3));
    assert_eq!(twice_area(&ring), 2 * 24);
}

#[test]
fn a_plane_cuts_just_under_itself() {
    let m = mesh(&cuboid(4, 6, 8));
    let layers = cut(&m, &[-1, 0, 1, 8, 9]);
    let counts: Vec<usize> = layers.iter().map(Vec::len).collect();
    assert_eq!(counts, [0, 0, 1, 1, 0]);
    // At the top the corners are the box's own, one to a corner.
    let top = &layers[3][0];
    assert_eq!(top.cuts().len(), 4);
    assert!(top.cuts().iter().all(|c| matches!(c, Cut::Vertex(_))));
    assert_eq!(twice_area(&grid_points(&m, &at_z(8), top)), 2 * 24);
}

#[test]
fn a_tip_on_the_plane_is_left_out() {
    let m = mesh(&soup(
        vec![g(0, 0, 0), g(4, 0, 0), g(0, 4, 0), g(1, 1, 5)],
        &[&[0, 2, 1], &[0, 1, 3], &[1, 2, 3], &[2, 0, 3]],
    ));
    let layers = cut(&m, &[5, 6]);
    assert!(layers[0].is_empty());
    assert!(layers[1].is_empty());
    assert_eq!(cut(&m, &[4])[0].len(), 1);
}

/// `input` moved by (`x`, `y`, `z`).
fn moved(mut input: Polygons, x: i64, y: i64, z: i64) -> Polygons {
    for v in &mut input.vertices {
        let Corner::Grid(p) = *v else {
            panic!("these shapes are on the grid");
        };
        *v = g(p.x.get() + x, p.y.get() + y, p.z.get() + z);
    }
    input
}

/// `input` with its faces turned in.
fn inside_out(mut input: Polygons) -> Polygons {
    let starts = input.loop_start.clone();
    for pair in starts.windows(2) {
        input.corners[pair[0] as usize..pair[1] as usize].reverse();
    }
    input
}

#[test]
fn a_cavity_turns_the_other_way() {
    let cavity = inside_out(moved(cuboid(4, 4, 4), 3, 3, 3));
    let m = mesh(&join(cuboid(10, 10, 10), &cavity));
    let layers = cut(&m, &[5]);
    assert_eq!(layers[0].len(), 2);
    let mut areas: Vec<i128> = layers[0]
        .iter()
        .map(|c| twice_area(&grid_points(&m, &at_z(5), c)))
        .collect();
    areas.sort_unstable();
    assert_eq!(areas, [-2 * 16, 2 * 100]);
}

#[test]
fn two_boxes_give_two_contours() {
    let m = mesh(&join(cuboid(2, 2, 2), &moved(cuboid(2, 2, 2), 5, 0, 0)));
    assert_eq!(cut(&m, &[1])[0].len(), 2);
}

/// A plate 2 thick from y = 0 to y = 2, of `outline` in x and z, counter-clockwise seen from −y.
fn plate(outline: &[(i64, i64)]) -> Polygons {
    let n = outline.len() as u32;
    let mut vertices = Vec::new();
    for y in [0, 2] {
        for &(x, z) in outline {
            vertices.push(g(x, y, z));
        }
    }
    let mut faces: Vec<Vec<u32>> = vec![(0..n).collect(), (n..2 * n).rev().collect()];
    for i in 0..n {
        let j = (i + 1) % n;
        faces.push(vec![j, i, i + n, j + n]);
    }
    let refs: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
    soup(vertices, &refs)
}

#[test]
fn a_face_with_many_crossings_pairs_them_along_the_line() {
    let comb = [
        (0, 0),
        (9, 0),
        (9, 6),
        (7, 6),
        (7, 2),
        (5, 2),
        (5, 6),
        (3, 6),
        (3, 2),
        (1, 2),
        (1, 6),
        (0, 6),
    ];
    let m = mesh(&plate(&comb));
    // Through the spine; at the floor between the teeth, which just under it is still the spine;
    // through the teeth; and at their tips.
    let layers = cut(&m, &[1, 2, 3, 6]);
    let counts: Vec<usize> = layers.iter().map(Vec::len).collect();
    assert_eq!(counts, [1, 1, 3, 3]);
    let total: i128 = layers[2]
        .iter()
        .map(|c| twice_area(&grid_points(&m, &at_z(3), c)))
        .sum();
    assert_eq!(total, 2 * 2 * (1 + 2 + 2));
}

#[test]
fn points_that_meet_at_a_vertex_on_the_plane_keep_their_order() {
    // Teeth with sharp tips, all at z = 6: just under it, tiny rings that close to nothing.
    let saw = [
        (0, 0),
        (12, 0),
        (12, 2),
        (10, 6),
        (8, 2),
        (6, 6),
        (4, 2),
        (2, 6),
        (0, 2),
    ];
    let m = mesh(&plate(&saw));
    let layers = cut(&m, &[5, 6]);
    assert_eq!(layers[0].len(), 3);
    assert!(layers[1].is_empty());
    // Notches with sharp tops, at z = 4: just under it the plate is in three, each ring with a
    // notch's top for corners.
    let notched = [
        (0, 0),
        (2, 0),
        (4, 4),
        (6, 0),
        (8, 4),
        (10, 0),
        (12, 0),
        (12, 6),
        (0, 6),
    ];
    let m = mesh(&plate(&notched));
    let layers = cut(&m, &[4]);
    assert_eq!(layers[0].len(), 3);
    let total: i128 = layers[0]
        .iter()
        .map(|c| twice_area(&grid_points(&m, &at_z(4), c)))
        .sum();
    assert_eq!(total, 2 * 2 * 12);
}

#[test]
fn contours_lie_on_their_plane_with_solid_on_the_left() {
    let heights = vec![
        vec![40, 56, 48, 64],
        vec![72, 40, 80, 48],
        vec![48, 64, 56, 72],
        vec![56, 48, 64, 40],
    ];
    let (input, _) = prism(&heights, 64);
    let m = mesh(&input);
    let levels = [8, 44, 52, 60, 70, 79];
    let layers = cut(&m, &levels);
    let classifier = Classifier::new(&m, &Exec::serial(), &mut meter()).unwrap();
    let mut seen = 0;
    for (layer, h) in layers.iter().zip(levels) {
        let plane = at_z(h);
        for c in layer {
            for &k in c.cuts() {
                match point(&m, &plane, k).unwrap() {
                    Point::Grid(p) => assert_eq!(plane.side(p).unwrap(), Sign::Zero),
                    Point::Meet(t) => assert_eq!(plane.side_of_tpi(&t).unwrap(), Sign::Zero),
                }
            }
            let ring = grid_points(&m, &plane, c);
            for (i, a) in ring.iter().enumerate() {
                let b = ring[(i + 1) % ring.len()];
                let (dx, dy) = (b.x.get() - a.x.get(), b.y.get() - a.y.get());
                if dx.abs() + dy.abs() < 16 {
                    continue;
                }
                // A little to the left of the middle of a long enough side.
                let (mx, my) = ((a.x.get() + b.x.get()) / 2, (a.y.get() + b.y.get()) / 2);
                let side = |s: i64| {
                    let (u, v) = (-dy.signum() * s, dx.signum() * s);
                    P3::new(mx + u, my + v, h).unwrap()
                };
                let inside = classifier.locate(side(2), &mut meter()).unwrap();
                let outside = classifier.locate(side(-2), &mut meter()).unwrap();
                if inside != Located::OnSurface && outside != Located::OnSurface {
                    assert_ne!(inside, outside);
                    seen += 1;
                }
            }
        }
    }
    assert!(seen > 10);
}

#[test]
fn faces_off_the_grid_cut_exactly() {
    let m = mesh(&frustum());
    let plane = Plane::new([0, 0, 3], -2).unwrap();
    let layers = slice(&m, &[plane], &Exec::serial(), &mut meter()).unwrap();
    assert_eq!(layers[0].len(), 1);
    for &k in layers[0][0].cuts() {
        let Point::Meet(t) = point(&m, &plane, k).unwrap() else {
            panic!("a cut through an edge is three planes");
        };
        assert_eq!(plane.side_of_tpi(&t).unwrap(), Sign::Zero);
    }
}

#[test]
fn a_tilted_plane_cuts_too() {
    let m = mesh(&cuboid(8, 8, 8));
    let plane = Plane::new([1, 1, 1], -12).unwrap();
    let layers = slice(&m, &[plane], &Exec::serial(), &mut meter()).unwrap();
    assert_eq!(layers[0].len(), 1);
    assert_eq!(layers[0][0].cuts().len(), 6);
}

#[test]
fn an_open_mesh_and_bad_planes_are_refused() {
    let open = mesh(&soup(
        vec![g(0, 0, 0), g(4, 0, 0), g(0, 4, 0)],
        &[&[0, 1, 2]],
    ));
    assert_eq!(
        slice(&open, &[at_z(1)], &Exec::serial(), &mut meter()).unwrap_err(),
        MeshError::NotClosed
    );
    let m = mesh(&cuboid(2, 2, 2));
    assert_eq!(
        slice(&m, &[at_z(2), at_z(1)], &Exec::serial(), &mut meter()).unwrap_err(),
        MeshError::SlicesOutOfOrder
    );
    let leaning = Plane::new([0, 1, 1], -1).unwrap();
    assert_eq!(
        slice(&m, &[at_z(1), leaning], &Exec::serial(), &mut meter()).unwrap_err(),
        MeshError::SlicesOutOfOrder
    );
}

#[test]
fn the_threads_do_not_show() {
    let heights: Vec<Vec<i64>> = (0..12)
        .map(|y| (0..12).map(|x| 8 + (x * 7 + y * 13) % 19).collect())
        .collect();
    let (input, _) = prism(&heights, 32);
    let m = mesh(&input);
    let planes: Vec<Plane> = (1..24).map(at_z).collect();
    let pool = Rayon::new(NonZeroUsize::new(4).unwrap()).unwrap();
    let serial = slice(&m, &planes, &Exec::serial(), &mut meter()).unwrap();
    let threaded = slice(&m, &planes, &Exec::with_pool(&pool), &mut meter()).unwrap();
    assert_eq!(serial, threaded);
}

#[test]
fn the_meter_stops_a_slice() {
    let m = mesh(&cuboid(2, 2, 2));
    let mut short = WorkMeter::new(Budget::of_work(4));
    assert!(matches!(
        slice(&m, &[at_z(1)], &Exec::serial(), &mut short),
        Err(MeshError::BudgetExceeded(_) | MeshError::Stopped(_))
    ));
}
