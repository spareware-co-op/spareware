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

use futures::TryStreamExt;
use kit_store::{FileStore, Key, Kv, Pair, Prefix, Reader, StoreError, Txn};
use object_store::path::Path;
use object_store::prefix::PrefixStore;
use object_store::{ObjectStore, ObjectStoreExt};
use std::sync::Arc;
use tikv_client::{
    CheckLevel, Error, Snapshot, Timestamp, TimestampExt, Transaction, TransactionClient,
    TransactionOptions,
};
use url::Url;

const PAGE: u32 = 2048;

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
            /// Every pair in a range, a page at a time.
            async fn range(
                &mut self,
                mut from: Vec<u8>,
                to: Vec<u8>,
            ) -> Result<Vec<Pair>, StoreError> {
                let mut pairs = Vec::new();
                loop {
                    let rows = self
                        .0
                        .scan(from.clone()..to.clone(), PAGE)
                        .await
                        .map_err(classify)?;
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
            async fn get(&mut self, key: &Key) -> Result<Option<Vec<u8>>, StoreError> {
                self.0.get(key.as_bytes().to_vec()).await.map_err(classify)
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

    async fn commit(mut self) -> Result<(), StoreError> {
        self.0.commit().await.map(|_| ()).map_err(classify)
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
        let options = TransactionOptions::new_optimistic().read_only();
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
