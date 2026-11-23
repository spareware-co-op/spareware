//! The stored form: a level is an element record of kind `level`; one spelling, the same bytes
//! for the same level, and anything writing would not give is refused on reading.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_levels::{CATEGORY_KEY, LEVEL_KIND, Level, LevelError, Levels};
use construct_model::element::{Element, Placement, Reference, Want};
use construct_params::Length;
use kit_core::Id;

const LEVEL_ROW: &str = concat!(
    r#"{"id":"L-1","document":"DOC-1","kind":"level","name":"First floor","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"level","op_version":1,"args":{}},"#,
    r#""props":{"@category":"3600","@elevation":"length:q0:20480000","@storey":"yesno:yes"},"#,
    r#""references":[]}"#
);

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn first() -> Level {
    Level::new(id("L-1"), "First floor", Length::from_mm(3200), true).unwrap()
}

/// A level's record, byte for byte.
#[test]
fn level_record_bytes_pinned() {
    let row = first().to_record(&id("DOC-1"));
    assert_eq!(row.kind, LEVEL_KIND);
    assert_eq!(serde_json::to_string(&row).unwrap(), LEVEL_ROW);
    let read: Element = serde_json::from_str(LEVEL_ROW).unwrap();
    assert_eq!(Level::from_record(&read).unwrap(), first());
}

/// The category key is the one every record carries (construct-types' own).
#[test]
fn the_category_key_is_the_records_own() {
    assert_eq!(CATEGORY_KEY, construct_types::CATEGORY_KEY);
}

fn read_changed(change: impl FnOnce(&mut Element)) -> Result<Level, LevelError> {
    let mut record = first().to_record(&id("DOC-1"));
    change(&mut record);
    Level::from_record(&record)
}

fn malformed(result: &Result<Level, LevelError>) -> bool {
    matches!(result, Err(LevelError::Malformed(_)))
}

#[test]
fn anything_a_level_is_not_written_as_is_refused() {
    assert!(malformed(&read_changed(|r| r.kind = "type".to_owned())));
    assert!(malformed(&read_changed(|r| r.level = 1)));
    assert!(malformed(&read_changed(
        |r| r.placement = Placement { x: 1, y: 0, z: 0 }
    )));
    assert!(malformed(&read_changed(|r| r.recipe.op_version = 2)));
    assert!(malformed(&read_changed(
        |r| r.name = " First floor".to_owned()
    )));
    assert!(malformed(&read_changed(|r| {
        r.references.push(Reference {
            doc: None,
            element: id("W-1"),
            name: "box.top".to_owned(),
            kind: "face".to_owned(),
            want: Want::Carrier,
            signature: None,
        });
    })));
    for (key, value) in [
        ("@category", "300"),
        ("@category", "03600"),
        // Version 1's whole millimetres are not how a level's elevation is written.
        ("@elevation", "length:3200"),
        ("@elevation", "area:q0:1"),
        ("@storey", "yes"),
        ("@storey", "yesno:maybe"),
        ("@other", "x"),
        ("note", "a plain key"),
    ] {
        let refused = read_changed(|r| {
            r.props.insert(key.to_owned(), value.to_owned());
        });
        assert!(malformed(&refused), "{key} = {value}");
    }
    for key in ["@category", "@elevation", "@storey"] {
        assert!(malformed(&read_changed(|r| {
            r.props.remove(key);
        })));
    }
}

/// A document with a level record that cannot be read is damaged, and says which.
#[test]
fn a_damaged_level_record_stops_the_reading_with_its_name() {
    let mut record = first().to_record(&id("DOC-1"));
    record.props.remove("@storey");
    let refused = Levels::from_records([&record]).unwrap_err();
    assert!(refused.to_string().contains("L-1"), "{refused}");
}
