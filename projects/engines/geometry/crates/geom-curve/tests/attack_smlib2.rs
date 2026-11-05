//! Attack tests from SMLib's confirmed defects, batch 2: `geom-curve`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: the right box, point or kind, from an independent
//! formula, or a defined refusal, and never a panic, a hang, a NaN or a wrong value.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::manual_midpoint,
    clippy::type_complexity,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::needless_range_loop,
    clippy::range_minus_one,
    reason = "tests: native arithmetic, casts and indexing, platform maths on an independent oracle (the formula as it is written), prose that names defect files, and a failure stops the test"
)]

use geom_curve::{
    Axes, Basis, Circle, Composite, CurveError, Ellipse, Frame, Hyperbola, Line, Offset, OffsetLaw,
    Parabola, ParamCurve, Trimmed,
};
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 36))
}

const RIGHT: Limit = Limit::FromRight;
const LEFT: Limit = Limit::FromLeft;

fn near2(a: [f64; 2], b: [f64; 2], tol: f64) -> bool {
    (a[0] - b[0]).abs() <= tol && (a[1] - b[1]).abs() <= tol
}

fn near3(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= tol)
}

/// A curve built from extreme or odd input is either refused at construction, or it evaluates,
/// bounds and converts without a panic, a NaN or the kernel's own `Defect`: each call gives finite
/// numbers or a typed refusal.
fn sane<C: ParamCurve<2>>(built: Result<C, CurveError>, what: &str) {
    let c = match built {
        Ok(c) => c,
        Err(e) => {
            assert!(!matches!(e, CurveError::Defect), "{what}: {e:?}");
            return;
        }
    };
    let (lo, hi) = c.domain();
    for t in [lo, lo + (hi - lo) / 3.0, (lo + hi) / 2.0, hi] {
        for side in [RIGHT, LEFT] {
            match c.point(t, side) {
                Ok(p) => assert!(p.iter().all(|x| x.is_finite()), "{what}: point {p:?}"),
                Err(e) => assert!(!matches!(e, CurveError::Defect), "{what}: {e:?}"),
            }
        }
    }
    match c.bound((lo, hi), &mut meter()) {
        Ok(b) => {
            for d in 0..2 {
                assert!(
                    b.lo[d].is_finite() && b.hi[d].is_finite() && b.lo[d] <= b.hi[d],
                    "{what}: box {b:?}"
                );
            }
        }
        Err(e) => assert!(!matches!(e, CurveError::Defect), "{what}: {e:?}"),
    }
    match c.to_nurbs(&mut meter()) {
        Ok(n) => assert!(n.deviation.is_finite() && n.deviation >= 0.0, "{what}"),
        Err(e) => assert!(!matches!(e, CurveError::Defect), "{what}: {e:?}"),
    }
}

