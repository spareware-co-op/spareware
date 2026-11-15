//! Inputs for `geom-intersect`'s hot paths (pieces G158 and G162 to G176, decision 32.46): a list of
//! items put in canonical order, a circle's canonical key, a pair of lines put in canonical order, a
//! point projected on an arc and on a band of a sphere, pairs of curves that meet, each path of 12.5
//! once, curves through surfaces, each path of 12.6 once, natural quadrics, four rows of 12.7, a
//! sphere and a cylinder in general position, whose curve is walked, three reductions of 12.7,
//! surfaces cut by a sphere and seen in silhouette, planes through a whole sphere's poles, sheets
//! tangent to a plane along a known line (G174), marched crossings to fit as intersection curves
//! (G175), swept loops crossing themselves (G176) and a sphere resting on a plane (G171).
//!
//! Class `typical` only: each list is [`ITEMS`] disjoint points in a shuffled order, each circle,
//! line, sphere and point within ±2^20 grid steps of the origin, so every call succeeds. Refusals are tested in
//! `geom-intersect` itself. The general searches, traces and marches are sized so that a batch of
//! 64 runs inside the count gates' jobs, each still on the path it measures: the spheres projected
//! on are the others divided by 2^8, the torus seen in silhouette is a band of it each view crosses
//! once, the swept loops reach an eighth of a unit each way, the known line is cut in four, and
//! the kernels take a band of a sphere and coarse floors (`kernels::intersect`). Three calls still
//! take hundreds of millions of instructions each, so their batches hold only a few inputs of the
//! 64 drawn, chosen to run all the code the 64 run ([`SWEEPS`], [`APEXES`] and [`POLES`];
//! `inputs::SHORT_BATCHES`).
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the call's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the calls of `geom-intersect`'s attack tests (`tests/attack_g158.rs`), under an
//! unlimited budget. Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few draws of [`SplitMix64`] per input, outside every measured run; [`crossings`] also
//! marches each of its six sheets once.

use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_intersect::general::subdivide;
use geom_intersect::known::{Known, KnownContact};
use geom_intersect::march::{Branch, march};
use geom_intersect::{Item, ParamBox, PointContact};
use geom_math::Box3;
use geom_num::Exactness;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Curve, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Extrusion, Plane, Sphere, SurfaceKind, Torus};

use super::{Class, batch_of, keep, p3, rng, within};
use crate::rng::SplitMix64;

/// The reach of an input's coordinates, in grid steps.
const REACH: i64 = 1 << 20;

/// How many items a list holds.
const ITEMS: u32 = 32;

/// A coordinate within the reach. It fits `i32`, so it converts exactly.
fn coordinate(g: &mut SplitMix64) -> f64 {
    f64::from(i32::try_from(within(g, REACH)).unwrap_or(0))
}

/// A point within the reach.
fn point(g: &mut SplitMix64) -> [f64; 3] {
    [coordinate(g), coordinate(g), coordinate(g)]
}

/// `places` in a shuffled order, by Fisher and Yates's method.
fn shuffled(g: &mut SplitMix64, mut places: Vec<u32>) -> Option<Vec<u32>> {
    for i in (1..places.len()).rev() {
        let j = usize::try_from(g.range(0, i64::try_from(i).ok()?)).ok()?;
        places.swap(i, j);
    }
    Some(places)
}

/// The point item at `place`.
fn item(place: u32, xyz: Box3) -> Option<Item> {
    let start = f64::from(place);
    let spans = [
        Interval::new(start, start + 0.5).ok()?,
        Interval::new(0.0, 1.0).ok()?,
    ];
    Some(Item::Point {
        at: ParamBox::new(&spans).ok()?,
        xyz,
        contact: PointContact::Crossing,
        class: Exactness::Exact,
    })
}

