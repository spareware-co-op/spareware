//! The record layer through its public interface: CRC-32C against its published values, records
//! both ways and every refusal, files read through sources that split their reads at random,
//! unknown records, the END record, salvage, chunks, sources and range checks.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    reason = "test code: native arithmetic and indexing, and a failure stops the test"
)]

use geom_format::checksum::crc32c;
use geom_format::chunk::{CHUNK_ROWS, chunk, chunk_count};
use geom_format::content::{ContentId, SHA256};
use geom_format::file::{END_TAG, FileError, HEADER_TAG, LABEL, RangeCursor, read, salvage, write};
use geom_format::range::{
    NORMAL_BITS, OFFSET_BITS, OutOfRange, REAL_LIMIT, RangeError, bits, grid, grid_column, plane,
    real, real_column,
};
use geom_format::record::{COPY_ALWAYS, PAYLOAD_CAP, REQUIRED, Record, RecordError};
use geom_format::source::{
    ByteSource, RangeSource, SliceSource, SourceError, read_exact, read_exact_at,
};
use geom_num::grid::COORD_MAX;
use geom_op::cbor::Encoder;
use geom_op::hash::sha256;
use geom_run::{Budget, WorkMeter};
use geom_test::rng::SplitMix64;
use geom_test::seed::{Seed, resolve};

/// A budget far above what any test here spends, so a decoder that stops advancing ends in a
/// failed test, not a hang.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1_000_000))
}

/// A payload: one canonical item, the array `[n, "x"]`.
fn payload(n: u64) -> Vec<u8> {
    let mut e = Encoder::new();
    e.array(2).unwrap();
    e.uint(n).unwrap();
    e.text("x").unwrap();
    e.finish().unwrap().into_bytes()
}

fn record(tag: u64, flags: u64, n: u64) -> Record {
    Record {
        tag,
        flags,
        schema: 1,
        depends: Vec::new(),
        payload: payload(n),
    }
}

/// One item's canonical bytes.
fn one(f: impl FnOnce(&mut Encoder)) -> Vec<u8> {
    let mut e = Encoder::new();
    f(&mut e);
    e.finish().unwrap().into_bytes()
}

/// A record's bytes written field by field, the check as given, so tests can break any rule.
fn raw(tag: u64, flags: u64, depends: &[(u64, &[u8])], body: &[u8], check: Option<u64>) -> Vec<u8> {
    let mut five = Vec::new();
    for item in [tag, flags, 1] {
        five.extend(one(|e| e.uint(item).unwrap()));
    }
    five.extend(one(|e| {
        e.array(depends.len() as u64).unwrap();
        for (alg, digest) in depends {
            e.array(2).unwrap();
            e.uint(*alg).unwrap();
            e.bytes(digest).unwrap();
        }
    }));
    five.extend(one(|e| e.bytes(body).unwrap()));
    let check = check.unwrap_or_else(|| u64::from(crc32c(&five)));
    let mut out = vec![0x86];
    out.extend(&five);
    out.extend(one(|e| e.uint(check).unwrap()));
    out
}

/// A source that returns at most a random number of bytes per call.
struct Split<'a> {
    rest: &'a [u8],
    g: SplitMix64,
}

impl ByteSource for Split<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        let most = 1 + (self.g.next_u64() % 9) as usize;
        let n = most.min(buf.len()).min(self.rest.len());
        buf[..n].copy_from_slice(&self.rest[..n]);
        self.rest = &self.rest[n..];
        Ok(n)
    }
}

#[test]
fn crc32c_matches_its_published_values() {
    assert_eq!(crc32c(b"123456789"), 0xe306_9283);
    // RFC 3720, appendix B.4.
    assert_eq!(crc32c(&[0; 32]), 0x8a91_36aa);
    assert_eq!(crc32c(&[0xff; 32]), 0x62a8_ab43);
    let up: Vec<u8> = (0..32).collect();
    assert_eq!(crc32c(&up), 0x46dd_794e);
    let down: Vec<u8> = (0..32).rev().collect();
    assert_eq!(crc32c(&down), 0x113f_db5c);
}

