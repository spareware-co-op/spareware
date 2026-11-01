//! Geometry kinds as canonical items: every curve and surface kind a body's pools hold, written as
//! `[code, fields…]` and read back to the same value (24.2, 24.3).
//!
//! What it does: [`write_curve`] and [`write_surface`] append one kind to an encoder;
//! [`read_curve`] and [`read_surface`] read one back from a decoder, rebuilding it through the
//! kind's own constructor, so a stored kind is checked as a built one is. The codes are in
//! [`curve`] and [`surface`]; the fields of each are the values its constructor takes, in the
//! order `format/cddl/kinds.cddl` gives them. Doubles are written in their shortest exact width,
//! lists of doubles as RFC 8746 `f64` columns (tag 86), an absent value as `[]`, a present one as
//! `[value]`, and a flag as 0 or 1.
//!
//! Guarantees:
//!
//! - **Value for value.** A kind read back equals the one written (`PartialEq`), every double
//!   with its bits, a negative zero included: a kind keeps what defines it, never a refit.
//! - **Caches are rebuilt, not stored.** An offset's fitted NURBS cache is a function of the
//!   offset; the item says whether it was there and reading computes it again
//!   (`Offset::cached`), which gives the same bits on every target (23.1). A curve on a surface
//!   keeps its 3D curve, which defines it.
//! - **Infinite ends.** A parameter interval's end may be infinite only on its own side (a ray or
//!   line); it is written `[]`.
//! - **Bounded.** Every kind nests at most [`DEPTH`] containers deep: a surface offset at most
//!   `geom_surface::offset::MAX_NESTING` (4) times, over a swept surface whose profile is a conic
//!   on axes on a frame.
//!
//! Recursion:
//!
//! - Cycle: `write_surface`, `write_offset_surface`.
//!   Bound: 9.
//!   Why: two frames per offset (the dispatch and the offset's writer, which writes its base one
//!   level down), and `geom_surface::Offset::new` refuses a base already `MAX_NESTING` = 4
//!   offsets deep; then the leaf's dispatch.
//!   Test: `surfaces_write_and_read_at_their_bound`.
//! - Cycle: `read_surface`, `read_offset_surface`.
//!   Bound: 21.
//!   Why: two frames per offset read, each inside its offset's array, and the decoder refuses an
//!   item nested deeper than `DEPTH` = 10; then the leaf's dispatch. Past four offsets the
//!   kind's constructor refuses what was read.
//!   Test: `surfaces_write_and_read_at_their_bound`.
//!
//! Method: chapter 24.2 and 24.3 of the kernel architecture,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`; the kinds are those of
//! chapter 7, `07-curves-surfaces.md`.
//!
//! Cost: linear in a kind's values, plus, for an offset stored with its cache, the cost of fitting
//! it again.

#![allow(
    clippy::many_single_char_names,
    reason = "e the encoder, d the decoder, m the meter, c, f, l, n and s the kind in hand"
)]

use alloc::vec::Vec;
use core::fmt;

use geom_curve::geom_nurbs::{self, NurbsError};
use geom_curve::{
    Axes, Basis, BulgePolyline, Circle, Composite, CurvatureLaw, CurveError, CurveKind, Ellipse,
    Frame, Helix, HelixAxes, Hyperbola, Involute, Line, OffsetLaw, OnSurface, Parabola, ParamCurve,
    Polynomial, Station, SurfaceRef, Transition, Trimmed,
};
use geom_op::cbor::{EncodeError, Encoder};
use geom_run::WorkMeter;
use geom_surface::{
    AffineSurface, Cone, Cylinder, Extrusion, Fun, ParamBox, ParamSurface, Plane, Revolution,
    Ruled, Sphere, SurfaceKind, Torus,
};

use crate::cbor::{CborError, Decoder, Item, float_column};

/// The containers and tags one kind nests at most: four surface offsets, a swept surface, its
/// profile, the profile's conic, its axes, their frame and the frame's vector.
pub const DEPTH: usize = 10;

/// The curve kinds' codes.
pub mod curve {
    /// A NURBS curve: degree, knots, homogeneous points, weights.
    pub const NURBS: u64 = 1;
    /// A line, ray or segment: origin, direction, domain.
    pub const LINE: u64 = 2;
    /// A circle: centre, radius, frame, domain.
    pub const CIRCLE: u64 = 3;
    /// An ellipse: centre, axes, domain.
    pub const ELLIPSE: u64 = 4;
    /// A hyperbola: centre, axes, domain.
    pub const HYPERBOLA: u64 = 5;
    /// A parabola: vertex, axes, domain.
    pub const PARABOLA: u64 = 6;
    /// A helix: centre, helix axes, domain.
    pub const HELIX: u64 = 7;
    /// A polynomial curve: coefficients, domain.
    pub const POLYNOMIAL: u64 = 8;
    /// An involute: centre, axes, domain.
    pub const INVOLUTE: u64 = 9;
    /// A bulge polyline: vertices, bulges, widths, closed, start.
    pub const BULGE: u64 = 10;
    /// A trimmed curve: its leaf kind, interval.
    pub const TRIMMED: u64 = 11;
    /// A composite curve: segments, starts.
    pub const COMPOSITE: u64 = 12;
    /// An offset curve: basis, law, whether cached.
    pub const OFFSET: u64 = 13;
    /// A curve on a surface: pcurve, surface, 3D curve, error.
    pub const ON_SURFACE: u64 = 14;
    /// A transition curve: start, direction, length, law, window, reversed.
    pub const TRANSITION: u64 = 15;
}

/// The surface kinds' codes.
pub mod surface {
    /// A NURBS surface: degrees, knots, homogeneous points, weights.
    pub const NURBS: u64 = 1;
    /// A plane: origin, two vectors, domain.
    pub const PLANE: u64 = 2;
    /// A cylinder: centre, radius, frame, domain, axial sign.
    pub const CYLINDER: u64 = 3;
    /// A cone: centre, radius, slope, frame, domain, axial sign.
    pub const CONE: u64 = 4;
    /// A sphere: centre, radius, frame, domain, axial sign.
    pub const SPHERE: u64 = 5;
    /// A torus: centre, major and minor radii, frame, domain, axial sign.
    pub const TORUS: u64 = 6;
    /// An affine image of an analytic kind: centre, terms, domain, angular sides.
    pub const AFFINE: u64 = 7;
    /// A surface of revolution: profile, axis, frame, angles, placement.
    pub const REVOLUTION: u64 = 8;
    /// An extrusion: profile, direction, extent, placement.
    pub const EXTRUSION: u64 = 9;
    /// A ruled surface: two profiles, extent, σ, placement.
    pub const RULED: u64 = 10;
    /// An offset surface: base, distance, domain, whether cached.
    pub const OFFSET: u64 = 11;
}

