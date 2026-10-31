//! Attacks on piece G187: integer carriers, rational points and the predicates on them
//! (`geom_num::carrier`; chapters 14.2 and 15.10 of the kernel architecture). Every test tries to
//! break a claim of the piece's done-when or of the module's guarantees.
//!
//! What decides what is right is an oracle in `dashu` rationals that shares no formulation with the
//! module: each point is divided out to its fractions, and each question asked of those, where the
//! module and the generated predicates never divide. The turn of three points is (b − a) × (c − a)
//! there and a determinant of homogeneous rows here.
//!
//! - **Random inputs** at every scale up to each budget, against the oracle.
//! - **The extreme values**: every sign combination of every input at its budget's largest value,
//!   where 15.10's widths are reached; no step may fail and every sign must match the oracle.
//! - **One step from degenerate**: points made to lie exactly on a carrier, then a carrier moved
//!   one step, so the filter must refuse and the exact path decide.
//! - **Properties**: a meeting lies exactly on both its carriers; a carrier agrees with orient2d;
//!   order and turn change sign as they should; a point keeps its place at any weight.
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
use geom_num::carrier::{Carrier, NORMAL_BITS, OFFSET_BITS, POINT_BITS, Rational2, WEIGHT_BITS};
use geom_num::generated::{carrier_order, carrier_side, rational_cmp, rational_orient};
use geom_num::grid::{COORD_MAX, P2};
use geom_num::predicate::orient2d;
use geom_num::{NumError, Sign};

/// The largest magnitudes each budget allows.
const N: i128 = (1 << NORMAL_BITS) - 1;
const D: i128 = (1 << OFFSET_BITS) - 1;
const P: i128 = (1 << POINT_BITS) - 1;
const W: i128 = (1 << WEIGHT_BITS) - 1;

/// The default seed of every randomised test here (G187).
const DEFAULT_SEED: u64 = 0x6718_7000_5eed_0187;

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), written out as the other predicate tests do:
/// `geom-num` cannot depend on its own generator for its tests' inputs without testing it with
/// itself.
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

fn sign_of(v: &RBig) -> Sign {
    match v.partial_cmp(&RBig::ZERO) {
        Some(core::cmp::Ordering::Greater) => Sign::Positive,
        Some(core::cmp::Ordering::Less) => Sign::Negative,
        _ => Sign::Zero,
    }
}

/// A point's two fractions.
fn fractions(p: &Rational2) -> [RBig; 2] {
    let ([x, y], w) = p.coordinates();
    [big(x) / big(w), big(y) / big(w)]
}

fn side_oracle(l: &Carrier, p: &Rational2) -> Sign {
    let ([a, b], c) = l.coefficients();
    let [x, y] = fractions(p);
    sign_of(&(big(a) * x + big(b) * y - big(c)))
}

fn order_oracle(l: &Carrier, p: &Rational2, q: &Rational2) -> Sign {
    let [dx, dy] = l.direction();
    let ([px, py], [qx, qy]) = (fractions(p), fractions(q));
    sign_of(&((qx - px) * big(dx) + (qy - py) * big(dy)))
}

fn orient_oracle(a: &Rational2, b: &Rational2, c: &Rational2) -> Sign {
    let ([ax, ay], [bx, by], [cx, cy]) = (fractions(a), fractions(b), fractions(c));
    sign_of(&((&bx - &ax) * (&cy - &ay) - (&by - &ay) * (&cx - &ax)))
}

fn compare_oracle(p: &Rational2, q: &Rational2) -> [Sign; 2] {
    let ([px, py], [qx, qy]) = (fractions(p), fractions(q));
    [sign_of(&(px - qx)), sign_of(&(py - qy))]
}

/// Every answer of the module on these inputs against the oracle.
fn agrees(l: &Carrier, p: &Rational2, q: &Rational2, r: &Rational2) {
    assert_eq!(l.side(p).unwrap(), side_oracle(l, p));
    assert_eq!(l.order(p, q).unwrap(), order_oracle(l, p, q));
    assert_eq!(Rational2::orient(p, q, r).unwrap(), orient_oracle(p, q, r));
    let [x, y] = compare_oracle(p, q);
    assert_eq!(p.cmp_x(q).unwrap(), x);
    assert_eq!(p.cmp_y(q).unwrap(), y);
    assert_eq!(p.same(q).unwrap(), x == Sign::Zero && y == Sign::Zero);
}

