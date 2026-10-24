//! The ledger (`PROVENANCE.md` and `NOTICE`) on seeded workspaces: a good one, and one defect
//! seeded at a time for each guarantee of `geom_test::ledger` and its modules.
//!
//! The workspaces are built in memory and read by the map (`geom_test::map::build`) exactly as
//! the real one is, so the ledger is tested on the records the map gives, never on a second
//! reading of headers. The last tests run the ledger on disk and through its binary.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "fixtures; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken fixture must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_test::ledger::{self, Ledger, Rule};
use geom_test::map::{CrateInput, GAPS, Input, SourceFile, build};

const KERNEL: &str = "projects/engines/geometry/crates/geom-x";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const SHA2: &str = "89abcdef0123456789abcdef0123456789abcdef";
const NOTICES: &str = "projects/engines/geometry/notices";
const METHODS: &str = "projects/engines/geometry/methods";

const MANIFEST: &str = "[package]\nname = \"geom-x\"\nversion = \"0.0.0\"\nlicense = \"MPL-2.0 AND MIT\"\n";

const LIB: &str = "//! Spare Kit geometry kernel: an example.
//!
//! One job: being an example for the ledger.
//!
//! Layer: 6, Arrangements.
//!
//! Invariants this crate keeps:
//!
//! - It keeps one.
//!
//! Sections: 29.
#![forbid(unsafe_code)]

pub mod noted;
pub mod ported;
";

fn ported() -> String {
    format!(
        "//! A ported example.
//!
//! Guarantees: it is an example.
//!
//! Method: the example paper (<https://doi.org/10.1000/182>).
//!
//! Cost: constant.
//!
//! Source: PORT from Ample `a.c` and Other `b.h`, MIT,
//!   <https://github.com/ex/ample/blob/{SHA}/src/a.c>
//!   Copyright (c) 2020 A. Author
//!   Authors: A. Author and C. Author.
//!   <https://github.com/other/lib/blob/{SHA2}/b.h>
//!   Copyright (c) 2021 B. Author
//!   Changed for Spare Kit: rewritten in Rust.
//! SPDX-License-Identifier: MPL-2.0 AND MIT
"
    )
}

fn noted() -> String {
    format!(
        "//! A module that implements a method.
//!
//! Guarantees: it is another example.
//!
//! Method: the method's paper (<https://doi.org/10.1000/183>).
//!
//! Cost: constant.
//!
//! Consulted: Lesser Lib `c.cpp`, LGPL-2.1-or-later,
//!   <https://github.com/lesser/lib/blob/{SHA}/c.cpp#L1-L9>, for its method.
//!   Method note: `projects/engines/geometry/methods/lesser-method.md`.
//! Consulted: Plain Lib, MIT, <https://github.com/plain/lib/blob/{SHA}/p.rs>, for its tests.
"
    )
}

fn notice(name: &str, repo: &str, sha: &str, licence: &str) -> String {
    format!(
        "Name: {name}\nLink: https://github.com/{repo}/tree/{sha}\nLicence: {licence}\n\
         Licence file: https://github.com/{repo}/blob/{sha}/LICENSE\n\n\
         Credit:\nThanks to everyone.\n\nLicence text:\nPermission is granted for {name}.\n"
    )
}

const TEMPLATE: &str = "# Method note: <the method>

| Field | Value |
| --- | --- |
| Source | <the source> |
| Licence | <its licence> |
| Ledger row | <the row> |
| Pinned link | <the link> |
| Files read | <the files> |
| Read by | <the session> |
| Written by | <the session> |
| Second review | <the review> |
| Code written from it | <the code> |

## The problem

<What it is for.>

## The method

<The steps.>

## Invariants

<The guarantees.>

## Failure cases

<Where it goes wrong.>

## Published sources

<The papers.>
";

fn note() -> String {
    format!(
        "# Method note: the lesser method

| Field | Value |
| --- | --- |
| Source | Lesser Lib, its method |
| Licence | LGPL-2.1-or-later |
| Ledger row | Chapter 29.9, a row |
| Pinned link | https://github.com/lesser/lib/blob/{SHA}/c.cpp |
| Files read | `c.cpp` lines 1 to 9 |
| Read by | Session A, 2026-10-24 |
| Written by | Session A, 2026-10-24 |
| Second review | The approval of the pull request that adds this note |
| Code written from it | `projects/engines/geometry/crates/geom-x/src/noted.rs`, by session B, without the source in its context |

## The problem

Something needs doing.

## The method

Do it in steps, in our words.

## Invariants

It stays done.

## Failure cases

It can fail when the input is empty.

## Published sources

A paper, <https://doi.org/10.1000/184>.
"
    )
}

