//! The reference container (24.5; piece G108): a file to blobs and back byte for byte, blobs that
//! verify themselves, records shared between bodies stored once, and salvage that keeps every
//! intact record of a damaged container.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

#[path = "../../geom-check/tests/common/mod.rs"]
mod rows;

use std::collections::BTreeSet;

use geom_format::body::{BODY_TAG, StoredBody, read_body, write_body};
use geom_format::container::{BlobError, Blobs, Container, ContainerError, Damage, read_record};
use geom_format::content::ContentId;
use geom_format::file::{RECORD_CAP, ReadFile, read, write};
use geom_format::header::{Header, PRE_EDITION};
use geom_format::record::{REQUIRED, Record};
use geom_format::registry::knows;
use geom_format::source::{RangeSource, SliceSource, SourceError};
use geom_format::stamp::Stamp;
use geom_num::grid::Steps;
use geom_run::{Budget, WorkMeter};
use geom_topo::{Body, BodyKind};
use rows::{cube, square_sheet};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn header() -> Record {
    Header::v1("geom-format container tests", Steps(640))
        .record()
        .unwrap()
}

fn file_of(records: &[Record]) -> Vec<u8> {
    write(&header(), records, &mut meter()).unwrap()
}

fn read_file(bytes: &[u8]) -> ReadFile {
    read(&mut SliceSource::new(bytes), knows, &mut meter()).unwrap()
}

fn container_of(bytes: &[u8]) -> Container {
    Container::from_file(&read_file(bytes), &mut meter()).unwrap()
}

/// A body's records as the body writer orders them: its chunks, then its index.
fn body_records(body: &Body) -> Vec<Record> {
    let stored = write_body(body, &mut meter()).unwrap();
    let mut out = stored.chunks;
    out.push(stored.index);
    out
}

fn stamp_of(body: &Body) -> Record {
    Stamp {
        body: write_body(body, &mut meter()).unwrap().content_id(),
        checker: 1,
        groups: vec![1, 4],
        edition: PRE_EDITION,
    }
    .record()
    .unwrap()
}

/// The records of a file, as a set of their bytes.
fn record_set(bytes: &[u8]) -> BTreeSet<Vec<u8>> {
    read_file(bytes)
        .records
        .iter()
        .map(|r| r.encode(&mut meter()).unwrap())
        .collect()
}

/// The archive: every file ever written as an example of a format version.
fn archive() -> Vec<(&'static str, &'static [u8])> {
    vec![
        (
            "v1/header-only.skgf",
            include_bytes!("archive/v1/header-only.skgf"),
        ),
        (
            "v1/stamped-bodies.skgf",
            include_bytes!("archive/v1/stamped-bodies.skgf"),
        ),
        (
            "v1/wall-body.skgf",
            include_bytes!("archive/v1/wall-body.skgf"),
        ),
    ]
}

#[test]
fn every_archived_file_goes_to_a_container_and_back_byte_for_byte() {
    for (name, bytes) in archive() {
        let c = container_of(bytes);
        assert_eq!(c.to_file(&mut meter()).unwrap(), bytes, "{name}");
        // And the container comes back from that file as it was.
        assert_eq!(container_of(&c.to_file(&mut meter()).unwrap()), c, "{name}");
    }
}

#[test]
fn a_container_holds_each_record_under_its_content_id_and_the_roots_in_order() {
    let a = cube().body(BodyKind::Solid);
    let b = square_sheet().body(BodyKind::Sheet);
    let mut records = body_records(&a);
    records.push(stamp_of(&a));
    records.extend(body_records(&b));
    let c = container_of(&file_of(&records));
    let (h, _) = Record::decode(&c.document, &mut meter()).unwrap();
    assert_eq!(h, header());
    assert_eq!(c.blobs.len(), records.len());
    for r in &records {
        let key = r.content_id();
        assert_eq!(c.blobs[&key], r.encode(&mut meter()).unwrap());
        assert_eq!(read_record(&c, &key, &mut meter()).unwrap(), *r);
    }
    // The roots: a's stamp (a's index is its dependency) and b's index.
    let b_index = records.last().unwrap().content_id();
    assert_eq!(c.roots, [stamp_of(&a).content_id(), b_index]);
}

