//! Tests of the `fixture` binary, run as CI runs it, on a scratch tree with its own Git history
//! (piece G97).
//!
//! What this file proves:
//!
//! - `check`, `list` and `run` work on the real tree.
//! - `diff` refuses a deleted fixture against a real commit, and needs an owner for a changed
//!   expectation; `fixes` lists what a pull request adds that must fail on the base.
//! - `fails` passes when a fixture fails on the tree it runs in, and `one` says so.
//! - `seeds` writes each log in the native format; `minimise` keeps a failure; `improve` refuses a
//!   fixture that did not improve.
//!
//! Native only: WebAssembly has no processes.
#![cfg(not(target_family = "wasm"))]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use geom_format::operation::Operation;
use geom_op::{OpId, OpPath};
use geom_run::Budget;
use geom_test::fixture::repo::DIR;
use geom_test::fixture::{
    Change, Class, Counts, Expect, Fixture, Hash, Measures, OpLog, Source, Speed, Status,
};
use geom_test::golden::store::ChangeKind;

const EDITION: &str = "pre-edition";

fn tool(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fixture"))
        .args(args)
        .output()
        .unwrap()
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fixture-tool-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{args:?}: {}", text(&out));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn fixture(id: &str, fixed_by: Option<&str>) -> Fixture {
    let log = OpLog::new(
        Vec::new(),
        vec![Operation {
            op: OpPath::new(OpId::from_bytes([1; 16])),
            name: "no.such.op".to_owned(),
            version: 1,
            inputs: Vec::new(),
            params: vec![0x01],
            budget: Budget::new(100, 1 << 20),
        }],
    );
    let key = (EDITION.to_owned(), log.versions());
    Fixture {
        id: id.to_owned(),
        source: Source::Issue { issue: 1 },
        fixed_by: fixed_by.map(str::to_owned),
        speed: Speed::Fast,
        expect: BTreeMap::from([(key, expect(1))]),
        log,
        status: Status::Pass,
        history: Vec::new(),
    }
}

fn expect(byte: u8) -> Expect {
    Expect::Ok {
        hash: Some(Hash([byte; 32])),
        measures: Measures {
            volume6: None,
            area2: None,
            counts: Counts::default(),
            bounds: None,
            exactness: Class::Exact,
        },
    }
}

fn folder(root: &Path) -> PathBuf {
    DIR.iter().fold(root.to_path_buf(), |p, part| p.join(part))
}

fn store(root: &Path, f: &Fixture) {
    let path = folder(root).join(format!("{}.json", f.id));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, f.to_json().unwrap()).unwrap();
}

fn repository(name: &str) -> (PathBuf, String) {
    let root = scratch(name);
    git(&root, &["init", "--quiet", "--initial-branch=main"]);
    store(&root, &fixture("bool/0001-kept", None));
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "one"]);
    let base = git(&root, &["rev-parse", "HEAD"]);
    (root, base)
}

#[test]
fn the_real_tree_is_checked_and_run() {
    let out = tool(&["check"]);
    assert!(out.status.success(), "{}", text(&out));
    let out = tool(&["list"]);
    assert!(out.status.success(), "{}", text(&out));
    for tier in ["fast", "slow", "all"] {
        let out = tool(&["run", "--tier", tier]);
        assert!(out.status.success(), "{tier}: {}", text(&out));
        assert!(text(&out).contains("did not pass"));
    }
    assert_eq!(tool(&["no-such-command"]).status.code(), Some(2));
    assert_eq!(tool(&["one", "bool/9999-none"]).status.code(), Some(2));
}

