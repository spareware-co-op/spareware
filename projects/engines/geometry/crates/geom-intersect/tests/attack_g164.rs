//! Attacks on G164: a line with each closed form at sizes where the answer can be read off, helices
//! lying on cylinders, grazing curves, and the done-when count: every root of 10,000 curves through
//! Bézier patches, the roots built in, found or flagged.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_curve::{CurveKind, Frame, Helix, HelixAxes, Line};
use geom_intersect::IntersectError;
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::result::{
    Coincidence, Coverage, Intersection, Item, PointContact, StepsInterval, Test, Unresolved,
};
use geom_num::grid::Steps;
use geom_nurbs::{Basis, Curve, Surface};
use geom_run::{Budget, CancelToken, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind, Torus};
use geom_test::{Seed, SplitMix64};

/// The meeting, within a budget well past the heaviest case here (a curve bent as the bowl is,
/// some 4.5 million units): a closed form lost to the engine on an infinite line stops instead of
/// running on.
fn meet(c: &CurveKind<3>, s: &SurfaceKind) -> Intersection {
    let mut m = WorkMeter::new(Budget::of_work(50_000_000));
    curve_surface(c, s, &mut m).unwrap()
}

fn line(o: [f64; 3], d: [f64; 3]) -> CurveKind<3> {
    CurveKind::Line(Line::infinite(o, d).unwrap())
}

fn segment(a: [f64; 3], b: [f64; 3]) -> CurveKind<3> {
    CurveKind::Line(Line::segment(a, b).unwrap().value)
}

fn frame() -> Frame<3> {
    Frame::standard().unwrap()
}

const ROUND: (f64, f64) = (0.0, 2.0);

fn plane() -> SurfaceKind {
    let side = (-100.0, 100.0);
    SurfaceKind::Plane(
        Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], (side, side)).unwrap(),
    )
}

fn sphere(r: f64) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere::new([0.0; 3], r, frame(), (ROUND, (-0.5, 0.5))).unwrap())
}

fn cylinder(r: f64) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], r, frame(), (ROUND, (-50.0, 50.0))).unwrap())
}

fn torus(major: f64, minor: f64) -> SurfaceKind {
    SurfaceKind::Torus(Torus::new([0.0; 3], major, minor, frame(), (ROUND, ROUND)).unwrap())
}

fn points(r: &Intersection) -> Vec<[f64; 3]> {
    let mut out: Vec<[f64; 3]> = r
        .items()
        .iter()
        .filter(|i| matches!(i, Item::Point { .. }))
        .map(|i| {
            let s = i.at().spans();
            [0, 1, 2].map(|k| s[k].lo() * 0.5 + s[k].hi() * 0.5)
        })
        .collect();
    out.sort_by(|a, b| a[0].total_cmp(&b[0]));
    out
}

fn contacts(r: &Intersection) -> Vec<PointContact> {
    r.items()
        .iter()
        .filter_map(|i| match i {
            Item::Point { contact, .. } => Some(*contact),
            _ => None,
        })
        .collect()
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} and {b}");
}

fn is_empty(r: &Intersection) {
    assert!(r.items().is_empty(), "{:?}", r.items());
    assert_eq!(r.coverage(), Coverage::TopologyCertified);
}

/// The one item's box holds each of `spans` and reaches past it by less than 1e-9.
fn spans_hold(r: &Intersection, spans: [(f64, f64); 3]) {
    let [item] = r.items() else {
        panic!("{:?}", r.items());
    };
    for (got, (lo, hi)) in item.at().spans().iter().zip(spans) {
        assert!(
            got.lo() <= lo && hi <= got.hi() && lo - got.lo() < 1e-9 && got.hi() - hi < 1e-9,
            "{got:?} against {lo} to {hi}"
        );
    }
}

fn flagged(r: &Intersection) -> Vec<Unresolved> {
    r.items()
        .iter()
        .filter_map(|i| match i {
            Item::Flagged { why, .. } => Some(*why),
            _ => None,
        })
        .collect()
}

#[test]
fn a_line_meets_a_plane_once() {
    let r = meet(&line([3.0, 4.0, 10.0], [0.0, 0.0, -2.0]), &plane());
    assert_eq!(contacts(&r), [PointContact::Crossing]);
    let p = points(&r);
    near(p[0][0], 5.0);
    near(p[0][1], 3.0);
    near(p[0][2], 4.0);
    // A segment that stops short, and one that ends on it.
    is_empty(&meet(&segment([3.0, 4.0, 10.0], [3.0, 4.0, 5.0]), &plane()));
    let r = meet(&segment([3.0, 4.0, 10.0], [3.0, 4.0, 0.0]), &plane());
    assert_eq!(contacts(&r), [PointContact::BoundaryTouch]);
    // Past the plane's domain, and short of it, on either side.
    is_empty(&meet(&line([500.0, 4.0, 1.0], [0.0, 0.0, 1.0]), &plane()));
    is_empty(&meet(&line([-500.0, 4.0, 1.0], [0.0, 0.0, 1.0]), &plane()));
    is_empty(&meet(&segment([3.0, 4.0, 5.0], [3.0, 4.0, 10.0]), &plane()));
    // A segment that starts on it.
    let r = meet(&segment([3.0, 4.0, 0.0], [3.0, 4.0, 10.0]), &plane());
    assert_eq!(contacts(&r), [PointContact::BoundaryTouch]);
    near(points(&r)[0][0], 0.0);
}