/// solidcore plan/upstream-issues/WR-34--smcompositecurve-calculateboundingbox-shifts-intervals-from.md
/// (b4de2149): the box of a composite curve shifted each segment's interval from the wrong
/// origin, so a composite whose first segment starts at t = 5 had the wrong box. The input is the
/// defect's: two segments, once with the composite starting at t = 5 and once with the first
/// segment's own interval starting at 5. The box of the whole and of every sub-span must hold the
/// curve at every sampled parameter and be no larger than the points it holds plus a small slack.
#[test]
fn wr_34_the_box_of_a_composite_starting_at_t_5_is_the_box_of_its_points() {
    // (5, 0) to (6, 0), then (6, 0) to (6, 2): infinite lines trimmed to parameters 5..6 and 0..2.
    let a = Line::<2>::infinite([0.0, 0.0], [1.0, 0.0]).unwrap();
    let b = Line::<2>::infinite([6.0, 0.0], [0.0, 1.0]).unwrap();
    let from_five = Composite::new(vec![
        Trimmed::new(Basis::Line(a.clone()), (5.0, 6.0)).unwrap(),
        Trimmed::new(Basis::Line(b.clone()), (0.0, 2.0)).unwrap(),
    ])
    .unwrap();
    assert_eq!(from_five.domain(), (0.0, 3.0));
    // The composite itself starting at 5: the same segments, domain (5, 8).
    let at_five = Composite::starting_at(
        vec![
            Trimmed::new(Basis::Line(a), (5.0, 6.0)).unwrap(),
            Trimmed::new(Basis::Line(b), (0.0, 2.0)).unwrap(),
        ],
        5.0,
    )
    .unwrap();
    assert_eq!(at_five.domain(), (5.0, 8.0));
    for c in [&from_five, &at_five] {
        let (lo, hi) = c.domain();
        // The curve: x runs 5 to 6 along y = 0 for a unit of parameter, then y runs 0 to 2.
        let whole = c.bound((lo, hi), &mut meter()).unwrap();
        assert!(whole.lo[0] <= 5.0 && whole.lo[0] > 5.0 - 1e-9, "{whole:?}");
        assert!(whole.hi[0] >= 6.0 && whole.hi[0] < 6.0 + 1e-9);
        assert!(whole.lo[1] <= 0.0 && whole.lo[1] > -1e-9);
        assert!(whole.hi[1] >= 2.0 && whole.hi[1] < 2.0 + 1e-9);
        // The first segment alone: the box is flat along y = 0, x from 5 to 6.
        let first = c.bound((lo, lo + 1.0), &mut meter()).unwrap();
        assert!(first.lo[0] <= 5.0 && first.hi[0] >= 6.0 && first.hi[0] < 6.0 + 1e-9);
        assert!(first.hi[1] < 1e-9 && first.lo[1] > -1e-9, "{first:?}");
        // The second alone: x is 6, y from 0 to 2.
        let second = c.bound((lo + 1.0, hi), &mut meter()).unwrap();
        assert!(
            second.lo[0] > 6.0 - 1e-9 && second.hi[0] < 6.0 + 1e-9,
            "{second:?}"
        );
        assert!(second.lo[1] <= 0.0 && second.hi[1] >= 2.0 && second.hi[1] < 2.0 + 1e-9);
        // Sub-spans: each box holds the sampled points, and is as tight as the points need.
        for (s0, s1) in [(0.2, 0.7), (0.5, 1.5), (1.25, 2.75), (0.0, 3.0)] {
            let (s0, s1) = (lo + s0, lo + s1);
            let b = c.bound((s0, s1), &mut meter()).unwrap();
            let mut seen = [[f64::MAX, f64::MIN]; 2];
            for k in 0..=60 {
                let t = s0 + (s1 - s0) * f64::from(k) / 60.0;
                let p = c.point(t, RIGHT).unwrap();
                for d in 0..2 {
                    assert!(
                        b.lo[d] <= p[d] && p[d] <= b.hi[d],
                        "t = {t}: {p:?} in {b:?}"
                    );
                    seen[d][0] = seen[d][0].min(p[d]);
                    seen[d][1] = seen[d][1].max(p[d]);
                }
            }
            for d in 0..2 {
                assert!(
                    b.lo[d] > seen[d][0] - 1e-6,
                    "span {s0}..{s1}: {b:?} over {seen:?}"
                );
                assert!(
                    b.hi[d] < seen[d][1] + 1e-6,
                    "span {s0}..{s1}: {b:?} over {seen:?}"
                );
            }
        }
        // A span that is not ordered, outside, or not finite is refused.
        for bad in [(hi, lo), (lo - 1.0, hi), (lo, hi + 1.0), (f64::NAN, hi)] {
            assert!(c.bound(bad, &mut meter()).is_err(), "{bad:?}");
        }
    }
}

