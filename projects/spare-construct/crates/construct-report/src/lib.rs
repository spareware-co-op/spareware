//! A report of a problem, or feedback, as a person sends it from Spare Construct's page and the
//! co-op's private reports store keeps it (piece A100, note
//! `projects/spare-construct/notes/A100-report-a-problem.md`; the record's published shape is
//! `FORMAT.md` beside this crate).
//!
//! What it promises:
//!
//! - [`Report`] is the one shape, version [`FORMAT`]: a closed record (a field it does not name is
//!   refused when it is read), so it holds only the parts it lists, each of which the person saw
//!   and could leave out before it was sent.
//! - [`Report::check`] refuses, in words, a record over its caps or holding what a report never
//!   holds; [`Report::scrub`] takes the names of people other than the sender out of its words.
//! - [`Stored`] is a report in the store: its number, its sender's account, the [`State`] the
//!   sender sees and the server's [`Replay`] of it. [`Stored::line`] is the one line a fixer reads
//!   first; [`public_issue`] is the generic issue a reproduced fault becomes, with no project data;
//!   [`by_topic`] groups feedback by its topic.
//!
//! It depends on kit-core (IDs), serde and `serde_json` (the record), base64 (a screenshot's bytes)
//! and regex (names as whole words). No store, no clock and no network: the server stores and
//! replays (`projects/spare-construct/server/src/report.rs` and `replay.rs`).

mod report;
mod stored;

pub use report::{
    ANOTHER, FORMAT, Failure, Invalid, Kind, MAX_EDITS, MAX_ELEMENTS, MAX_LINE_CHARS, MAX_LINES,
    MAX_RECORD_BYTES, MAX_SCREENSHOT_BYTES, MAX_SUMMARY_CHARS, MAX_WORDS_CHARS, OTHER, Part,
    Program, Report, SENDER, Screenshot, Source, Step, Topic, Where, other,
};
pub use stored::{Issue, Listed, Replay, Replayed, State, Stored, by_topic, public_issue};
