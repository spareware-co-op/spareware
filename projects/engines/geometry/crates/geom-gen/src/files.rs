//! Writing every generator's output, and checking that every committed output is current.
//!
//! What it does: [`validate`] checks a registry's rules. [`write()`] produces every output and writes
//! each one whose bytes changed. [`check`] produces every output in memory and compares it with the
//! file, byte for byte, reporting each as current, different (with the first line that differs) or
//! missing. Both also report every orphan: a file in a generated folder that no generator owns.
//! Both work on a workspace root given by the caller, so tests can use a scratch copy.
//!
//! Guarantees:
//!
//! - **Byte for byte.** `check` compares bytes, not text: a changed line end, a trailing space or a
//!   missing final newline is a difference.
//! - **Validated and generated before anything is written.** `write` validates the registry and
//!   runs every generator before it writes any file, so a broken registry or a failing generator
//!   leaves every file as it was. An I/O error part-way (a full disk, a folder where a file should
//!   be) can leave the outputs before it written; `write` returns the error, and running it again
//!   once the cause is fixed finishes the job, because it rewrites only what differs.
//! - **Every file is reported.** `check` does not stop at the first difference: the report names
//!   every output that differs or is missing, and every orphan.
//! - **No orphans.** Every output lies inside one of the registry's generated folders ([`Folder`]),
//!   and every file in such a folder must be the output of a registered generator or on the folder's
//!   keep list. So the old output of a renamed or removed generator is reported as
//!   [`Status::Orphan`] until someone deletes it. Neither `write` nor `check` deletes anything.
//! - **One file per output, on every system CI runs.** An output path is relative, uses `/`, and has
//!   only plain components of the portable filename character set (`A`–`Z`, `a`–`z`, `0`–`9`, `.`,
//!   `_`, `-`), none longer than 255 bytes, none starting with `-` or ending with `.`, and none a
//!   Windows device name (`CON`, `PRN`, `AUX`, `NUL`, `COM0`–`COM9`, `LPT0`–`LPT9`, in any case,
//!   with or without an extension); the whole path is at most [`MAX_OUTPUT_LEN`] bytes. No two
//!   outputs are equal when letter case is ignored (Windows and macOS file systems ignore it), and
//!   no output lies inside another. So two generators can never write one file on any runner, and
//!   every output can be a file.
//!
//! Method: regenerate-and-compare, the way CI keeps generated code honest (chapter 27 of the kernel
//! architecture, `projects/engines/geometry/architecture/27-crates-folders.md`: "CI reruns every
//! generator and fails on any difference"). File names follow POSIX's portable filename character
//! set (<https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap03.html#tag_03_282>) and
//! Microsoft's file naming rules
//! (<https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file>).
//!
//! Cost: every generator once, plus reading or writing each output and listing each generated
//! folder (at most [`MAX_FOLDER_DEPTH`] folders deep, without recursion). Validation compares
//! every pair of outputs and of folders: O(n²) in their number, times their length.

use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use geom_test::Seed;

use crate::generator::{GenError, Generator};

/// The longest output path, in bytes: with a checkout folder of up to 59 characters, every output
/// fits Windows' classic 260-character path limit (`MAX_PATH`), which Git for Windows keeps unless
/// `core.longpaths` is set.
pub const MAX_OUTPUT_LEN: usize = 200;

/// The deepest a generated folder's files may lie below it.
pub const MAX_FOLDER_DEPTH: usize = 16;

/// The device names Windows reserves, with or without an extension, in any case.
const RESERVED: [&str; 24] = [
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
    "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// A folder only generators write into: every file in it must be the output of a registered
/// generator, or on its keep list.
#[derive(Clone, Copy, Debug)]
pub struct Folder {
    /// The folder, relative to the workspace root, with `/` separators, under the same rules as an
    /// output path.
    pub path: &'static str,
    /// Files in the folder, by name, that no generator owns and that belong there (for example a
    /// `.gitattributes`).
    pub keep: &'static [&'static str],
}

