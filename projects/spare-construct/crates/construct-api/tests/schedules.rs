//! Schedules through the edit log, on the test store: the six schedule commands and their
//! refusals in words, rows worked out from the elements as they are (a row appearing, changing
//! and leaving as elements are added, edited and deleted), a floor's, a roof's and a room's
//! exact read-only quantities pinned to the product's own rows, the generic commands refused for
//! a schedule, a schedule with no shape, and undo and redo of every schedule command.

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

use construct_api::cloud::schedules::{ScheduleTable, read_table};
use construct_api::cloud::{Access, ProjectCommands};
use construct_categories::{DOORS, FLOORS, ROOFS, ROOMS};
use construct_model::element::Element;
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{Project, Team};
use construct_regen::parameters::{self as records};
use construct_regen::stand_in::Boxes;
use construct_schedules::QUANTITIES;
use construct_walls::default_walls;
use kit_core::Id;
use kit_ops::{Decider, Edit, Question};
use kit_store::memory::Memory;
use kit_store::{Kv, Scope, Txn, to_row};
use kit_sync::{Clock, Orderers, Reply};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DOCUMENT: &str = "doc";
/// How long a claim lasts here, as the server's setting would.
const CLAIM_MS: u64 = 30_000;
const PARTITION: &str = "wall-partition-stud-90";
const SCREED: &str = "floor-concrete-150-screed-50";
const STEEL: &str = "roof-steel-rafter-150";

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

/// The orderer's time, set by the test.
struct Fixed(AtomicU64);

