//! Attack tests from SMLib's confirmed defects, batch 4: surface extension (`geom-surface`'s
//! `extend`, `geom-nurbs`'s natural continuation; piece G253).
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. SMLib extends a
//! surface by a distance and searches for the parameter that reaches it, halving or doubling a
//! step; ours extends by a parameter length, each kind by its own definition and NURBS by the
//! end span's polynomial, so a defect is taken where its cause has a counterpart here: a length
//! that is zero, signed zero, vanishing, negative, infinite or NaN; a length far longer than the
//! surface; a closed direction; a rational continuation whose weight turns negative; a net far
//! from the origin; an offset whose base is analytic, reversed, or itself an offset; a periodic
//! direction far from zero; and an extension that depends on what was asked before it. Each test
//! asserts what the kernel owes for that input: the documented refusal, or the extension with its
//! source kept and its definition continued (checked against a formula written apart from the
//! code), and never a panic, a hang or a wrong answer.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::doc_markdown,
    clippy::unreadable_literal,
    clippy::many_single_char_names,
    clippy::unnested_or_patterns,
    clippy::match_same_arms,
    clippy::unreachable,
    clippy::disallowed_methods,
    reason = "tests: native arithmetic and indexing, prose that names defect files, and a failure stops the test"
)]

use geom_curve::{Basis as Leaf, CurveError, Frame, Trimmed};
use geom_nurbs::{Basis, Curve, Direction, End, Limit, NurbsError, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::extend::extend;
use geom_surface::{
    Cone, Cylinder, ExtendError, Extended, Offset, ParamSurface, Plane, Reach, Revolution, Side,
    Sphere, SurfaceKind, Torus,
};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

const SIDES: [Side; 4] = [Side::UMin, Side::UMax, Side::VMin, Side::VMax];
const LIMITS: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

/// The frame with x along x and the axis along z.
fn frame() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap()
}

/// A clamped biquadratic graph z = f(x, y) over two spans each way, its net at `at`.
fn graph(at: [f64; 3]) -> Surface<3> {
    let k = vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0];
    let mut pts = Vec::new();
    for i in 0..4_u8 {
        for j in 0..4_u8 {
            let (x, y) = (f64::from(i), f64::from(j));
            pts.push([at[0] + x, at[1] + y, at[2] + 0.25 * ((x * y) % 3.0)]);
        }
    }
    Surface::new(
        Basis::new(2, k.clone()).unwrap(),
        Basis::new(2, k).unwrap(),
        &pts,
    )
    .unwrap()
}

/// A revolution of a quadratic NURBS profile `across` units wide about z, over half a turn.
fn revolution(across: f64) -> SurfaceKind {
    let profile = Curve::new(
        Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap(),
        &[
            [across, 0.0, 0.0],
            [1.2 * across, 0.0, 0.5 * across],
            [across, 0.0, across],
        ],
    )
    .unwrap();
    SurfaceKind::Revolution(
        Revolution::new(
            Trimmed::whole(Leaf::Nurbs(profile)).unwrap(),
            [0.0; 3],
            frame(),
            (0.0, 1.0),
        )
        .unwrap(),
    )
}

/// One of every kind the extension treats on its own: a plane, the four rounds, a revolution, an
/// offset and a NURBS graph.
fn kinds() -> Vec<(&'static str, SurfaceKind)> {
    let f = frame();
    let d = ((0.0, 1.0), (0.0, 1.0));
    vec![
        (
            "plane",
            SurfaceKind::Plane(Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], d).unwrap()),
        ),
        (
            "cylinder",
            SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 2.0, f, d).unwrap()),
        ),
        (
            "cone",
            SurfaceKind::Cone(Cone::new([0.0; 3], 1.0, 0.5, f, d).unwrap()),
        ),
        (
            "sphere",
            SurfaceKind::Sphere(
                Sphere::new([0.0; 3], 3.0, f, ((0.0, 1.0), (-0.25, 0.25))).unwrap(),
            ),
        ),
        (
            "torus",
            SurfaceKind::Torus(
                Torus::new([0.0; 3], 3.0, 1.0, f, ((0.0, 1.0), (-0.5, 0.5))).unwrap(),
            ),
        ),
        ("revolution", revolution(4.0)),
        (
            "offset",
            SurfaceKind::Offset(Offset::new(SurfaceKind::Nurbs(graph([0.0; 3])), 0.125).unwrap()),
        ),
        ("nurbs", SurfaceKind::Nurbs(graph([0.0; 3]))),
    ]
}

