//! Tests that run, through the crate's public interface, the lines of G29's implicit points that no
//! other integration test reached (piece G21, the coverage gate of section 28.8).
//!
//! - **`generated::tpi_weight`, `lpi_weight` and `ssi_weight`.** The generated filter and exact
//!   path of the weight of a three-plane, line-plane and crossing point. `Plane::side_of_tpi`,
//!   `Lpi` and `Ssi` reach their weights through other paths for most inputs, so nothing ran their
//!   exact paths: here the exact value against a `num-bigint` oracle, on random inputs at every
//!   scale up to the extremes of the declared ranges, where the filter must answer the exact sign or
//!   nothing; `None` for an input one past its range.
//! - **`Lpi::points`, `Lpi::plane` and `Ssi::points`.** The accessors, which return what the
//!   point was built from.
//! - **`Ssi::orient2d` and `Plane::side_of_lpi` where the weight's filter cannot decide.** Two
//!   nearly parallel lines, and a line nearly parallel to its plane, at the edge of the grid: the
//!   weight is a small difference of huge products, so the classification takes the weight's exact
//!   path, which no other test made it take (piece G41's waiting lines). Each answer against the
//!   side of the point itself, formed by division in exact rationals (`num-rational`).
//!
//! Seeds: the randomised tests print `seed: <hex>` and run from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    reason = "oracles and generators, named as the formulas name them; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a failing test should stop loudly"
)]

use geom_num::crossing::Ssi;
use geom_num::generated::{lpi_weight, ssi_weight, tpi_weight};
use geom_num::grid::COORD_MAX;
use geom_num::plane::{Lpi, Plane};
use geom_num::{P2, P3, Sign};
use num_bigint::BigInt;
use num_rational::BigRational;

const DEFAULT_SEED: u64 = 0x6721_0000_5eed_0002;

/// The largest magnitude the weights' descriptions declare for a normal component: 83 bits.
const NORMAL_MAX: i128 = (1 << 83) - 1;

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), as the crate's other tests write it.
struct Rng(u64);

impl Rng {
    fn for_test(name: &str) -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_SEED);
        println!("{name}: seed: {seed:016x}");
        Rng(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A value of magnitude at most `max`, at a random scale: within 2^k of zero for a random k up
    /// to `max`'s bit length, or one of `±max` exactly.
    fn scaled(&mut self, max: i128) -> i128 {
        let bits = 128 - max.leading_zeros();
        match self.next() % 8 {
            0 => max,
            1 => -max,
            _ => {
                let k = (self.next() % u64::from(bits + 1)) as u32;
                let wide = (u128::from(self.next()) << 64) | u128::from(self.next());
                let magnitude = if k >= 127 {
                    wide >> 1
                } else {
                    wide & ((1u128 << k) - 1)
                };
                let v = (magnitude as i128).min(max);
                if self.next() & 1 == 0 { v } else { -v }
            }
        }
    }

    fn coord(&mut self) -> i64 {
        self.scaled(i128::from(COORD_MAX)) as i64
    }
}

fn sign_of(v: &BigInt) -> Sign {
    match v.sign() {
        num_bigint::Sign::Minus => Sign::Negative,
        num_bigint::Sign::NoSign => Sign::Zero,
        num_bigint::Sign::Plus => Sign::Positive,
    }
}

fn big(v: i128) -> BigInt {
    BigInt::from(v)
}

/// n1 · (n2 × n3) exactly.
fn tpi_oracle(n: [[i128; 3]; 3]) -> BigInt {
    let [a, b, c] = n.map(|v| v.map(big));
    let cross = [
        &b[1] * &c[2] - &b[2] * &c[1],
        &b[2] * &c[0] - &b[0] * &c[2],
        &b[0] * &c[1] - &b[1] * &c[0],
    ];
    &a[0] * &cross[0] + &a[1] * &cross[1] + &a[2] * &cross[2]
}

