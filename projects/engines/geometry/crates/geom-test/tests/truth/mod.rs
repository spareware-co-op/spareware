//! The kinds the law catalogue draws, each with its truth: the kind's definition as its
//! documentation states it, evaluated in `geom_test::fixed`'s 400-bit fixed point, and NURBS
//! exactly by `geom_test::nurbs_reference`. A truth reads only the doubles a kind is built from,
//! never the kernel's code.

#![allow(
    dead_code,
    reason = "each law binary uses part of the catalogue's kinds"
)]

use core::num::NonZeroU64;

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_curve::map::{LocalMap, ParamMap};
use geom_curve::offset::{Offset as CurveOffset, OffsetLaw};
use geom_curve::on_surface::{OnSurface, SurfaceRef};
use geom_curve::trimmed::Trimmed;
use geom_curve::{
    Axes, Basis, BulgePolyline, Circle, Composite, CurveKind, Ellipse, Frame, Helix, HelixAxes,
    Hyperbola, Involute, Line, Parabola, Polynomial, Transition,
};
use geom_curve::{IntersectionCurve, Span};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_nurbs::Basis as Knots;
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, Extrusion, Offset as SurfaceOffset, ParamSurface, Plane, Revolution, Ruled,
    Sphere, SurfaceKind, Torus,
};
use geom_test::SplitMix64;
use geom_test::fixed::{self, F, cos_sin_pi, div, exp, mul, one, sqrt};
use geom_test::nurbs_reference::{Patch, Spline};

use crate::{laws, series};

pub fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A double in the fixed point.
pub fn fx(x: f64) -> IBig {
    fixed::of(x).expect("a finite double")
}

fn zero<const D: usize>() -> [IBig; D] {
    core::array::from_fn(|_| IBig::ZERO)
}

fn fvec<const D: usize>(v: [f64; D]) -> [IBig; D] {
    v.map(fx)
}

fn dot(a: &[IBig], b: &[IBig]) -> IBig {
    a.iter().zip(b).map(|(x, y)| mul(x, y)).sum()
}

fn cross(a: &[IBig; 3], b: &[IBig; 3]) -> [IBig; 3] {
    [
        mul(&a[1], &b[2]) - mul(&a[2], &b[1]),
        mul(&a[2], &b[0]) - mul(&a[0], &b[2]),
        mul(&a[0], &b[1]) - mul(&a[1], &b[0]),
    ]
}

fn unit3(a: &[IBig; 3]) -> [IBig; 3] {
    let l = sqrt(&dot(a, a)).expect("a square");
    a.clone().map(|x| div(&x, &l).expect("a nonzero vector"))
}

fn lift<const D: usize>(v: &[IBig; D]) -> [IBig; 3] {
    core::array::from_fn(|i| v.get(i).cloned().unwrap_or(IBig::ZERO))
}

fn drop_to<const D: usize>(v: &[IBig; 3]) -> [IBig; D] {
    core::array::from_fn(|i| v[i].clone())
}

/// A frame's exact unit vectors x̂, ŷ, n̂ (`geom_curve::frame`'s definition): x̂ along
/// (n × x) × n, ŷ along n × x, n̂ along n.
pub fn frame_axes<const D: usize>(x: [f64; D], n: [f64; 3]) -> [[IBig; 3]; 3] {
    let (x, n) = (lift(&fvec(x)), fvec(n));
    let nx = cross(&n, &x);
    [unit3(&cross(&nx, &n)), unit3(&nx), unit3(&n)]
}

/// The scalar functions of each analytic curve kind, t in half-turns for the angles.
#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// (t, 0, 0).
    Linear,
    /// (cos πt, sin πt, 0).
    Trig,
    /// (cosh t, sinh t, 0).
    Hyperbolic,
    /// (t², t, 0).
    Parabolic,
    /// (cos πt, sin πt, t).
    Helical,
    /// (cos πt + πt·sin πt, sin πt − πt·cos πt, 0).
    Involute,
}

/// One segment of a bulge polyline: C(v) = (B₀p₀ + B₁h₁ + B₂p₂)/(B₀ + B₁w + B₂).
#[derive(Clone, Debug)]
pub struct Segment<const D: usize> {
    p0: [IBig; D],
    h1: [IBig; D],
    w: IBig,
    p2: [IBig; D],
}

/// A transform's exact entries: rows (m₀, m₁, m₂, t).
pub type Entries = [[IBig; 4]; 3];

/// A curve's definition.
#[derive(Clone, Debug)]
pub enum Truth<const D: usize> {
    /// c + f₁(t)·u + f₂(t)·v + f₃(t)·w, as [c, u, v, w].
    Form(Shape, [[IBig; D]; 4]),
    /// Σ aⱼ·tʲ.
    Poly(Vec<[IBig; D]>),
    /// A NURBS curve, exactly.
    Nurbs(Spline),
    /// A bulge polyline: its start, and its segments.
    Bulge(IBig, Vec<Segment<D>>),
    /// A composite: its starts, and each segment's truth and interval.
    Composite(Vec<IBig>, Vec<(Truth<D>, IBig, IBig)>),
    /// An offset: the basis, the distance, and the reference direction (3D; the left normal in
    /// 2D).
    Offset(Box<Truth<D>>, IBig, Option<[IBig; 3]>),
    /// A transition: piece G63's truth, in its 256-bit fixed point.
    Transition(Box<series::Truth>),
    /// The image of a curve under a transform.
    Image(Entries, Box<Truth<D>>),
}

fn shape_values(shape: Shape, t: &IBig) -> [IBig; 3] {
    match shape {
        Shape::Linear => [t.clone(), IBig::ZERO, IBig::ZERO],
        Shape::Parabolic => [mul(t, t), t.clone(), IBig::ZERO],
        Shape::Trig => {
            let (c, s) = cos_sin_pi(t);
            [c, s, IBig::ZERO]
        }
        Shape::Helical => {
            let (c, s) = cos_sin_pi(t);
            [c, s, t.clone()]
        }
        Shape::Hyperbolic => {
            let (e, ei) = (exp(t), exp(&-t.clone()));
            [(&e + &ei) >> 1, (&e - &ei) >> 1, IBig::ZERO]
        }
        Shape::Involute => {
            let (c, s) = cos_sin_pi(t);
            let th = mul(t, &fixed::pi());
            [&c + mul(&th, &s), &s - mul(&th, &c), IBig::ZERO]
        }
    }
}

