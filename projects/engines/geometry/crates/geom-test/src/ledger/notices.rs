//! The hand-kept licence texts of ported sources: one file per source under
//! `projects/engines/geometry/notices/`, matched to the ports the map records.
//!
//! What it does: [`parse`] reads one licence-text file ([`Notice`]); [`read`] reads every file of
//! the folder; [`match_ports`] groups the map's `port` sources by repository and commit and pairs
//! each group with its licence text ([`Port`]), which `NOTICE` reproduces.
//!
//! Why: a header can copy the origin's copyright lines, but not the whole licence text of every
//! origin it ports, and chapter 29.20, rule 4, asks that `geometry/NOTICE` "holds every origin
//! licence text and every credit". The text is copied exactly from the origin's licence file at
//! the pinned commit, so it is kept by hand in one place, and the check pairs it with the records
//! so neither can outlive the other.
//!
//! Guarantees:
//!
//! - **The file says what it is the licence of.** A licence-text file opens with `Key: value`
//!   lines: `Name`, `Link` (the source's repository pinned to a full commit), `Licence` (an SPDX
//!   expression on chapter 29.2's allow-list, since only allowed code is ported) and
//!   `Licence file` (the origin's licence file, pinned to the same repository and commit), and
//!   optionally `Project`; then an optional `Credit:` section and a required `Licence text:`
//!   section that runs to the end of the file. A missing, repeated, unknown or empty key, an
//!   unpinned link and a missing or empty text are problems.
//! - **One licence text per ported source, used.** Every group of `port` records with one
//!   repository and commit has exactly one licence-text file with that repository and commit, the
//!   same licence as every record of the group, and every licence-text file belongs to a group;
//!   every origin file a port names has at least one copyright line in its record, a run of links
//!   sharing the lines after it ([`crate::ledger::origins`]); anything else is a
//!   [`Rule::Notice`] problem.
//! - **Exactly as written.** The credit and the licence text are kept byte for byte, apart from
//!   `\r\n` read as `\n` and blank lines at the end of the file.
//!
//! Method: chapter 29.20, rule 4, on notices
//! (`projects/engines/geometry/architecture/29-sources-licences.md`); the Apache License 2.0,
//! section 4(a) and (d), which ask that a redistribution carries the licence and the attribution
//! notices (<https://www.apache.org/licenses/LICENSE-2.0>), as the MIT licence asks that its
//! notice be included.
//!
//! Cost: linear in the size of the files, and in the number of ports times the number of files to
//! match them.

use std::collections::BTreeMap;

use crate::ledger::{
    NOTICES_DIR, Problem, Rule, Source, Use, canonical, origins, repository_and_commit,
};
use crate::map::SourceFile;
use crate::map::classes::{self, Class};
use crate::map::header::states;
use crate::map::links;

/// The keys of a licence-text file, in the order they are written; `Project` is optional.
pub const KEYS: [&str; 5] = ["Name", "Project", "Link", "Licence", "Licence file"];

/// The line that opens the optional credit section.
pub const CREDIT: &str = "Credit:";

/// The line that opens the licence text, which runs to the end of the file.
pub const LICENCE_TEXT: &str = "Licence text:";

/// The folder's description, which is not a licence text.
pub const README: &str = "README.md";

/// One hand-kept licence text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// The file, relative to the workspace root.
    pub path: String,
    /// The source's name.
    pub name: String,
    /// The source's home page, if given.
    pub project: Option<String>,
    /// The source's repository, pinned to a full commit.
    pub link: String,
    /// The repository and commit the link names.
    pub repository: String,
    /// The commit.
    pub commit: String,
    /// The licence, as an SPDX expression.
    pub licence: String,
    /// The origin's licence file, pinned to the same commit.
    pub licence_file: String,
    /// Further credit, line by line, as written.
    pub credit: Vec<String>,
    /// The licence text, exactly as written, ending in one newline.
    pub text: String,
}

