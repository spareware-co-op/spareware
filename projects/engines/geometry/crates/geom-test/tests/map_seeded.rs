//! Seeded fixtures for every rule of the header grammar (`projects/engines/geometry/HEADERS.md`):
//! a good workspace that passes every rule, and for each rule bad workspaces that break it and
//! nothing else, so a rule that stopped firing, or fired on a good header, fails here.
//!
//! The fixtures are built in memory ([`geom_test::map::Input`]), so they run on every target,
//! wasm32-wasip1 included. `map_files.rs` runs the same cycle on disk and through the binary.

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

use std::collections::BTreeSet;

use geom_test::map::{CrateInput, Gap, Input, Problem, Rule, SourceFile, build, compare};

const KERNEL: &str = "projects/engines/geometry/crates/geom-x";
const DEV: &str = "projects/engines/geometry/crates/geom-fuzz";
const FACADE: &str = "projects/engines/crates/engine-geometry";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

const KERNEL_MANIFEST: &str = "[package]\nname = \"geom-x\"\nversion = \"0.0.0\"\n\
                               license = \"MPL-2.0 AND BSD-3-Clause\"\n\n[dependencies]\n\
                               geom-num = { path = \"../geom-num\" }\nlibm = \"0.2.16\"\n";

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
//!
//! Family: Combine and 2D.
//!
//! Status: scaffold only; built by pieces G1.
//!
//! Licences of the published crates this crate depends on:
//!
//! - MIT: `libm`.
#![forbid(unsafe_code)]

pub mod a;
pub mod error;
pub mod noted;
pub mod ported;
";

const MODULE_A: &str = "//! An example module: every module field, written once.
//!
//! What it does: nothing much.
//!
//! Guarantees:
//!
//! - One guarantee,
//!   over two lines.
//! - Another.
//!
//! Method: Shewchuk 1997 (<https://doi.org/10.1007/PL00009321>), as chapter 6 says
//! (`projects/engines/geometry/architecture/06-numbers.md`); his reference code is
//! <https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c>.
//!
//! Cost: constant.
//!
//! Family: Query.
//!
//! Consulted: J. R. Shewchuk, `predicates.c`, public domain,
//!   <https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c>, for its signs.
//! SPDX-License-Identifier: MPL-2.0

/// A function.
pub fn f() {}
";

const MODULE_ERROR: &str = "//! Errors of the example.
//!
//! Guarantees: every kind is documented.
//!
//! Method: none; a closed enum of failures.
//!
//! Cost: constant.

use core::fmt;

/// Why it failed.
///
/// More detail.
#[derive(Debug)]
#[non_exhaustive]
pub enum XError {
    /// It was degenerate.
    Degenerate,
    /// A value was out of range.
    OutOfRange {
        /// The value.
        value: i64,
    },
}

impl fmt::Display for XError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(\"failed\")
    }
}

/// Why a sweep failed: an error enum by its use, not its name.
pub enum SweepFailure {
    /// The mitres do not meet.
    MitreMismatch,
}

impl core::error::Error for SweepFailure {}

/// Not an error.
pub enum Sign {
    Negative,
}

#[cfg(test)]
mod tests {
    pub enum TestError {
        A,
    }
}
";

const MODULE_NOTED: &str = "//! A module that read LGPL code under the method-only procedure.
//!
//! Guarantees: as the note says.
//!
//! Method: the note's paper, doi:10.1000/182.
//!
//! Cost: linear in its input.
//!
//! Consulted: Example `widen.h`, `LGPL-3.0-or-later OR LicenseRef-Commercial`,
//!   <https://github.com/example/lgpl/blob/0123456789abcdef0123456789abcdef01234567/widen.h#L1-L9>,
//!   for its widening.
//!   Method note: `projects/engines/geometry/methods/example-widening.md`.
";

const MODULE_PORTED: &str = "//! A ported module.
//!
//! Guarantees: as its origin's.
//!
//! Methods followed: the origin's paper (doi:10.1000/example).
//!
//! Cost: linear in its input.
//!
//! Source: PORT from Example, BSD-3-Clause,
//!   https://github.com/example/example/blob/0123456789abcdef0123456789abcdef01234567/src/x.c
//!   Copyright (c) 2020 Example Authors
//!   Changed for Spare Kit: rewritten in Rust with checked arithmetic.
//! SPDX-License-Identifier: MPL-2.0 AND BSD-3-Clause
";

const BINARY: &str = "//! `tool`: an example binary.
//!
//! Guarantees: exits 0 or 1.
//!
//! Method: none; it calls the library.
//!
//! Cost: one call.

mod helper;

fn main() {}
";

const BINARY_HELPER: &str = "//! A module of the binary `tool`.
//!
//! Guarantees: it parses arguments.
//!
//! Method: none; argument parsing.
//!
//! Cost: linear in the arguments.

/// Parses.
pub fn parse() {}
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
//!
//! Method: none; the root defines one type.
//!
//! Cost: none at run time.

/// A development crate's error enums are not kernel error kinds.
pub enum DevError {
    Undocumented,
}
";

const FACADE_LIB: &str = "//! Shared engine: an example facade.
//!
//! One job: The stable facade.
//!
//! Layer: 13, Facade.
//!
//! Invariants this crate keeps:
//!
//! - No kernel logic.
//!
//! Sections: 25 and 27.
";

fn file(folder: &str, rel: &str, text: &str) -> SourceFile {
    SourceFile {
        path: format!("{folder}/src/{rel}"),
        text: Ok(text.to_owned()),
    }
}

