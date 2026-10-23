//! Double-double arithmetic (`geom_num::dd`) against an exact oracle.
//!
//! Oracle: `dashu` rationals (`RBig`, exact for every finite double) and integers. Nothing here
//! calls the code under test to compute an expected value. Every operation is held to the bound
//! its documentation states, `|result − r| ≤ B·|r| + A`, with B the published, machine-checked
//! relative bound as an exact rational (plus 2^−398 for the multiplicative operations) and A the
//! absolute 2^−1074 where the module states one; and the public constants are checked to be at
//! least those B. Inputs: random over the normal range and over the whole range (subnormal leading
//! parts, trailing parts down to the smallest subnormal), cancellation, results at the edges of
//! overflow and underflow, trailing parts exactly half a unit (ties) and trailing parts small
//! enough to be dropped (below 2^−400 of the leading part). The published inputs that bring each
//! algorithm close to its bound are reproduced to the digits their papers give, which shows the
//! algorithm here is the published one.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::needless_pass_by_value
)]

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;
use dashu_ratio::ops::Abs;
use geom_num::dd::{
    ADD_BOUND, ADD_F64_BOUND, DIV_BOUND, DIV_F64_BOUND, MUL_BOUND, MUL_F64_BOUND, SQRT_BOUND,
    UNDERFLOW_BOUND, two_prod_fma,
};
use geom_num::eft::two_prod;
use geom_num::{DD, NumError, Sign};
use std::cmp::Ordering;

// ---------------------------------------------------------------------------------------------
// The oracle.

/// A finite double as an exact rational.
fn rat(x: f64) -> RBig {
    assert!(x.is_finite(), "rat of {x}");
    RBig::try_from(x).unwrap()
}

/// 2^k as an exact rational.
fn p2(k: i64) -> RBig {
    if k >= 0 {
        RBig::from(UBig::ONE << k as usize)
    } else {
        RBig::from_parts(IBig::ONE, UBig::ONE << (-k) as usize)
    }
}

/// n/d as an exact rational.
fn frac(n: i64, d: u64) -> RBig {
    RBig::from_parts(IBig::from(n), UBig::from(d))
}

/// The exact value of a `DD`.
fn value(d: DD) -> RBig {
    rat(d.hi()) + rat(d.lo())
}

/// u = 2^−53.
fn u() -> RBig {
    p2(-53)
}

/// The published relative bounds, exactly, and the module's extras.
struct Bounds {
    add: RBig,
    add_f64: RBig,
    mul_f64: RBig,
    mul: RBig,
    div_f64: RBig,
    div: RBig,
    sqrt: RBig,
}

fn bounds() -> Bounds {
    let u = u();
    let u2 = &u * &u;
    let u3 = &u2 * &u;
    let drop = p2(-398);
    let one = RBig::ONE;
    Bounds {
        // JMP Theorem 3.1: 3u²/(1 − 4u).
        add: RBig::from(3) * &u2 / (&one - RBig::from(4) * &u),
        // JMP Theorem 2.2: 2u².
        add_f64: RBig::from(2) * &u2,
        // JMP Theorem 4.3: 2u², plus the dropped trailing parts.
        mul_f64: RBig::from(2) * &u2 + &drop,
        // Muller and Rideau, Theorem 2.8: (4u² + u³/2)/(1 + u)².
        mul: (RBig::from(4) * &u2 + &u3 / RBig::from(2)) / ((&one + &u) * (&one + &u)) + &drop,
        // JMP Theorem 6.2: 3u².
        div_f64: RBig::from(3) * &u2 + &drop,
        // JMP Theorem 7.2: 9.8u².
        div: frac(49, 5) * &u2 + &drop,
        // Lefèvre et al., Theorem 3.6: 25u²/8.
        sqrt: frac(25, 8) * &u2 + &drop,
    }
}

/// (1 − 2^−52)·2^1024: below it, no operation may report an overflow.
fn overflow_floor() -> RBig {
    (RBig::ONE - p2(-52)) * p2(1024)
}

/// The invariant: finite parts, `hi` the value rounded to nearest (so `hi + lo` rounds to `hi`),
/// zero parts +0.
fn check_invariant(what: &str, d: DD) {
    let (hi, lo) = (d.hi(), d.lo());
    assert!(
        hi.is_finite() && lo.is_finite(),
        "{what}: not finite {hi:e} {lo:e}"
    );
    assert_eq!(
        (hi + lo).to_bits(),
        hi.to_bits(),
        "{what}: not normalized {hi:e} {lo:e}"
    );
    assert!(hi.to_bits() != (-0.0f64).to_bits(), "{what}: hi is −0");
    assert!(lo.to_bits() != (-0.0f64).to_bits(), "{what}: lo is −0");
    if hi == 0.0 {
        assert_eq!(lo, 0.0, "{what}: zero hi with nonzero lo");
    }
}

