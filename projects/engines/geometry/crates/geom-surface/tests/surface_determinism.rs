//! The replay hash of piece G65: the analytic and swept surface kinds run over a fixed corpus and
//! hashed with SHA-256 must give the same digest on every target (chapters 21 and 28.13; the
//! golden store of piece G19).
//!
//! The corpus, from a fixed linear congruential sequence: 12 rounds of a plane, cylinder, cone,
//! sphere and torus on random frames, and a surface of revolution, an extrusion and a ruled surface
//! on fixed profiles, each enclosed at nine parameter pairs, bounded (with both derivative bounds
//! and the normal bound) over a box, converted to NURBS (knots, net, weights, maps, deviation),
//! split and reversed in each direction and transformed (each enclosed, with its deviation); and a
//! refusal of each kind. Every double is written as its bits and every interval as its two ends.

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
    Basis as Leaf, Circle, CurveError, DerivativeBound, Frame, Line, ParamMap, Trimmed,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Direction, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, Extrusion, ParamBox, ParamSurface, Plane, Revolution, Ruled, Sphere,
    SurfaceKind, Torus,
};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-surface/analytic";

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
    fn vector(&mut self, s: f64) -> [f64; 3] {
        [self.range(-s, s), self.range(-s, s), self.range(-s, s)]
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
    fn bounds(&mut self, b: &Bounds<3>) {
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
    fn map(&mut self, m: &ParamMap) {
        match m {
            ParamMap::Same => self.bytes.push(0),
            ParamMap::Circular(b) | ParamMap::Hyperbolic(b) => {
                self.bytes.push(1);
                for &x in b {
                    self.f64(x);
                }
            }
            ParamMap::Pieces(_) => self.bytes.push(2),
        }
    }
    fn enclose<K: ParamSurface>(&mut self, k: &K, ps: &[(f64, f64)]) {
        let m = &mut meter();
        for &(u, v) in ps {
            let r = k.enclose(u, v, (Limit::FromRight, Limit::FromRight), m);
            self.result(r, |s, e| {
                for &x in e.iter().flatten() {
                    s.iv(x);
                }
            });
        }
    }
    fn changed(&mut self, c: Result<Changed<SurfaceKind>, CurveError>, ps: &[(f64, f64)]) {
        self.result(c, |s, c| {
            s.f64(c.deviation);
            s.enclose(&c.value, ps);
        });
    }
}

fn samples(((a, b), (c, d)): ParamBox) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for i in 0..3 {
        for j in 0..3 {
            out.push((
                a + (b - a) * f64::from(i) / 2.0,
                c + (d - c) * f64::from(j) / 2.0,
            ));
        }
    }
    out
}

fn record(rec: &mut Recorder, k: &SurfaceKind, t: &TypedTransform) {
    let d = k.domain();
    let ps = samples(d);
    rec.enclose(k, &ps);
    let m = &mut meter();
    let ((a, b), (c, e)) = d;
    let region = (
        (a + (b - a) * 0.25, a + (b - a) * 0.6),
        (c + (e - c) * 0.3, c + (e - c) * 0.7),
    );
    rec.result(k.bound(region, m), |s, x| s.bounds(&x));
    for dir in [Direction::U, Direction::V] {
        rec.result(k.derivative_bound(region, dir, m), |s, x| match x {
            DerivativeBound::Bounded(x) => s.bounds(&x),
            DerivativeBound::Unbounded => s.bytes.push(9),
        });
    }
    rec.result(k.normal_bound(region, m), |s, x| s.bounds(&x));
    rec.result(k.to_nurbs(m), |s, n| {
        s.f64(n.deviation);
        let f = &n.value.surface;
        for &x in f.basis_u().knots().iter().chain(f.basis_v().knots()) {
            s.f64(x);
        }
        for &x in f.homogeneous().iter().flatten() {
            s.f64(x);
        }
        for &x in f.weights().unwrap_or(&[]) {
            s.f64(x);
        }
        s.map(&n.value.u);
        s.map(&n.value.v);
    });
    for dir in [Direction::U, Direction::V] {
        let mid = match dir {
            Direction::U => f64::midpoint(a, b),
            Direction::V => f64::midpoint(c, e),
        };
        rec.result(k.split(dir, mid, m), |s, (l, r)| {
            s.f64(l.deviation);
            s.f64(r.deviation);
        });
        rec.changed(k.reversed(dir, m), &[]);
    }
    rec.changed(k.transformed(t, m), &ps);
}

