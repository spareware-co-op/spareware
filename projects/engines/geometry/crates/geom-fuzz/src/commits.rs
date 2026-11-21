//! Fuzz target: `geom-poly`'s plane commit (G142), on any bytes, holds its edge exactly, stays in
//! budget, proves a deviation no corner beats, maps a class exactly under the grid's symmetries,
//! refuses a flip exactly when the ideal sign is certain and differs, and refuses bad doubles by
//! type.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: an edge, a tilt and a direction
//! of three doubles each, a point, a whole number of steps, an extent of up to [`MAX_CORNERS`]
//! corners, one of the grid's 48 signed permutations with a shift, a probe point and an interval.
//! Its grid points are on chapter 28.5's hard cases (small, on coarse sub-grids, one step off them,
//! at the grid's edge), and its doubles are fractions below one, whole numbers, the sines of common
//! slopes, any bits at all and the hostile values (NaN, the infinities, the zeros, the smallest
//! subnormal, 1e±300, the largest double); a tilt may be the edge itself, scaled by a power of two.
//! [`check`] commits the plane through the edge ([`through_edge`]), the free plane
//! ([`moved_along`]) and the plane of each class ([`PlaneClass::holding`], [`PlaneClass::along`])
//! and its image, and holds each to the laws below. [`harness`] is `decode` then `check`, the
//! function every fuzzing engine drives (the crate README sets out the layout).
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, and [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case:
//!   - a plane through an edge that does not hold both its ends exactly, by big integers;
//!   - a committed plane past the plane budget (a normal component of 2^83 or more, an offset of
//!     2^125 or more);
//!   - a deviation that is not a finite number of steps, or that the exact distance between the
//!     committed plane and the ideal one beats at a corner of the extent, decided in big integers
//!     with no rounding at all;
//!   - a class whose image's normal is not its normal under the signed permutation, written here
//!     from the transform's matrix, or whose image's plane is not the plane's image exactly;
//!   - a flip refused when the ideal sign is not certain or agrees, or let through when it is
//!     certain and differs; an ideal reading whose interval does not hold the exact reading;
//!   - a refusal that is not typed as its input asks: a double that is NaN or infinite is
//!     `NotFinite`; a direction of zeros, an edge of no length, a tilt along the edge, or no extent
//!     for a plane through its own point, is `Degenerate`; anything else is `Degenerate`,
//!     `NotFinite`, `BudgetExceeded` (a move past the plane budget) or `DivisorContainsZero`
//!     (chapter 22's `Undecided`: an ideal normal too short for its enclosure to be told from
//!     zero), and a commit is never refused as a flip.
//!
//! Method: chapter 13.2's "Committed planes" and decision 32.21
//! (`projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`), held to an oracle
//! written from scratch in `num-bigint`'s integers, sharing no code with `geom-num`. Every double is
//! an integer times a power of two, so an ideal normal is an integer vector `u` (a direction scaled,
//! or the edge crossed with the tilt scaled) and a committed plane's normal `n` and offset `d` are
//! integers already. At a corner `p` the committed plane reads (n·p + d)/|n| and the ideal one
//! u·(p − q)/|u| − k, for the class's point `q` and move `k`, and the deviation `e` must hold
//! |(n·p + d)/|n| − u·(p − q)/|u| + k| ≤ e. Each side of that is the sign of a sum of three terms
//! a√α + b√β + c√(αβ) with integer a, b, c, α and β, settled exactly by squaring: two terms of
//! opposite signs compare by their squares, and the square of the first two against the third is
//! again two terms, rational and one root.
//!
//! Cost: a dozen commits on each case, and two exact sign decisions for each corner of each, on
//! integers of at most a few thousand bits (a double's exponent spread); at most
//! [`MAX_INPUT_BYTES`] bytes read.

// The oracle's names are its formulas': n and d a committed plane, u an ideal normal, p and q points,
// k a move, and s a power of two.
#![allow(
    clippy::many_single_char_names,
    reason = "the names of the formulas the oracle writes out"
)]