/// Holds a result to `|result − exact| ≤ rel·|exact| + abs`; an overflow report is allowed only
/// when the exact result is above (1 − 2^−52)·2^1024.
fn check(what: &str, got: Result<DD, NumError>, exact: &RBig, rel: &RBig, abs: &RBig) {
    match got {
        Ok(d) => {
            check_invariant(what, d);
            let err = (value(d) - exact).abs();
            let allowed = rel * exact.clone().abs() + abs;
            assert!(
                err <= allowed,
                "{what}: error {:e} above the bound {:e} (result {:e} + {:e})",
                err.to_f64().value(),
                allowed.to_f64().value(),
                d.hi(),
                d.lo()
            );
        }
        Err(NumError::NotFinite) => {
            assert!(
                exact.clone().abs() > overflow_floor(),
                "{what}: overflow reported for {:e}",
                exact.to_f64().value()
            );
        }
        Err(e) => panic!("{what}: unexpected error {e:?}"),
    }
}

/// The relative error of a result, in units of u².
fn error_in_u2(d: DD, exact: &RBig) -> f64 {
    let rel = (value(d) - exact).abs() / exact.clone().abs();
    (rel / (u() * u())).to_f64().value()
}

// ---------------------------------------------------------------------------------------------
// Generators.

/// `SplitMix64`.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
}

/// A double from its sign, leading-bit exponent e (−1074 to 1023) and the fraction bits below the
/// leading bit.
fn with_exponent(negative: bool, e: i64, fraction: u64) -> f64 {
    assert!(
        (-1074..=1023).contains(&e),
        "no double has its leading bit at 2^{e}"
    );
    let bits = if e >= -1022 {
        (((e + 1023) as u64) << 52) | (fraction & ((1 << 52) - 1))
    } else {
        let lead = e + 1074;
        (1_u64 << lead) | (fraction & ((1_u64 << lead) - 1))
    };
    let x = f64::from_bits(bits);
    if negative { -x } else { x }
}

/// A random double with leading-bit exponent in [lo, hi].
fn double_in(rng: &mut Rng, lo: i64, hi: i64) -> f64 {
    let e = rng.range(lo, hi);
    let negative = rng.next() & 1 == 1;
    with_exponent(negative, e, rng.next())
}

/// The leading-bit exponent of a finite nonzero double.
fn exponent_of(x: f64) -> i64 {
    let bits = x.to_bits();
    let field = ((bits >> 52) & 0x7ff) as i64;
    if field == 0 {
        63 - i64::from((bits & ((1 << 52) - 1)).leading_zeros()) - 1074
    } else {
        field - 1023
    }
}

/// A random `DD` with leading-bit exponent in [lo, hi]: the trailing part zero, random below half
/// a unit, exactly half a unit, or tiny relative to the leading part (so the multiplicative
/// operations drop it), normalized by `from_sum`.
fn dd_in(rng: &mut Rng, lo: i64, hi: i64) -> DD {
    let h = double_in(rng, lo, hi);
    let e = exponent_of(h);
    let unit = (e - 52).max(-1074);
    let l = match rng.below(8) {
        0 => 0.0,
        1 => {
            // Exactly half a unit, either sign: a tie that from_sum resolves to even.
            if unit > -1074 {
                with_exponent(rng.next() & 1 == 1, unit - 1, 0)
            } else {
                0.0
            }
        }
        2 => {
            let k = rng.range(330, 700);
            if e - k >= -1074 {
                double_in(rng, (e - k).max(-1074), e - k)
            } else {
                0.0
            }
        }
        _ => {
            let top = unit - 2;
            if top >= -1074 {
                double_in(rng, (top - 60).max(-1074), top)
            } else {
                0.0
            }
        }
    };
    DD::from_sum(h, l).unwrap()
}

// ---------------------------------------------------------------------------------------------
// The oracle itself.

#[test]
fn oracle_is_exact() {
    assert_eq!(rat(1.5), frac(3, 2));
    assert_eq!(rat(f64::from_bits(1)), p2(-1074));
    assert_eq!(rat(f64::MAX), (RBig::from(2) - p2(-52)) * p2(1023));
    assert_eq!(rat(-0.1) + rat(0.1), RBig::ZERO);
    let mut rng = Rng(1);
    for _ in 0..2000 {
        let (a, b) = (
            double_in(&mut rng, -1074, 1023),
            double_in(&mut rng, -1074, 1023),
        );
        let s = a + b;
        if s.is_finite() {
            // The hardware sum is the exact sum correctly rounded: dashu's rounding agrees.
            assert_eq!(
                (rat(a) + rat(b)).to_f64().value().to_bits(),
                s.to_bits(),
                "{a:e}+{b:e}"
            );
        }
    }
    let b = bounds();
    // The public constants are at least the bounds they stand for.
    assert!(rat(ADD_BOUND) >= b.add);
    assert!(rat(ADD_F64_BOUND) >= b.add_f64);
    assert!(rat(MUL_F64_BOUND) >= b.mul_f64);
    assert!(rat(MUL_BOUND) >= b.mul);
    assert!(rat(DIV_F64_BOUND) >= b.div_f64);
    assert!(rat(DIV_BOUND) >= b.div);
    assert!(rat(SQRT_BOUND) >= b.sqrt);
    assert_eq!(rat(UNDERFLOW_BOUND), p2(-1074));
    // And not loose: within 2^−48·u² of the published bound.
    for (c, b) in [
        (ADD_BOUND, &b.add),
        (MUL_F64_BOUND, &b.mul_f64),
        (DIV_F64_BOUND, &b.div_f64),
        (SQRT_BOUND, &b.sqrt),
    ] {
        assert!(rat(c) - b < p2(-48) * u() * u());
    }
}

