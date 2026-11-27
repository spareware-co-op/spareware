//! A view's detail items: they belong to one view and to no other, they read back from a
//! document's records, and they are drawn over the model with the pens of their styles.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    clippy::assert_is_empty,
    clippy::assigning_clones,
    clippy::assertions_on_constants,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::{
    CategoryId, DETAIL_COMPONENTS, DETAIL_LINES, FILLED_REGIONS, MASKING_REGIONS, SPOT_DIMENSIONS,
    category_of_kind,
};
use construct_detailing::{
    BuiltIn, Curve, DetailComponent, DetailLine, DetailRepeat, Detailing, Faces, Fill, ItemKind,
    Library, LineStyle, OwnStyle, PAPER, Plane, Region, Settings, SlopeRefusal, SpotSlope,
    StyleRef, is_detail_kind, render,
};
use construct_drawing::hatch::FillPatterns;
use construct_grids::Point;
use construct_model::element::Element;
use construct_params::{Angle, Length};
use construct_styles::patterns::DASHED;
use construct_styles::{Colour, LineStyles, Scale};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(x: i32, y: i32) -> Point {
    Point::new(Length::from_mm(x), Length::from_mm(y))
}

fn doc() -> Id {
    id("doc")
}

/// One roof rising 3 in 4, over a 6 m by 4 m footprint.
struct Roof;

impl Faces for Roof {
    fn plane_at(&self, face: &Id, point: Point) -> Result<Plane, SlopeRefusal> {
        if face.as_str() != "roof-1" {
            return Err(SlopeRefusal::Missing(format!("There is no roof {face}.")));
        }
        if point.x.steps() > 6000 * 6400 || point.y.steps() > 4000 * 6400 {
            return Err(SlopeRefusal::OffFace("Off the roof.".to_owned()));
        }
        Ok(Plane::through_points(
            [0, 0, 0],
            [6000 * 6400, 0, 0],
            [0, 4000 * 6400, 3000 * 6400],
        )
        .unwrap())
    }
}

/// Every kind of item, two views' worth.
fn records(library: &Library) -> Vec<Element> {
    let square = vec![mm(0, 0), mm(1000, 0), mm(1000, 1000), mm(0, 1000)];
    let mut records = vec![
        OwnStyle::new(
            id("grout"),
            "Grout",
            LineStyle::new(3, DASHED, Colour(9, 9, 9)),
        )
        .unwrap()
        .to_record(&doc()),
        DetailLine::new(
            id("line-1"),
            id("plan-a"),
            StyleRef::Own(id("grout")),
            Curve::straight(mm(0, 0), mm(3000, 0)).unwrap(),
        )
        .to_record(&doc()),
        DetailLine::new(
            id("line-2"),
            id("plan-b"),
            StyleRef::Built(BuiltIn::Fine),
            Curve::arc(mm(0, 0), mm(1000, 0), mm(0, 1000)).unwrap(),
        )
        .to_record(&doc()),
        Region::new(
            id("fill-1"),
            id("plan-a"),
            Fill::Pattern {
                pattern: "insulation".to_owned(),
                colour: Colour(0, 0, 255),
            },
            Some(StyleRef::Own(id("grout"))),
            square.clone(),
        )
        .unwrap()
        .to_record(&doc()),
        Region::new(id("mask-1"), id("plan-a"), Fill::Masking, None, square)
            .unwrap()
            .to_record(&doc()),
        DetailComponent::new(
            id("comp-1"),
            id("plan-a"),
            library,
            "timber-section-90x45",
            mm(2000, 2000),
            Angle::from_units(0),
        )
        .unwrap()
        .to_record(&doc()),
        DetailRepeat::new(
            id("rep-1"),
            id("plan-a"),
            library,
            "blocking-cross-90x45",
            (mm(0, 0), mm(3000, 0)),
            Length::from_mm(600),
            Angle::from_units(0),
        )
        .unwrap()
        .to_record(&doc()),
        SpotSlope::new(id("slope-1"), id("plan-a"), id("roof-1"), mm(3000, 2000))
            .unwrap()
            .to_record(&doc()),
    ];
    // A record of another kind is left alone.
    let mut wall = records[1].clone();
    wall.kind = "wall.layered".to_owned();
    wall.id = id("wall-1");
    records.push(wall);
    records
}

