//! Attacks on what piece G196b adds to `geom_num::quartic` for the points where an ellipse meets
//! another conic: `sign_at_root`, the sign of one quartic at a real root of another, which is the
//! side of a line, a circle or an ellipse that such a point lies on (chapter 14.4 of the kernel
//! architecture).
//!
//! - **Exact against an oracle**: random quartics built from linear factors with rational roots
//!   and quadratic factors with roots ±√k or none; at every real root of one, the sign of the other
//!   is the product of its factors' signs there, each decided by exact arithmetic on its own.
//! - **Shared roots are zero**, at every multiplicity, and roots a hair apart are not.
//! - **The ends of the line**: at t = ∞ the sign is that of the coefficient of t⁴, zero when it is
//!   zero; the exact roots −1, 0 and 1 are read exactly.
//! - **Degenerate**: the zero polynomial is zero at every root, a constant has its own sign.
//! - **A half turn** (`Quartic::turned`, `RealRoot::turned`, t to −1 / t): every root turned is a
//!   root of the turned quartic, and turned roots compare as −1 / t does, by the oracle, with 0
//!   going to ∞ and 1 and −1 to each other.
//! - **The second instance** (`SecondQuartic`, chapter 14.2's as-built note of G196b): quartics of
//!   four rational roots of up to 160-bit numerators and denominators, past the first instance's
//!   budget, give back their roots in order, compare exactly with each other's, sign each other at
//!   them, and part two roots 2^−400 apart, all against exact rational arithmetic; a first-instance
//!   quartic and its roots carry over whole (`to_second`).
//!
//! Seeds: the random test prints `seed: <hex>` and runs from a fixed default; `SPAREKIT_SEED=<hex>`
//! replays another (chapter 28.17). Case counts are a tenth in debug builds.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "generators and oracles; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::cmp::Ordering;

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_num::Sign;
use geom_num::quartic::{
    Big, COEFFICIENT_BITS, Piece, Quartic, RealRoot, SECOND_COEFFICIENT_BITS, SECOND_LIMBS,
    SecondQuartic, SecondRoot, compare, sign_at_root,
};
use geom_num::wide::Wide;

/// The default seed of the random test here (G196b).
const DEFAULT_SEED: u64 = 0x6196_b000_5eed_196b;

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), written out as the other tests do.
struct Rng(u64);

impl Rng {
    fn new() -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_SEED);
        println!("seed: {seed:x}");
        Rng(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A whole number from `lo` to `hi`.
    fn range(&mut self, lo: i128, hi: i128) -> i128 {
        lo + i128::from(self.next() % (hi - lo + 1) as u64)
    }
}

/// One factor of a test quartic: q t − p (a root p / q, q > 0), or t² − k (roots ±√k when k > 0,
/// none when k < 0).
#[derive(Clone, Copy, Debug)]
enum Factor {
    Linear { q: i128, p: i128 },
    Square { k: i128 },
}

/// A real number the oracle knows exactly: p / q with q > 0, or s√k with k > 0 not a square.
#[derive(Clone, Copy, Debug)]
enum Exact {
    Ratio { p: i128, q: i128 },
    Root { s: i128, k: i128 },
}

/// The sign of a + b√k, k > 0, by comparing squares.
fn sign_of_surd(a: i128, b: i128, k: i128) -> Sign {
    let (sa, sb) = (a.signum(), b.signum());
    if sb == 0 || sa == sb {
        return Sign::of_i128(if sa == 0 { sb } else { sa });
    }
    if sa == 0 {
        return Sign::of_i128(sb);
    }
    // Opposite signs: the larger square wins.
    match (a * a).cmp(&(b * b * k)) {
        Ordering::Greater => Sign::of_i128(sa),
        Ordering::Less => Sign::of_i128(sb),
        Ordering::Equal => Sign::Zero,
    }
}

impl Exact {
    /// The sign of this number less `other`.
    fn cmp(self, other: Exact) -> Sign {
        match (self, other) {
            (Exact::Ratio { p, q }, Exact::Ratio { p: p2, q: q2 }) => {
                Sign::of_i128(p * q2 - p2 * q)
            }
            // p / q − s√k: the sign of p − q s √k.
            (Exact::Ratio { p, q }, Exact::Root { s, k }) => sign_of_surd(p, -q * s, k),
            (Exact::Root { .. }, Exact::Ratio { .. }) => other.cmp(self).negate(),
            (Exact::Root { s, k }, Exact::Root { s: s2, k: k2 }) => {
                if s != s2 {
                    return Sign::of_i128(s - s2);
                }
                // s√k − s√k2 has s's sign when k > k2.
                let by = Sign::of_i128((k - k2).signum());
                if s > 0 { by } else { by.negate() }
            }
        }
    }

