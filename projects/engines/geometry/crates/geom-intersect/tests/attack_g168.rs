//! Attacks on piece G168's claims: known curves found exactly where they are, closed and open, each
//! level function against its closed form, nothing missed on random pairs against a dense grid,
//! close branches kept apart, singular points and creases flagged, and every kind's equation.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{CurveError, Frame};
use geom_intersect::result::CurveEnd;
use geom_intersect::seam::Passing;
use geom_intersect::{
    Branch, Envelope, IntersectError, Isocline, LevelCurves, OnImplicit, Perspective, Silhouette,
    trace,
};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, ImplicitBound, ImplicitEnclosure, ParamSurface, Plane, Sphere, SurfaceKind,
    Torus,
};
use geom_test::Seed;
use geom_test::rng::SplitMix64;

const FLOOR: f64 = 1e-7;
const CEILING: f64 = 1e-3;
const TURN: f64 = 2.0;
const PI: f64 = core::f64::consts::PI;

/// A meter above what any case here takes, so a trace that loses its way fails, not hangs.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 20))
}

fn frame() -> Frame<3> {
    Frame::standard().unwrap()
}

fn patch(reach: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-reach, reach), (-reach, reach)),
        )
        .unwrap(),
    )
}

/// The middle of a vertex.
fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// Whether a vertex's coordinate holds `exact`, to `slack`.
fn holds(x: Interval, exact: f64, slack: f64) -> bool {
    x.lo() - slack <= exact && exact <= x.hi() + slack
}

fn only(found: &LevelCurves) -> &Branch {
    assert_eq!(found.branches.len(), 1, "{found:?}");
    assert!(found.flagged.is_empty(), "{:?}", found.flagged);
    &found.branches[0]
}

#[test]
fn a_sphere_cuts_a_plane_patch_in_one_closed_circle() {
    // Centre height 3, radius 5: the circle of radius 4 about (1, 2).
    let sphere = Sphere::new([1.0, 2.0, 3.0], 5.0, frame(), ((0.0, TURN), (-0.5, 0.5))).unwrap();
    let found = trace(
        &patch(10.0),
        &OnImplicit(&sphere),
        FLOOR,
        CEILING,
        &mut meter(),
    )
    .unwrap();
    let branch = only(&found);
    assert_eq!(branch.ends, [CurveEnd::Closed, CurveEnd::Closed]);
    assert!(branch.vertices.len() >= 16, "{branch:?}");
    assert!(branch.deviation <= CEILING, "{}", branch.deviation);
    let mut turned: f64 = 0.0;
    let mut last = None;
    for [u, v] in &branch.vertices {
        let (x, y) = (mid(*u) - 1.0, mid(*v) - 2.0);
        assert!((x.hypot(y) - 4.0).abs() < 2e-6, "{x} {y}");
        let angle = y.atan2(x);
        if let Some(a) = last {
            let mut step: f64 = angle - a;
            if step > PI {
                step -= 2.0 * PI;
            }
            if step < -PI {
                step += 2.0 * PI;
            }
            turned += step;
        }
        last = Some(angle);
    }
    // Once around, and always the same way: the vertices are in order along the branch.
    assert!((turned.abs() - 2.0 * PI).abs() < 1.0, "{turned}");
}

/// The chords stay within the bound the branch reports.
#[test]
fn the_chords_stay_within_the_deviation_reported() {
    let sphere = Sphere::new([0.0, 0.0, 3.0], 5.0, frame(), ((0.0, TURN), (-0.5, 0.5))).unwrap();
    let found = trace(
        &patch(10.0),
        &OnImplicit(&sphere),
        FLOOR,
        CEILING,
        &mut meter(),
    )
    .unwrap();
    let branch = only(&found);
    let n = branch.vertices.len();
    for k in 0..n {
        let [a, b] = branch.vertices[k];
        let [c, d] = branch.vertices[(k + 1) % n];
        let (x, y) = (mid(a) / 2.0 + mid(c) / 2.0, mid(b) / 2.0 + mid(d) / 2.0);
        assert!(
            (x.hypot(y) - 4.0).abs() <= branch.deviation + 2e-6,
            "{k}: {}",
            x.hypot(y)
        );
    }
}

/// A finer ceiling takes more boxes and keeps its bound.
#[test]
fn the_ceiling_sets_how_fine_a_branch_is() {
    let sphere = Sphere::new([0.0, 0.0, 3.0], 5.0, frame(), ((0.0, TURN), (-0.5, 0.5))).unwrap();
    let mut last = 0;
    for ceiling in [1e-1, 1e-3, 1e-5] {
        let found = trace(
            &patch(10.0),
            &OnImplicit(&sphere),
            FLOOR,
            ceiling,
            &mut meter(),
        )
        .unwrap();
        let branch = only(&found);
        assert!(branch.deviation <= ceiling, "{}", branch.deviation);
        assert!(branch.vertices.len() > last, "{}", branch.vertices.len());
        last = branch.vertices.len();
    }
}

