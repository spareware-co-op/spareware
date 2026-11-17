//! A circle lying on a plane, cylinder or cone (issue 974, decision 32.64). `curve_surface` answers
//! it as an overlap by identity from exact signs, a few predicates, where before the engine
//! bisected round the whole circle to the resolution floor and flagged it. Each test here pins one
//! branch of that closed form: the surface's kind, lying on it and not, coaxial and not, every end
//! of a domain either way, and the cases the signs cannot place, which stay the engine's.
//!
//! A closed form's answer is told from the engine's two ways: by what it says (one overlap, proven
//! identical, with no flag) and by its work, held to a count. A circle of radius 5 lying on a
//! surface costs the engine over 20,000 work units; the closed form, under [`FEW`].

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{Circle, CurveKind, Frame};
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::result::{Coincidence, Item};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, SurfaceKind};

const ROUND: (f64, f64) = (0.0, 2.0);
const SIDE: (f64, f64) = (-300.0, 300.0);
const X: [f64; 3] = [1.0, 0.0, 0.0];
const Y: [f64; 3] = [0.0, 1.0, 0.0];
const Z: [f64; 3] = [0.0, 0.0, 1.0];

/// The most work a closed form takes here.
const FEW: u128 = 200;

/// What `curve_surface` says of a pair, reduced to what the tests hold.
#[derive(Debug, PartialEq)]
enum Said {
    /// One overlap by identity and nothing else: its spans in t, u and v.
    Lying([(f64, f64); 3]),
    /// No item at all.
    Nothing,
    /// Items, none of them an overlap.
    Other,
}

fn circle(center: [f64; 3], radius: f64, x: [f64; 3], n: [f64; 3]) -> CurveKind<3> {
    CurveKind::Circle(Circle::new(center, radius, Frame::new(x, n).unwrap(), ROUND).unwrap())
}

fn plane(c: [f64; 3], a: [f64; 3], b: [f64; 3], u: (f64, f64), v: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(c, a, b, (u, v)).unwrap())
}

fn cylinder(center: [f64; 3], radius: f64, x: [f64; 3], n: [f64; 3]) -> Cylinder {
    Cylinder::new(center, radius, Frame::new(x, n).unwrap(), (ROUND, SIDE)).unwrap()
}

fn cone(center: [f64; 3], start: f64, slope: f64, x: [f64; 3], n: [f64; 3]) -> Cone {
    Cone::new(
        center,
        start,
        slope,
        Frame::new(x, n).unwrap(),
        (ROUND, SIDE),
    )
    .unwrap()
}

/// What the pair's answer is, and the work it took.
fn said(curve: &CurveKind<3>, surface: &SurfaceKind) -> (Said, u128) {
    let mut meter = WorkMeter::new(Budget::of_work(50_000_000));
    let found = curve_surface(curve, surface, &mut meter).unwrap();
    let work = meter.used().work;
    let what = match found.items() {
        [] => Said::Nothing,
        [
            Item::Overlap {
                at,
                proof: Coincidence::Identical,
                ..
            },
        ] => {
            let spans = at.spans();
            Said::Lying([0, 1, 2].map(|k| (spans[k].lo(), spans[k].hi())))
        }
        items => {
            assert!(
                items.iter().all(|i| !matches!(i, Item::Overlap { .. })),
                "an overlap among other items: {items:?}"
            );
            Said::Other
        }
    };
    (what, work)
}

/// The pair is an overlap by the closed form: the whole circle, with `u` and `v` in its spans.
fn lies(curve: &CurveKind<3>, surface: &SurfaceKind, u: f64, v: f64) -> [(f64, f64); 3] {
    let (what, work) = said(curve, surface);
    let Said::Lying(spans) = what else {
        panic!("no overlap: {what:?}");
    };
    assert!(work <= FEW, "{work} units");
    assert_eq!(spans[0], ROUND);
    assert!(spans[1].0 <= u && u <= spans[1].1, "u: {spans:?}");
    assert!(spans[2].0 <= v && v <= spans[2].1, "v: {spans:?}");
    spans
}

