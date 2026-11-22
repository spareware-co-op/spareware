//! Undo and redo through the edit log, on the test store: an undo is a new entry in the same log
//! that reverses one earlier step of the same person, built from that step's recorded before and
//! after values, through the same access, claims and model rules as any command. Nothing is ever
//! removed from the log, and the order of each person's steps is read from the log alone.

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_api::cloud::undo::{self, State, Step};
use construct_api::cloud::{Access, ProjectCommands};
use construct_model::element::Element;
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{Project, Team};
use construct_regen::stand_in::Boxes;
use kit_core::Id;
use kit_ops::{Decider, Edit, Entry, Question};
use kit_store::memory::Memory;
use kit_store::{Kv, Reader, Scope, Txn, read_row, to_row};
use kit_sync::{Clock, Orderers, Reply, read_log, verify_log};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DOCUMENT: &str = "doc";
/// How long a claim lasts here, as the server's setting would.
const CLAIM_MS: u64 = 30_000;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// Says yes. Access has tests of its own, in the server, against the real rules.
struct Yes;

impl Decider for Yes {
    fn decide(&self, _: &Question) -> Result<bool, String> {
        Ok(true)
    }
}

/// The orderer's time, set by the test, so a replay can run every edit at its recorded time.
struct Fixed(AtomicU64);

impl Clock for Fixed {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

type Ordering_ = Orderers<Memory, ProjectCommands<Memory, Yes>>;

fn orderers(store: &Memory, clock: &Arc<Fixed>) -> Ordering_ {
    let access = Arc::new(Access::new(store.clone(), Yes));
    let commands = ProjectCommands::new(store.clone(), access, CLAIM_MS, Boxes.edition());
    Orderers::new(store.clone(), Arc::new(commands), clock.clone(), 50)
}

struct Stack {
    store: Memory,
    orderers: Ordering_,
    clock: Arc<Fixed>,
    next: AtomicU64,
}

impl Stack {
    /// A store with the rows the directory would have written, and no model yet.
    async fn bare() -> Self {
        let store = Memory::new();
        let clock = Arc::new(Fixed(AtomicU64::new(1_000_000)));
        let orderers = orderers(&store, &clock);
        let mut txn = store.begin().await.unwrap();
        let project = Project {
            id: i("p"),
            name: "P".to_owned(),
            hub: i("h"),
        };
        txn.put(Place::Project(i("p")).key(), to_row(&project).unwrap())
            .await
            .unwrap();
        let team = Team {
            id: i("t"),
            name: "T".to_owned(),
        };
        let place = Place::Team {
            project: i("p"),
            team: i("t"),
        };
        txn.put(place.key(), to_row(&team).unwrap()).await.unwrap();
        txn.commit().await.unwrap();
        Self {
            store,
            orderers,
            clock,
            next: AtomicU64::new(0),
        }
    }

    async fn new() -> Self {
        let stack = Self::bare().await;
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
        stack
    }

    async fn run_on(orderers: &Ordering_, edit: Edit) -> Result<u64, String> {
        match orderers.run(&Scope::Project(i("p")), edit).await.unwrap() {
            Reply::Ack { seq, .. } => Ok(seq),
            Reply::Rejected { reason, .. } => Err(reason),
            Reply::Retry { .. } => Err("retry".to_owned()),
        }
    }

    fn next_edit(&self, person: &str, cmd: &str, args: Value) -> Edit {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        self.clock.0.fetch_add(10, Ordering::SeqCst);
        Edit {
            person: i(person),
            client: i(&format!("c{n}")),
            counter: 1,
            base: 0,
            target: i(DOCUMENT),
            cmd: cmd.to_owned(),
            args,
            author_ms: 0,
        }
    }

    async fn edit_as(&self, person: &str, cmd: &str, args: Value) -> Result<u64, String> {
        let edit = self.next_edit(person, cmd, args);
        Self::run_on(&self.orderers, edit).await
    }

