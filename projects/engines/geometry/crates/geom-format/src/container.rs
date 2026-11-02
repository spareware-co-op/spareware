//! Containers: a document's records as blobs keyed by content ID, the way Spare Construct's SQLite
//! container stores them, and the standalone file they come from or go to (24.5).
//!
//! What it does: a program's store implements [`Blobs`]: the bytes of one whole record, flags and
//! check included, under the content ID of its payload. A document is its **document blob**, the
//! header record (with its frames, once frame records exist), and its **roots**, the records
//! nothing else in it depends on, in order. [`read_record`] reads one blob back as a record and
//! checks it against its key, so every lookup verifies itself. [`Container`] is the reference
//! container, in memory: [`Container::from_file`] takes a file apart into blobs,
//! [`Container::to_file`] writes the standalone file again, and [`Container::salvage`] writes every
//! intact record of a damaged container out as a standalone file and names the damaged blobs.
//!
//! Guarantees:
//!
//! - **A file and back gives the same bytes.** [`Container::to_file`] writes each root after the
//!   records it depends on, depth first and each record once: a body's chunks in its index's order,
//!   then the index; any other record's dependencies in `depends` order. A file written in that
//!   order, as the body writer's are, comes back byte for byte; any file comes back as the same
//!   records, each byte for byte.
//! - **A container and back gives the same blobs.** Every record is stored as its canonical bytes
//!   under its own content ID, and a file read back from the container gives exactly those blobs.
//! - **Every lookup verifies itself.** A blob is refused unless it is one canonical record, its
//!   check matches and its payload's content ID is the key it was stored under.
//! - **Damage stays local.** [`Container::salvage`] keeps every intact record and lists each
//!   missing or damaged blob with its error; only a damaged document blob stops it.
//! - **No recursion, no panic.** Dependencies are walked with an explicit stack; every blob's
//!   length is checked against [`RECORD_CAP`] before anything is allocated for it.
//!
//! Method: chapter 24.5 of the kernel architecture,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`, and Spare Construct's
//! proposed SQLite storage (`projects/spare-construct/ARCHITECTURE.md`, storage and scale);
//! content-addressed storage as Git's object store keys each object by its hash
//! (<https://git-scm.com/book/en/v2/Git-Internals-Git-Objects>).
//!
//! Cost: linear in the records: one decode and one SHA-256 pass per blob read, one encode per
//! record written.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::fmt;

use geom_run::{Stopped, WorkMeter};
use geom_topo::TopoError;

use crate::body::{BODY_TAG, BodyError, chunk_ids};
use crate::cbor::{CborError, CborErrorKind};
use crate::content::ContentId;
use crate::file::{FileError, HEADER_TAG, RECORD_CAP, ReadFile, write};
use crate::record::{Record, RecordError};
use crate::source::{RangeSource, SourceError, read_exact_at};

/// A store of blobs keyed by content ID: what a program's container gives the kernel.
pub trait Blobs {
    /// One blob's bytes, read at any offset.
    type Blob<'a>: RangeSource
    where
        Self: 'a;

    /// The blob stored under `key`, or `None` if the store has none.
    ///
    /// # Errors
    ///
    /// The store's own failure.
    fn blob(&self, key: &ContentId) -> Result<Option<Self::Blob<'_>>, SourceError>;
}

/// Why a blob could not be read as the record its key names.
#[derive(Clone, Debug, PartialEq)]
pub enum BlobError {
    /// The store has no blob under the key.
    Missing,
    /// The store failed.
    Source(SourceError),
    /// A blob longer than any record may be ([`RECORD_CAP`]).
    TooLarge(u64),
    /// Bytes that are not one canonical record with a matching check.
    Record(RecordError),
    /// Bytes after the record.
    Trailing,
    /// A record whose payload's content ID is not its key.
    Key,
    /// The meter stopped the work.
    Stopped(Stopped),
}

