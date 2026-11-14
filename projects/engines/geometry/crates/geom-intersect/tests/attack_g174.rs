//! Attacks on piece G174's claims: a known curve, crossing or tangent, is taken out of the search
//! only inside a proven tube; a second intersection near it, a fillet's re-crossing from 1 to 100
//! steps away, is found or flagged, never dropped; and a stretch no tube covers is searched and
//! flagged as known but unproven.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_intersect::IntersectError;
use geom_intersect::general::{Subdivided, subdivide};
use geom_intersect::known::{Known, KnownContact, subdivide_known};
use geom_intersect::march::march;
use geom_intersect::result::Unresolved;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};

/// A step of the grid here: a floor of 1/128, a double.
const FLOOR: f64 = 1.0 / 128.0;
const CEILING: f64 = 0.25;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 23))
}

/// z = 0 over a patch holding every surface here.
fn ground() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        )
        .unwrap(),
    )
}

/// A sheet over x in [−1, 1] and v in [−1, 1], straight in x, with z the Bézier polynomial of
/// `heights` in v (y follows v as the control points' Greville abscissae give it): every height is
/// a dyadic double, so each root is where the test puts it, exactly.
fn sheet(heights: &[f64]) -> SurfaceKind {
    let degree = heights.len() - 1;
    let mut knots = vec![-1.0; degree + 1];
    knots.extend(vec![1.0; degree + 1]);
    let across = Basis::new(degree, knots).unwrap();
    let along = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for x in [-1.0, 1.0] {
        for (j, z) in heights.iter().enumerate() {
            let y = -1.0 + 2.0 * j as f64 / degree as f64;
            net.push([x, y, *z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap())
}

/// z = K·(t − ½)², t = (v + 1)/2: tangent to the ground along v = 0 and nowhere else.
fn tangent_sheet() -> SurfaceKind {
    let k = 0.75;
    sheet(&[k / 4.0, -k / 4.0, k / 4.0])
}

/// z = K·(t − ½)·(t − r): crossing the ground along v = 0 and again along v = 2r − 1.
fn crossing_sheet(r: f64) -> SurfaceKind {
    let k = 0.75;
    let (b0, b2) = (k * r / 2.0, k * (1.0 - r) / 2.0);
    sheet(&[b0, -(b0 + b2) / 2.0, b2])
}

/// z = K·(t − ½)²·(t − r), K = 3/4: tangent to the ground along v = 0, as a fillet is to its wall,
/// and crossing it again along v = 2r − 1, a re-crossing.
fn fillet(r: f64) -> SurfaceKind {
    sheet(&[
        -3.0 * r / 16.0,
        (1.0 + r) / 16.0,
        (r - 2.0) / 16.0,
        3.0 * (1.0 - r) / 16.0,
    ])
}

/// The line v = 0 of a sheet on the ground, its known curve: (u, 0) on the sheet, (u, 0) on the
/// ground, exact.
fn line(contact: KnownContact) -> Known {
    let point = |x: f64| Interval::point(x).unwrap();
    Known {
        vertices: (-4..=4)
            .map(|k| {
                let u = f64::from(k) / 4.0;
                [point(u), point(0.0), point(u), point(0.0)]
            })
            .collect(),
        bound: 0.0,
        contact,
    }
}

/// Whether a pair's sheet patch holds the line v = `v`.
fn holds(found: &Subdivided, v: f64) -> (bool, bool) {
    let settled = found
        .settled
        .iter()
        .any(|p| p.first.1.0 <= v && v <= p.first.1.1);
    let flagged = found
        .flagged
        .iter()
        .any(|p| p.first.1.0 <= v && v <= p.first.1.1);
    (settled, flagged)
}

#[test]
fn a_tangent_known_curve_is_taken_out_whole() {
    let (sheet, wall) = (tangent_sheet(), ground());
    // Alone, the search cannot settle a tangent contact: it flags it at the floor.
    let plain = subdivide(&sheet, &wall, FLOOR, &mut meter()).unwrap();
    assert!(!plain.flagged.is_empty(), "{plain:?}");
    // Known, the tube about it is proven (the sheet bends one way across it), and nothing is left.
    let known = subdivide_known(
        &sheet,
        &wall,
        &[line(KnownContact::Tangent)],
        FLOOR,
        &mut meter(),
    )
    .unwrap();
    assert!(known.flagged.is_empty(), "{:?}", known.flagged);
    assert!(known.settled.is_empty(), "{:?}", known.settled);
}

#[test]
fn either_surface_may_carry_the_tube() {
    let (sheet, wall) = (tangent_sheet(), ground());
    let mut curve = line(KnownContact::Tangent);
    for v in &mut curve.vertices {
        let [a, b, c, d] = *v;
        *v = [c, d, a, b];
    }
    let known = subdivide_known(&wall, &sheet, &[curve], FLOOR, &mut meter()).unwrap();
    assert!(known.flagged.is_empty(), "{:?}", known.flagged);
    assert!(known.settled.is_empty(), "{:?}", known.settled);
}

#[test]
fn a_crossing_known_curve_leaves_the_other_one_to_find() {
    // Roots along v = 0, the known curve, and v = 7/8.
    let r = 15.0 / 16.0;
    let (sheet, wall) = (crossing_sheet(r), ground());
    let plain = subdivide(&sheet, &wall, FLOOR, &mut meter()).unwrap();
    let traced = march(&sheet, &wall, &plain, FLOOR, CEILING, &mut meter()).unwrap();
    assert_eq!(traced.branches.len(), 2, "{traced:?}");
    let known = subdivide_known(
        &sheet,
        &wall,
        &[line(KnownContact::Crossing)],
        FLOOR,
        &mut meter(),
    )
    .unwrap();
    assert!(known.flagged.is_empty(), "{:?}", known.flagged);
    let traced = march(&sheet, &wall, &known, FLOOR, CEILING, &mut meter()).unwrap();
    assert_eq!(traced.branches.len(), 1, "{traced:?}");
    let other = 2.0 * r - 1.0;
    for x in &traced.branches[0].vertices {
        assert!(
            x[1].lo() - FLOOR <= other && other <= x[1].hi() + FLOOR,
            "{x:?}"
        );
    }
}

#[test]
fn a_fillets_re_crossing_is_found_or_flagged_never_dropped() {
    let wall = ground();
    for steps in [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0] {
        // The re-crossing along v = 2r − 1 = steps · FLOOR.
        let r = 0.5 + steps * FLOOR / 2.0;
        let sheet = fillet(r);
        let away = 2.0 * r - 1.0;
        let found = subdivide_known(
            &sheet,
            &wall,
            &[line(KnownContact::Tangent)],
            FLOOR,
            &mut meter(),
        )
        .unwrap();
        let (settled, flagged) = holds(&found, away);
        assert!(
            settled || flagged,
            "{steps}: the re-crossing was dropped: {found:?}"
        );
        if settled {
            let traced = march(&sheet, &wall, &found, FLOOR, CEILING, &mut meter()).unwrap();
            let near = traced.branches.iter().any(|b| {
                b.vertices
                    .iter()
                    .all(|x| (x[1].lo() - away).abs() < 4.0 * FLOOR)
            });
            assert!(near || flagged, "{steps}: {traced:?}");
        }
        for pair in &found.flagged {
            assert!(
                pair.why == Unresolved::KnownCurveUnproven
                    || matches!(pair.why, Unresolved::UndecidedAtFloor { .. }),
                "{steps}: {pair:?}"
            );
        }
        if steps >= 50.0 {
            // Far enough away for a tube: the known curve itself is out of the search.
            let (_, at_known) = holds(&found, 0.0);
            assert!(!at_known, "{steps}: {:?}", found.flagged);
        }
    }
}

#[test]
fn a_stretch_no_tube_covers_is_searched_and_flagged_as_known() {
    // Two NURBS sheets: neither has an implicit form, so no tangent tube is proven, and the
    // tangent contact the search cannot settle is flagged as a known curve unproven.
    let net = [
        [-8.0, -8.0, 0.0],
        [-8.0, 8.0, 0.0],
        [8.0, -8.0, 0.0],
        [8.0, 8.0, 0.0],
    ];
    let flat = |b: (f64, f64)| Basis::new(1, vec![b.0, b.0, b.1, b.1]).unwrap();
    let wall =
        SurfaceKind::Nurbs(Surface::new(flat((-8.0, 8.0)), flat((-8.0, 8.0)), &net).unwrap());
    let sheet = tangent_sheet();
    let found = subdivide_known(
        &sheet,
        &wall,
        &[line(KnownContact::Tangent)],
        FLOOR,
        &mut meter(),
    )
    .unwrap();
    // Pairs on the curve say it is known but unproven; pairs beside it in the tangent band say
    // why they could not decide; and the curve is covered, never dropped.
    assert!(
        found
            .flagged
            .iter()
            .any(|p| p.why == Unresolved::KnownCurveUnproven),
        "{:?}",
        found.flagged
    );
    assert!(
        found
            .flagged
            .iter()
            .all(|p| p.why == Unresolved::KnownCurveUnproven
                || matches!(p.why, Unresolved::UndecidedAtFloor { .. })),
        "{:?}",
        found.flagged
    );
    assert_eq!(holds(&found, 0.0), (false, true), "{found:?}");
}

#[test]
fn no_known_curve_is_the_plain_search() {
    let (sheet, wall) = (crossing_sheet(15.0 / 16.0), ground());
    let plain = subdivide(&sheet, &wall, FLOOR, &mut meter()).unwrap();
    let none = subdivide_known(&sheet, &wall, &[], FLOOR, &mut meter()).unwrap();
    assert_eq!(plain, none);
}

#[test]
fn a_budget_stops_it_and_a_floor_must_be_positive() {
    let (sheet, wall) = (tangent_sheet(), ground());
    let known = [line(KnownContact::Tangent)];
    let mut small = WorkMeter::new(Budget::of_work(8));
    assert_eq!(
        subdivide_known(&sheet, &wall, &known, FLOOR, &mut small),
        Err(IntersectError::BudgetExceeded)
    );
    for floor in [0.0, -1.0, f64::NAN] {
        assert_eq!(
            subdivide_known(&sheet, &wall, &known, floor, &mut meter()),
            Err(IntersectError::InvalidInput)
        );
    }
    let once = subdivide_known(&sheet, &wall, &known, FLOOR, &mut meter()).unwrap();
    let again = subdivide_known(&sheet, &wall, &known, FLOOR, &mut meter()).unwrap();
    assert_eq!(once, again);
}