/// The generators and the folders they own.
#[derive(Clone, Copy, Debug)]
pub struct Registry<'a> {
    /// Every generator, in the order they run.
    pub generators: &'a [Generator],
    /// Every generated folder.
    pub folders: &'a [Folder],
}

/// What happened to one output, or to an orphan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// The file holds exactly the generator's bytes.
    Current,
    /// `write` wrote the file, because it was missing or different.
    Written,
    /// The file differs from the generator's bytes.
    Differs {
        /// The first line (from 1) on which they differ.
        line: usize,
        /// The committed file's length in bytes.
        committed: usize,
        /// The generated output's length in bytes.
        generated: usize,
    },
    /// The file does not exist.
    Missing,
    /// A file in a generated folder that no generator owns and that is not on the folder's keep
    /// list.
    Orphan,
}

/// One output's result, or an orphan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The generator's name, or `None` for an orphan.
    pub name: Option<&'static str>,
    /// The file, relative to the workspace root, with `/` separators.
    pub output: String,
    /// The generator's seed, if it uses randomness.
    pub seed: Option<Seed>,
    /// What happened.
    pub status: Status,
}

impl Outcome {
    /// Whether the file now holds its generator's bytes. An orphan never does.
    #[must_use]
    pub fn is_current(&self) -> bool {
        matches!(self.status, Status::Current | Status::Written)
    }

    /// One line for people: `<name>: <output>: <what happened>`.
    #[must_use]
    pub fn describe(&self) -> String {
        let what = match &self.status {
            Status::Current => "current".to_owned(),
            Status::Written => "written".to_owned(),
            Status::Differs {
                line,
                committed,
                generated,
            } => format!(
                "DIFFERS from its generator, first on line {line} \
                 ({committed} bytes committed, {generated} generated)"
            ),
            Status::Missing => "MISSING".to_owned(),
            Status::Orphan => "ORPHAN: no generator owns this file in a generated folder; delete \
                               it, or register the generator that makes it"
                .to_owned(),
        };
        let name = self.name.unwrap_or("(no generator)");
        format!("{name}: {}: {what}", self.output)
    }
}

fn registry_error(message: String) -> GenError {
    GenError::Registry(message)
}