/// Why a kind could not be written or read back.
#[derive(Clone, Debug, PartialEq)]
pub enum KindError {
    /// The encoder refused a value: a double that is not finite where one must be.
    Encode(EncodeError),
    /// Bytes that break SKG's canonical CBOR.
    Cbor(CborError),
    /// An item that is not the kind its code names.
    Shape(&'static str),
    /// The kind's constructor refused the values.
    Curve(CurveError),
    /// A NURBS constructor refused the values.
    Nurbs(NurbsError),
}

impl fmt::Display for KindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KindError::Encode(e) => write!(f, "no encoding: {e}"),
            KindError::Cbor(e) => write!(f, "not canonical CBOR: {e}"),
            KindError::Shape(what) => write!(f, "not a stored kind: {what}"),
            KindError::Curve(e) => write!(f, "the kind's values were refused: {e:?}"),
            KindError::Nurbs(e) => write!(f, "the NURBS values were refused: {e:?}"),
        }
    }
}

impl core::error::Error for KindError {}

impl From<EncodeError> for KindError {
    fn from(e: EncodeError) -> Self {
        KindError::Encode(e)
    }
}

impl From<CborError> for KindError {
    fn from(e: CborError) -> Self {
        KindError::Cbor(e)
    }
}

impl From<CurveError> for KindError {
    fn from(e: CurveError) -> Self {
        KindError::Curve(e)
    }
}

impl From<NurbsError> for KindError {
    fn from(e: NurbsError) -> Self {
        KindError::Nurbs(e)
    }
}

/// What a writer returns.
type Written = Result<(), KindError>;

/// A count as an array length.
fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// Doubles as an array.
fn floats(e: &mut Encoder, v: &[f64]) -> Written {
    e.array(count(v.len()))?;
    for &x in v {
        e.float(x)?;
    }
    Ok(())
}

/// Doubles as an `f64` column.
fn column(e: &mut Encoder, v: impl IntoIterator<Item = f64>) -> Written {
    let v: Vec<f64> = v.into_iter().collect();
    e.f64_column(&v)?;
    Ok(())
}

/// A flag: 0 or 1.
fn flag(e: &mut Encoder, b: bool) -> Written {
    e.uint(u64::from(b))?;
    Ok(())
}

/// An optional double: `[]` or `[x]`.
fn optional(e: &mut Encoder, x: Option<f64>) -> Written {
    match x {
        None => e.array(0)?,
        Some(x) => {
            e.array(1)?;
            e.float(x)?;
        }
    }
    Ok(())
}

/// One end of an interval: a double, or `[]` for the infinity on its own side (`low` the start).
fn end(e: &mut Encoder, x: f64, low: bool) -> Written {
    if x.is_infinite() && (x < 0.0) == low {
        e.array(0)?;
    } else {
        e.float(x)?;
    }
    Ok(())
}

/// An interval `[start, end]`.
fn interval(e: &mut Encoder, (a, b): (f64, f64)) -> Written {
    e.array(2)?;
    end(e, a, true)?;
    end(e, b, false)
}

/// A parameter box `[u, v]`.
fn param_box(e: &mut Encoder, (u, v): ParamBox) -> Written {
    e.array(2)?;
    interval(e, u)?;
    interval(e, v)
}

/// An axial sign or σ: the integer 1 or −1.
fn sign(e: &mut Encoder, s: f64) -> Written {
    if s == 1.0 {
        e.int(1)?;
    } else if s == -1.0 {
        e.int(-1)?;
    } else {
        return Err(KindError::Shape("a sign other than +1 or −1"));
    }
    Ok(())
}

/// A frame `[x, normal]`.
fn frame<const D: usize>(e: &mut Encoder, f: &Frame<D>) -> Written {
    e.array(2)?;
    floats(e, &f.x())?;
    floats(e, &f.normal())
}

/// Axes: `[0, frame, a, b]` or `[1, u, v]`.
fn axes<const D: usize>(e: &mut Encoder, a: &Axes<D>) -> Written {
    match a {
        Axes::Frame { frame: f, a, b } => {
            e.array(4)?;
            e.uint(0)?;
            frame(e, f)?;
            e.float(*a)?;
            e.float(*b)?;
        }
        Axes::Vectors { u, v } => {
            e.array(3)?;
            e.uint(1)?;
            floats(e, u)?;
            floats(e, v)?;
        }
    }
    Ok(())
}

/// Helix axes: `[0, frame, radius, pitch]` or `[1, u, v, w]`.
fn helix_axes<const D: usize>(e: &mut Encoder, a: &HelixAxes<D>) -> Written {
    match a {
        HelixAxes::Frame {
            frame: f,
            radius,
            pitch,
        } => {
            e.array(4)?;
            e.uint(0)?;
            frame(e, f)?;
            e.float(*radius)?;
            e.float(*pitch)?;
        }
        HelixAxes::Vectors { u, v, w } => {
            e.array(4)?;
            e.uint(1)?;
            floats(e, u)?;
            floats(e, v)?;
            floats(e, w)?;
        }
    }
    Ok(())
}

/// A NURBS curve's four fields: degree, knots, homogeneous points, weights.
fn nurbs_fields<const D: usize>(e: &mut Encoder, c: &geom_nurbs::Curve<D>) -> Written {
    e.uint(count(c.basis().degree()))?;
    column(e, c.basis().knots().iter().copied())?;
    column(e, c.homogeneous().iter().flatten().copied())?;
    match c.weights() {
        None => e.array(0)?,
        Some(w) => {
            e.array(1)?;
            e.f64_column(w)?;
        }
    }
    Ok(())
}

