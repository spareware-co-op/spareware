//! Trust on open (24.7, 23.11; piece G108): which check each stored body gets at each trust, the
//! full check run later body by body, stamps earned and read back, and bad bodies kept read-only
//! while the rest of the document opens. The checker is geom-check's, wired in as a program would
//! wire it ([`GeomCheck`]); a double ([`Scripted`]) stands in where a full check must fail, which
//! geom-check's groups 1 and 4 cannot do for a body their cheap run passed.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

#[path = "../../geom-check/tests/common/mod.rs"]
mod rows;

use geom_check::Group;
use geom_format::body::write_body;
use geom_format::container::{BlobError, Container, Damage};
use geom_format::content::ContentId;
use geom_format::file::{read, write};
use geom_format::header::{Header, PRE_EDITION};
use geom_format::record::{REQUIRED, Record};
use geom_format::registry::knows;
use geom_format::source::SliceSource;
use geom_format::stamp::{STAMP_TAG, Stamp, StampError};
use geom_format::trust::{
    Checked, Checker, Entry, Level, OpenError, Opened, State, Trust, Unread, open,
};
use geom_num::grid::Steps;
use geom_run::{Budget, Stopped, WorkMeter};
use geom_topo::{Body, BodyKind};
use rows::{certified, cube, square_sheet};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// geom-check, as a program passes it: its version, its built groups, and its check at the level
/// asked, recorded certified (the index stores no exactness class yet, 24.4).
struct GeomCheck {
    levels: Vec<Level>,
}

impl GeomCheck {
    fn new() -> Self {
        GeomCheck { levels: Vec::new() }
    }
}

impl Checker for GeomCheck {
    type Report = geom_check::Report;

    fn version(&self) -> u64 {
        geom_check::VERSION
    }

    fn groups(&self) -> Vec<u64> {
        Group::ALL
            .into_iter()
            .filter(|g| g.built())
            .map(|g| u64::from(g.number()))
            .collect()
    }

    fn check(
        &mut self,
        body: &Body,
        level: Level,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Checked<geom_check::Report>, Stopped> {
        self.levels.push(level);
        let at = match level {
            Level::Cheap => geom_check::Level::Cheap,
            Level::Full => geom_check::Level::Full,
        };
        let report = geom_check::check(body, certified(), at, meter)?;
        Ok(Checked {
            ran: report.ran().map(|g| u64::from(g.number())).collect(),
            passed: report.passed(),
            report,
        })
    }
}

/// A checker whose answers are scripted: its version and groups, and whether each level passes.
struct Scripted {
    version: u64,
    groups: Vec<u64>,
    cheap: bool,
    full: bool,
    ran: Vec<u64>,
}

impl Checker for Scripted {
    type Report = &'static str;

    fn version(&self) -> u64 {
        self.version
    }

    fn groups(&self) -> Vec<u64> {
        self.groups.clone()
    }