#[test]
fn a_parallel_line_misses_a_plane_or_lies_in_it() {
    is_empty(&meet(&line([0.0, 0.0, 1.0], [1.0, 2.0, 0.0]), &plane()));
    // Lying in it: the overlap's box holds the whole segment, from either end.
    for (a, b) in [([0.0; 3], [5.0, 3.0, 0.0]), ([5.0, 3.0, 0.0], [0.0; 3])] {
        let r = meet(&segment(a, b), &plane());
        assert!(
            matches!(
                r.items(),
                [Item::Overlap {
                    proof: Coincidence::Identical,
                    ..
                }]
            ),
            "{:?}",
            r.items()
        );
        spans_hold(&r, [(0.0, 1.0), (0.0, 5.0), (0.0, 3.0)]);
    }
    // Parallel by the least the doubles can say: a tolerance would call it so, and it is not.
    let tiny = 2.0_f64.powi(-40);
    let r = meet(&line([0.0, 0.0, 1.0], [1.0, 0.0, -tiny]), &plane());
    is_empty(&r);
}

#[test]
fn a_tilted_plane_is_met_on_its_own_vectors() {
    let side = (-1000.0, 1000.0);
    let p = SurfaceKind::Plane(
        Plane::new(
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            (side, side),
        )
        .unwrap(),
    );
    let r = meet(&line([1.0, 4.0, 1.0], [1.0, -1.0, 1.0]), &p);
    let q = points(&r);
    assert_eq!(q.len(), 1);
    let [t, u, v] = q[0];
    near(1.0 + t, 1.0 + u);
    near(4.0 - t, 1.0 + u + v);
    near(1.0 + t, 1.0 + v);
}

/// A plane through (1, 2, 3) on (2, 1, 1) and (1, 3, 2): no vector of it along an axis, so every
/// coordinate of its exact sides counts.
fn leaning() -> SurfaceKind {
    let side = (-1000.0, 1000.0);
    SurfaceKind::Plane(
        Plane::new(
            [1.0, 2.0, 3.0],
            [2.0, 1.0, 1.0],
            [1.0, 3.0, 2.0],
            (side, side),
        )
        .unwrap(),
    )
}

#[test]
fn a_line_in_a_leaning_plane_lies_in_it() {
    // From c + a along a + b: (u, v) = (1, 0) + t·(1, 1).
    for (a, b) in [
        ([3.0, 3.0, 4.0], [6.0, 7.0, 7.0]),
        ([6.0, 7.0, 7.0], [3.0, 3.0, 4.0]),
    ] {
        let r = meet(&segment(a, b), &leaning());
        assert!(
            matches!(r.items(), [Item::Overlap { .. }]),
            "{:?}",
            r.items()
        );
        spans_hold(&r, [(0.0, 1.0), (1.0, 2.0), (0.0, 1.0)]);
    }
    // One step off it, parallel: nothing.
    is_empty(&meet(
        &segment([4.0, 3.0, 4.0], [7.0, 7.0, 7.0]),
        &leaning(),
    ));
}

#[test]
fn a_line_meets_a_sphere() {
    let s = sphere(5.0);
    let r = meet(&line([3.0, 0.0, -9.0], [0.0, 0.0, 1.0]), &s);
    assert_eq!(contacts(&r), [PointContact::Crossing; 2]);
    let p = points(&r);
    near(p[0][0], 5.0);
    near(p[1][0], 13.0);
    near(p[0][1], 0.0);
    let turn = 4.0_f64.atan2(3.0) / core::f64::consts::PI;
    near(p[0][2], -turn);
    near(p[1][2], turn);
    let r = meet(&line([5.0, 0.0, -9.0], [0.0, 0.0, 1.0]), &s);
    assert_eq!(contacts(&r), [PointContact::IsolatedTangent]);
    is_empty(&meet(&line([6.0, 0.0, -9.0], [0.0, 0.0, 1.0]), &s));
    // Through the poles: every u.
    let r = meet(&line([0.0, 0.0, -9.0], [0.0, 0.0, 1.0]), &s);
    let sp = r.items()[0].at().spans();
    near(sp[1].lo(), 0.0);
    near(sp[1].hi(), 2.0);
}

