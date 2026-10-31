//! The flat reference (`geom_test::flat_reference`, piece G136) against cases worked by hand.
//!
//! The board's done-when: "Brute-force rational boolean, imprint, cells, plane section, In-On-Out,
//! volumes and centroids, all-pairs clash and exact first contact, by a different author, each
//! checked on at least 50 hand cases and compared on every pull request."
//!
//! Every answer here comes from a closed form a person can check on paper, never from the code under
//! test: overlaps of boxes are products of interval overlaps, corner cuts are k³/6, and so on. Each
//! test counts its cases and requires at least 50 of its kind.
//!
//! No kernel boolean exists yet, so this file is the comparison for now: G140 sets the kernel's flat
//! booleans beside the same cases.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cloned_ref_to_slice_refs,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use core::cmp::Ordering;

use geom_test::flat_reference::{
    Cell, Op, Plane, Point, Polygon, Q, ReferenceError, Relation, Solid, Where, arrangement,
    boolean, clash, clashes, first_contact, imprint, int, labelled_cells, point, ratio, region,
    section, twice_area,
};

fn cuboid(low: [i64; 3], high: [i64; 3]) -> Solid {
    let [a, b, c] = low;
    let [d, e, f] = high;
    Solid::cuboid(&point(a, b, c), &point(d, e, f)).unwrap()
}

/// The overlap of `[a, b]` and `[c, d]`.
fn overlap(a: i64, b: i64, c: i64, d: i64) -> i64 {
    (b.min(d) - a.max(c)).max(0)
}

fn size(low: [i64; 3], high: [i64; 3]) -> i64 {
    (high[0] - low[0]) * (high[1] - low[1]) * (high[2] - low[2])
}

fn common(a: Bounds, b: Bounds) -> i64 {
    (0..3).fold(1, |all, i| all * overlap(a.0[i], a.1[i], b.0[i], b.1[i]))
}

/// A box by its corners.
type Bounds = ([i64; 3], [i64; 3]);

/// Boxes against a 4 by 3 by 2 box at many offsets, whose overlaps are worked by hand.
fn pairs() -> Vec<(Bounds, Bounds)> {
    let a = ([0, 0, 0], [4, 3, 2]);
    let mut out = Vec::new();
    for dx in [-5, -2, -1, 0, 1, 3, 4] {
        for dy in [-1, 0, 2] {
            for dz in [0, 1] {
                let low = [dx, dy, dz];
                let high = [dx + 2, dy + 3, dz + 2];
                out.push((a, (low, high)));
            }
        }
    }
    out
}

#[test]
fn every_boolean_of_two_boxes() {
    let mut cases = 0;
    for (a, b) in pairs() {
        let (sa, sb) = (cuboid(a.0, a.1), cuboid(b.0, b.1));
        let (va, vb) = (size(a.0, a.1), size(b.0, b.1));
        let i = common(a, b);
        for (op, want) in [
            (Op::Intersection, i),
            (Op::Union, va + vb - i),
            (Op::Difference, va - i),
            (Op::Exclusive, va + vb - 2 * i),
        ] {
            assert_eq!(
                boolean(&sa, &sb, op).unwrap().volume(),
                int(want),
                "{a:?} {b:?} {op:?}"
            );
            cases += 1;
        }
    }
    assert!(cases >= 50, "{cases}");
}

