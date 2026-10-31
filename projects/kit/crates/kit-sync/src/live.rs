//! What one browser connection sees. A browser subscribes to the models it has open, sends edits
//! up, and gets every ordered edit of those models down. After a dropped connection it sends the
//! last log number it saw, gets what it missed, and resends what wasn't confirmed; the orderer
//! drops repeats, so a restart of a server loses nothing and doubles nothing.
//!
//! This is the channel's logic with no socket in it, so it can be tested alone. The web server
//! only moves its messages.

use crate::orderer::Reply;
use kit_core::Id;
use kit_ops::Entry;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;

/// What a browser can send.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum FromBrowser {
    /// Starts or resumes: who this tab is, what it watches, the last log number it saw.
    Hello {
        client: String,
        models: Vec<String>,
        since: u64,
    },
    /// Watches one more model.
    Watch {
        model: String,
        since: u64,
    },
    Stop {
        model: String,
    },
    Op {
        counter: u64,
        base: u64,
        model: String,
        cmd: String,
        args: Value,
        author_ms: u64,
    },
}

/// What one connection is watching, and how far through the log it has got.
#[derive(Debug, Default)]
pub struct Watcher {
    client: Option<Id>,
    watching: HashSet<Id>,
    cursor: u64,
}

impl Watcher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_client(&mut self, client: Id) {
        self.client = Some(client);
    }

    #[must_use]
    pub fn client(&self) -> Option<&Id> {
        self.client.as_ref()
    }

    pub fn watch(&mut self, model: Id) {
        self.watching.insert(model);
    }

    pub fn stop(&mut self, model: &Id) {
        self.watching.remove(model);
    }

    /// Every model being watched.
    #[must_use]
    pub fn watched(&self) -> Vec<Id> {
        self.watching.iter().cloned().collect()
    }

    #[must_use]
    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    pub fn set_cursor(&mut self, seq: u64) {
        self.cursor = seq;
    }

    /// Whether this connection may be shown an entry. A turned-back edit goes only to the tab that
    /// sent it; everyone else has nothing to see.
    #[must_use]
    pub fn sees(&self, entry: &Entry) -> bool {
        let mine = self.client.as_ref() == Some(&entry.client);
        self.watching.contains(&entry.target) && (entry.failed.is_none() || mine)
    }

    /// Takes an entry off the live feed. Entries already sent while catching up are skipped.
    pub fn next(&mut self, entry: &Entry) -> Option<Value> {
        if entry.seq <= self.cursor {
            return None;
        }
        self.cursor = entry.seq;
        self.sees(entry).then(|| entry_message(entry))
    }
}

#[must_use]
pub fn entry_message(entry: &Entry) -> Value {
    json!({ "t": "entry", "entry": entry })
}

#[must_use]
pub fn reply_message(reply: &Reply) -> Value {
    match reply {
        Reply::Ack { counter, seq } => json!({ "t": "ack", "counter": counter, "seq": seq }),
        Reply::Rejected { counter, reason } => {
            json!({ "t": "rejected", "counter": counter, "reason": reason })
        }
        Reply::Retry { counter } => json!({ "t": "retry", "counter": counter }),
    }
}