// ---------------------------------------------------------------------------------------------
// TwoProduct.

/// `two_prod_fma` against `eft::two_prod` bit for bit, and its pair exact or reported.
fn check_two_prod_fma(a: f64, b: f64) {
    let (p, e) = two_prod_fma(a, b);
    let (q, f) = two_prod(a, b);
    if p.is_nan() {
        assert!(q.is_nan() && e.is_nan() && f.is_nan(), "{a:e}·{b:e}");
        return;
    }
    assert_eq!(p.to_bits(), q.to_bits(), "{a:e}·{b:e}: product");
    assert_eq!(
        e.to_bits(),
        f.to_bits(),
        "{a:e}·{b:e}: error {e:e} vs {f:e}"
    );
    if a.is_finite() && b.is_finite() && p.is_finite() {
        let exact = rat(a) * rat(b);
        if e.is_finite() {
            assert_eq!(rat(p) + rat(e), exact, "{a:e}·{b:e} not exact");
        } else {
            // Reported: no double holds the exact error, and e has its sign.
            let err = &exact - rat(p);
            assert!(
                !err.to_f64().value_with_exact().1,
                "{a:e}·{b:e}: a double holds the error"
            );
            assert_eq!(e > 0.0, err > RBig::ZERO, "{a:e}·{b:e}: sign of the report");
        }
    }
}

#[test]
fn two_prod_fma_matches_dekker_bit_for_bit() {
    let mut rng = Rng(2);
    for _ in 0..20_000 {
        let (a, b) = (
            double_in(&mut rng, -1074, 1023),
            double_in(&mut rng, -1074, 1023),
        );
        check_two_prod_fma(a, b);
    }
    // Products near every threshold: 2^−1074, 2^−1022, 2^−968 (the switch), 2^1022, Ω.
    for target in [
        -1075, -1074, -1060, -1023, -1022, -970, -969, -968, -967, 1021, 1022, 1023,
    ] {
        for _ in 0..400 {
            let ea = rng.range(-1074, 1023);
            let eb = (target - ea).clamp(-1074, 1023);
            let a = with_exponent(rng.next() & 1 == 1, ea, rng.next());
            let b = with_exponent(rng.next() & 1 == 1, eb, rng.next());
            check_two_prod_fma(a, b);
        }
    }
    for a in [
        0.0,
        -0.0,
        1.0,
        -1.0,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
    ] {
        for b in [
            0.0,
            -0.0,
            3.0,
            -f64::MAX,
            f64::INFINITY,
            f64::NAN,
            f64::from_bits(7),
        ] {
            check_two_prod_fma(a, b);
            check_two_prod_fma(b, a);
        }
    }
    // A zero error is +0 whatever the signs.
    assert_eq!(two_prod_fma(-3.0, 5.0).1.to_bits(), 0);
    assert_eq!(two_prod_fma(2.0 - 2.0f64.powi(-52), -1.0).1.to_bits(), 0);
}

// ---------------------------------------------------------------------------------------------
// Exact conversions.

#[test]
fn exact_conversions() {
    let mut rng = Rng(3);
    for v in [
        0,
        1,
        -1,
        i64::MAX,
        i64::MIN,
        i64::MAX - 1,
        1 << 53,
        (1 << 53) + 1,
        -(1 << 62) - 3,
    ] {
        let d = DD::from_i64(v);
        check_invariant("from_i64", d);
        assert_eq!(value(d), RBig::from(v), "from_i64({v})");
    }
    for _ in 0..5000 {
        let v = rng.next() as i64 >> rng.below(64);
        let d = DD::from_i64(v);
        check_invariant("from_i64", d);
        assert_eq!(value(d), RBig::from(v), "from_i64({v})");
    }
    let limit: i128 = 1 << 106;
    for v in [
        0,
        limit - 1,
        -(limit - 1),
        (1 << 105) + 1,
        -(1 << 105) - 1,
        3 << 100,
    ] {
        let d = DD::from_i128(v).unwrap();
        check_invariant("from_i128", d);
        assert_eq!(value(d), RBig::from(v), "from_i128({v})");
    }
    for v in [limit, -limit, i128::MAX, i128::MIN] {
        assert!(DD::from_i128(v).is_none(), "from_i128({v})");
    }
    for _ in 0..5000 {
        let v = ((u128::from(rng.next()) << 64 | u128::from(rng.next())) as i128)
            >> (22 + rng.below(100));
        let d = DD::from_i128(v).unwrap();
        check_invariant("from_i128", d);
        assert_eq!(value(d), RBig::from(v), "from_i128({v})");
    }
    for _ in 0..5000 {
        let (a, b) = (
            double_in(&mut rng, -1074, 1023),
            double_in(&mut rng, -1074, 1023),
        );
        let s = DD::from_sum(a, b);
        if (a + b).is_finite() {
            let d = s.unwrap();
            check_invariant("from_sum", d);
            assert_eq!(value(d), rat(a) + rat(b));
        } else {
            assert_eq!(s, Err(NumError::NotFinite));
        }
        if let Some(d) = DD::exact_product(a, b) {
            check_invariant("exact_product", d);
            assert_eq!(value(d), rat(a) * rat(b));
        } else {
            let p = a * b;
            let representable =
                p.is_finite() && (rat(a) * rat(b) - rat(p)).to_f64().value_with_exact().1;
            assert!(!representable, "exact_product({a:e}, {b:e}) refused");
        }
    }
    assert_eq!(DD::from_f64(f64::NAN), Err(NumError::NotFinite));
    assert_eq!(DD::from_f64(f64::NEG_INFINITY), Err(NumError::NotFinite));
    assert_eq!(DD::from_f64(-0.0).unwrap().hi().to_bits(), 0);
    assert_eq!(DD::from_sum(1.0, f64::NAN), Err(NumError::NotFinite));
    assert!(DD::exact_product(f64::INFINITY, 1.0).is_none());
    assert_eq!(DD::exact_product(-0.0, 5.0), Some(DD::ZERO));
    // to_f64 is the value correctly rounded.
    for _ in 0..2000 {
        let d = dd_in(&mut rng, -1074, 1023);
        assert_eq!(d.to_f64().to_bits(), value(d).to_f64().value().to_bits());
    }
}

