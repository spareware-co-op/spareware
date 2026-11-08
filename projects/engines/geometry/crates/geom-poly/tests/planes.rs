//! Canonical planes and the arrangement predicates, against big integers and on the extremes of
//! the grid (G123, 6.2, 11.3).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and short names as in the formulas"
)]

use core::cmp::Ordering;

use geom_num::grid::COORD_MAX;
use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, P3, Sign};
use geom_poly::order::{
    Flap, Line, Ray, about, along, around, normals_det, same_point, turn, turn_about,
};
use geom_poly::plane::{PlaneTable, TableError, canonical};
use num_bigint::BigInt;
use proptest::prelude::*;

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn plane(a: P3, b: P3, c: P3) -> Plane {
    Plane::through(a, b, c).unwrap()
}

/// x = k, facing +x.
fn at_x(k: i64) -> Plane {
    plane(p(k, 0, 0), p(k, 1, 0), p(k, 0, 1))
}

/// y = k, facing +y.
fn at_y(k: i64) -> Plane {
    plane(p(0, k, 0), p(0, k, 1), p(1, k, 0))
}

/// z = k, facing +z.
fn at_z(k: i64) -> Plane {
    plane(p(0, 0, k), p(1, 0, k), p(0, 1, k))
}

fn flipped(q: &Plane) -> Plane {
    let [x, y, z] = q.n();
    Plane::new([-x, -y, -z], -q.d()).unwrap()
}

fn big(v: i128) -> BigInt {
    BigInt::from(v)
}

/// det of three vectors of big integers.
fn det(a: &[BigInt; 3], b: &[BigInt; 3], c: &[BigInt; 3]) -> BigInt {
    &a[0] * (&b[1] * &c[2] - &b[2] * &c[1]) - &a[1] * (&b[0] * &c[2] - &b[2] * &c[0])
        + &a[2] * (&b[0] * &c[1] - &b[1] * &c[0])
}

fn normal(q: &Plane) -> [BigInt; 3] {
    q.n().map(big)
}

fn cross(a: &[BigInt; 3], b: &[BigInt; 3]) -> [BigInt; 3] {
    [
        &a[1] * &b[2] - &a[2] * &b[1],
        &a[2] * &b[0] - &a[0] * &b[2],
        &a[0] * &b[1] - &a[1] * &b[0],
    ]
}

fn dot(a: &[BigInt; 3], b: &[BigInt; 3]) -> BigInt {
    &a[0] * &b[0] + &a[1] * &b[1] + &a[2] * &b[2]
}

fn sign(v: &BigInt) -> Sign {
    match v.sign() {
        num_bigint::Sign::Minus => Sign::Negative,
        num_bigint::Sign::NoSign => Sign::Zero,
        num_bigint::Sign::Plus => Sign::Positive,
    }
}

/// The point where three planes meet, as numerators over a weight, by Cramer's rule: independent
/// of `Tpi`.
fn meeting(a: &Plane, b: &Plane, c: &Plane) -> ([BigInt; 3], BigInt) {
    let (na, nb, nc) = (normal(a), normal(b), normal(c));
    let w = det(&na, &nb, &nc);
    let (da, db, dc) = (-big(a.d()), -big(b.d()), -big(c.d()));
    let (bc, ca, ab) = (cross(&nb, &nc), cross(&nc, &na), cross(&na, &nb));
    let x = [
        &da * &bc[0] + &db * &ca[0] + &dc * &ab[0],
        &da * &bc[1] + &db * &ca[1] + &dc * &ab[1],
        &da * &bc[2] + &db * &ca[2] + &dc * &ab[2],
    ];
    (x, w)
}

/// The order along `n_p` × `n_q` of two points, by comparing their parameters as fractions.
fn oracle_along(line: (&Plane, &Plane), c: &Plane, d: &Plane) -> Ordering {
    let v = cross(&normal(line.0), &normal(line.1));
    let (xc, wc) = meeting(line.0, line.1, c);
    let (xd, wd) = meeting(line.0, line.1, d);
    // x_d / w_d − x_c / w_c, dotted with v, over w_c w_d.
    let diff = [
        &xd[0] * &wc - &xc[0] * &wd,
        &xd[1] * &wc - &xc[1] * &wd,
        &xd[2] * &wc - &xc[2] * &wd,
    ];
    let t = dot(&diff, &v) * &wc * &wd;
    match sign(&t) {
        Sign::Positive => Ordering::Less,
        Sign::Negative => Ordering::Greater,
        Sign::Zero => Ordering::Equal,
    }
}

