//! Fuzz target: `geom-format`'s body records, read from any index and chunks, give the same answer
//! twice, refuse inside their input, and give back a body that writes and reads to itself.
//!
//! What it does: [`decode`] splits the fuzzer's bytes into payloads, each a two-byte little-endian
//! length and that many bytes: the first is a body index, the rest its chunks. [`harness`] makes
//! them a `StoredBody` and reads it with `read_body`. A chunk is found by its payload's content ID,
//! a SHA-256 digest no fuzzer forges, so before reading the harness writes each chunk's real digest
//! over the digest the index lists in its place, and sets the index's `depends` to those IDs, as
//! the writer does; the bytes around them stay exactly as the fuzzer gave them. The corpus starts
//! from bodies `write_body` stored, so the fuzzer edits real chunks and the reader sees them past
//! the hash.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming it, with the input in
//!   hexadecimal. The laws (24.2 to 24.4, as the `body` module header states them): the same
//!   verdict twice, from `read_body` and from `chunk_ids`; a refusal's offset inside the payload it
//!   read; a body accepted writes with `write_body`, reads back equal, index for index, and its
//!   records write again to the same records, a fixed point.
//!
//! Method: the laws of `geom-format`'s `body` module header (chapter 24.2 to 24.4,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`), driven by coverage-guided
//! fuzzing (28.5, `projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: two reads of the records, and for an accepted body one write and two more reads, each
//! linear in the body, under a work meter of [`WORK`] units each.

use geom_format::body::{
    BODY_TAG, BodyError, CHUNK_TAG, SCHEMA, StoredBody, chunk_ids, read_body, write_body,
};
use geom_format::cbor::CborError;
use geom_format::content::ContentId;
use geom_format::kinds::KindError;
use geom_format::record::{REQUIRED, Record};
use geom_run::{Budget, WorkMeter};

use crate::Finding;
use crate::cbor::hex;

/// The work each read or write may do.
pub const WORK: u64 = 4_000_000;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(WORK))
}

/// The payloads of `data`: each a two-byte little-endian length and that many bytes, the last cut
/// short where the input ends.
#[must_use]
pub fn decode(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut rest = data;
    while let Some((len, tail)) = rest.split_first_chunk::<2>() {
        let (payload, next) = tail.split_at(usize::from(u16::from_le_bytes(*len)).min(tail.len()));
        out.push(payload);
        rest = next;
    }
    out
}

/// The records of `stored` framed as [`decode`] reads them, index first: how the corpus is made.
#[must_use]
pub fn encode(stored: &StoredBody) -> Vec<u8> {
    let mut out = Vec::new();
    for r in [&stored.index].into_iter().chain(&stored.chunks) {
        let len = u16::try_from(r.payload.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend(r.payload.iter().take(usize::from(len)));
    }
    out
}

fn record(tag: u64, payload: &[u8], depends: Vec<ContentId>) -> Record {
    Record {
        tag,
        flags: REQUIRED,
        schema: SCHEMA,
        depends,
        payload: payload.to_vec(),
    }
}

/// The stored body of `data`, its index relinked to its chunks.
#[must_use]
pub fn stored(data: &[u8]) -> Option<StoredBody> {
    let payloads = decode(data);
    let (index, chunks) = payloads.split_first()?;
    let chunks: Vec<Record> = chunks
        .iter()
        .map(|p| record(CHUNK_TAG, p, Vec::new()))
        .collect();
    let mut index = record(BODY_TAG, index, Vec::new());
    let listed = chunk_ids(&index, &mut meter()).unwrap_or_default();
    let mut from = 0;
    for (old, chunk) in listed.iter().zip(&chunks) {
        let new = chunk.content_id();
        let width = old.digest().len();
        // The format lets an ID of an algorithm other than SHA-256 carry any digest, the empty one
        // included (24.2, `content-id = [alg: uint, digest: bstr]`; `ContentId::new` keeps it as
        // read). An empty digest has no place in the index to find, and no slot is rewritten that
        // is not a SHA-256 digest's 32 bytes, so it is passed by and the search goes on from here.
        if width == 0 {
            continue;
        }
        let found = index
            .payload
            .get(from..)
            .and_then(|tail| tail.windows(width).position(|w| w == old.digest()));
        let Some(at) = found.and_then(|at| at.checked_add(from)) else {
            break;
        };
        let end = at.saturating_add(width);
        if let Some(slot) = index
            .payload
            .get_mut(at..end)
            .filter(|s| s.len() == new.digest().len())
        {
            slot.copy_from_slice(new.digest());
        }
        from = end;
    }
    let mut depends = chunk_ids(&index, &mut meter()).unwrap_or_default();
    depends.sort_by_key(|id| {
        id.item()
            .map(geom_op::cbor::Item::into_bytes)
            .unwrap_or_default()
    });
    depends.dedup();
    index.depends = depends;
    Some(StoredBody { index, chunks })
}

/// The offset a body refusal names, inside the payload it read.
fn offset(e: &BodyError) -> Option<usize> {
    match e {
        BodyError::Cbor(CborError { offset, .. })
        | BodyError::Kind(KindError::Cbor(CborError { offset, .. })) => Some(*offset),
        _ => None,
    }
}

fn law(law: &str, data: &[u8], what: impl FnOnce() -> String) -> Finding {
    Finding::new(law, || format!("{}: {}", hex(data), what()))
}

/// The laws on the stored body `s`, made from `data`.
///
/// # Errors
///
/// A [`Finding`] naming the law `s` broke.
pub fn check(data: &[u8], s: &StoredBody) -> Result<(), Finding> {
    let ids = chunk_ids(&s.index, &mut meter());
    if chunk_ids(&s.index, &mut meter()) != ids {
        return Err(law(
            "an index lists the same chunks twice",
            data,
            String::new,
        ));
    }
    let body = read_body(s, &mut meter());
    if read_body(s, &mut meter()) != body {
        return Err(law("a body reads the same twice", data, String::new));
    }
    let longest = [&s.index]
        .into_iter()
        .chain(&s.chunks)
        .map(|r| r.payload.len())
        .max();
    let body = match body {
        Err(e) if offset(&e).zip(longest).is_some_and(|(at, most)| at > most) => {
            return Err(law("a body refusal is inside its payload", data, || {
                format!("{e:?}")
            }));
        }
        Err(_) => return Ok(()),
        Ok(body) => body,
    };
    let again = write_body(&body, &mut meter());
    let back = again.as_ref().ok().map(|a| read_body(a, &mut meter()));
    match (&again, back) {
        (Ok(a), Some(Ok(b))) if b == body => {
            if write_body(&b, &mut meter()).as_ref() == Ok(a) {
                Ok(())
            } else {
                Err(law(
                    "a body's records write again to the same records",
                    data,
                    String::new,
                ))
            }
        }
        (a, b) => Err(law(
            "an accepted body writes and reads back equal",
            data,
            || format!("{a:?}\n{b:?}"),
        )),
    }
}

/// The fuzz harness: [`stored`], then [`check`].
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    match stored(data) {
        Some(s) => check(data, &s),
        None => Ok(()),
    }
}
