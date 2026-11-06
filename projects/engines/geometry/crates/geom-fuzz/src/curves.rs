//! Fuzz target: every curve kind keeps the kind contract on any bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a curve kind (one of the 15
//! of `geom_curve::CurveKind`, in two or three dimensions as the kind allows), the doubles it is
//! built from, up to four parameters in its domain and a typed transform. [`check`] builds the
//! kind and runs `geom_curve::contract::check` on it, the kind contract kit every kind is held to
//! (piece G60, with G68's derivative laws), under a bounded work meter. [`harness`] is `decode`
//! then `check`, the function every fuzzing engine drives (the crate README sets out the layout).
//! It reaches pieces G60 to G64, G67 and G68 (Gate 2, 32.44).
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case. Each
//!   double is read through a rule byte that makes the hard values common: 0, ±1, a whole number
//!   or a short binary fraction on a coarse grid (so coincident and parallel data come up), a
//!   tiny or huge power of two, or a double read from the input (non-finite ones replaced by 0).
//! - **What is a finding.** A law of the contract broken with no error behind it, or any call
//!   failing as `CurveError::Defect` (an internal step the kernel says cannot fail), is a
//!   [`Finding`]; a kind that refuses to be built, or a call that refuses with a typed error
//!   (an overflow at an extreme size, a budget stop), is legal. A panic fails every engine.
//! - **No hang.** Every call runs under a work meter of [`WORK`] units; running out is a typed
//!   stop, which is legal.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a
//!   unit test builds the input that reaches it.
//!
//! Method: the kind contract of chapter 7 as `geom_curve::contract` states it, driven by
//! coverage-guided fuzzing (chapter 28.5), with inputs aimed at the hard cases 28.5 lists
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: one kind built and one contract check per input, each bounded by [`WORK`] work units;
//! at most [`MAX_INPUT_BYTES`] bytes read.

use geom_curve::offset::{Offset, OffsetLaw};
use geom_curve::on_surface::{OnSurface, SurfaceRef};
use geom_curve::trimmed::Trimmed;
use geom_curve::{
    Axes, Basis, BulgePolyline, Circle, Composite, CurvatureLaw, CurveError, CurveKind, Ellipse,
    Frame, Helix, HelixAxes, Hyperbola, Involute, Line, Parabola, ParamCurve, Polynomial,
    Transition, contract,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_run::{Budget, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The doubles a case reads.
pub const VALUES: usize = 24;

/// The parameters a case reads.
pub const PARAMS: usize = 4;

/// The work units each call may spend.
pub const WORK: u64 = 2_000_000;

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

/// The kind contract on `kind` at `params` (fractions k/255 of its domain) under `transform`:
/// [`check`] on a kind handed in, so tests can show it catches a broken one.
///
/// # Errors
///
/// A [`Finding`] when a law is broken with no error behind it, or a call fails as
/// `CurveError::Defect`.
pub fn check_with<K: ParamCurve<D> + core::fmt::Debug, const D: usize>(
    kind: &K,
    params: &[u8; PARAMS],
    transform: &TypedTransform,
) -> Result<(), Finding> {
    let (a, b) = kind.domain();
    let (lo, hi) = (
        if a.is_finite() { a } else { -8.0 },
        if b.is_finite() { b } else { 8.0 },
    );
    let ts: Vec<f64> = params
        .iter()
        .map(|&k| (lo + (hi - lo) * (f64::from(k) / 255.0)).clamp(lo, hi))
        .collect();
    let mut meter = WorkMeter::new(Budget::of_work(WORK));
    match contract::check(kind, &ts, transform, &mut meter) {
        Ok(()) => Ok(()),
        Err(e) => match e.error {
            None => Err(Finding::new(
                "the kind contract (geom_curve::contract::check)",
                || format!("{e} on {kind:?} at {ts:?} under {transform:?}"),
            )),
            Some(CurveError::Defect) => Err(Finding::new("no call fails as a defect", || {
                format!("{e} on {kind:?} at {ts:?} under {transform:?}")
            })),
            Some(_) => Ok(()),
        },
    }
}

/// Builds the case's kind and holds it to the kind contract (module documentation).
///
/// # Errors
///
/// As [`check_with`].
pub fn check(case: &Case) -> Result<(), Finding> {
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
            return Ok(());
        };
        match build::<3>(case.kind, &mut v) {
            Some(k) => check_with(&k, &case.params, &t),
            None => Ok(()),
        }
    } else {
        let Some(t) = transform(case.transform, true, &mut v) else {
            return Ok(());
        };
        match build::<2>(case.kind, &mut v) {
            Some(k) => check_with(&k, &case.params, &t),
            None => Ok(()),
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
