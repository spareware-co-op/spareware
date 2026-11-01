//! Attacks on the numbers of piece G193: [`Disc`], [`FineLine`] and what `Crossing2` and `Conic` do
//! with them (chapters 14.2 and 14.3 of the kernel architecture). The tangent constructions on them
//! are attacked in `geom-2d`'s own `tests/attack_g193.rs`.
//!
//! - **A disc is the circle it says.** Every question agrees with a big-integer oracle, and with
//!   the [`Round`] or [`Circle2`] it was made from, at the budget's extremes and at random.
//! - **Every crossing is on both its curves**, as the exact questions read it back.
//! - **A gap is a bound**, zero exactly at a tangency.
//! - **A translated equation is the same curve.**
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17). Case counts are a twentieth in debug
//! builds.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "the formulas' own names, as in the module"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use dashu_int::IBig;
use geom_num::arc::{Circle2, Contact, Disc, Round, SQUARED_BITS};
use geom_num::carrier::{
    Carrier, FINE_NORMAL_BITS, FINE_OFFSET_BITS, FineLine, POINT_BITS, Rational2, WEIGHT_BITS,
};
use geom_num::conic::{Conic, Ellipse2, Meeting};
use geom_num::grid::{COORD_MAX, P2};
use geom_num::one_root::{Crossing2, Root, enclose};
use geom_num::quartic::Big;
use geom_num::{I256, NumError, Sign};

/// The default seed of every randomised test here (G193).
const DEFAULT_SEED: u64 = 0x6719_3000_5eed_1193;

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
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A value of at most `bits` bits, of a random size, and a random sign.
    fn sized(&mut self, bits: u32) -> i128 {
        let size = 1 + (self.next() % u64::from(bits)) as u32;
        let raw = (u128::from(self.next()) << 64 | u128::from(self.next())) >> (128 - size);
        let v = raw as i128;
        if self.next() & 1 == 0 { v } else { -v }
    }
}

fn cases() -> u64 {
    if cfg!(debug_assertions) { 400 } else { 8_000 }
}

fn big(v: i128) -> IBig {
    IBig::from(v)
}

fn sign_of(v: &IBig) -> Sign {
    match v.cmp(&IBig::ZERO) {
        core::cmp::Ordering::Greater => Sign::Positive,
        core::cmp::Ordering::Less => Sign::Negative,
        core::cmp::Ordering::Equal => Sign::Zero,
    }
}

fn wide(v: &I256) -> IBig {
    let mut out = IBig::ZERO;
    for limb in v.to_le_limbs().iter().rev() {
        out = (out << 64) + IBig::from(*limb);
    }
    out
}

/// The largest in-budget magnitude of each kind.
const TOP_POINT: i128 = (1 << POINT_BITS) - 1;
const TOP_WEIGHT: i128 = (1 << WEIGHT_BITS) - 1;

fn top_rr() -> I256 {
    I256::from_le_limbs([
        u64::MAX,
        u64::MAX,
        u64::MAX,
        (1 << (SQUARED_BITS - 192)) - 1,
    ])
}

