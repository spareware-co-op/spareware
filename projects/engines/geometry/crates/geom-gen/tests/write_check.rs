//! Write, then check: what CI relies on, in scratch copies of the workspace.
//!
//! - Writing every output and then checking passes, through the library and through the binary CI
//!   runs (`cargo run -p geom-gen -- check`).
//! - A hand-edited output fails the check, which names the file and the first line that differs,
//!   and the binary exits 1; so does a missing output, a change of line ends alone, and an orphan:
//!   a file in a generated folder that no generator owns.
//! - A seeded generator prints its seed, takes it from the registry, and a change of seed changes
//!   its output, so the check catches it. Every command prints the seeds, even when a generator
//!   fails.
//! - A broken registry or a failing generator writes nothing, and neither does a refused command
//!   line.
//! - The committed outputs in this workspace are current and have no orphan beside them, which is
//!   the check CI runs.
//!
//! The scratch copies live under cargo's `CARGO_TARGET_TMPDIR`, one folder per test and process,
//! removed when the test ends. Every test here uses the host's file system or runs the binary, so
//! none runs under WebAssembly; `grid_steps`'s unit tests compare the committed output there.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use geom_gen::files::{self, Status, output_path};
use geom_gen::registry::{REGISTRY, workspace_root};
use geom_gen::{Folder, GenError, Generate, Generator, Registry};
use geom_test::{Seed, SplitMix64};

/// A scratch folder, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Scratch {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("geom-gen-{label}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs the geom-gen binary with `args` and `--root <root>`.
fn bin(args: &[&str], root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_geom-gen"))
        .args(args)
        .arg("--root")
        .arg(root)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Every file under `dir`, relative to it, with `/` separators, sorted.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap();
                out.push(
                    rel.components()
                        .map(|c| c.as_os_str().to_str().unwrap().to_owned())
                        .collect::<Vec<_>>()
                        .join("/"),
                );
            }
        }
    }
    out.sort();
    out
}