/// A curvature law: `[code, values…]`, 1 to 9 in `CurvatureLaw`'s order.
fn law(e: &mut Encoder, l: &CurvatureLaw) -> Written {
    match *l {
        CurvatureLaw::Clothoid { start, end }
        | CurvatureLaw::Helmert { start, end }
        | CurvatureLaw::Bloss { start, end }
        | CurvatureLaw::Cosine { start, end }
        | CurvatureLaw::Sine { start, end } => {
            let code = match l {
                CurvatureLaw::Clothoid { .. } => 1,
                CurvatureLaw::Helmert { .. } => 2,
                CurvatureLaw::Bloss { .. } => 3,
                CurvatureLaw::Cosine { .. } => 4,
                _ => 5,
            };
            e.array(3)?;
            e.uint(code)?;
            e.float(start)?;
            e.float(end)?;
        }
        CurvatureLaw::CantCoupled {
            start,
            end,
            height,
            cant,
        } => {
            e.array(5)?;
            e.uint(6)?;
            for x in [start, end, height, cant] {
                e.float(x)?;
            }
        }
        CurvatureLaw::Polynomial { terms, from } => {
            e.array(3)?;
            e.uint(7)?;
            e.array(count(terms.len()))?;
            for t in terms {
                optional(e, t)?;
            }
            e.float(from)?;
        }
        CurvatureLaw::CosineSpiral {
            constant,
            cosine,
            length,
            from,
        } => {
            e.array(5)?;
            e.uint(8)?;
            optional(e, constant)?;
            for x in [cosine, length, from] {
                e.float(x)?;
            }
        }
        CurvatureLaw::SineSpiral {
            constant,
            linear,
            sine,
            length,
            from,
        } => {
            e.array(6)?;
            e.uint(9)?;
            optional(e, constant)?;
            optional(e, linear)?;
            for x in [sine, length, from] {
                e.float(x)?;
            }
        }
    }
    Ok(())
}

/// An offset law: `[1, distance]`, `[2, distance, direction]` or `[3, stations]`.
fn offset_law(e: &mut Encoder, l: &OffsetLaw) -> Written {
    match l {
        OffsetLaw::Planar { distance } => {
            e.array(2)?;
            e.uint(1)?;
            e.float(*distance)?;
        }
        OffsetLaw::Reference {
            distance,
            direction,
        } => {
            e.array(3)?;
            e.uint(2)?;
            e.float(*distance)?;
            floats(e, direction)?;
        }
        OffsetLaw::Distances(stations) => {
            e.array(2)?;
            e.uint(3)?;
            column(
                e,
                stations.iter().flat_map(|s| [s.at, s.lateral, s.vertical]),
            )?;
        }
    }
    Ok(())
}

/// A leaf kind: one of the kinds a trimmed curve may hold.
#[allow(clippy::too_many_lines, reason = "one short arm per leaf kind")]
fn leaf<const D: usize>(e: &mut Encoder, b: &Basis<D>) -> Written {
    match b {
        Basis::Nurbs(c) => {
            e.array(5)?;
            e.uint(curve::NURBS)?;
            nurbs_fields(e, c)
        }
        Basis::Line(c) => {
            e.array(4)?;
            e.uint(curve::LINE)?;
            floats(e, &c.origin())?;
            floats(e, &c.direction())?;
            interval(e, ParamCurve::domain(c))
        }
        Basis::Circle(c) => {
            e.array(5)?;
            e.uint(curve::CIRCLE)?;
            floats(e, &c.center())?;
            e.float(c.radius())?;
            frame(e, &c.frame())?;
            interval(e, ParamCurve::domain(c))
        }
        Basis::Ellipse(c) => on_axes(
            e,
            curve::ELLIPSE,
            &c.center(),
            &c.axes(),
            ParamCurve::domain(c),
        ),
        Basis::Hyperbola(c) => on_axes(
            e,
            curve::HYPERBOLA,
            &c.center(),
            &c.axes(),
            ParamCurve::domain(c),
        ),
        Basis::Parabola(c) => on_axes(
            e,
            curve::PARABOLA,
            &c.vertex(),
            &c.axes(),
            ParamCurve::domain(c),
        ),
        Basis::Involute(c) => on_axes(
            e,
            curve::INVOLUTE,
            &c.center(),
            &c.axes(),
            ParamCurve::domain(c),
        ),
        Basis::Helix(c) => {
            e.array(4)?;
            e.uint(curve::HELIX)?;
            floats(e, &c.center())?;
            helix_axes(e, &c.axes())?;
            interval(e, ParamCurve::domain(c))
        }
        Basis::Polynomial(c) => {
            e.array(3)?;
            e.uint(curve::POLYNOMIAL)?;
            column(e, c.coefficients().iter().flatten().copied())?;
            interval(e, ParamCurve::domain(c))
        }
        Basis::Bulge(c) => {
            e.array(6)?;
            e.uint(curve::BULGE)?;
            column(e, c.vertices().iter().flatten().copied())?;
            column(e, c.bulges().iter().copied())?;
            match c.widths() {
                None => e.array(0)?,
                Some(w) => {
                    e.array(1)?;
                    column(e, w.iter().flat_map(|&(a, b)| [a, b]))?;
                }
            }
            flag(e, c.is_closed())?;
            e.float(c.start())?;
            Ok(())
        }
        Basis::Transition(c) => {
            e.array(7)?;
            e.uint(curve::TRANSITION)?;
            floats(e, &c.start())?;
            floats(e, &c.direction())?;
            e.float(c.length())?;
            law(e, &c.law())?;
            interval(e, c.window())?;
            flag(e, c.is_reversed())
        }
    }
}

/// A kind on axes: `[code, centre, axes, domain]`.
fn on_axes<const D: usize>(
    e: &mut Encoder,
    code: u64,
    center: &[f64; D],
    a: &Axes<D>,
    domain: (f64, f64),
) -> Written {
    e.array(4)?;
    e.uint(code)?;
    floats(e, center)?;
    axes(e, a)?;
    interval(e, domain)
}

/// A trimmed curve: `[11, leaf, interval]`.
fn trimmed<const D: usize>(e: &mut Encoder, t: &Trimmed<D>) -> Written {
    e.array(3)?;
    e.uint(curve::TRIMMED)?;
    leaf(e, t.basis())?;
    interval(e, t.interval())
}