#[test]
fn booleans_of_cut_corners_and_tilted_planes() {
    let mut cases = 0;
    // A corner tetrahedron x + y + z <= k inside a box: k^3/6, whatever the box beyond it.
    for k in 1..=12 {
        let tet = Solid::tetrahedron([
            point(0, 0, 0),
            point(k, 0, 0),
            point(0, k, 0),
            point(0, 0, k),
        ])
        .unwrap();
        let block = cuboid([0, 0, 0], [20, 20, 20]);
        assert_eq!(
            boolean(&block, &tet, Op::Intersection).unwrap().volume(),
            ratio(k * k * k, 6)
        );
        assert_eq!(
            boolean(&block, &tet, Op::Difference).unwrap().volume(),
            int(8000) - ratio(k * k * k, 6)
        );
        cases += 2;
    }
    // A prism on a right triangle of legs a and b, height h, cut in half across its height.
    for (a, b, h) in [(3, 4, 2), (5, 2, 6), (1, 1, 2), (7, 3, 4)] {
        let base = [(int(0), int(0)), (int(a), int(0)), (int(0), int(b))];
        let prism = Solid::prism(&base, &int(0), &int(h)).unwrap();
        let half = cuboid([-1, -1, 0], [a + 1, b + 1, h / 2]);
        assert_eq!(
            boolean(&prism, &half, Op::Intersection).unwrap().volume(),
            ratio(a * b * (h / 2), 2)
        );
        cases += 1;
    }
    // A non-convex L-shaped prism: 3 by 3 less a 2 by 2 corner, height 2, and a box through it.
    let l = [
        (int(0), int(0)),
        (int(3), int(0)),
        (int(3), int(1)),
        (int(1), int(1)),
        (int(1), int(3)),
        (int(0), int(3)),
    ];
    let solid = Solid::prism(&l, &int(0), &int(2)).unwrap();
    assert_eq!(region(&solid).unwrap().volume(), int(10));
    let through = cuboid([-1, -1, 0], [4, 4, 1]);
    assert_eq!(
        boolean(&solid, &through, Op::Intersection)
            .unwrap()
            .volume(),
        int(5)
    );
    let hole = cuboid([0, 0, 0], [1, 1, 2]);
    assert_eq!(
        boolean(&solid, &hole, Op::Difference).unwrap().volume(),
        int(8)
    );
    cases += 3;
    // Rational coordinates: a box of side 1/3 in a box of side 1.
    for n in 2..=9_u64 {
        let side = ratio(1, n);
        let small =
            Solid::cuboid(&point(0, 0, 0), &[side.clone(), side.clone(), side.clone()]).unwrap();
        let unit = cuboid([0, 0, 0], [1, 1, 1]);
        assert_eq!(
            boolean(&unit, &small, Op::Difference).unwrap().volume(),
            int(1) - &side * &side * &side
        );
        cases += 1;
    }
    assert!(cases >= 30, "{cases}");
}

#[test]
fn a_shared_face_or_edge_leaves_nothing() {
    let a = cuboid([0, 0, 0], [2, 2, 2]);
    for b in [
        cuboid([2, 0, 0], [4, 2, 2]),
        cuboid([2, 2, 0], [4, 4, 2]),
        cuboid([2, 2, 2], [4, 4, 4]),
    ] {
        let r = boolean(&a, &b, Op::Intersection).unwrap();
        assert_eq!(r.cells().len(), 0);
        assert_eq!(r.centroid(), None);
    }
}

#[test]
fn cells_are_counted_and_fill_the_box() {
    let mut cases = 0;
    // a + b + c planes across a box cut it into (a + 1)(b + 1)(c + 1) cells.
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..3 {
                let mut planes = Vec::new();
                for (axis, n) in [(0, a), (1, b), (2, c)] {
                    for k in 1..=n {
                        let mut normal = point(0, 0, 0);
                        normal[axis] = int(1);
                        planes.push(Plane::new(normal, int(k)).unwrap());
                    }
                }
                let cells = arrangement(&planes, &point(0, 0, 0), &point(5, 5, 5)).unwrap();
                assert_eq!(cells.len() as i64, (a + 1) * (b + 1) * (c + 1));
                let total: Q = cells.iter().map(Cell::volume).fold(int(0), |s, v| s + v);
                assert_eq!(total, int(125));
                cases += 1;
            }
        }
    }
    // Two planes crossing make four cells; three in general position eight.
    let x = Plane::new([int(1), int(1), int(0)], int(2)).unwrap();
    let y = Plane::new([int(1), int(-1), int(0)], int(0)).unwrap();
    let z = Plane::new([int(0), int(0), int(1)], int(2)).unwrap();
    let box4 = (point(0, 0, 0), point(4, 4, 4));
    assert_eq!(
        arrangement(&[x.clone()], &box4.0, &box4.1).unwrap().len(),
        2
    );
    assert_eq!(
        arrangement(&[x.clone(), y.clone()], &box4.0, &box4.1)
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        arrangement(&[x.clone(), y, z], &box4.0, &box4.1)
            .unwrap()
            .len(),
        8
    );
    // One plane twice is one plane, however it is written.
    let twice = Plane::new([int(2), int(2), int(0)], int(4)).unwrap();
    assert_eq!(arrangement(&[x, twice], &box4.0, &box4.1).unwrap().len(), 2);
    cases += 4;
    // Two overlapping boxes: the cells in each are labelled, and the volumes in each are the boxes'.
    for (a, b) in pairs() {
        let (sa, sb) = (cuboid(a.0, a.1), cuboid(b.0, b.1));
        let labelled = labelled_cells(&[&sa, &sb], &[]).unwrap();
        let in_first: Q = labelled
            .iter()
            .filter(|(_, l)| l[0])
            .map(|(c, _)| c.volume())
            .fold(int(0), |s, v| s + v);
        assert_eq!(in_first, int(size(a.0, a.1)));
        cases += 1;
    }
    assert!(cases >= 50, "{cases}");
}

