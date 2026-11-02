//! Keeps a project's shapes up to date while it is in use, and tells everyone what they came to.
//! The orderer has already committed each edit's inputs by the time this runs, so nobody waits on
//! a rebuild (note A05). Any server may run this for any project: the regenerator's cursor lets one
//! pass win, and each checkpoint covers only what that server's own pass built.
//!
//! A checkpoint lost in a crash loses nothing but one comparison: the shapes it would have named
//! are in the store, and rebuilding from the log gives the same bytes.

use construct_model::rows::Checkpoint;
use construct_regen::regenerator::{Pass, Regenerator};
use kit_accounts::SYSTEM;
use kit_core::Id;
use kit_ops::Edit;
use kit_store::{FileStore, Kv, Scope, StoreError};
use kit_sync::{Clock, Commands, Orderers, Reply};
use rand::RngCore;
use serde_json::to_value;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::error::RecvError;

/// How often a checkpoint goes out, and how many bodies make one go out sooner.
const CHECKPOINT_EVERY: Duration = Duration::from_secs(1);
const CHECKPOINT_BODIES: usize = 256;
/// How long to wait after the store fails.
const BACK_OFF: Duration = Duration::from_secs(1);

/// A new client, for what the server itself writes to a log.
#[must_use]
pub fn fresh_client(now_ms: u64) -> Id {
    let mut random = [0u8; 10];
    rand::rng().fill_bytes(&mut random);
    Id::from_time_and_random(now_ms, random)
}

struct Keeper<K: Kv, F: FileStore, C: Commands<K>> {
    regen: Arc<Regenerator<K, F>>,
    orderers: Arc<Orderers<K, C>>,
    clock: Arc<dyn Clock>,
    project: Id,
    client: Id,
    counter: u64,
}

impl<K: Kv, F: FileStore, C: Commands<K>> Keeper<K, F, C> {
    async fn checkpoint(&mut self, pass: &Pass) -> Result<(), StoreError> {
        let scope = Scope::Project(self.project.clone());
        for (document, bodies) in &pass.bodies {
            let bodies: BTreeMap<String, String> = bodies
                .iter()
                .map(|(element, body)| (element.to_string(), body.to_string()))
                .collect();
            if bodies.is_empty() {
                continue;
            }
            let args = Checkpoint {
                through: pass.through,
                bodies,
            };
            let args = to_value(args).map_err(|e| StoreError::Damaged(e.to_string()))?;
            let person = Id::parse(SYSTEM).map_err(|e| StoreError::Damaged(e.to_string()))?;
            let now = self.clock.now_ms();
            let edit = Edit {
                person,
                client: self.client.clone(),
                counter: self.counter + 1,
                base: 0,
                target: document.clone(),
                cmd: "document.checkpoint".to_owned(),
                args,
                author_ms: now,
            };
            loop {
                match self.orderers.run(&scope, edit.clone()).await? {
                    Reply::Retry { .. } => {}
                    Reply::Ack { .. } => break,
                    Reply::Rejected { reason, .. } => {
                        tracing::warn!("checkpoint of {document}: {reason}");
                        break;
                    }
                }
            }
            self.counter += 1;
        }
        Ok(())
    }

    /// Rebuilds what has changed, and sends a checkpoint when one is due.
    async fn turn(&mut self, waiting: &mut Pass, sent: &mut Instant) -> Result<(), StoreError> {
        let pass = self.regen.catch_up(&self.project).await?;
        waiting.through = pass.through;
        for (document, bodies) in pass.bodies {
            waiting.bodies.entry(document).or_default().extend(bodies);
        }
        let count: usize = waiting.bodies.values().map(BTreeMap::len).sum();
        if count > 0 && (count >= CHECKPOINT_BODIES || sent.elapsed() >= CHECKPOINT_EVERY) {
            self.checkpoint(waiting).await?;
            waiting.bodies.clear();
            *sent = Instant::now();
        }
        Ok(())
    }

    async fn run(mut self) {
        let orderer = self
            .orderers
            .get(&Scope::Project(self.project.clone()))
            .await;
        let mut bus = orderer.subscribe();
        let mut waiting = Pass::default();
        let mut sent = Instant::now();
        loop {
            if let Err(e) = self.turn(&mut waiting, &mut sent).await {
                tracing::warn!("regenerating {}: {e}", self.project);
                tokio::time::sleep(BACK_OFF).await;
                continue;
            }
            let due = !waiting.bodies.is_empty();
            tokio::select! {
                entry = bus.recv() => {
                    if matches!(entry, Err(RecvError::Closed)) {
                        return;
                    }
                }
                () = tokio::time::sleep(CHECKPOINT_EVERY), if due => {}
            }
        }
    }
}

/// Starts keeping a project's shapes up to date, for as long as its orderer runs.
pub fn keep_up<K: Kv, F: FileStore, C: Commands<K>>(
    regen: Arc<Regenerator<K, F>>,
    orderers: Arc<Orderers<K, C>>,
    clock: Arc<dyn Clock>,
    project: Id,
) {
    let client = fresh_client(clock.now_ms());
    let keeper = Keeper {
        regen,
        orderers,
        clock,
        project,
        client,
        counter: 0,
    };
    tokio::spawn(keeper.run());
}
