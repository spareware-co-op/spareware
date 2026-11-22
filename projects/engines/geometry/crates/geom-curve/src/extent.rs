//! Closed-form extents: the range of one function of one parameter over a span, from its values at
//! the span's ends and at its critical points, each critical point in closed form.
//!
//! What it does: [`Scalar`] names the functions the analytic kinds are made of, one coordinate of
//! one derivative at a time, each with interval coefficients (enclosures of the exact ones the
//! stored data define), and the closed form of the zeros of its derivative:
//!
//! | Scalar | g(t) | Critical points (g′(t) = 0) |
//! | --- | --- | --- |
//! | `Wave` | p + q·t + a·cos πt + b·sin πt | with φ = atan2(b, a)/π and R = √(a² + b²): t = φ + m when q = 0; t = φ + ψ + 2m and t = φ + 1 − ψ + 2m, ψ = asin(q/(πR))/π, when \|q\| < πR; none when \|q\| ≥ πR |
//! | `Quadratic` | p + q·t + r·t² | t = −q/(2r) |
//! | `Hyperbolic` | p + a·cosh t + b·sinh t | t = ½·ln((a − b)/(a + b)), where that quotient is positive |
//! | `Involute` | p + a·(cos πt + πt·sin πt) + b·(sin πt − πt·cos πt) | t = 0, and t = φ + ½ + m |
//! | `Modulated` | p + (a + c·t)·cos πt + (b + d·t)·sin πt | no closed form |
//!
//! for every whole number m. [`extent`] encloses the range of g over a span: the hull of g at the
//! span's two ends and over the enclosure of each critical point inside it, so a box built from it
//! is the curve's or the surface's own extent, up to the rounding of a few operations, never the
//! wider box an interval extension over the whole span gives.
//!
//! Guarantees:
//!
//! - **Certified.** The result holds g(t) for every t of the span and every coefficient in its
//!   enclosure. A function continuous on a closed span takes its extremes at the ends or where g′
//!   is zero; every such zero inside the span lies in one of the enclosures evaluated (the table's
//!   closed forms in interval arithmetic: `geom-num`'s proven `atan2`, `ln`, `sinpi` and `cospi`,
//!   π between its neighbouring doubles), and g over an enclosure holds g there.
//! - **Few evaluations however long the span.** Periodic critical points come in families
//!   t = c + 2m, each one period of cos πt apart. Along a family the values of g are constant
//!   (`Wave` with q = 0) or step by the same amount from one member to the next (2q for `Wave`,
//!   ±2πR for `Involute`), so the extremes over the members inside the span are at the first or
//!   the last of them. So at most four members of a family are evaluated: the first two and the
//!   last two whose enclosure meets the span, since an enclosure narrower than a period may reach
//!   into the span while its member lies just outside, and the next member then lies inside.
//! - **Undecided cases are refined, not guessed.** Where the coefficients' enclosures leave a
//!   closed form undecided (a and b enclosing the origin without both being zero, \|q\| enclosing
//!   πR, r or a + b enclosing zero without being zero, a family wider than half a period, a span or
//!   critical point beyond 2⁵⁰), and for `Modulated`, which has no closed form, the span is cut
//!   into monotone pieces: at most 64 pieces of at most a quarter of a half-turn; a piece whose
//!   derivative's enclosure has a certain sign contributes g at its two ends, any other is cut in
//!   eight and tested again, and a piece still undecided contributes the intersection of g's
//!   interval extension over it and its mean-value form g(m) + g′·[−r, r] about its middle m. That
//!   is sound too, and only a little wider.
//! - **Deterministic.** The same function and span give the same bits on every target.
//!
//! Method: the extreme value theorem with the critical points in closed form, the amplitude and
//! phase form R·cos(π(t − φ)) of a·cos πt + b·sin πt, and interval arithmetic for every closed form
//! (R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to Interval Analysis*, SIAM (2009),
//! <https://doi.org/10.1137/1.9780898717716>: the natural interval extension, and the monotonicity
//! test and the mean-value form of chapter 6 for the pieces).
//!
//! Cost: a few interval evaluations of g, one `atan2` and at most sixteen evaluations per family;
//! the pieces' route at most 64 × 9 evaluations of g and of g′. One work unit per extent and per
//! piece tried, and [`FAR_CORNER`] more for each corner of an arctangent's box that the certified
//! arctangent takes to its 192-bit path, where it spends forty times its usual instructions.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': p, q, r, a, b, c and d the coefficients, g the function, t a parameter, m a member of a family"
)]

