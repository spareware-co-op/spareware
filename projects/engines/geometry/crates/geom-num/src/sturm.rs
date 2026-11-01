//! How many real roots a polynomial of degree at most four has, exactly, by Sturm's method.
//!
//! What it does: [`Chain::of`] builds the Sturm chain of an integer polynomial as a subresultant
//! chain, whose coefficients stay near the size of determinants of the polynomial's own. From it
//! [`Chain::roots`] counts the distinct real roots and says whether any is repeated, and
//! [`Chain::roots_between`] counts them between two rational points, ends excluded. [`filtered`]
//! and [`filtered_between`] give the same answers in `f64` intervals where those decide, so that
//! the wide path only runs near a repeated root (chapter 12.6).
//!
//! Guarantees, with B the bits of the largest coefficient:
//!
//! - **A Sturm chain.** Each element after the first two is a positive multiple of minus the
//!   remainder of the two before it, so Sturm's theorem counts the real roots: the multiplier of
//!   each pseudo-remainder is |lc|^(δ+1), and each exact divisor is a magnitude. Every multiple
//!   being positive is what lets the classical algorithm's quotients be read as Sturm's.
//! - **The budget, B ≤ 256.** By Hadamard's bound, on subresultants as determinants of 3, 5 and 7
//!   rows of coefficients below 2^(B+2), the chain's coefficients stay below 2^(3B+9), 2^(5B+17) and
//!   2^(7B+27). The largest value computed is a pseudo-remainder of the last step, at most
//!   8·lc²·|u|·... ≤ 2^(13B+50) = 2^3378 at B = 256, inside [`Big`]'s limit of 2^3455: [`LIMBS`]
//!   holds it, and every step is checked, so a wrong budget is `None`, never a wrong count. A line
//!   with a torus has B = 256 ([`crate::quadric`]'s derivation).
//! - **Exact division, proven at each use.** [`div_exact`] multiplies its quotient back by the
//!   divisor and compares, so a value that was not divisible is refused, never rounded.
//! - **The filter never guesses.** [`filtered`] returns `None` unless every leading coefficient of
//!   its chain has a certain sign, each proven by outward rounding, and then the chain has full
//!   length, so no root is repeated.
//! - **Ends excluded.** [`Chain::roots_between`] takes each element's sign just inside each end,
//!   so a root at an end is not counted, however often it repeats.
//!
//! Method: Sturm's theorem (Basu, Pollack and Roy, *Algorithms in Real Algebraic Geometry*, 2006,
//! chapter 2) on the subresultant polynomial remainder sequence (Brown and Traub 1971,
//! doi:10.1145/321662.321664; Cohen, *A Course in Computational Algebraic Number Theory*, algorithm
//! 3.3.1), with Hadamard's bound for the budget. Exact division digit by digit from the low end
//! (Jebelean 1993, doi:10.1006/jsco.1993.1017). The filter is the same chain in
//! [`crate::interval`]: a chain branches on its degrees, so it has no straight-line form for
//! `geom-gen`, and outward rounding proves each sign with no constant to derive.
//!
//! Cost: a chain is at most 24 products and 15 exact divisions of [`LIMBS`]-limb integers, on the
//! stack, each O(`LIMBS`²). The filter is at most 60 interval operations. Recursion: none.

use crate::error::NumError;
use crate::interval::Interval;
use crate::sign::Sign;
use crate::wide::Wide;

/// The limbs of a chain's integers: 3456 bits, for the budget of the module header.
pub const LIMBS: usize = 54;

/// The largest size [`filtered_between`] takes: every whole number up to it is an exact double.
const LIMIT: u64 = 1 << 53;

/// The integers of a chain. Beyond chapter 6.2's widths, so it waits for an owner's acceptance
/// (31.7); `geom-math`'s exact stages hold the same kind of width.
pub type Big = Wide<LIMBS>;

/// The most bits of a coefficient the budget allows.
pub const MAX_COEFFICIENT_BITS: u32 = 256;

/// One more than the highest degree: a chain's polynomials have up to this many coefficients.
pub const TERMS: usize = 5;

