//! Adversarial tests of piece G12a: the vetted allow-list in `deny-no-std.toml`, and the standard
//! crates that replaced hand-rolled tools. Each test tries to break a claim the piece makes, and
//! fails while the claim does not hold.
//!
//! What is attacked, against the piece's done-when (issue #198) and chapter 29.1's dependency rule:
//!
//! - "`deny-no-std.toml` holds that allow-list and refuses anything not on it": the real file must
//!   be vetted, pinned to what `Cargo.lock` holds and with every record whole; a version that only
//!   starts like the pinned one, a requirement in place of a version, a record mislabelled,
//!   reordered or emptied, and an empty list (which cargo-deny 0.20.2 reads as no list at all, and
//!   so allows every crate) are each refused.
//! - "every copied SHA-256" replaced by `sha2`: no Rust file in the repository holds SHA-256's
//!   round constants or initial hash value, which any implementation of it must write out.
//! - The scaffold's TOML reader is the `toml` crate's: what TOML calls one value reads as one
//!   value however it is laid out, and what cargo would refuse is refused.
//!
//! Oracles: the TOML 1.0 specification's examples of equivalent layouts
//! (<https://toml.io/en/v1.0.0>); cargo-deny 0.20.2's source, read for behaviour only (MIT OR
//! Apache-2.0, `src/bans.rs` checks `allow` only `if !allowed.0.is_empty()`, and
//! `src/cfg/package_spec.rs` makes a version after `@` exact); FIPS 180-4, sections 4.2.2 and
//! 5.3.3, for the constants every SHA-256 implementation carries.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![cfg(not(target_family = "wasm"))]

#[allow(dead_code)]
#[path = "scaffold/manifest.rs"]
mod manifest;
#[allow(dead_code)]
#[path = "scaffold/vetted.rs"]
mod vetted;

use std::fs;
use std::path::{Path, PathBuf};

/// The repository's root, four folders above this crate.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .to_path_buf()
}

/// The vetted record of the real libm entry, for seeding variants of it.
fn real_list() -> String {
    // With LF line ends, as committed: a Windows checkout may convert them to CRLF, and the seeded
    // changes below are written with LF.
    fs::read_to_string(root().join("projects/engines/geometry/deny-no-std.toml"))
        .unwrap()
        .replace("\r\n", "\n")
}

#[test]
fn attack_the_real_allow_list_is_vetted_and_pinned_to_the_lock() {
    let list = vetted::read_text(&real_list()).unwrap_or_else(|e| panic!("{e:#?}"));
    let names: Vec<&str> = list.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "libm",
            "block-buffer",
            "cfg-if",
            "cpufeatures",
            "crc",
            "crc-catalog",
            "crypto-common",
            "digest",
            "hybrid-array",
            "libc",
            "sha2",
            "tinyvec",
            "typenum",
            "unicode-normalization"
        ],
        "the list holds libm, sha2 with the crates it brings (G86), unicode-normalization with tinyvec (G87), and crc with its catalogue (G92)"
    );
    let lock = fs::read_to_string(root().join("Cargo.lock")).unwrap();
    assert_eq!(vetted::against_lock(&list, &lock), Vec::<String>::new());
    // Each audit is the one the board gave its piece, and it is committed: G24's for libm ("the
    // committed libm audit"), G86's for sha2 and every crate it brings (32.20), and G87's for
    // unicode-normalization and the crate it brings (32.19).
    for entry in &list {
        let audit = match entry.name.as_str() {
            "libm" => "projects/kit/notes/G24-libm-audit.md",
            "unicode-normalization" | "tinyvec" => {
                "projects/kit/notes/G87-unicode-normalization-audit.md"
            }
            name if name.starts_with("crc") => "projects/kit/notes/G92-crc-audit.md",
            _ => "projects/kit/notes/G86-sha2-audit.md",
        };
        assert!(
            matches!(&entry.audited, vetted::Audit::File(path) if path == audit),
            "{}: {:?}",
            entry.name,
            entry.audited
        );
        assert!(root().join(audit).is_file(), "{audit} is not committed");
    }
}

#[test]
fn attack_near_misses_of_the_pin_are_refused() {
    let real = real_list();
    for (what, from, to) in [
        ("a longer version", "pinned: 0.2.16,", "pinned: 0.2.160,"),
        ("a requirement", "libm@0.2.16", "libm@^0.2.16"),
        ("an exact requirement", "libm@0.2.16", "libm@=0.2.16"),
        ("a pre-release", "libm@0.2.16", "libm@0.2.16-rc.1"),
        ("build metadata", "libm@0.2.16", "libm@0.2.16+vendored"),
        ("a leading zero", "libm@0.2.16", "libm@0.02.16"),
        ("two versions", "libm@0.2.16", "libm@0.2.16, 0.2.17"),
        ("no version", "libm@0.2.16", "libm"),
        ("another crate's name", "libm@0.2.16", "libm2@0.2.16"),
    ] {
        assert!(real.contains(from), "{what}: the file lacks {from:?}");
        let seeded = real.replace(from, to);
        let outcome = vetted::read_text(&seeded);
        let refused_here = outcome.is_err();
        // A list that reads cleanly must not hold the lock's libm 0.2.16 under another name or
        // version: the lock check refuses it instead.
        let refused_by_lock = outcome.as_ref().is_ok_and(|list| {
            !vetted::against_lock(
                list,
                &fs::read_to_string(root().join("Cargo.lock")).unwrap(),
            )
            .is_empty()
        });
        assert!(
            refused_here || refused_by_lock,
            "{what}: {to:?} passed as vetted: {outcome:#?}"
        );
    }
}

