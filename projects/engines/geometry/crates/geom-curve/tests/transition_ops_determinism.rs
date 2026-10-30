//! The replay hash of piece G64: the transition kind's conversion to NURBS, split, reversal, parts,
//! bounds and typed transforms run over a fixed corpus and hashed with SHA-256 must give the same
//! digest on every target (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed linear congruential sequence: 6 rounds, each of one transition per law
//! (clothoid, Helmert, Bloss, cosine, sine, cant-coupled, and IFC's polynomial, cosine and sine
//! spirals) with random start, direction, length and terms, each with its NURBS form (every knot,
//! control point and the deviation), its split at 0.37 of its length (both parts' domains and their
//! enclosures there from each side), its reversal (domain and an enclosure), a reversed part of it
//! (domain, an enclosure and its bound), its bound and derivative bounds over a span, and its
//! images under a certified rotation and a uniform scale of the plane (kind, deviation and an enclosure); and a
//! refusal of each kind. Every double is written as its bits and every interval as its two ends,
//! each error as its name, so a target that rounds one step differently changes the digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    clippy::unwrap_used,
    clippy::panic,
    reason = "the corpus generator; the rules are for kernel code"
)]

use geom_curve::{CurvatureLaw, CurveError, CurveKind, DerivativeBound, ParamCurve, Transition};
use geom_math::{Axis, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Limit};
use geom_run::{Budget, WorkMeter};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-curve/transition-operations";

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
    /// A signed term of magnitude in [lo, hi].
    fn term(&mut self, lo: f64, hi: f64) -> f64 {
        let v = self.range(lo, hi);
        if self.next() >> 63 == 1 { -v } else { v }
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
    fn bounds(&mut self, b: &Bounds<2>) {
        b.lo.iter().chain(&b.hi).for_each(|&x| self.f64(x));
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
    /// An enclosure of a curve at t to order 2.
    fn enclosed<K: ParamCurve<2>>(&mut self, k: &K, t: f64, side: Limit) {
        self.result(k.enclose(t, side, 2, &mut meter()), |s, e| {
            e.iter().flatten().for_each(|&x| s.iv(x));
        });
    }
    /// A kind's tag and an enclosure of it at t.
    fn kind(&mut self, k: &CurveKind<2>, t: f64) {
        self.bytes.push(match k {
            CurveKind::Transition(_) => 1,
            CurveKind::Nurbs(_) => 2,
            _ => 3,
        });
        self.enclosed(k, t, Limit::FromRight);
    }
    /// A transition run through every operation of piece G64.
    fn transition(&mut self, start: [f64; 2], direction: [f64; 2], length: f64, law: CurvatureLaw) {
        let built = Transition::new(start, direction, length, law, &mut meter());
        self.result(built, |s, t| {
            s.result(t.to_nurbs(&mut meter()), |s, n| {
                let knots = n.value.curve.basis().knots();
                s.f64(knots.len() as f64);
                for &k in knots {
                    s.f64(k);
                }
                n.value
                    .curve
                    .homogeneous()
                    .iter()
                    .flatten()
                    .for_each(|&x| s.f64(x));
                s.f64(n.deviation);
            });
            let at = length * 0.37;
            s.result(t.split(at, &mut meter()), |s, (left, right)| {
                for part in [&left, &right] {
                    let (a, b) = part.value.domain();
                    s.f64(a);
                    s.f64(b);
                    s.f64(part.deviation);
                }
                s.enclosed(&left.value, at, Limit::FromLeft);
                s.enclosed(&right.value, at, Limit::FromRight);
            });
            s.result(t.reversed(&mut meter()), |s, r| {
                let (a, b) = r.value.domain();
                s.f64(a);
                s.f64(b);
                s.enclosed(&r.value, -0.5 * length, Limit::FromLeft);
            });
            s.result(t.segment((0.2 * length, 0.7 * length), true), |s, p| {
                let (a, b) = p.domain();
                s.f64(a);
                s.f64(b);
                s.enclosed(&p, -0.45 * length, Limit::FromRight);
                s.result(p.bound((a, b), &mut meter()), |s, b| s.bounds(&b));
            });
            let span = (0.1 * length, 0.9 * length);
            s.result(t.bound(span, &mut meter()), |s, b| s.bounds(&b));
            for order in [1, 2, 4] {
                s.result(
                    t.derivative_bound(span, order, &mut meter()),
                    |s, d| match d {
                        DerivativeBound::Bounded(b) => s.bounds(&b),
                        DerivativeBound::Unbounded => s.bytes.push(9),
                    },
                );
            }
            for tr in [
                TypedTransform::from(Transform::rotation(Axis::Z, 0.61).unwrap()),
                TypedTransform::from(Transform::scale(1.5, 1.5, 1.0).unwrap()),
            ] {
                s.result(t.transformed(&tr, &mut meter()), |s, moved| {
                    s.f64(moved.deviation);
                    s.kind(&moved.value, 0.5 * length);
                });
            }
        });
    }
}

fn round(rec: &mut Recorder, rng: &mut Lcg) {
    let place = |rng: &mut Lcg| {
        (
            [rng.range(-5e3, 5e3), rng.range(-5e3, 5e3)],
            [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)],
            rng.range(10.0, 300.0),
        )
    };
    let curvatures = |rng: &mut Lcg| (rng.term(0.0, 0.02), rng.term(0.0, 0.02));
    let (k1, k2) = curvatures(rng);
    let (s, d, l) = place(rng);
    rec.transition(s, d, l, CurvatureLaw::Clothoid { start: k1, end: k2 });
    let (k1, k2) = curvatures(rng);
    let (s, d, l) = place(rng);
    rec.transition(s, d, l, CurvatureLaw::Helmert { start: k1, end: k2 });
    let (k1, k2) = curvatures(rng);
    let (s, d, l) = place(rng);
    rec.transition(s, d, l, CurvatureLaw::Bloss { start: k1, end: k2 });
    let (k1, k2) = curvatures(rng);
    let (s, d, l) = place(rng);
    rec.transition(s, d, l, CurvatureLaw::Cosine { start: k1, end: k2 });
    let (k1, k2) = curvatures(rng);
    let (s, d, l) = place(rng);
    rec.transition(s, d, l, CurvatureLaw::Sine { start: k1, end: k2 });
    let (k1, k2) = curvatures(rng);
    let (s, d, l) = place(rng);
    let (height, cant) = (rng.range(1.0, 3.0), rng.term(0.0, 0.2));
    rec.transition(
        s,
        d,
        l,
        CurvatureLaw::CantCoupled {
            start: k1,
            end: k2,
            height,
            cant,
        },
    );
    let mut terms = [None; 8];
    for (k, slot) in terms.iter_mut().enumerate() {
        if rng.next() >> 62 != 0 {
            // |Aₖ| between about 50 and 500·(k + 1), so each term's curvature stays modest.
            *slot = Some(rng.term(50.0, 500.0 * (k as f64 + 1.0)));
        }
    }
    let (s, d, l) = place(rng);
    let from = rng.range(-20.0, 40.0);
    rec.transition(s, d, l, CurvatureLaw::Polynomial { terms, from });
    let (s, d, l) = place(rng);
    let (constant, cosine, length, from) = (
        rng.term(500.0, 5000.0),
        rng.term(100.0, 1000.0),
        rng.range(50.0, 400.0),
        rng.range(-20.0, 40.0),
    );
    rec.transition(
        s,
        d,
        l,
        CurvatureLaw::CosineSpiral {
            constant: Some(constant),
            cosine,
            length,
            from,
        },
    );
    let (s, d, l) = place(rng);
    let (constant, linear, sine, length, from) = (
        rng.term(500.0, 5000.0),
        rng.term(30.0, 300.0),
        rng.term(100.0, 1000.0),
        rng.range(50.0, 400.0),
        rng.range(-20.0, 40.0),
    );
    rec.transition(
        s,
        d,
        l,
        CurvatureLaw::SineSpiral {
            constant: Some(constant),
            linear: Some(linear),
            sine,
            length,
            from,
        },
    );
}

fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    let mut rng = Lcg(64);
    for _ in 0..6 {
        round(&mut rec, &mut rng);
    }
    // One refusal of each kind.
    let law = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 0.01,
    };
    let t = Transition::new([0.0, 0.0], [1.0, 0.0], 10.0, law, &mut meter()).unwrap();
    rec.result(t.split(0.0, &mut meter()), |_, _| {});
    rec.result(t.segment((4.0, 2.0), false), |_, _| {});
    rec.result(t.bound((-1.0, 2.0), &mut meter()), |_, _| {});
    let tilt = TypedTransform::from(Transform::rotation(Axis::X, 0.3).unwrap());
    rec.result(t.transformed(&tilt, &mut meter()), |_, _| {});
    rec.result(
        Transition::<3>::new([0.0; 3], [1.0, 0.0, 0.0], 10.0, law, &mut meter()),
        |_, _| {},
    );
    let straight = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 0.0,
    };
    let far = Transition::new([f64::MAX, 0.0], [1.0, 0.0], 1e300, straight, &mut meter()).unwrap();
    rec.result(far.to_nurbs(&mut meter()), |_, _| {});
    rec
}

#[test]
fn transition_operations_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-curve transition operations corpus: {} results ({} refusals), {} bytes, golden \
         hash {}",
        rec.results,
        rec.refusals,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert_eq!(rec.results, 978);
    assert_eq!(rec.refusals, 6, "only the six refusals expected");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different NURBS form, part, \
             bound or image (compare this log's digest with another target's), or a definition \
             changed in place, which needs a new version and a store record (chapter 26.2)"
        )
    });
}
