//! Every step of the predicate ladder gives the exact path's sign (chapter 23.2): each is held to a
//! big-integer oracle, and at the edge of the range it takes (G124).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and short names, and a failure stops the test"
)]

use geom_num::plane::{Plane, Tpi};
use geom_num::predicate::{
    incircle, incircle_step, orient2d, orient2d_narrow, orient2d_step, orient3d, orient3d_step,
};
use geom_num::{P2, P3, Sign, Step};
use num_bigint::BigInt;
use num_traits::Signed;
use proptest::prelude::*;

fn p2(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn p3(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn sign_of(v: &BigInt) -> Sign {
    if v.is_positive() {
        Sign::Positive
    } else if v.is_negative() {
        Sign::Negative
    } else {
        Sign::Zero
    }
}

fn big(v: i64) -> BigInt {
    BigInt::from(v)
}

fn turn(a: (i64, i64), b: (i64, i64), c: (i64, i64)) -> Sign {
    let (p, q) = (big(b.0 - a.0), big(c.1 - a.1));
    let (r, s) = (big(b.1 - a.1), big(c.0 - a.0));
    sign_of(&(p * q - r * s))
}

fn det3(p: [BigInt; 3], q: [BigInt; 3], r: [BigInt; 3]) -> BigInt {
    let [a, b, c] = p;
    let [d, e, f] = q;
    let [g, h, i] = r;
    a * (&e * &i - &f * &h) - b * (&d * &i - &f * &g) + c * (&d * &h - &e * &g)
}

fn volume(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3]) -> Sign {
    let from_a = |p: [i64; 3]| [big(p[0] - a[0]), big(p[1] - a[1]), big(p[2] - a[2])];
    sign_of(&det3(from_a(b), from_a(c), from_a(d)))
}

fn in_circle(a: (i64, i64), b: (i64, i64), c: (i64, i64), d: (i64, i64)) -> Sign {
    let row = |p: (i64, i64)| {
        let (x, y) = (big(p.0 - d.0), big(p.1 - d.1));
        [x.clone(), y.clone(), &x * &x + &y * &y]
    };
    sign_of(&det3(row(a), row(b), row(c)))
}

/// n · X + d · w times the sign of w, over the integers.
fn tpi_oracle(t: &Tpi, s: &Plane) -> Sign {
    let coefficients = |q: &Plane| (q.n().map(BigInt::from), BigInt::from(q.d()));
    let [p, q, r] = t.planes();
    let ((np, dp), (nq, dq), (nr, dr)) = (coefficients(p), coefficients(q), coefficients(r));
    let (ns, ds) = coefficients(s);
    let cross = |a: &[BigInt; 3], b: &[BigInt; 3]| {
        [
            &a[1] * &b[2] - &a[2] * &b[1],
            &a[2] * &b[0] - &a[0] * &b[2],
            &a[0] * &b[1] - &a[1] * &b[0],
        ]
    };
    let (qr, rp, pq) = (cross(&nq, &nr), cross(&nr, &np), cross(&np, &nq));
    let w = &np[0] * &qr[0] + &np[1] * &qr[1] + &np[2] * &qr[2];
    let x: Vec<BigInt> = (0..3)
        .map(|i| -(&dp * &qr[i] + &dq * &rp[i] + &dr * &pq[i]))
        .collect();
    let side = &ns[0] * &x[0] + &ns[1] * &x[1] + &ns[2] * &x[2] + ds * &w;
    sign_of(&side).mul(sign_of(&w))
}

const LIMIT_2D: i64 = 1 << 31;

#[test]
fn orient2d_stays_narrow_up_to_its_limit() {
    let top = LIMIT_2D - 1;
    let (a, b, c) = (p2(0, 0), p2(top, 0), p2(0, top));
    assert_eq!(orient2d_narrow(a, b, c), Some(Sign::Positive));
    assert_eq!(orient2d_narrow(a, c, b), Some(Sign::Negative));
    assert_eq!(orient2d_narrow(a, b, b), Some(Sign::Zero));
    // From opposite corners of the range, the determinant's widest reach.
    let (d, e, f) = (p2(top, top), p2(0, top), p2(top, 0));
    assert_eq!(orient2d_narrow(d, e, f), Some(Sign::Positive));
    for (x, y) in [(LIMIT_2D, 0), (-LIMIT_2D, 0), (0, LIMIT_2D), (0, -LIMIT_2D)] {
        let (far, turned) = (p2(x, y), p2(y, -x));
        assert_eq!(orient2d_narrow(a, far, turned), None);
        assert_eq!(orient2d_narrow(a, turned, far), None);
        assert_eq!(orient2d(a, far, turned), Ok(turn((0, 0), (x, y), (y, -x))));
    }
}

#[test]
fn orient2d_takes_the_narrow_step_only_where_it_filters() {
    // A tie with no zero among its differences, which a filter leaves.
    let (a, b, c) = (p2(0, 0), p2(9, 3), p2(3, 1));
    let step = if cfg!(all(
        target_pointer_width = "64",
        not(target_family = "wasm")
    )) {
        Step::Exact
    } else {
        Step::Narrow
    };
    assert_eq!(orient2d_step(a, b, c), Ok((Sign::Zero, step)));
}

#[test]
fn a_filter_settles_general_points_and_the_exact_path_their_ties() {
    // Far apart, and clear of each other: past the narrow step, and easy for the filter.
    let m = 1 << 38;
    let (a, b, c, d) = (p3(-m, -m, -m), p3(m, -m, -m), p3(-m, m, -m), p3(-m, -m, m));
    assert_eq!(
        orient3d_step(a, b, c, d),
        Ok((Sign::Positive, Step::Filter))
    );
    let (_, step) = incircle_step(p2(m, 0), p2(0, m), p2(-m, 0), p2(1, 1)).unwrap();
    assert_eq!(step, Step::Filter);
    // Exact ties with no zero among their differences, which only the exact path can call.
    let (u, v) = (p3(m, m / 2, m / 4), p3(m / 2, m, m / 8));
    let across = p3(m + m / 2, m + m / 2, m / 4 + m / 8);
    assert_eq!(
        orient3d_step(p3(0, 0, 0), u, v, across),
        Ok((Sign::Zero, Step::Exact))
    );
    assert_eq!(
        orient2d_step(p2(-m, -m), p2(m, m), p2(m / 2, m / 2)),
        Ok((Sign::Zero, Step::Exact))
    );
    let k = 1 << 36;
    let (q, r, s) = (p2(3 * k, 4 * k), p2(4 * k, 3 * k), p2(-3 * k, 4 * k));
    assert_eq!(
        incircle_step(q, r, s, p2(-4 * k, -3 * k)),
        Ok((Sign::Zero, Step::Exact))
    );
}

fn tilted() -> [Plane; 3] {
    [
        Plane::new([3, -4, 5], 2).unwrap(),
        Plane::new([1, 7, -2], -9).unwrap(),
        Plane::new([-6, 1, 1], 4).unwrap(),
    ]
}

#[test]
fn a_vertex_lies_on_its_own_planes_with_no_arithmetic() {
    let [p, q, r] = tilted();
    let t = Tpi::new(p, q, r).unwrap();
    for own in [p, q, r] {
        assert_eq!(own.side_of_tpi_step(&t), Ok((Sign::Zero, Step::Identity)));
        assert_eq!(own.side_of_tpi(&t), Ok(Sign::Zero));
    }
}

fn unit(axis: usize, at: i64) -> Plane {
    let mut n = [0i128; 3];
    n[axis] = 1;
    Plane::new(n, -i128::from(at)).unwrap()
}

fn vertex(at: [i64; 3]) -> Tpi {
    Tpi::new(unit(0, at[0]), unit(1, at[1]), unit(2, at[2])).unwrap()
}

#[test]
fn a_tie_the_filter_leaves_stays_narrow_up_to_its_limits() {
    let step = |s: Plane, t: &Tpi| s.side_of_tpi_step(t).unwrap();
    let t = vertex([1, 2, 3]);
    // The normal's limit.
    let edge = Plane::new([63, 1, 1], -68).unwrap();
    assert_eq!(step(edge, &t), (Sign::Zero, Step::Narrow));
    let past = Plane::new([64, 1, 1], -69).unwrap();
    assert_eq!(step(past, &t), (Sign::Zero, Step::Exact));
    // The offset's limit, on the other side of 2^40.
    let low = 1i64 << 39;
    let inside = vertex([low, low - 1, 0]);
    let s = Plane::new([1, 1, 1], -i128::from(2 * low - 1)).unwrap();
    assert_eq!(step(s, &inside), (Sign::Zero, Step::Narrow));
    let outside = vertex([low, low, 0]);
    let s = Plane::new([1, 1, 1], -i128::from(2 * low)).unwrap();
    assert_eq!(step(s, &outside), (Sign::Zero, Step::Exact));
    // One plane of the vertex out of range ends the step, wherever it stands.
    let wide = Plane::new([64, 0, 1], -67).unwrap();
    let s = Plane::new([1, 1, 1], -6).unwrap();
    for planes in [
        [wide, unit(1, 2), unit(0, 1)],
        [unit(1, 2), wide, unit(0, 1)],
        [unit(1, 2), unit(0, 1), wide],
    ] {
        let t = Tpi::new(planes[0], planes[1], planes[2]).unwrap();
        assert_eq!(step(s, &t), (Sign::Zero, Step::Exact));
    }
}

/// Small planes the filter cannot separate from a fourth, a small step of n · X + d · w from the
/// vertex: the only way to give the narrow step a sign to find.
#[test]
fn the_narrow_step_finds_the_sign_of_a_near_tie() {
    let at = 1i128 << 38;
    let p = Plane::new([61, -47, 59], at + 1).unwrap();
    let q = Plane::new([-53, 62, 37], -(at + 7)).unwrap();
    let r = Plane::new([41, 58, -63], at + 3).unwrap();
    let t = Tpi::new(p, q, r).unwrap();
    let swapped = Tpi::new(q, p, r).unwrap();
    let (x, w) = t.homogeneous().unwrap();
    let (x, w) = (x.map(|v| v.to_i128().unwrap()), w.to_i128().unwrap());
    let mut found = [false; 2];
    for a in -63i128..=63 {
        for b in -63i128..=63 {
            for c in -63i128..=63 {
                if a == 0 || b == 0 {
                    continue;
                }
                let along = a * x[0] + b * x[1] + c * x[2];
                let d = -(along + w.abs() / 2).div_euclid(w.abs()) * w.signum();
                let rest = along + d * w;
                if d.abs() >= 1 << 40 || rest.abs() >= 64 || rest == 0 {
                    continue;
                }
                let s = Plane::new([a, b, c], d).unwrap();
                for vertex in [&t, &swapped] {
                    let (sign, step) = s.side_of_tpi_step(vertex).unwrap();
                    assert_eq!(sign, tpi_oracle(vertex, &s));
                    if step == Step::Narrow {
                        found[usize::from(sign == Sign::Positive)] = true;
                    }
                }
            }
        }
    }
    assert_eq!(found, [true, true]);
}

/// Three steep planes through the grid point `at`, each from corners far from it, so the filter
/// sees huge coefficients and the point lands exactly on `at`.
fn big_vertex(at: [i64; 3]) -> Tpi {
    let m = 1 << 37;
    let through = |a: [i64; 3], b: [i64; 3]| {
        let shift = |p: [i64; 3]| p3(at[0] + p[0], at[1] + p[1], at[2] + p[2]);
        Plane::through(shift([0, 0, 0]), shift(a), shift(b)).unwrap()
    };
    Tpi::new(
        through([m, m - 3, 5], [7, m + 11, m - 2]),
        through([m - 1, 13, m], [17, m, -m + 9]),
        through([19, -m, m - 7], [m, 23, m + 1]),
    )
    .unwrap()
}

#[test]
fn a_filter_answers_a_tilted_plane_and_the_axis_step_what_it_leaves() {
    let at = [1 << 20, -(1 << 21), 3 << 19];
    let t = big_vertex(at);
    let clear = Plane::new([1, 0, 0], -(1 << 39)).unwrap();
    assert_eq!(clear.side_of_tpi_step(&t).unwrap().1, Step::Filter);
    for axis in 0..3 {
        for facing in [1i128, -1] {
            let mut n = [0i128; 3];
            n[axis] = facing;
            for off in [-1i128, 0, 1] {
                let s = Plane::new(n, -facing * (i128::from(at[axis]) + off)).unwrap();
                let (sign, step) = s.side_of_tpi_step(&t).unwrap();
                assert_eq!(step, Step::Axis, "{n:?} {off}");
                assert_eq!(sign, tpi_oracle(&t, &s), "{n:?} {off}");
                assert_eq!(s.side_of_tpi(&t), Ok(sign));
            }
        }
    }
}

#[test]
fn any_other_plane_the_filter_leaves_takes_the_exact_path() {
    let at = [1 << 20, -(1 << 21), 3 << 19];
    let t = big_vertex(at);
    // Square to an axis, but not canonical: its normal is 2.
    let s = Plane::new([2, 0, 0], -2 * i128::from(at[0])).unwrap();
    assert_eq!(s.side_of_tpi_step(&t), Ok((Sign::Zero, Step::Exact)));
    let tilted = Plane::new([1, 1, 0], -(i128::from(at[0]) + i128::from(at[1]))).unwrap();
    assert_eq!(tilted.side_of_tpi_step(&t), Ok((Sign::Zero, Step::Exact)));
}

fn coordinate() -> impl Strategy<Value = i64> {
    prop_oneof![
        -40i64..=40,
        -(1i64 << 21)..=(1 << 21),
        -(1i64 << 32)..=(1 << 32),
        -(1i64 << 39)..=(1 << 39),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn every_step_of_orient2d_gives_the_exact_sign(
        a in (coordinate(), coordinate()),
        b in (coordinate(), coordinate()),
        c in (coordinate(), coordinate()),
    ) {
        let (u, v, w) = (p2(a.0, a.1), p2(b.0, b.1), p2(c.0, c.1));
        prop_assert_eq!(orient2d(u, v, w), Ok(turn(a, b, c)));
    }

    #[test]
    fn every_step_of_orient3d_gives_the_exact_sign(
        a in [coordinate(), coordinate(), coordinate()],
        b in [coordinate(), coordinate(), coordinate()],
        c in [coordinate(), coordinate(), coordinate()],
        d in [coordinate(), coordinate(), coordinate()],
    ) {
        let at = |q: [i64; 3]| p3(q[0], q[1], q[2]);
        prop_assert_eq!(
            orient3d(at(a), at(b), at(c), at(d)),
            Ok(volume(a, b, c, d))
        );
    }

    #[test]
    fn every_step_of_incircle_gives_the_exact_sign(
        a in (coordinate(), coordinate()),
        b in (coordinate(), coordinate()),
        c in (coordinate(), coordinate()),
        d in (coordinate(), coordinate()),
    ) {
        let at = |q: (i64, i64)| p2(q.0, q.1);
        prop_assert_eq!(
            incircle(at(a), at(b), at(c), at(d)),
            Ok(in_circle(a, b, c, d))
        );
    }

    #[test]
    fn every_step_of_a_plane_gives_the_exact_sign(
        n in [-9i128..=9, -70i128..=70, -(1i128 << 22)..=(1 << 22)],
        d in prop_oneof![-50i128..=50, -(1i128 << 41)..=(1 << 41)],
        planes in prop::collection::vec(
            ([-9i128..=9, -9i128..=9, -70i128..=70], -(1i128 << 41)..=(1 << 41)),
            3,
        ),
    ) {
        let make = |(n, d): &([i128; 3], i128)| Plane::new(*n, *d);
        let (Ok(p), Ok(q), Ok(r)) = (make(&planes[0]), make(&planes[1]), make(&planes[2])) else {
            return Ok(());
        };
        let Ok(s) = Plane::new(n, d) else {
            return Ok(());
        };
        let Ok(t) = Tpi::new(p, q, r) else {
            return Ok(());
        };
        prop_assert_eq!(s.side_of_tpi(&t), Ok(tpi_oracle(&t, &s)));
    }
}

#[test]
fn a_route_is_the_first_step_that_answers() {
    assert_eq!(Step::route(true, true, true), Step::Filter);
    assert_eq!(Step::route(true, true, false), Step::Filter);
    assert_eq!(Step::route(true, false, true), Step::Narrow);
    assert_eq!(Step::route(true, false, false), Step::Exact);
    assert_eq!(Step::route(false, true, true), Step::Exact);
    assert_eq!(Step::route(false, false, false), Step::Exact);
}

#[test]
fn every_step_has_its_tables_name() {
    let names: Vec<&str> = Step::ALL.iter().map(|s| s.name()).collect();
    assert_eq!(names, ["identity", "narrow", "axis", "filter", "exact"]);
}
