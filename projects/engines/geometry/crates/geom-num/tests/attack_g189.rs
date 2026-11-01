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
    (big(&a[axis]), big(&b[axis]), big(&k), big(&w))
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

/// The turn of the grid point `s` and two crossings, by the oracle.
fn turn_oracle(s: P2, p: &Crossing2, q: &Crossing2) -> Option<Sign> {
    let (sx, sy) = (IBig::from(s.x.get()), IBig::from(s.y.get()));
    let relative = |c: &Crossing2| {
        let (xa, xb, k, w) = coordinate(c, 0);
        let (ya, yb, _, _) = coordinate(c, 1);
        (&xa - &sx * &w, &ya - &sy * &w, xb, yb, k)
    };
    let (ux, uy, bx, by, k) = relative(p);
    let (vx, vy, cx, cy, l) = relative(q);
    let alpha = &ux * &vy - &uy * &vx;
    let beta = &bx * &vy - &by * &vx;
    let gamma = &ux * &cy - &uy * &cx;
    let delta = &bx * &cy - &by * &cx;
    sign_oracle([&alpha, &beta, &gamma, &delta], &k, &l)
}

#[test]
fn random_rounds_orders_and_roots_match_the_oracle() {
    let mut rng = Rng::new();
    let (mut made, mut turned) = (0, 0);
    for _ in 0..cases(4_000) {
        let circle = rng.circle();
        let (l, m) = (rng.carrier_near(&circle), rng.carrier_near(&circle));
        let (Ok(p), Ok(q)) = (
            Crossing2::of_circle(&l, &circle, Root::First),
            Crossing2::of_circle(&l, &circle, Root::Second),
        ) else {
            continue;
        };
        made += 1;
        // Along the carrier: compared by the coordinate it moves in, by the oracle.
        let [dx, dy] = l.direction();
        let (axis, way) = if dx == 0 { (1, dy) } else { (0, dx) };
        let forward = if way > 0 {
            Sign::Positive
        } else {
            Sign::Negative
        };
        if let Some(sign) = compare_oracle(&q, &p, axis) {
            assert_eq!(p.order(&l, &q).unwrap(), sign.mul(forward));
        }
        // Against a round about a rational centre.
        let (x, y, w) = (rng.signed(100), rng.signed(100), rng.signed(60).abs() + 1);
        let centre = Rational2::new(x, y, w).unwrap();
        let on = rng.grid();
        let (cx, cy, cw) = (IBig::from(x), IBig::from(y), IBig::from(w));
        let across = IBig::from(on.x.get()) * &cw - &cx;
        let up = IBig::from(on.y.get()) * &cw - &cy;
        let rr = &across * &across + &up * &up;
        if let Some(sign) = circle_oracle(&p, &cx, &cy, &cw, &rr) {
            assert_eq!(p.against_round(&Round::new(on, centre)).unwrap(), sign);
        }
        // Two crossings of an arc, from its start.
        let (s, e) = (rng.grid(), rng.grid());
        let arc = Arc2::bisector(s, e)
            .and_then(|b| b.meet(&m))
            .and_then(|c| Arc2::new(s, e, c));
        let Ok(arc) = arc else {
            continue;
        };
        let (Ok(u), Ok(v)) = (
            Crossing2::of_round(&l, &arc.round(), Root::First),
            Crossing2::of_round(&m, &arc.round(), Root::Second),
        ) else {
            continue;
        };
        turned += 1;
        if let Some(sign) = turn_oracle(s, &u, &v) {
            assert_eq!(arc.order_crossings(&u, &v).unwrap(), sign);
        }
    }
    assert!(made > 10, "only {made} carriers met");
    assert!(turned > 0, "no arc met both carriers");
    for _ in 0..cases(20_000) {
        let (a, b, k) = (rng.signed(120), rng.signed(60), rng.signed(110).abs());
        let w = |v: i128| Wide::<8>::from_i128(v);
        let answer = sign_of_root(&w(a), &w(b), &w(k)).unwrap();
        let (ia, ib, ik, zero) = (IBig::from(a), IBig::from(b), IBig::from(k), IBig::from(0));
        if let Some(sign) = sign_oracle([&ia, &ib, &zero, &zero], &ik, &zero) {
            assert_eq!(answer, sign, "{a} {b} {k}");
        } else {
            assert_eq!(answer, Sign::Zero, "{a} {b} {k}");
        }
    }
}

