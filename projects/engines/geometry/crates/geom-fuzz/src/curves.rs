//! Fuzz target: every curve kind keeps the kind contract on any bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a curve kind (one of the 15
//! of `geom_curve::CurveKind`, in two or three dimensions as the kind allows), the doubles it is
//! built from, up to four parameters in its domain and a typed transform. [`check`] builds the
//! kind and runs `geom_curve::contract::check` on it, the kind contract kit every kind is held to
//! (piece G60, with G68's derivative laws), under a bounded work meter; when that check ends on
//! the budget, it runs `geom_curve::contract::check_without_conversions` under a meter of its own,
//! so the laws that need no conversion still run on a kind whose transform or NURBS form the
//! budget could not pay for. [`harness`] is `decode` then `check`, the function every fuzzing
//! engine drives (the crate README sets out the layout). An offset curve (tag [`OFFSET`]) is
//! passed without being built: offsets cost most of what this target's inputs charge, so they
//! have a target of their own, `crate::curve_offsets`, which reads the same bytes with the kind
//! made an offset and runs this module's code on it. It reaches pieces G60 to G64, G67 and G68
//! (Gate 2, 32.46).
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case. Each
//!   double is read through a rule byte that makes the hard values common: 0, ±1, a whole number
//!   or a short binary fraction on a coarse grid (so coincident and parallel data come up), a
//!   tiny or huge power of two, or a double read from the input (non-finite ones replaced by 0).
//! - **What is a finding.** A law of the contract broken with no error behind it, or any call
//!   failing as `CurveError::Defect` (an internal step the kernel says cannot fail), is a
//!   [`Finding`], in the check or in the pass after a budget stop; a kind that refuses to be
//!   built, or a call that refuses with a typed error (an overflow at an extreme size, a budget
//!   stop), is legal. A panic fails every engine.
//! - **No hang.** Every call runs under a work meter of [`WORK`] units, and the pass after a stop
//!   under one of [`PARTIAL_WORK`]; running out is a typed stop, which is legal.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a
//!   unit test builds the input that reaches it.
//!
//! Method: the kind contract of chapter 7 as `geom_curve::contract` states it, driven by
//! coverage-guided fuzzing (chapter 28.5), with inputs aimed at the hard cases 28.5 lists
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: one kind built and one contract check per input, each bounded by [`WORK`] work units,
//! and after a budget stop one pass bounded by [`PARTIAL_WORK`]; at most [`MAX_INPUT_BYTES`]
//! bytes read.

