//! Attacks on piece G61's claims: every enclosure, bound, derivative bound, NURBS conversion and
//! transform of the analytic kinds is checked against an independent reference in 400-bit fixed
//! point: cos, sin and exp by their series with π to 100 digits, derivatives by central
//! differences (so the closed-form derivatives are checked, not reused), NURBS forms by de
//! Casteljau's algorithm on their homogeneous control points, and the conic maps by their
//! tangents. A call may refuse an attack; it may never return a bound that is false.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::needless_range_loop,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_curve::{
    Axes, Circle, CurveKind, Ellipse, Frame, Helix, HelixAxes, Hyperbola, Involute, Line,
    NurbsForm, Parabola, ParamCurve, ParamMap, Polynomial,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Limit};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The fixed point: n stands for n / 2^F.
const F: usize = 400;

fn one() -> IBig {
    IBig::ONE << F
}

/// A double, exactly but for a truncation below 2^−400.
fn fx(x: f64) -> IBig {
    let q = RBig::try_from(x).unwrap();
    (q.numerator().clone() << F) / IBig::from(q.denominator().clone())
}

/// n / d.
fn frac(n: i64, d: i64) -> IBig {
    (IBig::from(n) << F) / IBig::from(d)
}

fn mul(a: &IBig, b: &IBig) -> IBig {
    (a * b) >> F
}

fn div(a: &IBig, b: &IBig) -> IBig {
    (a << F) / b
}

fn abs(a: &IBig) -> IBig {
    if *a < IBig::ZERO {
        -a.clone()
    } else {
        a.clone()
    }
}

/// π to 100 decimal places.
fn pi() -> IBig {
    let digits: IBig = "31415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679"
        .parse()
        .unwrap();
    (digits << F) / IBig::from(10).pow(100)
}

/// cos x and sin x by their series, for |x| ≤ 4.
fn cos_sin(x: &IBig) -> (IBig, IBig) {
    let mut term = one();
    let (mut c, mut s) = (IBig::ZERO, IBig::ZERO);
    for n in 0..400usize {
        match n % 4 {
            0 => c += &term,
            1 => s += &term,
            2 => c -= &term,
            _ => s -= &term,
        }
        term = mul(&term, x) / IBig::from(n + 1);
        if term == IBig::ZERO {
            break;
        }
    }
    (c, s)
}

/// cos πt and sin πt, t reduced exactly to [−1, 1) first.
fn cos_sin_pi(t: &IBig) -> (IBig, IBig) {
    let k = (t + one()) >> (F + 1);
    let r = t - (k << (F + 1));
    cos_sin(&mul(&r, &pi()))
}

/// e^x by its series, for |x| ≤ 16.
fn exp(x: &IBig) -> IBig {
    let mut term = one();
    let mut s = IBig::ZERO;
    for n in 0..1000usize {
        s += &term;
        term = mul(&term, x) / IBig::from(n + 1);
        if term == IBig::ZERO {
            break;
        }
    }
    s
}

/// The scalar functions of each analytic kind.
#[derive(Clone, Copy, Debug)]
enum Shape {
    Linear,
    Trig,
    Hyperbolic,
    Parabolic,
    Helical,
    Involute,
}

/// A kind's exact definition: c + f₁(t)·u + f₂(t)·v + f₃(t)·w, or a polynomial.
#[derive(Clone, Debug)]
enum Truth<const D: usize> {
    Form(Shape, [[IBig; D]; 4]),
    Poly(Vec<[IBig; D]>),
}

fn fvec<const D: usize>(v: [f64; D]) -> [IBig; D] {
    v.map(fx)
}

fn zero<const D: usize>() -> [IBig; D] {
    core::array::from_fn(|_| IBig::ZERO)
}

impl<const D: usize> Truth<D> {
    fn point(&self, t: &IBig) -> [IBig; D] {
        match self {
            Truth::Form(shape, [c, u, v, w]) => {
                let f = match shape {
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
                        let th = mul(t, &pi());
                        [&c + mul(&th, &s), &s - mul(&th, &c), IBig::ZERO]
                    }
                };
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
        }
    }

