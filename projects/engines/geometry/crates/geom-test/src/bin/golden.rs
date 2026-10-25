//! `golden`: the golden store's tool: list it, check it against the repository, run every golden in
//! one CI leg's configuration, and hold a pull request's edits to the owner-approval rule.
//!
//! ```text
//! cargo run -p geom-test --bin golden -- list
//! cargo run -p geom-test --bin golden -- check [--root <dir>]
//! cargo run -p geom-test --bin golden -- run [<cargo test arguments>...]
//! cargo run -p geom-test --bin golden -- diff --base <commit> [--root <dir>]
//!     [--reviews <file> --head <commit> --author <login> [--permissions <file>]]
//! ```
//!
//! - `list` prints every edition and golden of the store.
//! - `check` reads the store from the working tree and checks it against the kernel's sources
//!   ([`geom_test::golden::repo`]): every golden is checked where the store says, every golden
//!   check in the code is in the store, no golden digest is kept outside it, and every pinned
//!   golden file is unchanged. It also requires the store this binary was built with to be the
//!   working tree's, so a stale build cannot check an old store.
//! - `run` runs every golden of the current edition with `cargo test`, the given arguments added
//!   (`--release`, `--target wasm32-wasip1`, ...) and the environment as it is (`RUSTFLAGS`,
//!   `CARGO_TARGET_DIR`, a target runner), one invocation per crate and test target, prints each
//!   run's output, and requires that each ran exactly its goldens and that each agreed with the
//!   store ([`geom_test::golden::matrix`]).
//! - `diff` compares the store at `<commit>` (read with `git show`; none there is an empty base)
//!   with the working tree's and prints what changed ([`geom_test::golden::diff`]). A change no
//!   approval can make fails. A change that needs a kernel owner's approval fails unless the
//!   reviews given (GitHub's JSON list of the pull request's reviews, and optionally a JSON object
//!   of reviewers' repository permissions) approve the head commit.
//!
//! Guarantees: `list`, `check`, `run` and `diff` never write a file. Exit codes: 0 when everything
//! checked holds; 1 when a check fails (a problem, a failed or unproven golden, a refused or
//! unapproved edit); 2 for a usage error or a file or command that cannot be read or run.
//!
//! Method: none; it parses its arguments and calls [`geom_test::golden`], whose module headers
//! carry the methods.
//!
//! Cost: `run` costs one `cargo test` per crate and test target; the other commands read the store
//! and, for `check`, the kernel's sources once.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use geom_test::golden::diff::{approved, diff};
use geom_test::golden::matrix::{plan, verify};
use geom_test::golden::repo;
use geom_test::golden::store::{EMBEDDED, STORE_PATH, Store};
use geom_test::map::repo::workspace_root;

const USAGE: &str = "usage: golden list | check [--root <dir>] | run [<cargo test args>...] | \
                     diff --base <commit> [--root <dir>] [--reviews <file> --head <commit> \
                     --author <login> [--permissions <file>]]";

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let mut args = args.into_iter();
    let command = args.next().and_then(|a| a.into_string().ok());
    let rest: Vec<String> = match args.map(OsString::into_string).collect() {
        Ok(rest) => rest,
        Err(bad) => return usage(&format!("argument {bad:?} is not Unicode")),
    };
    match command.as_deref() {
        Some("list") if rest.is_empty() => list(),
        Some("check") => match options(&rest, &["--root"]) {
            Ok(o) => with_root(&o, check),
            Err(m) => usage(&m),
        },
        Some("run") => run(&rest),
        Some("diff") => match options(
            &rest,
            &[
                "--base",
                "--root",
                "--reviews",
                "--head",
                "--author",
                "--permissions",
            ],
        ) {
            Ok(o) => with_root(&o, |root| diff_command(root, &o)),
            Err(m) => usage(&m),
        },
        Some(other) => usage(&format!("unknown command or arguments: {other:?}")),
        None => usage("no command"),
    }
}

