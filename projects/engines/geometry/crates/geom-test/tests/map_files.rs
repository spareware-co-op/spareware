//! The map on disk and through the binary, in scratch workspaces: `write` then `check` passes, and
//! `check` fails, naming the file, when the committed map is stale or missing, when a header on
//! disk loses a field, and when a file is not UTF-8. The binary's exit codes are 0, 1 and 2 as its
//! header says, so CI fails exactly when what it checks is wrong.
//!
//! These tests write files and start the binary, which WebAssembly test runs cannot do; the
//! in-memory fixtures of every rule are in `map_seeded.rs`.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "fixtures; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken fixture must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![cfg(not(target_family = "wasm"))]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use geom_test::map::{MAP_FILE, Rule, repo};

const LIB: &str = "//! Spare Kit geometry kernel: a scratch crate.
//!
//! One job: being checked.
//!
//! Layer: 2, Geometry.
//!
//! Invariants this crate keeps:
//!
//! - One.
//!
//! Sections: 26.3.
#![forbid(unsafe_code)]
pub mod a;
";

const MODULE: &str = "//! A module.
//!
//! Guarantees: none broken.
//!
//! Method: chapter 26.3 (`projects/engines/geometry/architecture/26-how-it-grows.md`).
//!
//! Cost: constant.
";

/// An interval module that consulted LGPL code without a method note, as `geom-num`'s does.
const INTERVAL: &str = "//! Intervals.
//!
//! Guarantees: inclusion.
//!
//! Method: outward widening (doi:10.1000/182).
//!
//! Cost: constant.
//!
//! Consulted: CGAL `FPU.h`, `LGPL-3.0-or-later OR LicenseRef-Commercial`,
//!   <https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Number_types/include/CGAL/FPU.h#L347-L358>,
//!   for its widening.
";

/// A scratch workspace under `CARGO_TARGET_TMPDIR`, emptied first and removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("map-{name}"));
        let _ = fs::remove_dir_all(&root);
        let src = root.join("projects/engines/geometry/crates/geom-x/src");
        fs::create_dir_all(src.join("inner")).unwrap();
        fs::write(
            root.join("projects/engines/geometry/crates/geom-x/Cargo.toml"),
            "[package]\nname = \"geom-x\"\n",
        )
        .unwrap();
        fs::write(src.join("lib.rs"), LIB).unwrap();
        fs::write(src.join("a.rs"), MODULE).unwrap();
        fs::write(src.join("inner/deep.rs"), MODULE).unwrap();
        // Not a crate: a file in the crates folder, and a file in src that is not Rust.
        fs::write(
            root.join("projects/engines/geometry/crates/README.md"),
            "# Crates\n",
        )
        .unwrap();
        fs::write(src.join("notes.txt"), "not Rust\n").unwrap();
        Scratch(root)
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    fn module(&self) -> PathBuf {
        self.path("projects/engines/geometry/crates/geom-x/src/a.rs")
    }

    /// Adds a `geom-num` whose header has no Layer line and whose interval module consulted LGPL
    /// code with no method note, as the real ones do, so the binary's known gaps (`map::GAPS`)
    /// occur here too.
    fn with_the_known_gaps(self) -> Scratch {
        let num = self.path("projects/engines/geometry/crates/geom-num");
        fs::create_dir_all(num.join("src")).unwrap();
        fs::write(num.join("Cargo.toml"), "[package]\nname = \"geom-num\"\n").unwrap();
        fs::write(
            num.join("src/lib.rs"),
            LIB.replace("//! Layer: 2, Geometry.\n//!\n", "")
                .replace("pub mod a;\n", "pub mod interval;\n"),
        )
        .unwrap();
        fs::write(num.join("src/interval.rs"), INTERVAL).unwrap();
        self
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_map"))
        .args(args)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap(), text)
}

