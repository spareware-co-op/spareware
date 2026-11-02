//! Attacks on G167: one test per row of chapter 12.7 past the natural quadrics, each result read off
//! geometry a hand can check: tori square to, through and at Villarceau's angle to a plane, about a
//! quadric or another torus; revolutions cut by planes; extrusions cut by planes and by each other.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_curve::{
    Axes, Basis, Circle, CurveKind, Ellipse, Frame, Hyperbola, Line, Parabola, ParamCurve, Trimmed,
};
use geom_intersect::quadrics::natural;
use geom_intersect::result::{Coincidence, CurveContact, CurveEnd, Intersection, Item};
use geom_intersect::{Projection, project_curve, project_surface};
use geom_num::exactness::Exactness;
use geom_num::grid::Steps;
use geom_nurbs::{Curve, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, Extrusion, ParamSurface, Plane, Revolution, Sphere, SurfaceKind, Torus,
};

const ROUND: (f64, f64) = (0.0, 2.0);
const TALL: (f64, f64) = (-500.0, 500.0);
const WIDE: (f64, f64) = (-1000.0, 1000.0);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The meeting of `a` and `b`, checked on the way out by [`holds`].
fn meet(a: &SurfaceKind, b: &SurfaceKind) -> Intersection {
    let r = natural(a, b, &mut meter()).unwrap().unwrap();
    holds(&r, a, b);
    r
}

/// Where `p` lands in `s`'s parameters by the kernel's projection: `None` where it cannot say
/// (on an axis, where every turn is as near, or on a seam its search leaves open).
fn landing(s: &SurfaceKind, p: [f64; 3]) -> Option<[f64; 2]> {
    if let SurfaceKind::Revolution(x) = s {
        return Some(on_revolution(x, p).1);
    }
    match project_surface(s, &p, 1e-9, &mut meter()).unwrap() {
        Projection::Points(points) => {
            let n = points.first()?;
            let mid = |k: usize| f64::midpoint(n.at[k].lo(), n.at[k].hi());
            Some([mid(0), mid(1)])
        }
        Projection::Set { .. } | Projection::NonIsolated { .. } => None,
    }
}

/// How far `p` stands from `curve`, by the kernel's projection.
fn from_curve(curve: &CurveKind<3>, p: [f64; 3]) -> f64 {
    match project_curve(curve, &p, 1e-9, &mut meter()).unwrap() {
        Projection::Points(points) => points.first().map_or(f64::INFINITY, |n| n.distance.hi()),
        Projection::Set { distance, .. } => distance.hi(),
        Projection::NonIsolated { enclosure } => {
            panic!("{p:?} undecided on {curve:?}: {enclosure:?}")
        }
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(a: [f64; 3]) -> f64 {
    dot3(a, a).sqrt()
}

/// How far `p` stands off `s`: an analytic kind by its own equation, a revolution by `p` turned into
/// the half-plane of its profile, an extrusion by the kernel's projection.
fn off(s: &SurfaceKind, p: [f64; 3]) -> f64 {
    // The distance from the axis through `c` along unit `n`, and the height along it.
    let radial = |c: [f64; 3], n: [f64; 3]| {
        let n = n.map(|x| x / length(n));
        let w = sub(p, c);
        let z = dot3(w, n);
        (
            length([w[0] - z * n[0], w[1] - z * n[1], w[2] - z * n[2]]),
            z,
        )
    };
    match s {
        SurfaceKind::Plane(plane) => {
            let (c, a, b) = plane.vectors();
            let n = [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ];
            (dot3(sub(p, c), n) / length(n)).abs()
        }
        SurfaceKind::Sphere(x) => (length(sub(p, x.center())) - x.radius()).abs(),
        SurfaceKind::Cylinder(x) => (radial(x.center(), x.frame().normal()).0 - x.radius()).abs(),
        SurfaceKind::Cone(x) => {
            let (r0, k) = x.radius_and_slope();
            let (rho, z) = radial(x.center(), x.frame().normal());
            (rho - (r0 + k * z * x.axial_sign()).abs()).abs()
        }
        SurfaceKind::Torus(x) => {
            let (major, minor) = x.radii();
            let (rho, z) = radial(x.center(), x.frame().normal());
            ((rho - major).hypot(z) - minor).abs()
        }
        SurfaceKind::Revolution(x) => on_revolution(x, p).0,
        _ => match project_surface(s, &p, 1e-9, &mut meter()).unwrap() {
            Projection::Points(points) => points.first().map_or(f64::INFINITY, |n| n.distance.hi()),
            Projection::Set { distance, .. } => distance.hi(),
            Projection::NonIsolated { enclosure } => {
                panic!("{p:?} undecided on {s:?}: {enclosure:?}")
            }
        },
    }
}

/// How far `p` stands off the revolution `x`, and where it lands in its parameters: `p` turned into
/// the half-plane of the profile, where u is 0, the turn it took, and the profile's nearest point.
/// On the axis every turn lands there: the domain's first.
fn on_revolution(x: &Revolution, p: [f64; 3]) -> (f64, [f64; 2]) {
    let unit = |v: [f64; 3]| v.map(|c| c / length(v));
    let (profile, a, frame) = x.shape().unwrap();
    let n = unit(frame.normal());
    let w = sub(p, a);
    let z = dot3(w, n);
    let flat = sub(w, n.map(|c| c * z));
    let rho = length(flat);
    let (lo, hi) = profile.interval();
    let m = sub(
        profile
            .point(f64::midpoint(lo, hi), Limit::FromRight)
            .unwrap(),
        a,
    );
    let out = unit(sub(m, n.map(|c| c * dot3(m, n))));
    let side = [
        n[1] * out[2] - n[2] * out[1],
        n[2] * out[0] - n[0] * out[2],
        n[0] * out[1] - n[1] * out[0],
    ];
    let turn = if rho < 1e-12 {
        ParamSurface::domain(x).0.0
    } else {
        let t = dot3(flat, side).atan2(dot3(flat, out)) / core::f64::consts::PI;
        if t < 0.0 { t + 2.0 } else { t }
    };
    let turned = [0, 1, 2].map(|k| a[k] + rho * out[k] + z * n[k]);
    let curve = CurveKind::Trimmed(profile.clone());
    match project_curve(&curve, &turned, 1e-9, &mut meter()).unwrap() {
        Projection::Points(points) => {
            let near = points.first().unwrap();
            let [t] = near.at;
            (near.distance.hi(), [turn, f64::midpoint(t.lo(), t.hi())])
        }
        other => panic!("{turned:?} on {curve:?}: {other:?}"),
    }
}

/// Whether `p`, landing at `at`, lies within `reach` of an edge of `s`'s domain (`seam` false) or
/// of the seam of a whole turn: the surface's point on that edge level with `at`, measured in space.
fn on_edge(s: &SurfaceKind, at: [f64; 2], seam: bool, p: [f64; 3], reach: f64) -> bool {
    let turns = match s {
        SurfaceKind::Torus(_) => [true, true],
        SurfaceKind::Sphere(_)
        | SurfaceKind::Cylinder(_)
        | SurfaceKind::Cone(_)
        | SurfaceKind::Revolution(_) => [true, false],
        _ => [false, false],
    };
    let (u, v) = ParamSurface::domain(s);
    let sides = (Limit::FromRight, Limit::FromRight);
    [(0, u), (1, v)]
        .into_iter()
        .zip(turns)
        .any(|((k, (lo, hi)), turn)| {
            (turn && hi - lo >= 2.0) == seam
                && [lo, hi].into_iter().filter(|e| e.is_finite()).any(|e| {
                    let mut there = at;
                    there[k] = e;
                    s.point(there[0], there[1], sides)
                        .is_ok_and(|q| length(sub(p, q)) <= reach)
                })
        })
}

/// How far a curve of class `class` may stand from either surface at `p`, in the model's units (one
/// step of Q0 each, as `Steps` counts them), with rounding's share: an exact curve by rounding alone.
fn allowed(class: Exactness, p: [f64; 3]) -> f64 {
    let rounding = 1e-6 * (1.0 + length(p));
    match class {
        Exactness::Exact => rounding,
        Exactness::Certified {
            bound: Steps(n),
            step,
        } => f64::from(u32::try_from(n).unwrap()) * f64::from(step.quarter_nm()) / 625.0 + rounding,
        Exactness::Approximate { .. } => panic!("a closed form only approximate"),
    }
}

/// What every answer must be: each curve within its class's bound of both surfaces at every sample,
/// no two curves one, a loop ending where it starts, and each other end on an edge of a domain, or
/// on a seam where it says so.
fn holds(r: &Intersection, a: &SurfaceKind, b: &SurfaceKind) {
    let all: Vec<(&CurveKind<3>, Exactness, [CurveEnd; 2])> = r
        .items()
        .iter()
        .filter_map(|i| match i {
            Item::Curve {
                curve, class, ends, ..
            } => Some((&**curve, *class, *ends)),
            _ => None,
        })
        .collect();
    for (i, (c, class, _)) in all.iter().enumerate() {
        let points = samples(c);
        for p in &points {
            for s in [a, b] {
                let gap = off(s, *p);
                assert!(
                    gap <= allowed(*class, *p),
                    "curve {i} at {p:?} stands {gap} off {s:?}: {:?}",
                    r.items()
                );
            }
        }
        // Two copies of one curve: rounding apart, not a class's bound, which may span both.
        for (j, (d, ..)) in all.iter().enumerate().skip(i + 1) {
            let one = points[1..8]
                .iter()
                .all(|p| from_curve(d, *p) <= allowed(Exactness::Exact, *p));
            assert!(!one, "curves {i} and {j} are one: {:?}", r.items());
        }
    }
    for (curve, class, ends) in &all {
        let (lo, hi) = curve.domain();
        if *ends == [CurveEnd::Closed; 2] {
            let (p, q) = (
                curve.point(lo, Limit::FromRight).unwrap(),
                curve.point(hi, Limit::FromLeft).unwrap(),
            );
            let gap = length(sub(p, q));
            assert!(
                gap <= allowed(*class, p),
                "a loop from {p:?} ends {gap} away, at {q:?}: {:?}",
                r.items()
            );
        }
        for (end, t, side) in [
            (ends[0], lo, Limit::FromRight),
            (ends[1], hi, Limit::FromLeft),
        ] {
            let seam = match end {
                CurveEnd::Boundary => false,
                CurveEnd::Seam => true,
                _ => continue,
            };
            let p = curve.point(t, side).unwrap();
            let reach = allowed(*class, p);
            let edge = [a, b]
                .into_iter()
                .any(|s| landing(s, p).is_some_and(|at| on_edge(s, at, seam, p, reach)));
            assert!(
                edge,
                "a piece ends {end:?} at {p:?}, off every edge: {:?}",
                r.items()
            );
        }
    }
}

fn up() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap()
}

fn plane(c: [f64; 3], a: [f64; 3], b: [f64; 3]) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(c, a, b, (WIDE, WIDE)).unwrap())
}

fn level(z: f64) -> SurfaceKind {
    plane([0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
}

fn torus(z: f64, major: f64, minor: f64) -> SurfaceKind {
    SurfaceKind::Torus(Torus::new([0.0, 0.0, z], major, minor, up(), (ROUND, ROUND)).unwrap())
}

fn sphere(z: f64, r: f64) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere::new([0.0, 0.0, z], r, up(), (ROUND, (-0.5, 0.5))).unwrap())
}

fn cylinder(r: f64) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], r, up(), (ROUND, TALL)).unwrap())
}

fn cone(z: f64, k: f64) -> SurfaceKind {
    SurfaceKind::Cone(Cone::new([0.0, 0.0, z], 0.0, k, up(), (ROUND, TALL)).unwrap())
}

fn curves(r: &Intersection) -> Vec<&CurveKind<3>> {
    r.items()
        .iter()
        .filter_map(|i| match i {
            Item::Curve { curve, .. } => Some(&**curve),
            _ => None,
        })
        .collect()
}