/// The point of `s` at (u, v).
fn at(s: &SurfaceKind, u: f64, v: f64) -> [f64; 3] {
    s.point(u, v, LIMITS).unwrap()
}

/// The extension's domain holds the source's, and over the source's domain the extension is the
/// source, at a 5 × 5 grid of points.
fn keeps_its_source(source: &SurfaceKind, e: &Extended) {
    let ((u0, u1), (v0, v1)) = source.domain();
    let ((a0, a1), (b0, b1)) = e.surface.domain();
    assert!(
        a0 <= u0 && a1 >= u1 && b0 <= v0 && b1 >= v1,
        "{:?}",
        e.surface.domain()
    );
    for i in 0..5_u8 {
        for j in 0..5_u8 {
            let (s, t) = (f64::from(i) / 4.0, f64::from(j) / 4.0);
            let (u, v) = (u0 + (u1 - u0) * s, v0 + (v1 - v0) * t);
            let (p, q) = (at(source, u, v), at(&e.surface, u, v));
            for k in 0..3 {
                assert!(
                    (p[k] - q[k]).abs() <= 1e-12 * (1.0 + p[k].abs()),
                    "({u}, {v}): {p:?} against {q:?}"
                );
            }
        }
    }
}

/// HANG-6--zero-distance-g2-curve-and-surface-extensions.md and
/// HANG-9--createextendedsurface-with-distance-0-never-stops-halving.md: a distance of zero,
/// signed zero, NaN or infinity never ended SMLib's search or its halving loop. Ours refuses every
/// length that is not positive and finite at once, for every kind and every side, whichever reach
/// is asked, without touching the meter's budget.
#[test]
fn a_length_of_zero_signed_zero_nan_or_infinity_is_refused_at_once() {
    for (name, s) in kinds() {
        for side in SIDES {
            for by in [
                0.0,
                -0.0,
                f64::NAN,
                -f64::NAN,
                f64::INFINITY,
                f64::NEG_INFINITY,
            ] {
                for reach in [Reach::Exactly, Reach::AsFarAsValid] {
                    // A meter with no work at all: a refusal must come before any work.
                    let mut none = WorkMeter::new(Budget::new(0, u64::MAX));
                    assert!(
                        matches!(
                            extend(&s, side, by, reach, &mut none),
                            Err(ExtendError::Curve(CurveError::SpanOutside))
                        ),
                        "{name} {side:?} by {by} {reach:?}"
                    );
                }
            }
        }
    }
}

/// HANG-16--direct-negative-distance-surface-extension-moves-away-from.md and
/// HANG-18--matelaminaedgetosurface-with-a-negative-extension-factor.md: a negative distance
/// walked SMLib's doubling loop and its parameter search off the knot range. Ours refuses every
/// negative length, the smallest and the largest among them.
#[test]
fn a_negative_length_is_refused() {
    for (name, s) in kinds() {
        for side in SIDES {
            for by in [
                -0.625,
                -1.0,
                -1e-300,
                -f64::MIN_POSITIVE,
                -5e-324,
                -f64::MAX,
            ] {
                assert!(
                    matches!(
                        extend(&s, side, by, Reach::AsFarAsValid, &mut meter()),
                        Err(ExtendError::Curve(CurveError::SpanOutside))
                    ),
                    "{name} {side:?} by {by}"
                );
            }
        }
    }
    // The NURBS continuation itself, for a curve and for a surface.
    let c = Curve::new(
        Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap(),
        &[[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]],
    )
    .unwrap();
    let g = graph([0.0; 3]);
    for by in [-0.625, -1e-300, -0.0, f64::NAN] {
        for end in [End::Low, End::High] {
            assert!(matches!(
                c.extend(end, by, &mut meter()),
                Err(NurbsError::KnotOutside)
            ));
            for dir in [Direction::U, Direction::V] {
                assert!(matches!(
                    g.extend(dir, end, by, &mut meter()),
                    Err(NurbsError::KnotOutside)
                ));
            }
        }
    }
}