#[test]
fn a_documents_detail_items_are_found_by_their_kind_and_by_their_view() {
    let library = Library::built_in().unwrap();
    // The last record holds detail props but is a wall: it is left alone.
    let all = records(&library);
    let found = Detailing::from_records(&all, &library).unwrap();
    assert_eq!(found.styles().all().len(), 1);
    let a = found.of_view(&id("plan-a"));
    assert_eq!(
        (
            a.lines.len(),
            a.regions.len(),
            a.components.len(),
            a.repeats.len(),
            a.slopes.len()
        ),
        (1, 2, 1, 1, 1)
    );
    // The other view holds its own and nothing of the first's.
    let b = found.of_view(&id("plan-b"));
    assert_eq!(b.len(), 1);
    assert_eq!(b.lines[0].id(), &id("line-2"));
    assert!(found.of_view(&id("plan-c")).is_empty());
    assert_eq!(found.kind_of(&id("fill-1")), Some(ItemKind::Region));
    assert_eq!(found.kind_of(&id("mask-1")), Some(ItemKind::Masking));
    assert_eq!(found.kind_of(&id("rep-1")), Some(ItemKind::Repeat));
    assert_eq!(found.kind_of(&id("nothing")), None);
    // The style is used by a line and a region's boundary, in order of ID.
    assert_eq!(
        found.using_style(&StyleRef::Own(id("grout"))),
        [id("fill-1"), id("line-1")]
    );
    assert_eq!(found.reading(&id("roof-1")).len(), 1);
}

#[test]
fn every_kind_is_a_detail_kind_in_its_category_and_has_no_shape() {
    let library = Library::built_in().unwrap();
    let all = records(&library);
    for record in &all[..all.len() - 1] {
        assert!(is_detail_kind(&record.kind), "{}", record.kind);
        assert!(category_of_kind(&record.kind).is_ok(), "{}", record.kind);
        // Nothing stored but the record's own inputs: no level, no placement, no references.
        assert_eq!(record.level, 0);
        assert!(record.references.is_empty());
        // The category the record says is the category of its kind.
        let said = record.props.get("@category").unwrap();
        assert_eq!(
            said,
            &category_of_kind(&record.kind).unwrap().0.to_string(),
            "{}",
            record.kind
        );
    }
    assert!(!is_detail_kind("wall.layered"));
}

#[test]
fn an_unknown_detail_kind_is_refused_not_skipped() {
    let library = Library::built_in().unwrap();
    let mut all = records(&library);
    all.last_mut().unwrap().kind = "detail.cloud".to_owned();
    assert!(Detailing::from_records(&all, &library).is_err());
}

fn settings<'a>(
    own: &'a construct_detailing::OwnStyles,
    lines: &'a LineStyles,
    fills: &'a FillPatterns,
    library: &'a Library,
    shown: &'a dyn Fn(CategoryId) -> bool,
) -> Settings<'a> {
    Settings {
        scale: Scale::new(50).unwrap(),
        lines,
        fills,
        own,
        library,
        shown,
    }
}

#[test]
fn a_view_draws_its_items_over_its_model_and_never_anothers() {
    let library = Library::built_in().unwrap();
    let all = records(&library);
    let found = Detailing::from_records(&all[..all.len() - 1], &library).unwrap();
    let (lines, fills) = (LineStyles::default_metric(), FillPatterns::default_metric());
    let everything = |_: CategoryId| true;
    let set = settings(found.styles(), &lines, &fills, &library, &everything);
    let drawn = render(&found.of_view(&id("plan-a")), &set, &Roof);
    assert!(drawn.skipped.is_empty(), "{:?}", drawn.skipped);
    let over = &drawn.over;
    // One masking region: an opaque fill in the paper's colour, with no pattern lines.
    assert_eq!(over.fills.len(), 1);
    assert_eq!(over.fills[0].colour, PAPER);
    assert_eq!(over.fills[0].element, id("mask-1"));
    assert_eq!(over.fills[0].outline.len(), 4);
    // Pattern lines: the filled region's insulation (a square lattice), a timber section's
    // timber fill, and each of the repeat's none (the cross has no fill).
    let by = |element: &str| {
        over.hatches
            .iter()
            .filter(|h| h.element == id(element))
            .count()
    };
    assert_eq!(by("fill-1"), 1);
    assert_eq!(by("comp-1"), 1);
    assert_eq!(by("rep-1"), 0);
    let hatch = over
        .hatches
        .iter()
        .find(|h| h.element == id("fill-1"))
        .unwrap();
    assert_eq!(hatch.colour, Colour(0, 0, 255));
    assert!(!hatch.lines.is_empty());
    // Lines: the square's boundary (4 edges in the own style), the detail line (1), the timber's
    // four, six crosses of six lines (36), and the slope's arrow (3).
    let strokes = |element: &str| {
        over.strokes
            .iter()
            .filter(|s| s.element == id(element))
            .count()
    };
    assert_eq!(strokes("fill-1"), 4);
    assert_eq!(strokes("line-1"), 1);
    assert_eq!(strokes("comp-1"), 4);
    assert_eq!(strokes("rep-1"), 36);
    assert_eq!(strokes("slope-1"), 3);
    assert_eq!(strokes("line-2"), 0);
    assert_eq!(over.strokes.len(), 4 + 1 + 4 + 36 + 3);
    // The own style's pen is model pen 3: 0.25 mm up to 1:20, 0.18 at 1:50.
    let line = over
        .strokes
        .iter()
        .find(|s| s.element == id("line-1"))
        .unwrap();
    assert_eq!(line.width.get(), 180);
    assert_eq!(line.colour, Colour(9, 9, 9));
    // The other view's arc is drawn there.
    let other = render(&found.of_view(&id("plan-b")), &set, &Roof);
    assert_eq!(other.over.strokes.len(), 1);
    assert!(other.over.fills.is_empty());
    assert!(other.over.hatches.is_empty());
}

