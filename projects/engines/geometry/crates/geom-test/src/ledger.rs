//! The kernel's source ledger and notices: `projects/engines/geometry/PROVENANCE.md` and
//! `projects/engines/geometry/NOTICE`, generated from the map's provenance records, and the check
//! that both committed files are current.
//!
//! What it does: [`build`] takes the map as a value ([`crate::map::Generated::map`], the records
//! `MAP.json` holds), the hand-kept licence texts under `projects/engines/geometry/notices/`
//! ([`notices`]) and the method notes under `projects/engines/geometry/methods/` ([`methods`]),
//! and returns the text of both files with every [`Problem`]: a ported source with no licence
//! text, a licence text no port uses, a method note that breaks its template, a record whose
//! method note credits another source, or a map whose headers are incomplete. [`repo`] reads the
//! inputs from a workspace on disk and writes or checks the committed files; the binary
//! `provenance` (`cargo run -p geom-test --bin provenance -- check`) runs it in CI.
//!
//! Why: chapter 29.20, rule 4, says the generator that builds `MAP.json` reads the `Source:` and
//! `Consulted:` lines into one record per source, and "the same records produce
//! `geometry/PROVENANCE.md` and `geometry/NOTICE`, so they can never drift from the code"; rule 5
//! says CI compares the headers with `PROVENANCE.md`. Reading the records from the map, rather
//! than from the headers again, keeps one parser: the map's ([`crate::map::provenance`]).
//!
//! Guarantees:
//!
//! - **One text per input.** Sources, files and notes are written in a fixed order (sorted by
//!   class, name, link and path), with `\n` line ends, no timestamps and no absolute paths, so the
//!   same tree gives the same bytes on every target.
//! - **Every record is in the ledger.** Every `port`, `port-method` and `depend` source of the map
//!   is a row of `PROVENANCE.md`, with every file that takes it and what each took, and every
//!   method note is listed with the records that name it.
//! - **Every ported source carries its licence text and copyright lines.** Every source a `Source:`
//!   record ports, grouped by repository and commit, has exactly one licence text under
//!   `notices/`, for the same repository, commit and licence, and `NOTICE` reproduces it in full
//!   with the copyright lines each ported file's header copies; a port without one, a licence
//!   text no port uses, a licence that differs, and an origin file whose `Source:` record copies
//!   no copyright line for it are problems ([`Rule::Notice`]).
//! - **Every source consulted is credited.** Every `Consulted:` record's source is in `NOTICE`'s
//!   credits, with its licence, link, method note and what each file consulted it for (rule 4: a
//!   PORT METHOD module "credits every source it consulted").
//! - **Every method note keeps the procedure's record.** A note that breaks the template, holds
//!   code, is named by a record that credits another licence or repository, or does not record a
//!   module whose record names it is a problem
//!   ([`Rule::Note`]; [`methods`]).
//! - **Nothing generated from an incomplete map passes.** When the map's headers have problems,
//!   a record may be missing, so the check reports it ([`Rule::Map`]) and the map's own check names
//!   each one. A source the map lists that no file records, and a file's record that no source of
//!   the map holds, are [`Rule::Map`] problems too, so no record leaves the ledger in silence.
//!
//! Method: regenerate and compare, as the map does ([`crate::map`]) and as chapter 27 keeps every
//! generated file honest ("Generated code and data have one home",
//! `projects/engines/geometry/architecture/27-crates-folders.md`); the ledger's content is chapter
//! 29.20's rules 4 and 5 and its `SourceRecord`
//! (`projects/engines/geometry/architecture/29-sources-licences.md`); the notice file follows the
//! Apache License 2.0's NOTICE convention, section 4(d), of carrying attribution notices in one
//! readable text file distributed with the work (<https://www.apache.org/licenses/LICENSE-2.0>).
//!
//! Cost: linear in the map's records, the licence texts and the notes, plus sorting them.

pub mod methods;
pub mod notices;
pub mod render;
pub mod repo;

use core::fmt;
use std::collections::BTreeMap;

use crate::licence::Expr;
use crate::map::json::Value;
use crate::map::{SourceFile, links};

/// The generated ledger, relative to the workspace root.
pub const PROVENANCE_FILE: &str = "projects/engines/geometry/PROVENANCE.md";

/// The generated notices, relative to the workspace root.
pub const NOTICE_FILE: &str = "projects/engines/geometry/NOTICE";

/// The folder of hand-kept licence texts, one per ported source, relative to the workspace root.
pub const NOTICES_DIR: &str = "projects/engines/geometry/notices/";

/// The command that writes both files.
pub const WRITE_COMMAND: &str = "cargo run -p geom-test --bin provenance -- write";