#[test]
fn the_boundary_of_a_box_is_its_six_faces() {
    let mut cases = 0;
    for (x, y, z) in [(1, 1, 1), (2, 3, 4), (5, 1, 2), (3, 3, 1), (7, 2, 2)] {
        let r = region(&cuboid([0, 0, 0], [x, y, z])).unwrap();
        let areas = r.areas();
        assert_eq!(areas.len(), 6, "{x} {y} {z}");
        let total = areas.values().fold(int(0), |s, v| s + v);
        assert_eq!(total, int(4 * (x * y + y * z + z * x)));
        cases += 1;
    }
    // A union's shared wall is gone: 2 by 1 by 1 from two cubes.
    let u = boolean(
        &cuboid([0, 0, 0], [1, 1, 1]),
        &cuboid([1, 0, 0], [2, 1, 1]),
        Op::Union,
    )
    .unwrap();
    let total = u.areas().values().fold(int(0), |s, v| s + v);
    assert_eq!(total, int(4 * (2 + 1 + 2)));
    cases += 1;
    // Every piece faces outward.
    for face in region(&cuboid([0, 0, 0], [2, 2, 2])).unwrap().boundary() {
        let plane = face.plane();
        let c = point(1, 1, 1);
        assert_eq!(plane.side(&c), core::cmp::Ordering::Less);
    }
    assert!(cases >= 6);
}

#[test]
fn in_on_out() {
    let mut cases = 0;
    let unit = |s: i64| cuboid([0, 0, 0], [s, s, s]);
    // Every grid point of a 4-cube, and one each way past it.
    let cube = unit(4);
    for x in -1..=5 {
        for y in -1..=5 {
            for z in -1..=5 {
                let inside = [x, y, z].iter().all(|v| (1..=3).contains(v));
                let closed = [x, y, z].iter().all(|v| (0..=4).contains(v));
                let want = if inside {
                    Where::In
                } else if closed {
                    Where::On
                } else {
                    Where::Out
                };
                assert_eq!(cube.locate(&point(x, y, z)), want, "{x} {y} {z}");
                cases += 1;
            }
        }
    }
    // A ring frame: a 6 by 6 prism's outline as a non-convex shape with the notch at one side.
    let notch = [
        (int(0), int(0)),
        (int(6), int(0)),
        (int(6), int(6)),
        (int(4), int(6)),
        (int(4), int(2)),
        (int(2), int(2)),
        (int(2), int(6)),
        (int(0), int(6)),
    ];
    let u = Solid::prism(&notch, &int(0), &int(1)).unwrap();
    for (p, want) in [
        (point(3, 4, 0), Where::Out),
        (point(1, 3, 0), Where::On),
        (point(3, 1, 0), Where::On),
        (point(5, 3, 0), Where::On),
        (point(4, 3, 0), Where::On),
        (point(2, 3, 0), Where::On),
        (point(3, 2, 0), Where::On),
    ] {
        assert_eq!(u.locate(&p), want, "{p:?}");
        cases += 1;
    }
    let half = ratio(1, 2);
    for (x, y, want) in [
        (1, 3, Where::In),
        (3, 1, Where::In),
        (5, 3, Where::In),
        (3, 4, Where::Out),
        (3, 3, Where::Out),
    ] {
        assert_eq!(u.locate(&[int(x), int(y), half.clone()]), want, "{x} {y}");
        cases += 1;
    }
    // A tetrahedron: x + y + z < 3 inside, equal on, greater out, with the coordinates positive.
    let tet = Solid::tetrahedron([
        point(0, 0, 0),
        point(3, 0, 0),
        point(0, 3, 0),
        point(0, 0, 3),
    ])
    .unwrap();
    let third = ratio(1, 3);
    assert_eq!(
        tet.locate(&[third.clone(), third.clone(), third.clone()]),
        Where::In
    );
    assert_eq!(tet.locate(&point(1, 1, 1)), Where::On);
    assert_eq!(tet.locate(&point(2, 1, 1)), Where::Out);
    assert_eq!(tet.locate(&point(0, 0, 0)), Where::On);
    assert_eq!(tet.locate(&point(-1, 0, 0)), Where::Out);
    cases += 5;
    assert!(cases >= 50, "{cases}");
}

