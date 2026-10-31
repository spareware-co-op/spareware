//! One-root numbers in 2D, exact: where a line meets a circle, and every question about such points.
//!
//! A line and a circle meet where a quadratic with whole-number coefficients has its roots, so the
//! two coordinates of a crossing are (a + b√k) / w with one square root shared by both (chapter
//! 14.2). [`Crossing2`] holds that form and the [`Root`] it stands for, and is never rounded. Every
//! question about it is the sign of a + b√k, or of a + b√k₁ + c√k₂ + d√(k₁k₂) when two crossings
//! are compared, decided by squaring in fixed-width integers. So order along a line or an arc,
//! equality and side are exact however close the points, and a tangency, k = 0, is exact too.
//!
//! Guarantees:
//!
//! - Every result is exact, or an error. [`sign_of_root`] and [`sign_of_two_roots`] square only when
//!   the terms' signs differ, and read off what squaring leaves in integers; nothing rounds.
//! - Filter first, certified. Each crossing keeps its coordinates as [`Interval`]s, rounded outward
//!   from its integers and an outward-rounded root, so a comparison whose intervals are apart costs
//!   two subtractions; the filter's answer is the exact sign or no answer.
//! - Nothing overflows silently. Every step is a checked operation in [`Wide`], and a step that did
//!   not fit would be [`NumError::BudgetExceeded`], never an answer. The widths below follow from
//!   [`crate::carrier`]'s budgets and are asserted when the crate compiles; every width is reached
//!   at the extreme values in `tests/attack_g189.rs`.
//!
//!   | Quantity | Bits | Where |
//!   | --- | --- | --- |
//!   | a, of a crossing | 209 | cx · N + a · h |
//!   | b, of a crossing | 42 | the carrier's own coefficients |
//!   | k, of a crossing | 336 | rr · N − h² |
//!   | w, of a crossing | 166 | cw · N |
//!   | Side of a carrier | 505 | E² − F²k, E = a · x + b · y − c · w |
//!   | Against a circle | 1,010 | E² − F²k, E and F from (Z − P) · (Z + P − 2C) |
//!   | Two crossings' coordinates | 1,506 | A² − B²k₁, A and B from the first squaring |
//!   | The turn of a grid point and two crossings | 1,688 | the same, with the fourth term |
//!
//!   The last sets [`BIG_LIMBS`]. Rendering these by the description language of `geom-gen` would
//!   take inputs wider than its `i128`, and a filter of `f64`s past 2^1023, which is why this one
//!   module's exact paths are written out, each step named by its derivation, as `plane.rs`'s are.
//! - Deterministic: integers only, except the certified filter, whose answers are exact; no platform
//!   maths and no allocation.
//!
//! Method: sign of a real algebraic number of degree two or four by repeated squaring, as chapter
//! 14.2 sets out (`projects/engines/geometry/architecture/14-2d-drafting.md`); the one-root numbers
//! are those of the circle-segment traits Fogel, Halperin and Wein describe (2012,
//! <https://doi.org/10.1007/978-3-642-17283-0>), from their book; CGAL's source not read (29.2).
//!
//! Cost: constant time and no allocation: a square root and a few dozen interval operations to make
//! a crossing, two subtractions for a filtered comparison, and at worst about thirty products of 28
//! limbs on the exact path.

use crate::arc::{Arc2, Circle2, Round};
use crate::carrier::{Carrier, Rational2};
use crate::error::NumError;
use crate::interval::Interval;
use crate::sign::Sign;
use crate::wide::{I384, Wide};

/// Limbs of the widest exact step: the turn of a grid point and two crossings, 1,688 bits.
pub const BIG_LIMBS: usize = 28;

const _: () = assert!(1_688 < BIG_LIMBS * 64);

/// Integers wide enough for every step here.
type Big = Wide<BIG_LIMBS>;

/// The limbs of a crossing's stored integers: k, the widest, is 336 bits.
const STORED_LIMBS: usize = 6;

const _: () = assert!(336 < STORED_LIMBS * 64);

/// Which of a carrier's two crossings with a circle, by their order along the way it runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Root {
    /// The one reached first.
    First,
    /// The one reached second.
    Second,
}

/// The largest step: a checked product.
fn mul(a: &Big, b: &Big) -> Result<Big, NumError> {
    a.checked_mul(b).ok_or(NumError::BudgetExceeded)
}

