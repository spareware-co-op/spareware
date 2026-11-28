//! Spare Construct's cloud server: the program that wires the kit's crates and Construct's commands
//! into one server. See README.md for what it proves, deploy/README.md for putting it online, and
//! note K08 for why it is shaped this way. Thin on purpose: the rules are in the kit, in
//! `construct-api`, and in this crate's `service`.

use clap::{Parser, Subcommand};
use construct_api::cloud::{POLICY, ProjectCommands, SCHEMA};
use construct_model::geometry::Geometry;
use construct_model::keys::list;
use construct_model::rows::Project;
use construct_regen::regenerator::Regenerator;
use kit_accounts::{Accounts, Cedar, SignInSettings};
use kit_core::Id;
use kit_store::{Kv, Scope, StoreError, scan_rows};
use kit_store_tikv::{Files, Tikv};
use kit_sync::{Clock, Orderers, SystemClock, verify_log};
use spare_server::directory::DirectoryCommands;
use spare_server::http::{self, App, Shared};
use spare_server::report::Reports;
use spare_server::service::{Access, Service};
use spare_server::settings::Settings;
use spare_server::{backup, seed};
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::Layer as _;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

/// How long TiKV keeps old versions of a key, and how often it is told so.
const KEEP_VERSIONS_MS: u64 = 15 * 60 * 1000;
const COLLECT_EVERY: Duration = Duration::from_mins(5);

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
    /// Checks every log, every project's stored pieces against its log, and every stored shape
    /// against its recipe.
    Verify,
    /// Throws away a project's derived shapes and rebuilds them from its log.
    Regenerate { project: String },
    /// Writes every key and file to the backup location.
    Backup { name: String },
    /// Reads a backup into an empty store.
    Restore { name: String },
    /// Prints one hash over every key and file.
    Digest,
    /// Clears old versions of rewritten keys out of TiKV now, as a running server does by itself.
    Collect,
    /// The co-op's side of the reports people send (piece A100): read, replay, mark and group them.
    Report {
        #[command(subcommand)]
        action: ReportAction,
    },
}

#[derive(Subcommand)]
enum ReportAction {
    /// The one line a fixer reads first, then the replay's edits that ended otherwise.
    Line { number: u64 },
    /// The whole report, as JSON (its screenshot is kept apart).
    Show { number: u64 },
    /// Replays the report again into an empty project, and keeps what it found.
    Replay { number: u64 },
    /// The generic public issue a reproduced fault becomes: its title, a blank line, its body.
    Issue { number: u64 },
    /// Records the public issue the report became, by its address.
    Linked { number: u64, issue: String },
    /// Marks the report fixed in a version of the program.
    Fixed { number: u64, version: String },
    /// Every piece of feedback, grouped by its topic.
    Feedback,
}

