//! Reading a crate's `lib.rs` header: whether it states its one job, its layer, its invariants and
//! the sections it implements.
//!
//! What it does: takes the leading `//!` block of a `lib.rs` and answers whether it states the
//! four things chapter 27 asks of every crate, which layer it names, and which dependencies it
//! still waits for. The crate's attributes are read by `source`, its manifest by `manifest`.
//!
//! What it guarantees: a field counts only when written in the grammar below, so a header cannot
//! pass by mentioning a word in passing. The grammar is this piece's interim one; G15 writes the
//! full header grammar that `MAP.json` is generated from, and this reader follows it then.
//!
//! The grammar. A header's paragraphs are its runs of non-blank lines, read as one line each; a
//! sentence opens a paragraph or follows ". ", and runs to the next ". " or the paragraph's end.
//!
//! - Job: a sentence opening with "One job: ", "Job: ", "Job (" or "Its one job is ", followed by
//!   the job itself.
//! - Layer: a sentence opening with "Layer", an optional colon and a space, then the layer number
//!   of chapter 27's layer table or "Development".
//! - Invariants: a paragraph opening with "Invariants", followed by a "- " list item in the same
//!   paragraph or the next one.
//! - Sections: a sentence opening with "Sections: " or "Map: " that names at least one chapter of
//!   the architecture (a number from 1 to `LAST_CHAPTER`).
//!
//! Method: paragraph and sentence scanning; the grammar above is the whole method.
//!
//! Cost: linear in the length of the header.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The kernel lints deny native arithmetic in kernel code (Cargo.toml). This is test code: its
// readers count positions in a text they hold whole, which tests may do with native arithmetic.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "test code; the rule is for kernel code"
)]

use crate::table::Layer;

/// One of the four things every crate's header states (chapter 27).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Field {
    /// Its one job.
    Job,
    /// Its layer.
    Layer,
    /// The invariants it keeps.
    Invariants,
    /// The sections of the architecture it implements.
    Sections,
}

/// Every field, in the order a header states them.
pub const FIELDS: [Field; 4] = [Field::Job, Field::Layer, Field::Invariants, Field::Sections];

/// The highest chapter number of the kernel architecture (32, the owner's decisions).
pub const LAST_CHAPTER: u8 = 32;

/// The labels that open a sentence stating the crate's job.
const JOB_LABELS: [&str; 4] = ["One job: ", "Job: ", "Job (", "Its one job is "];

/// The labels that open a sentence naming the sections the crate implements.
const SECTIONS_LABELS: [&str; 2] = ["Sections: ", "Map: "];

/// The prefix of the line that names a dependency still to add when a crate under way merges.
pub const PENDING_PREFIX: &str = "Direct dependencies still to add when ";

/// The prefix of the status line every scaffolded crate carries.
pub const STATUS_PREFIX: &str = "Status: scaffold only; built by pieces ";

/// The marks a crate's header carried while its row waited for owner action G08: one for chapter
/// 27's amendments, one for chapter 30.5's. G08 accepted both (section 32.13), so a header that
/// still carries one states something no longer true.
pub const STALE_MARKS: [&str; 2] = [
    "Proposed in chapter 27, pending the owner's decision G08.",
    "Proposed in chapter 30, pending the owner's decision G08.",
];

/// The pending line a crate's header carries for one dependency under way.
pub fn pending_line(dep: &str, piece: &str) -> String {
    format!("{PENDING_PREFIX}{dep} merges ({piece}): {dep}")
}

/// The leading `//!` block of a `lib.rs`, one entry per line, without the `//!` and the one space
/// after it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// The lines of the block, in order.
    pub lines: Vec<String>,
}

impl Header {
    /// Reads the block of `//!` lines that opens `source`. Lines before the first `//!` line end
    /// the block at once: the header must come first.
    pub fn parse(source: &str) -> Self {
        let mut lines = Vec::new();
        for line in source.lines() {
            let Some(rest) = line.trim_end().strip_prefix("//!") else {
                break;
            };
            lines.push(rest.strip_prefix(' ').unwrap_or(rest).to_owned());
        }
        Self { lines }
    }

