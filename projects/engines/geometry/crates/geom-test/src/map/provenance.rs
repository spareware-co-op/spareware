//! The provenance lines of a header: `Source:` for a ported file, `Consulted:` for a source a
//! module consulted, `SPDX-License-Identifier:`, and the rule that every link to outside code is
//! credited.
//!
//! What it does: [`read`] parses a header's provenance records
//! ([`crate::map::header::Block::Record`]) into [`Record`]s and its licence line, and reports every
//! record that is incomplete or takes more than chapter 29.2 allows, and every link to source code
//! in the header's other text or in the file's doc comments that no record credits.
//!
//! Why: chapter 29.20, rule 4, says how a ported file and a module that consulted a source start,
//! and that the generator that builds `MAP.json` reads these lines into one record per source, so
//! the source ledger can never drift from the code. A record the generator cannot read, a source
//! linked but credited nowhere, or a licence 29.2 does not allow would leave a hole in that ledger.
//!
//! Guarantees:
//!
//! - **A `Source:` record is the whole notice, of code 29.2 lets us port.** It reads
//!   `Source: PORT from <name>, <licence>,` with a licence met by 29.2's allow-list
//!   ([`classes::Class::Allowed`]), a link pinned to a full commit, at least one copyright line
//!   (a line before `Changed for Spare Kit:` that opens with `Copyright`, `©`, `(c)` or
//!   `SPDX-FileCopyrightText:`, as the REUSE specification writes copyright notices), and a
//!   `Changed for Spare Kit: ` line that says what changed; the header carries an
//!   `SPDX-License-Identifier:` line met by the allow-list whose expression is that licence or has
//!   it as an `AND` operand. Anything else is a [`Rule::Source`] or [`Rule::Spdx`] problem.
//! - **A `Consulted:` record names a real licence and a read 29.2 allows.** It reads
//!   `Consulted: <name>, <licence>, <link>, <what for>`, the link a web address or a DOI; the
//!   licence is classified by [`classes`] (an identifier 29.2 does not classify is a problem); a
//!   link on a code-hosting site is pinned to a full commit; and code under a licence 29.2 never
//!   lets us read (AGPL, non-commercial or custom terms) is a problem, while LGPL code, and GPL
//!   code an owner allowed, carry a `Method note:` line naming the method-only procedure's note
//!   under `projects/engines/geometry/methods/` (rule 6). Anything else is a [`Rule::Consulted`]
//!   problem.
//! - **A module that ports nothing is ours.** Without a `Source:` record, an
//!   `SPDX-License-Identifier:` line, if any, is `MPL-2.0` (29.20, rule 4: "A PORT METHOD module is
//!   MPL-2.0 and ours").
//! - **Every link to outside code is credited.** A link to source code ([`links::is_code`])
//!   anywhere else in the header or in the file's doc comments needs a `Source:` or `Consulted:`
//!   record in the same header for the same repository (on a code-hosting site) or the same file
//!   (elsewhere), and a commit named by an abbreviated name (the word "commit" and its first
//!   seven to 39 hexadecimal digits) is a source read but not credited with a pinned link; either
//!   is a [`Rule::Uncredited`] problem.
//!
//! Method: the header forms of chapter 29.20, rule 4, the `SourceRecord` it proposes and the
//! classes of 29.2 (`projects/engines/geometry/architecture/29-sources-licences.md`); SPDX license
//! expressions per the SPDX specification 2.3, annex D
//! (<https://spdx.github.io/spdx-spec/v2.3/SPDX-license-expressions/>); the
//! `SPDX-License-Identifier:` line per its annex E
//! (<https://spdx.github.io/spdx-spec/v2.3/using-SPDX-short-identifiers-in-source-files/>); the
//! copyright notice's opening words per the REUSE specification 3.3
//! (<https://reuse.software/spec-3.3/>), with `(c)` added because an origin's lines are copied
//! exactly, however they open.
//!
//! Cost: linear in the header's and the doc comments' length, times the number of records for the
//! credit check.