/// A rational point of a line's parameter: `num / den`, with `den` positive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    /// The numerator.
    pub num: i64,
    /// The denominator.
    pub den: i64,
}

impl Point {
    /// A whole number.
    #[must_use]
    pub const fn whole(num: i64) -> Point {
        Point { num, den: 1 }
    }
}

/// How many real roots a polynomial has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Roots {
    /// How many, each counted once.
    pub distinct: u8,
    /// Whether some root, real or complex, is repeated.
    pub repeated: bool,
}

/// A polynomial: its coefficients, lowest first, and its degree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Poly<const L: usize> {
    terms: [Wide<L>; TERMS],
    degree: usize,
}

/// The degree of `terms`, or `None` for zero.
fn degree_of<const L: usize>(terms: &[Wide<L>; TERMS]) -> Option<usize> {
    terms.iter().rposition(|c| !c.is_zero())
}

impl<const L: usize> Poly<L> {
    /// The coefficient of the highest term.
    fn lead(&self) -> Option<Wide<L>> {
        self.terms.get(self.degree).copied()
    }

    /// Its derivative, or `None` if a step did not fit.
    fn derivative(&self) -> Option<[Wide<L>; TERMS]> {
        let mut out = [Wide::<L>::ZERO; TERMS];
        for (i, slot) in out.iter_mut().enumerate() {
            let next = i.checked_add(1)?;
            let scale = Wide::<L>::from_i128(i128::try_from(next).ok()?);
            *slot = match self.terms.get(next) {
                Some(c) => c.checked_mul(&scale)?,
                None => Wide::<L>::ZERO,
            };
        }
        Some(out)
    }

    /// The sign of its value at `at`: the sign of Σ cᵢ·numⁱ·den^(degree − i), since `den` is
    /// positive. Horner's rule keeps each partial sum an integer.
    fn sign_at(&self, num: &Wide<L>, den: &Wide<L>) -> Option<Sign> {
        let mut power = Wide::<L>::one();
        let mut sum = Wide::<L>::ZERO;
        for c in self.terms.iter().take(self.degree.checked_add(1)?).rev() {
            sum = sum.checked_mul(num)?.checked_add(&c.checked_mul(&power)?)?;
            power = power.checked_mul(den)?;
        }
        Some(sum.signum())
    }

    /// Its sign just to one side of `at`: the sign of the first derivative that is not zero there,
    /// with odd derivatives flipped on the left, from Taylor's expansion.
    fn sign_beside(&self, num: &Wide<L>, den: &Wide<L>, left: bool) -> Option<Sign> {
        let mut p = *self;
        let mut odd = false;
        loop {
            let s = p.sign_at(num, den)?;
            if s != Sign::Zero {
                return Some(if odd && left {
                    s.mul(Sign::Negative)
                } else {
                    s
                });
            }
            let terms = p.derivative()?;
            p = Poly {
                degree: degree_of(&terms)?,
                terms,
            };
            odd = !odd;
        }
    }
}

/// The base-2^64 inverse of an odd `x`, by Newton's iteration, which doubles the correct bits: `x`
/// is its own inverse to 3 bits, so five steps reach 96.
fn odd_inverse(x: u64) -> u64 {
    let once = |y: u64| y.wrapping_mul(2u64.wrapping_sub(x.wrapping_mul(y)));
    once(once(once(once(once(x)))))
}

/// `limbs` shifted right by `bits`.
fn shifted<const L: usize>(limbs: &[u64; L], bits: usize) -> [u64; L] {
    let (limb, part) = (bits / 64, bits % 64);
    let mut out = [0u64; L];
    for (i, slot) in out.iter_mut().enumerate() {
        let at = |k: usize| {
            i.checked_add(k)
                .and_then(|j| limbs.get(j))
                .copied()
                .unwrap_or(0)
        };
        let low = at(limb);
        let high = at(limb.saturating_add(1));
        // `part` is below 64, so the shifts are in range.
        *slot = if part == 0 {
            low
        } else {
            (low >> part) | (high << 64usize.saturating_sub(part))
        };
    }
    out
}

