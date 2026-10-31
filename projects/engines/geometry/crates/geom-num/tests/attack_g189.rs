//! Attacks on piece G189: one-root numbers, where a line meets a circle, and the comparisons on them
//! (`geom_num::one_root`; chapter 14.2 of the kernel architecture). Every test tries to break a claim
//! of the piece's done-when or of the module's guarantees.
//!
//! What decides what is right is an oracle in `dashu` integers that squares nothing: each root is
//! worked out as a whole number to as many bits as the question needs, by Newton's method, and the
//! sign read from a bracket. The module squares and never takes a root on its exact path.
//!
//! - **Random inputs** at every scale up to each budget, against the oracle.
//! - **The extreme values**, where every width in the module's table is reached.
//! - **Ties**: one place reached from different carriers and circles, with different roots, must
//!   compare equal exactly; a tangency, k = 0, must make both roots one point.
//! - **Properties**: a crossing lies exactly on its carrier and its circle; the first root comes
//!   before the second; comparison is antisymmetric.
//! - **The filter** never contradicts the exact path, and on a tangency-heavy corpus says how often
//!   it decides.
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
    reason = "a to d, k and w are the formulas'"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use dashu_int::IBig;
use geom_num::arc::{Arc2, Circle2, RADIUS_BITS, Round};
use geom_num::carrier::{Carrier, NORMAL_BITS, OFFSET_BITS, Rational2};
use geom_num::grid::{COORD_MAX, P2};
use geom_num::one_root::{BIG_LIMBS, Crossing2, Root, sign_of_root, sign_of_two_roots};
use geom_num::wide::Wide;
use geom_num::{NumError, Sign};

/// The largest magnitudes each budget allows.
const N: i128 = (1 << NORMAL_BITS) - 1;
const D: i128 = (1 << OFFSET_BITS) - 1;
const R: i128 = (1 << RADIUS_BITS) - 1;
const M: i64 = COORD_MAX;