#[test]
fn write_then_check_passes_and_reads_every_file_in_order() {
    let s = Scratch::new("cycle");
    let (problems, wrote) = repo::write(&s.0, &[]).unwrap();
    assert_eq!(problems, []);
    assert!(wrote);
    let (problems, wrote) = repo::write(&s.0, &[]).unwrap();
    assert_eq!(problems, []);
    assert!(!wrote, "an unchanged map is not rewritten");
    assert_eq!(repo::check(&s.0, &[]).unwrap(), []);

    let text = fs::read_to_string(s.path(MAP_FILE)).unwrap();
    let map: serde_json::Value = serde_json::from_str(&text).unwrap();
    let modules: Vec<&str> = map["crates"][0]["modules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap())
        .collect();
    assert_eq!(modules, ["geom_x::a", "geom_x::inner::deep"]);
    assert_eq!(map["crates"].as_array().unwrap().len(), 1);
    assert!(
        !text.contains(&*s.0.to_string_lossy()),
        "no absolute path in the map"
    );

    // A Windows checkout's line ends are not a difference.
    fs::write(s.path(MAP_FILE), text.replace('\n', "\r\n")).unwrap();
    assert_eq!(repo::check(&s.0, &[]).unwrap(), []);
}

#[test]
fn check_fails_on_a_stale_or_missing_map() {
    let s = Scratch::new("stale");
    repo::write(&s.0, &[]).unwrap();
    let text = fs::read_to_string(s.path(MAP_FILE)).unwrap();
    fs::write(s.path(MAP_FILE), text.replace("constant.", "free.")).unwrap();
    let found = repo::check(&s.0, &[]).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule, Rule::Stale);
    fs::remove_file(s.path(MAP_FILE)).unwrap();
    let found = repo::check(&s.0, &[]).unwrap();
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("missing"), "{}", found[0]);
}

#[test]
fn check_names_the_file_and_field_a_header_on_disk_lost() {
    let s = Scratch::new("field");
    repo::write(&s.0, &[]).unwrap();
    fs::write(s.module(), MODULE.replace("//! Cost: constant.\n", "")).unwrap();
    let found: Vec<String> = repo::check(&s.0, &[])
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(found[0].starts_with("projects/engines/geometry/MAP.json: Stale map: "));
    assert!(found[1].starts_with("projects/engines/geometry/crates/geom-x/src/a.rs: Cost: "));

    fs::write(s.module(), [0xff, 0xfe, b'/', b'/']).unwrap();
    let found = repo::check(&s.0, &[]).unwrap();
    assert!(
        found
            .iter()
            .any(|p| p.rule == Rule::Unreadable && p.file.ends_with("/a.rs"))
    );
}

#[test]
fn a_pending_folder_is_checked_once_it_exists() {
    // The scratch's module links into the architecture folder, which it does not hold: pending.
    let s = Scratch::new("pending");
    repo::write(&s.0, &[]).unwrap();
    assert_eq!(repo::check(&s.0, &[]).unwrap(), []);
    // Once the folder exists, links into it must resolve, and the pending entry is stale.
    fs::create_dir_all(s.path("projects/engines/geometry/architecture")).unwrap();
    let found = repo::check(&s.0, &[]).unwrap();
    let found: Vec<(Rule, &str)> = found.iter().map(|p| (p.rule, p.file.as_str())).collect();
    assert!(
        found.contains(&(Rule::GapStale, "projects/engines/geometry/architecture/")),
        "{found:?}"
    );
    assert!(
        found.contains(&(
            Rule::Method,
            "projects/engines/geometry/crates/geom-x/src/a.rs"
        )),
        "{found:?}"
    );
    fs::write(
        s.path("projects/engines/geometry/architecture/26-how-it-grows.md"),
        "# 26
",
    )
    .unwrap();
    let found = repo::check(&s.0, &[]).unwrap();
    assert!(found.iter().all(|p| p.rule != Rule::Method), "{found:#?}");
}

#[test]
fn the_binary_exits_0_1_and_2() {
    let s = Scratch::new("binary").with_the_known_gaps();
    let root = s.0.to_str().unwrap();
    let (code, text) = run(&["check", "--root", root]);
    assert_eq!(code, 1, "no map yet: {text}");
    assert!(
        text.contains("MAP.json: Stale map: the map is missing"),
        "{text}"
    );
    let (code, text) = run(&["write", "--root", root]);
    assert_eq!(code, 0, "{text}");
    let (code, text) = run(&["check", "--root", root]);
    assert_eq!(code, 0, "{text}");
    fs::write(
        s.module(),
        MODULE.replace("Guarantees: none broken.", "Hopes: none broken."),
    )
    .unwrap();
    let (code, text) = run(&["check", "--root", root]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("geom-x/src/a.rs: Guarantees: "), "{text}");
    // `write` still writes the map, and still fails while a header is incomplete.
    let (code, text) = run(&["write", "--root", root]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("wrote"), "{text}");

    let (code, text) = run(&["frobnicate"]);
    assert_eq!(code, 2, "{text}");
    let (code, text) = run(&["check", "--root"]);
    assert_eq!(code, 2, "{text}");
    let empty = s.path("no-workspace-here");
    fs::create_dir_all(&empty).unwrap();
    let (code, text) = run(&["check", "--root", empty.to_str().unwrap()]);
    assert_eq!(
        code, 2,
        "a root without the crates folder is an error, not a pass: {text}"
    );
}
