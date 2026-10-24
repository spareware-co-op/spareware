//! Attacks on piece G15 (the header grammar, the `MAP.json` generator and its CI check). Each test
//! states what `projects/engines/geometry/HEADERS.md`, the board text, the kernel architecture
//! (chapters 22, 26.3, 27 and 29) or the principles require, and fails while the piece does not do
//! it. A test that passes is a guard on a guarantee the piece already keeps.
//!
//! The in-memory attacks build an [`Input`] the way `map_seeded.rs` does; the ones that need files,
//! links or the real tree run natively only.

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

use geom_test::map::{CrateInput, Gap, Input, Problem, Rule, SourceFile, build};

/// G09's crate-header reader, read in place (`engine-geometry/tests/scaffold/header.rs`), so the
/// claim that both readers agree on the four crate fields can be checked against the real code.
#[path = "../../../../crates/engine-geometry/tests/scaffold/header.rs"]
#[allow(dead_code, reason = "only the field reader is used here")]
mod g09_header;

/// What G09's header reader needs from its crate table: the layer type, as `table.rs` defines it.
mod table {
    /// A crate's layer: one of the thirteen of chapter 27's layer table, or development only.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum Layer {
        /// Layer 1 to 13.
        N(u8),
        /// The development crates.
        Development,
    }
}

const KERNEL: &str = "projects/engines/geometry/crates/geom-x";
const DEV: &str = "projects/engines/geometry/crates/geom-fuzz";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

const KERNEL_LIB: &str = "//! Spare Kit geometry kernel: an example.
//!
//! One job: examples of every field.
//!
//! Layer: 6, Arrangements.
//!
//! Invariants this crate keeps:
//!
//! - It keeps one.
//!
//! Sections: 11.3 and 22.
#![forbid(unsafe_code)]

pub mod a;
";

const DEV_LIB: &str = "//! Spare Kit geometry kernel, development only: an example.
//!
//! Job (chapter 28, section 28.5): fuzz harnesses.
//!
//! Layer: Development.
//!
//! Invariants this crate keeps:
//!
//! - Development only.
//!
//! Sections: 28.5.
";

/// A complete module header, with a body.
const MODULE: &str = "//! An example module.
//!
//! Guarantees:
//!
//! - Every value it returns is finite.
//!
//! Method: Shewchuk 1997 (<https://doi.org/10.1007/PL00009321>).
//!
//! Cost: constant time and no allocation.
//!
//! SPDX-License-Identifier: MPL-2.0

/// A function.
pub fn f() {}
";

/// A module header whose fields are complete, with `{TAIL}` replaced by its provenance lines.
const MODULE_WITH_TAIL: &str = "//! An example module.
//!
//! Guarantees:
//!
//! - Every value it returns is finite.
//!
//! Method: Shewchuk 1997 (<https://doi.org/10.1007/PL00009321>).
//!
//! Cost: constant time and no allocation.
//!
{TAIL}
/// A function.
pub fn f() {}
";

fn file(folder: &str, rel: &str, text: &str) -> SourceFile {
    SourceFile {
        path: format!("{folder}/src/{rel}"),
        text: Ok(text.to_owned()),
    }
}

/// A workspace of one kernel crate `geom-x` with `lib` as its root and `modules` (path under
/// `src/`, text) as its other files, and the development crate `geom-fuzz` with `dev_lib`.
///
/// (Fixer, G15: `geom-x`'s manifest names `MPL-2.0 AND BSD-3-Clause`, because chapter 29.20,
/// rule 4, asks a crate that ports a file to name the origin's licence beside MPL-2.0 in its
/// `license` field, which the map now checks; the control ports below are BSD-3-Clause.)
fn workspace_with(lib: &str, modules: &[(&str, &str)], dev_lib: &str) -> Input {
    let mut files = vec![file(KERNEL, "lib.rs", lib)];
    files.extend(modules.iter().map(|(rel, text)| file(KERNEL, rel, text)));
    Input {
        crates: vec![
            CrateInput {
                name: "geom-x".into(),
                folder: KERNEL.into(),
                manifest: Some(
                    "[package]\nname = \"geom-x\"\nversion = \"0.0.0\"\n\
                     license = \"MPL-2.0 AND BSD-3-Clause\"\n"
                        .into(),
                ),
                files,
            },
            CrateInput {
                name: "geom-fuzz".into(),
                folder: DEV.into(),
                manifest: Some("[package]\nname = \"geom-fuzz\"\n".into()),
                files: vec![file(DEV, "lib.rs", dev_lib)],
            },
        ],
        ..Input::default()
    }
}