fn plain(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn build(
    settings: Settings,
    kv: Tikv,
    files: Files,
    geometry: Arc<dyn Geometry>,
) -> Result<Shared, StoreError> {
    let cedar = Cedar::new(POLICY, SCHEMA).map_err(StoreError::Damaged)?;
    let access: Arc<Access<Tikv>> = Arc::new(Access::new(kv.clone(), cedar));
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
    let commands = ProjectCommands::new(
        kv.clone(),
        access.clone(),
        settings.claim_ms,
        geometry.edition(),
    );
    let orderers = Orderers::new(
        kv.clone(),
        Arc::new(commands),
        clock.clone(),
        settings.tail_ms,
    );
    let service = Service::new(kv, access, accounts, orderers, clock, files, geometry)
        .with_drawer(spare_server::geometry::drawer());
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

/// Checks the directory's log and every project's, and rebuilds every shape to compare it.
async fn verify(kv: &Tikv, files: Files, geometry: Arc<dyn Geometry>) -> Result<(), String> {
    let regen = Regenerator::new(kv.clone(), files, geometry);
    let mut snapshot = kv.snapshot().await.map_err(plain)?;
    let check = verify_log(&mut snapshot, &Scope::Directory).await?;
    println!("directory: {} edits, ok", check.entries);
    let projects: Vec<Project> = scan_rows(&mut snapshot, &list::projects())
        .await
        .map_err(plain)?;
    let mut bad = 0_usize;
    for project in projects {
        let mut check = verify_log(&mut snapshot, &Scope::Project(project.id.clone())).await;
        let mut shapes = 0;
        if check.is_ok() {
            regen.catch_up(&project.id).await.map_err(plain)?;
            match regen.verify(&project.id).await {
                Ok(checked) => shapes = checked,
                Err(e) => check = Err(e),
            }
        }
        match check {
            Ok(check) => {
                println!(
                    "{}: {} edits, {} elements, {shapes} shapes, ok",
                    project.id, check.entries, check.pieces
                );
            }
            Err(e) => {
                println!("{}: FAILED: {e}", project.id);
                bad = bad.saturating_add(1);
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
    let geometry = spare_server::geometry::geometry(settings.geometry)?;
    match cli.command {
        Command::Serve => {
            let app = build(settings, kv.clone(), files, geometry).map_err(plain)?;
            serve(app, kv).await
        }
        Command::Seed => {
            let app = build(settings, kv.clone(), files, geometry).map_err(plain)?;
            seed::seed(&kv, &app.service).await.map_err(plain)
        }
        Command::Verify => verify(&kv, files, geometry).await,
        Command::Regenerate { project } => {
            let project = Id::parse(&project).map_err(plain)?;
            let regen = Regenerator::new(kv.clone(), files, geometry);
            let pass = regen.rebuild(&project).await.map_err(plain)?;
            println!("{project}: regenerated through {}", pass.through);
            Ok(())
        }
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
        Command::Report { action } => {
            let app = build(settings, kv, files, geometry).map_err(plain)?;
            report(&app.service.reports, action).await
        }
    }
}

/// The operators' report commands. They read and write the reports store only.
async fn report(reports: &Reports<Tikv>, action: ReportAction) -> Result<(), String> {
    match action {
        ReportAction::Line { number } => {
            let stored = reports
                .get(number)
                .await
                .map_err(plain)?
                .ok_or_else(|| format!("There is no report {number}."))?;
            println!("{}", stored.line());
            if let Some(replay) = &stored.replay {
                for step in replay.differing() {
                    let then = step.then.as_deref().unwrap_or("applied");
                    let now = step.now.as_deref().unwrap_or("applied");
                    println!(
                        "  edit {} {}: then {then}; in the replay {now}",
                        step.seq, step.cmd
                    );
                }
                if let Some(note) = &replay.note {
                    println!("  {note}");
                }
            }
        }
        ReportAction::Show { number } => {
            let stored = reports
                .get(number)
                .await
                .map_err(plain)?
                .ok_or_else(|| format!("There is no report {number}."))?;
            println!("{}", serde_json::to_string_pretty(&stored).map_err(plain)?);
        }
        ReportAction::Replay { number } => {
            println!("{}", reports.replay_again(number).await?.line());
        }
        ReportAction::Issue { number } => {
            let issue = reports.issue(number).await?;
            println!(
                "{}

{}",
                issue.title, issue.body
            );
        }
        ReportAction::Linked { number, issue } => {
            println!("{}", reports.linked(number, &issue).await?.line());
        }
        ReportAction::Fixed { number, version } => {
            println!("{}", reports.fixed(number, &version).await?.line());
        }
        ReportAction::Feedback => {
            for (topic, numbers) in reports.feedback().await.map_err(plain)? {
                let numbers: Vec<String> = numbers.iter().map(u64::to_string).collect();
                println!("{}: {}", topic.label(), numbers.join(" "));
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), String> {
    // The printed log, as RUST_LOG says; beside it, the lines of each request kept a little while
    // for the person's own reports (spare_server::recent).
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_filter(tracing_subscriber::EnvFilter::from_default_env()),
        )
        .with(spare_server::recent::layer())
        .init();
    run(Cli::parse()).await
}
