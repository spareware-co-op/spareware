//! A view's crop region: the rectangle and the shaped outline with their exact checks, every
//! refusal, the stored form and its round trip, the kinds that hold one, and old views that have
//! none.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_params::Length;
use construct_phases::{DesignOptions, ElementStaging, PhaseList};
use construct_styles::Scale;
use construct_views::{
    CROP_KEY, Crop, CutLine, DetailLevel, PlanSetup, Point, Region, Side, View, ViewError,
    ViewKind, ViewRange,
};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn p(x: i32, y: i32) -> Point {
    Point {
        x: Length::from_mm(x),
        y: Length::from_mm(y),
    }
}

fn staging() -> ElementStaging {
    let phases = PhaseList::new().add(id("ph-a"), "Existing").unwrap();
    ElementStaging::new(&phases, &DesignOptions::new(), id("ph-a"), None, None).unwrap()
}

fn plan() -> ViewKind {
    let mm = Length::from_mm;
    ViewKind::FloorPlan(PlanSetup {
        level: id("l1"),
        range: ViewRange::on_level(&id("l1"), mm(2300), mm(1200), mm(0), mm(-500)),
    })
}

fn section() -> ViewKind {
    let line = CutLine::new(p(0, 0), p(6000, 0), Side::Left, Length::from_mm(5000)).unwrap();
    ViewKind::Section(line)
}

fn view_of(kind: ViewKind) -> View {
    View::new(
        id("v1"),
        "View",
        Scale::new(100).unwrap(),
        DetailLevel::Medium,
        kind,
        staging(),
    )
    .unwrap()
}

fn outline(points: &[(i32, i32)]) -> Result<Region, ViewError> {
    Region::outline(points.iter().map(|&(x, y)| p(x, y)).collect())
}

fn rect() -> Region {
    Region::rectangle(p(0, 0), p(4000, 3000)).unwrap()
}

#[test]
fn a_rectangle_needs_its_minimum_below_its_maximum_on_both_axes() {
    assert_eq!(rect().key(), "rectangle");
    assert_eq!(
        rect().corners(),
        vec![p(0, 0), p(4000, 0), p(4000, 3000), p(0, 3000)]
    );
    assert_eq!(rect().bounds(), (p(0, 0), p(4000, 3000)));
    assert_eq!(
        rect().size(),
        Some((Length::from_mm(4000), Length::from_mm(3000)))
    );
    for (min, max) in [
        (p(0, 0), p(0, 3000)),
        (p(0, 0), p(4000, 0)),
        (p(4000, 0), p(0, 3000)),
        (p(0, 3000), p(4000, 0)),
        (p(1, 1), p(1, 1)),
    ] {
        assert_eq!(
            Region::rectangle(min, max).unwrap_err(),
            ViewError::CropEmptyRectangle
        );
    }
}

#[test]
fn a_shaped_outline_is_simple_with_three_or_more_corners_and_an_area() {
    // An L shape, either way round, a triangle and a concave arrow all hold.
    let l = [
        (0, 0),
        (4000, 0),
        (4000, 1000),
        (1000, 1000),
        (1000, 3000),
        (0, 3000),
    ];
    let region = outline(&l).unwrap();
    assert_eq!(region.key(), "outline");
    assert_eq!(region.corners().len(), 6);
    assert_eq!(region.bounds(), (p(0, 0), p(4000, 3000)));
    let mut reversed = l;
    reversed.reverse();
    assert!(outline(&reversed).is_ok());
    assert!(outline(&[(0, 0), (1000, 0), (0, 1000)]).is_ok());
    assert!(outline(&[(0, 0), (4000, 2000), (0, 4000), (1000, 2000)]).is_ok());
    // One grid step is not a tolerance: an almost flat but real triangle holds.
    let steps = |x, y| Point {
        x: Length::from_steps(x),
        y: Length::from_steps(y),
    };
    assert!(Region::outline(vec![steps(0, 0), steps(1_000_000, 1), steps(2_000_000, 0)]).is_ok());
}

#[test]
fn an_outline_with_fewer_than_three_corners_is_refused() {
    assert_eq!(
        outline(&[]).unwrap_err(),
        ViewError::CropFewCorners { count: 0 }
    );
    assert_eq!(
        outline(&[(0, 0), (1000, 0)]).unwrap_err(),
        ViewError::CropFewCorners { count: 2 }
    );
}