    /// The k-th derivative (k ≤ 3) by central differences with step 2^−60: error about
    /// 2^−120 times the (k + 2)-th derivative, and 2^−220 from the fixed point.
    fn derivative(&self, t: &IBig, k: usize) -> [IBig; D] {
        let h = one() >> 60;
        let at = |j: i64| self.point(&(t + &h * IBig::from(j)));
        let combine = |terms: &[(i64, [IBig; D])], shift: usize| -> [IBig; D] {
            core::array::from_fn(|i| {
                let mut s = IBig::ZERO;
                for (c, p) in terms {
                    s += IBig::from(*c) * &p[i];
                }
                s << shift
            })
        };
        match k {
            0 => self.point(t),
            1 => combine(&[(1, at(1)), (-1, at(-1))], 59),
            2 => combine(&[(1, at(1)), (-2, at(0)), (1, at(-1))], 120),
            _ => combine(&[(1, at(2)), (-2, at(1)), (2, at(-1)), (-1, at(-2))], 179),
        }
    }
}

/// The tolerance of the reference for order k at a value of size `size`.
fn tolerance(k: usize, size: &IBig) -> IBig {
    if k == 0 {
        one() >> 300
    } else {
        (one() + abs(size)) >> 100
    }
}

/// Whether the enclosure can hold the exact value: it meets the reference interval
/// [truth − tol, truth + tol], which holds the exact value. A miss by more than the reference's
/// tolerance (2^−300, or 2^−100 relative for a derivative) is caught.
fn holds(e: Interval, truth: &IBig, tol: &IBig) -> bool {
    fx(e.lo()) <= truth + tol && truth - tol <= fx(e.hi())
}

fn holds_box<const D: usize>(b: &Bounds<D>, truth: &[IBig; D], tol: &IBig) -> bool {
    (0..D).all(|i| fx(b.lo[i]) <= &truth[i] + tol && &truth[i] - tol <= fx(b.hi[i]))
}

/// |a − b|² ≤ r² in fixed point.
fn within<const D: usize>(a: &[IBig; D], b: &[IBig; D], r: &IBig) -> bool {
    let mut s = IBig::ZERO;
    for i in 0..D {
        let d = &a[i] - &b[i];
        s += mul(&d, &d);
    }
    s <= mul(r, r)
}

/// A NURBS form's point at the fixed parameter u: its Bezier piece by de Casteljau on homogeneous
/// points (every analytic kind's NURBS form is Bezier pieces with p-fold knots).
fn nurbs_point<const D: usize>(n: &geom_nurbs::Curve<D>, u: &IBig) -> [IBig; D] {
    let p = n.degree();
    let knots = n.basis().knots();
    let mut distinct: Vec<f64> = knots.to_vec();
    distinct.dedup();
    let mut j = 0;
    while j + 2 < distinct.len() && fx(distinct[j + 1]) < *u {
        j += 1;
    }
    let (a, b) = (fx(distinct[j]), fx(distinct[j + 1]));
    let v = div(&(u - &a), &(&b - &a));
    let w = n.weights();
    let mut pts: Vec<([IBig; D], IBig)> = (0..=p)
        .map(|i| {
            let k = j * p + i;
            (fvec(n.homogeneous()[k]), w.map_or(one(), |w| fx(w[k])))
        })
        .collect();
    let vm = one() - &v;
    for r in 1..=p {
        for i in 0..=p - r {
            let (x, y) = (pts[i].clone(), pts[i + 1].clone());
            pts[i] = (
                core::array::from_fn(|d| mul(&vm, &x.0[d]) + mul(&v, &y.0[d])),
                mul(&vm, &x.1) + mul(&v, &y.1),
            );
        }
    }
    let (h, w) = &pts[0];
    core::array::from_fn(|d| div(&h[d], w))
}

/// The exact NURBS parameter for the kind's t under a conic map.
fn map_truth(map: &ParamMap, t: &IBig) -> IBig {
    let (breaks, circular) = match map {
        ParamMap::Same => return t.clone(),
        ParamMap::Circular(b) => (b, true),
        ParamMap::Hyperbolic(b) => (b, false),
    };
    let mut k = 0;
    while k + 2 < breaks.len() && fx(breaks[k + 1]) < *t {
        k += 1;
    }
    let (t0, t1) = (fx(breaks[k]), fx(breaks[k + 1]));
    let tau = |x: &IBig| {
        if circular {
            let (c, s) = cos_sin(&mul(x, &pi()));
            div(&s, &c)
        } else {
            let e = exp(&(x << 1));
            div(&(&e - one()), &(&e + one()))
        }
    };
    let sigma = tau(&((&t1 - &t0) >> 1));
    let kf = IBig::from(k) << F;
    if k % 2 == 0 {
        kf + div(&tau(&((t - &t0) >> 1)), &sigma)
    } else {
        kf + one() - div(&tau(&((&t1 - t) >> 1)), &sigma)
    }
}

