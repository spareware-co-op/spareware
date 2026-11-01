//! Tangent constructions: fillets, tangent circles and lines, and common tangents, with exact joins.
//!
//! What it does: the ideal result of a tangent construction needs square roots that stay closed under
//! nothing, and rounding it to the grid opens a gap (chapter 14.3). So a construction takes a nearby
//! rational choice and builds everything else exactly from it. Where both lines' normals are rational
//! the fillet is exact: its circle is tangent to each line, a discriminant exactly zero. Everywhere
//! else the circle or line goes a hair past its neighbour, so that the two cross at an exact one-root
//! or conic-root point of their own carriers, which is the join, and the result is Certified with
//! the largest direction gap at a join, always within what the caller accepted.
//!
//! Guarantees:
//!
//! - **Every join is an exact shared point.** A join is a [`Pt2`] defined by the two carriers it
//!   joins, so it is on both whatever is asked of it; nothing is snapped.
//! - **No extra contact.** A curve crossing its neighbour at a hair's angle crosses it twice, the two
//!   a short chord apart, and between them it is on the far side: a cap. [`Fillet`] leaves one cap
//!   from its counter-clockwise end and arrives at the other's clockwise end, so the arc holds neither
//!   cap, and it is refused unless each cap is proven clear of the other. Likewise a line kept from
//!   the near crossing of one circle to the far crossing of another, and a [`Join`]'s two points.
//! - **Exact or within the bounds.** Exact when no choice was needed, with a gap of exactly zero.
//!   Otherwise the gap is from the exact discriminants, rounded up, and the join stays within half a
//!   step of the ideal where the numbers allow it; [`Accept::admit`] refuses what the caller did not
//!   accept. Chapter 14.3's ε_ang is the ladder's angular bound, 2^−34.
//! - **Within the budget.** A centre is a [`Rational2`] and a squared radius within [`Disc`]'s, and a
//!   line is a [`Carrier`] where it fits and a [`FineLine`] where it does not, so every width is
//!   `geom-num`'s.
//! - **Atomic.** A construction that fails adds nothing to the table.
//! - **Deterministic.** The aim is `crate::ideal`'s, integers do the rest; each corner of
//!   [`fillet_all`] is alone, so the result does not depend on how the work is shared.
//!
//! Method: chapter 14.3's rational choices (`projects/engines/geometry/architecture/14-2d-drafting.md`):
//! a centre within η ≈ ε_ang² · r / 2 of the ideal, a line in a whole direction, the simplest fraction
//! in the window of directions that cross, by continued fractions (Canny, Donald and Ressler 1992,
//! <https://doi.org/10.1145/142675.142726>, for rational directions). The fillet of two lines is by
//! offset, intersect and project (Rossignac and Requicha 1986,
//! <https://doi.org/10.1016/0167-8396(86)90017-8>).
//!
//! Cost: a few hundred double-double operations and a few dozen products of wide integers for a
//! corner, about 100 µs; a curve with an ellipse adds its quartic and its roots. [`fillet_all`] is
//! linear in its corners.

use alloc::vec::Vec;

use geom_num::arc::{Disc, RADIUS_BITS, Round};
use geom_num::carrier::{Carrier, FineLine, Rational2};
use geom_num::conic::{Conic, Ellipse2, Meeting};
use geom_num::grid::P2;
use geom_num::interval::Interval;
use geom_num::one_root::{Root, enclose};
use geom_num::{DD, Exactness, GridStep, I256, NumError, Real, Sign, Steps};
use geom_run::{ChunkPlan, Exec, Phase, RunError, Stopped, WorkMeter};

use crate::accept::Accept;
use crate::carrier::{Carriers, Circle2, Curve2, CurveIx, Line2};
use crate::ideal::{self, Aim, Equation, Foot, Pair, Solved, dd_of, dot, minus, norm, over};
use crate::point::{PointError, Pt2};

/// The phase of [`fillet_all`].
const CORNERS: Phase = Phase::new("fillet corners");

/// Corners in one chunk of [`fillet_all`], each some tens of microseconds' work.
const CHUNK: ChunkPlan = ChunkPlan::fixed::<256>();

/// The work of a corner, in units of [`fillet_all`]'s meter.
const CORNER_WORK: u64 = 1;

/// The most bits of a centre's fraction: a weight below a rational point's budget.
const MOST_BITS: i32 = 82;

/// Where a curve is touched: which curve, the side of it the circle's centre lies on at that place,
/// as [`Curve2::side`] says of a point beside it, and a grid point near the place. Of a line's
/// touches the side is all that counts; of a closed curve's, `near` says which.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Touch {
    /// The curve.
    pub curve: CurveIx,
    /// The side the centre lies on.
    pub side: Sign,
    /// A grid point near where it touches.
    pub near: P2,
}

/// An arc between two curves, running counter-clockwise about its circle from `start` to `end`.
#[derive(Clone, Copy, Debug)]
pub struct Fillet {
    /// The carrier it lies on.
    pub circle: CurveIx,
    /// Where it leaves the first curve.
    pub start: Pt2,
    /// Where it reaches the second.
    pub end: Pt2,
    /// How exact it is.
    pub exactness: Exactness,
    /// The largest direction gap at a join, in radians.
    pub gap: Real,
}

/// Where a circle meets a curve it touches: the two ends of the cap past it.
#[derive(Clone, Copy, Debug)]
pub struct Join {
    /// Where an arc running counter-clockwise leaves the curve.
    pub leaving: Pt2,
    /// Where an arc running counter-clockwise reaches the curve.
    pub arriving: Pt2,
}

/// A circle tangent to three curves.
#[derive(Clone, Copy, Debug)]
pub struct TangentCircle {
    /// The carrier it lies on.
    pub circle: CurveIx,
    /// Where it meets each curve.
    pub joins: [Join; 3],
    /// How exact it is.
    pub exactness: Exactness,
    /// The largest direction gap at a join, in radians.
    pub gap: Real,
}

/// A line from a point or a circle to a circle, kept from `start` to `end`.
#[derive(Clone, Copy, Debug)]
pub struct TangentLine {
    /// The carrier it lies on, running from `start` to `end`.
    pub line: CurveIx,
    /// Where it starts.
    pub start: Pt2,
    /// Where it ends.
    pub end: Pt2,
    /// How exact it is.
    pub exactness: Exactness,
    /// The largest direction gap at a join, in radians.
    pub gap: Real,
}

/// A corner of a polyline: the point `at`, between `before` and `after`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Corner {
    /// The point before it.
    pub before: P2,
    /// The corner.
    pub at: P2,
    /// The point after it.
    pub after: P2,
}

/// How a join is named, once its carriers have their places.
#[derive(Clone, Copy, Debug)]
enum Way {
    /// A crossing of a line and the circle, the line first.
    Line(Root),
    /// A crossing of the circle and another, the new circle first.
    Circles(Root),
    /// A root of the circle along an ellipse.
    Conic(u8),
}

