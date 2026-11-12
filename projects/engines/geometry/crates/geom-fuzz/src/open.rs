//! Fuzz target: `geom-format`'s containers and trust on open, on any file and any blob changed,
//! give the same answer twice, come back from a file to the same blobs, and keep damage with the
//! body it is in.
//!
//! What it does: [`decode`] splits the fuzzer's bytes into an [`Edit`] (which blob, where, which
//! byte, and whether it is flipped, inserted or cut) and a file. [`harness`] reads the file with
//! the registry's records, takes it into a `Container`, opens it at both trusts, then makes the
//! edit to one blob, or to the document blob, and opens and salvages it again. It also reads the
//! file's bytes as a stamp's payload, and every stamp the file holds, with `Stamp::read`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming it, with the input in
//!   hexadecimal. The laws (24.5 and 24.7, as the `container` and `trust` module headers state
//!   them): a container written to a file and read back is the same container; opening gives the
//!   same answer twice, at either trust; a changed blob leaves every body that does not need it
//!   exactly as it opened before, opens every body that does as damaged, and never stops a
//!   document that opened; salvage names only the changed blob; a stamp accepted writes back to
//!   the same record, and a stamp refused names an offset inside its payload.
//!
//! Method: the laws of `geom-format`'s `container`, `trust` and `stamp` module headers (chapter
//! 24.5 and 24.7, `projects/engines/geometry/architecture/24-file-format-exchange.md`), the
//! property harness of `geom-format`'s `tests/fuzz_open.rs` made a fuzz target, driven by
//! coverage-guided fuzzing (28.5, `projects/engines/geometry/architecture/28-testing.md`). The
//! checker passes every body, as that harness's does: the target is about reading, not checking.
//!
//! Cost: one read of the file, two writes of it and six opens, each linear in the file, under a work
//! meter of [`WORK`] units each.

use std::collections::{BTreeMap, BTreeSet};

use geom_format::cbor::CborError;
use geom_format::container::Container;
use geom_format::content::ContentId;
use geom_format::file::read;
use geom_format::registry::knows;
use geom_format::source::SliceSource;
use geom_format::stamp::{FLAGS, SCHEMA, STAMP_TAG, Stamp, StampError};
use geom_format::trust::{Checked, Checker, Entry, Level, OpenError, Opened, State, Trust, open};
use geom_run::{Budget, Stopped, WorkMeter};
use geom_test::{ByteSource, Entropy};
use geom_topo::Body;

use crate::Finding;
use crate::cbor::hex;

/// The work each read, write or open may do.
pub const WORK: u64 = 2_000_000;

/// The bytes of an [`Edit`], before the file.
pub const EDIT_BYTES: usize = 5;

/// The body a stamp read from the fuzzer's bytes must name.
pub const STAMP_BODY: &[u8] = b"a body";

/// One change to one blob.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edit {
    /// Which blob, in key order; one past the last is the document blob.
    pub which: u8,
    /// Where, modulo the blob's length.
    pub at: u16,
    /// The byte flipped in or inserted.
    pub byte: u8,
    /// 0 flips `byte | 1` in, 1 inserts `byte`, 2 cuts the byte at `at`.
    pub how: u8,
}

/// The edit, and the rest of `data` as a file.
#[must_use]
pub fn decode(data: &[u8]) -> (Edit, &[u8]) {
    let (head, file) = data.split_at(data.len().min(EDIT_BYTES));
    let mut src = ByteSource::new(head);
    let which = src.byte();
    let at = u16::from_le_bytes([src.byte(), src.byte()]);
    let edit = Edit {
        which,
        at,
        byte: src.byte(),
        how: src.byte() % 3,
    };
    (edit, file)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(WORK))
}

/// A checker that passes every body.
struct Pass;

impl Checker for Pass {
    type Report = ();

    fn version(&self) -> u64 {
        1
    }

    fn groups(&self) -> Vec<u64> {
        vec![1, 4]
    }

