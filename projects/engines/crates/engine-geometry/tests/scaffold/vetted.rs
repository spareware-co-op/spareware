//! The vetted allow-list: the published crates that may ship inside a `no_std` kernel crate, each
//! with its record against the kernel's principles, in one place, `deny-no-std.toml`'s
//! `bans.allow` (piece G12a; chapter 29.1 and 29.20, rule 3, as the owner directed on 2026-10-22).
//!
//! What it does: `read` takes the allow-list from the configuration's keys: every entry a table
//! `{ crate = "<name>@<version>", reason = "<record>" }`, cargo-deny's own form, in which `@` pins
//! the exact version (cargo-deny's `PackageSpec`: a version after `@` is made an exact
//! requirement). The record is four clauses, in this order, each saying how the crate meets one
//! principle:
//!
//! - `deterministic:` why it gives the same bits on every target (principle 2, chapter 21);
//! - `no_std:` that it builds without `std`, as every kernel crate must (chapter 27);
//! - `audited:` either the committed audit, by its path from the repository root, or `pending,
//!   piece <ID>` naming the board piece that owes it (29.20, rule 3: "gets a written audit
//!   first");
//! - `pinned:` the exact version again, then where it is held (`Cargo.lock`, and here).
//!
//! `against_lock` holds the list to `Cargo.lock`: every entry's crate at exactly its version, from
//! crates.io. The merge gate's rules (`deny::rules`) read the same list to refuse every crate in a
//! `no_std` kernel crate's graph that is not on it, at its version, from crates.io, and cargo-deny
//! refuses the same with the same file.
//!
//! What it guarantees: an entry is vetted only when every clause is there and says something, its
//! version is exact (`major.minor.patch`, digits only) and its `pinned:` clause names the same
//! version; anything else is refused with what is missing, never read in part. A crate is named
//! once. A list that is missing or empty is refused, because cargo-deny checks an allow-list only
//! when it holds an entry, and would then allow every crate. The list says nothing about which kernel crate may name the crate: that is the crate
//! table's rule (`table`, `check`), which holds that only `geom-num` names `libm`.
//!
//! Method: cargo-deny 0.20.2's `bans.allow` with its `crate` and `reason` keys
//! (<https://github.com/EmbarkStudios/cargo-deny/tree/bca0dde53651ee946720e4540b5ce2610bec8f06>,
//! `docs/src/checks/bans/cfg.md` and `src/cfg/package_spec.rs`, read for behaviour only), so the
//! one file is both cargo-deny's list and the record; TOML read by the `toml` crate through
//! `manifest`; `Cargo.lock`'s `[[package]]` tables (name, version, source), as cargo writes them
//! (<https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html>).
//!
//! Cost: linear in the list and in `Cargo.lock`'s packages.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;

use crate::manifest::{Keys, Value, keys};

/// The key of the allow-list in `deny-no-std.toml`.
pub const ALLOW_KEY: &str = "bans.allow";

/// The four clauses of a record, in their order.
pub const CLAUSES: [&str; 4] = ["deterministic", "no_std", "audited", "pinned"];

/// The source `Cargo.lock` gives a crate from crates.io.
pub const CRATES_IO_LOCK_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// How an entry was audited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Audit {
    /// The committed audit, by its path from the repository root.
    File(String),
    /// Not yet: the board piece that owes it, and what the clause says after it.
    Pending {
        /// The piece, such as `G24`.
        piece: String,
        /// The rest of the clause.
        detail: String,
    },
}

/// One crate of the allow-list, with its record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vetted {
    /// The crate's name.
    pub name: String,
    /// Its exact version.
    pub version: String,
    /// The `deterministic:` clause.
    pub deterministic: String,
    /// The `no_std:` clause.
    pub no_std: String,
    /// The `audited:` clause.
    pub audited: Audit,
    /// The `pinned:` clause.
    pub pinned: String,
}

/// Whether `version` is exact: three dot-separated runs of digits, with no leading zero but `0`.
fn exact_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.chars().all(|c| c.is_ascii_digit())
                && (*p == "0" || !p.starts_with('0'))
        })
}