#[cfg_attr(target_family = "wasm", ignore = "uses the host's file system")]
#[test]
fn write_then_check_passes() {
    let s = Scratch::new("write-then-check");
    let written = files::write(&s.0, &REGISTRY).unwrap();
    assert!(written.iter().all(|o| o.status == Status::Written));
    assert_eq!(written.len(), REGISTRY.generators.len());
    let checked = files::check(&s.0, &REGISTRY).unwrap();
    assert!(
        checked.iter().all(|o| o.status == Status::Current),
        "{checked:?}"
    );
    // Writing again changes nothing.
    let again = files::write(&s.0, &REGISTRY).unwrap();
    assert!(again.iter().all(|o| o.status == Status::Current));

    // The binary, as CI runs it.
    let out = bin(&["check"], &s.0);
    let stdout = text(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{stdout}{}", text(&out.stderr));
    assert!(
        stdout.contains("grid-steps: seed: none (uses no randomness)"),
        "{stdout}"
    );
    assert!(stdout.contains("every output is current"), "{stdout}");
}

#[cfg_attr(target_family = "wasm", ignore = "uses the host's file system")]
#[test]
fn a_hand_edited_output_fails_check() {
    let s = Scratch::new("hand-edited");
    files::write(&s.0, &REGISTRY).unwrap();
    for g in REGISTRY.generators {
        let path = output_path(&s.0, g.output).unwrap();
        let original = std::fs::read(&path).unwrap();

        // Change one character on the fifth line.
        let mut edited = original.clone();
        let fifth = edited
            .iter()
            .enumerate()
            .filter(|&(_, &b)| b == b'\n')
            .nth(3)
            .map(|(i, _)| i + 1)
            .unwrap();
        edited[fifth] = if edited[fifth] == b'#' { b'%' } else { b'#' };
        std::fs::write(&path, &edited).unwrap();

        let checked = files::check(&s.0, &REGISTRY).unwrap();
        let o = checked.iter().find(|o| o.name == Some(g.name)).unwrap();
        assert_eq!(
            o.status,
            Status::Differs {
                line: 5,
                committed: original.len(),
                generated: original.len()
            }
        );
        assert!(!o.is_current());

        let out = bin(&["check"], &s.0);
        let stdout = text(&out.stdout);
        let stderr = text(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{stdout}{stderr}");
        assert!(stderr.contains(g.output), "{stderr}");
        assert!(stderr.contains("DIFFERS"), "{stderr}");
        assert!(stderr.contains("first on line 5"), "{stderr}");

        // Line ends alone are a difference: a checkout that converted them must fail.
        let crlf = text(&original).replace('\n', "\r\n");
        std::fs::write(&path, crlf).unwrap();
        let checked = files::check(&s.0, &REGISTRY).unwrap();
        assert!(matches!(
            checked
                .iter()
                .find(|o| o.name == Some(g.name))
                .unwrap()
                .status,
            Status::Differs { line: 1, .. }
        ));

        // Writing repairs it.
        files::write(&s.0, &REGISTRY).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}

#[cfg_attr(target_family = "wasm", ignore = "uses the host's file system")]
#[test]
fn a_missing_output_fails_check() {
    let s = Scratch::new("missing");
    let checked = files::check(&s.0, &REGISTRY).unwrap();
    assert!(checked.iter().all(|o| o.status == Status::Missing));
    let out = bin(&["check"], &s.0);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("MISSING"));
}

/// The old output of a renamed or removed generator, or any other file a generated folder should
/// not hold, is an orphan: `check` and `write` report it and fail, and nothing deletes it. Files on
/// the folder's keep list are not orphans.
#[cfg_attr(target_family = "wasm", ignore = "uses the host's file system")]
#[test]
fn an_orphan_in_a_generated_folder_fails_check() {
    let s = Scratch::new("orphan");
    files::write(&s.0, &REGISTRY).unwrap();
    let folder = output_path(&s.0, REGISTRY.folders[0].path).unwrap();
    for keep in REGISTRY.folders[0].keep {
        std::fs::write(folder.join(keep), "* -text\n").unwrap();
    }
    assert!(
        files::check(&s.0, &REGISTRY)
            .unwrap()
            .iter()
            .all(files::Outcome::is_current)
    );

    // An old output left behind, and one in a subfolder.
    std::fs::write(folder.join("old-table.md"), "stale\n").unwrap();
    std::fs::create_dir_all(folder.join("sub")).unwrap();
    std::fs::write(folder.join("sub").join("deep.md"), "stale\n").unwrap();
    let base = REGISTRY.folders[0].path;
    for outcomes in [
        files::check(&s.0, &REGISTRY).unwrap(),
        files::write(&s.0, &REGISTRY).unwrap(),
    ] {
        let orphans: Vec<&str> = outcomes
            .iter()
            .filter(|o| o.status == Status::Orphan)
            .map(|o| o.output.as_str())
            .collect();
        assert_eq!(
            orphans,
            [
                format!("{base}/old-table.md"),
                format!("{base}/sub/deep.md")
            ]
        );
        assert!(
            outcomes
                .iter()
                .filter(|o| o.status == Status::Orphan)
                .all(|o| o.name.is_none() && !o.is_current())
        );
    }
    for command in ["check", "write"] {
        let out = bin(&[command], &s.0);
        let stderr = text(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{command}: {stderr}");
        assert!(stderr.contains("old-table.md: ORPHAN"), "{stderr}");
    }
    // Nothing was deleted.
    assert!(folder.join("old-table.md").is_file());
    assert!(folder.join("sub").join("deep.md").is_file());
}

/// A test-only seeded generator: eight numbers from `SplitMix64`, one per line.
#[allow(
    clippy::unnecessary_wraps,
    reason = "a generator's function returns a Result: `Generate::Seeded` takes this signature"
)]
fn seeded_numbers(seed: Seed) -> Result<Vec<u8>, GenError> {
    let mut rng = SplitMix64::new(seed);
    let mut out = String::new();
    for _ in 0..8 {
        writeln!(out, "{}", rng.next_u64()).unwrap();
    }
    Ok(out.into_bytes())
}

const TEST_FOLDERS: [Folder; 1] = [Folder {
    path: "generated",
    keep: &[],
}];

fn seeded(seed: Seed) -> [Generator; 1] {
    [Generator {
        name: "seeded-numbers",
        output: "generated/seeded-numbers.txt",
        about: "a test generator",
        generate: Generate::Seeded(seed, seeded_numbers),
    }]
}

fn registry(generators: &[Generator]) -> Registry<'_> {
    Registry {
        generators,
        folders: &TEST_FOLDERS,
    }
}

#[cfg_attr(target_family = "wasm", ignore = "uses the host's file system")]
#[test]
fn a_seeded_generator_prints_and_keeps_its_seed() {
    let s = Scratch::new("seeded");
    let generators = seeded(Seed(0x2a));
    assert_eq!(
        generators[0].seed_line(),
        "seeded-numbers: seed: 000000000000002a"
    );
    let written = files::write(&s.0, &registry(&generators)).unwrap();
    assert_eq!(written[0].seed, Some(Seed(0x2a)));
    assert!(files::check(&s.0, &registry(&generators)).unwrap()[0].is_current());
    // The same seed reproduces the output; another seed is a different output, which check catches.
    let other = seeded(Seed(0x2b));
    assert!(matches!(
        files::check(&s.0, &registry(&other)).unwrap()[0].status,
        Status::Differs { line: 1, .. }
    ));
}

