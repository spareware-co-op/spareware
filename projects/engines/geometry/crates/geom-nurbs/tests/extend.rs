//! The natural extension of curves and surfaces (piece G253) against the exact rational
//! reference: the source is kept, point for point and in exact value over its own domain; the
//! extension is the end span's polynomial continued, within the deviation it reports; the joint
//! is smooth; a surface's lines are the curve extension of each line; and every documented error.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::many_single_char_names,
    clippy::doc_markdown,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::range_plus_one,
    clippy::disallowed_methods,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod common;

use common::{Lcg, Q, cox_de_boor, knots, q, qi, zero};
use geom_nurbs::{Basis, Curve, Direction, End, Limit, NurbsError, Surface};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Clamped curves of dimension 3, degrees 1 to 5, rational and not.
fn curves(seed: u64) -> Vec<Curve<3>> {
    let mut rng = Lcg(seed);
    let mut out = Vec::new();
    for p in [1, 2, 3, 4, 5] {
        for kind in 0..3 {
            let n = p + rng.below(6);
            let basis = Basis::new(p, knots(&mut rng, p, n, true, kind)).unwrap();
            let points: Vec<[f64; 3]> = (0..basis.count())
                .map(|_| {
                    [
                        rng.unit() * 20.0 - 10.0,
                        rng.unit() * 1e-3,
                        rng.unit() * 500.0,
                    ]
                })
                .collect();
            out.push(Curve::new(basis.clone(), &points).unwrap());
            // Weights near 1, so a short continuation keeps them positive.
            let w: Vec<f64> = (0..basis.count()).map(|_| 0.9 + 0.2 * rng.unit()).collect();
            out.push(Curve::new_rational(basis, &points, &w).unwrap());
        }
    }
    out
}

/// The homogeneous control points of `curve`, exactly.
fn exact_homs(curve: &Curve<3>) -> Vec<[Q; 4]> {
    curve
        .homogeneous()
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let w = curve.weights().map_or(1.0, |ws| ws[i]);
            [q(r[0]), q(r[1]), q(r[2]), q(w)]
        })
        .collect()
}

/// C(u) exactly, from the curve's stored doubles, inside its domain.
fn exact_point(curve: &Curve<3>, u: f64, limit: Limit) -> [Q; 3] {
    let basis = curve.basis();
    let p = basis.degree();
    let table = cox_de_boor(basis.knots(), p, u, limit);
    let pts = exact_homs(curve);
    let mut a = [zero(), zero(), zero()];
    let mut w = zero();
    for (i, row) in pts.iter().enumerate() {
        let n = &table[p][i];
        if *n == zero() {
            continue;
        }
        for d in 0..3 {
            a[d] += n * &row[d];
        }
        w += n * &row[3];
    }
    a.map(|x| x / &w)
}

/// The end span's polynomial (span `k` of `curve`) at any `u`, exactly: de Boor's triangle on
/// that span, which continues it past the span's ends.
fn span_polynomial(curve: &Curve<3>, k: usize, u: f64) -> [Q; 3] {
    let p = curve.degree();
    let kn: Vec<Q> = curve.basis().knots().iter().map(|&x| q(x)).collect();
    let pts = exact_homs(curve);
    let uq = q(u);
    let mut d: Vec<[Q; 4]> = (0..=p).map(|j| pts[j + k - p].clone()).collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = j + k - p;
            let alpha = (&uq - &kn[i]) / (&kn[i + p + 1 - r] - &kn[i]);
            let beta = qi(1) - &alpha;
            let row: [Q; 4] = std::array::from_fn(|c| &alpha * &d[j][c] + &beta * &d[j - 1][c]);
            d[j] = row;
        }
    }
    let w = d[p][3].clone();
    [&d[p][0] / &w, &d[p][1] / &w, &d[p][2] / &w]
}

/// The length of the span at `which` end.
fn end_span_length(curve: &Curve<3>, which: End) -> f64 {
    let k = curve.basis().knots();
    let p = curve.degree();
    match which {
        End::High => k[k.len() - 1] - k[k.len() - p - 2],
        End::Low => k[p + 1] - k[0],
    }
}