/// A checked sum.
fn add(a: &Big, b: &Big) -> Result<Big, NumError> {
    a.checked_add(b).ok_or(NumError::BudgetExceeded)
}

/// A checked difference.
fn sub(a: &Big, b: &Big) -> Result<Big, NumError> {
    a.checked_sub(b).ok_or(NumError::BudgetExceeded)
}

/// `a` turned.
fn neg(a: &Big) -> Result<Big, NumError> {
    a.checked_neg().ok_or(NumError::BudgetExceeded)
}

/// A wide integer at the widest width.
fn big<const L: usize>(v: &Wide<L>) -> Result<Big, NumError> {
    v.resize().ok_or(NumError::BudgetExceeded)
}

/// The dot product of two pairs.
fn dot(a: &[Big; 2], b: &[Big; 2]) -> Result<Big, NumError> {
    let [ax, ay] = a;
    let [bx, by] = b;
    add(&mul(ax, bx)?, &mul(ay, by)?)
}

/// The cross product of two pairs.
fn cross(a: &[Big; 2], b: &[Big; 2]) -> Result<Big, NumError> {
    let [ax, ay] = a;
    let [bx, by] = b;
    sub(&mul(ax, by)?, &mul(ay, bx)?)
}

/// The sign of a + b√k, for k ≥ 0. Terms of one sign settle it; terms of opposite signs have the
/// sign of the larger, which is a's when a² > b²k.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] if a square did not fit [`BIG_LIMBS`] limbs, which nothing within
/// this module's budgets can reach.
pub fn sign_of_root<const L: usize>(
    a: &Wide<L>,
    b: &Wide<L>,
    k: &Wide<L>,
) -> Result<Sign, NumError> {
    let (a, b, k) = (big(a)?, big(b)?, big(k)?);
    root_sign(&a, &b, &k)
}

fn root_sign(a: &Big, b: &Big, k: &Big) -> Result<Sign, NumError> {
    let root = if k.is_zero() { Sign::Zero } else { b.signum() };
    let (first, second) = (a.signum(), root);
    if second == Sign::Zero || first == second {
        return Ok(first);
    }
    if first == Sign::Zero {
        return Ok(second);
    }
    let larger = sub(&mul(a, a)?, &mul(&mul(b, b)?, k)?)?;
    Ok(larger.signum().mul(first))
}

/// The sign of a + b√k₁ + c√k₂ + d√(k₁k₂), for k₁, k₂ ≥ 0. Written P + Q√k₂ with P = a + b√k₁ and
/// Q = c + d√k₁, it is settled by their signs unless they differ, and then by that of
/// P² − Q²k₂ = A + B√k₁, a value of one root, with A = a² + b²k₁ − (c² + d²k₁)k₂ and
/// B = 2(ab − cd · k₂).
///
/// # Errors
///
/// As for [`sign_of_root`].
#[allow(
    clippy::many_single_char_names,
    reason = "a to d and the k's are the formula's, A and B its first squaring's"
)]
pub fn sign_of_two_roots<const L: usize>(
    a: &Wide<L>,
    b: &Wide<L>,
    c: &Wide<L>,
    d: &Wide<L>,
    k1: &Wide<L>,
    k2: &Wide<L>,
) -> Result<Sign, NumError> {
    two_roots_sign(&[big(a)?, big(b)?, big(c)?, big(d)?], &big(k1)?, &big(k2)?)
}

fn two_roots_sign(terms: &[Big; 4], k1: &Big, k2: &Big) -> Result<Sign, NumError> {
    let [a, b, c, d] = terms;
    let p = root_sign(a, b, k1)?;
    let q = if k2.is_zero() {
        Sign::Zero
    } else {
        root_sign(c, d, k1)?
    };
    if q == Sign::Zero || p == q {
        return Ok(p);
    }
    if p == Sign::Zero {
        return Ok(q);
    }
    let squares = add(&mul(a, a)?, &mul(&mul(b, b)?, k1)?)?;
    let others = add(&mul(c, c)?, &mul(&mul(d, d)?, k1)?)?;
    let whole = sub(&squares, &mul(&others, k2)?)?;
    let half = sub(&mul(a, b)?, &mul(&mul(c, d)?, k2)?)?;
    let twice = add(&half, &half)?;
    Ok(root_sign(&whole, &twice, k1)?.mul(p))
}

