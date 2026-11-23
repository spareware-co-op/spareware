//! Rooms through the edit log, on the test store: a room placed at a point inside four walls, with
//! a name, a number (the next free one offered) and an upper limit; renamed, renumbered (a number
//! already used refused in words), moved and given another upper limit; deleted; what the search
//! refuses, refused in its own words; "not enclosed" and "redundant" kept as outcomes, never
//! refusals; a wall changed rebuilding the room round it, by regeneration alone; the generic
//! commands refused where a room's own exist; undo and redo of every room command; and the log
//! replayed from empty to the same bytes.

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

use construct_api::cloud::undo::{self, State};
use construct_api::cloud::{Access, ProjectCommands};
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_params::Area;
use construct_regen::parameters as records;
use construct_regen::parameters::At;
use construct_regen::regenerator::Regenerator;
use construct_regen::rooms::found_at;
use construct_regen::stand_in::Boxes;
use construct_rooms::{Outcome, number_of};
use construct_walls::default_walls;
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
const PARTITION: &str = "wall-partition-stud-90";

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
}

/// Ground at 0 and First at 3 m.
async fn two_levels(stack: &Stack) {
    for (id, name, elevation) in [("L-g", "Ground", "0"), ("L-1", "First", "3 m")] {
        let args = json!({ "level": id, "name": name, "elevation": elevation });
        stack.edit("level.create", args).await.unwrap();
    }
}

/// The default partition wall type, as `wall.type.create` takes it.
fn partition_type() -> Value {
    let walls = default_walls().unwrap();
    let wall = walls.iter().find(|wall| wall.id == PARTITION).unwrap();
    let layers: Vec<Value> = wall
        .assembly
        .layers()
        .iter()
        .map(|layer| {
            json!({ "function": layer.function, "thickness": layer.thickness.to_string(),
                    "material": layer.material.as_str(), "wraps": layer.wraps })
        })
        .collect();
    let assembly = json!({
        "core": {
            "exterior": wall.assembly.boundary(construct_walls::Side::Exterior),
            "interior": wall.assembly.boundary(construct_walls::Side::Interior),
        },
        "layers": layers,
    });
    json!({ "type": PARTITION, "name": wall.name, "assembly": assembly })
}

/// A wall of the partition type along its centre line, on Ground, its top on First.
async fn wall(stack: &Stack, id: &str, start: [&str; 2], end: [&str; 2]) {
    let args = json!({ "element": id, "type": PARTITION, "start": start, "end": end,
                       "level": "L-g", "top": "L-1" });
    stack.edit("wall.draw", args).await.unwrap();
}

/// Two levels, the partition type and four walls round a 4 m by 3 m box, their centre lines on
/// its edges: inside the faces it is 3885 mm by 2885 mm.
async fn a_box(stack: &Stack) {
    two_levels(stack).await;
    stack
        .edit("wall.type.create", partition_type())
        .await
        .unwrap();
    wall(stack, "W-s", ["0", "0"], ["4 m", "0"]).await;
    wall(stack, "W-e", ["4 m", "0"], ["4 m", "3 m"]).await;
    wall(stack, "W-n", ["4 m", "3 m"], ["0", "3 m"]).await;
    wall(stack, "W-w", ["0", "3 m"], ["0", "0"]).await;
}

/// `room.place`'s arguments: room `id` on Ground at (`x`, `y`) mm, up to First.
fn place(id: &str, x: i64, y: i64) -> Value {
    json!({ "element": id, "name": "Kitchen", "level": "L-g", "at": [x.to_string(), y.to_string()],
            "top": "L-1" })
}

/// What becomes of the room `id` as the document stands.
async fn outcome(stack: &Stack, id: &str) -> Outcome {
    let record = stack.record(id).await.unwrap();
    let mut snapshot = stack.store.snapshot().await.unwrap();
    found_at(&mut snapshot, &i("p"), &i(DOCUMENT), &record, At::Now)
        .await
        .unwrap()
        .unwrap()
}

fn area_of(outcome: &Outcome) -> Result<Area, &'static str> {
    match outcome {
        Outcome::Enclosed { measures, .. } => Ok(measures.area),
        other => Err(other.word()),
    }
}