/// The fixed-point parameter clamped into a NURBS domain, exactly.
fn clamp(x: &IBig, (lo, hi): &(RBig, RBig)) -> RBig {
    let q = fixed::rational(x);
    if q < *lo {
        lo.clone()
    } else if q > *hi {
        hi.clone()
    } else {
        q
    }
}

/// A NURBS curve's exact point at u (clamped into its domain), in the fixed point.
pub fn spline_at<const D: usize>(s: &Spline, u: &IBig) -> [IBig; D] {
    let d = s.domain().expect("a domain");
    let p = s.point(&clamp(u, &d)).expect("inside");
    core::array::from_fn(|i| fixed::of_rational(&p[i]))
}

/// A NURBS patch's exact point at (u, v), clamped into its domain.
pub fn patch_at(s: &Patch, u: &IBig, v: &IBig) -> [IBig; 3] {
    let [du, dv] = s.domain().expect("a domain");
    let p = s.point(&clamp(u, &du), &clamp(v, &dv)).expect("inside");
    core::array::from_fn(|i| fixed::of_rational(&p[i]))
}

/// The reference spline of a kernel curve: its stored homogeneous points and weights, exactly.
pub fn spline_of<const D: usize>(c: &geom_nurbs::Curve<D>) -> Spline {
    let b = c.basis();
    let pts: Vec<&[f64]> = c.homogeneous().iter().map(|p| &p[..]).collect();
    match c.weights() {
        Some(w) => Spline::new_rational(b.degree(), b.knots(), &pts, w).unwrap(),
        None => Spline::new(b.degree(), b.knots(), &pts).unwrap(),
    }
}

/// The reference patch of a kernel surface.
pub fn patch_of(s: &geom_nurbs::Surface<3>) -> Patch {
    let (bu, bv) = (s.basis_u(), s.basis_v());
    let pts: Vec<&[f64]> = s.homogeneous().iter().map(|p| &p[..]).collect();
    let (deg, kn) = ((bu.degree(), bv.degree()), (bu.knots(), bv.knots()));
    match s.weights() {
        Some(w) => Patch::new_rational(deg, kn, &pts, w).unwrap(),
        None => Patch::new(deg, kn, &pts).unwrap(),
    }
}

/// The index of the piece holding t among pieces starting at `starts` (the last one at the
/// end): the last start at or before t.
fn piece(starts: &[IBig], t: &IBig, pieces: usize) -> usize {
    let k = starts
        .iter()
        .take(pieces)
        .rposition(|s| s <= t)
        .unwrap_or(0);
    k.min(pieces - 1)
}

impl<const D: usize> Truth<D> {
    /// The point at t.
    pub fn point(&self, t: &IBig) -> [IBig; D] {
        match self {
            Truth::Form(shape, [c, u, v, w]) => {
                let f = shape_values(*shape, t);
                core::array::from_fn(|i| {
                    &c[i] + mul(&f[0], &u[i]) + mul(&f[1], &v[i]) + mul(&f[2], &w[i])
                })
            }
            Truth::Poly(a) => {
                let mut s = zero::<D>();
                for aj in a.iter().rev() {
                    for i in 0..D {
                        s[i] = mul(&s[i], t) + &aj[i];
                    }
                }
                s
            }
            Truth::Nurbs(s) => spline_at(s, t),
            Truth::Bulge(start, segments) => {
                let x = t - start;
                let k = usize::try_from((&x >> F).max(IBig::ZERO))
                    .unwrap_or(usize::MAX)
                    .min(segments.len() - 1);
                let v = x - (IBig::from(k) << F);
                let s = &segments[k];
                let vm = one() - &v;
                let (b0, b1, b2) = (mul(&vm, &vm), mul(&vm, &v) << 1, mul(&v, &v));
                let den = &b0 + mul(&b1, &s.w) + &b2;
                core::array::from_fn(|i| {
                    let num = mul(&b0, &s.p0[i]) + mul(&b1, &s.h1[i]) + mul(&b2, &s.p2[i]);
                    div(&num, &den).expect("a positive denominator")
                })
            }
            Truth::Composite(starts, segments) => {
                let k = piece(starts, t, segments.len());
                let (truth, a, b) = &segments[k];
                let (s0, s1) = (&starts[k], &starts[k + 1]);
                let local = a + div(&mul(&(t - s0), &(b - a)), &(s1 - s0)).expect("a segment");
                truth.point(&local)
            }
            Truth::Offset(basis, d, reference) => {
                let p = basis.point(t);
                let dp = basis.derivative(t);
                let n: [IBig; 3] = match reference {
                    None => [-dp[1].clone(), dp[0].clone(), IBig::ZERO],
                    Some(r) => cross(&lift(&dp), r),
                };
                let len = sqrt(&dot(&n, &n)).expect("a square");
                core::array::from_fn(|i| {
                    &p[i] + div(&mul(d, &n[i]), &len).expect("a regular basis")
                })
            }
            Truth::Transition(tr) => {
                let s = t >> (F - series::F);
                let q = tr.placed(tr.local(&s));
                core::array::from_fn(|i| q.get(i).map_or(IBig::ZERO, |x| x << (F - series::F)))
            }
            Truth::Image(e, basis) => image(e, &basis.point(t)),
        }
    }

    /// The first derivative at t by a central difference with step 2^−60: within about 2^−120
    /// times the third derivative.
    pub fn derivative(&self, t: &IBig) -> [IBig; D] {
        let h = one() >> 60;
        let (a, b) = (self.point(&(t + &h)), self.point(&(t - &h)));
        core::array::from_fn(|i| (&a[i] - &b[i]) << 59)
    }

    /// Whether the truth leans on a difference or on G63's 256-bit integral, which are good to
    /// about 2^−100 relative, not 2^−300.
    pub fn coarse(&self) -> bool {
        match self {
            Truth::Offset(..) | Truth::Transition(_) => true,
            Truth::Composite(_, s) => s.iter().any(|(t, _, _)| t.coarse()),
            Truth::Image(_, b) => b.coarse(),
            _ => false,
        }
    }
}