impl Way {
    /// The point, `curve` being the curve touched.
    const fn point(self, curve: CurveIx, circle: CurveIx) -> Pt2 {
        match self {
            Way::Line(root) => Pt2::Crossing {
                a: curve,
                b: circle,
                root,
            },
            Way::Circles(root) => Pt2::Crossing {
                a: circle,
                b: curve,
                root,
            },
            Way::Conic(k) => Pt2::ConicRoot {
                a: curve,
                b: circle,
                k,
            },
        }
    }
}

/// What a curve and the new circle have between them.
#[derive(Clone, Copy, Debug)]
struct Meet {
    /// How an arc running counter-clockwise leaves the curve.
    leaving: Way,
    /// How one reaches it.
    arriving: Way,
    /// The largest direction gap there.
    gap: f64,
    /// How far along the curve the join can be from the ideal touch, in steps.
    reach: f64,
    /// The way from the centre to the cap.
    spoke: [Interval; 2],
}

/// A curve to touch, with what a plan needs of it.
#[derive(Clone, Copy, Debug)]
struct Target {
    curve: Curve2,
    side: Sign,
    near: P2,
}

/// A construction's circle and joins, before any carrier has a place.
#[derive(Clone, Copy, Debug)]
struct Plan<const N: usize> {
    circle: Circle2,
    ways: [(Way, Way); N],
    exactness: Exactness,
    gap: Real,
}

/// The carrier at `at`.
fn curve(carriers: &Carriers, at: CurveIx) -> Result<Curve2, PointError> {
    carriers.get(at).copied().ok_or(PointError::NoCarrier(at))
}

/// 1 or −1 for a side of a curve.
fn unit(side: Sign) -> Result<f64, PointError> {
    match side {
        Sign::Positive => Ok(1.0),
        Sign::Negative => Ok(-1.0),
        Sign::Zero => Err(NumError::Degenerate.into()),
    }
}

/// A radius within the budget.
const fn checked_radius(r: i128) -> Result<i128, NumError> {
    if r <= 0 {
        return Err(NumError::Degenerate);
    }
    if r.unsigned_abs() >= 1_u128 << RADIUS_BITS {
        return Err(NumError::BudgetExceeded);
    }
    Ok(r)
}

/// The whole square root of `n`, if it has one.
fn whole_root(n: i128) -> Option<i128> {
    let root = n.unsigned_abs().isqrt();
    let root = i128::try_from(root).ok()?;
    (root.checked_mul(root)? == n).then_some(root)
}

/// The greatest common divisor of two magnitudes.
const fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let Some(r) = a.checked_rem(b) else {
            return a;
        };
        a = b;
        b = r;
    }
    a
}

/// An `i128` as a double-double.
fn dd(v: i128) -> Result<DD, NumError> {
    DD::from_i128(v).ok_or(NumError::BudgetExceeded)
}

/// `v` as a double-double, at the nearest whole number.
fn round(v: DD) -> Result<i128, NumError> {
    Ok(v.round_shifted(0).ok_or(NumError::BudgetExceeded)?.value())
}

/// The exact circle of a fillet of two lines whose normals are both rational: the centre where the
/// offsets a · x + b · y = c ± r · h meet, h² = a² + b², and rr = r²W². `None` if either normal has
/// no whole length.
fn rational_normals(
    lines: [&Line2; 2],
    sides: [Sign; 2],
    r: i128,
) -> Result<Option<Disc>, PointError> {
    let [first, second] = lines;
    let [s, t] = sides;
    let offset = |l: &Line2, side: Sign| -> Result<Option<Line2>, PointError> {
        let ([a, b], c) = l.coefficients();
        let n = a
            .checked_mul(a)
            .zip(b.checked_mul(b))
            .and_then(|(p, q)| p.checked_add(q))
            .ok_or(NumError::BudgetExceeded)?;
        let Some(h) = whole_root(n) else {
            return Ok(None);
        };
        let moved = match side {
            Sign::Positive => r.checked_mul(h),
            Sign::Negative => r.checked_mul(h).and_then(i128::checked_neg),
            Sign::Zero => None,
        };
        let c = moved
            .and_then(|m| c.checked_add(m))
            .ok_or(NumError::Degenerate)?;
        Ok(Some(Carrier::new(a, b, c)?))
    };
    let (Some(p), Some(q)) = (offset(first, s)?, offset(second, t)?) else {
        return Ok(None);
    };
    let centre = p.meet(&q)?;
    let (_, w) = centre.coordinates();
    let scaled = I256::from_i128(r.checked_mul(w).ok_or(NumError::BudgetExceeded)?);
    let rr = scaled
        .checked_mul(&scaled)
        .ok_or(NumError::BudgetExceeded)?;
    Ok(Some(Disc::new(centre, rr)?))
}

/// The aims at `targets`, about `origin`, each touched last at its `feet`.
fn aims_about<const N: usize>(
    targets: &[Target; N],
    feet: [Pair; N],
    origin: P2,
) -> Result<[Aim; N], PointError> {
    let aim = |target: &Target, foot: Pair| -> Result<Aim, PointError> {
        Ok(Aim {
            equation: Equation::about(&target.curve.conic()?, origin)?,
            side: unit(target.side)?,
            loose: matches!(target.curve, Curve2::Ellipse(_)),
            foot,
        })
    };
    let (Some(first), Some(foot)) = (targets.first(), feet.first()) else {
        return Err(NumError::Degenerate.into());
    };
    let mut made = [aim(first, *foot)?; N];
    for (slot, (target, foot)) in made.iter_mut().zip(targets.iter().zip(feet)) {
        *slot = aim(target, foot)?;
    }
    Ok(made)
}

/// A grid point as an offset from `origin`.
fn from_origin(p: P2, origin: P2) -> Result<Pair, NumError> {
    let part = |a: i64, b: i64| a.checked_sub(b).map(DD::from_i64).ok_or(NumError::BudgetExceeded);
    Ok([part(p.x.get(), origin.x.get())?, part(p.y.get(), origin.y.get())?])
}

/// `from`, shifted by the whole part of `by`, as a grid point.
fn shifted(from: P2, by: Pair) -> Result<P2, NumError> {
    let [x, y] = by;
    let part = |at: i64, v: DD| -> Result<i64, NumError> {
        let moved = i128::from(at)
            .checked_add(round(v)?)
            .ok_or(NumError::BudgetExceeded)?;
        i64::try_from(moved).map_err(|_| NumError::BudgetExceeded)
    };
    P2::new(part(from.x.get(), x)?, part(from.y.get(), y)?)
}

/// The floor of log₂ of a positive normal double, from its exponent field.
fn exponent(v: f64) -> Result<i32, NumError> {
    let field = u16::try_from((v.to_bits() >> 52) & 0x7ff).map_err(|_| NumError::BudgetExceeded)?;
    i32::from(field)
        .checked_sub(1_023)
        .ok_or(NumError::BudgetExceeded)
}

