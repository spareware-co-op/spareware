//! The fit's guards and bounds read exactly (G175, chapter 12.12): a ceiling that is not positive
//! and a branch of one vertex are refused before any work; a surface that moves fast is still
//! bounded; a settled point outside a surface's domain is never fit data; a refinement near an edge
//! starts from the middle of its data; a true point is proven in a wide box, and on a circle it is
//! the near one of two; and the bounds and true points of two planes (near the origin and far out)
//! and of the sheet and plane of `fit_refined` are the recorded bits, so any change to their
//! arithmetic is seen.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::panic,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{IntersectionCurve, ParamCurve, SurfaceRef};
use geom_intersect::IntersectError;
use geom_intersect::fit::{fit, true_point};
use geom_intersect::march::Branch;
use geom_intersect::result::CurveEnd;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};

const CEILING: f64 = 1.0 / 256.0;
const REFS: [SurfaceRef; 2] = [SurfaceRef(1), SurfaceRef(2)];
const K: f64 = 0.5;
const C: f64 = 0.125;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

/// The plane through `origin` along `u` and `v` over `domain`.
fn plane(
    origin: [f64; 3],
    u: [f64; 3],
    v: [f64; 3],
    domain: ((f64, f64), (f64, f64)),
) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(origin, u, v, domain).unwrap())
}

/// z = v − k·u² over [−1, 1]², its parameters x and y.
fn sheet(k: f64) -> SurfaceKind {
    let along = Basis::new(2, vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]).unwrap();
    let across = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for (x, square) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
        for y in [-1.0, 1.0] {
            net.push([x, y, y - k * square]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap())
}

/// The plane z = `c` as a bilinear patch over [−1, 1]² that moves four times as fast as its
/// parameters: (4u, 4v, c), so σ = |n|/√(E + G) = 16/√32, more than 2.
fn fast(c: f64) -> SurfaceKind {
    let line = || Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for x in [-4.0, 4.0] {
        for y in [-4.0, 4.0] {
            net.push([x, y, c]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(line(), line(), &net).unwrap())
}

fn branch(vertices: Vec<[f64; 4]>) -> Branch {
    Branch {
        vertices: vertices
            .into_iter()
            .map(|x| x.map(|q| Interval::point(q).unwrap()))
            .collect(),
        ends: [CurveEnd::Boundary, CurveEnd::Boundary],
        deviation: 0.0,
        passes: Vec::new(),
    }
}

/// The planes z = 0 (parameters x, y) and y = 0 (parameters x, z), which meet along the x axis.
fn crossing_planes() -> (SurfaceKind, SurfaceKind) {
    crossing_planes_over((-4.0, 4.0))
}

/// [`crossing_planes`] with x over `xs`.
fn crossing_planes_over(xs: (f64, f64)) -> (SurfaceKind, SurfaceKind) {
    let whole = (xs, (-4.0, 4.0));
    (
        plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], whole),
        plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], whole),
    )
}

#[test]
fn a_ceiling_that_is_not_positive_or_a_branch_of_one_vertex_is_refused() {
    // Refused before any work: under a meter with no work at all the answer is still the refusal,
    // never the meter's stop, so it is the guard's and not a later step's.
    let none = || WorkMeter::new(Budget::of_work(0));
    let (a, b) = crossing_planes();
    let two = branch(vec![[-1.0, 0.0, -1.0, 0.0], [2.0, 0.0, 2.0, 0.0]]);
    let one = branch(vec![[-1.0, 0.0, -1.0, 0.0]]);
    for ceiling in [f64::NAN, 0.0, -0.0, -1.0, f64::NEG_INFINITY] {
        assert_eq!(
            fit((&a, &b), REFS, &two, ceiling, &mut none()).err(),
            Some(IntersectError::InvalidInput),
            "{ceiling}"
        );
    }
    assert_eq!(
        fit((&a, &b), REFS, &one, CEILING, &mut none()).err(),
        Some(IntersectError::InvalidInput)
    );
    assert_eq!(
        fit((&a, &b), REFS, &two, CEILING, &mut none()).err(),
        Some(IntersectError::BudgetExceeded)
    );
    // The same two vertices under a positive ceiling are a curve: the refusals are the guard's.
    assert!(fit((&a, &b), REFS, &two, CEILING, &mut meter()).is_ok());
}

