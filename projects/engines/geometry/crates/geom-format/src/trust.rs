//! Trust on open: the program says where a document's bytes came from, and each stored body is
//! checked as that trust needs, the cheap checks at once and the full checker later, body by body,
//! so a bad body never stops the document (24.7, 23.11).
//!
//! What it does: [`open`] reads a document from a container ([`Blobs`], its document blob and its
//! roots): the header, then each root's body, every blob checked against its key, built back
//! through geom-topo's builder, which range-checks every coordinate and checks the topology again.
//! The program's [`Checker`] then runs the cheap groups on every body. A body whose trust needs the
//! full checker is left [`State::Pending`], and [`Opened::check_next`] runs the full checker on the
//! next pending body within the caller's budget. [`Opened::input`] hands a body out as an operation
//! input only once it is [`State::Usable`]; the stamps the full checks earn are kept for the program
//! to store ([`Opened::fresh_stamps`]).
//!
//! The checker is the program's to pass: chapter 27 puts geom-format above no checker (its row is
//! the base and everything stored), so [`Checker`] is how geom-check's `check` reaches it.
//!
//! Which check a body needs (24.7):
//!
//! - [`Trust::Known`], the program's own store or a source whose integrity it checks: a body whose
//!   stamp comes from a checker at least as new as the reader's, and lists every group the reader's
//!   full check runs, needs only the cheap groups. Any other body needs the full check.
//! - [`Trust::Unknown`], everything else: every body needs the full check, and stamps are not read
//!   at all. The kernel never infers trust from the file.
//!
//! Guarantees:
//!
//! - **The cheap check always runs**, at either trust, because the combinatorial group is what
//!   keeps the kernel's own loops finite (23.11).
//! - **A bad body never stops the document.** A body whose records are missing or damaged, or that
//!   does not build back, opens [`State::Damaged`], with no geometry; one that fails a check opens
//!   [`State::Failed`], read-only, with the checker's report. Neither is ever an operation input;
//!   healing one is an operation in the log. The other bodies open normally.
//! - **A pending body is not an input** until its full check passes.
//! - **Nothing is published from a stopped read.** If the meter stops inside [`open`], nothing
//!   opens; inside [`Opened::check_next`], the body stays pending and nothing about it changes.
//! - **Deterministic.** Bodies open in root order, and [`Opened::check_next`] takes the first
//!   pending one.
//!
//! Method: chapter 24.7 (trust on open) and 23.11 (checks on open) of the kernel architecture,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md` and `23-performance.md`, with
//! chapter 8.3's groups (`08-topology.md`).
//!
//! Cost: [`open`] is linear in the document's records and bodies, plus the cheap checks, which are
//! linear in each body; [`Opened::check_next`] is one full check of one body.

use alloc::vec::Vec;
use core::fmt;

use geom_run::{Stopped, WorkMeter};
use geom_topo::Body;

use crate::body::{BODY_TAG, BodyError, StoredBody, read_body};
use crate::container::{BlobError, Blobs, Damage, read_record, stop};
use crate::content::ContentId;
use crate::file::HEADER_TAG;
use crate::header::{Header, HeaderError, PRE_EDITION};
use crate::record::Record;
use crate::registry::knows;
use crate::stamp::{STAMP_TAG, Stamp, StampError};

/// Where a document's bytes came from, as the program states it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trust {
    /// The program's own store, or a source whose integrity it checks, such as a signed sharing
    /// package.
    Known,
    /// Anything else.
    Unknown,
}

/// Which of chapter 8.3's groups a check runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// The cheap groups: the combinatorial group and the exactness record.
    Cheap,
    /// Every group the checker has.
    Full,
}

/// What one check of a body found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked<R> {
    /// The groups that ran, by chapter 8.3's numbers.
    pub ran: Vec<u64>,
    /// Whether the body passed every group that ran.
    pub passed: bool,
    /// The checker's own report.
    pub report: R,
}

