//! The kernel's one lint posture: every crate that ships is built with exactly `geom-num`'s lint
//! settings.
//!
//! The rule, exactly: identical copies of the settings, not a subset. Take the crate the table
//! names in `lints_from` (`geom-num`), and every crate of the table whose kind ships (a kernel
//! crate, `geom-pool` or the facade) that is in the workspace. Then:
//!
//! - the keys under `lints` in its `Cargo.toml` are the same keys as the reference's, each with the
//!   same value; and
//! - the clippy configuration clippy reads for it holds the same keys as the one clippy reads for the
//!   reference, each with the same value.
//!
//! A crate may neither leave a setting out nor add one, looser or stricter, so one posture covers the
//! kernel, and a change to `geom-num`'s settings fails this check until every crate carries it.
//! Comments and layout are not settings: a copy may word its comments for itself, and spacing, line
//! breaks, comments inside arrays and a comma before a closing bracket are ignored
//! (`manifest::canonical`). Everything else is compared as written: a table header, a dotted key
//! and an inline table name the same key, but `"deny"` and `'deny'`, or an inline table with its
//! keys in another order, differ, so the check errs towards reporting a difference, never towards
//! hiding one.
//!
//! The clippy configuration clippy reads for a crate is the first `.clippy.toml` or `clippy.toml`, in
//! that order, in the crate's folder or the nearest folder above it that holds one: clippy's lookup
//! from `CARGO_MANIFEST_DIR` (<https://doc.rust-lang.org/clippy/configuration.html>). Clippy 1.99.0,
//! run on a probe crate, shows both the order (with both files in one folder it reads
//! `.clippy.toml` and warns that it ignores `clippy.toml`) and the walk upwards. The search here
//! stops at the workspace root: a file above it is not part of the repository, so this check does
//! not answer for it. `CLIPPY_CONF_DIR`, which would override the lookup, is set by no workflow;
//! the merge gate (G12) starts its own clippy runs without it, refuses a cargo configuration file
//! on the way to a crate of the family (whose `[env]` could set it: `posture::cargo_configs`), and
//! refuses any environment in its workflow beyond its own pins (`gate::workflow`).
//!
//! What it does not cover: lint attributes in source (`#![allow(...)]`), which the merge gate's
//! `overrides` holds for the lints the kernel denies; `geom-num`'s settings themselves, which its
//! own piece sets and the merge gate (G12, `posture`) checks against chapters 21 and 27;
//! development crates, whose test tooling may use what kernel code may not; and binding crates,
//! which hold the audited `unsafe`.
//!
//! Method: key by key comparison of two ordered maps read by `manifest`.
//!
//! Cost: linear in the number of keys of each crate.

use std::collections::BTreeMap;

use crate::check::{Observed, Rule};
use crate::table::{Kind, Row, Table, full_name};

/// The names of a clippy configuration file, in the order clippy looks for them in one folder.
pub const CLIPPY_FILES: [&str; 2] = [".clippy.toml", "clippy.toml"];

/// The longest value a violation shows whole.
const SHOWN: usize = 72;

/// How much of a long value a violation shows before its first difference.
const BEFORE: usize = 16;

/// The clippy configuration clippy reads for a crate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClippyConfig {
    /// Its path relative to the workspace root, one component per entry.
    pub file: Vec<String>,
    /// Its keys, values canonical; `None` when it is not TOML the reader reads.
    pub settings: Option<BTreeMap<String, String>>,
}

/// A crate's lint settings, as the files cargo and clippy read for it set them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lints {
    /// The keys under `lints` in its manifest, values canonical; `None` when the manifest is not
    /// TOML the reader reads.
    pub tables: Option<BTreeMap<String, String>>,
    /// The clippy configuration clippy reads for it; `None` when no folder from its own up to the
    /// workspace root holds one, so clippy runs with its defaults.
    pub clippy: Option<ClippyConfig>,
}

/// Whether a crate of this kind carries the kernel's lint posture: every crate that ships.
pub fn carries_the_posture(kind: Kind) -> bool {
    match kind {
        Kind::Kernel | Kind::KernelStd | Kind::Facade => true,
        Kind::Binding | Kind::Development => false,
    }
}

