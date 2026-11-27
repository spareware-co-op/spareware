//! A dimension's value is worked out from its references every time, exactly, and a reference
//! that cannot be used is reported by name instead of a wrong number. Written against this crate's
//! own types only: the references' places come from a table ([`Located`]), so the same tests judge
//! the model behind the locator whichever engine answers.

#![allow(
    clippy::unwrap_used,
    clippy::assert_is_empty,
    clippy::too_many_lines,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_dimensions::{
    Dimension, Direction, Feature, Layout, Located, Locus, Missing, OUT_OF_RANGE,
    REFERENCE_MISSING, REFERENCE_PARALLEL, Reference, Side, TextSize, Unmeasured, Why, display,
    labels, measure,
};
use construct_grids::Point;
use construct_params::{Format, Kind, Length, Unit, Units};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

fn wall_face(wall: &str) -> Reference {
    Reference::new(id(wall), Feature::WallFace(Side::Exterior))
}

/// A vertical line (a wall face running along y) at `x` millimetres.
fn vertical_at(x: i32) -> Locus {
    Locus::line(Point::mm(x, 0), mm(0), mm(3000))
}

/// A horizontal line at `y` millimetres.
fn horizontal_at(y: i32) -> Locus {
    Locus::line(Point::mm(0, y), mm(3000), mm(0))
}

fn horizontal(references: Vec<Reference>) -> Dimension {
    Dimension::new(
        id("D-1"),
        id("V-1"),
        Layout::horizontal(mm(5000)).unwrap(),
        references,
    )
    .unwrap()
}

#[test]
fn a_horizontal_dimension_reads_the_distance_between_two_faces_exactly() {
    let located = Located::new()
        .with(wall_face("W-1"), Ok(vertical_at(0)))
        .with(wall_face("W-2"), Ok(vertical_at(2400)));
    let dimension = horizontal(vec![wall_face("W-1"), wall_face("W-2")]);
    let measured = measure(&dimension, &located);
    assert!(measured.warnings.is_empty());
    let [segment] = measured.segments.as_slice() else {
        panic!("one segment")
    };
    assert_eq!(segment.value, Some(mm(2400)));
    assert!(segment.exact);
    assert_eq!((segment.from, segment.to), (0, 1));
    assert_eq!(segment.middle, Some(Point::mm(1200, 5000)));
    assert_eq!(measured.overall, Some(mm(2400)));
    // The witness lines run from each face's drawn point to the offset line, at the face's x.
    let witnesses: Vec<_> = measured.witnesses.iter().flatten().collect();
    assert_eq!(witnesses.len(), 2);
    assert_eq!(witnesses[0].from, Point::mm(0, 0));
    assert_eq!(witnesses[0].to, Point::mm(0, 5000));
    assert_eq!(witnesses[1].to, Point::mm(2400, 5000));
}

#[test]
fn a_string_has_a_segment_between_each_neighbour_and_an_overall_length() {
    let located = Located::new()
        .with(wall_face("W-1"), Ok(vertical_at(0)))
        .with(wall_face("W-2"), Ok(vertical_at(1000)))
        .with(wall_face("W-3"), Ok(vertical_at(2500)));
    let dimension = horizontal(vec![wall_face("W-1"), wall_face("W-2"), wall_face("W-3")]);
    let measured = measure(&dimension, &located);
    let values: Vec<_> = measured.segments.iter().map(|s| s.value).collect();
    assert_eq!(values, vec![Some(mm(1000)), Some(mm(1500))]);
    assert_eq!(measured.overall, Some(mm(2500)));
}

#[test]
fn the_value_follows_the_references_and_is_not_stored() {
    let dimension = horizontal(vec![wall_face("W-1"), wall_face("W-2")]);
    let before = Located::new()
        .with(wall_face("W-1"), Ok(vertical_at(0)))
        .with(wall_face("W-2"), Ok(vertical_at(2400)));
    let after = Located::new()
        .with(wall_face("W-1"), Ok(vertical_at(100)))
        .with(wall_face("W-2"), Ok(vertical_at(3000)));
    assert_eq!(
        measure(&dimension, &before).segments[0].value,
        Some(mm(2400))
    );
    // The same dimension, unchanged, and the new answer.
    assert_eq!(
        measure(&dimension, &after).segments[0].value,
        Some(mm(2900))
    );
    assert_eq!(
        dimension.to_record(&id("DOC-1")),
        dimension.to_record(&id("DOC-1"))
    );
}