/// The validity checker, as the program passes it.
pub trait Checker {
    /// The checker's own report of one check.
    type Report;

    /// The checker's version, as its stamps record it.
    fn version(&self) -> u64;

    /// The groups its full check runs, by chapter 8.3's numbers.
    fn groups(&self) -> Vec<u64>;

    /// Checks `body` at `level`, charging `meter`.
    ///
    /// # Errors
    ///
    /// The meter's stop; nothing is reported then.
    fn check(
        &mut self,
        body: &Body,
        level: Level,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Checked<Self::Report>, Stopped>;
}

/// Why a root's body could not be read.
#[derive(Clone, Debug, PartialEq)]
pub enum Unread {
    /// A record it needs is missing or damaged.
    Blob(Damage),
    /// Its stamp does not read.
    Stamp(StampError),
    /// Its records do not build a body.
    Body(BodyError),
}

impl fmt::Display for Unread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unread::Blob(d) => write!(f, "a record it needs: {}", d.error),
            Unread::Stamp(e) => write!(f, "its stamp: {e}"),
            Unread::Body(e) => write!(f, "its records: {e}"),
        }
    }
}

/// Where a body stands after opening.
#[derive(Clone, Debug, PartialEq)]
pub enum State<R> {
    /// Checked as its trust needs: an operation input.
    Usable,
    /// Passed the cheap check; the full check is still to run.
    Pending,
    /// Read-only, with no geometry: its records could not be read.
    Damaged(Unread),
    /// Read-only: a check failed.
    Failed(Checked<R>),
}

/// One root of the document, opened.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry<R> {
    /// The root's key: the body's index, or its stamp.
    pub root: ContentId,
    /// The body's content ID, once known.
    pub id: Option<ContentId>,
    /// The body, unless its records could not be read.
    pub body: Option<Body>,
    /// Where it stands.
    pub state: State<R>,
    /// Its stamp: the one stored with it, read for a known source only, or the one its full check
    /// earned here.
    pub stamp: Option<Stamp>,
    /// Whether the stamp was earned here, and so is the program's to store.
    pub fresh: bool,
}

/// A document, opened.
#[derive(Clone, Debug, PartialEq)]
pub struct Opened<R> {
    /// Its header.
    pub header: Header,
    /// The trust it was opened with.
    pub trust: Trust,
    /// Each body root, in root order.
    pub entries: Vec<Entry<R>>,
    /// Each root of another record type this reader knows, for the program to read.
    pub other: Vec<Record>,
    /// Each optional root of a type this reader does not know: its key and type.
    pub skipped: Vec<(ContentId, u64)>,
}

/// Why a document did not open at all.
#[derive(Clone, Debug, PartialEq)]
pub enum OpenError {
    /// The document blob is not one intact header record.
    Document(BlobError),
    /// The header does not read.
    Header(HeaderError),
    /// A required root of a record type this reader does not know (24.2).
    Unsupported {
        /// Its key.
        key: ContentId,
        /// Its type.
        tag: u64,
        /// Its schema.
        schema: u64,
    },
    /// The meter stopped the work.
    Stopped(Stopped),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::Document(e) => write!(f, "the document blob: {e}"),
            OpenError::Header(e) => write!(f, "the header: {e}"),
            OpenError::Unsupported { tag, schema, .. } => {
                write!(f, "a required record of type {tag}, schema {schema}")
            }
            OpenError::Stopped(s) => write!(f, "{s}"),
        }
    }
}

impl core::error::Error for OpenError {}

impl From<Stopped> for OpenError {
    fn from(s: Stopped) -> Self {
        OpenError::Stopped(s)
    }
}

