//! Which form a roof is, from which of its footprint's edges slope, and every roof the crate
//! cannot make refused in words naming its edges.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{at, degrees, edges, id, l_shape, rectangle, slope};
use construct_params::{Angle, Slope};
use construct_roofs::{EdgeSpec, Form, MOST_SLOPE, Pitch, Roof, RoofError};
use construct_slabs::{Boundary, SlabError};
use std::collections::BTreeMap;

/// No edge slopes: flat, on a rectangle or an L, with or without overhangs. One edge: mono-pitch
/// from it. Two opposite edges at one slope: a gable from both, in the footprint's order. An edge
/// a roof is not given has no overhang and no pitch.
#[test]
fn the_form_is_worked_out_from_which_edges_slope() {
    let flat = Roof::new(rectangle(), &BTreeMap::new()).unwrap();
    assert_eq!(*flat.form(), Form::Flat);
    assert_eq!(flat.edges().len(), 4);
    assert!(
        flat.edges()
            .iter()
            .all(|(_, spec)| *spec == EdgeSpec::default())
    );
    let l = Roof::new(l_shape(), &edges(&[("o0", 300, None), ("o3", 600, None)])).unwrap();
    assert_eq!(*l.form(), Form::Flat);
    assert_eq!(l.edge(&id("o3")).unwrap().overhang, common::mm(600));

    let mono = Roof::new(rectangle(), &edges(&[("o1", 0, Some(slope(1, 4)))])).unwrap();
    assert_eq!(*mono.form(), Form::Mono { eaves: id("o1") });
    assert_eq!(mono.form().name(), "mono");

    let gable = Roof::new(
        rectangle(),
        &edges(&[("o3", 0, Some(degrees(30))), ("o1", 0, Some(degrees(30)))]),
    )
    .unwrap();
    assert_eq!(
        *gable.form(),
        Form::Gable {
            eaves: [id("o1"), id("o3")]
        }
    );
    assert_eq!(gable.form().eaves(), [id("o1"), id("o3")]);
    // A slope and the angle it is worked as are one slope, so they make a gable.
    let same = Roof::new(
        rectangle(),
        &edges(&[
            ("o0", 0, Some(degrees(30))),
            ("o2", 0, Some(slope(577_350_269, 1_000_000_000))),
        ]),
    )
    .unwrap();
    assert_eq!(same.form().name(), "gable");
}

/// Edges that meet at a corner, three edges or all four are solved (hips); two opposite edges at
/// two slopes are an uneven gable; a turned rectangle and an L are solved. Slopes of two pitches
/// that meet at a hip, a footprint that is not right-angled, a verge on an L and two overhangs on
/// an L are refused, each naming its edges.
#[test]
fn hips_uneven_gables_and_other_footprints_are_solved_or_refused_by_name() {
    let quarter = Some(slope(1, 4));
    let made = |list: &[(&str, i64, Option<Pitch>)]| Roof::new(rectangle(), &edges(list));
    let every = made(&[
        ("o0", 0, quarter),
        ("o1", 0, quarter),
        ("o2", 0, quarter),
        ("o3", 0, quarter),
    ])
    .unwrap();
    assert_eq!(
        *every.form(),
        Form::Solved {
            eaves: vec![id("o0"), id("o1"), id("o2"), id("o3")]
        }
    );
    assert_eq!(every.form().name(), "solved");
    let corner = made(&[("o1", 0, quarter), ("o2", 0, quarter)]).unwrap();
    assert_eq!(corner.form().eaves(), [id("o1"), id("o2")]);
    let three = made(&[("o0", 0, quarter), ("o1", 0, quarter), ("o2", 0, quarter)]).unwrap();
    assert_eq!(three.form().name(), "solved");
    let uneven = made(&[("o0", 0, quarter), ("o2", 0, Some(slope(1, 3)))]).unwrap();
    assert_eq!(
        *uneven.form(),
        Form::Gable {
            eaves: [id("o0"), id("o2")]
        }
    );
    let l = Roof::new(
        l_shape(),
        &edges(&[
            ("o0", 0, quarter),
            ("o1", 0, quarter),
            ("o2", 0, quarter),
            ("o3", 0, quarter),
            ("o4", 0, quarter),
            ("o5", 0, quarter),
        ]),
    )
    .unwrap();
    assert_eq!(l.form().eaves().len(), 6);
    let turned = Boundary::from_corners(
        &[at(0, 0), at(4000, 3000), at(1000, 7000), at(-3000, 4000)],
        &[],
    )
    .unwrap();
    let gable = Roof::new(
        turned.clone(),
        &edges(&[("o0", 0, quarter), ("o2", 0, quarter)]),
    )
    .unwrap();
    assert_eq!(gable.form().name(), "solved");
}

