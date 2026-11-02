//! The refutation checker for curved bodies (chapter 28.4, the third check behind `geom-check`;
//! piece G178): a body's curves, pcurves and surfaces sampled densely in 400-bit fixed point and
//! exact rationals, looking for a fault, sharing no code with `geom-intersect` or `geom-check`.
//!
//! What it does: [`refute`] reads each curve, pcurve and surface of a body into its definition as
//! its kind's documentation states it, from the doubles the kind stores, and evaluates it in
//! [`crate::fixed`]'s fixed point, NURBS exactly by [`crate::nurbs_reference`]. Then it looks for
//! what chapter 28.4 names, each against the bound the body is recorded with:
//!
//! - **A vertex off its curve:** farther than the bound from its edge's curve at that end.
//! - **A vertex off its surface:** farther than the bound from its face's whole surface, by the
//!   surface's closed-form distance (planes and the round kinds).
//! - **An edge off its surface:** one of [`SAMPLES`] + 1 points of its curve farther than the bound
//!   from the whole surface of one of its faces.
//! - **A pcurve off its curve:** S(p(t)) farther than the bound from C(t) at one of the samples.
//! - **Two faces crossing away from a shared edge:** along each of [`ISO`] parameter lines each way
//!   across one face, at [`ALONG`] + 1 points, the other face's implicit equation changes sign
//!   between two samples that both clear [`LEVEL_BITS`]; the change is bisected to a point inside
//!   both faces and clear of every edge of each by the overlap the bound allows two surfaces meeting
//!   at that angle: bound / sin(θ/2), and the margin.
//!
//! [`Refutation::found`] lists each find, with the error it measured. [`Refutation::unjudged`]
//! lists what the checker has no definition or region for, so a test can see what it covered.
//!
//! A face's inside: a planar face's loops are chords in an orthonormal frame of its plane, each
//! with its sagitta (the farthest its arc can leave it), and a point well clear of every chord is
//! inside where its winding number is not zero. A face on a cylinder, cone, sphere or torus whose
//! every edge lies on its domain's boundary or on the seam of a full turn is its whole domain: a
//! point is inside where its axial coordinate is inside the domain's, cut back by how far the
//! boundary edges stray and a step, and well clear of every edge. Other faces have no region here
//! and are unjudged for crossings.
//!
//! Guarantees:
//!
//! - **Only ever "found invalid".** Each find is proven of the stored doubles. A distance is a lower
//!   bound, the surface's whole closed form, of which a face is a part, compared with the bound plus
//!   [`MARGIN_BITS`], which is far above the fixed point's error. A crossing is a sign change of a
//!   continuous function at two samples clear of [`LEVEL_BITS`], at a point shown inside both faces by
//!   the regions above. An empty list says nothing was found, never that the body is valid.
//! - **Independent.** No call into `geom-intersect` or `geom-check`, and no kind's own evaluation:
//!   every definition is written from its kind's documentation and read from the doubles it stores,
//!   as the law catalogue's truths are (`tests/truth`); a face's inside is decided from its own
//!   loops.
//! - **Deterministic.** Integer arithmetic, fixed samples, no clock and no random number.
//!
//! Not judged here: an Exact record on curved geometry (a rule of the record, which group 1 of
//! chapter 8.3 owns, with no error to measure), points where three planes meet (the flat second
//! checker's, `crate::second_check`), a face against itself, crossings with a face on a NURBS
//! surface or one with a NURBS edge (no closed-form equation or sagitta here yet), and a crossing
//! nearer an edge than the overlap a bound allows there, which a crease within the bound can make.
//!
//! Method: chapter 28.4's refutation checker (`projects/engines/geometry/architecture/28-testing.md`),
//! on chapter 8.3's rules (`projects/engines/geometry/architecture/08-topology.md`) and the kinds'
//! definitions of chapter 7 (`geom_curve::conic`, `geom_curve::line`, `geom_surface::analytic`);
//! the winding number by Sunday's crossing rule (D. Sunday, "Practical Geometry Algorithms", 2021,
//! chapter 5, <https://geomalgorithms.com/a03-_inclusion.html>); the chord error of linear
//! interpolation, at most h²/8 · max|C''| (de Boor, *A Practical Guide to Splines*, 2001, chapter
//! II, <https://doi.org/10.1007/978-1-4612-6333-3>); the distance to a surface of revolution in the
//! meridian plane through the point.
//!
//! Cost: per edge, [`SAMPLES`] + 1 evaluations and a distance to each face's surface; per fin with
//! a pcurve, as many surface evaluations; per pair of faces whose stored boxes meet, 2·[`ISO`] lines
//! of [`ALONG`] + 1 points each way, an equation per point, and [`BISECT`] steps at each sign
//! change. Sines and cosines are kept by their argument. No recursion.

// The lint is for fixed-width arithmetic, which can overflow. Every operator here is on dashu's
// arbitrary-precision integers and rationals, which cannot, or on doubles that are only read, and
// every division is by a value shown nonzero beside it.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "arbitrary-precision arithmetic, which cannot overflow; divisors are shown nonzero"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': a to d points or vectors, c a centre, e a direction, r and k radii and slopes, s a sagitta, t, u and v parameters, w a difference"
)]

use std::collections::BTreeMap;

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_curve::{Axes, CurveKind, ParamCurve};
use geom_num::Exactness;
use geom_surface::{ParamSurface, SurfaceKind};
use geom_topo::ix::Arena;
use geom_topo::{Arenas, Body, Edge, Face, Fin, Ix, PointKind, Sense, Vertex};

use crate::fixed::{self, F, cos_sin_pi, div, mul, one, sqrt};
use crate::nurbs_reference::{Patch, Spline};

/// The chords each span is sampled in.
pub const SAMPLES: u32 = 64;

/// The parameter lines across a face each way, for crossings.
pub const ISO: u32 = 8;

/// The chords each parameter line is sampled in, for crossings.
pub const ALONG: u32 = 32;

/// The halvings of a sign change.
pub const BISECT: u32 = 64;

/// What a distance must clear the bound by to be a find: 2^−`MARGIN_BITS` of a step, far above
/// the fixed point's error of about 2^−300 on values of up to 2^40 steps.
pub const MARGIN_BITS: usize = 20;

/// What an implicit equation must clear at a sample for its sign to count: 2^−`LEVEL_BITS`.
pub const LEVEL_BITS: usize = 100;

/// A point in the fixed point.
type P3 = [IBig; 3];

