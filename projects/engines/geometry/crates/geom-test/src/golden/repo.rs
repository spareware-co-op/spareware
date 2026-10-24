//! The golden store against the repository: every golden is checked where the store says, every
//! golden check in the code is in the store, no golden digest hides outside it, and every pinned
//! file is unchanged.
//!
//! What it does: [`check`] reads the kernel's crates under `projects/engines/geometry/crates` and
//! returns every [`Problem`]:
//!
//! - a golden of the current edition whose `check` names a test file that does not exist, or one
//!   that does not define the named test function or does not name the golden's id;
//! - a golden whose `file` is missing, or whose text no longer has the store's digest (the file was
//!   edited without the store, which would let it change unapproved);
//! - a call `golden::check(...)` (or `check_in`) in any kernel crate's source or tests that uses
//!   `geom_test::golden` (or in this crate's own code) whose id the current edition does not hold,
//!   or whose id cannot be read (it must be a string literal, or a `const` of the same file that is
//!   one); another crate's own `golden` module, such as `geom-bench`'s, is not the store's;
//! - a constant whose name holds `GOLDEN` or `DIGEST` and whose value is a 64-digit hexadecimal
//!   string, in any kernel crate's source or tests: a golden digest kept outside the store, where
//!   no edition, history or approval rule reaches it.
//!
//! Guarantees:
//!
//! - **It never passes by accident.** A folder it cannot read, a crate with no files, or a store
//!   whose current edition has no golden is a problem, not a pass.
//! - **Every problem names its file**, relative to the workspace root with `/`.
//! - **Deterministic.** Files are read in path order; problems are sorted.
//!
//! Method: a lexical scan for the call and constant forms above, the way the map (`crate::map`)
//! and the merge gate's source rules read the kernel's sources as text with a written grammar
//! (chapter 26.3 and 28.8, `projects/engines/geometry/architecture/28-testing.md`); comments are
//! not excluded, so an id named only in a comment still counts as named in the file.
//!
//! Cost: linear in the size of the kernel's Rust sources.

use core::fmt;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::sha256::Digest;
use super::store::{Edition, Store, text_digest};
use crate::deps::KERNEL_CRATES_DIR;

/// One way the repository and the store disagree.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Problem {
    /// The file concerned, relative to the workspace root with `/`.
    pub file: String,
    /// What is wrong, and what to do.
    pub what: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file, self.what)
    }
}

/// A Rust source file: its path relative to the workspace root with `/`, and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// Its path.
    pub path: String,
    /// Its text.
    pub text: String,
}

/// Every `.rs` file under `dir`, in path order, recursively; `Err` with the folder that could not
/// be read.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        paths.push(entry.path());
    }
    paths.sort();
    for path in paths {
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            rust_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// The path of `path` relative to `root`, with `/`.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Reads every Rust source of every kernel crate (its `src` and `tests` folders) under `root`.
///
/// # Errors
///
/// A message naming the folder or file that could not be read.
pub fn kernel_sources(root: &Path) -> Result<Vec<Source>, String> {
    let crates_dir = KERNEL_CRATES_DIR
        .iter()
        .fold(root.to_path_buf(), |p, c| p.join(c));
    let mut crates: Vec<PathBuf> = std::fs::read_dir(&crates_dir)
        .map_err(|e| format!("{}: {e}", crates_dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.join("Cargo.toml").is_file())
        .collect();
    crates.sort();
    if crates.is_empty() {
        return Err(format!("{}: no crate found", crates_dir.display()));
    }
    let mut files = Vec::new();
    for krate in &crates {
        for folder in ["src", "tests"] {
            let dir = krate.join(folder);
            if dir.is_dir() {
                rust_files(&dir, &mut files)?;
            }
        }
    }
    let mut out = Vec::new();
    for path in files {
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        out.push(Source {
            path: relative(root, &path),
            text,
        });
    }
    Ok(out)
}

/// The string literal starting at `rest` (which begins with `"`), without escapes, or `None`.
fn literal(rest: &str) -> Option<&str> {
    let body = rest.strip_prefix('"')?;
    let end = body.find('"')?;
    body.get(..end)
}

/// The identifier `rest` starts with.
fn identifier(rest: &str) -> &str {
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    rest.get(..end).unwrap_or("")
}

/// The value of `const <name>: &str = "<value>";` in `text`, if it is there.
fn const_value<'t>(text: &'t str, name: &str) -> Option<&'t str> {
    let needle = format!("const {name}: &str = ");
    let at = text.find(&needle)?;
    literal(text.get(at.saturating_add(needle.len())..)?)
}