    async fn edit(&self, cmd: &str, args: Value) -> Result<u64, String> {
        self.edit_as("anna", cmd, args).await
    }

    async fn undo(&self, person: &str) -> Result<u64, String> {
        self.edit_as(person, "edit.undo", json!({})).await
    }

    async fn redo(&self, person: &str) -> Result<u64, String> {
        self.edit_as(person, "edit.redo", json!({})).await
    }

    /// Lets every claim run out.
    fn later(&self) {
        self.clock.0.fetch_add(CLAIM_MS + 1, Ordering::SeqCst);
    }

    async fn record(&self, id: &str) -> Element {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let pointer = kit_store::Place::Piece {
            project: i("p"),
            document: i(DOCUMENT),
            piece: i(id),
        };
        let hash: String = read_row(&mut snapshot, &pointer.key())
            .await
            .unwrap()
            .unwrap();
        let record = kit_store::Place::Record {
            project: i("p"),
            hash: i(&hash),
        };
        read_row(&mut snapshot, &record.key())
            .await
            .unwrap()
            .unwrap()
    }

    async fn exists(&self, id: &str) -> bool {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let pointer = kit_store::Place::Piece {
            project: i("p"),
            document: i(DOCUMENT),
            piece: i(id),
        };
        snapshot.get(&pointer.key()).await.unwrap().is_some()
    }

    /// Every row under a table of the project, as bytes.
    async fn rows(&self, table: &str) -> BTreeMap<String, Vec<u8>> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let prefix = Scope::Project(i("p")).table_prefix(table, &[]);
        snapshot
            .scan(&prefix)
            .await
            .unwrap()
            .into_iter()
            .map(|(key, value)| (key.as_str().to_owned(), value))
            .collect()
    }

    /// Where every piece of the project points: the model as it stands.
    async fn pieces(&self) -> BTreeMap<String, Vec<u8>> {
        self.rows("piece").await
    }

    async fn log(&self) -> Vec<Entry> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let head: u64 = read_row(&mut snapshot, &Scope::Project(i("p")).head())
            .await
            .unwrap()
            .unwrap_or(0);
        read_log(&mut snapshot, &Scope::Project(i("p")), 0, head)
            .await
            .unwrap()
    }

    async fn entry(&self, seq: u64) -> Entry {
        self.log().await.into_iter().find(|e| e.seq == seq).unwrap()
    }

    async fn state(&self, person: &str) -> State {
        let mut snapshot = self.store.snapshot().await.unwrap();
        undo::state(&mut snapshot, &i("p"), &i(DOCUMENT), &i(person))
            .await
            .unwrap()
    }
}

fn recipe(op: &str, args: Value) -> Value {
    serde_json::to_value(Recipe::construct(op, serde_json::from_value(args).unwrap())).unwrap()
}

fn def(id: &str, name: &str, kind: &str, holder: &str) -> Value {
    json!({ "definition": { "id": id, "name": name, "kind": kind, "holder": holder,
                            "choices": [] } })
}

fn wall(id: &str) -> Value {
    let args = json!({ "length": 3000, "thickness": 200, "height": 2700 });
    json!({ "element": id, "kind": "wall", "level": 1, "placement": { "x": 0, "y": 0, "z": 0 },
            "recipe": recipe("wall.straight", args) })
}

fn door(id: &str, ty: &str) -> Value {
    let args = json!({ "width": 820, "thickness": 40, "height": 2100 });
    json!({ "element": id, "kind": "door", "level": 1, "type": ty,
            "placement": { "x": 0, "y": 0, "z": 0 }, "recipe": recipe("door.single", args) })
}

fn part(features: u64) -> Value {
    json!({ "owner": "workshop", "schema": 1, "op": "part", "op_version": 1,
            "args": { "end": features, "features": [], "material": "steel", "params": [] } })
}

fn at(element: &str, x: i64) -> Value {
    json!({ "element": element, "placement": { "x": x, "y": 0, "z": 0 } })
}