#[test]
fn a_line_meets_a_cylinder() {
    let c = cylinder(5.0);
    let r = meet(&line([-9.0, 3.0, 2.0], [1.0, 0.0, 0.0]), &c);
    assert_eq!(contacts(&r), [PointContact::Crossing; 2]);
    let p = points(&r);
    near(p[0][0], 5.0);
    near(p[1][0], 13.0);
    near(p[0][2], 2.0);
    let r = meet(&line([-9.0, 5.0, 2.0], [1.0, 0.0, 0.0]), &c);
    assert_eq!(contacts(&r), [PointContact::IsolatedTangent]);
    is_empty(&meet(&line([-9.0, 6.0, 2.0], [1.0, 0.0, 0.0]), &c));
    // Along the axis, off the surface: never. On a ruling: all of it.
    is_empty(&meet(&line([1.0, 1.0, 0.0], [0.0, 0.0, 1.0]), &c));
    let r = meet(&segment([5.0, 0.0, -9.0], [5.0, 0.0, 9.0]), &c);
    assert!(
        matches!(r.items(), [Item::Overlap { .. }]),
        "{:?}",
        r.items()
    );
    spans_hold(&r, [(0.0, 1.0), (0.0, 0.0), (-9.0, 9.0)]);
    let r = meet(&segment([5.0, 0.0, 9.0], [5.0, 0.0, -9.0]), &c);
    spans_hold(&r, [(0.0, 1.0), (0.0, 0.0), (-9.0, 9.0)]);
    // Past the cylinder's height.
    is_empty(&meet(&line([-9.0, 3.0, 60.0], [1.0, 0.0, 0.0]), &c));
}

#[test]
fn a_line_meets_a_cone() {
    // Radius 2 + v about z, for v from −10 to 10: the apex is at z = −2.
    let c =
        SurfaceKind::Cone(Cone::new([0.0; 3], 2.0, 1.0, frame(), (ROUND, (-10.0, 10.0))).unwrap());
    let r = meet(&line([-9.0, 0.0, 1.0], [1.0, 0.0, 0.0]), &c);
    assert_eq!(contacts(&r), [PointContact::Crossing; 2]);
    let p = points(&r);
    near(p[0][0], 6.0);
    near(p[1][0], 12.0);
    near(p[0][2], 1.0);
    near(p[0][1], 1.0);
    near(p[1][1], 0.0);
    // Below the apex the radius is negative and e(u) runs the other way.
    let p = points(&meet(&line([-9.0, 0.0, -5.0], [1.0, 0.0, 0.0]), &c));
    assert_eq!(p.len(), 2);
    near(p[0][1], 0.0);
    near(p[1][1], 1.0);
    // Parallel to a ruling: one root.
    let p = points(&meet(&line([0.0, 0.0, 0.0], [1.0, 0.0, 1.0]), &c));
    assert_eq!(p.len(), 1);
}

#[test]
fn a_line_meets_a_torus() {
    let t = torus(10.0, 2.0);
    // In the equator's plane: four, at ±8 and ±12.
    let r = meet(&line([-20.0, 0.0, 0.0], [1.0, 0.0, 0.0]), &t);
    let p = points(&r);
    assert_eq!(p.len(), 4, "{:?}", r.items());
    // The engine's enclosures reach less than a step.
    for (found, root) in p.iter().zip([8.0, 12.0, 28.0, 32.0]) {
        assert!((found[0] - root).abs() < 1.0, "{found:?} and {root}");
    }
    // Through the hole: nothing, by the exact chain.
    is_empty(&meet(&line([0.0, 0.0, -20.0], [0.0, 0.0, 1.0]), &t));
    is_empty(&meet(&line([0.0, 13.0, -20.0], [0.0, 0.0, 1.0]), &t));
    // Tangent on top of the tube, a double root at each side of the ring, by the exact chain.
    let r = meet(&line([-20.0, 0.0, 2.0], [1.0, 0.0, 0.0]), &t);
    assert_eq!(
        contacts(&r),
        [PointContact::IsolatedTangent; 2],
        "{:?}",
        r.items()
    );
}

#[test]
fn a_helix_on_a_coaxial_cylinder_lies_in_it() {
    let axes = HelixAxes::Frame {
        frame: frame(),
        radius: 5.0,
        pitch: 2.0,
    };
    let h = CurveKind::Helix(Helix::new([0.0; 3], axes, (0.0, 8.0)).unwrap());
    let r = meet(&h, &cylinder(5.0));
    assert!(
        matches!(
            r.items(),
            [Item::Overlap {
                proof: Coincidence::Identical,
                ..
            }]
        ),
        "{:?}",
        r.items()
    );
    // It rises half its pitch for each unit of t: from 0 to 8.
    spans_hold(&r, [(0.0, 8.0), (0.0, 2.0), (0.0, 8.0)]);
    // A helix inside a larger cylinder never reaches it.
    is_empty(&meet(&h, &cylinder(6.0)));
    // Moved off the axis by one step it crosses, and the engine finds it.
    let moved = CurveKind::Helix(Helix::new([1.0, 0.0, 0.0], axes, (0.0, 8.0)).unwrap());
    let r = meet(&moved, &cylinder(5.0));
    assert_ne!(r.items().len(), 0);
    assert!(!matches!(r.items()[0], Item::Overlap { .. }));
}

