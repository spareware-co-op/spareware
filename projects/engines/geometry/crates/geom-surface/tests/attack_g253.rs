//! Attacks on piece G253's claims (surface extension): far from the origin, lengths too small to
//! move a knot and far too long to stay valid, a budget that runs out, an infinite plane, and
//! extensions of extensions.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    reason = "tests: native arithmetic and indexing, and a failure stops the test"
)]

use geom_curve::Frame;
use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::extend::extend;
use geom_surface::{Cylinder, ExtendError, ParamSurface, Plane, Reach, Side, SurfaceKind};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A bicubic graph over two spans each way, placed at `at`.
fn graph(at: [f64; 3]) -> SurfaceKind {
    let k = vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0];
    let (bu, bv) = (Basis::new(3, k.clone()).unwrap(), Basis::new(3, k).unwrap());
    let mut pts = Vec::new();
    for i in 0..5_u8 {
        for j in 0..5_u8 {
            let (x, y) = (f64::from(i), f64::from(j));
            pts.push([at[0] + x, at[1] + y, at[2] + 0.1 * ((x + y) % 2.0)]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(bu, bv, &pts).unwrap())
}

#[test]
fn far_from_the_origin_the_source_is_kept_and_the_bound_holds() {
    for at in [[1e9, -1e9, 1e9], [-1.099_511_627_775e12, 0.0, 3.0]] {
        let s = graph(at);
        let e = extend(&s, Side::UMax, 0.5, Reach::Exactly, &mut meter()).unwrap();
        assert!(e.deviation.is_finite());
        for u in [0.0, 0.7, 1.9] {
            assert_eq!(
                e.surface
                    .point(u, 1.0, (Limit::FromRight, Limit::FromRight))
                    .unwrap(),
                s.point(u, 1.0, (Limit::FromRight, Limit::FromRight))
                    .unwrap()
            );
        }
    }
}

#[test]
fn a_length_too_small_to_move_the_knot_reaches_nothing() {
    let s = graph([0.0; 3]);
    assert!(matches!(
        extend(&s, Side::VMin, 1e-300, Reach::Exactly, &mut meter()),
        Err(ExtendError::ExtensionFailed { reached, .. }) if reached == 0.0
    ));
    let e = extend(&s, Side::VMin, 1e-300, Reach::AsFarAsValid, &mut meter()).unwrap();
    assert_eq!((e.reached, &e.surface), (0.0, &s));
}

#[test]
fn a_far_too_long_extension_is_refused_or_stopped_never_returned_unchecked() {
    let s = graph([0.0; 3]);
    for by in [10.0, 1e3, 1e6] {
        match extend(&s, Side::UMax, by, Reach::Exactly, &mut meter()) {
            Ok(e) => assert_eq!(e.reached, by),
            Err(ExtendError::ExtensionFailed { reached, .. }) => assert!(reached < by),
            Err(other) => panic!("{by}: {other}"),
        }
        match extend(&s, Side::UMax, by, Reach::AsFarAsValid, &mut meter()) {
            Ok(e) => {
                assert!(e.reached <= by);
                // What came back extends no further than it was certified to: asking for that
                // length again, exactly, is met.
                if e.reached > 0.0 {
                    extend(&s, Side::UMax, e.reached, Reach::Exactly, &mut meter()).unwrap();
                }
            }
            Err(other) => panic!("{by}: {other}"),
        }
    }
}

#[test]
fn a_budget_that_runs_out_stops_cleanly() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let kinds = [
        graph([0.0; 3]),
        SurfaceKind::Cylinder(
            Cylinder::new([0.0; 3], 1.0, frame, ((0.0, 1.9), (0.0, 1.0))).unwrap(),
        ),
    ];
    for s in &kinds {
        for work in [1_u64, 10, 100] {
            let mut tight = WorkMeter::new(Budget::new(work, u64::MAX));
            match extend(s, Side::UMax, 0.5, Reach::AsFarAsValid, &mut tight) {
                Ok(_) | Err(ExtendError::Stopped(_)) => {}
                Err(other) => panic!("{work}: {other}"),
            }
        }
    }
}

#[test]
fn an_infinite_side_stays_infinite_and_extensions_compose() {
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, f64::INFINITY), (0.0, 1.0)),
        )
        .unwrap(),
    );
    let e = extend(&plane, Side::UMax, 5.0, Reach::Exactly, &mut meter()).unwrap();
    assert_eq!(e.surface, plane);
    // A cylinder extended twice is the cylinder extended once by the sum.
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let cyl = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 1.0, frame, ((0.0, 1.0), (0.0, 1.0))).unwrap(),
    );
    let once = extend(&cyl, Side::VMax, 0.75, Reach::Exactly, &mut meter()).unwrap();
    let twice = extend(&cyl, Side::VMax, 0.25, Reach::Exactly, &mut meter()).unwrap();
    let twice = extend(
        &twice.surface,
        Side::VMax,
        0.5,
        Reach::Exactly,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(once.surface, twice.surface);
    // A NURBS extended twice keeps the first extension as its source.
    let s = graph([0.0; 3]);
    let first = extend(&s, Side::UMin, 0.25, Reach::Exactly, &mut meter()).unwrap();
    let second = extend(
        &first.surface,
        Side::UMin,
        0.25,
        Reach::Exactly,
        &mut meter(),
    )
    .unwrap();
    for u in [-0.25, 0.5, 1.5] {
        assert_eq!(
            second
                .surface
                .point(u, 0.5, (Limit::FromRight, Limit::FromRight))
                .unwrap(),
            first
                .surface
                .point(u, 0.5, (Limit::FromRight, Limit::FromRight))
                .unwrap()
        );
    }
}
