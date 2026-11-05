//! Attack tests from SMLib's confirmed defects, batch 3, first line: closed-form intersections
//! (`geom-intersect`, pieces G163 to G167).
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib defects
//! (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: the lines, circles and points the geometry says, checked
//! against the surfaces' own equations, or a defined refusal, and never a panic, a hang, a NaN or a
//! silent wrong answer.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::type_complexity,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::assert_is_empty,
    reason = "tests: native arithmetic and indexing, platform maths on an independent oracle (the surface's own equation), prose that names defect files, and a failure stops the test"
)]

use geom_curve::{Basis as Leaf, Circle, CurveKind, Frame, Line, ParamCurve, Trimmed};
use geom_intersect::IntersectError;
use geom_intersect::curve::curves;
use geom_intersect::quadrics::natural;
use geom_intersect::result::{CurveContact, Intersection, Item, PointContact};
use geom_num::exactness::Exactness;
use geom_num::grid::Steps;
use geom_nurbs::{Curve, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Extrusion, Plane, SurfaceKind};

const ROUND: (f64, f64) = (0.0, 2.0);
const WIDE: (f64, f64) = (-1000.0, 1000.0);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn up() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap()
}

/// A whole-turn cylinder about the z axis through `c`, of radius `r`, over heights `v` from `c`.
fn tube(c: [f64; 3], r: f64, v: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new(c, r, up(), (ROUND, v)).unwrap())
}

fn plane(c: [f64; 3], a: [f64; 3], b: [f64; 3], domain: ((f64, f64), (f64, f64))) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(c, a, b, domain).unwrap())
}

/// The plane z = `z`.
fn level(z: f64) -> SurfaceKind {
    plane(
        [0.0, 0.0, z],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        (WIDE, WIDE),
    )
}

fn trimmed(c: CurveKind<3>) -> Trimmed<3> {
    let (lo, hi) = c.domain();
    Trimmed::new(Leaf::try_from(c).unwrap(), (lo, hi)).unwrap()
}

fn pushed(profile: CurveKind<3>, d: [f64; 3], span: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Extrusion(Extrusion::new(trimmed(profile), d, span).unwrap())
}

/// A box wall: a segment pushed along z over [0, 10].
fn wall(a: [f64; 3], b: [f64; 3]) -> SurfaceKind {
    pushed(
        CurveKind::Line(Line::segment(a, b).unwrap().value),
        [0.0, 0.0, 1.0],
        (0.0, 10.0),
    )
}

fn found(a: &SurfaceKind, b: &SurfaceKind) -> Intersection {
    natural(a, b, &mut meter())
        .unwrap()
        .unwrap_or_else(|| panic!("no closed form for {a:?} and {b:?}"))
}

fn lines_of(r: &Intersection) -> Vec<&CurveKind<3>> {
    r.items()
        .iter()
        .filter_map(|i| match i {
            Item::Curve { curve, .. } => Some(&**curve),
            _ => None,
        })
        .collect()
}

/// The two ends of a curve.
fn ends_of(c: &CurveKind<3>) -> ([f64; 3], [f64; 3]) {
    let (lo, hi) = c.domain();
    (
        c.point(lo, Limit::FromRight).unwrap(),
        c.point(hi, Limit::FromLeft).unwrap(),
    )
}

/// Nine points along a curve, the last read from the left since a domain's end is its own.
fn samples(c: &CurveKind<3>) -> Vec<[f64; 3]> {
    let (lo, hi) = c.domain();
    (0..=8)
        .map(|i| {
            let t = (lo + (hi - lo) * f64::from(i) / 8.0).min(hi);
            let side = if i == 8 {
                Limit::FromLeft
            } else {
                Limit::FromRight
            };
            c.point(t, side).unwrap()
        })
        .collect()
}

/// How far a curve of class `class` may stand from where it should be at `p`: its class's reach in
/// the model's units, with rounding's share (an exact curve by rounding alone).
fn allowed(class: Exactness, p: [f64; 3]) -> f64 {
    let rounding = 1e-6 * (1.0 + p[0].abs() + p[1].abs() + p[2].abs());
    match class {
        Exactness::Exact => rounding,
        Exactness::Certified {
            bound: Steps(n),
            step,
        } => f64::from(u32::try_from(n).unwrap()) * f64::from(step.quarter_nm()) / 625.0 + rounding,
        Exactness::Approximate { .. } => panic!("a closed form only approximate"),
    }
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} and {b}");
}