/// The closed form proves the pair apart: nothing, for a few units.
fn misses(curve: &CurveKind<3>, surface: &SurfaceKind) {
    let (what, work) = said(curve, surface);
    assert_eq!(what, Said::Nothing);
    assert!(work <= FEW, "{work} units");
}

/// The pair is the engine's: no overlap is claimed.
fn engine(curve: &CurveKind<3>, surface: &SurfaceKind) -> Said {
    let (what, _) = said(curve, surface);
    assert!(!matches!(what, Said::Lying(_)), "an overlap: {what:?}");
    what
}

#[test]
fn a_circle_in_a_plane_is_an_overlap() {
    // The plane z = 5 from (3, 4, 5); the circle of radius 5 about (12, 8, 5) is at u in [4, 14]
    // and v in [-1, 9].
    let flat = plane([3.0, 4.0, 5.0], X, Y, SIDE, SIDE);
    let spans = lies(&circle([12.0, 8.0, 5.0], 5.0, X, Z), &flat, 4.0, 9.0);
    assert!(spans[1].0 > 3.0 && spans[1].1 < 15.0 && spans[2].0 > -2.0 && spans[2].1 < 10.0);
    // A plane on no coordinate axis: a = (1, 1, 0) and b = (0, 1, 1), normal (1, -1, 1); the
    // circle about c + a + 2b across that normal, however long the normal is stored.
    let tilted = plane(
        [3.0, 4.0, 5.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        SIDE,
        SIDE,
    );
    lies(
        &circle([4.0, 7.0, 7.0], 2.0, X, [1.0, -1.0, 1.0]),
        &tilted,
        1.0,
        2.0,
    );
    lies(
        &circle([4.0, 7.0, 7.0], 2.0, X, [-3.0, 3.0, -3.0]),
        &tilted,
        1.0,
        2.0,
    );
}

#[test]
fn a_circle_off_a_plane_is_the_engines() {
    let flat = plane([3.0, 4.0, 5.0], X, Y, SIDE, SIDE);
    // Across a normal that leans along a, along b, and a parallel circle a step off the plane
    // either way: each fails one exact sign alone.
    assert_eq!(
        engine(&circle([12.0, 8.0, 5.0], 5.0, Y, [1.0, 0.0, 1.0]), &flat),
        Said::Other
    );
    assert_eq!(
        engine(&circle([12.0, 8.0, 5.0], 5.0, X, [0.0, 1.0, 1.0]), &flat),
        Said::Other
    );
    assert_eq!(
        engine(&circle([12.0, 8.0, 6.0], 5.0, X, Z), &flat),
        Said::Nothing
    );
    assert_eq!(
        engine(&circle([12.0, 8.0, 4.0], 5.0, X, Z), &flat),
        Said::Nothing
    );
    // On the tilted plane, a centre off it along each coordinate in turn.
    let tilted = plane(
        [3.0, 4.0, 5.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        SIDE,
        SIDE,
    );
    for centre in [[5.0, 7.0, 7.0], [4.0, 8.0, 7.0], [4.0, 7.0, 8.0]] {
        engine(&circle(centre, 2.0, X, [1.0, -1.0, 1.0]), &tilted);
    }
}

#[test]
fn a_plane_whose_domain_holds_none_of_the_circle_misses_it() {
    // The circle is at u in [7, 17] and v in [3, 13].
    let ring = circle([12.0, 8.0, 0.0], 5.0, X, Z);
    let origin = [0.0; 3];
    misses(&ring, &plane(origin, X, Y, (100.0, 300.0), SIDE));
    misses(&ring, &plane(origin, X, Y, (-300.0, 0.0), SIDE));
    misses(&ring, &plane(origin, X, Y, SIDE, (100.0, 300.0)));
    misses(&ring, &plane(origin, X, Y, SIDE, (-300.0, 0.0)));
}

#[test]
fn a_circle_on_a_coaxial_cylinder_is_an_overlap() {
    let tube = SurfaceKind::Cylinder(cylinder([1.0, 2.0, 3.0], 5.0, X, Z));
    let spans = lies(&circle([1.0, 2.0, 10.0], 5.0, X, Z), &tube, 1.0, 7.0);
    assert_eq!(spans[1], ROUND);
    assert!(spans[2].1 - spans[2].0 < 1e-9);
    // The circle's own start and the length of its normal change nothing.
    lies(
        &circle([1.0, 2.0, 10.0], 5.0, Y, [0.0, 0.0, -4.0]),
        &tube,
        1.0,
        7.0,
    );
}

#[test]
fn a_circle_off_a_cylinder_is_the_engines() {
    let tube = SurfaceKind::Cylinder(cylinder([1.0, 2.0, 3.0], 5.0, X, Z));
    // A larger and a smaller radius about the axis; a centre off the axis; a normal off it.
    assert_eq!(
        engine(&circle([1.0, 2.0, 10.0], 6.0, X, Z), &tube),
        Said::Nothing
    );
    assert_eq!(
        engine(&circle([1.0, 2.0, 10.0], 4.0, X, Z), &tube),
        Said::Nothing
    );
    assert_eq!(
        engine(&circle([2.0, 2.0, 10.0], 5.0, X, Z), &tube),
        Said::Other
    );
    engine(&circle([1.0, 2.0, 10.0], 5.0, X, [0.0, 1.0, 1.0]), &tube);
    // A cylinder short of a whole turn may hold only part of the circle.
    let part = Cylinder::new(
        [1.0, 2.0, 3.0],
        0.5,
        Frame::new(X, Z).unwrap(),
        ((0.0, 1.5), SIDE),
    );
    engine(
        &circle([1.0, 2.0, 10.0], 0.5, X, Z),
        &SurfaceKind::Cylinder(part.unwrap()),
    );
}

/// A circle at height `h` along `n` over a cylinder about the origin lies on it at `v`, or misses.
fn at_height(tube: &Cylinder, n: [f64; 3], h: f64, v: Option<f64>) {
    let x = if n == X || n == [-1.0, 0.0, 0.0] {
        Y
    } else {
        X
    };
    let ring = circle(n.map(|c| c * h), 5.0, x, n);
    let surface = SurfaceKind::Cylinder(tube.clone());
    match v {
        Some(v) => {
            lies(&ring, &surface, 1.0, v);
        }
        None => misses(&ring, &surface),
    }
}

#[test]
fn a_circle_at_an_end_of_a_round_domain_is_placed_exactly() {
    let (top, past, short) = (300.0_f64, 300.0_f64.next_up(), 300.0_f64.next_down());
    // Along each coordinate axis, either way: on the end, a double past it, a double short of it,
    // and well past it, at both ends. The axial factor is 1, so v is the height.
    for n in [
        X,
        Y,
        Z,
        [-1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, -1.0],
    ] {
        let x = if n == X || n == [-1.0, 0.0, 0.0] {
            Y
        } else {
            X
        };
        let tube = cylinder([0.0; 3], 5.0, x, n);
        for sign in [1.0, -1.0] {
            at_height(&tube, n, sign * top, Some(sign * top));
            at_height(&tube, n, sign * short, Some(sign * short));
            at_height(&tube, n, sign * past, None);
            at_height(&tube, n, sign * 301.0, None);
        }
        // Turned over, v is the height negated: the same heights, at the other end.
        let turned = tube.flipped_axially().unwrap();
        for sign in [1.0, -1.0] {
            at_height(&turned, n, sign * top, Some(-sign * top));
            at_height(&turned, n, sign * short, Some(-sign * short));
            at_height(&turned, n, sign * past, None);
        }
    }
}

#[test]
fn an_axis_off_the_coordinate_axes_is_placed_by_enclosures_alone() {
    // The axis (0.6, 0.8, 0), whose unit vector no doubles hold; the centres are the stored axis
    // times a power of two, so they are on the axis exactly. Inside the domain an enclosure places
    // the circle, and past it too; at an end it cannot, and the pair is the engine's.
    let n = [0.6, 0.8, 0.0];
    let frame = Frame::new(Z, n).unwrap();
    let tube = SurfaceKind::Cylinder(cylinder([0.0; 3], 5.0, Z, n));
    let (what, work) = said(&circle([2.4, 3.2, 0.0], 5.0, Z, n), &tube);
    let Said::Lying([_, _, (lo, hi)]) = what else {
        panic!("no overlap: {what:?}");
    };
    assert!(
        work <= FEW && lo > 3.999 && hi < 4.001,
        "{work} units, v in {lo}..{hi}"
    );
    misses(&circle([307.2, 409.6, 0.0], 5.0, Z, n), &tube);
    // Small circles, so the engine's run round each is short.
    let ending = Cylinder::new([0.0; 3], 0.5, frame, (ROUND, (-300.0, 4.0))).unwrap();
    engine(
        &circle([2.4, 3.2, 0.0], 0.5, Z, n),
        &SurfaceKind::Cylinder(ending),
    );
    let starting = Cylinder::new([0.0; 3], 0.5, frame, (ROUND, (4.0, 300.0))).unwrap();
    engine(
        &circle([2.4, 3.2, 0.0], 0.5, Z, n),
        &SurfaceKind::Cylinder(starting),
    );
}

#[test]
fn a_circle_on_a_coaxial_cone_is_an_overlap() {
    // Radius 1 + 0.75·v at height v over (0, 0, 10): 4 at v = 4, and past the apex 2 at v = -4.
    let funnel = SurfaceKind::Cone(cone([0.0, 0.0, 10.0], 1.0, 0.75, X, Z));
    lies(&circle([0.0, 0.0, 14.0], 4.0, X, Z), &funnel, 1.0, 4.0);
    lies(&circle([0.0, 0.0, 6.0], 2.0, X, Z), &funnel, 1.0, -4.0);
    // Turned over, v is the height negated.
    let turned = cone([0.0, 0.0, 10.0], 1.0, 0.75, X, Z)
        .flipped_axially()
        .unwrap();
    lies(
        &circle([0.0, 0.0, 6.0], 4.0, X, Z),
        &SurfaceKind::Cone(turned),
        1.0,
        4.0,
    );
    // Along x, and against it.
    let along = SurfaceKind::Cone(cone([10.0, 0.0, 0.0], 1.0, 0.75, Y, X));
    lies(&circle([14.0, 0.0, 0.0], 4.0, Y, X), &along, 1.0, 4.0);
    let against = SurfaceKind::Cone(cone([10.0, 0.0, 0.0], 1.0, 0.75, Y, [-1.0, 0.0, 0.0]));
    lies(&circle([6.0, 0.0, 0.0], 4.0, Y, X), &against, 1.0, 4.0);
    // At the end of its domain: radius 226 at v = 300.
    lies(&circle([0.0, 0.0, 310.0], 226.0, X, Z), &funnel, 1.0, 300.0);
}

#[test]
fn a_circle_off_a_cone_is_the_engines() {
    let funnel = SurfaceKind::Cone(cone([0.0, 0.0, 10.0], 1.0, 0.75, X, Z));
    // Another radius at that height, either way; the radius of the other side of the apex; a
    // centre off the axis; a normal off it.
    assert_eq!(
        engine(&circle([0.0, 0.0, 14.0], 5.0, X, Z), &funnel),
        Said::Nothing
    );
    assert_eq!(
        engine(&circle([0.0, 0.0, 14.0], 3.0, X, Z), &funnel),
        Said::Nothing
    );
    assert_eq!(
        engine(&circle([0.0, 0.0, 14.0], 2.0, X, Z), &funnel),
        Said::Nothing
    );
    engine(&circle([1.0, 0.0, 14.0], 4.0, X, Z), &funnel);
    engine(&circle([0.0, 0.0, 14.0], 4.0, X, [0.0, 1.0, 1.0]), &funnel);
    // On an axis off the coordinate axes the height is no sum of products of stored doubles.
    let n = [0.6, 0.8, 0.0];
    // A small circle a quarter below the centre, where the radius is 0.5 - 0.1875.
    let leaning = SurfaceKind::Cone(cone([0.0; 3], 0.5, 0.75, Z, n));
    engine(&circle([-0.15, -0.2, 0.0], 0.3125, Z, n), &leaning);
}