/// The good workspace: a kernel crate with every field and form, a development crate whose root
/// holds code, and the facade.
fn good() -> Input {
    Input {
        crates: vec![
            CrateInput {
                name: "geom-x".into(),
                folder: KERNEL.into(),
                manifest: Some(KERNEL_MANIFEST.into()),
                files: vec![
                    file(KERNEL, "lib.rs", KERNEL_LIB),
                    file(KERNEL, "a.rs", MODULE_A),
                    file(KERNEL, "error.rs", MODULE_ERROR),
                    file(KERNEL, "noted.rs", MODULE_NOTED),
                    file(KERNEL, "ported.rs", MODULE_PORTED),
                    file(KERNEL, "bin/tool/main.rs", BINARY),
                    file(KERNEL, "bin/tool/helper.rs", BINARY_HELPER),
                ],
            },
            CrateInput {
                name: "geom-fuzz".into(),
                folder: DEV.into(),
                manifest: Some(
                    "[package]\nname = \"geom-fuzz\"\n\n[lib]\nname = \"geom_fuzz\"\n\n\
                     [dev-dependencies]\nbolero = \"0.13.7\"\n"
                        .into(),
                ),
                files: vec![file(DEV, "lib.rs", DEV_LIB)],
            },
            CrateInput {
                name: "engine-geometry".into(),
                folder: FACADE.into(),
                manifest: Some("[package]\nname = \"engine-geometry\"\n".into()),
                files: vec![file(FACADE, "lib.rs", FACADE_LIB)],
            },
        ],
        ..Input::default()
    }
}

/// The good workspace with one file's text replaced by `edit` applied to it.
fn with(rel: &str, edit: impl Fn(&str) -> String) -> Input {
    let mut input = good();
    let path = if let Some(dev) = rel.strip_prefix("dev:") {
        format!("{DEV}/src/{dev}")
    } else if let Some(facade) = rel.strip_prefix("facade:") {
        format!("{FACADE}/src/{facade}")
    } else {
        format!("{KERNEL}/src/{rel}")
    };
    let mut found = false;
    for krate in &mut input.crates {
        for f in &mut krate.files {
            if f.path == path {
                let text = f.text.clone().unwrap();
                f.text = Ok(edit(&text));
                found = true;
            }
        }
    }
    assert!(found, "no fixture file {path}");
    input
}

/// The good workspace with the kernel crate's manifest replaced by `edit` applied to it.
fn with_manifest(edit: impl Fn(&str) -> String) -> Input {
    let mut input = good();
    input.crates[0].manifest = Some(edit(KERNEL_MANIFEST));
    input
}

/// `text` with `old` replaced by `new`, which must occur.
fn replace(old: &'static str, new: &'static str) -> impl Fn(&str) -> String {
    move |text: &str| {
        assert!(text.contains(old), "fixture has no {old:?}");
        text.replacen(old, new, 1)
    }
}

fn problems(input: &Input) -> Vec<Problem> {
    build(input, &[]).problems
}