/// Reads one licence-text file; every problem is returned instead.
///
/// # Errors
///
/// Every problem with the file, as messages.
pub fn parse(path: &str, text: &str) -> Result<Notice, Vec<String>> {
    let text = text
        .strip_prefix('\u{feff}')
        .unwrap_or(text)
        .replace("\r\n", "\n");
    let mut problems = Vec::new();
    let mut lines = text.lines();
    let mut keys: BTreeMap<&str, String> = BTreeMap::new();
    for line in lines.by_ref() {
        if line.trim().is_empty() {
            break;
        }
        let Some((key, value)) = line.split_once(": ") else {
            problems.push(format!(
                "{line:?} is not a \"Key: value\" line; the keys are {}, then a blank line",
                KEYS.join(", ")
            ));
            continue;
        };
        let Some(&known) = KEYS.iter().find(|k| **k == key) else {
            problems.push(format!(
                "{key:?} is not a key of a licence-text file ({})",
                KEYS.join(", ")
            ));
            continue;
        };
        if keys.insert(known, value.trim().to_owned()).is_some() {
            problems.push(format!("the key {known:?} is written twice"));
        }
    }
    let mut field = |key: &str| -> String {
        let value = keys.get(key).cloned().unwrap_or_default();
        if !states(&value) {
            problems.push(format!("the key {key:?} is missing or states nothing"));
        }
        value
    };
    let name = field("Name");
    let link = field("Link");
    let licence = field("Licence");
    let licence_file = field("Licence file");
    let project = keys.get("Project").cloned();

    let rest: Vec<&str> = lines.collect();
    let text_at = rest.iter().position(|l| l.trim_end() == LICENCE_TEXT);
    let credit_lines = rest.get(..text_at.unwrap_or(rest.len())).unwrap_or_default();
    let credit = match credit_lines.iter().position(|l| !l.trim().is_empty()) {
        None => Vec::new(),
        Some(first) if credit_lines.get(first).is_some_and(|l| l.trim_end() == CREDIT) => {
            trim_blank(credit_lines.get(first.saturating_add(1)..).unwrap_or_default())
        }
        Some(first) => {
            problems.push(format!(
                "after the keys and a blank line comes \"{CREDIT}\" or \"{LICENCE_TEXT}\", not {:?}",
                credit_lines.get(first).copied().unwrap_or_default()
            ));
            Vec::new()
        }
    };
    let licence_text = match text_at {
        None => {
            problems.push(format!(
                "there is no \"{LICENCE_TEXT}\" line, followed by the origin's licence text \
                 copied exactly from its licence file at the pinned commit"
            ));
            String::new()
        }
        Some(at) => {
            let body = trim_blank(rest.get(at.saturating_add(1)..).unwrap_or_default());
            if !body.iter().any(|l| states(l)) {
                problems.push(format!("the text after \"{LICENCE_TEXT}\" is empty"));
            }
            let mut joined = body.join("\n");
            joined.push('\n');
            joined
        }
    };

    if !licence.is_empty() && classes::of(&licence) != Ok(Class::Allowed) {
        problems.push(format!(
            "the licence {licence:?} is not an SPDX expression on chapter 29.2's allow-list, and \
             only allowed code is ported"
        ));
    }
    let pinned = repository_and_commit(&link).filter(|(_, c)| links::is_full_commit(c));
    if !link.is_empty() && pinned.is_none() {
        problems.push(format!(
            "the link {link:?} does not name a repository pinned to a full commit"
        ));
    }
    let (repository, commit) = pinned.unwrap_or_default();
    if !licence_file.is_empty()
        && repository_and_commit(&licence_file) != Some((repository.clone(), commit.clone()))
    {
        problems.push(format!(
            "the licence file {licence_file:?} is not in the linked repository at the linked commit"
        ));
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(Notice {
        path: path.to_owned(),
        name,
        project,
        link,
        repository,
        commit,
        licence,
        licence_file,
        credit,
        text: licence_text,
    })
}

/// `lines` without blank lines at the start and the end, as owned strings.
fn trim_blank(lines: &[&str]) -> Vec<String> {
    let start = lines
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(start, |e| e.saturating_add(1));
    lines
        .get(start..end.max(start))
        .unwrap_or_default()
        .iter()
        .map(|l| l.trim_end().to_owned())
        .collect()
}

/// Reads every file of the folder: each `.txt` file is a licence text; `README.md` describes the
/// folder; anything else is a problem.
#[must_use]
pub fn read(files: &[SourceFile], problems: &mut Vec<Problem>) -> Vec<Notice> {
    let mut out = Vec::new();
    for file in files {
        let problem = |message: String| Problem {
            file: file.path.clone(),
            rule: Rule::Notice,
            message,
        };
        let name = file.path.rsplit('/').next().unwrap_or_default();
        if name == README {
            continue;
        }
        if !std::path::Path::new(name)
            .extension()
            .is_some_and(|e| e == "txt")
        {
            problems.push(problem(format!(
                "{NOTICES_DIR} holds only licence texts (`<source>.txt`) and its {README}"
            )));
            continue;
        }
        match &file.text {
            Err(why) => problems.push(problem(why.clone())),
            Ok(text) => match parse(&file.path, text) {
                Ok(notice) => out.push(notice),
                Err(found) => problems.extend(found.into_iter().map(problem)),
            },
        }
    }
    out
}

/// One ported source: every file's port record with an origin file in one repository at one
/// commit, and that source's licence text.
#[derive(Clone, Debug)]
pub struct Port<'a, 'n> {
    /// The repository.
    pub repository: String,
    /// The commit.
    pub commit: String,
    /// Every file's port record with an origin here, in file order.
    pub uses: Vec<Use<'a>>,
    /// Its licence text, when exactly one file holds it.
    pub notice: Option<&'n Notice>,
}

/// Groups the files' `port` records by the repository and commit of each origin file they name
/// (the record's link and every further link its notice copies), pairs each group with its
/// licence text, and reports every origin link that is not pinned, every group without exactly
/// one licence text, every licence that differs, and every licence text no group uses.
#[must_use]
pub fn match_ports<'a, 'n>(
    sources: &[Source<'a>],
    notices: &'n [Notice],
    problems: &mut Vec<Problem>,
) -> Vec<Port<'a, 'n>> {
    let mut groups: BTreeMap<(String, String), Vec<Use<'a>>> = BTreeMap::new();
    for source in sources.iter().filter(|s| s.take() == "port") {
        for used in &source.uses {
            for (link, lines) in origins(used.record) {
                if lines.is_empty() {
                    problems.push(Problem {
                        file: used.file.to_owned(),
                        rule: Rule::Notice,
                        message: format!(
                            "the Source record of this port copies no copyright line for the \
                             origin file {link}: write the origin's copyright lines, copied \
                             exactly, after its link (29.20, rule 4), or, where the origin file \
                             has none, a line saying so"
                        ),
                    });
                }
                let key = repository_and_commit(&link).filter(|(_, c)| links::is_full_commit(c));
                let Some(key) = key else {
                    // The record's own link is the map's Source rule; a further one is ours.
                    if link != links::source_key(used.record.text("link")) {
                        problems.push(Problem {
                            file: used.file.to_owned(),
                            rule: Rule::Notice,
                            message: format!(
                                "the origin file {link} that the Source record of this port names \
                                 is not in a repository pinned to a full commit, so its licence \
                                 text cannot be found: link it pinned"
                            ),
                        });
                    }
                    continue;
                };
                let group = groups.entry(key).or_default();
                if !group
                    .iter()
                    .any(|u| u.file == used.file && std::ptr::eq(u.record, used.record))
                {
                    group.push(*used);
                }
            }
        }
    }
    let mut used_notice = vec![false; notices.len()];
    let mut ports = Vec::new();
    for ((repository, commit), mut group) in groups {
        group.sort_by(|a, b| a.file.cmp(b.file));
        let matching: Vec<usize> = notices
            .iter()
            .enumerate()
            .filter(|(_, n)| n.repository == repository && n.commit == commit)
            .map(|(k, _)| k)
            .collect();
        for k in &matching {
            if let Some(flag) = used_notice.get_mut(*k) {
                *flag = true;
            }
        }
        let files: Vec<&str> = group.iter().map(|u| u.file).collect();
        let notice = match matching.as_slice() {
            [] => {
                problems.push(Problem {
                    file: files.first().copied().unwrap_or(NOTICES_DIR).to_owned(),
                    rule: Rule::Notice,
                    message: format!(
                        "the port of {repository} at commit {commit} ({}) has no licence text: add \
                         {NOTICES_DIR}<source>.txt with its Name, Link, Licence, Licence file and \
                         the licence text copied exactly from the origin's licence file \
                         ({NOTICES_DIR}{README})",
                        files.join(", ")
                    ),
                });
                None
            }
            [one] => notices.get(*one),
            more => {
                for k in more {
                    if let Some(n) = notices.get(*k) {
                        problems.push(Problem {
                            file: n.path.clone(),
                            rule: Rule::Notice,
                            message: format!(
                                "{} licence texts name {repository} at commit {commit}; keep one",
                                more.len()
                            ),
                        });
                    }
                }
                None
            }
        };
        if let Some(n) = notice {
            for used in &group {
                let licence = used.record.text("licence");
                if canonical(licence) != canonical(&n.licence) {
                    problems.push(Problem {
                        file: n.path.clone(),
                        rule: Rule::Notice,
                        message: format!(
                            "its licence is {:?}, but the port in {} states {licence:?}; 29.20 \
                             rule 1: where they disagree, stop and ask an owner",
                            n.licence, used.file
                        ),
                    });
                }
            }
        }
        ports.push(Port {
            repository,
            commit,
            uses: group,
            notice,
        });
    }
    for (notice, was_used) in notices.iter().zip(&used_notice) {
        if !was_used {
            problems.push(Problem {
                file: notice.path.clone(),
                rule: Rule::Notice,
                message: format!(
                    "no Source: record ports {} at commit {}, so this licence text is not owed: \
                     delete it",
                    notice.repository, notice.commit
                ),
            });
        }
    }
    ports.sort_by(|a, b| {
        let name = |p: &Port<'_, '_>| p.notice.map(|n| n.name.to_lowercase());
        name(a)
            .cmp(&name(b))
            .then_with(|| a.repository.cmp(&b.repository))
            .then_with(|| a.commit.cmp(&b.commit))
    });
    ports
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

    const SHA: &str = "0ce0cbc15935e24c656280e5a46c34e89f301b7a";

    fn good() -> String {
        format!(
            "Name: Example\nProject: https://example.org/\n\
             Link: https://github.com/ex/ample/tree/{SHA}\nLicence: MIT\n\
             Licence file: https://github.com/ex/ample/blob/{SHA}/LICENSE\n\n\
             Credit:\nThanks to A.\n\nLicence text:\n\nPermission is granted.\r\n\n  Indented.\n\n\n"
        )
    }

    #[test]
    fn reads_a_licence_text_exactly() {
        let n = parse("n/x.txt", &good()).unwrap();
        assert_eq!(n.name, "Example");
        assert_eq!(n.project.as_deref(), Some("https://example.org/"));
        assert_eq!(n.repository, "github.com/ex/ample");
        assert_eq!(n.commit, SHA);
        assert_eq!(n.credit, ["Thanks to A."]);
        assert_eq!(n.text, "Permission is granted.\n\n  Indented.\n");
    }

    #[test]
    fn every_key_and_section_is_checked() {
        let fails = |text: String, needle: &str| {
            let found = parse("n/x.txt", &text).unwrap_err();
            assert!(
                found.iter().any(|p| p.contains(needle)),
                "{needle:?} not in {found:#?}"
            );
        };
        fails(good().replace("Name: Example\n", ""), "\"Name\" is missing");
        fails(good().replace("Name: Example\n", "Name: TBD\n"), "\"Name\" is missing");
        fails(good().replace("Name: Example\n", "Name: A\nName: B\n"), "twice");
        fails(good().replace("Name: Example\n", "Name: A\nAuthor: B\n"), "not a key");
        fails(good().replace("Name: Example\n", "Name Example\n"), "not a \"Key: value\"");
        fails(good().replace("Licence: MIT", "Licence: GPL-3.0-only"), "allow-list");
        fails(good().replace(&format!("tree/{SHA}"), "tree/main"), "pinned");
        fails(
            good().replace(&format!("blob/{SHA}/LICENSE"), "blob/main/LICENSE"),
            "licence file",
        );
        fails(good().replace("Licence text:", "Text:"), "no \"Licence text:\"");
        fails(
            good().replace("Permission is granted.\r\n\n  Indented.", ""),
            "is empty",
        );
        fails(good().replace("Credit:\n", "Thanks:\n"), "not \"Thanks:\"");
    }
}
