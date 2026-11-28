//! The `kit-store` traits on TiKV, reached directly through `tikv-client`, and on `object_store`.
//!
//! Lessons from the direct route, which is less travelled than going through TiDB:
//!
//! - `tikv-client` panics when a transaction is dropped unfinished unless told otherwise. A dropped
//!   transaction is normal here, so `begin` turns that off.
//! - TiDB is what normally tells TiKV when old versions can go. There is no TiDB here, so without
//!   [`Tikv::collect_garbage`] every rewrite of a key stays on disk forever.
//! - A write conflict comes back nested inside key errors, and a commit whose outcome is unknown
//!   comes back as `UndeterminedError`. [`classify`] turns those into `kit-store`'s errors, which
//!   the orderer needs to tell apart.
//! - `tikv-client` 0.4.0 can't get past a dead TiKV. It keeps asking the leader it has cached, and
//!   a connection that fails gives it no answer to learn from, so a server could stay down for
//!   good while the two machines left had already elected new leaders. tikv/client-rust#562 tries
//!   the other copies, and Spare Construct's checklist now kills the node holding every leader.
//! - PD and TiKV stop on a hang-up signal even under `nohup`: run them under a service manager.
//! - `tikv-client` gives a read up after ten tries over 1.5 s. TiKV takes longer than that to get
//!   past a process or a machine that died, so a read in that time failed and the server passed
//!   the failure on to a person: a page answered "Something went wrong.", and a live channel
//!   closed (issue #762). A snapshot now waits until TiKV is past it: see [`READ_WAIT`].
//! - Two writers of the same rows at once can cross. `tikv-client` prewrites a transaction in
//!   16 KiB batches side by side, so one writer can lock a row of the other's while the other
//!   locks the primary (the first key written) that both name, as two passes of one project's
//!   regeneration do: a server's own and a page read's catching up. One then meets a lock whose
//!   primary holds no lock of its writer, and `tikv-client` gives up on such a lock with
//!   `TxnNotFound` after 1.5 s, before the lock's time to live has run out. That went on to a
//!   person as "Something went wrong." when the computer was busy enough for the batches to
//!   cross. A read that meets one now reads again ([`NO_PRIMARY_TRIES`]), and a commit that meets
//!   one is rolled back and reported as a conflict, which every caller runs again.

use futures::TryStreamExt;
use kit_store::{FileStore, Key, Kv, Pair, Prefix, Reader, StoreError, Txn};
use object_store::path::Path;
use object_store::prefix::PrefixStore;
use object_store::{ObjectStore, ObjectStoreExt};
use std::sync::Arc;
use tikv_client::{
    Backoff, CheckLevel, Error, RetryOptions, Snapshot, Timestamp, TimestampExt, Transaction,
    TransactionClient, TransactionOptions,
};
use url::Url;

const PAGE: u32 = 2048;

/// How long a snapshot's read waits for TiKV to answer, for a region with no leader and for a
/// lock alike: 2 ms doubling to 0.5 s, then every 0.5 s, 67 tries and 30 s in all. Each try asks
/// again, so a read goes on as soon as TiKV can answer it.
///
/// What it waits out:
/// - A writer that died between the two phases of its commit leaves its locks until their time
///   to live runs out: 3 s for a small transaction, 20 s at most (`DEFAULT_LOCK_TTL` and
///   `MAX_TTL` in `tikv-client`).
/// - The regions a dead machine led have no leader until the machines left elect one. A
///   follower calls an election after 10 to 20 s without word from its leader (TiKV's
///   `raft-election-timeout-ticks` of 10 one-second ticks, randomised up to twice that), and the
///   reads came back 15 and 18 s after a kill on three nodes on a development machine.
///
/// client-go, the client TiDB uses, gives a get or a scan 20 s, which an election near its
/// longest can outlast. 30 s covers the longest election with room for the client to learn the
/// new leader.
///
/// Only snapshots: writes go through the orderer, which tries again for itself and then hands
/// the edit back to its client to send again.
const READ_WAIT: RetryOptions = RetryOptions {
    region_backoff: Backoff::no_jitter_backoff(2, 500, 67),
    lock_backoff: Backoff::no_jitter_backoff(2, 500, 67),
};

/// How many times in all a read is tried while it meets a lock whose primary holds no lock of
/// its writer and no record of it (TiKV's `TxnNotFound`): a writer between the batches of its
/// prewrite, behind another writer's batches (see the lessons above), or one that died there.
///
/// `tikv-client` asks after that primary for a fixed 1.51 s (its `OPTIMISTIC_BACKOFF`, ten waits
/// from 2 ms to 0.5 s, whatever a transaction's options say) and then gives the read up, though
/// the lock lives for 3 s, or up to 20 s for a large transaction (`DEFAULT_LOCK_TTL` and
/// `MAX_TTL`), and is cleared by the first try after that. Each try waits at least those 1.51 s,
/// so 20 tries wait over 30 s, past the longest lock with room to spare. Reading again is always
/// safe: a read changes nothing.
const NO_PRIMARY_TRIES: u32 = 20;