fn usage(message: &str) -> ExitCode {
    eprintln!("golden: {message}\n{USAGE}");
    ExitCode::from(2)
}

fn fail(message: &str) -> ExitCode {
    eprintln!("golden: {message}");
    ExitCode::from(2)
}

/// `--name value` pairs, each name at most once and from `allowed`.
fn options(rest: &[String], allowed: &[&str]) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    let mut it = rest.iter();
    while let Some(name) = it.next() {
        if !allowed.contains(&name.as_str()) {
            return Err(format!("unexpected argument {name:?}"));
        }
        let Some(value) = it.next() else {
            return Err(format!("{name} needs a value"));
        };
        if out.insert(name.clone(), value.clone()).is_some() {
            return Err(format!("{name} given twice"));
        }
    }
    Ok(out)
}

fn with_root(options: &BTreeMap<String, String>, f: impl FnOnce(&Path) -> ExitCode) -> ExitCode {
    let root = match options.get("--root") {
        Some(r) => PathBuf::from(r),
        None => match workspace_root() {
            Ok(r) => r,
            Err(e) => return fail(&e.to_string()),
        },
    };
    f(&root)
}

fn store_path(root: &Path) -> PathBuf {
    STORE_PATH.iter().fold(root.to_path_buf(), |p, c| p.join(c))
}

fn list() -> ExitCode {
    let store = match Store::embedded() {
        Ok(s) => s,
        Err(e) => return fail(&e.to_string()),
    };
    for edition in &store.editions {
        println!(
            "edition {} ({:?}), {} golden(s)",
            edition.edition,
            edition.state,
            edition.goldens.len()
        );
        for g in &edition.goldens {
            let at = match (&g.check, &g.file) {
                (Some(c), _) => format!("{} --test {} {}", c.krate, c.test, c.function),
                (None, Some(f)) => format!("file {f}"),
                (None, None) => String::new(),
            };
            println!("  {} {}  [{at}]", g.sha256, g.id);
        }
    }
    ExitCode::SUCCESS
}

fn check(root: &Path) -> ExitCode {
    let path = store_path(root);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => return fail(&format!("{}: {e}", path.display())),
    };
    let store = match Store::parse(&text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("golden check: {e}");
            return ExitCode::from(1);
        }
    };
    let mut failed = false;
    if text.replace("\r\n", "\n") != EMBEDDED.replace("\r\n", "\n") {
        eprintln!(
            "golden check: this binary was built with another store than {}; rebuild it",
            path.display()
        );
        failed = true;
    }
    match repo::check(root, &store) {
        Err(e) => return fail(&e),
        Ok(problems) => {
            for p in &problems {
                eprintln!("{p}");
            }
            failed |= !problems.is_empty();
        }
    }
    let goldens = store.current().map_or(0, |e| e.goldens.len());
    if failed {
        eprintln!("golden check: the store and the repository disagree");
        return ExitCode::from(1);
    }
    println!(
        "golden check: {goldens} golden(s) in the current edition, each checked where the store \
         says; no golden outside the store; every pinned file unchanged"
    );
    ExitCode::SUCCESS
}