use geom_curve::offset::{Offset, OffsetLaw};
use geom_curve::on_surface::{OnSurface, SurfaceRef};
use geom_curve::trimmed::Trimmed;
use geom_curve::{
    Axes, Basis, BulgePolyline, Circle, Composite, ContractError, CurvatureLaw, CurveError,
    CurveKind, Ellipse, Frame, Helix, HelixAxes, Hyperbola, Involute, Line, Parabola, ParamCurve,
    Polynomial, Transition, contract,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_run::{Budget, Stop, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The doubles a case reads.
pub const VALUES: usize = 24;

/// The parameters a case reads.
pub const PARAMS: usize = 4;

/// The kind tag of `CurveKind::Offset`, which this target passes and `crate::curve_offsets`
/// fuzzes.
pub const OFFSET: u8 = 12;

/// The work units each call may spend: 2,000,000.
///
/// Where it comes from. It was chosen when the target was written (PR #453), with no derivation
/// stated, and is kept for what was measured since. On 3,856 stored and campaign inputs, each run
/// once without a budget on 2026-11-13, the work a whole check charges has a 95th percentile of
/// 6,958,418 units; seven to nine times that, the headroom `crate::sequence::WORK` was first set
/// with, would be about 63 million units, 15 to 20 s a case on a hosted runner, past the 10-second
/// limit on one input. So the limit, not the corpus, bounds this budget. Nearly all that work is
/// the offsets', which have a target of their own. Of the fourteen kinds this target builds, 41
/// inputs stop at this budget and 36 still stop at eight times it (helices and involutes over
/// long domains, and one transition), and no input of the other eleven kinds charges more than
/// 23,281 units: a larger budget would buy these kinds almost nothing. A stop is followed by the
/// pass of [`PARTIAL_WORK`].
pub const WORK: u64 = 2_000_000;

/// The work units of the pass without conversions after a budget stop: 250,000.
///
/// Measured on the offsets, the kind that stops most: run without a budget on the inputs of
/// [`WORK`], the laws that need no conversion charge a whole offset contract 41,532 units at the
/// median and 177,814 at the most, so 250,000 holds the dearest seen, and a stopped case costs at
/// most an eighth more than [`WORK`]. A law broken in the pass is a finding; a stop in it is legal.
pub const PARTIAL_WORK: u64 = 250_000;

/// The most bytes [`decode`] reads from any input: 223.
///
/// Three header bytes (the kind, the dimension and the transform), then [`VALUES`] doubles of a
/// rule byte and, for a double read from the input, eight bytes more, and [`PARAMS`] parameter
/// bytes: 3 + 24 · (1 + 8) + 4 = 223. The unit test `the_input_bound_is_reached_and_never_passed`
/// builds an input that reads exactly this many.
pub const MAX_INPUT_BYTES: usize = 3 + VALUES * 9 + PARAMS;

/// The curve kinds a case may be, in `CurveKind`'s order.
pub const KINDS: u8 = 15;

/// One input: a kind's tag, the dimension asked for, the transform's tag, the doubles and the
/// parameters' positions in the domain (each a fraction k/255 of it).
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// Which kind, `0..KINDS` in `CurveKind`'s order.
    pub kind: u8,
    /// Whether the kind is built in three dimensions (where it may be).
    pub space: bool,
    /// Which transform the transform law uses.
    pub transform: u8,
    /// The doubles the kind and the transform are built from.
    pub values: [f64; VALUES],
    /// Where the parameters sit in the domain, as k/255.
    pub params: [u8; PARAMS],
}

/// One double through its rule byte.
pub(crate) fn value(src: &mut ByteSource<'_>) -> f64 {
    let rule = src.byte();
    let small = f64::from(rule >> 4);
    match rule & 0x0F {
        0 => 0.0,
        1 => 1.0,
        2 => -1.0,
        // A whole number, or a quarter, from the rule's high bits: a coarse grid.
        3 => small,
        4 => -small,
        5 => small * 0.25,
        6 => -small * 0.25,
        // Tiny and huge powers of two: 2^(∓4k), k from the rule's high bits, built exactly from
        // their exponent bits.
        7 => f64::from_bits(
            u64::from(1023u16.saturating_sub(u16::from(rule >> 4).saturating_mul(4))) << 52,
        ),
        8 => f64::from_bits(
            u64::from(1023u16.saturating_add(u16::from(rule >> 4).saturating_mul(4))) << 52,
        ),
        _ => {
            let x = f64::from_bits(src.word());
            if x.is_finite() { x } else { 0.0 }
        }
    }
}

/// Any bytes as a case, and how many bytes were read.
#[must_use]
pub fn decode(bytes: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(bytes);
    let kind = src.byte().checked_rem(KINDS).unwrap_or(0);
    let space = src.byte() & 1 == 1;
    let transform = src.byte();
    let mut values = [0.0; VALUES];
    for v in &mut values {
        *v = value(&mut src);
    }
    let mut params = [0u8; PARAMS];
    for p in &mut params {
        *p = src.byte();
    }
    let read = bytes.len().saturating_sub(src.remaining());
    (
        Case {
            kind,
            space,
            transform,
            values,
            params,
        },
        read,
    )
}

/// The doubles of a case, read in order.
pub(crate) struct Values<'a> {
    rest: core::slice::Iter<'a, f64>,
}