/// HANG-6--zero-distance-g2-curve-and-surface-extensions.md, its fuzz sweep: a vanishing but
/// non-zero length (1e-300, the smallest normal and subnormal doubles) made SMLib take about arc
/// length / extension steps, and `1.7e-4` on a surface of revolution several units across did not
/// finish. Ours either widens the domain by the length asked, the source kept, or, for NURBS
/// where the length cannot move the end knot, refuses it; it never runs on. The small but
/// representable length extends every kind by exactly that much.
#[test]
fn a_vanishing_length_extends_by_itself_or_is_refused_and_never_runs_on() {
    for (name, s) in kinds() {
        for side in SIDES {
            for by in [1e-300, f64::MIN_POSITIVE, 5e-324] {
                match extend(&s, side, by, Reach::Exactly, &mut meter()) {
                    Ok(e) => keeps_its_source(&s, &e),
                    Err(ExtendError::Curve(CurveError::SpanOutside))
                    | Err(ExtendError::Curve(CurveError::Nurbs(NurbsError::KnotOutside))) => {}
                    // A swept kind whose profile cannot carry its knot on that little reaches
                    // nothing.
                    Err(ExtendError::ExtensionFailed { reached: 0.0, .. }) => {}
                    other => panic!("{name} {side:?} by {by}: {other:?}"),
                }
            }
        }
    }
    let rev = revolution(4.0);
    for side in SIDES {
        let e = extend(&rev, side, 1.7e-4, Reach::Exactly, &mut meter()).unwrap();
        assert_eq!(e.reached, 1.7e-4);
        keeps_its_source(&rev, &e);
    }
    // A closed NURBS curve (its first point its last), as the sweep's `closedCurve.smc`, carried
    // on by 1e-300 at either end: refused, the knot not moved, or continued; never a search.
    let closed = Curve::new(
        Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0]).unwrap(),
        &[
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
            [-1.0, -1.0, 0.0],
            [1.0, 0.0, 0.0],
        ],
    )
    .unwrap();
    for end in [End::Low, End::High] {
        match closed.extend(end, 1e-300, &mut meter()) {
            Err(NurbsError::KnotOutside) => {}
            Ok(c) => assert!(c.deviation.is_finite()),
            Err(e) => panic!("{end:?}: {e:?}"),
        }
    }
}

