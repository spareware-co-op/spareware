//! Attacks on piece G16 (the source ledger, the notices, the method notes and the banned-source
//! scan). Each test states what the board's done-when, chapter 29.20's rules 2 and 4 to 7, or a
//! guarantee written in `geom_test::ledger` or `geom_test::scan` requires, and tries to break it:
//! every marker of every table in every disguise the scanner claims to see through, every
//! taking phrase with every banned source, the reach of each negation, the ledger on records a
//! map and a header can get wrong, and the committed notes, ledger and notices checked against the
//! evidence and the map by a second, independent reading. A test that passes is a guard on a
//! guarantee the piece keeps.
//!
//! This file plants markers on purpose, so it is one of the scanner's named exemptions
//! (`geom_test::scan::repo::EXEMPT`).

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their fixtures.
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
use geom_test::map::json::Value;
use geom_test::map::{CrateInput, GAPS, Input, SourceFile, build};
use geom_test::scan::repo::{self as scan_repo, EXEMPT, Exempt, Kind};
use geom_test::scan::{Category, markers, scan_text};

// ---------------------------------------------------------------------------------------------
// The scanner: helpers.
// ---------------------------------------------------------------------------------------------

/// Whether `text` holds a hit of `category` on `line`.
fn found(text: &str, line: usize, category: Category) -> bool {
    scan_text(text)
        .iter()
        .any(|h| h.line == line && h.category == category)
}

/// Asserts that every text in `texts` holds a hit of `category` on its `line`.
fn all_found(category: Category, texts: &[(String, usize)]) {
    let missed: Vec<&(String, usize)> = texts
        .iter()
        .filter(|(text, line)| !found(text, *line, category))
        .collect();
    assert!(missed.is_empty(), "{category:?} not found in {missed:#?}");
}

/// Asserts that no text in `texts` holds any hit.
fn none_found(texts: &[String]) {
    let wrong: Vec<(&String, Vec<String>)> = texts
        .iter()
        .map(|t| (t, scan_text(t).iter().map(ToString::to_string).collect()))
        .filter(|(_, hits): &(&String, Vec<String>)| !hits.is_empty())
        .collect();
    assert!(wrong.is_empty(), "should pass: {wrong:#?}");
}

/// A word with its first letter in capitals.
fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

// ---------------------------------------------------------------------------------------------
// The scanner: every marker, in every disguise the header claims to see through.
// ---------------------------------------------------------------------------------------------

#[test]
fn every_routine_name_is_found_however_it_is_dressed() {
    let mut texts = Vec::new();
    for name in markers::NR_ROUTINES {
        texts.push((format!("fn {name}(a: &mut [f64]) {{}}"), 1));
        texts.push((format!("\n\nCALL {}(A, N)", name.to_uppercase()), 3));
        texts.push((format!("// see {name}, below"), 1));
        texts.push((format!("let s = \"{name}\";"), 1));
        texts.push((format!("#include \"nr/{name}.c\""), 1));
        texts.push((format!("fn st_{name}_f64() {{}}"), 1));
        texts.push((format!("fn my{}Helper() {{}}", capitalised(name)), 1));
        // A trailing number, however many digits, and the book's precision prefixes.
        texts.push((format!("fn {name}7() {{}}"), 1));
        texts.push((format!("fn {name}64() {{}}"), 1));
        texts.push((format!("fn d{name}() {{}}"), 1));
        texts.push((format!("fn s{name}2() {{}}"), 1));
    }
    all_found(Category::NumericalRecipes, &texts);
}

#[test]
fn every_macro_and_variable_is_found() {
    let mut texts = Vec::new();
    for name in markers::NR_MACROS {
        texts.push((format!("#define {name} 1"), 1));
        texts.push((format!("let {} = 1;", name.to_lowercase()), 1));
        texts.push((format!("const STEP_{name}: f64 = 1.0;"), 1));
    }
    for name in markers::NR_MACROS_EXACT {
        texts.push((format!("x = {name}(a, b);"), 1));
    }
    for name in markers::NR_VARIABLES {
        texts.push((format!("let {name} = 0.0;"), 1));
        texts.push((format!("let {} = 0.0;", name.to_uppercase()), 1));
        texts.push((format!("let {name}_buffer = vec![];"), 1));
        texts.push((format!("let {name}Buffer = vec![];"), 1));
    }
    all_found(Category::NumericalRecipes, &texts);
    // The exact macros are exact: ordinary code says `sign` and `Sign`.
    none_found(&["fn sign(x: f64) {}".to_owned(), "Sign::Positive".to_owned()]);
}

/// `words` joined into one line, across lines with each comment mark, and as one identifier.
fn dressed(words: &[&str]) -> Vec<(String, usize)> {
    let mut out = vec![(words.join(" "), 1)];
    for mark in ["//!", "///", "//", "#", " *", ";", "--", "%"] {
        let sep = format!("\n{mark} ");
        out.push((format!("{mark} {}", words.join(&sep)), 1));
    }
    out.push((format!("let x = \"{}\";", words.join(" \\\n    ")), 1));
    out.push((words.join("_"), 1));
    let camel: String = words.iter().map(|w| capitalised(w)).collect();
    out.push((camel, 1));
    out
}