/// The default seed of every randomised test here (G189).
const DEFAULT_SEED: u64 = 0x6718_9000_5eed_0189;

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

    fn circle(&mut self) -> Circle2 {
        loop {
            let centre = self.grid();
            if let Ok(circle) = Circle2::new(centre, self.signed(RADIUS_BITS)) {
                return circle;
            }
        }
    }

    /// A carrier through a point of `circle`'s neighbourhood, so that they usually meet.
    fn carrier_near(&mut self, circle: &Circle2) -> Carrier {
        loop {
            let c = circle.centre();
            let r = circle.radius() as i64;
            let offset = |rng: &mut Rng| (rng.signed(40) as i64).clamp(-r, r);
            let (dx, dy) = (offset(self), offset(self));
            let Some(p) = P2::new(
                (c.x.get() + dx / 2).clamp(-M, M),
                (c.y.get() + dy / 2).clamp(-M, M),
            )
            .ok() else {
                continue;
            };
            let q = self.grid();
            if let Ok(l) = Carrier::through(p, q) {
                return l;
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
}

fn cases(full: u64) -> u64 {
    if cfg!(debug_assertions) {
        full / 20
    } else {
        full
    }
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

fn p2(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
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

/// a + b√k + c√l + d√(kl) as a bracket at `p` fractional bits: its low and high ends, scaled.
fn bracket(terms: [&IBig; 4], k: &IBig, l: &IBig, p: usize) -> (IBig, IBig) {
    let scale = IBig::from(1) << p;
    let root = |v: &IBig| isqrt(&(v * &scale * &scale));
    let (rk, rl, rkl) = (root(k), root(l), root(&(k * l)));
    let term = |c: &IBig, r: &IBig| -> (IBig, IBig) {
        let (low, high) = (c * r, c * (r + IBig::from(1)));
        if low <= high {
            (low, high)
        } else {
            (high, low)
        }
    };
    let (a, b, c, d) = (terms[0], terms[1], terms[2], terms[3]);
    let mut low = a * &scale;
    let mut high = low.clone();
    for part in [term(b, &rk), term(c, &rl), term(d, &rkl)] {
        low += part.0;
        high += part.1;
    }
    (low, high)
}

/// The sign of the value, by brackets ever finer; `None` if it is within 2^-p of zero at the end,
/// which the caller reads as a tie it must know is one.
fn sign_oracle(terms: [&IBig; 4], k: &IBig, l: &IBig) -> Option<Sign> {
    for p in [64, 256, 1024, 4096] {
        let (low, high) = bracket(terms, k, l, p);
        if low > IBig::from(0) {
            return Some(Sign::Positive);
        }
        if high < IBig::from(0) {
            return Some(Sign::Negative);
        }
    }
    None
}

/// One coordinate of a crossing as whole numbers.
fn coordinate(p: &Crossing2, axis: usize) -> (IBig, IBig, IBig, IBig) {
    let (a, b, k, w) = p.parts();
    (big(&a[axis]), IBig::from(b[axis]), big(&k), big(&w))
}

fn compare_oracle(p: &Crossing2, q: &Crossing2, axis: usize) -> Option<Sign> {
    let (pa, pb, pk, pw) = coordinate(p, axis);
    let (qa, qb, qk, qw) = coordinate(q, axis);
    let alpha = &pa * &qw - &qa * &pw;
    let (beta, gamma) = (&pb * &qw, -(&qb * &pw));
    sign_oracle([&alpha, &beta, &gamma, &IBig::from(0)], &pk, &qk)
}

fn side_oracle(p: &Crossing2, l: &Carrier) -> Option<Sign> {
    let ([a, b], c) = l.coefficients();
    let (a, b, c) = (IBig::from(a), IBig::from(b), IBig::from(c));
    let (xa, xb, k, w) = coordinate(p, 0);
    let (ya, yb, _, _) = coordinate(p, 1);
    let whole = &a * &xa + &b * &ya - &c * &w;
    let root = &a * &xb + &b * &yb;
    sign_oracle(
        [&whole, &root, &IBig::from(0), &IBig::from(0)],
        &k,
        &IBig::from(0),
    )
}

/// Where `p` lies against a circle of centre (cx, cy) / cw and squared radius rr / cw², by expanding
/// ‖Z − C‖² − ρ directly.
fn circle_oracle(p: &Crossing2, cx: &IBig, cy: &IBig, cw: &IBig, rr: &IBig) -> Option<Sign> {
    let (xa, xb, k, w) = coordinate(p, 0);
    let (ya, yb, _, _) = coordinate(p, 1);
    let (ux, uy) = (cw * &xa - cx * &w, cw * &ya - cy * &w);
    let (vx, vy) = (cw * &xb, cw * &yb);
    let whole = &ux * &ux + &uy * &uy + (&vx * &vx + &vy * &vy) * &k - rr * &w * &w;
    let root = (&ux * &vx + &uy * &vy) * IBig::from(2);
    sign_oracle(
        [&whole, &root, &IBig::from(0), &IBig::from(0)],
        &k,
        &IBig::from(0),
    )
}

#[test]
fn random_crossings_match_the_oracle() {
    let mut rng = Rng::new();
    let mut made = 0;
    for _ in 0..cases(4_000) {
        let (c1, c2) = (rng.circle(), rng.circle());
        let (l1, l2) = (rng.carrier_near(&c1), rng.carrier_near(&c2));
        let (Ok(p), Ok(q)) = (
            Crossing2::of_circle(&l1, &c1, Root::First),
            Crossing2::of_circle(&l2, &c2, Root::Second),
        ) else {
            continue;
        };
        made += 1;
        for (axis, answer) in [(0, p.cmp_x(&q).unwrap()), (1, p.cmp_y(&q).unwrap())] {
            if let Some(sign) = compare_oracle(&p, &q, axis) {
                assert_eq!(answer, sign);
            }
        }
        if let Some(sign) = side_oracle(&p, &l2) {
            assert_eq!(p.side(&l2).unwrap(), sign);
        }
        let c = c2.centre();
        let (cx, cy) = (IBig::from(c.x.get()), IBig::from(c.y.get()));
        let rr = IBig::from(c2.radius()) * IBig::from(c2.radius());
        if let Some(sign) = circle_oracle(&p, &cx, &cy, &IBig::from(1), &rr) {
            assert_eq!(p.against_circle(&c2).unwrap(), sign);
        }
    }
    assert!(made > 100, "only {made} pairs met");
}

#[test]
fn a_crossing_lies_exactly_on_its_carrier_and_its_circle() {
    let mut rng = Rng::new();
    let mut made = 0;
    for _ in 0..cases(4_000) {
        let circle = rng.circle();
        let l = rng.carrier_near(&circle);
        let (Ok(p), Ok(q)) = (
            Crossing2::of_circle(&l, &circle, Root::First),
            Crossing2::of_circle(&l, &circle, Root::Second),
        ) else {
            continue;
        };
        made += 1;
        for c in [&p, &q] {
            assert_eq!(c.side(&l).unwrap(), Sign::Zero);
            assert_eq!(c.against_circle(&circle).unwrap(), Sign::Zero);
        }
        let (_, _, k, _) = p.parts();
        if k.is_zero() {
            assert!(p.same(&q).unwrap());
        } else {
            assert_eq!(p.order(&l, &q).unwrap(), Sign::Positive);
            assert_eq!(q.order(&l, &p).unwrap(), Sign::Negative);
            assert!(!p.same(&q).unwrap());
        }
    }
    assert!(made > 100, "only {made} pairs met");
}

#[test]
fn a_rational_centre_gives_crossings_on_its_circle() {
    // The circle about (3, 4 / 3) through (0, 0) and (6, 0), and lines across it.
    let round = Round::new(p2(0, 0), Rational2::new(9, 4, 3).unwrap());
    for (a, b, c) in [(0, 1, 1), (1, 0, 2), (1, 1, 3), (3, 4, 10), (5, -7, 2)] {
        let l = Carrier::new(a, b, c).unwrap();
        let p = Crossing2::of_round(&l, &round, Root::First).unwrap();
        let q = Crossing2::of_round(&l, &round, Root::Second).unwrap();
        for c in [&p, &q] {
            assert_eq!(c.side(&l).unwrap(), Sign::Zero);
            assert_eq!(c.against_round(&round).unwrap(), Sign::Zero);
        }
        assert_eq!(p.order(&l, &q).unwrap(), Sign::Positive);
    }
    let outside = Carrier::new(0, 1, 9).unwrap();
    assert_eq!(
        Crossing2::of_round(&outside, &round, Root::First).unwrap_err(),
        NumError::Degenerate
    );
}

#[test]
fn one_place_reached_two_ways_is_one_place() {
    // (3, 4) is on the circle of radius 5 about the origin, and on the line y = 4; (3, 4) is on the
    // circle of radius 5 about (6, 0) too, and on the line x = 3.
    let (a, b) = (
        Circle2::new(p2(0, 0), 5).unwrap(),
        Circle2::new(p2(6, 0), 5).unwrap(),
    );
    let (y4, x3) = (
        Carrier::new(0, 1, 4).unwrap(),
        Carrier::new(1, 0, 3).unwrap(),
    );
    let p = Crossing2::of_circle(&y4, &a, Root::Second).unwrap();
    let q = Crossing2::of_circle(&x3, &b, Root::First).unwrap();
    let r: Crossing2 = Crossing2::of_point(&p2(3, 4).into()).unwrap();
    assert!(p.same(&r).unwrap(), "{p:?}");
    assert!(q.same(&r).unwrap(), "{q:?}");
    assert!(p.same(&q).unwrap());
    // And a root that is irrational, reached by two lines: (√2, √2) on x = y and on x + y = 2√2 is
    // not a carrier, so take y = x and the circle of radius 2 about the origin from each side.
    let c = Circle2::new(p2(0, 0), 2).unwrap();
    let diagonal = Carrier::new(1, -1, 0).unwrap();
    let back = Carrier::new(-1, 1, 0).unwrap();
    let forward = Crossing2::of_circle(&diagonal, &c, Root::Second).unwrap();
    let again = Crossing2::of_circle(&back, &c, Root::First).unwrap();
    assert!(forward.same(&again).unwrap());
    assert_eq!(forward.cmp_x(&again).unwrap(), Sign::Zero);
    let doubled = Carrier::new(2, -2, 0).unwrap();
    let other = Crossing2::of_circle(&doubled, &c, Root::Second).unwrap();
    assert!(forward.same(&other).unwrap());
}

#[test]
fn a_tangency_is_one_point_and_k_is_exactly_zero() {
    let c = Circle2::new(p2(0, 0), 5).unwrap();
    let l = Carrier::new(3, 4, 25).unwrap();
    let (p, q) = (
        Crossing2::of_circle(&l, &c, Root::First).unwrap(),
        Crossing2::of_circle(&l, &c, Root::Second).unwrap(),
    );
    assert!(p.parts().2.is_zero());
    assert!(p.same(&q).unwrap());
    assert!(
        p.same(&Crossing2::of_point(&p2(3, 4).into()).unwrap())
            .unwrap()
    );
    assert_eq!(p.order(&l, &q).unwrap(), Sign::Zero);
}

#[test]
fn the_extreme_values_reach_every_width_and_every_step_fits() {
    let m = i128::from(M);
    for (x, y) in [(M, M), (M, -M), (-M, M), (-M, -M)] {
        let circle = Circle2::new(p2(x, y), R).unwrap();
        for (a, b) in [(N, N), (N, -N), (-N, N), (-N, -N), (N, 1), (1, N)] {
            let c = (a * (x.signum() as i128) * m + b * (y.signum() as i128) * m).clamp(-D, D);
            let l = Carrier::new(a, b, c).unwrap();
            let Ok(p) = Crossing2::of_circle(&l, &circle, Root::First) else {
                continue;
            };
            let q = Crossing2::of_circle(&l, &circle, Root::Second).unwrap();
            assert_eq!(p.side(&l).unwrap(), Sign::Zero);
            assert_eq!(p.against_circle(&circle).unwrap(), Sign::Zero);
            assert!(p.cmp_x(&q).is_ok());
            let other = Carrier::new(b, -a, 0).unwrap();
            assert!(p.side(&other).is_ok());
            let far = Circle2::new(p2(-x, -y), R).unwrap();
            assert!(p.against_circle(&far).is_ok());
        }
    }
    // 1,688 bits: the two-root form at the widths the turn of two crossings gives.
    let limit = 1_i128 << 126;
    let wide = |v: i128, by: u32| -> Wide<8> {
        let v = Wide::<8>::from_i128(v);
        (0..by).fold(v, |w, _| w.checked_add(&w).unwrap())
    };
    let (a, b, c, d) = (
        wide(limit, 294),
        wide(limit, 125),
        wide(-limit, 125),
        wide(limit, 40),
    );
    let (k, l) = (wide(limit, 209), wide(limit, 209));
    assert!(sign_of_two_roots(&a, &b, &c, &d, &k, &l).is_ok());
    assert!(BIG_LIMBS * 64 > 1_688);
}

#[test]
fn signs_of_small_roots_are_the_right_ones() {
    let w = |v: i128| Wide::<4>::from_i128(v);
    // 3 − 2√2 > 0, 2 − 3√2 < 0, −3 + 2√2 < 0, 3 + 0√2 > 0, 0 − 1√2 < 0, 2 − 1√4 = 0.
    assert_eq!(sign_of_root(&w(3), &w(-2), &w(2)).unwrap(), Sign::Positive);
    assert_eq!(sign_of_root(&w(2), &w(-3), &w(2)).unwrap(), Sign::Negative);
    assert_eq!(sign_of_root(&w(-3), &w(2), &w(2)).unwrap(), Sign::Negative);
    assert_eq!(sign_of_root(&w(3), &w(0), &w(2)).unwrap(), Sign::Positive);
    assert_eq!(sign_of_root(&w(0), &w(-1), &w(2)).unwrap(), Sign::Negative);
    assert_eq!(sign_of_root(&w(2), &w(-1), &w(4)).unwrap(), Sign::Zero);
    assert_eq!(sign_of_root(&w(0), &w(5), &w(0)).unwrap(), Sign::Zero);
    assert_eq!(sign_of_root(&w(0), &w(0), &w(7)).unwrap(), Sign::Zero);
    // √2 + √3 − √(5 + 2√6) is zero, as 1 + … shows in its two-root form: −0 + √2 + √3 − √6 · 0.
    let s = |a, b, c, d, k, l| sign_of_two_roots(&w(a), &w(b), &w(c), &w(d), &w(k), &w(l)).unwrap();
    assert_eq!(s(0, 1, -1, 0, 2, 2), Sign::Zero);
    assert_eq!(s(1, 1, -1, 0, 2, 3), Sign::Positive);
    assert_eq!(s(-1, 1, 1, 0, 2, 3), Sign::Positive);
    assert_eq!(s(0, 1, -1, 0, 2, 3), Sign::Negative);
    assert_eq!(s(0, 1, -1, 0, 3, 2), Sign::Positive);
    assert_eq!(s(0, 0, 1, 0, 2, 3), Sign::Positive);
    assert_eq!(s(0, 0, 0, -1, 2, 3), Sign::Negative);
    assert_eq!(s(5, 0, 0, -1, 2, 3), Sign::Positive);
    assert_eq!(s(2, 0, 0, -1, 2, 3), Sign::Negative);
    assert_eq!(s(6, 0, 0, -1, 4, 9), Sign::Zero);
    assert_eq!(s(0, 3, -2, 0, 4, 9), Sign::Zero);
    assert_eq!(s(1, 0, 3, 0, 2, 0), Sign::Positive);
    assert_eq!(s(0, 0, 0, 0, 5, 7), Sign::Zero);
}

#[test]
fn two_roots_match_the_oracle_at_every_scale() {
    let mut rng = Rng::new();
    for _ in 0..cases(20_000) {
        let [a, b, c, d] = [
            rng.signed(100),
            rng.signed(60),
            rng.signed(60),
            rng.signed(30),
        ];
        let (k, l) = (rng.signed(110).abs(), rng.signed(110).abs());
        let w = |v: i128| Wide::<8>::from_i128(v);
        let answer = sign_of_two_roots(&w(a), &w(b), &w(c), &w(d), &w(k), &w(l)).unwrap();
        let (ia, ib, ic, id) = (IBig::from(a), IBig::from(b), IBig::from(c), IBig::from(d));
        let (ik, il) = (IBig::from(k), IBig::from(l));
        if let Some(sign) = sign_oracle([&ia, &ib, &ic, &id], &ik, &il) {
            assert_eq!(answer, sign, "{a} {b} {c} {d} {k} {l}");
        } else {
            assert_eq!(answer, Sign::Zero, "{a} {b} {c} {d} {k} {l}");
        }
    }
}

#[test]
fn the_filter_never_contradicts_the_exact_path() {
    let mut rng = Rng::new();
    let mut decided = 0;
    for _ in 0..cases(4_000) {
        let circle = rng.circle();
        let l = rng.carrier_near(&circle);
        let m = rng.carrier_near(&circle);
        let (Ok(p), Ok(q)) = (
            Crossing2::of_circle(&l, &circle, Root::First),
            Crossing2::of_circle(&m, &circle, Root::Second),
        ) else {
            continue;
        };
        if let Some(sign) = p.cmp_x_filter(&q) {
            decided += 1;
            if let Some(exact) = compare_oracle(&p, &q, 0) {
                assert_eq!(sign, exact);
            }
        }
    }
    assert!(decided > 0);
}

#[test]
fn crossings_keep_their_order_along_an_arc() {
    // The circle of radius 5 about the origin, two lines across it, the arc from (5, 0) to (-5, 0)
    // running through (0, 5).
    let centre: Rational2 = p2(0, 0).into();
    let arc = Arc2::new(p2(5, 0), p2(-5, 0), centre).unwrap();
    let (right, up) = (
        Carrier::new(0, 1, 1).unwrap(),
        Carrier::new(1, 0, -2).unwrap(),
    );
    let circle = Round::new(p2(5, 0), centre);
    // y = 1 reaches the circle at x = ±√24; x = −2 at y = ±√21.
    let east = Crossing2::of_round(&right, &circle, Root::Second).unwrap();
    let west = Crossing2::of_round(&right, &circle, Root::First).unwrap();
    let north = Crossing2::of_round(&up, &circle, Root::First).unwrap();
    let south = Crossing2::of_round(&up, &circle, Root::Second).unwrap();
    let (a, b) = if east.cmp_x(&west).unwrap() == Sign::Positive {
        (east, west)
    } else {
        (west, east)
    };
    let (n, s) = if north.cmp_y(&south).unwrap() == Sign::Positive {
        (north, south)
    } else {
        (south, north)
    };
    for point in [&a, &b, &n] {
        assert!(arc.holds_crossing(point).unwrap());
    }
    assert!(!arc.holds_crossing(&s).unwrap());
    // From (5, 0): a, then n, then b.
    assert_eq!(arc.order_crossings(&a, &n).unwrap(), Sign::Positive);
    assert_eq!(arc.order_crossings(&n, &b).unwrap(), Sign::Positive);
    assert_eq!(arc.order_crossings(&a, &b).unwrap(), Sign::Positive);
    assert_eq!(arc.order_crossings(&b, &a).unwrap(), Sign::Negative);
    assert_eq!(arc.order_crossings(&n, &n).unwrap(), Sign::Zero);
}

#[test]
fn a_rational_point_compares_with_a_crossing() {
    let c = Circle2::new(p2(0, 0), 5).unwrap();
    let l = Carrier::new(0, 1, 1).unwrap();
    let p = Crossing2::of_circle(&l, &c, Root::First).unwrap();
    let q = Crossing2::of_point(&Rational2::new(7, 1, 2).unwrap()).unwrap();
    assert!(q.cmp_x(&p).is_ok());
    assert_eq!(q.against_circle(&c).unwrap(), Sign::Negative);
    assert_eq!(
        Crossing2::of_point(&p2(9, 0).into())
            .unwrap()
            .against_circle(&c)
            .unwrap(),
        Sign::Positive
    );
}

#[test]
fn random_carriers_are_refused_or_exact() {
    let mut rng = Rng::new();
    for _ in 0..cases(4_000) {
        let circle = rng.circle();
        let l = rng.carrier();
        match Crossing2::of_circle(&l, &circle, Root::First) {
            Ok(p) => assert_eq!(p.side(&l).unwrap(), Sign::Zero),
            Err(e) => assert_eq!(e, NumError::Degenerate),
        }
    }
}
