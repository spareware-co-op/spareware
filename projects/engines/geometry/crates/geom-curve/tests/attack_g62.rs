//! Attacks on piece G62's claims, against exact rational arithmetic: a bulge polyline's
//! enclosures hold its exact rational point (its segments are rational functions of the stored
//! doubles); its NURBS form lies within its stated deviation of it, at the same parameter, for
//! semicircles and major arcs too; an offset of a circle stays on the circle of radius r − d; and a
//! composite's enclosures at its joints and inside are its segments'. A call may refuse an attack;
//! it may never return a bound that is false.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_curve::{
    Basis, BulgePolyline, Circle, Composite, CurveError, CurveKind, DerivativeBound, Frame, Line,
    Offset, OffsetLaw, OnSurface, ParamCurve, ParamMap, Station, SurfaceRef, Trimmed,
};
use geom_math::{Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Basis as Knots, Curve, Limit};
use geom_run::{Budget, WorkMeter};

type Q = RBig;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn q(x: f64) -> Q {
    RBig::try_from(x).unwrap()
}

fn qi(k: i64) -> Q {
    RBig::from(IBig::from(k))
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
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
}

/// The exact point of a bulge segment from p₀ to p₂ with bulge b at the local parameter v.
fn bulge_point(p0: [f64; 2], p2: [f64; 2], b: f64, v: &Q) -> [Q; 2] {
    let (b, one, two) = (q(b), qi(1), qi(2));
    let s = &one + &b * &b;
    let k = &one - &b * &b;
    let w = &k / &s;
    let d = [q(p2[0]) - q(p0[0]), q(p2[1]) - q(p0[1])];
    let perp = [d[1].clone(), -d[0].clone()];
    let u = &one - v;
    let (b0, b1, b2) = (&u * &u, &two * &u * v, v * v);
    let den = &b0 + &b1 * &w + &b2;
    core::array::from_fn(|i| {
        let mid = (q(p0[i]) + q(p2[i])) / &two;
        let h1 = (&k * &mid + &b * &perp[i]) / &s;
        (&b0 * q(p0[i]) + &b1 * &h1 + &b2 * q(p2[i])) / &den
    })
}

fn inside(e: Interval, x: &Q) -> bool {
    q(e.lo()) <= *x && *x <= q(e.hi())
}

/// A rational NURBS curve's exact point at u (de Casteljau on its Bezier piece; every form here
/// is Bezier pieces with doubled knots).
fn nurbs_point(c: &Curve<2>, u: &Q) -> [Q; 2] {
    let p = c.degree();
    let mut distinct = c.basis().knots().to_vec();
    distinct.dedup();
    let mut j = 0;
    while j + 2 < distinct.len() && q(distinct[j + 1]) < *u {
        j += 1;
    }
    let (a, b) = (q(distinct[j]), q(distinct[j + 1]));
    let v = (u - &a) / (&b - &a);
    let w = c.weights();
    let mut pts: Vec<([Q; 2], Q)> = (0..=p)
        .map(|i| {
            let k = j * p + i;
            let h = c.homogeneous()[k];
            ([q(h[0]), q(h[1])], w.map_or(qi(1), |w| q(w[k])))
        })
        .collect();
    let vm = qi(1) - &v;
    for r in 1..=p {
        for i in 0..=p - r {
            let (x, y) = (pts[i].clone(), pts[i + 1].clone());
            pts[i] = (
                [&vm * &x.0[0] + &v * &y.0[0], &vm * &x.0[1] + &v * &y.0[1]],
                &vm * &x.1 + &v * &y.1,
            );
        }
    }
    let (h, w) = &pts[0];
    [&h[0] / w, &h[1] / w]
}

