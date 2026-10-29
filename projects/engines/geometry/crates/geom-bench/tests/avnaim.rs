//! Attacks on Avnaim, Boissonnat, Devillers, Preparata and Yvinec's signs of 2 × 2 and 3 × 3
//! determinants (`geom_bench::avnaim`, piece G42), chapter 23.3's reference for the exact paths of
//! orientation:
//!
//! - They give the shipped exact signs of orient2d and orient3d (`geom_num::generated`) on random
//!   grid points and at every sign combination of the grid's extremes.
//! - Against exact determinants: every 2 × 2 with entries in −7..=7 and every 3 × 3 with entries in
//!   −2..=2, entries at and one past their bounds in every position and sign, every 2 × 2 of
//!   entries near the bound, random 3 × 3s near theirs, random 3 × 3s of up to 20 bits, and 3 × 3s
//!   whose third row lies on or near the plane of the first two.
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    reason = "generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::similar_names
)]

use geom_bench::avnaim;
use geom_num::generated::{orient2d, orient3d};
use geom_num::grid::{COORD_MAX, P2, P3};
use geom_num::{I512, Sign};
use geom_test::rng::SplitMix64;
use geom_test::seed::{self, Seed};
use std::cmp::Ordering;

/// Random cases.
const CASES: usize = 20_000;

/// The default seed of the randomised tests here.
const SEED: Seed = Seed(0x0642_0000_0000_0002);

/// Draws inputs: a value of a random bit length and sign, an extreme, zero or a small value.
struct Draw(SplitMix64);

impl Draw {
    fn new(label: &str) -> Draw {
        Draw(SplitMix64::new(seed::resolve(label, SEED).unwrap()))
    }

    /// A value with |v| ≤ 2^bits − 1.
    fn value(&mut self, bits: u32) -> i128 {
        let max = (1i128 << bits) - 1;
        let pick = self.0.next_u64() % 16;
        let sign = if self.0.coin() { 1 } else { -1 };
        match pick {
            0..=3 => sign * max,
            4 | 5 => 0,
            6 | 7 => sign * i128::from(self.0.next_u64() % 4),
            _ => {
                let raw = (u128::from(self.0.next_u64()) << 64 | u128::from(self.0.next_u64()))
                    >> (128 - bits);
                let keep = self.0.next_u64() % u64::from(bits + 1);
                sign * (raw >> (u64::from(bits) - keep)) as i128
            }
        }
    }

    fn coord(&mut self) -> i64 {
        let v = self.value(40);
        assert!(v.unsigned_abs() <= COORD_MAX as u128);
        v as i64
    }

    fn p2(&mut self) -> P2 {
        P2::new(self.coord(), self.coord()).unwrap()
    }

    fn p3(&mut self) -> P3 {
        P3::new(self.coord(), self.coord(), self.coord()).unwrap()
    }
}

/// Every sign combination of ±(2^40 − 1) over `count` coordinates.
fn extremes(count: u32) -> impl Iterator<Item = Vec<i64>> {
    (0u32..1 << count).map(move |bits| {
        (0..count)
            .map(|i| {
                if bits >> i & 1 == 1 {
                    -COORD_MAX
                } else {
                    COORD_MAX
                }
            })
            .collect()
    })
}

fn p2s<const N: usize>(c: &[i64]) -> [P2; N] {
    core::array::from_fn(|i| P2::new(c[2 * i], c[2 * i + 1]).unwrap())
}

fn p3s<const N: usize>(c: &[i64]) -> [P3; N] {
    core::array::from_fn(|i| P3::new(c[3 * i], c[3 * i + 1], c[3 * i + 2]).unwrap())
}

