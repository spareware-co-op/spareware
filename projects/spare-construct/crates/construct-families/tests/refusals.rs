//! The checker refuses what the schema cannot see, naming the place in the document as a JSON
//! Pointer: formulas that do not work out or name an unknown parameter, references to points,
//! sketches and steps that do not exist or come later, loops of parameters, unknown materials,
//! categories and subcategories, a type setting what it does not hold, and two things with one
//! key. Each test changes one example and looks for its refusal.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{DOOR, TABLE, WINDOW, assert_has, edit, problems};
use construct_families::{Base, ProblemKind, Sort};
use serde_json::json;

fn missing(sort: Sort, key: &str) -> impl Fn(&ProblemKind) -> bool {
    let key = key.to_owned();
    move |kind| matches!(kind, ProblemKind::Missing { sort: s, key: k } if *s == sort && *k == key)
}

fn formula_code(code: &'static str) -> impl Fn(&ProblemKind) -> bool {
    move |kind| matches!(kind, ProblemKind::Formula(e) if e.code == code)
}

fn duplicate(key: &str) -> impl Fn(&ProblemKind) -> bool {
    let key = key.to_owned();
    move |kind| matches!(kind, ProblemKind::Duplicate { key: k } if *k == key)
}

// Formulas.

#[test]
fn a_formula_whose_units_do_not_agree_is_refused() {
    // A length times a length is an area, not a length.
    let text = edit(DOOR, |v| {
        v["parameters"][7]["formula"] = json!("width * frame");
    });
    assert_has(
        &problems(&text),
        "/parameters/7/formula",
        formula_code("Units"),
    );
    let text = edit(DOOR, |v| v["planes"][0]["offset"] = json!("2"));
    assert!(
        problems(&text).is_empty(),
        "a bare number is a length in millimetres"
    );
    let text = edit(DOOR, |v| v["planes"][0]["offset"] = json!("2 deg"));
    assert_has(&problems(&text), "/planes/0/offset", formula_code("Units"));
    let text = edit(DOOR, |v| {
        v["steps"][1]["start"] = json!("frame_depth / 2 - panel +");
    });
    assert_has(&problems(&text), "/steps/1/start", formula_code("Syntax"));
}

#[test]
fn a_formula_naming_an_unknown_parameter_is_refused() {
    let text = edit(DOOR, |v| {
        v["parameters"][7]["formula"] = json!("width + 2 * jamb");
    });
    assert_has(
        &problems(&text),
        "/parameters/7/formula",
        missing(Sort::Parameter, "jamb"),
    );
    let text = edit(DOOR, |v| {
        v["sketches"][0]["constraints"][6]["length"] = json!("breadth / 2");
    });
    assert_has(
        &problems(&text),
        "/sketches/0/constraints/6/length",
        missing(Sort::Parameter, "breadth"),
    );
}

#[test]
fn a_formula_may_read_only_numbers() {
    let text = edit(DOOR, |v| {
        v["parameters"][7]["formula"] = json!("width + panel_material");
    });
    assert_has(
        &problems(&text),
        "/parameters/7/formula",
        |kind| matches!(kind, ProblemKind::NotNumeric { key } if key == "panel_material"),
    );
    let text = edit(DOOR, |v| {
        v["parameters"][6]
            .as_object_mut()
            .unwrap()
            .remove("default");
        v["parameters"][6]["formula"] = json!("1");
    });
    assert_has(
        &problems(&text),
        "/parameters/6/formula",
        |kind| matches!(kind, ProblemKind::NotNumeric { key } if key == "swing"),
    );
}

#[test]
fn a_loop_of_parameters_is_refused() {
    let text = edit(DOOR, |v| {
        v["parameters"][1]
            .as_object_mut()
            .unwrap()
            .remove("default");
        v["parameters"][1]["formula"] = json!("opening_height - frame");
    });
    let found = problems(&text);
    assert_has(
        &found,
        "/parameters/1/formula",
        |kind| matches!(kind, ProblemKind::Cycle { keys } if keys == &["height", "opening_height"]),
    );
    let text = edit(TABLE, |v| {
        v["parameters"][7]["formula"] = json!("leg_height + 1 mm");
    });
    assert_has(
        &problems(&text),
        "/parameters/7/formula",
        |kind| matches!(kind, ProblemKind::Cycle { keys } if keys == &["leg_height"]),
    );
}

// Sketches.