/// Lists of [`ITEMS`] disjoint points, shuffled, for `Intersection::new`.
#[must_use]
pub fn items(class: Class) -> Vec<Vec<Item>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::items", class);
    batch_of(|_| {
        let xyz = Box3::new(p3(0, 0, 0), p3(1, 1, 1)).ok()?;
        let places = shuffled(&mut g, (0..ITEMS).collect())?;
        places.into_iter().map(|p| item(p, xyz)).collect()
    })
}

/// Circles for `CanonicalKey::of_curve`.
#[must_use]
pub fn circles(class: Class) -> Vec<CurveKind<3>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::circles", class);
    batch_of(|_| {
        let centre = point(&mut g);
        let radius = f64::from(i32::try_from(g.range(1, 1 << 16)).ok()?);
        let circle = Circle::full(centre, radius, Frame::standard().ok()?).ok()?;
        Some(CurveKind::Circle(circle))
    })
}

/// Pairs of lines for `pair`.
#[must_use]
pub fn lines(class: Class) -> Vec<(CurveKind<3>, CurveKind<3>)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::lines", class);
    let mut line = move || {
        let run = (
            f64::from(i32::try_from(g.range(1, 1 << 16)).ok()?),
            1.0,
            2.0,
        );
        Line::new(point(&mut g), [run.0, run.1, run.2], (0.0, 1.0))
            .ok()
            .map(CurveKind::Line)
    };
    batch_of(|_| Some((line()?, line()?)))
}

/// A radius of up to 2^16 grid steps.
fn radius(g: &mut SplitMix64) -> Option<f64> {
    i32::try_from(g.range(1, 1 << 16)).ok().map(f64::from)
}

/// Three quarters of a circle and a point to project on it.
#[must_use]
pub fn arcs(class: Class) -> Vec<(CurveKind<3>, [f64; 3])> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::arcs", class);
    batch_of(|_| {
        let arc = Circle::new(
            point(&mut g),
            radius(&mut g)?,
            Frame::standard().ok()?,
            (0.0, 1.5),
        );
        Some((CurveKind::Circle(arc.ok()?), point(&mut g)))
    })
}

/// A band of a sphere and a point to project on it.
#[must_use]
pub fn spheres(class: Class) -> Vec<(SurfaceKind, [f64; 3])> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::spheres", class);
    // The other inputs divided by 2^8 (module documentation).
    let near = |g: &mut SplitMix64| {
        let mut c = || f64::from(i32::try_from(within(g, REACH >> 8)).unwrap_or(0));
        [c(), c(), c()]
    };
    batch_of(|_| {
        let centre = near(&mut g);
        let r = f64::from(i32::try_from(g.range(1, 1 << 8)).ok()?);
        let band = Sphere::new(
            centre,
            r,
            Frame::standard().ok()?,
            ((0.0, 1.5), (-0.4, 0.4)),
        );
        Some((SurfaceKind::Sphere(band.ok()?), near(&mut g)))
    })
}

/// A plane patch and the centre of a sphere that cuts it in a closed circle, for `trace`.
#[must_use]
pub fn patches(class: Class) -> Vec<(SurfaceKind, [f64; 3])> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::patches", class);
    batch_of(|_| {
        let at = point(&mut g);
        let patch = Plane::new(
            [at[0], at[1], 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        );
        let lift = f64::from(i32::try_from(g.range(1, 4)).ok()?);
        Some((SurfaceKind::Plane(patch.ok()?), [at[0], at[1], lift]))
    })
}

/// A plane patch and the centre of a sphere that cuts it in a closed circle, for `subdivide`.
#[must_use]
pub fn sheets(class: Class) -> Vec<(SurfaceKind, [f64; 3])> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::sheets", class);
    batch_of(|_| {
        let at = point(&mut g);
        let patch = Plane::new(
            [at[0], at[1], 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        );
        let lift = f64::from(i32::try_from(g.range(1, 4)).ok()?);
        Some((SurfaceKind::Plane(patch.ok()?), [at[0], at[1], lift]))
    })
}