#[test]
fn a_corner_given_twice_in_a_row_is_refused() {
    assert_eq!(
        outline(&[(0, 0), (1000, 0), (1000, 0), (0, 1000)]).unwrap_err(),
        ViewError::CropRepeatedCorner { corner: 2 }
    );
    // The last corner equal to the first is the same.
    assert_eq!(
        outline(&[(0, 0), (1000, 0), (0, 1000), (0, 0)]).unwrap_err(),
        ViewError::CropRepeatedCorner { corner: 4 }
    );
}

#[test]
fn an_outline_that_crosses_itself_is_refused_naming_the_edges() {
    // A bow tie: edges 1 and 3 cross.
    let bow = outline(&[(0, 0), (1000, 1000), (1000, 0), (0, 1000)]).unwrap_err();
    assert_eq!(
        bow,
        ViewError::CropSelfCrossing {
            first: 1,
            second: 3
        }
    );
    assert_eq!(
        bow.to_string(),
        "A crop outline must not cross itself: edges 1 and 3 meet."
    );
    // A corner that touches another edge is a meeting too, with no gap allowed.
    assert!(matches!(
        outline(&[(0, 0), (4000, 0), (4000, 3000), (2000, 0), (0, 3000)]),
        Err(ViewError::CropSelfCrossing { .. })
    ));
    // A pinch at a shared point: two lobes joined at one corner twice.
    assert!(matches!(
        outline(&[
            (0, 0),
            (2000, 0),
            (2000, 2000),
            (2000, 0),
            (4000, 0),
            (4000, 2000)
        ]),
        Err(ViewError::CropRepeatedCorner { .. } | ViewError::CropSelfCrossing { .. })
    ));
    // Edges that lie along each other.
    assert!(matches!(
        outline(&[(0, 0), (3000, 0), (1000, 0), (1000, 2000)]),
        Err(ViewError::CropSelfCrossing { .. })
    ));
    // Neighbours folding back along one line.
    assert_eq!(
        outline(&[(0, 0), (3000, 0), (1000, 0)]).unwrap_err(),
        ViewError::CropSelfCrossing {
            first: 1,
            second: 2
        }
    );
    // The closing edge folding back on the first.
    assert!(matches!(
        outline(&[(1000, 0), (3000, 0), (3000, 2000), (2000, 0)]),
        Err(ViewError::CropSelfCrossing { .. })
    ));
}

#[test]
fn an_outline_of_no_area_is_refused() {
    assert!(matches!(
        outline(&[(0, 0), (1000, 0), (2000, 0)]),
        Err(ViewError::CropSelfCrossing { .. } | ViewError::CropNoArea)
    ));
    assert!(matches!(
        outline(&[(0, 0), (1000, 1000), (2000, 2000)]),
        Err(ViewError::CropSelfCrossing { .. } | ViewError::CropNoArea)
    ));
}

#[test]
fn coordinates_too_large_to_test_are_refused_not_rounded() {
    let big = |x, y| Point {
        x: Length::from_steps(x),
        y: Length::from_steps(y),
    };
    let refused = Region::outline(vec![
        big(i64::MIN, i64::MIN),
        big(i64::MAX, i64::MIN),
        big(0, i64::MAX),
    ])
    .unwrap_err();
    assert!(matches!(refused, ViewError::OutOfRange(_)), "{refused}");
}

#[test]
fn a_crop_has_its_switches_and_an_optional_annotation_region() {
    let crop = Crop::new(rect());
    assert!(crop.is_on() && crop.is_shown());
    assert!(crop.annotation().is_none());
    let more = Region::rectangle(p(-500, -500), p(4500, 3500)).unwrap();
    let crop = crop
        .with_on(false)
        .with_shown(false)
        .with_annotation(Some(more.clone()));
    assert!(!crop.is_on() && !crop.is_shown());
    assert_eq!(crop.annotation(), Some(&more));
    assert_eq!(crop.region(), &rect());
    assert_eq!(crop.with_annotation(None).annotation(), None);
}

