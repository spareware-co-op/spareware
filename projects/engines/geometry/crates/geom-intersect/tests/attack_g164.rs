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
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_curve::{CurveKind, Frame, Helix, HelixAxes, Line};
use geom_intersect::IntersectError;
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::result::{Coincidence, Coverage, Intersection, Item, PointContact, Unresolved};
use geom_nurbs::{Basis, Curve, Surface};
use geom_run::{Budget, CancelToken, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind, Torus};
use geom_test::{Seed, SplitMix64};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn meet(c: &CurveKind<3>, s: &SurfaceKind) -> Intersection {
    curve_surface(c, s, &mut meter()).unwrap()
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

#[test]
fn a_line_meets_a_plane_once() {
    let r = meet(&line([3.0, 4.0, 10.0], [0.0, 0.0, -2.0]), &plane());
    assert_eq!(contacts(&r), [PointContact::Crossing]);
    let p = points(&r);
    near(p[0][0], 5.0);
    near(p[0][1], 3.0);
    near(p[0][2], 4.0);
    // A segment that stops short, and one that ends on it.
    is_empty(&meet(
        &segment([3.0, 4.0, 10.0], [3.0, 4.0, 5.0]),
        &plane(),
    ));
    let r = meet(&segment([3.0, 4.0, 10.0], [3.0, 4.0, 0.0]), &plane());
    assert_eq!(contacts(&r), [PointContact::BoundaryTouch]);
    // Past the plane's domain.
    is_empty(&meet(&line([500.0, 4.0, 1.0], [0.0, 0.0, 1.0]), &plane()));
}

#[test]
fn a_parallel_line_misses_a_plane_or_lies_in_it() {
    is_empty(&meet(&line([0.0, 0.0, 1.0], [1.0, 2.0, 0.0]), &plane()));
    let r = meet(&segment([0.0, 0.0, 0.0], [5.0, 5.0, 0.0]), &plane());
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
    // Parallel by the least the doubles can say: a tolerance would call it so, and it is not.
    let tiny = 2.0_f64.powi(-40);
    let r = meet(&line([0.0, 0.0, 1.0], [1.0, 0.0, -tiny]), &plane());
    is_empty(&r);
}

#[test]
fn a_tilted_plane_is_met_on_its_own_vectors() {
    let side = (-1000.0, 1000.0);
    let p = SurfaceKind::Plane(
        Plane::new([1.0, 1.0, 1.0], [1.0, 1.0, 0.0], [0.0, 1.0, 1.0], (side, side)).unwrap(),
    );
    let r = meet(&line([1.0, 4.0, 1.0], [1.0, -1.0, 1.0]), &p);
    let q = points(&r);
    assert_eq!(q.len(), 1);
    let [t, u, v] = q[0];
    near(1.0 + t, 1.0 + u);
    near(4.0 - t, 1.0 + u + v);
    near(1.0 + t, 1.0 + v);
}

#[test]
fn a_line_meets_a_sphere() {
    let s = sphere(5.0);
    let r = meet(&line([3.0, 0.0, -9.0], [0.0, 0.0, 1.0]), &s);
    let p = points(&r);
    assert_eq!(p.len(), 2);
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
    let p = points(&meet(&line([-9.0, 3.0, 2.0], [1.0, 0.0, 0.0]), &c));
    assert_eq!(p.len(), 2);
    near(p[0][0], 5.0);
    near(p[1][0], 13.0);
    near(p[0][2], 2.0);
    let r = meet(&line([-9.0, 5.0, 2.0], [1.0, 0.0, 0.0]), &c);
    assert_eq!(contacts(&r), [PointContact::IsolatedTangent]);
    is_empty(&meet(&line([-9.0, 6.0, 2.0], [1.0, 0.0, 0.0]), &c));
    // Along the axis, off the surface: never. On a ruling: all of it.
    is_empty(&meet(&line([1.0, 1.0, 0.0], [0.0, 0.0, 1.0]), &c));
    let r = meet(&segment([5.0, 0.0, -9.0], [5.0, 0.0, 9.0]), &c);
    assert!(matches!(r.items(), [Item::Overlap { .. }]), "{:?}", r.items());
    // Past the cylinder's height.
    is_empty(&meet(&line([-9.0, 3.0, 60.0], [1.0, 0.0, 0.0]), &c));
}

#[test]
fn a_line_meets_a_cone() {
    // Radius 2 + v about z, for v from −10 to 10: the apex is at z = −2.
    let c = SurfaceKind::Cone(
        Cone::new([0.0; 3], 2.0, 1.0, frame(), (ROUND, (-10.0, 10.0))).unwrap(),
    );
    let p = points(&meet(&line([-9.0, 0.0, 1.0], [1.0, 0.0, 0.0]), &c));
    assert_eq!(p.len(), 2);
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
    let flat = bezier(&[
        [-2000.0, 0.0, 500.0],
        [2000.0, 0.0, 500.0],
    ]);
    let r = meet(&flat, &bowl());
    assert_eq!(contacts(&r), [PointContact::Crossing; 2], "{:?}", r.items());
    let above = bezier(&[
        [-2000.0, 0.0, 1500.0],
        [2000.0, 0.0, 1500.0],
    ]);
    is_empty(&meet(&above, &bowl()));
}

#[test]
fn a_grazing_curve_is_flagged_with_its_band() {
    // Tangent to the top of the bowl, z = 1000 at the middle.
    let graze = bezier(&[
        [-2000.0, 0.0, 1000.0],
        [2000.0, 0.0, 1000.0],
    ]);
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
    let same = bezier(&[
        [-1000.0, 0.0, 0.0],
        [0.0, 0.0, 2000.0],
        [1000.0, 0.0, 0.0],
    ]);
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
            &plus(
                &[e],
                &plus(
                    &[0.0, 0.0, a],
                    &times(&[b], &times(&g, &g)),
                ),
            ),
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
