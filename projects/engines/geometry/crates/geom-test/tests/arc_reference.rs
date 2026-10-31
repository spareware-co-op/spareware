//! The one-root arrangement reference (`geom_test::arc_reference`, piece G191) against cases worked
//! by hand.
//!
//! The board's done-when: "One-root number type; line-and-arc arrangement, booleans, offsets and
//! boundary identity checker on it; 50 hand cases; compared with geom-2d on every pull request."
//!
//! Every answer here comes from a closed form a person can check on paper, never from the code under
//! test: boxes overlap in boxes, two circles of radius 5 six apart cross at (3, ±4), and so on. The
//! tests count their cases and require 50.
//!
//! No kernel 2D boolean exists yet, so this file is the comparison for now: `geom-2d`'s booleans and
//! offsets join these cases when they land.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use core::cmp::Ordering;

use geom_test::arc_reference::{
    Arrangement, Carrier, Location, OneRoot, Op, Piece, Point, Q, ReferenceError, Region, Step,
    boolean, boundary_identity, boundary_sum, offset, root, same_boundary,
};

fn q(n: i64) -> Q {
    Q::from(n)
}

fn f(n: i64, d: u64) -> Q {
    Q::from(n) / Q::from(d)
}

fn pt(x: i64, y: i64) -> (Q, Q) {
    (q(x), q(y))
}

fn box_at(x: i64, y: i64, w: i64, h: i64) -> Region {
    Region::polygon(&[pt(x, y), pt(x + w, y), pt(x + w, y + h), pt(x, y + h)]).unwrap()
}

fn disc(x: i64, y: i64, r: i64) -> Region {
    Region::disc(pt(x, y), q(r * r)).unwrap()
}

fn arc(to: (i64, i64), centre: (i64, i64), ccw: bool) -> Step {
    Step::Arc {
        to: pt(to.0, to.1),
        centre: pt(centre.0, centre.1),
        ccw,
    }
}

fn line(x: i64, y: i64) -> Step {
    Step::Line(q(x), q(y))
}

/// `a` and `b` under each operator, the identity checked, and each answer compared with the one
/// stated.
fn all_three(a: &Region, b: &Region, union: &Region, inter: &Region, less: &Region) -> usize {
    let (u, i, d) = (
        boolean(a, b, Op::Union),
        boolean(a, b, Op::Intersection),
        boolean(a, b, Op::Difference),
    );
    assert!(same_boundary(&u, union), "union");
    assert!(same_boundary(&i, inter), "intersection");
    assert!(same_boundary(&d, less), "difference");
    assert!(boundary_identity(a, b, &u, &i), "the identity");
    assert!(Region::new(u.pieces().to_vec()).is_ok(), "the union closes");
    assert!(
        Region::new(i.pieces().to_vec()).is_ok(),
        "the intersection closes"
    );
    assert!(
        Region::new(d.pieces().to_vec()).is_ok(),
        "the difference closes"
    );
    3
}

#[test]
fn one_root_numbers_are_compared_exactly() {
    let root_of = |n: i64| OneRoot::new(q(0), q(1), q(n)).unwrap();
    assert_eq!(root_of(2).sign(), Ordering::Greater);
    assert_eq!(
        OneRoot::new(q(1), q(-1), q(2)).unwrap().sign(),
        Ordering::Less
    );
    assert_eq!(
        OneRoot::new(q(-1), q(1), q(2)).unwrap().sign(),
        Ordering::Greater
    );
    assert_eq!(
        OneRoot::new(q(-2), q(1), q(2)).unwrap().sign(),
        Ordering::Less
    );
    assert_eq!(
        OneRoot::new(q(2), q(-1), q(4)).unwrap().sign(),
        Ordering::Equal
    );
    assert_eq!(
        OneRoot::new(q(0), q(0), q(2)).unwrap().sign(),
        Ordering::Equal
    );
    assert!(root_of(2) < root_of(3));
    assert!(root_of(5) > OneRoot::rational(q(2)));
    assert!(root_of(5) < OneRoot::rational(f(9, 4)));
    // √8 and 2√2 are one value on two roots, and so are √2 + √8 and √18.
    assert_eq!(root_of(8), OneRoot::new(q(0), q(2), q(2)).unwrap());
    let (a, b) = (
        OneRoot::new(q(1), q(1), q(2)).unwrap(),
        OneRoot::new(q(1), q(2), f(1, 2)).unwrap(),
    );
    assert_eq!(a, b);
    // 1 + √2 is above √5, and below 1 + √3.
    assert!(a > root_of(5));
    assert!(a < OneRoot::new(q(1), q(1), q(3)).unwrap());
    // Three terms of opposite sign: (3 + √2)² = 11 + 6√2, which is between 19 and 20.
    assert!(OneRoot::new(q(3), q(1), q(2)).unwrap() < root_of(20));
    assert!(OneRoot::new(q(3), q(1), q(2)).unwrap() > root_of(19));
    assert!(OneRoot::new(q(-3), q(1), q(2)).unwrap() > OneRoot::new(q(0), q(-1), q(3)).unwrap());
    // A square folds: √(9/4) is 3/2.
    assert_eq!(root_of(4).as_rational(), Some(&q(2)));
    assert_eq!(
        OneRoot::new(q(1), q(2), f(9, 4)).unwrap().as_rational(),
        Some(&q(4))
    );
    assert_eq!(root_of(2).as_rational(), None);
    assert_eq!(root(&f(9, 4)), Some(f(3, 2)));
    assert_eq!(root(&q(2)), None);
    assert_eq!(root(&q(-4)), None);
    assert_eq!(root(&f(2, 9)), None);
    assert_eq!(root(&q(0)), Some(q(0)));
    assert_eq!(
        OneRoot::new(q(0), q(1), q(-1)).unwrap_err(),
        ReferenceError::NegativeRoot
    );
}