#[test]
fn a_helix_crosses_a_plane() {
    let axes = HelixAxes::Frame {
        frame: frame(),
        radius: 5.0,
        pitch: 2.0,
    };
    let h = CurveKind::Helix(Helix::new([0.0, 0.0, -3.5], axes, (0.0, 8.0)).unwrap());
    let p = points(&meet(&h, &plane()));
    assert_eq!(p.len(), 1);
    near(p[0][0], 3.5);
}

fn bezier(points: &[[f64; 3]]) -> CurveKind<3> {
    CurveKind::Nurbs(Curve::new(basis(points.len() - 1), points).unwrap())
}

fn basis(p: usize) -> Basis {
    let mut knots = vec![0.0; p + 1];
    knots.extend(vec![1.0; p + 1]);
    Basis::new(p, knots).unwrap()
}

/// The patch z = 1000·(1 − u² − v²) over [−1, 1]², a thousand steps wide each way, biquadratic.
fn bowl() -> SurfaceKind {
    let s = 1000.0;
    // Bernstein coefficients of 1 − u² − v² on [−1, 1]²: in each variable, 1, −1, 1 for u².
    let f = [1.0, -1.0, 1.0];
    let mut net = Vec::new();
    for (i, a) in f.iter().enumerate() {
        for (j, b) in f.iter().enumerate() {
            net.push([
                s * (i as f64 - 1.0),
                s * (j as f64 - 1.0),
                s * (1.0 - a - b),
            ]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(basis(2), basis(2), &net).unwrap())
}

#[test]
fn a_curve_goes_through_a_patch_once_or_twice() {
    let down = bezier(&[[100.0, 100.0, 2000.0], [100.0, 100.0, -2000.0]]);
    let p = points(&meet(&down, &bowl()));
    assert_eq!(p.len(), 1);
    let flat = bezier(&[[-2000.0, 0.0, 500.0], [2000.0, 0.0, 500.0]]);
    let r = meet(&flat, &bowl());
    assert_eq!(contacts(&r), [PointContact::Crossing; 2], "{:?}", r.items());
    let above = bezier(&[[-2000.0, 0.0, 1500.0], [2000.0, 0.0, 1500.0]]);
    is_empty(&meet(&above, &bowl()));
}

#[test]
fn a_grazing_curve_is_flagged_with_its_band() {
    // Tangent to the top of the bowl, z = 1000 at the middle.
    let graze = bezier(&[[-2000.0, 0.0, 1000.0], [2000.0, 0.0, 1000.0]]);
    let r = meet(&graze, &bowl());
    assert!(
        r.items().iter().any(|i| matches!(
            i,
            Item::Flagged {
                why: Unresolved::TangentBand { .. },
                ..
            }
        )),
        "{:?}",
        r.items()
    );
    assert_eq!(r.coverage(), Coverage::CompleteOutsideFlagged);
    // A curve bent as the surface is cannot be told from it at second order.
    let same = bezier(&[[-1000.0, 0.0, 0.0], [0.0, 0.0, 2000.0], [1000.0, 0.0, 0.0]]);
    let r = meet(&same, &bowl());
    assert!(
        r.items().iter().any(|i| matches!(
            i,
            Item::Flagged {
                why: Unresolved::HigherOrderContact | Unresolved::NearCoincident { .. },
                ..
            }
        )),
        "{:?}",
        r.items()
    );
}

#[test]
fn a_stop_stops() {
    let c = bezier(&[[100.0, 100.0, 2000.0], [100.0, 100.0, -2000.0]]);
    let token = CancelToken::new();
    token.cancel();
    let mut m = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    assert_eq!(
        curve_surface(&c, &bowl(), &mut m).unwrap_err(),
        IntersectError::Cancelled
    );
    let mut m = WorkMeter::new(Budget::new(3, 0));
    assert_eq!(
        curve_surface(&c, &bowl(), &mut m).unwrap_err(),
        IntersectError::BudgetExceeded
    );
}

/// A number in [0, 1).
fn unit(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 / 2.0_f64.powi(53)
}

fn times(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            out[i + j] += x * y;
        }
    }
    out
}

fn plus(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; a.len().max(b.len())];
    for (i, x) in a.iter().enumerate() {
        out[i] += x;
    }
    for (i, y) in b.iter().enumerate() {
        out[i] += y;
    }
    out
}

/// The Bernstein coefficients of the polynomial with these monomial coefficients, degree p.
fn bernstein(monomial: &[f64], p: usize) -> Vec<f64> {
    let choose = |n: usize, k: usize| -> f64 {
        (0..k).fold(1.0, |c, i| c * (n - i) as f64 / (i + 1) as f64)
    };
    (0..=p)
        .map(|j| {
            (0..=j.min(monomial.len() - 1))
                .map(|i| monomial[i] * choose(j, i) / choose(p, i))
                .sum()
        })
        .collect()
}

