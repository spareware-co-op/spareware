//! Reading a file's `//!` header: its blocks, and the crate and module fields of its grammar.
//!
//! What it does: [`Header::parse`] takes the `//!` lines that open a file. [`Header::blocks`]
//! splits them into paragraphs, lists and provenance records. [`crate_fields`] reads a crate
//! header's summary, job, layer, invariants, sections, family and status; [`module_fields`] reads a
//! module header's summary, what it does, guarantees, method, cost and family. Each returns the
//! fields it found and a problem for every required field that is missing, malformed or a
//! placeholder ([`PLACEHOLDERS`]).
//!
//! Guarantees:
//!
//! - **The written grammar, exactly.** Every label, form and rule here is the one
//!   `projects/engines/geometry/HEADERS.md` states, and its seeded fixtures (`tests/map_seeded.rs`)
//!   hold each one. A field counts only when written in that grammar, with one of its closed set of
//!   labels, so a header cannot pass by mentioning a word in passing or by a label that denies the
//!   field (`What this module never guarantees:`).
//! - **G09's four crate fields, found where G09 finds them.** Job, Layer, Invariants and Sections
//!   are located as the scaffold check in `engine-geometry` (`tests/scaffold/header.rs`) locates
//!   them: paragraphs are runs of non-blank lines, sentences open a paragraph or follow ". ", the
//!   same labels count, the layer is the first `Layer` sentence whose word is `Development` or a
//!   number, and the invariants follow the first line that opens a paragraph with `Invariants`.
//!   So a crate header states a field for one check exactly when it states it for the other (one
//!   difference: this reader skips a leading byte-order mark, which G09's does not). This
//!   reader then checks what the fields hold, which G09's leaves to its crate table: a job and
//!   every invariant state something, and the layer is in range and fits the crate's kind.
//! - **Deterministic and total.** No input panics; every field is the first one written, in
//!   header order, and a module field written twice is a problem rather than a silent choice.
//!
//! Method: line classification and block grouping as in `CommonMark`'s two-phase parsing strategy
//! (block structure first, inline text after, <https://spec.commonmark.org/0.31.2/#appendix-a-parsing-strategy>),
//! restricted to what headers use: paragraphs, `- ` list items with indented continuation lines,
//! fenced code blocks, and the line-level provenance records of chapter 29.20
//! (`projects/engines/geometry/architecture/29-sources-licences.md`). Sentence splitting and the
//! crate fields' positions follow G09's reader so the two checks agree.
//!
//! Cost: linear in the length of the header for each field read.

use crate::map::Rule;
use crate::map::links;

/// The labels that open a sentence stating a crate's job (G09).
pub const JOB_LABELS: [&str; 4] = ["One job: ", "Job: ", "Job (", "Its one job is "];

/// The labels that open a sentence naming the sections a crate implements (G09).
pub const SECTIONS_LABELS: [&str; 2] = ["Sections: ", "Map: "];

/// The highest chapter number of the kernel architecture (32, the owner's decisions).
pub const LAST_CHAPTER: u8 = 32;

/// The highest layer of chapter 27's layer table (13, the facade).
pub const LAST_LAYER: u8 = 13;

/// The families of chapter 10.2's operation catalogue, as written there.
pub const FAMILIES: [&str; 15] = [
    "Make",
    "2D",
    "Sketch",
    "Sweep",
    "Combine",
    "Blend",
    "Offset",
    "Transform",
    "Repair",
    "Query",
    "Draw",
    "Mesh",
    "Manufacture",
    "Free-form",
    "Ground",
];

/// The crates whose job, in chapter 27's crate table, is a family of chapter 10.2's catalogue, and
/// the families each must state: each row's job names the operations of those families' rows.
pub const FAMILY_CRATES: [(&str, &[&str]); 21] = [
    ("geom-2d", &["2D"]),
    ("geom-align", &["Ground"]),
    ("geom-analysis", &["Query"]),
    ("geom-blend", &["Blend"]),
    ("geom-bool", &["Combine"]),
    ("geom-combine", &["Combine"]),
    ("geom-convert", &["Free-form"]),
    ("geom-draw", &["Draw"]),
    ("geom-field", &["Free-form"]),
    ("geom-heal", &["Repair"]),
    ("geom-make", &["Make", "Transform"]),
    ("geom-mesh", &["Mesh"]),
    ("geom-offset", &["Offset"]),
    ("geom-panel", &["Free-form"]),
    ("geom-profile", &["Sketch"]),
    ("geom-skeleton", &["Ground"]),
    ("geom-sketch", &["Sketch"]),
    ("geom-spatial", &["Query"]),
    ("geom-subd", &["Free-form"]),
    ("geom-sweep", &["Sweep", "Manufacture"]),
    ("geom-terrain", &["Ground"]),
];