use core::cmp::Ordering;
use core::f64::consts::FRAC_1_SQRT_2;

use geom_math::{Axis, GridTransform};
use geom_num::grid::COORD_MAX;
use geom_num::interval::Interval;
use geom_num::{NumError, P3, Sign};
use geom_poly::commit::{
    CommitError, Committed, Ideal, PlaneClass, check_flip, moved_along, through_edge,
};
use geom_test::{ByteSource, Entropy};
use num_bigint::BigInt;

use crate::Finding;
use crate::adversarial::ORIGIN;

/// The most corners an extent holds, besides the edge's two ends.
pub const MAX_CORNERS: usize = 4;

/// The bytes one value reads: a rule and a word.
const VALUE_BYTES: usize = 1 + 8;

/// The bytes one grid point reads.
const POINT_BYTES: usize = 3 * VALUE_BYTES;

/// The most bytes [`decode`] reads: the mode, the edge's two ends and the second's rule, the tilt,
/// the direction, the point, the move, the extent's count and corners, the symmetry and its shift,
/// the probe and the interval's two ends.
pub const MAX_INPUT_BYTES: usize = 1
    + POINT_BYTES
    + 1
    + POINT_BYTES
    + 2 * POINT_BYTES
    + POINT_BYTES
    + VALUE_BYTES
    + 1
    + MAX_CORNERS * POINT_BYTES
    + 1
    + POINT_BYTES
    + POINT_BYTES
    + 2 * VALUE_BYTES;

/// One case.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// The edge's first end, and the point of a plane through it.
    pub a: P3,
    /// The edge's second end.
    pub b: P3,
    /// The direction a plane through the edge holds.
    pub tilt: [f64; 3],
    /// The normal direction of a free plane.
    pub direction: [f64; 3],
    /// The point a free plane, and a class's plane, is moved from.
    pub point: P3,
    /// How many whole steps it is moved along its normal.
    pub distance: i64,
    /// The corners a commit proves its deviation over.
    pub extent: Vec<P3>,
    /// A symmetry of the grid, for a class's image.
    pub symmetry: Option<GridTransform>,
    /// A point the ideal planes are read at.
    pub probe: P3,
    /// An interval for a flip check, if its two ends make one.
    pub interval: Option<Interval>,
}

/// A coordinate through its rule byte: small, on a sub-grid of 1 mm, one step off it, at the grid's
/// edge, or anything.
fn coordinate(src: &mut ByteSource<'_>) -> i64 {
    let rule = src.byte();
    let any = i64::from_le_bytes(src.word().to_le_bytes()) % (1 << 40);
    let fine = (any % 1024).saturating_mul(6_400);
    match rule % 6 {
        0 => any % 8,
        1 => fine,
        2 => fine.saturating_add(1),
        3 => COORD_MAX.saturating_sub((any % 8).saturating_abs()),
        4 => (any % 8).saturating_abs().saturating_sub(COORD_MAX),
        _ => any,
    }
}

/// A grid point; every coordinate is within the range, so the origin stands only for a defect in
/// [`coordinate`].
fn point(src: &mut ByteSource<'_>) -> P3 {
    let (x, y, z) = (coordinate(src), coordinate(src), coordinate(src));
    P3::new(x, y, z).unwrap_or(ORIGIN)
}

/// The sines and cosines of common slopes: 30°, 45°, 15°, 75°, 1.5° (a draft) and 88.5°.
const SLOPES: [f64; 7] = [
    0.5,
    0.866_025_403_784_438_6,
    FRAC_1_SQRT_2,
    0.258_819_045_102_520_74,
    0.965_925_826_289_068_3,
    0.026_176_948_307_873_15,
    0.999_657_324_975_557_4,
];

/// The hostile doubles.
const HOSTILE: [f64; 10] = [
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    0.0,
    -0.0,
    5e-324,
    1e-300,
    1e300,
    f64::MAX,
    f64::MIN_POSITIVE,
];