#[test]
fn no_root_is_lost_on_10_000_curves_through_patches() {
    // A patch z = a·u² + b·v² + c·u·v + e over [0, 1]², scaled, and a curve (t, g(t), z(t) − c·Π(t − rᵢ)),
    // which leaves the patch's graph exactly at the rᵢ: every rᵢ must lie in an item's box.
    let mut rng = SplitMix64::new(geom_test::seed::fixed("g164 pairs", Seed::DEFAULT));
    let scale = 1000.0;
    for pair in 0..10_000 {
        let n = 1 + (rng.next_u64() % 3) as usize;
        let mut roots: Vec<f64> = Vec::new();
        while roots.len() < n {
            let r = 0.05 + 0.9 * unit(&mut rng);
            if roots.iter().all(|x| (x - r).abs() > 0.05) {
                roots.push(r);
            }
        }
        let c = if pair % 2 == 0 { 1.0 } else { -1.0 } * (1.0 + 4.0 * unit(&mut rng));
        let (a, b, f, e) = (
            unit(&mut rng) - 0.5,
            unit(&mut rng) - 0.5,
            unit(&mut rng) - 0.5,
            unit(&mut rng),
        );
        // The patch's graph, on its Bernstein net of degree 2 by 2.
        let height = |i: usize, j: usize| {
            let (u, v) = (i as f64 * 0.5, j as f64 * 0.5);
            let bu = if i == 1 { 0.0 } else { u };
            let bv = if j == 1 { 0.0 } else { v };
            a * bu + b * bv + f * u * v + e
        };
        let mut net = Vec::new();
        for i in 0..3 {
            for j in 0..3 {
                net.push([
                    scale * i as f64 * 0.5,
                    scale * j as f64 * 0.5,
                    scale * height(i, j),
                ]);
            }
        }
        let patch = SurfaceKind::Nurbs(Surface::new(basis(2), basis(2), &net).unwrap());
        // v = g(t) = k + m·t, inside [0, 1] for t in [0, 1].
        let (k, m) = (0.1 + 0.4 * unit(&mut rng), 0.4 * unit(&mut rng));
        let g = [k, m];
        let graph = plus(
            &plus(&[e], &plus(&[0.0, 0.0, a], &times(&[b], &times(&g, &g)))),
            &times(&[0.0, f], &g),
        );
        let difference = roots.iter().fold(vec![c], |p, r| times(&p, &[-r, 1.0]));
        let z = plus(&graph, &difference);
        let degree = z.len().max(3) - 1;
        let (x, y, h) = (
            bernstein(&[0.0, scale], degree),
            bernstein(&times(&g, &[scale]), degree),
            bernstein(&times(&z, &[scale]), degree),
        );
        let points: Vec<[f64; 3]> = x
            .iter()
            .zip(&y)
            .zip(&h)
            .map(|((x, y), z)| [*x, *y, *z])
            .collect();
        let curve = bezier(&points);
        let r = meet(&curve, &patch);
        for root in &roots {
            let seen = r.items().iter().any(|i| {
                let s = i.at().spans();
                s[0].lo() - 1e-9 <= *root && *root <= s[0].hi() + 1e-9
            });
            assert!(seen, "pair {pair}: root {root} missed in {:?}", r.items());
        }
    }
}

#[test]
fn a_seam_is_no_boundary() {
    // Each closed kind's join at u = 0 (and the torus's at v = 0) is crossed like any other place.
    let round = |s: SurfaceKind, o: [f64; 3], count: usize| {
        let r = meet(&line(o, [1.0, 0.0, 0.0]), &s);
        assert_eq!(
            contacts(&r),
            vec![PointContact::Crossing; count],
            "{:?}",
            r.items()
        );
    };
    round(cylinder(5.0), [-9.0, 0.0, 2.0], 2);
    round(sphere(5.0), [-9.0, 0.0, 0.0], 2);
    let cone =
        SurfaceKind::Cone(Cone::new([0.0; 3], 2.0, 1.0, frame(), (ROUND, (-10.0, 10.0))).unwrap());
    round(cone, [-9.0, 0.0, 1.0], 2);
    round(torus(10.0, 2.0), [-20.0, 0.0, 0.0], 4);
    // The engine's crossings of a join too, each found once.
    let across = bezier(&[[-9.0, 0.0, 1.0], [0.0, 0.0, 1.0], [9.0, 0.0, 1.0]]);
    let r = meet(&across, &cylinder(5.0));
    assert_eq!(contacts(&r), [PointContact::Crossing; 2], "{:?}", r.items());
    let r = meet(
        &bezier(&[[-20.0, 0.0, 0.5], [0.0, 0.0, 0.5], [20.0, 0.0, 0.5]]),
        &torus(10.0, 2.0),
    );
    assert_eq!(contacts(&r), [PointContact::Crossing; 4], "{:?}", r.items());
    // Half a cylinder ends at u = 0 and u = 1: there a root is a touch, or flagged by the engine,
    // which cannot tell a root on a true end from one a rounding outside it.
    let half = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 5.0, frame(), ((0.0, 1.0), (-50.0, 50.0))).unwrap(),
    );
    let r = meet(&line([-9.0, 0.0, 2.0], [1.0, 0.0, 0.0]), &half);
    assert_eq!(contacts(&r), [PointContact::BoundaryTouch; 2]);
    let r = meet(&across, &half);
    assert_eq!(
        flagged(&r),
        [Unresolved::UndecidedAtFloor {
            test: Test::Uniqueness
        }; 2]
    );
}

