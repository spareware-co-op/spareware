//! `fixture`: the fixtures' tool: list them, check them, run them, hold a pull request to their
//! rules, and carry out what a run asks for.
//!
//! ```text
//! cargo run -p geom-test --bin fixture -- list
//! cargo run -p geom-test --bin fixture -- check [--root <dir>]
//! cargo run -p geom-test --bin fixture -- run [--tier fast|slow|all]
//! cargo run -p geom-test --bin fixture -- one <id> [--root <dir>]
//! cargo run -p geom-test --bin fixture -- diff --base <commit> [--root <dir>]
//!     [--reviews <file> --head <commit> --author <login> [--permissions <file>]]
//! cargo run -p geom-test --bin fixture -- fixes --base <commit> [--root <dir>]
//! cargo run -p geom-test --bin fixture -- fails [--timeout-seconds <n>] [--root <dir>] <id>...
//! cargo run -p geom-test --bin fixture -- improve <id> [--root <dir>]
//! cargo run -p geom-test --bin fixture -- minimise <id> [--root <dir>]
//! cargo run -p geom-test --bin fixture -- seeds <dir>
//! cargo run -p geom-test --bin fixture -- record <id> --family <name> --index <n>
//!     --derivation <words> [--root <dir>]
//! ```
//!
//! - `list` prints every fixture compiled in: id, speed, status and source.
//! - `check` parses every fixture in the working tree and requires the set compiled into this
//!   binary to be the same, so a stale build cannot check an old set.
//! - `run` replays the fixtures of a tier through the kernel, under the store's current edition. A
//!   Fail fails, and so does an Improved: the fixture must move to `Pass` (28.16).
//! - `one` replays one fixture from the working tree; it exits 0 only on Pass. `fails` is what
//!   calls it.
//! - `diff` compares the fixtures at `<commit>` with the working tree's and holds the change to the
//!   rules of [`geom_test::fixture::diff`], with the golden store's approval rule.
//! - `fixes` prints, one per line, the ids of the fixtures new since `<commit>` that must fail on
//!   the merge base.
//! - `fails` runs at the merge base with the new fixtures copied in, and requires each to fail
//!   there: a run that exits 0 fails the check; an exit, a crash, or a hang past the timeout
//!   (default 600 seconds) is the bug the fixture catches (28.16, step 3).
//! - `improve` moves a `Known` fixture that now gives its wanted result to `Pass`, writing the new
//!   expectation to its file.
//! - `minimise` prints the fixture with its log minimised, keeping its failure's class.
//! - `seeds` writes every fixture's log into `<dir>`, named by its id, for the fuzz corpora.
//! - `record` stores a log of the replay corpus as a fixture, holding what the kernel gives today:
//!   for a case whose answer is known, which the words of `--derivation` state.
//!
//! Guarantees: only `improve`, `seeds` and `record` write files; `improve` and `record` only the
//! fixture named. Exit
//! codes: 0 when everything checked holds; 1 when a check fails; 2 for a usage error or a file
//! or command that cannot be read or run.
//!
//! Method: none; it parses its arguments and calls [`geom_test::fixture`], whose module headers
//! carry the methods.
//!
//! Cost: `run` replays each fixture of the tier twice per key of the edition; the others read the
//! fixtures once.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, ExitStatus, Stdio};
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};
use geom_test::fixture::kernel::Kernel;
use geom_test::fixture::minimise::{minimise, same_failure};
use geom_test::fixture::replay::seeded;
use geom_test::fixture::repo::{self, File, must_fail_on_base, parse_all, read_dir};
use geom_test::fixture::run::{Replayed, Verdict, replay, run};
use geom_test::fixture::{Expect, Fixture, Hash, Source, Speed, Status, diff};
use geom_test::golden::diff::approved;
use geom_test::golden::store::Store;
use geom_test::map::repo::workspace_root;

/// The fixtures' tool.
#[derive(Parser)]
#[command(name = "fixture")]
struct Cli {
    #[command(subcommand)]
    command: Tool,
}

