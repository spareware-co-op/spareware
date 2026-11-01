//! A face splits into convex pieces along its own edges' planes: every plane is one it had, the
//! areas add up exactly, and nothing depends on anything but the order of the sides (G125, 11.3).

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
use geom_num::{NumError, P3};
use geom_poly::convex::{ConvexError, Piece, convex_pieces};
use geom_run::{Budget, WorkMeter};
use num_bigint::BigInt;
use proptest::prelude::*;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// z = 0, facing +z.
fn floor() -> Plane {
    Plane::through(p(0, 0, 0), p(1, 0, 0), p(0, 1, 0)).unwrap()
}

/// The vertical plane through `a` and `b`, positive to the left of a to b seen from above.
fn left_of(a: (i64, i64), b: (i64, i64)) -> Plane {
    Plane::through(p(a.0, a.1, 0), p(b.0, b.1, 0), p(a.0, a.1, 1)).map_or_else(
        |e| panic!("{e:?}"),
        |q| {
            let [x, y, z] = q.n();
            Plane::new([-x, -y, -z], -q.d()).unwrap()
        },
    )
}

/// A ring through `corners`, inside on the left.
fn ring(corners: &[(i64, i64)]) -> Vec<Plane> {
    (0..corners.len())
        .map(|k| left_of(corners[k], corners[(k + 1) % corners.len()]))
        .collect()
}

/// Twice a piece's area as an exact fraction, from its corners, each the meeting of two bounds in
/// turn; a piece turning the wrong way fails here.
fn doubled_area(piece: &Piece) -> (BigInt, BigInt) {
    let k = piece.bounds.len();
    let corners: Vec<(BigInt, BigInt, BigInt)> = (0..k)
        .map(|i| {
            let t = Tpi::new(piece.face, piece.bounds[i], piece.bounds[(i + 1) % k]).unwrap();
            let (x, w) = t.homogeneous().unwrap();
            let big = |v: i128| BigInt::from(v);
            (
                big(x[0].to_i128().unwrap()),
                big(x[1].to_i128().unwrap()),
                big(w.to_i128().unwrap()),
            )
        })
        .collect();
    let mut sum = (BigInt::from(0), BigInt::from(1));
    for i in 0..k {
        let (a, b) = (&corners[i], &corners[(i + 1) % k]);
        let num = &a.0 * &b.1 - &a.1 * &b.0;
        let den = &a.2 * &b.2;
        sum = (&sum.0 * &den + &num * &sum.1, &sum.1 * &den);
    }
    assert!(sum.0.sign() == sum.1.sign(), "a piece turns clockwise");
    sum
}

fn total(pieces: &[Piece]) -> (BigInt, BigInt) {
    pieces
        .iter()
        .map(doubled_area)
        .fold((BigInt::from(0), BigInt::from(1)), |s, a| {
            (&s.0 * &a.1 + &a.0 * &s.1, &s.1 * &a.1)
        })
}

fn shoelace(corners: &[(i64, i64)]) -> i128 {
    let k = corners.len();
    (0..k)
        .map(|i| {
            let (a, b) = (corners[i], corners[(i + 1) % k]);
            i128::from(a.0) * i128::from(b.1) - i128::from(a.1) * i128::from(b.0)
        })
        .sum()
}

/// Whether a fraction is `want`.
fn is(fraction: &(BigInt, BigInt), want: i128) -> bool {
    fraction.0 == &fraction.1 * BigInt::from(want)
}

/// Every bound of every piece is a side given.
fn only_its_own_planes(pieces: &[Piece], rings: &[Vec<Plane>]) {
    for piece in pieces {
        for bound in &piece.bounds {
            let [x, y, z] = bound.n();
            let turned = Plane::new([-x, -y, -z], -bound.d()).unwrap();
            assert!(
                rings.iter().flatten().any(|s| *s == *bound || *s == turned),
                "{bound:?}"
            );
        }
    }
}

fn split(rings: &[Vec<Plane>]) -> Vec<Piece> {
    let pieces = convex_pieces(&floor(), rings, &mut meter()).unwrap();
    only_its_own_planes(&pieces, rings);
    pieces
}

const SQUARE: [(i64, i64); 4] = [(0, 0), (4, 0), (4, 4), (0, 4)];

#[test]
fn a_convex_face_is_one_piece() {
    let rings = [ring(&SQUARE)];
    let pieces = split(&rings);
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].bounds.len(), 4);
    assert_eq!(pieces[0].face, floor());
    assert!(is(&total(&pieces), shoelace(&SQUARE)));
}

#[test]
fn a_notch_splits_the_face_along_its_own_edges() {
    let l = [(0, 0), (6, 0), (6, 2), (2, 2), (2, 6), (0, 6)];
    let rings = [ring(&l)];
    let pieces = split(&rings);
    assert!(pieces.len() >= 2, "{}", pieces.len());
    assert!(is(&total(&pieces), shoelace(&l)));
}

#[test]
fn a_hole_stays_out_of_every_piece() {
    let inner = [(1, 1), (1, 3), (3, 3), (3, 1)];
    let rings = [ring(&SQUARE), ring(&inner)];
    let pieces = split(&rings);
    assert!(pieces.len() >= 4, "{}", pieces.len());
    assert!(is(&total(&pieces), shoelace(&SQUARE) + shoelace(&inner)));
}

#[test]
fn a_comb_of_many_notches_adds_up() {
    let mut comb = vec![(0, 0), (40, 0), (40, 2)];
    let mut x = 40;
    while x > 0 {
        comb.extend([(x - 2, 2), (x - 2, 8), (x - 4, 8), (x - 4, 2)]);
        x -= 4;
    }
    comb.pop();
    let rings = [ring(&comb)];
    assert!(is(&total(&split(&rings)), shoelace(&comb)));
}