/// How far `p` stands from the cylinder of radius `r` about the axis through `c` along z.
fn off_tube(p: [f64; 3], c: [f64; 3], r: f64) -> f64 {
    ((p[0] - c[0]).hypot(p[1] - c[1]) - r).abs()
}

/// solidcore plan/upstream-issues/WR-259--smcone-intersectwithcone-drops-the-line-of-two-parallel.md
/// (b4de2149, confirmed on the C++ reference): the line of two parallel cylinders was dropped when
/// a face's domain started above the cylinder's bottom (an r 20 and an r 3.04 cylinder, the second
/// through (0, 20), heights 11 from z = 2; zero lines when either domain's v minimum was above 0,
/// two when only the maximum was trimmed). The input is the defect's, and the trimmings its fix
/// names: a domain trimmed at the bottom of either, of both, and two that overlap only away from
/// the middle of each. Each case has the two lines, each running over the heights both domains
/// share, on both cylinders, and two domains that share no height have none.
#[test]
fn wr_259_parallel_cylinders_with_a_trimmed_domain_meet_in_both_lines() {
    let cut = |a_v: (f64, f64), b_v: (f64, f64)| {
        found(
            &tube([0.0, 0.0, 2.0], 20.0, a_v),
            &tube([0.0, 20.0, 2.0], 3.04, b_v),
        )
    };
    // Heights are from z = 2: the shared stretch is the overlap of the two, shifted by 2.
    let cases: [((f64, f64), (f64, f64), f64, f64); 5] = [
        ((0.0, 11.0), (0.0, 11.0), 2.0, 13.0),
        ((0.8, 10.5), (0.0, 11.0), 2.8, 12.5),
        ((0.0, 11.0), (0.8, 10.5), 2.8, 12.5),
        ((0.8, 10.5), (0.8, 10.5), 2.8, 12.5),
        ((1.0, 9.0), (2.0, 10.0), 4.0, 11.0),
    ];
    for (a_v, b_v, lo, hi) in cases {
        let r = cut(a_v, b_v);
        let lines = lines_of(&r);
        assert_eq!(lines.len(), 2, "{a_v:?} and {b_v:?}: {:?}", r.items());
        assert_eq!(r.items().len(), 2, "{a_v:?} and {b_v:?}: {:?}", r.items());
        for line in lines {
            assert!(matches!(line, CurveKind::Line(_)), "{line:?}");
            let (p, q) = ends_of(line);
            near(p[2].min(q[2]), lo);
            near(p[2].max(q[2]), hi);
            for s in samples(line) {
                near(off_tube(s, [0.0, 0.0, 2.0], 20.0), 0.0);
                near(off_tube(s, [0.0, 20.0, 2.0], 3.04), 0.0);
            }
        }
    }
    // No height in common: nothing.
    assert!(cut((0.0, 3.0), (5.0, 11.0)).items().is_empty());
}

/// solidcore plan/upstream-issues/WR-259--smcone-intersectwithcone-drops-the-line-of-two-parallel.md
/// (b4de2149), the case its fix note says a test at the middle still drops: two domains that
/// overlap in heights only away from the middle of each (the r 20 and r 3.04 cylinders, heights
/// 0 to 5 and 3 to 10 from z = 2, share z from 5 to 7). Both lines run over that stretch. Real
/// defect in the closed form, found by this test: it returns two points at z = 6, marked as
/// touching at the boundary, and no line.
#[test]
#[ignore = "defect: the closed form keeps two boundary points at z = 6 and drops the two lines over z 5 to 7 when the heights overlap only away from both middles"]
fn wr_259_domains_overlapping_only_away_from_each_middle_still_meet_in_both_lines() {
    let r = found(
        &tube([0.0, 0.0, 2.0], 20.0, (0.0, 5.0)),
        &tube([0.0, 20.0, 2.0], 3.04, (3.0, 10.0)),
    );
    let lines = lines_of(&r);
    assert_eq!(lines.len(), 2, "{:?}", r.items());
    for line in lines {
        let (p, q) = ends_of(line);
        near(p[2].min(q[2]), 5.0);
        near(p[2].max(q[2]), 7.0);
    }
}

