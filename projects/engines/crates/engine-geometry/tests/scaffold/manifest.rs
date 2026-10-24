//! Reading TOML: a cargo manifest's lint tables, a clippy configuration, the toolchain file and the
//! cargo-deny configurations, each parsed by the `toml` crate and flattened into its keys.
//!
//! What it does: parses a TOML document with the `toml` crate and lists every key in full dotted
//! form with its value (`keys`); from them, the level of `unsafe_code` in a manifest's
//! `[lints.rust]` table, or in the workspace's `[workspace.lints.rust]` table when the manifest sets
//! `[lints] workspace = true` (`manifest_forbids_unsafe`), and every key under `lints` in a manifest
//! (`lint_settings`). The lint posture check compares the values these give; the merge gate (G12)
//! reads the items of arrays and the text of strings from them through `toml::Value`'s own
//! accessors.
//!
//! What it guarantees: a key counts only as a key of a real table, never as text inside a string,
//! an array or a comment; a table header, a dotted key and an inline table name the same key, as
//! TOML defines them. Values are TOML values, not text: two files that set the same value in
//! another layout (spacing, line breaks, comments, `"deny"` or `'deny'`, an inline table's keys in
//! another order) give equal values, as cargo and clippy read them, and any other change gives a
//! different one. An array, an array of tables included, is one value under its key. A file the
//! parser refuses forbids nothing and has no settings, so a malformed file shows as a violation,
//! never as a pass.
//!
//! Method: the TOML 1.0 grammar (<https://toml.io/en/v1.0.0>) as the `toml` crate reads it
//! (<https://docs.rs/toml/0.9.6/toml/>), the reader of the toml-rs project that cargo's own manifest
//! reading is built on; the tables are flattened with an explicit work list, no recursion. Piece
//! G12a put the crate in place of a reader written here (chapter 29.1, the owner's direction of
//! 2026-10-22: use well-known permissive crates rather than hand-rolling).
//!
//! Cost: linear in the length of the file for the parse, and one entry per key for the flattening.
//! The parser bounds how deep tables and arrays may nest, so a deep file is an error, not a stack
//! overflow.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

pub use toml::Value;

/// Every key of a TOML document, in full dotted form, with its value.
pub type Keys = BTreeMap<String, Value>;

/// Whether a manifest forbids unsafe code through its `[lints.rust]` table, or through the
/// workspace's `[workspace.lints.rust]` table when it sets `[lints] workspace = true`.
pub fn manifest_forbids_unsafe(manifest: &str, workspace_manifest: &str) -> bool {
    let Some(own) = keys(manifest) else {
        return false;
    };
    if let Some(forbids) = lint_forbids(&own, "lints.rust.unsafe_code") {
        return forbids;
    }
    if own.get("lints.workspace").and_then(Value::as_bool) != Some(true) {
        return false;
    }
    keys(workspace_manifest)
        .and_then(|root| lint_forbids(&root, "workspace.lints.rust.unsafe_code"))
        .unwrap_or(false)
}

/// The level a lint key sets, in its plain (`"forbid"`) or table (`{ level = "forbid" }`) form,
/// compared with `forbid`; `None` when the key is not set.
fn lint_forbids(keys: &Keys, key: &str) -> Option<bool> {
    let level = keys
        .get(key)
        .or_else(|| keys.get(&format!("{key}.level")))?;
    Some(level.as_str() == Some("forbid"))
}

/// Every key of a TOML document with its value: keys in full dotted form (`lints.rust.unsafe_code`,
/// from a table header, a dotted key or an inline table alike), each value a TOML value that is not
/// a table. An array, an array of tables included, is the value of its key. `None` when the text is
/// not TOML.
pub fn keys(text: &str) -> Option<Keys> {
    let table: toml::Table = text.parse().ok()?;
    let mut out = Keys::new();
    let mut work: Vec<(String, toml::Table)> = vec![(String::new(), table)];
    while let Some((prefix, table)) = work.pop() {
        for (key, value) in table {
            let full = if prefix.is_empty() {
                key
            } else {
                format!("{prefix}.{key}")
            };
            match value {
                Value::Table(inner) => work.push((full, inner)),
                other => {
                    out.insert(full, other);
                }
            }
        }
    }
    Some(out)
}

