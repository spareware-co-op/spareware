//! The records of a zip archive, as PKWARE's APPNOTE lays them out, read from byte slices: the end
//! of central directory record, the Zip64 end of central directory locator and record, central
//! directory headers, local file headers, and the Zip64 extended information extra field.
//!
//! What it does: each `read_*` function takes the bytes of one record (and, for a header, the
//! bytes after its fixed part) and returns its fields, or refuses a record whose signature,
//! lengths or values do not hold. [`find_end`] finds the end of central directory record in the
//! last bytes of an archive. Nothing here reads a source or allocates more than a name.
//!
//! Guarantees:
//!
//! - **No panics, no indexing, no unchecked arithmetic.** Every field is read with `get` from a
//!   slice whose length is checked first; every sum of lengths and offsets is checked.
//! - **Strict where a hardened reader must be.** The end record is the last one whose comment ends
//!   exactly at the end of the archive (no bytes after it); one disk only; a Zip64 locator must sit
//!   directly before the end record, and its record directly before the locator; a Zip64 value is
//!   read only for a field that holds the sentinel, and a Zip64 field shorter than the values it
//!   must hold is refused.
//!
//! Method: PKWARE's APPNOTE.TXT, version 6.3.10 (2022), sections 4.3.6 (local file header), 4.3.12
//! (central directory header), 4.3.14 to 4.3.16 (Zip64 end of central directory record and
//! locator, end of central directory record) and 4.5.3 (Zip64 extended information extra field),
//! <https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT>; the record layouts and
//! consistency checks of zip2 (below).
//!
//! Source: PORT from zip2, MIT,
//!   <https://github.com/zip-rs/zip2/blob/ddf3ec36017345b8936dd1a3516d9bc4632a0480/src/format/blocks.rs>
//!   <https://github.com/zip-rs/zip2/blob/ddf3ec36017345b8936dd1a3516d9bc4632a0480/src/format/find_central_directory.rs>
//!   <https://github.com/zip-rs/zip2/blob/ddf3ec36017345b8936dd1a3516d9bc4632a0480/src/extra_fields/zip64_extended_information.rs>
//!   Copyright (c) 2014 Mathijs van de Nes
//!   Changed for Spare Kit: the record layouts are read field by field from byte slices with `get`
//!   and `from_le_bytes`, in place of the source's `unsafe` plain-old-data casts of packed structs,
//!   so the code is `no_std` and safe; the end record must be the last whose comment ends exactly
//!   at the archive's end, where the source accepted garbage after the comment and searched for
//!   archives with prepended bytes; one disk only; the Zip64 record must abut its locator; errors
//!   are typed kinds with byte offsets.
//! SPDX-License-Identifier: Apache-2.0 AND MIT
//!
//! Cost: constant per record, plus a backward scan of at most 65,557 bytes for the end record. No
//! recursion.

use super::{ZipError, ZipErrorKind};

/// "PK\x05\x06".
pub(crate) const END_SIGNATURE: u32 = 0x0605_4b50;
/// "PK\x06\x07".
pub(crate) const ZIP64_LOCATOR_SIGNATURE: u32 = 0x0706_4b50;
/// "PK\x06\x06".
pub(crate) const ZIP64_END_SIGNATURE: u32 = 0x0606_4b50;
/// "PK\x01\x02".
pub(crate) const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
/// "PK\x03\x04".
pub(crate) const LOCAL_SIGNATURE: u32 = 0x0403_4b50;

/// The fixed part of each record, signature included.
pub(crate) const END_LEN: usize = 22;
pub(crate) const ZIP64_LOCATOR_LEN: usize = 20;
pub(crate) const ZIP64_END_LEN: usize = 56;
pub(crate) const CENTRAL_LEN: usize = 46;
pub(crate) const LOCAL_LEN: usize = 30;

/// The longest comment an end record can carry.
pub(crate) const MAX_COMMENT: usize = 65_535;

