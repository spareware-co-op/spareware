//! What a stored curve is rebuilt from (piece G94): a bulge polyline's start, given back and
//! refused when it is not finite, and a composite rebuilt from its own starts.

#![allow(
    clippy::unwrap_used,
    clippy::float_cmp,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests: a failure stops the test, and parameters are compared exactly"
)]

use geom_curve::{
    Basis, BulgePolyline, Composite, CurveError, CurveKind, Line, ParamCurve, Trimmed,
};
use geom_run::{Budget, WorkMeter};

fn polyline(start: f64) -> Result<BulgePolyline<2>, CurveError> {
    BulgePolyline::starting_at(
        vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        vec![0.5, -0.25],
        None,
        false,
        start,
    )
}

#[test]
fn a_bulge_polyline_starts_where_it_is_told() {
    assert_eq!(polyline(0.0).unwrap().start(), 0.0);
    assert_eq!(polyline(-2.0).unwrap().start(), -2.0);
    assert_eq!(ParamCurve::domain(&polyline(-2.0).unwrap()), (-2.0, 0.0));
    for start in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(polyline(start), Err(CurveError::NotFinite));
    }
    // Reversed, a polyline of two segments starts at −2, and is rebuilt from that start.
    let reversed = CurveKind::Bulge(polyline(0.0).unwrap())
        .reversed(&mut WorkMeter::new(Budget::UNLIMITED))
        .unwrap()
        .value;
    let CurveKind::Bulge(r) = &reversed else {
        panic!("{reversed:?}")
    };
    assert_eq!(r.start(), -2.0);
    let rebuilt = BulgePolyline::starting_at(
        r.vertices().to_vec(),
        r.bulges().to_vec(),
        r.widths().map(<[(f64, f64)]>::to_vec),
        r.is_closed(),
        r.start(),
    )
    .unwrap();
    assert_eq!(&rebuilt, r);
}

#[test]
fn a_composite_is_rebuilt_from_its_starts() {
    let segment = |a: [f64; 2], b: [f64; 2]| {
        let line = Line::new(a, [b[0] - a[0], b[1] - a[1]], (0.0, 1.0)).unwrap();
        Trimmed::new(Basis::Line(line), (0.0, 1.0)).unwrap()
    };
    let c = Composite::new(vec![
        segment([0.0, 0.0], [1.0, 0.0]),
        segment([1.0, 0.0], [1.0, 1.0]),
    ])
    .unwrap();
    let rebuilt = Composite::from_starts(c.segments().to_vec(), c.starts().to_vec()).unwrap();
    assert_eq!(rebuilt, c);
    // Joints apart are kept, with their gaps certified.
    let apart = Composite::from_starts(
        vec![
            segment([0.0, 0.0], [1.0, 0.0]),
            segment([3.0, 0.0], [3.0, 1.0]),
        ],
        vec![0.0, 1.0, 2.0],
    )
    .unwrap();
    assert!(apart.gaps()[0] >= 2.0);
    assert_eq!(
        Composite::from_starts(c.segments().to_vec(), vec![0.0, 1.0]),
        Err(CurveError::Degenerate)
    );
}