use alloc::vec;
use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_num::math;
use geom_run::WorkMeter;

use crate::error::CurveError;
use crate::form;
use crate::tight::{self, point};

/// The most pieces the pieces' route cuts a span into.
const MAX_PIECES: u32 = 64;

/// The pieces each undecided piece is cut into, once.
const SUBPIECES: u32 = 8;

/// The steps a scan for a family's first or last member in a span may take.
const SCAN: usize = 6;

/// 2⁵⁰: past it a period of 2 is too near the doubles' spacing to place a member of a family.
const FAR: f64 = 1_125_899_906_842_624.0;

/// The bits of 53 binades: the arctangent's first gate sends a corner whose coordinates' bit
/// patterns are this far apart straight to its accurate path ([`dear`]).
const FIRST_GATE: u64 = 53 << 52;

/// 2⁴⁴: near the positive x-axis, a corner whose x is at least this many times its |y| is charged
/// as dear. From 2⁴⁷ apart its angle is below 2⁻⁴⁷, which the arctangent's fast paths cannot
/// decide; from here to there they decide some corners and not others, more than one in ten not
/// ([`dear`]).
const NEAR_AXIS: f64 = 17_592_186_044_416.0;

/// 2⁹⁶: near the positive x-axis, a corner whose x is more than this many times its |y| has a
/// quotient y/x below 2⁻⁹⁶, whose rounding the arctangent's accurate path returns as it is
/// ([`dear`]).
const TINY_QUOTIENT: f64 = 79_228_162_514_264_337_593_543_950_336.0;

/// 2⁻⁹¹⁵: below it the arctangent tests a quotient for exactness with both sides scaled by
/// [`SCALE`], where the remainder cannot underflow ([`exact_quotient`]).
const SMALL_Y: f64 = f64::from_bits(108 << 52);

/// 2¹⁰⁵: see [`SMALL_Y`].
const SCALE: f64 = 40_564_819_207_303_340_847_894_502_572_032.0;

/// The work units an extent is charged for each corner of an arctangent's box that the certified
/// arctangent takes to its 192-bit path (the private `dear` says which, from the arctangent's own
/// gates, and counts in the band where the gates cannot say), on top of its own unit.
///
/// What it stands for, measured: the correctly rounded `atan2` (`geom_num::math`, the CORE-MATH
/// port) decides almost every argument in double precision, in about 240 instructions, but its
/// 192-bit path takes about 10,300 (callgrind on a release build of an offset of an ellipse with
/// axes of 2⁻⁶⁰ and 2⁶⁰, issue #765: 5,285,592 such calls took 54,441,804,432 instructions, and
/// the 2,642,796 others 641,714,340). A corner near the positive x-axis that both fast paths try
/// first takes about 2% more than one the first gate sends straight there (13,339 instructions a
/// call against 13,045, counted on a later build and on the arctangent alone, so only the ratio
/// is used here), and a corner the accurate path answers without its 192-bit approximation far
/// less (130 to 1,136), so it is not charged. At the curve offset's rate of about 285 instructions
/// a unit (`crate::offset`, "What the work costs"), 10,300 instructions are 36 units; the charge
/// is set at 40, which holds the 2% more of the corners the fast paths try first. The extent's own
/// unit stands for the rest of the closed form, the usual arctangent included, so an extent with
/// no such corner is charged as it always was.
pub const FAR_CORNER: u64 = 40;