/// Checks the registry's rules: unique kebab-case names; every folder and output a portable,
/// plain relative path (module header); no folder inside another; every output inside a folder and
/// not on its keep list; and no two outputs equal, ignoring case, or one inside another.
///
/// # Errors
///
/// [`GenError::Registry`] naming the first rule broken.
pub fn validate(registry: &Registry<'_>) -> Result<(), GenError> {
    let mut names = BTreeSet::new();
    for g in registry.generators {
        if !kebab(g.name) || !names.insert(g.name) {
            return Err(registry_error(format!(
                "generator name {:?} is not kebab case (a-z, 0-9 and inner hyphens) or is repeated",
                g.name
            )));
        }
    }

    let mut folders: Vec<Vec<String>> = Vec::new();
    for f in registry.folders {
        let parts = folded(&components(f.path, "folder")?);
        for keep in f.keep {
            if !plain_name(keep) {
                return Err(registry_error(format!(
                    "keep entry {keep:?} of folder {:?} is not one plain file name",
                    f.path
                )));
            }
        }
        if let Some(other) = folders
            .iter()
            .find(|o| starts_with(o, &parts) || starts_with(&parts, o))
        {
            return Err(registry_error(format!(
                "folder {:?} and folder {:?} overlap, ignoring case",
                f.path,
                other.join("/")
            )));
        }
        folders.push(parts);
    }

    // Every output, folded to one case and sorted: equal paths end up side by side, and a path that
    // contains another sorts before it, so each output is checked against those before it.
    let mut outputs: Vec<(Vec<String>, &str)> = Vec::new();
    for g in registry.generators {
        if g.output.len() > MAX_OUTPUT_LEN {
            return Err(registry_error(format!(
                "output {:?} is longer than {MAX_OUTPUT_LEN} bytes",
                g.output
            )));
        }
        let parts = components(g.output, "output")?;
        let folder = registry.folders.iter().find(|f| {
            let fp: Vec<&str> = f.path.split('/').collect();
            parts.len() > fp.len() && parts.iter().zip(&fp).all(|(a, b)| a == b)
        });
        let Some(folder) = folder else {
            return Err(registry_error(format!(
                "output {:?} is not inside a generated folder of the registry",
                g.output
            )));
        };
        // The output is the folder's components, then more: the folder's path, '/', the rest.
        let inside = g
            .output
            .get(folder.path.len()..)
            .and_then(|rest| rest.strip_prefix('/'))
            .unwrap_or_default();
        if folder.keep.contains(&inside) {
            return Err(registry_error(format!(
                "output {:?} is on the keep list of folder {:?}",
                g.output, folder.path
            )));
        }
        outputs.push((folded(&parts), g.output));
    }
    outputs.sort();
    for pair in outputs.windows(2) {
        if let [(a, a_text), (b, b_text)] = pair {
            if a == b {
                return Err(registry_error(format!(
                    "outputs {a_text:?} and {b_text:?} are one file where letter case is ignored \
                     (Windows, macOS)"
                )));
            }
        }
    }
    for (i, (b, b_text)) in outputs.iter().enumerate() {
        for (a, a_text) in outputs.iter().take(i) {
            if a.len() < b.len() && starts_with(b, a) {
                return Err(registry_error(format!(
                    "output {b_text:?} lies inside output {a_text:?}, which cannot then be a file"
                )));
            }
        }
    }
    Ok(())
}

/// Whether `name` is kebab case: lowercase ASCII letters, digits and hyphens, not starting or
/// ending with a hyphen.
fn kebab(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Whether `name` is one file name of the portable character set, not `.` or `..`.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 255
        && name.bytes().all(portable)
}

/// Whether a byte is in POSIX's portable filename character set.
fn portable(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-'
}

/// Whether `path` begins with all of `prefix`'s components.
fn starts_with(path: &[String], prefix: &[String]) -> bool {
    path.len() >= prefix.len() && path.iter().zip(prefix).all(|(a, b)| a == b)
}

/// The components, ASCII-lowercased: how a case-insensitive file system compares them. Only
/// ASCII is allowed, so this is exactly the comparison Windows and macOS make.
fn folded(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|p| p.to_ascii_lowercase()).collect()
}

/// The path's components, checked against the rules in the module header; `what` names the path
/// in errors.
fn components<'p>(path: &'p str, what: &str) -> Result<Vec<&'p str>, GenError> {
    let bad = |why: &str| registry_error(format!("{what} {path:?} {why}"));
    if path.is_empty() {
        return Err(bad("is empty"));
    }
    if path.contains('\\') {
        return Err(bad("uses \\; paths use / separators"));
    }
    let parts: Vec<&str> = path.split('/').collect();
    for p in &parts {
        if p.is_empty() || *p == "." || *p == ".." {
            return Err(bad(
                "is not a plain relative path (an empty, . or .. component)",
            ));
        }
        if !p.bytes().all(portable) {
            return Err(bad(
                "uses a character outside A-Z, a-z, 0-9, '.', '_' and '-' (POSIX's portable \
                 file names)",
            ));
        }
        if p.len() > 255 {
            return Err(bad("has a component longer than 255 bytes"));
        }
        if p.starts_with('-') {
            return Err(bad("has a component starting with '-'"));
        }
        if p.ends_with('.') {
            return Err(bad(
                "has a component ending with '.', which Windows drops, making it another name",
            ));
        }
        let stem = p.split('.').next().unwrap_or(p);
        if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
            return Err(bad(
                "has a component that is a Windows device name (CON, PRN, AUX, NUL, COM0-9, \
                 LPT0-9), with or without an extension",
            ));
        }
    }
    Ok(parts)
}