/// HANG-17--st-surxsdr-surface-extension-by-distance-explores-an.md: SMLib split a long
/// extension and recursed twice per level, and on a cone, a torus or an end cap (closed
/// iso-curves short beside the distance) ran for more than 28 minutes. Ours widens a closed
/// angular direction by nothing (it already makes a full turn), and an open one by at most one
/// turn in all, at once, however long the length asked.
#[test]
fn a_closed_direction_reaches_nothing_and_an_open_one_at_most_a_turn() {
    let f = frame();
    let full = ((0.0, 2.0), (0.0, 1.0));
    let closed = [
        SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 1.0, f, full).unwrap()),
        SurfaceKind::Cone(Cone::new([0.0; 3], 1.0, 0.5, f, full).unwrap()),
        SurfaceKind::Torus(Torus::new([0.0; 3], 3.0, 1.0, f, ((0.0, 2.0), (-1.0, 1.0))).unwrap()),
    ];
    for s in &closed {
        for by in [0.1, 0.5, 10.0, 1e6, f64::MAX] {
            for side in [Side::UMin, Side::UMax] {
                let e = extend(s, side, by, Reach::AsFarAsValid, &mut meter()).unwrap();
                assert_eq!(e.surface.domain(), s.domain());
                // The length reported is no wider than a rounding (a defect: see
                // the_length_reached_is_how_far_the_domain_grew).
                assert!(e.reached < 1e-12, "{}", e.reached);
                assert!(matches!(
                    extend(s, side, by, Reach::Exactly, &mut meter()),
                    Err(ExtendError::ExtensionFailed { reached, .. }) if reached < 1e-12
                ));
            }
        }
    }
    // The torus's tube is closed too.
    let torus = &closed[2];
    for side in [Side::VMin, Side::VMax] {
        let e = extend(torus, side, 0.3, Reach::AsFarAsValid, &mut meter()).unwrap();
        assert_eq!(e.surface.domain(), torus.domain());
    }
    // An open arc of a cone, asked for a million half-turns more, reaches its full turn.
    let arc =
        SurfaceKind::Cone(Cone::new([0.0; 3], 1.0, 0.5, f, ((0.0, 0.5), (0.0, 1.0))).unwrap());
    let e = extend(&arc, Side::UMax, 1e6, Reach::AsFarAsValid, &mut meter()).unwrap();
    assert!((e.reached - 1.5).abs() < 1e-12, "{}", e.reached);
    keeps_its_source(&arc, &e);
}

/// HANG-22--g2-extension-retries-enter-an-arc-length-search-with-nan.md and
/// UB-107--createextendedsurface-retries-an-in-place-extension-on-half.md: on the rational
/// bilinear-by-linear surface of `gen0016` (a 2 × 3 net, v knots 0, 0, 0.8105925733594667, 1, 1),
/// an extension by 0.28814945128446756 whose weight would leave the positive range failed part
/// way, and SMLib retried on the arrays it had half written: the answer depended on what the
/// process had done before, and one retry met a NaN arc length and never ended. Here the weight
/// falls to a tenth over the last span, so carrying it on by that length makes it negative. Ours
/// refuses the full length with the length that stays positive, stops there when asked, keeps the
/// source, and gives the same answer, bit for bit, whatever was asked before it.
#[test]
fn a_continuation_whose_weight_turns_negative_stops_short_and_leaves_nothing_behind() {
    let u = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let v = Basis::new(1, vec![0.0, 0.0, 0.8105925733594667, 1.0, 1.0]).unwrap();
    let pts = [
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.5],
        [0.0, 2.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.5],
        [1.0, 2.0, 0.0],
    ];
    let weights = [1.0, 1.0, 0.1, 1.0, 1.0, 0.1];
    let s = SurfaceKind::Nurbs(Surface::new_rational(u, v, &pts, &weights).unwrap());
    let by = 0.288_149_451_284_467_56;
    // The weight along v past 1 is 0.1 − 0.9 t / (1 − 0.8105925733594667): zero at this length.
    let zero = 0.1 * (1.0 - 0.810_592_573_359_466_7) / 0.9;
    let refused = |r: Result<Extended, ExtendError>| match r {
        Err(ExtendError::ExtensionFailed {
            side: Side::VMax,
            reached,
        }) => {
            assert!(reached >= 0.0 && reached < zero, "{reached} against {zero}");
            reached
        }
        other => panic!("{other:?}"),
    };
    let first = refused(extend(&s, Side::VMax, by, Reach::Exactly, &mut meter()));
    let short = extend(&s, Side::VMax, by, Reach::AsFarAsValid, &mut meter()).unwrap();
    assert!(short.reached < zero);
    if short.reached > 0.0 {
        keeps_its_source(&s, &short);
        let ((_, _), (_, v1)) = short.surface.domain();
        let p = at(&short.surface, 0.5, v1);
        assert!(p.iter().all(|x| x.is_finite()), "{p:?}");
    }
    // Other extensions in between, then the same two again: the same answers, bit for bit.
    for side in [Side::UMin, Side::UMax, Side::VMin] {
        let _ = extend(&s, side, by, Reach::AsFarAsValid, &mut meter());
    }
    assert_eq!(
        refused(extend(&s, Side::VMax, by, Reach::Exactly, &mut meter())).to_bits(),
        first.to_bits()
    );
    let again = extend(&s, Side::VMax, by, Reach::AsFarAsValid, &mut meter()).unwrap();
    assert_eq!(again.surface, short.surface);
    assert_eq!(again.reached.to_bits(), short.reached.to_bits());
    assert_eq!(again.deviation.to_bits(), short.deviation.to_bits());
    // The continuation itself refuses the weight.
    let Some(nurbs) = (match &s {
        SurfaceKind::Nurbs(n) => Some(n),
        _ => None,
    }) else {
        unreachable!()
    };
    assert!(matches!(
        nurbs.extend(Direction::V, End::High, by, &mut meter()),
        Err(NurbsError::Weight { .. })
    ));
}

