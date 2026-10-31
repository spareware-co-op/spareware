//! Attacks on piece G190: exact conics (`geom_num::quartic`, `geom_num::conic` and the ellipse
//! crossings of `geom_num::one_root`; chapter 14.2 of the kernel architecture). Every test tries to
//! break a claim of the piece's done-when or of the modules' guarantees.
//!
//! What decides what is right is never the code under test:
//!
//! - **Quartics** are built from their roots, so the answer is known before they are multiplied out.
//! - **Ellipses** are checked by `dashu` integers from the equation p²(u · M)² + q²(u · M⊥)² = p²‖M‖⁴,
//!   not the form the module derives from it, and each root's place by the sign of the other curve's
//!   equation at the ellipse's own points on either side of it.
//! - **Contacts** come from geometry worked by hand: tangent circles, the circle of curvature, an
//!   inscribed ellipse, a point on the ellipse by its Pythagorean parameter.
//!
//! CADCraft's issue 1690, a hatch boundary lost to the gaps where lines meet a flattened ellipse, is
//! `a_line_ending_on_an_ellipse_closes_its_boundary`.
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17). Case counts are a twentieth in debug
//! builds.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "p, q, k and the like are the formulas'"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::cmp::Ordering;

use dashu_int::IBig;
use geom_num::arc::{Arc2, Circle2, Contact, RADIUS_BITS, Round};
use geom_num::carrier::{Carrier, NORMAL_BITS, OFFSET_BITS, POINT_BITS, Rational2, WEIGHT_BITS};
use geom_num::conic::{Conic, Ellipse2, Meeting, RATIO_BITS};
use geom_num::generated::{ellipse_line, ellipse_side};
use geom_num::grid::{COORD_MAX, P2};
use geom_num::one_root::{BIG_LIMBS, Crossing2, Root};
use geom_num::quartic::{Big, Piece, Quartic, RealRoot, compare};
use geom_num::wide::Wide;
use geom_num::{NumError, Sign};

/// The largest magnitudes each budget allows.
const N: i128 = (1 << NORMAL_BITS) - 1;
const D: i128 = (1 << OFFSET_BITS) - 1;
const P: i128 = (1 << POINT_BITS) - 1;
const W: i128 = (1 << WEIGHT_BITS) - 1;
const M: i64 = COORD_MAX;
const TOP: i64 = (1 << RADIUS_BITS) - 1;

/// The default seed of every randomised test here (G190).
const DEFAULT_SEED: u64 = 0x6719_0000_5eed_0190;

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), written out as the other predicate tests do.
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
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A signed integer of at most `bits` bits, its size drawn too, so every scale is reached.
    fn signed(&mut self, bits: u32) -> i128 {
        let size = 1 + (self.next() % u64::from(bits)) as u32;
        let raw = (u128::from(self.next()) << 64) | u128::from(self.next());
        let magnitude = (raw >> (128 - size)) as i128;
        if self.next() & 1 == 0 {
            magnitude
        } else {
            -magnitude
        }
    }

    fn grid(&mut self) -> P2 {
        let mut c = || (self.signed(40) as i64).clamp(-M, M);
        P2::new(c(), c()).unwrap()
    }

    fn ellipse(&mut self) -> Ellipse2 {
        loop {
            let centre = self.grid();
            let major = [self.signed(RADIUS_BITS) as i64, self.signed(RADIUS_BITS) as i64];
            let q = 1 + (self.signed(RATIO_BITS).unsigned_abs() as i64);
            let p = 1 + (self.signed(RATIO_BITS).unsigned_abs() as i64) % q;
            if let Ok(e) = Ellipse2::new(centre, major, p, q) {
                return e;
            }
        }
    }

    fn point(&mut self) -> Rational2 {
        loop {
            let (x, y, w) = (
                self.signed(POINT_BITS),
                self.signed(POINT_BITS),
                self.signed(WEIGHT_BITS),
            );
            if let Ok(p) = Rational2::new(x, y, w) {
                return p;
            }
        }
    }

    fn carrier(&mut self) -> Carrier {
        loop {
            let (a, b, c) = (
                self.signed(NORMAL_BITS),
                self.signed(NORMAL_BITS),
                self.signed(OFFSET_BITS),
            );
            if let Ok(carrier) = Carrier::new(a, b, c) {
                return carrier;
            }
        }
    }

    /// A carrier through the ellipse's centre and a grid point, which always meets it.
    fn carrier_across(&mut self, e: &Ellipse2) -> Carrier {
        loop {
            if let Ok(l) = Carrier::through(e.centre(), self.grid()) {
                return l;
            }
        }
    }
}

fn cases(full: u64) -> u64 {
    if cfg!(debug_assertions) {
        full / 20
    } else {
        full
    }
}

