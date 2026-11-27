//! Detail lines and regions: their data, their checks, their areas worked by hand, and the one
//! spelling their records are written in.

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

use construct_detailing::{
    BuiltIn, Curve, DetailError, DetailLine, Fill, LineStyle, OwnStyle, OwnStyles, Region,
    StyleRef, check_outline,
};
use construct_grids::Point;
use construct_params::{Area, Length};
use construct_styles::patterns::{DASHED, SOLID};
use construct_styles::{Colour, LineStyles};
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

fn timber() -> Fill {
    Fill::Pattern {
        pattern: "timber".to_owned(),
        colour: Colour(0, 0, 0),
    }
}

/// An L: 4 m by 2 m with a 2 m by 2 m block on one end.
fn ell() -> Vec<Point> {
    vec![
        mm(0, 0),
        mm(4000, 0),
        mm(4000, 2000),
        mm(2000, 2000),
        mm(2000, 4000),
        mm(0, 4000),
    ]
}

// ---------------------------------------------------------------------------------------------
// Regions

#[test]
fn an_l_shaped_region_has_the_area_worked_by_hand() {
    // 4000 x 2000 = 8,000,000 mm2, plus 2000 x 2000 = 4,000,000: 12,000,000 mm2, which is 12 m2.
    let region = Region::new(id("r1"), id("v1"), timber(), None, ell()).unwrap();
    let area = region.area().unwrap();
    assert_eq!(area.area, Area::from_mm2(12_000_000));
    assert!(area.exact);
    assert_eq!(area.area.to_string(), "12000000 mm²");
}

#[test]
fn the_direction_round_does_not_change_the_area_and_hatching_is_anticlockwise() {
    let mut backwards = ell();
    backwards.reverse();
    let region = Region::new(id("r1"), id("v1"), timber(), None, backwards).unwrap();
    assert_eq!(region.area().unwrap().area, Area::from_mm2(12_000_000));
    let ring = region.anticlockwise();
    assert_eq!(ring.len(), 6);
    // Anticlockwise: twice the signed area is positive.
    let twice: i128 = ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| a[0] * b[1] - a[1] * b[0])
        .sum();
    assert!(twice > 0);
}

#[test]
fn an_odd_twice_area_is_reported_as_not_exact() {
    // A triangle of 1 step by 1 step: twice its area is 1 square step; half rounds up.
    let step = |x: i64, y: i64| Point::new(Length::from_steps(x), Length::from_steps(y));
    let region = Region::new(
        id("r1"),
        id("v1"),
        Fill::Masking,
        None,
        vec![step(0, 0), step(1, 0), step(0, 1)],
    )
    .unwrap();
    let area = region.area().unwrap();
    assert_eq!(area.twice, 1);
    assert_eq!(area.area, Area::from_square_steps(1));
    assert!(!area.exact);
}

#[test]
fn an_outline_must_be_a_simple_loop() {
    // Fewer than three corners.
    assert_eq!(
        check_outline(&[mm(0, 0), mm(1, 0)]),
        Err(DetailError::FewCorners(2))
    );
    // A corner given twice in a row.
    assert_eq!(
        check_outline(&[mm(0, 0), mm(1000, 0), mm(1000, 0), mm(0, 1000)]),
        Err(DetailError::RepeatedCorner(2))
    );
    // A bow tie crosses itself.
    let bow = [mm(0, 0), mm(1000, 1000), mm(1000, 0), mm(0, 1000)];
    assert_eq!(
        check_outline(&bow),
        Err(DetailError::SelfCrossing {
            first: 1,
            second: 3
        })
    );
    // An edge that only touches another at a corner of its own still touches.
    let touching = [
        mm(0, 0),
        mm(2000, 0),
        mm(2000, 2000),
        mm(1000, 0),
        mm(0, 2000),
    ];
    assert!(matches!(
        check_outline(&touching),
        Err(DetailError::SelfCrossing { .. })
    ));
    // Corners on one line have no area; neighbours that fold back are a crossing.
    assert_eq!(
        check_outline(&[mm(0, 0), mm(1000, 0), mm(2000, 0)]),
        Err(DetailError::SelfCrossing {
            first: 1,
            second: 3
        })
    );
    assert!(check_outline(&ell()).is_ok());
}

#[test]
fn an_outline_past_the_grid_is_refused() {
    let far = Point::new(Length::from_steps((1_i64 << 40) + 5), Length::from_steps(0));
    assert!(matches!(
        check_outline(&[mm(0, 0), far, mm(0, 1000)]),
        Err(DetailError::OutOfRange(_))
    ));
}