    /// The sign of a factor at this number.
    fn sign_of(self, f: Factor) -> Sign {
        match (f, self) {
            (Factor::Linear { q, p }, Exact::Ratio { p: a, q: b }) => Sign::of_i128(q * a - p * b),
            // q s√k − p.
            (Factor::Linear { q, p }, Exact::Root { s, k }) => sign_of_surd(-p, q * s, k),
            (Factor::Square { k }, Exact::Ratio { p, q }) => Sign::of_i128(p * p - k * q * q),
            (Factor::Square { k: k2 }, Exact::Root { k, .. }) => Sign::of_i128(k - k2),
        }
    }
}

/// The real roots of a factor.
fn roots_of(f: Factor) -> Vec<Exact> {
    match f {
        Factor::Linear { q, p } => vec![Exact::Ratio { p, q }],
        Factor::Square { k } if k > 0 => vec![Exact::Root { s: -1, k }, Exact::Root { s: 1, k }],
        Factor::Square { .. } => vec![],
    }
}

/// The polynomial `scale` times the product of `factors`, lowest term first.
fn quartic(scale: i128, factors: &[Factor]) -> Quartic {
    let mut c = vec![scale];
    for f in factors {
        let by: Vec<i128> = match *f {
            Factor::Linear { q, p } => vec![-p, q],
            Factor::Square { k } => vec![-k, 0, 1],
        };
        let mut next = vec![0; c.len() + by.len() - 1];
        for (i, a) in c.iter().enumerate() {
            for (j, b) in by.iter().enumerate() {
                next[i + j] += a * b;
            }
        }
        c = next;
    }
    assert!(c.len() <= 5, "degree four at most");
    let mut out = [Big::from_i64(0); 5];
    for (slot, v) in out.iter_mut().zip(&c) {
        *slot = Big::from_i128(*v);
    }
    Quartic::new(out)
}

/// The degree of a product of factors.
fn degree(factors: &[Factor]) -> usize {
    let mut n = 0;
    for f in factors {
        n += match f {
            Factor::Linear { .. } => 1,
            Factor::Square { .. } => 2,
        };
    }
    n
}

/// The sign the oracle gives `scale · ∏ factors` at `x`.
fn oracle(scale: i128, factors: &[Factor], x: Exact) -> Sign {
    factors
        .iter()
        .fold(Sign::of_i128(scale), |s, f| s.mul(x.sign_of(*f)))
}

/// The distinct real roots of a product of factors, increasing.
fn distinct(factors: &[Factor]) -> Vec<Exact> {
    let mut all: Vec<Exact> = factors.iter().flat_map(|f| roots_of(*f)).collect();
    all.sort_by(|a, b| match a.cmp(*b) {
        Sign::Negative => Ordering::Less,
        Sign::Zero => Ordering::Equal,
        Sign::Positive => Ordering::Greater,
    });
    all.dedup_by(|a, b| a.cmp(*b) == Sign::Zero);
    all
}

/// A random factor: a rational root of small numbers, or ±√k, or no real root.
fn factor(rng: &mut Rng) -> Factor {
    match rng.range(0, 3) {
        0 | 1 => Factor::Linear {
            q: rng.range(1, 7),
            p: rng.range(-20, 20),
        },
        2 => Factor::Square {
            k: [2, 3, 5, 6, 7, 8, 10][rng.range(0, 6) as usize],
        },
        _ => Factor::Square {
            k: -rng.range(1, 5),
        },
    }
}

/// Factors of degree at most four, with at least one real root among them when `real`.
fn factors(rng: &mut Rng, real: bool) -> Vec<Factor> {
    loop {
        let mut out = Vec::new();
        while degree(&out) < 4 && rng.range(0, 4) > 0 {
            let f = factor(rng);
            let mut with = out.clone();
            with.push(f);
            if degree(&with) <= 4 {
                out = with;
            }
        }
        if !real || !distinct(&out).is_empty() {
            return out;
        }
    }
}

