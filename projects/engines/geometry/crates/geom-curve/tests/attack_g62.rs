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
    Basis, BulgePolyline, Circle, Composite, Frame, Line, Offset, OffsetLaw, ParamCurve, Trimmed,
};
use geom_num::interval::Interval;
use geom_nurbs::{Curve, Limit};
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