#[test]
fn content_ids_are_sha256_of_the_payload() {
    let id = ContentId::of_payload(b"abc");
    assert_eq!(id.alg(), SHA256);
    assert_eq!(id.digest(), sha256(b"abc"));
    assert!(ContentId::new(SHA256, &[0; 31]).is_none());
    let other = ContentId::new(2, &[7; 5]).unwrap();
    assert_eq!(other.digest(), [7; 5]);
    assert_eq!(id.item().unwrap().as_bytes()[..4], [0x82, 0x01, 0x58, 0x20]);
}

#[test]
fn records_round_trip_and_are_canonical() {
    let mut r = record(10, REQUIRED | COPY_ALWAYS, 5);
    r.depends = vec![
        ContentId::of_payload(b"b"),
        ContentId::of_payload(b"a"),
        ContentId::new(2, &[1, 2]).unwrap(),
    ];
    let bytes = r.encode(&mut meter()).unwrap();
    let (back, len) = Record::decode(&bytes, &mut meter()).unwrap();
    assert_eq!(len, bytes.len());
    // depends comes back sorted by the bytes of each encoding.
    let mut sorted = r.depends.clone();
    sorted.sort_by_key(|d| d.item().unwrap().into_bytes());
    assert_eq!(back.depends, sorted);
    assert_eq!(back.payload, r.payload);
    assert!(back.is_required() && back.is_copy_always());
    assert!(!record(1, 0, 0).is_required() && !record(1, 0, 0).is_copy_always());
    assert_eq!(back.content_id(), ContentId::of_payload(&r.payload));
    // Bytes after a record are the caller's, not the record's.
    let mut more = bytes.clone();
    more.push(0);
    assert_eq!(Record::decode(&more, &mut meter()).unwrap().1, bytes.len());
    // The encoder and the field-by-field writer agree byte for byte.
    assert_eq!(
        record(3, 0, 9).encode(&mut meter()).unwrap(),
        raw(3, 0, &[], &payload(9), None)
    );
}

