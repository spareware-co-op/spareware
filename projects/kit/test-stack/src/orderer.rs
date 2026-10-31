//! The edit log. Each project has one orderer task on this server: it takes edits one batch at a
//! time, checks access and claims, and commits log entries and the model's pieces in one TiKV
//! transaction. This is note A05's server-ordered log, the way Replicache and Figma do it:
//! commands go up, changes come down.
//!
//! The log's order is decided by the `head` key inside that transaction, not by this process.
//! Run three servers and each has its own orderer task for a project; TiKV lets one commit win
//! each number and the others retry. Each task also watches `head`, so edits ordered elsewhere
//! still reach its browsers. Nothing changes but the settings.

use crate::access::{Access, Target};
use crate::keys;
use crate::kv::{self, Kv};
use crate::rows::{
    Change, Claim, ClaimChange, ClientMark, Element, Entry, Person, Shape, cells, sha256_hex,
};
use anyhow::{Context as _, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tikv_client::Transaction;
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio::time::{MissedTickBehavior, interval, sleep};

const BATCH: usize = 64;
const ATTEMPTS: u32 = 8;
const COMMAND_VERSION: u32 = 1;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |t| t.as_millis() as u64)
}

/// An edit on its way in.
pub struct Submit {
    pub person: String,
    pub client: String,
    pub counter: u64,
    pub base: u64,
    pub model: String,
    pub cmd: String,
    pub args: Value,
    pub author_ms: u64,
    pub reply: mpsc::UnboundedSender<Reply>,
}

/// What became of an edit.
#[derive(Clone, Debug)]
pub enum Reply {
    /// In the log, safely on disk.
    Ack { counter: u64, seq: u64 },
    /// In the log as turned back, with the reason.
    Rejected { counter: u64, reason: String },
    /// Not consumed: send it again.
    Retry { counter: u64 },
}

pub struct Orderers {
    kv: Kv,
    access: Arc<Access>,
    tail_ms: u64,
    claim_ms: u64,
    running: Mutex<HashMap<String, Arc<Orderer>>>,
}

pub struct Orderer {
    submit: mpsc::Sender<Submit>,
    bus: broadcast::Sender<Arc<Entry>>,
}

