//! Two writers regenerating one project at once, with the overlap forced rather than waited for: a
//! store that runs another writer at a chosen moment inside the transaction under test (as it
//! begins, or as it commits), so the interleaving is the same on every run.
//!
//! The writers are a rebuild's clear and a server's pass, as when an operator's rebuild meets the
//! pass a checkpoint starts (the server's kernel test of two doors failed so, now and then, with a
//! `Conflict`), or two servers' passes meet. However they interleave, the rows they leave are the
//! rows a rebuild alone makes, and `verify` agrees.

#![cfg(feature = "server")]
// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_model::element::{Element, Placement};
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::{Place, list};
use construct_model::rows::Model;
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use kit_core::Id;
use kit_ops::{Change, Edit};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Key, Kv, Pair, Prefix, Reader, Scope, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Clock, Commands, Context as Moment, Orderers, Outcome, Reply};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const PROJECT: &str = "p";
const DOCUMENT: &str = "doc";

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// Another writer's work, run once at the moment it is armed for.
type Writer = Pin<Box<dyn Future<Output = ()> + Send>>;

/// The test store, running an armed writer just before the next transaction begins (before it
/// takes its view of the store) or just before the next commit (after its view and its writes).
#[derive(Clone, Default)]
struct Interleaved {
    store: Memory,
    at_begin: Arc<Mutex<Option<Writer>>>,
    at_commit: Arc<Mutex<Option<Writer>>>,
}

impl Interleaved {
    fn before_next_begin(&self, writer: impl Future<Output = ()> + Send + 'static) {
        *self.at_begin.lock().unwrap() = Some(Box::pin(writer));
    }

    fn before_next_commit(&self, writer: impl Future<Output = ()> + Send + 'static) {
        *self.at_commit.lock().unwrap() = Some(Box::pin(writer));
    }
}

async fn run(armed: &Mutex<Option<Writer>>) {
    let writer = armed.lock().unwrap().take();
    if let Some(writer) = writer {
        writer.await;
    }
}

struct InterleavedTxn {
    txn: <Memory as Kv>::Txn,
    at_commit: Arc<Mutex<Option<Writer>>>,
}

impl Reader for InterleavedTxn {
    async fn get(&mut self, key: &Key) -> Result<Option<Vec<u8>>, StoreError> {
        self.txn.get(key).await
    }

    async fn scan(&mut self, prefix: &Prefix) -> Result<Vec<Pair>, StoreError> {
        self.txn.scan(prefix).await
    }

    async fn scan_between(&mut self, from: &Key, to: &Key) -> Result<Vec<Pair>, StoreError> {
        self.txn.scan_between(from, to).await
    }
}

impl Txn for InterleavedTxn {
    async fn put(&mut self, key: Key, value: Vec<u8>) -> Result<(), StoreError> {
        self.txn.put(key, value).await
    }

    async fn delete(&mut self, key: Key) -> Result<(), StoreError> {
        self.txn.delete(key).await
    }

    async fn commit(self) -> Result<(), StoreError> {
        run(&self.at_commit).await;
        self.txn.commit().await
    }

    async fn rollback(self) -> Result<(), StoreError> {
        self.txn.rollback().await
    }
}

impl Kv for Interleaved {
    type Snapshot = <Memory as Kv>::Snapshot;
    type Txn = InterleavedTxn;

    async fn snapshot(&self) -> Result<Self::Snapshot, StoreError> {
        self.store.snapshot().await
    }

    async fn begin(&self) -> Result<InterleavedTxn, StoreError> {
        run(&self.at_begin).await;
        Ok(InterleavedTxn {
            txn: self.store.begin().await?,
            at_commit: self.at_commit.clone(),
        })
    }
}

/// The only commands here: `put` stores a record as it is (`{"record": ...}`), and `model`
/// makes the document on the stand-in's edition.
struct Puts;

impl Commands<Memory> for Puts {
    fn version(&self) -> u32 {
        1
    }

    fn permit(
        &self,
        _: &Scope,
        _: &Edit,
    ) -> impl Future<Output = Result<Result<(), String>, StoreError>> + Send {
        std::future::ready(Ok(Ok(())))
    }