#[test]
fn the_three_plane_weight_is_exact_and_its_filter_never_guesses() {
    let mut rng = Rng::for_test("tpi_weight");
    for case in 0..20_000 {
        let mut n = [[0i128; 3]; 3];
        for row in &mut n {
            for x in row.iter_mut() {
                *x = rng.scaled(NORMAL_MAX);
            }
        }
        // A quarter of the cases make the third normal a combination of the first two, so the
        // weight is exactly zero.
        if case % 4 == 0 {
            let (s, t) = (
                i128::from(rng.next() % 5) - 2,
                i128::from(rng.next() % 5) - 2,
            );
            let [first, second, _] = n.map(|row| row.map(|v| v >> 3));
            n = [
                first,
                second,
                [0, 1, 2].map(|k| s * first[k] + t * second[k]),
            ];
        }
        let [[a, b, c], [d, e, f], [g, h, i]] = n;
        let expected = tpi_oracle(n);
        let sign = sign_of(&expected);
        let value = tpi_weight::value(a, b, c, d, e, f, g, h, i).expect("inside the ranges");
        assert_eq!(value.signum(), sign, "{n:?}");
        assert_eq!(
            tpi_weight::exact(a, b, c, d, e, f, g, h, i),
            Some(sign),
            "{n:?}"
        );
        if let Some(s) = tpi_weight::filter(a, b, c, d, e, f, g, h, i) {
            assert_eq!(s, sign, "the filter guessed for {n:?}");
        }
    }
    // One past the declared range, in each position, is refused.
    for k in 0..9 {
        let mut v = [1i128; 9];
        v[k] = NORMAL_MAX + 1;
        let [a, b, c, d, e, f, g, h, i] = v;
        assert_eq!(tpi_weight::value(a, b, c, d, e, f, g, h, i), None, "{k}");
        assert_eq!(tpi_weight::exact(a, b, c, d, e, f, g, h, i), None, "{k}");
    }
}

#[test]
fn the_line_plane_weight_is_exact_and_its_filter_never_guesses() {
    let mut rng = Rng::for_test("lpi_weight");
    for case in 0..20_000 {
        let n = [
            rng.scaled(NORMAL_MAX),
            rng.scaled(NORMAL_MAX),
            rng.scaled(NORMAL_MAX),
        ];
        let p = [rng.coord(), rng.coord(), rng.coord()];
        let mut q = [rng.coord(), rng.coord(), rng.coord()];
        // Every eighth case puts q − p in the plane of the normal, so the weight is exactly zero
        // or nearly so, where the filter must refuse rather than guess.
        if case % 8 == 0 {
            q = p;
        }
        let expected: BigInt = (0..3)
            .map(|k| big(n[k]) * (BigInt::from(q[k]) - BigInt::from(p[k])))
            .sum();
        let (pp, qp) = (
            P3::new(p[0], p[1], p[2]).unwrap(),
            P3::new(q[0], q[1], q[2]).unwrap(),
        );
        let value = lpi_weight::value(n[0], n[1], n[2], pp, qp).expect("inside the ranges");
        assert_eq!(big(value), expected, "{n:?} {p:?} {q:?}");
        let sign = sign_of(&expected);
        assert_eq!(lpi_weight::exact(n[0], n[1], n[2], pp, qp), Some(sign));
        if let Some(s) = lpi_weight::filter(n[0], n[1], n[2], pp, qp) {
            assert_eq!(s, sign, "the filter guessed for {n:?} {p:?} {q:?}");
        }
    }
    let o = P3::new(0, 0, 0).unwrap();
    let x = P3::new(1, 0, 0).unwrap();
    assert_eq!(lpi_weight::value(NORMAL_MAX + 1, 0, 0, o, x), None);
    assert_eq!(lpi_weight::exact(NORMAL_MAX + 1, 0, 0, o, x), None);
}

