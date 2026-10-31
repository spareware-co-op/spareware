//! The edit log. Each scope has one orderer task on this server: it takes edits a batch at a time,
//! asks whether each is allowed, runs it, and commits log entries and the pieces they change in
//! one transaction.
//!
//! The log's order is decided by the `head` key inside that transaction, not by this process. Run
//! three servers and each has its own task for a project; the store lets one commit win each
//! number and the others run again. Each task also watches `head`, so edits ordered elsewhere
//! still reach its browsers. Nothing changes but the settings.
//!
//! Lessons, each with a test in `tests/failures.rs`:
//!
//! - A commit that reports failure may have landed. So an edit is never assumed lost: the client
//!   counter in the same transaction makes a retry harmless.
//! - A client's edits must land in counter order, with no gaps. If edit 6 fails to commit and edit
//!   7 goes through, a later resend of 6 would look like a repeat and vanish. So an edit that
//!   arrives ahead of its turn is refused unconsumed, and sent again.
//! - An edit that can't be committed is never written as failed. Only a command's own answer
//!   ("Anna is working on this") goes in the log. A store outage leaves the edit with its client.
//! - Conflicts between servers are normal here, not an error. Long back-offs made two busy servers
//!   crawl, 117 edits a second with stalls near two seconds, so a conflict is retried at once with
//!   a small random wait, and edits that arrived meanwhile join the retry.

use crate::replay::read_log;
use kit_core::Id;
use kit_ops::{Change, ClaimChange, Edit, Entry};
use kit_store::{Key, Kv, Reader, Scope, StoreError, Txn, read_row, to_row};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::hash::{BuildHasher, Hasher, RandomState};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio::time::{Instant, MissedTickBehavior, interval, sleep};

/// Edits taken into one transaction.
const BATCH: usize = 64;
/// How long an edit is tried before it goes back to its client.
const GIVE_UP: Duration = Duration::from_secs(5);
/// The longest wait between tries when the store is failing.
const LONGEST_WAIT_MS: u64 = 500;
/// The longest wait after a conflict.
const CONFLICT_WAIT_MS: u64 = 8;
const INBOX: usize = 4096;

/// The time a command may read. A test can stop it.
pub trait Clock: Send + Sync + 'static {
    fn now_ms(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |t| t.as_millis() as u64)
    }
}

/// What a command can see of the moment it runs in.
pub struct Context<'a> {
    pub scope: &'a Scope,
    /// The orderer's time, recorded in the entry. A command reads no other clock, so replaying
    /// a log gives the same result on any machine.
    pub now_ms: u64,
    /// The log number before this edit.
    pub head: u64,
}

/// An entry to add to another scope's log in the same transaction, such as a package arriving
/// over a bridge.
#[derive(Clone, Debug, PartialEq)]
pub struct Elsewhere {
    pub scope: Scope,
    pub target: Id,
    pub cmd: String,
    pub args: Value,
}

/// What a command did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Applied {
    pub changes: Vec<Change>,
    pub claims: Vec<ClaimChange>,
    pub elsewhere: Vec<Elsewhere>,
}

/// A command's answer: what it did, or why it was turned back. Both are results and go in the log.
pub type Outcome = Result<Applied, String>;

/// A program's commands. The orderer runs these and knows nothing of what they mean.
pub trait Commands<K: Kv>: Send + Sync + 'static {
    /// The version of the commands, recorded in each entry.
    fn version(&self) -> u32;

    /// Whether this edit may run. `Err` in the answer is the reason it may not.
    fn permit(
        &self,
        scope: &Scope,
        edit: &Edit,
    ) -> impl Future<Output = Result<Result<(), String>, StoreError>> + Send;

    /// Edits with the same answer to `permit`, so a batch asks once. Defaults to each edit alone.
    fn permit_group(&self, edit: &Edit) -> String {
        format!("{}/{}/{}", edit.person, edit.cmd, edit.args)
    }

    /// Runs the command inside the transaction.
    fn apply(
        &self,
        txn: &mut K::Txn,
        context: &Context<'_>,
        edit: &Edit,
    ) -> impl Future<Output = Result<Outcome, StoreError>> + Send;
}

/// An edit and where its answer goes.
pub struct Submit {
    pub edit: Edit,
    pub reply: mpsc::UnboundedSender<Reply>,
}

/// What became of an edit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    /// In the log, safely on disk.
    Ack { counter: u64, seq: u64 },
    /// In the log as turned back, with the reason.
    Rejected { counter: u64, reason: String },
    /// Not consumed: send it again.
    Retry { counter: u64 },
}

/// The last edit taken from one client, in the same transaction as the edit.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientMark {
    pub counter: u64,
    pub seq: u64,
}

/// All the orderers on this server, started as they're needed.
pub struct Orderers<K: Kv, C: Commands<K>> {
    kv: K,
    commands: Arc<C>,
    clock: Arc<dyn Clock>,
    tail_ms: u64,
    running: Mutex<HashMap<Scope, Arc<Orderer>>>,
}