#[test]
fn records_refuse_what_breaks_the_envelope() {
    let mut m = meter();
    let good = raw(3, 1, &[], &payload(1), None);
    assert!(Record::decode(&good, &mut m).is_ok());
    // A wrong check, and a flipped payload byte the check catches.
    assert!(matches!(
        Record::decode(&raw(3, 1, &[], &payload(1), Some(7)), &mut m),
        Err(RecordError::Checksum { stored: 7, .. })
    ));
    let mut flipped = good.clone();
    let at = flipped.len() - 6;
    flipped[at] ^= 1;
    assert!(matches!(
        Record::decode(&flipped, &mut m),
        Err(RecordError::Checksum { .. })
    ));
    // Unknown flags, both ways.
    assert_eq!(
        Record::decode(&raw(3, 4, &[], &payload(1), None), &mut m),
        Err(RecordError::UnknownFlags(4))
    );
    assert_eq!(
        record(3, 8, 1).encode(&mut m),
        Err(RecordError::UnknownFlags(8))
    );
    // depends out of order, repeated, or a short SHA-256.
    let a = sha256(b"a");
    let b = sha256(b"b");
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    assert_eq!(
        Record::decode(&raw(3, 0, &[(1, &hi), (1, &lo)], &payload(1), None), &mut m),
        Err(RecordError::DependsOrder)
    );
    assert_eq!(
        Record::decode(&raw(3, 0, &[(1, &lo), (1, &lo)], &payload(1), None), &mut m),
        Err(RecordError::DependsOrder)
    );
    assert_eq!(
        Record::decode(&raw(3, 0, &[(1, &[0; 31])], &payload(1), None), &mut m),
        Err(RecordError::ContentId)
    );
    // A payload that is not one canonical item: a non-shortest head, or two items.
    for body in [&[0x18_u8, 0x05][..], &[0x01, 0x02]] {
        assert!(matches!(
            Record::decode(&raw(3, 0, &[], body, None), &mut m),
            Err(RecordError::Cbor(_))
        ));
        let mut bad = record(3, 0, 1);
        bad.payload = body.to_vec();
        assert!(matches!(bad.encode(&mut m), Err(RecordError::Cbor(_))));
    }
    // Not a record: the wrong array, or a field of the wrong type, each at its offset.
    assert!(matches!(
        Record::decode(&[0x85, 0, 0, 0, 0x80, 0x40], &mut m),
        Err(RecordError::Shape { offset: 0, .. })
    ));
    // Each a whole six-item array, so only the one field is wrong.
    for (bytes, offset) in [
        (&[0x86_u8, 0x60, 0, 1, 0x80, 0x40, 0][..], 1),
        (&[0x86, 1, 0x60, 1, 0x80, 0x40, 0], 2),
        (&[0x86, 1, 0, 0x60, 0x80, 0x40, 0], 3),
        (&[0x86, 1, 0, 1, 0x60, 0x40, 0], 4),
        (&[0x86, 1, 0, 1, 0x81, 0x83, 0, 0, 0, 0x40, 0], 5),
        (&[0x86, 1, 0, 1, 0x81, 0x82, 0x60, 0x40, 0x40, 0], 6),
        (&[0x86, 1, 0, 1, 0x81, 0x82, 2, 0x60, 0x40, 0], 7),
        (&[0x86, 1, 0, 1, 0x80, 0x60, 0], 5),
        (&[0x86, 1, 0, 1, 0x80, 0x41, 0, 0x60], 7),
    ] {
        assert_eq!(
            Record::decode(bytes, &mut m)
                .map(|_| ())
                .map_err(|e| match e {
                    RecordError::Shape { offset, .. } => offset,
                    _ => usize::MAX,
                }),
            Err(offset),
            "{bytes:02x?}"
        );
    }
    for e in [
        RecordError::DependsOrder,
        RecordError::ContentId,
        RecordError::PayloadTooLarge(1),
        RecordError::UnknownFlags(4),
        RecordError::Checksum {
            stored: 1,
            computed: 2,
        },
        RecordError::Shape {
            offset: 1,
            expected: "x",
        },
    ] {
        assert!(e.to_string().len() > 10);
    }
}

#[test]
#[cfg_attr(miri, ignore = "allocates 64 MiB")]
fn the_payload_cap_holds_both_ways() {
    let mut big = record(3, 0, 1);
    big.payload = vec![0; usize::try_from(PAYLOAD_CAP).unwrap() + 1];
    assert_eq!(
        big.encode(&mut meter()),
        Err(RecordError::PayloadTooLarge(PAYLOAD_CAP + 1))
    );
}

fn file() -> (Record, Vec<Record>, Vec<u8>) {
    let header = record(HEADER_TAG, REQUIRED, 0);
    let records = vec![
        record(10, REQUIRED, 1),
        record(11, 0, 2),
        record(10, REQUIRED | COPY_ALWAYS, 3),
    ];
    let bytes = write(&header, &records, &mut meter()).unwrap();
    (header, records, bytes)
}