    async fn apply(
        &self,
        txn: &mut <Memory as Kv>::Txn,
        _: &Moment<'_>,
        edit: &Edit,
    ) -> Result<Outcome, StoreError> {
        if edit.cmd == "model" {
            let place = Place::Model {
                project: i(PROJECT),
                model: edit.target.clone(),
            };
            let model = Model {
                id: edit.target.clone(),
                name: "Doc".to_owned(),
                team: i("t"),
                package: "Doc package".to_owned(),
                edition: Boxes.edition(),
            };
            txn.put(place.key(), to_row(&model)?).await?;
            return Ok(Ok(Applied::default()));
        }
        let record: Element = serde_json::from_value(edit.args["record"].clone()).unwrap();
        let bytes = to_row(&record)?;
        let hash = i(&kit_ops::sha256_hex(&bytes));
        let place = kit_store::Place::Record {
            project: i(PROJECT),
            hash: hash.clone(),
        };
        txn.put(place.key(), bytes).await?;
        let pointer = kit_store::Place::Piece {
            project: i(PROJECT),
            document: edit.target.clone(),
            piece: record.id.clone(),
        };
        let before: Option<String> = read_row(txn, &pointer.key()).await?;
        txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
        Ok(Ok(Applied {
            changes: vec![Change {
                piece: record.id,
                before: before.map(|h| i(&h)),
                after: Some(hash),
            }],
            ..Applied::default()
        }))
    }
}

/// The orderer's time, fixed.
struct Fixed;

impl Clock for Fixed {
    fn now_ms(&self) -> u64 {
        1_000_000
    }
}

/// A log with two walls regenerated and a third not yet: work for whichever writer gets to it.
struct Stack {
    store: Interleaved,
    files: MemoryFiles,
    orderers: Orderers<Memory, Puts>,
    regen: Regenerator<Interleaved, MemoryFiles>,
    next: AtomicU64,
}

impl Stack {
    async fn new() -> Self {
        let store = Interleaved::default();
        let files = MemoryFiles::default();
        let orderers = Orderers::new(store.store.clone(), Arc::new(Puts), Arc::new(Fixed), 50);
        let regen = Regenerator::new(store.clone(), files.clone(), Arc::new(Boxes));
        let stack = Self {
            store,
            files,
            orderers,
            regen,
            next: AtomicU64::new(0),
        };
        stack.run("model", json!({})).await;
        stack.put(&wall("W-1", 3000)).await;
        stack.put(&wall("W-2", 4000)).await;
        stack.regen.catch_up(&i(PROJECT)).await.unwrap();
        stack.put(&wall("W-3", 5000)).await;
        stack
    }

    async fn run(&self, cmd: &str, args: Json) {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        let edit = Edit {
            person: i("anna"),
            client: i(&format!("c{n}")),
            counter: 1,
            base: 0,
            target: i(DOCUMENT),
            cmd: cmd.to_owned(),
            args,
            author_ms: 0,
        };
        let reply = self
            .orderers
            .run(&Scope::Project(i(PROJECT)), edit)
            .await
            .unwrap();
        assert!(matches!(reply, Reply::Ack { .. }), "{reply:?}");
    }

    async fn put(&self, record: &Element) {
        self.run("put", json!({ "record": record })).await;
    }

    /// Another server's regenerator, on the same store without the interleaving.
    fn other(&self) -> Regenerator<Memory, MemoryFiles> {
        Regenerator::new(
            self.store.store.clone(),
            self.files.clone(),
            Arc::new(Boxes),
        )
    }

    /// Every derived row of the project and its cursor.
    async fn derived(&self) -> BTreeMap<String, Vec<u8>> {
        let mut snapshot = self.store.store.snapshot().await.unwrap();
        let mut rows = BTreeMap::new();
        for table in list::derived(&i(PROJECT)) {
            for (key, value) in snapshot.scan(&table).await.unwrap() {
                rows.insert(key.as_str().to_owned(), value);
            }
        }
        let cursor = Place::Regenerated {
            project: i(PROJECT),
        }
        .key();
        let at = snapshot.get(&cursor).await.unwrap().unwrap_or_default();
        rows.insert(cursor.as_str().to_owned(), at);
        rows
    }

    /// What the overlap left is what a rebuild alone makes, and `verify` finds it right.
    async fn settled(&self) {
        let left = self.derived().await;
        self.other().rebuild(&i(PROJECT)).await.unwrap();
        assert_eq!(left, self.derived().await, "the rows a rebuild alone makes");
        assert_eq!(self.other().verify(&i(PROJECT)).await, Ok(3));
    }
}

fn wall(id: &str, length: i64) -> Element {
    let mut args = BTreeMap::new();
    args.insert("length".to_owned(), json!(length));
    args.insert("thickness".to_owned(), json!(200));
    args.insert("height".to_owned(), json!(2700));
    Element {
        id: i(id),
        document: i(DOCUMENT),
        kind: "wall".to_owned(),
        name: id.to_owned(),
        level: 0,
        placement: Placement {
            x: length,
            y: 0,
            z: 0,
        },
        recipe: Recipe::construct("wall.straight", args),
        props: BTreeMap::new(),
        references: Vec::new(),
    }
}