#[test]
fn a_plane_cuts_a_cylinder_along_its_known_curve() {
    // z = y on a cylinder of radius 3: v = 3 sin πu, from one end of the domain to the other.
    let cylinder = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 3.0, frame(), ((0.0, TURN), (-20.0, 20.0))).unwrap(),
    );
    let plane = Plane::new(
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 1.0],
        ((-9.0, 9.0), (-9.0, 9.0)),
    )
    .unwrap();
    let found = trace(&cylinder, &OnImplicit(&plane), FLOOR, CEILING, &mut meter()).unwrap();
    let branch = only(&found);
    // The domain is one whole turn, so its two ends are one line and the curve one loop across
    // it (G173), the seam's point kept once.
    assert_eq!(branch.ends, [CurveEnd::Closed, CurveEnd::Closed]);
    assert_eq!(branch.passes.len(), 1, "{branch:?}");
    assert_eq!(branch.passes[0].passing, Passing::Seam);
    let seam = branch.vertices[branch.passes[0].at][0];
    assert!(seam.contains(0.0) || seam.contains(TURN), "{branch:?}");
    for [u, v] in &branch.vertices {
        assert!(holds(*v, 3.0 * (PI * mid(*u)).sin(), 2e-6), "{u:?} {v:?}");
    }
    // u runs one way round: every step but the seam's moves it the same way.
    let n = branch.vertices.len();
    let back = (0..n)
        .filter(|&k| branch.vertices[(k + 1) % n][0].hi() < branch.vertices[k][0].lo())
        .count();
    assert!(back == 1 || back == n - 1, "{back} of {n}");
}

#[test]
fn a_plane_through_a_torus_axis_cuts_two_closed_circles() {
    // The plane y = 0 on the torus's u-span holding it twice: v's circle at u = 0 and at u = 1.
    let torus = SurfaceKind::Torus(
        Torus::new([0.0; 3], 6.0, 2.0, frame(), ((-0.25, 1.75), (-1.0, 1.0))).unwrap(),
    );
    let plane = Plane::new(
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        ((-9.0, 9.0), (-9.0, 9.0)),
    )
    .unwrap();
    let found = trace(&torus, &OnImplicit(&plane), FLOOR, CEILING, &mut meter()).unwrap();
    assert_eq!(found.branches.len(), 2, "{found:?}");
    assert!(found.flagged.is_empty(), "{:?}", found.flagged);
    for (branch, u) in found.branches.iter().zip([0.0, 1.0]) {
        // v spans a whole turn too, so each circle is one loop across v's seam (G173).
        assert_eq!(branch.ends, [CurveEnd::Closed, CurveEnd::Closed]);
        assert_eq!(branch.passes.len(), 1, "{branch:?}");
        assert_eq!(branch.passes[0].passing, Passing::Seam);
        for [x, _] in &branch.vertices {
            assert!(holds(*x, u, 1e-6), "{x:?}");
        }
    }
}

#[test]
fn a_parallel_silhouette_of_a_sphere_is_its_equator() {
    let sphere = SurfaceKind::Sphere(
        Sphere::new([1.0, 2.0, 3.0], 5.0, frame(), ((0.0, TURN), (-0.4, 0.4))).unwrap(),
    );
    let level = Silhouette {
        direction: [0.0, 0.0, 2.0],
    };
    let found = trace(&sphere, &level, FLOOR, CEILING, &mut meter()).unwrap();
    let branch = only(&found);
    // A whole turn in u: the equator is one loop across the seam (G173).
    assert_eq!(branch.ends, [CurveEnd::Closed, CurveEnd::Closed]);
    assert_eq!(branch.passes.len(), 1, "{branch:?}");
    for [_, v] in &branch.vertices {
        assert!(holds(*v, 0.0, 1e-7), "{v:?}");
    }
}

#[test]
fn a_perspective_silhouette_of_a_sphere_is_its_tangent_circle() {
    // From height 13 over a sphere of radius 5, the tangent points are at height 25/13.
    let sphere = SurfaceKind::Sphere(
        Sphere::new([0.0; 3], 5.0, frame(), ((0.0, TURN), (-0.45, 0.45))).unwrap(),
    );
    let level = Perspective {
        eye: [0.0, 0.0, 13.0],
    };
    let found = trace(&sphere, &level, FLOOR, CEILING, &mut meter()).unwrap();
    let branch = only(&found);
    let v0 = (5.0f64 / 13.0).asin() / PI;
    for [_, v] in &branch.vertices {
        assert!(holds(*v, v0, 1e-7), "{v:?} against {v0}");
    }
}

#[test]
fn an_isocline_of_a_sphere_is_both_of_its_latitudes() {
    let sphere = SurfaceKind::Sphere(
        Sphere::new([0.0; 3], 5.0, frame(), ((0.0, TURN), (-0.45, 0.45))).unwrap(),
    );
    let level = Isocline {
        direction: [0.0, 0.0, 1.0],
        cosine: 0.6,
    };
    let found = trace(&sphere, &level, FLOOR, CEILING, &mut meter()).unwrap();
    assert_eq!(found.branches.len(), 2, "{found:?}");
    let v0 = 0.6f64.asin() / PI;
    let mut signs = [false; 2];
    for branch in &found.branches {
        for [_, v] in &branch.vertices {
            let positive = mid(*v) > 0.0;
            signs[usize::from(positive)] = true;
            assert!(
                holds(*v, if positive { v0 } else { -v0 }, 1e-7),
                "{v:?} against {v0}"
            );
        }
    }
    assert_eq!(signs, [true, true]);
}