/// Claim: every enclosure of a bulge polyline holds its exact point, including major arcs,
/// semicircles and parameters a rounding away from a vertex, and on the reversed polyline.
#[test]
fn bulge_enclosures_hold_the_exact_point() {
    let mut rng = Lcg(626);
    let m = &mut meter();
    for trial in 0..200 {
        let p0 = [rng.range(-50.0, 50.0), rng.range(-50.0, 50.0)];
        let p2 = [rng.range(-50.0, 50.0), rng.range(-50.0, 50.0)];
        let b = match trial % 4 {
            0 => 1.0,
            1 => rng.range(-20.0, 20.0),
            2 => rng.range(-1.0, 1.0),
            _ => -1.0,
        };
        let poly = BulgePolyline::new(vec![p0, p2], vec![b], None, false).unwrap();
        for t in [
            0.0,
            1.0,
            0.5,
            1.0f64.next_down(),
            f64::MIN_POSITIVE,
            rng.unit(),
        ] {
            let e = poly.enclose(t, Limit::FromRight, 0, m).unwrap()[0];
            let x = bulge_point(p0, p2, b, &q(t));
            assert!(
                inside(e[0], &x[0]) && inside(e[1], &x[1]),
                "trial {trial}, b = {b}, t = {t}"
            );
        }
        // The NURBS form within its deviation, at the same parameter.
        let n = poly.to_nurbs(m).unwrap();
        let dev = q(n.deviation);
        for k in 0..=20 {
            let t = f64::from(k) / 20.0;
            let a = nurbs_point(&n.value.curve, &q(t));
            let x = bulge_point(p0, p2, b, &q(t));
            let d2 = (&a[0] - &x[0]) * (&a[0] - &x[0]) + (&a[1] - &x[1]) * (&a[1] - &x[1]);
            assert!(
                d2 <= &dev * &dev,
                "trial {trial}, b = {b}, t = {t}: deviation {}",
                n.deviation
            );
        }
    }
}

/// Claim: an offset of a circle by d lies on the circle of radius r − d about the same centre,
/// every enclosure meeting that circle.
#[test]
fn an_offset_circle_stays_on_its_circle() {
    let mut rng = Lcg(627);
    let m = &mut meter();
    for _ in 0..50 {
        let c = [rng.range(-20.0, 20.0), rng.range(-20.0, 20.0)];
        let r = rng.range(2.0, 30.0);
        let d = rng.range(-r * 0.9, r * 0.9);
        let circle = Circle::new(c, r, Frame::standard().unwrap(), (0.0, 2.0)).unwrap();
        let o = Offset::new(
            Trimmed::whole(Basis::Circle(circle)).unwrap(),
            OffsetLaw::Planar { distance: d },
        )
        .unwrap();
        let rr = q(r) - q(d);
        for k in 0..=24 {
            let t = 2.0 * f64::from(k) / 24.0;
            let e = o.enclose(t, Limit::FromRight, 0, m).unwrap()[0];
            // The box's nearest and farthest squared distances from the centre bracket (r − d)².
            let (mut near, mut far) = (qi(0), qi(0));
            for i in 0..2 {
                let (lo, hi, ci) = (q(e[i].lo()), q(e[i].hi()), q(c[i]));
                let (a, b) = (&lo - &ci, &hi - &ci);
                let n = if lo <= ci && ci <= hi {
                    qi(0)
                } else if &a * &a < &b * &b {
                    &a * &a
                } else {
                    &b * &b
                };
                let f = if &a * &a > &b * &b { &a * &a } else { &b * &b };
                near += n;
                far += f;
            }
            let target = &rr * &rr;
            assert!(near <= target && target <= far, "t = {t}");
        }
    }
}

/// Claim: a composite's enclosure is its segment's, at joints from either side and inside.
#[test]
fn composite_enclosures_are_their_segments() {
    let m = &mut meter();
    let a = Line::new([0.0, 0.0], [3.0, 0.0], (0.0, 1.0)).unwrap();
    let poly = BulgePolyline::new(
        vec![[3.0, 0.0], [3.0, 4.0], [0.0, 4.0]],
        vec![0.5, -1.0],
        None,
        false,
    )
    .unwrap();
    let c = Composite::new(vec![
        Trimmed::whole(Basis::Line(a)).unwrap(),
        Trimmed::whole(Basis::Bulge(poly.clone())).unwrap(),
    ])
    .unwrap();
    // Both ends are the exact point (3, 0); the bulge's enclosure there widens by an ulp or two
    // (an interval product with an exact 0 is not cut back to 0), so the certified gap is that.
    assert!(c.gaps()[0] < 1e-14, "{:?}", c.gaps());
    for (t, local, side) in [
        (1.0, 0.0, Limit::FromRight),
        (1.5, 0.5, Limit::FromRight),
        (2.0, 1.0, Limit::FromLeft),
        (3.0, 2.0, Limit::FromLeft),
    ] {
        let e = c.enclose(t, side, 1, m).unwrap();
        let s = poly.enclose(local, side, 1, m).unwrap();
        assert_eq!(e, s, "t = {t}");
    }
    let e = c.enclose(1.0, Limit::FromLeft, 0, m).unwrap()[0];
    assert!(inside(e[0], &qi(3)) && inside(e[1], &qi(0)));
}

