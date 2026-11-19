//! A circle lying on a plane, cylinder or cone (issue 974, decision 32.64). `curve_surface` answers
//! it as an overlap by identity from exact signs, a few predicates, where before the engine
//! bisected round the whole circle to the resolution floor and flagged it. Each test here pins one
//! branch of that closed form: the surface's kind, lying on it and not, coaxial and not, every end
//! of a domain either way, and the cases the signs cannot place, which stay the engine's. A circle
//! about a line through a sphere's centre that stands off the sphere meets nothing, by one exact
//! sign (decision 32.74); on the sphere, or about another line, it is the engine's. A circle
//! across the axis of a cylinder or cone, its normal parallel to that axis, meets the surface where
//! it meets the circle that the surface cuts in its plane (decision 32.86): nothing past the
//! domain, nothing for two circles apart, two points for two that cross and one where they touch,
//! decided by enclosures and, on a coordinate axis, by exact signs; where neither tells, the engine.
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

use geom_curve::{Axes, Circle, CurveKind, Ellipse, Frame};
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::result::{Coincidence, Item, PointContact};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind};

const ROUND: (f64, f64) = (0.0, 2.0);
const SIDE: (f64, f64) = (-300.0, 300.0);
const X: [f64; 3] = [1.0, 0.0, 0.0];
const Y: [f64; 3] = [0.0, 1.0, 0.0];
const Z: [f64; 3] = [0.0, 0.0, 1.0];

/// The most work a closed form takes here.
const FEW: u128 = 200;

/// The most work a closed form takes here that answers points: it bounds the circle over each
/// point's span for the point's box in space, a few hundred units a point.
const FEW_POINTS: u128 = 1_000;

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