/// `rest` minus `digit · limbs · 2^(64 · from)`, modulo 2^(64 · L).
fn subtract_multiple<const L: usize>(
    rest: &mut [u64; L],
    digit: u64,
    limbs: &[u64; L],
    from: usize,
) {
    let (mut carry, mut borrow) = (0u64, false);
    for (slot, &y) in rest.iter_mut().skip(from).zip(limbs) {
        // At most (2^64 − 1)² + 2^64 − 1, below 2^128.
        let wide = u128::from(digit)
            .wrapping_mul(u128::from(y))
            .wrapping_add(u128::from(carry));
        let low = u64::try_from(wide & u128::from(u64::MAX)).unwrap_or(0);
        let high = u64::try_from(wide >> 64).unwrap_or(0);
        let (first, b1) = slot.overflowing_sub(low);
        let (second, b2) = first.overflowing_sub(u64::from(borrow));
        *slot = second;
        borrow = b1 || b2;
        carry = high;
    }
}

/// `n / d` for a `d` that divides `n`, or `None` if it does not, or if `d` is zero.
///
/// Digit by digit from the low end: past `d`'s trailing zeros `d` is odd, so each digit of the
/// quotient is the low limb of what is left times `d`'s inverse modulo 2^64.
#[must_use]
pub fn div_exact<const L: usize>(n: &Wide<L>, d: &Wide<L>) -> Option<Wide<L>> {
    let magnitude = |v: &Wide<L>| {
        if v.is_negative() {
            v.checked_neg().map(Wide::to_le_limbs)
        } else {
            Some(v.to_le_limbs())
        }
    };
    let (top, bottom) = (magnitude(n)?, magnitude(d)?);
    let zeros: usize = bottom
        .iter()
        .take_while(|l| **l == 0)
        .count()
        .checked_mul(64)?;
    let first = bottom.iter().find(|l| **l != 0)?;
    let zeros = zeros.checked_add(first.trailing_zeros().try_into().ok()?)?;
    let mut rest = shifted(&top, zeros);
    let odd = shifted(&bottom, zeros);
    let inverse = odd_inverse(*odd.first()?);
    let mut quotient = [0u64; L];
    for (i, digit) in quotient.iter_mut().enumerate() {
        *digit = rest.get(i)?.wrapping_mul(inverse);
        subtract_multiple(&mut rest, *digit, &odd, i);
    }
    let q = Wide::<L>::from_le_limbs(quotient);
    let answer = if n.is_negative() == d.is_negative() {
        q
    } else {
        q.checked_neg()?
    };
    (answer.checked_mul(d)? == *n).then_some(answer)
}

/// The sign just to one side of num / den, with a positive den, of the polynomial with `terms`,
/// lowest first; zero for the zero polynomial. `None` if a step did not fit.
#[must_use]
pub fn sign_beside<const L: usize>(
    terms: &[Wide<L>; TERMS],
    num: &Wide<L>,
    den: &Wide<L>,
    left: bool,
) -> Option<Sign> {
    let Some(degree) = degree_of(terms) else {
        return Some(Sign::Zero);
    };
    Poly {
        terms: *terms,
        degree,
    }
    .sign_beside(num, den, left)
}