/// The exact first derivative of a bulge segment at the local parameter v: (A′W − AW′)/W².
fn bulge_derivative(p0: [f64; 2], p2: [f64; 2], b: f64, v: &Q) -> [Q; 2] {
    let (b, one, two) = (q(b), qi(1), qi(2));
    let s = &one + &b * &b;
    let k = &one - &b * &b;
    let w = &k / &s;
    let d = [q(p2[0]) - q(p0[0]), q(p2[1]) - q(p0[1])];
    let perp = [d[1].clone(), -d[0].clone()];
    let u = &one - v;
    let ww = &u * &u + &two * &u * v * &w + v * v;
    let dw = -(&two * &u) + &two * (&u - v) * &w + &two * v;
    core::array::from_fn(|i| {
        let mid = (q(p0[i]) + q(p2[i])) / &two;
        let h1 = (&k * &mid + &b * &perp[i]) / &s;
        let a = &u * &u * q(p0[i]) + &two * &u * v * &h1 + v * v * q(p2[i]);
        let da = -(&two * &u) * q(p0[i]) + &two * (&u - v) * &h1 + &two * v * q(p2[i]);
        (&da * &ww - &a * &dw) / (&ww * &ww)
    })
}

/// A NURBS curve's exact point at u, by de Boor's algorithm on its homogeneous points (u past an
/// end extends the end piece).
fn deboor<const D: usize>(c: &Curve<D>, u: &Q) -> [Q; D] {
    let p = c.degree();
    let t: Vec<Q> = c.basis().knots().iter().map(|&x| q(x)).collect();
    let n = c.homogeneous().len() - 1;
    let mut k = p;
    while k < n && t[k + 1] <= *u {
        k += 1;
    }
    let w = c.weights();
    let mut d: Vec<(Vec<Q>, Q)> = (0..=p)
        .map(|j| {
            let i = j + k - p;
            (
                c.homogeneous()[i].iter().map(|&x| q(x)).collect(),
                w.map_or(qi(1), |w| q(w[i])),
            )
        })
        .collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let (lo, hi) = (&t[j + k - p], &t[j + 1 + k - r]);
            let a = (u - lo) / (hi - lo);
            let b = qi(1) - &a;
            let (x, y) = (d[j - 1].clone(), d[j].clone());
            d[j] = (
                x.0.iter().zip(&y.0).map(|(x, y)| &b * x + &a * y).collect(),
                &b * &x.1 + &a * &y.1,
            );
        }
    }
    let (h, w) = &d[p];
    core::array::from_fn(|i| &h[i] / w)
}

fn dist2<const D: usize>(a: &[Q; D], b: &[Q; D]) -> Q {
    a.iter()
        .zip(b)
        .fold(qi(0), |s, (x, y)| s + (x - y) * (x - y))
}

/// A segment's exact point at its own parameter.
type Exact<'a> = &'a dyn Fn(&Q) -> [Q; 2];