fn sphere(center: [f64; 3], radius: f64) -> SurfaceKind {
    let whole = (ROUND, (-0.5, 0.5));
    SurfaceKind::Sphere(Sphere::new(center, radius, Frame::new(X, Z).unwrap(), whole).unwrap())
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
    // A larger and a smaller radius about the axis; a centre off the axis; a normal off it. None
    // lies on the cylinder; the first three are now decided across the axis (decision 32.86).
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
    // centre off the axis; a normal off it. None lies on the cone; the four across the axis of z
    // are now decided there (decision 32.86).
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

#[test]
fn a_circle_about_a_spheres_centre_line_that_stands_off_it_misses_it() {
    // The stored fuzz case: a circle of radius 1 at 256 along z from the centre of a sphere of
    // radius 256 stands 0.002 outside it all the way round, which cost the engine 782,639 units.
    let ball = sphere([-136.0, 0.0, -44.0], 256.0);
    misses(&circle([-136.0, 0.0, -300.0], 1.0, X, Z), &ball);
    // Inside the sphere, and about its own centre, either way.
    misses(&circle([-136.0, 0.0, -299.0], 1.0, X, Z), &ball);
    misses(&circle([-136.0, 0.0, -44.0], 255.0, X, Z), &ball);
    misses(
        &circle([-136.0, 0.0, -44.0], 257.0, X, [0.0, 0.0, -3.0]),
        &ball,
    );
    // An axis off the coordinate axes: (3, 4, 0) from (1, 2, 3) reaches (4, 6, 3).
    let n = [3.0, 4.0, 0.0];
    misses(
        &circle([4.0, 6.0, 3.0], 2.0, Z, n),
        &sphere([1.0, 2.0, 3.0], 5.0),
    );
}

#[test]
fn a_circle_on_a_sphere_or_about_another_line_is_the_engines() {
    // On the sphere: 4² + 3² = 5², with the centres' dot product not zero.
    let ball = sphere([1.0, 2.0, 3.0], 5.0);
    assert_eq!(
        engine(&circle([1.0, 2.0, 6.0], 4.0, X, Z), &ball),
        Said::Other
    );
    // An axis that misses the centre, through the sphere: the circle crosses it twice.
    assert_eq!(
        engine(&circle([6.0, 2.0, 3.0], 1.0, X, Z), &ball),
        Said::Other
    );
}

/// The pair is not decided by a closed form: no overlap is claimed, and either something is said
/// or nothing is, for more work than a closed form takes.
fn undecided(curve: &CurveKind<3>, surface: &SurfaceKind) {
    let (what, work) = said(curve, surface);
    assert!(!matches!(what, Said::Lying(_)), "an overlap: {what:?}");
    assert!(
        what == Said::Other || work > FEW,
        "nothing, for {work} units"
    );
}

/// A small circle in the plane z = 0.
fn speck() -> CurveKind<3> {
    circle([12.0, 8.0, 0.0], 1.0 / 1024.0, X, Z)
}

/// The plane z = 0 on vectors turned off the coordinate axes, where no exact sign places a part
/// of a circle, over `u` and `v`.
fn turned(u: (f64, f64), v: (f64, f64)) -> SurfaceKind {
    plane([0.0; 3], [0.6, 0.8, 0.0], [-0.8, 0.6, 0.0], u, v)
}

/// The enclosures in u and v of the speck in the turned plane, read from the answer over a
/// domain far wider than it.
fn enclosed() -> [(f64, f64); 2] {
    let (what, _) = said(&speck(), &turned(SIDE, SIDE));
    let Said::Lying([_, u, v]) = what else {
        panic!("no overlap: {what:?}");
    };
    [u, v]
}

#[test]
fn an_enclosure_that_fills_the_planes_domain_exactly_is_wholly_held() {
    // A domain whose four ends are the enclosure's own holds all of it: the same overlap, by the
    // closed form. An end of the domain on an end of the enclosure cuts nothing off.
    let [u, v] = enclosed();
    let (what, work) = said(&speck(), &turned(u, v));
    assert_eq!(what, Said::Lying([ROUND, u, v]));
    assert!(work <= FEW, "{work} units");
}

#[test]
fn an_enclosure_clear_of_the_planes_domain_misses_it_on_each_side() {
    // Each side of the domain in turn, one double clear of the enclosure and no other side near:
    // nothing, by the closed form.
    let [u, v] = enclosed();
    misses(&speck(), &turned((u.1.next_up(), 300.0), SIDE));
    misses(&speck(), &turned((-300.0, u.0.next_down()), SIDE));
    misses(&speck(), &turned(SIDE, (v.1.next_up(), 300.0)));
    misses(&speck(), &turned(SIDE, (-300.0, v.0.next_down())));
}

#[test]
fn an_enclosure_that_reaches_the_planes_domain_is_not_proven_clear_of_it() {
    // Each side of the domain in turn, on the enclosure's own end: the enclosure and the domain
    // share that end, so the enclosure alone does not prove the circle clear.
    let [u, v] = enclosed();
    undecided(&speck(), &turned((u.1, 300.0), SIDE));
    undecided(&speck(), &turned((-300.0, u.0), SIDE));
    undecided(&speck(), &turned(SIDE, (v.1, 300.0)));
    undecided(&speck(), &turned(SIDE, (-300.0, v.0)));
}

#[test]
fn a_round_domain_short_of_a_turn_is_the_engines_wherever_it_starts() {
    // From 1 to 2.5 is short of a whole turn, though its two ends sum to more than one.
    let part = Cylinder::new(
        [1.0, 2.0, 3.0],
        0.5,
        Frame::new(X, Z).unwrap(),
        ((1.0, 2.5), SIDE),
    );
    engine(
        &circle([1.0, 2.0, 10.0], 0.5, X, Z),
        &SurfaceKind::Cylinder(part.unwrap()),
    );
}

#[test]
fn a_height_off_the_coordinate_axes_is_placed_only_where_its_enclosure_is_clear_of_an_end() {
    // The axis (0.6, 0.8, 0) and a small circle about it at height 4, as above. Its enclosure in
    // v is read from the answer over a wide domain.
    let n = [0.6, 0.8, 0.0];
    let frame = Frame::new(Z, n).unwrap();
    let ring = circle([2.4, 3.2, 0.0], 0.5, Z, n);
    let tube = |v: (f64, f64)| {
        SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 0.5, frame, (ROUND, v)).unwrap())
    };
    let (what, _) = said(&ring, &tube(SIDE));
    let Said::Lying([_, _, v]) = what else {
        panic!("no overlap: {what:?}");
    };
    // A domain one double clear of the enclosure, above it and below it: nothing, by the closed
    // form.
    misses(&ring, &tube((v.1.next_up(), 300.0)));
    misses(&ring, &tube((-300.0, v.0.next_down())));
    // A domain that ends on the enclosure's own end: the enclosure does not place the circle
    // against that end, and off the coordinate axes no exact sign does.
    undecided(&ring, &tube((v.1, 300.0)));
    undecided(&ring, &tube((-300.0, v.0)));
}

