//! The replay hash of piece G56's knot operations: every operation on a fixed corpus of curves and
//! surfaces, its result's knots, control points, weights and deviation, hashed with SHA-256, must
//! give the same digest on every target (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus: from a fixed sequence, curves of degrees 1 to 5, 7, 9 and 12, with whole, dyadic and
//! arbitrary knots, some repeated up to p + 1 times, clamped and not, rational and not; and
//! surfaces of degrees 1 to 4 in each direction. On each curve: knot insertion at every interior
//! knot and between them, up to the degree; refinement, by new knots, by the curve's own knots and
//! by both; elevation by 1 to 3; Bezier extraction;
//! removal of every interior knot at four tolerances; degree reduction at four tolerances, of the
//! curve and of its elevation; and derivative curves. On each surface the same in both directions.
//! Every result, success or error, is written in a fixed little-endian encoding, doubles as their
//! bits. The operations certify their results after the fact, so a change in how a result is
//! computed can still pass every bound the other tests check; here it changes the digest. A
//! changed digest is a bug, or a new definition with a golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "the corpus generator; the rules are for kernel code"
)]

mod common;

use common::{Lcg, knots};
use geom_nurbs::{Basis, Changed, Curve, Direction, NurbsError, Surface};
use geom_run::{Budget, WorkMeter};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-nurbs/operations";

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

#[derive(Default)]
struct Rec {
    bytes: Vec<u8>,
    calls: u32,
    errors: u32,
}

impl Rec {
    fn f(&mut self, v: f64) {
        assert!(!v.is_nan(), "a NaN result");
        self.bytes.extend_from_slice(&v.to_bits().to_le_bytes());
    }

    fn u(&mut self, v: usize) {
        self.bytes.extend_from_slice(&(v as u64).to_le_bytes());
    }

    fn err(&mut self, e: &NurbsError) {
        assert_ne!(*e, NurbsError::Defect, "a defect was reported");
        self.errors += 1;
        self.bytes.push(0xEE);
        self.bytes.extend_from_slice(format!("{e:?}").as_bytes());
    }

    fn basis(&mut self, b: &Basis) {
        self.u(b.degree());
        self.u(b.knots().len());
        for &k in b.knots() {
            self.f(k);
        }
    }

    fn curve(&mut self, c: &Curve<3>) {
        self.basis(c.basis());
        for row in c.homogeneous() {
            for &v in row {
                self.f(v);
            }
        }
        for &w in c.weights().unwrap_or(&[]) {
            self.f(w);
        }
    }

    fn surface(&mut self, s: &Surface<3>) {
        self.basis(s.basis_u());
        self.basis(s.basis_v());
        for row in s.homogeneous() {
            for &v in row {
                self.f(v);
            }
        }
        for &w in s.weights().unwrap_or(&[]) {
            self.f(w);
        }
    }

    fn changed_curve(&mut self, r: &Result<Changed<Curve<3>>, NurbsError>) {
        self.calls += 1;
        match r {
            Ok(c) => {
                self.bytes.push(1);
                self.curve(&c.value);
                self.f(c.deviation);
            }
            Err(e) => self.err(e),
        }
    }

    fn changed_surface(&mut self, r: &Result<Changed<Surface<3>>, NurbsError>) {
        self.calls += 1;
        match r {
            Ok(s) => {
                self.bytes.push(1);
                self.surface(&s.value);
                self.f(s.deviation);
            }
            Err(e) => self.err(e),
        }
    }
}

/// Curves of every kind the operations treat differently.
fn curves(rng: &mut Lcg) -> Vec<Curve<3>> {
    let mut out = Vec::new();
    for p in [1, 2, 3, 4, 5, 7, 9, 12] {
        for kind in 0..3 {
            for clamped in [true, false] {
                let n = p + 2 + rng.below(6);
                let basis = Basis::new(p, knots(rng, p, n, clamped, kind)).unwrap();
                let points: Vec<[f64; 3]> = (0..basis.count())
                    .map(|_| {
                        [
                            rng.unit() * 20.0 - 10.0,
                            rng.unit() * 4.0,
                            rng.unit() * 100.0,
                        ]
                    })
                    .collect();
                out.push(Curve::new(basis.clone(), &points).unwrap());
                let w: Vec<f64> = (0..basis.count()).map(|_| 0.3 + rng.unit() * 3.0).collect();
                out.push(Curve::new_rational(basis, &points, &w).unwrap());
            }
        }
    }
    out
}

/// The distinct interior knots of a basis.
fn interior(b: &Basis) -> Vec<f64> {
    let (lo, hi) = b.domain();
    let mut k: Vec<f64> = b
        .knots()
        .iter()
        .copied()
        .filter(|&k| lo < k && k < hi)
        .collect();
    k.dedup();
    k
}

