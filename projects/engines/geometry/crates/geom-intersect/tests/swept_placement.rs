//! A moved extrusion or surface of revolution is met where it stands (issue 917). Each swept kind
//! is a placement c + L·X of a shape X kept as built, and a transform moves only the placement, so
//! a reduced case that read the shape alone met a moved surface as if unmoved: a wall moved by
//! x ↦ (3x, 3y, −3z) met a cylinder moved with it in no lines.
//!
//! Each test meets a pair as built, then the same pair moved by each of [`MOVES`] (moved, turned,
//! mirrored, scaled and all at once, each a similarity whose entries and translation are whole), and
//! holds the moved answer to the first carried through the same move: each curve of the first,
//! moved exactly (its transform states no rounding), is a curve of the second, of the same kind,
//! and there are as many. A placement no reduced case can take exactly, a rounded turn or a
//! revolution stretched along one axis, is left to the general paths (`None`), never answered as
//! if unmoved.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    reason = "test code: a failure should stop loudly"
)]

mod miri;

use geom_curve::{Basis, Circle, CurveKind, Frame, Line, ParamCurve, Trimmed};
use geom_intersect::quadrics::natural;
use geom_intersect::result::{Intersection, Item};
use geom_math::{RationalTransform, Transform, TypedTransform};
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, Extrusion, ParamSurface, Plane, Revolution, Sphere, SurfaceKind, Torus,
};
use miri::rounds;

const ROUND: (f64, f64) = (0.0, 2.0);
const WIDE: (f64, f64) = (-1000.0, 1000.0);

/// The moves: each a linear part by rows and a translation, all whole.
type Move = ([[i64; 3]; 3], [i128; 3]);

/// Mirrors and turns both, so that a mirror's every way of showing in the rows is met: its first
/// row's first entry not zero ("mirrored", "upside down", "issue 917"), only its second ("x and y
/// swapped") or only its third ("x and z swapped", "all at once"). "Upside down" and "half
/// turned" are the look-alike pair: each takes a surface of revolution about z onto the same
/// surface, one mirroring and one not.
const MOVES: [(&str, Move); 10] = [
    ("moved", ([[1, 0, 0], [0, 1, 0], [0, 0, 1]], [5, -7, 11])),
    ("turned", ([[1, 0, 0], [0, 0, -1], [0, 1, 0]], [1, 2, 3])),
    ("mirrored", ([[-1, 0, 0], [0, 1, 0], [0, 0, 1]], [0, 0, 0])),
    ("scaled", ([[2, 0, 0], [0, 2, 0], [0, 0, 2]], [0, 0, 0])),
    ("issue 917", ([[3, 0, 0], [0, 3, 0], [0, 0, -3]], [0, 0, 0])),
    (
        "all at once",
        ([[0, 0, 2], [-2, 0, 0], [0, 2, 0]], [4, 0, -6]),
    ),
    (
        "upside down",
        ([[1, 0, 0], [0, 1, 0], [0, 0, -1]], [0, 0, 0]),
    ),
    (
        "half turned",
        ([[1, 0, 0], [0, -1, 0], [0, 0, -1]], [0, 0, 0]),
    ),
    (
        "x and y swapped",
        ([[0, 1, 0], [1, 0, 0], [0, 0, 1]], [2, 0, 0]),
    ),
    (
        "x and z swapped",
        ([[0, 0, 1], [0, 1, 0], [1, 0, 0]], [0, 0, 3]),
    ),
];

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(50_000_000))
}

fn transform((m, t): Move) -> TypedTransform {
    TypedTransform::from(RationalTransform::new(m, t, 1).unwrap())
}

fn moved_surface(s: &SurfaceKind, m: Move) -> SurfaceKind {
    let moved = s.transformed(&transform(m), &mut meter()).unwrap();
    assert_eq!(moved.deviation, 0.0, "{s:?}");
    moved.value
}