/// A transform's exact entries in fixed point: rows (m0, m1, m2, t).
fn entries(t: &TypedTransform) -> [[IBig; 4]; 3] {
    match t {
        TypedTransform::Exact(g) => {
            let r = RationalTransform::from_grid(g);
            rational(&r)
        }
        TypedTransform::Rational(r) => rational(r),
        TypedTransform::Certified(c) => c.rows().map(|row| row.map(fx)),
    }
}

fn rational(r: &RationalTransform) -> [[IBig; 4]; 3] {
    let d = IBig::from(r.denominator());
    let m = r.matrix();
    let t = r.translation_part();
    core::array::from_fn(|i| {
        core::array::from_fn(|j| {
            let n = if j < 3 {
                IBig::from(m[i][j])
            } else {
                IBig::from(t[i])
            };
            (n << F) / &d
        })
    })
}

fn image<const D: usize>(e: &[[IBig; 4]; 3], p: &[IBig; D]) -> [IBig; D] {
    core::array::from_fn(|i| {
        let mut s = e[i][3].clone();
        for j in 0..D {
            s += mul(&e[i][j], &p[j]);
        }
        s
    })
}

/// Test inputs only: Knuth's MMIX linear congruential sequence.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
    fn vector<const D: usize>(&mut self, s: f64) -> [f64; D] {
        core::array::from_fn(|_| self.range(-s, s))
    }
}

/// A frame whose unit vectors are rational: (3, 4, 0)/5 and its quarter turn, about z.
fn pythagorean<const D: usize>() -> (Frame<D>, [IBig; D], [IBig; D]) {
    let mut x = [0.0; D];
    x[0] = 3.0;
    x[1] = 4.0;
    let f = Frame::new(x, [0.0, 0.0, 1.0]).unwrap();
    let mut xh = zero::<D>();
    let mut yh = zero::<D>();
    xh[0] = frac(3, 5);
    xh[1] = frac(4, 5);
    yh[0] = frac(-4, 5);
    yh[1] = frac(3, 5);
    (f, xh, yh)
}

fn scaled<const D: usize>(s: f64, v: &[IBig; D]) -> [IBig; D] {
    let k = fx(s);
    core::array::from_fn(|i| mul(&k, &v[i]))
}

/// The kinds attacked, each with its exact definition.
fn attacked<const D: usize>(rng: &mut Lcg) -> Vec<(CurveKind<D>, Truth<D>)> {
    let c: [f64; D] = rng.vector(40.0);
    let (u, v, w): ([f64; D], [f64; D], [f64; D]) =
        (rng.vector(9.0), rng.vector(9.0), rng.vector(2.0));
    let r = rng.range(0.25, 12.0);
    let (frame, xh, yh) = pythagorean::<D>();
    let form =
        |s: Shape, w: [IBig; D], u: [IBig; D], v: [IBig; D]| Truth::Form(s, [fvec(c), u, v, w]);
    let mut out = vec![
        (
            CurveKind::Line(Line::new(c, u, (-2.5, 3.0)).unwrap()),
            form(Shape::Linear, zero(), fvec(u), zero()),
        ),
        (
            CurveKind::Circle(Circle::new(c, r, frame, (-0.75, 1.0)).unwrap()),
            form(Shape::Trig, zero(), scaled(r, &xh), scaled(r, &yh)),
        ),
        (
            CurveKind::Ellipse(Ellipse::new(c, Axes::Vectors { u, v }, (-1.0, 1.0)).unwrap()),
            form(Shape::Trig, zero(), fvec(u), fvec(v)),
        ),
        (
            CurveKind::Hyperbola(Hyperbola::new(c, Axes::Vectors { u, v }, (-2.0, 2.5)).unwrap()),
            form(Shape::Hyperbolic, zero(), fvec(u), fvec(v)),
        ),
        (
            CurveKind::Parabola(
                Parabola::new(
                    c,
                    Axes::Frame {
                        frame,
                        a: r,
                        b: 2.0 * r,
                    },
                    (-3.0, 2.0),
                )
                .unwrap(),
            ),
            form(
                Shape::Parabolic,
                zero(),
                scaled(r, &xh),
                scaled(2.0 * r, &yh),
            ),
        ),
        (
            CurveKind::Involute(
                Involute::with_axes(c, Axes::Vectors { u, v }, (-1.5, 2.5)).unwrap(),
            ),
            form(Shape::Involute, zero(), fvec(u), fvec(v)),
        ),
    ];
    let coeffs: Vec<[f64; D]> = (0..5).map(|_| rng.vector(3.0)).collect();
    out.push((
        CurveKind::Polynomial(Polynomial::new(coeffs.clone(), (-1.5, 2.0)).unwrap()),
        Truth::Poly(coeffs.into_iter().map(fvec).collect()),
    ));
    if D == 3 {
        out.push((
            CurveKind::Helix(Helix::new(c, HelixAxes::Vectors { u, v, w }, (-3.0, 5.0)).unwrap()),
            form(Shape::Helical, fvec(w), fvec(u), fvec(v)),
        ));
    }
    out
}