/// A plane patch and the centre of a sphere that cuts it in a closed circle, for `march`.
#[must_use]
pub fn plates(class: Class) -> Vec<(SurfaceKind, [f64; 3])> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::plates", class);
    batch_of(|_| {
        let at = point(&mut g);
        let patch = Plane::new(
            [at[0], at[1], 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        );
        let lift = f64::from(i32::try_from(g.range(1, 4)).ok()?);
        Some((SurfaceKind::Plane(patch.ok()?), [at[0], at[1], lift]))
    })
}

/// The inputs [`poles`] keeps of its batch of [`BATCH`](super::BATCH), by their places in it: each
/// march through both poles takes about 2.6e8 instructions, so the whole batch kept a count gate's
/// job on it for 8 to 11 minutes. The first eight run every function and every source line that
/// the 64 run (`inputs::SHORT_BATCHES`).
pub const POLES: [usize; 8] = [0, 1, 2, 3, 4, 5, 6, 7];

/// A plane patch holding the axis of a whole sphere about each centre, so it passes through both
/// of the sphere's poles, for `march` across them (piece G173). The inputs [`POLES`] names.
#[must_use]
pub fn poles(class: Class) -> Vec<(SurfaceKind, [f64; 3])> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::poles", class);
    let full = batch_of(|_| {
        let at = point(&mut g);
        let patch = Plane::new(
            at,
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        );
        Some((SurfaceKind::Plane(patch.ok()?), at))
    });
    keep(&POLES, full)
}

/// A sheet tangent to the ground z = 0 along the line v = 0, z = k·v²/4 for a dyadic k, and that
/// line as a known curve, for `subdivide_known` (piece G174).
#[must_use]
pub fn tangents(class: Class) -> Vec<(SurfaceKind, Known)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::tangents", class);
    batch_of(|_| {
        let k = f64::from(i32::try_from(g.range(1, 8)).ok()?) / 4.0;
        let across = Basis::new(2, vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]).ok()?;
        let along = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).ok()?;
        let mut net = Vec::new();
        for x in [-1.0, 1.0] {
            for (y, z) in [(-1.0, k / 4.0), (0.0, -k / 4.0), (1.0, k / 4.0)] {
                net.push([x, y, z]);
            }
        }
        let sheet = Surface::new(along, across, &net).ok()?;
        let point = |x: f64| Interval::point(x).ok();
        // Four stretches, half a unit each: every stretch takes the same tries, so fewer of them
        // keep a batch inside the count gates' jobs.
        let mut vertices = Vec::new();
        for step in -2..=2 {
            let u = f64::from(step) / 2.0;
            vertices.push([point(u)?, point(0.0)?, point(u)?, point(0.0)?]);
        }
        let known = Known {
            vertices,
            bound: 0.0,
            contact: KnownContact::Tangent,
        };
        Some((SurfaceKind::Nurbs(sheet), known))
    })
}

/// A sheet z = v − k·u² for a dyadic k, crossing the ground along a parabola, with that crossing
/// marched (outside every measured run), for `fit` (piece G175).
///
/// k is one of six eighths, so the batch holds six sheets, each many times: each is marched once,
/// the first time it is drawn, and copied to every later input that draws it. The march is
/// deterministic, so the batch is the one a march of every input gives, built with six marches in
/// place of [`BATCH`](super::BATCH); the count gates build it inside the measured process, under
/// Valgrind, where 64 marches took about a minute of each job.
#[must_use]
pub fn crossings(class: Class) -> Vec<(SurfaceKind, Branch)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::crossings", class);
    let mut marched: Vec<(i64, (SurfaceKind, Branch))> = Vec::new();
    batch_of(|_| {
        let eighths = g.range(1, 7);
        if let Some((_, input)) = marched.iter().find(|(drawn, _)| *drawn == eighths) {
            return Some(input.clone());
        }
        let input = crossing(eighths)?;
        marched.push((eighths, input.clone()));
        Some(input)
    })
}