/// One scope's orderer.
pub struct Orderer {
    inbox: mpsc::Sender<Submit>,
    bus: broadcast::Sender<Arc<Entry>>,
}

impl Orderer {
    /// Hands an edit in. The answer comes on the reply channel.
    pub async fn submit(&self, submit: Submit) -> Result<(), StoreError> {
        self.inbox
            .send(submit)
            .await
            .map_err(|_| StoreError::Unavailable("the orderer has stopped".to_owned()))
    }

    /// Every entry from now on.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Entry>> {
        self.bus.subscribe()
    }
}

impl<K: Kv, C: Commands<K>> Orderers<K, C> {
    pub fn new(kv: K, commands: Arc<C>, clock: Arc<dyn Clock>, tail_ms: u64) -> Self {
        Self { kv, commands, clock, tail_ms, running: Mutex::new(HashMap::new()) }
    }

    pub async fn get(&self, scope: &Scope) -> Arc<Orderer> {
        let mut running = self.running.lock().await;
        if let Some(orderer) = running.get(scope) {
            return orderer.clone();
        }
        let (inbox, mail) = mpsc::channel(INBOX);
        let (bus, _) = broadcast::channel(INBOX);
        let orderer = Arc::new(Orderer { inbox, bus: bus.clone() });
        running.insert(scope.clone(), orderer.clone());
        let task = Task {
            kv: self.kv.clone(),
            commands: self.commands.clone(),
            clock: self.clock.clone(),
            scope: scope.clone(),
            bus,
            seen: 0,
        };
        tokio::spawn(task.run(mail, self.tail_ms));
        orderer
    }

    /// Runs one edit from the server itself and waits for its answer.
    pub async fn run(&self, scope: &Scope, edit: Edit) -> Result<Reply, StoreError> {
        let orderer = self.get(scope).await;
        let (reply, mut replies) = mpsc::unbounded_channel();
        orderer.submit(Submit { edit, reply }).await?;
        replies
            .recv()
            .await
            .ok_or_else(|| StoreError::Unavailable("the orderer has stopped".to_owned()))
    }
}

struct Task<K: Kv, C: Commands<K>> {
    kv: K,
    commands: Arc<C>,
    clock: Arc<dyn Clock>,
    scope: Scope,
    bus: broadcast::Sender<Arc<Entry>>,
    /// The last log number passed to this server's browsers.
    seen: u64,
}

/// A batch ready to commit.
struct Built {
    head_before: u64,
    head_after: u64,
    entries: Vec<Arc<Entry>>,
    replies: Vec<(usize, Reply)>,
}

async fn read_head<R: Reader>(reader: &mut R, scope: &Scope) -> Result<u64, StoreError> {
    Ok(read_row::<u64, R>(reader, &scope.head()).await?.unwrap_or(0))
}

/// A short random wait, so two servers stop colliding on their next try.
fn jitter(most_ms: u64) -> Duration {
    let random = RandomState::new().build_hasher().finish();
    Duration::from_millis(random % (most_ms + 1))
}