/// The bad input breaks `rule` in `file` (relative to the kernel crate's `src`, or the path given)
/// and breaks nothing else; the message names the file and the field.
fn breaks(input: &Input, rule: Rule, file: &str) {
    let found = problems(input);
    let path = if file.starts_with("projects/") {
        file.to_owned()
    } else {
        format!("{KERNEL}/src/{file}")
    };
    assert!(
        !found.is_empty() && found.iter().all(|p| p.rule == rule && p.file == path),
        "expected only {rule:?} in {path}, found:\n{}",
        found
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
    for p in &found {
        let line = p.to_string();
        assert!(
            line.starts_with(&format!("{path}: {}: ", rule.label())),
            "{line}"
        );
    }
}

/// Module `a.rs` with `body` (records, each line with its `//! `) in place of its SPDX line.
fn a_with_tail(body: &'static str) -> Input {
    with("a.rs", move |t| {
        format!("{}{body}", t.split("//! SPDX").next().unwrap())
    })
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "every field of the good map is checked in one place"
)]
fn the_good_workspace_passes_every_rule() {
    let generated = build(&good(), &[]);
    assert_eq!(generated.problems, [], "{:#?}", generated.problems);
    let map: serde_json::Value = serde_json::from_str(&generated.text).unwrap();
    let crates = map["crates"].as_array().unwrap();
    let names: Vec<&str> = crates.iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["engine-geometry", "geom-fuzz", "geom-x"]);
    assert_eq!(crates[0]["kind"], "facade");
    assert_eq!(crates[0]["layer"], "13");
    let x = &crates[2];
    assert_eq!(x["kind"], "kernel");
    assert_eq!(x["layer"], "6");
    assert_eq!(x["job"], "examples of every field.");
    assert_eq!(x["chapters"], serde_json::json!([11, 22]));
    assert_eq!(x["family"], serde_json::json!(["Combine", "2D"]));
    assert_eq!(x["invariants"], serde_json::json!(["It keeps one."]));
    assert_eq!(x["status"], "scaffold only; built by pieces G1.");
    assert_eq!(x["depends"][0]["name"], "libm");
    assert_eq!(x["depends"][0]["licence"], "MIT");
    assert_eq!(x["depends"][0]["take"], "depend");
    let fuzz = &crates[1];
    assert_eq!(fuzz["job"], "fuzz harnesses.");
    assert_eq!(fuzz["layer"], "Development");
    // A development crate's root that holds code is a module too.
    assert_eq!(fuzz["modules"][0]["path"], "geom_fuzz");
    assert_eq!(
        fuzz["modules"][0]["guarantees"],
        serde_json::json!(["Development only."])
    );
    assert_eq!(fuzz["depends"], serde_json::json!([]));

    let modules = x["modules"].as_array().unwrap();
    let paths: Vec<(&str, &str)> = modules
        .iter()
        .map(|m| (m["path"].as_str().unwrap(), m["target"].as_str().unwrap()))
        .collect();
    assert_eq!(
        paths,
        [
            ("geom_x::a", "lib"),
            ("tool::helper", "bin"),
            ("tool", "bin"),
            ("geom_x::error", "lib"),
            ("geom_x::noted", "lib"),
            ("geom_x::ported", "lib"),
        ]
    );
    let a = &modules[0];
    assert_eq!(
        a["job"],
        "An example module: every module field, written once."
    );
    assert_eq!(a["does"], "nothing much.");
    assert_eq!(
        a["guarantees"],
        serde_json::json!(["One guarantee, over two lines.", "Another."])
    );
    assert_eq!(
        a["method"]["links"],
        serde_json::json!([
            "https://doi.org/10.1007/PL00009321",
            "projects/engines/geometry/architecture/06-numbers.md",
            "https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c"
        ])
    );
    assert_eq!(a["cost"], "constant.");
    assert_eq!(a["family"], serde_json::json!(["Query"]));
    assert_eq!(a["spdx"], "MPL-2.0");
    assert_eq!(a["provenance"][0]["take"], "port-method");
    assert_eq!(a["provenance"][0]["licence"], "public domain");
    assert_eq!(a["provenance"][0]["purpose"], "for its signs.");
    assert_eq!(modules[2]["target"], "bin");
    assert_eq!(modules[3]["method"]["none"], "a closed enum of failures.");
    assert_eq!(
        modules[4]["provenance"][0]["method_note"],
        "projects/engines/geometry/methods/example-widening.md"
    );
    assert_eq!(modules[5]["provenance"][0]["take"], "port");

    // Kernel error kinds are listed, by name and by use; a development crate's and a test
    // module's are not.
    let errors = x["errors"].as_array().unwrap();
    let error_names: Vec<&str> = errors.iter().map(|e| e["name"].as_str().unwrap()).collect();
    assert_eq!(error_names, ["XError", "SweepFailure"]);
    assert_eq!(errors[0]["module"], "geom_x::error");
    assert_eq!(errors[0]["doc"], "Why it failed.");
    assert_eq!(errors[0]["non_exhaustive"], true);
    assert_eq!(errors[0]["kinds"][1]["name"], "OutOfRange");
    assert_eq!(errors[0]["kinds"][1]["shape"], "struct");
    assert_eq!(errors[0]["kinds"][1]["doc"], "A value was out of range.");
    assert_eq!(fuzz["errors"], serde_json::json!([]));

    // One record per source, with every file that uses it; the link without its fragment.
    let sources = map["sources"].as_array().unwrap();
    let takes: Vec<(&str, &str)> = sources
        .iter()
        .map(|s| (s["take"].as_str().unwrap(), s["link"].as_str().unwrap()))
        .collect();
    assert_eq!(
        takes,
        [
            ("depend", "https://crates.io/crates/libm/0.2.16"),
            (
                "port",
                &*format!("https://github.com/example/example/blob/{SHA}/src/x.c")
            ),
            (
                "port-method",
                &*format!("https://github.com/example/lgpl/blob/{SHA}/widen.h")
            ),
            (
                "port-method",
                "https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c"
            ),
        ]
    );
    assert_eq!(map["format"], 1);
    assert_eq!(map["gaps"], serde_json::json!([]));
    assert_eq!(map["pending"], serde_json::json!([]));
}

#[test]
fn the_map_is_the_same_whatever_the_input_order() {
    let a = build(&good(), &[]).text;
    let mut shuffled = good();
    shuffled.crates.reverse();
    for k in &mut shuffled.crates {
        k.files.reverse();
    }
    assert_eq!(a, build(&shuffled, &[]).text);
    assert!(!a.contains('\r'));
    assert!(a.ends_with("}\n"));
}

#[test]
fn manifest() {
    let mut input = good();
    input.crates[0].manifest =
        Some(KERNEL_MANIFEST.replace("name = \"geom-x\"", "name = \"geom-y\""));
    breaks(&input, Rule::Manifest, &format!("{KERNEL}/Cargo.toml"));
    breaks(
        &with_manifest(|m| format!("{m}\n[lib]\npath = \"src/x.rs\"\n")),
        Rule::Manifest,
        &format!("{KERNEL}/Cargo.toml"),
    );
    let mut input = good();
    input.crates[0].manifest = None;
    let found = problems(&input);
    assert!(found.iter().any(|p| p.rule == Rule::Manifest));
}

#[test]
fn root() {
    let mut input = good();
    input.crates[0]
        .files
        .retain(|f| !f.path.ends_with("/lib.rs"));
    breaks(&input, Rule::Root, KERNEL);
}

#[test]
fn unreadable() {
    let mut input = good();
    input.crates[0].files[1].text = Err("the file is not UTF-8".into());
    breaks(&input, Rule::Unreadable, "a.rs");
    breaks(
        &with("error.rs", |t| format!("{t}\nconst S: &str = \"open;\n")),
        Rule::Unreadable,
        "error.rs",
    );
    // A crate folder that is a symbolic link, which `repo::read` reports.
    let mut input = good();
    input.unreadable.push(SourceFile {
        path: "projects/engines/geometry/crates/geom-y".into(),
        text: Err("a crate folder that is a symbolic link".into()),
    });
    breaks(
        &input,
        Rule::Unreadable,
        "projects/engines/geometry/crates/geom-y",
    );
}

#[test]
fn header() {
    breaks(
        &with("a.rs", |t| format!("// A plain comment first.\n{t}")),
        Rule::Header,
        "a.rs",
    );
    breaks(
        &with("lib.rs", |t| format!("\n{t}")),
        Rule::Header,
        "lib.rs",
    );
}