#[test]
fn refused_shapes_say_why() {
    assert_eq!(
        Piece::segment(pt(1, 1), pt(1, 1)).unwrap_err(),
        ReferenceError::Degenerate
    );
    assert_eq!(
        Region::polygon(&[pt(0, 0), pt(1, 0)]).unwrap_err(),
        ReferenceError::Degenerate
    );
    assert_eq!(
        Region::polygon(&[]).unwrap_err(),
        ReferenceError::Degenerate
    );
    assert_eq!(
        Region::disc(pt(0, 0), q(0)).unwrap_err(),
        ReferenceError::Degenerate
    );
    assert_eq!(
        Region::disc(pt(0, 0), q(-1)).unwrap_err(),
        ReferenceError::Degenerate
    );
    let off = Point::at(q(1), q(1));
    let on = Point::at(q(1), q(0));
    assert_eq!(
        Piece::arc(pt(0, 0), q(1), &off, &on, true).unwrap_err(),
        ReferenceError::OffCircle
    );
    assert_eq!(
        Piece::arc(pt(0, 0), q(1), &on, &off, true).unwrap_err(),
        ReferenceError::OffCircle
    );
    assert_eq!(
        Piece::arc(pt(0, 0), q(0), &on, &on, true).unwrap_err(),
        ReferenceError::Degenerate
    );
    let open = vec![Piece::segment(pt(0, 0), pt(1, 0)).unwrap()];
    assert_eq!(Region::new(open).unwrap_err(), ReferenceError::Open);
    let not_back = [line(1, 0), line(1, 1)];
    assert_eq!(
        Region::path(&pt(0, 0), &not_back).unwrap_err(),
        ReferenceError::Open
    );
    assert_eq!(ReferenceError::Open.to_string(), "chains that do not close");
    assert_eq!(
        ReferenceError::NegativeRoot.to_string(),
        "the square root of a negative number"
    );
    assert_eq!(
        ReferenceError::OffCircle.to_string(),
        "a point off its circle"
    );
    assert_eq!(
        ReferenceError::Degenerate.to_string(),
        "a degenerate piece or polygon"
    );
    assert_eq!(
        ReferenceError::NotRational.to_string(),
        "an offset outside the one-root numbers"
    );
}

#[test]
fn a_piece_tells_its_carrier_and_ends() {
    let s = Piece::segment(pt(2, 4), pt(4, 8)).unwrap();
    assert_eq!(s.start(), &Point::at(q(2), q(4)));
    assert_eq!(s.end(), &Point::at(q(4), q(8)));
    assert_eq!(s.reversed().start(), s.end());
    // y = 2x in its one form: x − y/2 = 0.
    assert_eq!(
        s.carrier(),
        &Carrier::Line {
            a: q(1),
            b: f(-1, 2),
            c: q(0)
        }
    );
    // A level line keeps b as the first that is not zero.
    assert_eq!(
        Piece::segment(pt(0, 3), pt(5, 3)).unwrap().carrier(),
        &Carrier::Line {
            a: q(0),
            b: q(1),
            c: q(3)
        }
    );
    let p = Point::new(
        OneRoot::new(q(1), q(1), q(2)).unwrap(),
        OneRoot::rational(q(0)),
    );
    assert_eq!(p.x(), &OneRoot::new(q(1), q(1), q(2)).unwrap());
    assert_eq!(p.y(), &OneRoot::rational(q(0)));
    // One whole circle is two pieces, cut at its left and right extremes; a quarter is one.
    assert_eq!(disc(0, 0, 3).pieces().len(), 2);
    let quarter = Piece::arc(
        pt(0, 0),
        q(25),
        &Point::at(q(5), q(0)),
        &Point::at(q(0), q(5)),
        true,
    )
    .unwrap();
    assert_eq!(quarter.len(), 1);
    // From the top round the left to the bottom is one piece's worth of each half.
    let half = Piece::arc(
        pt(0, 0),
        q(25),
        &Point::at(q(0), q(5)),
        &Point::at(q(0), q(-5)),
        true,
    )
    .unwrap();
    assert_eq!(half.len(), 2);
    // Three quarters, the short way refused: clockwise from the top to the left passes the right.
    let long = Piece::arc(
        pt(0, 0),
        q(25),
        &Point::at(q(0), q(5)),
        &Point::at(q(-5), q(0)),
        false,
    )
    .unwrap();
    assert_eq!(long.len(), 2);
    assert_eq!(long[0].start(), &Point::at(q(0), q(5)));
    assert_eq!(long[1].end(), &Point::at(q(-5), q(0)));
    // From lower right, counter-clockwise the long way to lower left: three pieces.
    let round = Piece::arc(
        pt(0, 0),
        q(25),
        &Point::at(q(3), q(-4)),
        &Point::at(q(-3), q(-4)),
        true,
    )
    .unwrap();
    assert_eq!(round.len(), 3);
}