fn p2(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

/// The sign `Disc::side` should give.
fn side_oracle(d: &Disc, z: &Rational2) -> Sign {
    let ([x, y], v) = z.coordinates();
    let ([cx, cy], w) = d.centre().coordinates();
    let dx = big(x) * big(w) - big(cx) * big(v);
    let dy = big(y) * big(w) - big(cy) * big(v);
    let rr = wide(&d.squared_radius());
    sign_of(&(&dx * &dx + &dy * &dy - rr * big(v) * big(v)))
}

/// The contact `Disc::meets` should give.
fn meets_oracle(d: &Disc, line: ([i128; 2], i128)) -> Contact {
    let ([a, b], c) = line;
    let ([x, y], w) = d.centre().coordinates();
    let h = big(a) * big(x) + big(b) * big(y) - big(c) * big(w);
    let n = big(a) * big(a) + big(b) * big(b);
    match sign_of(&(&h * &h - wide(&d.squared_radius()) * n)) {
        Sign::Positive => Contact::Apart,
        Sign::Zero => Contact::Tangent,
        Sign::Negative => Contact::Crossing,
    }
}

/// A disc of any size in the budget.
fn random_disc(rng: &mut Rng) -> Disc {
    loop {
        let w = rng.sized(WEIGHT_BITS).abs().max(1);
        let centre = Rational2::new(rng.sized(POINT_BITS), rng.sized(POINT_BITS), w).unwrap();
        let r = rng.sized(120);
        let squared = I256::from_i128(r).checked_mul(&I256::from_i128(r)).unwrap();
        if let Ok(d) = Disc::new(centre, squared) {
            return d;
        }
    }
}

#[test]
fn a_disc_is_made_only_within_its_budget() {
    let centre = Rational2::new(3, 4, 1).unwrap();
    assert_eq!(
        Disc::new(centre, I256::from_i64(0)).err(),
        Some(NumError::Degenerate)
    );
    assert_eq!(
        Disc::new(centre, I256::from_i64(-1)).err(),
        Some(NumError::Degenerate)
    );
    assert!(Disc::new(centre, top_rr()).is_ok());
    let over = top_rr().checked_add(&I256::from_i64(1)).unwrap();
    assert_eq!(
        Disc::new(centre, over).err(),
        Some(NumError::BudgetExceeded)
    );
}

#[test]
fn a_disc_answers_as_the_oracle_does_at_the_extremes() {
    let z = Rational2::new(TOP_POINT, -TOP_POINT, TOP_WEIGHT).unwrap();
    let centre = Rational2::new(-TOP_POINT, TOP_POINT, TOP_WEIGHT).unwrap();
    let d = Disc::new(centre, top_rr()).unwrap();
    assert_eq!(d.side(&z).unwrap(), side_oracle(&d, &z));
    let far = Rational2::new(TOP_POINT, TOP_POINT, 1).unwrap();
    assert_eq!(d.side(&far).unwrap(), side_oracle(&d, &far));
    let top = (1 << 41) - 1;
    for c in [(1_i128 << 82) - 1, -(1 << 82) + 1] {
        let l = Carrier::new(top, -top, c).unwrap();
        assert_eq!(d.meets(&l).unwrap(), meets_oracle(&d, l.coefficients()));
    }
    let fine_top = (1_i128 << FINE_NORMAL_BITS) - 1;
    let fine_c = (1_i128 << FINE_OFFSET_BITS) - 1;
    let l = FineLine::new(fine_top, -fine_top, fine_c).unwrap();
    assert_eq!(
        d.meets_fine(&l).unwrap(),
        meets_oracle(&d, l.coefficients())
    );
}

#[test]
fn a_disc_answers_as_the_oracle_does_at_random() {
    let mut rng = Rng::new();
    for _ in 0..cases() {
        let d = random_disc(&mut rng);
        let v = rng.sized(WEIGHT_BITS).abs().max(1);
        let z = Rational2::new(rng.sized(POINT_BITS), rng.sized(POINT_BITS), v).unwrap();
        assert_eq!(d.side(&z).unwrap(), side_oracle(&d, &z));
        let l = Carrier::new(rng.sized(41), rng.sized(41).abs().max(1), rng.sized(82)).unwrap();
        assert_eq!(d.meets(&l).unwrap(), meets_oracle(&d, l.coefficients()));
        let m = FineLine::new(rng.sized(75), rng.sized(75).abs().max(1), rng.sized(116)).unwrap();
        assert_eq!(
            d.meets_fine(&m).unwrap(),
            meets_oracle(&d, m.coefficients())
        );
    }
}

#[test]
fn a_disc_on_the_circle_exactly_is_zero_and_one_step_off_is_not() {
    // (3, 4) on the circle of radius 5 about the origin, over any weight.
    for w in [1_i128, 2, 1 << 40, TOP_WEIGHT] {
        let r = I256::from_i128(5 * w);
        let d = Disc::new(Rational2::new(0, 0, w).unwrap(), r.checked_mul(&r).unwrap()).unwrap();
        assert_eq!(d.side(&p2(3, 4).into()).unwrap(), Sign::Zero);
        assert_eq!(d.side(&p2(3, 3).into()).unwrap(), Sign::Negative);
        assert_eq!(d.side(&p2(4, 4).into()).unwrap(), Sign::Positive);
        let (tangent, near) = (
            Carrier::new(3, 4, 25).unwrap(),
            Carrier::new(3, 4, 24).unwrap(),
        );
        assert_eq!(d.meets(&tangent).unwrap(), Contact::Tangent);
        assert_eq!(d.meets(&near).unwrap(), Contact::Crossing);
        assert_eq!(
            d.meets(&Carrier::new(3, 4, 26).unwrap()).unwrap(),
            Contact::Apart
        );
    }
}

#[test]
fn a_round_and_a_grid_circle_are_the_discs_they_say() {
    let mut rng = Rng::new();
    for _ in 0..cases() {
        let on = p2(
            rng.sized(40).clamp(-1 << 39, 1 << 39) as i64,
            rng.sized(40).clamp(-1 << 39, 1 << 39) as i64,
        );
        let w = rng.sized(WEIGHT_BITS).abs().max(1);
        let centre = Rational2::new(rng.sized(POINT_BITS), rng.sized(POINT_BITS), w).unwrap();
        let round = Round::new(on, centre);
        let disc = Disc::of_round(&round).unwrap();
        let z = Rational2::new(rng.sized(POINT_BITS), rng.sized(POINT_BITS), 1).unwrap();
        assert_eq!(disc.side(&z).unwrap(), round.side(&z).unwrap());
        assert_eq!(disc.side(&on.into()).unwrap(), Sign::Zero);
        let l = Carrier::new(rng.sized(41), rng.sized(41).abs().max(1), rng.sized(82)).unwrap();
        assert_eq!(disc.meets(&l).unwrap(), round.meets(&l).unwrap());
        let circle = Circle2::new(on, rng.sized(41).abs().max(1)).unwrap();
        let disc = Disc::of_circle(&circle).unwrap();
        assert_eq!(disc.side(&z).unwrap(), circle.side(&z).unwrap());
        assert_eq!(disc.meets(&l).unwrap(), circle.meets(&l).unwrap());
    }
}

/// A line through a point of the disc's circle's inside: `(a, b)`, at a distance below the radius.
fn crossing_line(d: &Disc, rng: &mut Rng) -> ([i128; 2], i128) {
    let ([x, y], w) = d.centre().coordinates();
    let (a, b) = (rng.sized(30).abs().max(1), rng.sized(30));
    ([a, b], (a * (x / w) + b * (y / w)) + 1)
}

#[test]
fn a_crossing_of_a_disc_is_on_both() {
    let mut rng = Rng::new();
    let mut made = 0;
    for _ in 0..cases() {
        let w = 1 << (rng.next() % 40);
        let cx = rng.sized(80);
        let cy = rng.sized(80);
        let centre = Rational2::new(cx, cy, w).unwrap();
        let r = I256::from_i128(w * (1 + rng.sized(30).abs()));
        let disc = Disc::new(centre, r.checked_mul(&r).unwrap()).unwrap();
        let ([a, b], c) = crossing_line(&disc, &mut rng);
        let Ok(l) = Carrier::new(a, b, c) else {
            continue;
        };
        let m = FineLine::new(a << 20, b << 20, c << 20).unwrap();
        for root in [Root::First, Root::Second] {
            let Ok(z) = Crossing2::of_disc(&l, &disc, root) else {
                continue;
            };
            made += 1;
            assert_eq!(z.side(&l).unwrap(), Sign::Zero);
            assert_eq!(z.against_disc(&disc).unwrap(), Sign::Zero);
            let fine = Crossing2::of_fine(&m, &disc, root).unwrap();
            assert!(fine.same(&z).unwrap());
            assert_eq!(fine.side_of_fine(&m).unwrap(), Sign::Zero);
            assert_eq!(fine.against_disc(&disc).unwrap(), Sign::Zero);
        }
    }
    assert!(made > 100, "{made}");
}

#[test]
fn a_crossing_of_a_disc_is_the_crossing_of_its_round() {
    let centre = Rational2::new(7, 11, 3).unwrap();
    let round = Round::new(p2(40, 2), centre);
    let disc = Disc::of_round(&round).unwrap();
    let other = Round::new(p2(-8, 30), Rational2::new(50, -9, 7).unwrap());
    let l = Carrier::new(2, -3, 5).unwrap();
    for root in [Root::First, Root::Second] {
        let a = Crossing2::of_round(&l, &round, root).unwrap();
        let b = Crossing2::of_disc(&l, &disc, root).unwrap();
        assert!(a.same(&b).unwrap());
        assert_eq!(b.against_round(&round).unwrap(), Sign::Zero);
        let o = Disc::of_round(&other).unwrap();
        let c = Crossing2::of_rounds(&round, &other, root).unwrap();
        let d = Crossing2::of_discs(&disc, &o, root).unwrap();
        assert!(c.same(&d).unwrap());
        assert_eq!(d.against_disc(&disc).unwrap(), Sign::Zero);
        assert_eq!(d.against_disc(&o).unwrap(), Sign::Zero);
    }
}

#[test]
fn against_a_disc_says_which_side() {
    let unit = I256::from_i128(10);
    let disc = Disc::new(
        Rational2::new(0, 0, 1).unwrap(),
        unit.checked_mul(&unit).unwrap(),
    )
    .unwrap();
    let at = |x, y, w| Crossing2::of_point(&Rational2::new(x, y, w).unwrap()).unwrap();
    assert_eq!(at(3, 4, 1).against_disc(&disc).unwrap(), Sign::Negative);
    assert_eq!(at(6, 8, 1).against_disc(&disc).unwrap(), Sign::Zero);
    assert_eq!(at(61, 80, 10).against_disc(&disc).unwrap(), Sign::Positive);
}

#[test]
fn lines_past_a_carriers_budget_meet() {
    let (a, b) = (
        FineLine::new(1 << 70, 3, 5 << 70).unwrap(),
        FineLine::new(-3, 1 << 70, 7 << 70).unwrap(),
    );
    let z = Crossing2::of_lines(&a, &b).unwrap();
    assert_eq!(z.side_of_fine(&a).unwrap(), Sign::Zero);
    assert_eq!(z.side_of_fine(&b).unwrap(), Sign::Zero);
    // The other way round has a negative weight, turned.
    let w = Crossing2::of_lines(&b, &a).unwrap();
    assert!(w.same(&z).unwrap());
    let plain: FineLine = Carrier::new(1, 1, 3).unwrap().into();
    assert_eq!(
        Crossing2::of_lines(&plain, &plain).err(),
        Some(NumError::Degenerate)
    );
    let far = Crossing2::of_lines(&plain, &a).unwrap();
    assert_eq!(far.side_of_fine(&plain).unwrap(), Sign::Zero);
}

#[test]
fn a_fine_line_is_refused_past_its_budget() {
    let top = (1_i128 << FINE_NORMAL_BITS) - 1;
    assert!(FineLine::new(top, -top, 0).is_ok());
    assert_eq!(
        FineLine::new(top + 1, 0, 0).err(),
        Some(NumError::BudgetExceeded)
    );
    assert_eq!(
        FineLine::new(0, -top - 1, 0).err(),
        Some(NumError::BudgetExceeded)
    );
    assert_eq!(
        FineLine::new(1, 1, 1 << FINE_OFFSET_BITS).err(),
        Some(NumError::BudgetExceeded)
    );
    assert_eq!(FineLine::new(0, 0, 1).err(), Some(NumError::Degenerate));
}

#[test]
fn a_fine_line_has_its_side() {
    let mut rng = Rng::new();
    for _ in 0..cases() {
        let l = FineLine::new(rng.sized(75), rng.sized(75).abs().max(1), rng.sized(116)).unwrap();
        let ([a, b], c) = l.coefficients();
        let w = rng.sized(WEIGHT_BITS).abs().max(1);
        let z = Rational2::new(rng.sized(POINT_BITS), rng.sized(POINT_BITS), w).unwrap();
        let ([x, y], _) = z.coordinates();
        let value = big(a) * big(x) + big(b) * big(y) - big(c) * big(w);
        assert_eq!(l.side(&z).unwrap(), sign_of(&value));
    }
    let on = FineLine::new(1 << 70, 1 << 70, 7 << 70).unwrap();
    assert_eq!(on.side(&p2(3, 4).into()).unwrap(), Sign::Zero);
    assert_eq!(on.side(&p2(3, 5).into()).unwrap(), Sign::Positive);
}

#[test]
fn a_fine_line_meets_an_ellipse_on_both() {
    let e = Ellipse2::new(p2(0, 0), [5_000, 0], 3, 5).unwrap();
    let l = FineLine::new(1 << 60, 3 << 60, 100 << 60).unwrap();
    for root in [Root::First, Root::Second] {
        let z = Crossing2::of_fine_and_ellipse(&l, &e, root).unwrap();
        assert_eq!(z.side_of_fine(&l).unwrap(), Sign::Zero);
        assert_eq!(z.against_ellipse(&e).unwrap(), Sign::Zero);
    }
}

#[test]
fn a_gap_is_exactly_zero_at_a_tangency_and_a_bound_otherwise() {
    let five = I256::from_i128(5);
    let d = Disc::new(
        Rational2::new(0, 0, 1).unwrap(),
        five.checked_mul(&five).unwrap(),
    )
    .unwrap();
    let tangent = Carrier::new(3, 4, 25).unwrap();
    assert_eq!(d.gap_to_carrier(&tangent).unwrap().get(), 0.0);
    assert_eq!(
        d.gap_to_carrier(&Carrier::new(3, 4, 26).unwrap()).err(),
        Some(NumError::Degenerate)
    );
    // Through the centre: a right angle, and 1.6 is the bound's looseness there.
    let through = Carrier::new(1, 0, 0).unwrap();
    let right = d.gap_to_carrier(&through).unwrap().get();
    assert!((1.59..=1.61).contains(&right), "{right}");
    // At distance 24/5 of the radius 5 the sine is 7/25, past the small angle.
    let near = d
        .gap_to_carrier(&Carrier::new(3, 4, 24).unwrap())
        .unwrap()
        .get();
    assert!(near >= 0.28, "{near}");
    // The same line over 2^40: the same bound, within the few steps of outward rounding that
    // enclosing its larger integers takes.
    let fine = FineLine::new(3 << 40, 4 << 40, 24 << 40).unwrap();
    let scaled = d.gap_to_fine(&fine).unwrap().get();
    assert!(
        (scaled - near).abs() <= 8.0 * f64::EPSILON * near,
        "{scaled} and {near}"
    );
    // Hair-thin, by a depth of 2^-69 of the radius: the sine is 2^-34, and the bound holds it.
    let w = 1_i128 << 72;
    let r = 5 * w;
    let thin = Disc::new(
        Rational2::new(0, 0, w).unwrap(),
        I256::from_i128(r).checked_mul(&I256::from_i128(r)).unwrap(),
    )
    .unwrap();
    let c = 25;
    let l = Carrier::new(3, 4, c).unwrap();
    assert_eq!(thin.gap_to_carrier(&l).unwrap().get(), 0.0);
    let off = Disc::new(
        Rational2::new(1 << 6, 0, w).unwrap(),
        I256::from_i128(r).checked_mul(&I256::from_i128(r)).unwrap(),
    )
    .unwrap();
    let small = off.gap_to_carrier(&l);
    assert!(small.is_err() || small.unwrap().get() < 1e-9);
}

#[test]
fn a_gap_between_circles_is_their_angle() {
    let square = |r: i128| I256::from_i128(r).checked_mul(&I256::from_i128(r)).unwrap();
    let disc = |x, r| Disc::new(Rational2::new(x, 0, 1).unwrap(), square(r)).unwrap();
    // Touching outside and inside.
    assert_eq!(disc(0, 3).gap_to_disc(&disc(5, 2)).unwrap().get(), 0.0);
    assert_eq!(disc(0, 5).gap_to_disc(&disc(2, 3)).unwrap().get(), 0.0);
    assert_eq!(
        disc(0, 1).gap_to_disc(&disc(5, 2)).err(),
        Some(NumError::Degenerate)
    );
    // Radii 3 and 4, 5 apart: they cross at a right angle.
    let right = disc(0, 3).gap_to_disc(&disc(5, 4)).unwrap().get();
    assert!((1.59..=1.61).contains(&right), "{right}");
    // The same over other weights.
    let w = 1_i128 << 40;
    let scaled = Disc::new(Rational2::new(0, 0, w).unwrap(), square(3 * w)).unwrap();
    let over = scaled.gap_to_disc(&disc(5, 4)).unwrap().get();
    assert!(
        (over - right).abs() <= 8.0 * f64::EPSILON * right,
        "{over} and {right}"
    );
    // A small angle is bounded, not lost. Radii 1000 and 1000, 1999 apart: Heron's rule gives
    // the sine at a crossing as 1999·√3999 / (2·10^6) = 0.063207..., the angle 0.063249..., and
    // the bound may stand up to the 1.6 times the sine that asin x ≤ 1.6 x allows.
    let near = disc(0, 1_000)
        .gap_to_disc(&disc(1_999, 1_000))
        .unwrap()
        .get();
    assert!((0.063_249..=1.6 * 0.063_208).contains(&near), "{near}");
}

/// The value of `conic` at the grid point `at`.
fn value(conic: &Conic, at: P2) -> IBig {
    let [xx, xy, yy, x, y, c] = conic.coefficients().map(|v| {
        let mut out = IBig::ZERO;
        let negative = v.is_negative();
        let magnitude = if negative {
            v.checked_neg().unwrap()
        } else {
            v
        };
        for limb in magnitude.to_le_limbs().iter().rev() {
            out = (out << 64) + IBig::from(*limb);
        }
        if negative { -out } else { out }
    });
    let o = conic.origin();
    let u = big(i128::from(at.x.get())) - big(i128::from(o.x.get()));
    let v = big(i128::from(at.y.get())) - big(i128::from(o.y.get()));
    xx * &u * &u + xy * &u * &v + yy * &v * &v + x * &u + y * &v + c
}

#[test]
fn an_equation_about_another_point_is_the_same_curve() {
    let mut rng = Rng::new();
    let cases = cases() / 8;
    let e = Ellipse2::new(p2(1_000, -2_000), [400, 300], 3, 5).unwrap();
    let conics = [
        Conic::of_ellipse(&e).unwrap(),
        Conic::of_carrier(&Carrier::new(3, 4, 25).unwrap()).unwrap(),
        Conic::of_fine(&FineLine::new(3 << 60, 4 << 60, 25 << 60).unwrap()).unwrap(),
        Conic::of_circle(&Circle2::new(p2(5, 6), 7).unwrap()).unwrap(),
        Conic::of_round(&Round::new(p2(1, 2), Rational2::new(7, 8, 3).unwrap())).unwrap(),
    ];
    let at = |rng: &mut Rng| {
        p2(
            (rng.next() % (1 << 40)) as i64 - (1 << 39),
            (rng.next() % (1 << 30)) as i64,
        )
    };
    for conic in &conics {
        for _ in 0..cases {
            let (there, point) = (at(&mut rng), at(&mut rng));
            let moved = conic.about(there).unwrap();
            assert_eq!(moved.origin(), there);
            assert_eq!(value(&moved, point), value(conic, point));
        }
    }
}

#[test]
fn a_discs_equation_is_the_disc() {
    let w = 7_i128;
    let square = |r: i128| I256::from_i128(r).checked_mul(&I256::from_i128(r)).unwrap();
    // Centre (10, 20) over a weight of 7, radius 13, so (15, 32) is on it.
    let disc = Disc::new(Rational2::new(10 * w, 20 * w, w).unwrap(), square(13 * w)).unwrap();
    let conic = Conic::of_disc(&disc).unwrap();
    assert_eq!(value(&conic, p2(15, 32)), IBig::ZERO);
    assert!(value(&conic, p2(10, 20)) < IBig::ZERO);
    assert!(value(&conic, p2(30, 32)) > IBig::ZERO);
    // A centre between grid points.
    let beside = Disc::new(Rational2::new(1, 1, 2).unwrap(), square(10)).unwrap();
    let conic = Conic::of_disc(&beside).unwrap();
    assert_eq!(conic.origin(), p2(0, 0));
    assert_eq!(sign_of(&value(&conic, p2(3, 2))), Sign::Negative);
    // Off the frame there is no grid point to write it about.
    let m = i128::from(COORD_MAX);
    let off = Disc::new(Rational2::new(m + 2, 0, 1).unwrap(), square(1)).unwrap();
    assert!(Conic::of_disc(&off).is_err());
}

#[test]
fn a_disc_meets_an_ellipse_at_the_angles_of_its_roots() {
    // The circle of radius 5 about the origin, over a weight, and an ellipse through (3, 4)...
    let w = 1_i128 << 40;
    let square = |r: i128| I256::from_i128(r).checked_mul(&I256::from_i128(r)).unwrap();
    let disc = Disc::new(Rational2::new(0, 0, w).unwrap(), square(5 * w)).unwrap();
    // ...of semi-axes 5 and 3 along x: it touches the circle at (±5, 0), twice each.
    let e = Ellipse2::new(p2(0, 0), [5, 0], 3, 5).unwrap();
    let Meeting::At { roots, .. } = e.meeting(&Conic::of_disc(&disc).unwrap()).unwrap() else {
        panic!()
    };
    assert_eq!(roots.count(), 2);
    let cosines: Vec<f64> = roots
        .iter()
        .map(|r| {
            let [c, s] = r.angle().unwrap();
            assert!(s.lo() <= 0.0 && 0.0 <= s.hi());
            c.lo()
        })
        .collect();
    assert_eq!(cosines.len(), 2);
    assert!(cosines.iter().any(|c| (c - 1.0).abs() < 1e-9));
    assert!(cosines.iter().any(|c| (c + 1.0).abs() < 1e-9));
    // Where a smaller circle crosses, each angle holds the point it names.
    let small = Disc::new(Rational2::new(0, 0, 1).unwrap(), square(4)).unwrap();
    let Meeting::At { roots, .. } = e.meeting(&Conic::of_disc(&small).unwrap()).unwrap() else {
        panic!()
    };
    assert_eq!(roots.count(), 4);
    // x = 5 cos θ, y = 3 sin θ on the circle of radius 4: x² = 175/16 and y² = 81/16, so
    // cos θ = ±√175 / 20 and sin θ = ±3/4. An angle is as wide as isolation left its root, here a
    // whole quarter of the line each, so it is read as a box: each holds exactly one of the four
    // crossings, and no two the same one.
    let (cosine, sine) = (0.661_437_827_766_147_7, 0.75);
    let crossings = [
        (cosine, sine),
        (-cosine, sine),
        (-cosine, -sine),
        (cosine, -sine),
    ];
    let mut held: Vec<usize> = roots
        .iter()
        .map(|root| {
            let [c, s] = root.angle().unwrap();
            let hits: Vec<usize> = (0..4)
                .filter(|&i| {
                    let (a, b) = crossings[i];
                    c.lo() <= a && a <= c.hi() && s.lo() <= b && b <= s.hi()
                })
                .collect();
            assert_eq!(hits.len(), 1, "{c:?} {s:?}");
            hits[0]
        })
        .collect();
    held.sort_unstable();
    held.dedup();
    assert_eq!(held.len(), 4);
}

#[test]
fn an_integer_is_enclosed() {
    let v: Big = Big::from_i128(-(1 << 100) + 1);
    let i = enclose(&v).unwrap();
    let two_100 = (1_u128 << 100) as f64;
    assert!(i.lo() <= -two_100 + 1.0 && -two_100 <= i.hi());
    let zero = enclose(&Big::from_i64(0)).unwrap();
    assert_eq!((zero.lo(), zero.hi()), (0.0, 0.0));
}

#[test]
fn a_disc_s_side_keeps_its_laws() {
    let mut rng = Rng::new();
    for _ in 0..cases() {
        let (w, v) = (rng.sized(20).abs().max(1), rng.sized(20).abs().max(1));
        let (cx, cy, x, y) = (rng.sized(60), rng.sized(60), rng.sized(60), rng.sized(60));
        let r = rng.sized(60);
        let rr = I256::from_i128(r).checked_mul(&I256::from_i128(r)).unwrap();
        let Ok(d) = Disc::new(Rational2::new(cx, cy, w).unwrap(), rr) else {
            continue;
        };
        let z = Rational2::new(x, y, v).unwrap();
        let sign = d.side(&z).unwrap();
        assert_eq!(sign, side_oracle(&d, &z));
        // Moved together by a grid vector: the centre by its weight, the point by its own.
        let (tx, ty) = (rng.sized(30), rng.sized(30));
        let moved = Disc::new(Rational2::new(cx + tx * w, cy + ty * w, w).unwrap(), rr).unwrap();
        let there = Rational2::new(x + tx * v, y + ty * v, v).unwrap();
        assert_eq!(moved.side(&there).unwrap(), sign);
        // The same point over a larger weight.
        let k = rng.sized(8).abs().max(1);
        let scaled = Rational2::new(x * k, y * k, v * k).unwrap();
        assert_eq!(d.side(&scaled).unwrap(), sign);
    }
}

#[test]
fn a_fine_line_s_side_is_exact_at_the_extremes() {
    let (top, offset) = (
        (1_i128 << FINE_NORMAL_BITS) - 1,
        (1_i128 << FINE_OFFSET_BITS) - 1,
    );
    let signs = [1_i128, -1];
    for sa in signs {
        for sb in signs {
            for sc in signs {
                let (a, b, c) = (sa * top, sb * top, sc * offset);
                let l = FineLine::new(a, b, c).unwrap();
                for sx in signs {
                    for sy in signs {
                        let (x, y) = (sx * TOP_POINT, sy * TOP_POINT);
                        let z = Rational2::new(x, y, TOP_WEIGHT).unwrap();
                        let value = big(a) * big(x) + big(b) * big(y) - big(c) * big(TOP_WEIGHT);
                        assert_eq!(l.side(&z).unwrap(), sign_of(&value));
                    }
                }
            }
        }
    }
}

#[test]
fn a_fine_line_s_side_keeps_its_laws() {
    let mut rng = Rng::new();
    for _ in 0..cases() {
        let (a, b, c) = (rng.sized(75), rng.sized(75).abs().max(1), rng.sized(116));
        let l = FineLine::new(a, b, c).unwrap();
        let w = rng.sized(WEIGHT_BITS - 8).abs().max(1);
        let (x, y) = (rng.sized(POINT_BITS - 8), rng.sized(POINT_BITS - 8));
        let z = Rational2::new(x, y, w).unwrap();
        let sign = l.side(&z).unwrap();
        // The line turned round: the other side.
        let turned = FineLine::new(-a, -b, -c).unwrap();
        assert_eq!(turned.side(&z).unwrap(), sign.mul(Sign::Negative));
        // The same point over a larger weight.
        let k = rng.sized(8).abs().max(1);
        let scaled = Rational2::new(x * k, y * k, w * k).unwrap();
        assert_eq!(l.side(&scaled).unwrap(), sign);
        // A carrier answers as the fine line it makes.
        let m = Carrier::new(rng.sized(41), rng.sized(41).abs().max(1), rng.sized(82)).unwrap();
        assert_eq!(FineLine::from(m).side(&z).unwrap(), m.side(&z).unwrap());
    }
}

/// A line through the inside of `disc`, as a carrier, and where it crosses the disc.
fn crossing_of(disc: &Disc, rng: &mut Rng) -> Option<(Carrier, Crossing2)> {
    let ([a, b], c) = crossing_line(disc, rng);
    let l = Carrier::new(a, b, c).ok()?;
    let root = if rng.next() & 1 == 0 {
        Root::First
    } else {
        Root::Second
    };
    Some((l, Crossing2::of_disc(&l, disc, root).ok()?))
}

/// A disc about a centre of up to 80 bits over a power-of-two weight.
fn small_disc(rng: &mut Rng) -> Disc {
    let w = 1 << (rng.next() % 40);
    let centre = Rational2::new(rng.sized(80), rng.sized(80), w).unwrap();
    let r = I256::from_i128(w * (1 + rng.sized(30).abs()));
    Disc::new(centre, r.checked_mul(&r).unwrap()).unwrap()
}

/// A round through a grid point.
fn random_round(rng: &mut Rng) -> Round {
    let on = p2(
        rng.sized(40).clamp(-1 << 39, 1 << 39) as i64,
        rng.sized(40).clamp(-1 << 39, 1 << 39) as i64,
    );
    let w = rng.sized(WEIGHT_BITS).abs().max(1);
    Round::new(
        on,
        Rational2::new(rng.sized(POINT_BITS), rng.sized(POINT_BITS), w).unwrap(),
    )
}

#[test]
fn crossings_read_discs_and_fine_lines_as_their_rounds_and_carriers() {
    let mut rng = Rng::new();
    let mut made = 0;
    for _ in 0..cases() {
        let Some((_, z)) = crossing_of(&small_disc(&mut rng), &mut rng) else {
            continue;
        };
        made += 1;
        let round = random_round(&mut rng);
        let disc = Disc::of_round(&round).unwrap();
        assert_eq!(
            z.against_disc(&disc).unwrap(),
            z.against_round(&round).unwrap()
        );
        let m = Carrier::new(rng.sized(41), rng.sized(41).abs().max(1), rng.sized(82)).unwrap();
        assert_eq!(
            z.side_of_fine(&FineLine::from(m)).unwrap(),
            z.side(&m).unwrap()
        );
    }
    assert!(made > 100, "{made}");
}

#[test]
fn one_step_from_a_disc_or_a_fine_line_is_decided_by_the_exact_path() {
    let mut rng = Rng::new();
    let mut made = 0;
    for _ in 0..cases() {
        let disc = small_disc(&mut rng);
        let Some((l, z)) = crossing_of(&disc, &mut rng) else {
            continue;
        };
        made += 1;
        // On the circle: a step of rr either way puts the point inside or outside.
        let rr = disc.squared_radius();
        let one = I256::from_i64(1);
        let wider = Disc::new(disc.centre(), rr.checked_add(&one).unwrap()).unwrap();
        assert_eq!(z.against_disc(&disc).unwrap(), Sign::Zero);
        assert_eq!(z.against_disc(&wider).unwrap(), Sign::Negative);
        // A squared radius of one has no narrower disc.
        if let Ok(narrower) = Disc::new(disc.centre(), rr.checked_sub(&one).unwrap()) {
            assert_eq!(z.against_disc(&narrower).unwrap(), Sign::Positive);
        }
        // On the line, which a fine line writes 2^20 times as large: one step of c either way.
        let ([a, b], c) = l.coefficients();
        let fine = |c: i128| FineLine::new(a << 20, b << 20, c).unwrap();
        assert_eq!(z.side_of_fine(&fine(c << 20)).unwrap(), Sign::Zero);
        assert_eq!(
            z.side_of_fine(&fine((c << 20) + 1)).unwrap(),
            Sign::Negative
        );
        assert_eq!(
            z.side_of_fine(&fine((c << 20) - 1)).unwrap(),
            Sign::Positive
        );
    }
    assert!(made > 100, "{made}");
}

#[test]
fn crossings_at_the_extremes_read_discs_and_fine_lines_exactly() {
    let centre = Rational2::new(-TOP_POINT, TOP_POINT, TOP_WEIGHT).unwrap();
    let disc = Disc::new(centre, top_rr()).unwrap();
    let (top, offset) = (
        (1_i128 << FINE_NORMAL_BITS) - 1,
        (1_i128 << FINE_OFFSET_BITS) - 1,
    );
    let signs = [1_i128, -1];
    for sx in signs {
        for sy in signs {
            let p = Rational2::new(sx * TOP_POINT, sy * TOP_POINT, TOP_WEIGHT).unwrap();
            let z = Crossing2::of_point(&p).unwrap();
            assert_eq!(z.against_disc(&disc).unwrap(), disc.side(&p).unwrap());
            for (sa, sc) in [(1, 1), (1, -1), (-1, 1)] {
                let l = FineLine::new(sa * top, top, sc * offset).unwrap();
                assert_eq!(z.side_of_fine(&l).unwrap(), l.side(&p).unwrap());
            }
        }
    }
}

#[test]
fn a_crossing_s_disc_and_fine_line_keep_their_laws() {
    let mut rng = Rng::new();
    let mut made = 0;
    for _ in 0..cases() {
        let disc = small_disc(&mut rng);
        let Some((_, z)) = crossing_of(&small_disc(&mut rng), &mut rng) else {
            continue;
        };
        made += 1;
        let sign = z.against_disc(&disc).unwrap();
        // The same circle over a weight three times as large.
        let ([x, y], w) = disc.centre().coordinates();
        let nine = I256::from_i64(9);
        let same = Disc::new(
            Rational2::new(3 * x, 3 * y, 3 * w).unwrap(),
            disc.squared_radius().checked_mul(&nine).unwrap(),
        )
        .unwrap();
        assert_eq!(z.against_disc(&same).unwrap(), sign);
        // A fine line turned round: the other side; scaled up: the same side.
        let (a, b, c) = (rng.sized(60), rng.sized(60).abs().max(1), rng.sized(100));
        let l = FineLine::new(a, b, c).unwrap();
        let side = z.side_of_fine(&l).unwrap();
        let turned = FineLine::new(-a, -b, -c).unwrap();
        assert_eq!(z.side_of_fine(&turned).unwrap(), side.mul(Sign::Negative));
        let larger = FineLine::new(a << 10, b << 10, c << 10).unwrap();
        assert_eq!(z.side_of_fine(&larger).unwrap(), side);
    }
    assert!(made > 100, "{made}");
}