/// The finite roots of `q`, and whether it has one at t = ∞.
fn found(q: &Quartic) -> (Vec<RealRoot>, bool) {
    let roots: Vec<RealRoot> = q.roots().unwrap().iter().copied().collect();
    let infinite = roots.last().is_some_and(|r| r.piece() == Piece::Infinity);
    let finite = roots
        .into_iter()
        .filter(|r| r.piece() != Piece::Infinity)
        .collect();
    (finite, infinite)
}

#[test]
fn the_sign_at_every_root_is_the_oracle_s() {
    let mut rng = Rng::new();
    let cases = if cfg!(debug_assertions) { 150 } else { 1500 };
    let (mut zeros, mut signs, mut far) = (0, 0, 0);
    for case in 0..cases {
        let (qf, pf) = (factors(&mut rng, true), factors(&mut rng, false));
        let (qs, ps) = (
            rng.range(1, 3) * [1, -1][rng.range(0, 1) as usize],
            rng.range(-3, 3),
        );
        let (q, p) = (quartic(qs, &qf), quartic(ps, &pf));
        let want = distinct(&qf);
        let (roots, infinite) = found(&q);
        assert_eq!(roots.len(), want.len(), "case {case}: {qf:?}");
        for (r, x) in roots.iter().zip(&want) {
            let sign = sign_at_root(&p, &q, r).unwrap();
            assert_eq!(
                sign,
                oracle(ps, &pf, *x),
                "case {case}: {ps} {pf:?} at {x:?}, a root of {qf:?}"
            );
            if sign == Sign::Zero {
                zeros += 1;
            } else {
                signs += 1;
            }
        }
        // q of degree below four has the root t = ∞, where p read at degree four is its
        // coefficient of t⁴.
        assert_eq!(infinite, degree(&qf) < 4, "case {case}");
        if infinite {
            let r = q.roots().unwrap().iter().copied().last().unwrap();
            let top = if degree(&pf) == 4 {
                Sign::of_i128(ps)
            } else {
                Sign::Zero
            };
            assert_eq!(sign_at_root(&p, &q, &r).unwrap(), top, "case {case}");
            far += 1;
        }
    }
    println!("{zeros} shared roots, {signs} signs, {far} at infinity");
    assert!(zeros > cases / 20 && signs > cases && far > cases / 10);
}

#[test]
fn a_shared_root_is_zero_at_every_multiplicity_and_a_near_one_is_not() {
    let two = Factor::Square { k: 2 };
    let third = Factor::Linear { q: 3, p: 1 };
    // q has −√2, 1/3 and √2; p has √2 twice, and 1/3 three times.
    let q = quartic(1, &[two, third]);
    let p = quartic(-2, &[two, two]);
    let thrice = quartic(5, &[third, third, third]);
    let (roots, _) = found(&q);
    assert_eq!(roots.len(), 3);
    let at = |p: &Quartic| -> Vec<Sign> {
        roots
            .iter()
            .map(|r| sign_at_root(p, &q, r).unwrap())
            .collect()
    };
    // −2(t² − 2)² at −√2, 1/3 and √2.
    assert_eq!(at(&p), [Sign::Zero, Sign::Negative, Sign::Zero]);
    // 5(t − 1/3)³ · 27: negative below 1/3, zero there, positive above.
    assert_eq!(at(&thrice), [Sign::Negative, Sign::Zero, Sign::Positive]);
    // √(2 + 2^−80) is a hair above √2: t² − 2 − 2^−80 is negative at √2 and at −√2, and positive
    // nowhere among q's roots but beyond them.
    let k = 1_i128 << 80;
    let near = Quartic::new([
        Big::from_i128(-(2 * k + 1)),
        Big::from_i64(0),
        Big::from_i128(k),
        Big::from_i64(0),
        Big::from_i64(0),
    ]);
    assert_eq!(at(&near), [Sign::Negative, Sign::Negative, Sign::Negative]);
    // An even root below turns nothing: 4(t + 3)(2t − 1)² is negative at −5 and positive at 7.
    let even = quartic(
        4,
        &[
            Factor::Linear { q: 1, p: -3 },
            Factor::Linear { q: 2, p: 1 },
            Factor::Linear { q: 2, p: 1 },
        ],
    );
    let ends = quartic(
        1,
        &[
            Factor::Linear { q: 1, p: 7 },
            Factor::Linear { q: 1, p: -5 },
        ],
    );
    let (r, _) = found(&ends);
    assert_eq!(sign_at_root(&even, &ends, &r[0]).unwrap(), Sign::Negative);
    assert_eq!(sign_at_root(&even, &ends, &r[1]).unwrap(), Sign::Positive);
}

