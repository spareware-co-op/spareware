//! Inputs for interval arithmetic: [`new`], [`contains`], [`add`], [`sub`], [`mul`], [`div`],
//! [`mul_add`], [`square`], [`sqrt`], [`norm`], [`neg`] and [`certain_sign`]; and for the certified
//! enclosures of the proven maths functions, [`sinpi`], [`cospi`], [`exp`] and [`ln`].
//!
//! The enclosures' classes: `typical`, narrow intervals of half-turns around a peak, strictly
//! between peaks, and anywhere, within 2^10 half-turns; exponents within ±32; logarithms of
//! positive intervals from 2^−30 to 2^30. `extreme`, in rotation: for the periodic two, intervals
//! wider than a turn, real intervals between doubles from 2^53 up that hold a whole turn, ends at
//! multiples of ½ (exact values), a point at a peak, subnormal ends around zero, and an interval
//! ending one double short of a peak; for `exp`, upper ends either side of overflow (past it,
//! `NotFinite`), the deep underflow (the lower bound held at +0), an end at exactly 0, and subnormal
//! ends; for `ln`, intervals reaching below zero (`OutsideDomain`), a lower end of zero
//! (`NotFinite`), an end at exactly 1, subnormal lower ends and the top of the range.
//!
//! Classes:
//!
//! - `typical`: narrow positive intervals, narrow negative intervals and intervals around zero, with
//!   bounds of ordinary size (exponents −10 to 20), in rotation: the first operand of the n-th
//!   input has kind n mod 3, the second kind ⌊n / 3⌋ mod 3 and, for `mul_add` and `norm`, the third
//!   kind ⌊n / 9⌋ mod 3, so every pairing (and every triple) of kinds occurs. For `square`, the
//!   three kinds in rotation, one per branch (above, below and around zero). For `new`, ordered
//!   pairs of ordinary bounds (a point, a −0 bound); for `contains`, values inside, below, above
//!   and on each bound.
//! - `extreme`: in rotation, a result too large for a double (the operation returns `NotFinite`),
//!   subnormal bounds, huge with tiny, and the operation's other edge: a divisor containing zero
//!   (`DivisorContainsZero`), a square root of negative values (`NegativeSqrt`), a zero bound.
//!   For `square`, the lower square underflows to zero on subnormal bounds and is held at zero.
//!   For `mul_add`, a product whose sum with the third operand cancels near the top of the range.
//!   For `norm`, components whose squares, or whose sum of squares, pass the largest double;
//!   subnormal components and components around zero (coincident points), whose sum of squares
//!   widening takes below zero, so the length's lower bound is cut at exactly 0; and large
//!   finite lengths. Every extreme batch holds both successes and errors where its operation has
//!   errors (the tests check it). For `new`, each error (NaN, infinite, reversed bounds) and the
//!   subnormal, huge and signed-zero edges.
//!
//! Guarantees: every batch is full and the same on every target, and its classes are what they
//! claim (`tests/inputs.rs` checks every branch and error named here); every result is checked
//! against exact rationals by `tests/attack_g06.rs`.
//!
//! Method: the operand classes of round-to-nearest interval arithmetic with outward widening
//! (Harris, "Why interval arithmetic won't cure your floating point blues", 2011,
//! <https://accu.org/journals/overload/19/103/harris_1974/>): narrow and straddling intervals,
//! overflow, underflow and the zero cases each operation documents.
//!
//! Cost: a few draws of [`SplitMix64`] per input, once per batch, outside every measured run.

use geom_num::interval::Interval;

use super::eft::{double, positive, pow2, subnormal};
use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// The interval between `a` and `b`, in either order, if both are finite.
fn between(a: f64, b: f64) -> Option<Interval> {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    Interval::new(lo, hi).ok()
}

/// A narrow positive interval [x, x(1 + 2^−k)] of ordinary size.
fn narrow_positive(g: &mut SplitMix64) -> Option<Interval> {
    let x = positive(g, -10, 20);
    let k = g.range(8, 40);
    between(x, x + x * pow2(-k))
}

/// A narrow negative interval.
fn narrow_negative(g: &mut SplitMix64) -> Option<Interval> {
    narrow_positive(g).map(Interval::neg)
}