/// One fault found, by row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// A vertex off its edge's curve at that end.
    VertexOffCurve {
        /// The vertex.
        vertex: u32,
        /// The edge.
        edge: u32,
    },
    /// A vertex off the surface of a face it bounds.
    VertexOffSurface {
        /// The vertex.
        vertex: u32,
        /// The face.
        face: u32,
    },
    /// An edge off the surface of one of its faces.
    EdgeOffSurface {
        /// The edge.
        edge: u32,
        /// The face.
        face: u32,
    },
    /// A fin's pcurve, mapped by its face's surface, off its edge's curve.
    PcurveOffCurve {
        /// The fin.
        fin: u32,
    },
    /// Two faces crossing away from anything they share.
    FacesCross {
        /// The lower face row.
        first: u32,
        /// The higher.
        second: u32,
    },
}

/// One find: the fault, and the distance measured where it is a distance, in steps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Find {
    /// What was found.
    pub fault: Fault,
    /// The error measured, against the body's bound; `None` for a crossing.
    pub error: Option<f64>,
}

/// What the checker could not judge, by row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unjudged {
    /// An edge on a curve kind without a definition here.
    Curve(u32),
    /// A fin whose pcurve has no definition here, or another span than its edge.
    Pcurve(u32),
    /// A face on a surface kind without a definition here.
    Surface(u32),
    /// A face without a region here, left out of the crossing search.
    Region(u32),
}

/// What [`refute`] found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Refutation {
    /// Every fault found.
    pub found: Vec<Find>,
    /// Everything left unjudged.
    pub unjudged: Vec<Unjudged>,
}

impl Refutation {
    /// The faults found, without their errors.
    #[must_use]
    pub fn faults(&self) -> Vec<Fault> {
        self.found.iter().map(|f| f.fault).collect()
    }
}

/// `f` of each pair of coordinates.
fn zipped<const D: usize>(
    a: &[IBig; D],
    b: &[IBig; D],
    f: impl Fn(&IBig, &IBig) -> IBig,
) -> [IBig; D] {
    let mut it = a.iter().zip(b).map(|(x, y)| f(x, y));
    core::array::from_fn(|_| it.next().unwrap_or_default())
}

/// a − b.
fn minus<const D: usize>(a: &[IBig; D], b: &[IBig; D]) -> [IBig; D] {
    zipped(a, b, |x, y| x - y)
}

/// a + b.
fn plus<const D: usize>(a: &[IBig; D], b: &[IBig; D]) -> [IBig; D] {
    zipped(a, b, |x, y| x + y)
}

/// k·a.
fn times<const D: usize>(k: &IBig, a: &[IBig; D]) -> [IBig; D] {
    a.clone().map(|x| mul(k, &x))
}

/// a · b.
fn dot<const D: usize>(a: &[IBig; D], b: &[IBig; D]) -> IBig {
    // Integers, summed in coordinate order; their sum is exact in any order.
    a.iter().zip(b).fold(IBig::ZERO, |s, (x, y)| s + mul(x, y))
}

/// a × b.
fn cross(a: &P3, b: &P3) -> P3 {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    [
        mul(ay, bz) - mul(az, by),
        mul(az, bx) - mul(ax, bz),
        mul(ax, by) - mul(ay, bx),
    ]
}

/// |a|.
fn norm<const D: usize>(a: &[IBig; D]) -> Option<IBig> {
    sqrt(&dot(a, a))
}

/// a / |a|; `None` for a zero vector.
fn unit(a: &P3) -> Option<P3> {
    let n = norm(a)?;
    let [x, y, z] = a;
    Some([div(x, &n)?, div(y, &n)?, div(z, &n)?])
}

/// A double in the fixed point.
fn fx(x: f64) -> Option<IBig> {
    fixed::of(x)
}

/// A vector of doubles in the fixed point.
fn fv<const D: usize>(v: [f64; D]) -> Option<[IBig; D]> {
    let parts: Option<Vec<IBig>> = v.into_iter().map(fx).collect();
    parts?.try_into().ok()
}

/// The first D coordinates of a 3D vector.
fn first<const D: usize>(v: &P3) -> [IBig; D] {
    let mut it = v.iter().cloned();
    core::array::from_fn(|_| it.next().unwrap_or_default())
}

/// A frame's unit vectors x̂, ŷ, n̂, by `geom_curve::frame`'s definition: x̂ along (n × x) × n, ŷ
/// along n × x, n̂ along n; x is lifted to 3D with a zero third coordinate.
fn frame_axes<const D: usize>(x: [f64; D], n: [f64; 3]) -> Option<[P3; 3]> {
    let x: [IBig; D] = fv(x)?;
    let mut it = x.into_iter();
    let x: P3 = core::array::from_fn(|_| it.next().unwrap_or_default());
    let n = fv(n)?;
    let nx = cross(&n, &x);
    Some([unit(&cross(&nx, &n))?, unit(&nx)?, unit(&n)?])
}

/// cos πt and sin πt, kept by t.
#[derive(Default)]
struct Trig(BTreeMap<IBig, (IBig, IBig)>);

impl Trig {
    /// cos πt and sin πt.
    fn at(&mut self, t: &IBig) -> (IBig, IBig) {
        self.0
            .entry(t.clone())
            .or_insert_with(|| cos_sin_pi(t))
            .clone()
    }

    /// Keeps cos πt and sin πt at each of [`samples`]' parameters over `span`, by turning the first
    /// by the step's angle each time (cos(a + h) = cos a·cos h − sin a·sin h, and the sine's
    /// formula): two series in place of n + 1. Each turn adds a cut of 2^−400 or two, so after
    /// [`SAMPLES`] turns the values are within 2^−390 of the series', far inside [`MARGIN_BITS`].
    fn even(&mut self, span: &(IBig, IBig), n: u32) {
        let ts = samples(span, n);
        let Some(first) = ts.first() else { return };
        let count = IBig::from(n.max(1));
        let (ch, sh) = cos_sin_pi(&((&span.1 - &span.0) / &count));
        let (mut c, mut s) = self.at(first);
        for t in ts.iter().skip(1) {
            let turned = (mul(&c, &ch) - mul(&s, &sh), mul(&s, &ch) + mul(&c, &sh));
            (c, s) = turned;
            self.0
                .entry(t.clone())
                .or_insert_with(|| (c.clone(), s.clone()));
        }
    }
}

/// A curve's definition, D its dimension.
#[derive(Clone, Debug)]
enum Curve<const D: usize> {
    /// o + t·d.
    Line([IBig; D], [IBig; D]),
    /// c + cos πt·u + sin πt·v: a circle (u = r·x̂, v = r·ŷ) or an ellipse.
    Trig([IBig; D], [IBig; D], [IBig; D]),
    /// A NURBS curve, exactly.
    Nurbs(Spline),
}

