//! One written form: whatever key order, spacing or number spelling a recipe is read in, it is
//! written in one canonical form, the same bytes every time, and its content id is the SHA-256 of
//! those bytes.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{DOOR, EXAMPLES, TABLE, edit, read};
use construct_families::{ContentId, Recipe};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Compact, keys in alphabetical order (`serde_json`'s own map order): read, then written in the
/// canonical layout and key order.
#[test]
fn any_layout_and_key_order_writes_the_canonical_bytes() {
    for (name, text) in EXAMPLES {
        let value: Value = serde_json::from_str(text).unwrap();
        let compact = serde_json::to_string(&value).unwrap();
        assert_ne!(compact, text);
        assert_eq!(read(&compact).to_json(), text, "{name}");
        let spaced = serde_json::to_string_pretty(&value)
            .unwrap()
            .replace(": ", " :  ");
        assert_eq!(read(&spaced).to_json(), text, "{name}");
    }
}

/// Integral numbers in any spelling are written as plain integers.
#[test]
fn numbers_are_written_plain() {
    let text = edit(DOOR, |v| {
        v["format"] = json!(1.0);
        v["category"] = json!(3e2);
        v["steps"][0]["subcategory"] = json!(302.0);
    });
    assert!(text.contains("1.0") && text.contains("300.0") && text.contains("302.0"));
    assert_eq!(read(&text).to_json(), DOOR);
}

/// Writing is the same every time, and writing what was written changes nothing.
#[test]
fn writing_is_stable() {
    for (_, text) in EXAMPLES {
        let recipe = read(text);
        let once = recipe.to_json();
        assert_eq!(once, recipe.to_json());
        assert_eq!(read(&once).to_json(), once);
        assert_eq!(read(&once), recipe);
    }
}

/// The canonical form ends in one newline and indents by two spaces.
#[test]
fn the_layout_is_two_spaces_and_one_final_newline() {
    let text = read(TABLE).to_json();
    assert!(text.starts_with("{\n  \"format\": 1,\n  \"id\": "));
    assert!(text.ends_with("}\n") && !text.ends_with("}\n\n"));
    assert!(!text.contains('\t') && !text.contains('\r'));
}

/// A symbol with no condition is written with no `when` member.
#[test]
fn an_absent_condition_is_not_written() {
    let text = read(TABLE).to_json();
    assert!(!text.contains("\"when\""));
    let window = read(common::WINDOW).to_json();
    assert!(!window.contains("\"when\""));
    assert!(read(DOOR).to_json().contains("\"when\": {"));
}

/// Text that needs escaping is written escaped the one way, and reads back.
#[test]
fn strings_are_escaped_one_way() {
    let text = edit(DOOR, |v| v["name"] = json!("Door \"A\"\\\n\t\u{1}\u{e9}"));
    let written = read(&text).to_json();
    assert!(written.contains(r#""name": "Door \"A\"\\\n\t\u0001é","#));
    assert_eq!(read(&written).to_json(), written);
}

/// The content id is the SHA-256 of the canonical bytes, in lowercase hexadecimal, and any
/// change to the recipe changes it.
#[test]
fn the_content_id_is_the_hash_of_the_canonical_bytes() {
    let door = read(DOOR);
    let hash = hex::encode(Sha256::digest(door.to_json().as_bytes()));
    assert_eq!(door.content_id().to_string(), format!("sha256:{hash}"));
    assert_eq!(
        ContentId::parse(&format!("sha256:{hash}")),
        Some(door.content_id())
    );
    assert_eq!(
        ContentId::parse(&format!("sha256:{}", hash.to_uppercase())),
        None
    );
    assert_eq!(ContentId::parse("sha256:00"), None);
    let changed = read(&edit(DOOR, |v| v["name"] = json!("Single swing door 2")));
    assert_ne!(changed.content_id(), door.content_id());
    let compact = serde_json::to_string(&serde_json::from_str::<Value>(DOOR).unwrap()).unwrap();
    assert_eq!(read(&compact).content_id(), door.content_id());
}

/// Text that is not JSON, or JSON that is not a recipe, is refused with the place named.
#[test]
fn what_does_not_read_is_refused_with_its_place() {
    let not_json = Recipe::from_json("{\"format\": 1,").unwrap_err();
    assert_eq!(not_json.at(), "");
    let refused =
        Recipe::from_json(&edit(DOOR, |v| v["steps"][1]["material"] = json!({}))).unwrap_err();
    assert_eq!(refused.at(), "/steps/1/material");
    let refused = Recipe::from_json(&edit(DOOR, |v| {
        v["sketches"][0]["points"][3]["key"] = json!("I1");
    }))
    .unwrap_err();
    assert_eq!(refused.at(), "/sketches/0/points/3/key");
    let refused = Recipe::from_json(&edit(DOOR, |v| v["types"][0]["oops"] = json!(1))).unwrap_err();
    assert_eq!(refused.at(), "/types/0/oops");
    let refused = Recipe::from_json(&edit(DOOR, |v| {
        v["parameters"][0].as_object_mut().unwrap().remove("key");
    }))
    .unwrap_err();
    assert_eq!(refused.at(), "/parameters/0/key");
}