/// An interval around zero.
fn around_zero(g: &mut SplitMix64) -> Option<Interval> {
    between(-positive(g, -10, 10), positive(g, -10, 10))
}

/// A typical interval of kind `kind` mod 3: narrow positive, narrow negative or around zero.
fn typical(g: &mut SplitMix64, kind: usize) -> Option<Interval> {
    match kind % 3 {
        0 => narrow_positive(g),
        1 => narrow_negative(g),
        _ => around_zero(g),
    }
}

/// A typical interval that does not contain zero: positive for even `kind`, negative for odd.
fn away_from_zero(g: &mut SplitMix64, kind: usize) -> Option<Interval> {
    if kind.is_multiple_of(2) {
        narrow_positive(g)
    } else {
        narrow_negative(g)
    }
}

/// A positive interval with bounds from 2^1020 to just below 2^1024.
fn huge(g: &mut SplitMix64) -> Option<Interval> {
    between(positive(g, 1020, 1023), positive(g, 1020, 1023))
}

/// A positive interval with both bounds at or above 2^1023: any sum or product of two of them
/// passes the largest double.
fn top(g: &mut SplitMix64) -> Option<Interval> {
    between(positive(g, 1023, 1023), positive(g, 1023, 1023))
}

/// A positive interval with subnormal bounds.
fn tiny(g: &mut SplitMix64) -> Option<Interval> {
    let (a, b) = (subnormal(g), subnormal(g));
    between(if a < 0.0 { -a } else { a }, if b < 0.0 { -b } else { b })
}

/// Which binary operation a batch is for, so the extreme class can aim at its overflow.
#[derive(Clone, Copy)]
enum Binary {
    Add,
    Sub,
    Mul,
}