#[test]
fn summary() {
    breaks(
        &with("a.rs", replace("written once.\n", "written once\n")),
        Rule::Summary,
        "a.rs",
    );
    breaks(
        &with(
            "lib.rs",
            replace("kernel: an example.", "kernel: an example"),
        ),
        Rule::Summary,
        "lib.rs",
    );
    // The form the grammar writes for each kind of crate.
    breaks(
        &with(
            "lib.rs",
            replace("Spare Kit geometry kernel: an example.", "An example."),
        ),
        Rule::Summary,
        "lib.rs",
    );
    breaks(
        &with(
            "dev:lib.rs",
            replace(
                "geometry kernel, development only: an example.",
                "geometry kernel: an example.",
            ),
        ),
        Rule::Summary,
        &format!("{DEV}/src/lib.rs"),
    );
    breaks(
        &with(
            "facade:lib.rs",
            replace(
                "Shared engine: an example facade.",
                "Spare Kit geometry kernel: a facade.",
            ),
        ),
        Rule::Summary,
        &format!("{FACADE}/src/lib.rs"),
    );
    breaks(
        &with("lib.rs", replace("kernel: an example.", "kernel: TODO.")),
        Rule::Summary,
        "lib.rs",
    );
    // A module whose first paragraph is a field states no job.
    breaks(
        &with("error.rs", replace("//! Errors of the example.\n//!\n", "")),
        Rule::Summary,
        "error.rs",
    );
}

#[test]
fn job() {
    breaks(
        &with(
            "lib.rs",
            replace("One job: examples of every field.", "Its job is examples."),
        ),
        Rule::Job,
        "lib.rs",
    );
    breaks(
        &with(
            "lib.rs",
            replace("One job: examples of every field.", "One job:"),
        ),
        Rule::Job,
        "lib.rs",
    );
    breaks(
        &with(
            "lib.rs",
            replace("One job: examples of every field.", "One job: TBD."),
        ),
        Rule::Job,
        "lib.rs",
    );
}

#[test]
fn layer() {
    for bad in [
        "Layer: 14, beyond the table.",
        "Layer: the facade.",
        "Layer: Development.",
        "Layer: 0.",
    ] {
        breaks(
            &with("lib.rs", move |t| t.replace("Layer: 6, Arrangements.", bad)),
            Rule::Layer,
            "lib.rs",
        );
    }
    breaks(
        &with(
            "dev:lib.rs",
            replace("Layer: Development.", "Layer: 3, Topology."),
        ),
        Rule::Layer,
        &format!("{DEV}/src/lib.rs"),
    );
    breaks(
        &with("facade:lib.rs", replace("Layer: 13, Facade.", "Layer: 12.")),
        Rule::Layer,
        &format!("{FACADE}/src/lib.rs"),
    );
    // As G09 reads it: the first Layer sentence whose word is a number or Development.
    let generated = build(
        &with(
            "lib.rs",
            replace(
                "Layer: 6, Arrangements.",
                "Layer: the arrangements. Layer: 6.",
            ),
        ),
        &[],
    );
    assert_eq!(generated.problems, []);
    let map: serde_json::Value = serde_json::from_str(&generated.text).unwrap();
    assert_eq!(map["crates"][2]["layer"], "6");
}

#[test]
fn invariants() {
    breaks(
        &with(
            "lib.rs",
            replace("//! - It keeps one.\n", "//! It keeps one.\n"),
        ),
        Rule::Invariants,
        "lib.rs",
    );
    breaks(
        &with("lib.rs", replace("//! - It keeps one.\n", "//! - .\n")),
        Rule::Invariants,
        "lib.rs",
    );
    // As G09 reads it: the Invariants line opens a paragraph.
    breaks(
        &with(
            "lib.rs",
            replace(
                "//! Layer: 6, Arrangements.\n//!\n//! Invariants",
                "//! Layer: 6, Arrangements.\n//! Invariants",
            ),
        ),
        Rule::Invariants,
        "lib.rs",
    );
}

#[test]
fn sections() {
    breaks(
        &with(
            "lib.rs",
            replace("Sections: 11.3 and 22.", "Sections: G08 and G09 decide."),
        ),
        Rule::Sections,
        "lib.rs",
    );
    breaks(
        &with("lib.rs", replace("Sections: 11.3 and 22.", "Sections: 99.")),
        Rule::Sections,
        "lib.rs",
    );
}

#[test]
fn guarantees() {
    breaks(
        &with("a.rs", replace("//! Guarantees:\n", "//! Promises:\n")),
        Rule::Guarantees,
        "a.rs",
    );
    breaks(
        &with(
            "error.rs",
            replace("Guarantees: every kind is documented.", "Guarantees:"),
        ),
        Rule::Guarantees,
        "error.rs",
    );
    breaks(
        &with(
            "a.rs",
            replace(
                "//! Cost: constant.\n",
                "//! Cost: constant.\n//!\n//! Guarantees: again.\n",
            ),
        ),
        Rule::Guarantees,
        "a.rs",
    );
    for placeholder in ["Guarantees: none.", "Guarantees: TODO."] {
        breaks(
            &with("error.rs", move |t| {
                t.replace("Guarantees: every kind is documented.", placeholder)
            }),
            Rule::Guarantees,
            "error.rs",
        );
    }
    breaks(
        &with("a.rs", replace("//! - Another.\n", "//! - .\n")),
        Rule::Guarantees,
        "a.rs",
    );
    // A label that denies the field is not the field.
    for denial in [
        "What this module never guarantees: anything.",
        "Guarantees nothing: it is a stub.",
    ] {
        breaks(
            &with("error.rs", move |t| {
                t.replace("Guarantees: every kind is documented.", denial)
            }),
            Rule::Guarantees,
            "error.rs",
        );
    }
}