impl<'a> Values<'a> {
    /// The doubles `values`, from the first.
    pub(crate) fn of(values: &'a [f64]) -> Values<'a> {
        Values {
            rest: values.iter(),
        }
    }
    pub(crate) fn next(&mut self) -> f64 {
        self.rest.next().copied().unwrap_or(0.0)
    }
    pub(crate) fn vector<const D: usize>(&mut self) -> [f64; D] {
        core::array::from_fn(|_| self.next())
    }
    /// An ordered pair (a, b), a < b, from two doubles; (0, 1) when they are equal.
    pub(crate) fn span(&mut self) -> (f64, f64) {
        let (a, b) = (self.next(), self.next());
        if a < b {
            (a, b)
        } else if b < a {
            (b, a)
        } else {
            (0.0, 1.0)
        }
    }
    /// A positive number from a double: its magnitude, or 1 when that is 0.
    pub(crate) fn positive(&mut self) -> f64 {
        let x = self.next().abs();
        if x > 0.0 { x } else { 1.0 }
    }
}

/// The transform of tag `tag`: grid ones, an exact and a certified rotation, uniform and
/// non-uniform scales, acting in the plane for a 2D kind.
pub(crate) fn transform(tag: u8, planar: bool, v: &mut Values<'_>) -> Option<TypedTransform> {
    let axis = match (planar, tag >> 6) {
        (true, _) | (false, 2..) => Axis::Z,
        (false, 0) => Axis::X,
        (false, 1) => Axis::Y,
    };
    let z = |s: f64| if planar { 1.0 } else { s };
    Some(match tag.checked_rem(7).unwrap_or(0) {
        0 => TypedTransform::Exact(GridTransform::quarter_turns(axis, i32::from(tag >> 4))),
        1 => TypedTransform::Exact(GridTransform::mirror(axis)),
        2 => {
            let k = i64::from(tag);
            let t = [k, 0i64.saturating_sub(k), if planar { 0 } else { k }];
            TypedTransform::Exact(GridTransform::translation(t).ok()?)
        }
        3 => TypedTransform::from(
            RationalTransform::rotation(axis, Rotation::direction(3, 4).ok()?).ok()?,
        ),
        4 => TypedTransform::from(Transform::rotation(axis, v.next()).ok()?),
        5 => {
            let s = v.positive();
            TypedTransform::from(Transform::scale(s, s, z(s)).ok()?)
        }
        _ => {
            let (a, b) = (v.positive(), v.positive());
            TypedTransform::from(Transform::scale(a, b, z(a)).ok()?)
        }
    })
}

/// A frame from six doubles, or none where they are parallel or not finite.
fn frame<const D: usize>(v: &mut Values<'_>) -> Option<Frame<D>> {
    let x: [f64; D] = v.vector();
    let n: [f64; 3] = v.vector();
    let [_, _, nz] = n;
    let n = if D == 2 {
        [0.0, 0.0, if nz < 0.0 { -1.0 } else { 1.0 }]
    } else {
        n
    };
    Frame::new(x, n).ok()
}

/// A clamped NURBS curve of degree 1 to 3 through four control points, rational when asked.
fn nurbs<const D: usize>(
    v: &mut Values<'_>,
    degree: usize,
    rational: bool,
) -> Option<geom_nurbs::Curve<D>> {
    let p = degree.clamp(1, 3);
    let pts: Vec<[f64; D]> = (0..4).map(|_| v.vector()).collect();
    let inner: Vec<f64> = (0..(4usize.saturating_sub(p).saturating_sub(1)))
        .map(|_| 0.5)
        .collect();
    let mut knots = vec![0.0; p.saturating_add(1)];
    knots.extend(inner);
    knots.extend(core::iter::repeat_n(1.0, p.saturating_add(1)));
    let basis = geom_nurbs::Basis::new(p, knots).ok()?;
    if rational {
        let w: Vec<f64> = (0..4).map(|_| v.positive()).collect();
        geom_nurbs::Curve::new_rational(basis, &pts, &w).ok()
    } else {
        geom_nurbs::Curve::new(basis, &pts).ok()
    }
}

/// The kind of tag `kind` built from the case's doubles, or none where its constructor refuses.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per curve kind, in CurveKind's order"
)]
fn build<const D: usize>(kind: u8, v: &mut Values<'_>) -> Option<CurveKind<D>> {
    let c: [f64; D] = v.vector();
    Some(match kind {
        0 => CurveKind::Nurbs(nurbs(v, 3, true)?),
        1 => CurveKind::Line(Line::new(c, v.vector(), v.span()).ok()?),
        2 => CurveKind::Circle(Circle::new(c, v.positive(), frame(v)?, v.span()).ok()?),
        3 => CurveKind::Ellipse(
            Ellipse::new(
                c,
                Axes::Vectors {
                    u: v.vector(),
                    v: v.vector(),
                },
                v.span(),
            )
            .ok()?,
        ),
        4 => CurveKind::Hyperbola(
            Hyperbola::new(
                c,
                Axes::Vectors {
                    u: v.vector(),
                    v: v.vector(),
                },
                v.span(),
            )
            .ok()?,
        ),
        5 => CurveKind::Parabola(
            Parabola::new(
                c,
                Axes::Vectors {
                    u: v.vector(),
                    v: v.vector(),
                },
                v.span(),
            )
            .ok()?,
        ),
        6 => CurveKind::Helix(
            Helix::new(
                c,
                HelixAxes::Vectors {
                    u: v.vector(),
                    v: v.vector(),
                    w: v.vector(),
                },
                v.span(),
            )
            .ok()?,
        ),
        7 => {
            let coeffs: Vec<[f64; D]> = (0..4).map(|_| v.vector()).collect();
            CurveKind::Polynomial(Polynomial::new(coeffs, v.span()).ok()?)
        }
        8 => CurveKind::Involute(
            Involute::with_axes(
                c,
                Axes::Vectors {
                    u: v.vector(),
                    v: v.vector(),
                },
                v.span(),
            )
            .ok()?,
        ),
        9 => {
            let vertices: Vec<[f64; D]> = (0..3).map(|_| v.vector()).collect();
            let bulges = vec![v.next(), v.next()];
            CurveKind::Bulge(BulgePolyline::new(vertices, bulges, None, false).ok()?)
        }
        10 => {
            let circle = Circle::new(c, v.positive(), frame(v)?, (-1.0, 1.0)).ok()?;
            CurveKind::Trimmed(Trimmed::new(Basis::Circle(circle), (-0.5, 0.75)).ok()?)
        }
        11 => {
            let basis = Basis::Nurbs(nurbs(v, 2, true)?);
            let pieces = vec![
                Trimmed::new(basis.clone(), (0.0, 0.25)).ok()?,
                Trimmed::new(basis, (0.25, 1.0)).ok()?,
            ];
            CurveKind::Composite(Composite::new(pieces).ok()?)
        }
        12 => {
            let e = Ellipse::new(
                c,
                Axes::Vectors {
                    u: v.vector(),
                    v: v.vector(),
                },
                (-1.0, 1.0),
            )
            .ok()?;
            let basis = Trimmed::new(Basis::Ellipse(e), (-0.5, 0.5)).ok()?;
            let law = if D == 2 {
                OffsetLaw::Planar { distance: v.next() }
            } else {
                OffsetLaw::Reference {
                    distance: v.next(),
                    direction: v.vector(),
                }
            };
            CurveKind::Offset(Offset::new(basis, law).ok()?)
        }
        13 => {
            let line = Line::new([0.25, 0.5], [0.5, 0.25], (0.0, 1.0)).ok()?;
            let pcurve = Trimmed::new(Basis::Line(line), (0.0, 1.0)).ok()?;
            let cache = nurbs(v, 2, false)?;
            CurveKind::OnSurface(OnSurface::new(pcurve, SurfaceRef(1), cache, v.positive()).ok()?)
        }
        _ => {
            let law = CurvatureLaw::Clothoid {
                start: v.next(),
                end: v.next(),
            };
            let mut meter = WorkMeter::new(Budget::of_work(WORK));
            CurveKind::Transition(
                Transition::new(c, v.vector(), v.positive(), law, &mut meter).ok()?,
            )
        }
    })
}

