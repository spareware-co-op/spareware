//! A cylinder's or cone's axial factor (issue 911). Such a kind is c + (r₀ + k·v)·e(u) + s·v·n̂
//! (k = 0 for a cylinder): its point at v stands s·v along the unit axis, so a height z is v = z/s
//! and a cone's radius grows by k/s for each unit of height. The factor s is ±1 as built, where z/s
//! is z·s and k/s is k·s, but a similarity multiplies it by its scale (and r₀ and k with it), and
//! keeps each (u, v) the moved kind's point there. So a pair moved by a similarity meets at the
//! same parameters as before, on the moved curves: each test meets a pair built at factor ±1,
//! moves both by x ↦ 2x, by the mirror x ↦ (−2x, 2y, 2z) and by x ↦ (3x, 3y, −3z) (factors 2, −2
//! and −3), and holds each moved meeting to the first's parameters and to the moved surfaces'
//! closed forms.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{Basis, CurveKind, Frame, Helix, HelixAxes, Line, ParamCurve, Trimmed};
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::quadrics::natural;
use geom_intersect::result::{Intersection, Item};
use geom_math::{RationalTransform, TypedTransform};
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Extrusion, ParamSurface, Plane, SurfaceKind, Torus};

const ROUND: (f64, f64) = (0.0, 2.0);
const WIDE: (f64, f64) = (-1000.0, 1000.0);
const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

/// The similarities, each with the factor it multiplies a cylinder's or cone's axial factor by:
/// its scale, negated by a mirror.
const MOVES: [([[i64; 3]; 3], f64); 3] = [
    ([[2, 0, 0], [0, 2, 0], [0, 0, 2]], 2.0),
    ([[-2, 0, 0], [0, 2, 0], [0, 0, 2]], -2.0),
    ([[3, 0, 0], [0, 3, 0], [0, 0, -3]], -3.0),
];

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(50_000_000))
}

fn similarity(m: [[i64; 3]; 3]) -> TypedTransform {
    TypedTransform::from(RationalTransform::new(m, [0; 3], 1).unwrap())
}

/// `s` moved by `m`, within a rounding of it (a plane on vectors that are not axis-aligned rounds),
/// a cylinder's or cone's axial factor `by` times its own.
fn moved_surface(s: &SurfaceKind, m: [[i64; 3]; 3], by: f64) -> SurfaceKind {
    let moved = s.transformed(&similarity(m), &mut meter()).unwrap();
    assert!(moved.deviation < 1e-12, "{}", moved.deviation);
    let factor = |k: &SurfaceKind| match k {
        SurfaceKind::Cylinder(c) => Some(c.axial_sign()),
        SurfaceKind::Cone(c) => Some(c.axial_sign()),
        _ => None,
    };
    if let Some(f) = factor(s) {
        assert_eq!(factor(&moved.value), Some(f * by), "{:?}", moved.value);
    }
    moved.value
}

fn moved_curve(c: &CurveKind<3>, m: [[i64; 3]; 3]) -> CurveKind<3> {
    let moved = c.transformed(&similarity(m), &mut meter()).unwrap();
    assert_eq!(moved.deviation, 0.0);
    moved.value
}

fn up() -> Frame<3> {
    Frame::standard().unwrap()
}

fn line(o: [f64; 3], d: [f64; 3]) -> CurveKind<3> {
    CurveKind::Line(Line::infinite(o, d).unwrap())
}

fn segment(a: [f64; 3], b: [f64; 3]) -> CurveKind<3> {
    CurveKind::Line(Line::segment(a, b).unwrap().value)
}

fn plane(c: [f64; 3], a: [f64; 3], b: [f64; 3]) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(c, a, b, (WIDE, WIDE)).unwrap())
}

fn level(z: f64) -> SurfaceKind {
    plane([0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
}

/// Radius `r` about the z axis, heights −50 to 50.
fn cylinder(r: f64) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], r, up(), (ROUND, (-50.0, 50.0))).unwrap())
}