#[test]
fn two_planes_meeting_in_a_line_have_the_recorded_bounds() {
    // The cache and the pcurves are the line itself, up to the enclosures' rounding, and the
    // points reach x = 2, past 1, where a step of the double is counted from the point's size.
    // Far out, from x = 1000 to 1003, the bounds are rounding only, at the step of the doubles
    // there.
    for (xs, ends, want) in [
        ((-4.0, 4.0), [-1.0, 2.0], PLANES),
        ((990.0, 1010.0), [1000.0, 1003.0], PLANES_FAR),
    ] {
        let (a, b) = crossing_planes_over(xs);
        let two = branch(ends.map(|x| [x, 0.0, x, 0.0]).to_vec());
        on_the_axis(&a, &b, &two, want);
    }
}

/// The curve of `two` on the planes `a` and `b` has the recorded bits, and its true points are
/// on the x axis.
fn on_the_axis(a: &SurfaceKind, b: &SurfaceKind, two: &Branch, want: &[u64]) {
    let (a, b) = (a.clone(), b.clone());
    let curve = fit((&a, &b), REFS, two, CEILING, &mut meter()).unwrap();
    assert!(curve.error() <= 1e-9, "{}", curve.error());
    recorded("PLANES", &bits(&a, &b, &curve), want);
    let (lo, hi) = ParamCurve::domain(&curve);
    for k in 0..=4 {
        let t = lo + (hi - lo) * f64::from(k) / 4.0;
        let x = true_point(&a, &b, &curve, t, &mut meter())
            .unwrap()
            .unwrap_or_else(|| panic!("{t}: no true point"));
        let p = ParamCurve::point(&curve, t, geom_nurbs::Limit::FromRight).unwrap();
        // The point is (x, 0) on both, x the cache's.
        for (i, want) in [p[0], 0.0, p[0], 0.0].into_iter().enumerate() {
            assert!(x[i].lo() <= want && want <= x[i].hi(), "{t}: {x:?}");
        }
    }
}

/// The vertex of the sheet's crossing with z = c at x = `u`, v = c + k·u², as (x, y) on the sheet
/// and through `other` on the second surface.
fn on_parabola(u: f64, other: impl Fn(f64, f64) -> [f64; 2]) -> [f64; 4] {
    let v = C + K * u * u;
    let [p, q] = other(u, v);
    [u, v, p, q]
}

#[test]
fn a_surface_that_moves_fast_is_still_bounded() {
    // The plane's σ is more than 2: the reach in its parameters is the reach over σ, never more.
    let (top, flat) = (sheet(K), fast(C));
    let vertices = vec![
        on_parabola(-0.6, |x, y| [x / 4.0, y / 4.0]),
        on_parabola(0.7, |x, y| [x / 4.0, y / 4.0]),
    ];
    let curve = fit(
        (&top, &flat),
        REFS,
        &branch(vertices),
        CEILING,
        &mut meter(),
    )
    .unwrap();
    assert!(curve.error() <= CEILING);
    for s in curve.spans() {
        assert!(s.curve <= CEILING, "{s:?}");
    }
}

#[test]
fn a_settled_point_outside_the_first_domain_is_never_fit_data() {
    // The plane z = c read as (y, x): along the parabola y = c + k·x² its first parameter is y.
    // Both vertices have y above 0.2 and the true point between them (x near 0.05) has y near c,
    // below it: with the plane cut off at y = 0.2 that point is refused, and with nothing added the
    // fit is out of rounds at once. The same branch on the whole plane is refined into a curve.
    let swapped = |lo: f64| {
        plane(
            [0.0, 0.0, C],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            ((lo, 4.0), (-4.0, 4.0)),
        )
    };
    let top = sheet(K);
    let vertices = || {
        let at = |u: f64| {
            let [x, y, ..] = on_parabola(u, |x, y| [x, y]);
            [y, x, x, y]
        };
        branch(vec![at(-0.6), at(0.7)])
    };
    let whole = swapped(-4.0);
    assert!(fit((&whole, &top), REFS, &vertices(), CEILING, &mut meter()).is_ok());
    let cut = swapped(0.2);
    assert_eq!(
        fit((&cut, &top), REFS, &vertices(), CEILING, &mut meter()).err(),
        Some(IntersectError::BudgetExceeded)
    );
}