#[test]
fn an_envelope_of_a_turning_sphere_is_the_great_circle_through_the_centre() {
    // A sphere at the origin turning about the z axis through (5, 0, 0): y = 0.
    let sphere = SurfaceKind::Sphere(
        Sphere::new([0.0; 3], 3.0, frame(), ((0.5, 1.5), (-0.4, 0.4))).unwrap(),
    );
    let level = Envelope {
        linear: [0.0; 3],
        angular: [0.0, 0.0, 1.0],
        about: [5.0, 0.0, 0.0],
    };
    let found = trace(&sphere, &level, FLOOR, CEILING, &mut meter()).unwrap();
    let branch = only(&found);
    for [u, _] in &branch.vertices {
        assert!(holds(*u, 1.0, 1e-7), "{u:?}");
    }
}

/// A function of the plane's x and y through a closure, for curves with a known shape.
struct Curve {
    f: fn(f64, f64) -> [f64; 3],
    smooth: fn(&[Interval; 3]) -> bool,
    bound: fn(&[Interval; 3]) -> Result<[Interval; 3], CurveError>,
}

impl ImplicitBound for Curve {
    fn enclose_box(&self, region: [Interval; 3]) -> Result<ImplicitEnclosure, CurveError> {
        let _ = self.f;
        let [value, x, y] = (self.bound)(&region)?;
        Ok(ImplicitEnclosure {
            value,
            gradient: [x, y, Interval::point(0.0)?],
            smooth: (self.smooth)(&region),
        })
    }
}

fn nothing(_: f64, _: f64) -> [f64; 3] {
    [0.0; 3]
}

fn always(_: &[Interval; 3]) -> bool {
    true
}

#[test]
fn two_close_branches_are_kept_apart() {
    // (x − 0.3)(x − 0.301): two lines a thousandth apart.
    fn bound(r: &[Interval; 3]) -> Result<[Interval; 3], CurveError> {
        let a = r[0].sub(Interval::point(0.3)?)?;
        let b = r[0].sub(Interval::point(0.301)?)?;
        Ok([a.mul(b)?, a.add(b)?, Interval::point(0.0)?])
    }
    let curve = Curve {
        f: nothing,
        smooth: always,
        bound,
    };
    let found = trace(
        &patch(1.0),
        &OnImplicit(&curve),
        FLOOR,
        CEILING,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(found.branches.len(), 2, "{found:?}");
    assert!(found.flagged.is_empty(), "{:?}", found.flagged);
    for (branch, x) in found.branches.iter().zip([0.3, 0.301]) {
        assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
        for [u, _] in &branch.vertices {
            assert!(holds(*u, x, 1e-6), "{u:?} against {x}");
        }
    }
}

#[test]
fn a_lemniscate_stops_at_its_singular_point_and_flags_it() {
    // (x² + y²)² − 2(x² − y²): two lobes meeting at the origin, where the gradient vanishes.
    fn bound(r: &[Interval; 3]) -> Result<[Interval; 3], CurveError> {
        let (x2, y2) = (r[0].square()?, r[1].square()?);
        let sum = x2.add(y2)?;
        let two = Interval::point(2.0)?;
        let four = Interval::point(4.0)?;
        Ok([
            sum.square()?.sub(two.mul(x2.sub(y2)?)?)?,
            four.mul(r[0])?.mul(sum.sub(Interval::point(1.0)?)?)?,
            four.mul(r[1])?.mul(sum.add(Interval::point(1.0)?)?)?,
        ])
    }
    let curve = Curve {
        f: nothing,
        smooth: always,
        bound,
    };
    let found = trace(&patch(3.0), &OnImplicit(&curve), 1e-4, 1e-3, &mut meter()).unwrap();
    assert!(!found.flagged.is_empty(), "{found:?}");
    for flagged in &found.flagged {
        let [u, v] = flagged.at;
        assert!(u.lo().abs() < 2e-2 && v.lo().abs() < 2e-2, "{flagged:?}");
    }
    assert_eq!(found.branches.len(), 2, "{:?}", found.branches.len());
    for branch in &found.branches {
        assert_eq!(branch.ends, [CurveEnd::Flagged, CurveEnd::Flagged]);
    }
}

#[test]
fn a_crease_is_flagged_and_its_branches_end_there() {
    // y = |x| - 1/4, whose gradient jumps along x = 0.
    fn bound(r: &[Interval; 3]) -> Result<[Interval; 3], CurveError> {
        let (lo, hi) = (r[0].lo(), r[0].hi());
        let abs = if lo >= 0.0 {
            r[0]
        } else if hi <= 0.0 {
            r[0].neg()
        } else {
            Interval::new(0.0, (-lo).max(hi))?
        };
        let slope = if lo >= 0.0 {
            Interval::point(1.0)?
        } else if hi <= 0.0 {
            Interval::point(-1.0)?
        } else {
            Interval::new(-1.0, 1.0)?
        };
        Ok([
            r[1].sub(abs)?.add(Interval::point(0.25)?)?,
            slope.neg(),
            Interval::point(1.0)?,
        ])
    }
    fn away(r: &[Interval; 3]) -> bool {
        r[0].lo() > 0.0 || r[0].hi() < 0.0
    }
    let curve = Curve {
        f: nothing,
        smooth: away,
        bound,
    };
    let found = trace(&patch(1.0), &OnImplicit(&curve), 1e-4, 1e-3, &mut meter()).unwrap();
    assert!(!found.flagged.is_empty(), "{found:?}");
    assert_eq!(found.branches.len(), 2, "{:?}", found.branches);
    for branch in &found.branches {
        assert!(
            branch.ends.contains(&CurveEnd::Flagged),
            "{:?}",
            branch.ends
        );
        assert!(
            branch.ends.contains(&CurveEnd::Boundary),
            "{:?}",
            branch.ends
        );
    }
}

/// A diagonal of a square domain runs through the corners of the first cuts in both parameters:
/// the cuts move off it, and it is one branch. Only the domain's own corners stay on it.
#[test]
fn a_diagonal_through_the_cuts_corners_is_one_branch() {
    fn bound(r: &[Interval; 3]) -> Result<[Interval; 3], CurveError> {
        Ok([
            r[0].sub(r[1])?,
            Interval::point(1.0)?,
            Interval::point(-1.0)?,
        ])
    }
    let curve = Curve {
        f: nothing,
        smooth: always,
        bound,
    };
    let found = trace(&patch(1.0), &OnImplicit(&curve), 1e-5, 1e-3, &mut meter()).unwrap();
    assert_eq!(found.branches.len(), 1, "{found:?}");
    let branch = &found.branches[0];
    assert_eq!(branch.ends, [CurveEnd::Flagged, CurveEnd::Flagged]);
    for flagged in &found.flagged {
        let corner = flagged.at[0].lo().abs().max(flagged.at[0].hi().abs());
        assert!(corner > 0.99, "{flagged:?}");
    }
}

#[test]
fn a_touching_sphere_is_flagged_not_a_curve() {
    // Tangent to the patch at (1, 2): a single point, which the tests cannot decide.
    let sphere = Sphere::new([1.0, 2.0, 5.0], 5.0, frame(), ((0.0, TURN), (-0.5, 0.5))).unwrap();
    let found = trace(&patch(10.0), &OnImplicit(&sphere), 1e-5, 1e-3, &mut meter()).unwrap();
    assert!(found.branches.is_empty(), "{found:?}");
    assert_ne!(found.flagged.len(), 0);
    for flagged in &found.flagged {
        assert!(holds(flagged.at[0], 1.0, 0.1), "{flagged:?}");
        assert!(holds(flagged.at[1], 2.0, 0.1), "{flagged:?}");
    }
}

/// A flat strip over the unit square of parameters, so each parameter has a speed of its own.
fn strip(long: f64, wide: f64) -> SurfaceKind {
    let basis = || Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let net = [
        [0.0, 0.0, 0.0],
        [0.0, wide, 0.0],
        [long, 0.0, 0.0],
        [long, wide, 0.0],
    ];
    SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap())
}