#[test]
fn a_file_in_another_order_comes_back_as_the_same_records_in_the_writers_order() {
    let a = cube().body(BodyKind::Solid);
    let mut records = body_records(&a);
    let canonical = file_of(&records);
    records.rotate_right(1); // the index first, then its chunks
    let shuffled = file_of(&records);
    assert_ne!(shuffled, canonical);
    let back = container_of(&shuffled).to_file(&mut meter()).unwrap();
    assert_eq!(back, canonical);
    assert_eq!(record_set(&back), record_set(&shuffled));
}

#[test]
fn chunks_two_bodies_share_are_stored_and_written_once() {
    // The same rows as a solid and as a general body: two indexes over the same chunks.
    let solid = cube().body(BodyKind::Solid);
    let general = cube().body(BodyKind::General);
    let first = body_records(&solid);
    let second = body_records(&general);
    let chunks = first.len() - 1;
    assert_eq!(first[..chunks], second[..chunks]);
    let mut once = first.clone();
    once.push(second[chunks].clone());
    let bytes = file_of(&once);
    let c = container_of(&bytes);
    assert_eq!(c.blobs.len(), chunks + 2);
    assert_eq!(c.roots.len(), 2);
    assert_eq!(c.to_file(&mut meter()).unwrap(), bytes);
    // A file that repeats them goes back to holding them once.
    let mut twice = first;
    twice.extend(second);
    let back = container_of(&file_of(&twice))
        .to_file(&mut meter())
        .unwrap();
    assert_eq!(back, bytes);
    // Both bodies read back from the container's records.
    let file = read_file(&back);
    for (index, body) in [(chunks, &solid), (chunks + 1, &general)] {
        let index = file.records[index].clone();
        let stored = StoredBody {
            chunks: index
                .depends
                .iter()
                .map(|k| read_record(&c, k, &mut meter()).unwrap())
                .collect(),
            index,
        };
        assert_eq!(read_body(&stored, &mut meter()).unwrap(), *body);
    }
}

#[test]
fn two_records_with_one_content_id_are_refused() {
    let r = Record {
        tag: 9_998,
        flags: 0,
        schema: 1,
        depends: Vec::new(),
        payload: vec![0x00],
    };
    let twin = Record {
        tag: 9_997,
        ..r.clone()
    };
    let file = ReadFile {
        header: header(),
        records: vec![r.clone(), twin],
        skipped: Vec::new(),
    };
    let e = Container::from_file(&file, &mut meter()).unwrap_err();
    assert_eq!(e, ContainerError::Collision(r.content_id()));
    assert!(!e.to_string().is_empty());
}

#[test]
fn a_record_whose_dependency_the_file_lacks_is_refused() {
    let a = cube().body(BodyKind::Solid);
    let s = stamp_of(&a);
    let file = ReadFile {
        header: header(),
        records: vec![s.clone()],
        skipped: Vec::new(),
    };
    let e = Container::from_file(&file, &mut meter()).unwrap_err();
    assert_eq!(
        e,
        ContainerError::Blob(Damage {
            key: s.depends[0].clone(),
            error: BlobError::Missing,
        })
    );
    assert!(!e.to_string().is_empty());
}

/// A container of a cube and a sheet, and the first chunk key of the cube's index.
fn two_bodies() -> (Container, ContentId, ContentId) {
    let a = cube().body(BodyKind::Solid);
    let b = square_sheet().body(BodyKind::Sheet);
    let mut records = body_records(&a);
    let index = records.last().unwrap().content_id();
    let chunk = records[0].content_id();
    records.extend(body_records(&b));
    (container_of(&file_of(&records)), index, chunk)
}

