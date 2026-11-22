//! Three edges held to their true answers: a sphere touching a plane off its equator, where the
//! contact's solve weighs its unknowns; a crossing curve given backwards whose two vertex
//! enclosures touch at one end; and a torus strip whose nearest points to its tube's centre span
//! exactly twice the floor. Each pins the answer and the work it charges.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::contact::{Reading, analyse};
use geom_intersect::general::subdivide;
use geom_intersect::known::{Known, KnownContact, subdivide_known};
use geom_intersect::result::PointContact;
use geom_intersect::{Projection, project_surface};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, Sphere, SurfaceKind, Torus};

fn point(x: f64) -> Interval {
    Interval::point(x).unwrap()
}

/// The sphere of radius 5 about the origin, u around z and v its latitude, both in half turns,
/// against the plane through (0, 4, 3) along (5, 0, 0) and (0, 3, −4), over a square of half side
/// 4 a little off its origin so that no cut falls on the contact. (0, 4, 3) lies on the sphere and
/// the plane's directions are both orthogonal to it, so the plane is tangent there and nowhere
/// else meets the sphere: the one answer is an isolated tangent point at u = 1/2, sin(πv) = 3/5,
/// cos(πv) = 4/5 on the sphere and (0, 0) on the plane, with no gap. Off the equator the sphere's
/// second derivatives have parts along the plane, so the contact's solve couples its unknowns and
/// reads their weights; with every weight zero it cannot prove the root and leaves it undecided.
#[test]
fn a_sphere_touching_a_plane_off_its_equator_is_one_tangent_point() {
    let r = 4.0;
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.0, 4.0, 3.0],
            [5.0, 0.0, 0.0],
            [0.0, 3.0, -4.0],
            ((-r + 0.0123, r + 0.0123), (-r + 0.0311, r + 0.0311)),
        )
        .unwrap(),
    );
    let ball = SurfaceKind::Sphere(
        Sphere::new(
            [0.0; 3],
            5.0,
            Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    );
    let floor = 2.0;
    let found = subdivide(
        &ball,
        &plane,
        floor,
        &mut WorkMeter::new(Budget::of_work(1 << 25)),
    )
    .unwrap();
    let mut meter = WorkMeter::new(Budget::of_work(1 << 25));
    let read = analyse(&ball, &plane, &found, floor, floor, &mut meter).unwrap();
    let [Reading::Point { at, gap, contact }] = read.as_slice() else {
        panic!("{read:?}");
    };
    assert_eq!(*contact, PointContact::IsolatedTangent);
    let five = point(5.0);
    assert!(at[0].contains(0.5), "{at:?}");
    assert!(
        five.mul(at[1].sinpi().unwrap()).unwrap().contains(3.0),
        "{at:?}"
    );
    assert!(
        five.mul(at[1].cospi().unwrap()).unwrap().contains(4.0),
        "{at:?}"
    );
    assert!(at[2].contains(0.0) && at[3].contains(0.0), "{at:?}");
    assert!(gap.contains(0.0), "{gap:?}");
    assert_eq!(meter.used().work, 7_993);
}

/// A sheet over x, y in [−1, 1], its height a quadratic in y alone, which crosses z = 0 along a
/// line of constant y.
fn crossing_sheet() -> SurfaceKind {
    let (k, r) = (0.75, 15.0 / 16.0);
    let (b0, b2) = (k * r / 2.0, k * (1.0 - r) / 2.0);
    let heights = [b0, -(b0 + b2) / 2.0, b2];
    let across = Basis::new(2, vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]).unwrap();
    let along = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for x in [-1.0, 1.0] {
        for (j, z) in heights.iter().enumerate() {
            net.push([x, -1.0 + f64::from(u8::try_from(j).unwrap()), *z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap())
}

/// The plane z = 0.
fn ground() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        )
        .unwrap(),
    )
}

/// What the known search settles and flags, and the work it charges.
fn run(vertices: Vec<[Interval; 4]>) -> (usize, usize, u128) {
    let mut meter = WorkMeter::new(Budget::of_work(1 << 24));
    let known = Known {
        vertices,
        bound: 0.0,
        contact: KnownContact::Crossing,
    };
    let found = subdivide_known(
        &ground(),
        &crossing_sheet(),
        &[known],
        1.0 / 128.0,
        &mut meter,
    )
    .unwrap();
    (found.settled.len(), found.flagged.len(), meter.used().work)
}

/// The ground against the crossing sheet, with a known crossing curve of two vertices given in
/// decreasing order: u in [1/8, 3/8], then u in [−1/8, 1/8], each at v = 0 on both. The plane's x
/// is its u exactly, so the two enclosures meet at x = 1/8: nothing lies strictly between them,
/// no slab is proven, and the search settles the line as it would with no known curve, leaving
/// 10 settled and 4 flagged. A slab taken between enclosures that touch would cut a cell of no
/// width and leave the search many more boxes.
#[test]
fn a_backward_crossing_stretch_whose_ends_touch_gets_no_cell() {
    let touching: Vec<[Interval; 4]> = [(0.125, 0.375), (-0.125, 0.125)]
        .iter()
        .map(|&(lo, hi)| {
            let u = Interval::new(lo, hi).unwrap();
            [u, point(0.0), u, point(0.0)]
        })
        .collect();
    assert_eq!(
        [run(touching), run(Vec::new())],
        [(10, 4, 80_685), (10, 4, 80_454)]
    );
}

/// The widest of a box's three coordinates.
fn image(x: &[Interval; 3]) -> f64 {
    x.iter()
        .map(|i| i.hi() - i.lo())
        .fold(0.0, |most, w| if w > most { w } else { most })
}

/// The torus of radii 6 and 2 about z, over its whole u and the strip v in [−10⁻⁵, 10⁻⁵] half
/// turns about its outer equator, projected from (0, 6, 0), the centre of its tube at u = 1/2.
/// Every point of that tube's circle is at distance 2, so the nearest points are the arc of it
/// over the strip, 2·2π·10⁻⁵ long. The floor is half the hull's image of what the search leaves
/// undecided there, so that image is exactly twice the floor: within twice the floor, the hull is
/// one nearest point by the module's rule, a foot over the whole strip at distance 2.
#[test]
fn nearest_points_spanning_twice_the_floor_are_one_point() {
    let w = 1e-5;
    let ring = SurfaceKind::Torus(
        Torus::new(
            [0.0; 3],
            6.0,
            2.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-w, w)),
        )
        .unwrap(),
    );
    let floor = 6.283_185_306_146_048e-5;
    let mut meter = WorkMeter::new(Budget::of_work(1 << 22));
    let found = project_surface(&ring, &[0.0, 6.0, 0.0], floor, &mut meter).unwrap();
    let Projection::Points(points) = &found else {
        panic!("{found:?}");
    };
    let [foot] = points.as_slice() else {
        panic!("{found:?}");
    };
    assert_eq!(image(&foot.point), 2.0 * floor);
    assert!(foot.at[0].contains(0.5), "{foot:?}");
    assert_eq!((foot.at[1].lo(), foot.at[1].hi()), (-w, w));
    assert!(foot.distance.contains(2.0), "{foot:?}");
}