/// `c` carried through `m`: within its transform's stated deviation of the exact image, which
/// is 0 for exact data and a rounding of the answer's own doubles otherwise, far inside the
/// tolerance the curves are compared at.
fn moved_curve(c: &CurveKind<3>, m: Move) -> CurveKind<3> {
    let moved = c.transformed(&transform(m), &mut meter()).unwrap();
    assert!(moved.deviation <= 1e-12, "{c:?}: {}", moved.deviation);
    moved.value
}

fn up() -> Frame<3> {
    Frame::standard().unwrap()
}

fn trimmed(c: CurveKind<3>) -> Trimmed<3> {
    let span = c.domain();
    Trimmed::new(Basis::try_from(c).unwrap(), span).unwrap()
}

fn segment(a: [f64; 3], b: [f64; 3]) -> CurveKind<3> {
    CurveKind::Line(Line::segment(a, b).unwrap().value)
}

fn plane(c: [f64; 3], a: [f64; 3], b: [f64; 3]) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(c, a, b, (WIDE, WIDE)).unwrap())
}

/// The surface `profile` sweeps turning once about the z axis.
fn revolution(profile: CurveKind<3>) -> SurfaceKind {
    SurfaceKind::Revolution(Revolution::new(trimmed(profile), [0.0; 3], up(), ROUND).unwrap())
}

/// The surface `profile` sweeps along `d` over `along`.
fn extrusion(profile: CurveKind<3>, d: [f64; 3], along: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Extrusion(Extrusion::new(trimmed(profile), d, along).unwrap())
}

/// The cone-like revolution ρ = 7 + z/2 about z, from z = −10 to 10.
fn funnel() -> SurfaceKind {
    revolution(segment([2.0, 0.0, -10.0], [12.0, 0.0, 10.0]))
}

/// A wall from x = −10 to 10 at y = 3, from z = 30 to 40.
fn wall() -> SurfaceKind {
    extrusion(
        segment([-10.0, 3.0, 30.0], [10.0, 3.0, 30.0]),
        [0.0, 0.0, 1.0],
        (0.0, 10.0),
    )
}

/// A round column of radius 4 about the z axis, from z = 30 to 40, its profile a circle.
fn column() -> SurfaceKind {
    let ring = Circle::new([0.0, 0.0, 30.0], 4.0, up(), ROUND).unwrap();
    extrusion(CurveKind::Circle(ring), [0.0, 0.0, 2.0], (0.0, 5.0))
}

fn cylinder(r: f64, heights: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], r, up(), (ROUND, heights)).unwrap())
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The start of `x`'s debug form, enough to name a case.
fn short(x: &impl core::fmt::Debug) -> String {
    format!("{x:?}").chars().take(90).collect()
}

/// How far `p` stands from `c`, a line or segment or a circle.
fn gap(c: &CurveKind<3>, p: [f64; 3]) -> f64 {
    match c {
        CurveKind::Line(l) => {
            let (o, d) = (l.origin(), l.direction());
            let t = dot(sub(p, o), d) / dot(d, d);
            let (lo, hi) = l.domain();
            let t = t.clamp(lo, hi);
            length(sub(p, [o[0] + t * d[0], o[1] + t * d[1], o[2] + t * d[2]]))
        }
        CurveKind::Circle(x) => {
            let n = x.frame().normal();
            let n = n.map(|v| v / length(n));
            let w = sub(p, x.center());
            let h = dot(w, n);
            let rho = length(cross(w, n));
            let out = rho - x.radius();
            (out * out + h * h).sqrt()
        }
        other => panic!("no gap to {other:?}"),
    }
}

/// The curves of `r`.
fn curves(r: &Intersection) -> Vec<CurveKind<3>> {
    r.items()
        .iter()
        .map(|item| match item {
            Item::Curve { curve, .. } => curve.as_ref().clone(),
            other => panic!("{other:?}"),
        })
        .collect()
}

/// Points along `c`, its ends among them.
fn samples(c: &CurveKind<3>) -> Vec<[f64; 3]> {
    let (lo, hi) = c.domain();
    (0..=8)
        .map(|i| {
            c.point(lo + (hi - lo) * f64::from(i) / 8.0, Limit::FromRight)
                .unwrap()
        })
        .collect()
}