impl Clock for Fixed {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

struct Stack {
    store: Memory,
    orderers: Orderers<Memory, ProjectCommands<Memory, Yes>>,
    clock: Arc<Fixed>,
    next: AtomicU64,
}

impl Stack {
    async fn new() -> Self {
        let store = Memory::new();
        let geometry: Arc<dyn Geometry> = Arc::new(Boxes);
        let access = Arc::new(Access::new(store.clone(), Yes));
        let commands = ProjectCommands::new(store.clone(), access, CLAIM_MS, geometry.edition());
        let clock = Arc::new(Fixed(AtomicU64::new(1_000_000)));
        let orderers = Orderers::new(store.clone(), Arc::new(commands), clock.clone(), 50);
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
        let stack = Self {
            store,
            orderers,
            clock,
            next: AtomicU64::new(0),
        };
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
        stack
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

    async fn table(&self, id: &str) -> ScheduleTable {
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_table(&mut snapshot, &i("p"), &i(DOCUMENT), &i(id))
            .await
            .unwrap()
            .unwrap()
            .unwrap()
    }
}

fn recipe(op: &str, args: Value) -> Value {
    serde_json::to_value(Recipe::construct(op, serde_json::from_value(args).unwrap())).unwrap()
}

/// A door with a width, a thickness and a height as plain numbers.
fn door(id: &str, name: &str) -> Value {
    let args = json!({ "width": 820, "thickness": 40, "height": 2100 });
    json!({ "element": id, "kind": "door", "name": name, "level": 1,
            "placement": { "x": 0, "y": 0, "z": 0 }, "recipe": recipe("door.single", args) })
}

/// A mark (text) and a sill height (length) on every door.
async fn door_parameters(stack: &Stack) {
    for (id, name, kind) in [
        ("P-mark", "Mark", "text"),
        ("P-sill", "Sill height", "length"),
    ] {
        let args = json!({ "definition": { "id": id, "name": name, "kind": kind,
                                           "holder": "instance", "choices": [] } });
        stack.edit("parameter.define", args).await.unwrap();
        let bind = json!({ "parameter": id, "categories": [DOORS.0], "default": null });
        stack.edit("parameter.bind", bind).await.unwrap();
    }
}

async fn set(stack: &Stack, door: &str, parameter: &str, value: &str) {
    let args = json!({ "element": door, "parameter": parameter, "value": value });
    stack.edit("element.param", args).await.unwrap();
}

/// The first cell of every row, as shown.
fn shown(table: &ScheduleTable, column: usize) -> Vec<String> {
    table
        .rows
        .iter()
        .map(|row| row.cells[column].shown.clone().unwrap_or_default())
        .collect()
}

fn elements(table: &ScheduleTable) -> Vec<&str> {
    table.rows.iter().map(|row| row.element.as_str()).collect()
}

fn make(id: &str, name: &str, category: u32) -> Value {
    json!({ "schedule": id, "name": name, "category": category })
}

/// Doors with marks and sill heights, and a schedule of them.
async fn doors_with_a_schedule() -> Stack {
    let stack = Stack::new().await;
    door_parameters(&stack).await;
    for (id, name, mark, sill) in [
        ("D-1", "Front door", "D10", "900"),
        ("D-2", "Back door", "D2", "100 mm"),
        ("D-3", "Garage door", "d3", "1.2 m"),
    ] {
        stack.edit("element.create", door(id, name)).await.unwrap();
        set(&stack, id, "P-mark", mark).await;
        set(&stack, id, "P-sill", sill).await;
    }
    let mut args = make("S-1", "Door schedule", DOORS.0);
    args["fields"] = json!(["name", "parameter:P-mark", "parameter:P-sill"]);
    stack.edit("schedule.create", args).await.unwrap();
    stack
}

#[tokio::test]
async fn a_schedule_is_a_record_and_its_rows_follow_the_elements() {
    let stack = doors_with_a_schedule().await;
    let record = stack.record("S-1").await.unwrap();
    assert_eq!(record.kind, "schedule");
    assert_eq!(record.name, "Door schedule");
    assert_eq!(record.props["@category"], "7200");
    assert_eq!(record.props["@schedule.lists"], DOORS.0.to_string());

    // The rows are the doors, in ID order with no sorting, each cell exact and shown with its unit.
    let table = stack.table("S-1").await;
    assert_eq!(table.schedule.category_name, "Doors");
    assert_eq!(table.schedule.problem, None);
    assert_eq!(
        table
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["Name", "Mark", "Sill height"]
    );
    assert_eq!(elements(&table), ["D-1", "D-2", "D-3"]);
    assert_eq!(shown(&table, 0), ["Front door", "Back door", "Garage door"]);
    let sill = table.rows[1].cells[2].clone();
    assert_eq!(sill.shown.as_deref(), Some("100 mm"));
    assert_eq!(sill.value.as_deref(), Some("length:q0:640000"));

    // Sorting by sill height, then filtering on it, through the commands.
    let args = json!({ "schedule": "S-1", "sorting": [{ "field": "parameter:P-sill" }] });
    stack.edit("schedule.sorting", args).await.unwrap();
    assert_eq!(elements(&stack.table("S-1").await), ["D-2", "D-1", "D-3"]);
    let args = json!({ "schedule": "S-1",
        "sorting": [{ "field": "parameter:P-sill", "order": "descending" }] });
    stack.edit("schedule.sorting", args).await.unwrap();
    assert_eq!(elements(&stack.table("S-1").await), ["D-3", "D-1", "D-2"]);
    let args = json!({ "schedule": "S-1", "filters": [
        { "field": "parameter:P-sill", "is": "greater_or_equal", "to": "0.9 m" }] });
    stack.edit("schedule.filters", args).await.unwrap();
    let table = stack.table("S-1").await;
    assert_eq!(elements(&table), ["D-3", "D-1"]);
    assert_eq!(
        table.schedule.filters[0].to, "length:q0:5760000",
        "the filter's value is stored exactly"
    );

    // A row appears: a new door that passes. It changes: a value is edited. It leaves: the door
    // stops passing, then is deleted.
    stack
        .edit("element.create", door("D-4", "Side door"))
        .await
        .unwrap();
    set(&stack, "D-4", "P-mark", "D4").await;
    set(&stack, "D-4", "P-sill", "950").await;
    assert_eq!(elements(&stack.table("S-1").await), ["D-3", "D-4", "D-1"]);
    stack.later();
    set(&stack, "D-4", "P-sill", "980").await;
    let table = stack.table("S-1").await;
    assert_eq!(elements(&table), ["D-3", "D-4", "D-1"]);
    assert_eq!(shown(&table, 2)[1], "980 mm");
    stack.later();
    set(&stack, "D-4", "P-sill", "300").await;
    assert_eq!(elements(&stack.table("S-1").await), ["D-3", "D-1"]);
    stack.later();
    set(&stack, "D-4", "P-sill", "1000").await;
    assert_eq!(elements(&stack.table("S-1").await), ["D-3", "D-4", "D-1"]);
    stack.later();
    stack
        .edit("element.delete", json!({ "element": "D-4" }))
        .await
        .unwrap();
    assert_eq!(elements(&stack.table("S-1").await), ["D-3", "D-1"]);

    // Fields, name: the schedule's own columns change, and nothing else is stored.
    let args = json!({ "schedule": "S-1", "fields": ["parameter:P-mark"] });
    stack.edit("schedule.fields", args).await.unwrap();
    let args = json!({ "schedule": "S-1", "name": "Doors by sill" });
    stack.edit("schedule.rename", args).await.unwrap();
    let table = stack.table("S-1").await;
    assert_eq!(table.schedule.name, "Doors by sill");
    assert_eq!(table.columns.len(), 1);
    assert_eq!(shown(&table, 0), ["d3", "D10"]);

    // Text sorts by the stated rule: case aside.
    let args = json!({ "schedule": "S-1", "filters": [] });
    stack.edit("schedule.filters", args).await.unwrap();
    let args = json!({ "schedule": "S-1", "sorting": [{ "field": "parameter:P-mark" }] });
    stack.edit("schedule.sorting", args).await.unwrap();
    assert_eq!(shown(&stack.table("S-1").await, 0), ["D10", "D2", "d3"]);

    // Deleting a schedule takes its pointer away and leaves the elements.
    stack
        .edit("schedule.delete", json!({ "schedule": "S-1" }))
        .await
        .unwrap();
    assert!(stack.record("S-1").await.is_none());
    assert!(stack.record("D-1").await.is_some());
}

#[tokio::test]
async fn every_refusal_is_in_words_and_changes_nothing() {
    let stack = doors_with_a_schedule().await;
    let before = stack.record("S-1").await;
    let refused = |cmd: &'static str, args: Value| {
        let stack = &stack;
        async move { stack.edit(cmd, args).await.unwrap_err() }
    };