/// The floor is a length in space, along whichever parameter runs faster.
#[test]
fn a_flagged_box_stops_at_the_floor_in_space_along_both_parameters() {
    let floor = 1e-2;
    for (long, wide) in [(64.0, 1.0), (1.0, 64.0)] {
        let centre = [long / 2.0 + 0.0625, wide / 2.0 + 0.03125, 1.0];
        let sphere = Sphere::new(centre, 1.0, frame(), ((0.0, TURN), (-0.5, 0.5))).unwrap();
        let surface = strip(long, wide);
        let found = trace(&surface, &OnImplicit(&sphere), floor, 1.0, &mut meter()).unwrap();
        assert_ne!(found.flagged.len(), 0);
        for flagged in &found.flagged {
            let [u, v] = flagged.at;
            let reach = ((u.hi() - u.lo()) * long).max((v.hi() - v.lo()) * wide);
            assert!(reach <= 8.0 * floor, "{flagged:?}");
        }
    }
}

#[test]
fn a_sphere_clear_of_the_patch_has_nothing() {
    let sphere = Sphere::new([1.0, 2.0, 9.0], 5.0, frame(), ((0.0, TURN), (-0.5, 0.5))).unwrap();
    let found = trace(
        &patch(10.0),
        &OnImplicit(&sphere),
        FLOOR,
        CEILING,
        &mut meter(),
    )
    .unwrap();
    assert!(
        found.branches.is_empty() && found.flagged.is_empty(),
        "{found:?}"
    );
}

#[test]
fn a_budget_stops_a_trace_and_changes_nothing_it_lets_finish() {
    let sphere = Sphere::new([1.0, 2.0, 3.0], 5.0, frame(), ((0.0, TURN), (-0.5, 0.5))).unwrap();
    let level = OnImplicit(&sphere);
    let surface = patch(10.0);
    let tight = trace(
        &surface,
        &level,
        FLOOR,
        CEILING,
        &mut WorkMeter::new(Budget::of_work(10)),
    );
    assert_eq!(tight, Err(IntersectError::BudgetExceeded));
    let free = trace(&surface, &level, FLOOR, CEILING, &mut meter()).unwrap();
    let again = trace(
        &surface,
        &level,
        FLOOR,
        CEILING,
        &mut WorkMeter::new(Budget::of_work(1 << 40)),
    )
    .unwrap();
    assert_eq!(free, again);
}

