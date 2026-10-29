//! Piece G60's de Casteljau steps on Bernstein forms (`restrict` and `value_at` of curves and
//! patches), checked against exact rational arithmetic: every returned interval holds the exact
//! coefficient of the exact sub-piece, or the exact value, and is narrow; the whole domain gives the
//! coefficients back unchanged; every refusal is typed.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod common;

use common::{Lcg, Q, q, qi, zero};
use geom_num::interval::Interval;
use geom_nurbs::{BernsteinCurve, BernsteinPatch, NurbsError};

fn pt(x: f64) -> Interval {
    Interval::point(x).unwrap()
}

/// Whether `x` holds the exact `e` and is at most `width` wide.
fn holds(x: Interval, e: &Q, width: f64) -> bool {
    q(x.lo()) <= *e && *e <= q(x.hi()) && x.hi() - x.lo() <= width
}

/// The left and right parts of exact Bernstein coefficients `c` split at the local parameter `s`.
fn split(c: &[Vec<Q>], s: &Q) -> (Vec<Vec<Q>>, Vec<Vec<Q>>) {
    let one_s = qi(1) - s;
    let mut level = c.to_vec();
    let mut left = vec![level[0].clone()];
    let mut right = vec![level[level.len() - 1].clone()];
    while level.len() > 1 {
        level = level
            .windows(2)
            .map(|w| {
                w[0].iter()
                    .zip(&w[1])
                    .map(|(a, b)| &one_s * a + s * b)
                    .collect()
            })
            .collect();
        left.push(level[0].clone());
        right.push(level[level.len() - 1].clone());
    }
    right.reverse();
    (left, right)
}

/// The exact coefficients of `c` over [a, b] cut down to [lo, hi]: first the left part at lo, then
/// the right part's left part at hi (the reverse order of the code under test).
fn cut(c: &[Vec<Q>], (a, b): (f64, f64), (lo, hi): (f64, f64)) -> Vec<Vec<Q>> {
    let (a, b, lo, hi) = (q(a), q(b), q(lo), q(hi));
    let right = split(c, &((&lo - &a) / (&b - &a))).1;
    split(&right, &((&hi - &lo) / (&b - &lo))).0
}

/// Σ C(p, i)·s^i·(1 − s)^(p − i)·`c_i`, exactly.
fn bernstein_sum(c: &[Vec<Q>], s: &Q) -> Vec<Q> {
    let p = c.len() - 1;
    let mut out = vec![zero(); c[0].len()];
    let mut binom = qi(1);
    for i in 0..=p {
        let mut f = binom.clone();
        for _ in 0..i {
            f *= s;
        }
        for _ in i..p {
            f *= qi(1) - s;
        }
        for (o, x) in out.iter_mut().zip(&c[i]) {
            *o += &f * x;
        }
        binom = binom * qi((p - i) as i64) / qi(i as i64 + 1);
    }
    out
}

/// A random piece of degree 0 to 5 in D = 2, rational half the time, as exact homogeneous rows
/// (x, y, w) and as a [`BernsteinCurve`].
fn piece(rng: &mut Lcg, domain: (f64, f64)) -> (Vec<Vec<Q>>, BernsteinCurve<2>) {
    let p = rng.below(6);
    let rational = rng.below(2) == 0;
    let mut exact = Vec::new();
    let (mut pts, mut ws) = (Vec::new(), Vec::new());
    for _ in 0..=p {
        let (x, y) = (rng.unit() * 8.0 - 4.0, rng.unit() * 8.0 - 4.0);
        let w = if rational {
            0.25 + rng.unit() * 4.0
        } else {
            1.0
        };
        exact.push(vec![q(x), q(y), q(w)]);
        pts.push([pt(x), pt(y)]);
        ws.push(pt(w));
    }
    let c = BernsteinCurve::new(domain, pts, rational.then_some(ws)).unwrap();
    (exact, c)
}

/// The curve's coefficients as rows (x, y, w), w = 1 when nonrational.
fn rows(c: &BernsteinCurve<2>) -> Vec<[Interval; 3]> {
    c.points()
        .iter()
        .enumerate()
        .map(|(i, p)| [p[0], p[1], c.weights().map_or(pt(1.0), |w| w[i])])
        .collect()
}

/// Claim: a curve cut down to a sub-span encloses the exact sub-piece's coefficients (weights
/// included), narrowly, and keeps its degree and rationality.
#[test]
fn a_restricted_curve_encloses_the_exact_sub_piece() {
    let mut rng = Lcg(60);
    for trial in 0..400 {
        let a = rng.unit() * 2.0 - 1.0;
        let b = a + 0.5 + rng.unit() * 3.0;
        let (exact, c) = piece(&mut rng, (a, b));
        let (mut lo, mut hi) = (a + (b - a) * rng.unit(), a + (b - a) * rng.unit());
        if lo > hi {
            core::mem::swap(&mut lo, &mut hi);
        }
        match trial % 4 {
            0 => lo = a,
            1 => hi = b,
            _ => {}
        }
        if lo == hi {
            continue;
        }
        let r = c.restrict((lo, hi)).unwrap();
        assert_eq!(r.domain(), (lo, hi));
        assert_eq!(r.degree(), c.degree());
        assert_eq!(r.is_rational(), c.is_rational());
        let want = cut(&exact, (a, b), (lo, hi));
        for (k, (got, e)) in rows(&r).iter().zip(&want).enumerate() {
            for d in 0..3 {
                assert!(
                    holds(got[d], &e[d], 1e-11),
                    "trial {trial} coefficient {k}.{d}: {:?}",
                    got[d]
                );
            }
        }
    }
}

