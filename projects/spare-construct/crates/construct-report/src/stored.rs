//! A report in the co-op's private reports store: its number, its sender, the state the sender
//! sees, the server's replay of it, and what a fixer and the owner read of it.

use crate::report::{Invalid, Kind, Report, Screenshot, Source, Topic};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// Where a report stands, as its sender sees it. Stored as `"received"`, `"reproduced"` or
/// `{"fixed": {"version": "..."}}`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum State {
    /// The co-op has it.
    Received,
    /// The server's replay of its edits, in an empty project, ended in the same failure.
    Reproduced,
    /// A version of the program carries the fix.
    Fixed {
        /// That version.
        version: String,
    },
}

impl State {
    /// The state in words, as the page shows it.
    #[must_use]
    pub fn words(&self) -> String {
        match self {
            State::Received => "Received".to_owned(),
            State::Reproduced => "Reproduced".to_owned(),
            State::Fixed { version } => format!("Fixed in version {version}"),
        }
    }
}

/// One edit of a replay: how it ended when it was made, and how it ended in the replay.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Replayed {
    /// Its number in the model's log.
    pub seq: u64,
    /// The command.
    pub cmd: String,
    /// Why it was turned back when it was made, if it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<String>,
    /// Why it was turned back in the replay, if it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now: Option<String>,
}

/// What the server's replay of a report found.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Replay {
    /// The engine the replay built with, with its version.
    pub engine: String,
    /// Whether it ended in the report's failure: the same code and the same words.
    pub reproduced: bool,
    /// The failure it ended in, by code, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// That failure's words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<String>,
    /// Each edit replayed, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Replayed>,
    /// Anything a fixer should know of it: why it was not replayed, or where it went another way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Replay {
    /// The edits that ended otherwise in the replay than when they were made.
    pub fn differing(&self) -> impl Iterator<Item = &Replayed> {
        self.steps.iter().filter(|step| step.then != step.now)
    }
}

/// A report as the store keeps it. Its screenshot, if it has one, is kept beside it under its
/// own key, so a list of reports never reads the pictures.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Stored {
    /// Its number, from 1.
    pub number: u64,
    /// The sender's account, by ID: the only person a report names.
    pub sender: String,
    /// When the server received it, in milliseconds since 1970 by the server's clock.
    pub received_ms: u64,
    /// Where it stands.
    pub state: State,
    /// The server's replay of it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay: Option<Replay>,
    /// The public issue it became, by its address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// Whether a screenshot is kept beside it.
    #[serde(default)]
    pub screenshot: bool,
    /// The report, without its screenshot.
    pub report: Report,
}

/// A report as its sender's list shows it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Listed {
    /// Its number.
    pub number: u64,
    /// A problem or feedback.
    pub kind: Kind,
    /// Its summary.
    pub summary: String,
    /// The failure's code, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Where it stands.
    pub state: State,
    /// The state in words.
    pub shown: String,
}

impl Stored {
    /// A report just received: its screenshot taken out to be kept beside it.
    #[must_use]
    pub fn received(
        number: u64,
        sender: &str,
        received_ms: u64,
        mut report: Report,
    ) -> (Stored, Option<Screenshot>) {
        let screenshot = report.screenshot.take();
        let stored = Stored {
            number,
            sender: sender.to_owned(),
            received_ms,
            state: State::Received,
            replay: None,
            issue: None,
            screenshot: screenshot.is_some(),
            report,
        };
        (stored, screenshot)
    }

    /// The server's replay: a received report it reproduces is reproduced.
    pub fn replayed(&mut self, replay: Replay) {
        if replay.reproduced && self.state == State::Received {
            self.state = State::Reproduced;
        }
        self.replay = Some(replay);
    }

    /// The fix is in `version`.
    ///
    /// # Errors
    ///
    /// [`Invalid`] when `version` is not one line, or the report is already marked fixed.
    pub fn fixed(&mut self, version: &str) -> Result<(), Invalid> {
        let version = version.trim();
        if version.is_empty() || version.chars().count() > 100 || version.contains(char::is_control) {
            return Err(Invalid("A version is one short line.".to_owned()));
        }
        if let State::Fixed { version: was } = &self.state {
            return Err(Invalid(format!(
                "Report {} is already fixed in version {was}.",
                self.number
            )));
        }
        self.state = State::Fixed {
            version: version.to_owned(),
        };
        Ok(())
    }

    /// The report as its sender's list shows it.
    #[must_use]
    pub fn listed(&self) -> Listed {
        Listed {
            number: self.number,
            kind: self.report.kind,
            summary: self.report.summary.clone(),
            code: self.report.failure.as_ref().map(|f| f.code.clone()),
            state: self.state.clone(),
            shown: self.state.words(),
        }
    }

