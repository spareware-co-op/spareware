//! The fuzz harness of the record layer (30.1's standard bar; 24.7: every reader is a fuzz target),
//! driven here as property tests over arbitrary bytes, as `geom-op`'s and `geom-math`'s are. What it
//! proves, for every input:
//!
//! - no panic and no hang from [`Record::decode`], [`read`] (through a source that splits its reads
//!   at random) and [`salvage`];
//! - whatever [`Record::decode`] accepts is canonical: encoding the record gives back exactly the
//!   bytes it consumed;
//! - whatever [`read`] accepts with every record known is canonical: writing its header and records
//!   gives back exactly the file;
//! - bytes made by mutating a real file at random (a byte flipped, inserted or cut) are read or
//!   refused the same way however the reads split.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "a test harness; the rules are for kernel code"
)]

use geom_format::file::{HEADER_TAG, read, salvage, write};
use geom_format::record::{REQUIRED, Record};
use geom_format::source::{ByteSource, SliceSource, SourceError};
use geom_op::cbor::Encoder;
use geom_run::{Budget, WorkMeter};
use geom_test::rng::SplitMix64;
use geom_test::seed::Seed;
use proptest::prelude::*;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(2_000_000))
}

struct Split<'a> {
    rest: &'a [u8],
    g: SplitMix64,
}

impl ByteSource for Split<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        let most = 1 + (self.g.next_u64() % 7) as usize;
        let n = most.min(buf.len()).min(self.rest.len());
        buf[..n].copy_from_slice(&self.rest[..n]);
        self.rest = &self.rest[n..];
        Ok(n)
    }
}

/// A real file: a header and three records.
fn real_file() -> Vec<u8> {
    let record = |tag, n: u64| {
        let mut e = Encoder::new();
        e.array(2).unwrap();
        e.uint(n).unwrap();
        e.text("x").unwrap();
        Record {
            tag,
            flags: REQUIRED,
            schema: 1,
            depends: Vec::new(),
            payload: e.finish().unwrap().into_bytes(),
        }
    };
    write(
        &record(HEADER_TAG, 0),
        &[record(10, 1), record(11, 2), record(12, 3)],
        &mut meter(),
    )
    .unwrap()
}

fn check(bytes: &[u8], split_seed: u64) -> Result<(), TestCaseError> {
    if let Ok((record, len)) = Record::decode(bytes, &mut meter()) {
        prop_assert_eq!(record.encode(&mut meter()).unwrap(), bytes[..len].to_vec());
    }
    let whole = read(&mut SliceSource::new(bytes), |_, _| true, &mut meter());
    let mut split = Split {
        rest: bytes,
        g: SplitMix64::new(Seed(split_seed)),
    };
    prop_assert_eq!(&read(&mut split, |_, _| true, &mut meter()), &whole);
    if let Ok(file) = whole {
        prop_assert_eq!(
            write(&file.header, &file.records, &mut meter()).unwrap(),
            bytes.to_vec()
        );
    }
    let _ = salvage(bytes, &mut meter());
    Ok(())
}

#[test]
fn arbitrary_bytes_never_panic_and_only_canonical_bytes_pass() {
    let result = geom_test::prop::run(
        "record layer fuzz, arbitrary bytes",
        Seed(0x6392_f022),
        2000,
        &(proptest::collection::vec(any::<u8>(), 0..400), any::<u64>()),
        |(bytes, s)| check(&bytes, s),
    );
    if let Err(failure) = result {
        panic!("{failure}");
    }
}

#[test]
fn mutated_files_never_panic_and_read_alike_however_split() {
    let file = real_file();
    let len = file.len();
    let result = geom_test::prop::run(
        "record layer fuzz, mutated files",
        Seed(0x6392_f023),
        3000,
        &(0..len, any::<u8>(), 0_u8..3, any::<u64>()),
        |(at, byte, how, s)| {
            let mut bytes = file.clone();
            match how {
                0 => bytes[at] ^= byte | 1,
                1 => bytes.insert(at, byte),
                _ => {
                    bytes.remove(at);
                }
            }
            check(&bytes, s)
        },
    );
    if let Err(failure) = result {
        panic!("{failure}");
    }
}