/// A point of an answer: its spans in t, u and v, and how it stands.
type Spot = ([(f64, f64); 3], PointContact);

/// The points of the pair's answer, which holds nothing else, and the work it took.
fn points_of(curve: &CurveKind<3>, surface: &SurfaceKind) -> (Vec<Spot>, u128) {
    let mut meter = WorkMeter::new(Budget::of_work(50_000_000));
    let found = curve_surface(curve, surface, &mut meter).unwrap();
    let spots = found
        .items()
        .iter()
        .map(|item| match item {
            Item::Point { at, contact, .. } => {
                let s = at.spans();
                ([0, 1, 2].map(|k| (s[k].lo(), s[k].hi())), *contact)
            }
            other => panic!("not a point: {other:?}"),
        })
        .collect();
    (spots, meter.used().work)
}

/// The pair meets by the closed form at one point near each of `expected`, a (t, u, v), standing
/// as `contact`, each span narrow.
fn meets_at(
    curve: &CurveKind<3>,
    surface: &SurfaceKind,
    expected: &[[f64; 3]],
    contact: PointContact,
) {
    let (spots, work) = points_of(curve, surface);
    assert!(work <= FEW_POINTS, "{work} units");
    assert_eq!(spots.len(), expected.len(), "{spots:?}");
    for e in expected {
        let near = |(spans, c): &Spot| {
            *c == contact
                && spans
                    .iter()
                    .zip(e)
                    .all(|((lo, hi), x)| lo - 1e-12 <= *x && *x <= hi + 1e-12 && hi - lo < 1e-6)
        };
        assert!(spots.iter().any(near), "{e:?} not in {spots:?}");
    }
}

/// The circle about `center`, drawn as an ellipse of equal axes on the same frame and domain: the
/// same points at the same parameters, which no closed form takes.
fn as_ellipse(center: [f64; 3], radius: f64, x: [f64; 3], n: [f64; 3]) -> CurveKind<3> {
    let axes = Axes::Frame {
        frame: Frame::new(x, n).unwrap(),
        a: radius,
        b: radius,
    };
    CurveKind::Ellipse(Ellipse::new(center, axes, ROUND).unwrap())
}

/// The closed form's points are the engine's: as many, standing the same, each box meeting one of
/// the engine's, for a few units where the engine takes more.
fn as_the_engine_finds(center: [f64; 3], radius: f64, x: [f64; 3], n: [f64; 3], s: &SurfaceKind) {
    let (ours, few) = points_of(&circle(center, radius, x, n), s);
    let (theirs, more) = points_of(&as_ellipse(center, radius, x, n), s);
    assert!(
        few <= FEW_POINTS && more > FEW_POINTS,
        "{few} and {more} units"
    );
    assert_eq!(ours.len(), theirs.len(), "{ours:?} and {theirs:?}");
    for (spans, contact) in &ours {
        let found = theirs.iter().any(|(other, c)| {
            c == contact
                && spans
                    .iter()
                    .zip(other)
                    .all(|(a, b)| a.0 <= b.1 && b.0 <= a.1)
        });
        assert!(found, "{spans:?} not among {theirs:?}");
    }
}

/// The axis off the coordinate axes; the frames about it take z for x̂.
const LEAN: [f64; 3] = [0.6, 0.8, 0.0];

