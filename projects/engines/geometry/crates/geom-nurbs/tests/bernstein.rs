//! The Bernstein toolkit (piece G59) against exact evaluation: on 10,000 seeded patches and 2,500
//! seeded pairs of curves, every box, derivative box, slab and normal-patch bound contains the
//! exact values at a dense grid of parameters; products equal the products of the exact values;
//! Bezier clipping never drops a parameter where the curve is inside the slab; collapsed edges
//! divide out exactly; and the exact integer paths equal exact rational arithmetic, at the
//! extremes of the grid.
//!
//! Inputs are seeded: the default seed is printed as `seed: <hex>`, and `SPAREKIT_SEED=<hex>`
//! replaces it.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::doc_markdown,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::index_refutable_slice,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod common;

use common::{Lcg, Q, derivatives, q, qi, zero};
use dashu_int::IBig;
use geom_num::grid::P3;
use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_nurbs::exact::{normal_patch, plane_coefficients, signs};
use geom_nurbs::{
    Basis, BernsteinCurve, BernsteinPatch, Bounds, Curve, Direction, Edge, Limit, NurbsError,
    Surface,
};
use geom_run::{Budget, WorkMeter};

const DEFAULT_SEED: u64 = 0x4735_3942_4552_4e53;

fn seed() -> u64 {
    let s = std::env::var("SPAREKIT_SEED")
        .ok()
        .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(DEFAULT_SEED);
    println!("seed: {s:#x}");
    s
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn qb(x: i128) -> Q {
    Q::from(IBig::from(x))
}

/// lo ≤ x ≤ hi, exactly.
fn inside(x: &Q, lo: f64, hi: f64) -> bool {
    q(lo) <= *x && *x <= q(hi)
}

/// A box with its ends as exact rationals, converted once.
struct QBox<const D: usize> {
    lo: [Q; D],
    hi: [Q; D],
}

fn qbox<const D: usize>(b: &Bounds<D>) -> QBox<D> {
    QBox {
        lo: b.lo.map(q),
        hi: b.hi.map(q),
    }
}

fn in_box<const D: usize>(x: &[Q; D], b: &QBox<D>) -> bool {
    (0..D).all(|d| b.lo[d] <= x[d] && x[d] <= b.hi[d])
}

/// Exact value and first partials of a (possibly rational) B-spline surface at one parameter
/// pair, from basis tables (`derivatives` at order 1): S, `S_u`, `S_v` and W.
struct Exact3 {
    s: [Q; 3],
    su: [Q; 3],
    sv: [Q; 3],
    w: Q,
}

/// A surface's control points and weights as exact rationals, converted once.
struct QNet {
    pts: Vec<[Q; 3]>,
    w: Vec<Q>,
    cols: usize,
}

fn qnet(s: &Surface<3>) -> QNet {
    QNet {
        pts: s.homogeneous().iter().map(|p| p.map(q)).collect(),
        w: (0..s.homogeneous().len())
            .map(|k| s.weights().map_or(qi(1), |ws| q(ws[k])))
            .collect(),
        cols: s.basis_v().count(),
    }
}

fn exact_surface(net: &QNet, nu: &[Vec<Q>], nv: &[Vec<Q>]) -> Exact3 {
    let mut a: [Q; 3] = core::array::from_fn(|_| zero());
    let mut au = a.clone();
    let mut av = a.clone();
    let (mut w, mut wu, mut wv) = (zero(), zero(), zero());
    for (k, p) in net.pts.iter().enumerate() {
        let (i, j) = (k / net.cols, k % net.cols);
        if (nu[0][i] == zero() && nu[1][i] == zero()) || (nv[0][j] == zero() && nv[1][j] == zero())
        {
            continue;
        }
        let wk = &net.w[k];
        let b00 = &nu[0][i] * &nv[0][j];
        let b10 = &nu[1][i] * &nv[0][j];
        let b01 = &nu[0][i] * &nv[1][j];
        w += &b00 * wk;
        wu += &b10 * wk;
        wv += &b01 * wk;
        for d in 0..3 {
            a[d] += &b00 * &p[d];
            au[d] += &b10 * &p[d];
            av[d] += &b01 * &p[d];
        }
    }
    let w2 = &w * &w;
    let sp: [Q; 3] = core::array::from_fn(|d| &a[d] / &w);
    let su: [Q; 3] = core::array::from_fn(|d| (&au[d] * &w - &a[d] * &wu) / &w2);
    let sv: [Q; 3] = core::array::from_fn(|d| (&av[d] * &w - &a[d] * &wv) / &w2);
    Exact3 { s: sp, su, sv, w }
}

/// Exact basis values and first derivatives at each parameter, from inside [a, b].
fn tables(basis: &Basis, params: &[f64], dom: (f64, f64)) -> Vec<Vec<Vec<Q>>> {
    params
        .iter()
        .map(|&x| derivatives(basis.knots(), basis.degree(), x, inside_limit(x, dom), 1))
        .collect()
}

fn cross_q(x: &[Q; 3], y: &[Q; 3]) -> [Q; 3] {
    [
        &x[1] * &y[2] - &x[2] * &y[1],
        &x[2] * &y[0] - &x[0] * &y[2],
        &x[0] * &y[1] - &x[1] * &y[0],
    ]
}

/// The normal patch's exact value: `S_u` × `S_v` (nonrational), or W³·(`S_u` × `S_v`).
fn exact_normal(e: &Exact3, rational: bool) -> [Q; 3] {
    let n = cross_q(&e.su, &e.sv);
    if rational {
        let w3 = &e.w * &e.w * &e.w;
        n.map(|x| x * &w3)
    } else {
        n
    }
}

/// A random surface: degrees 1 to 3, one to three spans each way with dyadic knots, integer
/// control points up to 2^20, rational with integer weights 1 to 4 half the time.
fn random_surface(rng: &mut Lcg, single: bool) -> Surface<3> {
    let p = 1 + rng.below(3);
    let qd = 1 + rng.below(3);
    let knots = |rng: &mut Lcg, deg: usize| -> Vec<f64> {
        let spans = if single { 1 } else { 1 + rng.below(3) };
        let mut k = vec![0.0; deg + 1];
        let mut x = 0.0;
        for _ in 1..spans {
            x += f64::from(1 + rng.below(4) as u32) / 4.0;
            k.push(x);
        }
        x += f64::from(1 + rng.below(4) as u32) / 4.0;
        k.extend(std::iter::repeat_n(x, deg + 1));
        k
    };
    let bu = Basis::new(p, knots(rng, p)).unwrap();
    let bv = Basis::new(qd, knots(rng, qd)).unwrap();
    let n = bu.count() * bv.count();
    let pts: Vec<[f64; 3]> = (0..n)
        .map(|_| [0, 1, 2].map(|_| (rng.below(1 << 21) as i64 - (1 << 20)) as f64))
        .collect();
    if rng.below(2) == 0 {
        Surface::new(bu, bv, &pts).unwrap()
    } else {
        let w: Vec<f64> = (0..n).map(|_| f64::from(1 + rng.below(4) as u32)).collect();
        Surface::new_rational(bu, bv, &pts, &w).unwrap()
    }
}

/// Dense parameters over [a, b]: the ends and three interior dyadic points.
fn dense((a, b): (f64, f64)) -> [f64; 5] {
    [
        a,
        a + (b - a) * 0.25,
        a + (b - a) * 0.5,
        a + (b - a) * 0.75,
        b,
    ]
}

/// The limit that stays inside [a, b] at x.
fn inside_limit(x: f64, (_, b): (f64, f64)) -> Limit {
    if x == b {
        Limit::FromLeft
    } else {
        Limit::FromRight
    }
}

#[test]
fn patch_bounds_contain_dense_exact_evaluations_on_10000_patches() {
    let mut rng = Lcg(seed());
    let mut checked = 0;
    for trial in 0..10_000 {
        // Most patches are single Bezier patches; one in eight runs extraction over several spans.
        let s = random_surface(&mut rng, trial % 8 != 0);
        let rational = s.is_rational();
        let net = qnet(&s);
        let m = &mut meter();
        let patches = s.bernstein(m).unwrap();
        let normals = s.normal_patches(m).unwrap();
        for (patch, normal) in patches.iter().zip(&normals) {
            let bound = qbox(&patch.bound(m).unwrap());
            let du = qbox(&patch.derivative_bound(Direction::U, m).unwrap());
            let dv = qbox(&patch.derivative_bound(Direction::V, m).unwrap());
            let nb = qbox(&normal.bound(m).unwrap());
            let slab = match patch.fat_plane(m) {
                Ok(sl) => Some(sl),
                Err(NurbsError::Degenerate) => None,
                Err(e) => panic!("{e}"),
            };
            let slab_q = slab.map(|sl| (sl.normal.map(q), sl.origin.map(q), q(sl.lo), q(sl.hi)));
            let (ud, vd) = patch.domain();
            let divided = !normal_degrees_match(patch, normal);
            let tu = tables(s.basis_u(), &dense(ud), ud);
            let tv = tables(s.basis_v(), &dense(vd), vd);
            for nu in &tu {
                for nv in &tv {
                    let e = exact_surface(&net, nu, nv);
                    assert!(in_box(&e.s, &bound), "trial {trial}: point outside its box");
                    assert!(in_box(&e.su, &du), "trial {trial}: S_u outside its box");
                    assert!(in_box(&e.sv, &dv), "trial {trial}: S_v outside its box");
                    if !divided {
                        let n = exact_normal(&e, rational);
                        assert!(in_box(&n, &nb), "trial {trial}: normal outside its bound");
                    }
                    if let Some((n, o, lo, hi)) = &slab_q {
                        let mut dist = zero();
                        for d in 0..3 {
                            dist += &n[d] * (&e.s[d] - &o[d]);
                        }
                        assert!(
                            *lo <= dist && dist <= *hi,
                            "trial {trial}: outside its slab"
                        );
                    }
                    checked += 1;
                }
            }
        }
    }
    println!("{checked} exact evaluations inside their bounds");
}

/// Whether a normal patch has the degrees of an undivided one.
fn normal_degrees_match(patch: &BernsteinPatch<3>, normal: &BernsteinPatch<3>) -> bool {
    let (p, qd) = patch.degrees();
    let k = if patch.is_rational() { 3 } else { 2 };
    normal.degrees() == (k * p - 1, k * qd - 1)
}

/// A random curve of dimension D: degree 1 to 4, one to four spans, integer points, rational half
/// the time.
fn random_curve<const D: usize>(rng: &mut Lcg, single: bool) -> Curve<D> {
    let p = 1 + rng.below(4);
    let spans = if single { 1 } else { 1 + rng.below(4) };
    let mut k = vec![0.0; p + 1];
    let mut x = 0.0;
    for _ in 1..spans {
        x += f64::from(1 + rng.below(4) as u32) / 4.0;
        k.push(x);
    }
    x += 1.0;
    k.extend(std::iter::repeat_n(x, p + 1));
    let basis = Basis::new(p, k).unwrap();
    let n = basis.count();
    let pts: Vec<[f64; D]> = (0..n)
        .map(|_| core::array::from_fn(|_| (rng.below(1 << 21) as i64 - (1 << 20)) as f64))
        .collect();
    if rng.below(2) == 0 {
        Curve::new(basis, &pts).unwrap()
    } else {
        let w: Vec<f64> = (0..n).map(|_| f64::from(1 + rng.below(4) as u32)).collect();
        Curve::new_rational(basis, &pts, &w).unwrap()
    }
}

/// A curve's control points and weights as exact rationals.
type QCurve<const D: usize> = (Vec<[Q; D]>, Vec<Q>);

fn qcurve<const D: usize>(c: &Curve<D>) -> QCurve<D> {
    let pts = c.homogeneous().iter().map(|p| p.map(q)).collect();
    let w = (0..c.homogeneous().len())
        .map(|i| c.weights().map_or(qi(1), |ws| q(ws[i])))
        .collect();
    (pts, w)
}

/// Exact point and first derivative of a curve from a basis table at one parameter.
fn exact_curve<const D: usize>((pts, ws): &QCurve<D>, n: &[Vec<Q>]) -> ([Q; D], [Q; D]) {
    let mut a: [Q; D] = core::array::from_fn(|_| zero());
    let mut da = a.clone();
    let (mut w, mut dw) = (zero(), zero());
    for (i, p) in pts.iter().enumerate() {
        if n[0][i] == zero() && n[1][i] == zero() {
            continue;
        }
        w += &n[0][i] * &ws[i];
        dw += &n[1][i] * &ws[i];
        for d in 0..D {
            a[d] += &n[0][i] * &p[d];
            da[d] += &n[1][i] * &p[d];
        }
    }
    let point = core::array::from_fn(|d| &a[d] / &w);
    let deriv = core::array::from_fn(|d| (&da[d] * &w - &a[d] * &dw) / (&w * &w));
    (point, deriv)
}

#[test]
fn curve_bounds_and_clips_hold_on_2500_curves() {
    let mut rng = Lcg(seed() ^ 1);
    for trial in 0..2_500 {
        let a = random_curve::<2>(&mut rng, false);
        let b = random_curve::<2>(&mut rng, false);
        let (qa, qbc) = (qcurve(&a), qcurve(&b));
        let m = &mut meter();
        let pa = a.bernstein(m).unwrap();
        let pb = b.bernstein(m).unwrap();
        // b's exact points on each of its pieces, at 17 parameters, once.
        let b_points: Vec<Vec<(f64, [Q; 2])>> = pb
            .iter()
            .map(|other| {
                let (lo, hi) = other.domain();
                let us: Vec<f64> = (0..=16)
                    .map(|k| (lo + (hi - lo) * f64::from(k) / 16.0).clamp(lo, hi))
                    .collect();
                tables(b.basis(), &us, other.domain())
                    .iter()
                    .zip(&us)
                    .map(|(n, &u)| (u, exact_curve(&qbc, n).0))
                    .collect()
            })
            .collect();
        for piece in &pa {
            let bound = qbox(&piece.bound(m).unwrap());
            let db = qbox(&piece.derivative_bound(m).unwrap());
            let line = piece.fat_line(m).ok();
            let line_q = line.map(|l| (l.normal.map(q), l.origin.map(q), q(l.lo), q(l.hi)));
            for n in tables(a.basis(), &dense(piece.domain()), piece.domain()) {
                let (x, dx) = exact_curve(&qa, &n);
                assert!(in_box(&x, &bound), "trial {trial}: point outside its box");
                assert!(
                    in_box(&dx, &db),
                    "trial {trial}: derivative outside its box"
                );
                if let Some((nl, o, lo, hi)) = &line_q {
                    let dist = &nl[0] * (&x[0] - &o[0]) + &nl[1] * (&x[1] - &o[1]);
                    assert!(
                        *lo <= dist && dist <= *hi,
                        "trial {trial}: outside its fat line"
                    );
                }
            }
            // Clip every piece of b against this piece's fat line: no parameter of b inside the
            // line may fall outside the clip.
            let (Some(l), Some((nl, o, lo, hi))) = (line, line_q) else {
                continue;
            };
            for (other, points) in pb.iter().zip(&b_points) {
                let clip = other.clip(&l, m).unwrap();
                for (u, y) in points {
                    let dist = &nl[0] * (&y[0] - &o[0]) + &nl[1] * (&y[1] - &o[1]);
                    if lo <= dist && dist <= hi {
                        let (c0, c1) = clip.unwrap_or_else(|| {
                            panic!("trial {trial}: clip dropped the whole curve")
                        });
                        assert!(c0 <= *u && *u <= c1, "trial {trial}: clip dropped {u}");
                    }
                }
            }
        }
    }
}

/// An interval de Casteljau evaluation of a nonrational Bernstein curve at a dyadic t ∈ [0, 1].
fn casteljau<const D: usize>(c: &BernsteinCurve<D>, t: f64) -> [Interval; D] {
    let tt = Interval::point(t).unwrap();
    let one_t = Interval::point(1.0 - t).unwrap();
    let mut pts: Vec<[Interval; D]> = c.points().to_vec();
    while pts.len() > 1 {
        pts = pts
            .windows(2)
            .map(|w| {
                core::array::from_fn(|d| {
                    one_t
                        .mul(w[0][d])
                        .unwrap()
                        .add(tt.mul(w[1][d]).unwrap())
                        .unwrap()
                })
            })
            .collect();
    }
    pts[0]
}

/// Exact de Casteljau at a dyadic t on a Bernstein curve whose coefficients are points.
fn exact_casteljau<const D: usize>(c: &BernsteinCurve<D>, t: f64) -> [Q; D] {
    let (tt, one_t) = (q(t), qi(1) - q(t));
    let mut pts: Vec<[Q; D]> = c
        .points()
        .iter()
        .map(|p| {
            assert!(p.iter().all(|i| i.lo() == i.hi()));
            p.map(|i| q(i.lo()))
        })
        .collect();
    while pts.len() > 1 {
        pts = pts
            .windows(2)
            .map(|w| core::array::from_fn(|d| &one_t * &w[0][d] + &tt * &w[1][d]))
            .collect();
    }
    pts.remove(0)
}

fn enclosed(x: &Q, i: Interval) -> bool {
    inside(x, i.lo(), i.hi())
}

#[test]
fn products_are_the_products_of_the_values() {
    let mut rng = Lcg(seed() ^ 2);
    for trial in 0..500 {
        let a = random_curve::<3>(&mut rng, true);
        let b = random_curve::<3>(&mut rng, true);
        if a.is_rational() || b.is_rational() {
            continue;
        }
        let m = &mut meter();
        // Pieces on one domain: a's first piece against b reparametrised onto a's first domain.
        let pa = a.bernstein(m).unwrap()[0].clone();
        let pb0 = b.bernstein(m).unwrap()[0].clone();
        let pb = BernsteinCurve::new(pa.domain(), pb0.points().to_vec(), None).unwrap();
        let f = BernsteinCurve::new(
            pa.domain(),
            pb.points().iter().map(|p| [p[0]]).collect(),
            None,
        )
        .unwrap();
        let dot = pa.dot(&pb, m).unwrap();
        let cross = pa.cross(&pb, m).unwrap();
        let scaled = pa.scaled_by(&f, m).unwrap();
        assert_eq!(dot.degree(), pa.degree() + pb.degree());
        for k in 0..=8 {
            let t = f64::from(k) / 8.0;
            // The factors' coefficients are exact points here (single Bezier pieces of integer
            // data), so their exact values come from exact de Casteljau.
            let xq = exact_casteljau(&pa, t);
            let yq = exact_casteljau(&pb, t);
            let d0 = &xq[0] * &yq[0] + &xq[1] * &yq[1] + &xq[2] * &yq[2];
            assert!(
                enclosed(&d0, casteljau(&dot, t)[0]),
                "trial {trial}: dot at {t}"
            );
            let c = cross_q(&xq, &yq);
            let cv = casteljau(&cross, t);
            for d in 0..3 {
                assert!(enclosed(&c[d], cv[d]), "trial {trial}: cross at {t}");
                let sv = casteljau(&scaled, t);
                assert!(
                    enclosed(&(&yq[0] * &xq[d]), sv[d]),
                    "trial {trial}: scale at {t}"
                );
            }
        }
    }
    // Refusals.
    let m = &mut meter();
    let r = Curve::new_rational(
        Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap(),
        &[[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]],
        &[1.0, 2.0],
    )
    .unwrap()
    .bernstein(m)
    .unwrap()
    .remove(0);
    assert_eq!(r.hodograph(m), Err(NurbsError::Rational));
    assert_eq!(r.dot(&r, m), Err(NurbsError::Rational));
    let x = r.numerator();
    let y = BernsteinCurve::new((0.0, 2.0), x.points().to_vec(), None).unwrap();
    assert_eq!(x.dot(&y, m), Err(NurbsError::Domains));
    let flat = BernsteinCurve::new(
        (0.0, 1.0),
        vec![[Interval::point(1.0).unwrap(); 2]; 3],
        None,
    )
    .unwrap();
    assert_eq!(flat.fat_line(m), Err(NurbsError::Degenerate));
    let unclamped = Curve::new(
        Basis::new(1, vec![0.0, 1.0, 2.0, 3.0]).unwrap(),
        &[[0.0, 0.0], [1.0, 1.0]],
    )
    .unwrap();
    assert_eq!(unclamped.bernstein(m), Err(NurbsError::NotClamped));
}

/// A cone-like patch whose u = 0 edge is one point (the apex), degrees (1, 2).
fn apex_patch(rational: bool) -> Surface<3> {
    let bu = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let bv = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let apex = [0.0, 0.0, 8.0];
    let pts = [
        apex,
        apex,
        apex,
        [8.0, 0.0, 0.0],
        [8.0, 8.0, 0.0],
        [0.0, 8.0, 0.0],
    ];
    if rational {
        // The apex row with different weights, so its homogeneous points differ but its
        // Euclidean point does not.
        let w = [1.0, 2.0, 4.0, 1.0, 0.5, 1.0];
        Surface::new_rational(bu, bv, &pts, &w).unwrap()
    } else {
        Surface::new(bu, bv, &pts).unwrap()
    }
}

#[test]
fn collapsed_edges_divide_out_exactly() {
    for rational in [false, true] {
        let s = apex_patch(rational);
        assert_eq!(s.collapsed_edges(), vec![Edge::U0]);
        let m = &mut meter();
        let patch = s.bernstein(m).unwrap().remove(0);
        let full = patch.normal(m).unwrap();
        let reduced = s.normal_patches(m).unwrap().remove(0);
        let (p, qd) = full.degrees();
        assert_eq!(reduced.degrees(), (p - 1, qd));
        // The full normal patch's bound holds the zero vector (the apex); the reduced one's does
        // not: its z component is certainly positive.
        let fb = full.bound(m).unwrap();
        assert!((0..3).all(|d| fb.lo[d] <= 0.0 && fb.hi[d] >= 0.0));
        let rb = reduced.bound(m).unwrap();
        assert!(rb.lo[2] > 0.0, "rational {rational}: {rb:?}");
        // N = u·T at dense points, exactly within T's enclosure.
        for k in 1..=8 {
            for l in 0..=8 {
                let (u, v) = (f64::from(k) / 8.0, f64::from(l) / 8.0);
                let nu = &tables(s.basis_u(), &[u], (0.0, 1.0))[0];
                let nv = &tables(s.basis_v(), &[v], (0.0, 1.0))[0];
                let e = exact_surface(&qnet(&s), nu, nv);
                let n = exact_normal(&e, rational);
                let t = patch_eval(&reduced, u, v);
                for d in 0..3 {
                    assert!(
                        enclosed(&(&n[d] / q(u)), t[d]),
                        "rational {rational} at ({u}, {v})"
                    );
                }
            }
        }
    }
    // A rational edge whose weighted points are not one Euclidean point is not collapsed.
    let bu = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let bv = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let s = Surface::new_rational(
        bu,
        bv,
        &[
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0 + f64::EPSILON],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
        ],
        &[1.0, 3.0, 1.0, 1.0],
    )
    .unwrap();
    assert_eq!(s.collapsed_edges(), Vec::<Edge>::new());
}

