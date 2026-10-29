//! The replay hash of piece G62: the bulge polyline, trimmed, composite, offset and
//! curve-on-surface kinds run over a fixed corpus and hashed with SHA-256 must give the same
//! digest on every target (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed linear congruential sequence: 30 rounds, each of a bulge polyline (2D,
//! straight segments, semicircles and major arcs, with widths half the time), a trimmed NURBS
//! curve, a composite of a line, an arc and a NURBS curve, three offsets (of a circle in 2D, of a
//! helix against a reference direction, and of a helix by distances) and a curve on a surface,
//! each evaluated in doubles and enclosed at five parameters, bounded with two derivative bounds
//! over a span, converted to NURBS (knots, control points, weights, map and deviation), split,
//! reversed and transformed (each evaluated, with its deviation); and a refusal of each kind.
//! Every double is written as its bits and every interval as its two ends, each error as its name.

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
    Basis, BulgePolyline, Circle, Composite, CurveError, CurveKind, Frame, Helix, HelixAxes, Line,
    Offset, OffsetLaw, OnSurface, ParamCurve, ParamMap, Station, SurfaceRef, Trimmed,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Basis as Knots, Bounds, Changed, Curve, Limit};
use geom_run::{Budget, WorkMeter};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-curve/compound";

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
    fn eval<const D: usize, K: ParamCurve<D>>(&mut self, k: &K, ts: &[f64]) {
        let m = &mut meter();
        for &t in ts {
            let mut d = [[0.0; D]; 3];
            let r = k.derivatives(t, Limit::FromRight, &mut d).map(|()| d);
            self.result(r, |s, d| {
                for &x in d.iter().flatten() {
                    s.f64(x);
                }
            });
            self.result(k.enclose(t, Limit::FromRight, 2, m), |s, e| {
                for &x in e.iter().flatten() {
                    s.iv(x);
                }
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

fn record<const D: usize, K: ParamCurve<D>>(rec: &mut Recorder, k: &K, t: &TypedTransform) {
    let (a, b) = k.domain();
    let ts: Vec<f64> = (0..5).map(|q| a + (b - a) * f64::from(q) / 4.0).collect();
    rec.eval(k, &ts);
    let m = &mut meter();
    let span = (a + (b - a) * 0.25, a + (b - a) * 0.6);
    rec.result(k.bound(span, m), |s, x| s.bounds(&x));
    for order in 1..=2 {
        rec.result(k.derivative_bound(span, order, m), |s, x| match x {
            geom_curve::DerivativeBound::Bounded(x) => s.bounds(&x),
            geom_curve::DerivativeBound::Unbounded => s.bytes.push(9),
        });
    }
    rec.result(k.to_nurbs(m), |s, n| {
        s.f64(n.deviation);
        let c = &n.value.curve;
        for &x in c.basis().knots() {
            s.f64(x);
        }
        for &x in c.homogeneous().iter().flatten() {
            s.f64(x);
        }
        for &x in c.weights().unwrap_or(&[]) {
            s.f64(x);
        }
        match &n.value.map {
            ParamMap::Same => s.bytes.push(0),
            ParamMap::Circular(br) | ParamMap::Hyperbolic(br) => {
                s.bytes.push(1);
                for &x in br {
                    s.f64(x);
                }
            }
            ParamMap::Pieces(p) => {
                s.bytes.push(2);
                for piece in p {
                    s.f64(piece.start);
                    s.f64(piece.end);
                    s.f64(piece.shift);
                }
            }
        }
    });
    // Split at the middle sample, so each half holds its samples exactly.
    let (l, r) = k.split(ts[2], m).unwrap();
    rec.changed(Ok(l), &ts[..3]);
    rec.changed(Ok(r), &ts[2..]);
    let back: Vec<f64> = ts.iter().map(|t| -t).collect();
    rec.changed(k.reversed(m), &back);
    rec.changed(k.transformed(t, m), &ts);
}

fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    let mut rng = Lcg(62);
    let planar = transforms(true);
    let spatial = transforms(false);
    for i in 0..30 {
        let tp = &planar[i % planar.len()];
        let ts = &spatial[i % spatial.len()];
        // A bulge polyline.
        let n = 2 + rng.below(3);
        let vertices: Vec<[f64; 2]> = (0..n).map(|_| rng.vector(20.0)).collect();
        let bulges: Vec<f64> = (0..n - 1)
            .map(|j| match (i + j) % 4 {
                0 => 0.0,
                1 => 1.0,
                2 => rng.range(-4.0, 4.0),
                _ => rng.range(-0.5, 0.5),
            })
            .collect();
        let widths = (i % 2 == 0).then(|| {
            (0..n - 1)
                .map(|_| (rng.range(0.0, 1.0), rng.range(0.0, 1.0)))
                .collect()
        });
        let p = BulgePolyline::new(vertices, bulges, widths, false).unwrap();
        record(&mut rec, &p, tp);
        // A trimmed NURBS curve.
        let mut k = vec![0.0; 4];
        k.extend([1.0, 2.0]);
        k.extend([3.0; 4]);
        let basis = Knots::new(3, k).unwrap();
        let pts: Vec<[f64; 3]> = (0..basis.count()).map(|_| rng.vector(9.0)).collect();
        let c = Curve::new(basis, &pts).unwrap();
        let lo = rng.range(0.0, 1.2);
        let hi = rng.range(1.8, 3.0);
        let tr = Trimmed::new(Basis::Nurbs(c), (lo, hi)).unwrap();
        record(&mut rec, &tr, ts);
        // A composite of a line, an arc and a NURBS curve.
        let line = Line::new([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
        let circle = Circle::full([1.0, 1.0, 0.0], 1.0, Frame::standard().unwrap()).unwrap();
        let knots = Knots::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
        let spline =
            Curve::new(knots, &[[2.0, 1.0, 0.0], rng.vector(5.0), rng.vector(5.0)]).unwrap();
        let comp = Composite::new(vec![
            Trimmed::whole(Basis::Line(line)).unwrap(),
            Trimmed::new(Basis::Circle(circle), (1.5, 2.0)).unwrap(),
            Trimmed::whole(Basis::Nurbs(spline)).unwrap(),
        ])
        .unwrap();
        record(&mut rec, &comp, ts);
        // Offsets.
        let r = rng.range(3.0, 9.0);
        let c2 = Circle::new(
            rng.vector::<2>(5.0),
            r,
            Frame::standard().unwrap(),
            (0.0, 1.0),
        )
        .unwrap();
        let o2 = Offset::new(
            Trimmed::whole(Basis::Circle(c2)).unwrap(),
            OffsetLaw::Planar {
                distance: rng.range(-1.0, 1.0),
            },
        )
        .unwrap();
        record(&mut rec, &o2, tp);
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let axes = HelixAxes::Frame {
            frame,
            radius: r,
            pitch: rng.range(0.5, 3.0),
        };
        let h = Helix::new(rng.vector::<3>(5.0), axes, (0.0, 1.5)).unwrap();
        let o3 = Offset::new(
            Trimmed::whole(Basis::Helix(h.clone())).unwrap(),
            OffsetLaw::Reference {
                distance: 0.5,
                direction: [0.0, 0.0, 1.0],
            },
        )
        .unwrap();
        record(&mut rec, &o3, ts);
        let stations = vec![
            Station {
                at: 0.25,
                lateral: rng.range(-1.0, 1.0),
                vertical: 0.0,
            },
            Station {
                at: 1.0,
                lateral: rng.range(-1.0, 1.0),
                vertical: rng.range(0.0, 1.0),
            },
        ];
        let od = Offset::new(
            Trimmed::whole(Basis::Helix(h)).unwrap(),
            OffsetLaw::Distances(stations),
        )
        .unwrap();
        record(&mut rec, &od, ts);
        // A curve on a surface.
        let uv = Trimmed::whole(Basis::Line(
            Line::new([0.0, 0.0], [1.0, 2.0], (0.0, 1.0)).unwrap(),
        ))
        .unwrap();
        let cache = Curve::new(
            Knots::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap(),
            &[rng.vector::<3>(5.0), rng.vector::<3>(5.0)],
        )
        .unwrap();
        let on = OnSurface::new(uv, SurfaceRef(i as u64), cache, 1e-9).unwrap();
        record(&mut rec, &on, ts);
    }
    // One refusal of each kind.
    rec.result(
        BulgePolyline::<2>::new(vec![[0.0, 0.0], [0.0, 0.0]], vec![0.5], None, false),
        |_, _| {},
    );
    rec.result(
        BulgePolyline::<3>::new(vec![[0.0; 3], [1.0; 3]], vec![0.5], None, false),
        |_, _| {},
    );
    let line = Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
    rec.result(
        Trimmed::new(Basis::Line(line.clone()), (0.5, 2.0)),
        |_, _| {},
    );
    let far = Line::new([5.0, 0.0, 0.0], [1.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
    rec.result(
        Composite::new(vec![
            Trimmed::whole(Basis::Line(line.clone())).unwrap(),
            Trimmed::whole(Basis::Line(far)).unwrap(),
        ]),
        |_, _| {},
    );
    rec.result(
        Offset::new(
            Trimmed::whole(Basis::Line(line.clone())).unwrap(),
            OffsetLaw::Planar { distance: 1.0 },
        ),
        |_, _| {},
    );
    rec.result(
        Offset::new(
            Trimmed::whole(Basis::Line(line)).unwrap(),
            OffsetLaw::Distances(vec![]),
        ),
        |_, _| {},
    );
    rec
}

#[test]
fn compound_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-curve compound corpus: {} results ({} refusals), {} bytes, golden hash {}",
        rec.results,
        rec.refusals,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert_eq!(rec.results, 10_506);
    assert_eq!(rec.refusals, 6, "only the six refusals asked for");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different point, bound or \
             conversion (compare this log's digest with another target's), or a definition changed \
             in place, which chapter 26.2 forbids"
        )
    });
}
