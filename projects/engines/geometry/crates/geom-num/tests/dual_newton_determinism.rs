//! The replay hash of piece G52: the dual numbers' derivative rules over reals and intervals, the
//! Jacobians of fixed systems, and the Krawczyk and Hansen-Sengupta operators run over a fixed
//! corpus and hashed with SHA-256, must give the same digest on every target (chapters 21 and
//! 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed seed: every elemental operation of the dual numbers on 300 pairs of
//! dual reals and 300 pairs of dual intervals, values, partials and errors alike (including the
//! tripwire's refusals at chosen singular points and the cut of `atan2`); the values and Jacobians
//! of four fixed systems at 50 points and over 50 boxes; and both operators, five steps each from
//! 60 boxes around the zero of each of 60 seeded systems in each of one to four unknowns, with
//! singular, empty and failing systems besides. Each result is written in a canonical byte
//! encoding (doubles as their bits, each verdict and error as a tag), so a target that rounds one
//! step differently, pivots on another row or stops one step sooner changes the digest. The digest
//! lives in the store; a change is a bug, or a new definition with a golden of its own (chapter
//! 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::needless_range_loop,
    clippy::too_many_lines,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_num::NumError;
use geom_num::dual::{Dual, jacobian};
use geom_num::interval::Interval;
use geom_num::newton::{NewtonError, Residuals, Verdict, hansen_sengupta, krawczyk};
use geom_num::real::Real;
use geom_num::scalar::Scalar;
use geom_test::SplitMix64;
use geom_test::golden::{self, hex, sha256};
use geom_test::seed::Seed;

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-num/dual-and-newton";

/// The corpus's fixed seed. Never `SPAREKIT_SEED`: a golden's inputs are part of its definition.
const SEED: u64 = 0x4752_6475_616c_4e77;

fn r(x: f64) -> Real {
    Real::new(x).unwrap()
}

fn c<S: Scalar>(v: f64) -> S {
    S::constant(r(v))
}

fn uniform(rng: &mut SplitMix64, lo: f64, hi: f64) -> f64 {
    let u = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
    lo + (hi - lo) * u
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
    fn tag(&mut self, t: u8) {
        self.bytes.push(t);
        self.calls += 1;
    }
    fn num(&mut self, e: NumError) {
        self.bytes.extend_from_slice(format!("{e:?}").as_bytes());
    }
    fn interval(&mut self, i: Interval) {
        self.f64(i.lo());
        self.f64(i.hi());
    }
    fn real_dual<const N: usize>(&mut self, d: Result<Dual<Real, N>, NumError>) {
        match d {
            Ok(d) => {
                self.tag(1);
                self.f64(d.value().get());
                for p in d.partials() {
                    self.f64(p.get());
                }
            }
            Err(e) => {
                self.tag(2);
                self.num(e);
            }
        }
    }
    fn interval_dual<const N: usize>(&mut self, d: Result<Dual<Interval, N>, NumError>) {
        match d {
            Ok(d) => {
                self.tag(3);
                self.interval(d.value());
                for p in d.partials() {
                    self.interval(p);
                }
            }
            Err(e) => {
                self.tag(4);
                self.num(e);
            }
        }
    }
    fn verdict<const N: usize>(
        &mut self,
        v: Result<Verdict<N>, NewtonError>,
    ) -> Option<[Interval; N]> {
        match v {
            Ok(Verdict::NoZero) => {
                self.tag(10);
                None
            }
            Ok(Verdict::Unique {
                enclosure,
                contraction,
            }) => {
                self.tag(11);
                for i in enclosure {
                    self.interval(i);
                }
                self.f64(contraction.get());
                Some(enclosure)
            }
            Ok(Verdict::Narrowed {
                enclosure,
                contraction,
            }) => {
                self.tag(12);
                for i in enclosure {
                    self.interval(i);
                }
                self.f64(contraction.get());
                Some(enclosure)
            }
            Err(NewtonError::Singular) => {
                self.tag(13);
                None
            }
            Err(NewtonError::Num(e)) => {
                self.tag(14);
                self.num(e);
                None
            }
        }
    }
}