/// A double through its rule byte: a fraction below one, a small whole number, a slope's sine, any
/// bits at all, or a hostile value.
#[allow(
    clippy::cast_precision_loss,
    reason = "a whole number below 2^53, and one below 8, are exact doubles"
)]
fn double(src: &mut ByteSource<'_>) -> f64 {
    let rule = src.byte();
    let word = src.word();
    let negative = rule & 0x80 != 0;
    let pick = |list: &[f64]| {
        let i = usize::try_from(word % 64).unwrap_or(0);
        i.checked_rem(list.len())
            .and_then(|i| list.get(i))
            .copied()
            .unwrap_or(0.0)
    };
    let value = match rule % 10 {
        1 => (word % 8) as f64,
        2 => pick(&SLOPES),
        3 => f64::from_bits(word),
        4 => pick(&HOSTILE),
        _ => (word >> 11) as f64 / (1u64 << 53) as f64,
    };
    if negative { -value } else { value }
}

/// Three doubles.
fn doubles(src: &mut ByteSource<'_>) -> [f64; 3] {
    [double(src), double(src), double(src)]
}

/// A move through its rule byte: small, within the grid, past any budget, none, or moderate.
fn distance(src: &mut ByteSource<'_>) -> i64 {
    let rule = src.byte();
    let any = i64::from_le_bytes(src.word().to_le_bytes());
    match rule % 6 {
        0 => any % 8,
        1 => any % (1 << 40),
        2 => {
            if any < 0 {
                i64::MIN
            } else {
                i64::MAX
            }
        }
        3 => 0,
        _ => any % (1 << 20),
    }
}

/// The six orders of the axes.
const ORDERS: [[Axis; 3]; 6] = [
    [Axis::X, Axis::Y, Axis::Z],
    [Axis::X, Axis::Z, Axis::Y],
    [Axis::Y, Axis::X, Axis::Z],
    [Axis::Y, Axis::Z, Axis::X],
    [Axis::Z, Axis::X, Axis::Y],
    [Axis::Z, Axis::Y, Axis::X],
];

/// One of the 48 signed permutations, with a shift, or without one if the shift is past the
/// transforms' range.
fn symmetry(src: &mut ByteSource<'_>) -> Option<GridTransform> {
    let index = src.byte() % 48;
    let order = ORDERS.get(usize::from(index % 6)).copied()?;
    let signs = index / 6;
    let negate = [signs & 1 != 0, signs & 2 != 0, signs & 4 != 0];
    let shift = [coordinate(src), coordinate(src), coordinate(src)];
    GridTransform::from_parts(order, negate, shift)
        .or_else(|_| GridTransform::from_parts(order, negate, [0; 3]))
        .ok()
}

/// `a`'s coordinates moved by `delta`'s, or `a` if that leaves the grid.
fn offset(a: P3, delta: P3) -> P3 {
    let x = a.x.get().saturating_add(delta.x.get() % 1024);
    let y = a.y.get().saturating_add(delta.y.get() % 1024);
    let z = a.z.get().saturating_add(delta.z.get() % 1024);
    P3::new(x, y, z).unwrap_or(a)
}

/// The doubles of an integer edge, scaled by 2^`shift` and signed.
#[allow(
    clippy::cast_precision_loss,
    reason = "an edge's components are below 2^41, which a double holds exactly"
)]
fn along_edge(a: P3, b: P3, shift: u8, negative: bool) -> [f64; 3] {
    let scale = f64::from(1u32 << (shift % 16)) * if negative { -1.0 } else { 1.0 };
    let one = |p: i64, q: i64| q.saturating_sub(p) as f64 * scale;
    [
        one(a.x.get(), b.x.get()),
        one(a.y.get(), b.y.get()),
        one(a.z.get(), b.z.get()),
    ]
}