/// The radii and heights of the circles of `r`, sorted.
fn rings(r: &Intersection) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = curves(r)
        .iter()
        .map(|c| match c {
            CurveKind::Circle(c) => (c.radius(), c.center()[2]),
            other => panic!("{other:?}"),
        })
        .collect();
    out.sort_by(f64::total_cmp_pair);
    out
}

trait Pair {
    fn total_cmp_pair(a: &(f64, f64), b: &(f64, f64)) -> core::cmp::Ordering;
}

impl Pair for f64 {
    fn total_cmp_pair(a: &(f64, f64), b: &(f64, f64)) -> core::cmp::Ordering {
        a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1))
    }
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-3, "{a} and {b}");
}

/// The circles of `r` are these, to within the certified reach.
fn rings_are(r: &Intersection, want: &[(f64, f64)]) {
    let got = rings(r);
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want) {
        near(g.0, w.0);
        near(g.1, w.1);
    }
}

fn tangent(r: &Intersection) -> bool {
    r.items().iter().all(|i| {
        matches!(
            i,
            Item::Curve {
                contact: CurveContact::Tangent { .. },
                ..
            }
        )
    })
}

/// Samples of a curve.
fn samples(c: &CurveKind<3>) -> Vec<[f64; 3]> {
    let (lo, hi) = c.domain();
    (0..=8)
        .map(|i| {
            let t = lo + (hi - lo) * f64::from(i) / 8.0;
            c.point(t, Limit::FromRight).unwrap()
        })
        .collect()
}

/// How far `p` stands from the torus at the origin.
fn off_torus(p: [f64; 3], major: f64, minor: f64) -> f64 {
    let rho = (p[0] * p[0] + p[1] * p[1]).sqrt();
    ((rho - major).powi(2) + p[2] * p[2]).sqrt() - minor
}

#[test]
fn a_plane_square_to_a_torus() {
    let t = torus(0.0, 5.0, 1.0);
    let r = meet(&level(0.0), &t);
    rings_are(&r, &[(4.0, 0.0), (6.0, 0.0)]);
    let r = meet(&level(1.0), &t);
    assert_eq!(rings(&r).len(), 1);
    near(rings(&r)[0].0, 5.0);
    assert!(tangent(&r));
    assert_eq!(meet(&level(2.0), &t).items().len(), 0);
    let r = meet(&level(0.6), &t);
    let got = rings(&r);
    near(got[0].0, 4.2);
    near(got[1].0, 5.8);
}