/// The greatest common divisor of two integers, not negative, by Stein's binary method. `None` if a
/// step did not fit.
#[must_use]
pub fn gcd<const L: usize>(a: &Wide<L>, b: &Wide<L>) -> Option<Wide<L>> {
    let zeros = |v: &[u64; L]| -> Option<usize> {
        let whole = v.iter().take_while(|l| **l == 0).count();
        let part = v.get(whole).map_or(0, |l| l.trailing_zeros());
        whole.checked_mul(64)?.checked_add(part.try_into().ok()?)
    };
    let (mut x, mut y) = (magnitude(a)?.to_le_limbs(), magnitude(b)?.to_le_limbs());
    if x.iter().all(|l| *l == 0) {
        return Some(Wide::<L>::from_le_limbs(y));
    }
    if y.iter().all(|l| *l == 0) {
        return Some(Wide::<L>::from_le_limbs(x));
    }
    let (from_x, from_y) = (zeros(&x)?, zeros(&y)?);
    let common = from_x.min(from_y);
    x = shifted(&x, from_x);
    y = shifted(&y, from_y);
    // Both odd now. Each step clears a bit of one of them, so 2·64·L steps end it.
    for _ in 0..L.checked_mul(128)? {
        let (wide_x, wide_y) = (Wide::<L>::from_le_limbs(x), Wide::<L>::from_le_limbs(y));
        let difference = wide_x.checked_sub(&wide_y)?;
        if difference.is_zero() {
            let mut out = wide_x;
            let two = Wide::<L>::from_i64(2);
            for _ in 0..common {
                out = out.checked_mul(&two)?;
            }
            return Some(out);
        }
        let (rest, keep_y) = if difference.is_negative() {
            (difference.checked_neg()?, true)
        } else {
            (difference, false)
        };
        let limbs = rest.to_le_limbs();
        let odd = shifted(&limbs, zeros(&limbs)?);
        if keep_y {
            y = odd;
        } else {
            x = odd;
        }
    }
    None
}

/// The whole square root of `v`, if it has one, by Newton's iteration from above. `None` if a step did
/// not fit, or if `v` is negative or not a square.
#[must_use]
pub fn square_root<const L: usize>(v: &Wide<L>) -> Option<Wide<L>> {
    if v.is_negative() {
        return None;
    }
    if v.is_zero() {
        return Some(*v);
    }
    let limbs = v.to_le_limbs();
    let used = limbs.iter().rposition(|l| *l != 0)?;
    let bits = used.checked_mul(64)?.checked_add(
        usize::try_from(64_u32.checked_sub(limbs.get(used)?.leading_zeros())?).ok()?,
    )?;
    let two = Wide::<L>::from_i64(2);
    let mut guess = Wide::<L>::one();
    for _ in 0..bits.div_ceil(2) {
        guess = guess.checked_mul(&two)?;
    }
    // From a value at or above the root, the floor of Newton's step falls to it in at most `bits`
    // steps, and then rises: the first that does not fall is the floor of the root.
    for _ in 0..bits {
        let (quotient, _) = divided(v, &guess)?;
        let sum = guess.checked_add(&quotient)?;
        let (next, _) = divided(&sum, &two)?;
        if !next.checked_sub(&guess)?.is_negative() {
            break;
        }
        guess = next;
    }
    (guess.checked_mul(&guess)? == *v).then_some(guess)
}

/// `n / d` and its remainder, for an `n` not negative and a positive `d`, by long division in bits.
fn divided<const L: usize>(n: &Wide<L>, d: &Wide<L>) -> Option<(Wide<L>, Wide<L>)> {
    let limbs = n.to_le_limbs();
    let (mut quotient, mut rest) = ([0_u64; L], Wide::<L>::ZERO);
    let two = Wide::<L>::from_i64(2);
    for bit in (0..L.checked_mul(64)?).rev() {
        let set = (limbs.get(bit / 64)? >> (bit % 64)) & 1;
        rest = rest
            .checked_mul(&two)?
            .checked_add(&Wide::<L>::from_i64(i64::try_from(set).ok()?))?;
        let less = rest.checked_sub(d)?;
        if !less.is_negative() {
            rest = less;
            *quotient.get_mut(bit / 64)? |= 1 << (bit % 64);
        }
    }
    Some((Wide::<L>::from_le_limbs(quotient), rest))
}

/// The magnitude of `v`.
fn magnitude<const L: usize>(v: &Wide<L>) -> Option<Wide<L>> {
    if v.is_negative() {
        v.checked_neg()
    } else {
        Some(*v)
    }
}

/// `v` to the power `n`.
fn power<const L: usize>(v: &Wide<L>, n: usize) -> Option<Wide<L>> {
    let mut out = Wide::<L>::one();
    for _ in 0..n {
        out = out.checked_mul(v)?;
    }
    Some(out)
}