#[test]
fn a_masking_region_has_no_pattern_and_cannot_be_given_one() {
    let masking = Region::new(id("m1"), id("v1"), Fill::Masking, None, ell()).unwrap();
    assert!(masking.is_masking());
    assert!(masking.clone().with_fill(timber()).is_err());
    let filled = Region::new(id("r1"), id("v1"), timber(), None, ell()).unwrap();
    assert!(filled.with_fill(Fill::Masking).is_err());
}

#[test]
fn a_region_record_is_read_back_and_nothing_else_is() {
    let region = Region::new(
        id("r1"),
        id("v1"),
        timber(),
        Some(StyleRef::Built(BuiltIn::Bold)),
        ell(),
    )
    .unwrap();
    let record = region.to_record(&doc());
    assert_eq!(record.kind, "detail.region");
    assert_eq!(Region::from_record(&record).unwrap(), region);
    // Written again it is the same bytes.
    assert_eq!(
        Region::from_record(&record).unwrap().to_record(&doc()),
        record
    );
    // A masking region is another kind of record.
    let masking = Region::new(id("m1"), id("v1"), Fill::Masking, None, ell()).unwrap();
    let record = masking.to_record(&doc());
    assert_eq!(record.kind, "detail.masking");
    assert!(!record.props.contains_key("@detail.region.pattern"));
    assert_eq!(Region::from_record(&record).unwrap(), masking);
}

#[test]
fn a_region_record_that_is_not_as_written_is_refused() {
    let region = Region::new(id("r1"), id("v1"), timber(), None, ell()).unwrap();
    let good = region.to_record(&doc());
    let tamper = |change: &dyn Fn(&mut construct_model::element::Element)| {
        let mut record = good.clone();
        change(&mut record);
        Region::from_record(&record)
    };
    // A corner removed leaves a record that does not give back what it wrote.
    assert!(
        tamper(&|r| {
            r.props.remove("@detail.region.corner.0005.x");
        })
        .is_err()
    );
    // A pattern's colour in capitals is not the one spelling.
    assert!(
        tamper(&|r| {
            r.props
                .insert("@detail.region.colour".to_owned(), "#00000A".to_owned());
        })
        .is_err()
    );
    // A length in the old spelling reads, and is then not as written.
    assert!(
        tamper(&|r| {
            r.props.insert(
                "@detail.region.corner.0001.x".to_owned(),
                "length:4000".to_owned(),
            );
        })
        .is_err()
    );
    // A level or a placement means it is not a detail record.
    assert!(tamper(&|r| r.level = 1).is_err());
    assert!(tamper(&|r| r.kind = "wall.layered".to_owned()).is_err());
    // A view that is no ID.
    assert!(
        tamper(&|r| {
            r.props
                .insert("@detail.view".to_owned(), "not an id".to_owned());
        })
        .is_err()
    );
}

// ---------------------------------------------------------------------------------------------
// Lines

#[test]
fn a_straight_line_needs_two_different_ends_inside_the_grid() {
    assert!(Curve::straight(mm(0, 0), mm(3000, 0)).is_ok());
    assert_eq!(
        Curve::straight(mm(5, 5), mm(5, 5)),
        Err(DetailError::ZeroLength)
    );
    let far = Point::new(Length::from_steps(1 << 41), Length::from_steps(0));
    assert!(matches!(
        Curve::straight(mm(0, 0), far),
        Err(DetailError::OutOfRange(_))
    ));
}

#[test]
fn an_arc_is_the_solvers_and_its_end_must_lie_on_its_circle() {
    // A quarter of a circle of radius 1 m, anticlockwise from (1000, 0) to (0, 1000).
    assert!(Curve::arc(mm(0, 0), mm(1000, 0), mm(0, 1000)).is_ok());
    // The end 10 mm off the circle is not an arc.
    assert!(matches!(
        Curve::arc(mm(0, 0), mm(1000, 0), mm(0, 1010)),
        Err(DetailError::BadArc(_))
    ));
    // The end 0.005 mm off is inside the model's tolerance of 0.01 mm.
    let near = Point::new(Length::from_mm(0), Length::from_steps(1000 * 6400 + 32));
    assert!(Curve::arc(mm(0, 0), mm(1000, 0), near).is_ok());
    // The centre at the start has no radius.
    assert!(matches!(
        Curve::arc(mm(0, 0), mm(0, 0), mm(0, 1000)),
        Err(DetailError::BadArc(_))
    ));
}