#[test]
fn volumes_and_centroids() {
    let mut cases = 0;
    // A box's centroid is its centre; a corner tetrahedron's, a quarter of the way along each leg.
    for (x, y, z) in [(2, 4, 6), (1, 1, 1), (3, 5, 7), (10, 2, 4), (6, 6, 2)] {
        for shift in [0, 1, -3, 5, 11] {
            let r = region(&cuboid(
                [shift, shift, shift],
                [shift + x, shift + y, shift + z],
            ))
            .unwrap();
            assert_eq!(r.volume(), int(x * y * z));
            let want = [
                ratio(2 * shift + x, 2),
                ratio(2 * shift + y, 2),
                ratio(2 * shift + z, 2),
            ];
            assert_eq!(r.centroid().unwrap(), want);
            cases += 2;
        }
    }
    for (a, b, c) in [(1, 1, 1), (4, 8, 12), (2, 3, 5), (6, 6, 6), (9, 1, 3)] {
        let tet = Solid::tetrahedron([
            point(0, 0, 0),
            point(a, 0, 0),
            point(0, b, 0),
            point(0, 0, c),
        ])
        .unwrap();
        let r = region(&tet).unwrap();
        assert_eq!(r.volume(), ratio(a * b * c, 6));
        assert_eq!(
            r.centroid().unwrap(),
            [ratio(a, 4), ratio(b, 4), ratio(c, 4)]
        );
        cases += 2;
    }
    // A notched solid: the centroid is the volume-weighted mean of the two boxes that make it.
    let parts = [([0, 0, 0], [4, 2, 2]), ([0, 2, 0], [2, 4, 2])];
    let joined = boolean(
        &cuboid(parts[0].0, parts[0].1),
        &cuboid(parts[1].0, parts[1].1),
        Op::Union,
    )
    .unwrap();
    assert_eq!(joined.volume(), int(16 + 8));
    assert_eq!(
        joined.centroid().unwrap(),
        [ratio(16 * 2 + 8, 24), ratio(16 + 8 * 3, 24), int(1)]
    );
    cases += 2;
    assert!(cases >= 50, "{cases}");
}