#[test]
fn a_point_is_located_exactly() {
    let square = box_at(0, 0, 4, 4);
    assert_eq!(square.locate(&q(2), &q(2)), Location::In);
    assert_eq!(square.locate(&q(5), &q(2)), Location::Out);
    assert_eq!(square.locate(&q(-1), &q(2)), Location::Out);
    assert_eq!(square.locate(&q(4), &q(2)), Location::On);
    assert_eq!(square.locate(&q(0), &q(0)), Location::On);
    assert_eq!(square.locate(&q(4), &q(5)), Location::Out);
    // On a level with a corner, the first ray meets it, and the next slope is taken.
    assert_eq!(square.locate(&q(2), &q(4)), Location::On);
    assert_eq!(square.locate(&q(-3), &q(0)), Location::Out);
    assert_eq!(square.locate(&q(1), &q(1)), Location::In);
    let round = disc(0, 0, 5);
    assert_eq!(round.locate(&q(3), &q(4)), Location::On);
    assert_eq!(round.locate(&q(3), &q(3)), Location::In);
    assert_eq!(round.locate(&q(4), &q(4)), Location::Out);
    assert_eq!(round.locate(&q(-3), &q(-4)), Location::On);
    assert_eq!(round.locate(&q(0), &q(5)), Location::On);
    assert_eq!(round.locate(&q(0), &q(6)), Location::Out);
    // A ring has a hole.
    let ring = boolean(&disc(0, 0, 5), &disc(0, 0, 3), Op::Difference);
    assert_eq!(ring.locate(&q(0), &q(0)), Location::Out);
    assert_eq!(ring.locate(&q(4), &q(0)), Location::In);
    assert_eq!(ring.locate(&q(3), &q(0)), Location::On);
    assert_eq!(Region::empty().locate(&q(0), &q(0)), Location::Out);
    assert!(Region::empty().is_empty());
    assert!(!ring.is_empty());
    // The circle of radius √2 has the rational point (1, 1).
    let small = Region::disc(pt(0, 0), q(2)).unwrap();
    assert_eq!(small.locate(&q(1), &q(1)), Location::On);
    assert_eq!(small.locate(&f(1, 2), &f(1, 2)), Location::In);
    assert_eq!(small.locate(&f(3, 2), &q(0)), Location::Out);
}

