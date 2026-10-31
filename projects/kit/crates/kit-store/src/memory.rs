//! An in-process store, for tests only: nothing that ships turns on the `memory` feature. It keeps
//! the two promises [`Kv`] asks for (snapshot reads, and no two writers of a key both winning) and
//! adds what a real database can't do on demand: fail a chosen commit in a chosen way.

use crate::key::{Key, Prefix};
use crate::kv::{FileStore, Kv, Pair, Reader, StoreError, Txn};
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

/// What the next commit does instead of succeeding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Reports a conflict. Nothing lands.
    Conflict,
    /// Fails and says so. Nothing lands.
    Fails,
    /// Lands, but the answer is lost: the worst case a real database has.
    LandsAnswerLost,
}

/// Every version of every key.
#[derive(Default)]
struct Rows {
    versions: BTreeMap<String, Vec<(u64, Option<Vec<u8>>)>>,
    clock: u64,
    faults: VecDeque<Fault>,
    down: bool,
}

impl Rows {
    fn read(&self, key: &str, at: u64) -> Option<Vec<u8>> {
        let versions = self.versions.get(key)?;
        versions.iter().rev().find(|(ts, _)| *ts <= at)?.1.clone()
    }

    fn scan(&self, from: &[u8], to: &[u8], at: u64) -> Vec<Pair> {
        let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
        self.versions
            .range(text(from)..text(to))
            .filter_map(|(key, _)| Some((Key::from_stored(key.clone()), self.read(key, at)?)))
            .collect()
    }
}

#[derive(Clone, Default)]
pub struct Memory {
    rows: Arc<Mutex<Rows>>,
}

impl Memory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn rows(&self) -> MutexGuard<'_, Rows> {
        self.rows.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Makes one coming commit fail. Faults are used up in the order they were added.
    pub fn inject(&self, fault: Fault) {
        self.rows().faults.push_back(fault);
    }

    /// Takes the store down, or brings it back.
    pub fn set_down(&self, down: bool) {
        self.rows().down = down;
    }

    /// Overwrites a stored value, as a damaged disk or a bad hand would.
    pub fn tamper(&self, key: &Key, value: Option<Vec<u8>>) {
        let mut rows = self.rows();
        rows.clock += 1;
        let at = rows.clock;
        rows.versions.entry(key.as_str().to_owned()).or_default().push((at, value));
    }

    /// Every live key, for tests that look at the whole store.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        let rows = self.rows();
        let at = rows.clock;
        rows.versions.keys().filter(|key| rows.read(key, at).is_some()).cloned().collect()
    }
}

pub struct View {
    store: Memory,
    at: u64,
}

impl View {
    fn open(store: &Memory) -> Result<Self, StoreError> {
        let rows = store.rows();
        if rows.down {
            return Err(StoreError::Unavailable("the store is down".to_owned()));
        }
        let at = rows.clock;
        drop(rows);
        Ok(Self { store: store.clone(), at })
    }
}

impl Reader for View {
    async fn get(&mut self, key: &Key) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.store.rows().read(key.as_str(), self.at))
    }

    async fn scan(&mut self, prefix: &Prefix) -> Result<Vec<Pair>, StoreError> {
        let (from, to) = prefix.bounds();
        Ok(self.store.rows().scan(&from, &to, self.at))
    }

    async fn scan_between(&mut self, from: &Key, to: &Key) -> Result<Vec<Pair>, StoreError> {
        Ok(self.store.rows().scan(from.as_bytes(), to.as_bytes(), self.at))
    }
}

pub struct MemoryTxn {
    view: View,
    writes: BTreeMap<String, Option<Vec<u8>>>,
}

impl Reader for MemoryTxn {
    async fn get(&mut self, key: &Key) -> Result<Option<Vec<u8>>, StoreError> {
        match self.writes.get(key.as_str()) {
            Some(value) => Ok(value.clone()),
            None => self.view.get(key).await,
        }
    }

    async fn scan(&mut self, prefix: &Prefix) -> Result<Vec<Pair>, StoreError> {
        let (from, to) = prefix.bounds();
        let (from, to) = (
            Key::from_stored(String::from_utf8_lossy(&from).into_owned()),
            Key::from_stored(String::from_utf8_lossy(&to).into_owned()),
        );
        self.scan_between(&from, &to).await
    }

    async fn scan_between(&mut self, from: &Key, to: &Key) -> Result<Vec<Pair>, StoreError> {
        let mut pairs: BTreeMap<String, Vec<u8>> = self
            .view
            .scan_between(from, to)
            .await?
            .into_iter()
            .map(|(key, value)| (key.as_str().to_owned(), value))
            .collect();
        for (key, value) in self.writes.range(from.as_str().to_owned()..to.as_str().to_owned()) {
            match value {
                Some(value) => pairs.insert(key.clone(), value.clone()),
                None => pairs.remove(key),
            };
        }
        Ok(pairs.into_iter().map(|(key, value)| (Key::from_stored(key), value)).collect())
    }
}

impl Txn for MemoryTxn {
    async fn put(&mut self, key: Key, value: Vec<u8>) -> Result<(), StoreError> {
        self.writes.insert(key.as_str().to_owned(), Some(value));
        Ok(())
    }

    async fn delete(&mut self, key: Key) -> Result<(), StoreError> {
        self.writes.insert(key.as_str().to_owned(), None);
        Ok(())
    }

    async fn commit(self) -> Result<(), StoreError> {
        let mut rows = self.view.store.rows();
        if rows.down {
            return Err(StoreError::Unavailable("the store is down".to_owned()));
        }
        let fault = rows.faults.pop_front();
        match fault {
            Some(Fault::Conflict) => return Err(StoreError::Conflict),
            Some(Fault::Fails) => return Err(StoreError::Unavailable("injected".to_owned())),
            _ => {}
        }
        let at = self.view.at;
        let conflict = self.writes.keys().any(|key| {
            rows.versions.get(key).and_then(|v| v.last()).is_some_and(|(ts, _)| *ts > at)
        });
        if conflict {
            return Err(StoreError::Conflict);
        }
        rows.clock += 1;
        let now = rows.clock;
        for (key, value) in self.writes {
            rows.versions.entry(key).or_default().push((now, value));
        }
        match fault {
            Some(Fault::LandsAnswerLost) => {
                Err(StoreError::Undetermined("injected: the answer was lost".to_owned()))
            }
            _ => Ok(()),
        }
    }

    async fn rollback(self) -> Result<(), StoreError> {
        Ok(())
    }
}

impl Kv for Memory {
    type Snapshot = View;
    type Txn = MemoryTxn;

    async fn snapshot(&self) -> Result<View, StoreError> {
        View::open(self)
    }

    async fn begin(&self) -> Result<MemoryTxn, StoreError> {
        Ok(MemoryTxn { view: View::open(self)?, writes: BTreeMap::new() })
    }
}

/// Files in memory.
#[derive(Clone, Default)]
pub struct MemoryFiles {
    files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
}

impl FileStore for MemoryFiles {
    async fn put(&self, path: &str, bytes: Vec<u8>) -> Result<(), StoreError> {
        let mut files = self.files.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        files.insert(path.to_owned(), bytes);
        Ok(())
    }

    async fn get(&self, path: &str) -> Result<Vec<u8>, StoreError> {
        let files = self.files.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        files.get(path).cloned().ok_or_else(|| StoreError::Unavailable(format!("no file {path}")))
    }

    async fn list(&self) -> Result<Vec<String>, StoreError> {
        let files = self.files.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(files.keys().cloned().collect())
    }
}