#[test]
fn a_floor_or_ceiling_that_is_not_positive_is_refused() {
    let sphere = Sphere::new([1.0, 2.0, 3.0], 5.0, frame(), ((0.0, TURN), (-0.5, 0.5))).unwrap();
    let level = OnImplicit(&sphere);
    let surface = patch(10.0);
    for (floor, ceiling) in [
        (0.0, 1.0),
        (-1.0, 1.0),
        (f64::NAN, 1.0),
        (1.0, 0.0),
        (1.0, -1.0),
        (1.0, f64::NAN),
    ] {
        assert_eq!(
            trace(&surface, &level, floor, ceiling, &mut meter()),
            Err(IntersectError::InvalidInput)
        );
    }
}

fn rng(label: &str, seed: u64) -> SplitMix64 {
    SplitMix64::new(geom_test::seed::resolve(label, Seed(seed)).unwrap())
}

fn unit(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

/// Doubles for the sign of F on S at `(u, v)`.
fn on(surface: &SurfaceKind, sphere: &[f64; 4], u: f64, v: f64, sides: (Limit, Limit)) -> f64 {
    let p = surface.point(u, v, sides).unwrap();
    let d = [p[0] - sphere[0], p[1] - sphere[1], p[2] - sphere[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2] - sphere[3] * sphere[3]
}

/// The least distance from `(u, v)` to a branch's chords, in parameters.
fn reach(branch: &Branch, u: f64, v: f64) -> f64 {
    let points: Vec<[f64; 2]> = branch
        .vertices
        .iter()
        .map(|[a, b]| [mid(*a), mid(*b)])
        .collect();
    let closing =
        (branch.ends[0] == CurveEnd::Closed).then(|| [*points.last().unwrap(), points[0]]);
    points
        .windows(2)
        .map(|w| [w[0], w[1]])
        .chain(closing)
        .map(|[a, b]| {
            let (x, y) = (b[0] - a[0], b[1] - a[1]);
            let t = ((u - a[0]) * x + (v - a[1]) * y) / (x * x + y * y).max(1e-300);
            let t = t.clamp(0.0, 1.0);
            (u - a[0] - t * x).hypot(v - a[1] - t * y)
        })
        .fold(f64::INFINITY, f64::min)
}

/// Every sign change a dense grid sees is on a branch: the trace misses nothing the grid can see,
/// on every kind, and every vertex is on the curve.
#[test]
fn nothing_a_dense_grid_sees_is_missed() {
    const CELLS: u32 = 96;
    let mut r = rng("g168 grid", 0x4731_3638);
    let rounds = if cfg!(debug_assertions) { 8 } else { 240 };
    for round in 0..rounds {
        let radius = 2.0 + unit(&mut r) * 6.0;
        let sphere = [
            unit(&mut r) * 12.0 - 6.0,
            unit(&mut r) * 12.0 - 6.0,
            unit(&mut r) * 12.0 - 6.0,
            radius,
        ];
        let surface = match round % 4 {
            0 => patch(9.0),
            1 => SurfaceKind::Cylinder(
                Cylinder::new([0.0; 3], 4.0, frame(), ((0.1, 1.9), (-9.0, 9.0))).unwrap(),
            ),
            2 => SurfaceKind::Torus(
                Torus::new([0.0; 3], 6.0, 2.0, frame(), ((0.1, 1.9), (0.1, 1.9))).unwrap(),
            ),
            _ => SurfaceKind::Cone(
                Cone::new([0.0; 3], 2.0, 0.5, frame(), ((0.1, 1.9), (-1.0, 9.0))).unwrap(),
            ),
        };
        let level = Sphere::new(
            [sphere[0], sphere[1], sphere[2]],
            radius,
            frame(),
            ((0.0, TURN), (-0.5, 0.5)),
        )
        .unwrap();
        let found = trace(&surface, &OnImplicit(&level), 1e-6, 1e-2, &mut meter()).unwrap();
        for branch in &found.branches {
            for [u, v] in &branch.vertices {
                let g = on(
                    &surface,
                    &sphere,
                    mid(*u),
                    mid(*v),
                    (Limit::FromLeft, Limit::FromLeft),
                );
                assert!(g.abs() < 1e-3, "round {round}: {g} at {u:?} {v:?}");
            }
        }
        let ((a, b), (c, d)) = surface.domain();
        let (du, dv) = ((b - a) / f64::from(CELLS), (d - c) / f64::from(CELLS));
        for i in 0..CELLS {
            for j in 0..CELLS {
                let (u, v) = (a + du * f64::from(i), c + dv * f64::from(j));
                let sides = (Limit::FromRight, Limit::FromRight);
                let here = on(&surface, &sphere, u, v, sides);
                let left_side = (Limit::FromLeft, Limit::FromRight);
                let right = on(&surface, &sphere, (u + du).min(b), v, left_side);
                let lower = (Limit::FromRight, Limit::FromLeft);
                let up = on(&surface, &sphere, u, (v + dv).min(d), lower);
                if (here > 0.0) != (right > 0.0) || (here > 0.0) != (up > 0.0) {
                    let near = found
                        .branches
                        .iter()
                        .map(|b| reach(b, u, v))
                        .fold(f64::INFINITY, f64::min);
                    let flagged = found
                        .flagged
                        .iter()
                        .any(|f| holds(f.at[0], u, du * 2.0) && holds(f.at[1], v, dv * 2.0));
                    assert!(
                        near <= 2.0 * du.hypot(dv) || flagged,
                        "round {round}: nothing near ({u}, {v}), {near} away: {found:?}"
                    );
                }
            }
        }
    }
}

/// Each kind's equation vanishes on it, with its gradient along its normal.
#[test]
fn each_kinds_equation_vanishes_on_it_with_its_gradient_along_its_normal() {
    let frame = Frame::new([1.0, 2.0, 2.0], [2.0, -2.0, 1.0]).unwrap();
    let domain = ((0.1, 1.9), (-0.4, 0.4));
    let plane = Plane::new([1.0, 2.0, 3.0], [1.0, 1.0, 0.0], [0.0, 1.0, 2.0], domain).unwrap();
    let sphere = Sphere::new([1.0, 2.0, 3.0], 5.0, frame, domain).unwrap();
    let cylinder = Cylinder::new([1.0, 2.0, 3.0], 5.0, frame, domain).unwrap();
    let cone = Cone::new([1.0, 2.0, 3.0], 5.0, 0.25, frame, domain).unwrap();
    let torus = Torus::new([1.0, 2.0, 3.0], 7.0, 3.0, frame, domain).unwrap();
    let kinds: Vec<(SurfaceKind, Box<dyn ImplicitBound>)> = vec![
        (SurfaceKind::Plane(plane.clone()), Box::new(plane)),
        (SurfaceKind::Sphere(sphere.clone()), Box::new(sphere)),
        (SurfaceKind::Cylinder(cylinder.clone()), Box::new(cylinder)),
        (SurfaceKind::Cone(cone.clone()), Box::new(cone)),
        (SurfaceKind::Torus(torus.clone()), Box::new(torus)),
    ];
    let mut r = rng("g168 kinds", 0x4731_3639);
    for (kind, implicit) in &kinds {
        for _ in 0..500 {
            let (u, v) = (0.1 + unit(&mut r) * 1.8, unit(&mut r) * 0.8 - 0.4);
            let [s, su, sv] = kind
                .partials(u, v, (Limit::FromRight, Limit::FromRight))
                .unwrap();
            let e = implicit
                .enclose_box(s.map(|x| Interval::point(x).unwrap()))
                .unwrap();
            let gradient = e.gradient.map(mid);
            assert!(e.smooth);
            let length = gradient.iter().map(|x| x * x).sum::<f64>().sqrt();
            assert!(e.value.lo() < 1e-9 * (1.0 + length), "{u} {v}: {e:?}");
            assert!(e.value.hi() > -1e-9 * (1.0 + length), "{u} {v}: {e:?}");
            for t in [su, sv] {
                let along: f64 = t.iter().zip(&gradient).map(|(x, y)| x * y).sum();
                let size = t.iter().map(|x| x * x).sum::<f64>().sqrt();
                assert!(along.abs() < 1e-9 * size * length, "{u} {v}: {along}");
            }
        }
    }
}

/// A box's enclosures hold every point in it: the equations' gradients are true rates.
#[test]
fn each_kinds_gradient_is_the_rate_of_its_equation() {
    let frame = Frame::new([1.0, 2.0, 2.0], [2.0, -2.0, 1.0]).unwrap();
    let domain = ((0.1, 1.9), (-0.4, 0.4));
    let cone = Cone::new([1.0, 2.0, 3.0], 5.0, 0.25, frame, domain).unwrap();
    let torus = Torus::new([1.0, 2.0, 3.0], 7.0, 3.0, frame, domain).unwrap();
    let kinds: [&dyn ImplicitBound; 2] = [&cone, &torus];
    let mut r = rng("g168 rates", 0x4731_3640);
    for kind in kinds {
        for _ in 0..500 {
            let p: [f64; 3] = core::array::from_fn(|_| unit(&mut r) * 20.0 - 10.0);
            let at = |q: [f64; 3]| {
                kind.enclose_box(q.map(|x| Interval::point(x).unwrap()))
                    .unwrap()
            };
            let gradient = at(p).gradient;
            for axis in 0..3 {
                let h = 1e-5;
                let (mut above, mut below) = (p, p);
                above[axis] += h;
                below[axis] -= h;
                let rate = (mid(at(above).value) - mid(at(below).value)) / (2.0 * h);
                let g = mid(gradient[axis]);
                assert!(
                    (rate - g).abs() < 1e-4 * (1.0 + g.abs()),
                    "{axis}: {rate} {g}"
                );
            }
        }
    }
}

/// A function that jumps across a knot line: each side's branch ends there, at a seam, and each side
/// is read from its own side. z = xy left of x = 0 and x(y − 1) right of it, a crease along x = 0,
/// has `z_x` = 0 along y = 0 on the left and y = 1 on the right.
#[test]
fn a_branch_ends_at_a_seam_where_the_function_jumps() {
    let across = Basis::new(1, vec![0.0, 0.0, 0.5, 1.0, 1.0]).unwrap();
    let along = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let net = [
        [-1.0, -2.0, 2.0],
        [-1.0, 2.0, -2.0],
        [0.0, -2.0, 0.0],
        [0.0, 2.0, 0.0],
        [1.0, -2.0, -3.0],
        [1.0, 2.0, 1.0],
    ];
    let roof = SurfaceKind::Nurbs(Surface::new(across, along, &net).unwrap());
    let level = Silhouette {
        direction: [1.0, 0.0, 0.0],
    };
    let found = trace(&roof, &level, FLOOR, CEILING, &mut meter()).unwrap();
    assert_eq!(found.branches.len(), 2, "{found:?}");
    assert!(found.flagged.is_empty(), "{:?}", found.flagged);
    let (left, right) = (&found.branches[0], &found.branches[1]);
    assert!(left.ends.contains(&CurveEnd::Seam), "{:?}", left.ends);
    assert!(left.ends.contains(&CurveEnd::Boundary), "{:?}", left.ends);
    assert!(right.ends.contains(&CurveEnd::Seam), "{:?}", right.ends);
    for [u, v] in &left.vertices {
        assert!(mid(*u) <= 0.5 + 1e-9, "{u:?}");
        assert!(holds(*v, 0.5, 1e-5), "{v:?}");
    }
    for [u, v] in &right.vertices {
        assert!(mid(*u) >= 0.5 - 1e-9, "{u:?}");
        assert!(holds(*v, 0.75, 1e-5), "{v:?}");
    }
}

/// A root exactly on a bisection's middle still narrows, from both sides.
#[test]
fn a_root_on_a_middle_is_narrowed_from_both_sides() {
    fn bound(r: &[Interval; 3]) -> Result<[Interval; 3], CurveError> {
        Ok([
            r[1].sub(Interval::point(0.5)?)?,
            Interval::point(0.0)?,
            Interval::point(1.0)?,
        ])
    }
    // The line y = 1/2 on a patch whose left edge runs from −1 to 2: the root is 1/2, the middle.
    let wide = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-1.0, 1.0), (-1.0, 2.0)),
        )
        .unwrap(),
    );
    let curve = Curve {
        f: nothing,
        smooth: always,
        bound,
    };
    let found = trace(&wide, &OnImplicit(&curve), 1e-4, 1e-2, &mut meter()).unwrap();
    assert_eq!(found.branches.len(), 1, "{found:?}");
    for [_, v] in [
        found.branches[0].vertices[0],
        *found.branches[0].vertices.last().unwrap(),
    ] {
        assert!(v.contains(0.5), "{v:?}");
        assert!(v.hi() - v.lo() <= 1e-4, "{v:?}");
    }
}