/// How far the kind contract got on a case: what a test reads to see which laws ran. Every value
/// is a legal outcome; a broken law is a [`Finding`] instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reached {
    /// The transform or the kind refused to be built, or the target passes the kind: no law ran.
    Unbuilt,
    /// `contract::check` ran to its end: every law held.
    Whole,
    /// `contract::check` ended on a typed error that is not a budget stop.
    Refused(ContractError),
    /// `contract::check` ended on its budget, and the pass without conversions ran after it.
    Stopped {
        /// Where the check stopped: the law it was on, with the meter's stop.
        stop: ContractError,
        /// The pass's end: every law it runs held, or a typed error (its own budget's stop among
        /// them).
        pass: Result<(), ContractError>,
    },
}

/// The kind contract on `kind` at `params` (fractions k/255 of its domain) under `transform`,
/// and after a budget stop the pass without conversions: [`check`] on a kind handed in, so tests
/// can show it catches a broken one.
///
/// # Errors
///
/// A [`Finding`] when a law is broken with no error behind it, or a call fails as
/// `CurveError::Defect`, in the contract or in the pass after a budget stop.
pub fn check_with<K: ParamCurve<D> + core::fmt::Debug, const D: usize>(
    kind: &K,
    params: &[u8; PARAMS],
    transform: &TypedTransform,
) -> Result<(), Finding> {
    contract_on(kind, *params, transform, (WORK, PARTIAL_WORK)).map(drop)
}