#[test]
fn arrangements_count_as_euler_says() {
    let seg = |a, b, c, d| Piece::segment(pt(a, b), pt(c, d)).unwrap();
    // (vertices, edges, components, faces)
    let cases: Vec<(Vec<Piece>, [usize; 4])> = vec![
        (vec![seg(0, 0, 1, 0)], [2, 1, 1, 1]),
        (vec![seg(0, 0, 2, 2), seg(0, 2, 2, 0)], [5, 4, 1, 1]),
        (vec![seg(0, 0, 1, 0), seg(5, 5, 6, 5)], [4, 2, 2, 1]),
        // Two collinear overlapping segments are three edges in a row.
        (vec![seg(0, 0, 4, 0), seg(2, 0, 6, 0)], [4, 3, 1, 1]),
        // One inside the other, and one given twice.
        (vec![seg(0, 0, 4, 0), seg(1, 0, 2, 0)], [4, 3, 1, 1]),
        (vec![seg(0, 0, 4, 0), seg(4, 0, 0, 0)], [2, 1, 1, 1]),
        // A T: one end on the other's middle.
        (vec![seg(0, 0, 4, 0), seg(2, 0, 2, 3)], [4, 3, 1, 1]),
        (box_at(0, 0, 2, 2).pieces().to_vec(), [4, 4, 1, 2]),
        // A square with both diagonals: four triangles and the outside.
        (
            [
                box_at(0, 0, 2, 2).pieces().to_vec(),
                vec![seg(0, 0, 2, 2), seg(0, 2, 2, 0)],
            ]
            .concat(),
            [5, 8, 1, 5],
        ),
        (disc(0, 0, 5).pieces().to_vec(), [2, 2, 1, 2]),
        // Two discs crossing: each circle has four vertices, two shared, which cut each into four.
        (
            [disc(0, 0, 5).pieces(), disc(6, 0, 5).pieces()].concat(),
            [6, 8, 1, 4],
        ),
        // Two apart: nothing shared.
        (
            [disc(0, 0, 2).pieces(), disc(9, 0, 2).pieces()].concat(),
            [4, 4, 2, 3],
        ),
        // Tangent outside, at (5, 0), which is an extreme of each.
        (
            [disc(0, 0, 5).pieces(), disc(8, 0, 3).pieces()].concat(),
            [3, 4, 1, 3],
        ),
        // Tangent inside at (5, 0): the small circle's right extreme is the large one's.
        (
            [disc(0, 0, 5).pieces(), disc(2, 0, 3).pieces()].concat(),
            [3, 4, 1, 3],
        ),
        // Tangent at the top, (0, 5), which is an extreme of neither.
        (
            [disc(0, 0, 5).pieces(), disc(0, 7, 2).pieces()].concat(),
            [5, 6, 1, 3],
        ),
        // A chord: the line cuts each half of the circle once.
        (
            [disc(0, 0, 5).pieces().to_vec(), vec![seg(-3, 4, 3, 4)]].concat(),
            [4, 5, 1, 3],
        ),
        // A tangent line touching the top.
        (
            [disc(0, 0, 5).pieces().to_vec(), vec![seg(-7, 5, 7, 5)]].concat(),
            [5, 5, 1, 2],
        ),
        // A line through the centre, past both extremes.
        (
            [disc(0, 0, 5).pieces().to_vec(), vec![seg(-7, 0, 7, 0)]].concat(),
            [4, 5, 1, 3],
        ),
        // A long line missing the circle.
        (
            [disc(0, 0, 5).pieces().to_vec(), vec![seg(-7, 6, 7, 6)]].concat(),
            [4, 3, 2, 2],
        ),
    ];
    let mut count = 0;
    for (pieces, [v, e, c, faces]) in &cases {
        let a = Arrangement::of(pieces);
        assert_eq!(
            [a.vertices(), a.edges(), a.components(), a.faces()],
            [*v, *e, *c, *faces],
            "case {count}"
        );
        // Each component has one more face boundary than its faces beyond the outside.
        assert_eq!(a.cycles().len(), e + 2 * c - v, "case {count}");
        count += 1;
    }
    assert!(count >= 19);
}

#[test]
fn a_face_is_walked_with_it_on_the_left() {
    // A square's two cycles have four edges each; the walk is by the order around each corner.
    let square = Arrangement::of(box_at(0, 0, 2, 2).pieces());
    assert_eq!(
        square.cycles().iter().map(Vec::len).collect::<Vec<_>>(),
        [4, 4]
    );
    // A tangent arc leaves a line in one direction, and curvature orders them: inside the corner of
    // a rounded box the cycle goes line, arc, line, arc.
    let rounded = Region::path(
        &pt(1, 0),
        &[
            line(3, 0),
            arc((4, 1), (3, 1), true),
            line(4, 3),
            arc((3, 4), (3, 3), true),
            line(1, 4),
            arc((0, 3), (1, 3), true),
            line(0, 1),
            arc((1, 0), (1, 1), true),
        ],
    )
    .unwrap();
    let a = Arrangement::of(rounded.pieces());
    assert_eq!(a.cycles().len(), 2);
    assert_eq!(a.cycles()[0].len(), rounded.pieces().len());
    // Two circles tangent inside at (5, 0), and a line along the common tangent from there: the
    // three leave together upward, and the order is the larger circle, the smaller, then the line.
    let pieces = [
        disc(0, 0, 5).pieces().to_vec(),
        disc(2, 0, 3).pieces().to_vec(),
        vec![Piece::segment(pt(5, 0), pt(5, 4)).unwrap()],
    ]
    .concat();
    let a = Arrangement::of(&pieces);
    // Faces: outside, the crescent, the small disc; the line hangs in the outside face.
    assert_eq!((a.vertices(), a.edges(), a.faces()), (4, 5, 3));
    assert_eq!(a.cycles().len(), 3);
    // The outside cycle runs up the line and back, so one cycle has 2 + 2 edges.
    let mut sizes: Vec<usize> = a.cycles().iter().map(Vec::len).collect();
    sizes.sort_unstable();
    assert_eq!(sizes, [2, 4, 4]);
}