/// The sheet z = v − k·u² for k = `eighths`/8 and its crossing of the ground, marched.
fn crossing(eighths: i64) -> Option<(SurfaceKind, Branch)> {
    let ground = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-4.0, 4.0), (-4.0, 4.0)),
        )
        .ok()?,
    );
    let k = f64::from(i32::try_from(eighths).ok()?) / 8.0;
    let along = Basis::new(2, vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]).ok()?;
    let across = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).ok()?;
    let mut net = Vec::new();
    for (x, square) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
        for y in [-1.0, 1.0] {
            net.push([x, y, y - k * square]);
        }
    }
    let sheet = SurfaceKind::Nurbs(Surface::new(along, across, &net).ok()?);
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let found = subdivide(&sheet, &ground, 1.0 / 128.0, &mut meter).ok()?;
    let traced = march(&sheet, &ground, &found, 1.0 / 128.0, 1.0 / 16.0, &mut meter).ok()?;
    let branch = traced.branches.into_iter().next()?;
    Some((sheet, branch))
}

/// The inputs [`sweeps`] keeps of its batch of [`BATCH`](super::BATCH), by their places in it: a
/// self-intersection takes 2.3e8 to 1.9e9 instructions, 6.2e8 on average, so the whole batch kept
/// a count gate's job on it for 21 to 28 minutes. The inputs take different paths: alone, one runs
/// 1,576 to 1,602 of the 1,605 functions and source lines the 64 run, and the first four together
/// miss three of `geom-intersect`'s functions and 19 lines (among them a pair with a start that
/// cannot be traced, handed back unproven, and a walk that ends at a start among its box's own
/// points). Input 45 runs all but three of the lines, which input 4 runs, so the batch is those
/// two (`inputs::SHORT_BATCHES`).
pub const SWEEPS: [usize; 2] = [4, 45];

/// A sheet whose section at y = v is a cubic arch that loops, crossing itself along a line: the
/// arch with control points (−a, 0), (b, h), (−b, h), (a, 0) in x and z, a, b and h dyadic and
/// b > a in both end rows, for `self_intersect` (piece G176). The inputs [`SWEEPS`] names.
#[must_use]
pub fn sweeps(class: Class) -> Vec<SurfaceKind> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::sweeps", class);
    let full = batch_of(|_| {
        let mut eighths = |lo: i64, hi: i64| -> Option<f64> {
            Some(f64::from(i32::try_from(g.range(lo, hi)).ok()?) / 8.0)
        };
        let mut rows = Vec::new();
        for _ in 0..2 {
            let a = eighths(8, 12)?;
            let b = a + eighths(2, 5)?;
            let h = eighths(12, 20)?;
            rows.push([[-a, 0.0], [b, h], [-b, h], [a, 0.0]]);
        }
        let across = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).ok()?;
        let along = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).ok()?;
        // A sweep an eighth of a unit each way: the crossing is as long as the sweep, so a short one
        // keeps a batch inside the count gates' jobs.
        let mut net = Vec::new();
        for i in 0..4 {
            for (y, row) in [-0.125, 0.125].into_iter().zip(&rows) {
                let [x, z] = *row.get(i)?;
                net.push([x, y, z]);
            }
        }
        Some(SurfaceKind::Nurbs(Surface::new(across, along, &net).ok()?))
    });
    keep(&SWEEPS, full)
}

/// A plane patch and a sphere resting on it, for `analyse`.
#[must_use]
pub fn rests(class: Class) -> Vec<(SurfaceKind, SurfaceKind)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::rests", class);
    batch_of(|_| {
        let at = point(&mut g);
        let patch = Plane::new(
            [at[0], at[1], 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        );
        let radius = f64::from(i32::try_from(g.range(1, 4)).ok()?);
        // Its axis lies along the plane, so the contact is on its equator and far from a pole.
        let ball = Sphere::new(
            [at[0], at[1], radius],
            radius,
            Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).ok()?,
            ((0.0625, 2.0), (-0.49, 0.49)),
        );
        Some((
            SurfaceKind::Plane(patch.ok()?),
            SurfaceKind::Sphere(ball.ok()?),
        ))
    })
}