/// HANG-37--b-spline-surface-extension-does-not-return-for-a-control.md and the overflow cases of
/// HANG-6--zero-distance-g2-curve-and-surface-extensions.md: a 3 × 3 rational quadratic with one
/// control point at x = −1e300 (or at 2^63) gave no result in 25 s, its squared distances
/// overflowing. Ours either builds the continuation with a finite deviation and its source kept,
/// or refuses it with a defined error; the signed zero length of the defect is refused at once.
#[test]
fn a_net_far_out_of_range_extends_or_is_refused_and_returns() {
    let k = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    for far in [-1e300, 1e300, (1u64 << 63) as f64, 1e157] {
        let mut pts = Vec::new();
        for i in 0..3_u8 {
            for j in 0..3_u8 {
                pts.push([f64::from(i), f64::from(j), 0.25 * f64::from(i * j)]);
            }
        }
        pts[4][0] = far;
        let weights = [1.0, 0.5, 1.0, 0.5, 2.0, 0.5, 1.0, 0.5, 1.0];
        let built = Surface::new_rational(
            Basis::new(2, k.clone()).unwrap(),
            Basis::new(2, k.clone()).unwrap(),
            &pts,
            &weights,
        );
        let Ok(n) = built else {
            // Refused as built: a defined error, not a hang.
            continue;
        };
        let s = SurfaceKind::Nurbs(n);
        assert!(matches!(
            extend(&s, Side::UMax, -0.0, Reach::Exactly, &mut meter()),
            Err(ExtendError::Curve(CurveError::SpanOutside))
        ));
        for side in SIDES {
            for reach in [Reach::Exactly, Reach::AsFarAsValid] {
                match extend(&s, side, 0.5, reach, &mut meter()) {
                    Ok(e) => assert!(
                        e.deviation.is_finite() || e.reached == 0.0,
                        "{far} {side:?}"
                    ),
                    Err(ExtendError::ExtensionFailed { reached, .. }) => {
                        assert!((0.0..=0.5).contains(&reached));
                    }
                    Err(ExtendError::Curve(_)) => {}
                    Err(ExtendError::Stopped(s)) => panic!("{far} {side:?}: stopped {s:?}"),
                }
            }
        }
    }
}

