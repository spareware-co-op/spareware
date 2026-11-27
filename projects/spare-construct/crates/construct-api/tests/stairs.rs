//! Stairs through the edit log, on the test store: a stair made between two levels and laid out by
//! New Zealand's rule set; its width, direction and start, and its runs and landings changed; each
//! limit refused in construct-stairs' words, naming its clause and, for a height no count of
//! risers lays out, the nearest heights that work; a level moved laying the stair out again by
//! regeneration alone; the generic commands refused where a stair's own exist; undo and redo of
//! every stair command; the page's view of a document's stairs; and the log replayed from empty
//! to the same bytes.

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

use construct_api::cloud::stairs::read_stairs;
use construct_api::cloud::undo::{self, State};
use construct_api::cloud::{Access, ProjectCommands};
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_regen::parameters as records;
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_stairs::{Direction, stair_of};
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

    async fn view(&self) -> construct_api::cloud::stairs::StairsView {
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_stairs(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap()
    }
}

/// `stair.create`'s arguments: stair `id` from Ground to First, a common stairway from (0, 0)
/// towards +x, with `more` added.
fn create(id: &str, more: &Value) -> Value {
    let mut args = json!({ "element": id, "level": "L-g", "top": "L-1", "start": ["0", "0"] });
    if let (Some(args), Some(more)) = (args.as_object_mut(), more.as_object()) {
        for (key, value) in more {
            args.insert(key.clone(), value.clone());
        }
    }
    args
}