#[test]
fn method() {
    breaks(
        &with(
            "a.rs",
            replace("//! Method: Shewchuk", "//! Approach: Shewchuk"),
        ),
        Rule::Method,
        "a.rs",
    );
    breaks(
        &with(
            "error.rs",
            replace(
                "Method: none; a closed enum of failures.",
                "Method: the obvious one.",
            ),
        ),
        Rule::Method,
        "error.rs",
    );
    breaks(
        &with(
            "error.rs",
            replace("Method: none; a closed enum of failures.", "Method: none;"),
        ),
        Rule::Method,
        "error.rs",
    );
    // A DOI needs a prefix, a slash and a suffix.
    breaks(
        &with(
            "ported.rs",
            replace("(doi:10.1000/example)", "(doi:10.1000)"),
        ),
        Rule::Method,
        "ported.rs",
    );
    // `none` is for types, constants and trait implementations, and binaries' argument parsing.
    breaks(
        &with("error.rs", |t| {
            format!("{t}\n/// An algorithm.\npub fn widen() {{}}\n")
        }),
        Rule::Method,
        "error.rs",
    );
    // A link into the repository must exist when the input was read from one.
    let mut input = good();
    input.repo_paths = Some(BTreeSet::from([
        "projects/engines/geometry/architecture/06-numbers.md".to_owned(),
        "projects/engines/geometry/methods/example-widening.md".to_owned(),
    ]));
    assert_eq!(problems(&input), []);
    input.repo_paths = Some(BTreeSet::from([
        "projects/engines/geometry/methods/example-widening.md".to_owned(),
    ]));
    breaks(&input, Rule::Method, "a.rs");
}

#[test]
fn cost() {
    breaks(
        &with("a.rs", replace("//! Cost: constant.\n//!\n", "")),
        Rule::Cost,
        "a.rs",
    );
    breaks(
        &with("error.rs", replace("Cost: constant.", "Cost:")),
        Rule::Cost,
        "error.rs",
    );
    for placeholder in ["Cost: TBD.", "Cost: unknown.", "Cost: TODO."] {
        breaks(
            &with("error.rs", move |t| {
                t.replace("Cost: constant.", placeholder)
            }),
            Rule::Cost,
            "error.rs",
        );
    }
    // A crate root that holds code is a module, with a cost.
    breaks(
        &with(
            "dev:lib.rs",
            replace("//!\n//! Cost: none at run time.\n", ""),
        ),
        Rule::Cost,
        &format!("{DEV}/src/lib.rs"),
    );
}

#[test]
fn family() {
    breaks(
        &with("a.rs", replace("Family: Query.", "Family: Fillets.")),
        Rule::Family,
        "a.rs",
    );
    breaks(
        &with(
            "lib.rs",
            replace("Family: Combine and 2D.", "Family: Combine and 3D."),
        ),
        Rule::Family,
        "lib.rs",
    );
    // A crate whose job chapter 27 gives as a catalogue family states it.
    let mut input = good();
    input.crates[0].name = "geom-blend".into();
    input.crates[0].manifest = Some(KERNEL_MANIFEST.replace("geom-x", "geom-blend"));
    let found = problems(&input);
    assert!(
        found
            .iter()
            .any(|p| p.rule == Rule::Family && p.message.contains("Blend")),
        "{found:#?}"
    );
}

#[test]
fn source() {
    breaks(
        &with(
            "ported.rs",
            replace(
                "//!   Changed for Spare Kit: rewritten in Rust with checked arithmetic.\n",
                "",
            ),
        ),
        Rule::Source,
        "ported.rs",
    );
    breaks(
        &with(
            "ported.rs",
            replace("//!   Copyright (c) 2020 Example Authors\n", ""),
        ),
        Rule::Source,
        "ported.rs",
    );
    // A "(c)" inside the Changed line is no copyright line.
    breaks(
        &with(
            "ported.rs",
            replace(
                "//!   Copyright (c) 2020 Example Authors\n//!   Changed for Spare Kit: rewritten",
                "//!   Changed for Spare Kit: split case (c), rewritten",
            ),
        ),
        Rule::Source,
        "ported.rs",
    );
    breaks(
        &with(
            "ported.rs",
            replace(
                "blob/0123456789abcdef0123456789abcdef01234567/",
                "blob/main/",
            ),
        ),
        Rule::Source,
        "ported.rs",
    );
    breaks(
        &with(
            "ported.rs",
            replace("Source: PORT from Example", "Source: Example"),
        ),
        Rule::Source,
        "ported.rs",
    );
}

#[test]
fn a_port_needs_a_licence_on_the_allow_list() {
    for licence in [
        "GPL-3.0-only",
        "LGPL-2.1-or-later",
        "AGPL-3.0-only",
        "CC-BY-NC-4.0",
    ] {
        let found = problems(&with("ported.rs", move |t| {
            t.replace("BSD-3-Clause", licence)
        }));
        assert!(
            found.iter().any(|p| p.rule == Rule::Source),
            "{licence}: {found:#?}"
        );
    }
}