use crate::licence::Expr;
use crate::map::Rule;
use crate::map::classes::{self, Class, LicenceError};
use crate::map::header::{Block, Header, join, states};
use crate::map::links;

/// How a source was taken: chapter 29.20's `Take`, for the classes a header or manifest records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Take {
    /// PORT: code translated into our Rust, with its notice (a `Source:` record).
    Port,
    /// PORT METHOD: a source a module that implements a published method consulted (a
    /// `Consulted:` record).
    PortMethod,
    /// DEPEND: a published crate a kernel crate's manifest names, used as it is.
    Depend,
}

impl Take {
    /// The class's name in the map.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Take::Port => "port",
            Take::PortMethod => "port-method",
            Take::Depend => "depend",
        }
    }
}

/// One provenance record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// How it was taken.
    pub take: Take,
    /// The source's name.
    pub name: String,
    /// Its licence, as written: an SPDX expression or `public domain`.
    pub licence: String,
    /// The link to it.
    pub link: String,
    /// Whether the link names a full commit.
    pub pinned: bool,
    /// What it was consulted for, or what was changed in a port.
    pub purpose: String,
    /// The method-only procedure's note (29.20, rule 6), when the record names one.
    pub method_note: Option<String>,
    /// For a `Source:` record, the origin's notice as the record copies it: every line after the
    /// line holding the record's link and before `Changed for Spare Kit: `, trimmed. These are the
    /// origin's copyright lines, copied exactly, and the links of any further origin files the
    /// port takes, each followed by its own copyright lines; `NOTICE` reproduces them
    /// ([`crate::ledger`]). Empty for a `Consulted:` record.
    pub notice: Vec<String>,
}

/// A header's provenance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Provenance {
    /// The records, in header order.
    pub records: Vec<Record>,
    /// The `SPDX-License-Identifier:` expression, if the header has one.
    pub spdx: Option<String>,
    /// What is wrong, each with its rule.
    pub problems: Vec<(Rule, String)>,
}

/// The label of a `Consulted:` record's method-note line.
pub const METHOD_NOTE: &str = "Method note: ";

/// The folder the method-only procedure's notes live in (29.20, rule 6).
pub const METHODS_FOLDER: &str = "projects/engines/geometry/methods/";

/// The label of a `Source:` record's line that says what changed.
pub const CHANGED: &str = "Changed for Spare Kit: ";

/// The words a copyright line opens with (REUSE 3.3, and `(c)`), compared without case.
pub const COPYRIGHT_OPENINGS: [&str; 4] = ["spdx-filecopyrighttext:", "copyright", "©", "(c)"];

/// The text before the first web address or DOI of a record, the link, and the text after it.
fn split_at_link(text: &str) -> Option<(&str, String, &str)> {
    let link = links::find(text)
        .into_iter()
        .find(|l| matches!(l.kind, links::LinkKind::Web | links::LinkKind::Doi))?;
    let before = text.get(..link.at)?;
    let after = text.get(link.at..)?;
    let after = after.strip_prefix(link.text.as_str()).unwrap_or(after);
    let before = before.trim_end().trim_end_matches('<').trim_end();
    let after = after.trim_start_matches('>');
    Some((before, link.text, after))
}

/// `<name>, <licence>,` split at its last `, ` (a name may hold commas; a licence never does). A
/// licence may be written in backticks, as Clippy's `doc_markdown` lint asks of an identifier such
/// as `LicenseRef-Commercial`; the backticks are not part of it.
fn name_and_licence(before: &str) -> Option<(String, String)> {
    let before = before.trim().trim_end_matches(',').trim_end();
    let (name, licence) = before.rsplit_once(", ")?;
    let (name, licence) = (name.trim(), licence.trim().trim_matches('`').trim());
    if name.is_empty() || licence.is_empty() {
        return None;
    }
    Some((name.to_owned(), licence.to_owned()))
}

