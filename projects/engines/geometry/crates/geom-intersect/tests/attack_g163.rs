//! Attacks on G163: every closed form at sizes where the answer can be read off, the engine on kinds
//! with no closed form, tangency and near misses, and the two done-when counts: no miss on 10,000
//! rational polynomial pairs whose crossings are built in, and `CADCraft` 926 rebuilt from the
//! mathematics (a small circle on a much larger one crosses twice, and is no tangent).

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
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_curve::{Axes, Circle, CurveKind, Ellipse, Frame, Hyperbola, Line, Parabola};
use geom_intersect::IntersectError;
use geom_intersect::curve::{curves, self_crossings};
use geom_intersect::result::{Coincidence, Coverage, Intersection, Item, PointContact, Unresolved};
use geom_nurbs::{Basis, Curve};
use geom_run::{Budget, CancelToken, WorkMeter};
use geom_test::{Seed, SplitMix64};

/// A meter with a budget every test here fits in, so a change that sends the engine round for ever
/// fails on the budget instead of hanging.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(16_000_000))
}

fn meet<const D: usize>(a: &CurveKind<D>, b: &CurveKind<D>) -> Intersection {
    curves(a, b, &mut meter()).unwrap()
}

fn segment<const D: usize>(a: [f64; D], b: [f64; D]) -> CurveKind<D> {
    CurveKind::Line(Line::segment(a, b).unwrap().value)
}

fn infinite<const D: usize>(o: [f64; D], d: [f64; D]) -> CurveKind<D> {
    CurveKind::Line(Line::infinite(o, d).unwrap())
}

fn circle(c: [f64; 2], r: f64) -> CurveKind<2> {
    CurveKind::Circle(Circle::full(c, r, Frame::standard().unwrap()).unwrap())
}

fn arc(c: [f64; 2], r: f64, domain: (f64, f64)) -> CurveKind<2> {
    CurveKind::Circle(Circle::new(c, r, Frame::standard().unwrap(), domain).unwrap())
}

fn ellipse(c: [f64; 2], a: f64, b: f64) -> CurveKind<2> {
    let axes = Axes::Vectors {
        u: [a, 0.0],
        v: [0.0, b],
    };
    CurveKind::Ellipse(Ellipse::new(c, axes, (0.0, 2.0)).unwrap())
}

fn bezier<const D: usize>(points: &[[f64; D]]) -> CurveKind<D> {
    CurveKind::Nurbs(Curve::new(basis(points.len() - 1), points).unwrap())
}

fn basis(p: usize) -> Basis {
    let mut knots = vec![0.0; p + 1];
    knots.extend(vec![1.0; p + 1]);
    Basis::new(p, knots).unwrap()
}