/// The prefixes of the line-level provenance records of chapter 29.20.
pub const RECORD_PREFIXES: [&str; 3] = ["Source: ", "Consulted: ", "SPDX-License-Identifier: "];

/// How a kernel crate's summary opens.
pub const KERNEL_SUMMARY: &str = "Spare Kit geometry kernel: ";

/// How a development crate's summary opens.
pub const DEVELOPMENT_SUMMARY: &str = "Spare Kit geometry kernel, development only: ";

/// How the facade's and its binding crates' summaries open, as every shared engine's does.
pub const FACADE_SUMMARY: &str = "Shared engine: ";

/// The subjects of a `What <subject> guarantees` label.
pub const GUARANTEES_SUBJECTS: [&str; 5] = [
    "it",
    "this module",
    "this crate",
    "every function here",
    "each function here",
];

/// The labels of an `Invariants` paragraph read as a module's guarantees.
pub const INVARIANTS_LABELS: [&str; 4] = [
    "Invariants",
    "Invariants it keeps",
    "Invariants this module keeps",
    "Invariants this crate keeps",
];

/// The labels of the method field; `Method (<qualifier>)` is the fifth form.
pub const METHOD_LABELS: [&str; 4] = ["Method", "Methods", "Method followed", "Methods followed"];

/// Field text that states nothing: a field written only as one of these (in any case, without
/// its full stop) is missing.
pub const PLACEHOLDERS: [&str; 14] = [
    "none",
    "nothing",
    "n/a",
    "na",
    "tbd",
    "tba",
    "tk",
    "todo",
    "fixme",
    "xxx",
    "unknown",
    "same",
    "see above",
    "as above",
];

/// The problem of a crate header that states no layer.
pub const NO_LAYER: &str = "no sentence opens with \"Layer: \" and the crate's layer: a number \
                            from 1 to 13 of chapter 27's layer table, or Development";

/// What kind of crate a header belongs to, which decides its summary's form and its layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CrateKind {
    /// A crate under `projects/engines/geometry/crates` that is not a development crate.
    Kernel,
    /// `geom-test`, `geom-fuzz`, `geom-gen` or `geom-bench`.
    Development,
    /// The facade `engine-geometry` and its binding crates under `projects/engines/crates`
    /// (layer 13).
    Facade,
}

impl CrateKind {
    /// The kind's name in the map.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            CrateKind::Kernel => "kernel",
            CrateKind::Development => "development",
            CrateKind::Facade => "facade",
        }
    }

    /// How the kind's summary opens.
    #[must_use]
    pub fn summary_prefix(self) -> &'static str {
        match self {
            CrateKind::Kernel => KERNEL_SUMMARY,
            CrateKind::Development => DEVELOPMENT_SUMMARY,
            CrateKind::Facade => FACADE_SUMMARY,
        }
    }
}

/// The leading `//!` block of a file, one entry per line, without the `//!` and the one space
/// after it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// The lines of the block, in order.
    pub lines: Vec<String>,
}

/// A block of a header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    /// A run of lines that are not list items or records: prose, or a fenced code block.
    Paragraph(Vec<String>),
    /// A list: each item's lines, the first without its `- `.
    List(Vec<Vec<String>>),
    /// A provenance record: a `Source: `, `Consulted: ` or `SPDX-License-Identifier: ` line and
    /// its indented continuation lines.
    Record(Vec<String>),
}

/// Lines trimmed and joined with single spaces.
#[must_use]
pub fn join(lines: &[String]) -> String {
    lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

impl Block {
    /// The block's text: a paragraph's or record's lines joined with spaces; a list's items joined
    /// with spaces.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Block::Paragraph(lines) | Block::Record(lines) => join(lines),
            Block::List(items) => items.iter().map(|i| join(i)).collect::<Vec<_>>().join(" "),
        }
    }

    /// Each list item's text; empty for other blocks.
    #[must_use]
    pub fn items(&self) -> Vec<String> {
        match self {
            Block::List(items) => items.iter().map(|i| join(i)).collect(),
            _ => Vec::new(),
        }
    }
}

/// What the block parser is building.
enum Open {
    None,
    Paragraph(Vec<String>),
    List(Vec<Vec<String>>),
    Record(Vec<String>),
}

impl Open {
    fn close(self, blocks: &mut Vec<Block>) {
        match self {
            Open::None => {}
            Open::Paragraph(lines) => blocks.push(Block::Paragraph(lines)),
            Open::List(items) => blocks.push(Block::List(items)),
            Open::Record(lines) => blocks.push(Block::Record(lines)),
        }
    }
}

