//! The hardened zip reader every zip-based format goes through (3MF, and other Open Packaging
//! Conventions packages): an archive read through a [`RangeSource`], its central directory checked
//! against every local header, and each entry inflated within its declared size and the ratio
//! budget.
//!
//! What it does: [`ZipArchive::read`] finds the end of central directory record (and the Zip64
//! records), reads the central directory, and checks every entry before it lists any: a name that
//! is UTF-8, within [`ZipBudget::name`], not repeated and escaping no folder (zip-slip,
//! [`part::escapes`]); stored or deflated, not encrypted; a local header that agrees with the
//! central one; and an entry, its header included, that overlaps no other entry nor the central
//! directory. The sum of the declared sizes is held to [`ZipBudget::total`] and the count of
//! entries to [`ZipBudget::entries`]. [`ZipArchive::read_entry`] reads one entry: stored bytes
//! as they are, deflated ones through [`crate::inflate`] within the smaller of the entry's
//! declared size and [`ZipBudget::inflate`]; the output must be exactly the declared size and
//! match the declared CRC-32. [`ZipArchive::check_parts`] holds every name to the Open Packaging
//! Conventions' part-name rules ([`part::check`]) and refuses names equal as case-insensitive
//! ASCII.
//!
//! **Why the known attacks have nothing to act on.** A zip-slip name is refused before any entry
//! is read. A zip bomb is held to its declared size, which the archive's total is held to before
//! anything is inflated, and to the ratio budget, since inflate's limit is fixed by the entry's
//! compressed size. Overlapping entries (the non-recursive zip bomb, or an entry hiding inside
//! another) are refused, header and data together, as is a central directory that does not end
//! exactly where its end record starts (bytes hidden before or after it).
//!
//! Guarantees:
//!
//! - **Refused with a typed error and a byte offset, never a panic.** Every failure is a
//!   [`ZipError`] naming its [`ZipErrorKind`] and the byte offset in the archive.
//! - **Every budget holds**, checked before the allocation or the read it bounds.
//! - **An entry read is the entry declared**: its bytes are exactly its declared size, and their
//!   CRC-32 is its declared CRC-32.
//! - **The same bytes give the same archive and the same entries**, however the source splits its
//!   reads.
//!
//! Method: PKWARE's APPNOTE.TXT 6.3.10, sections 4.3 and 4.4,
//! <https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT>, read by [`spec`], ported from
//! zip2; the overlap check of zip2's `has_overlapping_entries` and David Fifield's analysis of
//! overlapping-file zip bombs (USENIX WOOT 2019, <https://www.bamsoftware.com/hacks/zipbomb/>);
//! the part-name rules of ECMA-376 Part 2 ([`part`]).
//!
//! Consulted: zip2, MIT,
//!   <https://github.com/zip-rs/zip2/blob/ddf3ec36017345b8936dd1a3516d9bc4632a0480/src/read/zip_archive.rs>,
//!   for its overlap check over entries' byte ranges, headers included, and its checks of each
//!   local header against its central header. No code copied.
//!
//! Cost: one read of the archive's last 65,557 bytes and of its central directory; one read of
//! each local header; entries sorted once by offset; each entry read is one pass of its bytes
//! and a CRC-32. Memory: the central directory and the entries' names. No recursion.

pub mod part;
mod spec;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use geom_format::checksum::crc32;
use geom_format::source::{ByteSource, RangeSource, SourceError, read_exact_at};
use geom_run::WorkMeter;

use crate::inflate::{InflateBudget, InflateErrorKind, inflate};
use part::PartRule;
use spec::{End, LOCAL_LEN};

/// The budgets an archive is read within.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZipBudget {
    /// The most entries an archive may hold.
    pub entries: usize,
    /// The longest entry name, in bytes.
    pub name: usize,
    /// The most bytes the entries may declare in all.
    pub total: u64,
    /// The budget each entry is inflated within (its size budget is lowered to the entry's
    /// declared size).
    pub inflate: InflateBudget,
}

