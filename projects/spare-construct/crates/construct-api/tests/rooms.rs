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

/// A room separation line splits an open plan into two rooms through the log: drawn, moved and
/// deleted by its own commands, each one step to undo and redo; the rooms it touches follow it by
/// regeneration alone; a line that crosses nothing is kept; the refusals are in words; and the
/// generic commands name the right one.
#[tokio::test]
async fn a_separation_line_splits_two_rooms_and_follows_its_commands() {
    let stack = Stack::new().await;
    a_box(&stack).await;
    stack
        .edit("room.place", place("R-1", 500, 1500))
        .await
        .unwrap();
    // The second room, in the same loop: it is redundant until the line is drawn.
    stack
        .edit("room.place", place("R-2", 3000, 1500))
        .await
        .unwrap();
    assert_eq!(
        outcome(&stack, "R-2").await,
        Outcome::Redundant { first: i("R-1") }
    );

    let draw =
        json!({ "element": "RL-1", "level": "L-g", "from": ["2000", "0"], "to": ["2000", "3 m"] });
    undone_and_redone(&stack, "roomline.draw", draw).await;
    // The walls' faces are 57.5 mm either side of their centre lines: 2000 - 57.5 = 1942.5 mm, by
    // the 2885 mm between the south and north walls' faces.
    let half = Area::from_square_steps(i128::from(19_425 * 640) * i128::from(2885 * 6400));
    assert_eq!(area_of(&outcome(&stack, "R-1").await), Ok(half));
    assert_eq!(area_of(&outcome(&stack, "R-2").await), Ok(half));
    assert_eq!(
        stack.bounds("R-2").await,
        Ok(([2000, 57, 0], [3943, 2943, 3000]))
    );
    let line = stack.record("RL-1").await.unwrap();
    assert_eq!(line.kind, "room_line");
    assert_eq!(
        stack.bounds("RL-1").await,
        Ok(([2000, 0, 0], [2000, 3000, 0]))
    );

    // Moved 1 m west: the left room narrows to 942.5 mm and the right widens to 2942.5 mm.
    let moved = json!({ "element": "RL-1", "from": ["1000", "0"], "to": ["1000", "3 m"] });
    undone_and_redone(&stack, "roomline.move", moved).await;
    let narrow = Area::from_square_steps(i128::from(9_425 * 640) * i128::from(2885 * 6400));
    let wide = Area::from_square_steps(i128::from(29_425 * 640) * i128::from(2885 * 6400));
    assert_eq!(area_of(&outcome(&stack, "R-1").await), Ok(narrow));
    assert_eq!(area_of(&outcome(&stack, "R-2").await), Ok(wide));

    // A line that crosses nothing is kept, and changes nothing.
    let far =
        json!({ "element": "RL-2", "level": "L-g", "from": ["20 m", "0"], "to": ["20 m", "3 m"] });
    stack.edit("roomline.draw", far).await.unwrap();
    assert_eq!(area_of(&outcome(&stack, "R-1").await), Ok(narrow));
    assert!(stack.record("RL-2").await.is_some());

    // Deleted: one loop again, and the later room is redundant.
    undone_and_redone(&stack, "roomline.delete", json!({ "element": "RL-1" })).await;
    assert!(stack.record("RL-1").await.is_none());
    assert_eq!(
        outcome(&stack, "R-2").await,
        Outcome::Redundant { first: i("R-1") }
    );

    // Refused, in words.
    for (cmd, args, words) in [
        (
            "roomline.draw",
            json!({ "element": "RL-3", "level": "L-g", "from": ["1 m", "1 m"], "to": ["1 m", "1 m"] }),
            "A line needs two different ends.",
        ),
        (
            "roomline.draw",
            json!({ "element": "RL-3", "from": ["1 m", "1 m"], "to": ["2 m", "1 m"] }),
            "A room separation line is drawn on a level: this command needs a level.",
        ),
        (
            "roomline.draw",
            json!({ "element": "RL-3", "level": "L-g", "to": ["2 m", "1 m"] }),
            "This command needs a first end, `from`.",
        ),
        (
            "roomline.draw",
            json!({ "element": "RL-2", "level": "L-g", "from": ["1 m", "1 m"], "to": ["2 m", "1 m"] }),
            "That element already exists.",
        ),
        (
            "roomline.move",
            json!({ "element": "W-s", "from": ["1 m", "1 m"], "to": ["2 m", "1 m"] }),
            "W-s is not a room separation line.",
        ),
        (
            "roomline.delete",
            json!({ "element": "nobody" }),
            "That element no longer exists.",
        ),
        (
            "element.delete",
            json!({ "element": "RL-2" }),
            "A room separation line RL-2 is changed with roomline.move or roomline.delete, which checks it.",
        ),
        (
            "element.create",
            json!({ "element": "RL-9", "kind": "room_line", "level": 0 }),
            "A room separation line is made with roomline.draw, which checks it.",
        ),
    ] {
        assert_eq!(stack.edit(cmd, args).await.unwrap_err(), words, "{cmd}");
    }
}