/// Where `output` lives under `root`.
///
/// # Errors
///
/// [`GenError::Registry`] if `output` is not a portable, plain relative path.
pub fn output_path(root: &Path, output: &str) -> Result<PathBuf, GenError> {
    let mut path = root.to_path_buf();
    for c in components(output, "output")? {
        path.push(c);
    }
    Ok(path)
}

/// Produces every generator's output, then writes each one whose bytes changed, then lists the
/// orphans.
///
/// # Errors
///
/// [`GenError::Registry`] for a broken registry, a generator's error, or [`GenError::Io`];
/// no file is written unless the registry is valid and every generator succeeded.
pub fn write(root: &Path, registry: &Registry<'_>) -> Result<Vec<Outcome>, GenError> {
    validate(registry)?;
    let produced = produce(registry.generators)?;
    let mut outcomes = Vec::new();
    for (g, bytes) in registry.generators.iter().zip(produced) {
        let path = output_path(root, g.output)?;
        let status = match read(&path)? {
            Some(old) if old == bytes => Status::Current,
            _ => {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| io(dir, &e))?;
                }
                std::fs::write(&path, &bytes).map_err(|e| io(&path, &e))?;
                Status::Written
            }
        };
        outcomes.push(outcome(g, status));
    }
    outcomes.extend(orphans(root, registry)?);
    Ok(outcomes)
}

/// Produces every generator's output in memory and compares it with the file under `root`, then
/// lists the orphans.
///
/// # Errors
///
/// [`GenError::Registry`] for a broken registry, a generator's error, or [`GenError::Io`]
/// for a file that exists but cannot be read or a generated folder that cannot be listed. A
/// missing or different file, and an orphan, are not errors: they are reported in the outcomes.
pub fn check(root: &Path, registry: &Registry<'_>) -> Result<Vec<Outcome>, GenError> {
    validate(registry)?;
    let produced = produce(registry.generators)?;
    let mut outcomes = Vec::new();
    for (g, bytes) in registry.generators.iter().zip(produced) {
        let path = output_path(root, g.output)?;
        let status = match read(&path)? {
            None => Status::Missing,
            Some(old) if old == bytes => Status::Current,
            Some(old) => Status::Differs {
                line: first_difference_line(&old, &bytes),
                committed: old.len(),
                generated: bytes.len(),
            },
        };
        outcomes.push(outcome(g, status));
    }
    outcomes.extend(orphans(root, registry)?);
    Ok(outcomes)
}

fn outcome(g: &Generator, status: Status) -> Outcome {
    Outcome {
        name: Some(g.name),
        output: g.output.to_owned(),
        seed: g.seed(),
        status,
    }
}