/// Whether `name` is a crate name crates.io accepts: ASCII letters, digits, `-` and `_`,
/// starting with a letter.
fn crate_name(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Whether `id` names a board piece: `G`, digits, and at most one lower-case letter (`G24`,
/// `G12a`).
fn piece_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix('G') else {
        return false;
    };
    let digits = rest.trim_end_matches(|c: char| c.is_ascii_lowercase());
    !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
        && rest.len() <= digits.len().saturating_add(1)
}

/// The `audited:` clause, read.
fn audit(clause: &str) -> Result<Audit, String> {
    if let Some(rest) = clause.strip_prefix("pending, piece ") {
        let (piece, detail) = rest.split_once(' ').unwrap_or((rest, ""));
        let piece = piece.trim_end_matches([',', ':', ';']);
        if !piece_id(piece) {
            return Err(format!(
                "its audit is pending but names no board piece that owes it (`pending, piece \
                 G24 ...`), only {rest:?}"
            ));
        }
        return Ok(Audit::Pending {
            piece: piece.to_owned(),
            detail: detail.trim().to_owned(),
        });
    }
    let path = clause
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches([',', ';', ':']);
    let is_path = path.contains('/')
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.split('/').any(|part| part.is_empty() || part == "..");
    if is_path {
        Ok(Audit::File(path.to_owned()))
    } else {
        Err(format!(
            "its audited: clause is neither the committed audit's path from the repository root \
             nor `pending, piece <ID>`: {clause:?}"
        ))
    }
}

/// The record a `reason` holds; what is wrong with it, otherwise.
fn record(reason: &str, version: &str) -> Result<(String, String, Audit, String), Vec<String>> {
    let mut wrong = Vec::new();
    let mut texts: Vec<String> = Vec::new();
    let mut rest = reason;
    for (i, clause) in CLAUSES.iter().enumerate() {
        let label = if i == 0 {
            format!("{clause}: ")
        } else {
            format!("; {clause}: ")
        };
        let Some(after) = rest.strip_prefix(&label) else {
            wrong.push(format!(
                "its record does not give the {clause}: clause where it belongs (the clauses are \
                 {}, in that order, each after \"; \")",
                CLAUSES.join(", ")
            ));
            return Err(wrong);
        };
        let end = CLAUSES
            .get(i.saturating_add(1))
            .and_then(|next| after.find(&format!("; {next}: ")))
            .unwrap_or(after.len());
        let (text, tail) = after.split_at(end);
        if text.trim().is_empty() {
            wrong.push(format!("its {clause}: clause says nothing"));
        }
        texts.push(text.trim().to_owned());
        rest = tail;
    }
    let [deterministic, no_std, audited, pinned] = <[String; 4]>::try_from(texts).unwrap();
    let audited = audit(&audited).map_err(|e| vec![e]);
    let pins = pinned
        .strip_prefix(version)
        .is_some_and(|after| after.is_empty() || after.starts_with([',', ' ', ';']));
    if !pins {
        wrong.push(format!(
            "its pinned: clause does not start with its exact version {version}: {pinned:?}"
        ));
    }
    match audited {
        Ok(audited) if wrong.is_empty() => Ok((deterministic, no_std, audited, pinned)),
        Ok(_) => Err(wrong),
        Err(e) => {
            wrong.extend(e);
            Err(wrong)
        }
    }
}

/// One entry of the list, read; what is wrong with it, otherwise.
fn entry(item: &Value) -> Result<Vetted, Vec<String>> {
    let Value::Table(table) = item else {
        return Err(vec![format!(
            "{item} is not vetted: every entry is {{ crate = \"<name>@<version>\", reason = \
             \"<record>\" }}, so it carries its pinned version and its record"
        )]);
    };
    let spec = table.get("crate").and_then(Value::as_str);
    let reason = table.get("reason").and_then(Value::as_str);
    let others: Vec<&String> = table
        .keys()
        .filter(|k| !matches!(k.as_str(), "crate" | "reason"))
        .collect();
    let (Some(spec), Some(reason)) = (spec, reason) else {
        return Err(vec![format!(
            "{item} lacks a crate or a reason string: every entry is {{ crate = \
             \"<name>@<version>\", reason = \"<record>\" }}"
        )]);
    };
    let mut wrong = Vec::new();
    if !others.is_empty() {
        wrong.push(format!(
            "{spec} sets {others:?}, which cargo-deny refuses beside crate and reason"
        ));
    }
    let Some((name, version)) = spec.split_once('@') else {
        return Err(vec![format!(
            "{spec} is not pinned: the crate is written <name>@<version>, which cargo-deny reads \
             as exactly that version"
        )]);
    };
    if !crate_name(name) {
        wrong.push(format!("{spec}: {name:?} is not a crate name"));
    }
    if !exact_version(version) {
        wrong.push(format!(
            "{spec} is not pinned to one version: {version:?} is not major.minor.patch"
        ));
    }
    match record(reason, version) {
        Ok((deterministic, no_std, audited, pinned)) if wrong.is_empty() => Ok(Vetted {
            name: name.to_owned(),
            version: version.to_owned(),
            deterministic,
            no_std,
            audited,
            pinned,
        }),
        Ok(_) => Err(wrong),
        Err(more) => {
            wrong.extend(more.into_iter().map(|m| format!("{spec}: {m}")));
            Err(wrong)
        }
    }
}

/// The allow-list among a `deny-no-std.toml`'s keys; every way it is not a vetted list, otherwise.
pub fn read(keys: &Keys) -> Result<Vec<Vetted>, Vec<String>> {
    let Some(value) = keys.get(ALLOW_KEY) else {
        return Err(vec![format!(
            "{ALLOW_KEY} is not set, so cargo-deny would allow every crate: the vetted \
             allow-list is the only way a published crate ships in a kernel crate (29.1)"
        )]);
    };
    let Some(items) = value.as_array() else {
        return Err(vec![format!("{ALLOW_KEY} is not a list")]);
    };
    if items.is_empty() {
        // cargo-deny 0.20.2 checks the list only when it is not empty (`src/bans.rs`, `if
        // !allowed.0.is_empty()`), so an empty one would let every crate through it.
        return Err(vec![format!(
            "{ALLOW_KEY} is empty, which cargo-deny reads as no allow-list at all, so every crate \
             would pass it"
        )]);
    }
    let mut vetted = Vec::new();
    let mut wrong = Vec::new();
    let mut names = BTreeSet::new();
    for item in items {
        match entry(item) {
            Ok(v) => {
                if !names.insert(v.name.clone()) {
                    wrong.push(format!("{} is listed twice", v.name));
                }
                vetted.push(v);
            }
            Err(e) => wrong.extend(e),
        }
    }
    if wrong.is_empty() {
        Ok(vetted)
    } else {
        Err(wrong)
    }
}

/// The allow-list of a `deny-no-std.toml`, from its text.
pub fn read_text(text: &str) -> Result<Vec<Vetted>, Vec<String>> {
    let Some(keys) = keys(text) else {
        return Err(vec!["it is not TOML".to_owned()]);
    };
    read(&keys)
}

/// What `Cargo.lock` says against the list: every entry's crate must be locked at exactly its
/// version, from crates.io, so the list never vets a version the workspace does not build, nor a
/// crate patched in.
pub fn against_lock(vetted: &[Vetted], lock: &str) -> Vec<String> {
    let Some(packages) = keys(lock)
        .and_then(|k| k.get("package").cloned())
        .and_then(|p| p.as_array().cloned())
    else {
        return vec!["Cargo.lock is not TOML with [[package]] tables".to_owned()];
    };
    let locked: BTreeSet<(String, String, String)> = packages
        .iter()
        .map(|p| {
            let text = |key: &str| p.get(key).and_then(Value::as_str).unwrap_or("").to_owned();
            (text("name"), text("version"), text("source"))
        })
        .collect();
    let mut wrong = Vec::new();
    for v in vetted {
        let wanted = (
            v.name.clone(),
            v.version.clone(),
            CRATES_IO_LOCK_SOURCE.to_owned(),
        );
        if !locked.contains(&wanted) {
            let found: Vec<String> = locked
                .iter()
                .filter(|(name, _, _)| *name == v.name)
                .map(|(_, version, source)| format!("{version} from {source:?}"))
                .collect();
            wrong.push(format!(
                "{}@{} is vetted, but Cargo.lock holds {}: the list and the lock name one version \
                 from crates.io, changed together with a new record",
                v.name,
                v.version,
                if found.is_empty() {
                    "no such crate".to_owned()
                } else {
                    found.join(", ")
                }
            ));
        }
    }
    wrong
}

/// Every entry whose record names a committed audit that `exists` does not find, by its path from
/// the repository root: a record may not point at an audit that is not there.
pub fn missing_audits(vetted: &[Vetted], exists: impl Fn(&str) -> bool) -> Vec<String> {
    vetted
        .iter()
        .filter_map(|v| match &v.audited {
            Audit::File(path) if !exists(path) => Some(format!(
                "{}@{}: its audit {path} is not in the repository",
                v.name, v.version
            )),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record that holds every clause, for the crate at `version`.
    fn record_text(version: &str) -> String {
        format!(
            "deterministic: pure Rust, correctly rounded; no_std: yes; audited: pending, piece \
             G24 owes it; pinned: {version}, in Cargo.lock and here"
        )
    }

    fn list(entries: &str) -> Result<Vec<Vetted>, Vec<String>> {
        read_text(&format!("[bans]\nallow = [\n{entries}\n]\n"))
    }

    #[test]
    fn a_vetted_entry_is_read_clause_by_clause() {
        let got = list(&format!(
            "{{ crate = \"libm@0.2.16\", reason = \"{}\" }},",
            record_text("0.2.16")
        ))
        .unwrap();
        assert_eq!(
            got,
            [Vetted {
                name: "libm".to_owned(),
                version: "0.2.16".to_owned(),
                deterministic: "pure Rust, correctly rounded".to_owned(),
                no_std: "yes".to_owned(),
                audited: Audit::Pending {
                    piece: "G24".to_owned(),
                    detail: "owes it".to_owned()
                },
                pinned: "0.2.16, in Cargo.lock and here".to_owned(),
            }]
        );
        let file = list(
            "{ crate = \"a@1.0.0\", reason = \"deterministic: x; no_std: y; audited: \
             projects/engines/geometry/audits/a-1.0.0.md, signed; pinned: 1.0.0\" }",
        )
        .unwrap();
        assert_eq!(
            file[0].audited,
            Audit::File("projects/engines/geometry/audits/a-1.0.0.md".to_owned())
        );
    }

    /// Entries that are not vetted, each with what its refusal must name.
    fn unvetted() -> Vec<(&'static str, String, &'static str)> {
        let good = record_text("0.2.16");
        vec![
            ("a bare name", "\"libm\"".to_owned(), "is not vetted"),
            (
                "no version",
                format!("{{ crate = \"libm\", reason = \"{good}\" }}"),
                "is not pinned",
            ),
            (
                "a range",
                format!("{{ crate = \"libm@0.2\", reason = \"{good}\" }}"),
                "not pinned to one version",
            ),
            (
                "a requirement",
                format!("{{ crate = \"libm@^0.2.16\", reason = \"{good}\" }}"),
                "not pinned to one version",
            ),
            (
                "no reason",
                "{ crate = \"libm@0.2.16\" }".to_owned(),
                "lacks a crate or a reason",
            ),
            (
                "no record",
                "{ crate = \"libm@0.2.16\", reason = \"vetted\" }".to_owned(),
                "deterministic: clause",
            ),
            (
                "a clause left out",
                format!(
                    "{{ crate = \"libm@0.2.16\", reason = \"{}\" }}",
                    good.replace("no_std: yes; ", "")
                ),
                "no_std: clause",
            ),
            (
                "clauses out of order",
                "{ crate = \"libm@0.2.16\", reason = \"no_std: yes; deterministic: x; audited: \
                 pending, piece G24; pinned: 0.2.16\" }"
                    .to_owned(),
                "deterministic: clause",
            ),
            (
                "an empty clause",
                format!(
                    "{{ crate = \"libm@0.2.16\", reason = \"{}\" }}",
                    good.replace("no_std: yes", "no_std:  ")
                ),
                "no_std: clause says nothing",
            ),
            (
                "an audit owed by nobody",
                format!(
                    "{{ crate = \"libm@0.2.16\", reason = \"{}\" }}",
                    good.replace("pending, piece G24 owes it", "pending")
                ),
                "neither the committed audit",
            ),
            (
                "an audit owed by no piece",
                format!(
                    "{{ crate = \"libm@0.2.16\", reason = \"{}\" }}",
                    good.replace("piece G24", "piece someone")
                ),
                "names no board piece",
            ),
            (
                "an audit outside the repository",
                format!(
                    "{{ crate = \"libm@0.2.16\", reason = \"{}\" }}",
                    good.replace("pending, piece G24 owes it", "../audit.md")
                ),
                "neither the committed audit",
            ),
            (
                "another version pinned",
                format!(
                    "{{ crate = \"libm@0.2.16\", reason = \"{}\" }}",
                    record_text("0.2.15")
                ),
                "does not start with its exact version 0.2.16",
            ),
            (
                "a key cargo-deny refuses",
                format!("{{ crate = \"libm@0.2.16\", reason = \"{good}\", version = \"1\" }}"),
                "cargo-deny refuses",
            ),
            (
                "listed twice",
                format!(
                    "{{ crate = \"libm@0.2.16\", reason = \"{good}\" }},\n\
                     {{ crate = \"libm@0.2.16\", reason = \"{good}\" }}"
                ),
                "listed twice",
            ),
        ]
    }

    #[test]
    fn every_unvetted_entry_is_refused_with_its_fault() {
        for (name, entries, needle) in unvetted() {
            let got = list(&entries);
            assert!(
                got.as_ref()
                    .is_err_and(|e| e.iter().any(|m| m.contains(needle))),
                "{name}: {got:#?} names no {needle:?}"
            );
        }
        // No list at all, or not a list: nothing is vetted, so everything would be allowed.
        assert!(read_text("[bans]\n").is_err_and(|e| e[0].contains("is not set")));
        assert!(
            read_text("[bans]\nallow = \"libm\"\n").is_err_and(|e| e[0].contains("not a list"))
        );
        assert!(read_text("[bans]\nallow = []\n").is_err_and(|e| e[0].contains("is empty")));
        assert!(read_text("[bans\n").is_err());
    }

    #[test]
    fn versions_names_and_pieces_are_read_strictly() {
        for good in ["0.2.16", "1.0.0", "10.20.30"] {
            assert!(exact_version(good), "{good}");
        }
        for bad in [
            "0.2",
            "0.2.16.1",
            "01.2.3",
            "0.2.x",
            "0.2.16-rc1",
            "",
            "0..1",
        ] {
            assert!(!exact_version(bad), "{bad}");
        }
        for good in ["libm", "serde_json", "a-b"] {
            assert!(crate_name(good), "{good}");
        }
        for bad in ["", "1abc", "a b", "a/b"] {
            assert!(!crate_name(bad), "{bad}");
        }
        for good in ["G24", "G12a", "G100"] {
            assert!(piece_id(good), "{good}");
        }
        for bad in ["G", "Gx", "G12ab", "24", "g24", "G12A"] {
            assert!(!piece_id(bad), "{bad}");
        }
    }

    #[test]
    fn a_committed_audit_must_be_there() {
        let vetted = list(
            "{ crate = \"a@1.0.0\", reason = \"deterministic: x; no_std: y; audited: \
             audits/a.md; pinned: 1.0.0\" },\n\
             { crate = \"b@1.0.0\", reason = \"deterministic: x; no_std: y; audited: pending, \
             piece G24; pinned: 1.0.0\" },",
        )
        .unwrap();
        assert_eq!(
            missing_audits(&vetted, |p| p == "audits/a.md"),
            Vec::<String>::new()
        );
        let missing = missing_audits(&vetted, |_| false);
        assert_eq!(
            missing,
            ["a@1.0.0: its audit audits/a.md is not in the repository"]
        );
    }

    const LOCK: &str = "version = 4\n\n\
        [[package]]\nname = \"libm\"\nversion = \"0.2.16\"\n\
        source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
        checksum = \"0\"\n\n\
        [[package]]\nname = \"geom-num\"\nversion = \"0.0.0\"\ndependencies = [\"libm\"]\n";

    #[test]
    fn the_list_must_name_what_the_lock_holds() {
        let vetted = list(&format!(
            "{{ crate = \"libm@0.2.16\", reason = \"{}\" }}",
            record_text("0.2.16")
        ))
        .unwrap();
        assert_eq!(against_lock(&vetted, LOCK), Vec::<String>::new());
        for (name, lock, needle) in [
            (
                "another version locked",
                LOCK.replace("0.2.16", "0.2.17"),
                "0.2.17 from",
            ),
            (
                "patched in from a folder",
                LOCK.replace(
                    "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
                    "",
                ),
                "from \"\"",
            ),
            (
                "not locked at all",
                LOCK.replace("name = \"libm\"", "name = \"libm2\""),
                "no such crate",
            ),
            ("not a lock file", "[[package]\n".to_owned(), "not TOML"),
        ] {
            let found = against_lock(&vetted, &lock);
            assert!(
                found.len() == 1 && found[0].contains(needle),
                "{name}: {found:#?}"
            );
        }
    }
}