#[test]
fn an_axis_off_the_origin_is_read_exactly() {
    // A cylinder and a torus about z through (3, 4, 0): a line tangent to each is a double root by
    // the exact signs, and one along a ruling lies in the cylinder.
    let c = SurfaceKind::Cylinder(
        Cylinder::new([3.0, 4.0, 0.0], 5.0, frame(), (ROUND, (-50.0, 50.0))).unwrap(),
    );
    let r = meet(&line([8.0, -20.0, 1.0], [0.0, 1.0, 0.0]), &c);
    assert_eq!(contacts(&r), [PointContact::IsolatedTangent]);
    let r = meet(&segment([8.0, 4.0, -9.0], [8.0, 4.0, 9.0]), &c);
    assert!(
        matches!(r.items(), [Item::Overlap { .. }]),
        "{:?}",
        r.items()
    );
    spans_hold(&r, [(0.0, 1.0), (0.0, 0.0), (-9.0, 9.0)]);
    let t = SurfaceKind::Torus(
        Torus::new([3.0, 4.0, 0.0], 10.0, 2.0, frame(), (ROUND, ROUND)).unwrap(),
    );
    let r = meet(&line([-20.0, 4.0, 2.0], [1.0, 0.0, 0.0]), &t);
    assert_eq!(contacts(&r), [PointContact::IsolatedTangent; 2]);
}

#[test]
fn a_helix_on_a_leaning_cylinder_off_the_origin_lies_in_it() {
    // About (1, 2, 2) through (3, 5, 7): every coordinate of the axes and the centres counts.
    let lean = Frame::new([2.0, -1.0, 0.0], [1.0, 2.0, 2.0]).unwrap();
    let axes = HelixAxes::Frame {
        frame: lean,
        radius: 5.0,
        pitch: 2.0,
    };
    let h = CurveKind::Helix(Helix::new([3.0, 5.0, 7.0], axes, (0.0, 8.0)).unwrap());
    let c = SurfaceKind::Cylinder(
        Cylinder::new([3.0, 5.0, 7.0], 5.0, lean, (ROUND, (-50.0, 50.0))).unwrap(),
    );
    let r = meet(&h, &c);
    assert!(
        matches!(
            r.items(),
            [Item::Overlap {
                proof: Coincidence::Identical,
                ..
            }]
        ),
        "{:?}",
        r.items()
    );
    spans_hold(&r, [(0.0, 8.0), (0.0, 2.0), (0.0, 8.0)]);
    // Axes (0, 1, 1) and (0, 1, −1) through one centre, one radius: they cross, never coincide.
    let a = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 1.0]).unwrap();
    let b = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, -1.0]).unwrap();
    let axes = HelixAxes::Frame {
        frame: a,
        radius: 5.0,
        pitch: 2.0,
    };
    let h = CurveKind::Helix(Helix::new([0.0; 3], axes, (0.0, 8.0)).unwrap());
    let c = SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 5.0, b, (ROUND, (-50.0, 50.0))).unwrap());
    let r = meet(&h, &c);
    assert!(
        r.items().iter().all(|i| !matches!(i, Item::Overlap { .. })),
        "{:?}",
        r.items()
    );
    assert_eq!(contacts(&r), [PointContact::Crossing; 8], "{:?}", r.items());
}

/// The bilinear patch z = 0 over [0, 1000]², flat.
fn flat_patch() -> SurfaceKind {
    let net = [
        [0.0, 0.0, 0.0],
        [0.0, 1000.0, 0.0],
        [1000.0, 0.0, 0.0],
        [1000.0, 1000.0, 0.0],
    ];
    SurfaceKind::Nurbs(Surface::new(basis(1), basis(1), &net).unwrap())
}

#[test]
fn a_curve_lying_in_a_patch_is_near_coincident_only_over_a_long_run() {
    // A run longer than 64 steps is a curve lying near the surface; 64 or fewer, a contact no order
    // tells apart. The segment's box is its control points', so its length is exact.
    let in_flat = |length: f64| {
        let r = meet(
            &bezier(&[[100.0, 500.0, 0.0], [100.0 + length, 500.0, 0.0]]),
            &flat_patch(),
        );
        assert_eq!(r.coverage(), Coverage::CompleteOutsideFlagged);
        flagged(&r)
    };
    let gap = StepsInterval::new(Steps(0), Steps(1)).unwrap();
    assert_eq!(in_flat(800.0), [Unresolved::NearCoincident { gap }]);
    assert_eq!(in_flat(64.0), [Unresolved::HigherOrderContact]);
    assert_eq!(in_flat(50.0), [Unresolved::HigherOrderContact]);
}

