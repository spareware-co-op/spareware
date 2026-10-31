//! Tests of the corpus register and its fetcher (piece G114), through the public interface.
//!
//! The board's done-when: "Manifest with source, version, SHA-256, licence and stored-or-fetched for
//! every corpus; hash-checked fetcher; CI refuses unlicensed or non-commercial vendored files and
//! missing credits."
//!
//! What this file proves:
//!
//! - Every corpus chapter 28.12 and 29.15 name is in the register, with what each must say.
//! - Each rule refuses what it must: a corpus without its pins, non-commercial data, data that may
//!   not be stored, a missing credit, a bad Git pin, a repeated id, unknown fields.
//! - A tree has one digest: order and machine do not matter, and any change moves it.
//! - The vendored check fails an unlisted, unlicensed or changed file.
//! - The fetcher takes a pinned source, refuses bytes that are not the register's and deletes them,
//!   keeps a good cached copy and replaces a bad one, driving a local Git repository and a `file:`
//!   URL so it needs no network.
//!
//! Native only: WebAssembly has no file system and no processes.

#![cfg(not(target_family = "wasm"))]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::disallowed_methods,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use geom_test::corpus::{
    Corpus, CorpusError, Fetch, FetchFile, Fetched, Manifest, Storage, check_vendored, digest_of,
    digest_source, fetch, tree_digest,
};
use geom_test::fixture::Hash;
use geom_test::golden::sha256;

fn entry(id: &str, licence: &str, storage: Storage) -> Corpus {
    let needs_bytes = matches!(storage, Storage::Stored | Storage::Fetched);
    Corpus {
        id: id.to_owned(),
        what: "models".to_owned(),
        used_for: "tests".to_owned(),
        source: "https://example.org".to_owned(),
        version: "1".to_owned(),
        licence: licence.to_owned(),
        storage,
        sha256: needs_bytes.then_some(Hash([1; 32])),
        credit: Some("Someone".to_owned()),
        fetch: needs_bytes.then(|| Fetch::Git {
            url: "https://example.org/x.git".to_owned(),
            commit: "a".repeat(40),
            paths: vec!["models".to_owned()],
        }),
        decision: (storage == Storage::NotUsed).then(|| "an owner, 29.15".to_owned()),
    }
}

fn parsed(corpora: Vec<Corpus>) -> Result<Manifest, CorpusError> {
    let m = Manifest {
        format: 1,
        about: "x".to_owned(),
        corpora,
    };
    Manifest::parse(&serde_json::to_string(&m).unwrap())
}