fn distance2(a: &[Q; 3], b: &[Q; 3]) -> Q {
    let mut s = zero();
    for d in 0..3 {
        let x = &a[d] - &b[d];
        s += &x * &x;
    }
    s
}

#[test]
fn the_source_is_kept_and_the_extension_is_its_end_span_continued() {
    let mut rng = Lcg(0x0253_0001);
    for curve in curves(0x0253_0002) {
        let p = curve.degree();
        let (lo, hi) = curve.basis().domain();
        let width = hi - lo;
        for which in [End::High, End::Low] {
            // A polynomial continues any length; a rational one a part of its end span's.
            let scale = if curve.is_rational() {
                end_span_length(&curve, which)
            } else {
                width
            };
            let by = scale * (0.05 + 0.6 * rng.unit());
            let changed = curve.extend(which, by, &mut meter()).unwrap();
            let new = &changed.value;
            let dev2 = q(changed.deviation) * q(changed.deviation);
            let (nlo, nhi) = new.basis().domain();
            // The source's points and knots, unchanged, then p new points and the new end knot.
            let (old_k, new_k) = (curve.basis().knots(), new.basis().knots());
            assert_eq!(new.homogeneous().len(), curve.homogeneous().len() + p);
            assert_eq!(new_k.len(), old_k.len() + p);
            match which {
                End::High => {
                    assert_eq!((nlo, nhi), (lo, hi + by));
                    assert_eq!(
                        &new.homogeneous()[..curve.homogeneous().len()],
                        curve.homogeneous()
                    );
                    assert_eq!(&new_k[..old_k.len() - 1], &old_k[..old_k.len() - 1]);
                    assert!(new_k[old_k.len() - 1..].iter().all(|&k| k == hi + by));
                }
                End::Low => {
                    assert_eq!((nlo, nhi), (lo - by, hi));
                    assert_eq!(&new.homogeneous()[p..], curve.homogeneous());
                    assert_eq!(&new_k[p + 1..], &old_k[1..]);
                    assert!(new_k[..p + 1].iter().all(|&k| k == lo - by));
                }
            }
            // Over the source's domain: the same doubles, and the same exact value.
            for j in 0..=8 {
                let u = if j == 8 {
                    hi
                } else {
                    lo + width * f64::from(j) / 8.0
                };
                let limit = if u == hi {
                    Limit::FromLeft
                } else {
                    Limit::FromRight
                };
                assert_eq!(
                    new.point(u, limit).unwrap().map(f64::to_bits),
                    curve.point(u, limit).unwrap().map(f64::to_bits),
                    "p {p}, {which:?}, u {u}"
                );
                assert_eq!(exact_point(new, u, limit), exact_point(&curve, u, limit));
            }
            // Past the end: the end span's polynomial, within the reported deviation.
            let end_span = match which {
                End::High => curve.basis().span(hi, Limit::FromLeft).unwrap(),
                End::Low => curve.basis().span(lo, Limit::FromRight).unwrap(),
            };
            for j in 1..=6 {
                let u = match which {
                    End::High => {
                        if j == 6 {
                            nhi
                        } else {
                            hi + by * f64::from(j) / 6.0
                        }
                    }
                    End::Low => {
                        if j == 6 {
                            nlo
                        } else {
                            lo - by * f64::from(j) / 6.0
                        }
                    }
                };
                let limit = if u == nhi {
                    Limit::FromLeft
                } else {
                    Limit::FromRight
                };
                let got = exact_point(new, u, limit);
                let want = span_polynomial(&curve, end_span, u);
                assert!(
                    distance2(&got, &want) <= dev2,
                    "p {p}, {which:?}, u {u}: off the continuation by more than {}",
                    changed.deviation
                );
            }
        }
    }
}