/// Another server's pass commits while a rebuild's clear is open: the clear conflicts, and clears
/// again what the pass left. It once failed the rebuild with the store's `Conflict`.
#[tokio::test(flavor = "multi_thread")]
async fn a_rebuild_clears_again_after_a_pass_commits_inside_it() {
    let stack = Stack::new().await;
    let other = stack.other();
    stack.store.before_next_commit(async move {
        other.catch_up(&i(PROJECT)).await.unwrap();
    });
    stack.regen.rebuild(&i(PROJECT)).await.unwrap();
    stack.settled().await;
}

/// A rebuild's clear commits after a pass has begun and before it reads the cursor: the pass reads
/// the cleared cursor and regenerates the whole log. Read before its transaction began, it once
/// wrote back the cursor it had read, over rows that were no longer there.
#[tokio::test(flavor = "multi_thread")]
async fn a_pass_starts_from_a_cursor_a_clear_has_just_reset() {
    let stack = Stack::new().await;
    let store = stack.store.store.clone();
    stack.store.before_next_begin(async move {
        clear(&store).await;
    });
    stack.regen.catch_up(&i(PROJECT)).await.unwrap();
    stack.settled().await;
}

/// A rebuild's clear commits after a pass has read the cursor and before it commits: the pass
/// conflicts on the cursor and starts again from nothing.
#[tokio::test(flavor = "multi_thread")]
async fn a_pass_starts_again_when_a_clear_commits_inside_it() {
    let stack = Stack::new().await;
    let store = stack.store.store.clone();
    stack.store.before_next_commit(async move {
        clear(&store).await;
    });
    stack.regen.catch_up(&i(PROJECT)).await.unwrap();
    stack.settled().await;
}

/// A rebuild's first transaction on its own, as another server runs it: every derived row of the
/// project and its cursor, deleted at once.
async fn clear(store: &Memory) {
    let mut txn = store.begin().await.unwrap();
    let mut keys = vec![
        Place::Regenerated {
            project: i(PROJECT),
        }
        .key(),
    ];
    for table in list::derived(&i(PROJECT)) {
        keys.extend(
            txn.scan(&table)
                .await
                .unwrap()
                .into_iter()
                .map(|(key, _)| key),
        );
    }
    for key in keys {
        txn.delete(key).await.unwrap();
    }
    txn.commit().await.unwrap();
}

/// Where regeneration stands now: the cursor, read without regenerating anything.
async fn cursor(stack: &Stack) -> u64 {
    let mut snapshot = stack.store.store.snapshot().await.unwrap();
    let place = Place::Regenerated {
        project: i(PROJECT),
    };
    read_row(&mut snapshot, &place.key())
        .await
        .unwrap()
        .unwrap_or(0)
}

/// A pass on its own begins where regeneration stood, builds what is left, and says so.
#[tokio::test(flavor = "multi_thread")]
async fn a_pass_says_where_it_began() {
    let stack = Stack::new().await;
    let before = cursor(&stack).await;
    let pass = stack.regen.catch_up_alone(&i(PROJECT)).await.unwrap();
    assert_eq!(pass.from, Some(before));
    assert!(!pass.overtaken);
    assert!(pass.through > before);
    assert!(pass.bodies[&i(DOCUMENT)].contains_key(&i("W-3")));
    assert_eq!(pass.through, cursor(&stack).await);
    stack.settled().await;
}

/// Another server's pass commits inside this one's: this one conflicts and, on its own, stops
/// where the other left regeneration, holding nothing (it built nothing that stands), so no
/// checkpoint can name the other's work with this pass's bodies. The server asks again.
#[tokio::test(flavor = "multi_thread")]
async fn a_pass_on_its_own_stops_where_another_overtook_it() {
    let stack = Stack::new().await;
    let before = cursor(&stack).await;
    let other = stack.other();
    stack.store.before_next_commit(async move {
        other.catch_up(&i(PROJECT)).await.unwrap();
    });
    let pass = stack.regen.catch_up_alone(&i(PROJECT)).await.unwrap();
    assert!(pass.overtaken);
    assert_eq!((pass.from, pass.through), (Some(before), before));
    assert!(pass.bodies.is_empty());
    let next = stack.regen.catch_up_alone(&i(PROJECT)).await.unwrap();
    assert!(next.from > Some(before));
    assert_eq!(next.from, Some(next.through));
    assert!(!next.overtaken && next.bodies.is_empty());
    stack.settled().await;
}

/// The same overlap under `catch_up`, which goes on to the end of the log: it says it was
/// overtaken.
#[tokio::test(flavor = "multi_thread")]
async fn a_pass_to_the_end_says_it_was_overtaken() {
    let stack = Stack::new().await;
    let other = stack.other();
    stack.store.before_next_commit(async move {
        other.catch_up(&i(PROJECT)).await.unwrap();
    });
    let pass = stack.regen.catch_up(&i(PROJECT)).await.unwrap();
    assert!(pass.overtaken);
    assert_eq!(pass.through, cursor(&stack).await);
    stack.settled().await;
}