impl ZipBudget {
    /// 1,000 entries (Apache POI's default for the same attack), 1 KiB names, 1 GiB declared in
    /// all, and inflate's default budget.
    pub const DEFAULT: ZipBudget = ZipBudget {
        entries: 1_000,
        name: 1_024,
        total: 1 << 30,
        inflate: InflateBudget::DEFAULT,
    };
}

/// Why an archive or an entry was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZipErrorKind {
    /// The source failed.
    Source(SourceError),
    /// No end of central directory record whose comment ends at the archive's end.
    End,
    /// The archive spans more than one disk.
    MultiDisk,
    /// A Zip64 locator, record or extra field is missing, misplaced or short.
    Zip64,
    /// The central directory does not end where its end record (or Zip64 record) starts, or its
    /// entries are not the count and size it declares.
    Directory,
    /// A central directory header is malformed.
    Header,
    /// More entries than the budget.
    TooManyEntries,
    /// A name longer than the budget.
    NameTooLong,
    /// A name that is not UTF-8.
    NameEncoding,
    /// Two entries with the same name.
    Duplicate,
    /// A name that could reach outside the folder it is read into (zip-slip).
    Escape,
    /// A name the Open Packaging Conventions forbid.
    Part(PartRule),
    /// Two entries, or an entry and the central directory, share bytes.
    Overlap,
    /// A local header that is malformed or disagrees with its central header.
    LocalHeader,
    /// An encrypted entry.
    Encrypted,
    /// A compression method other than stored (0) or deflated (8).
    Method,
    /// An entry whose bytes are not its declared size.
    Size,
    /// An entry whose CRC-32 is not its declared one.
    Crc,
    /// The entries declare more bytes in all than the budget.
    TooLarge,
    /// An entry's deflated stream was refused.
    Inflate(InflateErrorKind),
    /// The work meter stopped the read.
    Stopped,
    /// The allocator refused.
    OutOfMemory,
}

/// A refused archive or entry: what, and the byte offset in the archive where it was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZipError {
    /// What was refused.
    pub kind: ZipErrorKind,
    /// The byte offset in the archive.
    pub offset: u64,
}

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "zip refused at byte {}: {:?}", self.offset, self.kind)
    }
}

impl core::error::Error for ZipError {}

const fn refused(kind: ZipErrorKind, offset: u64) -> ZipError {
    ZipError { kind, offset }
}

/// How an entry's bytes are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// As they are (method 0).
    Stored,
    /// Raw DEFLATE (method 8).
    Deflated,
}

/// One entry of the central directory, checked against its local header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Its name, as stored.
    pub name: String,
    /// How its bytes are stored.
    pub method: Method,
    /// The CRC-32 of its bytes.
    pub crc32: u32,
    /// Its stored length.
    pub compressed: u64,
    /// Its length once read.
    pub size: u64,
    /// Where its local header starts.
    pub header: u64,
    /// Where its stored bytes start.
    pub data: u64,
}

/// A checked archive: its entries, in central directory order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZipArchive {
    entries: Vec<Entry>,
    directory: u64,
}

fn len_u64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

fn source_error(e: SourceError, at: u64) -> ZipError {
    refused(ZipErrorKind::Source(e), at)
}

/// Reads `len` bytes at `at`, checking the length against the source before allocating.
fn read_range<S: RangeSource + ?Sized>(source: &S, at: u64, len: u64) -> Result<Vec<u8>, ZipError> {
    let end = at
        .checked_add(len)
        .ok_or(source_error(SourceError::OutOfBounds, at))?;
    if end > source.len() {
        return Err(source_error(SourceError::OutOfBounds, at));
    }
    let n = usize::try_from(len).map_err(|_| refused(ZipErrorKind::OutOfMemory, at))?;
    let mut out = Vec::new();
    out.try_reserve_exact(n)
        .map_err(|_| refused(ZipErrorKind::OutOfMemory, at))?;
    out.resize(n, 0);
    read_exact_at(source, at, &mut out).map_err(|e| source_error(e, at))?;
    Ok(out)
}

