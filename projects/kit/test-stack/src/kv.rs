//! The one door to TiKV. Writes go through optimistic transactions, so two writers of a key can
//! never both win; reads go through a snapshot at one moment in time.

use anyhow::{Context, Result};
use serde::{Serialize, de::DeserializeOwned};
use std::ops::Range;
use tikv_client::{
    CheckLevel, Snapshot, Timestamp, TimestampExt, Transaction, TransactionClient,
    TransactionOptions,
};

const PAGE: u32 = 2048;

#[derive(Clone)]
pub struct Kv {
    client: TransactionClient,
}

/// The byte range of every key starting with `prefix`.
pub fn prefix_range(prefix: &str) -> Range<Vec<u8>> {
    let start = prefix.as_bytes().to_vec();
    let mut end = start.clone();
    *end.last_mut().expect("prefixes are never empty") += 1;
    start..end
}

impl Kv {
    pub async fn connect(pd: &[String]) -> Result<Self> {
        let client = TransactionClient::new(pd.to_vec())
            .await
            .context("connecting to TiKV")?;
        Ok(Self { client })
    }

    pub async fn begin(&self) -> Result<Transaction> {
        // A transaction dropped after a failed commit is normal here: the caller retries.
        let options = TransactionOptions::new_optimistic().drop_check(CheckLevel::None);
        Ok(self.client.begin_with_options(options).await?)
    }

    pub async fn snapshot(&self) -> Result<Snapshot> {
        let at = self.client.current_timestamp().await?;
        Ok(self
            .client
            .snapshot(at, TransactionOptions::new_optimistic().read_only()))
    }

    pub async fn get<T: DeserializeOwned>(&self, key: String) -> Result<Option<T>> {
        let mut snap = self.snapshot().await?;
        get(&mut snap, key).await
    }

    pub async fn scan(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>> {
        let mut snap = self.snapshot().await?;
        scan(&mut snap, prefix).await
    }

    pub async fn scan_rows<T: DeserializeOwned>(&self, prefix: &str) -> Result<Vec<T>> {
        let mut snap = self.snapshot().await?;
        scan_rows(&mut snap, prefix).await
    }

    /// Writes one row in its own transaction.
    pub async fn put<T: Serialize>(&self, key: String, value: &T) -> Result<()> {
        let mut txn = self.begin().await?;
        txn.put(key, serde_json::to_vec(value)?).await?;
        txn.commit().await?;
        Ok(())
    }

    pub async fn delete(&self, key: String) -> Result<()> {
        let mut txn = self.begin().await?;
        txn.delete(key).await?;
        txn.commit().await?;
        Ok(())
    }

    /// Lets TiKV drop old versions of rewritten keys. TiDB normally does this for TiKV; without
    /// it, every commit's old `head` value stays on disk for ever.
    pub async fn collect_garbage(&self, keep_ms: u64) -> Result<()> {
        let now = self.client.current_timestamp().await?;
        let safepoint = Timestamp::from_version(now.version() - (keep_ms << 18));
        self.client.gc(safepoint).await?;
        Ok(())
    }
}

pub async fn get<T: DeserializeOwned>(snap: &mut Snapshot, key: String) -> Result<Option<T>> {
    match snap.get(key).await? {
        Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        None => Ok(None),
    }
}

pub async fn scan_rows<T: DeserializeOwned>(snap: &mut Snapshot, prefix: &str) -> Result<Vec<T>> {
    let rows = scan(snap, prefix).await?;
    rows.iter()
        .map(|(_, value)| Ok(serde_json::from_slice(value)?))
        .collect()
}

pub async fn scan(snap: &mut Snapshot, prefix: &str) -> Result<Vec<(String, Vec<u8>)>> {
    let range = prefix_range(prefix);
    let mut rows = Vec::new();
    for (key, value) in scan_range(snap, range).await? {
        rows.push((String::from_utf8(key)?, value));
    }
    Ok(rows)
}

/// Every pair in a byte range, page by page.
pub async fn scan_range(
    snap: &mut Snapshot,
    range: Range<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
    let mut rows = Vec::new();
    let mut from = range.start;
    loop {
        let page: Vec<_> = snap
            .scan(from.clone()..range.end.clone(), PAGE)
            .await?
            .collect();
        let last_page = page.len() < PAGE as usize;
        for pair in page {
            let (key, value): (Vec<u8>, Vec<u8>) = (pair.0.into(), pair.1);
            rows.push((key, value));
        }
        match rows.last() {
            Some((key, _)) if !last_page => {
                from = key.clone();
                from.push(0);
            }
            _ => return Ok(rows),
        }
    }
}