#[test]
fn random_inputs_at_every_scale_match_the_oracle() {
    let mut rng = Rng::new();
    for _ in 0..cases(40_000) {
        let (l, p, q, r) = (rng.carrier(), rng.point(), rng.point(), rng.point());
        agrees(&l, &p, &q, &r);
    }
}

/// `tops` under every choice of signs.
fn corners<const N_: usize>(tops: [i128; N_]) -> impl Iterator<Item = [i128; N_]> {
    (0_u32..1 << N_).map(move |bits| {
        let mut k = 0;
        tops.map(|v| {
            k += 1;
            if bits >> (k - 1) & 1 == 0 { v } else { -v }
        })
    })
}

#[test]
fn the_extreme_values_reach_every_width_and_every_step_fits() {
    for [a, b] in corners([N, N]) {
        for c in [D, -D, 0] {
            let l = Carrier::new(a, b, c).unwrap();
            for [x, y, w] in corners([P, P, W]) {
                let p = Rational2::new(x, y, w).unwrap();
                assert_eq!(l.side(&p).unwrap(), side_oracle(&l, &p));
                let q = Rational2::new(-x, y, 1).unwrap();
                assert_eq!(l.order(&p, &q).unwrap(), order_oracle(&l, &p, &q));
                let [x_sign, y_sign] = compare_oracle(&p, &q);
                assert_eq!(p.cmp_x(&q).unwrap(), x_sign);
                assert_eq!(p.cmp_y(&q).unwrap(), y_sign);
            }
        }
    }
    for rows in corners([P, P, W, P, P, W, P, P, W]) {
        let [xa, ya, wa, xb, yb, wb, xc, yc, wc] = rows;
        let (p, q, r) = (
            Rational2::new(xa, ya, wa).unwrap(),
            Rational2::new(xb, yb, wb).unwrap(),
            Rational2::new(xc, yc, wc).unwrap(),
        );
        assert_eq!(
            Rational2::orient(&p, &q, &r).unwrap(),
            orient_oracle(&p, &q, &r)
        );
    }
    // Carriers at the extremes meet inside the point budget, exactly on both.
    let l = Carrier::new(-N, N, -D).unwrap();
    let m = Carrier::new(N, 1 - N, D).unwrap();
    let meeting = l.meet(&m).unwrap();
    assert_eq!(l.side(&meeting).unwrap(), Sign::Zero);
    assert_eq!(m.side(&meeting).unwrap(), Sign::Zero);
}

#[test]
fn the_largest_value_each_expression_can_take_is_inside_its_width() {
    // Every term its largest, all one way: the exact path holds what the filter refuses.
    let (a, b, c) = (N, N, -D);
    let (x, y, w) = (P, P, W);
    assert_eq!(carrier_side::exact(a, b, c, x, y, w), Some(Sign::Positive));
    assert_eq!(
        carrier_order::exact(N, -N, P, -P, -P, P, W, W),
        Some(Sign::Negative)
    );
    assert_eq!(rational_cmp::exact(P, -P, W, W), Some(Sign::Positive));
    assert_eq!(
        rational_orient::exact(P, P, -P, P, P, -P, W, W, W),
        Some(orient_oracle(
            &Rational2::new(P, P, W).unwrap(),
            &Rational2::new(-P, P, W).unwrap(),
            &Rational2::new(P, -P, W).unwrap(),
        ))
    );
    // One past a budget is outside each predicate's domain, never a wrapped answer.
    assert_eq!(carrier_side::exact(N + 1, b, c, x, y, w), None);
    assert_eq!(carrier_side::filter(a, b, c, P + 1, y, w), None);
    assert_eq!(rational_cmp::exact(P + 1, 0, 1, 1), None);
    assert_eq!(rational_orient::exact(0, 0, 0, 0, 0, 0, W + 1, 1, 1), None);
}

#[test]
fn a_meeting_lies_exactly_on_both_carriers() {
    let mut rng = Rng::new();
    let mut made = 0;
    for _ in 0..cases(40_000) {
        let (l, m) = (rng.carrier(), rng.carrier());
        match l.meet(&m) {
            Ok(p) => {
                assert_eq!(l.side(&p).unwrap(), Sign::Zero);
                assert_eq!(m.side(&p).unwrap(), Sign::Zero);
                let turned = m.meet(&l).unwrap();
                assert!(p.same(&turned).unwrap());
                assert!(p.coordinates().1 > 0);
                made += 1;
            }
            Err(e) => {
                assert_eq!(e, NumError::Degenerate);
                assert!(l.parallel(&m).unwrap());
            }
        }
    }
    assert!(made > 100);
}