#[cfg_attr(target_family = "wasm", ignore = "uses the host's file system")]
#[test]
fn a_broken_registry_or_generator_writes_nothing() {
    let s = Scratch::new("broken");
    let failing =
        |_: Seed| -> Result<Vec<u8>, GenError> { Err(GenError::generator("fails", "on purpose")) };
    let generators = [
        seeded(Seed(1))[0],
        Generator {
            name: "fails",
            output: "generated/fails.txt",
            about: "a generator that fails",
            generate: Generate::Seeded(Seed(1), failing),
        },
    ];
    assert_eq!(
        files::write(&s.0, &registry(&generators)),
        Err(GenError::generator("fails", "on purpose"))
    );
    // The first generator succeeded, but nothing was written.
    assert_eq!(files_under(&s.0), Vec::<String>::new());

    let mut twice = seeded(Seed(1)).to_vec();
    twice.push(twice[0]);
    let mut case_twin = seeded(Seed(1)).to_vec();
    case_twin.push(Generator {
        name: "twin",
        output: "generated/Seeded-Numbers.txt",
        ..case_twin[0]
    });
    let mut nested = seeded(Seed(1)).to_vec();
    nested.push(Generator {
        name: "nested",
        output: "generated/seeded-numbers.txt/inner.txt",
        ..nested[0]
    });
    let mut escaping = seeded(Seed(1));
    escaping[0].output = "../outside.txt";
    let mut outside_folders = seeded(Seed(1));
    outside_folders[0].output = "elsewhere/numbers.txt";
    let mut device = seeded(Seed(1));
    device[0].output = "generated/aux.txt";
    for (what, generators) in [
        ("a repeated output", twice),
        ("outputs that differ only in case", case_twin),
        ("an output inside another", nested),
        ("an output outside the root", escaping.to_vec()),
        (
            "an output outside every generated folder",
            outside_folders.to_vec(),
        ),
        ("a Windows device name", device.to_vec()),
    ] {
        assert!(
            matches!(
                files::write(&s.0, &registry(&generators)),
                Err(GenError::Registry(_))
            ),
            "{what}"
        );
        assert!(files_under(&s.0).is_empty(), "{what}");
    }
}

/// The binary prints every generator's seed before anything can fail: a `write` that fails on an
/// I/O error (a file where the generated folder should be) exits 2, with every seed line printed.
#[cfg_attr(target_family = "wasm", ignore = "runs the binary as a child process")]
#[test]
fn seeds_are_printed_before_anything_can_fail() {
    let s = Scratch::new("seeds-first");
    // A file where the generated folder should be: writing must fail with an I/O error.
    let folder_parent = output_path(&s.0, "projects/engines/geometry/crates/geom-gen").unwrap();
    std::fs::create_dir_all(&folder_parent).unwrap();
    std::fs::write(folder_parent.join("generated"), "not a folder\n").unwrap();
    let out = bin(&["write"], &s.0);
    let stdout = text(&out.stdout);
    assert_eq!(out.status.code(), Some(2), "{stdout}{}", text(&out.stderr));
    for g in REGISTRY.generators {
        assert!(stdout.contains(&g.seed_line()), "{stdout}");
    }
}

/// A refused command line writes nothing, even with a usable root: each command below carries
/// `--root <scratch>` and is refused for something else.
#[cfg_attr(target_family = "wasm", ignore = "runs the binary as a child process")]
#[test]
fn usage_errors_exit_2_and_write_nothing() {
    let s = Scratch::new("usage");
    let root = s.0.to_str().unwrap();
    for args in [
        &["bogus", "--root", root][..],
        &["write", "extra", "--root", root][..],
        &["write", "--root", root, "extra"][..],
        &["write", "--root", root, "--root", root][..],
        &["--root", root, "write"][..],
        &["check", "--root"][..],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_geom-gen"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(text(&out.stderr).contains("usage: geom-gen"), "{args:?}");
        assert!(files_under(&s.0).is_empty(), "{args:?} wrote something");
    }
    // The scratch root is usable: the same root with a valid command does write.
    let out = bin(&["write"], &s.0);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_ne!(files_under(&s.0), Vec::<String>::new());
}

#[cfg_attr(target_family = "wasm", ignore = "uses the host's file system")]
#[test]
fn the_committed_outputs_are_current() {
    let root = workspace_root().unwrap();
    let checked = files::check(&root, &REGISTRY).unwrap();
    for o in &checked {
        assert!(
            o.is_current(),
            "{}\nrun `cargo run -p geom-gen -- write` and commit the result",
            o.describe()
        );
    }
}