/// Three arcs in one box pair up by their order along the box.
#[test]
fn three_arcs_in_one_box_pair_up_in_order() {
    // y = x³ − x, through a strip it crosses three times.
    fn bound(r: &[Interval; 3]) -> Result<[Interval; 3], CurveError> {
        let x2 = r[0].square()?;
        let cubic = x2.mul(r[0])?.sub(r[0])?;
        let three = Interval::point(3.0)?;
        Ok([
            r[1].sub(cubic)?,
            three.mul(x2)?.sub(Interval::point(1.0)?)?.neg(),
            Interval::point(1.0)?,
        ])
    }
    let curve = Curve {
        f: nothing,
        smooth: always,
        bound,
    };
    let strip = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-1.3, 1.4), (-0.1, 0.15)),
        )
        .unwrap(),
    );
    let found = trace(&strip, &OnImplicit(&curve), 1e-3, 1e3, &mut meter()).unwrap();
    assert_eq!(found.branches.len(), 3, "{found:?}");
    for (branch, root) in found.branches.iter().zip([-1.0, 0.0, 1.0]) {
        assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
        for [u, _] in &branch.vertices {
            assert!((mid(*u) - root).abs() < 0.2, "{u:?} against {root}");
        }
    }
}

/// A curve that only touches the domain's edge cannot be told from one that misses it: flagged
/// there, and nowhere else.
#[test]
fn a_curve_touching_the_edge_is_flagged_there() {
    // The circle of radius 1 about (0, 0) in a patch whose top edge is y = 1.
    let sphere = Sphere::new([0.0; 3], 1.0, frame(), ((0.0, TURN), (-0.5, 0.5))).unwrap();
    let cut = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-3.0, 3.0), (-3.0, 1.0)),
        )
        .unwrap(),
    );
    let found = trace(&cut, &OnImplicit(&sphere), 1e-4, 1e-2, &mut meter()).unwrap();
    assert_ne!(found.flagged.len(), 0, "{found:?}");
    for flagged in &found.flagged {
        assert!(holds(flagged.at[0], 0.0, 1e-1), "{flagged:?}");
        assert!(holds(flagged.at[1], 1.0, 1e-3), "{flagged:?}");
    }
}