/// One function of one parameter with interval coefficients (module documentation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scalar {
    /// p + q·t + a·cos πt + b·sin πt: a line, a circle's or ellipse's coordinate, a helix's.
    Wave {
        /// The constant.
        p: Interval,
        /// The slope.
        q: Interval,
        /// The coefficient of cos πt.
        a: Interval,
        /// The coefficient of sin πt.
        b: Interval,
    },
    /// p + q·t + r·t²: a parabola's coordinate.
    Quadratic {
        /// The constant.
        p: Interval,
        /// The coefficient of t.
        q: Interval,
        /// The coefficient of t².
        r: Interval,
    },
    /// p + a·cosh t + b·sinh t: a hyperbola's coordinate.
    Hyperbolic {
        /// The constant.
        p: Interval,
        /// The coefficient of cosh t.
        a: Interval,
        /// The coefficient of sinh t.
        b: Interval,
    },
    /// p + a·(cos πt + πt·sin πt) + b·(sin πt − πt·cos πt): an involute's coordinate.
    Involute {
        /// The constant.
        p: Interval,
        /// The coefficient of cos πt + πt·sin πt.
        a: Interval,
        /// The coefficient of sin πt − πt·cos πt.
        b: Interval,
    },
    /// p + (a + c·t)·cos πt + (b + d·t)·sin πt: an involute's derivatives.
    Modulated {
        /// The constant.
        p: Interval,
        /// The constant part of the coefficient of cos πt.
        a: Interval,
        /// The constant part of the coefficient of sin πt.
        b: Interval,
        /// The slope of the coefficient of cos πt.
        c: Interval,
        /// The slope of the coefficient of sin πt.
        d: Interval,
    },
}

/// Where g′ is zero: families of critical points, or undecided.
enum Critical {
    /// Every critical point lies in one of these: (enclosure of c, whether it repeats every 2).
    Families(Vec<(Interval, bool)>),
    /// The enclosures leave the closed form undecided.
    Undecided,
}

/// Whether an enclosure is exactly the point zero: the test the closed forms use to see that a
/// coefficient, and so a term, is absent.
#[must_use]
pub fn is_zero(x: Interval) -> bool {
    x.lo() == 0.0 && x.hi() == 0.0
}

/// Whether an enclosure excludes zero.
fn excludes_zero(x: Interval) -> bool {
    x.lo() > 0.0 || x.hi() < 0.0
}

/// The hull of two enclosures.
fn hull(x: Interval, y: Interval) -> Result<Interval, CurveError> {
    Ok(Interval::new(
        tight::smaller(x.lo(), y.lo()),
        tight::larger(x.hi(), y.hi()),
    )?)
}

/// x times the value `f` gives, exactly zero when x is: a term with a zero coefficient adds
/// nothing, not a rounding.
fn times(
    x: Interval,
    f: impl FnOnce() -> Result<Interval, CurveError>,
) -> Result<Interval, CurveError> {
    if is_zero(x) {
        point(0.0)
    } else {
        tight::mul(x, f()?)
    }
}