#[test]
fn a_constraint_naming_a_missing_point_is_refused() {
    let text = edit(DOOR, |v| {
        v["sketches"][0]["constraints"][6]["a"] = json!("zz");
    });
    assert_has(
        &problems(&text),
        "/sketches/0/constraints/6/a",
        missing(Sort::Point, "zz"),
    );
    let text = edit(DOOR, |v| v["sketches"][0]["lines"][1]["end"] = json!("i9"));
    assert_has(
        &problems(&text),
        "/sketches/0/lines/1/end",
        missing(Sort::Point, "i9"),
    );
    let text = edit(DOOR, |v| {
        v["sketches"][1]["arcs"][0]["centre"] = json!("nowhere");
    });
    assert_has(
        &problems(&text),
        "/sketches/1/arcs/0/centre",
        missing(Sort::Point, "nowhere"),
    );
}

#[test]
fn a_constraint_naming_the_wrong_sort_of_item_is_refused() {
    // `base` is a line, where a point is wanted.
    let text = edit(DOOR, |v| {
        v["sketches"][0]["constraints"][6]["a"] = json!("base");
    });
    assert_has(&problems(&text), "/sketches/0/constraints/6/a", |kind| {
        matches!(
            kind,
            ProblemKind::WrongSort { key, wanted: Sort::Point, found: Sort::Line } if key == "base"
        )
    });
    let text = edit(DOOR, |v| {
        v["sketches"][0]["constraints"][10]["line"] = json!("i1");
    });
    assert_has(
        &problems(&text),
        "/sketches/0/constraints/10/line",
        |kind| {
            matches!(
                kind,
                ProblemKind::WrongSort {
                    wanted: Sort::Line,
                    found: Sort::Point,
                    ..
                }
            )
        },
    );
}

#[test]
fn a_sketch_on_a_missing_plane_is_refused() {
    let text = edit(DOOR, |v| v["sketches"][1]["plane"] = json!("ceiling"));
    assert_has(
        &problems(&text),
        "/sketches/1/plane",
        missing(Sort::Plane, "ceiling"),
    );
}

#[test]
fn a_point_not_at_a_length_is_refused() {
    let text = edit(DOOR, |v| {
        v["sketches"][0]["points"][0]["x"] = json!("angle:0");
    });
    assert_has(
        &problems(&text),
        "/sketches/0/points/0/x",
        |kind| matches!(kind, ProblemKind::NotALength { text } if text == "angle:0"),
    );
    let text = edit(DOOR, |v| {
        v["sketches"][0]["points"][0]["y"] = json!("length:q0:00");
    });
    assert_has(&problems(&text), "/sketches/0/points/0/y", |kind| {
        matches!(kind, ProblemKind::NotALength { .. })
    });
}

// Steps.

#[test]
fn a_step_using_a_missing_sketch_is_refused() {
    let text = edit(DOOR, |v| v["steps"][0]["sketch"] = json!("nothing"));
    assert_has(
        &problems(&text),
        "/steps/0/sketch",
        missing(Sort::Sketch, "nothing"),
    );
}

#[test]
fn a_step_using_a_step_that_is_missing_or_comes_later_is_refused() {
    let text = edit(WINDOW, |v| v["steps"][2]["target"] = json!("glass"));
    assert_has(
        &problems(&text),
        "/steps/2/target",
        |kind| matches!(kind, ProblemKind::NotEarlier { key } if key == "glass"),
    );
    let text = edit(WINDOW, |v| v["steps"][2]["tool"] = json!("frame"));
    assert_has(
        &problems(&text),
        "/steps/2/tool",
        |kind| matches!(kind, ProblemKind::NotEarlier { key } if key == "frame"),
    );
    let text = edit(WINDOW, |v| v["steps"][2]["tool"] = json!("sash"));
    assert_has(
        &problems(&text),
        "/steps/2/tool",
        missing(Sort::Step, "sash"),
    );
}

#[test]
fn a_profile_that_is_not_one_closed_loop_of_lines_is_refused() {
    let text = edit(DOOR, |v| {
        v["steps"][1]["profile"] = json!(["panel_foot", "inner_right", "inner_head"]);
    });
    assert_has(&problems(&text), "/steps/1/profile", |kind| {
        matches!(kind, ProblemKind::NotALoop)
    });
    let text = edit(DOOR, |v| {
        v["hosting"]["opening"]["profile"][0] = json!("arc_left");
    });
    assert_has(
        &problems(&text),
        "/hosting/opening/profile/0",
        missing(Sort::Line, "arc_left"),
    );
    let text = edit(DOOR, |v| v["steps"][1]["profile"][0] = json!("i1"));
    assert_has(&problems(&text), "/steps/1/profile/0", |kind| {
        matches!(
            kind,
            ProblemKind::WrongSort {
                wanted: Sort::Line,
                found: Sort::Point,
                ..
            }
        )
    });
}