impl ZipArchive {
    /// Reads and checks the central directory of the archive `source` holds.
    ///
    /// # Errors
    ///
    /// The first check the archive fails, as the module documentation lists them.
    #[allow(
        clippy::too_many_lines,
        reason = "one reading of APPNOTE's records, in the order the archive is read"
    )]
    pub fn read<S: RangeSource + ?Sized>(
        source: &S,
        budget: &ZipBudget,
        meter: &mut WorkMeter<'_>,
    ) -> Result<ZipArchive, ZipError> {
        let len = source.len();
        let tail_len = len.min(len_u64(spec::END_LEN.saturating_add(spec::MAX_COMMENT)));
        let tail_at = len.saturating_sub(tail_len);
        let tail = read_range(source, tail_at, tail_len)?;
        let (_, mut end) = spec::find_end(&tail, tail_at, len)?;
        let locator_len = len_u64(spec::ZIP64_LOCATOR_LEN);
        if let Some(locator) = end.at.checked_sub(locator_len) {
            let bytes = read_range(source, locator, locator_len)?;
            let has_locator = bytes.get(..4) == Some(0x0706_4b50_u32.to_le_bytes().as_slice());
            let entries16 = u16::try_from(end.entries).unwrap_or(u16::MAX);
            let size32 = u32::try_from(end.size).unwrap_or(u32::MAX);
            let offset32 = u32::try_from(end.offset).unwrap_or(u32::MAX);
            if has_locator || End::needs_zip64(entries16, size32, offset32) {
                let record = spec::read_zip64_locator(&bytes, locator)?;
                let fixed = read_range(source, record, len_u64(spec::ZIP64_END_LEN))?;
                spec::read_zip64_end(&fixed, record, locator, &mut end)?;
            }
        } else if End::needs_zip64(
            u16::try_from(end.entries).unwrap_or(u16::MAX),
            u32::try_from(end.size).unwrap_or(u32::MAX),
            u32::try_from(end.offset).unwrap_or(u32::MAX),
        ) {
            return Err(refused(ZipErrorKind::Zip64, end.at));
        }
        // The central directory ends exactly where its end record (or Zip64 record) starts: no
        // bytes before the archive, none hidden between.
        if end.offset.checked_add(end.size) != Some(end.directory_end()) {
            return Err(refused(ZipErrorKind::Directory, end.offset));
        }
        if end.entries > len_u64(budget.entries) {
            return Err(refused(ZipErrorKind::TooManyEntries, end.at));
        }
        meter
            .charge(end.size)
            .map_err(|_| refused(ZipErrorKind::Stopped, end.offset))?;
        let directory = read_range(source, end.offset, end.size)?;
        let mut entries: Vec<Entry> = Vec::new();
        entries
            .try_reserve_exact(usize::try_from(end.entries).unwrap_or(usize::MAX))
            .map_err(|_| refused(ZipErrorKind::OutOfMemory, end.offset))?;
        let mut at = 0_usize;
        let mut total = 0_u64;
        while at < directory.len() {
            let here = end.offset.saturating_add(len_u64(at));
            if len_u64(entries.len()) >= end.entries {
                return Err(refused(ZipErrorKind::Directory, here));
            }
            let c = spec::read_central(&directory, at, end.offset)?;
            let name_at = at.saturating_add(spec::CENTRAL_LEN);
            let raw = directory
                .get(name_at..name_at.saturating_add(c.name_len))
                .ok_or(refused(ZipErrorKind::Header, here))?;
            let entry = Self::entry(source, &c, raw, here, budget)?;
            total = total
                .checked_add(entry.size)
                .filter(|t| *t <= budget.total)
                .ok_or(refused(ZipErrorKind::TooLarge, here))?;
            entries.push(entry);
            at = at.saturating_add(c.len().unwrap_or(usize::MAX));
        }
        if len_u64(entries.len()) != end.entries {
            return Err(refused(ZipErrorKind::Directory, end.at));
        }
        Self::refuse_duplicates_and_overlaps(&entries, end.offset)?;
        Ok(ZipArchive {
            entries,
            directory: end.offset,
        })
    }

    /// One entry from its central header `c` and name `raw`, checked against its local header.
    fn entry<S: RangeSource + ?Sized>(
        source: &S,
        c: &spec::Central,
        raw: &[u8],
        here: u64,
        budget: &ZipBudget,
    ) -> Result<Entry, ZipError> {
        if raw.len() > budget.name {
            return Err(refused(ZipErrorKind::NameTooLong, here));
        }
        let name =
            core::str::from_utf8(raw).map_err(|_| refused(ZipErrorKind::NameEncoding, here))?;
        if part::escapes(name) {
            return Err(refused(ZipErrorKind::Escape, here));
        }
        // Bit 0: encrypted; bit 6: strong encryption (APPNOTE 4.4.4).
        if c.flags & 0x0041 != 0 {
            return Err(refused(ZipErrorKind::Encrypted, here));
        }
        let method = match c.method {
            0 => Method::Stored,
            8 => Method::Deflated,
            _ => return Err(refused(ZipErrorKind::Method, here)),
        };
        if method == Method::Stored && c.compressed != c.size {
            return Err(refused(ZipErrorKind::Size, here));
        }
        let fixed = read_range(source, c.header, len_u64(LOCAL_LEN))?;
        let l = spec::read_local(&fixed, c.header)?;
        let bad = refused(ZipErrorKind::LocalHeader, c.header);
        let name_at = c.header.checked_add(len_u64(LOCAL_LEN)).ok_or(bad)?;
        let local_name = read_range(source, name_at, len_u64(l.name_len))?;
        // Bit 3: sizes and CRC in a data descriptor after the data; the local fields may be 0.
        let described = l.flags & 0x0008 != 0;
        let sentinel =
            |v: u32, want: u64| (v == u32::MAX) || u64::from(v) == want || (described && v == 0);
        if local_name != raw
            || l.flags != c.flags
            || l.method != c.method
            || !(l.crc32 == c.crc32 || (described && l.crc32 == 0))
            || !sentinel(l.compressed, c.compressed)
            || !sentinel(l.size, c.size)
        {
            return Err(bad);
        }
        let data = name_at
            .checked_add(len_u64(l.name_len))
            .and_then(|d| d.checked_add(len_u64(l.extra_len)))
            .ok_or(bad)?;
        Ok(Entry {
            name: String::from(name),
            method,
            crc32: c.crc32,
            compressed: c.compressed,
            size: c.size,
            header: c.header,
            data,
        })
    }

    /// Refuses two entries with one name, and any two entries (headers included) or an entry and
    /// the central directory that share a byte.
    fn refuse_duplicates_and_overlaps(entries: &[Entry], directory: u64) -> Result<(), ZipError> {
        let mut order: Vec<&Entry> = Vec::new();
        order
            .try_reserve_exact(entries.len())
            .map_err(|_| refused(ZipErrorKind::OutOfMemory, directory))?;
        order.extend(entries.iter());
        // By name, then by header, so a repeated name is reported at its later header.
        order.sort_unstable_by(|a, b| a.name.cmp(&b.name).then(a.header.cmp(&b.header)));
        for pair in order.windows(2) {
            if let [a, b] = pair
                && a.name == b.name
            {
                return Err(refused(ZipErrorKind::Duplicate, b.header));
            }
        }
        order.sort_unstable_by_key(|e| e.header);
        let mut free_from = 0_u64;
        for e in order {
            let end = e
                .data
                .checked_add(e.compressed)
                .ok_or(refused(ZipErrorKind::Overlap, e.header))?;
            if e.header < free_from || end > directory {
                return Err(refused(ZipErrorKind::Overlap, e.header));
            }
            free_from = end;
        }
        Ok(())
    }

    /// The entries, in central directory order.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The entry named `name`, if any.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Where the central directory starts.
    #[must_use]
    pub const fn directory(&self) -> u64 {
        self.directory
    }

    /// Holds every name to the Open Packaging Conventions' part-name rules: the content types
    /// stream (`[Content_Types].xml`) and folder entries (a name ending in `/`, holding nothing)
    /// are not parts and are left out.
    ///
    /// # Errors
    ///
    /// [`ZipErrorKind::Part`] with the first rule an entry breaks, at its header.
    pub fn check_parts(&self) -> Result<(), ZipError> {
        let mut folded: Vec<(String, u64)> = Vec::new();
        for e in &self.entries {
            if e.name == "[Content_Types].xml" || (e.name.ends_with('/') && e.size == 0) {
                continue;
            }
            part::check(&e.name).map_err(|r| refused(ZipErrorKind::Part(r), e.header))?;
            folded
                .try_reserve(1)
                .map_err(|_| refused(ZipErrorKind::OutOfMemory, e.header))?;
            folded.push((part::fold(&e.name), e.header));
        }
        folded.sort_unstable();
        for pair in folded.windows(2) {
            if let [(a, _), (b, at)] = pair
                && a == b
            {
                return Err(refused(ZipErrorKind::Part(PartRule::Equivalent), *at));
            }
        }
        Ok(())
    }

    /// Reads `entry`'s bytes from `source`.
    ///
    /// # Errors
    ///
    /// [`ZipErrorKind::Source`]; [`ZipErrorKind::Inflate`] for a refused stream (a bomb past its
    /// declared size or the ratio budget among them); [`ZipErrorKind::Size`] or
    /// [`ZipErrorKind::Crc`] if the bytes are not the entry declared; [`ZipErrorKind::Stopped`];
    /// [`ZipErrorKind::OutOfMemory`].
    pub fn read_entry<S: RangeSource + ?Sized>(
        &self,
        source: &S,
        entry: &Entry,
        budget: &ZipBudget,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<u8>, ZipError> {
        let out = match entry.method {
            Method::Stored => {
                meter
                    .charge(entry.size)
                    .map_err(|_| refused(ZipErrorKind::Stopped, entry.data))?;
                read_range(source, entry.data, entry.size)?
            }
            Method::Deflated => {
                let declared = usize::try_from(entry.size).unwrap_or(usize::MAX);
                let entry_budget = InflateBudget {
                    output: budget.inflate.output.min(declared),
                    ..budget.inflate
                };
                let mut window = Window {
                    source,
                    at: entry.data,
                    end: entry.data.saturating_add(entry.compressed),
                };
                inflate(&mut window, entry.compressed, &entry_budget, meter).map_err(|e| {
                    let kind = match e.kind {
                        InflateErrorKind::Stopped => ZipErrorKind::Stopped,
                        InflateErrorKind::OutOfMemory => ZipErrorKind::OutOfMemory,
                        InflateErrorKind::Source(s) => ZipErrorKind::Source(s),
                        k => ZipErrorKind::Inflate(k),
                    };
                    refused(kind, entry.data.saturating_add(e.offset))
                })?
            }
        };
        if len_u64(out.len()) != entry.size {
            return Err(refused(ZipErrorKind::Size, entry.data));
        }
        if crc32(&out) != entry.crc32 {
            return Err(refused(ZipErrorKind::Crc, entry.data));
        }
        Ok(out)
    }
}

/// The bytes `at..end` of a range source, read in order.
struct Window<'a, S: RangeSource + ?Sized> {
    source: &'a S,
    at: u64,
    end: u64,
}

impl<S: RangeSource + ?Sized> ByteSource for Window<'_, S> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        let left = usize::try_from(self.end.saturating_sub(self.at)).unwrap_or(usize::MAX);
        let into = buf
            .get_mut(..buf.len().min(left))
            .ok_or(SourceError::Overrun)?;
        if into.is_empty() {
            return Ok(0);
        }
        let n = self.source.read_at(self.at, into)?;
        self.at = self.at.saturating_add(len_u64(n));
        Ok(n)
    }
}
