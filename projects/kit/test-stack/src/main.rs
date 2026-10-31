//! The Spare Construct test stack: the program that wires the kit's crates into one server. See
//! README.md for what it proves, deploy/README.md for putting it online, and note K08 for why it
//! is shaped this way. Thin on purpose: the rules are in the kit, in the accounts service, and in
//! this crate's `commands` and `service`.

use clap::{Parser, Subcommand};
use kit_accounts::rows::Project;
use kit_accounts::{Access, Accounts, DirectoryCommands, SignInSettings};
use kit_store::key::list;
use kit_store::{Kv, Scope, StoreError, scan_rows};
use kit_store_tikv::{Files, Tikv};
use kit_sync::{Clock, Orderers, SystemClock, verify_log};
use std::sync::Arc;
use std::time::Duration;
use test_stack::commands::ProjectCommands;
use test_stack::http::{self, App, Shared};
use test_stack::service::Service;
use test_stack::settings::Settings;
use test_stack::{backup, seed};

/// How long TiKV keeps old versions of a key, and how often it is told so.
const KEEP_VERSIONS_MS: u64 = 15 * 60 * 1000;
const COLLECT_EVERY: Duration = Duration::from_secs(5 * 60);

#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    settings: Settings,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Runs the web server.
    Serve,
    /// Writes the example hubs, people and projects into a store, through the edit logs.
    Seed,
    /// Checks every log, and every project's stored pieces against its log.
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

fn plain(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn build(settings: Settings, kv: Tikv, files: Files) -> Result<Shared, StoreError> {
    let access = Arc::new(Access::new(kv.clone())?);
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let directory = Orderers::new(
        kv.clone(),
        Arc::new(DirectoryCommands::new(access.clone())),
        clock.clone(),
        settings.tail_ms,
    );
    let sign_in = SignInSettings {
        issuer: settings.oidc_issuer.clone(),
        client_id: settings.oidc_client_id.clone(),
        client_secret: settings.oidc_client_secret.clone(),
        public_url: settings.public_url.clone(),
    };
    let accounts = Accounts::new(kv.clone(), clock.clone(), sign_in, directory);
    let commands = ProjectCommands::new(kv.clone(), access.clone(), settings.claim_ms);
    let orderers = Orderers::new(
        kv.clone(),
        Arc::new(commands),
        clock.clone(),
        settings.tail_ms,
    );
    let service = Service::new(kv, access, accounts, orderers, clock, files);
    Ok(Arc::new(App { service, settings }))
}

async fn serve(app: Shared, kv: Tikv) -> Result<(), String> {
    app.service
        .accounts
        .apply_people(&app.settings.people)
        .await
        .map_err(plain)?;
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(COLLECT_EVERY).await;
            if let Err(e) = kv.collect_garbage(KEEP_VERSIONS_MS).await {
                tracing::warn!("garbage collection: {e}");
            }
        }
    });
    let listener = tokio::net::TcpListener::bind(&app.settings.bind)
        .await
        .map_err(plain)?;
    println!("listening on {}", app.settings.bind);
    axum::serve(listener, http::routes(app))
        .await
        .map_err(plain)
}

/// Checks the directory's log and every project's.
async fn verify(kv: &Tikv) -> Result<(), String> {
    let mut snapshot = kv.snapshot().await.map_err(plain)?;
    let check = verify_log(&mut snapshot, &Scope::Directory).await?;
    println!("directory: {} edits, ok", check.entries);
    let projects: Vec<Project> = scan_rows(&mut snapshot, &list::projects())
        .await
        .map_err(plain)?;
    let mut bad = 0;
    for project in projects {
        match verify_log(&mut snapshot, &Scope::Project(project.id.clone())).await {
            Ok(check) => {
                println!(
                    "{}: {} edits, {} elements, ok",
                    project.id, check.entries, check.elements
                );
            }
            Err(e) => {
                println!("{}: FAILED: {e}", project.id);
                bad += 1;
            }
        }
    }
    if bad > 0 {
        return Err(format!("{bad} projects failed"));
    }
    Ok(())
}

async fn run(cli: Cli) -> Result<(), String> {
    let settings = cli.settings;
    let kv = Tikv::connect(&settings.pd).await.map_err(plain)?;
    let files = Files::open(&settings.files_url).map_err(plain)?;
    let backups = Files::open(&settings.backup_url).map_err(plain)?;
    match cli.command {
        Command::Serve => {
            let app = build(settings, kv.clone(), files).map_err(plain)?;
            serve(app, kv).await
        }
        Command::Seed => {
            let app = build(settings, kv.clone(), files).map_err(plain)?;
            seed::seed(&kv, &app.service).await.map_err(plain)
        }
        Command::Verify => verify(&kv).await,
        Command::Backup { name } => backup::backup(&kv, &files, &backups, &name)
            .await
            .map_err(plain),
        Command::Restore { name } => backup::restore(&kv, &files, &backups, &name)
            .await
            .map_err(plain),
        Command::Digest => {
            println!("{}", backup::digest(&kv, &files).await.map_err(plain)?);
            Ok(())
        }
        Command::Collect => kv.collect_garbage(0).await.map_err(plain),
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    run(Cli::parse()).await
}