#[test]
fn the_crossing_weight_is_exact_and_its_filter_never_guesses() {
    let mut rng = Rng::for_test("ssi_weight");
    for case in 0..20_000 {
        let mut pts = [[0i64; 2]; 4];
        for p in &mut pts {
            *p = [rng.coord(), rng.coord()];
        }
        // Every eighth case makes the two segments parallel: the weight is exactly zero.
        if case % 8 == 0 {
            let k = (rng.next() % 3) as i64 - 1;
            let dx = (pts[1][0] - pts[0][0]) / 2;
            let dy = (pts[1][1] - pts[0][1]) / 2;
            pts[3] = [pts[2][0] + k * dx, pts[2][1] + k * dy];
            if pts[3][0].abs() > COORD_MAX || pts[3][1].abs() > COORD_MAX {
                pts[3] = pts[2];
            }
        }
        let [a, b, c, d] = pts.map(|p| P2::new(p[0], p[1]).unwrap());
        let ba = [pts[1][0] - pts[0][0], pts[1][1] - pts[0][1]];
        let dc = [pts[3][0] - pts[2][0], pts[3][1] - pts[2][1]];
        let expected =
            BigInt::from(ba[0]) * BigInt::from(dc[1]) - BigInt::from(ba[1]) * BigInt::from(dc[0]);
        let value = ssi_weight::value(a, b, c, d).expect("inside the ranges");
        assert_eq!(big(value), expected, "{pts:?}");
        let sign = sign_of(&expected);
        assert_eq!(ssi_weight::exact(a, b, c, d), Some(sign));
        if let Some(s) = ssi_weight::filter(a, b, c, d) {
            assert_eq!(s, sign, "the filter guessed for {pts:?}");
        }
    }
}

#[test]
fn the_implicit_points_return_what_they_were_built_from() {
    let p = |x, y, z| P3::new(x, y, z).unwrap();
    let plane = Plane::through(p(0, 0, 0), p(1, 0, 0), p(0, 1, 0)).unwrap();
    let (a, b) = (p(3, -2, -5), p(3, -2, 7));
    let l = Lpi::new(a, b, plane).unwrap();
    assert_eq!(l.points(), [a, b]);
    assert_eq!(*l.plane(), plane);
    let q = |x, y| P2::new(x, y).unwrap();
    let pts = [q(0, 0), q(4, 4), q(0, 4), q(4, 0)];
    let s = Ssi::new(pts[0], pts[1], pts[2], pts[3]).unwrap();
    assert_eq!(s.points(), pts);
}

// ---------------------------------------------------------------------------------------------
// The exact weight, where the generated filter cannot decide it (piece G41's waiting lines).
// ---------------------------------------------------------------------------------------------

const M: i64 = COORD_MAX;

fn rational(v: i64) -> BigRational {
    BigRational::from_integer(BigInt::from(v))
}

fn sign_of_rational(v: &BigRational) -> Sign {
    sign_of(v.numer())
}

/// orient2d(p, q, x) for the point x where the line a b crosses the line c d, in exact rationals:
/// x = a + t (b − a) with t = orient2d(c, d, a) / (orient2d(c, d, a) − orient2d(c, d, b)), then
/// (q − p) × (x − p). The point is formed by division, which the kernel never does.
fn ssi_side_oracle(pts: [[i64; 2]; 4], p: [i64; 2], q: [i64; 2]) -> Sign {
    let cross = |o: [i64; 2], u: [i64; 2], v: [i64; 2]| {
        (rational(u[0]) - rational(o[0])) * (rational(v[1]) - rational(o[1]))
            - (rational(u[1]) - rational(o[1])) * (rational(v[0]) - rational(o[0]))
    };
    let [a, b, c, d] = pts;
    let (oa, ob) = (cross(c, d, a), cross(c, d, b));
    let t = &oa / (&oa - &ob);
    let x = [0, 1].map(|k| rational(a[k]) + &t * (rational(b[k]) - rational(a[k])));
    let side = (rational(q[0]) - rational(p[0])) * (&x[1] - rational(p[1]))
        - (rational(q[1]) - rational(p[1])) * (&x[0] - rational(p[0]));
    sign_of_rational(&side)
}