/// A record read for opening: the record, damage that stays with its body, or the meter's stop.
fn fetch<B: Blobs + ?Sized>(
    blobs: &B,
    key: &ContentId,
    meter: &mut WorkMeter<'_>,
) -> Result<Result<Record, Unread>, Stopped> {
    match read_record(blobs, key, meter) {
        Ok(record) => Ok(Ok(record)),
        Err(BlobError::Stopped(s)) => Err(s),
        Err(error) => Ok(Err(Unread::Blob(Damage {
            key: key.clone(),
            error,
        }))),
    }
}

/// Opens the document whose header is `document` and whose roots are `roots`, at `trust`.
///
/// # Errors
///
/// [`OpenError::Document`] or [`OpenError::Header`] for a document blob that does not read,
/// [`OpenError::Unsupported`] for a required root of an unknown type, and the meter's stop. A body
/// that does not read, or fails a check, is never an error here: it opens read-only.
pub fn open<B: Blobs + ?Sized, C: Checker>(
    blobs: &B,
    document: &[u8],
    roots: &[ContentId],
    trust: Trust,
    checker: &mut C,
    meter: &mut WorkMeter<'_>,
) -> Result<Opened<C::Report>, OpenError> {
    let header = header(document, meter)?;
    let mut opened = Opened {
        header,
        trust,
        entries: Vec::new(),
        other: Vec::new(),
        skipped: Vec::new(),
    };
    let groups = checker.groups();
    for root in roots {
        let record = match fetch(blobs, root, meter)? {
            Ok(record) => record,
            Err(unread) => {
                opened.entries.push(damaged(root, None, None, unread));
                continue;
            }
        };
        if !knows(record.tag, record.schema) {
            if record.is_required() {
                return Err(OpenError::Unsupported {
                    key: root.clone(),
                    tag: record.tag,
                    schema: record.schema,
                });
            }
            opened.skipped.push((root.clone(), record.tag));
            continue;
        }
        let (index, stamp) = match record.tag {
            BODY_TAG => (record, None),
            STAMP_TAG => {
                let stamp = match Stamp::read(&record, meter) {
                    Ok(stamp) => stamp,
                    Err(e) => {
                        if let Err(s) = meter.check() {
                            return Err(s.into());
                        }
                        opened.entries.push(damaged(root, None, None, Unread::Stamp(e)));
                        continue;
                    }
                };
                match fetch(blobs, &stamp.body, meter)? {
                    Ok(index) => (index, (trust == Trust::Known).then_some(stamp)),
                    Err(unread) => {
                        let id = Some(stamp.body.clone());
                        opened.entries.push(damaged(root, id, None, unread));
                        continue;
                    }
                }
            }
            _ => {
                opened.other.push(record);
                continue;
            }
        };
        let entry = body_entry(blobs, root, index, stamp, trust, &groups, checker, meter)?;
        opened.entries.push(entry);
    }
    Ok(opened)
}

/// The document blob's header.
fn header(document: &[u8], meter: &mut WorkMeter<'_>) -> Result<Header, OpenError> {
    let (record, used) = Record::decode(document, meter).map_err(|e| {
        meter
            .check()
            .map_or_else(OpenError::Stopped, |()| OpenError::Document(BlobError::Record(e)))
    })?;
    if used != document.len() {
        return Err(OpenError::Document(BlobError::Trailing));
    }
    if record.tag != HEADER_TAG {
        return Err(OpenError::Document(BlobError::Key));
    }
    Header::read(&record.payload, meter).map_err(|e| {
        meter
            .check()
            .map_or_else(OpenError::Stopped, |()| OpenError::Header(e))
    })
}

/// A root that opens with no body.
fn damaged<R>(
    root: &ContentId,
    id: Option<ContentId>,
    stamp: Option<Stamp>,
    unread: Unread,
) -> Entry<R> {
    Entry {
        root: root.clone(),
        id,
        body: None,
        state: State::Damaged(unread),
        stamp,
        fresh: false,
    }
}