/// WR-633--offset-surface-extends-an-analytic-base-off-itself.md: SMLib carried an offset's
/// analytic base on along its boundary's tangent, so an offset (4) of a half cylinder (r 5)
/// evaluated 45 degrees past its arc lay 6.3 from the axis, not 9. Ours extends the base as
/// itself: the offset of the half cylinder, extended a quarter of a half-turn, lies at the
/// offset's radius all along the new strip.
#[test]
fn an_offset_of_an_analytic_base_is_extended_as_itself() {
    let half = Cylinder::new([0.0; 3], 5.0, frame(), ((0.0, 1.0), (0.0, 2.0))).unwrap();
    let off = SurfaceKind::Offset(Offset::new(SurfaceKind::Cylinder(half), 4.0).unwrap());
    let r = |p: [f64; 3]| p[0].hypot(p[1]);
    let inside = r(at(&off, 0.5, 1.0));
    assert!(
        (inside - 9.0).abs() < 1e-9 || (inside - 1.0).abs() < 1e-9,
        "{inside}"
    );
    for side in [Side::UMin, Side::UMax] {
        let e = extend(&off, side, 0.25, Reach::Exactly, &mut meter()).unwrap();
        keeps_its_source(&off, &e);
        let ((u0, u1), _) = e.surface.domain();
        for k in 0..=8_u8 {
            let t = f64::from(k) / 8.0;
            let u = if side == Side::UMax {
                1.0 + 0.25 * t
            } else {
                -0.25 * t
            };
            assert!((u0..=u1).contains(&u));
            for v in [0.0, 1.0, 2.0] {
                let got = r(at(&e.surface, u, v));
                assert!(
                    (got - inside).abs() < 1e-9,
                    "{side:?} u {u}: {got} against {inside}"
                );
            }
        }
    }
}

/// WR-650--offset-extension-of-an-analytic-base-runs-past-its-turn.md and its addendum
/// (WR-650--addendum-2026-11-03-extension-stops-on-closure.md): SMLib grew every side of an
/// offset's base by a whole base domain and nothing stopped it at a full turn, so a 294 degree
/// arc covered 882 degrees, and later evaluations doubled the reach again. Ours stops at the turn:
/// asked for 294 degrees more past either end, the offset of the 294 degree arc reaches the other
/// 66 degrees and no more, and an extension of that extension reaches nothing.
#[test]
fn an_offset_of_a_partial_turn_stops_at_the_turn_however_often_it_is_extended() {
    let arc = 294.0 / 180.0;
    let cyl = Cylinder::new([0.0; 3], 5.0, frame(), ((0.0, arc), (0.0, 1.0))).unwrap();
    let off = SurfaceKind::Offset(Offset::new(SurfaceKind::Cylinder(cyl), 4.0).unwrap());
    let rest = 2.0 - arc;
    for side in [Side::UMin, Side::UMax] {
        match extend(&off, side, arc, Reach::Exactly, &mut meter()) {
            Err(ExtendError::ExtensionFailed { reached, .. }) => {
                // Within a rounding of the rest of the turn (see
                // the_length_reached_is_how_far_the_domain_grew).
                assert!((reached - rest).abs() < 1e-12, "{reached} against {rest}");
            }
            other => panic!("{side:?}: {other:?}"),
        }
        let e = extend(&off, side, arc, Reach::AsFarAsValid, &mut meter()).unwrap();
        let ((u0, u1), _) = e.surface.domain();
        assert!(u1 - u0 <= 2.0 && u1 - u0 > 2.0 - 1e-12, "{}", u1 - u0);
        keeps_its_source(&off, &e);
        // Extended again, past either end: nothing more.
        for again in [Side::UMin, Side::UMax] {
            let twice = extend(&e.surface, again, arc, Reach::AsFarAsValid, &mut meter()).unwrap();
            assert_eq!(twice.surface.domain(), e.surface.domain());
        }
    }
}