/// The case `data` decodes to, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let mode = src.byte();
    let a = point(&mut src);
    let rule = src.byte();
    let other = point(&mut src);
    let b = match rule % 4 {
        0 => a,
        1 => offset(a, other),
        _ => other,
    };
    let mut tilt = doubles(&mut src);
    if mode % 4 == 0 {
        tilt = along_edge(a, b, mode >> 4, mode & 2 != 0);
    }
    let direction = doubles(&mut src);
    let at = point(&mut src);
    let distance = distance(&mut src);
    let count = src.byte();
    let mut extent = Vec::with_capacity(MAX_CORNERS + 2);
    if count & 8 != 0 {
        extent.extend([a, b]);
    }
    for i in 0..MAX_CORNERS {
        let corner = point(&mut src);
        if i < usize::from(count % 5) {
            extent.push(corner);
        }
    }
    let symmetry = symmetry(&mut src);
    let probe = point(&mut src);
    let (lo, hi) = (double(&mut src), double(&mut src));
    let interval = Interval::new(lo, hi).ok();
    let read = data.len().saturating_sub(src.remaining());
    let case = Case {
        a,
        b,
        tilt,
        direction,
        point: at,
        distance,
        extent,
        symmetry,
        probe,
        interval,
    };
    (case, read)
}

/// `decode` then `check`.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

// The oracle: big integers, which grow instead of overflowing, so every step below is exact.

/// `x` as an integer times a power of two, for a finite `x`.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "a big integer's negation is exact"
)]
fn dyadic(x: f64) -> (BigInt, i32) {
    let bits = x.to_bits();
    let field = i32::try_from((bits >> 52) & 0x7ff).unwrap_or(0);
    let fraction = bits & ((1 << 52) - 1);
    let (mantissa, exponent) = if field == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), field.saturating_sub(1075))
    };
    let m = BigInt::from(mantissa);
    (if bits >> 63 == 1 { -m } else { m }, exponent)
}

/// A finite direction as integers, the same direction scaled by a power of two; `None` if a
/// component is NaN or infinite.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "big integers grow instead of overflowing; an exponent difference is below 2^12"
)]
fn exact_direction(v: [f64; 3]) -> Option<[BigInt; 3]> {
    if v.iter().any(|x| !x.is_finite()) {
        return None;
    }
    let parts = v.map(dyadic);
    let lowest = parts
        .iter()
        .filter(|(m, _)| m.sign() != num_bigint::Sign::NoSign)
        .map(|(_, e)| *e)
        .min()
        .unwrap_or(0);
    Some(parts.map(|(m, e)| m << usize::try_from(e - lowest).unwrap_or(0)))
}

/// A grid point's coordinates.
fn big_point(p: P3) -> [BigInt; 3] {
    [p.x.get(), p.y.get(), p.z.get()].map(BigInt::from)
}

/// a · b.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "big integers grow instead of overflowing"
)]
fn big_dot(a: &[BigInt; 3], b: &[BigInt; 3]) -> BigInt {
    let [ax, ay, az] = a;
    let [bx, by, bz] = b;
    ax * bx + ay * by + az * bz
}

/// a × b.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "big integers grow instead of overflowing"
)]
fn big_cross(a: &[BigInt; 3], b: &[BigInt; 3]) -> [BigInt; 3] {
    let [ax, ay, az] = a;
    let [bx, by, bz] = b;
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

/// The sign of `x`.
fn sign(x: &BigInt) -> Ordering {
    match x.sign() {
        num_bigint::Sign::Minus => Ordering::Less,
        num_bigint::Sign::NoSign => Ordering::Equal,
        num_bigint::Sign::Plus => Ordering::Greater,
    }
}

/// The sign of a√p + b√q, for p and q not negative.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "big integers grow instead of overflowing"
)]
fn sign_of_two(a: &BigInt, p: &BigInt, b: &BigInt, q: &BigInt) -> Ordering {
    let first = if sign(p) == Ordering::Equal {
        Ordering::Equal
    } else {
        sign(a)
    };
    let second = if sign(q) == Ordering::Equal {
        Ordering::Equal
    } else {
        sign(b)
    };
    if first == Ordering::Equal || first == second {
        return second;
    }
    if second == Ordering::Equal {
        return first;
    }
    // Opposite signs: the larger square wins.
    match (a * a * p).cmp(&(b * b * q)) {
        Ordering::Greater => first,
        Ordering::Less => second,
        Ordering::Equal => Ordering::Equal,
    }
}