    fn check(
        &mut self,
        _: &Body,
        level: Level,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Checked<&'static str>, Stopped> {
        meter.charge(1)?;
        let passed = match level {
            Level::Cheap => self.cheap,
            Level::Full => self.full,
        };
        Ok(Checked {
            ran: self.ran.clone(),
            passed,
            report: if passed { "passed" } else { "failed" },
        })
    }
}

fn scripted(full: bool) -> Scripted {
    Scripted {
        version: 3,
        groups: vec![1, 2, 4],
        cheap: true,
        full,
        ran: vec![4, 2, 1, 2],
    }
}

fn header() -> Record {
    Header::v1("geom-format trust tests", Steps(640))
        .record()
        .unwrap()
}

/// The records of `body`: its chunks, then its index.
fn records(body: &Body) -> (ContentId, Vec<Record>) {
    let stored = write_body(body, &mut meter()).unwrap();
    let id = stored.content_id();
    let mut out = stored.chunks;
    out.push(stored.index);
    (id, out)
}

/// A stamp for the body `id`.
fn stamp(id: &ContentId, checker: u64, groups: &[u64]) -> Stamp {
    Stamp {
        body: id.clone(),
        checker,
        groups: groups.to_vec(),
        edition: PRE_EDITION,
    }
}

/// A body's stamps: each one's checker and groups.
type Stamps<'a> = &'a [(u64, &'a [u64])];

/// A container holding `bodies`, each followed by its stamps.
fn container(bodies: &[(&Body, Stamps<'_>)]) -> (Vec<ContentId>, Container) {
    let mut all = Vec::new();
    let mut ids = Vec::new();
    for (body, stamps) in bodies {
        let (id, recs) = records(body);
        all.extend(recs);
        for (checker, groups) in *stamps {
            all.push(stamp(&id, *checker, groups).record().unwrap());
        }
        ids.push(id);
    }
    let bytes = write(&header(), &all, &mut meter()).unwrap();
    let file = read(&mut SliceSource::new(&bytes), knows, &mut meter()).unwrap();
    (ids, Container::from_file(&file, &mut meter()).unwrap())
}

fn open_at<C: Checker>(c: &Container, trust: Trust, checker: &mut C) -> Opened<C::Report> {
    open(c, &c.document, &c.roots, trust, checker, &mut meter()).unwrap()
}

/// geom-check's version and the groups it has: what a stamp must say to spare the full check.
fn current() -> (u64, Vec<u64>) {
    let g = GeomCheck::new();
    (g.version(), g.groups())
}

#[test]
fn a_known_body_with_a_current_stamp_needs_only_the_cheap_check() {
    let (v, groups) = current();
    let body = cube().body(BodyKind::Solid);
    let (ids, c) = container(&[(&body, &[(v, &groups)])]);
    let mut checker = GeomCheck::new();
    let opened = open_at(&c, Trust::Known, &mut checker);
    assert_eq!(checker.levels, [Level::Cheap]);
    let e = &opened.entries[0];
    assert_eq!(e.state, State::Usable);
    assert_eq!(e.id.as_ref(), Some(&ids[0]));
    assert_eq!(e.stamp, Some(stamp(&ids[0], v, &groups)));
    assert!(!e.fresh);
    assert_eq!(opened.input(0), Some(&body));
    assert!(!opened.pending());
    assert_eq!(opened.fresh_stamps().count(), 0);
    // A stamp from a newer checker counts too.
    let (_, c) = container(&[(&body, &[(v + 1, &groups)])]);
    let opened = open_at(&c, Trust::Known, &mut GeomCheck::new());
    assert_eq!(opened.entries[0].state, State::Usable);
}

#[test]
fn a_body_the_stamp_does_not_cover_waits_for_the_full_check() {
    let (v, groups) = current();
    let body = cube().body(BodyKind::Solid);
    // No stamp, a stamp from an older checker, and one missing a group the checker has.
    let short: Vec<u64> = groups[..1].to_vec();
    for stamps in [&[][..], &[(v - 1, &groups[..])][..], &[(v, &short[..])][..]] {
        let (_, c) = container(&[(&body, stamps)]);
        let mut checker = GeomCheck::new();
        let mut opened = open_at(&c, Trust::Known, &mut checker);
        assert_eq!(opened.entries[0].state, State::Pending, "{stamps:?}");
        assert_eq!(opened.input(0), None);
        assert!(opened.pending());
        assert_eq!(
            opened.check_next(&mut checker, &mut meter()).unwrap(),
            Some(0)
        );
        assert_eq!(checker.levels, [Level::Cheap, Level::Full]);
        assert_eq!(opened.entries[0].state, State::Usable);
        assert_eq!(opened.input(0), Some(&body));
        assert!(!opened.pending());
        assert_eq!(opened.check_next(&mut checker, &mut meter()).unwrap(), None);
    }
}

#[test]
fn an_unknown_source_is_checked_in_full_whatever_its_stamps_say() {
    let (v, groups) = current();
    let body = cube().body(BodyKind::Solid);
    let (_, c) = container(&[(&body, &[(v + 7, &groups)])]);
    let opened = open_at(&c, Trust::Unknown, &mut GeomCheck::new());
    assert_eq!(opened.trust, Trust::Unknown);
    let e = &opened.entries[0];
    assert_eq!(e.state, State::Pending);
    assert_eq!(e.stamp, None);
    // The root is the stamp; the body is the one it names.
    assert_ne!(Some(&e.root), e.id.as_ref());
}

#[test]
fn a_full_check_earns_a_stamp_that_spares_the_next_open() {
    let body = cube().body(BodyKind::Solid);
    let (ids, c) = container(&[(&body, &[])]);
    let mut checker = GeomCheck::new();
    let mut opened = open_at(&c, Trust::Unknown, &mut checker);
    opened.check_next(&mut checker, &mut meter()).unwrap();
    let (v, groups) = current();
    let earned: Vec<&Stamp> = opened.fresh_stamps().collect();
    assert_eq!(earned, [&stamp(&ids[0], v, &groups)]);
    assert!(opened.entries[0].fresh);
    // Stored beside the body, the stamp makes the next known open cheap.
    let (_, recs) = records(&body);
    let mut all = recs;
    all.push(earned[0].record().unwrap());
    let bytes = write(&header(), &all, &mut meter()).unwrap();
    let file = read(&mut SliceSource::new(&bytes), knows, &mut meter()).unwrap();
    let c = Container::from_file(&file, &mut meter()).unwrap();
    let mut checker = GeomCheck::new();
    let opened = open_at(&c, Trust::Known, &mut checker);
    assert_eq!(opened.entries[0].state, State::Usable);
    assert_eq!(checker.levels, [Level::Cheap]);
}

#[test]
fn a_stamp_earned_lists_each_group_once_in_order() {
    let body = cube().body(BodyKind::Solid);
    let (ids, c) = container(&[(&body, &[])]);
    let mut checker = scripted(true);
    let mut opened = open_at(&c, Trust::Known, &mut checker);
    opened.check_next(&mut checker, &mut meter()).unwrap();
    assert_eq!(opened.entries[0].stamp, Some(stamp(&ids[0], 3, &[1, 2, 4])));
    assert!(opened.entries[0].stamp.as_ref().unwrap().record().is_ok());
}

#[test]
fn a_body_that_fails_a_check_is_read_only_and_the_rest_opens() {
    let mut broken = cube();
    broken.faces[4].next = None;
    let bad = broken.body(BodyKind::Solid);
    let good = square_sheet().body(BodyKind::Sheet);
    let (v, groups) = current();
    let (_, c) = container(&[(&bad, &[(v, &groups)]), (&good, &[(v, &groups)])]);
    let opened = open_at(&c, Trust::Known, &mut GeomCheck::new());
    let State::Failed(failed) = &opened.entries[0].state else {
        panic!("{:?}", opened.entries[0].state);
    };
    assert!(!failed.passed);
    assert!(!failed.report.passed());
    assert_eq!(opened.entries[0].body.as_ref(), Some(&bad));
    assert_eq!(opened.input(0), None);
    assert_eq!(opened.entries[1].state, State::Usable);
    assert_eq!(opened.input(1), Some(&good));
    assert_eq!(opened.input(2), None);
}

#[test]
fn a_failed_full_check_leaves_the_body_read_only() {
    let body = cube().body(BodyKind::Solid);
    let (_, c) = container(&[(&body, &[])]);
    let mut checker = scripted(false);
    let mut opened = open_at(&c, Trust::Known, &mut checker);
    assert_eq!(
        opened.check_next(&mut checker, &mut meter()).unwrap(),
        Some(0)
    );
    let State::Failed(failed) = &opened.entries[0].state else {
        panic!("{:?}", opened.entries[0].state);
    };
    assert_eq!(failed.report, "failed");
    assert_eq!(opened.entries[0].stamp, None);
    assert!(!opened.entries[0].fresh);
    assert_eq!(opened.input(0), None);
    assert_eq!(opened.fresh_stamps().count(), 0);
}

#[test]
fn a_stopped_full_check_leaves_the_body_pending() {
    let body = cube().body(BodyKind::Solid);
    let (_, c) = container(&[(&body, &[])]);
    let mut checker = scripted(true);
    let mut opened = open_at(&c, Trust::Known, &mut checker);
    let before = opened.clone();
    let mut tight = WorkMeter::new(Budget::of_work(0));
    assert!(opened.check_next(&mut checker, &mut tight).is_err());
    assert_eq!(opened, before);
}

#[test]
fn the_first_pending_body_is_checked_first() {
    let a = cube().body(BodyKind::Solid);
    let b = square_sheet().body(BodyKind::Sheet);
    let (_, c) = container(&[(&a, &[]), (&b, &[])]);
    let mut checker = GeomCheck::new();
    let mut opened = open_at(&c, Trust::Unknown, &mut checker);
    assert_eq!(
        opened.check_next(&mut checker, &mut meter()).unwrap(),
        Some(0)
    );
    assert_eq!(opened.entries[1].state, State::Pending);
    assert_eq!(
        opened.check_next(&mut checker, &mut meter()).unwrap(),
        Some(1)
    );
    assert_eq!(opened.check_next(&mut checker, &mut meter()).unwrap(), None);
}

/// The blob under the `n`th chunk key the body `id` lists, in its `depends` order.
fn chunk_key(c: &Container, id: &ContentId, n: usize) -> ContentId {
    let (index, _) = Record::decode(&c.blobs[id], &mut meter()).unwrap();
    index.depends[n].clone()
}

#[test]
fn damaged_records_open_their_body_with_no_geometry_and_the_rest_opens() {
    let a = cube().body(BodyKind::Solid);
    let b = square_sheet().body(BodyKind::Sheet);
    let (ids, clean) = container(&[(&a, &[]), (&b, &[])]);
    let key = chunk_key(&clean, &ids[0], 0);
    // A flipped byte, a missing blob, a record under another key, and a damaged index.
    let mut flipped = clean.clone();
    let blob = flipped.blobs.get_mut(&key).unwrap();
    let last = blob.len() - 3;
    blob[last] ^= 1;
    let mut missing = clean.clone();
    missing.blobs.remove(&key);
    let mut moved = clean.clone();
    let other = chunk_key(&clean, &ids[0], 1);
    let bytes = moved.blobs[&other].clone();
    moved.blobs.insert(key.clone(), bytes);
    let mut index = clean.clone();
    index.blobs.get_mut(&ids[0]).unwrap()[2] ^= 0xff;
    for (c, at, why) in [
        (&flipped, key.clone(), "a checksum"),
        (&missing, key.clone(), "a missing blob"),
        (&moved, key.clone(), "a key"),
        (&index, ids[0].clone(), "the index"),
    ] {
        let opened = open_at(c, Trust::Known, &mut GeomCheck::new());
        let e: &Entry<_> = &opened.entries[0];
        let State::Damaged(Unread::Blob(Damage { key: k, error })) = &e.state else {
            panic!("{why}: {:?}", e.state);
        };
        assert_eq!(k, &at, "{why}");
        match why {
            "a missing blob" => assert_eq!(error, &BlobError::Missing),
            "a key" => assert_eq!(error, &BlobError::Key),
            _ => assert!(matches!(error, BlobError::Record(_)), "{why}: {error:?}"),
        }
        assert_eq!(e.body, None);
        assert_eq!(opened.input(0), None);
        assert_eq!(opened.entries[1].state, State::Pending, "{why}");
    }
}

#[test]
fn a_body_that_does_not_build_back_is_damaged() {
    // An index whose chunk is another body's, stored under its own key: every blob is intact, but
    // the records are not one body.
    let a = cube().body(BodyKind::Solid);
    let b = square_sheet().body(BodyKind::Sheet);
    let (ids, mut c) = container(&[(&a, &[]), (&b, &[])]);
    let (index_b, _) = Record::decode(&c.blobs[&ids[1]], &mut meter()).unwrap();
    let (mut index_a, _) = Record::decode(&c.blobs[&ids[0]], &mut meter()).unwrap();
    index_a.depends = index_b.depends.clone();
    let key = index_a.content_id();
    c.blobs
        .insert(key.clone(), index_a.encode(&mut meter()).unwrap());
    c.roots = vec![key];
    let opened = open_at(&c, Trust::Known, &mut GeomCheck::new());
    assert!(
        matches!(opened.entries[0].state, State::Damaged(Unread::Body(_))),
        "{:?}",
        opened.entries[0].state
    );
}

#[test]
fn a_stamp_that_does_not_read_or_names_a_missing_body_is_damaged() {
    let body = cube().body(BodyKind::Solid);
    let (ids, c) = container(&[(&body, &[(1, &[1])])]);
    // Groups out of order: an intact record that is not a stamp.
    let bad = Record {
        tag: STAMP_TAG,
        flags: 0,
        schema: 1,
        depends: vec![ids[0].clone()],
        payload: {
            let mut e = geom_op::cbor::Encoder::new();
            e.array(4).unwrap();
            e.item(&ids[0].item().unwrap()).unwrap();
            e.uint(1).unwrap();
            e.array(2).unwrap();
            e.uint(4).unwrap();
            e.uint(1).unwrap();
            e.uint(0).unwrap();
            e.finish().unwrap().into_bytes()
        },
    };
    let mut broken = c.clone();
    let key = bad.content_id();
    broken
        .blobs
        .insert(key.clone(), bad.encode(&mut meter()).unwrap());
    broken.roots = vec![key];
    let opened = open_at(&broken, Trust::Known, &mut GeomCheck::new());
    assert!(
        matches!(opened.entries[0].state, State::Damaged(Unread::Stamp(_))),
        "{:?}",
        opened.entries[0].state
    );
    // A stamp whose body is not in the store.
    let mut lost = c.clone();
    lost.blobs.remove(&ids[0]);
    let opened = open_at(&lost, Trust::Known, &mut GeomCheck::new());
    let e = &opened.entries[0];
    assert_eq!(e.id.as_ref(), Some(&ids[0]));
    assert!(matches!(
        &e.state,
        State::Damaged(Unread::Blob(Damage {
            error: BlobError::Missing,
            ..
        }))
    ));
    // A root whose own blob is gone.
    let mut gone = c;
    let root = gone.roots[0].clone();
    gone.blobs.remove(&root);
    let opened = open_at(&gone, Trust::Known, &mut GeomCheck::new());
    assert_eq!(opened.entries[0].root, root);
    assert_eq!(opened.entries[0].id, None);
}

#[test]
fn roots_of_other_types_are_handed_over_skipped_or_refused() {
    let body = cube().body(BodyKind::Solid);
    let (_, mut c) = container(&[(&body, &[])]);
    let chunk = c
        .blobs
        .keys()
        .find(|k| !c.roots.contains(k))
        .unwrap()
        .clone();
    let optional = Record {
        tag: 9_999,
        flags: 0,
        schema: 1,
        depends: Vec::new(),
        payload: vec![0x00],
    };
    let required = Record {
        flags: REQUIRED,
        payload: vec![0x01],
        ..optional.clone()
    };
    for r in [&optional, &required] {
        c.blobs
            .insert(r.content_id(), r.encode(&mut meter()).unwrap());
    }
    let roots = vec![chunk.clone(), optional.content_id(), c.roots[0].clone()];
    let opened = open(
        &c,
        &c.document,
        &roots,
        Trust::Known,
        &mut GeomCheck::new(),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(opened.other.len(), 1);
    assert_eq!(opened.other[0].content_id(), chunk);
    assert_eq!(opened.skipped, [(optional.content_id(), 9_999)]);
    assert_eq!(opened.entries.len(), 1);
    let roots = vec![required.content_id()];
    let e = open(
        &c,
        &c.document,
        &roots,
        Trust::Known,
        &mut GeomCheck::new(),
        &mut meter(),
    )
    .unwrap_err();
    assert_eq!(
        e,
        OpenError::Unsupported {
            key: required.content_id(),
            tag: 9_999,
            schema: 1
        }
    );
    assert!(e.to_string().contains("9999"));
}

#[test]
fn a_document_blob_that_is_not_a_header_stops_the_open() {
    let body = cube().body(BodyKind::Solid);
    let (ids, c) = container(&[(&body, &[])]);
    let mut g = GeomCheck::new();
    let mut trailing = c.document.clone();
    trailing.push(0);
    let index = c.blobs[&ids[0]].clone();
    let mut flipped = c.document.clone();
    flipped[3] ^= 0x40;
    let bad_payload = Record {
        payload: vec![0x80],
        ..Record::decode(&c.document, &mut meter()).unwrap().0
    }
    .encode(&mut meter())
    .unwrap();
    for (document, want) in [
        (&trailing, "trailing"),
        (&index, "key"),
        (&flipped, "record"),
        (&bad_payload, "header"),
    ] {
        let e = open(&c, document, &c.roots, Trust::Known, &mut g, &mut meter()).unwrap_err();
        match (want, &e) {
            ("trailing", OpenError::Document(BlobError::Trailing))
            | ("key", OpenError::Document(BlobError::Key))
            | ("record", OpenError::Document(BlobError::Record(_)))
            | ("header", OpenError::Header(_)) => {}
            _ => panic!("{want}: {e:?}"),
        }
        assert_ne!(e.to_string(), "");
    }
}

#[test]
fn a_stopped_open_opens_nothing_and_never_reports_damage() {
    let (v, groups) = current();
    let a = cube().body(BodyKind::Solid);
    let b = square_sheet().body(BodyKind::Sheet);
    let (_, c) = container(&[(&a, &[(v, &groups)]), (&b, &[])]);
    let whole = open_at(&c, Trust::Known, &mut GeomCheck::new());
    let mut stops = 0;
    for work in (0..4_000).step_by(7) {
        let mut m = WorkMeter::new(Budget::of_work(work));
        match open(
            &c,
            &c.document,
            &c.roots,
            Trust::Known,
            &mut GeomCheck::new(),
            &mut m,
        ) {
            Ok(opened) => assert_eq!(opened, whole, "budget {work}"),
            Err(e) => {
                assert!(matches!(e, OpenError::Stopped(_)), "budget {work}: {e:?}");
                assert_ne!(e.to_string(), "");
                stops += 1;
            }
        }
    }
    assert!(stops > 10, "{stops}");
}

#[test]
fn a_stamp_reads_back_as_it_was_written() {
    let id = ContentId::of_payload(b"a body");
    let s = stamp(&id, 2, &[1, 3, 4]);
    let r = s.record().unwrap();
    assert_eq!((r.tag, r.flags, r.schema), (STAMP_TAG, 0, 1));
    assert!(!r.is_required() && !r.is_copy_always());
    assert_eq!(r.depends, std::slice::from_ref(&id));
    assert_eq!(Stamp::read(&r, &mut meter()).unwrap(), s);
    assert!(s.passed(1) && s.passed(3) && s.passed(4));
    assert!(!s.passed(2) && !s.passed(0) && !s.passed(5));
    // Two stamps about two bodies never share a content ID.
    let other = stamp(&ContentId::of_payload(b"another"), 2, &[1, 3, 4]);
    assert_ne!(r.content_id(), other.record().unwrap().content_id());
    // Groups out of order are refused both ways.
    for groups in [&[2u64, 1][..], &[1, 1][..]] {
        assert_eq!(
            stamp(&id, 2, groups).record(),
            Err(StampError::Shape("groups strictly ascending"))
        );
    }
}

#[test]
fn a_stamp_reader_refuses_what_is_not_one() {
    let id = ContentId::of_payload(b"a body");
    let r = stamp(&id, 2, &[1, 3, 4]).record().unwrap();
    let refused = |r: &Record| Stamp::read(r, &mut meter()).unwrap_err();
    for wrong in [
        Record {
            tag: 4,
            ..r.clone()
        },
        Record {
            schema: 2,
            ..r.clone()
        },
        Record {
            flags: REQUIRED,
            ..r.clone()
        },
    ] {
        assert_eq!(
            refused(&wrong),
            StampError::Shape("a stamp record of schema 1")
        );
    }
    let payload = |bytes: Vec<u8>| Record {
        payload: bytes,
        ..r.clone()
    };
    let with_id = |head: u8, tail: &[u8]| {
        let mut v = vec![head];
        v.extend_from_slice(id.item().unwrap().as_bytes());
        v.extend_from_slice(tail);
        payload(v)
    };
    let cases: Vec<(Record, &str)> = vec![
        (payload(vec![0x83, 0, 0, 0]), "a stamp of four fields"),
        (
            payload(vec![0x84, 0x83, 0, 0, 0, 0, 0x80, 0]),
            "a content ID of two fields",
        ),
        (
            payload(vec![0x84, 0x82, 0x01, 0x05, 0, 0x80, 0]),
            "a digest",
        ),
        (
            payload(vec![0x84, 0x82, 0x01, 0x43, 1, 2, 3, 0, 0x80, 0]),
            "a content ID",
        ),
        (with_id(0x84, &[0x40, 0x80, 0]), "an unsigned integer"),
        (with_id(0x84, &[0x02, 0x01, 0]), "an array of groups"),
        (
            with_id(0x84, &[0x02, 0x82, 0x03, 0x03, 0]),
            "groups strictly ascending",
        ),
        (with_id(0x85, &[0x02, 0x80, 0, 0]), "a stamp of four fields"),
        (
            Record {
                depends: vec![ContentId::of_payload(b"another")],
                ..r.clone()
            },
            "depends other than the stamped body",
        ),
        (
            Record {
                depends: Vec::new(),
                ..r.clone()
            },
            "depends other than the stamped body",
        ),
    ];
    for (record, why) in cases {
        assert_eq!(refused(&record), StampError::Shape(why), "{why}");
    }
    // Bytes after the stamp.
    let mut longer = r.payload.clone();
    longer.push(0x00);
    let e = refused(&Record {
        payload: longer,
        ..r.clone()
    });
    assert!(
        matches!(
            e,
            StampError::Shape("bytes after the stamp") | StampError::Cbor(_)
        ),
        "{e:?}"
    );
    // Bytes that are not canonical CBOR: a head longer than its value needs.
    let e = refused(&Record {
        payload: vec![0x84, 0x82, 0x18, 0x01],
        ..r.clone()
    });
    assert!(matches!(e, StampError::Cbor(_)), "{e:?}");
    assert_ne!(e.to_string(), "");
    assert_ne!(StampError::Shape("x").to_string(), "");
}

#[test]
fn unread_bodies_say_why() {
    let key = ContentId::of_payload(b"k");
    for u in [
        Unread::Blob(Damage {
            key,
            error: BlobError::Missing,
        }),
        Unread::Stamp(StampError::Shape("x")),
        Unread::Body(geom_format::body::BodyError::Shape("y")),
    ] {
        assert_ne!(u.to_string(), "");
    }
}
