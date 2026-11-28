//! The report's shape, version 1, and its checks. `FORMAT.md` describes it for people; this file is
//! the reference for code. Published: a record written in this shape stays readable for ever, so a
//! change to it is a new format version, never an edit.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use kit_core::Id;
use regex::{Captures, RegexBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The version of the record's shape.
pub const FORMAT: u32 = 1;
/// The most a record holds without its screenshot: the bytes of its JSON.
pub const MAX_RECORD_BYTES: usize = 256 * 1024;
/// The most a screenshot holds, in bytes of the image.
pub const MAX_SCREENSHOT_BYTES: usize = 1024 * 1024;
/// The longest base64 text a screenshot in the cap can be.
const MAX_SCREENSHOT_BASE64: usize = MAX_SCREENSHOT_BYTES.div_ceil(3) * 4;
/// The record's cap, in the kilobytes its refusal names.
const MAX_RECORD_KB: usize = MAX_RECORD_BYTES / 1024;
/// The most elements a report names.
pub const MAX_ELEMENTS: usize = 50;
/// The most edits a report holds to replay.
pub const MAX_EDITS: usize = 500;
/// The most lines of the page's errors, and of the server's log, a report holds.
pub const MAX_LINES: usize = 200;
/// The longest such line, in characters.
pub const MAX_LINE_CHARS: usize = 500;
/// The longest summary, in characters.
pub const MAX_SUMMARY_CHARS: usize = 160;
/// The longest a person's own words may be, in characters.
pub const MAX_WORDS_CHARS: usize = 4000;
/// Who the sender is in a report's edits: never their name, nor their account.
pub const SENDER: &str = "sender";
/// The start of every other person's stand-in name in a report: `other-1`, `other-2`, in the order
/// they first appear.
pub const OTHER: &str = "other-";
/// What a name of another person that the server knows reads as in a report's words, when it is
/// not one of the report's own stand-ins.
pub const ANOTHER: &str = "another person";

/// The `n`th other person's stand-in name, counting from 1.
#[must_use]
pub fn other(n: usize) -> String {
    format!("{OTHER}{n}")
}

fn is_stand_in(by: &str) -> bool {
    by == SENDER
        || by
            .strip_prefix(OTHER)
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// Why a record is not a report this server keeps, in words for the person who sent it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invalid(pub String);

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Invalid {}

fn invalid<T>(words: impl Into<String>) -> Result<T, Invalid> {
    Err(Invalid(words.into()))
}

/// What a report is.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Something went wrong, with the facts to find it.
    Problem,
    /// An idea or a wish, in the person's words.
    Feedback,
}

/// What a piece of feedback is about, so the owner reads it grouped.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Topic {
    /// Drawing and changing the model: walls, floors, roofs, families.
    Modelling,
    /// Views, plans, sections and sheets.
    Views,
    /// Schedules, parameters and types.
    Data,
    /// Working with others: teams, sharing, links.
    Sharing,
    /// How fast it is.
    Speed,
    /// The look and the words.
    Words,
    /// Anything else.
    Other,
}

impl Topic {
    /// Every topic, in the order the page offers them.
    pub const ALL: [Topic; 7] = [
        Topic::Modelling,
        Topic::Views,
        Topic::Data,
        Topic::Sharing,
        Topic::Speed,
        Topic::Words,
        Topic::Other,
    ];

    /// The topic as the page names it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Topic::Modelling => "Drawing and changing the model",
            Topic::Views => "Views and sheets",
            Topic::Data => "Schedules, parameters and types",
            Topic::Sharing => "Working with others",
            Topic::Speed => "Speed",
            Topic::Words => "The look and the words",
            Topic::Other => "Something else",
        }
    }
}

/// Where a failure came from: the first part of its code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The server turned an edit back: `refused.<command>`.
    Refused,
    /// An element did not build: `build.<the builder's code>`.
    Build,
    /// A body the page built differs from the server's checkpoint: `mismatch.body`.
    Mismatch,
    /// The server answered a request with "Something went wrong": `server.failed`.
    Server,
    /// The page itself failed: `page.error`.
    Page,
}

impl Source {
    /// Every source.
    pub const ALL: [Source; 5] = [
        Source::Refused,
        Source::Build,
        Source::Mismatch,
        Source::Server,
        Source::Page,
    ];

