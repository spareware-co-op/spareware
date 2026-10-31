//! Backup and restore: every key from one snapshot, plus every file, written somewhere else.
//! The format is plain gzip'd JSON lines, readable without this program. A real cloud would use
//! TiKV's own `br` tool for speed; this is enough to prove a server can be lost and rebuilt.
//!
//! These are operators' tools, run on the server with the store's own credentials, so they go
//! round the edit log by design (note K08). `restore` refuses a store that holds anything.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use kit_ops::sha256_hex;
use kit_store::{FileStore, Key, Kv, Prefix, Reader, StoreError, Txn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};

const RESTORE_BATCH: usize = 256;

#[derive(Serialize, Deserialize)]
struct Line {
    k: String,
    v: String,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    pairs: usize,
    kv_sha256: String,
    files: Vec<(String, String)>,
}

fn damaged(e: impl std::fmt::Display) -> StoreError {
    StoreError::Damaged(e.to_string())
}

pub async fn backup<K: Kv, F: FileStore, T: FileStore>(
    kv: &K,
    files: &F,
    target: &T,
    name: &str,
) -> Result<(), StoreError> {
    let mut snapshot = kv.snapshot().await?;
    let pairs = snapshot.scan(&Prefix::everything()).await?;
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    for (key, value) in &pairs {
        let line = Line {
            k: key.as_str().to_owned(),
            v: STANDARD.encode(value),
        };
        serde_json::to_writer(&mut gz, &line).map_err(damaged)?;
        gz.write_all(b"\n").map_err(damaged)?;
    }
    let bytes = gz.finish().map_err(damaged)?;
    let kv_sha256 = sha256_hex(&bytes);
    target.put(&format!("{name}/kv.jsonl.gz"), bytes).await?;

    let mut copied = Vec::new();
    for path in files.list().await? {
        let bytes = files.get(&path).await?;
        copied.push((path.clone(), sha256_hex(&bytes)));
        target.put(&format!("{name}/files/{path}"), bytes).await?;
    }

    let manifest = Manifest {
        pairs: pairs.len(),
        kv_sha256,
        files: copied,
    };
    let body = serde_json::to_vec_pretty(&manifest).map_err(damaged)?;
    target.put(&format!("{name}/manifest.json"), body).await?;
    println!(
        "backup {name}: {} keys, {} files",
        manifest.pairs,
        manifest.files.len()
    );
    Ok(())
}

/// Restores into an empty store only, so it can never overwrite live work.
pub async fn restore<K: Kv, F: FileStore, S: FileStore>(
    kv: &K,
    files: &F,
    source: &S,
    name: &str,
) -> Result<(), StoreError> {
    let mut snapshot = kv.snapshot().await?;
    let existing = snapshot.scan(&Prefix::everything()).await?;
    if !existing.is_empty() {
        return Err(StoreError::Damaged(format!(
            "the store already holds {} keys; restore needs an empty one",
            existing.len()
        )));
    }
    let manifest = source.get(&format!("{name}/manifest.json")).await?;
    let manifest: Manifest = serde_json::from_slice(&manifest).map_err(damaged)?;
    let bytes = source.get(&format!("{name}/kv.jsonl.gz")).await?;
    if sha256_hex(&bytes) != manifest.kv_sha256 {
        return Err(damaged(format!(
            "backup {name}: its key file doesn't match the manifest"
        )));
    }

    let mut lines = Vec::new();
    for line in BufReader::new(GzDecoder::new(&bytes[..])).lines() {
        lines.push(serde_json::from_str::<Line>(&line.map_err(damaged)?).map_err(damaged)?);
    }
    if lines.len() != manifest.pairs {
        return Err(damaged(format!(
            "backup {name}: {} keys, manifest says {}",
            lines.len(),
            manifest.pairs
        )));
    }
    for chunk in lines.chunks(RESTORE_BATCH) {
        let mut txn = kv.begin().await?;
        for line in chunk {
            let value = STANDARD.decode(&line.v).map_err(damaged)?;
            txn.put(Key::from_stored(line.k.clone()), value).await?;
        }
        txn.commit().await?;
    }

    for (path, sha256) in &manifest.files {
        let bytes = source.get(&format!("{name}/files/{path}")).await?;
        if sha256_hex(&bytes) != *sha256 {
            return Err(damaged(format!(
                "backup {name}: {path} doesn't match the manifest"
            )));
        }
        files.put(path, bytes).await?;
    }
    println!(
        "restored {name}: {} keys, {} files",
        manifest.pairs,
        manifest.files.len()
    );
    Ok(())
}

/// One hash over every key, value and file, to compare two stores.
pub async fn digest<K: Kv, F: FileStore>(kv: &K, files: &F) -> Result<String, StoreError> {
    let mut snapshot = kv.snapshot().await?;
    let mut hash = Sha256::new();
    for (key, value) in snapshot.scan(&Prefix::everything()).await? {
        hash.update((key.as_bytes().len() as u64).to_le_bytes());
        hash.update(key.as_bytes());
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(&value);
    }
    let mut paths = files.list().await?;
    paths.sort();
    for path in paths {
        hash.update(path.as_bytes());
        hash.update(files.get(&path).await?);
    }
    Ok(hex::encode(hash.finalize()))
}
