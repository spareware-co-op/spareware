//! The format museum (chapter 28.13, piece G99): every exhibit opens and replays to its record on
//! this target, the golden `geom-test/museum` holds them all to the matrix, and the folder, the
//! catalogue and `geom-format`'s archive agree. The attacks plant one change at a time in a copy of
//! an exhibit or of the catalogue, and each must be caught as what it is.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failure should stop loudly"
)]

use geom_test::fixture::kernel::Kernel;
use geom_test::fixture::log::OpLog;
use geom_test::golden::{self, hex, sha256};
use geom_test::museum::{self, Catalogue, Exhibit, Kind, MuseumError, open, parse, stored, visit};

/// The exhibits compiled in, or a stop with what is wrong.
fn catalogue() -> Catalogue {
    museum::catalogue().unwrap()
}

/// The exhibit with file `file`.
fn exhibit(file: &str) -> Exhibit {
    catalogue()
        .exhibits
        .into_iter()
        .find(|e| e.file == file)
        .unwrap()
}

#[test]
fn every_exhibit_opens_and_replays_on_this_target() {
    let checked = golden::check("geom-test/museum", || {
        let seen = visit(&catalogue(), stored, &mut Kernel::new()).unwrap();
        (seen.digest, seen)
    });
    assert!(checked.value.exhibits >= 2);
    println!(
        "geom-test museum: {} exhibits, golden hash {}",
        checked.value.exhibits,
        hex(checked.value.digest)
    );
}

#[test]
#[ignore = "writes the museum's files (each written once and never changed)"]
fn write_the_exhibits() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("museum/v1");
    std::fs::create_dir_all(&dir).unwrap();
    let log = OpLog::new(Vec::new(), Vec::new()).encode().unwrap();
    std::fs::write(dir.join("empty-log.skgf"), log).unwrap();
}

#[test]
#[cfg_attr(target_os = "wasi", ignore = "reads the tree")]
#[cfg_attr(miri, ignore = "reads the tree")]
fn the_folder_the_catalogue_and_the_archive_agree() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    museum::repo(&root, &catalogue()).unwrap();
}

#[test]
fn a_byte_changed_in_a_stored_file_is_caught_first() {
    let e = exhibit("v1/header-only.skgf");
    let mut bytes = stored(&e.file).unwrap().to_vec();
    assert_eq!(open(&e, &bytes, &mut Kernel::new()), Ok(()));
    for at in [0, 11, 12, bytes.len() / 2, bytes.len() - 1] {
        bytes[at] ^= 1;
        assert_eq!(
            open(&e, &bytes, &mut Kernel::new()),
            Err(MuseumError::Changed(e.id.clone()))
        );
        bytes[at] ^= 1;
    }
}

/// `e` recorded for `bytes`, so the attack reaches what lies past the digest.
fn recorded(e: &Exhibit, bytes: &[u8]) -> Exhibit {
    Exhibit {
        sha256: hex(sha256(bytes)),
        ..e.clone()
    }
}

#[test]
fn a_file_that_does_not_read_or_does_not_hold_its_count_is_caught() {
    let e = exhibit("v1/header-only.skgf");
    let bytes = stored(&e.file).unwrap();
    // Cut short: no END record.
    let cut = &bytes[..bytes.len() - 3];
    assert!(matches!(
        open(&recorded(&e, cut), cut, &mut Kernel::new()),
        Err(MuseumError::Unreadable { .. })
    ));
    // Its record counts one more record than it holds.
    let wrong = Exhibit {
        kind: Kind::File { records: 1 },
        ..e.clone()
    };
    assert!(matches!(
        open(&wrong, bytes, &mut Kernel::new()),
        Err(MuseumError::Unreadable { .. })
    ));
}

#[test]
fn a_log_that_does_not_replay_to_its_hash_is_caught() {
    let e = exhibit("v1/empty-log.skgf");
    let bytes = stored(&e.file).unwrap();
    assert_eq!(open(&e, bytes, &mut Kernel::new()), Ok(()));
    let wrong = Exhibit {
        kind: Kind::Log {
            replays_to: "0".repeat(64),
        },
        ..e.clone()
    };
    assert!(matches!(
        open(&wrong, bytes, &mut Kernel::new()),
        Err(MuseumError::Replay { .. })
    ));
    // A file of records is not a log.
    let file = exhibit("v1/header-only.skgf");
    let other = stored(&file.file).unwrap();
    let wrong = Exhibit {
        kind: Kind::Log {
            replays_to: "0".repeat(64),
        },
        ..recorded(&e, other)
    };
    assert!(open(&wrong, other, &mut Kernel::new()).is_err());
}