fn indented(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

fn is_record(line: &str) -> bool {
    RECORD_PREFIXES.iter().any(|p| line.starts_with(p))
}

/// Adds `line` to the last item of a list.
fn push_to_item(items: &mut [Vec<String>], line: &str) {
    if let Some(last) = items.last_mut() {
        last.push(line.to_owned());
    }
}

impl Header {
    /// Reads the block of `//!` lines that opens `source` (a leading byte-order mark is skipped).
    /// The first line that does not start with `//!` ends the block, so the header must come
    /// first.
    #[must_use]
    pub fn parse(source: &str) -> Header {
        let source = source.strip_prefix('\u{feff}').unwrap_or(source);
        let mut lines = Vec::new();
        for line in source.lines() {
            let Some(rest) = line.trim_end().strip_prefix("//!") else {
                break;
            };
            lines.push(rest.strip_prefix(' ').unwrap_or(rest).to_owned());
        }
        Header { lines }
    }

    /// Whether the file has no header at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.trim().is_empty())
    }

    /// The header's blocks, in order (`HEADERS.md`, "Reading a header").
    #[must_use]
    pub fn blocks(&self) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut open = Open::None;
        let mut after_blank = false;
        let mut in_fence = false;
        for line in &self.lines {
            let line = line.as_str();
            if in_fence {
                match &mut open {
                    Open::Paragraph(lines) | Open::Record(lines) => lines.push(line.to_owned()),
                    Open::List(items) => push_to_item(items, line),
                    Open::None => open = Open::Paragraph(vec![line.to_owned()]),
                }
                in_fence = !is_fence(line);
                continue;
            }
            if line.trim().is_empty() {
                after_blank = true;
                continue;
            }
            if is_fence(line) {
                in_fence = true;
            }
            open = match open {
                Open::List(mut items) if line.starts_with("- ") => {
                    items.push(vec![line.get(2..).unwrap_or_default().to_owned()]);
                    Open::List(items)
                }
                Open::List(mut items) if indented(line) => {
                    push_to_item(&mut items, line);
                    Open::List(items)
                }
                Open::Record(mut lines) if indented(line) && !after_blank => {
                    lines.push(line.to_owned());
                    Open::Record(lines)
                }
                Open::Paragraph(mut lines)
                    if !after_blank && !line.starts_with("- ") && !is_record(line) =>
                {
                    lines.push(line.to_owned());
                    Open::Paragraph(lines)
                }
                other => {
                    other.close(&mut blocks);
                    if line.starts_with("- ") {
                        Open::List(vec![vec![line.get(2..).unwrap_or_default().to_owned()]])
                    } else if is_record(line) {
                        Open::Record(vec![line.to_owned()])
                    } else {
                        Open::Paragraph(vec![line.to_owned()])
                    }
                }
            };
            after_blank = false;
        }
        open.close(&mut blocks);
        blocks
    }

    /// G09's paragraphs: runs of non-blank lines, each joined with single spaces.
    fn runs(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut current: Vec<String> = Vec::new();
        for line in &self.lines {
            if line.trim().is_empty() {
                if !current.is_empty() {
                    out.push(join(&current));
                    current.clear();
                }
            } else {
                current.push(line.clone());
            }
        }
        if !current.is_empty() {
            out.push(join(&current));
        }
        out
    }

    /// G09's sentences: each opens a paragraph or follows ". ", and runs to the next ". " or the
    /// end of its paragraph.
    #[must_use]
    pub fn sentences(&self) -> Vec<String> {
        self.runs()
            .iter()
            .flat_map(|p| p.split(". ").map(str::to_owned).collect::<Vec<_>>())
            .collect()
    }

    /// The rest of every sentence that opens with one of `labels`, in order.
    fn after_labels(&self, labels: &[&str]) -> Vec<String> {
        self.sentences()
            .iter()
            .filter_map(|s| {
                labels
                    .iter()
                    .find_map(|l| s.strip_prefix(l))
                    .map(str::to_owned)
            })
            .collect()
    }
}

/// The fields of a crate header.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CrateFields {
    /// The first paragraph.
    pub summary: Option<String>,
    /// The text after the job label.
    pub job: Option<String>,
    /// The layer as written: a number from 1 to 13, or `Development`.
    pub layer: Option<String>,
    /// Each invariant, one list item each.
    pub invariants: Vec<String>,
    /// The text after the sections label.
    pub sections: Option<String>,
    /// The chapters the sections name, ascending, each once.
    pub chapters: Vec<u8>,
    /// The catalogue families, if stated.
    pub family: Vec<String>,
    /// The text after `Status: `, if any.
    pub status: Option<String>,
}

/// How a module says what method it follows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Method {
    /// The method, with the links it names.
    Linked {
        /// The method's text: its paragraph after the label, and its list if one follows.
        text: String,
        /// Every link in that text, in order, each once.
        links: Vec<String>,
    },
    /// No published method applies, and why.
    None {
        /// The reason given.
        reason: String,
    },
}