/// Checks a composite's NURBS form against the composite exactly, at the parameter its map gives
/// (`exact[k]` is segment k's exact point at its own parameter), and returns the stated deviation.
fn joins_within_deviation(c: &Composite<2>, exact: &[Exact<'_>], m: &mut WorkMeter<'_>) -> f64 {
    let n = c.to_nurbs(m).unwrap();
    let ParamMap::Pieces(pieces) = &n.value.map else {
        panic!("a composite's map is by pieces")
    };
    let dev = q(n.deviation);
    for (k, piece) in pieces.iter().enumerate() {
        let (a, b) = (q(piece.local.0), q(piece.local.1));
        let (s0, s1) = (q(piece.start), q(piece.end));
        for i in 0..=16 {
            let t = &s0 + (&s1 - &s0) * qi(i) / qi(16);
            let local = &a + (&t - &s0) * (&b - &a) / (&s1 - &s0);
            let u = &local + q(piece.shift);
            let d2 = dist2(&deboor(&n.value.curve, &u), &exact[k](&local));
            assert!(
                d2 <= &dev * &dev,
                "segment {k}, t = {t}: deviation {}",
                n.deviation
            );
        }
    }
    n.deviation
}

/// The x axis, trimmed to [lo, hi].
fn axis(lo: f64, hi: f64) -> Trimmed<2> {
    let line = Line::new([0.0, 0.0], [1.0, 0.0], (f64::NEG_INFINITY, f64::INFINITY)).unwrap();
    Trimmed::new(Basis::Line(line), (lo, hi)).unwrap()
}

/// Claim: a leaf answers with its kind's own point.
#[test]
fn a_leaf_answers_with_its_kind() {
    let line = Line::new([3.0, 4.0], [1.0, -2.0], (0.0, 1.0)).unwrap();
    let leaf = Basis::Line(line.clone());
    for t in [0.0, 0.25, 1.0] {
        assert_eq!(
            leaf.point(t, Limit::FromRight).unwrap(),
            line.point(t, Limit::FromRight).unwrap()
        );
    }
}

/// Claim: a bulge polyline refuses a single vertex, widths that are not finite and negative
/// widths, keeps what it is given, and scales its widths with a similarity.
#[test]
fn bulge_polylines_refuse_what_they_cannot_be() {
    let v = vec![[0.0, 0.0], [1.0, 0.0]];
    assert_eq!(
        BulgePolyline::new(vec![[0.0, 0.0]], vec![], None, false).err(),
        Some(CurveError::Degenerate)
    );
    for (w, e) in [
        ((f64::NAN, 1.0), CurveError::NotFinite),
        ((1.0, f64::INFINITY), CurveError::NotFinite),
        ((-1.0, 1.0), CurveError::Degenerate),
        ((1.0, -0.5), CurveError::Degenerate),
    ] {
        assert_eq!(
            BulgePolyline::new(v.clone(), vec![0.5], Some(vec![w]), false).err(),
            Some(e),
            "{w:?}"
        );
    }
    let open = BulgePolyline::new(v.clone(), vec![0.5], Some(vec![(1.0, 2.0)]), false).unwrap();
    assert_eq!(open.vertices(), &v[..]);
    assert!(!open.is_closed());
    let closed = BulgePolyline::new(
        vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        vec![0.0, 0.3, 0.0],
        None,
        true,
    )
    .unwrap();
    assert!(closed.is_closed());
    let m = &mut meter();
    let grow = TypedTransform::from(Transform::scale(3.0, 3.0, 1.0).unwrap());
    let CurveKind::Bulge(g) = open.transformed(&grow, m).unwrap().value else {
        panic!("a similarity keeps a polyline")
    };
    assert_eq!(g.widths(), Some(&[(3.0, 6.0)][..]));
}

/// Claim: at a vertex a polyline answers for the segment on the side asked for, and a parameter a
/// rounding from a vertex for the segment holding it, also on a reversed polyline, whose
/// parameters start at minus its length so the offsets from its start round.
#[test]
fn bulge_vertices_take_the_side_asked_for() {
    let m = &mut meter();
    let p = BulgePolyline::new(
        vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 2.0]],
        vec![0.0; 4],
        None,
        false,
    )
    .unwrap();
    let slope = |c: &BulgePolyline<2>, t: f64, side| {
        let mut d = [[0.0; 2]; 2];
        c.derivatives(t, side, &mut d).unwrap();
        d[1]
    };
    assert_eq!(slope(&p, 1.0, Limit::FromLeft), [1.0, 0.0]);
    assert_eq!(slope(&p, 1.0, Limit::FromRight), [0.0, 1.0]);
    let CurveKind::Bulge(r) = p.reversed(m).unwrap().value else {
        panic!("a polyline reverses to one")
    };
    assert_eq!(r.domain(), (-4.0, 0.0));
    // −1 is the vertex between (1, 1) → (1, 0) and (1, 0) → (0, 0); t + 4 rounds to 3 from below
    // and from above.
    assert_eq!(
        slope(&r, (-1.0f64).next_down(), Limit::FromRight),
        [0.0, -1.0]
    );
    assert_eq!(slope(&r, (-1.0f64).next_up(), Limit::FromLeft), [-1.0, 0.0]);
}

