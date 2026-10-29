//! Adversarial tests for piece G03 (`geom-math`).
//!
//! Each test tries one way the crate could be wrong: a result against an independent exact oracle
//! (`num-rational`, never the code under test), a value at the edge of a documented range, a
//! claim in the crate's own documentation, a rule of chapters 6.1, 7 and 17.1, or a difference
//! between targets or thread counts. A test that fails names the claim it breaks.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    clippy::single_match_else,
    clippy::unreadable_literal,
    reason = "test cases and oracles, named as in the formulas they check; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]

use std::cmp::Ordering;

use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

use geom_math::frame::{ORIGIN_MAX, SHIFT_MAX, steps_per_metre};
use geom_math::grid_transform::TRANSLATION_MAX;
use geom_math::vector::VEC_MAX;
use geom_math::{
    Axis, Box2, Box3, CrsId, F2, F3, FBox3, FrameMismatch, FrameRecord, GridTransform, MapPoint,
    MathError, Rotation, Scale, Transform, TransformKind, V3,
};
// The double-double type the frame conversions use: `geom-num`'s (piece G25), since piece G48
// moved `geom-math` onto it. G03's attacks on its own type now hold that one to the bounds G03
// documented, which `geom-num` proves tighter.
use geom_num::dd::DD;
use geom_num::grid::{COORD_MAX, GridStep, P2, P3};
use geom_num::ladder::ANGULAR;
use geom_num::{NumError, math};
use geom_test::golden::{self, Sha256, hex};

// ---------------------------------------------------------------------------------------------
// A deterministic generator (SplitMix64), so every run sees the same cases.
// ---------------------------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `lo..=hi`, for spans up to 2^64.
    fn range(&mut self, lo: i128, hi: i128) -> i128 {
        let span = u128::try_from(hi - lo + 1).unwrap();
        lo + i128::try_from(u128::from(self.next()) % span).unwrap()
    }

    fn coin(&mut self) -> bool {
        self.next() & 1 == 1
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[usize::try_from(self.next() % items.len() as u64).unwrap()]
    }
}

/// A grid coordinate, biased towards the edges of the range and towards small values.
fn coord(rng: &mut Rng) -> i64 {
    let m = i128::from(COORD_MAX);
    match rng.next() % 6 {
        0 => rng.pick(&[
            0,
            1,
            -1,
            COORD_MAX,
            -COORD_MAX,
            COORD_MAX - 1,
            -COORD_MAX + 1,
        ]),
        1 => i64::try_from(rng.range(-1000, 1000)).unwrap(),
        2 => i64::try_from(rng.range(-(1 << 20), 1 << 20)).unwrap(),
        _ => i64::try_from(rng.range(-m, m)).unwrap(),
    }
}

fn point(rng: &mut Rng) -> P3 {
    P3::new(coord(rng), coord(rng), coord(rng)).unwrap()
}

// ---------------------------------------------------------------------------------------------
// Exact oracles.
// ---------------------------------------------------------------------------------------------

fn int(v: i128) -> BigRational {
    BigRational::from_integer(BigInt::from(v))
}

fn pow2q(k: i32) -> BigRational {
    let p = BigInt::one() << k.unsigned_abs();
    if k >= 0 {
        BigRational::from_integer(p)
    } else {
        BigRational::new(BigInt::one(), p)
    }
}

/// 2^k as a double, for a normal power of two.
fn pow2f(k: i32) -> f64 {
    assert!((-1022..=1023).contains(&k));
    f64::from_bits(u64::try_from(k + 1023).unwrap() << 52)
}

/// The exact value of a finite double, from its bits.
fn exact(x: f64) -> BigRational {
    assert!(x.is_finite(), "exact() of {x}");
    let bits = x.to_bits();
    let biased = i32::try_from((bits >> 52) & 0x7ff).unwrap();
    let fraction = bits & ((1u64 << 52) - 1);
    let (m, e) = if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1u64 << 52), biased - 1075)
    };
    let v = BigRational::from_integer(BigInt::from(m)) * pow2q(e);
    if bits >> 63 == 1 { -v } else { v }
}

fn round_half_even(x: &BigRational) -> BigInt {
    let fl = x.floor().to_integer();
    let frac = x - BigRational::from_integer(fl.clone());
    let half = BigRational::new(BigInt::one(), BigInt::from(2));
    match frac.cmp(&half) {
        Ordering::Less => fl,
        Ordering::Greater => fl + BigInt::one(),
        Ordering::Equal => {
            if fl.is_even() {
                fl
            } else {
                fl + BigInt::one()
            }
        }
    }
}

/// Whether `d` is the double nearest `x`, ties to even.
fn is_nearest_double(d: f64, x: &BigRational) -> bool {
    let e = exact(d);
    let two = int(2);
    let below = (exact(d.next_down()) + &e) / &two;
    let above = (exact(d.next_up()) + &e) / &two;
    let even = d.to_bits() & 1 == 0;
    let lower_ok = if even { *x >= below } else { *x > below };
    let upper_ok = if even { *x <= above } else { *x < above };
    lower_ok && upper_ok
}

/// The sign of `a − b·√n` (n > 0, not necessarily a square), exactly, by comparing squares.
fn sign_minus_sqrt(a: &BigRational, b: &BigRational, n: u64) -> Ordering {
    let zero = BigRational::zero();
    let rn = int(i128::from(n));
    match (a.cmp(&zero), b.cmp(&zero)) {
        (Ordering::Equal, Ordering::Equal) => Ordering::Equal,
        (Ordering::Greater | Ordering::Equal, Ordering::Less | Ordering::Equal) => {
            Ordering::Greater
        }
        (Ordering::Less | Ordering::Equal, Ordering::Greater | Ordering::Equal) => Ordering::Less,
        (Ordering::Greater, Ordering::Greater) => (a * a).cmp(&(b * b * &rn)),
        (Ordering::Less, Ordering::Less) => (b * b * &rn).cmp(&(a * a)),
    }
}

fn dd_value(d: DD) -> BigRational {
    exact(d.hi()) + exact(d.lo())
}

/// |got − want| ≤ bound·|want|.
fn rel_ok(got: &BigRational, want: &BigRational, bound: &BigRational) -> bool {
    (got - want).abs() <= bound * want.abs()
}

/// The DD invariant: finite, `hi` the sum rounded to nearest, |lo| ≤ ulp(hi)/2.
fn normalized(d: DD) -> bool {
    let (h, l) = (d.hi(), d.lo());
    if !h.is_finite() || !l.is_finite() || h + l != h {
        return false;
    }
    if h == 0.0 {
        return l == 0.0;
    }
    let ulp = exact(math::abs(h).unwrap().get().next_up()) - exact(math::abs(h).unwrap().get());
    exact(l).abs() * int(2) <= ulp
}

/// Collects failures of a sweep and reports the first few.
struct Failures {
    count: usize,
    total: usize,
    first: Vec<String>,
}

impl Failures {
    fn new() -> Failures {
        Failures {
            count: 0,
            total: 0,
            first: Vec::new(),
        }
    }

    fn record(&mut self, r: Result<(), String>) {
        self.total += 1;
        if let Err(e) = r {
            self.count += 1;
            if self.first.len() < 6 {
                self.first.push(e);
            }
        }
    }