#[test]
fn boxes_overlap_in_boxes() {
    let mut count = 0;
    let a = box_at(0, 0, 4, 4);
    for dx in -5..=5 {
        for dy in -5..=5 {
            let b = box_at(dx, dy, 4, 4);
            let (x0, x1) = (i64::max(0, dx), i64::min(4, dx + 4));
            let (y0, y1) = (i64::max(0, dy), i64::min(4, dy + 4));
            let i = boolean(&a, &b, Op::Intersection);
            let u = boolean(&a, &b, Op::Union);
            let d = boolean(&a, &b, Op::Difference);
            if x0 < x1 && y0 < y1 {
                assert!(
                    same_boundary(&i, &box_at(x0, y0, x1 - x0, y1 - y0)),
                    "{dx} {dy}"
                );
            } else {
                assert!(i.is_empty(), "{dx} {dy}");
            }
            assert!(boundary_identity(&a, &b, &u, &i), "{dx} {dy}");
            // A − B and A ∩ B sum to A, and A − B and B sum to the union.
            assert!(
                boundary_sum(&[(1, &d), (1, &i), (-1, &a)]).is_empty(),
                "{dx} {dy}"
            );
            assert!(
                boundary_sum(&[(1, &d), (1, &b), (-1, &u)]).is_empty(),
                "{dx} {dy}"
            );
            count += 1;
        }
    }
    assert!(count >= 50);
}