/// What a ledger problem breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    /// A ported source without its licence text, a licence text no port uses, or one malformed.
    Notice,
    /// A method note that breaks the template or credits another source than its records.
    Note,
    /// The map's headers have problems, so the ledger may lack records.
    Map,
    /// A committed file is missing or differs from the generated one.
    Stale,
}

impl Rule {
    /// Every rule.
    pub const ALL: [Rule; 4] = [Rule::Notice, Rule::Note, Rule::Map, Rule::Stale];

    /// The rule's name, as every problem message writes it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Rule::Notice => "Notice",
            Rule::Note => "Method note",
            Rule::Map => "Map",
            Rule::Stale => "Stale ledger",
        }
    }
}

/// What is wrong, where, and by which rule.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Problem {
    /// The file, relative to the workspace root, with `/`.
    pub file: String,
    /// The rule broken.
    pub rule: Rule,
    /// What is wrong and what to do.
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.file, self.rule.label(), self.message)
    }
}

/// Everything the ledger is built from.
#[derive(Clone, Copy, Debug)]
pub struct Input<'a> {
    /// The map, as [`crate::map::build`] returns it.
    pub map: &'a Value,
    /// How many problems the map's headers have ([`crate::map::Generated::problems`]).
    pub map_problems: usize,
    /// Every file directly under [`NOTICES_DIR`], in path order.
    pub notices: &'a [SourceFile],
    /// Every file directly under [`crate::map::provenance::METHODS_FOLDER`], in path order.
    pub notes: &'a [SourceFile],
}

/// The generated files and every problem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ledger {
    /// The text of `PROVENANCE.md`.
    pub provenance: String,
    /// The text of `NOTICE`.
    pub notice: String,
    /// Every problem, sorted by file, rule and message.
    pub problems: Vec<Problem>,
}

/// One file's record of a source: the file and the record as the map holds it.
#[derive(Clone, Copy, Debug)]
pub struct Use<'a> {
    /// The file whose header (or, for `depend`, whose manifest) holds the record.
    pub file: &'a str,
    /// That file's `SPDX-License-Identifier:` expression, or empty.
    pub spdx: &'a str,
    /// The record: `name`, `licence`, `link`, `purpose`, `notice`, `method_note`, `take`, and for
    /// `depend` the `manifest`.
    pub record: &'a Value,
}

/// One source of the map, merged by how it was taken and its link, with every file's record of it.
#[derive(Clone, Debug)]
pub struct Source<'a> {
    /// The map's merged record: `name`, `licence`, `link`, `pinned`, `take`, `method_note`,
    /// `used_by`.
    pub summary: &'a Value,
    /// Every file's record of it, in file order.
    pub uses: Vec<Use<'a>>,
}

impl Source<'_> {
    /// How it was taken: `port`, `port-method` or `depend`.
    #[must_use]
    pub fn take(&self) -> &str {
        self.summary.text("take")
    }
}

/// A licence expression in canonical form, or the text itself when it does not parse.
#[must_use]
pub fn canonical(licence: &str) -> String {
    Expr::parse(licence).map_or_else(|_| licence.to_owned(), |e| e.canonical())
}

/// Sources keyed as the map merges them, each with every file's record of it.
type Uses<'a> = BTreeMap<(String, String), Vec<Use<'a>>>;

/// Adds one file's `records` to `out`, keyed as the map merges them: by how each source was taken
/// and its link without the fragment. A `depend` record names its manifest, which is its file.
fn add_uses<'a>(out: &mut Uses<'a>, file: &'a str, spdx: &'a str, records: &'a [Value]) {
    for record in records {
        let key = (
            record.text("take").to_owned(),
            links::source_key(record.text("link")),
        );
        let file = record
            .get("manifest")
            .and_then(Value::as_str)
            .unwrap_or(file);
        out.entry(key).or_default().push(Use { file, spdx, record });
    }
}

/// Every file's record of every source in `map`.
fn uses(map: &Value) -> Uses<'_> {
    let mut out = Uses::new();
    for krate in map.list("crates") {
        let file = krate.text("file");
        add_uses(&mut out, file, krate.text("spdx"), krate.list("provenance"));
        add_uses(&mut out, file, "", krate.list("depends"));
        for module in krate.list("modules") {
            add_uses(
                &mut out,
                module.text("file"),
                module.text("spdx"),
                module.list("provenance"),
            );
        }
    }
    for list in out.values_mut() {
        list.sort_by(|a, b| a.file.cmp(b.file));
    }
    out
}