/// Runs a command, undoes it and redoes it: the undo puts every piece back as it was before the
/// command, and the redo as it was after.
async fn undone_and_redone(stack: &Stack, cmd: &str, args: Value) {
    let before = stack.pieces().await;
    stack
        .edit(cmd, args)
        .await
        .unwrap_or_else(|why| panic!("{cmd}: {why}"));
    let after = stack.pieces().await;
    assert_ne!(before, after, "{cmd} changed something");
    assert_eq!(
        stack.state("anna").await.undo.map(|step| step.cmd),
        Some(cmd.to_owned())
    );
    stack
        .undo("anna")
        .await
        .unwrap_or_else(|why| panic!("undo of {cmd}: {why}"));
    assert_eq!(stack.pieces().await, before, "undo of {cmd}");
    stack
        .redo("anna")
        .await
        .unwrap_or_else(|why| panic!("redo of {cmd}: {why}"));
    assert_eq!(stack.pieces().await, after, "redo of {cmd}");
}

#[tokio::test]
async fn an_undo_is_a_new_entry_that_reverses_the_recorded_change() {
    let stack = Stack::new().await;
    stack.edit("element.create", wall("W-1")).await.unwrap();
    let set = stack
        .edit("element.set", json!({ "element": "W-1", "name": "North" }))
        .await
        .unwrap();
    let before = stack.log().await;

    let seq = stack.undo("anna").await.unwrap();
    assert_eq!(stack.record("W-1").await.name, "W-1");
    // The undo is an ordinary entry, after everything else, whose change is the step's change
    // turned round; every earlier entry is still there, unchanged.
    let log = stack.log().await;
    assert_eq!(&log[..before.len()], &before[..]);
    assert_eq!(log.len(), before.len() + 1);
    let (undone, step) = (stack.entry(seq).await, stack.entry(set).await);
    assert_eq!(undone.cmd, "edit.undo");
    assert_eq!(undone.failed, None);
    assert_eq!(undone.changes.len(), 1);
    assert_eq!(undone.changes[0].piece, step.changes[0].piece);
    assert_eq!(undone.changes[0].before, step.changes[0].after);
    assert_eq!(undone.changes[0].after, step.changes[0].before);
    assert_eq!(undone.claims[0].claim.as_ref().unwrap().person, i("anna"));

    // Redo is the undo of the undo: its change is the undo's, turned round.
    let seq = stack.redo("anna").await.unwrap();
    assert_eq!(stack.record("W-1").await.name, "North");
    let redone = stack.entry(seq).await;
    assert_eq!(redone.cmd, "edit.redo");
    assert_eq!(redone.changes[0].before, undone.changes[0].after);
    assert_eq!(redone.changes[0].after, undone.changes[0].before);

    let mut snapshot = stack.store.snapshot().await.unwrap();
    verify_log(&mut snapshot, &Scope::Project(i("p")))
        .await
        .unwrap();
}