    fn check(
        &mut self,
        _: &Body,
        _: Level,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Checked<()>, Stopped> {
        meter.charge(1)?;
        Ok(Checked {
            ran: vec![1, 4],
            passed: true,
            report: (),
        })
    }
}

fn law(law: &str, data: &[u8], what: impl FnOnce() -> String) -> Finding {
    Finding::new(law, || format!("{}: {}", hex(data), what()))
}

fn opened(c: &Container, trust: Trust) -> Result<Opened<()>, OpenError> {
    open(c, &c.document, &c.roots, trust, &mut Pass, &mut meter())
}

/// The laws on a stamp read from `record`.
fn check_stamp(data: &[u8], record: &geom_format::record::Record) -> Result<(), Finding> {
    match Stamp::read(record, &mut meter()) {
        Ok(stamp) if stamp.record().as_ref() != Ok(record) => Err(law(
            "an accepted stamp writes back to its record",
            data,
            || format!("{record:?}"),
        )),
        Err(StampError::Cbor(CborError { offset, .. })) if offset > record.payload.len() => {
            Err(law("a stamp refusal is inside its payload", data, || {
                format!("{record:?} at {offset}")
            }))
        }
        _ => Ok(()),
    }
}

/// The keys each body root of `c` needs: the root, a stamp's body index, and the index's chunks.
fn needs(c: &Container, entries: &[Entry<()>]) -> BTreeMap<ContentId, BTreeSet<ContentId>> {
    let mut out = BTreeMap::new();
    for e in entries {
        let mut keys = BTreeSet::from([e.root.clone()]);
        let mut at = Some(e.root.clone());
        while let Some(key) = at.take() {
            let record = c
                .blobs
                .get(&key)
                .and_then(|b| geom_format::record::Record::decode(b, &mut meter()).ok());
            if let Some((r, _)) = record {
                keys.extend(r.depends.iter().cloned());
                if r.tag == STAMP_TAG {
                    at = r.depends.first().cloned();
                }
            }
        }
        out.insert(e.root.clone(), keys);
    }
    out
}

/// The fuzz harness: [`decode`], then the laws on the file and on the edited container.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    let (edit, file) = decode(data);
    let as_stamp = geom_format::record::Record {
        tag: STAMP_TAG,
        flags: FLAGS,
        schema: SCHEMA,
        depends: vec![ContentId::of_payload(STAMP_BODY)],
        payload: file.to_vec(),
    };
    check_stamp(data, &as_stamp)?;
    let Ok(read_file) = read(&mut SliceSource::new(file), knows, &mut meter()) else {
        return Ok(());
    };
    for r in read_file.records.iter().filter(|r| r.tag == STAMP_TAG) {
        check_stamp(data, r)?;
    }
    let Ok(clean) = Container::from_file(&read_file, &mut meter()) else {
        return Ok(());
    };
    let again = clean.to_file(&mut meter()).ok().and_then(|bytes| {
        let file = read(&mut SliceSource::new(&bytes), knows, &mut meter()).ok()?;
        Container::from_file(&file, &mut meter()).ok()
    });
    if again.as_ref() != Some(&clean) {
        return Err(law(
            "a container written and read back is itself",
            data,
            || format!("{again:?}"),
        ));
    }
    let mut changed = clean.clone();
    let keys: Vec<ContentId> = clean.blobs.keys().cloned().collect();
    let key = usize::from(edit.which)
        .checked_rem(keys.len().saturating_add(1))
        .and_then(|at| keys.get(at))
        .cloned();
    let bytes = match &key {
        Some(k) => changed.blobs.get_mut(k),
        None => Some(&mut changed.document),
    };
    if let Some(bytes) = bytes {
        let at = usize::from(edit.at)
            .checked_rem(bytes.len())
            .unwrap_or_default();
        match edit.how {
            0 => {
                if let Some(b) = bytes.get_mut(at) {
                    *b ^= edit.byte | 1;
                }
            }
            1 => bytes.insert(at, edit.byte),
            _ if at < bytes.len() => {
                bytes.remove(at);
            }
            _ => {}
        }
    }
    for trust in [Trust::Known, Trust::Unknown] {
        let before = opened(&clean, trust);
        if opened(&clean, trust) != before {
            return Err(law("a document opens the same twice", data, || {
                format!("{trust:?}")
            }));
        }
        let after = opened(&changed, trust);
        if opened(&changed, trust) != after {
            return Err(law("a changed document opens the same twice", data, || {
                format!("{trust:?}")
            }));
        }
        let (Ok(before), Some(k)) = (before, &key) else {
            continue;
        };
        local(data, k, &needs(&clean, &before.entries), &before, after)?;
    }
    let _ = changed.to_file(&mut meter());
    if let (Some(k), Ok(s)) = (&key, changed.salvage(&mut meter())) {
        if s.damaged.iter().any(|d| d.key != *k) {
            return Err(law("salvage names only the changed blob", data, || {
                format!("{:?}", s.damaged)
            }));
        }
    }
    Ok(())
}

/// Damage stays with the bodies that need the changed blob `k`.
fn local(
    data: &[u8],
    k: &ContentId,
    needs: &BTreeMap<ContentId, BTreeSet<ContentId>>,
    before: &Opened<()>,
    after: Result<Opened<()>, OpenError>,
) -> Result<(), Finding> {
    let after = match after {
        Ok(after) => after,
        Err(OpenError::Stopped(_)) => return Ok(()),
        Err(e) => {
            return Err(law("a changed blob never stops a document", data, || {
                format!("{e:?}")
            }));
        }
    };
    let clean: BTreeMap<&ContentId, &Entry<()>> =
        before.entries.iter().map(|e| (&e.root, e)).collect();
    for entry in &after.entries {
        let hit = needs.get(&entry.root).is_none_or(|keys| keys.contains(k));
        let holds = match clean.get(&entry.root) {
            _ if hit => matches!(entry.state, State::Damaged(_)),
            Some(was) => *was == entry,
            None => false,
        };
        if !holds || (hit && !needs.contains_key(&entry.root) && entry.root != *k) {
            return Err(law(
                "a changed blob harms only the bodies that need it",
                data,
                || format!("{entry:?}"),
            ));
        }
    }
    if after.entries.len() < before.entries.len() {
        return Err(law("every body still opens", data, || format!("{after:?}")));
    }
    Ok(())
}