// ---------------------------------------------------------------------------------------------
// The operations against their bounds.

/// Runs every binary operation on `(x, y)` against the oracle.
fn check_all(x: DD, y: DD, b: &Bounds) {
    let (vx, vy) = (value(x), value(y));
    let zero = RBig::ZERO;
    let tiny = p2(-1074);
    let what = |op: &str| {
        format!(
            "{op}(({:e}, {:e}), ({:e}, {:e}))",
            x.hi(),
            x.lo(),
            y.hi(),
            y.lo()
        )
    };
    check(&what("add"), x.add(y), &(&vx + &vy), &b.add, &zero);
    check(&what("sub"), x.sub(y), &(&vx - &vy), &b.add, &zero);
    check(
        &what("add_f64"),
        x.add_f64(y.hi()),
        &(&vx + rat(y.hi())),
        &b.add_f64,
        &zero,
    );
    check(
        &what("sub_f64"),
        x.sub_f64(y.hi()),
        &(&vx - rat(y.hi())),
        &b.add_f64,
        &zero,
    );
    check(&what("mul"), x.mul(y), &(&vx * &vy), &b.mul, &tiny);
    check(
        &what("mul_f64"),
        x.mul_f64(y.hi()),
        &(&vx * rat(y.hi())),
        &b.mul_f64,
        &tiny,
    );
    if y.hi() == 0.0 {
        assert_eq!(x.div(y), Err(NumError::NotFinite));
        assert_eq!(x.div_f64(y.hi()), Err(NumError::NotFinite));
    } else {
        check(&what("div"), x.div(y), &(&vx / &vy), &b.div, &tiny);
        check(
            &what("div_f64"),
            x.div_f64(y.hi()),
            &(&vx / rat(y.hi())),
            &b.div_f64,
            &tiny,
        );
    }
    check_sqrt(x, b);
}

/// The root of `x` against `(1 − B)²·x ≤ z² ≤ (1 + B)²·x`, or `OutsideDomain` for a negative `x`.
fn check_sqrt(x: DD, b: &Bounds) {
    let vx = value(x);
    match x.sqrt() {
        Ok(z) => {
            check_invariant("sqrt", z);
            assert!(vx >= RBig::ZERO, "sqrt of a negative value answered");
            let z2 = value(z) * value(z);
            let low = (RBig::ONE - &b.sqrt) * (RBig::ONE - &b.sqrt) * &vx;
            let high = (RBig::ONE + &b.sqrt) * (RBig::ONE + &b.sqrt) * &vx;
            assert!(
                z.hi() >= 0.0 && z2 >= low && z2 <= high,
                "sqrt({:e}, {:e})",
                x.hi(),
                x.lo()
            );
        }
        Err(NumError::OutsideDomain) => assert!(vx < RBig::ZERO),
        Err(e) => panic!("sqrt({:e}, {:e}): {e:?}", x.hi(), x.lo()),
    }
}

#[test]
fn operations_within_their_bounds_in_the_normal_range() {
    let b = bounds();
    let mut rng = Rng(4);
    for _ in 0..1200 {
        let x = dd_in(&mut rng, -300, 300);
        let y = dd_in(&mut rng, -300, 300);
        check_all(x, y, &b);
    }
}

#[test]
fn operations_within_their_bounds_over_the_whole_range() {
    let b = bounds();
    let mut rng = Rng(5);
    for _ in 0..1200 {
        let x = dd_in(&mut rng, -1074, 1023);
        let y = dd_in(&mut rng, -1074, 1023);
        check_all(x, y, &b);
    }
}

