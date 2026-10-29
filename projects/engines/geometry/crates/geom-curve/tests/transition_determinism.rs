//! The replay hash of piece G63: the transition kind's laws run over a fixed corpus and hashed with
//! SHA-256 must give the same digest on every target (chapters 21 and 28.13; the golden store of
//! piece G19).
//!
//! The corpus, from a fixed linear congruential sequence: 30 rounds, each of one transition per law
//! (clothoid, Helmert, Bloss, cosine, sine, cant-coupled, and IFC's polynomial, cosine and sine
//! spirals) with random start, direction, length and terms, each with its panel count and
//! remainder, and at six stations its point and three derivatives in doubles, its enclosures to the
//! third derivative, its turn and its curvature; and a refusal of each kind. Every double is written
//! as its bits and every interval as its two ends, each error as its name, so a target that rounds
//! one step differently changes the digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    clippy::unwrap_used,
    clippy::panic,
    reason = "the corpus generator; the rules are for kernel code"
)]

use geom_curve::{CurvatureLaw, CurveError, Transition};
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-curve/transition";

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
    /// A transition built and evaluated at six stations.
    fn transition(&mut self, start: [f64; 2], direction: [f64; 2], length: f64, law: CurvatureLaw) {
        let built = Transition::new(start, direction, length, law, &mut meter());
        self.result(built, |s, t| {
            s.f64(t.panels() as f64);
            s.f64(t.remainder());
            for q in [0.0, 0.1, 0.37, 0.5, 0.83, 1.0] {
                let station = length * q;
                let mut d = [[0.0; 2]; 4];
                let r = t.derivatives(station, &mut d).map(|()| d);
                s.result(r, |s, d| d.iter().flatten().for_each(|&x| s.f64(x)));
                s.result(t.enclose(station, 3, &mut meter()), |s, e| {
                    e.iter().flatten().for_each(|&x| s.iv(x));
                });
                s.result(t.turn(station), Recorder::iv);
                s.result(t.curvature(station), Recorder::iv);
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
    let mut rng = Lcg(63);
    for _ in 0..30 {
        round(&mut rec, &mut rng);
    }
    // One refusal of each kind.
    let law = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 0.01,
    };
    let mut zero = [None; 8];
    zero[3] = Some(0.0);
    for (start, direction, length, law) in [
        ([0.0, 0.0], [0.0, 0.0], 1.0, law),
        ([f64::NAN, 0.0], [1.0, 0.0], 1.0, law),
        (
            [0.0, 0.0],
            [1.0, 0.0],
            1.0,
            CurvatureLaw::Clothoid {
                start: 1e9,
                end: 1e9,
            },
        ),
        (
            [0.0, 0.0],
            [1.0, 0.0],
            1.0,
            CurvatureLaw::Polynomial {
                terms: zero,
                from: 0.0,
            },
        ),
    ] {
        rec.result(
            Transition::new(start, direction, length, law, &mut meter()),
            |_, _| {},
        );
    }
    let t = Transition::new([0.0, 0.0], [1.0, 0.0], 1.0, law, &mut meter()).unwrap();
    rec.result(t.point(2.0), |_, _| {});
    rec
}

#[test]
fn transition_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-curve transition corpus: {} results ({} refusals), {} bytes, golden hash {}",
        rec.results,
        rec.refusals,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert_eq!(rec.results, 6755);
    assert_eq!(rec.refusals, 5, "only the five refusals asked for");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different point, enclosure, \
             turn or curvature (compare this log's digest with another target's), or a definition \
             changed in place, which chapter 26.2 forbids"
        )
    });
}
