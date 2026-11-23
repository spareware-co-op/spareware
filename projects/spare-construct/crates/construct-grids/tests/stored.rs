//! The stored forms, byte for byte, and what reading refuses.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{degrees, id, mm, pt};
use construct_grids::{
    Datum, Datums, Grid, GridError, PlaneShape, ReferencePlane, Shape, datum_of_recipe,
    is_datum_kind,
};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const GRID_LINE: &str = concat!(
    r#"{"id":"G-A","document":"DOC-1","kind":"grid","name":"A","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"grid.line","op_version":1,"#,
    r#""args":{"x0":"length:q0:0","x1":"length:q0:38400000","y0":"length:q0:0","y1":"length:q0:0"}},"#,
    r#""props":{"@category":"3700"},"references":[]}"#
);

const GRID_ARC: &str = concat!(
    r#"{"id":"G-B","document":"DOC-1","kind":"grid","name":"B","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"grid.arc","op_version":1,"#,
    r#""args":{"cx":"length:q0:0","cy":"length:q0:0","from":"angle:0","radius":"length:q0:6400000","to":"angle:810000000"}},"#,
    r#""props":{"@category":"3700"},"references":[]}"#
);

const PLANE_VERTICAL: &str = concat!(
    r#"{"id":"P-1","document":"DOC-1","kind":"refplane","name":"Mid","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"refplane.vertical","op_version":1,"#,
    r#""args":{"x0":"length:q0:19200000","x1":"length:q0:19200000","y0":"length:q0:0","y1":"length:q0:6400000"}},"#,
    r#""props":{"@category":"3800"},"references":[]}"#
);

const PLANE_HORIZONTAL: &str = concat!(
    r#"{"id":"P-2","document":"DOC-1","kind":"refplane","name":"","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"refplane.horizontal","op_version":1,"#,
    r#""args":{"z":"length:q0:-1920000"}},"#,
    r#""props":{"@category":"3800"},"references":[]}"#
);

fn document() -> kit_core::Id {
    id("DOC-1")
}

fn straight() -> Grid {
    Grid::new(id("G-A"), "A", Shape::line(pt(0, 0), pt(6000, 0)).unwrap()).unwrap()
}

fn curved() -> Grid {
    let arc = Shape::arc(pt(0, 0), mm(1000), degrees(0), degrees(90)).unwrap();
    Grid::new(id("G-B"), "B", arc).unwrap()
}

fn mid() -> ReferencePlane {
    let shape = PlaneShape::Vertical {
        from: pt(3000, 0),
        to: pt(3000, 1000),
    };
    ReferencePlane::new(id("P-1"), Some("Mid"), shape).unwrap()
}

fn level() -> ReferencePlane {
    let shape = PlaneShape::Horizontal {
        elevation: mm(-300),
    };
    ReferencePlane::new(id("P-2"), None, shape).unwrap()
}

fn text(record: &Element) -> String {
    serde_json::to_string(record).unwrap()
}

#[test]
fn a_straight_grids_record_is_pinned() {
    assert_eq!(text(&straight().to_record(&document())), GRID_LINE);
}

#[test]
fn an_arc_grids_record_is_pinned() {
    assert_eq!(text(&curved().to_record(&document())), GRID_ARC);
}

#[test]
fn a_vertical_planes_record_is_pinned() {
    assert_eq!(text(&mid().to_record(&document())), PLANE_VERTICAL);
}

#[test]
fn a_horizontal_planes_record_is_pinned_and_an_unnamed_planes_name_is_empty() {
    assert_eq!(text(&level().to_record(&document())), PLANE_HORIZONTAL);
}

#[test]
fn every_record_carries_its_category() {
    for record in [
        straight().to_record(&document()),
        curved().to_record(&document()),
    ] {
        assert_eq!(record.props["@category"], "3700");
        assert!(is_datum_kind(&record.kind));
    }
    for record in [mid().to_record(&document()), level().to_record(&document())] {
        assert_eq!(record.props["@category"], "3800");
        assert!(is_datum_kind(&record.kind));
    }
    assert!(!is_datum_kind("wall"));
}