/// solidcore plan/upstream-issues/UB-58--smcompositecurve-getknots-reads-past-its-array-and-loops.md,
/// UB-224--smcompositecurve-dim-curves-closed-senses-gaps-indexes.md and
/// UB-59--smcompositecurve-null-dereferences.md (b4de2149): a composite of two or more segments
/// read past its knot array, a composite built from no curves or from short sense or gap arrays
/// read past them, and a composite of unconnected segments was approximated through a null. The
/// inputs are the defects': no segments, two segments, two unconnected segments. Ours refuses no
/// segments (`Degenerate`) and unconnected ones (`Disjoint`), records the joint of connected ones
/// with its gap, and evaluates at every joint from both sides.
#[test]
fn ub_58_ub_224_ub_59_composites_of_no_two_and_unconnected_segments() {
    assert_eq!(
        Composite::<2>::new(Vec::new()).err(),
        Some(CurveError::Degenerate)
    );
    assert_eq!(
        Composite::<2>::starting_at(Vec::new(), 3.0).err(),
        Some(CurveError::Degenerate)
    );
    let seg = |a: [f64; 2], b: [f64; 2]| {
        Trimmed::whole(Basis::Line(
            Line::new(a, [b[0] - a[0], b[1] - a[1]], (0.0, 1.0)).unwrap(),
        ))
        .unwrap()
    };
    // Two connected segments: starts, one gap, the joint from each side.
    let two = Composite::new(vec![
        seg([0.0, 0.0], [1.0, 0.0]),
        seg([1.0, 0.0], [1.0, 3.0]),
    ])
    .unwrap();
    assert_eq!(two.starts(), &[0.0, 1.0, 2.0]);
    assert_eq!(two.gaps().len(), 1);
    assert_eq!(two.gaps()[0], 0.0);
    assert!(near2(two.point(1.0, LEFT).unwrap(), [1.0, 0.0], 1e-15));
    assert!(near2(two.point(1.0, RIGHT).unwrap(), [1.0, 0.0], 1e-15));
    assert!(near2(two.point(2.0, LEFT).unwrap(), [1.0, 3.0], 1e-15));
    assert!(near2(two.point(0.5, RIGHT).unwrap(), [0.5, 0.0], 1e-15));
    assert!(near2(two.point(1.5, RIGHT).unwrap(), [1.0, 1.5], 1e-15));
    // Outside the domain, from either side: refused.
    for bad in [-1e-9, 2.0 + 1e-9, f64::NAN] {
        assert!(two.point(bad, RIGHT).is_err(), "{bad}");
    }
    // It converts to one NURBS curve, with a finite deviation.
    let n = two.to_nurbs(&mut meter()).unwrap();
    assert!(n.deviation.is_finite() && n.deviation < 1e-12);
    // Unconnected segments are refused, however small the gap.
    for gap in [1e-3, 1e-9, 1e-12] {
        assert_eq!(
            Composite::new(vec![
                seg([0.0, 0.0], [1.0, 0.0]),
                seg([1.0, gap], [1.0, 3.0])
            ])
            .err(),
            Some(CurveError::Disjoint),
            "gap {gap}"
        );
    }
}

/// solidcore plan/upstream-issues/WR-32--smcircle-createsimpleoffset-never-shrinks-for-the-anti.md
/// (b4de2149): the simple offset of a circle never shrank for the antiparallel normal: with the
/// normal -Z the radius grew. The input is the defect's: a circle of radius 5 in the xy-plane,
/// offset by 1 with the reference direction +Z and then -Z. The left of the circle's
/// direction of travel is inward, so with V = +Z (tangent x V points outward) the offset circle has
/// radius 6, and with V = -Z it has radius 4; the sign of the distance flips it again.
#[test]
fn wr_32_an_offset_against_the_antiparallel_reference_shrinks_the_circle() {
    let r = 5.0;
    let circle = Circle::<3>::new([0.0; 3], r, Frame::standard().unwrap(), (0.0, 1.0)).unwrap();
    let basis = Trimmed::new(Basis::Circle(circle), (0.0, 1.0)).unwrap();
    let offset = |distance: f64, v: [f64; 3]| {
        Offset::new(
            basis.clone(),
            OffsetLaw::Reference {
                distance,
                direction: v,
            },
        )
        .unwrap()
    };
    let cases = [
        (1.0, [0.0, 0.0, 1.0], 6.0),
        (1.0, [0.0, 0.0, -1.0], 4.0),
        (-1.0, [0.0, 0.0, 1.0], 4.0),
        (-1.0, [0.0, 0.0, -1.0], 6.0),
        // A reference that is not a unit vector, and one that is not along z at all but still
        // perpendicular to the plane's tangents' cross: only its direction counts.
        (1.0, [0.0, 0.0, 50.0], 6.0),
        (1.0, [0.0, 0.0, -0.001], 4.0),
    ];
    for (d, v, radius) in cases {
        let o = offset(d, v);
        for (t, dir) in [(0.0, [1.0, 0.0]), (0.5, [0.0, 1.0]), (1.0, [-1.0, 0.0])] {
            let p = o
                .point(t, RIGHT)
                .unwrap_or_else(|_| o.point(t, LEFT).unwrap());
            let want = [dir[0] * radius, dir[1] * radius, 0.0];
            assert!(
                near3(p, want, 1e-9),
                "d = {d}, v = {v:?}, t = {t}: {p:?} against {want:?}"
            );
        }
        // A quarter and an eighth of the way round, at the right radius.
        for t in [0.25, 0.125, 0.8] {
            let p = o.point(t, RIGHT).unwrap();
            let rr = (p[0] * p[0] + p[1] * p[1]).sqrt();
            assert!(
                (rr - radius).abs() < 1e-9,
                "d = {d}, v = {v:?}, t = {t}: radius {rr}"
            );
            assert!(p[2].abs() < 1e-12);
        }
    }
    // A zero reference direction has no side: refused.
    assert_eq!(
        Offset::new(
            basis.clone(),
            OffsetLaw::Reference {
                distance: 1.0,
                direction: [0.0; 3]
            }
        )
        .err(),
        Some(CurveError::Degenerate)
    );
}