/// One body root: its chunks, the body built back, and the cheap check.
#[allow(
    clippy::too_many_arguments,
    reason = "the document's store, the root and its records, and the open's trust, checker and meter"
)]
fn body_entry<B: Blobs + ?Sized, C: Checker>(
    blobs: &B,
    root: &ContentId,
    index: Record,
    stamp: Option<Stamp>,
    trust: Trust,
    groups: &[u64],
    checker: &mut C,
    meter: &mut WorkMeter<'_>,
) -> Result<Entry<C::Report>, OpenError> {
    let id = index.content_id();
    let mut chunks = Vec::new();
    for key in &index.depends {
        match fetch(blobs, key, meter)? {
            Ok(chunk) => chunks.push(chunk),
            Err(unread) => return Ok(damaged(root, Some(id), stamp, unread)),
        }
    }
    let body = match read_body(&StoredBody { index, chunks }, meter) {
        Ok(body) => body,
        Err(e) => {
            if let Some(s) = stop(&e, meter) {
                return Err(s.into());
            }
            return Ok(damaged(root, Some(id), stamp, Unread::Body(e)));
        }
    };
    let cheap = checker.check(&body, Level::Cheap, meter)?;
    let state = if !cheap.passed {
        State::Failed(cheap)
    } else if needs_full(trust, stamp.as_ref(), checker.version(), groups) {
        State::Pending
    } else {
        State::Usable
    };
    Ok(Entry {
        root: root.clone(),
        id: Some(id),
        body: Some(body),
        state,
        stamp,
        fresh: false,
    })
}

/// Whether a body opened at `trust` with `stamp` needs the full check of a checker at `version`
/// whose full check runs `groups`.
fn needs_full(trust: Trust, stamp: Option<&Stamp>, version: u64, groups: &[u64]) -> bool {
    match (trust, stamp) {
        (Trust::Known, Some(s)) => s.checker < version || !groups.iter().all(|&g| s.passed(g)),
        _ => true,
    }
}

impl<R> Opened<R> {
    /// The body of entry `at` as an operation input: only once it is [`State::Usable`].
    #[must_use]
    pub fn input(&self, at: usize) -> Option<&Body> {
        let entry = self.entries.get(at)?;
        match entry.state {
            State::Usable => entry.body.as_ref(),
            _ => None,
        }
    }

    /// Whether any body is still waiting for its full check.
    #[must_use]
    pub fn pending(&self) -> bool {
        self.entries
            .iter()
            .any(|e| matches!(e.state, State::Pending))
    }

    /// The stamps the full checks earned here, in entry order: the program's to store beside its
    /// bodies.
    pub fn fresh_stamps(&self) -> impl Iterator<Item = &Stamp> {
        self.entries
            .iter()
            .filter(|e| e.fresh)
            .filter_map(|e| e.stamp.as_ref())
    }

    /// Runs the full check on the first pending body, and says which entry it was; `None` if no
    /// body is pending. A body that passes becomes [`State::Usable`] with a fresh stamp from this
    /// checker; one that fails becomes [`State::Failed`].
    ///
    /// # Errors
    ///
    /// The meter's stop; the body then stays pending, unchanged.
    pub fn check_next<C: Checker<Report = R>>(
        &mut self,
        checker: &mut C,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<usize>, Stopped> {
        for (at, entry) in self.entries.iter_mut().enumerate() {
            let (State::Pending, Some(body), Some(id)) = (&entry.state, &entry.body, &entry.id)
            else {
                continue;
            };
            let id = id.clone();
            let full = checker.check(body, Level::Full, meter)?;
            if full.passed {
                let mut groups = full.ran;
                groups.sort_unstable();
                groups.dedup();
                entry.stamp = Some(Stamp {
                    body: id,
                    checker: checker.version(),
                    groups,
                    edition: PRE_EDITION,
                });
                entry.fresh = true;
                entry.state = State::Usable;
            } else {
                entry.state = State::Failed(full);
            }
            return Ok(Some(at));
        }
        Ok(None)
    }
}