/// Whether the certified arctangent takes the corner (x, y) to its 192-bit path, its rational
/// approximation of degree 29, read from the port's own gates (`geom_num::math`'s `atan2.rs`,
/// CORE-MATH's `atan2.c`):
///
/// - Its first gate sends a corner whose coordinates' bit patterns are 53 binades apart or more
///   (`dxy`; with |y| the larger, more than 53) straight to the accurate path. For normal
///   coordinates that is the larger at least 2⁵³ times the smaller (more than, with |y| the
///   larger); with a subnormal one the bit patterns, not the values, set the cut.
/// - Its two fast paths decide almost every other corner. Those they never decide are near the
///   positive x-axis, where the angle is about y/x: the second path's error bound has a fixed part
///   of 1.25·2⁻¹⁰¹ (`EPS2_REL`·`EPS2_ABS`), wider than an ulp of an angle below 2⁻⁴⁷, so it never
///   decides a corner with x > 0 and x ≥ 2⁴⁷·|y|, and the accurate path takes it.
/// - The accurate path answers three kinds of corner without its rational approximation: a
///   quotient y/x that rounds to zero, and, with x > 0, a quotient that is exactly a double
///   ([`exact_quotient`]) or one below 2⁻⁹⁶ (`tiny_quotient`), whose atan2 rounds as it does.
///
/// So a corner is certainly dear near the positive x-axis from 2⁴⁷ to 2⁹⁶ apart unless its
/// quotient is exact, and elsewhere past the first gate unless its quotient rounds to zero; a zero
/// coordinate is on an axis, decided at once. Checked against the port's own count of the calls
/// that reach its rational approximation, on 400,000 corners of every kind (subnormal, exact,
/// every quadrant and distance): none of those charged in vain.
///
/// One band is charged from the box, because only the arctangent itself can tell which of its
/// corners are dear (issue #853): just short of 2⁴⁷ apart near the positive x-axis the bound's
/// fixed part is still a large share of an ulp, 5/16 of it from 2⁴⁶ to 2⁴⁷ apart and half as much
/// in each binade below, so a corner whose angle falls that near a rounding boundary goes to the
/// 192-bit path too. Which ones is decided by the angle's last bits. Counted on the same 400,000
/// corners, of those whose quotient is not exact: 1,161 of 1,823 from 2⁴⁶ to 2⁴⁷ apart (64%), 560
/// of 1,828 from 2⁴⁵ (31%), 268 of 1,855 from 2⁴⁴ (14%), then 134 of 1,815 (7%), 67, 35, 27 and 6
/// a binade down to 2³⁹. An ellipse's coefficients are fixed, so an offset of one asks the same
/// corners in every evaluation, and where they are dear and free it runs at two to three times
/// its charge (two stored fuzz inputs did, 623 and 872 instructions a unit). So every corner from
/// 2⁴⁴ apart ([`NEAR_AXIS`]) is charged as dear unless its quotient is exact, which a fast path
/// or the accurate path's own shortcut answers: the three binades where more than one corner in
/// ten is dear. There the charge is in vain for the others, 36%, 69% and 86% of them.
///
/// What is still unpriced: the dear corners below 2⁴⁴ apart near the positive x-axis, fewer than
/// one in twelve and half as many a binade; and, since that bound also grows with the cube of the
/// angle, about one corner in 2,000 some 2⁷ apart there (2,044 of 4,000,000), fewer with each
/// binade and none found from 2¹³ apart, and one in 100,000 anywhere else (337 of 40,000,000).
/// Pricing those too needs the arctangent to say which path it took.
fn dear(y: f64, x: f64) -> bool {
    let (ay, ax) = (y.abs(), x.abs());
    if ay == 0.0 || ax == 0.0 {
        return false;
    }
    if x > 0.0 && ay * NEAR_AXIS <= ax {
        // Both products are exact, or infinite where the real product passes every double.
        return ax <= ay * TINY_QUOTIENT && !exact_quotient(y, x);
    }
    let (bx, by) = (ax.to_bits(), ay.to_bits());
    let apart = bx.abs_diff(by);
    let first_gate = if bx >= by {
        apart >= FIRST_GATE
    } else {
        apart > FIRST_GATE
    };
    first_gate && y / x != 0.0
}

/// Whether the quotient y/x (x > 0, |y/x| ≤ 2⁻⁴⁴) is exactly a double, by the arctangent's own
/// test: t = y/x rounded, and t·x − y by a fused multiply-add, zero only where t is the quotient;
/// for |y| below 2⁻⁹¹⁵, where that remainder may underflow, both sides scaled by 2¹⁰⁵.
fn exact_quotient(y: f64, x: f64) -> bool {
    let t = y / x;
    let zero = |a: f64, b: f64, c: f64| math::fma(a, b, c).is_ok_and(|r| r.get() == 0.0);
    zero(t, x, -y) && (y.abs() >= SMALL_Y || zero(t * SCALE, x, -y * SCALE))
}