/// Whether the contract ended on its meter's budget.
pub(crate) fn over_budget(e: &ContractError) -> bool {
    matches!(e.error, Some(CurveError::Stopped(s)) if matches!(s.reason(), Stop::OverBudget(_)))
}

/// The verdict on a contract's error: a finding when the law broke with no error behind it or a
/// call failed as a defect, else legal. `law` names the entry, `detail` the case.
pub(crate) fn verdict(
    e: &ContractError,
    law: &str,
    detail: impl FnOnce() -> String,
) -> Result<(), Finding> {
    match e.error {
        None => Err(Finding::new(law, move || format!("{e} {}", detail()))),
        Some(CurveError::Defect) => Err(Finding::new("no call fails as a defect", move || {
            format!("{e} {}", detail())
        })),
        Some(_) => Ok(()),
    }
}

/// [`check_with`] on the budgets `(work, partial)`: `work` for the contract, `partial` for the pass
/// without conversions after it stops on its budget; how far it got.
pub(crate) fn contract_on<K: ParamCurve<D> + core::fmt::Debug, const D: usize>(
    kind: &K,
    params: [u8; PARAMS],
    transform: &TypedTransform,
    (work, partial): (u64, u64),
) -> Result<Reached, Finding> {
    let (a, b) = kind.domain();
    let (lo, hi) = (
        if a.is_finite() { a } else { -8.0 },
        if b.is_finite() { b } else { 8.0 },
    );
    let ts: Vec<f64> = params
        .iter()
        .map(|&k| (lo + (hi - lo) * (f64::from(k) / 255.0)).clamp(lo, hi))
        .collect();
    let mut meter = WorkMeter::new(Budget::of_work(work));
    let Err(e) = contract::check(kind, &ts, transform, &mut meter) else {
        return Ok(Reached::Whole);
    };
    verdict(
        &e,
        "the kind contract (geom_curve::contract::check)",
        || format!("on {kind:?} at {ts:?} under {transform:?}"),
    )?;
    if !over_budget(&e) {
        return Ok(Reached::Refused(e));
    }
    // The laws that need no conversion, under a meter of their own.
    let mut meter = WorkMeter::new(Budget::of_work(partial));
    let pass = contract::check_without_conversions(kind, &ts, &mut meter);
    if let Err(p) = &pass {
        verdict(
            p,
            "the kind contract without its conversions, after a budget stop \
             (geom_curve::contract::check_without_conversions)",
            || format!("on {kind:?} at {ts:?}, after {e} under {transform:?}"),
        )?;
    }
    Ok(Reached::Stopped { stop: e, pass })
}

/// Builds the case's kind and holds it to the kind contract (module documentation); an offset
/// passes unbuilt, as `crate::curve_offsets` fuzzes it.
///
/// # Errors
///
/// As [`check_with`].
pub fn check(case: &Case) -> Result<(), Finding> {
    reached(case).map(drop)
}

/// [`check`], and how far the contract got.
///
/// # Errors
///
/// As [`check_with`].
pub fn reached(case: &Case) -> Result<Reached, Finding> {
    if case.kind == OFFSET {
        return Ok(Reached::Unbuilt);
    }
    reached_on(case, (WORK, PARTIAL_WORK))
}

/// The case's kind built, whatever its tag, and held to the kind contract on the budgets
/// `budgets`, as [`contract_on`] takes them; how far it got.
pub(crate) fn reached_on(case: &Case, budgets: (u64, u64)) -> Result<Reached, Finding> {
    let mut v = Values::of(&case.values);
    // The helix, the on-surface curve and the 3D offset are space curves; the bulge polyline and
    // the transition are plane curves.
    let space = match case.kind {
        6 | 13 => true,
        9 | 14 => false,
        _ => case.space,
    };
    if space {
        let Some(t) = transform(case.transform, false, &mut v) else {
            return Ok(Reached::Unbuilt);
        };
        match build::<3>(case.kind, &mut v) {
            Some(k) => contract_on(&k, case.params, &t, budgets),
            None => Ok(Reached::Unbuilt),
        }
    } else {
        let Some(t) = transform(case.transform, true, &mut v) else {
            return Ok(Reached::Unbuilt);
        };
        match build::<2>(case.kind, &mut v) {
            Some(k) => contract_on(&k, case.params, &t, budgets),
            None => Ok(Reached::Unbuilt),
        }
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