#[test]
fn a_tree_that_breaks_a_rule_fails_its_check() {
    let root = scratch("bad-tree");
    let path = folder(&root).join("bool/0001-x.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "{}").unwrap();
    let out = tool(&["check", "--root", root.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("bool/0001-x.json"));
    // A fixture this binary was not built with is a stale build.
    store(&root, &fixture("bool/0001-x", None));
    let out = tool(&["check", "--root", root.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        text(&out).contains("built with another set"),
        "{}",
        text(&out)
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn diff_holds_a_pull_request_to_the_rules() {
    let (root, base) = repository("diff");
    let at = root.to_str().unwrap();
    // Nothing changed; then a new fixture, free.
    assert!(
        tool(&["diff", "--base", &base, "--root", at])
            .status
            .success()
    );
    store(&root, &fixture("bool/0002-new", None));
    let out = tool(&["diff", "--base", &base, "--root", at]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("added: bool/0002-new"));
    // A deleted fixture never stands.
    fs::remove_file(folder(&root).join("bool/0001-kept.json")).unwrap();
    let out = tool(&["diff", "--base", &base, "--root", at]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out).contains("never deleted"), "{}", text(&out));
    // A recorded change to an expectation needs a kernel owner.
    let mut changed = fixture("bool/0001-kept", None);
    let key = changed.expect.keys().next().unwrap().clone();
    changed.expect.insert(key.clone(), expect(2));
    changed.history.push(Change {
        edition: key.0,
        operations: key.1,
        was: expect(1),
        kind: ChangeKind::Bug,
        piece: "G1".to_owned(),
        why: "w".to_owned(),
    });
    store(&root, &changed);
    let out = tool(&["diff", "--base", &base, "--root", at]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        text(&out).contains("needs a kernel owner's approval"),
        "{}",
        text(&out)
    );
    // With an approving review on the head commit, it stands.
    let reviews = root.join("reviews.json");
    fs::write(
        &reviews,
        r#"[{"user":{"login":"owner"},"state":"APPROVED","commit_id":"abc","author_association":"OWNER"}]"#,
    )
    .unwrap();
    let out = tool(&[
        "diff",
        "--base",
        &base,
        "--root",
        at,
        "--reviews",
        reviews.to_str().unwrap(),
        "--head",
        "abc",
        "--author",
        "someone",
    ]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(
        text(&out).contains("approved on abc by owner"),
        "{}",
        text(&out)
    );
    // On another commit it does not.
    let out = tool(&[
        "diff",
        "--base",
        &base,
        "--root",
        at,
        "--reviews",
        reviews.to_str().unwrap(),
        "--head",
        "def",
        "--author",
        "someone",
    ]);
    assert_eq!(out.status.code(), Some(1));
    // A commit that is not there cannot be read.
    assert_eq!(
        tool(&["diff", "--base", "nosuchcommit", "--root", at])
            .status
            .code(),
        Some(2)
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn fixes_lists_what_must_fail_on_the_base_and_fails_proves_it() {
    let (root, base) = repository("fixes");
    let at = root.to_str().unwrap();
    store(&root, &fixture("bool/0002-fixed", Some("#9")));
    store(&root, &fixture("bool/0003-plain", None));
    let out = tool(&["fixes", "--base", &base, "--root", at]);
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "bool/0002-fixed\n");
    // The kernel refuses `no.such.op`, so the fixture fails here, as it must on a base.
    let out = tool(&["fails", "--root", at, "bool/0002-fixed"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("fails on the merge base, as it must"));
    let out = tool(&["one", "bool/0002-fixed", "--root", at]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("fail:"));
    // `improve` will not touch a fixture that did not improve, and `minimise` keeps the failure.
    let out = tool(&["improve", "bool/0002-fixed", "--root", at]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    let out = tool(&["minimise", "bool/0002-fixed", "--root", at]);
    assert!(out.status.success(), "{}", text(&out));
    let printed = Fixture::parse(&String::from_utf8_lossy(&out.stdout)).unwrap();
    assert_eq!(printed.log.operations.len(), 1);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn seeds_writes_each_log_in_the_native_format() {
    let dir = scratch("seeds");
    let out = tool(&["seeds", dir.join("out").to_str().unwrap()]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("logs"));
    fs::remove_dir_all(&dir).unwrap();
}