#[test]
fn a_circle_across_a_round_axis_whose_plane_is_past_the_domain_misses_it() {
    // The cylinder of radius 5 about z over v in [-300, 300], and the cone of radius 1 + 0.75·v
    // about z from (0, 0, 10), over z in [-290, 310]: circles across the axis past either end,
    // however they lie in their planes.
    let tube = SurfaceKind::Cylinder(cylinder([0.0; 3], 5.0, X, Z));
    misses(&circle([1.0, 0.0, 400.0], 50.0, X, Z), &tube);
    misses(&circle([1.0, 0.0, -301.0], 3.0, X, [0.0, 0.0, -1.0]), &tube);
    let funnel = SurfaceKind::Cone(cone([0.0, 0.0, 10.0], 1.0, 0.75, X, Z));
    misses(&circle([0.0, 0.0, 311.0], 3.0, X, Z), &funnel);
    misses(&circle([9.0, 9.0, -291.0], 500.0, Y, Z), &funnel);
    // On the axis off the coordinate axes, by the enclosure of the height.
    let leaning = SurfaceKind::Cylinder(cylinder([0.0; 3], 5.0, Z, LEAN));
    misses(&circle([240.0, 320.0, 0.0], 3.0, Z, LEAN), &leaning);
}

#[test]
fn circles_apart_in_one_plane_meet_nothing() {
    let tube = SurfaceKind::Cylinder(cylinder([0.0; 3], 5.0, X, Z));
    // Beside the cut circle, inside it, round it, and about its centre larger and smaller.
    for (center, radius) in [
        ([20.0, 0.0, 10.0], 3.0),
        ([1.0, 0.0, 10.0], 1.0),
        ([1.0, 0.0, 10.0], 20.0),
        ([0.0, 0.0, 10.0], 6.0),
        ([0.0, 0.0, 10.0], 4.0),
    ] {
        misses(&circle(center, radius, X, Z), &tube);
    }
    // The cone's cut at z = 14 has radius 4, and past the apex at z = 6 radius 2.
    let funnel = SurfaceKind::Cone(cone([0.0, 0.0, 10.0], 1.0, 0.75, X, Z));
    misses(&circle([20.0, 0.0, 14.0], 3.0, X, Z), &funnel);
    misses(&circle([0.5, 0.0, 14.0], 1.0, Y, [0.0, 0.0, -2.0]), &funnel);
    misses(&circle([10.0, 0.0, 6.0], 3.0, X, Z), &funnel);
    // A cylinder along x, and one on the axis off the coordinate axes.
    let along = SurfaceKind::Cylinder(cylinder([0.0; 3], 5.0, Y, X));
    misses(&circle([7.0, 0.0, 20.0], 3.0, Y, X), &along);
    let leaning = SurfaceKind::Cylinder(cylinder([0.0; 3], 5.0, Z, LEAN));
    misses(&circle([2.4, 3.2, 20.0], 3.0, Z, LEAN), &leaning);
    misses(&circle([2.4, 3.2, 1.0], 1.0, Z, LEAN), &leaning);
}

#[test]
fn a_round_domain_or_a_circle_short_of_a_turn_is_the_engines_across_the_axis() {
    // The crossings at a third and two thirds of a turn round the circle, a sixth either side of
    // the join round the axis, all inside domains of 1.9 half-turns from 0.05: each may hold only
    // part of the other, so the pair is the engine's.
    let part = (0.05, 1.95);
    let short = Cylinder::new([0.0; 3], 5.0, Frame::new(X, Z).unwrap(), (part, SIDE)).unwrap();
    let (what, work) = said(
        &circle([5.0, 0.0, 10.0], 5.0, X, Z),
        &SurfaceKind::Cylinder(short),
    );
    assert!(
        what == Said::Other && work > FEW_POINTS,
        "{what:?}, {work} units"
    );
    let arc = CurveKind::Circle(
        Circle::new([5.0, 0.0, 10.0], 5.0, Frame::new(X, Z).unwrap(), part).unwrap(),
    );
    let tube = SurfaceKind::Cylinder(cylinder([0.0; 3], 5.0, X, Z));
    let (what, work) = said(&arc, &tube);
    assert!(
        what == Said::Other && work > FEW_POINTS,
        "{what:?}, {work} units"
    );
}