#[test]
fn a_refinement_near_the_edge_starts_from_the_middle_of_its_data() {
    // From x = 0.95 to 0.5 on the sheet, near its edge at x = 1: the first fit fails, and the
    // true point between the two vertices is found from their middle, x = 0.725. A start any
    // nearer the edge than that, as far out as 0.95 + 0.5/2, would be off the sheet.
    let (top, flat) = (
        sheet(K),
        plane(
            [0.0, 0.0, C],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-4.0, 4.0), (-4.0, 4.0)),
        ),
    );
    for us in [[0.95, 0.5], [0.5, 0.95]] {
        let vertices = us.map(|u| on_parabola(u, |x, y| [x, y])).to_vec();
        let curve = fit(
            (&top, &flat),
            REFS,
            &branch(vertices),
            CEILING,
            &mut meter(),
        )
        .unwrap();
        assert!(curve.error() <= CEILING, "{us:?}");
    }
}

#[test]
fn a_true_point_in_a_wide_box_is_still_proven() {
    // Under a coarse ceiling the spans' bounds are wide, so the box about the pcurves' points is
    // wide too: the solver must cut it and throw out the parts with no zero, by the field over a
    // box, to prove the one point.
    let (top, flat) = (
        sheet(1.0 / 8.0),
        plane(
            [0.0, 0.0, C],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-4.0, 4.0), (-4.0, 4.0)),
        ),
    );
    let at = |u: f64| [u, C + u * u / 8.0, u, C + u * u / 8.0];
    let curve = fit(
        (&top, &flat),
        REFS,
        &branch(vec![at(-0.9), at(0.9)]),
        1.0 / 4.0,
        &mut meter(),
    )
    .unwrap();
    let widest = curve
        .spans()
        .iter()
        .fold(0.0, |m: f64, s| if s.curve > m { s.curve } else { m });
    assert!(widest >= 1.0 / 64.0, "{widest}");
    let (lo, hi) = ParamCurve::domain(&curve);
    for k in 0..=8 {
        let t = lo + (hi - lo) * f64::from(k) / 8.0;
        let x = true_point(&top, &flat, &curve, t, &mut meter()).unwrap();
        assert!(x.is_some(), "{t}: no true point; widest bound {widest}");
    }
}

#[test]
fn the_true_point_on_a_circle_is_the_near_one_of_two() {
    // The plane z = c cuts the bowl z = x² + y² − 1/4 in a circle, and the plane across the cache
    // at t passes near its centre, so it meets the circle twice, at opposite sides: the box about
    // the pcurves' points is as wide as the bounds ask and no wider, so it holds only the near one.
    let along = || Basis::new(2, vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for (x, sx) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
        for (y, sy) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
            net.push([x, y, sx + sy - 0.25]);
        }
    }
    let bowl = SurfaceKind::Nurbs(Surface::new(along(), along(), &net).unwrap());
    let flat = plane(
        [0.0, 0.0, C],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        ((-4.0, 4.0), (-4.0, 4.0)),
    );
    let r = (C + 0.25_f64).sqrt();
    // At 0, a third and two thirds of a turn: (cos, sin) written out.
    let h = 3.0_f64.sqrt() / 2.0;
    let at = |(c, s): (f64, f64)| {
        let (x, y) = (r * c, r * s);
        [x, y, x, y].map(|q| Interval::point(q).unwrap())
    };
    let ring = Branch {
        vertices: vec![at((1.0, 0.0)), at((-0.5, h)), at((-0.5, -h))],
        ends: [CurveEnd::Closed, CurveEnd::Closed],
        deviation: 0.0,
        passes: Vec::new(),
    };
    let curve = fit((&bowl, &flat), REFS, &ring, CEILING, &mut meter()).unwrap();
    let (lo, hi) = ParamCurve::domain(&curve);
    for k in 1..8 {
        let t = lo + (hi - lo) * f64::from(k) / 8.0;
        let x = true_point(&bowl, &flat, &curve, t, &mut meter())
            .unwrap()
            .unwrap_or_else(|| panic!("{t}: no true point"));
        let p = ParamCurve::point(&curve, t, geom_nurbs::Limit::FromRight).unwrap();
        // The near one: within the ceiling of the cache's point, not across the circle.
        let m = x.map(|i| i.lo() / 2.0 + i.hi() / 2.0);
        let d = ((m[0] - p[0]) * (m[0] - p[0]) + (m[1] - p[1]) * (m[1] - p[1])).sqrt();
        assert!(d <= 2.0 * CEILING, "{t}: {x:?} {p:?}");
    }
}