/// The pseudo-remainder of `u` by `v`: |lc(v)|^(δ+1) · u, less a multiple of `v`, of degree below
/// `v`'s. The multiplier is positive, as a Sturm chain needs.
fn remainder<const L: usize>(dividend: &Poly<L>, divisor: &Poly<L>) -> Option<[Wide<L>; TERMS]> {
    let lead = divisor.lead()?;
    let size = magnitude(&lead)?;
    let flip = lead.is_negative();
    let mut r = dividend.terms;
    let mut top = dividend.degree;
    while top >= divisor.degree {
        let shift = top.checked_sub(divisor.degree)?;
        let head = *r.get(top)?;
        for (i, slot) in r.iter_mut().enumerate() {
            let scaled = slot.checked_mul(&size)?;
            let below = i.checked_sub(shift).and_then(|k| divisor.terms.get(k));
            *slot = match below {
                Some(term) if i <= top => {
                    let product = head.checked_mul(term)?;
                    if flip {
                        scaled.checked_add(&product)?
                    } else {
                        scaled.checked_sub(&product)?
                    }
                }
                _ => scaled,
            };
        }
        // The divisor is of degree at least one: a chain stops at a constant.
        top = top.checked_sub(1)?;
    }
    Some(r)
}

/// A Sturm chain: up to five polynomials of falling degree, the last the greatest common divisor
/// of the polynomial and its derivative.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chain<const L: usize = LIMBS> {
    polys: [Poly<L>; TERMS],
    len: usize,
}

/// How many times the sign changes along `signs`, zeros skipped.
fn changes(signs: impl Iterator<Item = Sign>) -> u8 {
    let mut count = 0u8;
    let mut last = Sign::Zero;
    for s in signs.filter(|s| *s != Sign::Zero) {
        if last != Sign::Zero && s != last {
            count = count.saturating_add(1);
        }
        last = s;
    }
    count
}

impl<const L: usize> Chain<L> {
    /// The chain of the polynomial with `coefficients`, lowest first.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] for a constant polynomial, which has no chain.
    /// [`NumError::BudgetExceeded`] if a step did not fit, which coefficients below
    /// 2^[`MAX_COEFFICIENT_BITS`] cannot cause (the module header).
    pub fn of(coefficients: &[Wide<L>; TERMS]) -> Result<Chain<L>, NumError> {
        let degree = degree_of(coefficients).ok_or(NumError::Degenerate)?;
        if degree == 0 {
            return Err(NumError::Degenerate);
        }
        Self::build(coefficients, degree).ok_or(NumError::BudgetExceeded)
    }

    /// [`Chain::of`] past its checks.
    fn build(terms: &[Wide<L>; TERMS], degree: usize) -> Option<Chain<L>> {
        let first = Poly {
            terms: *terms,
            degree,
        };
        let slope = first.derivative()?;
        let second = Poly {
            terms: slope,
            degree: degree_of(&slope)?,
        };
        let mut polys = [first; TERMS];
        *polys.get_mut(1)? = second;
        let mut len: usize = 2;
        // Cohen's lead and step, as magnitudes.
        let (mut lead, mut step) = (Wide::<L>::one(), Wide::<L>::one());
        while let (Some(above), Some(below)) = (
            len.checked_sub(2).and_then(|i| polys.get(i)).copied(),
            len.checked_sub(1).and_then(|i| polys.get(i)).copied(),
        ) {
            if below.degree == 0 {
                break;
            }
            let gap = above.degree.checked_sub(below.degree)?;
            let raw = remainder(&above, &below)?;
            let divisor = lead.checked_mul(&power(&step, gap)?)?;
            let mut next = [Wide::<L>::ZERO; TERMS];
            for (slot, term) in next.iter_mut().zip(&raw) {
                *slot = div_exact(term, &divisor)?.checked_neg()?;
            }
            let Some(degree) = degree_of(&next) else {
                break;
            };
            *polys.get_mut(len)? = Poly {
                terms: next,
                degree,
            };
            len = len.checked_add(1)?;
            lead = magnitude(&below.lead()?)?;
            step = match gap.checked_sub(1)? {
                0 => lead,
                less => div_exact(&power(&lead, gap)?, &power(&step, less)?)?,
            };
        }
        Some(Chain { polys, len })
    }

