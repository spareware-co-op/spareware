//! The stored form: a type is an element record of kind `type`, an instance's record names its
//! type in `props`; one spelling, the same bytes for the same data, and anything writing would
//! not give is refused on reading.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{FINISH, HEIGHT, MARK, SILL, WIDTH, door_type, id, mm, parameters, text, types};
use construct_categories::CategoryTable;
use construct_model::element::{Element, Placement};
use construct_params::ParamError;
use construct_types::{ElementType, Instance, TYPE_KIND, TypeError, Types};
use std::collections::BTreeMap;

const TYPE_ROW: &str = concat!(
    r#"{"id":"T-820","document":"DOC-1","kind":"type","name":"Single 820","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"type","op_version":1,"args":{}},"#,
    r#""props":{"@category":"300","@params":"P-finish,P-height,P-mark,P-sill,P-width","#,
    r#""P-width":"length:820"},"references":[]}"#
);

fn record(types: &Types, type_id: &str) -> Element {
    types.get(&id(type_id)).unwrap().to_record(&id("DOC-1"))
}

/// A type's record, byte for byte.
#[test]
fn type_record_bytes_pinned() {
    let params = parameters();
    let types = types(&params);
    let row = record(&types, "T-820");
    assert_eq!(row.kind, TYPE_KIND);
    assert_eq!(serde_json::to_string(&row).unwrap(), TYPE_ROW);
}

/// An instance's entries in its record's `props`, byte for byte: its type and its own values.
#[test]
fn instance_props_pinned() {
    let params = parameters();
    let types = types(&params);
    let ty = types.get(&id("T-820")).unwrap();
    let mut door = Instance::new(ty);
    door.set(&params, ty, &id(SILL), mm(150)).unwrap();
    door.set(&params, ty, &id(MARK), text("D01")).unwrap();
    assert_eq!(
        serde_json::to_string(&door.to_props()).unwrap(),
        r#"{"@type":"T-820","P-mark":"text:D01","P-sill":"length:150"}"#
    );
}

/// The same data gives the same bytes, however it was put together.
#[test]
fn same_data_same_bytes() {
    let params = parameters();
    let mut one = door_type("T-a", "A", &[WIDTH, HEIGHT, SILL], &params);
    one.set(&params, &id(WIDTH), mm(700)).unwrap();
    one.set(&params, &id(HEIGHT), mm(2000)).unwrap();
    let mut two = door_type("T-a", "A", &[SILL, HEIGHT], &params);
    two.set(&params, &id(HEIGHT), mm(2000)).unwrap();
    two.add_parameter(&params, &id(WIDTH)).unwrap();
    two.add_parameter(&params, &id(SILL)).unwrap();
    two.set(&params, &id(WIDTH), mm(700)).unwrap();
    assert_eq!(one, two);
    let doc = id("DOC-1");
    assert_eq!(
        serde_json::to_vec(&one.to_record(&doc)).unwrap(),
        serde_json::to_vec(&two.to_record(&doc)).unwrap()
    );
}

/// Reading a record back gives the same type, and writing it again the same bytes; a whole
/// project's types go the same way.
#[test]
fn round_trip() {
    let params = parameters();
    let categories = CategoryTable::new();
    let types = types(&params);
    let row: Element = serde_json::from_str(TYPE_ROW).unwrap();
    let read = ElementType::from_record(&row, &params, &categories).unwrap();
    assert_eq!(&read, types.get(&id("T-820")).unwrap());
    assert_eq!(
        serde_json::to_string(&read.to_record(&id("DOC-1"))).unwrap(),
        TYPE_ROW
    );

    let rows = types.to_records(&id("DOC-1"));
    assert_eq!(rows.len(), 3);
    let again = Types::from_records(&rows, &params, &categories).unwrap();
    assert_eq!(again, types);

    let ty = types.get(&id("T-820")).unwrap();
    let mut door = Instance::new(ty);
    door.set(&params, ty, &id(FINISH), text("oiled")).unwrap();
    let mut props = door.to_props();
    props.insert("legacy".to_owned(), "kept".to_owned());
    assert_eq!(Instance::from_props(&props, &params, &types).unwrap(), door);
}

/// Two type records with names that differ only by case, in one category, are refused.
#[test]
fn duplicate_names_refused_on_reading() {
    let params = parameters();
    let types = types(&params);
    let mut rows = types.to_records(&id("DOC-1"));
    let mut copy = rows.first().cloned().unwrap();
    copy.id = id("T-copy");
    copy.name = "SINGLE 820".to_owned();
    rows.push(copy);
    assert!(matches!(
        Types::from_records(&rows, &params, &CategoryTable::new()),
        Err(TypeError::DuplicateName { .. })
    ));
}