/// Most of a torus and a view direction, for a silhouette.
#[must_use]
pub fn tori(class: Class) -> Vec<(SurfaceKind, [f64; 3])> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("intersect::tori", class);
    batch_of(|_| {
        // A sixth of a turn round the axis and four tenths round the tube, which every view of the
        // batch crosses once: one silhouette branch, from edge to edge, as on most of the torus.
        let band = Torus::new(
            point(&mut g),
            8.0,
            2.0,
            Frame::standard().ok()?,
            ((0.1, 0.4), (0.1, 0.9)),
        );
        let run = f64::from(i32::try_from(g.range(1, 4)).ok()?);
        Some((SurfaceKind::Torus(band.ok()?), [run, 1.0, 2.0]))
    })
}

/// A coordinate of a 2D curve.
fn plain(g: &mut SplitMix64) -> f64 {
    coordinate(g)
}

/// A batch of `build`'s values, for the typical class alone.
fn typical<T>(
    class: Class,
    name: &str,
    mut build: impl FnMut(&mut SplitMix64) -> Option<T>,
) -> Vec<T> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(name, class);
    batch_of(|_| build(&mut g))
}

/// The segment across `at`, its ends `run` either way.
fn across(at: [f64; 2], [x, y]: [f64; 2]) -> Option<CurveKind<2>> {
    let [a, b] = at;
    let line = Line::segment([a - x, b - y], [a + x, b + y]).ok()?;
    Some(CurveKind::Line(line.value))
}

/// A run of at least one.
fn run(g: &mut SplitMix64) -> [f64; 2] {
    [plain(g) + f64::from(1 << 21), plain(g)]
}

/// Two segments through one point.
#[must_use]
pub fn segment_pairs(class: Class) -> Vec<(CurveKind<2>, CurveKind<2>)> {
    typical(class, "intersect::segment_pairs", |g| {
        let at = [plain(g), plain(g)];
        let [x, y] = run(g);
        Some((across(at, [x, y])?, across(at, [y, -x])?))
    })
}

/// A full circle.
fn circle(at: [f64; 2], radius: f64) -> Option<CurveKind<2>> {
    let frame = Frame::standard().ok()?;
    Circle::full(at, radius, frame).ok().map(CurveKind::Circle)
}

/// Two circles of very different sizes, the second centred on the first.
#[must_use]
pub fn circle_pairs(class: Class) -> Vec<(CurveKind<2>, CurveKind<2>)> {
    typical(class, "intersect::circle_pairs", |g| {
        let at = [plain(g), plain(g)];
        let big = f64::from(1 << 22) + plain(g).abs();
        let small = 1.0 + plain(g).abs() / 8.0;
        Some((circle(at, big)?, circle([at[0] + big, at[1]], small)?))
    })
}

/// A segment through a circle's centre, and the circle.
#[must_use]
pub fn line_circles(class: Class) -> Vec<(CurveKind<2>, CurveKind<2>)> {
    typical(class, "intersect::line_circles", |g| {
        let at = [plain(g), plain(g)];
        let radius = 1.0 + plain(g).abs();
        Some((across(at, run(g))?, circle(at, radius)?))
    })
}

/// A cubic Bézier curve with these points.
fn cubic(points: &[[f64; 2]; 4]) -> Option<CurveKind<2>> {
    let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    let basis = Basis::new(3, knots).ok()?;
    Curve::new(basis, points).ok().map(CurveKind::Nurbs)
}