/// The certified angle atan2(y, x) of every point of the box, with the meter charged
/// [`FAR_CORNER`] units for each corner that is [`dear`].
///
/// The charge is a function of the box alone (chapter 23.10: a count no shortcut can change): the
/// four corners `Interval::atan2` takes the angle of, less the boxes it answers without them (one
/// that holds the negative x-axis and points below it, whose angles are all of [−π, π]).
fn angle(y: Interval, x: Interval, meter: &mut WorkMeter<'_>) -> Result<Interval, CurveError> {
    let all_round = x.lo() < 0.0 && y.lo() < 0.0 && y.hi() >= 0.0;
    if !all_round {
        for (cy, cx) in [
            (y.lo(), x.lo()),
            (y.lo(), x.hi()),
            (y.hi(), x.lo()),
            (y.hi(), x.hi()),
        ] {
            // One charge a dear corner, and none where no corner is dear: for every such extent
            // the meter's count of charges (and with it where it reads a cancellation) is as it
            // was.
            if dear(cy, cx) {
                meter.charge(FAR_CORNER)?;
            }
        }
    }
    Ok(y.atan2(x)?)
}

/// φ = atan2(b, a)/π, in half-turns, for (a, b) not both zero.
fn phase(a: Interval, b: Interval, meter: &mut WorkMeter<'_>) -> Result<Interval, CurveError> {
    let radians = angle(b, a, meter)?;
    if is_zero(radians) {
        // On the positive x-axis: exactly 0, not 0/π widened.
        return Ok(radians);
    }
    tight::div(radians, form::pi()?)
}

/// cosh t and sinh t over `t`.
fn cosh_sinh(t: Interval) -> Result<(Interval, Interval), CurveError> {
    let (e, ei) = (t.exp()?, t.neg().exp()?);
    let half = point(0.5)?;
    Ok((
        tight::mul(tight::add(e, ei)?, half)?,
        tight::mul(tight::sub(e, ei)?, half)?,
    ))
}

impl Scalar {
    /// The constant function `p`.
    ///
    /// # Errors
    ///
    /// None in practice: building the zero interval cannot fail.
    pub fn constant(p: Interval) -> Result<Scalar, CurveError> {
        let zero = point(0.0)?;
        Ok(Scalar::Wave {
            p,
            q: zero,
            a: zero,
            b: zero,
        })
    }

    /// g at every parameter of `t`: its natural interval extension.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`] if a value overflows.
    pub fn over(&self, t: Interval) -> Result<Interval, CurveError> {
        match *self {
            Scalar::Wave { p, q, a, b } => {
                let s = tight::add(p, times(q, || Ok(t))?)?;
                let s = tight::add(s, times(a, || Ok(t.cospi()?))?)?;
                tight::add(s, times(b, || Ok(t.sinpi()?))?)
            }
            Scalar::Quadratic { p, q, r } => {
                let s = tight::add(p, times(q, || Ok(t))?)?;
                tight::add(s, times(r, || tight::square(t))?)
            }
            Scalar::Hyperbolic { p, a, b } => {
                let (ch, sh) = cosh_sinh(t)?;
                let s = tight::add(p, times(a, || Ok(ch))?)?;
                tight::add(s, times(b, || Ok(sh))?)
            }
            Scalar::Involute { p, a, b } => {
                let (c, s) = (t.cospi()?, t.sinpi()?);
                let theta = tight::mul(form::pi()?, t)?;
                let sum = tight::add(p, times(a, || tight::add(c, tight::mul(theta, s)?))?)?;
                tight::add(sum, times(b, || tight::sub(s, tight::mul(theta, c)?))?)
            }
            Scalar::Modulated { p, a, b, c, d } => {
                let cos = tight::add(a, times(c, || Ok(t))?)?;
                let sin = tight::add(b, times(d, || Ok(t))?)?;
                let s = tight::add(p, times(cos, || Ok(t.cospi()?))?)?;
                tight::add(s, times(sin, || Ok(t.sinpi()?))?)
            }
        }
    }