/// The class of `licence`, or the problem with it.
fn class_of(licence: &str) -> Result<Class, String> {
    classes::of(licence).map_err(|e| match e {
        LicenceError::Syntax(why) => format!(
            "the licence {licence:?} is neither an SPDX expression nor \"public domain\" ({why})"
        ),
        LicenceError::Unclassified(u) => format!(
            "the licence {licence:?} names {:?}, which chapter 29.2 does not classify: write the \
             source's licence exactly as its LICENSE file, file headers and manifest state it at \
             the pinned commit (29.20, rule 1), and if 29.2 has no row for it, stop and ask an owner",
            u.id
        ),
    })
}

/// Reads one `Consulted:` record.
fn consulted(lines: &[String], out: &mut Provenance) {
    let text = join(lines);
    let rest = text.strip_prefix("Consulted:").unwrap_or_default().trim();
    let (rest, note) = match rest.split_once(METHOD_NOTE) {
        Some((before, note)) => (before.trim(), Some(note)),
        None => (rest, None),
    };
    let form = "\"Consulted: <name>, <licence>, <link>, <what for>\"";
    let Some((before, link, after)) = split_at_link(rest) else {
        out.problems.push((
            Rule::Consulted,
            format!("{rest:?} has no link to the source; write {form}"),
        ));
        return;
    };
    let Some((name, licence)) = name_and_licence(before) else {
        out.problems.push((
            Rule::Consulted,
            format!("{rest:?} does not give a name and a licence before its link; write {form}"),
        ));
        return;
    };
    let class = class_of(&licence);
    if let Err(problem) = &class {
        out.problems.push((Rule::Consulted, problem.clone()));
    }
    let purpose = after.trim().trim_start_matches(',').trim().to_owned();
    if !states(&purpose) {
        out.problems.push((
            Rule::Consulted,
            format!("the record for {name} does not say what the source was consulted for"),
        ));
    }
    let pinned = links::is_pinned(&link);
    if links::on_forge(&link) && !pinned {
        out.problems.push((
            Rule::Consulted,
            format!("the link {link} is on a code-hosting site but names no full commit"),
        ));
    }
    let method_note = note.map(|note| method_note(&name, note, out));
    if links::is_code(&link)
        && let Ok(class) = class
    {
        check_read(&name, &licence, class, method_note.is_some(), out);
    }
    out.records.push(Record {
        take: Take::PortMethod,
        name,
        licence,
        link,
        pinned,
        purpose,
        method_note: method_note.filter(|p| !p.is_empty()),
        notice: Vec::new(),
    });
}

/// The note a record's method-note line names (empty, with a problem, when it names none).
fn method_note(name: &str, note: &str, out: &mut Provenance) -> String {
    let path = links::find(note)
        .into_iter()
        .find(|l| l.kind == links::LinkKind::Repo)
        .map(|l| l.text)
        .filter(|p| {
            p.starts_with(METHODS_FOLDER)
                && std::path::Path::new(p)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("md"))
        });
    if path.is_none() {
        out.problems.push((
            Rule::Consulted,
            format!(
                "the record for {name} has a method-note line that names no note: write \
                 \"{METHOD_NOTE}`{METHODS_FOLDER}<note>.md`\""
            ),
        ));
    }
    path.unwrap_or_default()
}

/// Whether chapter 29.2 lets code under `licence`, of `class`, be read as the record `name` says
/// it was: never for the never-read class, and with a method note for LGPL and GPL code.
fn check_read(name: &str, licence: &str, class: Class, noted: bool, out: &mut Provenance) {
    match class {
        Class::Never => out.problems.push((
            Rule::Consulted,
            format!(
                "the record for {name} says code under {licence} was read, which chapter 29.2 \
                 never allows (AGPL, non-commercial and custom terms: \"Read the source? No\"); \
                 take nothing from it, delete the record, and tell an owner"
            ),
        )),
        Class::OwnerOnly | Class::MethodOnly if !noted => {
            let owner = if class == Class::OwnerOnly {
                " after a kernel owner allowed reading it in chapter 29's row"
            } else {
                ""
            };
            out.problems.push((
                Rule::Consulted,
                format!(
                    "the record for {name} reads code under {licence}, which chapter 29.2 allows \
                     only{owner} under the method-only procedure (29.20, rule 6): add a \
                     \"{METHOD_NOTE}`{METHODS_FOLDER}<note>.md`\" line naming its note"
                ),
            ));
        }
        _ => {}
    }
}