#[test]
fn one_step_from_a_crossing_is_decided_by_the_exact_path() {
    // (3, 4) · 2^30 on the circle of radius 5 · 2^30 about the origin, on y = 4 · 2^30, and the
    // points a step of 2^−82 from it, far inside any enclosure of it.
    let s = 1_i128 << 30;
    let circle = Circle2::new(p2(0, 0), 5 * s).unwrap();
    let l = Carrier::new(0, 1, 4 * s).unwrap();
    let p = Crossing2::of_circle(&l, &circle, Root::Second).unwrap();
    let w = 1_i128 << 82;
    let at = |dx: i128, dy: i128| {
        let z = Rational2::new(3 * s * w + dx, 4 * s * w + dy, w).unwrap();
        Crossing2::of_point(&z).unwrap()
    };
    for (step, sign) in [(0, Sign::Zero), (1, Sign::Negative), (-1, Sign::Positive)] {
        assert_eq!(p.cmp_x(&at(step, 0)).unwrap(), sign);
        assert_eq!(p.cmp_y(&at(0, step)).unwrap(), sign);
        assert_eq!(p.cmp_x_filter(&at(step, 0)), None);
        assert_eq!(p.order(&l, &at(step, 0)).unwrap(), sign.negate());
    }
    // A carrier through it, and a step of its offset either way, at 2^−73 of that offset.
    let (a, b, c) = (3 << 38, 4 << 38, 25 * (1_i128 << 68));
    for (step, sign) in [(0, Sign::Zero), (1, Sign::Negative), (-1, Sign::Positive)] {
        let through = Carrier::new(a, b, c + step).unwrap();
        assert_eq!(p.side(&through).unwrap(), sign);
    }
    // A step of the radius, and of the point a round passes through.
    let origin: Rational2 = p2(0, 0).into();
    let far = 5 << 30;
    for (step, sign) in [(0, Sign::Zero), (1, Sign::Negative), (-1, Sign::Positive)] {
        let wider = Circle2::new(p2(0, 0), 5 * s + step).unwrap();
        assert_eq!(p.against_circle(&wider).unwrap(), sign);
        let round = Round::new(p2(far + step as i64, 0), origin);
        assert_eq!(p.against_round(&round).unwrap(), sign);
    }
    // Along an arc: the lines y = 4 · 2^30 and a step either side of it.
    let arc = Arc2::new(p2(far, 0), p2(-far, 0), origin).unwrap();
    let reach = |dy: i128| {
        let line = Carrier::new(0, 1, 4 * s + dy).unwrap();
        Crossing2::of_round(&line, &arc.round(), Root::Second).unwrap()
    };
    let here = reach(0);
    for (step, sign) in [(0, Sign::Zero), (1, Sign::Positive), (-1, Sign::Negative)] {
        assert_eq!(arc.order_crossings(&here, &reach(step)).unwrap(), sign);
    }
    // A root, and a step of the term beside it.
    let word = |v: i128| Wide::<8>::from_i128(v);
    let top = 1_i128 << 100;
    let squared = word(top).checked_mul(&word(top)).unwrap();
    for (step, sign) in [(0, Sign::Zero), (1, Sign::Positive), (-1, Sign::Negative)] {
        assert_eq!(
            sign_of_root(&word(top + step), &word(-1), &squared).unwrap(),
            sign
        );
        assert_eq!(
            sign_of_two_roots(
                &word(6 + step),
                &word(0),
                &word(0),
                &word(-1),
                &word(4),
                &word(9)
            )
            .unwrap(),
            sign
        );
    }
}

#[test]
fn filters_cross_their_boundary_both_ways() {
    // (√24, 1) and (1, √24) on the circle of radius 5, against the dyadic points just below and
    // just above them, at every size of step: the filter decides while it can, says nothing after,
    // and the answer never changes.
    let circle = Circle2::new(p2(0, 0), 5).unwrap();
    let east =
        Crossing2::of_circle(&Carrier::new(0, 1, 1).unwrap(), &circle, Root::Second).unwrap();
    let north =
        Crossing2::of_circle(&Carrier::new(1, 0, 1).unwrap(), &circle, Root::First).unwrap();
    let (mut decided, mut undecided) = (0, 0);
    for e in 1..=80_u32 {
        let w = 1_i128 << e;
        let below = i128::try_from(isqrt(&(IBig::from(24) << (2 * e as usize)))).unwrap();
        for (near, sign) in [(below, Sign::Positive), (below + 1, Sign::Negative)] {
            let (x, y) = (
                Crossing2::of_point(&Rational2::new(near, 0, w).unwrap()).unwrap(),
                Crossing2::of_point(&Rational2::new(0, near, w).unwrap()).unwrap(),
            );
            match east.cmp_x_filter(&x) {
                Some(answer) => {
                    decided += 1;
                    assert_eq!(answer, sign);
                }
                None => undecided += 1,
            }
            assert_eq!(east.cmp_x(&x).unwrap(), sign);
            assert_eq!(north.cmp_y(&y).unwrap(), sign);
            if e <= 40 {
                let line = Carrier::new(w, 0, near).unwrap();
                assert_eq!(east.side(&line).unwrap(), sign);
            }
        }
    }
    assert!(decided > 0 && undecided > 0, "{decided} {undecided}");
}