/// A curve kind's definition, with its span in the fixed point; `None` for a kind without one
/// here, or one whose span is not finite.
fn curve_of<const D: usize>(kind: &CurveKind<D>) -> Option<(Curve<D>, (IBig, IBig))> {
    let (t0, t1) = kind.domain();
    let span = (fx(t0)?, fx(t1)?);
    let curve = match kind {
        CurveKind::Line(l) => Curve::Line(fv(l.origin())?, fv(l.direction())?),
        CurveKind::Circle(c) => {
            let [x, y, _] = frame_axes(c.frame().x(), c.frame().normal())?;
            let r = fx(c.radius())?;
            Curve::Trig(
                fv(c.center())?,
                first(&times(&r, &x)),
                first(&times(&r, &y)),
            )
        }
        CurveKind::Ellipse(e) => match e.axes() {
            Axes::Frame { frame, a, b } => {
                let [x, y, _] = frame_axes(frame.x(), frame.normal())?;
                Curve::Trig(
                    fv(e.center())?,
                    first(&times(&fx(a)?, &x)),
                    first(&times(&fx(b)?, &y)),
                )
            }
            Axes::Vectors { u, v } => Curve::Trig(fv(e.center())?, fv(u)?, fv(v)?),
        },
        CurveKind::Nurbs(n) => {
            let b = n.basis();
            let points: Vec<&[f64]> = n.homogeneous().iter().map(|p| &p[..]).collect();
            let spline = match n.weights() {
                Some(w) => Spline::new_rational(b.degree(), b.knots(), &points, w),
                None => Spline::new(b.degree(), b.knots(), &points),
            };
            Curve::Nurbs(spline.ok()?)
        }
        _ => return None,
    };
    Some((curve, span))
}

/// The fixed-point parameter clamped into a NURBS domain, exactly.
fn clamped(t: &IBig, (lo, hi): &(RBig, RBig)) -> RBig {
    let q = fixed::rational(t);
    if q < *lo {
        lo.clone()
    } else if q > *hi {
        hi.clone()
    } else {
        q
    }
}

impl<const D: usize> Curve<D> {
    /// The point at t; `None` where a NURBS curve cannot be evaluated.
    fn point(&self, t: &IBig, trig: &mut Trig) -> Option<[IBig; D]> {
        match self {
            Curve::Line(o, d) => Some(plus(o, &times(t, d))),
            Curve::Trig(c, u, v) => {
                let (co, si) = trig.at(t);
                Some(plus(c, &plus(&times(&co, u), &times(&si, v))))
            }
            Curve::Nurbs(s) => {
                let p = s.point(&clamped(t, &s.domain().ok()?)).ok()?;
                let mut it = p.iter().map(fixed::of_rational);
                Some(core::array::from_fn(|_| it.next().unwrap_or_default()))
            }
        }
    }

    /// A bound on how far the curve leaves the chord between its points at t and t + h: h²/8 ·
    /// max|C''|, 0 for a line, π²·√(|u|² + |v|²) for C'' of the trigonometric form; `None` for
    /// NURBS.
    fn sagitta(&self, h: &IBig) -> Option<IBig> {
        match self {
            Curve::Line(..) => Some(IBig::ZERO),
            Curve::Trig(_, u, v) => {
                let pi = fixed::pi();
                let reach = sqrt(&(dot(u, u) + dot(v, v)))?;
                // One unit more than the formula, for the fixed point's cuts in it.
                Some((mul(&mul(&mul(h, h), &mul(&pi, &pi)), &reach) >> 3) + IBig::ONE)
            }
            Curve::Nurbs(_) => None,
        }
    }
}

/// `n` + 1 parameters evenly over a span, its ends exactly.
fn samples((t0, t1): &(IBig, IBig), n: u32) -> Vec<IBig> {
    let count = IBig::from(n);
    (0..=n)
        .map(|j| {
            if j == n {
                t1.clone()
            } else {
                t0 + (t1 - t0) * IBig::from(j) / &count
            }
        })
        .collect()
}

/// The shapes of the round kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Round {
    /// c + r·e(u) + s·v·n̂.
    Cylinder,
    /// c + (r₀ + k·v)·e(u) + s·v·n̂.
    Cone,
    /// c + r·cos πv·e(u) + s·r·sin πv·n̂.
    Sphere,
    /// c + (R + r·cos πv)·e(u) + s·r·sin πv·n̂.
    Torus,
}

/// A surface's definition.
#[derive(Clone, Debug)]
enum Surface {
    /// c + u·a + v·b, with an orthonormal frame e₁, e₂ of the plane and its unit normal m.
    Plane {
        /// c, a and b.
        abc: [P3; 3],
        /// e₁, e₂ and m.
        frame: [P3; 3],
    },
    /// A round kind: its shape, centre c, x̂, ŷ, s·n̂, its two lengths (r and 0, r₀ and k, r and 0,
    /// R and r), and its domain in the fixed point.
    Round {
        /// Which.
        shape: Round,
        /// c, x̂, ŷ and s·n̂.
        axes: [P3; 4],
        /// The two lengths.
        lengths: [IBig; 2],
        /// ((u₀, u₁), (v₀, v₁)).
        domain: [(IBig, IBig); 2],
    },
    /// A NURBS surface, exactly.
    Nurbs(Patch),
}

/// A round kind's centre, frame, axial sign and lengths, read into its definition.
fn round(
    shape: Round,
    (center, frame, sign): ([f64; 3], geom_curve::Frame<3>, f64),
    lengths: [f64; 2],
    domain: ((f64, f64), (f64, f64)),
) -> Option<Surface> {
    let [x, y, n] = frame_axes(frame.x(), frame.normal())?;
    let ((u0, u1), (v0, v1)) = domain;
    let [l0, l1] = lengths;
    Some(Surface::Round {
        shape,
        axes: [fv(center)?, x, y, times(&fx(sign)?, &n)],
        lengths: [fx(l0)?, fx(l1)?],
        domain: [(fx(u0)?, fx(u1)?), (fx(v0)?, fx(v1)?)],
    })
}