/// The map's sources, each with every file's record of it.
#[must_use]
pub fn sources(map: &Value) -> Vec<Source<'_>> {
    let mut by_key = uses(map);
    map.list("sources")
        .iter()
        .map(|summary| {
            let key = (
                summary.text("take").to_owned(),
                summary.text("link").to_owned(),
            );
            Source {
                summary,
                uses: by_key.remove(&key).unwrap_or_default(),
            }
        })
        .collect()
}

/// The repository and commit of a pinned link, or `None` when it is not on a code-hosting site or
/// names no revision.
#[must_use]
pub fn repository_and_commit(link: &str) -> Option<(String, String)> {
    let view = links::view(link)?;
    Some((view.repository, view.revision?))
}

/// The origin files of one port record, each with the lines its notice copies: the record's link
/// first, then every further link the notice names on a line of its own, each followed by the
/// lines after it (the origin's copyright lines). A run of links with no line between them shares
/// the lines after the last link of the run, as a header writes two files of one author ("`a.c`
/// link, `b.c` link, then their one copyright line"). Links are written without their fragment.
#[must_use]
pub fn origins(record: &Value) -> Vec<(String, Vec<String>)> {
    let mut out = origin_runs(record);
    let mut shared: Option<Vec<String>> = None;
    for (_, lines) in out.iter_mut().rev() {
        if lines.is_empty() {
            if let Some(next) = &shared {
                lines.clone_from(next);
            }
        } else {
            shared = Some(lines.clone());
        }
    }
    out
}

/// The origin files of one port record, each with only the lines written directly after its own
/// link ([`origins`] shares a run's lines).
fn origin_runs(record: &Value) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> =
        vec![(links::source_key(record.text("link")), Vec::new())];
    for line in record.list("notice").iter().filter_map(Value::as_str) {
        let bare = line
            .trim()
            .trim_start_matches('<')
            .trim_end_matches(['>', ',', '.', ';']);
        let is_link = links::find(bare)
            .first()
            .is_some_and(|l| l.kind == links::LinkKind::Web && l.at == 0 && l.text == bare);
        if is_link {
            out.push((links::source_key(bare), Vec::new()));
        } else if let Some(last) = out.last_mut() {
            last.1.push(line.to_owned());
        }
    }
    out
}