#[test]
fn comparison_side_and_order_keep_their_laws() {
    let mut rng = Rng::new();
    let (mut made, mut arcs) = (0, 0);
    for _ in 0..cases(2_000) {
        let circle = rng.circle();
        let (l, m) = (rng.carrier_near(&circle), rng.carrier_near(&circle));
        let (Ok(p), Ok(q)) = (
            Crossing2::of_circle(&l, &circle, Root::First),
            Crossing2::of_circle(&m, &circle, Root::Second),
        ) else {
            continue;
        };
        made += 1;
        assert_eq!(p.cmp_x(&q).unwrap(), q.cmp_x(&p).unwrap().negate());
        assert_eq!(p.cmp_y(&q).unwrap(), q.cmp_y(&p).unwrap().negate());
        assert_eq!(p.order(&l, &q).unwrap(), q.order(&l, &p).unwrap().negate());
        // A carrier turned has its sides swapped.
        let ([a, b], c) = m.coefficients();
        let turned = Carrier::new(-a, -b, -c).unwrap();
        assert_eq!(p.side(&turned).unwrap(), p.side(&m).unwrap().negate());
        // A circle and the round of the same circle give one answer.
        let other = rng.circle();
        let (centre, r) = (other.centre(), other.radius() as i64);
        if let Ok(on) = P2::new(centre.x.get() + r, centre.y.get()) {
            let round = Round::new(on, centre.into());
            assert_eq!(
                p.against_circle(&other).unwrap(),
                p.against_round(&round).unwrap()
            );
        }
        // Two crossings of an arc, each way.
        let (s, e) = (rng.grid(), rng.grid());
        let arc = Arc2::bisector(s, e)
            .and_then(|b| b.meet(&m))
            .and_then(|c| Arc2::new(s, e, c));
        if let Ok(arc) = arc {
            if let (Ok(u), Ok(v)) = (
                Crossing2::of_round(&m, &arc.round(), Root::First),
                Crossing2::of_round(&m, &arc.round(), Root::Second),
            ) {
                arcs += 1;
                assert_eq!(
                    arc.order_crossings(&u, &v).unwrap(),
                    arc.order_crossings(&v, &u).unwrap().negate()
                );
            }
        }
        // The value turned has its sign turned.
        let w = |v: i128| Wide::<8>::from_i128(v);
        let [a, b, c, d] = [
            rng.signed(100),
            rng.signed(60),
            rng.signed(60),
            rng.signed(30),
        ];
        let (k, n) = (rng.signed(110).abs(), rng.signed(110).abs());
        assert_eq!(
            sign_of_root(&w(a), &w(b), &w(k)).unwrap(),
            sign_of_root(&w(-a), &w(-b), &w(k)).unwrap().negate()
        );
        assert_eq!(
            sign_of_two_roots(&w(a), &w(b), &w(c), &w(d), &w(k), &w(n)).unwrap(),
            sign_of_two_roots(&w(-a), &w(-b), &w(-c), &w(-d), &w(k), &w(n))
                .unwrap()
                .negate()
        );
    }
    assert!(made > 10, "only {made} carriers met");
    assert!(arcs > 0, "no arc met both carriers");
}

