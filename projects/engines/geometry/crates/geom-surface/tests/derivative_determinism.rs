//! The replay hash of piece G68: every surface kind's second-derivative and normal-derivative
//! bounds over a fixed corpus of boxes, hashed with SHA-256, must give the same digest on every
//! target (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed linear congruential sequence: 6 rounds of a rational bicubic NURBS
//! patch with an interior knot each way and weights from 2⁻¹⁰ to 2¹⁰ (with 10⁻³ and 10³ among
//! them), and a nonrational biquadratic one, each over its whole domain, a quarter of it, a box of
//! a hundredth of each side and a box across its interior knots; and a plane, a cylinder, a cone
//! (a box at its apex and one away from it), a sphere (a box at its pole and one away), a torus
//! and an affine image of a torus on random frames, over one box each. For every box: the
//! partials to order 2, the second-derivative bound with its chord constant M, and the
//! normal-derivative bound (each partial bounded or not); and 2 refusals. Every double is written
//! as its bits and every interval as its two ends.

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

use geom_curve::{CurveError, DerivativeBound, Frame};
use geom_math::{Transform, TypedTransform};
use geom_nurbs::{Basis, Bounds, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, ParamBox, ParamSurface, Plane, SecondDerivativeBound, Sphere, SurfaceKind,
    Torus,
};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-surface/derivative-bounds";

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
    fn below(&mut self, n: u64) -> u64 {
        (self.next() >> 33) % n
    }
}

/// 2^e, exactly.
fn pow2(e: i64) -> f64 {
    let mut x = 1.0;
    for _ in 0..e.unsigned_abs() {
        x *= 2.0;
    }
    if e < 0 { 1.0 / x } else { x }
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
    /// Every bound of piece G68 over one box.
    fn region<K: ParamSurface>(&mut self, k: &K, r: ParamBox) {
        let m = &mut meter();
        self.result(k.partials_over(r, SIDES, 2, m), |s, p| {
            for x in p.iter().flatten() {
                s.f64(x.lo());
                s.f64(x.hi());
            }
        });
        self.result(k.second_derivative_bound(r, SIDES, m), |s, b| match b {
            SecondDerivativeBound::Bounded(b) => {
                s.bytes.push(1);
                s.bounds(&b.uu);
                s.bounds(&b.uv);
                s.bounds(&b.vv);
                s.result(b.chord(), Recorder::f64);
            }
            SecondDerivativeBound::Unbounded => s.bytes.push(0),
        });
        self.result(k.normal_derivative_bound(r, SIDES, m), |s, n| {
            for b in n {
                match b {
                    DerivativeBound::Bounded(b) => {
                        s.bytes.push(1);
                        s.bounds(&b);
                    }
                    DerivativeBound::Unbounded => s.bytes.push(0),
                }
            }
        });
    }
}

fn frame(rng: &mut Lcg) -> Frame<3> {
    loop {
        if let Ok(f) = Frame::new(rng.vector(1.0), rng.vector(1.0)) {
            return f;
        }
    }
}

/// A patch of degree `p` both ways with one interior knot each way.
fn patch(rng: &mut Lcg, p: usize, rational: bool) -> Surface<3> {
    let knots = |rng: &mut Lcg| {
        let mut k = vec![0.0; p + 1];
        k.push(rng.range(0.3, 0.7));
        k.extend(std::iter::repeat_n(1.0, p + 1));
        Basis::new(p, k).unwrap()
    };
    let (bu, bv) = (knots(rng), knots(rng));
    let n = bu.count() * bv.count();
    let pts: Vec<[f64; 3]> = (0..n).map(|_| rng.vector(10.0)).collect();
    if !rational {
        return Surface::new(bu, bv, &pts).unwrap();
    }
    let mut w: Vec<f64> = (0..n)
        .map(|_| pow2(rng.below(21).cast_signed() - 10) * rng.range(1.0, 2.0))
        .collect();
    w[0] = 1e-3;
    w[n - 1] = 1e3;
    Surface::new_rational(bu, bv, &pts, &w).unwrap()
}

fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    let rng = &mut Lcg(0x6768);
    for _ in 0..6 {
        for (p, rational) in [(3, true), (2, false)] {
            let s = patch(rng, p, rational);
            let ku = s.basis_u().knots()[p + 1];
            let kv = s.basis_v().knots()[p + 1];
            let (u, v) = (rng.range(0.0, 0.99), rng.range(0.0, 0.99));
            for r in [
                ((0.0, 1.0), (0.0, 1.0)),
                ((0.25, 0.5), (0.5, 0.75)),
                ((u, u + 0.01), (v, v + 0.01)),
                ((ku - 0.05, ku + 0.05), (kv - 0.05, kv + 0.05)),
            ] {
                rec.region(&s, r);
            }
        }
        let c = rng.vector(20.0);
        let f = frame(rng);
        let kinds = [
            (
                SurfaceKind::Plane(
                    Plane::new(
                        c,
                        rng.vector(3.0),
                        rng.vector(3.0),
                        ((-1.0, 1.0), (-1.0, 1.0)),
                    )
                    .unwrap(),
                ),
                ((-0.5, 0.25), (0.0, 1.0)),
            ),
            (
                SurfaceKind::Cylinder(
                    Cylinder::new(c, rng.range(0.5, 5.0), f, ((0.0, 2.0), (-1.0, 1.0))).unwrap(),
                ),
                ((0.1, 0.3), (-0.5, 0.5)),
            ),
            (
                SurfaceKind::Cone(
                    Cone::new(c, 0.0, rng.range(0.5, 2.0), f, ((0.0, 2.0), (0.0, 1.0))).unwrap(),
                ),
                ((0.0, 0.25), (0.0, 0.5)),
            ),
            (
                SurfaceKind::Cone(
                    Cone::new(c, 0.0, rng.range(0.5, 2.0), f, ((0.0, 2.0), (0.0, 1.0))).unwrap(),
                ),
                ((0.0, 0.0625), (0.5, 0.5625)),
            ),
            (
                SurfaceKind::Sphere(
                    Sphere::new(c, rng.range(0.5, 5.0), f, ((0.0, 2.0), (-0.5, 0.5))).unwrap(),
                ),
                ((0.0, 0.5), (0.25, 0.5)),
            ),
            (
                SurfaceKind::Sphere(
                    Sphere::new(c, rng.range(0.5, 5.0), f, ((0.0, 2.0), (-0.5, 0.5))).unwrap(),
                ),
                ((0.0, 0.0625), (-0.25, -0.1875)),
            ),
            (
                SurfaceKind::Torus(
                    Torus::new(c, 5.0, rng.range(0.5, 2.0), f, ((0.0, 2.0), (0.0, 2.0))).unwrap(),
                ),
                ((0.0, 0.125), (0.25, 0.375)),
            ),
        ];
        for (k, r) in &kinds {
            rec.region(k, *r);
        }
        let torus = SurfaceKind::Torus(
            Torus::new(c, 5.0, rng.range(0.5, 2.0), f, ((0.0, 2.0), (0.0, 2.0))).unwrap(),
        );
        let t = TypedTransform::from(Transform::scale(1.0, 2.0, 0.5).unwrap());
        let affine = torus.transformed(&t, &mut meter()).unwrap().value;
        rec.region(&affine, ((0.0, 0.125), (0.25, 0.375)));
    }
    // Refusals: a box outside the domain, for each bound.
    let s = patch(rng, 3, true);
    let m = &mut meter();
    rec.result(
        s.second_derivative_bound(((-1.0, 1.0), (0.0, 1.0)), SIDES, m),
        |_, _| {},
    );
    rec.result(
        s.normal_derivative_bound(((0.0, 1.0), (0.5, 2.0)), SIDES, m),
        |_, _| {},
    );
    rec
}

#[test]
fn derivative_bounds_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-surface derivative-bounds corpus: {} results ({} refusals), {} bytes, golden hash {}",
        rec.results,
        rec.refusals,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert_eq!(rec.refusals, 2, "only the two refusals asked for");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different bound, or a \
             definition changed in place, which chapter 26.2 forbids"
        )
    });
}
