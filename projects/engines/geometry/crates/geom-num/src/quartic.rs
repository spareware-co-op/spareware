//! The real roots of a quartic with whole-number coefficients, exact: how many, how multiple, and
//! in what order.
//!
//! An ellipse meets a circle or another conic where a quartic has its roots (chapter 14.2). What
//! drafting needs of those roots is exact: their number, which are tangencies, which of two is
//! first, and whether two come from one point. So they are never computed as numbers. A root is
//! isolated, in a dyadic interval that holds it and no other, or found to be a dyadic number itself,
//! and from then on every question is the sign of a polynomial at a dyadic point, or a test for a
//! common root.
//!
//! What it does: [`Quartic::roots`] lists the real roots of a quartic in increasing order on the
//! real projective line, so that t = ∞ is a root when the leading coefficient vanishes. The line is
//! cut at −1, 0, 1 and ∞, which are tested exactly. Each of the four open pieces maps onto (0, 1),
//! where Descartes' rule bounds the roots by the sign variations of the Möbius image, and bisection
//! brings that bound to 0 or 1. A count that stays above 1 past [`FAST_DEPTH`] means a multiple root
//! or two very near: the quartic is then split by subresultant gcds into square-free factors, one
//! for each multiplicity, and each isolated apart, so that a root's multiplicity is the factor it
//! came from. [`compare`] orders two roots of two quartics.
//!
//! Guarantees:
//!
//! - Every result is exact, or an error. Nothing here is a floating-point number; a root is two
//!   whole numbers and a power of two, and a sign is a sign of a whole number.
//! - Every root is found, once, with its multiplicity. Descartes' rule over-counts only, so a
//!   stretch whose count is 0 holds no root, and one whose count is 1 holds exactly one, simple.
//! - Nothing overflows silently. Every step is a checked operation in [`Big`] or the wider integers
//!   of the exact paths; a step that did not fit, or a bisection past its limit, is
//!   [`NumError::BudgetExceeded`], never an answer. For coefficients below
//!   2^[`COEFFICIENT_BITS`] no input reaches either, by the bounds below and the module's asserts:
//!
//!   | Quantity | Bound | Where |
//!   | --- | --- | --- |
//!   | A factor's coefficients | 2^303 | Mignotte's bound, 2^4 · ‖c‖₂ |
//!   | Two roots of one factor, apart | 2^−921 | Davenport (1985) and Mahler (1964) |
//!   | Roots of two quartics, apart if not one | 2^−2,442 | their resultant, a whole number |
//!   | A polynomial bisected s times | 2^(4s + 307) | the terms of 2^(4s) · c((x + a) / 2^s) |
//!
//! - Bounded work. Live stretches each count at least two, and Descartes' counts of disjoint
//!   stretches sum to at most four, so at most two are live at once and the frontier is a fixed
//!   array of three nodes.
//!
//! Method: Descartes' rule of signs by bisection, Collins and Akritas 1976
//! (<https://doi.org/10.1145/800205.806346>), the method chapter 14.2 names
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`); subresultant remainder sequences
//! for the gcds (Brown and Traub 1971, <https://doi.org/10.1145/321662.321664>); square-free
//! factors by repeated gcds (Musser 1971, as Knuth sets it out, volume 2, section 4.6.3); Stein's
//! binary gcd for a polynomial's content; the root separation bound of Davenport (1985) and Mahler
//! (1964).
//!
//! Cost: a few hundred products of 64 limbs for simple, well-separated roots. A multiple root adds a
//! few hundred more of 160 limbs, about ten exact divisions, each a bit at a time, and the binary
//! gcds of the content. [`compare`] of roots a long way apart is a few subtractions; of near ones,
//! three signs at each level it must narrow them. No allocation; no recursion.

use crate::error::NumError;
use crate::interval::Interval;
use crate::one_root::enclose;
use crate::sign::Sign;
use crate::wide::Wide;

/// Limbs of the integers of a quartic and its isolation: 4,096 bits.
pub const LIMBS: usize = 64;

/// The integers of a quartic.
pub type Big = Wide<LIMBS>;

/// Limbs of the exact paths, the gcds and the comparison of two quartics' roots: 10,240 bits.
const WIDE_LIMBS: usize = 160;

/// The integers of the exact paths.
type Wider = Wide<WIDE_LIMBS>;

/// Coefficients within this many bits reach every root inside the budget.
pub const COEFFICIENT_BITS: u32 = 297;

/// Levels of bisection past which a stretch is given up: the separation bound's 921, and some.
pub const MAX_DEPTH: u32 = 930;

/// Levels of bisection before a stretch is taken to hold a multiple root and the gcd is worked out.
/// Far below any separation real drawings show, and only a cost: past it the same roots are found.
pub const FAST_DEPTH: u32 = 64;

/// Levels of narrowing past which two roots are given up: the resultant's 2,442, and some.
const COMPARE_DEPTH: u32 = 2_470;

/// The bits of [`Big`].
const BITS: u32 = 4_096;

/// The bits of [`Wider`].
const WIDE_BITS: u32 = 10_240;

const _: () = {
    // A factor's coefficients, and a bisected polynomial's, each with its Taylor shift.
    assert!(4 * MAX_DEPTH + COEFFICIENT_BITS + 6 + 8 + 8 < BITS);
    assert!(BITS as usize == 64 * LIMBS);
    // Davenport and Mahler: degree 4, with a factor's coefficients.
    assert!(3 * (COEFFICIENT_BITS + 8) + 6 < MAX_DEPTH);
    // Res(f, g) ≥ 1 by |lc f|⁴ |lc g|⁴ ∏ |αᵢ − βⱼ|, each difference but one below the sum of two
    // Mahler measures, each measure at most ‖c‖₂ < 2^(COEFFICIENT_BITS + 5).
    assert!(8 * (COEFFICIENT_BITS + 5) + 26 < COMPARE_DEPTH);
    // A polynomial's value at a dyadic point, at that depth.
    assert!(4 * COMPARE_DEPTH + COEFFICIENT_BITS + 6 + 8 < WIDE_BITS);
    assert!(WIDE_BITS as usize == 64 * WIDE_LIMBS);
};

/// One polynomial of degree at most four, lowest term first.
type Terms<const L: usize> = [Wide<L>; 5];

fn budget<T>(v: Option<T>) -> Result<T, NumError> {
    v.ok_or(NumError::BudgetExceeded)
}

fn mul<const L: usize>(a: &Wide<L>, b: &Wide<L>) -> Result<Wide<L>, NumError> {
    budget(a.checked_mul(b))
}

fn add<const L: usize>(a: &Wide<L>, b: &Wide<L>) -> Result<Wide<L>, NumError> {
    budget(a.checked_add(b))
}

fn sub<const L: usize>(a: &Wide<L>, b: &Wide<L>) -> Result<Wide<L>, NumError> {
    budget(a.checked_sub(b))
}

/// `v` times a small number.
fn times<const L: usize>(v: &Wide<L>, k: i64) -> Result<Wide<L>, NumError> {
    mul(v, &Wide::from_i64(k))
}

/// |v|.
fn magnitude<const L: usize>(v: &Wide<L>) -> Result<Wide<L>, NumError> {
    if v.is_negative() {
        budget(v.checked_neg())
    } else {
        Ok(*v)
    }
}

/// The number of bits of |v|.
fn bits<const L: usize>(v: &Wide<L>) -> Result<u32, NumError> {
    let mut count = 0_u32;
    for (limb, at) in magnitude(v)?.to_le_limbs().iter().zip(0_u32..) {
        if *limb != 0 {
            let top = 64_u32.saturating_sub(limb.leading_zeros());
            count = budget(at.checked_mul(64))?.saturating_add(top);
        }
    }
    Ok(count)
}

/// v · 2^n, by limbs, its two's complement unchanged but for the shift.
fn shifted<const L: usize>(v: &Wide<L>, n: u32) -> Result<Wide<L>, NumError> {
    if v.is_zero() {
        return Ok(*v);
    }
    let room = budget(u32::try_from(L).ok().and_then(|l| l.checked_mul(64)))?;
    if bits(v)?.saturating_add(n).saturating_add(1) >= room {
        return Err(NumError::BudgetExceeded);
    }
    let words = budget(usize::try_from(n / 64).ok())?;
    let spare = n % 64;
    let limbs = v.to_le_limbs();
    let mut out = [0_u64; L];
    for (k, slot) in out.iter_mut().enumerate().skip(words) {
        let at = budget(k.checked_sub(words))?;
        let low = limbs.get(at).copied().unwrap_or(0);
        let below = at
            .checked_sub(1)
            .and_then(|p| limbs.get(p).copied())
            .unwrap_or(0);
        *slot = if spare == 0 {
            low
        } else {
            let up = low.checked_shl(spare).unwrap_or(0);
            up | below.checked_shr(64_u32.saturating_sub(spare)).unwrap_or(0)
        };
    }
    Ok(Wide::from_le_limbs(out))
}

/// v / 2 of a whole number that is not negative, by limbs.
fn half<const L: usize>(v: &Wide<L>) -> Wide<L> {
    let limbs = v.to_le_limbs();
    let mut out = [0_u64; L];
    for (k, slot) in out.iter_mut().enumerate() {
        let low = limbs.get(k).copied().unwrap_or(0);
        let above = k
            .checked_add(1)
            .and_then(|n| limbs.get(n).copied())
            .unwrap_or(0);
        *slot = (low >> 1) | (above << 63);
    }
    Wide::from_le_limbs(out)
}

/// Whether `v` is even.
fn even<const L: usize>(v: &Wide<L>) -> bool {
    v.to_le_limbs().first().is_none_or(|l| l & 1 == 0)
}

/// The greatest common divisor of two whole numbers, by Stein's binary algorithm: a division-free
/// loop, at most two steps a bit, each a shift or a difference.
fn whole_gcd<const L: usize>(a: &Wide<L>, b: &Wide<L>) -> Result<Wide<L>, NumError> {
    let (mut x, mut y) = (magnitude(a)?, magnitude(b)?);
    if x.is_zero() {
        return Ok(y);
    }
    if y.is_zero() {
        return Ok(x);
    }
    let mut twos = 0_u32;
    while even(&x) && even(&y) {
        x = half(&x);
        y = half(&y);
        twos = budget(twos.checked_add(1))?;
    }
    while even(&x) {
        x = half(&x);
    }
    while !y.is_zero() {
        while even(&y) {
            y = half(&y);
        }
        if sub(&x, &y)?.signum() == Sign::Positive {
            core::mem::swap(&mut x, &mut y);
        }
        y = sub(&y, &x)?;
    }
    shifted(&x, twos)
}

/// The bit of `limbs` at `at`.
fn bit<const L: usize>(limbs: &[u64; L], at: u32) -> bool {
    usize::try_from(at / 64)
        .ok()
        .and_then(|k| limbs.get(k))
        .is_some_and(|l| l.checked_shr(at % 64).is_some_and(|v| v & 1 == 1))
}

/// `a / b`, which is exact where it is called, by restoring long division.
fn exact_quotient<const L: usize>(a: &Wide<L>, b: &Wide<L>) -> Result<Wide<L>, NumError> {
    if b.is_zero() {
        return Err(NumError::BudgetExceeded);
    }
    let negative = a.is_negative() != b.is_negative();
    let (n, d) = (magnitude(a)?, magnitude(b)?);
    let limbs = n.to_le_limbs();
    let one = Wide::from_i64(1);
    let mut remainder = Wide::from_i64(0);
    let mut quotient = Wide::from_i64(0);
    for at in (0..bits(&n)?).rev() {
        remainder = add(&remainder, &remainder)?;
        if bit(&limbs, at) {
            remainder = add(&remainder, &one)?;
        }
        quotient = add(&quotient, &quotient)?;
        let less = sub(&remainder, &d)?;
        if less.signum() != Sign::Negative {
            remainder = less;
            quotient = add(&quotient, &one)?;
        }
    }
    if !remainder.is_zero() {
        return Err(NumError::BudgetExceeded);
    }
    if negative {
        budget(quotient.checked_neg())
    } else {
        Ok(quotient)
    }
}

/// The highest term with a nonzero coefficient.
fn degree<const L: usize>(c: &Terms<L>) -> Option<usize> {
    c.iter().rposition(|v| !v.is_zero())
}

/// The coefficient of t^i, zero past the end.
fn coefficient<const L: usize>(c: &Terms<L>, i: usize) -> Wide<L> {
    c.get(i).copied().unwrap_or_else(|| Wide::from_i64(0))
}

/// `c` with x replaced by x + 1.
fn taylor_shift<const L: usize>(c: &Terms<L>) -> Result<Terms<L>, NumError> {
    let [c0, c1, c2, c3, c4] = c;
    Ok([
        add(&add(&add(&add(c0, c1)?, c2)?, c3)?, c4)?,
        add(
            &add(&add(c1, &times(c2, 2)?)?, &times(c3, 3)?)?,
            &times(c4, 4)?,
        )?,
        add(&add(c2, &times(c3, 3)?)?, &times(c4, 6)?)?,
        add(c3, &times(c4, 4)?)?,
        *c4,
    ])
}

/// `c` with x replaced by −x.
fn mirrored<const L: usize>(c: &Terms<L>) -> Result<Terms<L>, NumError> {
    let turn = |v: &Wide<L>| budget(v.checked_neg());
    let [c0, c1, c2, c3, c4] = c;
    Ok([*c0, turn(c1)?, *c2, turn(c3)?, *c4])
}

/// x⁴ · c(1 / x).
const fn reversed<const L: usize>(c: &Terms<L>) -> Terms<L> {
    let [c0, c1, c2, c3, c4] = c;
    [*c4, *c3, *c2, *c1, *c0]
}

/// 16 · c(x / 2): the left half of (0, 1), on (0, 1).
fn left_half<const L: usize>(c: &Terms<L>) -> Result<Terms<L>, NumError> {
    let [c0, c1, c2, c3, c4] = c;
    Ok([
        shifted(c0, 4)?,
        shifted(c1, 3)?,
        shifted(c2, 2)?,
        shifted(c3, 1)?,
        *c4,
    ])
}

/// The sign variations of a list, zeros skipped.
fn variations<const L: usize>(c: &Terms<L>) -> u32 {
    let mut last = Sign::Zero;
    let mut count = 0_u32;
    for v in c {
        let s = v.signum();
        if s != Sign::Zero {
            if last != Sign::Zero && s != last {
                count = count.saturating_add(1);
            }
            last = s;
        }
    }
    count
}

/// Descartes' bound on the roots of `c` in (0, 1): the variations of (x + 1)⁴ c(1 / (x + 1)).
fn descartes<const L: usize>(c: &Terms<L>) -> Result<u32, NumError> {
    Ok(variations(&taylor_shift(&reversed(c))?))
}

/// The sign of `c` at a / 2^s: Horner's rule with the denominators cleared.
fn sign_at<const L: usize>(c: &Terms<L>, a: &Wide<L>, s: u32) -> Result<Sign, NumError> {
    let mut value = Wide::from_i64(0);
    for (powers, v) in (0_u32..).zip(c.iter().rev()) {
        let term = shifted(v, budget(s.checked_mul(powers))?)?;
        value = add(&mul(&value, a)?, &term)?;
    }
    Ok(value.signum())
}

/// The derivative of `c`.
fn derivative<const L: usize>(c: &Terms<L>) -> Result<Terms<L>, NumError> {
    let [_, c1, c2, c3, c4] = c;
    Ok([
        *c1,
        times(c2, 2)?,
        times(c3, 3)?,
        times(c4, 4)?,
        Wide::from_i64(0),
    ])
}

/// The sign of the square-free `c` just above a / 2^s, which at a root is its derivative's.
fn sign_above<const L: usize>(c: &Terms<L>, a: &Wide<L>, s: u32) -> Result<Sign, NumError> {
    match sign_at(c, a, s)? {
        Sign::Zero => sign_at(&derivative(c)?, a, s),
        sign => Ok(sign),
    }
}

/// The sign of the square-free `c` just below a / 2^s.
fn sign_below<const L: usize>(c: &Terms<L>, a: &Wide<L>, s: u32) -> Result<Sign, NumError> {
    match sign_at(c, a, s)? {
        Sign::Zero => Ok(sign_at(&derivative(c)?, a, s)?.negate()),
        sign => Ok(sign),
    }
}

/// `a` shifted up by `by` terms.
fn raised<const L: usize>(a: &Terms<L>, by: usize) -> Result<Terms<L>, NumError> {
    let mut out = [Wide::from_i64(0); 5];
    for (k, slot) in out.iter_mut().enumerate().skip(by) {
        *slot = coefficient(a, budget(k.checked_sub(by))?);
    }
    Ok(out)
}

/// The pseudo-remainder of `a` by `b`: lc(b)^(δ + 1) · a modulo b.
fn pseudo_remainder<const L: usize>(a: &Terms<L>, b: &Terms<L>) -> Result<Terms<L>, NumError> {
    let (Some(da), Some(db)) = (degree(a), degree(b)) else {
        return Err(NumError::BudgetExceeded);
    };
    let lead = coefficient(b, db);
    let mut r = *a;
    let mut steps = budget(da.checked_add(1).and_then(|n| n.checked_sub(db)))?;
    while let Some(dr) = degree(&r).filter(|d| *d >= db) {
        let top = coefficient(&r, dr);
        let by = raised(b, budget(dr.checked_sub(db))?)?;
        for (slot, term) in r.iter_mut().zip(by.iter()) {
            *slot = sub(&mul(slot, &lead)?, &mul(term, &top)?)?;
        }
        steps = budget(steps.checked_sub(1))?;
    }
    for _ in 0..steps {
        for slot in &mut r {
            *slot = mul(slot, &lead)?;
        }
    }
    Ok(r)
}

/// `v` to the power `n`.
fn power<const L: usize>(v: &Wide<L>, n: usize) -> Result<Wide<L>, NumError> {
    let mut out = Wide::from_i64(1);
    for _ in 0..n {
        out = mul(&out, v)?;
    }
    Ok(out)
}

/// The polynomial 1.
fn one<const L: usize>() -> Terms<L> {
    let zero = Wide::from_i64(0);
    [Wide::from_i64(1), zero, zero, zero, zero]
}

/// A gcd of `a` and `b` by the subresultant remainder sequence, degree a ≥ degree b, b nonzero.
#[allow(
    clippy::many_single_char_names,
    reason = "a, b, g, h, p, q and r are the sequence's, as Brown and Traub write them"
)]
fn subresultant_gcd<const L: usize>(a: &Terms<L>, b: &Terms<L>) -> Result<Terms<L>, NumError> {
    let (mut p, mut q) = (*a, *b);
    let (mut g, mut h) = (Wide::from_i64(1), Wide::from_i64(1));
    loop {
        let (Some(dp), Some(dq)) = (degree(&p), degree(&q)) else {
            return Err(NumError::BudgetExceeded);
        };
        let delta = budget(dp.checked_sub(dq))?;
        let r = pseudo_remainder(&p, &q)?;
        match degree(&r) {
            None => return Ok(q),
            Some(0) => return Ok(one()),
            Some(_) => {}
        }
        let divisor = mul(&g, &power(&h, delta)?)?;
        p = q;
        q = r;
        for slot in &mut q {
            *slot = exact_quotient(slot, &divisor)?;
        }
        g = coefficient(&p, dq);
        if let Some(less) = delta.checked_sub(1) {
            h = exact_quotient(&power(&g, delta)?, &power(&h, less)?)?;
        }
    }
}

/// `c` over the greatest common divisor of its coefficients.
fn primitive<const L: usize>(c: &Terms<L>) -> Result<Terms<L>, NumError> {
    let mut content = Wide::from_i64(0);
    for v in c {
        content = whole_gcd(&content, v)?;
    }
    if content.is_zero() || content == Wide::from_i64(1) {
        return Ok(*c);
    }
    let mut out = *c;
    for slot in &mut out {
        *slot = exact_quotient(slot, &content)?;
    }
    Ok(out)
}

/// The primitive gcd of `a` and `b`, 1 if either is a nonzero constant.
fn gcd<const L: usize>(a: &Terms<L>, b: &Terms<L>) -> Result<Terms<L>, NumError> {
    match (degree(a), degree(b)) {
        (None, _) => primitive(b),
        (_, None) => primitive(a),
        (Some(0), _) | (_, Some(0)) => Ok(one()),
        (Some(da), Some(db)) if da >= db => primitive(&subresultant_gcd(a, b)?),
        _ => primitive(&subresultant_gcd(b, a)?),
    }
}

/// a / b, b dividing a exactly but for a constant: primitive, its sign whatever it comes to.
fn divided<const L: usize>(a: &Terms<L>, b: &Terms<L>) -> Result<Terms<L>, NumError> {
    let (Some(da), Some(db)) = (degree(a), degree(b)) else {
        return Err(NumError::BudgetExceeded);
    };
    let lead = coefficient(b, db);
    let mut r = *a;
    let mut quotient = [Wide::from_i64(0); 5];
    // lead^k · a = quotient · b + r after k steps.
    for top in (db..=da).rev() {
        let value = coefficient(&r, top);
        let at = budget(top.checked_sub(db))?;
        let by = raised(b, at)?;
        for (slot, term) in r.iter_mut().zip(by.iter()) {
            *slot = sub(&mul(slot, &lead)?, &mul(term, &value)?)?;
        }
        for slot in &mut quotient {
            *slot = mul(slot, &lead)?;
        }
        if let Some(slot) = quotient.get_mut(at) {
            *slot = value;
        }
    }
    if degree(&r).is_some() {
        return Err(NumError::BudgetExceeded);
    }
    primitive(&quotient)
}

/// `c` at the width of the exact paths.
fn widened(c: &Terms<LIMBS>) -> Result<Terms<WIDE_LIMBS>, NumError> {
    let [c0, c1, c2, c3, c4] = c;
    let up = |v: &Big| budget(v.resize());
    Ok([up(c0)?, up(c1)?, up(c2)?, up(c3)?, up(c4)?])
}

/// `c` back at a quartic's width: a factor of a quartic is no larger than the budget allows.
fn narrowed_terms(c: &Terms<WIDE_LIMBS>) -> Result<Terms<LIMBS>, NumError> {
    let [c0, c1, c2, c3, c4] = c;
    let down = |v: &Wider| budget(v.resize());
    Ok([down(c0)?, down(c1)?, down(c2)?, down(c3)?, down(c4)?])
}

/// `c`'s roots each once: c / gcd(c, c′), primitive.
fn square_free(c: &Terms<WIDE_LIMBS>) -> Result<Terms<WIDE_LIMBS>, NumError> {
    divided(c, &gcd(c, &derivative(c)?)?)
}

/// One factor of a quartic's square-free factorisation: its roots are simple in it, and have this
/// multiplicity in the quartic.
#[derive(Clone, Copy)]
struct Factor {
    terms: Terms<LIMBS>,
    multiplicity: u8,
}

/// Musser's factors: with g = gcd(c, c′) and w = c / g, at step i, y = gcd(w, g) leaves in w / y the
/// roots of multiplicity exactly i, and w and g go on as y and g / y.
fn factors(c: &Terms<LIMBS>) -> Result<[Option<Factor>; 4], NumError> {
    let mut out = [None; 4];
    let c = primitive(&widened(c)?)?;
    let mut g = gcd(&c, &derivative(&c)?)?;
    let mut w = divided(&c, &g)?;
    let mut multiplicity = 1_u8;
    let mut slots = out.iter_mut();
    while degree(&w).is_some_and(|d| d > 0) {
        let y = gcd(&w, &g)?;
        let exactly = divided(&w, &y)?;
        if degree(&exactly).is_some_and(|d| d > 0) {
            let Some(slot) = slots.next() else {
                return Err(NumError::BudgetExceeded);
            };
            *slot = Some(Factor {
                terms: narrowed_terms(&exactly)?,
                multiplicity,
            });
        }
        g = divided(&g, &y)?;
        w = y;
        multiplicity = budget(multiplicity.checked_add(1))?;
    }
    Ok(out)
}

/// Which stretch of the real projective line a root lies in, in increasing order of t.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Piece {
    /// Below −1: t = −1 / x.
    FarBelow,
    /// t = −1.
    MinusOne,
    /// Between −1 and 0: t = −x.
    Below,
    /// t = 0.
    Zero,
    /// Between 0 and 1: t = x.
    Above,
    /// t = 1.
    One,
    /// Above 1: t = 1 / x.
    FarAbove,
    /// t = ∞, which the leading coefficient's vanishing makes a root.
    Infinity,
}

const OPEN: [Piece; 4] = [Piece::FarBelow, Piece::Below, Piece::Above, Piece::FarAbove];

impl Piece {
    /// `c` as a polynomial in x on this piece's (0, 1).
    fn on_unit<const L: usize>(self, c: &Terms<L>) -> Result<Terms<L>, NumError> {
        match self {
            Piece::FarBelow => mirrored(&reversed(c)),
            Piece::Below => mirrored(c),
            Piece::FarAbove => Ok(reversed(c)),
            _ => Ok(*c),
        }
    }

    /// `c` on (0, 1) with its roots at x = 0 divided out: reading a polynomial of lower degree at
    /// degree four leaves them there, and they are no root of the piece.
    fn open_on_unit<const L: usize>(self, c: &Terms<L>) -> Result<Terms<L>, NumError> {
        let on_unit = self.on_unit(c)?;
        let lowest = on_unit.iter().position(|v| !v.is_zero()).unwrap_or(0);
        let mut out = [Wide::from_i64(0); 5];
        for (slot, v) in out.iter_mut().zip(on_unit.iter().skip(lowest)) {
            *slot = *v;
        }
        Ok(out)
    }

    /// Whether t falls as x rises.
    const fn falling(self) -> bool {
        matches!(self, Piece::Below | Piece::FarAbove)
    }
}

/// A real root: where it is, and how many times it is a root.
#[derive(Clone, Copy, Debug)]
pub struct RealRoot {
    piece: Piece,
    /// The root's x is a / 2^s if `point`, and lies in (a / 2^s, (a + 1) / 2^s) if not; unused on
    /// an exact piece.
    a: Big,
    s: u32,
    point: bool,
    multiplicity: u8,
}

impl RealRoot {
    /// The stretch of the line it lies in.
    #[must_use]
    pub const fn piece(&self) -> Piece {
        self.piece
    }

    /// How many times it is a root: 2 and 4 at a tangency, 1 and 3 at a crossing.
    #[must_use]
    pub const fn multiplicity(&self) -> u8 {
        self.multiplicity
    }

    /// Where it lies as an angle: cos θ and sin θ for t = tan(θ / 2), certainly. An enclosure as wide
    /// as the stretch isolation left it.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step did not fit, which no root within the module's depth
    /// can reach.
    pub fn angle(&self) -> Result<[Interval; 2], NumError> {
        let (zero, one) = (Interval::point(0.0)?, Interval::point(1.0)?);
        let exact = match self.piece {
            Piece::Zero => Some([one, zero]),
            Piece::One => Some([zero, one]),
            Piece::MinusOne => Some([zero, one.neg()]),
            Piece::Infinity => Some([one.neg(), zero]),
            Piece::FarBelow | Piece::Below | Piece::Above | Piece::FarAbove => None,
        };
        if let Some(at) = exact {
            return Ok(at);
        }
        // Two to the s, within the depth, from its exponent field.
        let exponent = u64::from(self.s.saturating_add(1_023));
        let scale = Interval::point(f64::from_bits(exponent.wrapping_shl(52)))?;
        let low = enclose(&self.a)?.div(scale)?;
        let x = if self.point {
            low
        } else {
            let next = self.a.checked_add(&Big::one()).ok_or(NumError::BudgetExceeded)?;
            Interval::new(low.lo(), enclose(&next)?.div(scale)?.hi())?
        };
        let square = x.mul(x)?;
        let below = one.add(square)?;
        let turned = one.sub(square)?.div(below)?;
        let sine = x.add(x)?.div(below)?;
        Ok(match self.piece {
            Piece::Above => [turned, sine],
            Piece::Below => [turned, sine.neg()],
            Piece::FarAbove => [turned.neg(), sine],
            _ => [turned.neg(), sine.neg()],
        })
    }
}

/// The real roots of a quartic, increasing, on the projective line.
#[derive(Clone, Copy, Debug)]
pub struct Roots {
    list: [Option<RealRoot>; 4],
}

impl Roots {
    /// The roots, in order.
    pub fn iter(&self) -> impl Iterator<Item = &RealRoot> {
        self.list.iter().flatten()
    }

    /// How many distinct roots there are.
    #[must_use]
    pub fn count(&self) -> usize {
        self.iter().count()
    }

    fn push(&mut self, root: &RealRoot) -> Result<(), NumError> {
        let Some(slot) = self.list.iter_mut().find(|s| s.is_none()) else {
            return Err(NumError::BudgetExceeded);
        };
        *slot = Some(*root);
        Ok(())
    }
}

/// One dyadic stretch of (0, 1), or a dyadic point of it, at the width of the exact paths.
#[derive(Clone, Copy)]
struct Stretch {
    a: Wider,
    s: u32,
    point: bool,
}

impl Stretch {
    fn of(root: &RealRoot) -> Result<Stretch, NumError> {
        Ok(Stretch {
            a: budget(root.a.resize())?,
            s: root.s,
            point: root.point,
        })
    }

    /// `root`, in this stretch.
    fn applied(&self, root: &RealRoot) -> Result<RealRoot, NumError> {
        Ok(RealRoot {
            a: budget(self.a.resize())?,
            s: self.s,
            point: self.point,
            ..*root
        })
    }

    /// Its two ends over 2^`top`, `top` at least its own s.
    fn ends(&self, top: u32) -> Result<(Wider, Wider), NumError> {
        let up = budget(top.checked_sub(self.s))?;
        let low = shifted(&self.a, up)?;
        if self.point {
            return Ok((low, low));
        }
        Ok((low, shifted(&add(&self.a, &Wide::from_i64(1))?, up)?))
    }

    /// The half of it that holds the simple root of `c` it holds, or the root itself.
    fn narrowed(&self, c: &Terms<WIDE_LIMBS>) -> Result<Stretch, NumError> {
        let s = budget(self.s.checked_add(1))?;
        if s > COMPARE_DEPTH {
            return Err(NumError::BudgetExceeded);
        }
        let low = add(&self.a, &self.a)?;
        let middle = add(&low, &Wide::from_i64(1))?;
        let at_middle = sign_at(c, &middle, s)?;
        if at_middle == Sign::Zero {
            return Ok(Stretch {
                a: middle,
                s,
                point: true,
            });
        }
        let upper = sign_above(c, &low, s)? == at_middle;
        Ok(Stretch {
            a: if upper { middle } else { low },
            s,
            point: false,
        })
    }
}

/// The sign of p less q, if their stretches tell.
fn apart(p: &Stretch, q: &Stretch) -> Result<Option<Sign>, NumError> {
    let top = p.s.max(q.s);
    let ((p_low, p_high), (q_low, q_high)) = (p.ends(top)?, q.ends(top)?);
    if p.point && q.point {
        return Ok(Some(sub(&p_low, &q_low)?.signum()));
    }
    if sub(&p_high, &q_low)?.signum() != Sign::Positive {
        return Ok(Some(Sign::Negative));
    }
    if sub(&q_high, &p_low)?.signum() != Sign::Positive {
        return Ok(Some(Sign::Positive));
    }
    Ok(None)
}

/// The order in x of the root of `pc` in `p` and the root of `qc` in `q`, both polynomials
/// square-free. Apart stretches settle it. Else a common root in the overlap is both roots, for
/// each stretch holds one root of its polynomial; and without one, narrowing parts them.
fn order(
    mut p: Stretch,
    pc: &Terms<WIDE_LIMBS>,
    mut q: Stretch,
    qc: &Terms<WIDE_LIMBS>,
) -> Result<Sign, NumError> {
    if let Some(sign) = apart(&p, &q)? {
        return Ok(sign);
    }
    let common = gcd(pc, qc)?;
    let shared = degree(&common).is_some_and(|d| d > 0);
    loop {
        if let Some(sign) = apart(&p, &q)? {
            return Ok(sign);
        }
        if p.point {
            if sign_at(qc, &p.a, p.s)? == Sign::Zero {
                return Ok(Sign::Zero);
            }
            q = q.narrowed(qc)?;
        } else if q.point {
            if sign_at(pc, &q.a, q.s)? == Sign::Zero {
                return Ok(Sign::Zero);
            }
            p = p.narrowed(pc)?;
        } else {
            let top = p.s.max(q.s);
            let ((p_low, p_high), (q_low, q_high)) = (p.ends(top)?, q.ends(top)?);
            let low = if sub(&p_low, &q_low)?.signum() == Sign::Positive {
                p_low
            } else {
                q_low
            };
            let high = if sub(&p_high, &q_high)?.signum() == Sign::Negative {
                p_high
            } else {
                q_high
            };
            if shared && sign_above(&common, &low, top)? != sign_below(&common, &high, top)? {
                return Ok(Sign::Zero);
            }
            p = p.narrowed(pc)?;
            q = q.narrowed(qc)?;
        }
    }
}

/// A polynomial with whole-number coefficients, of degree at most four.
#[derive(Clone, Copy, Debug)]
pub struct Quartic {
    c: Terms<LIMBS>,
}

/// One stretch of (0, 1) still to be decided.
#[derive(Clone, Copy)]
struct Node {
    terms: Terms<LIMBS>,
    a: Big,
    s: u32,
}

/// A root found on an open piece, and which factor it is a root of.
#[derive(Clone, Copy)]
struct Found {
    root: RealRoot,
    factor: usize,
}

/// The roots found so far, in no order.
struct Isolated {
    list: [Option<Found>; 4],
}

impl Isolated {
    fn push(&mut self, found: &Found) -> Result<(), NumError> {
        let Some(slot) = self.list.iter_mut().find(|s| s.is_none()) else {
            return Err(NumError::BudgetExceeded);
        };
        *slot = Some(*found);
        Ok(())
    }
}

/// One factor being isolated on one piece.
struct Job {
    piece: Piece,
    factor: usize,
    multiplicity: u8,
    limit: u32,
}

impl Job {
    /// `a` and `s` as a root of this job, `times` a root of its polynomial.
    fn root(&self, a: &Big, s: u32, point: bool, times: u8) -> Result<Found, NumError> {
        Ok(Found {
            root: RealRoot {
                piece: self.piece,
                a: *a,
                s,
                point,
                multiplicity: budget(self.multiplicity.checked_mul(times))?,
            },
            factor: self.factor,
        })
    }

    /// `node` decided: nothing, a root, or a place on the list.
    fn place(
        &self,
        node: &Node,
        found: &mut Isolated,
        live: &mut [Option<Node>; 3],
    ) -> Result<(), NumError> {
        match descartes(&node.terms)? {
            0 => Ok(()),
            1 => found.push(&self.root(&node.a, node.s, false, 1)?),
            _ => {
                let Some(slot) = live.iter_mut().find(|n| n.is_none()) else {
                    return Err(NumError::BudgetExceeded);
                };
                *slot = Some(*node);
                Ok(())
            }
        }
    }

    /// Every root of `c` on the piece's open stretch. `false` if a stretch stayed undecided at the
    /// limit, which a multiple root does.
    fn isolate(&self, c: &Terms<LIMBS>, found: &mut Isolated) -> Result<bool, NumError> {
        let mut live: [Option<Node>; 3] = [None; 3];
        let start = Node {
            terms: self.piece.on_unit(c)?,
            a: Big::from_i64(0),
            s: 0,
        };
        self.place(&start, found, &mut live)?;
        while let Some(node) = live.iter_mut().find_map(Option::take) {
            if node.s >= self.limit {
                return Ok(false);
            }
            let left = left_half(&node.terms)?;
            let right = taylor_shift(&left)?;
            let doubled = add(&node.a, &node.a)?;
            let middle = add(&doubled, &Big::from_i64(1))?;
            let s = budget(node.s.checked_add(1))?;
            // The lowest terms of the polynomial from the middle say how multiple a root it is.
            let times = right.iter().position(|v| !v.is_zero()).unwrap_or(0);
            if times > 0 {
                found.push(&self.root(&middle, s, true, budget(u8::try_from(times).ok())?)?)?;
            }
            let halves = [(left, doubled), (right, middle)];
            for (terms, a) in halves {
                self.place(&Node { terms, a, s }, found, &mut live)?;
            }
        }
        Ok(true)
    }
}

/// Puts one piece's roots in increasing order of t.
fn sort_open(
    here: &mut [Option<Found>; 4],
    list: &[Option<Factor>; 4],
    piece: Piece,
) -> Result<(), NumError> {
    let simple = |found: &Found| -> Result<Terms<WIDE_LIMBS>, NumError> {
        let Some(Some(factor)) = list.get(found.factor) else {
            return Err(NumError::BudgetExceeded);
        };
        piece.open_on_unit(&widened(&factor.terms)?)
    };
    // Bubble, on at most four.
    for _ in 0..4 {
        for k in 0..3_usize {
            let next = budget(k.checked_add(1))?;
            let (Some(Some(p)), Some(Some(q))) = (here.get(k).copied(), here.get(next).copied())
            else {
                continue;
            };
            let x = order(
                Stretch::of(&p.root)?,
                &simple(&p)?,
                Stretch::of(&q.root)?,
                &simple(&q)?,
            )?;
            let t = if piece.falling() { x.negate() } else { x };
            if t == Sign::Positive {
                here.swap(k, next);
            }
        }
    }
    // Isolated factor by factor, a stretch may hold a neighbour's root: part each from the next, so
    // that a root's stretch holds that root of the whole quartic and no other. Neighbours apart
    // leave every pair apart, as the roots are in order.
    for k in 0..3_usize {
        let next = budget(k.checked_add(1))?;
        let (Some(Some(p)), Some(Some(q))) = (here.get(k).copied(), here.get(next).copied()) else {
            continue;
        };
        let (pc, qc) = (simple(&p)?, simple(&q)?);
        let (mut a, mut b) = (Stretch::of(&p.root)?, Stretch::of(&q.root)?);
        while apart(&a, &b)?.is_none() {
            if !a.point {
                a = a.narrowed(&pc)?;
            }
            if !b.point {
                b = b.narrowed(&qc)?;
            }
        }
        if let Some(slot) = here.get_mut(k) {
            *slot = Some(Found {
                root: a.applied(&p.root)?,
                ..p
            });
        }
        if let Some(slot) = here.get_mut(next) {
            *slot = Some(Found {
                root: b.applied(&q.root)?,
                ..q
            });
        }
    }
    Ok(())
}

impl Quartic {
    /// The polynomial with these coefficients, lowest term first.
    #[must_use]
    pub const fn new(c: [Big; 5]) -> Quartic {
        Quartic { c }
    }

    /// Its coefficients, lowest term first.
    #[must_use]
    pub const fn coefficients(&self) -> [Big; 5] {
        self.c
    }

    /// Whether every coefficient is zero, so that every t is a root.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        degree(&self.c).is_none()
    }

    /// The multiplicity of t = ∞, as a root of the polynomial read at degree four.
    fn at_infinity(&self) -> u8 {
        degree(&self.c)
            .and_then(|top| 4_usize.checked_sub(top))
            .and_then(|m| u8::try_from(m).ok())
            .unwrap_or(0)
    }

    /// The multiplicity of the exact root `piece`, 0 if it is none.
    fn at_exact(&self, piece: Piece) -> Result<u8, NumError> {
        let shifted_to = match piece {
            Piece::Zero => self.c,
            Piece::One => taylor_shift(&self.c)?,
            Piece::MinusOne => mirrored(&taylor_shift(&mirrored(&self.c)?)?)?,
            _ => return Ok(self.at_infinity()),
        };
        let lowest = shifted_to.iter().position(|v| !v.is_zero()).unwrap_or(0);
        Ok(u8::try_from(lowest).unwrap_or(0))
    }

    /// The roots of the factors in `list` on the open pieces, each to `limit`. `None` if a stretch
    /// stayed undecided.
    fn isolated(list: &[Option<Factor>; 4], limit: u32) -> Result<Option<Isolated>, NumError> {
        let mut found = Isolated { list: [None; 4] };
        for (at, factor) in list.iter().enumerate() {
            let Some(Factor {
                terms,
                multiplicity,
            }) = factor
            else {
                continue;
            };
            for piece in OPEN {
                let job = Job {
                    piece,
                    factor: at,
                    multiplicity: *multiplicity,
                    limit,
                };
                if !job.isolate(terms, &mut found)? {
                    return Ok(None);
                }
            }
        }
        Ok(Some(found))
    }

    /// The real roots, increasing on the projective line, with their multiplicities.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] for the zero polynomial, of which every t is a root;
    /// [`NumError::BudgetExceeded`] if a step did not fit, which coefficients within
    /// [`COEFFICIENT_BITS`] cannot reach.
    pub fn roots(&self) -> Result<Roots, NumError> {
        if self.is_zero() {
            return Err(NumError::Degenerate);
        }
        let itself = [
            Some(Factor {
                terms: self.c,
                multiplicity: 1,
            }),
            None,
            None,
            None,
        ];
        if let Some(found) = Self::isolated(&itself, FAST_DEPTH)? {
            return self.assemble(&itself, &found);
        }
        let list = factors(&self.c)?;
        let found = budget(Self::isolated(&list, MAX_DEPTH)?)?;
        self.assemble(&list, &found)
    }

    /// The open stretches' roots with the exact ones, in order.
    fn assemble(&self, list: &[Option<Factor>; 4], open: &Isolated) -> Result<Roots, NumError> {
        let mut out = Roots { list: [None; 4] };
        for piece in [
            Piece::FarBelow,
            Piece::MinusOne,
            Piece::Below,
            Piece::Zero,
            Piece::Above,
            Piece::One,
            Piece::FarAbove,
            Piece::Infinity,
        ] {
            if OPEN.contains(&piece) {
                let mut here: [Option<Found>; 4] = [None; 4];
                let on_this = open.list.iter().flatten().filter(|f| f.root.piece == piece);
                for (slot, found) in here.iter_mut().zip(on_this) {
                    *slot = Some(*found);
                }
                sort_open(&mut here, list, piece)?;
                for found in here.iter().flatten() {
                    out.push(&found.root)?;
                }
            } else {
                let multiplicity = self.at_exact(piece)?;
                if multiplicity > 0 {
                    out.push(&RealRoot {
                        piece,
                        a: Big::from_i64(0),
                        s: 0,
                        point: true,
                        multiplicity,
                    })?;
                }
            }
        }
        Ok(out)
    }
}

/// How two real roots of two quartics compare: the sign of the first less the second, zero when
/// they are one number. Disjoint stretches settle it; overlapping ones are narrowed unless the
/// quartics' gcd has a root in both, which makes the two roots one.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] if a step did not fit, which coefficients within
/// [`COEFFICIENT_BITS`] cannot reach.
pub fn compare(
    first: &Quartic,
    p: &RealRoot,
    second: &Quartic,
    q: &RealRoot,
) -> Result<Sign, NumError> {
    if p.piece != q.piece {
        return Ok(if p.piece < q.piece {
            Sign::Negative
        } else {
            Sign::Positive
        });
    }
    let piece = p.piece;
    if !OPEN.contains(&piece) {
        return Ok(Sign::Zero);
    }
    let simple =
        |quartic: &Quartic| piece.open_on_unit(&square_free(&primitive(&widened(&quartic.c)?)?)?);
    let x = order(
        Stretch::of(p)?,
        &simple(first)?,
        Stretch::of(q)?,
        &simple(second)?,
    )?;
    Ok(if piece.falling() { x.negate() } else { x })
}
