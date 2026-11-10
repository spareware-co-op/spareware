//! Independent audit of `geom_num::perturb` (symbolic perturbation ordered by keys): each
//! perturbed predicate against its own definition, evaluated another way.
//!
//! The module defines the answer as the sign, as ε goes to 0 from above, of the determinant whose
//! row for a point is its coordinates (and, for incircle and insphere, its lift |p|²), each moved
//! by ε^(2^e), and a final 1, where e is the entry's place in the order by key, then by column;
//! orient3d and insphere are that sign negated (chapter 6.2's convention). The module finds it by
//! walking bit masks and replacing rows by unit vectors. Here the determinant is expanded as a
//! polynomial in ε by Leibniz's formula over every permutation, with big-integer coefficients and
//! the exponents as bit sets, and the sign is that of the lowest-degree coefficient that is not
//! zero: no step is shared with the module.
//!
//! Inputs: points with coordinates in a small range (0, ±1, ±2, ±3, so most sets are degenerate:
//! coincident, collinear, coplanar, cocircular, cospherical), the same at the scale of the frame,
//! and wide random points; keys small or random, in every order against the arguments.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "test code: a broken oracle must stop the test loudly"
)]

use std::collections::BTreeMap;

use geom_num::grid::{P2, P3};
use geom_num::{Sign, perturb};
use num_bigint::BigInt;
use num_traits::{One, Signed, Zero};

type Poly = BTreeMap<u32, BigInt>;

fn mul(a: &Poly, b: &Poly) -> Poly {
    let mut out = Poly::new();
    for (ea, ca) in a {
        for (eb, cb) in b {
            assert_eq!(ea & eb, 0, "an entry used twice");
            *out.entry(ea | eb).or_insert_with(BigInt::zero) += ca * cb;
        }
    }
    out
}

fn permutations(n: usize) -> Vec<(Vec<usize>, bool)> {
    fn go(cur: &mut Vec<usize>, used: &mut Vec<bool>, n: usize, out: &mut Vec<(Vec<usize>, bool)>) {
        if cur.len() == n {
            let mut inv = 0;
            for i in 0..n {
                for j in i + 1..n {
                    if cur[i] > cur[j] {
                        inv += 1;
                    }
                }
            }
            out.push((cur.clone(), inv % 2 == 1));
            return;
        }
        for v in 0..n {
            if !used[v] {
                used[v] = true;
                cur.push(v);
                go(cur, used, n, out);
                cur.pop();
                used[v] = false;
            }
        }
    }
    let mut out = Vec::new();
    go(&mut Vec::new(), &mut vec![false; n], n, &mut out);
    out
}

/// The sign, as ε → 0+, of the determinant whose row i is (rows[i] perturbed, 1): entry (i, c) is
/// rows[i][c] + ε^(2^(rank(key_i)·C + c)).
fn sos(rows: &[Vec<BigInt>], keys: &[u64]) -> Sign {
    let n = rows.len();
    let c = n - 1;
    let rank = |i: usize| keys.iter().filter(|&&k| k < keys[i]).count();
    let entry = |i: usize, j: usize| -> Poly {
        let mut p = Poly::new();
        if j == c {
            p.insert(0, BigInt::one());
        } else {
            p.insert(0, rows[i][j].clone());
            p.insert(1u32 << (rank(i) * c + j), BigInt::one());
        }
        p
    };
    let mut det = Poly::new();
    for (perm, odd) in permutations(n) {
        let mut term = Poly::new();
        term.insert(0, BigInt::one());
        for (i, &j) in perm.iter().enumerate() {
            term = mul(&term, &entry(i, j));
        }
        for (e, v) in term {
            let slot = det.entry(e).or_insert_with(BigInt::zero);
            if odd {
                *slot -= v;
            } else {
                *slot += v;
            }
        }
    }
    for (_, v) in det {
        if v.is_positive() {
            return Sign::Positive;
        }
        if v.is_negative() {
            return Sign::Negative;
        }
    }
    panic!("the perturbed determinant vanishes identically");
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn small(&mut self, r: i64) -> i64 {
        (self.next() % (2 * r as u64 + 1)) as i64 - r
    }
    fn keys(&mut self, n: usize) -> Vec<u64> {
        loop {
            let k: Vec<u64> = (0..n)
                .map(|_| {
                    if self.next() & 1 == 0 {
                        self.next() % 8
                    } else {
                        self.next()
                    }
                })
                .collect();
            if (0..n).all(|i| (0..i).all(|j| k[i] != k[j])) {
                return k;
            }
        }
    }
}