// Ids against the project.

#[test]
fn an_unknown_material_is_refused() {
    let text = edit(DOOR, |v| {
        v["steps"][0]["material"]["fixed"] = json!("unobtainium");
    });
    assert_has(
        &problems(&text),
        "/steps/0/material/fixed",
        |kind| matches!(kind, ProblemKind::UnknownMaterial { id } if id.as_str() == "unobtainium"),
    );
    let text = edit(DOOR, |v| {
        v["parameters"][5]["default"] = json!("material:unobtainium");
    });
    assert_has(&problems(&text), "/parameters/5/default", |kind| {
        matches!(kind, ProblemKind::UnknownMaterial { .. })
    });
    let text = edit(DOOR, |v| {
        v["steps"][1]["material"]["parameter"] = json!("width");
    });
    assert_has(
        &problems(&text),
        "/steps/1/material/parameter",
        |kind| matches!(kind, ProblemKind::NotAMaterial { key } if key == "width"),
    );
}

#[test]
fn an_unknown_category_or_subcategory_is_refused() {
    let text = edit(DOOR, |v| v["category"] = json!(99_999));
    assert_has(
        &problems(&text),
        "/category",
        |kind| matches!(kind, ProblemKind::UnknownCategory { id } if id.0 == 99_999),
    );
    let text = edit(DOOR, |v| v["steps"][0]["subcategory"] = json!(399));
    assert_has(
        &problems(&text),
        "/steps/0/subcategory",
        |kind| matches!(kind, ProblemKind::UnknownCategory { id } if id.0 == 399),
    );
    // A window's frame is a subcategory, but not of doors.
    let text = edit(DOOR, |v| v["symbols"][0]["subcategory"] = json!(401));
    assert_has(
        &problems(&text),
        "/symbols/0/subcategory",
        |kind| matches!(kind, ProblemKind::NotOwnSubcategory { id } if id.0 == 401),
    );
}

// Types.

#[test]
fn a_type_setting_an_instance_parameter_is_refused() {
    let text = edit(DOOR, |v| {
        v["types"][0]["values"]
            .as_array_mut()
            .unwrap()
            .push(json!({"parameter": "swing", "value": "choice:right"}));
    });
    assert_has(
        &problems(&text),
        "/types/0/values/2/parameter",
        |kind| matches!(kind, ProblemKind::HeldByInstance { key } if key == "swing"),
    );
}

#[test]
fn a_type_setting_a_worked_out_parameter_or_a_bad_value_is_refused() {
    let text = edit(DOOR, |v| {
        v["types"][1]["values"][0] =
            json!({"parameter": "opening_width", "value": "length:q0:5760000"});
    });
    assert_has(
        &problems(&text),
        "/types/1/values/0/parameter",
        |kind| matches!(kind, ProblemKind::SetsFormula { key } if key == "opening_width"),
    );
    let text = edit(DOOR, |v| {
        v["types"][1]["values"][0]["value"] = json!("length:wide");
    });
    assert_has(&problems(&text), "/types/1/values/0/value", |kind| {
        matches!(kind, ProblemKind::BadValue(_))
    });
    let text = edit(DOOR, |v| {
        v["types"][1]["values"][0]["parameter"] = json!("depth");
    });
    assert_has(
        &problems(&text),
        "/types/1/values/0/parameter",
        missing(Sort::Parameter, "depth"),
    );
}

// Two things with one key.

#[test]
fn two_things_with_one_key_are_refused() {
    let text = edit(DOOR, |v| v["parameters"][1]["key"] = json!("width"));
    assert_has(&problems(&text), "/parameters/1/key", duplicate("width"));
    let text = edit(DOOR, |v| v["steps"][1]["key"] = json!("frame"));
    assert_has(&problems(&text), "/steps/1/key", duplicate("frame"));
    // Within a sketch, points, lines and curves share one set of keys.
    let text = edit(DOOR, |v| v["sketches"][1]["lines"][0]["key"] = json!("hl"));
    assert_has(&problems(&text), "/sketches/1/lines/0/key", duplicate("hl"));
    let text = edit(DOOR, |v| v["sketches"][1]["key"] = json!("elevation"));
    assert_has(&problems(&text), "/sketches/1/key", duplicate("elevation"));
    let text = edit(DOOR, |v| v["types"][2]["key"] = json!("d720"));
    assert_has(&problems(&text), "/types/2/key", duplicate("d720"));
    let text = edit(DOOR, |v| v["symbols"][3]["key"] = json!("mark_left"));
    assert_has(&problems(&text), "/symbols/3/key", duplicate("mark_left"));
    let text = edit(DOOR, |v| v["planes"][1]["key"] = json!("centre"));
    assert_has(&problems(&text), "/planes/1/key", duplicate("centre"));
}