/// The fields of a module header.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModuleFields {
    /// The first paragraph: the module's job.
    pub job: Option<String>,
    /// The text after `What it does: `, if any.
    pub does: Option<String>,
    /// Each guarantee: one per list item, or the paragraph's text when no list follows.
    pub guarantees: Vec<String>,
    /// The method.
    pub method: Option<Method>,
    /// The text after `Cost: `.
    pub cost: Option<String>,
    /// The catalogue families, if stated.
    pub family: Vec<String>,
}

/// A problem with one rule.
pub type Found = (Rule, String);

/// Whether `text` is one of [`PLACEHOLDERS`], in any case, without its closing punctuation.
#[must_use]
pub fn is_placeholder(text: &str) -> bool {
    let bare = text
        .trim()
        .trim_end_matches(['.', '!', ';', ':', ','])
        .trim()
        .to_lowercase();
    PLACEHOLDERS.contains(&bare.as_str())
}

/// Whether `text` states something: it has a letter or digit, and is not a placeholder.
#[must_use]
pub fn states(text: &str) -> bool {
    text.chars().any(char::is_alphanumeric) && !is_placeholder(text)
}

/// The first block's text, when it is a paragraph ending in a full stop.
fn first_paragraph(blocks: &[Block]) -> Option<String> {
    let text = match blocks.first()? {
        Block::Paragraph(lines) => join(lines),
        _ => return None,
    };
    text.ends_with('.').then_some(text)
}

/// A crate's summary: the first paragraph, `<the kind's opening><topic>.`.
fn crate_summary(blocks: &[Block], kind: CrateKind, problems: &mut Vec<Found>) -> Option<String> {
    let prefix = kind.summary_prefix();
    let summary = first_paragraph(blocks).filter(|text| {
        text.strip_prefix(prefix)
            .and_then(|topic| topic.strip_suffix('.'))
            .is_some_and(states)
    });
    if summary.is_none() {
        problems.push((
            Rule::Summary,
            format!(
                "the first paragraph must be the {} crate's summary, \"{prefix}<topic>.\"",
                kind.name()
            ),
        ));
    }
    summary
}

/// A module's summary: the first paragraph, one or more sentences ending in a full stop that say
/// what the module is for, and not a field.
fn module_summary(blocks: &[Block], problems: &mut Vec<Found>) -> Option<String> {
    let summary = first_paragraph(blocks).filter(|text| {
        states(text) && !labelled(text).is_some_and(|(label, _)| is_field_label(label))
    });
    if summary.is_none() {
        problems.push((
            Rule::Summary,
            "the first paragraph must be the summary, one or more sentences ending in a full \
             stop that say what the module is for (not a field such as \"Guarantees: \")"
                .to_owned(),
        ));
    }
    summary
}

/// The label of a paragraph (its text before the first `:`) and the text after the colon.
fn labelled(text: &str) -> Option<(&str, &str)> {
    let (label, rest) = text.split_once(':')?;
    Some((label.trim(), rest.trim()))
}

/// Whether a paragraph label opens a module field.
#[must_use]
pub fn is_field_label(label: &str) -> bool {
    is_guarantees_label(label)
        || is_method_label(label)
        || matches!(label, "Cost" | "What it does" | "Family")
}

/// The family field: `Family: ` and one or more catalogue families, separated by `, ` or ` and `.
fn family(blocks: &[Block], problems: &mut Vec<Found>) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = false;
    for block in blocks {
        let Block::Paragraph(lines) = block else {
            continue;
        };
        let text = join(lines);
        let Some(rest) = text.strip_prefix("Family: ") else {
            continue;
        };
        if seen {
            problems.push((Rule::Family, "the family is stated twice".to_owned()));
            continue;
        }
        seen = true;
        let rest = rest.trim().trim_end_matches('.');
        for name in rest.split(", ").flat_map(|part| part.split(" and ")) {
            let name = name.trim();
            if FAMILIES.contains(&name) {
                out.push(name.to_owned());
            } else {
                problems.push((
                    Rule::Family,
                    format!(
                        "{name:?} is not a family of chapter 10.2's catalogue ({})",
                        FAMILIES.join(", ")
                    ),
                ));
            }
        }
    }
    out
}

/// The families chapters 10.2 and 27 give the crate `name` ([`FAMILY_CRATES`]).
#[must_use]
pub fn required_families(name: &str) -> &'static [&'static str] {
    FAMILY_CRATES
        .iter()
        .find(|(krate, _)| *krate == name)
        .map_or(&[], |(_, families)| families)
}