/// One file under `folder`.
fn file(folder: &str, name: &str, text: &str) -> SourceFile {
    SourceFile {
        path: format!("{folder}/{name}"),
        text: Ok(text.to_owned()),
    }
}

/// The seeded workspace's files: the crate's sources, the licence texts and the notes.
struct Seeded {
    sources: Vec<(String, String)>,
    notices: Vec<SourceFile>,
    notes: Vec<SourceFile>,
}

impl Seeded {
    fn good() -> Seeded {
        Seeded {
            sources: vec![
                ("lib.rs".to_owned(), LIB.to_owned()),
                ("noted.rs".to_owned(), noted()),
                ("ported.rs".to_owned(), ported()),
            ],
            notices: vec![
                file(NOTICES, "README.md", "# Licence texts\n"),
                file(NOTICES, "ample.txt", &notice("Ample", "ex/ample", SHA, "MIT")),
                file(NOTICES, "other.txt", &notice("Other", "other/lib", SHA2, "MIT")),
            ],
            notes: vec![
                file(METHODS, "README.md", "# Method notes\n"),
                file(METHODS, "TEMPLATE.md", TEMPLATE),
                file(METHODS, "lesser-method.md", &note()),
            ],
        }
    }

    fn source(mut self, name: &str, text: String) -> Seeded {
        let slot = self.sources.iter_mut().find(|(n, _)| n == name).unwrap();
        slot.1 = text;
        self
    }

    fn ledger(&self) -> Ledger {
        let input = Input {
            crates: vec![CrateInput {
                name: "geom-x".to_owned(),
                folder: KERNEL.to_owned(),
                manifest: Some(MANIFEST.to_owned()),
                files: self
                    .sources
                    .iter()
                    .map(|(name, text)| file(&format!("{KERNEL}/src"), name, text))
                    .collect(),
            }],
            ..Input::default()
        };
        let generated = build(&input, GAPS);
        assert_eq!(generated.problems, [], "the seeded headers are complete");
        ledger::build(&ledger::Input {
            map: &generated.map,
            map_problems: generated.problems.len(),
            notices: &self.notices,
            notes: &self.notes,
        })
    }
}

/// The problems of a ledger, as (file, rule) pairs.
fn rules(ledger: &Ledger) -> Vec<(String, Rule)> {
    ledger
        .problems
        .iter()
        .map(|p| (p.file.clone(), p.rule))
        .collect()
}

/// Asserts that `ledger` has exactly one problem, of `rule`, in a file ending in `file`, whose
/// message holds `needle`.
fn one_problem(ledger: &Ledger, file: &str, rule: Rule, needle: &str) {
    assert_eq!(ledger.problems.len(), 1, "{:#?}", ledger.problems);
    let p = &ledger.problems[0];
    assert!(
        p.file.ends_with(file) && p.rule == rule && p.message.contains(needle),
        "expected {rule:?} in {file} saying {needle:?}: {p:#?}"
    );
}