#[test]
fn sections_are_cut_where_they_should_be() {
    let mut cases = 0;
    // A box cut by planes across each axis: the section's area is the other two sides' product.
    let (x, y, z) = (4, 6, 8);
    let solid = cuboid([0, 0, 0], [x, y, z]);
    for (axis, across, length) in [(0, y * z, x), (1, x * z, y), (2, x * y, z)] {
        for k in -1..=length + 1 {
            let mut normal = point(0, 0, 0);
            normal[axis] = int(1);
            let plane = Plane::new(normal, int(k)).unwrap();
            let pieces = section(&solid, &plane).unwrap();
            let want = if (0..=length).contains(&k) {
                2 * across
            } else {
                0
            };
            assert_eq!(twice_area(&pieces, &plane), int(want), "{axis} {k}");
            cases += 1;
        }
    }
    // A corner tetrahedron cut by z = h: a right triangle of legs k - h.
    for (k, h) in (3..=8).flat_map(|k| (0..k).map(move |h| (k, h))) {
        let tet = Solid::tetrahedron([
            point(0, 0, 0),
            point(k, 0, 0),
            point(0, k, 0),
            point(0, 0, k),
        ])
        .unwrap();
        let plane = Plane::new(point(0, 0, 1), int(h)).unwrap();
        let pieces = section(&tet, &plane).unwrap();
        assert_eq!(
            twice_area(&pieces, &plane),
            int((k - h) * (k - h)),
            "{k} {h}"
        );
        cases += 1;
    }
    // A slanted cut through a cube along a diagonal: the plane x = y through a 2-cube has a
    // rectangle 2 by 2√2, so its area vector along (1, -1, 0) has size 8 in twice-area units of
    // the canonical normal, whose length is √2.
    let cube = cuboid([0, 0, 0], [2, 2, 2]);
    let diagonal = Plane::new([int(1), int(-1), int(0)], int(0)).unwrap();
    let pieces = section(&cube, &diagonal).unwrap();
    assert_eq!(twice_area(&pieces, &diagonal), int(8));
    cases += 1;
    // The notched prism cut across its notch has two rectangles.
    let notch = [
        (int(0), int(0)),
        (int(6), int(0)),
        (int(6), int(6)),
        (int(4), int(6)),
        (int(4), int(2)),
        (int(2), int(2)),
        (int(2), int(6)),
        (int(0), int(6)),
    ];
    let u = Solid::prism(&notch, &int(0), &int(1)).unwrap();
    let across = Plane::new(point(0, 1, 0), int(4)).unwrap();
    assert_eq!(
        twice_area(&section(&u, &across).unwrap(), &across),
        int(2 * (2 + 2))
    );
    cases += 1;
    assert!(cases >= 50, "{cases}");
}

#[test]
fn imprints_are_where_the_faces_cross() {
    let total = |segments: &[(Point, Point)]| {
        // Axis-aligned segments: each length is the sum of its coordinate differences.
        segments.iter().fold(int(0), |sum, (a, b)| {
            let mut length = int(0);
            for i in 0..3 {
                let d = &b[i] - &a[i];
                length += if d < int(0) { -d } else { d };
            }
            sum + length
        })
    };
    let mut cases = 0;
    // Boxes piercing a box's top: each square's outline, 4s long, is imprinted on the top.
    let block = cuboid([0, 0, 0], [30, 30, 2]);
    for s in 1..=9 {
        for at in [1, 3, 5, 7, 9, 11] {
            let post = cuboid([at, at, 1], [at + s, at + s, 5]);
            let got = imprint(&block, &post);
            assert_eq!(total(&got), int(4 * s), "{s} {at}");
            cases += 1;
        }
    }
    // Apart boxes leave nothing.
    let far = cuboid([30, 30, 30], [31, 31, 31]);
    assert_eq!(imprint(&block, &far).len(), 0);
    cases += 1;
    // A tilted tetrahedron corner through the top face at z = 2, of the block above: a triangle
    // with legs k - 2, and its hypotenuse is counted by coordinate differences.
    let tet = Solid::tetrahedron([
        point(3, 3, 0),
        point(9, 3, 0),
        point(3, 9, 0),
        point(3, 3, 6),
    ])
    .unwrap();
    let plate = cuboid([0, 0, 2], [20, 20, 3]);
    let got = imprint(&plate, &tet);
    assert_ne!(got.len(), 0);
    assert!(got.iter().any(|(a, b)| a[2] == int(2) && b[2] == int(2)));
    cases += 1;
    assert!(cases >= 50, "{cases}");
}