#[test]
fn a_vertical_dimension_reads_along_y_between_door_centres() {
    let door = |name: &str| Reference::new(id(name), Feature::Centre);
    let located = Located::new()
        .with(door("DR-1"), Ok(horizontal_at(500)))
        .with(door("DR-2"), Ok(horizontal_at(3500)));
    let dimension = Dimension::new(
        id("D-2"),
        id("V-1"),
        Layout::vertical(mm(-300)).unwrap(),
        vec![door("DR-1"), door("DR-2")],
    )
    .unwrap();
    let measured = measure(&dimension, &located);
    assert_eq!(measured.segments[0].value, Some(mm(3000)));
    assert_eq!(measured.segments[0].middle, Some(Point::mm(-300, 2000)));
}

#[test]
fn a_point_reference_is_read_at_the_foot_of_its_perpendicular() {
    let edge = |name: &str| Reference::new(id(name), Feature::GridLine);
    let located = Located::new()
        .with(edge("G-1"), Ok(Locus::Point(Point::mm(100, 40))))
        .with(edge("G-2"), Ok(Locus::Point(Point::mm(4100, -900))));
    let dimension = horizontal(vec![edge("G-1"), edge("G-2")]);
    let measured = measure(&dimension, &located);
    assert_eq!(measured.segments[0].value, Some(mm(4000)));
    assert!(measured.segments[0].exact);
}

#[test]
fn a_slanted_line_is_read_where_it_meets_the_offset_line() {
    // A line at 45 degrees through the origin meets y = 1000 mm at x = 1000 mm; a vertical line at
    // 5000 mm meets it at 5000 mm.
    let located = Located::new()
        .with(
            wall_face("W-1"),
            Ok(Locus::line(Point::mm(0, 0), mm(1000), mm(1000))),
        )
        .with(wall_face("W-2"), Ok(vertical_at(5000)));
    let dimension = Dimension::new(
        id("D-3"),
        id("V-1"),
        Layout::horizontal(mm(1000)).unwrap(),
        vec![wall_face("W-1"), wall_face("W-2")],
    )
    .unwrap();
    let measured = measure(&dimension, &located);
    assert_eq!(measured.segments[0].value, Some(mm(4000)));
    assert!(measured.segments[0].exact);
}

#[test]
fn an_aligned_dimension_on_a_three_four_five_slope_is_exact() {
    let point = |name: &str| Reference::new(id(name), Feature::Centre);
    let located = Located::new()
        .with(point("A"), Ok(Locus::Point(Point::mm(0, 0))))
        .with(point("B"), Ok(Locus::Point(Point::mm(3000, 4000))));
    let direction = Direction::new(mm(3), mm(4)).unwrap();
    let dimension = Dimension::new(
        id("D-4"),
        id("V-1"),
        Layout::aligned(Point::mm(0, 0), direction).unwrap(),
        vec![point("A"), point("B")],
    )
    .unwrap();
    let measured = measure(&dimension, &located);
    assert_eq!(measured.segments[0].value, Some(mm(5000)));
    assert!(measured.segments[0].exact);
}

#[test]
fn an_irrational_value_is_the_nearest_step_and_says_it_is_not_exact() {
    // 1000 mm along x, read along a 45 degree line: 1000 / sqrt(2) mm = 4,525,483.3996 steps.
    let point = |name: &str| Reference::new(id(name), Feature::Centre);
    let located = Located::new()
        .with(point("A"), Ok(Locus::Point(Point::mm(0, 0))))
        .with(point("B"), Ok(Locus::Point(Point::mm(1000, 0))));
    let direction = Direction::new(mm(1), mm(1)).unwrap();
    let dimension = Dimension::new(
        id("D-5"),
        id("V-1"),
        Layout::aligned(Point::mm(0, 0), direction).unwrap(),
        vec![point("A"), point("B")],
    )
    .unwrap();
    let measured = measure(&dimension, &located);
    assert_eq!(
        measured.segments[0].value,
        Some(Length::from_steps(4_525_483))
    );
    assert!(!measured.segments[0].exact);
}