/// Every elemental operation on one pair, in a fixed order.
fn elementals<S: Scalar>(a: S, b: S) -> [Result<S, NumError>; 13] {
    [
        a.add(b),
        a.sub(b),
        a.mul(b),
        a.div(b),
        a.mul_add(b, a),
        a.square(),
        a.sqrt(),
        a.exp(),
        a.ln(),
        a.sinpi(),
        a.cospi(),
        a.atan2(b),
        Ok(a.neg()),
    ]
}

/// cospi(x) + y = 0 and x − 2y = 1/2.
struct CosLine;
impl Residuals<2> for CosLine {
    fn residuals<S: Scalar>(&self, [x, y]: [S; 2]) -> Result<[S; 2], NumError> {
        Ok([x.cospi()?.add(y)?, x.sub(y.add(y)?)?.sub(c(0.5))?])
    }
}

/// exp(a) = b, ln(b) + a·c = 0, atan2(c, d) = π/4 and √(c + d + 2) = 2.
struct Four;
impl Residuals<4> for Four {
    fn residuals<S: Scalar>(&self, [a, b, cc, d]: [S; 4]) -> Result<[S; 4], NumError> {
        Ok([
            a.exp()?.sub(b)?,
            a.mul_add(cc, b.ln()?)?,
            cc.atan2(d)?.sub(S::pi().div(c(4.0))?)?,
            cc.add(d)?.add(c(2.0))?.sqrt()?.sub(c(2.0))?,
        ])
    }
}

/// x² = 0: a double root.
struct Double;
impl Residuals<1> for Double {
    fn residuals<S: Scalar>(&self, [x]: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x.square()?])
    }
}

/// Fᵢ(x) = Σⱼ aᵢⱼ (xⱼ − rⱼ) + qᵢ (xᵢ − rᵢ)², with zero `r`.
struct Random<const N: usize> {
    a: [[f64; N]; N],
    q: [f64; N],
    r: [f64; N],
}

impl<const N: usize> Residuals<N> for Random<N> {
    fn residuals<S: Scalar>(&self, x: [S; N]) -> Result<[S; N], NumError> {
        let mut out = [c::<S>(0.0); N];
        for i in 0..N {
            let di = x[i].sub(c(self.r[i]))?;
            let mut acc = c::<S>(self.q[i]).mul(di.square()?)?;
            for j in 0..N {
                acc = c::<S>(self.a[i][j]).mul_add(x[j].sub(c(self.r[j]))?, acc)?;
            }
            out[i] = acc;
        }
        Ok(out)
    }
}

fn random<const N: usize>(rng: &mut SplitMix64) -> Random<N> {
    let mut a = [[0.0; N]; N];
    for row in &mut a {
        for v in row.iter_mut() {
            // Entries of either sign and of mixed sizes, with ties of equal magnitude in a
            // column now and then, so the pivoting's choices and tie rule all show.
            *v = match rng.next_u64() % 4 {
                0 => 1.0,
                1 => -1.0,
                _ => uniform(rng, -3.0, 3.0),
            };
        }
    }
    let q = [(); N].map(|()| uniform(rng, -1.0, 1.0));
    let r = [(); N].map(|()| uniform(rng, -10.0, 10.0));
    Random { a, q, r }
}

fn operators<const N: usize>(rec: &mut Recorder, rng: &mut SplitMix64) {
    for _ in 0..60 {
        let f = random::<N>(rng);
        for op in [krawczyk::<Random<N>, N>, hansen_sengupta::<Random<N>, N>] {
            let mut b = f.r.map(|v| {
                let (lo, hi) = (uniform(rng, 0.0, 0.6), uniform(rng, 0.0, 0.6));
                Interval::new(v - lo, v + hi).unwrap()
            });
            for _ in 0..5 {
                match rec.verdict(op(&f, b)) {
                    Some(next) => b = next,
                    None => break,
                }
            }
        }
    }
}

fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    let mut rng = SplitMix64::new(Seed(SEED));

    // Dual reals: every elemental at seeded pairs, then at chosen singular points.
    for _ in 0..300 {
        let x = [uniform(&mut rng, -4.0, 4.0), uniform(&mut rng, -4.0, 4.0)].map(r);
        let [a, b] = Dual::<Real, 2>::variables(x);
        for d in elementals(a, b) {
            rec.real_dual(d);
        }
    }
    let x1 = |v: f64| Dual::<Real, 1>::variables([r(v)])[0];
    for v in [0.0, f64::from_bits(1), -1.0, 800.0, 1e-300] {
        for d in elementals(x1(v), x1(0.0)) {
            rec.real_dual(d);
        }
    }

    // Dual intervals: every elemental over seeded boxes, some across zero and the cut.
    for _ in 0..300 {
        let mut pick = || {
            let (u, v) = (uniform(&mut rng, -3.0, 3.0), uniform(&mut rng, -3.0, 3.0));
            if u < v {
                Interval::new(u, v).unwrap()
            } else {
                Interval::new(v, u).unwrap()
            }
        };
        let x = [pick(), pick()];
        let [a, b] = Dual::<Interval, 2>::variables(x);
        for d in elementals(a, b) {
            rec.interval_dual(d);
        }
    }

    // Systems' values and Jacobians at points and over boxes.
    for _ in 0..50 {
        let p = [uniform(&mut rng, 0.3, 0.7), uniform(&mut rng, -0.2, 0.2)];
        let (v, j) = jacobian(|v| CosLine.residuals(v), p.map(r)).unwrap();
        v.iter()
            .chain(j.iter().flatten())
            .for_each(|x| rec.f64(x.get()));
        let w = uniform(&mut rng, 0.0, 0.2);
        let (v, j) = jacobian(
            |v| CosLine.residuals(v),
            p.map(|x| Interval::new(x, x + w).unwrap()),
        )
        .unwrap();
        v.iter()
            .chain(j.iter().flatten())
            .for_each(|x| rec.interval(*x));
        let p4 = [
            uniform(&mut rng, -0.2, 0.2),
            uniform(&mut rng, 0.8, 1.2),
            uniform(&mut rng, 0.8, 1.2),
            uniform(&mut rng, 0.8, 1.2),
        ];
        let (v, j) = jacobian(|v| Four.residuals(v), p4.map(r)).unwrap();
        v.iter()
            .chain(j.iter().flatten())
            .for_each(|x| rec.f64(x.get()));
        let (v, j) = jacobian(
            |v| Four.residuals(v),
            p4.map(|x| Interval::new(x, x + w).unwrap()),
        )
        .unwrap();
        v.iter()
            .chain(j.iter().flatten())
            .for_each(|x| rec.interval(*x));
        rec.tag(5);
    }

    // The operators on seeded systems, and on singular, empty and failing ones.
    operators::<1>(&mut rec, &mut rng);
    operators::<2>(&mut rec, &mut rng);
    operators::<3>(&mut rec, &mut rng);
    operators::<4>(&mut rec, &mut rng);
    for b in [[(-1.0, 1.0)], [(-0.5, 1.0)], [(-1e-3, 3e-3)], [(1.0, 2.0)]] {
        let b = b.map(|(lo, hi)| Interval::new(lo, hi).unwrap());
        rec.verdict(krawczyk(&Double, b));
        rec.verdict(hansen_sengupta(&Double, b));
    }
    for b in [
        [(0.4, 0.6), (-0.1, 0.1)],
        [(0.0, 1.0), (-1.0, 1.0)],
        [(2.0, 3.0), (5.0, 6.0)],
        [(-1e300, 1e300), (0.0, 1.0)],
    ] {
        let b = b.map(|(lo, hi)| Interval::new(lo, hi).unwrap());
        rec.verdict(krawczyk(&CosLine, b));
        rec.verdict(hansen_sengupta(&CosLine, b));
    }
    for b in [
        [(-0.1, 0.1), (0.9, 1.1), (0.9, 1.1), (0.9, 1.1)],
        [(-0.1, 0.1), (-0.5, 1.1), (0.9, 1.1), (0.9, 1.1)],
        [(-0.1, 0.1), (0.9, 1.1), (-1.0, 1.0), (-1.0, -0.5)],
    ] {
        let b = b.map(|(lo, hi)| Interval::new(lo, hi).unwrap());
        rec.verdict(krawczyk(&Four, b));
        rec.verdict(hansen_sengupta(&Four, b));
    }
    rec
}

/// The golden hash. The same digest must hold on every target; a change is a bug (see the module
/// documentation).
#[test]
fn dual_and_newton_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-num dual and newton corpus: {} results, {} bytes, golden hash {}",
        rec.calls,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert!(rec.calls >= 10_000, "only {} results", rec.calls);
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different derivative, \
             enclosure or verdict (compare this log's digest with another target's), or a \
             definition changed in place, which chapter 26.2 forbids"
        )
    });
}
