//! Piece G414c's attack: tries to break every claim of the zip reader (`geom_exchange::zip`) with
//! archives written byte by byte here: stored and deflated entries read back, each refusal with its
//! kind and offset, zip-slip names, duplicates, names the Open Packaging Conventions forbid and
//! their case-insensitive equivalents, overlapping entries (two central headers at one local
//! header, and an entry hidden inside another's data), bombs held to their declared size and to
//! the ratio budget, too many entries, long and non-UTF-8 names, encryption, other methods, end
//! records missing, followed by bytes or after prepended bytes, more than one disk, a hand-built
//! Zip64 archive, local headers that disagree, and every archive read again through a source that
//! splits its reads.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "test cases and the archive writer; the rules are for kernel code"
)]

use geom_exchange::inflate::{InflateBudget, InflateErrorKind};
use geom_exchange::zip::part::{self, PartRule};
use geom_exchange::zip::{Method, ZipArchive, ZipBudget, ZipError, ZipErrorKind};
use geom_format::checksum::crc32;
use geom_format::source::{RangeSource, SourceError};
use geom_run::{Budget, WorkMeter};

/// One entry as the writer lays it out. Each field is written as given, so a test can make the
/// central and local headers disagree.
#[derive(Clone)]
struct Item {
    name: Vec<u8>,
    /// The local header's name, if not `name`.
    local_name: Option<Vec<u8>>,
    flags: u16,
    method: u16,
    crc: u32,
    /// The declared size once read.
    size: u32,
    /// The stored bytes.
    body: Vec<u8>,
    /// The declared stored length, in both headers, if not the body's.
    compressed: Option<u32>,
    /// Whether a local header and body are written (otherwise only a central header).
    local: bool,
    /// The central header's local header offset, if not where this item's local header was put.
    offset: Option<u32>,
    /// Sizes and CRC in a data descriptor after the body, zero in the local header (flag bit 3).
    described: bool,
}

impl Item {
    fn stored(name: &str, data: &[u8]) -> Item {
        Item {
            name: name.as_bytes().to_vec(),
            local_name: None,
            flags: 0,
            method: 0,
            crc: crc32(data),
            size: data.len() as u32,
            body: data.to_vec(),
            compressed: None,
            local: true,
            offset: None,
            described: false,
        }
    }

    fn deflated(name: &str, data: &[u8]) -> Item {
        Item {
            method: 8,
            body: miniz_oxide::deflate::compress_to_vec(data, 6),
            ..Item::stored(name, data)
        }
    }

    fn compressed(&self) -> u32 {
        self.compressed.unwrap_or(self.body.len() as u32)
    }
}

/// An archive and where the writer put its parts.
struct Laid {
    bytes: Vec<u8>,
    /// Each item's local header (or its central header's offset, for an item with none).
    headers: Vec<u64>,
    /// Each item's central header.
    central: Vec<u64>,
    /// The central directory and the end record.
    directory: u64,
    end: u64,
}