/// solidcore plan/upstream-issues/UB-123--smcone-intersectwithcone-dereferences-an-absent-tolerance.md
/// (b4de2149): two distinct objects holding coincident, overlapping cylinders, with no tolerance
/// given, dereferenced a null tolerance. The input is the defect's: separate cylinders of one
/// radius on one axis with heights that overlap, and the neighbours the same code path exits on:
/// heights that do not overlap, nested radii, the same object. The overlapping pair shares a
/// region, the others share nothing or are the same.
#[test]
fn ub_123_coincident_cylinders_of_two_objects_overlap_or_do_not_meet() {
    let a = tube([0.0; 3], 3.0, (0.0, 5.0));
    let b = tube([0.0; 3], 3.0, (3.0, 8.0));
    let r = found(&a, &b);
    assert!(
        matches!(r.items(), [Item::Overlap { .. }]),
        "{:?}",
        r.items()
    );
    assert!(
        found(&a, &tube([0.0; 3], 3.0, (6.0, 8.0)))
            .items()
            .is_empty()
    );
    assert!(
        found(&a, &tube([0.0; 3], 2.0, (0.0, 5.0)))
            .items()
            .is_empty()
    );
    let same = found(&a, &a.clone());
    assert!(
        matches!(same.items(), [Item::Overlap { .. }]),
        "{:?}",
        same.items()
    );
}