#[test]
fn hand_cases_with_arcs() {
    let mut cases = 0;
    let big = disc(0, 0, 5);

    // Two circles of radius 5 six apart cross at (3, ±4).
    let other = disc(6, 0, 5);
    let lens = Region::path(
        &pt(3, -4),
        &[arc((3, 4), (0, 0), true), arc((3, -4), (6, 0), true)],
    )
    .unwrap();
    let both = Region::path(
        &pt(3, -4),
        &[arc((3, 4), (6, 0), true), arc((3, -4), (0, 0), true)],
    )
    .unwrap();
    let crescent = Region::path(
        &pt(3, 4),
        &[arc((3, -4), (0, 0), true), arc((3, 4), (6, 0), false)],
    )
    .unwrap();
    cases += all_three(&big, &other, &both, &lens, &crescent);

    // One inside the other: the union is the large one, and the difference a ring.
    let ring = Region::new(
        [
            big.pieces().to_vec(),
            disc(0, 0, 3).pieces().iter().map(Piece::reversed).collect(),
        ]
        .concat(),
    )
    .unwrap();
    cases += all_three(&big, &disc(0, 0, 3), &big, &disc(0, 0, 3), &ring);
    let hole = Region::empty();
    cases += all_three(&disc(0, 0, 3), &big, &big, &disc(0, 0, 3), &hole);

    // Apart: the union is both.
    let apart = disc(20, 0, 2);
    let pair = Region::new([big.pieces(), apart.pieces()].concat()).unwrap();
    cases += all_three(&big, &apart, &pair, &hole, &big);

    // Tangent outside at (8, 0): nothing in common.
    let touching = disc(11, 0, 3);
    let joined = Region::new([disc(5, 0, 3).pieces(), touching.pieces()].concat()).unwrap();
    cases += all_three(&disc(5, 0, 3), &touching, &joined, &hole, &disc(5, 0, 3));

    // Tangent inside at (5, 0).
    let inner = disc(2, 0, 3);
    let moon = Region::new(
        [
            big.pieces().to_vec(),
            inner.pieces().iter().map(Piece::reversed).collect(),
        ]
        .concat(),
    )
    .unwrap();
    cases += all_three(&big, &inner, &big, &inner, &moon);

    // The same disc twice.
    cases += all_three(&big, &big, &big, &big, &hole);

    // A disc cut by a half-plane through its centre: a half disc each way.
    let right = box_at(0, -9, 9, 18);
    let half = Region::path(&pt(0, -5), &[arc((0, 5), (0, 0), true), line(0, -5)]).unwrap();
    let left_half = Region::path(&pt(0, 5), &[arc((0, -5), (0, 0), true), line(0, 5)]).unwrap();
    let keyhole = Region::path(
        &pt(0, -9),
        &[
            line(9, -9),
            line(9, 9),
            line(0, 9),
            line(0, 5),
            arc((0, -5), (0, 0), true),
            line(0, -9),
        ],
    )
    .unwrap();
    cases += all_three(&big, &right, &keyhole, &half, &left_half);

    // A chord at y = 4 between (−3, 4) and (3, 4): the cap above it.
    let high = box_at(-9, 4, 18, 9);
    let cap = Region::path(&pt(3, 4), &[arc((-3, 4), (0, 0), true), line(3, 4)]).unwrap();
    let rest = Region::path(&pt(-3, 4), &[arc((3, 4), (0, 0), true), line(-3, 4)]).unwrap();
    let dome = Region::path(
        &pt(-9, 4),
        &[
            line(-3, 4),
            arc((3, 4), (0, 0), true),
            line(9, 4),
            line(9, 13),
            line(-9, 13),
            line(-9, 4),
        ],
    )
    .unwrap();
    cases += all_three(&big, &high, &dome, &cap, &rest);

    // A square inscribed in the disc, its corners on the circle: four caps are left.
    let inscribed = Region::polygon(&[pt(3, -4), pt(4, 3), pt(-3, 4), pt(-4, -3)]).unwrap();
    let inside = boolean(&inscribed, &big, Op::Intersection);
    assert!(same_boundary(&inside, &inscribed));
    assert!(same_boundary(&boolean(&inscribed, &big, Op::Union), &big));
    assert!(boolean(&inscribed, &big, Op::Difference).is_empty());
    let caps = boolean(&big, &inscribed, Op::Difference);
    assert!(boundary_sum(&[(1, &caps), (1, &inscribed), (-1, &big)]).is_empty());
    // Four chords, and the circle cut at the four corners and its two extremes.
    assert_eq!(caps.pieces().len(), 10);
    cases += 4;

    // A square whose sides touch the disc from outside at (±5, 0) and (0, ±5).
    let around = box_at(-5, -5, 10, 10);
    let corners = boolean(&around, &big, Op::Difference);
    assert!(same_boundary(
        &boolean(&around, &big, Op::Intersection),
        &big
    ));
    assert!(same_boundary(&boolean(&around, &big, Op::Union), &around));
    assert!(boundary_sum(&[(1, &corners), (1, &big), (-1, &around)]).is_empty());
    assert_eq!(corners.locate(&f(49, 10), &f(49, 10)), Location::In);
    assert_eq!(corners.locate(&q(0), &q(0)), Location::Out);
    cases += 3;

    // A segment-bounded box whose crossing with the circle is irrational: x = 1 meets r = 2 at ±√3.
    let two = disc(0, 0, 2);
    let tall = box_at(1, -9, 9, 18);
    let u = boolean(&two, &tall, Op::Union);
    let i = boolean(&two, &tall, Op::Intersection);
    assert!(boundary_identity(&two, &tall, &u, &i));
    assert_eq!(i.locate(&f(3, 2), &q(0)), Location::In);
    assert_eq!(i.locate(&f(1, 2), &q(0)), Location::Out);
    assert_eq!(i.locate(&q(2), &q(0)), Location::On);
    // Its two line pieces and its arcs: the chord, and the arc split at the right extreme.
    assert_eq!(i.pieces().len(), 3);
    cases += 2;

    // Radii that are irrational: r² = 2 about the origin and about (2, 0) cross at (1, ±1).
    let s = Region::disc(pt(0, 0), q(2)).unwrap();
    let t = Region::disc(pt(2, 0), q(2)).unwrap();
    let lens = Region::path(
        &pt(1, -1),
        &[arc((1, 1), (0, 0), true), arc((1, -1), (2, 0), true)],
    )
    .unwrap();
    let both = Region::path(
        &pt(1, -1),
        &[arc((1, 1), (2, 0), true), arc((1, -1), (0, 0), true)],
    )
    .unwrap();
    let crescent = Region::path(
        &pt(1, 1),
        &[arc((1, -1), (0, 0), true), arc((1, 1), (2, 0), false)],
    )
    .unwrap();
    cases += all_three(&s, &t, &both, &lens, &crescent);

    // Crossings with two different roots: r = 2 and r = 3 about the origin, cut by x = 1.
    let wide = disc(0, 0, 3);
    let band = boolean(&wide, &two, Op::Difference);
    let cut = boolean(&band, &tall, Op::Intersection);
    let off = boolean(&band, &tall, Op::Difference);
    assert!(boundary_sum(&[(1, &cut), (1, &off), (-1, &band)]).is_empty());
    assert_eq!(cut.locate(&q(0), &q(0)), Location::Out);
    assert_eq!(cut.locate(&f(5, 2), &q(0)), Location::In);
    assert_eq!(off.locate(&f(-5, 2), &q(0)), Location::In);
    assert_eq!(off.locate(&f(5, 2), &q(0)), Location::Out);
    // The ring's piece on x = 1 runs from √3 to √8 each side.
    let up = cut
        .pieces()
        .iter()
        .filter(|p| matches!(p.carrier(), Carrier::Line { .. }))
        .count();
    assert_eq!(up, 2);
    cases += 2;

    // Boxes against a disc at every grid offset: only the laws are stated.
    for x in -7..=3 {
        for y in -7..=3 {
            let b = box_at(x, y, 4, 4);
            let u = boolean(&big, &b, Op::Union);
            let i = boolean(&big, &b, Op::Intersection);
            let d = boolean(&big, &b, Op::Difference);
            assert!(boundary_identity(&big, &b, &u, &i), "{x} {y}");
            assert!(
                boundary_sum(&[(1, &d), (1, &i), (-1, &big)]).is_empty(),
                "{x} {y}"
            );
            assert!(Region::new(u.pieces().to_vec()).is_ok(), "{x} {y}");
            cases += 1;
        }
    }
    assert!(cases >= 50, "{cases}");
}