/// Slopes of two pitches that meet at a hip (or across a turned gable), a verge on an L, two
/// overhangs on an L, an overhang on a turned footprint and a footprint that is not right-angled
/// are refused, each naming its edges.
#[test]
fn what_the_solver_cannot_hold_is_refused_by_name() {
    let quarter = Some(slope(1, 4));
    let made = |list: &[(&str, i64, Option<Pitch>)]| Roof::new(rectangle(), &edges(list));
    let turned = Boundary::from_corners(
        &[at(0, 0), at(4000, 3000), at(1000, 7000), at(-3000, 4000)],
        &[],
    )
    .unwrap();
    let two = made(&[("o0", 0, quarter), ("o1", 0, Some(slope(1, 3)))]).unwrap_err();
    assert_eq!(
        two,
        RoofError::UnevenPitches {
            first: id("o0"),
            second: id("o1")
        }
    );
    assert!(two.to_string().contains("different pitches"), "{two}");
    let turned_uneven = Roof::new(
        turned.clone(),
        &edges(&[("o0", 0, quarter), ("o2", 0, Some(slope(1, 3)))]),
    )
    .unwrap_err();
    assert!(matches!(turned_uneven, RoofError::UnevenPitches { .. }));
    let verge = Roof::new(l_shape(), &edges(&[("o0", 0, quarter)])).unwrap_err();
    assert_eq!(verge, RoofError::VergeOnReflex { edge: id("o1") });
    assert!(verge.to_string().contains("inside corner"), "{verge}");
    let all = |o5: i64| {
        edges(&[
            ("o0", 300, quarter),
            ("o1", 300, quarter),
            ("o2", 300, quarter),
            ("o3", 300, quarter),
            ("o4", 300, quarter),
            ("o5", o5, quarter),
        ])
    };
    assert!(Roof::new(l_shape(), &all(300)).is_ok());
    assert_eq!(
        Roof::new(l_shape(), &all(200)).unwrap_err(),
        RoofError::OverhangsDiffer {
            first: id("o0"),
            second: id("o5")
        }
    );
    let over = Roof::new(turned, &edges(&[("o0", 100, quarter), ("o2", 0, quarter)])).unwrap_err();
    assert_eq!(over, RoofError::OverhangOffAxis { edge: id("o0") });
    let triangle = Boundary::from_corners(&[at(0, 0), at(4000, 0), at(0, 3000)], &[]).unwrap();
    let slanted = Roof::new(triangle, &edges(&[("o0", 0, quarter)])).unwrap_err();
    assert_eq!(
        slanted,
        RoofError::NotRightAngled {
            edges: vec![id("o0")]
        }
    );
    assert!(
        slanted
            .to_string()
            .starts_with("edge o0 slopes, and a sloped roof's footprint has right angles")
    );
}

/// An arc edge is refused by construct-slabs' own reading of the footprint, before any roof is
/// worked out.
#[test]
fn an_arc_edge_is_refused() {
    let roof = Roof::new(rectangle(), &edges(&[("o0", 0, Some(slope(1, 4)))])).unwrap();
    let mut footprint = roof.footprint().to_value();
    footprint["outer"][0] = serde_json::json!({ "id": "o0", "arc": 0, "reversed": false });
    let refused = Roof::from_values(&footprint, &roof.edges_value()).unwrap_err();
    assert!(
        matches!(refused, RoofError::Slab(_) | RoofError::Malformed(_)),
        "{refused:?}"
    );
}