impl fmt::Display for BlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlobError::Missing => f.write_str("no blob under the key"),
            BlobError::Source(e) => write!(f, "the store failed: {e}"),
            BlobError::TooLarge(n) => write!(f, "a blob of {n} bytes, over the record cap"),
            BlobError::Record(e) => write!(f, "not an intact record: {e}"),
            BlobError::Trailing => f.write_str("bytes after the record"),
            BlobError::Key => f.write_str("a record stored under another content ID"),
            BlobError::Stopped(s) => write!(f, "{s}"),
        }
    }
}

impl core::error::Error for BlobError {}

/// A blob that could not be read, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Damage {
    /// The key it was looked up under.
    pub key: ContentId,
    /// What was wrong with it.
    pub error: BlobError,
}

/// Why a container could not be made from a file, or a file from a container.
#[derive(Clone, Debug, PartialEq)]
pub enum ContainerError {
    /// The document blob is not one intact header record.
    Document(BlobError),
    /// A record the file needs is missing or damaged.
    Blob(Damage),
    /// Two different records whose payloads share a content ID.
    Collision(ContentId),
    /// A record with no canonical encoding.
    Record(RecordError),
    /// The file could not be written.
    File(FileError),
    /// The meter stopped the work.
    Stopped(Stopped),
}

impl fmt::Display for ContainerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContainerError::Document(e) => write!(f, "the document blob: {e}"),
            ContainerError::Blob(d) => write!(f, "a record the file needs: {}", d.error),
            ContainerError::Collision(_) => f.write_str("two different records share a content ID"),
            ContainerError::Record(e) => write!(f, "a record: {e}"),
            ContainerError::File(e) => write!(f, "the file: {e}"),
            ContainerError::Stopped(s) => write!(f, "{s}"),
        }
    }
}

impl core::error::Error for ContainerError {}

impl From<RecordError> for ContainerError {
    fn from(e: RecordError) -> Self {
        match e {
            RecordError::Cbor(CborError {
                kind: CborErrorKind::Stopped(s),
                ..
            }) => ContainerError::Stopped(s),
            e => ContainerError::Record(e),
        }
    }
}

/// The writer's error, or the meter's stop behind it: the writer reports a stop inside a record's
/// encoding as that record's error, and the meter latches it.
fn file_error(e: FileError, meter: &WorkMeter<'_>) -> ContainerError {
    match meter.check() {
        Err(s) => ContainerError::Stopped(s),
        Ok(()) => ContainerError::File(e),
    }
}

/// A record's error as a blob's: the meter's stop kept apart from damage, so a cancelled read never
/// reports a record as damaged.
fn record_error(e: RecordError) -> BlobError {
    match e {
        RecordError::Cbor(CborError {
            kind: CborErrorKind::Stopped(s),
            ..
        }) => BlobError::Stopped(s),
        e => BlobError::Record(e),
    }
}

impl From<Stopped> for ContainerError {
    fn from(s: Stopped) -> Self {
        ContainerError::Stopped(s)
    }
}

/// Reads the blob under `key` as one record, checked against the key.
///
/// # Errors
///
/// [`BlobError::Missing`] if there is none; [`BlobError::TooLarge`] before anything is allocated
/// for a blob over [`RECORD_CAP`]; [`BlobError::Record`] for bytes that are not one intact canonical
/// record, [`BlobError::Trailing`] for bytes after it, [`BlobError::Key`] for a record stored under
/// another content ID; the store's and the meter's errors.
pub fn read_record<B: Blobs + ?Sized>(
    blobs: &B,
    key: &ContentId,
    meter: &mut WorkMeter<'_>,
) -> Result<Record, BlobError> {
    let blob = blobs
        .blob(key)
        .map_err(BlobError::Source)?
        .ok_or(BlobError::Missing)?;
    let len = blob.len();
    if len > RECORD_CAP {
        return Err(BlobError::TooLarge(len));
    }
    let n = usize::try_from(len).map_err(|_| BlobError::TooLarge(len))?;
    let mut bytes = Vec::new();
    if bytes.try_reserve_exact(n).is_err() {
        return Err(BlobError::Stopped(meter.out_of_memory()));
    }
    bytes.resize(n, 0);
    read_exact_at(&blob, 0, &mut bytes).map_err(BlobError::Source)?;
    let (record, used) = Record::decode(&bytes, meter).map_err(record_error)?;
    if used != n {
        return Err(BlobError::Trailing);
    }
    if record.content_id() != *key {
        return Err(BlobError::Key);
    }
    Ok(record)
}

