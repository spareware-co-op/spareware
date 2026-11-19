//! Known curves through every branch of the tube's proof (G174, chapter 12.11): a tangent curve
//! running along a surface's second parameter, either way; one slanting across both parameters;
//! tangent tubes carried by a cone's and a sphere's equations; a crossing curve run backwards; a
//! crossing curve that stops short of the surfaces' edges; and vertex enclosures that touch, which
//! do not run strictly one way. Each pair pins what the search leaves and the work it charges, so
//! a cell built wider or narrower than its proof, or along the wrong parameter, shows.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::known::{Known, KnownContact, subdivide_known};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind};

const FLOOR: f64 = 1.0 / 128.0;

fn point(x: f64) -> Interval {
    Interval::point(x).unwrap()
}

/// z = 0 over a patch holding every surface here.
fn ground() -> SurfaceKind {
    tilted([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
}

/// z = 0 again, as o + u·`a` + v·`b`.
fn tilted(a: [f64; 3], b: [f64; 3]) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new([0.0; 3], a, b, ((-8.0, 8.0), (-8.0, 8.0))).unwrap())
}

/// The cylinder of radius 1/2 about the line y = 0, z = 1/2, along x from −1 to 1: u = 0 points
/// up, so it rests on z = 0 at u = 1, v = x.
fn roller() -> SurfaceKind {
    SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0, 0.0, 0.5],
            0.5,
            Frame::new([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]).unwrap(),
            ((0.0, 2.0), (-1.0, 1.0)),
        )
        .unwrap(),
    )
}