/// Claim: a polyline's derivative bound holds the exact derivative over a span where the weight's
/// derivative is 0 at one end only (a semicircle, from its start to its middle), far from the
/// origin so every term counts.
#[test]
fn bulge_derivative_bounds_keep_every_term() {
    let m = &mut meter();
    let (p0, p2) = ([100.0, 0.0], [102.0, 0.0]);
    let poly = BulgePolyline::new(vec![p0, p2], vec![1.0], None, false).unwrap();
    let DerivativeBound::Bounded(b) = poly.derivative_bound((0.0, 0.5), 1, m).unwrap() else {
        panic!("a polyline's derivatives are bounded")
    };
    for k in 0..=8 {
        let v = qi(k) / qi(16);
        let d = bulge_derivative(p0, p2, 1.0, &v);
        for (i, x) in d.iter().enumerate() {
            assert!(
                q(b.lo[i]) <= *x && *x <= q(b.hi[i]),
                "v = {v}, coordinate {i}: {b:?}"
            );
        }
    }
}

/// Claim: a composite refuses no segments, a segment too short for its start's precision and a
/// start past the largest double, and keeps the segments it is given.
#[test]
fn composites_refuse_starts_they_cannot_hold() {
    assert_eq!(
        Composite::<2>::new(vec![]).err(),
        Some(CurveError::Degenerate)
    );
    // 10¹⁶ + 0.5 rounds back to 10¹⁶; 1.7·10³⁰⁸ + 10³⁰⁸ overflows.
    assert_eq!(
        Composite::starting_at(vec![axis(0.0, 0.5)], 1e16).err(),
        Some(CurveError::Degenerate)
    );
    assert_eq!(
        Composite::starting_at(vec![axis(0.0, 1e308)], 1.7e308).err(),
        Some(CurveError::Degenerate)
    );
    let c = Composite::new(vec![axis(0.0, 1.0), axis(1.0, 3.0)]).unwrap();
    assert_eq!(c.segments(), &[axis(0.0, 1.0), axis(1.0, 3.0)]);
}

/// Claim: a composite's derivatives scale by the ratio of a segment's parameter length to its
/// share of the composite's: 3/4 where 10¹⁶ + 3 rounds to 10¹⁶ + 4.
#[test]
fn composite_derivatives_scale_by_the_parameter_ratio() {
    let m = &mut meter();
    let c = Composite::starting_at(vec![axis(0.0, 3.0)], 1e16).unwrap();
    assert_eq!(c.starts(), &[1e16, 1e16 + 4.0]);
    let mut o = [[0.0; 2]; 2];
    c.derivatives(1e16 + 2.0, Limit::FromRight, &mut o).unwrap();
    assert_eq!(o, [[1.5, 0.0], [0.75, 0.0]]);
    let e = c.enclose(1e16 + 2.0, Limit::FromRight, 1, m).unwrap();
    assert!(inside(e[1][0], &(qi(3) / qi(4))));
}

/// Claim: a composite's derivative enclosure holds the exact ratio's multiple also where the
/// ratio is within a rounding of 1, so its enclosure has 1 at one end.
#[test]
fn composite_derivatives_near_a_ratio_of_one_hold_the_exact_value() {
    let mut rng = Lcg(629);
    let m = &mut meter();
    for _ in 0..300 {
        let len = rng.range(0.5, 2.0);
        let start = len * rng.range(0.0, 0.01);
        let c = Composite::starting_at(vec![axis(0.0, len)], start).unwrap();
        let (s0, s1) = (c.starts()[0], c.starts()[1]);
        let r = q(len) / (q(s1) - q(s0));
        let e = c
            .enclose(f64::midpoint(s0, s1), Limit::FromRight, 1, m)
            .unwrap();
        assert!(inside(e[1][0], &r), "{len} from {start}");
    }
}