/// Parameters across a domain: ends, quarter turns, their neighbours, and random ones.
fn samples(rng: &mut Lcg, (a, b): (f64, f64)) -> Vec<f64> {
    let mut ts = vec![a, b];
    let mut q = (a * 2.0).ceil() / 2.0;
    while q <= b {
        ts.extend([q, q.next_up(), q.next_down()]);
        q += 0.5;
    }
    for _ in 0..12 {
        ts.push(rng.range(a, b));
    }
    ts.retain(|&t| a <= t && t <= b);
    ts
}

fn attack_enclosures<const D: usize>(seed: u64) {
    let mut rng = Lcg(seed);
    let m = &mut meter();
    for trial in 0..12 {
        for (kind, truth) in attacked::<D>(&mut rng) {
            for t in samples(&mut rng, kind.domain()) {
                let e = kind.enclose(t, Limit::FromRight, 3, m).unwrap();
                for k in 0..=3 {
                    let d = truth.derivative(&fx(t), k);
                    for i in 0..D {
                        let tol = tolerance(k, &d[i]);
                        assert!(
                            holds(e[k][i], &d[i], &tol),
                            "{D}D trial {trial}: {kind:?} order {k} at {t}: {:?}",
                            e[k][i]
                        );
                    }
                }
            }
        }
    }
}

/// Claim: every enclosure of a point and its first three derivatives holds the exact value,
/// at the ends, at quarter turns and either side of them.
#[test]
fn enclosures_hold_the_exact_values() {
    attack_enclosures::<2>(611);
    attack_enclosures::<3>(612);
}

/// Claim: a bound over any span holds the curve, and a derivative bound its derivative, at every
/// parameter of the span.
#[test]
fn bounds_hold_over_every_span() {
    let mut rng = Lcg(613);
    let m = &mut meter();
    for _ in 0..8 {
        for (kind, truth) in attacked::<3>(&mut rng) {
            let (a, b) = kind.domain();
            for _ in 0..4 {
                let lo = rng.range(a, b);
                let hi = rng.range(lo, b);
                let boxes = [
                    kind.bound((lo, hi), m).unwrap(),
                    match kind.derivative_bound((lo, hi), 1, m).unwrap() {
                        geom_curve::DerivativeBound::Bounded(x) => x,
                        geom_curve::DerivativeBound::Unbounded => panic!("analytic kinds bound"),
                    },
                    match kind.derivative_bound((lo, hi), 2, m).unwrap() {
                        geom_curve::DerivativeBound::Bounded(x) => x,
                        geom_curve::DerivativeBound::Unbounded => panic!("analytic kinds bound"),
                    },
                ];
                for j in 0..=10 {
                    let t = (lo + (hi - lo) * f64::from(j) / 10.0).clamp(lo, hi);
                    for (k, bx) in boxes.iter().enumerate() {
                        let d = truth.derivative(&fx(t), k);
                        let tol = tolerance(k, &d[0]);
                        assert!(
                            holds_box(bx, &d, &tol),
                            "{kind:?} order {k} at {t} in [{lo}, {hi}]"
                        );
                    }
                }
            }
        }
    }
}

