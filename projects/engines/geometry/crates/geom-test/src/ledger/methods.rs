//! The method notes of the method-only procedure: every note under
//! `projects/engines/geometry/methods/` read, held to the template, and matched to the records that
//! name it.
//!
//! What it does: [`parse`] reads one note ([`Note`]) and reports where it breaks the template;
//! [`read`] reads every file of the folder, holding `TEMPLATE.md` to the same fields and sections;
//! [`check_records`] compares each note with the `Consulted:` records whose `Method note:` line
//! names it.
//!
//! Why: chapter 29.20, rule 6, has one session read a copyleft source and write a note in plain
//! words, a second reviewer check it, and a different session write the code from it. The note is
//! the procedure's only lasting record, so every note must carry the same fields (who read what,
//! at which commit, who reviewed it, what code came from it) and hold no code, and the module that
//! cites it must credit the source the note describes.
//!
//! Guarantees:
//!
//! - **The template's fields, once each.** A note opens with `# Method note: <title>`, and its
//!   table has one row for each of [`FIELDS`], each stating something and none left as the
//!   template's `<placeholder>`. `TEMPLATE.md` must have every field and section too, with
//!   placeholders, so the template and the check cannot drift apart.
//! - **A copyleft source, pinned.** The licence is one chapter 29.2 classifies as method only
//!   (LGPL) or owner only (GPL), the classes rule 6 serves, and the pinned link names a full
//!   commit.
//! - **The sections, in order, each with content.** [`SECTIONS`] appear as `## ` headings in that
//!   order, each followed by text; other sections may come between or after them.
//! - **No code** (rule 6, step 3). A line that opens a fenced code block (three backticks or
//!   tildes, however indented), an indented code block (four spaces or a tab after a blank line)
//!   or an HTML `pre` or `code` element is a problem. Inline backticks, for the public names a
//!   note may use, are not.
//! - **The record credits the source the note describes.** A `Consulted:` record naming the note
//!   gives the note's licence and links into the note's repository (or, off a code-hosting site,
//!   to the same file); otherwise the note has a problem, and stop and ask an owner (rule 1).
//!
//! Method: chapter 29.20, rule 6, the method-only procedure
//! (`projects/engines/geometry/architecture/29-sources-licences.md`); headings, fences and
//! indented code blocks as CommonMark 0.31.2 defines them, sections 4.2, 4.4 and 4.5
//! (<https://spec.commonmark.org/0.31.2/>), and tables as GitHub Flavored Markdown's table
//! extension (<https://github.github.com/gfm/#tables-extension->).
//!
//! Cost: linear in the size of the notes, and in the number of records times the number of notes.

use std::collections::BTreeMap;

use crate::ledger::{Problem, Rule, Source, canonical};
use crate::map::classes::{self, Class};
use crate::map::header::states;
use crate::map::links::{self, LinkKind};
use crate::map::provenance::METHODS_FOLDER;
use crate::map::SourceFile;

/// A note's title line opens with this.
pub const TITLE: &str = "# Method note: ";

/// The fields of a note's table, in the template's order.
pub const FIELDS: [&str; 9] = [
    "Source",
    "Licence",
    "Ledger row",
    "Pinned link",
    "Files read",
    "Read by",
    "Written by",
    "Second review",
    "Code written from it",
];

/// The sections a note must have, in order.
pub const SECTIONS: [&str; 5] = [
    "The problem",
    "The method",
    "Invariants",
    "Failure cases",
    "Published sources",
];

/// The template every note is written from.
pub const TEMPLATE: &str = "TEMPLATE.md";

/// The folder's description, which is not a note.
pub const README: &str = "README.md";

/// One method note, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// The file, relative to the workspace root.
    pub path: String,
    /// The title after `# Method note: `.
    pub title: String,
    /// Each field's value, by field.
    pub fields: BTreeMap<String, String>,
    /// The `## ` sections in order, each with whether it has content.
    pub sections: Vec<(String, bool)>,
}

impl Note {
    /// A field's value, or empty.
    #[must_use]
    pub fn field(&self, name: &str) -> &str {
        self.fields.get(name).map_or("", String::as_str)
    }

    /// The pinned link: the first web address in the `Pinned link` field.
    #[must_use]
    pub fn pinned_link(&self) -> Option<String> {
        links::find(self.field("Pinned link"))
            .into_iter()
            .find(|l| l.kind == LinkKind::Web)
            .map(|l| l.text)
    }
}