/// The cubic steps of the room `R-1`'s volume as its view reads it.
async fn room_volume(stack: &Stack) -> i128 {
    let record = stack.record("R-1").await.unwrap();
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let units = construct_params::Units::default();
    let (worked, view) = construct_api::cloud::rooms::view_of(
        &mut snapshot,
        &i("p"),
        &i(DOCUMENT),
        &record,
        &units,
        At::Now,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(view.volume.is_some());
    worked.1.unwrap().volume.cubic_steps()
}

/// A room's volume is its area times its height less what the floors in its space take: the floor
/// of the level above takes its thickness from the top, a sloped one by its slope; the number is
/// exact and is what the room's view reads.
#[tokio::test]
async fn a_rooms_volume_is_less_what_the_floors_in_its_space_take() {
    let stack = Stack::new().await;
    a_box(&stack).await;
    stack
        .edit("room.place", place("R-1", 2000, 1500))
        .await
        .unwrap();
    let area_steps = i128::from(3885 * 6400) * i128::from(2885 * 6400);
    let height = i128::from(3000 * 6400);
    assert_eq!(room_volume(&stack).await, area_steps * height);

    // A 200 mm floor on First, over the whole box: its underside is 200 mm below the room's top.
    let own = json!({ "type": "ft-200", "name": "Slab 200", "layers": [
        { "function": "structure", "thickness": "200 mm", "material": "concrete-regular" }] });
    stack.edit("floor.type.create", own).await.unwrap();
    let floor = json!({ "element": "F-1", "type": "ft-200", "level": "L-1",
        "boundary": { "outer": [
            { "id": "r0", "at": ["0", "0"] }, { "id": "r1", "at": ["4000", "0"] },
            { "id": "r2", "at": ["4000", "3000"] }, { "id": "r3", "at": ["0", "3000"] }] } });
    stack.edit("floor.create", floor).await.unwrap();
    let thick = i128::from(200 * 6400);
    assert_eq!(room_volume(&stack).await, area_steps * (height - thick));

    // Sloped: its top rises 500 mm over the 4 m along x, so its underside crosses the room's top
    // 1600 mm from the west wall's centre line, and before it the floor takes 200 - x/8 of every
    // column, from the room's west face 57.5 mm in.
    let slope = json!({ "element": "F-1", "slope": { "tail": ["0", "0"], "head": ["4000", "0"],
        "at_tail": "0", "at_head": "500 mm" } });
    stack.edit("floor.slope", slope).await.unwrap();
    let (a, b, t) = (
        i128::from(368_000),
        i128::from(10_240_000),
        i128::from(1_280_000),
    );
    let per_depth = t * (b - a) - (b * b - a * a) / 16;
    let depth = i128::from(2885 * 6400);
    assert_eq!(
        room_volume(&stack).await,
        area_steps * height - per_depth * depth
    );
}

/// An area scheme, an area plan and areas through the log: a gross scheme with an area plan of
/// Ground, six boundary lines drawn round an L, an area placed inside it with the gross area of
/// the L (56 square metres); a boundary line picked from a wall's face; every refusal in words;
/// each command one step to undo and redo; a scheme in use cannot be deleted.
#[tokio::test]
async fn an_area_plan_finds_an_areas_gross_area_from_its_boundary_lines() {
    let stack = Stack::new().await;
    a_box(&stack).await;
    undone_and_redone(
        &stack,
        "areascheme.add",
        json!({ "element": "S-g", "name": "Gross", "measure": "gross" }),
    )
    .await;
    stack
        .edit(
            "areascheme.add",
            json!({ "element": "S-r", "name": "Rentable", "measure": "rentable" }),
        )
        .await
        .unwrap();
    undone_and_redone(
        &stack,
        "view.create",
        json!({ "view": "V-a", "kind": "area_plan", "name": "Ground areas", "level": "L-g",
                "scheme": "S-g" }),
    )
    .await;
    let plan = stack.record("V-a").await.unwrap();
    assert_eq!(
        plan.props.get("@area.scheme").map(String::as_str),
        Some("S-g")
    );

    let corners = [
        ["0", "0"],
        ["10 m", "0"],
        ["10 m", "4 m"],
        ["4 m", "4 m"],
        ["4 m", "8 m"],
        ["0", "8 m"],
    ];
    for k in 0..corners.len() {
        let args = json!({ "element": format!("AL-{k}"), "scheme": "S-g", "level": "L-g",
                           "from": corners[k], "to": corners[(k + 1) % corners.len()] });
        if k == 0 {
            undone_and_redone(&stack, "arealine.draw", args).await;
        } else {
            stack.edit("arealine.draw", args).await.unwrap();
        }
    }
    let at = json!({ "element": "A-1", "scheme": "S-g", "level": "L-g", "at": ["2 m", "2 m"],
                     "name": "Ground gross" });
    undone_and_redone(&stack, "area.place", at).await;
    let area = stack.record("A-1").await.unwrap();
    assert_eq!(
        (area.name.as_str(), construct_rooms::area_number_of(&area)),
        ("Ground gross", Some("1"))
    );
    assert_eq!(
        stack.bounds("A-1").await,
        Ok(([0, 0, 0], [10_000, 8000, 0]))
    );
    let measures = {
        let mut snapshot = stack.store.snapshot().await.unwrap();
        construct_regen::rooms::area_worked_at(&mut snapshot, &i("p"), &i(DOCUMENT), &area, At::Now)
            .await
            .unwrap()
            .unwrap()
            .1
            .unwrap()
    };
    // 10000 x 4000 + 4000 x 4000 = 56 square metres; 36 metres round.
    assert_eq!(measures.area, Area::from_mm2(56_000_000));
    assert_eq!(
        measures.perimeter,
        construct_params::Length::from_mm(36_000)
    );
    // The same boundary lines in another scheme are no part of it, and walls bound no area.
    let other = json!({ "element": "AL-x", "scheme": "S-r", "level": "L-g",
                        "from": ["2 m", "0"], "to": ["2 m", "8 m"] });
    stack.edit("arealine.draw", other).await.unwrap();
    assert_eq!(
        stack.bounds("A-1").await,
        Ok(([0, 0, 0], [10_000, 8000, 0]))
    );

    // A boundary line picked from a wall's face: the south wall's face nearest the click.
    undone_and_redone(
        &stack,
        "arealine.pick",
        json!({ "element": "AL-p", "scheme": "S-r", "wall": "W-s", "near": ["2000", "-300"] }),
    )
    .await;
    let picked = stack.record("AL-p").await.unwrap();
    let ends = construct_rooms::ends_of(&picked.recipe).unwrap();
    assert_eq!(
        ends[0][1], -368_000,
        "the outer face, 57.5 mm south of the wall's line"
    );
    assert_eq!(ends[1][1], -368_000);
    assert!((ends[0][0] - ends[1][0]).abs() >= 4000 * 6400);

    // Moved and deleted, as the room's lines are.
    undone_and_redone(
        &stack,
        "arealine.move",
        json!({ "element": "AL-p", "from": ["0", "-1 m"], "to": ["4 m", "-1 m"] }),
    )
    .await;
    undone_and_redone(&stack, "arealine.delete", json!({ "element": "AL-p" })).await;
    undone_and_redone(
        &stack,
        "area.rename",
        json!({ "element": "A-1", "name": "Gross L" }),
    )
    .await;
    undone_and_redone(
        &stack,
        "area.number",
        json!({ "element": "A-1", "number": "G1" }),
    )
    .await;
    undone_and_redone(
        &stack,
        "area.move",
        json!({ "element": "A-1", "at": ["1 m", "1 m"] }),
    )
    .await;
    undone_and_redone(
        &stack,
        "areascheme.rename",
        json!({ "element": "S-r", "name": "Net lettable" }),
    )
    .await;

    // A line deleted leaves the area, not enclosed; the record is kept.
    stack
        .edit("arealine.delete", json!({ "element": "AL-3" }))
        .await
        .unwrap();
    let failed = stack.bounds("A-1").await.unwrap_err();
    assert!(
        failed.starts_with("NotEnclosed: area G1 Gross L is not enclosed"),
        "{failed}"
    );
    // The warnings say so too, naming the area.
    let warned = {
        let mut snapshot = stack.store.snapshot().await.unwrap();
        construct_api::cloud::warnings::read_warnings(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap()
    };
    let area_warnings: Vec<_> = warned
        .warnings
        .iter()
        .filter(|w| w.kind == "area.not_enclosed")
        .collect();
    assert_eq!(area_warnings.len(), 1);
    assert_eq!(area_warnings[0].elements, ["A-1"]);
    assert_eq!(
        area_warnings[0].words,
        "Area G1 Gross L is not enclosed: the area boundary lines round its point do not close"
    );

    // Refused, in words.
    for (cmd, args, words) in [
        (
            "areascheme.delete",
            json!({ "element": "S-g" }),
            "Gross is used by 7 areas, boundary lines or area plans: delete those first.",
        ),
        (
            "areascheme.add",
            json!({ "element": "S-9", "name": "gross" }),
            "There is already an area scheme named gross: give this one another name.",
        ),
        (
            "areascheme.add",
            json!({ "element": "S-9", "name": "X", "measure": "volume" }),
            "\"volume\" is not a measure. The measures are gross, rentable and other.",
        ),
        (
            "area.place",
            json!({ "element": "A-2", "level": "L-g", "at": ["1 m", "1 m"] }),
            "This command needs an area scheme, `scheme`.",
        ),
        (
            "area.place",
            json!({ "element": "A-2", "scheme": "W-s", "level": "L-g", "at": ["1 m", "1 m"] }),
            "W-s is not an area scheme of this model.",
        ),
        (
            "area.place",
            json!({ "element": "A-2", "scheme": "S-g", "at": ["1 m", "1 m"] }),
            "An area stands on a level: this command needs a level.",
        ),
        (
            "area.place",
            json!({ "element": "A-2", "scheme": "S-g", "level": "L-g", "at": ["1 m", "1 m"],
                    "number": "G1" }),
            "Number G1 is already used by area Gross L in this scheme: give this area another number.",
        ),
        (
            "area.place",
            json!({ "element": "A-1", "scheme": "S-g", "level": "L-g", "at": ["1 m", "1 m"] }),
            "That element already exists.",
        ),
        (
            "arealine.draw",
            json!({ "element": "AL-9", "scheme": "S-g", "level": "L-g", "from": ["0", "0"],
                    "to": ["0", "0"] }),
            "A line needs two different ends.",
        ),
        (
            "arealine.pick",
            json!({ "element": "AL-9", "scheme": "S-g", "wall": "A-1", "near": ["0", "0"] }),
            "Gross L is not a wall.",
        ),
        (
            "arealine.move",
            json!({ "element": "RL-0", "from": ["0", "0"], "to": ["1 m", "0"] }),
            "That element no longer exists.",
        ),
        (
            "view.create",
            json!({ "view": "V-b", "kind": "area_plan", "name": "No scheme", "level": "L-g" }),
            "An area plan needs an area scheme (scheme).",
        ),
        (
            "view.create",
            json!({ "view": "V-b", "kind": "area_plan", "name": "Nowhere", "level": "L-g",
                    "scheme": "W-s" }),
            "W-s is not an area scheme of this model: make one first.",
        ),
        (
            "view.create",
            json!({ "view": "V-b", "kind": "floor_plan", "name": "Plain", "level": "L-g",
                    "scheme": "S-g" }),
            "A floor plan has no area scheme.",
        ),
        (
            "element.delete",
            json!({ "element": "A-1" }),
            "An area Gross L is changed with area.rename, area.number, area.move or area.delete, which checks it.",
        ),
        (
            "element.create",
            json!({ "element": "S-9", "kind": "area_scheme", "level": 0 }),
            "An area scheme is made with areascheme.add, which checks it.",
        ),
    ] {
        assert_eq!(stack.edit(cmd, args).await.unwrap_err(), words, "{cmd}");
    }
    // Deleting the line it holds frees the scheme to go.
    stack
        .edit("arealine.delete", json!({ "element": "AL-x" }))
        .await
        .unwrap();
    undone_and_redone(&stack, "areascheme.delete", json!({ "element": "S-r" })).await;
    assert!(stack.record("S-r").await.is_none());
}