#[test]
fn the_joint_is_smooth() {
    let mut rng = Lcg(0x0253_0003);
    for curve in curves(0x0253_0004) {
        let p = curve.degree();
        let (_, hi) = curve.basis().domain();
        let by = 0.3 * end_span_length(&curve, End::High) * (0.5 + rng.unit());
        let new = curve.extend(End::High, by, &mut meter()).unwrap().value;
        let mut left = vec![[0.0; 3]; p + 1];
        let mut right = vec![[0.0; 3]; p + 1];
        new.derivs(hi, Limit::FromLeft, &mut left).unwrap();
        new.derivs(hi, Limit::FromRight, &mut right).unwrap();
        for k in 0..=p {
            let size = left[k]
                .iter()
                .chain(&right[k])
                .fold(1.0_f64, |m, x| m.max(x.abs()));
            for d in 0..3 {
                assert!(
                    (left[k][d] - right[k][d]).abs() <= 1e-6 * size,
                    "p {p}: derivative {k} jumps at the joint: {:?} and {:?}",
                    left[k],
                    right[k]
                );
            }
        }
    }
}

#[test]
fn a_surface_extends_line_by_line() {
    let mut rng = Lcg(0x0253_0005);
    for (p, qd) in [(1, 2), (3, 3), (2, 4)] {
        let bu = Basis::new(p, knots(&mut rng, p, p + 3, true, 1)).unwrap();
        let bv = Basis::new(qd, knots(&mut rng, qd, qd + 2, true, 2)).unwrap();
        let count = bu.count() * bv.count();
        let points: Vec<[f64; 3]> = (0..count)
            .map(|_| [rng.unit() * 4.0, rng.unit() * 4.0, rng.unit() * 4.0])
            .collect();
        let w: Vec<f64> = (0..count).map(|_| 0.9 + 0.2 * rng.unit()).collect();
        for s in [
            Surface::new(bu.clone(), bv.clone(), &points).unwrap(),
            Surface::new_rational(bu.clone(), bv.clone(), &points, &w).unwrap(),
        ] {
            for dir in [Direction::U, Direction::V] {
                for which in [End::Low, End::High] {
                    let changed = s.extend(dir, which, 0.05, &mut meter()).unwrap();
                    let e = &changed.value;
                    let (rows, cols) = (s.basis_u().count(), s.basis_v().count());
                    let (erows, ecols) = (e.basis_u().count(), e.basis_v().count());
                    let lines = if dir == Direction::U { cols } else { rows };
                    for l in 0..lines {
                        // Line l of the source as a curve, extended; and line l of the result.
                        let (len, elen) = if dir == Direction::U {
                            (rows, erows)
                        } else {
                            (cols, ecols)
                        };
                        let at = |k: usize, c: usize| {
                            if dir == Direction::U {
                                k * c + l
                            } else {
                                l * c + k
                            }
                        };
                        let line: Vec<[f64; 3]> =
                            (0..len).map(|k| s.homogeneous()[at(k, cols)]).collect();
                        let lw: Option<Vec<f64>> = s
                            .weights()
                            .map(|ws| (0..len).map(|k| ws[at(k, cols)]).collect());
                        let basis = if dir == Direction::U {
                            bu.clone()
                        } else {
                            bv.clone()
                        };
                        let curve = Curve::from_homogeneous(basis, line, lw).unwrap();
                        let c = curve.extend(which, 0.05, &mut meter()).unwrap().value;
                        for k in 0..elen {
                            assert_eq!(e.homogeneous()[at(k, ecols)], c.homogeneous()[k]);
                            if let (Some(a), Some(b)) = (e.weights(), c.weights()) {
                                assert_eq!(a[at(k, ecols)], b[k]);
                            }
                        }
                    }
                    assert!(changed.deviation.is_finite() && changed.deviation >= 0.0);
                }
            }
        }
    }
}