#[test]
fn an_exhibit_without_its_file_is_caught() {
    let mut c = catalogue();
    c.exhibits[0].file = "v1/gone.skgf".to_owned();
    assert_eq!(
        visit(&c, stored, &mut Kernel::new()),
        Err(MuseumError::Missing(c.exhibits[0].id.clone()))
    );
}

#[test]
fn the_digest_covers_every_exhibit_its_bytes_and_its_hash() {
    let whole = visit(&catalogue(), stored, &mut Kernel::new()).unwrap();
    // One exhibit fewer.
    let mut c = catalogue();
    c.exhibits.pop();
    let fewer = visit(&c, stored, &mut Kernel::new()).unwrap();
    assert_ne!(whole.digest, fewer.digest);
    assert_eq!(fewer.exhibits, whole.exhibits - 1);
}

#[test]
fn the_catalogues_rules_hold() {
    let good = r#"{"exhibits": [
        {"id": "1/a", "what": "w", "format": 1, "edition": "pre-edition", "file": "v1/a.skgf",
         "sha256": "AAAA", "kind": {"file": {"records": 0}}}
    ]}"#;
    let digest = "a".repeat(64);
    let good = good.replace("AAAA", &digest);
    assert!(parse(&good).is_ok());
    let refused = |text: String| {
        assert!(
            matches!(parse(&text), Err(MuseumError::Catalogue(_))),
            "{text}"
        );
    };
    refused(good.replace("\"what\"", "\"whet\""));
    refused(good.replace("{\"records\": 0}", "{\"records\": 0, \"more\": 1}"));
    refused(good.replace("v1/a.skgf", "v2/a.skgf"));
    refused(good.replace("\"1/a\"", "\"2/a\""));
    refused(good.replace(&digest, "short"));
    refused(good.replace(&digest, &"g".repeat(64)));
    refused(good.replace(
        "{\"file\": {\"records\": 0}}",
        "{\"log\": {\"replays_to\": \"short\"}}",
    ));
    assert!(
        parse(&good.replace(
            "{\"file\": {\"records\": 0}}",
            &format!("{{\"log\": {{\"replays_to\": \"{digest}\"}}}}")
        ))
        .is_ok()
    );
    // Two exhibits out of order, or one twice.
    let two = |first: &str, second: &str| {
        let item = |id: &str| {
            format!(
                r#"{{"id": "{id}", "what": "w", "format": 1, "edition": "e", "file": "v1/a.skgf",
                "sha256": "{digest}", "kind": {{"file": {{"records": 0}}}}}}"#
            )
        };
        format!(r#"{{"exhibits": [{}, {}]}}"#, item(first), item(second))
    };
    assert!(parse(&two("1/a", "1/b")).is_ok());
    refused(two("1/b", "1/a"));
    refused(two("1/a", "1/a"));
}

/// The kernel's CI workflow, as committed.
const KERNEL_YML: &str = include_str!("../../../../../../.github/workflows/kernel.yml");

#[test]
fn format_round_trips_run_on_a_big_endian_target() {
    let job = KERNEL_YML
        .split("\n  big-endian:\n")
        .nth(1)
        .and_then(|rest| rest.split("\n  guard:\n").next())
        .expect("kernel.yml has a job big-endian before its guard job");
    assert!(job.contains("targets: s390x-unknown-linux-gnu"), "{job}");
    assert!(job.contains("qemu-s390x"), "{job}");
    for command in [
        "cargo test --locked --release -p geom-format --target s390x-unknown-linux-gnu",
        "cargo test --locked --release -p geom-test --test museum --target s390x-unknown-linux-gnu",
    ] {
        assert!(job.contains(command), "{job}");
    }
}

#[test]
fn every_error_says_what_is_wrong() {
    for e in [
        MuseumError::Catalogue("c".to_owned()),
        MuseumError::Missing("m".to_owned()),
        MuseumError::Changed("h".to_owned()),
        MuseumError::Unreadable {
            id: "u".to_owned(),
            why: "w".to_owned(),
        },
        MuseumError::NotIdentical("n".to_owned()),
        MuseumError::Replay {
            id: "r".to_owned(),
            got: "g".to_owned(),
        },
    ] {
        assert_ne!(e.to_string(), "");
    }
}