/// Two lines whose directions (M, M − 1) and (M − 1, M − 2) are nearly parallel: the weight
/// (b − a) × (d − c) is M (M − 2) − (M − 1)² = −1, the difference of two products near 2^80, far
/// inside the `f64` filter's bound. So `Ssi::orient2d` must take the weight's exact path, which
/// no other test made it do. Its answer is checked against the side of the crossing point
/// computed in exact rationals, for lines through the point (the two lines themselves, either
/// way round: `Zero`), for lines beside it, and for random lines at every scale; with the second
/// line reversed the weight is +1 and every answer is the same, since the point is the same.
#[test]
fn a_crossing_whose_weight_the_filter_cannot_decide_is_classified_exactly() {
    let mut rng = Rng::for_test("ssi undecided weight");
    let q2 = |p: [i64; 2]| P2::new(p[0], p[1]).unwrap();
    let (a, b, c, d) = ([0, 0], [M, M - 1], [-3, -7], [M - 4, M - 9]);
    for (pts, weight) in [
        ([a, b, c, d], Sign::Negative),
        ([a, b, d, c], Sign::Positive),
    ] {
        let [pa, pb, pc, pd] = pts.map(q2);
        assert_eq!(ssi_weight::filter(pa, pb, pc, pd), None, "{pts:?}");
        assert_eq!(ssi_weight::exact(pa, pb, pc, pd), Some(weight), "{pts:?}");
        let s = Ssi::new(pa, pb, pc, pd).unwrap();

        let check = |p: [i64; 2], q: [i64; 2]| -> Sign {
            let expected = ssi_side_oracle(pts, p, q);
            assert_eq!(
                s.orient2d(q2(p), q2(q)),
                Ok(expected),
                "{pts:?} {p:?} {q:?}"
            );
            expected
        };
        // The point lies on both of its own lines, whichever way each is walked.
        for (p, q) in [(a, b), (b, a), (c, d), (d, c)] {
            assert_eq!(check(p, q), Sign::Zero);
        }
        // With orient2d(c, d, a) = 4M − 1 and orient2d(c, d, b) = 4M, the lines cross at
        // −(4M − 1) b, far below and to the left of the frame. So the x axis walked rightwards
        // has the point on its right, and walked leftwards on its left.
        assert_eq!(check([0, 0], [1, 0]), Sign::Negative);
        assert_eq!(check([1, 0], [0, 0]), Sign::Positive);
        // A line parallel to a b, one step above it and one step below it.
        assert_eq!(check([0, 1], [M, M]), Sign::Negative);
        assert_eq!(check([0, -1], [M, M - 2]), Sign::Positive);

        let mut seen = [0usize; 3];
        for _ in 0..4_000 {
            let p = [rng.coord(), rng.coord()];
            let q = [rng.coord(), rng.coord()];
            seen[match check(p, q) {
                Sign::Negative => 0,
                Sign::Zero => 1,
                Sign::Positive => 2,
            }] += 1;
        }
        assert!(seen[0] > 0 && seen[2] > 0, "{seen:?}");
    }
}

/// n · x + d at the point x where the line p q meets `first`, for the plane (n, d) = `second`,
/// in exact rationals: x = p + t (q − p) with t = −(n₁ · p + d₁) / (n₁ · (q − p)). The point is
/// formed by division, which the kernel never does.
fn lpi_side_oracle(first: &Plane, second: &Plane, p: [i64; 3], q: [i64; 3]) -> Sign {
    let int = |v: i128| BigRational::from_integer(BigInt::from(v));
    let at = |plane: &Plane, x: &[BigRational; 3]| {
        let n = plane.n();
        int(n[0]) * &x[0] + int(n[1]) * &x[1] + int(n[2]) * &x[2] + int(plane.d())
    };
    let (rp, rq) = (p.map(rational), q.map(rational));
    let (sp, sq) = (at(first, &rp), at(first, &rq));
    let t = -&sp / (&sq - &sp);
    let x = [0, 1, 2].map(|k| &rp[k] + &t * (&rq[k] - &rp[k]));
    sign_of_rational(&at(second, &x))
}