#[test]
fn clash_sorts_every_pair() {
    let mut cases = 0;
    let a = ([0, 0, 0], [4, 4, 4]);
    let sa = cuboid(a.0, a.1);
    for dx in -6..=6 {
        for dy in [-5, -1, 0, 3] {
            let b = ([dx, dy, 0], [dx + 2, dy + 2, 4]);
            let sb = cuboid(b.0, b.1);
            let shared = common(a, b);
            let touches = dx + 2 >= 0 && dx <= 4 && dy + 2 >= 0 && dy <= 4;
            let want = if shared == size(b.0, b.1) {
                Relation::Contains
            } else if shared > 0 {
                Relation::Overlapping
            } else if touches {
                Relation::Touching
            } else {
                Relation::Apart
            };
            assert_eq!(clash(&sa, &sb).unwrap(), want, "{dx} {dy}");
            cases += 1;
        }
    }
    assert_eq!(
        clash(&cuboid([1, 1, 1], [2, 2, 2]), &sa).unwrap(),
        Relation::Within
    );
    // A row of boxes, each touching the next by a face: only neighbours clash.
    let row: Vec<Solid> = (0..6).map(|i| cuboid([i, 0, 0], [i + 1, 1, 1])).collect();
    assert_eq!(
        clashes(&row).unwrap(),
        [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)]
    );
    // Corner to corner counts as touching, not apart.
    assert_eq!(
        clash(&cuboid([0, 0, 0], [1, 1, 1]), &cuboid([1, 1, 1], [2, 2, 2])).unwrap(),
        Relation::Touching
    );
    cases += 2;
    assert!(cases >= 50, "{cases}");
}

#[test]
fn first_contact_is_exact() {
    let mut cases = 0;
    let still = cuboid([10, 0, 0], [12, 4, 4]);
    // A box sliding along x: it arrives after (10 - 2 - start) of its travel.
    for start in -9..=5 {
        for travel in [10, 15, 20, 40] {
            let moving = cuboid([start, 1, 1], [start + 2, 3, 3]);
            let gap = 10 - (start + 2);
            let got = first_contact(&moving, &still, &point(travel, 0, 0)).unwrap();
            let want = (gap <= travel).then(|| ratio(gap, travel as u64));
            assert_eq!(got, want, "{start} {travel}");
            cases += 1;
        }
    }
    // Already touching or overlapping is t = 0.
    let touching = cuboid([8, 0, 0], [10, 4, 4]);
    assert_eq!(
        first_contact(&touching, &still, &point(1, 0, 0)).unwrap(),
        Some(int(0))
    );
    cases += 1;
    // Moving away, or passing clear above, never meets.
    let clear = cuboid([0, 0, 5], [2, 4, 7]);
    assert_eq!(
        first_contact(&clear, &still, &point(40, 0, 0)).unwrap(),
        None
    );
    let moving = cuboid([0, 0, 0], [2, 4, 4]);
    assert_eq!(
        first_contact(&moving, &still, &point(-5, 0, 0)).unwrap(),
        None
    );
    cases += 2;
    // Edge on edge: a tilted motion makes corners meet first, at an exact fraction.
    let block = cuboid([0, 0, 0], [1, 1, 1]);
    let target = cuboid([3, 3, 0], [4, 4, 1]);
    assert_eq!(
        first_contact(&block, &target, &point(4, 4, 0)).unwrap(),
        Some(ratio(1, 2))
    );
    cases += 1;
    // A vertex of a tetrahedron reaching a box's face.
    let tet = Solid::tetrahedron([
        point(0, 0, 0),
        point(-1, 0, 0),
        point(0, -1, 0),
        point(0, 0, -1),
    ])
    .unwrap();
    let wall = cuboid([3, -2, -2], [4, 2, 2]);
    assert_eq!(
        first_contact(&tet, &wall, &point(6, 0, 0)).unwrap(),
        Some(ratio(1, 2))
    );
    cases += 1;
    assert!(cases >= 50, "{cases}");
}

/// A tetrahedron with one edge along y at height `z` and the other along x, two below it.
fn roof(z: i64) -> Solid {
    Solid::tetrahedron([
        point(0, -3, z),
        point(0, 3, z),
        point(-3, 0, z - 2),
        point(3, 0, z - 2),
    ])
    .unwrap()
}

/// The same with its edges turned over: x at height `z`, y two above it.
fn trough(z: i64) -> Solid {
    Solid::tetrahedron([
        point(-3, 0, z),
        point(3, 0, z),
        point(0, -3, z + 2),
        point(0, 3, z + 2),
    ])
    .unwrap()
}