fn u16le(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn u32le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn u64le(v: &mut Vec<u8>, x: u64) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn put_u16(v: &mut [u8], at: u64, x: u16) {
    let at = at as usize;
    v[at..at + 2].copy_from_slice(&x.to_le_bytes());
}

fn put_u32(v: &mut [u8], at: u64, x: u32) {
    let at = at as usize;
    v[at..at + 4].copy_from_slice(&x.to_le_bytes());
}

/// Writes `items` as PKWARE's APPNOTE lays an archive out (4.3.6): each local header and its
/// body, then the central directory, then the end record with `comment`.
fn lay(items: &[Item], comment: &[u8]) -> Laid {
    let mut b = Vec::new();
    let mut headers = Vec::new();
    for it in items {
        let at = it.offset.map_or(b.len() as u64, u64::from);
        headers.push(at);
        if !it.local {
            continue;
        }
        let name = it.local_name.as_ref().unwrap_or(&it.name);
        u32le(&mut b, 0x0403_4b50);
        u16le(&mut b, 20);
        u16le(&mut b, it.flags);
        u16le(&mut b, it.method);
        u32le(&mut b, 0);
        let (crc, compressed, size) = if it.described {
            (0, 0, 0)
        } else {
            (it.crc, it.compressed(), it.size)
        };
        u32le(&mut b, crc);
        u32le(&mut b, compressed);
        u32le(&mut b, size);
        u16le(&mut b, name.len() as u16);
        u16le(&mut b, 0);
        b.extend_from_slice(name);
        b.extend_from_slice(&it.body);
        if it.described {
            u32le(&mut b, 0x0807_4b50);
            u32le(&mut b, it.crc);
            u32le(&mut b, it.compressed());
            u32le(&mut b, it.size);
        }
    }
    let directory = b.len() as u64;
    let mut central = Vec::new();
    for (it, &at) in items.iter().zip(&headers) {
        central.push(b.len() as u64);
        u32le(&mut b, 0x0201_4b50);
        u16le(&mut b, 20);
        u16le(&mut b, 20);
        u16le(&mut b, it.flags);
        u16le(&mut b, it.method);
        u32le(&mut b, 0);
        u32le(&mut b, it.crc);
        u32le(&mut b, it.compressed());
        u32le(&mut b, it.size);
        u16le(&mut b, it.name.len() as u16);
        u16le(&mut b, 0);
        u16le(&mut b, 0);
        u16le(&mut b, 0);
        u16le(&mut b, 0);
        u32le(&mut b, 0);
        u32le(&mut b, at as u32);
        b.extend_from_slice(&it.name);
    }
    let end = b.len() as u64;
    u32le(&mut b, 0x0605_4b50);
    u16le(&mut b, 0);
    u16le(&mut b, 0);
    u16le(&mut b, items.len() as u16);
    u16le(&mut b, items.len() as u16);
    u32le(&mut b, (end - directory) as u32);
    u32le(&mut b, directory as u32);
    u16le(&mut b, comment.len() as u16);
    b.extend_from_slice(comment);
    Laid {
        bytes: b,
        headers,
        central,
        directory,
        end,
    }
}

/// A one-entry Zip64 archive (APPNOTE 4.3.14, 4.3.15, 4.5.3): every size, offset and count in the
/// headers and the end record holds its sentinel, and the values are in Zip64 fields.
fn zip64(data: &[u8], extra_values: usize) -> Vec<u8> {
    let name = b"big.bin";
    let mut b = Vec::new();
    u32le(&mut b, 0x0403_4b50);
    u16le(&mut b, 45);
    u16le(&mut b, 0);
    u16le(&mut b, 0);
    u32le(&mut b, 0);
    u32le(&mut b, crc32(data));
    u32le(&mut b, u32::MAX);
    u32le(&mut b, u32::MAX);
    u16le(&mut b, name.len() as u16);
    u16le(&mut b, 20);
    b.extend_from_slice(name);
    u16le(&mut b, 1);
    u16le(&mut b, 16);
    u64le(&mut b, data.len() as u64);
    u64le(&mut b, data.len() as u64);
    b.extend_from_slice(data);
    let directory = b.len() as u64;
    u32le(&mut b, 0x0201_4b50);
    u16le(&mut b, 45);
    u16le(&mut b, 45);
    u16le(&mut b, 0);
    u16le(&mut b, 0);
    u32le(&mut b, 0);
    u32le(&mut b, crc32(data));
    u32le(&mut b, u32::MAX);
    u32le(&mut b, u32::MAX);
    u16le(&mut b, name.len() as u16);
    u16le(&mut b, 4 + 8 * extra_values as u16);
    u16le(&mut b, 0);
    u16le(&mut b, 0);
    u16le(&mut b, 0);
    u32le(&mut b, 0);
    u32le(&mut b, u32::MAX);
    b.extend_from_slice(name);
    u16le(&mut b, 1);
    u16le(&mut b, 8 * extra_values as u16);
    for v in [data.len() as u64, data.len() as u64, 0]
        .iter()
        .take(extra_values)
    {
        u64le(&mut b, *v);
    }
    let record = b.len() as u64;
    u32le(&mut b, 0x0606_4b50);
    u64le(&mut b, 44);
    u16le(&mut b, 45);
    u16le(&mut b, 45);
    u32le(&mut b, 0);
    u32le(&mut b, 0);
    u64le(&mut b, 1);
    u64le(&mut b, 1);
    u64le(&mut b, record - directory);
    u64le(&mut b, directory);
    u32le(&mut b, 0x0706_4b50);
    u32le(&mut b, 0);
    u64le(&mut b, record);
    u32le(&mut b, 1);
    u32le(&mut b, 0x0605_4b50);
    u16le(&mut b, 0);
    u16le(&mut b, 0);
    u16le(&mut b, u16::MAX);
    u16le(&mut b, u16::MAX);
    u32le(&mut b, u32::MAX);
    u32le(&mut b, u32::MAX);
    u16le(&mut b, 0);
    b
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn open(bytes: &[u8]) -> Result<ZipArchive, ZipError> {
    ZipArchive::read(bytes, &ZipBudget::DEFAULT, &mut meter())
}

fn refused(bytes: &[u8], kind: ZipErrorKind, offset: u64) {
    assert_eq!(
        open(bytes).map(|_| ()),
        Err(ZipError { kind, offset }),
        "archive {bytes:02x?}"
    );
}

/// Every entry's bytes, in order.
fn contents<S: RangeSource + ?Sized>(
    zip: &ZipArchive,
    source: &S,
) -> Vec<Result<Vec<u8>, ZipError>> {
    zip.entries()
        .iter()
        .map(|e| zip.read_entry(source, e, &ZipBudget::DEFAULT, &mut meter()))
        .collect()
}

/// A source that gives at most `.1` bytes a read.
struct Short<'a>(&'a [u8], usize);

impl RangeSource for Short<'_> {
    fn len(&self) -> u64 {
        self.0.len() as u64
    }

    fn read_at(&self, at: u64, buf: &mut [u8]) -> Result<usize, SourceError> {
        let at = at as usize;
        if at > self.0.len() {
            return Err(SourceError::OutOfBounds);
        }
        let n = buf.len().min(self.1).min(self.0.len() - at);
        buf[..n].copy_from_slice(&self.0[at..at + n]);
        Ok(n)
    }
}