#[test]
fn attack_records_that_are_not_whole_are_refused() {
    let real = real_list();
    for (what, from, to) in [
        ("mislabelled", "deterministic:", "deterministically:"),
        ("unspaced", "no_std: yes", "no_std:yes"),
        (
            "emptied",
            "audited: projects/kit/notes/G24-libm-audit.md, the written audit",
            "audited: the written audit",
        ),
        (
            "an audit with no owner",
            "audited: projects/kit/notes/G24-libm-audit.md,",
            "audited: pending, piece,",
        ),
        (
            "an audit outside the repository",
            "audited: projects/kit/notes/",
            "audited: /projects/kit/notes/",
        ),
        ("the pin left out", "; \\\npinned: 0.2.16,", "; \\\n0.2.16,"),
    ] {
        assert!(real.contains(from), "{what}: the file lacks {from:?}");
        let seeded = real.replace(from, to);
        assert!(
            vetted::read_text(&seeded).is_err(),
            "{what}: a record that is not whole passed"
        );
    }
}

#[test]
fn attack_an_empty_or_missing_list_lets_nothing_through() {
    // cargo-deny would allow every crate with either; the scaffold refuses both.
    let real = real_list();
    let start = real.find("allow = [").unwrap();
    let end = start + real[start..].find("\n]\n").unwrap() + 3;
    let emptied = format!("{}allow = []\n{}", &real[..start], &real[end..]);
    let removed = format!("{}{}", &real[..start], &real[end..]);
    assert!(vetted::read_text(&emptied).is_err_and(|e| e[0].contains("empty")));
    assert!(vetted::read_text(&removed).is_err_and(|e| e[0].contains("not set")));
}

/// Every `.rs` file under `dir`, skipping build output and hidden folders. Explicit work list.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut work = vec![dir.to_path_buf()];
    while let Some(folder) = work.pop() {
        for entry in fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                if name != "target" && !name.starts_with('.') {
                    work.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

#[test]
fn attack_no_copy_of_sha256_is_left_in_the_repository() {
    // The first three round constants (cube roots of 2, 3 and 5) and the sixth and seventh words
    // of the initial hash value (square roots of 13 and 17), FIPS 180-4, 4.2.2 and 5.3.3, built
    // from pieces so this file does not hold them. The first two words of the initial value are
    // not looked for: they are also the leading bits of the square roots of 2 and 3, which maths
    // code holds (`0x1.6a09e667f3bcdp+0`) and Philox's key schedule uses (its second Weyl
    // constant), for reasons of their own.
    let needles = [
        "428a", "2f98", "7137", "4491", "b5c0", "fbcf", "9b05", "688c", "1f83", "d9ab",
    ]
    .chunks(2)
    .map(<[&str]>::concat)
    .collect::<Vec<String>>();
    let files = rust_files(&root().join("projects"));
    assert!(files.len() > 100, "only {} files were read", files.len());
    let mut copies = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path)
            .unwrap()
            .replace('_', "")
            .to_lowercase();
        for needle in &needles {
            if text.contains(needle.as_str()) {
                copies.push(format!("{}: {needle}", path.display()));
            }
        }
    }
    assert!(
        copies.is_empty(),
        "SHA-256 written out again; take it from the sha2 crate (29.1): {copies:#?}"
    );
}

#[test]
fn attack_toml_layouts_that_are_one_value_read_as_one() {
    // The same lint table, written as a header, as dotted keys and as an inline table, with
    // comments and the other quote style: one set of settings, as cargo reads it.
    let forms = [
        "[lints.clippy]\npanic = \"deny\" # a comment\n",
        "lints.clippy.panic = 'deny'\n",
        "[lints]\nclippy = { panic = \"deny\" }\n",
        "lints = { clippy = { panic = \"deny\" } }\n",
    ];
    let read: Vec<_> = forms
        .iter()
        .map(|f| manifest::lint_settings(f).unwrap())
        .collect();
    assert!(read.windows(2).all(|w| w[0] == w[1]), "{read:#?}");
    // What cargo refuses is refused: a key defined twice, a table defined twice, a value with no
    // key.
    for refused in [
        "[lints.clippy]\npanic = \"deny\"\npanic = \"warn\"\n",
        "[lints]\n[lints]\n",
        "[lints.clippy]\n= \"deny\"\n",
    ] {
        assert_eq!(manifest::lint_settings(refused), None, "{refused}");
    }
}