    /// The one line a fixer reads first: the number and state, the code and words, the elements,
    /// how many edits and what the replay found, the engine and the program's version.
    #[must_use]
    pub fn line(&self) -> String {
        let report = &self.report;
        let mut line = format!("#{} {}", self.number, self.state.words().to_lowercase());
        match (&report.failure, report.kind) {
            (Some(failure), _) => {
                let _ = write!(line, " | {}: {}", failure.code, failure.words);
            }
            (None, Kind::Feedback) => {
                let topic = report.topic.map_or("", Topic::label);
                let _ = write!(line, " | feedback ({topic}): {}", report.summary);
            }
            (None, Kind::Problem) => {
                let _ = write!(line, " | {}", report.summary);
            }
        }
        if !report.elements.is_empty() {
            let ids: Vec<&str> = report.elements.iter().map(|p| p.id.as_str()).collect();
            let _ = write!(line, " | elements {}", ids.join(" "));
        }
        if !report.edits.is_empty() {
            let _ = write!(line, " | {} edits", report.edits.len());
        }
        if let Some(replay) = &self.replay {
            let found = if replay.reproduced {
                "reproduced it"
            } else {
                "did not reproduce it"
            };
            let _ = write!(line, "; the replay on {} {found}", replay.engine);
        }
        if let Some(program) = &report.program {
            let _ = write!(line, " | {} | version {}", program.engine, program.version);
        }
        line
    }
}

/// A public issue: its title and its body.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    /// One line.
    pub title: String,
    /// The text, in Markdown.
    pub body: String,
}

/// A name a program gives (a command, an element's kind, a builder's code): it is the program's
/// own, never a project's, when it is short and plain.
fn programs_own(name: &str) -> bool {
    let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    !name.is_empty() && name.len() <= 64 && name.chars().all(plain)
}

/// The generic public issue a reproduced fault becomes: its code, the commands its replay ran and
/// the kinds of element it involved, the engine and the program's version, and the private
/// report's number. Nothing else: no IDs, no arguments, no records, no words, no names.
///
/// # Errors
///
/// [`Invalid`] when the report is not a problem the server's replay reproduced.
pub fn public_issue(stored: &Stored) -> Result<Issue, Invalid> {
    let report = &stored.report;
    let (Some(failure), Some(replay)) = (&report.failure, &stored.replay) else {
        return Err(Invalid(format!(
            "Report {} has no failure the server replayed.",
            stored.number
        )));
    };
    if !replay.reproduced || !programs_own(&failure.code) {
        return Err(Invalid(format!(
            "Report {}'s failure was not reproduced: a public issue comes from a reproduced fault.",
            stored.number
        )));
    }
    let what = match failure.source() {
        Some(Source::Refused) => "the server turned an edit back",
        Some(Source::Build) => "an element did not build",
        Some(Source::Mismatch) => "a shape built in the browser differed from the server's",
        Some(Source::Server) => "the server failed on a request",
        Some(Source::Page) => "the page failed",
        None => "something went wrong",
    };
    let mut commands: Vec<&str> = Vec::new();
    for step in &report.edits {
        if programs_own(&step.cmd) && !commands.contains(&step.cmd.as_str()) {
            commands.push(&step.cmd);
        }
    }
    let kinds: BTreeSet<&str> = report
        .elements
        .iter()
        .filter_map(|part| part.record.get("kind").and_then(serde_json::Value::as_str))
        .filter(|kind| programs_own(kind))
        .collect();
    let mut body = format!(
        "A fault reproduced from a private report: number {} in the co-op's reports store, which is not public.\n\n",
        stored.number
    );
    let _ = writeln!(body, "- What happened: {what} (`{}`).", failure.code);
    let _ = writeln!(
        body,
        "- The replay: {} edits into an empty project, on the engine `{}`, ended in the same failure.",
        report.edits.len(),
        replay.engine
    );
    if !commands.is_empty() {
        let listed: Vec<String> = commands.iter().map(|c| format!("`{c}`")).collect();
        let _ = writeln!(body, "- Commands replayed, by first use: {}.", listed.join(", "));
    }
    if !kinds.is_empty() {
        let listed: Vec<String> = kinds.iter().map(|k| format!("`{k}`")).collect();
        let _ = writeln!(body, "- Kinds of element involved: {}.", listed.join(", "));
    }
    if let Some(program) = &report.program
        && programs_own(&program.version.replace('+', "."))
    {
        let _ = writeln!(body, "- Program version: {}.", program.version);
    }
    body.push_str(
        "\nThe private report holds the elements, the edits and the sender's words; this issue holds none of them. \
         The fix comes with a failing test made from the replay with its own generic values, and the report is marked \
         fixed in the version that carries it.\n",
    );
    Ok(Issue {
        title: format!("Reproduced from report {}: {}", stored.number, failure.code),
        body,
    })
}

/// Feedback grouped by its topic, for the owner: each topic with its reports' numbers, in order.
#[must_use]
pub fn by_topic(stored: &[Stored]) -> BTreeMap<Topic, Vec<u64>> {
    let mut grouped: BTreeMap<Topic, Vec<u64>> = BTreeMap::new();
    for one in stored {
        if one.report.kind == Kind::Feedback
            && let Some(topic) = one.report.topic
        {
            grouped.entry(topic).or_default().push(one.number);
        }
    }
    for numbers in grouped.values_mut() {
        numbers.sort_unstable();
    }
    grouped
}