#[test]
fn a_deleted_reference_is_named_and_no_wrong_number_is_given() {
    // W-2 is gone: the table does not hold it.
    let located = Located::new()
        .with(wall_face("W-1"), Ok(vertical_at(0)))
        .with(wall_face("W-3"), Ok(vertical_at(1000)))
        .with(wall_face("W-4"), Ok(vertical_at(1800)));
    let dimension = horizontal(vec![
        wall_face("W-1"),
        wall_face("W-2"),
        wall_face("W-3"),
        wall_face("W-4"),
    ]);
    let measured = measure(&dimension, &located);
    let values: Vec<_> = measured.segments.iter().map(|s| s.value).collect();
    // The two segments W-2 ends have no value; the one between W-3 and W-4 still does.
    assert_eq!(values, vec![None, None, Some(mm(800))]);
    assert_eq!(measured.overall, None);
    assert!(measured.witnesses[1].is_none());
    assert_eq!(measured.unusable, vec![1]);
    let [warning] = measured.warnings.as_slice() else {
        panic!("one warning")
    };
    assert_eq!(warning.kind, REFERENCE_MISSING);
    assert_eq!(warning.elements, vec![id("D-1"), id("W-2")]);
    assert_eq!(warning.identity(), "dimension.reference_missing|D-1|W-2");
    assert_eq!(warning.severity.word(), "warning");
    assert_eq!(
        warning.words,
        "Dimension D-1: reference 2 (the exterior face of W-2) cannot be found: the element is \
         not in this model any more."
    );
}

#[test]
fn a_reference_with_no_place_in_the_view_says_why() {
    let level = Reference::new(id("L-1"), Feature::Level);
    let located = Located::new()
        .with(wall_face("W-1"), Ok(vertical_at(0)))
        .with(
            level.clone(),
            Err(Missing::no_such_part("a level has no place in a plan")),
        );
    let measured = measure(&horizontal(vec![wall_face("W-1"), level]), &located);
    assert_eq!(measured.segments[0].value, None);
    assert!(
        measured.warnings[0]
            .words
            .contains("a level has no place in a plan")
    );
    assert!(measured.warnings[0].words.contains("the level L-1"));
}

#[test]
fn a_line_along_the_offset_line_has_no_place_along_it() {
    // A horizontal face against a horizontal dimension.
    let located = Located::new()
        .with(wall_face("W-1"), Ok(vertical_at(0)))
        .with(wall_face("W-2"), Ok(horizontal_at(2000)));
    let measured = measure(
        &horizontal(vec![wall_face("W-1"), wall_face("W-2")]),
        &located,
    );
    assert_eq!(measured.segments[0].value, None);
    assert_eq!(measured.warnings[0].kind, REFERENCE_PARALLEL);
    assert_eq!(measured.warnings[0].elements, vec![id("D-1"), id("W-2")]);
}

#[test]
fn the_largest_distance_the_grid_holds_is_worked_out_exactly() {
    // Two points on an aligned line a little under 2^41 steps apart in x and y.
    let far = Length::from_steps(1_099_511_627_775);
    let near = Length::from_steps(-1_099_511_627_775);
    let point = |name: &str| Reference::new(id(name), Feature::Centre);
    let located = Located::new()
        .with(point("A"), Ok(Locus::Point(Point::new(near, near))))
        .with(point("B"), Ok(Locus::Point(Point::new(far, far))));
    let dimension = Dimension::new(
        id("D-6"),
        id("V-1"),
        Layout::aligned(Point::mm(0, 0), Direction::new(mm(1), mm(1)).unwrap()).unwrap(),
        vec![point("A"), point("B")],
    )
    .unwrap();
    let measured = measure(&dimension, &located);
    // 2199023255550 * sqrt 2 = 3109888511972.65: a real value, exactly rounded.
    assert!(measured.warnings.is_empty());
    assert_eq!(
        measured.segments[0].value,
        Some(Length::from_steps(3_109_888_511_973))
    );
    assert!(!measured.segments[0].exact);
}

#[test]
fn a_meeting_beyond_the_grid_is_a_warning_not_a_wrapped_number() {
    // A line that almost runs along the horizontal offset line meets it about 2^80 steps away.
    let limit = Length::from_steps(1_099_511_627_775);
    let nearly = Locus::line(
        Point::new(Length::from_steps(0), limit),
        limit,
        Length::from_steps(1),
    );
    let located = Located::new()
        .with(wall_face("W-1"), Ok(vertical_at(0)))
        .with(wall_face("W-2"), Ok(nearly));
    let dimension = Dimension::new(
        id("D-6"),
        id("V-1"),
        Layout::horizontal(mm(0)).unwrap(),
        vec![wall_face("W-1"), wall_face("W-2")],
    )
    .unwrap();
    let measured = measure(&dimension, &located);
    assert_eq!(measured.segments[0].value, None);
    assert_eq!(measured.overall, None);
    let [warning] = measured.warnings.as_slice() else {
        panic!("one warning")
    };
    assert_eq!(warning.kind, OUT_OF_RANGE);
    assert_eq!(warning.elements, vec![id("D-6"), id("W-2")]);
    assert!(warning.words.contains("reference 2"));
}