#[test]
fn edges_meet_when_no_corner_is_near_a_face() {
    // The moving solid's lowest edge runs along x and the still one's highest along y, so they
    // first touch at one point in the middle of both: no corner reaches a face at that moment.
    let (still, moving) = (roof(0), trough(1));
    for (by, want) in [
        (point(0, 0, -2), Some(ratio(1, 2))),
        (point(0, 0, -4), Some(ratio(1, 4))),
        (point(0, 0, -1), Some(int(1))),
        (point(1, 1, -2), Some(ratio(1, 2))),
        (point(-2, 3, -3), Some(ratio(1, 3))),
        (point(0, 0, 2), None),
    ] {
        assert_eq!(first_contact(&moving, &still, &by).unwrap(), want, "{by:?}");
    }
    // Slide along x past the end of the edge: at that height it has gone by, and never touches.
    assert_eq!(
        first_contact(&moving, &still, &point(14, 0, -2)).unwrap(),
        None
    );
    // Short of reaching, it is still in the air at the end of its travel.
    assert_eq!(
        first_contact(&trough(3), &still, &point(0, 0, -1)).unwrap(),
        None
    );
}

#[test]
fn a_still_corner_reaches_a_moving_face() {
    // The still solid's apex meets the moving box's underside, and no corner of the box is near.
    let still = Solid::tetrahedron([
        point(0, 0, 0),
        point(-1, -1, -2),
        point(3, -1, -2),
        point(-1, 3, -2),
    ])
    .unwrap();
    let moving = cuboid([-5, -5, 2], [5, 5, 3]);
    for (by, want) in [
        (point(0, 0, -4), Some(ratio(1, 2))),
        (point(0, 0, -8), Some(ratio(1, 4))),
        (point(1, 0, -4), Some(ratio(1, 2))),
        (point(0, 0, -1), None),
        (point(0, 0, 4), None),
    ] {
        assert_eq!(first_contact(&moving, &still, &by).unwrap(), want, "{by:?}");
    }
}

#[test]
fn a_tetrahedron_faces_out_however_its_corners_are_given() {
    let corners = [
        point(0, 0, 0),
        point(2, 0, 0),
        point(0, 2, 0),
        point(0, 0, 2),
    ];
    let inner = [ratio(1, 2), ratio(1, 2), ratio(1, 2)];
    for order in [[0, 1, 2, 3], [1, 0, 2, 3], [0, 2, 3, 1], [3, 2, 1, 0]] {
        let tet = Solid::tetrahedron(order.map(|i: usize| corners[i].clone())).unwrap();
        for face in tet.faces() {
            assert_eq!(face.plane().side(&inner), Ordering::Less, "{order:?}");
        }
    }
}

#[test]
fn a_region_holds_its_cells() {
    let cube = cuboid([0, 0, 0], [2, 2, 2]);
    let whole = region(&cube).unwrap();
    assert_eq!(whole.cells().len(), 1);
    assert_eq!(whole.cells()[0].volume(), int(8));
    let other = cuboid([1, 1, 1], [3, 3, 3]);
    let both = boolean(&cube, &other, Op::Intersection).unwrap();
    assert_eq!(both.cells().len(), 1);
    assert_eq!(both.cells()[0].volume(), int(1));
    let cut = boolean(&cube, &other, Op::Difference).unwrap();
    assert_eq!(cut.cells().len(), 7);
}

#[test]
fn a_slanted_face_counts_along_its_canonical_normal() {
    let tet = Solid::tetrahedron([
        point(0, 0, 0),
        point(2, 0, 0),
        point(0, 2, 0),
        point(0, 0, 2),
    ])
    .unwrap();
    let areas = region(&tet).unwrap().areas();
    assert_eq!(areas.len(), 4);
    // Its area vector is (4, 4, 4), and the canonical normal (1, 1, 1): four of them.
    let slanted = Plane::new(point(2, 2, 2), int(4)).unwrap();
    assert_eq!(areas[&slanted.canonical()], int(4));
    let floor = Plane::new(point(0, 0, 1), int(0)).unwrap();
    assert_eq!(areas[&floor.canonical()], int(4));
}