#[derive(Args)]
struct Root {
    /// The workspace root; the one this binary was built in by default.
    #[arg(long)]
    root: Option<PathBuf>,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Tier {
    Fast,
    Slow,
    All,
}

#[derive(Subcommand)]
enum Tool {
    /// Print every fixture compiled in.
    List,
    /// Check every fixture in the working tree.
    Check(Root),
    /// Replay a tier's fixtures through the kernel.
    Run {
        /// The tier.
        #[arg(long, value_enum, default_value = "fast")]
        tier: Tier,
    },
    /// Replay one fixture from the working tree.
    One {
        /// Its id.
        id: String,
        #[command(flatten)]
        root: Root,
    },
    /// Hold what a pull request does to the stored fixtures to their rules.
    Diff {
        /// The base commit.
        #[arg(long)]
        base: String,
        /// GitHub's JSON list of the pull request's reviews.
        #[arg(long)]
        reviews: Option<PathBuf>,
        /// The pull request's head commit.
        #[arg(long)]
        head: Option<String>,
        /// The pull request's author.
        #[arg(long)]
        author: Option<String>,
        /// A JSON object of reviewers' repository permissions.
        #[arg(long)]
        permissions: Option<PathBuf>,
        #[command(flatten)]
        root: Root,
    },
    /// List the new fixtures that must fail on the merge base.
    Fixes {
        /// The base commit.
        #[arg(long)]
        base: String,
        #[command(flatten)]
        root: Root,
    },
    /// Require each fixture to fail here, on the merge base.
    Fails {
        /// How long one fixture may run before it counts as a hang.
        #[arg(long, default_value_t = 600)]
        timeout_seconds: u64,
        /// The fixtures.
        ids: Vec<String>,
        #[command(flatten)]
        root: Root,
    },
    /// Move a Known fixture that gives its wanted result to Pass.
    Improve {
        /// Its id.
        id: String,
        #[command(flatten)]
        root: Root,
    },
    /// Print a failing fixture with its log minimised.
    Minimise {
        /// Its id.
        id: String,
        #[command(flatten)]
        root: Root,
    },
    /// Write every fixture's log into a folder, for the fuzz corpora.
    Seeds {
        /// The folder.
        dir: PathBuf,
    },
    /// Store a log of the replay corpus as a fixture.
    Record {
        /// Its id.
        id: String,
        /// The family of the corpus.
        #[arg(long)]
        family: String,
        /// Which of the family's logs.
        #[arg(long)]
        index: usize,
        /// Where its answer comes from.
        #[arg(long)]
        derivation: String,
        #[command(flatten)]
        root: Root,
    },
}

fn main() -> ExitCode {
    match command(Cli::parse().command) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("fixture: {message}");
            ExitCode::from(2)
        }
    }
}

fn root_of(root: &Root) -> Result<PathBuf, String> {
    match &root.root {
        Some(r) => Ok(r.clone()),
        None => workspace_root().map_err(|e| e.to_string()),
    }
}

/// The current edition: the store's, as this binary was built with it.
fn edition() -> Result<String, String> {
    let store = Store::embedded().map_err(|e| e.to_string())?;
    let current = store.current().map_err(|e| e.to_string())?;
    Ok(current.edition.clone())
}

fn parsed(files: &[File]) -> Result<Vec<Fixture>, String> {
    parse_all(files).map_err(|problems| problems.join("\n"))
}

/// One fixture of the working tree.
fn tree_fixture(root: &Root, id: &str) -> Result<Fixture, String> {
    let fixtures = parsed(&read_dir(&root_of(root)?)?)?;
    fixtures
        .into_iter()
        .find(|f| f.id == id)
        .ok_or_else(|| format!("no fixture {id}"))
}

/// The output of `git <args>` in `root`, or `None` if git says no.
fn git(root: &Path, args: &[&str]) -> Result<Option<String>, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Ok(None);
    }
    String::from_utf8(out.stdout)
        .map(Some)
        .map_err(|e| format!("git: {e}"))
}