/// The same archive and the same entries read whole and through sources that split every read.
fn split_never_matters(bytes: &[u8]) {
    let whole = open(bytes);
    for n in [1, 3, 7, 1_000] {
        let short = Short(bytes, n);
        let split = ZipArchive::read(&short, &ZipBudget::DEFAULT, &mut meter());
        assert_eq!(split, whole, "reads of {n}");
        if let (Ok(a), Ok(_)) = (&whole, &split) {
            assert_eq!(
                contents(a, &short),
                contents(a, bytes),
                "entries in reads of {n}"
            );
        }
    }
}

fn model(len: usize) -> Vec<u8> {
    let mut v = Vec::new();
    let mut i = 0_u64;
    while v.len() < len {
        v.extend_from_slice(
            format!(
                "<vertex x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
                i % 997,
                i * 7 % 101,
                i % 13
            )
            .as_bytes(),
        );
        i += 1;
    }
    v.truncate(len);
    v
}

#[test]
fn stored_and_deflated_entries_read_back() {
    let text = model(5_000);
    let items = [
        Item::stored("[Content_Types].xml", b"<Types/>"),
        Item::stored("3D/", b""),
        Item::deflated("3D/3dmodel.model", &text),
        Item::stored("empty", b""),
        Item::deflated("Metadata/thumbnail.png", b""),
    ];
    for comment in [&b""[..], &b"made by a test"[..]] {
        let laid = lay(&items, comment);
        let zip = open(&laid.bytes).unwrap();
        assert_eq!(zip.directory(), laid.directory);
        assert_eq!(zip.entries().len(), items.len());
        for ((e, it), &at) in zip.entries().iter().zip(&items).zip(&laid.headers) {
            assert_eq!(e.name.as_bytes(), it.name.as_slice());
            assert_eq!(e.header, at);
            assert_eq!(e.data, at + 30 + it.name.len() as u64);
            assert_eq!(e.compressed, it.body.len() as u64);
            assert_eq!(e.size, u64::from(it.size));
            assert_eq!(e.crc32, it.crc);
        }
        assert_eq!(zip.entries()[2].method, Method::Deflated);
        assert_eq!(zip.entries()[3].method, Method::Stored);
        let model = zip.find("3D/3dmodel.model").unwrap();
        assert_eq!(
            zip.read_entry(&laid.bytes[..], model, &ZipBudget::DEFAULT, &mut meter()),
            Ok(text.clone())
        );
        assert_eq!(zip.find("3d/3dmodel.model"), None);
        let all = contents(&zip, &laid.bytes[..]);
        assert_eq!(all[0], Ok(b"<Types/>".to_vec()));
        assert_eq!(all[1], Ok(Vec::new()));
        assert_eq!(all[4], Ok(Vec::new()));
        assert_eq!(zip.check_parts(), Ok(()));
        split_never_matters(&laid.bytes);
    }
    // An empty archive: only its end record.
    let empty = lay(&[], b"");
    assert!(open(&empty.bytes).unwrap().entries().is_empty());
    // Sizes and CRC in a data descriptor, the local header's fields zero.
    let mut described = Item::deflated("d.txt", &text);
    described.flags = 0x0008;
    described.described = true;
    let laid = lay(&[described, Item::stored("e.txt", b"e")], b"");
    let zip = open(&laid.bytes).unwrap();
    assert_eq!(contents(&zip, &laid.bytes[..])[0], Ok(text));
    split_never_matters(&laid.bytes);
}

