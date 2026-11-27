//! Detail components: the library's own designs and their schema, the recipe's reader, a component
//! placed and turned by any angle, and a component repeated along a line, each with its numbers
//! worked by hand.

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
    BuiltIn, ComponentLine, ComponentRecipe, DetailComponent, DetailError, DetailRepeat, Library,
    MOST_REPEATS, SCHEMA, place,
};
use construct_grids::Point;
use construct_modify::Turn;
use construct_params::{Angle, Length};
use construct_styles::Colour;
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(x: i32, y: i32) -> Point {
    Point::new(Length::from_mm(x), Length::from_mm(y))
}

fn degrees(d: i64) -> Angle {
    Angle::from_units(d * Angle::UNITS_PER_DEGREE)
}

fn library() -> Library {
    Library::built_in().unwrap()
}

/// A recipe of one line from the origin 45 mm along x.
fn stick() -> ComponentRecipe {
    ComponentRecipe::new(
        "stick",
        "Stick",
        vec![ComponentLine {
            from: [Length::from_mm(0), Length::from_mm(0)],
            to: [Length::from_mm(45), Length::from_mm(0)],
            style: BuiltIn::Standard,
        }],
        vec![],
    )
    .unwrap()
}

// ---------------------------------------------------------------------------------------------
// The library and its schema

#[test]
fn the_library_holds_our_three_designs() {
    let library = library();
    let keys: Vec<&str> = library.all().iter().map(ComponentRecipe::key).collect();
    assert_eq!(
        keys,
        [
            "blocking-cross-90x45",
            "insulation-batt-90x600",
            "timber-section-90x45"
        ]
    );
    let timber = library.get("timber-section-90x45").unwrap();
    assert_eq!(timber.name(), "Timber section 90 x 45");
    assert_eq!(timber.lines().len(), 4);
    assert_eq!(timber.fills().len(), 1);
    // 22.5 mm is exact on the grid: 144,000 steps.
    assert_eq!(timber.lines()[0].from[1], Length::from_steps(-144_000));
    let cross = library.get("blocking-cross-90x45").unwrap();
    assert_eq!(cross.lines().len(), 6);
    assert!(cross.fills().is_empty());
}

#[test]
fn every_library_recipe_is_what_the_schema_takes_and_writes_back_the_same() {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for recipe in library().all() {
        let written = recipe.to_json().unwrap();
        let value: serde_json::Value = serde_json::from_str(&written).unwrap();
        assert!(validator.is_valid(&value), "{}", recipe.key());
        // The canonical form reads back to the same recipe, and writes the same bytes again.
        let read = ComponentRecipe::from_json(&written).unwrap();
        assert_eq!(&read, recipe);
        assert_eq!(read.to_json().unwrap(), written);
    }
}

#[test]
fn the_schema_refuses_what_the_reader_refuses() {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let good = library()
        .get("timber-section-90x45")
        .unwrap()
        .to_json()
        .unwrap();
    for (what, bad) in [
        (
            "a field it does not know",
            good.replacen("\"name\"", "\"colour2\": 1, \"name\"", 1),
        ),
        (
            "another version",
            good.replacen("\"schema\": 1", "\"schema\": 2", 1),
        ),
        (
            "a style that is not built in",
            good.replacen("standard", "chunky", 1),
        ),
        (
            "a colour in capitals",
            good.replacen("#000000", "#00000A", 1),
        ),
        (
            "a key with a capital",
            good.replacen("timber-section", "Timber", 1),
        ),
    ] {
        let value: serde_json::Value = serde_json::from_str(&bad).unwrap();
        assert!(!validator.is_valid(&value), "schema: {what}");
        assert!(ComponentRecipe::from_json(&bad).is_err(), "reader: {what}");
    }
}