/// The fixture files at `commit`; none if the folder is not there yet.
fn files_at(root: &Path, commit: &str) -> Result<Vec<File>, String> {
    let dir = repo::DIR.join("/");
    let listing = git(root, &["ls-tree", "-r", "--name-only", commit, "--", &dir])?
        .ok_or_else(|| format!("no commit {commit}"))?;
    let mut files = Vec::new();
    let is_json = |p: &&str| Path::new(p).extension().is_some_and(|e| e == "json");
    for path in listing.lines().filter(is_json) {
        let text = git(root, &["show", &format!("{commit}:{path}")])?
            .ok_or_else(|| format!("cannot read {path} at {commit}"))?;
        let relative = path.strip_prefix(&dir).unwrap_or(path);
        files.push(File {
            path: relative.trim_start_matches('/').to_owned(),
            text,
        });
    }
    Ok(files)
}

fn list() -> Result<ExitCode, String> {
    for fixture in parsed(&repo::embedded())? {
        println!(
            "{} {:?} {} {:?}",
            fixture.id,
            fixture.speed,
            match fixture.status {
                Status::Pass => "pass",
                Status::Known { .. } => "known",
            },
            fixture.source
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn check(root: &Root) -> Result<ExitCode, String> {
    let files = read_dir(&root_of(root)?)?;
    match parse_all(&files) {
        Ok(fixtures) if files == repo::embedded() => {
            println!(
                "fixture check: {} fixtures hold their rules",
                fixtures.len()
            );
            Ok(ExitCode::SUCCESS)
        }
        Ok(_) => {
            eprintln!("fixture check: this binary was built with another set of fixtures");
            Ok(ExitCode::from(1))
        }
        Err(problems) => {
            for problem in problems {
                eprintln!("fixture check: {problem}");
            }
            Ok(ExitCode::from(1))
        }
    }
}

fn run_tier(tier: Tier) -> Result<ExitCode, String> {
    let edition = edition()?;
    let mut bad = 0_usize;
    let mut ran = 0_usize;
    for fixture in parsed(&repo::embedded())? {
        let wanted = match tier {
            Tier::Fast => fixture.speed == Speed::Fast,
            Tier::Slow => fixture.speed == Speed::Slow,
            Tier::All => true,
        };
        if !wanted {
            continue;
        }
        ran = ran.saturating_add(1);
        match run(&fixture, &edition, &mut Kernel::new()) {
            Verdict::Pass => println!("fixture {}: pass", fixture.id),
            Verdict::Improved => {
                bad = bad.saturating_add(1);
                eprintln!(
                    "fixture {}: improved: it now gives its wanted result; move it to Pass with \
                     `fixture improve`",
                    fixture.id
                );
            }
            Verdict::Fail(failure) => {
                bad = bad.saturating_add(1);
                eprintln!("fixture {}: fail: {failure}", fixture.id);
            }
        }
    }
    println!("fixture run: {ran} ran, {bad} did not pass");
    Ok(if bad == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn one(id: &str, root: &Root) -> Result<ExitCode, String> {
    let fixture = tree_fixture(root, id)?;
    match run(&fixture, &edition()?, &mut Kernel::new()) {
        Verdict::Pass => {
            println!("fixture {id}: pass");
            Ok(ExitCode::SUCCESS)
        }
        Verdict::Improved => {
            println!("fixture {id}: improved");
            Ok(ExitCode::from(1))
        }
        Verdict::Fail(failure) => {
            println!("fixture {id}: fail: {failure}");
            Ok(ExitCode::from(1))
        }
    }
}

struct Reviews<'a> {
    list: Option<&'a Path>,
    head: Option<&'a str>,
    author: Option<&'a str>,
    permissions: Option<&'a Path>,
}

fn diff_command(base: &str, reviews: &Reviews<'_>, root: &Root) -> Result<ExitCode, String> {
    let root = root_of(root)?;
    let before = parsed(&files_at(&root, base)?)?;
    let after = parsed(&read_dir(&root)?)?;
    let store = Store::embedded().map_err(|e| e.to_string())?;
    let d = diff::diff(&before, &after, |edition| {
        store
            .editions
            .iter()
            .find(|e| e.edition == edition)
            .map(|e| e.state)
    });
    for id in &d.added {
        println!("added: {id}");
    }
    for line in &d.free {
        println!("{line}");
    }
    for line in &d.approval {
        println!("needs a kernel owner's approval: {line}");
    }
    for line in &d.refused {
        eprintln!("refused: {line}");
    }
    if !d.refused.is_empty() {
        return Ok(ExitCode::from(1));
    }
    if d.approval.is_empty() {
        return Ok(ExitCode::SUCCESS);
    }
    let (Some(list), Some(head), Some(author)) = (reviews.list, reviews.head, reviews.author)
    else {
        eprintln!(
            "fixture diff: this change needs a kernel owner's approval; pass --reviews, --head \
             and --author to check the pull request's reviews"
        );
        return Ok(ExitCode::from(1));
    };
    let list = std::fs::read_to_string(list).map_err(|e| format!("{}: {e}", list.display()))?;
    let permissions: BTreeMap<String, String> = match reviews.permissions {
        None => BTreeMap::new(),
        Some(file) => std::fs::read_to_string(file)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
            .map_err(|e| format!("{}: {e}", file.display()))?,
    };
    match approved(&list, head, author, &permissions) {
        Ok(who) => {
            println!("fixture diff: approved on {head} by {who}");
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            eprintln!("fixture diff: {e}");
            Ok(ExitCode::from(1))
        }
    }
}

fn fixes(base: &str, root: &Root) -> Result<ExitCode, String> {
    let root = root_of(root)?;
    let before = parsed(&files_at(&root, base)?)?;
    let after = parsed(&read_dir(&root)?)?;
    for id in must_fail_on_base(&before, &after) {
        println!("{id}");
    }
    Ok(ExitCode::SUCCESS)
}

/// Waits for `child` up to `limit`; `None` if it is still running.
#[cfg(not(target_family = "wasm"))]
fn wait(child: &mut Child, limit: Duration) -> std::io::Result<Option<ExitStatus>> {
    wait_timeout::ChildExt::wait_timeout(child, limit)
}

/// WebAssembly starts no processes, so there is never a child to time.
#[cfg(target_family = "wasm")]
fn wait(child: &mut Child, _limit: Duration) -> std::io::Result<Option<ExitStatus>> {
    child.wait().map(Some)
}

fn fails(timeout_seconds: u64, ids: &[String], root: &Root) -> Result<ExitCode, String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let root = root_of(root)?;
    let mut bad = 0_usize;
    for id in ids {
        let mut child = Command::new(&me)
            .args(["one", id])
            .arg("--root")
            .arg(&root)
            .stdout(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("{id}: {e}"))?;
        let status = wait(&mut child, Duration::from_secs(timeout_seconds))
            .map_err(|e| format!("{id}: {e}"))?;
        if status.is_none() {
            child.kill().map_err(|e| format!("{id}: {e}"))?;
            child.wait().map_err(|e| format!("{id}: {e}"))?;
        }
        if status.is_some_and(|s| s.success()) {
            bad = bad.saturating_add(1);
            eprintln!(
                "fixture fails: {id} passes on the merge base, so it does not prove it catches the \
                 bug (28.16, step 3)"
            );
        } else {
            println!("fixture fails: {id} fails on the merge base, as it must");
        }
    }
    Ok(if bad == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn improve(id: &str, root: &Root) -> Result<ExitCode, String> {
    let path_root = root_of(root)?;
    let mut fixture = tree_fixture(root, id)?;
    let edition = edition()?;
    if run(&fixture, &edition, &mut Kernel::new()) != Verdict::Improved {
        eprintln!("fixture improve: {id} did not improve");
        return Ok(ExitCode::from(1));
    }
    let keys: Vec<_> = fixture
        .expect
        .keys()
        .filter(|(e, _)| *e == edition)
        .cloned()
        .collect();
    for key in keys {
        let log = fixture.log.at_versions(&key.1);
        let Replayed::Done {
            hash,
            measures,
            work,
        } = replay(&mut Kernel::new(), &log)
        else {
            eprintln!("fixture improve: {id} did not succeed under {:?}", key.1);
            return Ok(ExitCode::from(1));
        };
        fixture.expect.insert(
            key,
            Expect::Ok {
                hash: Some(Hash(hash.0)),
                work: Some(work),
                measures,
            },
        );
    }
    fixture.status = Status::Pass;
    let text = fixture.to_json().map_err(|e| e.to_string())?;
    let path = repo::DIR
        .iter()
        .fold(path_root, |p, part| p.join(part))
        .join(format!("{id}.json"));
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("fixture improve: {id} is now Pass");
    Ok(ExitCode::SUCCESS)
}

fn minimise_command(id: &str, root: &Root) -> Result<ExitCode, String> {
    let fixture = tree_fixture(root, id)?;
    let edition = edition()?;
    let mut engine = Kernel::new();
    let Verdict::Fail(failure) = run(&fixture, &edition, &mut engine) else {
        eprintln!("fixture minimise: {id} does not fail");
        return Ok(ExitCode::from(1));
    };
    let small = minimise(&fixture.log, |log| {
        same_failure(&fixture, &edition, &mut engine, &failure, log)
    })
    .map_err(|e| e.to_string())?;
    let mut smaller = fixture.clone();
    smaller.log = small;
    print!("{}", smaller.to_json().map_err(|e| e.to_string())?);
    Ok(ExitCode::SUCCESS)
}

fn seeds(dir: &Path) -> Result<ExitCode, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let fixtures = parsed(&repo::embedded())?;
    for fixture in &fixtures {
        let bytes = fixture.log.encode().map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.skgf", fixture.id.replace('/', "-")));
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    println!("fixture seeds: {} logs", fixtures.len());
    Ok(ExitCode::SUCCESS)
}

fn record(
    id: &str,
    family: &str,
    index: usize,
    derivation: &str,
    root: &Root,
) -> Result<ExitCode, String> {
    let logs = seeded(family)?.ok_or_else(|| format!("no family {family}"))?;
    let log = logs
        .get(index)
        .ok_or_else(|| format!("{family} has no log {index}"))?
        .clone();
    let Some(expect) = replay(&mut Kernel::new(), &log).expectation() else {
        return Err(format!("{family} {index} is broken"));
    };
    let fixture = Fixture {
        id: id.to_owned(),
        source: Source::ClosedForm {
            derivation: derivation.to_owned(),
        },
        fixed_by: None,
        speed: Speed::Fast,
        expect: BTreeMap::from([((edition()?, log.versions()), expect)]),
        log,
        status: Status::Pass,
        history: Vec::new(),
    };
    fixture.check().map_err(|e| e.to_string())?;
    let path = repo::DIR
        .iter()
        .fold(root_of(root)?, |p, part| p.join(part))
        .join(format!("{id}.json"));
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    let text = fixture.to_json().map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("fixture record: {id}");
    Ok(ExitCode::SUCCESS)
}

fn command(tool: Tool) -> Result<ExitCode, String> {
    match tool {
        Tool::List => list(),
        Tool::Check(root) => check(&root),
        Tool::Run { tier } => run_tier(tier),
        Tool::One { id, root } => one(&id, &root),
        Tool::Diff {
            base,
            reviews,
            head,
            author,
            permissions,
            root,
        } => diff_command(
            &base,
            &Reviews {
                list: reviews.as_deref(),
                head: head.as_deref(),
                author: author.as_deref(),
                permissions: permissions.as_deref(),
            },
            &root,
        ),
        Tool::Fixes { base, root } => fixes(&base, &root),
        Tool::Fails {
            timeout_seconds,
            ids,
            root,
        } => fails(timeout_seconds, &ids, &root),
        Tool::Improve { id, root } => improve(&id, &root),
        Tool::Minimise { id, root } => minimise_command(&id, &root),
        Tool::Seeds { dir } => seeds(&dir),
        Tool::Record {
            id,
            family,
            index,
            derivation,
            root,
        } => record(&id, &family, index, &derivation, &root),
    }
}