/// Claim: a composite refuses the splits it cannot make: at its end, and where the cut rounds onto
/// a segment's end (a segment at 10¹⁶, whose parameters step by 2).
#[test]
fn composite_splits_refuse_cuts_on_a_segment_end() {
    let m = &mut meter();
    let c = Composite::new(vec![axis(1e16, 1e16 + 4.0)]).unwrap();
    assert_eq!(c.split(4.0, m).err(), Some(CurveError::ParameterOutside));
    assert_eq!(c.split(1.0, m).err(), Some(CurveError::Degenerate));
    assert_eq!(c.split(3.0, m).err(), Some(CurveError::Degenerate));
    let (l, r) = c.split(2.0, m).unwrap();
    assert_eq!(
        (l.value.domain(), r.value.domain()),
        ((0.0, 2.0), (2.0, 4.0))
    );
}

/// Claim: a composite's NURBS form holds its deviation exactly, with a rational segment whose
/// weights are rescaled to meet the joint (its first weight 3 after a line's 1), a segment of two
/// Bezier pieces, and a first segment whose parameters start at 2.
#[test]
fn joins_rescale_weights_and_hold_their_deviation() {
    let m = &mut meter();
    let line = Line::new([0.0, 0.0], [1.0, 0.0], (0.0, 1.0)).unwrap();
    let rational = Curve::new_rational(
        Knots::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap(),
        &[[1.0, 0.0], [2.0, 1.0], [3.0, 0.0]],
        &[3.0, 1.0, 2.0],
    )
    .unwrap();
    let two = Curve::new(
        Knots::new(2, vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]).unwrap(),
        &[[3.0, 0.0], [4.0, -1.0], [5.0, 1.0], [6.0, 0.0]],
    )
    .unwrap();
    let c = Composite::new(vec![
        Trimmed::whole(Basis::Line(line)).unwrap(),
        Trimmed::whole(Basis::Nurbs(rational.clone())).unwrap(),
        Trimmed::whole(Basis::Nurbs(two.clone())).unwrap(),
    ])
    .unwrap();
    let on_line = |v: &Q| [v.clone(), qi(0)];
    let on_rational = |v: &Q| deboor(&rational, v);
    let on_two = |v: &Q| deboor(&two, v);
    let dev = joins_within_deviation(&c, &[&on_line, &on_rational, &on_two], m);
    assert!(dev < 1e-14, "{dev}");
    let shifted = Curve::new(
        Knots::new(2, vec![2.0, 2.0, 2.0, 3.0, 3.0, 3.0]).unwrap(),
        &[[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]],
    )
    .unwrap();
    let c = Composite::new(vec![
        Trimmed::whole(Basis::Nurbs(shifted.clone())).unwrap(),
        Trimmed::whole(Basis::Line(
            Line::new([2.0, 0.0], [1.0, 0.0], (0.0, 1.0)).unwrap(),
        ))
        .unwrap(),
    ])
    .unwrap();
    let on_shifted = |v: &Q| deboor(&shifted, v);
    let on_line = |v: &Q| [qi(2) + v, qi(0)];
    let dev = joins_within_deviation(&c, &[&on_shifted, &on_line], m);
    assert!(dev < 1e-14, "{dev}");
}

/// Claim: a NURBS segment with a break inside it (an interior knot of full multiplicity) joins
/// with that gap in its deviation.
#[test]
fn a_break_inside_a_segment_counts_in_the_deviation() {
    let m = &mut meter();
    let broken = Curve::new(
        Knots::new(2, vec![0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0]).unwrap(),
        &[
            [0.0, 0.0],
            [1.0, 1.0],
            [2.0, 0.0],
            [2.0, 0.5],
            [3.0, 1.0],
            [4.0, 0.0],
        ],
    )
    .unwrap();
    let c = Composite::new(vec![Trimmed::whole(Basis::Nurbs(broken)).unwrap()]).unwrap();
    let n = c.to_nurbs(m).unwrap();
    assert!(n.deviation >= 0.5, "{}", n.deviation);
}