/// Two cubics across the same span, one rising through the other.
#[must_use]
pub fn cubic_pairs(class: Class) -> Vec<(CurveKind<2>, CurveKind<2>)> {
    typical(class, "intersect::cubic_pairs", |g| {
        let w = f64::from(1 << 12);
        let mut height = || plain(g) / 256.0;
        let (p, q) = ([height(), height()], [height(), height()]);
        let [a, b] = p;
        let [c, e] = q;
        let first = cubic(&[[0.0, -w], [w, a], [2.0 * w, b], [3.0 * w, w]])?;
        let second = cubic(&[[0.0, w], [w, c], [2.0 * w, e], [3.0 * w, -w]])?;
        Some((first, second))
    })
}

/// A cubic with a loop, its size drawn.
#[must_use]
pub fn loops(class: Class) -> Vec<CurveKind<2>> {
    typical(class, "intersect::loops", |g| {
        let s = 1024.0 + plain(g).abs() / 256.0;
        cubic(&[
            [0.0, 0.0],
            [3.0 * s, 2.0 * s],
            [-2.0 * s, 2.0 * s],
            [s, 0.0],
        ])
    })
}

/// A line through a point of a plane.
#[must_use]
pub fn line_planes(class: Class) -> Vec<(CurveKind<3>, SurfaceKind)> {
    typical(class, "intersect::line_planes", |g| {
        let [a, b, c] = point(g);
        let [x, y, z] = point(g);
        let side = (-f64::from(1 << 22), f64::from(1 << 22));
        let plane = Plane::new([a, b, c], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], (side, side)).ok()?;
        let line = Line::new(
            [a + x, b + y, c + z.abs() + 1.0],
            [0.0, 0.0, -1.0],
            (0.0, f64::from(1 << 23)),
        )
        .ok()?;
        Some((CurveKind::Line(line), SurfaceKind::Plane(plane)))
    })
}

/// A line along a torus's equator, through its tube.
#[must_use]
pub fn line_tori(class: Class) -> Vec<(CurveKind<3>, SurfaceKind)> {
    typical(class, "intersect::line_tori", |g| {
        let [a, b, c] = point(g);
        let major = 1024.0 + plain(g).abs() / 8.0;
        let minor = 1.0 + plain(g).abs() / 2048.0;
        let turn = (0.0, 2.0);
        let frame = Frame::standard().ok()?;
        let torus = Torus::new([a, b, c], major, minor, frame, (turn, turn)).ok()?;
        let line = Line::new([a - 2.0 * major, b, c], [1.0, 0.0, 0.0], (0.0, 4.0 * major)).ok()?;
        Some((CurveKind::Line(line), SurfaceKind::Torus(torus)))
    })
}

/// A cubic down through a biquadratic graph.
#[must_use]
pub fn cubic_patches(class: Class) -> Vec<(CurveKind<3>, SurfaceKind)> {
    typical(class, "intersect::cubic_patches", |g| {
        let w = f64::from(1 << 12);
        let mut height = || plain(g) / 2048.0;
        let mut net = Vec::with_capacity(9);
        for i in 0..3_i32 {
            for j in 0..3_i32 {
                net.push([f64::from(i) * w, f64::from(j) * w, height()]);
            }
        }
        let knots = || vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let surface = Surface::new(
            Basis::new(2, knots()).ok()?,
            Basis::new(2, knots()).ok()?,
            &net,
        )
        .ok()?;
        let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let points = [
            [0.25 * w, 0.5 * w, 4.0 * w],
            [0.75 * w, 0.75 * w, 2.0 * w],
            [1.25 * w, 1.25 * w, -2.0 * w],
            [1.75 * w, 1.5 * w, -4.0 * w],
        ];
        let curve = Curve::new(Basis::new(3, knots).ok()?, &points).ok()?;
        Some((CurveKind::Nurbs(curve), SurfaceKind::Nurbs(surface)))
    })
}