/// Appends a curve kind.
///
/// # Errors
///
/// [`KindError::Encode`] for a value that is not finite where the item needs a double.
pub fn write_curve<const D: usize>(e: &mut Encoder, c: &CurveKind<D>) -> Written {
    match c {
        CurveKind::Nurbs(c) => leaf(e, &Basis::Nurbs(c.clone())),
        CurveKind::Line(c) => leaf(e, &Basis::Line(c.clone())),
        CurveKind::Circle(c) => leaf(e, &Basis::Circle(c.clone())),
        CurveKind::Ellipse(c) => leaf(e, &Basis::Ellipse(c.clone())),
        CurveKind::Hyperbola(c) => leaf(e, &Basis::Hyperbola(c.clone())),
        CurveKind::Parabola(c) => leaf(e, &Basis::Parabola(c.clone())),
        CurveKind::Helix(c) => leaf(e, &Basis::Helix(c.clone())),
        CurveKind::Polynomial(c) => leaf(e, &Basis::Polynomial(c.clone())),
        CurveKind::Involute(c) => leaf(e, &Basis::Involute(c.clone())),
        CurveKind::Bulge(c) => leaf(e, &Basis::Bulge(c.clone())),
        CurveKind::Transition(c) => leaf(e, &Basis::Transition(c.clone())),
        CurveKind::Trimmed(t) => trimmed(e, t),
        CurveKind::Composite(c) => {
            e.array(3)?;
            e.uint(curve::COMPOSITE)?;
            e.array(count(c.segments().len()))?;
            for t in c.segments() {
                trimmed(e, t)?;
            }
            column(e, c.starts().iter().copied())
        }
        CurveKind::Offset(o) => {
            e.array(4)?;
            e.uint(curve::OFFSET)?;
            trimmed(e, o.basis())?;
            offset_law(e, o.law())?;
            flag(e, o.cache().is_some())
        }
        CurveKind::OnSurface(o) => {
            e.array(5)?;
            e.uint(curve::ON_SURFACE)?;
            trimmed(e, o.pcurve())?;
            e.uint(o.surface().0)?;
            e.array(4)?;
            nurbs_fields(e, o.cache())?;
            e.float(o.error())?;
            Ok(())
        }
    }
}

/// A swept surface's placement: `[c, L by rows]`.
fn placement(e: &mut Encoder, (c, l): ([f64; 3], [[f64; 3]; 3])) -> Written {
    e.array(2)?;
    floats(e, &c)?;
    let l: Vec<f64> = l.iter().flatten().copied().collect();
    floats(e, &l)
}

/// A frame-based surface's tail: frame, domain, axial sign.
fn round_tail(e: &mut Encoder, f: &Frame<3>, domain: ParamBox, s: f64) -> Written {
    frame(e, f)?;
    param_box(e, domain)?;
    sign(e, s)
}

/// A term function's code: 0 for 1, 1 for t, 2 for cos πt, 3 for sin πt.
fn fun(f: Fun) -> u64 {
    match f {
        Fun::One => 0,
        Fun::Lin => 1,
        Fun::Cos => 2,
        Fun::Sin => 3,
    }
}

/// Appends a surface kind.
///
/// # Errors
///
/// [`KindError::Encode`] for a value that is not finite where the item needs a double;
/// [`KindError::Curve`] never (a swept surface holds its own shape).
pub fn write_surface(e: &mut Encoder, s: &SurfaceKind) -> Written {
    match s {
        SurfaceKind::Offset(o) => write_offset_surface(e, o),
        _ => write_surface_leaf(e, s),
    }
}

/// An offset surface: `[11, base, distance, domain, cached]`.
fn write_offset_surface(e: &mut Encoder, o: &geom_surface::Offset) -> Written {
    e.array(5)?;
    e.uint(surface::OFFSET)?;
    write_surface(e, o.base())?;
    e.float(o.distance())?;
    param_box(e, o.domain())?;
    flag(e, o.cache().is_some())
}

/// Every surface kind but an offset, which [`write_surface`] hands to [`write_offset_surface`].
#[allow(clippy::too_many_lines, reason = "one short arm per surface kind")]
fn write_surface_leaf(e: &mut Encoder, s: &SurfaceKind) -> Written {
    match s {
        SurfaceKind::Nurbs(n) => {
            e.array(7)?;
            e.uint(surface::NURBS)?;
            e.uint(count(n.basis_u().degree()))?;
            column(e, n.basis_u().knots().iter().copied())?;
            e.uint(count(n.basis_v().degree()))?;
            column(e, n.basis_v().knots().iter().copied())?;
            column(e, n.homogeneous().iter().flatten().copied())?;
            match n.weights() {
                None => e.array(0)?,
                Some(w) => {
                    e.array(1)?;
                    e.f64_column(w)?;
                }
            }
            Ok(())
        }
        SurfaceKind::Plane(p) => {
            let (origin, a, b) = p.vectors();
            e.array(5)?;
            e.uint(surface::PLANE)?;
            floats(e, &origin)?;
            floats(e, &a)?;
            floats(e, &b)?;
            param_box(e, p.domain())
        }
        SurfaceKind::Cylinder(c) => {
            e.array(6)?;
            e.uint(surface::CYLINDER)?;
            floats(e, &c.center())?;
            e.float(c.radius())?;
            round_tail(e, &c.frame(), c.domain(), c.axial_sign())
        }
        SurfaceKind::Cone(c) => {
            let (radius, slope) = c.radius_and_slope();
            e.array(7)?;
            e.uint(surface::CONE)?;
            floats(e, &c.center())?;
            e.float(radius)?;
            e.float(slope)?;
            round_tail(e, &c.frame(), c.domain(), c.axial_sign())
        }
        SurfaceKind::Sphere(c) => {
            e.array(6)?;
            e.uint(surface::SPHERE)?;
            floats(e, &c.center())?;
            e.float(c.radius())?;
            round_tail(e, &c.frame(), c.domain(), c.axial_sign())
        }
        SurfaceKind::Torus(c) => {
            let (major, minor) = c.radii();
            e.array(7)?;
            e.uint(surface::TORUS)?;
            floats(e, &c.center())?;
            e.float(major)?;
            e.float(minor)?;
            round_tail(e, &c.frame(), c.domain(), c.axial_sign())
        }
        SurfaceKind::Affine(a) => {
            let terms = a.terms();
            let (angular_u, angular_v) = a.angular();
            e.array(6)?;
            e.uint(surface::AFFINE)?;
            floats(e, &a.center())?;
            e.array(count(terms.len()))?;
            for (f, g, w) in terms {
                e.array(3)?;
                e.uint(fun(f))?;
                e.uint(fun(g))?;
                floats(e, &w)?;
            }
            param_box(e, a.domain())?;
            flag(e, angular_u)?;
            flag(e, angular_v)
        }
        SurfaceKind::Revolution(r) => {
            let (profile, axis, f) = r.shape()?;
            e.array(6)?;
            e.uint(surface::REVOLUTION)?;
            trimmed(e, profile)?;
            floats(e, &axis)?;
            frame(e, &f)?;
            interval(e, r.domain().0)?;
            placement(e, r.placement())
        }
        SurfaceKind::Extrusion(x) => {
            let (profile, direction) = x.shape()?;
            e.array(5)?;
            e.uint(surface::EXTRUSION)?;
            trimmed(e, profile)?;
            floats(e, &direction)?;
            interval(e, x.domain().1)?;
            placement(e, x.placement())
        }
        SurfaceKind::Ruled(r) => {
            let (first, second, sigma) = r.shape()?;
            e.array(6)?;
            e.uint(surface::RULED)?;
            trimmed(e, first)?;
            trimmed(e, second)?;
            interval(e, r.domain().1)?;
            sign(e, sigma)?;
            placement(e, r.placement())
        }
        SurfaceKind::Offset(_) => Err(KindError::Shape("an offset is written by write_surface")),
    }
}

