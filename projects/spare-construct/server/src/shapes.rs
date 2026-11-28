//! Keeps a project's shapes up to date while it is in use, and tells everyone what they came to.
//! The orderer has already committed each edit's inputs by the time this runs, so nobody waits on
//! a rebuild (note A05). Any server may run this for any project: the regenerator's cursor lets one
//! pass win, and each checkpoint covers only what that server's own pass built.
//!
//! A checkpoint names its bodies with one point of the log, `through`, and each must be what the
//! log builds there. Bodies wait up to [`CHECKPOINT_EVERY`] to go out together, so they are taken
//! only from passes that each began where the last ended ([`take`]). Another pass may regenerate
//! part of the log in between: a read that catches up for itself (a view's plan or dimensions,
//! which every page asks for after each entry) or another server. It may build one of the waiting
//! elements again, an element changed twice within the second, so what waits goes out at once,
//! through the point it was built at. A body that pass built is not compared at all; one named
//! with a later point would be compared with what a browser built from the later records, and
//! differ.
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

/// Adds what one of this server's passes built to what waits to go out, and hands back what must
/// go out first. What waits holds at `waiting.through`. A pass that began anywhere else follows
/// regeneration another pass did, which may have built some of the same elements again: what
/// waited then goes out as it is, through the point it was built at, and waiting starts again
/// from the new pass.
fn take(waiting: &mut Pass, pass: Pass) -> Option<Pass> {
    let after_another = pass.from != Some(waiting.through);
    let due = (after_another && !waiting.bodies.is_empty()).then(|| Pass {
        through: waiting.through,
        bodies: std::mem::take(&mut waiting.bodies),
        ..Pass::default()
    });
    waiting.through = pass.through;
    for (document, bodies) in pass.bodies {
        waiting.bodies.entry(document).or_default().extend(bodies);
    }
    due
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
            let counter = self
                .counter
                .checked_add(1)
                .ok_or_else(|| StoreError::Damaged("the checkpoint counter is full".to_owned()))?;
            let edit = Edit {
                person,
                client: self.client.clone(),
                counter,
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
            self.counter = counter;
        }
        Ok(())
    }

    /// Rebuilds what has changed, and sends a checkpoint when one is due.
    async fn turn(&mut self, waiting: &mut Pass, sent: &mut Instant) -> Result<(), StoreError> {
        loop {
            let pass = self.regen.catch_up_alone(&self.project).await?;
            let overtaken = pass.overtaken;
            if let Some(due) = take(waiting, pass) {
                self.checkpoint(&due).await?;
                *sent = Instant::now();
            }
            if !overtaken {
                break;
            }
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

#[cfg(test)]
mod tests {
    use super::{Pass, take};
    use kit_core::Id;
    use std::collections::BTreeMap;

    fn id(text: &str) -> Id {
        Id::parse(text).unwrap()
    }

    /// A pass that began at `from` and built `element` to `body` through `through`.
    fn pass(from: u64, through: u64, built: &[(&str, &str)]) -> Pass {
        let mut bodies = BTreeMap::new();
        for (element, body) in built {
            bodies.insert(id(element), id(body));
        }
        let mut documents = BTreeMap::new();
        if !bodies.is_empty() {
            documents.insert(id("doc"), bodies);
        }
        Pass {
            from: Some(from),
            through,
            bodies: documents,
            ..Pass::default()
        }
    }

    /// Passes that each begin where the last ended add up: a later body of an element stands.
    #[test]
    fn passes_one_after_another_wait_together() {
        let mut waiting = pass(4, 10, &[("desk", "retyped"), ("basin", "hung")]);
        assert_eq!(
            take(&mut waiting, pass(10, 11, &[("desk", "turned")])),
            None
        );
        assert_eq!(
            waiting,
            pass(4, 11, &[("desk", "turned"), ("basin", "hung")])
        );
    }

    /// Another pass regenerated entries 11 to 12, the desk's turn among them, and this one found
    /// nothing left: what waited goes out through 10, where it was built, never through 12 with
    /// the desk as it was before its turn.
    #[test]
    fn what_waits_goes_out_where_it_was_built_when_another_pass_came_between() {
        let mut waiting = pass(4, 10, &[("desk", "retyped"), ("basin", "hung")]);
        let due = take(&mut waiting, pass(12, 12, &[])).unwrap();
        assert_eq!(due.through, 10);
        assert_eq!(
            due.bodies,
            pass(4, 10, &[("desk", "retyped"), ("basin", "hung")]).bodies
        );
        assert_eq!(waiting.through, 12);
        assert!(waiting.bodies.is_empty());
    }

    /// The same when this server's own pass built more after the other's: those wait on their own.
    #[test]
    fn a_pass_after_another_waits_on_its_own() {
        let mut waiting = pass(4, 10, &[("desk", "retyped")]);
        let due = take(&mut waiting, pass(12, 13, &[("basin", "moved")])).unwrap();
        assert_eq!(
            (due.through, due.bodies),
            (10, pass(0, 0, &[("desk", "retyped")]).bodies)
        );
        assert_eq!(
            (waiting.through, waiting.bodies),
            (13, pass(0, 0, &[("basin", "moved")]).bodies)
        );
    }

    /// Nothing waiting, nothing goes out, whoever regenerated in between.
    #[test]
    fn nothing_waiting_sends_nothing() {
        let mut waiting = Pass::default();
        assert_eq!(take(&mut waiting, pass(7, 9, &[("desk", "placed")])), None);
        assert_eq!(waiting.through, 9);
        assert_eq!(waiting.bodies, pass(0, 0, &[("desk", "placed")]).bodies);
    }
}