#[test]
fn every_phrase_is_found_across_lines_comment_marks_and_identifiers() {
    for phrase in markers::NR_PHRASES {
        all_found(Category::NumericalRecipes, &dressed(phrase));
    }
    for phrase in markers::COPYLEFT_NOTICES {
        all_found(Category::CopyleftNotice, &dressed(phrase));
    }
}

#[test]
fn every_taking_phrase_with_every_banned_source_is_found() {
    let mut texts = Vec::new();
    for taking in markers::TAKING {
        for source in markers::COPYLEFT_SOURCES {
            let source = source.join(" ");
            texts.push((format!("// {} {source}", taking.join(" ")), 1));
            texts.push((
                format!(
                    "//! {} the\n//! {}'s code",
                    capitalised(&taking.join(" ")),
                    source.to_uppercase()
                ),
                1,
            ));
            if let [head, tail] = taking {
                // The two words of a phrase, with words put between them.
                texts.push((format!("// {head}, with changes, {tail} {source}"), 1));
            }
        }
    }
    all_found(Category::TakenFromCopyleft, &texts);
}

#[test]
fn a_negation_cancels_a_taking_phrase_only_in_its_own_sentence() {
    let mut cancelled = Vec::new();
    let mut kept = Vec::new();
    for negation in markers::NEGATIONS {
        cancelled.push(format!("// {negation} code taken from CGAL"));
        kept.push((format!("// {negation} code. Taken from CGAL."), 1));
        kept.push((format!("// {negation} tests; taken from CGAL"), 1));
        kept.push((format!("// {negation} tests!\n// Taken from CGAL"), 2));
        for limit in markers::NEGATION_LIMITS {
            kept.push((format!("// {negation} {limit} taken from CGAL"), 1));
        }
    }
    none_found(&cancelled);
    all_found(Category::TakenFromCopyleft, &kept);
}

/// `n` filler words.
fn fill(n: usize) -> String {
    vec!["word"; n].join(" ")
}

#[test]
fn the_gap_window_and_reach_have_their_stated_sizes() {
    let taken = Category::TakenFromCopyleft;
    let gap = markers::TAKING_GAP;
    // The second word of a phrase at most TAKING_GAP words after the first.
    assert!(found(
        &format!("taken {} from CGAL", fill(gap - 1)),
        1,
        taken
    ));
    assert!(!found(&format!("taken {} from CGAL", fill(gap)), 1, taken));
    // The source within WINDOW words after the phrase.
    let window = markers::WINDOW;
    assert!(found(
        &format!("taken from {} CGAL", fill(window - 1)),
        1,
        taken
    ));
    assert!(!found(
        &format!("taken from {} CGAL", fill(window)),
        1,
        taken
    ));
    // A negation within NEGATION_REACH words before it.
    let reach = markers::NEGATION_REACH;
    assert!(!found(
        &format!("no {} taken from CGAL", fill(reach - 1)),
        1,
        taken
    ));
    assert!(found(
        &format!("no {} taken from CGAL", fill(reach)),
        1,
        taken
    ));
}

#[test]
fn every_autodesk_name_and_description_is_found() {
    let mut texts = Vec::new();
    for name in markers::ACAD_PATTERNS {
        texts.push((format!("*{name}, a pattern"), 1));
        texts.push((format!("Pattern::named(\"{}\")", name.to_lowercase()), 1));
        texts.push((
            format!("const P: &str = \"{}\";", name.replace('-', "_")),
            1,
        ));
        texts.push((format!("\n// {name}"), 2));
    }
    for n in 2..=15 {
        texts.push((format!("*ACAD_ISO{n:02}W100,"), 1));
        texts.push((format!("\"acad_iso{n:02}w100\""), 1));
    }
    all_found(Category::AcadPattern, &texts);

    let mut texts = Vec::new();
    for description in markers::ACAD_DESCRIPTIONS {
        texts.push((format!("*X, {description}"), 1));
        texts.push((format!("(\"X\", \"{}\")", description.to_uppercase()), 1));
        // A Rust string literal escapes every backslash.
        texts.push((
            format!("(\"X\", \"{}\")", description.replace('\\', "\\\\")),
            1,
        ));
        // Split at its last space, across two comment lines and across two plain lines.
        let (head, tail) = description.rsplit_once(' ').unwrap();
        texts.push((format!("//! {head}\n//! {tail}"), 1));
        texts.push((format!("# {head}\n#   {tail}"), 1));
        texts.push((format!("{head}\n{tail}"), 1));
    }
    all_found(Category::AcadPattern, &texts);
}

#[test]
fn the_constant_is_found_however_its_decimal_literal_is_written() {
    let mut texts = Vec::new();
    for literal in [
        "1.89e-4",
        "1.89E-4",
        "1.89e-04",
        "1.890e-4",
        "18.9e-5",
        "189e-6",
        "0.189e-3",
        "0.000189",
        "0.000_189",
        "1_89e-6",
        ".000189",
        "1.89e-4_f64",
        "1.89e-4f",
        "1.89e-4L",
        "1.89_e-4",
    ] {
        texts.push((format!("#define LIMIT {literal}"), 1));
        texts.push((format!("\nlet limit = -{literal};"), 2));
    }
    all_found(Category::NumericalRecipes, &texts);
    none_found(&[
        "let a = 1.89e-3; let b = 1.8900001e-4; let c = 1.89; let d = 0.0189;".to_owned(),
        "let e = x.000189; let f = v1.89e-4;".to_owned(),
    ]);
}