/// Every file in a generated folder under `root` that is neither a registered output nor on the
/// folder's keep list, folder by folder in registry order and by name within each, as
/// [`Status::Orphan`] outcomes. A generated folder that does not exist has no orphans.
///
/// # Errors
///
/// [`GenError::Io`] if a folder cannot be listed, or holds files more than
/// [`MAX_FOLDER_DEPTH`] folders deep.
fn orphans(root: &Path, registry: &Registry<'_>) -> Result<Vec<Outcome>, GenError> {
    let outputs: BTreeSet<&str> = registry.generators.iter().map(|g| g.output).collect();
    let mut found = Vec::new();
    for folder in registry.folders {
        let start = output_path(root, folder.path)?;
        if !start.is_dir() {
            continue;
        }
        // Depth-first with an explicit stack: (folder on disk, its path relative to the root, depth).
        let mut stack = vec![(start, folder.path.to_owned(), 0usize)];
        while let Some((dir, rel, depth)) = stack.pop() {
            let mut entries: Vec<(String, PathBuf, bool)> = Vec::new();
            for entry in std::fs::read_dir(&dir).map_err(|e| io(&dir, &e))? {
                let entry = entry.map_err(|e| io(&dir, &e))?;
                let kind = entry.file_type().map_err(|e| io(&entry.path(), &e))?;
                let name = entry.file_name().to_string_lossy().into_owned();
                entries.push((name, entry.path(), kind.is_dir()));
            }
            entries.sort();
            // Pushed in reverse, so folders are listed in name order.
            for (name, path, is_dir) in entries.into_iter().rev() {
                let child = format!("{rel}/{name}");
                if is_dir {
                    // A folder is pushed only at a depth of at most MAX_FOLDER_DEPTH, so depth + 1
                    // fits: `saturating_add` never saturates.
                    let deeper = depth.saturating_add(1);
                    if deeper > MAX_FOLDER_DEPTH {
                        return Err(GenError::Io {
                            path: path.display().to_string(),
                            message: format!(
                                "lies more than {MAX_FOLDER_DEPTH} folders inside generated \
                                 folder {}",
                                folder.path
                            ),
                        });
                    }
                    stack.push((path, child, deeper));
                } else if !outputs.contains(child.as_str())
                    && (depth != 0 || !folder.keep.contains(&name.as_str()))
                {
                    found.push(Outcome {
                        name: None,
                        output: child,
                        seed: None,
                        status: Status::Orphan,
                    });
                }
            }
        }
    }
    Ok(found)
}

/// Every generator's output, in registry order.
fn produce(generators: &[Generator]) -> Result<Vec<Vec<u8>>, GenError> {
    generators.iter().map(Generator::run).collect()
}

/// The file's bytes, or `None` if it does not exist.
fn read(path: &Path) -> Result<Option<Vec<u8>>, GenError> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(path, &e)),
    }
}

fn io(path: &Path, e: &std::io::Error) -> GenError {
    GenError::Io {
        path: path.display().to_string(),
        message: e.to_string(),
    }
}