/// The next head; an item that ends early is refused.
fn next<'a>(d: &mut Decoder<'a>, m: &mut WorkMeter<'_>) -> Result<Item<'a>, KindError> {
    d.next(m)?
        .ok_or(KindError::Shape("an item that ends early"))
}

/// An unsigned integer.
fn uint(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>) -> Result<u64, KindError> {
    match next(d, m)? {
        Item::Uint(n) => Ok(n),
        _ => Err(KindError::Shape("an unsigned integer expected")),
    }
}

/// An array's length.
fn length(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>) -> Result<u64, KindError> {
    match next(d, m)? {
        Item::Array(n) => Ok(n),
        _ => Err(KindError::Shape("an array expected")),
    }
}

/// An array of exactly `n` items.
fn fields(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>, n: u64) -> Result<(), KindError> {
    if length(d, m)? == n {
        Ok(())
    } else {
        Err(KindError::Shape("an array with another number of items"))
    }
}

/// A double.
fn float(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>) -> Result<f64, KindError> {
    match next(d, m)? {
        Item::Float(x) => Ok(x),
        _ => Err(KindError::Shape("a double expected")),
    }
}

/// A flag.
fn read_flag(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>) -> Result<bool, KindError> {
    match uint(d, m)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(KindError::Shape("a flag other than 0 or 1")),
    }
}

/// An optional double.
fn read_optional(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>) -> Result<Option<f64>, KindError> {
    match length(d, m)? {
        0 => Ok(None),
        1 => Ok(Some(float(d, m)?)),
        _ => Err(KindError::Shape("an optional value of more than one item")),
    }
}

/// One end of an interval.
fn read_end(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>, low: bool) -> Result<f64, KindError> {
    match next(d, m)? {
        Item::Float(x) => Ok(x),
        Item::Array(0) if low => Ok(f64::NEG_INFINITY),
        Item::Array(0) => Ok(f64::INFINITY),
        _ => Err(KindError::Shape("an interval's end")),
    }
}

/// An interval.
fn read_interval(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>) -> Result<(f64, f64), KindError> {
    fields(d, m, 2)?;
    Ok((read_end(d, m, true)?, read_end(d, m, false)?))
}

/// A parameter box.
fn read_box(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>) -> Result<ParamBox, KindError> {
    fields(d, m, 2)?;
    Ok((read_interval(d, m)?, read_interval(d, m)?))
}

/// `N` doubles.
fn vector<const N: usize>(
    d: &mut Decoder<'_>,
    m: &mut WorkMeter<'_>,
) -> Result<[f64; N], KindError> {
    fields(d, m, count(N))?;
    let mut out = [0.0; N];
    for x in &mut out {
        *x = float(d, m)?;
    }
    Ok(out)
}

/// An `f64` column.
fn read_column(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>) -> Result<Vec<f64>, KindError> {
    if next(d, m)? != Item::Tag(86) {
        return Err(KindError::Shape("an f64 column expected"));
    }
    match next(d, m)? {
        Item::Bytes(b) => {
            let mut out = Vec::new();
            float_column(b, &mut out, m)?;
            Ok(out)
        }
        _ => Err(KindError::Shape("an f64 column expected")),
    }
}

/// A column cut into rows of `N`.
fn rows<const N: usize>(v: &[f64]) -> Result<Vec<[f64; N]>, KindError> {
    let (whole, rest) = v.as_chunks::<N>();
    if rest.is_empty() {
        Ok(whole.to_vec())
    } else {
        Err(KindError::Shape("a column that is not whole rows"))
    }
}

/// An optional column: `[]` or `[column]`.
fn read_optional_column(
    d: &mut Decoder<'_>,
    m: &mut WorkMeter<'_>,
) -> Result<Option<Vec<f64>>, KindError> {
    match length(d, m)? {
        0 => Ok(None),
        1 => Ok(Some(read_column(d, m)?)),
        _ => Err(KindError::Shape("an optional column of more than one item")),
    }
}

/// An axial sign or σ.
fn read_sign(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>) -> Result<f64, KindError> {
    match next(d, m)? {
        Item::Uint(1) => Ok(1.0),
        Item::Nint(0) => Ok(-1.0),
        _ => Err(KindError::Shape("a sign other than 1 or −1")),
    }
}

