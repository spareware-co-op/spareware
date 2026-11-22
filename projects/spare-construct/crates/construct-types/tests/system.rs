//! A type's one optional system block: an owner tag and that owner's text, stored, returned and
//! compared byte for byte, never read inside. A type without one is written exactly as before.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{MARK, id, parameters, text, types};
use construct_categories::CategoryTable;
use construct_model::element::Element;
use construct_types::{
    ElementType, Instance, SystemData, TypeError, system_type_recipe, type_recipe,
};

const SYSTEM_ROW: &str = concat!(
    r#"{"id":"T-820","document":"DOC-1","kind":"type","name":"Single 820","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"type","op_version":2,"args":{}},"#,
    r#""props":{"@category":"300","@params":"P-finish,P-height,P-mark,P-sill,P-width","#,
    r#""@system.sample":"{\"a\":1}","P-width":"length:820"},"references":[]}"#
);

fn sample() -> SystemData {
    SystemData::new("sample", r#"{"a":1}"#).unwrap()
}

fn with_sample() -> ElementType {
    let params = parameters();
    types(&params)
        .get(&id("T-820"))
        .cloned()
        .unwrap()
        .with_system(sample())
}

/// A type with a block, byte for byte: version 2 of the type recipe and one `@system.` key.
#[test]
fn system_record_bytes_pinned() {
    let row = with_sample().to_record(&id("DOC-1"));
    assert_eq!(row.recipe, system_type_recipe());
    assert_eq!(serde_json::to_string(&row).unwrap(), SYSTEM_ROW);
}

/// Read back, the block is the same text; written again, the same bytes. Clearing it gives the
/// version 1 record of the type it came from.
#[test]
fn system_round_trip_and_clear() {
    let params = parameters();
    let row: Element = serde_json::from_str(SYSTEM_ROW).unwrap();
    let read = ElementType::from_record(&row, &params, &CategoryTable::new()).unwrap();
    assert_eq!(read, with_sample());
    assert_eq!(read.system().map(SystemData::owner), Some("sample"));
    assert_eq!(read.system().map(SystemData::data), Some(r#"{"a":1}"#));
    assert_eq!(
        serde_json::to_string(&read.to_record(&id("DOC-1"))).unwrap(),
        SYSTEM_ROW
    );
    let plain = read.without_system();
    assert!(plain.system().is_none());
    assert_eq!(plain.to_record(&id("DOC-1")).recipe, type_recipe());
    assert_eq!(&plain, types(&params).get(&id("T-820")).unwrap());
}

/// Blocks compare as text: a different spelling is a different block.
#[test]
fn system_compared_byte_for_byte() {
    let spaced = SystemData::new("sample", r#"{"a": 1}"#).unwrap();
    assert_ne!(spaced, sample());
    assert_ne!(with_sample(), with_sample().with_system(spaced));
}

/// A duplicate carries the block; moving an instance between types of one category is allowed
/// whatever their blocks.
#[test]
fn duplicate_carries_block_and_change_type_ignores_it() {
    let params = parameters();
    let mut types = types(&params);
    let first = types
        .get(&id("T-820"))
        .cloned()
        .unwrap()
        .with_system(sample());
    *types.get_mut(&id("T-820")).unwrap() = first;
    types
        .duplicate(&id("T-820"), id("T-820b"), "Single 820 copy")
        .unwrap();
    assert_eq!(types.get(&id("T-820b")).unwrap().system(), Some(&sample()));

    let first = types.get(&id("T-820")).unwrap();
    let second = types.get(&id("T-900")).unwrap();
    let mut door = Instance::new(first);
    door.set(&params, first, &id(MARK), text("D1")).unwrap();
    assert!(door.change_type(first, second).unwrap().is_empty());
    assert!(door.change_type(second, first).unwrap().is_empty());
}

/// A tag that is not 1 to 32 of a-z, 0-9 and `-` starting with a letter, or empty data, is
/// refused.
#[test]
fn bad_blocks_refused() {
    let long = "w".repeat(33);
    for owner in [
        "",
        "Wall",
        "1wall",
        "wall.layers",
        "wall layers",
        long.as_str(),
    ] {
        assert!(
            matches!(SystemData::new(owner, "x"), Err(TypeError::BadSystem(_))),
            "{owner:?}"
        );
    }
    assert!(matches!(
        SystemData::new("wall", ""),
        Err(TypeError::BadSystem(_))
    ));
    assert_eq!(SystemData::new("roof-2", "x").unwrap().owner(), "roof-2");
}

fn read_changed(change: impl FnOnce(&mut Element)) -> Result<ElementType, TypeError> {
    let mut row: Element = serde_json::from_str(SYSTEM_ROW).unwrap();
    change(&mut row);
    ElementType::from_record(&row, &parameters(), &CategoryTable::new())
}

/// Two blocks, a malformed block, and a block on the wrong recipe version are refused.
#[test]
fn system_record_refusals() {
    let malformed = |r: Result<ElementType, TypeError>| matches!(r, Err(TypeError::Malformed(_)));
    let add = |key: &'static str, value: &'static str| {
        move |r: &mut Element| {
            r.props.insert(key.to_owned(), value.to_owned());
        }
    };
    assert!(malformed(read_changed(add("@system.other", "x"))));
    assert!(malformed(read_changed(add("@system.", "x"))));
    assert!(malformed(read_changed(add("@system.sample", ""))));
    for key in ["@system.Sample", "@system."] {
        assert!(malformed(read_changed(|r| {
            r.props.remove("@system.sample");
            r.props.insert(key.to_owned(), "x".to_owned());
        })));
    }
    assert!(malformed(read_changed(|r| r.recipe.op_version = 1)));
    assert!(malformed(read_changed(|r| r.recipe.op_version = 3)));
}