/// A surface kind's definition; `None` for a kind without one here.
fn surface_of(kind: &SurfaceKind) -> Option<Surface> {
    let domain = kind.domain();
    match kind {
        SurfaceKind::Plane(p) => {
            let (c, a, b) = p.vectors();
            let (c, a, b) = (fv(c)?, fv(a)?, fv(b)?);
            let m = unit(&cross(&a, &b))?;
            let e1 = unit(&a)?;
            let e2 = cross(&m, &e1);
            Some(Surface::Plane {
                abc: [c, a, b],
                frame: [e1, e2, m],
            })
        }
        SurfaceKind::Cylinder(s) => round(
            Round::Cylinder,
            (s.center(), s.frame(), s.axial_sign()),
            [s.radius(), 0.0],
            domain,
        ),
        SurfaceKind::Cone(s) => {
            let (r, k) = s.radius_and_slope();
            round(
                Round::Cone,
                (s.center(), s.frame(), s.axial_sign()),
                [r, k],
                domain,
            )
        }
        SurfaceKind::Sphere(s) => round(
            Round::Sphere,
            (s.center(), s.frame(), s.axial_sign()),
            [s.radius(), 0.0],
            domain,
        ),
        SurfaceKind::Torus(s) => {
            let (major, minor) = s.radii();
            round(
                Round::Torus,
                (s.center(), s.frame(), s.axial_sign()),
                [major, minor],
                domain,
            )
        }
        SurfaceKind::Nurbs(s) => {
            let (bu, bv) = (s.basis_u(), s.basis_v());
            let points: Vec<&[f64]> = s.homogeneous().iter().map(|p| &p[..]).collect();
            let (degrees, knots) = ((bu.degree(), bv.degree()), (bu.knots(), bv.knots()));
            let patch = match s.weights() {
                Some(w) => Patch::new_rational(degrees, knots, &points, w),
                None => Patch::new(degrees, knots, &points),
            };
            patch.ok().map(Surface::Nurbs)
        }
        _ => None,
    }
}

/// Where a point lies about a round kind's axis: its axial coordinate along s·n̂ and its distance
/// from the axis.
fn about(axes: &[P3; 4], x: &P3) -> Option<(IBig, IBig)> {
    let [c, _, _, n] = axes;
    let w = minus(x, c);
    let a = dot(&w, n);
    let r2 = dot(&w, &w) - mul(&a, &a);
    let rho = sqrt(&r2.max(IBig::ZERO))?;
    Some((a, rho))
}

impl Surface {
    /// The point at (u, v); `None` where a NURBS surface cannot be evaluated.
    fn point(&self, u: &IBig, v: &IBig, trig: &mut Trig) -> Option<P3> {
        match self {
            Surface::Plane { abc: [c, a, b], .. } => {
                Some(plus(c, &plus(&times(u, a), &times(v, b))))
            }
            Surface::Round {
                shape,
                axes: [c, x, y, n],
                lengths: [l0, l1],
                ..
            } => {
                let (cu, su) = trig.at(u);
                let e = plus(&times(&cu, x), &times(&su, y));
                let (radial, axial) = match shape {
                    Round::Cylinder => (l0.clone(), v.clone()),
                    Round::Cone => (l0 + mul(l1, v), v.clone()),
                    Round::Sphere => {
                        let (cv, sv) = trig.at(v);
                        (mul(l0, &cv), mul(l0, &sv))
                    }
                    Round::Torus => {
                        let (cv, sv) = trig.at(v);
                        (l0 + mul(l1, &cv), mul(l1, &sv))
                    }
                };
                Some(plus(c, &plus(&times(&radial, &e), &times(&axial, n))))
            }
            Surface::Nurbs(p) => {
                let [du, dv] = p.domain().ok()?;
                let q = p.point(&clamped(u, &du), &clamped(v, &dv)).ok()?;
                let mut it = q.iter().map(fixed::of_rational);
                Some(core::array::from_fn(|_| it.next().unwrap_or_default()))
            }
        }
    }

    /// A lower bound on the distance from x to the whole surface, by its closed form: to the whole
    /// plane, or in the meridian plane through x to both of the surface's meridians there; `None`
    /// for NURBS.
    fn distance(&self, x: &P3) -> Option<IBig> {
        match self {
            Surface::Plane {
                abc: [c, ..],
                frame: [_, _, m],
            } => Some(fixed::abs(&dot(&minus(x, c), m))),
            Surface::Round {
                shape,
                axes,
                lengths: [l0, l1],
                ..
            } => {
                let (a, rho) = about(axes, x)?;
                let gap = |d: IBig| fixed::abs(&d);
                Some(match shape {
                    Round::Cylinder => gap(&rho - l0),
                    Round::Sphere => gap(sqrt(&(mul(&rho, &rho) + mul(&a, &a)))? - l0),
                    Round::Cone => {
                        let across = sqrt(&(one() + mul(l1, l1)))?;
                        let near = div(&gap(&rho - l0 - mul(l1, &a)), &across)?;
                        let far = div(&gap(&rho + l0 + mul(l1, &a)), &across)?;
                        near.min(far)
                    }
                    Round::Torus => {
                        let tube = |rho: IBig| -> Option<IBig> {
                            Some(gap(sqrt(&(mul(&rho, &rho) + mul(&a, &a)))? - l1))
                        };
                        tube(&rho - l0)?.min(tube(&rho + l0)?)
                    }
                })
            }
            Surface::Nurbs(_) => None,
        }
    }

    /// A positive multiple of the gradient of [`Surface::equation`] at x, the surface's normal
    /// direction there; `None` for NURBS.
    fn gradient(&self, x: &P3) -> Option<P3> {
        match self {
            Surface::Plane {
                frame: [_, _, m], ..
            } => Some(m.clone()),
            Surface::Round {
                shape,
                axes: [c, _, _, n],
                lengths: [l0, l1],
                ..
            } => {
                let w = minus(x, c);
                let a = dot(&w, n);
                let radial = minus(&w, &times(&a, n));
                Some(match shape {
                    Round::Cylinder => radial,
                    Round::Sphere => w,
                    Round::Cone => {
                        let r = l0 + mul(l1, &a);
                        minus(&radial, &times(&mul(&r, l1), n))
                    }
                    Round::Torus => {
                        let q = dot(&w, &w) + mul(l0, l0) - mul(l1, l1);
                        minus(&times(&q, &w), &times(&(mul(l0, l0) << 1), &radial))
                    }
                })
            }
            Surface::Nurbs(_) => None,
        }
    }

    /// An implicit equation of the whole surface: continuous, zero exactly on it; `None` for NURBS.
    fn equation(&self, x: &P3) -> Option<IBig> {
        match self {
            Surface::Plane {
                abc: [c, ..],
                frame: [_, _, m],
            } => Some(dot(&minus(x, c), m)),
            Surface::Round {
                shape,
                axes: [c, _, _, n],
                lengths: [l0, l1],
                ..
            } => {
                let w = minus(x, c);
                let (w2, a) = (dot(&w, &w), dot(&w, n));
                let rho2 = &w2 - mul(&a, &a);
                Some(match shape {
                    Round::Cylinder => rho2 - mul(l0, l0),
                    Round::Sphere => w2 - mul(l0, l0),
                    Round::Cone => {
                        let r = l0 + mul(l1, &a);
                        rho2 - mul(&r, &r)
                    }
                    Round::Torus => {
                        let q = &w2 + mul(l0, l0) - mul(l1, l1);
                        mul(&q, &q) - (mul(&mul(l0, l0), &rho2) << 2)
                    }
                })
            }
            Surface::Nurbs(_) => None,
        }
    }
}

