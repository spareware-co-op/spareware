//! The fit's guards and bounds read exactly (G175, chapter 12.12): a ceiling that is not positive
//! and a branch of one vertex are refused before any work; a surface that moves fast is still
//! bounded; a settled point outside a surface's domain is never fit data; a refinement near an edge
//! starts from the middle of its data; a true point is proven in a wide box, and on a circle it is
//! the near one of two; a cubic patch against a plane, whose pcurve is 0 across, is a curve or out
//! of rounds, never refused; a pcurve's side a few steps of the double wide on a surface curved
//! across it is widened by half the widest side, and one exactly a step wide is not, as the work
//! shows; and the bounds and true points of two planes (near the origin and far out), of the sheet
//! and plane of `fit_refined` and of those thin sides are the recorded bits, so any change to their
//! arithmetic is seen.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly; the names are chapter 12.12's"
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
    // Under Miri, where the true points' proofs are nearly all of the test's time, the axis is
    // checked at t = ¼ alone (near the origin a quick proof, far out a full one): the points at the
    // span's ends and middle are the ones `bits` has just proven, held to the recorded bits.
    #[cfg(miri)]
    let quarters = [1];
    #[cfg(not(miri))]
    let quarters = 0..=4;
    for k in quarters {
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

/// (u³, v, 0) over [1/4, 1] × [−1, 1], as a cubic NURBS: its Bézier points along u are the blossom
/// of u³ at the ends, 1/64, 1/16, 1/4 and 1, all exact.
fn cubic() -> SurfaceKind {
    let along = Basis::new(3, vec![0.25, 0.25, 0.25, 0.25, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let across = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for x in [1.0 / 64.0, 1.0 / 16.0, 0.25, 1.0] {
        for y in [-1.0, 1.0] {
            net.push([x, y, 0.0]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap())
}

#[test]
fn a_cubic_patch_against_a_plane_is_a_curve_or_out_of_rounds() {
    // The cubic patch (u³, v, 0) meets the plane y = 0 along v = 0, from u = 0.5 to 0.8: both
    // vertices lie on both surfaces and inside both domains, so the branch is valid (#1083: it was
    // refused as InvalidInput). The pcurve's v is 0 over every span, and its bound over a half
    // span is a few subnormals wide, a box the patch's partials could not be read over before
    // #1140.
    let (patch, (_, across)) = (cubic(), crossing_planes());
    let at = |u: f64| [u, 0.0, u * u * u, 0.0];
    let two = || branch(vec![at(0.5), at(0.8)]);
    let coarse = 1.0 / 64.0;
    let curve = fit((&patch, &across), REFS, &two(), coarse, &mut meter()).unwrap();
    assert!(curve.error() <= coarse, "{}", curve.error());
    for s in curve.spans() {
        assert!(s.curve <= coarse, "{s:?}");
    }
    // The curve is a straight segment, so the cache keeps one span however much data refinement
    // adds. The best cubic span of the pcurve u = x^(1/3) is within 1/256 of it (its gap is about
    // 1.5e-3), but the certified bound stops it: the turning kappa, about 6.7 at u = 1/2 on this
    // flat patch (about 12 over the binding piece's widened box), with the sine bound 0.64 where
    // the surfaces cross at right angles, keeps the curve's bound near 4.5e-3, over 1/256 every
    // round. The fit is out of rounds, as its errors say, never a refusal of the input.
    assert_eq!(
        fit((&patch, &across), REFS, &two(), CEILING, &mut meter()).err(),
        Some(IntersectError::BudgetExceeded)
    );
}

/// (x(u), v, v²) over `along`'s domain × [−1, 1], x(u) the curve along u with Bézier points `xs`:
/// curved across v, so a box's width in v moves its partials' bounds.
fn ridge(along: Basis, xs: &[f64]) -> SurfaceKind {
    let across = Basis::new(2, vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for &x in xs {
        for (y, z) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
            net.push([x, y, z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap())
}

/// (v, (u − 3)², u − 3) over [2, 4] × [0, 1]: through the x axis at u = 3, curved across u, so a
/// box's width in u moves its partials' bounds.
fn trough() -> SurfaceKind {
    let along = Basis::new(2, vec![2.0, 2.0, 2.0, 4.0, 4.0, 4.0]).unwrap();
    let across = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for (y, z) in [(1.0, -1.0), (-1.0, 0.0), (1.0, 1.0)] {
        for x in [0.0, 1.0] {
            net.push([x, y, z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap())
}

#[test]
fn a_trough_side_a_few_steps_wide_is_widened_by_half_the_widest_side() {
    // The ridge (u, v, v²) and the trough, from x = 1/4 to 3/4: on the ridge the pcurve's v is 0
    // throughout, a line; on the trough its u runs from 3 to 3 + 2⁻⁵⁰, two steps of the double at
    // 3 and under 4ε·(1 + 3), so that side is widened by half the widest side, its v from 1/4 to
    // 3/4. The trough is curved across u, so how far it is widened shows in the bounds.
    let line = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let (top, low) = (ridge(line, &[0.0, 1.0]), trough());
    let w = 3.0 + 4.0 * f64::EPSILON;
    assert!(w - 3.0 < 4.0 * f64::EPSILON * (1.0 + w));
    let two = branch(vec![[0.25, 0.0, 3.0, 0.25], [0.75, 0.0, w, 0.75]]);
    let curve = fit((&top, &low), REFS, &two, CEILING, &mut meter()).unwrap();
    assert!(curve.error() <= CEILING, "{}", curve.error());
    recorded("TROUGH_THIN", &bits(&top, &low, &curve), TROUGH_THIN);
}

#[test]
fn a_trough_side_exactly_a_step_wide_is_not_widened() {
    // The same ridge and trough with the trough's u from 3 − 2⁻⁴⁸ to 3: over the whole span the
    // pcurve's box is its two held ends, so that side is exactly 4ε·(1 + 3) wide, a full step, and
    // is kept as it is. Over so thin a box the trough's normal cannot be bounded away from 0, so the
    // first round's whole-span piece stops there; widened, it would read the surface over a wide box
    // and stop later, at the angle, for more work. The work is the recorded count.
    let line = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let (top, low) = (ridge(line, &[0.0, 1.0]), trough());
    let lo = 3.0 - 16.0 * f64::EPSILON;
    assert_eq!(3.0 - lo, 4.0 * f64::EPSILON * (1.0 + 3.0));
    let two = branch(vec![[0.25, 0.0, lo, 0.25], [0.75, 0.0, 3.0, 0.75]]);
    let mut m = meter();
    let curve = fit((&top, &low), REFS, &two, CEILING, &mut m).unwrap();
    assert_eq!(m.used().work, 4379);
    assert!(curve.error() <= CEILING, "{}", curve.error());
    recorded("TROUGH_STEP", &bits(&top, &low, &curve), TROUGH_STEP);
}

/// The recorded bits of `bits` for the ridge and the trough exactly a step wide.
const TROUGH_STEP: &[u64] = &[
    0x0000_0000_0000_0000,
    0x3ff0_0000_0000_0000,
    0x3d2e_07d0_c2ce_5d31,
    0x3d0c_9a30_a407_90d4,
    0x3d25_fb16_64f1_8991,
    0x0000_0000_0000_0000,
    0x0000_0000_0000_0000,
    0x0000_0000_0000_0000,
];

/// The recorded bits of `bits` for the ridge and the trough a few steps wide.
const TROUGH_THIN: &[u64] = &[
    0x0000_0000_0000_0000,
    0x3ff0_0000_0000_0000,
    0x3d26_cef3_bbae_e5b6,
    0x3d03_dccd_6c92_1ffa,
    0x3d11_7057_5e22_0c9c,
    0x0000_0000_0000_0000,
    0x0000_0000_0000_0000,
    0x0000_0000_0000_0000,
];

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
            .map(|b| {
                let word = |k: u32| (b >> k) & 0xffff;
                format!(
                    "0x{:04x}_{:04x}_{:04x}_{:04x},",
                    word(48),
                    word(32),
                    word(16),
                    word(0)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The recorded bits of `bits` for the planes z = 0 and y = 0 from x = 1000 to 1003.
const PLANES_FAR: &[u64] = &[
    0x0000_0000_0000_0000,
    0x3ff0_0000_0000_0000,
    0x3d8e_0b00_0000_0018,
    0x3d7e_0b00_0000_0005,
    0x3d7e_0b00_0000_0005,
    0x408f_3fff_ffff_fff8,
    0x408f_4000_0000_0008,
    0x8000_0000_0000_0011,
    0x0000_0000_0000_0011,
    0x408f_3fff_ffff_fff8,
    0x408f_4000_0000_0008,
    0x8000_0000_0000_0011,
    0x0000_0000_0000_0011,
    0x408f_4bff_ffff_fff8,
    0x408f_4c00_0000_0008,
    0x8000_0000_0000_0011,
    0x0000_0000_0000_0011,
    0x408f_4bff_ffff_fff8,
    0x408f_4c00_0000_0008,
    0x8000_0000_0000_0011,
    0x0000_0000_0000_0011,
    0x408f_57ff_ffff_fff8,
    0x408f_5800_0000_0008,
    0x8000_0000_0000_0011,
    0x0000_0000_0000_0011,
    0x408f_57ff_ffff_fff8,
    0x408f_5800_0000_0008,
    0x8000_0000_0000_0011,
    0x0000_0000_0000_0011,
];

/// The recorded bits of `bits` for the planes z = 0 and y = 0 from x = −1 to 2.
const PLANES: &[u64] = &[
    0x0000_0000_0000_0000,
    0x3ff0_0000_0000_0000,
    0x3d06_4000_0000_0014,
    0x3cf6_4000_0000_0005,
    0x3cf6_4000_0000_0005,
    0xbff0_0000_0000_0008,
    0xbfef_ffff_ffff_fff7,
    0x8000_0000_0000_0011,
    0x0000_0000_0000_0011,
    0xbff0_0000_0000_0008,
    0xbfef_ffff_ffff_fff7,
    0x8000_0000_0000_0011,
    0x0000_0000_0000_0011,
    0x3fdf_ffff_ffff_fff6,
    0x3fe0_0000_0000_0008,
    0x8000_0000_0000_0011,
    0x0000_0000_0000_0011,
    0x3fdf_ffff_ffff_fff6,
    0x3fe0_0000_0000_0008,
    0x8000_0000_0000_0011,
    0x0000_0000_0000_0011,
    0x3fff_ffff_ffff_fff7,
    0x4000_0000_0000_0008,
    0x8000_0000_0000_0011,
    0x0000_0000_0000_0011,
    0x3fff_ffff_ffff_fff7,
    0x4000_0000_0000_0008,
    0x8000_0000_0000_0011,
    0x0000_0000_0000_0011,
];

/// The recorded bits of `bits` for the sheet z = v − k·u² and the plane z = c, from x = −0.6 to
/// 0.7 (`fit_refined`'s first case), at this version of the fit: three spans, each with its ends,
/// its three bounds and the true point's enclosure at its ends and middle.
const SHEET_AND_PLANE: &[u64] = &[
    0x0000_0000_0000_0000,
    0x3fd0_0836_d865_3c75,
    0x3f67_cbfe_f442_1d45,
    0x3f53_a517_766f_8266,
    0x3f4b_1af9_89b9_b2cd,
    0xbfe3_3333_3333_33c8,
    0xbfe3_3333_3333_329f,
    0x3fd3_851e_b851_ea94,
    0x3fd3_851e_b851_ec75,
    0xbfe3_3333_3333_3432,
    0xbfe3_3333_3333_3235,
    0x3fd3_851e_b851_e9f8,
    0x3fd3_851e_b851_ed13,
    0xbfdc_902f_7234_285f,
    0xbfdc_902f_7234_262d,
    0x3fcc_bf6e_5992_b7e0,
    0x3fcc_bf6e_5992_bb50,
    0xbfdc_902f_7234_2923,
    0xbfdc_902f_7234_2567,
    0x3fcc_bf6e_5992_b6c3,
    0x3fcc_bf6e_5992_bc6a,
    0xbfd2_2250_0468_a6bc,
    0xbfd2_2250_0468_a4d1,
    0x3fc5_235f_67e3_95ec,
    0x3fc5_235f_67e3_9901,
    0xbfd2_2250_0468_a76c,
    0xbfd2_2250_0468_a421,
    0x3fc5_235f_67e3_94e3,
    0x3fc5_235f_67e3_9a07,
    0x3fd0_0836_d865_3c75,
    0x3fd8_0dc4_8f58_6425,
    0x3f6a_772d_005d_7891,
    0x3f55_7f15_8cf8_9a85,
    0x3f4e_4135_3be3_e52c,
    0xbfd2_2250_0468_a6bc,
    0xbfd2_2250_0468_a4d1,
    0x3fc5_235f_67e3_95ec,
    0x3fc5_235f_67e3_9901,
    0xbfd2_2250_0468_a76c,
    0xbfd2_2250_0468_a421,
    0x3fc5_235f_67e3_94e3,
    0x3fc5_235f_67e3_9a07,
    0xbfc9_7587_5dfa_b451,
    0xbfc9_7587_5dfa_b108,
    0x3fc2_882a_655e_70db,
    0x3fc2_882a_655e_73b8,
    0xbfc9_7587_5dfa_b593,
    0xbfc9_7587_5dfa_afcd,
    0x3fc2_882a_655e_6fe5,
    0x3fc2_882a_655e_74b7,
    0xbfbc_f24f_f341_85f3,
    0xbfbc_f24f_f341_805c,
    0x3fc0_d179_b61d_cbd1,
    0x3fc0_d179_b61d_ce83,
    0xbfbc_f24f_f341_8850,
    0xbfbc_f24f_f341_7e11,
    0x3fc0_d179_b61d_cada,
    0x3fc0_d179_b61d_cf73,
    0x3fd8_0dc4_8f58_6425,
    0x3fe1_5cb3_0b41_b539,
    0x3f57_d2b8_8439_cd57,
    0x3f43_dc65_bcd0_58d9,
    0x3f3b_7fa1_70a7_5d85,
    0xbfbc_f24f_f341_85f3,
    0xbfbc_f24f_f341_805c,
    0x3fc0_d179_b61d_cbd1,
    0x3fc0_d179_b61d_ce83,
    0xbfbc_f24f_f341_8850,
    0xbfbc_f24f_f341_7e11,
    0x3fc0_d179_b61d_cada,
    0x3fc0_d179_b61d_cf73,
    0x3f62_a4e7_1d55_3bcb,
    0x3f62_a4e7_1d55_db32,
    0x3fc0_0015_b9ab_8f93,
    0x3fc0_0015_b9ab_9239,
    0x3f62_a4e7_1d54_e956,
    0x3f62_a4e7_1d56_2d1e,
    0x3fc0_0015_b9ab_8e87,
    0x3fc0_0015_b9ab_9341,
    0x3fbe_1c93_8283_1911,
    0x3fbe_1c93_8283_1f50,
    0x3fc0_e2ad_70cc_101c,
    0x3fc0_e2ad_70cc_131b,
    0x3fbe_1c93_8283_167f,
    0x3fbe_1c93_8283_21ec,
    0x3fc0_e2ad_70cc_0f13,
    0x3fc0_e2ad_70cc_1424,
    0x3fe1_5cb3_0b41_b539,
    0x3ff0_0000_0000_0000,
    0x3f66_861a_1107_3d66,
    0x3f57_b874_dbd7_957f,
    0x3f49_861f_8f80_cf2b,
    0x3fbe_1c93_8283_1911,
    0x3fbe_1c93_8283_1f50,
    0x3fc0_e2ad_70cc_101c,
    0x3fc0_e2ad_70cc_131b,
    0x3fbe_1c93_8283_167f,
    0x3fbe_1c93_8283_21ec,
    0x3fc0_e2ad_70cc_0f13,
    0x3fc0_e2ad_70cc_1424,
    0x3fdb_20e8_a359_9129,
    0x3fdb_20e8_a359_9360,
    0x3fcb_7fd5_35c4_8cfa,
    0x3fcb_7fd5_35c4_90d2,
    0x3fdb_20e8_a359_9070,
    0x3fdb_20e8_a359_941a,
    0x3fcb_7fd5_35c4_8bd5,
    0x3fcb_7fd5_35c4_91f7,
    0x3fe6_6666_6666_65b9,
    0x3fe6_6666_6666_6715,
    0x3fd7_ae14_7ae1_467b,
    0x3fd7_ae14_7ae1_48dd,
    0x3fe6_6666_6666_6545,
    0x3fe6_6666_6666_6787,
    0x3fd7_ae14_7ae1_45be,
    0x3fd7_ae14_7ae1_499b,
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