const M: i64 = COORD_MAX;

/// A grid point on a coarse sub-grid or anywhere in the cube.
fn coord() -> impl Strategy<Value = i64> {
    prop_oneof![
        (-16i64..=16).prop_map(|k| k * 6_400),
        (-M..=M),
        Just(M),
        Just(-M),
    ]
}

fn point() -> impl Strategy<Value = P3> {
    (coord(), coord(), coord()).prop_map(|(x, y, z)| p(x, y, z))
}

fn any_plane() -> impl Strategy<Value = Plane> {
    (point(), point(), point())
        .prop_filter_map("collinear", |(a, b, c)| Plane::through(a, b, c).ok())
}

#[test]
fn canonical_planes_are_one_plane_whatever_made_them() {
    let a = plane(p(0, 0, 0), p(2, 0, 0), p(0, 2, 0));
    let b = plane(p(5, 1, 0), p(7, 1, 0), p(5, 4, 0));
    assert_eq!(a.n(), [0, 0, 4]);
    assert_eq!(b.n(), [0, 0, 6]);
    let (ca, cb) = (canonical(&a).unwrap(), canonical(&b).unwrap());
    assert_eq!(ca.plane, cb.plane);
    assert_eq!(ca.plane.n(), [0, 0, 1]);
    assert_eq!(ca.plane.d(), 0);
    assert!(!ca.flipped);
    let down = canonical(&flipped(&a)).unwrap();
    assert_eq!(down.plane, ca.plane);
    assert!(down.flipped);
}

#[test]
fn the_first_non_zero_component_is_positive() {
    for (n, want) in [
        ([0, 0, -3], [0, 0, 1]),
        ([0, -2, 3], [0, 2, -3]),
        ([-4, 2, 6], [2, -1, -3]),
        ([7, -14, 0], [1, -2, 0]),
    ] {
        let got = canonical(&Plane::new(n, 0).unwrap()).unwrap();
        assert_eq!(got.plane.n(), want);
        assert_eq!(got.flipped, n.iter().find(|v| **v != 0).unwrap() < &0);
    }
}

#[test]
fn the_offset_shares_the_divisor() {
    let got = canonical(&Plane::new([4, 0, 0], 6).unwrap()).unwrap();
    assert_eq!((got.plane.n(), got.plane.d()), ([2, 0, 0], 3));
    let one = canonical(&Plane::new([3, 0, 0], 1).unwrap()).unwrap();
    assert_eq!((one.plane.n(), one.plane.d()), ([3, 0, 0], 1));
}

#[test]
fn a_table_holds_each_plane_once_at_its_narrowest() {
    let mut table = PlaneTable::new();
    assert!(table.is_empty());
    let a = table.insert(&at_z(0)).unwrap();
    let b = table
        .insert(&plane(p(5, 1, 0), p(7, 1, 0), p(5, 4, 0)))
        .unwrap();
    let c = table.insert(&flipped(&at_z(0))).unwrap();
    assert_eq!(a.plane, b.plane);
    assert_eq!(a.plane, c.plane);
    assert!(!a.flipped && !b.flipped && c.flipped);
    assert_eq!(table.len(), 1);
    assert_eq!(table.plane(a.plane), Some(at_z(0)));

    // Past i64: a plane through far corners with no common factor.
    let far = plane(p(M, 1, 2), p(3, M, 5), p(7, 11, M));
    let w = table.insert(&far).unwrap();
    assert_eq!(table.len(), 2);
    assert_eq!(table.widths(), (1, 1));
    assert_eq!(table.plane(w.plane), Some(canonical(&far).unwrap().plane));
    assert_eq!(w.plane.row(), 1);
    let again = table.insert(&at_x(4)).unwrap();
    let third = table.insert(&at_y(4)).unwrap();
    assert_eq!((again.plane.row(), third.plane.row()), (2, 3));
    assert_eq!(table.widths(), (3, 1));
    assert_eq!(
        table.plane(geom_poly::plane::PlaneIx::default()),
        Some(at_z(0))
    );
}

#[test]
fn a_table_says_when_it_holds_something() {
    let mut table = PlaneTable::new();
    assert!(table.is_empty());
    table.insert(&Plane::new([1, 0, 0], 0).unwrap()).unwrap();
    assert!(!table.is_empty());
}

#[test]
fn a_table_refuses_room_no_allocator_can_give() {
    let mut table = PlaneTable::new();
    assert_eq!(table.reserve(usize::MAX), Err(TableError::OutOfMemory));
    assert_eq!(table.reserve(4), Ok(()));
    assert!(table.is_empty());
}