impl Orderer {
    pub async fn submit(&self, edit: Submit) -> Result<()> {
        self.submit
            .send(edit)
            .await
            .map_err(|_| anyhow!("orderer stopped"))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Entry>> {
        self.bus.subscribe()
    }
}

impl Orderers {
    pub fn new(kv: Kv, access: Arc<Access>, tail_ms: u64, claim_ms: u64) -> Self {
        Self {
            kv,
            access,
            tail_ms,
            claim_ms,
            running: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get(&self, project: &str) -> Arc<Orderer> {
        let mut running = self.running.lock().await;
        if let Some(orderer) = running.get(project) {
            return orderer.clone();
        }
        let (submit, inbox) = mpsc::channel(4096);
        let (bus, _) = broadcast::channel(4096);
        let orderer = Arc::new(Orderer {
            submit,
            bus: bus.clone(),
        });
        running.insert(project.to_owned(), orderer.clone());
        let task = Task {
            kv: self.kv.clone(),
            access: self.access.clone(),
            project: project.to_owned(),
            claim_ms: self.claim_ms,
            bus,
            seen: 0,
        };
        tokio::spawn(task.run(inbox, self.tail_ms));
        orderer
    }
}

struct Task {
    kv: Kv,
    access: Arc<Access>,
    project: String,
    claim_ms: u64,
    bus: broadcast::Sender<Arc<Entry>>,
    /// The last log number passed to this server's browsers.
    seen: u64,
}

/// A batch that committed.
struct Built {
    head_before: u64,
    head_after: u64,
    entries: Vec<Arc<Entry>>,
    replies: Vec<(usize, Reply)>,
}

impl Task {
    async fn run(mut self, mut inbox: mpsc::Receiver<Submit>, tail_ms: u64) {
        loop {
            match self.kv.get::<u64>(keys::head(&self.project)).await {
                Ok(head) => {
                    self.seen = head.unwrap_or(0);
                    break;
                }
                Err(e) => {
                    tracing::warn!("orderer {}: {e:#}", self.project);
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
        let mut tick = interval(Duration::from_millis(tail_ms.max(10)));
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                first = inbox.recv() => {
                    let Some(first) = first else { return };
                    let mut batch = vec![first];
                    while batch.len() < BATCH {
                        match inbox.try_recv() {
                            Ok(next) => batch.push(next),
                            Err(_) => break,
                        }
                    }
                    self.commit(batch).await;
                }
                _ = tick.tick(), if self.bus.receiver_count() > 0 => {
                    if let Err(e) = self.tail().await {
                        tracing::warn!("orderer {}: {e:#}", self.project);
                    }
                }
            }
        }
    }

    /// Passes on entries ordered by another server.
    async fn tail(&mut self) -> Result<()> {
        let head = self
            .kv
            .get::<u64>(keys::head(&self.project))
            .await?
            .unwrap_or(0);
        if head <= self.seen {
            return Ok(());
        }
        let mut snap = self.kv.snapshot().await?;
        for entry in read_log(&mut snap, &self.project, self.seen, head).await? {
            self.seen = entry.seq;
            let _ = self.bus.send(Arc::new(entry));
        }
        Ok(())
    }

    async fn commit(&mut self, batch: Vec<Submit>) {
        let mut allowed = HashMap::new();
        for attempt in 0..ATTEMPTS {
            match self.try_commit(&batch, &mut allowed).await {
                Ok(built) => {
                    if built.head_before != self.seen {
                        if let Err(e) = self.tail().await {
                            tracing::warn!("orderer {}: {e:#}", self.project);
                        }
                    } else {
                        self.seen = built.head_after;
                        for entry in &built.entries {
                            let _ = self.bus.send(entry.clone());
                        }
                    }
                    for (index, reply) in built.replies {
                        let _ = batch[index].reply.send(reply);
                    }
                    return;
                }
                Err(e) => {
                    tracing::debug!("orderer {} attempt {attempt}: {e:#}", self.project);
                    sleep(Duration::from_millis(5 << attempt)).await;
                }
            }
        }
        // Nothing was consumed, so the browser keeps the edit and sends it again.
        tracing::warn!(
            "orderer {}: gave up on a batch of {}",
            self.project,
            batch.len()
        );
        for edit in &batch {
            let _ = edit.reply.send(Reply::Retry {
                counter: edit.counter,
            });
        }
    }

    async fn try_commit(
        &self,
        batch: &[Submit],
        allowed: &mut HashMap<(String, String), bool>,
    ) -> Result<Built> {
        let mut txn = self.kv.begin().await?;
        let result = self.build(&mut txn, batch, allowed).await;
        match result {
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
        txn: &mut Transaction,
        batch: &[Submit],
        allowed: &mut HashMap<(String, String), bool>,
    ) -> Result<Built> {
        let project = &self.project;
        let head_key = keys::head(project);
        let head_before: u64 = read(txn, head_key.clone()).await?.unwrap_or(0);
        let now = now_ms();
        let mut head = head_before;
        let mut entries = Vec::new();
        let mut replies = Vec::new();
        for (index, edit) in batch.iter().enumerate() {
            let mark_key = keys::client(project, &edit.person, &edit.client);
            let mark: Option<ClientMark> = read(txn, mark_key.clone()).await?;
            let last = mark.as_ref().map_or(0, |m| m.counter);
            if edit.counter <= last {
                // A resend of something already in the log: confirm it, change nothing.
                let seq = mark.as_ref().map_or(0, |m| m.seq);
                replies.push((
                    index,
                    Reply::Ack {
                        counter: edit.counter,
                        seq,
                    },
                ));
                continue;
            }
            if edit.counter != last + 1 {
                // An earlier edit from this tab is still to come. Taking this one first would
                // make that one look like a repeat and lose it, so it waits and is sent again.
                replies.push((
                    index,
                    Reply::Retry {
                        counter: edit.counter,
                    },
                ));
                continue;
            }

            let permit = (edit.person.clone(), edit.model.clone());
            let may_edit = match allowed.get(&permit) {
                Some(answer) => *answer,
                None => {
                    let target = Target::Model(project, &edit.model);
                    let answer = self
                        .access
                        .allowed(&edit.person, "model.edit", target, None)
                        .await?;
                    allowed.insert(permit, answer);
                    answer
                }
            };

            let outcome = if may_edit {
                self.apply(txn, edit, now).await?
            } else {
                Err("You don't have access to edit this model.".to_owned())
            };
            let (changes, claims, failed) = match outcome {
                Ok((changes, claims)) => (changes, claims, None),
                Err(reason) => (Vec::new(), Vec::new(), Some(reason)),
            };

            head += 1;
            let entry = Entry {
                seq: head,
                client: edit.client.clone(),
                counter: edit.counter,
                op_id: format!("{}.{}", edit.client, edit.counter),
                author: edit.person.clone(),
                base: edit.base,
                author_ms: edit.author_ms,
                orderer_ms: now,
                model: edit.model.clone(),
                cmd: edit.cmd.clone(),
                version: COMMAND_VERSION,
                args: edit.args.clone(),
                content_id: sha256_hex(&serde_json::to_vec(&changes)?),
                changes,
                claims,
                failed,
            };
            txn.put(keys::log(project, head), serde_json::to_vec(&entry)?)
                .await?;
            let mark = ClientMark {
                counter: edit.counter,
                seq: head,
            };
            txn.put(mark_key, serde_json::to_vec(&mark)?).await?;
            let reply = match &entry.failed {
                None => Reply::Ack {
                    counter: edit.counter,
                    seq: head,
                },
                Some(reason) => Reply::Rejected {
                    counter: edit.counter,
                    reason: reason.clone(),
                },
            };
            replies.push((index, reply));
            entries.push(Arc::new(entry));
        }
        if head != head_before {
            txn.put(head_key, serde_json::to_vec(&head)?).await?;
        }
        Ok(Built {
            head_before,
            head_after: head,
            entries,
            replies,
        })
    }

    /// Runs one command. `Err` in the inner result is a turned-back edit, not a failure here.
    async fn apply(
        &self,
        txn: &mut Transaction,
        edit: &Submit,
        now: u64,
    ) -> Result<Result<(Vec<Change>, Vec<ClaimChange>), String>> {
        let project = &self.project;
        let model = &edit.model;
        let args: Args = match serde_json::from_value(edit.args.clone()) {
            Ok(args) => args,
            Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
        };
        if edit.cmd == "element.create" {
            return create(txn, project, model, &args).await;
        }

        let element = args.element.as_deref().unwrap_or_default();
        let elem_key = keys::elem(project, model, element);
        let claim_key = keys::claim(project, model, element);
        let Some(hash) = read::<String>(txn, elem_key.clone()).await? else {
            return Ok(Err("That element no longer exists.".to_owned()));
        };

        let claim: Option<Claim> = read(txn, claim_key.clone()).await?;
        if let Some(claim) = &claim
            && claim.person != edit.person
            && claim.until_ms > now
        {
            return Ok(Err(format!("{} is working on this.", claim.name)));
        }

        let mut claims = Vec::new();
        let name = read::<Person>(txn, keys::person(&edit.person))
            .await?
            .context("the author has no person row")?
            .name;
        let mine = Claim {
            person: edit.person.clone(),
            name,
            until_ms: now + self.claim_ms,
        };
        match edit.cmd.as_str() {
            "element.claim" => {
                txn.put(claim_key, serde_json::to_vec(&mine)?).await?;
                claims.push(ClaimChange {
                    element: element.to_owned(),
                    claim: Some(mine),
                });
                return Ok(Ok((Vec::new(), claims)));
            }
            "element.release" => {
                txn.delete(claim_key).await?;
                claims.push(ClaimChange {
                    element: element.to_owned(),
                    claim: None,
                });
                return Ok(Ok((Vec::new(), claims)));
            }
            _ => {}
        }

        let before = read_record(txn, project, &hash).await?;
        let shape: Shape = read(txn, keys::shape(project, &before.shape))
            .await?
            .context("element without a shape")?;
        let mut after = before.clone();
        match edit.cmd.as_str() {
            "element.move" => {
                let (Some(x), Some(y)) = (args.x, args.y) else {
                    return Ok(Err("A move needs x and y.".to_owned()));
                };
                after.x = x;
                after.y = y;
            }
            "element.set" => {
                if let Some(name) = &args.name {
                    after.name = name.clone();
                }
                after.props.extend(args.props.clone().unwrap_or_default());
            }
            "element.delete" => {
                for cell in cells(&before, &shape) {
                    txn.delete(keys::area(project, &cell, element)).await?;
                }
                txn.delete(elem_key).await?;
                txn.delete(claim_key).await?;
                claims.push(ClaimChange {
                    element: element.to_owned(),
                    claim: None,
                });
                let change = Change {
                    element: element.to_owned(),
                    before: Some(hash),
                    after: None,
                };
                return Ok(Ok((vec![change], claims)));
            }
            other => return Ok(Err(format!("Unknown command {other}."))),
        }

        let new_hash = write_record(txn, project, &after).await?;
        for cell in cells(&before, &shape) {
            txn.delete(keys::area(project, &cell, element)).await?;
        }
        for cell in cells(&after, &shape) {
            txn.put(
                keys::area(project, &cell, element),
                serde_json::to_vec(&new_hash)?,
            )
            .await?;
        }
        txn.put(elem_key, serde_json::to_vec(&new_hash)?).await?;
        txn.put(claim_key, serde_json::to_vec(&mine)?).await?;
        claims.push(ClaimChange {
            element: element.to_owned(),
            claim: Some(mine),
        });
        let change = Change {
            element: element.to_owned(),
            before: Some(hash),
            after: Some(new_hash),
        };
        Ok(Ok((vec![change], claims)))
    }
}

/// The arguments of every command; each uses some of them.
#[derive(Deserialize, Default)]
struct Args {
    element: Option<String>,
    kind: Option<String>,
    name: Option<String>,
    level: Option<i32>,
    x: Option<i64>,
    y: Option<i64>,
    w: Option<i64>,
    d: Option<i64>,
    h: Option<i64>,
    props: Option<BTreeMap<String, String>>,
}

async fn create(
    txn: &mut Transaction,
    project: &str,
    model: &str,
    args: &Args,
) -> Result<Result<(Vec<Change>, Vec<ClaimChange>), String>> {
    let (Some(id), Some(kind), Some(x), Some(y), Some(w), Some(d), Some(h)) = (
        &args.element,
        &args.kind,
        args.x,
        args.y,
        args.w,
        args.d,
        args.h,
    ) else {
        return Ok(Err(
            "A new element needs an id, a kind, a position and a size.".to_owned(),
        ));
    };
    let plain = |c: char| c.is_ascii_alphanumeric() || "-_.".contains(c);
    if id.is_empty() || !id.chars().all(plain) {
        return Ok(Err("That element id isn't valid.".to_owned()));
    }
    if w <= 0 || d <= 0 || h <= 0 {
        return Ok(Err("A box needs a size above zero.".to_owned()));
    }
    let elem_key = keys::elem(project, model, id);
    if read::<String>(txn, elem_key.clone()).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let shape = Shape { w, d, h };
    let shape_bytes = serde_json::to_vec(&shape)?;
    let shape_hash = sha256_hex(&shape_bytes);
    txn.put(keys::shape(project, &shape_hash), shape_bytes)
        .await?;
    let element = Element {
        id: id.clone(),
        model: model.to_owned(),
        kind: kind.clone(),
        name: args.name.clone().unwrap_or_else(|| id.clone()),
        level: args.level.unwrap_or(0),
        x,
        y,
        shape: shape_hash,
        props: args.props.clone().unwrap_or_default(),
    };
    let hash = write_record(txn, project, &element).await?;
    for cell in cells(&element, &shape) {
        txn.put(keys::area(project, &cell, id), serde_json::to_vec(&hash)?)
            .await?;
    }
    txn.put(elem_key, serde_json::to_vec(&hash)?).await?;
    let change = Change {
        element: id.clone(),
        before: None,
        after: Some(hash),
    };
    Ok(Ok((vec![change], Vec::new())))
}

async fn write_record(txn: &mut Transaction, project: &str, element: &Element) -> Result<String> {
    let bytes = serde_json::to_vec(element)?;
    let hash = sha256_hex(&bytes);
    txn.put(keys::record(project, &hash), bytes).await?;
    Ok(hash)
}

async fn read_record(txn: &mut Transaction, project: &str, hash: &str) -> Result<Element> {
    read(txn, keys::record(project, hash))
        .await?
        .context("element record missing")
}

async fn read<T: serde::de::DeserializeOwned>(
    txn: &mut Transaction,
    key: String,
) -> Result<Option<T>> {
    match txn.get(key).await? {
        Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        None => Ok(None),
    }
}

/// Log entries after `from`, up to and including `to`.
pub async fn read_log(
    snap: &mut tikv_client::Snapshot,
    project: &str,
    from: u64,
    to: u64,
) -> Result<Vec<Entry>> {
    let range = keys::log(project, from + 1).into_bytes()..keys::log(project, to + 1).into_bytes();
    let rows = kv::scan_range(snap, range).await?;
    rows.iter()
        .map(|(_, value)| Ok(serde_json::from_slice(value)?))
        .collect()
}

/// What `verify` found.
#[derive(Debug)]
pub struct LogCheck {
    pub entries: u64,
    pub elements: usize,
}

/// Checks a project's stored pieces against its log: no gaps in the numbers, every content ID
/// right, and replaying the log gives exactly the elements stored. The log is the truth, so if
/// this passes nothing has been lost.
pub async fn verify_project(kv: &Kv, project: &str) -> Result<LogCheck> {
    let mut snap = kv.snapshot().await?;
    let head: u64 = kv::get(&mut snap, keys::head(project)).await?.unwrap_or(0);
    let entries = read_log(&mut snap, project, 0, head).await?;
    if entries.len() as u64 != head {
        bail!("{project}: {} log entries, head says {head}", entries.len());
    }
    let mut replayed: BTreeMap<(String, String), String> = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.seq != index as u64 + 1 {
            bail!("{project}: log entry {} is out of place", entry.seq);
        }
        if entry.content_id != sha256_hex(&serde_json::to_vec(&entry.changes)?) {
            bail!(
                "{project}: log entry {} has the wrong content ID",
                entry.seq
            );
        }
        for change in &entry.changes {
            let key = (entry.model.clone(), change.element.clone());
            let stood = replayed.get(&key);
            if stood != change.before.as_ref() {
                bail!(
                    "{project}: log entry {} doesn't follow from the one before",
                    entry.seq
                );
            }
            match &change.after {
                Some(hash) => replayed.insert(key, hash.clone()),
                None => replayed.remove(&key),
            };
        }
    }

    let prefix = format!("p:{project}/elem/");
    let mut stored = BTreeMap::new();
    for (key, value) in kv::scan(&mut snap, &prefix).await? {
        let tail = key.strip_prefix(&prefix).context("element key")?;
        let (model, element) = tail.split_once('/').context("element key")?;
        stored.insert(
            (model.to_owned(), element.to_owned()),
            serde_json::from_slice(&value)?,
        );
    }
    if stored != replayed {
        bail!("{project}: stored elements differ from the replayed log");
    }
    for ((_, element), hash) in &stored {
        let bytes = snap
            .get(keys::record(project, hash))
            .await?
            .with_context(|| format!("{project}: record of {element} missing"))?;
        if sha256_hex(&bytes) != *hash {
            bail!("{project}: record of {element} doesn't match its hash");
        }
    }
    Ok(LogCheck {
        entries: head,
        elements: stored.len(),
    })
}