    /// g′ at every parameter of `t`, enclosed: the pieces' monotonicity test.
    fn slope(&self, t: Interval) -> Result<Interval, CurveError> {
        let pi = form::pi()?;
        match *self {
            // q + π·(b·cos πt − a·sin πt).
            Scalar::Wave { q, a, b, .. } => {
                let w = tight::sub(tight::mul(b, t.cospi()?)?, tight::mul(a, t.sinpi()?)?)?;
                tight::add(q, tight::mul(pi, w)?)
            }
            // q + 2r·t.
            Scalar::Quadratic { q, r, .. } => {
                tight::add(q, tight::mul(tight::mul(point(2.0)?, r)?, t)?)
            }
            // a·sinh t + b·cosh t.
            Scalar::Hyperbolic { a, b, .. } => {
                let (ch, sh) = cosh_sinh(t)?;
                tight::add(tight::mul(a, sh)?, tight::mul(b, ch)?)
            }
            // π²·t·(a·cos πt + b·sin πt).
            Scalar::Involute { a, b, .. } => {
                let w = tight::add(tight::mul(a, t.cospi()?)?, tight::mul(b, t.sinpi()?)?)?;
                tight::mul(tight::mul(tight::square(pi)?, t)?, w)
            }
            // (c + π·b + π·d·t)·cos πt + (d − π·a − π·c·t)·sin πt.
            Scalar::Modulated { a, b, c, d, .. } => {
                let pb = tight::mul(pi, tight::add(b, tight::mul(d, t)?)?)?;
                let pa = tight::mul(pi, tight::add(a, tight::mul(c, t)?)?)?;
                let cos = tight::add(c, pb)?;
                let sin = tight::sub(d, pa)?;
                tight::add(tight::mul(cos, t.cospi()?)?, tight::mul(sin, t.sinpi()?)?)
            }
        }
    }

    /// The critical points in closed form (module documentation's table).
    fn critical(&self, meter: &mut WorkMeter<'_>) -> Result<Critical, CurveError> {
        let one = point(1.0)?;
        match *self {
            Scalar::Wave { q, a, b, .. } => wave_critical(q, a, b, meter),
            Scalar::Quadratic { q, r, .. } => {
                if is_zero(r) {
                    Ok(Critical::Families(Vec::new()))
                } else if excludes_zero(r) {
                    let t = tight::div(q.neg(), tight::mul(point(2.0)?, r)?)?;
                    Ok(Critical::Families(vec![(t, false)]))
                } else {
                    Ok(Critical::Undecided)
                }
            }
            Scalar::Hyperbolic { a, b, .. } => {
                let s = tight::add(a, b)?;
                if is_zero(s) || (is_zero(a) && is_zero(b)) {
                    // a·e^{−t} (or zero): monotone.
                    return Ok(Critical::Families(Vec::new()));
                }
                if !excludes_zero(s) {
                    return Ok(Critical::Undecided);
                }
                // tanh t = −b/a, so e^{2t} = (a − b)/(a + b).
                let z = tight::div(tight::sub(a, b)?, s)?;
                if z.hi() <= 0.0 {
                    Ok(Critical::Families(Vec::new()))
                } else if z.lo() > 0.0 {
                    let t = tight::mul(z.ln()?, point(0.5)?)?;
                    Ok(Critical::Families(vec![(t, false)]))
                } else {
                    Ok(Critical::Undecided)
                }
            }
            Scalar::Involute { a, b, .. } => {
                if is_zero(a) && is_zero(b) {
                    return Ok(Critical::Families(Vec::new()));
                }
                // g′ = π²·t·R·cos(π(t − φ)).
                let phi = tight::add(phase(a, b, meter)?, point(0.5)?)?;
                Ok(Critical::Families(vec![
                    (point(0.0)?, false),
                    (phi, true),
                    (tight::add(phi, one)?, true),
                ]))
            }
            // With no t in its coefficients it is a wave: p + a·cos πt + b·sin πt.
            Scalar::Modulated { a, b, c, d, .. } if is_zero(c) && is_zero(d) => {
                wave_critical(point(0.0)?, a, b, meter)
            }
            Scalar::Modulated { .. } => Ok(Critical::Undecided),
        }
    }
}