/// The Zip64 extended information extra field's header id.
const ZIP64_EXTRA: u16 = 0x0001;

fn refused(kind: ZipErrorKind, offset: u64) -> ZipError {
    ZipError { kind, offset }
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    let end = at.checked_add(2)?;
    let s = b.get(at..end)?;
    Some(u16::from_le_bytes([*s.first()?, *s.get(1)?]))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16_at(b, at)?) | (u32::from(u16_at(b, at.checked_add(2)?)?) << 16))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from(u32_at(b, at)?) | (u64::from(u32_at(b, at.checked_add(4)?)?) << 32))
}

fn offset_of(base: u64, at: usize) -> u64 {
    base.saturating_add(u64::try_from(at).unwrap_or(u64::MAX))
}

/// The end of central directory record (APPNOTE 4.3.16), widened by its Zip64 record if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct End {
    /// Where the end record starts.
    pub(crate) at: u64,
    /// Where the Zip64 record starts, if the archive has one: the central directory ends there.
    pub(crate) zip64_at: Option<u64>,
    /// Entries in the central directory.
    pub(crate) entries: u64,
    /// The central directory's size and offset.
    pub(crate) size: u64,
    pub(crate) offset: u64,
}

impl End {
    /// Where the central directory must end: the Zip64 record, or the end record.
    pub(crate) fn directory_end(&self) -> u64 {
        self.zip64_at.unwrap_or(self.at)
    }

    /// Whether a field holds its sentinel, so the Zip64 record holds the value.
    pub(crate) fn needs_zip64(entries: u16, size: u32, offset: u32) -> bool {
        entries == u16::MAX || size == u32::MAX || offset == u32::MAX
    }
}

/// Finds the end record in `tail`, the last bytes of an archive of `len` bytes starting at
/// `tail_at`: the last signature whose comment ends exactly at `len`.
pub(crate) fn find_end(tail: &[u8], tail_at: u64, len: u64) -> Result<(usize, End), ZipError> {
    let last = tail
        .len()
        .checked_sub(END_LEN)
        .ok_or(refused(ZipErrorKind::End, len))?;
    let mut at = last;
    loop {
        if u32_at(tail, at) == Some(END_SIGNATURE) {
            let comment = usize::from(u16_at(tail, at.saturating_add(20)).unwrap_or(u16::MAX));
            if at.checked_add(END_LEN).and_then(|e| e.checked_add(comment)) == Some(tail.len()) {
                let end = read_end(tail, at, tail_at)?;
                return Ok((at, end));
            }
        }
        if at == 0 {
            return Err(refused(ZipErrorKind::End, len));
        }
        at = at.saturating_sub(1);
    }
}

/// Reads the end record at `at` in `b` (whose first byte is at `base` in the archive).
fn read_end(b: &[u8], at: usize, base: u64) -> Result<End, ZipError> {
    let here = offset_of(base, at);
    let field = |o: usize| at.saturating_add(o);
    let bad = refused(ZipErrorKind::End, here);
    let disk = u16_at(b, field(4)).ok_or(bad)?;
    let cd_disk = u16_at(b, field(6)).ok_or(bad)?;
    let entries_here = u16_at(b, field(8)).ok_or(bad)?;
    let entries = u16_at(b, field(10)).ok_or(bad)?;
    let size = u32_at(b, field(12)).ok_or(bad)?;
    let offset = u32_at(b, field(16)).ok_or(bad)?;
    if disk != 0 || cd_disk != 0 || entries_here != entries {
        return Err(refused(ZipErrorKind::MultiDisk, here));
    }
    Ok(End {
        at: here,
        zip64_at: None,
        entries: u64::from(entries),
        size: u64::from(size),
        offset: u64::from(offset),
    })
}

