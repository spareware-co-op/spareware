//! The specification's generated tables against the code (piece G93's drift check), the registry's
//! constants against the writers', and every archived file against the CDDL.
//!
//! `format/REGISTRY.md` and `format/SPEC.md` hold tables between `<!-- generated: NAME -->` and
//! `<!-- end generated -->`. `the_tables_are_the_codes` fails when one differs from what
//! `geom_format::registry` renders; `write_the_tables` (ignored) rewrites them. Every file under
//! `tests/archive/` (24.6's archive: none is ever deleted) must read whole and validate, record by
//! record and payload by payload, against `format/cddl/`, with the `cddl-cat` crate (MIT), a test
//! dependency only.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: native arithmetic and indexing, and a failure stops the test"
)]

use std::path::{Path, PathBuf};

use geom_format::file::{END_TAG, HEADER_TAG, read, write};
use geom_format::header::{self, Header, rung};
use geom_format::record::{REQUIRED, Record};
use geom_format::registry::{
    ENVELOPE, RECORDS, RUNGS, field_table, knows, registry_table, rung_table,
};
use geom_format::source::SliceSource;
use geom_num::grid::Steps;
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The kernel's `format/` folder.
fn format_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../format")
}

/// Each generated section: the file, its name and what the code renders.
fn sections() -> Vec<(&'static str, &'static str, String)> {
    let by_name = |name: &str| RECORDS.iter().find(|r| r.name == name).unwrap().fields;
    vec![
        ("REGISTRY.md", "registry", registry_table()),
        ("SPEC.md", "envelope", field_table(ENVELOPE)),
        ("SPEC.md", "header", field_table(by_name("header"))),
        ("SPEC.md", "rungs", rung_table()),
        ("SPEC.md", "end", field_table(by_name("end"))),
        ("SPEC.md", "names", field_table(by_name("names"))),
        ("SPEC.md", "history", field_table(by_name("history"))),
        ("SPEC.md", "recipe", field_table(by_name("recipe"))),
        ("SPEC.md", "attributes", field_table(by_name("attributes"))),
        ("SPEC.md", "operation", field_table(by_name("operation"))),
        ("SPEC.md", "pruned", field_table(by_name("pruned"))),
    ]
}

/// `text` with section `name` replaced by `body`, or `None` if its markers are missing.
fn with_section(text: &str, name: &str, body: &str) -> Option<String> {
    let open = format!("<!-- generated: {name} -->\n");
    let close = "<!-- end generated -->";
    let start = text.find(&open)? + open.len();
    let end = start + text[start..].find(close)?;
    Some(format!("{}{body}{}", &text[..start], &text[end..]))
}

#[test]
#[cfg_attr(target_os = "wasi", ignore = "reads the format folder")]
#[cfg_attr(miri, ignore = "reads the format folder")]
fn the_tables_are_the_codes() {
    for (file, name, body) in sections() {
        let text = std::fs::read_to_string(format_dir().join(file))
            .unwrap()
            .replace("\r\n", "\n");
        let want = with_section(&text, name, &body).expect(name);
        assert!(
            text == want,
            "{file}'s table {name} differs from the code; run write_the_tables. It should be:\n{body}"
        );
    }
}

#[test]
#[ignore = "rewrites the specification's generated tables"]
fn write_the_tables() {
    for (file, name, body) in sections() {
        let path = format_dir().join(file);
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\r\n", "\n");
        std::fs::write(&path, with_section(&text, name, &body).unwrap()).unwrap();
    }
}

