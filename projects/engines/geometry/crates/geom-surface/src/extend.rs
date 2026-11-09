//! Surface extension (chapter 13.9; piece G253): a surface continued past one side of its domain,
//! every kind by its own definition or, for NURBS, by the natural continuation of its end span,
//! and refused where the continuation may degenerate.
//!
//! What it does: [`extend`] continues a surface past one [`Side`] of its domain by a parameter
//! length, the parameters of the source unchanged:
//!
//! - **Planes, cylinders, cones, spheres and tori** are the same kind on the wider domain. An
//!   angular direction reaches at most one turn in all; a sphere's latitude reaches its poles,
//!   which stay poles; a cone's height reaches up to its apex, never past it, and a spindle
//!   torus's tube angle up to the circle where its radius R + r·cos πv vanishes, each decided by
//!   the kind's certified radius sign over the new strip.
//! - **Affine images of those** are the same terms on the wider domain, an angular direction at
//!   most one turn.
//! - **Surfaces of revolution, extrusions and ruled surfaces** widen a linear direction or a
//!   revolution's angle by definition, and a profile direction by widening each profile: inside
//!   its basis curve's domain by definition, past a NURBS basis's end by that basis's natural
//!   continuation, past a line's by the wider line.
//! - **Offsets** are the offset, at the same distance, of their base extended (an offset of an
//!   offset, of its innermost base, without recursion).
//! - **NURBS** continue their end span's polynomial (`geom_nurbs::Surface::extend`, Hu, Tai and
//!   Zhang's natural extension), the source's net kept bit for bit.
//!
//! Where a kind's definition does not reach the length asked (a turn, a pole, an apex, a basis
//! curve's end, a rational continuation whose weight may not be positive), the longest length it
//! reaches is found by bisection on the length. Every kind but the analytic ones then has the
//! new strip's normal certified: the strip is cut into slabs from the old side outward, each slab
//! into boxes across it, and a box passes when the kind's normal bound (a box holding a positive
//! multiple of the normal, piece G68) has a component certainly not zero. A slab that does not
//! pass is halved, the near half first; one that still fails at 2⁻²⁴ of the length ends the
//! certified reach at its near edge. A surface that would fold over at a cusp, or degenerate,
//! fails there. [`Reach::Exactly`] then refuses with [`ExtendError::ExtensionFailed`], naming the
//! side and the length reached; [`Reach::AsFarAsValid`] returns the surface extended that far,
//! certified again.
//!
//! Guarantees:
//!
//! - **The source unchanged.** Over the source's domain the extended surface is the source: the
//!   same kind's definition on a wider domain, or (NURBS, a continued profile basis) the source's
//!   own control points and knots kept, the extension after them.
//! - **The definition, continued.** An analytic or swept extension is the kind's own definition
//!   on the wider domain, exactly; a NURBS extension is the end span's polynomial continued,
//!   C∞ across the old side, within the reported deviation.
//! - **No degenerate normal in the extension.** Every non-analytic extension returned has a
//!   normal certified not zero over the whole new strip (so it cannot fold back at a cusp there);
//!   an analytic one keeps its definition's limits. Where that cannot be shown, the extension is
//!   refused with the length reached, or, when asked, stopped there.
//! - **Not checked: global self-intersection.** A long extension may meet a far part of the
//!   surface without any local fold. The operation that uses an extension checks its result for
//!   global self-intersection (piece G176), as chapter 13.9 asks.
//! - **The length reached is found, not proved the longest.** Where a definition stops short
//!   (bisection) or the certificate does (halving), the length is one that works; a rational
//!   weight's sign need not be monotone in the length.
//!
//! Method: chapter 13.9 (`projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`):
//! each kind by its definition, NURBS by S.-M. Hu, C.-L. Tai and S.-H. Zhang's natural extension
//! (Computer-Aided Design 34(5), 2002, <https://doi.org/10.1016/S0010-4485(01)00108-7>); the
//! normal certified by interval subdivision of the kinds' normal bounds (R. E. Moore, *Interval
//! Analysis*, 1966); the reach by bisection on the length.
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/6432dafe7d716a973a653fca72d5d4c7cd7002c8/crates/solidcore/src/surface/extend.rs>,
//!   for the analytic kinds widened on their own parameters and the refusal of continued weights
//!   that are not positive, naming a length that works. No code copied.
//! Consulted: NVIDIA USD `BRep` `SMLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/3fd5b9548d19aa8c12c29bc9637a96d42a06f8a5/source/SMLib/src/SmBSplineSurfaceNLib.cpp>,
//!   for `CreateExtendedSurface`, which skips a side at a singularity and shortens its length
//!   until the extension succeeds; its tolerance-based distance and knot removal are not taken.
//!   No code copied.
//!
//! Cost: a definition's extension is the kind's construction; a NURBS one is
//! `geom_nurbs::Surface::extend`'s; a reach not met costs at most [`BISECTIONS`] more of them.
//! The certificate costs at most [`MAX_BOXES`] normal bounds, each charged to the meter, and an
//! as-far-as-valid extension at most [`SHRINKS`] more certificates; [`extend_capped`] takes other
//! caps ([`Caps`]). No recursion: explicit stacks.

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use geom_curve::CurveError;
use geom_nurbs::{Bounds, Changed, Direction, End, NurbsError};
use geom_run::{Stopped, WorkMeter};