/// An interval holding `v`: its limbs, the highest first, each step rounded outward.
fn enclose<const L: usize>(v: &Wide<L>) -> Result<Interval, NumError> {
    let negative = v.is_negative();
    let magnitude = if negative {
        v.checked_neg().ok_or(NumError::BudgetExceeded)?
    } else {
        *v
    };
    let base = Interval::point(18_446_744_073_709_551_616.0)?;
    let mut sum = None;
    for limb in magnitude.to_le_limbs().iter().rev() {
        #[allow(
            clippy::cast_precision_loss,
            reason = "the interval's two steps cover the rounding"
        )]
        let nearest = *limb as f64;
        let part = Interval::new(nearest.next_down(), nearest.next_up())?;
        sum = match sum {
            Some(high) => Some(Interval::mul(high, base)?.add(part)?),
            None if *limb == 0 => None,
            None => Some(part),
        };
    }
    // Nothing but zero limbs is zero exactly, the root of k = 0.
    let sum = sum.map_or_else(|| Interval::point(0.0), Ok)?;
    Ok(if negative { sum.neg() } else { sum })
}

/// A point of the plane at ((a₀ + b₀√k) / w, (a₁ + b₁√k) / w): a crossing of a carrier and a
/// circle, or a rational point, whose b's and k are zero. One root, shared by both coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Crossing2 {
    a: [I384; 2],
    b: [I384; 2],
    k: I384,
    w: I384,
    /// Where it lies, certainly: the filter's whole answer.
    at: [Interval; 2],
}

/// One coordinate of a [`Crossing2`], as the exact paths take it.
struct Part {
    a: Big,
    b: Big,
    k: Big,
    w: Big,
}

impl Crossing2 {
    /// Makes a crossing from its integers, its enclosure from theirs.
    fn from_parts(a: [I384; 2], b: [I384; 2], k: I384, w: I384) -> Result<Crossing2, NumError> {
        let root = enclose(&k)?.sqrt()?;
        let weight = enclose(&w)?;
        let coordinate = |a: &I384, b: &I384| -> Result<Interval, NumError> {
            let along = enclose(b)?.mul(root)?;
            enclose(a)?.add(along)?.div(weight)
        };
        let [ax, ay] = &a;
        let [bx, by] = &b;
        let at = [coordinate(ax, bx)?, coordinate(ay, by)?];
        Ok(Crossing2 { a, b, k, w, at })
    }