#[test]
fn the_documented_limits_are_real() {
    // The header's "Words, not meaning" names what the scan does not see. If one of these starts
    // being found, narrow that guarantee in scan.rs rather than delete the case.
    none_found(&[
        // A look-alike letter (Cyrillic es).
        "fn svd\u{0441}mp() {}".to_owned(),
        // Built at run time.
        "let name = concat!(\"svd\", \"cmp\");".to_owned(),
        // A hexadecimal float.
        "double limit = 0x1.8c7e28240b780p-13;".to_owned(),
        // The source named before the phrase that takes it.
        "// CGAL, from which this was taken.".to_owned(),
        // Paraphrased.
        "// This came over from CGAL more or less as it was.".to_owned(),
    ]);
}

#[test]
fn kernel_prose_about_banned_sources_passes() {
    none_found(&[
        "//! Consulted: OCCT `BRepTools_History`, LGPL-2.1-only WITH OCCT-exception-1.0,\n\
         //!   <https://github.com/Open-Cascade-SAS/OCCT/blob/53706f78a8640c4407bdcc6752df13a270b159f2/x.hxx>,\n\
         //!   for the history contract.\n\
         //!   Method note: `projects/engines/geometry/methods/occt-history-contract.md`."
            .to_owned(),
        "//! ORACLE: CGAL runs out of process and exchanges only files (29.20, rule 8).".to_owned(),
        "//! No code is taken from FreeCAD, and nothing was copied from OCCT.".to_owned(),
        "//! The method note describes FreeCAD's element map in plain words.".to_owned(),
        "//! A port of CORE-MATH's exp, MIT, with its notice kept; based on Ziv's method.".to_owned(),
        "//! Written from Golub and Van Loan; Brent's method; a Jacobi rotation; the sign.".to_owned(),
    ]);
}

#[test]
fn a_file_is_read_in_every_encoding() {
    let utf16 = |text: &str, big: bool, mark: bool| -> Vec<u8> {
        let mut out = Vec::new();
        if mark {
            out.extend(if big { [0xfe, 0xff] } else { [0xff, 0xfe] });
        }
        for unit in text.encode_utf16() {
            out.extend(if big {
                unit.to_be_bytes()
            } else {
                unit.to_le_bytes()
            });
        }
        out
    };
    let text = "// one\nfn svdcmp() {}\n";
    let cases: [(&str, Vec<u8>); 5] = [
        ("utf-8", text.as_bytes().to_vec()),
        ("utf-16le with its mark", utf16(text, false, true)),
        ("utf-16be with its mark", utf16(text, true, true)),
        ("utf-16le without a mark", utf16(text, false, false)),
        (
            "invalid bytes around it",
            b"\xff\xfe\x00// one\nfn svdcmp() {}\n\xc3".to_vec(),
        ),
    ];
    for (what, bytes) in cases {
        let files = vec![("a/b.rs".to_owned(), Ok(scan_repo::readings(&bytes)))];
        let findings = scan_repo::scan_files(&files, &[]);
        assert!(
            findings
                .iter()
                .any(|f| matches!(&f.kind, Kind::Hit(h) if h.line == 2 && h.category == Category::NumericalRecipes)),
            "{what}: {findings:#?}"
        );
    }
    // A zero byte that separates two words does not join them out of sight: every reading counts.
    let files = vec![(
        "a/b.rs".to_owned(),
        Ok(scan_repo::readings(b"abc\x00svdcmp")),
    )];
    assert_eq!(scan_repo::scan_files(&files, &[]).len(), 1);
}

#[test]
fn every_exemption_holds_a_marker_and_this_file_is_one() {
    let this = "projects/engines/geometry/crates/geom-test/tests/attack_g16.rs";
    assert!(EXEMPT.iter().any(|e| e.file == this));
    assert_ne!(scan_text(include_str!("attack_g16.rs")), []);
    // An exemption names one file exactly, never a folder or a prefix.
    let exempt = [Exempt {
        file: "projects/engines/geometry/crates/geom-x/src/a.rs",
        reason: "test",
    }];
    let files = vec![
        (
            "projects/engines/geometry/crates/geom-x/src/a.rs".to_owned(),
            Ok(vec!["fn svdcmp() {}".to_owned()]),
        ),
        (
            "projects/engines/geometry/crates/geom-x/src/a.rs.bak".to_owned(),
            Ok(vec!["fn svdcmp() {}".to_owned()]),
        ),
    ];
    let findings = scan_repo::scan_files(&files, &exempt);
    assert_eq!(findings.len(), 1, "{findings:#?}");
    assert_eq!(
        findings[0].file,
        "projects/engines/geometry/crates/geom-x/src/a.rs.bak"
    );
}

// ---------------------------------------------------------------------------------------------
// The cited Cash-Karp fixture.
// ---------------------------------------------------------------------------------------------

/// The fixture, as the repository holds it.
const CASH_KARP: &str = include_str!("scan/cash_karp.rs");