/// Which way `c`, a line or circle, runs: a line from its start to its end, a circle about the
/// normal its samples turn about (the sum of the cross products of each two in turn, from its
/// centre).
fn travel(c: &CurveKind<3>) -> [f64; 3] {
    let at = samples(c);
    match c {
        CurveKind::Line(_) => sub(at[at.len() - 1], at[0]),
        CurveKind::Circle(x) => at.windows(2).fold([0.0; 3], |sum, w| {
            let turn = cross(sub(w[0], x.center()), sub(w[1], x.center()));
            [sum[0] + turn[0], sum[1] + turn[1], sum[2] + turn[2]]
        }),
        other => panic!("no travel along {other:?}"),
    }
}

/// Whether `a` and `b` are one curve, of one kind, running one way: a line or circle each point of
/// either on the other and travelling as the other does (a mirror reverses a circle's sense about
/// the moved normal, a turn keeps it), and any other kind (an affine copy of a profile, on the
/// profile's parameters) its points the other's at the same parameters.
fn same_curve(a: &CurveKind<3>, b: &CurveKind<3>, size: f64) -> bool {
    let tol = 1e-9 * size;
    if core::mem::discriminant(a) != core::mem::discriminant(b) {
        return false;
    }
    match a {
        CurveKind::Line(_) | CurveKind::Circle(_) => {
            let near =
                |x: &CurveKind<3>, y: &CurveKind<3>| samples(x).iter().all(|p| gap(y, *p) <= tol);
            near(a, b) && near(b, a) && dot(travel(a), travel(b)) > 0.0
        }
        _ => {
            a.domain() == b.domain()
                && samples(a)
                    .iter()
                    .zip(samples(b))
                    .all(|(p, q)| length(sub(*p, q)) <= tol)
        }
    }
}

/// The size of the pair's answers, for a relative tolerance.
fn size_of(curves: &[CurveKind<3>]) -> f64 {
    curves
        .iter()
        .flat_map(samples)
        .map(length)
        .fold(1.0, |a: f64, b| if b > a { b } else { a })
}

/// `a` with `b` as built: `count` items. Then both moved by each of [`MOVES`]: the first answer's
/// curves carried through the move, exactly, are the moved answer's.
fn pinned(a: &SurfaceKind, b: &SurfaceKind, count: usize) {
    pinned_by(a, b, count, &MOVES);
}

/// [`pinned`], moved by each of `moves`, some of [`MOVES`].
fn pinned_by(a: &SurfaceKind, b: &SurfaceKind, count: usize, moves: &[(&str, Move)]) {
    let base = natural(a, b, &mut meter()).unwrap().unwrap();
    assert_eq!(base.items().len(), count, "{:?}", base.items());
    let first = curves(&base);
    for &(name, m) in moves {
        let (x, y) = (moved_surface(a, m), moved_surface(b, m));
        let what = format!("{} with {} {name}", short(a), short(b));
        let found = natural(&x, &y, &mut meter());
        let Some(r) = found.unwrap_or_else(|e| panic!("{what}: {e:?}")) else {
            panic!("{what}: no closed form");
        };
        let found = curves(&r);
        let carried: Vec<_> = first.iter().map(|c| moved_curve(c, m)).collect();
        let size = size_of(&carried);
        assert_eq!(found.len(), carried.len(), "{what}: {found:?}");
        for c in &carried {
            assert!(
                found.iter().any(|f| same_curve(c, f, size)),
                "{what}: {c:?} not among {found:?}"
            );
        }
        for f in &found {
            assert!(
                carried.iter().any(|c| same_curve(c, f, size)),
                "{what}: {f:?} not among {carried:?}"
            );
        }
    }
}

#[test]
fn a_moved_wall_meets_a_moved_cylinder_in_the_moved_lines() {
    // The smallest input of issue 917: the wall at y = 3 meets the cylinder of radius 5 in the
    // lines x = ±4, from z = 30 to 40. Moved by x ↦ (3x, 3y, −3z), the cross-section read was the
    // wall's as built, and met the moved cylinder's in nothing.
    pinned(&cylinder(5.0, (0.0, 50.0)), &wall(), 2);
    pinned(&cylinder(5.0, (0.0, 50.0)), &column(), 0);
    pinned(&cylinder(6.0, (0.0, 50.0)), &column(), 0);
}