#[test]
fn every_documented_refusal() {
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]).unwrap();
    let pts = [
        [0.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [2.0, 0.0, 0.0],
        [3.0, 1.0, 0.0],
    ];
    let curve = Curve::new(basis, &pts).unwrap();
    for by in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e-300] {
        assert!(
            matches!(
                curve.extend(End::High, by, &mut meter()),
                Err(NurbsError::KnotOutside)
            ),
            "{by}"
        );
    }
    // So long that the de Casteljau parameter overflows.
    assert!(matches!(
        curve.extend(End::High, f64::MAX, &mut meter()),
        Err(NurbsError::NotFinite)
    ));
    // Not clamped at the end asked for.
    let open = Curve::new(
        Basis::new(2, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap(),
        &pts,
    )
    .unwrap();
    for which in [End::Low, End::High] {
        assert!(matches!(
            open.extend(which, 1.0, &mut meter()),
            Err(NurbsError::NotClamped)
        ));
    }
    // A rational line whose weight falls to 0.1: continued its length again, the weight would
    // be 1 − 0.9·2 < 0.
    let line = Curve::new_rational(
        Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap(),
        &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
        &[1.0, 0.1],
    )
    .unwrap();
    assert!(matches!(
        line.extend(End::High, 1.0, &mut meter()),
        Err(NurbsError::Weight { .. })
    ));
    assert!(line.extend(End::High, 0.05, &mut meter()).is_ok());
    // The meter is charged first.
    let mut tight = WorkMeter::new(Budget::new(1, 1));
    assert!(matches!(
        curve.extend(End::High, 1.0, &mut tight),
        Err(NurbsError::Stopped(_))
    ));
}

/// A rational line from the origin, weight 1, to (1, 2, 3), weight `w`, on [0, 1]; or, for a
/// low end, the same weights the other way round, so the weight falls past the end extended.
fn falling_line(w: f64, which: End) -> Curve<3> {
    let weights = match which {
        End::High => [1.0, w],
        End::Low => [w, 1.0],
    };
    Curve::new_rational(
        Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap(),
        &[[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]],
        &weights,
    )
    .unwrap()
}

#[test]
fn a_weight_continued_close_to_zero_is_covered_by_the_deviation() {
    // Continued by e, the line's weight is 1 − (1 + e)(1 − w): with e just short of where that
    // vanishes, the new point lies hundreds of times farther out than the old ones and its weight
    // is a near cancellation, so the weight's rounding moves it far more than the coordinates'
    // rounding does. The deviation must cover that too.
    for (w, e) in [
        (0.5005, 0.998),
        (0.25, 0.3329),
        (0.75, 2.9937),
        (0.6, 1.4985),
    ] {
        for which in [End::High, End::Low] {
            let line = falling_line(w, which);
            let changed = line.extend(which, e, &mut meter()).unwrap();
            assert!(changed.deviation >= 0.0, "{w}, {which:?}");
            let new = &changed.value;
            let (nlo, nhi) = new.basis().domain();
            let (lo, hi) = line.basis().domain();
            let (u, limit, span) = match which {
                End::High => (nhi, Limit::FromLeft, line.basis().span(hi, Limit::FromLeft)),
                End::Low => (
                    nlo,
                    Limit::FromRight,
                    line.basis().span(lo, Limit::FromRight),
                ),
            };
            let span = span.unwrap();
            let got = exact_point(new, u, limit);
            let want = span_polynomial(&line, span, u);
            assert!(
                distance2(&got, &want) <= q(changed.deviation) * q(changed.deviation),
                "{w}, {which:?}: off the continuation by more than {}",
                changed.deviation
            );
        }
    }
}

#[test]
fn a_weight_continued_to_exactly_zero_is_refused_at_its_point() {
    // Weights 1 and 1/2, continued by 1: the new weight is 1 − 2·(1/2) = 0 exactly, at the new
    // point: index 2 past a high end, 0 past a low one.
    for (which, index) in [(End::High, 2), (End::Low, 0)] {
        let got = falling_line(0.5, which).extend(which, 1.0, &mut meter());
        assert!(
            matches!(got, Err(NurbsError::Weight { index: i }) if i == index),
            "{which:?}: {got:?}"
        );
    }
}