#[test]
fn a_row_past_the_table_is_none() {
    let mut table = PlaneTable::new();
    let a = table.insert(&at_z(0)).unwrap();
    let mut other = PlaneTable::new();
    assert_eq!(other.plane(a.plane), None);
    other.insert(&at_x(1)).unwrap();
    assert_eq!(
        other.plane(a.plane),
        Some(canonical(&at_x(1)).unwrap().plane)
    );
}

#[test]
fn a_table_error_says_what_was_wrong() {
    assert_eq!(
        TableError::Full.to_string(),
        "a plane table holds 2^32 planes"
    );
    assert_eq!(
        TableError::OutOfMemory.to_string(),
        "the allocator refused a plane table's room"
    );
    assert!(
        TableError::from(NumError::Degenerate)
            .to_string()
            .starts_with("a plane the numbers layer refuses")
    );
}

#[test]
fn points_fall_in_order_along_an_axis() {
    let line = Line::new(at_y(0), at_z(0)).unwrap();
    // n_y × n_z points along +x.
    assert_eq!(along(&line, &at_x(1), &at_x(2)), Ok(Ordering::Less));
    assert_eq!(along(&line, &at_x(2), &at_x(1)), Ok(Ordering::Greater));
    assert_eq!(along(&line, &at_x(2), &at_x(2)), Ok(Ordering::Equal));
    // A plane's facing does not move the point it makes.
    assert_eq!(
        along(&line, &flipped(&at_x(1)), &at_x(2)),
        Ok(Ordering::Less)
    );
    assert_eq!(
        along(&line, &at_x(1), &flipped(&at_x(2))),
        Ok(Ordering::Less)
    );
    let reversed = Line::new(at_z(0), at_y(0)).unwrap();
    assert_eq!(along(&reversed, &at_x(1), &at_x(2)), Ok(Ordering::Greater));
}

#[test]
fn a_plane_parallel_to_the_line_is_refused() {
    let line = Line::new(at_y(0), at_z(0)).unwrap();
    assert_eq!(along(&line, &at_y(3), &at_x(1)), Err(NumError::Degenerate));
    assert_eq!(along(&line, &at_x(1), &at_z(3)), Err(NumError::Degenerate));
    assert_eq!(Line::new(at_x(0), at_x(4)), Err(NumError::Degenerate));
    assert_eq!(
        Line::new(at_x(0), flipped(&at_x(0))),
        Err(NumError::Degenerate)
    );
}

#[test]
fn one_point_made_two_ways_is_one_point() {
    let a = Tpi::new(at_x(0), at_y(0), at_z(0)).unwrap();
    let slant = plane(p(0, 0, 0), p(1, 1, 0), p(0, 0, 1));
    let b = Tpi::new(slant, at_y(0), at_z(0)).unwrap();
    assert_eq!(same_point(&a, &b), Ok(true));
    assert_eq!(same_point(&b, &a), Ok(true));
    let c = Tpi::new(at_x(1), at_y(0), at_z(0)).unwrap();
    assert_eq!(same_point(&a, &c), Ok(false));
    let d = Tpi::new(at_x(0), at_y(1), at_z(0)).unwrap();
    assert_eq!(same_point(&a, &d), Ok(false));
    let e = Tpi::new(at_x(0), at_y(0), at_z(1)).unwrap();
    assert_eq!(same_point(&a, &e), Ok(false));
}

fn ray(with: Plane) -> Ray {
    Ray {
        with,
        reversed: false,
    }
}

#[test]
fn rays_turn_about_their_face() {
    // On z = 0, n_z × n_x points along +y and n_z × n_y along −x.
    let face = at_z(0);
    let (y, minus_x) = (ray(at_x(0)), ray(at_y(0)));
    assert_eq!(turn(&face, &y, &minus_x), Ok(Sign::Positive));
    assert_eq!(turn(&face, &minus_x, &y), Ok(Sign::Negative));
    assert_eq!(turn(&face, &y, &y), Ok(Sign::Zero));
    let back = Ray {
        with: at_x(0),
        reversed: true,
    };
    assert_eq!(turn(&face, &back, &minus_x), Ok(Sign::Negative));
    assert_eq!(turn(&flipped(&face), &y, &minus_x), Ok(Sign::Negative));
}