/// The margin a law allows the truth itself at a point of size `size`.
pub fn slack(coarse: bool, size: &[IBig]) -> IBig {
    if coarse {
        let s: IBig = size.iter().map(fixed::abs).sum();
        (one() + s) >> 90
    } else {
        one() >> 200
    }
}

/// A transform's exact entries in the fixed point: rows (m₀, m₁, m₂, t).
pub fn entries(t: &TypedTransform) -> Entries {
    let rational = |r: &RationalTransform| -> Entries {
        let d = IBig::from(r.denominator());
        let (m, tr) = (r.matrix(), r.translation_part());
        core::array::from_fn(|i| {
            core::array::from_fn(|j| {
                let n = if j < 3 {
                    IBig::from(m[i][j])
                } else {
                    IBig::from(tr[i])
                };
                (n << F) / &d
            })
        })
    };
    match t {
        TypedTransform::Exact(g) => rational(&RationalTransform::from_grid(g)),
        TypedTransform::Rational(r) => rational(r),
        TypedTransform::Rotation(r) => {
            let (m, d) = (r.matrix().unwrap(), IBig::from(r.norm2()));
            core::array::from_fn(|i| {
                core::array::from_fn(|j| {
                    if j < 3 {
                        (IBig::from(m[i][j]) << F) / &d
                    } else {
                        IBig::ZERO
                    }
                })
            })
        }
        TypedTransform::Certified(c) => c.rows().map(|row| row.map(fx)),
    }
}

/// The image of a point (z = 0 for a 2D point).
pub fn image<const D: usize>(e: &Entries, p: &[IBig; D]) -> [IBig; D] {
    core::array::from_fn(|i| {
        let mut s = e[i][3].clone();
        for j in 0..D {
            s += mul(&e[i][j], &p[j]);
        }
        s
    })
}

/// A conic map's exact u at t: the tangent half-angle parameter over its breaks
/// (`geom_curve::map`'s definition).
fn conic(breaks: &[f64], circular: bool, t: &IBig) -> IBig {
    let mut k = 0;
    while k + 2 < breaks.len() && fx(breaks[k + 1]) < *t {
        k += 1;
    }
    let (t0, t1) = (fx(breaks[k]), fx(breaks[k + 1]));
    let tau = |x: &IBig| {
        if circular {
            fixed::tan_pi(x).expect("inside a piece")
        } else {
            fixed::tanh(x)
        }
    };
    let sigma = tau(&((&t1 - &t0) >> 1));
    let kf = IBig::from(k) << F;
    if k % 2 == 0 {
        kf + div(&tau(&((t - &t0) >> 1)), &sigma).expect("a piece")
    } else {
        kf + one() - div(&tau(&((&t1 - t) >> 1)), &sigma).expect("a piece")
    }
}

/// A parameter map's exact u at t.
pub fn map_at(map: &ParamMap, t: &IBig) -> IBig {
    match map {
        ParamMap::Same => t.clone(),
        ParamMap::Circular(b) => conic(b, true, t),
        ParamMap::Hyperbolic(b) => conic(b, false, t),
        ParamMap::Pieces(pieces) => {
            let p = pieces
                .iter()
                .find(|p| fx(p.start) <= *t && *t <= fx(p.end))
                .expect("a piece holds t");
            let (a, b) = (fx(p.local.0), fx(p.local.1));
            let (s0, s1) = (fx(p.start), fx(p.end));
            let l = &a + div(&mul(&(t - &s0), &(&b - &a)), &(&s1 - &s0)).expect("a piece");
            let u = match &p.map {
                LocalMap::Same => l,
                LocalMap::Circular(br) => conic(br, true, &l),
                LocalMap::Hyperbolic(br) => conic(br, false, &l),
            };
            u + fx(p.shift)
        }
    }
}

/// The parameters a map changes pieces at.
pub fn map_breaks(map: &ParamMap) -> Vec<f64> {
    match map {
        ParamMap::Same => Vec::new(),
        ParamMap::Circular(b) | ParamMap::Hyperbolic(b) => b.clone(),
        ParamMap::Pieces(p) => p.iter().flat_map(|p| [p.start, p.end]).collect(),
    }
}

/// The seeded draws.
pub struct Rng(pub SplitMix64);