/// Claim: the whole domain is no cut at all: the coefficients come back bit for bit, not widened.
#[test]
fn restricting_to_the_whole_domain_changes_nothing() {
    let mut rng = Lcg(61);
    for _ in 0..50 {
        let (_, c) = piece(&mut rng, (-1.0, 2.0));
        let r = c.restrict((-1.0, 2.0)).unwrap();
        assert_eq!(r, c);
    }
}

/// Claim: a curve's value at a parameter of its domain, ends included, encloses the exact
/// homogeneous value and weight, narrowly.
#[test]
fn a_curve_value_encloses_the_exact_value() {
    let mut rng = Lcg(62);
    for trial in 0..400 {
        let (a, b) = (0.25, 2.5);
        let (exact, c) = piece(&mut rng, (a, b));
        let t = match trial % 5 {
            0 => a,
            1 => b,
            _ => a + (b - a) * rng.unit(),
        };
        let (v, w) = c.value_at(t).unwrap();
        let e = bernstein_sum(&exact, &((q(t) - q(a)) / (q(b) - q(a))));
        for d in 0..2 {
            assert!(holds(v[d], &e[d], 1e-11), "trial {trial}: {:?}", v[d]);
        }
        assert!(holds(w, &e[2], 1e-11), "trial {trial}: {w:?}");
    }
}

/// A random patch of degrees up to (3, 3) on `domain`, rational half the time, as exact rows of
/// (x, y, w), row by row, and as a [`BernsteinPatch`].
fn patch(
    rng: &mut Lcg,
    domain: ((f64, f64), (f64, f64)),
) -> ((usize, usize), Vec<Vec<Q>>, BernsteinPatch<2>) {
    let (p, qd) = (rng.below(4), rng.below(4));
    let rational = rng.below(2) == 0;
    let mut exact = Vec::new();
    let (mut pts, mut ws) = (Vec::new(), Vec::new());
    for _ in 0..(p + 1) * (qd + 1) {
        let (x, y) = (rng.unit() * 8.0 - 4.0, rng.unit() * 8.0 - 4.0);
        let w = if rational {
            0.25 + rng.unit() * 4.0
        } else {
            1.0
        };
        exact.push(vec![q(x), q(y), q(w)]);
        pts.push([pt(x), pt(y)]);
        ws.push(pt(w));
    }
    let s = BernsteinPatch::new(domain, (p, qd), pts, rational.then_some(ws)).unwrap();
    ((p, qd), exact, s)
}

/// The exact patch coefficients cut to `u` × `v`: every column in u first, then every row in v.
fn cut_patch(
    (p, qd): (usize, usize),
    c: &[Vec<Q>],
    (du, dv): ((f64, f64), (f64, f64)),
    (u, v): ((f64, f64), (f64, f64)),
) -> Vec<Vec<Q>> {
    let mut grid = c.to_vec();
    for j in 0..=qd {
        let column: Vec<Vec<Q>> = (0..=p).map(|i| grid[i * (qd + 1) + j].clone()).collect();
        for (i, x) in cut(&column, du, u).into_iter().enumerate() {
            grid[i * (qd + 1) + j] = x;
        }
    }
    let mut out = Vec::new();
    for i in 0..=p {
        out.extend(cut(&grid[i * (qd + 1)..(i + 1) * (qd + 1)], dv, v));
    }
    out
}

/// Claim: a patch cut down to a sub-box encloses the exact sub-patch's coefficients, narrowly; the
/// whole domain changes nothing.
#[test]
fn a_restricted_patch_encloses_the_exact_sub_patch() {
    let mut rng = Lcg(63);
    let domain = ((0.0, 1.0), (-2.0, 2.0));
    for trial in 0..200 {
        let (degrees, exact, s) = patch(&mut rng, domain);
        let mut sub = |(a, b): (f64, f64)| {
            let (x, y) = (a + (b - a) * rng.unit(), a + (b - a) * rng.unit());
            if x < y { (x, y) } else { (y, x) }
        };
        let (u, v) = match trial % 3 {
            0 => (domain.0, sub(domain.1)),
            1 => (sub(domain.0), domain.1),
            _ => (sub(domain.0), sub(domain.1)),
        };
        let r = s.restrict(u, v).unwrap();
        assert_eq!(r.domain(), (u, v));
        assert_eq!(r.degrees(), degrees);
        assert_eq!(r.is_rational(), s.is_rational());
        let want = cut_patch(degrees, &exact, domain, (u, v));
        for (k, e) in want.iter().enumerate() {
            let got = r.points()[k];
            let w = r.weights().map_or(pt(1.0), |w| w[k]);
            for (d, x) in [got[0], got[1], w].into_iter().enumerate() {
                assert!(
                    holds(x, &e[d], 1e-10),
                    "trial {trial} coefficient {k}.{d}: {x:?}"
                );
            }
        }
        assert_eq!(s.restrict(domain.0, domain.1).unwrap(), s);
    }
}

