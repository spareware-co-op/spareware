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
//!   four rational roots of up to 140-bit numerators and denominators, past the first instance's
//!   budget, give back their roots in order, compare exactly with each other's, sign each other at
//!   them, and part two roots 2^−400 apart, all against exact rational arithmetic; a first-instance
//!   quartic and its roots carry over whole (`to_second`).
//! - **Conic-root points** (`geom_num::conic_root`): where x²/36 + y²/9 = 1 meets x² + y² = 25, at
//!   (±8/√3, ±√(11/3)), each point lies exactly on both curves and on the right side of lines and
//!   line pairs; its x and y compare exactly with rational points, the convergents of 8/√3 and
//!   √(11/3) among them; its halves are the right ones; points of one vertical or horizontal chord
//!   compare equal through the mirror, and the ends of the range of x at t = 0 and t = ∞ compare
//!   right; on a rotated ellipse every comparison agrees with the enclosures where they tell, and
//!   turns over with its arguments.
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
    clippy::indexing_slicing,
    clippy::many_single_char_names
)]

use std::cmp::Ordering;

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_num::NumError;
use geom_num::Sign;
use geom_num::arc::Circle2;
use geom_num::carrier::Rational2;
use geom_num::conic::{Conic, Ellipse2, Meeting};
use geom_num::conic_root::{Axis, ConicRoot};
use geom_num::grid::P2;
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
    // Each case works its gcds bit by bit in the second instance's 316 limbs: a few seconds each
    // unoptimised, so few cases.
    let cases = if cfg!(debug_assertions) { 2 } else { 12 };
    let mut widest = 0;
    for case in 0..cases {
        // Four rational roots each, of numerators and denominators up to 140 bits: coefficients up
        // to about 565 bits, past the first instance's COEFFICIENT_BITS.
        let draw = |rng: &mut Rng| -> Vec<(IBig, IBig)> {
            (0..4)
                .map(|_| {
                    let q = random_big(rng, 140, false) + IBig::from(1);
                    (q, random_big(rng, 140, true))
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

// ------------------------------------------------------------------------------------------------
// Conic-root points (`geom_num::conic_root`).
// ------------------------------------------------------------------------------------------------

fn p2(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn circle(c: (i64, i64), radius: i128) -> Conic {
    Conic::of_circle(&Circle2::new(p2(c.0, c.1), radius).unwrap()).unwrap()
}

/// A conic about the origin from its six coefficients.
fn conic(c: [i64; 6]) -> Conic {
    Conic::new(p2(0, 0), c.map(Big::from_i64))
}

/// x²/36 + y²/9 = 1 and x² + y² = 25 meet at (±8/√3, ±√(11/3)): the four points, and for each the
/// signs of its x and y, read from its enclosure.
fn four() -> (Ellipse2, Vec<(ConicRoot, i128, i128)>) {
    let e = Ellipse2::new(p2(0, 0), [6, 0], 1, 2).unwrap();
    let c = circle((0, 0), 5);
    let points = (0..4)
        .map(|i| {
            let z = ConicRoot::of(&e, &c, i).unwrap();
            let [x, y] = z.enclosure();
            let sx = if x.lo() > 0.0 { 1 } else { -1 };
            let sy = if y.lo() > 0.0 { 1 } else { -1 };
            assert!(x.hi() * x.lo() > 0.0 && y.hi() * y.lo() > 0.0);
            (z, sx, sy)
        })
        .collect();
    (e, points)
}

/// The sign of s·√(n / 3) less the fraction a / b, b > 0: by squares when the signs agree.
fn surd_less(s: i128, n: i128, a: i128, b: i128) -> Sign {
    if a == 0 || a.signum() != s {
        return Sign::of_i128(s);
    }
    // Same sign: s(√(n/3) − |a|/b) has s's sign when n b² > 3 a².
    let by = Sign::of_i128((n * b * b - 3 * a * a).signum());
    if s > 0 { by } else { by.negate() }
}

#[test]
fn a_conic_root_lies_exactly_on_both_curves_and_on_the_right_side_of_others() {
    let (e, points) = four();
    let mut mixed = 0;
    for (_, sx, sy) in &points {
        mixed += sx * sy;
    }
    assert_eq!(mixed, 0, "two points in quadrants of each parity");
    for (z, sx, _) in &points {
        assert_eq!(z.ellipse(), e);
        assert_eq!(z.sign_of(&circle((0, 0), 5)).unwrap(), Sign::Zero);
        assert_eq!(
            z.sign_of(&Conic::of_ellipse(&e).unwrap()).unwrap(),
            Sign::Zero
        );
        // x² = 64/3 and y² = 11/3 exactly: 3x² − 64 and 3y² − 11 vanish, their neighbours do not.
        assert_eq!(z.sign_of(&conic([3, 0, 0, 0, 0, -64])).unwrap(), Sign::Zero);
        assert_eq!(
            z.sign_of(&conic([3, 0, 0, 0, 0, -63])).unwrap(),
            Sign::Positive
        );
        assert_eq!(z.sign_of(&conic([0, 0, 3, 0, 0, -11])).unwrap(), Sign::Zero);
        assert_eq!(
            z.sign_of(&conic([0, 0, 3, 0, 0, -12])).unwrap(),
            Sign::Negative
        );
        // x − 4 has the sign of x; x − 5 is negative at all four.
        assert_eq!(
            z.sign_of(&conic([0, 0, 0, 1, 0, -4])).unwrap(),
            Sign::of_i128(*sx)
        );
        assert_eq!(
            z.sign_of(&conic([0, 0, 0, 1, 0, -5])).unwrap(),
            Sign::Negative
        );
        // The quartic it is a root of.
        assert_eq!(
            sign_at_root(&z.quartic(), &z.quartic(), &z.root()).unwrap(),
            Sign::Zero
        );
    }
    // The ellipse itself, and a fifth root, are no conic roots.
    assert_eq!(
        ConicRoot::of(&e, &Conic::of_ellipse(&e).unwrap(), 0).err(),
        Some(NumError::Degenerate)
    );
    assert_eq!(
        ConicRoot::of(&e, &circle((0, 0), 5), 4).err(),
        Some(NumError::Degenerate)
    );
}

#[test]
fn a_conic_root_s_coordinates_compare_exactly_with_rational_points() {
    let (_, points) = four();
    let mut rng = Rng::new();
    // Convergents of 8/√3 = 4.6188... and √(11/3) = 1.9148..., so close that only the exact path
    // tells, then random fractions.
    let near_x = [
        (37_i128, 8_i128),
        (1_327, 287),
        (443_403, 96_000),
        (1_847_361_301, 399_964_000),
    ];
    let near_y = [(23_i128, 12_i128), (1_225, 640), (60_011_489, 31_340_000)];
    for (z, sx, sy) in &points {
        let ask = |a: i128, b: i128, axis: Axis, s: i128, n: i128| {
            for a in [a, -a] {
                let (x, y) = if axis == Axis::X { (a, 0) } else { (0, a) };
                let r = Rational2::new(x, y, b).unwrap();
                assert_eq!(
                    z.cmp_rational(&r, axis).unwrap(),
                    surd_less(s, n, a, b),
                    "{axis:?} against {a}/{b}"
                );
            }
        };
        for (a, b) in near_x {
            ask(a, b, Axis::X, *sx, 64);
        }
        for (a, b) in near_y {
            ask(a, b, Axis::Y, *sy, 11);
        }
        for _ in 0..200 {
            let b = rng.range(1, 1_000);
            ask(rng.range(-8_000, 8_000), b, Axis::X, *sx, 64);
            ask(rng.range(-4_000, 4_000), b, Axis::Y, *sy, 11);
        }
    }
}

#[test]
fn points_of_one_ellipse_compare_along_its_halves_and_across_its_chords() {
    let (_, points) = four();
    for (z, sx, sy) in &points {
        // Counter-clockwise, x falls above the axis and rises below it; y rises right of it.
        assert_eq!(z.half(Axis::X).unwrap(), Sign::of_i128(-sy));
        assert_eq!(z.half(Axis::Y).unwrap(), Sign::of_i128(*sx));
        for (w, tx, ty) in &points {
            // Points of one vertical chord have one x: the mirror carries one onto the other's half.
            assert_eq!(
                z.cmp_along(w, Axis::X).unwrap(),
                Some(Sign::of_i128(sx - tx)),
                "x of ({sx}, {sy}) and ({tx}, {ty})"
            );
            assert_eq!(
                z.cmp_along(w, Axis::Y).unwrap(),
                Some(Sign::of_i128(sy - ty)),
                "y of ({sx}, {sy}) and ({tx}, {ty})"
            );
        }
    }
    // The circle of radius 6 touches it at (6, 0) and (−6, 0): t = 0 and t = ∞, the ends of the
    // range of x, and the ends of one horizontal chord.
    let e = Ellipse2::new(p2(0, 0), [6, 0], 1, 2).unwrap();
    let touching = circle((0, 0), 6);
    let (right, left) = (
        ConicRoot::of(&e, &touching, 0).unwrap(),
        ConicRoot::of(&e, &touching, 1).unwrap(),
    );
    assert_eq!(right.root().piece(), Piece::Zero);
    assert_eq!(left.root().piece(), Piece::Infinity);
    assert_eq!(right.half(Axis::X).unwrap(), Sign::Zero);
    assert_eq!(left.half(Axis::X).unwrap(), Sign::Zero);
    assert_eq!(right.half(Axis::Y).unwrap(), Sign::Positive);
    assert_eq!(left.half(Axis::Y).unwrap(), Sign::Negative);
    assert_eq!(
        right.cmp_along(&left, Axis::X).unwrap(),
        Some(Sign::Positive)
    );
    assert_eq!(
        left.cmp_along(&right, Axis::X).unwrap(),
        Some(Sign::Negative)
    );
    assert_eq!(left.cmp_along(&left, Axis::X).unwrap(), Some(Sign::Zero));
    assert_eq!(right.cmp_along(&left, Axis::Y).unwrap(), Some(Sign::Zero));
    for (w, tx, _) in &points {
        assert_eq!(right.cmp_along(w, Axis::X).unwrap(), Some(Sign::Positive));
        assert_eq!(
            w.cmp_along(&left, Axis::X).unwrap(),
            Some(Sign::Positive),
            "{tx}"
        );
    }
    // A point of another ellipse is not this module's to compare.
    let other = Ellipse2::new(p2(0, 0), [0, 6], 1, 2).unwrap();
    let z = ConicRoot::of(&other, &circle((0, 0), 5), 0).unwrap();
    assert_eq!(right.cmp_along(&z, Axis::X).unwrap(), None);
}

#[test]
fn points_of_a_rotated_ellipse_agree_with_their_enclosures_and_each_other() {
    let mut rng = Rng::new();
    let cases = if cfg!(debug_assertions) { 6 } else { 40 };
    let mut decided = 0;
    for case in 0..cases {
        let e = Ellipse2::new(p2(1, -2), [5, 2], 1, 3).unwrap();
        let centre = (rng.range(-4, 6) as i64, rng.range(-5, 2) as i64);
        let c = circle(centre, rng.range(2, 6));
        let Ok(Meeting::At { roots, .. }) = e.meeting(&c) else {
            continue;
        };
        let points: Vec<ConicRoot> = (0..roots.count())
            .map(|i| ConicRoot::of(&e, &c, i).unwrap())
            .collect();
        for z in &points {
            for w in &points {
                for (axis, k) in [(Axis::X, 0), (Axis::Y, 1)] {
                    let s = z.cmp_along(w, axis).unwrap().unwrap();
                    assert_eq!(
                        w.cmp_along(z, axis).unwrap().unwrap(),
                        s.negate(),
                        "case {case}"
                    );
                    let gap = z.enclosure()[k].sub(w.enclosure()[k]).unwrap();
                    if let Some(sign @ (Sign::Positive | Sign::Negative)) = gap.certain_sign() {
                        assert_eq!(s, sign, "case {case}: {axis:?}");
                        decided += 1;
                    }
                }
            }
        }
    }
    assert!(decided > cases);
}

#[test]
fn conic_roots_at_the_ends_of_the_line_and_at_random_agree_with_their_enclosures() {
    // The circle of radius 6 touches x²/36 + y²/9 = 1 at t = 0 and t = ∞: (6, 0) and (−6, 0).
    let e = Ellipse2::new(p2(0, 0), [6, 0], 1, 2).unwrap();
    let touching = circle((0, 0), 6);
    for (i, x) in [(0, 6), (1, -6)] {
        let z = ConicRoot::of(&e, &touching, i).unwrap();
        assert_eq!(z.sign_of(&conic([0, 0, 0, 1, 0, -x])).unwrap(), Sign::Zero);
        assert_eq!(z.sign_of(&conic([0, 0, 1, 0, 0, 0])).unwrap(), Sign::Zero);
        let at = Rational2::new(i128::from(x), 0, 1).unwrap();
        assert_eq!(z.cmp_rational(&at, Axis::X).unwrap(), Sign::Zero);
        assert_eq!(z.cmp_rational(&at, Axis::Y).unwrap(), Sign::Zero);
        assert_eq!(z.half(Axis::X).unwrap(), Sign::Zero);
    }
    // At random: lines through two grid points and rational points against the points where a
    // rotated ellipse meets random circles, wherever the enclosure tells.
    let mut rng = Rng::new();
    let rotated = Ellipse2::new(p2(1, -2), [5, 2], 1, 3).unwrap();
    let mut told = 0;
    for _ in 0..if cfg!(debug_assertions) { 10 } else { 60 } {
        let c = circle(
            (rng.range(-4, 6) as i64, rng.range(-5, 2) as i64),
            rng.range(2, 6),
        );
        let Ok(Meeting::At { roots, .. }) = rotated.meeting(&c) else {
            continue;
        };
        for i in 0..roots.count() {
            let z = ConicRoot::of(&rotated, &c, i).unwrap();
            let [ex, ey] = z.enclosure();
            let (a, b, k) = (rng.range(-5, 5), rng.range(-5, 5), rng.range(-30, 30));
            let line = conic([0, 0, 0, a as i64, b as i64, k as i64]);
            let value = ex
                .mul(enclose(a))
                .unwrap()
                .add(ey.mul(enclose(b)).unwrap())
                .unwrap()
                .add(enclose(k))
                .unwrap();
            if let Some(sign @ (Sign::Positive | Sign::Negative)) = value.certain_sign() {
                assert_eq!(z.sign_of(&line).unwrap(), sign);
                told += 1;
            }
            let (x, w) = (rng.range(-40, 40), rng.range(1, 7));
            let r = Rational2::new(x, x, w).unwrap();
            for (axis, e) in [(Axis::X, ex), (Axis::Y, ey)] {
                let gap = e.sub(enclose(x).div(enclose(w)).unwrap()).unwrap();
                if let Some(sign @ (Sign::Positive | Sign::Negative)) = gap.certain_sign() {
                    assert_eq!(z.cmp_rational(&r, axis).unwrap(), sign);
                }
            }
            // y rises counter-clockwise where the form's x-derivative 2A u + B v is positive.
            let [fa, fb, _] = rotated.form().unwrap();
            let (u, v) = (ex.sub(enclose(1)).unwrap(), ey.sub(enclose(-2)).unwrap());
            let slope = geom_num::one_root::enclose(&fa)
                .unwrap()
                .mul(enclose(2))
                .unwrap()
                .mul(u)
                .unwrap()
                .add(geom_num::one_root::enclose(&fb).unwrap().mul(v).unwrap())
                .unwrap();
            if let Some(sign @ (Sign::Positive | Sign::Negative)) = slope.certain_sign() {
                assert_eq!(z.half(Axis::Y).unwrap(), sign);
            }
        }
    }
    assert!(told > 10);
}

/// A small whole number as a point interval, exact.
fn enclose(v: i128) -> geom_num::interval::Interval {
    geom_num::one_root::enclose(&Big::from_i128(v)).unwrap()
}
