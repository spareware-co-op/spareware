//! The real roots of a quartic with whole-number coefficients, exact: how many, how multiple, and
//! in what order.
//!
//! An ellipse meets a circle or another conic where a quartic has its roots (chapter 14.2). What
//! drafting needs of those roots is exact: their number, which are tangencies, which of two is
//! first, and whether two come from one point. So they are never computed as numbers. A root is
//! isolated, in a dyadic interval that holds it and no other, and from then on every question is
//! the sign of a polynomial at a dyadic point, or a test for a common root.
//!
//! What it does: [`Quartic::roots`] lists the real roots of a quartic in increasing order on the
//! real projective line, so that t = ∞ is a root when the leading coefficient vanishes. The line is
//! cut at −1, 0, 1 and ∞, which are tested exactly. Each of the four open pieces maps onto (0, 1),
//! where Descartes' rule bounds the roots by the sign variations of the Möbius image, and bisection
//! brings that bound to 0 or 1. A piece that stays above 1 past [`FAST_DEPTH`] has a multiple root:
//! the polynomial is replaced by its square-free part, from a subresultant gcd, and isolated again.
//! [`compare`] orders two roots of two quartics.
//!
//! Guarantees:
//!
//! - Every result is exact, or an error. Nothing here is a floating-point number; a root is two
//!   whole numbers and a power of two, and a sign is a sign of a whole number.
//! - Every root is found, once, with its multiplicity. Descartes' rule over-counts only, so a piece
//!   whose count is 0 holds no root, and one whose count is 1 holds exactly one, simple.
//! - Nothing overflows silently. Every step is a checked operation in [`Big`]; a step that did not
//!   fit, or a bisection past [`MAX_DEPTH`], is [`NumError::BudgetExceeded`], never an answer. For
//!   coefficients below 2^[`COEFFICIENT_BITS`] no input reaches either: two simple roots lie at least
//!   2^−882 apart by the Davenport–Mahler bound, each level of bisection adds 4 bits, and so the
//!   largest value is below 2^(4 · 900 + 293 + 8) < 2^(64 · 64), which the module's constants assert.
//! - Bounded work. At most four pieces are live at any level, by the subadditivity of Descartes'
//!   rule, so the frontier is a fixed array of five nodes.
//!
//! Method: Descartes' rule of signs by bisection, Collins and Akritas 1976
//! (<https://doi.org/10.1145/800205.806346>), the method chapter 14.2 names
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`); subresultant remainder sequences
//! for the gcd (Brown and Traub 1971, <https://doi.org/10.1145/321662.321664>); the root separation
//! bound of Davenport (1985) and Mahler (1964).
//!
//! Cost: a few hundred products of 64 limbs for simple, well-separated roots. A gcd adds a few
//! hundred more and about ten exact divisions, each a bit at a time. No allocation; no recursion.

use crate::error::NumError;
use crate::sign::Sign;
use crate::wide::Wide;

/// Limbs of the integers here: 4,096 bits.
pub const LIMBS: usize = 64;

/// The integers of this module.
pub type Big = Wide<LIMBS>;

/// Coefficients within this many bits reach every root inside the budget.
pub const COEFFICIENT_BITS: u32 = 293;

/// Levels of bisection past which a piece is given up: the separation bound's 882, and some.
pub const MAX_DEPTH: u32 = 900;

/// Levels of bisection before a piece is taken to hold a multiple root and the gcd is worked out.
/// Far below any separation real drawings show, and only a cost: past it the same roots are found.
pub const FAST_DEPTH: u32 = 64;

const _: () = {
    assert!(4 * MAX_DEPTH + COEFFICIENT_BITS + 8 < 64 * LIMBS as u32);
    assert!(3 * (COEFFICIENT_BITS + 2) + 6 < MAX_DEPTH);
};

/// One polynomial of degree at most four, lowest term first.
type Terms = [Big; 5];

fn budget<T>(v: Option<T>) -> Result<T, NumError> {
    v.ok_or(NumError::BudgetExceeded)
}

fn mul(a: &Big, b: &Big) -> Result<Big, NumError> {
    budget(a.checked_mul(b))
}

fn add(a: &Big, b: &Big) -> Result<Big, NumError> {
    budget(a.checked_add(b))
}

fn sub(a: &Big, b: &Big) -> Result<Big, NumError> {
    budget(a.checked_sub(b))
}

/// `v` times a small number.
fn times(v: &Big, k: i64) -> Result<Big, NumError> {
    mul(v, &Big::from_i64(k))
}

/// The number of bits of |v|.
fn bits(v: &Big) -> Result<u32, NumError> {
    let magnitude = if v.is_negative() {
        budget(v.checked_neg())?
    } else {
        *v
    };
    let mut count = 0_u32;
    for (limb, at) in magnitude.to_le_limbs().iter().zip(0_u32..) {
        if *limb != 0 {
            count = 64 * at + (64 - limb.leading_zeros());
        }
    }
    Ok(count)
}

/// v · 2^n, by limbs, its two's complement unchanged but for the shift.
fn shifted(v: &Big, n: u32) -> Result<Big, NumError> {
    if v.is_zero() {
        return Ok(*v);
    }
    if bits(v)? + n >= 64 * LIMBS as u32 - 1 {
        return Err(NumError::BudgetExceeded);
    }
    let (words, spare) = ((n / 64) as usize, n % 64);
    let limbs = v.to_le_limbs();
    let mut out = [0_u64; LIMBS];
    for (k, slot) in out.iter_mut().enumerate().skip(words) {
        let at = k - words;
        let low = limbs.get(at).copied().unwrap_or(0);
        let below = at.checked_sub(1).and_then(|p| limbs.get(p).copied()).unwrap_or(0);
        *slot = if spare == 0 {
            low
        } else {
            (low << spare) | (below >> (64 - spare))
        };
    }
    Ok(Big::from_le_limbs(out))
}

/// The bit of |v| at `at`.
fn bit(limbs: &[u64; LIMBS], at: u32) -> bool {
    limbs
        .get((at / 64) as usize)
        .is_some_and(|l| (l >> (at % 64)) & 1 == 1)
}

/// `a / b`, which is exact by the subresultant theorem, by restoring long division.
fn exact_quotient(a: &Big, b: &Big) -> Result<Big, NumError> {
    if b.is_zero() {
        return Err(NumError::BudgetExceeded);
    }
    let negative = a.is_negative() != b.is_negative();
    let positive = |v: &Big| {
        if v.is_negative() {
            budget(v.checked_neg())
        } else {
            Ok(*v)
        }
    };
    let (n, d) = (positive(a)?, positive(b)?);
    let limbs = n.to_le_limbs();
    let mut remainder = Big::from_i64(0);
    let mut quotient = Big::from_i64(0);
    for at in (0..bits(&n)?).rev() {
        remainder = add(&remainder, &remainder)?;
        if bit(&limbs, at) {
            remainder = add(&remainder, &Big::from_i64(1))?;
        }
        quotient = add(&quotient, &quotient)?;
        if sub(&remainder, &d)?.signum() != Sign::Negative {
            remainder = sub(&remainder, &d)?;
            quotient = add(&quotient, &Big::from_i64(1))?;
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
fn degree(c: &Terms) -> Option<usize> {
    c.iter().rposition(|v| !v.is_zero())
}

/// The coefficient of t^i, zero past the end.
fn coefficient(c: &Terms, i: usize) -> Big {
    c.get(i).copied().unwrap_or_else(|| Big::from_i64(0))
}

/// `c` with x replaced by x + 1.
fn taylor_shift(c: &Terms) -> Result<Terms, NumError> {
    let [c0, c1, c2, c3, c4] = c;
    Ok([
        add(&add(&add(&add(c0, c1)?, c2)?, c3)?, c4)?,
        add(&add(&add(c1, &times(c2, 2)?)?, &times(c3, 3)?)?, &times(c4, 4)?)?,
        add(&add(c2, &times(c3, 3)?)?, &times(c4, 6)?)?,
        add(c3, &times(c4, 4)?)?,
        *c4,
    ])
}

/// `c` with x replaced by −x.
fn mirrored(c: &Terms) -> Result<Terms, NumError> {
    let turn = |v: &Big| budget(v.checked_neg());
    let [c0, c1, c2, c3, c4] = c;
    Ok([*c0, turn(c1)?, *c2, turn(c3)?, *c4])
}

/// x⁴ · c(1 / x).
fn reversed(c: &Terms) -> Terms {
    let [c0, c1, c2, c3, c4] = c;
    [*c4, *c3, *c2, *c1, *c0]
}

/// 16 · c(x / 2): the left half of (0, 1), on (0, 1).
fn left_half(c: &Terms) -> Result<Terms, NumError> {
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
fn variations(c: &Terms) -> u32 {
    let mut last = Sign::Zero;
    let mut count = 0;
    for v in c {
        let s = v.signum();
        if s != Sign::Zero {
            if last != Sign::Zero && s != last {
                count += 1;
            }
            last = s;
        }
    }
    count
}

/// Descartes' bound on the roots of `c` in (0, 1): the variations of (x + 1)⁴ c(1 / (x + 1)).
fn descartes(c: &Terms) -> Result<u32, NumError> {
    Ok(variations(&taylor_shift(&reversed(c))?))
}

/// The sign of `c` at a / 2^s: Horner's rule with the denominators cleared.
fn sign_at(c: &Terms, a: &Big, s: u32) -> Result<Sign, NumError> {
    let mut value = Big::from_i64(0);
    for (powers, v) in (0_u32..).zip(c.iter().rev()) {
        let term = shifted(v, budget(s.checked_mul(powers))?)?;
        value = add(&mul(&value, a)?, &term)?;
    }
    Ok(value.signum())
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
    fn on_unit(self, c: &Terms) -> Result<Terms, NumError> {
        match self {
            Piece::FarBelow => mirrored(&reversed(c)),
            Piece::Below => mirrored(c),
            Piece::FarAbove => Ok(reversed(c)),
            _ => Ok(*c),
        }
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
    /// The root's x lies in (a / 2^s, (a + 1) / 2^s); unused on an exact piece.
    a: Big,
    s: u32,
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

    fn push(&mut self, root: RealRoot) -> Result<(), NumError> {
        let Some(slot) = self.list.iter_mut().find(|s| s.is_none()) else {
            return Err(NumError::BudgetExceeded);
        };
        *slot = Some(root);
        Ok(())
    }
}

/// A polynomial with whole-number coefficients, of degree at most four.
#[derive(Clone, Copy, Debug)]
pub struct Quartic {
    c: Terms,
}

/// One stretch of (0, 1) still to be decided.
#[derive(Clone, Copy)]
struct Node {
    terms: Terms,
    a: Big,
    s: u32,
}

impl Quartic {
    /// The polynomial with these coefficients, lowest term first.
    #[must_use]
    pub const fn new(c: [Big; 5]) -> Quartic {
        Quartic { c }
    }

    /// Whether every coefficient is zero, so that every t is a root.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        degree(&self.c).is_none()
    }

    /// The multiplicity of t = ∞, as a root of the polynomial read at degree four.
    fn at_infinity(&self) -> u8 {
        let top = degree(&self.c).unwrap_or(0);
        u8::try_from(4 - top).unwrap_or(0)
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

    /// Every root of `c` on the piece's open stretch, simple ones only in `found`. `false` if a
    /// stretch stayed unsplit at `limit`, which a multiple root does.
    fn isolate(
        c: &Terms,
        piece: Piece,
        limit: u32,
        found: &mut Roots,
        roots: &mut u32,
    ) -> Result<bool, NumError> {
        let start = Node {
            terms: piece.on_unit(c)?,
            a: Big::from_i64(0),
            s: 0,
        };
        let mut live: [Option<Node>; 5] = [Some(start), None, None, None, None];
        while let Some(node) = live.iter_mut().find_map(Option::take) {
            match descartes(&node.terms)? {
                0 => {}
                1 => {
                    *roots += 1;
                    found.push(RealRoot {
                        piece,
                        a: node.a,
                        s: node.s,
                        multiplicity: 1,
                    })?;
                }
                _ if node.s >= limit => return Ok(false),
                _ => Self::split(&node, piece, found, roots, &mut live)?,
            }
        }
        Ok(true)
    }

    /// `node` cut at its middle: a root there, and what is left of each half on the list.
    fn split(
        node: &Node,
        piece: Piece,
        found: &mut Roots,
        roots: &mut u32,
        live: &mut [Option<Node>; 5],
    ) -> Result<(), NumError> {
        let left = left_half(&node.terms)?;
        let right = taylor_shift(&left)?;
        let doubled = add(&node.a, &node.a)?;
        let s = node.s + 1;
        if right.first().is_some_and(Wide::is_zero) {
            // The middle is a root: a dyadic rational, isolated as the stretch starting there.
            *roots += 1;
            found.push(RealRoot {
                piece,
                a: add(&doubled, &Big::from_i64(1))?,
                s,
                multiplicity: 1,
            })?;
        }
        let halves = [
            Node {
                terms: left,
                a: doubled,
                s,
            },
            Node {
                terms: right,
                a: add(&doubled, &Big::from_i64(1))?,
                s,
            },
        ];
        for half in halves {
            let Some(slot) = live.iter_mut().find(|n| n.is_none()) else {
                return Err(NumError::BudgetExceeded);
            };
            *slot = Some(half);
        }
        Ok(())
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
        let mut quick = Roots { list: [None; 4] };
        let mut count = 0;
        let mut whole = true;
        for piece in OPEN {
            whole &= Self::isolate(&self.c, piece, FAST_DEPTH, &mut quick, &mut count)?;
        }
        if !whole {
            let square_free = primitive_square_free(&self.c)?;
            let mut found = Roots { list: [None; 4] };
            let mut seen = 0;
            for piece in OPEN {
                if !Self::isolate(&square_free, piece, MAX_DEPTH, &mut found, &mut seen)? {
                    return Err(NumError::BudgetExceeded);
                }
            }
            quick = found;
        }
        self.assemble(&quick)
    }

    /// The open stretches' roots with the exact ones, in order, each with its multiplicity.
    fn assemble(&self, open: &Roots) -> Result<Roots, NumError> {
        let mut out = Roots { list: [None; 4] };
        let exact = |piece: Piece| -> Result<Option<RealRoot>, NumError> {
            let multiplicity = self.at_exact(piece)?;
            Ok((multiplicity > 0).then(|| RealRoot {
                piece,
                a: Big::from_i64(0),
                s: 0,
                multiplicity,
            }))
        };
        let order = [
            Piece::FarBelow,
            Piece::MinusOne,
            Piece::Below,
            Piece::Zero,
            Piece::Above,
            Piece::One,
            Piece::FarAbove,
            Piece::Infinity,
        ];
        for piece in order {
            if OPEN.contains(&piece) {
                let mut here: [Option<RealRoot>; 4] = [None; 4];
                for (slot, root) in here
                    .iter_mut()
                    .zip(open.iter().filter(|r| r.piece == piece))
                {
                    *slot = Some(*root);
                }
                sort_open(&mut here, &self.c, piece)?;
                for root in here.into_iter().flatten() {
                    let multiplicity = self.multiplicity_of(&root)?;
                    out.push(RealRoot {
                        multiplicity,
                        ..root
                    })?;
                }
            } else if let Some(root) = exact(piece)? {
                out.push(root)?;
            }
        }
        Ok(out)
    }

    /// How many times `root` is a root: one more than it is a root of each derivative in turn,
    /// tested by the sign change of the square-free gcd across its stretch.
    fn multiplicity_of(&self, root: &RealRoot) -> Result<u8, NumError> {
        let mut current = self.c;
        let mut multiplicity = 1;
        while let Some(next) = next_derivative_gcd(&current)? {
            let on_unit = root.piece.on_unit(&next)?;
            if !changes_sign(&on_unit, root)? {
                break;
            }
            multiplicity += 1;
            current = next;
        }
        Ok(multiplicity)
    }
}

/// Puts roots of one stretch in increasing order of t.
fn sort_open(here: &mut [Option<RealRoot>; 4], _c: &Terms, piece: Piece) -> Result<(), NumError> {
    // Bubble, on at most four: disjoint stretches compare by their left ends over a common power.
    for _ in 0..4 {
        for k in 0..3 {
            let (Some(Some(p)), Some(Some(q))) = (here.get(k).copied(), here.get(k + 1).copied())
            else {
                continue;
            };
            let top = p.s.max(q.s);
            let (pa, qa) = (shifted(&p.a, top - p.s)?, shifted(&q.a, top - q.s)?);
            let x_order = sub(&pa, &qa)?.signum();
            let ascending = if piece.falling() {
                x_order == Sign::Positive
            } else {
                x_order == Sign::Negative
            };
            if !ascending {
                here.swap(k, k + 1);
            }
        }
    }
    Ok(())
}

/// Whether `c`, on (0, 1), changes sign across the root's stretch, or has a zero at either end.
fn changes_sign(c: &Terms, root: &RealRoot) -> Result<bool, NumError> {
    let start = sign_at(c, &root.a, root.s)?;
    let end = sign_at(c, &add(&root.a, &Big::from_i64(1))?, root.s)?;
    Ok(start == Sign::Zero || end == Sign::Zero || start != end)
}

/// `a` shifted up by `by` terms.
fn raised(a: &Terms, by: usize) -> Terms {
    let mut out = [Big::from_i64(0); 5];
    for (k, slot) in out.iter_mut().enumerate().skip(by) {
        *slot = coefficient(a, k - by);
    }
    out
}

/// The pseudo-remainder of `a` by `b`: lc(b)^(δ + 1) · a modulo b.
fn pseudo_remainder(a: &Terms, b: &Terms) -> Result<Terms, NumError> {
    let (Some(da), Some(db)) = (degree(a), degree(b)) else {
        return Err(NumError::BudgetExceeded);
    };
    let lead = coefficient(b, db);
    let mut r = *a;
    let mut steps = da + 1 - db;
    while let Some(dr) = degree(&r).filter(|d| *d >= db) {
        let top = coefficient(&r, dr);
        let by = raised(b, dr - db);
        for (slot, term) in r.iter_mut().zip(by.iter()) {
            *slot = sub(&mul(slot, &lead)?, &mul(term, &top)?)?;
        }
        steps -= 1;
    }
    for _ in 0..steps {
        for slot in &mut r {
            *slot = mul(slot, &lead)?;
        }
    }
    Ok(r)
}

/// `power` of `v`.
fn power(v: &Big, power: usize) -> Result<Big, NumError> {
    let mut out = Big::from_i64(1);
    for _ in 0..power {
        out = mul(&out, v)?;
    }
    Ok(out)
}

/// A gcd of `a` and `b` by the subresultant remainder sequence, degree a ≥ degree b, b nonzero.
fn subresultant_gcd(a: &Terms, b: &Terms) -> Result<Terms, NumError> {
    let (mut p, mut q) = (*a, *b);
    let (mut g, mut h) = (Big::from_i64(1), Big::from_i64(1));
    loop {
        let (Some(dp), Some(dq)) = (degree(&p), degree(&q)) else {
            return Err(NumError::BudgetExceeded);
        };
        let delta = dp - dq;
        let r = pseudo_remainder(&p, &q)?;
        if degree(&r).is_none() {
            return Ok(q);
        }
        if degree(&r) == Some(0) {
            let mut one = [Big::from_i64(0); 5];
            if let Some(first) = one.first_mut() {
                *first = Big::from_i64(1);
            }
            return Ok(one);
        }
        let divisor = mul(&g, &power(&h, delta)?)?;
        p = q;
        q = r;
        for slot in &mut q {
            *slot = exact_quotient(slot, &divisor)?;
        }
        g = coefficient(&p, dq);
        h = if delta == 0 {
            h
        } else {
            let top = power(&g, delta)?;
            exact_quotient(&top, &power(&h, delta - 1)?)?
        };
    }
}

/// The derivative of `c`.
fn derivative(c: &Terms) -> Result<Terms, NumError> {
    let [_, c1, c2, c3, c4] = c;
    Ok([
        *c1,
        times(c2, 2)?,
        times(c3, 3)?,
        times(c4, 4)?,
        Big::from_i64(0),
    ])
}

/// The largest whole number dividing every coefficient by a power of two, left out: the content
/// that matters here is only size, and halving while every coefficient is even is cheap and exact.
fn halved(c: &Terms) -> Result<Terms, NumError> {
    let mut out = *c;
    while out
        .iter()
        .all(|v| v.is_zero() || v.to_le_limbs().first().is_some_and(|l| l & 1 == 0))
        && degree(&out).is_some()
    {
        for slot in &mut out {
            *slot = exact_quotient(slot, &Big::from_i64(2))?;
        }
    }
    Ok(out)
}

/// gcd(c, c′), or `None` when c has no repeated root.
fn next_derivative_gcd(c: &Terms) -> Result<Option<Terms>, NumError> {
    let top = degree(c).unwrap_or(0);
    if top == 0 {
        return Ok(None);
    }
    let gcd = subresultant_gcd(c, &derivative(c)?)?;
    Ok(if degree(&gcd).unwrap_or(0) == 0 {
        None
    } else {
        Some(gcd)
    })
}

/// c with its repeated roots made simple: c / gcd(c, c′), by pseudo-division, kept to whole
/// numbers and halved of its powers of two.
fn primitive_square_free(c: &Terms) -> Result<Terms, NumError> {
    let Some(gcd) = next_derivative_gcd(c)? else {
        return Ok(*c);
    };
    let (Some(dc), Some(dg)) = (degree(c), degree(&gcd)) else {
        return Err(NumError::BudgetExceeded);
    };
    let lead = coefficient(&gcd, dg);
    let mut r = *c;
    let mut quotient = [Big::from_i64(0); 5];
    for top in (dg..=dc).rev() {
        // r · lead, q · lead, and the next quotient term, until r is gone.
        let value = coefficient(&r, top);
        let by = raised(&gcd, top - dg);
        for (slot, term) in r.iter_mut().zip(by.iter()) {
            *slot = sub(&mul(slot, &lead)?, &mul(term, &value)?)?;
        }
        for slot in &mut quotient {
            *slot = mul(slot, &lead)?;
        }
        if let Some(slot) = quotient.get_mut(top - dg) {
            *slot = value;
        }
    }
    if degree(&r).is_some() {
        return Err(NumError::BudgetExceeded);
    }
    halved(&quotient)
}

/// Whether the stretch of `p` and that of `q` overlap, both read in increasing x on one piece.
fn overlap(p: &RealRoot, q: &RealRoot) -> Result<bool, NumError> {
    let top = p.s.max(q.s);
    let (pa, qa) = (shifted(&p.a, top - p.s)?, shifted(&q.a, top - q.s)?);
    let one = Big::from_i64(1);
    let (pb, qb) = (
        shifted(&add(&p.a, &one)?, top - p.s)?,
        shifted(&add(&q.a, &one)?, top - q.s)?,
    );
    Ok(sub(&pa, &qb)?.signum() == Sign::Negative && sub(&qa, &pb)?.signum() == Sign::Negative)
}

/// `root` halved: the half of its stretch that holds it, by the sign of `c`'s simple form.
fn narrowed(c: &Terms, root: &RealRoot) -> Result<RealRoot, NumError> {
    let a = add(&root.a, &root.a)?;
    let s = root.s + 1;
    if s > MAX_DEPTH {
        return Err(NumError::BudgetExceeded);
    }
    let middle = add(&a, &Big::from_i64(1))?;
    let (low, mid) = (sign_at(c, &a, s)?, sign_at(c, &middle, s)?);
    // A root at the middle is taken as the right half's left end, as `split` records it.
    let a = if mid == Sign::Zero || low == mid {
        middle
    } else {
        a
    };
    Ok(RealRoot { a, s, ..*root })
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
    if OPEN.iter().all(|o| *o != p.piece) {
        return Ok(Sign::Zero);
    }
    let piece = p.piece;
    let simple = |c: &Terms| -> Result<Terms, NumError> {
        piece.on_unit(&primitive_square_free(c)?)
    };
    let (pc, qc) = (simple(&first.c)?, simple(&second.c)?);
    let common = {
        let (dp, dq) = (degree(&pc).unwrap_or(0), degree(&qc).unwrap_or(0));
        if dp >= dq {
            subresultant_gcd(&pc, &qc)?
        } else {
            subresultant_gcd(&qc, &pc)?
        }
    };
    let (mut p, mut q) = (*p, *q);
    while overlap(&p, &q)? {
        let top = p.s.max(q.s);
        let (lo, hi) = (shifted(&p.a, top - p.s)?, shifted(&q.a, top - q.s)?);
        let start = if sub(&lo, &hi)?.signum() == Sign::Positive {
            RealRoot { a: lo, s: top, ..p }
        } else {
            RealRoot { a: hi, s: top, ..q }
        };
        if degree(&common).unwrap_or(0) > 0
            && changes_sign(&common, &start)?
            && p.s.max(q.s) == top
            && sign_at(&common, &start.a, top)? != Sign::Zero
        {
            return Ok(Sign::Zero);
        }
        p = narrowed(&pc, &p)?;
        q = narrowed(&qc, &q)?;
    }
    let top = p.s.max(q.s);
    let (pa, qa) = (shifted(&p.a, top - p.s)?, shifted(&q.a, top - q.s)?);
    let order = sub(&pa, &qa)?.signum();
    Ok(if piece.falling() { order.negate() } else { order })
}