use crate::offset::Offset;
use crate::param::{ParamBox, ParamSurface, SurfaceKind};

/// The most normal bounds one certificate takes.
pub const MAX_BOXES: usize = 1 << 14;

/// The bisection steps that find the longest length a definition reaches.
pub const BISECTIONS: usize = 60;

/// The narrowest slab the certificate splits, as a fraction of the length: 2⁻²⁴.
const NARROWEST: f64 = 1.0 / 16_777_216.0;

/// The narrowest box across a slab, as a fraction of the side's width: 2⁻¹².
const NARROWEST_ACROSS: f64 = 1.0 / 4096.0;

/// How many times an as-far-as-valid extension is rebuilt shorter and certified again.
pub const SHRINKS: usize = 8;

/// A side of a surface's domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// u at its smallest.
    UMin,
    /// u at its largest.
    UMax,
    /// v at its smallest.
    VMin,
    /// v at its largest.
    VMax,
}

impl Side {
    /// The direction the side is crossed in.
    #[must_use]
    pub const fn direction(self) -> Direction {
        match self {
            Side::UMin | Side::UMax => Direction::U,
            Side::VMin | Side::VMax => Direction::V,
        }
    }

    /// Which end of that direction's domain it is.
    #[must_use]
    pub const fn end(self) -> End {
        match self {
            Side::UMin | Side::VMin => End::Low,
            Side::UMax | Side::VMax => End::High,
        }
    }
}

/// How far an extension goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reach {
    /// The length asked, or [`ExtendError::ExtensionFailed`].
    Exactly,
    /// The length asked where the extension stays valid, or as far as it does (13.9's "as far as
    /// possible", only when asked), with the length reached reported.
    AsFarAsValid,
}

/// The work caps of an extension: how many normal bounds one certificate may take, and how many
/// times an as-far-as-valid extension may be rebuilt shorter and certified again. [`extend`] uses
/// [`Caps::DEFAULT`]; [`extend_capped`] takes others. A cap reached ends the work as a refusal
/// would, never with a normal it has not certified.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Caps {
    /// The most normal bounds one certificate takes.
    pub boxes: usize,
    /// How many times an as-far-as-valid extension is rebuilt shorter and certified again.
    pub shrinks: usize,
}

impl Caps {
    /// [`MAX_BOXES`] and [`SHRINKS`].
    pub const DEFAULT: Caps = Caps {
        boxes: MAX_BOXES,
        shrinks: SHRINKS,
    };
}

/// An extended surface.
#[derive(Clone, Debug, PartialEq)]
pub struct Extended {
    /// The surface, on its source's parameters, its domain wider past the side.
    pub surface: SurfaceKind,
    /// The parameter length the domain was widened by: the length asked, or under
    /// [`Reach::AsFarAsValid`] as far as the extension stays valid (0 for none).
    pub reached: f64,
    /// A certified bound on how far the surface's defining data (a NURBS net, a continued profile
    /// basis, an offset's base) lies from the exact continuation's: 0 for an extension by
    /// definition.
    pub deviation: f64,
}

/// What went wrong with an extension.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExtendError {
    /// The extension would leave the kind's definition, degenerate or fold, and no shorter one
    /// was asked for (13.9's `ExtensionFailed`): the side and the parameter length reached.
    ExtensionFailed {
        /// The side.
        side: Side,
        /// The longest length found valid.
        reached: f64,
    },
    /// The surface or its parts refused: a length that is not positive and finite, a NURBS form
    /// not clamped at the side, a value that is not finite.
    Curve(CurveError),
    /// The work meter stopped.
    Stopped(Stopped),
}

