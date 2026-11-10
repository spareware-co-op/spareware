//! Audit of the knot operations on curves and surfaces of higher degrees and denser knot patterns
//! than the crate's own tests use (degrees to 12, interior knots of every multiplicity up to the
//! degree, elevation by up to 6, unclamped curves for insertion and refinement), by sampling: the
//! result of each operation is compared with its input at many parameters, in doubles, against
//! the operation's stated deviation plus the evaluations' own rounding.
//!
//! The references are the curve's and surface's own point evaluation (A3.1, A4.1, A3.5), which the
//! crate's tests check against exact rationals, and for derivative curves the derivative
//! evaluation (A3.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

use geom_nurbs::{Basis, Curve, Direction, Limit, NurbsError, Surface};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

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
}

/// A knot vector of degree `p`: clamped or not, with interior knots of multiplicity up to `p`.
fn knots(rng: &mut Lcg, p: usize, clamped: bool) -> Vec<f64> {
    let mut k: Vec<f64> = Vec::new();
    let mut x = rng.range(-3.0, 3.0);
    if clamped {
        k.extend(std::iter::repeat_n(x, p + 1));
    } else {
        for _ in 0..=p {
            k.push(x);
            x += rng.range(0.05, 0.6);
        }
        k.push(x);
    }
    for _ in 0..rng.below(5) {
        x += [0.125, 0.5, rng.range(1e-3, 2.0)][rng.below(3)];
        let mult = match rng.below(4) {
            0 => p,
            1 => 1 + rng.below(p),
            _ => 1,
        };
        k.extend(std::iter::repeat_n(x, mult));
    }
    x += rng.range(0.1, 1.5);
    if clamped {
        k.extend(std::iter::repeat_n(x, p + 1));
    } else {
        for _ in 0..=p {
            k.push(x);
            x += rng.range(0.05, 0.6);
        }
    }
    k
}

fn curve(rng: &mut Lcg, p: usize, clamped: bool, rational: bool) -> Curve<3> {
    let basis = Basis::new(p, knots(rng, p, clamped)).unwrap();
    let pts: Vec<[f64; 3]> = (0..basis.count())
        .map(|_| [rng.range(-10.0, 10.0), rng.range(-1e-3, 1e-3), rng.range(0.0, 500.0)])
        .collect();
    if rational {
        let w: Vec<f64> = (0..basis.count()).map(|_| rng.range(0.2, 4.0)).collect();
        Curve::new_rational(basis, &pts, &w).unwrap()
    } else {
        Curve::new(basis, &pts).unwrap()
    }
}

fn dist(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f64>().sqrt()
}

/// Parameters across a domain: its ends, every knot inside it, and points between.
fn params(basis: &Basis, rng: &mut Lcg) -> Vec<f64> {
    let (lo, hi) = basis.domain();
    let mut out: Vec<f64> = basis.knots().iter().copied().filter(|&k| lo <= k && k <= hi).collect();
    for _ in 0..40 {
        out.push(rng.range(lo, hi));
    }
    out.push(lo);
    out.push(hi);
    out
}

/// The largest distance between two curves on one domain, from both sides at each parameter.
fn apart(a: &Curve<3>, b: &Curve<3>, rng: &mut Lcg) -> f64 {
    let mut worst: f64 = 0.0;
    for u in params(a.basis(), rng) {
        for limit in [Limit::FromRight, Limit::FromLeft] {
            let (p, q) = (a.point(u, limit).unwrap(), b.point(u, limit).unwrap());
            worst = worst.max(dist(&p, &q));
        }
    }
    worst
}

/// What rounding the two evaluations may add: far above γ·Σ N·|P| for these sizes.
const ROUNDING: f64 = 1e-9;

