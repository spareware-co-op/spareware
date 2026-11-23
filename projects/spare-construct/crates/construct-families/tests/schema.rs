//! The published schema and the reader agree: every example validates and reads, and for every
//! mutant of a recipe (each member taken out, an unknown member put in, each value swapped for
//! values of every other JSON type, for strings the format does and does not take, and for
//! numbers in and out of range) the schema accepts it exactly when the reader does.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{EXAMPLES, every_shape};
use construct_families::{Recipe, SCHEMA};
use jsonschema::Validator;
use serde_json::{Value, json};

fn validator() -> Validator {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    jsonschema::draft202012::new(&schema).unwrap()
}

/// The published file is the crate's `SCHEMA`, and it is a valid draft 2020-12 schema.
#[test]
fn the_schema_is_published_and_valid() {
    let file = include_str!("../schema/family-recipe-1.schema.json");
    assert_eq!(SCHEMA, file);
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    assert!(jsonschema::draft202012::meta::is_valid(&schema));
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
}

/// Every example validates against the schema.
#[test]
fn every_example_validates() {
    let validator = validator();
    for (name, text) in EXAMPLES {
        let value: Value = serde_json::from_str(text).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&value)
            .map(|e| format!("{} at {}", e, e.instance_path()))
            .collect();
        assert!(errors.is_empty(), "{name}: {errors:#?}");
        assert!(every_shape_reads(&value), "{name} does not read");
    }
    assert!(validator.is_valid(&every_shape()));
    assert!(every_shape_reads(&every_shape()));
}

fn every_shape_reads(value: &Value) -> bool {
    Recipe::from_value(value).is_ok()
}

/// Strings put in place of a value: empty, spaced, too long, a key of 64 and of 65 characters, an
/// id of 64 and of 65, a capital, a leading digit, a letter outside ASCII, and words of the
/// format's own (enum values, kinds, ops) in places they do not belong.
fn strings() -> Vec<Value> {
    let mut out: Vec<Value> = [
        "",
        " ",
        "x",
        "Not a key",
        "Key",
        "1key",
        "k\u{e9}y",
        "a-b",
        "a.b",
        "_a",
        "length:q0:5248000",
        "left",
        "Left",
        "outside",
        "plan",
        "elevation",
        "xy",
        "xz",
        "yz",
        "wall",
        "unhosted",
        "floor",
        "face",
        "extrude",
        "subtract",
        "unite",
        "distance",
        "fixed",
        "radius",
        "type",
        "instance",
        "length",
        "choice",
        "yesno",
    ]
    .iter()
    .map(|s| json!(s))
    .collect();
    out.push(json!(format!("k{}", "a".repeat(63))));
    out.push(json!(format!("k{}", "a".repeat(64))));
    out.push(json!(format!("F{}", "-".repeat(63))));
    out.push(json!(format!("F{}", "-".repeat(64))));
    out
}

/// Numbers put in place of a value: in and out of range, integral and not, as floats.
fn numbers() -> Vec<Value> {
    let floats: Vec<Value> = [
        1.0,
        2.0,
        1.5,
        -0.0,
        300.0,
        4_294_967_295.0,
        4_294_967_296.0,
        1e300,
    ]
    .iter()
    .map(|f| json!(f))
    .collect();
    let mut out = vec![
        json!(0),
        json!(1),
        json!(2),
        json!(-1),
        json!(300),
        json!(4_294_967_295_u64),
        json!(4_294_967_296_u64),
        json!(u64::MAX),
        json!(i64::MIN),
    ];
    out.extend(floats);
    out
}

/// Every other kind of value.
fn others() -> Vec<Value> {
    vec![
        Value::Null,
        json!(true),
        json!(false),
        json!([]),
        json!(["x"]),
        json!({}),
        json!({"x": 1}),
    ]
}

/// Every mutant of `value` with its one change at or under the member `under` of the document
/// (anywhere for `None`).
fn mutants(value: &Value, under: Option<&str>) -> Vec<Value> {
    let replacements: Vec<Value> = strings()
        .into_iter()
        .chain(numbers())
        .chain(others())
        .collect();
    let mut paths = Vec::new();
    walk(value, &mut Vec::new(), &mut paths);
    paths.retain(|path| match (under, path.first()) {
        (None, _) => true,
        (Some(name), Some(Step::Member(first))) => first == name,
        (Some(_), _) => false,
    });
    let mut out = Vec::new();
    for path in &paths {
        let here = at(value, path);
        for replacement in &replacements {
            if replacement != here {
                let mut copy = value.clone();
                *at_mut(&mut copy, path) = replacement.clone();
                out.push(copy);
            }
        }
        if let Value::Object(map) = here {
            for name in map.keys() {
                let mut copy = value.clone();
                if let Value::Object(map) = at_mut(&mut copy, path) {
                    map.remove(name);
                }
                out.push(copy);
            }
            for name in [
                "extra", "Key", "when", "default", "formula", "opening", "fixed",
            ] {
                if !map.contains_key(name) {
                    let mut copy = value.clone();
                    if let Value::Object(map) = at_mut(&mut copy, path) {
                        map.insert(name.to_owned(), json!("x"));
                    }
                    out.push(copy);
                }
            }
        }
        if let Value::Array(items) = here
            && let Some(first) = items.first()
        {
            let mut copy = value.clone();
            if let Value::Array(items) = at_mut(&mut copy, path) {
                items.push(first.clone());
                items.remove(0);
            }
            out.push(copy);
        }
    }
    out
}

#[derive(Clone)]
enum Step {
    Member(String),
    Item(usize),
}