/// The fixture's header: its leading `//!` lines.
fn cash_karp_header() -> String {
    CASH_KARP
        .lines()
        .take_while(|l| l.starts_with("//!"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_cited_fixture_passes_and_cites_its_method_and_every_constant() {
    assert_eq!(scan_text(CASH_KARP), []);
    let header = cash_karp_header();
    // The method's primary sources, by DOI.
    for doi in ["10.1145/79505.79507", "10.1007/978-3-540-78862-1"] {
        assert!(header.contains(doi), "the header does not cite {doi}");
    }
    // Every scalar constant's value is derived in the header (rule 7: "every constant is derived
    // from the named primary source and justified in the module header").
    let mut constants = 0;
    for line in CASH_KARP.lines() {
        let Some(rest) = line.strip_prefix("pub const ") else {
            continue;
        };
        let Some((name, value)) = rest.split_once(" = ") else {
            continue;
        };
        let value = value.trim_end_matches(';');
        let Ok(number) = value.parse::<f64>() else {
            continue;
        };
        constants += 1;
        let written = format!("{number}");
        assert!(
            header.contains(&written),
            "the constant {name} = {value} is not derived in the header"
        );
    }
    assert_eq!(
        constants, 4,
        "LOWER_ORDER, SAFETY_FACTOR, LARGEST_GROWTH and SMALLEST_SHRINK"
    );
}

#[test]
fn the_cited_fixture_meets_the_module_header_grammar() {
    // Held to the same grammar as a kernel module, as a module of a seeded kernel crate.
    let input = Input {
        crates: vec![CrateInput {
            name: "geom-x".to_owned(),
            folder: KERNEL.to_owned(),
            manifest: Some(MANIFEST.to_owned()),
            files: vec![
                file(
                    &format!("{KERNEL}/src"),
                    "lib.rs",
                    &LIB.replace("pub mod noted;\npub mod ported;\n", "pub mod cash_karp;\n"),
                ),
                file(&format!("{KERNEL}/src"), "cash_karp.rs", CASH_KARP),
            ],
        }],
        ..Input::default()
    };
    let generated = build(&input, GAPS);
    let modules = generated
        .map
        .list("crates")
        .iter()
        .flat_map(|c| c.list("modules"))
        .filter(|m| m.text("file").ends_with("cash_karp.rs"))
        .count();
    assert_eq!(modules, 1, "the map reads the fixture as a module");
    let mine: Vec<String> = generated
        .problems
        .iter()
        .filter(|p| p.file.ends_with("cash_karp.rs"))
        .map(ToString::to_string)
        .collect();
    assert!(mine.is_empty(), "{mine:#?}");
}

// ---------------------------------------------------------------------------------------------
// The ledger: a seeded workspace.
// ---------------------------------------------------------------------------------------------

const KERNEL: &str = "projects/engines/geometry/crates/geom-x";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const NOTICES: &str = "projects/engines/geometry/notices";
const METHODS: &str = "projects/engines/geometry/methods";

const MANIFEST: &str =
    "[package]\nname = \"geom-x\"\nversion = \"0.0.0\"\nlicense = \"MPL-2.0 AND MIT\"\n";

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

/// A module that ports `a.c` and `b.c`, whose `Source:` record's origin lines are `origin`.
fn ported(origin: &str) -> String {
    format!(
        "//! A ported example.
//!
//! Guarantees: it is an example.
//!
//! Method: the example paper (<https://doi.org/10.1000/182>).
//!
//! Cost: constant.
//!
//! Source: PORT from Ample `a.c` and `b.c`, MIT,
//!   <https://github.com/ex/ample/blob/{SHA}/src/a.c>
{origin}//!   Changed for Spare Kit: rewritten in Rust.
//! SPDX-License-Identifier: MPL-2.0 AND MIT
"
    )
}

/// The good origin lines: each file with its copyright line.
fn good_origin() -> String {
    format!(
        "//!   Copyright (c) 2020 A. Author\n\
         //!   <https://github.com/ex/ample/blob/{SHA}/src/b.c>\n\
         //!   Copyright (c) 2021 A. Author\n"
    )
}

/// A module that consulted an LGPL source under a method note, and `name` under MIT.
fn noted(name: &str) -> String {
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
//! Consulted: {name}, MIT, <https://github.com/plain/lib/blob/{SHA}/p.rs>, for its tests.
"
    )
}

fn licence_text() -> String {
    format!(
        "Name: Ample\nLink: https://github.com/ex/ample/tree/{SHA}\nLicence: MIT\n\
         Licence file: https://github.com/ex/ample/blob/{SHA}/LICENSE\n\n\
         Licence text:\nPermission is granted.\n"
    )
}

const TEMPLATE: &str = include_str!("../../../methods/TEMPLATE.md");

/// The seeded note, recording `code` as the code written from it.
fn note(code: &str) -> String {
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
| Code written from it | {code} |

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

/// The good note's record of its code.
const CODE: &str =
    "`projects/engines/geometry/crates/geom-x/src/noted.rs`, by session B, without the source";

fn file(folder: &str, name: &str, text: &str) -> SourceFile {
    SourceFile {
        path: format!("{folder}/{name}"),
        text: Ok(text.to_owned()),
    }
}

/// A seeded workspace, its parts replaceable.
#[derive(Clone)]
struct Seeded {
    lib: String,
    ported: String,
    noted: String,
    notices: Vec<SourceFile>,
    notes: Vec<SourceFile>,
}

impl Seeded {
    fn good() -> Seeded {
        Seeded {
            lib: LIB.to_owned(),
            ported: ported(&good_origin()),
            noted: noted("Plain Lib"),
            notices: vec![file(NOTICES, "ample.txt", &licence_text())],
            notes: vec![
                file(METHODS, "TEMPLATE.md", TEMPLATE),
                file(METHODS, "lesser-method.md", &note(CODE)),
            ],
        }
    }

    fn input(&self) -> Input {
        Input {
            crates: vec![CrateInput {
                name: "geom-x".to_owned(),
                folder: KERNEL.to_owned(),
                manifest: Some(MANIFEST.to_owned()),
                files: vec![
                    file(&format!("{KERNEL}/src"), "lib.rs", &self.lib),
                    file(&format!("{KERNEL}/src"), "noted.rs", &self.noted),
                    file(&format!("{KERNEL}/src"), "ported.rs", &self.ported),
                ],
            }],
            ..Input::default()
        }
    }

    /// The ledger from `map`, a map of this workspace or one changed from it.
    fn ledger_of(&self, map: &Value, map_problems: usize) -> Ledger {
        ledger::build(&ledger::Input {
            map,
            map_problems,
            notices: &self.notices,
            notes: &self.notes,
        })
    }

    fn ledger(&self) -> Ledger {
        let generated = build(&self.input(), GAPS);
        assert_eq!(generated.problems, [], "the seeded headers are complete");
        self.ledger_of(&generated.map, 0)
    }
}

/// Asserts that `ledger` has a problem of `rule` in a file ending in `file` saying `needle`.
fn has_problem(ledger: &Ledger, file: &str, rule: Rule, needle: &str) {
    assert!(
        ledger
            .problems
            .iter()
            .any(|p| p.rule == rule && p.file.ends_with(file) && p.message.contains(needle)),
        "no {rule:?} in {file} saying {needle:?}: {:#?}",
        ledger.problems
    );
}

#[test]
fn the_seeded_workspace_is_clean() {
    let ledger = Seeded::good().ledger();
    assert_eq!(ledger.problems, []);
}

#[test]
fn windows_line_ends_in_any_input_give_the_same_ledger() {
    let good = Seeded::good();
    let crlf = |t: &str| t.replace('\n', "\r\n");
    let mut windows = good.clone();
    windows.lib = crlf(&good.lib);
    windows.ported = crlf(&good.ported);
    windows.noted = crlf(&good.noted);
    for f in windows.notices.iter_mut().chain(windows.notes.iter_mut()) {
        f.text = Ok(crlf(f.text.as_ref().unwrap()));
    }
    assert_eq!(good.ledger(), windows.ledger());
}

#[test]
fn an_origin_without_a_copyright_line_fails_and_a_run_of_links_shares_one() {
    // `b.c` linked last, with no line after it.
    let mut s = Seeded::good();
    s.ported = ported(&format!(
        "//!   Copyright (c) 2020 A. Author\n//!   <https://github.com/ex/ample/blob/{SHA}/src/b.c>\n"
    ));
    has_problem(
        &s.ledger(),
        "ported.rs",
        Rule::Notice,
        "no copyright line for the origin file",
    );
    // Two links, then the one copyright line both files carry: shared, and NOTICE says so twice.
    let mut s = Seeded::good();
    s.ported = ported(&format!(
        "//!   <https://github.com/ex/ample/blob/{SHA}/src/b.c>\n//!   Copyright (c) 2022 A. Author\n"
    ));
    let ledger = s.ledger();
    assert_eq!(ledger.problems, []);
    let n = &ledger.notice;
    for origin in ["src/a.c", "src/b.c"] {
        let line = format!("{SHA}/{origin}\n      Copyright (c) 2022 A. Author\n");
        assert!(n.contains(&line), "NOTICE lacks {line:?}:\n{n}");
    }
}

#[test]
fn a_note_must_record_every_module_that_names_it() {
    let mut s = Seeded::good();
    s.notes[1] = file(
        METHODS,
        "lesser-method.md",
        &note("`geom-x`'s module, by session B, without the source"),
    );
    has_problem(
        &s.ledger(),
        "lesser-method.md",
        Rule::Note,
        "geom-x/src/noted.rs names this note",
    );
}

#[test]
fn a_map_whose_sources_and_records_disagree_fails() {
    let s = Seeded::good();
    let generated = build(&s.input(), GAPS);
    let Value::Object(map) = &generated.map else {
        panic!("the map is an object");
    };
    // A source no file records.
    let mut extra = map.clone();
    let Some(Value::Array(sources)) = extra.get_mut("sources") else {
        panic!("the map has sources");
    };
    let mut ghost = sources[0].clone();
    if let Value::Object(g) = &mut ghost {
        g.insert(
            "link".to_owned(),
            Value::from(format!("https://github.com/ghost/lib/blob/{SHA}/g.rs")),
        );
        g.insert("name".to_owned(), Value::from("Ghost Lib"));
    }
    sources.push(ghost);
    has_problem(
        &s.ledger_of(&Value::Object(extra), 0),
        "MAP.json",
        Rule::Map,
        "Ghost Lib",
    );
    // A file's record that no source holds.
    let mut fewer = map.clone();
    let Some(Value::Array(sources)) = fewer.get_mut("sources") else {
        panic!("the map has sources");
    };
    sources.retain(
        |v| !matches!(v, Value::Object(o) if o.get("name") == Some(&Value::from("Plain Lib"))),
    );
    let ledger = s.ledger_of(&Value::Object(fewer), 0);
    has_problem(&ledger, "noted.rs", Rule::Map, "Plain Lib");
}

#[test]
fn a_crate_header_s_record_is_in_both_files() {
    let mut s = Seeded::good();
    s.lib = LIB.replace(
        "//! Sections: 29.\n",
        &format!(
            "//! Sections: 29.\n//!\n//! Consulted: Crate Lib, BSD-3-Clause,\n//!   \
             <https://github.com/crate/lib/blob/{SHA}/k.rs>, for its layout.\n"
        ),
    );
    let generated = build(&s.input(), GAPS);
    assert_eq!(generated.problems, [], "the seeded headers are complete");
    let ledger = s.ledger_of(&generated.map, 0);
    assert_eq!(ledger.problems, []);
    assert!(
        ledger
            .notice
            .contains("Crate Lib\n  Licence:      BSD-3-Clause"),
        "{}",
        ledger.notice
    );
    assert!(
        ledger
            .provenance
            .contains("`projects/engines/geometry/crates/geom-x/src/lib.rs`: for its layout."),
        "{}",
        ledger.provenance
    );
}

/// The number of cell separators in a Markdown table row: its `|` not escaped as `\|`.
fn separators(row: &str) -> usize {
    let mut count = 0;
    let mut escaped = false;
    for c in row.chars() {
        if c == '|' && !escaped {
            count += 1;
        }
        escaped = c == '\\' && !escaped;
    }
    count
}

#[test]
fn a_pipe_in_a_record_keeps_every_table_row_whole() {
    let mut s = Seeded::good();
    s.noted = noted("Pipe | Lib");
    let ledger = s.ledger();
    assert_eq!(ledger.problems, []);
    let rows: Vec<&str> = ledger
        .provenance
        .lines()
        .filter(|l| l.starts_with('|'))
        .collect();
    let row = rows.iter().find(|r| r.contains("Pipe")).unwrap();
    assert_eq!(separators(row), 7, "a six-column row: {row}");
    // Every row of every table has as many cells as its header.
    let mut width = 0;
    for line in ledger.provenance.lines() {
        if !line.starts_with('|') {
            width = 0;
            continue;
        }
        if width == 0 {
            width = separators(line);
        }
        assert_eq!(separators(line), width, "{line}");
    }
}

#[test]
fn a_committed_file_differing_by_any_byte_but_line_ends_is_stale() {
    let generated = "a\nb\n";
    assert!(ledger::compare("F", Some("a\r\nb\r\n"), generated).is_none());
    for committed in ["a\nb", "a\nb\n\n", "a \nb\n", "\u{feff}a\nb\n", "a\nB\n"] {
        assert!(
            ledger::compare("F", Some(committed), generated).is_some(),
            "{committed:?} is not stale"
        );
    }
}

#[test]
fn the_template_is_a_template_and_a_filled_template_is_a_note() {
    let (_, problems) = ledger::methods::parse("TEMPLATE.md", TEMPLATE, true);
    assert_eq!(problems, Vec::<String>::new());
    // Filling every placeholder with real text makes a note the check accepts.
    let filled: String = TEMPLATE
        .lines()
        .map(|line| {
            if line.starts_with("# Method note: <") {
                "# Method note: a method".to_owned()
            } else if line.starts_with("| Licence | <") {
                "| Licence | LGPL-2.1-or-later |".to_owned()
            } else if line.starts_with("| Pinned link | <") {
                format!("| Pinned link | https://github.com/lesser/lib/blob/{SHA}/c.cpp |")
            } else if let Some(at) = line.find("| <") {
                format!("{}| Written. |", &line[..at])
            } else if line.starts_with('<') {
                "Written in our own words.".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (_, problems) = ledger::methods::parse("filled.md", &filled, false);
    assert_eq!(problems, Vec::<String>::new(), "{filled}");
}

// ---------------------------------------------------------------------------------------------
// The committed tree, read a second way: the pull-request template, CI, the notes, the ledger.
// ---------------------------------------------------------------------------------------------

#[cfg(not(target_family = "wasm"))]
mod tree {
    use std::fs;
    use std::path::PathBuf;

    use geom_test::ledger::methods::{self, FIELDS};
    use geom_test::map::classes::{self, Class};
    use geom_test::map::repo::workspace_root;

    fn root() -> PathBuf {
        workspace_root().unwrap()
    }

    fn read(rel: &str) -> String {
        fs::read_to_string(root().join(rel))
            .unwrap()
            .replace("\r\n", "\n")
    }

    /// The text of chapter 29.
    fn chapter_29() -> String {
        read("projects/engines/geometry/architecture/29-sources-licences.md")
    }

    #[test]
    fn the_pull_request_template_has_rule_5_s_columns_and_rule_6_s_attestation() {
        let chapter = chapter_29();
        // Rule 5's list, as the chapter words it, then the column that holds each item.
        assert!(chapter.contains(
            "for each source, the pinned link, the licence, the class from 29.1, the files \
             affected and what was taken"
        ));
        let template = read(".github/pull_request_template.md");
        let header = template
            .lines()
            .find(|l| l.starts_with("| Source |"))
            .expect("an outside-sources table");
        let columns: Vec<&str> = header.trim_matches('|').split('|').map(str::trim).collect();
        assert_eq!(
            columns,
            [
                "Source",
                "Pinned link",
                "Licence",
                "Class (29.1)",
                "Files affected",
                "What was taken"
            ]
        );
        for class in ["DEPEND", "PORT", "PORT METHOD", "ORACLE", "STUDY"] {
            assert!(
                template.contains(class),
                "the template does not name the class {class}"
            );
        }
        // Rule 6: the session that read, the session that wrote and that the source was not in
        // its context (step 5), and the second reviewer's check for copied expression (step 4).
        assert!(chapter.contains(
            "The pull request names that session and states that the source was not in its context"
        ));
        let attestation = &template[template.find("## Session attestation").unwrap()..];
        for words in [
            "the session that read each source",
            "the session that wrote it",
            "not in its context",
            "second reviewer",
            "copied expression",
        ] {
            assert!(
                attestation.contains(words),
                "the attestation does not ask for {words:?}"
            );
        }
    }

    #[test]
    fn ci_runs_the_ledger_and_the_scan_on_every_runner() {
        let workflow = read(".github/workflows/kernel-dev.yml");
        let native = workflow.find("\n  native:").expect("the native job");
        let wasm = workflow.find("\n  wasm:").expect("the wasm job");
        for step in [
            "run: cargo run --locked -p geom-test --bin provenance -- check",
            "run: cargo run --locked -p geom-test --bin source-scan -- check",
            "run: cargo run --locked -p geom-test --bin map -- check",
        ] {
            let at = workflow
                .find(step)
                .unwrap_or_else(|| panic!("no step {step}"));
            assert!(native < at && at < wasm, "{step} is not in the native job");
        }
        let job = &workflow[native..wasm];
        for os in [
            "ubuntu-latest",
            "ubuntu-24.04-arm",
            "windows-latest",
            "windows-11-arm",
            "macos-latest",
            "macos-15-intel",
        ] {
            assert!(
                job.contains(&format!("os: {os}")),
                "the native job does not run on {os}"
            );
        }
    }

    /// Whether a file name is a Markdown file's.
    fn is_markdown(name: &str) -> bool {
        std::path::Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("md"))
    }

    /// Every note under `methods/`, apart from the template and the README, with its text.
    fn notes() -> Vec<(String, String)> {
        let dir = root().join("projects/engines/geometry/methods");
        let mut out: Vec<(String, String)> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| is_markdown(n) && n != "TEMPLATE.md" && n != "README.md")
            .map(|n| {
                let text = read(&format!("projects/engines/geometry/methods/{n}"));
                (n, text)
            })
            .collect();
        out.sort();
        out
    }

    /// A field of a note, read from its table directly.
    fn field<'a>(note: &'a str, name: &str) -> &'a str {
        note.lines()
            .find_map(|l| l.strip_prefix(&format!("| {name} | ")))
            .and_then(|v| v.strip_suffix(" |"))
            .unwrap_or_else(|| panic!("no field {name}"))
    }

    #[test]
    fn the_board_s_first_two_notes_are_there_and_follow_their_ledger_rows() {
        let notes = notes();
        let names: Vec<&str> = notes.iter().map(|(n, _)| n.as_str()).collect();
        for wanted in ["occt-history-contract.md", "freecad-naming-lessons.md"] {
            assert!(names.contains(&wanted), "no note {wanted}: {names:?}");
        }
        let chapter = chapter_29();
        for (row, note) in [
            (
                "| OCCT `TNaming` and `BRepTools_History` | LGPL-2.1 with the OCCT exception |",
                "occt-history-contract.md",
            ),
            (
                "| FreeCAD element map | LGPL-2.1-or-later |",
                "freecad-naming-lessons.md",
            ),
        ] {
            assert!(chapter.contains(row), "chapter 29 has no row {row}");
            let text = &notes.iter().find(|(n, _)| n == note).unwrap().1;
            let licence = field(text, "Licence");
            assert_eq!(
                classes::of(licence),
                Ok(Class::MethodOnly),
                "{note}: {licence}"
            );
            let ledger_row = field(text, "Ledger row");
            let row_name = row.trim_start_matches("| ").split(" |").next().unwrap();
            assert!(
                ledger_row.contains(row_name),
                "{note} does not name its row {row_name}"
            );
        }
    }

    #[test]
    fn every_note_s_reading_is_in_the_evidence_it_names() {
        // Rule 6, step 2: the session that read the source pinned the commit and listed the files.
        // Each note says which evidence reports hold that reading; every file and line range it
        // lists must be cited there at the pinned commit.
        for (name, note) in notes() {
            let (_, problems) = methods::parse(&name, &note, false);
            assert_eq!(problems, Vec::<String>::new(), "{name}");
            for f in FIELDS {
                assert!(!field(&note, f).is_empty(), "{name}: {f}");
            }
            let files_read = field(&note, "Files read");
            let commit = files_read
                .split('`')
                .nth(1)
                .filter(|c| c.len() == 40)
                .unwrap_or_else(|| panic!("{name}: Files read opens with the pinned commit"));
            assert!(
                field(&note, "Pinned link").contains(commit),
                "{name}: the pinned link's commit"
            );
            let read_by = field(&note, "Read by");
            let reports: String = read_by
                .split('`')
                .filter(|p| p.starts_with("projects/kit/notes/G01-evidence/") && is_markdown(p))
                .map(read)
                .collect();
            assert!(
                !reports.is_empty(),
                "{name}: Read by names no evidence report"
            );
            let pieces: Vec<&str> = files_read.split('`').collect();
            let mut checked = 0;
            for k in (3..pieces.len()).step_by(2) {
                let path = pieces[k];
                let after = pieces.get(k + 1).copied().unwrap_or_default();
                let prefix = format!("{commit}/{path}");
                assert!(
                    reports.contains(&prefix),
                    "{name}: {path} is not cited at {commit}"
                );
                // Every line number the reports cite for this file.
                let mut cited: Vec<String> = Vec::new();
                for (at, _) in reports.match_indices(&prefix) {
                    let rest = &reports[at + prefix.len()..];
                    let rest = rest.strip_prefix("?plain=1").unwrap_or(rest);
                    if let Some(range) = rest.strip_prefix("#L") {
                        let range: String = range
                            .chars()
                            .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == 'L')
                            .collect();
                        cited.extend(range.split("-L").map(str::to_owned));
                    }
                }
                let words: Vec<&str> = after.split_whitespace().collect();
                for w in words.windows(3) {
                    if w[1] == "to" {
                        let from = w[0].trim_matches(|c: char| !c.is_ascii_digit());
                        let to = w[2].trim_matches(|c: char| !c.is_ascii_digit());
                        if from.is_empty() || to.is_empty() {
                            continue;
                        }
                        assert!(
                            cited.iter().any(|c| c == from) && cited.iter().any(|c| c == to),
                            "{name}: {path} lines {from} to {to} are not cited in the reports \
                             (cited: {cited:?})"
                        );
                        checked += 1;
                    }
                }
            }
            assert!(checked > 0, "{name}: no line range checked");
        }
    }

    #[test]
    fn the_gap_g15_left_is_closed_by_a_note_true_to_its_record() {
        assert_eq!(geom_test::map::GAPS, &[]);
        let interval = read("projects/engines/geometry/crates/geom-num/src/interval.rs");
        assert!(interval.contains("Number_types/include/CGAL/FPU.h#L347-L358"));
        assert!(interval.contains(
            "Method note: `projects/engines/geometry/methods/cgal-nextafter-widening.md`"
        ));
        let note = read("projects/engines/geometry/methods/cgal-nextafter-widening.md");
        assert!(field(&note, "Files read").contains("lines 347 to 358"));
        assert!(
            field(&note, "Code written from it")
                .contains("projects/engines/geometry/crates/geom-num/src/interval.rs")
        );
        // The note says plainly that step 5 is not met for this module and that an owner decides.
        assert!(field(&note, "Code written from it").contains("is not met"));
    }

    #[test]
    fn every_source_of_the_committed_map_is_in_both_committed_files() {
        // Read MAP.json with serde_json, not the crate's own reader, and look for each source in
        // the committed PROVENANCE.md and NOTICE as text.
        let map: serde_json::Value =
            serde_json::from_str(&read("projects/engines/geometry/MAP.json")).unwrap();
        let provenance = read("projects/engines/geometry/PROVENANCE.md");
        let notice = read("projects/engines/geometry/NOTICE");
        let sources = map["sources"].as_array().unwrap();
        assert!(sources.len() > 20, "too few sources: {}", sources.len());
        let mut counts = [0_usize; 3];
        for source in sources {
            let take = source["take"].as_str().unwrap();
            let link = source["link"].as_str().unwrap();
            let name = source["name"].as_str().unwrap();
            let link_cell = format!("<{link}>");
            assert!(
                provenance.contains(&link_cell),
                "PROVENANCE.md lacks {link}"
            );
            for file in source["used_by"].as_array().unwrap() {
                let file = file.as_str().unwrap();
                assert!(
                    provenance.contains(&format!("`{file}`")),
                    "PROVENANCE.md lacks {file}"
                );
                assert!(
                    take == "depend" || notice.contains(file),
                    "NOTICE lacks {file}, which takes {name}"
                );
            }
            match take {
                "port" => {
                    counts[0] += 1;
                    assert!(
                        notice.contains(&format!("    {link}\n")),
                        "NOTICE lacks {link}"
                    );
                }
                "port-method" => {
                    counts[1] += 1;
                    assert!(
                        notice.contains(&format!(
                            "{name}\n  Licence:      {}\n  Link:         {link}\n",
                            source["licence"].as_str().unwrap()
                        )),
                        "NOTICE does not credit {name}"
                    );
                }
                "depend" => {
                    counts[2] += 1;
                    assert!(
                        notice.contains(&format!("  {name} ")),
                        "NOTICE lacks the crate {name}"
                    );
                }
                other => panic!("a source taken as {other}"),
            }
        }
        // The summary counts the same sources.
        for (class, count) in ["PORT", "PORT METHOD", "DEPEND"].iter().zip(counts) {
            assert!(
                provenance.contains(&format!("| {class} | {count} |")),
                "PROVENANCE.md's summary does not count {count} {class} source(s)"
            );
        }
    }
}