    /// The polynomials, first to last.
    fn elements(&self) -> impl Iterator<Item = &Poly<L>> {
        self.polys.iter().take(self.len)
    }

    /// The real roots, each counted once, and whether any root is repeated.
    #[must_use]
    pub fn roots(&self) -> Roots {
        let at_infinity = |negative: bool| {
            changes(self.elements().map(|p| {
                let s = p.lead().map_or(Sign::Zero, |c| c.signum());
                if negative && p.degree % 2 == 1 {
                    s.mul(Sign::Negative)
                } else {
                    s
                }
            }))
        };
        Roots {
            distinct: at_infinity(true).saturating_sub(at_infinity(false)),
            repeated: self.elements().last().is_some_and(|p| p.degree > 0),
        }
    }

    /// The real roots strictly between `lo` and `hi`, each counted once.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] unless `lo` is below `hi` and both denominators are positive.
    /// [`NumError::BudgetExceeded`] if a step did not fit, which |num| and den below 2^63 and the
    /// module header's coefficients cannot cause: each value is at most 5·2^(5B+17)·2^(63·4).
    pub fn roots_between(&self, lo: Point, hi: Point) -> Result<u8, NumError> {
        let ordered = i128::from(lo.num)
            .checked_mul(i128::from(hi.den))
            .zip(i128::from(hi.num).checked_mul(i128::from(lo.den)))
            .is_some_and(|(a, b)| a < b);
        if lo.den <= 0 || hi.den <= 0 || !ordered {
            return Err(NumError::Degenerate);
        }
        let ends = |at: Point| (Wide::<L>::from_i64(at.num), Wide::<L>::from_i64(at.den));
        let ((a, b), (c, d)) = (ends(lo), ends(hi));
        self.roots_in(&a, &b, &c, &d)
            .ok_or(NumError::BudgetExceeded)
    }

    /// The sign changes of the chain just to one side of (num / den).
    fn side(&self, num: &Wide<L>, den: &Wide<L>, left: bool) -> Option<u8> {
        let mut signs = [Sign::Zero; TERMS];
        for (slot, p) in signs.iter_mut().zip(self.elements()) {
            *slot = p.sign_beside(num, den, left)?;
        }
        Some(changes(signs.iter().copied()))
    }

    /// The real roots strictly between two points written num / den with a positive den, each counted
    /// once: [`Chain::roots_between`] for numbers past an `i64`. `None` if a step did not fit.
    #[must_use]
    pub fn roots_in(
        &self,
        lo_num: &Wide<L>,
        lo_den: &Wide<L>,
        hi_num: &Wide<L>,
        hi_den: &Wide<L>,
    ) -> Option<u8> {
        let start = self.side(lo_num, lo_den, false)?;
        let end = self.side(hi_num, hi_den, true)?;
        Some(start.saturating_sub(end))
    }

    /// Whether the polynomial is zero at num / den. `None` if a step did not fit.
    #[must_use]
    pub fn vanishes_at(&self, num: &Wide<L>, den: &Wide<L>) -> Option<bool> {
        Some(self.polys.first()?.sign_at(num, den)? == Sign::Zero)
    }

    /// The chain's last polynomial, the greatest common divisor of the first and its derivative up to
    /// a rational multiple: its coefficients, lowest first, and its degree.
    #[must_use]
    pub fn last(&self) -> Option<(&[Wide<L>; TERMS], usize)> {
        self.elements().last().map(|p| (&p.terms, p.degree))
    }
}

