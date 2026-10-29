//! The replay hash of piece G59: the Bernstein toolkit run over a fixed corpus and hashed with
//! SHA-256 must give the same digest on every target (chapters 21 and 28.13; the golden store of
//! piece G19).
//!
//! The corpus, from a fixed linear congruential sequence: 200 surfaces (degrees 1 to 3, one to
//! three spans each way, rational half the time) with every patch's coefficients, box, derivative
//! boxes, fat plane and normal patch; 200 pairs of plane curves with every piece's box,
//! derivative box, fat line and the clips of the other curve's pieces; 50 products (scalar, dot
//! and cross); 20 exact normal patches and plane coefficients of grid patches; and a refusal of
//! each kind. Every double is written as its bits and every interval as its two ends, each error
//! as its name, so a target that rounds one step differently changes the digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_num::grid::P3;
use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_nurbs::exact::{normal_patch, plane_coefficients};
use geom_nurbs::{
    Basis, BernsteinCurve, BernsteinPatch, Bounds, Curve, Direction, NurbsError, Slab, Surface,
};
use geom_run::{Budget, WorkMeter};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-nurbs/bernstein";

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
    fn below(&mut self, n: usize) -> usize {
        ((self.next() >> 33) % n as u64) as usize
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
    fn result<T>(&mut self, r: Result<T, NurbsError>, f: impl FnOnce(&mut Self, T)) {
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
    fn bounds<const D: usize>(&mut self, b: Bounds<D>) {
        for &x in b.lo.iter().chain(&b.hi) {
            self.f64(x);
        }
    }
    fn slab<const D: usize>(&mut self, s: Slab<D>) {
        for &x in s.origin.iter().chain(&s.normal) {
            self.f64(x);
        }
        self.f64(s.lo);
        self.f64(s.hi);
    }
    fn curve<const D: usize>(&mut self, c: &BernsteinCurve<D>) {
        for p in c.points() {
            for &x in p {
                self.iv(x);
            }
        }
        for &w in c.weights().unwrap_or(&[]) {
            self.iv(w);
        }
    }
    fn patch<const D: usize>(&mut self, c: &BernsteinPatch<D>) {
        let (p, q) = c.degrees();
        self.bytes.push(p as u8);
        self.bytes.push(q as u8);
        for pt in c.points() {
            for &x in pt {
                self.iv(x);
            }
        }
        for &w in c.weights().unwrap_or(&[]) {
            self.iv(w);
        }
    }
}

fn knots(rng: &mut Lcg, deg: usize, spans: usize) -> Vec<f64> {
    let mut k = vec![0.0; deg + 1];
    let mut x = 0.0;
    for _ in 1..spans {
        x += f64::from(1 + rng.below(4) as u32) / 4.0;
        k.push(x);
    }
    x += 1.0;
    k.extend(std::iter::repeat_n(x, deg + 1));
    k
}

fn coord(rng: &mut Lcg) -> f64 {
    (rng.below(1 << 21) as i64 - (1 << 20)) as f64
}

fn surface(rng: &mut Lcg) -> Surface<3> {
    let (p, q) = (1 + rng.below(3), 1 + rng.below(3));
    let (su, sv) = (1 + rng.below(3), 1 + rng.below(3));
    let bu = Basis::new(p, knots(rng, p, su)).unwrap();
    let bv = Basis::new(q, knots(rng, q, sv)).unwrap();
    let n = bu.count() * bv.count();
    let pts: Vec<[f64; 3]> = (0..n)
        .map(|_| [coord(rng), coord(rng), coord(rng)])
        .collect();
    if rng.below(2) == 0 {
        Surface::new(bu, bv, &pts).unwrap()
    } else {
        let w: Vec<f64> = (0..n).map(|_| f64::from(1 + rng.below(4) as u32)).collect();
        Surface::new_rational(bu, bv, &pts, &w).unwrap()
    }
}

fn curve<const D: usize>(rng: &mut Lcg, spans: usize) -> Curve<D> {
    let p = 1 + rng.below(4);
    let basis = Basis::new(p, knots(rng, p, spans)).unwrap();
    let n = basis.count();
    let pts: Vec<[f64; D]> = (0..n)
        .map(|_| core::array::from_fn(|_| coord(rng)))
        .collect();
    if rng.below(2) == 0 {
        Curve::new(basis, &pts).unwrap()
    } else {
        let w: Vec<f64> = (0..n).map(|_| f64::from(1 + rng.below(4) as u32)).collect();
        Curve::new_rational(basis, &pts, &w).unwrap()
    }
}

fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    let mut rng = Lcg(0x4735_3942_4445_5401);
    let m = &mut meter();
    for _ in 0..200 {
        let s = surface(&mut rng);
        let patches = s.bernstein(m).unwrap();
        let normals = s.normal_patches(m).unwrap();
        for (patch, normal) in patches.iter().zip(&normals) {
            rec.patch(patch);
            rec.result(patch.bound(m), Recorder::bounds);
            rec.result(patch.derivative_bound(Direction::U, m), Recorder::bounds);
            rec.result(patch.derivative_bound(Direction::V, m), Recorder::bounds);
            rec.result(patch.fat_plane(m), Recorder::slab);
            rec.patch(normal);
        }
    }
    for _ in 0..200 {
        let (sa, sb) = (1 + rng.below(4), 1 + rng.below(4));
        let a = curve::<2>(&mut rng, sa);
        let b = curve::<2>(&mut rng, sb);
        let pb = b.bernstein(m).unwrap();
        for piece in a.bernstein(m).unwrap() {
            rec.curve(&piece);
            rec.result(piece.bound(m), Recorder::bounds);
            rec.result(piece.derivative_bound(m), Recorder::bounds);
            let line = piece.fat_line(m);
            if let Ok(l) = &line {
                for other in &pb {
                    rec.result(other.clip(l, m), |r, c| match c {
                        Some((x, y)) => {
                            r.f64(x);
                            r.f64(y);
                        }
                        None => r.bytes.push(2),
                    });
                }
            }
            rec.result(line, Recorder::slab);
        }
    }
    for _ in 0..50 {
        let a = curve::<3>(&mut rng, 1)
            .bernstein(m)
            .unwrap()
            .remove(0)
            .numerator();
        let b0 = curve::<3>(&mut rng, 1).bernstein(m).unwrap().remove(0);
        let b = BernsteinCurve::new(a.domain(), b0.points().to_vec(), None).unwrap();
        let f = BernsteinCurve::new(
            a.domain(),
            b.points().iter().map(|p| [p[1]]).collect(),
            None,
        )
        .unwrap();
        rec.result(a.dot(&b, m), |r, c| r.curve(&c));
        rec.result(a.cross(&b, m), |r, c| r.curve(&c));
        rec.result(a.scaled_by(&f, m), |r, c| r.curve(&c));
    }
    for t in 0..20 {
        let (p, q) = (1 + t % 3, 1 + (t / 3) % 3);
        let pts: Vec<P3> = (0..(p + 1) * (q + 1))
            .map(|_| {
                P3::new(
                    rng.below(2001) as i64 - 1000,
                    rng.below(2001) as i64 - 1000,
                    rng.below(2001) as i64 - 1000,
                )
                .unwrap()
            })
            .collect();
        rec.result(normal_patch(&pts, (p, q), m), |r, n| {
            for c in n.coefficients() {
                for x in c {
                    for l in x.to_le_limbs() {
                        r.bytes.extend_from_slice(&l.to_le_bytes());
                    }
                }
            }
        });
        let plane = Plane::through(pts[0], pts[1], pts[2]);
        if let Ok(plane) = plane {
            rec.result(plane_coefficients(&pts, &plane, m), |r, c| {
                for v in c {
                    r.bytes.extend_from_slice(&v.to_le_bytes());
                }
            });
        }
    }
    // A refusal of each kind.
    let r = curve::<3>(&mut Lcg(7), 1).bernstein(m).unwrap().remove(0);
    let rational = BernsteinCurve::new(
        r.domain(),
        r.points().to_vec(),
        Some(vec![Interval::point(2.0).unwrap(); r.points().len()]),
    )
    .unwrap();
    rec.result(rational.hodograph(m), |r, c| r.curve(&c));
    let other = BernsteinCurve::new((5.0, 6.0), r.numerator().points().to_vec(), None).unwrap();
    rec.result(r.numerator().dot(&other, m), |r, c| r.curve(&c));
    let point = BernsteinCurve::new(
        (0.0, 1.0),
        vec![[Interval::point(1.0).unwrap(); 2]; 2],
        None,
    )
    .unwrap();
    rec.result(point.fat_line(m), Recorder::slab);
    rec.result(
        Surface::new(
            Basis::new(1, vec![0.0, 1.0, 2.0, 3.0]).unwrap(),
            Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap(),
            &[[0.0; 3]; 4],
        )
        .unwrap()
        .bernstein(m),
        |r, v| r.bytes.push(v.len() as u8),
    );
    rec
}

#[test]
fn bernstein_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-nurbs Bernstein corpus: {} results ({} refusals), {} bytes, golden hash {}",
        rec.results,
        rec.refusals,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert_eq!(rec.results, 5_960);
    assert_eq!(rec.refusals, 4, "only the four refusals asked for");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different coefficient, bound \
             or clip (compare this log's digest with another target's), or a definition changed \
             in place, which chapter 26.2 forbids"
        )
    });
}