/// Reads the Zip64 locator that must end where the end record starts, at `locator` in the
/// archive, from its 20 bytes `b`; returns where the Zip64 record starts.
pub(crate) fn read_zip64_locator(b: &[u8], locator: u64) -> Result<u64, ZipError> {
    let bad = refused(ZipErrorKind::Zip64, locator);
    if u32_at(b, 0) != Some(ZIP64_LOCATOR_SIGNATURE) {
        return Err(bad);
    }
    let disk = u32_at(b, 4).ok_or(bad)?;
    let at = u64_at(b, 8).ok_or(bad)?;
    let disks = u32_at(b, 16).ok_or(bad)?;
    if disk != 0 || disks > 1 {
        return Err(refused(ZipErrorKind::MultiDisk, locator));
    }
    Ok(at)
}

/// Reads the Zip64 record's fixed part `b` at `at`, which must end at `locator` (its extensible
/// data, if any, between), into `end`.
pub(crate) fn read_zip64_end(
    b: &[u8],
    at: u64,
    locator: u64,
    end: &mut End,
) -> Result<(), ZipError> {
    let bad = refused(ZipErrorKind::Zip64, at);
    if u32_at(b, 0) != Some(ZIP64_END_SIGNATURE) {
        return Err(bad);
    }
    let record = u64_at(b, 4).ok_or(bad)?;
    // The record's size counts what follows its first 12 bytes: at least the 44 fixed bytes.
    let fixed = u64::try_from(ZIP64_END_LEN.saturating_sub(12)).unwrap_or(u64::MAX);
    let ends = at.checked_add(12).and_then(|e| e.checked_add(record));
    if record < fixed || ends != Some(locator) {
        return Err(bad);
    }
    let disk = u32_at(b, 16).ok_or(bad)?;
    let cd_disk = u32_at(b, 20).ok_or(bad)?;
    let entries_here = u64_at(b, 24).ok_or(bad)?;
    let entries = u64_at(b, 32).ok_or(bad)?;
    if disk != 0 || cd_disk != 0 || entries_here != entries {
        return Err(refused(ZipErrorKind::MultiDisk, at));
    }
    end.zip64_at = Some(at);
    end.entries = entries;
    end.size = u64_at(b, 40).ok_or(bad)?;
    end.offset = u64_at(b, 48).ok_or(bad)?;
    Ok(())
}

/// The fields of a central directory header (APPNOTE 4.3.12) a reader keeps, Zip64 values applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Central {
    pub(crate) flags: u16,
    pub(crate) method: u16,
    pub(crate) crc32: u32,
    pub(crate) compressed: u64,
    pub(crate) size: u64,
    pub(crate) header: u64,
    pub(crate) name_len: usize,
    pub(crate) extra_len: usize,
    pub(crate) comment_len: usize,
}

impl Central {
    /// The header's whole length: fixed part, name, extra field and comment.
    pub(crate) fn len(&self) -> Option<usize> {
        CENTRAL_LEN
            .checked_add(self.name_len)?
            .checked_add(self.extra_len)?
            .checked_add(self.comment_len)
    }
}

