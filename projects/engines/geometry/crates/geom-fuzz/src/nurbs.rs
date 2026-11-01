//! Fuzz target: NURBS curves keep their stated bounds against exact rational evaluation on any
//! bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a NURBS curve in two or three
//! dimensions (degree 1 to 4, a clamped or uniform knot vector with repeated knots, rational or
//! not), a parameter, a knot to insert and the curve's control points again as data to
//! interpolate. [`check`] builds it with `geom-nurbs` and holds four laws against
//! `geom_test::nurbs_reference`, which computes the same curves in exact rationals:
//!
//! - **A point lies within its stated bound** of the exact point, coordinate by coordinate:
//!   `γ_(7p+1)`·Σ `N_j`·|`P_j`|, or `γ_(14p+3)`·Σ `N_j`·|`w_j·P_j`| / Σ `N_j·w_j` for a rational
//!   curve (`geom_nurbs::curve`'s header; piece G56).
//! - **Knot insertion keeps the curve within its stated deviation**, at the parameter (G56).
//! - **A Bezier piece's enclosure holds the exact homogeneous point** (`Curve::bernstein` and
//!   `BernsteinCurve::value_at`, piece G59).
//! - **An interpolating curve is within its certified error of its data** at each data
//!   parameter (`Curve::interpolate`, piece G58).
//!
//! [`harness`] is `decode` then `check`, the function every fuzzing engine drives.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, through the rule bytes of the `curves` target: 0, ±1, whole
//!   numbers and quarters on a coarse grid, tiny and huge powers of two, or a double read from the
//!   input. Knot spans and weights are drawn positive; a curve or a fit `geom-nurbs` refuses is
//!   legal.
//! - **What is a finding.** A law above broken, decided exactly, or any call failing as
//!   `NurbsError::Defect`; typed refusals (an overflow, a singular fit, a budget stop) are legal.
//! - **No hang.** Every call runs under a work meter of [`WORK`] units.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below.
//!
//! Method: the bounds `geom-nurbs` states, checked exactly against the reference implementation
//! of chapter 28.9 (`projects/engines/geometry/architecture/28-testing.md`), as piece G69's law
//! catalogue does, driven here by coverage-guided fuzzing (28.5).
//!
//! Cost: one curve, one insertion, one Bernstein decomposition and one interpolation of at most
//! [`MAX_POINTS`] points per input, each bounded by [`WORK`] work units, and their exact
//! evaluations; at most [`MAX_INPUT_BYTES`] bytes read.

// The lint is for fixed-width arithmetic, which can overflow. The oracle's arithmetic is on
// dashu's arbitrary-precision rationals, which cannot; the doubles' arithmetic is IEEE's.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "exact rational arithmetic in the oracle, which cannot overflow"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': c the curve, s its reference, a and b its domain, t and u parameters, p the degree, q a rational"
)]

use dashu_ratio::RBig;
use geom_nurbs::{Basis, Curve, FitVersion, Limit, NurbsError, Parametrization};
use geom_run::{Budget, WorkMeter};
use geom_test::nurbs_reference::Spline;
use geom_test::{ByteSource, Entropy};

use crate::Finding;
use crate::curves::value;

/// The most control points a case has.
pub const MAX_POINTS: usize = 8;

/// The work units each call may spend.
pub const WORK: u64 = 2_000_000;

/// The most bytes [`decode`] reads from any input: 306.
///
/// Five header bytes (the dimension, the degree, the count, whether it is rational and whether
/// it is clamped), [`MAX_POINTS`] − 1 + 4 knot-step bytes, then for each of [`MAX_POINTS`] points
/// three coordinates and a weight, each a rule byte and at most eight bytes more, then two
/// position bytes: 5 + 11 + 8 · 4 · 9 + 2 = 306. The unit test
/// `the_input_bound_is_reached_and_never_passed` builds an input that reads exactly this many.
pub const MAX_INPUT_BYTES: usize = 5 + (MAX_POINTS + 3) + MAX_POINTS * 4 * 9 + 2;