/// A chord of an edge: its ends and its sagitta.
#[derive(Clone, Debug)]
struct Chord {
    /// The ends.
    ends: [P3; 2],
    /// How far the edge between them can leave the chord.
    sagitta: IBig,
}

/// The distance from x to the segment a b.
fn to_segment(x: &P3, [a, b]: &[P3; 2]) -> Option<IBig> {
    let d = minus(b, a);
    let dd = dot(&d, &d);
    let w = minus(x, a);
    let t = if dd == IBig::ZERO {
        IBig::ZERO
    } else {
        div(&dot(&w, &d), &dd)?.clamp(IBig::ZERO, one())
    };
    norm(&minus(&w, &times(&t, &d)))
}

/// Whether x is farther than `clear` from every chord, beyond each chord's sagitta.
fn clear_of(x: &P3, chords: &[Chord], clear: &IBig) -> Option<bool> {
    for c in chords {
        if to_segment(x, &c.ends)? <= &c.sagitta + clear {
            return Some(false);
        }
    }
    Some(true)
}

/// Where a face is, for the crossing search (module documentation).
#[derive(Clone, Debug)]
enum Region {
    /// A planar face: its loops' chords in its plane's frame, in loop order.
    Loops(Vec<[[IBig; 2]; 2]>),
    /// A round face that is its whole domain: the axial coordinates inside it, if bounded.
    Whole(Option<(IBig, IBig)>),
}

/// A face read: its surface's definition, its edges' chords and its region.
struct Sheet {
    /// The row.
    face: u32,
    /// The surface.
    surface: Surface,
    /// Every chord of every edge, in loop order.
    rim: Vec<Chord>,
    /// Its region; `None` if it has none here.
    region: Option<Region>,
}

/// A body's rows read once: each curve's definition and chords by edge.
struct Read<'a> {
    /// The rows.
    a: &'a Arenas,
    /// The bound, in steps.
    bound: IBig,
    /// What a find must clear the bound by.
    limit: IBig,
    /// Each edge's curve and span, by edge row.
    curves: Vec<Option<(Curve<3>, (IBig, IBig))>>,
    /// Each edge's chords from start to end, by edge row; `None` without a sagitta.
    chords: Vec<Option<Vec<Chord>>>,
    /// Sines and cosines.
    trig: Trig,
    /// What was found.
    out: Refutation,
}

/// The rows of an arena.
fn rows<K>(n: u32) -> impl Iterator<Item = Ix<K>> {
    (0..n).filter_map(Ix::new)
}

/// A vertex's point; `None` unless it is a grid point.
fn place(a: &Arenas, v: Ix<Vertex>) -> Option<P3> {
    match a.vertices.point.get(v).and_then(|p| a.points.get(*p))? {
        PointKind::Grid(p) => Some([p.x, p.y, p.z].map(|c| IBig::from(c.get()) << F)),
        PointKind::Meet(_) => None,
    }
}

/// The fins of face f, loop by loop, each loop in ring order.
fn fins_of(a: &Arenas, f: Ix<Face>) -> Vec<Vec<Ix<Fin>>> {
    let mut out = Vec::new();
    let mut lp = a.faces.first_loop.get(f).copied().flatten();
    for _ in 0..=a.rows(Arena::Loop) {
        let Some(l) = lp else { break };
        let mut ring = Vec::new();
        if let Some(start) = a.loops.first_fin.get(l).copied().flatten() {
            let mut at = start;
            for _ in 0..=a.rows(Arena::Fin) {
                ring.push(at);
                match a.fins.next.get(at) {
                    Some(&n) if n != start => at = n,
                    _ => break,
                }
            }
        }
        out.push(ring);
        lp = a.loops.next.get(l).copied().flatten();
    }
    out
}

/// The bound a body is recorded with, in steps.
fn bound_of(exactness: Exactness) -> u64 {
    match exactness {
        Exactness::Exact => 0,
        Exactness::Certified { bound, .. } => bound.get(),
        Exactness::Approximate { deviation, .. } => deviation.get(),
    }
}