/// A connection to TiKV. Cheap to clone.
#[derive(Clone)]
pub struct Tikv {
    client: TransactionClient,
}

fn is_conflict(error: &Error) -> bool {
    match error {
        Error::KeyError(key_error) => key_error.conflict.is_some(),
        Error::MultipleKeyErrors(errors) => errors.iter().any(is_conflict),
        _ => false,
    }
}

/// A lock in the way named a primary that holds no lock of its writer and no record of it.
fn names_a_missing_primary(error: &Error) -> bool {
    match error {
        Error::TxnNotFound(_) => true,
        Error::KeyError(key_error) => key_error.txn_not_found.is_some(),
        Error::MultipleKeyErrors(errors) => errors.iter().any(names_a_missing_primary),
        _ => false,
    }
}

/// Sorts TiKV's errors into the three cases that call for different handling.
fn classify(error: Error) -> StoreError {
    match &error {
        Error::UndeterminedError(_) => StoreError::Undetermined(error.to_string()),
        _ if is_conflict(&error) => StoreError::Conflict,
        _ => StoreError::Unavailable(error.to_string()),
    }
}

impl Tikv {
    pub async fn connect(pd: &[String]) -> Result<Self, StoreError> {
        let client = TransactionClient::new(pd.to_vec())
            .await
            .map_err(classify)?;
        Ok(Self { client })
    }

    /// Lets TiKV drop versions older than this many milliseconds. The server calls it on a timer.
    pub async fn collect_garbage(&self, keep_ms: u64) -> Result<(), StoreError> {
        let now = self.client.current_timestamp().await.map_err(classify)?;
        let safepoint = Timestamp::from_version(now.version() - (keep_ms << 18));
        self.client.gc(safepoint).await.map_err(classify)?;
        Ok(())
    }
}

/// A read-only view at one moment.
pub struct TikvSnapshot(Snapshot);

/// A transaction.
pub struct TikvTxn(Transaction);

macro_rules! reader {
    ($name:ident) => {
        impl $name {
            /// Every pair in a range, a page at a time. A page that meets a lock with no primary
            /// is read again, [`NO_PRIMARY_TRIES`] times in all for the whole range.
            async fn range(
                &mut self,
                mut from: Vec<u8>,
                to: Vec<u8>,
            ) -> Result<Vec<Pair>, StoreError> {
                let mut pairs = Vec::new();
                let mut tries = 1;
                loop {
                    let rows = match self.0.scan(from.clone()..to.clone(), PAGE).await {
                        Ok(rows) => rows,
                        Err(error)
                            if tries < NO_PRIMARY_TRIES && names_a_missing_primary(&error) =>
                        {
                            tries += 1;
                            continue;
                        }
                        Err(error) => return Err(classify(error)),
                    };
                    let rows: Vec<_> = rows.collect();
                    let last_page = rows.len() < PAGE as usize;
                    for pair in rows {
                        let key: Vec<u8> = pair.0.into();
                        from.clone_from(&key);
                        from.push(0);
                        let text = String::from_utf8(key)
                            .map_err(|e| StoreError::Damaged(e.to_string()))?;
                        pairs.push((Key::from_stored(text), pair.1));
                    }
                    if last_page {
                        return Ok(pairs);
                    }
                }
            }
        }

        impl Reader for $name {
            /// One key, read as a batch of one: `tikv-client`'s `get` keeps its own 1.5 s for a
            /// region with no leader whatever the options say, and `batch_get` follows them. A
            /// read that meets a lock with no primary is read again, [`NO_PRIMARY_TRIES`] times in
            /// all.
            async fn get(&mut self, key: &Key) -> Result<Option<Vec<u8>>, StoreError> {
                let mut tries = 1;
                let mut found = loop {
                    match self.0.batch_get([key.as_bytes().to_vec()]).await {
                        Ok(found) => break found,
                        Err(error)
                            if tries < NO_PRIMARY_TRIES && names_a_missing_primary(&error) =>
                        {
                            tries += 1;
                        }
                        Err(error) => return Err(classify(error)),
                    }
                };
                Ok(found.next().map(|pair| pair.1))
            }

            async fn scan(&mut self, prefix: &Prefix) -> Result<Vec<Pair>, StoreError> {
                let (from, to) = prefix.bounds();
                self.range(from, to).await
            }

            async fn scan_between(
                &mut self,
                from: &Key,
                to: &Key,
            ) -> Result<Vec<Pair>, StoreError> {
                self.range(from.as_bytes().to_vec(), to.as_bytes().to_vec())
                    .await
            }
        }
    };
}

reader!(TikvSnapshot);
reader!(TikvTxn);