/// The line (from 1) of the first byte at which `a` and `b` differ; if one is a prefix of the
/// other, the line where the shorter one ends.
#[must_use]
pub fn first_difference_line(a: &[u8], b: &[u8]) -> usize {
    let same = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let prefix = a.get(..same).unwrap_or(&[]);
    // Split at its n line ends, the prefix is n + 1 pieces: the line the difference is on.
    prefix.split(|&c| c == b'\n').count()
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;
    use crate::generator::Generate;

    #[allow(
        clippy::unnecessary_wraps,
        reason = "a generator's function returns a Result: `Generate::Plain` takes this signature"
    )]
    fn bytes() -> Result<Vec<u8>, GenError> {
        Ok(b"x\n".to_vec())
    }

    fn gen_at(name: &'static str, output: &'static str) -> Generator {
        Generator {
            name,
            output,
            about: "a test generator",
            generate: Generate::Plain(bytes),
        }
    }

    const FOLDERS: [Folder; 2] = [
        Folder {
            path: "generated",
            keep: &[".gitattributes"],
        },
        Folder {
            path: "other/Gen",
            keep: &[],
        },
    ];

    fn validate_outputs(outputs: &[&'static str]) -> Result<(), GenError> {
        let names = ["a", "b", "c", "d"];
        let generators: Vec<Generator> = outputs
            .iter()
            .zip(names)
            .map(|(o, n)| gen_at(n, o))
            .collect();
        validate(&Registry {
            generators: &generators,
            folders: &FOLDERS,
        })
    }

    #[test]
    fn plain_portable_relative_paths_only() {
        assert!(components("a/b/c.md", "output").is_ok());
        assert!(components("A_b-1/.hidden/x.tar.gz", "output").is_ok());
        assert!(components("console/conx.md/com10.md/lpt.md", "output").is_ok());
        for bad in [
            "",
            "/a",
            "a//b",
            "a/../b",
            "./a",
            "a/.",
            "C:/a",
            "a\\b",
            "a/",
            "..",
            "a/b c",
            "a/b:c",
            "a/é",
            "a/t.md.",
            "a/-x",
            "NUL",
            "a/nul",
            "a/aux.md",
            "a/Com1.tar.gz",
            "a/LPT9",
            "a/con.",
            "a/b*",
            "a/b?",
            "a/<b>",
            "a/b|c",
            "a/\"b\"",
        ] {
            assert!(components(bad, "output").is_err(), "{bad:?}");
        }
        let long = "x".repeat(256);
        assert!(components(&long, "output").is_err());
        assert!(components(&"x".repeat(255), "output").is_ok());
    }

    #[test]
    fn outputs_that_are_one_file_somewhere_are_refused() {
        assert!(validate_outputs(&["generated/a.md", "generated/b.md"]).is_ok());
        for (a, b) in [
            ("generated/Table.md", "generated/table.md"),
            ("generated/T.MD", "generated/t.md"),
            ("generated/x.md", "generated/x.md"),
            ("generated/x", "generated/x/y.md"),
            ("generated/x/y.md", "generated/X"),
            ("generated/d/e/f.md", "generated/D/e"),
        ] {
            assert!(
                matches!(validate_outputs(&[a, b]), Err(GenError::Registry(_))),
                "{a} and {b}"
            );
        }
        // Prefixes that are not whole components are not nesting.
        assert!(validate_outputs(&["generated/x", "generated/xy/z.md"]).is_ok());
        assert!(validate_outputs(&["generated/x.md", "generated/x.md.bak"]).is_ok());
    }

    #[test]
    fn outputs_must_lie_in_a_generated_folder_off_its_keep_list() {
        assert!(validate_outputs(&["other/Gen/x.md"]).is_ok());
        for bad in [
            "elsewhere/x.md",
            "generated",
            "other/gen/x.md",
            "other/Gen",
            "generated/.gitattributes",
        ] {
            assert!(
                matches!(validate_outputs(&[bad]), Err(GenError::Registry(_))),
                "{bad}"
            );
        }
        // A keep entry deeper in the folder is a plain output.
        assert!(validate_outputs(&["generated/sub/.gitattributes"]).is_ok());
    }

    #[test]
    fn names_and_folders_keep_their_rules() {
        let g = |name| gen_at(name, "generated/a.md");
        for bad in ["", "A", "a_b", "-a", "a-", "a b"] {
            let gens = [g(bad)];
            let r = validate(&Registry {
                generators: &gens,
                folders: &FOLDERS,
            });
            assert!(matches!(r, Err(GenError::Registry(_))), "{bad:?}");
        }
        let twice = [g("a"), gen_at("a", "generated/b.md")];
        assert!(
            validate(&Registry {
                generators: &twice,
                folders: &FOLDERS
            })
            .is_err()
        );
        let overlapping = [
            Folder {
                path: "gen",
                keep: &[],
            },
            Folder {
                path: "Gen/sub",
                keep: &[],
            },
        ];
        assert!(
            validate(&Registry {
                generators: &[],
                folders: &overlapping
            })
            .is_err()
        );
        let bad_keep = [Folder {
            path: "gen",
            keep: &["a/b"],
        }];
        assert!(
            validate(&Registry {
                generators: &[],
                folders: &bad_keep
            })
            .is_err()
        );
    }

    #[test]
    fn first_difference_line_counts_from_one() {
        assert_eq!(first_difference_line(b"a\nb\nc\n", b"a\nb\nX\n"), 3);
        assert_eq!(first_difference_line(b"abc", b"abd"), 1);
        assert_eq!(first_difference_line(b"a\n", b"a\nmore\n"), 2);
        assert_eq!(first_difference_line(b"", b"x"), 1);
        assert_eq!(first_difference_line(b"a\r\n", b"a\n"), 1);
    }
}
