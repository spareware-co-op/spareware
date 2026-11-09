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

use geom_num::Sign;
use geom_num::quartic::{Big, Piece, Quartic, RealRoot, sign_at_root};

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