#[test]
fn a_record_read_back_is_the_same_datum_and_writes_the_same_bytes() {
    for text in [GRID_LINE, GRID_ARC] {
        let record: Element = serde_json::from_str(text).unwrap();
        let grid = Grid::from_record(&record).unwrap();
        assert_eq!(grid.to_record(&document()), record);
    }
    for text in [PLANE_VERTICAL, PLANE_HORIZONTAL] {
        let record: Element = serde_json::from_str(text).unwrap();
        let plane = ReferencePlane::from_record(&record).unwrap();
        assert_eq!(plane.to_record(&document()), record);
    }
}

#[test]
fn the_geometry_is_exact_to_the_step() {
    // 12.5 mm is 80,000 steps: a whole-millimetre record could not hold it.
    let shape = Shape::line(pt(0, 0), common::at_steps(80_000, 160_000)).unwrap();
    let grid = Grid::new(id("G-X"), "X", shape).unwrap();
    let record = grid.to_record(&document());
    assert_eq!(record.recipe.args["x1"], json!("length:q0:80000"));
    assert_eq!(record.recipe.args["y1"], json!("length:q0:160000"));
    assert_eq!(Grid::from_record(&record).unwrap(), grid);
}

#[test]
fn a_whole_document_goes_out_and_comes_back() {
    let mut set = Datums::new();
    set.add_grid(straight()).unwrap();
    set.add_grid(curved()).unwrap();
    set.add_plane(mid()).unwrap();
    set.add_plane(level()).unwrap();
    let records = set.to_records(&document());
    assert_eq!(records.len(), 4);
    let wall = Element {
        kind: "wall".to_owned(),
        ..straight().to_record(&document())
    };
    let mut all = records.clone();
    all.push(wall);
    assert_eq!(
        Datums::from_records(&all).unwrap(),
        set,
        "other kinds are passed over"
    );
}

#[test]
fn other_props_are_kept_when_a_record_is_updated() {
    let mut record = straight().to_record(&document());
    record
        .props
        .insert("P-mark".to_owned(), "text:G1".to_owned());
    record.props.insert("@type".to_owned(), "T-1".to_owned());
    assert_eq!(
        Grid::from_record(&record).unwrap(),
        straight(),
        "other keys are not read"
    );
    let moved = straight()
        .reshaped(Shape::line(pt(0, 100), pt(6000, 100)).unwrap())
        .unwrap()
        .renamed("A1")
        .unwrap();
    let after = moved.update_record(&record);
    assert_eq!(after.props, record.props);
    assert_eq!(after.name, "A1");
    assert_eq!(Grid::from_record(&after).unwrap(), moved);
}

fn grid_record() -> Element {
    straight().to_record(&document())
}

fn refused(record: &Element) -> String {
    match Grid::from_record(record) {
        Err(GridError::Malformed(why)) => why,
        other => panic!("expected a malformed record, got {other:?}"),
    }
}