/// Claim: where a segment's knots round when shifted into the joined form, the
/// reparametrisation is in the deviation, checked exactly (a speed of 2^20, and every control
/// point exact, so the rounding alone moves the point).
#[test]
fn rounded_joint_knots_count_in_the_deviation() {
    let mut rng = Lcg(628);
    let m = &mut meter();
    let s = 1_048_576.0;
    let whole = (f64::NEG_INFINITY, f64::INFINITY);
    for trial in 0..200 {
        let lo0 = rng.range(0.0, 1.0);
        let hi0 = lo0 + rng.range(0.1, 1.0);
        // Half the trials shift by an exact difference (within a factor of 2), so only one end of
        // the shifted piece may round.
        let lo1 = if trial % 2 == 0 {
            hi0 * rng.range(0.6, 1.5)
        } else {
            rng.range(0.05, 3.0)
        };
        // Within a factor of 2 of lo1, so the segment's end point is exact.
        let hi1 = lo1 * rng.range(1.1, 1.9);
        let x = Line::new([0.0, 0.0], [s, 0.0], whole).unwrap();
        let y = Line::new([s * hi0, -s * lo1], [0.0, s], whole).unwrap();
        let c = Composite::new(vec![
            Trimmed::new(Basis::Line(x), (lo0, hi0)).unwrap(),
            Trimmed::new(Basis::Line(y), (lo1, hi1)).unwrap(),
        ])
        .unwrap();
        let on_x = |v: &Q| [q(s) * v, qi(0)];
        let on_y = |v: &Q| [q(s * hi0), q(-s * lo1) + q(s) * v];
        joins_within_deviation(&c, &[&on_x, &on_y], m);
    }
}

/// Claim: a join whose knots would collapse (pieces narrower than the knots' precision so far
/// along) is refused.
#[test]
fn joins_whose_knots_collapse_are_refused() {
    let m = &mut meter();
    let long = Line::new([0.0, 0.0], [1e-16, 0.0], (0.0, 1e16)).unwrap();
    let e = long.point(1e16, Limit::FromLeft).unwrap();
    let spline = Curve::new(
        Knots::new(1, vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0]).unwrap(),
        &[
            e,
            [e[0] + 1.0, 1.0],
            [e[0] + 2.0, 0.0],
            [e[0] + 3.0, 1.0],
            [e[0] + 4.0, 0.0],
        ],
    )
    .unwrap();
    let c = Composite::new(vec![
        Trimmed::whole(Basis::Line(long)).unwrap(),
        Trimmed::whole(Basis::Nurbs(spline)).unwrap(),
    ])
    .unwrap();
    assert_eq!(c.starts(), &[0.0, 1e16, 1e16 + 4.0]);
    assert_eq!(c.to_nurbs(m).err(), Some(CurveError::Degenerate));
}

/// Claim: an offset's NURBS form is cut at its basis's knots and vertices and at the stations
/// inside its domain, with eight Taylor pieces to each smooth interval where that is enough.
#[test]
fn offset_forms_are_cut_at_their_breaks() {
    let m = &mut meter();
    let knots_of = |o: &Offset<2>, m: &mut WorkMeter<'_>| {
        let mut k = o.to_nurbs(m).unwrap().value.curve.basis().knots().to_vec();
        k.dedup();
        k
    };
    let spline = Curve::new(
        Knots::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.3, 0.6, 1.0, 1.0, 1.0, 1.0]).unwrap(),
        &[
            [0.0, 0.0],
            [1.0, 0.2],
            [2.0, 0.3],
            [3.0, 0.2],
            [4.0, 0.1],
            [5.0, 0.2],
        ],
    )
    .unwrap();
    let o = Offset::new(
        Trimmed::whole(Basis::Nurbs(spline)).unwrap(),
        OffsetLaw::Planar { distance: 0.1 },
    )
    .unwrap();
    let k = knots_of(&o, m);
    assert!(k.contains(&0.3) && k.contains(&0.6), "{k:?}");
    let poly = BulgePolyline::new(
        vec![[0.0, 0.0], [2.0, 0.0], [3.0, 1.0], [3.0, 3.0]],
        vec![0.2, -0.3, 0.1],
        None,
        false,
    )
    .unwrap();
    let o = Offset::new(
        Trimmed::whole(Basis::Bulge(poly)).unwrap(),
        OffsetLaw::Planar { distance: 0.1 },
    )
    .unwrap();
    let k = knots_of(&o, m);
    assert!(k.contains(&1.0) && k.contains(&2.0), "{k:?}");
    // Stations before, at and after the ends, and one inside; the offset is linear in each piece,
    // so eight pieces each are enough: 17 distinct knots.
    let st = |at, lateral| Station {
        at,
        lateral,
        vertical: 0.0,
    };
    let line = Line::new([0.0, 0.0], [1.0, 0.0], (0.0, 1.0)).unwrap();
    let o = Offset::new(
        Trimmed::whole(Basis::Line(line)).unwrap(),
        OffsetLaw::Distances(vec![
            st(-1.0, 0.0),
            st(0.0, 0.5),
            st(0.25, 1.0),
            st(1.0, 1.0),
            st(5.0, 0.0),
        ]),
    )
    .unwrap();
    let k = knots_of(&o, m);
    assert_eq!((k[0], k[k.len() - 1], k.len()), (0.0, 1.0, 17), "{k:?}");
    assert!(k.contains(&0.25), "{k:?}");
}