#[test]
fn the_exact_roots_and_infinity_are_read_exactly() {
    // q = t(t − 1)(t + 1): the roots −1, 0 and 1, and ∞ (degree three).
    let q = quartic(
        1,
        &[
            Factor::Linear { q: 1, p: 0 },
            Factor::Linear { q: 1, p: 1 },
            Factor::Linear { q: 1, p: -1 },
        ],
    );
    let all: Vec<RealRoot> = q.roots().unwrap().iter().copied().collect();
    let pieces: Vec<Piece> = all.iter().map(RealRoot::piece).collect();
    assert_eq!(
        pieces,
        [Piece::MinusOne, Piece::Zero, Piece::One, Piece::Infinity]
    );
    let signs = |p: &Quartic| -> Vec<Sign> {
        all.iter()
            .map(|r| sign_at_root(p, &q, r).unwrap())
            .collect()
    };
    // 2t − 1: −3, −1, 1, and no t⁴ term.
    let line = quartic(1, &[Factor::Linear { q: 2, p: 1 }]);
    assert_eq!(
        signs(&line),
        [Sign::Negative, Sign::Negative, Sign::Positive, Sign::Zero]
    );
    // −(t² + 1)²: negative everywhere, its t⁴ term too.
    let below = quartic(-1, &[Factor::Square { k: -1 }, Factor::Square { k: -1 }]);
    assert_eq!(signs(&below), [Sign::Negative; 4]);
    // t⁴ − t² = t²(t − 1)(t + 1): zero at −1, 0 and 1, positive at ∞.
    let shared = quartic(
        1,
        &[
            Factor::Linear { q: 1, p: 0 },
            Factor::Linear { q: 1, p: 0 },
            Factor::Linear { q: 1, p: 1 },
            Factor::Linear { q: 1, p: -1 },
        ],
    );
    assert_eq!(
        signs(&shared),
        [Sign::Zero, Sign::Zero, Sign::Zero, Sign::Positive]
    );
    // The zero polynomial is zero at every root; a constant has its own sign at each, ∞ included,
    // where it is read at degree four: zero there.
    let zero = Quartic::new([Big::from_i64(0); 5]);
    assert_eq!(signs(&zero), [Sign::Zero; 4]);
    let constant = quartic(-7, &[]);
    assert_eq!(
        signs(&constant),
        [Sign::Negative, Sign::Negative, Sign::Negative, Sign::Zero]
    );
}

/// The sign of −1 / a less −1 / b on the projective line, ∞ last, for real roots a and b.
fn turned_order(a: Exact, b: Exact) -> Sign {
    let zero = Exact::Ratio { p: 0, q: 1 };
    let (za, zb) = (a.cmp(zero), b.cmp(zero));
    match (za, zb) {
        (Sign::Zero, Sign::Zero) => Sign::Zero,
        (Sign::Zero, _) => Sign::Positive,
        (_, Sign::Zero) => Sign::Negative,
        // −1/a + 1/b = (a − b) / (a b).
        _ => a.cmp(b).mul(za).mul(zb),
    }
}