fn workspace(lib: &str, modules: &[(&str, &str)]) -> Input {
    workspace_with(lib, modules, DEV_LIB)
}

/// The problems of the workspace whose module `a.rs` has `text`.
fn module_problems(text: &str) -> Vec<Problem> {
    build(&workspace(KERNEL_LIB, &[("a.rs", text)]), &[]).problems
}

/// `lines` as a `//!` header, one `//! ` line each.
fn as_header<'a>(lines: impl IntoIterator<Item = &'a str>) -> String {
    lines.into_iter().fold(String::new(), |mut out, l| {
        out.push_str("//! ");
        out.push_str(l);
        out.push('\n');
        out
    })
}

/// The problems of a module whose provenance lines are `tail` (each line without its `//! `).
fn tail_problems(tail: &str) -> Vec<Problem> {
    module_problems(&MODULE_WITH_TAIL.replace("{TAIL}\n", &as_header(tail.lines())))
}

fn show(problems: &[Problem]) -> String {
    problems
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Asserts that every case in `cases` gives at least one problem, and reports every case that
/// gave none.
fn each_fails(what: &str, cases: &[(&str, Vec<Problem>)]) {
    let passed: Vec<&str> = cases
        .iter()
        .filter(|(_, p)| p.is_empty())
        .map(|(c, _)| *c)
        .collect();
    assert!(
        passed.is_empty(),
        "{what}: these cases pass the map check with no problem:\n{}",
        passed.join("\n")
    );
}

fn the_map(input: &Input) -> serde_json::Value {
    serde_json::from_str(&build(input, &[]).text).unwrap()
}

#[test]
fn the_baseline_fixture_is_clean() {
    // So that every problem an attack below expects comes from the attack, not the fixture.
    let generated = build(&workspace(KERNEL_LIB, &[("a.rs", MODULE)]), &[]);
    assert_eq!(generated.problems, [], "{}", show(&generated.problems));
    assert_eq!(tail_problems("SPDX-License-Identifier: MPL-2.0"), []);
}

// ---------------------------------------------------------------------------------------------
// The module and crate grammar.
// ---------------------------------------------------------------------------------------------

/// HEADERS.md: the module summary is "the job ... what the module is for". A header that opens
/// with its Guarantees paragraph has no job; the map records the guarantee as the job.
#[test]
fn a_module_whose_first_paragraph_is_a_field_states_no_job() {
    let text = "//! Guarantees: every value it returns is finite.
//!
//! Method: Shewchuk 1997 (<https://doi.org/10.1007/PL00009321>).
//!
//! Cost: constant time and no allocation.
";
    let problems = module_problems(text);
    assert!(
        problems.iter().any(|p| p.rule == Rule::Summary),
        "a module with no job statement passes; the map's job is the guarantee:\n{}",
        show(&problems)
    );
}

/// HEADERS.md, crate headers, Summary, "Exact syntax": "For kernel crates, `Spare Kit geometry
/// kernel: <topic>.`; for development crates, `Spare Kit geometry kernel, development only:
/// <topic>.`" The check only asks for a full stop.
#[test]
fn a_crate_summary_has_the_form_the_grammar_writes() {
    let kernel = KERNEL_LIB.replace("Spare Kit geometry kernel: an example.", "An example.");
    let dev = DEV_LIB.replace(
        "Spare Kit geometry kernel, development only: an example.",
        "Spare Kit geometry kernel: an example.",
    );
    each_fails(
        "crate summaries outside the written form",
        &[
            (
                "kernel crate summary \"An example.\"",
                build(&workspace(&kernel, &[("a.rs", MODULE)]), &[]).problems,
            ),
            (
                "development crate with the kernel crates' summary form",
                build(&workspace_with(KERNEL_LIB, &[("a.rs", MODULE)], &dev), &[]).problems,
            ),
        ],
    );
}

/// Principles and chapter 26.3: every module header names what it guarantees and what it costs.
/// A placeholder names neither, yet the check calls the header complete.
#[test]
fn placeholder_guarantees_and_costs_are_not_a_complete_header() {
    let guarantees = |g: &str| {
        module_problems(&MODULE.replace(
            "//! Guarantees:\n//!\n//! - Every value it returns is finite.\n",
            g,
        ))
    };
    let cost =
        |c: &str| module_problems(&MODULE.replace("Cost: constant time and no allocation.", c));
    each_fails(
        "placeholder fields",
        &[
            ("Guarantees: none.", guarantees("//! Guarantees: none.\n")),
            ("Guarantees: TODO.", guarantees("//! Guarantees: TODO.\n")),
            (
                "Guarantees: a list whose one item has no word (\"- .\")",
                guarantees("//! Guarantees:\n//!\n//! - .\n"),
            ),
            ("Cost: TBD.", cost("Cost: TBD.")),
            ("Cost: unknown.", cost("Cost: unknown.")),
            ("Cost: TODO.", cost("Cost: TODO.")),
        ],
    );
}

/// HEADERS.md: "Invariants: a paragraph ... followed by a `- ` list. Each item is one invariant."
/// An item with no word in it is no invariant.
#[test]
fn an_invariant_without_a_word_is_not_an_invariant() {
    let lib = KERNEL_LIB.replace("//! - It keeps one.\n", "//! - .\n");
    let problems = build(&workspace(&lib, &[("a.rs", MODULE)]), &[]).problems;
    assert!(
        problems.iter().any(|p| p.rule == Rule::Invariants),
        "a crate whose only invariant is \".\" passes:\n{}",
        show(&problems)
    );
}

/// HEADERS.md: "A field counts only when it is written in this grammar, so a header cannot pass by
/// mentioning a word in passing." A label that denies guarantees is read as the Guarantees field.
#[test]
fn a_label_that_denies_guarantees_is_not_the_guarantees_field() {
    let with = |g: &str| {
        module_problems(&MODULE.replace(
            "//! Guarantees:\n//!\n//! - Every value it returns is finite.\n",
            g,
        ))
    };
    each_fails(
        "labels that deny a guarantee",
        &[
            (
                "What this module never guarantees: anything.",
                with("//! What this module never guarantees: anything.\n"),
            ),
            (
                "Guarantees nothing: it is a stub.",
                with("//! Guarantees nothing: it is a stub.\n"),
            ),
        ],
    );
}

/// HEADERS.md: a method link may be "a DOI written `doi:10.…`"; the DOI Handbook's syntax is a
/// prefix `10.<registrant>`, a `/` and a suffix. `doi:10.` alone is read as a link.
#[test]
fn a_doi_needs_a_prefix_and_a_suffix() {
    let with = |m: &str| {
        module_problems(&MODULE.replace(
            "Method: Shewchuk 1997 (<https://doi.org/10.1007/PL00009321>).",
            m,
        ))
    };
    each_fails(
        "method lines whose only link is not a DOI",
        &[
            (
                "Method: the paper, doi:10.",
                with("Method: the paper, doi:10."),
            ),
            ("Method: doi:10.1000.", with("Method: doi:10.1000.")),
        ],
    );
}

/// The claim that the four crate fields are read "as the scaffold check in `engine-geometry`
/// (`tests/scaffold/header.rs`) reads them ... So a crate header passes one check exactly when it
/// passes the other on these four fields" (`map/header.rs`), tested against G09's reader itself.
#[test]
#[allow(clippy::too_many_lines, reason = "the cases are written out in full")]
fn the_four_crate_fields_read_as_g09_reads_them() {
    use g09_header::{Field, Header as G09};
    use geom_test::map::header::{CrateKind, Header as G15, crate_fields};
    use table::Layer;

    let base = [
        "Spare Kit geometry kernel: a thing.",
        "",
        "One job: things.",
        "",
        "Layer: 6, Arrangements.",
        "",
        "Invariants this crate keeps:",
        "",
        "- It keeps one.",
        "",
        "Sections: 11.3.",
    ];
    let cases: Vec<(&str, Vec<&str>)> = vec![
        ("the baseline", base.to_vec()),
        (
            "an Invariants line straight after a list item",
            vec![
                "Spare Kit geometry kernel: a thing.",
                "",
                "One job: things.",
                "",
                "Layer: 6, Arrangements.",
                "",
                "- A note in a list.",
                "Invariants this crate keeps:",
                "- It keeps one.",
                "",
                "Sections: 11.3.",
            ],
        ),
        (
            "an Invariants line straight after an SPDX line",
            vec![
                "Spare Kit geometry kernel: a thing.",
                "",
                "One job: things.",
                "",
                "Layer: 6, Arrangements.",
                "",
                "SPDX-License-Identifier: MPL-2.0",
                "Invariants this crate keeps:",
                "- It keeps one.",
                "",
                "Sections: 11.3.",
            ],
        ),
        (
            "an Invariants paragraph whose list follows a Consulted line",
            vec![
                "Spare Kit geometry kernel: a thing.",
                "",
                "One job: things.",
                "",
                "Layer: 6, Arrangements.",
                "",
                "Invariants this crate keeps:",
                "Consulted: X, MIT, <https://example.org/x.c>, for a test.",
                "- It keeps one.",
                "",
                "Sections: 11.3.",
            ],
        ),
        (
            "a Layer sentence with a word before the one with the number",
            vec![
                "Spare Kit geometry kernel: a thing.",
                "",
                "One job: things.",
                "",
                "Layer: the arrangements. Layer: 6, Arrangements.",
                "",
                "Invariants this crate keeps:",
                "",
                "- It keeps one.",
                "",
                "Sections: 11.3.",
            ],
        ),
    ];
    let mut disagreements = Vec::new();
    for (name, lines) in &cases {
        let source = as_header(lines.iter().copied());
        let g09 = G09::parse(&source);
        let g09_says = [
            g09.has(Field::Job),
            matches!(g09.layer(), Some(Layer::N(n)) if (1..=13).contains(&n)),
            g09.has(Field::Invariants),
            g09.has(Field::Sections),
        ];
        let (_, problems) = crate_fields(&G15::parse(&source), CrateKind::Kernel);
        let g15_says = [Rule::Job, Rule::Layer, Rule::Invariants, Rule::Sections]
            .map(|rule| !problems.iter().any(|(r, _)| *r == rule));
        if g09_says != g15_says {
            disagreements.push(format!(
                "{name}: G09 (job, layer, invariants, sections) = {g09_says:?}, G15 = {g15_says:?}"
            ));
        }
    }
    assert!(
        disagreements.is_empty(),
        "G15 and G09 disagree on the four crate fields:\n{}",
        disagreements.join("\n")
    );
}

// ---------------------------------------------------------------------------------------------
// Provenance: Source, Consulted and SPDX lines (chapter 29).
// ---------------------------------------------------------------------------------------------

fn port(licence: &str) -> String {
    format!(
        "Source: PORT from Example, {licence},\n  \
         https://github.com/example/example/blob/{SHA}/src/x.c\n  \
         Copyright (c) 2020 Example Authors\n  \
         Changed for Spare Kit: rewritten in Rust with checked arithmetic.\n\
         SPDX-License-Identifier: {licence}"
    )
}

/// Chapter 29.2: "These SPDX identifiers, and no others, may appear ... in a ported file"; GPL,
/// LGPL and AGPL code is never copied (principles; 29.2's table). A PORT from any of them passes.
#[test]
fn a_port_needs_a_licence_on_the_allow_list() {
    assert_eq!(
        tail_problems(&port("BSD-3-Clause")),
        [],
        "the control port must pass"
    );
    let cases: Vec<(&str, Vec<Problem>)> = [
        "GPL-3.0-only",
        "LGPL-2.1-or-later",
        "AGPL-3.0-only",
        "CC-BY-NC-4.0",
        "LicenseRef-Proprietary",
    ]
    .iter()
    .map(|l| (*l, tail_problems(&port(l))))
    .collect();
    each_fails("ported licences off chapter 29.2's allow-list", &cases);
}

/// HEADERS.md: "`Consulted: <name>, <licence>, <link>, <what it was consulted for>` ... The
/// licence an SPDX expression or `public domain`". A record that omits the licence passes when the
/// name has a comma, because the name's last part is taken as an SPDX identifier; so do words
/// that are no licence at all.
#[test]
fn a_consulted_record_names_a_real_licence() {
    assert_eq!(
        tail_problems("Consulted: Foo, MIT, <https://example.org/foo.c>, for its signs."),
        [],
        "the control record must pass"
    );
    let cgal = format!(
        "Consulted: CGAL, `FPU.h`, \
         <https://github.com/CGAL/cgal/blob/{SHA}/Number_types/include/CGAL/FPU.h>, \
         for its rounding."
    );
    each_fails(
        "Consulted records without a licence",
        &[
            (
                "Consulted: CGAL, `FPU.h`, <pinned link>, ... (no licence)",
                tail_problems(&cgal),
            ),
            (
                "Consulted: Foo, unknown, <link>, ...",
                tail_problems(
                    "Consulted: Foo, unknown, <https://example.org/foo.c>, for its signs.",
                ),
            ),
            (
                "Consulted: Foo, TBD, <link>, ...",
                tail_problems("Consulted: Foo, TBD, <https://example.org/foo.c>, for its signs."),
            ),
        ],
    );
}

/// Chapter 29.2's table: AGPL code and code under non-commercial terms is not read ("Read the
/// source? No"). A Consulted record is a statement that it was read; the check accepts it.
#[test]
fn code_that_may_never_be_read_cannot_be_consulted() {
    let consulted = |licence: &str| {
        tail_problems(&format!(
            "Consulted: TetGen, {licence},\n  \
             <https://github.com/example/tetgen/blob/{SHA}/tetgen.cxx>,\n  \
             for its refinement order."
        ))
    };
    each_fails(
        "Consulted records of code 29.2 forbids reading",
        &[
            ("AGPL-3.0-only", consulted("AGPL-3.0-only")),
            (
                "PolyForm-Noncommercial-1.0.0",
                consulted("PolyForm-Noncommercial-1.0.0"),
            ),
        ],
    );
}

/// Chapter 29.20, rule 4: "A PORT METHOD module is MPL-2.0 and ours." A module that ports nothing
/// but declares another licence passes.
#[test]
fn a_module_that_ports_nothing_is_mpl() {
    let problems = tail_problems(
        "Consulted: Foo, MIT, <https://example.org/foo.c>, for its signs.\n\
         SPDX-License-Identifier: GPL-3.0-only",
    );
    assert!(
        !problems.is_empty(),
        "a module with no Source line declares GPL-3.0-only and passes"
    );
}

/// HEADERS.md: a Source record carries "the origin's copyright lines, copied exactly (at least one
/// line containing `Copyright`, `(c)` or `©`)". The test is any continuation line, so a
/// `Changed for Spare Kit:` line or the link itself satisfies it.
#[test]
fn the_copyright_line_is_a_copyright_line() {
    let changed_line = format!(
        "Source: PORT from Example, BSD-3-Clause,\n  \
         https://github.com/example/example/blob/{SHA}/src/x.c\n  \
         Changed for Spare Kit: split case (c) of the origin into two functions.\n\
         SPDX-License-Identifier: BSD-3-Clause"
    );
    let link_path = format!(
        "Source: PORT from Example, BSD-3-Clause,\n  \
         https://github.com/example/copyright-free-geometry/blob/{SHA}/src/x.c\n  \
         Changed for Spare Kit: rewritten in Rust.\n\
         SPDX-License-Identifier: BSD-3-Clause"
    );
    each_fails(
        "Source records with no copyright line",
        &[
            (
                "only the Changed line has \"(c)\"",
                tail_problems(&changed_line),
            ),
            ("only the link has \"copyright\"", tail_problems(&link_path)),
        ],
    );
}

/// Chapter 29.20, rule 1: where the licences a source states disagree, "the strictest applies,
/// and you stop and ask"; rule 4: the generator reads the lines "into one record per source". Two
/// headers that credit one file under two licences are merged silently, keeping the first.
#[test]
fn one_source_under_two_licences_is_not_merged_silently() {
    let a = MODULE_WITH_TAIL.replace(
        "{TAIL}\n",
        "//! Consulted: Foo, MIT, <https://example.org/foo.c>, for its signs.\n",
    );
    let b = MODULE_WITH_TAIL.replace(
        "{TAIL}\n",
        "//! Consulted: Foo, GPL-3.0-only, <https://example.org/foo.c>, for its signs.\n",
    );
    let input = workspace(KERNEL_LIB, &[("a.rs", &a), ("b.rs", &b)]);
    let generated = build(&input, &[]);
    let map: serde_json::Value = serde_json::from_str(&generated.text).unwrap();
    let licences: Vec<String> = map["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["link"] == "https://example.org/foo.c")
        .map(|s| s["licence"].as_str().unwrap_or_default().to_owned())
        .collect();
    let both_kept =
        licences.iter().any(|l| l.contains("GPL")) && licences.iter().any(|l| l.contains("MIT"));
    assert!(
        !generated.problems.is_empty() || both_kept,
        "two licences for one source give no problem, and the ledger keeps only {licences:?}"
    );
}

/// Chapter 29.20, rule 4: "one record per source". One file consulted at two line ranges is one
/// source, but the map keys records by the whole link, fragment included.
#[test]
fn one_file_at_two_line_ranges_is_one_source() {
    let link = format!(
        "https://github.com/BrunoLevy/geogram/blob/{SHA}/src/lib/geogram/numerics/interval_nt.h"
    );
    let a = MODULE_WITH_TAIL.replace(
        "{TAIL}\n",
        &format!("//! Consulted: Geogram, BSD-3-Clause, <{link}#L455-L462>, for its rounding.\n"),
    );
    let b = MODULE_WITH_TAIL.replace(
        "{TAIL}\n",
        &format!("//! Consulted: Geogram, BSD-3-Clause, <{link}#L1-L40>, for its layout.\n"),
    );
    let map = the_map(&workspace(KERNEL_LIB, &[("a.rs", &a), ("b.rs", &b)]));
    let records: Vec<&str> = map["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["link"].as_str().unwrap())
        .collect();
    assert_eq!(
        records.len(),
        1,
        "one file at one commit is recorded as {} sources: {records:?}",
        records.len()
    );
}

/// HEADERS.md: "a link on a code-hosting site pinned to a full commit". Only five hosts and
/// `gitlab.*` count as code-hosting sites, so an unpinned link on any other forge, cgit, gitweb or
/// Gitiles host passes as if the site had no commits.
#[test]
fn a_consulted_link_on_any_code_host_is_pinned() {
    let consulted = |link: &str| {
        tail_problems(&format!(
            "Consulted: Origin, BSD-2-Clause, <{link}>, for the reduction."
        ))
    };
    let cases: Vec<(&str, Vec<Problem>)> = [
        "https://cgit.freebsd.org/src/tree/lib/msun/src/s_sinpi.c",
        "https://gitee.com/mirrors/example/blob/master/src/a.c",
        "https://salsa.debian.org/science-team/example/-/blob/master/src/a.c",
        "https://invent.kde.org/graphics/example/-/blob/master/src/a.cpp",
        "https://chromium.googlesource.com/external/example/+/refs/heads/main/src/a.cc",
    ]
    .iter()
    .map(|l| (*l, consulted(l)))
    .collect();
    each_fails("unpinned Consulted links on code hosts", &cases);
}

/// HEADERS.md: "Every link to outside code is credited." A link to source code that is not on one
/// of the five forges and does not end in a listed extension is not seen as code at all: glibc's
/// LGPL `s_sin.c` through sourceware's gitweb, a gist, a docs.rs source page.
#[test]
fn a_link_to_code_on_any_host_needs_credit() {
    let method = |link: &str| {
        module_problems(&MODULE.replace(
            "Method: Shewchuk 1997 (<https://doi.org/10.1007/PL00009321>).",
            &format!(
                "Method: Shewchuk 1997 (<https://doi.org/10.1007/PL00009321>), as in <{link}>."
            ),
        ))
    };
    let cases: Vec<(&str, Vec<Problem>)> = [
        "https://sourceware.org/git/?p=glibc.git;a=blob;f=sysdeps/ieee754/dbl-64/s_sin.c;hb=HEAD",
        "https://gist.github.com/someone/0123456789abcdef0123",
        "https://docs.rs/libm/0.2.16/src/libm/math/sin.rs.html",
    ]
    .iter()
    .map(|l| (*l, method(l)))
    .collect();
    each_fails("links to outside code with no credit line", &cases);
}

// ---------------------------------------------------------------------------------------------
// Gaps, error kinds and module roles.
// ---------------------------------------------------------------------------------------------

/// HEADERS.md, known gaps: a gap is "a problem that cannot be fixed in the piece that finds it".
/// The gap list is keyed by file and rule only, so a gap for a missing Layer line also hides a
/// wrong one, and the map then records the wrong layer with the check green.
#[test]
fn a_gap_hides_only_the_problem_it_names() {
    // (Fixer, G15: a gap now names the one problem it accepts, by its exact message.)
    const MISSING_LAYER: Gap = Gap {
        file: "projects/engines/geometry/crates/geom-x/src/lib.rs",
        rule: Rule::Layer,
        problem: geom_test::map::header::NO_LAYER,
        reason: "the header predates the layer table and has no Layer line",
    };
    let lib = KERNEL_LIB.replace("Layer: 6, Arrangements.", "Layer: 99, beyond the table.");
    let generated = build(&workspace(&lib, &[("a.rs", MODULE)]), &[MISSING_LAYER]);
    let map: serde_json::Value = serde_json::from_str(&generated.text).unwrap();
    assert!(
        !generated.problems.is_empty(),
        "a gap for a missing Layer line hides \"Layer: 99\"; the map records layer {}",
        map["crates"][1]["layer"]
    );
}

/// Chapter 26.3: the map lists "every ... error kind", and HEADERS.md says the name rule makes the
/// list complete. Chapter 22 does not name the per-family detail enums `OpErrorDetail` holds, and
/// an error enum named otherwise, which implements `core::error::Error` and is the error type of
/// a public function, is neither listed nor reported.
#[test]
fn an_error_enum_is_found_by_what_it_is_not_only_by_its_name() {
    let text = "//! Sweeps.
//!
//! Guarantees: a sweep either succeeds or says why not.
//!
//! Method: chapter 13 (`projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`).
//!
//! Cost: constant.

use core::fmt;

/// Why a sweep failed.
#[derive(Debug)]
pub enum SweepFailure {
    MitreMismatch,
}

impl fmt::Display for SweepFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(\"the mitres do not meet\")
    }
}

impl core::error::Error for SweepFailure {}

/// Sweeps a profile.
///
/// # Errors
///
/// [`SweepFailure::MitreMismatch`] when the mitres do not meet.
pub fn sweep() -> Result<(), SweepFailure> {
    Ok(())
}
";
    let input = workspace(KERNEL_LIB, &[("sweep.rs", text)]);
    let generated = build(&input, &[]);
    let map: serde_json::Value = serde_json::from_str(&generated.text).unwrap();
    let listed = map["crates"][1]["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == "SweepFailure");
    let reported = generated
        .problems
        .iter()
        .any(|p| p.message.contains("SweepFailure"));
    assert!(
        listed || reported,
        "an undocumented error kind of a kernel crate is neither in the map nor reported"
    );
}

/// HEADERS.md: modules are `src/a.rs`, `src/a/b.rs` ...; binaries are `src/bin/*.rs`. Cargo's
/// layout makes `src/bin/tool/main.rs` the binary `tool` and `src/bin/tool/helper.rs` one of its
/// modules, but the map lists the module as a binary named `tool/helper`.
#[test]
fn a_module_of_a_binary_is_not_a_binary() {
    let binary = "//! `tool`: an example binary.
//!
//! Guarantees: exits 0 or 1.
//!
//! Method: none; it calls the library.
//!
//! Cost: one call.

mod helper;

fn main() {}
";
    let map = the_map(&workspace(
        KERNEL_LIB,
        &[("bin/tool/main.rs", binary), ("bin/tool/helper.rs", MODULE)],
    ));
    let wrong: Vec<String> = map["crates"][1]["modules"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["target"] == "bin" && m["path"].as_str().unwrap().contains('/'))
        .map(|m| format!("{} as {}", m["file"], m["path"]))
        .collect();
    assert!(
        wrong.is_empty(),
        "modules of a binary listed as binaries: {wrong:?}"
    );
}

/// HEADERS.md: "The same tree gives the same bytes on every target", and the map check runs on
/// Windows runners, whose checkouts may turn line ends into `\r\n`. A guard.
#[test]
fn sources_with_windows_line_ends_give_the_same_map() {
    let lf = build(&workspace(KERNEL_LIB, &[("a.rs", MODULE)]), &[]);
    let crlf = build(
        &workspace(
            &KERNEL_LIB.replace('\n', "\r\n"),
            &[("a.rs", &MODULE.replace('\n', "\r\n"))],
        ),
        &[],
    );
    assert_eq!(crlf.problems, lf.problems);
    assert!(crlf.text == lf.text, "the map differs with \\r\\n sources");
}

// ---------------------------------------------------------------------------------------------
// On disk, and the real tree.
// ---------------------------------------------------------------------------------------------

#[cfg(not(target_family = "wasm"))]
mod on_disk {
    use std::fs;
    use std::path::{Path, PathBuf};

    use geom_test::map::{MAP_FILE, Problem, build, repo};

    use super::{KERNEL_LIB, MODULE, show};

    /// A path under `root` from a `/`-separated relative path, with the system's separators.
    fn under(root: &Path, rel: &str) -> PathBuf {
        rel.split('/')
            .fold(root.to_path_buf(), |p, part| p.join(part))
    }

    /// A scratch workspace with one kernel crate `geom-x` (root and module `a.rs`).
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str, module: &str) -> Scratch {
            let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("attack-g15-{name}"));
            let _ = fs::remove_dir_all(&root);
            let krate = under(&root, "projects/engines/geometry/crates/geom-x");
            fs::create_dir_all(krate.join("src")).unwrap();
            fs::write(krate.join("Cargo.toml"), "[package]\nname = \"geom-x\"\n").unwrap();
            fs::write(krate.join("src").join("lib.rs"), KERNEL_LIB).unwrap();
            fs::write(krate.join("src").join("a.rs"), module).unwrap();
            let arch = under(&root, "projects/engines/geometry/architecture");
            fs::create_dir_all(&arch).unwrap();
            fs::write(arch.join("06-numbers.md"), "# 6. Numbers\n").unwrap();
            Scratch(root)
        }

        fn problems(&self) -> Vec<Problem> {
            build(&repo::read(&self.0).unwrap(), &[]).problems
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A folder link (a junction on Windows, which needs no privilege; a symbolic link elsewhere).
    fn link_folder(link: &Path, target: &Path) -> bool {
        #[cfg(windows)]
        {
            std::process::Command::new("cmd")
                .arg("/C")
                .arg("mklink")
                .arg("/J")
                .arg(link)
                .arg(target)
                .output()
                .is_ok_and(|o| o.status.success())
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = (link, target);
            false
        }
    }

    /// HEADERS.md: a method link is "a path in this repository in backticks". A path that is not
    /// in the repository is accepted as the method's link.
    #[test]
    fn a_method_link_into_the_repository_must_exist() {
        let good = MODULE.replace(
            "Method: Shewchuk 1997 (<https://doi.org/10.1007/PL00009321>).",
            "Method: chapter 6 (`projects/engines/geometry/architecture/06-numbers.md`).",
        );
        let s = Scratch::new("repo-link-good", &good);
        assert_eq!(s.problems(), [], "the control link exists and must pass");
        drop(s);

        let dead = MODULE.replace(
            "Method: Shewchuk 1997 (<https://doi.org/10.1007/PL00009321>).",
            "Method: chapter 99 (`projects/engines/geometry/architecture/99-no-such-chapter.md`).",
        );
        let s = Scratch::new("repo-link-dead", &dead);
        let problems = s.problems();
        assert!(
            problems.iter().any(|p| p.file.ends_with("/a.rs")),
            "a method whose only link is a repository path that does not exist passes:\n{}",
            show(&problems)
        );
    }

    /// `map/repo.rs`: "A file that is not UTF-8 and a symbolic link are reported, not skipped."
    /// A linked folder under `src/`, or a linked crate folder, is skipped without a word, so the
    /// modules in it are never checked.
    #[test]
    fn a_linked_folder_is_reported_not_skipped() {
        let s = Scratch::new("links", MODULE);
        let elsewhere = s.0.join("elsewhere");
        let module_target = elsewhere.join("modules");
        fs::create_dir_all(&module_target).unwrap();
        fs::write(module_target.join("bad.rs"), "pub fn no_header() {}\n").unwrap();
        let crate_target = elsewhere.join("geom-y");
        fs::create_dir_all(crate_target.join("src")).unwrap();
        fs::write(
            crate_target.join("Cargo.toml"),
            "[package]\nname = \"geom-y\"\n",
        )
        .unwrap();
        fs::write(
            crate_target.join("src").join("lib.rs"),
            "pub fn no_header() {}\n",
        )
        .unwrap();

        let src = under(&s.0, "projects/engines/geometry/crates/geom-x/src");
        assert!(
            link_folder(&src.join("linked"), &module_target),
            "could not create a folder link under src/"
        );
        assert!(
            link_folder(
                &under(&s.0, "projects/engines/geometry/crates/geom-y"),
                &crate_target
            ),
            "could not create a linked crate folder"
        );
        let problems = s.problems();
        let mut missing = Vec::new();
        if !problems
            .iter()
            .any(|p| p.file.contains("geom-x/src/linked"))
        {
            missing.push("the linked folder geom-x/src/linked");
        }
        if !problems.iter().any(|p| p.file.contains("geom-y")) {
            missing.push("the linked crate folder geom-y");
        }
        assert!(
            missing.is_empty(),
            "skipped without a problem: {missing:?}; problems:\n{}",
            show(&problems)
        );
    }

    fn real_map() -> serde_json::Value {
        let root = repo::workspace_root().unwrap();
        let text = fs::read_to_string(root.join(MAP_FILE)).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    /// Chapter 26.3: "A machine-readable map of every crate"; chapter 27's layer table puts the
    /// facade `engine-geometry` on layer 13 of the kernel. The map leaves it out.
    #[test]
    fn the_map_lists_the_facade() {
        let map = real_map();
        let names: Vec<&str> = map["crates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect();
        assert!(
            names.contains(&"engine-geometry"),
            "MAP.json has {} crates and not the layer-13 facade engine-geometry",
            names.len()
        );
    }

    /// Chapter 29.20, rule 4: a module "credits every source it consulted with a credit line", and
    /// the map reads those lines "into one record per source". geom-fuzz's crate header says it
    /// read bolero's and cargo-bolero's source files ("`bolero/src/test/mod.rs`", "at commit
    /// 7642943, read 2026-10-21"), with no Consulted line and an abbreviated commit, so the ledger
    /// misses them and the check passes.
    #[test]
    fn a_source_a_header_says_it_read_is_in_the_ledger() {
        let map = real_map();
        let names: Vec<String> = map["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap().to_lowercase())
            .collect();
        assert!(
            names.iter().any(|n| n.contains("bolero")),
            "geom-fuzz's header names bolero source files it read, and the ledger has: {names:?}"
        );
    }
}
