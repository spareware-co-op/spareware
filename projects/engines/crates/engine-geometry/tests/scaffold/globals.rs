//! The merge gate's ban on global state (piece G12): no crate that ships names a `static`, a
//! thread-local or a lazy global.
//!
//! What it does: for every crate of the table whose kind ships (a kernel crate, `geom-pool` or the
//! facade: the crates `lints::carries_the_posture` names) and that is in the workspace, reads every
//! Rust source file of the crate outside its `tests`, `benches` and `examples` folders, which hold
//! cargo's test, benchmark and example targets, not the crate's own code, and reports each place a
//! file names global state, by `source::global_state`. That folder scan is fast and needs no
//! build, so it runs on every pull request in the rules job; but rustc compiles more into a
//! library than the `.rs` files of its folder: a module `#[path]` takes from `tests/` or another
//! crate's folder, a file of any name `include!` pastes in, and code a build script writes into
//! `OUT_DIR`. So the gate's builds and clippy runs also read, from the compiler's dep-info, every
//! file it read for each library (`gate::compiled_sources`), and hold those to the same rule
//! (`merge` joins them to the folder scan).
//!
//! The rule, decided from the chapters: chapter 28.8 asks for "a check that rejects statics,
//! thread-locals and lazy globals in kernel crates (principle 7)", and 28.1 calls it "a ban on
//! statics and thread-locals". Neither chapter 21 nor chapter 27 allows any static, not even an
//! immutable table, so none is allowed: a constant table is a `const`, which holds no state, and a
//! reference to it is still a reference into read-only memory (the Rust Reference, "Constant
//! promotion", <https://doc.rust-lang.org/reference/destructors.html#constant-promotion>). The
//! ban covers every source file of the crate, its `#[cfg(test)]` modules and any build script,
//! because a static there is still a static in a build of the crate. It covers the facade and
//! `geom-pool` as well as the `no_std` crates, as the lint posture does (`lints`): both ship, and
//! principle 7's "no global state, no session singletons" is about what ships. The development
//! crates and the binding crates are not covered, for the reasons `lints` gives.
//!
//! What it guarantees: every place is reported, with its file and line, sorted; a crate none of
//! whose sources could be read, or one of whose sources does not lex, is reported too, so a limit
//! of the reader is a violation, never a pass (a data file a crate pastes in with
//! `include_bytes!` is listed by the compiler too, and counts as not lexing unless it reads as Rust
//! tokens; no crate that ships pastes data in today). It reads text, so a static made by a macro
//! from another crate would pass it; kernel crates may name no crate but kernel crates and `libm`
//! (`check`, and the cargo-deny configurations in `projects/engines/geometry/`), so no such macro
//! can reach them; and a build script's text is read, not what it writes until the build runs,
//! which is why the compiled files are read too.
//!
//! Method: the token scan of `source::global_state` over every file; the file list of rustc's
//! dep-info (`--emit=dep-info`, which cargo asks for on every build) for what was compiled.
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

use std::collections::{BTreeMap, BTreeSet};

use crate::check::{Rule, Violation};
use crate::lints::carries_the_posture;
use crate::posture::{CLIPPY_GROUPS, DENIED_LINTS};
use crate::source::{GlobalUse, LintAttributes, global_state, lint_attributes};
use crate::table::Table;

/// The folders directly in a crate's folder that hold cargo's other targets, not the crate's own
/// code, and the build output folder, which the scan leaves out.
pub const NOT_CRATE_CODE: [&str; 4] = ["tests", "benches", "examples", "target"];

/// One Rust source file of a crate, as the gate's source rules read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// Its path relative to the workspace root, one component per entry; for a file outside the
    /// workspace root (code a build script wrote, read through the compiler's dep-info), its full
    /// path.
    pub path: Vec<String>,
    /// The places it names global state; `None` when it does not lex.
    pub uses: Option<Vec<GlobalUse>>,
    /// What its lint attributes say (`overrides`); `None` when it does not lex or its delimiters
    /// do not close.
    pub lints: Option<LintAttributes>,
}

impl SourceFile {
    /// Reads one file's text.
    pub fn read(path: Vec<String>, text: &str) -> Self {
        SourceFile {
            path,
            uses: global_state(text),
            lints: lint_attributes(text, &DENIED_LINTS, &CLIPPY_GROUPS),
        }
    }
}

/// Every crate's source files, by crate name.
pub type Sources = BTreeMap<String, Vec<SourceFile>>;

/// Joins `more` to `sources`: each file of a crate that `sources` does not already hold under the
/// same path.
pub fn merge(sources: &mut Sources, more: Sources) {
    for (krate, files) in more {
        let held = sources.entry(krate).or_default();
        for file in files {
            if !held.iter().any(|f| f.path == file.path) {
                held.push(file);
            }
        }
    }
}

/// The crates whose sources the ban covers: the table's rows whose kind ships.
pub fn covered<'t>(table: &'t Table<'t>) -> BTreeSet<&'t str> {
    table
        .rows
        .iter()
        .filter(|row| carries_the_posture(row.kind))
        .map(|row| row.name)
        .collect()
}

/// Checks the sources of every covered crate in `sources`; crates the table does not cover are
/// not looked at.
pub fn check(table: &Table<'_>, sources: &Sources) -> Vec<Violation> {
    let mut found = BTreeSet::new();
    let covered = covered(table);
    for (krate, files) in sources.iter().filter(|(k, _)| covered.contains(k.as_str())) {
        let mut add = |detail: String| {
            found.insert(Violation {
                krate: krate.clone(),
                rule: Rule::GlobalState,
                detail,
            });
        };
        if files.is_empty() {
            add("no Rust source of it was read, so it cannot be shown free of global state".into());
        }
        for file in files {
            let path = file.path.join("/");
            match &file.uses {
                None => add(format!(
                    "{path} does not lex, so it cannot be shown free of global state"
                )),
                Some(uses) => {
                    for used in uses {
                        add(format!(
                            "{path}:{}: {}; crates that ship hold no global state (principle 7, \
                             28.8): make a table a `const`, and pass state in as an argument",
                            used.line, used.what
                        ));
                    }
                }
            }
        }
    }
    found.into_iter().collect()
}
