//! The merge gate's rule on lint attributes in source (piece G12): outside test code, no crate that
//! ships allows, warns on or expects a lint the kernel denies.
//!
//! What it does: chapter 27 has `clippy` deny `unwrap`, `expect`, `panic` and unchecked indexing in
//! kernel code, chapter 21 has it ban its lists, and `posture` holds every crate that ships to
//! those denials in its manifest. A lint attribute in the code overrides the manifest:
//! `#![allow(clippy::unwrap_used)]` at a crate root switches the denial off for the whole crate,
//! and clippy then passes what the posture says it refuses. So for every crate of the table whose
//! kind ships and that is in the workspace, this check reads every source file the gate reads for
//! it (`repo::crate_sources`, and the files the compiler read, from its dep-info) and reports each
//! `allow`, `warn` or `expect`, outside test code, that names a lint of `posture::DENIED_LINTS` or
//! any clippy group (`posture::CLIPPY_GROUPS`, since a group may hold one), and each lint
//! attribute it cannot read (`source::lint_attributes` says which).
//!
//! Test code may still lower them, as clippy's own `allow-*-in-tests` settings let tests unwrap
//! and panic: an item under `#[cfg(test)]` or `#[test]`, a module after `#![cfg(test)]`, and a
//! test module kept in a file of its own (`#[cfg(test)] mod tests;`), whose file this check finds
//! from the declaring file's place, or from its `#[path]`, as rustc does (the Rust Reference,
//! "Module source filenames" and "The `path` attribute",
//! <https://doc.rust-lang.org/reference/items/modules.html>). Modules declared inside such a file
//! are read as code that ships, which reports more, never less.
//!
//! What it guarantees: every such place is reported with its file and line, sorted; a file that
//! does not lex, or whose delimiters do not close, is reported too, so a limit of the reader is a
//! violation, never a pass. The lints and groups `geom-num`'s settings lower legitimately
//! (`arithmetic_side_effects` and the casts, each beside its stated bound; `pedantic` names such
//! as `many_single_char_names`) are neither denied by chapter 27 nor a group, so they are not
//! this rule's.
//!
//! Method: the token reader of `source`, after the Rust Reference's attribute grammar
//! (<https://doc.rust-lang.org/reference/attributes.html>) and its lint-check attributes
//! (<https://doc.rust-lang.org/reference/attributes/diagnostics.html#lint-check-attributes>).
//!
//! Cost: linear in the size of the crates' sources.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;

use crate::check::{Rule, Violation};
use crate::globals::{SourceFile, Sources, covered};
use crate::table::Table;

/// The files `file`'s test modules kept in files of their own may be in, each one path relative to
/// the workspace root: for `mod name;` the Reference's `name.rs` and `name/mod.rs`, beside the
/// declaring file when it is a crate root or a `mod.rs`, else in the folder named after it; for a
/// `#[path]`, that path from the declaring file's folder.
fn test_module_files(file: &SourceFile) -> Vec<Vec<String>> {
    let Some(lints) = &file.lints else {
        return Vec::new();
    };
    let Some((name, folder)) = file.path.split_last() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (module, path) in &lints.test_modules {
        if let Some(path) = path {
            let mut target = folder.to_vec();
            for part in path.split(['/', '\\']) {
                match part {
                    "" | "." => {}
                    ".." => {
                        target.pop();
                    }
                    part => target.push(part.to_owned()),
                }
            }
            found.push(target);
            continue;
        }
        let mut base = folder.to_vec();
        if !matches!(name.as_str(), "lib.rs" | "main.rs" | "mod.rs") {
            base.push(name.trim_end_matches(".rs").to_owned());
        }
        let mut flat = base.clone();
        flat.push(format!("{module}.rs"));
        let mut nested = base;
        nested.push(module.clone());
        nested.push("mod.rs".to_owned());
        found.push(flat);
        found.push(nested);
    }
    found
}

/// Checks the sources of every covered crate in `sources`; crates the table does not cover are not
/// looked at.
pub fn check(table: &Table<'_>, sources: &Sources) -> Vec<Violation> {
    let mut found = BTreeSet::new();
    let covered = covered(table);
    for (krate, files) in sources.iter().filter(|(k, _)| covered.contains(k.as_str())) {
        let test_files: BTreeSet<Vec<String>> = files.iter().flat_map(test_module_files).collect();
        for file in files.iter().filter(|f| !test_files.contains(&f.path)) {
            let path = file.path.join("/");
            let mut add = |detail: String| {
                found.insert(Violation {
                    krate: krate.clone(),
                    rule: Rule::LintOverride,
                    detail,
                });
            };
            match &file.lints {
                None => add(format!(
                    "{path} does not lex or its delimiters do not close, so its lint attributes \
                     cannot be read"
                )),
                Some(lints) if lints.test_file => {}
                Some(lints) => {
                    for o in &lints.overrides {
                        add(format!(
                            "{path}:{}: {}; outside #[cfg(test)] code a crate that ships keeps \
                             every lint chapters 21 and 27 deny (and every lint group) at its \
                             manifest's level: change the code, not the lint",
                            o.line, o.what
                        ));
                    }
                }
            }
        }
    }
    found.into_iter().collect()
}