impl Rng {
    pub fn unit(&mut self) -> f64 {
        (self.0.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.0.below(NonZeroU64::new(n).unwrap())
    }
    /// A value in [−s, s]: any double, or on the grid of 2^−12 (so whole-step translations add
    /// to it exactly).
    pub fn value(&mut self, s: f64, grid: bool) -> f64 {
        let x = self.range(-s, s);
        if grid {
            ((x * 4096.0) as i64) as f64 / 4096.0
        } else {
            x
        }
    }
    pub fn vector<const D: usize>(&mut self, s: f64, grid: bool) -> [f64; D] {
        core::array::from_fn(|_| self.value(s, grid))
    }
    /// A length in [lo, hi], on the grid if asked.
    pub fn length(&mut self, lo: f64, hi: f64, grid: bool) -> f64 {
        let x = self.range(lo, hi);
        if grid {
            (((x * 4096.0) as i64) as f64 / 4096.0).max(lo)
        } else {
            x
        }
    }
    /// A weight 2^e·(1 + f), e a whole number from −4 to 4, exactly a double.
    pub fn weight(&mut self) -> f64 {
        let e = self.below(9) as i32 - 4;
        let f = ((self.unit() * 256.0) as i64) as f64 / 256.0;
        (1.0 + f)
            * if e < 0 {
                1.0 / f64::from(1u32 << -e)
            } else {
                f64::from(1u32 << e)
            }
    }
    pub fn frame<const D: usize>(&mut self, grid: bool) -> (Frame<D>, [f64; D], [f64; 3]) {
        loop {
            let x: [f64; D] = self.vector(1.0, grid);
            let n = if D == 2 {
                [0.0, 0.0, if self.below(2) == 0 { 1.0 } else { -2.0 }]
            } else {
                self.vector(1.0, grid)
            };
            if let Ok(f) = Frame::new(x, n) {
                return (f, x, n);
            }
        }
    }
}

/// A clamped knot vector of degree p on [0, 1] with up to two interior knots on the grid of 2^−12.
pub fn knots(rng: &mut Rng, p: usize) -> Knots {
    let mut k = vec![0.0; p + 1];
    let interior = rng.below(3);
    for i in 1..=interior {
        let x = i as f64 / (interior + 1) as f64 + rng.range(-0.025, 0.025);
        k.push(((x * 4096.0) as i64) as f64 / 4096.0);
    }
    k.extend(std::iter::repeat_n(1.0, p + 1));
    Knots::new(p, k).unwrap()
}

/// A NURBS curve of degree 1 to 4 on [0, 1], rational for two in three.
pub fn nurbs_curve<const D: usize>(rng: &mut Rng, grid: bool) -> geom_nurbs::Curve<D> {
    let p = 1 + rng.below(4) as usize;
    let b = knots(rng, p);
    let pts: Vec<[f64; D]> = (0..b.count()).map(|_| rng.vector(10.0, grid)).collect();
    if rng.below(3) == 0 {
        geom_nurbs::Curve::new(b, &pts).unwrap()
    } else {
        let w: Vec<f64> = (0..pts.len()).map(|_| rng.weight()).collect();
        geom_nurbs::Curve::new_rational(b, &pts, &w).unwrap()
    }
}

/// A NURBS surface of degree 1 to 3 each way, rational for two in three; when `gentle`, near a
/// graph over the plane with weights within a factor of 2 of each other, so its normal stays well
/// away from zero (an offset's base: where the weights spread, the offset may honestly refuse to
/// convert, its normal not separated from zero over the boxes it certifies on).
pub fn nurbs_surface(rng: &mut Rng, grid: bool, gentle: bool) -> geom_nurbs::Surface<3> {
    let (p, q) = (1 + rng.below(3) as usize, 1 + rng.below(3) as usize);
    let (bu, bv) = (knots(rng, p), knots(rng, q));
    let (n, m) = (bu.count(), bv.count());
    let mut pts = Vec::with_capacity(n * m);
    for i in 0..n {
        for j in 0..m {
            // On the grid of 2^−12, so whole-step translations move it exactly.
            let on = |x: f64| ((x * 4096.0) as i64) as f64 / 4096.0;
            pts.push(if gentle {
                [
                    on(4.0 * i as f64 / (n - 1) as f64),
                    on(4.0 * j as f64 / (m - 1) as f64),
                    rng.value(0.5, grid),
                ]
            } else {
                rng.vector(10.0, grid)
            });
        }
    }
    if rng.below(3) == 0 {
        geom_nurbs::Surface::new(bu, bv, &pts).unwrap()
    } else {
        let w: Vec<f64> = (0..pts.len())
            .map(|_| {
                if gentle {
                    1.0 + ((rng.unit() * 256.0) as i64) as f64 / 256.0
                } else {
                    rng.weight()
                }
            })
            .collect();
        geom_nurbs::Surface::new_rational(bu, bv, &pts, &w).unwrap()
    }
}

/// A bulge polyline's segments from its vertices and bulges (`geom_curve::bulge`'s definition):
/// w = (1 − b²)/(1 + b²) and h₁ = ((1 − b²)·m + b·(d.y, −d.x))/(1 + b²), m the chord's middle,
/// d = p₂ − p₀.
fn bulge_truth<const D: usize>(vertices: &[[f64; D]], bulges: &[f64], closed: bool) -> Truth<D> {
    let n = vertices.len();
    let segments = bulges
        .iter()
        .enumerate()
        .map(|(i, &b)| {
            let (p0, p2) = (fvec(vertices[i]), fvec(vertices[(i + 1) % n]));
            let b = fx(b);
            let bb = mul(&b, &b);
            let (num, den) = (one() - &bb, one() + &bb);
            let d: [IBig; D] = core::array::from_fn(|k| &p2[k] - &p0[k]);
            let perp = [d[1].clone(), -d[0].clone()];
            let h1 = core::array::from_fn(|k| {
                let middle = (&p0[k] + &p2[k]) >> 1;
                div(&(mul(&num, &middle) + mul(&b, &perp[k])), &den).unwrap()
            });
            Segment {
                p0,
                h1,
                w: div(&num, &den).unwrap(),
                p2,
            }
        })
        .collect();
    let _ = closed;
    Truth::Bulge(IBig::ZERO, segments)
}

/// The analytic curve kinds and the leaf kinds built on them, each with its truth.
pub fn curves<const D: usize>(rng: &mut Rng, grid: bool) -> Vec<(CurveKind<D>, Truth<D>)> {
    let c: [f64; D] = rng.vector(40.0, grid);
    let (u, v, w): ([f64; D], [f64; D], [f64; D]) = (
        rng.vector(9.0, grid),
        rng.vector(9.0, grid),
        rng.vector(2.0, grid),
    );
    let r = rng.length(0.25, 12.0, grid);
    let (frame, fx_, fn_) = rng.frame::<D>(grid);
    let [xh, yh, _] = frame_axes(fx_, fn_);
    let scaled =
        |s: f64, a: &[IBig; 3]| -> [IBig; D] { drop_to(&a.clone().map(|x| mul(&fx(s), &x))) };
    let form =
        |s: Shape, u: [IBig; D], v: [IBig; D], w: [IBig; D]| Truth::Form(s, [fvec(c), u, v, w]);
    let circle = Circle::new(c, r, frame, (-0.75, 1.0)).unwrap();
    let circle_truth = form(Shape::Trig, scaled(r, &xh), scaled(r, &yh), zero());
    let ellipse = Ellipse::new(c, Axes::Vectors { u, v }, (-1.0, 1.0)).unwrap();
    let ellipse_truth = form(Shape::Trig, fvec(u), fvec(v), zero());
    let coeffs: Vec<[f64; D]> = (0..5).map(|_| rng.vector(3.0, grid)).collect();
    let nurbs = nurbs_curve::<D>(rng, grid);
    let nurbs_truth = Truth::Nurbs(spline_of(&nurbs));
    let mut out = vec![
        (
            CurveKind::Line(Line::new(c, u, (-2.5, 3.0)).unwrap()),
            form(Shape::Linear, fvec(u), zero(), zero()),
        ),
        (CurveKind::Circle(circle.clone()), circle_truth.clone()),
        (CurveKind::Ellipse(ellipse.clone()), ellipse_truth.clone()),
        (
            CurveKind::Hyperbola(Hyperbola::new(c, Axes::Vectors { u, v }, (-2.0, 2.5)).unwrap()),
            form(Shape::Hyperbolic, fvec(u), fvec(v), zero()),
        ),
        (
            CurveKind::Parabola(Parabola::new(c, Axes::Vectors { u, v }, (-3.0, 2.0)).unwrap()),
            form(Shape::Parabolic, fvec(u), fvec(v), zero()),
        ),
        (
            CurveKind::Involute(
                Involute::with_axes(c, Axes::Vectors { u, v }, (-1.5, 2.5)).unwrap(),
            ),
            form(Shape::Involute, fvec(u), fvec(v), zero()),
        ),
        (
            CurveKind::Polynomial(Polynomial::new(coeffs.clone(), (-1.5, 2.0)).unwrap()),
            Truth::Poly(coeffs.into_iter().map(fvec).collect()),
        ),
        (CurveKind::Nurbs(nurbs.clone()), nurbs_truth.clone()),
        (
            CurveKind::Trimmed(Trimmed::new(Basis::Circle(circle.clone()), (-0.5, 0.625)).unwrap()),
            circle_truth.clone(),
        ),
        (
            CurveKind::Trimmed(Trimmed::new(Basis::Nurbs(nurbs.clone()), (0.125, 0.75)).unwrap()),
            nurbs_truth.clone(),
        ),
    ];
    // Composites of pieces of one basis, which meet exactly.
    for (basis, truth, (a, m, b)) in [
        (
            Basis::Circle(circle.clone()),
            circle_truth.clone(),
            (-0.75, 0.25, 1.0),
        ),
        (
            Basis::Nurbs(nurbs.clone()),
            nurbs_truth.clone(),
            (0.0, 0.375, 1.0),
        ),
    ] {
        let segments = vec![
            Trimmed::new(basis.clone(), (a, m)).unwrap(),
            Trimmed::new(basis, (m, b)).unwrap(),
        ];
        let k = Composite::new(segments).unwrap();
        let starts = k.starts().iter().map(|&s| fx(s)).collect();
        let truth = Truth::Composite(
            starts,
            vec![(truth.clone(), fx(a), fx(m)), (truth, fx(m), fx(b))],
        );
        out.push((CurveKind::Composite(k), truth));
    }
    let d = rng.length(0.25, 1.5, grid);
    if D == 2 {
        let basis = Trimmed::new(Basis::Ellipse(ellipse), (-0.5, 0.5)).unwrap();
        let k = CurveOffset::new(basis, OffsetLaw::Planar { distance: d }).unwrap();
        out.push((
            CurveKind::Offset(k),
            Truth::Offset(Box::new(ellipse_truth), fx(d), None),
        ));
        let n = 3 + rng.below(2) as usize;
        let vertices: Vec<[f64; D]> = (0..n).map(|_| rng.vector(10.0, grid)).collect();
        let closed = rng.below(2) == 0;
        let segments = if closed { n } else { n - 1 };
        let bulges: Vec<f64> = (0..segments)
            .map(|i| if i == 0 { 0.0 } else { rng.value(1.5, true) })
            .collect();
        let truth = bulge_truth(&vertices, &bulges, closed);
        let k = BulgePolyline::new(vertices, bulges, None, closed).unwrap();
        out.push((CurveKind::Bulge(k), truth));
        let cases = laws::cases();
        let case = &cases[rng.below(cases.len() as u64) as usize];
        let start = core::array::from_fn(|i| case.start[i]);
        let direction = core::array::from_fn(|i| case.direction[i]);
        let k = Transition::new(start, direction, case.length, case.law, &mut meter()).unwrap();
        let truth = laws::truth(&case.law, case.length, case.start, case.unit);
        out.push((CurveKind::Transition(k), Truth::Transition(Box::new(truth))));
    }
    if D == 3 {
        let helix = Helix::new(c, HelixAxes::Vectors { u, v, w }, (-3.0, 5.0)).unwrap();
        let helix_truth = form(Shape::Helical, fvec(u), fvec(v), fvec(w));
        out.push((CurveKind::Helix(helix.clone()), helix_truth.clone()));
        // Along the helix's pivot, so C′ × V never vanishes: a regular offset.
        let direction: [f64; 3] = core::array::from_fn(|i| w[i]);
        let basis = Trimmed::new(Basis::Helix(helix), (-1.0, 1.0)).unwrap();
        let law = OffsetLaw::Reference {
            distance: d,
            direction,
        };
        if let Ok(k) = CurveOffset::new(basis, law) {
            out.push((
                CurveKind::Offset(k),
                Truth::Offset(Box::new(helix_truth), fx(d), Some(fvec(direction))),
            ));
        }
        let pcurve = Trimmed::new(
            Basis::Line(Line::new([0.25, 0.5], [0.5, 0.25], (0.0, 1.0)).unwrap()),
            (0.0, 1.0),
        )
        .unwrap();
        let cache = nurbs_curve::<D>(rng, grid);
        let k = OnSurface::new(pcurve, SurfaceRef(7), cache.clone(), 1e-9).unwrap();
        out.push((CurveKind::OnSurface(k), Truth::Nurbs(spline_of(&cache))));
    }
    out
}

/// The analytic surface kinds' shapes (`geom_surface::analytic`'s table), with e(u) =
/// cos πu·x̂ + sin πu·ŷ.
#[derive(Clone, Copy, Debug)]
pub enum Round {
    /// c + r·e(u) + v·n̂.
    Cylinder,
    /// c + (r₀ + k·v)·e(u) + v·n̂.
    Cone,
    /// c + r·cos πv·e(u) + r·sin πv·n̂.
    Sphere,
    /// c + (R + r·cos πv)·e(u) + r·sin πv·n̂.
    Torus,
}

/// A surface's definition.
#[derive(Clone, Debug)]
pub enum Surface {
    /// c + u·a + v·b.
    Plane([[IBig; 3]; 3]),
    /// An analytic kind: its centre, frame (x̂, ŷ, n̂) and two lengths.
    Round(Round, [IBig; 3], [[IBig; 3]; 3], [IBig; 2]),
    /// A NURBS surface, exactly.
    Patch(Patch),
    /// A revolution of a profile about the pivot through a along n̂ (`geom_surface::swept`):
    /// a + A(v) + cos πu·B(v) + sin πu·C(v), w = P(v) − a, A = (w·n̂)·n̂, B = w − A, C = n̂ × w.
    Revolution(Truth<3>, [IBig; 3], [IBig; 3]),
    /// P(u) + v·d.
    Extrusion(Truth<3>, [IBig; 3]),
    /// P₁(u) + v·(P₂(u) − P₁(u)).
    Ruled(Truth<3>, Truth<3>),
    /// `S + d·(S_u × S_v)/|S_u × S_v|`.
    Offset(Box<Surface>, IBig),
    /// The image of a surface under a transform.
    Image(Entries, Box<Surface>),
    /// A surface within a stated distance of another: a kind built by a transform, which states
    /// how far its own definition may be from the exact image (an affine image of a torus, whose
    /// stored terms round its irrational frame).
    Within(IBig, Box<Surface>),
}

impl Surface {
    /// The point at (u, v).
    pub fn point(&self, u: &IBig, v: &IBig) -> [IBig; 3] {
        match self {
            Surface::Plane([c, a, b]) => {
                core::array::from_fn(|i| &c[i] + mul(u, &a[i]) + mul(v, &b[i]))
            }
            Surface::Round(which, c, [x, y, n], [l0, l1]) => {
                let (cu, su) = cos_sin_pi(u);
                let e: [IBig; 3] = core::array::from_fn(|i| mul(&cu, &x[i]) + mul(&su, &y[i]));
                let (radial, axial) = match which {
                    Round::Cylinder => (l0.clone(), v.clone()),
                    Round::Cone => (l0 + mul(l1, v), v.clone()),
                    Round::Sphere => {
                        let (cv, sv) = cos_sin_pi(v);
                        (mul(l0, &cv), mul(l0, &sv))
                    }
                    Round::Torus => {
                        let (cv, sv) = cos_sin_pi(v);
                        (l0 + mul(l1, &cv), mul(l1, &sv))
                    }
                };
                core::array::from_fn(|i| &c[i] + mul(&radial, &e[i]) + mul(&axial, &n[i]))
            }
            Surface::Patch(p) => patch_at(p, u, v),
            Surface::Revolution(profile, a, n) => {
                let p = profile.point(v);
                let w: [IBig; 3] = core::array::from_fn(|i| &p[i] - &a[i]);
                let wn = dot(&w, n);
                let along: [IBig; 3] = core::array::from_fn(|i| mul(&wn, &n[i]));
                let b: [IBig; 3] = core::array::from_fn(|i| &w[i] - &along[i]);
                let cc = cross(n, &w);
                let (cu, su) = cos_sin_pi(u);
                core::array::from_fn(|i| &a[i] + &along[i] + mul(&cu, &b[i]) + mul(&su, &cc[i]))
            }
            Surface::Extrusion(profile, d) => {
                let p = profile.point(u);
                core::array::from_fn(|i| &p[i] + mul(v, &d[i]))
            }
            Surface::Ruled(first, second) => {
                let (p, q) = (first.point(u), second.point(u));
                core::array::from_fn(|i| &p[i] + mul(v, &(&q[i] - &p[i])))
            }
            Surface::Offset(base, d) => {
                let p = base.point(u, v);
                let (su, sv) = base.partials(u, v);
                let n = cross(&su, &sv);
                let len = sqrt(&dot(&n, &n)).expect("a square");
                core::array::from_fn(|i| &p[i] + div(&mul(d, &n[i]), &len).expect("regular"))
            }
            Surface::Image(e, base) => image(e, &base.point(u, v)),
            Surface::Within(_, base) => base.point(u, v),
        }
    }

