//! The replay hash of piece G53: the double-double balls and the root-isolation engine run over a
//! fixed corpus and hashed with SHA-256, must give the same digest on every target (chapters 21
//! and 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed seed: every operation of the balls on 400 seeded pairs, results and
//! errors alike; and the engine on seeded polynomial systems in one to four unknowns (products of
//! integer affine forms, as `tests/attack_g53.rs` makes them, by both operators), on the edge
//! cases of `attack_g53` (zeros on split lines and on the domain's boundary, zeros closer than the
//! floor, multiple zeros, a dependent system, a zero without a derivative, the double-double
//! fallbacks), and on its caps (the work budget, an unreachable target, an imprecise position, a
//! box that cannot be split). Each result is written in a canonical byte encoding: every bound as
//! its bits, each precision, reason and error as a tag, and the work each isolation charged, so a
//! target that rounds one step differently, splits in another order or charges one unit more
//! changes the digest. The digest lives in the store; a change is a bug, or a new definition with a
//! golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::too_many_lines,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_num::NumError;
use geom_num::ball::Ball;
use geom_num::interval::Interval;
use geom_num::isolate::{Options, Reason, System, isolate};
use geom_num::newton::{Operator, Precision, Residuals};
use geom_num::real::Real;
use geom_num::scalar::Scalar;
use geom_run::{Budget, WorkMeter};
use geom_test::SplitMix64;
use geom_test::golden::{self, hex, sha256};
use geom_test::seed::Seed;

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-num/root-isolation";

/// The corpus's fixed seed. Never `SPAREKIT_SEED`: a golden's inputs are part of its definition.
const SEED: u64 = 0x4753_3352_6f6f_7473;

fn r(x: f64) -> Real {
    Real::new(x).unwrap()
}

fn iv(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

fn c<S: Scalar>(v: f64) -> S {
    S::constant(r(v))
}

/// The integer dot product of `a` and `p`, in order.
fn dot<const N: usize>(a: &[i64; N], p: &[i64; N]) -> i64 {
    let mut s = 0;
    for (x, y) in a.iter().zip(p) {
        s += x * y;
    }
    s
}

/// 2^k exactly, for k in the normal range, from its bits (no platform maths).
fn pow2(k: i32) -> f64 {
    f64::from_bits(((1023 + k) as u64) << 52)
}

fn between(rng: &mut SplitMix64, lo: i64, hi: i64) -> i64 {
    lo + (rng.next_u64() % (hi - lo + 1) as u64) as i64
}

#[derive(Default)]
struct Recorder {
    bytes: Vec<u8>,
    results: u32,
}

impl Recorder {
    fn f64(&mut self, v: f64) {
        self.bytes.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    fn tag(&mut self, t: u8) {
        self.bytes.push(t);
        self.results += 1;
    }
    fn num(&mut self, e: NumError) {
        self.bytes.extend_from_slice(format!("{e:?}").as_bytes());
    }
    fn interval(&mut self, i: Interval) {
        self.f64(i.lo());
        self.f64(i.hi());
    }
    fn ball(&mut self, b: Result<Ball, NumError>) {
        match b {
            Ok(b) => {
                self.tag(1);
                self.f64(b.mid().hi());
                self.f64(b.mid().lo());
                self.f64(b.rad());
            }
            Err(e) => {
                self.tag(2);
                self.num(e);
            }
        }
    }
    fn precision(&mut self, p: Precision) {
        self.bytes.push(match p {
            Precision::Double => 1,
            Precision::DoubleDouble => 2,
        });
    }
    fn isolation<const N: usize, Sys: System<N, D>, const D: usize>(
        &mut self,
        system: &Sys,
        domain: [(f64, f64); N],
        options: Options,
        budget: Budget,
    ) {
        let mut meter = WorkMeter::new(budget);
        let found = isolate(
            system,
            domain.map(|(lo, hi)| iv(lo, hi)),
            options,
            &mut meter,
        );
        self.bytes
            .extend_from_slice(&meter.used().work.to_le_bytes());
        self.bytes
            .extend_from_slice(&meter.used().logical_bytes.to_le_bytes());
        match found {
            Ok(found) => {
                self.tag(10);
                for z in &found.zeros {
                    self.tag(11);
                    z.enclosure.iter().for_each(|i| self.interval(*i));
                    z.proof.iter().for_each(|i| self.interval(*i));
                    self.precision(z.precision);
                }
                for f in &found.flagged {
                    self.tag(12);
                    f.region.iter().for_each(|i| self.interval(*i));
                    self.bytes.push(match f.reason {
                        Reason::Unproven => 1,
                        Reason::Singular => 2,
                        Reason::NotSmooth => 3,
                    });
                }
            }
            Err(e) => {
                self.tag(20);
                self.bytes.extend_from_slice(format!("{e:?}").as_bytes());
            }
        }
    }
}

/// A system whose equation `i` is the product of affine forms `a·x − b` with integer coefficients,
/// its position the unknowns times 64 grid steps.
struct Products<const N: usize> {
    forms: Vec<Vec<([i64; N], i64)>>,
}

impl<const N: usize> Residuals<N> for Products<N> {
    fn residuals<S: Scalar>(&self, x: [S; N]) -> Result<[S; N], NumError> {
        let mut out = [c::<S>(0.0); N];
        for (o, forms) in out.iter_mut().zip(&self.forms) {
            let mut acc = c::<S>(1.0);
            for (a, b) in forms {
                let mut form = c::<S>(-(*b as f64));
                for (aj, xj) in a.iter().zip(x) {
                    form = c::<S>(*aj as f64).mul_add(xj, form)?;
                }
                acc = acc.mul(form)?;
            }
            *o = acc;
        }
        Ok(out)
    }
}

impl<const N: usize> System<N, N> for Products<N> {
    fn position<S: Scalar>(&self, x: [S; N]) -> Result<[S; N], NumError> {
        let mut out = x;
        for o in &mut out {
            *o = c::<S>(64.0).mul(*o)?;
        }
        Ok(out)
    }
}

/// `count` seeded product systems in `N` unknowns, each isolated by both operators.
fn products<const N: usize>(rec: &mut Recorder, rng: &mut SplitMix64, count: usize) {
    let degrees: &[usize] = match N {
        1 => &[3],
        2 => &[2, 2],
        3 => &[2, 2, 1],
        _ => &[2, 1, 1, 1],
    };
    for _ in 0..count {
        let forms = degrees
            .iter()
            .map(|&d| {
                (0..d)
                    .map(|_| {
                        let mut a = [0i64; N];
                        while a.iter().all(|v| *v == 0) {
                            for v in &mut a {
                                *v = between(rng, -3, 3);
                            }
                        }
                        let p: [i64; N] = core::array::from_fn(|_| between(rng, -6, 6));
                        let b = dot(&a, &p) + between(rng, -2, 2);
                        (a, b)
                    })
                    .collect()
            })
            .collect();
        let system = Products { forms };
        for operator in [Operator::Krawczyk, Operator::HansenSengupta] {
            let options = Options {
                operator,
                ..Options::DEFAULT
            };
            rec.isolation(&system, [(-8.0, 8.0); N], options, Budget::of_work(200_000));
        }
    }
}

/// One unknown: the product of `x − rᵢ`, the position `x` times `scale`.
struct Roots {
    roots: Vec<f64>,
    scale: f64,
}

impl Residuals<1> for Roots {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        let mut acc = c::<S>(1.0);
        for root in &self.roots {
            acc = acc.mul(x[0].sub(c(*root))?)?;
        }
        Ok([acc])
    }
}

impl System<1, 1> for Roots {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([c::<S>(self.scale).mul(x[0])?])
    }
}