#[test]
fn an_entry_that_is_not_what_it_declares_is_refused_when_read() {
    let text = model(3_000);
    // A CRC that is not the bytes'.
    let mut bad_crc = Item::deflated("a", &text);
    bad_crc.crc ^= 1;
    // Deflated bytes shorter than declared.
    let mut longer = Item::deflated("b", &text);
    longer.size += 1;
    // Deflated bytes longer than declared: refused at the declared size, inside the stream.
    let mut shorter = Item::deflated("c", &text);
    shorter.size -= 1;
    let mut bad_stored = Item::stored("d", b"spare");
    bad_stored.crc = crc32(b"sparE");
    let laid = lay(&[bad_crc, longer, shorter, bad_stored], b"");
    let zip = open(&laid.bytes).unwrap();
    let e = zip.entries();
    let got = contents(&zip, &laid.bytes[..]);
    assert_eq!(
        got[0],
        Err(ZipError {
            kind: ZipErrorKind::Crc,
            offset: e[0].data
        })
    );
    assert_eq!(
        got[1],
        Err(ZipError {
            kind: ZipErrorKind::Size,
            offset: e[1].data
        })
    );
    let refusal = *got[2].as_ref().unwrap_err();
    assert_eq!(
        refusal.kind,
        ZipErrorKind::Inflate(InflateErrorKind::TooLarge)
    );
    assert!(refusal.offset >= e[2].data && refusal.offset < e[2].data + e[2].compressed);
    assert_eq!(
        got[3],
        Err(ZipError {
            kind: ZipErrorKind::Crc,
            offset: e[3].data
        })
    );
    // A stored entry whose stored and read lengths differ is refused with the directory.
    let mut odd = Item::stored("e", b"spare");
    odd.size = 4;
    let laid = lay(&[odd], b"");
    refused(&laid.bytes, ZipErrorKind::Size, laid.central[0]);
    // A deflated body that is not DEFLATE.
    let mut junk = Item::stored("f", &[0xff; 8]);
    junk.method = 8;
    junk.size = 100;
    let laid = lay(&[junk], b"");
    let zip = open(&laid.bytes).unwrap();
    let got = contents(&zip, &laid.bytes[..]);
    assert_eq!(
        got[0],
        Err(ZipError {
            kind: ZipErrorKind::Inflate(InflateErrorKind::BlockType),
            offset: zip.entries()[0].data
        })
    );
}