    /// How far the kind's own definition may be from this truth: the deviations its construction
    /// stated.
    pub fn margin(&self) -> IBig {
        match self {
            Surface::Within(d, base) => d + base.margin(),
            Surface::Offset(base, _) | Surface::Image(_, base) => base.margin(),
            _ => IBig::ZERO,
        }
    }

    /// `S_u` and `S_v` by central differences with step 2^−60.
    pub fn partials(&self, u: &IBig, v: &IBig) -> ([IBig; 3], [IBig; 3]) {
        let h = one() >> 60;
        let diff = |a: [IBig; 3], b: [IBig; 3]| -> [IBig; 3] {
            core::array::from_fn(|i| (&a[i] - &b[i]) << 59)
        };
        (
            diff(self.point(&(u + &h), v), self.point(&(u - &h), v)),
            diff(self.point(u, &(v + &h)), self.point(u, &(v - &h))),
        )
    }

    /// Whether the truth leans on a difference or a coarse curve truth.
    pub fn coarse(&self) -> bool {
        match self {
            Surface::Offset(..) => true,
            Surface::Revolution(p, ..) | Surface::Extrusion(p, _) => p.coarse(),
            Surface::Ruled(p, q) => p.coarse() || q.coarse(),
            Surface::Image(_, b) | Surface::Within(_, b) => b.coarse(),
            _ => false,
        }
    }
}

/// The surface kinds, each with its truth: the analytic kinds, NURBS, the swept kinds and an
/// offset, and the affine image of a torus.
pub fn surfaces(rng: &mut Rng, grid: bool) -> Vec<(SurfaceKind, Surface)> {
    let c: [f64; 3] = rng.vector(40.0, grid);
    let (frame, fx_, fn_) = rng.frame::<3>(grid);
    let axes = frame_axes(fx_, fn_);
    let (r, k) = (rng.length(0.5, 5.0, grid), rng.value(1.5, grid));
    let (major, minor) = (rng.length(4.0, 8.0, grid), rng.length(0.5, 2.0, grid));
    let round = |w: Round, l: [f64; 2]| Surface::Round(w, fvec(c), axes.clone(), l.map(fx));
    let (a, b): ([f64; 3], [f64; 3]) = (rng.vector(3.0, grid), rng.vector(3.0, grid));
    let cylinder = Cylinder::new(c, r, frame, ((0.0, 2.0), (-1.0, 1.0))).unwrap();
    let torus = Torus::new(c, major, minor, frame, ((0.0, 2.0), (0.0, 2.0))).unwrap();
    let torus_truth = round(Round::Torus, [major, minor]);
    let nurbs = nurbs_surface(rng, grid, false);
    let gentle = nurbs_surface(rng, grid, true);
    let mut out = vec![
        (
            SurfaceKind::Plane(Plane::new(c, a, b, ((-1.0, 1.0), (-0.5, 2.0))).unwrap()),
            Surface::Plane([fvec(c), fvec(a), fvec(b)]),
        ),
        (
            SurfaceKind::Cylinder(cylinder.clone()),
            round(Round::Cylinder, [r, 0.0]),
        ),
        (
            SurfaceKind::Cylinder(Cylinder::new(c, r, frame, ((0.25, 1.5), (0.0, 3.0))).unwrap()),
            round(Round::Cylinder, [r, 0.0]),
        ),
        (
            SurfaceKind::Cone(Cone::new(c, r, k, frame, ((0.0, 2.0), (0.0, 1.0))).unwrap()),
            round(Round::Cone, [r, k]),
        ),
        (
            SurfaceKind::Sphere(Sphere::new(c, r, frame, ((0.0, 2.0), (-0.5, 0.5))).unwrap()),
            round(Round::Sphere, [r, 0.0]),
        ),
        (SurfaceKind::Torus(torus.clone()), torus_truth.clone()),
        (
            SurfaceKind::Nurbs(nurbs.clone()),
            Surface::Patch(patch_of(&nurbs)),
        ),
    ];
    // The affine image of a torus: a non-uniform scale makes one.
    let t = TypedTransform::from(Transform::scale(1.0, 2.0, 0.5).unwrap());
    let affine = SurfaceKind::Torus(torus)
        .transformed(&t, &mut meter())
        .unwrap();
    out.push((
        affine.value,
        Surface::Within(
            fx(affine.deviation),
            Box::new(Surface::Image(entries(&t), Box::new(torus_truth))),
        ),
    ));
    // The swept kinds, on profiles from the curve kinds.
    let profile = nurbs_curve::<3>(rng, grid);
    let profile_truth = Truth::Nurbs(spline_of(&profile));
    let pivot: [f64; 3] = rng.vector(5.0, grid);
    let revolution = Revolution::new(
        Trimmed::new(Basis::Nurbs(profile.clone()), (0.0, 1.0)).unwrap(),
        pivot,
        frame,
        (0.0, 1.5),
    )
    .unwrap();
    out.push((
        SurfaceKind::Revolution(revolution),
        Surface::Revolution(profile_truth.clone(), fvec(pivot), axes[2].clone()),
    ));
    let (circle_frame, cx, cn) = rng.frame::<3>(grid);
    let [cxh, cyh, _] = frame_axes(cx, cn);
    let circle = Circle::new(c, r, circle_frame, (0.0, 1.5)).unwrap();
    let rr = fx(r);
    let circle_truth = Truth::Form(
        Shape::Trig,
        [
            fvec(c),
            cxh.map(|x| mul(&rr, &x)),
            cyh.map(|x| mul(&rr, &x)),
            zero(),
        ],
    );
    let d: [f64; 3] = rng.vector(4.0, grid);
    out.push((
        SurfaceKind::Extrusion(
            Extrusion::new(
                Trimmed::new(Basis::Circle(circle), (0.25, 1.25)).unwrap(),
                d,
                (-1.0, 2.0),
            )
            .unwrap(),
        ),
        Surface::Extrusion(circle_truth, fvec(d)),
    ));
    let second = nurbs_curve::<3>(rng, grid);
    out.push((
        SurfaceKind::Ruled(
            Ruled::new(
                Trimmed::new(Basis::Nurbs(profile), (0.0, 1.0)).unwrap(),
                Trimmed::new(Basis::Nurbs(second.clone()), (0.0, 1.0)).unwrap(),
                (0.0, 1.0),
            )
            .unwrap(),
        ),
        Surface::Ruled(profile_truth, Truth::Nurbs(spline_of(&second))),
    ));
    // Offsets of bases whose normals never vanish.
    let dist = rng.length(0.125, 0.5, grid);
    for (base, truth) in [
        (
            SurfaceKind::Cylinder(cylinder),
            round(Round::Cylinder, [r, 0.0]),
        ),
        (
            SurfaceKind::Nurbs(gentle.clone()),
            Surface::Patch(patch_of(&gentle)),
        ),
    ] {
        let o = SurfaceOffset::new(base, dist).unwrap();
        out.push((
            SurfaceKind::Offset(o),
            Surface::Offset(Box::new(truth), fx(dist)),
        ));
    }
    out
}

/// What a transform is, for chapter 7's rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// A whole-step translation, quarter turn or mirror (`TypedTransform::Exact`).
    Grid,
    /// A rotation: lengths kept.
    Rigid,
    /// A uniform scale other than 1.
    Scale,
    /// Anything else.
    NonUniform,
}