    /// The header's paragraphs: runs of non-blank lines, each joined with single spaces.
    fn paragraphs(&self) -> Vec<String> {
        let mut paragraphs = Vec::new();
        let mut current: Vec<&str> = Vec::new();
        for line in &self.lines {
            let line = line.trim();
            if line.is_empty() {
                if !current.is_empty() {
                    paragraphs.push(current.join(" "));
                    current.clear();
                }
            } else {
                current.push(line);
            }
        }
        if !current.is_empty() {
            paragraphs.push(current.join(" "));
        }
        paragraphs
    }

    /// The header's sentences: each opens a paragraph or follows ". ", and runs to the next ". "
    /// or the end of its paragraph.
    fn sentences(&self) -> Vec<String> {
        let mut sentences = Vec::new();
        for paragraph in self.paragraphs() {
            sentences.extend(paragraph.split(". ").map(str::to_owned));
        }
        sentences
    }

    /// The rest of every sentence that opens with one of `labels`.
    fn after_labels(&self, labels: &[&str]) -> Vec<String> {
        self.sentences()
            .iter()
            .filter_map(|s| {
                labels
                    .iter()
                    .find_map(|label| s.strip_prefix(label))
                    .map(str::to_owned)
            })
            .collect()
    }

    /// Whether the header states `field`, in the grammar of this module's header.
    pub fn has(&self, field: Field) -> bool {
        match field {
            Field::Job => self
                .after_labels(&JOB_LABELS)
                .iter()
                .any(|rest| rest.chars().any(char::is_alphanumeric)),
            Field::Layer => self.layer().is_some(),
            Field::Invariants => self.lines.iter().enumerate().any(|(at, line)| {
                let opens_paragraph = at == 0 || self.lines[at - 1].trim().is_empty();
                opens_paragraph
                    && line.starts_with("Invariants")
                    && list_follows(&self.lines[at + 1..])
            }),
            Field::Sections => self
                .after_labels(&SECTIONS_LABELS)
                .iter()
                .any(|rest| names_a_chapter(rest)),
        }
    }

    /// The layer the header states, if it states one in the grammar of this module's header.
    pub fn layer(&self) -> Option<Layer> {
        self.sentences().iter().find_map(|s| {
            let rest = s.strip_prefix("Layer")?;
            let rest = rest.strip_prefix(':').unwrap_or(rest).strip_prefix(' ')?;
            let token: String = rest
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect();
            if token == "Development" {
                Some(Layer::Development)
            } else {
                token.parse::<u8>().ok().map(Layer::N)
            }
        })
    }

    /// Whether some line of the header starts with `prefix`.
    pub fn has_line_starting(&self, prefix: &str) -> bool {
        self.lines.iter().any(|l| l.starts_with(prefix))
    }

    /// Whether the header's text, with its lines joined by spaces, contains `text`.
    pub fn contains(&self, text: &str) -> bool {
        self.lines.join(" ").contains(text)
    }

    /// The crates named by the header's pending lines, in order.
    pub fn pending_deps(&self) -> Vec<String> {
        self.lines
            .iter()
            .filter_map(|l| {
                let rest = l.strip_prefix(PENDING_PREFIX)?;
                Some(rest.split(" merges").next().unwrap_or(rest).to_owned())
            })
            .collect()
    }
}

/// Whether a "- " list item follows: in the rest of the current paragraph, or as the first line
/// of the next one.
fn list_follows(rest: &[String]) -> bool {
    let mut after_blank = false;
    for line in rest {
        if line.starts_with("- ") {
            return true;
        }
        if line.trim().is_empty() {
            after_blank = true;
        } else if after_blank {
            return false;
        }
    }
    false
}