#[test]
fn a_spot_slope_arrow_runs_down_the_face_and_is_eight_millimetres_of_paper_long() {
    let library = Library::built_in().unwrap();
    let all = records(&library);
    let found = Detailing::from_records(&all[..all.len() - 1], &library).unwrap();
    let (lines, fills) = (LineStyles::default_metric(), FillPatterns::default_metric());
    let everything = |_: CategoryId| true;
    let set = settings(found.styles(), &lines, &fills, &library, &everything);
    let drawn = render(&found.of_view(&id("plan-a")), &set, &Roof);
    let shaft = drawn
        .over
        .strokes
        .iter()
        .find(|s| s.element == id("slope-1"))
        .unwrap();
    // At 1:50, 8 mm of paper is 400 mm of the model: 2,560,000 steps, straight towards -y from
    // (3000, 2000) mm, since the roof falls to -y.
    let construct_drawing::drawing::Shape::Segment { start, end } = shaft.shape else {
        panic!("a shaft is a segment");
    };
    assert_eq!((start.x, start.y, start.den), (3000 * 6400, 2000 * 6400, 1));
    assert_eq!((end.x, end.y), (3000 * 6400, 2000 * 6400 - 400 * 6400));
}

#[test]
fn what_cannot_be_drawn_is_named_and_the_rest_is_drawn() {
    let library = Library::built_in().unwrap();
    let mut all = records(&library);
    all.pop();
    // A slope on a roof that is not there, and a region whose pattern the table lacks.
    all.push(
        SpotSlope::new(id("slope-2"), id("plan-a"), id("roof-9"), mm(10, 10))
            .unwrap()
            .to_record(&doc()),
    );
    all.push(
        Region::new(
            id("fill-2"),
            id("plan-a"),
            Fill::Pattern {
                pattern: "sparkle".to_owned(),
                colour: Colour(0, 0, 0),
            },
            None,
            vec![mm(0, 0), mm(500, 0), mm(0, 500)],
        )
        .unwrap()
        .to_record(&doc()),
    );
    let found = Detailing::from_records(&all, &library).unwrap();
    let (lines, fills) = (LineStyles::default_metric(), FillPatterns::default_metric());
    let everything = |_: CategoryId| true;
    let set = settings(found.styles(), &lines, &fills, &library, &everything);
    let drawn = render(&found.of_view(&id("plan-a")), &set, &Roof);
    let named: Vec<(String, bool)> = drawn
        .skipped
        .iter()
        .map(|s| (s.item.to_string(), !s.why.is_empty()))
        .collect();
    assert_eq!(
        named,
        [("fill-2".to_owned(), true), ("slope-2".to_owned(), true)]
    );
    assert_eq!(drawn.over.fills.len(), 1);
    assert_eq!(drawn.over.strokes.len(), 4 + 1 + 4 + 36 + 3);
}

#[test]
fn a_view_that_hides_a_category_leaves_its_items_out() {
    let library = Library::built_in().unwrap();
    let all = records(&library);
    let found = Detailing::from_records(&all[..all.len() - 1], &library).unwrap();
    let (lines, fills) = (LineStyles::default_metric(), FillPatterns::default_metric());
    for (hidden, fewer) in [
        (DETAIL_LINES, 1),
        (SPOT_DIMENSIONS, 3),
        (DETAIL_COMPONENTS, 40),
        (MASKING_REGIONS, 0),
        (FILLED_REGIONS, 0),
    ] {
        let shown = move |c: CategoryId| c != hidden;
        let set = settings(found.styles(), &lines, &fills, &library, &shown);
        let drawn = render(&found.of_view(&id("plan-a")), &set, &Roof);
        let full = 4 + 1 + 4 + 36 + 3;
        let regions = usize::from(hidden == FILLED_REGIONS) * 4;
        assert_eq!(
            drawn.over.strokes.len(),
            full - fewer - regions,
            "{hidden:?}"
        );
        assert_eq!(
            drawn.over.fills.len(),
            usize::from(hidden != MASKING_REGIONS)
        );
    }
}