#[test]
fn equalities_are_reported_against_the_exact_values() {
    let refs = vec![
        wall_face("W-1"),
        wall_face("W-2"),
        wall_face("W-3"),
        wall_face("W-4"),
    ];
    let dimension = horizontal(refs.clone())
        .with_equalities(vec![vec![0, 1], vec![1, 2]])
        .unwrap_err();
    // Groups share segment 1: refused.
    assert!(dimension.to_string().contains("share none"));
    let dimension = horizontal(refs).with_equalities(vec![vec![0, 2]]).unwrap();
    let even = Located::new()
        .with(wall_face("W-1"), Ok(vertical_at(0)))
        .with(wall_face("W-2"), Ok(vertical_at(900)))
        .with(wall_face("W-3"), Ok(vertical_at(1400)))
        .with(wall_face("W-4"), Ok(vertical_at(2300)));
    assert_eq!(measure(&dimension, &even).equalities[0].equal, Some(true));
    let uneven = Located::new()
        .with(wall_face("W-1"), Ok(vertical_at(0)))
        .with(wall_face("W-2"), Ok(vertical_at(900)))
        .with(wall_face("W-3"), Ok(vertical_at(1400)))
        .with(wall_face("W-4"), Ok(vertical_at(2301)));
    assert_eq!(
        measure(&dimension, &uneven).equalities[0].equal,
        Some(false)
    );
    let broken = Located::new().with(wall_face("W-1"), Ok(vertical_at(0)));
    assert_eq!(measure(&dimension, &broken).equalities[0].equal, None);
    // The stored equality is data: the dimension still reads as before.
    assert_eq!(dimension.equalities(), &[vec![0, 2]]);
}

#[test]
fn the_shown_text_is_the_value_in_the_persons_units() {
    let units = Units::default();
    assert_eq!(display(mm(2400), &units).unwrap(), "2400 mm");
    // The default shows whole millimetres (12.5 mm is 12, half to even); one place shows it all.
    assert_eq!(
        display(Length::from_steps(80_000), &units).unwrap(),
        "12 mm"
    );
    let mut fine = Units::default();
    fine.set(
        Kind::Length,
        Format {
            unit: Unit::Millimetre,
            places: 1,
        },
    )
    .unwrap();
    assert_eq!(
        display(Length::from_steps(80_000), &fine).unwrap(),
        "12.5 mm"
    );
    let mut metres = Units::default();
    metres
        .set(
            Kind::Length,
            Format {
                unit: Unit::Metre,
                places: 3,
            },
        )
        .unwrap();
    assert_eq!(display(mm(2400), &metres).unwrap(), "2.400 m");
}

#[test]
fn labels_sit_mid_segment_and_know_whether_they_fit_once_text_can_say() {
    let located = Located::new()
        .with(wall_face("W-1"), Ok(vertical_at(0)))
        .with(wall_face("W-2"), Ok(vertical_at(400)));
    let dimension = horizontal(vec![wall_face("W-1"), wall_face("W-2")]);
    let measured = measure(&dimension, &located);
    let units = Units::default();
    let unknown = labels(&measured, &units, &Unmeasured).unwrap();
    assert_eq!(unknown.len(), 1);
    assert_eq!(unknown[0].text, "400 mm");
    assert_eq!(unknown[0].at, Point::mm(200, 5000));
    assert_eq!(unknown[0].fits, None);

    assert_eq!(
        labels(&measured, &units, &HundredPerCharacter).unwrap()[0].fits,
        Some(false)
    );
}

#[test]
fn a_reference_missing_why_is_readable() {
    let located = Located::new();
    let measured = measure(
        &horizontal(vec![wall_face("W-1"), wall_face("W-2")]),
        &located,
    );
    assert_eq!(measured.warnings.len(), 2);
    assert_eq!(Missing::deleted().why, Why::Deleted);
    assert_eq!(Missing::unreadable("x").why, Why::Unreadable);
}

/// A text engine that draws each character 100 mm wide, at 250 mm high.
struct HundredPerCharacter;

impl TextSize for HundredPerCharacter {
    fn size(&self, text: &str) -> Option<(Length, Length)> {
        let wide = i32::try_from(text.len()).ok()?.checked_mul(100)?;
        Some((Length::from_mm(wide), Length::from_mm(250)))
    }
}