/// How far a tall surface reaches along its axis, and a plane each way.
const TALL: (f64, f64) = (-16_777_216.0, 16_777_216.0);

/// A whole turn, in half-turns.
const TURN: (f64, f64) = (0.0, 2.0);

/// A plane at 45 degrees across a cylinder, which cuts an ellipse.
#[must_use]
pub fn plane_cylinders(class: Class) -> Vec<(SurfaceKind, SurfaceKind)> {
    typical(class, "intersect::plane_cylinders", |g| {
        let at = point(g);
        let radius = 1024.0 + plain(g).abs() / 8.0;
        let frame = Frame::standard().ok()?;
        let tube = Cylinder::new(at, radius, frame, (TURN, TALL)).ok()?;
        let sheet = Plane::new(at, [1.0, 0.0, 1.0], [0.0, 1.0, 0.0], (TALL, TALL)).ok()?;
        Some((SurfaceKind::Plane(sheet), SurfaceKind::Cylinder(tube)))
    })
}

/// Two spheres, each through the other's centre, which meet in a circle.
#[must_use]
pub fn sphere_pairs(class: Class) -> Vec<(SurfaceKind, SurfaceKind)> {
    typical(class, "intersect::sphere_pairs", |g| {
        let [a, b, c] = point(g);
        let radius = 1024.0 + plain(g).abs() / 8.0;
        let band = (-0.5, 0.5);
        let first = Sphere::new([a, b, c], radius, Frame::standard().ok()?, (TURN, band)).ok()?;
        let second = Sphere::new(
            [a + radius, b, c],
            radius,
            Frame::standard().ok()?,
            (TURN, band),
        )
        .ok()?;
        Some((SurfaceKind::Sphere(first), SurfaceKind::Sphere(second)))
    })
}

/// A pipe tee: two equal cylinders whose axes cross, which meet in two ellipses.
#[must_use]
pub fn tees(class: Class) -> Vec<(SurfaceKind, SurfaceKind)> {
    typical(class, "intersect::tees", |g| {
        let at = point(g);
        // A whole radius: the exact test that the axes' sphere touches both reads it.
        let radius = f64::from(1024 + i32::try_from(g.range(0, 65_536)).ok()?);
        let across = Frame::new([0.0, 1.0, 0.0], [1.0, 0.0, 0.0]).ok()?;
        let main = Cylinder::new(at, radius, Frame::standard().ok()?, (TURN, TALL)).ok()?;
        let branch = Cylinder::new(at, radius, across, (TURN, TALL)).ok()?;
        Some((SurfaceKind::Cylinder(main), SurfaceKind::Cylinder(branch)))
    })
}

/// How far each cone of [`apexes`] reaches along its axis either way: each ruling is cut where it
/// leaves the other cone's domain, so a shorter reach than [`TALL`] keeps a batch inside the count
/// gates' jobs.
const NAPPE: (f64, f64) = (-1024.0, 1024.0);

/// The inputs [`apexes`] keeps of its batch of [`BATCH`](super::BATCH), by their places in it: each
/// pair of cones takes about 4.6e8 instructions, so the whole batch kept a count gate's job on it
/// for 15 to 21 minutes. Every input is the same pair of cones about another apex and runs the same
/// functions, but only inputs 8 and 13 fail the first rounding test of `geom-num`'s `atan2` and
/// run its second fast path, which the first four never reach. So the batch is the first three
/// inputs and input 8, which together run every function and every source line that the 64 run
/// (`inputs::SHORT_BATCHES`).
pub const APEXES: [usize; 4] = [0, 1, 2, 8];