/// Whether `text` names a chapter of the architecture: a run of digits from 1 to `LAST_CHAPTER`
/// that does not continue a word, a number or a section number (so `G08` and the `.6` of `15.6`
/// are not chapters).
fn names_a_chapter(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0;
    while at < chars.len() {
        if !chars[at].is_ascii_digit() {
            at += 1;
            continue;
        }
        let start = at;
        while chars.get(at).is_some_and(char::is_ascii_digit) {
            at += 1;
        }
        let continues =
            start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '.');
        let digits: String = chars[start..at].iter().collect();
        if !continues
            && digits
                .parse::<u8>()
                .is_ok_and(|n| (1..=LAST_CHAPTER).contains(&n))
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "//! Spare Kit geometry kernel: a thing.\n//!\n//! One job: things.\n\
        //!\n//! Layer: 6, Arrangements.\n//!\n//! Invariants this crate keeps:\n//!\n\
        //! - It keeps one.\n//!\n//! Sections: 11.3.\n";

    fn of(lines: &[&str]) -> Header {
        let text: String = lines.iter().flat_map(|l| ["//! ", l, "\n"]).collect();
        Header::parse(&text)
    }

    #[test]
    fn a_complete_header_states_every_field() {
        let header = Header::parse(GOOD);
        for field in FIELDS {
            assert!(header.has(field), "{field:?}");
        }
        assert_eq!(header.layer(), Some(Layer::N(6)));
    }

    #[test]
    fn words_in_passing_do_not_count() {
        let header = Header::parse(
            "//! The numbers layer. Layers above it use it.\n//! - A list without a label.\n\
             //! Its job is numbers; see the map.\n",
        );
        for field in FIELDS {
            assert!(!header.has(field), "{field:?}");
        }
        // A label in the middle of a sentence, a label with nothing after it, and a Sections
        // sentence that names no chapter.
        for lines in [
            &["Each module here has one job: one concept."][..],
            &["One job:"],
            &["One job:", "", "Layer: 1."],
        ] {
            assert!(!of(lines).has(Field::Job), "{lines:?}");
        }
        for lines in [
            &["Sections: to be decided."][..],
            &["A heat Map: one colour per vertex."],
            &["Sections: G08 and G09 decide."],
            &["Sections: 99."],
            &["Sections: the rest. Chapter 27 follows."],
        ] {
            assert!(!of(lines).has(Field::Sections), "{lines:?}");
        }
        // An Invariants line whose list is a paragraph away.
        let lines = [
            "Invariants this crate keeps:",
            "",
            "Some prose.",
            "",
            "- One.",
        ];
        assert!(!of(&lines).has(Field::Invariants));
        // A Layer word inside a wrapped sentence.
        assert_eq!(
            of(&["It is used by the crates of the", "Layer 3 crates."]).layer(),
            None
        );
    }

    #[test]
    fn the_accepted_forms_of_each_field() {
        for lines in [
            &["One job: points."][..],
            &["Job (chapter 28): the corpus."],
            &["The numbers layer. Its one job is numbers: grids."],
            &["One job:", "points and vectors."],
        ] {
            assert!(of(lines).has(Field::Job), "{lines:?}");
        }
        for lines in [
            &["Sections: 15.6 and 16.6; open question OQ-15.7."][..],
            &["Map: chapter 6 of the kernel architecture."],
            &["It holds the budgets. Map: chapters 23.4 and 23.10."],
            &["Sections: 5, 22, 25 and 27 of the kernel", "architecture."],
        ] {
            assert!(of(lines).has(Field::Sections), "{lines:?}");
        }
        for lines in [
            &["Invariants this crate keeps:", "", "- One."][..],
            &["Invariants, each tested:", "- One."],
            &[
                "Invariants this crate keeps, each",
                "with a test:",
                "",
                "- One.",
            ],
        ] {
            assert!(of(lines).has(Field::Invariants), "{lines:?}");
        }
    }

    #[test]
    fn the_header_must_open_the_file() {
        let header = Header::parse(&format!("// A plain comment first.\n{GOOD}"));
        assert_eq!(header.lines, Vec::<String>::new());
    }

    #[test]
    fn layer_forms() {
        let layer = |text: &str| Header::parse(text).layer();
        assert_eq!(layer("//! Layer: 13, Facade."), Some(Layer::N(13)));
        assert_eq!(
            layer("//! Layer 1, the bottom of the kernel."),
            Some(Layer::N(1))
        );
        assert_eq!(layer("//! Layer: Development."), Some(Layer::Development));
        assert_eq!(layer("//! Layer: the facade."), None);
        assert_eq!(layer("//! Layers: 6."), None);
    }
}