#[test]
fn a_cut_ends_each_part_of_an_edge_it_crosses_where_it_crosses() {
    // In each star a long edge crosses an early cut, and a later cut passes through the part of
    // that edge's line the other side of the first: a part still holding the end it had would
    // follow that later cut into a cell it never reaches.
    let stars: [&[(i64, i64)]; 4] = [
        &[
            (0, 4),
            (-17, -12),
            (-9, -10),
            (-1, -7),
            (0, -7),
            (1, -3),
            (4, -8),
        ],
        &[
            (19, 17),
            (4, 9),
            (2, 9),
            (-1, 7),
            (-2, 4),
            (-10, 2),
            (0, -4),
        ],
        &[
            (0, 4),
            (-16, -12),
            (-9, -10),
            (-1, -7),
            (0, -7),
            (1, -3),
            (3, -8),
        ],
        &[(18, 9), (4, 4), (3, 13), (-9, 0), (6, -1)],
    ];
    for corners in stars {
        assert!(is(&total(&split(&[ring(corners)])), shoelace(corners)));
    }
}

#[test]
fn the_same_sides_give_the_same_pieces() {
    let l = [(0, 0), (6, 0), (6, 2), (2, 2), (2, 6), (0, 6)];
    let rings = [ring(&l)];
    assert_eq!(split(&rings), split(&rings));
}

#[test]
fn another_order_of_the_sides_gives_another_cut_of_the_same_face() {
    let l = [(0, 0), (6, 0), (6, 2), (2, 2), (2, 6), (0, 6)];
    let (a, b) = (ring(&l), {
        let mut turned = l.to_vec();
        turned.rotate_left(2);
        ring(&turned)
    });
    let want = shoelace(&l);
    assert!(is(&total(&split(&[a])), want));
    assert!(is(&total(&split(&[b])), want));
}

#[test]
fn no_rings_make_no_pieces() {
    assert_eq!(split(&[]), []);
}

#[test]
fn a_side_parallel_to_the_face_is_refused() {
    let mut sides = ring(&SQUARE);
    sides[1] = Plane::through(p(0, 0, 3), p(1, 0, 3), p(0, 1, 3)).unwrap();
    assert_eq!(
        convex_pieces(&floor(), &[sides], &mut meter()),
        Err(ConvexError::Num(NumError::Degenerate))
    );
}

#[test]
fn adjacent_sides_on_one_line_are_refused() {
    let doubled = [(0, 0), (2, 0), (4, 0), (4, 4), (0, 4)];
    assert_eq!(
        convex_pieces(&floor(), &[ring(&doubled)], &mut meter()),
        Err(ConvexError::Num(NumError::Degenerate))
    );
}

#[test]
fn rings_that_leave_a_piece_without_an_end_are_refused() {
    // A hole alone: inside is the whole plane outside the square.
    let hole = [(1, 1), (1, 3), (3, 3), (3, 1)];
    // An L turned inside out, from its notch: the first whole cell is a wedge.
    let open = [(2, 6), (2, 2), (6, 2), (6, 0), (0, 0), (0, 6)];
    for corners in [&hole[..], &open[..]] {
        assert_eq!(
            convex_pieces(&floor(), &[ring(corners)], &mut meter()),
            Err(ConvexError::Num(NumError::Degenerate))
        );
    }
}

#[test]
fn a_small_budget_stops_the_split() {
    let mut small = WorkMeter::new(Budget::new(1, u64::MAX));
    let got = convex_pieces(&floor(), &[ring(&SQUARE)], &mut small);
    assert!(matches!(got, Err(ConvexError::Stopped(_))), "{got:?}");
}

#[test]
fn a_convex_error_says_what_was_wrong() {
    assert!(
        ConvexError::Num(NumError::Degenerate)
            .to_string()
            .starts_with("a face that cannot be split")
    );
    let mut small = WorkMeter::new(Budget::new(0, u64::MAX));
    let stopped = convex_pieces(&floor(), &[ring(&SQUARE)], &mut small).unwrap_err();
    assert_eq!(stopped.to_string(), "the meter stopped the split");
}

/// A star-shaped polygon about the origin: its corners by angle, on small grid steps.
fn star() -> impl Strategy<Value = Vec<(i64, i64)>> {
    prop::collection::btree_set((-20i64..=20, -20i64..=20), 3..14).prop_filter_map(
        "no star here",
        |points| {
            let mut corners: Vec<(i64, i64)> = points
                .into_iter()
                .filter(|(x, y)| x.abs() + y.abs() >= 4)
                .collect();
            corners.sort_by(|a, b| {
                let half = |p: &(i64, i64)| u8::from(p.1 < 0 || (p.1 == 0 && p.0 < 0));
                half(a)
                    .cmp(&half(b))
                    .then((b.0 * a.1 - b.1 * a.0).cmp(&0))
                    .then((a.0 * a.0 + a.1 * a.1).cmp(&(b.0 * b.0 + b.1 * b.1)))
            });
            let k = corners.len();
            if k < 3 {
                return None;
            }
            // Strictly around the origin, with no three corners in a row on one line.
            let cross = |o: (i64, i64), a: (i64, i64), b: (i64, i64)| {
                (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
            };
            let good = (0..k).all(|i| {
                let (a, b, c) = (corners[i], corners[(i + 1) % k], corners[(i + 2) % k]);
                a.0 * b.1 - a.1 * b.0 > 0 && cross(a, b, c) != 0
            });
            good.then_some(corners)
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn a_star_adds_up_whatever_its_shape(corners in star()) {
        let rings = [ring(&corners)];
        let pieces = split(&rings);
        prop_assert!(is(&total(&pieces), shoelace(&corners)));
    }
}