fn run(extra: &[String]) -> ExitCode {
    let store = match Store::embedded() {
        Ok(s) => s,
        Err(e) => return fail(&e.to_string()),
    };
    let root = match workspace_root() {
        Ok(r) => r,
        Err(e) => return fail(&e.to_string()),
    };
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let groups = plan(&store);
    if groups.is_empty() {
        eprintln!("golden run: the store's current edition has no golden checked by a test");
        return ExitCode::from(1);
    }
    let mut failures = Vec::new();
    for group in &groups {
        let args = group.cargo_args(extra);
        println!("golden run: cargo {}", args.join(" "));
        let child = Command::new(&cargo)
            .args(&args)
            .current_dir(&root)
            .stdout(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => return fail(&format!("cannot run cargo: {e}")),
        };
        let mut output = String::new();
        if let Some(stdout) = child.stdout.take() {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                println!("{line}");
                output.push_str(&line);
                output.push('\n');
            }
        }
        let status = match child.wait() {
            Ok(s) => s,
            Err(e) => return fail(&format!("cargo did not finish: {e}")),
        };
        let proven = verify(group, &output);
        let what = format!("{} --test {}", group.krate, group.test);
        match (status.success(), proven) {
            (true, Ok(())) => println!("golden run: {what}: {} golden(s) agree", group.ids.len()),
            (_, Err(e)) => failures.push(format!("{what}: {e}")),
            (false, Ok(())) => failures.push(format!("{what}: cargo failed ({status})")),
        }
    }
    let files: Vec<String> = store
        .current()
        .map(|e| e.goldens.iter().filter_map(|g| g.file.clone()).collect())
        .unwrap_or_default();
    for f in &files {
        println!("golden run: {f} is pinned by digest and checked by `golden check`");
    }
    if failures.is_empty() {
        let total: usize = groups
            .iter()
            .map(|g| g.ids.len())
            .fold(0, usize::saturating_add);
        println!(
            "golden run: every golden of the current edition agrees ({total} golden(s) in {} \
             run(s), arguments {extra:?})",
            groups.len()
        );
        ExitCode::SUCCESS
    } else {
        for f in &failures {
            eprintln!("golden run: FAILED: {f}");
        }
        ExitCode::from(1)
    }
}

/// The store at `base`, read with `git show`; `None` when the commit has no store.
fn store_at(root: &Path, base: &str) -> Result<Option<Store>, String> {
    let output = Command::new("git")
        .arg("show")
        .arg(format!("{base}:{}", STORE_PATH.join("/")))
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // git's words when the commit exists but the path does not.
        if stderr.contains("does not exist in") || stderr.contains("exists on disk, but not in") {
            return Ok(None);
        }
        return Err(format!("git show {base}: {}", stderr.trim()));
    }
    let text = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    Store::parse(&text)
        .map(Some)
        .map_err(|e| format!("the store at {base}: {e}"))
}

fn diff_command(root: &Path, o: &BTreeMap<String, String>) -> ExitCode {
    let Some(base) = o.get("--base") else {
        return usage("diff needs --base <commit>");
    };
    let head_text = match std::fs::read_to_string(store_path(root)) {
        Ok(t) => t,
        Err(e) => return fail(&e.to_string()),
    };
    let head = match Store::parse(&head_text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("golden diff: the store in the working tree: {e}");
            return ExitCode::from(1);
        }
    };
    let base_store = match store_at(root, base) {
        Ok(s) => s,
        Err(e) => return fail(&e),
    };
    let d = diff(base_store.as_ref(), &head);
    println!("{d}");
    if !d.is_acceptable() {
        return ExitCode::from(1);
    }
    if !d.needs_approval() {
        return ExitCode::SUCCESS;
    }
    let (Some(reviews), Some(head_commit), Some(author)) =
        (o.get("--reviews"), o.get("--head"), o.get("--author"))
    else {
        eprintln!(
            "golden diff: this change needs a kernel owner's approval; pass --reviews, --head and \
             --author to check the pull request's reviews"
        );
        return ExitCode::from(1);
    };
    let reviews = match std::fs::read_to_string(reviews) {
        Ok(t) => t,
        Err(e) => return fail(&format!("{reviews}: {e}")),
    };
    let permissions: BTreeMap<String, String> = match o.get("--permissions") {
        None => BTreeMap::new(),
        Some(file) => match std::fs::read_to_string(file)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
        {
            Ok(p) => p,
            Err(e) => return fail(&format!("{file}: {e}")),
        },
    };
    match approved(&reviews, head_commit, author, &permissions) {
        Ok(who) => {
            println!("golden diff: approved on {head_commit} by {who}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("golden diff: {e}");
            ExitCode::from(1)
        }
    }
}