    /// Where carrier `l` meets the circle about (cx, cy) / cw, of squared radius rr / cw².
    /// N = a² + b², h = c · cw − a · cx − b · cy and k = rr · N − h², so the crossing is
    /// (C · N + n · h ± d√k) / (cw · N), d = (b, −a) the way the carrier runs.
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b, c, h, k and n are the formula's, l the carrier"
    )]
    fn on_circle(
        l: &Carrier,
        centre: &[Big; 2],
        cw: &Big,
        rr: &Big,
        root: Root,
    ) -> Result<Crossing2, NumError> {
        let ([a, b], c) = l.coefficients();
        let (a, b, c) = (Big::from_i128(a), Big::from_i128(b), Big::from_i128(c));
        let [cx, cy] = centre;
        let n = add(&mul(&a, &a)?, &mul(&b, &b)?)?;
        let h = sub(&sub(&mul(&c, cw)?, &mul(&a, cx)?)?, &mul(&b, cy)?)?;
        let k = sub(&mul(rr, &n)?, &mul(&h, &h)?)?;
        if k.is_negative() {
            return Err(NumError::Degenerate);
        }
        let stored =
            |v: Big| -> Result<I384, NumError> { v.resize().ok_or(NumError::BudgetExceeded) };
        let ax = stored(add(&mul(cx, &n)?, &mul(&a, &h)?)?)?;
        let ay = stored(add(&mul(cy, &n)?, &mul(&b, &h)?)?)?;
        let ([da, db], _) = l.coefficients();
        let (ahead, across) = (db, da);
        let turned = |v: i128| v.checked_neg().ok_or(NumError::BudgetExceeded);
        let bs = match root {
            Root::First => [turned(ahead)?, across],
            Root::Second => [ahead, turned(across)?],
        }
        .map(I384::from_i128);
        Crossing2::from_parts([ax, ay], bs, stored(k)?, stored(mul(cw, &n)?)?)
    }

    /// Where the carrier `l` meets the circle `round`, `root` choosing which.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] when they do not meet; [`NumError::BudgetExceeded`] if a step did
    /// not fit, which no input in the budget can reach.
    pub fn of_round(l: &Carrier, round: &Round, root: Root) -> Result<Crossing2, NumError> {
        let ([cx, cy], cw) = round.centre().coordinates();
        let on = round.on();
        let (cx, cy, cw) = (Big::from_i128(cx), Big::from_i128(cy), Big::from_i128(cw));
        let dx = sub(&mul(&Big::from_i64(on.x.get()), &cw)?, &cx)?;
        let dy = sub(&mul(&Big::from_i64(on.y.get()), &cw)?, &cy)?;
        let rr = add(&mul(&dx, &dx)?, &mul(&dy, &dy)?)?;
        Crossing2::on_circle(l, &[cx, cy], &cw, &rr, root)
    }

    /// Where the carrier `l` meets the circle `circle`, `root` choosing which.
    ///
    /// # Errors
    ///
    /// As for [`Crossing2::of_round`].
    pub fn of_circle(l: &Carrier, circle: &Circle2, root: Root) -> Result<Crossing2, NumError> {
        let c = circle.centre();
        let r = Big::from_i128(circle.radius());
        let centre = [Big::from_i64(c.x.get()), Big::from_i64(c.y.get())];
        Crossing2::on_circle(l, &centre, &Big::from_i64(1), &mul(&r, &r)?, root)
    }

    /// A rational point in this form, b and k zero, so that it can be compared with a crossing.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step did not fit, which no point in the budget can reach.
    pub fn of_point(p: &Rational2) -> Result<Crossing2, NumError> {
        let ([x, y], w) = p.coordinates();
        Crossing2::from_parts(
            [I384::from_i128(x), I384::from_i128(y)],
            [I384::from_i64(0), I384::from_i64(0)],
            I384::from_i64(0),
            I384::from_i128(w),
        )
    }

    /// Its homogeneous parts: `[a₀, a₁]`, `[b₀, b₁]`, k and w, w positive.
    #[must_use]
    pub const fn parts(&self) -> ([I384; 2], [I384; 2], I384, I384) {
        (self.a, self.b, self.k, self.w)
    }

    /// Where it lies, certainly.
    #[must_use]
    pub const fn enclosure(&self) -> [Interval; 2] {
        self.at
    }

    /// One coordinate, at the widest width.
    fn part(&self, axis: usize) -> Result<Part, NumError> {
        let (Some(a), Some(b)) = (self.a.get(axis), self.b.get(axis)) else {
            return Err(NumError::BudgetExceeded);
        };
        Ok(Part {
            a: big(a)?,
            b: big(b)?,
            k: big(&self.k)?,
            w: big(&self.w)?,
        })
    }

    /// The exact sign of this coordinate less `other`'s: over the weights' product, α + β√k₁ + γ√k₂
    /// with α = a₁ · w₂ − a₂ · w₁, β = b₁ · w₂ and γ = −b₂ · w₁; one root when the k's are equal.
    fn compare(&self, other: &Crossing2, axis: usize) -> Result<Sign, NumError> {
        let (p, q) = (self.part(axis)?, other.part(axis)?);
        let alpha = sub(&mul(&p.a, &q.w)?, &mul(&q.a, &p.w)?)?;
        let beta = mul(&p.b, &q.w)?;
        let gamma = neg(&mul(&q.b, &p.w)?)?;
        if p.k == q.k {
            return root_sign(&alpha, &add(&beta, &gamma)?, &p.k);
        }
        two_roots_sign(&[alpha, beta, gamma, Big::from_i64(0)], &p.k, &q.k)
    }

    /// The filter's answer on `axis`: the sign when the enclosures are apart.
    fn apart(&self, other: &Crossing2, axis: usize) -> Option<Sign> {
        let (p, q) = (self.at.get(axis)?, other.at.get(axis)?);
        match p.sub(*q).ok()?.certain_sign() {
            Some(Sign::Zero) | None => None,
            sign => sign,
        }
    }

    /// [`Crossing2::cmp_x`]'s filter: the sign, or `None` when the enclosures overlap.
    #[must_use]
    pub fn cmp_x_filter(&self, other: &Crossing2) -> Option<Sign> {
        self.apart(other, 0)
    }

    /// How this point's x compares with `other`'s.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step did not fit, which no input in the budget can reach.
    pub fn cmp_x(&self, other: &Crossing2) -> Result<Sign, NumError> {
        self.apart(other, 0)
            .map_or_else(|| self.compare(other, 0), Ok)
    }

    /// How this point's y compares with `other`'s.
    ///
    /// # Errors
    ///
    /// As for [`Crossing2::cmp_x`].
    pub fn cmp_y(&self, other: &Crossing2) -> Result<Sign, NumError> {
        self.apart(other, 1)
            .map_or_else(|| self.compare(other, 1), Ok)
    }

    /// Whether this point and `other` are one place, however they were made.
    ///
    /// # Errors
    ///
    /// As for [`Crossing2::cmp_x`].
    pub fn same(&self, other: &Crossing2) -> Result<bool, NumError> {
        Ok(self.cmp_x(other)? == Sign::Zero && self.cmp_y(other)? == Sign::Zero)
    }

    /// Which side of the carrier `l` it lies on, as [`Carrier::side`] says of a rational point: the
    /// sign of E + F√k with E = a · a₀ + b · a₁ − c · w and F = a · b₀ + b · b₁.
    ///
    /// # Errors
    ///
    /// As for [`Crossing2::cmp_x`].
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b and c are the carrier's, x and y the coordinates, l the carrier"
    )]
    pub fn side(&self, l: &Carrier) -> Result<Sign, NumError> {
        let ([a, b], c) = l.coefficients();
        let exact = |v: i128| enclose(&I384::from_i128(v));
        let [at_x, at_y] = self.at;
        let value = exact(a)?
            .mul(at_x)?
            .add(exact(b)?.mul(at_y)?)?
            .sub(exact(c)?)?;
        if let Some(sign @ (Sign::Positive | Sign::Negative)) = value.certain_sign() {
            return Ok(sign);
        }
        let (a, b, c) = (Big::from_i128(a), Big::from_i128(b), Big::from_i128(c));
        let (x, y) = (self.part(0)?, self.part(1)?);
        let whole = sub(&add(&mul(&a, &x.a)?, &mul(&b, &y.a)?)?, &mul(&c, &x.w)?)?;
        let root = add(&mul(&a, &x.b)?, &mul(&b, &y.b)?)?;
        root_sign(&whole, &root, &x.k)
    }

    /// Where it lies against the circle `round`: [`Sign::Negative`] inside, [`Sign::Zero`] on it,
    /// [`Sign::Positive`] outside. With Z = (A + B√k) / W, U = A − W · P and T = cw(A + W · P) − 2W · C,
    /// the value (Z − P) · (Z + P − 2C), times W² · cw, is E + F√k with
    /// E = U · T + cw · ‖B‖²k and F = cw · U · B + T · B.
    ///
    /// # Errors
    ///
    /// As for [`Crossing2::cmp_x`].
    #[allow(
        clippy::many_single_char_names,
        reason = "b, c, p, t, u, x and y are the formula's"
    )]
    pub fn against_round(&self, round: &Round) -> Result<Sign, NumError> {
        let ([cx, cy], cw) = round.centre().coordinates();
        let on = round.on();
        let (x, y) = (self.part(0)?, self.part(1)?);
        let (cw, c) = (Big::from_i128(cw), [Big::from_i128(cx), Big::from_i128(cy)]);
        let p = [Big::from_i64(on.x.get()), Big::from_i64(on.y.get())];
        let [px, py] = &p;
        let [cx, cy] = &c;
        let scale_w = |v: &Big| mul(v, &x.w);
        let u = [sub(&x.a, &scale_w(px)?)?, sub(&y.a, &scale_w(py)?)?];
        let twice = |v: &Big| add(v, v);
        let t = [
            sub(
                &mul(&cw, &add(&x.a, &scale_w(px)?)?)?,
                &twice(&scale_w(cx)?)?,
            )?,
            sub(
                &mul(&cw, &add(&y.a, &scale_w(py)?)?)?,
                &twice(&scale_w(cy)?)?,
            )?,
        ];
        let b = [x.b, y.b];
        let whole = add(&dot(&u, &t)?, &mul(&mul(&cw, &dot(&b, &b)?)?, &x.k)?)?;
        let root = add(&mul(&cw, &dot(&u, &b)?)?, &dot(&t, &b)?)?;
        root_sign(&whole, &root, &x.k)
    }

    /// Where it lies against the circle `circle`, as [`Crossing2::against_round`] says: with
    /// U = A − W · C, the value ‖Z − C‖² − r², times W², is E + F√k with
    /// E = ‖U‖² + ‖B‖²k − r²W² and F = 2U · B.
    ///
    /// # Errors
    ///
    /// As for [`Crossing2::cmp_x`].
    #[allow(
        clippy::many_single_char_names,
        reason = "b, c, r, u, x and y are the formula's"
    )]
    pub fn against_circle(&self, circle: &Circle2) -> Result<Sign, NumError> {
        let (x, y) = (self.part(0)?, self.part(1)?);
        let c = circle.centre();
        let r = Big::from_i128(circle.radius());
        let scale_w = |v: i64| mul(&Big::from_i64(v), &x.w);
        let u = [
            sub(&x.a, &scale_w(c.x.get())?)?,
            sub(&y.a, &scale_w(c.y.get())?)?,
        ];
        let b = [x.b, y.b];
        let rw = mul(&r, &x.w)?;
        let whole = sub(
            &add(&dot(&u, &u)?, &mul(&dot(&b, &b)?, &x.k)?)?,
            &mul(&rw, &rw)?,
        )?;
        let half = dot(&u, &b)?;
        root_sign(&whole, &add(&half, &half)?, &x.k)
    }

    /// Which way along the carrier `l` `other` lies from this point, as [`Carrier::order`] says.
    ///
    /// # Errors
    ///
    /// As for [`Crossing2::cmp_x`].
    pub fn order(&self, l: &Carrier, other: &Crossing2) -> Result<Sign, NumError> {
        let [dx, dy] = l.direction();
        let (sx, sy) = (other.cmp_x(self)?, other.cmp_y(self)?);
        if dx == 0 {
            return Ok(if dy > 0 { sy } else { sy.negate() });
        }
        Ok(if dx > 0 { sx } else { sx.negate() })
    }
}