#[test]
fn a_crop_is_held_by_plans_sections_and_elevations_only() {
    let crop = Crop::new(rect());
    for kind in [plan(), section()] {
        assert!(Crop::check_kind(&kind).is_ok());
        let view = view_of(kind).with_crop(Some(crop.clone()));
        assert_eq!(view.crop(), Some(&crop));
    }
    for (kind, words) in [
        (ViewKind::ThreeD, "three d"),
        (ViewKind::Drafting, "drafting"),
        (ViewKind::Legend, "legend"),
    ] {
        let refusal = Crop::check_kind(&kind).unwrap_err();
        assert_eq!(
            refusal,
            ViewError::NoCropOnKind {
                kind: words.to_owned()
            }
        );
        // A record of such a view with a crop does not read.
        let held = view_of(plan()).with_crop(Some(Crop::new(rect())));
        let text = held
            .to_record(&id("doc"))
            .props
            .get(CROP_KEY)
            .unwrap()
            .clone();
        let mut record = view_of(kind).to_record(&id("doc"));
        record.props.insert(CROP_KEY.to_owned(), text);
        assert!(View::from_record(&record).is_err());
    }
}

#[test]
fn a_crop_is_stored_in_one_spelling_and_reads_back() {
    let shaped = outline(&[
        (0, 0),
        (4000, 0),
        (4000, 1000),
        (1000, 1000),
        (1000, 3000),
        (0, 3000),
    ])
    .unwrap();
    let crops = [
        Crop::new(rect()),
        Crop::new(rect()).with_on(false).with_shown(false),
        Crop::new(shaped.clone()).with_annotation(Some(rect())),
        Crop::new(rect()).with_annotation(Some(shaped)),
    ];
    for crop in crops {
        for kind in [plan(), section()] {
            let view = view_of(kind).with_crop(Some(crop.clone()));
            let record = view.to_record(&id("doc"));
            assert!(record.props.contains_key(CROP_KEY));
            let read = View::from_record(&record).unwrap();
            assert_eq!(read, view);
            assert_eq!(read.to_record(&id("doc")), record, "the same bytes");
        }
    }
    // The text, pinned: a rectangle at 4 m by 3 m.
    let record = view_of(plan())
        .with_crop(Some(Crop::new(rect())))
        .to_record(&id("doc"));
    let text = record.props.get(CROP_KEY).unwrap();
    assert!(text.starts_with(r#"{"format":1,"on":true,"shown":true,"region":{"rectangle":["#));
    assert!(text.ends_with(r#"]},"annotation":null}"#), "{text}");
}

#[test]
fn an_old_view_has_no_crop_and_reads_and_writes_as_before() {
    let view = view_of(plan());
    assert!(view.crop().is_none());
    let record = view.to_record(&id("doc"));
    assert!(!record.props.contains_key(CROP_KEY));
    assert_eq!(View::from_record(&record).unwrap(), view);
}

#[test]
fn a_record_with_a_crop_that_is_not_as_written_is_refused() {
    let view = view_of(plan()).with_crop(Some(Crop::new(rect())));
    let good = view.to_record(&id("doc"));
    let text = good.props.get(CROP_KEY).unwrap().clone();
    let cases = [
        text.replace("\"format\":1", "\"format\":2"),
        text.replace("\"on\":true", "\"on\":1"),
        format!("{text} "),
        text.replace("\"annotation\":null", "\"annotation\":null,\"extra\":1"),
        text.replace("length:q0:", "length:"),
        // A rectangle turned inside out, and an outline that crosses.
        r#"{"format":1,"on":true,"shown":true,"region":{"rectangle":["length:q0:25600000","length:q0:0","length:q0:0","length:q0:19200000"]},"annotation":null}"#.to_owned(),
        r#"{"format":1,"on":true,"shown":true,"region":{"outline":[["length:q0:0","length:q0:0"],["length:q0:6400000","length:q0:6400000"],["length:q0:6400000","length:q0:0"],["length:q0:0","length:q0:6400000"]]},"annotation":null}"#.to_owned(),
        "{\"format\":1,\"on\":true,\"shown\":true,\"region\":{\"outline\":[]},\"annotation\":null}"
            .to_owned(),
        "not json".to_owned(),
    ];
    for bad in cases {
        let mut record = good.clone();
        record.props.insert(CROP_KEY.to_owned(), bad.clone());
        let refusal = View::from_record(&record).unwrap_err().to_string();
        assert!(refusal.contains("The view record"), "{bad} gave {refusal}");
    }
}