    // A name already taken, case aside, on create and on rename.
    let why = refused("schedule.create", make("S-2", "DOOR SCHEDULE", DOORS.0)).await;
    assert_eq!(
        why,
        "There is already a schedule named \"DOOR SCHEDULE\" in this model."
    );
    stack
        .edit("schedule.create", make("S-2", "Floors", FLOORS.0))
        .await
        .unwrap();
    let why = refused(
        "schedule.rename",
        json!({ "schedule": "S-2", "name": "door schedule" }),
    )
    .await;
    assert!(why.contains("already a schedule named"), "{why}");
    // An unknown category, and one with nothing to list.
    let why = refused("schedule.create", make("S-3", "Nowhere", 424_242)).await;
    assert_eq!(why, "There is no category 424242.");
    let why = refused("schedule.create", make("S-3", "Viewports", 7400)).await;
    assert!(why.starts_with("A schedule cannot list"), "{why}");
    // A field the category does not have.
    let mut args = make("S-3", "Doors", DOORS.0);
    args["fields"] = json!(["quantity:Area"]);
    let why = refused("schedule.create", args).await;
    assert_eq!(why, "Doors have no field quantity:Area.");
    let why = refused(
        "schedule.fields",
        json!({ "schedule": "S-1", "fields": ["parameter:P-none"] }),
    )
    .await;
    assert_eq!(why, "Doors have no field parameter:P-none.");
    let why = refused(
        "schedule.fields",
        json!({ "schedule": "S-1", "fields": ["Name"] }),
    )
    .await;
    assert!(why.contains("is not a field"), "{why}");
    let why = refused(
        "schedule.fields",
        json!({ "schedule": "S-1", "fields": ["name", "name"] }),
    )
    .await;
    assert_eq!(why, "The field name is listed twice.");
    // A filter comparing a number with text, text with a number, a comparison that makes no
    // sense, an unknown comparison, a sort order that is not one.
    let filter = |field: &str, is: &str, to: &str| json!({ "schedule": "S-1", "filters": [{ "field": field, "is": is, "to": to }] });
    let why = refused(
        "schedule.filters",
        filter("parameter:P-sill", "equals", "tall"),
    )
    .await;
    assert!(
        why.starts_with("The filter on Sill height cannot compare it with \"tall\""),
        "{why}"
    );
    let why = refused(
        "schedule.filters",
        filter("parameter:P-sill", "contains", "9"),
    )
    .await;
    assert_eq!(
        why,
        "Sill height holds a length, so a filter on it cannot use contains."
    );
    let why = refused("schedule.filters", filter("name", "like", "x")).await;
    assert!(why.contains("is not a comparison"), "{why}");
    let why = refused("schedule.filters", filter("quantity:Area", "equals", "1")).await;
    assert_eq!(why, "Doors have no field quantity:Area.");
    let args = json!({ "schedule": "S-1", "sorting": [{ "field": "name", "order": "up" }] });
    let why = refused("schedule.sorting", args).await;
    assert!(why.contains("is not an order"), "{why}");
    let args = json!({ "schedule": "S-1", "sorting": [{ "field": "name" }, { "field": "name" }] });
    assert_eq!(
        refused("schedule.sorting", args).await,
        "The field name is sorted on twice."
    );
    // The category is not changed; an unknown schedule; an ID in use; missing arguments.
    let args = json!({ "schedule": "S-1", "category": FLOORS.0, "name": "x" });
    assert!(
        refused("schedule.rename", args)
            .await
            .contains("make another schedule")
    );
    let why = refused("schedule.delete", json!({ "schedule": "S-9" })).await;
    assert_eq!(why, "There is no schedule S-9 in this model.");
    let why = refused("schedule.create", make("S-1", "Again", DOORS.0)).await;
    assert_eq!(why, "That element already exists.");
    let why = refused(
        "schedule.create",
        json!({ "schedule": "S-4", "name": "No category" }),
    )
    .await;
    assert_eq!(why, "This command needs a category.");
    let why = refused("schedule.create", make("S-4", "   ", DOORS.0)).await;
    assert_eq!(why, "A schedule needs a name.");

