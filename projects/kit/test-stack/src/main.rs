//! The Spare Construct test stack: one small server that proves the cloud design end to end.
//! See README.md for what it proves, deploy/README.md for putting it online.

mod access;
mod api;
mod auth;
mod backup;
mod config;
mod folders;
mod keys;
mod kv;
mod orderer;
mod rows;
mod seed;
mod ws;

use access::Access;
use anyhow::{Context as _, Result, bail};
use api::{Shared, StackState};
use clap::{Parser, Subcommand};
use config::Config;
use object_store::ObjectStore;
use object_store::prefix::PrefixStore;
use orderer::Orderers;
use rows::Project;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

/// How long TiKV keeps old versions of a key, and how often it is told so.
const KEEP_VERSIONS_MS: u64 = 15 * 60 * 1000;
const COLLECT_EVERY: Duration = Duration::from_secs(5 * 60);

#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    config: Config,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Runs the web server.
    Serve,
    /// Writes the example hubs, people and projects into an empty store.
    Seed,
    /// Checks every project's stored pieces against its edit log.
    Verify,
    /// Writes every key and file to the backup location.
    Backup { name: String },
    /// Reads a backup into an empty store.
    Restore { name: String },
    /// Prints one hash over every key and file.
    Digest,
    /// Clears old versions of rewritten keys out of TiKV now, as a running server does by itself.
    Collect,
}

/// Opens an object store from its URL. Credentials come from the usual AWS_ and GOOGLE_ settings,
/// so RustFS, S3 and Google Cloud Storage differ only in settings.
fn open_store(address: &str) -> Result<Arc<dyn ObjectStore>> {
    let url = Url::parse(address).with_context(|| format!("store address {address}"))?;
    let options = std::env::vars()
        .filter(|(name, _)| name.starts_with("AWS_") || name.starts_with("GOOGLE_"))
        .map(|(name, value)| (name.to_lowercase(), value));
    let (store, prefix) = object_store::parse_url_opts(&url, options)?;
    if prefix.as_ref().is_empty() {
        return Ok(Arc::from(store));
    }
    Ok(Arc::new(PrefixStore::new(store, prefix)))
}

async fn connect(config: Config) -> Result<Shared> {
    let kv = kv::Kv::connect(&config.pd).await?;
    let access = Arc::new(Access::new(kv.clone())?);
    let orderers = Orderers::new(kv.clone(), access.clone(), config.tail_ms, config.claim_ms);
    let files = open_store(&config.files_url)?;
    Ok(Arc::new(StackState {
        config,
        kv,
        access,
        orderers,
        files,
        oidc: Default::default(),
    }))
}

async fn serve(st: Shared) -> Result<()> {
    auth::apply_people(&st).await?;
    let kv = st.kv.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(COLLECT_EVERY).await;
            if let Err(e) = kv.collect_garbage(KEEP_VERSIONS_MS).await {
                tracing::warn!("garbage collection: {e:#}");
            }
        }
    });
    let listener = tokio::net::TcpListener::bind(&st.config.bind).await?;
    println!("listening on {}", st.config.bind);
    axum::serve(listener, api::routes(st)).await?;
    Ok(())
}

async fn verify(st: &Shared) -> Result<()> {
    let projects: Vec<Project> = st.kv.scan_rows("dir/project/").await?;
    let mut bad = 0;
    for project in projects {
        match orderer::verify_project(&st.kv, &project.id).await {
            Ok(check) => {
                println!(
                    "{}: {} edits, {} elements, ok",
                    project.id, check.entries, check.elements
                )
            }
            Err(e) => {
                println!("{}: FAILED: {e:#}", project.id);
                bad += 1;
            }
        }
    }
    if bad > 0 {
        bail!("{bad} projects failed");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    let backup_url = cli.config.backup_url.clone();
    let st = connect(cli.config).await?;
    match cli.command {
        Command::Serve => serve(st).await,
        Command::Seed => seed::seed(&st).await,
        Command::Verify => verify(&st).await,
        Command::Backup { name } => {
            backup::backup(&st.kv, &st.files, &open_store(&backup_url)?, &name).await
        }
        Command::Restore { name } => {
            backup::restore(&st.kv, &st.files, &open_store(&backup_url)?, &name).await
        }
        Command::Digest => {
            println!("{}", backup::digest(&st.kv, &st.files).await?);
            Ok(())
        }
        Command::Collect => st.kv.collect_garbage(0).await,
    }
}