#[test]
fn the_reader_checks_what_the_schema_cannot_see() {
    let good = library()
        .get("timber-section-90x45")
        .unwrap()
        .to_json()
        .unwrap();
    // A length with a unit that is not a length.
    let bad = good.replacen("\"45 mm\"", "\"45 kg\"", 1);
    assert!(matches!(
        ComponentRecipe::from_json(&bad),
        Err(DetailError::BadRecipe(why)) if why.contains("/lines/0")
    ));
    // A fill that crosses itself (a bow tie) is refused with its place.
    let mut value: serde_json::Value = serde_json::from_str(&good).unwrap();
    let outline = value["fills"][0]["outline"].as_array_mut().unwrap();
    outline.swap(1, 2);
    let bow = value.to_string();
    assert!(matches!(
        ComponentRecipe::from_json(&bow),
        Err(DetailError::BadRecipe(why)) if why.contains("Fill 1")
    ));
    // Not JSON at all.
    assert!(ComponentRecipe::from_json("timber").is_err());
}

#[test]
fn a_recipe_is_checked_when_made() {
    let line = |to: i32| ComponentLine {
        from: [Length::from_mm(0), Length::from_mm(0)],
        to: [Length::from_mm(to), Length::from_mm(0)],
        style: BuiltIn::Fine,
    };
    assert!(ComponentRecipe::new("a", "A", vec![line(0)], vec![]).is_err());
    assert!(ComponentRecipe::new("a", "A", vec![], vec![]).is_err());
    assert!(ComponentRecipe::new("A b", "A", vec![line(1)], vec![]).is_err());
    assert!(ComponentRecipe::new("a", " A", vec![line(1)], vec![]).is_err());
    // Nothing reaches further than 10 m from the origin.
    assert!(ComponentRecipe::new("a", "A", vec![line(10_001)], vec![]).is_err());
    assert!(ComponentRecipe::new("a", "A", vec![line(10_000)], vec![]).is_ok());
    // Two recipes of one key do not make a library.
    assert!(Library::new(vec![stick(), stick()]).is_err());
}

// ---------------------------------------------------------------------------------------------
// Placing, and turning by any angle

#[test]
fn an_unturned_component_is_its_recipe_at_its_position() {
    let library = library();
    let timber = library.get("timber-section-90x45").unwrap();
    let placed = place(timber, mm(1000, 2000), &Turn::none()).unwrap();
    // The first line's start: (-45 mm, -22.5 mm) from (1000 mm, 2000 mm).
    assert_eq!(placed.lines[0].from, [955 * 6400, 1_977_500 * 6400 / 1000]);
    assert_eq!(placed.lines.len(), 4);
    assert_eq!(placed.fills.len(), 1);
    assert_eq!(placed.fills[0].pattern, "timber");
    assert_eq!(placed.fills[0].colour, Colour(0, 0, 0));
}

#[test]
fn a_quarter_turn_is_exact() {
    // The corner (45 mm, -22.5 mm) turned 90 degrees about the origin is (22.5 mm, 45 mm).
    let library = library();
    let timber = library.get("timber-section-90x45").unwrap();
    let placed = place(timber, mm(0, 0), &Turn::of_angle(degrees(90))).unwrap();
    assert_eq!(placed.lines[1].from, [144_000, 288_000]);
    // Four quarter turns are no turn.
    let four = place(timber, mm(0, 0), &Turn::of_angle(degrees(360))).unwrap();
    assert_eq!(four, place(timber, mm(0, 0), &Turn::none()).unwrap());
}

#[test]
fn a_thirty_degree_turn_is_the_nearest_step_to_the_true_place() {
    // A line from the origin 45 mm along x, placed at (1000 mm, 2000 mm) and turned 30 degrees:
    // its end is 45 mm x (cos 30, sin 30) = (38.97114317 mm, 22.5 mm) from the position. In steps
    // (45 mm = 288,000): x is 288,000 x 0.8660254037844386 = 249,415.316..., so 249,415; y is
    // 144,000 exactly.
    let placed = place(&stick(), mm(1000, 2000), &Turn::of_angle(degrees(30))).unwrap();
    let line = placed.lines[0];
    assert_eq!(line.from, [6_400_000, 12_800_000]);
    assert_eq!(line.to, [6_400_000 + 249_415, 12_800_000 + 144_000]);
}

