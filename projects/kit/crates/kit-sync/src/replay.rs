//! Reads a log back, and proves what is stored follows from it. The log is the truth, so if this
//! passes, nothing has been lost. Run after every failure test, by `test-stack verify`, and after a
//! restore.

use kit_core::Id;
use kit_ops::{Entry, content_id, sha256_hex};
use kit_store::key::list;
use kit_store::{Place, Reader, Scope, StoreError, from_row, read_row};
use std::collections::BTreeMap;

/// Log entries after `from`, up to and including `to`.
pub async fn read_log<R: Reader>(
    reader: &mut R,
    scope: &Scope,
    from: u64,
    to: u64,
) -> Result<Vec<Entry>, StoreError> {
    let pairs = reader.scan_between(&scope.log(from + 1), &scope.log(to.saturating_add(1))).await?;
    pairs
        .iter()
        .map(|(_, bytes)| {
            Entry::decode(bytes).map_err(|e| StoreError::Damaged(e.to_string()))
        })
        .collect()
}

/// What a check found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogCheck {
    pub entries: u64,
    pub elements: usize,
}

/// Checks a scope: no gaps in the numbers, every content ID right, each change following from the
/// one before, and, for a project, replaying the log gives exactly the elements stored, each
/// record matching its hash.
pub async fn verify_log<R: Reader>(reader: &mut R, scope: &Scope) -> Result<LogCheck, String> {
    let fault = |e: StoreError| e.to_string();
    let head: u64 = read_row(reader, &scope.head()).await.map_err(fault)?.unwrap_or(0);
    let entries = read_log(reader, scope, 0, head).await.map_err(fault)?;
    if entries.len() as u64 != head {
        return Err(format!("{} log entries, head says {head}", entries.len()));
    }
    let mut replayed: BTreeMap<(Id, Id), Id> = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.seq != index as u64 + 1 {
            return Err(format!("log entry {} is out of place", entry.seq));
        }
        if entry.content_id != content_id(&entry.changes) {
            return Err(format!("log entry {} has the wrong content ID", entry.seq));
        }
        for change in &entry.changes {
            let key = (entry.target.clone(), change.piece.clone());
            if replayed.get(&key) != change.before.as_ref() {
                return Err(format!("log entry {} doesn't follow from the one before", entry.seq));
            }
            match &change.after {
                Some(hash) => replayed.insert(key, hash.clone()),
                None => replayed.remove(&key),
            };
        }
    }
    let Scope::Project(project) = scope else {
        return Ok(LogCheck { entries: head, elements: 0 });
    };

    let mut stored = BTreeMap::new();
    for (key, value) in reader.scan(&list::all_elements(project)).await.map_err(fault)? {
        let Some(Place::Element { model, element, .. }) = key.parse() else {
            return Err(format!("{key} isn't an element key"));
        };
        let hash: String = from_row(&value).map_err(fault)?;
        let hash = Id::parse(&hash).map_err(|e| e.to_string())?;
        stored.insert((model, element), hash);
    }
    if stored != replayed {
        return Err("the stored elements differ from the replayed log".to_owned());
    }
    for ((_, element), hash) in &stored {
        let place = Place::Record { project: project.clone(), hash: hash.clone() };
        let bytes = reader.get(&place.key()).await.map_err(fault)?;
        let bytes = bytes.ok_or_else(|| format!("the record of {element} is missing"))?;
        if sha256_hex(&bytes) != hash.as_str() {
            return Err(format!("the record of {element} doesn't match its hash"));
        }
    }
    Ok(LogCheck { entries: head, elements: stored.len() })
}