#[test]
fn a_file_is_not_written_from_a_damaged_container() {
    let (c, index, chunk) = two_bodies();
    let mut missing = c.clone();
    missing.blobs.remove(&chunk);
    assert_eq!(
        missing.to_file(&mut meter()).unwrap_err(),
        ContainerError::Blob(Damage {
            key: chunk.clone(),
            error: BlobError::Missing
        })
    );
    let mut flipped = c.clone();
    let blob = flipped.blobs.get_mut(&index).unwrap();
    let at = blob.len() - 2;
    blob[at] ^= 1;
    let e = flipped.to_file(&mut meter()).unwrap_err();
    assert!(
        matches!(&e, ContainerError::Blob(Damage { key, error: BlobError::Record(_) }) if *key == index),
        "{e:?}"
    );
    // The document blob: damaged, not a header, or with bytes after it.
    let mut document = c.clone();
    document.document[3] ^= 0x40;
    let mut body = c.clone();
    body.document = c.blobs[&index].clone();
    let mut trailing = c.clone();
    trailing.document.push(0);
    for (bad, want) in [
        (&document, "record"),
        (&body, "key"),
        (&trailing, "trailing"),
    ] {
        for e in [
            bad.to_file(&mut meter()).unwrap_err(),
            bad.salvage(&mut meter()).unwrap_err(),
        ] {
            match (want, &e) {
                ("record", ContainerError::Document(BlobError::Record(_)))
                | ("key", ContainerError::Document(BlobError::Key))
                | ("trailing", ContainerError::Document(BlobError::Trailing)) => {}
                _ => panic!("{want}: {e:?}"),
            }
            assert!(!e.to_string().is_empty());
        }
    }
}

#[test]
fn salvage_keeps_every_intact_record_and_names_the_damage() {
    let (c, index, chunk) = two_bodies();
    // Intact: the file to_file writes, and no damage.
    let whole = c.salvage(&mut meter()).unwrap();
    assert_eq!(whole.file, c.to_file(&mut meter()).unwrap());
    assert!(whole.damaged.is_empty());
    // A damaged chunk: every other record is kept.
    let mut flipped = c.clone();
    let blob = flipped.blobs.get_mut(&chunk).unwrap();
    let at = blob.len() - 2;
    blob[at] ^= 1;
    let s = flipped.salvage(&mut meter()).unwrap();
    assert_eq!(s.damaged.len(), 1);
    assert_eq!(s.damaged[0].key, chunk);
    assert!(matches!(s.damaged[0].error, BlobError::Record(_)));
    let kept = record_set(&s.file);
    assert_eq!(kept.len(), c.blobs.len() - 1);
    for (key, bytes) in &c.blobs {
        assert_eq!(kept.contains(bytes), *key != chunk);
    }
    // A damaged index: its chunks, which nothing else reaches, are kept too.
    let mut lost = c.clone();
    let blob = lost.blobs.get_mut(&index).unwrap();
    let at = blob.len() - 2;
    blob[at] ^= 1;
    let s = lost.salvage(&mut meter()).unwrap();
    assert_eq!(s.damaged.len(), 1);
    assert_eq!(s.damaged[0].key, index);
    assert_eq!(record_set(&s.file).len(), c.blobs.len() - 1);
    // A missing root is named; a damaged blob nothing reaches is named once.
    let mut gone = c.clone();
    gone.blobs.remove(&index);
    gone.blobs
        .insert(ContentId::of_payload(b"stray"), vec![0xff]);
    let s = gone.salvage(&mut meter()).unwrap();
    let named: Vec<(&ContentId, &BlobError)> =
        s.damaged.iter().map(|d| (&d.key, &d.error)).collect();
    assert_eq!(named.len(), 2, "{named:?}");
    assert!(named.contains(&(&index, &BlobError::Missing)));
    assert!(
        named
            .iter()
            .any(|(k, e)| **k == ContentId::of_payload(b"stray")
                && matches!(e, BlobError::Record(_)))
    );
}

#[test]
fn a_body_index_whose_list_does_not_read_keeps_its_dependencies_first() {
    // An intact record of type 3 whose payload is not an index: the file still carries it, after
    // its dependencies in `depends` order.
    let a = cube().body(BodyKind::Solid);
    let chunks: Vec<Record> = write_body(&a, &mut meter()).unwrap().chunks;
    let mut depends: Vec<ContentId> = chunks.iter().map(Record::content_id).collect();
    depends.sort_by_key(|id| id.item().unwrap().into_bytes());
    let odd = Record {
        tag: BODY_TAG,
        flags: REQUIRED,
        schema: 1,
        depends: depends.clone(),
        payload: vec![0x00],
    };
    let mut records = chunks;
    records.push(odd.clone());
    let c = container_of(&file_of(&records));
    let back = read_file(&c.to_file(&mut meter()).unwrap());
    let order: Vec<ContentId> = back.records.iter().map(Record::content_id).collect();
    let mut want = depends;
    want.push(odd.content_id());
    assert_eq!(order, want);
}