#[test]
fn consulted() {
    breaks(
        &with(
            "a.rs",
            replace(
                "J. R. Shewchuk, `predicates.c`, public domain,",
                "Shewchuk's predicates.c,",
            ),
        ),
        Rule::Consulted,
        "a.rs",
    );
    breaks(
        &with("a.rs", replace("public domain,\n", "not a licence!,\n")),
        Rule::Consulted,
        "a.rs",
    );
    // An identifier chapter 29.2 does not classify is no licence.
    for word in ["TBD,\n", "unknown,\n", "`FPU.h`,\n"] {
        breaks(
            &with("a.rs", move |t| t.replace("public domain,\n", word)),
            Rule::Consulted,
            "a.rs",
        );
    }
    breaks(
        &with("a.rs", replace(">, for its signs.", ">.")),
        Rule::Consulted,
        "a.rs",
    );
    breaks(
        &a_with_tail(
            "//! Consulted: X, MIT, <https://github.com/a/b/blob/main/x.rs>, for a test.\n\
             //! SPDX-License-Identifier: MPL-2.0\n",
        ),
        Rule::Consulted,
        "a.rs",
    );
    // A pin on any code host, cgit's in its query.
    breaks(
        &a_with_tail(
            "//! Consulted: msun, BSD-2-Clause,\n//!   <https://cgit.freebsd.org/src/tree/lib/msun/src/s_sinpi.c>, for its reduction.\n",
        ),
        Rule::Consulted,
        "a.rs",
    );
    // Code 29.2 never lets us read.
    breaks(
        &a_with_tail(
            "//! Consulted: TetGen, AGPL-3.0-only,\n//!   <https://example.org/tetgen.cxx>, for its order.\n",
        ),
        Rule::Consulted,
        "a.rs",
    );
    // LGPL code without its method note.
    breaks(
        &with(
            "noted.rs",
            replace(
                "//!   Method note: `projects/engines/geometry/methods/example-widening.md`.\n",
                "",
            ),
        ),
        Rule::Consulted,
        "noted.rs",
    );
    // Two headers that give one source different licences.
    let input = with("a.rs", |t| {
        t.replace(
            "J. R. Shewchuk, `predicates.c`, public domain,",
            "J. R. Shewchuk, `predicates.c`, MIT,",
        )
    });
    let mut input = input;
    let mut b = input.crates[0].files[1].clone();
    b.path = format!("{KERNEL}/src/b.rs");
    b.text = Ok(MODULE_A.to_owned());
    input.crates[0].files.push(b);
    let found = problems(&input);
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(found.iter().all(|p| p.rule == Rule::Consulted));
}

#[test]
fn spdx() {
    breaks(
        &with(
            "ported.rs",
            replace(
                "//! SPDX-License-Identifier: MPL-2.0 AND BSD-3-Clause\n",
                "",
            ),
        ),
        Rule::Spdx,
        "ported.rs",
    );
    breaks(
        &with("ported.rs", replace("MPL-2.0 AND BSD-3-Clause", "MPL-2.0")),
        Rule::Spdx,
        "ported.rs",
    );
    breaks(
        &with(
            "a.rs",
            replace(
                "SPDX-License-Identifier: MPL-2.0",
                "SPDX-License-Identifier: MPL 2!",
            ),
        ),
        Rule::Spdx,
        "a.rs",
    );
    // A module that ports nothing is MPL-2.0.
    breaks(
        &with(
            "a.rs",
            replace(
                "SPDX-License-Identifier: MPL-2.0",
                "SPDX-License-Identifier: GPL-3.0-only",
            ),
        ),
        Rule::Spdx,
        "a.rs",
    );
    // The crate's manifest names a ported file's licence beside MPL-2.0.
    breaks(
        &with_manifest(|m| {
            m.replace(
                "license = \"MPL-2.0 AND BSD-3-Clause\"",
                "license = \"MPL-2.0\"",
            )
        }),
        Rule::Spdx,
        &format!("{KERNEL}/Cargo.toml"),
    );
    breaks(
        &with_manifest(|m| {
            m.replace(
                "license = \"MPL-2.0 AND BSD-3-Clause\"",
                "license.workspace = true",
            )
        }),
        Rule::Spdx,
        &format!("{KERNEL}/Cargo.toml"),
    );
}

#[test]
fn depend() {
    breaks(
        &with(
            "lib.rs",
            replace("//! - MIT: `libm`.\n", "//! - MIT: `libc`.\n"),
        ),
        Rule::Depend,
        "lib.rs",
    );
    breaks(
        &with(
            "lib.rs",
            replace("//! - MIT: `libm`.\n", "//! - GPL-3.0-only: `libm`.\n"),
        ),
        Rule::Depend,
        "lib.rs",
    );
    breaks(
        &with_manifest(|m| format!("{m}\n[build-dependencies]\ncc = \"1.2.0\"\n")),
        Rule::Depend,
        "lib.rs",
    );
}

#[test]
fn uncredited() {
    breaks(
        &with(
            "a.rs",
            replace(
                "//! Consulted: J. R. Shewchuk, `predicates.c`, public domain,\n//!   <https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c>, for its signs.\n",
                "",
            ),
        ),
        Rule::Uncredited,
        "a.rs",
    );
    breaks(
        &with(
            "error.rs",
            replace(
                "Method: none; a closed enum of failures.",
                "Method: as <https://github.com/a/b/blob/0123456789abcdef0123456789abcdef01234567/e.rs>.",
            ),
        ),
        Rule::Uncredited,
        "error.rs",
    );
    // Code reached through a query string, a gist and a source viewer.
    for link in [
        "https://sourceware.org/git/?p=glibc.git;a=blob;f=sysdeps/ieee754/dbl-64/s_sin.c;hb=HEAD",
        "https://gist.github.com/someone/0123456789abcdef0123",
        "https://docs.rs/libm/0.2.16/src/libm/math/sin.rs.html",
    ] {
        breaks(
            &with("error.rs", move |t| {
                t.replace(
                    "Method: none; a closed enum of failures.",
                    &format!("Method: as <{link}>."),
                )
            }),
            Rule::Uncredited,
            "error.rs",
        );
    }
    // In an item's documentation.
    breaks(
        &with(
            "error.rs",
            replace(
                "/// Why it failed.\n///\n/// More detail.\n",
                "/// Why it failed.\n///\n/// As <https://github.com/a/b/blob/main/e.rs> does.\n",
            ),
        ),
        Rule::Uncredited,
        "error.rs",
    );
    // A commit named by an abbreviated name.
    breaks(
        &with(
            "error.rs",
            replace(
                "Guarantees: every kind is documented.",
                "Guarantees: every kind is documented, as bolero's at commit 7642943.",
            ),
        ),
        Rule::Uncredited,
        "error.rs",
    );
}

