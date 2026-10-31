//! The completeness gate: chapter 26.1's checklist as a machine check (chapter 28.2, piece G103).
//!
//! What it does: [`problems`] reads the map's `operations` and `kinds` and the facade's registry,
//! and refuses a merge if the two disagree, or if an entry's tier lets it lack what it lacks. Its
//! test, `tests/completeness.rs`, runs on every pull request through `kernel-dev.yml`.
//!
//! Guarantees:
//!
//! - **The registry and the map agree.** Every operation the facade lists is in the map at its
//!   version and tier, and the map lists no other.
//! - **Stable means complete.** A Stable operation or kind with any part missing is a problem. A
//!   Preview one may lack parts, and each is listed in its `missing` (chapter 25).
//! - **`missing` is the truth.** It must be exactly the parts its `evidence` lacks, so the map
//!   cannot be edited to hide one.
//!
//! Method: chapter 28.2's table (`projects/engines/geometry/architecture/28-testing.md`) over the
//! map's fields, which `geom_test::map::registry` documents.
//!
//! Cost: linear in the entries.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::map::registry::{KIND_PARTS, OPERATION_PARTS};

/// The entries of `list` in `map`.
fn entries<'a>(map: &'a Value, list: &str) -> &'a [Value] {
    map.get(list)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// The text of `key` in `value`.
fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// The strings of the array `key` in `value`.
fn strings(value: &Value, key: &str) -> BTreeSet<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// What is wrong with one entry, against `parts`.
fn entry_problems(entry: &Value, parts: &[&str], found: &mut Vec<String>) {
    let name = text(entry, "name").unwrap_or("an unnamed entry");
    let missing = strings(entry, "missing");
    let evidence = entry.get("evidence");
    let lacked: BTreeSet<String> = parts
        .iter()
        .filter(|part| evidence.and_then(|e| e.get(**part)).is_none())
        .map(|part| (*part).to_owned())
        .collect();
    if missing != lacked {
        found.push(format!(
            "{name} lists {missing:?} as missing, and its evidence lacks {lacked:?}"
        ));
    }
    let tier = text(entry, "tier").unwrap_or("");
    if tier == "Stable" && !lacked.is_empty() {
        found.push(format!("{name} is Stable, and lacks {lacked:?}"));
    } else if tier != "Stable" && !tier.starts_with("Preview since ") {
        found.push(format!("{name} has the tier {tier:?}"));
    }
}

/// Everything the gate refuses in `map` (`MAP.json`) against `tools` (the facade's generated tool
/// descriptions), in a fixed order.
#[must_use]
pub fn problems(map: &Value, tools: &Value) -> Vec<String> {
    let mut found = Vec::new();
    let listed = entries(tools, "tools");
    let mapped = entries(map, "operations");
    if listed.len() != mapped.len() {
        found.push(format!(
            "the registry lists {} operations, and the map {}",
            listed.len(),
            mapped.len()
        ));
    }
    for tool in listed {
        let name = text(tool, "name").unwrap_or("an unnamed tool");
        let kernel = tool
            .get("_meta")
            .and_then(|m| m.get("org.spareware/kernel"));
        let same = mapped.iter().any(|e| {
            text(e, "name") == Some(name)
                && e.get("tier") == kernel.and_then(|k| k.get("tier"))
                && e.get("version") == kernel.and_then(|k| k.get("version"))
        });
        if !same {
            found.push(format!("{name} is not in the map at its tier and version"));
        }
    }
    for entry in mapped {
        entry_problems(entry, &OPERATION_PARTS, &mut found);
    }
    for entry in entries(map, "kinds") {
        entry_problems(entry, &KIND_PARTS, &mut found);
    }
    found
}