#[test]
fn circles_crossing_in_one_plane_meet_at_two_points_as_the_engine_finds() {
    // Each circle passes through the centre of the cut circle, of its own radius: the crossings
    // a third and two thirds of a turn round the circle, a sixth either side of the join round
    // the axis.
    let third = 1.0 / 3.0;
    let tube = SurfaceKind::Cylinder(cylinder([0.0; 3], 5.0, X, Z));
    let across = [[2.0 * third, third, 10.0], [4.0 * third, 5.0 * third, 10.0]];
    meets_at(
        &circle([5.0, 0.0, 10.0], 5.0, X, Z),
        &tube,
        &across,
        PointContact::Crossing,
    );
    as_the_engine_finds([5.0, 0.0, 10.0], 5.0, X, Z, &tube);
    // The cone's cut of radius 4 at z = 14, v = 4.
    let funnel = SurfaceKind::Cone(cone([0.0, 0.0, 10.0], 1.0, 0.75, X, Z));
    meets_at(
        &circle([4.0, 0.0, 14.0], 4.0, X, Z),
        &funnel,
        &[[2.0 * third, third, 4.0], [4.0 * third, 5.0 * third, 4.0]],
        PointContact::Crossing,
    );
    as_the_engine_finds([4.0, 0.0, 14.0], 4.0, X, Z, &funnel);
    // Past the apex, v = -4 and the radius -2: the point at u is half a turn round.
    meets_at(
        &circle([2.0, 0.0, 6.0], 2.0, X, Z),
        &funnel,
        &[
            [2.0 * third, 4.0 * third, -4.0],
            [4.0 * third, 2.0 * third, -4.0],
        ],
        PointContact::Crossing,
    );
    as_the_engine_finds([2.0, 0.0, 6.0], 2.0, X, Z, &funnel);
    // On the axis off the coordinate axes, about 4·(0.6, 0.8, 0) + 5·z: by enclosures alone.
    let leaning = SurfaceKind::Cylinder(cylinder([0.0; 3], 5.0, Z, LEAN));
    meets_at(
        &circle([2.4, 3.2, 5.0], 5.0, Z, LEAN),
        &leaning,
        &[[2.0 * third, third, 4.0], [4.0 * third, 5.0 * third, 4.0]],
        PointContact::Crossing,
    );
    as_the_engine_finds([2.4, 3.2, 5.0], 5.0, Z, LEAN, &leaning);
    // Coaxial and of the cut's radius, the circle still lies on the surface: one overlap.
    lies(&circle([0.0, 0.0, 14.0], 4.0, X, Z), &funnel, 1.0, 4.0);
}