#[test]
fn files_read_the_same_however_the_reads_split() {
    let (header, records, bytes) = file();
    assert_eq!(bytes[..12], LABEL);
    let whole = read(&mut SliceSource::new(&bytes), |t, _| t == 10, &mut meter()).unwrap();
    assert_eq!(whole.header, header);
    assert_eq!(whole.records, vec![records[0].clone(), records[2].clone()]);
    assert_eq!(whole.skipped.len(), 1);
    assert_eq!(whole.skipped[0].0, 11);
    let seed = resolve("G92 split reads", Seed(0x6392_0001)).unwrap();
    let mut g = SplitMix64::new(seed);
    for _ in 0..200 {
        let mut split = Split {
            rest: &bytes,
            g: SplitMix64::new(Seed(g.next_u64())),
        };
        assert_eq!(
            read(&mut split, |t, _| t == 10, &mut meter()).unwrap(),
            whole
        );
    }
    // Through a range source.
    let mut cursor = RangeCursor::new(&bytes[..], 0);
    assert_eq!(
        read(&mut cursor, |t, _| t == 10, &mut meter()).unwrap(),
        whole
    );
    // Every truncation fails, at the same place however the reads split.
    for cut in 0..bytes.len() {
        let one = read(
            &mut SliceSource::new(&bytes[..cut]),
            |_, _| true,
            &mut meter(),
        );
        let mut split = Split {
            rest: &bytes[..cut],
            g: SplitMix64::new(Seed(cut as u64)),
        };
        let two = read(&mut split, |_, _| true, &mut meter());
        assert!(one.is_err(), "{cut}");
        assert_eq!(one, two, "{cut}");
    }
}

#[test]
fn files_refuse_what_24_2_refuses() {
    let (header, records, bytes) = file();
    let all = |_, _| true;
    // An unknown required record stops the read, with its offset.
    let offset = 12 + header.encode(&mut meter()).unwrap().len() as u64;
    assert_eq!(
        read(&mut SliceSource::new(&bytes), |t, _| t == 11, &mut meter()),
        Err(FileError::UnsupportedRecord {
            tag: 10,
            schema: 1,
            offset
        })
    );
    // The label.
    let mut wrong = bytes.clone();
    wrong[7] ^= 1;
    assert_eq!(
        read(&mut SliceSource::new(&wrong), all, &mut meter()),
        Err(FileError::Label)
    );
    // Bytes after END.
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        read(&mut SliceSource::new(&trailing), all, &mut meter()),
        Err(FileError::TrailingBytes {
            offset: bytes.len() as u64
        })
    );
    // An END record whose count does not match.
    let mut lying = Vec::from(LABEL);
    for r in std::iter::once(&header).chain(&records) {
        lying.extend(r.encode(&mut meter()).unwrap());
    }
    let end = Record {
        tag: END_TAG,
        flags: REQUIRED,
        schema: 1,
        depends: Vec::new(),
        payload: one(|e| {
            e.array(2).unwrap();
            e.uint(3).unwrap();
            e.bytes(&sha256(&lying)).unwrap();
        }),
    };
    lying.extend(end.encode(&mut meter()).unwrap());
    assert_eq!(
        read(&mut SliceSource::new(&lying), all, &mut meter()),
        Err(FileError::End)
    );
    // A file that does not start with the header, and a header or END in the wrong place.
    assert!(matches!(
        write(&record(5, 0, 0), &[], &mut meter()),
        Err(FileError::Misplaced { tag: 5, .. })
    ));
    assert!(matches!(
        write(
            &record(HEADER_TAG, 1, 0),
            &[record(END_TAG, 1, 0)],
            &mut meter()
        ),
        Err(FileError::Misplaced { tag: END_TAG, .. })
    ));
    let mut headless = Vec::from(LABEL);
    headless.extend(record(10, 1, 0).encode(&mut meter()).unwrap());
    assert!(matches!(
        read(&mut SliceSource::new(&headless), all, &mut meter()),
        Err(FileError::Misplaced { tag: 10, .. })
    ));
    // A head no record holds, and a string over the cap, refused before reading on.
    let mut reserved = Vec::from(LABEL);
    reserved.push(0x1c);
    assert_eq!(
        read(&mut SliceSource::new(&reserved), all, &mut meter()),
        Err(FileError::Malformed { offset: 12 })
    );
    let mut huge = Vec::from(LABEL);
    huge.extend([
        0x86, 0x01, 0x01, 0x01, 0x80, 0x5b, 0, 0, 0, 0, 0x04, 0, 0, 1,
    ]);
    assert_eq!(
        read(&mut SliceSource::new(&huge), all, &mut meter()),
        Err(FileError::Malformed { offset: 17 })
    );
    for e in [
        FileError::Label,
        FileError::End,
        FileError::Malformed { offset: 1 },
        FileError::TrailingBytes { offset: 1 },
        FileError::Misplaced { tag: 1, offset: 2 },
    ] {
        assert!(e.to_string().len() > 5);
    }
}