#[test]
fn grazing_is_banded_from_either_side() {
    let band = |c: &CurveKind<3>| {
        let r = meet(c, &bowl());
        assert_eq!(r.coverage(), Coverage::CompleteOutsideFlagged);
        flagged(&r)
    };
    let width = Steps(32);
    // A straight curve on the top, bending less than the bowl, and a parabola bending twice as
    // much, under it: the curvatures differ by the same amount, of the other sign.
    let straight = bezier(&[[-2000.0, 0.0, 1000.0], [2000.0, 0.0, 1000.0]]);
    assert_eq!(band(&straight), [Unresolved::TangentBand { width }]);
    let sharper = bezier(&[
        [-1000.0, 0.0, -1000.0],
        [0.0, 0.0, 3000.0],
        [1000.0, 0.0, -1000.0],
    ]);
    assert_eq!(band(&sharper), [Unresolved::TangentBand { width }]);
    // At a sphere's pole the surface has no normal: no order can be read.
    let pole = bezier(&[[-9.0, 0.0, 5.0], [9.0, 0.0, 5.0]]);
    assert_eq!(
        flagged(&meet(&pole, &sphere(5.0))),
        [Unresolved::HigherOrderContact]
    );
}

#[test]
fn a_crossing_before_a_graze_is_kept() {
    // Along the diagonal x = y = s, z = bowl + 10⁻⁶·(s + 500)(s − 500)²: through the bowl at
    // s = −500 (t = 1/4), touching it at s = 500 (t = 3/4), every parameter of the touch beyond
    // the crossing's.
    let s = [-1000.0, 2000.0];
    let bowl_z = plus(&[1000.0], &times(&[-0.002], &times(&s, &s)));
    let gap = times(
        &[1e-6],
        &times(
            &plus(&s, &[500.0]),
            &times(&plus(&s, &[-500.0]), &plus(&s, &[-500.0])),
        ),
    );
    let (x, z) = (bernstein(&s, 3), bernstein(&plus(&bowl_z, &gap), 3));
    let net: Vec<[f64; 3]> = x.iter().zip(&z).map(|(a, h)| [*a, *a, *h]).collect();
    let r = meet(&bezier(&net), &bowl());
    assert_eq!(contacts(&r), [PointContact::Crossing]);
    let p = points(&r);
    assert!((p[0][0] - 0.25).abs() < 1e-3, "{p:?}");
    // The band is as wide as the run of t the touch stands for, 105 steps: the second-order bound
    // on its own, the square root of 2·floor over the curvatures' gap, is far narrower.
    assert_eq!(
        flagged(&r),
        [Unresolved::TangentBand { width: Steps(105) }],
        "{:?}",
        r.items()
    );
}

#[test]
fn the_work_is_what_the_clipping_leaves() {
    // The projected polyhedron step only speeds the engine (12.6), so what it does shows in the
    // work: these are the counts with it, each a deterministic function of the input.
    let work = |c: &CurveKind<3>, s: &SurfaceKind| {
        let mut m = WorkMeter::new(Budget::of_work(100_000));
        curve_surface(c, s, &mut m).unwrap();
        m.used().work
    };
    let down = bezier(&[[100.0, 100.0, 2000.0], [100.0, 100.0, -2000.0]]);
    assert_eq!(work(&down, &bowl()), 363);
    let flat = bezier(&[[-2000.0, 0.0, 500.0], [2000.0, 0.0, 500.0]]);
    assert_eq!(work(&flat, &bowl()), 2042);
    let above = bezier(&[[-2000.0, 0.0, 1500.0], [2000.0, 0.0, 1500.0]]);
    assert_eq!(work(&above, &bowl()), 316);
    let tilted = bezier(&[
        [-900.0, -700.0, 1800.0],
        [300.0, 100.0, 0.0],
        [800.0, 900.0, -1500.0],
    ]);
    assert_eq!(work(&tilted, &bowl()), 2510);
    let graze = bezier(&[[-2000.0, 0.0, 1000.0], [2000.0, 0.0, 1000.0]]);
    assert_eq!(work(&graze, &bowl()), 15_032);
    let straight = segment([100.0, 100.0, 2000.0], [100.0, 100.0, -2000.0]);
    assert_eq!(work(&straight, &bowl()), 293);
    // The torus's quartic, by its own residual and reach.
    assert_eq!(
        work(&line([-20.0, 0.0, 0.0], [1.0, 0.0, 0.0]), &torus(10.0, 2.0)),
        35
    );
    let t = SurfaceKind::Torus(
        Torus::new([3.0, 4.0, 0.0], 10.0, 2.0, frame(), (ROUND, ROUND)).unwrap(),
    );
    assert_eq!(work(&line([-20.0, 4.0, 2.0], [1.0, 0.0, 0.0]), &t), 39);
    // A line of speed 2 reaches twice as far for each unit of t.
    let ring = torus(10.0, 2.0);
    assert_eq!(work(&line([-20.0, 0.0, 0.0], [2.0, 0.0, 0.0]), &ring), 35);
    assert_eq!(work(&line([-20.0, 0.0, 2.0], [2.0, 0.0, 0.0]), &ring), 39);
    // A patch of two pieces in u: each box is clipped by the piece that holds it.
    let down = bezier(&[[600.0, 100.0, 2000.0], [600.0, 100.0, -2000.0]]);
    assert_eq!(work(&down, &two_pieces()), 633);
    let across = bezier(&[[-2000.0, 0.0, 100.0], [2000.0, 0.0, 100.0]]);
    assert_eq!(work(&across, &two_pieces()), 12_188);
}

