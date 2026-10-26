//! The replay hash of piece G58: every fitting operation, run over a fixed corpus and hashed with
//! SHA-256, must give the same digest on every target (chapters 21 and 28.13; the golden store
//! of piece G19), so a fit made on one machine is the fit every machine makes.
//!
//! The corpus, from a fixed linear congruential sequence: curve interpolation of 2 to 40 points
//! at degrees 1 to 5 by all three parameter rules; interpolation with end derivatives;
//! least-squares curves; weighted least squares with point and derivative constraints; surface
//! interpolation and least squares on grids up to 9 × 9; approximation to a stated error of a
//! helix, a corner and noisy data in both knot modes; and a refusal of each kind. Each result is
//! written in a canonical byte encoding (doubles as their bits, each error as its name), so a
//! target that rounds one step differently, or takes one more Newton step or knot removal,
//! changes the digest. The digest lives in the store; a change is a bug, or a new
//! [`FitVersion`] with a golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_nurbs::{
    Constraint, Curve, CurveFit, FitVersion, KnotMode, NurbsError, Parametrization, Surface,
    SurfaceFit, least_squares_knots, parameters,
};
use geom_run::{Budget, WorkMeter};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-nurbs/fit";

const V: FitVersion = FitVersion::V1;
const RULES: [Parametrization; 3] = [
    Parametrization::Uniform,
    Parametrization::ChordLength,
    Parametrization::Centripetal,
];

/// sin(πx), correctly rounded on every target.
fn sinpi(x: f64) -> f64 {
    geom_num::math::sinpi(x).unwrap().get()
}

/// cos(πx), correctly rounded on every target.
fn cospi(x: f64) -> f64 {
    geom_num::math::cospi(x).unwrap().get()
}

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
    fn below(&mut self, n: usize) -> usize {
        ((self.next() >> 33) % n as u64) as usize
    }
}

#[derive(Default)]
struct Recorder {
    bytes: Vec<u8>,
    calls: u32,
    refusals: u32,
}

impl Recorder {
    fn f64(&mut self, v: f64) {
        self.bytes.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    fn error(&mut self, e: NurbsError) {
        self.refusals += 1;
        self.bytes.push(0);
        self.bytes.extend_from_slice(format!("{e:?}").as_bytes());
    }
    fn points<const D: usize>(&mut self, pts: &[[f64; D]]) {
        for p in pts {
            for &x in p {
                self.f64(x);
            }
        }
    }
    fn curve(&mut self, r: Result<CurveFit<3>, NurbsError>) {
        self.calls += 1;
        match r {
            Ok(fit) => {
                self.bytes.push(1);
                self.bytes.push(fit.curve.degree() as u8);
                for &k in fit.curve.basis().knots() {
                    self.f64(k);
                }
                self.points(fit.curve.homogeneous());
                for &u in &fit.parameters {
                    self.f64(u);
                }
                self.f64(fit.error);
                self.f64(fit.constraint_error);
            }
            Err(e) => self.error(e),
        }
    }
    fn curve2(&mut self, r: Result<CurveFit<2>, NurbsError>) {
        self.calls += 1;
        match r {
            Ok(fit) => {
                self.bytes.push(1);
                for &k in fit.curve.basis().knots() {
                    self.f64(k);
                }
                self.points(fit.curve.homogeneous());
                for &u in &fit.parameters {
                    self.f64(u);
                }
                self.f64(fit.error);
            }
            Err(e) => self.error(e),
        }
    }
    fn surface(&mut self, r: Result<SurfaceFit<3>, NurbsError>) {
        self.calls += 1;
        match r {
            Ok(fit) => {
                self.bytes.push(1);
                for &k in fit
                    .surface
                    .basis_u()
                    .knots()
                    .iter()
                    .chain(fit.surface.basis_v().knots())
                {
                    self.f64(k);
                }
                self.points(fit.surface.homogeneous());
                for &u in fit.u.iter().chain(&fit.v) {
                    self.f64(u);
                }
                self.f64(fit.error);
            }
            Err(e) => self.error(e),
        }
    }
}

fn polygon(rng: &mut Lcg, count: usize) -> Vec<[f64; 3]> {
    let mut p = [rng.unit(), rng.unit(), rng.unit()];
    let mut out = Vec::new();
    for _ in 0..count {
        out.push(p);
        for x in &mut p {
            *x += 0.2 + rng.unit();
        }
        p[1] -= 0.7 * rng.unit();
    }
    out
}

fn grid(rng: &mut Lcg, rows: usize, cols: usize) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    for i in 0..rows {
        for j in 0..cols {
            let (x, y) = (
                i as f64 + 0.3 * rng.unit(),
                j as f64 * 1.5 + 0.3 * rng.unit(),
            );
            out.push([x, y, x * 0.1 * y - 0.05 * x * x + 0.05 * rng.unit()]);
        }
    }
    out
}

fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    let mut rng = Lcg(0x4735_3846_4954_0001);
    for t in 0..64 {
        let count = 2 + rng.below(39);
        let data = polygon(&mut rng, count);
        let p = 1 + rng.below(5.min(count - 1));
        rec.curve(Curve::interpolate(&data, p, RULES[t % 3], V, &mut meter()));
    }
    for t in 0..16 {
        let count = 2 + rng.below(20);
        let data = polygon(&mut rng, count);
        let ends = [[1.0, rng.unit(), 0.5], [rng.unit(), -1.0, 2.0]];
        let p = 2 + rng.below(3.min(count));
        rec.curve(Curve::interpolate_with_ends(
            &data,
            ends,
            p,
            RULES[t % 3],
            V,
            &mut meter(),
        ));
    }
    for t in 0..32 {
        let count = 4 + rng.below(40);
        let data = polygon(&mut rng, count);
        let p = 1 + rng.below(3);
        let n = (p + 1 + rng.below(10)).min(count);
        rec.curve(Curve::least_squares(
            &data,
            n,
            p,
            RULES[t % 3],
            V,
            &mut meter(),
        ));
    }
    for t in 0..16 {
        let count = 10 + rng.below(20);
        let data = polygon(&mut rng, count);
        let u = parameters(&data, Parametrization::ChordLength, &mut meter()).unwrap();
        let p = 2 + rng.below(2);
        let basis = least_squares_knots(&u, p + 4, p, &mut meter()).unwrap();
        let weights: Vec<f64> = (0..count).map(|_| 0.5 + rng.unit()).collect();
        let cons = [
            Constraint {
                parameter: 0.0,
                order: 0,
                value: data[0],
            },
            Constraint {
                parameter: 1.0,
                order: 1,
                value: [1.0, 0.0, -1.0],
            },
            Constraint {
                parameter: 0.4,
                order: 2,
                value: [0.0, 0.25, 0.5],
            },
        ];
        rec.curve(Curve::least_squares_at(
            &data,
            Some(&weights),
            &u,
            basis,
            &cons[..t % 4],
            V,
            &mut meter(),
        ));
    }
    for t in 0..8 {
        let (rows, cols) = (2 + rng.below(8), 2 + rng.below(8));
        let data = grid(&mut rng, rows, cols);
        let degrees = (
            1 + rng.below(3.min(rows - 1)),
            1 + rng.below(3.min(cols - 1)),
        );
        rec.surface(Surface::interpolate(
            &data,
            (rows, cols),
            degrees,
            RULES[t % 3],
            V,
            &mut meter(),
        ));
    }
    for t in 0..8 {
        let (rows, cols) = (4 + rng.below(6), 4 + rng.below(6));
        let data = grid(&mut rng, rows, cols);
        let degrees = (1 + rng.below(2), 1 + rng.below(2));
        let counts = (degrees.0 + 2, degrees.1 + 2);
        rec.surface(Surface::least_squares(
            &data,
            (rows, cols),
            counts,
            degrees,
            RULES[t % 3],
            V,
            &mut meter(),
        ));
    }
    let helix: Vec<[f64; 3]> = (0..150)
        .map(|k| {
            let t = f64::from(k) / 149.0 * 1.3;
            [cospi(t) * 2.0, sinpi(t) * 2.0, 0.9 * t]
        })
        .collect();
    let mut corner: Vec<[f64; 2]> = (0..30).map(|k| [f64::from(k) / 29.0, 0.0]).collect();
    corner.extend((1..30).map(|k| [1.0, f64::from(k) / 29.0]));
    let noisy: Vec<[f64; 2]> = (0..100)
        .map(|k| {
            let x = f64::from(k) / 99.0;
            [x, sinpi(x * 2.0) + 0.01 * (rng.unit() - 0.5)]
        })
        .collect();
    for mode in [KnotMode::Single, KnotMode::Multiple] {
        for (tol, degree) in [(1e-3, 3), (1e-4, 2)] {
            rec.curve(Curve::approximate(
                &helix,
                degree,
                tol,
                mode,
                V,
                &mut meter(),
            ));
        }
        rec.curve2(Curve::approximate(&corner, 3, 1e-4, mode, V, &mut meter()));
        rec.curve2(Curve::approximate(&noisy, 3, 0.02, mode, V, &mut meter()));
    }
    // A refusal of each kind the fits make before any arithmetic.
    let few = polygon(&mut rng, 3);
    rec.curve(Curve::interpolate(
        &few,
        3,
        Parametrization::ChordLength,
        V,
        &mut meter(),
    ));
    rec.curve(Curve::interpolate(
        &[[0.0; 3], [0.0; 3]],
        1,
        Parametrization::ChordLength,
        V,
        &mut meter(),
    ));
    rec.curve(Curve::least_squares(
        &few,
        4,
        2,
        Parametrization::ChordLength,
        V,
        &mut meter(),
    ));
    rec.curve(Curve::approximate(
        &few,
        2,
        -1.0,
        KnotMode::Single,
        V,
        &mut meter(),
    ));
    rec.curve(Curve::interpolate(
        &helix,
        3,
        Parametrization::ChordLength,
        V,
        &mut WorkMeter::new(Budget::of_work(1_000)),
    ));
    rec
}

#[test]
fn fit_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-nurbs fit corpus: {} results ({} refusals), {} bytes, golden hash {}",
        rec.calls,
        rec.refusals,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert_eq!(rec.calls, 157);
    assert_eq!(rec.refusals, 5, "only the five refusals asked for");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different control point, \
             parameter, error bound or step count (compare this log's digest with another \
             target's), or a fit changed in place, which chapter 26.2 forbids: a new result is \
             a new FitVersion"
        )
    });
}