#[test]
fn an_identity_with_a_fault_in_it_fails() {
    let (a, b) = (box_at(0, 0, 4, 4), box_at(2, 2, 4, 4));
    let (u, i) = (
        boolean(&a, &b, Op::Union),
        boolean(&a, &b, Op::Intersection),
    );
    assert!(boundary_identity(&a, &b, &u, &i));
    assert!(!boundary_identity(&a, &b, &a, &i));
    assert!(!boundary_identity(&a, &b, &u, &b));
    assert!(!same_boundary(&a, &b));
    assert!(same_boundary(&a, &a));
    // One piece turned round counts twice.
    let turned = Region::new(a.pieces().iter().map(Piece::reversed).collect()).unwrap();
    assert!(!same_boundary(&a, &turned));
    assert_eq!(boundary_sum(&[(1, &a), (1, &turned)]).len(), 0);
    assert_eq!(boundary_sum(&[(1, &a), (-1, &turned)]).len(), 4);
    assert_eq!(boundary_sum(&[(2, &a)])[0].1, 2);
    assert_eq!(boundary_sum(&[(1, &turned)])[0].1, 1);
}

#[test]
fn offsets_are_minkowski_sums() {
    let mut cases = 0;
    // A box grown by 1: its corners round off.
    let rounded = |x: i64, y: i64, w: i64, h: i64, r: i64| {
        Region::path(
            &pt(x + r, y),
            &[
                line(x + w - r, y),
                arc((x + w, y + r), (x + w - r, y + r), true),
                line(x + w, y + h - r),
                arc((x + w - r, y + h), (x + w - r, y + h - r), true),
                line(x + r, y + h),
                arc((x, y + h - r), (x + r, y + h - r), true),
                line(x, y + r),
                arc((x + r, y), (x + r, y + r), true),
            ],
        )
        .unwrap()
    };
    for r in 1..=3 {
        let grown = offset(&box_at(0, 0, 4, 2), &q(r)).unwrap();
        assert!(
            same_boundary(&grown, &rounded(-r, -r, 4 + 2 * r, 2 + 2 * r, r)),
            "{r}"
        );
        cases += 1;
    }
    // A disc grows and shrinks to a disc; shrunk past its radius, nothing is left.
    assert!(same_boundary(
        &offset(&disc(0, 0, 5), &q(2)).unwrap(),
        &disc(0, 0, 7)
    ));
    assert!(same_boundary(
        &offset(&disc(0, 0, 5), &q(-2)).unwrap(),
        &disc(0, 0, 3)
    ));
    assert!(same_boundary(
        &offset(&disc(2, 3, 5), &q(5)).unwrap(),
        &disc(2, 3, 10)
    ));
    assert!(offset(&disc(0, 0, 5), &q(-6)).unwrap().is_empty());
    assert!(offset(&disc(0, 0, 5), &q(-5)).unwrap().is_empty());
    cases += 5;
    // Shrinking a box keeps its corners sharp, and shrinking past half its width empties it.
    assert!(same_boundary(
        &offset(&box_at(0, 0, 6, 4), &q(-1)).unwrap(),
        &box_at(1, 1, 4, 2)
    ));
    assert!(offset(&box_at(0, 0, 6, 4), &q(-3)).unwrap().is_empty());
    assert!(offset(&box_at(0, 0, 6, 4), &q(-2)).unwrap().is_empty());
    cases += 3;
    // No offset at all.
    assert!(same_boundary(
        &offset(&box_at(0, 0, 2, 2), &q(0)).unwrap(),
        &box_at(0, 0, 2, 2)
    ));
    cases += 1;
    // A box on a 3-4-5 slant has rational unit normals.
    let slant = Region::polygon(&[pt(0, 0), pt(4, 3), pt(1, 7), pt(-3, 4)]).unwrap();
    let grown = offset(&slant, &q(5)).unwrap();
    let shrunk = offset(&grown, &q(-5)).unwrap();
    assert!(same_boundary(&shrunk, &slant));
    assert_eq!(grown.locate(&f(1, 2), &q(0)), Location::In);
    assert_eq!(grown.locate(&q(0), &q(-5)), Location::On);
    assert_eq!(grown.locate(&q(0), &q(-6)), Location::Out);
    cases += 1;
    // A hole grows inward: a ring grown by 1 is thicker on both sides.
    let ring = boolean(&disc(0, 0, 5), &disc(0, 0, 3), Op::Difference);
    let thick = offset(&ring, &q(1)).unwrap();
    assert!(same_boundary(
        &thick,
        &boolean(&disc(0, 0, 6), &disc(0, 0, 2), Op::Difference)
    ));
    let closed = offset(&ring, &q(3)).unwrap();
    assert!(same_boundary(&closed, &disc(0, 0, 8)));
    cases += 2;
    // An L grown: its inner corner stays sharp, and shrunk its outer corners stay sharp, and its
    // inner corner rounds.
    let ell =
        Region::polygon(&[pt(0, 0), pt(6, 0), pt(6, 2), pt(2, 2), pt(2, 6), pt(0, 6)]).unwrap();
    let grown = offset(&ell, &q(1)).unwrap();
    let want = Region::path(
        &pt(0, -1),
        &[
            line(6, -1),
            arc((7, 0), (6, 0), true),
            line(7, 2),
            arc((6, 3), (6, 2), true),
            line(3, 3),
            line(3, 6),
            arc((2, 7), (2, 6), true),
            line(0, 7),
            arc((-1, 6), (0, 6), true),
            line(-1, 0),
            arc((0, -1), (0, 0), true),
        ],
    )
    .unwrap();
    assert!(same_boundary(&grown, &want));
    // Shrunk, an L with arms 4 wide keeps its outer corners sharp, and its inner corner rounds.
    let ell =
        Region::polygon(&[pt(0, 0), pt(8, 0), pt(8, 4), pt(4, 4), pt(4, 8), pt(0, 8)]).unwrap();
    let want = Region::path(
        &pt(1, 1),
        &[
            line(7, 1),
            line(7, 3),
            line(4, 3),
            arc((3, 4), (4, 4), false),
            line(3, 7),
            line(1, 7),
            line(1, 1),
        ],
    )
    .unwrap();
    assert!(same_boundary(&offset(&ell, &q(-1)).unwrap(), &want));
    // With arms 2 wide, 1 in from each side leaves no area along the arms, which regularising
    // drops, and in the elbow a curved triangle against the inner corner's disc.
    let narrow =
        Region::polygon(&[pt(0, 0), pt(6, 0), pt(6, 2), pt(2, 2), pt(2, 6), pt(0, 6)]).unwrap();
    let elbow = Region::path(
        &pt(1, 1),
        &[line(2, 1), arc((1, 2), (2, 2), false), line(1, 1)],
    )
    .unwrap();
    assert!(same_boundary(&offset(&narrow, &q(-1)).unwrap(), &elbow));
    cases += 2;
    // A half disc, its arc grown and its straight side moved, its corners rounded by the discs.
    let half = Region::path(&pt(0, -3), &[arc((0, 3), (0, 0), true), line(0, -3)]).unwrap();
    let grown = offset(&half, &q(2)).unwrap();
    assert_eq!(grown.locate(&q(5), &q(0)), Location::On);
    assert_eq!(grown.locate(&q(-2), &q(0)), Location::On);
    assert_eq!(grown.locate(&q(0), &q(5)), Location::On);
    assert_eq!(grown.locate(&q(3), &q(4)), Location::On);
    assert_eq!(grown.locate(&q(-2), &q(-3)), Location::On);
    assert_eq!(grown.locate(&q(0), &q(0)), Location::In);
    cases += 1;
    // An arc reaching past its centre: a half disc of radius 1 grown by 2.
    let small = Region::path(&pt(0, -1), &[arc((0, 1), (0, 0), true), line(0, -1)]).unwrap();
    let grown = offset(&small, &q(2)).unwrap();
    assert_eq!(grown.locate(&q(3), &q(0)), Location::On);
    assert_eq!(grown.locate(&q(-2), &q(0)), Location::On);
    cases += 1;
    // What the numbers cannot hold is refused.
    let skew = Region::polygon(&[pt(0, 0), pt(1, 0), pt(0, 1)]).unwrap();
    assert_eq!(
        offset(&skew, &q(1)).unwrap_err(),
        ReferenceError::NotRational
    );
    let irrational = Region::disc(pt(0, 0), q(2)).unwrap();
    assert_eq!(
        offset(&irrational, &q(1)).unwrap_err(),
        ReferenceError::NotRational
    );
    let crossing = boolean(&disc(0, 0, 2), &box_at(1, -9, 9, 18), Op::Intersection);
    assert_eq!(
        offset(&crossing, &q(1)).unwrap_err(),
        ReferenceError::NotRational
    );
    cases += 3;
    assert!(cases >= 19, "{cases}");
}