/// The sign of x/√α − y/√β + n/2^s, for positive α and β: times 2^s·√(αβ), the sign of
/// u√β + v√α + n√(αβ) with u = 2^s·x and v = −2^s·y.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "big integers grow instead of overflowing"
)]
fn sign_of_three(
    x: &BigInt,
    alpha: &BigInt,
    y: &BigInt,
    beta: &BigInt,
    n: &BigInt,
    s: usize,
) -> Ordering {
    let u = x << s;
    let v = -(y << s);
    let both = alpha * beta;
    let first = sign_of_two(&u, beta, &v, alpha);
    let third = sign(n);
    if third == Ordering::Equal || first == third {
        return first;
    }
    if first == Ordering::Equal {
        return third;
    }
    // Opposite signs: (u√β + v√α)² − n²αβ = (u²β + v²α − n²αβ) + 2uv√(αβ).
    let rational = &u * &u * beta + &v * &v * alpha - n * n * &both;
    let root = BigInt::from(2) * &u * &v;
    match sign_of_two(&rational, &BigInt::from(1), &root, &both) {
        Ordering::Greater => first,
        Ordering::Less => third,
        Ordering::Equal => Ordering::Equal,
    }
}

/// `k` + `x` as n/2^s, for a finite `x`.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "big integers grow instead of overflowing; an exponent is below 2^11"
)]
fn plus(k: i64, x: f64) -> (BigInt, usize) {
    let (m, e) = dyadic(x);
    let k = BigInt::from(k);
    match usize::try_from(-e) {
        Ok(s) => ((k << s) + m, s),
        Err(_) => (k + (m << usize::try_from(e).unwrap_or(0)), 0),
    }
}

/// What a committed plane is held to: the ideal plane's integer normal `u`, through `q` moved `k`
/// steps along it.
struct Target<'a> {
    what: &'static str,
    u: [BigInt; 3],
    q: P3,
    k: i64,
    extent: &'a [P3],
}

/// The laws of one committed plane: within budget, a finite deviation, and no corner of the extent
/// further from the ideal plane than the deviation says.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "big integers grow instead of overflowing"
)]
fn kept(case: &Case, target: &Target<'_>, got: &Committed) -> Result<(), Finding> {
    let detail = || format!("{}: {got:?}: {case:?}", target.what);
    let n = got.plane.n();
    if n.iter().any(|c| c.unsigned_abs() >= 1 << 83) || got.plane.d().unsigned_abs() >= 1 << 125 {
        return Err(Finding::new("a committed plane is within budget", detail));
    }
    if !got.deviation.is_finite() || got.deviation < 0.0 {
        return Err(Finding::new(
            "a deviation is a finite number of steps",
            detail,
        ));
    }
    let n = n.map(BigInt::from);
    let d = BigInt::from(got.plane.d());
    let (alpha, beta) = (big_dot(&n, &n), big_dot(&target.u, &target.u));
    if sign(&beta) == Ordering::Equal {
        return Err(Finding::new(
            "a plane is committed only where its ideal has a normal",
            detail,
        ));
    }
    let q = big_point(target.q);
    for corner in target.extent {
        let p = big_point(*corner);
        let committed = big_dot(&n, &p) + &d;
        let [px, py, pz] = &p;
        let [qx, qy, qz] = &q;
        let ideal = big_dot(&target.u, &[px - qx, py - qy, pz - qz]);
        // Above the deviation, then below its negative.
        let (high, s) = plus(target.k, -got.deviation);
        let (low, t) = plus(target.k, got.deviation);
        if sign_of_three(&committed, &alpha, &ideal, &beta, &high, s) == Ordering::Greater
            || sign_of_three(&committed, &alpha, &ideal, &beta, &low, t) == Ordering::Less
        {
            return Err(Finding::new(
                "no corner of the extent is further from the ideal plane than the deviation",
                || format!("{corner:?}: {}", detail()),
            ));
        }
    }
    Ok(())
}