/// `value` as a violation shows it: whole when short, else a window that starts a little before
/// character `from`.
fn shown(value: &str, from: usize) -> String {
    let count = value.chars().count();
    if count <= SHOWN {
        return value.to_owned();
    }
    let start = from.saturating_sub(BEFORE);
    let window: String = value.chars().skip(start).take(SHOWN).collect();
    let head = if start > 0 { "..." } else { "" };
    let tail = if start.saturating_add(SHOWN) < count {
        "..."
    } else {
        ""
    };
    format!("{head}{window}{tail}")
}

/// What differs between the settings of a copy and of its reference, one entry per key, in key
/// order: a key the copy leaves out, a key it adds, or a key it sets to another value.
pub fn differences(
    reference: &BTreeMap<String, String>,
    copy: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut found = Vec::new();
    for (key, wanted) in reference {
        match copy.get(key) {
            None => found.push(format!("lacks {key} = {}", shown(wanted, 0))),
            Some(got) if got != wanted => {
                let at = got
                    .chars()
                    .zip(wanted.chars())
                    .position(|(a, b)| a != b)
                    .unwrap_or_else(|| got.chars().count().min(wanted.chars().count()));
                found.push(format!(
                    "sets {key} = {}, not {}",
                    shown(got, at),
                    shown(wanted, at)
                ));
            }
            Some(_) => {}
        }
    }
    for (key, got) in copy {
        if !reference.contains_key(key) {
            found.push(format!("adds {key} = {}", shown(got, 0)));
        }
    }
    found
}

/// Checks the table's `lints_from`: it must name a kernel crate, whose settings the others copy.
pub fn table(
    table: &Table<'_>,
    rows: &BTreeMap<&str, &Row<'_>>,
    add: &mut impl FnMut(&str, Rule, String),
) {
    let source = full_name(table.lints_from);
    let kernel = rows
        .get(source.as_str())
        .is_some_and(|row| row.kind == Kind::Kernel);
    if !kernel {
        add(
            &source,
            Rule::BadLintSource,
            "the crate whose lint settings every crate that ships copies must be a kernel crate's \
             row"
            .to_owned(),
        );
    }
}

/// The reference's clippy configuration, as (its path, its settings); `None` after reporting why
/// it cannot serve.
fn reference_clippy<'o>(
    source: &str,
    lints: &'o Lints,
    add: &mut impl FnMut(&str, Rule, String),
) -> Option<(String, &'o BTreeMap<String, String>)> {
    let Some(config) = &lints.clippy else {
        add(
            source,
            Rule::ClippyConfig,
            "clippy reads no configuration for it, and every crate that ships copies its clippy \
             settings (chapter 21's disallowed methods and types)"
                .to_owned(),
        );
        return None;
    };
    let file = config.file.join("/");
    let Some(settings) = &config.settings else {
        add(
            source,
            Rule::ClippyConfig,
            format!(
                "{file} is not TOML this check reads, so the kernel's clippy settings are unknown"
            ),
        );
        return None;
    };
    Some((file, settings))
}

/// Compares one crate's `lints` keys with the reference's.
fn compare_tables(
    name: &str,
    source: &str,
    wanted: &BTreeMap<String, String>,
    lints: &Lints,
    add: &mut impl FnMut(&str, Rule, String),
) {
    let Some(got) = &lints.tables else {
        add(
            name,
            Rule::LintTables,
            "its Cargo.toml is not TOML this check reads".to_owned(),
        );
        return;
    };
    let wrong = differences(wanted, got);
    if !wrong.is_empty() {
        add(
            name,
            Rule::LintTables,
            format!(
                "its Cargo.toml's lint settings must be exactly {source}'s, the kernel's one \
                 posture: it {}",
                wrong.join("; it ")
            ),
        );
    }
}