#[test]
fn circles_touching_on_a_coordinate_axis_meet_at_one_tangent_point() {
    // Outside the cut circle and inside it, each touching it at (0, 5, 10), a quarter turn round
    // the axis: no enclosure tells a touch, and the exact signs do.
    let tube = SurfaceKind::Cylinder(cylinder([0.0; 3], 5.0, X, Z));
    let touch = PointContact::IsolatedTangent;
    meets_at(
        &circle([0.0, 8.0, 10.0], 3.0, X, Z),
        &tube,
        &[[1.5, 0.5, 10.0]],
        touch,
    );
    meets_at(
        &circle([0.0, 2.0, 10.0], 3.0, X, Z),
        &tube,
        &[[0.5, 0.5, 10.0]],
        touch,
    );
    // Beside and round the cone's cut of radius 4 at z = 14.
    let funnel = SurfaceKind::Cone(cone([0.0, 0.0, 10.0], 1.0, 0.75, X, Z));
    meets_at(
        &circle([0.0, 7.0, 14.0], 3.0, X, Z),
        &funnel,
        &[[1.5, 0.5, 4.0]],
        touch,
    );
    meets_at(
        &circle([0.0, -3.0, 14.0], 7.0, X, Z),
        &funnel,
        &[[0.5, 0.5, 4.0]],
        touch,
    );
    // The same cone turned over, v the height negated: the cut of radius 4 at z = 6.
    let turned = cone([0.0, 0.0, 10.0], 1.0, 0.75, X, Z)
        .flipped_axially()
        .unwrap();
    meets_at(
        &circle([0.0, 7.0, 6.0], 3.0, X, Z),
        &SurfaceKind::Cone(turned),
        &[[1.5, 0.5, 4.0]],
        touch,
    );
    // About -z, where x̂ × n̂ turns u the other way: the touch at (0, 4, 6) is at u = 1.5.
    let down = SurfaceKind::Cone(cone([0.0, 0.0, 10.0], 1.0, 0.75, X, [0.0, 0.0, -1.0]));
    meets_at(
        &circle([0.0, 7.0, 6.0], 3.0, X, Z),
        &down,
        &[[1.5, 1.5, 4.0]],
        touch,
    );
    // About x, and about y from a centre off the origin: the other coordinates are the plane's.
    let along = SurfaceKind::Cylinder(cylinder([0.0; 3], 5.0, Y, X));
    meets_at(
        &circle([7.0, 0.0, 8.0], 3.0, Y, X),
        &along,
        &[[1.5, 0.5, 7.0]],
        touch,
    );
    let aside = SurfaceKind::Cylinder(cylinder([1.0, 0.0, 2.0], 5.0, Z, Y));
    meets_at(
        &circle([9.0, 4.0, 2.0], 3.0, Z, Y),
        &aside,
        &[[1.5, 0.5, 4.0]],
        touch,
    );
}

#[test]
fn circles_a_hair_apart_on_a_coordinate_axis_are_told_apart_exactly() {
    // The cylinder of radius 1.5 and a circle about (3, 3, 10), √18 from the axis, of the radius
    // r nearest √18 − 1.5 below it: r + 1.5 falls short of √18 by under a step of 18 in the
    // squares, where their enclosures meet. The exact signs say apart.
    let tube = SurfaceKind::Cylinder(cylinder([0.0; 3], 1.5, X, Z));
    let short = 2.742_640_687_119_284_8;
    misses(&circle([3.0, 3.0, 10.0], short, X, Z), &tube);
    // A double larger, the two cross by a hair: two crossings no enclosures hold apart, the
    // engine's.
    undecided(&circle([3.0, 3.0, 10.0], short.next_up(), X, Z), &tube);
}

#[test]
fn circles_touching_off_the_coordinate_axes_are_the_engines() {
    // About the axis (0.6, 0.8, 0) at height 4, circles whose centres stand off the axis along z
    // touch the cut, outside it and inside it: no enclosure tells, and no sum of products of
    // stored doubles is the height.
    let leaning = SurfaceKind::Cylinder(cylinder([0.0; 3], 0.5, Z, LEAN));
    undecided(&circle([2.4, 3.2, 0.75], 0.25, Z, LEAN), &leaning);
    let funnel = SurfaceKind::Cone(cone([0.0; 3], 0.5, 0.0625, Z, LEAN));
    undecided(&circle([2.4, 3.2, 0.5], 0.25, Z, LEAN), &funnel);
}

#[test]
fn a_circle_in_the_apex_plane_is_the_engines() {
    // The cone of radius v about z from the origin cuts its apex's plane in one point, no circle
    // to meet, so the closed form leaves it.
    let funnel = SurfaceKind::Cone(cone([0.0; 3], 0.0, 1.0, X, Z));
    let (spots, work) = points_of(&circle([3.0, 0.0, 0.0], 0.5, X, Z), &funnel);
    assert!(spots.is_empty() && work > FEW, "{spots:?}, {work} units");
}

/// A cylinder of radius 5 about the axis off the coordinate axes, over `v`.
fn leaning_tube(v: (f64, f64)) -> SurfaceKind {
    let frame = Frame::new(Z, LEAN).unwrap();
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 5.0, frame, (ROUND, v)).unwrap())
}