/// Whether `line` opens as a copyright notice does and goes on to name a year or a holder.
fn is_copyright_line(line: &str) -> bool {
    let lower = line.trim().to_lowercase();
    COPYRIGHT_OPENINGS.iter().any(|opening| {
        lower
            .strip_prefix(opening)
            .is_some_and(|rest| rest.chars().any(char::is_alphanumeric))
    })
}

/// Reads one `Source:` record.
fn source(lines: &[String], out: &mut Provenance) {
    let text = join(lines);
    let form = "\"Source: PORT from <name>, <licence>,\" then the pinned link, the origin's copyright \
                lines and \"Changed for Spare Kit: ...\"";
    let Some(rest) = text.strip_prefix("Source: PORT from ") else {
        out.problems
            .push((Rule::Source, format!("a Source line reads {form}")));
        return;
    };
    let Some((before, link, _)) = split_at_link(rest).filter(|(_, l, _)| !l.starts_with("doi:"))
    else {
        out.problems.push((
            Rule::Source,
            format!("the Source record has no link to the file ported; it reads {form}"),
        ));
        return;
    };
    let Some((name, licence)) = name_and_licence(before) else {
        out.problems.push((
            Rule::Source,
            format!("the Source record gives no name and licence before its link; it reads {form}"),
        ));
        return;
    };
    match class_of(&licence) {
        Err(problem) => out.problems.push((Rule::Source, problem)),
        Ok(Class::Allowed) => {}
        Ok(Class::Readable) if licence == classes::PUBLIC_DOMAIN => out.problems.push((
            Rule::Source,
            format!(
                "{name} is public domain, which has no SPDX identifier, so chapter 29.2 takes it as \
                 PORT METHOD: write a Consulted record and our own MPL-2.0 code"
            ),
        )),
        Ok(class) => out.problems.push((
            Rule::Source,
            format!(
                "{licence} is not on chapter 29.2's allow-list (\"these SPDX identifiers, and no \
                 others, may appear ... in a ported file\"): code in its class ({}) is never \
                 ported",
                class.name()
            ),
        )),
    }
    let pinned = links::is_pinned(&link);
    if !pinned {
        out.problems.push((
            Rule::Source,
            format!("the link {link} of a ported file must name a full commit"),
        ));
    }
    let continuation = lines.get(1..).unwrap_or_default();
    let changed_at = continuation
        .iter()
        .position(|l| l.trim().starts_with(CHANGED));
    let before_changed = continuation
        .get(..changed_at.unwrap_or(continuation.len()))
        .unwrap_or_default();
    if !before_changed
        .iter()
        .any(|l| !l.contains(link.as_str()) && is_copyright_line(l))
    {
        out.problems.push((
            Rule::Source,
            format!(
                "the Source record for {name} copies none of the origin's copyright lines: a line \
                 before \"{CHANGED}\" that opens with \"Copyright\", \"©\", \"(c)\" or \
                 \"SPDX-FileCopyrightText:\""
            ),
        ));
    }
    let changed = changed_at
        .and_then(|at| continuation.get(at..))
        .map(join)
        .and_then(|text| text.strip_prefix(CHANGED).map(str::to_owned));
    let notice = notice_lines(lines, &link, changed_at);
    match &changed {
        None => out.problems.push((
            Rule::Source,
            format!("the Source record for {name} has no \"{CHANGED}\" line"),
        )),
        Some(what) if !states(what) => out.problems.push((
            Rule::Source,
            format!("the Source record for {name} does not say what changed for Spare Kit"),
        )),
        Some(_) => {}
    }
    out.records.push(Record {
        take: Take::Port,
        name,
        licence,
        link,
        pinned,
        purpose: changed.unwrap_or_default(),
        method_note: None,
        notice,
    });
}