#[test]
fn error_enum() {
    breaks(
        &with("a.rs", |t| {
            format!("{t}\nfn g() {{\n    pub enum LocalError {{ A }}\n}}\n")
        }),
        Rule::ErrorEnum,
        "a.rs",
    );
}

#[test]
fn error_doc() {
    breaks(
        &with("error.rs", replace("    /// It was degenerate.\n", "")),
        Rule::ErrorDoc,
        "error.rs",
    );
    breaks(
        &with(
            "error.rs",
            replace("/// Why it failed.\n///\n/// More detail.\n", ""),
        ),
        Rule::ErrorDoc,
        "error.rs",
    );
    // An error enum found by its use, not its name.
    breaks(
        &with("error.rs", replace("    /// The mitres do not meet.\n", "")),
        Rule::ErrorDoc,
        "error.rs",
    );
}

/// The good error module with a `kind` function for `XError` whose match arms are `arms`, written
/// as a trait implementation so the module still defines no function of its own.
fn with_kind(arms: &'static str) -> Input {
    with("error.rs", move |t| {
        format!(
            "{t}
impl Detail for XError {{
    fn kind(&self) -> ErrorKind {{
{arms}
    }}
}}
"
        )
    })
}

/// The base kinds, as an `ErrorKind` enum the map finds.
const ERROR_KIND: &str = "
/// The base kinds.
pub enum ErrorKind {
    /// Degenerate.
    Degenerate,
    /// Out of range.
    FrameRangeExceeded,
}
";

#[test]
fn error_base() {
    let good_arms = "        match self {
            XError::Degenerate => ErrorKind::Degenerate,
                                 Self::OutOfRange { .. } => { ErrorKind::FrameRangeExceeded }
        }";
    // With no `ErrorKind` enum in the map, no kind is a base kind.
    breaks(&with_kind(good_arms), Rule::ErrorBase, "error.rs");
    let bad = [
        // The body is not one `match self`.
        "        let k = ErrorKind::Degenerate;
        k",
        // An arm in another form.
        "        match self {
            XError::Degenerate => other(),
                     XError::OutOfRange { .. } => ErrorKind::FrameRangeExceeded,
        }",
        // A pattern in another form.
        "        match self {
            _ => ErrorKind::Degenerate,
        }",
        // A variant with no arm.
        "        match self {
            XError::Degenerate => ErrorKind::Degenerate,
        }",
        // A variant with two.
        "        match self {
            XError::Degenerate | XError::Degenerate => ErrorKind::Degenerate,
                     XError::OutOfRange { .. } => ErrorKind::FrameRangeExceeded,
        }",
        // A variant that does not exist.
        "        match self {
            XError::Degenerate => ErrorKind::Degenerate,
                     XError::OutOfRange { .. } => ErrorKind::FrameRangeExceeded,
                     XError::Gone => ErrorKind::Degenerate,
        }",
        // A kind that is not a variant of `ErrorKind`.
        "        match self {
            XError::Degenerate => ErrorKind::Degenerate,
                     XError::OutOfRange { .. } => ErrorKind::NoSolution,
        }",
    ];
    for arms in bad {
        let input = with_kind(arms);
        let input = {
            let mut input = input;
            for f in &mut input.crates[0].files {
                if f.path.ends_with("/error.rs") {
                    f.text = Ok(format!("{}{ERROR_KIND}", f.text.clone().unwrap()));
                }
            }
            input
        };
        breaks(&input, Rule::ErrorBase, "error.rs");
    }
}

#[test]
fn base_kinds_are_read_from_the_kind_function() {
    let arms = "        match *self {
            XError::Degenerate => crate::ErrorKind::Degenerate,
                            Self::OutOfRange { .. } => {
                ErrorKind::FrameRangeExceeded
            }
        }";
    let mut input = with_kind(arms);
    for f in &mut input.crates[0].files {
        if f.path.ends_with("/error.rs") {
            f.text = Ok(format!(
                "{}{ERROR_KIND}
/// A detail that holds another.
pub enum HeldError {{
                     /// The held one.
    X(XError),
}}

impl HeldError {{
                     pub fn kind(&self) -> ErrorKind {{
        match self {{
                             HeldError::X(detail) => detail.kind(),
        }}
    }}
}}
",
                f.text.clone().unwrap()
            ));
        }
    }
    // The inherent `kind` makes the module define a function, so its Method needs a link.
    let generated = build(&input, &[]);
    let problems: Vec<String> = generated
        .problems
        .iter()
        .filter(|p| p.rule != Rule::Method)
        .map(ToString::to_string)
        .collect();
    assert_eq!(problems, Vec::<String>::new());
    let map: serde_json::Value = serde_json::from_str(&generated.text).unwrap();
    let krate = map["crates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "geom-x")
        .unwrap();
    let errors = krate["errors"].as_array().unwrap();
    let kinds = |name: &str| -> Vec<(String, Option<String>)> {
        errors.iter().find(|e| e["name"] == name).unwrap()["kinds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| {
                (
                    k["name"].as_str().unwrap().to_owned(),
                    k.get("base_kind").map(|b| b.as_str().unwrap().to_owned()),
                )
            })
            .collect()
    };
    assert_eq!(
        kinds("XError"),
        [
            ("Degenerate".to_owned(), Some("Degenerate".to_owned())),
            (
                "OutOfRange".to_owned(),
                Some("FrameRangeExceeded".to_owned())
            ),
        ]
    );
    assert_eq!(
        kinds("HeldError"),
        [("X".to_owned(), Some("held detail".to_owned()))]
    );
    assert_eq!(
        kinds("ErrorKind"),
        [
            ("Degenerate".to_owned(), None),
            ("FrameRangeExceeded".to_owned(), None),
        ]
    );
    // An enum without a `kind` function lists no base kinds.
    assert_eq!(kinds("SweepFailure"), [("MitreMismatch".to_owned(), None)]);
}