/// Claim: a patch's value at a parameter pair of its domain, corners included, encloses the exact
/// homogeneous value and weight, narrowly.
#[test]
fn a_patch_value_encloses_the_exact_value() {
    let mut rng = Lcg(64);
    let domain = ((0.5, 1.5), (-1.0, 3.0));
    for trial in 0..200 {
        let ((p, qd), exact, s) = patch(&mut rng, domain);
        let pick = |rng: &mut Lcg, (a, b): (f64, f64), k: usize| match k % 3 {
            0 => a,
            1 => b,
            _ => a + (b - a) * rng.unit(),
        };
        let u = pick(&mut rng, domain.0, trial);
        let v = pick(&mut rng, domain.1, trial / 3);
        let (su, sv) = (
            (q(u) - q(domain.0.0)) / (q(domain.0.1) - q(domain.0.0)),
            (q(v) - q(domain.1.0)) / (q(domain.1.1) - q(domain.1.0)),
        );
        let column: Vec<Vec<Q>> = (0..=p)
            .map(|i| bernstein_sum(&exact[i * (qd + 1)..(i + 1) * (qd + 1)], &sv))
            .collect();
        let e = bernstein_sum(&column, &su);
        let (val, w) = s.value_at(u, v).unwrap();
        for (d, x) in [val[0], val[1], w].into_iter().enumerate() {
            assert!(
                holds(x, &e[d], 1e-10),
                "trial {trial} ({u}, {v}) {d}: {x:?}"
            );
        }
    }
}

/// Claim: a sub-span must lie strictly ordered inside the domain, with finite ends, and a parameter
/// inside it; anything else is refused by kind.
#[test]
fn spans_and_parameters_outside_the_domain_are_refused() {
    let mut rng = Lcg(65);
    let (_, c) = piece(&mut rng, (0.0, 1.0));
    for span in [
        (0.5, 0.5),
        (0.6, 0.4),
        (-0.1, 0.5),
        (0.5, 1.1),
        (f64::NAN, 0.5),
        (0.25, f64::NAN),
        (f64::NEG_INFINITY, 0.5),
        (0.5, f64::INFINITY),
    ] {
        assert_eq!(c.restrict(span), Err(NurbsError::KnotOutside), "{span:?}");
    }
    assert!(c.restrict((0.0, 1.0)).is_ok());
    for t in [-1e-300, 1.0 + f64::EPSILON, f64::NAN] {
        assert_eq!(c.value_at(t), Err(NurbsError::ParameterOutside), "{t}");
    }
    let (_, _, s) = patch(&mut rng, ((0.0, 1.0), (0.0, 1.0)));
    let whole = (0.0, 1.0);
    for (u, v) in [
        ((0.5, 0.5), whole),
        (whole, (0.7, 0.2)),
        ((-0.5, 0.5), whole),
        (whole, (0.5, 2.0)),
    ] {
        assert_eq!(
            s.restrict(u, v),
            Err(NurbsError::KnotOutside),
            "{u:?} {v:?}"
        );
    }
    for (u, v) in [(-0.1, 0.5), (0.5, -0.1), (1.1, 0.5), (0.5, 1.1)] {
        assert_eq!(
            s.value_at(u, v),
            Err(NurbsError::ParameterOutside),
            "{u} {v}"
        );
    }
}

/// Claim: the union of two boxes holds both, side by side, and has the same bits in either order;
/// a side that is not finite is refused.
#[test]
fn a_union_of_boxes_holds_both_in_either_order() {
    use geom_nurbs::Bounds;
    let a = Bounds {
        lo: [0.0, -1.0, 2.0],
        hi: [1.0, 3.0, 2.5],
    };
    let b = Bounds {
        lo: [-0.5, 0.0, 2.0],
        hi: [0.5, 4.0, 2.25],
    };
    let both = Bounds {
        lo: [-0.5, -1.0, 2.0],
        hi: [1.0, 4.0, 2.5],
    };
    assert_eq!(a.union(&b).unwrap(), both);
    assert_eq!(b.union(&a).unwrap(), both);
    assert_eq!(a.union(&a).unwrap(), a);
    let open = Bounds {
        lo: [f64::NEG_INFINITY, 0.0, 0.0],
        hi: [0.0, 0.0, 0.0],
    };
    assert_eq!(a.union(&open), Err(NurbsError::NotFinite));
    assert_eq!(open.union(&a), Err(NurbsError::NotFinite));
}