impl Read<'_> {
    /// Records a find.
    fn find(&mut self, fault: Fault, error: Option<&IBig>) {
        if !self.out.found.iter().any(|f| f.fault == fault) {
            let error = error.map(|e| fixed::rational(e).to_f64().value());
            self.out.found.push(Find { fault, error });
        }
    }

    /// Records what was left unjudged, once.
    fn unjudged(&mut self, what: Unjudged) {
        if !self.out.unjudged.contains(&what) {
            self.out.unjudged.push(what);
        }
    }

    /// Each edge's curve, its vertices at its ends, and its chords.
    fn edges(&mut self) {
        let a = self.a;
        for e in rows::<Edge>(a.rows(Arena::Edge)) {
            let read = a
                .edges
                .curve
                .get(e)
                .and_then(|c| a.curves.get(*c))
                .and_then(curve_of);
            let Some((curve, span)) = read else {
                self.unjudged(Unjudged::Curve(e.row()));
                self.curves.push(None);
                self.chords.push(None);
                continue;
            };
            if matches!(curve, Curve::Trig(..)) {
                self.trig.even(&span, SAMPLES);
            }
            let ts = samples(&span, SAMPLES);
            let points: Option<Vec<P3>> =
                ts.iter().map(|t| curve.point(t, &mut self.trig)).collect();
            let h = (&span.1 - &span.0) / IBig::from(SAMPLES);
            let chords = points.as_ref().and_then(|p| {
                let s = curve.sagitta(&fixed::abs(&h))?;
                Some(
                    p.windows(2)
                        .filter_map(|w| match w {
                            [x, y] => Some(Chord {
                                ends: [x.clone(), y.clone()],
                                sagitta: s.clone(),
                            }),
                            _ => None,
                        })
                        .collect(),
                )
            });
            let ends = [
                a.edges.start.get(e).copied().flatten(),
                a.edges.end.get(e).copied().flatten(),
            ];
            let at = points
                .as_ref()
                .map(|p| [p.first().cloned(), p.last().cloned()]);
            for (vertex, end) in ends.into_iter().zip(at.unwrap_or_default()) {
                let (Some(v), Some(end)) = (vertex, end) else {
                    continue;
                };
                let Some(x) = place(a, v) else { continue };
                if let Some(d) = norm(&minus(&x, &end))
                    && d > self.limit
                {
                    self.find(
                        Fault::VertexOffCurve {
                            vertex: v.row(),
                            edge: e.row(),
                        },
                        Some(&d),
                    );
                }
            }
            if points.is_none() {
                self.unjudged(Unjudged::Curve(e.row()));
            }
            self.curves.push(Some((curve, span)));
            self.chords.push(chords);
        }
    }

    /// Face f's surface, its vertices and edges on it, its fins' pcurves, and its region.
    fn face(&mut self, f: Ix<Face>) -> Option<Sheet> {
        let a = self.a;
        let Some(surface) = a
            .faces
            .surface
            .get(f)
            .and_then(|s| a.surfaces.get(*s))
            .and_then(surface_of)
        else {
            self.unjudged(Unjudged::Surface(f.row()));
            return None;
        };
        let loops = fins_of(a, f);
        let mut rim = Vec::new();
        let mut chorded = true;
        for ring in &loops {
            let mut chain: Vec<Chord> = Vec::new();
            for &n in ring {
                let Some(&e) = a.fins.edge.get(n) else {
                    continue;
                };
                self.on_surface(&surface, e, f);
                self.pcurve(&surface, n, e);
                let along = self.chords.get(e.to_usize()).cloned().flatten();
                match along {
                    Some(mut c) => {
                        if a.fins.sense.get(n) == Some(&Sense::Reversed) {
                            c.reverse();
                            for x in &mut c {
                                x.ends.reverse();
                            }
                        }
                        chain.extend(c);
                    }
                    None => chorded = false,
                }
            }
            // The gaps between one edge's last sample and the next one's first, the loop closed.
            let mut closed = Vec::new();
            for (i, c) in chain.iter().enumerate() {
                closed.push(c.clone());
                let next = chain.get(i + 1).or_else(|| chain.first());
                if let (Some(n), [_, end]) = (next, &c.ends)
                    && let [start, _] = &n.ends
                    && start != end
                {
                    closed.push(Chord {
                        ends: [end.clone(), start.clone()],
                        sagitta: IBig::ZERO,
                    });
                }
            }
            rim.extend(closed);
        }
        let region = if chorded {
            self.region(&surface, &rim)
        } else {
            None
        };
        if region.is_none() {
            self.unjudged(Unjudged::Region(f.row()));
        }
        Some(Sheet {
            face: f.row(),
            surface,
            rim,
            region,
        })
    }

    /// Edge e's vertices and samples on face f's whole surface.
    fn on_surface(&mut self, surface: &Surface, e: Ix<Edge>, f: Ix<Face>) {
        let a = self.a;
        for v in [a.edges.start.get(e), a.edges.end.get(e)]
            .into_iter()
            .filter_map(|v| v.copied().flatten())
        {
            if let Some(x) = place(a, v)
                && let Some(d) = surface.distance(&x)
                && d > self.limit
            {
                self.find(
                    Fault::VertexOffSurface {
                        vertex: v.row(),
                        face: f.row(),
                    },
                    Some(&d),
                );
            }
        }
        let Some(chords) = self.chords.get(e.to_usize()).cloned().flatten() else {
            return;
        };
        let mut worst = IBig::ZERO;
        for c in &chords {
            for x in &c.ends {
                if let Some(d) = surface.distance(x) {
                    worst = worst.max(d);
                }
            }
        }
        if worst > self.limit {
            self.find(
                Fault::EdgeOffSurface {
                    edge: e.row(),
                    face: f.row(),
                },
                Some(&worst),
            );
        }
    }

    /// Fin n's pcurve, mapped by its face's surface, against edge e's curve.
    fn pcurve(&mut self, surface: &Surface, n: Ix<Fin>, e: Ix<Edge>) {
        let a = self.a;
        let Some(kind) = a
            .fins
            .pcurve
            .get(n)
            .copied()
            .flatten()
            .and_then(|p| a.pcurves.get(p))
        else {
            return;
        };
        let Some(Some((curve, span))) = self.curves.get(e.to_usize()).cloned() else {
            return;
        };
        let read = curve_of(kind).filter(|(_, s)| *s == span);
        let Some((p, _)) = read else {
            self.unjudged(Unjudged::Pcurve(n.row()));
            return;
        };
        let mut worst = IBig::ZERO;
        for t in samples(&span, SAMPLES) {
            let mapped = p
                .point(&t, &mut self.trig)
                .and_then(|[u, v]| surface.point(&u, &v, &mut self.trig));
            let (Some(x), Some(y)) = (mapped, curve.point(&t, &mut self.trig)) else {
                self.unjudged(Unjudged::Pcurve(n.row()));
                return;
            };
            if let Some(d) = norm(&minus(&x, &y)) {
                worst = worst.max(d);
            }
        }
        if worst > self.limit {
            self.find(Fault::PcurveOffCurve { fin: n.row() }, Some(&worst));
        }
    }

    /// A face's region from its surface and rim (module documentation); `None` if it has none.
    fn region(&self, surface: &Surface, rim: &[Chord]) -> Option<Region> {
        match surface {
            Surface::Plane {
                abc: [c, ..],
                frame: [e1, e2, _],
            } => Some(Region::Loops(
                rim.iter()
                    .map(|ch| {
                        ch.ends.clone().map(|x| {
                            let w = minus(&x, c);
                            [dot(&w, e1), dot(&w, e2)]
                        })
                    })
                    .collect(),
            )),
            Surface::Round {
                shape,
                axes,
                lengths,
                domain,
            } => self.round_region(*shape, axes, lengths, domain, rim),
            Surface::Nurbs(_) => None,
        }
    }

    /// A round face's region: its whole domain, if every edge lies on the domain's boundary or the
    /// seam of a full turn (module documentation); `None` otherwise.
    fn round_region(
        &self,
        shape: Round,
        axes: &[P3; 4],
        [l0, l1]: &[IBig; 2],
        [(u0, u1), (v0, v1)]: &[(IBig, IBig); 2],
        rim: &[Chord],
    ) -> Option<Region> {
        if u1 - u0 != IBig::from(2) << F {
            return None;
        }
        let mut trig = Trig::default();
        // The seam: the half-plane at u₀, whose across direction is e(u₀ + 1/2).
        let (cu, su) = trig.at(u0);
        let [c, x, y, _] = axes;
        let toward = plus(&times(&cu, x), &times(&su, y));
        let across = minus(&times(&cu, y), &times(&su, x));
        let near = &self.bound + one();
        let on_seam = |ch: &Chord| {
            ch.ends.iter().all(|p| {
                let w = minus(p, c);
                fixed::abs(&dot(&w, &across)) + &ch.sagitta <= near && dot(&w, &toward) > ch.sagitta
            })
        };
        // The axial coordinate a and radius ρ at a parameter v, and its range of v.
        let at_v = |v: &IBig, trig: &mut Trig| -> (IBig, IBig) {
            match shape {
                Round::Cylinder => (v.clone(), l0.clone()),
                Round::Cone => (v.clone(), l0 + mul(l1, v)),
                Round::Sphere => {
                    let (cv, sv) = trig.at(v);
                    (mul(l0, &sv), mul(l0, &cv))
                }
                Round::Torus => {
                    let (cv, sv) = trig.at(v);
                    (mul(l1, &sv), l0 + mul(l1, &cv))
                }
            }
        };
        if shape == Round::Torus {
            // Only the whole torus: v a full turn too, its parallel at v₀ another seam.
            if v1 - v0 != IBig::from(2) << F {
                return None;
            }
            let (a0, r0) = at_v(v0, &mut trig);
            let on_parallel = |ch: &Chord| {
                ch.ends.iter().all(|p| {
                    about(axes, p).is_some_and(|(a, rho)| {
                        fixed::abs(&(a - &a0)) + fixed::abs(&(rho - &r0)) + &ch.sagitta <= near
                    })
                })
            };
            return rim
                .iter()
                .all(|ch| on_seam(ch) || on_parallel(ch))
                .then_some(Region::Whole(None));
        }
        let (lo, hi) = (at_v(v0, &mut trig).0, at_v(v1, &mut trig).0);
        let quarter = (&hi - &lo) >> 2;
        let (mut stray_lo, mut stray_hi) = (IBig::ZERO, IBig::ZERO);
        for ch in rim {
            if on_seam(ch) {
                continue;
            }
            let axial: Option<Vec<IBig>> = ch
                .ends
                .iter()
                .map(|p| about(axes, p).map(|(a, _)| a))
                .collect();
            let axial = axial?;
            let off = |b: &IBig| {
                axial
                    .iter()
                    .map(|a| fixed::abs(&(a - b)))
                    .max()
                    .unwrap_or_default()
                    + &ch.sagitta
            };
            let (from_lo, from_hi) = (off(&lo), off(&hi));
            if from_lo <= quarter {
                stray_lo = stray_lo.max(from_lo);
            } else if from_hi <= quarter {
                stray_hi = stray_hi.max(from_hi);
            } else {
                return None;
            }
        }
        let inside = (lo + stray_lo + one(), hi - stray_hi - one());
        (inside.0 < inside.1).then_some(Region::Whole(Some(inside)))
    }
}