fn p2(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn ellipse(c: (i64, i64), major: [i64; 2], p: i64, q: i64) -> Ellipse2 {
    Ellipse2::new(p2(c.0, c.1), major, p, q).unwrap()
}

fn circle(c: (i64, i64), radius: i128) -> Conic {
    Conic::of_circle(&Circle2::new(p2(c.0, c.1), radius).unwrap()).unwrap()
}

fn big<const L: usize>(v: &Wide<L>) -> IBig {
    let limbs = v.to_le_limbs();
    let negative = v.is_negative();
    let mut out = IBig::from(0);
    for (i, limb) in limbs.iter().enumerate() {
        out += IBig::from(*limb) << (64 * i);
    }
    if negative {
        out - (IBig::from(1) << (64 * L))
    } else {
        out
    }
}

fn sign_of(v: &IBig) -> Sign {
    match v.cmp(&IBig::from(0)) {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

// ------------------------------------------------------------------------------------------------
// Quartics by their roots.
// ------------------------------------------------------------------------------------------------

/// `c` times the polynomial `by`, lowest term first.
fn multiply(c: [Big; 5], by: [i128; 3]) -> [Big; 5] {
    let mut out = [Big::from_i64(0); 5];
    for (i, v) in c.iter().enumerate() {
        for (j, w) in by.iter().enumerate() {
            let term = v.checked_mul(&Big::from_i128(*w)).unwrap();
            if i + j < 5 {
                out[i + j] = out[i + j].checked_add(&term).unwrap();
            } else {
                assert!(term.is_zero(), "past degree four");
            }
        }
    }
    out
}

/// The polynomial with each (q, p) of `linear` as a factor q t − p, a root p / q, and each of
/// `quadratics` as a factor, lowest term first. (0, −1) is the factor 1: a root at infinity when the
/// polynomial is read at degree four.
fn quartic(linear: &[(i128, i128)], quadratics: &[[i128; 3]]) -> Quartic {
    let mut c = [Big::from_i64(0); 5];
    c[0] = Big::from_i64(1);
    for (q, p) in linear {
        c = multiply(c, [-p, *q, 0]);
    }
    for f in quadratics {
        c = multiply(c, *f);
    }
    Quartic::new(c)
}

fn found(q: &Quartic) -> Vec<(Piece, u8)> {
    q.roots()
        .unwrap()
        .iter()
        .map(|r| (r.piece(), r.multiplicity()))
        .collect()
}

/// The only finite root of the quartic with this one rational root, and that quartic.
fn single(q: i128, p: i128) -> (Quartic, RealRoot) {
    let quartic = quartic(&[(q, p), (0, -1), (0, -1), (0, -1)], &[]);
    let root = *quartic.roots().unwrap().iter().next().unwrap();
    (quartic, root)
}

#[test]
fn every_stretch_of_the_line_and_every_cut_is_found() {
    use Piece::*;
    let between = quartic(&[(1, -3), (2, -1), (2, 1), (1, 3)], &[]);
    assert_eq!(
        found(&between),
        [(FarBelow, 1), (Below, 1), (Above, 1), (FarAbove, 1)]
    );
    let on = quartic(&[(1, 1), (1, 0), (1, -1), (0, -1)], &[]);
    assert_eq!(
        found(&on),
        [(MinusOne, 1), (Zero, 1), (One, 1), (Infinity, 1)]
    );
}

#[test]
fn every_multiplicity_is_found() {
    use Piece::*;
    let (a, b) = ((3, 1), (5, 4));
    assert_eq!(found(&quartic(&[a, a, b, b], &[])), [(Above, 2), (Above, 2)]);
    assert_eq!(found(&quartic(&[a, a, a, b], &[])), [(Above, 3), (Above, 1)]);
    assert_eq!(found(&quartic(&[a, a, a, a], &[])), [(Above, 4)]);
    assert_eq!(
        found(&quartic(&[a, a, (1, -2), (3, 7)], &[])),
        [(FarBelow, 1), (Above, 2), (FarAbove, 1)]
    );
    assert_eq!(found(&quartic(&[(1, 0); 4], &[])), [(Zero, 4)]);
    assert_eq!(found(&quartic(&[(0, -1); 4], &[])), [(Infinity, 4)]);
    assert_eq!(
        found(&quartic(&[(1, 1), (1, 1), (0, -1), (0, -1)], &[])),
        [(One, 2), (Infinity, 2)]
    );
    assert_eq!(
        found(&quartic(&[(1, -1), (1, -1), (1, -1), (0, -1)], &[])),
        [(MinusOne, 3), (Infinity, 1)]
    );
    assert_eq!(
        found(&quartic(&[(1, 2), (0, -1), (0, -1), (0, -1)], &[])),
        [(FarAbove, 1), (Infinity, 3)]
    );
}

#[test]
fn complex_roots_are_no_roots() {
    use Piece::*;
    let (i, w) = ([1, 0, 1], [1, 1, 1]);
    assert!(found(&quartic(&[], &[i, w])).is_empty());
    assert!(found(&quartic(&[], &[i, i])).is_empty());
    assert_eq!(found(&quartic(&[(1, 3), (1, 3)], &[i])), [(FarAbove, 2)]);
    // A pair a hair from a double root of 1/3: 2^120 (3t − 1)² + 1, and its neighbour with a real
    // pair a hair apart: the two that a sign alone cannot tell, 2^−60 from each other.
    let k = 1_i128 << 120;
    let apart = [k + 1, -6 * k, 9 * k];
    assert!(found(&quartic(&[], &[apart, i])).is_empty());
    let meeting = [k - 1, -6 * k, 9 * k];
    assert_eq!(
        found(&quartic(&[], &[meeting, i])),
        [(Above, 1), (Above, 1)]
    );
}

#[test]
fn roots_a_hair_apart_stay_apart() {
    use Piece::*;
    // 1/3 and 1/3 + 2^−70, past the depth where the fast path gives up, beside a double root.
    let s = 3_i128 << 70;
    let q = quartic(&[(s, s / 3), (s, s / 3 + 3), (1, 2), (1, 2)], &[]);
    assert_eq!(
        found(&q),
        [(Above, 1), (Above, 1), (FarAbove, 2)]
    );
    let roots: Vec<RealRoot> = q.roots().unwrap().iter().copied().collect();
    assert_eq!(compare(&q, &roots[0], &q, &roots[1]).unwrap(), Sign::Negative);
    let (third, at) = single(3, 1);
    assert_eq!(compare(&third, &at, &q, &roots[0]).unwrap(), Sign::Zero);
    assert_eq!(compare(&third, &at, &q, &roots[1]).unwrap(), Sign::Negative);
}

/// Its place on the line, as `Piece` orders it.
fn piece_of(q: i128, p: i128) -> Piece {
    if q == 0 {
        return Piece::Infinity;
    }
    if p < -q {
        Piece::FarBelow
    } else if p == -q {
        Piece::MinusOne
    } else if p < 0 {
        Piece::Below
    } else if p == 0 {
        Piece::Zero
    } else if p < q {
        Piece::Above
    } else if p == q {
        Piece::One
    } else {
        Piece::FarAbove
    }
}

#[test]
fn random_quartics_give_back_their_roots() {
    const PATTERNS: [&[u8]; 7] = [
        &[1, 1, 1, 1],
        &[2, 1, 1],
        &[2, 2],
        &[3, 1],
        &[4],
        &[1, 1],
        &[2],
    ];
    let mut rng = Rng::new();
    for _ in 0..cases(1_500) {
        let pattern = PATTERNS[(rng.next() % PATTERNS.len() as u64) as usize];
        // Distinct roots p / q, q > 0, of any size to 58 bits, or infinity now and then.
        let mut roots: Vec<(i128, i128, u8)> = Vec::new();
        for &multiplicity in pattern {
            loop {
                let (q, p) = if rng.next() % 16 == 0 {
                    (0, -1)
                } else {
                    (1 + rng.signed(58).abs(), rng.signed(58))
                };
                let same = roots
                    .iter()
                    .any(|&(a, b, _)| a * p == b * q || (a == 0 && q == 0));
                if !same {
                    roots.push((q, p, multiplicity));
                    break;
                }
            }
        }
        let degree: u8 = pattern.iter().copied().sum();
        let mut factors: Vec<(i128, i128)> = Vec::new();
        for &(q, p, m) in &roots {
            for _ in 0..m {
                factors.push((q, p));
            }
        }
        let quadratics: Vec<[i128; 3]> = if degree == 2 {
            vec![[
                1 + rng.signed(50).abs(),
                rng.signed(20),
                1 + rng.signed(50).abs(),
            ]]
            .into_iter()
            .filter(|[c, b, a]| b * b < 4 * a * c)
            .collect()
        } else {
            Vec::new()
        };
        let q = quartic(&factors, &quadratics);
        if degree == 2 && quadratics.is_empty() {
            continue;
        }
        // Increasing on the projective line: infinity last, the rest by cross-multiplication.
        roots.sort_by(|a, b| match (a.0, b.0) {
            (0, 0) => Ordering::Equal,
            (0, _) => Ordering::Greater,
            (_, 0) => Ordering::Less,
            _ => (a.1 * b.0).cmp(&(b.1 * a.0)),
        });
        let expected: Vec<(Piece, u8)> = roots
            .iter()
            .map(|&(a, b, m)| (piece_of(a, b), m))
            .collect();
        assert_eq!(found(&q), expected, "{roots:?}");
        // Each against each, by rational order.
        let list: Vec<RealRoot> = q.roots().unwrap().iter().copied().collect();
        for (i, r) in list.iter().enumerate() {
            let (a, b, _) = roots[i];
            if a == 0 {
                continue;
            }
            let (one, at) = single(a, b);
            assert_eq!(compare(&one, &at, &q, r).unwrap(), Sign::Zero, "{roots:?}");
            for (j, other) in list.iter().enumerate() {
                let order = match i.cmp(&j) {
                    Ordering::Less => Sign::Negative,
                    Ordering::Equal => Sign::Zero,
                    Ordering::Greater => Sign::Positive,
                };
                assert_eq!(compare(&one, &at, &q, other).unwrap(), order, "{roots:?}");
            }
        }
    }
}

#[test]
fn roots_of_two_quartics_compare_exactly() {
    // −5, −√2, 1/3, √2; and −√2, 1/7, 1/3, √2.
    let two = [-2, 0, 1];
    let p = quartic(&[(3, 1), (1, -5)], &[two]);
    let q = quartic(&[(3, 1), (7, 1)], &[two]);
    let (a, b): (Vec<RealRoot>, Vec<RealRoot>) = (
        p.roots().unwrap().iter().copied().collect(),
        q.roots().unwrap().iter().copied().collect(),
    );
    assert_eq!((a.len(), b.len()), (4, 4));
    for (i, j, sign) in [
        (0, 0, Sign::Negative),
        (1, 0, Sign::Zero),
        (2, 2, Sign::Zero),
        (3, 3, Sign::Zero),
        (2, 1, Sign::Positive),
        (1, 1, Sign::Negative),
        (3, 2, Sign::Positive),
    ] {
        assert_eq!(compare(&p, &a[i], &q, &b[j]).unwrap(), sign, "{i} {j}");
        assert_eq!(
            compare(&q, &b[j], &p, &a[i]).unwrap(),
            sign.negate(),
            "{i} {j}"
        );
    }
    // √2 and √(2 + 2^−80): no root in common, a hair apart.
    let k = 1_i128 << 80;
    let (near, far) = (quartic(&[], &[two, [1, 0, 1]]), quartic(&[], &[[-(2 * k + 1), 0, k], [1, 0, 1]]));
    let (lo, hi): (Vec<RealRoot>, Vec<RealRoot>) = (
        near.roots().unwrap().iter().copied().collect(),
        far.roots().unwrap().iter().copied().collect(),
    );
    assert_eq!(compare(&near, &lo[1], &far, &hi[1]).unwrap(), Sign::Negative);
    assert_eq!(compare(&near, &lo[0], &far, &hi[0]).unwrap(), Sign::Positive);
}

#[test]
fn the_zero_polynomial_is_refused_and_a_budget_is_never_an_answer() {
    let zero = Quartic::new([Big::from_i64(0); 5]);
    assert!(zero.is_zero());
    assert_eq!(zero.roots().err(), Some(NumError::Degenerate));
    // Coefficients far past the budget: the right roots, or an error, never others.
    let mut huge = Big::from_i64(1);
    for _ in 0..3_500 {
        huge = huge.checked_add(&huge).unwrap();
    }
    let c = [24, -50, 35, -10, 1].map(|v| Big::from_i64(v).checked_mul(&huge).unwrap());
    if let Ok(roots) = Quartic::new(c).roots() {
        assert_eq!(roots.count(), 4);
    } else {
        assert_eq!(
            Quartic::new(c).roots().err(),
            Some(NumError::BudgetExceeded)
        );
    }
}

// ------------------------------------------------------------------------------------------------
// The oracle for ellipses, from the equation.
// ------------------------------------------------------------------------------------------------

/// The parts of an ellipse, as whole numbers.
struct Whole {
    c: [IBig; 2],
    m: [IBig; 2],
    p: IBig,
    q: IBig,
}

fn whole(e: &Ellipse2) -> Whole {
    let [mx, my] = e.major();
    let (p, q) = e.ratio();
    Whole {
        c: [IBig::from(e.centre().x.get()), IBig::from(e.centre().y.get())],
        m: [IBig::from(mx), IBig::from(my)],
        p: IBig::from(p),
        q: IBig::from(q),
    }
}

impl Whole {
    /// (u · M, u · M⊥).
    fn axes(&self, ux: &IBig, uy: &IBig) -> (IBig, IBig) {
        let [mx, my] = &self.m;
        (ux * mx + uy * my, uy * mx - ux * my)
    }

    /// ‖M‖².
    fn length(&self) -> IBig {
        let [mx, my] = &self.m;
        mx * mx + my * my
    }

    /// The ellipse's equation at (x, y) / w, times w²: negative inside.
    fn value(&self, x: &IBig, y: &IBig, w: &IBig) -> IBig {
        let (ux, uy) = (x - w * &self.c[0], y - w * &self.c[1]);
        let (along, across) = self.axes(&ux, &uy);
        let level = &self.p * self.length() * w;
        &self.p * &self.p * &along * &along + &self.q * &self.q * &across * &across
            - &level * &level
    }

    /// Its point at t = n / d: numerators over w.
    fn at(&self, n: &IBig, d: &IBig) -> [IBig; 3] {
        let (cosine, sine, w) = (d * d - n * n, IBig::from(2) * n * d, d * d + n * n);
        let [mx, my] = &self.m;
        let across = |c: &IBig| c * &self.q * &w;
        [
            &self.q * &cosine * mx - &self.p * &sine * my + across(&self.c[0]),
            &self.q * &cosine * my + &self.p * &sine * mx + across(&self.c[1]),
            &self.q * &w,
        ]
    }
}

fn side_oracle(e: &Ellipse2, z: &Rational2) -> Sign {
    let ([x, y], w) = z.coordinates();
    sign_of(&whole(e).value(&IBig::from(x), &IBig::from(y), &IBig::from(w)))
}

/// The discriminant of where a carrier meets an ellipse, along the carrier from its point nearest
/// the centre.
fn meets_oracle(e: &Ellipse2, l: &Carrier) -> Contact {
    let ([a, b], c) = l.coefficients();
    let (a, b, c) = (IBig::from(a), IBig::from(b), IBig::from(c));
    let w = whole(e);
    let n = &a * &a + &b * &b;
    let h = &c - &a * &w.c[0] - &b * &w.c[1];
    let (p2, q2) = (&w.p * &w.p, &w.q * &w.q);
    let (da, dc) = w.axes(&b, &(-&a));
    let (ua, uc) = w.axes(&(&h * &a), &(&h * &b));
    let shape_d = &p2 * &da * &da + &q2 * &dc * &dc;
    let bilinear = &p2 * &ua * &da + &q2 * &uc * &dc;
    let shape_u = &p2 * &ua * &ua + &q2 * &uc * &uc;
    let level = &w.p * w.length() * &n;
    let discriminant = &bilinear * &bilinear - shape_d * (shape_u - &level * &level);
    match sign_of(&discriminant) {
        Sign::Positive => Contact::Crossing,
        Sign::Zero => Contact::Tangent,
        Sign::Negative => Contact::Apart,
    }
}

/// ⌊√n⌋ by Newton's method.
fn isqrt(n: &IBig) -> IBig {
    if *n < IBig::from(2) {
        return n.clone();
    }
    let mut x = IBig::from(1) << (n.to_string().len() * 2);
    loop {
        let y = (&x + n / &x) >> 1;
        if y >= x {
            return x;
        }
        x = y;
    }
}

/// The sign of a + b√k, by brackets ever finer; `None` within 2^−4096 of zero.
fn root_oracle(a: &IBig, b: &IBig, k: &IBig) -> Option<Sign> {
    for p in [64_usize, 256, 1024, 4096] {
        let scale = IBig::from(1) << p;
        let r = isqrt(&(k * &scale * &scale));
        let (one, other) = (b * &r, b * (&r + IBig::from(1)));
        let (low, high) = if one <= other {
            (one, other)
        } else {
            (other, one)
        };
        let base = a * &scale;
        if &base + &low > IBig::from(0) {
            return Some(Sign::Positive);
        }
        if &base + &high < IBig::from(0) {
            return Some(Sign::Negative);
        }
    }
    None
}

/// Where a crossing lies against an ellipse, from the equation with (A + B√k) / W for the point.
fn against_oracle(z: &Crossing2, e: &Ellipse2) -> Option<Sign> {
    let ([xa, ya], [xb, yb], k, v) = z.parts();
    let (xa, ya, xb, yb, k, v) = (big(&xa), big(&ya), big(&xb), big(&yb), big(&k), big(&v));
    let w = whole(e);
    let (ux, uy) = (&xa - &v * &w.c[0], &ya - &v * &w.c[1]);
    let (along, across) = w.axes(&ux, &uy);
    let (along_root, across_root) = w.axes(&xb, &yb);
    let (p2, q2) = (&w.p * &w.p, &w.q * &w.q);
    let level = &w.p * w.length() * &v;
    let plain = &p2 * (&along * &along + &along_root * &along_root * &k)
        + &q2 * (&across * &across + &across_root * &across_root * &k)
        - &level * &level;
    let root = IBig::from(2) * (&p2 * &along * &along_root + &q2 * &across * &across_root);
    root_oracle(&plain, &root, &k)
}

// ------------------------------------------------------------------------------------------------
// Ellipses, by hand.
// ------------------------------------------------------------------------------------------------

fn contacts(e: &Ellipse2, other: &Conic) -> Vec<(Piece, u8)> {
    match e.meeting(other).unwrap() {
        Meeting::Same => panic!("one curve"),
        Meeting::At { roots, .. } => roots
            .iter()
            .map(|r| (r.piece(), r.multiplicity()))
            .collect(),
    }
}

#[test]
fn tangent_circles_touch_at_one_double_root() {
    use Piece::*;
    let round = Ellipse2::of_circle(&Circle2::new(p2(0, 0), 5).unwrap()).unwrap();
    assert_eq!(contacts(&round, &circle((8, 0), 3)), [(Zero, 2)]);
    assert_eq!(contacts(&round, &circle((3, 0), 2)), [(Zero, 2)]);
    assert_eq!(contacts(&round, &circle((-8, 0), 3)), [(Infinity, 2)]);
    assert_eq!(contacts(&round, &circle((0, 8), 3)), [(One, 2)]);
    assert_eq!(contacts(&round, &circle((0, -8), 3)), [(MinusOne, 2)]);
    assert!(contacts(&round, &circle((9, 0), 1)).is_empty());
    assert!(contacts(&round, &circle((0, 0), 1)).is_empty());
    // A step from tangent either way: two crossings, or none.
    assert_eq!(
        contacts(&round, &circle((8, 0), 4))
            .iter()
            .map(|c| c.1)
            .collect::<Vec<u8>>(),
        [1, 1]
    );
    assert!(contacts(&round, &circle((9, 0), 3)).is_empty());
}

#[test]
fn an_inscribed_ellipse_touches_at_both_ends() {
    use Piece::*;
    let flat = ellipse((0, 0), [5, 0], 3, 5);
    assert_eq!(
        contacts(&flat, &circle((0, 0), 5)),
        [(Zero, 2), (Infinity, 2)]
    );
    assert_eq!(
        contacts(&flat, &circle((0, 0), 3)),
        [(MinusOne, 2), (One, 2)]
    );
    assert_eq!(contacts(&flat, &circle((0, 0), 4)).len(), 4);
    // Turned, and away from the origin.
    let turned = ellipse((7, -3), [3, 4], 3, 5);
    assert_eq!(
        contacts(&turned, &circle((7, -3), 5)),
        [(Zero, 2), (Infinity, 2)]
    );
}

#[test]
fn the_circle_of_curvature_has_contact_of_order_four_or_three() {
    use Piece::*;
    // At the vertex (5, 0) of x²/25 + y²/9 = 1: radius 9/5, centre (16/5, 0).
    let flat = ellipse((0, 0), [5, 0], 3, 5);
    let centre = Rational2::new(16, 0, 5).unwrap();
    let vertex = Conic::of_round(&Round::new(p2(5, 0), centre)).unwrap();
    assert_eq!(contacts(&flat, &vertex), [(Zero, 4)]);
    // At (15, 12) on x²/625 + y²/225 = 1, t = 1/2, on the evolute's point (1296, −5120) / 375.
    let wide = ellipse((0, 0), [25, 0], 3, 5);
    let centre = Rational2::new(1296, -5120, 375).unwrap();
    let osculating = Conic::of_round(&Round::new(p2(15, 12), centre)).unwrap();
    let list = contacts(&wide, &osculating);
    assert_eq!(list.len(), 2, "{list:?}");
    assert!(list.contains(&(Above, 3)), "{list:?}");
    let (half, at) = single(2, 1);
    let Meeting::At { quartic, roots } = wide.meeting(&osculating).unwrap() else {
        panic!("one curve");
    };
    let triple = roots.iter().find(|r| r.multiplicity() == 3).unwrap();
    assert_eq!(compare(&quartic, triple, &half, &at).unwrap(), Sign::Zero);
}

#[test]
fn one_curve_twice_is_the_same() {
    let e = ellipse((7, -3), [15, 20], 3, 5);
    let reversed = ellipse((7, -3), [-15, -20], 3, 5);
    let is_same = |e: &Ellipse2, c: &Conic| matches!(e.meeting(c).unwrap(), Meeting::Same);
    assert!(is_same(&e, &Conic::of_ellipse(&e).unwrap()));
    assert!(is_same(&e, &Conic::of_ellipse(&reversed).unwrap()));
    // A circle, along any Pythagorean major axis, as a circle and as a round.
    let c = Circle2::new(p2(1, 2), 25).unwrap();
    let along = ellipse((1, 2), [15, 20], 1, 1);
    assert!(is_same(&along, &Conic::of_circle(&c).unwrap()));
    assert!(is_same(&Ellipse2::of_circle(&c).unwrap(), &Conic::of_ellipse(&along).unwrap()));
    let round = Round::new(p2(26, 2), p2(1, 2).into());
    assert!(is_same(&along, &Conic::of_round(&round).unwrap()));
}

#[test]
fn a_crossing_at_the_far_end_is_not_lost() {
    // The circle about (−2, 4) of radius 5 passes through (−5, 0), the point t = ∞, across the
    // ellipse, whose substitution loses its leading term there.
    let e = ellipse((0, 0), [5, 0], 3, 5);
    let list = contacts(&e, &circle((-2, 4), 5));
    assert_eq!(list.last(), Some(&(Piece::Infinity, 1)), "{list:?}");
    assert_eq!(list.iter().map(|c| c.1).sum::<u8>() % 2, 0, "{list:?}");
}

#[test]
fn four_crossings_come_in_order_along_each_curve() {
    use Piece::*;
    let (a, b) = (
        ellipse((0, 0), [5, 0], 3, 5),
        ellipse((0, 0), [0, 5], 3, 5),
    );
    let order = [(FarBelow, 1), (Below, 1), (Above, 1), (FarAbove, 1)];
    assert_eq!(contacts(&a, &Conic::of_ellipse(&b).unwrap()), order);
    assert_eq!(contacts(&b, &Conic::of_ellipse(&a).unwrap()), order);
}

#[test]
fn a_line_and_a_circle_take_their_places_along_an_ellipse() {
    use Piece::*;
    let e = ellipse((0, 0), [25, 0], 3, 5);
    // y = 12 meets it at t = 1/2 and t = 2 ((±15, 12)), and a line's quartic has no other real root.
    let line = Conic::of_carrier(&Carrier::new(0, 1, 12).unwrap()).unwrap();
    let Meeting::At { quartic, roots } = e.meeting(&line).unwrap() else {
        panic!("one curve");
    };
    let listed: Vec<(Piece, u8)> = roots
        .iter()
        .map(|r| (r.piece(), r.multiplicity()))
        .collect();
    assert_eq!(listed, [(Above, 1), (FarAbove, 1)]);
    // The circle about (15, 0) of radius 12 passes through the same point, (15, 12).
    let Meeting::At {
        quartic: other,
        roots: others,
    } = e.meeting(&circle((15, 0), 12)).unwrap()
    else {
        panic!("one curve");
    };
    let first = roots.iter().next().unwrap();
    let on = others
        .iter()
        .filter(|r| compare(&quartic, first, &other, r) == Ok(Sign::Zero))
        .count();
    assert_eq!(on, 1, "one point, reached by a line and by a circle");
}

/// How many of `roots`, of odd multiplicity, lie between t = lo / d and t = hi / d.
fn odd_between(quartic: &Quartic, roots: &[RealRoot], lo: i128, hi: i128, d: i128) -> usize {
    let (low, at_low) = single(d, lo);
    let (high, at_high) = single(d, hi);
    roots
        .iter()
        .filter(|r| {
            r.multiplicity() % 2 == 1
                && compare(quartic, r, &low, &at_low).unwrap() == Sign::Positive
                && compare(quartic, r, &high, &at_high).unwrap() == Sign::Negative
        })
        .count()
}

#[test]
fn each_root_stands_between_the_signs_of_the_other_curve() {
    // Walk the ellipse at t = j / 4 and read the other ellipse's sign at each point from its own
    // equation: a change of sign between two neighbours means an odd number of odd roots there, and
    // no change an even number.
    let mut rng = Rng::new();
    let mut crossings = 0;
    for _ in 0..cases(200) {
        let e = rng.ellipse();
        let other = rng.ellipse();
        // Near enough to meet, often.
        let shifted = Ellipse2::new(
            e.centre(),
            other.major(),
            other.ratio().0,
            other.ratio().1,
        )
        .unwrap();
        let conic = Conic::of_ellipse(&shifted).unwrap();
        let Meeting::At { quartic, roots } = e.meeting(&conic).unwrap() else {
            continue;
        };
        let list: Vec<RealRoot> = roots.iter().copied().collect();
        let (near, far) = (whole(&e), whole(&shifted));
        let d = IBig::from(4);
        let sign_at = |j: i128| {
            let [x, y, w] = near.at(&IBig::from(j), &d);
            sign_of(&far.value(&x, &y, &w))
        };
        // Skip a step whose ends sit on a root: the sign there says nothing.
        let mut last = None;
        for j in -40_i128..=40 {
            let sign = sign_at(j);
            if sign == Sign::Zero {
                last = None;
                continue;
            }
            if let Some((i, before)) = last {
                let odd = odd_between(&quartic, &list, i, j, 4);
                assert_eq!(odd % 2 == 1, sign != before, "t = {i} / 4 to {j} / 4");
                crossings += odd;
            }
            last = Some((j, sign));
        }
    }
    assert!(crossings > 0, "no pair crossed");
}

#[test]
fn random_tangent_circles_are_double_roots() {
    // A vertex of an ellipse along a Pythagorean axis, and a circle on the axis through it.
    let mut rng = Rng::new();
    for _ in 0..cases(300) {
        let (a, b, c) = [(3, 4, 5), (5, 12, 13), (8, 15, 17), (7, 24, 25)][(rng.next() % 4) as usize];
        let k = 1 + (rng.signed(24).unsigned_abs() as i64);
        let lambda = 1 + (rng.signed(8).unsigned_abs() as i64);
        let (mx, my) = (a * k, b * k);
        let Ok(centre) = P2::new(rng.signed(30) as i64, rng.signed(30) as i64) else {
            continue;
        };
        let (cx, cy) = (centre.x.get(), centre.y.get());
        let e = ellipse((cx, cy), [mx, my], 3, 5);
        let Ok(outer) = P2::new(cx + mx * (1 + lambda), cy + my * (1 + lambda)) else {
            continue;
        };
        let Ok(circle) = Circle2::new(outer, i128::from(c * k * lambda)) else {
            continue;
        };
        let conic = Conic::of_circle(&circle).unwrap();
        assert_eq!(contacts(&e, &conic), [(Piece::Zero, 2)]);
    }
}

#[test]
fn an_ellipse_and_a_line_tell_tangent_from_crossing() {
    // At (15, 12) on x²/625 + y²/225 = 1 the tangent is 9x + 20y = 375.
    let e = ellipse((0, 0), [25, 0], 3, 5);
    let touching = Carrier::new(9, 20, 375).unwrap();
    assert_eq!(e.meets(&touching).unwrap(), Contact::Tangent);
    assert_eq!(
        e.meets(&Carrier::new(9, 20, 374).unwrap()).unwrap(),
        Contact::Crossing
    );
    assert_eq!(
        e.meets(&Carrier::new(9, 20, 376).unwrap()).unwrap(),
        Contact::Apart
    );
    let (p, q) = (
        Crossing2::of_ellipse(&touching, &e, Root::First).unwrap(),
        Crossing2::of_ellipse(&touching, &e, Root::Second).unwrap(),
    );
    assert!(p.parts().2.is_zero());
    assert!(p.same(&q).unwrap());
    let at: Rational2 = p2(15, 12).into();
    assert!(p.same(&Crossing2::of_point(&at).unwrap()).unwrap());
    assert_eq!(e.side(&at).unwrap(), Sign::Zero);
    assert_eq!(
        Crossing2::of_ellipse(&Carrier::new(9, 20, 376).unwrap(), &e, Root::First).err(),
        Some(NumError::Degenerate)
    );
}

#[test]
fn a_line_ending_on_an_ellipse_closes_its_boundary() {
    // CADCraft 1690: a hatch boundary of lines and an ellipse is lost to the gaps a flattened ellipse
    // leaves. Here a line ends exactly on the ellipse, its end is on both carriers, and two lines
    // ending at one point of it end at one point, rational or not.
    let e = ellipse((7, -3), [25, 0], 3, 5);
    // (15, 12) from the centre, reached by a line from each of two grid points.
    let (z, from, to) = (p2(22, 9), p2(0, 0), p2(40, 0));
    let (a, b) = (
        Carrier::through(from, z).unwrap(),
        Carrier::through(to, z).unwrap(),
    );
    let on_e = |l: &Carrier| {
        let (p, q) = (
            Crossing2::of_ellipse(l, &e, Root::First).unwrap(),
            Crossing2::of_ellipse(l, &e, Root::Second).unwrap(),
        );
        for point in [&p, &q] {
            assert_eq!(point.side(l).unwrap(), Sign::Zero);
            assert_eq!(point.against_ellipse(&e).unwrap(), Sign::Zero);
        }
        assert_eq!(p.order(l, &q).unwrap(), Sign::Positive);
        (p, q)
    };
    let (p, q) = on_e(&a);
    let (r, s) = on_e(&b);
    let vertex = Crossing2::of_point(&z.into()).unwrap();
    let hit = |x: &Crossing2| x.same(&vertex).unwrap();
    assert!(hit(&p) ^ hit(&q));
    assert!(hit(&r) ^ hit(&s));
    let (one, two) = (if hit(&p) { p } else { q }, if hit(&r) { r } else { s });
    assert!(one.same(&two).unwrap());
    // Where the root is irrational: one line, written three ways, ends in one place.
    let l = Carrier::new(1, -1, 5).unwrap();
    let doubled = Carrier::new(2, -2, 10).unwrap();
    let back = Carrier::new(-1, 1, -5).unwrap();
    let (x, _) = on_e(&l);
    let (y, _) = on_e(&doubled);
    let (_, w) = on_e(&back);
    let k = big(&x.parts().2);
    let root = isqrt(&k);
    assert_ne!(&root * &root, k, "a root that is irrational");
    assert!(x.same(&y).unwrap());
    assert!(x.same(&w).unwrap());
}

// ------------------------------------------------------------------------------------------------
// The classes of the verification matrix.
// ------------------------------------------------------------------------------------------------

#[test]
fn random_inputs_at_every_scale_match_the_oracle() {
    let mut rng = Rng::new();
    for _ in 0..cases(10_000) {
        let e = rng.ellipse();
        let z = rng.point();
        assert_eq!(e.side(&z).unwrap(), side_oracle(&e, &z));
        let l = rng.carrier();
        assert_eq!(e.meets(&l).unwrap(), meets_oracle(&e, &l));
        let m = rng.carrier_across(&e);
        assert_eq!(e.meets(&m).unwrap(), Contact::Crossing);
        let p = Crossing2::of_ellipse(&m, &e, Root::First).unwrap();
        let q = Crossing2::of_ellipse(&m, &e, Root::Second).unwrap();
        assert_eq!(p.side(&m).unwrap(), Sign::Zero);
        assert_eq!(p.against_ellipse(&e).unwrap(), Sign::Zero);
        assert_eq!(q.against_ellipse(&e).unwrap(), Sign::Zero);
        assert_eq!(p.order(&m, &q).unwrap(), Sign::Positive);
        let other = rng.ellipse();
        if let Some(sign) = against_oracle(&p, &other) {
            assert_eq!(p.against_ellipse(&other).unwrap(), sign);
        }
    }
}

#[test]
fn one_step_from_an_ellipse_is_decided_by_the_exact_path() {
    // A small ellipse a long way out, along (3, 4), so a step is small beside the coordinates.
    let k = 1_i64 << 20;
    let (cx, cy) = (1_i64 << 38, -(1_i64 << 37));
    let e = ellipse((cx, cy), [3 * k, 4 * k], 3, 5);
    let (vx, vy) = (i128::from(cx + 3 * k), i128::from(cy + 4 * k));
    let w = 1_i128 << 60;
    let at = |step: i128| Rational2::new(vx * w + 3 * step, vy * w + 4 * step, w).unwrap();
    for (step, sign) in [(0, Sign::Zero), (1, Sign::Positive), (-1, Sign::Negative)] {
        assert_eq!(e.side(&at(step)).unwrap(), sign);
        assert_eq!(side_oracle(&e, &at(step)), sign);
    }
    // The tangent at the vertex, and a step of it either way.
    let c = 3 * vx + 4 * vy;
    for (step, contact) in [
        (0, Contact::Tangent),
        (1, Contact::Apart),
        (-1, Contact::Crossing),
    ] {
        let l = Carrier::new(3, 4, c + step).unwrap();
        assert_eq!(e.meets(&l).unwrap(), contact);
        assert_eq!(meets_oracle(&e, &l), contact);
    }
    // A crossing on the ellipse, and the ellipse's major axis a step longer or shorter.
    let along = Carrier::new(3, 4, 3 * vx + 4 * vy - 25).unwrap();
    let p = Crossing2::of_ellipse(&along, &e, Root::First).unwrap();
    for (step, sign) in [(0, Sign::Zero), (1, Sign::Negative), (-1, Sign::Positive)] {
        let wider = ellipse((cx, cy), [3 * k + 3 * step, 4 * k + 4 * step], 3, 5);
        assert_eq!(p.against_ellipse(&wider).unwrap(), sign);
    }
    // Two quartics with a root a step apart at the smallest step of a ratio.
    let (near, at_near) = single(1 << 60, 1 << 59);
    let (next, at_next) = single(1 << 60, (1 << 59) + 1);
    assert_eq!(compare(&near, &at_near, &next, &at_next).unwrap(), Sign::Negative);
    assert_eq!(compare(&next, &at_next, &near, &at_near).unwrap(), Sign::Positive);
    assert_eq!(compare(&near, &at_near, &near, &at_near).unwrap(), Sign::Zero);
}

fn crossed(family: impl Fn(u32) -> (Option<Sign>, Sign, Sign)) {
    let (mut inside, mut outside) = (0, 0);
    for step in 0..38 {
        let (filter, answer, oracle) = family(step);
        assert_eq!(answer, oracle, "step {step}");
        match filter {
            Some(sign) => {
                assert_eq!(sign, oracle, "step {step}");
                outside += 1;
            }
            None => inside += 1,
        }
    }
    assert!(inside > 0, "the filter refused nothing");
    assert!(outside > 0, "the filter decided nothing");
}

#[test]
fn ellipse_filters_cross_their_boundary_both_ways() {
    let k = 1_i64 << 10;
    let (cx, cy) = (1_i64 << 39, -(1_i64 << 38));
    let centre = p2(cx, cy);
    let e = ellipse((cx, cy), [3 * k, 4 * k], 3, 5);
    let (mx, my, p, q) = (i128::from(3 * k), i128::from(4 * k), 3, 5);
    let (vx, vy) = (i128::from(cx) + mx, i128::from(cy) + my);
    crossed(|step| {
        let z = Rational2::new(vx + (3 << step), vy + (4 << step), 1).unwrap();
        let ([x, y], v) = z.coordinates();
        (
            ellipse_side::filter(x, y, v, centre, mx, my, p, q),
            e.side(&z).unwrap(),
            Sign::Positive,
        )
    });
    let as_sign = |contact: Contact| match contact {
        Contact::Apart => Sign::Negative,
        Contact::Tangent => Sign::Zero,
        Contact::Crossing => Sign::Positive,
    };
    crossed(|step| {
        let l = Carrier::new(3, 4, 3 * vx + 4 * vy + (1 << step)).unwrap();
        let ([a, b], c) = l.coefficients();
        (
            ellipse_line::filter(a, b, c, centre, mx, my, p, q),
            as_sign(e.meets(&l).unwrap()),
            Sign::Negative,
        )
    });
}

#[test]
fn the_extreme_values_reach_every_width_and_every_step_fits() {
    let (p, q) = ((1_i64 << RATIO_BITS) - 1, 1_i64 << RATIO_BITS);
    let arc = Arc2::new(p2(M, M), p2(-M, M), p2(0, 0).into()).unwrap();
    for (x, y) in [(M, M), (M, -M), (-M, M), (-M, -M)] {
        for [mx, my] in [[TOP, TOP], [TOP, -TOP], [TOP, 1], [1, TOP]] {
            let e = ellipse((x, y), [mx, my], p, q);
            let other = ellipse((-x, -y), [my, -mx], p, q);
            for z in [
                Rational2::new(P, -P, W).unwrap(),
                Rational2::new(-P, P, 1).unwrap(),
                p2(x, y).into(),
            ] {
                assert_eq!(e.side(&z).unwrap(), side_oracle(&e, &z));
            }
            for (a, b) in [(N, N - 1), (N, -N + 1), (N, 1), (1, N)] {
                let c = (a * i128::from(x) + b * i128::from(y)).clamp(-D, D);
                let l = Carrier::new(a, b, c).unwrap();
                assert_eq!(e.meets(&l).unwrap(), meets_oracle(&e, &l));
                let Ok(u) = Crossing2::of_ellipse(&l, &e, Root::First) else {
                    continue;
                };
                let v = Crossing2::of_ellipse(&l, &e, Root::Second).unwrap();
                assert_eq!(u.side(&l).unwrap(), Sign::Zero);
                assert_eq!(u.against_ellipse(&e).unwrap(), Sign::Zero);
                assert!(u.cmp_x(&v).is_ok());
                assert!(u.cmp_y(&v).is_ok());
                assert!(arc.order_crossings(&u, &v).is_ok());
                if let Some(sign) = against_oracle(&u, &other) {
                    assert_eq!(u.against_ellipse(&other).unwrap(), sign);
                }
            }
            assert!(e.meeting(&Conic::of_ellipse(&other).unwrap()).is_ok());
        }
    }
    assert!(BIG_LIMBS * 64 > 2_024);
    // A quartic at the budget, with its roots a hair apart: the largest coefficients a pair of
    // ellipses makes.
    let a = ellipse((M, M), [TOP, TOP], p, q);
    let b = ellipse((M, M), [TOP, TOP - 1], p, q);
    let Meeting::At { quartic, roots } = a.meeting(&Conic::of_ellipse(&b).unwrap()).unwrap() else {
        panic!("not one curve");
    };
    let list: Vec<RealRoot> = roots.iter().copied().collect();
    for pair in list.windows(2) {
        assert_eq!(
            compare(&quartic, &pair[0], &quartic, &pair[1]).unwrap(),
            Sign::Negative
        );
    }
    // Past a budget is refused, never wrapped.
    assert_eq!(
        Ellipse2::new(p2(0, 0), [TOP + 1, 0], 1, 1).err(),
        Some(NumError::BudgetExceeded)
    );
    assert_eq!(
        Ellipse2::new(p2(0, 0), [1, 0], 1, q + 1).err(),
        Some(NumError::BudgetExceeded)
    );
    for (major, a, b) in [([0, 0], 1, 1), ([1, 0], 0, 1), ([1, 0], 3, 2), ([1, 0], 2, 4)] {
        assert_eq!(
            Ellipse2::new(p2(0, 0), major, a, b).err(),
            Some(NumError::Degenerate)
        );
    }
}

#[test]
fn ellipses_keep_their_laws() {
    let mut rng = Rng::new();
    for _ in 0..cases(4_000) {
        let e = rng.ellipse();
        let [mx, my] = e.major();
        let (p, q) = e.ratio();
        let reversed = Ellipse2::new(e.centre(), [-mx, -my], p, q).unwrap();
        let z = rng.point();
        let l = rng.carrier();
        // One curve, whichever way its axis is written.
        assert_eq!(e.side(&z).unwrap(), reversed.side(&z).unwrap());
        assert_eq!(e.meets(&l).unwrap(), reversed.meets(&l).unwrap());
        // A carrier turned is the same line.
        let ([a, b], c) = l.coefficients();
        let turned = Carrier::new(-a, -b, -c).unwrap();
        assert_eq!(e.meets(&l).unwrap(), e.meets(&turned).unwrap());
        // Moved by a grid step with the point, nothing changes.
        let ([x, y], w) = z.coordinates();
        let (cx, cy) = (e.centre().x.get(), e.centre().y.get());
        if cx < M - 8 {
            if let Ok(moved) = Rational2::new(x + 8 * w, y, w) {
                let there = Ellipse2::new(p2(cx + 8, cy), [mx, my], p, q).unwrap();
                assert_eq!(e.side(&z).unwrap(), there.side(&moved).unwrap());
            }
        }
    }
    // A pair's contacts are the same set from either curve.
    for _ in 0..cases(200) {
        let (e, other) = (rng.ellipse(), rng.ellipse());
        let there = Ellipse2::new(e.centre(), other.major(), other.ratio().0, other.ratio().1)
            .unwrap();
        let (a, b) = (
            e.meeting(&Conic::of_ellipse(&there).unwrap()).unwrap(),
            there.meeting(&Conic::of_ellipse(&e).unwrap()).unwrap(),
        );
        let sorted = |m: &Meeting| {
            let Meeting::At { roots, .. } = m else {
                return None;
            };
            let mut list: Vec<u8> = roots.iter().map(RealRoot::multiplicity).collect();
            list.sort_unstable();
            Some(list)
        };
        assert_eq!(sorted(&a), sorted(&b));
    }
}