/// The middle of each point item's parameter box, sorted by the first parameter.
fn points(r: &Intersection) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = r
        .items()
        .iter()
        .filter(|i| matches!(i, Item::Point { .. }))
        .map(|i| {
            let s = i.at().spans();
            (
                s[0].lo() * 0.5 + s[0].hi() * 0.5,
                s[1].lo() * 0.5 + s[1].hi() * 0.5,
            )
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
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
fn two_segments_cross() {
    let r = meet(
        &segment([0.0, 0.0], [10.0, 10.0]),
        &segment([0.0, 10.0], [10.0, 0.0]),
    );
    assert_eq!(contacts(&r), [PointContact::Crossing]);
    let p = points(&r);
    near(p[0].0, 0.5);
    assert_eq!(r.coverage(), Coverage::TopologyCertified);
}

#[test]
fn segments_that_stop_short_do_not_meet() {
    is_empty(&meet(
        &segment([0.0, 0.0], [1.0, 1.0]),
        &segment([0.0, 10.0], [10.0, 0.0]),
    ));
}

#[test]
fn segments_that_end_on_each_other_touch() {
    let r = meet(
        &segment([0.0, 0.0], [5.0, 0.0]),
        &segment([5.0, 0.0], [5.0, 5.0]),
    );
    assert_eq!(contacts(&r), [PointContact::BoundaryTouch]);
}

#[test]
fn parallel_lines_never_meet_and_collinear_ones_overlap() {
    is_empty(&meet(
        &infinite([0.0, 0.0], [1.0, 2.0]),
        &infinite([1.0, 0.0], [1.0, 2.0]),
    ));
    let r = meet(
        &segment([0.0, 0.0], [10.0, 0.0]),
        &segment([4.0, 0.0], [20.0, 0.0]),
    );
    assert!(matches!(
        r.items(),
        [Item::Overlap {
            proof: Coincidence::Identical,
            ..
        }]
    ));
    let s = r.items()[0].at().spans();
    near(s[0].lo(), 0.4);
    near(s[0].hi(), 1.0);
    is_empty(&meet(
        &segment([0.0, 0.0], [1.0, 0.0]),
        &segment([4.0, 0.0], [20.0, 0.0]),
    ));
}

#[test]
fn nearly_parallel_lines_keep_their_crossing() {
    // 2^-50 apart in direction: a tolerance would call them parallel, and the determinant's exact
    // sign does not.
    let tiny = 2.0_f64.powi(-50);
    let r = meet(
        &infinite([0.0, 0.0], [1.0, 0.0]),
        &infinite([3.0, 0.0], [1.0, tiny]),
    );
    assert_eq!(r.items().len(), 1);
}

#[test]
fn lines_in_space_are_decided_exactly() {
    // Skew by one step in a long way.
    is_empty(&meet(
        &segment([0.0, 0.0, 0.0], [1000.0, 0.0, 0.0]),
        &segment([500.0, -500.0, 1.0], [500.0, 500.0, 1.0]),
    ));
    let r = meet(
        &segment([0.0, 0.0, 0.0], [1000.0, 0.0, 0.0]),
        &segment([500.0, -500.0, 0.0], [500.0, 500.0, 0.0]),
    );
    assert_eq!(contacts(&r), [PointContact::Crossing]);
    // In a plane no coordinate plane holds, and in one where x and y alone would be degenerate.
    let r = meet(
        &segment([0.0, 7.0, 0.0], [0.0, 7.0, 9.0]),
        &segment([-4.0, 7.0, 3.0], [4.0, 7.0, 3.0]),
    );
    assert_eq!(contacts(&r), [PointContact::Crossing]);
    let r = meet(
        &segment([1.0, 1.0, 1.0], [5.0, 5.0, 5.0]),
        &segment([3.0, 3.0, 3.0], [9.0, 9.0, 9.0]),
    );
    assert!(matches!(r.items(), [Item::Overlap { .. }]));
    // Parallel and apart.
    is_empty(&meet(
        &segment([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        &segment([0.0, 0.0, 1.0], [1.0, 1.0, 2.0]),
    ));
}

#[test]
fn two_circles_cross_touch_or_miss() {
    let a = circle([0.0, 0.0], 5.0);
    assert_eq!(meet(&a, &circle([6.0, 0.0], 5.0)).items().len(), 2);
    let r = meet(&a, &circle([10.0, 0.0], 5.0));
    assert_eq!(contacts(&r), [PointContact::IsolatedTangent]);
    near(points(&r)[0].0, 0.0);
    near(points(&r)[0].1, 1.0);
    let r = meet(&a, &circle([3.0, 0.0], 2.0));
    assert_eq!(contacts(&r), [PointContact::IsolatedTangent]);
    is_empty(&meet(&a, &circle([11.0, 0.0], 5.0)));
    is_empty(&meet(&a, &circle([1.0, 0.0], 2.0)));
    is_empty(&meet(&a, &circle([0.0, 0.0], 4.0)));
}

#[test]
fn a_3_4_5_pair_meets_where_it_should() {
    // Radii 5 and 5 with centres (0, 0) and (6, 0): the crossings are (3, ±4).
    let r = meet(&circle([0.0, 0.0], 5.0), &circle([6.0, 0.0], 5.0));
    let turn = 4.0_f64.atan2(3.0) / core::f64::consts::PI;
    let p = points(&r);
    near(p[0].0, turn);
    near(p[0].1, 1.0 - turn);
    near(p[1].0, 2.0 - turn);
    near(p[1].1, 1.0 + turn);
}

#[test]
fn cadcraft_926_a_small_circle_on_a_large_one_crosses_twice() {
    // CADCraft's issue 926: for very different radii a real two-point crossing collapsed to one
    // false "tangent" whose foot need not lie on the small circle. Rebuilt from the mathematics: a
    // unit circle centred on a circle of radius 2^30 crosses it twice, about one half-turn of the
    // small circle apart.
    let big = 2.0_f64.powi(30);
    let a = circle([0.0, 0.0], big);
    let b = circle([big, 0.0], 1.0);
    let r = meet(&a, &b);
    assert_eq!(
        contacts(&r),
        [PointContact::Crossing, PointContact::Crossing],
        "{:?}",
        r.items()
    );
    let p = points(&r);
    // Each point is on both circles: t on the small one, s on the large.
    for (s, t) in &p {
        let (sin, cos) = (t * core::f64::consts::PI).sin_cos();
        let (x, y) = (big + cos, sin);
        assert!((x.hypot(y) - big).abs() < 1e-5, "{t}");
        let (ss, sc) = (s * core::f64::consts::PI).sin_cos();
        assert!((big * sc - x).abs() + (big * ss - y).abs() < 1e-3, "{s}");
    }
    // Radii a billion apart, either side of the true tangent by the least the doubles can say.
    let touch = circle([big + 1.0, 0.0], 1.0);
    assert_eq!(contacts(&meet(&a, &touch)), [PointContact::IsolatedTangent]);
    let past = circle([big + 1.0 + 2.0_f64.powi(-22), 0.0], 1.0);
    is_empty(&meet(&a, &past));
    let short = circle([big + 1.0 - 2.0_f64.powi(-22), 0.0], 1.0);
    assert_eq!(meet(&a, &short).items().len(), 2);
}

#[test]
fn circles_too_close_to_tell_apart_are_one_band_or_two_points() {
    // Off the axes, so the distance between the centres is rounded and the height may reach zero,
    // while the exact signs still say the circles cross.
    let (x, y) = (1_000_000_007.0_f64, 998_244_353.0_f64);
    let r = x.hypot(y) * 0.5;
    for k in -3..=3 {
        let q = r + f64::from(k) * 2.0_f64.powi(-23);
        let a = circle([0.0, 0.0], r);
        let found = meet(&a, &circle([x, y], q));
        match found.items() {
            [] | [Item::Point { .. }] | [Item::Point { .. }, Item::Point { .. }] => {
                assert_eq!(found.coverage(), Coverage::TopologyCertified);
            }
            [Item::Flagged { .. }] => {
                assert_eq!(found.coverage(), Coverage::CompleteOutsideFlagged);
            }
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn two_infinite_lines_overlap_as_far_as_the_grid_reaches() {
    let r = meet(
        &infinite([0.0, 0.0], [1.0, 1.0]),
        &infinite([2.0, 2.0], [3.0, 3.0]),
    );
    assert!(
        matches!(r.items(), [Item::Overlap { .. }]),
        "{:?}",
        r.items()
    );
}

#[test]
fn arcs_of_one_circle_overlap_where_they_share_turns() {
    let a = arc([0.0, 0.0], 5.0, (0.0, 1.0));
    let b = arc([0.0, 0.0], 5.0, (0.5, 1.5));
    let r = meet(&a, &b);
    assert!(
        matches!(r.items(), [Item::Overlap { .. }]),
        "{:?}",
        r.items()
    );
    let s = r.items()[0].at().spans();
    near(s[0].lo(), 0.5);
    near(s[0].hi(), 1.0);
    near(s[1].lo(), 0.5);
    near(s[1].hi(), 1.0);
    is_empty(&meet(&a, &arc([0.0, 0.0], 5.0, (1.25, 1.75))));
    // Ends meet: one point, no overlap.
    let r = meet(&a, &arc([0.0, 0.0], 5.0, (1.0, 1.5)));
    assert_eq!(contacts(&r), [PointContact::BoundaryTouch]);
    // Two long arcs share two runs.
    let r = meet(
        &arc([0.0, 0.0], 5.0, (0.0, 1.5)),
        &arc([0.0, 0.0], 5.0, (1.0, 2.5)),
    );
    assert_eq!(r.items().len(), 2, "{:?}", r.items());
    assert_eq!(
        meet(&circle([1.0, 2.0], 3.0), &circle([1.0, 2.0], 3.0))
            .items()
            .len(),
        1
    );
}

#[test]
fn arcs_keep_only_the_crossings_on_them() {
    let a = circle([0.0, 0.0], 5.0);
    assert_eq!(meet(&a, &arc([6.0, 0.0], 5.0, (0.5, 1.0))).items().len(), 1);
    is_empty(&meet(&a, &arc([6.0, 0.0], 5.0, (-0.5, 0.5))));
    let r = meet(&arc([0.0, 0.0], 5.0, (-0.5, 0.5)), &circle([6.0, 0.0], 5.0));
    assert_eq!(r.items().len(), 2);
    assert!(points(&r).iter().all(|(s, _)| (-0.5..=0.5).contains(s)));
}

#[test]
fn a_line_meets_a_circle() {
    let a = circle([0.0, 0.0], 5.0);
    let r = meet(&infinite([0.0, 3.0], [1.0, 0.0]), &a);
    let p = points(&r);
    assert_eq!(p.len(), 2);
    near(p[0].0, -4.0);
    near(p[1].0, 4.0);
    let turn = 3.0_f64.atan2(4.0) / core::f64::consts::PI;
    near(p[0].1, 1.0 - turn);
    near(p[1].1, turn);
    let tangent = meet(&infinite([0.0, 5.0], [1.0, 0.0]), &a);
    assert_eq!(contacts(&tangent), [PointContact::IsolatedTangent]);
    is_empty(&meet(&infinite([0.0, 6.0], [1.0, 0.0]), &a));
    // A whole-number line and circle decide a tangent that no interval could see: 3-4-5 scaled up.
    let k = 2.0_f64.powi(30);
    let huge = circle([0.0, 0.0], 5.0 * k);
    let line = infinite([3.0 * k * 3.0, 4.0 * k * 3.0], [-4.0, 3.0]);
    is_empty(&meet(&line, &huge));
    let line = infinite([3.0 * k, 4.0 * k], [-4.0, 3.0]);
    assert_eq!(
        contacts(&meet(&line, &huge)),
        [PointContact::IsolatedTangent]
    );
}

#[test]
fn a_swapped_pair_gives_the_same_points_swapped() {
    let a = circle([0.0, 0.0], 5.0);
    let b = infinite([0.0, 3.0], [1.0, 0.0]);
    let ab = meet(&b, &a);
    let ba = meet(&a, &b);
    let mut p: Vec<(f64, f64)> = points(&ba).into_iter().map(|(s, t)| (t, s)).collect();
    p.sort_by(|x, y| x.0.total_cmp(&y.0));
    for (x, y) in p.iter().zip(points(&ab)) {
        near(x.0, y.0);
        near(x.1, y.1);
    }
}

#[test]
fn a_segment_and_a_ray_stop_where_they_stop() {
    let a = circle([0.0, 0.0], 5.0);
    assert_eq!(meet(&segment([0.0, 3.0], [9.0, 3.0]), &a).items().len(), 1);
    is_empty(&meet(&segment([0.0, 3.0], [1.0, 3.0]), &a));
    let ray = CurveKind::Line(Line::ray([0.0, 3.0], [1.0, 0.0]).unwrap());
    assert_eq!(meet(&ray, &a).items().len(), 1);
}

#[test]
fn a_line_meets_each_conic() {
    let e = ellipse([1.0, 1.0], 4.0, 2.0);
    let r = meet(&infinite([1.0, -9.0], [0.0, 1.0]), &e);
    let p = points(&r);
    assert_eq!(p.len(), 2);
    near(p[0].0, 8.0);
    near(p[1].0, 12.0);
    is_empty(&meet(&infinite([6.0, 0.0], [0.0, 1.0]), &e));

    let axes = Axes::Vectors {
        u: [3.0, 0.0],
        v: [0.0, 2.0],
    };
    let h = CurveKind::Hyperbola(Hyperbola::new([0.0, 0.0], axes, (-4.0, 4.0)).unwrap());
    // x = 6 is cosh t = 2 on the branch: two points, sinh t = ±√3.
    let p = points(&meet(&infinite([6.0, 0.0], [0.0, 1.0]), &h));
    assert_eq!(p.len(), 2);
    near(p[0].1, -(3.0_f64.sqrt().asinh()));
    near(p[1].1, 3.0_f64.sqrt().asinh());
    // x = -6 meets only the other branch, which is not the curve.
    is_empty(&meet(&infinite([-6.0, 0.0], [0.0, 1.0]), &h));

    let axes = Axes::Vectors {
        u: [1.0, 0.0],
        v: [0.0, 1.0],
    };
    let parabola = CurveKind::Parabola(Parabola::new([0.0, 0.0], axes, (-3.0, 3.0)).unwrap());
    // x = t², y = t: y = 1 runs along the axis and meets it once; x = 4 twice but off the arc's end.
    let p = points(&meet(&infinite([-5.0, 1.0], [1.0, 0.0]), &parabola));
    assert_eq!(p.len(), 1);
    near(p[0].0, 6.0);
    near(p[0].1, 1.0);
    let p = points(&meet(&infinite([4.0, 0.0], [0.0, 1.0]), &parabola));
    assert_eq!(p.len(), 2);
    near(p[0].1, -2.0);
    near(p[1].1, 2.0);
    is_empty(&meet(&infinite([10.0, 0.0], [0.0, 1.0]), &parabola));
    is_empty(&meet(&infinite([-1.0, 0.0], [0.0, 1.0]), &parabola));
}

#[test]
fn a_rotated_circle_is_met_on_its_own_turns() {
    let frame = Frame::new([0.0, 1.0], [0.0, 0.0, 1.0]).unwrap();
    let a = CurveKind::Circle(Circle::full([0.0, 0.0], 5.0, frame).unwrap());
    let p = points(&meet(&segment([0.0, 0.0], [0.0, 9.0]), &a));
    assert_eq!(p.len(), 1);
    near(p[0].1, 0.0);
    let frame = Frame::new([1.0, 0.0], [0.0, 0.0, -1.0]).unwrap();
    let a = CurveKind::Circle(Circle::full([0.0, 0.0], 5.0, frame).unwrap());
    let p = points(&meet(&segment([0.0, 0.0], [0.0, 9.0]), &a));
    near(p[0].1, 1.5);
}

#[test]
fn two_ellipses_meet_four_times() {
    let r = meet(
        &ellipse([0.0, 0.0], 100.0, 10.0),
        &ellipse([0.0, 0.0], 10.0, 100.0),
    );
    assert_eq!(contacts(&r).len(), 4, "{:?}", r.items());
    assert!(r.items().iter().all(|i| matches!(i, Item::Point { .. })));
}

#[test]
fn a_cubic_pair_meets_nine_times() {
    // A cubic of two S-bends against its reflection: y = f(x) and x = f(y) with
    // f(u) = 4u³ − 3u scaled, three by three.
    let s = 1000.0;
    let a = bezier(&[[-s, -s], [-s / 3.0, 5.0 * s], [s / 3.0, -5.0 * s], [s, s]]);
    let b = bezier(&[[-s, -s], [5.0 * s, -s / 3.0], [-5.0 * s, s / 3.0], [s, s]]);
    let r = meet(&a, &b);
    let n = r.items().len();
    assert_eq!(n, 9, "{:?}", r.items());
}

#[test]
fn a_curve_in_space_passes_a_line_or_is_flagged_beside_it() {
    // 12.5: two space curves meeting is no stable fact, so a shadow's crossing whose third coordinate
    // agrees only to within its enclosure is a near miss, and one a step away is nothing.
    let a = bezier(&[[0.0, 0.0, 0.0], [10.0, 0.0, 5.0], [20.0, 0.0, 0.0]]);
    let through = segment([10.0, -9.0, 2.5], [10.0, 9.0, 2.5]);
    let r = meet(&a, &through);
    assert!(
        matches!(
            r.items(),
            [Item::Flagged {
                why: Unresolved::NearMiss { .. },
                ..
            }]
        ),
        "{:?}",
        r.items()
    );
    is_empty(&meet(&a, &segment([10.0, -9.0, 3.5], [10.0, 9.0, 3.5])));
}

#[test]
fn a_pair_in_one_plane_of_space_is_proven() {
    // z is the same double everywhere, so the coordinate left out agrees exactly.
    let a = bezier(&[[0.0, 0.0, 7.0], [10.0, 40.0, 7.0], [20.0, 0.0, 7.0]]);
    let b = bezier(&[[0.0, 10.0, 7.0], [20.0, 10.0, 7.0]]);
    assert_eq!(contacts(&meet(&a, &b)), [PointContact::Crossing; 2]);
    // In the plane y = 7, where x and y alone would see a pair of lines.
    let a = bezier(&[[0.0, 7.0, 0.0], [10.0, 7.0, 40.0], [20.0, 7.0, 0.0]]);
    let b = bezier(&[[0.0, 7.0, 10.0], [20.0, 7.0, 10.0]]);
    assert_eq!(contacts(&meet(&a, &b)), [PointContact::Crossing; 2]);
}

#[test]
fn the_least_gap_in_space_is_a_gap() {
    // The same shadow's crossing, the third coordinate apart by 1e-300 and proven so.
    let a = bezier(&[[0.0, 0.0, 0.0], [10.0, 10.0, 0.0], [20.0, 0.0, 0.0]]);
    is_empty(&meet(
        &a,
        &segment([10.0, -9.0, 1e-300], [10.0, 9.0, 1e-300]),
    ));
}

#[test]
fn conics_proven_to_share_a_plane_are_decided_in_it() {
    // A tilted plane through the origin, on a whole-number frame: x̂ = (1, 0, 0), n = (0, 1, 1).
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 1.0]).unwrap();
    let circle = CurveKind::Circle(Circle::full([0.0, 0.0, 0.0], 5.0, frame).unwrap());
    // Along x, through the middle: two crossings, by the line's own parameter.
    let p = points(&meet(&infinite([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]), &circle));
    assert_eq!(p.len(), 2);
    near(p[0].0, -5.0);
    near(p[1].0, 5.0);
    // The same line, lifted out of the plane by one step: a pair in space with no proof of a plane.
    let r = meet(&infinite([0.0, 1.0, 0.0], [1.0, 0.0, 0.0]), &circle);
    assert_eq!(contacts(&r).len(), 0);
    // Two circles level in z.
    let level = |c: [f64; 3], r: f64| {
        CurveKind::Circle(Circle::full(c, r, Frame::standard().unwrap()).unwrap())
    };
    let r = meet(&level([0.0, 0.0, 4.0], 5.0), &level([6.0, 0.0, 4.0], 5.0));
    assert_eq!(contacts(&r), [PointContact::Crossing; 2]);
    let r = meet(&level([0.0, 0.0, 4.0], 5.0), &level([10.0, 0.0, 4.0], 5.0));
    assert_eq!(contacts(&r), [PointContact::IsolatedTangent]);
    // Level in x, by a frame whose axis is x.
    let side = |c: [f64; 3], r: f64| {
        let frame = Frame::new([0.0, 1.0, 0.0], [1.0, 0.0, 0.0]).unwrap();
        CurveKind::Circle(Circle::full(c, r, frame).unwrap())
    };
    let r = meet(&side([2.0, 0.0, 0.0], 5.0), &side([2.0, 6.0, 0.0], 5.0));
    assert_eq!(contacts(&r), [PointContact::Crossing; 2]);
}

#[test]
fn a_closed_form_that_cannot_decide_says_so() {
    // A line that an interval cannot tell from tangent to an ellipse: the engine takes it and the
    // answer records the change of route.
    let e = ellipse([0.0, 0.0], 3.0, 7.0);
    let r = meet(&infinite([-9.0, 7.0], [1.0, 0.0]), &e);
    assert_eq!(r.diagnostics().len(), 1);
    assert_eq!(r.diagnostics()[0].code, geom_intersect::curve::ROUTE);
    let r = meet(&infinite([-9.0, 1.0], [1.0, 0.0]), &e);
    assert_eq!(r.diagnostics().len(), 0);
}

#[test]
fn a_meeting_on_a_closed_curves_join_is_one_proven_point() {
    // A cubic that crosses the circle at t = 0 and only there: the join is no boundary.
    let c = circle([0.0, 0.0], 500.0);
    let across = bezier(&[[300.0, 40.0], [450.0, 20.0], [550.0, -20.0], [700.0, -40.0]]);
    let r = meet(&c, &across);
    assert_eq!(contacts(&r), [PointContact::Crossing], "{:?}", r.items());
    let (s, _) = points(&r)[0];
    assert!(s < 1e-6 || s > 2.0 - 1e-6 || (s - 0.0).abs() < 1e-3, "{s}");
    let e = ellipse([0.0, 0.0], 500.0, 300.0);
    assert_eq!(contacts(&meet(&e, &across)), [PointContact::Crossing]);
    // Past the join, on the far side of the turn.
    let up = bezier(&[
        [-700.0, -30.0],
        [-550.0, -10.0],
        [-450.0, 10.0],
        [-300.0, 30.0],
    ]);
    assert_eq!(contacts(&meet(&c, &up)), [PointContact::Crossing]);
}

#[test]
fn a_tangency_without_a_sign_change_is_not_lost() {
    // y = 4x(1 − x) under a roof at y = 1 touches once and changes no sign: sample-and-bisect misses
    // it, and a Bézier line keeps the pair away from the closed forms.
    let a = bezier(&[[0.0, 0.0], [500.0, 2000.0], [1000.0, 0.0]]);
    let roof = bezier(&[[0.0, 1000.0], [1000.0, 1000.0]]);
    let r = meet(&a, &roof);
    let flagged: Vec<&Item> = r
        .items()
        .iter()
        .filter(|i| {
            matches!(
                i,
                Item::Flagged {
                    why: Unresolved::TangentBand { .. },
                    ..
                }
            )
        })
        .collect();
    assert_eq!(flagged.len(), 1, "{:?}", r.items());
    let s = flagged[0].at().spans();
    assert!(s[0].contains(0.5), "{s:?}");
    assert_eq!(r.coverage(), Coverage::CompleteOutsideFlagged);
}

#[test]
fn a_near_miss_is_nothing_and_a_near_crossing_is_two() {
    let a = bezier(&[[0.0, 0.0], [500.0, 2000.0], [1000.0, 0.0]]);
    is_empty(&meet(&a, &bezier(&[[0.0, 1001.0], [1000.0, 1001.0]])));
    let r = meet(&a, &bezier(&[[0.0, 990.0], [1000.0, 990.0]]));
    assert_eq!(contacts(&r), [PointContact::Crossing; 2], "{:?}", r.items());
}

#[test]
fn an_inflection_on_a_line_is_of_higher_order() {
    // y = x³ on the x axis: curvatures agree where the tangents do.
    let a = bezier(&[
        [-300.0, -2700.0],
        [-100.0, 2700.0],
        [100.0, -2700.0],
        [300.0, 2700.0],
    ]);
    let r = meet(&a, &bezier(&[[-300.0, 0.0], [300.0, 0.0]]));
    assert!(
        r.items().iter().any(|i| matches!(
            i,
            Item::Flagged {
                why: Unresolved::HigherOrderContact,
                ..
            }
        )),
        "{:?}",
        r.items()
    );
}

#[test]
fn a_curve_overlaps_itself_exactly() {
    let a = bezier(&[[0.0, 0.0], [5.0, 9.0], [10.0, 0.0]]);
    let r = meet(&a, &a);
    assert!(matches!(
        r.items(),
        [Item::Overlap {
            proof: Coincidence::Identical,
            ..
        }]
    ));
}

#[test]
fn a_near_copy_is_flagged_and_never_reported_apart() {
    let a = bezier(&[[0.0, 0.0], [500.0, 900.0], [1000.0, 0.0]]);
    let b = bezier(&[[0.0, 2.0_f64.powi(-30)], [500.0, 900.0], [1000.0, 0.0]]);
    let r = meet(&a, &b);
    assert_eq!(r.coverage(), Coverage::CompleteOutsideFlagged);
    assert!(
        r.items().iter().any(|i| matches!(i, Item::Flagged { .. })),
        "{:?}",
        r.items()
    );
}

#[test]
fn an_infinite_line_cuts_a_curve_wherever_it_starts() {
    let a = bezier(&[[0.0, 0.0], [500.0, 2000.0], [1000.0, 0.0]]);
    let r = meet(&infinite([-1e6, 500.0], [1.0, 0.0]), &a);
    assert_eq!(r.items().len(), 2, "{:?}", r.items());
    is_empty(&meet(&infinite([-1e6, 5000.0], [1.0, 0.0]), &a));
}

#[test]
fn a_loop_crosses_itself_once() {
    let a = bezier(&[
        [0.0, 0.0],
        [3000.0, 2000.0],
        [-2000.0, 2000.0],
        [1000.0, 0.0],
    ]);
    let r = self_crossings(&a, &mut meter()).unwrap();
    assert_eq!(contacts(&r), [PointContact::Crossing], "{:?}", r.items());
    let p = points(&r);
    assert!(p[0].0 < p[0].1);
    // The two parameters are symmetric about the middle, as the control points are.
    near(p[0].0 + p[0].1, 1.0);
}

#[test]
fn curves_without_a_loop_have_no_crossing() {
    is_empty(
        &self_crossings(
            &bezier(&[[0.0, 0.0], [5.0, 9.0], [10.0, 0.0]]),
            &mut meter(),
        )
        .unwrap(),
    );
    let s = bezier(&[[0.0, 0.0], [300.0, 900.0], [700.0, -900.0], [1000.0, 0.0]]);
    is_empty(&self_crossings(&s, &mut meter()).unwrap());
    is_empty(&self_crossings(&segment([0.0, 0.0], [9.0, 9.0]), &mut meter()).unwrap());
}

#[test]
fn a_closed_curve_is_not_crossed_at_its_join() {
    let c = circle([0.0, 0.0], 500.0);
    is_empty(&self_crossings(&c, &mut meter()).unwrap());
    // Closed with a corner at the join, which the tangents cannot run through.
    let drop = bezier(&[[0.0, 0.0], [900.0, 900.0], [-900.0, 900.0], [0.0, 0.0]]);
    is_empty(&self_crossings(&drop, &mut meter()).unwrap());
}

#[test]
fn a_stop_stops() {
    let a = bezier(&[[0.0, 0.0], [500.0, 2000.0], [1000.0, 0.0]]);
    let b = bezier(&[[0.0, 500.0], [1000.0, 500.0]]);
    let token = CancelToken::new();
    token.cancel();
    let mut m = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    assert_eq!(
        curves(&a, &b, &mut m).unwrap_err(),
        IntersectError::Cancelled
    );
    let mut m = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    assert_eq!(
        self_crossings(&a, &mut m).unwrap_err(),
        IntersectError::Cancelled
    );
    let mut m = WorkMeter::new(Budget::new(3, 0));
    assert_eq!(
        curves(&a, &b, &mut m).unwrap_err(),
        IntersectError::BudgetExceeded
    );
}

/// A number in [0, 1).
fn unit(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 / 2.0_f64.powi(53)
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

/// The rational curve with homogeneous polynomials X, Y over W, on [0, 1], in degree p.
fn rational(x: &[f64], y: &[f64], w: &[f64], p: usize) -> CurveKind<2> {
    let (bx, by, bw) = (bernstein(x, p), bernstein(y, p), bernstein(w, p));
    let points: Vec<[f64; 2]> = bx
        .iter()
        .zip(&by)
        .zip(&bw)
        .map(|((x, y), w)| [x / w, y / w])
        .collect();
    CurveKind::Nurbs(Curve::new_rational(basis(p), &points, &bw).unwrap())
}

#[test]
fn no_crossing_is_missed_on_10_000_rational_polynomial_pairs() {
    // y = q(x) against y = q(x) + c·Π(x − rᵢ)/w(x) with w > 0, scaled to a thousand steps: the two
    // meet exactly at the rᵢ, none of them double, and the second is rational. Every rᵢ must lie in
    // some item's box, in a run of 10,000 pairs, and no point may be reported anywhere else.
    let mut rng = SplitMix64::new(geom_test::seed::fixed("g163 pairs", Seed::DEFAULT));
    let scale = 1000.0;
    for pair in 0..10_000 {
        let n = 1 + (rng.next_u64() % 4) as usize;
        let mut roots: Vec<f64> = Vec::new();
        while roots.len() < n {
            let r = 0.05 + 0.9 * unit(&mut rng);
            if roots.iter().all(|x| (x - r).abs() > 0.05) {
                roots.push(r);
            }
        }
        let c = if pair % 2 == 0 { 1.0 } else { -1.0 } * (1.0 + 9.0 * unit(&mut rng));
        let difference = roots.iter().fold(vec![c], |p, r| times(&p, &[-r, 1.0]));
        let q = [unit(&mut rng), unit(&mut rng) - 0.5, unit(&mut rng) - 0.5];
        // A weight with no root on [0, 1]: positive at each Bernstein coefficient.
        let w = [
            1.0 + unit(&mut rng),
            unit(&mut rng) - 0.5,
            0.2 * unit(&mut rng),
        ];
        let degree = (n + 1).max(q.len() + w.len() - 2).max(w.len());
        let x = times(&w, &[0.0, scale]);
        let y = plus(
            &times(&times(&q, &w), &[scale]),
            &times(&difference, &[scale]),
        );
        let a = rational(&x, &y, &w, degree);
        let b = rational(&[0.0, scale], &times(&q, &[scale]), &[1.0], 2);
        let r = meet(&a, &b);
        for root in &roots {
            let seen = r.items().iter().any(|i| {
                let s = i.at().spans();
                s[0].lo() - 1e-9 <= *root && *root <= s[0].hi() + 1e-9
            });
            assert!(seen, "pair {pair}: root {root} missed in {:?}", r.items());
        }
        for item in r.items() {
            let s = item.at().spans();
            assert!(
                roots
                    .iter()
                    .any(|x| s[0].lo() - 1e-9 <= *x && *x <= s[0].hi() + 1e-9),
                "pair {pair}: {item:?}, roots {roots:?}"
            );
        }
    }
}

#[test]
fn a_crossing_past_one_segment_s_end_is_no_meeting() {
    // The lines cross at (5, 0), inside one segment and past an end of the other: before its start
    // or after its end, the first's or the second's. Nothing is shared, and nothing fails.
    let a = segment([0.0, 0.0], [10.0, 0.0]);
    for b in [
        segment([5.0, 1.0], [5.0, 3.0]),
        segment([5.0, -3.0], [5.0, -1.0]),
    ] {
        is_empty(&meet(&a, &b));
        is_empty(&meet(&b, &a));
    }
    for b in [
        segment([-5.0, -1.0], [-5.0, 1.0]),
        segment([15.0, -1.0], [15.0, 1.0]),
    ] {
        is_empty(&meet(&a, &b));
        is_empty(&meet(&b, &a));
    }
}

#[test]
fn an_end_on_the_other_segment_is_a_boundary_touch_within_both_domains() {
    // A T: one segment's start or end on the other's inside, each segment first.
    let bar = segment([0.0, 0.0], [10.0, 0.0]);
    for stem in [
        segment([5.0, 0.0], [5.0, 5.0]),
        segment([5.0, 5.0], [5.0, 0.0]),
    ] {
        for r in [meet(&stem, &bar), meet(&bar, &stem)] {
            assert_eq!(
                contacts(&r),
                [PointContact::BoundaryTouch],
                "{:?}",
                r.items()
            );
            for span in r.items()[0].at().spans() {
                assert!(0.0 <= span.lo() && span.hi() <= 1.0, "{span:?}");
            }
        }
    }
}

#[test]
fn a_collinear_overlap_is_cut_to_both_domains() {
    // The second runs past the first's start, then past its end: the overlap stops where the first
    // does.
    let a = segment([0.0, 0.0], [10.0, 0.0]);
    for (b, want) in [
        (segment([-5.0, 0.0], [5.0, 0.0]), (0.0, 0.5)),
        (segment([5.0, 0.0], [15.0, 0.0]), (0.5, 1.0)),
    ] {
        let r = meet(&a, &b);
        assert!(
            matches!(r.items(), [Item::Overlap { .. }]),
            "{:?}",
            r.items()
        );
        let s = r.items()[0].at().spans()[0];
        near(s.lo(), want.0);
        near(s.hi(), want.1);
    }
}

#[test]
fn parallel_lines_of_space_one_coordinate_apart_do_not_meet() {
    // In the plane y = 0, one unit apart in z: projected along z they would be one line.
    for (a, b) in [
        (
            segment([0.0, 0.0, 0.0], [10.0, 0.0, 0.0]),
            segment([0.0, 0.0, 1.0], [10.0, 0.0, 1.0]),
        ),
        (
            segment([0.0, 0.0, 0.0], [0.0, 10.0, 0.0]),
            segment([3.0, 0.0, 0.0], [3.0, 10.0, 0.0]),
        ),
        (
            segment([0.0, 0.0, 0.0], [0.0, 0.0, 10.0]),
            segment([0.0, 2.0, 0.0], [0.0, 2.0, 10.0]),
        ),
    ] {
        is_empty(&meet(&a, &b));
    }
}

#[test]
fn a_line_and_a_conic_in_closed_form_take_only_its_work() {
    // The closed form answers these from the quadratic's discriminant; a refused discriminant
    // would leave them to the engine, which takes far more. The counts are a deterministic
    // function of the input.
    let work = |a: &CurveKind<2>, b: &CurveKind<2>| {
        let mut m = meter();
        curves(a, b, &mut m).unwrap();
        m.used().work
    };
    let e = ellipse([1.0, 1.0], 4.0, 2.0);
    let miss = work(&infinite([6.0, 0.0], [0.0, 1.0]), &e);
    let twice = work(&infinite([1.0, -9.0], [0.0, 1.0]), &e);
    assert_eq!((miss, twice), (0, 6));
}

/// Each item of `r` as whether it is an overlap, and its parameter box's bounds, s then t.
fn pieces(r: &Intersection) -> Vec<(bool, [f64; 4])> {
    r.items()
        .iter()
        .map(|i| {
            let s = i.at().spans();
            (
                matches!(i, Item::Overlap { .. }),
                [s[0].lo(), s[0].hi(), s[1].lo(), s[1].hi()],
            )
        })
        .collect()
}

/// `r`'s items are `want`'s, each bound within 2^-40.
fn pieces_are(r: &Intersection, want: &[(bool, [f64; 4])]) {
    let got = pieces(r);
    assert_eq!(got.len(), want.len(), "{got:?}");
    for ((o, g), (p, w)) in got.iter().zip(want) {
        assert_eq!(o, p, "{got:?}");
        assert!(
            g.iter()
                .zip(w)
                .all(|(g, w)| (g - w).abs() < 2.0_f64.powi(-40)),
            "{got:?}"
        );
    }
}

/// An arc of the circle about the origin of radius 5, its frame's x̂ along `x` and its normal along
/// ±z as `n` is.
fn turned(x: [f64; 2], n: f64, domain: (f64, f64)) -> CurveKind<2> {
    let frame = Frame::new(x, [0.0, 0.0, n]).unwrap();
    CurveKind::Circle(Circle::new([0.0, 0.0], 5.0, frame, domain).unwrap())
}

#[test]
fn an_arc_of_one_circle_is_placed_by_its_turn_and_its_normal() {
    // The upper half, 0 to π; parameters are in half-turns.
    let a = turned([1.0, 0.0], 1.0, (0.0, 1.0));
    // Turned by π, the same way round: the lower half, meeting a at its two ends.
    pieces_are(
        &meet(&a, &turned([-1.0, 0.0], 1.0, (0.0, 1.0))),
        &[(false, [0.0, 0.0, 1.0, 1.0]), (false, [1.0, 1.0, 0.0, 0.0])],
    );
    // Turned by π/2: from π/2 to 3π/2, one with a on a's second quarter.
    pieces_are(
        &meet(&a, &turned([0.0, 1.0], 1.0, (0.0, 1.0))),
        &[(true, [0.5, 1.0, 0.0, 0.5])],
    );
    // The other way round from π: the upper half again, run backwards.
    pieces_are(
        &meet(&a, &turned([-1.0, 0.0], -1.0, (0.0, 1.0))),
        &[(true, [0.0, 1.0, 0.0, 1.0])],
    );
    // The other way round from 0: the lower half, meeting a at its two ends.
    pieces_are(
        &meet(&a, &turned([1.0, 0.0], -1.0, (0.0, 1.0))),
        &[(false, [0.0, 0.0, 0.0, 0.0]), (false, [1.0, 1.0, 1.0, 1.0])],
    );
    // Four turns on: the same half, its parameters 8 more.
    pieces_are(
        &meet(&a, &turned([1.0, 0.0], 1.0, (8.0, 9.0))),
        &[(true, [0.0, 1.0, 8.0, 9.0])],
    );
}

#[test]
fn a_whole_circle_and_an_arc_of_it_share_only_the_arc() {
    let whole = turned([1.0, 0.0], 1.0, (0.0, 2.0));
    let part = turned([1.0, 0.0], 1.0, (0.5, 1.75));
    pieces_are(&meet(&part, &whole), &[(true, [0.5, 1.75, 0.5, 1.75])]);
    pieces_are(&meet(&whole, &part), &[(true, [0.5, 1.75, 0.5, 1.75])]);
    pieces_are(&meet(&whole, &whole), &[(true, [0.0, 2.0, 0.0, 2.0])]);
}

#[test]
fn arcs_end_to_end_on_one_circle_touch_at_both_shared_ends() {
    // b runs from π to 2π, so it meets a, 0 to π, at π and at 2π, which is a's 0.
    let r = meet(
        &turned([1.0, 0.0], 1.0, (0.0, 1.0)),
        &turned([1.0, 0.0], 1.0, (1.0, 2.0)),
    );
    assert!(
        r.items().iter().all(|i| matches!(
            i,
            Item::Point {
                contact: PointContact::BoundaryTouch,
                ..
            }
        )),
        "{:?}",
        r.items()
    );
    pieces_are(
        &r,
        &[(false, [0.0, 0.0, 2.0, 2.0]), (false, [1.0, 1.0, 1.0, 1.0])],
    );
}

#[test]
fn the_engine_takes_only_its_work() {
    // Pairs with no closed form go to the subdivision engine; what it charges is a deterministic
    // function of the input, so each count pins the order it cuts boxes in, which boxes its range
    // and clipping tests drop, and how far it refines each zero.
    let work = |a: &CurveKind<2>, b: &CurveKind<2>| {
        let mut m = meter();
        let r = curves(a, b, &mut m).unwrap();
        (r.items().len(), m.used().work)
    };
    let shifted = |points: &[[f64; 2]]| {
        let p = points.len() - 1;
        let mut knots = vec![1.0; p + 1];
        knots.extend(vec![2.0; p + 1]);
        CurveKind::Nurbs(Curve::new(Basis::new(p, knots).unwrap(), points).unwrap())
    };
    let wave = [[0.0, 0.0], [1.0, 3.0], [2.0, -2.0], [3.0, 1.0]];
    let other = [[0.0, 1.0], [1.0, -1.0], [2.0, 2.0], [3.0, 0.0]];
    let two = CurveKind::Nurbs(
        Curve::new(
            Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]).unwrap(),
            &[
                [0.0, -1000.0],
                [1000.0, 1000.0],
                [2000.0, -1000.0],
                [3000.0, 1000.0],
            ],
        )
        .unwrap(),
    );
    let axis = bezier(&[[-1000.0, 0.0], [1500.0, 0.0], [4000.0, 0.0]]);
    let got = [
        work(&bezier(&wave), &bezier(&other)),
        work(&shifted(&wave), &bezier(&other)),
        work(&bezier(&wave), &shifted(&other)),
        work(
            &bezier(&[[0.0, 0.0], [1.0, 2.0], [2.0, 0.0]]),
            &bezier(&[[0.0, 0.9], [2.0, 0.9]]),
        ),
        work(
            &bezier(&[[0.0, 0.0], [1.0, 2.0], [2.0, 0.0]]),
            &bezier(&[[0.0, 1.01], [2.0, 1.01]]),
        ),
        work(&two, &axis),
    ];
    assert_eq!(
        got,
        [
            (3, 3589),
            (3, 3589),
            (3, 3589),
            (2, 507),
            (0, 254),
            (3, 3880)
        ]
    );
}

#[test]
fn conics_in_a_leaning_plane_meet_curves_in_it_as_crossings() {
    // The plane z = x, through whole numbers: the shared plane is proven from the conics' centres
    // and axes, so a meeting needs no third coordinate and is a crossing, never a near miss.
    let axes = || Axes::Vectors {
        u: [2.0, 0.0, 2.0],
        v: [0.0, 1.0, 0.0],
    };
    let across = bezier(&[[-3.0, 2.0, -3.0], [1.0, 2.0, 1.0], [5.0, 2.0, 5.0]]);
    let up = bezier(&[[2.0, -3.0, 2.0], [2.0, 2.0, 2.0], [2.0, 7.0, 2.0]]);
    let kinds: [CurveKind<3>; 3] = [
        CurveKind::Ellipse(Ellipse::new([1.0, 2.0, 1.0], axes(), (0.0, 2.0)).unwrap()),
        CurveKind::Hyperbola(Hyperbola::new([1.0, 2.0, 1.0], axes(), (-2.0, 2.0)).unwrap()),
        CurveKind::Parabola(Parabola::new([1.0, 2.0, 1.0], axes(), (-2.0, 2.0)).unwrap()),
    ];
    let mut counts = Vec::new();
    for k in &kinds {
        for b in [&across, &up] {
            let r = meet(k, b);
            assert!(
                r.items().iter().all(|i| matches!(
                    i,
                    Item::Point {
                        contact: PointContact::Crossing,
                        ..
                    }
                )),
                "{:?}",
                r.items()
            );
            counts.push(r.items().len());
        }
    }
    // The hyperbola's branch starts at x = 3, so the line x = 2 misses it.
    assert_eq!(counts, [2, 2, 1, 0, 1, 2]);
}

#[test]
fn curves_whose_shadows_cross_a_unit_apart_do_not_meet() {
    // Not in one plane, so a meeting of two coordinates counts only if the third agrees: they are
    // a unit apart in z, either way round.
    let high = bezier(&[[0.0, 0.0, 1.0], [1.0, 1.0, 1.0], [2.0, 2.0, 1.0]]);
    let low = bezier(&[[0.0, 2.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]]);
    is_empty(&meet(&high, &low));
    is_empty(&meet(&low, &high));
}

#[test]
fn a_rational_curve_is_not_in_the_plane_of_its_weighted_points() {
    // Weights 1, 2, 1 on points at heights 2, 1, 2 put every stored w·P at height 2, but the curve
    // is at 2 / (1 + B1(t)): 4/3 halfway, where its shadow crosses x = 1. A curve at height 2
    // through that shadow point is 2/3 above it, so the two do not meet, either way round.
    let a = CurveKind::Nurbs(
        Curve::new_rational(
            basis(2),
            &[[0.0, 0.0, 2.0], [1.0, 2.0, 1.0], [2.0, 0.0, 2.0]],
            &[1.0, 2.0, 1.0],
        )
        .unwrap(),
    );
    let b = bezier(&[[1.0, 0.0, 2.0], [1.0, 1.0, 2.0], [1.0, 3.0, 2.0]]);
    is_empty(&meet(&a, &b));
    is_empty(&meet(&b, &a));
}

#[test]
fn a_conic_near_the_grid_s_reach_keeps_its_plane() {
    // In the plane z = x again, centred three quarters of the grid's reach out: the centre moved
    // along each axis stays on the grid (moved back, it would leave it), so the plane is proven and
    // the meeting with a line in it is a crossing.
    let k = 0.75 * 2.0_f64.powi(40);
    let half = 2.0_f64.powi(39);
    let axes = Axes::Vectors {
        u: [-half, 0.0, -half],
        v: [0.0, half, 0.0],
    };
    let e = CurveKind::Ellipse(Ellipse::new([k, -k, k], axes, (0.0, 2.0)).unwrap());
    let reach = 2.0_f64.powi(40) - 1.0;
    let line = bezier(&[[k, -reach, k], [k, -k, k], [k, 0.0, k]]);
    let r = meet(&e, &line);
    assert!(
        matches!(
            r.items(),
            [Item::Point {
                contact: PointContact::Crossing,
                ..
            }]
        ),
        "{:?}",
        r.items()
    );
    // At a quarter turn: the centre plus v.
    assert_eq!(points(&r).len(), 1);
    assert!((points(&r)[0].0 - 0.5).abs() < 1e-9);
}

#[test]
fn a_line_all_but_tangent_to_a_circle_is_answered() {
    // The line through (k, 0) and (2k, 1) passes within about 1 / 2k² of the unit circle's edge,
    // near (0, -1), and meets it twice: the exact count says so, while the discriminant, exactly 1
    // against terms near k⁴, has an enclosure reaching far below zero. The closed form must take
    // the root of what is not negative and leave the pair to the engine, never fail.
    for e in [20, 26] {
        let k = 2.0_f64.powi(e);
        let unit = circle([0.0, 0.0], 1.0);
        is_empty(&meet(&segment([k, 0.0], [2.0 * k, 1.0]), &unit));
        let r = meet(&infinite([k, 0.0], [k, 1.0]), &unit);
        assert_eq!(r.items().len(), 2, "{:?}", r.items());
        assert!(
            r.items()
                .iter()
                .all(|i| (i.at().spans()[0].lo() + 1.0).abs() < 1e-6),
            "{:?}",
            r.items()
        );
    }
}

#[test]
fn an_overlap_far_along_its_parameter_is_flagged_within_budget() {
    // One curve runs on parameters near 2^40, where a double's step is 2^-12, and covers 16,384
    // grid steps a unit: a side one step of a double wide still reaches 4 grid steps, past the
    // floor. Such a side cannot be halved, so its box is flagged there; splitting it into itself
    // would go round for ever. The two lines are one, so the answer is one flagged run.
    let t = 2.0_f64.powi(40);
    let far = CurveKind::Nurbs(
        Curve::new(
            Basis::new(1, vec![t, t, t + 1.0, t + 1.0]).unwrap(),
            &[[0.0, 0.0], [16384.0, 0.0]],
        )
        .unwrap(),
    );
    let near = bezier(&[[0.0, 0.0], [16384.0, 0.0]]);
    let r = curves(&far, &near, &mut WorkMeter::new(Budget::of_work(4_000_000))).unwrap();
    assert!(
        matches!(
            r.items(),
            [Item::Flagged {
                why: Unresolved::HigherOrderContact,
                ..
            }]
        ),
        "{:?}",
        r.items()
    );
    let s = r.items()[0].at().spans();
    assert_eq!(
        (s[0].lo(), s[0].hi(), s[1].lo(), s[1].hi()),
        (t, t + 1.0, 0.0, 1.0)
    );
}

/// Each point item's contact and the middle of its first parameter, sorted by it.
fn touches(r: &Intersection) -> Vec<(PointContact, f64)> {
    let mut out: Vec<(PointContact, f64)> = r
        .items()
        .iter()
        .map(|i| match i {
            Item::Point { contact, at, .. } => {
                let s = at.spans()[0];
                (*contact, s.lo() * 0.5 + s.hi() * 0.5)
            }
            other => panic!("not a point: {other:?}"),
        })
        .collect();
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    out
}

/// `got`'s contacts are `want`'s, each parameter within 2^-30.
fn touches_are(got: &[(PointContact, f64)], want: &[(PointContact, f64)]) {
    assert_eq!(got.len(), want.len(), "{got:?}");
    for ((c, s), (d, t)) in got.iter().zip(want) {
        assert!(c == d && (s - t).abs() < 2.0_f64.powi(-30), "{got:?}");
    }
}

#[test]
fn a_closed_curve_s_join_is_no_boundary_and_an_open_end_is() {
    use PointContact::{BoundaryTouch, Crossing};
    let x_axis = infinite([0.0, 0.0], [1.0, 0.0]);
    // A whole circle or ellipse is joined at its start: the x axis crosses it there.
    touches_are(
        &touches(&meet(&x_axis, &circle([0.0, 0.0], 5.0))),
        &[(Crossing, -5.0), (Crossing, 5.0)],
    );
    touches_are(
        &touches(&meet(&x_axis, &ellipse([0.0, 0.0], 4.0, 2.0))),
        &[(Crossing, -4.0), (Crossing, 4.0)],
    );
    // An arc ends there, and a segment ends where it meets the circle.
    touches_are(
        &touches(&meet(&x_axis, &arc([0.0, 0.0], 5.0, (0.0, 1.0)))),
        &[(BoundaryTouch, -5.0), (BoundaryTouch, 5.0)],
    );
    touches_are(
        &touches(&meet(
            &segment([0.0, 0.0], [0.0, 5.0]),
            &circle([0.0, 0.0], 5.0),
        )),
        &[(BoundaryTouch, 1.0)],
    );
    // Two circles: the one about (5, 5) crosses the one about the origin at (5, 0), its join or an
    // arc's end, and at (0, 5).
    let other = circle([5.0, 5.0], 5.0);
    touches_are(
        &touches(&meet(&circle([0.0, 0.0], 5.0), &other)),
        &[(Crossing, 0.0), (Crossing, 0.5)],
    );
    touches_are(
        &touches(&meet(&arc([0.0, 0.0], 5.0, (0.0, 1.0)), &other)),
        &[(BoundaryTouch, 0.0), (Crossing, 0.5)],
    );
}

#[test]
fn a_line_meets_a_hyperbola_only_on_its_arc() {
    // The branch (cosh t, sinh t) for t in [-1, 1] reaches y = ±1.18: y = ±3 meets the branch only
    // at t = ±1.82, outside the arc, and y = 1 meets it inside.
    let axes = Axes::Vectors {
        u: [1.0, 0.0],
        v: [0.0, 1.0],
    };
    let h = CurveKind::Hyperbola(Hyperbola::new([0.0, 0.0], axes, (-1.0, 1.0)).unwrap());
    is_empty(&meet(&infinite([0.0, 3.0], [1.0, 0.0]), &h));
    is_empty(&meet(&infinite([0.0, -3.0], [1.0, 0.0]), &h));
    let r = meet(&infinite([0.0, 1.0], [1.0, 0.0]), &h);
    touches_are(&touches(&r), &[(PointContact::Crossing, 2.0_f64.sqrt())]);
}

/// A closed cubic B-spline on a figure eight's polygon, a thousand grid steps to a unit, starting
/// and ending at the right lobe's tip: it crosses itself once, at the waist, away from its join.
fn closed_eight() -> CurveKind<2> {
    let polygon: Vec<[f64; 2]> = [
        [10.0, 0.0],
        [10.0, 6.0],
        [5.0, 6.0],
        [-5.0, -6.0],
        [-10.0, -6.0],
        [-10.0, 6.0],
        [-5.0, 6.0],
        [5.0, -6.0],
        [10.0, -6.0],
        [10.0, 0.0],
    ]
    .iter()
    .map(|[x, y]| [x * 1000.0, y * 1000.0])
    .collect();
    let knots = vec![
        0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 7.0, 7.0, 7.0,
    ];
    CurveKind::Nurbs(Curve::new(Basis::new(3, knots).unwrap(), &polygon).unwrap())
}

#[test]
fn a_closed_curve_crossing_itself_away_from_its_join_is_found() {
    // Sampled 20,000 times, the curve's only crossing is at parameters 1.5 and 5.5.
    let r = self_crossings(&closed_eight(), &mut meter()).unwrap();
    assert!(
        matches!(
            r.items(),
            [Item::Point {
                contact: PointContact::Crossing,
                ..
            }]
        ),
        "{:?}",
        r.items()
    );
    let [s, t] = [r.items()[0].at().spans()[0], r.items()[0].at().spans()[1]];
    assert!(s.contains(1.5) && t.contains(5.5), "{s:?} {t:?}");
}

#[test]
fn a_crossing_through_a_closed_curve_s_join_is_kept_flagged() {
    // The join is the waist: the curve passes it again halfway. The engine cannot prove a zero
    // at the end of its domain, so both pairs are flagged; neither is the join itself.
    let eight = bezier(&[
        [0.0, 0.0],
        [12000.0, 12000.0],
        [12000.0, -12000.0],
        [-12000.0, 12000.0],
        [-12000.0, -12000.0],
        [0.0, 0.0],
    ]);
    let r = self_crossings(&eight, &mut meter()).unwrap();
    let at: Vec<(f64, f64)> = r
        .items()
        .iter()
        .map(|i| {
            assert!(matches!(i, Item::Flagged { .. }), "{i:?}");
            let s = i.at().spans();
            (
                s[0].lo() * 0.5 + s[0].hi() * 0.5,
                s[1].lo() * 0.5 + s[1].hi() * 0.5,
            )
        })
        .collect();
    assert_eq!(at.len(), 2, "{:?}", r.items());
    assert!(
        (at[0].0 - 0.0).abs() < 1e-3 && (at[0].1 - 0.5).abs() < 1e-3,
        "{at:?}"
    );
    assert!(
        (at[1].0 - 0.5).abs() < 1e-3 && (at[1].1 - 1.0).abs() < 1e-3,
        "{at:?}"
    );
}

#[test]
fn self_crossings_take_only_their_work() {
    // As for pairs: what the search charges pins which boxes its tests prune, the join's among them.
    let work = |c: &CurveKind<2>| {
        let mut m = meter();
        let r = self_crossings(c, &mut m).unwrap();
        (r.items().len(), m.used().work)
    };
    let got = [
        work(&closed_eight()),
        work(&bezier(&[
            [0.0, 0.0],
            [3000.0, 3000.0],
            [-1000.0, 3000.0],
            [2000.0, 0.0],
        ])),
        work(&bezier(&[
            [0.0, 0.0],
            [12000.0, 2000.0],
            [-4000.0, 8000.0],
            [4000.0, -4000.0],
        ])),
    ];
    assert_eq!(got, [(1, 39450), (1, 8789), (1, 6774)]);
}

#[test]
fn a_tangency_s_band_is_set_by_the_curvatures_difference() {
    // Each pair touches at (1000, 1000), where the hump has curvature -1/500 and the dip +1/500,
    // and the wide hump -1/1000: 12.9's band √(2δ/Δκ), δ one step, is √500 = 22.4 and √2000 = 44.7,
    // rounded up. Run the other way round, a curve's curvature changes sign with its tangent, so
    // the band is the same; so it is with the pair swapped.
    let hump = bezier(&[[0.0, 0.0], [1000.0, 2000.0], [2000.0, 0.0]]);
    let dip = bezier(&[[0.0, 2000.0], [1000.0, 0.0], [2000.0, 2000.0]]);
    let pid = bezier(&[[2000.0, 2000.0], [1000.0, 0.0], [0.0, 2000.0]]);
    let wide = bezier(&[[-1000.0, -1000.0], [1000.0, 3000.0], [3000.0, -1000.0]]);
    let band = |a: &CurveKind<2>, b: &CurveKind<2>| match meet(a, b).items() {
        [
            Item::Flagged {
                why: Unresolved::TangentBand { width },
                ..
            },
        ] => width.0,
        other => panic!("{other:?}"),
    };
    let got = [
        band(&hump, &dip),
        band(&hump, &pid),
        band(&dip, &hump),
        band(&pid, &hump),
        band(&hump, &wide),
        band(&wide, &hump),
    ];
    assert_eq!(got, [23, 23, 23, 23, 45, 45]);
}

#[test]
fn a_tangency_too_sharp_to_read_at_one_step_is_higher_order() {
    // Both curves are bent to about a grid step's radius where they touch, at (1000, 500000), the
    // hump at curvature -1 and the wider one at -0.8 or -0.95: across the run of boxes left at the
    // floor each tangent turns through about a quarter, so the derivative bounds cannot say whether
    // the tangents agree or oppose, and no difference of curvatures can be read. The second order
    // test gives up rather than guess a band (for the first, √(2/0.2) is 3.2 steps; a guess gave 2).
    let hump = bezier(&[[0.0, 0.0], [1000.0, 1.0e6], [2000.0, 0.0]]);
    for (y0, y1) in [(-1.1e6, 2.1e6), (-1.4e6, 2.4e6)] {
        let wider = bezier(&[[-1000.0, y0], [1000.0, y1], [3000.0, y0]]);
        let r = meet(&hump, &wider);
        assert!(
            matches!(
                r.items(),
                [Item::Flagged {
                    why: Unresolved::HigherOrderContact,
                    ..
                }]
            ),
            "{:?}",
            r.items()
        );
    }
}

#[test]
fn parallel_lines_in_space_apart_stay_apart_in_the_plane_they_are_read_in() {
    // Two parallel lines are read in the plane across picks from n = d × w, w the step between
    // their origins. In these pairs, chosen by a search over whole d and w in [-3, 3]³, the plane
    // must not drop y or x: along it the two lines would be one.
    for (d, w) in [
        ([-3.0, -3.0, 1.0], [-3.0, -1.0, 1.0]),
        ([-3.0, -3.0, 2.0], [-3.0, -2.0, 2.0]),
        ([-3.0, -2.0, 0.0], [-1.0, -1.0, 0.0]),
        ([-3.0, -2.0, 3.0], [-2.0, -2.0, 2.0]),
        ([-3.0, -3.0, 1.0], [-1.0, -3.0, 1.0]),
    ] {
        is_empty(&meet(&infinite([0.0; 3], d), &infinite(w, d)));
        let far = |o: [f64; 3]| [o[0] + 5.0 * d[0], o[1] + 5.0 * d[1], o[2] + 5.0 * d[2]];
        is_empty(&meet(
            &segment([0.0; 3], far([0.0; 3])),
            &segment(w, far(w)),
        ));
    }
}

#[test]
fn circles_off_the_axes_are_compared_by_the_distance_of_their_centres() {
    // (3, 4) and (9, 12) are 10 apart: radii 5 and 5 touch once, 5 and 4 miss, 5 and 6 cross. Every
    // coordinate is nonzero, so each cross term of |c - e|² counts.
    let a = circle([3.0, 4.0], 5.0);
    let touch = meet(&a, &circle([9.0, 12.0], 5.0));
    assert!(
        matches!(
            touch.items(),
            [Item::Point {
                contact: PointContact::IsolatedTangent,
                ..
            }]
        ),
        "{:?}",
        touch.items()
    );
    is_empty(&meet(&a, &circle([9.0, 12.0], 4.0)));
    assert_eq!(meet(&a, &circle([9.0, 12.0], 6.0)).items().len(), 2);
}

#[test]
fn a_crossing_on_a_whole_circle_s_join_is_found_wherever_its_turn_starts() {
    // A whole turn of radius 5000 from 0 or from 1 (half-turns), against lines through its
    // join, through the point half a turn on, and one slanting through the join from 1. Each
    // crossing is found once, on the domain, either way round.
    let turn = |from: f64| {
        let frame = Frame::standard().unwrap();
        CurveKind::Circle(Circle::new([0.0, 0.0], 5000.0, frame, (from, from + 2.0)).unwrap())
    };
    let east = bezier(&[[0.0, 0.0], [5000.0, 0.0], [10000.0, 0.0]]);
    let west = bezier(&[[0.0, 0.0], [-5000.0, 0.0], [-10000.0, 0.0]]);
    let slant = bezier(&[[-6000.0, -1000.0], [-5000.0, 0.0], [-4000.0, 1000.0]]);
    for (a, b, s, t) in [
        (turn(0.0), east.clone(), 0.0, 0.5),
        (turn(0.0), west.clone(), 1.0, 0.5),
        (turn(1.0), west.clone(), 1.0, 0.5),
        (turn(1.0), east.clone(), 2.0, 0.5),
        (turn(1.0), slant.clone(), 1.0, 0.5),
        (east.clone(), turn(0.0), 0.5, 0.0),
        (west.clone(), turn(1.0), 0.5, 1.0),
    ] {
        let r = meet(&a, &b);
        assert!(
            matches!(
                r.items(),
                [Item::Point {
                    contact: PointContact::Crossing,
                    ..
                }]
            ),
            "{:?}",
            r.items()
        );
        let at = r.items()[0].at().spans();
        let (ms, mt) = (
            at[0].lo() * 0.5 + at[0].hi() * 0.5,
            at[1].lo() * 0.5 + at[1].hi() * 0.5,
        );
        assert!((ms - s).abs() < 1e-4 && (mt - t).abs() < 1e-4, "{at:?}");
    }
    // Crossings within the search's pad of the join (a 1024th of a turn), just before it and just
    // after: each found once, on the domain, where it is.
    let near = 5000.0 * (core::f64::consts::PI / 4096.0).sin();
    for (y, s) in [(-near, 2.0 - 1.0 / 4096.0), (near, 1.0 / 4096.0)] {
        let r = meet(
            &turn(0.0),
            &bezier(&[[4000.0, y], [5000.0, y], [6000.0, y]]),
        );
        assert_eq!(r.items().len(), 1, "{:?}", r.items());
        let at = r.items()[0].at().spans()[0];
        assert!((at.lo() * 0.5 + at.hi() * 0.5 - s).abs() < 1e-9, "{at:?}");
    }
    // A line all but tangent at the join crosses on both sides of it, within the pad: two
    // crossings, never one, whichever side's boxes hold them.
    let x = 5000.0 * (core::f64::consts::PI / 8192.0).cos();
    let r = meet(&turn(0.0), &bezier(&[[x, -100.0], [x, 0.0], [x, 100.0]]));
    let mut at: Vec<f64> = r
        .items()
        .iter()
        .map(|i| {
            let s = i.at().spans()[0];
            s.lo() * 0.5 + s.hi() * 0.5
        })
        .collect();
    at.sort_by(f64::total_cmp);
    assert_eq!(at.len(), 2, "{:?}", r.items());
    assert!(
        (at[0] - 1.0 / 8192.0).abs() < 1e-9 && (at[1] - (2.0 - 1.0 / 8192.0)).abs() < 1e-9,
        "{at:?}"
    );
    // An arc from a quarter to three quarters of a turn, whose ends add to 2, is no whole turn:
    // the y axis touches it at both ends.
    let r = meet(
        &infinite([0.0, 0.0], [0.0, 1.0]),
        &arc([0.0, 0.0], 5.0, (0.5, 1.5)),
    );
    assert!(
        r.items().iter().all(|i| matches!(
            i,
            Item::Point {
                contact: PointContact::BoundaryTouch,
                ..
            }
        )) && r.items().len() == 2,
        "{:?}",
        r.items()
    );
}

#[test]
fn a_meeting_at_the_grid_s_edge_is_on_the_grid() {
    // The lines cross at (2^40 - 2, 0): the grid box around the crossing, rounded outward, reaches
    // 2^40 - 1, the last grid coordinate, and is a box of the grid, not out of its range.
    let reach = 2.0_f64.powi(40) - 1.0;
    let x = reach - 1.0;
    let r = meet(
        &segment([x, -1.0], [x, 1.0]),
        &segment([0.0, 0.0], [reach, 0.0]),
    );
    assert!(
        matches!(
            r.items(),
            [Item::Point {
                contact: PointContact::Crossing,
                ..
            }]
        ),
        "{:?}",
        r.items()
    );
}

#[test]
fn a_whole_circle_s_search_takes_only_its_work() {
    // What the engine charges on a whole circle pins where its search runs past the join and how
    // a box across the join is read: the pad either side, and the stretch moved by a turn.
    let turn = CurveKind::Circle(
        Circle::new([0.0, 0.0], 5000.0, Frame::standard().unwrap(), (0.0, 2.0)).unwrap(),
    );
    let work = |b: &CurveKind<2>| {
        let mut m = meter();
        let r = curves(&turn, b, &mut m).unwrap();
        (r.items().len(), m.used().work)
    };
    let x = 5000.0 * (core::f64::consts::PI / 8192.0).cos();
    let near = 5000.0 * (core::f64::consts::PI / 4096.0).sin();
    let got = [
        work(&bezier(&[[x, -100.0], [x, 0.0], [x, 100.0]])),
        work(&bezier(&[
            [4000.0, -near],
            [5000.0, -near],
            [6000.0, -near],
        ])),
        work(&bezier(&[[4000.0, near], [5000.0, near], [6000.0, near]])),
        work(&bezier(&[[0.0, 0.0], [5000.0, 0.0], [10000.0, 0.0]])),
    ];
    assert_eq!(got, [(2, 707_831), (1, 1263), (1, 1263), (1, 1313)]);
}

#[test]
fn circles_on_later_turns_cross_on_their_own_domains() {
    // The circle about (6, 0) on whole turns from 8 or from -6 (half-turns): its two crossings with
    // the circle about the origin are at the same points, at parameters moved by those turns.
    let on = |d: (f64, f64)| {
        CurveKind::Circle(Circle::new([6.0, 0.0], 5.0, Frame::standard().unwrap(), d).unwrap())
    };
    let first = 0.704_832_764_699_133;
    // From 9 the first crossing falls in the next turn: 9.295 and 10.705.
    for (d, want) in [
        ((8.0, 10.0), [8.0 + first, 10.0 - first]),
        ((-6.0, -4.0), [-6.0 + first, -4.0 - first]),
        ((9.0, 11.0), [10.0 - first, 10.0 + first]),
    ] {
        let r = meet(&circle([0.0, 0.0], 5.0), &on(d));
        let mut t: Vec<f64> = r
            .items()
            .iter()
            .map(|i| {
                let s = i.at().spans()[1];
                s.lo() * 0.5 + s.hi() * 0.5
            })
            .collect();
        t.sort_by(f64::total_cmp);
        assert_eq!(t.len(), 2, "{:?}", r.items());
        assert!(
            (t[0] - want[0]).abs() < 1e-9 && (t[1] - want[1]).abs() < 1e-9,
            "{t:?}"
        );
    }
}

#[test]
fn a_curve_of_two_spans_crossing_in_each_keeps_every_crossing() {
    // A quadratic B-spline of two spans zigzags across the x axis three times: in its first span,
    // at the knot between them and in its second. A box over both spans is clipped by neither.
    let two = CurveKind::Nurbs(
        Curve::new(
            Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]).unwrap(),
            &[
                [0.0, -1000.0],
                [1000.0, 1000.0],
                [2000.0, -1000.0],
                [3000.0, 1000.0],
            ],
        )
        .unwrap(),
    );
    let r = meet(
        &two,
        &bezier(&[[-1000.0, 0.0], [1500.0, 0.0], [4000.0, 0.0]]),
    );
    let mut s: Vec<f64> = r
        .items()
        .iter()
        .map(|i| {
            let s = i.at().spans()[0];
            s.lo() * 0.5 + s.hi() * 0.5
        })
        .collect();
    s.sort_by(f64::total_cmp);
    assert_eq!(s.len(), 3, "{:?}", r.items());
    for (got, want) in s.iter().zip([1.0 / 3.0, 1.0, 5.0 / 3.0]) {
        assert!((got - want).abs() < 1e-4, "{s:?}");
    }
}

#[test]
fn a_conic_in_the_plane_z_equals_y_meets_a_line_in_it_as_crossings() {
    // The plane z = y fixes y, so the corner the shared plane is proven from must keep its y.
    let axes = Axes::Vectors {
        u: [2.0, 1.0, 1.0],
        v: [0.0, 1.0, 1.0],
    };
    let e = CurveKind::Ellipse(Ellipse::new([1.0, 2.0, 2.0], axes, (0.0, 2.0)).unwrap());
    let line = bezier(&[[-3.0, 2.0, 2.0], [1.0, 2.0, 2.0], [5.0, 2.0, 2.0]]);
    let r = meet(&e, &line);
    assert_eq!(r.items().len(), 2, "{:?}", r.items());
    assert!(
        r.items().iter().all(|i| matches!(
            i,
            Item::Point {
                contact: PointContact::Crossing,
                ..
            }
        )),
        "{:?}",
        r.items()
    );
}

#[test]
fn curves_apart_in_space_whose_shadows_meet_do_not_meet() {
    // A line along (1, 1, 1) and a curve at height -10 whose shadow on z = 0 crosses the line's
    // at (2, 2): 12 apart there. A line along (0, 1, 1) and a curve 5 off it in x whose end lies on
    // its shadow on x = 0: a run at the end, apart in x. Neither meets, either way round.
    for (a, b) in [
        (
            infinite([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            bezier(&[[0.0, 4.0, -10.0], [2.0, 2.0, -10.0], [4.0, 0.0, -10.0]]),
        ),
        (
            infinite([0.0, 0.0, 0.0], [0.0, 1.0, 1.0]),
            bezier(&[[5.0, -5.0, 5.0], [5.0, 0.0, 5.0], [5.0, 5.0, 5.0]]),
        ),
    ] {
        is_empty(&meet(&a, &b));
        is_empty(&meet(&b, &a));
    }
}

#[test]
fn a_pair_s_route_is_on_the_record_only_when_a_closed_form_could_not_decide() {
    // 12.7: a pair with a closed form that sent it on to the engine says so; a pair the engine
    // takes from the start, or one a closed form decides, does not.
    let routed = |a: &CurveKind<2>, b: &CurveKind<2>| {
        meet(a, b)
            .diagnostics()
            .iter()
            .any(|d| d.code == "closed-form-to-engine")
    };
    let k = 2.0_f64.powi(20);
    assert!(routed(
        &infinite([k, 0.0], [k, 1.0]),
        &circle([0.0, 0.0], 1.0)
    ));
    let hump = bezier(&[[0.0, -1.0], [2.0, 3.0], [4.0, -1.0]]);
    let level = infinite([0.0, 0.0], [1.0, 0.0]);
    let off_grid = [
        segment([0.5, 0.0], [2.5, 2.0]),
        segment([0.5, 2.0], [2.5, 0.0]),
    ];
    for (a, b) in [
        (&hump, &level),
        (&level, &hump),
        (&circle([2.0, 0.0], 1.0), &hump),
        (&off_grid[0], &off_grid[1]),
    ] {
        assert!(!routed(a, b));
        assert_ne!(meet(a, b).items().len(), 0);
    }
}

#[test]
fn circles_in_space_are_read_in_their_own_plane() {
    let frame = |x: [f64; 3], n: [f64; 3]| Frame::new(x, n).unwrap();
    let ring = |c: [f64; 3], r: f64, f: Frame<3>| CurveKind::Circle(Circle::full(c, r, f).unwrap());
    // Two circles of radius 5 about the origin, one flat and one standing in the plane y = 0: they
    // cross at (5, 0, 0) and (-5, 0, 0), though their centres are level in z.
    let flat = ring([0.0; 3], 5.0, frame([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
    let standing = ring([0.0; 3], 5.0, frame([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
    let r = meet(&flat, &standing);
    assert_eq!(r.items().len(), 2, "{:?}", r.items());
    // Two in the plane y = 1, centres 6 apart: they cross twice there.
    let a = ring(
        [0.0, 1.0, 0.0],
        5.0,
        frame([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    );
    let b = ring(
        [6.0, 1.0, 0.0],
        5.0,
        frame([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    );
    let r = meet(&a, &b);
    assert!(
        r.items().len() == 2
            && r.items().iter().all(|i| matches!(
                i,
                Item::Point {
                    contact: PointContact::Crossing,
                    ..
                }
            )),
        "{:?}",
        r.items()
    );
}

#[test]
fn curves_in_a_plane_off_the_grid_are_read_in_that_plane() {
    // Both in the plane x = 0.5, off the grid, so no plane is proven: the engine reads them in the
    // two coordinates they spread over, y and z, never along x, where they are a line.
    let hump = bezier(&[[0.5, 0.0, 0.0], [0.5, 0.5, 8.0], [0.5, 1.0, 0.0]]);
    let level = bezier(&[[0.5, 0.0, 2.0], [0.5, 0.5, 2.0], [0.5, 1.0, 2.0]]);
    // Equal in x but off the grid, so each meeting is a near miss in x the doubles cannot rule
    // out, at s = t = (1 ± √½) / 2: the level line runs with the hump in y.
    let r = meet(&hump, &level);
    let mut s: Vec<(f64, f64)> = r
        .items()
        .iter()
        .map(|i| {
            let [s, t] = [i.at().spans()[0], i.at().spans()[1]];
            (s.lo() * 0.5 + s.hi() * 0.5, t.lo() * 0.5 + t.hi() * 0.5)
        })
        .collect();
    s.sort_by(|a, b| a.0.total_cmp(&b.0));
    let half = 0.5_f64.sqrt();
    assert_eq!(s.len(), 2, "{:?}", r.items());
    assert!(
        [s[0].0, s[0].1, s[1].0, s[1].1]
            .iter()
            .zip([1.0 - half, 1.0 - half, 1.0 + half, 1.0 + half])
            .all(|(got, want)| (got - want / 2.0).abs() < 0.01),
        "{s:?}"
    );
}

#[test]
fn a_ray_meets_what_lies_ahead_of_it() {
    // A ray from the origin along x and a hump crossing it at x = 1 and x = 3: the engine cuts the
    // ray to the grid's reach, as it does a whole line.
    let ray = CurveKind::Line(Line::ray([0.0, 0.0], [1.0, 0.0]).unwrap());
    let hump = bezier(&[[1.0, -1.0], [2.0, 3.0], [3.0, -1.0]]);
    assert_eq!(meet(&ray, &hump).items().len(), 2);
    assert_eq!(meet(&hump, &ray).items().len(), 2);
}