/// A sheet over x in [−1, 1] and v in [−1, 1], straight in x, with z the Bézier polynomial of
/// `heights` in v.
fn sheet(heights: &[f64]) -> SurfaceKind {
    let degree = heights.len() - 1;
    let mut knots = vec![-1.0; degree + 1];
    knots.extend(vec![1.0; degree + 1]);
    let across = Basis::new(degree, knots).unwrap();
    let along = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for x in [-1.0, 1.0] {
        for (j, z) in heights.iter().enumerate() {
            let y = -1.0
                + 2.0 * f64::from(u32::try_from(j).unwrap())
                    / f64::from(u32::try_from(degree).unwrap());
            net.push([x, y, *z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap())
}

/// z = K·(t − ½)², t = (v + 1)/2: tangent to the ground along v = 0 and nowhere else.
fn tangent_sheet() -> SurfaceKind {
    let k = 0.75;
    sheet(&[k / 4.0, -k / 4.0, k / 4.0])
}

/// z = K·(t − ½)·(t − 15/16): crossing the ground along v = 0 and again along v = 7/8.
fn crossing_sheet() -> SurfaceKind {
    let (k, r) = (0.75, 15.0 / 16.0);
    let (b0, b2) = (k * r / 2.0, k * (1.0 - r) / 2.0);
    sheet(&[b0, -(b0 + b2) / 2.0, b2])
}

/// The cylinder's line of contact with the ground, (1, x) on it and (x, 0) on the ground, x from
/// −1 to 1 in eighths, the cylinder first.
fn roller_line() -> Vec<[Interval; 4]> {
    (-8..=8)
        .map(|k| {
            let x = f64::from(k) / 8.0;
            [point(1.0), point(x), point(x), point(0.0)]
        })
        .collect()
}

/// The line v = 0 of a sheet on the ground, (u, 0) on both, u = `k`/4 for each `k`, each u held
/// by an enclosure `half` either side of it.
fn sheet_line(ks: core::ops::RangeInclusive<i32>, half: f64) -> Vec<[Interval; 4]> {
    ks.map(|k| {
        let u = f64::from(k) / 4.0;
        let x = Interval::new(u - half, u + half).unwrap();
        [x, point(0.0), x, point(0.0)]
    })
    .collect()
}

fn swapped(vertices: &[[Interval; 4]]) -> Vec<[Interval; 4]> {
    vertices.iter().map(|&[a, b, c, d]| [c, d, a, b]).collect()
}

fn reversed(vertices: &[[Interval; 4]]) -> Vec<[Interval; 4]> {
    vertices.iter().rev().copied().collect()
}

/// What the search leaves, (settled, flagged), and the work it charges.
fn run(
    first: &SurfaceKind,
    second: &SurfaceKind,
    vertices: Vec<[Interval; 4]>,
    contact: KnownContact,
) -> (usize, usize, u128) {
    let mut meter = WorkMeter::new(Budget::of_work(1 << 24));
    let known = Known {
        vertices,
        bound: 0.0,
        contact,
    };
    let found = subdivide_known(first, second, &[known], FLOOR, &mut meter).unwrap();
    (found.settled.len(), found.flagged.len(), meter.used().work)
}

#[test]
fn a_tangent_curve_along_the_second_parameter_is_proven_either_way() {
    let (cylinder, wall) = (roller(), ground());
    let line = roller_line();
    // The ground's equation, carried by the cylinder: the line runs along its v, and the cell
    // is built across its u, at its own speed.
    let up = run(&cylinder, &wall, line.clone(), KnownContact::Tangent);
    let down = run(&cylinder, &wall, reversed(&line), KnownContact::Tangent);
    // The cylinder's equation, carried by the ground: the line runs along its u.
    let flat = run(&wall, &cylinder, swapped(&line), KnownContact::Tangent);
    let flat_down = run(
        &wall,
        &cylinder,
        reversed(&swapped(&line)),
        KnownContact::Tangent,
    );
    // Carried by the cylinder, either way, the whole line is proven and nothing is left; carried
    // by the ground, the same four pairs are flagged either way.
    assert_eq!(
        [up, down, flat, flat_down],
        [(0, 0, 15393), (0, 0, 15393), (0, 4, 21849), (0, 4, 21849)]
    );
}

#[test]
fn enclosures_that_touch_do_not_run_one_way() {
    // Each vertex's enclosure meets the next one's at a point, so no stretch runs strictly one
    // way along the line and no tangent cell is proven: the curve is flagged as known but
    // unproven, whichever way it runs.
    let (sheet, wall) = (tangent_sheet(), ground());
    let line = sheet_line(-4..=4, 1.0 / 8.0);
    let up = run(&sheet, &wall, line.clone(), KnownContact::Tangent);
    let down = run(&sheet, &wall, reversed(&line), KnownContact::Tangent);
    assert_eq!([up, down], [(0, 1522, 1726789), (0, 1522, 1726789)]);
}

#[test]
fn a_crossing_curve_is_proven_whichever_way_it_runs() {
    let (sheet, wall) = (crossing_sheet(), ground());
    let line = sheet_line(-4..=4, 0.0);
    let up = run(&sheet, &wall, line.clone(), KnownContact::Crossing);
    let down = run(&sheet, &wall, reversed(&line), KnownContact::Crossing);
    // Stopping short of the sheet's edges, a quarter in from each: past each end the search
    // settles the line itself.
    let short = sheet_line(-3..=3, 0.0);
    let inner = run(&sheet, &wall, short.clone(), KnownContact::Crossing);
    let inner_down = run(&sheet, &wall, reversed(&short), KnownContact::Crossing);
    assert_eq!(
        [up, down, inner, inner_down],
        [(9, 0, 58814), (9, 0, 58796), (17, 0, 74282), (17, 0, 74192)]
    );
}

#[test]
fn a_slanting_tangent_curve_is_built_along_its_longer_run() {
    let cylinder = roller();
    // The ground with its axes turned an eighth of a turn: the line x runs (x/2, −x/2), as far
    // in u as in v, so the cell is built along u.
    let even = tilted([1.0, 1.0, 0.0], [-1.0, 1.0, 0.0]);
    let line: Vec<[Interval; 4]> = (-8..=8)
        .map(|k| {
            let x = f64::from(k) / 8.0;
            [point(x / 2.0), point(-x / 2.0), point(1.0), point(x)]
        })
        .collect();
    let level = run(&even, &cylinder, line, KnownContact::Tangent);
    // Turned further: x runs (x/5, −2x/5), twice as far in v as in u, so the cell is built
    // along v.
    let steep = tilted([1.0, 2.0, 0.0], [-2.0, 1.0, 0.0]);
    let line: Vec<[Interval; 4]> = (-12..=12)
        .map(|k| {
            let x = f64::from(k) * 5.0 / 64.0;
            [point(x / 5.0), point(-2.0 * x / 5.0), point(1.0), point(x)]
        })
        .collect();
    let along_v = run(&steep, &cylinder, line, KnownContact::Tangent);
    // Each pinned whole: what it leaves flagged and the work it charges.
    assert_eq!([level, along_v], [(0, 7, 51994), (0, 155, 123845)]);
}

#[test]
fn a_cone_and_a_sphere_carry_a_tangent_tube_by_their_own_equations() {
    // A cone about the x axis, radius 1/2 + x/4, u = 0 up, lying along its lowest ruling on the
    // sheet z = −1/2 − x/4, which has no implicit form: only the cone can carry the tube.
    let cone = SurfaceKind::Cone(
        Cone::new(
            [0.0; 3],
            0.5,
            0.25,
            Frame::new([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]).unwrap(),
            ((0.0, 2.0), (-1.0, 1.0)),
        )
        .unwrap(),
    );
    let flat = |b: (f64, f64)| Basis::new(1, vec![b.0, b.0, b.1, b.1]).unwrap();
    let net = [
        [-1.0, -1.0, -0.25],
        [-1.0, 1.0, -0.25],
        [1.0, -1.0, -0.75],
        [1.0, 1.0, -0.75],
    ];
    let slope =
        SurfaceKind::Nurbs(Surface::new(flat((-1.0, 1.0)), flat((-1.0, 1.0)), &net).unwrap());
    let ruling: Vec<[Interval; 4]> = (-8..=8)
        .map(|k| {
            let x = f64::from(k) / 8.0;
            [point(x), point(0.0), point(1.0), point(x)]
        })
        .collect();
    let on_cone = run(&slope, &cone, ruling, KnownContact::Tangent);
    // A sphere of radius 1/2 inside a cylinder of the same radius, touching it along its
    // equator: the tube is carried by the cylinder, read through the sphere's equation.
    let frame = || Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let tube = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 0.5, frame(), ((0.0, 2.0), (-1.0, 1.0))).unwrap(),
    );
    let ball = SurfaceKind::Sphere(
        Sphere::new([0.0; 3], 0.5, frame(), ((0.0, 2.0), (-0.5, 0.5))).unwrap(),
    );
    let equator: Vec<[Interval; 4]> = (0..=16)
        .map(|k| {
            let u = f64::from(k) / 8.0;
            [point(u), point(0.0), point(u), point(0.0)]
        })
        .collect();
    let in_cylinder = run(&tube, &ball, equator, KnownContact::Tangent);
    // Both tubes are proven whole: nothing is left to search.
    assert_eq!([on_cone, in_cylinder], [(0, 0, 24144), (0, 0, 135759)]);
}