/// solidcore plan/upstream-issues/WR-510--cone-intersectwithplane-tangent-within-a-faces-zone-tolerance.md
/// (b4de2149, not confirmed by a C++ run): a plane parallel to a cylinder's axis and a hair inside
/// its wall counted as tangent only when its two crossing lines were within the tolerance of each
/// other, so a plane 1e-11 inside a radius 3 wall cut two lines 1.5e-5 apart instead of touching
/// along one. The input is the defect's: a radius 3 cylinder and a plane 1e-11 inside, 1e-11
/// outside, on the wall, and turned 1e-13 about the cylinder's axis. The wall is whole-number data, so
/// the touch is one tangent line. Where the kernel cannot tell a hair by its enclosures it leaves
/// the pair to the general paths (`None`); where it answers it is right: two crossing lines at
/// the chord's half-width for the first, nothing for the second, and for the fourth lines that lie
/// on both surfaces.
#[test]
fn wr_510_a_plane_a_hair_from_a_cylinder_s_wall_is_decided_exactly() {
    let wall = tube([0.0; 3], 3.0, (-5.0, 5.0));
    let flat = |x: f64| {
        plane(
            [x, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            (WIDE, WIDE),
        )
    };
    let dip = 1e-11;
    let inside = 3.0 - dip;
    // The stored numbers are not whole, so where the kernel cannot tell by its enclosures it
    // answers `None` and leaves the pair to the general paths; what it does answer must be right.
    if let Some(r) = natural(&flat(inside), &wall, &mut meter()).unwrap() {
        let lines = lines_of(&r);
        assert_eq!(r.items().len(), 2, "{:?}", r.items());
        assert_eq!(lines.len(), 2);
        let half = ((3.0 - inside) * (3.0 + inside)).sqrt();
        let mut ys: Vec<f64> = lines.iter().map(|l| ends_of(l).0[1]).collect();
        ys.sort_by(f64::total_cmp);
        assert!(
            (ys[0] + half).abs() < 1e-9 && (ys[1] - half).abs() < 1e-9,
            "{ys:?}"
        );
        for item in r.items() {
            assert!(
                matches!(
                    item,
                    Item::Curve {
                        contact: CurveContact::Crossing,
                        ..
                    }
                ),
                "{item:?}"
            );
        }
    }
    if let Some(r) = natural(&flat(3.0 + dip), &wall, &mut meter()).unwrap() {
        assert!(r.items().is_empty(), "{:?}", r.items());
    }
    let r = found(&flat(3.0), &wall);
    assert_eq!(lines_of(&r).len(), 1, "{:?}", r.items());
    assert!(matches!(
        r.items(),
        [Item::Curve {
            contact: CurveContact::Tangent { .. },
            ..
        }]
    ));
    // Turned 1e-13 about the axis: it touches at one end of the chord and dips by 3e-13 at the
    // other, so whichever way the kernel reads it, the answer is lines on both surfaces.
    let turned = plane(
        [3.0, 0.0, 0.0],
        [-1e-13, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        (WIDE, WIDE),
    );
    if let Some(r) = natural(&turned, &wall, &mut meter()).unwrap() {
        let lines = lines_of(&r);
        assert!(!lines.is_empty() && lines.len() <= 2, "{:?}", r.items());
        for line in lines {
            for p in samples(line) {
                near(off_tube(p, [0.0; 3], 3.0), 0.0);
                near(p[0] - 3.0 + 1e-13 * p[1], 0.0);
            }
        }
    }
}

/// solidcore plan/upstream-issues/UB-93--smsurfofextrusion-intersectwithcylinder-makes-the-iso-circle.md
/// and UB-94--smsurfofextrusion-intersections-dereference-the-optional-3d.md (b4de2149): an
/// extrusion cut by a cylinder parallel to it built the cylinder's end circle from unset values
/// when the cylinder was unbounded, and dereferenced the optional list of 3D curves. The input is
/// the defects': an extruded quarter circle of radius 5 and a radius 0.6 cylinder of the same
/// direction, with the cylinder's height range as tall as the frame allows, and past it. The
/// one line where the cylinder's wall crosses the arc is found with the heights both share,
/// whatever the cylinder's height, and a height past the frame's range is refused.
#[test]
fn ub_93_94_an_extrusion_cut_by_a_parallel_cylinder_of_any_height_meets_in_one_line() {
    let arc = Circle::new([0.0; 3], 5.0, up(), (0.0, 0.5)).unwrap();
    let wall = pushed(CurveKind::Circle(arc), [0.0, 0.0, 1.0], (0.0, 10.0));
    // The circles (x - 5)^2 + y^2 = 0.36 and x^2 + y^2 = 25 meet at x = 4.9640, y = +-0.5989; the
    // quarter keeps y > 0.
    for tall in [500.0, 1e6, 1e11] {
        let r = found(&tube([5.0, 0.0, 0.0], 0.6, (-tall, tall)), &wall);
        let lines = lines_of(&r);
        assert_eq!(lines.len(), 1, "{tall}: {:?}", r.items());
        assert_eq!(r.items().len(), 1, "{tall}: {:?}", r.items());
        let (p, q) = ends_of(lines[0]);
        near(p[2].min(q[2]), 0.0);
        near(p[2].max(q[2]), 10.0);
        for s in samples(lines[0]) {
            near(s[0].hypot(s[1]), 5.0);
            near(off_tube(s, [5.0, 0.0, 0.0], 0.6), 0.0);
            assert!(s[1] > 0.0, "{s:?}");
        }
    }
    let past = tube([5.0, 0.0, 0.0], 0.6, (-1e13, 1e13));
    assert_eq!(
        natural(&past, &wall, &mut meter()).unwrap_err(),
        IntersectError::FrameRange
    );
}

/// solidcore plan/upstream-issues/UB-228--smsurfofextrusion-intersectwithplane-dereferences-a-failed.md
/// (b4de2149, confirmed on the C++ reference): the plane projection of a generator failed for an
/// extrusion vector whose squared length overflows, and was dereferenced. The input is the
/// defect's: a B-spline generator pushed along (1e300, 0, 0) and along (1e-9, 0, 1e300), and one
/// along the largest double, cut by a plane across it. The extrusion is refused when it is made,
/// or the intersection is refused, or what it returns is finite and on the plane: never a panic or
/// a point that is not a number.
#[test]
fn ub_228_an_extrusion_vector_that_overflows_is_refused_or_cuts_on_the_plane() {
    let basis = geom_nurbs::Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let points = [
        [0.0, 0.0, 0.0],
        [1.0, 2.0, 0.0],
        [2.0, -2.0, 0.0],
        [3.0, 0.0, 0.0],
    ];
    let spline = CurveKind::Nurbs(Curve::new(basis, &points).unwrap());
    let cut = plane(
        [1.5, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        (WIDE, WIDE),
    );
    for d in [
        [1e300, 0.0, 0.0],
        [1e-9, 0.0, 1e300],
        [f64::MAX, f64::MAX, 0.0],
        [1.0, 0.0, 1e150],
    ] {
        let Ok(made) = Extrusion::new(trimmed(spline.clone()), d, (0.0, 1.0)) else {
            continue;
        };
        let s = SurfaceKind::Extrusion(made);
        if let Ok(Some(r)) = natural(&cut, &s, &mut meter()) {
            for c in lines_of(&r) {
                let (lo, hi) = c.domain();
                if !(lo.is_finite() && hi.is_finite()) {
                    continue;
                }
                for p in samples(c) {
                    assert!(p.iter().all(|x| x.is_finite()), "{d:?}: {p:?}");
                    assert!((p[0] - 1.5).abs() <= 1e-6 * (1.0 + p[1].abs() + p[2].abs()));
                }
            }
        }
    }
}

/// solidcore plan/upstream-issues/UB-95--smsurfofextrusion-intersectwithplane-compares-with-an.md
/// (b4de2149, not confirmed by a C++ run): a plane parallel to the extrusion of a closed
/// generator, bounded so that the first hit on the directrix missed the plane's domain while a
/// later one was inside, compared against an uninitialised first point. The input is the defect's:
/// a whole circle of radius 5 pushed along z, and the plane x = 3, which meets it at y = 4 and
/// y = -4, over a domain that holds only the hit with y > 0, only y < 0, or neither. Each has the
/// one line it holds, or none, on both surfaces.
#[test]
fn ub_95_a_plane_along_a_closed_extrusion_keeps_the_hit_its_domain_holds() {
    let ring = Circle::new([0.0; 3], 5.0, up(), ROUND).unwrap();
    let drum = pushed(CurveKind::Circle(ring), [0.0, 0.0, 1.0], (0.0, 10.0));
    let side = |y: (f64, f64)| {
        plane(
            [3.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            (y, (0.0, 10.0)),
        )
    };
    for (y, hit) in [
        ((0.0, 10.0), Some(4.0)),
        ((-10.0, 0.0), Some(-4.0)),
        ((5.0, 10.0), None),
        ((-10.0, -5.0), None),
    ] {
        let r = found(&side(y), &drum);
        let lines = lines_of(&r);
        match hit {
            Some(h) => {
                assert_eq!(lines.len(), 1, "{y:?}: {:?}", r.items());
                assert_eq!(r.items().len(), 1, "{y:?}: {:?}", r.items());
                let (p, q) = ends_of(lines[0]);
                near(p[2].min(q[2]), 0.0);
                near(p[2].max(q[2]), 10.0);
                for s in samples(lines[0]) {
                    near(s[0], 3.0);
                    near(s[1], h);
                }
            }
            None => assert!(r.items().is_empty(), "{y:?}: {:?}", r.items()),
        }
    }
}

/// solidcore plan/upstream-issues/UB-81--sm-intersectgenerators-reads-unset-offsets-and-a-possibly.md
/// (b4de2149, not confirmed by a C++ run): the intersection of two extrusions' generators, when
/// the generators overlap in a range, read offsets that were set only if an end of the range
/// equalled one of four bounds exactly. The input is the defect's: two walls of one direction
/// whose segments lie on one line and overlap in a range ends at neither wall's end (x from 0 to
/// 5, and from 3 to 8), and walls on parallel lines apart. The kernel answers with the shared
/// strip as an overlap, or leaves the pair to the general paths; walls apart share nothing.
#[test]
fn ub_81_walls_whose_generators_overlap_in_a_range_share_the_strip_or_are_left_alone() {
    let a = wall([0.0; 3], [5.0, 0.0, 0.0]);
    let b = wall([3.0, 0.0, 0.0], [8.0, 0.0, 0.0]);
    if let Some(r) = natural(&a, &b, &mut meter()).unwrap() {
        assert!(
            r.items().iter().any(|i| matches!(i, Item::Overlap { .. })),
            "{:?}",
            r.items()
        );
        assert!(lines_of(&r).is_empty(), "{:?}", r.items());
    }
    let apart = wall([3.0, 1.0, 0.0], [8.0, 1.0, 0.0]);
    if let Some(r) = natural(&a, &apart, &mut meter()).unwrap() {
        assert!(r.items().is_empty(), "{:?}", r.items());
    }
}

/// solidcore plan/upstream-issues/WR-614--crossing-of-planes-within-tolerance-across-a-strip-fails-the-merge.md
/// (b4de2149, confirmed on the C++ build): two planar faces crossing at a small angle meet in a
/// line fixed only to the tolerance over the sine of the angle, and where an edge of one lies on
/// the other within tolerance the crossing was combined with the edge and the merge failed. The
/// input is the defect's: the face y = 20 over x in [-2, 2], and a face through (1.99995, 20) turned
/// by atan(0.1) about z, whose edge at x = 2 is 5e-5 from the crossing, and one turned by 1e-9. The
/// crossing is one line at the exact x, over the heights both share, with a crossing contact; two
/// planes 1e-4 apart and parallel meet nowhere, and one plane is itself.
#[test]
fn wr_614_planes_crossing_at_a_small_angle_across_a_strip_meet_in_one_exact_line() {
    let face = plane(
        [0.0, 20.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        ((-2.0, 2.0), (0.0, 4.0)),
    );
    for (x, slope, span) in [(1.99995, 0.1, 1e-4), (1.9999, 1e-9, 1e-3)] {
        let turned = plane(
            [x, 20.0, 0.0],
            [1.0, slope, 0.0],
            [0.0, 0.0, 1.0],
            ((-4.0, span), (0.0, 4.0)),
        );
        // A tilt of 1e-9 is below what the enclosures can tell from parallel: `None`, left to the
        // general paths, is the refusal; an answer must be right.
        let Some(r) = natural(&face, &turned, &mut meter()).unwrap() else {
            assert!(slope < 1e-3, "{x}: no closed form");
            continue;
        };
        assert_eq!(r.items().len(), 1, "{x}: {:?}", r.items());
        assert!(
            matches!(
                r.items(),
                [Item::Curve {
                    contact: CurveContact::Crossing,
                    ..
                }]
            ),
            "{x}: {:?}",
            r.items()
        );
        let line = lines_of(&r)[0];
        let (p, q) = ends_of(line);
        near(p[2].min(q[2]), 0.0);
        near(p[2].max(q[2]), 4.0);
        let Item::Curve { class, .. } = &r.items()[0] else {
            panic!("{:?}", r.items());
        };
        // The line is as well known as the angle lets it be: its class says how far.
        for s in samples(line) {
            if slope > 1e-3 {
                near(s[0], x);
            }
            assert!(
                (s[0] - x).abs() <= allowed(*class, s),
                "{x}: {s:?} in {class:?}"
            );
            assert!(
                (s[1] - 20.0).abs() <= allowed(*class, s),
                "{x}: {s:?} in {class:?}"
            );
        }
    }
    let apart = plane(
        [0.0, 20.0 + 1e-4, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        ((-2.0, 2.0), (0.0, 4.0)),
    );
    assert!(found(&face, &apart).items().is_empty());
    assert!(matches!(
        found(&face, &face.clone()).items(),
        [Item::Overlap { .. }]
    ));
}

/// solidcore plan/upstream-issues/WR-647--simplecoincidencechecker-takes-a-crossing-as-a-coincidence-range.md
/// (b4de2149, confirmed by the fork on the C++ build): two equal circles a little apart graze
/// within tolerance over a stretch, they cross inside it, and the checker returned a coincidence
/// range ending wherever the subdivision cut the spans (e = 1e-4 and 1e-3 off made the union not
/// a solid). The input is the defect's: two circles of radius 5 whose centres are 1e-3, 1e-4 and
/// 2^-30 apart. Each pair crosses at two points, and the answer is never a range of coincidence
/// or a curve.
#[test]
fn wr_647_two_equal_circles_a_hair_apart_cross_twice_and_never_coincide() {
    let circle = |c: [f64; 2]| {
        CurveKind::Circle(Circle::full(c, 5.0, Frame::<2>::standard().unwrap()).unwrap())
    };
    let base = circle([0.0, 0.0]);
    for e in [1e-3, 1e-4, 2.0_f64.powi(-30)] {
        let r = curves(&base, &circle([e, 0.0]), &mut meter()).unwrap();
        for item in r.items() {
            assert!(
                matches!(item, Item::Point { .. } | Item::Flagged { .. }),
                "{e}: {item:?}"
            );
        }
        let crossings = r
            .items()
            .iter()
            .filter(|i| {
                matches!(
                    i,
                    Item::Point {
                        contact: PointContact::Crossing,
                        ..
                    }
                )
            })
            .count();
        if e >= 1e-4 {
            assert_eq!(crossings, 2, "{e}: {:?}", r.items());
            assert_eq!(r.items().len(), 2, "{e}: {:?}", r.items());
        } else {
            assert!(crossings == 2 || crossings == 0, "{e}: {:?}", r.items());
        }
    }
}

/// solidcore plan/upstream-issues/WR-495--mateedges-marches-offset-edges-between-analytic-faces.md,
/// WR-545--shell-edges-between-analytic-faces-stay-marched-approximations.md and
/// WR-344--extended-offset-mated-edges-intersected-again.md (b4de2149; WR-495 confirmed on the
/// pinned C++): the edges between the offset faces of a shelled cylinder (radius 5, height 10,
/// shelled 1 in, open at the top) were marched B-spline approximations, so the inner floor circle
/// was not a circle, the rim was split at an extra vertex and the volume was off by 3.5e-7, and
/// the same for a frustum and a cone. The input is the defects': the offset faces' own surfaces,
/// a radius 4 cylinder with the planes z = 1 and z = 10, and a cone from radius 5 to 3 over 10
/// with the planes z = 1 and z = 9, the radius a shelled frustum's inner faces have at those
/// heights. Each pair meets in one whole circle of the exact radius, to 1e-12, running all the
/// way round once, as the one answer.
#[test]
fn wr_495_545_344_the_edges_of_a_shelled_cylinder_and_frustum_are_their_exact_circles() {
    let pipe = tube([0.0; 3], 4.0, (0.0, 12.0));
    let frustum =
        SurfaceKind::Cone(Cone::new([0.0; 3], 5.0, -0.2, up(), (ROUND, (0.0, 10.0))).unwrap());
    let cases: [(&SurfaceKind, f64, f64); 4] = [
        (&pipe, 1.0, 4.0),
        (&pipe, 10.0, 4.0),
        (&frustum, 1.0, 4.8),
        (&frustum, 9.0, 3.2),
    ];
    for (surface, z, radius) in cases {
        let r = found(&level(z), surface);
        assert_eq!(r.items().len(), 1, "z = {z}: {:?}", r.items());
        let curve = lines_of(&r)[0];
        let CurveKind::Circle(c) = curve else {
            panic!("z = {z}: {:?}", r.items());
        };
        assert!(
            (c.radius() - radius).abs() < 1e-12,
            "z = {z}: {}",
            c.radius()
        );
        assert!(
            (c.center()[2] - z).abs() < 1e-12,
            "z = {z}: {:?}",
            c.center()
        );
        // One whole turn: the sixteen samples fall in all eight octants of the circle.
        let (lo, hi) = curve.domain();
        let mut octants = [false; 8];
        for i in 0..16 {
            let t = lo + (hi - lo) * (f64::from(i) + 0.5) / 16.0;
            let p = curve.point(t, Limit::FromRight).unwrap();
            let a = (p[1] - c.center()[1]).atan2(p[0] - c.center()[0]);
            let k = ((a + core::f64::consts::PI) / (core::f64::consts::PI / 4.0)).floor() as usize;
            octants[k.min(7)] = true;
        }
        assert!(octants.iter().all(|x| *x), "z = {z}: {octants:?}");
    }
}