#[test]
fn curve_operations_keep_the_curve_at_higher_degrees() {
    let mut rng = Lcg(0xa0d1_7095);
    let mut failures = Vec::new();
    for round in 0..160 {
        let p = 1 + rng.below(12);
        let rational = round % 2 == 1;
        let clamped = round % 5 != 0;
        let c = curve(&mut rng, p, clamped, rational);
        let (lo, hi) = c.basis().domain();
        let label = format!("round {round}: degree {p}, rational {rational}, clamped {clamped}");
        // Insertion of a new value and of an existing knot, up to the degree.
        let u = rng.range(lo, hi);
        if lo < u && u < hi {
            let s = c.basis().multiplicity(u);
            for times in [1, p - s] {
                if times == 0 {
                    continue;
                }
                match c.insert_knot(u, times, &mut meter()) {
                    Ok(r) => {
                        let d = apart(&c, &r.value, &mut rng);
                        if d > r.deviation + ROUNDING {
                            failures.push(format!("{label}: insert {u} x{times}: {d:e} apart, deviation {:e}", r.deviation));
                        }
                    }
                    Err(e) => failures.push(format!("{label}: insert {u} x{times} failed: {e:?}")),
                }
            }
        }
        let interior: Vec<f64> = c.basis().knots().iter().copied().filter(|&k| lo < k && k < hi).collect();
        if let Some(&k) = interior.first() {
            let s = c.basis().multiplicity(k);
            if s < p {
                match c.insert_knot(k, p - s, &mut meter()) {
                    Ok(r) => {
                        let d = apart(&c, &r.value, &mut rng);
                        if d > r.deviation + ROUNDING {
                            failures.push(format!("{label}: insert knot {k}: {d:e} apart, deviation {:e}", r.deviation));
                        }
                    }
                    Err(e) => failures.push(format!("{label}: insert knot {k} failed: {e:?}")),
                }
            }
        }
        // Refinement with new values, some repeated, and some existing knots where they fit.
        let mut x: Vec<f64> = (0..rng.below(7)).map(|_| rng.range(lo, hi)).filter(|&v| lo < v && v < hi).collect();
        if let Some(&first) = x.first() {
            if p >= 2 {
                x.push(first);
            }
        }
        for &k in &interior {
            if c.basis().multiplicity(k) < p && rng.below(2) == 0 && !x.contains(&k) {
                x.push(k);
            }
        }
        x.sort_by(f64::total_cmp);
        match c.refine(&x, &mut meter()) {
            Ok(r) => {
                let d = apart(&c, &r.value, &mut rng);
                if d > r.deviation + ROUNDING {
                    failures.push(format!("{label}: refine {x:?}: {d:e} apart, deviation {:e}", r.deviation));
                }
            }
            Err(e) => failures.push(format!("{label}: refine {x:?} failed: {e:?}")),
        }
        if !clamped {
            continue;
        }
        // Elevation by up to 6, and the reduction of an elevated curve back.
        let t = 1 + rng.below(6);
        match c.elevate(t, &mut meter()) {
            Ok(r) => {
                let d = apart(&c, &r.value, &mut rng);
                if d > r.deviation + ROUNDING {
                    failures.push(format!("{label}: elevate by {t}: {d:e} apart, deviation {:e}", r.deviation));
                }
                if r.value.degree() != p + t {
                    failures.push(format!("{label}: elevate by {t} gave degree {}", r.value.degree()));
                }
            }
            Err(e) => failures.push(format!("{label}: elevate by {t} failed: {e:?}")),
        }
        if let Ok(up) = c.elevate(1, &mut meter()) {
            match up.value.reduce_degree(1e-6, &mut meter()) {
                Ok(r) => {
                    let d = apart(&c, &r.value, &mut rng);
                    if d > r.deviation + up.deviation + ROUNDING {
                        failures.push(format!("{label}: reduce after elevate: {d:e} apart, deviation {:e}", r.deviation));
                    }
                }
                // A refusal is allowed: reduction is ill-conditioned at high degree (the certified
                // error of reducing an elevated curve grows from about 1e-12 at degree 3 to 1e-7 at
                // degree 12 on coordinates of a few hundred), and it never returns a curve it
                // cannot certify.
                Err(NurbsError::OutsideTolerance) => {}
                Err(e) => failures.push(format!("{label}: reduce failed: {e:?}")),
            }
        }
        // Bezier pieces.
        match c.to_bezier(&mut meter()) {
            Ok(r) => {
                for piece in &r.value {
                    let (a, b) = piece.basis().domain();
                    for i in 0..=12 {
                        let u = (a + (b - a) * f64::from(i) / 12.0).clamp(a, b);
                        let limit = if i == 0 { Limit::FromRight } else { Limit::FromLeft };
                        let d = dist(&c.point(u, limit).unwrap(), &piece.point(u, limit).unwrap());
                        if d > r.deviation + ROUNDING {
                            failures.push(format!("{label}: a Bezier piece at {u}: {d:e} apart, deviation {:e}", r.deviation));
                        }
                    }
                }
            }
            Err(e) => failures.push(format!("{label}: to_bezier failed: {e:?}")),
        }
        // A knot put in is taken out again, within its tolerance.
        if lo < u && u < hi && c.basis().multiplicity(u) == 0 {
            let put = c.insert_knot(u, 1, &mut meter()).unwrap();
            match put.value.remove_knot(u, 1, 1e-6, &mut meter()) {
                Ok((r, removed)) => {
                    let d = apart(&c, &r.value, &mut rng);
                    if d > r.deviation + put.deviation + ROUNDING {
                        failures.push(format!("{label}: remove {u}: {d:e} apart, deviation {:e}", r.deviation));
                    }
                    // Not removing is allowed (close knots at high degree make removal
                    // ill-conditioned); the curve returned must still be the input's.
                    let _ = removed;
                }
                Err(e) => failures.push(format!("{label}: remove {u} failed: {e:?}")),
            }
        }
        // Derivative curves against the derivative evaluation.
        if !rational && p >= 2 {
            let k = 1 + rng.below(p - 1);
            match c.derivative_curve(k, &mut meter()) {
                Ok(dc) => {
                    for u in params(c.basis(), &mut rng) {
                        let mut d = vec![[0.0; 3]; k + 1];
                        c.derivs(u, Limit::FromRight, &mut d).unwrap();
                        let q = dc.point(u, Limit::FromRight).unwrap();
                        let size = d[k].iter().fold(1.0_f64, |m, x| m.max(x.abs()));
                        if dist(&d[k], &q) > 1e-9 * size {
                            failures.push(format!("{label}: derivative curve {k} at {u}: {q:?} against {:?}", d[k]));
                            break;
                        }
                    }
                }
                Err(NurbsError::Discontinuous) => {}
                Err(e) => failures.push(format!("{label}: derivative curve {k} failed: {e:?}")),
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

fn surface(rng: &mut Lcg, rational: bool) -> Surface<3> {
    let (p, q) = (1 + rng.below(6), 1 + rng.below(6));
    let bu = Basis::new(p, knots(rng, p, true)).unwrap();
    let bv = Basis::new(q, knots(rng, q, true)).unwrap();
    let n = bu.count() * bv.count();
    let pts: Vec<[f64; 3]> = (0..n)
        .map(|_| [rng.range(-10.0, 10.0), rng.range(-1e-3, 1e-3), rng.range(0.0, 500.0)])
        .collect();
    if rational {
        let w: Vec<f64> = (0..n).map(|_| rng.range(0.2, 4.0)).collect();
        Surface::new_rational(bu, bv, &pts, &w).unwrap()
    } else {
        Surface::new(bu, bv, &pts).unwrap()
    }
}

fn surfaces_apart(a: &Surface<3>, b: &Surface<3>, rng: &mut Lcg) -> f64 {
    let mut worst: f64 = 0.0;
    let (us, vs) = (params(a.basis_u(), rng), params(a.basis_v(), rng));
    for (i, &u) in us.iter().enumerate() {
        for &v in vs.iter().skip(i % 3).step_by(3) {
            for limits in [(Limit::FromRight, Limit::FromRight), (Limit::FromLeft, Limit::FromLeft)] {
                let (p, q) = (a.point(u, v, limits).unwrap(), b.point(u, v, limits).unwrap());
                worst = worst.max(dist(&p, &q));
            }
        }
    }
    worst
}

#[test]
fn surface_operations_keep_the_surface_in_both_directions() {
    let mut rng = Lcg(0xa0d1_7096);
    let mut failures = Vec::new();
    for round in 0..40 {
        let rational = round % 2 == 1;
        let s = surface(&mut rng, rational);
        for dir in [Direction::U, Direction::V] {
            let basis = if dir == Direction::U { s.basis_u() } else { s.basis_v() };
            let p = basis.degree();
            let (lo, hi) = basis.domain();
            let label = format!("round {round}: {dir:?}, degree {p}, rational {rational}");
            let u = rng.range(lo, hi);
            if lo < u && u < hi && basis.multiplicity(u) == 0 {
                match s.insert_knot(dir, u, p, &mut meter()) {
                    Ok(r) => {
                        let d = surfaces_apart(&s, &r.value, &mut rng);
                        if d > r.deviation + ROUNDING {
                            failures.push(format!("{label}: insert {u}: {d:e} apart, deviation {:e}", r.deviation));
                        }
                    }
                    Err(e) => failures.push(format!("{label}: insert failed: {e:?}")),
                }
            }
            let mut x: Vec<f64> = (0..1 + rng.below(4)).map(|_| rng.range(lo, hi)).filter(|&v| lo < v && v < hi).collect();
            x.sort_by(f64::total_cmp);
            match s.refine(dir, &x, &mut meter()) {
                Ok(r) => {
                    let d = surfaces_apart(&s, &r.value, &mut rng);
                    if d > r.deviation + ROUNDING {
                        failures.push(format!("{label}: refine: {d:e} apart, deviation {:e}", r.deviation));
                    }
                }
                Err(e) => failures.push(format!("{label}: refine failed: {e:?}")),
            }
            let t = 1 + rng.below(4);
            match s.elevate(dir, t, &mut meter()) {
                Ok(r) => {
                    let d = surfaces_apart(&s, &r.value, &mut rng);
                    if d > r.deviation + ROUNDING {
                        failures.push(format!("{label}: elevate by {t}: {d:e} apart, deviation {:e}", r.deviation));
                    }
                }
                Err(e) => failures.push(format!("{label}: elevate failed: {e:?}")),
            }
        }
        // Derivative surfaces against the partial derivatives.
        let (p, q) = (s.basis_u().degree(), s.basis_v().degree());
        if !rational && p >= 2 && q >= 2 {
            let (k, l) = (rng.below(p), rng.below(q));
            match s.derivative_surface((k, l), &mut meter()) {
                Ok(ds) => {
                    'outer: for u in params(s.basis_u(), &mut rng).into_iter().take(12) {
                        for v in params(s.basis_v(), &mut rng).into_iter().take(12) {
                            let limits = (Limit::FromRight, Limit::FromRight);
                            let mut d = vec![[0.0; 3]; (k + 1) * (l + 1)];
                            s.derivs(u, v, limits, (k, l), &mut d).unwrap();
                            let want = d[k * (l + 1) + l];
                            let got = ds.point(u, v, limits).unwrap();
                            let size = want.iter().fold(1.0_f64, |m, x| m.max(x.abs()));
                            if dist(&want, &got) > 1e-9 * size {
                                failures.push(format!(
                                    "round {round}: derivative surface ({k}, {l}) at ({u}, {v}): {got:?} against {want:?}"
                                ));
                                break 'outer;
                            }
                        }
                    }
                }
                Err(NurbsError::Discontinuous) => {}
                Err(e) => failures.push(format!("round {round}: derivative surface ({k}, {l}) failed: {e:?}")),
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