/// `(x − c)²`, a double zero just above 1, with the position `(x − 1) · 2⁶⁰`: a box one double wide
/// near it spans 256 grid steps.
struct Stretched;

impl Residuals<1> for Stretched {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x[0].sub(c(1.0 + 2.0 * f64::EPSILON))?.square()?])
    }
}

impl System<1, 1> for Stretched {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x[0].sub(c(1.0))?.mul(c(pow2(60)))?])
    }
}

/// `((x + shift) − shift) − at`, with the position `(x + pshift) − pshift`.
struct Cancelling {
    shift: f64,
    pshift: f64,
    at: f64,
}

impl Residuals<1> for Cancelling {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x[0]
            .add(c(self.shift))?
            .sub(c(self.shift))?
            .sub(c(self.at))?])
    }
}

impl System<1, 1> for Cancelling {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x[0].add(c(self.pshift))?.sub(c(self.pshift))?])
    }
}

/// `x + y = 0, 2x + 2y = 0`, and `√(x²) = 0` with `√(y² + 1) − 2 = 0`: a dependent system and
/// residuals without a derivative at a zero, or whose balls fail.
struct Odd {
    dependent: bool,
}

impl Residuals<2> for Odd {
    fn residuals<S: Scalar>(&self, x: [S; 2]) -> Result<[S; 2], NumError> {
        if self.dependent {
            let s = x[0].add(x[1])?;
            Ok([s, c::<S>(2.0).mul(s)?])
        } else {
            Ok([
                x[0].square()?.sqrt()?,
                x[1].square()?.add(c(1.0))?.sqrt()?.sub(c(2.0))?,
            ])
        }
    }
}

impl System<2, 2> for Odd {
    fn position<S: Scalar>(&self, x: [S; 2]) -> Result<[S; 2], NumError> {
        let big = pow2(60);
        Ok([c::<S>(16.0).mul(x[0])?, x[1].add(c(big))?.sub(c(big))?])
    }
}