/// The critical points of p + q·t + a·cos πt + b·sin πt (module documentation's table).
fn wave_critical(
    q: Interval,
    a: Interval,
    b: Interval,
    meter: &mut WorkMeter<'_>,
) -> Result<Critical, CurveError> {
    let one = point(1.0)?;
    if is_zero(a) && is_zero(b) {
        // p + q·t: monotone.
        return Ok(Critical::Families(Vec::new()));
    }
    let phi = phase(a, b, meter)?;
    if is_zero(q) {
        // Maxima at φ + 2m, minima at φ + 1 + 2m.
        return Ok(Critical::Families(vec![
            (phi, true),
            (tight::add(phi, one)?, true),
        ]));
    }
    let r = tight::norm(&[a, b])?;
    if r.lo() <= 0.0 {
        return Ok(Critical::Undecided);
    }
    // g′ = q − πR·sin(π(t − φ)), zero where sin(π(t − φ)) = ρ.
    let rho = tight::div(q, tight::mul(form::pi()?, r)?)?;
    if rho.lo() >= 1.0 || rho.hi() <= -1.0 {
        // g′ keeps the sign of q, zero at most at isolated points: monotone.
        return Ok(Critical::Families(Vec::new()));
    }
    // Where ρ's enclosure reaches ±1, ρ² reaches 1 and 1 − ρ² reaches 0: undecided.
    let cos2 = tight::sub(one, tight::square(rho)?)?;
    if cos2.lo() <= 0.0 {
        return Ok(Critical::Undecided);
    }
    // ψ = asin ρ / π, in [−½, ½]: the angle whose sine is ρ and cosine √(1 − ρ²).
    let psi = tight::div(angle(rho, tight::sqrt(cos2)?, meter)?, form::pi()?)?;
    Ok(Critical::Families(vec![
        (tight::add(phi, psi)?, true),
        (tight::sub(tight::add(phi, one)?, psi)?, true),
    ]))
}

/// `base` + 2m, enclosed.
fn member(base: Interval, m: f64) -> Result<Interval, CurveError> {
    tight::add(base, point(2.0 * m)?)
}

/// The enclosure cut down to the span (it meets it).
fn clip(e: Interval, (t0, t1): (f64, f64)) -> Result<Interval, CurveError> {
    Ok(Interval::new(
        tight::larger(e.lo(), t0),
        tight::smaller(e.hi(), t1),
    )?)
}

/// The first member of a family whose enclosure reaches `t0`: the estimate (t₀ − c)/2, which
/// rounding moves by less than one, so a scan up from two below it starts below the answer and
/// meets it within [`SCAN`] steps; `None` if it does not.
fn first_reaching(base: Interval, t0: f64) -> Result<Option<f64>, CurveError> {
    let mut m = math::ceil((t0 - base.hi()) * 0.5)?.get() - 2.0;
    for _ in 0..SCAN {
        if member(base, m)?.hi() >= t0 {
            return Ok(Some(m));
        }
        m += 1.0;
    }
    Ok(None)
}

/// The last member of a family whose enclosure does not pass `t1`: a scan down from two above
/// the estimate, as [`first_reaching`].
fn last_within(base: Interval, t1: f64) -> Result<Option<f64>, CurveError> {
    let mut m = math::floor((t1 - base.lo()) * 0.5)?.get() + 2.0;
    for _ in 0..SCAN {
        if member(base, m)?.lo() <= t1 {
            return Ok(Some(m));
        }
        m -= 1.0;
    }
    Ok(None)
}

/// The members of a family to evaluate, clipped to the span (module documentation), or `None` if
/// they cannot be placed: a family as wide as half a period (coefficients enclosing the origin).
/// The bases are phases within [−2, 3] and the span is inside ±2⁵⁰ ([`extent`]), so 2m is exact.
fn members(
    (base, periodic): (Interval, bool),
    (t0, t1): (f64, f64),
) -> Result<Option<Vec<Interval>>, CurveError> {
    if !periodic {
        return Ok(Some(if base.hi() >= t0 && base.lo() <= t1 {
            vec![clip(base, (t0, t1))?]
        } else {
            Vec::new()
        }));
    }
    if base.hi() - base.lo() >= 1.0 {
        return Ok(None);
    }
    let (Some(first), Some(last)) = (first_reaching(base, t0)?, last_within(base, t1)?) else {
        return Ok(None);
    };
    let mut out = Vec::with_capacity(4);
    let mut previous = f64::NEG_INFINITY;
    for m in [first, first + 1.0, last - 1.0, last] {
        if first <= m && m <= last && m > previous {
            out.push(clip(member(base, m)?, (t0, t1))?);
            previous = m;
        }
    }
    Ok(Some(out))
}