/// A bowl over [−1000, 1000]² of two biquadratic pieces in u, joined at u = 1/2.
fn two_pieces() -> SurfaceKind {
    let s = 1000.0;
    let u = Basis::new(2, vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for x in [-1.0, -0.5, 0.5, 1.0] {
        for y in [-1.0, 0.0, 1.0] {
            net.push([s * x, s * y, s * (0.5 - 0.5 * x * x - 0.5 * y * y)]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(u, basis(2), &net).unwrap())
}

#[test]
fn a_tangent_band_on_a_torus_is_as_wide_as_its_run() {
    // Radii that are not whole leave the tangency to the engine. On a ring of radius about 10^6 a
    // line along the top of its tube stays within a step of it for some 1,400 steps either side of
    // the touch, where no box can be split: a band as wide in steps as its run of t times the
    // line's speed, 2.
    let major = 1_000_000.5;
    let t =
        SurfaceKind::Torus(Torus::new([0.0; 3], major, 100.0, frame(), (ROUND, ROUND)).unwrap());
    let r = meet(&line([-5000.0, major, 100.0], [2.0, 0.0, 0.0]), &t);
    let bands: Vec<_> = r
        .items()
        .iter()
        .filter_map(|i| match i {
            Item::Flagged {
                at,
                why: Unresolved::TangentBand { width },
                ..
            } => Some((at.spans()[0], width.0)),
            _ => None,
        })
        .collect();
    assert!(!bands.is_empty(), "{:?}", r.items());
    for (t, width) in bands {
        let run = ((t.hi() - t.lo()) * 2.0).ceil() as u64;
        assert!(
            width == run || width == run + 1,
            "{width} for a run of {t:?}"
        );
        assert!(
            width >= 3,
            "{width}: too narrow to tell the speed's use apart"
        );
    }
}

#[test]
fn a_bound_on_a_domain_end_is_past_it() {
    // Every enclosure is rounded outward, so one whose bound lands exactly on a domain's end holds
    // a value strictly past it. A segment stopping 9 doubles short of the plane, and lines 5 doubles
    // above and below the cylinder's height, meet each enclosure's end exactly, and miss.
    let h = 1.0 + 9.0 * 2.0_f64.powi(-52);
    is_empty(&meet(
        &segment([3.0, 4.0, h], [3.0, 4.0, h - 1.0]),
        &plane(),
    ));
    for side in [-1.0, 1.0] {
        let z = side * (50.0 + 5.0 * 2.0_f64.powi(-47));
        is_empty(&meet(
            &line([-9.0, 3.0, z], [1.0, 0.0, 0.0]),
            &cylinder(5.0),
        ));
    }
}

#[test]
fn a_leaning_axis_is_read_on_all_its_coordinates() {
    // About (1, 2, 2) through (3, 5, 7), radius 6: the line through (7, 7, 3) = centre + (4, 2, −4)
    // along (2, −2, 1), square to both, touches it, by the exact signs.
    let lean = Frame::new([2.0, -1.0, 0.0], [1.0, 2.0, 2.0]).unwrap();
    let c = SurfaceKind::Cylinder(
        Cylinder::new([3.0, 5.0, 7.0], 6.0, lean, (ROUND, (-50.0, 50.0))).unwrap(),
    );
    let r = meet(&line([7.0, 7.0, 3.0], [2.0, -2.0, 1.0]), &c);
    assert_eq!(
        contacts(&r),
        [PointContact::IsolatedTangent],
        "{:?}",
        r.items()
    );
    // Along its axis, one radius out: a ruling.
    let r = meet(&segment([7.0, 7.0, 3.0], [8.0, 9.0, 5.0]), &c);
    assert!(
        matches!(r.items(), [Item::Overlap { .. }]),
        "{:?}",
        r.items()
    );
}

#[test]
fn a_segment_through_a_flat_patch_is_found_where_the_clip_pins_it() {
    // A flat bilinear patch z = 0 over [−1000, 1000]² and a segment straight down through it: the
    // patch's fat plane has no thickness and the segment's slabs none either, so each clip pins
    // its parameter to a single value, and the box of one point must stand.
    let net = [
        [-1000.0, -1000.0, 0.0],
        [-1000.0, 1000.0, 0.0],
        [1000.0, -1000.0, 0.0],
        [1000.0, 1000.0, 0.0],
    ];
    let flat = SurfaceKind::Nurbs(Surface::new(basis(1), basis(1), &net).unwrap());
    let r = meet(
        &bezier(&[[250.0, -500.0, 1000.0], [250.0, -500.0, -1000.0]]),
        &flat,
    );
    assert_eq!(contacts(&r), [PointContact::Crossing]);
    let p = points(&r);
    for (got, want) in p[0].iter().zip([0.5, 0.625, 0.25]) {
        assert!((got - want).abs() < 1e-9, "{p:?}");
    }
}