#[test]
fn operations_near_overflow_and_underflow() {
    let b = bounds();
    let mut rng = Rng(6);
    for _ in 0..600 {
        // Products and quotients that land at the overflow threshold or deep in the subnormals.
        let ex = rng.range(-1074, 1023);
        let target = [1023, 1024, 1022, -1022, -1023, -1060, -1074, -1075][rng.below(8) as usize];
        let ey = (target - ex).clamp(-1074, 1023);
        let x = dd_in(&mut rng, ex, ex);
        let y = dd_in(&mut rng, ey, ey);
        check_all(x, y, &b);
        let yi = dd_in(
            &mut rng,
            (ex - target).clamp(-1074, 1023),
            (ex - target).clamp(-1074, 1023),
        );
        check_all(x, yi, &b);
        // Sums at the overflow threshold.
        let big = dd_in(&mut rng, 1023, 1023);
        let other = dd_in(&mut rng, 1015, 1023);
        check_all(big, other, &b);
    }
    // Ω + Ω overflows; Ω − Ω and Ω·1 do not.
    let omega = DD::from_f64(f64::MAX).unwrap();
    assert_eq!(omega.add(omega), Err(NumError::NotFinite));
    assert_eq!(omega.sub(omega), Ok(DD::ZERO));
    assert_eq!(omega.mul(DD::ONE), Ok(omega));
    assert_eq!(omega.mul_f64(2.0), Err(NumError::NotFinite));
    assert_eq!(omega.div_f64(0.5), Err(NumError::NotFinite));
    // The largest DD: Ω plus the largest trailing part below its half unit.
    let top = DD::from_sum(f64::MAX, 2.0f64.powi(969) * (1.0 - 2.0f64.powi(-53))).unwrap();
    check_all(top, DD::ONE, &b);
    check_all(top, top.negate(), &b);
}

#[test]
fn cancellation() {
    let b = bounds();
    let mut rng = Rng(7);
    for _ in 0..1000 {
        let x = dd_in(&mut rng, -1074, 1023);
        // y = −x moved by a few units of its last place, of either part.
        let top = (exponent_of(x.hi()) - 50).max(-1074);
        let wiggle = double_in(&mut rng, (top - 70).max(-1074), top);
        let Ok(y) = x.negate().add_f64(wiggle) else {
            continue;
        };
        check_all(x, y, &b);
        // x ÷ (x + tiny), √ of a difference that cancels.
        if let Ok(z) = x.add(y) {
            check_all(x, x.sub(z).unwrap_or(x), &b);
        }
    }
}

#[test]
fn trailing_parts_below_the_drop_floor() {
    // A trailing part at, just above and just below 2^−400 of the leading part, where the
    // multiplicative operations drop it, keep it, or drop it after scaling rounds it.
    let b = bounds();
    let mut rng = Rng(8);
    for _ in 0..120 {
        let h = double_in(&mut rng, -600, 600);
        let e = exponent_of(h);
        for k in [399, 400, 401, 450, 600, 1000] {
            if e - k < -1074 {
                continue;
            }
            for l in [
                with_exponent(false, e - k, 0),
                with_exponent(true, e - k, rng.next()),
            ] {
                let x = DD::from_sum(h, l).unwrap();
                let y = dd_in(&mut rng, -300, 300);
                check_all(x, y, &b);
                check_all(y, x, &b);
            }
        }
    }
}

#[test]
fn errors_and_special_operands() {
    let x = DD::from_f64(3.0).unwrap();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(x.add_f64(bad), Err(NumError::NotFinite));
        assert_eq!(x.sub_f64(bad), Err(NumError::NotFinite));
        assert_eq!(x.mul_f64(bad), Err(NumError::NotFinite));
        assert_eq!(x.div_f64(bad), Err(NumError::NotFinite));
    }
    assert_eq!(x.div_f64(0.0), Err(NumError::NotFinite));
    assert_eq!(x.div_f64(-0.0), Err(NumError::NotFinite));
    assert_eq!(x.div(DD::ZERO), Err(NumError::NotFinite));
    assert_eq!(DD::ZERO.div(DD::ZERO), Err(NumError::NotFinite));
    assert_eq!(x.negate().sqrt(), Err(NumError::OutsideDomain));
    assert_eq!(DD::ZERO.sqrt(), Ok(DD::ZERO));
    assert_eq!(DD::ZERO.div(x), Ok(DD::ZERO));
    assert_eq!(x.mul_f64(-0.0), Ok(DD::ZERO));
    // A zero result is +0 in both parts.
    let z = x.sub(x).unwrap();
    assert_eq!((z.hi().to_bits(), z.lo().to_bits()), (0, 0));
    let z = x.negate().mul(DD::ZERO).unwrap();
    assert_eq!((z.hi().to_bits(), z.lo().to_bits()), (0, 0));
    assert_eq!(DD::ZERO.negate(), DD::ZERO);
    assert_eq!(x.sign(), Sign::Positive);
    assert_eq!(x.negate().sign(), Sign::Negative);
    assert_eq!(DD::ZERO.sign(), Sign::Zero);
    assert_eq!((-x).abs(), x);
}

// ---------------------------------------------------------------------------------------------
// The published inputs that bring each algorithm close to its bound.

/// A `DD` from a leading and a trailing rational, each a double.
fn dd_of(hi: RBig, lo: RBig) -> DD {
    let (h, l) = (hi.to_f64().unwrap(), lo.to_f64().unwrap());
    let d = DD::from_sum(h, l).unwrap();
    assert_eq!(
        (d.hi(), d.lo()),
        (h, l),
        "the published pair is a double-word number"
    );
    d
}