#[test]
fn a_half_turn_keeps_every_root_and_orders_them_as_minus_one_over_t() {
    let mut rng = Rng::new();
    let cases = if cfg!(debug_assertions) { 100 } else { 1000 };
    let mut pairs = 0;
    for case in 0..cases {
        let qf = factors(&mut rng, true);
        let q = quartic(1, &qf);
        let turned = q.turned().unwrap();
        let want = distinct(&qf);
        let (roots, _) = found(&q);
        for (r, x) in roots.iter().zip(&want) {
            let t = r.turned();
            assert_eq!(
                sign_at_root(&turned, &turned, &t).unwrap(),
                Sign::Zero,
                "case {case}: {x:?} turned is a root"
            );
            for (s, y) in roots.iter().zip(&want) {
                assert_eq!(
                    compare(&turned, &t, &turned, &s.turned()).unwrap(),
                    turned_order(*x, *y),
                    "case {case}: {x:?} against {y:?}"
                );
                pairs += 1;
            }
        }
    }
    assert!(pairs > cases);
    // The exact roots: t(t − 1)(t + 1) has −1, 0, 1 and ∞; turned, 1, ∞, −1 and 0, each a root of
    // the turned quartic, t⁴ · c(−1 / t), which is t³ − t again.
    let q = quartic(
        1,
        &[
            Factor::Linear { q: 1, p: 0 },
            Factor::Linear { q: 1, p: 1 },
            Factor::Linear { q: 1, p: -1 },
        ],
    );
    let turned = q.turned().unwrap();
    let all: Vec<RealRoot> = q.roots().unwrap().iter().copied().collect();
    let pieces: Vec<Piece> = all.iter().map(|r| r.turned().piece()).collect();
    assert_eq!(
        pieces,
        [Piece::One, Piece::Infinity, Piece::MinusOne, Piece::Zero]
    );
    for r in &all {
        assert_eq!(
            sign_at_root(&turned, &turned, &r.turned()).unwrap(),
            Sign::Zero
        );
        assert_eq!(r.turned().multiplicity(), r.multiplicity());
    }
    // And on every open piece: roots below −1, between −1 and 0, between 0 and 1 and above 1.
    let open = quartic(
        1,
        &[
            Factor::Linear { q: 1, p: -3 },
            Factor::Linear { q: 2, p: -1 },
            Factor::Linear { q: 3, p: 1 },
            Factor::Linear { q: 1, p: 5 },
        ],
    );
    let pieces: Vec<Piece> = open
        .roots()
        .unwrap()
        .iter()
        .map(|r| r.turned().piece())
        .collect();
    assert_eq!(
        pieces,
        [Piece::Above, Piece::FarAbove, Piece::FarBelow, Piece::Below]
    );
}

/// A random whole number below 2^`bits` in magnitude, of either sign when `signed`.
fn random_big(rng: &mut Rng, bits: u32, signed: bool) -> IBig {
    let mut v = IBig::from(0);
    for _ in 0..bits.div_ceil(64) {
        v = (v << 64) + IBig::from(rng.next());
    }
    v >>= (bits.div_ceil(64) * 64 - bits) as usize;
    if signed && rng.next() % 2 == 1 { -v } else { v }
}

/// `v` at the second instance's width.
fn second(v: &IBig) -> Wide<SECOND_LIMBS> {
    let negative = *v < IBig::from(0);
    let mut magnitude = if negative { -v.clone() } else { v.clone() };
    let mut limbs = [0_u64; SECOND_LIMBS];
    let mask = (IBig::from(1) << 64) - IBig::from(1);
    for limb in &mut limbs {
        let low: IBig = magnitude.clone() & mask.clone();
        *limb = u64::try_from(low).unwrap();
        magnitude >>= 64;
    }
    assert_eq!(magnitude, IBig::from(0), "fits the second instance");
    let w = Wide::from_le_limbs(limbs);
    if negative {
        w.checked_neg().unwrap()
    } else {
        w
    }
}

/// The product of the factors q t − p, in the second instance, and its coefficients' widest bits.
fn second_quartic(roots: &[(IBig, IBig)]) -> (SecondQuartic, usize) {
    let mut c = vec![IBig::from(1)];
    for (q, p) in roots {
        let mut next = vec![IBig::from(0); c.len() + 1];
        for (i, a) in c.iter().enumerate() {
            next[i] -= a * p;
            next[i + 1] += a * q;
        }
        c = next;
    }
    let bits = |v: &IBig| {
        let mut m = if *v < IBig::from(0) {
            -v.clone()
        } else {
            v.clone()
        };
        let mut n = 0;
        while m > IBig::from(0) {
            m >>= 1;
            n += 1;
        }
        n
    };
    let widest = c.iter().map(bits).max().unwrap_or(0);
    let mut out = [Wide::from_i64(0); 5];
    for (slot, v) in out.iter_mut().zip(&c) {
        *slot = second(v);
    }
    (SecondQuartic::new(out), widest)
}

fn ratio(q: &IBig, p: &IBig) -> RBig {
    RBig::from(p.clone()) / RBig::from(q.clone())
}