/// Claim: an offset by distances takes the slope of the piece on the side asked for at a station,
/// and of the piece holding a parameter between stations.
#[test]
fn offsets_by_distances_take_the_side_asked_for() {
    let line = Line::new([0.0, 0.0], [1.0, 0.0], (0.0, 1.0)).unwrap();
    let st = |at, lateral| Station {
        at,
        lateral,
        vertical: 0.0,
    };
    let o = Offset::new(
        Trimmed::whole(Basis::Line(line)).unwrap(),
        OffsetLaw::Distances(vec![st(0.0, 0.0), st(0.5, 1.0), st(1.0, 1.0)]),
    )
    .unwrap();
    for (t, side, want) in [
        (0.5, Limit::FromLeft, [1.0, 2.0]),
        (0.5, Limit::FromRight, [1.0, 0.0]),
        (0.25, Limit::FromRight, [1.0, 2.0]),
        (0.25, Limit::FromLeft, [1.0, 2.0]),
        (0.75, Limit::FromLeft, [1.0, 0.0]),
    ] {
        let mut d = [[0.0; 2]; 2];
        o.derivatives(t, side, &mut d).unwrap();
        assert!(
            (d[1][0] - want[0]).abs() < 1e-12 && (d[1][1] - want[1]).abs() < 1e-12,
            "t = {t}, {side:?}: {:?}",
            d[1]
        );
    }
}

/// Claim: where an offset's normal vanishes (the basis's speed underflows near a cusp) its
/// derivative bound is unbounded and its point bound still holds, the vertical offset included.
#[test]
fn offsets_where_the_normal_vanishes_still_bound_their_points() {
    let m = &mut meter();
    let cusp = Curve::new(
        Knots::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap(),
        &[[0.0; 3], [0.0; 3], [1.0, 0.0, 0.0]],
    )
    .unwrap();
    let o = Offset::new(
        Trimmed::whole(Basis::Nurbs(cusp)).unwrap(),
        OffsetLaw::Distances(vec![Station {
            at: 0.0,
            lateral: 0.5,
            vertical: 0.75,
        }]),
    )
    .unwrap();
    let span = (0.0, 1e-200);
    assert_eq!(
        o.derivative_bound(span, 1, m).unwrap(),
        DerivativeBound::Unbounded
    );
    // Every point there is 0.75 above the basis, which lies on z = 0.
    let b = o.bound(span, m).unwrap();
    assert!(b.lo[2] <= 0.75 && 0.75 <= b.hi[2], "{b:?}");
}

/// Claim: a curve on a surface runs over its pcurve's interval, and the whole of a ray is refused
/// as unbounded.
#[test]
fn trimmed_intervals_are_the_domains() {
    let line = Line::new([0.0, 0.0], [1.0, 2.0], (f64::NEG_INFINITY, f64::INFINITY)).unwrap();
    let uv = Trimmed::new(Basis::Line(line), (2.0, 3.0)).unwrap();
    let cache = Curve::new(
        Knots::new(1, vec![2.0, 2.0, 3.0, 3.0]).unwrap(),
        &[[0.0; 3], [1.0, 2.0, 0.0]],
    )
    .unwrap();
    let on = OnSurface::new(uv, SurfaceRef(3), cache, 0.0).unwrap();
    assert_eq!(on.domain(), (2.0, 3.0));
    let ray = Line::new([0.0, 0.0], [1.0, 0.0], (0.0, f64::INFINITY)).unwrap();
    assert_eq!(
        Trimmed::whole(Basis::Line(ray)).err(),
        Some(CurveError::Unbounded)
    );
}