/// An interval evaluation of a nonrational Bernstein patch at (u, v) in its domain.
fn patch_eval(p: &BernsteinPatch<3>, u: f64, v: f64) -> [Interval; 3] {
    let ((u0, u1), (v0, v1)) = p.domain();
    let (pd, qd) = p.degrees();
    let tu = (u - u0) / (u1 - u0);
    let tv = (v - v0) / (v1 - v0);
    let rows: Vec<[Interval; 3]> = (0..=pd)
        .map(|i| {
            let row = BernsteinCurve::new(
                (0.0, 1.0),
                p.points()[i * (qd + 1)..(i + 1) * (qd + 1)].to_vec(),
                None,
            )
            .unwrap();
            casteljau(&row, tv)
        })
        .collect();
    casteljau(&BernsteinCurve::new((0.0, 1.0), rows, None).unwrap(), tu)
}

fn p3(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

#[test]
fn the_exact_paths_are_exact_at_the_extremes() {
    const M: i64 = (1 << 40) - 1;
    let m = &mut meter();
    // Plane coefficients at the extremes of the grid.
    let plane = Plane::through(p3(-M, -M, -M), p3(M, -M, M), p3(-M, M, M)).unwrap();
    let corners: Vec<P3> = [-M, M]
        .iter()
        .flat_map(|&x| {
            [-M, M]
                .into_iter()
                .flat_map(move |y| [-M, M].map(|z| (x, y, z)))
        })
        .map(|(x, y, z)| p3(x, y, z))
        .collect();
    let c = plane_coefficients(&corners, &plane, m).unwrap();
    for (p, &v) in corners.iter().zip(&c) {
        let n = plane.n();
        let want = qb(n[0]) * qb(i128::from(p.x.get()))
            + qb(n[1]) * qb(i128::from(p.y.get()))
            + qb(n[2]) * qb(i128::from(p.z.get()))
            + qb(plane.d());
        assert_eq!(qb(v), want);
    }
    let s = signs(&c);
    assert_eq!(s.positive + s.negative + s.zero, 8);
    // The normal patch of grid patches, extreme and random, against exact rationals: the scaled
    // form equals (S_u/p) × (S_v/q) as a polynomial, checked at dyadic points.
    let mut rng = Lcg(seed() ^ 3);
    for trial in 0..200 {
        let (p, qd) = if trial == 0 {
            (28, 28)
        } else {
            (1 + rng.below(4), 1 + rng.below(4))
        };
        let extreme = trial < 4;
        let pts: Vec<P3> = (0..(p + 1) * (qd + 1))
            .map(|_| {
                let r = |rng: &mut Lcg| {
                    if extreme {
                        if rng.below(2) == 0 { M } else { -M }
                    } else {
                        rng.below(2001) as i64 - 1000
                    }
                };
                p3(r(&mut rng), r(&mut rng), r(&mut rng))
            })
            .collect();
        let n = normal_patch(&pts, (p, qd), m).unwrap();
        assert!(n.divided().is_empty() || !extreme);
        let budget = Q::from(IBig::from(1) << 193);
        for c in n.coefficients() {
            for x in c {
                // Within the 2^193 budget of the module header.
                let v = wide_q(x);
                assert!(v < budget && -v < budget, "trial {trial}");
            }
        }
        if p > 4 {
            continue;
        }
        let (dp, dq) = n.degrees();
        for k in 0..=4 {
            for l in 0..=4 {
                let (s, t) = (qi(k) / qi(4), qi(l) / qi(4));
                let lhs: [Q; 3] = scaled_eval(n.coefficients(), (dp, dq), &s, &t);
                let (su, sv) = grid_partials(&pts, (p, qd), &s, &t);
                let rhs = cross_q(&su, &sv);
                assert_eq!(lhs, rhs, "trial {trial} at ({k}/4, {l}/4)");
            }
        }
    }
    // A collapsed grid edge divides out: the reduced form times s equals the full one.
    let apex = p3(0, 0, 8);
    let pts = vec![apex, apex, apex, p3(8, 0, 0), p3(8, 8, 0), p3(0, 8, 0)];
    let n = normal_patch(&pts, (1, 2), m).unwrap();
    assert_eq!(n.divided(), &[Edge::U0]);
    assert!(n.signs(2).unwrap().excludes_zero());
    for k in 1..=4 {
        for l in 0..=4 {
            let (s, t) = (qi(k) / qi(4), qi(l) / qi(4));
            let lhs: [Q; 3] = scaled_eval(n.coefficients(), n.degrees(), &s, &t).map(|x| x * &s);
            let (su, sv) = grid_partials(&pts, (1, 2), &s, &t);
            assert_eq!(lhs, cross_q(&su, &sv));
        }
    }
    assert_eq!(
        normal_patch(&pts, (0, 2), m),
        Err(NurbsError::Degree { degree: 0 })
    );
    assert_eq!(
        normal_patch(&pts[..5], (1, 2), m),
        Err(NurbsError::PointCount {
            points: 5,
            expected: 6
        })
    );
}

/// Σ c_ab s^a (1 − s)^(dp−a) t^b (1 − t)^(dq−b), exactly.
fn scaled_eval(c: &[[geom_num::wide::I256; 3]], (dp, dq): (usize, usize), s: &Q, t: &Q) -> [Q; 3] {
    let one = qi(1);
    let pow = |x: &Q, n: usize| -> Q { (0..n).fold(qi(1), |acc, _| acc * x) };
    let mut out: [Q; 3] = core::array::from_fn(|_| zero());
    for a in 0..=dp {
        for b in 0..=dq {
            let basis = pow(s, a) * pow(&(&one - s), dp - a) * pow(t, b) * pow(&(&one - t), dq - b);
            for d in 0..3 {
                out[d] += &basis * wide_q(&c[a * (dq + 1) + b][d]);
            }
        }
    }
    out
}

fn wide_q(x: &geom_num::wide::I256) -> Q {
    let limbs = x.to_le_limbs();
    let neg = x.is_negative();
    let mut v = IBig::from(0);
    for &l in limbs.iter().rev() {
        v = (v << 64) + IBig::from(l);
    }
    if neg {
        v -= IBig::from(1) << 256;
    }
    Q::from(v)
}

/// (S_u / p, S_v / q) of the Bezier patch on [0, 1]² with integer points, at (s, t), exactly.
fn grid_partials(pts: &[P3], (p, qd): (usize, usize), s: &Q, t: &Q) -> ([Q; 3], [Q; 3]) {
    let one = qi(1);
    let bern = |n: usize, i: usize, x: &Q| -> Q {
        let c = (0..i).fold(qi(1), |acc, k| {
            acc * qi((n - k) as i64) / qi((k + 1) as i64)
        });
        let pow = |y: &Q, m: usize| -> Q { (0..m).fold(qi(1), |acc, _| acc * y) };
        c * pow(x, i) * pow(&(&one - x), n - i)
    };
    let at = |i: usize, j: usize| -> [Q; 3] {
        let p = pts[i * (qd + 1) + j];
        [p.x.get(), p.y.get(), p.z.get()].map(|v| qb(i128::from(v)))
    };
    let mut su: [Q; 3] = core::array::from_fn(|_| zero());
    let mut sv = su.clone();
    for i in 0..p {
        for j in 0..=qd {
            let b = bern(p - 1, i, s) * bern(qd, j, t);
            let (a0, a1) = (at(i, j), at(i + 1, j));
            for d in 0..3 {
                su[d] += &b * (&a1[d] - &a0[d]);
            }
        }
    }
    for i in 0..=p {
        for j in 0..qd {
            let b = bern(p, i, s) * bern(qd - 1, j, t);
            let (a0, a1) = (at(i, j), at(i, j + 1));
            for d in 0..3 {
                sv[d] += &b * (&a1[d] - &a0[d]);
            }
        }
    }
    (su, sv)
}
