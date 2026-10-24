//! The merge gate's dependency rules (piece G12): cargo-deny with chapter 29.2's licence
//! allow-list, security advisories and allowed sources over every crate of the kernel's family;
//! git dependencies refused; `libm` the only published crate in the `no_std` kernel crates, and
//! the one from crates.io; rayon only through `geom-pool` in what ships; and the gate's own rules
//! on the same graphs, where cargo-deny's leave a gap (chapter 28.8; 29.1, 29.2, 29.4; 29.20,
//! rule 3).
//!
//! What it does:
//!
//! - `family_config`, `no_std_config` and `ships_config` read the three configurations,
//!   `projects/engines/geometry/deny.toml`, `deny-no-std.toml` and `deny-ships.toml`, and report
//!   every way they say something other than the gate requires: an allow-list that is not exactly
//!   29.2's, an exception or clarification that widens it, development or build dependencies left
//!   out, an advisory ignored or downgraded, a registry or git source allowed, a graph narrowed,
//!   a `no_std` allow-list other than `libm`, or the rayon bans not exactly 29.4's. Fast and
//!   hermetic.
//! - `run::cargo_deny` runs cargo-deny (`cargo deny`, with `--format json`, offline after
//!   `run::fetch` has fetched the advisory database and the crates) and reads its diagnostics and
//!   exit code, whose bits say which check failed (`ADVISORIES`, `BANS`, `LICENSES`, `SOURCES`;
//!   cargo-deny's documentation, "Exit Codes").
//! - `rules` holds the gate's own rules, run on the same graph as each cargo-deny run, its
//!   refusals reported under the check they belong to. cargo-deny 0.20.2 leaves three gaps they
//!   close. Its `private.ignore`, which `deny.toml` sets so our own unpublished crates (the
//!   facade and `kit-core` have no licence field) pass, skips the licence of every crate whose
//!   manifest says `publish = false` (`Krate::is_private` in `src/lib.rs` reads `publish` only), a
//!   crate from crates.io or from a folder outside the workspace included: the rules refuse every
//!   such crate that is not of this workspace, and read our own crates' licence fields themselves
//!   (29.2's identifiers only, and a crate of the family's naming MPL-2.0, Spare Kit's own licence,
//!   beside any ported file's, as rule 4 asks). Its bans match crate names only and its sources
//!   check never reads a path crate: the rules refuse every crate that is neither of this
//!   workspace nor from crates.io (a `[patch]` to a local folder included), every crate outside
//!   the workspace that takes the name of a crate of the family (`geom-pool`, through which rayon
//!   may enter), and, in a `no_std` kernel crate, a `libm` that does not come from crates.io. And
//!   it accepts a licence that offers a choice without saying which was taken: the rules require
//!   every such crate's choice in `licence::LICENCE_CHOICES` (29.20, rule 3: "record the choice
//!   in the gate"), and `run::every_crate` refuses a row no graph needs.
//! - `run::every_crate` is the gate: for every crate of the table that is in the workspace, one
//!   run of every check with `deny.toml`, that crate's manifest as the only root of the graph; for
//!   every crate that ships (kernel, `geom-pool`, the facade, a binding) one run of the bans check
//!   with `deny-ships.toml`; and for every `no_std` kernel crate one run of the bans check with
//!   `deny-no-std.toml`.
//! - `run::Graph` builds the graph cargo-deny would read for a crate (`cargo metadata` with every
//!   feature on, from `Cargo.lock`), adds a package and an edge to it or changes a declaration,
//!   and hands the copy to cargo-deny with `--metadata-path` and to the rules, so each rule is
//!   shown to fail on a seeded violation without changing the repository.
//!
//! What it guarantees: a run that does not end with exit code 0, or that the rules refuse, fails
//! the gate; a seeded run must fail its own check and name each seeded violation in an error (for
//! licences and sources, exactly the seeded crates that break the rule, and none that keep it).
//! The tests that start cargo-deny are ignored by a plain `cargo test`, which may run where
//! cargo-deny is not installed; the merge gate installs the pinned cargo-deny and runs them
//! (`--ignored gate_deny`).
//!
//! Method: cargo-deny 0.20.2 (MIT OR Apache-2.0,
//! <https://github.com/EmbarkStudios/cargo-deny/tree/bca0dde53651ee946720e4540b5ce2610bec8f06>),
//! as its documentation describes its configuration (`docs/src/checks/*/cfg.md`), its graph
//! (`--manifest-path` of a workspace member makes that crate the only root: `docs/src/cli/
//! common.md`) and its exit code (`docs/src/cli/check.md`), and as its source behaves where the
//! documentation is silent (`src/lib.rs`, `Krate::is_private`; `src/licenses.rs`, which skips a
//! private crate with `SkippedPrivateWorkspaceCrate`; read for behaviour only, nothing copied);
//! the seeded graph and the rules read `cargo metadata`'s documented format
//! (<https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>), walking the resolved graph
//! from the root as cargo-deny does (`gather_krates` in `src/cargo-deny/common.rs`, which ignores
//! development edges outside the workspace, or everywhere under `exclude-dev`): every kind of
//! edge, a development edge only from a workspace crate, none when the configuration excludes
//! them.
//!
//! Cost: the text checks are linear in the files. A cargo-deny run takes about two seconds offline;
//! the gate runs one per crate of the family, one per crate that ships and one per `no_std` kernel
//! crate, and the rules one linear walk of each graph.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The kernel lints deny native arithmetic in kernel code (Cargo.toml). This is test code: it
// counts runs, which tests may do with native arithmetic.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "test code; the rule is for kernel code"
)]

use std::collections::{BTreeMap, BTreeSet};

use crate::manifest::{array_items, settings, string_text, toml_keys};

/// The configuration for every crate of the kernel's family, relative to the workspace root.
pub const FAMILY_CONFIG: &str = "projects/engines/geometry/deny.toml";

/// The configuration for the `no_std` kernel crates' bans, relative to the workspace root.
pub const NO_STD_CONFIG: &str = "projects/engines/geometry/deny-no-std.toml";

/// The configuration for the bans on what the crates that ship build with, relative to the
/// workspace root.
pub const SHIPS_CONFIG: &str = "projects/engines/geometry/deny-ships.toml";

/// Chapter 29.2's allow-list, typed in again from the chapter
/// (`projects/engines/geometry/architecture/29-sources-licences.md`, section 29.2, "The
/// allow-list"): "These SPDX identifiers, and no others, may appear in the kernel workspace's
/// `Cargo.lock` or in a ported file."
pub const ALLOW_LIST_29_2: [&str; 15] = [
    "MIT",
    "MIT-0",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSL-1.0",
    "ISC",
    "Zlib",
    "0BSD",
    "Unlicense",
    "CC0-1.0",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "MPL-2.0",
];

/// crates.io's index, the one registry crates may come from.
pub const CRATES_IO_INDEX: &str = "https://github.com/rust-lang/crates.io-index";

/// The only published crate a `no_std` kernel crate may build with (29.1).
pub const NO_STD_ALLOWED: &str = "libm";

/// The rayon bans: each crate, and the only crates that may depend on it directly (29.4, 23.4).
pub const RAYON_BANS: [(&str, &[&str]); 3] = [
    ("rayon", &["geom-pool", "wasm-bindgen-rayon"]),
    ("wasm-bindgen-rayon", &["geom-pool"]),
    ("rayon-core", &["rayon"]),
];

/// The texts of an array of strings, in order; `None` when it is not one.
fn strings(value: &str) -> Option<Vec<String>> {
    array_items(value)?.iter().map(|i| string_text(i)).collect()
}

/// The text of the string at `key`, if it is one.
fn text_at(keys: &BTreeMap<String, String>, key: &str) -> Option<String> {
    keys.get(key).and_then(|v| string_text(v))
}

/// Reports, into `wrong`, each key of `keys` set other than `want` (as written, canonical).
fn require(
    keys: &BTreeMap<String, String>,
    wrong: &mut Vec<String>,
    key: &str,
    want: &str,
    why: &str,
) {
    match keys.get(key) {
        Some(got) if got == want => {}
        Some(got) => wrong.push(format!("{key} = {got}, not {want}: {why}")),
        None => wrong.push(format!("{key} is not set; it must be {want}: {why}")),
    }
}

/// Reports, into `wrong`, once each, every one of `prefixes` that some key is, or is a table or
/// array of tables of.
fn forbid_keys(
    keys: &BTreeMap<String, String>,
    wrong: &mut Vec<String>,
    prefixes: &[&str],
    why: &str,
) {
    for prefix in prefixes {
        let set = keys.keys().any(|key| {
            key == prefix
                || key.starts_with(&format!("{prefix}."))
                || key.starts_with(&format!("{prefix}[]"))
        });
        if set {
            wrong.push(format!("{prefix} is set: {why}"));
        }
    }
}

/// What is wrong with an array of strings at `key` that must hold exactly `want`, in words.
fn exact_list(
    keys: &BTreeMap<String, String>,
    wrong: &mut Vec<String>,
    key: &str,
    want: &[&str],
    why: &str,
) {
    let Some(list) = keys.get(key).and_then(|v| strings(v)) else {
        wrong.push(format!(
            "{key} is not a list of strings this check reads: {why}"
        ));
        return;
    };
    let mut seen = BTreeSet::new();
    for item in &list {
        if !seen.insert(item.as_str()) {
            wrong.push(format!("{key} lists {item} twice"));
        }
    }
    let want: BTreeSet<&str> = want.iter().copied().collect();
    for extra in seen.difference(&want) {
        wrong.push(format!("{key} lists {extra}, which it may not: {why}"));
    }
    for missing in want.difference(&seen) {
        wrong.push(format!("{key} lacks {missing}: {why}"));
    }
}

/// The graph keys that would leave crates, platforms or dependencies out of the graph.
const NARROWING_GRAPH_KEYS: [&str; 4] = [
    "graph.exclude",
    "graph.targets",
    "graph.exclude-unpublished",
    "graph.features",
];

/// Everything `deny.toml`, the family's configuration, says other than the gate requires, in words.
pub fn family_config(text: &str) -> Vec<String> {
    let Some(keys) = settings(text) else {
        return vec!["it is not TOML this check reads".to_owned()];
    };
    let mut wrong = Vec::new();
    family_graph_and_licences(&keys, &mut wrong);
    family_advisories_and_sources(&keys, &mut wrong);
    family_bans(&keys, &mut wrong);
    wrong
}

/// The graph: every feature, every platform, every crate and dependency kind. Licences: exactly
/// 29.2's list, for every kind of dependency, widened nowhere.
fn family_graph_and_licences(keys: &BTreeMap<String, String>, wrong: &mut Vec<String>) {
    require(
        keys,
        wrong,
        "graph.all-features",
        "true",
        "a feature's dependencies are dependencies",
    );
    forbid_keys(
        keys,
        wrong,
        &NARROWING_GRAPH_KEYS,
        "the graph must hold every platform's dependencies of every crate (29.20, rule 3)",
    );
    forbid_keys(
        keys,
        wrong,
        &["graph.exclude-dev"],
        "development dependencies must pass the licence gate too (29.20, rule 3)",
    );
    exact_list(
        keys,
        wrong,
        "licenses.allow",
        &ALLOW_LIST_29_2,
        "the allow-list is exactly chapter 29.2's; adding an identifier needs a row in 29.2 and a \
         kernel owner's yes",
    );
    require(
        keys,
        wrong,
        "licenses.include-dev",
        "true",
        "29.20, rule 3: dev-dependencies too",
    );
    if keys
        .get("licenses.include-build")
        .is_some_and(|v| v != "true")
    {
        wrong.push(
            "licenses.include-build is not true: build dependencies must pass too".to_owned(),
        );
    }
    forbid_keys(
        keys,
        wrong,
        &[
            "licenses.exceptions",
            "licenses.clarify",
            "licenses.private.registries",
            "licenses.private.ignore-sources",
        ],
        "it widens 29.2's allow-list for some crates, which needs a decision note an owner accepts \
         (29.20)",
    );
    if let Some(threshold) = keys.get("licenses.confidence-threshold") {
        let ok = threshold
            .parse::<f64>()
            .is_ok_and(|t| (0.8..=1.0).contains(&t));
        if !ok {
            wrong.push(format!(
                "licenses.confidence-threshold = {threshold}: not a number from cargo-deny's \
                 default 0.8 to 1.0, so a licence text could be matched loosely"
            ));
        }
    }
}