#[test]
fn zip_slip_names_are_refused_before_any_entry_is_read() {
    for name in [
        "../evil",
        "a/../../evil",
        "/etc/passwd",
        "C:evil",
        "c:/evil",
        "a\\b",
        "..\\evil",
        "a\0b",
        "./a",
        "a/./b",
        "a/..",
        "",
    ] {
        let laid = lay(&[Item::stored("ok", b"1"), Item::stored(name, b"x")], b"");
        refused(&laid.bytes, ZipErrorKind::Escape, laid.central[1]);
        assert!(part::escapes(name), "{name:?}");
    }
    for name in [
        "a",
        "a/b",
        "a..b",
        ".rels",
        "_rels/.rels",
        "ab:c",
        "...",
        "a/.../b",
    ] {
        assert!(!part::escapes(name), "{name:?}");
    }
}

#[test]
fn duplicates_are_refused_at_the_later_header() {
    let laid = lay(
        &[
            Item::stored("b", b"1"),
            Item::stored("a", b"2"),
            Item::stored("b", b"3"),
        ],
        b"",
    );
    refused(&laid.bytes, ZipErrorKind::Duplicate, laid.headers[2]);
    // Named the same only when folded: a duplicate part, not a duplicate entry.
    let laid = lay(
        &[
            Item::stored("Doc.xml", b"1"),
            Item::stored("x", b"2"),
            Item::stored("doc.XML", b"3"),
        ],
        b"",
    );
    let zip = open(&laid.bytes).unwrap();
    assert_eq!(
        zip.check_parts(),
        Err(ZipError {
            kind: ZipErrorKind::Part(PartRule::Equivalent),
            offset: laid.headers[2]
        })
    );
}

#[test]
fn names_the_packaging_conventions_forbid_are_refused_by_their_rule() {
    use PartRule as R;
    for (name, rule) in [
        ("a//b", R::EmptySegment),
        ("a b", R::NotPchar),
        ("a%2", R::NotPchar),
        ("a%zz", R::NotPchar),
        ("a\u{e9}", R::NotPchar),
        ("a%2Fb", R::EncodedSlash),
        ("a%2fb", R::EncodedSlash),
        ("a%5Cb", R::EncodedSlash),
        ("a%41", R::EncodedUnreserved),
        ("a%7e", R::EncodedUnreserved),
        ("a.", R::TrailingDot),
        ("a/b./c", R::TrailingDot),
        ("a/...", R::AllDots),
    ] {
        assert_eq!(part::check(name), Err(rule), "{name:?}");
        let laid = lay(
            &[Item::stored("ok.xml", b"1"), Item::stored(name, b"x")],
            b"",
        );
        let zip = open(&laid.bytes).unwrap();
        assert_eq!(
            zip.check_parts(),
            Err(ZipError {
                kind: ZipErrorKind::Part(rule),
                offset: laid.headers[1]
            }),
            "{name:?}"
        );
    }
    assert_eq!(part::check(""), Err(R::Empty));
    assert_eq!(part::check("a/"), Err(R::TrailingSlash));
    // A folder entry holding bytes is a part with a trailing slash.
    let laid = lay(&[Item::stored("dir/", b"x")], b"");
    assert_eq!(
        open(&laid.bytes).unwrap().check_parts().unwrap_err().kind,
        ZipErrorKind::Part(R::TrailingSlash)
    );
    for name in [
        "3D/3dmodel.model",
        "_rels/.rels",
        "a%20b",
        "%E2%82%AC",
        "a-b_c~d!$&'()*+,;=:@",
    ] {
        assert_eq!(part::check(name), Ok(()), "{name:?}");
    }
    assert_eq!(part::fold("A/b.XML"), "a/b.xml");
}

