//! Fuzz target: every surface kind keeps the kind contract on any bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a surface kind (one of the 11
//! of `geom_surface::SurfaceKind`), the doubles it is built from, up to four parameter pairs in
//! its domain and a typed transform. [`check`] builds the kind and runs
//! `geom_surface::contract::check` on it, the kind contract kit every surface kind is held to
//! (piece G60, with G68's derivative laws), under a bounded work meter. [`harness`] is `decode`
//! then `check`, the function every fuzzing engine drives (the crate README sets out the layout).
//! It reaches pieces G60 and G65 to G68 (Gate 2, 32.44).
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case,
//!   through the same rule bytes as the `curves` target: 0, ±1, whole numbers and quarters on a
//!   coarse grid, tiny and huge powers of two, or a double read from the input.
//! - **What is a finding.** A law of the contract broken with no error behind it, or any call
//!   failing as `CurveError::Defect`, is a [`Finding`]; a kind that refuses to be built, or a call
//!   that refuses with a typed error (an overflow, a normal that cannot be separated from zero, a
//!   budget stop), is legal. A panic fails every engine.
//! - **No hang.** Every call runs under a work meter of [`WORK`] units; running out is a typed
//!   stop, which is legal.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a
//!   unit test builds the input that reaches it.
//!
//! Method: the kind contract of chapter 7 as `geom_surface::contract` states it, driven by
//! coverage-guided fuzzing (chapter 28.5), with inputs aimed at the hard cases 28.5 lists
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: one kind built and one contract check per input, each bounded by [`WORK`] work units;
//! at most [`MAX_INPUT_BYTES`] bytes read.

use geom_curve::trimmed::Trimmed;
use geom_curve::{Basis, CurveError, Frame, Line};
use geom_math::TypedTransform;
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, Extrusion, Offset, ParamSurface, Plane, Revolution, Ruled, Sphere, SurfaceKind,
    Torus, contract,
};
use geom_test::ByteSource;

use crate::Finding;
use crate::curves::{Values, transform, value};

/// The doubles a case reads.
pub const VALUES: usize = 32;

/// The parameter pairs a case reads.
pub const PARAMS: usize = 4;

/// The work units each call may spend.
pub const WORK: u64 = 4_000_000;

/// The most bytes [`decode`] reads from any input: 298.
///
/// Two header bytes (the kind and the transform), then [`VALUES`] doubles of a rule byte and, for
/// a double read from the input, eight bytes more, and two bytes per parameter pair:
/// 2 + 32 · (1 + 8) + 2 · 4 = 298. The unit test `the_input_bound_is_reached_and_never_passed`
/// builds an input that reads exactly this many.
pub const MAX_INPUT_BYTES: usize = 2 + VALUES * 9 + 2 * PARAMS;

/// The surface kinds a case may be, in `SurfaceKind`'s order.
pub const KINDS: u8 = 11;

/// One input: a kind's tag, the transform's tag, the doubles and the parameter pairs' positions
/// in the domain (each a fraction k/255 of each side).
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// Which kind, `0..KINDS` in `SurfaceKind`'s order.
    pub kind: u8,
    /// Which transform the transform law uses.
    pub transform: u8,
    /// The doubles the kind and the transform are built from.
    pub values: [f64; VALUES],
    /// Where the parameter pairs sit in the domain, as k/255 of each side.
    pub params: [(u8, u8); PARAMS],
}

/// Any bytes as a case, and how many bytes were read.
#[must_use]
pub fn decode(bytes: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(bytes);
    let kind = geom_test::Entropy::byte(&mut src)
        .checked_rem(KINDS)
        .unwrap_or(0);
    let transform = geom_test::Entropy::byte(&mut src);
    let mut values = [0.0; VALUES];
    for v in &mut values {
        *v = value(&mut src);
    }
    let mut params = [(0u8, 0u8); PARAMS];
    for p in &mut params {
        *p = (
            geom_test::Entropy::byte(&mut src),
            geom_test::Entropy::byte(&mut src),
        );
    }
    let read = bytes.len().saturating_sub(src.remaining());
    (
        Case {
            kind,
            transform,
            values,
            params,
        },
        read,
    )
}

/// A frame from six doubles, or none where they are parallel or not finite.
fn frame(v: &mut Values<'_>) -> Option<Frame<3>> {
    Frame::new(v.vector(), v.vector()).ok()
}

/// A trimmed profile: a NURBS curve through four of the case's points, rational.
fn profile(v: &mut Values<'_>) -> Option<Trimmed<3>> {
    let pts: Vec<[f64; 3]> = (0..4).map(|_| v.vector()).collect();
    let w: Vec<f64> = (0..4).map(|_| v.positive()).collect();
    let basis = geom_nurbs::Basis::new(2, vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]).ok()?;
    let c = geom_nurbs::Curve::new_rational(basis, &pts, &w).ok()?;
    Trimmed::new(Basis::Nurbs(c), (0.0, 1.0)).ok()
}