    /// The first part of a code of this source.
    #[must_use]
    pub fn prefix(self) -> &'static str {
        match self {
            Source::Refused => "refused",
            Source::Build => "build",
            Source::Mismatch => "mismatch",
            Source::Server => "server",
            Source::Page => "page",
        }
    }

    /// The source a code names by its first part, if it names one.
    #[must_use]
    pub fn of(code: &str) -> Option<Source> {
        let (first, rest) = code.split_once('.')?;
        if rest.is_empty() {
            return None;
        }
        Source::ALL.into_iter().find(|s| s.prefix() == first)
    }

    /// Whether the server replays a failure of this source: a refusal through the commands, a
    /// build failure through the builder. The others happened outside what the edits decide.
    #[must_use]
    pub fn replayed(self) -> bool {
        matches!(self, Source::Refused | Source::Build)
    }
}

/// What went wrong: a stable code and the words the person was shown.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    /// `<source>.<detail>`: `refused.element.param`, `build.Unfilled`, `mismatch.body`,
    /// `server.failed` or `page.error`.
    pub code: String,
    /// The words the person was shown.
    pub words: String,
    /// The element it was about, by ID, when it was about one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    /// The server's ID of the request it happened in, when there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
}

impl Failure {
    /// The server turned back `cmd` with these words.
    #[must_use]
    pub fn refused(cmd: &str, words: impl Into<String>) -> Failure {
        Failure {
            code: format!("{}.{cmd}", Source::Refused.prefix()),
            words: words.into(),
            element: None,
            request: None,
        }
    }

    /// `element` did not build, with the builder's code and words.
    #[must_use]
    pub fn build(code: &str, words: impl Into<String>, element: &str) -> Failure {
        Failure {
            code: format!("{}.{code}", Source::Build.prefix()),
            words: words.into(),
            element: Some(element.to_owned()),
            request: None,
        }
    }

    /// Where it came from, by its code.
    #[must_use]
    pub fn source(&self) -> Option<Source> {
        Source::of(&self.code)
    }

    fn check(&self) -> Result<(), Invalid> {
        let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
        if self.source().is_none() || self.code.len() > 100 || !self.code.chars().all(plain) {
            return invalid(format!(
                "{:?} isn't a failure code: it is refused., build., mismatch., server. or page. and then a name.",
                truncate(&self.code, 100)
            ));
        }
        one_line("the failure's words", &self.words, MAX_LINE_CHARS, true)?;
        if let Some(element) = &self.element {
            id("the failure's element", element)?;
        }
        if let Some(request) = &self.request {
            one_line("the failure's request", request, 64, true)?;
        }
        Ok(())
    }
}

/// The project and model a report is about, by ID: what it belongs to, never their data.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Where {
    /// The project.
    pub project: String,
    /// The model (document) in it.
    pub model: String,
}

/// An element the failure is about, or one it reads, with its record as stored: its kind, its
/// recipe and its inputs.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Part {
    /// Its permanent ID.
    pub id: String,
    /// Its record as the model stores it.
    pub record: Value,
}

/// One edit of the model's log that touched those elements, to replay: the command and its
/// arguments, who sent it as a stand-in, the log's own time, and how it ended then.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Its number in the model's log.
    pub seq: u64,
    /// Who sent it: [`SENDER`], or another person's stand-in (`other-1`).
    pub by: String,
    /// The log's time for it, which the replay runs it at.
    pub at_ms: u64,
    /// The command.
    pub cmd: String,
    /// Its arguments, as sent.
    pub args: Value,
    /// Why it was turned back then, when it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed: Option<String>,
}

/// The program that was running.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Program {
    /// The server's version.
    pub version: String,
    /// The engine that built the model's shapes, with its version: `stand-in 1`.
    pub engine: String,
}

/// A picture the person chose to add.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Screenshot {
    /// `image/png` or `image/jpeg`.
    pub media: String,
    /// The image, as base64.
    pub data: String,
}

impl Screenshot {
    /// The image's bytes, checked: a PNG or a JPEG, as its media type says, at most
    /// [`MAX_SCREENSHOT_BYTES`].
    ///
    /// # Errors
    ///
    /// [`Invalid`], saying what is wrong with it.
    pub fn bytes(&self) -> Result<Vec<u8>, Invalid> {
        // Base64 holds three bytes in four characters: a longer text cannot be a picture in the cap.
        if self.data.len() > MAX_SCREENSHOT_BASE64 {
            return invalid("The screenshot is larger than 1 MB. Send a smaller one.");
        }
        let Ok(bytes) = STANDARD.decode(&self.data) else {
            return invalid("The screenshot isn't readable as an image.");
        };
        if bytes.len() > MAX_SCREENSHOT_BYTES {
            return invalid("The screenshot is larger than 1 MB. Send a smaller one.");
        }
        let png = bytes.starts_with(b"\x89PNG\r\n\x1a\n");
        let jpeg = bytes.starts_with(&[0xFF, 0xD8, 0xFF]);
        match self.media.as_str() {
            "image/png" if png => Ok(bytes),
            "image/jpeg" if jpeg => Ok(bytes),
            _ => invalid("A screenshot is a PNG or a JPEG picture."),
        }
    }
}

