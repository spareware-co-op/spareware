//! Attacks on piece G188: circles and arcs, their inside and outside, tangency and order
//! (`geom_num::arc`; chapters 14.2 and 15.10 of the kernel architecture). Every test tries to break a
//! claim of the piece's done-when or of the module's guarantees.
//!
//! What decides what is right is an oracle in `dashu` rationals that shares no formulation with the
//! module: each point is divided out to its fractions and each radius squared directly, where the
//! module writes differences of squares and never divides; a tangency is a line's distance squared
//! against a radius squared, and an arc is walked by the angle's quadrant and tangent.
//!
//! - **Random inputs** at every scale up to each budget, against the oracle.
//! - **The extreme values**: every sign combination at each budget's largest value, where the
//!   certificate's widths are reached.
//! - **One step from degenerate**: points exactly on a circle and lines exactly tangent to one, from
//!   Pythagorean triples, then moved one step.
//! - **Properties**: an arc's ends lie on its circle exactly, whatever the bisector point; order
//!   along an arc agrees with walking it; a pair of circles is what their radii say.
//! - **The budget and the degenerate inputs**, each refused with its error.
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
    reason = "a, b and c are a carrier's, x, y and w a point's, as in the predicates"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_num::arc::{Arc2, Circle2, Contact, Pair, RADIUS_BITS, Round};
use geom_num::carrier::{Carrier, NORMAL_BITS, OFFSET_BITS, POINT_BITS, Rational2, WEIGHT_BITS};
use geom_num::generated::{
    circle_gap, circle_line, circle_side, round_grid, round_line, round_rational,
};
use geom_num::grid::{COORD_MAX, P2};
use geom_num::{NumError, Sign};

/// The largest magnitudes each budget allows.
const N: i128 = (1 << NORMAL_BITS) - 1;
const D: i128 = (1 << OFFSET_BITS) - 1;
const P: i128 = (1 << POINT_BITS) - 1;
const W: i128 = (1 << WEIGHT_BITS) - 1;
const R: i128 = (1 << RADIUS_BITS) - 1;
const M: i64 = COORD_MAX;