/// The sign of an `i128`.
fn sign_of(v: i128) -> Sign {
    match v.cmp(&0) {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

/// The exact sign of x1·y2 − y1·x2: entries of at most 2^62 make products below 2^124 and their
/// difference below 2^125, so `i128` holds them.
fn det2_sign(x1: i64, y1: i64, x2: i64, y2: i64) -> Sign {
    let p = |a: i64, b: i64| i128::from(a) * i128::from(b);
    sign_of(p(x1, y2) - p(y1, x2))
}

/// The exact sign of the 3 × 3 determinant with rows `u1`, `u2`, `u3`, in `I512`, which holds
/// every product of three `i64` entries.
fn det3_sign(u1: [i64; 3], u2: [i64; 3], u3: [i64; 3]) -> Sign {
    let w = I512::from_i64;
    let mul = |a: I512, b: I512| a.checked_mul(&b).unwrap();
    let minor = |a: [i64; 3], b: [i64; 3], i: usize, j: usize| {
        mul(w(a[i]), w(b[j]))
            .checked_sub(&mul(w(a[j]), w(b[i])))
            .unwrap()
    };
    let t0 = mul(w(u1[0]), minor(u2, u3, 1, 2));
    let t1 = mul(w(u1[1]), minor(u2, u3, 0, 2));
    let t2 = mul(w(u1[2]), minor(u2, u3, 0, 1));
    t0.checked_sub(&t1)
        .unwrap()
        .checked_add(&t2)
        .unwrap()
        .signum()
}

/// The exact 3 × 3 determinant in `i128`, for entries below 2^40 (products below 2^120).
fn det3_small(u1: [i64; 3], u2: [i64; 3], u3: [i64; 3]) -> i128 {
    let w = i128::from;
    let minor =
        |a: [i64; 3], b: [i64; 3], i: usize, j: usize| w(a[i]) * w(b[j]) - w(a[j]) * w(b[i]);
    w(u1[0]) * minor(u2, u3, 1, 2) - w(u1[1]) * minor(u2, u3, 0, 2) + w(u1[2]) * minor(u2, u3, 0, 1)
}

/// Avnaim et al.'s signs of orient2d and orient3d are the shipped exact signs on random grid
/// points.
#[test]
fn avnaim_gives_the_shipped_signs_on_random_points() {
    let mut g = Draw::new("avnaim::avnaim_gives_the_shipped_signs_on_random_points");
    for _ in 0..CASES {
        let (a, b, c) = (g.p2(), g.p2(), g.p2());
        assert_eq!(avnaim::orient2d(a, b, c), orient2d::exact(a, b, c));
        let (a, b, c, d) = (g.p3(), g.p3(), g.p3(), g.p3());
        assert_eq!(avnaim::orient3d(a, b, c, d), orient3d::exact(a, b, c, d));
    }
}

/// And at every sign combination of the grid's extremes.
#[test]
fn avnaim_gives_the_shipped_signs_at_every_extreme() {
    for c in extremes(6) {
        let [a, b, e] = p2s(&c);
        assert_eq!(avnaim::orient2d(a, b, e), orient2d::exact(a, b, e));
    }
    for c in extremes(12) {
        let [a, b, e, f] = p3s(&c);
        assert_eq!(avnaim::orient3d(a, b, e, f), orient3d::exact(a, b, e, f));
    }
}

/// `avnaim`'s signs against exact determinants on every 2 × 2 with entries in −7..=7 and every
/// 3 × 3 with entries in −2..=2: every equality its tests separate occurs there (equal entries,
/// equal or opposite rows, a zero row or column, collinear first rows, a halving landing exactly
/// on a half).
#[test]
fn avnaim_signs_are_exact_on_every_small_matrix() {
    for x1 in -7..=7 {
        for y1 in -7..=7 {
            for x2 in -7..=7 {
                for y2 in -7..=7 {
                    assert_eq!(
                        avnaim::sign_of_det2(x1, y1, x2, y2),
                        Some(det2_sign(x1, y1, x2, y2)),
                        "|{x1} {y1}; {x2} {y2}|"
                    );
                }
            }
        }
    }
    let entries = [-2i64, -1, 0, 1, 2];
    for i in 0..5usize.pow(9) {
        let mut k = i;
        let mut m = [0i64; 9];
        for v in &mut m {
            *v = entries[k % 5];
            k /= 5;
        }
        let (u1, u2, u3) = ([m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]);
        let small = |a: [i64; 3], b: [i64; 3], i: usize, j: usize| {
            i128::from(a[i]) * i128::from(b[j]) - i128::from(a[j]) * i128::from(b[i])
        };
        let d = i128::from(u1[0]) * small(u2, u3, 1, 2) - i128::from(u1[1]) * small(u2, u3, 0, 2)
            + i128::from(u1[2]) * small(u2, u3, 0, 1);
        assert_eq!(avnaim::sign_of_det3(u1, u2, u3), Some(sign_of(d)), "{m:?}");
    }
}

/// `avnaim` takes entries up to its bounds, in every position and with either sign, and refuses
/// one past them; at and near the bounds its signs are exact.
#[test]
fn avnaim_entries_at_and_past_their_bounds() {
    // The bounds the module documents, 2^62 − 1 and 2^59 − 1, as numbers.
    assert!(avnaim::sign_of_det2((1 << 62) - 1, 1, 1, 1).is_some());
    assert_eq!(avnaim::sign_of_det2(1 << 62, 1, 1, 1), None);
    assert!(avnaim::sign_of_det3([(1 << 59) - 1, 1, 1], [1, 2, 3], [4, 5, 7]).is_some());
    assert_eq!(
        avnaim::sign_of_det3([1 << 59, 1, 1], [1, 2, 3], [4, 5, 7]),
        None
    );
    let m = avnaim::ENTRY_MAX;
    for pos in 0..4 {
        for s in [1i64, -1] {
            let mut at = [3i64, 5, 7, 11];
            at[pos] = s * m;
            let [x1, y1, x2, y2] = at;
            assert_eq!(
                avnaim::sign_of_det2(x1, y1, x2, y2),
                Some(det2_sign(x1, y1, x2, y2))
            );
            let mut past = at;
            past[pos] = s * (m + 1);
            let [x1, y1, x2, y2] = past;
            assert_eq!(avnaim::sign_of_det2(x1, y1, x2, y2), None, "{past:?}");
        }
    }
    let big = [
        0,
        1,
        -1,
        2,
        m,
        -m,
        m - 1,
        -(m - 1),
        m / 2,
        -(m / 2),
        m / 2 + 1,
        -(m / 2 + 1),
    ];
    for &x1 in &big {
        for &y1 in &big {
            for &x2 in &big {
                for &y2 in &big {
                    assert_eq!(
                        avnaim::sign_of_det2(x1, y1, x2, y2),
                        Some(det2_sign(x1, y1, x2, y2)),
                        "|{x1} {y1}; {x2} {y2}|"
                    );
                }
            }
        }
    }
    let d = avnaim::DET3_ENTRY_MAX;
    for pos in 0..9 {
        for s in [1i64, -1] {
            let mut at = [2i64, 3, 5, 7, 11, 13, 17, 19, 23];
            at[pos] = s * d;
            let rows = |e: [i64; 9]| ([e[0], e[1], e[2]], [e[3], e[4], e[5]], [e[6], e[7], e[8]]);
            let (u1, u2, u3) = rows(at);
            assert_eq!(
                avnaim::sign_of_det3(u1, u2, u3),
                Some(det3_sign(u1, u2, u3))
            );
            let mut past = at;
            past[pos] = s * (d + 1);
            let (u1, u2, u3) = rows(past);
            assert_eq!(avnaim::sign_of_det3(u1, u2, u3), None, "{past:?}");
        }
    }
    let mut draw = Draw::new("avnaim_entries_at_and_past_their_bounds");
    let near = [0, 1, -1, d, -d, d - 1, -(d - 1), d / 2, -(d / 2)];
    for _ in 0..300 {
        let mut e = [0i64; 9];
        for v in &mut e {
            *v = if draw.0.coin() {
                near[usize::try_from(draw.0.next_u64() % 9).unwrap()]
            } else {
                i64::try_from(draw.value(59)).unwrap()
            };
        }
        let (u1, u2, u3) = ([e[0], e[1], e[2]], [e[3], e[4], e[5]], [e[6], e[7], e[8]]);
        assert_eq!(
            avnaim::sign_of_det3(u1, u2, u3),
            Some(det3_sign(u1, u2, u3)),
            "{e:?}"
        );
    }
}

/// `avnaim::sign_of_det3` on random rows of every size up to 2^20, and on rows near and on the
/// plane of the first two (u3 = a·u1 + b·u2 plus a small offset, or none), where the lattice
/// walk's and the second step's comparisons meet equality: a third row on a lattice point, at a
/// cell's corner or on its diagonals, and at the corners' heights.
#[test]
fn avnaim_det3_on_random_and_near_plane_rows() {
    let mut draw = Draw::new("avnaim_det3_on_random_and_near_plane_rows");
    let check = |u1: [i64; 3], u2: [i64; 3], u3: [i64; 3]| {
        assert_eq!(
            avnaim::sign_of_det3(u1, u2, u3),
            Some(sign_of(det3_small(u1, u2, u3))),
            "{u1:?} {u2:?} {u3:?}"
        );
    };
    for _ in 0..20_000 {
        let bits = 1 + u32::try_from(draw.0.next_u64() % 20).unwrap();
        let mut e = [0i64; 9];
        for v in &mut e {
            *v = i64::try_from(draw.value(bits)).unwrap();
        }
        check([e[0], e[1], e[2]], [e[3], e[4], e[5]], [e[6], e[7], e[8]]);
    }
    let small = |draw: &mut Draw, n: u64| {
        i64::try_from(draw.0.next_u64() % (2 * n + 1)).unwrap() - i64::try_from(n).unwrap()
    };
    for _ in 0..40_000 {
        let bits = 1 + u32::try_from(draw.0.next_u64() % 12).unwrap();
        let mut row = || {
            [
                i64::try_from(draw.value(bits)).unwrap(),
                i64::try_from(draw.value(bits)).unwrap(),
                i64::try_from(draw.value(bits)).unwrap(),
            ]
        };
        let (u1, u2) = (row(), row());
        let (a, b) = (small(&mut draw, 6), small(&mut draw, 6));
        let off = if draw.0.coin() {
            [0; 3]
        } else {
            [
                small(&mut draw, 2),
                small(&mut draw, 2),
                small(&mut draw, 2),
            ]
        };
        let u3: [i64; 3] = core::array::from_fn(|k| a * u1[k] + b * u2[k] + off[k]);
        check(u1, u2, u3);
    }
}