/// A report of a problem, or feedback, as the person sent it: only the parts they left in.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// The version of this shape: [`FORMAT`].
    pub format: u32,
    /// A problem or feedback.
    pub kind: Kind,
    /// One line: what a fixer reads first.
    pub summary: String,
    /// The person's own words.
    #[serde(default)]
    pub words: String,
    /// Feedback's topic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<Topic>,
    /// What went wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
    /// The project and model it is about.
    #[serde(default, rename = "where", skip_serializing_if = "Option::is_none")]
    pub place: Option<Where>,
    /// The elements it is about and those they read, with their records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<Part>,
    /// The edits of the model's log that touched them, in log order, to replay.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<Step>,
    /// The program's version and engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<Program>,
    /// The browser, as it names itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    /// The page's own error lines.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub page_errors: Vec<String>,
    /// The server's log lines for the sender's own requests, by request ID.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub server_log: Vec<String>,
    /// A picture the person added.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<Screenshot>,
}

fn truncate(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}

fn one_line(what: &str, text: &str, most: usize, required: bool) -> Result<(), Invalid> {
    if required && text.trim().is_empty() {
        return invalid(format!("Give {what}."));
    }
    if text.chars().count() > most {
        return invalid(format!(
            "{} is longer than {most} characters.",
            capital(what)
        ));
    }
    if text.chars().any(char::is_control) {
        return invalid(format!("{} is one line.", capital(what)));
    }
    Ok(())
}