    // Nothing was written by any of them.
    assert_eq!(stack.record("S-1").await, before);
    assert!(stack.record("S-3").await.is_none());
    assert!(stack.record("S-4").await.is_none());

    // The generic commands are not a way round the schedule commands.
    let generic = json!({ "element": "S-5", "kind": "schedule", "level": 0,
        "placement": { "x": 0, "y": 0, "z": 0 }, "recipe": recipe("schedule", json!({})) });
    let why = refused("element.create", generic).await;
    assert_eq!(why, "A schedule is made: use schedule.create.");
    let why = refused("element.set", json!({ "element": "S-1", "name": "Hacked" })).await;
    assert!(
        why.starts_with("A schedule is changed: use schedule.rename"),
        "{why}"
    );
    let why = refused(
        "element.move",
        json!({ "element": "S-1", "placement": { "x": 1, "y": 1, "z": 1 } }),
    )
    .await;
    assert!(why.starts_with("A schedule is changed"), "{why}");
    let why = refused("element.delete", json!({ "element": "S-1" })).await;
    assert_eq!(why, "A schedule is deleted: use schedule.delete.");
    let why = refused(
        "element.param",
        json!({ "element": "S-1", "parameter": "P-mark", "value": "x" }),
    )
    .await;
    assert!(why.contains("not as elements"), "{why}");
    assert_eq!(stack.record("S-1").await, before);