/// The transforms each law applies, named: three of the grid, two rotations, a uniform and a
/// non-uniform scale; in the plane for 2D kinds.
pub fn transforms<const D: usize>(rng: &mut Rng) -> Vec<(TypedTransform, Class, &'static str)> {
    let mut k = [0i64; 3];
    for x in &mut k {
        *x = rng.below(2001) as i64 - 1000;
    }
    let planar = D == 2;
    let (turn, mirror, other) = if planar {
        (Axis::Z, Axis::Y, Axis::Z)
    } else {
        (Axis::X, Axis::Z, Axis::Y)
    };
    if planar {
        k[2] = 0;
    }
    let (s, z) = (1.25, if planar { 1.0 } else { 1.5 });
    vec![
        (
            TypedTransform::Exact(GridTransform::quarter_turns(turn, 1)),
            Class::Grid,
            "a quarter turn",
        ),
        (
            TypedTransform::Exact(GridTransform::mirror(mirror)),
            Class::Grid,
            "a mirror",
        ),
        (
            TypedTransform::Exact(GridTransform::translation(k).unwrap()),
            Class::Grid,
            "a whole-step translation",
        ),
        (
            TypedTransform::from(
                RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap(),
            ),
            Class::Rigid,
            "the 3-4-5 rotation",
        ),
        (
            TypedTransform::from(Transform::rotation(other, 0.37).unwrap()),
            Class::Rigid,
            "a rotation by 0.37 half-turns",
        ),
        (
            TypedTransform::from(Transform::scale(s, s, if planar { 1.0 } else { s }).unwrap()),
            Class::Scale,
            "a uniform scale",
        ),
        (
            TypedTransform::from(Transform::scale(2.0, 0.5, z).unwrap()),
            Class::NonUniform,
            "a non-uniform scale",
        ),
    ]
}

