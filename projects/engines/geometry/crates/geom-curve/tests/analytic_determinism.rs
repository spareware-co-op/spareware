//! The replay hash of piece G61: the analytic curve kinds run over a fixed corpus and hashed with
//! SHA-256 must give the same digest on every target (chapters 21 and 28.13; the golden store of
//! piece G19).
//!
//! The corpus, from a fixed linear congruential sequence: 40 rounds, each of one line, circle,
//! ellipse, hyperbola, parabola, polynomial and involute in 2D and the same with a helix in 3D,
//! each with its point and two derivatives in doubles and enclosed at five parameters, its bound
//! and derivative bounds over a span, its NURBS form (knots, control points, weights, map and
//! deviation), its split, its reversal and its image under a transform (each evaluated at the
//! same parameters, with its deviation); and a refusal of each kind. Every double is written as
//! its bits and every interval as its two ends, each error as its name, so a target that rounds
//! one step differently changes the digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "the corpus generator; the rules are for kernel code"
)]

use geom_curve::{
    Axes, Circle, CurveError, CurveKind, Ellipse, Frame, Helix, HelixAxes, Hyperbola, Involute,
    Line, Parabola, ParamCurve, ParamMap, Polynomial,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit};
use geom_run::{Budget, WorkMeter};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-curve/analytic";

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
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
    fn below(&mut self, n: usize) -> usize {
        ((self.next() >> 33) % n as u64) as usize
    }
    fn vector<const D: usize>(&mut self, s: f64) -> [f64; D] {
        core::array::from_fn(|_| self.range(-s, s))
    }
}

#[derive(Default)]
struct Recorder {
    bytes: Vec<u8>,
    results: u32,
    refusals: u32,
}

impl Recorder {
    fn f64(&mut self, v: f64) {
        self.bytes.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    fn iv(&mut self, i: Interval) {
        self.f64(i.lo());
        self.f64(i.hi());
    }
    fn bounds<const D: usize>(&mut self, b: &Bounds<D>) {
        for &x in b.lo.iter().chain(&b.hi) {
            self.f64(x);
        }
    }
    fn result<T>(&mut self, r: Result<T, CurveError>, f: impl FnOnce(&mut Self, T)) {
        self.results += 1;
        match r {
            Ok(v) => {
                self.bytes.push(1);
                f(self, v);
            }
            Err(e) => {
                self.refusals += 1;
                self.bytes.push(0);
                self.bytes.extend_from_slice(format!("{e:?}").as_bytes());
            }
        }
    }
    /// A kind evaluated at the parameters: doubles, then enclosures.
    fn eval<const D: usize, K: ParamCurve<D>>(&mut self, k: &K, ts: &[f64]) {
        let m = &mut meter();
        for &t in ts {
            let mut d = [[0.0; D]; 3];
            let r = k.derivatives(t, Limit::FromRight, &mut d).map(|()| d);
            self.result(r, |s, d| d.iter().flatten().for_each(|&x| s.f64(x)));
            self.result(k.enclose(t, Limit::FromRight, 2, m), |s, e| {
                e.iter().flatten().for_each(|&x| s.iv(x));
            });
        }
    }
    fn changed<const D: usize>(
        &mut self,
        c: Result<Changed<CurveKind<D>>, CurveError>,
        ts: &[f64],
    ) {
        self.result(c, |s, c| {
            s.f64(c.deviation);
            s.eval(&c.value, ts);
        });
    }
}

fn frame<const D: usize>(rng: &mut Lcg) -> Frame<D> {
    loop {
        let x = rng.vector::<D>(1.0);
        let n = if D == 2 {
            [0.0, 0.0, if rng.below(2) == 0 { 1.0 } else { -1.0 }]
        } else {
            rng.vector::<3>(1.0)
        };
        if let Ok(f) = Frame::new(x, n) {
            return f;
        }
    }
}

fn axes<const D: usize>(rng: &mut Lcg) -> Axes<D> {
    if rng.below(2) == 0 {
        Axes::Frame {
            frame: frame(rng),
            a: rng.range(0.5, 20.0),
            b: rng.range(0.5, 20.0),
        }
    } else {
        Axes::Vectors {
            u: rng.vector(20.0),
            v: rng.vector(20.0),
        }
    }
}

fn kinds<const D: usize>(rng: &mut Lcg) -> Vec<CurveKind<D>> {
    let c = rng.vector::<D>(50.0);
    let mut out = vec![
        CurveKind::Line(Line::new(c, rng.vector(10.0), (-1.0, 2.0)).unwrap()),
        CurveKind::Circle(Circle::new(c, rng.range(0.1, 30.0), frame(rng), (-0.5, 1.25)).unwrap()),
        CurveKind::Ellipse(Ellipse::new(c, axes(rng), (0.0, 2.0)).unwrap()),
        CurveKind::Hyperbola(Hyperbola::new(c, axes(rng), (-2.0, 1.5)).unwrap()),
        CurveKind::Parabola(Parabola::new(c, axes(rng), (-3.0, 3.0)).unwrap()),
        CurveKind::Polynomial(
            Polynomial::new(
                (0..2 + rng.below(6)).map(|_| rng.vector(5.0)).collect(),
                (-1.0, 1.5),
            )
            .unwrap(),
        ),
        CurveKind::Involute(Involute::with_axes(c, axes(rng), (-1.0, 2.0)).unwrap()),
    ];
    if D == 3 {
        let ax = HelixAxes::Frame {
            frame: frame(rng),
            radius: rng.range(0.5, 10.0),
            pitch: rng.range(-8.0, 8.0),
        };
        out.push(CurveKind::Helix(Helix::new(c, ax, (-2.0, 4.0)).unwrap()));
    }
    out
}

fn transforms(planar: bool) -> Vec<TypedTransform> {
    let mut out = vec![
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1)),
        TypedTransform::from(
            RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap(),
        ),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 1.0).unwrap()),
    ];
    if !planar {
        out.push(TypedTransform::from(
            Transform::scale(1.5, 0.75, 3.0).unwrap(),
        ));
    }
    out
}