impl fmt::Display for ExtendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtendError::ExtensionFailed { side, reached } => write!(
                f,
                "the surface cannot be extended past {side:?} as far as asked: it stays valid for a length of {reached}"
            ),
            ExtendError::Curve(e) => write!(f, "the surface refused: {e:?}"),
            ExtendError::Stopped(s) => write!(f, "stopped: {s:?}"),
        }
    }
}

impl From<CurveError> for ExtendError {
    fn from(e: CurveError) -> Self {
        match e {
            CurveError::Stopped(s) => ExtendError::Stopped(s),
            other => ExtendError::Curve(other),
        }
    }
}

impl From<Stopped> for ExtendError {
    fn from(s: Stopped) -> Self {
        ExtendError::Stopped(s)
    }
}

/// `domain` widened by `len` past `side`.
fn grown(domain: ParamBox, side: Side, len: f64) -> ParamBox {
    let ((u0, u1), (v0, v1)) = domain;
    match side {
        Side::UMin => ((u0 - len, u1), (v0, v1)),
        Side::UMax => ((u0, u1 + len), (v0, v1)),
        Side::VMin => ((u0, u1), (v0 - len, v1)),
        Side::VMax => ((u0, u1), (v0, v1 + len)),
    }
}

/// The side's parameter in `domain`, and the other direction's range.
fn side_of(domain: ParamBox, side: Side) -> (f64, (f64, f64)) {
    let ((u0, u1), (v0, v1)) = domain;
    match side {
        Side::UMin => (u0, (v0, v1)),
        Side::UMax => (u1, (v0, v1)),
        Side::VMin => (v0, (u0, u1)),
        Side::VMax => (v1, (u0, u1)),
    }
}

/// Whether a kind is decided by its closed-form limits rather than by the certificate.
fn analytic(kind: &SurfaceKind) -> bool {
    matches!(
        kind,
        SurfaceKind::Plane(_)
            | SurfaceKind::Cylinder(_)
            | SurfaceKind::Cone(_)
            | SurfaceKind::Sphere(_)
            | SurfaceKind::Torus(_)
    )
}