/// solidcore plan/upstream-issues/WR-31--smellipse-convertto2d-uses-dthetaxrad-for-both-axes.md
/// (b4de2149): projecting an ellipse whose axes are not perpendicular gave the projection equal
/// axes, because the second axis was built with the first's angle. The input is the defect's: an
/// ellipse whose two axis vectors are not perpendicular, (2, 0) and (1, 1.5). It has the point
/// c + cos(pi t) u + sin(pi t) v at every t, its box holds all of them, and its NURBS form is the
/// same ellipse within its deviation.
#[test]
fn wr_31_an_ellipse_with_oblique_axes_keeps_both_its_axes() {
    let c = [3.0, -1.0];
    let (u, v) = ([2.0, 0.0], [1.0, 1.5]);
    let e = Ellipse::<2>::new(c, Axes::Vectors { u, v }, (0.0, 2.0)).unwrap();
    let at = |t: f64| {
        let (cs, sn) = (
            (std::f64::consts::PI * t).cos(),
            (std::f64::consts::PI * t).sin(),
        );
        [c[0] + cs * u[0] + sn * v[0], c[1] + cs * u[1] + sn * v[1]]
    };
    for k in 0..=24 {
        let t = f64::from(k) / 12.0;
        assert!(near2(e.point(t, RIGHT).unwrap(), at(t), 1e-12), "t = {t}");
    }
    // The axes are kept: t = 0 is c + u and t = 1/2 is c + v, not equal lengths.
    assert!(near2(e.point(0.0, RIGHT).unwrap(), [5.0, -1.0], 1e-13));
    assert!(near2(e.point(0.5, RIGHT).unwrap(), [4.0, 0.5], 1e-13));
    let b = e.bound((0.0, 2.0), &mut meter()).unwrap();
    for k in 0..=200 {
        let p = at(f64::from(k) / 100.0);
        assert!(
            b.lo[0] <= p[0] && p[0] <= b.hi[0] && b.lo[1] <= p[1] && p[1] <= b.hi[1],
            "{p:?}"
        );
    }
    let n = e.to_nurbs(&mut meter()).unwrap();
    assert!(
        n.deviation.is_finite() && n.deviation < 1e-9,
        "{}",
        n.deviation
    );
    // Parallel axes are not an ellipse: refused.
    assert_eq!(
        Ellipse::<2>::new(
            c,
            Axes::Vectors {
                u: [1.0, 2.0],
                v: [-2.0, -4.0]
            },
            (0.0, 2.0)
        )
        .err(),
        Some(CurveError::Degenerate)
    );
    assert_eq!(
        Ellipse::<2>::new(
            c,
            Axes::Vectors {
                u: [1.0, 2.0],
                v: [1.0, 2.0]
            },
            (0.0, 2.0)
        )
        .err(),
        Some(CurveError::Degenerate)
    );
    assert_eq!(
        Ellipse::<2>::new(
            c,
            Axes::Vectors {
                u: [0.0, 0.0],
                v: [1.0, 2.0]
            },
            (0.0, 2.0)
        )
        .err(),
        Some(CurveError::Degenerate)
    );
}