/// One input.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// Whether the curve is in three dimensions.
    pub space: bool,
    /// The degree, 1 to 4.
    pub degree: usize,
    /// The number of control points, degree + 1 to [`MAX_POINTS`].
    pub count: usize,
    /// Whether the curve is rational.
    pub rational: bool,
    /// Whether the knot vector is clamped.
    pub clamped: bool,
    /// Each knot step: 0 repeats a knot, else a step of k/4.
    pub steps: [u8; MAX_POINTS + 3],
    /// The control points (the third coordinate unused in 2D).
    pub points: [[f64; 3]; MAX_POINTS],
    /// The weights' doubles (their magnitudes are used, 1 for 0).
    pub weights: [f64; MAX_POINTS],
    /// Where the parameter and the inserted knot sit in the domain, as k/255.
    pub at: [u8; 2],
}

/// Any bytes as a case, and how many bytes were read.
#[must_use]
pub fn decode(bytes: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(bytes);
    let space = src.byte() & 1 == 1;
    let degree = usize::from(src.byte() & 3).saturating_add(1);
    let extra = usize::from(src.byte())
        .checked_rem(MAX_POINTS.saturating_sub(degree))
        .unwrap_or(0);
    let count = degree
        .saturating_add(1)
        .saturating_add(extra)
        .min(MAX_POINTS);
    let rational = src.byte() & 1 == 1;
    let clamped = src.byte() & 3 != 0;
    let mut steps = [0u8; MAX_POINTS + 3];
    for s in &mut steps {
        *s = src.byte();
    }
    let mut points = [[0.0; 3]; MAX_POINTS];
    let mut weights = [1.0; MAX_POINTS];
    for (p, w) in points.iter_mut().zip(&mut weights) {
        for x in p.iter_mut() {
            *x = value(&mut src);
        }
        *w = value(&mut src);
    }
    let at = [src.byte(), src.byte()];
    let read = bytes.len().saturating_sub(src.remaining());
    (
        Case {
            space,
            degree,
            count,
            rational,
            clamped,
            steps,
            points,
            weights,
            at,
        },
        read,
    )
}

/// The case's knot vector: clamped (p + 1 equal ends) or not. Each step byte k moves the next
/// knot on by (k >> 4)/4, at least a quarter; in a clamped vector a step byte whose low half is 0
/// repeats the knot instead, at most p times.
fn knots(case: &Case) -> Vec<f64> {
    let p = case.degree;
    let mut bytes = case.steps.iter().copied();
    let mut next = || bytes.next().unwrap_or(0x40);
    let step = |b: u8| 0.25 * f64::from((b >> 4).max(1));
    if !case.clamped {
        let mut x = 0.0;
        return (0..=(case.count + p))
            .map(|_| {
                let k = x;
                x += step(next());
                k
            })
            .collect();
    }
    let mut k = vec![0.0; p + 1];
    // The first knot is already repeated p + 1 times, so the first interior one steps on.
    let (mut x, mut run) = (0.0, p + 1);
    for _ in 0..case.count.saturating_sub(p + 1) {
        let b = next();
        if b.trailing_zeros() >= 4 && run < p {
            run += 1;
        } else {
            x += step(b);
            run = 1;
        }
        k.push(x);
    }
    let end = x + step(next());
    k.extend(core::iter::repeat_n(end, p + 1));
    k
}

/// x exactly.
fn q(x: f64) -> Option<RBig> {
    RBig::try_from(x).ok()
}

/// |x|.
fn abs(x: &RBig) -> RBig {
    if *x < RBig::ZERO {
        -x.clone()
    } else {
        x.clone()
    }
}

/// `γ_n` = n·u/(1 − n·u), u = 2^−53, exactly.
fn gamma(n: usize) -> Option<RBig> {
    let nu = q(f64::from(u32::try_from(n).ok()?))? / q(9_007_199_254_740_992.0)?;
    Some(&nu / (RBig::ONE - &nu))
}

