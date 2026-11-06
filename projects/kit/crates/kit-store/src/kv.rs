//! The small transactional store everything above is written against, and the file store beside
//! it. It asks for one thing of a database: snapshot reads, and transactions where two writers of
//! a key can't both win. TiKV gives that, and so does the in-memory store used in tests.
//!
//! Lesson from TiKV: an error from `commit` doesn't always mean nothing happened. The commit may
//! have landed and only the answer been lost. [`StoreError::Undetermined`] says so, and whoever
//! retries must be safe against the first try having worked. Edit counters do that for the log.

use crate::key::{Key, Prefix};
use std::future::Future;
use thiserror::Error;

/// A key and its value.
pub type Pair = (Key, Vec<u8>);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// Another writer changed something this transaction wrote. Nothing landed: run it again.
    #[error("another writer got there first")]
    Conflict,
    /// The commit may or may not have landed. Only a retry that is safe either way may follow.
    #[error("the commit may or may not have landed: {0}")]
    Undetermined(String),
    /// The store can't be reached, or failed.
    #[error("the store failed: {0}")]
    Unavailable(String),
    /// A stored row can't be read.
    #[error("a stored row is damaged: {0}")]
    Damaged(String),
    /// The row is from a newer version of the format than this program knows.
    #[error("this row is format version {found}; this program reads up to {known}")]
    TooNew { found: u32, known: u32 },
}

/// Reading: a snapshot at one moment, or a transaction's view of the store.
pub trait Reader: Send {
    fn get(
        &mut self,
        key: &Key,
    ) -> impl Future<Output = Result<Option<Vec<u8>>, StoreError>> + Send;

    /// Every pair in a range, in key order.
    fn scan(
        &mut self,
        prefix: &Prefix,
    ) -> impl Future<Output = Result<Vec<Pair>, StoreError>> + Send;

    /// Every pair from one key up to, and not including, another.
    fn scan_between(
        &mut self,
        from: &Key,
        to: &Key,
    ) -> impl Future<Output = Result<Vec<Pair>, StoreError>> + Send;
}

/// A transaction. It sees a snapshot plus its own writes. Dropping one without committing leaves
/// nothing behind.
pub trait Txn: Reader {
    fn put(
        &mut self,
        key: Key,
        value: Vec<u8>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    fn delete(&mut self, key: Key) -> impl Future<Output = Result<(), StoreError>> + Send;

    fn commit(self) -> impl Future<Output = Result<(), StoreError>> + Send;

    fn rollback(self) -> impl Future<Output = Result<(), StoreError>> + Send;
}

/// A store. Cheap to clone: clones share the same connection.
pub trait Kv: Clone + Send + Sync + 'static {
    type Snapshot: Reader;
    type Txn: Txn;

    fn snapshot(&self) -> impl Future<Output = Result<Self::Snapshot, StoreError>> + Send;
    fn begin(&self) -> impl Future<Output = Result<Self::Txn, StoreError>> + Send;
}

/// Files: package files and backups. RustFS, S3, Google Cloud Storage or a folder.
pub trait FileStore: Clone + Send + Sync + 'static {
    fn put(
        &self,
        path: &str,
        bytes: Vec<u8>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    fn get(&self, path: &str) -> impl Future<Output = Result<Vec<u8>, StoreError>> + Send;

    fn list(&self) -> impl Future<Output = Result<Vec<String>, StoreError>> + Send;
}