/// A biquadratic NURBS patch on a 3 × 3 net of the case's points, rational.
fn patch(v: &mut Values<'_>) -> Option<geom_nurbs::Surface<3>> {
    let pts: Vec<[f64; 3]> = (0..9).map(|_| v.vector()).collect();
    let w: Vec<f64> = (0..9).map(|_| v.positive()).collect();
    let knots = || geom_nurbs::Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    geom_nurbs::Surface::new_rational(knots().ok()?, knots().ok()?, &pts, &w).ok()
}

/// The kind of tag `kind` built from the case's doubles, or none where its constructor refuses.
fn build(kind: u8, v: &mut Values<'_>) -> Option<SurfaceKind> {
    let c: [f64; 3] = v.vector();
    Some(match kind {
        0 => SurfaceKind::Nurbs(patch(v)?),
        1 => SurfaceKind::Plane(Plane::new(c, v.vector(), v.vector(), (v.span(), v.span())).ok()?),
        2 => SurfaceKind::Cylinder(
            Cylinder::new(c, v.positive(), frame(v)?, ((0.0, 2.0), v.span())).ok()?,
        ),
        3 => SurfaceKind::Cone(
            Cone::new(c, v.next(), v.next(), frame(v)?, ((0.0, 2.0), v.span())).ok()?,
        ),
        4 => SurfaceKind::Sphere(
            Sphere::new(c, v.positive(), frame(v)?, ((0.0, 2.0), (-0.5, 0.5))).ok()?,
        ),
        5 => {
            let (major, minor) = (v.positive(), v.positive());
            SurfaceKind::Torus(
                Torus::new(c, major, minor, frame(v)?, ((0.0, 2.0), (0.0, 2.0))).ok()?,
            )
        }
        6 => {
            // An affine image: a torus under a non-uniform scale.
            let torus = Torus::new(
                c,
                v.positive(),
                v.positive(),
                frame(v)?,
                ((0.0, 2.0), (0.0, 2.0)),
            )
            .ok()?;
            let t = geom_math::Transform::scale(v.positive(), v.positive(), v.positive()).ok()?;
            let mut meter = WorkMeter::new(Budget::of_work(WORK));
            SurfaceKind::Torus(torus)
                .transformed(&TypedTransform::from(t), &mut meter)
                .ok()?
                .value
        }
        7 => SurfaceKind::Revolution(Revolution::new(profile(v)?, c, frame(v)?, (0.0, 1.5)).ok()?),
        8 => SurfaceKind::Extrusion(Extrusion::new(profile(v)?, v.vector(), v.span()).ok()?),
        9 => {
            let line = Line::new(c, v.vector(), (0.0, 1.0)).ok()?;
            let first = Trimmed::new(Basis::Line(line), (0.0, 1.0)).ok()?;
            SurfaceKind::Ruled(Ruled::new(first, profile(v)?, (0.0, 1.0)).ok()?)
        }
        _ => {
            let base = SurfaceKind::Cylinder(
                Cylinder::new(c, v.positive(), frame(v)?, ((0.0, 2.0), v.span())).ok()?,
            );
            SurfaceKind::Offset(Offset::new(base, v.next()).ok()?)
        }
    })
}

/// The kind contract on `kind` at `params` (fractions k/255 of each side of its domain) under
/// `transform`: [`check`] on a kind handed in, so tests can show it catches a broken one.
///
/// # Errors
///
/// A [`Finding`] when a law is broken with no error behind it, or a call fails as
/// `CurveError::Defect`.
pub fn check_with<K: ParamSurface + core::fmt::Debug>(
    kind: &K,
    params: &[(u8, u8); PARAMS],
    transform: &TypedTransform,
) -> Result<(), Finding> {
    let ((u0, u1), (v0, v1)) = kind.domain();
    let side = |lo: f64, hi: f64, k: u8| {
        let (lo, hi) = (
            if lo.is_finite() { lo } else { -8.0 },
            if hi.is_finite() { hi } else { 8.0 },
        );
        (lo + (hi - lo) * (f64::from(k) / 255.0)).clamp(lo, hi)
    };
    let ps: Vec<(f64, f64)> = params
        .iter()
        .map(|&(i, j)| (side(u0, u1, i), side(v0, v1, j)))
        .collect();
    let mut meter = WorkMeter::new(Budget::of_work(WORK));
    match contract::check(kind, &ps, transform, &mut meter) {
        Ok(()) => Ok(()),
        Err(e) => match e.error {
            None => Err(Finding::new(
                "the kind contract (geom_surface::contract::check)",
                || format!("{e} on {kind:?} at {ps:?} under {transform:?}"),
            )),
            Some(CurveError::Defect) => Err(Finding::new("no call fails as a defect", || {
                format!("{e} on {kind:?} at {ps:?} under {transform:?}")
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
    let Some(t) = transform(case.transform, false, &mut v) else {
        return Ok(());
    };
    match build(case.kind, &mut v) {
        Some(k) => check_with(&k, &case.params, &t),
        None => Ok(()),
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