/// A cut whose ends g cannot sign moves off: a line through one end of the first cut, on either
/// parameter, stays one branch.
#[test]
fn a_cut_moves_off_a_curve_through_one_of_its_ends() {
    fn across(r: &[Interval; 3]) -> Result<[Interval; 3], CurveError> {
        // Through (−1/8, 1), the top end of the first cut in u.
        let x = r[0].add(Interval::point(0.125)?)?;
        Ok([
            r[1].sub(Interval::point(1.0)?)?
                .sub(Interval::point(3.0)?.mul(x)?)?,
            Interval::point(-3.0)?,
            Interval::point(1.0)?,
        ])
    }
    fn up(r: &[Interval; 3]) -> Result<[Interval; 3], CurveError> {
        // Through (1, −1/8), the right end of the first cut in v.
        let y = r[1].add(Interval::point(0.125)?)?;
        Ok([
            r[0].sub(Interval::point(1.0)?)?
                .sub(Interval::point(3.0)?.mul(y)?)?,
            Interval::point(1.0)?,
            Interval::point(-3.0)?,
        ])
    }
    for bound in [
        across as fn(&[Interval; 3]) -> Result<[Interval; 3], CurveError>,
        up,
    ] {
        let curve = Curve {
            f: nothing,
            smooth: always,
            bound,
        };
        let found = trace(&patch(1.0), &OnImplicit(&curve), 1e-5, 1e-3, &mut meter()).unwrap();
        assert_eq!(found.branches.len(), 1, "{found:?}");
        assert!(found.flagged.is_empty(), "{:?}", found.flagged);
    }
}