fn read_changed(change: impl FnOnce(&mut Element)) -> Result<ElementType, TypeError> {
    let params = parameters();
    let mut row: Element = serde_json::from_str(TYPE_ROW).unwrap();
    change(&mut row);
    ElementType::from_record(&row, &params, &CategoryTable::new())
}

fn set_prop(row: &mut Element, key: &str, value: &str) {
    row.props.insert(key.to_owned(), value.to_owned());
}

/// Anything in a type record that writing would not give is refused, never repaired.
#[test]
fn type_record_refusals() {
    let malformed = |r: Result<ElementType, TypeError>| matches!(r, Err(TypeError::Malformed(_)));
    assert!(malformed(read_changed(
        |r| r.kind = "wall.straight".to_owned()
    )));
    assert!(malformed(read_changed(
        |r| r.name = " Single 820".to_owned()
    )));
    assert!(malformed(read_changed(|r| r.name = String::new())));
    assert!(malformed(read_changed(|r| r.level = 1)));
    assert!(malformed(read_changed(
        |r| r.placement = Placement { x: 1, y: 0, z: 0 }
    )));
    assert!(malformed(read_changed(
        |r| r.recipe.op = "door.single".to_owned()
    )));
    assert!(malformed(read_changed(|r| r.recipe.op_version = 2)));
    assert!(malformed(read_changed(|r| {
        r.recipe
            .args
            .insert("x".to_owned(), serde_json::Value::from(1));
    })));
    assert!(malformed(read_changed(|r| {
        r.props.remove("@category");
    })));
    assert!(malformed(read_changed(|r| {
        r.props.remove("@params");
    })));
    assert!(malformed(read_changed(|r| set_prop(
        r,
        "@category",
        "0300"
    ))));
    assert!(malformed(read_changed(|r| set_prop(
        r,
        "@category",
        "door"
    ))));
    assert!(malformed(read_changed(|r| set_prop(
        r,
        "@params",
        "P-width,P-height"
    ))));
    assert!(malformed(read_changed(|r| set_prop(
        r,
        "@params",
        "P-height,P-height,P-width"
    ))));
    assert!(malformed(read_changed(|r| set_prop(r, "@colour", "red"))));
    assert!(malformed(read_changed(|r| set_prop(r, "legacy", "text:x"))));
    assert_eq!(
        read_changed(|r| set_prop(r, "@category", "999999")).unwrap_err(),
        TypeError::UnknownCategory(construct_categories::CategoryId(999_999))
    );
    assert_eq!(
        read_changed(|r| set_prop(r, "@params", "P-nobody,P-width")).unwrap_err(),
        TypeError::UnknownParameter(id("P-nobody"))
    );
    assert_eq!(
        read_changed(|r| set_prop(r, SILL, "length:5")).unwrap_err(),
        TypeError::HeldByInstance {
            parameter: "Sill height".to_owned(),
        }
    );
    assert_eq!(
        read_changed(|r| set_prop(r, WIDTH, "length:0820")).unwrap_err(),
        TypeError::Param(ParamError::BadStored {
            parameter: "Width".to_owned(),
            text: "length:0820".to_owned(),
        })
    );
}

/// An instance's entries are refused when its type is missing or unknown, when it holds a value
/// its type holds, or when it holds a value for a parameter its type lacks. Other keys in the
/// record's `props` are not this crate's, and are left alone.
#[test]
fn instance_props_refusals() {
    let params = parameters();
    let types = types(&params);
    let read = |pairs: &[(&str, &str)]| {
        let props: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Instance::from_props(&props, &params, &types)
    };
    assert!(matches!(
        read(&[(MARK, "text:D1")]),
        Err(TypeError::Malformed(_))
    ));
    assert!(matches!(
        read(&[("@type", "not an id")]),
        Err(TypeError::Malformed(_))
    ));
    assert_eq!(
        read(&[("@type", "T-none")]).unwrap_err(),
        TypeError::UnknownType(id("T-none"))
    );
    assert_eq!(
        read(&[("@type", "T-820"), (WIDTH, "length:1000")]).unwrap_err(),
        TypeError::HeldByType {
            parameter: "Width".to_owned(),
        }
    );
    assert_eq!(
        read(&[("@type", "T-900"), (FINISH, "text:oiled")]).unwrap_err(),
        TypeError::NotOnType {
            type_name: "Single 900".to_owned(),
            parameter: "Finish".to_owned(),
        }
    );
    assert!(matches!(
        read(&[("@type", "T-900"), ("@colour", "red")]),
        Err(TypeError::Malformed(_))
    ));
    let door = read(&[("@type", "T-900"), ("colour", "red")]).unwrap();
    assert!(door.values().is_empty());
}