fn frame(rng: &mut Lcg) -> Frame<3> {
    loop {
        if let Ok(f) = Frame::new(rng.vector(1.0), rng.vector(1.0)) {
            return f;
        }
    }
}

fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    let mut rng = Lcg(65);
    let all = [
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1)),
        TypedTransform::from(
            RationalTransform::rotation(Axis::Y, Rotation::direction(3, 4).unwrap()).unwrap(),
        ),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap()),
    ];
    let axis = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let arc = Trimmed::whole(Leaf::Circle(
        Circle::new([0.0; 3], 2.0, Frame::standard().unwrap(), (0.0, 1.0)).unwrap(),
    ))
    .unwrap();
    let line = Trimmed::whole(Leaf::Line(
        Line::new([2.0, 0.0, 1.0], [1.0, 0.5, 2.0], (0.0, 1.0)).unwrap(),
    ))
    .unwrap();
    for i in 0..12 {
        let t = &all[i % all.len()];
        let c = rng.vector(20.0);
        let kinds = vec![
            SurfaceKind::Plane(
                Plane::new(
                    c,
                    rng.vector(5.0),
                    rng.vector(5.0),
                    ((0.0, 2.0), (-1.0, 1.0)),
                )
                .unwrap(),
            ),
            SurfaceKind::Cylinder(
                Cylinder::new(
                    c,
                    rng.range(1.0, 5.0),
                    frame(&mut rng),
                    ((0.0, 1.5), (0.0, 3.0)),
                )
                .unwrap(),
            ),
            SurfaceKind::Cone(
                Cone::new(
                    c,
                    rng.range(1.0, 5.0),
                    rng.range(-0.5, 0.5),
                    frame(&mut rng),
                    ((-0.5, 0.5), (0.0, 2.0)),
                )
                .unwrap(),
            ),
            SurfaceKind::Sphere(
                Sphere::new(
                    c,
                    rng.range(1.0, 5.0),
                    frame(&mut rng),
                    ((0.0, 2.0), (-0.5, 0.25)),
                )
                .unwrap(),
            ),
            SurfaceKind::Torus(
                Torus::new(
                    c,
                    rng.range(4.0, 6.0),
                    rng.range(1.0, 3.0),
                    frame(&mut rng),
                    ((0.0, 1.0), (-1.0, 1.0)),
                )
                .unwrap(),
            ),
            SurfaceKind::Revolution(
                Revolution::new(line.clone(), [0.0; 3], axis, (0.0, 1.5)).unwrap(),
            ),
            SurfaceKind::Extrusion(
                Extrusion::new(arc.clone(), rng.vector(3.0), (0.0, 2.0)).unwrap(),
            ),
            SurfaceKind::Ruled(Ruled::new(line.clone(), arc.clone(), (0.0, 1.0)).unwrap()),
        ];
        for k in &kinds {
            record(&mut rec, k, t);
        }
    }
    let f = Frame::standard().unwrap();
    rec.result(
        Cylinder::new([0.0; 3], -1.0, f, ((0.0, 1.0), (0.0, 1.0))),
        |_, _| {},
    );
    rec.result(
        Sphere::new([0.0; 3], 1.0, f, ((0.0, 1.0), (0.0, 1.0))),
        |_, _| {},
    );
    rec.result(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        ),
        |_, _| {},
    );
    rec.result(
        Revolution::new(line.clone(), [0.0; 3], axis, (0.0, 3.0)),
        |_, _| {},
    );
    rec.result(Extrusion::new(arc.clone(), [0.0; 3], (0.0, 1.0)), |_, _| {});
    rec
}

#[test]
fn surface_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-surface analytic corpus: {} results ({} refusals), {} bytes, golden hash {}",
        rec.results,
        rec.refusals,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert_eq!(rec.refusals, 5, "only the five refusals asked for");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different point, bound or \
             conversion, or a definition changed in place, which chapter 26.2 forbids"
        )
    });
}