#[test]
fn overlapping_entries_are_refused() {
    // Two central headers at one local header, under one name: a duplicate.
    let mut again = Item::stored("a", b"spare");
    again.local = false;
    again.offset = Some(0);
    let laid = lay(&[Item::stored("a", b"spare"), again.clone()], b"");
    refused(&laid.bytes, ZipErrorKind::Duplicate, 0);
    // Under another name: the local header names the first.
    again.name = b"b".to_vec();
    let laid = lay(&[Item::stored("a", b"spare"), again], b"");
    refused(&laid.bytes, ZipErrorKind::LocalHeader, 0);
    // An entry hidden inside another's bytes (Fifield's overlap): the outer entry's body is the
    // inner entry's whole local record, and the inner central header points into it.
    let inner = lay(&[Item::stored("b", b"bee")], b"");
    let record = inner.bytes[..inner.directory as usize].to_vec();
    let mut hidden = Item::stored("b", b"bee");
    hidden.local = false;
    hidden.offset = Some(30 + 1);
    let laid = lay(&[Item::stored("a", &record), hidden], b"");
    refused(&laid.bytes, ZipErrorKind::Overlap, 31);
    // An entry whose declared length runs into the central directory.
    let mut long = Item::stored("a", b"spare");
    long.compressed = Some(6);
    long.size = 6;
    let laid = lay(&[long], b"");
    refused(&laid.bytes, ZipErrorKind::Overlap, 0);
}

#[test]
fn bombs_are_held_to_their_declared_size_and_the_ratio() {
    // 10 MiB of zeros in about 10 KiB: declared truly, it passes the archive's checks and is
    // refused for its ratio when read, inside its stream.
    let zeros = vec![0_u8; 10 << 20];
    let laid = lay(&[Item::deflated("bomb", &zeros)], b"");
    let zip = open(&laid.bytes).unwrap();
    let e = &zip.entries()[0];
    let got = zip
        .read_entry(&laid.bytes[..], e, &ZipBudget::DEFAULT, &mut meter())
        .unwrap_err();
    assert_eq!(got.kind, ZipErrorKind::Inflate(InflateErrorKind::Ratio));
    assert!(got.offset >= e.data && got.offset < e.data + e.compressed);
    // Read in full when the ratio allows it.
    let generous = ZipBudget {
        inflate: InflateBudget {
            ratio: 10_000,
            ..InflateBudget::DEFAULT
        },
        ..ZipBudget::DEFAULT
    };
    assert_eq!(
        zip.read_entry(&laid.bytes[..], e, &generous, &mut meter()),
        Ok(zeros.clone())
    );
    // Declared small, it is refused at its declared size whatever the ratio.
    let mut small = Item::deflated("bomb", &zeros);
    small.size = 1_000;
    let laid = lay(&[small], b"");
    let zip = open(&laid.bytes).unwrap();
    assert_eq!(
        zip.read_entry(&laid.bytes[..], &zip.entries()[0], &generous, &mut meter())
            .unwrap_err()
            .kind,
        ZipErrorKind::Inflate(InflateErrorKind::TooLarge)
    );
    // Declared large, the archive is refused before anything is inflated.
    let mut a = Item::deflated("a", b"x");
    a.size = 600 << 20;
    let mut b = a.clone();
    b.name = b"b".to_vec();
    let laid = lay(&[a, b], b"");
    refused(&laid.bytes, ZipErrorKind::TooLarge, laid.central[1]);
}