/// Advisories: every kind is an error, none ignored. Sources: crates.io only; no git at all.
fn family_advisories_and_sources(keys: &BTreeMap<String, String>, wrong: &mut Vec<String>) {
    for (key, want, why) in [
        (
            "advisories.yanked",
            "\"deny\"",
            "a yanked crate fails the gate",
        ),
        (
            "advisories.unmaintained",
            "\"all\"",
            "an unmaintained crate anywhere fails",
        ),
        (
            "advisories.unsound",
            "\"all\"",
            "an unsound crate anywhere fails",
        ),
        (
            "sources.unknown-registry",
            "\"deny\"",
            "crates come from crates.io only",
        ),
        (
            "sources.unknown-git",
            "\"deny\"",
            "git dependencies are refused",
        ),
    ] {
        require(keys, wrong, key, want, why);
    }
    forbid_keys(
        keys,
        wrong,
        &[
            "advisories.ignore",
            "advisories.disable-yank-checking",
            "advisories.db-urls",
            "advisories.db-path",
        ],
        "the gate checks every advisory of the RustSec database; an exception needs an owner's yes",
    );
    exact_list(
        keys,
        wrong,
        "sources.allow-registry",
        &[CRATES_IO_INDEX],
        "crates come from crates.io only",
    );
    if keys
        .get("sources.allow-git")
        .and_then(|v| strings(v))
        .is_none_or(|l| !l.is_empty())
    {
        wrong.push(
            "sources.allow-git must be an empty list: git dependencies are refused".to_owned(),
        );
    }
    forbid_keys(
        keys,
        wrong,
        &["sources.private", "sources.allow-org"],
        "it allows git repositories, and git dependencies are refused",
    );
}

/// Bans: no wildcard versions. Which crates may enter what ships is `deny-ships.toml`'s and
/// `deny-no-std.toml`'s; the development crates, and every crate's development dependencies, may
/// use any crate whose licence and source pass (29.1: "Outside the kernel ... a crate may be used
/// as long as nothing it computes feeds a kernel result"; 29.16 allows criterion, which brings
/// rayon).
fn family_bans(keys: &BTreeMap<String, String>, wrong: &mut Vec<String>) {
    require(
        keys,
        wrong,
        "bans.wildcards",
        "\"deny\"",
        "a registry dependency names its version",
    );
}

/// What a `bans.deny` list says about the rayon bans other than 29.4's, into `wrong`.
fn rayon_bans(keys: &BTreeMap<String, String>, wrong: &mut Vec<String>) {
    match keys.get("bans.deny").map(|v| bans(v)) {
        None => wrong.push("bans.deny is not set: the rayon bans are missing".to_owned()),
        Some(None) => wrong.push("bans.deny is not a list this check reads".to_owned()),
        Some(Some(found)) => {
            for (krate, wrappers) in RAYON_BANS {
                let want: BTreeSet<String> = wrappers.iter().map(|w| (*w).to_owned()).collect();
                match found.get(krate) {
                    Some(got) if *got == want => {}
                    Some(got) => wrong.push(format!(
                        "bans.deny lets {got:?} use {krate} directly, not exactly {want:?} (29.4)"
                    )),
                    None => wrong.push(format!(
                        "bans.deny does not ban {krate}, which only {want:?} may use (29.4)"
                    )),
                }
            }
        }
    }
}

/// Everything `deny-ships.toml`, the configuration of what the crates that ship build with, says
/// other than the gate requires, in words: every feature, every platform, normal and build
/// dependencies only, and exactly 29.4's rayon bans.
pub fn ships_config(text: &str) -> Vec<String> {
    let Some(keys) = settings(text) else {
        return vec!["it is not TOML this check reads".to_owned()];
    };
    let mut wrong = Vec::new();
    require(
        &keys,
        &mut wrong,
        "graph.all-features",
        "true",
        "a feature's dependencies are dependencies",
    );
    require(
        &keys,
        &mut wrong,
        "graph.exclude-dev",
        "true",
        "what ships is normal and build dependencies",
    );
    forbid_keys(
        &keys,
        &mut wrong,
        &NARROWING_GRAPH_KEYS,
        "the graph must hold every platform's dependencies of the crate",
    );
    rayon_bans(&keys, &mut wrong);
    forbid_keys(
        &keys,
        &mut wrong,
        &["bans.allow", "bans.skip", "bans.skip-tree"],
        "the rayon bans are the whole of this file",
    );
    wrong
}

/// The crates a `bans.deny` list bans, each with its wrappers; `None` when it cannot be read.
fn bans(value: &str) -> Option<BTreeMap<String, BTreeSet<String>>> {
    let mut found = BTreeMap::new();
    for item in array_items(value)? {
        let (krate, wrappers) = if item.starts_with('{') {
            let keys = toml_keys(&format!("item = {item}\n"))?;
            let krate = text_at(&keys, "item.crate").or_else(|| text_at(&keys, "item.name"))?;
            let wrappers = match keys.get("item.wrappers") {
                Some(list) => strings(list)?,
                None => Vec::new(),
            };
            (krate, wrappers)
        } else {
            (string_text(&item)?, Vec::new())
        };
        found.insert(krate, wrappers.into_iter().collect());
    }
    Some(found)
}

/// Everything `deny-no-std.toml`, the `no_std` kernel crates' configuration, says other than the
/// gate requires, in words.
pub fn no_std_config(text: &str) -> Vec<String> {
    let Some(keys) = settings(text) else {
        return vec!["it is not TOML this check reads".to_owned()];
    };
    let mut wrong = Vec::new();
    require(
        &keys,
        &mut wrong,
        "graph.all-features",
        "true",
        "a feature's dependencies are dependencies",
    );
    require(
        &keys,
        &mut wrong,
        "graph.exclude-dev",
        "true",
        "what ships is normal and build dependencies",
    );
    forbid_keys(
        &keys,
        &mut wrong,
        &NARROWING_GRAPH_KEYS,
        "the graph must hold every platform's dependencies of the crate",
    );
    exact_list(
        &keys,
        &mut wrong,
        "bans.allow",
        &[NO_STD_ALLOWED],
        "inside kernel crates the only published crate is libm (29.1)",
    );
    require(
        &keys,
        &mut wrong,
        "bans.allow-workspace",
        "true",
        "our own crates are allowed; which of them a kernel crate names is the crate table's rule",
    );
    require(
        &keys,
        &mut wrong,
        "bans.wildcards",
        "\"deny\"",
        "a registry dependency names its version",
    );
    wrong
}

/// Running cargo-deny: the gate's runs over the repository and the seeded runs. Native only, as
/// they start processes.
#[cfg(not(target_family = "wasm"))]
pub mod run {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::rules::{self, Choice, Refusal};
    use super::{FAMILY_CONFIG, NO_STD_CONFIG, SHIPS_CONFIG};
    use crate::check::Observed;
    use crate::json::{self, Json};
    use crate::licence::LICENCE_CHOICES;
    use crate::repo;
    use crate::table::{Kind, TABLE, Table};

    /// The bit of cargo-deny's exit code set when the advisories check fails.
    pub const ADVISORIES: i32 = 0x1;
    /// The bit set when the bans check fails.
    pub const BANS: i32 = 0x2;
    /// The bit set when the licences check fails.
    pub const LICENSES: i32 = 0x4;
    /// The bit set when the sources check fails.
    pub const SOURCES: i32 = 0x8;