fn order_of(a: &RBig, b: &RBig) -> Sign {
    match a.cmp(b) {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

#[test]
fn the_second_instance_reaches_roots_past_the_first_s_budget_exactly() {
    let mut rng = Rng::new();
    // Each case works its gcds bit by bit in the second instance's 729 limbs: a few seconds each
    // unoptimised, so few cases.
    let cases = if cfg!(debug_assertions) { 2 } else { 12 };
    let mut widest = 0;
    for case in 0..cases {
        // Four rational roots each, of numerators and denominators up to 160 bits: coefficients up
        // to about 650 bits, past the first instance's COEFFICIENT_BITS.
        let draw = |rng: &mut Rng| -> Vec<(IBig, IBig)> {
            (0..4)
                .map(|_| {
                    let q = random_big(rng, 160, false) + IBig::from(1);
                    (q, random_big(rng, 160, true))
                })
                .collect()
        };
        let (pr, qr) = (draw(&mut rng), draw(&mut rng));
        let ((p, wp), (q, wq)) = (second_quartic(&pr), second_quartic(&qr));
        widest = widest.max(wp).max(wq);
        assert!(wp <= SECOND_COEFFICIENT_BITS as usize && wq <= SECOND_COEFFICIENT_BITS as usize);
        let mut want_p: Vec<RBig> = pr.iter().map(|(q, p)| ratio(q, p)).collect();
        let mut want_q: Vec<RBig> = qr.iter().map(|(q, p)| ratio(q, p)).collect();
        want_p.sort();
        want_q.sort();
        let (got_p, got_q): (Vec<SecondRoot>, Vec<SecondRoot>) = (
            p.roots().unwrap().iter().copied().collect(),
            q.roots().unwrap().iter().copied().collect(),
        );
        assert_eq!((got_p.len(), got_q.len()), (4, 4), "case {case}");
        for (r, x) in got_p.iter().zip(&want_p) {
            for (s, y) in got_q.iter().zip(&want_q) {
                assert_eq!(
                    compare(&p, r, &q, s).unwrap(),
                    order_of(x, y),
                    "case {case}"
                );
            }
            // q at p's root: the product of its factors' signs there.
            let at = qr.iter().fold(Sign::Positive, |acc, (b, a)| {
                acc.mul(order_of(
                    &(RBig::from(b.clone()) * x),
                    &RBig::from(a.clone()),
                ))
            });
            assert_eq!(sign_at_root(&q, &p, r).unwrap(), at, "case {case}");
            assert_eq!(sign_at_root(&p, &p, r).unwrap(), Sign::Zero);
        }
    }
    assert!(
        widest > COEFFICIENT_BITS as usize,
        "past the first instance: {widest} bits"
    );
    // A first-instance quartic and its roots carry over whole: each root is the same root of the
    // same polynomial there.
    for _ in 0..20 {
        let qf = factors(&mut rng, true);
        let q = quartic(2, &qf);
        let wide = q.to_second().unwrap();
        let (mine, theirs): (Vec<RealRoot>, Vec<SecondRoot>) = (
            q.roots().unwrap().iter().copied().collect(),
            wide.roots().unwrap().iter().copied().collect(),
        );
        assert_eq!(mine.len(), theirs.len());
        for (i, r) in mine.iter().enumerate() {
            let carried = r.to_second().unwrap();
            assert_eq!(carried.piece(), r.piece());
            for (j, t) in theirs.iter().enumerate() {
                let want = Sign::of_i128(i as i128 - j as i128);
                assert_eq!(compare(&wide, &carried, &wide, t).unwrap(), want);
            }
        }
    }
    // 3 and 3 + 2^−400: coefficients of 402 bits, a hair apart, parted exactly.
    let k = IBig::from(1) << 400;
    let three = (k.clone(), IBig::from(3) * &k);
    let above = (k.clone(), IBig::from(3) * &k + IBig::from(1));
    let one = (IBig::from(1), IBig::from(1));
    let ((near, _), (far, _)) = (
        second_quartic(&[three, one.clone()]),
        second_quartic(&[above, one]),
    );
    let (a, b): (Vec<SecondRoot>, Vec<SecondRoot>) = (
        near.roots().unwrap().iter().copied().collect(),
        far.roots().unwrap().iter().copied().collect(),
    );
    assert_eq!(compare(&near, &a[1], &far, &b[1]).unwrap(), Sign::Negative);
    assert_eq!(compare(&near, &a[0], &far, &b[0]).unwrap(), Sign::Zero);
}
