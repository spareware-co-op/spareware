//! Something worked out from an element's record and the rows it read, kept with those rows so it
//! is worked out again only when one of them changed: what a pass of the regenerator reads a
//! neighbouring wall by, again and again, as each wall near it is built ([`crate::joins`],
//! [`crate::rooms`]).
//!
//! What it promises:
//! - [`Recording`] is a [`Reader`] that reads through another and notes every row it was given
//!   and every row that was missing. A record named by its content's hash (a `record` row) is not
//!   noted: its key says what it holds, so it cannot change.
//! - [`Memo::kept`] gives what was kept for an element only when the element's record is the one
//!   it was worked out from and every noted row reads the same now, byte for byte; else nothing.
//!   So a value it gives is the one working it out again would give, as long as the work reads
//!   the store and nothing else (no clock, no random number), as every reader of this crate does.
//!   Work that scanned a range is not kept ([`Recording::noted`]), since a range can gain a row.
//! - Given a [`Written`], the log of the rows a pass of the regenerator wrote to the kind index,
//!   it reads the noted rows again only when one of them was written since they were last read,
//!   for work that read no row but the kind index's (and records): the kind index is written by
//!   the regenerator alone, and each of its writes is logged, so a row not logged since reads as
//!   it did. Work that read any other row is read again each time.
//!
//! Depends on construct-model (records and keys) and kit-store. Nothing here is interim.

use construct_model::element::Element;
use construct_model::keys::Place as ModelPlace;
use kit_core::Id;
use kit_store::{Key, Pair, Place, Prefix, Reader, StoreError};
use std::collections::BTreeMap;

/// The rows a piece of work read, each with what it held (`None`: missing).
pub type Noted = Vec<(Key, Option<Vec<u8>>)>;

/// A reader that notes what it reads (the module header).
pub struct Recording<'a, R> {
    inner: &'a mut R,
    noted: Noted,
    scanned: bool,
}

impl<'a, R: Reader> Recording<'a, R> {
    /// A reader that reads through `inner` and notes each row.
    pub fn new(inner: &'a mut R) -> Self {
        Self {
            inner,
            noted: Vec::new(),
            scanned: false,
        }
    }

    /// The rows read, or `None` when a range was scanned.
    #[must_use]
    pub fn noted(self) -> Option<Noted> {
        (!self.scanned).then_some(self.noted)
    }
}

/// Whether a row's key names its content: a record by its hash.
fn fixed(key: &Key) -> bool {
    matches!(key.parse(), Some(Place::Record { .. }))
}

/// Whether a row is one of the kind index's, which only the regenerator writes.
fn indexed(key: &Key) -> bool {
    matches!(ModelPlace::parse(key), Some(ModelPlace::Kind { .. }))
}

impl<R: Reader> Reader for Recording<'_, R> {
    async fn get(&mut self, key: &Key) -> Result<Option<Vec<u8>>, StoreError> {
        let value = self.inner.get(key).await?;
        if !fixed(key) {
            self.noted.push((key.clone(), value.clone()));
        }
        Ok(value)
    }

    async fn scan(&mut self, prefix: &Prefix) -> Result<Vec<Pair>, StoreError> {
        self.scanned = true;
        self.inner.scan(prefix).await
    }

    async fn scan_between(&mut self, from: &Key, to: &Key) -> Result<Vec<Pair>, StoreError> {
        self.scanned = true;
        self.inner.scan_between(from, to).await
    }
}

/// The rows of the kind index a pass wrote, each with when: a count of the writes so far.
#[derive(Clone, Debug, Default)]
pub struct Written {
    now: u64,
    rows: BTreeMap<Key, u64>,
}

impl Written {
    /// The kind index's row `key` is written.
    pub fn wrote(&mut self, key: Key) {
        self.now = self.now.saturating_add(1);
        self.rows.insert(key, self.now);
    }

    /// Whether `key` was written after the count `at`.
    fn since(&self, key: &Key, at: u64) -> bool {
        self.rows.get(key).is_some_and(|when| *when > at)
    }
}

/// What was worked out for an element, with its record and the rows the work read.
#[derive(Clone, Debug)]
struct Entry<T> {
    record: Element,
    noted: Noted,
    /// Whether every noted row is the kind index's.
    indexed: bool,
    /// The count of writes when the noted rows were last read.
    checked: u64,
    value: T,
}

/// Values worked out for elements, each kept with what it read (the module header).
#[derive(Clone, Debug)]
pub struct Memo<T> {
    entries: BTreeMap<Id, Entry<T>>,
}

impl<T> Default for Memo<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl<T: Clone> Memo<T> {
    /// The value kept for `record`'s element, if it was worked out from this very record and every
    /// row its work read reads the same through `reader` now: read again unless `written` says
    /// none of them was written since they were last read.
    ///
    /// # Errors
    /// A [`StoreError`] when the store fails.
    pub async fn kept<R: Reader>(
        &mut self,
        reader: &mut R,
        record: &Element,
        written: Option<&Written>,
    ) -> Result<Option<T>, StoreError> {
        let Some(entry) = self.entries.get_mut(&record.id) else {
            return Ok(None);
        };
        if entry.record != *record {
            return Ok(None);
        }
        if let Some(written) = written
            && entry.indexed
            && !entry
                .noted
                .iter()
                .any(|(key, _)| written.since(key, entry.checked))
        {
            return Ok(Some(entry.value.clone()));
        }
        for (key, value) in &entry.noted {
            if reader.get(key).await? != *value {
                return Ok(None);
            }
        }
        if let Some(written) = written {
            entry.checked = written.now;
        }
        Ok(Some(entry.value.clone()))
    }

    /// Keeps `value`, worked out from `record` having read `noted`, at the count of writes in
    /// `written`; work that scanned a range (`None`) is not kept, and whatever was kept for the
    /// element goes.
    pub fn keep(
        &mut self,
        record: &Element,
        noted: Option<Noted>,
        value: T,
        written: Option<&Written>,
    ) {
        match noted {
            Some(noted) => {
                let indexed = noted.iter().all(|(key, _)| indexed(key));
                self.entries.insert(
                    record.id.clone(),
                    Entry {
                        record: record.clone(),
                        noted,
                        indexed,
                        checked: written.map_or(0, |w| w.now),
                        value,
                    },
                );
            }
            None => {
                self.entries.remove(&record.id);
            }
        }
    }

    /// Forgets what was kept for element `id`.
    pub fn forget(&mut self, id: &Id) {
        self.entries.remove(id);
    }
}