/// A refusal that means the definition does not reach the length asked: `None`.
fn reaches<T>(r: Result<T, CurveError>) -> Result<Option<T>, ExtendError> {
    match r {
        Ok(t) => Ok(Some(t)),
        Err(
            CurveError::SpanOutside
            | CurveError::Nurbs(NurbsError::Weight { .. } | NurbsError::KnotOutside),
        ) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Whether a cone's or torus's radius keeps one certain sign over the new strip of a v side,
/// which runs from the old side to the new one, so that the extension stops short of an apex or
/// a spindle's singular circle (true for a u side, which turns about the axis).
fn radius_kept(
    sign: impl Fn((f64, f64)) -> Result<Option<bool>, CurveError>,
    source: ParamBox,
    side: Side,
    len: f64,
) -> Result<bool, CurveError> {
    let (_, (v0, v1)) = source;
    let strip = match side {
        Side::VMin => (v0 - len, v0),
        Side::VMax => (v1, v1 + len),
        Side::UMin | Side::UMax => return Ok(true),
    };
    Ok(sign(strip)?.is_some())
}

/// The surface (not an offset) extended `len` past `side` by its definition, or `None` where its
/// definition does not reach that far.
fn built_plain(
    kind: &SurfaceKind,
    side: Side,
    len: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Changed<SurfaceKind>>, ExtendError> {
    let domain = kind.domain();
    let wider = grown(domain, side, len);
    let exact = |k: SurfaceKind| Changed {
        value: k,
        deviation: 0.0,
    };
    let (dir, end) = (side.direction(), side.end());
    Ok(match kind {
        SurfaceKind::Nurbs(s) => reaches(
            s.extend(dir, end, len, meter)
                .map_err(CurveError::from)
                .map(|c| Changed {
                    value: SurfaceKind::Nurbs(c.value),
                    deviation: c.deviation,
                }),
        )?,
        SurfaceKind::Plane(p) => {
            reaches(p.with_domain(wider).map(|k| exact(SurfaceKind::Plane(k))))?
        }
        SurfaceKind::Cylinder(c) => reaches(
            c.with_domain(wider)
                .map(|k| exact(SurfaceKind::Cylinder(k))),
        )?,
        SurfaceKind::Sphere(s) => {
            reaches(s.with_domain(wider).map(|k| exact(SurfaceKind::Sphere(k))))?
        }
        SurfaceKind::Cone(c) => {
            if radius_kept(|v| c.radius_sign_over(v), domain, side, len)? {
                reaches(c.with_domain(wider).map(|k| exact(SurfaceKind::Cone(k))))?
            } else {
                None
            }
        }
        SurfaceKind::Torus(t) => {
            if radius_kept(|v| t.radius_sign_over(v), domain, side, len)? {
                reaches(t.with_domain(wider).map(|k| exact(SurfaceKind::Torus(k))))?
            } else {
                None
            }
        }
        SurfaceKind::Affine(a) => {
            reaches(a.with_domain(wider).map(|k| exact(SurfaceKind::Affine(k))))?
        }
        SurfaceKind::Revolution(r) => reaches(r.extended(dir, end, len, meter))?,
        SurfaceKind::Extrusion(e) => reaches(e.extended(dir, end, len, meter))?,
        SurfaceKind::Ruled(r) => reaches(r.extended(dir, end, len, meter))?,
        SurfaceKind::Offset(_) => return Err(CurveError::Defect.into()),
    })
}

/// The surface extended `len` past `side` by its definition, an offset as the offset of its
/// innermost base extended; `None` where the definition does not reach that far.
fn built(
    kind: &SurfaceKind,
    side: Side,
    len: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Changed<SurfaceKind>>, ExtendError> {
    let mut distances = Vec::new();
    let mut base = kind;
    while let SurfaceKind::Offset(o) = base {
        distances.push(o.distance());
        base = o.base();
    }
    let Some(grown_base) = built_plain(base, side, len, meter)? else {
        return Ok(None);
    };
    let mut value = grown_base.value;
    for d in distances.iter().rev() {
        value = SurfaceKind::Offset(Offset::new(value, *d)?);
    }
    Ok(Some(Changed {
        value,
        deviation: grown_base.deviation,
    }))
}

/// The longest length in [0, `by`] the definition reaches, by bisection; `by` itself does not.
fn longest(
    kind: &SurfaceKind,
    side: Side,
    by: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<f64, ExtendError> {
    let (mut lo, mut hi) = (0.0, by);
    for _ in 0..BISECTIONS {
        let mid = 0.5 * lo + 0.5 * hi;
        if !(lo < mid && mid < hi) {
            break;
        }
        if built(kind, side, mid, meter)?.is_some() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(lo)
}

/// Whether a normal bound certainly excludes the zero vector.
fn excludes_zero(b: &Bounds<3>) -> bool {
    b.lo.iter()
        .zip(&b.hi)
        .any(|(&lo, &hi)| lo > 0.0 || hi < 0.0)
}

/// The box of `s` over [`a`, `b`] across the side in its direction, and `across` in the other.
fn region(side: Side, (a, b): (f64, f64), across: (f64, f64)) -> ParamBox {
    let along = if a <= b { (a, b) } else { (b, a) };
    match side.direction() {
        Direction::U => (along, across),
        Direction::V => (across, along),
    }
}

/// Whether the normal of `s` is certainly not zero over the slab between `a` and `b` (side
/// parameters) and all of `across`, splitting across with an explicit stack; false once the
/// certificate's `boxes` pass `cap`.
fn slab_passes(
    s: &SurfaceKind,
    side: Side,
    along: (f64, f64),
    across: (f64, f64),
    (boxes, cap): (&mut usize, usize),
    meter: &mut WorkMeter<'_>,
) -> Result<bool, ExtendError> {
    let narrow = (across.1 - across.0) * NARROWEST_ACROSS;
    let mut stack = vec![across];
    while let Some((a, b)) = stack.pop() {
        *boxes = boxes.saturating_add(1);
        if *boxes > cap {
            return Ok(false);
        }
        meter.charge(1)?;
        let ok = match s.normal_bound(region(side, along, (a, b)), meter) {
            Ok(bound) => excludes_zero(&bound),
            Err(CurveError::Stopped(st)) => return Err(ExtendError::Stopped(st)),
            Err(_) => false,
        };
        if ok {
            continue;
        }
        if b - a <= narrow {
            return Ok(false);
        }
        let mid = 0.5 * a + 0.5 * b;
        stack.push((mid, b));
        stack.push((a, mid));
    }
    Ok(true)
}

/// Whether the new strip of `s`, from the old side `old` out to its side now, has a normal
/// certainly not zero: `None` if it has everywhere, or the length from the old side it is
/// certified to. The strip is cut into slabs from the old side outward, halved where they do not
/// pass; a slab still failing at its narrowest ends the reach at its near edge. At most `cap`
/// normal bounds are taken.
fn certified(
    s: &SurfaceKind,
    side: Side,
    old: f64,
    cap: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<f64>, ExtendError> {
    let (far, across) = side_of(s.domain(), side);
    let length = (far - old).abs();
    let at = |d: f64| -> f64 {
        if d >= length {
            far
        } else {
            match side.end() {
                End::Low => old - d,
                End::High => old + d,
            }
        }
    };
    let narrowest = length * NARROWEST;
    let mut todo = vec![(0.0, length)];
    let mut reached = 0.0;
    let mut boxes = 0usize;
    while let Some((d0, d1)) = todo.pop() {
        meter.charge(1)?;
        if slab_passes(s, side, (at(d0), at(d1)), across, (&mut boxes, cap), meter)? {
            reached = d1;
        } else if d1 - d0 <= narrowest {
            return Ok(Some(reached));
        } else {
            let mid = 0.5 * d0 + 0.5 * d1;
            todo.push((mid, d1));
            todo.push((d0, mid));
        }
    }
    Ok(None)
}

/// The surface `surface` continued `by` (a parameter length) past `side` of its domain, by its
/// definition or the natural continuation of its end span, as far as `reach` asks (module
/// documentation).
///
/// # Errors
///
/// [`ExtendError::ExtensionFailed`] under [`Reach::Exactly`] where the definition does not reach
/// `by` or the new strip's normal cannot be certified not zero, with the length reached;
/// [`ExtendError::Curve`] for a length that is not positive and finite, a NURBS form not clamped
/// at the side, or another refusal of the kind; [`ExtendError::Stopped`] if the meter stops it.
pub fn extend(
    surface: &SurfaceKind,
    side: Side,
    by: f64,
    reach: Reach,
    meter: &mut WorkMeter<'_>,
) -> Result<Extended, ExtendError> {
    extend_capped(surface, side, by, reach, Caps::DEFAULT, meter)
}

/// [`extend`] under the work caps `caps`: a certificate that would take more than `caps.boxes`
/// normal bounds ends where it has reached, as one that cannot pass does, and an as-far-as-valid
/// extension is rebuilt shorter at most `caps.shrinks` times before the source is returned
/// unchanged.
///
/// # Errors
///
/// As [`extend`].
pub fn extend_capped(
    surface: &SurfaceKind,
    side: Side,
    by: f64,
    reach: Reach,
    caps: Caps,
    meter: &mut WorkMeter<'_>,
) -> Result<Extended, ExtendError> {
    if !(by.is_finite() && by > 0.0) {
        return Err(CurveError::SpanOutside.into());
    }
    let unchanged = || Extended {
        surface: surface.clone(),
        reached: 0.0,
        deviation: 0.0,
    };
    let failed = |reached: f64| ExtendError::ExtensionFailed { side, reached };
    // The definition's reach.
    let (mut made, mut len) = if let Some(c) = built(surface, side, by, meter)? {
        (c, by)
    } else {
        let r = longest(surface, side, by, meter)?;
        if reach == Reach::Exactly {
            return Err(failed(r));
        }
        match built(surface, side, r, meter)? {
            Some(c) if r > 0.0 => (c, r),
            _ => return Ok(unchanged()),
        }
    };
    // The certificate, for every kind not decided in closed form.
    if !analytic(surface) {
        let (old, _) = side_of(surface.domain(), side);
        let mut tries = 0usize;
        while let Some(r) = certified(&made.value, side, old, caps.boxes, meter)? {
            if reach == Reach::Exactly {
                return Err(failed(r));
            }
            tries = tries.saturating_add(1);
            if r <= 0.0 || tries > caps.shrinks {
                return Ok(unchanged());
            }
            let Some(c) = built(surface, side, r, meter)? else {
                return Ok(unchanged());
            };
            made = c;
            len = r;
        }
    }
    Ok(Extended {
        surface: made.value,
        reached: len,
        deviation: made.deviation,
    })
}