/// The cells of a table row, without its outer pipes; `None` for a line that is not a row.
fn cells(line: &str) -> Option<Vec<String>> {
    let inner = line.trim().strip_prefix('|')?.strip_suffix('|')?;
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut escaped = false;
    for c in inner.chars() {
        match c {
            '|' if !escaped => out.push(core::mem::take(&mut cell).trim().to_owned()),
            _ => cell.push(c),
        }
        escaped = c == '\\' && !escaped;
    }
    out.push(cell.trim().to_owned());
    Some(out)
}

/// Whether `line` opens or closes a fenced code block, however far it is indented (an indented
/// fence is still code), or opens an HTML code element.
fn holds_code_markup(line: &str) -> bool {
    let rest = line.trim_start();
    let lower = line.to_lowercase();
    rest.starts_with("```")
        || rest.starts_with("~~~")
        || lower.contains("<pre")
        || lower.contains("<code")
}

/// Reads one note, holding it to the template; `template` reads `TEMPLATE.md`, whose values must
/// be placeholders.
#[must_use]
pub fn parse(path: &str, text: &str, template: bool) -> (Note, Vec<String>) {
    let text = text
        .strip_prefix('\u{feff}')
        .unwrap_or(text)
        .replace("\r\n", "\n");
    let mut problems = Vec::new();
    let title = text
        .lines()
        .next()
        .and_then(|l| l.strip_prefix(TITLE))
        .map(str::trim)
        .unwrap_or_default()
        .to_owned();
    if !states(&title) {
        problems.push(format!("the note does not open with \"{TITLE}<title>\""));
    }
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut sections: Vec<(String, bool)> = Vec::new();
    let mut previous_blank = false;
    for (number, line) in text.lines().enumerate().skip(1) {
        let line_number = number.saturating_add(1);
        if holds_code_markup(line) {
            problems.push(format!(
                "line {line_number} opens a fenced code block or an HTML code element; a method \
                 note holds no code (29.20, rule 6, step 3)"
            ));
        } else if previous_blank && (line.starts_with("    ") || line.starts_with('\t')) {
            problems.push(format!(
                "line {line_number} is an indented code block; a method note holds no code \
                 (29.20, rule 6, step 3)"
            ));
        }
        previous_blank = line.trim().is_empty();
        if let Some(heading) = line.strip_prefix("## ") {
            sections.push((heading.trim().to_owned(), false));
            continue;
        }
        if let Some(last) = sections.last_mut()
            && !line.trim().is_empty()
        {
            last.1 = true;
        }
        if sections.is_empty()
            && let Some(row) = cells(line)
            && let [name, value] = row.as_slice()
            && FIELDS.contains(&name.as_str())
            && fields.insert(name.clone(), value.clone()).is_some()
        {
            problems.push(format!("the field {name:?} is written twice"));
        }
    }
    for name in FIELDS {
        let value = fields.get(name).map_or("", String::as_str);
        let placeholder = value.starts_with('<');
        if template {
            if !placeholder {
                problems.push(format!(
                    "the template's field {name:?} must hold a <placeholder> saying what to write"
                ));
            }
        } else if !states(value) || placeholder {
            problems.push(format!(
                "the field {name:?} is missing, states nothing or is still the template's \
                 placeholder"
            ));
        }
    }
    let mut at = 0_usize;
    for wanted in SECTIONS {
        let found = sections
            .iter()
            .enumerate()
            .skip(at)
            .find(|(_, (name, _))| name == wanted);
        match found {
            None => problems.push(format!(
                "the section \"## {wanted}\" is missing or out of order (the sections are {})",
                SECTIONS.join(", ")
            )),
            Some((k, (_, content))) => {
                if !content {
                    problems.push(format!("the section \"## {wanted}\" is empty"));
                }
                at = k.saturating_add(1);
            }
        }
    }
    let note = Note {
        path: path.to_owned(),
        title,
        fields,
        sections,
    };
    if !template {
        check_source(&note, &mut problems);
    }
    (note, problems)
}

/// A note's licence is method only or owner only, and its pinned link names a full commit.
fn check_source(note: &Note, problems: &mut Vec<String>) {
    let licence = note.field("Licence");
    match classes::of(licence) {
        Ok(Class::MethodOnly | Class::OwnerOnly) => {}
        Ok(class) => problems.push(format!(
            "the licence {licence:?} is in chapter 29.2's class \"{}\"; a method note is the \
             method-only procedure's record, for LGPL code and GPL code an owner allowed (29.20, \
             rule 6)",
            class.name()
        )),
        Err(_) if licence.is_empty() => {}
        Err(_) => problems.push(format!(
            "the licence {licence:?} is not an SPDX expression chapter 29.2 classifies"
        )),
    }
    match note.pinned_link() {
        Some(link) if links::is_pinned(&link) => {}
        Some(link) => problems.push(format!(
            "the pinned link {link} does not name a full commit"
        )),
        None if note.field("Pinned link").is_empty() => {}
        None => problems.push("the pinned link field holds no web address".to_owned()),
    }
}

