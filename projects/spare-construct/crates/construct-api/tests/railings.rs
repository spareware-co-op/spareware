//! Railings through the edit log, on the test store: a sketched railing made on a level and
//! changed by its own commands (its type, its path, its side) and deleted; a railing on a stair
//! laid out from the stair and laid out again by regeneration alone when the stair's level moves;
//! a limit broken shown as a warning by name, never a refusal; every refusal in words; the generic
//! commands refused where a railing's own exist; undo and redo of every railing command; the
//! page's view of a document's railings; and the log replayed from empty to the same bytes.

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

use construct_api::cloud::railings::read_railings;
use construct_api::cloud::undo::{self, State};
use construct_api::cloud::{Access, ProjectCommands};
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_regen::parameters as records;
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use kit_core::Id;
use kit_ops::{Decider, Edit, Question};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Reader, Scope, Txn, read_row, to_row};
use kit_sync::{Clock, Orderers, Reply, read_log};
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

struct Stack {
    store: Memory,
    orderers: Orderers<Memory, ProjectCommands<Memory, Yes>>,
    regen: Regenerator<Memory, MemoryFiles>,
    clock: Arc<Fixed>,
    next: AtomicU64,
}

impl Stack {
    /// A store with the rows the directory would have written, and no model yet.
    async fn bare() -> Self {
        let store = Memory::new();
        let geometry: Arc<dyn Geometry> = Arc::new(Boxes);
        let access = Arc::new(Access::new(store.clone(), Yes));
        let commands = ProjectCommands::new(store.clone(), access, CLAIM_MS, geometry.edition());
        let clock = Arc::new(Fixed(AtomicU64::new(1_000_000)));
        let orderers = Orderers::new(store.clone(), Arc::new(commands), clock.clone(), 50);
        let regen = Regenerator::new(store.clone(), MemoryFiles::default(), geometry);
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
            regen,
            clock,
            next: AtomicU64::new(0),
        }
    }

    /// A document with Ground at 0 and First at 2700 mm.
    async fn new() -> Self {
        let stack = Self::bare().await;
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
        for (id, name, elevation) in [("L-g", "Ground", "0"), ("L-1", "First", "2700 mm")] {
            let args = json!({ "level": id, "name": name, "elevation": elevation });
            stack.edit("level.create", args).await.unwrap();
        }
        stack
    }

    async fn run(&self, edit: Edit) -> Result<u64, String> {
        match self
            .orderers
            .run(&Scope::Project(i("p")), edit)
            .await
            .unwrap()
        {
            Reply::Ack { seq, .. } => Ok(seq),
            Reply::Rejected { reason, .. } => Err(reason),
            Reply::Retry { .. } => Err("retry".to_owned()),
        }
    }

    async fn edit_as(&self, person: &str, cmd: &str, args: Value) -> Result<u64, String> {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        self.clock.0.fetch_add(10, Ordering::SeqCst);
        let edit = Edit {
            person: i(person),
            client: i(&format!("c{n}")),
            counter: 1,
            base: 0,
            target: i(DOCUMENT),
            cmd: cmd.to_owned(),
            args,
            author_ms: 0,
        };
        self.run(edit).await
    }

    async fn edit(&self, cmd: &str, args: Value) -> Result<u64, String> {
        self.edit_as("anna", cmd, args).await
    }

    async fn record(&self, id: &str) -> Option<Element> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        records::read_element(&mut snapshot, &i("p"), &i(DOCUMENT), &i(id))
            .await
            .unwrap()
            .map(|(_, record)| record)
    }

    async fn built(&self, id: &str) -> BuiltElement {
        self.regen.catch_up(&i("p")).await.unwrap();
        let place = Place::Built {
            project: i("p"),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_row(&mut snapshot, &place.key())
            .await
            .unwrap()
            .unwrap()
    }

    /// An element's built bounds, in whole millimetres, or its failure in words.
    async fn bounds(&self, id: &str) -> Result<([i64; 3], [i64; 3]), String> {
        match self.built(id).await.result {
            BuiltResult::Shape { bounds, .. } => Ok((bounds.min, bounds.max)),
            BuiltResult::Failed { code, message } => Err(format!("{code}: {message}")),
        }
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

    async fn derived(&self) -> BTreeMap<String, Vec<u8>> {
        let mut rows = BTreeMap::new();
        for table in ["body", "display", "built", "area", "kind", "reads"] {
            rows.extend(self.rows(table).await);
        }
        rows
    }

    async fn state(&self, person: &str) -> State {
        let mut snapshot = self.store.snapshot().await.unwrap();
        undo::state(&mut snapshot, &i("p"), &i(DOCUMENT), &i(person))
            .await
            .unwrap()
    }

    async fn view(&self) -> construct_api::cloud::railings::RailingsView {
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_railings(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap()
    }
}

/// `railing.create`'s arguments: railing `id` on Ground along a 3 m path, with `more` added.
fn create(id: &str, more: &Value) -> Value {
    let mut args = json!({ "element": id, "level": "L-g", "path": [["0", "0"], ["3 m", "0"]] });
    if let (Some(args), Some(more)) = (args.as_object_mut(), more.as_object()) {
        for (key, value) in more {
            args.insert(key.clone(), value.clone());
        }
    }
    args
}

/// A main private stair `S-1` from Ground to First (2700 mm): 15 risers of 180 mm on 280 mm
/// goings, one run 3920 mm long.
async fn with_stair(stack: &Stack) {
    let args = json!({ "element": "S-1", "level": "L-g", "top": "L-1", "start": ["0", "0"],
                       "category": "main-private" });
    stack.edit("stair.create", args).await.unwrap();
}

/// A sketched railing is made on a level and changed by its own commands: 3 m at the standard
/// type's 110 mm takes 27 balusters and two posts; its type made 130 mm apart leaves 110 mm
/// between them, a warning in the view (a 100 mm sphere passes, F4/AS1 1.2.1 b)), never a
/// refusal; its path made an L adds a post; flipped, its handrail goes to its other side; and it
/// is deleted. Every refusal changes nothing.
#[tokio::test]
async fn a_railing_is_made_changed_and_deleted_by_its_own_commands() {
    let stack = Stack::new().await;
    stack
        .edit(
            "railing.create",
            create("R-1", &json!({ "name": "Balcony" })),
        )
        .await
        .unwrap();
    let record = stack.record("R-1").await.unwrap();
    assert_eq!(
        (record.kind.as_str(), record.name.as_str()),
        ("railing", "Balcony")
    );
    let view = stack.view().await;
    let laid = view.railings[0].laid.as_ref().unwrap();
    assert_eq!((laid.balusters, laid.posts), (27, 2));
    assert_eq!(laid.rail_length.shown, "3000 mm");
    assert_eq!(laid.warnings.len(), 0);
    assert_eq!(
        stack.bounds("R-1").await.unwrap(),
        ([-25, -25, 0], [3025, 25, 1000])
    );

    stack
        .edit(
            "railing.type",
            json!({ "element": "R-1", "type": { "balusters": { "spacing": "130 mm" } } }),
        )
        .await
        .unwrap();
    let view = stack.view().await;
    let laid = view.railings[0].laid.as_ref().unwrap();
    assert_eq!(laid.warnings.len(), 1);
    assert_eq!(laid.warnings[0].identity, "railing.opening|R-1");
    assert_eq!(laid.warnings[0].severity, "warning");

    stack
        .edit(
            "railing.path",
            json!({ "element": "R-1", "path": [["0", "0"], ["3 m", "0"], ["3 m", "2 m"]] }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack.view().await.railings[0].laid.as_ref().unwrap().posts,
        3
    );
    let before = stack.record("R-1").await.unwrap();
    stack
        .edit("railing.flip", json!({ "element": "R-1" }))
        .await
        .unwrap();
    assert_ne!(stack.record("R-1").await.unwrap().recipe, before.recipe);

    let rows = stack.rows("piece").await;
    let refusals = [
        (
            "railing.create",
            create("R-1", &json!({})),
            "That element already exists.",
        ),
        (
            "railing.create",
            create("R-2", &json!({ "name": " " })),
            "A railing needs a name.",
        ),
        (
            "railing.create",
            create("R-2", &json!({ "path": [["0", "0"]] })),
            "A railing's path needs at least two points, not 1.",
        ),
        (
            "railing.create",
            create("R-2", &json!({ "category": "attic" })),
            "The rule set nz-f4-as1 has no barrier category \"attic\"; it has household-stair, household-edge, stair, seating, other.",
        ),
        (
            "railing.type",
            json!({ "element": "R-1", "type": { "balusters": { "spacing": "10 mm" } } }),
            "Balusters 20 mm deep along the path do not fit at a spacing of 10 mm.",
        ),
        (
            "railing.type",
            json!({ "element": "R-1" }),
            "This command needs a type, a category or an openings rule.",
        ),
        (
            "railing.path",
            json!({ "element": "R-1", "side": "left" }),
            "A sketched railing is moved by its path, `path`.",
        ),
        (
            "railing.stair",
            json!({ "element": "R-2", "stair": "R-1" }),
            "Balcony is not a stair.",
        ),
    ];
    for (cmd, args, words) in refusals {
        assert_eq!(stack.edit(cmd, args).await.unwrap_err(), words, "{cmd}");
    }
    assert_eq!(stack.rows("piece").await, rows, "nothing changed");

    stack
        .edit("railing.delete", json!({ "element": "R-1" }))
        .await
        .unwrap();
    assert!(stack.record("R-1").await.is_none());
}

/// A railing on a stair follows it: on the stair of 15 risers, both sides 50 mm in, each rail
/// along the pitch, 280 sqrt(277) = 4660.13 mm long. First moved to 2400 mm lays the stair out in
/// 13 risers and the railing with it, by regeneration alone, its record unchanged. A railing on a
/// stair flips to the stair's other side; on both sides it has none to flip to.
#[tokio::test]
async fn a_railing_on_a_stair_follows_it() {
    let stack = Stack::new().await;
    with_stair(&stack).await;
    stack
        .edit("railing.stair", json!({ "element": "R-1", "stair": "S-1" }))
        .await
        .unwrap();
    let view = stack.view().await;
    let railing = &view.railings[0];
    assert_eq!(
        (railing.stair.as_deref(), railing.side),
        (Some("S-1"), Some("both"))
    );
    let laid = railing.laid.as_ref().unwrap();
    assert_eq!(
        laid.rail_length.value,
        format!("length:q0:{}", 2 * 29_824_824)
    );
    assert_eq!(laid.posts, 4);
    let record = stack.record("R-1").await.unwrap();
    assert_eq!(record.props["@level"], "L-g");
    stack
        .edit(
            "level.elevation",
            json!({ "level": "L-1", "elevation": "2400 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("R-1").await.unwrap(), record);
    let moved = stack.view().await.railings[0]
        .laid
        .as_ref()
        .unwrap()
        .rail_length
        .value
        .clone();
    assert_ne!(moved, laid.rail_length.value);
    assert_eq!(stack.bounds("R-1").await.unwrap().1[2], 2400 + 1000);
    assert_eq!(
        stack
            .edit("railing.flip", json!({ "element": "R-1" }))
            .await
            .unwrap_err(),
        "A railing on both sides of a stair has no other side to flip to."
    );
    stack
        .edit(
            "railing.path",
            json!({ "element": "R-1", "side": "left", "inset": "60 mm" }),
        )
        .await
        .unwrap();
    stack
        .edit("railing.flip", json!({ "element": "R-1" }))
        .await
        .unwrap();
    assert_eq!(stack.view().await.railings[0].side, Some("right"));
    // Its stair deleted, it is a build failure naming it, and the view says why.
    stack
        .edit("stair.delete", json!({ "element": "S-1" }))
        .await
        .unwrap();
    assert_eq!(
        stack.bounds("R-1").await.unwrap_err(),
        "HostMissing: its stair S-1 is not in the model"
    );
    let view = stack.view().await;
    assert_eq!(
        view.railings[0].refusal.as_deref(),
        Some("its stair S-1 is not in the model")
    );
}

/// The generic commands do not go behind a railing's own.
#[tokio::test]
async fn the_generic_commands_are_refused_for_a_railing() {
    let stack = Stack::new().await;
    stack
        .edit("railing.create", create("R-1", &json!({})))
        .await
        .unwrap();
    let record = stack.record("R-1").await.unwrap();
    let made = stack
        .edit(
            "element.create",
            json!({ "element": "R-2", "kind": "railing", "name": "R", "recipe": record.recipe }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        made,
        "A railing is made with railing.create or railing.stair, which checks it as a railing."
    );
    let moved = stack
        .edit(
            "element.move",
            json!({ "element": "R-1", "placement": { "x": 1, "y": 0, "z": 0 } }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        moved,
        "Railing is moved with railing.path, which checks it as a railing."
    );
    let deleted = stack
        .edit("element.delete", json!({ "element": "R-1" }))
        .await
        .unwrap_err();
    assert_eq!(
        deleted,
        "Railing is deleted with railing.delete, which checks it as a railing."
    );
    let set = stack
        .edit("element.set", json!({ "element": "R-1", "name": "Other" }))
        .await
        .unwrap_err();
    assert!(
        set.starts_with("Railing is changed with railing.type"),
        "{set}"
    );
    stack
        .edit("element.claim", json!({ "element": "R-1" }))
        .await
        .unwrap();
    assert_eq!(stack.record("R-1").await.unwrap(), record);
}

/// Runs a command, undoes it and redoes it: the undo puts every piece back as it was before the
/// command, and the redo as it was after.
async fn undone_and_redone(stack: &Stack, cmd: &str, args: Value) {
    let before = stack.rows("piece").await;
    stack
        .edit(cmd, args)
        .await
        .unwrap_or_else(|why| panic!("{cmd}: {why}"));
    let after = stack.rows("piece").await;
    assert_ne!(before, after, "{cmd} changed something");
    assert_eq!(
        stack.state("anna").await.undo.map(|step| step.cmd),
        Some(cmd.to_owned())
    );
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, before, "{cmd} undone");
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, after, "{cmd} redone");
}

#[tokio::test]
async fn every_railing_command_is_undone_and_redone() {
    let stack = Stack::new().await;
    with_stair(&stack).await;
    for (cmd, args) in [
        ("railing.create", create("R-1", &json!({}))),
        (
            "railing.type",
            json!({ "element": "R-1", "type": { "top_rail": { "height": "1.1 m" } }, "category": "other" }),
        ),
        (
            "railing.path",
            json!({ "element": "R-1", "path": [["0", "0"], ["2 m", "0"]] }),
        ),
        ("railing.flip", json!({ "element": "R-1" })),
        ("railing.delete", json!({ "element": "R-1" })),
        (
            "railing.stair",
            json!({ "element": "R-2", "stair": "S-1", "side": "right" }),
        ),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
    assert!(stack.record("R-1").await.is_none());
    assert!(stack.record("R-2").await.is_some());
}

/// The page's view lists the rule set's categories and openings rules, and each railing.
#[tokio::test]
async fn the_view_lists_the_categories_and_every_railing() {
    let stack = Stack::new().await;
    let view = stack.view().await;
    assert_eq!(view.rules, "nz-f4-as1");
    assert!(view.rules_name.contains("F4/AS1"), "{}", view.rules_name);
    assert_eq!(
        view.categories
            .iter()
            .map(|c| (c.id.as_str(), c.level_least.shown.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("household-stair", "900 mm"),
            ("household-edge", "1000 mm"),
            ("stair", "1100 mm"),
            ("seating", "800 mm"),
            ("other", "1100 mm")
        ]
    );
    assert_eq!(
        view.openings
            .iter()
            .map(|o| (o.id.as_str(), o.by))
            .collect::<Vec<_>>(),
        vec![("children", "sphere"), ("no-children", "most")]
    );
    assert_eq!(view.railings.len(), 0);
    stack
        .edit(
            "railing.create",
            create("R-1", &json!({ "category": "other" })),
        )
        .await
        .unwrap();
    let view = stack.view().await;
    let railing = &view.railings[0];
    assert_eq!(
        (railing.category.as_str(), railing.openings.as_str()),
        ("other", "children")
    );
    assert_eq!(railing.height.shown, "1000 mm");
    assert_eq!(
        railing.laid.as_ref().unwrap().warnings[0].identity,
        "railing.low|R-1"
    );
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    with_stair(&stack).await;
    for (cmd, args) in [
        ("railing.create", create("R-1", &json!({}))),
        ("railing.stair", json!({ "element": "R-2", "stair": "S-1" })),
        (
            "railing.type",
            json!({ "element": "R-1", "type": { "balusters": { "justify": "start" } } }),
        ),
        (
            "level.elevation",
            json!({ "level": "L-1", "elevation": "2400 mm" }),
        ),
        ("edit.undo", json!({})),
        ("railing.delete", json!({ "element": "R-1" })),
        (
            "railing.create",
            create("R-1", &json!({ "path": [["0", "0"]] })),
        ),
    ] {
        let _ = stack.edit(cmd, args).await;
    }
    stack.regen.catch_up(&i("p")).await.unwrap();
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let head: u64 = read_row(&mut snapshot, &Scope::Project(i("p")).head())
        .await
        .unwrap()
        .unwrap();
    let entries = read_log(&mut snapshot, &Scope::Project(i("p")), 0, head)
        .await
        .unwrap();
    assert!(entries.iter().any(|entry| entry.failed.is_some()));

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
        let _ = again.run(edit).await;
    }
    again.regen.catch_up(&i("p")).await.unwrap();
    for table in ["record", "piece"] {
        assert_eq!(again.rows(table).await, stack.rows(table).await, "{table}");
    }
    assert_eq!(again.derived().await, stack.derived().await);
    let incremental = stack.derived().await;
    stack.regen.rebuild(&i("p")).await.unwrap();
    assert_eq!(stack.derived().await, incremental);
}