/// Every id passed to `golden::check(` or `golden::check_in(` in `text` (`check_in` takes the store
/// first, so its id is the second argument), with `None` for a call whose id is not a literal or a
/// literal `const` of the file.
#[must_use]
pub fn checked_ids(text: &str) -> Vec<Option<String>> {
    let mut out = Vec::new();
    for (call, skip_first) in [("golden::check(", false), ("golden::check_in(", true)] {
        let mut from = 0;
        while let Some(found) = text.get(from..).and_then(|t| t.find(call)) {
            let start = from.saturating_add(found).saturating_add(call.len());
            from = start;
            let mut rest = text.get(start..).unwrap_or("").trim_start();
            if skip_first {
                let Some(comma) = rest.find(',') else {
                    out.push(None);
                    continue;
                };
                rest = rest
                    .get(comma.saturating_add(1)..)
                    .unwrap_or("")
                    .trim_start();
            }
            let id = if rest.starts_with('"') {
                literal(rest).map(str::to_owned)
            } else {
                let name = identifier(rest);
                if name.is_empty() {
                    None
                } else {
                    const_value(text, name).map(str::to_owned)
                }
            };
            out.push(id);
        }
    }
    out
}

/// Every `const NAME: &str = "<64 hex digits>"` whose name holds `GOLDEN` or `DIGEST`, as
/// (name, line number from 1).
#[must_use]
pub fn stray_digests(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let Some(at) = line.find("const ") else {
            continue;
        };
        let rest = line.get(at.saturating_add(6)..).unwrap_or("");
        let name = identifier(rest);
        if !(name.contains("GOLDEN") || name.contains("DIGEST")) {
            continue;
        }
        let Some(eq) = rest.find("= ") else { continue };
        let value = rest.get(eq.saturating_add(2)..).unwrap_or("").trim_start();
        if literal(value).is_some_and(|v| Digest::from_hex(v).is_some()) {
            out.push((name.to_owned(), index.saturating_add(1)));
        }
    }
    out
}

/// The test file of a check: `tests/<test>.rs` or `tests/<test>/main.rs`, or, for `lib`, the source
/// file whose module path the function's path starts with.
fn check_file<'s>(
    sources: &'s [Source],
    krate: &str,
    test: &str,
    function: &str,
) -> Option<&'s Source> {
    let base = format!("{}/{krate}", KERNEL_CRATES_DIR.join("/"));
    if test == "lib" {
        // `a::b::tests::f`: the module is the path before `tests::f` (or before `f`).
        let parts: Vec<&str> = function.split("::").collect();
        let module: Vec<&str> = match parts.iter().position(|p| *p == "tests") {
            Some(at) => parts.get(..at).unwrap_or(&[]).to_vec(),
            None => parts
                .get(..parts.len().saturating_sub(1))
                .unwrap_or(&[])
                .to_vec(),
        };
        let stem = if module.is_empty() {
            format!("{base}/src/lib.rs")
        } else {
            format!("{base}/src/{}", module.join("/"))
        };
        let candidates = [stem.clone(), format!("{stem}.rs"), format!("{stem}/mod.rs")];
        sources.iter().find(|s| candidates.contains(&s.path))
    } else {
        let candidates = [
            format!("{base}/tests/{test}.rs"),
            format!("{base}/tests/{test}/main.rs"),
        ];
        sources.iter().find(|s| candidates.contains(&s.path))
    }
}

/// Every disagreement between `store` and the kernel's `sources`, with `file_text` reading a pinned
/// file by its path relative to the workspace root.
#[must_use]
pub fn problems(
    store: &Store,
    sources: &[Source],
    file_text: &dyn Fn(&str) -> Result<String, String>,
) -> Vec<Problem> {
    let mut found = BTreeSet::new();
    let store_file = super::store::STORE_PATH.join("/");
    let mut add = |file: &str, what: String| {
        found.insert(Problem {
            file: file.to_owned(),
            what,
        });
    };
    let Ok(edition) = store.current() else {
        add(&store_file, "the store has no edition".to_owned());
        return found.into_iter().collect();
    };
    if edition.goldens.is_empty() {
        add(&store_file, "the current edition has no golden".to_owned());
    }
    store_side(edition, sources, file_text, &mut add);
    source_side(edition, sources, &mut add);
    found.into_iter().collect()
}