/// Whether `text` names a chapter of the architecture: a run of digits from 1 to [`LAST_CHAPTER`]
/// that does not continue a word, a number or a section number (G09's rule).
#[must_use]
pub fn chapters_named(text: &str) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut previous: Option<char> = None;
    let mut digits = String::new();
    let mut digits_follow: Option<char> = None;
    for c in text.chars().chain(core::iter::once(' ')) {
        if c.is_ascii_digit() {
            if digits.is_empty() {
                digits_follow = previous;
            }
            digits.push(c);
        } else if !digits.is_empty() {
            let continues = digits_follow.is_some_and(|p| p.is_alphanumeric() || p == '.');
            if let Ok(n) = digits.parse::<u8>()
                && !continues
                && (1..=LAST_CHAPTER).contains(&n)
            {
                out.push(n);
            }
            digits.clear();
        }
        previous = Some(c);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The layer a header states, as G09 reads it: the first sentence opening with "Layer", an
/// optional colon and a space, then a word of letters and digits that is `Development` or a
/// number from 0 to 255.
fn layer(header: &Header) -> Option<String> {
    header.sentences().iter().find_map(|s| {
        let rest = s.strip_prefix("Layer")?;
        let rest = rest.strip_prefix(':').unwrap_or(rest).strip_prefix(' ')?;
        let token: String = rest
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        (token == "Development" || token.parse::<u8>().is_ok()).then_some(token)
    })
}

/// G09's "a `- ` list item follows": the index of the first `- ` line in the rest of the
/// paragraph that the lines from `from` continue, or of the first line of the next one.
fn list_after(lines: &[String], from: usize) -> Option<usize> {
    let mut after_blank = false;
    for (at, line) in lines.iter().enumerate().skip(from) {
        if line.starts_with("- ") {
            return Some(at);
        }
        if line.trim().is_empty() {
            after_blank = true;
        } else if after_blank {
            return None;
        }
    }
    None
}

/// The invariants, where G09 finds them: the first line that opens a paragraph (the header's
/// first line, or one after a blank line) and starts with "Invariants", with a `- ` list item in
/// the rest of its paragraph or as the first line of the next. The items are that list's.
fn invariants(header: &Header) -> Option<Vec<String>> {
    let lines = &header.lines;
    lines.iter().enumerate().find_map(|(at, line)| {
        let opens = at
            .checked_sub(1)
            .is_none_or(|b| lines.get(b).is_some_and(|l| l.trim().is_empty()));
        if !opens || !line.starts_with("Invariants") {
            return None;
        }
        let first = list_after(lines, at.checked_add(1)?)?;
        let list = Header {
            lines: lines.get(first..)?.to_vec(),
        };
        list.blocks().first().map(Block::items)
    })
}

/// The layer problem of a crate of `kind` whose header states `layer`, if any.
fn layer_problem(kind: CrateKind, layer: Option<&str>) -> Option<String> {
    let number = layer.and_then(|l| l.parse::<u8>().ok());
    match (kind, layer, number) {
        (_, None, _) => Some(NO_LAYER.to_owned()),
        (CrateKind::Development, Some("Development"), _) => None,
        (CrateKind::Development, Some(other), _) => Some(format!(
            "a development crate's layer is Development, not {other}"
        )),
        (_, Some("Development"), _) => Some(format!(
            "a {} crate's layer is a number from 1 to {LAST_LAYER}, not Development",
            kind.name()
        )),
        (CrateKind::Facade, _, Some(LAST_LAYER)) => None,
        (CrateKind::Facade, Some(other), _) => Some(format!(
            "the facade and its binding crates are layer {LAST_LAYER}, not {other}"
        )),
        (CrateKind::Kernel, Some(_), Some(n)) if (1..=LAST_LAYER).contains(&n) => None,
        (CrateKind::Kernel, Some(other), _) => Some(format!(
            "layer {other:?} is not a number from 1 to {LAST_LAYER} or Development"
        )),
    }
}

/// Reads a crate header (`HEADERS.md`, "Crate headers") of a crate of `kind`.
#[must_use]
pub fn crate_fields(header: &Header, kind: CrateKind) -> (CrateFields, Vec<Found>) {
    let mut problems = Vec::new();
    let blocks = header.blocks();
    let mut fields = CrateFields {
        summary: crate_summary(&blocks, kind, &mut problems),
        ..CrateFields::default()
    };

    let job = header.sentences().iter().find_map(|s| {
        let (label, rest) = JOB_LABELS
            .iter()
            .find_map(|l| s.strip_prefix(l).map(|rest| (*l, rest)))?;
        if !rest.chars().any(char::is_alphanumeric) {
            return None;
        }
        // "Job (chapter 28, ...): the job": the job is what follows the parenthesis.
        let job = match (label, rest.split_once("): ")) {
            ("Job (", Some((_, job))) => job,
            _ => rest,
        };
        Some(job.to_owned())
    });
    match &job {
        None => problems.push((
            Rule::Job,
            "no sentence opens with \"One job: \", \"Job: \", \"Job (\" or \"Its one job is \" \
             followed by the job"
                .to_owned(),
        )),
        Some(text) if !states(text) => problems.push((
            Rule::Job,
            format!("the job {text:?} states nothing; write what the crate is for"),
        )),
        Some(_) => {}
    }
    fields.job = job;

    fields.layer = layer(header);
    if let Some(problem) = layer_problem(kind, fields.layer.as_deref()) {
        problems.push((Rule::Layer, problem));
    }

    match invariants(header) {
        Some(items) => {
            for item in &items {
                if !states(item) {
                    problems.push((
                        Rule::Invariants,
                        format!("the invariant {item:?} states nothing; write the invariant"),
                    ));
                }
            }
            fields.invariants = items;
        }
        None => problems.push((
            Rule::Invariants,
            "no paragraph opens with \"Invariants\" followed by a \"- \" list of the invariants \
             the crate keeps"
                .to_owned(),
        )),
    }

    let sections = header
        .after_labels(&SECTIONS_LABELS)
        .into_iter()
        .find(|rest| !chapters_named(rest).is_empty());
    match sections {
        Some(text) => {
            fields.chapters = chapters_named(&text);
            fields.sections = Some(text);
        }
        None => problems.push((
            Rule::Sections,
            "no sentence opens with \"Sections: \" or \"Map: \" and names a chapter of the \
             architecture, 1 to 32"
                .to_owned(),
        )),
    }

    fields.family = family(&blocks, &mut problems);
    // The status line (G09's scaffold lines) and its wrapped continuation: the following non-blank
    // lines.
    let status: Vec<String> = header
        .lines
        .iter()
        .skip_while(|l| !l.starts_with("Status: "))
        .take_while(|l| !l.trim().is_empty())
        .cloned()
        .collect();
    fields.status = join(&status).strip_prefix("Status: ").map(str::to_owned);
    (fields, problems)
}

/// Whether a paragraph label opens the guarantees field: `Guarantees`, `What <subject>
/// guarantees` with a subject of [`GUARANTEES_SUBJECTS`], or one of [`INVARIANTS_LABELS`], each
/// alone or followed by `, ` and a qualifier.
#[must_use]
pub fn is_guarantees_label(label: &str) -> bool {
    let head = label.split_once(", ").map_or(label, |(head, _)| head);
    head == "Guarantees"
        || INVARIANTS_LABELS.contains(&head)
        || head
            .strip_prefix("What ")
            .and_then(|rest| rest.strip_suffix(" guarantees"))
            .is_some_and(|subject| GUARANTEES_SUBJECTS.contains(&subject))
}

/// Whether a paragraph label opens the method field: one of [`METHOD_LABELS`], or
/// `Method (<qualifier>)`.
#[must_use]
pub fn is_method_label(label: &str) -> bool {
    METHOD_LABELS.contains(&label)
        || label
            .strip_prefix("Method (")
            .is_some_and(|rest| rest.ends_with(')'))
}

/// The paragraphs labelled by `is_label`, each with the text after its colon and the block after
/// it.
fn labelled_paragraphs(
    blocks: &[Block],
    is_label: impl Fn(&str) -> bool,
) -> Vec<(String, Option<&Block>)> {
    let mut out = Vec::new();
    for (at, block) in blocks.iter().enumerate() {
        let Block::Paragraph(lines) = block else {
            continue;
        };
        let text = join(lines);
        if let Some((label, rest)) = labelled(&text)
            && is_label(label)
        {
            out.push((rest.to_owned(), blocks.get(at.saturating_add(1))));
        }
    }
    out
}

/// The one paragraph of a module field, or a problem when it is missing or written twice.
fn one<'b>(
    blocks: &'b [Block],
    rule: Rule,
    is_label: impl Fn(&str) -> bool,
    missing: &str,
    problems: &mut Vec<Found>,
) -> Option<(String, Option<&'b Block>)> {
    let mut found = labelled_paragraphs(blocks, is_label).into_iter();
    let first = found.next();
    if first.is_none() {
        problems.push((rule, missing.to_owned()));
    }
    if found.next().is_some() {
        problems.push((rule, format!("the {} field is written twice", rule.label())));
    }
    first
}