#[test]
fn a_component_record_holds_a_position_and_any_angle_and_nothing_else() {
    let library = library();
    // 37.5 degrees, and a turn of a full circle and a half more reads as the same place.
    let angle = Angle::from_units(37 * Angle::UNITS_PER_DEGREE + Angle::UNITS_PER_DEGREE / 2);
    let one = DetailComponent::new(
        id("c1"),
        id("v1"),
        &library,
        "timber-section-90x45",
        mm(500, 500),
        angle,
    )
    .unwrap();
    let record = one.to_record(&id("doc"));
    assert_eq!(record.kind, "detail.component");
    assert_eq!(
        record.props.get("@detail.component.rotation").unwrap(),
        "angle:337500000"
    );
    assert_eq!(
        DetailComponent::from_record(&record, &library).unwrap(),
        one
    );
    let spun = DetailComponent::new(
        id("c1"),
        id("v1"),
        &library,
        "timber-section-90x45",
        mm(500, 500),
        Angle::from_units(angle.units() + 4 * 810_000_000),
    )
    .unwrap();
    assert_eq!(spun, one);
    assert_eq!(spun.rotation(), angle);
    // A recipe the library lacks.
    assert_eq!(
        DetailComponent::new(id("c2"), id("v1"), &library, "sofa", mm(0, 0), degrees(0)),
        Err(DetailError::UnknownComponent("sofa".to_owned()))
    );
    // Moved and turned: a new placement of the same item.
    let moved = one.with_placement(&library, mm(0, 0), degrees(90)).unwrap();
    assert_eq!(moved.position(), mm(0, 0));
    assert_eq!(moved.id(), &id("c1"));
}

// ---------------------------------------------------------------------------------------------
// Repeating along a line

#[test]
fn a_repeat_along_a_three_metre_line_at_600_makes_six() {
    // The first at the start, then every 600 mm: at 0, 600, 1200, 1800, 2400 and 3000 mm.
    let library = library();
    let repeat = DetailRepeat::new(
        id("r1"),
        id("v1"),
        &library,
        "blocking-cross-90x45",
        (mm(0, 0), mm(3000, 0)),
        Length::from_mm(600),
        degrees(0),
    )
    .unwrap();
    assert_eq!(repeat.count().unwrap(), 6);
    let at: Vec<Point> = (0..6).map(|k| mm(600 * k, 0)).collect();
    assert_eq!(repeat.positions().unwrap(), at);
    // Each copy is the cross, along the line: six crosses of six lines.
    let drawn = repeat.draw(&library).unwrap();
    assert_eq!(drawn.len(), 6);
    assert!(drawn.iter().all(|c| c.lines.len() == 6));
    // The last cross's first line starts at (3000 - 45) mm, -22.5 mm.
    assert_eq!(drawn[5].lines[0].from, [2_955 * 6400, -144_000]);
}

#[test]
fn a_line_a_little_short_of_the_last_copy_makes_one_fewer() {
    let library = library();
    let count = |to: i32| {
        DetailRepeat::new(
            id("r1"),
            id("v1"),
            &library,
            "blocking-cross-90x45",
            (mm(0, 0), mm(to, 0)),
            Length::from_mm(600),
            degrees(0),
        )
        .unwrap()
        .count()
        .unwrap()
    };
    assert_eq!(count(2999), 5);
    assert_eq!(count(3000), 6);
    assert_eq!(count(3599), 6);
    assert_eq!(count(3600), 7);
    assert_eq!(count(10), 1);
}