#[test]
fn a_plane_through_a_torus_s_axis() {
    let t = torus(0.0, 5.0, 1.0);
    let r = meet(&plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), &t);
    let c = curves(&r);
    assert_eq!(c.len(), 2);
    for curve in c {
        let CurveKind::Circle(circle) = curve else {
            panic!("{curve:?}");
        };
        near(circle.radius(), 1.0);
        near(circle.center()[0].abs(), 5.0);
        for p in samples(curve) {
            near(off_torus(p, 5.0, 1.0), 0.0);
        }
    }
    // Off the axis it is no special position.
    let off = plane([0.0, 2.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    assert!(natural(&off, &t, &mut meter()).unwrap().is_none());
}

#[test]
fn villarceau_s_two_circles() {
    // R = 5, r = 3: sin θ = 3/5, a plane through the centre with slope 3/4.
    let t = torus(0.0, 5.0, 3.0);
    let cut = plane([0.0; 3], [4.0, 0.0, 3.0], [0.0, 1.0, 0.0]);
    let r = meet(&cut, &t);
    let c = curves(&r);
    assert_eq!(c.len(), 2);
    for curve in c {
        let CurveKind::Circle(circle) = curve else {
            panic!("{curve:?}");
        };
        near(circle.radius(), 5.0);
        near(circle.center()[1].abs(), 3.0);
        near(circle.center()[0], 0.0);
        for p in samples(curve) {
            near(off_torus(p, 5.0, 3.0), 0.0);
            near(p[0] * 3.0 - p[2] * 4.0, 0.0);
        }
    }
    // One step off the angle is general position, and so is a parallel plane.
    let steeper = plane([0.0; 3], [4.0, 0.0, 2.0], [0.0, 1.0, 0.0]);
    assert!(natural(&steeper, &t, &mut meter()).unwrap().is_none());
    let shifted = plane([0.0, 0.0, 1.0], [4.0, 0.0, 3.0], [0.0, 1.0, 0.0]);
    assert!(natural(&shifted, &t, &mut meter()).unwrap().is_none());
    // A torus off the grid has no exact angle to read.
    let round = torus(0.0, 5.5, 3.3);
    assert!(natural(&cut, &round, &mut meter()).unwrap().is_none());
}

#[test]
fn quadrics_about_a_torus_s_axis() {
    let t = torus(0.0, 5.0, 1.0);
    // Cylinders through the tube, touching it, and past it.
    let r = meet(&cylinder(5.0), &t);
    rings_are(&r, &[(5.0, -1.0), (5.0, 1.0)]);
    let r = meet(&cylinder(6.0), &t);
    rings_are(&r, &[(6.0, 0.0)]);
    assert!(tangent(&r));
    assert_eq!(meet(&cylinder(7.0), &t).items().len(), 0);
    // Spheres about its centre.
    let r = meet(&sphere(0.0, 6.0), &t);
    rings_are(&r, &[(6.0, 0.0)]);
    assert!(tangent(&r));
    assert_eq!(meet(&sphere(0.0, 3.0), &t).items().len(), 0);
    let r = meet(&sphere(0.0, 5.0), &t);
    assert_eq!(rings(&r).len(), 2);
    for (radius, z) in rings(&r) {
        near(radius * radius + z * z, 25.0);
    }
    // Off the axis there is no ring.
    let aside =
        SurfaceKind::Sphere(Sphere::new([1.0, 0.0, 0.0], 5.0, up(), (ROUND, (-0.5, 0.5))).unwrap());
    assert!(natural(&aside, &t, &mut meter()).unwrap().is_none());
    // A cone from its centre: slope 1 cuts the tube in the two ends of a 45-degree chord, each nappe.
    let r = meet(&cone(0.0, 1.0), &torus(0.0, 2.0, 1.5));
    assert_eq!(rings(&r).len(), 4);
    for (radius, z) in rings(&r) {
        near(radius, z.abs());
    }
}

#[test]
fn two_tori_about_one_axis() {
    let a = torus(0.0, 5.0, 1.0);
    let r = meet(&a, &torus(1.0, 5.0, 1.0));
    assert_eq!(rings(&r).len(), 2);
    let r = meet(&a, &torus(2.0, 5.0, 1.0));
    rings_are(&r, &[(5.0, 1.0)]);
    assert!(tangent(&r));
    assert_eq!(meet(&a, &torus(3.0, 5.0, 1.0)).items().len(), 0);
    let same = meet(&a, &torus(0.0, 5.0, 1.0));
    assert!(matches!(
        same.items(),
        [Item::Overlap {
            proof: Coincidence::Identical,
            ..
        }]
    ));
}

fn trimmed(c: CurveKind<3>) -> Trimmed<3> {
    let (lo, hi) = c.domain();
    Trimmed::new(Basis::try_from(c).unwrap(), (lo, hi)).unwrap()
}

/// The surface of a segment turned about z.
fn cup() -> SurfaceKind {
    let line = Line::segment([1.0, 0.0, 0.0], [3.0, 0.0, 4.0])
        .unwrap()
        .value;
    let profile = trimmed(CurveKind::Line(line));
    SurfaceKind::Revolution(Revolution::new(profile, [0.0; 3], up(), ROUND).unwrap())
}

/// A quadratic in the plane y = 0: ρ = 2 + 8t − 8t², z = 4t.
fn vase_profile() -> CurveKind<3> {
    let basis = geom_nurbs::Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let points = [[2.0, 0.0, 0.0], [6.0, 0.0, 2.0], [2.0, 0.0, 4.0]];
    CurveKind::Nurbs(Curve::new(basis, &points).unwrap())
}

/// The quadratic turned about z.
fn vase() -> SurfaceKind {
    SurfaceKind::Revolution(
        Revolution::new(trimmed(vase_profile()), [0.0; 3], up(), ROUND).unwrap(),
    )
}

#[test]
fn a_plane_square_to_a_revolution_cuts_a_circle_at_each_root() {
    let r = meet(&level(2.0), &cup());
    assert_eq!(rings(&r).len(), 1);
    near(rings(&r)[0].0, 2.0);
    assert_eq!(meet(&level(5.0), &cup()).items().len(), 0);
    // The quadratic's middle reaches ρ = 4 at z = 2.
    let r = meet(&level(2.0), &vase());
    near(rings(&r)[0].0, 4.0);
    near(rings(&r)[0].1, 2.0);
}

#[test]
fn a_quadric_about_a_revolution() {
    // The quadratic at ρ = 3 twice, once on each side of z = 2.
    let r = meet(&cylinder(3.0), &vase());
    assert_eq!(rings(&r).len(), 2);
    for (radius, z) in rings(&r) {
        near(radius, 3.0);
        near((z - 2.0).abs(), 2.0_f64.sqrt());
    }
    // A cylinder holding the profile's whole straight side would overlap; a sphere meets the cone.
    let r = meet(&sphere(0.0, 5.0), &cup());
    assert_eq!(rings(&r).len(), 1);
}

#[test]
fn a_plane_through_a_revolution_s_axis_cuts_two_copies() {
    let cut = plane([0.0; 3], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let r = meet(&cut, &cup());
    let c = curves(&r);
    assert_eq!(c.len(), 2);
    for curve in &c {
        assert!(matches!(curve, CurveKind::Line(_)), "{curve:?}");
        let [a, .., b] = samples(curve)[..] else {
            panic!();
        };
        near(a[0], 0.0);
        near(a[1].abs().min(b[1].abs()), 1.0);
        near(a[1].abs().max(b[1].abs()), 3.0);
    }
    let r = meet(&cut, &vase());
    let c = curves(&r);
    assert_eq!(c.len(), 2);
    for curve in &c {
        assert!(
            matches!(curve, CurveKind::Nurbs(_) | CurveKind::Trimmed(_)),
            "{curve:?}"
        );
    }
}

/// A box wall: a segment pushed along z.
fn wall(a: [f64; 3], b: [f64; 3]) -> SurfaceKind {
    let line = Line::segment(a, b).unwrap().value;
    SurfaceKind::Extrusion(
        Extrusion::new(trimmed(CurveKind::Line(line)), [0.0, 0.0, 1.0], (0.0, 10.0)).unwrap(),
    )
}

/// A quarter of a circle of radius 5 on a frame whose every axis leans, pushed along its normal.
fn leaning_wall() -> SurfaceKind {
    let third = 1.0 / 3.0;
    let frame = Frame::new(
        [2.0 * third, 2.0 * third, third],
        [third, -2.0 * third, 2.0 * third],
    )
    .unwrap();
    let arc = Circle::new([0.0; 3], 5.0, frame, (0.0, 0.5)).unwrap();
    pushed(CurveKind::Circle(arc), [1.0, -2.0, 2.0], (0.0, 3.0))
}

/// A quarter of a circle of radius 5 pushed along z.
fn curved_wall() -> SurfaceKind {
    let arc = Circle::new([0.0; 3], 5.0, up(), (0.0, 0.5)).unwrap();
    SurfaceKind::Extrusion(
        Extrusion::new(
            trimmed(CurveKind::Circle(arc)),
            [0.0, 0.0, 1.0],
            (0.0, 10.0),
        )
        .unwrap(),
    )
}

#[test]
fn a_plane_along_an_extrusion_cuts_lines() {
    let cut = plane([3.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let r = meet(&cut, &curved_wall());
    let c = curves(&r);
    assert_eq!(c.len(), 1);
    let CurveKind::Line(line) = c[0] else {
        panic!("{:?}", c[0]);
    };
    near(line.origin()[0], 3.0);
    near(line.origin()[1], 4.0);
    assert_eq!(line.direction(), [0.0, 0.0, 1.0]);
    // Past the profile's end: no line.
    let past = plane([6.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    assert_eq!(meet(&past, &curved_wall()).items().len(), 0);
    // A wall in the plane is the plane.
    let same = meet(
        &plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        &wall([0.0; 3], [5.0, 0.0, 0.0]),
    );
    assert!(matches!(same.items(), [Item::Overlap { .. }]));
}

#[test]
fn a_plane_across_an_extrusion_cuts_a_copy_of_the_profile() {
    // Square: the profile itself at that height.
    let r = meet(&level(4.0), &wall([0.0; 3], [5.0, 0.0, 0.0]));
    let c = curves(&r);
    assert_eq!(c.len(), 1);
    assert!(matches!(c[0], CurveKind::Line(_)));
    for p in samples(c[0]) {
        near(p[2], 4.0);
        near(p[1], 0.0);
    }
    // Tilted: lines stay lines, an arc becomes an ellipse on the plane.
    let tilted = plane([0.0, 0.0, 5.0], [1.0, 0.0, 0.25], [0.0, 1.0, 0.0]);
    let r = meet(&tilted, &curved_wall());
    let c = curves(&r);
    assert_eq!(c.len(), 1);
    assert!(matches!(c[0], CurveKind::Ellipse(_)), "{:?}", c[0]);
    for p in samples(c[0]) {
        near(p[0] * p[0] + p[1] * p[1], 25.0);
        near(p[2], 5.0 + p[0] * 0.25);
    }
    // Past the sweep's end at one edge: a shorter stretch of the copy.
    let steep = plane([0.0, 0.0, 8.0], [1.0, 0.0, 1.0], [0.0, 1.0, 0.0]);
    let r = meet(&steep, &curved_wall());
    let c = curves(&r);
    assert_eq!(c.len(), 1);
    for p in samples(c[0]) {
        assert!(p[2] <= 10.0 + 1e-6, "{p:?}");
        near(p[0] * p[0] + p[1] * p[1], 25.0);
    }
    assert_eq!(
        meet(&level(20.0), &wall([0.0; 3], [5.0, 0.0, 0.0]))
            .items()
            .len(),
        0
    );
}

#[test]
fn two_sweeps_of_one_direction_meet_in_lines() {
    let a = wall([0.0, -5.0, 0.0], [0.0, 5.0, 0.0]);
    let b = wall([-5.0, 1.0, 0.0], [5.0, 1.0, 0.0]);
    let r = meet(&a, &b);
    let c = curves(&r);
    assert_eq!(c.len(), 1);
    let CurveKind::Line(line) = c[0] else {
        panic!("{:?}", c[0]);
    };
    near(line.origin()[0], 0.0);
    near(line.origin()[1], 1.0);
    let (lo, hi) = line.domain();
    near(hi - lo, 10.0);
    // A curved wall with a straight one, and with a cylinder of its own direction.
    let r = meet(&wall([3.0, -5.0, 0.0], [3.0, 5.0, 0.0]), &curved_wall());
    assert_eq!(curves(&r).len(), 1);
    let tube =
        SurfaceKind::Cylinder(Cylinder::new([5.0, 0.0, 0.0], 5.0, up(), (ROUND, TALL)).unwrap());
    let r = meet(&tube, &curved_wall());
    let c = curves(&r);
    assert_eq!(c.len(), 1);
    // Crossing lines through (2.5, ±4.33): the arc's first quadrant keeps one.
    let CurveKind::Line(line) = c[0] else {
        panic!("{:?}", c[0]);
    };
    near(line.origin()[0], 2.5);
    // Walls of two directions are no sweep of one.
    let other = SurfaceKind::Extrusion(
        Extrusion::new(
            trimmed(CurveKind::Line(
                Line::segment([0.0; 3], [5.0, 0.0, 0.0]).unwrap().value,
            )),
            [0.0, 1.0, 0.0],
            (0.0, 10.0),
        )
        .unwrap(),
    );
    assert!(natural(&a, &other, &mut meter()).unwrap().is_none());
}

#[test]
fn a_part_of_a_turn_is_left_to_the_general_paths() {
    let half =
        SurfaceKind::Torus(Torus::new([0.0; 3], 5.0, 1.0, up(), ((0.0, 1.0), ROUND)).unwrap());
    assert!(natural(&level(0.0), &half, &mut meter()).unwrap().is_none());
}

/// The segment from `a` to `b`.
fn segment(a: [f64; 3], b: [f64; 3]) -> CurveKind<3> {
    CurveKind::Line(Line::segment(a, b).unwrap().value)
}

/// `profile` pushed along `d` over `span`.
fn pushed(profile: CurveKind<3>, d: [f64; 3], span: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Extrusion(Extrusion::new(trimmed(profile), d, span).unwrap())
}

/// `profile` on `span` turned about the origin's axis along `frame`, over `turn`.
fn spun(profile: CurveKind<3>, span: (f64, f64), frame: Frame<3>, turn: (f64, f64)) -> SurfaceKind {
    let profile = Trimmed::new(Basis::try_from(profile).unwrap(), span).unwrap();
    SurfaceKind::Revolution(Revolution::new(profile, [0.0; 3], frame, turn).unwrap())
}

/// The frame about z the other way up.
fn down() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]).unwrap()
}

/// The conic axes u and v.
fn axes(u: [f64; 3], v: [f64; 3]) -> Axes<3> {
    Axes::Vectors { u, v }
}

/// A cylinder about `c` on `frame`, on part of its turn and height.
fn tube(c: [f64; 3], r: f64, frame: Frame<3>, turn: (f64, f64), height: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new(c, r, frame, (turn, height)).unwrap())
}

/// A cone with its apex at `c` on `frame`, on part of its turn and height.
fn funnel(
    c: [f64; 3],
    k: f64,
    frame: Frame<3>,
    turn: (f64, f64),
    height: (f64, f64),
) -> SurfaceKind {
    SurfaceKind::Cone(Cone::new(c, 0.0, k, frame, (turn, height)).unwrap())
}

/// A wall whose profile climbs as it runs: each line along it starts where the profile stands at
/// its crossing, not where its middle does.
#[test]
fn a_climbing_profile_sweeps_from_each_crossing() {
    let climbing = pushed(
        segment([0.0; 3], [4.0, 3.0, 4.0]),
        [0.0, 0.0, 1.0],
        (0.0, 10.0),
    );
    let r = meet(&climbing, &wall([3.0, 0.0, 0.0], [3.0, 3.0, 0.0]));
    let c = curves(&r);
    assert_eq!(c.len(), 1, "{:?}", r.items());
    let [a, .., b] = samples(c[0])[..] else {
        panic!();
    };
    near(a[2].min(b[2]), 3.0);
    near(a[2].max(b[2]), 10.0);
}

/// One pair of each reduction, about one axis and off it, by what `natural` makes of it: tori,
/// revolutions and extrusions with planes, quadrics and each other, whole and on part of a turn,
/// and the pairs that only look reducible (off the axis, slanted, part of a turn), which are left
/// to the general paths. Each answer is checked by [`holds`] as it is read; the pins hold every
/// guard, ring and trim, so a change to any is a change to what is made.
#[test]
fn every_reduction_makes_what_it_made() {
    let (x, y, z) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let off_up = Frame::new([1.0, 0.0, 0.0], [0.0, 0.6, 0.8]).unwrap();
    let bitangent = (1.0_f64 / 3.0).asin();
    let villarceau = plane(
        [0.0; 3],
        [bitangent.cos(), 0.0, bitangent.sin()],
        [0.0, 1.0, 0.0],
    );
    let part_torus =
        SurfaceKind::Torus(Torus::new([0.0; 3], 3.0, 1.0, up(), ((0.0, 1.0), ROUND)).unwrap());
    let tilted_torus =
        SurfaceKind::Torus(Torus::new([0.0; 3], 3.0, 1.0, off_up, (ROUND, ROUND)).unwrap());
    let off_sphere =
        SurfaceKind::Sphere(Sphere::new([1.0, 0.0, 0.0], 3.0, up(), (ROUND, (-0.5, 0.5))).unwrap());
    let pairs: Vec<(SurfaceKind, SurfaceKind)> = vec![
        // A torus: planes square to its axis, through it, at Villarceau's angle, and slanted.
        (torus(0.0, 3.0, 1.0), level(0.5)),
        (torus(0.0, 3.0, 1.0), level(1.0)),
        (torus(0.0, 3.0, 1.0), level(0.0)),
        (torus(0.0, 3.0, 1.0), plane([0.0; 3], x, z)),
        (torus(0.0, 3.0, 1.0), villarceau),
        (torus(0.0, 3.0, 1.0), plane([0.0; 3], [1.0, 0.0, 0.3], y)),
        // Quadrics and tori about its axis, and off it.
        (torus(0.0, 3.0, 1.0), sphere(0.0, 3.0)),
        (torus(0.0, 3.0, 1.0), sphere(1.0, 2.5)),
        (torus(0.0, 3.0, 1.0), cylinder(3.5)),
        (torus(0.0, 3.0, 1.0), cone(-2.0, 1.2)),
        (torus(0.0, 3.0, 1.0), torus(1.0, 3.0, 1.0)),
        (torus(0.0, 3.0, 1.0), off_sphere),
        (tilted_torus, level(0.5)),
        (part_torus, level(0.5)),
        // Revolutions: planes square to and through the axis, a quadric about it.
        (cup(), level(2.0)),
        (cup(), plane([0.0; 3], x, z)),
        (cup(), sphere(0.0, 3.0)),
        (cup(), cylinder(2.0)),
        (vase(), level(1.0)),
        (vase(), level(2.0)),
        (vase(), plane([0.0; 3], y, z)),
        (vase(), cylinder(3.0)),
        // Extrusions: planes along and across, slanted, and two sweeps of one direction.
        (
            wall([0.0, 0.0, 0.0], [4.0, 3.0, 0.0]),
            plane([2.0, 0.0, 0.0], y, z),
        ),
        (wall([0.0, 0.0, 0.0], [4.0, 3.0, 0.0]), level(5.0)),
        (
            wall([0.0, 0.0, 0.0], [4.0, 3.0, 0.0]),
            plane([0.0, 0.0, 5.0], [1.0, 0.0, 0.5], y),
        ),
        (curved_wall(), plane([3.0, 0.0, 0.0], y, z)),
        (curved_wall(), level(4.0)),
        (curved_wall(), plane([0.0, 0.0, 5.0], [1.0, 0.0, 0.5], y)),
        (curved_wall(), wall([0.0, 0.0, 0.0], [6.0, 6.0, 0.0])),
        (
            wall([0.0, 0.0, 0.0], [4.0, 3.0, 0.0]),
            wall([4.0, 0.0, 0.0], [0.0, 3.0, 0.0]),
        ),
        // The tube's crossings: behind the axis (a spindle), on it (a horn), at a line's very
        // start (a cone's apex on a horn), at a cylinder's very end, past it, and the other way up.
        (torus(0.0, 1.0, 2.0), level(0.5)),
        (torus(0.0, 1.0, 1.0), level(0.0)),
        (torus(0.0, 1.0, 1.0), cone(0.0, 1.0)),
        (torus(0.0, 3.0, 1.0), sphere(0.0, 2.0)),
        (
            torus(0.0, 3.0, 1.0),
            tube([0.0; 3], 3.0, up(), ROUND, (0.0, 1.0)),
        ),
        (
            torus(0.0, 3.0, 1.0),
            tube([0.0; 3], 3.5, up(), ROUND, (0.0, 0.5)),
        ),
        (
            torus(0.0, 3.0, 1.0),
            tube([0.0; 3], 3.5, down(), ROUND, (-0.2, 3.0)),
        ),
        (
            torus(0.0, 3.0, 1.0),
            funnel([0.0, 0.0, -2.0], 1.2, down(), ROUND, (-8.0, 1.0)),
        ),
        // About the axis on part of a turn, and whole but off it: left to the general paths.
        (
            torus(0.0, 3.0, 1.0),
            tube([0.0; 3], 3.5, up(), (0.0, 1.0), TALL),
        ),
        (
            torus(0.0, 3.0, 1.0),
            funnel([0.0, 0.0, -2.0], 1.2, up(), (0.0, 1.0), TALL),
        ),
        (
            torus(0.0, 3.0, 1.0),
            tube([1.0, 0.0, 0.0], 3.5, up(), ROUND, TALL),
        ),
        (
            torus(0.0, 3.0, 1.0),
            funnel([1.0, 0.0, -2.0], 1.2, up(), ROUND, TALL),
        ),
        (
            torus(0.0, 3.0, 1.0),
            SurfaceKind::Torus(
                Torus::new([0.5, 0.0, 1.0], 3.0, 1.0, up(), (ROUND, ROUND)).unwrap(),
            ),
        ),
        (
            SurfaceKind::Torus(Torus::new([0.0; 3], 3.0, 1.0, up(), ((0.5, 1.5), ROUND)).unwrap()),
            level(0.5),
        ),
        // Revolutions whose profile starts or ends on the axis, a profile on part of its span, part
        // of a turn, the other way up, and a plane beside the axis.
        (
            spun(segment([0.0; 3], [3.0, 0.0, 4.0]), (0.0, 1.0), up(), ROUND),
            level(2.0),
        ),
        (
            spun(segment([0.0; 3], [3.0, 0.0, 4.0]), (0.0, 1.0), up(), ROUND),
            cone(0.0, 0.5),
        ),
        (
            spun(
                segment([3.0, 0.0, 0.0], [0.0, 0.0, 4.0]),
                (0.0, 1.0),
                up(),
                ROUND,
            ),
            level(2.0),
        ),
        (
            spun(segment([0.0; 3], [3.0, 0.0, 4.0]), (0.0, 1.0), up(), ROUND),
            plane([0.0; 3], x, z),
        ),
        (
            spun(
                segment([3.0, 0.0, 0.0], [0.0, 0.0, 4.0]),
                (0.0, 1.0),
                up(),
                ROUND,
            ),
            plane([0.0; 3], y, z),
        ),
        (spun(vase_profile(), (0.6, 1.0), up(), ROUND), level(3.0)),
        (
            spun(vase_profile(), (0.6, 1.0), up(), ROUND),
            plane([0.0; 3], x, z),
        ),
        (
            spun(vase_profile(), (0.0, 1.0), up(), (0.5, 1.5)),
            level(2.0),
        ),
        (
            spun(vase_profile(), (0.0, 1.0), up(), (0.5, 1.5)),
            sphere(0.0, 3.0),
        ),
        (spun(vase_profile(), (0.0, 1.0), down(), ROUND), level(2.0)),
        (cup(), plane([1.0, 0.0, 0.0], y, z)),
        // Planes across extrusions that leave them at both ends, and slanted so a circle's image is
        // an ellipse with equal axes or with one axis kept.
        (
            wall([0.0; 3], [4.0, 0.0, 0.0]),
            plane([2.0, 0.0, 5.0], [1.0, 0.0, 3.0], y),
        ),
        (
            curved_wall(),
            plane([0.0, 0.0, 5.0], [2.0, 0.0, -1.0], [0.0, 2.0, -1.0]),
        ),
        (curved_wall(), plane([0.0, 0.0, 5.0], x, [0.0, 2.0, -1.0])),
        (
            leaning_wall(),
            plane([2.5, -5.0, 5.0], [2.0, 2.0, 1.0], [-2.0, 1.0, 2.0]),
        ),
        // Sweeps: a long direction, one the other way, cylinders whole, offset, the other way up and
        // on part of a turn, and reaches that never meet.
        (
            pushed(
                segment([0.0; 3], [4.0, 3.0, 0.0]),
                [0.0, 0.0, 2.0],
                (1.0, 4.0),
            ),
            wall([4.0, 0.0, 0.0], [0.0, 3.0, 0.0]),
        ),
        (
            pushed(
                segment([0.0, 0.0, 10.0], [4.0, 3.0, 10.0]),
                [0.0, 0.0, -1.0],
                (2.0, 6.0),
            ),
            wall([4.0, 0.0, 0.0], [0.0, 3.0, 0.0]),
        ),
        (wall([0.0; 3], [4.0, 3.0, 0.0]), cylinder(3.0)),
        (
            curved_wall(),
            tube([3.0, 3.0, 2.0], 2.0, up(), ROUND, (1.0, 5.0)),
        ),
        (
            wall([0.0; 3], [4.0, 3.0, 0.0]),
            tube([0.0; 3], 3.0, down(), ROUND, (-3.0, 2.0)),
        ),
        (
            wall([0.0; 3], [4.0, 3.0, 0.0]),
            tube([0.0; 3], 3.0, up(), (0.0, 1.0), TALL),
        ),
        (
            wall([0.0; 3], [4.0, 3.0, 0.0]),
            tube([0.0; 3], 3.0, up(), ROUND, (20.0, 30.0)),
        ),
        // A tube the other way up whose height ends ten steps past a crossing (at this scale an
        // edge within a step of a ring would cut it); a sweep along a direction with every
        // component, across a plane leaving it at both ends; a sweep high above its start, against
        // a tall one; profiles of every conic kind, pushed or turned; a circle pushed slantwise
        // whose image has equal axes that are not square; and two revolutions, one on part of a
        // turn.
        (
            torus(0.0, 300.0, 100.0),
            tube([0.0; 3], 399.5, down(), ROUND, (-20.0, 300.0)),
        ),
        (
            pushed(
                segment([0.0; 3], [4.0, 0.0, 0.0]),
                [1.0, 1.0, 2.0],
                (0.0, 5.0),
            ),
            plane([4.0, 0.0, 1.0], [1.0, 0.0, 3.0], [0.0, 1.0, 1.0]),
        ),
        (
            pushed(
                segment([0.0, 0.0, 10.0], [4.0, 3.0, 10.0]),
                [0.0, 0.0, 2.0],
                (1.0, 4.0),
            ),
            pushed(
                segment([4.0, 0.0, 0.0], [0.0, 3.0, 0.0]),
                [0.0, 0.0, 1.0],
                (0.0, 30.0),
            ),
        ),
        (
            pushed(
                CurveKind::Ellipse(
                    Ellipse::new([0.0; 3], axes([4.0, 0.0, 0.0], [0.0, 2.0, 0.0]), (0.0, 0.5))
                        .unwrap(),
                ),
                [0.0, 0.0, 1.0],
                (0.0, 10.0),
            ),
            level(4.0),
        ),
        (
            pushed(
                CurveKind::Hyperbola(
                    Hyperbola::new(
                        [0.0; 3],
                        axes([2.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
                        (-1.0, 1.0),
                    )
                    .unwrap(),
                ),
                [0.0, 0.0, 1.0],
                (0.0, 10.0),
            ),
            level(4.0),
        ),
        (
            pushed(
                CurveKind::Parabola(
                    Parabola::new(
                        [0.0; 3],
                        axes([1.0, 0.0, 0.0], [0.0, 2.0, 0.0]),
                        (-1.0, 1.0),
                    )
                    .unwrap(),
                ),
                [0.0, 0.0, 1.0],
                (0.0, 10.0),
            ),
            level(4.0),
        ),
        (
            spun(
                CurveKind::Ellipse(
                    Ellipse::new(
                        [5.0, 0.0, 0.0],
                        axes([1.0, 0.0, 0.0], [0.0, 0.0, 2.0]),
                        ROUND,
                    )
                    .unwrap(),
                ),
                ROUND,
                up(),
                ROUND,
            ),
            level(1.0),
        ),
        (
            pushed(
                CurveKind::Circle(Circle::new([0.0; 3], 5.0, up(), (0.0, 0.5)).unwrap()),
                [1.0, 1.0, 1.0],
                (0.0, 4.0),
            ),
            plane([4.0, 0.0, 3.0], [0.0, 1.0, 0.0], [1.0, 0.0, -2.0]),
        ),
        (spun(vase_profile(), (0.0, 1.0), up(), (0.5, 1.5)), cup()),
        (cup(), spun(vase_profile(), (0.0, 1.0), up(), (0.5, 1.5))),
        // A wall past the frame's range: refused.
        (
            wall([1e17, 0.0, 0.0], [1e17 + 4096.0, 3072.0, 0.0]),
            plane([1e17, 0.0, 5.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ),
        // A whole circle whose turn starts at a half, pushed and cut across by a square whose
        // edges cut the ring: a loop not starting at 0, trimmed into four arcs.
        (
            pushed(
                CurveKind::Circle(Circle::new([0.0; 3], 50.0, up(), (0.5, 2.5)).unwrap()),
                [0.0, 0.0, 1.0],
                (0.0, 10.0),
            ),
            SurfaceKind::Plane(
                Plane::new(
                    [0.0, 0.0, 5.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    ((-40.0, 40.0), (-40.0, 40.0)),
                )
                .unwrap(),
            ),
        ),
        // Rings about axes leaning every way, which take their frames' reference by the axis's
        // smallest component: (5, 2, 9), whose x is not the smallest but lies below z, and
        // (9, 2, 5), whose y lies below z.
        (
            SurfaceKind::Torus(
                Torus::new(
                    [1.0, 2.0, 3.0],
                    3.0,
                    1.0,
                    Frame::new([1.0, 0.0, 0.0], [5.0, 2.0, 9.0]).unwrap(),
                    (ROUND, ROUND),
                )
                .unwrap(),
            ),
            SurfaceKind::Sphere(
                Sphere::new([1.0, 2.0, 3.0], 3.0, up(), (ROUND, (-0.5, 0.5))).unwrap(),
            ),
        ),
        (
            SurfaceKind::Torus(
                Torus::new(
                    [1.0, 2.0, 3.0],
                    3.0,
                    1.0,
                    Frame::new([0.0, 1.0, 0.0], [9.0, 2.0, 5.0]).unwrap(),
                    (ROUND, ROUND),
                )
                .unwrap(),
            ),
            SurfaceKind::Sphere(
                Sphere::new([1.0, 2.0, 3.0], 3.0, up(), (ROUND, (-0.5, 0.5))).unwrap(),
            ),
        ),
        // An arc on the turn from 1.5 to 2, pushed and cut across: open, though its ends sum past 2.
        (
            pushed(
                CurveKind::Circle(Circle::new([0.0; 3], 50.0, up(), (1.5, 2.0)).unwrap()),
                [0.0, 0.0, 1.0],
                (0.0, 10.0),
            ),
            level(5.0),
        ),
    ];
    let mut out = Vec::new();
    for (k, (a, b)) in pairs.iter().enumerate() {
        let line = match natural(a, b, &mut meter()) {
            Ok(Some(r)) => {
                holds(&r, a, b);
                format!("{:?} {:?}", r.items(), r.coverage())
            }
            other => format!("{other:?}"),
        };
        println!("{k}: {line}");
        out.push(line);
    }
    assert_eq!(
        out.len(),
        REDUCTIONS.len(),
        "the pins are the lines printed above"
    );
    for (k, (got, want)) in out.iter().zip(REDUCTIONS).enumerate() {
        assert_eq!(got, want, "pair {k}");
    }
}

/// What [`every_reduction_makes_what_it_made`] read.
const REDUCTIONS: &[&str] = &[
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.06364167792230506, hi: 0.9474315432887469 }, Interval { lo: -3.8660254037844477, hi: 3.8660254037844477 }, Interval { lo: -3.8660254037844486, hi: 3.8660254037844486 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.5000000000000001], radius: 3.8660254037844384, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.5000000000000001], [3.8660254037844384, 0.0, 0.0], [0.0, 3.8660254037844384, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.5000000000000001, hi: 0.5000000000000001 }], [Interval { lo: 3.8660254037844384, hi: 3.8660254037844384 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.8660254037844384, hi: 3.8660254037844384 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.4886120310229627, hi: 0.9474315432887469 }, Interval { lo: -2.133974596215568, hi: 2.133974596215568 }, Interval { lo: -2.1339745962155687, hi: 2.1339745962155687 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.5000000000000001], radius: 2.1339745962155616, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.5000000000000001], [2.1339745962155616, 0.0, 0.0], [0.0, 2.1339745962155616, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.5000000000000001, hi: 0.5000000000000001 }], [Interval { lo: 2.1339745962155616, hi: 2.1339745962155616 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.1339745962155616, hi: 2.1339745962155616 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.21569416631608382, hi: 0.8975836176504337 }, Interval { lo: -3.000000000000008, hi: 3.000000000000008 }, Interval { lo: -3.000000000000009, hi: 3.000000000000009 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 1.0], radius: 3.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 1.0], [3.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Tangent { band: Steps(0), gap: StepsInterval { lo: Steps(0), hi: Steps(0) } }, ends: [Closed, Closed], class: Certified { bound: Steps(1), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0000000000000004 }, Interval { lo: -4.000000000000012, hi: 4.000000000000012 }, Interval { lo: -4.000000000000014, hi: 4.000000000000014 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.0], radius: 4.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 4.0, hi: 4.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.0, hi: 4.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(1), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.9999999999999999, hi: 1.0000000000000002 }, Interval { lo: -2.000000000000006, hi: 2.000000000000006 }, Interval { lo: -2.000000000000007, hi: 2.000000000000007 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.0], radius: 2.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 2.0, hi: 2.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0, hi: 2.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(1), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.9999999999999999, hi: 1.0000000000000002 }, Interval { lo: 0.0, hi: 1.0000000000000004 }, Interval { lo: -4.0000000000000115, hi: -1.999999999999995 }, Interval { lo: -1.0000000000000033, hi: 1.0000000000000033 }], unknowns: 4 }, curve: Circle(Circle { center: [-3.0, 0.0, 0.0], radius: 1.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, -1.0, 0.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[-3.0, 0.0, 0.0], [1.0, 0.0, 0.0], [-0.0, 0.0, 1.0], [0.0, -0.0, 0.0]], exact: [[Interval { lo: -3.0, hi: -3.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 1.0, hi: 1.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1e-323 }, Interval { lo: 0.0, hi: 1.0000000000000004 }, Interval { lo: 1.999999999999995, hi: 4.0000000000000115 }, Interval { lo: -1.0000000000000033, hi: 1.0000000000000033 }], unknowns: 4 }, curve: Circle(Circle { center: [3.0, 0.0, 0.0], radius: 1.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, -1.0, 0.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[3.0, 0.0, 0.0], [1.0, 0.0, 0.0], [-0.0, 0.0, 1.0], [0.0, -0.0, 0.0]], exact: [[Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 1.0, hi: 1.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "Ok(None)",
    "Ok(None)",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.1010788877616233, hi: 1.7533419930744358 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -0.5, hi: -0.07680261833334258 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -0.9860132971832694], radius: 2.8333333333333335, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, -0.9860132971832694], [2.8333333333333335, 0.0, 0.0], [0.0, 2.8333333333333335, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -0.9860132971832694, hi: -0.9860132971832694 }], [Interval { lo: 2.8333333333333335, hi: 2.8333333333333335 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.8333333333333335, hi: 2.8333333333333335 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.24665800692556436, hi: 0.8989211122383763 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.07680261833334258, hi: 0.5 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.9860132971832694], radius: 2.8333333333333335, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.9860132971832694], [2.8333333333333335, 0.0, 0.0], [0.0, 2.8333333333333335, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.9860132971832694, hi: 0.9860132971832694 }], [Interval { lo: 2.8333333333333335, hi: 2.8333333333333335 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.8333333333333335, hi: 2.8333333333333335 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.324757295032693, hi: 0.9107546143419116 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -0.5, hi: -0.012267822785722096 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.8638734908183773], radius: 2.496291163606126, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.8638734908183773], [2.496291163606126, 0.0, 0.0], [0.0, 2.496291163606126, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.8638734908183773, hi: 0.8638734908183773 }], [Interval { lo: 2.496291163606126, hi: 2.496291163606126 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.496291163606126, hi: 2.496291163606126 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.041031966275694, hi: 1.4510623702558312 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -0.5, hi: -0.1404905524032256 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -0.3888734908183773], radius: 2.0787088363938744, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, -0.3888734908183773], [2.0787088363938744, 0.0, 0.0], [0.0, 2.0787088363938744, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -0.3888734908183773, hi: -0.3888734908183773 }], [Interval { lo: 2.0787088363938744, hi: 2.0787088363938744 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0787088363938744, hi: 2.0787088363938744 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.089456187511033, hi: 1.8669471896464274 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -0.866025403784439, hi: -0.8660254037844382 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -0.8660254037844386], radius: 3.5, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, -0.8660254037844386], [3.5, 0.0, 0.0], [0.0, 3.5, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -0.8660254037844386, hi: -0.8660254037844386 }], [Interval { lo: 3.5, hi: 3.5 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.5, hi: 3.5 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.13305281035357297, hi: 0.9105438124889669 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.8660254037844382, hi: 0.866025403784439 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.8660254037844386], radius: 3.5, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.8660254037844386], [3.5, 0.0, 0.0], [0.0, 3.5, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.8660254037844386, hi: 0.8660254037844386 }], [Interval { lo: 3.5, hi: 3.5 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.5, hi: 3.5 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.0313044117802863, hi: 1.388442547497091 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.7040079876288536, hi: 1.7040079876288554 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -0.2959920123711456], radius: 2.044809585154625, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, -0.2959920123711456], [2.044809585154625, 0.0, 0.0], [0.0, 2.044809585154625, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -0.2959920123711456, hi: -0.2959920123711456 }], [Interval { lo: 2.044809585154625, hi: 2.044809585154625 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.044809585154625, hi: 2.044809585154625 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.13904292650930658, hi: 0.9085757409901021 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 2.886155946797373, hi: 2.8861559467973765 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.8861559467973747], radius: 3.46338713615685, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.8861559467973747], [3.46338713615685, 0.0, 0.0], [0.0, 3.46338713615685, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.8861559467973747, hi: 0.8861559467973747 }], [Interval { lo: 3.46338713615685, hi: 3.46338713615685 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.46338713615685, hi: 3.46338713615685 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.06364167792230506, hi: 0.9474315432887469 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.052568456711253, hi: 1.9363583220776952 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.5000000000000001], radius: 3.8660254037844384, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.5000000000000001], [3.8660254037844384, 0.0, 0.0], [0.0, 3.8660254037844384, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.5000000000000001, hi: 0.5000000000000001 }], [Interval { lo: 3.8660254037844384, hi: 3.8660254037844384 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.8660254037844384, hi: 3.8660254037844384 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.4886120310229627, hi: 0.9474315432887469 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.052568456711253, hi: 1.5113879689770375 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.5000000000000001], radius: 2.1339745962155616, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.5000000000000001], [2.1339745962155616, 0.0, 0.0], [0.0, 2.1339745962155616, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.5000000000000001, hi: 0.5000000000000001 }], [Interval { lo: 2.1339745962155616, hi: 2.1339745962155616 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.1339745962155616, hi: 2.1339745962155616 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "Ok(None)",
    "Ok(None)",
    "Ok(None)",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: -2.000000000000006, hi: 2.000000000000006 }, Interval { lo: -2.000000000000007, hi: 2.000000000000007 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 2.0], radius: 2.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 2.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0, hi: 2.0 }], [Interval { lo: 2.0, hi: 2.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0, hi: 2.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: -3.000000000000008, hi: -0.9999999999999976 }, Interval { lo: -1.24e-322, hi: 4.000000000000012 }], unknowns: 4 }, curve: Line(Line { origin: [-1.0000000000000002, 0.0, 0.0], direction: [-1.9999999999999996, 0.0, 4.000000000000001], domain: (0.0, 1.0), form: Form { shape: Linear, doubles: [[-1.0000000000000002, 0.0, 0.0], [-1.9999999999999996, 0.0, 4.000000000000001], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: -1.0000000000000002, hi: -1.0000000000000002 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: -1.9999999999999996, hi: -1.9999999999999996 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.000000000000001, hi: 4.000000000000001 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.9999999999999976, hi: 3.000000000000008 }, Interval { lo: -1.24e-322, hi: 4.000000000000012 }], unknowns: 4 }, curve: Line(Line { origin: [1.0000000000000002, 0.0, 0.0], direction: [1.9999999999999996, 0.0, 4.000000000000001], domain: (0.0, 1.0), form: Form { shape: Linear, doubles: [[1.0000000000000002, 0.0, 0.0], [1.9999999999999996, 0.0, 4.000000000000001], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 1.9999999999999996, hi: 1.9999999999999996 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.000000000000001, hi: 4.000000000000001 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.20165309451863986, hi: 0.5 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 2.1612496949731392], radius: 2.08062484748657, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 2.1612496949731392], [2.08062484748657, 0.0, 0.0], [0.0, 2.08062484748657, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.1612496949731392, hi: 2.1612496949731392 }], [Interval { lo: 2.08062484748657, hi: 2.08062484748657 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.08062484748657, hi: 2.08062484748657 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.9999999999999993, hi: 2.000000000000002 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 2.0000000000000004], radius: 2.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 2.0000000000000004], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0000000000000004, hi: 2.0000000000000004 }], [Interval { lo: 2.0, hi: 2.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0, hi: 2.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: -3.5000000000000084, hi: 3.5000000000000084 }, Interval { lo: -3.5000000000000093, hi: 3.5000000000000093 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 1.0], radius: 3.5, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 1.0], [3.5, 0.0, 0.0], [0.0, 3.5, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 3.5, hi: 3.5 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.5, hi: 3.5 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: -4.000000000000013, hi: 4.000000000000013 }, Interval { lo: -4.000000000000015, hi: 4.000000000000015 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 2.0], radius: 4.000000000000001, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 2.0], [4.000000000000001, 0.0, 0.0], [0.0, 4.000000000000001, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0, hi: 2.0 }], [Interval { lo: 4.000000000000001, hi: 4.000000000000001 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.000000000000001, hi: 4.000000000000001 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: -6.000000000000013, hi: -1.9999999999999951 }, Interval { lo: -2e-322, hi: 4.000000000000012 }], unknowns: 4 }, curve: Trimmed(Trimmed { basis: Nurbs(Curve { basis: Basis { degree: 2, knots: [0.0, 0.0, 0.0, 1.0, 1.0, 1.0] }, points: [[0.0, -2.000000000000001, 0.0], [0.0, -6.0, 2.0000000000000004], [0.0, -2.0000000000000004, 4.000000000000001]], weights: None }), interval: (0.0, 1.0) }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 1.9999999999999951, hi: 6.000000000000013 }, Interval { lo: -2e-322, hi: 4.000000000000012 }], unknowns: 4 }, curve: Trimmed(Trimmed { basis: Nurbs(Curve { basis: Basis { degree: 2, knots: [0.0, 0.0, 0.0, 1.0, 1.0, 1.0] }, points: [[0.0, 2.000000000000001, 0.0], [0.0, 6.0, 2.0000000000000004], [0.0, 2.0000000000000004, 4.000000000000001]], weights: None }), interval: (0.0, 1.0) }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.5857700404229973, hi: 0.5857700404229982 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.5857700404229977], radius: 3.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.5857700404229977], [3.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.5857700404229977, hi: 0.5857700404229977 }], [Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 3.4142299595770007, hi: 3.4142299595770043 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 3.4142299595770025], radius: 3.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 3.4142299595770025], [3.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.4142299595770025, hi: 3.4142299595770025 }], [Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: 1.4999999999999958, hi: 1.500000000000003 }, Interval { lo: -9e-323, hi: 10.000000000000025 }], unknowns: 4 }, curve: Line(Line { origin: [2.0000000000000004, 1.4999999999999998, 0.0], direction: [0.0, 0.0, 1.0], domain: (0.0, 10.0), form: Form { shape: Linear, doubles: [[2.0000000000000004, 1.4999999999999998, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 2.0000000000000004, hi: 2.0000000000000004 }, Interval { lo: 1.4999999999999998, hi: 1.4999999999999998 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: -1.24e-322, hi: 4.000000000000013 }, Interval { lo: -1.5e-322, hi: 3.000000000000008 }], unknowns: 4 }, curve: Line(Line { origin: [0.0, 0.0, 4.999999999999999], direction: [4.000000000000001, 3.0, 0.0], domain: (0.0, 1.0), form: Form { shape: Linear, doubles: [[0.0, 0.0, 4.999999999999999], [4.000000000000001, 3.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.999999999999999, hi: 4.999999999999999 }], [Interval { lo: 4.000000000000001, hi: 4.000000000000001 }, Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: -1e-322, hi: 4.000000000000013 }, Interval { lo: -1.5e-322, hi: 3.000000000000008 }], unknowns: 4 }, curve: Line(Line { origin: [0.0, 0.0, 5.0], direction: [4.000000000000001, 3.0, 2.000000000000001], domain: (0.0, 1.0), form: Form { shape: Linear, doubles: [[0.0, 0.0, 5.0], [4.000000000000001, 3.0, 2.000000000000001], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 5.0, hi: 5.0 }], [Interval { lo: 4.000000000000001, hi: 4.000000000000001 }, Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 2.000000000000001, hi: 2.000000000000001 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 0.5 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: 3.9999999969042803, hi: 3.9999999969042985 }, Interval { lo: -1.5e-322, hi: 10.000000000000025 }], unknowns: 4 }, curve: Line(Line { origin: [2.9999999992545714, 3.9999999969042905, 0.0], direction: [0.0, 0.0, 1.0], domain: (0.0, 10.0), form: Form { shape: Linear, doubles: [[2.9999999992545714, 3.9999999969042905, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 2.9999999992545714, hi: 2.9999999992545714 }, Interval { lo: 3.9999999969042905, hi: 3.9999999969042905 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 0.5 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: -1.73e-322, hi: 5.000000000000014 }, Interval { lo: -1.73e-322, hi: 5.000000000000016 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 4.000000000000001], radius: 5.0, frame: Frame { x: [5.0, 0.0, 0.0], normal: [0.0, 0.0, 25.0] }, domain: (0.0, 0.5), form: Form { shape: Trig, doubles: [[0.0, 0.0, 4.000000000000001], [5.0, 0.0, 0.0], [0.0, 5.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.000000000000001, hi: 4.000000000000001 }], [Interval { lo: 5.0, hi: 5.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 5.0, hi: 5.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 0.5 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: -1.4e-322, hi: 5.000000000000015 }, Interval { lo: -1.73e-322, hi: 5.000000000000015 }], unknowns: 4 }, curve: Ellipse(Ellipse { center: [0.0, 0.0, 5.0], axes: Vectors { u: [5.0, 0.0, 2.5000000000000004], v: [0.0, 5.0, 0.0] }, domain: (0.0, 0.5), form: Form { shape: Trig, doubles: [[0.0, 0.0, 5.0], [5.0, 0.0, 2.5000000000000004], [0.0, 5.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 5.0, hi: 5.0 }], [Interval { lo: 5.0, hi: 5.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.5000000000000004, hi: 2.5000000000000004 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 5.0, hi: 5.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 0.5 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 10.0 }], unknowns: 4 }, curve: Line(Line { origin: [3.5355339059327378, 3.5355339059327378, 0.0], direction: [0.0, 0.0, 1.0], domain: (0.0, 10.000000000000002), form: Form { shape: Linear, doubles: [[3.5355339059327378, 3.5355339059327378, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 3.5355339059327378, hi: 3.5355339059327378 }, Interval { lo: 3.5355339059327378, hi: 3.5355339059327378 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 10.0 }], unknowns: 4 }, curve: Line(Line { origin: [2.0000000000000018, 1.5, 0.0], direction: [0.0, 0.0, 1.0], domain: (0.0, 10.000000000000002), form: Form { shape: Linear, doubles: [[2.0000000000000018, 1.5, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 2.0000000000000018, hi: 2.0000000000000018 }, Interval { lo: 1.5, hi: 1.5 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.05006316554749434, hi: 0.8524163823495671 }, Interval { lo: -2.936491673103716, hi: 2.936491673103716 }, Interval { lo: -2.936491673103717, hi: 2.936491673103717 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.5000000000000001], radius: 2.9364916731037085, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.5000000000000001], [2.9364916731037085, 0.0, 0.0], [0.0, 2.9364916731037085, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.5000000000000001, hi: 0.5000000000000001 }], [Interval { lo: 2.9364916731037085, hi: 2.9364916731037085 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.9364916731037085, hi: 2.9364916731037085 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Point { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.9999999999999999, hi: 1.0000000000000002 }, Interval { lo: -6e-323, hi: 6e-323 }, Interval { lo: -6e-323, hi: 6e-323 }], unknowns: 4 }, xyz: Box3 { min: P3 { x: Coord(-1), y: Coord(-1), z: Coord(-1) }, max: P3 { x: Coord(1), y: Coord(1), z: Coord(1) } }, contact: IsolatedTangent, class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0000000000000004 }, Interval { lo: -2.000000000000006, hi: 2.000000000000006 }, Interval { lo: -2.000000000000007, hi: 2.000000000000007 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.0], radius: 2.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 2.0, hi: 2.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0, hi: 2.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(1), step: Q0 } }] TopologyCertified",
    "[Point { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.9999999999999999, hi: 1.0000000000000002 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -4.4e-323, hi: 4.4e-323 }], unknowns: 4 }, xyz: Box3 { min: P3 { x: Coord(-1), y: Coord(-1), z: Coord(-1) }, max: P3 { x: Coord(1), y: Coord(1), z: Coord(1) } }, contact: IsolatedTangent, class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.2499999999999993, hi: 1.625000000000001 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -1.0000000000000009, hi: -0.9999999999999996 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -1.0], radius: 1.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -1.0, hi: -1.0 }], [Interval { lo: 1.0, hi: 1.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(1), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.3749999999999991, hi: 0.7500000000000004 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.9999999999999996, hi: 1.0000000000000009 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 1.0], radius: 1.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 1.0, hi: 1.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(1), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.9999999999999999, hi: 1.0000000000000002 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -0.5, hi: 0.5 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.0], radius: 2.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 2.0, hi: 2.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0, hi: 2.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Tangent { band: Steps(0), gap: StepsInterval { lo: Steps(0), hi: Steps(0) } }, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.21569416631608382, hi: 0.8975836176504337 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.9999999999999996, hi: 1.0 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 1.0], radius: 3.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 1.0], [3.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(1), step: Q0 } }] TopologyCertified",
    "[] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.089456187511033, hi: 1.8669471896464274 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.8660254037844382, hi: 0.866025403784439 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -0.8660254037844386], radius: 3.5, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, -0.8660254037844386], [3.5, 0.0, 0.0], [0.0, 3.5, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -0.8660254037844386, hi: -0.8660254037844386 }], [Interval { lo: 3.5, hi: 3.5 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.5, hi: 3.5 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.13904292650930658, hi: 0.9085757409901021 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -2.8861559467973765, hi: -2.886155946797373 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 0.8861559467973747], radius: 3.46338713615685, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 0.8861559467973747], [3.46338713615685, 0.0, 0.0], [0.0, 3.46338713615685, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.8861559467973747, hi: 0.8861559467973747 }], [Interval { lo: 3.46338713615685, hi: 3.46338713615685 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.46338713615685, hi: 3.46338713615685 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.0313044117802863, hi: 1.388442547497091 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -1.7040079876288554, hi: -1.7040079876288536 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -0.2959920123711456], radius: 2.044809585154625, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, -0.2959920123711456], [2.044809585154625, 0.0, 0.0], [0.0, 2.044809585154625, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -0.2959920123711456, hi: -0.2959920123711456 }], [Interval { lo: 2.044809585154625, hi: 2.044809585154625 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.044809585154625, hi: 2.044809585154625 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "Ok(None)",
    "Ok(None)",
    "Ok(None)",
    "Ok(None)",
    "Ok(None)",
    "Ok(None)",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: -1.5000000000000036, hi: 1.5000000000000036 }, Interval { lo: -1.500000000000004, hi: 1.500000000000004 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 2.0], radius: 1.4999999999999996, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 2.0], [1.4999999999999996, 0.0, 0.0], [0.0, 1.4999999999999996, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0, hi: 2.0 }], [Interval { lo: 1.4999999999999996, hi: 1.4999999999999996 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.4999999999999996, hi: 1.4999999999999996 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Point { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -4.4e-323, hi: 1e-322 }], unknowns: 4 }, xyz: Box3 { min: P3 { x: Coord(-1), y: Coord(-1), z: Coord(-1) }, max: P3 { x: Coord(1), y: Coord(1), z: Coord(1) } }, contact: IsolatedTangent, class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: -1.5000000000000033, hi: 1.5000000000000033 }, Interval { lo: -1.5000000000000038, hi: 1.5000000000000038 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 2.0], radius: 1.4999999999999993, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 2.0], [1.4999999999999993, 0.0, 0.0], [0.0, 1.4999999999999993, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0, hi: 2.0 }], [Interval { lo: 1.4999999999999993, hi: 1.4999999999999993 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.4999999999999993, hi: 1.4999999999999993 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: -3.0000000000000058, hi: 1.5e-322 }, Interval { lo: -1.24e-322, hi: 4.000000000000012 }], unknowns: 4 }, curve: Line(Line { origin: [0.0, 0.0, 0.0], direction: [-2.999999999999999, 0.0, 4.000000000000001], domain: (0.0, 1.0), form: Form { shape: Linear, doubles: [[0.0, 0.0, 0.0], [-2.999999999999999, 0.0, 4.000000000000001], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: -2.999999999999999, hi: -2.999999999999999 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.000000000000001, hi: 4.000000000000001 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: -1.5e-322, hi: 3.0000000000000058 }, Interval { lo: -1.24e-322, hi: 4.000000000000012 }], unknowns: 4 }, curve: Line(Line { origin: [0.0, 0.0, 0.0], direction: [2.999999999999999, 0.0, 4.000000000000001], domain: (0.0, 1.0), form: Form { shape: Linear, doubles: [[0.0, 0.0, 0.0], [2.999999999999999, 0.0, 4.000000000000001], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 2.999999999999999, hi: 2.999999999999999 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.000000000000001, hi: 4.000000000000001 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: -3.000000000000007, hi: 8.881784197001278e-16 }, Interval { lo: -1.24e-322, hi: 4.000000000000012 }], unknowns: 4 }, curve: Line(Line { origin: [0.0, -3.0000000000000004, 0.0], direction: [0.0, 3.0000000000000013, 4.000000000000001], domain: (0.0, 1.0), form: Form { shape: Linear, doubles: [[0.0, -3.0000000000000004, 0.0], [0.0, 3.0000000000000013, 4.000000000000001], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -3.0000000000000004, hi: -3.0000000000000004 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0000000000000013, hi: 3.0000000000000013 }, Interval { lo: 4.000000000000001, hi: 4.000000000000001 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: -8.881784197001278e-16, hi: 3.000000000000007 }, Interval { lo: -1.24e-322, hi: 4.000000000000012 }], unknowns: 4 }, curve: Line(Line { origin: [0.0, 3.0000000000000004, 0.0], direction: [0.0, -3.0000000000000013, 4.000000000000001], domain: (0.0, 1.0), form: Form { shape: Linear, doubles: [[0.0, 3.0000000000000004, 0.0], [0.0, -3.0000000000000013, 4.000000000000001], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0000000000000004, hi: 3.0000000000000004 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -3.0000000000000013, hi: -3.0000000000000013 }, Interval { lo: 4.000000000000001, hi: 4.000000000000001 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.6, hi: 1.0 }, Interval { lo: -3.500000000000009, hi: 3.500000000000009 }, Interval { lo: -3.5000000000000098, hi: 3.5000000000000098 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 3.0], radius: 3.5000000000000004, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 3.0], [3.5000000000000004, 0.0, 0.0], [0.0, 3.5000000000000004, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0, hi: 3.0 }], [Interval { lo: 3.5000000000000004, hi: 3.5000000000000004 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.5000000000000004, hi: 3.5000000000000004 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.6, hi: 1.0 }, Interval { lo: -3.9200000000000164, hi: -1.9999999999999951 }, Interval { lo: 2.399999999999989, hi: 4.000000000000012 }], unknowns: 4 }, curve: Trimmed(Trimmed { basis: Nurbs(Curve { basis: Basis { degree: 2, knots: [0.0, 0.0, 0.0, 1.0, 1.0, 1.0] }, points: [[-2.0000000000000004, 0.0, 0.0], [-6.0, 0.0, 2.0000000000000004], [-2.0000000000000004, 0.0, 4.000000000000001]], weights: None }), interval: (0.6, 1.0) }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.6, hi: 1.0 }, Interval { lo: 1.9999999999999951, hi: 3.9200000000000164 }, Interval { lo: 2.399999999999989, hi: 4.000000000000012 }], unknowns: 4 }, curve: Trimmed(Trimmed { basis: Nurbs(Curve { basis: Basis { degree: 2, knots: [0.0, 0.0, 0.0, 1.0, 1.0, 1.0] }, points: [[2.0000000000000004, 0.0, 0.0], [6.0, 0.0, 2.0000000000000004], [2.0000000000000004, 0.0, 4.000000000000001]], weights: None }), interval: (0.6, 1.0) }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "Ok(None)",
    "Ok(None)",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: -4.000000000000013, hi: 4.000000000000013 }, Interval { lo: -4.000000000000015, hi: 4.000000000000015 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 2.0], radius: 4.000000000000001, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, -1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 2.0], [4.000000000000001, 0.0, 0.0], [0.0, -4.000000000000001, 0.0], [0.0, 0.0, -0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0, hi: 2.0 }], [Interval { lo: 4.000000000000001, hi: 4.000000000000001 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -4.000000000000001, hi: -4.000000000000001 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "Ok(None)",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: -1.6666666666666712, hi: 1.6666666666666738 }, Interval { lo: -8e-323, hi: 8e-323 }], unknowns: 4 }, curve: Line(Line { origin: [0.0, 0.0, -0.9999999999999993], direction: [4.000000000000001, 0.0, 11.999999999999998], domain: (0.0833333333333333, 0.9166666666666669), form: Form { shape: Linear, doubles: [[0.0, 0.0, -0.9999999999999993], [4.000000000000001, 0.0, 11.999999999999998], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -0.9999999999999993, hi: -0.9999999999999993 }], [Interval { lo: 4.000000000000001, hi: 4.000000000000001 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 11.999999999999998, hi: 11.999999999999998 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 0.5 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: -0.04314724608053228, hi: 2.715736230402663 }, Interval { lo: -0.04314724608053213, hi: 2.7157362304026633 }], unknowns: 4 }, curve: Ellipse(Ellipse { center: [0.0, 0.0, 5.0], axes: Vectors { u: [5.0, 0.0, -2.5000000000000004], v: [0.0, 5.0, -2.5000000000000004] }, domain: (0.0, 0.5), form: Form { shape: Trig, doubles: [[0.0, 0.0, 5.0], [5.0, 0.0, -2.5000000000000004], [0.0, 5.0, -2.5000000000000004], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 5.0, hi: 5.0 }], [Interval { lo: 5.0, hi: 5.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -2.5000000000000004, hi: -2.5000000000000004 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 5.0, hi: 5.0 }, Interval { lo: -2.5000000000000004, hi: -2.5000000000000004 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 0.5 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: -1e-322, hi: 5.000000000000014 }, Interval { lo: -4e-323, hi: 2.500000000000008 }], unknowns: 4 }, curve: Ellipse(Ellipse { center: [0.0, 0.0, 5.0], axes: Vectors { u: [5.0, 0.0, 0.0], v: [0.0, 5.0, -2.5000000000000004] }, domain: (0.0, 0.5), form: Form { shape: Trig, doubles: [[0.0, 0.0, 5.0], [5.0, 0.0, 0.0], [0.0, 5.0, -2.5000000000000004], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 5.0, hi: 5.0 }], [Interval { lo: 5.0, hi: 5.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 5.0, hi: 5.0 }, Interval { lo: -2.5000000000000004, hi: -2.5000000000000004 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 0.5 }, Interval { lo: 0.0, hi: 3.0 }, Interval { lo: -0.18518518518518934, hi: 1.9830007282406374 }, Interval { lo: -0.1851851851851908, hi: 1.9830007282406363 }], unknowns: 4 }, curve: Circle(Circle { center: [2.5, -5.0, 5.0], radius: 5.0, frame: Frame { x: [3.3333333333333335, 3.3333333333333335, 1.6666666666666667], normal: [8.333333333333332, -16.666666666666668, 16.666666666666668] }, domain: (0.0, 0.5), form: Form { shape: Trig, doubles: [[2.5, -5.0, 5.0], [3.333333333333334, 3.333333333333333, 1.666666666666667], [-3.333333333333333, 1.666666666666667, 3.333333333333333], [0.0, -0.0, 0.0]], exact: [[Interval { lo: 2.5, hi: 2.5 }, Interval { lo: -5.0, hi: -5.0 }, Interval { lo: 5.0, hi: 5.0 }], [Interval { lo: 3.333333333333326, hi: 3.3333333333333406 }, Interval { lo: 3.333333333333326, hi: 3.33333333333334 }, Interval { lo: 1.6666666666666627, hi: 1.6666666666666707 }], [Interval { lo: -3.3333333333333375, hi: -3.33333333333333 }, Interval { lo: 1.666666666666665, hi: 1.666666666666669 }, Interval { lo: 3.3333333333333295, hi: 3.333333333333337 }], [Interval { lo: -5e-324, hi: 5e-324 }, Interval { lo: -5e-324, hi: 5e-324 }, Interval { lo: -5e-324, hi: 5e-324 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 1.0, hi: 4.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 10.0 }], unknowns: 4 }, curve: Line(Line { origin: [2.0000000000000018, 1.5, 0.0], direction: [0.0, 0.0, 1.0], domain: (2.0000000000000004, 8.000000000000002), form: Form { shape: Linear, doubles: [[2.0000000000000018, 1.5, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 2.0000000000000018, hi: 2.0000000000000018 }, Interval { lo: 1.5, hi: 1.5 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 2.0, hi: 6.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 10.0 }], unknowns: 4 }, curve: Line(Line { origin: [2.0000000000000018, 1.5, 0.0], direction: [0.0, 0.0, 1.0], domain: (3.9999999999999982, 8.0), form: Form { shape: Linear, doubles: [[2.0000000000000018, 1.5, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 2.0000000000000018, hi: 2.0000000000000018 }, Interval { lo: 1.5, hi: 1.5 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: 0.20483276469913306, hi: 0.20483276469913356 }, Interval { lo: -3.5e-323, hi: 10.000000000000009 }], unknowns: 4 }, curve: Line(Line { origin: [2.4000000000000012, 1.7999999999999992, 0.0], direction: [0.0, 0.0, 1.0], domain: (0.0, 10.000000000000002), form: Form { shape: Linear, doubles: [[2.4000000000000012, 1.7999999999999992, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 2.4000000000000012, hi: 2.4000000000000012 }, Interval { lo: 1.7999999999999992, hi: 1.7999999999999992 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 0.5 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: 0.693432958871339, hi: 0.6934329588713398 }, Interval { lo: 1.0, hi: 5.0 }], unknowns: 4 }, curve: Line(Line { origin: [1.858058909292495, 4.641941090707505, 0.0], direction: [0.0, 0.0, 1.0], domain: (3.000000000000001, 7.000000000000002), form: Form { shape: Linear, doubles: [[1.858058909292495, 4.641941090707505, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 1.858058909292495, hi: 1.858058909292495 }, Interval { lo: 4.641941090707505, hi: 4.641941090707505 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 0.5 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: 1.80656704112866, hi: 1.806567041128661 }, Interval { lo: 1.0, hi: 5.0 }], unknowns: 4 }, curve: Line(Line { origin: [4.6419410907075065, 1.8580589092924935, 0.0], direction: [0.0, 0.0, 1.0], domain: (3.000000000000001, 7.000000000000002), form: Form { shape: Linear, doubles: [[4.6419410907075065, 1.8580589092924935, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 4.6419410907075065, hi: 4.6419410907075065 }, Interval { lo: 1.8580589092924935, hi: 1.8580589092924935 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: 1.7951672353008663, hi: 1.7951672353008672 }, Interval { lo: -3.0, hi: 3.5e-323 }], unknowns: 4 }, curve: Line(Line { origin: [2.4000000000000012, 1.7999999999999992, 0.0], direction: [0.0, 0.0, 1.0], domain: (0.0, 3.000000000000001), form: Form { shape: Linear, doubles: [[2.4000000000000012, 1.7999999999999992, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 2.4000000000000012, hi: 2.4000000000000012 }, Interval { lo: 1.7999999999999992, hi: 1.7999999999999992 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "Ok(None)",
    "[] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.011991974714848426, hi: 0.9894068540950576 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -9.987492177719096, hi: -9.987492177719082 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 9.987492177719089], radius: 399.5, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 9.987492177719089], [399.5, 0.0, 0.0], [0.0, 399.5, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 9.987492177719089, hi: 9.987492177719089 }], [Interval { lo: 399.5, hi: 399.5 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 399.5, hi: 399.5 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.0105931459049422, hi: 1.9880080252851517 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 9.987492177719082, hi: 9.987492177719096 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -9.987492177719089], radius: 399.5, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, -9.987492177719089], [399.5, 0.0, 0.0], [0.0, 399.5, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -9.987492177719089, hi: -9.987492177719089 }], [Interval { lo: 399.5, hi: 399.5 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 399.5, hi: 399.5 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 5.0 }, Interval { lo: -4.424242424242445, hi: 5.42424242424245 }, Interval { lo: -8.636363636363676, hi: 13.636363636363697 }], unknowns: 4 }, curve: Line(Line { origin: [5.5, 5.5, 11.0], direction: [-2.000000000000001, -6.0, -12.0], domain: (0.08333333333333326, 0.9166666666666667), form: Form { shape: Linear, doubles: [[5.5, 5.5, 11.0], [-2.000000000000001, -6.0, -12.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 5.5, hi: 5.5 }, Interval { lo: 5.5, hi: 5.5 }, Interval { lo: 11.0, hi: 11.0 }], [Interval { lo: -2.000000000000001, hi: -2.000000000000001 }, Interval { lo: -6.0, hi: -6.0 }, Interval { lo: -12.0, hi: -12.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 1.0, hi: 4.0 }, Interval { lo: 0.0, hi: 1.0 }, Interval { lo: 0.0, hi: 30.0 }], unknowns: 4 }, curve: Line(Line { origin: [2.0000000000000018, 1.5, 0.0], direction: [0.0, 0.0, 1.0], domain: (12.0, 18.0), form: Form { shape: Linear, doubles: [[2.0000000000000018, 1.5, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 2.0000000000000018, hi: 2.0000000000000018 }, Interval { lo: 1.5, hi: 1.5 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 0.5 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: -1e-322, hi: 4.000000000000013 }, Interval { lo: -1.5e-322, hi: 2.0000000000000075 }], unknowns: 4 }, curve: Ellipse(Ellipse { center: [0.0, 0.0, 4.000000000000001], axes: Vectors { u: [4.000000000000001, 0.0, 0.0], v: [0.0, 2.0000000000000004, 0.0] }, domain: (0.0, 0.5), form: Form { shape: Trig, doubles: [[0.0, 0.0, 4.000000000000001], [4.000000000000001, 0.0, 0.0], [0.0, 2.0000000000000004, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.000000000000001, hi: 4.000000000000001 }], [Interval { lo: 4.000000000000001, hi: 4.000000000000001 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0000000000000004, hi: 2.0000000000000004 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: -1.0, hi: 1.0 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: 1.9999999999999944, hi: 3.086161269630499 }, Interval { lo: -1.1752011936438063, hi: 1.1752011936438063 }], unknowns: 4 }, curve: Hyperbola(Hyperbola { center: [0.0, 0.0, 4.000000000000001], axes: Vectors { u: [2.0000000000000004, 0.0, 0.0], v: [0.0, 1.0000000000000002, 0.0] }, domain: (-1.0, 1.0), form: Form { shape: Hyperbolic, doubles: [[0.0, 0.0, 4.000000000000001], [2.0000000000000004, 0.0, 0.0], [0.0, 1.0000000000000002, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.000000000000001, hi: 4.000000000000001 }], [Interval { lo: 2.0000000000000004, hi: 2.0000000000000004 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: -1.0, hi: 1.0 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: -1e-322, hi: 1.0000000000000033 }, Interval { lo: -2.0000000000000067, hi: 2.0000000000000067 }], unknowns: 4 }, curve: Parabola(Parabola { vertex: [0.0, 0.0, 4.000000000000001], axes: Vectors { u: [1.0000000000000002, 0.0, 0.0], v: [0.0, 2.0000000000000004, 0.0] }, domain: (-1.0, 1.0), form: Form { shape: Parabolic, doubles: [[0.0, 0.0, 4.000000000000001], [1.0000000000000002, 0.0, 0.0], [0.0, 2.0000000000000004, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.000000000000001, hi: 4.000000000000001 }], [Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0000000000000004, hi: 2.0000000000000004 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -5.866025403784452, hi: 5.866025403784452 }, Interval { lo: -5.8660254037844535, hi: 5.8660254037844535 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 1.0], radius: 5.866025403784437, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 1.0], [5.866025403784437, 0.0, 0.0], [0.0, 5.866025403784437, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 5.866025403784437, hi: 5.866025403784437 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 5.866025403784437, hi: 5.866025403784437 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -4.133974596215573, hi: 4.133974596215573 }, Interval { lo: -4.133974596215575, hi: 4.133974596215575 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 1.0], radius: 4.133974596215561, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 1.0], [4.133974596215561, 0.0, 0.0], [0.0, 4.133974596215561, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 4.133974596215561, hi: 4.133974596215561 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.133974596215561, hi: 4.133974596215561 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 0.5 }, Interval { lo: 0.0, hi: 4.0 }, Interval { lo: 0.33333333333333287, hi: 8.666666666666694 }, Interval { lo: -0.3333333333333343, hi: 1.3333333333333373 }], unknowns: 4 }, curve: Ellipse(Ellipse { center: [3.666666666666667, 3.666666666666667, 3.666666666666667], axes: Vectors { u: [1.6666666666666665, -3.333333333333333, -3.333333333333333], v: [0.0, 5.0, 0.0] }, domain: (0.0, 0.5), form: Form { shape: Trig, doubles: [[3.666666666666667, 3.666666666666667, 3.666666666666667], [1.6666666666666665, -3.333333333333333, -3.333333333333333], [0.0, 5.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 3.666666666666667, hi: 3.666666666666667 }, Interval { lo: 3.666666666666667, hi: 3.666666666666667 }, Interval { lo: 3.666666666666667, hi: 3.666666666666667 }], [Interval { lo: 1.6666666666666665, hi: 1.6666666666666665 }, Interval { lo: -3.333333333333333, hi: -3.333333333333333 }, Interval { lo: -3.333333333333333, hi: -3.333333333333333 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 5.0, hi: 5.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "Ok(None)",
    "Ok(None)",
    "Err(FrameRange)",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.5, hi: 2.5 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: -40.0, hi: -29.999999999596977 }, Interval { lo: -40.0, hi: -29.999999999996557 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 4.999999999999999], radius: 50.0, frame: Frame { x: [50.0, 0.0, 0.0], normal: [0.0, 0.0, 2500.0] }, domain: (1.2048327646991068, 1.2951672353040729), form: Form { shape: Trig, doubles: [[0.0, 0.0, 4.999999999999999], [50.0, 0.0, 0.0], [0.0, 50.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.999999999999999, hi: 4.999999999999999 }], [Interval { lo: 50.0, hi: 50.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 50.0, hi: 50.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.5, hi: 2.5 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: -40.0, hi: -29.99999999999934 }, Interval { lo: 29.999999999999858, hi: 40.0 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 4.999999999999999], radius: 50.0, frame: Frame { x: [50.0, 0.0, 0.0], normal: [0.0, 0.0, 2500.0] }, domain: (0.704832764699129, 0.7951672353008669), form: Form { shape: Trig, doubles: [[0.0, 0.0, 4.999999999999999], [50.0, 0.0, 0.0], [0.0, 50.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.999999999999999, hi: 4.999999999999999 }], [Interval { lo: 50.0, hi: 50.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 50.0, hi: 50.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.5, hi: 2.5 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: 29.999999999596977, hi: 40.0 }, Interval { lo: -40.0, hi: -29.999999999996557 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 4.999999999999999], radius: 50.0, frame: Frame { x: [50.0, 0.0, 0.0], normal: [0.0, 0.0, 2500.0] }, domain: (1.7048327646959271, 1.7951672353008932), form: Form { shape: Trig, doubles: [[0.0, 0.0, 4.999999999999999], [50.0, 0.0, 0.0], [0.0, 50.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.999999999999999, hi: 4.999999999999999 }], [Interval { lo: 50.0, hi: 50.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 50.0, hi: 50.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.5, hi: 2.5 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: 29.99999999999934, hi: 40.0 }, Interval { lo: 29.999999999999847, hi: 40.0 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 4.999999999999999], radius: 50.0, frame: Frame { x: [50.0, 0.0, 0.0], normal: [0.0, 0.0, 2500.0] }, domain: (2.204832764699133, 2.295167235300871), form: Form { shape: Trig, doubles: [[0.0, 0.0, 4.999999999999999], [50.0, 0.0, 0.0], [0.0, 50.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.999999999999999, hi: 4.999999999999999 }], [Interval { lo: 50.0, hi: 50.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 50.0, hi: 50.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0000000000000004 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -0.5, hi: 0.5 }], unknowns: 4 }, curve: Circle(Circle { center: [0.5299366043185282, 1.8119746417274114, 2.1538858877733507], radius: 2.8333333333333335, frame: Frame { x: [0.0, 1.0, 0.0], normal: [0.4767312946227962, 0.19069251784911845, 0.8581163303210331] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.5299366043185282, 1.8119746417274114, 2.1538858877733507], [-0.26239067441255864, 2.7813411487731217, -0.47230321394260544], [-2.476778949011024, 0.0, 1.3759883050061243], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.5299366043185282, hi: 0.5299366043185282 }, Interval { lo: 1.8119746417274114, hi: 1.8119746417274114 }, Interval { lo: 2.1538858877733507, hi: 2.1538858877733507 }], [Interval { lo: -0.2623906744125589, hi: -0.26239067441255837 }, Interval { lo: 2.7813411487731186, hi: 2.7813411487731243 }, Interval { lo: -0.47230321394260594, hi: -0.472303213942605 }], [Interval { lo: -2.4767789490110252, hi: -2.476778949011022 }, Interval { lo: -2e-323, hi: 2e-323 }, Interval { lo: 1.3759883050061235, hi: 1.3759883050061252 }], [Interval { lo: -5e-324, hi: 5e-324 }, Interval { lo: -5e-324, hi: 5e-324 }, Interval { lo: -5e-324, hi: 5e-324 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0000000000000004 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -0.5, hi: 0.5 }], unknowns: 4 }, curve: Circle(Circle { center: [1.4700633956814717, 2.1880253582725886, 3.8461141122266493], radius: 2.8333333333333335, frame: Frame { x: [0.0, 1.0, 0.0], normal: [0.4767312946227962, 0.19069251784911845, 0.8581163303210331] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[1.4700633956814717, 2.1880253582725886, 3.8461141122266493], [-0.26239067441255864, 2.7813411487731217, -0.47230321394260544], [-2.476778949011024, 0.0, 1.3759883050061243], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 1.4700633956814717, hi: 1.4700633956814717 }, Interval { lo: 2.1880253582725886, hi: 2.1880253582725886 }, Interval { lo: 3.8461141122266493, hi: 3.8461141122266493 }], [Interval { lo: -0.2623906744125589, hi: -0.26239067441255837 }, Interval { lo: 2.7813411487731186, hi: 2.7813411487731243 }, Interval { lo: -0.47230321394260594, hi: -0.472303213942605 }], [Interval { lo: -2.4767789490110252, hi: -2.476778949011022 }, Interval { lo: -2e-323, hi: 2e-323 }, Interval { lo: 1.3759883050061235, hi: 1.3759883050061252 }], [Interval { lo: -5e-324, hi: 5e-324 }, Interval { lo: -5e-324, hi: 5e-324 }, Interval { lo: -5e-324, hi: 5e-324 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0000000000000004 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -0.5, hi: 0.5 }], unknowns: 4 }, curve: Circle(Circle { center: [0.15388588777335072, 1.8119746417274114, 2.529936604318528], radius: 2.8333333333333335, frame: Frame { x: [0.0, 1.0, 0.0], normal: [0.8581163303210331, 0.19069251784911845, 0.4767312946227962] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.15388588777335072, 1.8119746417274114, 2.529936604318528], [-0.47230321394260544, 2.7813411487731217, -0.26239067441255864], [-1.3759883050061243, 0.0, 2.476778949011024], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.15388588777335072, hi: 0.15388588777335072 }, Interval { lo: 1.8119746417274114, hi: 1.8119746417274114 }, Interval { lo: 2.529936604318528, hi: 2.529936604318528 }], [Interval { lo: -0.47230321394260594, hi: -0.472303213942605 }, Interval { lo: 2.7813411487731186, hi: 2.7813411487731243 }, Interval { lo: -0.2623906744125589, hi: -0.26239067441255837 }], [Interval { lo: -1.375988305006125, hi: -1.3759883050061235 }, Interval { lo: -2e-323, hi: 2e-323 }, Interval { lo: 2.476778949011022, hi: 2.476778949011025 }], [Interval { lo: -5e-324, hi: 5e-324 }, Interval { lo: -5e-324, hi: 5e-324 }, Interval { lo: -5e-324, hi: 5e-324 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.0, hi: 1.0000000000000004 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -0.5, hi: 0.5 }], unknowns: 4 }, curve: Circle(Circle { center: [1.8461141122266493, 2.1880253582725886, 3.470063395681472], radius: 2.8333333333333335, frame: Frame { x: [0.0, 1.0, 0.0], normal: [0.8581163303210331, 0.19069251784911845, 0.4767312946227962] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[1.8461141122266493, 2.1880253582725886, 3.470063395681472], [-0.47230321394260544, 2.7813411487731217, -0.26239067441255864], [-1.3759883050061243, 0.0, 2.476778949011024], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 1.8461141122266493, hi: 1.8461141122266493 }, Interval { lo: 2.1880253582725886, hi: 2.1880253582725886 }, Interval { lo: 3.470063395681472, hi: 3.470063395681472 }], [Interval { lo: -0.47230321394260594, hi: -0.472303213942605 }, Interval { lo: 2.7813411487731186, hi: 2.7813411487731243 }, Interval { lo: -0.2623906744125589, hi: -0.26239067441255837 }], [Interval { lo: -1.375988305006125, hi: -1.3759883050061235 }, Interval { lo: -2e-323, hi: 2e-323 }, Interval { lo: 2.476778949011022, hi: 2.476778949011025 }], [Interval { lo: -5e-324, hi: 5e-324 }, Interval { lo: -5e-324, hi: 5e-324 }, Interval { lo: -5e-324, hi: 5e-324 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 1.5, hi: 2.0 }, Interval { lo: 0.0, hi: 10.0 }, Interval { lo: -1.285e-321, hi: 50.00000000000013 }, Interval { lo: -50.00000000000014, hi: 1.285e-321 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 4.999999999999999], radius: 50.0, frame: Frame { x: [50.0, 0.0, 0.0], normal: [0.0, 0.0, 2500.0] }, domain: (1.5, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 4.999999999999999], [50.0, 0.0, 0.0], [0.0, 50.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.999999999999999, hi: 4.999999999999999 }], [Interval { lo: 50.0, hi: 50.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 50.0, hi: 50.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
];