#[test]
fn salvage_keeps_intact_records_and_names_the_damage() {
    let (header, records, bytes) = file();
    let clean = salvage(&bytes, &mut meter()).unwrap();
    assert!(clean.label);
    assert_eq!(clean.damaged, vec![]);
    assert_eq!(clean.records.len(), 5);
    assert_eq!(clean.records[0].1, header);
    // Break the second record's payload: it alone is lost, and its span is named.
    let start = 12 + header.encode(&mut meter()).unwrap().len();
    let len = records[0].encode(&mut meter()).unwrap().len();
    let mut broken = bytes.clone();
    broken[start + len - 6] ^= 0x40;
    let s = salvage(&broken, &mut meter()).unwrap();
    assert_eq!(s.records.len(), 4);
    assert_eq!(s.damaged, vec![(start as u64, (start + len) as u64)]);
    assert!(s.records.iter().all(|(_, r)| *r != records[0]));
    // No label: the records still come out.
    let s = salvage(&bytes[12..], &mut meter()).unwrap();
    assert!(!s.label);
    assert_eq!(s.records.len(), 5);
    // Damage at the end.
    let s = salvage(&bytes[..bytes.len() - 3], &mut meter()).unwrap();
    assert_eq!(s.records.len(), 4);
    assert_eq!(s.damaged.last().unwrap().1, (bytes.len() - 3) as u64);
}

#[test]
fn chunks_are_fixed_runs() {
    assert_eq!(CHUNK_ROWS, 4096);
    assert_eq!(chunk_count(0), 0);
    assert_eq!(chunk_count(1), 1);
    assert_eq!(chunk_count(4096), 1);
    assert_eq!(chunk_count(4097), 2);
    assert_eq!(chunk(0, 0), None);
    assert_eq!(chunk(4097, 0), Some(0..4096));
    assert_eq!(chunk(4097, 1), Some(4096..4097));
    assert_eq!(chunk(4097, 2), None);
    assert_eq!(chunk(4096, 1), None);
    assert_eq!(chunk(usize::MAX, usize::MAX), None);
    let rows = 3 * 4096 + 5;
    let covered = (0..chunk_count(rows)).fold(0, |n, i| n + chunk(rows, i).unwrap().len());
    assert_eq!(covered, rows);
}

#[test]
fn sources_fill_the_buffer_or_say_why() {
    /// A source that says it wrote more than it was given room for.
    struct Liar;
    impl ByteSource for Liar {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
            Ok(buf.len() + 1)
        }
    }
    let bytes: Vec<u8> = (0..20).collect();
    let mut buf = [0; 7];
    let mut s = SliceSource::new(&bytes);
    read_exact(&mut s, &mut buf).unwrap();
    assert_eq!(buf, [0, 1, 2, 3, 4, 5, 6]);
    let mut short = SliceSource::new(&bytes[..3]);
    assert_eq!(
        read_exact(&mut short, &mut buf),
        Err(SourceError::UnexpectedEnd)
    );
    assert_eq!(read_exact(&mut Liar, &mut buf), Err(SourceError::Overrun));
    read_exact_at(&bytes[..], 10, &mut buf).unwrap();
    assert_eq!(buf, [10, 11, 12, 13, 14, 15, 16]);
    assert_eq!(
        read_exact_at(&bytes[..], 14, &mut buf),
        Err(SourceError::OutOfBounds)
    );
    assert_eq!(
        read_exact_at(&bytes[..], u64::MAX, &mut buf),
        Err(SourceError::OutOfBounds)
    );
    assert_eq!(RangeSource::len(&bytes[..]), 20);
    assert!(!RangeSource::is_empty(&bytes[..]));
    for e in [
        SourceError::UnexpectedEnd,
        SourceError::Overrun,
        SourceError::OutOfBounds,
        SourceError::Failed(3),
    ] {
        assert!(e.to_string().len() > 5);
    }
}

