//! The fuzz harness of trust on open and the container (30.1's standard bar; 24.7: every reader is
//! a fuzz target), driven as property tests, as `fuzz_records.rs` drives the record layer. What it
//! proves, for every input:
//!
//! - no panic and no hang from [`Stamp::read`] on any payload, and whatever it accepts writes back
//!   to the same record;
//! - no panic and no hang from [`open`], at either trust, from [`Container::to_file`] or from
//!   [`Container::salvage`], whichever blob of a real container is changed (a byte flipped,
//!   inserted or cut), the document blob included;
//! - damage stays local: every body whose blobs were not changed opens exactly as it does from the
//!   intact container, and salvage names only the changed blob.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "a test harness; the rules are for kernel code"
)]

#[path = "../../geom-check/tests/common/mod.rs"]
mod rows;

use std::collections::BTreeSet;

use geom_format::body::write_body;
use geom_format::container::Container;
use geom_format::content::ContentId;
use geom_format::file::{read, write};
use geom_format::header::{Header, PRE_EDITION};
use geom_format::record::Record;
use geom_format::registry::knows;
use geom_format::source::SliceSource;
use geom_format::stamp::{STAMP_TAG, Stamp};
use geom_format::trust::{Checked, Checker, Level, State, Trust, open};
use geom_num::grid::Steps;
use geom_run::{Budget, Stopped, WorkMeter};
use geom_test::seed::Seed;
use geom_topo::{Body, BodyKind};
use proptest::prelude::*;
use rows::{cube, square_sheet};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(2_000_000))
}

/// A checker that passes every body: the harness is about reading, not about checking.
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

/// A real container: a stamped cube and an unstamped sheet, and each body's keys (its root, its
/// index and its chunks).
fn real() -> (Container, Vec<BTreeSet<ContentId>>) {
    let mut records = Vec::new();
    let mut keys = Vec::new();
    for (body, stamped) in [
        (cube().body(BodyKind::Solid), true),
        (square_sheet().body(BodyKind::Sheet), false),
    ] {
        let stored = write_body(&body, &mut meter()).unwrap();
        let mut mine: BTreeSet<ContentId> = stored.chunks.iter().map(Record::content_id).collect();
        mine.insert(stored.content_id());
        let stamp = Stamp {
            body: stored.content_id(),
            checker: 1,
            groups: vec![1, 4],
            edition: PRE_EDITION,
        };
        records.extend(stored.chunks);
        records.push(stored.index);
        if stamped {
            let r = stamp.record().unwrap();
            mine.insert(r.content_id());
            records.push(r);
        }
        keys.push(mine);
    }
    let h = Header::v1("geom-format fuzz", Steps(640)).record().unwrap();
    let bytes = write(&h, &records, &mut meter()).unwrap();
    let file = read(&mut SliceSource::new(&bytes), knows, &mut meter()).unwrap();
    (Container::from_file(&file, &mut meter()).unwrap(), keys)
}

#[test]
fn arbitrary_stamp_payloads_never_panic_and_only_stamps_pass() {
    let body = ContentId::of_payload(b"a body");
    let result = geom_test::prop::run(
        "stamp reader fuzz, arbitrary payloads",
        Seed(0x6108_0001),
        3000,
        &proptest::collection::vec(any::<u8>(), 0..80),
        |payload| {
            let r = Record {
                tag: STAMP_TAG,
                flags: 0,
                schema: 1,
                depends: vec![body.clone()],
                payload: payload.clone(),
            };
            if let Ok(s) = Stamp::read(&r, &mut meter()) {
                prop_assert_eq!(s.record().unwrap(), r);
            }
            Ok(())
        },
    );
    if let Err(failure) = result {
        panic!("{failure}");
    }
}

#[test]
fn a_changed_blob_never_panics_and_harms_only_its_own_body() {
    let (clean, keys) = real();
    let blob_keys: Vec<ContentId> = clean.blobs.keys().cloned().collect();
    let whole = [Trust::Known, Trust::Unknown].map(|t| {
        open(
            &clean,
            &clean.document,
            &clean.roots,
            t,
            &mut Pass,
            &mut meter(),
        )
        .unwrap()
    });
    let result = geom_test::prop::run(
        "trust on open fuzz, changed blobs",
        Seed(0x6108_0002),
        1500,
        &(0..=blob_keys.len(), any::<u16>(), any::<u8>(), 0_u8..3),
        |(which, at, byte, how)| {
            let mut c = clean.clone();
            let changed = blob_keys.get(which).cloned();
            let bytes = match &changed {
                Some(k) => c.blobs.get_mut(k).unwrap(),
                None => &mut c.document,
            };
            let at = usize::from(at) % bytes.len();
            match how {
                0 => bytes[at] ^= byte | 1,
                1 => bytes.insert(at, byte),
                _ => {
                    bytes.remove(at);
                }
            }
            for (t, clean_open) in [Trust::Known, Trust::Unknown].into_iter().zip(&whole) {
                let got = open(&c, &c.document, &c.roots, t, &mut Pass, &mut meter());
                match (&changed, got) {
                    (Some(k), Ok(opened)) => {
                        prop_assert_eq!(opened.entries.len(), clean_open.entries.len());
                        for (i, entry) in opened.entries.iter().enumerate() {
                            if !keys[i].contains(k) {
                                prop_assert_eq!(entry, &clean_open.entries[i]);
                            } else {
                                prop_assert!(
                                    matches!(entry.state, State::Damaged(_)),
                                    "{:?}",
                                    entry.state
                                );
                            }
                        }
                    }
                    (Some(_), Err(e)) => prop_assert!(false, "{e:?}"),
                    (None, _) => {}
                }
            }
            let _ = c.to_file(&mut meter());
            if let (Some(k), Ok(s)) = (&changed, c.salvage(&mut meter())) {
                prop_assert!(s.damaged.iter().all(|d| d.key == *k), "{:?}", s.damaged);
            }
            Ok(())
        },
    );
    if let Err(failure) = result {
        panic!("{failure}");
    }
}