/// The lines of a `Source:` record after the one holding its `link` and before the continuation
/// line at `changed_at` (`Changed for Spare Kit: `), trimmed and without blank lines: the origin's
/// notice as the record copies it.
fn notice_lines(lines: &[String], link: &str, changed_at: Option<usize>) -> Vec<String> {
    let Some(link_at) = lines.iter().position(|l| l.contains(link)) else {
        return Vec::new();
    };
    // `changed_at` counts continuation lines, which start at the record's second line.
    let end = changed_at.map_or(lines.len(), |at| at.saturating_add(1));
    lines
        .get(link_at.saturating_add(1)..end.max(link_at.saturating_add(1)))
        .unwrap_or_default()
        .iter()
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .collect()
}

/// The canonical `AND` operands of an expression, and its canonical whole.
fn operands(expr: &Expr) -> Vec<String> {
    let mut out = vec![expr.canonical()];
    if let Expr::And(terms) = expr {
        out.extend(terms.iter().map(Expr::canonical));
    }
    out
}

/// Reads a header's provenance records and checks them, with `docs`, the file's doc comments
/// outside the header, for the credit rule.
#[must_use]
pub fn read(header: &Header, docs: &[String]) -> Provenance {
    let mut out = Provenance::default();
    let blocks = header.blocks();
    let mut source_lines = false;
    for block in &blocks {
        let Block::Record(lines) = block else {
            continue;
        };
        let first = lines.first().map(String::as_str).unwrap_or_default();
        if first.starts_with("Consulted: ") {
            consulted(lines, &mut out);
        } else if first.starts_with("Source: ") {
            source_lines = true;
            source(lines, &mut out);
        } else if let Some(expr) = first.strip_prefix("SPDX-License-Identifier: ") {
            let expr = expr.trim();
            if out.spdx.is_some() {
                out.problems
                    .push((Rule::Spdx, "the SPDX line is written twice".to_owned()));
            } else if Expr::parse(expr).is_err() {
                out.problems.push((
                    Rule::Spdx,
                    format!("{expr:?} is not an SPDX license expression"),
                ));
            }
            out.spdx.get_or_insert_with(|| expr.to_owned());
        }
    }
    check_spdx(&mut out, source_lines);
    check_credits(&blocks, docs, &mut out);
    out
}

/// A ported file's licence line names its origin's licence and only allow-listed licences; a
/// file that ports nothing (no `Source:` line, even one the reader could not read) is MPL-2.0.
fn check_spdx(out: &mut Provenance, source_lines: bool) {
    let ports: Vec<String> = out
        .records
        .iter()
        .filter(|r| r.take == Take::Port)
        .map(|r| r.licence.clone())
        .collect();
    let spdx = out.spdx.as_deref().and_then(|s| Expr::parse(s).ok());
    if ports.is_empty() {
        if !source_lines
            && let Some(spdx) = spdx
            && spdx.canonical() != "MPL-2.0"
        {
            out.problems.push((
                Rule::Spdx,
                format!(
                    "the SPDX line {:?} is not MPL-2.0, but nothing here is ported: a PORT METHOD \
                     module is MPL-2.0 and ours (29.20, rule 4)",
                    spdx.canonical()
                ),
            ));
        }
        return;
    }
    let Some(spdx) = spdx else {
        out.problems.push((
            Rule::Spdx,
            "a ported file needs an \"SPDX-License-Identifier: \" line with its origin's licence"
                .to_owned(),
        ));
        return;
    };
    if classes::of_expr(&spdx) != Ok(Class::Allowed) {
        out.problems.push((
            Rule::Spdx,
            format!(
                "the SPDX line {:?} of a ported file names a licence off chapter 29.2's allow-list",
                spdx.canonical()
            ),
        ));
    }
    let allowed = operands(&spdx);
    for licence in ports {
        let canonical = Expr::parse(&licence)
            .map(|e| e.canonical())
            .unwrap_or(licence);
        if !allowed.contains(&canonical) {
            out.problems.push((
                Rule::Spdx,
                format!(
                    "the SPDX line {:?} does not carry the ported licence {canonical:?}",
                    spdx.canonical()
                ),
            ));
        }
    }
}