/// Reads every file of the folder: each `.md` file is a note, apart from the folder's description
/// and the template, which is held to the same fields and sections; anything else is a problem.
#[must_use]
pub fn read(files: &[SourceFile], problems: &mut Vec<Problem>) -> Vec<Note> {
    let mut notes = Vec::new();
    let mut template_seen = false;
    for file in files {
        let name = file.path.rsplit('/').next().unwrap_or_default();
        let mut problem = |message: String| {
            problems.push(Problem {
                file: file.path.clone(),
                rule: Rule::Note,
                message,
            });
        };
        if name == README {
            continue;
        }
        if !std::path::Path::new(name)
            .extension()
            .is_some_and(|e| e == "md")
        {
            problem(format!(
                "{METHODS_FOLDER} holds only method notes (`<note>.md`), its {TEMPLATE} and its \
                 {README}"
            ));
            continue;
        }
        let text = match &file.text {
            Ok(text) => text,
            Err(why) => {
                problem(why.clone());
                continue;
            }
        };
        let template = name == TEMPLATE;
        template_seen |= template;
        let (note, found) = parse(&file.path, text, template);
        for message in found {
            problem(message);
        }
        if !template {
            notes.push(note);
        }
    }
    if !files.is_empty() && !template_seen {
        problems.push(Problem {
            file: METHODS_FOLDER.to_owned(),
            rule: Rule::Note,
            message: format!("the folder has notes but no {TEMPLATE} to write them from"),
        });
    }
    notes
}

/// Each note named by a `Consulted:` record credits the same licence and repository as the
/// record; a note no file exists for is the map's problem (its `Consulted` rule), not repeated.
pub fn check_records(sources: &[Source<'_>], notes: &[Note], problems: &mut Vec<Problem>) {
    for source in sources {
        for record in source.uses.iter().map(|u| u.record) {
            let Some(path) = record.get("method_note").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(note) = notes.iter().find(|n| n.path == path) else {
                continue;
            };
            let licence = record.text("licence");
            let name = record.text("name");
            if canonical(licence) != canonical(note.field("Licence")) {
                problems.push(Problem {
                    file: note.path.clone(),
                    rule: Rule::Note,
                    message: format!(
                        "the record for {name} names this note but credits {licence:?}, not the \
                         note's {:?}; 29.20 rule 1: where they disagree, stop and ask an owner",
                        note.field("Licence")
                    ),
                });
            }
            let record_repo = links::repository(record.text("link"));
            let note_repo = note.pinned_link().map(|l| links::repository(&l));
            if note_repo.as_deref() != Some(record_repo.as_str()) {
                problems.push(Problem {
                    file: note.path.clone(),
                    rule: Rule::Note,
                    message: format!(
                        "the record for {name} names this note but links to {record_repo}, which \
                         is not the repository of the note's pinned link"
                    ),
                });
            }
        }
    }
}

/// The records naming each note, by note path: each as `<file>` of the record.
#[must_use]
pub fn named_by<'a>(sources: &'a [Source<'a>]) -> BTreeMap<&'a str, Vec<&'a str>> {
    let mut out: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for source in sources {
        for used in &source.uses {
            if let Some(path) = used.record.get("method_note").and_then(|v| v.as_str()) {
                out.entry(path).or_default().push(used.file);
            }
        }
    }
    for files in out.values_mut() {
        files.sort_unstable();
        files.dedup();
    }
    out
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

    #[test]
    fn table_cells_keep_escaped_pipes() {
        assert_eq!(
            cells("| Source | a \\| b |"),
            Some(vec!["Source".to_owned(), "a \\| b".to_owned()])
        );
        assert_eq!(cells("not a row"), None);
    }

    #[test]
    fn fences_and_html_code_are_code_however_indented() {
        assert!(holds_code_markup("```"));
        assert!(holds_code_markup("   ~~~rust"));
        assert!(holds_code_markup("    ```"));
        assert!(holds_code_markup("\t```"));
        assert!(holds_code_markup("<PRE>x</PRE>"));
        assert!(holds_code_markup("a <code>f()</code>"));
        assert!(!holds_code_markup("a ``` inside"));
        assert!(!holds_code_markup("`inline` names are fine"));
    }
}