#[test]
fn a_good_workspace_writes_every_record() {
    let ledger = Seeded::good().ledger();
    assert_eq!(ledger.problems, []);

    let n = &ledger.notice;
    // Both origin repositories of the port, each with its licence text, credit and the
    // copyright lines the ported file's header copies, under the right origin.
    for wanted in [
        "Ample\n-----",
        "Other\n-----",
        "Permission is granted for Ample.",
        "Permission is granted for Other.",
        "Thanks to everyone.",
        &format!("Commit:    {SHA}"),
        &format!("Commit:    {SHA2}"),
        "SPDX-License-Identifier: MPL-2.0 AND MIT",
        &format!(
            "    https://github.com/ex/ample/blob/{SHA}/src/a.c\n      Copyright (c) 2020 A. \
             Author\n      Authors: A. Author and C. Author.\n"
        ),
        &format!("    https://github.com/other/lib/blob/{SHA2}/b.h\n      Copyright (c) 2021 B. Author\n"),
    ] {
        assert!(n.contains(wanted), "NOTICE lacks {wanted:?}:\n{n}");
    }
    // The Ample entry lists only Ample's origin file, and the Other entry only Other's.
    let ample = &n[n.find("Ample\n-----").unwrap()..n.find("Other\n-----").unwrap()];
    assert!(!ample.contains("other/lib"), "{ample}");
    // Both consulted sources are credited, with their purposes and the method note.
    for wanted in [
        "Lesser Lib `c.cpp`\n  Licence:      LGPL-2.1-or-later",
        "  Method note:  projects/engines/geometry/methods/lesser-method.md",
        "      for its method.",
        "Plain Lib\n  Licence:      MIT",
        "      for its tests.",
    ] {
        assert!(n.contains(wanted), "NOTICE lacks {wanted:?}:\n{n}");
    }

    let p = &ledger.provenance;
    for wanted in [
        "| PORT | 1 | 1 |",
        "| PORT METHOD | 2 | 1 |",
        "| DEPEND | 0 | 0 |",
        "1 method note(s)",
        "| Ample `a.c` and Other `b.h` | MIT | allowed |",
        "| `projects/engines/geometry/crates/geom-x/src/ported.rs` | rewritten in Rust. |",
        "| Lesser Lib `c.cpp` | LGPL-2.1-or-later | method only |",
        "| `projects/engines/geometry/methods/lesser-method.md` |",
        "`projects/engines/geometry/crates/geom-x/src/noted.rs`: for its method. |",
        "| [the lesser method](methods/lesser-method.md) | Lesser Lib, its method |",
        "| (none) | | | | | |",
    ] {
        assert!(p.contains(wanted), "PROVENANCE.md lacks {wanted:?}:\n{p}");
    }
    assert!(p.ends_with("`projects/engines/geometry/crates/geom-x/src/noted.rs` |\n"));
}

#[test]
fn the_ledger_is_the_same_whatever_order_its_inputs_come_in() {
    let good = Seeded::good();
    let first = good.ledger();
    assert_eq!(first, good.ledger());
    let mut shuffled = Seeded::good();
    shuffled.notices.reverse();
    shuffled.notes.reverse();
    shuffled.sources.reverse();
    assert_eq!(first, shuffled.ledger());
}

// ---------------------------------------------------------------------------------------------
// Licence texts.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_port_without_its_licence_text_fails() {
    let mut s = Seeded::good();
    s.notices.retain(|f| !f.path.ends_with("other.txt"));
    let ledger = s.ledger();
    one_problem(&ledger, "ported.rs", Rule::Notice, "other/lib");
    assert!(ledger.notice.contains("Licence text: MISSING"));
}

#[test]
fn a_licence_text_no_port_uses_fails() {
    let mut s = Seeded::good();
    s.notices
        .push(file(NOTICES, "unused.txt", &notice("Unused", "un/used", SHA, "MIT")));
    one_problem(&s.ledger(), "unused.txt", Rule::Notice, "not owed");
    // A licence text at another commit of a ported repository is not the port's either.
    let mut s = Seeded::good();
    s.notices
        .retain(|f| !f.path.ends_with("ample.txt"));
    s.notices
        .push(file(NOTICES, "ample.txt", &notice("Ample", "ex/ample", SHA2, "MIT")));
    let r = rules(&s.ledger());
    // Problems sort by file: the crate's folder comes before the licence texts' folder.
    assert_eq!(
        r,
        [
            (format!("{KERNEL}/src/ported.rs"), Rule::Notice),
            (format!("{NOTICES}/ample.txt"), Rule::Notice),
        ]
    );
}

#[test]
fn two_licence_texts_for_one_source_fail() {
    let mut s = Seeded::good();
    s.notices
        .push(file(NOTICES, "ample2.txt", &notice("Ample", "ex/ample", SHA, "MIT")));
    let r = rules(&s.ledger());
    assert_eq!(
        r,
        [
            (format!("{NOTICES}/ample.txt"), Rule::Notice),
            (format!("{NOTICES}/ample2.txt"), Rule::Notice),
        ]
    );
}

