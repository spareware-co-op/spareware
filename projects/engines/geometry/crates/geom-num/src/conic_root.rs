//! Conic-root points in 2D, exact: where an ellipse meets a circle or another ellipse, kept as a
//! real root of their quartic, and the questions about one that its own ellipse can answer.
//!
//! What it does: a [`ConicRoot`] is one real root of another conic's equation along an ellipse
//! (`Ellipse2::meeting`), chapter 14.2's fourth point class, never rounded. Every question about
//! it is the sign of a polynomial at that root:
//!
//! - [`ConicRoot::sign_of`] is the sign of any conic's equation there: which side of a line it lies
//!   on, inside or outside a circle or an ellipse. The conic along the ellipse is a quartic, signed
//!   at the root by `quartic::sign_at_root`.
//! - [`ConicRoot::cmp_rational`] places its x or y against a rational point's: the side of the
//!   vertical or horizontal line through that point.
//! - [`ConicRoot::half`] says which way its x or y goes as the ellipse runs counter-clockwise: the
//!   sign of the coordinate's derivative in θ, which splits the ellipse into two halves between the
//!   points where the coordinate is least and greatest, and is zero at those two.
//! - [`ConicRoot::cmp_along`] places its x or y against another conic root of the same ellipse.
//!   Along a half the coordinate is monotone, so two points of one half compare as they lie along
//!   it: by t, or by −1 / t on the half that passes θ = π, where t passes ∞ (`Quartic::turned`).
//!   A point of the other half is first carried to this one by the mirror that keeps the
//!   coordinate, t ↦ −(a + b t) / (b − a t), whose image of the quartic is past the first
//!   instance's budget and so is taken in the second (`quartic::SecondQuartic`, chapter 14.2's
//!   as-built note of G196b). The mirror reverses order along the halves, so the carried point is
//!   found by its rank.
//!
//! Each point keeps a certified enclosure of its place, from its root narrowed [`NARROW`] levels,
//! so a comparison of points that are apart costs a few interval operations.
//!
//! Guarantees:
//!
//! - **Exact.** Every answer is a sign of whole numbers through `quartic`; the enclosure answers
//!   only when its sign is certain. A point of another ellipse is not this module's to compare:
//!   [`ConicRoot::cmp_along`] says so with `None`.
//! - **Within the budgets.** The ellipse's equation for a conic of `Conic::new`'s stated widths is
//!   within `quartic::COEFFICIENT_BITS` (`conic`'s asserts), and so is every polynomial here but the
//!   mirror's image: the coordinate's numerator and derivative about the centre have parts below
//!   2^64, and the line through a rational point (X / W, W below 2^83 and X below 2^124) has a
//!   first-degree coefficient W and a constant X − W · K below 2^125. The mirror multiplies each of
//!   the quartic's coefficients by four factors below 2^64 and sums five terms: below
//!   2^(297 + 256 + 3), within `quartic::SECOND_COEFFICIENT_BITS`.
//! - **Deterministic.** Integers and certified intervals only; no platform maths, no allocation.
//!
//! Method: the rational parametrisation of the ellipse by t = tan(θ / 2), as `conic` sets it out,
//! with chapter 14.2's real-root isolation and exact common-root tests
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`); the monotone halves of a closed
//! convex curve and the involution that pairs the ends of its parallel chords are textbook analytic
//! geometry.
//!
//! Cost: one `Ellipse2::meeting` and [`NARROW`] levels of narrowing to make a point; one
//! `quartic::sign_at_root` per question; one `quartic::compare` for two points of one half, and
//! for two halves a few dozen products for the mirror's quartic, its roots in the second instance,
//! and up to a dozen comparisons to rank them. No recursion.

#![allow(
    clippy::many_single_char_names,
    reason = "a, b, c, e, g, h, k, p, q, r, s, t and w are the geometry's and the roots' own letters"
)]

use crate::carrier::Rational2;
use crate::conic::{Conic, Ellipse2, Meeting};
use crate::error::NumError;
use crate::interval::Interval;
use crate::one_root::enclose;
use crate::quartic::{
    Big, COEFFICIENT_BITS, Instance, Piece, Quartic, QuarticOf, RealRoot, RealRootOf,
    SECOND_COEFFICIENT_BITS, SECOND_LIMBS, SecondQuartic, SecondRoot, compare, sign_at_root,
};
use crate::sign::Sign;
use crate::wide::Wide;

// The mirror's image: a first-instance coefficient by four factors below 2^64, five terms.
const _: () = assert!(COEFFICIENT_BITS + 4 * 64 + 3 < SECOND_COEFFICIENT_BITS);

/// Levels to which a point's root is narrowed for its enclosure: 2^−64 of the parameter, as the
/// arrangement narrows the roots it weighs.
pub const NARROW: u32 = 64;

/// Which coordinate a question is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// x.
    X,
    /// y.
    Y,
}

/// A point where an ellipse meets another conic: one real root of the other's equation along the
/// ellipse.
#[derive(Clone, Copy, Debug)]
pub struct ConicRoot {
    ellipse: Ellipse2,
    quartic: Quartic,
    root: RealRoot,
    enclosure: [Interval; 2],
}

fn budget<T>(v: Option<T>) -> Result<T, NumError> {
    v.ok_or(NumError::BudgetExceeded)
}

fn mul(a: &Big, b: &Big) -> Result<Big, NumError> {
    budget(a.checked_mul(b))
}

fn sub(a: &Big, b: &Big) -> Result<Big, NumError> {
    budget(a.checked_sub(b))
}

fn big(v: i64) -> Big {
    Big::from_i64(v)
}

/// An `i64` as a certified interval.
fn interval(v: i64) -> Result<Interval, NumError> {
    enclose(&big(v))
}

impl ConicRoot {
    /// The `index`-th real root, in order along `ellipse` from θ = −π, of `other`'s equation along
    /// it.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] if `other` is `ellipse`'s own curve, or has no more real roots along
    /// it; otherwise as for `Ellipse2::meeting`.
    pub fn of(ellipse: &Ellipse2, other: &Conic, index: usize) -> Result<ConicRoot, NumError> {
        let Meeting::At { quartic, roots } = ellipse.meeting(other)? else {
            return Err(NumError::Degenerate);
        };
        let narrowed = roots.narrowed(&quartic, NARROW)?;
        let root = *narrowed.iter().nth(index).ok_or(NumError::Degenerate)?;
        let [cos, sin] = root.angle()?;
        let k = ellipse.centre();
        let [mx, my] = ellipse.major();
        let (p, q) = ellipse.ratio();
        let ratio = interval(p)?.div(interval(q)?)?;
        let x = interval(k.x.get())?
            .add(interval(mx)?.mul(cos)?)?
            .sub(ratio.mul(interval(my)?)?.mul(sin)?)?;
        let y = interval(k.y.get())?
            .add(interval(my)?.mul(cos)?)?
            .add(ratio.mul(interval(mx)?)?.mul(sin)?)?;
        Ok(ConicRoot {
            ellipse: *ellipse,
            quartic,
            root,
            enclosure: [x, y],
        })
    }

    /// The ellipse it lies on.
    #[must_use]
    pub const fn ellipse(&self) -> Ellipse2 {
        self.ellipse
    }

    /// The other conic's equation along the ellipse, of which it is a root.
    #[must_use]
    pub const fn quartic(&self) -> Quartic {
        self.quartic
    }

    /// The root, narrowed [`NARROW`] levels.
    #[must_use]
    pub const fn root(&self) -> RealRoot {
        self.root
    }

    /// Where it lies, certainly: x and y.
    #[must_use]
    pub const fn enclosure(&self) -> [Interval; 2] {
        self.enclosure
    }

    /// The sign of `c`'s equation here: zero on it, and otherwise which side, as the equation's
    /// value has it.
    ///
    /// # Errors
    ///
    /// As for `quartic::sign_at_root`, which a conic of `Conic::new`'s stated widths cannot reach.
    pub fn sign_of(&self, c: &Conic) -> Result<Sign, NumError> {
        sign_at_root(&self.ellipse.quartic(c)?, &self.quartic, &self.root)
    }

    /// The sign of its x (`Axis::X`) or y less `z`'s.
    ///
    /// # Errors
    ///
    /// As for [`ConicRoot::sign_of`].
    pub fn cmp_rational(&self, z: &Rational2, axis: Axis) -> Result<Sign, NumError> {
        let ([zx, zy], w) = z.coordinates();
        let k = self.ellipse.centre();
        let [ex, ey] = self.enclosure;
        let (part, at, enclosed) = match axis {
            Axis::X => (zx, k.x.get(), ex),
            Axis::Y => (zy, k.y.get(), ey),
        };
        let fraction = enclose(&Big::from_i128(part))?.div(enclose(&Big::from_i128(w))?)?;
        if let Some(sign @ (Sign::Positive | Sign::Negative)) =
            enclosed.sub(fraction)?.certain_sign()
        {
            return Ok(sign);
        }
        // W · (c − K) − (Z − W · K) = W · c − Z, W > 0: the line c = Z / W, about the centre.
        let w = Big::from_i128(w);
        let offset = sub(&Big::from_i128(part), &mul(&w, &big(at))?)?;
        let constant = sub(&big(0), &offset)?;
        let zero = big(0);
        let coefficients = match axis {
            Axis::X => [zero, zero, zero, w, zero, constant],
            Axis::Y => [zero, zero, zero, zero, w, constant],
        };
        self.sign_of(&Conic::new(k, coefficients))
    }

    /// Which way its x (`Axis::X`) or y goes as the ellipse runs counter-clockwise:
    /// [`Sign::Positive`] rising, [`Sign::Negative`] falling, [`Sign::Zero`] at the least or the
    /// greatest. Each half of the ellipse between those two points has one sign.
    ///
    /// # Errors
    ///
    /// As for [`ConicRoot::sign_of`].
    pub fn half(&self, axis: Axis) -> Result<Sign, NumError> {
        half_of(&self.ellipse, axis, &self.quartic, &self.root)
    }

    /// The sign of its x (`Axis::X`) or y less `other`'s, both on one ellipse; `None` for a point
    /// of another ellipse, which this module does not compare.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step did not fit, which the module's budgets keep every
    /// input from.
    pub fn cmp_along(&self, other: &ConicRoot, axis: Axis) -> Result<Option<Sign>, NumError> {
        if self.ellipse != other.ellipse {
            return Ok(None);
        }
        let (h, g) = (self.half(axis)?, other.half(axis)?);
        if h == Sign::Zero || g == Sign::Zero {
            // An end of the coordinate's range is above every other point at the greatest and
            // below every other at the least; two ends are one point or the two ends.
            let (s, t) = (self.beyond_centre(axis)?, other.beyond_centre(axis)?);
            return Ok(Some(match (h, g) {
                (Sign::Zero, Sign::Zero) if s == t => Sign::Zero,
                (Sign::Zero, _) => s,
                _ => t.negate(),
            }));
        }
        let along = if h == g {
            along_half(
                &self.ellipse,
                axis,
                h,
                (&self.quartic, &self.root),
                (&other.quartic, &other.root),
            )?
        } else {
            let (mine, root) = (self.quartic.to_second()?, self.root.to_second()?);
            let (image, carried) = other.mirrored(axis, g)?;
            along_half(&self.ellipse, axis, h, (&mine, &root), (&image, &carried))?
        };
        // Along a rising half the coordinate follows the order along it; along a falling one, back.
        Ok(Some(if h == Sign::Positive {
            along
        } else {
            along.negate()
        }))
    }

    /// The sign of its coordinate less the centre's: the coordinate's numerator about the centre,
    /// at t = ∞ its coefficient of t², as θ = π is K − M.
    fn beyond_centre(&self, axis: Axis) -> Result<Sign, NumError> {
        let [n0, n1, n2] = numerator(&self.ellipse, axis)?;
        if self.root.piece() == Piece::Infinity {
            return Ok(n2.signum());
        }
        let zero = big(0);
        sign_at_root(
            &Quartic::new([n0, n1, n2, zero, zero]),
            &self.quartic,
            &self.root,
        )
    }

    /// This point carried, by the mirror that keeps `axis`'s coordinate, from its half (`half`) to
    /// the other: the mirror's image of its quartic in the second instance, and the root of it that
    /// is this one's image. The mirror reverses order along the halves, so that root has this one's
    /// rank among its quartic's roots on its half, counted from the other end.
    fn mirrored(&self, axis: Axis, half: Sign) -> Result<(SecondQuartic, SecondRoot), NumError> {
        let mine = on_half(&self.ellipse, axis, half, &self.quartic)?;
        let mut rank = None;
        for (i, r) in mine.iter().enumerate() {
            if let Some(r) = r
                && compare(&self.quartic, r, &self.quartic, &self.root)? == Sign::Zero
            {
                rank = Some(i);
            }
        }
        let image = mirror(&self.ellipse, axis, &self.quartic)?;
        let theirs = on_half(&self.ellipse, axis, half.negate(), &image)?;
        // Their roots fill the list from its start, so the one `rank` places from the other end
        // is the `rank`-th of them taken backwards.
        let rank = budget(rank)?;
        let carried = theirs.iter().flatten().rev().nth(rank).copied();
        Ok((image, budget(carried)?))
    }
}

/// The parts m and m' of the major axis along and across `axis` (m' turned for x), times q and p:
/// (q · m, p · m'), each below 2^63.
fn parts(e: &Ellipse2, axis: Axis) -> Result<(Big, Big), NumError> {
    let ([mx, my], (p, q)) = (e.major(), e.ratio());
    let (along, across) = match axis {
        Axis::X => (big(mx), sub(&big(0), &big(my))?),
        Axis::Y => (big(my), big(mx)),
    };
    Ok((mul(&big(q), &along)?, mul(&big(p), &across)?))
}

/// The numerator of `axis`'s coordinate less the centre's, over q(1 + t²): for x,
/// q·mx (1 − t²) − 2p·my t, and for y, q·my (1 − t²) + 2p·mx t.
fn numerator(e: &Ellipse2, axis: Axis) -> Result<[Big; 3], NumError> {
    let (along, across) = parts(e, axis)?;
    Ok([along, mul(&big(2), &across)?, sub(&big(0), &along)?])
}

/// Half the numerator of the coordinate's derivative in t: with N = n₀ + n₁ t + n₂ t² its
/// numerator, d(N / (1 + t²)) / dt is (n₁ + 2(n₂ − n₀) t − n₁ t²) / (1 + t²)², here
/// p·m' − 2q·m t − p·m' t².
fn rate(e: &Ellipse2, axis: Axis) -> Result<[Big; 5], NumError> {
    let (along, across) = parts(e, axis)?;
    let zero = big(0);
    Ok([
        across,
        sub(&zero, &mul(&big(2), &along)?)?,
        sub(&zero, &across)?,
        zero,
        zero,
    ])
}

/// `c` as a quartic of the instance `B`.
fn in_instance<B: Instance, const L: usize, const W: usize>(
    c: &[Big; 5],
) -> Result<QuarticOf<B, L, W>, NumError> {
    let [c0, c1, c2, c3, c4] = c;
    let to = |v: &Big| budget(v.resize::<L>());
    Ok(QuarticOf::new([
        to(c0)?,
        to(c1)?,
        to(c2)?,
        to(c3)?,
        to(c4)?,
    ]))
}

/// The half of the root `r` of `q` along `e` for `axis`: the sign of the coordinate's derivative
/// there, at t = ∞ that of its coefficient of t².
fn half_of<B: Instance, const L: usize, const W: usize>(
    e: &Ellipse2,
    axis: Axis,
    q: &QuarticOf<B, L, W>,
    r: &RealRootOf<L>,
) -> Result<Sign, NumError> {
    let derivative = rate(e, axis)?;
    if r.piece() == Piece::Infinity {
        let [_, _, top, _, _] = derivative;
        return Ok(top.signum());
    }
    sign_at_root(&in_instance::<B, L, W>(&derivative)?, q, r)
}

/// The order along the half `half` of `e` of two roots of quartics of one instance: t's order,
/// or −1 / t's on the half that passes θ = π, the half whose sign the derivative has at ∞.
fn along_half<B: Instance, const L: usize, const W: usize>(
    e: &Ellipse2,
    axis: Axis,
    half: Sign,
    first: (&QuarticOf<B, L, W>, &RealRootOf<L>),
    second: (&QuarticOf<B, L, W>, &RealRootOf<L>),
) -> Result<Sign, NumError> {
    let [_, _, top, _, _] = rate(e, axis)?;
    let ((p, r), (q, s)) = (first, second);
    if top.signum() == half {
        compare(&p.turned()?, &r.turned(), &q.turned()?, &s.turned())
    } else {
        compare(p, r, q, s)
    }
}

/// The roots of `q` on the half `half` of `e` for `axis`, in order along it.
fn on_half<B: Instance, const L: usize, const W: usize>(
    e: &Ellipse2,
    axis: Axis,
    half: Sign,
    q: &QuarticOf<B, L, W>,
) -> Result<[Option<RealRootOf<L>>; 4], NumError> {
    let mut out: [Option<RealRootOf<L>>; 4] = [None; 4];
    let mut slots = out.iter_mut();
    for r in q.roots()?.iter() {
        if half_of(e, axis, q, r)? == half {
            *budget(slots.next())? = Some(*r);
        }
    }
    // Insertion, on at most four.
    for k in 1..4_usize {
        let mut at = k;
        while let Some(before) = at.checked_sub(1) {
            let (Some(Some(a)), Some(Some(b))) = (out.get(before).copied(), out.get(at).copied())
            else {
                break;
            };
            if along_half(e, axis, half, (q, &a), (q, &b))? != Sign::Positive {
                break;
            }
            out.swap(before, at);
            at = before;
        }
    }
    Ok(out)
}

/// The image of `q` under the mirror that keeps `axis`'s coordinate, t ↦ −(a + b t) / (b − a t)
/// with a = p·m' and b = −q·m (an involution: its matrix has trace zero), in the second instance:
/// Σ cᵢ (−(a + b t))ⁱ (b − a t)^(4 − i), whose roots are the mirrors of `q`'s.
fn mirror(e: &Ellipse2, axis: Axis, q: &Quartic) -> Result<SecondQuartic, NumError> {
    let (along, across) = parts(e, axis)?;
    let wide = |v: &Big| budget(v.resize::<SECOND_LIMBS>());
    let (a, b) = (wide(&across)?, wide(&sub(&big(0), &along)?)?);
    let zero = Wide::<SECOND_LIMBS>::from_i64(0);
    let neg = |v: &Wide<SECOND_LIMBS>| budget(v.checked_neg());
    let up = [neg(&a)?, neg(&b)?, zero, zero, zero];
    let down = [b, neg(&a)?, zero, zero, zero];
    let times = |x: &[Wide<SECOND_LIMBS>; 5],
                 y: &[Wide<SECOND_LIMBS>; 5]|
     -> Result<[Wide<SECOND_LIMBS>; 5], NumError> {
        let mut out = [zero; 5];
        for (i, u) in x.iter().enumerate() {
            for (j, v) in y.iter().enumerate() {
                let term = budget(u.checked_mul(v))?;
                if term.is_zero() {
                    continue;
                }
                let slot = budget(out.get_mut(i.saturating_add(j)))?;
                *slot = budget(slot.checked_add(&term))?;
            }
        }
        Ok(out)
    };
    let mut sum = [zero; 5];
    for (i, c) in q.coefficients().iter().enumerate() {
        let mut term = [Wide::<SECOND_LIMBS>::from_i64(1), zero, zero, zero, zero];
        for k in 0..4_usize {
            term = times(&term, if k < i { &up } else { &down })?;
        }
        let c = wide(c)?;
        for (slot, t) in sum.iter_mut().zip(term.iter()) {
            *slot = budget(slot.checked_add(&budget(c.checked_mul(t))?))?;
        }
    }
    Ok(SecondQuartic::new(sum))
}