/// A line nearly parallel to its plane: the plane through (0, 0, 0), (M, M − 1, 0) and
/// (0, 0, M) has the normal (M (M − 1), −M², 0), and the line's direction (M − 1, M − 2, 0)
/// gives the weight n · (q − p) = M ((M − 1)² − M (M − 2)) = M, the difference of two products
/// near 2^120, far inside the `f64` filter's bound. So `Plane::side_of_lpi` must take the
/// weight's exact path, which no other test made it do. Its answer is checked against the side
/// of the meeting point computed in exact rationals: `Zero` for the line's own plane and for
/// planes that hold the line, and the oracle's sign for planes through random grid points at
/// every scale; with the line reversed the weight is −M and every answer is the same.
#[test]
fn a_line_plane_point_whose_weight_the_filter_cannot_decide_is_classified_exactly() {
    let mut rng = Rng::for_test("lpi undecided weight");
    let p3 = |p: [i64; 3]| P3::new(p[0], p[1], p[2]).unwrap();
    let through = |a: [i64; 3], b: [i64; 3], c: [i64; 3]| Plane::through(p3(a), p3(b), p3(c));
    let first = through([0, 0, 0], [M, M - 1, 0], [0, 0, M]).unwrap();
    let m = i128::from(M);
    assert_eq!(first.n(), [m * (m - 1), -m * m, 0]);
    let (p, q) = ([0, 0, 1], [M - 1, M - 2, 1]);
    for (p, q, weight) in [(p, q, Sign::Positive), (q, p, Sign::Negative)] {
        let [nx, ny, nz] = first.n();
        assert_eq!(lpi_weight::filter(nx, ny, nz, p3(p), p3(q)), None);
        assert_eq!(lpi_weight::exact(nx, ny, nz, p3(p), p3(q)), Some(weight));
        assert_eq!(
            lpi_weight::value(nx, ny, nz, p3(p), p3(q)).map(i128::abs),
            Some(m)
        );
        let l = Lpi::new(p3(p), p3(q), first).unwrap();

        let check = |second: &Plane| -> Sign {
            let expected = lpi_side_oracle(&first, second, p, q);
            assert_eq!(
                second.side_of_lpi(&l),
                Ok(expected),
                "{second:?} {p:?} {q:?}"
            );
            expected
        };
        // The point lies on its own plane, and on every plane that holds its line.
        assert_eq!(check(&first), Sign::Zero);
        assert_eq!(check(&through(p, q, [0, 0, 0]).unwrap()), Sign::Zero);
        assert_eq!(check(&through(p, q, [-M, M, -M]).unwrap()), Sign::Zero);
        // The line runs at height 1: the plane z = 0 with its normal up has the point above it,
        // and the same plane with its normal down has it below.
        let floor = through([0, 0, 0], [1, 0, 0], [0, 1, 0]).unwrap();
        assert_eq!(floor.n(), [0, 0, 1]);
        assert_eq!(check(&floor), Sign::Positive);
        let ceiling = through([0, 0, 0], [0, 1, 0], [1, 0, 0]).unwrap();
        assert_eq!(check(&ceiling), Sign::Negative);

        let mut seen = [0usize; 3];
        for _ in 0..4_000 {
            let mut corner = || [rng.coord(), rng.coord(), rng.coord()];
            // Three collinear points make no plane; such a draw is skipped.
            let Ok(second) = through(corner(), corner(), corner()) else {
                continue;
            };
            seen[match check(&second) {
                Sign::Negative => 0,
                Sign::Zero => 1,
                Sign::Positive => 2,
            }] += 1;
        }
        assert!(seen[0] > 0 && seen[2] > 0, "{seen:?}");
    }
}