/// The corpus, recorded in its canonical encoding.
fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    let mut rng = SplitMix64::new(Seed(SEED));
    for _ in 0..400 {
        let mut ball = || {
            let scale = pow2(between(&mut rng, -20, 20) as i32);
            let lo = between(&mut rng, -1000, 1000) as f64 / 7.0 * scale;
            let width = between(&mut rng, 0, 3) as f64 * scale / 10.0;
            Ball::from_interval(iv(lo, lo + width)).unwrap()
        };
        let (a, b) = (ball(), ball());
        rec.ball(a.add(b));
        rec.ball(a.sub(b));
        rec.ball(a.mul(b));
        rec.ball(a.div(b));
        rec.ball(a.mul_add(b, a));
        rec.ball(Ok(a.neg()));
        rec.ball(a.square());
        rec.ball(a.sqrt());
        rec.ball(a.exp());
        rec.ball(a.ln());
        rec.ball(a.sinpi());
        rec.ball(a.cospi());
        rec.ball(a.atan2(b));
        match a.to_interval() {
            Ok(i) => {
                rec.tag(3);
                rec.interval(i);
            }
            Err(e) => {
                rec.tag(4);
                rec.num(e);
            }
        }
    }
    rec.ball(Ok(Ball::pi()));
    products::<1>(&mut rec, &mut rng, 24);
    products::<2>(&mut rec, &mut rng, 12);
    products::<3>(&mut rec, &mut rng, 6);
    products::<4>(&mut rec, &mut rng, 4);
    let unlimited = Budget::UNLIMITED;
    let default = Options::DEFAULT;
    let hs = Options {
        operator: Operator::HansenSengupta,
        ..default
    };
    for roots in [
        vec![-1.0, -0.5, 0.0, 0.5, 1.0],
        vec![0.5, 0.5],
        vec![1.0, 1.0, 1.0],
        vec![0.0],
    ] {
        for options in [default, hs] {
            rec.isolation(
                &Roots {
                    roots: roots.clone(),
                    scale: 64.0,
                },
                [(-2.0, 2.0)],
                options,
                unlimited,
            );
            rec.isolation(
                &Roots {
                    roots: roots.clone(),
                    scale: 64.0,
                },
                [(0.0, 1.0)],
                options,
                unlimited,
            );
        }
    }
    let near = Roots {
        roots: vec![0.25, 0.5],
        scale: 1.0,
    };
    rec.isolation(&near, [(-3.0, 3.0)], default, unlimited);
    let fine = Options {
        target: r(1e-9),
        ..default
    };
    rec.isolation(
        &Roots {
            roots: vec![1.5, 3.25],
            scale: 1.0,
        },
        [(-8.0, 8.0)],
        fine,
        unlimited,
    );
    let big = pow2(60);
    for options in [default, hs] {
        rec.isolation(
            &Cancelling {
                shift: big,
                pshift: big,
                at: 1000.25,
            },
            [(0.0, 2048.0)],
            options,
            unlimited,
        );
        rec.isolation(
            &Cancelling {
                shift: big,
                pshift: 0.0,
                at: 1024.0,
            },
            [(0.0, 2048.0)],
            options,
            unlimited,
        );
    }
    rec.isolation(
        &Cancelling {
            shift: pow2(30),
            pshift: 0.0,
            at: 0.125,
        },
        [(-4.0, 4.0)],
        fine,
        unlimited,
    );
    rec.isolation(
        &Odd { dependent: true },
        [(-1.0, 1.0), (-1.0, 1.0)],
        default,
        unlimited,
    );
    rec.isolation(
        &Odd { dependent: false },
        [(-1.3, 1.1), (-4.0, 4.0)],
        default,
        Budget::of_work(100_000),
    );
    // The caps.
    rec.isolation(
        &Roots {
            roots: vec![-0.75, 0.3, 1.2],
            scale: 64.0,
        },
        [(-2.0, 2.0)],
        default,
        Budget::of_work(5),
    );
    rec.isolation(
        &Roots {
            roots: vec![-0.75, 0.3, 1.2],
            scale: 64.0,
        },
        [(-2.0, 2.0)],
        default,
        Budget::new(u64::MAX, 100),
    );
    rec.isolation(
        &Roots {
            roots: vec![1.5],
            scale: 1.0,
        },
        [(-4.0, 4.0)],
        Options {
            target: r(1e-300),
            ..default
        },
        unlimited,
    );
    rec.isolation(
        &Cancelling {
            shift: 0.0,
            pshift: pow2(200),
            at: 1.5,
        },
        [(0.0, 4.0)],
        default,
        unlimited,
    );
    rec.isolation(
        &Stretched,
        [(1.0, 1.0 + 4.0 * f64::EPSILON)],
        default,
        unlimited,
    );
    rec
}

/// The golden hash. The same digest must hold on every target; a change is a bug (see the module
/// documentation).
#[test]
fn root_isolation_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-num root-isolation corpus: {} results, {} bytes, golden hash {}",
        rec.results,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert!(rec.results >= 5_000, "only {} results", rec.results);
    // The corpus reaches each cap's error.
    let text = String::from_utf8_lossy(&rec.bytes);
    for words in ["Exceeded", "CapReached", "Imprecise", "Unsplittable"] {
        assert!(text.contains(words), "the corpus reaches no {words}");
    }
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different enclosure, splits \
             or charges differently (compare this log's digest with another target's), or a \
             definition changed in place, which chapter 26.2 forbids"
        )
    });
}