/// Four rays on z = 0 at right angles, counter-clockwise from +y.
fn compass() -> [Ray; 4] {
    let y = ray(at_x(0));
    let minus_x = ray(at_y(0));
    let minus_y = Ray {
        reversed: true,
        ..y
    };
    let x = Ray {
        reversed: true,
        ..minus_x
    };
    [y, minus_x, minus_y, x]
}

#[test]
fn around_a_vertex_every_ray_has_its_place() {
    let face = at_z(0);
    for start in 0..4 {
        let turned = |k: usize| compass()[(start + k) % 4];
        let first = turned(0);
        for (i, j) in [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)] {
            assert_eq!(
                around(&face, &first, &turned(i), &turned(j)),
                Ok(Ordering::Less),
                "{start}: {i} {j}"
            );
            assert_eq!(
                around(&face, &first, &turned(j), &turned(i)),
                Ok(Ordering::Greater)
            );
        }
        for k in 0..4 {
            assert_eq!(
                around(&face, &first, &turned(k), &turned(k)),
                Ok(Ordering::Equal)
            );
        }
    }
}

#[test]
fn two_rays_at_one_side_are_ordered_by_their_turn() {
    // From +y: towards −x comes before −x itself when both lie left of +y.
    let face = at_z(0);
    let first = ray(at_x(0));
    // The plane x + y = 0 faces (−1, −1, 0), so its ray runs along (1, −1, 0) and back.
    let slant = plane(p(0, 0, 0), p(1, -1, 0), p(0, 0, 1));
    let up_left = Ray {
        with: slant,
        reversed: true,
    };
    let left = ray(at_y(0));
    assert_eq!(turn(&face, &first, &up_left), Ok(Sign::Positive));
    assert_eq!(around(&face, &first, &up_left, &left), Ok(Ordering::Less));
    assert_eq!(
        around(&face, &first, &left, &up_left),
        Ok(Ordering::Greater)
    );
    // A second ray on the right side of `first`.
    let rights = compass()[3];
    let down_right = Ray {
        with: slant,
        reversed: false,
    };
    assert_eq!(
        around(&face, &first, &rights, &down_right),
        Ok(Ordering::Greater)
    );
    assert_eq!(
        around(&face, &first, &down_right, &rights),
        Ok(Ordering::Less)
    );
}

fn flap(q: Plane) -> Flap {
    Flap {
        plane: q,
        reversed: false,
    }
}

#[test]
fn flaps_turn_about_their_line() {
    // The line is the x axis; n_y × n_z runs along +x.
    let line = Line::new(at_y(0), at_z(0)).unwrap();
    let (a, b) = (flap(at_y(0)), flap(at_z(0)));
    assert_eq!(turn_about(&line, &a, &b), Ok(Sign::Positive));
    assert_eq!(turn_about(&line, &b, &a), Ok(Sign::Negative));
    assert_eq!(turn_about(&line, &a, &a), Ok(Sign::Zero));
    let back = Flap {
        reversed: true,
        ..a
    };
    assert_eq!(turn_about(&line, &back, &b), Ok(Sign::Negative));
}

#[test]
fn around_an_edge_every_flap_has_its_place() {
    let line = Line::new(at_y(0), at_z(0)).unwrap();
    let flaps = |k: usize| {
        let (y, z) = (flap(at_y(0)), flap(at_z(0)));
        let ring = [
            y,
            z,
            Flap {
                reversed: true,
                ..y
            },
            Flap {
                reversed: true,
                ..z
            },
        ];
        ring[k % 4]
    };
    for start in 0..4 {
        let first = flaps(start);
        for (i, j) in [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)] {
            assert_eq!(
                about(&line, &first, &flaps(start + i), &flaps(start + j)),
                Ok(Ordering::Less),
                "{start}: {i} {j}"
            );
        }
    }
    // Flaps on opposite planes of one face: flipped normals, reversed, are the same flap.
    let first = flap(at_y(0));
    let again = Flap {
        plane: flipped(&at_y(0)),
        reversed: true,
    };
    assert_eq!(about(&line, &first, &first, &again), Ok(Ordering::Equal));
    let opposite = flap(flipped(&at_y(0)));
    assert_eq!(
        about(&line, &first, &opposite, &flap(at_z(0))),
        Ok(Ordering::Greater)
    );
    // Two on one side: ordered by their turn.
    let slant = plane(p(0, 0, 0), p(1, 0, 0), p(0, 1, -1));
    assert_eq!(
        about(&line, &first, &flap(slant), &flap(at_z(0))),
        Ok(Ordering::Less)
    );
    assert_eq!(
        about(&line, &first, &flap(at_z(0)), &flap(slant)),
        Ok(Ordering::Greater)
    );
}