#[test]
fn the_budgets_hold() {
    let items: Vec<Item> = (0..4)
        .map(|i| Item::stored(&format!("{i}"), b"x"))
        .collect();
    let laid = lay(&items, b"");
    let three = ZipBudget {
        entries: 3,
        ..ZipBudget::DEFAULT
    };
    assert_eq!(
        ZipArchive::read(&laid.bytes[..], &three, &mut meter()).unwrap_err(),
        ZipError {
            kind: ZipErrorKind::TooManyEntries,
            offset: laid.end
        }
    );
    let four = ZipBudget {
        entries: 4,
        ..ZipBudget::DEFAULT
    };
    assert!(ZipArchive::read(&laid.bytes[..], &four, &mut meter()).is_ok());
    // Names of the budget's length are read; one byte longer is refused.
    let longest = "n".repeat(1_024);
    assert!(open(&lay(&[Item::stored(&longest, b"x")], b"").bytes).is_ok());
    let laid = lay(&[Item::stored(&format!("{longest}n"), b"x")], b"");
    refused(&laid.bytes, ZipErrorKind::NameTooLong, laid.central[0]);
    let mut latin = Item::stored("x", b"x");
    latin.name = b"caf\xe9".to_vec();
    let laid = lay(&[latin], b"");
    refused(&laid.bytes, ZipErrorKind::NameEncoding, laid.central[0]);
    // The meter: the central directory is charged before it is read.
    let laid = lay(&items, b"");
    let mut few = WorkMeter::new(Budget::of_work(10));
    assert_eq!(
        ZipArchive::read(&laid.bytes[..], &ZipBudget::DEFAULT, &mut few).unwrap_err(),
        ZipError {
            kind: ZipErrorKind::Stopped,
            offset: laid.directory
        }
    );
}

#[test]
fn encrypted_entries_and_other_methods_are_refused() {
    for flags in [0x0001, 0x0040, 0x0041] {
        let mut it = Item::stored("a", b"x");
        it.flags = flags;
        let laid = lay(&[it], b"");
        refused(&laid.bytes, ZipErrorKind::Encrypted, laid.central[0]);
    }
    for method in [1, 9, 12, 14, 93, 99] {
        let mut it = Item::stored("a", b"x");
        it.method = method;
        let laid = lay(&[it], b"");
        refused(&laid.bytes, ZipErrorKind::Method, laid.central[0]);
    }
}

#[test]
fn the_end_record_must_end_the_archive_on_one_disk() {
    let laid = lay(&[Item::stored("a", b"spare")], b"note");
    // No end record at all, or the archive cut inside it.
    refused(
        &laid.bytes[..laid.end as usize],
        ZipErrorKind::End,
        laid.end,
    );
    refused(&[], ZipErrorKind::End, 0);
    refused(
        &laid.bytes[..laid.bytes.len() - 1],
        ZipErrorKind::End,
        laid.bytes.len() as u64 - 1,
    );
    // Bytes after the comment.
    let mut after = laid.bytes.clone();
    after.push(0);
    refused(&after, ZipErrorKind::End, after.len() as u64);
    // Bytes before the archive: the central directory is not where its end record says.
    let mut before = vec![0x50, 0x4b, 0];
    before.extend_from_slice(&laid.bytes);
    refused(&before, ZipErrorKind::Directory, laid.directory);
    // Bytes between the central directory and the end record.
    let mut between = laid.bytes[..laid.end as usize].to_vec();
    between.push(0);
    between.extend_from_slice(&laid.bytes[laid.end as usize..]);
    refused(&between, ZipErrorKind::Directory, laid.directory);
    // More entries in the directory than the end record counts, and fewer.
    let two = lay(&[Item::stored("a", b"1"), Item::stored("b", b"2")], b"");
    for (count, at) in [(1, two.central[1]), (3, two.end)] {
        let mut b = two.bytes.clone();
        put_u16(&mut b, two.end + 8, count);
        put_u16(&mut b, two.end + 10, count);
        refused(&b, ZipErrorKind::Directory, at);
    }
    // A broken central header.
    let mut b = two.bytes.clone();
    b[two.central[1] as usize] ^= 1;
    refused(&b, ZipErrorKind::Header, two.central[1]);
    // More than one disk, in the end record or in a central header.
    for field in [4, 6, 8] {
        let mut b = laid.bytes.clone();
        put_u16(&mut b, laid.end + field, 7);
        refused(&b, ZipErrorKind::MultiDisk, laid.end);
    }
    let mut b = laid.bytes.clone();
    put_u16(&mut b, laid.central[0] + 34, 1);
    refused(&b, ZipErrorKind::MultiDisk, laid.central[0]);
    // A comment holding an end record's signature: only a record whose comment ends the archive
    // is the end record.
    let mut fake = vec![0x50, 0x4b, 5, 6];
    fake.extend_from_slice(&[b'x'; 24]);
    let laid = lay(&[Item::stored("a", b"spare")], &fake);
    assert_eq!(open(&laid.bytes).unwrap().entries().len(), 1);
}