#[test]
fn published_worst_cases_are_reproduced() {
    let b = bounds();
    let u = u();
    // JMP equation (2), AccurateDWPlusDW: 2.24999999999999956·u².
    let x = dd_of(
        RBig::from(9_007_199_254_740_991_i64),
        -frac(9_007_199_254_740_991, 1) * p2(-54),
    );
    let y = dd_of(
        -frac(9_007_199_254_740_987, 2),
        -frac(9_007_199_254_740_991, 1) * p2(-56),
    );
    let r = error_in_u2(x.add(y).unwrap(), &(value(x) + value(y)));
    assert!(
        (r - 2.249_999_999_999_999_6).abs() < 1e-13,
        "AccurateDWPlusDW, JMP (2): {r}"
    );
    // Muller and Rideau, Property 2.1: x = (1, u − u²), y = (−1/2 + u/2, −u²/2 + u³): the bound
    // 3u²/(1 − 4u) is asymptotically optimal; 2.99999999999999877875·u² for p = 53.
    let x = dd_of(RBig::ONE, &u - &u * &u);
    let y = dd_of(
        -frac(1, 2) + &u / RBig::from(2),
        -(&u * &u) / RBig::from(2) + &u * &u * &u,
    );
    let r = error_in_u2(x.add(y).unwrap(), &(value(x) + value(y)));
    assert!(
        (r - 2.999_999_999_999_998_8).abs() < 1e-13,
        "AccurateDWPlusDW, MR 2.1: {r}"
    );
    assert!(rat(r) * &u * &u <= b.add);
    // JMP section 4, DWTimesFP3: 1.984…·u².
    let x = dd_of(
        RBig::from(4_505_619_370_757_448_i64),
        -frac(9_003_265_529_542_491, 1) * p2(-54),
    );
    let y = 4_511_413_997_183_120.0;
    let r = error_in_u2(x.mul_f64(y).unwrap(), &(value(x) * rat(y)));
    assert!((1.984..1.985).contains(&r), "DWTimesFP3, JMP: {r}");
    // Muller and Rideau, after Theorem 2.8, DWTimesDW3: 3.997…·u².
    let x = dd_of(
        frac(2_251_799_825_991_851, 1) * p2(-51),
        frac(9_007_199_203_085_987, 1) * p2(-106),
    );
    let y = dd_of(
        frac(4_503_599_627_471_459, 1) * p2(-52),
        frac(4_503_599_627_284_651, 1) * p2(-105),
    );
    let r = error_in_u2(x.mul(y).unwrap(), &(value(x) * value(y)));
    assert!((3.997..3.998).contains(&r), "DWTimesDW3, MR: {r}");
    // JMP section 6, DWDivFP3: 2.95157083…·u².
    let x = dd_of(
        RBig::from(4_588_860_379_563_012_i64),
        -frac(4_474_949_195_791_253, 1) * p2(-53),
    );
    let y = 4_578_284_000_230_917.0;
    let r = error_in_u2(x.div_f64(y).unwrap(), &(value(x) / rat(y)));
    // JMP print 2.95157083…; the exact value of the published algorithm on these inputs is
    // 2.9515708297…, which rounds to their digits (`operations_are_the_published_algorithms`
    // checks the algorithm step by step).
    assert!(
        (2.951_570_82..2.951_570_84).contains(&r),
        "DWDivFP3, JMP: {r}"
    );
    // JMP section 7, DWDivDW3: 5.922…·u².
    let x = dd_of(
        RBig::from(4_528_288_502_329_187_i64),
        frac(1_125_391_118_633_487, 1) * p2(-51),
    );
    let y = dd_of(
        RBig::from(4_522_593_432_466_394_i64),
        -frac(9_006_008_290_016_505, 1) * p2(-54),
    );
    let r = error_in_u2(x.div(y).unwrap(), &(value(x) / value(y)));
    assert!((5.922..5.923).contains(&r), "DWDivDW3, JMP: {r}");
    // The same inputs scaled into the subnormal and huge ranges give the same relative errors
    // (scaling by powers of two is exact), except where a part underflows.
    let x = dd_of(
        RBig::from(4_505_619_370_757_448_i64),
        -frac(9_003_265_529_542_491, 1) * p2(-54),
    );
    for k in [-900, -500, 500, 900] {
        let xs = DD::from_sum(x.hi() * 2.0f64.powi(k), x.lo() * 2.0f64.powi(k)).unwrap();
        let r = error_in_u2(
            xs.mul_f64(4_511_413_997_183_120.0).unwrap(),
            &(value(xs) * rat(4_511_413_997_183_120.0)),
        );
        assert!(
            (1.984..1.985).contains(&r),
            "DWTimesFP3 scaled by 2^{k}: {r}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The published algorithms, written out from their listings with an independent fused
// multiply-add (the exact value rounded by dashu), against the module bit for bit.

/// RN(a·b + c), from the exact value.
fn fma_ref(a: f64, b: f64, c: f64) -> f64 {
    (rat(a) * rat(b) + rat(c)).to_f64().value()
}

/// `2Sum`, and `Fast2Sum` where its condition holds: the rounded sum and its exact error.
fn two_sum_ref(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    (s, (rat(a) + rat(b) - rat(s)).to_f64().value())
}

/// `Fast2Mult` (JMP Algorithm 3).
fn two_prod_ref(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    (p, fma_ref(a, b, -p))
}

/// A pair with zeros made +0, for comparing bits.
fn plus_zeros((h, l): (f64, f64)) -> (u64, u64) {
    ((h + 0.0).to_bits(), (l + 0.0).to_bits())
}

/// JMP Algorithm 6, `AccurateDWPlusDW`.
fn accurate_dw_plus_dw(x: (f64, f64), y: (f64, f64)) -> (f64, f64) {
    let (sh, sl) = two_sum_ref(x.0, y.0);
    let (th, tl) = two_sum_ref(x.1, y.1);
    let c = sl + th;
    let (vh, vl) = two_sum_ref(sh, c);
    let w = tl + vl;
    two_sum_ref(vh, w)
}

/// JMP Algorithm 4, `DWPlusFP`.
fn dw_plus_fp(x: (f64, f64), y: f64) -> (f64, f64) {
    let (sh, sl) = two_sum_ref(x.0, y);
    let v = x.1 + sl;
    two_sum_ref(sh, v)
}

/// JMP Algorithm 9, `DWTimesFP3`.
fn dw_times_fp3(x: (f64, f64), y: f64) -> (f64, f64) {
    let (ch, cl1) = two_prod_ref(x.0, y);
    let cl3 = fma_ref(x.1, y, cl1);
    two_sum_ref(ch, cl3)
}

/// JMP Algorithm 12, `DWTimesDW3`.
fn dw_times_dw3(x: (f64, f64), y: (f64, f64)) -> (f64, f64) {
    let (ch, cl1) = two_prod_ref(x.0, y.0);
    let tl0 = x.1 * y.1;
    let tl1 = fma_ref(x.0, y.1, tl0);
    let cl2 = fma_ref(x.1, y.0, tl1);
    let cl3 = cl1 + cl2;
    two_sum_ref(ch, cl3)
}

/// JMP Algorithm 15, `DWDivFP3`.
fn dw_div_fp3(x: (f64, f64), y: f64) -> (f64, f64) {
    let th = x.0 / y;
    let (ph, pl) = two_prod_ref(th, y);
    let dh = x.0 - ph;
    let dt = dh - pl;
    let d = dt + x.1;
    let tl = d / y;
    two_sum_ref(th, tl)
}

/// JMP Algorithm 18, `DWDivDW3`.
fn dw_div_dw3(x: (f64, f64), y: (f64, f64)) -> (f64, f64) {
    let th = 1.0 / y.0;
    let rh = fma_ref(-y.0, th, 1.0);
    let rl = -(y.1 * th);
    let e = two_sum_ref(rh, rl);
    let d = dw_times_fp3(e, th);
    let m = dw_plus_fp(d, th);
    dw_times_dw3(x, m)
}

/// Lefèvre et al., Algorithm 8, `SQRTDWtoDW`.
fn sqrt_dw_to_dw(x: (f64, f64)) -> (f64, f64) {
    let sh = x.0.sqrt();
    let rho1 = fma_ref(-sh, sh, x.0);
    let rho2 = x.1 + rho1;
    let sl = rho2 / (2.0 * sh);
    two_sum_ref(sh, sl)
}

/// A random `DD` in the normal range whose trailing part is not dropped: there the module's
/// scaling is exact, so it must return the published algorithm's bits.
fn dd_plain(rng: &mut Rng, lo: i64, hi: i64) -> DD {
    let h = double_in(rng, lo, hi);
    let top = exponent_of(h) - 54;
    let l = if rng.below(5) == 0 {
        0.0
    } else {
        double_in(rng, top - 100, top)
    };
    DD::from_sum(h, l).unwrap()
}

/// The operands of `DD::mul` in the order the module documents: the one whose value scaled to a
/// leading part in [1, 2) has the larger magnitude first (when the two are equal the order does
/// not matter). Computed from the exact values.
fn mul_order(x: DD, y: DD) -> (DD, DD) {
    let scaled = |d: DD| value(d).abs() * p2(-exponent_of(d.hi()));
    if scaled(x) >= scaled(y) {
        (x, y)
    } else {
        (y, x)
    }
}

#[test]
fn operations_are_the_published_algorithms() {
    let mut rng = Rng(11);
    let pair = |d: DD| (d.hi(), d.lo());
    let bits = |d: DD| plus_zeros(pair(d));
    for _ in 0..5000 {
        let x = dd_plain(&mut rng, -200, 200);
        let y = dd_plain(&mut rng, -200, 200);
        let what = format!("({:e}, {:e}), ({:e}, {:e})", x.hi(), x.lo(), y.hi(), y.lo());
        assert_eq!(
            bits(x.add(y).unwrap()),
            plus_zeros(accurate_dw_plus_dw(pair(x), pair(y))),
            "add {what}"
        );
        assert_eq!(
            bits(x.add_f64(y.hi()).unwrap()),
            plus_zeros(dw_plus_fp(pair(x), y.hi())),
            "add_f64 {what}"
        );
        assert_eq!(
            bits(x.mul_f64(y.hi()).unwrap()),
            plus_zeros(dw_times_fp3(pair(x), y.hi())),
            "mul_f64 {what}"
        );
        let (a, b) = mul_order(x, y);
        assert_eq!(
            bits(x.mul(y).unwrap()),
            plus_zeros(dw_times_dw3(pair(a), pair(b))),
            "mul {what}"
        );
        assert_eq!(
            bits(x.div_f64(y.hi()).unwrap()),
            plus_zeros(dw_div_fp3(pair(x), y.hi())),
            "div_f64 {what}"
        );
        assert_eq!(
            bits(x.div(y).unwrap()),
            plus_zeros(dw_div_dw3(pair(x), pair(y))),
            "div {what}"
        );
        let a = x.abs();
        assert_eq!(
            bits(a.sqrt().unwrap()),
            plus_zeros(sqrt_dw_to_dw(pair(a))),
            "sqrt {what}"
        );
    }
}

#[test]
fn add_and_mul_are_commutative_bit_for_bit() {
    // DWTimesDW3's listing in its two orders differs in the last bit of the trailing part for
    // about one pair in a hundred (the adversary found 1856 of 200 000); `asymmetric` counts the
    // pairs here for which it does, so the test is known to reach them.
    let pair = |d: DD| (d.hi(), d.lo());
    let mut rng = Rng(0x636f_6d6d);
    let mut asymmetric = 0;
    for i in 0..100_000 {
        let (x, y) = if i % 4 == 0 {
            (dd_in(&mut rng, -1074, 1023), dd_in(&mut rng, -1074, 1023))
        } else {
            let (x, y) = (dd_plain(&mut rng, -3, 3), dd_plain(&mut rng, -3, 3));
            if plus_zeros(dw_times_dw3(pair(x), pair(y)))
                != plus_zeros(dw_times_dw3(pair(y), pair(x)))
            {
                asymmetric += 1;
            }
            (x, y)
        };
        assert_eq!(x.mul(y), y.mul(x), "mul ({x:?}, {y:?})");
        assert_eq!(x.add(y), y.add(x), "add ({x:?}, {y:?})");
        // Equal magnitudes, either sign, and the symmetries the order keeps.
        assert_eq!(x.mul(x.negate()), x.negate().mul(x), "mul ({x:?}, −x)");
        assert_eq!(
            x.negate().mul(y),
            x.mul(y).map(DD::negate),
            "sign ({x:?}, {y:?})"
        );
    }
    // The order matters for a few in a hundred random pairs; the test exercises it.
    assert!(asymmetric > 100, "{asymmetric}");
}

// ---------------------------------------------------------------------------------------------
// Order and rounding to integers.

#[test]
fn order_is_the_order_of_the_values() {
    let mut rng = Rng(9);
    let mut values = Vec::new();
    for _ in 0..400 {
        let x = dd_in(&mut rng, -1074, 1023);
        values.push(x);
        values.push(x.negate());
        // Same hi, neighbouring lo.
        if let Ok(y) = DD::from_sum(x.hi(), x.lo() + f64::from_bits(1)) {
            values.push(y);
        }
    }
    values.push(DD::ZERO);
    for (i, &a) in values.iter().enumerate() {
        for &b in values.iter().skip(i).take(25) {
            let expected = value(a).cmp(&value(b));
            assert_eq!(
                a.cmp(&b),
                expected,
                "({:e}, {:e}) vs ({:e}, {:e})",
                a.hi(),
                a.lo(),
                b.hi(),
                b.lo()
            );
            assert_eq!(a == b, expected == Ordering::Equal);
        }
    }
}

/// The exact value minus `offset` rounded to the nearest integer, ties to even, and whether it was
/// an integer.
fn round_oracle(v: &RBig, offset: i128) -> (IBig, bool) {
    let w = v - RBig::from(offset);
    let f = w.floor();
    let rest = &w - RBig::from(f.clone());
    let half = frac(1, 2);
    let up = match rest.cmp(&half) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => (&f % IBig::from(2)) != IBig::ZERO,
    };
    (if up { f + IBig::ONE } else { f }, rest == RBig::ZERO)
}

#[test]
fn round_shifted_rounds_the_exact_value() {
    let mut rng = Rng(10);
    for _ in 0..5000 {
        let x = dd_in(&mut rng, -10, 99);
        let offset = i128::from(rng.next() as i64 >> rng.below(64));
        let got = x.round_shifted(offset).unwrap();
        let (n, exact) = round_oracle(&value(x), offset);
        assert_eq!(
            IBig::from(got.value()),
            n,
            "({:e}, {:e}) − {offset}",
            x.hi(),
            x.lo()
        );
        assert_eq!(got.is_exact(), exact);
    }
    // Ties in each part and across the parts.
    for (h, l) in [
        (2.5, 0.0),
        (3.5, 0.0),
        (-2.5, 0.0),
        (4.0, 0.5),
        (5.0, -0.5),
        (2f64.powi(60), 0.5),
        (2f64.powi(60), -0.5),
        (2f64.powi(60), 1.5),
    ] {
        let x = DD::from_sum(h, l).unwrap();
        let got = x.round_shifted(7).unwrap();
        let (n, exact) = round_oracle(&value(x), 7);
        assert_eq!(IBig::from(got.value()), n, "{h} + {l}");
        assert_eq!(got.is_exact(), exact);
    }
    assert!(
        DD::from_f64(2f64.powi(101))
            .unwrap()
            .round_shifted(0)
            .is_none()
    );
    assert!(
        DD::from_f64(2f64.powi(100))
            .unwrap()
            .round_shifted(i128::MIN)
            .is_none()
    );
}