#[test]
fn a_licence_text_under_another_licence_fails() {
    let mut s = Seeded::good();
    s.notices.retain(|f| !f.path.ends_with("other.txt"));
    s.notices
        .push(file(NOTICES, "other.txt", &notice("Other", "other/lib", SHA2, "BSD-3-Clause")));
    one_problem(&s.ledger(), "other.txt", Rule::Notice, "stop and ask an owner");
}

#[test]
fn a_malformed_licence_text_or_a_stray_file_fails() {
    let mut s = Seeded::good();
    s.notices.retain(|f| !f.path.ends_with("other.txt"));
    s.notices.push(file(
        NOTICES,
        "other.txt",
        &notice("Other", "other/lib", SHA2, "MIT").replace("Licence text:", "Text:"),
    ));
    let ledger = s.ledger();
    assert!(
        ledger
            .problems
            .iter()
            .any(|p| p.file.ends_with("other.txt") && p.message.contains("Licence text:")),
        "{:#?}",
        ledger.problems
    );
    let mut s = Seeded::good();
    s.notices.push(file(NOTICES, "notes.md", "# Notes\n"));
    one_problem(&s.ledger(), "notes.md", Rule::Notice, "only licence texts");
}

#[test]
fn an_unpinned_further_origin_fails() {
    let s = Seeded::good().source(
        "ported.rs",
        ported().replace(&format!("other/lib/blob/{SHA2}/"), "other/lib/blob/main/"),
    );
    let ledger = s.ledger();
    assert!(
        ledger
            .problems
            .iter()
            .any(|p| p.file.ends_with("ported.rs") && p.message.contains("pinned")),
        "{:#?}",
        ledger.problems
    );
}

// ---------------------------------------------------------------------------------------------
// Method notes.
// ---------------------------------------------------------------------------------------------

/// The good workspace with its note's text changed by `change`.
fn with_note(change: impl Fn(String) -> String) -> Ledger {
    let mut s = Seeded::good();
    s.notes.retain(|f| !f.path.ends_with("lesser-method.md"));
    s.notes.push(file(METHODS, "lesser-method.md", &change(note())));
    s.ledger()
}

#[test]
fn a_note_must_keep_the_template() {
    let cases: [(&str, Box<dyn Fn(String) -> String>); 12] = [
        ("does not open", Box::new(|n| n.replacen("# Method note: ", "# Note: ", 1))),
        (
            "\"Read by\" is missing",
            Box::new(|n| n.replace("| Read by | Session A, 2026-10-24 |\n", "")),
        ),
        (
            "written twice",
            Box::new(|n| n.replace("| Read by |", "| Read by | Twice |\n| Read by |")),
        ),
        (
            "\"Second review\" is missing, states nothing or is still the template's placeholder",
            Box::new(|n| {
                n.replace(
                    "The approval of the pull request that adds this note",
                    "<the review>",
                )
            }),
        ),
        (
            "\"Files read\" is missing",
            Box::new(|n| n.replace("`c.cpp` lines 1 to 9", "TBD")),
        ),
        ("class \"allowed\"", Box::new(|n| n.replace("| LGPL-2.1-or-later |", "| MIT |"))),
        (
            "does not name a full commit",
            Box::new(|n| n.replace(&format!("blob/{SHA}/c.cpp"), "blob/main/c.cpp")),
        ),
        (
            "\"## Invariants\" is missing or out of order",
            Box::new(|n| n.replace("## Invariants\n\nIt stays done.\n\n", "")),
        ),
        (
            "\"## The method\" is missing or out of order",
            Box::new(|n| {
                n.replace(
                    "## The problem\n\nSomething needs doing.\n\n## The method\n\nDo it in steps, \
                     in our words.\n\n",
                    "## The method\n\nDo it in steps, in our words.\n\n## The problem\n\n\
                     Something needs doing.\n\n",
                )
            }),
        ),
        (
            "\"## Failure cases\" is empty",
            Box::new(|n| n.replace("It can fail when the input is empty.\n", "")),
        ),
        (
            "fenced code block",
            Box::new(|n| n.replace("Do it in steps, in our words.", "```\nfn f() {}\n```")),
        ),
        (
            "indented code block",
            Box::new(|n| n.replace("Do it in steps, in our words.", "Steps:\n\n    f(x)")),
        ),
    ];
    for (needle, change) in cases {
        let ledger = with_note(change);
        assert!(
            ledger.problems.iter().any(|p| p.rule == Rule::Note
                && p.file.ends_with("lesser-method.md")
                && p.message.contains(needle)),
            "{needle:?} not reported: {:#?}",
            ledger.problems
        );
    }
}

