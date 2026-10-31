//! The map's operations and geometry kinds: each with its tier, what it has of chapter 26.1's
//! parts, and what it still lacks (piece G103).
//!
//! What it does: [`values`] reads every operation's name, family, tier and version from the facade's
//! generated tool descriptions, and every kind from `CurveKind` and `SurfaceKind` in the source. A
//! test, fuzz or benchmark file claims what it covers with a `Covers:` paragraph in its header and,
//! for a family's laws, a `Family laws:` paragraph; [`values`] gathers the claims into each entry's
//! `evidence`, and writes `missing` as the parts no file claims. `crate::completeness` then holds
//! the entries to their tiers.
//!
//! Guarantees:
//!
//! - **A claim names something real.** A `Covers:` item that is no operation or kind is a
//!   [`Rule::Covers`] problem, so a renamed operation cannot keep a stale claim.
//! - **Where a file sits says what it proves.** A file in `geom-fuzz/src` is a fuzz target, in
//!   `geom-bench/src` a benchmark, `tests/contract_laws.rs` the contract; any other test is a unit
//!   test, and a property test too if it holds `proptest!`. For a kind, a test in `geom-format` is
//!   the file tag's round trip, one in `geom-intersect` the kind's row of the intersection matrix,
//!   and any other its contract.
//!
//! Method: chapter 28.2's table of what the gate requires
//! (`projects/engines/geometry/architecture/28-testing.md`). The generated tool descriptions are
//! read because they are `OPERATIONS` itself, kept current by `geom-gen check`, and the map reads
//! files rather than linking the facade.
//!
//! Cost: linear in the files read.

use std::collections::{BTreeMap, BTreeSet};

use super::json::Value;
use super::{Collected, CrateInput, Registry, Rule};

/// What chapter 28.2 requires of an operation. `contract` is its test for each error kind, the
/// history laws and its passing example, which `tests/contract_laws.rs` runs together.
pub const OPERATION_PARTS: [&str; 6] = [
    "benchmark",
    "contract",
    "family_laws",
    "fuzz",
    "property",
    "unit",
];

/// What chapter 28.2 requires of a curve or surface kind. `contract` is its conversion to NURBS
/// within its stated error and its transforms, through the kind contract kits.
pub const KIND_PARTS: [&str; 3] = ["contract", "file_tag", "intersection"];

/// The enums whose variants are the kinds, with the crate that holds each.
const KINDS: [(&str, &str); 2] = [("geom-curve", "CurveKind"), ("geom-surface", "SurfaceKind")];

/// A paragraph of a file's leading `//!` header, from the line `//! {field}: ` to its end.
fn paragraph(text: &str, field: &str) -> Option<String> {
    let start = format!("{field}: ");
    let mut found: Option<String> = None;
    for line in text.lines().take_while(|l| l.starts_with("//!")) {
        let body = line.trim_start_matches("//!").trim();
        match &mut found {
            None => found = body.strip_prefix(start.as_str()).map(str::to_owned),
            Some(_) if body.is_empty() => break,
            Some(so_far) => {
                so_far.push(' ');
                so_far.push_str(body);
            }
        }
    }
    found
}

/// The items of a list paragraph: `` `make.box`, make.cone. `` gives the two names.
fn items(list: &str) -> Vec<String> {
    list.split(',')
        .map(|i| i.trim().trim_end_matches('.').trim_matches('`').to_owned())
        .filter(|i| !i.is_empty())
        .collect()
}

/// The variants of `pub enum {name}` in `text`.
fn variants(text: &str, name: &str) -> Vec<String> {
    let start = format!("pub enum {name}");
    let Some(at) = text.find(&start) else {
        return Vec::new();
    };
    text.get(at..)
        .unwrap_or_default()
        .lines()
        .skip(1)
        .take_while(|l| !l.starts_with('}'))
        .filter_map(|l| l.strip_prefix("    "))
        .filter(|l| l.chars().next().is_some_and(char::is_uppercase))
        .map(|l| l.chars().take_while(char::is_ascii_alphanumeric).collect())
        .collect()
}

/// The kinds, each as `Enum::Variant`, with their enum's tier.
fn kinds(crates: &[&CrateInput], collected: &mut Collected) -> BTreeMap<String, String> {
    let mut all = BTreeMap::new();
    for (crate_name, kind) in KINDS {
        let files = crates
            .iter()
            .filter(|k| k.name == crate_name)
            .flat_map(|k| &k.files);
        for file in files {
            let Ok(text) = &file.text else { continue };
            if !text.contains(&format!("pub enum {kind}")) {
                continue;
            }
            let tier = paragraph(text, "Tier").map(|t| t.trim_end_matches('.').to_owned());
            let Some(tier) = tier else {
                collected.one(
                    &file.path,
                    Rule::Covers,
                    format!("the header of {kind}'s module states no `Tier:`"),
                );
                continue;
            };
            for variant in variants(text, kind) {
                all.insert(format!("{kind}::{variant}"), tier.clone());
            }
        }
    }
    all
}