#[test]
fn a_meeting_at_the_budget_stays_inside_it() {
    // The largest numerators and weight two carriers can make.
    let l = Carrier::new(N, N, D).unwrap();
    let m = Carrier::new(N, -N, -D).unwrap();
    let p = l.meet(&m).unwrap();
    let ([x, y], w) = p.coordinates();
    assert_eq!(l.side(&p).unwrap(), Sign::Zero);
    assert_eq!(m.side(&p).unwrap(), Sign::Zero);
    assert!(x.unsigned_abs() < 1 << POINT_BITS);
    assert!(y.unsigned_abs() < 1 << POINT_BITS);
    assert!(w.unsigned_abs() < 1 << WEIGHT_BITS);
    assert_eq!(w, 2 * N * N);
    assert_eq!(y, 2 * N * D);
}

#[test]
fn one_step_from_a_carrier_is_decided_by_the_exact_path() {
    let mut rng = Rng::new();
    let (mut filtered, mut refused) = (0, 0);
    for _ in 0..cases(40_000) {
        let (l, m) = (rng.carrier(), rng.carrier());
        let Ok(p) = l.meet(&m) else { continue };
        let ([a, b], c) = l.coefficients();
        let ([x, y], w) = p.coordinates();
        for (by, want) in [(1, Sign::Negative), (-1, Sign::Positive)] {
            let Ok(moved) = Carrier::new(a, b, c + by) else {
                continue;
            };
            // a X + b Y − (c + by) W = −by · W, and W is positive.
            assert_eq!(moved.side(&p).unwrap(), want);
            match carrier_side::filter(a, b, c + by, x, y, w) {
                Some(sign) => {
                    assert_eq!(sign, want);
                    filtered += 1;
                }
                None => refused += 1,
            }
        }
    }
    assert!(refused > 0, "the filter decided everything");
    println!("one step off: filter decided {filtered}, refused {refused}");
}

/// `l` with c moved by `by`.
fn shifted(l: &Carrier, by: i128) -> Carrier {
    let ([a, b], c) = l.coefficients();
    Carrier::new(a, b, c + by).unwrap()
}

#[test]
fn one_step_along_a_carrier_is_decided_by_the_exact_path() {
    // Two meetings on one carrier a fraction of a step apart, and a third off it by as little.
    let mut rng = Rng::new();
    for _ in 0..cases(20_000) {
        let (l, m, k) = (rng.carrier(), rng.carrier(), rng.carrier());
        let ([_, _], c) = m.coefficients();
        if c.unsigned_abs() + 1 >= 1 << OFFSET_BITS {
            continue;
        }
        let (Ok(p), Ok(q), Ok(r)) = (l.meet(&m), l.meet(&shifted(&m, 1)), l.meet(&k)) else {
            continue;
        };
        assert_eq!(l.order(&p, &q).unwrap(), order_oracle(&l, &p, &q));
        assert_eq!(p.cmp_x(&q).unwrap(), compare_oracle(&p, &q)[0]);
        assert_eq!(p.cmp_y(&q).unwrap(), compare_oracle(&p, &q)[1]);
        assert_eq!(Rational2::orient(&p, &q, &r).unwrap(), Sign::Zero);
        let ([_, _], lc) = l.coefficients();
        if lc.unsigned_abs() + 1 >= 1 << OFFSET_BITS {
            continue;
        }
        if let Ok(off) = shifted(&l, 1).meet(&k) {
            let turn = Rational2::orient(&p, &q, &off).unwrap();
            assert_eq!(turn, orient_oracle(&p, &q, &off));
        }
    }
}

/// Three carriers in general position within ±2^32 steps, fixed so each boundary test sweeps one
/// family.
fn three_carriers() -> [Carrier; 3] {
    let line = |p: [i64; 2], q: [i64; 2]| {
        Carrier::through(P2::new(p[0], p[1]).unwrap(), P2::new(q[0], q[1]).unwrap()).unwrap()
    };
    [
        line(
            [-1_000_000_007, 2_000_000_011],
            [3_000_000_019, -500_000_003],
        ),
        line(
            [-2_000_000_033, -1_000_000_021],
            [1_500_000_041, 2_500_000_047],
        ),
        line(
            [2_000_000_081, 3_000_000_077],
            [-3_500_000_099, 1_000_000_103],
        ),
    ]
}

