//! An edit, and the log entry it becomes.
//!
//! A log entry holds what note A05 lists: the order, who and when, the intent (the command and
//! its arguments, kept for history, undo and AI audit), the input changes, their content ID, and
//! the result. The orderer's time is the only time a command may read, so replaying the log can't
//! depend on any clock.
//!
//! This is a published format. `decode` reads every version ever written; `encode` writes the
//! newest. Field order is fixed by these structs, because a content ID hashes the written bytes.
//! Numbers in `args` and `changes` are whole: a hash must never depend on how a float prints.

use kit_core::Id;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;

/// The version every entry is written with now.
pub const ENTRY_FORMAT: u32 = 1;

/// An edit on its way in, before it has a place in the log.
#[derive(Clone, Debug, PartialEq)]
pub struct Edit {
    pub person: Id,
    /// One browser tab, or one call from a server.
    pub client: Id,
    /// This client's own count. It goes up by one with each edit, so a resend is seen for what it
    /// is and never applied twice (Zero's `lastMutationID`).
    pub counter: u64,
    /// The last log number the client had seen.
    pub base: u64,
    /// What the edit acts on: a model, or in the directory a hub or person.
    pub target: Id,
    pub cmd: String,
    pub args: Value,
    /// The client's clock, kept for history. Nothing runs on it.
    pub author_ms: u64,
}

/// One input change: a piece went from one stored version to another.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Change {
    #[serde(with = "crate::id")]
    pub piece: Id,
    #[serde(with = "crate::id::option")]
    pub before: Option<Id>,
    #[serde(with = "crate::id::option")]
    pub after: Option<Id>,
}

/// Someone is working on a piece. It runs out by itself if they stop.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Claim {
    #[serde(with = "crate::id")]
    pub person: Id,
    pub name: String,
    pub until_ms: u64,
}

/// A claim taken or let go.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClaimChange {
    #[serde(with = "crate::id")]
    pub piece: Id,
    pub claim: Option<Claim>,
}

/// One entry of a log.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub v: u32,
    pub seq: u64,
    #[serde(with = "crate::id")]
    pub client: Id,
    pub counter: u64,
    pub op_id: String,
    #[serde(with = "crate::id")]
    pub author: Id,
    pub base: u64,
    pub author_ms: u64,
    pub orderer_ms: u64,
    #[serde(with = "crate::id")]
    pub target: Id,
    pub cmd: String,
    pub command_version: u32,
    pub args: Value,
    pub changes: Vec<Change>,
    pub claims: Vec<ClaimChange>,
    pub content_id: String,
    /// `None` when the edit applied; otherwise why it was turned back.
    pub failed: Option<String>,
}

impl Entry {
    /// An entry for an edit, in its place in the log.
    #[must_use]
    pub fn new(
        seq: u64,
        edit: &Edit,
        command_version: u32,
        orderer_ms: u64,
        changes: Vec<Change>,
        claims: Vec<ClaimChange>,
        failed: Option<String>,
    ) -> Self {
        Self {
            v: ENTRY_FORMAT,
            seq,
            client: edit.client.clone(),
            counter: edit.counter,
            op_id: format!("{}.{}", edit.client, edit.counter),
            author: edit.person.clone(),
            base: edit.base,
            author_ms: edit.author_ms,
            orderer_ms,
            target: edit.target.clone(),
            cmd: edit.cmd.clone(),
            command_version,
            args: edit.args.clone(),
            content_id: content_id(&changes),
            changes,
            claims,
            failed,
        }
    }

    /// The bytes stored in the log.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        // A struct of plain fields and a JSON value can't fail to serialise.
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Reads an entry of any version. Bad bytes are an error, never a panic.
    pub fn decode(bytes: &[u8]) -> Result<Self, FormatError> {
        #[derive(Deserialize)]
        struct Version {
            v: u32,
        }
        let damaged = |e: serde_json::Error| FormatError::Damaged(e.to_string());
        let version: Version = serde_json::from_slice(bytes).map_err(damaged)?;
        match version.v {
            1 => serde_json::from_slice(bytes).map_err(damaged),
            found => Err(FormatError::TooNew { found, known: ENTRY_FORMAT }),
        }
    }
}

/// Why an entry can't be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatError {
    Damaged(String),
    TooNew { found: u32, known: u32 },
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Damaged(why) => write!(f, "a log entry is damaged: {why}"),
            Self::TooNew { found, known } => {
                write!(f, "this log entry is version {found}; this program reads up to {known}")
            }
        }
    }
}

impl std::error::Error for FormatError {}

/// SHA-256 of some bytes, in hexadecimal.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// The content ID of a list of changes: SHA-256 over their written bytes.
#[must_use]
pub fn content_id(changes: &[Change]) -> String {
    sha256_hex(&serde_json::to_vec(changes).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn i(text: &str) -> Id {
        Id::parse(text).unwrap()
    }

    fn entry() -> Entry {
        let edit = Edit {
            person: i("anna"),
            client: i("tab-1"),
            counter: 7,
            base: 41,
            target: i("arch-model"),
            cmd: "element.move".to_owned(),
            args: json!({ "element": "D-104", "x": 1200, "y": 300 }),
            author_ms: 1000,
        };
        let change = Change { piece: i("D-104"), before: Some(i("aa")), after: Some(i("bb")) };
        Entry::new(42, &edit, 1, 2000, vec![change], Vec::new(), None)
    }

    #[test]
    fn an_entry_reads_back_exactly() {
        let entry = entry();
        assert_eq!(Entry::decode(&entry.encode()).unwrap(), entry);
    }

    #[test]
    fn a_change_to_the_content_changes_the_content_id() {
        let one = Change { piece: i("a"), before: None, after: Some(i("b")) };
        let two = Change { piece: i("a"), before: None, after: Some(i("c")) };
        assert_ne!(content_id(&[one]), content_id(&[two]));
    }

    #[test]
    fn an_entry_from_a_newer_program_is_refused() {
        let err = Entry::decode(br#"{"v":2}"#).unwrap_err();
        assert_eq!(err, FormatError::TooNew { found: 2, known: ENTRY_FORMAT });
    }

    #[test]
    fn a_bad_id_inside_an_entry_is_refused() {
        let mut value: Value = serde_json::from_slice(&entry().encode()).unwrap();
        value["author"] = json!("a/b");
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(matches!(Entry::decode(&bytes), Err(FormatError::Damaged(_))));
    }
}