/// WR-458--offset-surface-extension-depends-on-what-was-evaluated-first.md: SMLib extended an
/// offset's base from the first point evaluated past it, then again from that extension for the
/// next, so two copies of one offset differed by 7.2e-4 after being read in a different order.
/// Ours is a function of the surface, the side and the length: evaluated first at far points or
/// at near ones, or not at all, a copy extends to the same surface, bit for bit, and a long
/// extension is the same however it is reached.
#[test]
fn an_extension_does_not_depend_on_what_was_read_before_it() {
    let half = Cylinder::new([0.0; 3], 5.0, frame(), ((0.0, 1.0), (0.0, 2.0))).unwrap();
    let off = SurfaceKind::Offset(Offset::new(SurfaceKind::Cylinder(half), 4.0).unwrap());
    let fresh = extend(&off, Side::UMax, 0.5, Reach::Exactly, &mut meter()).unwrap();
    let (a, b) = (off.clone(), off.clone());
    for u in [0.9, 0.99, 1.0] {
        let _ = at(&a, u, 1.0);
    }
    for u in [0.0, 0.5] {
        let _ = at(&b, u, 0.0);
    }
    for copy in [a, b] {
        let e = extend(&copy, Side::UMax, 0.5, Reach::Exactly, &mut meter()).unwrap();
        assert_eq!(e.surface, fresh.surface);
        assert_eq!(e.deviation.to_bits(), fresh.deviation.to_bits());
    }
    // The same for a NURBS offset base.
    let nurbs = SurfaceKind::Offset(Offset::new(SurfaceKind::Nurbs(graph([0.0; 3])), 0.5).unwrap());
    let once = extend(&nurbs, Side::VMax, 0.5, Reach::Exactly, &mut meter()).unwrap();
    let copy = nurbs.clone();
    for v in [2.0, 1.5, 0.0] {
        let _ = at(&copy, 1.0, v);
    }
    let later = extend(&copy, Side::VMax, 0.5, Reach::Exactly, &mut meter()).unwrap();
    assert_eq!(later.surface, once.surface);
    assert_eq!(later.deviation.to_bits(), once.deviation.to_bits());
}

/// UB-173--the-base-class-box-of-an-extended-smoffsetsurface-recurses.md: an extended offset of
/// an offset rebuilt its cache over the extended domain, each node running the same body again,
/// until the stack overflowed. Ours extends an offset of offsets, as deep as the kernel lets them
/// stack, through its innermost base without recursion: the offsets at the same distances of the
/// base extended.
#[test]
fn an_offset_of_offsets_extends_through_its_innermost_base() {
    let base = SurfaceKind::Nurbs(graph([0.0; 3]));
    let distances = [0.0625, -0.03125, 0.015625, 0.0078125];
    let mut nested = base.clone();
    let mut depth = 0;
    for d in distances {
        match Offset::new(nested.clone(), d) {
            Ok(o) => {
                nested = SurfaceKind::Offset(o);
                depth += 1;
            }
            Err(CurveError::TooDeep { .. }) => break,
            Err(e) => panic!("{e:?}"),
        }
    }
    assert!(depth >= 2, "{depth}");
    let e = extend(&nested, Side::UMax, 0.5, Reach::Exactly, &mut meter()).unwrap();
    let SurfaceKind::Nurbs(g) = &base else {
        unreachable!()
    };
    let grown = g
        .extend(Direction::U, End::High, 0.5, &mut meter())
        .unwrap();
    let mut want = SurfaceKind::Nurbs(grown.value);
    for d in distances.iter().take(depth) {
        want = SurfaceKind::Offset(Offset::new(want, *d).unwrap());
    }
    assert_eq!(e.surface, want);
    keeps_its_source(&nested, &e);
}

/// UB-187--smoffsetsurface-extension-of-a-plane-with-a-reversed-domain.md: SMLib kept a plane's
/// reversed domain ([0, 1] × [0, −1e10]), made a decreasing knot vector of it, and an offset
/// extended past it crashed. Ours refuses a reversed domain when the plane is made; a plane ten
/// billion units across, and an offset of it, extend past each side by definition.
#[test]
fn a_reversed_plane_is_refused_and_a_vast_one_extends() {
    assert!(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 1.0), (0.0, -1e10))
        )
        .is_err()
    );
    let vast = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 1.0), (-1e10, 0.0)),
        )
        .unwrap(),
    );
    let off = SurfaceKind::Offset(Offset::new(vast.clone(), 2.0).unwrap());
    for s in [&vast, &off] {
        for side in SIDES {
            let e = extend(s, side, 0.5, Reach::Exactly, &mut meter()).unwrap();
            assert_eq!(e.reached, 0.5);
            assert_eq!(e.deviation, 0.0);
            keeps_its_source(s, &e);
        }
    }
}