/// The reference spline of a kernel curve: its stored homogeneous points and weights, exactly.
fn spline<const D: usize>(c: &Curve<D>) -> Option<Spline> {
    let b = c.basis();
    let pts: Vec<&[f64]> = c.homogeneous().iter().map(<[f64; D]>::as_slice).collect();
    match c.weights() {
        Some(w) => Spline::new_rational(b.degree(), b.knots(), &pts, w).ok(),
        None => Spline::new(b.degree(), b.knots(), &pts).ok(),
    }
}

/// A finding for a `geom-nurbs` call that failed as a defect; `None` for any other result.
fn defect<T>(what: &str, r: &Result<T, NurbsError>) -> Option<Finding> {
    matches!(r, Err(NurbsError::Defect))
        .then(|| Finding::new("no call fails as a defect", || what.to_owned()))
}

/// The laws on the case, with `shift` added to every evaluated point's first coordinate: 0 for
/// [`check`], and a broken evaluation for tests to show the point law catches it.
#[allow(
    clippy::too_many_lines,
    reason = "the four laws one after another, each a few lines"
)]
fn laws<const D: usize>(case: &Case, shift: f64) -> Result<(), Finding> {
    let Ok(basis) = Basis::new(case.degree, knots(case)) else {
        return Ok(());
    };
    let pts: Vec<[f64; D]> = case
        .points
        .iter()
        .take(case.count)
        .map(|p| core::array::from_fn(|i| p.get(i).copied().unwrap_or(0.0)))
        .collect();
    let built = if case.rational {
        let w: Vec<f64> = case
            .weights
            .iter()
            .take(case.count)
            // Weights in the normal range, as the stated bound takes them.
            .map(|w| if w.abs().is_normal() { w.abs() } else { 1.0 })
            .collect();
        Curve::new_rational(basis, &pts, &w)
    } else {
        Curve::new(basis, &pts)
    };
    let Ok(c) = built else { return Ok(()) };
    let Some(s) = spline(&c) else { return Ok(()) };
    let knots = c.basis().knots();
    let (Some(&a), Some(&b)) = (knots.get(c.degree()), knots.get(case.count)) else {
        return Ok(());
    };
    let [ti, ui] = case.at;
    let at = |k: u8| (a + (b - a) * (f64::from(k) / 255.0)).clamp(a, b);
    let (t, u) = (at(ti), at(ui));
    let Some(tq) = q(t) else { return Ok(()) };
    let mut meter = WorkMeter::new(Budget::of_work(WORK));

    // The point within its stated bound.
    if let (Ok(mut got), Ok(exact), Ok(mag)) = (
        c.point(t, Limit::FromRight),
        s.point(&tq),
        s.magnitudes().homogeneous_point(&tq),
    ) {
        if let Some(x) = got.first_mut() {
            *x += shift;
        }
        let p = c.degree();
        let n = if c.is_rational() {
            14 * p + 3
        } else {
            7 * p + 1
        };
        let g = gamma(n);
        // Gradual underflow's absolute part, n·2^−1074 (`geom_nurbs::curve`, "Underflow").
        let tiny = q(f64::from_bits(1)).map(|t| t * RBig::from(n));
        for d in 0..D {
            let (Some(g), Some(gd), Some(ed), Some(md)) = (
                g.as_ref(),
                got.get(d).and_then(|&x| q(x)),
                exact.get(d),
                mag.get(d),
            ) else {
                continue;
            };
            let Some(tiny) = tiny.as_ref() else { continue };
            let bound = if c.is_rational() {
                match mag.get(D) {
                    Some(w) if *w > RBig::ZERO => (g * md + tiny) / w + tiny,
                    _ => continue,
                }
            } else {
                g * md + tiny
            };
            if abs(&(gd - ed)) > bound {
                return Err(Finding::new(
                    "a point lies within its stated bound of the exact point",
                    || format!("{c:?} at {t}, coordinate {d}: {got:?}"),
                ));
            }
        }
    }

    // Knot insertion keeps the curve within its stated deviation.
    let inserted = c.insert_knot(u, 1, &mut meter);
    if let Some(f) = defect("insert_knot", &inserted) {
        return Err(f);
    }
    if let Ok(r) = inserted
        && let (Some(new), Ok(old)) = (spline(&r.value), s.point(&tq))
        && let (Ok(newp), Some(dev)) = (new.point(&tq), q(r.deviation))
    {
        let dist: RBig = old
            .iter()
            .zip(&newp)
            .fold(RBig::ZERO, |acc, (x, y)| acc + (x - y) * (x - y));
        if dist > &dev * &dev {
            return Err(Finding::new(
                "knot insertion keeps the curve within its stated deviation",
                || format!("{c:?}, knot {u}, at {t}: deviation {}", r.deviation),
            ));
        }
    }

    // A Bezier piece's enclosure holds the exact homogeneous point.
    if c.basis().is_clamped() {
        let pieces = c.bernstein(&mut meter);
        if let Some(f) = defect("bernstein", &pieces) {
            return Err(f);
        }
        if let (Ok(pieces), Ok(h)) = (pieces, s.homogeneous_point(&tq)) {
            for piece in &pieces {
                let (lo, hi) = piece.domain();
                if !(lo <= t && t <= hi) {
                    continue;
                }
                let Ok((enc, w)) = piece.value_at(t) else {
                    continue;
                };
                let holds = |i: geom_num::interval::Interval, x: &RBig| matches!((q(i.lo()), q(i.hi())), (Some(l), Some(u)) if l <= *x && *x <= u);
                let coords = enc.iter().zip(&h).all(|(i, x)| holds(*i, x));
                let weight = !c.is_rational() || h.get(D).is_some_and(|x| holds(w, x));
                if !(coords && weight) {
                    return Err(Finding::new(
                        "a Bezier piece's enclosure holds the exact point",
                        || format!("{c:?} at {t}: {enc:?}, {w:?}"),
                    ));
                }
            }
        }
    }

    // An interpolating curve is within its certified error of its data.
    let fit = Curve::<D>::interpolate(
        &pts,
        c.degree(),
        Parametrization::ChordLength,
        FitVersion::V1,
        &mut meter,
    );
    if let Some(f) = defect("interpolate", &fit) {
        return Err(f);
    }
    if let Ok(fit) = fit
        && let (Some(fs), Some(err)) = (spline(&fit.curve), q(fit.error))
    {
        for (p, &ur) in pts.iter().zip(&fit.parameters) {
            let Some(uq) = q(ur) else { continue };
            let Ok(on) = fs.point(&uq) else { continue };
            let dist = on
                .iter()
                .zip(p)
                .filter_map(|(x, &y)| q(y).map(|y| (x - &y) * (x - &y)))
                .fold(RBig::ZERO, |acc, d| acc + d);
            if dist > &err * &err {
                return Err(Finding::new(
                    "an interpolating curve is within its certified error of its data",
                    || format!("{fit:?} at {ur}: data {p:?}"),
                ));
            }
        }
    }
    Ok(())
}

/// The four laws on the case (module documentation).
///
/// # Errors
///
/// A [`Finding`] for the first law broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    check_with(case, 0.0)
}

/// [`check`] with every evaluated point's first coordinate moved by `shift`: a broken
/// evaluation, so tests can show the point law catches it (28.4, fault injection).
///
/// # Errors
///
/// As [`check`].
pub fn check_with(case: &Case, shift: f64) -> Result<(), Finding> {
    if case.space {
        laws::<3>(case, shift)
    } else {
        laws::<2>(case, shift)
    }
}

/// [`decode`], then [`check`].
///
/// # Errors
///
/// As [`check`].
pub fn harness(bytes: &[u8]) -> Result<(), Finding> {
    check(&decode(bytes).0)
}