fn capital(what: &str) -> String {
    let mut chars = what.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn id(what: &str, text: &str) -> Result<(), Invalid> {
    Id::parse(text)
        .map(|_| ())
        .map_err(|_| Invalid(format!("{} {text:?} isn't an ID.", capital(what))))
}

fn lines(what: &str, list: &[String]) -> Result<(), Invalid> {
    if list.len() > MAX_LINES {
        return invalid(format!(
            "{} holds more than {MAX_LINES} lines.",
            capital(what)
        ));
    }
    for line in list {
        one_line(what, line, MAX_LINE_CHARS, false)?;
    }
    Ok(())
}

/// The command families that are not a model's own edits: making models, packages, links and
/// checkpoints. A report replays a model's own edits only.
const NOT_REPLAYED: [&str; 4] = ["model.", "package.", "link.", "document."];

impl Report {
    /// Whether this is a report this server keeps: its format, every part within its cap, a
    /// feedback with a topic and no failure, edits that are a model's own in log order, sent by
    /// stand-ins, and a screenshot that is a picture.
    ///
    /// # Errors
    ///
    /// [`Invalid`], saying the first thing wrong.
    pub fn check(&self) -> Result<(), Invalid> {
        if self.format != FORMAT {
            return invalid(format!(
                "This report is format {}; this server reads format {FORMAT}.",
                self.format
            ));
        }
        one_line("a summary", &self.summary, MAX_SUMMARY_CHARS, true)?;
        if self.words.chars().count() > MAX_WORDS_CHARS {
            return invalid(format!(
                "Your words are longer than {MAX_WORDS_CHARS} characters."
            ));
        }
        self.check_kind()?;
        if let Some(failure) = &self.failure {
            failure.check()?;
        }
        if let Some(place) = &self.place {
            id("the project", &place.project)?;
            id("the model", &place.model)?;
        } else if !self.elements.is_empty() || !self.edits.is_empty() {
            return invalid("Elements and edits come with the model they are from.");
        }
        self.check_elements()?;
        self.check_edits()?;
        if let Some(program) = &self.program {
            one_line("the program's version", &program.version, 100, true)?;
            one_line("the engine", &program.engine, 100, true)?;
        }
        if let Some(browser) = &self.browser {
            one_line("the browser", browser, 300, false)?;
        }
        lines("the page's errors", &self.page_errors)?;
        lines("the server's log", &self.server_log)?;
        if let Some(screenshot) = &self.screenshot {
            screenshot.bytes()?;
        }
        let bare = Report {
            screenshot: None,
            ..self.clone()
        };
        let size = serde_json::to_vec(&bare).map_or(usize::MAX, |bytes| bytes.len());
        if size > MAX_RECORD_BYTES {
            return invalid(format!(
                "This report is {} KB without its screenshot; the most is {MAX_RECORD_KB} KB. Leave a part out.",
                size.div_ceil(1024)
            ));
        }
        Ok(())
    }

    fn check_kind(&self) -> Result<(), Invalid> {
        match self.kind {
            Kind::Feedback => {
                if self.topic.is_none() {
                    return invalid("Choose what your feedback is about.");
                }
                if self.failure.is_some() || !self.elements.is_empty() || !self.edits.is_empty() {
                    return invalid(
                        "Feedback holds your words; a problem with elements and edits is a report of a problem.",
                    );
                }
            }
            Kind::Problem => {
                if self.topic.is_some() {
                    return invalid("A topic is for feedback.");
                }
            }
        }
        Ok(())
    }

    fn check_elements(&self) -> Result<(), Invalid> {
        if self.elements.len() > MAX_ELEMENTS {
            return invalid(format!("A report names at most {MAX_ELEMENTS} elements."));
        }
        let mut seen = BTreeSet::new();
        for part in &self.elements {
            id("an element", &part.id)?;
            if !seen.insert(part.id.as_str()) {
                return invalid(format!("Element {} is in the report twice.", part.id));
            }
            if !part.record.is_object() {
                return invalid(format!("Element {}'s record isn't a record.", part.id));
            }
        }
        Ok(())
    }

    fn check_edits(&self) -> Result<(), Invalid> {
        if self.edits.len() > MAX_EDITS {
            return invalid(format!("A report holds at most {MAX_EDITS} edits."));
        }
        let mut last = 0_u64;
        for step in &self.edits {
            if step.seq <= last {
                return invalid("The edits are in the order of the log, each once.");
            }
            last = step.seq;
            if !is_stand_in(&step.by) {
                return invalid(format!(
                    "Edit {} is by {:?}: an edit is by {SENDER} or by another person's stand-in, never by a name.",
                    step.seq,
                    truncate(&step.by, 40)
                ));
            }
            let plain = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.';
            if step.cmd.is_empty() || step.cmd.len() > 64 || !step.cmd.chars().all(plain) {
                return invalid(format!(
                    "Edit {} has no command this server knows.",
                    step.seq
                ));
            }
            if NOT_REPLAYED
                .iter()
                .any(|family| step.cmd.starts_with(family))
            {
                return invalid(format!(
                    "Edit {} is {}: a report replays a model's own edits only.",
                    step.seq, step.cmd
                ));
            }
            if !step.args.is_object() {
                return invalid(format!("Edit {}'s arguments aren't a record.", step.seq));
            }
            if let Some(failed) = &step.failed {
                one_line("an edit's refusal", failed, MAX_LINE_CHARS, true)?;
            }
        }
        Ok(())
    }

    /// The sender's edit that was turned back last: the one a refusal is about.
    #[must_use]
    pub fn failing(&self) -> Option<&Step> {
        self.edits
            .iter()
            .rev()
            .find(|step| step.by == SENDER && step.failed.is_some())
    }

    /// Takes names out of the report's words: each of `names` (a person's name, and what it reads
    /// as instead) as a whole word, in any case, in the summary, the person's words, the failure's
    /// words, each edit's refusal, the page's errors and the server's lines. The records and the
    /// edits' arguments are the model's inputs, and are left as they are.
    ///
    /// # Errors
    ///
    /// [`Invalid`] when there are so many names that they cannot be looked for at once.
    pub fn scrub(&mut self, names: &[(String, String)]) -> Result<(), Invalid> {
        let mut instead: BTreeMap<String, &str> = BTreeMap::new();
        for (name, with) in names {
            let name = name.trim();
            if !name.is_empty() {
                instead.insert(name.to_lowercase(), with.as_str());
            }
        }
        if instead.is_empty() {
            return Ok(());
        }
        // Longest first, so "Anna Lee" is found before "Anna".
        let mut sorted: Vec<&String> = instead.keys().collect();
        sorted.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()).then(a.cmp(b)));
        let pattern = sorted
            .iter()
            .map(|name| regex::escape(name))
            .collect::<Vec<_>>()
            .join("|");
        let found = RegexBuilder::new(&format!(r"\b(?:{pattern})\b"))
            .case_insensitive(true)
            .build()
            .map_err(|_| Invalid("There are too many names to look for.".to_owned()))?;
        let replace = |text: &mut String| {
            let changed = found.replace_all(text, |captures: &Captures<'_>| {
                let name = captures.get(0).map_or("", |m| m.as_str()).to_lowercase();
                instead.get(&name).copied().unwrap_or(ANOTHER).to_owned()
            });
            *text = changed.into_owned();
        };
        replace(&mut self.summary);
        replace(&mut self.words);
        if let Some(failure) = &mut self.failure {
            replace(&mut failure.words);
        }
        for step in &mut self.edits {
            if let Some(failed) = &mut step.failed {
                replace(failed);
            }
        }
        for line in self
            .page_errors
            .iter_mut()
            .chain(self.server_log.iter_mut())
        {
            replace(line);
        }
        Ok(())
    }
}