impl<K: Kv, C: Commands<K>> Task<K, C> {
    async fn run(mut self, mut mail: mpsc::Receiver<Submit>, tail_ms: u64) {
        loop {
            match self.head().await {
                Ok(head) => {
                    self.seen = head;
                    break;
                }
                Err(_) => sleep(Duration::from_millis(LONGEST_WAIT_MS)).await,
            }
        }
        let mut tick = interval(Duration::from_millis(tail_ms.max(5)));
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                first = mail.recv() => {
                    let Some(first) = first else { return };
                    let mut batch = vec![first];
                    collect(&mut mail, &mut batch);
                    self.commit(&mut mail, batch).await;
                }
                _ = tick.tick(), if self.bus.receiver_count() > 0 => {
                    // A failed look is simply tried at the next tick.
                    let _ = self.tail().await;
                }
            }
        }
    }

    async fn head(&self) -> Result<u64, StoreError> {
        let mut snapshot = self.kv.snapshot().await?;
        read_head(&mut snapshot, &self.scope).await
    }

    /// Passes on entries that reached the log by another server.
    async fn tail(&mut self) -> Result<(), StoreError> {
        let mut snapshot = self.kv.snapshot().await?;
        let head = read_head(&mut snapshot, &self.scope).await?;
        if head > self.seen {
            for entry in read_log(&mut snapshot, &self.scope, self.seen, head).await? {
                self.seen = entry.seq;
                let _ = self.bus.send(Arc::new(entry));
            }
        }
        Ok(())
    }

    async fn commit(&mut self, mail: &mut mpsc::Receiver<Submit>, mut batch: Vec<Submit>) {
        let started = Instant::now();
        let mut allowed = HashMap::new();
        let mut failures = 0u32;
        while started.elapsed() < GIVE_UP {
            match self.try_commit(&batch, &mut allowed).await {
                Ok(built) => {
                    if built.head_before == self.seen {
                        self.seen = built.head_after;
                        for entry in built.entries {
                            let _ = self.bus.send(entry);
                        }
                    } else {
                        // Others got in first: the log itself has the whole order. If this look
                        // fails, the next tick makes it.
                        let _ = self.tail().await;
                    }
                    for (index, reply) in built.replies {
                        if let Some(submit) = batch.get(index) {
                            let _ = submit.reply.send(reply);
                        }
                    }
                    return;
                }
                Err(StoreError::Conflict) => sleep(jitter(CONFLICT_WAIT_MS)).await,
                Err(_) => {
                    failures += 1;
                    let wait = (5u64 << failures.min(7)).min(LONGEST_WAIT_MS);
                    sleep(Duration::from_millis(wait) + jitter(wait)).await;
                }
            }
            collect(mail, &mut batch);
        }
        // Nothing was consumed, so the client keeps each edit and sends it again.
        for submit in &batch {
            let _ = submit.reply.send(Reply::Retry { counter: submit.edit.counter });
        }
    }

    async fn try_commit(
        &self,
        batch: &[Submit],
        allowed: &mut HashMap<String, Result<(), String>>,
    ) -> Result<Built, StoreError> {
        let mut txn = self.kv.begin().await?;
        match self.build(&mut txn, batch, allowed).await {
            Ok(built) => {
                txn.commit().await?;
                Ok(built)
            }
            Err(e) => {
                let _ = txn.rollback().await;
                Err(e)
            }
        }
    }

    async fn build(
        &self,
        txn: &mut K::Txn,
        batch: &[Submit],
        allowed: &mut HashMap<String, Result<(), String>>,
    ) -> Result<Built, StoreError> {
        let head_before = read_head(txn, &self.scope).await?;
        let now_ms = self.clock.now_ms();
        let mut head = head_before;
        let mut entries = Vec::new();
        let mut replies = Vec::new();
        for (index, submit) in batch.iter().enumerate() {
            let edit = &submit.edit;
            let mark_key = self.scope.client(&edit.person, &edit.client);
            let mark: Option<ClientMark> = read_row(txn, &mark_key).await?;
            let last = mark.map_or(0, |m| m.counter);
            if edit.counter <= last {
                // A resend of something already in the log: confirm it, change nothing.
                let seq = mark.map_or(0, |m| m.seq);
                replies.push((index, Reply::Ack { counter: edit.counter, seq }));
                continue;
            }
            if edit.counter != last + 1 {
                replies.push((index, Reply::Retry { counter: edit.counter }));
                continue;
            }

            let group = self.commands.permit_group(edit);
            let permitted = match allowed.get(&group) {
                Some(answer) => answer.clone(),
                None => {
                    let answer = self.commands.permit(&self.scope, edit).await?;
                    allowed.insert(group, answer.clone());
                    answer
                }
            };
            let context = Context { scope: &self.scope, now_ms, head };
            let outcome = match permitted {
                Ok(()) => self.commands.apply(txn, &context, edit).await?,
                Err(reason) => Err(reason),
            };
            let (applied, failed) = match outcome {
                Ok(applied) => (applied, None),
                Err(reason) => (Applied::default(), Some(reason)),
            };

            head += 1;
            let version = self.commands.version();
            let entry =
                Entry::new(head, edit, version, now_ms, applied.changes, applied.claims, failed);
            for other in applied.elsewhere {
                self.write_elsewhere(txn, edit, &entry, other).await?;
            }
            txn.put(self.scope.log(head), entry.encode()).await?;
            txn.put(mark_key, to_row(&ClientMark { counter: edit.counter, seq: head })?).await?;
            let reply = match &entry.failed {
                None => Reply::Ack { counter: edit.counter, seq: head },
                Some(reason) => Reply::Rejected { counter: edit.counter, reason: reason.clone() },
            };
            replies.push((index, reply));
            entries.push(Arc::new(entry));
        }
        if head != head_before {
            txn.put(self.scope.head(), to_row(&head)?).await?;
        }
        Ok(Built { head_before, head_after: head, entries, replies })
    }

    /// Adds an entry to another scope's log. Its own orderer finds it by watching `head`.
    async fn write_elsewhere(
        &self,
        txn: &mut K::Txn,
        edit: &Edit,
        entry: &Entry,
        other: Elsewhere,
    ) -> Result<(), StoreError> {
        let seq = read_head(txn, &other.scope).await? + 1;
        let arriving = Edit { target: other.target, cmd: other.cmd, args: other.args, ..edit.clone() };
        let version = self.commands.version();
        let written = Entry::new(seq, &arriving, version, entry.orderer_ms, vec![], vec![], None);
        txn.put(other.scope.log(seq), written.encode()).await?;
        let head: Key = other.scope.head();
        txn.put(head, to_row(&seq)?).await
    }
}

/// Takes whatever else is already waiting, up to a full batch.
fn collect(mail: &mut mpsc::Receiver<Submit>, batch: &mut Vec<Submit>) {
    while batch.len() < BATCH {
        match mail.try_recv() {
            Ok(next) => batch.push(next),
            Err(_) => return,
        }
    }
}