#[test]
fn range_checks_hold_at_the_frame_edge() {
    let limit = COORD_MAX;
    assert_eq!(REAL_LIMIT, limit as f64);
    for v in [0, limit, -limit] {
        assert!(grid(v));
        assert!(real(v as f64));
    }
    for v in [limit + 1, -limit - 1, i64::MAX, i64::MIN] {
        assert!(!grid(v));
    }
    let next = f64::from_bits(REAL_LIMIT.to_bits() + 1);
    for x in [next, -next, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(!real(x));
    }
    assert_eq!(bits(0), 0);
    assert_eq!(bits(1), 1);
    assert_eq!(bits(-1), 1);
    assert_eq!(bits(i128::MIN), 128);
    assert_eq!(bits(i128::from(limit + 1)), 41);
    assert_eq!(grid_column(&[0, limit, -limit], 4, 100, "x"), Ok(()));
    assert_eq!(
        grid_column(&[0, limit + 1], 4, 100, "x"),
        Err(RangeError::OutOfRange(OutOfRange {
            record: 4,
            offset: 108,
            field: "x",
            bits: 41
        }))
    );
    assert_eq!(real_column(&[1.5, -REAL_LIMIT], 4, 0, "w"), Ok(()));
    let Err(RangeError::OutOfRange(o)) = real_column(&[1.0, 2.0, f64::NAN], 4, 8, "w") else {
        panic!("a NaN passed")
    };
    assert_eq!((o.offset, o.bits), (24, 1025));
    // Just past 2^40 − 1, still below 2^40: 40 bits, past the frame by a fraction; 2^40 needs 41.
    let Err(RangeError::OutOfRange(o)) = real_column(&[next], 4, 0, "w") else {
        panic!("a double past 2^40 - 1 passed")
    };
    assert_eq!(o.bits, 40);
    let Err(RangeError::OutOfRange(o)) = real_column(&[REAL_LIMIT + 1.0], 4, 0, "w") else {
        panic!("2^40 passed")
    };
    assert_eq!(o.bits, 41);
    // Planes: reduced and within their bits, or refused.
    let top = (1_i128 << NORMAL_BITS) - 1;
    let d = (1_i128 << OFFSET_BITS) - 1;
    assert_eq!(NORMAL_BITS, 83);
    assert_eq!(OFFSET_BITS, 125);
    assert_eq!(plane([top, 0, 0], d, 7, 0, "p"), Ok(()));
    assert_eq!(plane([0, 0, 1], 0, 7, 0, "p"), Ok(()));
    assert_eq!(plane([0, -1, 0], 3, 7, 0, "p"), Ok(()));
    assert!(matches!(
        plane([top + 1, 0, 0], 1, 7, 0, "p"),
        Err(RangeError::OutOfRange(o)) if o.bits == 84
    ));
    assert!(matches!(
        plane([1, 0, 0], d + 1, 7, 0, "p"),
        Err(RangeError::OutOfRange(o)) if o.bits == 126
    ));
    assert!(matches!(
        plane([2, 4, 0], 6, 7, 0, "p"),
        Err(RangeError::Malformed { .. })
    ));
    assert!(matches!(
        plane([0, 0, 0], 1, 7, 0, "p"),
        Err(RangeError::Malformed { .. })
    ));
    let e = RangeError::Malformed {
        record: 1,
        offset: 2,
        field: "p",
    };
    assert!(e.to_string().contains("reduced"));
    assert!(RangeError::OutOfRange(o).to_string().contains("41 bits"));
}