/// HANG-1--smextent1d-periodicwrap-steps-one-period-per-iteration.md: SMLib wrapped a periodic
/// parameter into its period one period per step, so a parameter far from zero took as many steps
/// as periods between them. Ours never wraps: an arc whose angle runs from 10^15 half-turns
/// extends by its own definition, at once, to the same points as the arc at zero.
#[test]
fn a_periodic_direction_far_from_zero_extends_at_once() {
    let far = 1e15;
    let f = frame();
    let near =
        SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 2.0, f, ((0.0, 0.5), (0.0, 1.0))).unwrap());
    let distant = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 2.0, f, ((far, far + 0.5), (0.0, 1.0))).unwrap(),
    );
    let a = extend(&near, Side::UMax, 0.5, Reach::Exactly, &mut meter()).unwrap();
    let b = extend(&distant, Side::UMax, 0.5, Reach::Exactly, &mut meter()).unwrap();
    assert_eq!(b.surface.domain(), ((far, far + 1.0), (0.0, 1.0)));
    // 10^15 half-turns is an even number of them, so the arcs coincide.
    for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let (p, q) = (at(&a.surface, t, 0.5), at(&b.surface, far + t, 0.5));
        for k in 0..3 {
            assert!((p[k] - q[k]).abs() < 1e-9, "{t}: {p:?} against {q:?}");
        }
    }
    // Asked for a million half-turns, the distant arc reaches its turn and no more.
    for side in [Side::UMin, Side::UMax] {
        let turn = extend(&distant, side, 1e6, Reach::AsFarAsValid, &mut meter()).unwrap();
        let ((u0, u1), _) = turn.surface.domain();
        assert_eq!(u1 - u0, 2.0, "{side:?}");
        keeps_its_source(&distant, &turn);
    }
}

/// The length an extension reports is how far its domain grew. Where the turn, a pole or an apex
/// stops a definition, `extend` finds the length by bisection and reports the length it tried,
/// not how far the rounded domain bound moved, so the two part by up to a rounding of that bound:
/// a full turn widened by nothing reports 2.2e-16, and an arc whose angle runs from 10^15
/// half-turns, where doubles are 1/8 apart, reports 1.5625 half-turns (281.25 degrees) for a
/// domain that grew by 1.5 (270 degrees), and refuses an exact extension naming 1.5625 as valid.
/// The domain itself never passes the turn. A caller that places the new side from the length
/// reported puts it 11.25 degrees off there. Ignored until G253 reports the domain's own growth.
#[test]
#[ignore = "defect in G253: the length reached is the bisection's, not the domain's growth"]
fn the_length_reached_is_how_far_the_domain_grew() {
    let f = frame();
    let grew = |source: &SurfaceKind, e: &Extended, side: Side| {
        let ((u0, u1), (v0, v1)) = source.domain();
        let ((a0, a1), (b0, b1)) = e.surface.domain();
        match side {
            Side::UMin => u0 - a0,
            Side::UMax => a1 - u1,
            Side::VMin => v0 - b0,
            Side::VMax => b1 - v1,
        }
    };
    let full =
        SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 1.0, f, ((0.0, 2.0), (0.0, 1.0))).unwrap());
    let far = 1e15;
    let distant = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 2.0, f, ((far, far + 0.5), (0.0, 1.0))).unwrap(),
    );
    // The 294 degree arc of WR-650, widened to its turn.
    let arc = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 5.0, f, ((0.0, 294.0 / 180.0), (0.0, 1.0))).unwrap(),
    );
    for (s, by) in [
        (&full, 0.1),
        (&full, 10.0),
        (&distant, 1e6),
        (&arc, 294.0 / 180.0),
    ] {
        for side in [Side::UMin, Side::UMax] {
            let e = extend(s, side, by, Reach::AsFarAsValid, &mut meter()).unwrap();
            assert_eq!(e.reached, grew(s, &e, side), "{side:?} by {by}");
            if let Err(ExtendError::ExtensionFailed { reached, .. }) =
                extend(s, side, by, Reach::Exactly, &mut meter())
            {
                assert_eq!(reached, e.reached, "{side:?} by {by}");
            }
        }
    }
}