#[test]
fn the_registry_agrees_with_the_writers() {
    let mut tags: Vec<u64> = RECORDS.iter().map(|r| r.tag).collect();
    tags.sort_unstable();
    tags.dedup();
    assert_eq!(tags.len(), RECORDS.len(), "a tag appears twice");
    let h = Header::v1("test", Steps(640)).record().unwrap();
    assert_eq!(
        (h.tag, h.schema, h.flags),
        (HEADER_TAG, header::SCHEMA, REQUIRED)
    );
    assert!(knows(HEADER_TAG, 1) && knows(END_TAG, 1));
    assert!(!knows(HEADER_TAG, 2) && !knows(3, 1));
    let codes: Vec<u64> = RUNGS.iter().map(|r| r.0).collect();
    assert_eq!(
        codes,
        Header::v1("x", Steps(1))
            .ladder
            .keys()
            .copied()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        codes,
        [
            rung::FIT,
            rung::PER_OPERATION,
            rung::HEAL,
            rung::HEAL_CAP,
            rung::SHORT_EDGE_WARNING,
            rung::COMMIT_PER_AXIS,
            rung::ANGULAR_LOG2
        ]
    );
}

/// The archive: every file ever written as an example of a format version.
fn archive() -> Vec<(&'static str, &'static [u8])> {
    vec![(
        "v1/header-only.skgf",
        include_bytes!("archive/v1/header-only.skgf"),
    )]
}

/// The archive's first file: format 1's header and nothing else.
fn header_only() -> Vec<u8> {
    let h = Header::v1("geom-format archive v1", geom_num::ladder::HEAL_DEFAULT);
    write(&h.record().unwrap(), &[], &mut meter()).unwrap()
}

#[test]
#[ignore = "writes the archive's files (each written once and never changed)"]
fn write_the_archive() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/archive/v1");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("header-only.skgf"), header_only()).unwrap();
}

#[test]
#[cfg_attr(target_os = "wasi", ignore = "lists the archive folder")]
#[cfg_attr(miri, ignore = "lists the archive folder")]
fn the_archive_list_is_every_archived_file() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/archive");
    let mut found = Vec::new();
    for version in std::fs::read_dir(&root).unwrap() {
        let version = version.unwrap().path();
        if !version.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&version).unwrap() {
            let file = file.unwrap().path();
            if file.extension().is_some_and(|e| e == "skgf") {
                found.push(format!(
                    "{}/{}",
                    version.file_name().unwrap().to_str().unwrap(),
                    file.file_name().unwrap().to_str().unwrap()
                ));
            }
        }
    }
    found.sort();
    let listed: Vec<String> = archive().iter().map(|(n, _)| (*n).to_owned()).collect();
    assert_eq!(found, listed);
}

#[test]
fn every_archived_file_reads() {
    for (name, bytes) in archive() {
        let file = read(&mut SliceSource::new(bytes), knows, &mut meter()).unwrap();
        assert!(file.skipped.is_empty(), "{name}");
        let h = Header::read(&file.header.payload, &mut meter()).unwrap();
        assert_eq!(h.format.0, 1, "{name}");
    }
}

#[test]
fn every_archived_file_matches_the_cddl() {
    let record = include_str!("../../../format/cddl/record.cddl");
    let header_schema = include_str!("../../../format/cddl/header.cddl");
    let end_schema = include_str!("../../../format/cddl/end.cddl");
    for (name, bytes) in archive() {
        let file = read(&mut SliceSource::new(bytes), knows, &mut meter()).unwrap();
        cddl_cat::validate_cbor_bytes("header", header_schema, &file.header.payload)
            .unwrap_or_else(|e| panic!("{name}: header: {e:?}"));
        // Every record's own bytes against the envelope, and END's payload against its schema.
        let mut at = 12;
        let mut m = meter();
        while at < bytes.len() {
            let (r, len) = Record::decode(&bytes[at..], &mut m).unwrap();
            cddl_cat::validate_cbor_bytes("record", record, &bytes[at..at + len])
                .unwrap_or_else(|e| panic!("{name}: record at {at}: {e:?}"));
            if r.tag == END_TAG {
                cddl_cat::validate_cbor_bytes("end", end_schema, &r.payload)
                    .unwrap_or_else(|e| panic!("{name}: END: {e:?}"));
            }
            at += len;
        }
    }
}