/// Two to `e`, for an `e` a normal double holds.
fn two_to(e: i32) -> Result<f64, NumError> {
    let field = e.checked_add(1_023).ok_or(NumError::BudgetExceeded)?;
    let field = u64::try_from(field).map_err(|_| NumError::BudgetExceeded)?;
    Ok(f64::from_bits(field.wrapping_shl(52)))
}

/// The disc of a solve: its centre rounded to bits fine enough for its least depth, and its radius.
fn disc_of_solved<const N: usize>(solved: &Solved<N>, origin: P2) -> Result<Disc, PointError> {
    let least = solved.depths.iter().fold(f64::INFINITY, |least, d| {
        if d.depth.hi() < least { d.depth.hi() } else { least }
    });
    if !least.is_finite() || least <= 0.0 {
        return Err(NumError::Degenerate.into());
    }
    let bits = 4_i32
        .checked_sub(exponent(least)?)
        .ok_or(NumError::BudgetExceeded)?;
    let bits = if bits < 0 { 0 } else { bits };
    if bits > MOST_BITS {
        return Err(NumError::BudgetExceeded.into());
    }
    let scale = two_to(bits)?;
    let weight = 1_i128
        .checked_shl(bits.unsigned_abs())
        .ok_or(NumError::BudgetExceeded)?;
    let part = |at: i64, v: DD| -> Result<i128, NumError> {
        i128::from(at)
            .checked_mul(weight)
            .zip(Some(round(v.mul_f64(scale)?)?))
            .and_then(|(p, q)| p.checked_add(q))
            .ok_or(NumError::BudgetExceeded)
    };
    let [x, y] = solved.centre;
    let centre = Rational2::new(part(origin.x.get(), x)?, part(origin.y.get(), y)?, weight)?;
    let radius = I256::from_i128(round(solved.radius.mul_f64(scale)?)?);
    let rr = radius
        .checked_mul(&radius)
        .ok_or(NumError::BudgetExceeded)?;
    Ok(Disc::new(centre, rr)?)
}

/// The radius of `disc`, certainly.
fn radius_of(disc: &Disc) -> Result<Interval, NumError> {
    let (_, w) = disc.centre().coordinates();
    enclose(&disc.squared_radius())?
        .sqrt()?
        .div(enclose(&I256::from_i128(w))?)
}

/// `v` as an interval.
fn exactly(v: i128) -> Result<Interval, NumError> {
    enclose(&I256::from_i128(v))
}

/// What a line of normal `normal` and `disc`, on `side` of it, have between them.
fn line_meet(normal: [i128; 2], side: Sign, disc: &Disc, gap: Real) -> Result<Meet, PointError> {
    let ([a, b], back) = (normal, Interval::point(-unit(side)?)?);
    let (leaving, arriving) = if side == Sign::Positive {
        (Root::Second, Root::First)
    } else {
        (Root::First, Root::Second)
    };
    Ok(Meet {
        leaving: Way::Line(leaving),
        arriving: Way::Line(arriving),
        gap: gap.get(),
        reach: radius_of(disc)?.mul(Interval::point(gap.get())?)?.hi(),
        spoke: [exactly(a)?.mul(back)?, exactly(b)?.mul(back)?],
    })
}

/// What another circle and `disc` have between them: the cap is on the way to the other's centre, or
/// away from it when `toward` is false.
fn circle_meet(other: &Disc, disc: &Disc, toward: bool) -> Result<Meet, PointError> {
    let gap = disc.gap_to_disc(other)?;
    let (leaving, arriving) = if toward {
        (Root::First, Root::Second)
    } else {
        (Root::Second, Root::First)
    };
    let ([x, y], w) = disc.centre().coordinates();
    let ([u, v], t) = other.centre().coordinates();
    let wide = I256::from_i128;
    let part = |p: i128, q: i128| -> Result<Interval, NumError> {
        let numerator = wide(p)
            .checked_mul(&wide(w))
            .zip(wide(q).checked_mul(&wide(t)))
            .and_then(|(m, n)| m.checked_sub(&n))
            .ok_or(NumError::BudgetExceeded)?;
        enclose(&numerator)
    };
    let (dx, dy) = (part(u, x)?, part(v, y)?);
    let weights = exactly(t)?.mul(exactly(w)?)?;
    let apart = Interval::norm(&[dx, dy])?.div(weights)?;
    let reach = radius_of(disc)?
        .mul(radius_of(other)?)?
        .mul(Interval::point(gap.get())?)?
        .div(apart)?
        .hi();
    let way = Interval::point(if toward { 1.0 } else { -1.0 })?;
    Ok(Meet {
        leaving: Way::Circles(leaving),
        arriving: Way::Circles(arriving),
        gap: gap.get(),
        reach,
        spoke: [dx.mul(way)?, dy.mul(way)?],
    })
}

/// The largest angle with a sine in `sine`, positive and at most `sine.hi()`.
fn angle_of_sine(sine: Interval) -> Result<f64, NumError> {
    let top = larger(sine.hi(), sine.neg().hi());
    let factor = if top < 1.0 / 1_024.0 { 1.000_001 } else { 1.6 };
    Interval::point(top)?.mul(Interval::point(factor)?).map(Interval::hi)
}