/// A frame.
fn read_frame<const D: usize>(
    d: &mut Decoder<'_>,
    m: &mut WorkMeter<'_>,
) -> Result<Frame<D>, KindError> {
    fields(d, m, 2)?;
    let x = vector::<D>(d, m)?;
    let normal = vector::<3>(d, m)?;
    Ok(Frame::new(x, normal)?)
}

/// Axes.
fn read_axes<const D: usize>(
    d: &mut Decoder<'_>,
    m: &mut WorkMeter<'_>,
) -> Result<Axes<D>, KindError> {
    let n = length(d, m)?;
    match (n, uint(d, m)?) {
        (4, 0) => Ok(Axes::Frame {
            frame: read_frame(d, m)?,
            a: float(d, m)?,
            b: float(d, m)?,
        }),
        (3, 1) => Ok(Axes::Vectors {
            u: vector(d, m)?,
            v: vector(d, m)?,
        }),
        _ => Err(KindError::Shape("axes of another form")),
    }
}

/// Helix axes.
fn read_helix_axes<const D: usize>(
    d: &mut Decoder<'_>,
    m: &mut WorkMeter<'_>,
) -> Result<HelixAxes<D>, KindError> {
    fields(d, m, 4)?;
    match uint(d, m)? {
        0 => Ok(HelixAxes::Frame {
            frame: read_frame(d, m)?,
            radius: float(d, m)?,
            pitch: float(d, m)?,
        }),
        1 => Ok(HelixAxes::Vectors {
            u: vector(d, m)?,
            v: vector(d, m)?,
            w: vector(d, m)?,
        }),
        _ => Err(KindError::Shape("helix axes of another form")),
    }
}

/// A NURBS curve's four fields.
fn read_nurbs<const D: usize>(
    d: &mut Decoder<'_>,
    m: &mut WorkMeter<'_>,
) -> Result<geom_nurbs::Curve<D>, KindError> {
    let degree =
        usize::try_from(uint(d, m)?).map_err(|_| KindError::Shape("a degree past usize"))?;
    let knots = read_column(d, m)?;
    let points = rows::<D>(&read_column(d, m)?)?;
    let weights = read_optional_column(d, m)?;
    let basis = geom_nurbs::Basis::new(degree, knots)?;
    Ok(geom_nurbs::Curve::from_homogeneous(basis, points, weights)?)
}

/// A curvature law.
fn read_law(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>) -> Result<CurvatureLaw, KindError> {
    let n = length(d, m)?;
    let code = uint(d, m)?;
    let wrong = KindError::Shape("a curvature law of another form");
    match (code, n) {
        (1..=5, 3) => {
            let (start, end) = (float(d, m)?, float(d, m)?);
            Ok(match code {
                1 => CurvatureLaw::Clothoid { start, end },
                2 => CurvatureLaw::Helmert { start, end },
                3 => CurvatureLaw::Bloss { start, end },
                4 => CurvatureLaw::Cosine { start, end },
                _ => CurvatureLaw::Sine { start, end },
            })
        }
        (6, 5) => Ok(CurvatureLaw::CantCoupled {
            start: float(d, m)?,
            end: float(d, m)?,
            height: float(d, m)?,
            cant: float(d, m)?,
        }),
        (7, 3) => {
            fields(d, m, 8)?;
            let mut terms = [None; 8];
            for t in &mut terms {
                *t = read_optional(d, m)?;
            }
            Ok(CurvatureLaw::Polynomial {
                terms,
                from: float(d, m)?,
            })
        }
        (8, 5) => Ok(CurvatureLaw::CosineSpiral {
            constant: read_optional(d, m)?,
            cosine: float(d, m)?,
            length: float(d, m)?,
            from: float(d, m)?,
        }),
        (9, 6) => Ok(CurvatureLaw::SineSpiral {
            constant: read_optional(d, m)?,
            linear: read_optional(d, m)?,
            sine: float(d, m)?,
            length: float(d, m)?,
            from: float(d, m)?,
        }),
        _ => Err(wrong),
    }
}

/// An offset law.
fn read_offset_law(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>) -> Result<OffsetLaw, KindError> {
    let n = length(d, m)?;
    match (uint(d, m)?, n) {
        (1, 2) => Ok(OffsetLaw::Planar {
            distance: float(d, m)?,
        }),
        (2, 3) => Ok(OffsetLaw::Reference {
            distance: float(d, m)?,
            direction: vector(d, m)?,
        }),
        (3, 2) => Ok(OffsetLaw::Distances(
            rows::<3>(&read_column(d, m)?)?
                .into_iter()
                .map(|[at, lateral, vertical]| Station {
                    at,
                    lateral,
                    vertical,
                })
                .collect(),
        )),
        _ => Err(KindError::Shape("an offset law of another form")),
    }
}