/// Pairs for a binary operation other than division.
fn pairs(op: &str, which: Binary, class: Class) -> Vec<(Interval, Interval)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|n| Some((typical(&mut g, n)?, typical(&mut g, n / 3)?))),
        Class::Extreme => batch_of(|n| match n % 4 {
            // A bound of the exact result passes 2^1024: NotFinite.
            0 => match which {
                Binary::Add | Binary::Mul => Some((top(&mut g)?, top(&mut g)?)),
                Binary::Sub => Some((top(&mut g)?, top(&mut g)?.neg())),
            },
            1 => Some((tiny(&mut g)?, tiny(&mut g)?)),
            2 => Some((huge(&mut g)?, tiny(&mut g)?)),
            _ => Some((typical(&mut g, n / 4)?, huge(&mut g)?)),
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// (lo, hi) pairs for [`Interval::new`].
#[must_use]
pub fn new(class: Class) -> Vec<(f64, f64)> {
    let mut g = rng("interval::new", class);
    match class {
        Class::Typical => batch_of(|n| {
            Some(match n % 4 {
                // A point.
                0 => {
                    let x = double(&mut g, -10, 20);
                    (x, x)
                }
                // A −0 lower bound, stored as +0.
                1 => (-0.0, positive(&mut g, -10, 20)),
                _ => {
                    let (a, b) = (double(&mut g, -10, 20), double(&mut g, -10, 20));
                    if a <= b { (a, b) } else { (b, a) }
                }
            })
        }),
        Class::Extreme => batch_of(|n| {
            Some(match n % 8 {
                0 => (f64::NAN, positive(&mut g, -10, 20)),
                1 => (f64::NEG_INFINITY, positive(&mut g, -10, 20)),
                2 => (-positive(&mut g, -10, 20), f64::INFINITY),
                // Reversed bounds: EmptyInterval.
                3 => {
                    let x = positive(&mut g, -10, 20);
                    (x, x - x * pow2(-g.range(1, 40)))
                }
                4 => {
                    let (a, b) = (subnormal(&mut g), subnormal(&mut g));
                    if a <= b { (a, b) } else { (b, a) }
                }
                5 => (-f64::MAX, f64::MAX),
                6 => (-0.0, -0.0),
                _ => (-positive(&mut g, 1000, 1023), positive(&mut g, 1000, 1023)),
            })
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// (interval, value) pairs for [`Interval::contains`]: values inside, below, above, on the lower
/// bound and on the upper bound, in rotation.
#[must_use]
pub fn contains(class: Class) -> Vec<(Interval, f64)> {
    let mut g = rng("interval::contains", class);
    match class {
        Class::Typical => batch_of(|n| {
            let x = typical(&mut g, n / 5)?;
            let width = x.hi() - x.lo();
            let v = match n % 5 {
                0 => x.lo() + width / 2.0,
                1 => x.lo() - width - 1.0,
                2 => x.hi() + width + 1.0,
                3 => x.lo(),
                _ => x.hi(),
            };
            Some((x, v))
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Pairs for [`Interval::add`].
#[must_use]
pub fn add(class: Class) -> Vec<(Interval, Interval)> {
    pairs("interval::add", Binary::Add, class)
}

/// Pairs for [`Interval::sub`].
#[must_use]
pub fn sub(class: Class) -> Vec<(Interval, Interval)> {
    pairs("interval::sub", Binary::Sub, class)
}

/// Pairs for [`Interval::mul`].
#[must_use]
pub fn mul(class: Class) -> Vec<(Interval, Interval)> {
    pairs("interval::mul", Binary::Mul, class)
}

/// (dividend, divisor) pairs for [`Interval::div`].
#[must_use]
pub fn div(class: Class) -> Vec<(Interval, Interval)> {
    let mut g = rng("interval::div", class);
    match class {
        Class::Typical => batch_of(|n| Some((typical(&mut g, n)?, away_from_zero(&mut g, n / 3)?))),
        Class::Extreme => batch_of(|n| match n % 4 {
            // DivisorContainsZero.
            0 => Some((typical(&mut g, n / 4)?, around_zero(&mut g)?)),
            // Huge over subnormal: NotFinite.
            1 => Some((huge(&mut g)?, tiny(&mut g)?)),
            // Subnormal over huge: the quotient underflows to zero.
            2 => Some((tiny(&mut g)?, huge(&mut g)?)),
            _ => Some((huge(&mut g)?, huge(&mut g)?)),
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// (a, b, c) triples for [`Interval::mul_add`], a · b + c.
#[must_use]
pub fn mul_add(class: Class) -> Vec<(Interval, Interval, Interval)> {
    let mut g = rng("interval::mul_add", class);
    match class {
        Class::Typical => batch_of(|n| {
            Some((
                typical(&mut g, n)?,
                typical(&mut g, n / 3)?,
                typical(&mut g, n / 9)?,
            ))
        }),
        Class::Extreme => batch_of(|n| match n % 4 {
            // Both factors at or above 2^1023: the upper bound passes the largest double,
            // NotFinite.
            0 => Some((top(&mut g)?, top(&mut g)?, typical(&mut g, n / 4)?)),
            // Subnormal factors and addend: the product underflows below every subnormal.
            1 => Some((tiny(&mut g)?, tiny(&mut g)?, tiny(&mut g)?)),
            // Huge times tiny, plus an ordinary value.
            2 => Some((huge(&mut g)?, tiny(&mut g)?, typical(&mut g, n / 4)?)),
            // A product from 2^1020 to 2^1022 less an addend as large: cancellation, which the
            // single rounding of the fused multiply-add keeps exact.
            _ => {
                let (a, b) = (positive(&mut g, 510, 510), positive(&mut g, 510, 510));
                let c = between(-a * b, -a * b * (1.0 - pow2(-g.range(1, 40))))?;
                Some((between(a, a)?, between(b, b)?, c))
            }
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Intervals for [`Interval::square`].
#[must_use]
pub fn square(class: Class) -> Vec<Interval> {
    let mut g = rng("interval::square", class);
    match class {
        // Above, below and around zero in rotation: one per branch.
        Class::Typical => batch_of(|n| typical(&mut g, n)),
        Class::Extreme => batch_of(|n| {
            let x = match n % 4 {
                // A bound of 2^512 or more: the square passes the largest double, NotFinite.
                0 => between(positive(&mut g, 512, 1000), positive(&mut g, 512, 1000)),
                // Subnormal bounds: the lower square underflows to zero and is held there.
                1 => tiny(&mut g),
                // Large finite squares, up to 2^1022.
                2 => between(positive(&mut g, 500, 510), positive(&mut g, 500, 510)),
                // A zero bound: the lower bound of the square is exactly 0.
                _ => between(0.0, positive(&mut g, -10, 20)),
            }?;
            // Every other round of the rotation below zero, mirrored.
            Some(if (n / 4).is_multiple_of(2) {
                x
            } else {
                x.neg()
            })
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Intervals for [`Interval::sqrt`].
#[must_use]
pub fn sqrt(class: Class) -> Vec<Interval> {
    let mut g = rng("interval::sqrt", class);
    match class {
        Class::Typical => batch_of(|_| narrow_positive(&mut g)),
        Class::Extreme => batch_of(|n| match n % 4 {
            // A zero lower bound, whose root is exact.
            0 => between(0.0, positive(&mut g, -10, 20)),
            1 => tiny(&mut g),
            2 => huge(&mut g),
            // NegativeSqrt.
            _ => around_zero(&mut g),
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Vectors of three components for [`Interval::norm`]: the coordinate differences of two points
/// in 3D.
#[must_use]
pub fn norm(class: Class) -> Vec<[Interval; 3]> {
    let mut g = rng("interval::norm", class);
    match class {
        Class::Typical => batch_of(|n| {
            Some([
                typical(&mut g, n)?,
                typical(&mut g, n / 3)?,
                typical(&mut g, n / 9)?,
            ])
        }),
        Class::Extreme => batch_of(|n| {
            Some(match n % 5 {
                // A component of 2^512 or more: its square passes the largest double, NotFinite.
                0 => [
                    between(positive(&mut g, 512, 1000), positive(&mut g, 512, 1000))?,
                    typical(&mut g, n / 5)?,
                    typical(&mut g, n / 5)?,
                ],
                // Components of about 1.38 · 2^511: each square is below 2^1023, and so is the
                // sum of two, but the sum of three passes the largest double, NotFinite.
                1 => {
                    let x = pow2(511) * 1.375;
                    let mut c = || between(x, x * (1.0 + pow2(-g.range(8, 40))));
                    [c()?, c()?, c()?]
                }
                // Subnormal components: the squares underflow, and widening the sum takes its
                // lower bound below zero, where the length is cut at exactly 0.
                2 => [tiny(&mut g)?, tiny(&mut g)?, tiny(&mut g)?],
                // Components around zero, as between coincident points: the length starts at 0.
                3 => [
                    around_zero(&mut g)?,
                    around_zero(&mut g)?,
                    around_zero(&mut g)?,
                ],
                // Components from 2^509 to 2^510: squares below 2^1020, a finite length below
                // 2^511.
                _ => {
                    let mut c = || between(positive(&mut g, 509, 509), positive(&mut g, 509, 509));
                    [c()?, c()?, c()?]
                }
            })
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Intervals for [`Interval::neg`].
#[must_use]
pub fn neg(class: Class) -> Vec<Interval> {
    let mut g = rng("interval::neg", class);
    match class {
        Class::Typical => batch_of(|n| typical(&mut g, n)),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Intervals for [`Interval::certain_sign`]: positive, negative, around zero and exactly zero, in
/// rotation, so every branch is taken.
#[must_use]
pub fn certain_sign(class: Class) -> Vec<Interval> {
    let mut g = rng("interval::certain_sign", class);
    match class {
        Class::Typical => batch_of(|n| match n % 4 {
            0 => narrow_positive(&mut g),
            1 => narrow_negative(&mut g),
            2 => around_zero(&mut g),
            _ => between(0.0, 0.0),
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// A narrow interval of half-turns near `centre`: from `centre − a` to `centre + b`, with `a` and
/// `b` from 2^−30 to 2^−3.
fn narrow_around(g: &mut SplitMix64, centre: f64) -> Option<Interval> {
    let a = positive(g, -30, -3);
    let b = positive(g, -30, -3);
    between(centre - a, centre + b)
}

/// Half-turns for the periodic enclosures, with `peak` the offset of a peak in each turn (½ for
/// the sine's maximum, 0 for the cosine's): typical intervals, in rotation, around a peak, strictly
/// between two peaks, and anywhere; or the extreme ones.
fn half_turns(g: &mut SplitMix64, class: Class, peak: f64) -> Vec<Interval> {
    match class {
        Class::Typical => batch_of(|n| {
            // A whole turn below 2^10, of either sign.
            let turn = 2.0 * f64::from(u16::try_from(g.below(512)).unwrap_or(0)) * g.sign() as f64;
            match n % 3 {
                0 => narrow_around(g, turn + peak),
                // A quarter turn past the peak, at least 2^−4 from either peak or trough.
                1 => narrow_around(g, turn + peak + 0.25),
                _ => {
                    let offset = positive(g, -10, 0);
                    narrow_around(g, turn + peak + offset)
                }
            }
        }),
        Class::Extreme => batch_of(|n| match n % 6 {
            // Wider than a turn: [−1, 1].
            0 => between(-positive(g, 0, 10), positive(g, 1, 10)),
            // From 2^53 the doubles are even integers, but a real interval between two of them
            // holds a whole turn.
            1 => {
                let x = pow2(53 + g.range(0, 900));
                between(x, x + 2.0 * x * pow2(-52))
            }
            // Ends at multiples of ½, whose values are exact.
            2 => {
                let k = f64::from(u16::try_from(g.below(64)).unwrap_or(0)) * 0.5;
                between(k, k + 0.5)
            }
            // A point interval at a peak.
            3 => between(peak + 2.0, peak + 2.0),
            // Around zero, subnormal ends.
            4 => between(-subnormal(g).abs(), subnormal(g).abs()),
            // Up to one double short of a peak.
            _ => between(peak + 1.75, (peak + 2.0).next_down()),
        }),
        _ => Vec::new(),
    }
}

/// Intervals for [`Interval::sinpi`], in half-turns.
#[must_use]
pub fn sinpi(class: Class) -> Vec<Interval> {
    half_turns(&mut rng("interval::sinpi", class), class, 0.5)
}

/// Intervals for [`Interval::cospi`], in half-turns.
#[must_use]
pub fn cospi(class: Class) -> Vec<Interval> {
    half_turns(&mut rng("interval::cospi", class), class, 0.0)
}

/// Intervals for [`Interval::exp`]: typical ones within ±32; or, in rotation, near overflow on
/// either side of 709.78 (past it, `NotFinite`), deep in the underflow (the lower bound held at
/// +0), with an end exactly 0 (whose value is exactly 1), and tiny ones around zero.
#[must_use]
pub fn exp(class: Class) -> Vec<Interval> {
    let mut g = rng("interval::exp", class);
    match class {
        Class::Typical => batch_of(|_| between(double(&mut g, -10, 4), double(&mut g, -10, 4))),
        Class::Extreme => batch_of(|n| {
            let g = &mut g;
            match n % 5 {
                0 => between(700.0, 709.0 + positive(g, -10, -1)),
                1 => between(700.0, 709.79 + positive(g, -10, 4)),
                2 => between(-1000.0 - positive(g, 0, 8), -745.0 - positive(g, -10, 4)),
                3 => between(0.0, positive(g, -10, 4)),
                _ => between(-subnormal(g).abs(), subnormal(g).abs()),
            }
        }),
        _ => Vec::new(),
    }
}

/// Intervals for [`Interval::ln`]: typical positive ones from 2^−30 to 2^30; or, in rotation, ones
/// reaching below zero (`OutsideDomain`), a lower bound of zero (`NotFinite`), an end exactly 1
/// (whose logarithm is exactly 0), subnormal lower bounds, and bounds near the largest double.
#[must_use]
pub fn ln(class: Class) -> Vec<Interval> {
    let mut g = rng("interval::ln", class);
    match class {
        Class::Typical => {
            batch_of(|_| between(positive(&mut g, -30, 30), positive(&mut g, -30, 30)))
        }
        Class::Extreme => batch_of(|n| {
            let g = &mut g;
            match n % 5 {
                0 => around_zero(g),
                1 => between(0.0, positive(g, -10, 10)),
                2 => between(1.0, positive(g, 0, 10)),
                3 => between(subnormal(g).abs(), positive(g, -10, 10)),
                _ => huge(g),
            }
        }),
        _ => Vec::new(),
    }
}