/// The parts a file claims for an operation.
fn operation_parts(path: &str, text: &str) -> Vec<&'static str> {
    if path.contains("/geom-fuzz/src/") {
        vec!["fuzz"]
    } else if path.contains("/geom-bench/src/") {
        vec!["benchmark"]
    } else if path.ends_with("/tests/contract_laws.rs") {
        vec!["contract"]
    } else if text.contains("proptest!") {
        vec!["property", "unit"]
    } else {
        vec!["unit"]
    }
}

/// The part a file claims for a kind.
fn kind_part(path: &str) -> &'static str {
    if path.contains("/geom-format/") {
        "file_tag"
    } else if path.contains("/geom-intersect/") {
        "intersection"
    } else {
        "contract"
    }
}

/// Evidence for one subject: part to the files that claim it.
type Evidence = BTreeMap<&'static str, BTreeSet<String>>;

/// One entry of `operations` or `kinds`: its fields, `evidence` and `missing`.
fn entry(pairs: Vec<(&str, Value)>, evidence: &Evidence, parts: &[&str]) -> Value {
    let mut fields: BTreeMap<String, Value> =
        pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
    let claimed: BTreeMap<String, Value> = evidence
        .iter()
        .map(|(part, files)| {
            let files = files.iter().map(|f| Value::from(f.as_str())).collect();
            ((*part).to_owned(), Value::Array(files))
        })
        .collect();
    let missing = parts
        .iter()
        .filter(|p| !evidence.contains_key(**p))
        .map(|p| Value::from(*p))
        .collect();
    fields.insert("evidence".to_owned(), Value::Object(claimed));
    fields.insert("missing".to_owned(), Value::Array(missing));
    Value::Object(fields)
}

/// The operations in the facade's tool descriptions: name, family, tier and version.
fn operations(
    tools: &str,
    file: &str,
    collected: &mut Collected,
) -> Vec<(String, String, String, u64)> {
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(tools);
    let Ok(parsed) = parsed else {
        collected.one(
            file,
            Rule::Covers,
            "the tool descriptions are not JSON".to_owned(),
        );
        return Vec::new();
    };
    let listed = parsed
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    listed
        .iter()
        .filter_map(|tool| {
            let kernel = tool.get("_meta")?.get("org.spareware/kernel")?;
            Some((
                tool.get("name")?.as_str()?.to_owned(),
                kernel.get("family")?.as_str()?.to_owned(),
                kernel.get("tier")?.as_str()?.to_owned(),
                kernel.get("version")?.as_u64()?,
            ))
        })
        .collect()
}

/// The map's `operations` and `kinds`, in name order, and a [`Rule::Covers`] problem for every claim
/// that names neither.
pub(super) fn values(
    registry: &Registry,
    crates: &[&CrateInput],
    collected: &mut Collected,
) -> (Vec<Value>, Vec<Value>) {
    let ops = operations(&registry.tools, super::repo::TOOLS, collected);
    let kind_tiers = kinds(crates, collected);
    let mut by_op: BTreeMap<String, Evidence> = BTreeMap::new();
    let mut by_kind: BTreeMap<String, Evidence> = BTreeMap::new();
    let mut laws: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in &registry.evidence {
        let Ok(text) = &file.text else { continue };
        if let Some(list) = paragraph(text, "Family laws") {
            for family in items(&list) {
                laws.entry(family).or_default().insert(file.path.clone());
            }
        }
        let Some(list) = paragraph(text, "Covers") else {
            continue;
        };
        for item in items(&list) {
            if ops.iter().any(|(name, ..)| *name == item) {
                for part in operation_parts(&file.path, text) {
                    let claim = by_op.entry(item.clone()).or_default().entry(part);
                    claim.or_default().insert(file.path.clone());
                }
                continue;
            }
            let prefix = format!("{item}::");
            let named: Vec<&String> = kind_tiers
                .keys()
                .filter(|k| **k == item || k.starts_with(&prefix))
                .collect();
            if named.is_empty() {
                collected.one(
                    &file.path,
                    Rule::Covers,
                    format!("`Covers:` names {item}, which is no operation or kind"),
                );
            }
            for kind in named {
                let claim = by_kind.entry(kind.clone()).or_default();
                let part = claim.entry(kind_part(&file.path)).or_default();
                part.insert(file.path.clone());
            }
        }
    }
    let op_values = ops
        .iter()
        .map(|(name, family, tier, version)| {
            let mut evidence = by_op.get(name).cloned().unwrap_or_default();
            if let Some(files) = laws.get(family) {
                evidence.insert("family_laws", files.clone());
            }
            let fields = vec![
                ("family", Value::from(family.as_str())),
                ("name", Value::from(name.as_str())),
                ("tier", Value::from(tier.as_str())),
                ("version", Value::Number(*version)),
            ];
            entry(fields, &evidence, &OPERATION_PARTS)
        })
        .collect();
    let kind_values = kind_tiers
        .iter()
        .map(|(name, tier)| {
            let evidence = by_kind.get(name).cloned().unwrap_or_default();
            let fields = vec![
                ("name", Value::from(name.as_str())),
                ("tier", Value::from(tier.as_str())),
            ];
            entry(fields, &evidence, &KIND_PARTS)
        })
        .collect();
    (op_values, kind_values)
}