/// Compares the settings of the clippy configuration one crate is built with with the reference's.
fn compare_clippy(
    name: &str,
    reference_file: &str,
    wanted: &BTreeMap<String, String>,
    lints: &Lints,
    add: &mut impl FnMut(&str, Rule, String),
) {
    let Some(config) = &lints.clippy else {
        add(
            name,
            Rule::ClippyConfig,
            format!("clippy reads no configuration for it: copy {reference_file} into its folder"),
        );
        return;
    };
    let file = config.file.join("/");
    let Some(got) = &config.settings else {
        add(
            name,
            Rule::ClippyConfig,
            format!(
                "{file}, the clippy configuration it is built with, is not TOML this check reads"
            ),
        );
        return;
    };
    let wrong = differences(wanted, got);
    if !wrong.is_empty() {
        add(
            name,
            Rule::ClippyConfig,
            format!(
                "{file}, the clippy configuration it is built with, must hold exactly the settings \
                 of {reference_file}: it {}",
                wrong.join("; it ")
            ),
        );
    }
}

/// Proves every crate that carries the posture is built with exactly the reference's settings.
pub fn workspace(table: &Table<'_>, observed: &Observed, add: &mut impl FnMut(&str, Rule, String)) {
    let source = full_name(table.lints_from);
    let Some(reference) = observed.packages.get(&source) else {
        add(
            &source,
            Rule::LintTables,
            "it is not in the workspace, and every crate that ships copies its lint settings, so \
             none can be checked"
                .to_owned(),
        );
        return;
    };
    let tables = reference.lints.tables.as_ref();
    if tables.is_none() {
        add(
            &source,
            Rule::LintTables,
            "its Cargo.toml is not TOML this check reads, so the kernel's lint settings are unknown"
                .to_owned(),
        );
    }
    let clippy = reference_clippy(&source, &reference.lints, add);
    for row in table
        .rows
        .iter()
        .filter(|row| carries_the_posture(row.kind) && row.name != source)
    {
        let Some(package) = observed.packages.get(row.name) else {
            continue;
        };
        if let Some(wanted) = tables {
            compare_tables(row.name, &source, wanted, &package.lints, add);
        }
        if let Some((reference_file, wanted)) = &clippy {
            compare_clippy(row.name, reference_file, wanted, &package.lints, add);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn identical_settings_have_no_differences() {
        let reference = map(&[("a", "\"deny\""), ("b", "true")]);
        assert_eq!(
            differences(&reference, &reference.clone()),
            Vec::<String>::new()
        );
        assert_eq!(
            differences(&BTreeMap::new(), &BTreeMap::new()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn every_kind_of_difference_is_named() {
        let reference = map(&[("a", "\"deny\""), ("b", "true"), ("c", "1")]);
        let copy = map(&[("a", "\"warn\""), ("c", "1"), ("d", "\"allow\"")]);
        assert_eq!(
            differences(&reference, &copy),
            [
                "sets a = \"warn\", not \"deny\"",
                "lacks b = true",
                "adds d = \"allow\"",
            ]
        );
        // Stricter is a difference too: the copy may not add a setting or raise a level.
        let stricter = map(&[
            ("a", "\"forbid\""),
            ("b", "true"),
            ("c", "1"),
            ("e", "\"deny\""),
        ]);
        assert_eq!(differences(&reference, &stricter).len(), 2);
    }

    #[test]
    fn a_long_value_is_shown_around_its_first_difference() {
        let wanted = format!("[{}\"f64::max\"]", "\"f64::sin\",".repeat(20));
        let got = format!("[{}\"f64::min\"]", "\"f64::sin\",".repeat(20));
        let found = differences(&map(&[("m", &wanted)]), &map(&[("m", &got)]));
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("\"f64::min\"]"), "{found:?}");
        assert!(found[0].contains("\"f64::max\"]"), "{found:?}");
        assert!(found[0].starts_with("sets m = ..."), "{found:?}");
    }

    #[test]
    fn the_kinds_that_ship_carry_the_posture() {
        for (kind, carries) in [
            (Kind::Kernel, true),
            (Kind::KernelStd, true),
            (Kind::Facade, true),
            (Kind::Binding, false),
            (Kind::Development, false),
        ] {
            assert_eq!(carries_the_posture(kind), carries, "{kind:?}");
        }
    }
}