#[test]
fn a_record_must_credit_the_source_its_note_describes() {
    let ledger = with_note(|n| n.replace("| LGPL-2.1-or-later |", "| LGPL-3.0-or-later |"));
    one_problem(&ledger, "lesser-method.md", Rule::Note, "credits \"LGPL-2.1-or-later\"");
    let ledger = with_note(|n| n.replace("github.com/lesser/lib/", "github.com/other/repo/"));
    one_problem(&ledger, "lesser-method.md", Rule::Note, "not the repository");
}

#[test]
fn the_template_is_held_to_the_same_fields() {
    let mut s = Seeded::good();
    s.notes.retain(|f| !f.path.ends_with("TEMPLATE.md"));
    one_problem(&s.ledger(), &format!("{METHODS}/"), Rule::Note, "no TEMPLATE.md");
    let mut s = Seeded::good();
    s.notes.retain(|f| !f.path.ends_with("TEMPLATE.md"));
    s.notes.push(file(
        METHODS,
        "TEMPLATE.md",
        &TEMPLATE.replace("| <the source> |", "| A real source |"),
    ));
    one_problem(&s.ledger(), "TEMPLATE.md", Rule::Note, "\"Source\" must hold a <placeholder>");
    let mut s = Seeded::good();
    s.notes.retain(|f| !f.path.ends_with("TEMPLATE.md"));
    s.notes.push(file(
        METHODS,
        "TEMPLATE.md",
        &TEMPLATE.replace("## Published sources\n\n<The papers.>\n", ""),
    ));
    one_problem(&s.ledger(), "TEMPLATE.md", Rule::Note, "Published sources");
    let mut s = Seeded::good();
    s.notes.push(file(METHODS, "draft.txt", "notes\n"));
    one_problem(&s.ledger(), "draft.txt", Rule::Note, "only method notes");
}

// ---------------------------------------------------------------------------------------------
// The map, and staleness.
// ---------------------------------------------------------------------------------------------

#[test]
fn an_incomplete_map_fails_the_ledger() {
    let s = Seeded::good();
    let input = Input {
        crates: vec![CrateInput {
            name: "geom-x".to_owned(),
            folder: KERNEL.to_owned(),
            manifest: Some(MANIFEST.to_owned()),
            files: vec![file(&format!("{KERNEL}/src"), "lib.rs", "pub mod a;\n")],
        }],
        ..Input::default()
    };
    let generated = build(&input, GAPS);
    assert!(!generated.problems.is_empty());
    let ledger = ledger::build(&ledger::Input {
        map: &generated.map,
        map_problems: generated.problems.len(),
        notices: &s.notices,
        notes: &s.notes,
    });
    assert!(
        ledger
            .problems
            .iter()
            .any(|p| p.rule == Rule::Map && p.message.contains("map -- check")),
        "{:#?}",
        ledger.problems
    );
}

#[test]
fn every_rule_has_a_seeded_case() {
    // Notice, Note and Map above; Stale in ledger_files below and in the unit tests of `compare`.
    assert_eq!(
        Rule::ALL,
        [Rule::Notice, Rule::Note, Rule::Map, Rule::Stale]
    );
    for rule in Rule::ALL {
        assert!(!rule.label().is_empty());
    }
}

#[cfg(not(target_family = "wasm"))]
mod files {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use geom_test::ledger::{NOTICE_FILE, PROVENANCE_FILE, Rule, repo};

    use super::{KERNEL, LIB, MANIFEST, METHODS, NOTICES, SHA, SHA2, TEMPLATE, note, noted, notice, ported};