/// Reports every disagreement between the map's merged `sources` and the files' records of them:
/// a source no file records, and a file's record that no source holds. The map writes both from the
/// same headers, so neither occurs in a map [`crate::map::build`] made; a map edited by hand, or
/// a change to how the map merges records, would otherwise drop a record from the ledger in
/// silence.
fn reconcile(map: &Value, sources: &[Source<'_>], problems: &mut Vec<Problem>) {
    for source in sources.iter().filter(|s| s.uses.is_empty()) {
        problems.push(Problem {
            file: crate::map::MAP_FILE.to_owned(),
            rule: Rule::Map,
            message: format!(
                "the map lists the source {} ({}) but no file's record of it, so the ledger \
                 cannot say who takes it: run `cargo run -p geom-test --bin map -- write`",
                source.summary.text("name"),
                source.summary.text("link")
            ),
        });
    }
    let listed: Vec<(&str, &str)> = sources
        .iter()
        .map(|s| (s.take(), s.summary.text("link")))
        .collect();
    for ((take, link), list) in uses(map) {
        if listed.contains(&(take.as_str(), link.as_str())) {
            continue;
        }
        for used in list {
            problems.push(Problem {
                file: used.file.to_owned(),
                rule: Rule::Map,
                message: format!(
                    "the {take} record of {} ({link}) is in no source of the map, so the ledger \
                     would leave it out: run `cargo run -p geom-test --bin map -- write`",
                    used.record.text("name")
                ),
            });
        }
    }
}

/// Builds `PROVENANCE.md` and `NOTICE` from `input`, with every problem.
#[must_use]
pub fn build(input: &Input<'_>) -> Ledger {
    let mut problems = Vec::new();
    if input.map_problems > 0 {
        problems.push(Problem {
            file: crate::map::MAP_FILE.to_owned(),
            rule: Rule::Map,
            message: format!(
                "the crate and module headers have {} problem(s), so a provenance record may be \
                 missing from the ledger: run `cargo run -p geom-test --bin map -- check` and fix \
                 each one",
                input.map_problems
            ),
        });
    }
    let sources = sources(input.map);
    reconcile(input.map, &sources, &mut problems);
    let read_notices = notices::read(input.notices, &mut problems);
    let ports = notices::match_ports(&sources, &read_notices, &mut problems);
    let notes = methods::read(input.notes, &mut problems);
    methods::check_records(&sources, &notes, &mut problems);
    methods::check_code_record(&sources, &notes, &mut problems);
    let provenance = render::provenance(&sources, &notes);
    let notice = render::notice(&sources, &ports);
    problems.sort();
    problems.dedup();
    Ledger {
        provenance,
        notice,
        problems,
    }
}

/// The problem of comparing a committed file with its generated text: line ends are compared as
/// `\n` (a Windows checkout may turn them into `\r\n`), every other byte exactly.
#[must_use]
pub fn compare(file: &str, committed: Option<&str>, generated: &str) -> Option<Problem> {
    let problem = |message: String| Problem {
        file: file.to_owned(),
        rule: Rule::Stale,
        message,
    };
    let Some(committed) = committed else {
        return Some(problem(format!(
            "the file is missing; run `{WRITE_COMMAND}` and commit it"
        )));
    };
    let committed = committed.replace("\r\n", "\n");
    if committed == generated {
        return None;
    }
    let line = committed
        .lines()
        .zip(generated.lines())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| committed.lines().count().min(generated.lines().count()))
        .saturating_add(1);
    Some(problem(format!(
        "the committed file differs from the records from line {line}; run `{WRITE_COMMAND}` and \
         commit the result"
    )))
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;
    use crate::map::json::object;

    #[test]
    fn a_stale_file_names_its_first_differing_line() {
        assert_eq!(compare("F", Some("a\r\nb\n"), "a\nb\n"), None);
        let p = compare("F", Some("a\nb\n"), "a\nc\n").unwrap();
        assert_eq!(p.rule, Rule::Stale);
        assert!(p.message.contains("from line 2"), "{}", p.message);
        let p = compare("F", Some("a\n"), "a\nb\n").unwrap();
        assert!(p.message.contains("from line 2"), "{}", p.message);
        let p = compare("F", None, "a\n").unwrap();
        assert!(p.message.contains("missing"), "{}", p.message);
    }

    #[test]
    fn repositories_and_commits_of_pinned_links() {
        let sha = "0ce0cbc15935e24c656280e5a46c34e89f301b7a";
        assert_eq!(
            repository_and_commit(&format!(
                "https://gitlab.inria.fr/core-math/core-math/-/blob/{sha}/src/a.c"
            )),
            Some((
                "gitlab.inria.fr/core-math/core-math".to_owned(),
                sha.to_owned()
            ))
        );
        assert_eq!(repository_and_commit("https://doi.org/10.1/x"), None);
        assert_eq!(repository_and_commit("https://github.com/a/b"), None);
    }

    #[test]
    fn origins_split_at_each_further_link() {
        let record = object([
            ("link", Value::from("https://h.org/a.c#L1")),
            (
                "notice",
                Value::from(vec![
                    "Copyright (c) 1 A".to_owned(),
                    "Authors: A.".to_owned(),
                    "<https://h.org/b.h>".to_owned(),
                    "Copyright (c) 2 B".to_owned(),
                ]),
            ),
        ]);
        assert_eq!(
            origins(&record),
            [
                (
                    "https://h.org/a.c".to_owned(),
                    vec!["Copyright (c) 1 A".to_owned(), "Authors: A.".to_owned()]
                ),
                (
                    "https://h.org/b.h".to_owned(),
                    vec!["Copyright (c) 2 B".to_owned()]
                ),
            ]
        );
    }

    #[test]
    fn a_run_of_links_shares_the_lines_after_it() {
        let record = object([
            ("link", Value::from("https://h.org/a.c")),
            (
                "notice",
                Value::from(vec![
                    "https://h.org/b.c".to_owned(),
                    "Copyright (c) 1 A".to_owned(),
                    "https://h.org/c.c".to_owned(),
                    "https://h.org/d.c".to_owned(),
                    "Copyright (c) 2 B".to_owned(),
                    "https://h.org/e.c".to_owned(),
                ]),
            ),
        ]);
        let found = origins(&record);
        let lines: Vec<(&str, Vec<&str>)> = found
            .iter()
            .map(|(l, v)| (l.as_str(), v.iter().map(String::as_str).collect()))
            .collect();
        assert_eq!(
            lines,
            [
                ("https://h.org/a.c", vec!["Copyright (c) 1 A"]),
                ("https://h.org/b.c", vec!["Copyright (c) 1 A"]),
                ("https://h.org/c.c", vec!["Copyright (c) 2 B"]),
                ("https://h.org/d.c", vec!["Copyright (c) 2 B"]),
                // The last link has no line after it, and no run to share one with.
                ("https://h.org/e.c", vec![]),
            ]
        );
    }

    #[test]
    fn canonical_licences() {
        assert_eq!(
            canonical("Apache-2.0 OR MIT"),
            canonical("MIT OR Apache-2.0")
        );
        assert_eq!(canonical("public domain"), "public domain");
    }
}
