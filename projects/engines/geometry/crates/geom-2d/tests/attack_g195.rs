//! Piece G195's attack tests: contacts between splines and other curves of the plane
//! (`geom_2d::contact`). A proven crossing is a Crossing, a proven tangency a Touching, and what the
//! certified search cannot settle is Undecided, which never cuts a trim or closes a boundary.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    reason = "test code: a failure should stop loudly"
)]

use geom_2d::contact::{Contact, Kind, Why, contacts};
use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_intersect::result::Unresolved;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Curve};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The Bézier curve through `points`, x evenly spaced over [0, 20] so the curve is a graph.
fn bezier(ys: &[f64]) -> CurveKind<2> {
    let n = ys.len() - 1;
    let knots: Vec<f64> = core::iter::repeat_n(0.0, n + 1)
        .chain(core::iter::repeat_n(1.0, n + 1))
        .collect();
    let basis = Basis::new(n, knots).unwrap();
    let points: Vec<[f64; 2]> = ys
        .iter()
        .enumerate()
        .map(|(i, y)| [20.0 * i as f64 / n as f64, *y])
        .collect();
    CurveKind::Nurbs(Curve::new(basis, &points).unwrap())
}

/// The x axis from x = −5 to 25.
fn axis() -> CurveKind<2> {
    CurveKind::Line(Line::new([-5.0, 0.0], [1.0, 0.0], (0.0, 30.0)).unwrap())
}

fn kinds(found: &[Contact]) -> Vec<Kind> {
    found.iter().map(|c| c.kind).collect()
}

#[test]
fn a_spline_crossing_a_line_is_one_crossing_that_cuts() {
    let found = contacts(&bezier(&[-10.0, -10.0, 10.0, 10.0]), &axis(), &mut meter()).unwrap();
    assert_eq!(kinds(&found), [Kind::Crossing], "{found:?}");
    let (on_spline, on_line) = found[0].cuts().unwrap();
    assert!(on_spline.contains(0.5), "{on_spline:?}");
    assert!(on_line.contains(15.0), "{on_line:?}");
    assert!(found[0].closes());
}

#[test]
fn two_splines_crossing_twice_give_two_crossings() {
    let arch = bezier(&[-10.0, 30.0, 30.0, -10.0]);
    let bowl = bezier(&[10.0, -10.0, -10.0, 10.0]);
    let found = contacts(&arch, &bowl, &mut meter()).unwrap();
    assert_eq!(kinds(&found), [Kind::Crossing, Kind::Crossing], "{found:?}");
}

#[test]
fn a_spline_tangent_to_a_line_is_never_a_crossing() {
    // y = 10 (1 − 2t)², tangent to the x axis at (10, 0).
    let found = contacts(&bezier(&[10.0, -10.0, 10.0]), &axis(), &mut meter()).unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    // The search proves no isolated tangency here, only a band too tangent to tell: undecided,
    // so it neither cuts nor closes.
    assert!(
        matches!(
            found[0].kind,
            Kind::Undecided(Why::Unresolved(Unresolved::TangentBand { .. }))
        ),
        "{found:?}"
    );
    assert_eq!(found[0].cuts(), None);
}

#[test]
fn a_contact_of_fourth_order_is_never_a_crossing() {
    // y = 160 (t − 1/2)⁴, its Bernstein coefficients 10, −10, 10, −10, 10.
    let found = contacts(
        &bezier(&[10.0, -10.0, 10.0, -10.0, 10.0]),
        &axis(),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(
        kinds(&found),
        [Kind::Undecided(Why::Unresolved(
            Unresolved::HigherOrderContact
        ))],
        "{found:?}"
    );
    assert!(!found[0].closes());
}

#[test]
fn an_undecided_contact_never_cuts_or_closes() {
    let span = Interval::new(0.25, 0.75).unwrap();
    let undecided = Contact {
        kind: Kind::Undecided(Why::Singular { branches: 2 }),
        first: span,
        second: span,
        place: None,
    };
    assert_eq!(undecided.cuts(), None);
    assert!(!undecided.closes());
    for kind in [Kind::Crossing, Kind::Touching] {
        let decided = Contact { kind, ..undecided };
        assert_eq!(decided.cuts(), Some((span, span)));
        assert!(decided.closes());
    }
}

#[test]
fn a_circle_tangent_to_a_line_touches_it_and_a_spline_ending_on_it_is_undecided() {
    // The closed form proves the tangency at (10, 0).
    let frame = Frame::new([1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let wheel = CurveKind::Circle(Circle::full([10.0, 5.0], 5.0, frame).unwrap());
    let found = contacts(&wheel, &axis(), &mut meter()).unwrap();
    assert_eq!(kinds(&found), [Kind::Touching], "{found:?}");
    assert!(found[0].cuts().is_some() && found[0].closes());
    // A spline that starts on the axis and rises: the search cannot prove the meeting unique at
    // its floor, so the contact is undecided and never cuts.
    let found = contacts(&bezier(&[0.0, 10.0, 10.0, 10.0]), &axis(), &mut meter()).unwrap();
    assert!(
        found
            .iter()
            .all(|c| matches!(c.kind, Kind::Undecided(_)) && c.cuts().is_none()),
        "{found:?}"
    );
}

#[test]
fn curves_that_never_meet_have_no_contact() {
    let high = bezier(&[5.0, 8.0, 6.0, 9.0]);
    assert_eq!(contacts(&high, &axis(), &mut meter()).unwrap(), []);
}
