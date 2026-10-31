//! The core queries on topology and points (10.2, 22; piece G102): faces listed, the bounding box,
//! exact twice-area vectors and six-times volumes, and the side of a shell, on geom-topo's wall (a
//! box of 4,000 by 200 by 2,700 steps), on shapes written for each claim, and on bodies the queries
//! must refuse.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{WALL, ix, many_faces, meter, plane, wall, wall_builder, wall_outside};
use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_math::Box3;
use geom_num::P3;
use geom_num::wide::I256;
use geom_run::{Budget, Stop, WorkMeter};
use geom_surface::{Sphere, SurfaceKind};
use geom_topo::query::{
    QueryError, Side, bounding_box, face_area, faces, loop_area, point, shell_side, shell_volume,
    side_of_shell, volume,
};
use geom_topo::{
    Body, BodyKind, Builder, Edge, Face, FaceIx, Fin, Loop, LoopIx, Region, RegionKind, Sense,
    Shell, ShellIx, Vertex, VertexIx,
};

const X: i64 = WALL[0];
const Y: i64 = WALL[1];
const Z: i64 = WALL[2];

fn rebuilt(edit: impl FnOnce(&mut Builder)) -> Body {
    let mut b = wall_builder(true);
    edit(&mut b);
    b.publish(BodyKind::Solid, wall_outside(), &mut meter())
        .unwrap()
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// A body of closed shells, each given by its vertices and its faces as outer loops of vertex
/// numbers, every face on one plane the test does not read: the fins are what the queries use.
/// One solid region holds every shell.
fn shells(shapes: &[(&[[i64; 3]], &[&[u32]])]) -> Body {
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
    a.surfaces.push(plane([0.0; 3], 0, 1)).unwrap();
    let (mut face, mut fin, mut edge, mut vertex) = (0_u32, 0_u32, 0_u32, 0_u32);
    let count = shapes.len() as u32;
    let mut first_fin_of_pair: std::collections::BTreeMap<(u32, u32), u32> = Default::default();
    for (s, (points, polys)) in shapes.iter().enumerate() {
        let s = s as u32;
        a.shells
            .push(Shell {
                region: ix(0),
                next: (s + 1 < count).then(|| ix(s + 1)),
                first_face: Some(ix(face)),
            })
            .unwrap();
        for q in *points {
            a.points.push(p(q[0], q[1], q[2])).unwrap();
            a.vertices.push(Vertex { point: ix(vertex) }).unwrap();
            vertex += 1;
        }
        let base = vertex - points.len() as u32;
        for (n, poly) in polys.iter().enumerate() {
            let last = n + 1 == polys.len();
            a.faces
                .push(Face {
                    shell: ix(s),
                    next: (!last).then(|| ix(face + 1)),
                    surface: ix(0),
                    sense: Sense::Forward,
                    first_loop: Some(ix(face)),
                    bound: Box3::new(p(0, 0, 0), p(0, 0, 0)).unwrap(),
                })
                .unwrap();
            a.loops
                .push(Loop {
                    face: ix(face),
                    next: None,
                    first_fin: Some(ix(fin)),
                })
                .unwrap();
            let k = poly.len() as u32;
            for (i, &v) in poly.iter().enumerate() {
                let i = i as u32;
                let w = poly[((i + 1) % k) as usize];
                let (lo, hi) = (v.min(w) + base, v.max(w) + base);
                let this = fin + i;
                let (e, sense, radial) = match first_fin_of_pair.get(&(lo, hi)) {
                    Some(&other) => {
                        let e = a.fins.edge.get(ix(other)).copied().unwrap();
                        a.fins.radial.set(ix(other), ix(this)).unwrap();
                        (e, Sense::Reversed, ix(other))
                    }
                    None => {
                        first_fin_of_pair.insert((lo, hi), this);
                        let q = |r: u32| {
                            let c = points[(r - base) as usize];
                            [c[0] as f64, c[1] as f64, c[2] as f64]
                        };
                        let (s0, s1) = (q(v + base), q(w + base));
                        let d = [s1[0] - s0[0], s1[1] - s0[1], s1[2] - s0[2]];
                        a.curves
                            .push(CurveKind::Line(Line::new(s0, d, (0.0, 1.0)).unwrap()))
                            .unwrap();
                        a.edges
                            .push(Edge {
                                first_fin: Some(ix(this)),
                                start: Some(ix(v + base)),
                                end: Some(ix(w + base)),
                                curve: ix(edge),
                            })
                            .unwrap();
                        edge += 1;
                        (ix(edge - 1), Sense::Forward, ix(this))
                    }
                };
                a.fins
                    .push(Fin {
                        loop_: ix(face),
                        next: ix(fin + (i + 1) % k),
                        prev: ix(fin + (i + k - 1) % k),
                        edge: e,
                        sense,
                        radial,
                        pcurve: None,
                    })
                    .unwrap();
            }
            fin += k;
            face += 1;
        }
    }
    b.publish(BodyKind::Solid, ix(1), &mut meter()).unwrap()
}

/// The corners of a box from `lo` to `hi`: bit 0 is x, bit 1 is y, bit 2 is z.
fn corners(lo: [i64; 3], hi: [i64; 3]) -> Vec<[i64; 3]> {
    (0..8)
        .map(|v| [0, 1, 2].map(|k| if v >> k & 1 == 1 { hi[k] } else { lo[k] }))
        .collect()
}

/// A box's faces, each counter-clockwise seen from outside.
const BOX: [&[u32]; 6] = [
    &[0, 2, 3, 1],
    &[4, 5, 7, 6],
    &[0, 1, 5, 4],
    &[2, 6, 7, 3],
    &[0, 4, 6, 2],
    &[1, 3, 7, 5],
];

/// The same faces, each clockwise seen from outside: a cavity's.
const INSIDE_OUT: [&[u32]; 6] = [
    &[1, 3, 2, 0],
    &[6, 7, 5, 4],
    &[4, 5, 1, 0],
    &[3, 7, 6, 2],
    &[2, 6, 4, 0],
    &[5, 7, 3, 1],
];

/// A staircase: an L-shaped prism, not convex, with its two end faces as one loop of six.
const L_POINTS: [[i64; 3]; 12] = [
    [0, 0, 0],
    [4, 0, 0],
    [4, 2, 0],
    [2, 2, 0],
    [2, 4, 0],
    [0, 4, 0],
    [0, 0, 3],
    [4, 0, 3],
    [4, 2, 3],
    [2, 2, 3],
    [2, 4, 3],
    [0, 4, 3],
];
const L_FACES: [&[u32]; 8] = [
    &[5, 4, 3, 2, 1, 0],
    &[6, 7, 8, 9, 10, 11],
    &[0, 1, 7, 6],
    &[1, 2, 8, 7],
    &[2, 3, 9, 8],
    &[3, 4, 10, 9],
    &[4, 5, 11, 10],
    &[5, 0, 6, 11],
];

#[test]
fn a_boxes_volume_and_area_vectors_are_exact() {
    let w = wall();
    let six = I256::from_i128(6 * i128::from(X * Y * Z));
    assert_eq!(volume(&w, &mut meter()).unwrap(), six);
    let s: ShellIx = ix(0);
    assert_eq!(shell_volume(&w, s, &mut meter()).unwrap(), six);
    let mut seen: Vec<[i128; 3]> = (0..6)
        .map(|f| face_area(&w, ix(f), &mut meter()).unwrap())
        .collect();
    let (x, y, z) = (i128::from(X), i128::from(Y), i128::from(Z));
    let mut want = vec![
        [2 * y * z, 0, 0],
        [-2 * y * z, 0, 0],
        [0, 2 * x * z, 0],
        [0, -2 * x * z, 0],
        [0, 0, 2 * x * y],
        [0, 0, -2 * x * y],
    ];
    seen.sort_unstable();
    want.sort_unstable();
    assert_eq!(seen, want);
    // A closed shell's area vectors sum to nothing.
    let total = seen
        .iter()
        .fold([0; 3], |t, a| [t[0] + a[0], t[1] + a[1], t[2] + a[2]]);
    assert_eq!(total, [0; 3]);
}

#[test]
fn a_loops_area_is_its_faces_and_a_vertex_is_its_point() {
    let w = wall();
    let l: LoopIx = ix(0);
    let face: FaceIx = ix(0);
    assert_eq!(
        loop_area(&w, l, &mut meter()).unwrap(),
        face_area(&w, face, &mut meter()).unwrap()
    );
    let v: VertexIx = ix(0);
    assert_eq!(
        point(&w, v).unwrap(),
        w.arenas().points.get(ix(0)).copied().unwrap()
    );
}

#[test]
fn a_loop_with_two_fins_or_none_encloses_nothing() {
    // A face of one loop with no fins has a loop and no vertex.
    let empty = rebuilt(|b| {
        b.arenas.loops.first_fin.set(ix(0), None).unwrap();
    });
    assert_eq!(loop_area(&empty, ix(0), &mut meter()).unwrap(), [0; 3]);
    assert_eq!(
        face_area(&empty, ix(0), &mut meter()),
        Err(QueryError::Loopless(ix(0)))
    );
}

#[test]
fn a_cavity_counts_negative() {
    let outer = corners([0, 0, 0], [10, 10, 10]);
    let inner = corners([2, 3, 4], [5, 7, 9]);
    let body = shells(&[(&outer, &BOX), (&inner, &INSIDE_OUT)]);
    assert_eq!(
        shell_volume(&body, ix(0), &mut meter()).unwrap(),
        I256::from_i64(6000)
    );
    assert_eq!(
        shell_volume(&body, ix(1), &mut meter()).unwrap(),
        I256::from_i64(-6 * 3 * 4 * 5)
    );
    assert_eq!(
        volume(&body, &mut meter()).unwrap(),
        I256::from_i64(6000 - 360)
    );
}

#[test]
fn turning_every_face_inside_out_negates_the_volume() {
    let inside_out = rebuilt(|b| {
        for row in 0..b.arenas.fins.len() {
            let f = b.arenas.fins.get(ix(row)).unwrap();
            let flipped = match f.sense {
                Sense::Forward => Sense::Reversed,
                Sense::Reversed => Sense::Forward,
            };
            b.arenas.fins.sense.set(ix(row), flipped).unwrap();
            b.arenas.fins.next.set(ix(row), f.prev).unwrap();
            b.arenas.fins.prev.set(ix(row), f.next).unwrap();
        }
    });
    assert_eq!(
        volume(&inside_out, &mut meter()).unwrap(),
        I256::from_i128(-6 * i128::from(X * Y * Z))
    );
}

#[test]
fn the_volume_does_not_depend_on_where_the_body_is() {
    let at = |d: i64| {
        let lo = [d, -d, 2 * d];
        let hi = [d + 7, -d + 11, 2 * d + 13];
        corners(lo, hi)
    };
    for d in [0, 1, 1000, -123_456_789, 500_000_000_000] {
        let points = at(d);
        let body = shells(&[(&points, &BOX)]);
        assert_eq!(
            volume(&body, &mut meter()).unwrap(),
            I256::from_i64(6 * 7 * 11 * 13),
            "{d}"
        );
    }
}

#[test]
fn the_largest_box_on_the_grid_fits() {
    let m = (1_i64 << 40) - 1;
    let points = corners([-m, -m, -m], [m, m, m]);
    let body = shells(&[(&points, &BOX)]);
    let side = I256::from_i64(2 * m);
    let cube = side.checked_mul(&side).unwrap().checked_mul(&side).unwrap();
    assert_eq!(
        volume(&body, &mut meter()).unwrap(),
        cube.checked_mul(&I256::from_i64(6)).unwrap()
    );
    // A point inside it, and a corner: no room beyond the box is needed.
    assert_eq!(
        side_of_shell(&body, ix(0), p(0, 0, 0), &mut meter()).unwrap(),
        Some(Side::Inside)
    );
    assert_eq!(
        side_of_shell(&body, ix(0), p(m, m, m), &mut meter()).unwrap(),
        None
    );
}

#[test]
fn the_box_of_a_flat_body_is_its_vertices_and_tight() {
    let (b, tight) = bounding_box(&wall(), &mut meter()).unwrap().unwrap();
    assert!(tight);
    assert_eq!([b.min().x.get(), b.min().y.get(), b.min().z.get()], [0, 0, 0]);
    assert_eq!([b.max().x.get(), b.max().y.get(), b.max().z.get()], [X, Y, Z]);
    // Faces without loops add their stored boxes, and the box is no longer tight.
    let (b, tight) = bounding_box(&many_faces(3), &mut meter()).unwrap().unwrap();
    assert!(!tight);
    assert_eq!([b.max().x.get(), b.max().y.get()], [3, 1]);
    // A body with no vertex and no face has no box.
    let mut e = Builder::new();
    e.arenas
        .regions
        .push(Region {
            kind: RegionKind::Void,
            first_shell: None,
        })
        .unwrap();
    let none = e.publish(BodyKind::General, ix(0), &mut meter()).unwrap();
    assert_eq!(bounding_box(&none, &mut meter()).unwrap(), None);
}

#[test]
fn faces_are_listed_in_arena_order_with_their_rows() {
    let w = wall();
    let rows = faces(&w, &mut meter()).unwrap();
    assert_eq!(rows.len(), 6);
    for (i, r) in rows.iter().enumerate() {
        let f = w.arenas().faces.get(r.face).unwrap();
        assert_eq!(r.face.to_usize(), i);
        assert_eq!(
            (r.shell, r.surface, r.sense, r.loops),
            (f.shell, f.surface, f.sense, 1)
        );
        assert_eq!(r.name, None);
    }
    assert_eq!(faces(&many_faces(2), &mut meter()).unwrap()[0].loops, 0);
}

#[test]
fn a_points_side_of_a_box_is_exact_at_each_face() {
    let points = corners([0, 0, 0], [10, 10, 10]);
    let body = shells(&[(&points, &BOX)]);
    let side = |q| side_of_shell(&body, ix(0), q, &mut meter()).unwrap();
    assert_eq!(side(p(5, 5, 5)), Some(Side::Inside));
    assert_eq!(side(p(1, 1, 1)), Some(Side::Inside));
    assert_eq!(side(p(9, 9, 9)), Some(Side::Inside));
    for q in [
        p(11, 5, 5),
        p(-1, 5, 5),
        p(5, 11, 5),
        p(5, -1, 5),
        p(5, 5, 11),
        p(5, 5, -1),
    ] {
        assert_eq!(side(q), Some(Side::Outside), "{q:?}");
    }
    // A corner is held by the shell; a point away from every vertex with the ray through an edge
    // or a vertex still counts once.
    assert_eq!(side(p(0, 0, 0)), None);
    assert_eq!(side(p(10, 10, 10)), None);
    assert_eq!(side(p(-5, 0, 0)), Some(Side::Outside));
    assert_eq!(side(p(-5, 10, 10)), Some(Side::Outside));
    assert_eq!(side(p(-5, 0, 5)), Some(Side::Outside));
    assert_eq!(side(p(15, 5, 5)), Some(Side::Outside));
}

#[test]
fn a_points_side_does_not_depend_on_which_way_the_shell_faces() {
    let points = corners([0, 0, 0], [10, 10, 10]);
    let inward = shells(&[(&points, &INSIDE_OUT)]);
    assert_eq!(
        side_of_shell(&inward, ix(0), p(5, 5, 5), &mut meter()).unwrap(),
        Some(Side::Inside)
    );
    assert_eq!(
        side_of_shell(&inward, ix(0), p(50, 5, 5), &mut meter()).unwrap(),
        Some(Side::Outside)
    );
}

#[test]
fn a_loop_that_is_not_convex_is_counted_correctly() {
    let body = shells(&[(&L_POINTS, &L_FACES)]);
    assert_eq!(
        volume(&body, &mut meter()).unwrap(),
        I256::from_i64(6 * 12 * 3)
    );
    let side = |q| side_of_shell(&body, ix(0), q, &mut meter()).unwrap();
    // The notch of the L: inside the box round it, outside the shape.
    assert_eq!(side(p(3, 3, 1)), Some(Side::Outside));
    assert_eq!(side(p(1, 1, 1)), Some(Side::Inside));
    assert_eq!(side(p(3, 1, 1)), Some(Side::Inside));
    assert_eq!(side(p(1, 3, 1)), Some(Side::Inside));
}

#[test]
fn a_point_in_a_faces_plane_outside_it_is_outside() {
    // Every fan triangle of the L's end face lies in the plane z = 0, and a point there in the
    // notch sees the triangles of the fan cancel.
    let body = shells(&[(&L_POINTS, &L_FACES)]);
    assert_eq!(
        side_of_shell(&body, ix(0), p(3, 3, 0), &mut meter()).unwrap(),
        Some(Side::Outside)
    );
    assert_eq!(
        side_of_shell(&body, ix(0), p(3, 3, 3), &mut meter()).unwrap(),
        Some(Side::Outside)
    );
    assert_eq!(
        side_of_shell(&body, ix(0), p(7, 1, 0), &mut meter()).unwrap(),
        Some(Side::Outside)
    );
}

/// Whether `q` lies in the L scaled by `k`, by arithmetic alone: `gap` 0 for the closed shape, 1
/// for its open interior.
fn in_l(q: [i64; 3], k: i64, gap: i64) -> bool {
    let within = |c: i64, lo: i64, hi: i64| lo + gap <= c && c <= hi - gap;
    within(q[2], 0, 3 * k)
        && (within(q[0], 0, 4 * k) && within(q[1], 0, 2 * k)
            || within(q[0], 0, 2 * k) && within(q[1], 0, 4 * k))
}

#[test]
fn the_side_of_a_point_agrees_with_arithmetic_on_every_grid_point_near_a_shape() {
    // Every grid point in a box round the L that is not on it: rays run through its vertices and
    // edges, along its faces and through the diagonals of its fans.
    let k = 2;
    let scaled: Vec<[i64; 3]> = L_POINTS
        .iter()
        .map(|c| [c[0] * k, c[1] * k, c[2] * k])
        .collect();
    let body = shells(&[(&scaled, &L_FACES)]);
    let mut seen = [0, 0];
    for x in -2..11 {
        for y in -2..11 {
            for z in -2..9 {
                let q = [x, y, z];
                let inside = in_l(q, k, 1);
                if !inside && in_l(q, k, 0) {
                    continue;
                }
                let want = if inside { Side::Inside } else { Side::Outside };
                seen[usize::from(inside)] += 1;
                assert_eq!(
                    side_of_shell(&body, ix(0), p(x, y, z), &mut meter()).unwrap(),
                    Some(want),
                    "{x} {y} {z}"
                );
            }
        }
    }
    assert!(seen[0] > 1000 && seen[1] > 50, "{seen:?}");
}

#[test]
fn a_point_on_a_face_is_the_point_moved_a_little_up_y_up_z_and_along_x() {
    // The moved point is p + (ε³, ε, ε²). A face across x: moved along +x, so a point on the low
    // face is inside and one on the high face outside.
    let points = corners([0, 0, 0], [10, 10, 10]);
    let body = shells(&[(&points, &BOX)]);
    let side = |q| side_of_shell(&body, ix(0), q, &mut meter()).unwrap();
    assert_eq!(side(p(0, 3, 4)), Some(Side::Inside));
    assert_eq!(side(p(10, 3, 4)), Some(Side::Outside));
    // A face across y or z alone does not meet the ray: the move up y or z decides.
    assert_eq!(side(p(3, 0, 4)), Some(Side::Inside));
    assert_eq!(side(p(3, 10, 4)), Some(Side::Outside));
    assert_eq!(side(p(3, 4, 0)), Some(Side::Inside));
    assert_eq!(side(p(3, 4, 10)), Some(Side::Outside));
    // A sloping face x + y + z = 12: its normal has y, and the move up y leaves the solid.
    let corner: [[i64; 3]; 4] = [[0, 0, 0], [12, 0, 0], [0, 12, 0], [0, 0, 12]];
    let faces: [&[u32]; 4] = [&[0, 2, 1], &[0, 1, 3], &[0, 3, 2], &[1, 2, 3]];
    let tetra = shells(&[(&corner, &faces)]);
    assert_eq!(
        side_of_shell(&tetra, ix(0), p(3, 4, 5), &mut meter()).unwrap(),
        Some(Side::Outside)
    );
    // Its mirror in y, −y sloping the other way: the move up y enters it.
    let mirrored: [[i64; 3]; 4] = [[0, 0, 0], [12, 0, 0], [0, -12, 0], [0, 0, 12]];
    let flipped: [&[u32]; 4] = [&[0, 1, 2], &[0, 3, 1], &[0, 2, 3], &[1, 3, 2]];
    let tetra = shells(&[(&mirrored, &flipped)]);
    assert_eq!(
        side_of_shell(&tetra, ix(0), p(3, -4, 5), &mut meter()).unwrap(),
        Some(Side::Inside)
    );
    // A face x + z = 12, its normal without y: the move up z decides, out of the wedge.
    let wedge: [[i64; 3]; 6] = [
        [0, 0, 0],
        [12, 0, 0],
        [0, 0, 12],
        [0, 12, 0],
        [12, 12, 0],
        [0, 12, 12],
    ];
    let wedge_faces: [&[u32]; 5] = [
        &[0, 2, 1],
        &[3, 4, 5],
        &[0, 1, 4, 3],
        &[1, 2, 5, 4],
        &[2, 0, 3, 5],
    ];
    let body = shells(&[(&wedge, &wedge_faces)]);
    assert_eq!(
        side_of_shell(&body, ix(0), p(5, 3, 7), &mut meter()).unwrap(),
        Some(Side::Outside)
    );
    // And the wedge the other way up, x − z = 0 from below: the move up z enters it.
    let under: [[i64; 3]; 6] = [
        [0, 0, 0],
        [12, 0, 0],
        [12, 0, 12],
        [0, 12, 0],
        [12, 12, 0],
        [12, 12, 12],
    ];
    let body = shells(&[(&under, &wedge_faces)]);
    let v = volume(&body, &mut meter()).unwrap();
    assert!(v.is_negative() || !v.is_zero(), "{v:?}");
    assert_eq!(
        side_of_shell(&body, ix(0), p(7, 3, 7), &mut meter()).unwrap(),
        Some(Side::Inside)
    );
}

#[test]
fn one_shell_lies_wholly_on_one_side_of_another() {
    let outer = corners([0, 0, 0], [10, 10, 10]);
    let inner = corners([2, 2, 2], [8, 8, 8]);
    let apart = corners([20, 20, 20], [30, 30, 30]);
    let body = shells(&[(&outer, &BOX), (&inner, &INSIDE_OUT), (&apart, &BOX)]);
    let side = |a: u32, b: u32| shell_side(&body, ix(a), ix(b), &mut meter()).unwrap();
    assert_eq!(side(1, 0), Some(Side::Inside));
    assert_eq!(side(0, 1), Some(Side::Outside));
    assert_eq!(side(2, 0), Some(Side::Outside));
    assert_eq!(side(0, 2), Some(Side::Outside));
    // Every vertex of a shell is a vertex of itself: nothing decides.
    assert_eq!(side(0, 0), None);
}

#[test]
fn a_vertex_two_shells_share_is_passed_over() {
    // The cavity's corner (0, 0, 0) is the outer box's: its next vertex decides.
    let outer = corners([0, 0, 0], [10, 10, 10]);
    let inner = corners([0, 0, 0], [4, 4, 4]);
    let body = shells(&[(&outer, &BOX), (&inner, &INSIDE_OUT)]);
    assert_eq!(
        shell_side(&body, ix(1), ix(0), &mut meter()).unwrap(),
        Some(Side::Inside)
    );
}

#[test]
fn what_is_not_a_polygon_is_refused_by_name() {
    let w = wall();
    // A curved edge.
    let curved = rebuilt(|b| {
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let c = Circle::new([0.0; 3], 5.0, frame, (0.0, 1.0)).unwrap();
        b.arenas.curves.set(ix(0), CurveKind::Circle(c)).unwrap();
    });
    let edge = (0..curved.arenas().edges.len())
        .find(|&e| curved.arenas().edges.get(ix(e)).unwrap().curve.row() == 0)
        .unwrap();
    assert!(matches!(
        volume(&curved, &mut meter()),
        Err(QueryError::Curved(e)) if e.row() == edge
    ));
    // An edge with no vertices: a closed edge is curved here.
    let closed = rebuilt(|b| {
        b.arenas.edges.start.set(ix(0), None).unwrap();
    });
    assert!(matches!(
        volume(&closed, &mut meter()),
        Err(QueryError::Curved(e)) if e.row() == 0
    ));
    let open_end = rebuilt(|b| {
        b.arenas.edges.end.set(ix(0), None).unwrap();
    });
    assert!(matches!(
        volume(&open_end, &mut meter()),
        Err(QueryError::Curved(e)) if e.row() == 0
    ));
    // A face on a sphere.
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
        .unwrap() as u32;
    assert_eq!(
        face_area(&round, ix(face), &mut meter()),
        Err(QueryError::NotPlanar(ix(face)))
    );
    let (_, tight) = bounding_box(&round, &mut meter()).unwrap().unwrap();
    assert!(!tight);
    // The side of a shell needs polygons too.
    assert!(matches!(
        side_of_shell(&curved, ix(0), P3::new(1, 1, 1).unwrap(), &mut meter()),
        Err(QueryError::Curved(_))
    ));
    assert!(matches!(
        shell_side(&curved, ix(0), ix(0), &mut meter()),
        Err(QueryError::Curved(_))
    ));
    // A face with no loop, a body that is not a solid, and rows out of range.
    let sheet = many_faces(2);
    assert_eq!(
        face_area(&sheet, ix(0), &mut meter()),
        Err(QueryError::Loopless(ix(0)))
    );
    assert_eq!(volume(&sheet, &mut meter()), Err(QueryError::NotSolid));
    assert_eq!(
        face_area(&w, ix(99), &mut meter()),
        Err(QueryError::NoSuchRow)
    );
    assert_eq!(point(&w, ix(99)), Err(QueryError::NoSuchRow));
    assert_eq!(
        shell_volume(&w, ix(9), &mut meter()),
        Err(QueryError::NoSuchRow)
    );
    assert_eq!(
        loop_area(&w, ix(99), &mut meter()),
        Err(QueryError::NoSuchRow)
    );
    for e in [
        QueryError::NoSuchRow,
        QueryError::Ring,
        QueryError::Curved(ix(1)),
        QueryError::NotPlanar(ix(2)),
        QueryError::Loopless(ix(3)),
        QueryError::NotSolid,
        QueryError::Overflow,
    ] {
        assert_ne!(e.to_string(), "");
    }
}

#[test]
fn a_ring_that_does_not_end_is_reported_never_followed() {
    // A fin ring that never returns to the loop's first fin: fin 1 points at itself.
    let ring = rebuilt(|b| {
        b.arenas.fins.next.set(ix(1), ix(1)).unwrap();
    });
    assert_eq!(
        loop_area(&ring, ix(0), &mut meter()),
        Err(QueryError::Ring)
    );
    // A list of faces that comes back on itself.
    let faces_ring = rebuilt(|b| {
        b.arenas.faces.next.set(ix(1), Some(ix(0))).unwrap();
    });
    assert_eq!(
        shell_volume(&faces_ring, ix(0), &mut meter()),
        Err(QueryError::Ring)
    );
    // A list of loops that comes back on itself.
    let loops_ring = rebuilt(|b| {
        b.arenas.loops.next.set(ix(0), Some(ix(0))).unwrap();
    });
    assert_eq!(
        face_area(&loops_ring, ix(0), &mut meter()),
        Err(QueryError::Ring)
    );
}

#[test]
fn every_query_is_charged_and_stops_with_its_meter() {
    let w = wall();
    let mut m = meter();
    volume(&w, &mut m).unwrap();
    // One unit per face, loop and fin: 6, 6 and 24.
    assert_eq!(m.used().work, 6 + 6 + 24);
    let mut m = meter();
    faces(&w, &mut m).unwrap();
    assert_eq!(m.used().work, 6 + 6);
    let mut m = meter();
    bounding_box(&w, &mut m).unwrap();
    assert_eq!(m.used().work, 8 + 6 + 24);
    let mut m = meter();
    side_of_shell(&w, ix(0), P3::new(1, 1, 1).unwrap(), &mut m).unwrap();
    assert_eq!(m.used().work, 6 + 6 + 24);
    for work in 0..40 {
        let mut tight = WorkMeter::new(Budget::of_work(work));
        match volume(&w, &mut tight) {
            Ok(v) => assert_eq!(v, volume(&w, &mut meter()).unwrap()),
            Err(QueryError::Stopped(s)) => {
                assert!(matches!(s.reason(), Stop::OverBudget(_)));
                assert_ne!(QueryError::Stopped(s).to_string(), "");
            }
            Err(e) => panic!("{work}: {e:?}"),
        }
        assert!(matches!(
            faces(&w, &mut WorkMeter::new(Budget::of_work(work))),
            Ok(_) | Err(QueryError::Stopped(_))
        ));
        assert!(matches!(
            bounding_box(&w, &mut WorkMeter::new(Budget::of_work(work))),
            Ok(_) | Err(QueryError::Stopped(_))
        ));
        assert!(matches!(
            side_of_shell(
                &w,
                ix(0),
                P3::new(1, 1, 1).unwrap(),
                &mut WorkMeter::new(Budget::of_work(work))
            ),
            Ok(_) | Err(QueryError::Stopped(_))
        ));
    }
}

#[test]
fn a_query_leaves_its_body_as_it_was() {
    let w = wall();
    let before = format!("{:?}", w.arenas().points.iter().collect::<Vec<_>>());
    volume(&w, &mut meter()).unwrap();
    faces(&w, &mut meter()).unwrap();
    bounding_box(&w, &mut meter()).unwrap();
    assert_eq!(
        before,
        format!("{:?}", w.arenas().points.iter().collect::<Vec<_>>())
    );
}
