//! The completeness gate on the real tree, and on the violations it exists to refuse.
//!
//! What this file proves: `MAP.json` and the facade's registry pass `geom_test::completeness`, every
//! operation and kind is in the map, and each way of breaking the gate's rules is refused: a Stable
//! entry with a part missing, a `missing` list edited to hide one, an operation the map dropped, a
//! tier the map changed. A Preview entry may lack parts, as long as it says so.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::needless_pass_by_value,
    reason = "test code: a failure should stop loudly"
)]

use std::fs;
use std::path::{Path, PathBuf};

use geom_test::completeness::problems;
use serde_json::{Value, json};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .to_path_buf()
}

fn read(relative: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(root().join(relative)).unwrap()).unwrap()
}

fn map() -> Value {
    read("projects/engines/geometry/MAP.json")
}

fn tools() -> Value {
    read("projects/engines/crates/engine-geometry/generated/mcp-tools.json")
}

fn the_parts() -> Value {
    json!({
        "benchmark": ["b"], "contract": ["c"], "family_laws": ["l"],
        "fuzz": ["f"], "property": ["p"], "unit": ["u"]
    })
}

/// A map and registry of one operation.
fn one(tier: &str, evidence: Value, missing: Value) -> (Value, Value) {
    let map = json!({
        "operations": [{
            "name": "make.x", "version": 1, "tier": tier, "evidence": evidence, "missing": missing
        }],
        "kinds": []
    });
    let tools = json!({
        "tools": [{
            "name": "make.x",
            "_meta": { "org.spareware/kernel": { "tier": tier, "version": 1 } }
        }]
    });
    (map, tools)
}

#[test]
fn the_real_tree_passes_and_lists_everything() {
    let (map, tools) = (map(), tools());
    assert_eq!(problems(&map, &tools), Vec::<String>::new());
    assert_ne!(map["operations"].as_array().unwrap().len(), 0);
    // Fifteen curve kinds and eleven surface kinds (chapter 7).
    assert_eq!(map["kinds"].as_array().unwrap().len(), 26);
}

#[test]
fn every_real_entry_carries_its_contract() {
    for entry in map()["operations"].as_array().unwrap() {
        assert!(entry["evidence"].get("contract").is_some(), "{entry}");
    }
    for entry in map()["kinds"].as_array().unwrap() {
        assert!(entry["evidence"].get("contract").is_some(), "{entry}");
    }
}

#[test]
fn a_complete_operation_passes_in_either_tier() {
    for tier in ["Stable", "Preview since 2026-10-31"] {
        let (map, tools) = one(tier, the_parts(), json!([]));
        assert_eq!(problems(&map, &tools), Vec::<String>::new(), "{tier}");
    }
}

#[test]
fn a_preview_operation_may_lack_a_part_it_lists() {
    let mut evidence = the_parts();
    evidence.as_object_mut().unwrap().remove("fuzz");
    let (map, tools) = one("Preview since 2026-10-31", evidence, json!(["fuzz"]));
    assert_eq!(problems(&map, &tools), Vec::<String>::new());
}

#[test]
fn a_stable_operation_may_not_lack_a_part() {
    let mut evidence = the_parts();
    evidence.as_object_mut().unwrap().remove("fuzz");
    let (map, tools) = one("Stable", evidence, json!(["fuzz"]));
    let found = problems(&map, &tools);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("Stable"), "{found:?}");
}

#[test]
fn a_hidden_gap_is_refused() {
    let mut evidence = the_parts();
    evidence.as_object_mut().unwrap().remove("fuzz");
    let (map, tools) = one("Preview since 2026-10-31", evidence, json!([]));
    assert!(problems(&map, &tools)[0].contains("evidence lacks"));
}

#[test]
fn a_map_that_drops_or_changes_an_operation_is_refused() {
    let (mut map, tools) = one("Stable", the_parts(), json!([]));
    map["operations"] = json!([]);
    assert!(problems(&map, &tools).len() >= 2);
    let (mut map, tools) = one("Stable", the_parts(), json!([]));
    map["operations"][0]["tier"] = json!("Preview since 2026-10-31");
    assert!(problems(&map, &tools)[0].contains("tier and version"));
}

#[test]
fn a_tier_chapter_25_does_not_have_is_refused() {
    let (map, tools) = one("Beta", the_parts(), json!([]));
    assert!(problems(&map, &tools)[0].contains("tier"));
}

#[test]
fn a_kind_is_held_to_its_own_parts() {
    let kind = |tier: &str, evidence: Value, missing: Value| {
        json!({
            "operations": [],
            "kinds": [{"name": "CurveKind::Line", "tier": tier, "evidence": evidence, "missing": missing}]
        })
    };
    let no_tools = json!({ "tools": [] });
    let full = json!({ "contract": ["c"], "file_tag": ["t"], "intersection": ["i"] });
    assert_eq!(
        problems(&kind("Stable", full, json!([])), &no_tools),
        Vec::<String>::new()
    );
    let lacking = json!({ "contract": ["c"], "file_tag": ["t"] });
    let found = problems(&kind("Stable", lacking, json!(["intersection"])), &no_tools);
    assert_eq!(found.len(), 1, "{found:?}");
}