#[test]
fn the_extremes_of_the_grid_stay_within_budget() {
    // Planes through the corners of the frame's cube reach the budget's values (6.2).
    let corner = |k: usize| {
        let at = |bit: usize| if k >> bit & 1 == 1 { M } else { -M };
        p(at(0), at(1), at(2))
    };
    let mut planes = Vec::new();
    for a in 0..8 {
        for b in a + 1..8 {
            for c in b + 1..8 {
                if let Ok(q) = Plane::through(corner(a), corner(b), corner(c)) {
                    planes.push(q);
                }
            }
        }
    }
    assert_eq!(planes.len(), 56);
    let mut compared = 0;
    for (i, a) in planes.iter().enumerate().step_by(5) {
        for b in planes.iter().skip(i + 1).step_by(7) {
            let Ok(line) = Line::new(*a, *b) else {
                continue;
            };
            for c in planes.iter().step_by(11) {
                for d in planes.iter().skip(3).step_by(13) {
                    if normals_det(a, b, c).unwrap() == Sign::Zero
                        || normals_det(a, b, d).unwrap() == Sign::Zero
                    {
                        continue;
                    }
                    assert_eq!(along(&line, c, d), Ok(oracle_along((a, b), c, d)));
                    compared += 1;
                }
            }
        }
    }
    assert!(compared > 100, "{compared}");
}

/// `s` turned over.
fn opposite(s: Sign) -> Sign {
    match s {
        Sign::Negative => Sign::Positive,
        Sign::Zero => Sign::Zero,
        Sign::Positive => Sign::Negative,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn canonical_keeps_the_plane(q in any_plane(), a in point(), b in point()) {
        let c = canonical(&q).unwrap();
        let again = canonical(&c.plane).unwrap();
        prop_assert_eq!(again.plane, c.plane);
        prop_assert!(!again.flipped);
        for r in [a, b] {
            let side = q.side(r).unwrap();
            let want = if c.flipped { opposite(side) } else { side };
            prop_assert_eq!(c.plane.side(r).unwrap(), want);
        }
    }

    #[test]
    fn along_agrees_with_big_integers(
        a in any_plane(), b in any_plane(), c in any_plane(), d in any_plane()
    ) {
        prop_assume!(Line::new(a, b).is_ok());
        prop_assume!(normals_det(&a, &b, &c).unwrap() != Sign::Zero);
        prop_assume!(normals_det(&a, &b, &d).unwrap() != Sign::Zero);
        let line = Line::new(a, b).unwrap();
        prop_assert_eq!(along(&line, &c, &d).unwrap(), oracle_along((&a, &b), &c, &d));
        prop_assert_eq!(
            along(&line, &c, &d).unwrap(),
            along(&line, &d, &c).unwrap().reverse()
        );
    }

    #[test]
    fn determinants_agree_with_big_integers(
        a in any_plane(), b in any_plane(), c in any_plane()
    ) {
        let want = sign(&det(&normal(&a), &normal(&b), &normal(&c)));
        prop_assert_eq!(normals_det(&a, &b, &c).unwrap(), want);
    }

    #[test]
    fn turns_agree_with_big_integers(
        f in any_plane(), g in any_plane(), h in any_plane(), x: bool, y: bool
    ) {
        let (nf, ng, nh) = (normal(&f), normal(&g), normal(&h));
        let scale = |v: [BigInt; 3], reversed: bool| {
            if reversed { v.map(|c| -c) } else { v }
        };
        let (dg, dh) = (scale(cross(&nf, &ng), x), scale(cross(&nf, &nh), y));
        let want = sign(&dot(&cross(&dg, &dh), &nf));
        let got = turn(
            &f,
            &Ray { with: g, reversed: x },
            &Ray { with: h, reversed: y },
        );
        prop_assert_eq!(got.unwrap(), want);
    }

    #[test]
    fn a_table_gives_one_entry_to_one_plane(q in any_plane(), r in any_plane()) {
        let mut table = PlaneTable::new();
        let (a, b) = (table.insert(&q).unwrap(), table.insert(&flipped(&q)).unwrap());
        prop_assert_eq!(a.plane, b.plane);
        prop_assert_ne!(a.flipped, b.flipped);
        let c = table.insert(&r).unwrap();
        let same = canonical(&q).unwrap().plane == canonical(&r).unwrap().plane;
        prop_assert_eq!(a.plane == c.plane, same);
        prop_assert_eq!(table.plane(c.plane), Some(canonical(&r).unwrap().plane));
    }
}