/// A store whose one blob is `blob`.
struct One<S>(S);

impl<S: RangeSource> Blobs for One<S> {
    type Blob<'a>
        = &'a S
    where
        S: 'a;

    fn blob(&self, _: &ContentId) -> Result<Option<&S>, SourceError> {
        Ok(Some(&self.0))
    }
}

/// A blob of any length that fails every read.
struct Broken(u64);

impl RangeSource for Broken {
    fn len(&self) -> u64 {
        self.0
    }

    fn read_at(&self, _: u64, _: &mut [u8]) -> Result<usize, SourceError> {
        Err(SourceError::Failed(7))
    }
}

/// A store that fails.
struct Down;

impl Blobs for Down {
    type Blob<'a> = &'a [u8];

    fn blob(&self, _: &ContentId) -> Result<Option<&[u8]>, SourceError> {
        Err(SourceError::Failed(3))
    }
}

#[test]
fn a_blob_is_read_only_as_the_one_record_its_key_names() {
    let r = header();
    let key = r.content_id();
    let bytes = r.encode(&mut meter()).unwrap();
    assert_eq!(
        read_record(&One(bytes.as_slice()), &key, &mut meter()),
        Ok(r)
    );
    let mut longer = bytes.clone();
    longer.push(0);
    let other = ContentId::of_payload(b"other");
    for (got, want) in [
        (
            read_record(&One(Broken(RECORD_CAP + 1)), &key, &mut meter()),
            BlobError::TooLarge(RECORD_CAP + 1),
        ),
        (
            read_record(&One(Broken(8)), &key, &mut meter()),
            BlobError::Source(SourceError::Failed(7)),
        ),
        (
            read_record(&Down, &key, &mut meter()),
            BlobError::Source(SourceError::Failed(3)),
        ),
        (
            read_record(&One(longer.as_slice()), &key, &mut meter()),
            BlobError::Trailing,
        ),
        (
            read_record(&One(bytes.as_slice()), &other, &mut meter()),
            BlobError::Key,
        ),
        (
            read_record(&Container::default(), &key, &mut meter()),
            BlobError::Missing,
        ),
    ] {
        assert_eq!(got, Err(want.clone()));
        assert!(!want.to_string().is_empty());
    }
}

#[test]
fn a_stopped_meter_is_a_stop_and_never_damage() {
    let (c, _, _) = two_bodies();
    let file = c.to_file(&mut meter()).unwrap();
    let parsed = read_file(&file);
    let mut stops = 0;
    for work in (0..3_000).step_by(11) {
        let tight = || WorkMeter::new(Budget::of_work(work));
        for got in [
            c.to_file(&mut tight()),
            c.salvage(&mut tight()).map(|s| s.file),
            Container::from_file(&parsed, &mut tight()).map(|_| file.clone()),
        ] {
            match got {
                Ok(bytes) => assert_eq!(bytes, file, "budget {work}"),
                Err(ContainerError::Stopped(s)) => {
                    assert!(!ContainerError::Stopped(s).to_string().is_empty());
                    stops += 1;
                }
                Err(e) => panic!("budget {work}: {e:?}"),
            }
        }
        let key = c.roots[0].clone();
        match read_record(&c, &key, &mut tight()) {
            Ok(_) | Err(BlobError::Stopped(_)) => {}
            Err(e) => panic!("budget {work}: {e:?}"),
        }
    }
    assert!(stops > 10, "{stops}");
}

#[test]
fn errors_say_what_went_wrong() {
    let e = ContainerError::Record(geom_format::record::RecordError::DependsOrder);
    assert!(!e.to_string().is_empty());
    let e = ContainerError::File(geom_format::file::FileError::Label);
    assert!(!e.to_string().is_empty());
}
