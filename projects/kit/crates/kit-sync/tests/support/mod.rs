//! A small set of commands for testing the orderer: one "note" element per id, claims on them, and
//! a list of who is refused. Only what the tests need.
#![allow(dead_code, reason = "each test file uses part of this")]

use kit_core::Id;
use kit_ops::{Change, Claim, Edit, sha256_hex};
use kit_store::memory::Memory;
use kit_store::{Kv, Place, Reader, Scope, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Clock, Commands, Context, Elsewhere, Orderers, Outcome, Reply, claims};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub const CLAIM_MS: u64 = 1000;

pub fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

pub fn project() -> Scope {
    Scope::Project(i("project"))
}

/// A clock a test moves by hand.
#[derive(Default)]
pub struct TestClock(pub AtomicU64);

impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
pub struct Notes {
    pub refused: Mutex<HashSet<String>>,
}

impl Commands<Memory> for Notes {
    fn version(&self) -> u32 {
        1
    }

    async fn permit(&self, _scope: &Scope, edit: &Edit) -> Result<Result<(), String>, StoreError> {
        let refused = self.refused.lock().unwrap().contains(edit.person.as_str());
        Ok(if refused { Err("You don't have access to this.".to_owned()) } else { Ok(()) })
    }

    async fn apply(
        &self,
        txn: &mut <Memory as Kv>::Txn,
        context: &Context<'_>,
        edit: &Edit,
    ) -> Result<Outcome, StoreError> {
        let Scope::Project(project) = context.scope else {
            return Ok(Err("Notes live in projects.".to_owned()));
        };
        let piece = Id::parse(edit.args["note"].as_str().unwrap_or("")).unwrap();
        if let Some(claim) = claims::held_by_another(
            txn,
            project,
            &edit.target,
            &piece,
            &edit.person,
            context.now_ms,
        )
        .await?
        {
            return Ok(Err(format!("{} is working on this.", claim.name)));
        }
        let place = Place::Element {
            project: project.clone(),
            model: edit.target.clone(),
            element: piece.clone(),
        };
        let before: Option<String> = read_row(txn, &place.key()).await?;
        let record = serde_json::to_vec(&json!({ "note": piece.as_str(), "text": edit.args["text"] }))
            .unwrap();
        let hash = sha256_hex(&record);
        let record_place =
            Place::Record { project: project.clone(), hash: Id::parse(&hash).unwrap() };
        txn.put(record_place.key(), record).await?;
        txn.put(place.key(), to_row(&hash)?).await?;
        let claim = Claim {
            person: edit.person.clone(),
            name: edit.person.to_string(),
            until_ms: context.now_ms + CLAIM_MS,
        };
        let claim = claims::take(txn, project, &edit.target, &piece, claim).await?;
        let change = Change {
            piece,
            before: before.map(|hash| Id::parse(&hash).unwrap()),
            after: Some(Id::parse(&hash).unwrap()),
        };
        let elsewhere = match edit.args["copy_to"].as_str() {
            Some(other) => vec![Elsewhere {
                scope: Scope::Project(i(other)),
                target: edit.target.clone(),
                cmd: "note.arrived".to_owned(),
                args: edit.args.clone(),
            }],
            None => Vec::new(),
        };
        Ok(Ok(Applied { changes: vec![change], claims: vec![claim], elsewhere }))
    }
}

pub type Server = Orderers<Memory, Notes>;

pub fn server(store: &Memory, notes: &Arc<Notes>, clock: &Arc<TestClock>) -> Server {
    Orderers::new(store.clone(), notes.clone(), clock.clone(), 5)
}

pub fn edit(person: &str, client: &str, counter: u64, note: &str, text: &str) -> Edit {
    Edit {
        person: i(person),
        client: i(client),
        counter,
        base: 0,
        target: i("model"),
        cmd: "note.set".to_owned(),
        args: json!({ "note": note, "text": text }),
        author_ms: 0,
    }
}

/// Runs an edit, sending it again for as long as the orderer says to.
pub async fn run_until_answered(server: &Server, scope: &Scope, edit: Edit) -> Reply {
    loop {
        match server.run(scope, edit.clone()).await {
            Ok(Reply::Retry { .. }) | Err(_) => tokio::task::yield_now().await,
            Ok(reply) => return reply,
        }
    }
}

pub async fn head(store: &Memory, scope: &Scope) -> u64 {
    let mut snapshot = store.snapshot().await.unwrap();
    read_row::<u64, _>(&mut snapshot, &scope.head()).await.unwrap().unwrap_or(0)
}

pub async fn log(store: &Memory, scope: &Scope) -> Vec<kit_ops::Entry> {
    let mut snapshot = store.snapshot().await.unwrap();
    let to = head(store, scope).await;
    kit_sync::read_log(&mut snapshot, scope, 0, to).await.unwrap()
}

pub async fn verify(store: &Memory, scope: &Scope) -> Result<kit_sync::LogCheck, String> {
    let mut snapshot = store.snapshot().await.unwrap();
    kit_sync::verify_log(&mut snapshot, scope).await
}

pub fn value_text(value: &Value) -> String {
    value.as_str().unwrap_or("").to_owned()
}

pub async fn stored_text(store: &Memory, note: &str) -> Option<String> {
    let mut snapshot = store.snapshot().await.unwrap();
    let place =
        Place::Element { project: i("project"), model: i("model"), element: i(note) };
    let hash: String = read_row(&mut snapshot, &place.key()).await.unwrap()?;
    let record_place = Place::Record { project: i("project"), hash: i(&hash) };
    let bytes = snapshot.get(&record_place.key()).await.unwrap()?;
    let record: Value = serde_json::from_slice(&bytes).unwrap();
    Some(value_text(&record["text"]))
}