/// A refusal is typed as its input asks: `NotFinite` for a NaN or infinite double, `Degenerate`
/// where `degenerate` says the input has no plane, and otherwise one an input can cause: those two,
/// `BudgetExceeded` for a move past the plane budget, or `DivisorContainsZero` (chapter 22's
/// `Undecided`) for an ideal normal too short for its enclosure to be told from zero; never a flip,
/// and never an error only a defect makes.
fn refused(
    case: &Case,
    what: &str,
    e: CommitError,
    doubles: &[f64],
    degenerate: bool,
) -> Result<(), Finding> {
    let detail = || format!("{what}: {e:?}: {case:?}");
    let CommitError::Num(why) = e else {
        return Err(Finding::new("a commit is never refused as a flip", detail));
    };
    if doubles.iter().any(|x| !x.is_finite()) {
        return if why == NumError::NotFinite {
            Ok(())
        } else {
            Err(Finding::new(
                "a NaN or infinite double is NotFinite",
                detail,
            ))
        };
    }
    if degenerate {
        return if why == NumError::Degenerate {
            Ok(())
        } else {
            Err(Finding::new("an input with no plane is Degenerate", detail))
        };
    }
    match why {
        NumError::Degenerate
        | NumError::NotFinite
        | NumError::BudgetExceeded
        | NumError::DivisorContainsZero => Ok(()),
        _ => Err(Finding::new(
            "a commit refuses only as its input can cause",
            detail,
        )),
    }
}

/// Whether every component is zero.
fn zero(v: [f64; 3]) -> bool {
    v.iter().all(|x| *x == 0.0)
}

/// The edge from `a` to `b`, in integers.
fn edge(case: &Case) -> [i128; 3] {
    let one = |p: i64, q: i64| i128::from(q).saturating_sub(i128::from(p));
    [
        one(case.a.x.get(), case.b.x.get()),
        one(case.a.y.get(), case.b.y.get()),
        one(case.a.z.get(), case.b.z.get()),
    ]
}

/// The ideal normal of the planes through the edge that hold the tilt: the edge crossed with it.
fn edge_normal(case: &Case) -> Option<[BigInt; 3]> {
    let tilt = exact_direction(case.tilt)?;
    Some(big_cross(&edge(case).map(BigInt::from), &tilt))
}

/// Whether the tilt lies exactly along the edge, as `decode` makes it when it scales the edge by a
/// power of two.
fn along_the_edge(case: &Case) -> bool {
    let e = edge(case);
    e != [0; 3] && edge_normal(case).is_some_and(|n| n.iter().all(|c| sign(c) == Ordering::Equal))
}

/// The plane through the edge.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "big integers grow instead of overflowing"
)]
fn edge_laws(case: &Case) -> Result<(), Finding> {
    let degenerate = case.a == case.b || zero(case.tilt) || along_the_edge(case);
    match through_edge(case.a, case.b, case.tilt, &case.extent) {
        Ok(got) => {
            let n = got.plane.n().map(BigInt::from);
            let d = BigInt::from(got.plane.d());
            for end in [case.a, case.b] {
                if sign(&(big_dot(&n, &big_point(end)) + &d)) != Ordering::Equal {
                    return Err(Finding::new(
                        "a plane through an edge holds both its ends exactly",
                        || format!("{end:?}: {got:?}: {case:?}"),
                    ));
                }
            }
            let Some(u) = edge_normal(case) else {
                return Err(Finding::new(
                    "a NaN or infinite double is NotFinite",
                    || format!("{got:?}: {case:?}"),
                ));
            };
            let target = Target {
                what: "through the edge",
                u,
                q: case.a,
                k: 0,
                extent: &case.extent,
            };
            kept(case, &target, &got)
        }
        Err(e) => {
            let no_extent =
                case.extent.is_empty() && PlaneClass::holding(edge(case), case.tilt).is_ok();
            refused(
                case,
                "through the edge",
                e,
                &case.tilt,
                degenerate || no_extent,
            )
        }
    }
}