/// A stair is made between two levels and changed by its own commands. From Ground to First, 2700
/// mm, a common stairway needs a landing (Table 7: 2500 mm at most between landings), so one run
/// is refused in those words; two runs with a 1000 mm landing take 15 risers of 180 mm, 8 and 7,
/// on 280 mm goings. It is made wider, turned and moved, given a quarter turn on its landing and a
/// longer going, and deleted; every refusal changes nothing.
#[tokio::test]
async fn a_stair_is_made_changed_and_deleted_by_its_own_commands() {
    let stack = Stack::new().await;
    let refused = stack
        .edit("stair.create", create("S-1", &json!({})))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "Run 1 rises 2700 mm on a common stairway: the most a stair may rise between landings is 2500 mm (D1/AS1 4.3.2 and Table 7). Add a landing."
    );
    assert!(stack.record("S-1").await.is_none());
    let two_runs = json!({
        "name": "Main stair",
        "runs": [{}, {}],
        "landings": [{ "length": "1000 mm", "turn": "straight" }],
    });
    stack
        .edit("stair.create", create("S-1", &two_runs))
        .await
        .unwrap();
    let record = stack.record("S-1").await.unwrap();
    assert_eq!(record.name, "Main stair");
    assert_eq!(record.kind, "stair");
    let view = stack.view().await;
    let layout = view.stairs[0].layout.as_ref().unwrap();
    assert_eq!(layout.risers, 15);
    assert_eq!(layout.riser_height.shown, "180 mm");
    assert_eq!(layout.going.shown, "280 mm");
    assert_eq!(
        layout.runs.iter().map(|r| r.risers).collect::<Vec<_>>(),
        vec![8, 7]
    );
    assert_eq!(layout.total_run.shown, "4640 mm");
    // The first run, the landing and the second: 1960 + 1000 + 1680 mm.
    assert_eq!(stack.bounds("S-1").await.unwrap().1[0], 4640);

    stack
        .edit(
            "stair.width",
            json!({ "element": "S-1", "width": "1200 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.bounds("S-1").await.unwrap().1[1], 1200);
    let narrow = stack
        .edit(
            "stair.width",
            json!({ "element": "S-1", "width": "850 mm" }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        narrow,
        "A width of 850 mm is too narrow for a common stairway: the narrowest it may be is 900 mm (D1/AS1 4.2.1)."
    );
    stack
        .edit(
            "stair.direction",
            json!({ "element": "S-1", "direction": "+y", "start": ["2 m", "1 m"] }),
        )
        .await
        .unwrap();
    let stair = stair_of(&stack.record("S-1").await.unwrap().recipe).unwrap();
    assert_eq!(stair.direction(), Direction::PlusY);
    let (min, max) = stack.bounds("S-1").await.unwrap();
    // Going up +y from (2000, 1000), lying to its left (-x), 1200 mm wide.
    assert_eq!((min[0], max[0], min[1], max[1]), (800, 2000, 1000, 5640));
    assert_eq!(
        stack
            .edit(
                "stair.direction",
                json!({ "element": "S-1", "direction": "up" })
            )
            .await
            .unwrap_err(),
        "A stair runs +x, -x, +y or -y, not \"up\"."
    );
    stack
        .edit(
            "stair.runs",
            json!({ "element": "S-1", "runs": [{ "risers": 9 }, {}],
                    "landings": [{ "length": "1200 mm", "turn": "left" }], "going": "300 mm" }),
        )
        .await
        .unwrap();
    let view = stack.view().await;
    let layout = view.stairs[0].layout.as_ref().unwrap();
    assert_eq!(layout.going.shown, "300 mm");
    assert_eq!(
        layout.runs.iter().map(|r| r.risers).collect::<Vec<_>>(),
        vec![9, 6]
    );
    assert_eq!(layout.landings[0].turn, "left");
    let short = stack
        .edit(
            "stair.runs",
            json!({ "element": "S-1", "runs": [{}, {}],
                    "landings": [{ "length": "800 mm", "turn": "straight" }] }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        short,
        "Landing 1 is 800 mm long: the shortest a landing may be is 900 mm (D1/AS1 4.3.4)."
    );
    let bad_turn = stack
        .edit(
            "stair.runs",
            json!({ "element": "S-1", "runs": [{}, {}],
                    "landings": [{ "length": "1 m", "turn": "around" }] }),
        )
        .await
        .unwrap_err();
    assert!(
        bad_turn.starts_with("A landing turns straight, left"),
        "{bad_turn}"
    );

    stack
        .edit("stair.delete", json!({ "element": "S-1" }))
        .await
        .unwrap();
    assert!(stack.record("S-1").await.is_none());
    assert_eq!(
        stack
            .edit("stair.width", json!({ "element": "S-1", "width": "1 m" }))
            .await
            .unwrap_err(),
        "That element no longer exists."
    );
}

/// What a stair is made with is checked: its levels, its top above its base, and a height no
/// count of risers lays out, refused naming the nearest heights that work.
#[tokio::test]
async fn a_stair_is_refused_in_words() {
    let stack = Stack::new().await;
    let args = json!({ "level": "L-2", "name": "Low", "elevation": "400 mm" });
    stack.edit("level.create", args).await.unwrap();
    let low = stack
        .edit("stair.create", create("S-1", &json!({ "top": "L-2" })))
        .await
        .unwrap_err();
    assert_eq!(
        low,
        "No count of risers lays out a rise of 400 mm on a common stairway with a going of 280 mm and at least 2 risers, two to each run: the lowest a riser may be is 150 mm (D1/AS1 4.1.1 and Figure 11 (the least riser height it charts)), and the highest a riser may be is 190 mm (D1/AS1 4.1.1 and Table 6); the nearest floor-to-floor heights that work are 380 mm and 450 mm."
    );
    let down = stack
        .edit(
            "stair.create",
            create("S-1", &json!({ "level": "L-1", "top": "L-g" })),
        )
        .await
        .unwrap_err();
    assert!(down.contains("a top is above its base"), "{down}");
    let missing = stack
        .edit("stair.create", create("S-1", &json!({ "top": "L-9" })))
        .await
        .unwrap_err();
    assert!(missing.contains("L-9"), "{missing}");
    let grand = stack
        .edit(
            "stair.create",
            create(
                "S-1",
                &json!({ "category": "grand", "runs": [{}, {}],
                                  "landings": [{ "length": "1 m", "turn": "straight" }] }),
            ),
        )
        .await
        .unwrap_err();
    assert!(
        grand.starts_with("The rule set nz-d1-as1 has no stairway category"),
        "{grand}"
    );
    let one = stack
        .edit(
            "stair.create",
            create("S-1", &json!({ "runs": [{ "risers": 1 }] })),
        )
        .await
        .unwrap_err();
    assert_eq!(
        one,
        "Run 1 is given 1 riser(s): a run takes at least two, so that it has a tread."
    );
    assert!(stack.record("S-1").await.is_none());
}

/// A stair follows its levels: First moved to 2400 mm lays it out again in 13 risers by
/// regeneration alone, its record unchanged; moved to 6 m it is refused in its built row (each
/// run would rise past Table 7's 2500 mm), and the view says why.
#[tokio::test]
async fn a_moved_level_lays_the_stair_out_again() {
    let stack = Stack::new().await;
    let args = create(
        "S-1",
        &json!({ "runs": [{}, {}], "landings": [{ "length": "1 m", "turn": "back-left" }] }),
    );
    stack.edit("stair.create", args).await.unwrap();
    let record = stack.record("S-1").await.unwrap();
    stack
        .edit(
            "level.elevation",
            json!({ "level": "L-1", "elevation": "2400 mm" }),
        )
        .await
        .unwrap();
    assert!(stack.bounds("S-1").await.is_ok());
    assert_eq!(stack.record("S-1").await.unwrap(), record);
    let view = stack.view().await;
    assert_eq!(view.stairs[0].layout.as_ref().unwrap().risers, 13);
    stack
        .edit(
            "level.elevation",
            json!({ "level": "L-1", "elevation": "6 m" }),
        )
        .await
        .unwrap();
    let failed = stack.bounds("S-1").await.unwrap_err();
    assert!(
        failed.starts_with("BadStair: Run 1 rises 3000 mm"),
        "{failed}"
    );
    let view = stack.view().await;
    assert!(view.stairs[0].layout.is_none());
    assert!(
        view.stairs[0]
            .refusal
            .as_deref()
            .unwrap()
            .starts_with("Run 1 rises 3000 mm")
    );
}

/// The generic commands do not go behind a stair's own: making one with `element.create`, and
/// changing, moving or deleting one with `element.set`, `element.move` or `element.delete`, are
/// refused naming the command to use; picking one is allowed.
#[tokio::test]
async fn the_generic_commands_are_refused_for_a_stair() {
    let stack = Stack::new().await;
    let args = create(
        "S-1",
        &json!({ "runs": [{}, {}], "landings": [{ "length": "1 m", "turn": "straight" }] }),
    );
    stack.edit("stair.create", args).await.unwrap();
    let record = stack.record("S-1").await.unwrap();
    let made = stack
        .edit(
            "element.create",
            json!({ "element": "S-2", "kind": "stair", "name": "S", "recipe": record.recipe }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        made,
        "A stair is made with stair.create, which checks it as a stair."
    );
    let moved = stack
        .edit(
            "element.move",
            json!({ "element": "S-1", "placement": { "x": 1, "y": 0, "z": 0 } }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        moved,
        "Stair is moved with stair.direction, which checks it as a stair."
    );
    let deleted = stack
        .edit("element.delete", json!({ "element": "S-1" }))
        .await
        .unwrap_err();
    assert_eq!(
        deleted,
        "Stair is deleted with stair.delete, which checks it as a stair."
    );
    let set = stack
        .edit("element.set", json!({ "element": "S-1", "name": "Other" }))
        .await
        .unwrap_err();
    assert!(
        set.starts_with("Stair is changed with stair.width"),
        "{set}"
    );
    stack
        .edit("element.claim", json!({ "element": "S-1" }))
        .await
        .unwrap();
    assert_eq!(stack.record("S-1").await.unwrap(), record);
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
async fn every_stair_command_is_undone_and_redone() {
    let stack = Stack::new().await;
    for (cmd, args) in [
        (
            "stair.create",
            create(
                "S-1",
                &json!({ "runs": [{}, {}], "landings": [{ "length": "1 m", "turn": "straight" }] }),
            ),
        ),
        (
            "stair.width",
            json!({ "element": "S-1", "width": "1100 mm" }),
        ),
        (
            "stair.direction",
            json!({ "element": "S-1", "direction": "-x" }),
        ),
        (
            "stair.runs",
            json!({ "element": "S-1", "runs": [{}, {}],
                    "landings": [{ "length": "1100 mm", "turn": "right" }] }),
        ),
        ("stair.delete", json!({ "element": "S-1" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
    assert!(stack.record("S-1").await.is_none());
}

/// The page's view lists the rule set's categories, and each stair with its layout.
#[tokio::test]
async fn the_view_lists_the_categories_and_every_stair() {
    let stack = Stack::new().await;
    let view = stack.view().await;
    assert_eq!(view.rules, "nz-d1-as1");
    assert!(view.rules_name.contains("D1/AS1"), "{}", view.rules_name);
    assert_eq!(
        view.categories
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "accessible",
            "common",
            "main-private",
            "secondary-private",
            "minor-private",
            "service"
        ]
    );
    assert_eq!(view.stairs.len(), 0);
    let args = create(
        "S-1",
        &json!({ "category": "main-private", "name": "Back stair" }),
    );
    stack.edit("stair.create", args).await.unwrap();
    let view = stack.view().await;
    let stair = &view.stairs[0];
    assert_eq!(
        (stair.id.as_str(), stair.name.as_str()),
        ("S-1", "Back stair")
    );
    assert_eq!(stair.category, "main-private");
    assert_eq!(stair.direction, "+x");
    assert_eq!(stair.width.shown, "1000 mm");
    let layout = stair.layout.as_ref().unwrap();
    assert_eq!((layout.risers, layout.treads), (15, 14));
    assert_eq!(layout.rise.value, "length:q0:17280000");
    assert_eq!(layout.lowest_riser, layout.riser_height);
    assert_eq!(stair.refusal, None);
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    for (cmd, args) in [
        (
            "stair.create",
            create(
                "S-1",
                &json!({ "runs": [{}, {}], "landings": [{ "length": "1 m", "turn": "left" }] }),
            ),
        ),
        ("stair.create", create("S-2", &json!({}))),
        (
            "stair.width",
            json!({ "element": "S-1", "width": "1200 mm" }),
        ),
        (
            "level.elevation",
            json!({ "level": "L-1", "elevation": "2400 mm" }),
        ),
        ("edit.undo", json!({})),
        ("stair.delete", json!({ "element": "S-1" })),
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
