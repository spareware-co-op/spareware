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