/// The free plane.
fn free_laws(case: &Case) -> Result<(), Finding> {
    match moved_along(case.direction, case.point, case.distance, &case.extent) {
        Ok(got) => {
            let Some(u) = exact_direction(case.direction) else {
                return Err(Finding::new(
                    "a NaN or infinite double is NotFinite",
                    || format!("{got:?}: {case:?}"),
                ));
            };
            let target = Target {
                what: "moved along",
                u,
                q: case.point,
                k: case.distance,
                extent: &case.extent,
            };
            kept(case, &target, &got)
        }
        Err(e) => refused(
            case,
            "moved along",
            e,
            &case.direction,
            zero(case.direction),
        ),
    }
}

/// `v` under the signed permutation of `t`, from its matrix.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "big integers grow instead of overflowing"
)]
fn permuted(t: &GridTransform, v: &[BigInt; 3]) -> [BigInt; 3] {
    t.matrix().map(|row| {
        let [r0, r1, r2] = row.map(BigInt::from);
        let [x, y, z] = v;
        r0 * x + r1 * y + r2 * z
    })
}

/// A class's planes, and their images under the case's symmetry.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "big integers grow instead of overflowing"
)]
fn class_laws(
    case: &Case,
    what: &'static str,
    class: &PlaneClass,
    u: &[BigInt; 3],
) -> Result<(), Finding> {
    let own = match class.plane(case.point, case.distance, &case.extent) {
        Ok(got) => {
            let target = Target {
                what,
                u: u.clone(),
                q: case.point,
                k: case.distance,
                extent: &case.extent,
            };
            kept(case, &target, &got)?;
            Some(got)
        }
        Err(e) => {
            let degenerate = case.extent.is_empty() && case.distance == 0;
            refused(case, what, e, &[], degenerate)?;
            None
        }
    };
    let Some(t) = case.symmetry else {
        return Ok(());
    };
    let image = class.image(&t);
    let normal = class.normal().map(BigInt::from);
    let turned = permuted(&t, &normal);
    if image.normal().map(BigInt::from) != turned {
        return Err(Finding::new(
            "a class's image has its normal under the signed permutation",
            || format!("{what}: {class:?}: {case:?}"),
        ));
    }
    let moved: Option<Vec<P3>> = case.extent.iter().map(|p| t.apply(*p).ok()).collect();
    let (Ok(at), Some(moved), Some(own)) = (t.apply(case.point), moved, own) else {
        return Ok(());
    };
    let Ok(got) = image.plane(at, case.distance, &moved) else {
        return Ok(());
    };
    let shift = t.translation_part().map(BigInt::from);
    let d = BigInt::from(own.plane.d()) - big_dot(&turned, &shift);
    if got.plane.n().map(BigInt::from) != turned || BigInt::from(got.plane.d()) != d {
        return Err(Finding::new(
            "an image's plane is the plane's image exactly",
            || format!("{what}: {got:?} from {own:?}: {case:?}"),
        ));
    }
    let target = Target {
        what,
        u: permuted(&t, u),
        q: at,
        k: case.distance,
        extent: &moved,
    };
    kept(case, &target, &got)
}