/// The method field's content, checked: links, or "none" and a reason.
fn method(rest: &str, after: Option<&Block>, problems: &mut Vec<Found>) -> Option<Method> {
    for mark in [",", ";", ":", "."] {
        if let Some(reason) = rest.strip_prefix("none").and_then(|r| r.strip_prefix(mark)) {
            let reason = reason.trim();
            if states(reason) {
                return Some(Method::None {
                    reason: reason.to_owned(),
                });
            }
            problems.push((
                Rule::Method,
                "\"Method: none\" needs the reason no published method applies".to_owned(),
            ));
            return None;
        }
    }
    let mut text = rest.to_owned();
    if let Some(list @ Block::List(_)) = after {
        text.push(' ');
        text.push_str(&list.text());
    }
    let text = text.trim().to_owned();
    let links: Vec<String> = links::find(&text).into_iter().map(|l| l.text).collect();
    if links.is_empty() {
        problems.push((
            Rule::Method,
            "the method names no link: a URL, a DOI, or a repository path to a .md file or folder \
             in backticks; write \"Method: none; <reason>\" if no published method applies"
                .to_owned(),
        ));
        return None;
    }
    Some(Method::Linked { text, links })
}

/// Reads a module header (`HEADERS.md`, "Module headers").
#[must_use]
pub fn module_fields(header: &Header) -> (ModuleFields, Vec<Found>) {
    let mut problems = Vec::new();
    let blocks = header.blocks();
    let mut fields = ModuleFields {
        job: module_summary(&blocks, &mut problems),
        ..ModuleFields::default()
    };

    fields.does = labelled_paragraphs(&blocks, |l| l == "What it does")
        .into_iter()
        .next()
        .map(|(rest, _)| rest);

    if let Some((rest, after)) = one(
        &blocks,
        Rule::Guarantees,
        is_guarantees_label,
        "no paragraph opens with \"Guarantees:\" followed by a list or by the guarantee itself",
        &mut problems,
    ) {
        match after {
            Some(list @ Block::List(_)) => fields.guarantees = list.items(),
            _ if !rest.is_empty() => fields.guarantees = vec![rest],
            _ => problems.push((
                Rule::Guarantees,
                "the guarantees label is followed by neither a \"- \" list nor text".to_owned(),
            )),
        }
        for g in &fields.guarantees {
            if !states(g) {
                problems.push((
                    Rule::Guarantees,
                    format!("the guarantee {g:?} states nothing; write what the module guarantees"),
                ));
            }
        }
    }

    if let Some((rest, after)) = one(
        &blocks,
        Rule::Method,
        is_method_label,
        "no paragraph opens with \"Method: \" naming the published method with a link",
        &mut problems,
    ) {
        fields.method = method(&rest, after, &mut problems);
    }

    if let Some((rest, _)) = one(
        &blocks,
        Rule::Cost,
        |l| l == "Cost",
        "no paragraph opens with \"Cost: \" stating what the module costs",
        &mut problems,
    ) {
        if states(&rest) {
            fields.cost = Some(rest);
        } else {
            problems.push((
                Rule::Cost,
                format!("\"Cost: {rest}\" states no cost; write its time, memory and allocation"),
            ));
        }
    }

    fields.family = family(&blocks, &mut problems);
    (fields, problems)
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

    fn of(lines: &[&str]) -> Header {
        Header {
            lines: lines.iter().map(|l| (*l).to_owned()).collect(),
        }
    }

    #[test]
    fn blocks_follow_the_grammar() {
        let h = of(&[
            "Summary line.",
            "",
            "Guarantees:",
            "- One,",
            "  continued.",
            "",
            "  Still item one.",
            "- Two.",
            "",
            "Method: a method.",
            "Consulted: X, MIT, <https://example.org/x.c>,",
            "  for testing.",
            "SPDX-License-Identifier: MPL-2.0",
            "",
            "```text",
            "- not an item",
            "",
            "Cost: not a field",
            "```",
        ]);
        let blocks = h.blocks();
        assert_eq!(
            blocks,
            [
                Block::Paragraph(vec!["Summary line.".into()]),
                Block::Paragraph(vec!["Guarantees:".into()]),
                Block::List(vec![
                    vec![
                        "One,".into(),
                        "  continued.".into(),
                        "  Still item one.".into()
                    ],
                    vec!["Two.".into()],
                ]),
                Block::Paragraph(vec!["Method: a method.".into()]),
                Block::Record(vec![
                    "Consulted: X, MIT, <https://example.org/x.c>,".into(),
                    "  for testing.".into()
                ]),
                Block::Record(vec!["SPDX-License-Identifier: MPL-2.0".into()]),
                Block::Paragraph(vec![
                    "```text".into(),
                    "- not an item".into(),
                    String::new(),
                    "Cost: not a field".into(),
                    "```".into()
                ]),
            ]
        );
        assert_eq!(
            blocks[2].items(),
            ["One, continued. Still item one.", "Two."]
        );
    }

    #[test]
    fn the_header_must_open_the_file() {
        assert!(Header::parse("// plain\n//! late\n").is_empty());
        assert_eq!(
            Header::parse("\u{feff}//! A.\r\n//!\r\n//!  B\r\nfn f() {}").lines,
            ["A.", "", " B"]
        );
    }

    #[test]
    fn chapters_follow_g09() {
        assert_eq!(
            chapters_named("15.6 and 16.6; open question OQ-15.7."),
            [15, 16]
        );
        assert_eq!(chapters_named("G08 and G09 decide."), Vec::<u8>::new());
        assert_eq!(chapters_named("chapter 99, 6.1 and 21."), [6, 21]);
    }

    #[test]
    fn labels() {
        for yes in [
            "Guarantees",
            "Guarantees, for every function here and every argument",
            "What every function here guarantees, for every input (there are no preconditions)",
            "What it guarantees",
            "What this module guarantees",
            "Invariants this module keeps",
            "Invariants this crate keeps, each with a test",
        ] {
            assert!(is_guarantees_label(yes), "{yes}");
        }
        for no in [
            "Guaranteed",
            "What it does",
            "This guarantees",
            "Guarantees nothing",
            "What this module never guarantees",
            "What it no longer guarantees",
            "Invariants it never keeps",
        ] {
            assert!(!is_guarantees_label(no), "{no}");
        }
        for yes in [
            "Method",
            "Methods",
            "Methods followed",
            "Method (chapter 6)",
        ] {
            assert!(is_method_label(yes), "{yes}");
        }
        for no in ["Methodology", "The method", "Methods we never followed"] {
            assert!(!is_method_label(no), "{no}");
        }
    }

    #[test]
    fn placeholders_state_nothing() {
        for no in ["none.", "TODO", "  TBD. ", "Unknown!", "-", ".", "n/a"] {
            assert!(!states(no), "{no:?}");
        }
        for yes in ["none at run time.", "constant.", "O(n)."] {
            assert!(states(yes), "{yes:?}");
        }
    }

    #[test]
    fn layers_and_invariants_are_found_where_g09_finds_them() {
        let header = of(&[
            "Spare Kit geometry kernel: a thing.",
            "",
            "One job: things.",
            "",
            "Layer: the arrangements. Layer: 6, Arrangements.",
            "",
            "Invariants this crate keeps:",
            "Consulted: X, MIT, <https://example.org/x.c>, for a test.",
            "- It keeps one.",
            "  Over two lines.",
            "",
            "Sections: 11.3.",
        ]);
        let (fields, problems) = crate_fields(&header, CrateKind::Kernel);
        assert_eq!(problems, []);
        assert_eq!(fields.layer.as_deref(), Some("6"));
        assert_eq!(fields.invariants, ["It keeps one. Over two lines."]);
        // An Invariants line that does not open a paragraph is not the field.
        let header = of(&[
            "Spare Kit geometry kernel: a thing.",
            "",
            "One job: things.",
            "",
            "Layer: 6.",
            "",
            "SPDX-License-Identifier: MPL-2.0",
            "Invariants this crate keeps:",
            "- It keeps one.",
            "",
            "Sections: 11.3.",
        ]);
        let (_, problems) = crate_fields(&header, CrateKind::Kernel);
        let rules: Vec<Rule> = problems.iter().map(|(r, _)| *r).collect();
        assert_eq!(rules, [Rule::Invariants]);
    }

    #[test]
    fn summaries_have_their_kind_s_form() {
        let lib = |first: &str| {
            of(&[
                first,
                "",
                "One job: x.",
                "",
                "Layer: 13.",
                "",
                "Invariants:",
                "- One.",
                "",
                "Sections: 25.",
            ])
        };
        let summary_problems = |first: &str, kind: CrateKind| {
            crate_fields(&lib(first), kind)
                .1
                .into_iter()
                .filter(|(r, _)| *r == Rule::Summary)
                .count()
        };
        assert_eq!(
            summary_problems("Shared engine: the facade.", CrateKind::Facade),
            0
        );
        assert_eq!(
            summary_problems("Spare Kit geometry kernel: x.", CrateKind::Facade),
            1
        );
        assert_eq!(
            summary_problems("Spare Kit geometry kernel: x.", CrateKind::Kernel),
            0
        );
        assert_eq!(
            summary_problems("Spare Kit geometry kernel: TODO.", CrateKind::Kernel),
            1
        );
        assert_eq!(summary_problems("An example.", CrateKind::Kernel), 1);
        let (_, problems) = module_fields(&of(&[
            "Cost: constant.",
            "",
            "Method: none; a type.",
            "",
            "Guarantees: one.",
        ]));
        assert!(problems.iter().any(|(r, _)| *r == Rule::Summary));
    }
}