/// The abbreviated commit names in `text`: a word `commit` followed by 7 to 39 hexadecimal digits,
/// in backticks or not.
#[must_use]
pub fn abbreviated_commits(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    words
        .windows(2)
        .filter(|pair| {
            pair.first().is_some_and(|w| {
                w.trim_matches(|c: char| !c.is_alphanumeric())
                    .eq_ignore_ascii_case("commit")
            })
        })
        .filter_map(|pair| {
            let name = pair
                .get(1)?
                .trim_matches(|c: char| !c.is_ascii_alphanumeric());
            ((7..40).contains(&name.len()) && name.chars().all(|c| c.is_ascii_hexdigit()))
                .then(|| name.to_owned())
        })
        .collect()
}

/// Every link to outside code outside the records, and every abbreviated commit, is credited by a
/// record of the same repository or file.
fn check_credits(blocks: &[Block], docs: &[String], out: &mut Provenance) {
    // A record credits its link even when it is incomplete: that record's own problem says what
    // is missing, and reporting the link as uncredited too would say the same thing twice.
    let credited: Vec<String> = blocks
        .iter()
        .filter_map(|b| match b {
            Block::Record(lines)
                if lines
                    .first()
                    .is_some_and(|l| l.starts_with("Source: ") || l.starts_with("Consulted: ")) =>
            {
                Some(b.text())
            }
            _ => None,
        })
        .flat_map(|text| links::find(&text))
        .filter(|l| l.kind == links::LinkKind::Web)
        .map(|l| links::repository(&l.text))
        .collect();
    let texts: Vec<(String, &str)> = blocks
        .iter()
        .filter(|b| !matches!(b, Block::Record(_)))
        .map(|b| (b.text(), "the header"))
        .chain(docs.iter().map(|d| (d.clone(), "a doc comment")))
        .collect();
    let mut reported: Vec<String> = Vec::new();
    for (text, place) in &texts {
        for link in links::find(text) {
            if link.kind != links::LinkKind::Web || !links::is_code(&link.text) {
                continue;
            }
            let repo = links::repository(&link.text);
            if !credited.contains(&repo) && !reported.contains(&repo) {
                out.problems.push((
                    Rule::Uncredited,
                    format!(
                        "{place} links to source code at {} but no \"Consulted: \" or \"Source: \" \
                         record credits {repo}",
                        link.text
                    ),
                ));
                reported.push(repo);
            }
        }
        for commit in abbreviated_commits(text) {
            out.problems.push((
                Rule::Uncredited,
                format!(
                    "{place} names commit {commit} by an abbreviated name: credit what was read at \
                     it with a \"Consulted: \" or \"Source: \" record whose link names the full \
                     commit"
                ),
            ));
        }
    }
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

    fn of(text: &str) -> Header {
        let mut source = String::new();
        for l in text.lines() {
            source.push_str("//! ");
            source.push_str(l);
            source.push('\n');
        }
        Header::parse(&source)
    }

    #[test]
    fn the_examples_of_chapter_29_read() {
        let port = of("Semi-static filter generator for exact predicates.\n\
             \n\
             Source: PORT from Geogram PCK, BSD-3-Clause,\n\
             \x20 https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/bin/pck\n\
             \x20 Copyright (c) 2000-2022 Inria\n\
             \x20 Changed for Spare Kit: integer-grid inputs,\n\
             \x20 Rust output.\n\
             SPDX-License-Identifier: BSD-3-Clause");
        let p = read(&port, &[]);
        assert_eq!(p.problems, []);
        assert_eq!(p.records.len(), 1);
        assert_eq!(p.records[0].name, "Geogram PCK");
        assert_eq!(p.records[0].licence, "BSD-3-Clause");
        assert_eq!(p.records[0].purpose, "integer-grid inputs, Rust output.");
        assert!(p.records[0].pinned);
        assert_eq!(p.records[0].notice, ["Copyright (c) 2000-2022 Inria"]);

        let consulted = of("Knot removal.\n\
             \n\
             Consulted: NVIDIA USD BRep NLib, Apache-2.0, <https://github.com/NVIDIA-Omniverse/usd-brep/blob/0123456789abcdef0123456789abcdef01234567/NL_x.cpp>,\n\
             \x20 for failure cases only. No code copied.\n\
             SPDX-License-Identifier: MPL-2.0");
        let p = read(&consulted, &[]);
        assert_eq!(p.problems, []);
        assert_eq!(
            p.records[0].purpose,
            "for failure cases only. No code copied."
        );
        assert_eq!(p.records[0].take, Take::PortMethod);
        assert_eq!(p.spdx.as_deref(), Some("MPL-2.0"));
    }

    #[test]
    fn names_with_commas_public_domain_and_dois() {
        let h = of(
            "Predicates.\n\nConsulted: J. R. Shewchuk, `predicates.c`, public domain,\n\
                    \x20 <https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c>, for its signs.\n\
             Consulted: Moore, Kearfott and Cloud, MIT, doi:10.1137/1.9780898717716, for the power.",
        );
        let p = read(&h, &[]);
        assert_eq!(p.problems, []);
        assert_eq!(p.records[0].name, "J. R. Shewchuk, `predicates.c`");
        assert_eq!(p.records[0].licence, "public domain");
        assert!(!p.records[0].pinned);
        assert_eq!(p.records[1].link, "doi:10.1137/1.9780898717716");
    }

    #[test]
    fn a_method_note_is_a_repository_path_to_a_note() {
        let base = "X.\n\nConsulted: CGAL `FPU.h`, `LGPL-3.0-or-later OR LicenseRef-Commercial`,\n\
                    \x20 <https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/FPU.h>,\n\
                    \x20 for its widening.";
        let rules = |text: &str| -> Vec<Rule> {
            read(&of(text), &[])
                .problems
                .iter()
                .map(|(r, _)| *r)
                .collect()
        };
        assert_eq!(rules(base), [Rule::Consulted]);
        let noted = format!(
            "{base}\n\x20 Method note: `projects/engines/geometry/methods/cgal-fpu-widening.md`."
        );
        assert_eq!(rules(&noted), []);
        assert_eq!(
            read(&of(&noted), &[]).records[0].method_note.as_deref(),
            Some("projects/engines/geometry/methods/cgal-fpu-widening.md")
        );
        assert_eq!(
            rules(&format!("{base}\n\x20 Method note: none.")),
            [Rule::Consulted]
        );
    }

    #[test]
    fn uncredited_code_is_reported_once_per_repository() {
        let h = of(
            "Intervals.\n\nMethod: as in <https://github.com/a/b/blob/0123456789abcdef0123456789abcdef01234567/x.h> \
                    and <https://github.com/a/b/blob/0123456789abcdef0123456789abcdef01234567/y.h>, \
                    and the paper <https://doi.org/10.1/x>.",
        );
        let p = read(&h, &[]);
        let rules: Vec<Rule> = p.problems.iter().map(|(r, _)| *r).collect();
        assert_eq!(rules, [Rule::Uncredited]);
        assert!(p.problems[0].1.contains("github.com/a/b"));
        // In an item's documentation too.
        let p = read(
            &of("Sines."),
            &["As msun's <https://cgit.freebsd.org/src/tree/lib/msun/src/s_sinpi.c>.".to_owned()],
        );
        assert_eq!(p.problems.len(), 1);
        assert!(p.problems[0].1.starts_with("a doc comment links"));
    }

    #[test]
    fn abbreviated_commits_are_found() {
        assert_eq!(
            abbreviated_commits("`test.rs` at commit 7642943, read 2026-10-21; Commit `2e63bdc4`."),
            ["7642943", "2e63bdc4"]
        );
        assert_eq!(
            abbreviated_commits("commit 7642943b6c1f8a9b5e824dd1cd29b7c8e56068b8; commit message"),
            Vec::<String>::new()
        );
    }
}