/// Reads the central directory header at `at` of `b` (the whole directory, starting at `base` in
/// the archive): its fixed fields, then its Zip64 values from its extra field.
pub(crate) fn read_central(b: &[u8], at: usize, base: u64) -> Result<Central, ZipError> {
    let here = offset_of(base, at);
    let bad = refused(ZipErrorKind::Header, here);
    let field = |o: usize| at.saturating_add(o);
    if u32_at(b, at) != Some(CENTRAL_SIGNATURE) {
        return Err(bad);
    }
    let disk = u16_at(b, field(34)).ok_or(bad)?;
    if disk != 0 {
        return Err(refused(ZipErrorKind::MultiDisk, here));
    }
    let compressed32 = u32_at(b, field(20)).ok_or(bad)?;
    let size32 = u32_at(b, field(24)).ok_or(bad)?;
    let header32 = u32_at(b, field(42)).ok_or(bad)?;
    let mut c = Central {
        flags: u16_at(b, field(8)).ok_or(bad)?,
        method: u16_at(b, field(10)).ok_or(bad)?,
        crc32: u32_at(b, field(16)).ok_or(bad)?,
        compressed: u64::from(compressed32),
        size: u64::from(size32),
        header: u64::from(header32),
        name_len: usize::from(u16_at(b, field(28)).ok_or(bad)?),
        extra_len: usize::from(u16_at(b, field(30)).ok_or(bad)?),
        comment_len: usize::from(u16_at(b, field(32)).ok_or(bad)?),
    };
    let total = c.len().ok_or(bad)?;
    if field(total) > b.len() {
        return Err(bad);
    }
    let extra_at = field(CENTRAL_LEN).saturating_add(c.name_len);
    let extra = b
        .get(extra_at..extra_at.saturating_add(c.extra_len))
        .ok_or(bad)?;
    let wanted = [
        size32 == u32::MAX,
        compressed32 == u32::MAX,
        header32 == u32::MAX,
    ];
    if wanted.iter().any(|w| *w) {
        let values = read_zip64_extra(extra, wanted, here)?;
        let [size, compressed, header] = values;
        c.size = size.unwrap_or(c.size);
        c.compressed = compressed.unwrap_or(c.compressed);
        c.header = header.unwrap_or(c.header);
    }
    Ok(c)
}

/// The Zip64 values (uncompressed size, compressed size, local header offset, in that order) of
/// the fields `wanted` marks, from an extra field's blocks (APPNOTE 4.5.3): only a field that held
/// the sentinel is read, as zip2 does, and a block too short for them is refused.
fn read_zip64_extra(
    extra: &[u8],
    wanted: [bool; 3],
    here: u64,
) -> Result<[Option<u64>; 3], ZipError> {
    let bad = refused(ZipErrorKind::Zip64, here);
    let mut at = 0_usize;
    while at < extra.len() {
        let id = u16_at(extra, at).ok_or(bad)?;
        let len = usize::from(u16_at(extra, at.saturating_add(2)).ok_or(bad)?);
        let body_at = at.checked_add(4).ok_or(bad)?;
        let body = extra
            .get(body_at..body_at.checked_add(len).ok_or(bad)?)
            .ok_or(bad)?;
        if id == ZIP64_EXTRA {
            let mut out = [None; 3];
            let mut read = 0_usize;
            for (slot, want) in out.iter_mut().zip(wanted) {
                if want {
                    *slot = Some(u64_at(body, read).ok_or(bad)?);
                    read = read.saturating_add(8);
                }
            }
            return Ok(out);
        }
        at = body_at.saturating_add(len);
    }
    Err(bad)
}

/// The fields of a local file header (APPNOTE 4.3.7) checked against its central header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Local {
    pub(crate) flags: u16,
    pub(crate) method: u16,
    pub(crate) crc32: u32,
    pub(crate) compressed: u32,
    pub(crate) size: u32,
    pub(crate) name_len: usize,
    pub(crate) extra_len: usize,
}

/// Reads a local header's fixed 30 bytes `b`, at `here` in the archive.
pub(crate) fn read_local(b: &[u8], here: u64) -> Result<Local, ZipError> {
    let bad = refused(ZipErrorKind::LocalHeader, here);
    if u32_at(b, 0) != Some(LOCAL_SIGNATURE) {
        return Err(bad);
    }
    Ok(Local {
        flags: u16_at(b, 6).ok_or(bad)?,
        method: u16_at(b, 8).ok_or(bad)?,
        crc32: u32_at(b, 14).ok_or(bad)?,
        compressed: u32_at(b, 18).ok_or(bad)?,
        size: u32_at(b, 22).ok_or(bad)?,
        name_len: usize::from(u16_at(b, 26).ok_or(bad)?),
        extra_len: usize::from(u16_at(b, 28).ok_or(bad)?),
    })
}