/// The goldens' side: each is checked where the store says, and each pinned file keeps its digest.
fn store_side(
    edition: &Edition,
    sources: &[Source],
    file_text: &dyn Fn(&str) -> Result<String, String>,
    add: &mut impl FnMut(&str, String),
) {
    let store_file = super::store::STORE_PATH.join("/");
    for golden in &edition.goldens {
        if let Some(check) = &golden.check {
            match check_file(sources, &check.krate, &check.test, &check.function) {
                None => add(
                    &store_file,
                    format!(
                        "golden {}: its check names test target {:?} of {}, and no such file \
                         exists",
                        golden.id, check.test, check.krate
                    ),
                ),
                Some(src) => {
                    let name = check.function.rsplit("::").next().unwrap_or("");
                    if !src.text.contains(&format!("fn {name}(")) {
                        add(
                            &src.path,
                            format!(
                                "the store checks golden {} in function {}, which this file does \
                                 not define",
                                golden.id, check.function
                            ),
                        );
                    }
                    if !src.text.contains(&format!("\"{}\"", golden.id)) {
                        add(
                            &src.path,
                            format!(
                                "the store checks golden {} here, but this file never names it",
                                golden.id
                            ),
                        );
                    }
                }
            }
        }
        if let Some(file) = &golden.file {
            match file_text(file) {
                Err(e) => add(file, format!("golden {}: cannot read it: {e}", golden.id)),
                Ok(text) => {
                    let digest = text_digest(&text).to_string();
                    if digest != golden.sha256 {
                        add(
                            file,
                            format!(
                                "its text has digest {digest} but golden {} pins {}: this golden \
                                 file changed without the store. Record the change in {store_file} \
                                 (the old digest in the golden's history, with its kind, piece and \
                                 why); a kernel owner approves it",
                                golden.id, golden.sha256
                            ),
                        );
                    }
                }
            }
        }
    }
}

/// The sources' side: every golden check names a golden of the store, and no digest hides outside
/// it.
fn source_side(edition: &Edition, sources: &[Source], add: &mut impl FnMut(&str, String)) {
    let store_file = super::store::STORE_PATH.join("/");
    let known: BTreeSet<&str> = edition.goldens.iter().map(|g| g.id.as_str()).collect();
    for src in sources {
        // The scanner's own fixtures and documentation name the call forms in prose.
        if src.path.ends_with("geom-test/src/golden/repo.rs") {
            continue;
        }
        // Another crate's own `golden` module (geom-bench's checks its benchmarks' digests) is
        // not the store's: only files that use `geom_test::golden`, and this crate's own code,
        // call the store's check.
        let store_checks =
            src.text.contains("geom_test::golden") || src.path.contains("crates/geom-test/src/");
        let ids = if store_checks {
            checked_ids(&src.text)
        } else {
            Vec::new()
        };
        for id in ids {
            match id {
                None => add(
                    &src.path,
                    "a golden check whose id is neither a string literal nor a string `const` of \
                     this file; name the id so the store can be checked against it"
                        .to_owned(),
                ),
                Some(id) if !known.contains(id.as_str()) => add(
                    &src.path,
                    format!(
                        "checks golden {id:?}, which the store's current edition does not hold; \
                         add it to {store_file}"
                    ),
                ),
                Some(_) => {}
            }
        }
        for (name, line) in stray_digests(&src.text) {
            add(
                &src.path,
                format!(
                    "line {line}: const {name} holds a 64-digit digest; a golden digest lives in \
                     the store ({store_file}), where its edition, history and approval rule \
                     reach it"
                ),
            );
        }
    }
}