/// solidcore plan/upstream-issues/WR-211--smellipse-adjuststepinterval-collapses-an-arc-moved-a-turn.md
/// (b4de2149): an elliptical arc whose sweep domain starts a whole turn past 0 (360 degrees) was
/// collapsed when its interval was adjusted. The input is the defect's: arcs of an ellipse over
/// the quarter turn starting at 1 turn (t = 2), and at 2, -1 and -2 turns, and far away. Ours is
/// evaluated where it is: the start is at c + (a, 0) and the quarter turn on is at c + (0, b).
#[test]
fn wr_211_an_elliptical_arc_a_whole_turn_from_zero_is_not_collapsed() {
    let c = [1.0, 2.0];
    let (a, b) = (4.0, 2.0);
    let frame = Frame::<2>::standard().unwrap();
    for start in [2.0, 4.0, -2.0, -4.0, 1000.0, 1e6] {
        let arc = Ellipse::<2>::new(c, Axes::Frame { frame, a, b }, (start, start + 0.5)).unwrap();
        assert_eq!(arc.domain(), (start, start + 0.5));
        assert!(
            near2(arc.point(start, RIGHT).unwrap(), [c[0] + a, c[1]], 1e-9),
            "start {start}"
        );
        assert!(
            near2(
                arc.point(start + 0.5, LEFT).unwrap(),
                [c[0], c[1] + b],
                1e-9
            ),
            "start {start}"
        );
        let bx = arc.bound((start, start + 0.5), &mut meter()).unwrap();
        assert!(bx.lo[0] <= c[0] && bx.hi[0] >= c[0] + a - 1e-9);
        assert!(bx.lo[1] <= c[1] && bx.hi[1] >= c[1] + b - 1e-9);
        // The arc is a quarter turn of the ellipse: its box is the quarter's, not a whole turn's.
        if start.abs() <= 4.0 {
            assert!(
                bx.lo[0] > c[0] - 1e-6 && bx.hi[0] < c[0] + a + 1e-6,
                "{bx:?}"
            );
            assert!(
                bx.lo[1] > c[1] - 1e-6 && bx.hi[1] < c[1] + b + 1e-6,
                "{bx:?}"
            );
        }
        match arc.to_nurbs(&mut meter()) {
            Ok(n) => assert!(
                n.deviation.is_finite() && n.deviation >= 0.0,
                "start {start}"
            ),
            Err(e) => assert!(!matches!(e, CurveError::Defect), "start {start}: {e:?}"),
        }
    }
    // More than one turn of domain is refused.
    assert!(Ellipse::<2>::new(c, Axes::Frame { frame, a, b }, (2.0, 4.5)).is_err());
}

/// solidcore plan/upstream-issues/UB-46--nurb-less-curves-are-dereferenced.md and
/// UB-46--addendum-2026-11-01-ellipse-axis-overflow.md, with UB-57--conic-and-ellipse-uninitialised-reads.md
/// (b4de2149): a hyperbola, parabola or ellipse built from extreme but finite input (a semi axis of
/// 1e300, a centre at 9.2e18 with unit axes, an axis component of 1.34e154 or more, a zero focal
/// distance, a degenerate pair of axes) was left without its NURB and dereferenced later. The inputs
/// are the defects': each must be refused at construction, or evaluate, bound and convert to
/// finite numbers or a typed refusal, and never reach `Defect`.
#[test]
fn ub_46_ub_57_conics_from_extreme_finite_input_are_refused_or_finite() {
    let frame = Frame::<2>::standard().unwrap();
    let centres = [[0.0, 0.0], [9.2e18, 9.2e18], [-1e300, 1e300]];
    let sizes = [1.0, 1e-300, 1e150, 1.34e154, 1e200, 1e300, f64::MAX];
    for c in centres {
        for s in sizes {
            let axes = Axes::Frame { frame, a: s, b: s };
            sane(Circle::<2>::new(c, s, frame, (0.0, 1.0)), "circle");
            sane(Ellipse::<2>::new(c, axes, (0.0, 1.0)), "ellipse");
            sane(Ellipse::<2>::new(c, axes, (0.0, 2.0)), "full ellipse");
            sane(Hyperbola::<2>::new(c, axes, (-1.0, 1.0)), "hyperbola");
            sane(Parabola::<2>::new(c, axes, (-1.0, 1.0)), "parabola");
            sane(
                Parabola::<2>::focal(c, s, frame, (-1.0, 1.0)),
                "focal parabola",
            );
            let vectors = Axes::Vectors {
                u: [s, 0.0],
                v: [0.0, s],
            };
            sane(Ellipse::<2>::new(c, vectors, (0.0, 1.0)), "vector ellipse");
            sane(
                Hyperbola::<2>::new(c, vectors, (-1.0, 1.0)),
                "vector hyperbola",
            );
        }
        // A zero or negative size, and a degenerate pair of axes.
        for s in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(
                Circle::<2>::new(c, s, frame, (0.0, 1.0)).is_err(),
                "radius {s}"
            );
            assert!(
                Parabola::<2>::focal(c, s, frame, (-1.0, 1.0)).is_err(),
                "focal {s}"
            );
            assert!(
                Hyperbola::<2>::new(
                    c,
                    Axes::Frame {
                        frame,
                        a: s,
                        b: 1.0
                    },
                    (-1.0, 1.0)
                )
                .is_err(),
                "semi axis {s}"
            );
        }
        let parallel = Axes::Vectors {
            u: [1.0, 1.0],
            v: [2.0, 2.0],
        };
        assert!(Hyperbola::<2>::new(c, parallel, (-1.0, 1.0)).is_err());
        assert!(Parabola::<2>::new(c, parallel, (-1.0, 1.0)).is_err());
        assert!(Ellipse::<2>::new(c, parallel, (0.0, 1.0)).is_err());
    }
}