/// The larger of two doubles.
fn larger(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// Which two of `angles` are nearest the angle `aim`.
fn nearest_two(angles: &[[Interval; 2]], aim: [f64; 2]) -> (usize, usize) {
    let [ax, ay] = aim;
    let mut best = [(f64::INFINITY, 0_usize); 2];
    for (k, [c, s]) in angles.iter().enumerate() {
        let (dx, dy) = (c.lo() - ax, s.lo() - ay);
        let far = dx * dx + dy * dy;
        let [first, second] = best;
        if far < first.0 {
            best = [(far, k), first];
        } else if far < second.0 {
            best = [first, (far, k)];
        }
    }
    let [(_, i), (_, j)] = best;
    (i, j)
}

/// What an ellipse and `disc`, whose ideal touch is `foot` about `origin`, have between them. The
/// circle and the ellipse cross twice around the touch, and between those two points, a stretch of
/// the ellipse Δs long, h = ‖P − C‖² − r² has a zero of its derivative, so by the mean value theorem
/// its derivative at a crossing, 2r · sin θ, is at most Δs · sup |h″| = 2Δs(1 + r · κ).
fn ellipse_meet(
    ellipse: &Ellipse2,
    disc: &Disc,
    side: Sign,
    foot: &Foot,
    origin: P2,
) -> Result<Meet, PointError> {
    let Meeting::At { roots, .. } = ellipse.meeting(&Conic::of_disc(disc)?)? else {
        return Err(NumError::Degenerate.into());
    };
    let mut angles = [[Interval::point(0.0)?; 2]; 4];
    let mut count = 0_usize;
    for (slot, root) in angles.iter_mut().zip(roots.iter()) {
        *slot = root.angle()?;
        count = count.saturating_add(1);
    }
    let found = angles.get(..count).ok_or(NumError::Degenerate)?;
    let [mx, my] = ellipse.major();
    let (p, q) = ellipse.ratio();
    let (dx, dy) = (DD::from_i64(mx), DD::from_i64(my));
    let m = [dx, dy];
    let length = dot(m, m)?;
    let turned = [dy.negate(), dx];
    let along = minus(foot.at, from_origin(ellipse.centre(), origin)?)?;
    let ratio = DD::from_i64(q).div(DD::from_i64(p))?;
    let aim = [
        dot(along, m)?.div(length)?.hi(),
        dot(along, turned)?.div(length)?.mul(ratio)?.hi(),
    ];
    let (first, second) = nearest_two(found, aim);
    let at = |k: usize| found.get(k).copied().ok_or(NumError::Degenerate);
    let times = |k: usize| {
        roots
            .iter()
            .nth(k)
            .map(geom_num::quartic::RealRoot::multiplicity)
            .ok_or(NumError::Degenerate)
    };
    let name = |k: usize| u8::try_from(k).map_err(|_| NumError::Degenerate);
    let (ax, ay) = (exactly(i128::from(mx))?, exactly(i128::from(my))?);
    let major = Interval::norm(&[ax, ay])?;
    let [cosine, sine] = at(first)?;
    let qp = exactly(i128::from(q))?.div(exactly(i128::from(p))?)?;
    let back = Interval::point(-unit(side)?)?;
    let spoke = [
        cosine.mul(ax)?.sub(sine.mul(qp)?.mul(ay)?)?.mul(back)?,
        cosine.mul(ay)?.add(sine.mul(qp)?.mul(ax)?)?.mul(back)?,
    ];
    if times(first)? > 1 {
        let way = Way::Conic(name(first)?);
        return Ok(Meet {
            leaving: way,
            arriving: way,
            gap: 0.0,
            reach: 0.0,
            spoke,
        });
    }
    let (low, high) = if first < second {
        (first, second)
    } else {
        (second, first)
    };
    let last = count.saturating_sub(1);
    let (earlier, later) = if high == low.saturating_add(1) {
        (low, high)
    } else if low == 0 && high == last && count > 1 {
        (high, low)
    } else {
        return Err(NumError::Degenerate.into());
    };
    let (e, l) = (name(earlier)?, name(later)?);
    let (leaving, arriving) = if side == Sign::Negative { (l, e) } else { (e, l) };
    let [c1, s1] = at(earlier)?;
    let [c2, s2] = at(later)?;
    let swept = angle_of_sine(c1.mul(s2)?.sub(s1.mul(c2)?)?)?;
    if c1.mul(c2)?.add(s1.mul(s2)?)?.lo() <= 0.0 {
        return Err(NumError::Degenerate.into());
    }
    let swept = Interval::point(swept)?;
    let bent = major
        .div(radius_of(disc)?)?
        .add(qp.mul(qp)?)?
        .mul(Interval::point(1.01)?)?;
    Ok(Meet {
        leaving: Way::Conic(leaving),
        arriving: Way::Conic(arriving),
        gap: swept.mul(bent)?.hi(),
        reach: swept.mul(major)?.hi(),
        spoke,
    })
}

/// What `target` and `disc` have between them, the ideal touch being `foot` with `reach`.
fn meet(
    target: &Target,
    disc: &Disc,
    foot: &Foot,
    reach: f64,
    origin: P2,
) -> Result<Meet, PointError> {
    let side = target.side;
    let centre = disc.centre();
    match &target.curve {
        Curve2::Line(l) => {
            if l.side(&centre)? != side {
                return Err(NumError::Degenerate.into());
            }
            let (normal, _) = l.coefficients();
            line_meet(normal, side, disc, disc.gap_to_carrier(l)?)
        }
        Curve2::Fine(l) => {
            if l.side(&centre)? != side {
                return Err(NumError::Degenerate.into());
            }
            let (normal, _) = l.coefficients();
            line_meet(normal, side, disc, disc.gap_to_fine(l)?)
        }
        Curve2::Circle(c) => circle_meet(
            &c.disc()?,
            disc,
            side == Sign::Positive || reach < 0.0,
        ),
        Curve2::Ellipse(e) => ellipse_meet(e, disc, side, foot, origin),
    }
}

/// Whether the ways `u` and `v` are certainly further apart than `angle`, by their sine or cosine.
fn clear(u: [Interval; 2], v: [Interval; 2], angle: f64) -> Result<bool, NumError> {
    let ([ux, uy], [vx, vy]) = (u, v);
    let lengths = Interval::norm(&[ux, uy])?.mul(Interval::norm(&[vx, vy])?)?;
    let cosine = ux.mul(vx)?.add(uy.mul(vy)?)?.div(lengths)?;
    if cosine.hi() <= 0.0 {
        return Ok(true);
    }
    let sine = ux.mul(vy)?.sub(uy.mul(vx)?)?.div(lengths)?;
    Ok(sine.lo() > angle || sine.neg().lo() > angle)
}

/// The class of a result that reached no further than `reach` steps along a curve from its ideal, at
/// `step`.
fn certified(reach: f64, step: GridStep) -> Result<Exactness, NumError> {
    let whole = if reach <= 1.0 {
        1
    } else {
        let up = round(DD::from_f64(reach)?)?
            .checked_add(1)
            .ok_or(NumError::BudgetExceeded)?;
        u64::try_from(up).map_err(|_| NumError::BudgetExceeded)?
    };
    Ok(Exactness::Certified {
        bound: Steps(whole),
        step,
    })
}

/// What the targets of a solve and its disc have between them, each cap proven clear of the others',
/// as a plan.
fn planned<const N: usize>(
    targets: &[Target; N],
    solved: &Solved<N>,
    origin: P2,
    step: GridStep,
    accept: &Accept,
) -> Result<Plan<N>, PointError> {
    let disc = disc_of_solved(solved, origin)?;
    let mut meets = [None; N];
    for (slot, (target, (foot, depth))) in meets
        .iter_mut()
        .zip(targets.iter().zip(solved.feet.iter().zip(&solved.depths)))
    {
        *slot = Some(meet(target, &disc, foot, depth.reach, origin)?);
    }
    let (mut gap, mut reach) = (0.0, 0.0);
    let mut ways = [(Way::Conic(0), Way::Conic(0)); N];
    for (k, slot) in meets.iter().enumerate() {
        let meeting = slot.ok_or(NumError::Degenerate)?;
        gap = larger(gap, meeting.gap);
        reach = larger(reach, meeting.reach);
        if let Some(way) = ways.get_mut(k) {
            *way = (meeting.leaving, meeting.arriving);
        }
        for other in meets.iter().take(k).flatten() {
            let both = Interval::point(meeting.gap)?.add(Interval::point(other.gap)?)?.hi();
            if !clear(meeting.spoke, other.spoke, both)? {
                return Err(NumError::Degenerate.into());
            }
        }
    }
    let gap = Real::new(gap)?;
    let exactness = accept.admit(certified(reach, step)?, gap)?;
    Ok(Plan {
        circle: Circle2::Disc(disc),
        ways,
        exactness,
        gap,
    })
}

/// The plan of a fillet of radius `r` between two curves.
fn fillet_plan(
    targets: &[Target; 2],
    r: i128,
    step: GridStep,
    accept: &Accept,
) -> Result<Plan<2>, PointError> {
    let r = checked_radius(r)?;
    let [first, second] = targets;
    if let (Curve2::Line(l), Curve2::Line(m)) = (&first.curve, &second.curve) {
        if l.parallel(m)? {
            return Err(NumError::Degenerate.into());
        }
        if let Some(disc) = rational_normals([l, m], [first.side, second.side], r)? {
            let way = Way::Line(Root::First);
            return Ok(Plan {
                circle: Circle2::Disc(disc),
                ways: [(way, way), (way, way)],
                exactness: accept.admit(Exactness::Exact, Real::new(0.0)?)?,
                gap: Real::new(0.0)?,
            });
        }
    }
    let radius = dd(r)?;
    let origin = first.near;
    let (seed, other) = (
        from_origin(first.near, origin)?,
        from_origin(second.near, origin)?,
    );
    let mut aims = aims_about(targets, [seed, other], origin)?;
    let ([sx, sy], [ox, oy]) = (seed, other);
    let start = over([sx.add(ox)?, sy.add(oy)?], DD::from_i64(2))?;
    let rough = ideal::with_radius(&mut aims, start, radius)?;
    let centred = shifted(origin, rough.centre)?;
    let moved = from_origin(centred, origin)?;
    let [one, two] = rough.feet;
    let mut aims = aims_about(
        targets,
        [minus(one.at, moved)?, minus(two.at, moved)?],
        centred,
    )?;
    let solved = ideal::with_radius(&mut aims, minus(rough.centre, moved)?, radius)?;
    planned(targets, &solved, centred, step, accept)
}

impl<const N: usize> Plan<N> {
    /// The ways of the `k`th touch, as points.
    fn points(&self, k: usize, curve: CurveIx, circle: CurveIx) -> Result<(Pt2, Pt2), NumError> {
        let (leaving, arriving) = self.ways.get(k).ok_or(NumError::Degenerate)?;
        Ok((leaving.point(curve, circle), arriving.point(curve, circle)))
    }
}

/// The target of `touch`.
fn target(carriers: &Carriers, touch: &Touch) -> Result<Target, PointError> {
    Ok(Target {
        curve: curve(carriers, touch.curve)?,
        side: touch.side,
        near: touch.near,
    })
}

/// The arc of radius `radius` steps from `first` to `second`, running counter-clockwise about its
/// centre. Pass the curves the other way round for the arc that runs the other way.
///
/// # Errors
///
/// [`PointError::Num`] with [`NumError::Degenerate`] where no such arc exists or the two joins are
/// too near each other to prove clear, and with [`NumError::BudgetExceeded`] for a radius or a centre
/// past the budget; [`PointError::NoCarrier`] for a curve the table does not hold;
/// [`PointError::ExactnessNotAccepted`] for a result the caller did not accept.
pub fn fillet(
    carriers: &mut Carriers,
    first: &Touch,
    second: &Touch,
    radius: i128,
    step: GridStep,
    accept: &Accept,
) -> Result<Fillet, PointError> {
    let targets = [target(carriers, first)?, target(carriers, second)?];
    let plan = fillet_plan(&targets, radius, step, accept)?;
    let circle = carriers.push(Curve2::Circle(plan.circle))?;
    let (start, _) = plan.points(0, first.curve, circle)?;
    let (_, end) = plan.points(1, second.curve, circle)?;
    Ok(Fillet {
        circle,
        start,
        end,
        exactness: plan.exactness,
        gap: plan.gap,
    })
}

/// The half circle joining the parallel lines `first` and `second`, tangent to each: it runs
/// counter-clockwise from `at`, a grid point of `first`, about a centre on their midline, and is
/// exact, always. Pass the other line's end for the half running the other way.
///
/// # Errors
///
/// [`PointError::Num`] with [`NumError::Degenerate`] unless both are lines, parallel and not one
/// line, and `at` is on `first`; with [`NumError::BudgetExceeded`] for a centre past the budget even
/// in lowest terms; [`PointError::NoCarrier`] for a curve the table does not hold.
pub fn semicircle(
    carriers: &mut Carriers,
    first: CurveIx,
    second: CurveIx,
    at: P2,
) -> Result<Fillet, PointError> {
    let (Curve2::Line(l), Curve2::Line(m)) = (curve(carriers, first)?, curve(carriers, second)?)
    else {
        return Err(NumError::Degenerate.into());
    };
    if !l.parallel(&m)? || l.same(&m)? || l.side(&at.into())? != Sign::Zero {
        return Err(NumError::Degenerate.into());
    }
    let ([a, b], c) = m.coefficients();
    let (px, py) = (i128::from(at.x.get()), i128::from(at.y.get()));
    let fits = NumError::BudgetExceeded;
    let n = a
        .checked_mul(a)
        .zip(b.checked_mul(b))
        .and_then(|(p, q)| p.checked_add(q))
        .ok_or(fits)?;
    let e = a
        .checked_mul(px)
        .zip(b.checked_mul(py))
        .and_then(|(p, q)| p.checked_add(q))
        .and_then(|sum| c.checked_sub(sum))
        .ok_or(fits)?;
    let w = n.checked_mul(2).ok_or(fits)?;
    let part = |p: i128, normal: i128| -> Result<i128, NumError> {
        w.checked_mul(p)
            .zip(normal.checked_mul(e))
            .and_then(|(s, t)| s.checked_add(t))
            .ok_or(fits)
    };
    let (x, y) = (part(px, a)?, part(py, b)?);
    let common = gcd(gcd(x.unsigned_abs(), y.unsigned_abs()), w.unsigned_abs());
    let common = i128::try_from(common).map_err(|_| fits)?;
    let lowest = |v: i128| v.checked_div(common).ok_or(fits);
    let centre = Rational2::new(lowest(x)?, lowest(y)?, lowest(w)?)?;
    let circle = carriers.push(Curve2::Circle(Circle2::Round(Round::new(at, centre))))?;
    Ok(Fillet {
        circle,
        start: Pt2::Grid(at),
        end: Pt2::Crossing {
            a: second,
            b: circle,
            root: Root::First,
        },
        exactness: Exactness::Exact,
        gap: Real::new(0.0)?,
    })
}

/// The circle touching all three curves, on the given sides of each and near the given points.
///
/// # Errors
///
/// As for [`fillet`].
pub fn tangent_circle(
    carriers: &mut Carriers,
    touches: &[Touch; 3],
    step: GridStep,
    accept: &Accept,
) -> Result<TangentCircle, PointError> {
    let [a, b, c] = touches;
    let targets = [
        target(carriers, a)?,
        target(carriers, b)?,
        target(carriers, c)?,
    ];
    let origin = a.near;
    let seeds = [
        from_origin(a.near, origin)?,
        from_origin(b.near, origin)?,
        from_origin(c.near, origin)?,
    ];
    let [[px, py], [qx, qy], [rx, ry]] = seeds;
    let start = over(
        [px.add(qx)?.add(rx)?, py.add(qy)?.add(ry)?],
        DD::from_i64(3),
    )?;
    let radius = larger_norm(&seeds, start)?;
    let mut aims = aims_about(&targets, seeds, origin)?;
    let rough = ideal::of_three(&mut aims, start, radius)?;
    let centred = shifted(origin, rough.centre)?;
    let moved = from_origin(centred, origin)?;
    let [one, two, three] = rough.feet;
    let mut aims = aims_about(
        &targets,
        [
            minus(one.at, moved)?,
            minus(two.at, moved)?,
            minus(three.at, moved)?,
        ],
        centred,
    )?;
    let solved = ideal::of_three(&mut aims, minus(rough.centre, moved)?, rough.radius)?;
    let plan = planned(&targets, &solved, centred, step, accept)?;
    let circle = carriers.push(Curve2::Circle(plan.circle))?;
    let join = |k: usize, curve: CurveIx| -> Result<Join, NumError> {
        let (leaving, arriving) = plan.points(k, curve, circle)?;
        Ok(Join { leaving, arriving })
    };
    Ok(TangentCircle {
        circle,
        joins: [join(0, a.curve)?, join(1, b.curve)?, join(2, c.curve)?],
        exactness: plan.exactness,
        gap: plan.gap,
    })
}

/// The farthest of `seeds` from `from`, as a first guess at a radius.
fn larger_norm(seeds: &[Pair; 3], from: Pair) -> Result<DD, NumError> {
    let mut far = DD::from_i64(1);
    for seed in seeds {
        let d = norm(minus(*seed, from)?)?;
        if far < d {
            far = d;
        }
    }
    Ok(far)
}

/// The simplest fraction strictly between a / b and c / d, 0 ≤ a / b < c / d, as numerator and
/// denominator: the last term of their common continued fraction, one more, or the reciprocal step
/// when the lower end is whole.
fn simplest(ends: [i128; 4]) -> Option<[i128; 2]> {
    let [mut a, mut b, mut c, mut d] = ends;
    let (mut p, mut q) = ((1_i128, 0_i128), (0_i128, 1_i128));
    let term = |t: i128, p: (i128, i128), q: (i128, i128)| {
        let next = t.checked_mul(p.0)?.checked_add(p.1)?;
        let other = t.checked_mul(q.0)?.checked_add(q.1)?;
        Some(((next, p.0), (other, q.0)))
    };
    // Euclid halves b or d every two terms.
    for _ in 0..400 {
        let f = a.checked_div(b)?;
        let up = f.checked_add(1)?;
        if up.checked_mul(d)? < c {
            let ((n, _), (m, _)) = term(up, p, q)?;
            return Some([n, m]);
        }
        let low = a.checked_sub(f.checked_mul(b)?)?;
        let high = c.checked_sub(f.checked_mul(d)?)?;
        (p, q) = term(f, p, q)?;
        if low == 0 {
            let last = d.checked_div(high)?.checked_add(1)?;
            let ((n, _), (m, _)) = term(last, p, q)?;
            return Some([n, m]);
        }
        (a, b, c, d) = (d, high, b, low);
    }
    None
}

/// `v` in units of 2^−125: in two rounds, as a double-double rounds only below 2^100.
fn fixed(v: DD) -> Result<i128, NumError> {
    let (coarse_scale, fine_scale) = (two_to(60)?, two_to(125)?);
    let coarse = round(v.mul_f64(coarse_scale)?)?;
    let rest = v.sub(dd(coarse)?.mul_f64(two_to(-60)?)?)?;
    let fine = round(rest.mul_f64(fine_scale)?)?;
    coarse
        .checked_shl(65)
        .and_then(|high| high.checked_add(fine))
        .ok_or(NumError::BudgetExceeded)
}

/// The simplest whole direction strictly between the directions `low` and `high`, a hair apart.
fn between(low: Pair, high: Pair) -> Result<[i128; 2], PointError> {
    let ([lx, ly], [hx, hy]) = (low, high);
    let flat = ly.abs() < lx.abs();
    let (major, minor, far_major, far_minor) = if flat {
        (lx, ly, hx, hy)
    } else {
        (ly, lx, hy, hx)
    };
    let (mut first, mut second) = (minor.div(major)?, far_minor.div(far_major)?);
    if second < first {
        (first, second) = (second, first);
    }
    let across = first.sign() != second.sign();
    let negative = second.sign() != Sign::Positive;
    let (near, far) = if negative {
        (second.negate(), first.negate())
    } else {
        (first, second)
    };
    let [p, q] = if across {
        [0, 1]
    } else {
        let one = 1_i128 << 125;
        let (n, m) = (fixed(near)?, fixed(far)?);
        if m <= n {
            return Err(NumError::Degenerate.into());
        }
        simplest([n, one, m, one]).ok_or(NumError::BudgetExceeded)?
    };
    let turned = |v: i128, on: bool| if on { v.checked_neg() } else { Some(v) };
    let way = major.sign() == Sign::Negative;
    let part_major = turned(q, way).ok_or(NumError::BudgetExceeded)?;
    let part_minor = turned(p, negative != way).ok_or(NumError::BudgetExceeded)?;
    Ok(if flat {
        [part_major, part_minor]
    } else {
        [part_minor, part_major]
    })
}

/// The way a line runs whose left normal n has n · `delta` = `offset`, with `delta` ahead of it.
fn way(delta: Pair, offset: DD) -> Result<Pair, PointError> {
    let length = norm(delta)?;
    let along = offset.div(length)?;
    let left = DD::from_i64(1).sub(along.mul(along)?)?;
    if left.sign() != Sign::Positive {
        return Err(NumError::Degenerate.into());
    }
    let [ux, uy] = over(delta, length)?;
    let across = left.sqrt()?;
    let [nx, ny] = [
        along.mul(ux)?.sub(across.mul(uy)?)?,
        along.mul(uy)?.add(across.mul(ux)?)?,
    ];
    Ok([ny, nx.negate()])
}

/// The line a · x + b · y = c, as the narrower form it fits.
fn line(a: i128, b: i128, c: i128) -> Result<Curve2, NumError> {
    match Carrier::new(a, b, c) {
        Ok(l) => Ok(Curve2::Line(l)),
        Err(NumError::BudgetExceeded) => Ok(Curve2::Fine(FineLine::new(a, b, c)?)),
        Err(e) => Err(e),
    }
}

/// The largest direction gap between `line` and `disc`, whose centre must be on `side`.
fn line_gap(line: &Curve2, disc: &Disc, side: Sign) -> Result<Real, PointError> {
    if line.side(&disc.centre())? != side {
        return Err(NumError::Degenerate.into());
    }
    match line {
        Curve2::Line(l) => Ok(disc.gap_to_carrier(l)?),
        Curve2::Fine(l) => Ok(disc.gap_to_fine(l)?),
        Curve2::Circle(_) | Curve2::Ellipse(_) => Err(PointError::WrongClass),
    }
}

/// The circle at `at`.
fn disc_at(carriers: &Carriers, at: CurveIx) -> Result<Disc, PointError> {
    match curve(carriers, at)? {
        Curve2::Circle(c) => Ok(c.disc()?),
        Curve2::Line(_) | Curve2::Fine(_) | Curve2::Ellipse(_) => Err(PointError::WrongClass),
    }
}

/// `disc`'s radius and its centre about `origin`, as double-doubles.
fn ideal_of(disc: &Disc, origin: P2) -> Result<(DD, Pair), NumError> {
    let ([x, y], w) = disc.centre().coordinates();
    let weight = dd(w)?;
    let part = |c: i128, at: i64| -> Result<DD, NumError> {
        let wide = I256::from_i128;
        let numerator = wide(i128::from(at))
            .checked_mul(&wide(w))
            .and_then(|p| wide(c).checked_sub(&p))
            .ok_or(NumError::BudgetExceeded)?;
        dd_of(&numerator)?.div(weight)
    };
    Ok((
        dd_of(&disc.squared_radius())?.sqrt()?.div(weight)?,
        [part(x, origin.x.get())?, part(y, origin.y.get())?],
    ))
}

/// The class of a tangent line's result, with its gap.
fn line_class(
    gaps: &[Real],
    disc: &Disc,
    step: GridStep,
    accept: &Accept,
) -> Result<(Exactness, Real), PointError> {
    let gap = gaps.iter().fold(0.0, |most, g| larger(most, g.get()));
    let reach = radius_of(disc)?.mul(Interval::point(gap)?)?.hi();
    let gap = Real::new(gap)?;
    Ok((accept.admit(certified(reach, step)?, gap)?, gap))
}

/// The line from the grid point `from` to the circle at `circle`, with the centre on `side` of the
/// way it runs. It goes a hair into the circle and ends at the crossing past the touch, so an arc
/// going on the way the line was going meets nothing again. A point on the circle has the exact
/// tangent there.
///
/// # Errors
///
/// [`PointError::Num`] with [`NumError::Degenerate`] for a point inside the circle;
/// [`PointError::WrongClass`] unless `circle` is a circle; otherwise as for [`fillet`].
pub fn tangent_from(
    carriers: &mut Carriers,
    from: P2,
    circle: CurveIx,
    side: Sign,
    step: GridStep,
    accept: &Accept,
) -> Result<TangentLine, PointError> {
    let disc = disc_at(carriers, circle)?;
    let s = unit(side)?;
    let (px, py) = (i128::from(from.x.get()), i128::from(from.y.get()));
    let through = |a: i128, b: i128| -> Result<Curve2, NumError> {
        let c = a
            .checked_mul(px)
            .zip(b.checked_mul(py))
            .and_then(|(p, q)| p.checked_add(q))
            .ok_or(NumError::BudgetExceeded)?;
        line(a, b, c)
    };
    let finished = |carriers: &mut Carriers, made: Curve2, root, exactness, gap| {
        let at = carriers.push(made)?;
        Ok(TangentLine {
            line: at,
            start: Pt2::Grid(from),
            end: Pt2::Crossing {
                a: at,
                b: circle,
                root,
            },
            exactness,
            gap,
        })
    };
    match disc.side(&from.into())? {
        Sign::Negative => Err(NumError::Degenerate.into()),
        Sign::Zero => {
            let ([x, y], w) = disc.centre().coordinates();
            let fits = NumError::BudgetExceeded;
            let across = |c: i128, at: i128| {
                w.checked_mul(at).and_then(|p| c.checked_sub(p)).ok_or(fits)
            };
            let (vx, vy) = (across(x, px)?, across(y, py)?);
            let common = i128::try_from(gcd(vx.unsigned_abs(), vy.unsigned_abs()))
                .map_err(|_| fits)?;
            let lowest = |v: i128| v.checked_div(common).ok_or(fits);
            // The way is the radius turned, clockwise for a centre on the left.
            let (a, b) = (lowest(vx)?, lowest(vy)?);
            let made = if side == Sign::Positive {
                through(a, b)?
            } else {
                through(
                    a.checked_neg().ok_or(fits)?,
                    b.checked_neg().ok_or(fits)?,
                )?
            };
            let gap = Real::new(0.0)?;
            let exactness = accept.admit(Exactness::Exact, gap)?;
            finished(carriers, made, Root::First, exactness, gap)
        }
        Sign::Positive => {
            let (r, delta) = ideal_of(&disc, from)?;
            let depth = ideal::plain_depth(r)?;
            let aim = |share: f64| -> Result<Pair, PointError> {
                way(delta, r.sub(depth.mul_f64(share)?)?.mul_f64(s)?)
            };
            let [dx, dy] = between(aim(0.5)?, aim(1.5)?)?;
            let made = through(dy.checked_neg().ok_or(NumError::BudgetExceeded)?, dx)?;
            let gap = line_gap(&made, &disc, side)?;
            let (exactness, gap) = line_class(&[gap], &disc, step, accept)?;
            finished(carriers, made, Root::Second, exactness, gap)
        }
        }
}

/// The line from the circle at `first` to the circle at `second`, each centre on its `sides` of the
/// way it runs: the same side for an outer tangent, opposite sides for an inner one. It goes a hair
/// into each, starting at the crossing before the first touch and ending at the crossing past the
/// second, so arcs of those circles coming on to it and going on from it meet nothing again.
///
/// # Errors
///
/// [`PointError::Num`] with [`NumError::Degenerate`] where no such tangent exists, as when one circle
/// is inside the other; [`PointError::WrongClass`] unless both are circles; otherwise as for
/// [`fillet`].
pub fn common_tangent(
    carriers: &mut Carriers,
    first: CurveIx,
    second: CurveIx,
    sides: [Sign; 2],
    step: GridStep,
    accept: &Accept,
) -> Result<TangentLine, PointError> {
    let (one, two) = (disc_at(carriers, first)?, disc_at(carriers, second)?);
    let [left, right] = sides;
    let (s, t) = (unit(left)?, unit(right)?);
    let ([x, y], w) = one.centre().coordinates();
    let whole = |v: i128| i64::try_from(v.div_euclid(w)).map_err(|_| NumError::BudgetExceeded);
    let origin = P2::new(whole(x)?, whole(y)?)?;
    let (near, here) = ideal_of(&one, origin)?;
    let (far, there) = ideal_of(&two, origin)?;
    let delta = minus(there, here)?;
    let (reach, depth) = (ideal::plain_depth(near)?, ideal::plain_depth(far)?);
    let start = near.sub(reach)?.mul_f64(s)?;
    let aim = |share: f64| -> Result<Pair, PointError> {
        way(delta, far.sub(depth.mul_f64(share)?)?.mul_f64(t)?.sub(start)?)
    };
    let [dx, dy] = between(aim(0.5)?, aim(1.5)?)?;
    let length = norm([dd(dx)?, dd(dy)?])?;
    // Fine enough that a whole c places the line within an eighth of the lesser depth.
    let least = if reach < depth { reach } else { depth };
    let wanted = DD::from_i64(8).div(least)?.div(length)?;
    let bits = exponent(wanted.hi())?.saturating_add(1);
    let bits = if bits < 0 { 0 } else { bits.unsigned_abs() };
    let fits = NumError::BudgetExceeded;
    let (dx, dy) = (
        dx.checked_shl(bits).ok_or(fits)?,
        dy.checked_shl(bits).ok_or(fits)?,
    );
    let (a, b) = (dy.checked_neg().ok_or(fits)?, dx);
    let normal = [dd(a)?, dd(b)?];
    let offset = dot(normal, here)?.sub(start.mul(norm(normal)?)?)?;
    let c = a
        .checked_mul(i128::from(origin.x.get()))
        .zip(b.checked_mul(i128::from(origin.y.get())))
        .and_then(|(p, q)| p.checked_add(q))
        .zip(Some(round(offset)?))
        .and_then(|(p, q)| p.checked_add(q))
        .ok_or(fits)?;
    let made = line(a, b, c)?;
    let gaps = [line_gap(&made, &one, left)?, line_gap(&made, &two, right)?];
    let wider = if near < far { two } else { one };
    let (exactness, gap) = line_class(&gaps, &wider, step, accept)?;
    let at = carriers.push(made)?;
    Ok(TangentLine {
        line: at,
        start: Pt2::Crossing {
            a: at,
            b: first,
            root: Root::First,
        },
        end: Pt2::Crossing {
            a: at,
            b: second,
            root: Root::Second,
        },
        exactness,
        gap,
    })
}

/// What a corner of [`fillet_all`] comes to, before its carriers have places.
#[derive(Clone, Copy, Debug)]
struct Filleted {
    lines: [Line2; 2],
    plan: Plan<2>,
    left: bool,
}

/// Why a corner gave no plan.
#[derive(Clone, Copy, Debug)]
enum Failed {
    Point(PointError),
    Stopped(Stopped),
}

impl From<PointError> for Failed {
    fn from(e: PointError) -> Failed {
        Failed::Point(e)
    }
}

impl From<NumError> for Failed {
    fn from(e: NumError) -> Failed {
        Failed::Point(e.into())
    }
}

/// The plan of one corner: a turn to the left has its centre on the left of both lines and its arc
/// running from the first to the second, and a turn to the right the other way about. No turn, no
/// fillet.
fn corner_plan(
    corner: &Corner,
    r: i128,
    step: GridStep,
    accept: &Accept,
) -> Result<Option<Filleted>, PointError> {
    let (first, second) = (
        Line2::through(corner.before, corner.at)?,
        Line2::through(corner.at, corner.after)?,
    );
    let turn = first.side(&corner.after.into())?;
    if turn == Sign::Zero {
        return Ok(None);
    }
    let left = turn == Sign::Positive;
    let aim = |l: Line2| Target {
        curve: Curve2::Line(l),
        side: turn,
        near: corner.at,
    };
    let targets = if left {
        [aim(first), aim(second)]
    } else {
        [aim(second), aim(first)]
    };
    let plan = fillet_plan(&targets, r, step, accept)?;
    Ok(Some(Filleted {
        lines: [first, second],
        plan,
        left,
    }))
}

/// A fillet of radius `radius` steps at each corner, the same way along the polyline as it turns: the
/// corners that turn nowhere have `None`. Each corner adds its two lines and its circle to the table,
/// in corner order, and a fillet's `start` is on its first line if it turns left and its second if
/// right, as [`fillet`] runs. A radius too large for a corner's segments is not this function's to
/// see: an arc is only ever as long as its own two lines say.
///
/// # Errors
///
/// [`RunError::Failed`] with the error of the lowest corner that failed, as [`fillet`]'s; a stop of
/// `meter`, as [`RunError`] says. Nothing is added to the table.
pub fn fillet_all(
    carriers: &mut Carriers,
    corners: &[Corner],
    radius: i128,
    step: GridStep,
    accept: &Accept,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Option<Fillet>>, RunError<PointError>> {
    let plans = exec.try_map(
        meter,
        CORNERS,
        CHUNK,
        corners,
        |_, corner, meter| -> Result<Option<Filleted>, Failed> {
            meter.charge(CORNER_WORK).map_err(Failed::Stopped)?;
            Ok(corner_plan(corner, radius, step, accept)?)
        },
    );
    let plans = plans.map_err(|e| match e {
        RunError::Failed(Failed::Point(e)) => RunError::Failed(e),
        RunError::Failed(Failed::Stopped(s)) => meter.error(s, CORNERS),
        RunError::BudgetExceeded(e) => RunError::BudgetExceeded(e),
        RunError::Cancelled => RunError::Cancelled,
        RunError::OutOfMemory => RunError::OutOfMemory,
        RunError::Internal(what) => RunError::Internal(what),
    })?;
    let mut table = carriers.clone();
    let mut out = Vec::new();
    out.try_reserve_exact(plans.len())
        .map_err(|_| RunError::OutOfMemory)?;
    for planned in &plans {
        let done = planned
            .as_ref()
            .map(|f| name_corner(&mut table, f))
            .transpose()
            .map_err(RunError::Failed)?;
        out.push(done);
    }
    *carriers = table;
    Ok(out)
}

/// Gives a corner's carriers their places.
fn name_corner(table: &mut Carriers, corner: &Filleted) -> Result<Fillet, PointError> {
    let [first, second] = corner.lines;
    let (first, second) = (
        table.push(Curve2::Line(first))?,
        table.push(Curve2::Line(second))?,
    );
    let circle = table.push(Curve2::Circle(corner.plan.circle))?;
    let (from, to) = if corner.left {
        (first, second)
    } else {
        (second, first)
    };
    let (start, _) = corner.plan.points(0, from, circle)?;
    let (_, end) = corner.plan.points(1, to, circle)?;
    Ok(Fillet {
        circle,
        start,
        end,
        exactness: corner.plan.exactness,
        gap: corner.plan.gap,
    })
}