/// Whether x, on the sheet's surface, lies inside its face, well clear of its edges.
fn inside(sheet: &Sheet, x: &P3, clear: &IBig) -> Option<bool> {
    if !clear_of(x, &sheet.rim, clear)? {
        return Some(false);
    }
    match (&sheet.region, &sheet.surface) {
        (
            Some(Region::Loops(chords)),
            Surface::Plane {
                abc: [c, ..],
                frame: [e1, e2, _],
            },
        ) => {
            let w = minus(x, c);
            let q = [dot(&w, e1), dot(&w, e2)];
            Some(winding(chords, &q) != 0)
        }
        (Some(Region::Whole(range)), Surface::Round { axes, .. }) => Some(match range {
            None => true,
            Some((lo, hi)) => {
                let (a, _) = about(axes, x)?;
                *lo < a && a < *hi
            }
        }),
        _ => None,
    }
}

/// The winding number of the closed chains of `chords` about q, by Sunday's crossing rule; q is
/// clear of every chord, so each sign is decided.
fn winding(chords: &[[[IBig; 2]; 2]], q: &[IBig; 2]) -> i64 {
    let [_, qy] = q;
    let mut wn = 0i64;
    for [a, b] in chords {
        let ([_, ay], [_, by]) = (a, b);
        let left = {
            let (d, w) = (minus(b, a), minus(q, a));
            let ([dx, dy], [wx, wy]) = (d, w);
            mul(&dx, &wy) - mul(&dy, &wx)
        };
        if ay <= qy {
            if by > qy && left > IBig::ZERO {
                wn += 1;
            }
        } else if by <= qy && left < IBig::ZERO {
            wn -= 1;
        }
    }
    wn
}

/// A parameter line across a face: the point at t along it.
enum Iso {
    /// A round face's line of fixed u (`true`) or fixed v, at that value.
    Round(bool, IBig),
    /// A planar face's line of fixed first frame coordinate (`true`) or second.
    Plane(bool, IBig),
}

/// The point at t along a line of a sheet.
fn along(sheet: &Sheet, line: &Iso, t: &IBig, trig: &mut Trig) -> Option<P3> {
    match (line, &sheet.surface) {
        (Iso::Round(fixed_u, k), s) => {
            if *fixed_u {
                s.point(k, t, trig)
            } else {
                s.point(t, k, trig)
            }
        }
        (
            Iso::Plane(first, k),
            Surface::Plane {
                abc: [c, ..],
                frame: [e1, e2, _],
            },
        ) => {
            let (p, q) = if *first { (k, t) } else { (t, k) };
            Some(plus(c, &plus(&times(p, e1), &times(q, e2))))
        }
        _ => None,
    }
}