fn on_curve(rec: &mut Rec, rng: &mut Lcg, c: &Curve<3>) {
    let b = c.basis().clone();
    let p = b.degree();
    let (lo, hi) = b.domain();
    let knots_in = interior(&b);
    for &u in knots_in.iter().take(3) {
        for times in 1..=2 {
            rec.changed_curve(&c.insert_knot(u, times, &mut meter()));
        }
        let s = b.multiplicity(u);
        for tol in [0.0, 1e-9, 1e-3, f64::MAX] {
            match c.remove_knot(u, s, tol, &mut meter()) {
                Ok((r, removed)) => {
                    rec.changed_curve(&Ok(r));
                    rec.u(removed);
                }
                Err(e) => rec.err(&e),
            }
        }
    }
    let between = lo + (hi - lo) * (0.1 + 0.8 * rng.unit());
    for times in [1, p] {
        let ins = c.insert_knot(between, times, &mut meter());
        rec.changed_curve(&ins);
        // Remove what was inserted, at each tolerance: the sweeps of every removal step.
        if let Ok(ins) = ins {
            for tol in [0.0, 1e-9, f64::MAX] {
                match ins.value.remove_knot(between, times, tol, &mut meter()) {
                    Ok((r, removed)) => {
                        rec.changed_curve(&Ok(r));
                        rec.u(removed);
                    }
                    Err(e) => rec.err(&e),
                }
            }
        }
    }
    let mut x: Vec<f64> = (0..3).map(|_| lo + (hi - lo) * rng.unit()).collect();
    x.sort_by(f64::total_cmp);
    rec.changed_curve(&c.refine(&x, &mut meter()));
    // Refinement by knots the curve has already, and by a mix of old and new.
    rec.changed_curve(&c.refine(&knots_in, &mut meter()));
    let mut mixed: Vec<f64> = knots_in.iter().copied().chain(x.iter().copied()).collect();
    mixed.sort_by(f64::total_cmp);
    rec.changed_curve(&c.refine(&mixed, &mut meter()));
    for t in 1..=3 {
        rec.changed_curve(&c.elevate(t, &mut meter()));
    }
    rec.calls += 1;
    match c.to_bezier(&mut meter()) {
        Ok(pieces) => {
            rec.u(pieces.value.len());
            for piece in &pieces.value {
                rec.curve(piece);
            }
            rec.f(pieces.deviation);
        }
        Err(e) => rec.err(&e),
    }
    for tol in [0.0, 1e-9, 1e-3, f64::MAX] {
        rec.changed_curve(&c.reduce_degree(tol, &mut meter()));
    }
    if let Ok(up) = c.elevate(1, &mut meter()) {
        for tol in [1e-9, 1e-3, f64::MAX] {
            rec.changed_curve(&up.value.reduce_degree(tol, &mut meter()));
        }
    }
    for k in 1..=2 {
        rec.calls += 1;
        match c.derivative_curve(k, &mut meter()) {
            Ok(d) => rec.curve(&d),
            Err(e) => rec.err(&e),
        }
    }
}

fn on_surface(rec: &mut Rec, rng: &mut Lcg, s: &Surface<3>) {
    for dir in [Direction::U, Direction::V] {
        let b = match dir {
            Direction::U => s.basis_u().clone(),
            Direction::V => s.basis_v().clone(),
        };
        let (lo, hi) = b.domain();
        let u = lo + (hi - lo) * (0.1 + 0.8 * rng.unit());
        for times in 1..=2 {
            rec.changed_surface(&s.insert_knot(dir, u, times, &mut meter()));
        }
        let mut x: Vec<f64> = (0..3).map(|_| lo + (hi - lo) * rng.unit()).collect();
        x.sort_by(f64::total_cmp);
        rec.changed_surface(&s.refine(dir, &x, &mut meter()));
        for t in 1..=2 {
            rec.changed_surface(&s.elevate(dir, t, &mut meter()));
        }
    }
    rec.calls += 1;
    match s.to_bezier(&mut meter()) {
        Ok(pieces) => {
            rec.u(pieces.value.len());
            for piece in &pieces.value {
                rec.surface(piece);
            }
            rec.f(pieces.deviation);
        }
        Err(e) => rec.err(&e),
    }
    for orders in [(1, 0), (0, 1), (1, 1), (2, 1)] {
        rec.calls += 1;
        match s.derivative_surface(orders, &mut meter()) {
            Ok(d) => rec.surface(&d),
            Err(e) => rec.err(&e),
        }
    }
}

fn corpus() -> Rec {
    let mut rng = Lcg(0x0056_0056);
    let mut rec = Rec::default();
    for c in curves(&mut rng) {
        on_curve(&mut rec, &mut rng, &c);
    }
    for (p, q) in [(1, 1), (2, 1), (2, 3), (3, 3), (4, 2), (1, 4)] {
        for clamped in [true, false] {
            for rational in [false, true] {
                let (nu, nv) = (p + 2 + rng.below(3), q + 2 + rng.below(3));
                let (ku, kv) = (rng.below(3), rng.below(3));
                let bu = Basis::new(p, knots(&mut rng, p, nu, clamped, ku)).unwrap();
                let bv = Basis::new(q, knots(&mut rng, q, nv, clamped, kv)).unwrap();
                let count = bu.count() * bv.count();
                let pts: Vec<[f64; 3]> = (0..count)
                    .map(|_| [rng.unit() * 10.0, rng.unit() * 10.0 - 5.0, rng.unit()])
                    .collect();
                let s = if rational {
                    let w: Vec<f64> = (0..count).map(|_| 0.5 + rng.unit() * 2.0).collect();
                    Surface::new_rational(bu, bv, &pts, &w).unwrap()
                } else {
                    Surface::new(bu, bv, &pts).unwrap()
                };
                on_surface(&mut rec, &mut rng, &s);
            }
        }
    }
    rec
}

#[test]
fn operations_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-nurbs operations corpus: {} results, {} refused, {} bytes, golden hash {}",
        rec.calls,
        rec.errors,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert!(rec.calls > 1500, "only {} results", rec.calls);
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes an operation's result \
             differently (compare this log's digest with another target's), or a definition \
             changed in place, which chapter 26.2 forbids"
        )
    });
}