/// Overhangs and pitches are checked edge by edge: no overhang below nothing, a pitch that rises
/// and is no steeper than 10 in 1 (an angle under 90 degrees); an edge the footprint lacks is
/// refused; a footprint with a hole is refused, naming job 4.2.
#[test]
fn each_edge_is_checked() {
    let one = |edge: &str, overhang: i64, pitch: Option<Pitch>| {
        Roof::new(rectangle(), &edges(&[(edge, overhang, pitch)]))
    };
    assert_eq!(
        one("o9", 0, None).unwrap_err(),
        RoofError::UnknownEdge { edge: id("o9") }
    );
    assert_eq!(
        one("o1", -1, None).unwrap_err(),
        RoofError::NegativeOverhang { edge: id("o1") }
    );
    assert_eq!(
        one("o1", 0, Some(slope(0, 1))).unwrap_err(),
        RoofError::NoRise { edge: id("o1") }
    );
    assert_eq!(
        one("o1", 0, Some(slope(-1, 4))).unwrap_err(),
        RoofError::NoRise { edge: id("o1") }
    );
    assert_eq!(
        one("o1", 0, Some(degrees(0))).unwrap_err(),
        RoofError::NoRise { edge: id("o1") }
    );
    let (rise, run) = MOST_SLOPE;
    assert!(
        one("o1", 0, Some(slope(rise, run))).is_ok(),
        "10 in 1 is taken"
    );
    assert_eq!(
        one("o1", 0, Some(slope(rise * 1000 + 1, run * 1000))).unwrap_err(),
        RoofError::TooSteep { edge: id("o1") }
    );
    assert_eq!(
        one("o1", 0, Some(degrees(90))).unwrap_err(),
        RoofError::TooSteep { edge: id("o1") }
    );
    assert_eq!(
        one("o1", 0, Some(degrees(85))).unwrap_err(),
        RoofError::TooSteep { edge: id("o1") },
        "85 degrees is steeper than 10 in 1"
    );
    assert!(one("o1", 0, Some(degrees(84))).is_ok());
    let holed = Boundary::from_corners(
        &[at(0, 0), at(6000, 0), at(6000, 4000), at(0, 4000)],
        &[vec![at(1000, 1000), at(2000, 1000), at(2000, 2000)]],
    )
    .unwrap();
    let refused = Roof::new(holed, &BTreeMap::new()).unwrap_err();
    assert_eq!(refused, RoofError::Holes { count: 1 });
    assert!(refused.to_string().contains("job 4.2"));
}

/// A flat roof's overhangs move each edge out square to itself, so they need every edge along an
/// axis; edges on one line with two overhangs, or overhangs that fold the outline, are refused.
/// With no overhang, a footprint off the axes is a flat roof as it stands.
#[test]
fn a_flat_roofs_overhangs_need_a_footprint_along_the_axes() {
    let triangle = Boundary::from_corners(&[at(0, 0), at(4000, 0), at(0, 3000)], &[]).unwrap();
    assert!(Roof::new(triangle.clone(), &BTreeMap::new()).is_ok());
    let off = Roof::new(triangle, &edges(&[("o0", 100, None)])).unwrap_err();
    assert_eq!(off, RoofError::OverhangOffAxis { edge: id("o1") });
    assert!(off.to_string().contains("along the axes"));
    let straight = Boundary::from_corners(
        &[
            at(0, 0),
            at(3000, 0),
            at(6000, 0),
            at(6000, 4000),
            at(0, 4000),
        ],
        &[],
    )
    .unwrap();
    assert!(
        Roof::new(
            straight.clone(),
            &edges(&[("o0", 200, None), ("o1", 200, None)])
        )
        .is_ok()
    );
    assert_eq!(
        Roof::new(straight, &edges(&[("o0", 200, None), ("o1", 300, None)])).unwrap_err(),
        RoofError::OverhangFolds { edge: id("o1") }
    );
    // The L's inner corner: an overhang on o2 (the top of its right half, at y = 4 m) deeper than
    // the 4 m step up to o4 runs o3 backwards and folds the outline there.
    let folded = Roof::new(l_shape(), &edges(&[("o2", 5000, None)])).unwrap_err();
    assert!(
        matches!(folded, RoofError::OverhangFolds { .. }),
        "{folded:?}"
    );
}

/// A roof too shallow or too narrow to rise a grid step is refused, naming its edge; a footprint
/// construct-slabs refuses is refused in its words.
#[test]
fn a_roof_that_does_not_rise_is_refused() {
    let tiny = Pitch::Slope(Slope::new(1, 1_000_000_000_000).unwrap());
    let shallow = Roof::new(rectangle(), &edges(&[("o0", 0, Some(tiny))])).unwrap_err();
    assert_eq!(shallow, RoofError::TooShallow { edge: id("o0") });
    let gable = Roof::new(
        rectangle(),
        &edges(&[("o0", 0, Some(tiny)), ("o2", 0, Some(tiny))]),
    )
    .unwrap_err();
    assert_eq!(gable, RoofError::TooShallow { edge: id("o0") });
    let open = Boundary::from_corners(&[at(0, 0), at(1, 0)], &[]).unwrap_err();
    assert!(matches!(open, SlabError::TooFewSides { .. }));
    let plain = Roof::from_plain(&serde_json::json!({
        "footprint": { "outer": [{ "at": [0, 0] }, { "at": [6400, 0] }] }
    }))
    .unwrap_err();
    assert!(
        matches!(plain, RoofError::Slab(SlabError::TooFewSides { .. })),
        "{plain:?}"
    );
    // One angle unit short of 90 degrees has a slope, steeper than 10 in 1.
    let steep = Pitch::Angle(Angle::from_units(90 * Angle::UNITS_PER_DEGREE - 1));
    assert!(steep.slope().is_some());
}