/// The reference container: a document blob, its roots and its blobs, in memory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Container {
    /// The document blob: the header record's bytes.
    pub document: Vec<u8>,
    /// The records nothing else in the document depends on, in order.
    pub roots: Vec<ContentId>,
    /// Every other blob, by key.
    pub blobs: BTreeMap<ContentId, Vec<u8>>,
}

impl Blobs for Container {
    type Blob<'a> = &'a [u8];

    fn blob(&self, key: &ContentId) -> Result<Option<&[u8]>, SourceError> {
        Ok(self.blobs.get(key).map(Vec::as_slice))
    }
}

/// A damaged container, salvaged: a standalone file of every intact record, and the damage.
#[derive(Clone, Debug, PartialEq)]
pub struct Salvaged {
    /// The file.
    pub file: Vec<u8>,
    /// Each blob that was missing or damaged, in the order it was met.
    pub damaged: Vec<Damage>,
}

impl Container {
    /// A file's records as a container: the header as the document blob, every other record as a
    /// blob under its content ID, and the records no other record depends on as the roots, in file
    /// order.
    ///
    /// # Errors
    ///
    /// [`ContainerError::Collision`] for two different records with one content ID,
    /// [`ContainerError::Blob`] (as [`BlobError::Missing`]) for a record that depends on one the
    /// file does not hold, and the encoder's and the meter's errors.
    pub fn from_file(
        file: &ReadFile,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Container, ContainerError> {
        let document = file.header.encode(meter)?;
        let mut blobs: BTreeMap<ContentId, Vec<u8>> = BTreeMap::new();
        let mut needed: BTreeSet<ContentId> = BTreeSet::new();
        let mut order = Vec::new();
        for record in &file.records {
            meter.charge(1)?;
            let key = record.content_id();
            let bytes = record.encode(meter)?;
            match blobs.get(&key) {
                Some(old) if *old != bytes => return Err(ContainerError::Collision(key)),
                Some(_) => {}
                None => {
                    blobs.insert(key.clone(), bytes);
                    order.push(key);
                }
            }
            needed.extend(record.depends.iter().cloned());
        }
        if let Some(key) = needed.iter().find(|k| !blobs.contains_key(k)) {
            return Err(ContainerError::Blob(Damage {
                key: key.clone(),
                error: BlobError::Missing,
            }));
        }
        let roots = order.into_iter().filter(|k| !needed.contains(k)).collect();
        Ok(Container {
            document,
            roots,
            blobs,
        })
    }

    /// The standalone file of the document: the header, then each root after the records it
    /// depends on, each record once, then the END record.
    ///
    /// # Errors
    ///
    /// [`ContainerError::Document`] for a document blob that is not one intact header record,
    /// [`ContainerError::Blob`] for a record a root needs that is missing or damaged, and the
    /// writer's and the meter's errors.
    pub fn to_file(&self, meter: &mut WorkMeter<'_>) -> Result<Vec<u8>, ContainerError> {
        let header = self.header(meter)?;
        let mut records = Vec::new();
        let mut damaged = Vec::new();
        walk(self, &self.roots, &mut records, &mut damaged, meter)?;
        if let Some(d) = damaged.into_iter().next() {
            return Err(ContainerError::Blob(d));
        }
        write(&header, &records, meter).map_err(|e| file_error(e, meter))
    }

    /// Every intact record as a standalone file: the records the roots reach in
    /// [`Container::to_file`]'s order, skipping each missing or damaged one, then every other
    /// intact blob in key order; and the damaged blobs.
    ///
    /// # Errors
    ///
    /// [`ContainerError::Document`] for a document blob that is not one intact header record, and
    /// the writer's and the meter's errors.
    pub fn salvage(&self, meter: &mut WorkMeter<'_>) -> Result<Salvaged, ContainerError> {
        let header = self.header(meter)?;
        let mut records = Vec::new();
        let mut damaged = Vec::new();
        let reached = walk(self, &self.roots, &mut records, &mut damaged, meter)?;
        for key in self.blobs.keys() {
            if reached.contains(key) {
                continue;
            }
            match read_record(self, key, meter) {
                Ok(record) => records.push(record),
                Err(BlobError::Stopped(s)) => return Err(ContainerError::Stopped(s)),
                Err(error) => damaged.push(Damage {
                    key: key.clone(),
                    error,
                }),
            }
        }
        Ok(Salvaged {
            file: write(&header, &records, meter).map_err(|e| file_error(e, meter))?,
            damaged,
        })
    }

    /// The document blob as its header record.
    fn header(&self, meter: &mut WorkMeter<'_>) -> Result<Record, ContainerError> {
        let (record, used) =
            Record::decode(&self.document, meter).map_err(|e| match record_error(e) {
                BlobError::Stopped(s) => ContainerError::Stopped(s),
                e => ContainerError::Document(e),
            })?;
        if used != self.document.len() {
            return Err(ContainerError::Document(BlobError::Trailing));
        }
        if record.tag != HEADER_TAG {
            return Err(ContainerError::Document(BlobError::Key));
        }
        Ok(record)
    }
}

/// The records `roots` reach, each after the records it depends on and each once, pushed onto
/// `out`; each blob that is missing or damaged pushed onto `damaged` instead, and not walked
/// further. Returns every key met, intact or not.
fn walk<B: Blobs + ?Sized>(
    blobs: &B,
    roots: &[ContentId],
    out: &mut Vec<Record>,
    damaged: &mut Vec<Damage>,
    meter: &mut WorkMeter<'_>,
) -> Result<BTreeSet<ContentId>, ContainerError> {
    let mut met: BTreeSet<ContentId> = BTreeSet::new();
    // Each frame is a record read and its children still to visit, in reverse order.
    let mut stack: Vec<(Record, Vec<ContentId>)> = Vec::new();
    for root in roots {
        let mut next = Some(root.clone());
        loop {
            if let Some(key) = next.take()
                && met.insert(key.clone())
            {
                match read_record(blobs, &key, meter) {
                    Ok(record) => {
                        let mut children = children(&record, meter)?;
                        children.reverse();
                        stack.push((record, children));
                    }
                    Err(BlobError::Stopped(s)) => return Err(ContainerError::Stopped(s)),
                    Err(error) => damaged.push(Damage { key, error }),
                }
            }
            let Some((_, children)) = stack.last_mut() else {
                break;
            };
            if let Some(child) = children.pop() {
                next = Some(child);
            } else if let Some((record, _)) = stack.pop() {
                out.push(record);
            }
        }
    }
    Ok(met)
}

/// The records `record` depends on, in the order a file holds them before it: a body's chunks in
/// its index's order, then anything else it depends on; any other record's `depends` as sorted, as
/// is a body index whose chunk list does not read (the body reader refuses it; a file only carries
/// it).
fn children(record: &Record, meter: &mut WorkMeter<'_>) -> Result<Vec<ContentId>, ContainerError> {
    if record.tag != BODY_TAG {
        return Ok(record.depends.clone());
    }
    let ids = match chunk_ids(record, meter) {
        Ok(ids) => ids,
        Err(e) => {
            if let Some(s) = stop(&e, meter) {
                return Err(ContainerError::Stopped(s));
            }
            Vec::new()
        }
    };
    let mut seen: BTreeSet<&ContentId> = BTreeSet::new();
    let mut out = Vec::new();
    for id in ids.iter().chain(&record.depends) {
        if seen.insert(id) {
            out.push(id.clone());
        }
    }
    Ok(out)
}

/// The meter's stop behind a body error, if that is what it was: the meter latches every stop, and
/// an allocation geom-topo refused is the meter's out-of-memory stop. Anything else is damage.
pub(crate) fn stop(e: &BodyError, meter: &mut WorkMeter<'_>) -> Option<Stopped> {
    if let Err(s) = meter.check() {
        return Some(s);
    }
    matches!(e, BodyError::Topo(TopoError::OutOfMemory)).then(|| meter.out_of_memory())
}