#[test]
fn a_line_record_is_read_back_for_a_straight_line_and_an_arc() {
    let straight = DetailLine::new(
        id("l1"),
        id("v1"),
        StyleRef::Built(BuiltIn::Standard),
        Curve::straight(mm(0, 0), mm(3000, 1500)).unwrap(),
    );
    let record = straight.to_record(&doc());
    assert_eq!(record.kind, "detail.line");
    assert_eq!(
        record.props.get("@detail.line.style").unwrap(),
        "builtin:standard"
    );
    assert_eq!(DetailLine::from_record(&record).unwrap(), straight);
    let arc = DetailLine::new(
        id("l2"),
        id("v1"),
        StyleRef::Own(id("s1")),
        Curve::arc(mm(0, 0), mm(1000, 0), mm(0, 1000)).unwrap(),
    );
    let record = arc.to_record(&doc());
    assert_eq!(record.props.get("@detail.line.shape").unwrap(), "arc");
    assert_eq!(DetailLine::from_record(&record).unwrap(), arc);
    // An edit of the points writes the same record kind with the new points.
    let moved = arc.with_curve(Curve::arc(mm(0, 0), mm(0, 1000), mm(-1000, 0)).unwrap());
    assert_eq!(
        DetailLine::from_record(&moved.to_record(&doc())).unwrap(),
        moved
    );
}

// ---------------------------------------------------------------------------------------------
// Line styles

#[test]
fn the_six_built_in_styles_are_in_the_default_pens_and_patterns() {
    let styles = LineStyles::default_metric();
    for built in BuiltIn::ALL {
        built.style().check(&styles).unwrap();
        assert_eq!(BuiltIn::from_key(built.key()), Some(built));
        let stored = StyleRef::Built(built).stored();
        assert_eq!(
            StyleRef::from_stored(&stored).unwrap(),
            StyleRef::Built(built)
        );
    }
    assert!(StyleRef::from_stored("builtin:fancy").is_err());
}

#[test]
fn a_style_prints_thinner_at_a_small_scale() {
    let own = OwnStyles::new();
    let styles = LineStyles::default_metric();
    let bold = StyleRef::Built(BuiltIn::Bold);
    let at = |n: u32| {
        own.printed(&bold, &styles, construct_styles::Scale::new(n).unwrap())
            .unwrap()
            .width
            .get()
    };
    // Pen 6 is 0.7 mm up to 1:20, one rung down (0.5 mm) at 1:50 and 1:100, two (0.35 mm) at 1:200.
    assert_eq!(at(20), 700);
    assert_eq!(at(50), 500);
    assert_eq!(at(100), 500);
    assert_eq!(at(200), 350);
}

#[test]
fn a_persons_own_style_is_a_record_with_its_own_name() {
    let style = OwnStyle::new(
        id("s1"),
        "Grout joint",
        LineStyle::new(3, DASHED, Colour(200, 30, 30)),
    )
    .unwrap();
    let record = style.to_record(&doc());
    assert_eq!(record.kind, "detail.linestyle");
    assert_eq!(record.props.get("@detail.style.colour").unwrap(), "#c81e1e");
    assert_eq!(OwnStyle::from_record(&record).unwrap(), style);
    let mut own = OwnStyles::from_records([&record]).unwrap();
    assert_eq!(
        own.resolve(&StyleRef::Own(id("s1"))).unwrap(),
        style.style()
    );
    // Names are unique, case aside, and a built-in name is taken.
    let again = OwnStyle::new(
        id("s2"),
        "GROUT JOINT",
        LineStyle::new(2, SOLID, Colour(0, 0, 0)),
    )
    .unwrap();
    assert!(own.add(again).is_err());
    assert!(OwnStyle::new(id("s3"), "Fine", LineStyle::new(2, SOLID, Colour(0, 0, 0))).is_err());
    assert!(OwnStyle::new(id("s3"), "  ", LineStyle::new(2, SOLID, Colour(0, 0, 0))).is_err());
    // A style that is not there.
    assert!(matches!(
        own.resolve(&StyleRef::Own(id("nope"))),
        Err(DetailError::UnknownStyle(_))
    ));
    // A pen or pattern the project does not have.
    let styles = LineStyles::default_metric();
    assert!(
        LineStyle::new(99, SOLID, Colour(0, 0, 0))
            .check(&styles)
            .is_err()
    );
    assert!(
        LineStyle::new(2, construct_styles::PatternId(77), Colour(0, 0, 0))
            .check(&styles)
            .is_err()
    );
}