#[tokio::test]
async fn every_command_that_changes_a_model_is_undone_and_redone() {
    let stack = Stack::new().await;
    for (cmd, args) in [
        (
            "parameter.define",
            def("P-mark", "Mark", "text", "instance"),
        ),
        (
            "parameter.define",
            def("P-width", "Width", "length", "type"),
        ),
        (
            "type.create",
            json!({ "type": "T-wall", "name": "Basic", "category": 100 }),
        ),
        (
            "type.create",
            json!({ "type": "T-door", "name": "Single", "category": 300 }),
        ),
        (
            "parameter.bind",
            json!({ "parameter": "P-mark", "categories": [100, 300] }),
        ),
        (
            "parameter.bind",
            json!({ "parameter": "P-width", "categories": [300], "default": "900" }),
        ),
        (
            "type.rename",
            json!({ "type": "T-door", "name": "Single leaf" }),
        ),
        (
            "type.duplicate",
            json!({ "type": "T-door", "new": "T-door2", "name": "Wide" }),
        ),
        (
            "type.set",
            json!({ "type": "T-door2", "parameter": "P-width", "value": "1.2 m" }),
        ),
        ("element.create", wall("W-1")),
        (
            "element.set",
            json!({ "element": "W-1", "name": "North", "props": { "note": "kept" } }),
        ),
        ("element.move", at("W-1", 500)),
        (
            "element.param",
            json!({ "element": "W-1", "parameter": "P-mark", "value": "W01" }),
        ),
        ("element.create", door("D-1", "T-door")),
        (
            "element.type",
            json!({ "element": "D-1", "type": "T-door2" }),
        ),
        ("type.delete", json!({ "type": "T-wall" })),
        ("element.delete", json!({ "element": "W-1" })),
        // Workshop's parts are elements, changed by the same three commands.
        (
            "element.create",
            json!({ "element": "WP-1", "kind": "workshop.part", "name": "Bracket", "level": 0,
                    "placement": { "x": 0, "y": 0, "z": 0 }, "recipe": part(0) }),
        ),
        (
            "element.set",
            json!({ "element": "WP-1", "name": "Bracket 2", "recipe": part(1) }),
        ),
        ("element.delete", json!({ "element": "WP-1" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
    let mut snapshot = stack.store.snapshot().await.unwrap();
    verify_log(&mut snapshot, &Scope::Project(i("p")))
        .await
        .unwrap();
}

#[tokio::test]
async fn one_step_is_one_change_as_a_person_thinks_of_it() {
    let stack = Stack::new().await;
    stack
        .edit(
            "parameter.define",
            def("P-fire", "Fire rating", "text", "type"),
        )
        .await
        .unwrap();
    for (ty, category) in [("T-a", 300), ("T-b", 300), ("T-w", 100)] {
        stack
            .edit(
                "type.create",
                json!({ "type": ty, "name": ty, "category": category }),
            )
            .await
            .unwrap();
    }
    // One bind rewrites the table and every type of its categories: one step.
    let before = stack.pieces().await;
    let bind = stack
        .edit(
            "parameter.bind",
            json!({ "parameter": "P-fire", "categories": [100, 300] }),
        )
        .await
        .unwrap();
    assert_eq!(stack.entry(bind).await.changes.len(), 4);
    stack.undo("anna").await.unwrap();
    assert_eq!(stack.pieces().await, before);
    stack.redo("anna").await.unwrap();

    // A drag sends a move every few milliseconds, after the claim it starts with: the whole drag
    // is one step, and a second drag another.
    stack.edit("element.create", wall("W-1")).await.unwrap();
    stack
        .edit("element.claim", json!({ "element": "W-1" }))
        .await
        .unwrap();
    for x in [50, 100, 150, 200, 250] {
        stack.edit("element.move", at("W-1", x)).await.unwrap();
    }
    stack
        .edit("element.claim", json!({ "element": "W-1" }))
        .await
        .unwrap();
    for x in [300, 350, 400] {
        stack.edit("element.move", at("W-1", x)).await.unwrap();
    }
    stack.undo("anna").await.unwrap();
    assert_eq!(stack.record("W-1").await.placement.x, 250);
    stack.undo("anna").await.unwrap();
    assert_eq!(stack.record("W-1").await.placement.x, 0);
    stack.undo("anna").await.unwrap();
    assert!(!stack.exists("W-1").await, "then the wall itself");
    stack.redo("anna").await.unwrap();
    stack.redo("anna").await.unwrap();
    assert_eq!(stack.record("W-1").await.placement.x, 250);
}

#[tokio::test]
async fn each_person_undoes_only_their_own_steps() {
    let stack = Stack::new().await;
    assert_eq!(
        stack.undo("anna").await,
        Err(undo::NOTHING_TO_UNDO.to_owned())
    );
    assert_eq!(
        stack.redo("anna").await,
        Err(undo::NOTHING_TO_REDO.to_owned())
    );
    stack.edit("element.create", wall("W-1")).await.unwrap();
    stack
        .edit("element.set", json!({ "element": "W-1", "name": "Anna's" }))
        .await
        .unwrap();
    stack.later();
    stack
        .edit_as("ben", "element.create", wall("W-2"))
        .await
        .unwrap();
    stack
        .edit_as(
            "ben",
            "element.set",
            json!({ "element": "W-1", "props": { "note": "Ben's" } }),
        )
        .await
        .unwrap();
    stack.later();

    // Anna's undo reverses her rename, never Ben's later edits, and keeps his value of another
    // column of the same wall.
    stack.undo("anna").await.unwrap();
    let w1 = stack.record("W-1").await;
    assert_eq!(w1.name, "W-1");
    assert_eq!(w1.props["note"], "Ben's", "Ben's note stays");
    assert!(stack.exists("W-2").await);

    // Ben's own order is his: his note, then his wall.
    stack.later();
    stack.undo("ben").await.unwrap();
    assert!(!stack.record("W-1").await.props.contains_key("note"));
    stack.undo("ben").await.unwrap();
    assert!(!stack.exists("W-2").await);
    assert_eq!(
        stack.undo("ben").await,
        Err(undo::NOTHING_TO_UNDO.to_owned())
    );
    assert!(
        stack.exists("W-1").await,
        "Anna's wall is not Ben's to undo"
    );
    assert_eq!(
        stack.state("anna").await.undo.map(|step| step.cmd),
        Some("element.create".to_owned())
    );
}

/// A05's stale-write rule, column by column, and the transaction rule: an undo is one command,
/// so when any value it would put back has been changed since by someone else, the whole undo is
/// turned back and nothing of it is written.
#[tokio::test]
async fn a_value_changed_since_by_someone_else_turns_the_whole_undo_back() {
    let stack = Stack::new().await;
    for (id, name) in [("P-fire", "Fire rating"), ("P-acoustic", "Acoustic rating")] {
        stack
            .edit("parameter.define", def(id, name, "text", "type"))
            .await
            .unwrap();
    }
    for (ty, category) in [("T-door", 300), ("T-wall", 100)] {
        stack
            .edit(
                "type.create",
                json!({ "type": ty, "name": ty, "category": category }),
            )
            .await
            .unwrap();
    }
    stack
        .edit(
            "parameter.bind",
            json!({ "parameter": "P-fire", "categories": [100, 300] }),
        )
        .await
        .unwrap();
    stack.later();
    // Ben binds another parameter to doors: the table and the door type's list of parameters,
    // both of which Anna's bind wrote, now hold his values. The wall type is not his.
    stack
        .edit_as(
            "ben",
            "parameter.bind",
            json!({ "parameter": "P-acoustic", "categories": [300] }),
        )
        .await
        .unwrap();
    stack.later();
    let before = stack.pieces().await;
    let refused = stack.undo("anna").await.unwrap_err();
    assert_eq!(refused, "ben has changed Parameters since.");
    assert_eq!(stack.pieces().await, before, "nothing of it was written");
    assert_eq!(
        stack.record("T-wall").await.props["@params"],
        "P-fire",
        "not even the wall type, which nobody else changed"
    );
    // The refusal is in the log as turned back, and it is no step: her bind is still hers to
    // undo once Ben's is undone.
    let last = stack.log().await.pop().unwrap();
    assert_eq!(last.cmd, "edit.undo");
    assert_eq!(last.failed.as_deref(), Some(refused.as_str()));
    stack.undo("ben").await.unwrap();
    stack.later();
    stack.undo("anna").await.unwrap();
    assert_eq!(stack.record("T-wall").await.props["@params"], "");

    // A column nobody else changed is put back even when others changed other columns.
    stack.edit("element.create", wall("W-1")).await.unwrap();
    stack
        .edit("element.set", json!({ "element": "W-1", "name": "Mine" }))
        .await
        .unwrap();
    stack.later();
    stack
        .edit_as("ben", "element.move", at("W-1", 900))
        .await
        .unwrap();
    stack.later();
    stack.undo("anna").await.unwrap();
    let w1 = stack.record("W-1").await;
    assert_eq!((w1.name.as_str(), w1.placement.x), ("W-1", 900));
    // And a piece someone else deleted since can't be changed back.
    stack.redo("anna").await.unwrap();
    stack.later();
    stack
        .edit_as("ben", "element.delete", json!({ "element": "W-1" }))
        .await
        .unwrap();
    stack.later();
    assert_eq!(
        stack.undo("anna").await,
        Err("ben has changed Mine since.".to_owned())
    );
    assert!(!stack.exists("W-1").await);
}

#[tokio::test]
async fn another_persons_claim_refuses_an_undo_as_it_refuses_any_edit() {
    let stack = Stack::new().await;
    stack.edit("element.create", wall("W-1")).await.unwrap();
    stack
        .edit("element.set", json!({ "element": "W-1", "name": "North" }))
        .await
        .unwrap();
    stack.later();
    stack
        .edit_as("ben", "element.claim", json!({ "element": "W-1" }))
        .await
        .unwrap();
    assert_eq!(
        stack.undo("anna").await,
        Err("ben is working on this.".to_owned())
    );
    assert_eq!(stack.record("W-1").await.name, "North");
    stack
        .edit_as("ben", "element.release", json!({ "element": "W-1" }))
        .await
        .unwrap();
    stack.undo("anna").await.unwrap();
    assert_eq!(stack.record("W-1").await.name, "W-1");
}

#[tokio::test]
async fn a_new_edit_clears_redo() {
    let stack = Stack::new().await;
    stack.edit("element.create", wall("W-1")).await.unwrap();
    stack
        .edit("element.set", json!({ "element": "W-1", "name": "A" }))
        .await
        .unwrap();
    stack.undo("anna").await.unwrap();
    assert!(stack.state("anna").await.redo.is_some());
    stack
        .edit("element.set", json!({ "element": "W-1", "name": "B" }))
        .await
        .unwrap();
    assert_eq!(stack.state("anna").await.redo, None);
    assert_eq!(
        stack.redo("anna").await,
        Err(undo::NOTHING_TO_REDO.to_owned())
    );
    assert_eq!(stack.record("W-1").await.name, "B");
    // Another person's edit leaves her redo alone.
    stack.undo("anna").await.unwrap();
    stack.later();
    stack
        .edit_as("ben", "element.create", wall("W-2"))
        .await
        .unwrap();
    stack.redo("anna").await.unwrap();
    assert_eq!(stack.record("W-1").await.name, "B");
}

#[tokio::test]
async fn a_reversal_that_would_break_a_model_rule_is_refused_whole() {
    let stack = Stack::new().await;
    stack
        .edit(
            "type.create",
            json!({ "type": "T-door", "name": "Single", "category": 300 }),
        )
        .await
        .unwrap();
    stack.later();
    stack
        .edit_as("ben", "element.create", door("D-1", "T-door"))
        .await
        .unwrap();
    // Taking the type away would leave Ben's door of a type that isn't there.
    let refused = stack.undo("anna").await.unwrap_err();
    assert!(refused.starts_with(undo::CANNOT), "{refused}");
    assert!(stack.exists("T-door").await);
}

/// The order is read from the log alone: a server that starts again, or the same log run again
/// on an empty store, gives the same state, and the next undo does the same.
#[tokio::test]
async fn the_undo_order_comes_from_the_log_alone() {
    let stack = Stack::new().await;
    stack.edit("element.create", wall("W-1")).await.unwrap();
    for name in ["A", "B", "C"] {
        stack
            .edit("element.set", json!({ "element": "W-1", "name": name }))
            .await
            .unwrap();
    }
    stack.later();
    stack
        .edit_as("ben", "element.create", wall("W-2"))
        .await
        .unwrap();
    stack.undo("anna").await.unwrap();
    stack.undo("anna").await.unwrap();
    stack.redo("anna").await.unwrap();
    let _ = stack.redo("ben").await;
    let anna = stack.state("anna").await;
    assert_eq!(
        anna,
        State {
            undo: Some(Step {
                seq: anna.undo.as_ref().unwrap().seq,
                cmd: "edit.redo".to_owned()
            }),
            redo: Some(Step {
                seq: anna.redo.as_ref().unwrap().seq,
                cmd: "edit.undo".to_owned()
            }),
        }
    );
    assert_eq!(stack.record("W-1").await.name, "B");

    // The same log, run again in order at its recorded times on an empty store.
    let entries = stack.log().await;
    let again = Stack::bare().await;
    for entry in &entries {
        again.clock.0.store(entry.orderer_ms, Ordering::SeqCst);
        let edit = Edit {
            person: entry.author.clone(),
            client: entry.client.clone(),
            counter: entry.counter,
            base: entry.base,
            target: entry.target.clone(),
            cmd: entry.cmd.clone(),
            args: entry.args.clone(),
            author_ms: entry.author_ms,
        };
        let _ = Stack::run_on(&again.orderers, edit).await;
    }
    assert_eq!(again.log().await, entries);
    for table in ["record", "piece"] {
        assert_eq!(again.rows(table).await, stack.rows(table).await, "{table}");
    }
    for person in ["anna", "ben"] {
        assert_eq!(again.state(person).await, stack.state(person).await);
    }

    // A server started again holds nothing of the order: new orderers on the same store.
    let restarted = orderers(&stack.store, &stack.clock);
    let edit = stack.next_edit("anna", "edit.undo", json!({}));
    Stack::run_on(&restarted, edit).await.unwrap();
    assert_eq!(stack.record("W-1").await.name, "A");
    let edit = stack.next_edit("anna", "edit.redo", json!({}));
    Stack::run_on(&restarted, edit).await.unwrap();
    assert_eq!(stack.record("W-1").await.name, "B");
}

/// A click on an element, as the page sends it: a claim, then a move to where it already is.
async fn click(stack: &Stack, element: &str) -> u64 {
    stack
        .edit("element.claim", json!({ "element": element }))
        .await
        .unwrap();
    stack.edit("element.move", at(element, 0)).await.unwrap()
}

/// Clicking an element to select it claims it and, on letting go, sends a move to where it
/// already is: an entry that changes no record. It is no step: the undo after it reverses the
/// change before it, and it leaves the redo order alone.
#[tokio::test]
async fn an_edit_that_changed_nothing_is_no_step() {
    let stack = Stack::new().await;
    stack.edit("element.create", wall("W-1")).await.unwrap();
    let set = stack
        .edit("element.set", json!({ "element": "W-1", "name": "A" }))
        .await
        .unwrap();
    let still = click(&stack, "W-1").await;
    let entry = stack.entry(still).await;
    assert_eq!(entry.changes[0].before, entry.changes[0].after);
    assert_eq!(
        stack.state("anna").await.undo,
        Some(Step {
            seq: set,
            cmd: "element.set".to_owned()
        })
    );

    let undone = stack.undo("anna").await.unwrap();
    assert_eq!(stack.record("W-1").await.name, "W-1");
    let state = stack.state("anna").await;
    assert_eq!(
        state.undo.map(|step| step.cmd),
        Some("element.create".to_owned())
    );
    assert_eq!(
        state.redo,
        Some(Step {
            seq: undone,
            cmd: "edit.undo".to_owned()
        })
    );

    // Another click after the undo keeps the redo.
    click(&stack, "W-1").await;
    assert!(stack.state("anna").await.redo.is_some());
    stack.redo("anna").await.unwrap();
    assert_eq!(stack.record("W-1").await.name, "A");
}