#[test]
fn circles_apart_at_an_end_of_the_domain_meet_nothing() {
    // Each circle's plane is an end of the domain: on a coordinate axis an exact sign says it is on
    // the end, not past it, and off them its enclosure reaches the end. The surface's points in that
    // plane are the cut circle's, on the end or past it, so two circles apart there meet nothing
    // wherever the end lies (issue 995, group R). Inside the cut circle, round it, and beside it with
    // their boxes meeting, at each end.
    let tube = SurfaceKind::Cylinder(cylinder([0.0; 3], 5.0, X, Z));
    for z in [300.0, -300.0] {
        for (center, radius) in [([1.0, 0.0], 1.0), ([1.0, 0.0], 20.0), ([7.0, 7.0], 4.5)] {
            misses(&circle([center[0], center[1], z], radius, X, Z), &tube);
        }
    }
    // The cone's cut at z = 310, v = 300, has radius 226, and past the apex at z = -290, v = -300,
    // radius 224: circles about 8 from the axis inside and round it, as in the stored input, and
    // one beside it.
    let funnel = SurfaceKind::Cone(cone([0.0, 0.0, 10.0], 1.0, 0.75, X, Z));
    for z in [310.0, -290.0] {
        for (center, radius) in [
            ([8.0, 0.0], 100.0),
            ([0.0, 8.0], 400.0),
            ([200.0, 200.0], 50.0),
        ] {
            misses(&circle([center[0], center[1], z], radius, X, Z), &funnel);
        }
    }
    // Off the coordinate axes, about 4·(0.6, 0.8, 0), on a domain that ends there either way: by
    // enclosures alone.
    for ends in [(-300.0, 4.0), (4.0, 300.0)] {
        let leaning = leaning_tube(ends);
        misses(&circle([2.4, 3.2, 1.0], 1.0, Z, LEAN), &leaning);
        misses(&circle([2.4, 3.2, 1.0], 20.0, Z, LEAN), &leaning);
        misses(&circle([-3.2, 7.4, 7.0], 4.5, Z, LEAN), &leaning);
        // The cone of radius 0.5 + 0.75·v about that axis, of radius 3.5 at v = 4.
        let frame = Frame::new(Z, LEAN).unwrap();
        let funnel = Cone::new([0.0; 3], 0.5, 0.75, frame, (ROUND, ends)).unwrap();
        misses(
            &circle([2.4, 3.2, 0.5], 1.0, Z, LEAN),
            &SurfaceKind::Cone(funnel),
        );
    }
}

/// The pair is the engine's: what it finds at the end of the domain it flags at its floor, where a
/// closed form would have proven points.
fn the_engines(curve: &CurveKind<3>, surface: &SurfaceKind) {
    let mut meter = WorkMeter::new(Budget::of_work(50_000_000));
    let found = curve_surface(curve, surface, &mut meter).unwrap();
    let items = found.items();
    assert!(
        items.iter().any(|i| matches!(i, Item::Flagged { .. }))
            && items.iter().all(|i| !matches!(i, Item::Overlap { .. })),
        "{items:?}"
    );
}

#[test]
fn circles_crossing_or_touching_at_an_end_of_the_domain_are_the_engines() {
    // At an end the closed form places no points (decision 32.86): crossing the cut circle and
    // touching it, at each end of a cylinder about z, and crossing it off the coordinate axes.
    let tube = SurfaceKind::Cylinder(cylinder([0.0; 3], 5.0, X, Z));
    for z in [300.0, -300.0] {
        the_engines(&circle([5.0, 0.0, z], 5.0, X, Z), &tube);
        the_engines(&circle([0.0, 8.0, z], 3.0, X, Z), &tube);
    }
    for ends in [(-300.0, 4.0), (4.0, 300.0)] {
        the_engines(&circle([2.4, 3.2, 5.0], 5.0, Z, LEAN), &leaning_tube(ends));
    }
}