/// A room is placed inside four walls with a number the server offers, renamed, renumbered,
/// moved, given another upper limit and deleted; a number already used, a point in a wall and
/// every other refusal come back in words and change nothing.
#[tokio::test]
async fn a_room_is_placed_changed_and_deleted_by_its_own_commands() {
    let stack = Stack::new().await;
    a_box(&stack).await;
    stack
        .edit("room.place", place("R-1", 2000, 1500))
        .await
        .unwrap();
    let room = stack.record("R-1").await.unwrap();
    assert_eq!(
        (room.name.as_str(), number_of(&room)),
        ("Kitchen", Some("1"))
    );
    assert_eq!(
        area_of(&outcome(&stack, "R-1").await),
        Ok(Area::from_mm2(3885 * 2885))
    );
    // The next free number is offered, and a room that is not enclosed is still a room.
    let mut elsewhere = place("R-2", 20_000, 20_000);
    elsewhere["name"] = json!("Yard");
    stack.edit("room.place", elsewhere).await.unwrap();
    assert_eq!(number_of(&stack.record("R-2").await.unwrap()), Some("2"));
    assert_eq!(area_of(&outcome(&stack, "R-2").await), Err("not enclosed"));
    let failed = stack.bounds("R-2").await.unwrap_err();
    assert!(
        failed.starts_with("NotEnclosed: room 2 Yard is not enclosed"),
        "{failed}"
    );

    stack
        .edit("room.rename", json!({ "element": "R-1", "name": "Pantry" }))
        .await
        .unwrap();
    assert_eq!(stack.record("R-1").await.unwrap().name, "Pantry");
    stack
        .edit("room.number", json!({ "element": "R-1", "number": "101" }))
        .await
        .unwrap();
    assert_eq!(number_of(&stack.record("R-1").await.unwrap()), Some("101"));
    let mut third = place("R-3", 20_000, 21_000);
    third["number"] = json!("G.04");
    stack.edit("room.place", third).await.unwrap();
    let mut fourth = place("R-4", 20_000, 22_000);
    fourth.as_object_mut().unwrap().remove("name");
    stack.edit("room.place", fourth).await.unwrap();
    let fourth = stack.record("R-4").await.unwrap();
    assert_eq!(
        (fourth.name.as_str(), number_of(&fourth)),
        ("Room", Some("102"))
    );

    // Refused, in words, each changing nothing.
    let rows = stack.rows("piece").await;
    for (cmd, args, words) in [
        (
            "room.number",
            json!({ "element": "R-1", "number": "2" }),
            "Number 2 is already used by room Yard: give this room another number.",
        ),
        (
            "room.place",
            json!({ "element": "R-5", "number": "101", "level": "L-g", "at": ["1 m", "1 m"],
                    "top": "L-1" }),
            "Number 101 is already used by room Pantry: give this room another number.",
        ),
        (
            "room.number",
            json!({ "element": "R-1", "number": " " }),
            "A room needs a number.",
        ),
        (
            "room.rename",
            json!({ "element": "R-1", "name": "" }),
            "A room needs a name.",
        ),
        (
            "room.move",
            json!({ "element": "R-1", "at": ["0", "1500"] }),
            "The room's point is in wall W-w: place it in the open space the room is.",
        ),
        (
            "room.place",
            json!({ "element": "R-6", "level": "L-g", "at": ["1 m", "1 m"] }),
            "A room needs its upper limit tied to a level or a height.",
        ),
        (
            "room.place",
            json!({ "element": "R-6", "level": "L-g", "at": ["1 m", "1 m"], "top": "L-1",
                    "height": "2 m" }),
            "A room's upper limit is tied to a level or given as a height, not both.",
        ),
        (
            "room.place",
            json!({ "element": "R-6", "at": ["1 m", "1 m"], "height": "2 m" }),
            "A room stands on a level: this command needs a level.",
        ),
        (
            "room.place",
            json!({ "element": "R-6", "level": "L-g", "at": ["1 m", "1 m"], "height": "0" }),
            "The room's upper limit is at or below its base.",
        ),
        (
            "room.place",
            place("R-1", 1000, 1000),
            "That element already exists.",
        ),
        (
            "room.rename",
            json!({ "element": "W-s", "name": "X" }),
            "W-s is not a room.",
        ),
        (
            "room.delete",
            json!({ "element": "nobody" }),
            "That element no longer exists.",
        ),
    ] {
        let refused = stack.edit(cmd, args).await.unwrap_err();
        assert_eq!(refused, words, "{cmd}");
    }
    // A level that is not in the model, a top at the base and text that is no length.
    for args in [
        json!({ "element": "R-6", "level": "L-none", "at": ["1 m", "1 m"], "top": "L-1" }),
        json!({ "element": "R-6", "level": "L-g", "top": "L-g", "at": ["1 m", "1 m"] }),
        json!({ "element": "R-6", "level": "L-g", "at": ["one", "1 m"], "top": "L-1" }),
    ] {
        let refused = stack.edit("room.place", args).await.unwrap_err();
        assert_ne!(refused, "", "a refusal has words");
    }
    assert_eq!(stack.rows("piece").await, rows, "a refusal changes nothing");

    // Moved inside the box, it is the same room; given a height, its volume follows.
    stack
        .edit(
            "room.move",
            json!({ "element": "R-1", "at": ["1 m", "1 m"] }),
        )
        .await
        .unwrap();
    assert_eq!(
        area_of(&outcome(&stack, "R-1").await),
        Ok(Area::from_mm2(3885 * 2885))
    );
    stack
        .edit(
            "room.upper",
            json!({ "element": "R-1", "height": "2400 mm" }),
        )
        .await
        .unwrap();
    let Outcome::Enclosed { measures, .. } = outcome(&stack, "R-1").await else {
        panic!("enclosed");
    };
    assert_eq!(
        measures.volume,
        construct_params::Volume::from_mm3(3885 * 2885 * 2400)
    );
    assert!(
        !stack
            .record("R-1")
            .await
            .unwrap()
            .props
            .contains_key("@top.level"),
        "a given height ties no top"
    );
    stack
        .edit(
            "room.upper",
            json!({ "element": "R-1", "top": "L-1", "top_offset": "-100 mm" }),
        )
        .await
        .unwrap();
    let Outcome::Enclosed { measures, .. } = outcome(&stack, "R-1").await else {
        panic!("enclosed");
    };
    assert_eq!(
        measures.volume,
        construct_params::Volume::from_mm3(3885 * 2885 * 2900)
    );

    stack
        .edit("room.delete", json!({ "element": "R-2" }))
        .await
        .unwrap();
    assert!(stack.record("R-2").await.is_none());
}