    /// One diagnostic cargo-deny wrote.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Diagnostic {
        /// Its code, such as `rejected` or `banned`.
        pub code: String,
        /// `error`, `warning`, `note` or `help`.
        pub severity: String,
        /// Its message.
        pub message: String,
        /// The crates its graphs start from.
        pub krates: Vec<String>,
    }

    /// What one cargo-deny run gave, with the gate's own rules on the same graph.
    #[derive(Clone, Debug)]
    pub struct Outcome {
        /// Its command line, for messages.
        pub command: String,
        /// cargo-deny's exit code; `None` when it was stopped by a signal.
        pub code: Option<i32>,
        /// Every diagnostic it wrote, and one error for each refusal of the gate's rules (its
        /// code starting `gate-`).
        pub diagnostics: Vec<Diagnostic>,
        /// Every line of its output that is not a diagnostic, kept for messages.
        pub other: Vec<String>,
        /// The checks (exit code bits) the gate's rules refused something under.
        pub gate_bits: i32,
    }

    impl Outcome {
        /// Whether the check with exit code bit `bit` failed: cargo-deny said so, or the gate's
        /// rules refused something under it. A run that did not end with a bitset of failed checks
        /// counts as failing every check.
        pub fn failed(&self, bit: i32) -> bool {
            self.gate_bits & bit != 0 || self.code.is_none_or(|code| code & bit != 0 || code > 0xf)
        }

        /// Whether the run passed: cargo-deny exited with 0 and the gate's rules refused nothing.
        pub fn passed(&self) -> bool {
            self.code == Some(0) && self.gate_bits == 0
        }

        /// The outcome with the refusals of the gate's rules added.
        fn with(mut self, refusals: Vec<Refusal>) -> Self {
            for r in refusals {
                self.gate_bits |= r.bit;
                self.diagnostics.push(Diagnostic {
                    code: r.code.to_owned(),
                    severity: "error".to_owned(),
                    message: r.message,
                    krates: vec![r.krate],
                });
            }
            self
        }

        /// The crates named by its errors.
        pub fn refused(&self) -> BTreeSet<String> {
            self.diagnostics
                .iter()
                .filter(|d| d.severity == "error")
                .flat_map(|d| d.krates.iter().cloned())
                .collect()
        }

        /// The codes of its errors that name `krate`.
        pub fn codes_for(&self, krate: &str) -> BTreeSet<String> {
            self.diagnostics
                .iter()
                .filter(|d| d.severity == "error" && d.krates.iter().any(|k| k == krate))
                .map(|d| d.code.clone())
                .collect()
        }

        /// The run, for a message: its command, exit code, errors and other output.
        pub fn show(&self) -> String {
            use std::fmt::Write as _;
            let mut text = format!(
                "{}\nexit code {:?}, gate rules' bits {:#x}",
                self.command, self.code, self.gate_bits
            );
            // Writing to a String cannot fail.
            for d in &self.diagnostics {
                let krates = d.krates.join(", ");
                write!(
                    text,
                    "\n  {}[{}] {krates}: {}",
                    d.severity, d.code, d.message
                )
                .unwrap();
            }
            for line in &self.other {
                write!(text, "\n  | {line}").unwrap();
            }
            text
        }
    }

    /// The folder the seeded graphs are written to.
    fn scratch() -> PathBuf {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("gate-deny");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Reads cargo-deny's `--format json` output: one JSON object per line.
    fn read_output(text: &str) -> (Vec<Diagnostic>, Vec<String>) {
        let mut diagnostics = Vec::new();
        let mut other = Vec::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let Ok(value) = json::parse(line) else {
                other.push(line.to_owned());
                continue;
            };
            let fields = value.get("fields");
            match value.get("type").and_then(Json::as_str) {
                Some("diagnostic") => {
                    let field = |key: &str| {
                        fields
                            .and_then(|f| f.get(key))
                            .and_then(Json::as_str)
                            .unwrap_or_default()
                            .to_owned()
                    };
                    let krates = fields
                        .and_then(|f| f.get("graphs"))
                        .and_then(Json::as_array)
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|g| g.get("Krate")?.get("name")?.as_str())
                        .map(str::to_owned)
                        .collect();
                    diagnostics.push(Diagnostic {
                        code: field("code"),
                        severity: field("severity"),
                        message: field("message"),
                        krates,
                    });
                }
                Some("summary") => {}
                _ => other.push(line.to_owned()),
            }
        }
        (diagnostics, other)
    }

    /// Runs `cargo deny` from the workspace root `root` with the configuration `config` (relative
    /// to the root), the crate whose manifest is `manifest` as the graph's root, and, if given,
    /// the graph `metadata` in place of the one cargo would give; `checks` are the checks to run.
    /// Offline: `fetch` has fetched the advisory database and the crates first.
    pub fn cargo_deny(
        root: &Path,
        manifest: &Path,
        config: &str,
        metadata: Option<&Path>,
        checks: &[&str],
    ) -> Outcome {
        let mut command = Command::new(repo::cargo());
        command
            .current_dir(root)
            .args([
                "deny",
                "--format",
                "json",
                "--color",
                "never",
                "--locked",
                "--offline",
            ])
            .arg("--manifest-path")
            .arg(manifest)
            .arg("--config")
            .arg(root.join(config));
        if let Some(metadata) = metadata {
            command.arg("--metadata-path").arg(metadata);
        }
        command.arg("check").args(checks);
        let shown = format!(
            "cargo deny --config {config} --manifest-path {}{} check {}",
            manifest.display(),
            metadata.map_or(String::new(), |m| format!(
                " --metadata-path {}",
                m.display()
            )),
            checks.join(" ")
        );
        let output = command.output().unwrap_or_else(|e| {
            panic!("cannot start `cargo deny` (is cargo-deny installed?): {e}")
        });
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let (diagnostics, other) = read_output(&text);
        Outcome {
            command: shown,
            code: output.status.code(),
            diagnostics,
            other,
            gate_bits: 0,
        }
    }

    /// The gate's check of one graph: cargo-deny's `checks` with `config`, on the graph `metadata`
    /// names (or, when `None`, the one cargo gives for `manifest`), and the gate's own rules on
    /// `graph`, the same graph read, with the record of licence choices `choices`.
    pub fn check_graph(
        root: &Path,
        manifest: &Path,
        config: &str,
        graph: &Json,
        metadata: Option<&Path>,
        checks: &[&str],
        choices: &[Choice],
    ) -> Outcome {
        let outcome = cargo_deny(root, manifest, config, metadata, checks);
        let krate = root_name(graph, manifest);
        let family: BTreeSet<&str> = TABLE.rows.iter().map(|r| r.name).collect();
        let refusals = rules::graph(graph, &krate, config, checks, &family, choices);
        outcome.with(refusals)
    }

    /// The name of the workspace crate whose manifest is `manifest`, in `graph`.
    fn root_name(graph: &Json, manifest: &Path) -> String {
        let canonical = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        graph
            .get("packages")
            .and_then(Json::as_array)
            .unwrap()
            .iter()
            .find(|p| {
                p.get("manifest_path")
                    .and_then(Json::as_str)
                    .is_some_and(|m| canonical(Path::new(m)) == canonical(manifest))
            })
            .and_then(|p| p.get("name").and_then(Json::as_str))
            .unwrap_or_else(|| panic!("no package of {} in the graph", manifest.display()))
            .to_owned()
    }

    /// The record of licence choices, as the rules read it.
    pub fn recorded_choices() -> Vec<Choice> {
        LICENCE_CHOICES
            .iter()
            .map(|(krate, expression, chosen)| Choice {
                krate: (*krate).to_owned(),
                expression: (*expression).to_owned(),
                chosen: (*chosen).to_owned(),
            })
            .collect()
    }

    /// Fetches what the offline runs read: the advisory database and every crate of `Cargo.lock`
    /// (`cargo deny fetch all`, online). Once per test process: the database sits behind one lock
    /// file in cargo's home, which tests running at once would otherwise contend for; and, when
    /// another process (another checkout's gate) holds that lock past cargo-deny's wait, up to
    /// three tries.
    pub fn fetch(root: &Path) {
        // Test code: the kernel's ban on global state is for the crates that ship.
        static FETCHED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        FETCHED.get_or_init(|| {
            let mut last = String::new();
            for _ in 0..3 {
                let output = Command::new(repo::cargo())
                    .current_dir(root)
                    .args(["deny", "--color", "never", "--locked", "--manifest-path"])
                    .arg(root.join("Cargo.toml"))
                    .arg("--config")
                    .arg(root.join(FAMILY_CONFIG))
                    .args(["fetch", "all"])
                    .output()
                    .unwrap_or_else(|e| {
                        panic!("cannot start `cargo deny` (is cargo-deny installed?): {e}")
                    });
                if output.status.success() {
                    return;
                }
                last = format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                if !last.contains("lock") {
                    break;
                }
            }
            panic!("cargo deny fetch all failed:\n{last}");
        });
    }

    /// The manifest of a crate `observed` holds.
    pub fn manifest_of(root: &Path, observed: &Observed, krate: &str) -> PathBuf {
        let package = observed
            .packages
            .get(krate)
            .unwrap_or_else(|| panic!("{krate} is not in the workspace"));
        package
            .dir
            .iter()
            .fold(root.to_path_buf(), |p, part| p.join(part))
            .join("Cargo.toml")
    }

    /// Whether a crate of this kind ships: its normal and build dependencies are in what is
    /// delivered, so rayon may enter them only through `geom-pool` (29.4).
    pub fn ships(kind: Kind) -> bool {
        !matches!(kind, Kind::Development)
    }

    /// The gate: every check with `deny.toml` for every crate of the table in the workspace, the
    /// bans check with `deny-ships.toml` for every crate that ships, and the bans check with
    /// `deny-no-std.toml` for every `no_std` kernel crate, each crate the only root of its graph,
    /// each run with the gate's own rules on the same graph; then every row of the record of
    /// licence choices that no graph needed. Returns every failed run, shown; prints one line per
    /// run.
    pub fn every_crate(table: &Table<'_>, observed: &Observed) -> Vec<String> {
        let root = repo::workspace_root();
        fetch(&root);
        let choices = recorded_choices();
        let mut failures = Vec::new();
        let mut needed = BTreeSet::new();
        let present = table
            .rows
            .iter()
            .filter(|row| observed.packages.contains_key(row.name));
        let mut runs = 0usize;
        for row in present {
            let manifest = manifest_of(&root, observed, row.name);
            let graph = Graph::of(row.name);
            needed.extend(rules::choices_needed(&graph.json, row.name));
            let mut configs = vec![(
                FAMILY_CONFIG,
                vec!["advisories", "bans", "licenses", "sources"],
            )];
            if ships(row.kind) {
                configs.push((SHIPS_CONFIG, vec!["bans"]));
            }
            if row.kind == Kind::Kernel {
                configs.push((NO_STD_CONFIG, vec!["bans"]));
            }
            for (config, checks) in configs {
                let outcome = check_graph(
                    &root,
                    &manifest,
                    config,
                    &graph.json,
                    None,
                    &checks,
                    &choices,
                );
                runs += 1;
                let ok = outcome.passed();
                println!(
                    "{} {}: {}",
                    if ok { "ok  " } else { "FAIL" },
                    row.name,
                    outcome.command
                );
                if !ok {
                    failures.push(outcome.show());
                }
            }
        }
        assert!(
            runs > 0,
            "no crate of the table is in the workspace: the gate checked nothing"
        );
        failures.extend(rules::unneeded_choices(&choices, &needed));
        failures
    }

    /// How a seeded crate is depended on.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Edge {
        /// `[dependencies]`.
        Normal,
        /// `[dev-dependencies]`.
        Dev,
        /// `[build-dependencies]`.
        Build,
    }

    /// A crate to seed into a graph.
    #[derive(Clone, Copy, Debug)]
    pub struct Seed<'a> {
        /// Its name.
        pub name: &'a str,
        /// Its version.
        pub version: &'a str,
        /// Its source as `cargo metadata` writes it; `None` for a path crate outside the
        /// workspace.
        pub source: Option<&'a str>,
        /// Its licence expression, if its manifest has one.
        pub license: Option<&'a str>,
    }

    /// crates.io's source, as `cargo metadata` writes it.
    pub const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

    /// A seed from crates.io with a licence.
    pub fn published<'a>(name: &'a str, version: &'a str, license: &'a str) -> Seed<'a> {
        Seed {
            name,
            version,
            source: Some(CRATES_IO),
            license: Some(license),
        }
    }

    /// A JSON string.
    fn text(t: &str) -> Json {
        Json::Str(t.to_owned())
    }

    /// A JSON string, or `null`.
    fn or_null(t: Option<&str>) -> Json {
        t.map_or(Json::Null, text)
    }

    /// A JSON object of `entries`.
    fn object(entries: Vec<(&str, Json)>) -> Json {
        Json::Object(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
        )
    }

    /// A seeded package's id, in `cargo metadata`'s form: its source without a commit, then its
    /// name and version.
    fn seeded_id(seed: &Seed<'_>) -> String {
        match seed.source {
            Some(source) => format!(
                "{}#{}@{}",
                source.split('#').next().unwrap_or(source),
                seed.name,
                seed.version
            ),
            None => format!("path+file:///seeded/{0}#{0}@{1}", seed.name, seed.version),
        }
    }

    /// A seeded package as `cargo metadata` lists one: its folder under the test's temporary
    /// folder, which holds no file, so nothing but its manifest's fields speaks for it.
    fn seeded_package(seed: &Seed<'_>, id: &str) -> Json {
        let folder = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("gate-deny")
            .join("seeded")
            .join(seed.name);
        let path = |p: PathBuf| text(&p.display().to_string());
        let target = object(vec![
            ("kind", Json::Array(vec![text("lib")])),
            ("crate_types", Json::Array(vec![text("lib")])),
            ("name", text(&seed.name.replace('-', "_"))),
            ("src_path", path(folder.join("src").join("lib.rs"))),
            ("edition", text("2021")),
            ("doc", Json::Bool(true)),
            ("doctest", Json::Bool(true)),
            ("test", Json::Bool(true)),
        ]);
        let mut entries = vec![
            ("name", text(seed.name)),
            ("version", text(seed.version)),
            ("id", text(id)),
            ("license", or_null(seed.license)),
            ("source", or_null(seed.source)),
            ("dependencies", Json::Array(Vec::new())),
            ("targets", Json::Array(vec![target])),
            ("features", Json::Object(BTreeMap::new())),
            ("manifest_path", path(folder.join("Cargo.toml"))),
            ("edition", text("2021")),
        ];
        for empty in ["authors", "categories", "keywords"] {
            entries.push((empty, Json::Array(Vec::new())));
        }
        for null in [
            "license_file",
            "description",
            "metadata",
            "publish",
            "readme",
            "repository",
            "homepage",
            "documentation",
            "links",
            "default_run",
            "rust_version",
        ] {
            entries.push((null, Json::Null));
        }
        object(entries)
    }

    /// A graph as `cargo metadata` gives it for one crate, to seed and hand to cargo-deny and the
    /// gate's rules.
    pub struct Graph {
        /// The workspace root.
        pub root: PathBuf,
        /// The crate's manifest, the root of the graph.
        pub manifest: PathBuf,
        /// The metadata.
        pub json: Json,
        /// The record of licence choices the rules read: the gate's, unless a test changes it.
        pub choices: Vec<Choice>,
    }

    impl Graph {
        /// The graph of the workspace crate `krate`, every feature on, from `Cargo.lock`: the
        /// graph cargo-deny itself reads with the gate's configurations.
        pub fn of(krate: &str) -> Graph {
            let root = repo::workspace_root();
            let observed = repo::observe();
            let manifest = manifest_of(&root, &observed, krate);
            let output = Command::new(repo::cargo())
                .args([
                    "metadata",
                    "--format-version",
                    "1",
                    "--locked",
                    "--all-features",
                ])
                .arg("--manifest-path")
                .arg(&manifest)
                .output()
                .expect("cannot run cargo metadata");
            assert!(
                output.status.success(),
                "cargo metadata failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let text = String::from_utf8(output.stdout).expect("cargo metadata wrote non-UTF-8");
            let json = json::parse(&text).expect("cargo metadata wrote invalid JSON");
            Graph {
                root,
                manifest,
                json,
                choices: recorded_choices(),
            }
        }

        /// The id of the first package named `name`.
        fn id_of(&self, name: &str) -> String {
            self.json
                .get("packages")
                .and_then(Json::as_array)
                .unwrap()
                .iter()
                .find(|p| p.get("name").and_then(Json::as_str) == Some(name))
                .and_then(|p| p.get("id")?.as_str())
                .unwrap_or_else(|| panic!("no package {name} in the graph"))
                .to_owned()
        }

        /// The graph's packages, to change.
        fn packages_mut(&mut self) -> &mut Vec<Json> {
            self.json
                .get_mut("packages")
                .and_then(Json::as_array_mut)
                .unwrap()
        }

        /// The resolved graph's nodes, to change.
        fn nodes_mut(&mut self) -> &mut Vec<Json> {
            self.json
                .get_mut("resolve")
                .and_then(|r| r.get_mut("nodes"))
                .and_then(Json::as_array_mut)
                .unwrap()
        }

        /// Adds `seed` as a dependency of the package named `parent`, of kind `edge`, with the
        /// version requirement `req`; the package itself once, however many crates depend on it.
        pub fn add(&mut self, parent: &str, edge: Edge, req: &str, seed: Seed<'_>) -> &mut Self {
            let id = seeded_id(&seed);
            let parent_id = self.id_of(parent);
            let kind = match edge {
                Edge::Normal => Json::Null,
                Edge::Dev => text("dev"),
                Edge::Build => text("build"),
            };
            let has = |items: &[Json]| {
                items
                    .iter()
                    .any(|p| p.get("id").and_then(Json::as_str) == Some(id.as_str()))
            };
            if !has(self.packages_mut()) {
                self.packages_mut().push(seeded_package(&seed, &id));
                self.nodes_mut().push(object(vec![
                    ("id", text(&id)),
                    ("dependencies", Json::Array(Vec::new())),
                    ("deps", Json::Array(Vec::new())),
                    ("features", Json::Array(Vec::new())),
                ]));
            }
            let with_parent_id =
                |p: &&mut Json| p.get("id").and_then(Json::as_str) == Some(parent_id.as_str());
            // The parent's declaration, as its manifest would give it.
            self.packages_mut()
                .iter_mut()
                .find(with_parent_id)
                .and_then(|p| p.get_mut("dependencies"))
                .and_then(Json::as_array_mut)
                .unwrap()
                .push(object(vec![
                    ("name", text(seed.name)),
                    ("source", or_null(seed.source)),
                    ("req", text(req)),
                    ("kind", kind.clone()),
                    ("rename", Json::Null),
                    ("optional", Json::Bool(false)),
                    ("uses_default_features", Json::Bool(true)),
                    ("features", Json::Array(Vec::new())),
                    ("target", Json::Null),
                    ("registry", Json::Null),
                ]));
            // The resolved edge.
            let node = self.nodes_mut().iter_mut().find(with_parent_id).unwrap();
            node.get_mut("dependencies")
                .and_then(Json::as_array_mut)
                .unwrap()
                .push(text(&id));
            node.get_mut("deps")
                .and_then(Json::as_array_mut)
                .unwrap()
                .push(object(vec![
                    ("name", text(&seed.name.replace('-', "_"))),
                    ("pkg", text(&id)),
                    (
                        "dep_kinds",
                        Json::Array(vec![object(vec![("kind", kind), ("target", Json::Null)])]),
                    ),
                ]));
            self
        }

        /// Changes the version requirement with which the package named `package` declares its
        /// dependency `dep`, as if its manifest said `req`. cargo-deny reads a requirement from
        /// the graph and finds where a manifest declares it, so only a declaration the manifest
        /// really has can be changed this way.
        pub fn set_req(&mut self, package: &str, dep: &str, req: &str) -> &mut Self {
            let declaration = self
                .json
                .get_mut("packages")
                .and_then(Json::as_array_mut)
                .unwrap()
                .iter_mut()
                .find(|p| p.get("name").and_then(Json::as_str) == Some(package))
                .and_then(|p| p.get_mut("dependencies"))
                .and_then(Json::as_array_mut)
                .unwrap()
                .iter_mut()
                .find(|d| d.get("name").and_then(Json::as_str) == Some(dep))
                .unwrap_or_else(|| panic!("{package} declares no {dep}"));
            *declaration.get_mut("req").unwrap() = Json::Str(req.to_owned());
            self
        }

        /// Runs cargo-deny's `checks` with `config` on this graph, written to a file named after
        /// `name`, and the gate's rules on the same graph: the gate's check (`check_graph`).
        pub fn deny(&self, name: &str, config: &str, checks: &[&str]) -> Outcome {
            let path = scratch().join(format!("{name}.json"));
            fs::write(&path, self.json.to_text()).unwrap();
            check_graph(
                &self.root,
                &self.manifest,
                config,
                &self.json,
                Some(&path),
                checks,
                &self.choices,
            )
        }

        /// Sets the field `key` of the package named `name` to `value`, as if its manifest said so.
        pub fn set_field(&mut self, name: &str, key: &str, value: Json) -> &mut Self {
            let package = self
                .packages_mut()
                .iter_mut()
                .find(|p| p.get("name").and_then(Json::as_str) == Some(name))
                .unwrap_or_else(|| panic!("no package {name} in the graph"));
            *package
                .get_mut(key)
                .unwrap_or_else(|| panic!("{name} has no field {key}")) = value;
            self
        }
    }
}