fn b(v: i64) -> BigInt {
    BigInt::from(v)
}

#[test]
fn perturbed_predicates_match_their_definition() {
    let mut rng = Rng(0x6ad1_7000_5eed_0004);
    println!(
        "perturbed_predicates_match_their_definition: seed: {:016x}",
        rng.0
    );
    let n = if cfg!(debug_assertions) {
        1_000
    } else {
        60_000
    };
    let mut ties = [0usize; 4];
    for case in 0..n {
        let r = [0, 1, 1, 2, 3, 1 << 20][(rng.next() % 6) as usize];
        let scale = if rng.next() % 4 == 0 {
            (1i64 << 39) / r.max(1)
        } else {
            1
        };
        // 2D
        let pts: Vec<[i64; 2]> = (0..4)
            .map(|_| [rng.small(r) * scale, rng.small(r) * scale])
            .collect();
        let keys = rng.keys(4);
        let p = |i: usize| (P2::new(pts[i][0], pts[i][1]).unwrap(), keys[i]);
        let rows3: Vec<Vec<BigInt>> = (0..3).map(|i| vec![b(pts[i][0]), b(pts[i][1])]).collect();
        let want = sos(&rows3, &keys[..3]);
        let got = perturb::orient2d(p(0), p(1), p(2)).unwrap();
        assert_eq!(got, want, "case {case}: orient2d {pts:?} {keys:?}");
        ties[0] += usize::from(
            geom_num::predicate::orient2d(p(0).0, p(1).0, p(2).0).unwrap() == Sign::Zero,
        );
        let lift2 = |q: [i64; 2]| b(q[0]) * b(q[0]) + b(q[1]) * b(q[1]);
        let rows4: Vec<Vec<BigInt>> = (0..4)
            .map(|i| vec![b(pts[i][0]), b(pts[i][1]), lift2(pts[i])])
            .collect();
        let want = sos(&rows4, &keys);
        let got = perturb::incircle(p(0), p(1), p(2), p(3)).unwrap();
        assert_eq!(got, want, "case {case}: incircle {pts:?} {keys:?}");
        ties[1] += usize::from(
            geom_num::predicate::incircle(p(0).0, p(1).0, p(2).0, p(3).0).unwrap() == Sign::Zero,
        );
        // 3D
        let pts: Vec<[i64; 3]> = (0..5)
            .map(|_| {
                [
                    rng.small(r) * scale,
                    rng.small(r) * scale,
                    rng.small(r) * scale,
                ]
            })
            .collect();
        let keys = rng.keys(5);
        let p = |i: usize| (P3::new(pts[i][0], pts[i][1], pts[i][2]).unwrap(), keys[i]);
        let rows4: Vec<Vec<BigInt>> = (0..4)
            .map(|i| pts[i].iter().map(|&v| b(v)).collect())
            .collect();
        let want = sos(&rows4, &keys[..4]).negate();
        let got = perturb::orient3d(p(0), p(1), p(2), p(3)).unwrap();
        assert_eq!(got, want, "case {case}: orient3d {pts:?} {keys:?}");
        ties[2] += usize::from(
            geom_num::predicate::orient3d(p(0).0, p(1).0, p(2).0, p(3).0).unwrap() == Sign::Zero,
        );
        let lift3 = |q: [i64; 3]| b(q[0]) * b(q[0]) + b(q[1]) * b(q[1]) + b(q[2]) * b(q[2]);
        let rows5: Vec<Vec<BigInt>> = (0..5)
            .map(|i| {
                let mut v: Vec<BigInt> = pts[i].iter().map(|&v| b(v)).collect();
                v.push(lift3(pts[i]));
                v
            })
            .collect();
        let want = sos(&rows5, &keys).negate();
        let got = perturb::insphere(p(0), p(1), p(2), p(3), p(4)).unwrap();
        assert_eq!(got, want, "case {case}: insphere {pts:?} {keys:?}");
        ties[3] += usize::from(
            geom_num::predicate::insphere(p(0).0, p(1).0, p(2).0, p(3).0, p(4).0).unwrap()
                == Sign::Zero,
        );
    }
    println!(
        "exact ties broken: orient2d {}, incircle {}, orient3d {}, insphere {}",
        ties[0], ties[1], ties[2], ties[3]
    );
}