/// The fields of a leaf kind whose code and length are read.
#[allow(clippy::too_many_lines, reason = "one short arm per leaf kind")]
fn read_leaf_fields<const D: usize>(
    code: u64,
    n: u64,
    d: &mut Decoder<'_>,
    m: &mut WorkMeter<'_>,
) -> Result<Basis<D>, KindError> {
    let want = |k: u64| {
        if n == k {
            Ok(())
        } else {
            Err(KindError::Shape(
                "a curve kind with another number of fields",
            ))
        }
    };
    Ok(match code {
        curve::NURBS => {
            want(5)?;
            Basis::Nurbs(read_nurbs(d, m)?)
        }
        curve::LINE => {
            want(4)?;
            Basis::Line(Line::new(
                vector(d, m)?,
                vector(d, m)?,
                read_interval(d, m)?,
            )?)
        }
        curve::CIRCLE => {
            want(5)?;
            Basis::Circle(Circle::new(
                vector(d, m)?,
                float(d, m)?,
                read_frame(d, m)?,
                read_interval(d, m)?,
            )?)
        }
        curve::ELLIPSE => {
            want(4)?;
            Basis::Ellipse(Ellipse::new(
                vector(d, m)?,
                read_axes(d, m)?,
                read_interval(d, m)?,
            )?)
        }
        curve::HYPERBOLA => {
            want(4)?;
            Basis::Hyperbola(Hyperbola::new(
                vector(d, m)?,
                read_axes(d, m)?,
                read_interval(d, m)?,
            )?)
        }
        curve::PARABOLA => {
            want(4)?;
            Basis::Parabola(Parabola::new(
                vector(d, m)?,
                read_axes(d, m)?,
                read_interval(d, m)?,
            )?)
        }
        curve::HELIX => {
            want(4)?;
            Basis::Helix(Helix::new(
                vector(d, m)?,
                read_helix_axes(d, m)?,
                read_interval(d, m)?,
            )?)
        }
        curve::POLYNOMIAL => {
            want(3)?;
            let coefficients = rows::<D>(&read_column(d, m)?)?;
            Basis::Polynomial(Polynomial::new(coefficients, read_interval(d, m)?)?)
        }
        curve::INVOLUTE => {
            want(4)?;
            Basis::Involute(Involute::with_axes(
                vector(d, m)?,
                read_axes(d, m)?,
                read_interval(d, m)?,
            )?)
        }
        curve::BULGE => {
            want(6)?;
            let vertices = rows::<D>(&read_column(d, m)?)?;
            let bulges = read_column(d, m)?;
            let widths = read_optional_column(d, m)?
                .map(|w| rows::<2>(&w))
                .transpose()?
                .map(|w| w.into_iter().map(|[a, b]| (a, b)).collect());
            let closed = read_flag(d, m)?;
            let start = float(d, m)?;
            Basis::Bulge(BulgePolyline::starting_at(
                vertices, bulges, widths, closed, start,
            )?)
        }
        curve::TRANSITION => {
            want(7)?;
            let start = vector(d, m)?;
            let direction = vector(d, m)?;
            let length = float(d, m)?;
            let law = read_law(d, m)?;
            let window = read_interval(d, m)?;
            let reversed = read_flag(d, m)?;
            Basis::Transition(
                Transition::new(start, direction, length, law, m)?.segment(window, reversed)?,
            )
        }
        _ => return Err(KindError::Shape("not a leaf curve kind")),
    })
}

/// A trimmed curve.
fn read_trimmed<const D: usize>(
    d: &mut Decoder<'_>,
    m: &mut WorkMeter<'_>,
) -> Result<Trimmed<D>, KindError> {
    fields(d, m, 3)?;
    if uint(d, m)? != curve::TRIMMED {
        return Err(KindError::Shape("a trimmed curve expected"));
    }
    let n = length(d, m)?;
    let code = uint(d, m)?;
    let basis = read_leaf_fields(code, n, d, m)?;
    Ok(Trimmed::new(basis, read_interval(d, m)?)?)
}

/// Reads a curve kind.
///
/// # Errors
///
/// [`KindError::Shape`] for an item that is not a curve kind of this dimension's form,
/// [`KindError::Curve`] or [`KindError::Nurbs`] for values its constructor refuses, and the
/// decoder's and the meter's errors.
pub fn read_curve<const D: usize>(
    d: &mut Decoder<'_>,
    m: &mut WorkMeter<'_>,
) -> Result<CurveKind<D>, KindError> {
    let n = length(d, m)?;
    let code = uint(d, m)?;
    let want = |k: u64| {
        if n == k {
            Ok(())
        } else {
            Err(KindError::Shape(
                "a curve kind with another number of fields",
            ))
        }
    };
    Ok(match code {
        curve::TRIMMED => {
            want(3)?;
            let n = length(d, m)?;
            let code = uint(d, m)?;
            let basis = read_leaf_fields(code, n, d, m)?;
            CurveKind::Trimmed(Trimmed::new(basis, read_interval(d, m)?)?)
        }
        curve::COMPOSITE => {
            want(3)?;
            let k = length(d, m)?;
            let mut segments = Vec::new();
            for _ in 0..k {
                segments.push(read_trimmed(d, m)?);
            }
            CurveKind::Composite(Composite::from_starts(segments, read_column(d, m)?)?)
        }
        curve::OFFSET => {
            want(4)?;
            let basis = read_trimmed(d, m)?;
            let law = read_offset_law(d, m)?;
            let offset = geom_curve::Offset::new(basis, law)?;
            CurveKind::Offset(if read_flag(d, m)? {
                offset.cached(m)?
            } else {
                offset
            })
        }
        curve::ON_SURFACE => {
            want(5)?;
            let pcurve = read_trimmed::<2>(d, m)?;
            let surface = SurfaceRef(uint(d, m)?);
            fields(d, m, 4)?;
            let cache = read_nurbs(d, m)?;
            CurveKind::OnSurface(OnSurface::new(pcurve, surface, cache, float(d, m)?)?)
        }
        _ => match read_leaf_fields(code, n, d, m)? {
            Basis::Nurbs(c) => CurveKind::Nurbs(c),
            Basis::Line(c) => CurveKind::Line(c),
            Basis::Circle(c) => CurveKind::Circle(c),
            Basis::Ellipse(c) => CurveKind::Ellipse(c),
            Basis::Hyperbola(c) => CurveKind::Hyperbola(c),
            Basis::Parabola(c) => CurveKind::Parabola(c),
            Basis::Helix(c) => CurveKind::Helix(c),
            Basis::Polynomial(c) => CurveKind::Polynomial(c),
            Basis::Involute(c) => CurveKind::Involute(c),
            Basis::Bulge(c) => CurveKind::Bulge(c),
            Basis::Transition(c) => CurveKind::Transition(c),
        },
    })
}

/// A swept surface's placement.
fn read_placement(
    d: &mut Decoder<'_>,
    m: &mut WorkMeter<'_>,
) -> Result<([f64; 3], [[f64; 3]; 3]), KindError> {
    fields(d, m, 2)?;
    let c = vector::<3>(d, m)?;
    let [a, b, cc, e, f, g, h, i, j] = vector::<9>(d, m)?;
    Ok((c, [[a, b, cc], [e, f, g], [h, i, j]]))
}

/// A term function.
fn read_fun(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>) -> Result<Fun, KindError> {
    match uint(d, m)? {
        0 => Ok(Fun::One),
        1 => Ok(Fun::Lin),
        2 => Ok(Fun::Cos),
        3 => Ok(Fun::Sin),
        _ => Err(KindError::Shape("a term function from 0 to 3")),
    }
}