impl Txn for TikvTxn {
    async fn put(&mut self, key: Key, value: Vec<u8>) -> Result<(), StoreError> {
        self.0
            .put(key.as_bytes().to_vec(), value)
            .await
            .map_err(classify)
    }

    async fn delete(&mut self, key: Key) -> Result<(), StoreError> {
        self.0
            .delete(key.as_bytes().to_vec())
            .await
            .map_err(classify)
    }

    /// A commit that fails is rolled back, unless it may have landed. `tikv-client` prewrites a
    /// transaction in batches of 16 KiB, side by side, and a batch that loses a write conflict
    /// fails the commit while the others keep the locks they wrote; it hands them back to the
    /// caller, whose `rollback` clears them (`Transaction::rollback`: "a transaction that already
    /// started committing may have placed prewrite (2PC) locks"), as client-go's committer does
    /// for itself. Left alone, a read that meets one asks after a primary that was never locked,
    /// and fails with `TxnNotFound` until the lock's time to live runs out.
    ///
    /// If the rollback fails as well, the locks are left to run out as a dead writer's are, and
    /// the commit's own error is still the one to act on, so the rollback's is not returned.
    ///
    /// A commit whose prewrite stopped at a lock with no primary (see [`NO_PRIMARY_TRIES`]) is a
    /// conflict: another writer holds that lock, this commit's primary was never committed, so
    /// nothing of it landed, and run again it lands once that writer has finished or its lock has
    /// run out. Two passes of one project's regeneration met this when their batches crossed (see the
    /// lessons above): the one holding the primary both name failed with `TxnNotFound` as if the
    /// store were down, and the page read that ran it failed.
    async fn commit(mut self) -> Result<(), StoreError> {
        let error = match self.0.commit().await {
            Ok(_) => return Ok(()),
            Err(error) => error,
        };
        if matches!(error, Error::UndeterminedError(_)) {
            return Err(classify(error));
        }
        let _ = self.0.rollback().await;
        if names_a_missing_primary(&error) {
            return Err(StoreError::Conflict);
        }
        Err(classify(error))
    }

    async fn rollback(mut self) -> Result<(), StoreError> {
        self.0.rollback().await.map_err(classify)
    }
}

impl Kv for Tikv {
    type Snapshot = TikvSnapshot;
    type Txn = TikvTxn;

    async fn snapshot(&self) -> Result<TikvSnapshot, StoreError> {
        let at = self.client.current_timestamp().await.map_err(classify)?;
        let options = TransactionOptions::new_optimistic()
            .read_only()
            .retry_options(READ_WAIT);
        Ok(TikvSnapshot(self.client.snapshot(at, options)))
    }

    async fn begin(&self) -> Result<TikvTxn, StoreError> {
        let options = TransactionOptions::new_optimistic().drop_check(CheckLevel::None);
        Ok(TikvTxn(
            self.client
                .begin_with_options(options)
                .await
                .map_err(classify)?,
        ))
    }
}

/// Files, through `object_store`. Which provider is a matter of settings.
#[derive(Clone)]
pub struct Files {
    store: Arc<dyn ObjectStore>,
}

impl Files {
    /// Opens a store from its address: `s3://bucket`, `gs://bucket` or `file:///folder`.
    /// Credentials come from the usual `AWS_` and `GOOGLE_` settings.
    pub fn open(address: &str) -> Result<Self, StoreError> {
        let bad = |e: &dyn std::fmt::Display| StoreError::Unavailable(format!("{address}: {e}"));
        let url = Url::parse(address).map_err(|e| bad(&e))?;
        let options = std::env::vars()
            .filter(|(name, _)| name.starts_with("AWS_") || name.starts_with("GOOGLE_"))
            .map(|(name, value)| (name.to_lowercase(), value));
        let (store, prefix) = object_store::parse_url_opts(&url, options).map_err(|e| bad(&e))?;
        if prefix.as_ref().is_empty() {
            return Ok(Self {
                store: Arc::from(store),
            });
        }
        Ok(Self {
            store: Arc::new(PrefixStore::new(store, prefix)),
        })
    }
}

fn failed(error: object_store::Error) -> StoreError {
    StoreError::Unavailable(error.to_string())
}

impl FileStore for Files {
    async fn put(&self, path: &str, bytes: Vec<u8>) -> Result<(), StoreError> {
        self.store
            .put(&Path::from(path), bytes.into())
            .await
            .map_err(failed)?;
        Ok(())
    }

    async fn get(&self, path: &str) -> Result<Vec<u8>, StoreError> {
        let file = self.store.get(&Path::from(path)).await.map_err(failed)?;
        Ok(file.bytes().await.map_err(failed)?.to_vec())
    }

    async fn list(&self) -> Result<Vec<String>, StoreError> {
        let found: Vec<_> = self.store.list(None).try_collect().await.map_err(failed)?;
        Ok(found
            .into_iter()
            .map(|object| object.location.to_string())
            .collect())
    }
}
