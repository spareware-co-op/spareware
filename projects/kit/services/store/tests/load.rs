//! The direct `tikv-client` route under load: the orderer on real TiKV, with no web server in the
//! way. Going direct is the less-travelled path, and "many writers at once" is why TiKV was
//! chosen, so these are the numbers that decide whether the choice holds. They need a running
//! TiKV at STACK_PD. The figures in Spare Construct's server README come from here.

use kit_core::Id;
use kit_ops::{Change, Edit, sha256_hex};
use kit_store::{Kv, Place, Reader, Scope, StoreError, Txn, read_row, to_row};
use kit_store_tikv::{Tikv, TikvTxn};
use kit_sync::{Applied, Commands, Context, Orderers, Outcome, Reply, SystemClock};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// One command: rewrite a piece.
struct Rewrite;

impl Commands<Tikv> for Rewrite {
    fn version(&self) -> u32 {
        1
    }

    async fn permit(&self, _scope: &Scope, _edit: &Edit) -> Result<Result<(), String>, StoreError> {
        Ok(Ok(()))
    }

    async fn apply(
        &self,
        txn: &mut TikvTxn,
        context: &Context<'_>,
        edit: &Edit,
    ) -> Result<Outcome, StoreError> {
        let Scope::Project(project) = context.scope else {
            return Ok(Err("projects only".to_owned()));
        };
        let piece = Id::parse(edit.args["piece"].as_str().unwrap_or("")).unwrap();
        let place = Place::Piece {
            project: project.clone(),
            document: edit.target.clone(),
            piece: piece.clone(),
        };
        let before: Option<String> = read_row(txn, &place.key()).await?;
        let record = serde_json::to_vec(&edit.args).unwrap();
        let hash = sha256_hex(&record);
        let id = Id::parse(&hash).unwrap();
        txn.put(
            Place::Record {
                project: project.clone(),
                hash: id.clone(),
            }
            .key(),
            record,
        )
        .await?;
        txn.put(place.key(), to_row(&hash)?).await?;
        let change = Change {
            piece,
            before: before.map(|hash| Id::parse(&hash).unwrap()),
            after: Some(id),
        };
        Ok(Ok(Applied {
            changes: vec![change],
            ..Applied::default()
        }))
    }
}

async fn tikv() -> Tikv {
    let pd = std::env::var("STACK_PD").unwrap_or_else(|_| "127.0.0.1:2379".to_owned());
    let endpoints: Vec<String> = pd.split(',').map(str::to_owned).collect();
    Tikv::connect(&endpoints)
        .await
        .expect("TiKV should be running")
}

fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

type Server = Orderers<Tikv, Rewrite>;

fn server(store: &Tikv) -> Arc<Server> {
    Arc::new(Orderers::new(
        store.clone(),
        Arc::new(Rewrite),
        Arc::new(SystemClock),
        30,
    ))
}

/// One person's edits, as fast as each is confirmed.
async fn writer(server: Arc<Server>, scope: Scope, name: String, edits: u64) -> Vec<u64> {
    let mut times = Vec::new();
    let id = Id::parse(&name).unwrap();
    for counter in 1..=edits {
        let edit = Edit {
            person: id.clone(),
            client: id.clone(),
            counter,
            base: 0,
            target: Id::parse("document").unwrap(),
            cmd: "rewrite".to_owned(),
            args: json!({ "piece": name, "n": counter }),
            author_ms: 0,
        };
        let sent = Instant::now();
        loop {
            match server.run(&scope, edit.clone()).await {
                Ok(Reply::Ack { .. }) => break,
                Ok(Reply::Rejected { reason, .. }) => panic!("{reason}"),
                Ok(Reply::Retry { .. }) | Err(_) => {
                    tokio::time::sleep(Duration::from_millis(50)).await
                }
            }
        }
        times.push(sent.elapsed().as_millis() as u64);
    }
    times
}

struct Run {
    label: &'static str,
    servers: usize,
    projects: usize,
    writers: usize,
    edits: u64,
}

async fn run(store: &Tikv, run: Run) {
    let servers: Vec<_> = (0..run.servers).map(|_| server(store)).collect();
    let tag = stamp();
    let scopes: Vec<Scope> = (0..run.projects)
        .map(|n| Scope::Project(Id::parse(&format!("load-{tag}-{n}")).unwrap()))
        .collect();
    let started = Instant::now();
    let mut tasks = Vec::new();
    for n in 0..run.writers {
        let server = servers[n % servers.len()].clone();
        let scope = scopes[n % scopes.len()].clone();
        tasks.push(tokio::spawn(writer(
            server,
            scope,
            format!("writer-{n}"),
            run.edits,
        )));
    }
    let mut times = Vec::new();
    for task in tasks {
        times.extend(task.await.unwrap());
    }
    let seconds = started.elapsed().as_secs_f64();
    times.sort_unstable();
    let at = |share: f64| times[((times.len() - 1) as f64 * share) as usize];
    println!(
        "{}: {} edits/s; confirmed in median {} ms, 99% {} ms, worst {} ms",
        run.label,
        (times.len() as f64 / seconds).round(),
        at(0.5),
        at(0.99),
        times.last().unwrap(),
    );

    let expected = run.writers as u64 * run.edits / run.projects as u64;
    let mut snapshot = store.snapshot().await.unwrap();
    for scope in &scopes {
        let check = kit_sync::verify_log(&mut snapshot, scope).await.unwrap();
        assert_eq!(check.entries, expected, "{}: every edit once", run.label);
    }
    let _ = snapshot.get(&scopes[0].head()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a running TiKV"]
async fn load() {
    let store = tikv().await;
    let edits: u64 = std::env::var("LOAD_EDITS")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(200);
    run(
        &store,
        Run {
            label: "1 project, 8 writers, 1 server",
            servers: 1,
            projects: 1,
            writers: 8,
            edits,
        },
    )
    .await;
    run(
        &store,
        Run {
            label: "1 project, 8 writers, 2 servers",
            servers: 2,
            projects: 1,
            writers: 8,
            edits,
        },
    )
    .await;
    run(
        &store,
        Run {
            label: "8 projects, 8 writers, 1 server",
            servers: 1,
            projects: 8,
            writers: 8,
            edits,
        },
    )
    .await;
    run(
        &store,
        Run {
            label: "32 projects, 64 writers, 2 servers",
            servers: 2,
            projects: 32,
            writers: 64,
            edits: edits / 4,
        },
    )
    .await;
}
