//! `corpus`: the corpus register's tool: list it, check it and the corpora folder, fetch what it
//! lists, and print the digest a new entry needs.
//!
//! ```text
//! cargo run -p geom-test --bin corpus -- list
//! cargo run -p geom-test --bin corpus -- check [--root <dir>]
//! cargo run -p geom-test --bin corpus -- fetch --cache <dir> [<id>...]
//! cargo run -p geom-test --bin corpus -- digest --scratch <dir> <id>
//! ```
//!
//! - `list` prints every corpus: id, storage, licence and version.
//! - `check` reads the register from the working tree, requires it to be the one this binary was
//!   built with, and holds the corpora folder to it ([`geom_test::corpus::check_vendored`]): an
//!   unlisted, unlicensed or changed file fails.
//! - `fetch` brings the given corpora into the cache, or every fetched one if none is named, each
//!   checked against the register's digest.
//! - `digest` downloads a corpus's pinned source and prints the digest of what came, for a new or
//!   changed entry. It writes nothing but the scratch folder.
//!
//! Guarantees: only `fetch` and `digest` write, and only under the folder given. Exit codes: 0 when
//! everything checked holds; 1 when a check fails; 2 for a usage error or a file
//! or command that cannot be read or run.
//!
//! Method: none; it parses its arguments and calls [`geom_test::corpus`], whose header carries the
//! method.
//!
//! Cost: `check` reads every vendored byte once; `fetch` costs its downloads.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use geom_test::corpus::{
    EMBEDDED, Fetched, MANIFEST_PATH, Manifest, Storage, check_vendored, digest_source, fetch,
};
use geom_test::map::repo::workspace_root;

/// The corpus register's tool.
#[derive(Parser)]
#[command(name = "corpus")]
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

#[derive(Subcommand)]
enum Tool {
    /// Print every corpus.
    List,
    /// Check the register and the corpora folder.
    Check(Root),
    /// Fetch corpora into a cache.
    Fetch {
        /// The cache folder.
        #[arg(long)]
        cache: PathBuf,
        /// The corpora; every fetched one by default.
        ids: Vec<String>,
    },
    /// Print the digest of a corpus's pinned source.
    Digest {
        /// A folder to download into.
        #[arg(long)]
        scratch: PathBuf,
        /// The corpus.
        id: String,
    },
}

fn main() -> ExitCode {
    match command(Cli::parse().command) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("corpus: {message}");
            ExitCode::from(2)
        }
    }
}

fn manifest() -> Result<Manifest, String> {
    Manifest::embedded().map_err(|e| e.to_string())
}

fn list() -> Result<ExitCode, String> {
    for corpus in manifest()?.corpora {
        println!(
            "{} {:?} {} ({})",
            corpus.id, corpus.storage, corpus.licence, corpus.version
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn check(root: &Root) -> Result<ExitCode, String> {
    let root = match &root.root {
        Some(r) => r.clone(),
        None => workspace_root().map_err(|e| e.to_string())?,
    };
    let path = MANIFEST_PATH.iter().fold(root, |p, part| p.join(part));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let manifest = match Manifest::parse(&text) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("corpus check: {e}");
            return Ok(ExitCode::from(1));
        }
    };
    if text != EMBEDDED {
        eprintln!("corpus check: this binary was built with another register");
        return Ok(ExitCode::from(1));
    }
    let dir = path
        .parent()
        .ok_or_else(|| format!("{}: no folder", path.display()))?;
    let problems = check_vendored(&manifest, dir)?;
    for problem in &problems {
        eprintln!("corpus check: {problem}");
    }
    if problems.is_empty() {
        println!(
            "corpus check: {} corpora hold their rules",
            manifest.corpora.len()
        );
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn fetch_command(cache: &std::path::Path, ids: &[String]) -> Result<ExitCode, String> {
    let manifest = manifest()?;
    let wanted: Vec<_> = if ids.is_empty() {
        manifest
            .corpora
            .iter()
            .filter(|c| c.storage == Storage::Fetched)
            .collect()
    } else {
        ids.iter()
            .map(|id| manifest.get(id).ok_or_else(|| format!("no corpus {id}")))
            .collect::<Result<_, _>>()?
    };
    let mut bad = 0_usize;
    for corpus in wanted {
        match fetch(corpus, cache) {
            Ok(Fetched::Cached) => println!("{}: already here", corpus.id),
            Ok(Fetched::Downloaded) => println!("{}: fetched", corpus.id),
            Err(e) => {
                bad = bad.saturating_add(1);
                eprintln!("corpus fetch: {e}");
            }
        }
    }
    Ok(if bad == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn digest(scratch: &std::path::Path, id: &str) -> Result<ExitCode, String> {
    let manifest = manifest()?;
    let corpus = manifest.get(id).ok_or_else(|| format!("no corpus {id}"))?;
    println!("{}", String::from(digest_source(corpus, scratch)?));
    Ok(ExitCode::SUCCESS)
}

fn command(tool: Tool) -> Result<ExitCode, String> {
    match tool {
        Tool::List => list(),
        Tool::Check(root) => check(&root),
        Tool::Fetch { cache, ids } => fetch_command(&cache, &ids),
        Tool::Digest { scratch, id } => digest(&scratch, &id),
    }
}