/// Every key under `lints` of a manifest (`lints.rust.unsafe_code`, `lints.clippy.pedantic.level`
/// and so on), with its value; `None` when the manifest is not TOML.
pub fn lint_settings(manifest: &str) -> Option<Keys> {
    let mut found = keys(manifest)?;
    found.retain(|key, _| key == "lints" || key.starts_with("lints."));
    Some(found)
}

/// The texts of an array of strings, in order; `None` when the value is not one.
pub fn strings(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_owned))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_lints() {
        let own = "[package]\nname = \"a\"\n\n[lints.rust]\nunsafe_code = \"forbid\" # never\n";
        assert!(manifest_forbids_unsafe(own, ""));
        let table_form = "[lints.rust]\nunsafe_code = { level = \"forbid\", priority = 1 }\n";
        assert!(manifest_forbids_unsafe(table_form, ""));
        let dotted = "[lints]\nrust.unsafe_code = \"forbid\"\n";
        assert!(manifest_forbids_unsafe(dotted, ""));
        let inline = "lints = { rust = { unsafe_code = 'forbid' } }\n";
        assert!(manifest_forbids_unsafe(inline, ""));
        let deny = "[lints.rust]\nunsafe_code = \"deny\"\n";
        assert!(!manifest_forbids_unsafe(deny, ""));
        let inherited = "[lints]\nworkspace = true\n";
        let root = "[workspace.lints.rust]\nunsafe_code = \"forbid\"\n";
        assert!(manifest_forbids_unsafe(inherited, root));
        assert!(!manifest_forbids_unsafe(inherited, "[workspace]\n"));
        let other_table = "[lints.clippy]\nunsafe_code = \"forbid\"\n";
        assert!(!manifest_forbids_unsafe(other_table, ""));
        // `workspace = "true"` is a string, not the boolean cargo reads.
        assert!(!manifest_forbids_unsafe("[lints]\nworkspace = \"true\"\n", root));
    }

    #[test]
    fn a_lint_table_written_inside_a_string_is_text() {
        for quotes in ["\"\"\"", "'''"] {
            let manifest = format!(
                "[package]\nname = \"a\"\ndescription = {quotes}\n[lints.rust]\n\
                 unsafe_code = \"forbid\"\n{quotes}\n"
            );
            assert!(!manifest_forbids_unsafe(&manifest, ""), "{manifest}");
        }
        let in_array = "[package]\nkeywords = [\n  \"[lints.rust]\", # a comment ]\n  \
                        \"unsafe_code = 'forbid'\",\n]\n";
        assert!(!manifest_forbids_unsafe(in_array, ""));
        // What follows the string is read again: the real table counts.
        let after = "[package]\ndescription = \"\"\"\n[x]\n\"\"\"\n\n[lints.rust]\n\
                     unsafe_code = \"forbid\"\n";
        assert!(manifest_forbids_unsafe(after, ""));
        // A manifest the parser refuses forbids nothing.
        assert!(!manifest_forbids_unsafe(
            "[lints.rust]\nunsafe_code = \"forbid\n",
            ""
        ));
    }

    #[test]
    fn keys_of_a_manifest() {
        let found = keys(
            "[package]\nname = \"geom-a\" # name\nedition.workspace = true\n\
             [dependencies]\ngeom-num = { path = \"../geom-num\" }\n\
             [[bench]]\nname = \"b\"\n[[bench]]\nname = \"c\"\n",
        )
        .unwrap();
        assert_eq!(found["package.name"].as_str(), Some("geom-a"));
        assert_eq!(found["package.edition.workspace"].as_bool(), Some(true));
        assert_eq!(found["dependencies.geom-num.path"].as_str(), Some("../geom-num"));
        // An array of tables is one value, every table of it kept.
        let benches = found["bench"].as_array().unwrap();
        assert_eq!(benches.len(), 2);
        assert_eq!(benches[1].get("name").and_then(Value::as_str), Some("c"));
        // A key defined twice, and a document nested past the parser's bound, are refused.
        assert_eq!(keys("a = 1\na = 2\n"), None);
        let deep = format!("a = {}1{}\n", "[".repeat(1000), "]".repeat(1000));
        assert_eq!(keys(&deep), None);
    }

    #[test]
    fn values_compare_as_toml_reads_them() {
        let read = |text: &str| keys(&format!("v = {text}\n")).unwrap()["v"].clone();
        // Layout, comments, quote style and an inline table's key order are not values.
        assert_eq!(
            read("[\n    # max may return either zero.\n    { path = \"f64::max\", \
                  reason = \"# not a comment\" },\n    { path = 'f64::min' },\n]"),
            read("[{reason=\"# not a comment\",path=\"f64::max\"},{path=\"f64::min\"}]")
        );
        assert_eq!(read("\"deny\""), read("'deny'"));
        assert_eq!(read("{ a = 1, b = 2 }"), read("{ b = 2, a = 1 }"));
        // Everything else is a value: text in strings, types, numbers, order in arrays.
        assert_ne!(read("\"a b\""), read("\"ab\""));
        assert_ne!(read("true"), read("\"true\""));
        assert_ne!(read("-1"), read("1"));
        assert_ne!(read("[\"a\", \"b\"]"), read("[\"b\", \"a\"]"));
        assert_eq!(read("\"deny\"").to_string(), "\"deny\"");
    }

    #[test]
    fn strings_of_an_array() {
        let found = keys(
            "allow = [\n  \"MIT\", # a comment, with a comma\n  'Apache-2.0 WITH LLVM-exception',\n]\n\
             mixed = [\"a\", 1]\nnot = \"MIT\"\n",
        )
        .unwrap();
        assert_eq!(
            strings(&found["allow"]).unwrap(),
            ["MIT", "Apache-2.0 WITH LLVM-exception"]
        );
        assert_eq!(strings(&found["mixed"]), None);
        assert_eq!(strings(&found["not"]), None);
    }

    #[test]
    fn lint_settings_of_a_manifest() {
        let tables = "[package]\nname = \"a\"\n\n[lints.rust]\nunsafe_code = \"forbid\"\n\n\
                      [lints.clippy]\n# pedantic, below the others\n\
                      pedantic = { level = \"warn\", priority = -1 }\nfloat_cmp = \"allow\"\n";
        let got = lint_settings(tables).unwrap();
        let expected: Keys = [
            ("lints.rust.unsafe_code", Value::from("forbid")),
            ("lints.clippy.pedantic.level", Value::from("warn")),
            ("lints.clippy.pedantic.priority", Value::Integer(-1)),
            ("lints.clippy.float_cmp", Value::from("allow")),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        assert_eq!(got, expected);
        // Only keys under `lints`.
        let dotted = "lints.rust.unsafe_code = 'forbid'\n[package]\nname = \"b\"\n";
        assert_eq!(
            lint_settings(dotted)
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>(),
            [("lints.rust.unsafe_code".to_owned(), Value::from("forbid"))]
        );
        // The same settings in another layout, with other comments, read the same.
        let relaid = "[lints]\nclippy.float_cmp = \"allow\" # exact\nrust = { unsafe_code = \"forbid\" }\n\
                      [lints.clippy.pedantic]\npriority = -1\nlevel = \"warn\"\n";
        assert_eq!(lint_settings(relaid).unwrap(), expected);
        // A manifest the parser refuses has no settings.
        assert_eq!(lint_settings("[lints.rust\n"), None);
        assert_eq!(keys("a = \"open\n"), None);
    }
}