/// What a law found: the first failure of each case, and how many there were.
#[derive(Default)]
pub struct Findings {
    first: Vec<(String, String, usize)>,
}

impl Findings {
    /// A failure of `case`, described by `detail` the first time.
    pub fn add(&mut self, case: String, detail: impl FnOnce() -> String) {
        if let Some(f) = self.first.iter_mut().find(|f| f.0 == case) {
            f.2 += 1;
        } else {
            self.first.push((case, detail(), 1));
        }
    }

    /// Fails with every case found, if any.
    pub fn assert_none(&self, label: &str) {
        let lines: Vec<String> = self
            .first
            .iter()
            .map(|(c, d, n)| format!("- {c} ({n} times), first: {d}"))
            .collect();
        assert!(
            self.first.is_empty(),
            "{label}: {} broken:
{}",
            self.first.len(),
            lines.join(
                "
"
            )
        );
    }
}

/// A curve kind's name.
pub fn curve_name<const D: usize>(k: &CurveKind<D>) -> &'static str {
    match k {
        CurveKind::Nurbs(_) => "Nurbs",
        CurveKind::Line(_) => "Line",
        CurveKind::Circle(_) => "Circle",
        CurveKind::Ellipse(_) => "Ellipse",
        CurveKind::Hyperbola(_) => "Hyperbola",
        CurveKind::Parabola(_) => "Parabola",
        CurveKind::Helix(_) => "Helix",
        CurveKind::Polynomial(_) => "Polynomial",
        CurveKind::Involute(_) => "Involute",
        CurveKind::Bulge(_) => "Bulge",
        CurveKind::Trimmed(_) => "Trimmed",
        CurveKind::Composite(_) => "Composite",
        CurveKind::Offset(_) => "Offset",
        CurveKind::OnSurface(_) => "OnSurface",
        CurveKind::Transition(_) => "Transition",
        CurveKind::Intersection(_) => "Intersection",
    }
}