/// Reads the kernel's sources under `root` and checks them against `store`.
///
/// # Errors
///
/// A message naming a folder or file that could not be read.
pub fn check(root: &Path, store: &Store) -> Result<Vec<Problem>, String> {
    let sources = kernel_sources(root)?;
    let read = |relative: &str| -> Result<String, String> {
        let path = relative
            .split('/')
            .fold(root.to_path_buf(), |p, c| p.join(c));
        std::fs::read_to_string(&path).map_err(|e| e.to_string())
    };
    Ok(problems(store, &sources, &read))
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use super::*;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn store(extra: &str) -> Store {
        Store::parse(&format!(
            r#"{{"format": 1, "about": "t", "editions": [{{"edition": "pre-edition",
            "state": "open", "goldens": [{{"id": "geom-x/a", "what": "w", "sha256": "{A}",
            "check": {{"crate": "geom-x", "test": "golden", "function": "it_holds"}}}}{extra}]}}]}}"#
        ))
        .unwrap()
    }

    fn src(path: &str, text: &str) -> Source {
        Source {
            path: format!("projects/engines/geometry/crates/{path}"),
            text: text.to_owned(),
        }
    }

    fn no_files(_: &str) -> Result<String, String> {
        Err("no files here".to_owned())
    }

    #[test]
    fn ids_are_read_from_literals_and_consts() {
        let text = "golden::check(\"geom-x/a\", f); const ID: &str = \"geom-x/b\"; \
                    golden::check(ID, f); golden::check_in(&s, \"geom-x/c\", f); \
                    golden::check(make_id(), f);";
        assert_eq!(
            checked_ids(text),
            [
                Some("geom-x/a".to_owned()),
                Some("geom-x/b".to_owned()),
                None,
                Some("geom-x/c".to_owned()),
            ]
        );
    }

    #[test]
    fn stray_digests_are_found_by_name_and_value() {
        let text = format!(
            "const GOLDEN: &str = \"{A}\";\nconst OTHER: &str = \"{A}\";\n\
             const FNV_GOLDEN: u64 = 0x1;\npub const WORKLOAD_DIGEST: &str = \"{A}\";\n\
             const GOLDEN_NAME: &str = \"geom-x/a\";"
        );
        assert_eq!(
            stray_digests(&text),
            [("GOLDEN".to_owned(), 1), ("WORKLOAD_DIGEST".to_owned(), 4)]
        );
    }

    #[test]
    fn a_consistent_tree_has_no_problem() {
        let sources = [src(
            "geom-x/tests/golden.rs",
            "#[test]\nfn it_holds() { golden::check(\"geom-x/a\", f); }",
        )];
        assert_eq!(problems(&store(""), &sources, &no_files), []);
    }

    #[test]
    fn every_disagreement_is_found() {
        let good = "#[test]\nfn it_holds() { golden::check(\"geom-x/a\", f); }";
        let cases: Vec<(&str, Vec<Source>)> = vec![
            ("missing test file", vec![]),
            (
                "missing function",
                vec![src(
                    "geom-x/tests/golden.rs",
                    "golden::check(\"geom-x/a\", f);",
                )],
            ),
            (
                "id not named",
                vec![src("geom-x/tests/golden.rs", "#[test]\nfn it_holds() {}")],
            ),
            (
                "unknown id",
                vec![
                    src("geom-x/tests/golden.rs", good),
                    src(
                        "geom-y/tests/t.rs",
                        "use geom_test::golden;
golden::check(\"geom-y/zz\", f);",
                    ),
                ],
            ),
            (
                "unreadable id",
                vec![
                    src("geom-x/tests/golden.rs", good),
                    src(
                        "geom-y/tests/t.rs",
                        "use geom_test::golden;
golden::check(id(), f);",
                    ),
                ],
            ),
            (
                "stray digest",
                vec![
                    src("geom-x/tests/golden.rs", good),
                    src(
                        "geom-y/src/lib.rs",
                        &format!("const GOLDEN: &str = \"{A}\";"),
                    ),
                ],
            ),
        ];
        for (name, sources) in cases {
            let p = problems(&store(""), &sources, &no_files);
            assert!(!p.is_empty(), "{name}: no problem found");
        }
    }

    /// Another crate's own `golden::check` (geom-bench's) is not a store check.
    #[test]
    fn another_crates_golden_module_is_not_the_stores() {
        let sources = [
            src(
                "geom-x/tests/golden.rs",
                "use geom_test::golden;
#[test]
fn it_holds() { golden::check(\"geom-x/a\", f); }",
            ),
            src(
                "geom-bench/src/guest.rs",
                "use crate::golden;
golden::check(name, d);",
            ),
        ];
        assert_eq!(problems(&store(""), &sources, &no_files), []);
        let unknown = [
            sources[0].clone(),
            src(
                "geom-y/tests/t.rs",
                "use geom_test::golden;
golden::check(\"geom-y/zz\", f);",
            ),
        ];
        assert_eq!(problems(&store(""), &unknown, &no_files).len(), 1);
    }

    #[test]
    fn a_pinned_file_must_keep_its_digest() {
        let text = "line one\nline two\n";
        let digest = text_digest(text).to_string();
        let extra = format!(
            r#",{{"id": "geom-z/file", "what": "w", "sha256": "{digest}", "file": "a/golden.txt"}}"#
        );
        let sources = [src(
            "geom-x/tests/golden.rs",
            "#[test]\nfn it_holds() { golden::check(\"geom-x/a\", f); }",
        )];
        let same = |_: &str| Ok("line one\r\nline two\r\n".to_owned());
        assert_eq!(problems(&store(&extra), &sources, &same), []);
        let edited = |_: &str| Ok("line one\nline 2\n".to_owned());
        assert_eq!(problems(&store(&extra), &sources, &edited).len(), 1);
        assert_eq!(problems(&store(&extra), &sources, &no_files).len(), 1);
    }

    #[test]
    fn lib_checks_find_their_module_file() {
        let sources = [
            src("geom-x/src/lib.rs", ""),
            src(
                "geom-x/src/golden/shuffle.rs",
                "fn pinned() { \"geom-x/a\" }",
            ),
        ];
        let found = check_file(&sources, "geom-x", "lib", "golden::shuffle::tests::pinned");
        assert_eq!(
            found.map(|s| s.path.as_str()),
            Some("projects/engines/geometry/crates/geom-x/src/golden/shuffle.rs")
        );
        let root = check_file(&sources, "geom-x", "lib", "tests::pinned");
        assert_eq!(
            root.map(|s| s.path.as_str()),
            Some("projects/engines/geometry/crates/geom-x/src/lib.rs")
        );
    }
}