/// Claim: the NURBS form at the parameter its map gives lies within the stated deviation of the
/// curve, for the exact conic maps and the Taylor forms alike.
#[test]
fn nurbs_forms_keep_their_deviation() {
    let mut rng = Lcg(614);
    let m = &mut meter();
    for _ in 0..6 {
        for (kind, truth) in attacked::<3>(&mut rng) {
            let n = kind.to_nurbs(m).unwrap();
            let NurbsForm { curve, map } = &n.value;
            let dev = fx(n.deviation) + (one() >> 200);
            for t in samples(&mut rng, kind.domain()) {
                let tf = fx(t);
                let p = truth.point(&tf);
                let q = nurbs_point(curve, &map_truth(map, &tf));
                assert!(
                    within(&p, &q, &dev),
                    "{kind:?} at {t}: deviation {} not enough",
                    n.deviation
                );
            }
        }
    }
}

/// Claim: a transformed kind lies within its deviation of the exact image, under exact, rational,
/// certified, non-uniform and mirroring transforms.
#[test]
fn transforms_keep_their_deviation() {
    let all = [
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1)),
        TypedTransform::Exact(GridTransform::mirror(Axis::Y)),
        TypedTransform::from(
            RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap(),
        ),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.37).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 1.0).unwrap()),
        TypedTransform::from(Transform::scale(1.25, 1.25, 1.0).unwrap()),
    ];
    let mut rng = Lcg(615);
    let m = &mut meter();
    for trial in 0..6 {
        for (kind, truth) in attacked::<2>(&mut rng) {
            for tr in &all {
                let moved = kind.transformed(tr, m).unwrap();
                let e = entries(tr);
                let dev = fx(moved.deviation) + (one() >> 200);
                for t in samples(&mut rng, kind.domain()) {
                    let exact = image(&e, &truth.point(&fx(t)));
                    let got = moved.value.enclose(t, Limit::FromRight, 0, m).unwrap()[0];
                    // The moved curve's own enclosure holds its exact point; widen it by the
                    // deviation and it must meet the exact image.
                    for i in 0..2 {
                        assert!(
                            fx(got[i].lo()) - &dev <= exact[i]
                                && exact[i] <= fx(got[i].hi()) + &dev,
                            "trial {trial}: {kind:?} under {tr:?} at {t}"
                        );
                    }
                }
            }
        }
    }
}

/// Claim: a helix many turns long, and an involute far from its circle, still evaluate, bound and
/// convert within their stated errors.
#[test]
fn long_helices_and_far_involutes() {
    let m = &mut meter();
    let (frame, xh, yh) = pythagorean::<3>();
    let h = Helix::new(
        [1.0, -2.0, 3.0],
        HelixAxes::Frame {
            frame,
            radius: 3.0,
            pitch: 0.5,
        },
        (999.5, 1003.0),
    )
    .unwrap();
    let truth = Truth::Form(
        Shape::Helical,
        [
            fvec([1.0, -2.0, 3.0]),
            scaled(3.0, &xh),
            scaled(3.0, &yh),
            [IBig::ZERO, IBig::ZERO, frac(1, 4)],
        ],
    );
    let n = h.to_nurbs(m).unwrap();
    assert!(n.deviation < 1e-11, "{}", n.deviation);
    for t in [999.5, 1000.0, 1000.25, 1001.123, 1003.0] {
        let e = h.enclose(t, Limit::FromRight, 1, m).unwrap();
        for k in 0..=1 {
            let d = truth.derivative(&fx(t), k);
            for i in 0..3 {
                assert!(
                    holds(e[k][i], &d[i], &tolerance(k, &d[i])),
                    "helix order {k} at {t}"
                );
            }
        }
        let q = nurbs_point(&n.value.curve, &fx(t));
        assert!(within(
            &truth.point(&fx(t)),
            &q,
            &(fx(n.deviation) + (one() >> 200))
        ));
    }
    let inv = Involute::new([0.0, 0.0], 2.0, Frame::standard().unwrap(), (8.0, 10.0)).unwrap();
    let n = inv.to_nurbs(m).unwrap();
    assert!(n.deviation < 1e-10, "{}", n.deviation);
}