/// A sheet's parameter lines, each with its span.
fn lines(sheet: &Sheet) -> Vec<(Iso, (IBig, IBig))> {
    let (spans, round) = match (&sheet.surface, &sheet.region) {
        (Surface::Round { domain, .. }, Some(Region::Whole(_))) => (domain.clone(), true),
        (Surface::Plane { .. }, Some(Region::Loops(chords))) => {
            let mut lo = [None::<IBig>, None];
            let mut hi = [None::<IBig>, None];
            for p in chords.iter().flatten() {
                for ((l, h), x) in lo.iter_mut().zip(&mut hi).zip(p) {
                    *l = Some(l.take().map_or(x.clone(), |y| y.min(x.clone())));
                    *h = Some(h.take().map_or(x.clone(), |y| y.max(x.clone())));
                }
            }
            let ([Some(l0), Some(l1)], [Some(h0), Some(h1)]) = (lo, hi) else {
                return Vec::new();
            };
            ([(l0, h0), (l1, h1)], false)
        }
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for (fixed_first, across, other) in [(true, 0usize, 1usize), (false, 1, 0)] {
        let (Some(at), Some(span)) = (spans.get(across), spans.get(other)) else {
            continue;
        };
        let width = &at.1 - &at.0;
        for i in 0..ISO {
            let value = &at.0 + &width * IBig::from(2 * i + 1) / IBig::from(2 * ISO);
            let line = if round {
                Iso::Round(fixed_first, value)
            } else {
                Iso::Plane(fixed_first, value)
            };
            out.push((line, span.clone()));
        }
    }
    out
}

/// How far a crossing at x must be from every edge of f and g: the most two surfaces that meet
/// there at the acute angle θ can overlap while each passes within the bound of an edge they share,
/// bound / sin(θ/2) (the half-diagonal of the rhombus of points within the bound of both), plus
/// the margin; `None` where the surfaces are within 2^−20 of parallel there, or a normal vanishes.
fn clearance(f: &Sheet, g: &Sheet, x: &P3, bound: &IBig) -> Option<IBig> {
    let (p, q) = (f.surface.gradient(x)?, g.surface.gradient(x)?);
    let (lp, lq) = (norm(&p)?, norm(&q)?);
    let cos = div(&fixed::abs(&dot(&p, &q)), &mul(&lp, &lq))?;
    let half = sqrt(&((one() - cos.min(one())) >> 1))?;
    if half <= one() >> MARGIN_BITS {
        return None;
    }
    Some(div(bound, &half)? + (one() >> MARGIN_BITS))
}

/// Whether sheet f's lines cross sheet g's surface at a point inside both, clear of their edges by
/// [`clearance`].
fn crosses(f: &Sheet, g: &Sheet, bound: &IBig, trig: &mut Trig) -> Option<bool> {
    let eps = one() >> LEVEL_BITS;
    for (line, span) in lines(f) {
        if matches!(f.surface, Surface::Round { .. }) {
            trig.even(&span, ALONG);
        }
        let ts = samples(&span, ALONG);
        let mut last: Option<(IBig, IBig)> = None;
        for t in ts {
            let Some(x) = along(f, &line, &t, trig) else {
                continue;
            };
            let value = g.surface.equation(&x)?;
            // A sample on the surface, or too near it to sign, is passed over: a change of sign
            // across it is still one, and a touch that keeps its sign is not.
            if fixed::abs(&value) <= eps {
                continue;
            }
            if let Some((s, before)) = &last
                && (before > &IBig::ZERO) != (value > IBig::ZERO)
                && let Some(x) = bisect(
                    f,
                    g,
                    &line,
                    (s.clone(), t.clone()),
                    before > &IBig::ZERO,
                    trig,
                )
                && let Some(clear) = clearance(f, g, &x, bound)
                && inside(f, &x, &clear)?
                && inside(g, &x, &clear)?
            {
                return Some(true);
            }
            last = Some((t, value));
        }
    }
    Some(false)
}

/// The point of a sign change of g's equation along f's line between s and t, halved [`BISECT`]
/// times; `positive` the sign at s.
fn bisect(
    f: &Sheet,
    g: &Sheet,
    line: &Iso,
    (mut s, mut t): (IBig, IBig),
    positive: bool,
    trig: &mut Trig,
) -> Option<P3> {
    let mut x = along(f, line, &s, trig)?;
    for _ in 0..BISECT {
        let m = (&s + &t) >> 1;
        x = along(f, line, &m, trig)?;
        let value = g.surface.equation(&x)?;
        if value == IBig::ZERO {
            break;
        }
        if (value > IBig::ZERO) == positive {
            s = m;
        } else {
            t = m;
        }
    }
    Some(x)
}

/// Whether planar sheet f lies on one side of planar sheet g's plane, touching it at most: a planar
/// face lies within its rim's sagittas of the hull of its chords' ends, so when those ends' signed
/// distances from g's plane, widened by the largest sagitta, keep one sign, no point well inside f
/// has the other, and the pair cannot cross away from its edges in either order.
fn one_side(f: &Sheet, g: &Sheet) -> bool {
    if !matches!(
        (&f.surface, &g.surface, &f.region),
        (
            Surface::Plane { .. },
            Surface::Plane { .. },
            Some(Region::Loops(_))
        )
    ) || f.rim.is_empty()
    {
        return false;
    }
    let eps = one() >> LEVEL_BITS;
    let mut widest = IBig::ZERO;
    let (mut lo, mut hi): (Option<IBig>, Option<IBig>) = (None, None);
    for c in &f.rim {
        widest = widest.max(c.sagitta.clone());
        for x in &c.ends {
            let Some(d) = g.surface.equation(x) else {
                return false;
            };
            lo = Some(lo.map_or(d.clone(), |l| l.min(d.clone())));
            hi = Some(hi.map_or(d.clone(), |h| h.max(d)));
        }
    }
    let (Some(lo), Some(hi)) = (lo, hi) else {
        return false;
    };
    lo - &widest >= -eps.clone() || hi + &widest <= eps
}

/// Whether two faces' stored boxes, each grown by `grow` steps, meet.
fn boxes_meet(a: &Arenas, f: Ix<Face>, g: Ix<Face>, grow: i64) -> bool {
    let (Some(p), Some(q)) = (a.faces.bound.get(f), a.faces.bound.get(g)) else {
        return true;
    };
    let (pl, ph, ql, qh) = (p.min(), p.max(), q.min(), q.max());
    [
        (pl.x, ph.x, ql.x, qh.x),
        (pl.y, ph.y, ql.y, qh.y),
        (pl.z, ph.z, ql.z, qh.z),
    ]
    .into_iter()
    .all(|(a0, a1, b0, b1)| {
        a0.get().saturating_sub(grow) <= b1.get().saturating_add(grow)
            && b0.get().saturating_sub(grow) <= a1.get().saturating_add(grow)
    })
}

/// Looks for the faults of chapter 28.4 in `body`, recorded at `exactness` (module
/// documentation). It can only ever find a fault; an empty list proves nothing.
#[must_use]
pub fn refute(body: &Body, exactness: Exactness) -> Refutation {
    let a = body.arenas();
    let steps = bound_of(exactness);
    let bound = IBig::from(steps) << F;
    let mut r = Read {
        a,
        limit: &bound + (one() >> MARGIN_BITS),
        bound,
        curves: Vec::new(),
        chords: Vec::new(),
        trig: Trig::default(),
        out: Refutation::default(),
    };
    r.edges();
    let faces: Vec<Ix<Face>> = rows(a.rows(Arena::Face)).collect();
    let mut sheets = Vec::new();
    for &f in &faces {
        if let Some(s) = r.face(f) {
            sheets.push((f, s));
        }
    }
    let grow = i64::try_from(steps.saturating_add(1)).unwrap_or(i64::MAX);
    for (i, (f, s)) in sheets.iter().enumerate() {
        for (g, t) in sheets.iter().skip(i + 1) {
            if s.region.is_none()
                || t.region.is_none()
                || !boxes_meet(a, *f, *g, grow)
                || one_side(s, t)
                || one_side(t, s)
            {
                continue;
            }
            let found = crosses(s, t, &r.bound, &mut r.trig) == Some(true)
                || crosses(t, s, &r.bound, &mut r.trig) == Some(true);
            if found {
                r.find(
                    Fault::FacesCross {
                        first: s.face.min(t.face),
                        second: s.face.max(t.face),
                    },
                    None,
                );
            }
        }
    }
    r.out
}