    // Another person's claim on the schedule refuses a change in the claimant's words.
    stack.later();
    let args = json!({ "schedule": "S-1", "name": "Ben's name" });
    stack.edit_as("ben", "schedule.rename", args).await.unwrap();
    let args = json!({ "schedule": "S-1", "name": "Renamed by Anna" });
    let why = stack.edit("schedule.rename", args).await.unwrap_err();
    assert!(why.ends_with("is working on this."), "{why}");
}

#[tokio::test]
async fn every_schedule_command_can_be_undone_and_redone() {
    let stack = doors_with_a_schedule().await;
    let commands = [
        ("schedule.create", make("S-2", "Floors", FLOORS.0), "S-2"),
        (
            "schedule.rename",
            json!({ "schedule": "S-1", "name": "Renamed" }),
            "S-1",
        ),
        (
            "schedule.fields",
            json!({ "schedule": "S-1", "fields": ["name"] }),
            "S-1",
        ),
        (
            "schedule.filters",
            json!({ "schedule": "S-1", "filters": [{ "field": "name", "is": "begins_with", "to": "F" }] }),
            "S-1",
        ),
        (
            "schedule.sorting",
            json!({ "schedule": "S-1", "sorting": [{ "field": "name", "order": "descending" }] }),
            "S-1",
        ),
        ("schedule.delete", json!({ "schedule": "S-1" }), "S-1"),
    ];
    for (cmd, args, id) in &commands {
        stack.later();
        let before = stack.record(id).await;
        stack.edit(cmd, args.clone()).await.unwrap();
        let after = stack.record(id).await;
        assert_ne!(before, after, "{cmd} wrote");
        stack.later();
        stack.edit("edit.undo", json!({})).await.unwrap();
        assert_eq!(stack.record(id).await, before, "{cmd} undone");
        stack.later();
        stack.edit("edit.redo", json!({})).await.unwrap();
        assert_eq!(stack.record(id).await, after, "{cmd} redone");
    }
    // Everything back, step by step, in reverse: the schedules are as they began.
    for _ in &commands {
        stack.later();
        stack.edit("edit.undo", json!({})).await.unwrap();
    }
    let table = stack.table("S-1").await;
    assert_eq!(table.schedule.name, "Door schedule");
    assert_eq!(elements(&table), ["D-1", "D-2", "D-3"]);
    assert!(stack.record("S-2").await.is_none());
}

/// The partition type and four walls round a 4 m by 3 m box, on Ground and First levels.
async fn a_box(stack: &Stack) {
    for (id, name, elevation) in [("L-g", "Ground", "0"), ("L-1", "First", "3 m")] {
        let args = json!({ "level": id, "name": name, "elevation": elevation });
        stack.edit("level.create", args).await.unwrap();
    }
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
    let args = json!({ "type": PARTITION, "name": wall.name, "assembly": assembly });
    stack.edit("wall.type.create", args).await.unwrap();
    for (id, start, end) in [
        ("W-s", ["0", "0"], ["4 m", "0"]),
        ("W-e", ["4 m", "0"], ["4 m", "3 m"]),
        ("W-n", ["4 m", "3 m"], ["0", "3 m"]),
        ("W-w", ["0", "3 m"], ["0", "0"]),
    ] {
        let args = json!({ "element": id, "type": PARTITION, "start": start, "end": end,
                           "level": "L-g", "top": "L-1" });
        stack.edit("wall.draw", args).await.unwrap();
    }
}

fn at(id: &str, x: i64, y: i64) -> Value {
    json!({ "id": id, "at": [x.to_string(), y.to_string()] })
}

fn rectangle(x: i64, y: i64) -> Value {
    json!({ "outer": [at("r0", 0, 0), at("r1", x, 0), at("r2", x, y), at("r3", 0, y)] })
}

/// A floor, a roof and a room, and a schedule of each listing every quantity the table gives its
/// category: each cell is exactly what the product's own read-only row for that element says, and
/// the table lists every quantity of the product's rows but a roof's pitch and per-layer ones.
#[tokio::test]
async fn quantities_are_the_products_own_exact_read_only_rows() {
    let stack = Stack::new().await;
    a_box(&stack).await;
    stack.edit("floor.defaults", json!({})).await.unwrap();
    let args = json!({ "element": "F-1", "type": SCREED, "level": "L-1",
                       "boundary": rectangle(4000, 3000) });
    stack.edit("floor.create", args).await.unwrap();
    stack.edit("roof.defaults", json!({})).await.unwrap();
    let args = json!({ "element": "R-1", "type": STEEL, "level": "L-1",
                       "footprint": rectangle(8000, 6000), "form": "gable", "pitch": "1:4" });
    stack.edit("roof.create", args).await.unwrap();
    let args = json!({ "element": "M-1", "name": "Kitchen", "level": "L-g",
                       "at": ["2000", "1500"], "top": "L-1" });
    stack.edit("room.place", args).await.unwrap();
    let args = json!({ "element": "M-2", "name": "Yard", "level": "L-g",
                       "at": ["20000", "20000"], "top": "L-1" });
    stack.edit("room.place", args).await.unwrap();

    let mut snapshot = stack.store.snapshot().await.unwrap();
    let context = records::read_context(&mut snapshot, &i("p"), &i(DOCUMENT))
        .await
        .unwrap()
        .unwrap();
    for (id, element, category) in [("S-f", "F-1", FLOORS), ("S-r", "R-1", ROOFS)] {
        let record = stack.record(element).await.unwrap();
        let ty = records::type_of(&record).unwrap();
        let ty = records::read_type(&mut snapshot, &i("p"), &i(DOCUMENT), &ty, &context)
            .await
            .unwrap()
            .unwrap();
        let view = context.view_cut(&record, Some(&ty), &[]).unwrap();
        let own: Vec<_> = view
            .parameters
            .iter()
            .filter(|p| p.from == "read-only")
            .collect();
        let ours: Vec<_> = QUANTITIES
            .iter()
            .filter(|q| q.category == category)
            .collect();
        // Every quantity in the table is a row of the product, of the kind the table says...
        for q in &ours {
            let row = own
                .iter()
                .find(|p| p.name == q.name)
                .unwrap_or_else(|| panic!("{}", q.name));
            assert_eq!(row.kind, q.kind.to_string(), "{}", q.name);
            assert_eq!(row.id, q.id, "{}", q.name);
        }
        // ...and every row of the product is in it but a roof's pitch and the per-layer ones.
        for row in &own {
            let known = ours.iter().any(|q| q.name == row.name);
            assert!(
                known || row.name == "Pitch" || row.name.starts_with("Layer "),
                "{}",
                row.name
            );
        }
        let mut args = make(id, &format!("Quantities {element}"), category.0);
        let keys: Vec<String> = ours
            .iter()
            .map(|q| format!("quantity:{}", q.name))
            .collect();
        args["fields"] = json!(keys);
        stack.edit("schedule.create", args).await.unwrap();
        let table = stack.table(id).await;
        assert_eq!(elements(&table), [element]);
        for (at, q) in ours.iter().enumerate() {
            let row = own.iter().find(|p| p.name == q.name).unwrap();
            let cell = &table.rows[0].cells[at];
            assert_eq!(cell.value, row.value, "{}", q.name);
            assert_eq!(cell.shown, row.shown, "{}", q.name);
            assert_eq!(cell.value.is_some(), row.value.is_some(), "{}", q.name);
            if q.name != "Mass (kg)" {
                assert!(cell.value.is_some(), "{} has a value", q.name);
            }
        }
    }

    // A room: its state always, its area, perimeter and volume when it is enclosed; the exact
    // values are the measures the room search gives, not the shown text.
    let mut args = make("S-m", "Rooms", ROOMS.0);
    args["fields"] = json!([
        "name",
        "quantity:State",
        "quantity:Area",
        "quantity:Perimeter",
        "quantity:Volume"
    ]);
    args["sorting"] = json!([{ "field": "name" }]);
    stack.edit("schedule.create", args).await.unwrap();
    let table = stack.table("S-m").await;
    assert_eq!(elements(&table), ["M-1", "M-2"]);
    let kitchen = &table.rows[0].cells;
    assert_eq!(kitchen[1].shown.as_deref(), Some("enclosed"));
    // 3885 mm by 2885 mm inside the faces: 11,208,225 mm2 exactly.
    let exact = construct_params::Value::Area(construct_params::Area::from_mm2(11_208_225));
    assert_eq!(kitchen[2].value, Some(exact.stored()));
    assert!(
        kitchen[3]
            .value
            .as_deref()
            .unwrap()
            .starts_with("length:q0:")
    );
    assert!(
        kitchen[4]
            .value
            .as_deref()
            .unwrap()
            .starts_with("volume:q0:")
    );
    let yard = &table.rows[1].cells;
    assert_eq!(yard[1].shown.as_deref(), Some("not enclosed"));
    assert_eq!(yard[2].value, None);
    // A filter on a room's exact area lists the kitchen and not the yard, whose area is absent.
    let args = json!({ "schedule": "S-m", "filters": [
        { "field": "quantity:Area", "is": "greater", "to": "11 m2" }] });
    stack.edit("schedule.filters", args).await.unwrap();
    assert_eq!(elements(&stack.table("S-m").await), ["M-1"]);
    let args = json!({ "schedule": "S-m", "filters": [
        { "field": "quantity:State", "is": "equals", "to": "not enclosed" }] });
    stack.edit("schedule.filters", args).await.unwrap();
    assert_eq!(elements(&stack.table("S-m").await), ["M-2"]);

    // A schedule is never listed among the elements: it has no shape.
    let table = stack.table("S-f").await;
    assert_eq!(table.schedule.id, "S-f");
    let record = stack.record("S-f").await.unwrap();
    assert!(!records::has_shape(&record));
}