/// `f`'s chain in intervals: each element's coefficients, lowest first, and its degree; `None`
/// unless every leading coefficient's sign is certain and the degree falls by one each step.
fn interval_chain(
    f: &[Interval; TERMS],
    degree: usize,
) -> Option<[([Interval; TERMS], usize); TERMS]> {
    let zero = Interval::point(0.0).ok()?;
    let mut slope = [zero; TERMS];
    for (i, slot) in slope.iter_mut().enumerate() {
        let next = i.checked_add(1)?;
        if let Some(factor) = f.get(next) {
            let n = u32::try_from(next).ok()?;
            *slot = factor.mul(Interval::point(f64::from(n)).ok()?).ok()?;
        }
    }
    let mut chain = [(*f, degree); TERMS];
    *chain.get_mut(1)? = (slope, degree.checked_sub(1)?);
    let mut k = 2;
    while k <= degree {
        let (mut rest, upper) = *chain.get(k.checked_sub(2)?)?;
        let (pivot, lower) = *chain.get(k.checked_sub(1)?)?;
        let lead = *pivot.get(lower)?;
        lead.certain_sign().filter(|s| *s != Sign::Zero)?;
        for top in (lower..=upper).rev() {
            let factor = rest.get(top)?.div(lead).ok()?;
            let shift = top.checked_sub(lower)?;
            for (i, term) in pivot.iter().enumerate().take(lower.checked_add(1)?) {
                let slot = rest.get_mut(i.checked_add(shift)?)?;
                *slot = slot.sub(factor.mul(*term).ok()?).ok()?;
            }
            *rest.get_mut(top)? = zero;
        }
        let mut next = [zero; TERMS];
        for (slot, factor) in next.iter_mut().zip(&rest) {
            *slot = factor.neg();
        }
        *chain.get_mut(k)? = (next, lower.checked_sub(1)?);
        k = k.checked_add(1)?;
    }
    let (last, d) = chain.get(degree)?;
    last.get(*d)?.certain_sign().filter(|s| *s != Sign::Zero)?;
    Some(chain)
}

/// `x`, of at most 2^53 in size, as an interval of one point.
#[allow(
    clippy::cast_precision_loss,
    reason = "the callers hold |x| to 2^53, so the conversion is exact"
)]
fn whole(x: i64) -> Option<Interval> {
    Interval::point(x as f64).ok()
}

/// [`Chain::roots`] in intervals around the coefficients, or `None` if they do not decide it. An
/// answer has no repeated root: a full chain ends in a constant that is not zero.
#[must_use]
pub fn filtered(f: &[Interval; TERMS], degree: usize) -> Option<Roots> {
    let lead = *f.get(degree)?;
    lead.certain_sign().filter(|s| *s != Sign::Zero)?;
    let chain = interval_chain(f, degree)?;
    let at_infinity = |negative: bool| -> Option<u8> {
        let mut signs = [Sign::Zero; TERMS];
        for (slot, (p, d)) in signs
            .iter_mut()
            .zip(chain.iter().take(degree.checked_add(1)?))
        {
            let s = p.get(*d)?.certain_sign()?;
            *slot = if negative && d % 2 == 1 {
                s.mul(Sign::Negative)
            } else {
                s
            };
        }
        Some(changes(signs.iter().copied()))
    };
    Some(Roots {
        distinct: at_infinity(true)?.checked_sub(at_infinity(false)?)?,
        repeated: false,
    })
}

/// [`Chain::roots_between`] in intervals, or `None` if they do not decide it or a point's ends are
/// not exact doubles (|num| and den at most 2^53).
#[must_use]
pub fn filtered_between(f: &[Interval; TERMS], degree: usize, lo: Point, hi: Point) -> Option<u8> {
    let chain = interval_chain(f, degree)?;
    let value = |at: Point| -> Option<Interval> {
        let exact = at.num.unsigned_abs() <= LIMIT && at.den.unsigned_abs() <= LIMIT && at.den > 0;
        if !exact {
            return None;
        }
        whole(at.num)?.div(whole(at.den)?).ok()
    };
    let side = |at: Interval| -> Option<u8> {
        let mut signs = [Sign::Zero; TERMS];
        for (slot, (p, d)) in signs
            .iter_mut()
            .zip(chain.iter().take(degree.checked_add(1)?))
        {
            let mut sum = *p.get(*d)?;
            for c in p.iter().take(*d).rev() {
                sum = sum.mul(at).ok()?.add(*c).ok()?;
            }
            *slot = sum.certain_sign().filter(|s| *s != Sign::Zero)?;
        }
        Some(changes(signs.iter().copied()))
    };
    side(value(lo)?)?.checked_sub(side(value(hi)?)?)
}