    fn assert_none(&self, what: &str) {
        assert!(
            self.count == 0,
            "{what}: {} of {} cases failed; first: {:#?}",
            self.count,
            self.total,
            self.first
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Double-double: documented errors and bounds.
// ---------------------------------------------------------------------------------------------

/// `DD::mul_f64` documents `NotFinite` only "if `y` is not finite or the result overflows".
/// 2^1000 · 0.5 = 2^999 does not overflow.
#[test]
fn dd_mul_f64_of_a_large_finite_value_is_not_an_error() {
    let x = DD::from_f64(pow2f(1000)).unwrap();
    let got = x.mul_f64(0.5);
    assert_eq!(
        got.map(DD::to_f64),
        Ok(pow2f(999)),
        "2^1000 · 0.5 refused although it is finite"
    );
}

/// `DD::mul` documents `NotFinite` only "if the result overflows". 2^997 · 2^-2 = 2^995.
#[test]
fn dd_mul_of_large_finite_values_is_not_an_error() {
    let x = DD::from_f64(pow2f(997)).unwrap();
    let y = DD::from_f64(0.25).unwrap();
    assert_eq!(
        x.mul(y).map(DD::to_f64),
        Ok(pow2f(995)),
        "2^997 · 1/4 refused"
    );
}

/// `DD::div` documents `NotFinite` only "if the result overflows". Both quotients are finite.
#[test]
fn dd_div_with_a_large_operand_is_not_an_error() {
    let big = DD::from_f64(pow2f(1000)).unwrap();
    let four = DD::from_f64(4.0).unwrap();
    let one = DD::from_f64(1.0).unwrap();
    let mut errs = Vec::new();
    let a = big.div(four);
    if a.map(DD::to_f64) != Ok(pow2f(998)) {
        errs.push(format!("2^1000 / 4 = {a:?}, expected 2^998"));
    }
    let b = one.div(big);
    if b.map(DD::to_f64) != Ok(pow2f(-1000)) {
        errs.push(format!("1 / 2^1000 = {b:?}, expected 2^-1000"));
    }
    assert!(errs.is_empty(), "{errs:#?}");
}

/// `DD::sqrt` documents only `NegativeSqrt`. The square root of the largest double is finite.
#[test]
fn dd_sqrt_of_the_largest_double_succeeds() {
    let x = DD::from_f64(f64::MAX).unwrap();
    let r = x.sqrt();
    assert!(
        r.is_ok(),
        "sqrt(f64::MAX) = {r:?}; the docs list only NegativeSqrt"
    );
    let r = r.unwrap();
    // Within 5u² of the root: compare squares exactly.
    let a = dd_value(x);
    let beta = int(5) * pow2q(-106);
    let rv = dd_value(r);
    let lo = (int(1) - &beta) * (int(1) - &beta) * &a;
    let hi = (int(1) + &beta) * (int(1) + &beta) * &a;
    assert!(rv.clone() * &rv >= lo && rv.clone() * &rv <= hi);
}

/// The `dd` module's underflow clause: "When one does [underflow], the error gains an absolute
/// term of a few multiples of 2^−1074." In `sqrt`, x₀² for a near 2^−1000 has an error term below
/// the subnormal range, and the Newton step divides that residual error by 2x₀ ≈ 2^−499, so the
/// absolute error is about 2^−576, not a few multiples of 2^−1074 (relative about 2^−76, against
/// the table's 5u² ≈ 2^−104).
#[test]
fn dd_sqrt_near_the_underflow_threshold_keeps_the_documented_error() {
    let mut rng = Rng(0x5097_0001);
    let mut worst = BigRational::zero();
    let mut errs = Vec::new();
    for _ in 0..400 {
        let hi = math::abs(rand_double(&mut rng, -1010, -940)).unwrap().get();
        let e = exponent(hi);
        let lo = if e - 54 < -1022 || rng.coin() {
            0.0
        } else {
            rand_double(&mut rng, (e - 106).max(-1022), e - 54)
        };
        let a = DD::from_sum(hi, lo).unwrap();
        let r = a.sqrt().unwrap();
        let (va, vr) = (dd_value(a), dd_value(r));
        // |r − √a| ≤ δ with δ = 6u²·r + 16·2^−1074 (5u² of the table, rounded up for using r in
        // place of √a, plus the clause's absolute term), checked exactly by squaring.
        let delta = int(6) * pow2q(-106) * &vr + int(16) * pow2q(-1074);
        let (lo, hi) = (&vr - &delta, &vr + &delta);
        if !(&lo * &lo <= va && va <= &hi * &hi) {
            // The error, from a 300-bit integer root of a (a = m·2^−2k exactly).
            let scaled = (&va * pow2q(2200)).to_integer();
            let root = BigRational::new(scaled.sqrt(), BigInt::one() << 1100u32);
            let err = (&vr - &root).abs() / &root;
            if err > worst {
                worst = err.clone();
            }
            if errs.len() < 3 {
                let e = err.to_f64().unwrap();
                errs.push(format!("sqrt({a:?}) = {r:?}: relative error {e:e}"));
            }
        }
    }
    assert!(
        errs.is_empty(),
        "worst relative error {:e}: {errs:#?}",
        worst.to_f64().unwrap()
    );
}

fn rand_double(rng: &mut Rng, emin: i32, emax: i32) -> f64 {
    let e = i32::try_from(rng.range(emin.into(), emax.into())).unwrap();
    let frac = match rng.next() % 5 {
        0 => (1u64 << 52) - 1,
        1 => 0,
        2 => 1,
        _ => rng.next() & ((1u64 << 52) - 1),
    };
    let v = f64::from_bits((u64::try_from(e + 1023).unwrap() << 52) | frac);
    if rng.coin() { -v } else { v }
}

fn exponent(x: f64) -> i32 {
    i32::try_from((x.to_bits() >> 52) & 0x7ff).unwrap() - 1023
}

/// A normalized DD with a hostile trailing part: zero, exactly half an ulp (a tie), or random.
fn rand_dd(rng: &mut Rng, emin: i32, emax: i32) -> DD {
    let hi = rand_double(rng, emin, emax);
    let e = exponent(hi);
    let lo = match rng.next() % 4 {
        0 => 0.0,
        1 => {
            let h = pow2f(e - 53);
            if rng.coin() { h } else { -h }
        }
        _ => rand_double(rng, e - 106, e - 54),
    };
    DD::from_sum(hi, lo).unwrap()
}

/// Every operation stays within the relative bound in the table of `dd`'s module documentation,
/// and every result is normalized, on adversarial operands (all-ones and all-zeros significands,
/// trailing parts of exactly half an ulp, cancelling sums).
#[test]
fn dd_operations_meet_their_documented_bounds() {
    let mut rng = Rng(0x00D0_0B1E);
    let u2 = pow2q(-106);
    let u3 = pow2q(-159);
    let b_add = int(3) * &u2 + int(13) * &u3;
    let b_addf = int(2) * &u2 + int(5) * &u3;
    let b_mulf = int(2) * &u2;
    let b_mul = int(7) * &u2;
    let b_div = int(4) * &u2 + int(60) * &u3;
    let b_sqrt = int(5) * &u2;
    let mut f = Failures::new();
    for i in 0..6000 {
        let a = rand_dd(&mut rng, -250, 250);
        // Every third case cancels: b is close to −a.
        let b = if i % 3 == 0 {
            let k = i32::try_from(rng.range(1, 100)).unwrap();
            DD::from_sum(-a.hi(), a.lo() * pow2f(-k)).unwrap_or(a)
        } else {
            rand_dd(&mut rng, -250, 250)
        };
        let y = rand_double(&mut rng, -250, 250);
        let (va, vb, vy) = (dd_value(a), dd_value(b), exact(y));
        let mut check = |name: &str, got: Result<DD, NumError>, want: BigRational, bound| {
            let r = match got {
                Ok(d) if !normalized(d) => Err(format!("{name}: not normalized {d:?}")),
                Ok(d) if !rel_ok(&dd_value(d), &want, bound) => Err(format!(
                    "{name}({a:?}, {b:?}, {y:e}) = {d:?} outside its bound"
                )),
                Ok(_) => Ok(()),
                Err(e) => Err(format!("{name}({a:?}, {b:?}, {y:e}) failed: {e:?}")),
            };
            f.record(r);
        };
        check("add", a.add(b), &va + &vb, &b_add);
        check("sub", a.sub(b), &va - &vb, &b_add);
        check("add_f64", a.add_f64(y), &va + &vy, &b_addf);
        check("mul_f64", a.mul_f64(y), &va * &vy, &b_mulf);
        check("mul", a.mul(b), &va * &vb, &b_mul);
        if !vb.is_zero() {
            check("div", a.div(b), &va / &vb, &b_div);
        }
        // Square root of |a|.
        let pa = if a.hi() < 0.0 { -a } else { a };
        let vpa = dd_value(pa);
        let r = match pa.sqrt() {
            Ok(r) if !normalized(r) => Err(format!("sqrt: not normalized {r:?}")),
            Ok(r) => {
                let rv = dd_value(r);
                let lo = (int(1) - &b_sqrt) * (int(1) - &b_sqrt) * &vpa;
                let hi = (int(1) + &b_sqrt) * (int(1) + &b_sqrt) * &vpa;
                let sq = &rv * &rv;
                if rv >= BigRational::zero() && sq >= lo && sq <= hi {
                    Ok(())
                } else {
                    Err(format!("sqrt({pa:?}) = {r:?} outside 5u²"))
                }
            }
            Err(e) => Err(format!("sqrt({pa:?}) failed: {e:?}")),
        };
        f.record(r);
    }
    f.assert_none("double-double bounds");
}

/// `round_shifted` against exact rounding, ties to even, including the documented limits.
#[test]
fn dd_round_shifted_matches_exact_rounding() {
    let mut rng = Rng(0x0005_41F7);
    let mut f = Failures::new();
    let limit = pow2f(100);
    for _ in 0..20000 {
        let hi = match rng.next() % 5 {
            0 => f64::from(i32::try_from(rng.range(-1_000_000, 1_000_000)).unwrap()) + 0.5,
            1 => rand_double(&mut rng, -3, 99),
            2 => {
                let k = i32::try_from(rng.range(53, 100)).unwrap();
                let v = pow2f(k);
                if rng.coin() { -v } else { v }
            }
            3 => rng.pick(&[0.5, -0.5, 1.5, -1.5, 2.5, -2.5, 0.0, limit, -limit]),
            _ => rand_double(&mut rng, -60, 60),
        };
        let e = if hi == 0.0 { -60 } else { exponent(hi) };
        let lo = match rng.next() % 4 {
            0 => 0.0,
            1 => {
                let h = pow2f((e - 53).max(-1000));
                if rng.coin() { h } else { -h }
            }
            2 => {
                // The smaller of the two, by comparison (chapter 21: never `f64::min`).
                let quarter = rng.pick(&[0.5f64, -0.5, 0.25, -0.25]);
                let h = pow2f((e - 53).max(-1000));
                if h < quarter { h } else { quarter }
            }
            _ => rand_double(&mut rng, (e - 106).max(-1000), (e - 54).max(-999)),
        };
        let Ok(d) = DD::from_sum(hi, lo) else {
            continue;
        };
        let offset = match rng.next() % 3 {
            0 => 0,
            1 => rng.range(-(1 << 40), 1 << 40),
            _ => rng.range(-(1i128 << 62), 1i128 << 62) << 38,
        };
        let got = d.round_shifted(offset);
        let r = if math::abs(d.hi()).unwrap().get() > limit {
            if got.is_none() {
                Ok(())
            } else {
                Err(format!("{d:?}: expected None, got {got:?}"))
            }
        } else {
            let v = dd_value(d) - int(offset);
            let want = round_half_even(&v);
            match (want.to_i128(), got) {
                (Some(w), Some(g)) if g.value() == w && g.is_exact() == v.is_integer() => Ok(()),
                (None, None) => Ok(()),
                (w, g) => Err(format!("{d:?} − {offset}: expected {w:?}, got {g:?}")),
            }
        };
        f.record(r);
    }
    // The documented edges.
    let edge = DD::from_f64(limit).unwrap();
    f.record(match edge.round_shifted(0) {
        Some(g) if g.value() == 1i128 << 100 && g.is_exact() => Ok(()),
        g => Err(format!("2^100 rounds to {g:?}")),
    });
    let over = DD::from_f64(limit.next_up()).unwrap();
    f.record(if over.round_shifted(0).is_none() {
        Ok(())
    } else {
        Err("above 2^100".into())
    });
    // −1 − i128::MAX is i128::MIN, which fits; −2 − i128::MAX does not.
    let fits = DD::from_f64(-1.0).unwrap().round_shifted(i128::MAX);
    let over = DD::from_f64(-2.0).unwrap().round_shifted(i128::MAX);
    f.record(match (fits, over) {
        (Some(g), None) if g.value() == i128::MIN && g.is_exact() => Ok(()),
        other => Err(format!("i128 edges: {other:?}")),
    });
    f.assert_none("round_shifted");
}

#[test]
fn dd_integer_conversions_at_the_extremes() {
    for v in [
        i64::MIN,
        i64::MIN + 1,
        i64::MAX,
        i64::MAX - 1,
        (1 << 53) + 1,
        -(1 << 53) - 1,
        0,
    ] {
        let d = DD::from_i64(v);
        assert_eq!(dd_value(d), int(i128::from(v)), "from_i64({v})");
        assert!(normalized(d), "from_i64({v}) = {d:?}");
    }
    let edge = (1i128 << 106) - 1;
    for v in [
        edge,
        -edge,
        edge - 12345,
        (1i128 << 105) + 1,
        -(1i128 << 105) - 1,
    ] {
        let d = DD::from_i128(v).unwrap();
        assert_eq!(dd_value(d), int(v), "from_i128({v})");
        assert!(normalized(d), "from_i128({v}) = {d:?}");
    }
    assert_eq!(DD::from_i128(1i128 << 106), None);
    assert_eq!(DD::from_i128(-(1i128 << 106)), None);
    assert_eq!(DD::from_i128(i128::MIN), None);
}

// ---------------------------------------------------------------------------------------------
// General transforms.
// ---------------------------------------------------------------------------------------------

/// `TransformKind::Degenerate` is documented as "it collapses space onto a plane, a line or a
/// point, and has no inverse". A uniform scale by 10^103 does neither: its inverse, a scale by
/// 10^−103, is finite. Its determinant (10^309) overflows the interval enclosure.
#[test]
fn transform_uniform_scale_with_overflowing_determinant_is_not_degenerate() {
    let t = Transform::uniform_scale(1e103).unwrap();
    let c = t.classify().unwrap();
    let inv = t.inverse();
    assert!(
        c.kind == TransformKind::UniformScale && inv.is_ok(),
        "scale 1e103 classified {c:?}, inverse {inv:?}"
    );
}

/// The same with a determinant (10^−330) that underflows: a scale by 10^−110 has the finite
/// inverse 10^110, so it is not singular.
#[test]
fn transform_uniform_scale_with_underflowing_determinant_is_not_degenerate() {
    let t = Transform::uniform_scale(1e-110).unwrap();
    let c = t.classify().unwrap();
    let inv = t.inverse();
    assert!(
        c.kind == TransformKind::UniformScale && inv.is_ok(),
        "scale 1e-110 classified {c:?}, inverse {inv:?}"
    );
}

/// The module table and `Classification::keeps_lengths` say a `Rigid` transform keeps every length
/// within a relative `ε_ang`, and a `UniformScale` multiplies every length by s = |`L·e_x`| within a
/// relative `ε_ang`. L = c·(I + a·J) (J all ones) stretches (1, 1, 1) by c(1 + 3a) and the plane
/// orthogonal to it by c, so its extreme squared length ratios are exact rationals. The attack
/// found a = `0.495·ε_ang` classified `Rigid` although it stretches (1, 1, 1) by `1.485·ε_ang`.
///
/// Fixer: the original test asserted `Rigid` for that matrix as a precondition. The classification
/// is now certified by a Gershgorin bound on `LᵀL` − I (and on LᵀL/G₀₀ − I), which correctly no
/// longer calls that matrix `Rigid`, so the precondition no longer holds. The test now checks the
/// promise itself across a sweep of a over the classification boundary, for both kinds and with a
/// mirror, and requires both kinds to occur, so it is no weaker.
#[test]
fn transform_rigid_keeps_lengths_within_the_angular_bound() {
    let eps = exact(ANGULAR.get());
    let one = int(1);
    let (lo, hi) = (&one - &eps, &one + &eps);
    let (lo2, hi2) = (&lo * &lo, &hi * &hi);
    let mut kinds = [0usize; 2];
    let mut errs = Vec::new();
    for c in [1.0, 3.0, -0.25] {
        for i in -40i32..=40 {
            let a = f64::from(i) * 0.025 * ANGULAR.get();
            let (d, o) = (c * (1.0 + a), c * a);
            let t = Transform::from_rows([[d, o, o, 0.0], [o, d, o, 0.0], [o, o, d, 0.0]]).unwrap();
            let cl = t.classify().unwrap();
            // Exact squared length ratios along (1, 1, 1) and (1, −1, 0): (d + 2o)² and (d − o)².
            let (de, oe) = (exact(d), exact(o));
            let r1 = (&de + &oe * int(2)) * (&de + &oe * int(2));
            let r2 = (&de - &oe) * (&de - &oe);
            // The reference: 1 for Rigid, s² = |first column|² = d² + 2o² for UniformScale.
            let reference = match cl.kind {
                TransformKind::Rigid => {
                    kinds[0] += 1;
                    one.clone()
                }
                TransformKind::UniformScale => {
                    kinds[1] += 1;
                    &de * &de + &oe * &oe * int(2)
                }
                _ => continue,
            };
            for r in [&r1, &r2] {
                let q = r / &reference;
                if q < lo2 || q > hi2 {
                    let off = (q - &one).to_f64().unwrap() / ANGULAR.get();
                    errs.push(format!(
                        "{cl:?}, c = {c}, a = {i}/40 ε: squared ratio 1 + {off} ε"
                    ));
                }
            }
        }
    }
    assert!(errs.is_empty(), "{errs:#?}");
    assert!(
        kinds[0] > 10 && kinds[1] > 10,
        "kinds near the boundary: {kinds:?}"
    );
}

/// The 48 signed permutations, as grid transforms with the given translation.
fn signed_permutations(t: [i64; 3]) -> Vec<GridTransform> {
    let perms = [
        [Axis::X, Axis::Y, Axis::Z],
        [Axis::X, Axis::Z, Axis::Y],
        [Axis::Y, Axis::X, Axis::Z],
        [Axis::Y, Axis::Z, Axis::X],
        [Axis::Z, Axis::X, Axis::Y],
        [Axis::Z, Axis::Y, Axis::X],
    ];
    let mut out = Vec::new();
    for p in perms {
        for bits in 0..8u8 {
            let neg = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0];
            out.push(GridTransform::from_parts(p, neg, t).unwrap());
        }
    }
    out
}

fn det3(m: [[i64; 3]; 3]) -> i64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Classification of small integer matrices against the exact determinant and Gram matrix:
/// Degenerate exactly when det = 0, mirror exactly when det < 0, Rigid exactly when `LᵀL` = I,
/// `UniformScale` exactly when `LᵀL` = s²I with s² ≠ 1.
#[test]
fn transform_classification_of_integer_matrices_matches_exact_gram() {
    let mut rng = Rng(0xC1A5_5157);
    let mut mats: Vec<[[i64; 3]; 3]> = Vec::new();
    for g in signed_permutations([0; 3]) {
        let m = g.matrix().map(|r| r.map(i64::from));
        for k in [1, 2, -3] {
            mats.push(m.map(|r| r.map(|v| v * k)));
        }
    }
    mats.push([[1, 2, 2], [2, 1, -2], [2, -2, 1]]);
    mats.push([[-1, 2, 2], [2, -1, 2], [2, 2, -1]]);
    for _ in 0..3000 {
        mats.push([[0; 3]; 3].map(|r| r.map(|_| i64::try_from(rng.range(-2, 2)).unwrap())));
    }
    let mut f = Failures::new();
    for m in mats {
        let tr = if rng.coin() {
            [0.0; 3]
        } else {
            [3.0, -7.5, 1e9]
        };
        let rows = [0, 1, 2].map(|i| [m[i][0] as f64, m[i][1] as f64, m[i][2] as f64, tr[i]]);
        let t = Transform::from_rows(rows).unwrap();
        let c = t.classify().unwrap();
        let det = det3(m);
        let gram = |i: usize, j: usize| (0..3).map(|k| m[k][i] * m[k][j]).sum::<i64>();
        let identity = m == [[1, 0, 0], [0, 1, 0], [0, 0, 1]];
        let g00 = gram(0, 0);
        let want_gram = |i: usize, j: usize| if i == j { g00 } else { 0 };
        let scaled_identity = (0..3).all(|i| (0..3).all(|j| gram(i, j) == want_gram(i, j)));
        let want_kind = if identity {
            if tr == [0.0; 3] {
                TransformKind::Identity
            } else {
                TransformKind::Translation
            }
        } else if det == 0 {
            TransformKind::Degenerate
        } else if scaled_identity && g00 == 1 {
            TransformKind::Rigid
        } else if scaled_identity {
            TransformKind::UniformScale
        } else {
            TransformKind::NonUniform
        };
        let want_mirror = det < 0;
        let r = if c.kind == want_kind && c.mirror == want_mirror {
            let inv = t.inverse();
            match (want_kind, inv) {
                (TransformKind::Degenerate, Err(MathError::Singular)) => Ok(()),
                (TransformKind::Degenerate, other) => Err(format!("{m:?}: inverse {other:?}")),
                (_, Ok(_)) => Ok(()),
                (_, other) => Err(format!("{m:?}: inverse {other:?}")),
            }
        } else {
            Err(format!(
                "{m:?}: got {c:?}, expected {want_kind:?} mirror {want_mirror}"
            ))
        };
        f.record(r);
    }
    f.assert_none("integer classification");
}

/// The inverse's doc: the determinant it divides by lies inside the certified enclosure, so a
/// transform that is not `Degenerate` is never refused as `Singular`. Nearly singular matrices
/// (third row = first + second + 2^−k·noise) probe the edge.
#[test]
fn transform_inverse_never_refuses_a_certified_transform() {
    let mut rng = Rng(0x1A7E_25E0);
    let mut f = Failures::new();
    for _ in 0..20000 {
        let r0 = [0; 3].map(|_| rand_double(&mut rng, -3, 3));
        let r1 = [0; 3].map(|_| rand_double(&mut rng, -3, 3));
        let k = i32::try_from(rng.range(10, 70)).unwrap();
        let noise = [0; 3].map(|_| rand_double(&mut rng, -k - 2, -k));
        let r2 = [0, 1, 2].map(|i| r0[i] + r1[i] + noise[i]);
        let t = Transform::from_rows([
            [r0[0], r0[1], r0[2], 0.0],
            [r1[0], r1[1], r1[2], 0.0],
            [r2[0], r2[1], r2[2], 0.0],
        ])
        .unwrap();
        let c = t.classify().unwrap();
        let inv = t.inverse();
        let r = match (c.kind, inv) {
            (TransformKind::Degenerate, Err(MathError::Singular)) => Ok(()),
            (TransformKind::Degenerate, other) => Err(format!("Degenerate but inverse {other:?}")),
            (_, Err(MathError::Singular)) => Err(format!("{c:?} refused as Singular: {t:?}")),
            _ => Ok(()),
        };
        // The certified mirror flag agrees with the exact determinant's sign.
        let r = r.and_then(|()| {
            if c.kind == TransformKind::Degenerate {
                return Ok(());
            }
            let m = t
                .rows()
                .map(|row| [exact(row[0]), exact(row[1]), exact(row[2])]);
            let det = &m[0][0] * (&m[1][1] * &m[2][2] - &m[1][2] * &m[2][1])
                - &m[0][1] * (&m[1][0] * &m[2][2] - &m[1][2] * &m[2][0])
                + &m[0][2] * (&m[1][0] * &m[2][1] - &m[1][1] * &m[2][0]);
            if det.is_zero() || (det < BigRational::zero()) != c.mirror {
                Err(format!(
                    "{c:?} but the exact determinant is {}",
                    det.to_f64().unwrap()
                ))
            } else {
                Ok(())
            }
        });
        f.record(r);
    }
    f.assert_none("certified inverse");
}

fn bits(t: &Transform) -> [[u64; 4]; 3] {
    t.rows().map(|r| r.map(f64::to_bits))
}

/// Chapter 7: quarter turns stay exact. `Transform::rotation` by k half-quarter-turns must have
/// the same bits as the grid transform's quarter turns, for every axis, both directions.
#[test]
fn transform_rotation_by_quarter_turns_equals_the_grid_transform() {
    for axis in Axis::ALL {
        for k in -9..=9 {
            let t = Transform::rotation(axis, f64::from(k) * 0.5).unwrap();
            let g = Transform::from_grid(&GridTransform::quarter_turns(axis, k));
            assert_eq!(bits(&t), bits(&g), "axis {axis:?}, {k} quarter turns");
            assert_eq!(
                t.classify().unwrap().kind,
                if k.rem_euclid(4) == 0 {
                    TransformKind::Identity
                } else {
                    TransformKind::Rigid
                }
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Grid transforms, exhaustively.
// ---------------------------------------------------------------------------------------------

fn mat_mul(a: [[i64; 3]; 3], b: [[i64; 3]; 3]) -> [[i64; 3]; 3] {
    let mut c = [[0i64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            c[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    c
}

fn mat_vec(a: [[i64; 3]; 3], v: [i64; 3]) -> [i64; 3] {
    [0, 1, 2].map(|i| (0..3).map(|k| a[i][k] * v[k]).sum())
}

fn mat(g: &GridTransform) -> [[i64; 3]; 3] {
    g.matrix().map(|r| r.map(i64::from))
}

fn arr(p: P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

/// All 48 × 48 compositions against matrix products; inverses, determinants, images of points,
/// vectors and boxes against the matrix; the float transform of a composition against the
/// composition of float transforms, bit for bit; the classification of every grid transform.
#[test]
fn grid_transforms_exhaustively_against_matrices() {
    let mut rng = Rng(0x0048_0048);
    let mut f = Failures::new();
    let tr = |rng: &mut Rng| [0; 3].map(|_| i64::try_from(rng.range(-(1 << 30), 1 << 30)).unwrap());
    let all: Vec<GridTransform> = signed_permutations([0; 3])
        .into_iter()
        .map(|g| {
            GridTransform::translation(tr(&mut rng))
                .unwrap()
                .compose(&g)
                .unwrap()
        })
        .collect();
    for g in &all {
        let (mg, tg) = (mat(g), g.translation_part());
        let det = det3(mg);
        f.record(
            if i64::from(g.determinant()) == det && g.reverses_orientation() == (det < 0) {
                Ok(())
            } else {
                Err(format!("{g:?}: determinant {} vs {det}", g.determinant()))
            },
        );
        let inv = g.inverse().unwrap();
        f.record(
            if inv.compose(g) == Ok(GridTransform::IDENTITY)
                && g.compose(&inv) == Ok(GridTransform::IDENTITY)
            {
                Ok(())
            } else {
                Err(format!("{g:?}: inverse {inv:?} does not undo it"))
            },
        );
        let ft = Transform::from_grid(g);
        let fi = ft.inverse();
        f.record(match fi {
            Ok(fi) if bits(&fi) == bits(&Transform::from_grid(&inv)) => Ok(()),
            other => Err(format!("{g:?}: float inverse {other:?}")),
        });
        let c = ft.classify().unwrap();
        let kind_ok = matches!(
            c.kind,
            TransformKind::Rigid | TransformKind::Identity | TransformKind::Translation
        );
        f.record(if kind_ok && c.mirror == (det < 0) {
            Ok(())
        } else {
            Err(format!("{g:?} classified {c:?}"))
        });
        for _ in 0..4 {
            let p = P3::new(
                i64::try_from(rng.range(-(1 << 39), 1 << 39)).unwrap(),
                i64::try_from(rng.range(-(1 << 39), 1 << 39)).unwrap(),
                coord(&mut rng),
            )
            .unwrap();
            let want = mat_vec(mg, arr(p));
            let want = [0, 1, 2].map(|i| want[i] + tg[i]);
            let got = g.apply(p);
            let in_range = want.iter().all(|v| v.abs() <= COORD_MAX);
            f.record(match got {
                Ok(q) if in_range && arr(q) == want => Ok(()),
                Err(NumError::OutOfRange { .. }) if !in_range => Ok(()),
                other => Err(format!("{g:?}.apply({p:?}) = {other:?}, expected {want:?}")),
            });
            let v = V3::new(coord(&mut rng), coord(&mut rng), coord(&mut rng)).unwrap();
            let gv = g.apply_vector(v).unwrap();
            f.record(if gv.to_array() == mat_vec(mg, v.to_array()) {
                Ok(())
            } else {
                Err(format!("{g:?}.apply_vector({v:?}) = {gv:?}"))
            });
            // Boxes: the image box is the box of the eight images of the corners.
            let q = P3::new(
                i64::try_from(rng.range(-(1 << 38), 1 << 38)).unwrap(),
                i64::try_from(rng.range(-(1 << 38), 1 << 38)).unwrap(),
                i64::try_from(rng.range(-(1 << 38), 1 << 38)).unwrap(),
            )
            .unwrap();
            let b = Box3::from_points([p, q]).unwrap();
            let corners: Vec<[i64; 3]> = (0..8)
                .map(|k| {
                    let (lo, hi) = (arr(b.min()), arr(b.max()));
                    let c = [0, 1, 2].map(|i| if k >> i & 1 == 1 { hi[i] } else { lo[i] });
                    let m = mat_vec(mg, c);
                    [0, 1, 2].map(|i| m[i] + tg[i])
                })
                .collect();
            let bmin = [0, 1, 2].map(|i| corners.iter().map(|c| c[i]).min().unwrap());
            let bmax = [0, 1, 2].map(|i| corners.iter().map(|c| c[i]).max().unwrap());
            let in_range = corners.iter().flatten().all(|v| v.abs() <= COORD_MAX);
            f.record(match b.transformed(g) {
                Ok(t) if in_range && arr(t.min()) == bmin && arr(t.max()) == bmax => Ok(()),
                Err(NumError::OutOfRange { .. }) if !in_range => Ok(()),
                other => Err(format!("{g:?}: box {b:?} → {other:?}")),
            });
        }
        for h in &all {
            let (mh, th) = (mat(h), h.translation_part());
            let c = g.compose(h).unwrap();
            let want_t = mat_vec(mg, th);
            let want_t = [0, 1, 2].map(|i| want_t[i] + tg[i]);
            f.record(
                if mat(&c) == mat_mul(mg, mh) && c.translation_part() == want_t {
                    Ok(())
                } else {
                    Err(format!("{g:?} ∘ {h:?} = {c:?}"))
                },
            );
            let fc = Transform::from_grid(g)
                .compose(&Transform::from_grid(h))
                .unwrap();
            f.record(if bits(&fc) == bits(&Transform::from_grid(&c)) {
                Ok(())
            } else {
                Err(format!(
                    "float composition of {g:?} and {h:?} differs: {fc:?}"
                ))
            });
        }
    }
    f.assert_none("grid transforms");
}

#[test]
fn grid_transform_translation_limits() {
    let max = GridTransform::translation([TRANSLATION_MAX, 0, -TRANSLATION_MAX]).unwrap();
    assert!(GridTransform::translation([TRANSLATION_MAX + 1, 0, 0]).is_err());
    assert!(GridTransform::translation([0, 0, -TRANSLATION_MAX - 1]).is_err());
    let one = GridTransform::translation([1, 0, 0]).unwrap();
    assert!(matches!(
        max.compose(&one),
        Err(NumError::OutOfRange { .. })
    ));
    let back = GridTransform::translation([-1, 0, 0]).unwrap();
    assert!(max.compose(&back).is_ok());
    // The extreme translation maps −M to M and nothing else onto the grid.
    let p = P3::new(-COORD_MAX, 0, COORD_MAX).unwrap();
    assert_eq!(arr(max.apply(p).unwrap()), [COORD_MAX, 0, -COORD_MAX]);
    assert!(
        max.apply(P3::new(-COORD_MAX + 1, 0, COORD_MAX).unwrap())
            .is_err()
    );
    assert!(max.apply(P3::new(0, 0, 0).unwrap()).is_err());
    // A mirrored extreme translation inverts to itself.
    let m = GridTransform::from_parts(
        [Axis::X, Axis::Y, Axis::Z],
        [true, false, false],
        [TRANSLATION_MAX, 0, 0],
    )
    .unwrap();
    assert_eq!(m.inverse(), Ok(m));
}

// ---------------------------------------------------------------------------------------------
// Grid vectors at the bit budget.
// ---------------------------------------------------------------------------------------------

fn bi(v: i64) -> BigInt {
    BigInt::from(v)
}

/// Every product at its largest documented operands, against big integers. In a release build an
/// i128 overflow would wrap silently, so this is the only check that the budgets hold.
#[test]
fn vectors_at_the_bit_budget_extremes() {
    let d = VEC_MAX;
    let comps = [-d, -d + 1, 0, d - 1, d];
    let mut vs = Vec::new();
    for x in comps {
        for y in comps {
            for z in comps {
                vs.push([x, y, z]);
            }
        }
    }
    let mut f = Failures::new();
    let mut rng = Rng(0x7E1C_7012);
    for i in 0..vs.len() {
        for j in 0..vs.len() {
            let (a, b) = (vs[i], vs[j]);
            let (va, vb) = (
                V3::new(a[0], a[1], a[2]).unwrap(),
                V3::new(b[0], b[1], b[2]).unwrap(),
            );
            let cross = va.cross(vb).unwrap();
            let want = [
                bi(a[1]) * bi(b[2]) - bi(a[2]) * bi(b[1]),
                bi(a[2]) * bi(b[0]) - bi(a[0]) * bi(b[2]),
                bi(a[0]) * bi(b[1]) - bi(a[1]) * bi(b[0]),
            ];
            let got = [cross.x(), cross.y(), cross.z()].map(BigInt::from);
            let dot = bi(a[0]) * bi(b[0]) + bi(a[1]) * bi(b[1]) + bi(a[2]) * bi(b[2]);
            f.record(if got == want && BigInt::from(va.dot(vb).unwrap()) == dot {
                Ok(())
            } else {
                Err(format!("cross/dot of {a:?}, {b:?}"))
            });
            // A random third vector from the same extremes.
            let c = vs[usize::try_from(rng.next() % vs.len() as u64).unwrap()];
            let vc = V3::new(c[0], c[1], c[2]).unwrap();
            let triple = &want[0] * bi(c[0]) + &want[1] * bi(c[1]) + &want[2] * bi(c[2]);
            // triple(self, b, c) = self · (b × c): here c · (a × b).
            f.record(
                if BigInt::from(vc.triple(va, vb).unwrap()) == triple
                    && BigInt::from(cross.dot(vc).unwrap()) == triple
                {
                    Ok(())
                } else {
                    Err(format!("triple of {c:?}, {a:?}, {b:?}"))
                },
            );
        }
    }
    // Sums and offsets leave the range only with an error.
    let top = V3::new(d, d, d).unwrap();
    let one = V3::new(1, 0, 0).unwrap();
    f.record(
        if top.add(one).is_err() && top.neg().unwrap().sub(one).is_err() && top.sub(one).is_ok() {
            Ok(())
        } else {
            Err("vector range at the top".into())
        },
    );
    f.record(
        if V3::new(d + 1, 0, 0).is_err() && V3::new(0, -d - 1, 0).is_err() {
            Ok(())
        } else {
            Err("constructor range".into())
        },
    );
    let edge = P3::new(COORD_MAX, -COORD_MAX, 0).unwrap();
    f.record(match (one.offset(edge), one.neg().unwrap().offset(edge)) {
        (Err(NumError::OutOfRange { .. }), Ok(p)) if p.x.get() == COORD_MAX - 1 => Ok(()),
        other => Err(format!("offset at the edge: {other:?}")),
    });
    f.assert_none("vector budgets");
}

// ---------------------------------------------------------------------------------------------
// Floating-point points and boxes.
// ---------------------------------------------------------------------------------------------

/// `normalized` documents `Degenerate` for "a vector shorter than about 10^−154 whose square
/// underflows", and otherwise returns the vector divided by its length. For 10^−160 the square
/// is subnormal (about 11 significant bits), so the "unit" vector is off by about 10^−4.
#[test]
fn float_normalized_tiny_vector_is_unit_or_refused() {
    let mut errs = Vec::new();
    for x in [1e-158, 1e-160, 3e-161] {
        match F2::new(x, 0.0).unwrap().normalized() {
            Err(NumError::Degenerate) => {}
            Ok(v) if math::abs(v.x() - 1.0).unwrap().get() <= 4.0 * f64::EPSILON => {}
            other => errs.push(format!("F2({x:e}, 0).normalized() = {other:?}")),
        }
        match F3::new(0.0, x, x).unwrap().normalized() {
            Err(NumError::Degenerate) => {}
            Ok(v) => {
                let len2 = v.y() * v.y() + v.z() * v.z();
                if math::abs(len2 - 1.0).unwrap().get() > 8.0 * f64::EPSILON {
                    errs.push(format!(
                        "F3(0, {x:e}, {x:e}).normalized() has |v|² = {len2}"
                    ));
                }
            }
            other => errs.push(format!("F3(0, {x:e}, {x:e}).normalized() = {other:?}")),
        }
    }
    assert!(errs.is_empty(), "{errs:#?}");
}

/// A transformed float box contains the exact image of every corner (hence, by convexity, of
/// the box), and its outward grid box contains it.
#[test]
fn float_boxes_contain_exact_images() {
    let mut rng = Rng(0xB0C5_B0C5);
    let mut f = Failures::new();
    for _ in 0..3000 {
        let rows = [0; 3].map(|_| {
            [
                rand_double(&mut rng, -6, 2),
                rand_double(&mut rng, -6, 2),
                rand_double(&mut rng, -6, 2),
                rand_double(&mut rng, -20, 30),
            ]
        });
        let t = Transform::from_rows(rows).unwrap();
        let a = F3::new(
            rand_double(&mut rng, -10, 34),
            rand_double(&mut rng, -10, 34),
            rand_double(&mut rng, -10, 34),
        )
        .unwrap();
        let b = F3::new(
            rand_double(&mut rng, -10, 34),
            rand_double(&mut rng, -10, 34),
            rand_double(&mut rng, -10, 34),
        )
        .unwrap();
        let bx = FBox3::from_point(a).union(&FBox3::from_point(b));
        let tb = bx.transformed(&t).unwrap();
        let (lo, hi) = (bx.lo().to_array(), bx.hi().to_array());
        let (tlo, thi) = (tb.lo().to_array(), tb.hi().to_array());
        let mut ok = true;
        for k in 0..8 {
            let c = [0, 1, 2].map(|i| {
                if k >> i & 1 == 1 {
                    exact(hi[i])
                } else {
                    exact(lo[i])
                }
            });
            for (i, row) in rows.iter().enumerate() {
                let img = exact(row[0]) * &c[0]
                    + exact(row[1]) * &c[1]
                    + exact(row[2]) * &c[2]
                    + exact(row[3]);
                if img < exact(tlo[i]) || img > exact(thi[i]) {
                    ok = false;
                }
            }
        }
        let g = tb.to_grid_outward();
        let grid_ok = match g {
            Ok(g) => (0..3).all(|i| {
                int(i128::from(arr(g.min())[i])) <= exact(tlo[i])
                    && int(i128::from(arr(g.max())[i])) >= exact(thi[i])
            }),
            Err(NumError::OutOfRange { .. }) => tlo
                .iter()
                .chain(thi.iter())
                .any(|v| v.abs() > (COORD_MAX - 1) as f64),
            Err(_) => false,
        };
        f.record(if ok && grid_ok {
            Ok(())
        } else {
            Err(format!("box {bx:?} under {t:?} gave {tb:?} / {g:?}"))
        });
    }
    f.assert_none("float boxes");
}

/// The bbox docs promise zero bounds stored as +0, so equal boxes have equal bits.
#[test]
fn float_box_zero_bounds_are_positive_zero() {
    let neg = F3::new(-0.0, -0.0, -0.0).unwrap();
    let pos = F3::new(0.0, 1.0, 0.0).unwrap();
    let boxes = [
        FBox3::new(neg, pos).unwrap(),
        FBox3::from_point(neg),
        FBox3::from_point(neg).union(&FBox3::from_point(pos)),
        FBox3::from_point(pos).union(&FBox3::from_point(neg)),
        FBox3::new(neg, pos)
            .unwrap()
            .intersection(&FBox3::from_point(neg))
            .unwrap(),
        FBox3::from_point(neg)
            .transformed(&Transform::mirror(Axis::X))
            .unwrap(),
    ];
    for b in boxes {
        for v in b.lo().to_array().into_iter().chain(b.hi().to_array()) {
            assert!(v.to_bits() != (-0.0f64).to_bits(), "a −0 bound in {b:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Frames: oracles.
// ---------------------------------------------------------------------------------------------

fn s_int(step: GridStep) -> i128 {
    i128::from(steps_per_metre(step))
}

/// The exact horizontal frame coordinates of map coordinates (e, n): X = ax/ρ and Y = ay/ρ, with
/// ρ = r (an integer) or √n2.
struct Horizontal {
    ax: BigRational,
    ay: BigRational,
    r: Option<u32>,
    n2: u64,
}

fn horizontal(f: &FrameRecord, e: f64, n: f64) -> Horizontal {
    let s = int(s_int(f.step()));
    let [oe, on, _] = f.origin();
    let u = exact(e) * &s - int(oe.into());
    let v = exact(n) * &s - int(on.into());
    let c = int(f.rotation().cos().into());
    let sn = int(f.rotation().sin().into());
    let kd = int(f.scale().den().into());
    let kn = int(f.scale().num().into());
    let ax = &kd * (&c * &u + &sn * &v) / &kn;
    let ay = &kd * (&c * &v - &sn * &u) / &kn;
    Horizontal {
        ax,
        ay,
        r: f.rotation().exact_norm(),
        n2: f.rotation().norm2(),
    }
}

/// Whether `res` is a/√n rounded to the nearest integer: |a/√n − res| < 1/2, exactly (a/√n is
/// irrational unless a = 0, so it is never a midpoint).
fn nearest_to_sqrt_ratio(res: i64, a: &BigRational, n: u64) -> bool {
    let half = BigRational::new(BigInt::one(), BigInt::from(2));
    let lo = int(res.into()) - &half;
    let hi = int(res.into()) + &half;
    sign_minus_sqrt(a, &lo, n) == Ordering::Greater && sign_minus_sqrt(a, &hi, n) == Ordering::Less
}

/// Whether a/√n rounds into the frame's range: |a/√n| < 2^40 − 1/2, exactly.
fn sqrt_ratio_in_range(a: &BigRational, n: u64) -> bool {
    let limit = int(COORD_MAX.into()) + BigRational::new(BigInt::one(), BigInt::from(2));
    sign_minus_sqrt(a, &limit, n) == Ordering::Less
        && sign_minus_sqrt(a, &(-limit), n) == Ordering::Greater
}

fn check_to_frame(f: &FrameRecord, m: MapPoint) -> Result<(), String> {
    let got = f.to_frame(m);
    let h = horizontal(f, m.easting, m.northing);
    let s = int(s_int(f.step()));
    let zx = exact(m.height) * &s - int(f.origin()[2].into());
    let ez = round_half_even(&zx);
    let max = BigInt::from(COORD_MAX);
    let fits = |v: &BigInt| v.abs() <= max;
    let ctx = || format!("{f:?} {m:?}");
    match h.r {
        Some(r) => {
            let rr = int(r.into());
            let (x, y) = (&h.ax / &rr, &h.ay / &rr);
            let (ex, ey) = (round_half_even(&x), round_half_even(&y));
            let exact_all = x.is_integer() && y.is_integer() && zx.is_integer();
            if fits(&ex) && fits(&ey) && fits(&ez) {
                match got {
                    Ok(p)
                        if BigInt::from(p.value().x.get()) == ex
                            && BigInt::from(p.value().y.get()) == ey
                            && BigInt::from(p.value().z.get()) == ez
                            && p.is_exact() == exact_all =>
                    {
                        Ok(())
                    }
                    other => Err(format!(
                        "{}: got {other:?}, expected ({ex}, {ey}, {ez}) exact {exact_all}",
                        ctx()
                    )),
                }
            } else {
                match got {
                    Err(NumError::OutOfRange { .. }) => Ok(()),
                    other => Err(format!("{}: expected OutOfRange, got {other:?}", ctx())),
                }
            }
        }
        // Fixer: irrational directions are now settled exactly too, so the oracle requires the
        // correctly rounded value and decides the range exactly (it accepted 1/2 + 2^−59 before).
        None => {
            let n = h.n2;
            let rho = math::sqrt(n as f64).unwrap().get();
            let (xa, ya) = (h.ax.to_f64().unwrap() / rho, h.ay.to_f64().unwrap() / rho);
            let in_range =
                sqrt_ratio_in_range(&h.ax, n) && sqrt_ratio_in_range(&h.ay, n) && fits(&ez);
            let want_exact = h.ax.is_zero() && h.ay.is_zero() && zx.is_integer();
            match got {
                Ok(p) => {
                    let ok = nearest_to_sqrt_ratio(p.value().x.get(), &h.ax, n)
                        && nearest_to_sqrt_ratio(p.value().y.get(), &h.ay, n)
                        && BigInt::from(p.value().z.get()) == ez
                        && p.is_exact() == want_exact
                        && in_range;
                    if ok {
                        Ok(())
                    } else {
                        Err(format!(
                            "{}: got {p:?}; exact ≈ ({xa}, {ya}, {ez}), exact flag {want_exact}",
                            ctx()
                        ))
                    }
                }
                Err(NumError::OutOfRange { .. }) if !in_range => Ok(()),
                Err(e) => Err(format!("{}: {e:?}; exact ≈ ({xa}, {ya}, {ez})", ctx())),
            }
        }
    }
}

/// Map coordinates as R + Q/√n2 per horizontal axis (Q = 0 when r is an integer), and the height.
fn exact_map(f: &FrameRecord, p: P3) -> [BigRational; 5] {
    let s = int(s_int(f.step()));
    let [oe, on, oh] = f.origin().map(|v| int(v.into()));
    let (x, y, z) = (
        i128::from(p.x.get()),
        i128::from(p.y.get()),
        i128::from(p.z.get()),
    );
    let (c, sn) = (
        i128::from(f.rotation().cos()),
        i128::from(f.rotation().sin()),
    );
    let (ae, an) = (int(c * x - sn * y), int(sn * x + c * y));
    let kn = int(f.scale().num().into());
    let kd = int(f.scale().den().into());
    let height = (oh + int(z)) / &s;
    if let Some(r) = f.rotation().exact_norm() {
        let r = int(r.into());
        let e = (&oe + &kn * &ae / (&kd * &r)) / &s;
        let n = (&on + &kn * &an / (&kd * &r)) / &s;
        [e, BigRational::zero(), n, BigRational::zero(), height]
    } else {
        let qe = &kn * &ae / (&kd * &s);
        let qn = &kn * &an / (&kd * &s);
        [oe / &s, qe, on / &s, qn, height]
    }
}

/// Whether d is the double nearest R + Q/√n (Q ≠ 0, so the value is irrational and never a
/// midpoint): it lies strictly between the midpoints of d with its two neighbours, exactly.
///
/// Fixer: this was "within an ulp"; the frame now returns the correctly rounded double for
/// irrational directions too, so the oracle requires it.
fn map_nearest(d: f64, r: &BigRational, q: &BigRational, n: u64) -> bool {
    let two = int(2);
    let lo = (exact(d.next_down()) + exact(d)) / &two - r;
    let hi = (exact(d.next_up()) + exact(d)) / &two - r;
    // R + Q/√n > R + lo ⇔ Q > lo·√n.
    sign_minus_sqrt(q, &lo, n) == Ordering::Greater && sign_minus_sqrt(q, &hi, n) == Ordering::Less
}

fn check_to_map(f: &FrameRecord, p: P3) -> Result<(), String> {
    let got = f
        .to_map(p)
        .map_err(|e| format!("{f:?}.to_map({p:?}) failed: {e:?}"))?;
    let [re, qe, rn, qn, h] = exact_map(f, p);
    let m = got.value();
    let ctx = || format!("{f:?}.to_map({p:?}) = {got:?}");
    if !is_nearest_double(m.height, &h) {
        return Err(format!("{}: height not nearest", ctx()));
    }
    let n2 = f.rotation().norm2();
    let axis = |d: f64, r: &BigRational, q: &BigRational| -> (bool, bool) {
        if q.is_zero() {
            (is_nearest_double(d, r), exact(d) == *r)
        } else {
            (map_nearest(d, r, q, n2), false)
        }
    };
    let (e_ok, e_exact) = axis(m.easting, &re, &qe);
    let (n_ok, n_exact) = axis(m.northing, &rn, &qn);
    let want_exact = e_exact && n_exact && exact(m.height) == h;
    if e_ok && n_ok && got.is_exact() == want_exact {
        Ok(())
    } else {
        let c = ctx();
        Err(format!(
            "{c}: easting ok {e_ok}, northing ok {n_ok}, exact should be {want_exact}"
        ))
    }
}

const DIRECTIONS: [(i32, i32); 18] = [
    (1, 0),
    (0, 1),
    (-1, 0),
    (0, -1),
    (3, 4),
    (-4, 3),
    (4, -3),
    (-3, -4),
    (5, 12),
    (20, 21),
    (2_147_395_599, 92_680),
    (-92_680, 2_147_395_599),
    (1, 1),
    (1, 2),
    (-7, 3),
    (i32::MAX, 1),
    (i32::MIN, i32::MAX),
    (i32::MIN + 1, i32::MIN),
];

const SCALES: [(u64, u64); 10] = [
    (1, 1),
    (1, 2),
    (2, 1),
    (9_996, 10_000),
    (99_987_654, 100_000_000),
    (4_294_967_291, 4_294_967_295),
    (2_147_483_648, 4_294_967_291),
    (4_294_967_291, 2_147_483_648),
    // Ratios outside [1/2, 2], which chapter 17.1 allows.
    (7, 1_000),
    (4_000_000_007, 3),
];

const STEPS: [GridStep; 3] = [GridStep::Q0, GridStep::Q1, GridStep::Q2];

fn origin(rng: &mut Rng) -> [i64; 3] {
    let o = |rng: &mut Rng| -> i64 {
        match rng.next() % 5 {
            0 => rng.pick(&[0, ORIGIN_MAX, -ORIGIN_MAX, ORIGIN_MAX - 1, 1]),
            1 => i64::try_from(rng.range(-(1 << 46), 1 << 46)).unwrap(),
            2 => i64::try_from(rng.range(-1000, 1000)).unwrap(),
            _ => i64::try_from(rng.range(-i128::from(ORIGIN_MAX), i128::from(ORIGIN_MAX))).unwrap(),
        }
    };
    [o(rng), o(rng), o(rng)]
}

fn frame(rng: &mut Rng, dir: (i32, i32), scale: (u64, u64), step: GridStep) -> FrameRecord {
    FrameRecord::new(
        CrsId::Epsg(2193),
        step,
        origin(rng),
        Rotation::direction(dir.0, dir.1).unwrap(),
        Scale::new(scale.0, scale.1).unwrap(),
    )
    .unwrap()
}

fn nudge(x: f64, k: i32) -> f64 {
    let mut v = x;
    for _ in 0..k.unsigned_abs() {
        v = if k > 0 { v.next_up() } else { v.next_down() };
    }
    v
}

/// Map points that stress the conversion: images of grid points nudged by a few ulps (near whole
/// steps), averages of neighbouring images (near midpoints), and multiples of 2^−12 m (whole and
/// half steps on every step size, since S = 2^11·odd).
fn hostile_map_points(rng: &mut Rng, f: &FrameRecord) -> Vec<MapPoint> {
    let mut out = Vec::new();
    let s = steps_per_metre(f.step()) as f64;
    for _ in 0..6 {
        let p = point(rng);
        let Ok(m) = f.to_map(p) else { continue };
        let m = m.value();
        for (de, dn, dh) in [(0, 0, 0), (1, -1, 2), (-2, 3, -1), (40, -40, 0)] {
            out.push(MapPoint {
                easting: nudge(m.easting, de),
                northing: nudge(m.northing, dn),
                height: nudge(m.height, dh),
            });
        }
        let q = P3::new(
            (p.x.get() + 1).min(COORD_MAX),
            (p.y.get() - 1).max(-COORD_MAX),
            (p.z.get() + 1).min(COORD_MAX),
        )
        .unwrap();
        if let Ok(m2) = f.to_map(q) {
            let m2 = m2.value();
            out.push(MapPoint {
                easting: f64::midpoint(m.easting, m2.easting),
                northing: f64::midpoint(m.northing, m2.northing),
                height: f64::midpoint(m.height, m2.height),
            });
        }
    }
    // Multiples of 2^−12 m near the origin.
    let [oe, on, oh] = f.origin();
    for _ in 0..6 {
        let near = |o: i64, rng: &mut Rng| -> f64 {
            let du = f64::from(i32::try_from(rng.range(-(1 << 30), 1 << 30)).unwrap())
                * f64::from(rng.pick(&[1u16, 7, 64, 255]));
            ((o as f64 + du) * 4096.0 / s).round() / 4096.0
        };
        out.push(MapPoint {
            easting: near(oe, rng),
            northing: near(on, rng),
            height: near(oh, rng),
        });
    }
    out
}

/// `to_frame` against the exact oracle on every direction (quarter turns, Pythagorean, the largest
/// i32 directions, irrational), every scale (the 1/2 and 2 limits, 32-bit coprime ratios), every
/// step, and origins up to ±2^60.
#[test]
fn frame_to_frame_matches_the_exact_oracle() {
    let mut rng = Rng(0xF2A3_E000);
    let mut f = Failures::new();
    for dir in DIRECTIONS {
        for scale in SCALES {
            for step in STEPS {
                let fr = frame(&mut rng, dir, scale, step);
                for m in hostile_map_points(&mut rng, &fr) {
                    f.record(check_to_frame(&fr, m));
                }
            }
        }
    }
    f.assert_none("to_frame");
}

/// `to_map` against the exact oracle: correctly rounded for every frame (for irrational directions
/// too, since the fix), with the exact flag right.
#[test]
fn frame_to_map_matches_the_exact_oracle() {
    let mut rng = Rng(0x7A9F_0000);
    let mut f = Failures::new();
    for dir in DIRECTIONS {
        for scale in SCALES {
            for step in STEPS {
                let fr = frame(&mut rng, dir, scale, step);
                for _ in 0..12 {
                    f.record(check_to_map(&fr, point(&mut rng)));
                }
                for sx in [-1, 1] {
                    let e = sx * COORD_MAX;
                    f.record(check_to_map(&fr, P3::new(e, -e, e).unwrap()));
                }
            }
        }
    }
    f.assert_none("to_map");
}

/// The frame module documented `to_map` for an irrational direction as "within half an ulp plus a
/// relative 2^−100", and the builder reported "within an ulp". When the origin nearly cancels the
/// rotated offset, the double-double error of the offset is magnified: direction (1, 1),
/// x − y = 2q and `O_E` = −p, with p² − 2q² = ±1 (Pell), puts the easting at (q√2 − p)/S, about
/// 1/(2√2·q·S) metres, and the error of q√2 (about 2^−106·q) is then a relative 2^−105·q².
/// Since the fix, `check_to_map` requires the correctly rounded double, which is stronger than the
/// name of this test says.
#[test]
fn frame_to_map_irrational_with_cancellation_is_within_an_ulp() {
    let mut pells = Vec::new();
    let (mut p, mut q) = (1i64, 1i64);
    while q + p <= COORD_MAX {
        let (np, nq) = (p + 2 * q, p + q);
        p = np;
        q = nq;
        pells.push((p, q));
    }
    let mut errs = Vec::new();
    for &(p, q) in pells.iter().rev().take(6) {
        let fr = FrameRecord::new(
            CrsId::Local(1),
            GridStep::Q0,
            [-p, 0, 0],
            Rotation::direction(1, 1).unwrap(),
            Scale::ONE,
        )
        .unwrap();
        let pt = P3::new(q, -q, 0).unwrap();
        if let Err(e) = check_to_map(&fr, pt) {
            // How far off, in ulps, from a 200-bit approximation of q√2.
            let k = 200u32;
            let root = ((BigInt::from(2) * BigInt::from(q) * BigInt::from(q)) << (2 * k)).sqrt();
            let val = (BigRational::new(root, BigInt::one() << k) - int(p.into()))
                / int(s_int(GridStep::Q0));
            let got = fr.to_map(pt).unwrap().value().easting;
            let ulps = (exact(got) - &val).abs()
                / (exact(math::abs(got).unwrap().get().next_up())
                    - exact(math::abs(got).unwrap().get()));
            errs.push(format!(
                "p = {p}, q = {q}: easting {got:e}, true ≈ {:e}, off by {:.3e} ulps ({e})",
                val.to_f64().unwrap(),
                ulps.to_f64().unwrap()
            ));
        }
    }
    assert!(errs.is_empty(), "{errs:#?}");
}

/// `shift_to` against the exact lattice oracle, and every accepted shift moves points to the same
/// exact map position.
#[test]
fn frame_shift_matches_the_exact_oracle() {
    let mut rng = Rng(0x5A1F_7000);
    let mut f = Failures::new();
    let dirs = [
        (1, 0),
        (0, 1),
        (-1, 0),
        (0, -1),
        (3, 4),
        (-5, 12),
        (2_147_395_599, 92_680),
        (1, 2),
    ];
    let scales = [(1, 1), (1, 2), (2, 1), (9_996, 10_000), (3, 5)];
    for _ in 0..4000 {
        let dir = rng.pick(&dirs);
        let scale = rng.pick(&scales);
        let rot = Rotation::direction(dir.0, dir.1).unwrap();
        let sc = Scale::new(scale.0, scale.1).unwrap();
        let from = rng.pick(&STEPS);
        let to = rng.pick(&STEPS);
        let a_origin = [0; 3].map(|_| i64::try_from(rng.range(-(1 << 50), 1 << 50)).unwrap());
        let a = FrameRecord::new(CrsId::Epsg(2193), from, a_origin, rot, sc).unwrap();
        let rungs = to.rung().checked_sub(from.rung());
        let fac = 25i128.pow(rungs.unwrap_or(0));
        // ΔO: on the lattice by construction (k_n·M·t), or random.
        let (c, s) = (i128::from(rot.cos()), i128::from(rot.sin()));
        let kn = i128::from(sc.num());
        let delta = if rng.coin() {
            let t = [0; 2].map(|_| rng.range(-3, 3));
            [
                kn * (c * t[0] - s * t[1]),
                kn * (s * t[0] + c * t[1]),
                rng.range(-1000, 1000),
            ]
        } else {
            [0; 3].map(|_| rng.range(-(1 << 20), 1 << 20))
        };
        let b_origin = [0, 1, 2].map(|i| fac * i128::from(a_origin[i]) - delta[i]);
        let Ok(b_origin) = b_origin
            .iter()
            .map(|v| i64::try_from(*v))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| [v[0], v[1], v[2]])
        else {
            continue;
        };
        let Ok(b) = FrameRecord::new(CrsId::Epsg(2193), to, b_origin, rot, sc) else {
            continue;
        };
        let got = a.shift_to(&b);
        if rungs.is_none() {
            f.record(match got {
                Err(MathError::Num(NumError::WrongDirection)) => Ok(()),
                other => Err(format!("coarsening shift {a:?} → {b:?}: {other:?}")),
            });
            continue;
        }
        let (de, dn, dh) = (int(delta[0]), int(delta[1]), delta[2]);
        let kd = int(sc.den().into());
        let expected: Result<[BigRational; 2], ()> = match rot.exact_norm() {
            Some(r) => {
                let den = int(kn) * int(r.into());
                let (cq, sq) = (int(c), int(s));
                Ok([
                    &kd * (&cq * &de + &sq * &dn) / &den,
                    &kd * (&cq * &dn - &sq * &de) / &den,
                ])
            }
            None if de.is_zero() && dn.is_zero() => Ok([BigRational::zero(), BigRational::zero()]),
            None => Err(()),
        };
        let r = match (expected, got) {
            (Err(()), Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice))) => Ok(()),
            (Ok([ox, oy]), got) if !(ox.is_integer() && oy.is_integer()) => match got {
                Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice)) => Ok(()),
                other => Err(format!(
                    "{a:?} → {b:?}: offset ({ox}, {oy}) but got {other:?}"
                )),
            },
            (Ok([ox, oy]), Ok(shift)) => {
                let want = [ox.to_integer(), oy.to_integer(), BigInt::from(dh)];
                let have = shift.offset().map(BigInt::from);
                if have == want && i128::from(shift.refine()) == fac {
                    // Points move to the same exact map position.
                    let mut res = Ok(());
                    for _ in 0..4 {
                        let p = point(&mut rng);
                        let want_q: Vec<i128> = [0, 1, 2]
                            .iter()
                            .map(|&i| fac * i128::from(arr(p)[i]) + i128::from(shift.offset()[i]))
                            .collect();
                        let fits = want_q.iter().all(|v| v.abs() <= i128::from(COORD_MAX));
                        match shift.apply(p) {
                            Ok(q) if fits => {
                                if want_q != arr(q).map(i128::from).to_vec()
                                    || exact_map(&a, p) != exact_map(&b, q)
                                {
                                    res = Err(format!("{a:?} → {b:?}: {p:?} ↦ {q:?} moved"));
                                }
                            }
                            Err(NumError::OutOfRange { .. }) if !fits => {}
                            other => res = Err(format!("apply({p:?}) = {other:?}")),
                        }
                    }
                    res
                } else {
                    Err(format!("{a:?} → {b:?}: offset {have:?}, expected {want:?}"))
                }
            }
            (exp, got) => Err(format!("{a:?} → {b:?}: expected {exp:?}, got {got:?}")),
        };
        f.record(r);
    }
    // Mismatches, in the documented order.
    let base = FrameRecord::map(CrsId::Epsg(2193), GridStep::Q0, [0; 3]).unwrap();
    let other_crs = FrameRecord::map(CrsId::Epsg(2105), GridStep::Q1, [0; 3]).unwrap();
    let rotated = FrameRecord::new(
        CrsId::Epsg(2193),
        GridStep::Q0,
        [0; 3],
        Rotation::quarter_turns(1),
        Scale::ONE,
    )
    .unwrap();
    let scaled = FrameRecord::new(
        CrsId::Epsg(2193),
        GridStep::Q0,
        [0; 3],
        Rotation::IDENTITY,
        Scale::new(9_996, 10_000).unwrap(),
    )
    .unwrap();
    f.record(
        match (
            base.shift_to(&other_crs),
            base.shift_to(&rotated),
            base.shift_to(&scaled),
        ) {
            (
                Err(MathError::FrameMismatch(FrameMismatch::Crs)),
                Err(MathError::FrameMismatch(FrameMismatch::Rotation)),
                Err(MathError::FrameMismatch(FrameMismatch::Scale)),
            ) => Ok(()),
            other => Err(format!("mismatch kinds: {other:?}")),
        },
    );
    // The largest shift: Q0 → Q2 with origins ±2^60 gives 625·2^60 + 2^60 > SHIFT_MAX.
    let far_a = FrameRecord::map(CrsId::Epsg(2193), GridStep::Q0, [ORIGIN_MAX, 0, 0]).unwrap();
    let far_b = FrameRecord::map(CrsId::Epsg(2193), GridStep::Q2, [-ORIGIN_MAX, 0, 0]).unwrap();
    f.record(match far_a.shift_to(&far_b) {
        Err(MathError::Num(NumError::OutOfRange { value })) if value > i128::from(SHIFT_MAX) => {
            Ok(())
        }
        other => Err(format!("625·2^60 + 2^60 shift: {other:?}")),
    });
    f.assert_none("shift_to");
}

/// G48: round trips at NZTM extremes and at ±(2^40 − 1); the frame docs: a q0 map frame round
/// trip is the identity below 2^29 m.
#[test]
fn frame_round_trips_near_nztm_and_at_the_range_edges() {
    let mut rng = Rng(0x2193_2193);
    let mut f = Failures::new();
    let s = s_int(GridStep::Q0);
    let nztm = [
        i64::try_from(1_757_000 * s).unwrap(),
        i64::try_from(5_921_000 * s).unwrap(),
        i64::try_from(50 * s).unwrap(),
    ];
    let far = i64::try_from((1i128 << 29) * s).unwrap() - COORD_MAX;
    let frames = [
        FrameRecord::map(CrsId::Epsg(2193), GridStep::Q0, nztm).unwrap(),
        FrameRecord::map(CrsId::Epsg(2193), GridStep::Q0, [far, far, 0]).unwrap(),
        FrameRecord::new(
            CrsId::Epsg(2193),
            GridStep::Q0,
            nztm,
            Rotation::direction(3, 4).unwrap(),
            Scale::new(9_996, 10_000).unwrap(),
        )
        .unwrap(),
        FrameRecord::new(
            CrsId::Epsg(2193),
            GridStep::Q0,
            nztm,
            Rotation::direction(1, 2).unwrap(),
            Scale::new(99_987_654, 100_000_000).unwrap(),
        )
        .unwrap(),
        FrameRecord::new(
            CrsId::Epsg(2193),
            GridStep::Q0,
            [0, i64::try_from(10_000_000 * s).unwrap(), 0],
            Rotation::quarter_turns(3),
            Scale::ONE,
        )
        .unwrap(),
    ];
    for fr in &frames {
        let mut pts: Vec<P3> = (0..3000).map(|_| point(&mut rng)).collect();
        for x in [-COORD_MAX, COORD_MAX] {
            for y in [-COORD_MAX, COORD_MAX] {
                pts.push(P3::new(x, y, -x).unwrap());
            }
        }
        for p in pts {
            let r = match fr
                .to_map(p)
                .and_then(|m| fr.to_frame(m.value()).map(|b| (m, b)))
            {
                Ok((_, back)) if back.value() == p => Ok(()),
                other => Err(format!("{fr:?}: {p:?} → {other:?}")),
            };
            f.record(r);
        }
    }
    f.assert_none("round trips");
}

/// In an irrational frame only the origin converts exactly; a point on the frame's own y axis has
/// x = 0 exactly but an irrational y, so the point is not exact.
#[test]
fn frame_irrational_exactness_flag() {
    let s = s_int(GridStep::Q0);
    let fr = FrameRecord::new(
        CrsId::Local(7),
        GridStep::Q0,
        [
            i64::try_from(1234 * s).unwrap(),
            i64::try_from(-77 * s).unwrap(),
            5,
        ],
        Rotation::direction(1, 2).unwrap(),
        Scale::ONE,
    )
    .unwrap();
    let origin = fr.to_map(P3::new(0, 0, -5).unwrap()).unwrap();
    assert!(
        origin.is_exact(),
        "the origin's map position is exact: {origin:?}"
    );
    let back = fr.to_frame(origin.value()).unwrap();
    assert!(
        back.is_exact() && arr(back.value()) == [0, 0, -5],
        "{back:?}"
    );
    let on_axis = fr.to_map(P3::new(0, 7, 0).unwrap()).unwrap();
    assert!(!on_axis.is_exact());
    let back = fr.to_frame(on_axis.value()).unwrap();
    assert!(
        !back.is_exact() && arr(back.value()) == [0, 7, 0],
        "{back:?}"
    );
    // Tiny and subnormal inputs: never exact unless exactly at the origin.
    for e in [f64::from_bits(1), -f64::from_bits(1), 2.2e-308, 1e-300] {
        for fr in [
            FrameRecord::map(CrsId::Local(1), GridStep::Q2, [0, 3, 0]).unwrap(),
            FrameRecord::new(
                CrsId::Local(1),
                GridStep::Q2,
                [0, 0, 0],
                Rotation::direction(3, 4).unwrap(),
                Scale::new(2, 3).unwrap(),
            )
            .unwrap(),
            FrameRecord::new(
                CrsId::Local(1),
                GridStep::Q1,
                [0, 0, 0],
                Rotation::direction(1, 1).unwrap(),
                Scale::ONE,
            )
            .unwrap(),
        ] {
            let m = MapPoint {
                easting: e,
                northing: 0.0,
                height: e,
            };
            check_to_frame(&fr, m).unwrap();
        }
    }
}

#[test]
fn frame_records_reject_and_canonicalize() {
    let ok = |o| FrameRecord::map(CrsId::Epsg(2193), GridStep::Q0, o);
    assert!(ok([ORIGIN_MAX, -ORIGIN_MAX, 0]).is_ok());
    assert!(ok([ORIGIN_MAX + 1, 0, 0]).is_err());
    assert!(ok([0, 0, i64::MIN]).is_err());
    assert_eq!(
        Rotation::direction(6, 8).unwrap(),
        Rotation::direction(3, 4).unwrap()
    );
    assert_eq!(
        Rotation::direction(i32::MIN, 0).unwrap(),
        Rotation::quarter_turns(2)
    );
    assert_eq!(
        Rotation::direction(0, i32::MIN).unwrap(),
        Rotation::quarter_turns(-1)
    );
    let r = Rotation::direction(i32::MIN, i32::MIN).unwrap();
    assert_eq!((r.cos(), r.sin()), (-1, -1));
    let r = Rotation::direction(i32::MIN, i32::MAX).unwrap();
    assert_eq!(
        u128::from(r.norm2()),
        (1u128 << 62) + (u128::from(i32::MAX.unsigned_abs())).pow(2)
    );
    assert_eq!(r.exact_norm(), None);
    assert_eq!(Rotation::direction(0, 0), Err(NumError::Degenerate));
    assert_eq!(Rotation::direction(-3, -4).unwrap().exact_norm(), Some(5));
    assert_eq!(Scale::new(2, 4).unwrap(), Scale::new(1, 2).unwrap());
    assert_eq!(Scale::new(u64::MAX, u64::MAX).unwrap(), Scale::ONE);
    // Fixer: chapter 17.1 puts no limit on a Ground frame's scale ratio (the review found the
    // builder's [1/2, 2] limit to be its own rule), so any positive ratio of 32-bit integers is
    // accepted and reduced, and only a zero part is refused. Extreme ratios are checked against the
    // exact oracle in the frame sweeps below (SCALES).
    assert_eq!(Scale::new(3, 9).unwrap(), Scale::new(1, 3).unwrap());
    assert!(Scale::new(3, 1).is_ok() && Scale::new(1, u64::MAX).is_ok());
    assert!(Scale::new(0, 1).is_err());
    assert!(Scale::new(1, 0).is_err());
}

#[test]
fn frame_non_finite_and_out_of_range_inputs() {
    let fr = FrameRecord::new(
        CrsId::Epsg(2193),
        GridStep::Q0,
        [0; 3],
        Rotation::direction(1, 2).unwrap(),
        Scale::new(9_996, 10_000).unwrap(),
    )
    .unwrap();
    let map = FrameRecord::map(CrsId::Epsg(2193), GridStep::Q0, [0; 3]).unwrap();
    for f in [fr, map] {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for m in [
                MapPoint {
                    easting: bad,
                    northing: 0.0,
                    height: 0.0,
                },
                MapPoint {
                    easting: 0.0,
                    northing: bad,
                    height: 0.0,
                },
                MapPoint {
                    easting: 0.0,
                    northing: 0.0,
                    height: bad,
                },
            ] {
                assert_eq!(f.to_frame(m), Err(NumError::NotFinite), "{m:?}");
            }
        }
        for big in [pow2f(62), -pow2f(62), pow2f(62).next_up(), f64::MAX, 1e30] {
            let r = f.to_frame(MapPoint {
                easting: big,
                northing: 0.0,
                height: 0.0,
            });
            assert!(
                matches!(r, Err(NumError::OutOfRange { .. })),
                "{big:e}: {r:?}"
            );
        }
    }
    assert!(DD::from_f64(f64::NAN).is_err());
    assert!(DD::from_sum(1.0, f64::INFINITY).is_err());
    assert!(DD::from_f64(1.0).unwrap().add_f64(f64::NAN).is_err());
    assert!(DD::from_f64(1.0).unwrap().mul_f64(f64::INFINITY).is_err());
    // `geom-num`'s double-double refuses these as its own maths functions do.
    assert_eq!(
        DD::from_f64(1.0).unwrap().div(DD::ZERO),
        Err(NumError::NotFinite)
    );
    assert_eq!(
        DD::from_f64(-1.0).unwrap().sqrt(),
        Err(NumError::OutsideDomain)
    );
    assert!(
        DD::from_f64(f64::MAX)
            .unwrap()
            .add(DD::from_f64(f64::MAX).unwrap())
            .is_err()
    );
    assert!(Transform::from_rows([[f64::NAN, 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4]]).is_err());
    assert!(Transform::rotation(Axis::Z, f64::INFINITY).is_err());
    assert!(F3::new(0.0, f64::NAN, 0.0).is_err());
    assert!(
        F2::new(f64::MAX, 0.0)
            .unwrap()
            .add(F2::new(f64::MAX, 0.0).unwrap())
            .is_err()
    );
}

/// The frame module documented `ORIGIN_MAX` = 2^60 steps as "180,000 km at q0, 288,000 km at q2";
/// 2^60 steps at q0 are in fact about 180 million km.
///
/// Fixer: the documentation now says "about 180 million km at q0 and 288,000 km at q2", and this
/// test checks those figures (the original asserted the wrong 180,000 km it quoted).
#[test]
fn doc_origin_max_distance_at_q0() {
    let km = |step: GridStep| ORIGIN_MAX as f64 * f64::from(step.quarter_nm()) * 0.25e-9 / 1000.0;
    let q2 = km(GridStep::Q2);
    assert!(
        math::abs(q2 / 288_000.0 - 1.0).unwrap().get() < 0.01,
        "q2: {q2} km"
    );
    let q0 = km(GridStep::Q0);
    assert!(
        math::abs(q0 / 180e6 - 1.0).unwrap().get() < 0.01,
        "2^60 steps at q0 is {q0:.0} km, not 180 million km"
    );
}

fn is_midpoint(x: &BigRational) -> bool {
    x - BigRational::from_integer(x.floor().to_integer())
        == BigRational::new(BigInt::one(), BigInt::from(2))
}

/// Values on and a few ulps beside exact midpoints and whole steps, where the double-double value
/// cannot decide and the exact signs must: map coordinates j/4096 m (whole or half steps at every
/// step size) nudged by up to two ulps, small enough that an ulp is far below 2^−40 steps; j = 0
/// nudged gives subnormal inputs.
#[test]
fn frame_to_frame_settles_values_beside_midpoints() {
    let mut f = Failures::new();
    let (mut ties, mut beside) = (0u32, 0u32);
    let filter = pow2q(-40);
    let half = BigRational::new(BigInt::one(), BigInt::from(2));
    for dir in DIRECTIONS {
        for scale in SCALES {
            for step in STEPS {
                {
                    let o = if step == GridStep::Q1 {
                        [1, -2, 3]
                    } else {
                        [0, 0, 0]
                    };
                    let rot = Rotation::direction(dir.0, dir.1).unwrap();
                    let sc = Scale::new(scale.0, scale.1).unwrap();
                    let fr = FrameRecord::new(CrsId::Local(3), step, o, rot, sc).unwrap();
                    for j in -3i32..=3 {
                        for i in [-3i32, 4] {
                            for k in [-2, 0, 1] {
                                let e = nudge(f64::from(j) / 4096.0, k);
                                let n = nudge(f64::from(i) / 4096.0, -k);
                                let m = MapPoint {
                                    easting: e,
                                    northing: n,
                                    height: e,
                                };
                                let h = horizontal(&fr, e, n);
                                if let Some(r) = h.r {
                                    for a in [&h.ax, &h.ay] {
                                        let x = a / int(r.into());
                                        let fl = BigRational::from_integer(x.floor().to_integer());
                                        let d = (&x - &fl - &half).abs();
                                        if is_midpoint(&x) {
                                            ties += 1;
                                        } else if d < filter {
                                            beside += 1;
                                        }
                                    }
                                }
                                f.record(check_to_frame(&fr, m));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        ties > 100 && beside > 100,
        "too few hard cases: {ties} ties, {beside} beside"
    );
    eprintln!(
        "{} cases: {ties} exact midpoints, {beside} within 2^-40 of one",
        f.total
    );
    f.assert_none("to_frame beside midpoints");
}

/// Grid boxes against brute force on a small grid.
#[test]
fn grid_boxes_against_brute_force() {
    let mut rng = Rng(0xB0C5_0002);
    let mut f = Failures::new();
    let pt = |rng: &mut Rng| {
        P2::new(
            i64::try_from(rng.range(-4, 4)).unwrap(),
            i64::try_from(rng.range(-4, 4)).unwrap(),
        )
        .unwrap()
    };
    for _ in 0..3000 {
        let a = Box2::from_points([pt(&mut rng), pt(&mut rng)]).unwrap();
        let b = Box2::from_points([pt(&mut rng), pt(&mut rng)]).unwrap();
        let u = a.union(&b);
        let i = a.intersection(&b);
        let mut ok = u.contains_box(&a) && u.contains_box(&b);
        ok &= a.overlaps(&b) == i.is_some() && b.overlaps(&a) == a.overlaps(&b);
        for x in -5..=5 {
            for y in -5..=5 {
                let p = P2::new(x, y).unwrap();
                let both = a.contains(p) && b.contains(p);
                ok &= i.is_some_and(|i| i.contains(p)) == both;
                ok &= !(a.contains(p) || b.contains(p)) || u.contains(p);
            }
        }
        let e = a.extent().unwrap();
        ok &= e.x() == a.max().x.get() - a.min().x.get() && e.x() >= 0 && e.y() >= 0;
        f.record(if ok {
            Ok(())
        } else {
            Err(format!("{a:?} {b:?}"))
        });
    }
    f.record(
        if Box2::new(P2::new(1, 0).unwrap(), P2::new(0, 0).unwrap()).is_err() {
            Ok(())
        } else {
            Err("empty box accepted".into())
        },
    );
    f.assert_none("grid boxes");
}

// ---------------------------------------------------------------------------------------------
// Determinism across targets and thread counts.
// ---------------------------------------------------------------------------------------------

/// The workload's hash: SHA-256 (chapter 28.13), each value taken as its 8 little-endian bytes.
struct Digest(Sha256);

impl Digest {
    fn eat(&mut self, v: u64) {
        self.0.write(&v.to_le_bytes());
    }
}

/// The workload's SHA-256 digest, in hexadecimal.
fn workload(seed: u64) -> String {
    let mut rng = Rng(seed);
    let mut h = Digest(Sha256::new());
    let s = s_int(GridStep::Q0);
    let o = [
        i64::try_from(1_757_000 * s).unwrap(),
        i64::try_from(5_921_000 * s).unwrap(),
        0,
    ];
    let ground = |step, dir: (i32, i32), k: (u64, u64)| {
        let rot = Rotation::direction(dir.0, dir.1).unwrap();
        FrameRecord::new(
            CrsId::Epsg(2193),
            step,
            o,
            rot,
            Scale::new(k.0, k.1).unwrap(),
        )
        .unwrap()
    };
    let frames = [
        FrameRecord::map(CrsId::Epsg(2193), GridStep::Q0, o).unwrap(),
        ground(GridStep::Q1, (3, 4), (9_996, 10_000)),
        ground(GridStep::Q0, (1, 2), (99_987_654, 100_000_000)),
    ];
    for i in 0..1500 {
        let fr = frames[i % 3];
        let p = point(&mut rng);
        match fr.to_map(p) {
            Ok(m) => {
                h.eat(m.value().easting.to_bits());
                h.eat(m.value().northing.to_bits());
                h.eat(u64::from(m.is_exact()));
                let back = fr.to_frame(MapPoint {
                    easting: nudge(m.value().easting, 1),
                    ..m.value()
                });
                if let Ok(b) = back {
                    for v in arr(b.value()) {
                        h.eat(v as u64);
                    }
                    h.eat(u64::from(b.is_exact()));
                }
            }
            Err(_) => h.eat(1),
        }
        let a = rand_dd(&mut rng, -100, 100);
        let b = rand_dd(&mut rng, -100, 100);
        for r in [a.add(b), a.mul(b), a.div(b), a.sqrt()] {
            match r {
                Ok(d) => {
                    h.eat(d.hi().to_bits());
                    h.eat(d.lo().to_bits());
                }
                Err(_) => h.eat(2),
            }
        }
        let t = Transform::rotation(Axis::ALL[i % 3], rand_double(&mut rng, -10, 3)).unwrap();
        let stretch = Transform::scale(1.0, 1.0 + rand_double(&mut rng, -40, -20), 1.0).unwrap();
        let t = t.compose(&stretch);
        if let Ok(t) = t {
            let c = t.classify().unwrap();
            h.eat(c.kind as u64);
            h.eat(u64::from(c.mirror));
            if let Ok(inv) = t.inverse() {
                for v in inv.rows().iter().flatten() {
                    h.eat(v.to_bits());
                }
            }
        }
    }
    hex(h.0.finish())
}

/// The workload's golden in the golden store (`geom-test/golden/store.json`): its SHA-256 digest
/// on one thread. Every target the kernel builds for must give it (`kernel.yml` runs it in every
/// configuration through `golden run`, each time twice in one process on two differently perturbed
/// heaps), and the store's digest records these changes:
///
/// It replaced the workload's 64-bit FNV-1a digest, `0x32b6_dcb7_d33c_e019`, when the hash moved
/// to SHA-256 (chapter 28.13): both are hashes of the same bytes, so no result changed. It replaced
/// `c1c5d1b6…747c9` when piece G05 made `geom_num::math::sinpi` and `cospi`, which the workload's
/// rotations call, correctly rounded (ports of CORE-MATH in place of an msun-method
/// approximation): of the maths functions geom-math calls, only those two changed, so every
/// rotation now uses the correctly rounded sine and cosine.
const WORKLOAD: &str = "geom-math/workload";

/// Principle 5 and chapter 21: bit-identical results on every target. The workload covers map
/// conversions both ways, double-double arithmetic and transform classification and inversion;
/// its digest is pinned, so a target that rounds differently anywhere in it fails here.
#[test]
fn results_identical_across_targets() {
    let checked = golden::check(WORKLOAD, || {
        let one = workload(0x0D5E_ED00);
        let digest = golden::Digest::from_hex(&one).unwrap();
        (digest.0, one)
    });
    checked
        .verdict()
        .unwrap_or_else(|e| panic!("the workload's digest differs: {e}"));
}

/// Principle 5: bit-identical results on every thread count chapter 21 lists (1, 2, 3 and 8).
#[test]
#[cfg_attr(
    all(target_family = "wasm", not(target_feature = "atomics")),
    ignore = "WebAssembly without the atomics feature (wasm32-wasip1) has no threads: \
              std::thread::scope cannot spawn"
)]
fn results_identical_across_thread_counts() {
    let one = workload(0x0D5E_ED00);
    for threads in [2usize, 3, 8] {
        let digests: Vec<String> = std::thread::scope(|sc| {
            let handles: Vec<_> = (0..threads)
                .map(|_| sc.spawn(|| workload(0x0D5E_ED00)))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(
            digests.iter().all(|d| *d == one),
            "{threads} threads: {digests:?} vs {one}"
        );
    }
}