impl Arc2 {
    /// Whether the crossing `z` of its circle lies on the arc, as [`Arc2::holds`] says.
    ///
    /// # Errors
    ///
    /// As for [`Crossing2::cmp_x`].
    pub fn holds_crossing(&self, z: &Crossing2) -> Result<bool, NumError> {
        let chord = Carrier::through(self.start(), self.end())?;
        Ok(z.side(&chord)? != Sign::Positive)
    }

    /// Which of two crossings of the arc comes first from its start, as [`Arc2::order`] says: the
    /// turn of the start and the two, over the weights, is α + β√k₁ + γ√k₂ + δ√(k₁k₂) with
    /// Uᵢ = Aᵢ − Wᵢ · S, α = U₁ × U₂, β = B₁ × U₂, γ = U₁ × B₂ and δ = B₁ × B₂.
    ///
    /// # Errors
    ///
    /// As for [`Crossing2::cmp_x`].
    pub fn order_crossings(&self, a: &Crossing2, b: &Crossing2) -> Result<Sign, NumError> {
        let start = self.start();
        let (sx, sy) = (Big::from_i64(start.x.get()), Big::from_i64(start.y.get()));
        let relative = |c: &Crossing2| -> Result<([Big; 2], [Big; 2]), NumError> {
            let (x, y) = (c.part(0)?, c.part(1)?);
            Ok((
                [sub(&x.a, &mul(&sx, &x.w)?)?, sub(&y.a, &mul(&sy, &x.w)?)?],
                [x.b, y.b],
            ))
        };
        let (ua, ba) = relative(a)?;
        let (ub, bb) = relative(b)?;
        let (ka, kb) = (big(&a.k)?, big(&b.k)?);
        let (alpha, beta) = (cross(&ua, &ub)?, cross(&ba, &ub)?);
        let (gamma, delta) = (cross(&ua, &bb)?, cross(&ba, &bb)?);
        if ka == kb {
            return root_sign(&add(&alpha, &mul(&delta, &ka)?)?, &add(&beta, &gamma)?, &ka);
        }
        two_roots_sign(&[alpha, beta, gamma, delta], &ka, &kb)
    }
}