#[test]
fn two_parameters_with_one_id_or_two_types_with_one_name_are_refused() {
    let text = edit(DOOR, |v| {
        v["parameters"][1]["definition"]["id"] = json!("01KAXF3M1Y0D00R5W1NG000001.width");
    });
    assert_has(&problems(&text), "/parameters/1/definition/id", |kind| {
        matches!(kind, ProblemKind::DuplicateId { .. })
    });
    let text = edit(DOOR, |v| v["types"][2]["name"] = json!(" 720 X 2040 "));
    assert_has(&problems(&text), "/types/2/name", |kind| {
        matches!(kind, ProblemKind::DuplicateName { .. })
    });
}

// Values, definitions, names.

#[test]
fn a_definition_or_default_construct_params_refuses_is_refused() {
    let text = edit(DOOR, |v| {
        v["parameters"][6]["definition"]["choices"][1]["key"] = json!("left");
    });
    assert_has(&problems(&text), "/parameters/6/definition", |kind| {
        matches!(kind, ProblemKind::BadDefinition(_))
    });
    let text = edit(DOOR, |v| v["parameters"][0]["default"] = json!("angle:900"));
    assert_has(&problems(&text), "/parameters/0/default", |kind| {
        matches!(kind, ProblemKind::BadValue(_))
    });
    let text = edit(DOOR, |v| {
        v["parameters"][6]["default"] = json!("choice:middle");
    });
    assert_has(&problems(&text), "/parameters/6/default", |kind| {
        matches!(kind, ProblemKind::BadValue(_))
    });
    let text = edit(DOOR, |v| v["name"] = json!("   "));
    assert_has(&problems(&text), "/name", |kind| {
        matches!(kind, ProblemKind::EmptyName)
    });
}

// Symbols and hosting.

#[test]
fn a_symbol_or_opening_on_the_wrong_plane_is_refused() {
    let text = edit(DOOR, |v| v["symbols"][0]["view"] = json!("elevation"));
    assert_has(&problems(&text), "/symbols/0/sketch", |kind| {
        matches!(kind, ProblemKind::WrongPlane { wanted: Base::Xz })
    });
    let text = edit(DOOR, |v| v["planes"][0]["base"] = json!("yz"));
    let found = problems(&text);
    assert_has(&found, "/hosting/opening/sketch", |kind| {
        matches!(kind, ProblemKind::WrongPlane { wanted: Base::Xz })
    });
}

#[test]
fn a_symbol_with_a_missing_edge_or_a_bad_condition_is_refused() {
    let text = edit(DOOR, |v| v["symbols"][0]["edges"][1] = json!("arc_middle"));
    assert_has(
        &problems(&text),
        "/symbols/0/edges/1",
        missing(Sort::Edge, "arc_middle"),
    );
    let text = edit(DOOR, |v| v["symbols"][0]["edges"][1] = json!("hl"));
    assert_has(&problems(&text), "/symbols/0/edges/1", |kind| {
        matches!(
            kind,
            ProblemKind::WrongSort {
                wanted: Sort::Edge,
                found: Sort::Point,
                ..
            }
        )
    });
    let text = edit(DOOR, |v| {
        v["symbols"][0]["when"]["parameter"] = json!("width");
    });
    assert_has(
        &problems(&text),
        "/symbols/0/when/parameter",
        |kind| matches!(kind, ProblemKind::NotACondition { key } if key == "width"),
    );
    let text = edit(DOOR, |v| v["symbols"][0]["when"]["is"] = json!("choice:up"));
    assert_has(&problems(&text), "/symbols/0/when/is", |kind| {
        matches!(kind, ProblemKind::BadValue(_))
    });
}

/// Every problem is reported at once, in document order.
#[test]
fn every_problem_is_reported_in_document_order() {
    let text = edit(DOOR, |v| {
        v["category"] = json!(99_999);
        v["steps"][0]["sketch"] = json!("nothing");
        v["types"][0]["values"][0]["parameter"] = json!("swing");
    });
    let found = problems(&text);
    let places: Vec<&str> = found.iter().map(construct_families::Problem::at).collect();
    assert_eq!(
        places,
        [
            "/category",
            "/steps/0/sketch",
            "/types/0/values/0/parameter"
        ],
        "{found:#?}"
    );
    for problem in &found {
        assert_ne!(problem.to_string(), "");
    }
}