/// A floor no double can reach: boxes run out of doubles, and what is left is flagged.
#[test]
fn a_floor_below_the_doubles_still_ends() {
    fn bound(r: &[Interval; 3]) -> Result<[Interval; 3], CurveError> {
        let (x, y) = (
            r[0].sub(Interval::point(0.3)?)?,
            r[1].sub(Interval::point(0.7)?)?,
        );
        let two = Interval::point(2.0)?;
        Ok([x.square()?.add(y.square()?)?, two.mul(x)?, two.mul(y)?])
    }
    let curve = Curve {
        f: nothing,
        smooth: always,
        bound,
    };
    let mut free = WorkMeter::new(Budget::of_work(1 << 22));
    let found = trace(&patch(1.0), &OnImplicit(&curve), 1e-300, 1e-2, &mut free).unwrap();
    assert!(found.branches.is_empty(), "{found:?}");
    assert_ne!(found.flagged.len(), 0);
}

/// A trace's work and the shape of its answer, to the unit: a trace is a pure function, so a change
/// here is a change to what is pruned or to how far a chord is said to reach, which a test of where
/// the curve lies cannot see.
#[test]
fn each_trace_takes_the_work_it_always_took() {
    let sphere = Sphere::new([1.0, 2.0, 3.0], 5.0, frame(), ((0.0, TURN), (-0.5, 0.5))).unwrap();
    let cylinder = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 3.0, frame(), ((0.0, TURN), (-20.0, 20.0))).unwrap(),
    );
    let tilted = Plane::new(
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 1.0],
        ((-9.0, 9.0), (-9.0, 9.0)),
    )
    .unwrap();
    let torus = SurfaceKind::Torus(
        Torus::new([0.0; 3], 6.0, 2.0, frame(), ((0.1, 1.9), (0.1, 1.9))).unwrap(),
    );
    let flat = patch(10.0);
    let circle = OnImplicit(&sphere);
    let ellipse = OnImplicit(&tilted);
    let outline = Silhouette {
        direction: [1.0, 2.0, 3.0],
    };
    // Tangent to an edge from outside the strip: a double root that only the floor can stop, along
    // a parameter whose speed is not 1.
    let beside = Sphere::new([-1.0, 32.5, 0.0], 1.0, frame(), ((0.0, TURN), (-0.5, 0.5))).unwrap();
    let edge = strip(1.0, 64.0);
    let edged = OnImplicit(&beside);
    // Dipping out of the band and back: two arcs in one cell, over spans of u that must not overlap.
    let band = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-6.0, 6.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    let dipping = Sphere::new([0.0, 9.5, 0.0], 10.0, frame(), ((0.0, TURN), (-0.5, 0.5))).unwrap();
    let dip = OnImplicit(&dipping);
    // A root the bisection lands on exactly: 0, 4, 2, 1, 1.5, 1.25.
    let upright = Plane::new(
        [1.25, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        ((-1.0, 1.0), (-1.0, 1.0)),
    )
    .unwrap();
    let wall = patch(8.0);
    let exact = OnImplicit(&upright);
    let cases: [(
        &SurfaceKind,
        &dyn geom_intersect::Level,
        f64,
        u64,
        usize,
        u64,
    ); 6] = [
        (&flat, &circle, 1e-2, 27_162, 131, 4_576_741_634_584_209_240),
        // The whole turn closes (G173): the loop keeps its seam's point once, a vertex fewer.
        (
            &cylinder,
            &ellipse,
            1e-2,
            34_293,
            165,
            4_576_905_993_249_919_340,
        ),
        (
            &torus,
            &outline,
            1e-2,
            244_468,
            292,
            4_576_913_829_044_423_827,
        ),
        (&edge, &edged, 1e-2, 9_230, 0, 0),
        (&band, &dip, 8.0, 336, 4, 4_616_454_535_679_897_858),
        (&wall, &exact, 1e-2, 192, 2, 80),
    ];
    for (surface, level, ceiling, work, vertices, reach) in cases {
        let run = |budget: u64| {
            let mut m = WorkMeter::new(Budget::of_work(budget));
            trace(surface, level, 1e-4, ceiling, &mut m)
        };
        assert_eq!(run(work - 1), Err(IntersectError::BudgetExceeded));
        let found = run(work).unwrap();
        let total: usize = found.branches.iter().map(|b| b.vertices.len()).sum();
        let most = found
            .branches
            .iter()
            .fold(0.0f64, |m, b| m.max(b.deviation));
        assert_eq!((total, most.to_bits()), (vertices, reach));
    }
}
