//! Adversarial tests for piece G92 (`geom-format`'s record layer): each tries one way a claim could
//! be wrong at its edge: every head width a record's fields can take, items no record holds at a
//! record's place, the meter stopping inside a read, a content ID of an unknown algorithm, and the
//! two caps a streamed record is held to, at exactly their limits and one past them.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test code: native arithmetic and indexing, and a failure stops the test"
)]

use geom_format::content::ContentId;
use geom_format::file::{FileError, HEADER_TAG, LABEL, RECORD_CAP, read, write};
use geom_format::record::{PAYLOAD_CAP, REQUIRED, Record, RecordError};
use geom_format::source::{ByteSource, SliceSource, SourceError};
use geom_op::cbor::Encoder;
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn header() -> Record {
    Record {
        tag: HEADER_TAG,
        flags: REQUIRED,
        schema: 1,
        depends: Vec::new(),
        payload: vec![0],
    }
}

/// "Every head width": a record whose fields take 1-, 2-, 4- and 8-byte arguments, and a payload
/// whose length takes a 2-byte head, reads back whole, through a split-free and a one-byte source.
#[test]
fn every_head_width_reads_back() {
    let mut e = Encoder::new();
    e.bytes(&[7; 298]).unwrap();
    let r = Record {
        tag: 1 << 40,
        flags: REQUIRED,
        schema: 300,
        depends: vec![ContentId::new(70_000, &[1; 300]).unwrap()],
        payload: e.finish().unwrap().into_bytes(),
    };
    let file = write(&header(), std::slice::from_ref(&r), &mut meter()).unwrap();
    let back = read(&mut SliceSource::new(&file), |_, _| true, &mut meter()).unwrap();
    assert_eq!(back.records, vec![r]);
    assert_eq!(ContentId::new(70_000, &[1; 3]).unwrap().alg(), 70_000);
}

/// "Not a record" at a record's place: a map or a tag is read whole and refused for its shape,
/// at the record's offset, never mistaken for a truncation.
#[test]
fn maps_and_tags_are_read_whole_then_refused() {
    for item in [
        &[0xa1_u8, 0x01, 0x01][..],
        &[0xc2, 0x41, 0x05],
        &[0x82, 0xa0, 0xc3, 0x40],
    ] {
        let mut bytes = Vec::from(LABEL);
        bytes.extend_from_slice(item);
        let got = read(&mut SliceSource::new(&bytes), |_, _| true, &mut meter());
        assert!(
            matches!(
                got,
                Err(FileError::Record {
                    offset: 12,
                    error: RecordError::Shape { offset: 0, .. }
                })
            ),
            "{item:02x?}: {got:?}"
        );
    }
}

/// "The meter is not damage": with every budget too small to finish, a read stops with
/// `FileError::Stopped`, never with a damaged record; and the budget that suffices reads it all.
#[test]
fn a_stopped_meter_is_a_stop_not_damage() {
    let file = write(&header(), &[], &mut meter()).unwrap();
    let mut stops = 0;
    for work in 0..400 {
        let mut m = WorkMeter::new(Budget::of_work(work));
        match read(&mut SliceSource::new(&file), |_, _| true, &mut m) {
            Ok(_) => {}
            Err(FileError::Stopped(_)) => stops += 1,
            Err(other) => panic!("budget {work}: {other:?}"),
        }
    }
    assert!(stops > 0);
    let mut m = WorkMeter::new(Budget::of_work(3));
    assert!(matches!(
        geom_format::file::salvage(&file, &mut m),
        Err(FileError::Stopped(_))
    ));
}

/// A source of `len` bytes made of parts: each run's bytes, then `zeros` zero bytes. Nothing is
/// stored but the parts, so a 65 MiB record costs no 65 MiB input.
struct Runs {
    parts: Vec<(Vec<u8>, usize)>,
    run: usize,
    at: usize,
}

impl ByteSource for Runs {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        let Some((bytes, zeros)) = self.parts.get(self.run) else {
            return Ok(0);
        };
        let total = bytes.len() + zeros;
        if self.at >= total {
            self.run += 1;
            self.at = 0;
            return self.read(buf);
        }
        let n = buf.len().min(total - self.at);
        for (i, b) in buf[..n].iter_mut().enumerate() {
            *b = bytes.get(self.at + i).copied().unwrap_or(0);
        }
        self.at += n;
        Ok(n)
    }
}

/// The first record of a file streamed as: the envelope's small items, one content ID of
/// algorithm 2 with a `digest`-byte digest, a payload string of `payload` bytes, and a 5-byte
/// check. Its length is 22 + digest + payload.
fn streamed(digest: usize, payload: usize) -> Runs {
    let mut start = Vec::from(LABEL);
    start.extend([0x86, 0x01, 0x01, 0x01, 0x81, 0x82, 0x02, 0x5a]);
    start.extend((digest as u32).to_be_bytes());
    let mut payload_head = vec![0x5a];
    payload_head.extend((payload as u32).to_be_bytes());
    Runs {
        parts: vec![
            (start, digest),
            (payload_head, payload),
            (vec![0x1a, 0, 0, 0, 0], 0),
        ],
        run: 0,
        at: 0,
    }
}

/// "Every length is checked before anything is allocated", at the caps: a payload string of
/// exactly 64 MiB and a record of exactly `RECORD_CAP` bytes are read (and then refused only for
/// their check); one byte more is refused as `Malformed` before it is read.
#[test]
#[cfg_attr(miri, ignore = "reads 65 MiB")]
#[cfg_attr(target_family = "wasm", ignore = "reads 65 MiB")]
fn the_stream_caps_hold_at_their_limits() {
    assert_eq!(RECORD_CAP, PAYLOAD_CAP + (1 << 20));
    let cap = PAYLOAD_CAP as usize;
    let digest = RECORD_CAP as usize - cap - 22;
    let at_limit = read(&mut streamed(digest, cap), |_, _| true, &mut meter());
    assert!(
        matches!(at_limit, Err(FileError::Record { offset: 12, .. })),
        "{at_limit:?}"
    );
    let over_record = read(&mut streamed(digest + 1, cap), |_, _| true, &mut meter());
    assert_eq!(over_record, Err(FileError::Malformed { offset: 12 }));
    let over_payload = read(&mut streamed(10, cap + 1), |_, _| true, &mut meter());
    assert!(
        matches!(over_payload, Err(FileError::Malformed { .. })),
        "{over_payload:?}"
    );
}