/// Sweeps a family whose exact value grows from a step past zero: each member gives the filter's
/// answer, the module's and the oracle's. The filter must refuse the smallest members and decide the
/// largest, and every answer must be the oracle's.
fn crossed(family: impl Fn(u32) -> (Option<Sign>, Sign, Sign)) {
    let (mut inside, mut outside) = (0, 0);
    for step in 0..60 {
        let (filter, answer, oracle) = family(step);
        assert_eq!(answer, oracle, "step {step}");
        assert_ne!(oracle, Sign::Zero, "step {step}");
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
fn side_filter_boundary_crossed_both_ways() {
    let [l, m, _] = three_carriers();
    let p = l.meet(&m).unwrap();
    let ([x, y], w) = p.coordinates();
    crossed(|step| {
        let moved = shifted(&l, 1 << step);
        let ([a, b], c) = moved.coefficients();
        (
            carrier_side::filter(a, b, c, x, y, w),
            moved.side(&p).unwrap(),
            side_oracle(&moved, &p),
        )
    });
}

#[test]
fn order_filter_boundary_crossed_both_ways() {
    let [l, m, _] = three_carriers();
    let p = l.meet(&m).unwrap();
    let ([xp, yp], wp) = p.coordinates();
    let ([a, b], _) = l.coefficients();
    crossed(|step| {
        let q = l.meet(&shifted(&m, 1 << step)).unwrap();
        let ([xq, yq], wq) = q.coordinates();
        (
            carrier_order::filter(a, b, xp, yp, xq, yq, wp, wq),
            l.order(&p, &q).unwrap(),
            order_oracle(&l, &p, &q),
        )
    });
}

#[test]
fn cmp_filter_boundary_crossed_both_ways() {
    let [l, m, _] = three_carriers();
    let p = l.meet(&m).unwrap();
    let ([xp, yp], wp) = p.coordinates();
    crossed(|step| {
        let q = l.meet(&shifted(&m, 1 << step)).unwrap();
        let ([xq, _], wq) = q.coordinates();
        (
            rational_cmp::filter(xp, xq, wp, wq),
            p.cmp_x(&q).unwrap(),
            compare_oracle(&p, &q)[0],
        )
    });
    crossed(|step| {
        let q = l.meet(&shifted(&m, 1 << step)).unwrap();
        let ([_, yq], wq) = q.coordinates();
        (
            rational_cmp::filter(yp, yq, wp, wq),
            p.cmp_y(&q).unwrap(),
            compare_oracle(&p, &q)[1],
        )
    });
}

#[test]
fn orient_filter_boundary_crossed_both_ways() {
    let [l, m, k] = three_carriers();
    let (p, q) = (l.meet(&m).unwrap(), l.meet(&k).unwrap());
    let ([xa, ya], wa) = p.coordinates();
    let ([xb, yb], wb) = q.coordinates();
    crossed(|step| {
        let r = shifted(&l, 1 << step)
            .meet(&Carrier::new(5, 7, 11).unwrap())
            .unwrap();
        let ([xc, yc], wc) = r.coordinates();
        (
            rational_orient::filter(xa, ya, xb, yb, xc, yc, wa, wb, wc),
            Rational2::orient(&p, &q, &r).unwrap(),
            orient_oracle(&p, &q, &r),
        )
    });
}

#[test]
fn the_filters_never_contradict_the_exact_paths_and_both_outcomes_occur() {
    let mut rng = Rng::new();
    let mut decided = [0_u64; 4];
    for _ in 0..cases(100_000) {
        let (l, m, p, q, r) = (
            rng.carrier(),
            rng.carrier(),
            rng.point(),
            rng.point(),
            rng.point(),
        );
        let ([a, b], c) = l.coefficients();
        let ([xp, yp], wp) = p.coordinates();
        let ([xq, yq], wq) = q.coordinates();
        let ([xr, yr], wr) = r.coordinates();
        let meeting = l.meet(&m).ok();
        let on = meeting.map_or((xp, yp, wp), |k| {
            let ([x, y], w) = k.coordinates();
            (x, y, w)
        });
        let side = carrier_side::filter(a, b, c, on.0, on.1, on.2);
        let side_exact = carrier_side::exact(a, b, c, on.0, on.1, on.2);
        let order = carrier_order::filter(a, b, xp, yp, xq, yq, wp, wq);
        let order_exact = carrier_order::exact(a, b, xp, yp, xq, yq, wp, wq);
        let cmp = rational_cmp::filter(xp, xq, wp, wq);
        let cmp_exact = rational_cmp::exact(xp, xq, wp, wq);
        let turn = rational_orient::filter(xp, yp, xq, yq, xr, yr, wp, wq, wr);
        let turn_exact = rational_orient::exact(xp, yp, xq, yq, xr, yr, wp, wq, wr);
        for (k, (fast, sure)) in [
            (side, side_exact),
            (order, order_exact),
            (cmp, cmp_exact),
            (turn, turn_exact),
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(sign) = fast {
                assert_eq!(Some(sign), sure);
                decided[k] += 1;
            }
        }
    }
    for (k, n) in decided.iter().enumerate() {
        assert!(*n > 0, "filter {k} decided nothing");
    }
}

#[test]
fn a_carrier_through_grid_points_agrees_with_orient2d() {
    let mut rng = Rng::new();
    let grid = |rng: &mut Rng| {
        let mut c = || (rng.signed(40) as i64).clamp(-COORD_MAX, COORD_MAX);
        P2::new(c(), c()).unwrap()
    };
    for _ in 0..cases(40_000) {
        let (p, q, r) = (grid(&mut rng), grid(&mut rng), grid(&mut rng));
        let Ok(l) = Carrier::through(p, q) else {
            assert_eq!(p, q);
            continue;
        };
        assert_eq!(l.side(&r.into()).unwrap(), orient2d(p, q, r).unwrap());
        assert_eq!(l.side(&p.into()).unwrap(), Sign::Zero);
        assert_eq!(l.side(&q.into()).unwrap(), Sign::Zero);
        assert_eq!(
            Rational2::orient(&p.into(), &q.into(), &r.into()).unwrap(),
            orient2d(p, q, r).unwrap()
        );
        assert_eq!(l.order(&p.into(), &q.into()).unwrap(), Sign::Positive);
    }
    // The grid's corners reach the budget and stay in it.
    let m = COORD_MAX;
    let (p, q) = (P2::new(-m, -m).unwrap(), P2::new(m, m - 1).unwrap());
    let l = Carrier::through(p, q).unwrap();
    assert_eq!(l.side(&p.into()).unwrap(), Sign::Zero);
    assert_eq!(l.side(&q.into()).unwrap(), Sign::Zero);
}

#[test]
fn order_and_turn_change_sign_as_they_should() {
    let mut rng = Rng::new();
    for _ in 0..cases(20_000) {
        let (l, p, q, r) = (rng.carrier(), rng.point(), rng.point(), rng.point());
        assert_eq!(l.order(&p, &q).unwrap(), l.order(&q, &p).unwrap().negate());
        let turn = Rational2::orient(&p, &q, &r).unwrap();
        assert_eq!(Rational2::orient(&q, &r, &p).unwrap(), turn);
        assert_eq!(Rational2::orient(&q, &p, &r).unwrap(), turn.negate());
        assert_eq!(Rational2::orient(&p, &p, &r).unwrap(), Sign::Zero);
        assert_eq!(p.cmp_x(&q).unwrap(), q.cmp_x(&p).unwrap().negate());
        assert_eq!(p.cmp_y(&q).unwrap(), q.cmp_y(&p).unwrap().negate());
        // The carrier run the other way sees every side and order turned.
        let ([a, b], c) = l.coefficients();
        let back = Carrier::new(-a, -b, -c).unwrap();
        assert_eq!(back.side(&p).unwrap(), l.side(&p).unwrap().negate());
        assert_eq!(
            back.order(&p, &q).unwrap(),
            l.order(&p, &q).unwrap().negate()
        );
        assert!(back.same(&l).unwrap());
    }
}

#[test]
fn a_point_keeps_its_place_at_any_weight_and_a_line_at_any_scale() {
    let p = Rational2::new(3, -5, 7).unwrap();
    let q = Rational2::new(-30, 50, -70).unwrap();
    assert_eq!(q.coordinates(), ([30, -50], 70));
    assert!(p.same(&q).unwrap());
    assert!(!p.same(&Rational2::new(3, -5, 8).unwrap()).unwrap());
    assert!(!p.same(&Rational2::new(3, -4, 7).unwrap()).unwrap());
    assert!(!p.same(&Rational2::new(4, -5, 7).unwrap()).unwrap());
    let l = Carrier::new(2, 4, 6).unwrap();
    assert!(l.same(&Carrier::new(1, 2, 3).unwrap()).unwrap());
    assert!(l.same(&Carrier::new(-3, -6, -9).unwrap()).unwrap());
    assert!(l.parallel(&Carrier::new(1, 2, 4).unwrap()).unwrap());
    assert!(!l.same(&Carrier::new(1, 2, 4).unwrap()).unwrap());
    assert!(!l.same(&Carrier::new(2, 1, 3).unwrap()).unwrap());
    assert!(!l.parallel(&Carrier::new(2, 1, 3).unwrap()).unwrap());
    // A line through the origin differs from its parallels only by c.
    let o = Carrier::new(1, 0, 0).unwrap();
    assert!(!o.same(&Carrier::new(1, 0, 1).unwrap()).unwrap());
    assert!(
        !Carrier::new(0, 1, 0)
            .unwrap()
            .same(&Carrier::new(0, 1, 1).unwrap())
            .unwrap()
    );
    assert_eq!(l.direction(), [4, -2]);
    assert_eq!(l.coefficients(), ([2, 4], 6));
}

#[test]
fn the_budget_and_the_degenerate_inputs_are_refused() {
    let over = NumError::BudgetExceeded;
    for (a, b, c) in [
        (N + 1, 0, 0),
        (-N - 1, 0, 0),
        (0, N + 1, 0),
        (0, -N - 1, 0),
        (1, 0, D + 1),
        (1, 0, -D - 1),
    ] {
        assert_eq!(Carrier::new(a, b, c).unwrap_err(), over);
    }
    assert_eq!(Carrier::new(0, 0, 5).unwrap_err(), NumError::Degenerate);
    assert!(Carrier::new(N, -N, D).is_ok());
    assert!(Carrier::new(-N, N, -D).is_ok());
    assert!(Carrier::new(0, 1, 0).is_ok());
    assert!(Carrier::new(1, 0, 0).is_ok());
    for (x, y, w) in [
        (P + 1, 0, 1),
        (-P - 1, 0, 1),
        (0, P + 1, 1),
        (0, -P - 1, 1),
        (0, 0, W + 1),
        (0, 0, -W - 1),
    ] {
        assert_eq!(Rational2::new(x, y, w).unwrap_err(), over);
    }
    assert_eq!(Rational2::new(1, 2, 0).unwrap_err(), NumError::Degenerate);
    assert!(Rational2::new(P, -P, W).is_ok());
    assert_eq!(
        Rational2::new(-P, P, -W).unwrap().coordinates(),
        ([P, -P], W)
    );
    let p = P2::new(5, 7).unwrap();
    assert_eq!(Carrier::through(p, p).unwrap_err(), NumError::Degenerate);
    let l = Carrier::new(1, 2, 3).unwrap();
    assert_eq!(
        l.meet(&Carrier::new(2, 4, 7).unwrap()).unwrap_err(),
        NumError::Degenerate
    );
    assert_eq!(l.meet(&l).unwrap_err(), NumError::Degenerate);
}

#[test]
fn a_t_junction_on_a_sloping_wall_stays_on_its_one_carrier() {
    // 15.10's case: a wall from (0, 0) to (7, 3) and a wall ending on it, which no grid point can
    // hold. The junction is on the sloping wall exactly, and splits it into two pieces of one line.
    let origin = P2::new(0, 0).unwrap();
    let wall = Carrier::through(origin, P2::new(7, 3).unwrap()).unwrap();
    let stem = Carrier::through(P2::new(2, 5).unwrap(), P2::new(2, 0).unwrap()).unwrap();
    let junction = wall.meet(&stem).unwrap();
    assert_eq!(wall.side(&junction).unwrap(), Sign::Zero);
    assert_eq!(stem.side(&junction).unwrap(), Sign::Zero);
    assert!(!junction.same(&P2::new(2, 1).unwrap().into()).unwrap());
    assert_eq!(
        wall.order(&origin.into(), &junction).unwrap(),
        Sign::Positive
    );
    assert_eq!(
        wall.order(&junction, &P2::new(7, 3).unwrap().into())
            .unwrap(),
        Sign::Positive
    );
}