/// A frame-based surface's tail: frame, domain, axial sign.
fn read_round_tail(
    d: &mut Decoder<'_>,
    m: &mut WorkMeter<'_>,
) -> Result<(Frame<3>, ParamBox, bool), KindError> {
    Ok((read_frame(d, m)?, read_box(d, m)?, read_sign(d, m)? < 0.0))
}

/// Reads a surface kind.
///
/// # Errors
///
/// [`KindError::Shape`] for an item that is not a surface kind's form, [`KindError::Curve`] or
/// [`KindError::Nurbs`] for values its constructor refuses, and the decoder's and the meter's
/// errors.
pub fn read_surface(d: &mut Decoder<'_>, m: &mut WorkMeter<'_>) -> Result<SurfaceKind, KindError> {
    let n = length(d, m)?;
    match uint(d, m)? {
        surface::OFFSET => read_offset_surface(n, d, m),
        code => read_surface_leaf(code, n, d, m),
    }
}

/// An offset surface's fields, its code and length read.
fn read_offset_surface(
    n: u64,
    d: &mut Decoder<'_>,
    m: &mut WorkMeter<'_>,
) -> Result<SurfaceKind, KindError> {
    if n != 5 {
        return Err(KindError::Shape(
            "a surface kind with another number of fields",
        ));
    }
    let base = read_surface(d, m)?;
    let distance = float(d, m)?;
    let domain = read_box(d, m)?;
    let offset = geom_surface::Offset::new(base, distance)?.restricted(domain)?;
    Ok(SurfaceKind::Offset(if read_flag(d, m)? {
        offset.cached(m)?
    } else {
        offset
    }))
}

/// The fields of every surface kind but an offset, its code and length read.
#[allow(clippy::too_many_lines, reason = "one short arm per surface kind")]
fn read_surface_leaf(
    code: u64,
    n: u64,
    d: &mut Decoder<'_>,
    m: &mut WorkMeter<'_>,
) -> Result<SurfaceKind, KindError> {
    let want = |k: u64| {
        if n == k {
            Ok(())
        } else {
            Err(KindError::Shape(
                "a surface kind with another number of fields",
            ))
        }
    };
    Ok(match code {
        surface::NURBS => {
            want(7)?;
            let du = usize::try_from(uint(d, m)?)
                .map_err(|_| KindError::Shape("a degree past usize"))?;
            let ku = read_column(d, m)?;
            let dv = usize::try_from(uint(d, m)?)
                .map_err(|_| KindError::Shape("a degree past usize"))?;
            let kv = read_column(d, m)?;
            let points = rows::<3>(&read_column(d, m)?)?;
            let weights = read_optional_column(d, m)?;
            SurfaceKind::Nurbs(geom_nurbs::Surface::from_homogeneous(
                geom_nurbs::Basis::new(du, ku)?,
                geom_nurbs::Basis::new(dv, kv)?,
                points,
                weights,
            )?)
        }
        surface::PLANE => {
            want(5)?;
            SurfaceKind::Plane(Plane::new(
                vector(d, m)?,
                vector(d, m)?,
                vector(d, m)?,
                read_box(d, m)?,
            )?)
        }
        surface::CYLINDER => {
            want(6)?;
            let (center, radius) = (vector(d, m)?, float(d, m)?);
            let (f, domain, flipped) = read_round_tail(d, m)?;
            let c = Cylinder::new(center, radius, f, domain)?;
            SurfaceKind::Cylinder(if flipped { c.flipped_axially()? } else { c })
        }
        surface::CONE => {
            want(7)?;
            let (center, radius, slope) = (vector(d, m)?, float(d, m)?, float(d, m)?);
            let (f, domain, flipped) = read_round_tail(d, m)?;
            let c = Cone::new(center, radius, slope, f, domain)?;
            SurfaceKind::Cone(if flipped { c.flipped_axially()? } else { c })
        }
        surface::SPHERE => {
            want(6)?;
            let (center, radius) = (vector(d, m)?, float(d, m)?);
            let (f, domain, flipped) = read_round_tail(d, m)?;
            let c = Sphere::new(center, radius, f, domain)?;
            SurfaceKind::Sphere(if flipped { c.flipped_axially()? } else { c })
        }
        surface::TORUS => {
            want(7)?;
            let (center, major, minor) = (vector(d, m)?, float(d, m)?, float(d, m)?);
            let (f, domain, flipped) = read_round_tail(d, m)?;
            let c = Torus::new(center, major, minor, f, domain)?;
            SurfaceKind::Torus(if flipped { c.flipped_axially()? } else { c })
        }
        surface::AFFINE => {
            want(6)?;
            let center = vector(d, m)?;
            let k = length(d, m)?;
            let mut terms = Vec::new();
            for _ in 0..k {
                fields(d, m, 3)?;
                terms.push((read_fun(d, m)?, read_fun(d, m)?, vector(d, m)?));
            }
            let domain = read_box(d, m)?;
            let angular = (read_flag(d, m)?, read_flag(d, m)?);
            SurfaceKind::Affine(AffineSurface::from_terms(center, &terms, domain, angular)?)
        }
        surface::REVOLUTION => {
            want(6)?;
            let profile = read_trimmed(d, m)?;
            let axis = vector(d, m)?;
            let f = read_frame(d, m)?;
            let angles = read_interval(d, m)?;
            let (c, l) = read_placement(d, m)?;
            SurfaceKind::Revolution(Revolution::new(profile, axis, f, angles)?.placed(c, l)?)
        }
        surface::EXTRUSION => {
            want(5)?;
            let profile = read_trimmed(d, m)?;
            let direction = vector(d, m)?;
            let along = read_interval(d, m)?;
            let (c, l) = read_placement(d, m)?;
            SurfaceKind::Extrusion(Extrusion::new(profile, direction, along)?.placed(c, l)?)
        }
        surface::RULED => {
            want(6)?;
            let first = read_trimmed(d, m)?;
            let second = read_trimmed(d, m)?;
            let along = read_interval(d, m)?;
            let sigma = read_sign(d, m)?;
            let (c, l) = read_placement(d, m)?;
            SurfaceKind::Ruled(
                Ruled::new(first, second, along)?
                    .with_sigma(sigma)?
                    .placed(c, l)?,
            )
        }
        _ => return Err(KindError::Shape("an unknown surface kind")),
    })
}