#[test]
fn a_zip64_archive_reads_back() {
    let data = model(700);
    let b = zip64(&data, 3);
    let zip = open(&b).unwrap();
    let e = &zip.entries()[0];
    assert_eq!(e.name, "big.bin");
    assert_eq!(e.size, 700);
    assert_eq!(e.compressed, 700);
    assert_eq!(e.header, 0);
    assert_eq!(e.data, 30 + 7 + 20);
    assert_eq!(contents(&zip, &b[..]), [Ok(data.clone())]);
    split_never_matters(&b);
    // A Zip64 field too short for the three values its sentinels ask for.
    let short = zip64(&data, 2);
    let directory = 30 + 7 + 20 + 700;
    refused(&short, ZipErrorKind::Zip64, directory);
    // The locator's signature broken, while the end record holds sentinels.
    let mut b2 = b.clone();
    let locator = b2.len() - 22 - 20;
    b2[locator] ^= 1;
    refused(&b2, ZipErrorKind::Zip64, locator as u64);
    // The Zip64 record not ending at its locator.
    let mut b3 = b.clone();
    let record = locator - 56;
    b3[record + 4] = 45;
    refused(&b3, ZipErrorKind::Zip64, record as u64);
    // More than one disk in the locator.
    let mut b4 = b.clone();
    b4[locator + 16] = 2;
    refused(&b4, ZipErrorKind::MultiDisk, locator as u64);
}

#[test]
fn local_headers_that_disagree_with_the_directory_are_refused() {
    let base = Item::deflated("a.txt", &model(400));
    let mut cases: Vec<Item> = Vec::new();
    let mut it = base.clone();
    it.local_name = Some(b"b.txt".to_vec());
    cases.push(it);
    let mut it = base.clone();
    it.local_name = Some(b"a.tx".to_vec());
    cases.push(it);
    for case in cases {
        let laid = lay(&[case], b"");
        refused(&laid.bytes, ZipErrorKind::LocalHeader, 0);
    }
    // One field of the local header changed at a time.
    let laid = lay(&[base], b"");
    for (field, value) in [(0, 0x0403_4b51_u32), (14, 1), (18, 1), (22, 1)] {
        let mut b = laid.bytes.clone();
        put_u32(&mut b, field, value);
        refused(&b, ZipErrorKind::LocalHeader, 0);
    }
    for (field, value) in [(6, 0x0002_u16), (8, 0)] {
        let mut b = laid.bytes.clone();
        put_u16(&mut b, field, value);
        refused(&b, ZipErrorKind::LocalHeader, 0);
    }
    // A local header past the end of the archive.
    let mut b = laid.bytes.clone();
    put_u32(&mut b, laid.central[0] + 42, 1 << 20);
    assert_eq!(
        open(&b).unwrap_err().kind,
        ZipErrorKind::Source(SourceError::OutOfBounds)
    );
}

#[test]
fn errors_say_what_and_where() {
    let e = ZipError {
        kind: ZipErrorKind::Crc,
        offset: 12,
    };
    assert_eq!(e.to_string(), "zip refused at byte 12: Crc");
}
