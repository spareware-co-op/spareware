//! Spare Kit: commands and operations.
//!
//! Every change is an operation: commands, transactions, undo and redo, and the operation log.
//! People, AI agents, plugins and sync all go through this one path. Map: root ARCHITECTURE.md;
//! note K08; note A05 for why the log looks as it does.
//!
//! This crate holds what an edit is and how a log entry is written. It knows no database: running
//! edits is `kit-sync`'s job. The log entry is a published format, read forever (`FORMAT.md` in
//! `kit-store`, and the recorded samples in `tests/samples`).

pub mod entry;
pub mod id;

pub use entry::{
    Change, Claim, ClaimChange, ENTRY_FORMAT, Edit, Entry, FormatError, content_id, sha256_hex,
};