fn rule(c: Corpus) -> String {
    match parsed(vec![c]) {
        Err(CorpusError::Rule { what, .. }) => what,
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_register_holds_every_corpus_the_chapters_name() {
    let m = Manifest::embedded().unwrap();
    for id in [
        "manifold-models",
        "thingi10k",
        "ember-pairs",
        "ssi-corpus",
        "nist-cad-models",
        "step-conformance",
        "abc-dataset",
        "cadquery-cases",
        "openscad-csg",
        "buildingsmart-sample-test-files",
        "ifc43-sample-models",
        "ifc4x-alignment-set",
        "jts-xml-suite",
        "ezdxf-examples",
        "pdal-las-samples",
        "rhino3dm-samples",
        "threemf-test-suites",
        "kicad-gerber",
        "failure-catalogues",
        "generated",
        "recorded-oracle-outputs",
    ] {
        assert!(m.get(id).is_some(), "{id}");
    }
    let stored_or_fetched = m
        .corpora
        .iter()
        .filter(|c| matches!(c.storage, Storage::Stored | Storage::Fetched));
    for c in stored_or_fetched {
        assert!(c.sha256.is_some() && c.fetch.is_some(), "{}", c.id);
    }
    // The decisions of 29.15: these wait, and stay unused.
    for id in ["abc-dataset", "ifc4x-alignment-set", "openscad-csg"] {
        assert_eq!(m.get(id).unwrap().storage, Storage::NotUsed, "{id}");
    }
    assert_eq!(m.get("no-such").map(|c| c.id.clone()), None);
}

#[test]
fn a_corpus_says_all_of_it() {
    assert!(parsed(vec![entry("a", "MIT", Storage::Stored)]).is_ok());
    let mut c = entry("a", "MIT", Storage::Stored);
    c.sha256 = None;
    assert!(rule(c).contains("digest"));
    let mut c = entry("a", "MIT", Storage::Fetched);
    c.fetch = None;
    assert!(rule(c).contains("pinned source"));
    let mut c = entry("a", "MIT", Storage::Rebuilt);
    c.sha256 = Some(Hash([1; 32]));
    assert!(rule(c).contains("bytes to pin"));
    let mut c = entry("a", "MIT", Storage::Generated);
    c.fetch = entry("a", "MIT", Storage::Fetched).fetch;
    assert!(rule(c).contains("bytes to pin"));
    let mut c = entry("a", "MIT", Storage::NotUsed);
    c.decision = None;
    assert!(rule(c).contains("decision"));
    let mut c = entry("a", "MIT", Storage::Rebuilt);
    c.decision = Some("x".to_owned());
    assert!(rule(c).contains("decision"));
    for blank in ["what", "used_for", "source", "version", "licence"] {
        let mut c = entry("a", "MIT", Storage::Rebuilt);
        match blank {
            "what" => c.what = " ".to_owned(),
            "used_for" => c.used_for = String::new(),
            "source" => c.source = String::new(),
            "version" => c.version = String::new(),
            _ => c.licence = String::new(),
        }
        assert!(rule(c).contains(blank), "{blank}");
    }
    assert!(rule(entry("Not Ok", "MIT", Storage::Rebuilt)).contains("id"));
    let a = entry("a", "MIT", Storage::Rebuilt);
    assert!(matches!(
        parsed(vec![a.clone(), a]),
        Err(CorpusError::Rule { what, .. }) if what.contains("one id")
    ));
}

#[test]
fn nothing_non_commercial_anywhere() {
    for licence in [
        "CC-BY-NC-4.0",
        "CC-BY-NC-SA-3.0",
        "LicenseRef-Non-Commercial",
    ] {
        for storage in [Storage::Stored, Storage::Fetched, Storage::Rebuilt] {
            assert!(rule(entry("a", licence, storage)).contains("non-commercial"));
        }
        assert!(parsed(vec![entry("a", licence, Storage::NotUsed)]).is_ok());
    }
}

#[test]
fn storing_follows_the_licence() {
    for good in [
        "MIT",
        "Apache-2.0",
        "BSD-3-Clause",
        "MIT OR Apache-2.0",
        "CC0-1.0",
        "CC-BY-4.0",
        "CC-BY-ND-4.0",
        "EDL-1.0",
        "GPL-3.0-only OR MIT",
    ] {
        assert!(
            parsed(vec![entry("a", good, Storage::Stored)]).is_ok(),
            "{good}"
        );
    }
    for bad in [
        "GPL-2.0-only",
        "AGPL-3.0-only",
        "MIT AND GPL-3.0-only",
        "LicenseRef-Unknown",
        "LGPL-3.0-only",
    ] {
        assert!(
            rule(entry("a", bad, Storage::Stored)).contains("cannot be stored"),
            "{bad}"
        );
    }
    // Copyleft data may be fetched, never stored (32.6).
    assert!(parsed(vec![entry("a", "GPL-2.0-only", Storage::Fetched)]).is_ok());
    // A credit is needed unless the licence asks for none.
    let mut c = entry("a", "CC-BY-4.0", Storage::Stored);
    c.credit = None;
    assert!(rule(c).contains("credit"));
    let mut c = entry("a", "CC0-1.0", Storage::Stored);
    c.credit = None;
    assert!(parsed(vec![c]).is_ok());
    let mut c = entry("a", "CC0-1.0 OR MIT", Storage::Stored);
    c.credit = None;
    assert!(rule(c).contains("credit"));
}

#[test]
fn a_source_is_pinned() {
    let mut c = entry("a", "MIT", Storage::Fetched);
    c.fetch = Some(Fetch::Git {
        url: "u".to_owned(),
        commit: "main".to_owned(),
        paths: vec!["x".to_owned()],
    });
    assert!(rule(c.clone()).contains("full commit"));
    c.fetch = Some(Fetch::Git {
        url: "u".to_owned(),
        commit: "g".repeat(40),
        paths: vec!["x".to_owned()],
    });
    assert!(rule(c.clone()).contains("full commit"));
    c.fetch = Some(Fetch::Git {
        url: "u".to_owned(),
        commit: "a".repeat(40),
        paths: Vec::new(),
    });
    assert!(rule(c.clone()).contains("paths"));
    c.fetch = Some(Fetch::Files(Vec::new()));
    assert!(rule(c).contains("at least one"));
}

#[test]
fn nothing_ambiguous_parses() {
    assert!(matches!(Manifest::parse("{"), Err(CorpusError::Json(_))));
    let m = Manifest {
        format: 2,
        about: "x".to_owned(),
        corpora: Vec::new(),
    };
    assert_eq!(
        Manifest::parse(&serde_json::to_string(&m).unwrap()),
        Err(CorpusError::Format(2))
    );
    let text = serde_json::to_string(&Manifest {
        format: 1,
        about: "x".to_owned(),
        corpora: vec![entry("a", "MIT", Storage::Rebuilt)],
    })
    .unwrap();
    assert!(Manifest::parse(&text).is_ok());
    assert!(matches!(
        Manifest::parse(&text.replace("\"about\"", "\"note\"")),
        Err(CorpusError::Json(_))
    ));
    assert!(matches!(
        Manifest::parse(&text.replace("\"rebuilt\"", "\"maybe\"")),
        Err(CorpusError::Json(_))
    ));
}

/// A fresh, empty folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("corpus-test-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn tree(dir: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let full = dir.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    }
}

#[test]
fn a_tree_has_one_digest() {
    let one = [("b/x".to_owned(), vec![1, 2]), ("a".to_owned(), vec![3])];
    let two = [one[1].clone(), one[0].clone()];
    assert_eq!(digest_of(&one), digest_of(&two), "order does not matter");
    let mut changed = one.clone();
    changed[0].1 = vec![1, 3];
    assert_ne!(digest_of(&one), digest_of(&changed));
    let mut moved = one.clone();
    moved[1].0 = "c".to_owned();
    assert_ne!(digest_of(&one), digest_of(&moved));
    // A file's digest, size and path in the stated line, one line per file.
    let line = format!("{} 3 a\n", String::from(Hash(sha256(b"abc"))));
    assert_eq!(
        digest_of(&[("a".to_owned(), b"abc".to_vec())]),
        Hash(sha256(line.as_bytes()))
    );
    let dir = scratch("digest");
    tree(&dir, &[("b/x", "\u{1}\u{2}"), ("a", "\u{3}")]);
    assert_eq!(tree_digest(&dir).unwrap(), digest_of(&one));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_vendored_folder_holds_to_the_register() {
    let dir = scratch("vendored");
    tree(
        &dir,
        &[
            ("manifest.json", "{}"),
            ("README.md", "x"),
            ("ok/model.obj", "v 0 0 0"),
        ],
    );
    let mut ok = entry("ok", "CC0-1.0", Storage::Stored);
    ok.sha256 = Some(tree_digest(&dir.join("ok")).unwrap());
    let fetched = entry("fetched", "MIT", Storage::Fetched);
    let m = parsed(vec![ok, fetched]).unwrap();
    assert_eq!(check_vendored(&m, &dir).unwrap(), Vec::<String>::new());
    // A file changed.
    tree(&dir, &[("ok/model.obj", "v 0 0 1")]);
    assert!(check_vendored(&m, &dir).unwrap()[0].contains("digest"));
    tree(&dir, &[("ok/model.obj", "v 0 0 0")]);
    // An unlisted file beside it, in its folder: the digest moves.
    tree(&dir, &[("ok/other.obj", "x")]);
    assert!(check_vendored(&m, &dir).unwrap()[0].contains("digest"));
    fs::remove_file(dir.join("ok/other.obj")).unwrap();
    // A file no corpus lists, and a folder no corpus lists.
    tree(&dir, &[("stray.obj", "x")]);
    assert!(check_vendored(&m, &dir).unwrap()[0].contains("no corpus lists this file"));
    fs::remove_file(dir.join("stray.obj")).unwrap();
    tree(&dir, &[("unknown/a", "x")]);
    assert!(check_vendored(&m, &dir).unwrap()[0].contains("no corpus lists this folder"));
    fs::remove_dir_all(dir.join("unknown")).unwrap();
    // A corpus that may only be fetched has no business here.
    tree(&dir, &[("fetched/a", "x")]);
    assert!(check_vendored(&m, &dir).unwrap()[0].contains("may not be stored"));
    fs::remove_dir_all(&dir).unwrap();
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "protocol.file.allow=always",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// A local repository to fetch from, and its commit.
fn repository(dir: &Path) -> String {
    tree(
        dir,
        &[
            ("models/a.obj", "v 1"),
            ("models/b/c.obj", "v 2"),
            ("code/x.rs", "fn x() {}"),
        ],
    );
    git(dir, &["init", "--quiet", "--initial-branch=main"]);
    git(dir, &["config", "uploadpack.allowFilter", "true"]);
    git(dir, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
    git(dir, &["add", "."]);
    git(dir, &["commit", "--quiet", "-m", "one"]);
    git(dir, &["rev-parse", "HEAD"])
}

#[test]
fn a_git_source_is_fetched_by_its_commit_and_its_digest() {
    let root = scratch("git");
    let upstream = root.join("upstream");
    fs::create_dir_all(&upstream).unwrap();
    let commit = repository(&upstream);
    let cache = root.join("cache");
    let mut c = entry("models", "MIT", Storage::Fetched);
    c.fetch = Some(Fetch::Git {
        url: format!("file://{}", upstream.display()),
        commit,
        paths: vec!["models".to_owned()],
    });
    // The digest a new entry takes, of the paths named and nothing else.
    let digest = digest_source(&c, &root.join("scratch")).unwrap();
    let want = digest_of(&[
        ("models/a.obj".to_owned(), b"v 1".to_vec()),
        ("models/b/c.obj".to_owned(), b"v 2".to_vec()),
    ]);
    assert_eq!(digest, want);
    c.sha256 = Some(digest);
    assert_eq!(fetch(&c, &cache).unwrap(), Fetched::Downloaded);
    assert!(cache.join("models/models/a.obj").is_file());
    assert!(!cache.join("models/code").exists());
    assert!(!cache.join("models/.git").exists());
    // A good copy is kept; a damaged one is replaced.
    assert_eq!(fetch(&c, &cache).unwrap(), Fetched::Cached);
    fs::write(cache.join("models/models/a.obj"), "tampered").unwrap();
    assert_eq!(fetch(&c, &cache).unwrap(), Fetched::Downloaded);
    assert_eq!(
        fs::read_to_string(cache.join("models/models/a.obj")).unwrap(),
        "v 1"
    );
    // Bytes that are not the register's are refused and left nowhere.
    c.sha256 = Some(Hash([9; 32]));
    let error = fetch(&c, &cache).unwrap_err();
    assert!(error.contains("digest"), "{error}");
    assert!(!cache.join("models").exists());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_file_source_is_checked_file_by_file() {
    let root = scratch("files");
    tree(&root, &[("up/one.bin", "one")]);
    let url = format!("file://{}", root.join("up/one.bin").display());
    let cache = root.join("cache");
    let mut c = entry("one", "MIT", Storage::Fetched);
    c.fetch = Some(Fetch::Files(vec![FetchFile {
        url: url.clone(),
        name: "one.bin".to_owned(),
        sha256: Hash(sha256(b"one")),
    }]));
    c.sha256 = Some(digest_of(&[("one.bin".to_owned(), b"one".to_vec())]));
    assert_eq!(fetch(&c, &cache).unwrap(), Fetched::Downloaded);
    // A file whose own hash is wrong stops the fetch and nothing stays.
    c.fetch = Some(Fetch::Files(vec![FetchFile {
        url,
        name: "one.bin".to_owned(),
        sha256: Hash([0; 32]),
    }]));
    fs::remove_dir_all(cache.join("one")).unwrap();
    assert!(fetch(&c, &cache).unwrap_err().contains("SHA-256"));
    assert!(!cache.join("one").exists());
    // A source that is not there is an error.
    c.fetch = Some(Fetch::Files(vec![FetchFile {
        url: format!("file://{}", root.join("gone").display()),
        name: "gone".to_owned(),
        sha256: Hash([0; 32]),
    }]));
    assert!(fetch(&c, &cache).is_err());
    // A corpus with no bytes has nothing to fetch.
    let rebuilt = entry("r", "MIT", Storage::Rebuilt);
    assert!(fetch(&rebuilt, &cache).is_err());
    assert!(digest_source(&rebuilt, &cache).is_err());
    fs::remove_dir_all(&root).unwrap();
}