#[test]
fn stale() {
    let text = build(&good(), &[]).text;
    assert_eq!(compare(Some(&text), &text), None);
    // A Windows checkout's line ends are not a difference.
    assert_eq!(compare(Some(&text.replace('\n', "\r\n")), &text), None);
    for committed in [
        None,
        Some("{}\n".to_owned()),
        Some(text.replace("geom-x", "geom-z")),
    ] {
        let p = compare(committed.as_deref(), &text).unwrap();
        assert_eq!(p.rule, Rule::Stale);
        assert_eq!(p.file, "projects/engines/geometry/MAP.json");
    }
    let p = compare(Some(&text.replacen("\"kernel\"", "\"kernal\"", 1)), &text).unwrap();
    let line = text.lines().position(|l| l.contains("\"kernel\"")).unwrap() + 1;
    assert!(
        p.message.contains(&format!("from line {line}")),
        "{}",
        p.message
    );
}

#[test]
fn gaps() {
    const COST: Gap = Gap {
        file: "projects/engines/geometry/crates/geom-x/src/a.rs",
        rule: Rule::Cost,
        problem: "no paragraph opens with \"Cost: \" stating what the module costs",
        reason: "a test gap",
    };
    // A gap that occurs hides its problem and is listed in the map.
    let bad = with("a.rs", replace("//! Cost: constant.\n//!\n", ""));
    let generated = build(&bad, &[COST]);
    assert_eq!(generated.problems, []);
    let map: serde_json::Value = serde_json::from_str(&generated.text).unwrap();
    assert_eq!(map["gaps"][0]["rule"], "Cost");
    assert_eq!(map["gaps"][0]["problem"], COST.problem);
    assert_eq!(map["gaps"][0]["reason"], "a test gap");
    // A gap that no longer occurs is a problem.
    let found = build(&good(), &[COST]).problems;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule, Rule::GapStale);
    assert_eq!(found[0].file, COST.file);
    // A gap hides only the problem it names: another problem of the same file and rule is
    // reported, and the gap is stale.
    let other = with("a.rs", replace("Cost: constant.", "Cost: TODO."));
    let found = build(&other, &[COST]).problems;
    let rules: Vec<Rule> = found.iter().map(|p| p.rule).collect();
    assert_eq!(rules, [Rule::Cost, Rule::GapStale], "{found:#?}");
}

#[test]
fn every_rule_has_a_seeded_fixture_and_a_label() {
    // The tests above, by rule. A new rule must get its fixtures before this list grows.
    let seeded = [
        Rule::Manifest,
        Rule::Root,
        Rule::Unreadable,
        Rule::Header,
        Rule::Summary,
        Rule::Job,
        Rule::Layer,
        Rule::Invariants,
        Rule::Sections,
        Rule::Guarantees,
        Rule::Method,
        Rule::Cost,
        Rule::Family,
        Rule::Source,
        Rule::Consulted,
        Rule::Spdx,
        Rule::Depend,
        Rule::Uncredited,
        Rule::ErrorEnum,
        Rule::ErrorDoc,
        Rule::ErrorBase,
        Rule::Covers,
        Rule::Stale,
        Rule::GapStale,
    ];
    assert_eq!(seeded, Rule::ALL);
    let mut labels: Vec<&str> = Rule::ALL.iter().map(|r| r.label()).collect();
    labels.sort_unstable();
    labels.dedup();
    assert_eq!(labels.len(), Rule::ALL.len());
}

/// The good workspace's map, byte for byte, as committed in `golden/map-fixture.json`, on every
/// target CI runs, the 32-bit wasm32-wasip1 included: the map's text depends on nothing but its
/// input. Line ends are compared as `\n`, because a Windows checkout may convert the file's.
///
/// When the map's format changes on purpose, regenerate the golden with
/// `cargo test -p geom-test --test map_seeded -- --ignored --nocapture print_the_fixture_map` and
/// copy the text between the markers into the file.
#[test]
fn the_fixture_map_is_the_same_bytes_on_every_target() {
    let golden = include_str!("golden/map-fixture.json").replace("\r\n", "\n");
    let text = build(&good(), &[]).text;
    assert!(
        text == golden,
        "the fixture map differs from tests/golden/map-fixture.json:\n{text}"
    );
}

/// Not a check: prints the good workspace's map, for regenerating the golden.
#[test]
#[ignore = "run by hand to regenerate tests/golden/map-fixture.json"]
fn print_the_fixture_map() {
    println!(
        "--- begin map ---\n{}--- end map ---",
        build(&good(), &[]).text
    );
}