#[test]
fn a_slanted_line_places_copies_at_exact_distances_along_it() {
    // From (0, 0) to (3000, 4000) mm: 5000 mm long. Every 1000 mm along it is every (600, 800).
    let library = library();
    let repeat = DetailRepeat::new(
        id("r1"),
        id("v1"),
        &library,
        "blocking-cross-90x45",
        (mm(0, 0), mm(3000, 4000)),
        Length::from_mm(1000),
        degrees(0),
    )
    .unwrap();
    assert_eq!(repeat.count().unwrap(), 6);
    let want: Vec<Point> = (0..6).map(|k| mm(600 * k, 800 * k)).collect();
    assert_eq!(repeat.positions().unwrap(), want);
    // A line that is no Pythagorean triple: 1000 mm along (1000, 1000) is 707.1068 mm on each axis.
    // 1000 mm is 6,400,000 steps; each part is 6,400,000 / sqrt 2 = 4,525,483.399..., so 4,525,483.
    let slanted = DetailRepeat::new(
        id("r2"),
        id("v1"),
        &library,
        "blocking-cross-90x45",
        (mm(0, 0), mm(3000, 3000)),
        Length::from_mm(1000),
        degrees(0),
    )
    .unwrap();
    let p = slanted.positions().unwrap();
    assert_eq!(p.len(), 5);
    assert_eq!(p[1].x.steps(), 4_525_483);
    assert_eq!(p[1].y.steps(), 4_525_483);
    // The far end is not hit exactly: four copies of 1000 mm fit in 4242.64 mm.
    assert_eq!(p[4].x.steps(), 18_101_934);
}

#[test]
fn a_repeat_turns_each_copy_to_lie_along_its_line_and_then_by_its_own_angle() {
    let library = library();
    let along_y = DetailRepeat::new(
        id("r1"),
        id("v1"),
        &library,
        "blocking-cross-90x45",
        (mm(0, 0), mm(0, 1000)),
        Length::from_mm(1000),
        degrees(0),
    )
    .unwrap();
    // A line up the y axis turns the cross a quarter: its corner (45, -22.5) mm becomes
    // (22.5, 45) mm from the copy's place.
    let first = along_y.draw(&library).unwrap();
    assert_eq!(first[0].lines[1].from, [144_000, 288_000]);
    // And an extra quarter turn on top puts it back across the line: (-45, 22.5) mm.
    let more = along_y
        .clone()
        .with_spacing(&library, Length::from_mm(1000), degrees(90))
        .unwrap();
    assert_eq!(
        more.draw(&library).unwrap()[0].lines[1].from,
        [-288_000, 144_000]
    );
}

#[test]
fn a_repeat_is_refused_by_name_when_it_cannot_be_made() {
    let library = library();
    let make = |line: (Point, Point), spacing: i64| {
        DetailRepeat::new(
            id("r1"),
            id("v1"),
            &library,
            "blocking-cross-90x45",
            line,
            Length::from_steps(spacing),
            degrees(0),
        )
    };
    assert_eq!(
        make((mm(5, 5), mm(5, 5)), 6400),
        Err(DetailError::ZeroLength)
    );
    assert!(matches!(
        make((mm(0, 0), mm(1000, 0)), 0),
        Err(DetailError::BadSpacing(_))
    ));
    assert!(matches!(
        make((mm(0, 0), mm(1000, 0)), -5),
        Err(DetailError::BadSpacing(_))
    ));
    // A 100 m line every millimetre would make 100,001 copies.
    assert_eq!(
        make((mm(0, 0), mm(100_000, 0)), 6400),
        Err(DetailError::TooMany(100_001))
    );
    assert!(MOST_REPEATS < 100_001);
    assert!(matches!(
        DetailRepeat::new(
            id("r1"),
            id("v1"),
            &library,
            "sofa",
            (mm(0, 0), mm(1, 0)),
            Length::from_mm(1),
            degrees(0)
        ),
        Err(DetailError::UnknownComponent(_))
    ));
}

#[test]
fn a_repeat_record_is_read_back() {
    let library = library();
    let repeat = DetailRepeat::new(
        id("r1"),
        id("v1"),
        &library,
        "insulation-batt-90x600",
        (mm(0, 0), mm(3000, 0)),
        Length::from_mm(600),
        degrees(0),
    )
    .unwrap();
    let record = repeat.to_record(&id("doc"));
    assert_eq!(record.kind, "detail.repeat");
    assert_eq!(
        DetailRepeat::from_record(&record, &library).unwrap(),
        repeat
    );
    let moved = repeat.with_line(&library, (mm(0, 0), mm(0, 3000))).unwrap();
    assert_eq!(moved.line(), (mm(0, 0), mm(0, 3000)));
    assert_eq!(
        DetailRepeat::from_record(&moved.to_record(&id("doc")), &library).unwrap(),
        moved
    );
}