/// The pieces' route (module documentation): g over [t0, t1] by monotone pieces.
fn pieces(
    g: &Scalar,
    (t0, t1): (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Interval, CurveError> {
    let n = math::ceil((t1 - t0) * 4.0)?.get();
    let count = if n >= f64::from(MAX_PIECES) {
        MAX_PIECES
    } else if n >= 1.0 {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "n is a whole number in [1, 64)"
        )]
        let m = n as u32;
        m
    } else {
        1
    };
    let mut out = g.over(point(t0)?)?;
    for (s0, s1) in cut((t0, t1), count) {
        meter.charge(1)?;
        let d = g.slope(Interval::new(s0, s1)?)?;
        if excludes_zero(d) {
            out = hull(out, hull(g.over(point(s0)?)?, g.over(point(s1)?)?)?)?;
            continue;
        }
        meter.charge(u64::from(SUBPIECES))?;
        for (r0, r1) in cut((s0, s1), SUBPIECES) {
            let piece = Interval::new(r0, r1)?;
            let slope = g.slope(piece)?;
            let range = if excludes_zero(slope) {
                hull(g.over(point(r0)?)?, g.over(point(r1)?)?)?
            } else {
                // Both the interval extension and the mean-value form g(m) + g′·[−r, r] hold g
                // over the piece; near a critical point g′ is small, so the second is far
                // tighter, and their intersection holds g too.
                let m = f64::midpoint(r0, r1);
                let r = tight::larger(
                    tight::sub(point(m)?, point(r0)?)?.hi(),
                    tight::sub(point(r1)?, point(m)?)?.hi(),
                );
                let spread = tight::mul(slope, Interval::new(-r, r)?)?;
                let mean = tight::add(g.over(point(m)?)?, spread)?;
                let natural = g.over(piece)?;
                Interval::new(
                    tight::larger(mean.lo(), natural.lo()),
                    tight::smaller(mean.hi(), natural.hi()),
                )?
            };
            out = hull(out, range)?;
        }
    }
    Ok(out)
}

/// [lo, hi] cut into `n` pieces that join exactly and stay inside it.
fn cut((lo, hi): (f64, f64), n: u32) -> Vec<(f64, f64)> {
    let width = hi - lo;
    let count = f64::from(n);
    let mut out = Vec::new();
    let mut start = lo;
    for i in 1..=n {
        let end = if i == n {
            hi
        } else {
            lo + width * (f64::from(i) / count)
        };
        // Rounding may step past either end; the piece stays in [start, hi].
        let end = tight::larger(tight::smaller(end, hi), start);
        out.push((start, end));
        start = end;
    }
    out
}

/// The range of `g` over the span [t0, t1], enclosed (module documentation).
///
/// # Errors
///
/// [`CurveError::SpanOutside`] unless t0 ≤ t1, both finite; [`CurveError::NotFinite`] if a value
/// overflows; [`CurveError::Stopped`] by the meter.
pub fn extent(
    g: &Scalar,
    (t0, t1): (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Interval, CurveError> {
    if !(t0.is_finite() && t1.is_finite() && t0 <= t1) {
        return Err(CurveError::SpanOutside);
    }
    meter.charge(1)?;
    if t0.abs() >= FAR || t1.abs() >= FAR {
        return pieces(g, (t0, t1), meter);
    }
    let Critical::Families(families) = g.critical(meter)? else {
        return pieces(g, (t0, t1), meter);
    };
    let mut out = hull(g.over(point(t0)?)?, g.over(point(t1)?)?)?;
    for family in families {
        let Some(list) = members(family, (t0, t1))? else {
            return pieces(g, (t0, t1), meter);
        };
        for e in list {
            out = hull(out, g.over(e)?)?;
        }
    }
    Ok(out)
}
