//! The replay hash of piece G51: Brent's zero and minimum, the quadrature rules and the
//! integrator, run over a fixed corpus and hashed with SHA-256, must give the same digest on every
//! target (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus: the generated rules' every node and weight; `zero` on four function families
//! (linear, cubic, triple and steep zeros) at fifty roots, three brackets and two tolerances;
//! `minimum` on six shapes (parabolas, cubics, kinks, quartics, staircases and an upside-down
//! parabola) at eight places and two tolerances; `gauss_legendre` with every rule size on five
//! integrands (one with values in the thousands); and `integrate` with every Kronrod rule, two
//! absolute and two relative tolerances, on the same integrands. Every point each search and
//! integration evaluated is recorded too, in order, so a step that changes and still reaches the
//! same answer changes the digest. Each result is written in a canonical byte
//! encoding (doubles as their bits, counts as little-endian integers, each error as a tag), so a
//! target that rounds one step differently, splits a different panel, or stops one evaluation
//! sooner changes the digest. The digest lives in the store; a change is a bug, or a new
//! definition with a golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic)]

use geom_num::NumError;
use geom_num::brent::{BrentError, minimum, zero};
use geom_num::math;
use geom_num::quadrature::{QuadError, gauss_legendre, integrate};
use geom_num::quadrature_rules::{GAUSS_KRONROD, GAUSS_LEGENDRE};
use geom_num::real::Real;
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-num/roots-and-quadrature";

fn r(x: f64) -> Real {
    Real::new(x).unwrap()
}

#[derive(Default)]
struct Recorder {
    bytes: Vec<u8>,
    calls: u32,
}