/// The gate's own rules on a graph cargo-deny reads (see the module header): where cargo-deny's
/// licence, source and ban checks leave a gap. Native only, as the graph is `cargo metadata`'s
/// JSON.
#[cfg(not(target_family = "wasm"))]
pub mod rules {
    use std::collections::{BTreeMap, BTreeSet};

    use super::run::{BANS, LICENSES, SOURCES};
    use super::{NO_STD_ALLOWED, NO_STD_CONFIG, SHIPS_CONFIG};
    use crate::json::Json;
    use crate::licence::{Way, allowed, chosen_way, ways};

    /// The sources `cargo metadata` gives crates.io, by either protocol.
    pub const CRATES_IO_SOURCES: [&str; 2] = [
        "registry+https://github.com/rust-lang/crates.io-index",
        "sparse+https://index.crates.io/",
    ];

    /// One row of the record of licence choices.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Choice {
        /// The crate.
        pub krate: String,
        /// Its licence expression, exactly as its manifest gives it.
        pub expression: String,
        /// The licence (or licences, joined by `AND`) taken.
        pub chosen: String,
    }

    /// One refusal of the gate's rules.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Refusal {
        /// The check it belongs to, as cargo-deny's exit code bit.
        pub bit: i32,
        /// The crate refused.
        pub krate: String,
        /// Its code, starting `gate-`.
        pub code: &'static str,
        /// What is wrong, and why.
        pub message: String,
    }

    /// A graph, read: its packages by id, its workspace's members, and the packages reached from
    /// its root.
    struct View<'a> {
        packages: BTreeMap<&'a str, &'a Json>,
        members: BTreeSet<&'a str>,
        reached: BTreeSet<&'a str>,
    }

    /// A package's text field `key`, or empty.
    fn field<'a>(package: &'a Json, key: &str) -> &'a str {
        package.get(key).and_then(Json::as_str).unwrap_or_default()
    }

    /// Reads `graph` from the workspace crate `root`, following development edges only out of a
    /// workspace crate, and none when `exclude_dev`; `None` when the graph cannot be read.
    fn view<'a>(graph: &'a Json, root: &str, exclude_dev: bool) -> Option<View<'a>> {
        let packages: BTreeMap<&str, &Json> = graph
            .get("packages")?
            .as_array()?
            .iter()
            .map(|p| Some((p.get("id")?.as_str()?, p)))
            .collect::<Option<_>>()?;
        let members: BTreeSet<&str> = graph
            .get("workspace_members")?
            .as_array()?
            .iter()
            .map(Json::as_str)
            .collect::<Option<_>>()?;
        let nodes: BTreeMap<&str, &Json> = graph
            .get("resolve")?
            .get("nodes")?
            .as_array()?
            .iter()
            .map(|n| Some((n.get("id")?.as_str()?, n)))
            .collect::<Option<_>>()?;
        let start = members
            .iter()
            .copied()
            .find(|id| packages.get(id).is_some_and(|p| field(p, "name") == root))?;
        let mut reached = BTreeSet::new();
        let mut work = vec![start];
        while let Some(id) = work.pop() {
            if !reached.insert(id) {
                continue;
            }
            for dep in nodes.get(id)?.get("deps")?.as_array()? {
                let kinds = dep.get("dep_kinds")?.as_array()?;
                let dev_only = kinds
                    .iter()
                    .all(|k| k.get("kind").and_then(Json::as_str) == Some("dev"));
                if dev_only && (exclude_dev || !members.contains(id)) {
                    continue;
                }
                work.push(dep.get("pkg")?.as_str()?);
            }
        }
        Some(View {
            packages,
            members,
            reached,
        })
    }

    /// The gate's rules on `graph`, whose root is the workspace crate `root`, for a run of
    /// `checks` with `config`; `family` names the crates of the kernel's family and `choices` is
    /// the record of licence choices.
    pub fn graph(
        graph: &Json,
        root: &str,
        config: &str,
        checks: &[&str],
        family: &BTreeSet<&str>,
        choices: &[Choice],
    ) -> Vec<Refusal> {
        let exclude_dev = config.ends_with(NO_STD_CONFIG) || config.ends_with(SHIPS_CONFIG);
        let Some(view) = view(graph, root, exclude_dev) else {
            return vec![Refusal {
                bit: LICENSES | SOURCES | BANS,
                krate: root.to_owned(),
                code: "gate-unreadable",
                message: "the graph cannot be read, so the gate's rules cannot pass it".to_owned(),
            }];
        };
        let mut found = Vec::new();
        if checks.contains(&"licenses") {
            licences(&view, family, choices, &mut found);
        }
        if checks.contains(&"sources") {
            sources(&view, family, &mut found);
        }
        if checks.contains(&"bans") && config.ends_with(NO_STD_CONFIG) {
            only_libm(&view, &mut found);
        }
        found
    }

    /// Whether a package comes from crates.io.
    fn from_crates_io(package: &Json) -> bool {
        CRATES_IO_SOURCES.contains(&field(package, "source"))
    }

    /// The licence rules: no crate outside the workspace skips the licence check as private; our
    /// own crates' licence fields hold 29.2's identifiers only, the family's naming MPL-2.0; and
    /// every choice a licence offers is recorded.
    fn licences(
        view: &View<'_>,
        family: &BTreeSet<&str>,
        choices: &[Choice],
        found: &mut Vec<Refusal>,
    ) {
        for id in &view.reached {
            let Some(package) = view.packages.get(id) else {
                continue;
            };
            let name = field(package, "name");
            let licence = package.get("license").and_then(Json::as_str);
            let mut refuse = |code: &'static str, message: String| {
                found.push(Refusal {
                    bit: LICENSES,
                    krate: name.to_owned(),
                    code,
                    message,
                });
            };
            let member = view.members.contains(id);
            let private = package
                .get("publish")
                .and_then(Json::as_array)
                .is_some_and(<[Json]>::is_empty);
            if private && !member {
                refuse(
                    "gate-private-outsider",
                    "its manifest says publish = false, so cargo-deny's private.ignore skips its \
                     licence, yet it is not a crate of this workspace: every crate in the graph \
                     must pass the licence gate (29.20, rule 3)"
                        .to_owned(),
                );
            }
            if member {
                // A workspace crate with no licence field is ours, under the repository's
                // LICENSE (MPL-2.0); one that names a licence names what it holds (rule 4).
                let Some(licence) = licence else {
                    continue;
                };
                let Some(ways) = ways(licence) else {
                    refuse(
                        "gate-own-licence",
                        format!("its licence field {licence:?} is not an SPDX expression"),
                    );
                    continue;
                };
                let in_family = family.contains(name);
                let bad = ways
                    .iter()
                    .any(|w| !allowed(w) || (in_family && !w.contains("MPL-2.0")));
                if bad {
                    refuse(
                        "gate-own-licence",
                        format!(
                            "its licence field {licence:?} names a licence 29.2 leaves out{}: a \
                             crate of ours names only 29.2's identifiers, and a crate of the \
                             kernel's family names MPL-2.0, Spare Kit's own licence, beside any \
                             ported file's (29.2; 29.20, rule 4)",
                            if in_family { ", or not MPL-2.0" } else { "" }
                        ),
                    );
                }
                continue;
            }
            if let Some(licence) = licence {
                recorded(name, licence, choices, &mut refuse);
            }
        }
    }

    /// The rule on a licence that offers a choice, into `refuse`: the record holds the crate under
    /// its exact expression, and its choice is one way, all of it on 29.2's list.
    fn recorded(
        name: &str,
        licence: &str,
        choices: &[Choice],
        refuse: &mut impl FnMut(&'static str, String),
    ) {
        let Some(ways) = ways(licence) else {
            // cargo-deny refuses what it cannot read; nothing to record.
            return;
        };
        if ways.len() < 2 {
            return;
        }
        let row = choices
            .iter()
            .find(|c| c.krate == name && c.expression == licence);
        let Some(row) = row else {
            refuse(
                "gate-choice-unrecorded",
                format!(
                    "its licence {licence:?} offers a choice, and the gate records none for it: \
                     add the row (\"{name}\", \"{licence}\", \"<the permissive way taken>\") to \
                     licence::LICENCE_CHOICES (29.20, rule 3)"
                ),
            );
            return;
        };
        let way: Option<Way> = chosen_way(&row.chosen);
        if !way.is_some_and(|w| ways.contains(&w) && allowed(&w)) {
            refuse(
                "gate-choice-wrong",
                format!(
                    "the choice recorded for its licence {licence:?}, {:?}, is not one of the \
                     ways it offers on 29.2's allow-list (29.20, rule 3)",
                    row.chosen
                ),
            );
        }
    }

    /// The source rules: every crate is of this workspace or from crates.io (a crate patched in
    /// from a folder is neither), and none outside the workspace takes the name of a crate of the
    /// family.
    fn sources(view: &View<'_>, family: &BTreeSet<&str>, found: &mut Vec<Refusal>) {
        for id in &view.reached {
            let Some(package) = view.packages.get(id) else {
                continue;
            };
            if view.members.contains(id) {
                continue;
            }
            let name = field(package, "name");
            if !from_crates_io(package) {
                found.push(Refusal {
                    bit: SOURCES,
                    krate: name.to_owned(),
                    code: "gate-not-crates-io",
                    message: format!(
                        "it comes from {}, which is neither this workspace nor crates.io: crates \
                         come from crates.io and nowhere else (29.1, 29.20)",
                        package.get("source").and_then(Json::as_str).map_or_else(
                            || field(package, "manifest_path").to_owned(),
                            str::to_owned
                        )
                    ),
                });
            }
            if family.contains(name) {
                found.push(Refusal {
                    bit: SOURCES,
                    krate: name.to_owned(),
                    code: "gate-family-name",
                    message: "it takes the name of a crate of the kernel's family but is not this \
                              workspace's: the bans that name family crates (rayon only through \
                              geom-pool) would trust it"
                        .to_owned(),
                });
            }
        }
    }

    /// The rule of a `no_std` kernel crate's graph: every crate outside the workspace is `libm`
    /// from crates.io (29.1, 29.4: the audited release `Cargo.lock` pins, not code patched in).
    fn only_libm(view: &View<'_>, found: &mut Vec<Refusal>) {
        for id in &view.reached {
            let Some(package) = view.packages.get(id) else {
                continue;
            };
            if view.members.contains(id) {
                continue;
            }
            let name = field(package, "name");
            if name != NO_STD_ALLOWED || !from_crates_io(package) {
                found.push(Refusal {
                    bit: BANS,
                    krate: name.to_owned(),
                    code: "gate-not-libm",
                    message: format!(
                        "inside kernel crates the only published crate is {NO_STD_ALLOWED}, from \
                         crates.io (29.1, 29.4); this is {name} from {}",
                        package
                            .get("source")
                            .and_then(Json::as_str)
                            .unwrap_or("a folder")
                    ),
                });
            }
        }
    }

    /// Every crate outside the workspace in the graph of `root` (with development edges, as the
    /// family's run reads it) whose licence offers a choice, with its expression.
    pub fn choices_needed(graph: &Json, root: &str) -> BTreeSet<(String, String)> {
        let Some(view) = view(graph, root, false) else {
            return BTreeSet::new();
        };
        view.reached
            .iter()
            .filter(|id| !view.members.contains(*id))
            .filter_map(|id| view.packages.get(id))
            .filter_map(|p| {
                let licence = p.get("license").and_then(Json::as_str)?;
                (ways(licence)?.len() > 1)
                    .then(|| (field(p, "name").to_owned(), licence.to_owned()))
            })
            .collect()
    }

    /// Every row of the record no graph needed, in words.
    pub fn unneeded_choices(
        choices: &[Choice],
        needed: &BTreeSet<(String, String)>,
    ) -> Vec<String> {
        choices
            .iter()
            .filter(|c| !needed.contains(&(c.krate.clone(), c.expression.clone())))
            .map(|c| {
                format!(
                    "licence::LICENCE_CHOICES records ({:?}, {:?}), which no crate of the kernel \
                     family's graphs has: delete the row, so the record says what is taken",
                    c.krate, c.expression
                )
            })
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::json;

        /// A small graph: the workspace crates `geom-a` (MPL-2.0) and `kit-x` (no licence), and
        /// the crates `extra` adds; edges from `geom-a` to every other package, of `kind` each.
        fn graph(extra: &[(&str, &str, &str, &str)], kinds: &[&str]) -> Json {
            // extra: (name, source or "", licence, publish: "null" or "[]")
            let mut packages = vec![
                "{\"id\":\"a\",\"name\":\"geom-a\",\"source\":null,\"license\":\"MPL-2.0\",\
                 \"publish\":[],\"manifest_path\":\"/w/a/Cargo.toml\"}"
                    .to_owned(),
                "{\"id\":\"k\",\"name\":\"kit-x\",\"source\":null,\"license\":null,\
                 \"publish\":[],\"manifest_path\":\"/w/k/Cargo.toml\"}"
                    .to_owned(),
            ];
            let mut deps = vec!["{\"pkg\":\"k\",\"dep_kinds\":[{\"kind\":null}]}".to_owned()];
            let mut nodes = vec!["{\"id\":\"k\",\"deps\":[]}".to_owned()];
            for (i, (name, source, licence, publish)) in extra.iter().enumerate() {
                let source = if source.is_empty() {
                    "null".to_owned()
                } else {
                    format!("\"{source}\"")
                };
                packages.push(format!(
                    "{{\"id\":\"x{i}\",\"name\":\"{name}\",\"source\":{source},\
                     \"license\":\"{licence}\",\"publish\":{publish},\
                     \"manifest_path\":\"/o/{name}/Cargo.toml\"}}"
                ));
                let kind = kinds.get(i).copied().unwrap_or("null");
                deps.push(format!(
                    "{{\"pkg\":\"x{i}\",\"dep_kinds\":[{{\"kind\":{kind}}}]}}"
                ));
                nodes.push(format!("{{\"id\":\"x{i}\",\"deps\":[]}}"));
            }
            nodes.push(format!("{{\"id\":\"a\",\"deps\":[{}]}}", deps.join(",")));
            json::parse(&format!(
                "{{\"packages\":[{}],\"workspace_members\":[\"a\",\"k\"],\
                 \"resolve\":{{\"nodes\":[{}]}}}}",
                packages.join(","),
                nodes.join(",")
            ))
            .unwrap()
        }

        const IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

        fn codes(found: &[Refusal]) -> Vec<(String, &'static str)> {
            let mut codes: Vec<(String, &'static str)> =
                found.iter().map(|r| (r.krate.clone(), r.code)).collect();
            codes.sort();
            codes
        }

        fn run(g: &Json, config: &str, checks: &[&str], choices: &[Choice]) -> Vec<Refusal> {
            let family = BTreeSet::from(["geom-a", "geom-pool"]);
            graph_rules(g, config, checks, &family, choices)
        }

        fn graph_rules(
            g: &Json,
            config: &str,
            checks: &[&str],
            family: &BTreeSet<&str>,
            choices: &[Choice],
        ) -> Vec<Refusal> {
            super::graph(g, "geom-a", config, checks, family, choices)
        }

        #[test]
        fn a_clean_graph_passes_every_rule() {
            let g = graph(&[("libm", IO, "MIT", "null")], &[]);
            for (config, checks) in [
                ("deny.toml", &["licenses", "sources", "bans"][..]),
                (NO_STD_CONFIG, &["bans"][..]),
                (SHIPS_CONFIG, &["bans"][..]),
            ] {
                assert_eq!(run(&g, config, checks, &[]), Vec::new(), "{config}");
            }
        }

        #[test]
        fn each_rule_refuses_its_seeded_violation() {
            let choice = |chosen: &str| Choice {
                krate: "dual".to_owned(),
                expression: "MIT OR GPL-3.0-only".to_owned(),
                chosen: chosen.to_owned(),
            };
            let g = graph(
                &[
                    ("vendored", "", "AGPL-3.0-only", "[]"),
                    ("hidden", IO, "GPL-3.0-only", "[]"),
                    ("geom-pool", "", "MIT", "null"),
                    ("libm", "", "MIT", "null"),
                    ("dual", IO, "MIT OR GPL-3.0-only", "null"),
                ],
                &[],
            );
            assert_eq!(
                codes(&run(&g, "deny.toml", &["licenses", "sources"], &[])),
                [
                    ("dual".to_owned(), "gate-choice-unrecorded"),
                    ("geom-pool".to_owned(), "gate-family-name"),
                    ("geom-pool".to_owned(), "gate-not-crates-io"),
                    ("hidden".to_owned(), "gate-private-outsider"),
                    ("libm".to_owned(), "gate-not-crates-io"),
                    ("vendored".to_owned(), "gate-not-crates-io"),
                    ("vendored".to_owned(), "gate-private-outsider"),
                ]
            );
            assert_eq!(
                codes(&run(
                    &g,
                    "deny.toml",
                    &["licenses"],
                    &[choice("GPL-3.0-only")]
                ))
                .iter()
                .filter(|(k, _)| k == "dual")
                .count(),
                1
            );
            assert!(
                run(&g, "deny.toml", &["licenses"], &[choice("MIT")])
                    .iter()
                    .all(|r| r.krate != "dual")
            );
            let no_std = run(&g, NO_STD_CONFIG, &["bans"], &[]);
            assert!(no_std.iter().all(|r| r.code == "gate-not-libm"));
            assert_eq!(no_std.len(), 5, "{no_std:#?}");
        }

        #[test]
        fn our_own_licences_are_read() {
            // geom-a is of the family: GPL, or no MPL-2.0, is refused; kit-x is ours, outside it.
            let g = graph(&[], &[]);
            let set = |g: &mut Json, id: &str, licence: &str| {
                let packages = g.get_mut("packages").and_then(Json::as_array_mut).unwrap();
                let p = packages
                    .iter_mut()
                    .find(|p| p.get("id").and_then(Json::as_str) == Some(id))
                    .unwrap();
                *p.get_mut("license").unwrap() = Json::Str(licence.to_owned());
            };
            for (id, licence, refused) in [
                ("a", "GPL-3.0-only", true),
                ("a", "MIT", true),
                ("a", "MPL-2.0 OR GPL-3.0-only", true),
                ("a", "MPL-2.0 AND BSD-3-Clause", false),
                ("k", "Apache-2.0", false),
                ("k", "LGPL-2.1-only", true),
            ] {
                let mut g2 = g.clone();
                set(&mut g2, id, licence);
                let found = run(&g2, "deny.toml", &["licenses"], &[]);
                assert_eq!(!found.is_empty(), refused, "{id} {licence}: {found:#?}");
            }
            // A development edge out of a workspace crate is followed, except under exclude-dev.
            let dev = graph(&[("devonly", "", "GPL-3.0-only", "[]")], &["\"dev\""]);
            assert_ne!(run(&dev, "deny.toml", &["licenses"], &[]), Vec::new());
            assert_eq!(run(&dev, NO_STD_CONFIG, &["bans"], &[]), Vec::new());
            let normal = graph(&[("normal", "", "GPL-3.0-only", "[]")], &[]);
            assert_ne!(run(&normal, NO_STD_CONFIG, &["bans"], &[]), Vec::new());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A family configuration that holds every rule, laid out differently from the real file.
    const FAMILY: &str = "[graph]\nall-features = true\n\
        [licenses]\nallow = [\"MIT\", \"MIT-0\", \"Apache-2.0\", 'Apache-2.0 WITH LLVM-exception',\n\
        \"BSD-2-Clause\", \"BSD-3-Clause\", \"BSL-1.0\", \"ISC\", \"Zlib\", \"0BSD\", \"Unlicense\",\n\
        \"CC0-1.0\", \"Unicode-3.0\", \"Unicode-DFS-2016\", \"MPL-2.0\"]\n\
        include-dev = true\nconfidence-threshold = 0.93\nprivate = { ignore = true }\n\
        [advisories]\nyanked = \"deny\"\nunmaintained = \"all\"\nunsound = \"all\"\n\
        [sources]\nunknown-registry = \"deny\"\nunknown-git = \"deny\"\n\
        allow-registry = [\"https://github.com/rust-lang/crates.io-index\"]\nallow-git = []\n\
        [bans]\nwildcards = \"deny\"\nallow-wildcard-paths = true\n";

    const NO_STD: &str = "[graph]\nall-features = true\nexclude-dev = true\n\
        [bans]\nallow = [\"libm\"]\nallow-workspace = true\nwildcards = \"deny\"\n";

    /// A configuration of what ships that holds every rule, a ban written in each of
    /// cargo-deny's forms.
    const SHIPS: &str = "[graph]\nall-features = true\nexclude-dev = true\n\
        [bans]\ndeny = [\n\
        { crate = \"rayon\", wrappers = [\"wasm-bindgen-rayon\", \"geom-pool\"] },\n\
        { crate = \"wasm-bindgen-rayon\", wrappers = [\"geom-pool\"] },\n\
        { name = \"rayon-core\", wrappers = [\"rayon\"], reason = \"x\" },\n\
        \"openssl\",\n]\n";

    #[test]
    fn configurations_that_hold_every_rule_pass() {
        assert_eq!(family_config(FAMILY), Vec::<String>::new());
        assert_eq!(no_std_config(NO_STD), Vec::<String>::new());
        assert_eq!(ships_config(SHIPS), Vec::<String>::new());
    }

    #[test]
    fn every_seeded_change_to_the_ships_configuration_is_refused() {
        let swap = |from: &str, to: &str| {
            assert!(SHIPS.contains(from), "the example lacks {from:?}");
            SHIPS.replace(from, to)
        };
        let cases: Vec<(&str, String, &str)> = vec![
            (
                "a wrapper added",
                swap(
                    "[\"wasm-bindgen-rayon\", \"geom-pool\"]",
                    "[\"wasm-bindgen-rayon\", \"geom-pool\", \"geom-num\"]",
                ),
                "use rayon directly",
            ),
            (
                "a ban dropped",
                swap(
                    "{ crate = \"wasm-bindgen-rayon\", wrappers = [\"geom-pool\"] },\n",
                    "",
                ),
                "does not ban wasm-bindgen-rayon",
            ),
            (
                "development dependencies in",
                swap("exclude-dev = true", "exclude-dev = false"),
                "graph.exclude-dev",
            ),
            (
                "one platform",
                swap(
                    "exclude-dev = true",
                    "exclude-dev = true\ntargets = [\"wasm32-unknown-unknown\"]",
                ),
                "graph.targets",
            ),
            (
                "a crate skipped",
                swap("[bans]\n", "[bans]\nskip = [\"rayon\"]\n"),
                "bans.skip",
            ),
        ];
        for (name, text, needle) in &cases {
            exactly(name, &ships_config(text), &[needle]);
        }
    }

    /// Asserts `found` holds exactly one problem per needle, each naming its needle.
    fn exactly(name: &str, found: &[String], needles: &[&str]) {
        let named = needles
            .iter()
            .all(|needle| found.iter().any(|problem| problem.contains(needle)));
        assert!(
            found.len() == needles.len() && named,
            "{name}: expected one problem naming each of {needles:?}, found {found:#?}"
        );
    }

    /// `FAMILY` with `from` replaced by `to`; `from` must be in it.
    fn swap(from: &str, to: &str) -> String {
        assert!(FAMILY.contains(from), "the example lacks {from:?}");
        FAMILY.replace(from, to)
    }

    /// Changes to the family configuration, each with what the check must name.
    fn family_faults() -> Vec<(&'static str, String, Vec<&'static str>)> {
        let mut faults = licence_faults();
        faults.extend(advisory_source_and_ban_faults());
        faults
    }

    /// Changes to the graph and licence rules.
    fn licence_faults() -> Vec<(&'static str, String, Vec<&'static str>)> {
        vec![
            (
                "add GPL",
                swap("\"MPL-2.0\"]", "\"MPL-2.0\", \"GPL-3.0-only\"]"),
                vec!["lists GPL-3.0-only"],
            ),
            (
                "drop MPL",
                swap(", \"MPL-2.0\"]", "]"),
                vec!["lacks MPL-2.0"],
            ),
            (
                "twice",
                swap("\"ISC\",", "\"ISC\", \"ISC\","),
                vec!["lists ISC twice"],
            ),
            (
                "an exception's licence instead",
                swap(
                    "'Apache-2.0 WITH LLVM-exception'",
                    "'Apache-2.0 WITH Classpath-exception-2.0'",
                ),
                vec![
                    "lists Apache-2.0 WITH Classpath-exception-2.0",
                    "lacks Apache-2.0 WITH LLVM-exception",
                ],
            ),
            (
                "dev left out",
                swap("include-dev = true", "include-dev = false"),
                vec!["licenses.include-dev"],
            ),
            (
                "build left out",
                swap(
                    "include-dev = true",
                    "include-dev = true\ninclude-build = false",
                ),
                vec!["include-build"],
            ),
            (
                "exception",
                swap(
                    "private = ",
                    "exceptions = [{ crate = \"x\", allow = [\"GPL-3.0-only\"] }]\nprivate = ",
                ),
                vec!["licenses.exceptions"],
            ),
            (
                "clarify",
                format!("{FAMILY}[[licenses.clarify]]\ncrate = \"x\"\nexpression = \"MIT\"\n"),
                vec!["licenses.clarify"],
            ),
            (
                "loose text",
                swap("0.93", "0.5"),
                vec!["confidence-threshold"],
            ),
        ]
    }

    /// Changes to the advisory, source and ban rules.
    fn advisory_source_and_ban_faults() -> Vec<(&'static str, String, Vec<&'static str>)> {
        vec![
            (
                "yanked warns",
                swap("yanked = \"deny\"", "yanked = \"warn\""),
                vec!["advisories.yanked"],
            ),
            (
                "unmaintained",
                swap("unmaintained = \"all\"", "unmaintained = \"workspace\""),
                vec!["advisories.unmaintained"],
            ),
            (
                "ignored",
                swap(
                    "unsound = \"all\"",
                    "unsound = \"all\"\nignore = [\"RUSTSEC-2021-0003\"]",
                ),
                vec!["advisories.ignore"],
            ),
            (
                "git allowed",
                swap("allow-git = []", "allow-git = [\"https://github.com/x/y\"]"),
                vec!["allow-git"],
            ),
            (
                "git warns",
                swap("unknown-git = \"deny\"", "unknown-git = \"warn\""),
                vec!["sources.unknown-git"],
            ),
            (
                "another registry",
                swap(
                    "crates.io-index\"]",
                    "crates.io-index\", \"https://example.org/index\"]",
                ),
                vec!["lists https://example.org/index"],
            ),
            (
                "a git organisation",
                format!("{FAMILY}[sources.allow-org]\ngithub = [\"x\"]\n"),
                vec!["sources.allow-org"],
            ),
            (
                "one platform",
                swap(
                    "all-features = true",
                    "all-features = true\ntargets = [\"x86_64-unknown-linux-gnu\"]",
                ),
                vec!["graph.targets"],
            ),
            (
                "dev excluded",
                swap(
                    "all-features = true",
                    "all-features = true\nexclude-dev = true",
                ),
                vec!["graph.exclude-dev"],
            ),
            (
                "wildcards",
                swap("wildcards = \"deny\"", "wildcards = \"allow\""),
                vec!["bans.wildcards"],
            ),
            ("not TOML", swap("[bans]", "[bans"), vec!["not TOML"]),
        ]
    }

    /// Each seeded change breaks the family configuration in one way, which the check names.
    #[test]
    fn every_seeded_change_to_the_family_configuration_is_refused() {
        for (name, text, needles) in &family_faults() {
            exactly(name, &family_config(text), needles);
        }
    }

    #[test]
    fn every_seeded_change_to_the_no_std_configuration_is_refused() {
        let swap = |from: &str, to: &str| {
            assert!(NO_STD.contains(from), "the example lacks {from:?}");
            NO_STD.replace(from, to)
        };
        let cases: Vec<(&str, String, &str)> = vec![
            (
                "another crate",
                swap("[\"libm\"]", "[\"libm\", \"hashbrown\"]"),
                "lists hashbrown",
            ),
            (
                "no allow-list",
                swap("allow = [\"libm\"]\n", ""),
                "bans.allow",
            ),
            (
                "dev in",
                swap("exclude-dev = true", "exclude-dev = false"),
                "graph.exclude-dev",
            ),
            (
                "features off",
                swap("all-features = true", "all-features = false"),
                "graph.all-features",
            ),
            (
                "a crate excluded",
                swap(
                    "exclude-dev = true",
                    "exclude-dev = true\nexclude = [\"geom-op\"]",
                ),
                "graph.exclude",
            ),
            (
                "workspace not allowed",
                swap("allow-workspace = true", "allow-workspace = false"),
                "bans.allow-workspace",
            ),
        ];
        for (name, text, needle) in &cases {
            exactly(name, &no_std_config(text), &[needle]);
        }
    }
}

/// The gate's cargo-deny runs, on the repository and on seeded graphs. They need cargo-deny and
/// the network, so a plain `cargo test` leaves them out; the merge gate runs them
/// (`cargo test -p engine-geometry --test scaffold -- --ignored gate_deny`).
#[cfg(all(test, not(target_family = "wasm")))]
mod gate {
    use std::collections::BTreeSet;

    use super::rules::Choice;
    use super::run::{
        ADVISORIES, BANS, CRATES_IO, Edge, Graph, LICENSES, Outcome, SOURCES, Seed, published,
    };
    use super::{ALLOW_LIST_29_2, FAMILY_CONFIG, NO_STD_CONFIG, SHIPS_CONFIG};
    use crate::json::Json;
    use crate::repo;
    use crate::table::TABLE;

    /// The crates named, as a set.
    fn names<'a>(seeds: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
        seeds.into_iter().map(str::to_owned).collect()
    }

    /// Asserts the run failed `bit`, refused every crate of `refused` and none of `passed`.
    fn expect(outcome: &Outcome, bit: i32, refused: &[&str], passed: &[&str]) {
        let got = outcome.refused();
        let missed: Vec<&&str> = refused.iter().filter(|r| !got.contains(**r)).collect();
        let wrongly: Vec<&&str> = passed.iter().filter(|p| got.contains(**p)).collect();
        assert!(
            outcome.failed(bit) && missed.is_empty() && wrongly.is_empty(),
            "expected the check (exit bit {bit:#x}) to refuse {refused:?} and pass {passed:?}; \
             it missed {missed:?} and refused {wrongly:?}:\n{}",
            outcome.show()
        );
    }

    /// Asserts the run passed: cargo-deny and the gate's rules.
    fn expect_pass(outcome: &Outcome) {
        assert!(outcome.passed(), "expected a pass:\n{}", outcome.show());
    }

    #[test]
    #[ignore = "runs cargo-deny, which the merge gate installs; run with --ignored"]
    fn gate_deny_every_crate_of_the_family_passes() {
        let observed = repo::observe();
        let failures = super::run::every_crate(&TABLE, &observed);
        assert!(
            failures.is_empty(),
            "{} cargo-deny run(s) failed:\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }

    /// Seeds one crate under each licence of 29.2's list and one under a choice that includes a
    /// permissive licence, which must pass; and crates under licences the list leaves out, under
    /// none, and as development and build dependencies, which must be refused. Returns the
    /// names that must be refused and those that must pass.
    fn seed_licences(graph: &mut Graph) -> (Vec<String>, Vec<String>) {
        let refused_licences = [
            "GPL-2.0-only",
            "GPL-3.0-or-later",
            "LGPL-2.1-or-later",
            "LGPL-3.0-only",
            "AGPL-3.0-only",
            "EUPL-1.2",
            "CDDL-1.0",
            "EPL-2.0",
            "MPL-1.1",
            "BSD-4-Clause",
            "Apache-1.1",
            "CC-BY-4.0",
            "CC-BY-NC-4.0",
            "PolyForm-Noncommercial-1.0.0",
            "Apache-2.0 WITH Classpath-exception-2.0",
            "MIT AND GPL-3.0-only",
            "LicenseRef-PublicDomain",
        ];
        let mut must_pass = Vec::new();
        for (i, licence) in ALLOW_LIST_29_2.iter().enumerate() {
            let name = format!("seeded-allowed-{i}");
            graph.add(
                "geom-num",
                Edge::Normal,
                "=1.0.0",
                published(&name, "1.0.0", licence),
            );
            must_pass.push(name);
        }
        // A choice that includes a permissive licence is taken under it, and the choice is
        // recorded (29.20, rule 3): passes. Unrecorded, or recorded as a licence the list leaves
        // out: refused.
        let mut must_refuse = Vec::new();
        for (name, chosen, passes) in [
            ("seeded-choice", Some("MIT"), true),
            ("seeded-choice-unrecorded", None, false),
            ("seeded-choice-wrong", Some("GPL-3.0-only"), false),
        ] {
            graph.add(
                "geom-num",
                Edge::Normal,
                "=1.0.0",
                published(name, "1.0.0", "MIT OR GPL-3.0-only"),
            );
            if let Some(chosen) = chosen {
                graph.choices.push(Choice {
                    krate: name.to_owned(),
                    expression: "MIT OR GPL-3.0-only".to_owned(),
                    chosen: chosen.to_owned(),
                });
            }
            if passes {
                must_pass.push(name.to_owned());
            } else {
                must_refuse.push(name.to_owned());
            }
        }
        for (i, licence) in refused_licences.iter().enumerate() {
            let name = format!("seeded-refused-{i}");
            graph.add(
                "geom-num",
                Edge::Normal,
                "=1.0.0",
                published(&name, "1.0.0", licence),
            );
            must_refuse.push(name);
        }
        // Development and build dependencies pass the gate too (29.20, rule 3), and a crate with
        // no licence at all is refused.
        graph
            .add(
                "geom-num",
                Edge::Dev,
                "=1.0.0",
                published("seeded-dev-gpl", "1.0.0", "GPL-3.0-only"),
            )
            .add(
                "geom-num",
                Edge::Build,
                "=1.0.0",
                published("seeded-build-lgpl", "1.0.0", "LGPL-3.0-or-later"),
            )
            .add(
                "geom-num",
                Edge::Normal,
                "=1.0.0",
                Seed {
                    name: "seeded-unlicensed",
                    version: "1.0.0",
                    source: Some(CRATES_IO),
                    license: None,
                },
            );
        must_refuse.extend(
            ["seeded-dev-gpl", "seeded-build-lgpl", "seeded-unlicensed"].map(str::to_owned),
        );
        (must_refuse, must_pass)
    }

    #[test]
    #[ignore = "runs cargo-deny, which the merge gate installs; run with --ignored"]
    fn gate_deny_licences_fail_on_seeded_violations() {
        super::run::fetch(&repo::workspace_root());
        // The unchanged graph passes through the same path, so what fails below is the seeds.
        let clean = Graph::of("geom-num");
        expect_pass(&clean.deny("licences-clean", FAMILY_CONFIG, &["licenses"]));
        let mut graph = Graph::of("geom-num");
        let (must_refuse, must_pass) = seed_licences(&mut graph);
        let outcome = graph.deny("licences-seeded", FAMILY_CONFIG, &["licenses"]);
        let refuse: Vec<&str> = must_refuse.iter().map(String::as_str).collect();
        let pass: Vec<&str> = must_pass.iter().map(String::as_str).collect();
        expect(&outcome, LICENSES, &refuse, &pass);
        // Exactly the seeds that break the list are refused, nothing else.
        assert_eq!(outcome.refused(), names(refuse), "{}", outcome.show());
    }

    #[test]
    #[ignore = "runs cargo-deny, which the merge gate installs; run with --ignored"]
    fn gate_deny_sources_fail_on_seeded_violations() {
        super::run::fetch(&repo::workspace_root());
        let seed = |name, source| Seed {
            name,
            version: "1.0.0",
            source: Some(source),
            license: Some("MIT"),
        };
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let git = format!("git+https://github.com/example/seeded?rev=0123456#{sha}");
        let dev_git = format!("git+https://github.com/example/dev#{sha}");
        let mut graph = Graph::of("geom-op");
        graph
            .add("geom-op", Edge::Normal, "*", seed("seeded-git", &git))
            .add("geom-op", Edge::Dev, "*", seed("seeded-dev-git", &dev_git))
            .add(
                "geom-op",
                Edge::Normal,
                "=1.0.0",
                seed("seeded-registry", "registry+https://example.org/index"),
            )
            .add(
                "geom-op",
                Edge::Normal,
                "=1.0.0",
                seed("seeded-sparse", "sparse+https://example.org/index/"),
            )
            .add(
                "geom-op",
                Edge::Normal,
                "=1.0.0",
                seed("seeded-crates-io", CRATES_IO),
            )
            // A crate from a folder outside the workspace, which cargo-deny's sources check never
            // reads: the gate's rule refuses it.
            .add(
                "geom-op",
                Edge::Normal,
                "=1.0.0",
                Seed {
                    name: "seeded-path",
                    version: "1.0.0",
                    source: None,
                    license: Some("MIT"),
                },
            );
        let outcome = graph.deny("sources-seeded", FAMILY_CONFIG, &["sources"]);
        let refused = [
            "seeded-git",
            "seeded-dev-git",
            "seeded-registry",
            "seeded-sparse",
            "seeded-path",
        ];
        expect(&outcome, SOURCES, &refused, &["seeded-crates-io"]);
        assert_eq!(outcome.refused(), names(refused), "{}", outcome.show());
    }

    #[test]
    #[ignore = "runs cargo-deny, which the merge gate installs; run with --ignored"]
    fn gate_deny_rayon_fails_on_seeded_violations() {
        super::run::fetch(&repo::workspace_root());
        let rayon = published("rayon", "1.12.0", "MIT OR Apache-2.0");
        let rayon_core = published("rayon-core", "1.13.0", "MIT OR Apache-2.0");
        let adapter = published("wasm-bindgen-rayon", "1.3.0", "Apache-2.0");
        // The way chapter 27 and 29.4 allow: geom-pool names rayon and wasm-bindgen-rayon, which
        // names rayon, which names rayon-core. geom-pool is the workspace's own crate, which the
        // facade names behind its `threads` feature (every feature is on in the graph), so the
        // seeds hang from the real crate; seeding a second geom-pool beside it would give the
        // facade two dependencies of one name, which no manifest can declare. Only the bans run
        // here; the other checks' seeded runs are their own tests'.
        let mut allowed = Graph::of("engine-geometry");
        // The facade's resolved node names the workspace's geom-pool, so the route seeded below
        // is one cargo-deny walks from the facade, not a crate off to one side of its graph.
        let id = |name: &str| {
            allowed
                .json
                .get("packages")
                .and_then(Json::as_array)
                .unwrap()
                .iter()
                .find(|p| p.get("name").and_then(Json::as_str) == Some(name))
                .and_then(|p| p.get("id")?.as_str())
                .unwrap_or_else(|| panic!("no package {name} in the facade's graph"))
                .to_owned()
        };
        let (facade, pool) = (id("engine-geometry"), id("geom-pool"));
        let reaches_pool = allowed
            .json
            .get("resolve")
            .and_then(|r| r.get("nodes"))
            .and_then(Json::as_array)
            .unwrap()
            .iter()
            .find(|n| n.get("id").and_then(Json::as_str) == Some(facade.as_str()))
            .and_then(|n| n.get("dependencies")?.as_array())
            .is_some_and(|deps| deps.iter().any(|d| d.as_str() == Some(pool.as_str())));
        assert!(
            reaches_pool,
            "the facade's graph, every feature on, does not reach the workspace's geom-pool"
        );
        allowed
            .add("geom-pool", Edge::Normal, "^1.12", rayon)
            .add("geom-pool", Edge::Normal, "^1.3", adapter)
            .add("wasm-bindgen-rayon", Edge::Normal, "^1.12", rayon)
            .add("rayon", Edge::Normal, "^1.13", rayon_core);
        expect_pass(&allowed.deny("bans-allowed", SHIPS_CONFIG, &["bans"]));
        // Each other way into what ships is refused: rayon from a kernel crate or as its build
        // dependency; rayon-core around rayon; the adapter without geom-pool.
        let cases: [(&str, Edge, Seed<'_>); 4] = [
            ("geom-num", Edge::Normal, rayon),
            ("geom-op", Edge::Build, rayon),
            ("engine-geometry", Edge::Normal, rayon_core),
            ("geom-bvh", Edge::Normal, adapter),
        ];
        for (parent, edge, seed) in cases {
            let mut graph = Graph::of(parent);
            graph.add(parent, edge, &format!("={}", seed.version), seed);
            let outcome = graph.deny(
                &format!("bans-{parent}-{}", seed.name),
                SHIPS_CONFIG,
                &["bans"],
            );
            assert!(
                outcome.failed(BANS) && outcome.codes_for(seed.name).contains("banned"),
                "{parent} -> {} ({edge:?}) passed the bans:\n{}",
                seed.name,
                outcome.show()
            );
        }
        // Outside what ships, rayon may be used (29.1; 29.16's criterion brings it): as a
        // development dependency of a kernel crate, and by a development crate, through a
        // benchmark framework. Neither run of a configuration that applies refuses it.
        let criterion = published("criterion", "0.7.0", "Apache-2.0 OR MIT");
        let mut dev = Graph::of("geom-op");
        dev.add("geom-op", Edge::Dev, "=1.12.0", rayon);
        expect_pass(&dev.deny("bans-geom-op-dev-rayon", SHIPS_CONFIG, &["bans"]));
        let mut bench = Graph::of("geom-test");
        bench
            .add("geom-test", Edge::Normal, "=0.7.0", criterion)
            .add("criterion", Edge::Normal, "^1.12", rayon);
        expect_pass(&bench.deny("bans-geom-test-criterion", FAMILY_CONFIG, &["bans"]));
    }

    /// The gate's own rules (`rules`), end to end through the gate's check of a graph: what
    /// cargo-deny alone passes.
    #[test]
    #[ignore = "runs cargo-deny, which the merge gate installs; run with --ignored"]
    fn gate_deny_our_own_rules_fail_on_seeded_violations() {
        super::run::fetch(&repo::workspace_root());
        // Licences: our own kernel crate under GPL; a crate from a folder outside the workspace,
        // and one from crates.io, each saying publish = false, which cargo-deny's private.ignore
        // skips.
        let mut graph = Graph::of("geom-num");
        graph
            .add(
                "geom-num",
                Edge::Dev,
                "=0.1.0",
                Seed {
                    name: "seeded-private-path",
                    version: "0.1.0",
                    source: None,
                    license: Some("AGPL-3.0-only"),
                },
            )
            .add(
                "geom-num",
                Edge::Normal,
                "=1.0.0",
                published("seeded-private-published", "1.0.0", "GPL-3.0-only"),
            )
            .set_field("geom-num", "license", Json::Str("GPL-3.0-only".to_owned()))
            .set_field("seeded-private-path", "publish", Json::Array(Vec::new()))
            .set_field(
                "seeded-private-published",
                "publish",
                Json::Array(Vec::new()),
            );
        let outcome = graph.deny("rules-licences", FAMILY_CONFIG, &["licenses"]);
        let refused = [
            "geom-num",
            "seeded-private-path",
            "seeded-private-published",
        ];
        expect(&outcome, LICENSES, &refused, &[]);
        assert_eq!(outcome.refused(), names(refused), "{}", outcome.show());
        // Sources: a crate outside the workspace named as a crate of the family, from crates.io.
        let mut impostor = Graph::of("engine-geometry");
        impostor.add(
            "engine-geometry",
            Edge::Normal,
            "=0.0.1",
            published("geom-pool", "0.0.1", "MIT"),
        );
        let outcome = impostor.deny("rules-impostor", FAMILY_CONFIG, &["sources"]);
        assert!(
            outcome.failed(SOURCES) && outcome.codes_for("geom-pool").contains("gate-family-name"),
            "{}",
            outcome.show()
        );
        // The clean graph of the facade, which has no licence field and depends on kit-core,
        // which has none either: ours, under the repository's licence, so it passes.
        expect_pass(&Graph::of("engine-geometry").deny(
            "rules-facade",
            FAMILY_CONFIG,
            &["licenses", "sources"],
        ));
    }

    #[test]
    #[ignore = "runs cargo-deny, which the merge gate installs; run with --ignored"]
    fn gate_deny_wildcard_versions_fail_on_seeded_violations() {
        super::run::fetch(&repo::workspace_root());
        // A registry dependency without a version bound: geom-num's own libm, as if its manifest
        // said `libm = "*"`.
        let mut wildcard = Graph::of("geom-num");
        wildcard.set_req("geom-num", "libm", "*");
        let outcome = wildcard.deny("bans-wildcard", FAMILY_CONFIG, &["bans"]);
        assert!(
            outcome.failed(BANS) && outcome.diagnostics.iter().any(|d| d.code == "wildcard"),
            "a wildcard version passed:\n{}",
            outcome.show()
        );
    }

    #[test]
    #[ignore = "runs cargo-deny, which the merge gate installs; run with --ignored"]
    fn gate_deny_no_std_kernel_crates_name_only_libm() {
        super::run::fetch(&repo::workspace_root());
        // geom-num names libm, and its development dependencies (proptest, the oracles) are left
        // out: it passes.
        expect_pass(&Graph::of("geom-num").deny("no-std-clean", NO_STD_CONFIG, &["bans"]));
        let path_crate = Seed {
            name: "seeded-path",
            version: "0.1.0",
            source: None,
            license: Some("MIT"),
        };
        let cases: [(&str, Edge, Seed<'_>); 4] = [
            (
                "a published crate",
                Edge::Normal,
                published("hashbrown", "0.15.5", "MIT OR Apache-2.0"),
            ),
            (
                "a build dependency",
                Edge::Build,
                published("cc", "1.2.0", "MIT OR Apache-2.0"),
            ),
            (
                "a path crate outside the workspace",
                Edge::Normal,
                path_crate,
            ),
            (
                "a maths crate other than libm",
                Edge::Normal,
                published("micromath", "2.1.0", "Apache-2.0 OR MIT"),
            ),
        ];
        for (what, edge, seed) in cases {
            let mut graph = Graph::of("geom-op");
            graph.add("geom-op", edge, &format!("={}", seed.version), seed);
            let outcome = graph.deny(&format!("no-std-{}", seed.name), NO_STD_CONFIG, &["bans"]);
            assert!(
                outcome.failed(BANS) && outcome.codes_for(seed.name).contains("not-allowed"),
                "{what} ({}) passed deny-no-std.toml:\n{}",
                seed.name,
                outcome.show()
            );
        }
        // A development dependency is not what ships.
        let mut dev = Graph::of("geom-op");
        dev.add(
            "geom-op",
            Edge::Dev,
            "=0.15.5",
            published("hashbrown", "0.15.5", "MIT OR Apache-2.0"),
        );
        expect_pass(&dev.deny("no-std-dev", NO_STD_CONFIG, &["bans"]));
        // libm patched in from a folder (`[patch.crates-io] libm = { path = ... }` changes the
        // resolved package's id and source, not geom-num's declaration): cargo-deny's bans allow
        // it by name, and the gate's rules refuse it, in the bans of deny-no-std.toml and the
        // sources of deny.toml.
        let mut patched = Graph::of("geom-num");
        let old_id = patched
            .json
            .get("packages")
            .and_then(Json::as_array)
            .unwrap()
            .iter()
            .find(|p| p.get("name").and_then(Json::as_str) == Some("libm"))
            .and_then(|p| p.get("id")?.as_str())
            .unwrap()
            .to_owned();
        let version = old_id.rsplit('@').next().unwrap().to_owned();
        let text = patched
            .json
            .to_text()
            .replace(&old_id, &format!("path+file:///vendor/libm#libm@{version}"));
        patched.json = crate::json::parse(&text).unwrap();
        patched.set_field("libm", "source", Json::Null);
        let no_std = patched.deny("no-std-libm-path", NO_STD_CONFIG, &["bans"]);
        let family = patched.deny("family-libm-path", FAMILY_CONFIG, &["sources"]);
        assert!(
            no_std.failed(BANS)
                && no_std.codes_for("libm").contains("gate-not-libm")
                && family.failed(SOURCES)
                && family.codes_for("libm").contains("gate-not-crates-io"),
            "a libm from a folder passed:\n{}\n\n{}",
            no_std.show(),
            family.show()
        );
    }

    #[test]
    #[ignore = "runs cargo-deny, which the merge gate installs; run with --ignored"]
    fn gate_deny_advisories_fail_on_seeded_violations() {
        super::run::fetch(&repo::workspace_root());
        // Each seed is a published version with a RustSec advisory of one kind, or one crates.io
        // has yanked: smallvec 1.6.0, a vulnerability (RUSTSEC-2021-0003); ansi_term 0.12.1,
        // unmaintained (RUSTSEC-2021-0139); atty 0.2.14, unsound (RUSTSEC-2021-0145); libm 0.2.4,
        // yanked. libm is the kernel's own dependency, so its crates.io index entry is always in
        // the local cache the yanked check reads.
        let mut graph = Graph::of("geom-num");
        for (name, version) in [
            ("smallvec", "1.6.0"),
            ("ansi_term", "0.12.1"),
            ("atty", "0.2.14"),
            ("libm", "0.2.4"),
        ] {
            graph.add(
                "geom-num",
                Edge::Normal,
                &format!("={version}"),
                published(name, version, "MIT"),
            );
        }
        let outcome = graph.deny("advisories-seeded", FAMILY_CONFIG, &["advisories"]);
        for (name, code) in [
            ("smallvec", "vulnerability"),
            ("ansi_term", "unmaintained"),
            ("atty", "unsound"),
            ("libm", "yanked"),
        ] {
            assert!(
                outcome.failed(ADVISORIES) && outcome.codes_for(name).contains(code),
                "{name}: no {code} error:\n{}",
                outcome.show()
            );
        }
        // The real graph has none of these.
        expect_pass(&Graph::of("geom-num").deny(
            "advisories-clean",
            FAMILY_CONFIG,
            &["advisories"],
        ));
    }
}
