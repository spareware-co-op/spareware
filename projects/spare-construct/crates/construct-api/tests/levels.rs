//! Levels through the edit log, on the test store: the level commands and their refusals, a new
//! element tied by a level's ID, old records on the levels their integers imply, a moved level
//! moving what is tied to it without a record being written, undo and redo of every level
//! command, and the log replayed from empty.

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

use construct_api::cloud::undo::{self, CANNOT, State};
use construct_api::cloud::{Access, ProjectCommands};
use construct_categories::LEVELS;
use construct_levels::{Level, Levels};
use construct_model::element::{Element, Placement};
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::{Place, list};
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_params::Length;
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use kit_core::Id;
use kit_ops::{Decider, Edit, Question, sha256_hex};
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

    async fn new() -> Self {
        let stack = Self::bare().await;
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
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

    /// Lets every claim run out.
    fn later(&self) {
        self.clock.0.fetch_add(CLAIM_MS + 1, Ordering::SeqCst);
    }

    async fn record(&self, id: &str) -> Element {
        let mut snapshot = self.store.snapshot().await.unwrap();
        construct_regen::parameters::read_element(&mut snapshot, &i("p"), &i(DOCUMENT), &i(id))
            .await
            .unwrap()
            .unwrap()
            .1
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

    async fn levels(&self) -> Levels {
        let mut snapshot = self.store.snapshot().await.unwrap();
        construct_regen::levels::read_levels(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap()
            .unwrap()
            .0
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

    /// How high a built shape is, and how high its base sits.
    async fn heights(&self, id: &str) -> (i64, Option<String>) {
        let built = self.built(id).await;
        match built.result {
            BuiltResult::Shape { bounds, .. } => (bounds.max[2] - bounds.min[2], built.elevation),
            failed @ BuiltResult::Failed { .. } => panic!("{id}: {failed:?}"),
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

    async fn in_cell(&self, level: &str) -> Vec<String> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let prefix = list::cell(&i("p"), &i(DOCUMENT), &i(level), 0, 0);
        snapshot
            .scan(&prefix)
            .await
            .unwrap()
            .into_iter()
            .filter_map(|(key, _)| match Place::parse(&key) {
                Some(Place::Area { element, .. }) => Some(element.to_string()),
                _ => None,
            })
            .collect()
    }

    async fn state(&self, person: &str) -> State {
        let mut snapshot = self.store.snapshot().await.unwrap();
        undo::state(&mut snapshot, &i("p"), &i(DOCUMENT), &i(person))
            .await
            .unwrap()
    }

    /// Writes a record and its pointer as a document written before levels were elements holds
    /// them, outside the log.
    async fn put_old(&self, record: &Element) {
        let bytes = to_row(record).unwrap();
        let hash = sha256_hex(&bytes);
        let mut txn = self.store.begin().await.unwrap();
        let place = kit_store::Place::Record {
            project: i("p"),
            hash: i(&hash),
        };
        txn.put(place.key(), bytes).await.unwrap();
        let pointer = kit_store::Place::Piece {
            project: i("p"),
            document: i(DOCUMENT),
            piece: record.id.clone(),
        };
        txn.put(pointer.key(), to_row(&hash.as_str()).unwrap())
            .await
            .unwrap();
        txn.commit().await.unwrap();
    }
}

fn recipe(op: &str, args: Value) -> Value {
    serde_json::to_value(Recipe::construct(op, serde_json::from_value(args).unwrap())).unwrap()
}

/// A 3000 mm wall along x, 3000 mm high, at `x`, on `level` (an ID, an integer, or nothing).
fn wall(id: &str, level: &Value, x: i64) -> Value {
    let args = json!({ "length": 3000, "thickness": 200, "height": 3000 });
    json!({ "element": id, "kind": "wall", "level": level,
            "placement": { "x": x, "y": 0, "z": 0 }, "recipe": recipe("wall.straight", args) })
}

fn level(id: &str, name: &str, elevation: &str) -> Value {
    json!({ "level": id, "name": name, "elevation": elevation })
}

fn mm(mm: i32) -> String {
    construct_params::Value::Length(Length::from_mm(mm)).stored()
}

/// Ground at 0 and First at 3.2 m.
async fn two_levels(stack: &Stack) {
    stack
        .edit("level.create", level("L-g", "Ground", "0"))
        .await
        .unwrap();
    stack
        .edit("level.create", level("L-1", "First", "3.2 m"))
        .await
        .unwrap();
}

#[tokio::test]
async fn levels_are_made_renamed_moved_flagged_and_deleted_through_the_log() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    let first = stack.record("L-1").await;
    assert_eq!(first.kind, "level");
    assert_eq!(first.props["@category"], LEVELS.0.to_string());
    assert_eq!(
        Level::from_record(&first).unwrap(),
        Level::new(i("L-1"), "First", Length::from_mm(3200), true).unwrap()
    );

    let refusals = [
        (
            "level.create",
            level("L-2", "ground", "6 m"),
            "There is already a level named \"ground\" in this model.",
        ),
        (
            "level.create",
            json!({ "level": "L-2", "name": "Mezzanine", "elevation": "3200" }),
            "Mezzanine can't be a storey at 3200 mm: First is already the storey at that elevation.",
        ),
        (
            "level.create",
            level("L-g", "Again", "9 m"),
            "That element already exists.",
        ),
        (
            "level.create",
            level("L-2", "Roof", "6 m²"),
            "Elevation takes a length, and \"m²\" is a unit of an area",
        ),
        (
            "level.rename",
            json!({ "level": "L-9", "name": "Roof" }),
            "There is no level L-9 in this model.",
        ),
        (
            "level.rename",
            json!({ "level": "L-1", "name": "  " }),
            "A level needs a name.",
        ),
    ];
    for (cmd, args, why) in refusals {
        let refused = stack.edit(cmd, args.clone()).await.unwrap_err();
        assert!(refused.starts_with(why), "{cmd} {args}: {refused}");
    }

    // Two levels at one elevation: allowed when only one of them is a storey.
    stack
        .edit(
            "level.create",
            json!({ "level": "L-t", "name": "Top of slab", "elevation": "3200", "storey": false }),
        )
        .await
        .unwrap();
    let refused = stack
        .edit("level.storey", json!({ "level": "L-t", "storey": true }))
        .await
        .unwrap_err();
    assert!(refused.contains("First is already the storey"), "{refused}");
    stack
        .edit(
            "level.rename",
            json!({ "level": "L-1", "name": "Level one" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "level.elevation",
            json!({ "level": "L-1", "elevation": "3.6 m" }),
        )
        .await
        .unwrap();
    stack
        .edit("level.storey", json!({ "level": "L-t", "storey": true }))
        .await
        .unwrap();
    let names: Vec<String> = stack
        .levels()
        .await
        .ordered()
        .iter()
        .map(|l| format!("{} {}", l.name(), l.elevation()))
        .collect();
    assert_eq!(
        names,
        ["Ground 0 mm", "Top of slab 3200 mm", "Level one 3600 mm"]
    );

    // A level with elements on it stays, and says how many.
    for (id, x) in [("W-1", 0), ("W-2", 4000)] {
        stack
            .edit("element.create", wall(id, &json!("L-1"), x))
            .await
            .unwrap();
    }
    let refused = stack
        .edit("level.delete", json!({ "level": "L-1" }))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "Level one can't be deleted: 2 elements are tied to it."
    );
    stack
        .edit("level.delete", json!({ "level": "L-t" }))
        .await
        .unwrap();
    assert!(!stack.exists("L-t").await);
    let refused = stack
        .edit("level.delete", json!({ "level": "L-t" }))
        .await
        .unwrap_err();
    assert_eq!(refused, "There is no level L-t in this model.");

    // A level is not an element: the element commands leave it alone, and refuse to make one.
    let refused = stack
        .edit("element.set", json!({ "element": "L-g", "name": "x" }))
        .await
        .unwrap_err();
    assert!(refused.contains("levels"), "{refused}");
    let made = json!({ "element": "L-x", "kind": "level", "placement": { "x": 0, "y": 0, "z": 0 },
                       "recipe": recipe("level", json!({})) });
    let refused = stack.edit("element.create", made).await.unwrap_err();
    assert_eq!(
        refused,
        "Levels are made and changed by the level commands."
    );
}

#[tokio::test]
async fn a_new_element_is_tied_by_the_levels_id_and_no_integer_is_written() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("element.create", wall("W-1", &json!("L-1"), 0))
        .await
        .unwrap();
    let record = stack.record("W-1").await;
    assert_eq!(record.level, 0);
    assert_eq!(record.props["@level"], "L-1");
    assert_eq!(record.props["@level.offset"], mm(0));
    let built = stack.built("W-1").await;
    assert_eq!(built.level, Some(i("L-1")));
    assert_eq!(built.elevation, Some(mm(3200)));
    assert_eq!(stack.in_cell("L-1").await, ["W-1"]);

    // An old page's integer names the level it implies; nothing names no level.
    stack
        .edit("element.create", wall("W-2", &json!(2), 0))
        .await
        .unwrap();
    let old = stack.record("W-2").await;
    assert_eq!((old.level, old.props["@level"].as_str()), (0, "2"));
    assert_eq!(stack.built("W-2").await.elevation, Some(mm(3000)));
    stack
        .edit("element.create", wall("W-3", &Value::Null, 0))
        .await
        .unwrap();
    let none = stack.record("W-3").await;
    assert!(!none.props.contains_key("@level"));
    let built = stack.built("W-3").await;
    assert_eq!((built.level, built.elevation), (None, None));
    assert!(matches!(built.result, BuiltResult::Shape { .. }));

    for (level, why) in [
        (json!("L-x"), "There is no level L-x in this model."),
        (json!("W-1"), "There is no level W-1 in this model."),
        (json!(1.5), "1.5 is not a level."),
        (json!({}), "A level is named by its ID."),
    ] {
        let refused = stack
            .edit("element.create", wall("W-9", &level, 0))
            .await
            .unwrap_err();
        assert_eq!(refused, why);
    }
    // A tie is set only by the level commands.
    let set = json!({ "element": "W-1", "props": { "@level": "L-g" } });
    assert!(stack.edit("element.set", set).await.is_err());

    // level.tie moves it to another level with an offset, and clears an old integer.
    stack
        .edit(
            "level.tie",
            json!({ "element": "W-2", "level": "L-g", "offset": "-150" }),
        )
        .await
        .unwrap();
    let tied = stack.record("W-2").await;
    assert_eq!(tied.props["@level"], "L-g");
    assert_eq!(tied.props["@level.offset"], mm(-150));
    assert_eq!(stack.built("W-2").await.elevation, Some(mm(-150)));
    assert_eq!(stack.in_cell("2").await, Vec::<String>::new());
    let refused = stack
        .edit("level.tie", json!({ "element": "W-2", "level": "L-x" }))
        .await
        .unwrap_err();
    assert_eq!(refused, "There is no level L-x in this model.");
    let refused = stack
        .edit("level.tie", json!({ "element": "L-1", "level": "L-g" }))
        .await
        .unwrap_err();
    assert!(refused.contains("not tied to levels"), "{refused}");
}

#[tokio::test]
async fn old_records_stand_on_the_levels_their_integers_imply() {
    let stack = Stack::new().await;
    // A wall of a document written when levels were integers: level 2, no tie.
    let args = json!({ "length": 3000, "thickness": 200, "height": 3000 });
    let old = Element {
        id: i("W-old"),
        document: i(DOCUMENT),
        kind: "wall".to_owned(),
        name: "Old wall".to_owned(),
        level: 2,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: serde_json::from_value(recipe("wall.straight", args)).unwrap(),
        props: BTreeMap::new(),
        references: Vec::new(),
    };
    stack.put_old(&old).await;
    // An edit of it keeps its integer and adds no tie, so the log holds an old record.
    stack
        .edit(
            "element.set",
            json!({ "element": "W-old", "name": "Old wall A" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("W-old").await.level, 2);
    let built = stack.built("W-old").await;
    assert_eq!(built.level, Some(i("2")));
    assert_eq!(built.elevation, Some(mm(3000)));
    assert_eq!(stack.in_cell("2").await, ["W-old"]);

    // The implied level is listed, and deleting it says what is on it.
    let levels = stack.levels().await;
    assert!(levels.is_implied(&i("2")));
    assert_eq!(levels.get(&i("2")).unwrap().name(), "Level 2");
    let refused = stack
        .edit("level.delete", json!({ "level": "2" }))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "Level 2 can't be deleted: 1 element is tied to it."
    );

    // Moving it writes its record, and the old wall moves with it; the wall's record stays.
    let wall = stack.record("W-old").await;
    stack
        .edit(
            "level.elevation",
            json!({ "level": "2", "elevation": "3.5 m" }),
        )
        .await
        .unwrap();
    assert!(stack.exists("2").await);
    assert!(!stack.levels().await.is_implied(&i("2")));
    assert_eq!(stack.record("W-old").await, wall);
    assert_eq!(stack.built("W-old").await.elevation, Some(mm(3500)));
    // Undoing that takes the record away again, and the implied level returns.
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert!(!stack.exists("2").await);
    assert_eq!(stack.built("W-old").await.elevation, Some(mm(3000)));
}

#[tokio::test]
async fn moving_a_level_moves_what_is_tied_to_it_and_writes_no_element() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("level.create", level("L-r", "Roof", "6.4 m"))
        .await
        .unwrap();
    // W-a stands on Ground with its top tied to First, 200 below it; W-b stands on First; W-c
    // stands on Ground, its own height; W-d is on no level.
    for (id, on, x) in [
        ("W-a", json!("L-g"), 0),
        ("W-b", json!("L-1"), 4000),
        ("W-c", json!("L-g"), 8000),
        ("W-d", Value::Null, 12000),
    ] {
        stack
            .edit("element.create", wall(id, &on, x))
            .await
            .unwrap();
    }
    stack
        .edit(
            "level.top",
            json!({ "element": "W-a", "level": "L-1", "offset": "-200" }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack.record("W-a").await.recipe.args["height"],
        json!({ "tie": "top" })
    );
    assert_eq!(stack.heights("W-a").await, (3000, Some(mm(0))));
    assert_eq!(stack.heights("W-b").await, (3000, Some(mm(3200))));

    let records = stack.rows("record").await;
    let pieces = stack.rows("piece").await;
    let before = [stack.built("W-c").await, stack.built("W-d").await];
    stack
        .edit(
            "level.elevation",
            json!({ "level": "L-1", "elevation": "3.6 m" }),
        )
        .await
        .unwrap();
    // The tied walls' shapes moved: W-a is 400 taller, W-b's base 400 higher.
    assert_eq!(stack.heights("W-a").await, (3400, Some(mm(0))));
    assert_eq!(stack.heights("W-b").await, (3000, Some(mm(3600))));
    // The untied ones did not, not by a byte.
    assert_eq!([stack.built("W-c").await, stack.built("W-d").await], before);
    // No element's record was written: only the level's, and its pointer.
    let changed: Vec<String> = stack
        .rows("piece")
        .await
        .into_iter()
        .filter(|(key, hash)| pieces.get(key) != Some(hash))
        .map(|(key, _)| key)
        .collect();
    assert_eq!(changed, ["p:p/piece/doc/L-1"]);
    assert_eq!(stack.rows("record").await.len(), records.len() + 1);

    // A top can't go below its base, only a wall's top is tied, and untying keeps the height.
    let refused = stack
        .edit("level.top", json!({ "element": "W-b", "level": "L-g" }))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "W-b's top would be -3600 mm from its base: a top is above its base."
    );
    let door = json!({ "element": "D-1", "kind": "door", "level": "L-g",
                       "placement": { "x": 0, "y": 0, "z": 0 },
                       "recipe": recipe("door.single",
                                        json!({ "width": 900, "thickness": 40, "height": 2100 })) });
    stack.edit("element.create", door).await.unwrap();
    let refused = stack
        .edit("level.top", json!({ "element": "D-1", "level": "L-1" }))
        .await
        .unwrap_err();
    assert_eq!(refused, "Only a wall's top is tied to a level.");
    stack
        .edit("level.top", json!({ "element": "W-a", "level": null }))
        .await
        .unwrap();
    let untied = stack.record("W-a").await;
    assert_eq!(untied.recipe.args["height"], json!(3400));
    assert!(!untied.props.contains_key("@top.level"));
    let refused = stack
        .edit("level.top", json!({ "element": "W-a", "level": null }))
        .await
        .unwrap_err();
    assert_eq!(refused, "W-a's top is not tied to a level.");
    // Moving First again leaves W-a, now untied at the top, as it is.
    stack.later();
    stack
        .edit(
            "level.elevation",
            json!({ "level": "L-1", "elevation": "4 m" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.heights("W-a").await, (3400, Some(mm(0))));
    assert_eq!(stack.heights("W-b").await, (3000, Some(mm(4000))));

    // Built from empty, the derived rows are the same bytes.
    let incremental = stack.derived().await;
    stack.regen.rebuild(&i("p")).await.unwrap();
    assert_eq!(stack.derived().await, incremental);
    assert_eq!(stack.regen.verify(&i("p")).await.unwrap(), 5);
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
async fn every_level_command_is_undone_and_redone() {
    let stack = Stack::new().await;
    undone_and_redone(&stack, "level.create", level("L-g", "Ground", "0")).await;
    undone_and_redone(&stack, "level.create", level("L-1", "First", "3 m")).await;
    stack
        .edit("element.create", wall("W-1", &json!("L-g"), 0))
        .await
        .unwrap();
    for (cmd, args) in [
        ("level.rename", json!({ "level": "L-1", "name": "Upper" })),
        (
            "level.elevation",
            json!({ "level": "L-1", "elevation": "3.3 m" }),
        ),
        ("level.storey", json!({ "level": "L-1", "storey": false })),
        (
            "level.tie",
            json!({ "element": "W-1", "level": "L-1", "offset": "50" }),
        ),
        (
            "level.top",
            json!({ "element": "W-1", "level": "L-g", "offset": "6 m" }),
        ),
        ("level.top", json!({ "element": "W-1", "level": null })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
    stack
        .edit("level.tie", json!({ "element": "W-1", "level": "L-g" }))
        .await
        .unwrap();
    undone_and_redone(&stack, "level.delete", json!({ "level": "L-1" })).await;
    // An implied level changed writes its record; the undo takes it away again.
    stack
        .edit("element.create", wall("W-2", &json!(3), 0))
        .await
        .unwrap();
    undone_and_redone(
        &stack,
        "level.rename",
        json!({ "level": "3", "name": "Third" }),
    )
    .await;
}

#[tokio::test]
async fn an_undo_may_not_leave_an_element_tied_to_a_level_that_is_gone() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("level.create", level("L-2", "Second", "6 m"))
        .await
        .unwrap();
    stack.later();
    // Ben ties his wall to Anna's new level; her undo of making it would leave his wall on
    // nothing, so it is turned back.
    stack
        .edit_as("ben", "element.create", wall("W-b", &json!("L-2"), 0))
        .await
        .unwrap();
    let refused = stack.edit("edit.undo", json!({})).await.unwrap_err();
    assert_eq!(
        refused,
        format!("{CANNOT}W-b would be tied to level L-2, which is no longer in this model.")
    );
    assert!(stack.exists("L-2").await);

    // Anna moves her wall from Ground to First, then to Second; Ben moves his to First and
    // deletes Ground. Her undo of the second move is fine; of the first, it would put her wall
    // back on Ground, which is gone.
    stack
        .edit("element.create", wall("W-a", &json!("L-g"), 4000))
        .await
        .unwrap();
    stack
        .edit("level.tie", json!({ "element": "W-a", "level": "L-1" }))
        .await
        .unwrap();
    stack
        .edit("level.tie", json!({ "element": "W-a", "level": "L-2" }))
        .await
        .unwrap();
    stack.later();
    stack
        .edit_as("ben", "level.delete", json!({ "level": "L-g" }))
        .await
        .unwrap();
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.record("W-a").await.props["@level"], "L-1");
    let refused = stack.edit("edit.undo", json!({})).await.unwrap_err();
    assert_eq!(
        refused,
        format!("{CANNOT}W-a would be tied to level L-g, which is no longer in this model.")
    );
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    for (cmd, args) in [
        ("element.create", wall("W-1", &json!("L-g"), 0)),
        ("element.create", wall("W-2", &json!(2), 4000)),
        ("level.top", json!({ "element": "W-1", "level": "L-1" })),
        (
            "level.elevation",
            json!({ "level": "L-1", "elevation": "3.4 m" }),
        ),
        ("level.rename", json!({ "level": "2", "name": "Upper" })),
        ("level.create", level("L-x", "ground", "0")),
        (
            "level.tie",
            json!({ "element": "W-2", "level": "L-1", "offset": "10" }),
        ),
        ("edit.undo", json!({})),
        ("level.storey", json!({ "level": "L-g", "storey": false })),
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
    let mut replayed = again.store.snapshot().await.unwrap();
    let after = read_log(&mut replayed, &Scope::Project(i("p")), 0, head)
        .await
        .unwrap();
    assert_eq!(after, entries);
}