/// A wall deleted opens the room round it and the room stays, not enclosed; drawn again it is
/// enclosed; a second room in the same loop is redundant, naming the first, and is not refused.
#[tokio::test]
async fn a_rooms_walls_change_and_regeneration_rebuilds_it() {
    let stack = Stack::new().await;
    a_box(&stack).await;
    stack
        .edit("room.place", place("R-1", 2000, 1500))
        .await
        .unwrap();
    stack
        .edit("element.delete", json!({ "element": "W-w" }))
        .await
        .unwrap();
    assert_eq!(area_of(&outcome(&stack, "R-1").await), Err("not enclosed"));
    assert!(stack.record("R-1").await.is_some(), "its record stays");
    let failed = stack.bounds("R-1").await.unwrap_err();
    assert!(
        failed.starts_with("NotEnclosed: room 1 Kitchen"),
        "{failed}"
    );
    wall(&stack, "W-w", ["0", "3 m"], ["0", "0"]).await;
    stack.bounds("R-1").await.unwrap();
    stack
        .edit("room.place", place("R-2", 1000, 1000))
        .await
        .unwrap();
    assert_eq!(
        stack.bounds("R-2").await.unwrap_err().split(':').next(),
        Some("Redundant")
    );
    assert_eq!(
        outcome(&stack, "R-2").await,
        Outcome::Redundant { first: i("R-1") }
    );
}

/// The generic commands refuse a room, naming the command to use; another person who is working
/// on a room turns a change back.
#[tokio::test]
async fn the_generic_commands_refuse_a_room_and_name_the_right_one() {
    let stack = Stack::new().await;
    a_box(&stack).await;
    stack
        .edit("room.place", place("R-1", 2000, 1500))
        .await
        .unwrap();
    let recipe = stack.record("R-1").await.unwrap().recipe;
    for (cmd, args, words) in [
        (
            "element.create",
            json!({ "element": "R-9", "kind": "room", "level": 0, "recipe": recipe }),
            "A room is placed with room.place, which checks it as a room.",
        ),
        (
            "element.set",
            json!({ "element": "R-1", "name": "X" }),
            "Kitchen is changed with room.rename, room.number, room.move or room.upper, which check it as a room.",
        ),
        (
            "element.move",
            json!({ "element": "R-1", "placement": { "x": 1, "y": 1, "z": 0 } }),
            "Kitchen is moved with room.move, which checks it as a room.",
        ),
        (
            "element.delete",
            json!({ "element": "R-1" }),
            "Kitchen is deleted with room.delete, which checks it as a room.",
        ),
    ] {
        assert_eq!(stack.edit(cmd, args).await.unwrap_err(), words, "{cmd}");
    }
    let refused = stack
        .edit_as(
            "ben",
            "room.rename",
            json!({ "element": "R-1", "name": "X" }),
        )
        .await
        .unwrap_err();
    assert!(refused.ends_with("is working on this."), "{refused}");
    stack.later();
    stack
        .edit_as(
            "ben",
            "room.rename",
            json!({ "element": "R-1", "name": "X" }),
        )
        .await
        .unwrap();
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
async fn every_room_command_is_undone_and_redone() {
    let stack = Stack::new().await;
    a_box(&stack).await;
    for (cmd, args) in [
        ("room.place", place("R-1", 2000, 1500)),
        ("room.rename", json!({ "element": "R-1", "name": "Pantry" })),
        ("room.number", json!({ "element": "R-1", "number": "101" })),
        (
            "room.move",
            json!({ "element": "R-1", "at": ["1 m", "1 m"] }),
        ),
        (
            "room.upper",
            json!({ "element": "R-1", "height": "2400 mm" }),
        ),
        ("room.delete", json!({ "element": "R-1" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
    assert!(stack.record("R-1").await.is_none());
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    a_box(&stack).await;
    for (cmd, args) in [
        ("room.place", place("R-1", 2000, 1500)),
        ("room.place", place("R-2", 1000, 1000)),
        ("room.number", json!({ "element": "R-2", "number": "1" })),
        ("element.delete", json!({ "element": "W-w" })),
        (
            "room.move",
            json!({ "element": "R-1", "at": ["0", "1500"] }),
        ),
        ("edit.undo", json!({})),
        ("room.delete", json!({ "element": "R-1" })),
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