#[test]
fn reading_refuses_what_writing_would_not_give() {
    let mut other_kind = grid_record();
    other_kind.kind = "wall".to_owned();
    assert!(refused(&other_kind).contains("kind"));

    let mut on_a_level = grid_record();
    on_a_level.level = 1;
    assert!(refused(&on_a_level).contains("no level"));

    let mut placed = grid_record();
    placed.placement = Placement { x: 1, y: 0, z: 0 };
    assert!(refused(&placed).contains("no level"));

    let mut no_category = grid_record();
    no_category.props.remove("@category");
    assert!(refused(&no_category).contains("category"));

    let mut wrong_category = grid_record();
    wrong_category
        .props
        .insert("@category".to_owned(), "3800".to_owned());
    assert!(refused(&wrong_category).contains("category"));

    let mut extra = grid_record();
    extra
        .recipe
        .args
        .insert("z".to_owned(), json!("length:q0:0"));
    assert!(refused(&extra).contains("arguments"));

    let mut missing = grid_record();
    missing.recipe.args.remove("y1");
    assert!(refused(&missing).contains("arguments"));

    // The old whole-millimetre spelling reads as a length elsewhere, but is not what is written here.
    let mut old = grid_record();
    old.recipe
        .args
        .insert("x1".to_owned(), json!("length:6000"));
    assert!(refused(&old).contains("spelling"));

    let mut number = grid_record();
    number.recipe.args.insert("x1".to_owned(), json!(6000));
    assert!(refused(&number).contains("x1"));

    let mut wrong_kind_of_value = grid_record();
    wrong_kind_of_value
        .recipe
        .args
        .insert("x1".to_owned(), json!("angle:5"));
    assert!(refused(&wrong_kind_of_value).contains("not a length"));

    let mut version = grid_record();
    version.recipe.op_version = 2;
    assert!(refused(&version).contains("version 1"));

    let mut untrimmed = grid_record();
    untrimmed.name = " A".to_owned();
    assert!(refused(&untrimmed).contains("trimmed"));

    let mut not_a_grid = grid_record();
    not_a_grid.recipe = Recipe::construct("wall.straight", BTreeMap::new());
    assert!(refused(&not_a_grid).contains("not a grid"));
}

#[test]
fn reading_refuses_a_shape_the_rules_refuse() {
    let mut no_length = grid_record();
    no_length
        .recipe
        .args
        .insert("x1".to_owned(), json!("length:q0:0"));
    assert_eq!(Grid::from_record(&no_length), Err(GridError::ZeroLength));

    let mut no_radius = curved().to_record(&document());
    no_radius
        .recipe
        .args
        .insert("radius".to_owned(), json!("length:q0:0"));
    assert_eq!(Grid::from_record(&no_radius), Err(GridError::ZeroRadius));

    let mut no_sweep = curved().to_record(&document());
    no_sweep
        .recipe
        .args
        .insert("to".to_owned(), json!("angle:0"));
    assert_eq!(Grid::from_record(&no_sweep), Err(GridError::NoSweep));

    // An angle outside 0 to 360 degrees is not what is written, even if it means the same.
    let mut past_a_turn = curved().to_record(&document());
    past_a_turn
        .recipe
        .args
        .insert("to".to_owned(), json!("angle:3240000001"));
    let why = refused(&past_a_turn);
    assert!(why.contains("360"), "{why}");

    let mut far = grid_record();
    far.recipe
        .args
        .insert("x1".to_owned(), json!("length:q0:9999999999999"));
    assert_eq!(Grid::from_record(&far), Err(GridError::OutOfRange));

    let mut plane = mid().to_record(&document());
    plane.kind = "grid".to_owned();
    assert!(matches!(
        Grid::from_record(&plane),
        Err(GridError::Malformed(_))
    ));
    assert!(matches!(
        ReferencePlane::from_record(&grid_record()),
        Err(GridError::Malformed(_))
    ));
}

#[test]
fn a_recipe_of_another_operation_is_not_this_crates() {
    let wall = Recipe::construct("wall.straight", BTreeMap::new());
    assert!(datum_of_recipe(&wall).is_none());
    let mut other_owner = straight().to_record(&document()).recipe;
    other_owner.owner = "workshop".to_owned();
    assert!(datum_of_recipe(&other_owner).is_none());
    let read = datum_of_recipe(&straight().to_record(&document()).recipe)
        .unwrap()
        .unwrap();
    assert_eq!(read, Datum::Grid(*straight().shape()));
}

#[test]
fn a_recipe_is_the_same_bytes_every_time() {
    let one = serde_json::to_vec(&straight().to_record(&document())).unwrap();
    let two = serde_json::to_vec(&straight().to_record(&document())).unwrap();
    assert_eq!(one, two);
    let args: Value = serde_json::to_value(&straight().to_record(&document()).recipe.args).unwrap();
    assert_eq!(
        args.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["x0", "x1", "y0", "y1"],
        "sorted by key"
    );
}