#[test]
fn a_section_faces_the_way_its_plane_does() {
    let cube = cuboid([0, 0, 0], [4, 4, 4]);
    let far = point(100, 100, 100);
    for (normal, offset, area) in [
        (point(0, 0, 1), 2, 32),
        (point(0, 0, -1), -2, 32),
        (point(1, 1, 0), 4, 32),
        (point(-1, -1, 0), -4, 32),
    ] {
        let plane = Plane::new(normal, int(offset)).unwrap();
        let pieces = section(&cube, &plane).unwrap();
        assert!(!pieces.is_empty());
        for piece in &pieces {
            assert_eq!(piece.plane().side(&far), plane.side(&far), "{plane:?}");
        }
        assert_eq!(twice_area(&pieces, &plane), int(area), "{plane:?}");
        let turned: Vec<Polygon> = pieces.iter().map(Polygon::flipped).collect();
        assert_eq!(twice_area(&turned, &plane), int(area), "{plane:?}");
    }
}

#[test]
fn an_imprint_is_sorted_joined_and_has_length() {
    // A plate across an L: its face runs under the notch's edge lines, one stretch from end to end.
    let base = [(0, 0), (4, 0), (4, 2), (2, 2), (2, 4), (0, 4)].map(|(x, y)| (int(x), int(y)));
    let ell = Solid::prism(&base, &int(0), &int(2)).unwrap();
    let plate = cuboid([-1, 1, 1], [5, 3, 3]);
    let segments = imprint(&ell, &plate);
    assert!(
        segments.contains(&(point(0, 1, 2), point(4, 1, 2))),
        "{segments:?}"
    );
    assert!(segments.iter().all(|(x, y)| x < y), "{segments:?}");
    // Two faces whose stretches of one line meet at a point: nothing of no length.
    let cube = cuboid([0, 0, 0], [2, 2, 2]);
    let touching = cuboid([2, 1, 1], [4, 3, 3]);
    let segments = imprint(&cube, &touching);
    assert!(segments.iter().all(|(x, y)| x < y), "{segments:?}");
}

#[test]
fn bad_shapes_are_refused() {
    let tri = |a: Point, b: Point, c: Point| Polygon::new(vec![a, b, c]);
    assert_eq!(
        Polygon::new(vec![point(0, 0, 0), point(1, 0, 0)]),
        Err(ReferenceError::NoArea)
    );
    assert_eq!(
        tri(point(0, 0, 0), point(1, 0, 0), point(2, 0, 0)),
        Err(ReferenceError::NoArea)
    );
    assert_eq!(
        Polygon::new(vec![
            point(0, 0, 0),
            point(1, 0, 0),
            point(1, 1, 0),
            point(0, 1, 1)
        ]),
        Err(ReferenceError::NotPlanar)
    );
    let face = tri(point(0, 0, 0), point(1, 0, 0), point(0, 1, 0)).unwrap();
    assert_eq!(Solid::new(vec![face]), Err(ReferenceError::NotClosed));
    assert_eq!(Solid::new(Vec::new()), Err(ReferenceError::NotClosed));
    assert_eq!(
        Plane::new(point(0, 0, 0), int(1)),
        Err(ReferenceError::ZeroNormal)
    );
    assert_eq!(
        Solid::tetrahedron([
            point(0, 0, 0),
            point(1, 0, 0),
            point(0, 1, 0),
            point(1, 1, 0)
        ]),
        Err(ReferenceError::NoArea)
    );
    assert!(labelled_cells(&[], &[]).is_err());
    assert!(ReferenceError::NotClosed.to_string().contains("pair"));
    assert!(ReferenceError::NoArea.to_string().contains("area"));
    assert!(ReferenceError::NotPlanar.to_string().contains("plane"));
    assert!(ReferenceError::ZeroNormal.to_string().contains("normal"));
}

#[test]
fn a_moved_solid_is_the_same_solid_elsewhere() {
    let a = cuboid([0, 0, 0], [2, 3, 4]);
    let b = a.moved(&point(5, -1, 2));
    assert_eq!(b, cuboid([5, -1, 2], [7, 2, 6]));
    let tilted = Plane::new(point(1, 1, 1), int(3)).unwrap();
    assert_eq!(tilted.normal(), &point(1, 1, 1));
    assert_eq!(
        tilted.canonical(),
        Plane::new(point(2, 2, 2), int(6)).unwrap().canonical()
    );
}