/// The default seed of every randomised test here (G188).
const DEFAULT_SEED: u64 = 0x6718_8000_5eed_0188;

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

    fn point(&mut self) -> Rational2 {
        loop {
            let (x, y, w) = (
                self.signed(POINT_BITS),
                self.signed(POINT_BITS),
                self.signed(WEIGHT_BITS),
            );
            if let Ok(point) = Rational2::new(x, y, w) {
                return point;
            }
        }
    }

    fn circle(&mut self) -> Circle2 {
        loop {
            let centre = self.grid();
            if let Ok(circle) = Circle2::new(centre, self.signed(RADIUS_BITS)) {
                return circle;
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

fn big(v: i128) -> RBig {
    RBig::from(IBig::from(v))
}

fn grid(p: P2) -> [RBig; 2] {
    [i128::from(p.x.get()), i128::from(p.y.get())].map(big)
}

fn p2(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn sign_of(v: &RBig) -> Sign {
    match v.partial_cmp(&RBig::ZERO) {
        Some(core::cmp::Ordering::Greater) => Sign::Positive,
        Some(core::cmp::Ordering::Less) => Sign::Negative,
        _ => Sign::Zero,
    }
}

fn fractions(p: &Rational2) -> [RBig; 2] {
    let ([x, y], w) = p.coordinates();
    [big(x) / big(w), big(y) / big(w)]
}

fn squared(a: &[RBig; 2], b: &[RBig; 2]) -> RBig {
    let (dx, dy) = (&a[0] - &b[0], &a[1] - &b[1]);
    &dx * &dx + &dy * &dy
}

/// The sign of the distance from `z` squared against the radius squared.
fn round_oracle(r: &Round, z: &[RBig; 2]) -> Sign {
    let centre = fractions(&r.centre());
    let on = grid(r.on());
    sign_of(&(squared(z, &centre) - squared(&on, &centre)))
}

fn circle_oracle(c: &Circle2, z: &[RBig; 2]) -> Sign {
    let centre = grid(c.centre());
    let radius = big(c.radius());
    sign_of(&(squared(z, &centre) - &radius * &radius))
}

/// How a line meets a circle of centre `k` and squared radius `rr`, by the line's distance.
fn contact_oracle(l: &Carrier, k: &[RBig; 2], rr: &RBig) -> Contact {
    let ([a, b], c) = l.coefficients();
    let (a, b, c) = (big(a), big(b), big(c));
    let h = &a * &k[0] + &b * &k[1] - c;
    let distance = (&h * &h) / (&a * &a + &b * &b);
    match sign_of(&(distance - rr)) {
        Sign::Positive => Contact::Apart,
        Sign::Zero => Contact::Tangent,
        Sign::Negative => Contact::Crossing,
    }
}

fn round_contact_oracle(r: &Round, l: &Carrier) -> Contact {
    let centre = fractions(&r.centre());
    let rr = squared(&grid(r.on()), &centre);
    contact_oracle(l, &centre, &rr)
}

fn circle_contact_oracle(c: &Circle2, l: &Carrier) -> Contact {
    let radius = big(c.radius());
    contact_oracle(l, &grid(c.centre()), &(&radius * &radius))
}

#[test]
fn random_inputs_at_every_scale_match_the_oracle() {
    let mut rng = Rng::new();
    for _ in 0..cases(40_000) {
        let round = Round::new(rng.grid(), rng.point());
        let (z, p, l) = (rng.grid(), rng.point(), rng.carrier());
        assert_eq!(
            round.side_of_grid(z).unwrap(),
            round_oracle(&round, &grid(z))
        );
        assert_eq!(
            round.side(&p).unwrap(),
            round_oracle(&round, &fractions(&p))
        );
        assert_eq!(round.meets(&l).unwrap(), round_contact_oracle(&round, &l));
        let circle = rng.circle();
        assert_eq!(
            circle.side(&p).unwrap(),
            circle_oracle(&circle, &fractions(&p))
        );
        assert_eq!(
            circle.meets(&l).unwrap(),
            circle_contact_oracle(&circle, &l)
        );
        // Two points in order along an arc: the turn of its start and them.
        let (s, e, q) = (rng.grid(), rng.grid(), rng.point());
        let arc = Arc2::bisector(s, e)
            .and_then(|b| b.meet(&l))
            .and_then(|c| Arc2::new(s, e, c));
        if let Ok(arc) = arc {
            assert_eq!(
                arc.order(&p, &q).unwrap(),
                Rational2::orient(&s.into(), &p, &q).unwrap()
            );
        }
    }
}

/// `tops` under every choice of signs.
fn corners<const K: usize>(tops: [i128; K]) -> impl Iterator<Item = [i128; K]> {
    (0_u32..1 << K).map(move |bits| {
        let mut k = 0;
        tops.map(|v| {
            k += 1;
            if bits >> (k - 1) & 1 == 0 { v } else { -v }
        })
    })
}

#[test]
fn the_extreme_values_reach_every_width_and_every_step_fits() {
    let m = i128::from(M);
    for [zx, zy, px, py] in corners([m, m, m, m]) {
        let (z, p) = (p2(zx as i64, zy as i64), p2(px as i64, py as i64));
        for [cx, cy, w] in corners([P, P, W]) {
            let round = Round::new(p, Rational2::new(cx, cy, w).unwrap());
            assert_eq!(
                round.side_of_grid(z).unwrap(),
                round_oracle(&round, &grid(z))
            );
            let q = Rational2::new(-cy, cx, 1 + (w >> 1)).unwrap();
            assert_eq!(
                round.side(&q).unwrap(),
                round_oracle(&round, &fractions(&q))
            );
            let l = Carrier::new(N, -N, D).unwrap();
            assert_eq!(round.meets(&l).unwrap(), round_contact_oracle(&round, &l));
        }
        let circle = Circle2::new(p, R).unwrap();
        for [x, y, v] in corners([P, P, W]) {
            let q = Rational2::new(x, y, v).unwrap();
            assert_eq!(
                circle.side(&q).unwrap(),
                circle_oracle(&circle, &fractions(&q))
            );
        }
        for [a, b, c] in corners([N, N, D]) {
            let l = Carrier::new(a, b, c).unwrap();
            assert_eq!(
                circle.meets(&l).unwrap(),
                circle_contact_oracle(&circle, &l)
            );
        }
        let other = Circle2::new(z, R - 1).unwrap();
        assert!(circle.against(&other).is_ok());
    }
    let arc = Arc2::new(p2(M, M), p2(-M, M), p2(0, 0).into()).unwrap();
    for [x, y, w] in corners([P, P, W]) {
        let (a, b) = (
            Rational2::new(x, y, w).unwrap(),
            Rational2::new(-y, x, w).unwrap(),
        );
        assert_eq!(
            arc.order(&a, &b).unwrap(),
            Rational2::orient(&arc.start().into(), &a, &b).unwrap()
        );
    }
    // One past a budget is outside each predicate's domain, never a wrapped answer.
    let (o, big_p) = (p2(0, 0), P + 1);
    assert_eq!(round_grid::exact(o, o, big_p, 0, 1), None);
    assert_eq!(round_rational::exact(big_p, 0, 1, o, 0, 0, 1), None);
    assert_eq!(round_line::exact(N + 1, 0, 0, o, 0, 0, 1), None);
    assert_eq!(circle_side::exact(0, 0, W + 1, o, 1), None);
    assert_eq!(circle_line::exact(1, 0, D + 1, o, 1), None);
    assert_eq!(circle_gap::exact(o, o, 1 << 42), None);
}

#[test]
fn a_chord_of_any_kind_puts_both_ends_on_the_circle() {
    let mut rng = Rng::new();
    for _ in 0..cases(20_000) {
        let (s, e) = (rng.grid(), rng.grid());
        let Ok(bisector) = Arc2::bisector(s, e) else {
            continue;
        };
        let Ok(centre) = bisector.meet(&rng.carrier()) else {
            continue;
        };
        let arc = Arc2::new(s, e, centre).unwrap();
        assert_eq!(arc.round().side_of_grid(s).unwrap(), Sign::Zero);
        assert_eq!(arc.round().side_of_grid(e).unwrap(), Sign::Zero);
        assert_eq!(arc.round().side(&s.into()).unwrap(), Sign::Zero);
        assert_eq!(arc.round().side(&e.into()).unwrap(), Sign::Zero);
        assert!(arc.holds(&s.into()).unwrap());
        assert!(arc.holds(&e.into()).unwrap());
        assert_eq!((arc.start(), arc.end()), (s, e));
    }
}

#[test]
fn one_step_from_a_circle_is_decided_by_the_exact_path() {
    // A centre c, a radius 5k and the points of the circle at (3k, 4k) from it.
    let (c, k) = ([1_000_003_i64, -700_001], 200_000_i64);
    let centre: Rational2 = p2(c[0], c[1]).into();
    let on = p2(c[0] + 4 * k, c[1] + 3 * k);
    let round = Round::new(on, centre);
    let circle = Circle2::new(p2(c[0], c[1]), i128::from(5 * k)).unwrap();
    for [ux, uy] in [[3, 4], [-3, 4], [3, -4], [-3, -4], [4, 3], [5, 0], [0, -5]] {
        let z = p2(c[0] + ux * k, c[1] + uy * k);
        assert_eq!(round.side_of_grid(z).unwrap(), Sign::Zero);
        assert_eq!(round.side(&z.into()).unwrap(), Sign::Zero);
        assert_eq!(circle.side(&z.into()).unwrap(), Sign::Zero);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let moved = p2(c[0] + ux * k + dx, c[1] + uy * k + dy);
            let want = round_oracle(&round, &grid(moved));
            assert_ne!(want, Sign::Zero);
            assert_eq!(round.side_of_grid(moved).unwrap(), want);
            assert_eq!(round.side(&moved.into()).unwrap(), want);
            assert_eq!(circle.side(&moved.into()).unwrap(), want);
        }
    }
    // A rational centre: the circle through (0, 0) and (6, 0) about (3, 4 / 3).
    let centre = Rational2::new(9, 4, 3).unwrap();
    let arc = Arc2::new(p2(0, 0), p2(6, 0), centre).unwrap();
    assert_eq!(arc.round().side_of_grid(p2(3, 0)).unwrap(), Sign::Negative);
    assert_eq!(arc.round().side_of_grid(p2(0, 1)).unwrap(), Sign::Negative);
    assert_eq!(arc.round().side_of_grid(p2(0, 3)).unwrap(), Sign::Positive);
}

#[test]
fn tangency_is_exactly_zero_and_one_step_is_not() {
    // The line 3x + 4y = 5 · 5k + 3·c.x + 4·c.y touches the circle of radius 5k about c.
    let (c, k) = ([1_000_003_i128, -700_001], 200_000_i128);
    let centre = p2(c[0] as i64, c[1] as i64);
    let circle = Circle2::new(centre, 5 * k).unwrap();
    let round = Round::new(
        p2((c[0] + 3 * k) as i64, (c[1] + 4 * k) as i64),
        centre.into(),
    );
    let offset = 3 * c[0] + 4 * c[1] + 25 * k;
    let tangent = Carrier::new(3, 4, offset).unwrap();
    assert_eq!(circle.meets(&tangent).unwrap(), Contact::Tangent);
    assert_eq!(round.meets(&tangent).unwrap(), Contact::Tangent);
    let inside = Carrier::new(3, 4, offset - 1).unwrap();
    assert_eq!(circle.meets(&inside).unwrap(), Contact::Crossing);
    assert_eq!(round.meets(&inside).unwrap(), Contact::Crossing);
    let outside = Carrier::new(3, 4, offset + 1).unwrap();
    assert_eq!(circle.meets(&outside).unwrap(), Contact::Apart);
    assert_eq!(round.meets(&outside).unwrap(), Contact::Apart);
    // At a rational centre: the line y = 0 under the circle about (3, 4 / 3) through (3, 3 / 1)
    // is at distance 4 / 3 from a radius of 5 / 3, so it crosses; y = 3 touches it.
    let about = Rational2::new(9, 4, 3).unwrap();
    let r = Round::new(p2(3, 3), about);
    assert_eq!(
        r.meets(&Carrier::new(0, 1, 0).unwrap()).unwrap(),
        Contact::Crossing
    );
    assert_eq!(
        r.meets(&Carrier::new(0, 1, 3).unwrap()).unwrap(),
        Contact::Tangent
    );
    assert_eq!(
        r.meets(&Carrier::new(0, 1, 4).unwrap()).unwrap(),
        Contact::Apart
    );
}

#[test]
fn order_along_an_arc_agrees_with_walking_it() {
    // Points of the circle of radius 65k about the origin, from its Pythagorean triples.
    let triples = [
        [65, 0],
        [63, 16],
        [60, 25],
        [56, 33],
        [52, 39],
        [39, 52],
        [33, 56],
        [25, 60],
        [16, 63],
        [0, 65],
        [-16, 63],
        [-25, 60],
        [-33, 56],
        [-39, 52],
        [-52, 39],
        [-56, 33],
        [-60, 25],
        [-63, 16],
        [-65, 0],
        [-63, -16],
        [-60, -25],
        [-56, -33],
        [-52, -39],
        [-39, -52],
        [-33, -56],
        [-25, -60],
        [-16, -63],
        [0, -65],
        [16, -63],
        [25, -60],
        [33, -56],
        [39, -52],
        [52, -39],
        [56, -33],
        [60, -25],
        [63, -16],
    ];
    let k = 1_000;
    let at = |t: [i64; 2]| p2(t[0] * k, t[1] * k);
    let centre: Rational2 = p2(0, 0).into();
    let count = triples.len();
    for start in 0..count {
        for sweep in [3, 9, 18, 25, 35] {
            let end = (start + sweep) % count;
            let arc = Arc2::new(at(triples[start]), at(triples[end]), centre).unwrap();
            for i in 0..count {
                let p = at(triples[(start + i) % count]);
                assert_eq!(
                    arc.holds(&p.into()).unwrap(),
                    i <= sweep,
                    "{start} {sweep} {i}"
                );
                if i > sweep {
                    continue;
                }
                for j in 0..=sweep {
                    let q = at(triples[(start + j) % count]);
                    let want = if i == 0 || j == 0 || i == j {
                        Sign::Zero
                    } else if i < j {
                        Sign::Positive
                    } else {
                        Sign::Negative
                    };
                    assert_eq!(arc.order(&p.into(), &q.into()).unwrap(), want);
                }
            }
        }
    }
}

#[test]
fn a_pair_of_circles_is_what_their_radii_say() {
    let c = |x: i64, y: i64, r: i128| Circle2::new(p2(x, y), r).unwrap();
    let a = c(0, 0, 5);
    assert_eq!(a.against(&c(30, 40, 44)).unwrap(), Pair::Apart);
    assert_eq!(a.against(&c(30, 40, 45)).unwrap(), Pair::TouchingOutside);
    assert_eq!(a.against(&c(30, 40, 46)).unwrap(), Pair::Crossing);
    assert_eq!(a.against(&c(30, 40, 54)).unwrap(), Pair::Crossing);
    assert_eq!(a.against(&c(30, 40, 55)).unwrap(), Pair::TouchingInside);
    assert_eq!(a.against(&c(30, 40, 56)).unwrap(), Pair::Nested);
    assert_eq!(c(30, 40, 56).against(&a).unwrap(), Pair::Nested);
    assert_eq!(c(30, 40, 55).against(&a).unwrap(), Pair::TouchingInside);
    assert_eq!(a.against(&c(0, 0, 5)).unwrap(), Pair::Same);
    assert_eq!(a.against(&c(0, 0, 6)).unwrap(), Pair::Nested);
    assert_eq!(a.against(&c(3, 0, 2)).unwrap(), Pair::TouchingInside);
    assert_eq!(a.against(&c(7, 0, 2)).unwrap(), Pair::TouchingOutside);
    assert_eq!(c(1, 0, 3).against(&c(0, 0, 3)).unwrap(), Pair::Crossing);
    // The grid's far corners and the largest radii.
    let far = c(M, M, R).against(&c(-M, -M, R)).unwrap();
    assert_eq!(far, Pair::Crossing);
}

/// The filter's answer, the module's, and the oracle's, as a value grows from a step past zero.
fn crossed(family: impl Fn(u32) -> (Option<Sign>, Sign, Sign)) {
    let (mut inside, mut outside) = (0, 0);
    for step in 0..40 {
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

/// A far circle with the rational centre (3 / 7 away from a grid point), and a grid point on it.
fn far_round() -> (P2, Rational2) {
    let on = p2(1 << 38, -(1 << 37));
    let (x, y) = (i128::from(on.x.get()), i128::from(on.y.get()));
    (on, Rational2::new(7 * x + 3, 7 * y - 2, 7).unwrap())
}

#[test]
fn round_filters_cross_their_boundary_both_ways() {
    let (on, centre) = far_round();
    let round = Round::new(on, centre);
    let ([cx, cy], w) = centre.coordinates();
    crossed(|step| {
        let z = p2(on.x.get() + (1 << step), on.y.get());
        (
            round_grid::filter(z, on, cx, cy, w),
            round.side_of_grid(z).unwrap(),
            round_oracle(&round, &grid(z)),
        )
    });
    crossed(|step| {
        let q: Rational2 = p2(on.x.get() + (1 << step), on.y.get()).into();
        let ([x, y], v) = q.coordinates();
        (
            round_rational::filter(x, y, v, on, cx, cy, w),
            round.side(&q).unwrap(),
            round_oracle(&round, &fractions(&q)),
        )
    });
    crossed(|step| {
        let l = Carrier::new(1, 0, i128::from(on.x.get()) + (1 << step)).unwrap();
        let ([a, b], c) = l.coefficients();
        (
            round_line::filter(a, b, c, on, cx, cy, w),
            match round.meets(&l).unwrap() {
                Contact::Apart => Sign::Positive,
                Contact::Tangent => Sign::Zero,
                Contact::Crossing => Sign::Negative,
            },
            match round_contact_oracle(&round, &l) {
                Contact::Apart => Sign::Positive,
                Contact::Tangent => Sign::Zero,
                Contact::Crossing => Sign::Negative,
            },
        )
    });
}

#[test]
fn circle_filters_cross_their_boundary_both_ways() {
    // A tiny circle a long way out, so that a step is small beside the coordinates.
    let centre = p2(1 << 38, -(1 << 37));
    let radius = 1_i128 << 9;
    let circle = Circle2::new(centre, radius).unwrap();
    let on = p2(centre.x.get() + (1 << 9), centre.y.get());
    crossed(|step| {
        let q: Rational2 = p2(on.x.get() + (1 << step), on.y.get()).into();
        let ([x, y], v) = q.coordinates();
        (
            circle_side::filter(x, y, v, centre, radius),
            circle.side(&q).unwrap(),
            circle_oracle(&circle, &fractions(&q)),
        )
    });
    let as_sign = |contact: Contact| match contact {
        Contact::Apart => Sign::Positive,
        Contact::Tangent => Sign::Zero,
        Contact::Crossing => Sign::Negative,
    };
    crossed(|step| {
        let l = Carrier::new(1, 0, i128::from(on.x.get()) + (1 << step)).unwrap();
        let ([a, b], c) = l.coefficients();
        (
            circle_line::filter(a, b, c, centre, radius),
            as_sign(circle.meets(&l).unwrap()),
            as_sign(circle_contact_oracle(&circle, &l)),
        )
    });
    // 2^(2 step) + k² − k², beside squares of 2^78.
    let k = 1_i64 << 39;
    crossed(|step| {
        let other = p2(1 << step, k);
        (
            circle_gap::filter(p2(0, 0), other, i128::from(k)),
            circle_gap::exact(p2(0, 0), other, i128::from(k)).unwrap(),
            Sign::Positive,
        )
    });
}

#[test]
fn the_budget_and_the_degenerate_inputs_are_refused() {
    let o = p2(0, 0);
    assert_eq!(
        Circle2::new(o, R + 1).unwrap_err(),
        NumError::BudgetExceeded
    );
    assert_eq!(
        Circle2::new(o, -R - 1).unwrap_err(),
        NumError::BudgetExceeded
    );
    assert_eq!(Circle2::new(o, 0).unwrap_err(), NumError::Degenerate);
    assert_eq!(Circle2::new(o, -3).unwrap_err(), NumError::Degenerate);
    assert!(Circle2::new(o, R).is_ok());
    assert!(Circle2::new(o, 1).is_ok());
    let c = Rational2::new(9, 4, 3).unwrap();
    assert_eq!(Arc2::new(o, o, c).unwrap_err(), NumError::Degenerate);
    // A centre off the bisector, by the least that can be.
    let off = Rational2::new(9, 4, 3).unwrap();
    assert_eq!(
        Arc2::new(o, p2(6, 1), off).unwrap_err(),
        NumError::Degenerate
    );
    assert_eq!(Arc2::bisector(o, o).unwrap_err(), NumError::Degenerate);
    assert_eq!(
        Arc2::bisector(p2(-M, 0), p2(M, 1)).unwrap_err(),
        NumError::BudgetExceeded
    );
    // Halved where it can be: |q|^2 − |p|^2 even.
    let halved = Arc2::bisector(o, p2(2, 0)).unwrap();
    assert_eq!(halved.coefficients(), ([2, 0], 2));
    let whole = Arc2::bisector(o, p2(1, 0)).unwrap();
    assert_eq!(whole.coefficients(), ([2, 0], 1));
    // A round keeps what it was made from.
    let r = Round::new(p2(1, 2), c);
    assert_eq!(r.on(), p2(1, 2));
    assert_eq!(r.centre().coordinates(), ([9, 4], 3));
    let d = Circle2::new(p2(1, 2), 3).unwrap();
    assert_eq!((d.centre(), d.radius()), (p2(1, 2), 3));
}

#[test]
fn circles_keep_their_answers_under_a_move_and_any_weight() {
    let mut rng = Rng::new();
    for _ in 0..cases(20_000) {
        // Small enough that a move stays in the budget.
        let draw =
            |rng: &mut Rng, reach: u64| ((rng.next() % (2 * reach + 1)) as i64) - reach as i64;
        let (x, y) = (draw(&mut rng, 1 << 30), draw(&mut rng, 1 << 30));
        let (dx, dy) = (draw(&mut rng, 1 << 30), draw(&mut rng, 1 << 30));
        let on = p2(x, y);
        let (cx, cy, w) = (
            rng.signed(90),
            rng.signed(90),
            i128::from(rng.next() % (1 << 30)) + 1,
        );
        let z = p2(draw(&mut rng, 1 << 30), draw(&mut rng, 1 << 30));
        let round = Round::new(on, Rational2::new(cx, cy, w).unwrap());
        let shift = |p: P2| p2(p.x.get() + dx, p.y.get() + dy);
        let moved = Round::new(
            shift(on),
            Rational2::new(cx + i128::from(dx) * w, cy + i128::from(dy) * w, w).unwrap(),
        );
        assert_eq!(
            round.side_of_grid(z).unwrap(),
            moved.side_of_grid(shift(z)).unwrap()
        );
        // A place at another weight is one place.
        let heavier = Round::new(on, Rational2::new(3 * cx, 3 * cy, 3 * w).unwrap());
        assert_eq!(
            round.side_of_grid(z).unwrap(),
            heavier.side_of_grid(z).unwrap()
        );
        let point =
            Rational2::new(i128::from(z.x.get()) * 5, i128::from(z.y.get()) * 5, 5).unwrap();
        assert_eq!(round.side(&point).unwrap(), round.side_of_grid(z).unwrap());
        // A grid circle is one with its centre at weight 1 and a point at its radius.
        let circle = Circle2::new(on, i128::from(dx.abs() + 1)).unwrap();
        let round = Round::new(p2(x + dx.abs() + 1, y), on.into());
        assert_eq!(
            circle.side(&z.into()).unwrap(),
            round.side_of_grid(z).unwrap()
        );
        let l = rng.carrier();
        assert_eq!(circle.meets(&l).unwrap(), round.meets(&l).unwrap());
    }
}

#[test]
fn a_chord_puts_the_centre_on_the_side_its_sweep_says() {
    // 15.10's side of an arc's chord: the minor arc has its centre to the left of its chord.
    let (s, e) = (p2(5, 0), p2(0, 5));
    let centre: Rational2 = p2(0, 0).into();
    let minor = Arc2::new(s, e, centre).unwrap();
    let chord = Carrier::through(minor.start(), minor.end()).unwrap();
    assert_eq!(chord.side(&centre).unwrap(), Sign::Positive);
    assert!(minor.holds(&p2(4, 3).into()).unwrap());
    assert!(!minor.holds(&p2(-5, 0).into()).unwrap());
    let major = Arc2::new(e, s, centre).unwrap();
    let chord = Carrier::through(major.start(), major.end()).unwrap();
    assert_eq!(chord.side(&centre).unwrap(), Sign::Negative);
    assert!(!major.holds(&p2(4, 3).into()).unwrap());
    assert!(major.holds(&p2(-5, 0).into()).unwrap());
}