/// Radius r₀ + k·v about the z axis at height v, over `heights`.
fn cone(r0: f64, k: f64, heights: (f64, f64)) -> Cone {
    Cone::new([0.0; 3], r0, k, up(), (ROUND, heights)).unwrap()
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// A point's height along a frame's unit axis from `c`, and its distance from that axis.
fn polar(c: [f64; 3], frame: &Frame<3>, p: [f64; 3]) -> (f64, f64) {
    let n = frame.normal();
    let n = n.map(|x| x / length(n));
    let w = sub(p, c);
    (dot(w, n), length(cross(w, n)))
}

/// How far `p` stands from `s`, by its closed form: a cone's radius at height h along its unit
/// axis is |r₀ + (k/s)·h|.
fn off(s: &SurfaceKind, p: [f64; 3]) -> f64 {
    match s {
        SurfaceKind::Plane(x) => {
            let (c, a, b) = x.vectors();
            let n = cross(a, b);
            (dot(sub(p, c), n) / length(n)).abs()
        }
        SurfaceKind::Cylinder(x) => (polar(x.center(), &x.frame(), p).1 - x.radius()).abs(),
        SurfaceKind::Cone(x) => {
            let (r0, k) = x.radius_and_slope();
            let (h, rho) = polar(x.center(), &x.frame(), p);
            (rho - (r0 + k / x.axial_sign() * h).abs()).abs()
        }
        SurfaceKind::Torus(x) => {
            let (major, minor) = x.radii();
            let (h, rho) = polar(x.center(), &x.frame(), p);
            let out = rho - major;
            ((out * out + h * h).sqrt() - minor).abs()
        }
        // An extrusion is held by the same-parameter check alone.
        _ => 0.0,
    }
}

fn middles(item: &Item) -> Vec<f64> {
    item.at()
        .spans()
        .iter()
        .map(|s| f64::midpoint(s.lo(), s.hi()))
        .collect()
}

/// The start of `x`'s debug form, enough to name a case.
fn short(x: &impl core::fmt::Debug) -> String {
    format!("{x:?}").chars().take(100).collect()
}

/// The widest of an item's parameter spans.
fn width(item: &Item) -> f64 {
    item.at()
        .spans()
        .iter()
        .map(|s| s.hi() - s.lo())
        .fold(0.0, |a: f64, b| if b > a { b } else { a })
}

const fn rank(item: &Item) -> u8 {
    match item {
        Item::Point { .. } => 0,
        Item::Curve { .. } => 1,
        Item::Overlap { .. } => 2,
        Item::Flagged { .. } => 3,
    }
}

/// `moved` holds `base`'s items, of the same kinds at the same parameters.
fn alike(base: &Intersection, moved: &Intersection, what: &str) {
    let (a, b) = (base.items(), moved.items());
    let all = |r: &[Item]| r.iter().map(middles).collect::<Vec<_>>();
    assert_eq!(
        a.len(),
        b.len(),
        "{what}: {:?} against {:?}",
        all(a),
        all(b)
    );
    for (x, y) in a.iter().zip(b) {
        assert_eq!(rank(x), rank(y), "{what}: {x:?} against {y:?}");
        // Within the enclosures' widths, which a move may change.
        let slack = width(x) + width(y);
        for (p, q) in middles(x).iter().zip(middles(y)) {
            assert!(
                (p - q).abs() <= 1e-9 * (1.0 + p.abs()) + slack,
                "{what}: {:?} against {:?}",
                middles(x),
                middles(y)
            );
        }
    }
}

/// Each point `r` finds of `curve` on `surface` is the surface's own point at its (u, v).
fn points_hold(r: &Intersection, curve: &CurveKind<3>, surface: &SurfaceKind, what: &str) {
    for item in r.items() {
        if let Item::Point { .. } = item {
            let [t, u, v] = middles(item)[..] else {
                panic!("{item:?}")
            };
            let p = curve.point(t, Limit::FromRight).unwrap();
            let q = surface.point(u, v, SIDES).unwrap();
            // Within the enclosure's width, at the speeds of these kinds (at most a few times
            // their size per unit of a parameter).
            let reach = (1e-9 + 10.0 * width(item)) * (1.0 + length(p));
            assert!(length(sub(p, q)) <= reach, "{what}: {p:?} and {q:?}");
        }
    }
}

/// Every curve `r` finds lies on both surfaces, by their closed forms.
fn curves_hold(r: &Intersection, a: &SurfaceKind, b: &SurfaceKind, what: &str) {
    for item in r.items() {
        if let Item::Curve { curve, .. } = item {
            let (lo, hi) = curve.domain();
            for i in 0..=8 {
                let t = lo + (hi - lo) * f64::from(i) / 8.0;
                let p = curve.point(t, Limit::FromRight).unwrap();
                for s in [a, b] {
                    let gap = off(s, p);
                    assert!(
                        gap < 1e-7 * (1.0 + length(p)),
                        "{what}: {p:?} {gap} off {s:?}"
                    );
                }
            }
        }
    }
}

/// `curve` with `surface`, built and moved by each of [`MOVES`]: `count` items each time, at the
/// same parameters, each point the moved surface's own.
fn same_on_curve(curve: &CurveKind<3>, surface: &SurfaceKind, count: usize) {
    let base = curve_surface(curve, surface, &mut meter()).unwrap();
    assert_eq!(base.items().len(), count, "{curve:?}: {:?}", base.items());
    points_hold(&base, curve, surface, "as built");
    for (m, by) in MOVES {
        let (c, s) = (moved_curve(curve, m), moved_surface(surface, m, by));
        let r = curve_surface(&c, &s, &mut meter()).unwrap();
        let what = format!("{} moved by {m:?}", short(curve));
        alike(&base, &r, &what);
        points_hold(&r, &c, &s, &what);
    }
}

/// `a` with `b`, built and moved by each of [`MOVES`]: `count` items each time, at the same
/// parameters, each curve on both moved surfaces.
fn same_pair(a: &SurfaceKind, b: &SurfaceKind, count: usize) {
    let base = natural(a, b, &mut meter()).unwrap().unwrap();
    assert_eq!(base.items().len(), count, "{:?}", base.items());
    curves_hold(&base, a, b, "as built");
    for (m, by) in MOVES {
        let (x, y) = (moved_surface(a, m, by), moved_surface(b, m, by));
        let r = natural(&x, &y, &mut meter()).unwrap().unwrap();
        let what = format!("{} with {} moved by {m:?}", short(a), short(b));
        alike(&base, &r, &what);
        curves_hold(&r, &x, &y, &what);
    }
}

#[test]
fn a_line_meets_a_moved_cylinder_at_the_same_parameters() {
    // line_cylinder's roots and along_ruling's span read v = z/s: z·s put the line at height 80 on
    // the doubled cylinder (radius 10, heights −100 to 100) at v = 160, off its domain, not at 40.
    let c = cylinder(5.0);
    same_on_curve(&line([-9.0, 3.0, 2.0], [1.0, 0.0, 0.0]), &c, 2);
    same_on_curve(&line([-9.0, 3.0, 40.0], [1.0, 0.0, 0.0]), &c, 2);
    same_on_curve(&line([-9.0, 5.0, 2.0], [1.0, 0.0, 0.0]), &c, 1);
    same_on_curve(&line([-9.0, 0.0, 2.0], [1.0, 0.0, 1.0]), &c, 2);
    same_on_curve(&segment([5.0, 0.0, -9.0], [5.0, 0.0, 9.0]), &c, 1);
    same_on_curve(&segment([0.0, -5.0, 30.0], [0.0, -5.0, 45.0]), &c, 1);
}

#[test]
fn a_line_meets_a_moved_cone_at_the_same_parameters() {
    // Radius 2 + v at height v, its apex at −2. line_cone's quadratic takes the radius per unit of
    // height k/s and its roots v = z/s: with k·s, the doubled cone (radius 4 + 2v at height 2v, so
    // 4 + z) was read as 4 + 4z.
    let c = SurfaceKind::Cone(cone(2.0, 1.0, (-10.0, 10.0)));
    same_on_curve(&line([-9.0, 0.0, 1.0], [1.0, 0.0, 0.0]), &c, 2);
    same_on_curve(&line([-9.0, 0.0, -5.0], [1.0, 0.0, 0.0]), &c, 2);
    same_on_curve(&line([0.0, 0.0, 0.0], [1.0, 0.0, 1.0]), &c, 1);
    same_on_curve(&line([-20.0, 1.0, 7.0], [3.0, 0.0, -1.0]), &c, 2);
}

#[test]
fn a_helix_on_a_moved_cylinder_lies_in_it_over_the_same_parameters() {
    // helix_on_cylinder's span in v is the rise over s: radius 5, pitch 2, from 0 to 8 high.
    let axes = HelixAxes::Frame {
        frame: up(),
        radius: 5.0,
        pitch: 2.0,
    };
    let h = CurveKind::Helix(Helix::new([0.0, 0.0, 3.0], axes, (0.0, 8.0)).unwrap());
    same_on_curve(&h, &cylinder(5.0), 1);
}

#[test]
fn a_plane_cuts_a_moved_cylinder_or_cone_at_the_same_parameters() {
    // `parameters` reads v = z/s on both kinds: the circle at height 80 on the doubled cylinder
    // stood at v = 160 with z·s, off its domain, so it was dropped.
    same_pair(&level(40.0), &cylinder(5.0), 1);
    let c = SurfaceKind::Cone(cone(2.0, 1.0, (-10.0, 10.0)));
    // funnel's slope is the tangent of the half-angle, |k/s|, along the unit axis: |k| along s·n̂
    // made the doubled cone's circle at height 18 of radius 44, not 22.
    same_pair(&level(9.0), &c, 1);
    // A plane whose normal leans from the axis by more than the half-angle's complement: a
    // hyperbola, both branches.
    same_pair(
        &plane([0.0, 0.0, 3.0], [2.0, 0.0, -5.0], [0.0, 1.0, 0.0]),
        &c,
        2,
    );
    // One nappe 200 high and 10 wide, cut along its axis: one branch, whose ends the hyperbola's
    // span must reach. cone_extent reads no axial factor ((far + |r₀|)·(1 + |k|) + 1 is 221 on the
    // doubled cone, 400 high), and this pins that it need not: the branch's span grows by the ratio
    // of its axes, and reaches the doubled nappe's ends as it does the one built.
    let tall = SurfaceKind::Cone(cone(0.0, 0.05, (0.0, 200.0)));
    same_pair(
        &plane([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        &tall,
        1,
    );
}

#[test]
fn a_moved_cylinder_meets_a_coaxial_torus_or_sweep_at_the_same_parameters() {
    // Radius 5 about z, heights 0 to 50: its meridian runs over the heights s·v, not over v or
    // −v by the axis's sense alone. On the doubled pair the tube about height 80 lay past the
    // meridian's old end at 50.
    let tube =
        SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 5.0, up(), (ROUND, (0.0, 50.0))).unwrap());
    let torus =
        SurfaceKind::Torus(Torus::new([0.0, 0.0, 40.0], 5.0, 2.0, up(), (ROUND, ROUND)).unwrap());
    same_pair(&tube, &torus, 2);
    // A sweep of the cylinder's direction runs s·v·(n̂ · d̂) along it: a wall from 30 to 40 high
    // meets it in two lines. (Only the cylinder is given other factors here: a moved extrusion
    // keeps its shape unplaced, which the sweep pairs read as it stands.)
    let profile = segment([-10.0, 3.0, 30.0], [10.0, 3.0, 30.0]);
    let (lo, hi) = profile.domain();
    let wall = SurfaceKind::Extrusion(
        Extrusion::new(
            Trimmed::new(Basis::try_from(profile).unwrap(), (lo, hi)).unwrap(),
            [0.0, 0.0, 1.0],
            (0.0, 10.0),
        )
        .unwrap(),
    );
    let base = natural(&tube, &wall, &mut meter()).unwrap().unwrap();
    assert_eq!(base.items().len(), 2, "{:?}", base.items());
    curves_hold(&base, &tube, &wall, "as built");
    // The same cylinder with v a quarter of the height, either way up (factors 4 and −4): the
    // sweep took its run as v itself, 0 to 12.5 or −12.5 to 0, below the wall.
    for (factor, heights, v) in [
        (4.0, (0.0, 12.5), (7.5, 10.0)),
        (-4.0, (-12.5, 0.0), (-10.0, -7.5)),
    ] {
        let stretched = SurfaceKind::Cylinder(
            Cylinder::new([0.0; 3], 5.0, up(), (ROUND, heights))
                .unwrap()
                .with_axial_sign(factor)
                .unwrap(),
        );
        let r = natural(&stretched, &wall, &mut meter()).unwrap().unwrap();
        assert_eq!(r.items().len(), 2, "{factor}: {:?}", r.items());
        curves_hold(&r, &stretched, &wall, "stretched");
        for item in r.items() {
            // On the cylinder from height 30 to 40: v = height/s.
            let on = item.at().spans()[1];
            assert!(
                on.lo() <= v.0 + 1e-9 && on.hi() >= v.1 - 1e-9 && on.hi() - on.lo() < 2.5 + 1e-6,
                "{factor}: {item:?}"
            );
        }
    }
}

#[test]
fn a_cylinder_of_any_factor_meets_a_coaxial_torus_over_both_ends_of_its_heights() {
    // The meridian runs over s·v at both ends of v's domain. A cylinder whose v starts at 0 cannot
    // tell s·v from v/s there, so these start past it: radius 5 from height 41 to 50 meets the
    // tube of radius 2 about height 40 in the ring at 42, not at 38. Moved by the factors 2, −2
    // and −3 the meridian starts at 82 (or 123) beside the tube about 80 (or 120), and v₀/s would
    // start it at 20.5 (or 13.7), below the second ring too.
    let torus =
        SurfaceKind::Torus(Torus::new([0.0, 0.0, 40.0], 5.0, 2.0, up(), (ROUND, ROUND)).unwrap());
    let tube =
        SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 5.0, up(), (ROUND, (41.0, 50.0))).unwrap());
    same_pair(&tube, &torus, 1);
    // Built at factor 2 or −2, v over a quarter of 41 to 50 times 2 either way up: the one ring of
    // radius 5 about height 42, exactly where the data are. With v₀/s the meridian ran from 10.25
    // (two rings), or from 12.5 to 41 (the ring at 38).
    for (factor, heights) in [(2.0, (20.5, 25.0)), (-2.0, (-25.0, -20.5))] {
        let stretched = SurfaceKind::Cylinder(
            Cylinder::new([0.0; 3], 5.0, up(), (ROUND, heights))
                .unwrap()
                .with_axial_sign(factor)
                .unwrap(),
        );
        let r = natural(&stretched, &torus, &mut meter()).unwrap().unwrap();
        let [Item::Curve { curve, .. }] = r.items() else {
            panic!("{factor}: {:?}", r.items())
        };
        let CurveKind::Circle(ring) = curve.as_ref() else {
            panic!("{factor}: {curve:?}")
        };
        assert_eq!(ring.radius(), 5.0, "{factor}: {ring:?}");
        assert_eq!(ring.center(), [0.0, 0.0, 42.0], "{factor}: {ring:?}");
        curves_hold(&r, &stretched, &torus, "stretched");
    }
}