/// A surface kind's name.
pub fn surface_name(k: &SurfaceKind) -> &'static str {
    match k {
        SurfaceKind::Nurbs(_) => "Nurbs",
        SurfaceKind::Plane(_) => "Plane",
        SurfaceKind::Cylinder(_) => "Cylinder",
        SurfaceKind::Cone(_) => "Cone",
        SurfaceKind::Sphere(_) => "Sphere",
        SurfaceKind::Torus(_) => "Torus",
        SurfaceKind::Affine(_) => "Affine",
        SurfaceKind::Revolution(_) => "Revolution",
        SurfaceKind::Extrusion(_) => "Extrusion",
        SurfaceKind::Ruled(_) => "Ruled",
        SurfaceKind::Offset(_) => "Offset",
    }
}

/// The kind a curve becomes under a transform of class `class`, by chapter 7 and each kind's
/// stated rule: a circle stays a circle only under a similarity and is otherwise an ellipse; a
/// bulge polyline stays one under a similarity and is otherwise its NURBS form; an offset stays
/// one under a grid transform that moves its basis exactly (`exact`) and is otherwise its moved
/// cache; a transition stays one where lengths are kept and is otherwise its NURBS form; every
/// other kind keeps its kind.
pub fn curve_rule<const D: usize>(k: &CurveKind<D>, class: Class, exact: bool) -> &'static str {
    let name = curve_name(k);
    match (name, class) {
        ("Circle", Class::NonUniform) => "Ellipse",
        ("Offset", Class::Grid) if exact => name,
        ("Bulge", Class::NonUniform)
        | ("Offset", _)
        | ("Transition", Class::Scale | Class::NonUniform) => "Nurbs",
        _ => name,
    }
}

/// The kind a surface becomes, by chapter 7 and each kind's stated rule: the analytic kinds but
/// the plane stay themselves under a similarity and are otherwise affine images; an offset stays
/// one under a grid transform that moves its base exactly (`exact`) and is otherwise its moved
/// cache; every other kind keeps its kind.
pub fn surface_rule(k: &SurfaceKind, class: Class, exact: bool) -> &'static str {
    let name = surface_name(k);
    match (name, class) {
        ("Cylinder" | "Cone" | "Sphere" | "Torus", Class::NonUniform) => "Affine",
        ("Offset", Class::Grid) if exact => name,
        ("Offset", _) => "Nurbs",
        _ => name,
    }
}

/// All the work in release builds, an eighth in debug builds (and under WebAssembly).
pub fn scale(n: usize) -> usize {
    if cfg!(debug_assertions) {
        (n / 8).max(1)
    } else {
        n
    }
}

/// Parameters across a domain: its ends, the breaks given and their neighbours, n − 1 evenly
/// spaced and n random ones.
pub fn samples(rng: &mut Rng, (a, b): (f64, f64), breaks: &[f64], n: usize) -> Vec<f64> {
    let mut ts = vec![a, b];
    for &x in breaks {
        ts.extend([x, x.next_up(), x.next_down()]);
    }
    for i in 1..n {
        ts.push(a + (b - a) * (i as f64 / n as f64));
    }
    for _ in 0..n {
        ts.push(rng.range(a, b));
    }
    ts.retain(|&t| a <= t && t <= b);
    ts
}