fn walk(value: &Value, path: &mut Vec<Step>, out: &mut Vec<Vec<Step>>) {
    out.push(path.clone());
    match value {
        Value::Object(map) => {
            for (name, inner) in map {
                path.push(Step::Member(name.clone()));
                walk(inner, path, out);
                path.pop();
            }
        }
        Value::Array(items) => {
            for (n, inner) in items.iter().enumerate() {
                path.push(Step::Item(n));
                walk(inner, path, out);
                path.pop();
            }
        }
        _ => {}
    }
}

fn at<'a>(value: &'a Value, path: &[Step]) -> &'a Value {
    path.iter().fold(value, |v, step| match step {
        Step::Member(name) => &v[name.as_str()],
        Step::Item(n) => &v[*n],
    })
}

fn at_mut<'a>(value: &'a mut Value, path: &[Step]) -> &'a mut Value {
    path.iter().fold(value, |v, step| match step {
        Step::Member(name) => &mut v[name.as_str()],
        Step::Item(n) => &mut v[*n],
    })
}

/// The schema and the reader give the same verdict on every mutant.
fn agree_on_mutants_of(base: &Value, under: Option<&str>) -> (usize, usize) {
    let validator = validator();
    let (mut accepted, mut refused) = (0, 0);
    for mutant in mutants(base, under) {
        let by_schema = validator.is_valid(&mutant);
        let by_reader = Recipe::from_value(&mutant);
        assert_eq!(
            by_schema,
            by_reader.is_ok(),
            "schema says {by_schema}, reader says {by_reader:?}, for:\n{}",
            serde_json::to_string_pretty(&mutant).unwrap()
        );
        if by_schema {
            accepted += 1;
        } else {
            refused += 1;
        }
    }
    (accepted, refused)
}

#[test]
fn schema_and_reader_agree_on_every_mutant_of_every_shape() {
    let (accepted, refused) = agree_on_mutants_of(&every_shape(), None);
    assert!(accepted > 100, "only {accepted} mutants were accepted");
    assert!(refused > 1000, "only {refused} mutants were refused");
}

/// The three other hosting forms, each mutated in its hosting member.
#[test]
fn schema_and_reader_agree_on_every_hosting() {
    let validator = validator();
    for host in ["unhosted", "floor", "face"] {
        let mut base = every_shape();
        base["hosting"] = json!({ "host": host });
        assert!(validator.is_valid(&base), "{host}");
        assert!(Recipe::from_value(&base).is_ok(), "{host}");
        let (accepted, refused) = agree_on_mutants_of(&base, Some("hosting"));
        assert!(
            accepted > 0 && refused > 0,
            "{host}: {accepted} and {refused}"
        );
        let mut with_opening = base.clone();
        with_opening["hosting"]["opening"] = every_shape()["hosting"]["opening"].clone();
        assert!(!validator.is_valid(&with_opening), "{host}");
        assert!(Recipe::from_value(&with_opening).is_err(), "{host}");
    }
}

/// Cases written out: what both take and both refuse.
#[test]
fn schema_and_reader_agree_on_cases_written_out() {
    let validator = validator();
    let mut cases: Vec<(Value, bool)> = Vec::new();
    let mut case = |change: &dyn Fn(&mut Value), ok: bool| {
        let mut value = every_shape();
        change(&mut value);
        cases.push((value, ok));
    };
    case(&|_| {}, true);
    case(&|v| v["format"] = json!(1.0), true);
    case(&|v| v["format"] = json!(2), false);
    case(&|v| v["category"] = json!(2.6e3), true);
    case(&|v| v["category"] = json!(-1), false);
    case(&|v| v["parameters"][0]["formula"] = json!("1 mm"), false);
    case(
        &|v| {
            v["parameters"][0]
                .as_object_mut()
                .unwrap()
                .remove("default");
        },
        false,
    );
    case(&|v| v["parameters"][0]["key"] = json!("a".repeat(64)), true);
    case(
        &|v| v["parameters"][0]["key"] = json!("a".repeat(65)),
        false,
    );
    case(&|v| v["id"] = json!("F".repeat(64)), true);
    case(&|v| v["id"] = json!("F".repeat(65)), false);
    case(&|v| v["id"] = json!("-F"), false);
    case(&|v| v["name"] = json!(""), false);
    case(
        &|v| v["steps"][0]["material"] = json!({"fixed": "x", "parameter": "y"}),
        false,
    );
    case(&|v| v["steps"][0]["material"] = json!({}), false);
    case(&|v| v["symbols"][0]["when"] = Value::Null, false);
    case(
        &|v| v["sketches"][0]["constraints"][0]["kind"] = json!("vertical"),
        false,
    );
    case(
        &|v| v["sketches"][0]["constraints"][1]["kind"] = json!("vertical"),
        true,
    );
    case(&|v| v["steps"][1]["op"] = json!("unite"), false);
    // The type table is optional; its rows' value counts are the checker's, not the shape's.
    case(
        &|v| {
            v.as_object_mut().unwrap().remove("type_table");
        },
        true,
    );
    case(&|v| v["type_table"]["rows"][0]["values"] = json!([]), true);
    case(&|v| v["type_table"]["columns"] = json!([]), true);
    case(&|v| v["type_table"] = json!({}), false);
    case(
        &|v| v["type_table"] = json!({"columns": [], "rows": [], "x": 1}),
        false,
    );
    case(&|v| v["type_table"] = Value::Null, false);
    case(
        &|v| v["type_table"]["rows"][0]["values"][0] = json!(""),
        false,
    );
    for (value, ok) in cases {
        assert_eq!(validator.is_valid(&value), ok, "schema on {value}");
        assert_eq!(Recipe::from_value(&value).is_ok(), ok, "reader on {value}");
    }
}