/// Two cones through one apex, on crossing axes, which meet in rulings. The inputs [`APEXES`]
/// names.
#[must_use]
pub fn apexes(class: Class) -> Vec<(SurfaceKind, SurfaceKind)> {
    let full = typical(class, "intersect::apexes", |g| {
        let at = point(g);
        let across = Frame::new([0.0, 1.0, 0.0], [1.0, 0.0, 0.0]).ok()?;
        let first = Cone::new(at, 0.0, 2.0, Frame::standard().ok()?, (TURN, NAPPE)).ok()?;
        let second = Cone::new(at, 0.0, 2.0, across, (TURN, NAPPE)).ok()?;
        Some((SurfaceKind::Cone(first), SurfaceKind::Cone(second)))
    });
    keep(&APEXES, full)
}

/// A cylinder through a sphere, off its centre: one curve or two, walked.
#[must_use]
pub fn walks(class: Class) -> Vec<(SurfaceKind, SurfaceKind)> {
    typical(class, "intersect::walks", |g| {
        let [a, b, c] = point(g);
        let radius = 1024.0 + plain(g).abs() / 8.0;
        let band = (-0.5, 0.5);
        let sphere = Sphere::new([a, b, c], radius, Frame::standard().ok()?, (TURN, band)).ok()?;
        let tube = Cylinder::new(
            [a + radius * 0.25, b, c],
            radius * 0.5,
            Frame::standard().ok()?,
            (TURN, TALL),
        )
        .ok()?;
        Some((SurfaceKind::Sphere(sphere), SurfaceKind::Cylinder(tube)))
    })
}

/// A torus about z.
fn ring_about(at: [f64; 3], major: f64, minor: f64) -> Option<SurfaceKind> {
    let frame = Frame::standard().ok()?;
    let torus = Torus::new(at, major, minor, frame, (TURN, TURN)).ok()?;
    Some(SurfaceKind::Torus(torus))
}

/// A plane square to a torus's axis, through its tube.
#[must_use]
pub fn torus_cuts(class: Class) -> Vec<(SurfaceKind, SurfaceKind)> {
    typical(class, "intersect::torus_cuts", |g| {
        let [a, b, c] = point(g);
        let minor = 1024.0 + plain(g).abs() / 8.0;
        let sheet = Plane::new(
            [a, b, c + minor * 0.5],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            (TALL, TALL),
        )
        .ok()?;
        Some((
            SurfaceKind::Plane(sheet),
            ring_about([a, b, c], minor * 3.0, minor)?,
        ))
    })
}

/// A cylinder about a torus's axis, through its tube.
#[must_use]
pub fn torus_tubes(class: Class) -> Vec<(SurfaceKind, SurfaceKind)> {
    typical(class, "intersect::torus_tubes", |g| {
        let [a, b, c] = point(g);
        let minor = 1024.0 + plain(g).abs() / 8.0;
        let tube = Cylinder::new(
            [a, b, c],
            minor * 3.5,
            Frame::standard().ok()?,
            (TURN, TALL),
        )
        .ok()?;
        Some((
            SurfaceKind::Cylinder(tube),
            ring_about([a, b, c], minor * 3.0, minor)?,
        ))
    })
}

/// A sloping plane across a wall: a segment pushed along z.
#[must_use]
pub fn wall_cuts(class: Class) -> Vec<(SurfaceKind, SurfaceKind)> {
    typical(class, "intersect::wall_cuts", |g| {
        let [a, b, c] = point(g);
        let run = 1024.0 + plain(g).abs() / 8.0;
        let line = Line::segment([a, b, c], [a + run, b, c]).ok()?.value;
        let (lo, hi) = geom_curve::ParamCurve::domain(&line);
        let profile = geom_curve::Trimmed::new(geom_curve::Basis::Line(line), (lo, hi)).ok()?;
        let wall = Extrusion::new(profile, [0.0, 0.0, 1.0], (0.0, run)).ok()?;
        let sheet = Plane::new(
            [a, b, c + run * 0.25],
            [1.0, 0.0, 0.125],
            [0.0, 1.0, 0.0],
            (TALL, TALL),
        )
        .ok()?;
        Some((SurfaceKind::Plane(sheet), SurfaceKind::Extrusion(wall)))
    })
}