fn round<const D: usize>(rec: &mut Recorder, rng: &mut Lcg, i: usize) {
    let all = transforms(D == 2);
    for (j, k) in kinds::<D>(rng).into_iter().enumerate() {
        let (a, b) = k.domain();
        let ts: Vec<f64> = (0..5).map(|q| a + (b - a) * f64::from(q) / 4.0).collect();
        rec.eval(&k, &ts);
        let m = &mut meter();
        let span = (a + (b - a) * 0.25, a + (b - a) * 0.6);
        rec.result(k.bound(span, m), |s, x| s.bounds(&x));
        for order in 1..=3 {
            rec.result(k.derivative_bound(span, order, m), |s, x| match x {
                geom_curve::DerivativeBound::Bounded(x) => s.bounds(&x),
                geom_curve::DerivativeBound::Unbounded => s.bytes.push(9),
            });
        }
        rec.result(k.to_nurbs(m), |s, n| {
            s.f64(n.deviation);
            let c = &n.value.curve;
            c.basis().knots().iter().for_each(|&x| s.f64(x));
            c.homogeneous().iter().flatten().for_each(|&x| s.f64(x));
            c.weights().unwrap_or(&[]).iter().for_each(|&x| s.f64(x));
            match &n.value.map {
                ParamMap::Same => s.bytes.push(0),
                ParamMap::Pieces(_) => s.bytes.push(2),
                ParamMap::Circular(br) | ParamMap::Hyperbolic(br) => {
                    s.bytes.push(1);
                    for &x in br {
                        s.f64(x);
                    }
                }
            }
        });
        let (l, r) = k.split(f64::midpoint(a, b), m).unwrap();
        rec.changed(Ok(l), &ts[..3]);
        rec.changed(Ok(r), &ts[2..]);
        let back: Vec<f64> = ts.iter().map(|t| -t).collect();
        rec.changed(k.reversed(m), &back);
        rec.changed(k.transformed(&all[(i + j) % all.len()], m), &ts);
    }
}

fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    let mut rng = Lcg(61);
    for i in 0..40 {
        round::<2>(&mut rec, &mut rng, i);
        round::<3>(&mut rec, &mut rng, i);
    }
    // One refusal of each kind.
    let m = &mut meter();
    let std2 = Frame::<2>::standard().unwrap();
    let ray = Line::ray([0.0, 0.0], [0.1, 0.7]).unwrap();
    rec.result(ray.to_nurbs(m), |_, _| {});
    rec.result(
        ray.transformed(
            &TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
            m,
        ),
        |_, _| {},
    );
    rec.result(Circle::new([0.0; 2], 1.0, std2, (0.0, 2.5)), |_, _| {});
    rec.result(Circle::new([0.0; 2], -1.0, std2, (0.0, 1.0)), |_, _| {});
    rec.result(Frame::<3>::new([1.0, 2.0, 3.0], [2.0, 4.0, 6.0]), |_, _| {});
    rec.result(
        Circle::new([0.0; 2], 1.0, std2, (0.0, 1.0))
            .unwrap()
            .transformed(
                &TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1)),
                m,
            ),
        |_, _| {},
    );
    rec
}

#[test]
fn analytic_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-curve analytic corpus: {} results ({} refusals), {} bytes, golden hash {}",
        rec.results,
        rec.refusals,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert_eq!(rec.results, 30_606);
    assert_eq!(rec.refusals, 6, "only the six refusals asked for");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different point, bound or \
             conversion (compare this log's digest with another target's), or a definition changed \
             in place, which chapter 26.2 forbids"
        )
    });
}