    /// A scratch workspace under `CARGO_TARGET_TMPDIR`, emptied first and removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ledger-{name}"));
            let _ = fs::remove_dir_all(&root);
            let files: [(String, String); 10] = [
                (format!("{KERNEL}/Cargo.toml"), MANIFEST.to_owned()),
                (format!("{KERNEL}/src/lib.rs"), LIB.to_owned()),
                (format!("{KERNEL}/src/ported.rs"), ported()),
                (format!("{KERNEL}/src/noted.rs"), noted()),
                (format!("{NOTICES}/ample.txt"), notice("Ample", "ex/ample", SHA, "MIT")),
                (format!("{NOTICES}/other.txt"), notice("Other", "other/lib", SHA2, "MIT")),
                (format!("{METHODS}/README.md"), "# Notes\n".to_owned()),
                (format!("{METHODS}/TEMPLATE.md"), TEMPLATE.to_owned()),
                (format!("{METHODS}/lesser-method.md"), note()),
                ("Cargo.toml".to_owned(), "[workspace]\n".to_owned()),
            ];
            for (rel, text) in files {
                let path = root.join(rel);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, text).unwrap();
            }
            // The map is committed too, so the map's own check passes here.
            geom_test::map::repo::write(&root, &[]).unwrap();
            Scratch(root)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn write_then_check_passes_and_a_change_is_stale() {
        let s = Scratch::new("cycle");
        let found = repo::check(&s.0).unwrap();
        let stale: Vec<&str> = found
            .iter()
            .filter(|p| p.rule == Rule::Stale)
            .map(|p| p.file.as_str())
            .collect();
        assert_eq!(stale, [NOTICE_FILE, PROVENANCE_FILE], "{found:#?}");
        let (problems, written) = repo::write(&s.0).unwrap();
        assert_eq!(problems, []);
        assert_eq!(written, [PROVENANCE_FILE, NOTICE_FILE]);
        let (_, written) = repo::write(&s.0).unwrap();
        assert!(written.is_empty(), "unchanged files are not rewritten");
        assert_eq!(repo::check(&s.0).unwrap(), []);
        // A Windows checkout's line ends do not make the files stale.
        let notice_path = s.0.join(NOTICE_FILE);
        let crlf = fs::read_to_string(&notice_path).unwrap().replace('\n', "\r\n");
        fs::write(&notice_path, crlf).unwrap();
        assert_eq!(repo::check(&s.0).unwrap(), []);
        // A changed licence text makes NOTICE stale.
        fs::write(
            s.0.join(NOTICES).join("ample.txt"),
            notice("Ample", "ex/ample", SHA, "MIT").replace("everyone", "all"),
        )
        .unwrap();
        let found = repo::check(&s.0).unwrap();
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!((found[0].file.as_str(), found[0].rule), (NOTICE_FILE, Rule::Stale));
    }

    fn run(args: &[&str]) -> (i32, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_provenance"))
            .args(args)
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        (out.status.code().unwrap(), text)
    }

    #[test]
    fn the_binary_exits_0_1_and_2() {
        let s = Scratch::new("binary");
        let root = s.0.to_str().unwrap();
        let (code, text) = run(&["check", "--root", root]);
        assert_eq!(code, 1, "nothing written yet: {text}");
        assert!(text.contains("Stale ledger: the file is missing"), "{text}");
        let (code, text) = run(&["write", "--root", root]);
        assert_eq!(code, 0, "{text}");
        assert!(text.contains("wrote projects/engines/geometry/NOTICE"), "{text}");
        let (code, text) = run(&["check", "--root", root]);
        assert_eq!(code, 0, "{text}");
        fs::remove_file(s.0.join(NOTICES).join("other.txt")).unwrap();
        let (code, text) = run(&["check", "--root", root]);
        assert_eq!(code, 1, "{text}");
        assert!(text.contains("geom-x/src/ported.rs: Notice: "), "{text}");
        let (code, text) = run(&["write", "--root", root]);
        assert_eq!(code, 1, "write still fails while a problem remains: {text}");
        let (code, text) = run(&["frobnicate"]);
        assert_eq!(code, 2, "{text}");
        let (code, text) = run(&["check", "--root"]);
        assert_eq!(code, 2, "{text}");
        let empty = s.0.join("no-workspace-here");
        fs::create_dir_all(&empty).unwrap();
        let (code, text) = run(&["check", "--root", empty.to_str().unwrap()]);
        assert_eq!(code, 2, "a root without the crates folder is an error, not a pass: {text}");
    }
}