impl Recorder {
    fn f64(&mut self, v: f64) {
        self.bytes.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    /// Every point a search or an integration evaluated, in the order it evaluated them: what the
    /// caller's function sees, so a change in any step shows here even when the answer is the same.
    fn points(&mut self, seen: &[f64]) {
        self.u32(u32::try_from(seen.len()).unwrap());
        for v in seen {
            self.f64(*v);
        }
    }
    fn tag(&mut self, t: u8) {
        self.bytes.push(t);
        self.calls += 1;
    }
    fn num(&mut self, e: NumError) {
        // The error's name: stable, readable, and distinct for each kind.
        self.bytes.extend_from_slice(format!("{e:?}").as_bytes());
    }
    fn brent_error(&mut self, e: BrentError) {
        match e {
            BrentError::SameSign => self.tag(10),
            BrentError::BadInterval => self.tag(11),
            BrentError::CapReached => self.tag(12),
            BrentError::Num(n) => {
                self.tag(13);
                self.num(n);
            }
        }
    }
    fn quad_error(&mut self, e: QuadError) {
        match e {
            QuadError::BadInterval => self.tag(20),
            QuadError::NoSuchRule => self.tag(21),
            QuadError::CapReached => self.tag(22),
            QuadError::TooNarrow => self.tag(23),
            QuadError::Num(n) => {
                self.tag(24);
                self.num(n);
            }
        }
    }
}

/// An integrand.
type Integrand = fn(Real) -> Result<Real, NumError>;

/// A function family with a zero at its second argument.
type Family = fn(f64, f64) -> f64;

fn integrands() -> [(Integrand, f64, f64); 5] {
    [
        (|x| math::exp(x.get()), 0.0, 1.0),
        (
            |x| Real::new(1.0 / (1.0 + 25.0 * x.get() * x.get())),
            -1.0,
            1.0,
        ),
        (|x| math::sinpi(x.get()), 0.0, 1.0),
        (|x| math::sqrt(x.get()), 0.0, 2.0),
        // Large values, so a relative tolerance differs from the same number taken absolutely.
        (
            |x| Real::new(1000.0 * x.get() * x.get() * x.get()),
            0.0,
            3.0,
        ),
    ]
}

fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    for rule in GAUSS_LEGENDRE {
        rec.tag(1);
        rule.nodes
            .iter()
            .chain(rule.weights)
            .for_each(|v| rec.f64(*v));
    }
    for rule in GAUSS_KRONROD {
        rec.tag(2);
        rule.nodes
            .iter()
            .chain(rule.kronrod)
            .chain(rule.gauss)
            .for_each(|v| rec.f64(*v));
    }
    let families: [Family; 4] = [
        |x, r| x - r,
        |x, r| (x - r) * (x * x + 1.0),
        |x, r| (x - r) * (x - r) * (x - r),
        |x, r| (1e8 * (x - r)).clamp(-1.0, 1.0),
    ];
    for f in families {
        for i in 0..50 {
            let root = -10.0 + 0.4123 * f64::from(i);
            for (left, right) in [(3.7, 1.3), (0.01, 50.0), (1e-3, 1e-3)] {
                for tol in [0.0, 1e-6] {
                    let mut seen = Vec::new();
                    let g = |x: Real| {
                        seen.push(x.get());
                        Real::new(f(x.get(), root))
                    };
                    let found = zero(g, r(root - left), r(root + right), r(tol), 3000);
                    rec.points(&seen);
                    match found {
                        Ok(z) => {
                            rec.tag(3);
                            rec.f64(z.x.get());
                            rec.f64(z.fx.get());
                            rec.f64(z.other.get());
                            rec.u32(z.evaluations);
                        }
                        Err(e) => rec.brent_error(e),
                    }
                }
            }
        }
    }
    // Minima of parabolas, cubics, kinks, quartics, staircases and an upside-down parabola, inside
    // the interval and at its ends.
    let shapes: [Family; 6] = [
        |x, m| (x - m) * (x - m) + 1.0,
        |x, m| (x - m) * (x - m) * (x - m),
        |x, m| (x - m).abs(),
        |x, m| (x - m) * (x - m) * (x - m) * (x - m),
        |x, m| ((x - m).abs() * 8.0).floor(),
        |x, m| -((x - m) * (x - m)),
    ];
    for shape in shapes {
        for m in [0.0, 0.25, 1.0 / 3.0, 0.5, 0.7, 0.999, -0.4, 1.4] {
            for tol in [0.0, 1e-9] {
                let mut seen = Vec::new();
                let f = |x: Real| {
                    seen.push(x.get());
                    Real::new(shape(x.get(), m))
                };
                let found = minimum(f, r(-0.5), r(1.0), r(tol), 200);
                rec.points(&seen);
                match found {
                    Ok(found) => {
                        rec.tag(4);
                        rec.f64(found.x.get());
                        rec.f64(found.fx.get());
                        rec.u32(found.evaluations);
                    }
                    Err(e) => rec.brent_error(e),
                }
            }
        }
    }
    for (f, a, b) in integrands() {
        for n in 0..=21 {
            match gauss_legendre(f, r(a), r(b), n) {
                Ok(v) => {
                    rec.tag(5);
                    rec.f64(v.get());
                }
                Err(e) => rec.quad_error(e),
            }
        }
        for rule in 0..=GAUSS_KRONROD.len() {
            for (tol, rel) in [(1e-6, 0.0), (1e-13, 0.0), (0.0, 1e-10), (0.0, 0.0)] {
                let mut seen = Vec::new();
                let g = |x: Real| {
                    seen.push(x.get());
                    f(x)
                };
                let found = integrate(g, r(a), r(b), r(tol), r(rel), rule, 100);
                rec.points(&seen);
                match found {
                    Ok(i) => {
                        rec.tag(6);
                        rec.f64(i.value.get());
                        rec.f64(i.error.get());
                        rec.u32(i.panels);
                        rec.u32(i.evaluations);
                    }
                    Err(e) => rec.quad_error(e),
                }
            }
        }
    }
    rec
}

/// The golden hash. The same digest must hold on every target; a change is a bug (see the module
/// documentation).
#[test]
fn roots_and_quadrature_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-num roots and quadrature corpus: {} results, {} bytes, golden hash {}",
        rec.calls,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert!(rec.calls >= 1_440, "only {} results", rec.calls);
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different point, panel or count \
             (compare this log's digest with another target's), or a definition changed in place, \
             which chapter 26.2 forbids"
        )
    });
}