#[test]
fn moved_extrusions_of_one_direction_meet_in_the_moved_lines() {
    // A second wall at x = 2, from z = 25 to 45 (direction twice the first's): one line, at
    // (2, 3), from 30 to 40. And the round column of radius 4 crosses the first wall at x = ±√7.
    let other = extrusion(
        segment([2.0, -10.0, 25.0], [2.0, 10.0, 25.0]),
        [0.0, 0.0, 2.0],
        (0.0, 10.0),
    );
    pinned(&wall(), &other, 1);
    pinned(&wall(), &column(), 2);
}

#[test]
fn a_plane_cuts_a_moved_extrusion_in_the_moved_section() {
    // Under Miri, four of the moves: a turn, and a mirror showing in each row (with a scale and a
    // translation among them). Each move is one meeting, and the same claims hold for each.
    let few = [MOVES[1], MOVES[4], MOVES[5], MOVES[8]];
    let moves = rounds(&MOVES, &few);
    let pinned = |a: &SurfaceKind, b: &SurfaceKind, count| pinned_by(a, b, count, moves);
    // Along the direction: the plane x = 1 cuts the wall in a line, the column in two.
    let along = plane([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    pinned(&along, &wall(), 1);
    pinned(&along, &column(), 2);
    // Across it: a sloped plane cuts the wall in a segment and the column in an ellipse.
    let across = plane([0.0, 0.0, 35.0], [4.0, 0.0, 1.0], [0.0, 1.0, 0.0]);
    pinned(&across, &wall(), 1);
    pinned(&across, &column(), 1);
}

#[test]
fn a_plane_cuts_a_moved_revolution_in_the_moved_circles_and_profiles() {
    // Square to the axis at z = 4: the circle of radius 9.
    pinned(
        &plane([0.0, 0.0, 4.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        &funnel(),
        1,
    );
    // Through the axis: the profile and its mirror image.
    pinned(
        &plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        &funnel(),
        2,
    );
}

#[test]
fn a_moved_revolution_meets_a_coaxial_surface_in_the_moved_circles() {
    // ρ = 7 + z/2 meets the sphere of radius 10 about the origin where (7 + z/2)² + z² = 100: at
    // z = (−7 ± √304)/2.5, about 4.17 and −9.77.
    let sphere =
        SurfaceKind::Sphere(Sphere::new([0.0; 3], 10.0, up(), (ROUND, (-0.5, 0.5))).unwrap());
    pinned(&sphere, &funnel(), 2);
    // The cylinder of radius 5 at z = −4.
    pinned(&cylinder(5.0, (-50.0, 50.0)), &funnel(), 1);
    // The cone ρ = |4 − v| at z = −2, radius 6; its other nappe would meet the profile at z = 22,
    // past its end.
    let cone =
        SurfaceKind::Cone(Cone::new([0.0; 3], 4.0, -1.0, up(), (ROUND, (-10.0, 10.0))).unwrap());
    pinned(&cone, &funnel(), 1);
    // The torus of radii 8 and 1 about z = 2: the profile runs through its tube's centre.
    let torus =
        SurfaceKind::Torus(Torus::new([0.0, 0.0, 2.0], 8.0, 1.0, up(), (ROUND, ROUND)).unwrap());
    pinned(&torus, &funnel(), 2);
    // A second revolution, ρ = 7 − 3z/10: the circle of radius 7 at z = 0.
    let other = revolution(segment([10.0, 0.0, -10.0], [4.0, 0.0, 10.0]));
    pinned(&funnel(), &other, 1);
}

#[test]
fn a_moved_revolution_about_an_axis_off_the_origin_keeps_its_profile_on_the_axis() {
    // ρ = 7 + z/2 about the axis through (5, −7, 0), its profile in the half-plane y = −7. Placed
    // at c by L, the profile stands at c + L·(P − a): read at c + L·(P + a) it stood 2·L·a off,
    // in no half-plane of the axis.
    let off = SurfaceKind::Revolution(
        Revolution::new(
            trimmed(segment([7.0, -7.0, -10.0], [17.0, -7.0, 10.0])),
            [5.0, -7.0, 0.0],
            up(),
            ROUND,
        )
        .unwrap(),
    );
    // Square to the axis at z = 4: the circle of radius 9 about (5, −7, 4).
    pinned(
        &plane([0.0, 0.0, 4.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        &off,
        1,
    );
    // The cylinder of radius 9 about the same axis, there too.
    let tube = SurfaceKind::Cylinder(
        Cylinder::new([5.0, -7.0, 0.0], 9.0, up(), (ROUND, (-50.0, 50.0))).unwrap(),
    );
    pinned(&tube, &off, 1);
}

#[test]
fn a_mirrored_revolution_is_told_from_a_turned_one_by_the_way_its_circles_run() {
    // The mirror z ↦ −z and the half turn about x take ρ = 7 + z/2 (no mirror image of itself in
    // any plane square to z) onto one surface, ρ = 7 − z/2, the sphere of radius 10 onto itself,
    // and their circles onto the same two at z = (7 ± √304)/2.5. The mirror reverses the sense
    // in which each circle runs about z and the turn keeps it, as the moved surface of revolution
    // turns about −z or z: each answer is the first carried through its own move, and the two
    // run opposite ways.
    let sphere =
        SurfaceKind::Sphere(Sphere::new([0.0; 3], 10.0, up(), (ROUND, (-0.5, 0.5))).unwrap());
    let level = plane([0.0, 0.0, -4.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    // Under Miri, each pair is pinned under the two moves this test tells apart, not all ten: each
    // move is one meeting, and the same claims hold for each.
    let moves = rounds(&MOVES, &MOVES[6..8]);
    for (partner, count) in [(&sphere, 2), (&level, 1)] {
        pinned_by(partner, &funnel(), count, moves);
        let base = curves(&natural(partner, &funnel(), &mut meter()).unwrap().unwrap());
        let mut answers = Vec::new();
        for name in ["upside down", "half turned"] {
            let (_, m) = MOVES.iter().find(|(n, _)| *n == name).copied().unwrap();
            let (x, y) = (moved_surface(partner, m), moved_surface(&funnel(), m));
            let found = curves(&natural(&x, &y, &mut meter()).unwrap().unwrap());
            let carried: Vec<_> = base.iter().map(|c| moved_curve(c, m)).collect();
            for c in &carried {
                assert!(
                    found.iter().any(|f| same_curve(c, f, 100.0)),
                    "{name}: {c:?} not among {found:?}"
                );
            }
            answers.push(found);
        }
        let [mirrored, turned] = &answers[..] else {
            panic!()
        };
        assert_eq!(mirrored.len(), turned.len());
        for c in mirrored {
            // On a circle of the other, running the other way.
            let reversed = |f: &CurveKind<3>| {
                samples(c).iter().all(|p| gap(f, *p) <= 1e-7) && dot(travel(c), travel(f)) < 0.0
            };
            assert!(turned.iter().any(reversed), "{c:?} against {turned:?}");
        }
    }
}

#[test]
fn a_profile_is_a_meridian_only_where_it_lies_in_a_half_plane_of_its_axis() {
    let mut m = meter();
    let level = |z: f64| plane([0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    // About the axis through (5, −7, 0), ρ = 7 + z/2 again: its half-plane y = −7 holds the axis,
    // not the origin. The profile was read off the plane through the origin, by 7, and refused.
    let off = SurfaceKind::Revolution(
        Revolution::new(
            trimmed(segment([7.0, -7.0, -10.0], [17.0, -7.0, 10.0])),
            [5.0, -7.0, 0.0],
            up(),
            ROUND,
        )
        .unwrap(),
    );
    let r = natural(&level(4.0), &off, &mut m).unwrap().unwrap();
    let [CurveKind::Circle(c)] = &curves(&r)[..] else {
        panic!("{:?}", r.items())
    };
    assert!((c.radius() - 9.0).abs() < 1e-9, "{c:?}");
    assert!(length(sub(c.center(), [5.0, -7.0, 4.0])) < 1e-9, "{c:?}");
    // A skew segment about z sweeps the hyperboloid ρ² = 32 + z²/50. Its middle (4, 4, 0) is in
    // the half-plane x = y, its ends off it along (1, −1, 0), whose entries sum to zero: the
    // profile was read as lying in it, and z = 5 cut in the circle ρ = √32 of a cylinder, through
    // the axis in two upright lines.
    let skew = revolution(segment([3.0, 5.0, -10.0], [5.0, 3.0, 10.0]));
    if let Some(r) = natural(&level(5.0), &skew, &mut m).unwrap() {
        for curve in curves(&r) {
            for p in samples(&curve) {
                let rho = (p[0] * p[0] + p[1] * p[1]).sqrt();
                assert!(
                    (rho - 32.5_f64.sqrt()).abs() < 1e-9,
                    "{p:?} off the hyperboloid"
                );
            }
        }
    }
    let through = plane([0.0; 3], [1.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    assert!(natural(&through, &skew, &mut m).unwrap().is_none());
}

#[test]
fn a_placement_no_reduced_case_takes_exactly_is_left_to_the_general_paths() {
    let mut m = meter();
    // A revolution stretched along x is no surface of revolution: the plane square to its axis
    // cuts an ellipse, which the circle of the shape as built is not.
    let SurfaceKind::Revolution(r) = funnel() else {
        panic!()
    };
    let stretched = SurfaceKind::Revolution(
        r.placed(
            [0.0; 3],
            [[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        )
        .unwrap(),
    );
    let level = plane([0.0, 0.0, 4.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    assert!(natural(&level, &stretched, &mut m).unwrap().is_none());
    // Stretched along y, its rows square to one another and the first as long as the last but not
    // as the second: still no similarity, and the profile in y = 0 the stretch leaves where it was
    // would read it as the funnel unstretched.
    let tall = SurfaceKind::Revolution(
        r.placed(
            [0.0; 3],
            [[1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 1.0]],
        )
        .unwrap(),
    );
    assert!(natural(&level, &tall, &mut m).unwrap().is_none());
    // A wall turned by a third of a quarter turn, its turn rounded: its data placed are not
    // doubles, and the lines of the wall as built (x = ±4 at y = 3) are not on the turned wall.
    let (c, s) = (0.866_025_403_784_438_6_f64, 0.5_f64);
    let turn = TypedTransform::from(
        Transform::from_rows([[c, -s, 0.0, 0.0], [s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]]).unwrap(),
    );
    let tube = cylinder(5.0, (0.0, 50.0));
    // The same wall on a profile trimmed from an endless line, whose turned direction, rounded,
    // leaves its image unbounded.
    let endless = extrusion(
        CurveKind::Line(Line::new([0.0, 3.0, 30.0], [1.0, 0.0, 0.0], (-10.0, 10.0)).unwrap()),
        [0.0, 0.0, 1.0],
        (0.0, 10.0),
    );
    for built in [wall(), endless] {
        let turned = built.transformed(&turn, &mut m).unwrap().value;
        match natural(&tube, &turned, &mut m).unwrap() {
            None => {}
            Some(r) => {
                // Were it taken, its lines would stand on the turned wall: 3 from the axis.
                for curve in curves(&r) {
                    for p in samples(&curve) {
                        let off = (s * p[0] - c * p[1]).abs();
                        assert!((off - 3.0).abs() < 1e-9, "{p:?} off the turned wall");
                    }
                }
            }
        }
    }
    // A placement that flattens the wall along its direction leaves no surface to sweep.
    let SurfaceKind::Extrusion(x) = wall() else {
        panic!()
    };
    let flat = SurfaceKind::Extrusion(
        x.placed(
            [0.0; 3],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]],
        )
        .unwrap(),
    );
    assert!(natural(&tube, &flat, &mut m).unwrap().is_none());
}
