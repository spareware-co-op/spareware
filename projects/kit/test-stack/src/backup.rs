//! Backup and restore: every key from one snapshot, plus every file, written somewhere else.
//! The format is plain gzip'd JSON lines, readable without this program. A real cloud would use
//! TiKV's own `br` tool for speed; this is enough to prove a server can be lost and rebuilt.

use crate::kv::{self, Kv};
use crate::rows::sha256_hex;
use anyhow::{Context as _, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use futures::TryStreamExt;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;

/// Every key the stack writes sorts inside this range.
fn whole_range() -> std::ops::Range<Vec<u8>> {
    b"!".to_vec()..b"~".to_vec()
}

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

pub async fn backup(
    kv: &Kv,
    files: &Arc<dyn ObjectStore>,
    target: &Arc<dyn ObjectStore>,
    name: &str,
) -> Result<()> {
    let mut snap = kv.snapshot().await?;
    let pairs = kv::scan_range(&mut snap, whole_range()).await?;
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    for (key, value) in &pairs {
        let line = Line {
            k: String::from_utf8(key.clone())?,
            v: STANDARD.encode(value),
        };
        serde_json::to_writer(&mut gz, &line)?;
        gz.write_all(b"\n")?;
    }
    let bytes = gz.finish()?;
    let kv_sha256 = sha256_hex(&bytes);
    target
        .put(&Path::from(format!("{name}/kv.jsonl.gz")), bytes.into())
        .await?;

    let mut copied = Vec::new();
    for object in files.list(None).try_collect::<Vec<_>>().await? {
        let bytes = files.get(&object.location).await?.bytes().await?;
        copied.push((object.location.to_string(), sha256_hex(&bytes)));
        let path = Path::from(format!("{name}/files/{}", object.location));
        target.put(&path, bytes.into()).await?;
    }

    let manifest = Manifest {
        pairs: pairs.len(),
        kv_sha256,
        files: copied,
    };
    let body = serde_json::to_vec_pretty(&manifest)?;
    target
        .put(&Path::from(format!("{name}/manifest.json")), body.into())
        .await?;
    println!(
        "backup {name}: {} keys, {} files",
        manifest.pairs,
        manifest.files.len()
    );
    Ok(())
}

/// Restores into an empty store only, so it can never overwrite live work.
pub async fn restore(
    kv: &Kv,
    files: &Arc<dyn ObjectStore>,
    source: &Arc<dyn ObjectStore>,
    name: &str,
) -> Result<()> {
    let mut snap = kv.snapshot().await?;
    let existing = kv::scan_range(&mut snap, whole_range()).await?;
    if !existing.is_empty() {
        bail!(
            "the store already holds {} keys; restore needs an empty one",
            existing.len()
        );
    }
    let manifest = source
        .get(&Path::from(format!("{name}/manifest.json")))
        .await?;
    let manifest: Manifest = serde_json::from_slice(&manifest.bytes().await?)?;
    let bytes = source
        .get(&Path::from(format!("{name}/kv.jsonl.gz")))
        .await?
        .bytes()
        .await?;
    if sha256_hex(&bytes) != manifest.kv_sha256 {
        bail!("backup {name} is damaged: its key file doesn't match the manifest");
    }

    let mut lines = Vec::new();
    for line in BufReader::new(GzDecoder::new(&bytes[..])).lines() {
        lines.push(serde_json::from_str::<Line>(&line?)?);
    }
    if lines.len() != manifest.pairs {
        bail!(
            "backup {name} is damaged: {} keys, manifest says {}",
            lines.len(),
            manifest.pairs
        );
    }
    for chunk in lines.chunks(RESTORE_BATCH) {
        let mut txn = kv.begin().await?;
        for line in chunk {
            txn.put(line.k.clone(), STANDARD.decode(&line.v)?).await?;
        }
        txn.commit().await?;
    }

    for (path, sha256) in &manifest.files {
        let from = Path::from(format!("{name}/files/{path}"));
        let bytes = source
            .get(&from)
            .await?
            .bytes()
            .await
            .context("reading a backed-up file")?;
        if sha256_hex(&bytes) != *sha256 {
            bail!("backup {name} is damaged: {path} doesn't match the manifest");
        }
        files.put(&Path::from(path.as_str()), bytes.into()).await?;
    }
    println!(
        "restored {name}: {} keys, {} files",
        manifest.pairs,
        manifest.files.len()
    );
    Ok(())
}

/// One hash over every key, value and file, to compare two stores.
pub async fn digest(kv: &Kv, files: &Arc<dyn ObjectStore>) -> Result<String> {
    let mut snap = kv.snapshot().await?;
    let mut hash = Sha256::new();
    for (key, value) in kv::scan_range(&mut snap, whole_range()).await? {
        hash.update((key.len() as u64).to_le_bytes());
        hash.update(&key);
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(&value);
    }
    let mut objects = files.list(None).try_collect::<Vec<_>>().await?;
    objects.sort_by(|a, b| a.location.cmp(&b.location));
    for object in objects {
        hash.update(object.location.as_ref().as_bytes());
        hash.update(files.get(&object.location).await?.bytes().await?);
    }
    Ok(hex::encode(hash.finalize()))
}