/// Every bound and true point of a fitted curve as bits: each span's ends and bounds, then the
/// true point's enclosure at each span's ends and middle.
fn bits(first: &SurfaceKind, second: &SurfaceKind, curve: &IntersectionCurve<3>) -> Vec<u64> {
    let mut out = Vec::new();
    for s in curve.spans() {
        for x in [s.at.0, s.at.1, s.curve, s.pcurves[0], s.pcurves[1]] {
            out.push(x.to_bits());
        }
        for t in [s.at.0, s.at.0 / 2.0 + s.at.1 / 2.0, s.at.1] {
            match true_point(first, second, curve, t, &mut meter()).unwrap() {
                Some(x) => {
                    for i in x {
                        out.push(i.lo().to_bits());
                        out.push(i.hi().to_bits());
                    }
                }
                None => out.push(0),
            }
        }
    }
    out
}

/// `got` is the recorded `want`, or the test stops with the new bits listed for a deliberate change.
fn recorded(name: &str, got: &[u64], want: &[u64]) {
    assert!(
        got == want,
        "{name}: recorded bits moved, now {} values\n{}",
        got.len(),
        got.iter()
            .map(|b| format!("0x{b:016x},"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The recorded bits of `bits` for the planes z = 0 and y = 0 from x = 1000 to 1003.
const PLANES_FAR: &[u64] = &[
    0x0000000000000000,
    0x3ff0000000000000,
    0x3d8e0b0000000018,
    0x3d7e0b0000000005,
    0x3d7e0b0000000005,
    0x408f3ffffffffff8,
    0x408f400000000008,
    0x8000000000000011,
    0x0000000000000011,
    0x408f3ffffffffff8,
    0x408f400000000008,
    0x8000000000000011,
    0x0000000000000011,
    0x408f4bfffffffff8,
    0x408f4c0000000008,
    0x8000000000000011,
    0x0000000000000011,
    0x408f4bfffffffff8,
    0x408f4c0000000008,
    0x8000000000000011,
    0x0000000000000011,
    0x408f57fffffffff8,
    0x408f580000000008,
    0x8000000000000011,
    0x0000000000000011,
    0x408f57fffffffff8,
    0x408f580000000008,
    0x8000000000000011,
    0x0000000000000011,
];

/// The recorded bits of `bits` for the planes z = 0 and y = 0 from x = −1 to 2.
const PLANES: &[u64] = &[
    0x0000000000000000,
    0x3ff0000000000000,
    0x3d06400000000014,
    0x3cf6400000000005,
    0x3cf6400000000005,
    0xbff0000000000008,
    0xbfeffffffffffff7,
    0x8000000000000011,
    0x0000000000000011,
    0xbff0000000000008,
    0xbfeffffffffffff7,
    0x8000000000000011,
    0x0000000000000011,
    0x3fdffffffffffff6,
    0x3fe0000000000008,
    0x8000000000000011,
    0x0000000000000011,
    0x3fdffffffffffff6,
    0x3fe0000000000008,
    0x8000000000000011,
    0x0000000000000011,
    0x3ffffffffffffff7,
    0x4000000000000008,
    0x8000000000000011,
    0x0000000000000011,
    0x3ffffffffffffff7,
    0x4000000000000008,
    0x8000000000000011,
    0x0000000000000011,
];

/// The recorded bits of `bits` for the sheet z = v − k·u² and the plane z = c, from x = −0.6 to
/// 0.7 (`fit_refined`'s first case), at this version of the fit: three spans, each with its ends,
/// its three bounds and the true point's enclosure at its ends and middle.
const SHEET_AND_PLANE: &[u64] = &[
    0x0000000000000000,
    0x3fd00836d8653c75,
    0x3f67cbfef4421d45,
    0x3f53a517766f8266,
    0x3f4b1af989b9b2cd,
    0xbfe33333333333c8,
    0xbfe333333333329f,
    0x3fd3851eb851ea94,
    0x3fd3851eb851ec75,
    0xbfe3333333333432,
    0xbfe3333333333235,
    0x3fd3851eb851e9f8,
    0x3fd3851eb851ed13,
    0xbfdc902f7234285f,
    0xbfdc902f7234262d,
    0x3fccbf6e5992b7e0,
    0x3fccbf6e5992bb50,
    0xbfdc902f72342923,
    0xbfdc902f72342567,
    0x3fccbf6e5992b6c3,
    0x3fccbf6e5992bc6a,
    0xbfd222500468a6bc,
    0xbfd222500468a4d1,
    0x3fc5235f67e395ec,
    0x3fc5235f67e39901,
    0xbfd222500468a76c,
    0xbfd222500468a421,
    0x3fc5235f67e394e3,
    0x3fc5235f67e39a07,
    0x3fd00836d8653c75,
    0x3fd80dc48f586425,
    0x3f6a772d005d7891,
    0x3f557f158cf89a85,
    0x3f4e41353be3e52c,
    0xbfd222500468a6bc,
    0xbfd222500468a4d1,
    0x3fc5235f67e395ec,
    0x3fc5235f67e39901,
    0xbfd222500468a76c,
    0xbfd222500468a421,
    0x3fc5235f67e394e3,
    0x3fc5235f67e39a07,
    0xbfc975875dfab451,
    0xbfc975875dfab108,
    0x3fc2882a655e70db,
    0x3fc2882a655e73b8,
    0xbfc975875dfab593,
    0xbfc975875dfaafcd,
    0x3fc2882a655e6fe5,
    0x3fc2882a655e74b7,
    0xbfbcf24ff34185f3,
    0xbfbcf24ff341805c,
    0x3fc0d179b61dcbd1,
    0x3fc0d179b61dce83,
    0xbfbcf24ff3418850,
    0xbfbcf24ff3417e11,
    0x3fc0d179b61dcada,
    0x3fc0d179b61dcf73,
    0x3fd80dc48f586425,
    0x3fe15cb30b41b539,
    0x3f57d2b88439cd57,
    0x3f43dc65bcd058d9,
    0x3f3b7fa170a75d85,
    0xbfbcf24ff34185f3,
    0xbfbcf24ff341805c,
    0x3fc0d179b61dcbd1,
    0x3fc0d179b61dce83,
    0xbfbcf24ff3418850,
    0xbfbcf24ff3417e11,
    0x3fc0d179b61dcada,
    0x3fc0d179b61dcf73,
    0x3f62a4e71d553bcb,
    0x3f62a4e71d55db32,
    0x3fc00015b9ab8f93,
    0x3fc00015b9ab9239,
    0x3f62a4e71d54e956,
    0x3f62a4e71d562d1e,
    0x3fc00015b9ab8e87,
    0x3fc00015b9ab9341,
    0x3fbe1c9382831911,
    0x3fbe1c9382831f50,
    0x3fc0e2ad70cc101c,
    0x3fc0e2ad70cc131b,
    0x3fbe1c938283167f,
    0x3fbe1c93828321ec,
    0x3fc0e2ad70cc0f13,
    0x3fc0e2ad70cc1424,
    0x3fe15cb30b41b539,
    0x3ff0000000000000,
    0x3f66861a11073d66,
    0x3f57b874dbd7957f,
    0x3f49861f8f80cf2b,
    0x3fbe1c9382831911,
    0x3fbe1c9382831f50,
    0x3fc0e2ad70cc101c,
    0x3fc0e2ad70cc131b,
    0x3fbe1c938283167f,
    0x3fbe1c93828321ec,
    0x3fc0e2ad70cc0f13,
    0x3fc0e2ad70cc1424,
    0x3fdb20e8a3599129,
    0x3fdb20e8a3599360,
    0x3fcb7fd535c48cfa,
    0x3fcb7fd535c490d2,
    0x3fdb20e8a3599070,
    0x3fdb20e8a359941a,
    0x3fcb7fd535c48bd5,
    0x3fcb7fd535c491f7,
    0x3fe66666666665b9,
    0x3fe6666666666715,
    0x3fd7ae147ae1467b,
    0x3fd7ae147ae148dd,
    0x3fe6666666666545,
    0x3fe6666666666787,
    0x3fd7ae147ae145be,
    0x3fd7ae147ae1499b,
];

#[test]
fn the_sheet_and_plane_bounds_are_the_recorded_ones() {
    let (top, flat) = (
        sheet(K),
        plane(
            [0.0, 0.0, C],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-4.0, 4.0), (-4.0, 4.0)),
        ),
    );
    let vertices = vec![
        on_parabola(-0.6, |x, y| [x, y]),
        on_parabola(0.7, |x, y| [x, y]),
    ];
    let curve = fit(
        (&top, &flat),
        REFS,
        &branch(vertices),
        CEILING,
        &mut meter(),
    )
    .unwrap();
    recorded(
        "SHEET_AND_PLANE",
        &bits(&top, &flat, &curve),
        SHEET_AND_PLANE,
    );
}
