//! The replay hash of piece G66: the offset surface kind run over a fixed corpus and hashed with
//! SHA-256 must give the same digest on every target (chapters 21 and 28.13; the golden store of
//! piece G19).
//!
//! The corpus, from a fixed linear congruential sequence: 3 rounds of offsets of a bicubic NURBS
//! patch with an interior knot (rational in the second round), of a cylinder and of a torus on
//! random frames, and in the first an offset of an offset, each enclosed at nine parameter pairs, with its
//! partials to order 3 at three, bounded (with both derivative bounds and the normal bound) over a
//! box, converted to NURBS by default and the first patch's offset within 10⁻¹ (knots, net,
//! weights, maps, deviation), split and reversed in each direction and transformed (each enclosed,
//! with its deviation; the offset of an offset is not converted, and moves by a grid mirror,
//! which keeps the kind); and 4 refusals. Every double is written as its bits and every interval as
//! its two ends.

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

use geom_curve::{CurveError, DerivativeBound, Frame, ParamMap};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Bounds, Changed, Direction, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cylinder, Offset, ParamBox, ParamSurface, Plane, SurfaceForm, SurfaceKind, Torus,
};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-surface/offset";

const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

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
    fn form(&mut self, n: &Changed<SurfaceForm>) {
        self.f64(n.deviation);
        let f = &n.value.surface;
        for &x in f.basis_u().knots().iter().chain(f.basis_v().knots()) {
            self.f64(x);
        }
        for &x in f.homogeneous().iter().flatten() {
            self.f64(x);
        }
        for &x in f.weights().unwrap_or(&[]) {
            self.f64(x);
        }
        self.map(&n.value.u);
        self.map(&n.value.v);
    }
    fn enclose<K: ParamSurface>(&mut self, k: &K, ps: &[(f64, f64)]) {
        let m = &mut meter();
        for &(u, v) in ps {
            let r = k.enclose(u, v, SIDES, m);
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

/// What `record` converts: nothing (an offset of an offset, whose cache costs the most), the
/// default cache, or that and a cache within 10⁻¹.
#[derive(Clone, Copy, PartialEq)]
enum Convert {
    None,
    Default,
    Within,
}

fn record(rec: &mut Recorder, k: &Offset, t: &TypedTransform, convert: Convert) {
    // A converted offset keeps its cache, so the non-uniform scale moves the same one.
    let kept = match convert {
        Convert::None => k.clone(),
        Convert::Default | Convert::Within => k.cached(&mut meter()).unwrap(),
    };
    let k = &SurfaceKind::Offset(kept);
    let d = k.domain();
    let ps = samples(d);
    rec.enclose(k, &ps);
    let m = &mut meter();
    for &(u, v) in ps.iter().skip(2).step_by(3) {
        rec.result(k.partials_over(((u, u), (v, v)), SIDES, 3, m), |s, p| {
            for &x in p.iter().flatten() {
                s.iv(x);
            }
        });
    }
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
    if convert != Convert::None {
        rec.result(k.to_nurbs(m), |s, n| s.form(&n));
    }
    if convert == Convert::Within
        && let SurfaceKind::Offset(o) = k
    {
        rec.result(o.to_nurbs_within(1e-1, m), |s, n| s.form(&n));
    }
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
    if convert == Convert::None {
        let grid = TypedTransform::Exact(GridTransform::mirror(Axis::Z));
        rec.changed(k.transformed(&grid, m), &ps);
    } else {
        rec.changed(k.transformed(t, m), &ps);
    }
}

fn frame(rng: &mut Lcg) -> Frame<3> {
    loop {
        if let Ok(f) = Frame::new(rng.vector(1.0), rng.vector(1.0)) {
            return f;
        }
    }
}

/// A bicubic patch over [0, 1]² with a knot at u = 1/2, rational when asked.
fn patch(rng: &mut Lcg, rational: bool) -> Surface<3> {
    let bu = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let bv = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut pts = Vec::new();
    let mut ws = Vec::new();
    for i in 0..5 {
        for j in 0..4 {
            pts.push([f64::from(i) * 0.75, f64::from(j), rng.range(-0.3, 0.3)]);
            ws.push(rng.range(0.95, 1.05));
        }
    }
    if rational {
        Surface::new_rational(bu, bv, &pts, &ws).unwrap()
    } else {
        Surface::new(bu, bv, &pts).unwrap()
    }
}

fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    let rng = &mut Lcg(0x6606_d37e);
    let transforms = [
        TypedTransform::Exact(GridTransform::mirror(Axis::Z)),
        TypedTransform::from(
            RationalTransform::rotation(Axis::X, Rotation::direction(3, 4).unwrap()).unwrap(),
        ),
        TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap()),
    ];
    for (round, t) in transforms.iter().enumerate() {
        let nurbs = SurfaceKind::Nurbs(patch(rng, round == 1));
        let off = Offset::new(nurbs, rng.range(-0.4, 0.4)).unwrap();
        let convert = if round == 0 {
            Convert::Within
        } else {
            Convert::Default
        };
        record(&mut rec, &off, t, convert);
        let c = rng.vector(10.0);
        let cyl = Cylinder::new(
            c,
            rng.range(1.0, 4.0),
            frame(rng),
            ((0.0, 0.5), (-1.0, 1.0)),
        )
        .unwrap();
        let k = Offset::new(SurfaceKind::Cylinder(cyl), rng.range(-0.8, 0.8)).unwrap();
        record(&mut rec, &k, t, Convert::Default);
        let torus = Torus::new(c, 5.0, 1.5, frame(rng), ((0.0, 0.4), (-0.3, 0.3))).unwrap();
        let k = Offset::new(SurfaceKind::Torus(torus), rng.range(-0.8, 0.8)).unwrap();
        record(&mut rec, &k, t, Convert::Default);
        if round == 0 {
            let twice = Offset::new(SurfaceKind::Offset(off), rng.range(-0.3, 0.3)).unwrap();
            record(&mut rec, &twice, t, Convert::None);
        }
    }
    // Refusals: a distance that is not finite, a parameter outside, an unbounded plane's cache,
    // and a tolerance of zero.
    let m = &mut meter();
    let base = SurfaceKind::Nurbs(patch(rng, false));
    rec.result(Offset::new(base.clone(), f64::NAN), |_, _| {});
    let off = Offset::new(base, 0.25).unwrap();
    rec.result(off.enclose(2.0, 0.5, SIDES, m), |_, _| {});
    let plane = Plane::new(
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        ((0.0, f64::INFINITY), (0.0, 1.0)),
    )
    .unwrap();
    let flat = Offset::new(SurfaceKind::Plane(plane), 1.0).unwrap();
    rec.result(flat.to_nurbs(m), |_, _| {});
    rec.result(off.to_nurbs_within(0.0, m), |_, _| {});
    rec
}

#[test]
fn offset_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-surface offset corpus: {} results ({} refusals), {} bytes, golden hash {}",
        rec.results,
        rec.refusals,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert_eq!(rec.refusals, 4, "only the four refusals asked for");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different point, bound or \
             conversion, or a definition changed in place, which chapter 26.2 forbids"
        )
    });
}