#[test]
fn two_circles_meet_on_both_and_their_order_is_the_one_stated() {
    // (3, 4) on the circle of radius 5 about the origin and on the circle of radius 5 about (6, 0);
    // (3, −4) is the other, to the right of the way from the first centre to the second.
    let (a, b) = (
        Circle2::new(p2(0, 0), 5).unwrap(),
        Circle2::new(p2(6, 0), 5).unwrap(),
    );
    let left = Crossing2::of_circles(&a, &b, Root::First).unwrap();
    let right = Crossing2::of_circles(&a, &b, Root::Second).unwrap();
    assert!(
        left.same(&Crossing2::of_point(&p2(3, 4).into()).unwrap())
            .unwrap()
    );
    assert!(
        right
            .same(&Crossing2::of_point(&p2(3, -4).into()).unwrap())
            .unwrap()
    );
    // The same two as rounds, a way along their own circles.
    let (r, s) = (
        Round::new(p2(5, 0), p2(0, 0).into()),
        Round::new(p2(11, 0), p2(6, 0).into()),
    );
    assert!(
        left.same(&Crossing2::of_rounds(&r, &s, Root::First).unwrap())
            .unwrap()
    );
    assert!(
        right
            .same(&Crossing2::of_rounds(&s, &r, Root::First).unwrap())
            .unwrap()
    );
    assert!(
        left.same(&Crossing2::of_round_and_circle(&r, &b, Root::First).unwrap())
            .unwrap()
    );
    // Tangent, apart, and one centre.
    let touching = Circle2::new(p2(10, 0), 5).unwrap();
    let t = Crossing2::of_circles(&a, &touching, Root::First).unwrap();
    assert!(t.parts().2.is_zero());
    assert!(
        t.same(&Crossing2::of_point(&p2(5, 0).into()).unwrap())
            .unwrap()
    );
    let apart = Circle2::new(p2(11, 0), 5).unwrap();
    assert_eq!(
        Crossing2::of_circles(&a, &apart, Root::First).err(),
        Some(NumError::Degenerate)
    );
    assert_eq!(
        Crossing2::of_circles(&a, &a, Root::First).err(),
        Some(NumError::Degenerate)
    );
    // Random rounds, at every scale: each crossing lies on both.
    let mut rng = Rng::new();
    let mut made = 0;
    for _ in 0..cases(4_000) {
        let (on, near) = (rng.grid(), rng.grid());
        let (x, y) = (
            i128::from(on.x.get()) + rng.signed(30),
            i128::from(on.y.get()) + rng.signed(30),
        );
        let w = 1 + rng.signed(40).abs();
        let first = Round::new(on, Rational2::new(x * w, y * w + 1, w).unwrap());
        let second = Round::new(
            on,
            Rational2::new(i128::from(near.x.get()), i128::from(near.y.get()), 1).unwrap(),
        );
        let (Ok(p), Ok(q)) = (
            Crossing2::of_rounds(&first, &second, Root::First),
            Crossing2::of_rounds(&first, &second, Root::Second),
        ) else {
            continue;
        };
        made += 1;
        for point in [&p, &q] {
            assert_eq!(point.against_round(&first).unwrap(), Sign::Zero);
            assert_eq!(point.against_round(&second).unwrap(), Sign::Zero);
        }
        // Each passes through `on`, so that is one of the two.
        let at = Crossing2::of_point(&on.into()).unwrap();
        assert!(p.same(&at).unwrap() || q.same(&at).unwrap());
    }
    assert!(made > 10, "only {made} pairs met");
    // At the budget: centres at the largest numerators and weights, and every width reached.
    let (top, weight) = ((1_i128 << 124) - 1, (1_i128 << 83) - 1);
    let (near, far) = (
        Rational2::new(top, -top, weight).unwrap(),
        Rational2::new(-top, top, weight - 2).unwrap(),
    );
    let (r, s) = (Round::new(p2(-M, M), near), Round::new(p2(M, -M), far));
    let p = Crossing2::of_rounds(&r, &s, Root::First).unwrap();
    let q = Crossing2::of_rounds(&r, &s, Root::Second).unwrap();
    assert_eq!(p.against_round(&r).unwrap(), Sign::Zero);
    assert_eq!(p.against_round(&s).unwrap(), Sign::Zero);
    assert!(p.cmp_x(&q).is_ok());
    let arc = Arc2::new(p2(M, M), p2(-M, M), p2(0, 0).into()).unwrap();
    assert!(arc.order_crossings(&p, &q).is_ok());
    assert!(BIG_LIMBS * 64 > 5_041);
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
            assert!(p.cmp_y(&q).is_ok());
            assert!(p.order(&l, &q).is_ok());
            let other = Carrier::new(b, -a, 0).unwrap();
            assert!(p.side(&other).is_ok());
            let far = Circle2::new(p2(-x, -y), R).unwrap();
            assert!(p.against_circle(&far).is_ok());
        }
    }
    // The turn of a grid point and two crossings of a round, at the budget.
    let arc = Arc2::new(p2(M, M), p2(-M, M), p2(0, 0).into()).unwrap();
    let round = arc.round();
    let (up, across) = (
        Carrier::new(N, N - 1, 1).unwrap(),
        Carrier::new(N - 1, -N, 1).unwrap(),
    );
    let a = Crossing2::of_round(&up, &round, Root::First).unwrap();
    let b = Crossing2::of_round(&across, &round, Root::Second).unwrap();
    assert!(arc.order_crossings(&a, &b).is_ok());
    assert_eq!(a.against_round(&round).unwrap(), Sign::Zero);
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
    assert!(sign_of_root(&a, &b, &k).is_ok());
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
