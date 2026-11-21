//! [`ParamCurve::enclose_point`]: item 0 of [`ParamCurve::enclose`] at order 0, for the same work,
//! on every conic (each encloses its point directly, with no list) and on a line (the trait's
//! default), each directly and through `CurveKind`; and off the domain, the same error.
#![allow(
    clippy::unwrap_used,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{Axes, Circle, CurveKind, Ellipse, Frame, Hyperbola, Line, Parabola, ParamCurve};
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};

fn same<C: ParamCurve<3>>(curve: &C) {
    let (lo, hi) = curve.domain();
    for t in [lo, lo * 0.75 + hi * 0.25, lo * 0.5 + hi * 0.5, hi] {
        for side in [Limit::FromLeft, Limit::FromRight] {
            let mut by_list = WorkMeter::new(Budget::UNLIMITED);
            let list = curve.enclose(t, side, 0, &mut by_list).unwrap();
            let mut by_point = WorkMeter::new(Budget::UNLIMITED);
            let point = curve
                .enclose_point(t, side, &mut by_point)
                .unwrap()
                .unwrap();
            assert_eq!(list.len(), 1, "one value at order 0");
            let want = list.first().unwrap();
            for (p, w) in point.iter().zip(want) {
                assert_eq!(
                    (p.lo().to_bits(), p.hi().to_bits()),
                    (w.lo().to_bits(), w.hi().to_bits())
                );
            }
            assert_eq!(by_point.used(), by_list.used());
        }
    }
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let past = hi + (hi - lo);
    let off = curve
        .enclose(past, Limit::FromLeft, 0, &mut meter)
        .unwrap_err();
    let by_point = curve
        .enclose_point(past, Limit::FromLeft, &mut meter)
        .unwrap_err();
    assert_eq!(format!("{by_point:?}"), format!("{off:?}"));
}

#[test]
fn each_conics_point_and_a_lines_are_item_zero_of_their_enclosure() {
    let frame = Frame::standard().unwrap();
    let axes = Axes::Frame {
        frame,
        a: 1031.5,
        b: 517.25,
    };
    let at = [3.0, -5.0, 16_777_216.0];
    let circle = Circle::new(at, 1031.5, frame, (0.0, 2.0)).unwrap();
    let ellipse = Ellipse::new(at, axes, (0.0, 2.0)).unwrap();
    let hyperbola = Hyperbola::new(at, axes, (-2.0, 2.0)).unwrap();
    let parabola = Parabola::new(at, axes, (-2.0, 2.0)).unwrap();
    let line = Line::new([1.5, -2.25, 7.0], [0.6, 0.8, 0.0], (-10.0, 10.0)).unwrap();
    same(&circle);
    same(&ellipse);
    same(&hyperbola);
    same(&parabola);
    same(&line);
    for kind in [
        CurveKind::Circle(circle),
        CurveKind::Ellipse(ellipse),
        CurveKind::Hyperbola(hyperbola),
        CurveKind::Parabola(parabola),
        CurveKind::Line(line),
    ] {
        same(&kind);
    }
}