/// Both classes: the planes holding the edge and the tilt, and those along the direction.
fn classes(case: &Case) -> Result<(), Finding> {
    match PlaneClass::holding(edge(case), case.tilt) {
        Ok(class) => {
            let Some(u) = edge_normal(case) else {
                return Err(Finding::new(
                    "a NaN or infinite double is NotFinite",
                    || format!("{class:?}: {case:?}"),
                ));
            };
            class_laws(case, "a class holding the edge", &class, &u)?;
        }
        Err(e) => {
            let degenerate = case.a == case.b || zero(case.tilt) || along_the_edge(case);
            refused(case, "a class holding the edge", e, &case.tilt, degenerate)?;
        }
    }
    match PlaneClass::along(case.direction) {
        Ok(class) => {
            let Some(u) = exact_direction(case.direction) else {
                return Err(Finding::new(
                    "a NaN or infinite double is NotFinite",
                    || format!("{class:?}: {case:?}"),
                ));
            };
            class_laws(case, "a class along the direction", &class, &u)
        }
        Err(e) => refused(
            case,
            "a class along the direction",
            e,
            &case.direction,
            zero(case.direction),
        ),
    }
}

/// A flip is refused exactly when the ideal sign is certain and the committed one differs.
fn flips(case: &Case, ideal: Interval) -> Result<(), Finding> {
    let (lo, hi) = (ideal.lo(), ideal.hi());
    for committed in [Sign::Negative, Sign::Zero, Sign::Positive] {
        let certain_and_differs = (lo > 0.0 && committed != Sign::Positive)
            || (hi < 0.0 && committed != Sign::Negative)
            || (lo == 0.0 && hi == 0.0 && committed != Sign::Zero);
        let got = check_flip(committed, ideal);
        let expected = if certain_and_differs {
            Err(CommitError::Flip)
        } else {
            Ok(())
        };
        if got != expected {
            return Err(Finding::new(
                "a flip is refused exactly when the ideal sign is certain and differs",
                || format!("{committed:?} against {ideal:?}: {got:?}: {case:?}"),
            ));
        }
    }
    Ok(())
}

/// An ideal plane's reading at the probe holds the exact reading u·(p − q)/|u| − k.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "big integers grow instead of overflowing"
)]
fn encloses(case: &Case, u: &[BigInt; 3], q: P3, k: i64, reading: Interval) -> Result<(), Finding> {
    let beta = big_dot(u, u);
    let [px, py, pz] = big_point(case.probe);
    let [qx, qy, qz] = big_point(q);
    let along = big_dot(u, &[px - qx, py - qy, pz - qz]);
    // The reading less an end, times 2^s·√β: along·2^s + n√β, where n/2^s is −k less the end.
    let side = |end: f64| {
        let (n, s) = plus(k.saturating_neg(), -end);
        sign_of_two(&(&along << s), &BigInt::from(1), &n, &beta)
    };
    if sign(&beta) == Ordering::Equal
        || side(reading.lo()) == Ordering::Less
        || side(reading.hi()) == Ordering::Greater
    {
        return Err(Finding::new(
            "an ideal reading holds the exact reading",
            || format!("{reading:?}: {case:?}"),
        ));
    }
    Ok(())
}

/// The ideal planes read at the probe, and the case's own interval, against the flip check.
fn flip_laws(case: &Case) -> Result<(), Finding> {
    if let (Ok(ideal), Some(u)) = (
        Ideal::through_edge(case.a, case.b, case.tilt),
        edge_normal(case),
    ) && let Ok(reading) = ideal.reading(case.probe)
    {
        encloses(case, &u, case.a, 0, reading)?;
        flips(case, reading)?;
    }
    if let (Ok(ideal), Some(u)) = (
        Ideal::moved(case.direction, case.point, case.distance),
        exact_direction(case.direction),
    ) && let Ok(reading) = ideal.reading(case.probe)
    {
        encloses(case, &u, case.point, case.distance, reading)?;
        flips(case, reading)?;
    }
    match case.interval {
        Some(interval) => flips(case, interval),
        None => Ok(()),
    }
}

/// Holds the case to the laws.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    edge_laws(case)?;
    free_laws(case)?;
    classes(case)?;
    flip_laws(case)
}