#[test]
fn a_cone_meets_its_mirror_image_and_a_stretched_copy_in_circles() {
    // Stored data equal but for the axial factor are not one cone: c + (2 + v)·e(u) − v·n̂ is the
    // mirror image of c + (2 + v)·e(u) + v·n̂ in the plane z = 0, and they meet there, in the
    // circle of radius 2. cone_cone called the pair one surface.
    let a = cone(2.0, 1.0, (-10.0, 10.0));
    let b = a.flipped_axially().unwrap();
    let (x, y) = (SurfaceKind::Cone(a.clone()), SurfaceKind::Cone(b));
    let r = natural(&x, &y, &mut meter()).unwrap().unwrap();
    let radii = circles(&r);
    assert_eq!(radii.len(), 1, "{:?}", r.items());
    assert!((radii[0] - 2.0).abs() < 1e-9, "{radii:?}");
    curves_hold(&r, &x, &y, "mirrored");
    // With factor 2 the radius is 2 + h/2 at height h: they meet where 2 + h = ±(2 + h/2), at
    // h = 0 (radius 2) and h = −8/3 (radius 2/3).
    let z = SurfaceKind::Cone(a.with_axial_sign(2.0).unwrap());
    let r = natural(&x, &z, &mut meter()).unwrap().unwrap();
    let mut radii = circles(&r);
    radii.sort_by(f64::total_cmp);
    assert_eq!(radii.len(), 2, "{:?}", r.items());
    assert!((radii[0] - 2.0 / 3.0).abs() < 1e-9, "{radii:?}");
    assert!((radii[1] - 2.0).abs() < 1e-9, "{radii:?}");
    curves_hold(&r, &x, &z, "stretched");
    same_pair(&x, &y, 1);
}

/// The radii of `r`'s items, each a circle.
fn circles(r: &Intersection) -> Vec<f64> {
    r.items()
        .iter()
        .map(|item| match item {
            Item::Curve { curve, .. } => match curve.as_ref() {
                CurveKind::Circle(c) => c.radius(),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        })
        .collect()
}
